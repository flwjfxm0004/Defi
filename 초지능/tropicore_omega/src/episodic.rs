//! Non-Parametric PPM-D / Witten-Bell 2-Way LRU Context-Tree (Ord 1..=8) + Adaptive Order-0 Unigram Prior
//! + Tropical-L1 + 4-Way Order-∞ Match-Model + Zero-Tokenizer WordBigram + Skip-1/Gapped Context Expert
//! + 13-Expert × 8-Context Simplex-Normalized Calibrated-Temperature Meta-Mixer (`TropiCore-Omega v17.0`).
//!
//! Total 13 Experts unified in an 8-context-bucket exact simplex (`sum(alpha_e) == 1.0`) with learned `temp[ctx]`:
//!   - Expert 0    : Deep SwiGLU + Conv-4 + Separate-Q/K DeltaNet-SSM + Diff-EqReg Adaptive Banach Brain
//!   - Experts 1..8: 2-Way LRU Exact Suffix CTW (`orders 1..=8`) with exact PPM-D escape backed by adaptive `p_uni`
//!   - Expert 9    : Tropical-L1 Latent Key-Value Recall backed by adaptive `p_uni`
//!   - Expert 10   : 4-Way Order-∞ Exact Match-Model (`4..128 B` copies, unigram-backed non-match tail)
//!   - Expert 11   : Zero-Tokenizer Word-Bigram & Long-Identifier Prefix Expert (PPM-D + `p_uni`)
//!   - Expert 12   : Skip-1 / Gapped Context Expert (`[b_{n-4},b_{n-3},b_{n-2}]` & `[b_{n-3},b_{n-1}]`, PPM-D + `p_uni`)

pub const NUM_EPISODIC_EXPERTS: usize = 12; // 0..8=CTW1..8, 8=Tropical-L1, 9=4-Way Match, 10=WordBigram, 11=Skip1
pub const NUM_TOTAL_EXPERTS: usize = 13;    // 0 = Neural Brain, 1..=12 = Episodic Experts
pub const NUM_MIXER_CONTEXTS: usize = 8;

const CTW_NUM_SETS: usize = 32_768;
const CTW_SET_MASK: usize = CTW_NUM_SETS - 1;
const CTW_WAYS: usize = 2;
const CTW_SLOTS: usize = CTW_NUM_SETS * CTW_WAYS;
const MAX_ENTRIES_PER_SLOT: usize = 8;

const MATCH_HIST_CAP: usize = 65_536;
const MATCH_HASH_SIZE: usize = 65_536;
const MATCH_HASH_MASK: usize = MATCH_HASH_SIZE - 1;
const MATCH_WAYS: usize = 4;

/// Strictly causal context classifier (`0..8`) driven ONLY by previous observed byte & causal match length!
#[inline]
pub fn mixer_context_bucket(prev_byte: u8, match_len: usize) -> usize {
    if match_len >= 12 {
        return 7; // Long exact copy regime (4-Way Order-∞ Match-Model dominates!)
    }
    if match_len >= 5 {
        return 6; // Medium exact match regime
    }
    match prev_byte {
        b'\n' | b'\r' => 0,
        b' ' | b'\t' => 1,
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => 2,
        b'0'..=b'9' => 3,
        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'.' | b',' | b';' | b':' | b'='
        | b'+' | b'-' | b'*' | b'/' | b'<' | b'>' | b'&' | b'|' | b'!' | b'"' | b'\'' => 4,
        _ => 5,
    }
}

/// 8-Context-Bucket × 13-Expert Simplex-Normalized Log-Opinion Pool + Calibrated Online Temperature (`temp[ctx]`).
/// Guarantees `sum_{e active} alpha[e] == 1.0` EXACTLY!
#[derive(Clone, Debug)]
pub struct OnlineLogisticMetaMixer {
    pub theta: [[f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
    pub temp: [f32; NUM_MIXER_CONTEXTS],
    pub grad_sq_theta: [[f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
    pub grad_sq_temp: [f32; NUM_MIXER_CONTEXTS],
    pub updates: u64,
}

impl OnlineLogisticMetaMixer {
    pub fn new() -> Self {
        // Log-space simplex priors: [Neural, CTW1..8, Tropical-L1, 4-Way Match, WordBigram, Skip1/Gapped]
        let base_theta = [
            0.85f32, // 0: Neural Brain
            -1.20, -0.60, -0.05, 0.35, 0.68, 0.96, 1.22, 1.44, // 1..=8: CTW Ord 1..8 (PPM-D!)
            -0.85,   // 9: Tropical-L1
            1.95,    // 10: 4-Way Order-∞ Match-Model
            1.25,    // 11: Zero-Tokenizer WordBigramExpert
            0.65,    // 12: Skip-1 / Gapped Context Expert
        ];
        let mut theta = [base_theta; NUM_MIXER_CONTEXTS];
        // Space, identifier, and operator transitions benefit strongly from WordBigram + Skip-1
        theta[1][11] = 1.48;
        theta[2][11] = 1.48;
        theta[4][11] = 1.42;
        theta[3][12] = 0.95; // Digits/indices benefit heavily from Skip-1 gapped patterns!
        theta[4][12] = 0.95; // Punctuation/operators benefit from Skip-1!
        // Context 6 (medium match) & Context 7 (long match) start with stronger 4-Way Match-Model mass
        theta[6][10] = 2.40;
        theta[7][10] = 3.05;

        let mut temp = [1.08f32; NUM_MIXER_CONTEXTS];
        temp[6] = 1.18;
        temp[7] = 1.26;

        Self {
            theta,
            temp,
            grad_sq_theta: [[1.0; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
            grad_sq_temp: [1.0; NUM_MIXER_CONTEXTS],
            updates: 0,
        }
    }

    /// Exact active-expert probability simplex: `alpha[e] >= 0`, `sum_e alpha[e] == 1.0` ALWAYS!
    #[inline]
    pub fn compute_active_simplex(
        &self,
        ctx: usize,
        active: &[bool; NUM_EPISODIC_EXPERTS],
        w_mem_scale: f32,
        w_mem_cap: f32,
    ) -> [f32; NUM_TOTAL_EXPERTS] {
        let c_idx = ctx.min(NUM_MIXER_CONTEXTS - 1);
        let th = &self.theta[c_idx];

        let mut mx = th[0];
        for k in 0..NUM_EPISODIC_EXPERTS {
            if active[k] && th[k + 1] > mx {
                mx = th[k + 1];
            }
        }

        let mut raw = [0.0f32; NUM_TOTAL_EXPERTS];
        raw[0] = (th[0] - mx).max(-20.0).exp();
        let mut ep_sum = 0.0f32;
        for k in 0..NUM_EPISODIC_EXPERTS {
            if active[k] {
                let v = (th[k + 1] - mx).max(-20.0).exp();
                raw[k + 1] = v;
                ep_sum += v;
            }
        }
        if ep_sum <= 1e-12 {
            let mut only_brain = [0.0f32; NUM_TOTAL_EXPERTS];
            only_brain[0] = 1.0;
            return only_brain;
        }

        let tot = raw[0] + ep_sum;
        let a0 = raw[0] / tot;
        let a_ep = ep_sum / tot;

        let eff_cap = if active[9] {
            w_mem_cap.max(0.96).clamp(0.10, 0.985)
        } else {
            w_mem_cap.clamp(0.10, 0.96)
        };
        let scaled_ep = a_ep * w_mem_scale.max(0.05);
        let target_ep = (scaled_ep / (a0 + scaled_ep).max(1e-8)).min(eff_cap);
        let target_0 = 1.0 - target_ep;

        let mut alpha = [0.0f32; NUM_TOTAL_EXPERTS];
        alpha[0] = target_0;
        let ep_mul = target_ep / ep_sum;
        for k in 0..NUM_EPISODIC_EXPERTS {
            if active[k] {
                alpha[k + 1] = raw[k + 1] * ep_mul;
            }
        }
        alpha
    }

    pub fn mix_256(
        &self,
        ctx: usize,
        p_brain: &[f32; 256],
        experts: &[[f32; 256]; NUM_EPISODIC_EXPERTS],
        active: &[bool; NUM_EPISODIC_EXPERTS],
        w_mem_scale: f32,
        w_mem_cap: f32,
        use_log_odds: bool,
    ) -> [f32; 256] {
        let mut any_active = false;
        for &a in active {
            if a {
                any_active = true;
                break;
            }
        }
        if !any_active {
            return *p_brain;
        }

        let c_idx = ctx.min(NUM_MIXER_CONTEXTS - 1);
        let alpha = self.compute_active_simplex(c_idx, active, w_mem_scale, w_mem_cap);
        let tau = self.temp[c_idx].clamp(0.55, 2.25);

        let mut out = [0.0f32; 256];
        if use_log_odds {
            let mut lbuf = [0.0f32; 256];
            let a0 = alpha[0];
            if a0 > 1e-6 {
                for c in 0..256 {
                    lbuf[c] = a0 * p_brain[c].max(1e-7).ln();
                }
            }
            for k in 0..NUM_EPISODIC_EXPERTS {
                if active[k] {
                    let ak = alpha[k + 1];
                    if ak > 1e-6 {
                        let ek = &experts[k];
                        for c in 0..256 {
                            lbuf[c] += ak * ek[c].max(1e-7).ln();
                        }
                    }
                }
            }
            let mut mx = f32::NEG_INFINITY;
            for c in 0..256 {
                lbuf[c] *= tau;
                if lbuf[c] > mx {
                    mx = lbuf[c];
                }
            }
            let mut sum = 0.0f32;
            for c in 0..256 {
                let v = (lbuf[c] - mx).max(-40.0).exp().max(1e-7);
                out[c] = v;
                sum += v;
            }
            let inv = 1.0 / sum.max(1e-12);
            for c in 0..256 {
                out[c] *= inv;
            }
        } else {
            let a0 = alpha[0];
            for c in 0..256 {
                out[c] = a0 * p_brain[c];
            }
            for k in 0..NUM_EPISODIC_EXPERTS {
                if active[k] {
                    let ak = alpha[k + 1];
                    if ak > 1e-6 {
                        let ek = &experts[k];
                        for c in 0..256 {
                            out[c] += ak * ek[c];
                        }
                    }
                }
            }
            let mut sum = 0.0f32;
            for c in 0..256 {
                out[c] = out[c].max(1e-7);
                sum += out[c];
            }
            let inv = 1.0 / sum.max(1e-12);
            for c in 0..256 {
                out[c] *= inv;
            }
        }
        out
    }

    pub fn observe_online(
        &mut self,
        ctx: usize,
        p_brain: &[f32; 256],
        experts: &[[f32; 256]; NUM_EPISODIC_EXPERTS],
        active: &[bool; NUM_EPISODIC_EXPERTS],
        p_mix: &[f32; 256],
        actual_byte: u8,
        w_mem_scale: f32,
        w_mem_cap: f32,
        use_log_odds: bool,
        lr: f32,
    ) {
        let mut any_active = false;
        for &a in active {
            if a {
                any_active = true;
                break;
            }
        }
        if !any_active {
            return;
        }

        self.updates = self.updates.saturating_add(1);
        let c_idx = ctx.min(NUM_MIXER_CONTEXTS - 1);
        let alpha = self.compute_active_simplex(c_idx, active, w_mem_scale, w_mem_cap);
        let tau = self.temp[c_idx].clamp(0.55, 2.25);
        let y = actual_byte as usize;

        let mut g_alpha = [0.0f32; NUM_TOTAL_EXPERTS];
        let mut d_temp = 0.0f32;

        if use_log_odds {
            let mut exp_0 = 0.0f32;
            for c in 0..256 {
                exp_0 += p_mix[c] * p_brain[c].max(1e-7).ln();
            }
            let e0 = (exp_0 - p_brain[y].max(1e-7).ln()).clamp(-6.0, 6.0);
            g_alpha[0] = tau * e0;
            d_temp += alpha[0] * e0;

            for k in 0..NUM_EPISODIC_EXPERTS {
                if !active[k] {
                    continue;
                }
                let ek = &experts[k];
                let mut exp_k = 0.0f32;
                for c in 0..256 {
                    exp_k += p_mix[c] * ek[c].max(1e-7).ln();
                }
                let ek_err = (exp_k - ek[y].max(1e-7).ln()).clamp(-6.0, 6.0);
                g_alpha[k + 1] = tau * ek_err;
                d_temp += alpha[k + 1] * ek_err;
            }
        } else {
            let pm_y = p_mix[y].max(1e-6);
            g_alpha[0] = (1.0 - p_brain[y] / pm_y).clamp(-6.0, 6.0);
            for k in 0..NUM_EPISODIC_EXPERTS {
                if active[k] {
                    g_alpha[k + 1] = (1.0 - experts[k][y] / pm_y).clamp(-6.0, 6.0);
                }
            }
        }

        let mut mean_g = alpha[0] * g_alpha[0];
        for k in 0..NUM_EPISODIC_EXPERTS {
            if active[k] {
                mean_g += alpha[k + 1] * g_alpha[k + 1];
            }
        }

        let th = &mut self.theta[c_idx];
        let gsq = &mut self.grad_sq_theta[c_idx];
        let lr_th = lr * 2.0;

        let gt0 = (alpha[0] * (g_alpha[0] - mean_g)).clamp(-4.0, 4.0);
        gsq[0] = (gsq[0] * 0.992 + gt0 * gt0).max(0.20);
        th[0] = (th[0] - lr_th * gt0 / gsq[0].sqrt()).clamp(-4.5, 4.5);

        for k in 0..NUM_EPISODIC_EXPERTS {
            if !active[k] {
                continue;
            }
            let idx = k + 1;
            let gtk = (alpha[idx] * (g_alpha[idx] - mean_g)).clamp(-4.0, 4.0);
            gsq[idx] = (gsq[idx] * 0.992 + gtk * gtk).max(0.20);
            th[idx] = (th[idx] - lr_th * gtk / gsq[idx].sqrt()).clamp(-4.5, 4.5);
        }

        let mut th_mean = 0.0f32;
        for &v in th.iter() {
            th_mean += v;
        }
        th_mean /= NUM_TOTAL_EXPERTS as f32;
        for v in th.iter_mut() {
            *v = (*v - th_mean).clamp(-4.5, 4.5);
        }

        if use_log_odds {
            let gt = d_temp.clamp(-4.0, 4.0);
            let gst = &mut self.grad_sq_temp[c_idx];
            *gst = (*gst * 0.992 + gt * gt).max(0.25);
            self.temp[c_idx] = (self.temp[c_idx] - (lr * 0.45) * gt / gst.sqrt()).clamp(0.55, 2.25);
        }
    }

    pub fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.updates.to_le_bytes());
        for c in 0..NUM_MIXER_CONTEXTS {
            for &v in &self.theta[c] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            for &g in &self.grad_sq_theta[c] {
                out.extend_from_slice(&g.to_le_bytes());
            }
            out.extend_from_slice(&self.temp[c].to_le_bytes());
            out.extend_from_slice(&self.grad_sq_temp[c].to_le_bytes());
        }
    }

    pub fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let need = 8 + NUM_MIXER_CONTEXTS * (NUM_TOTAL_EXPERTS * 8 + 8);
        if *pos + need > data.len() {
            return Err("meta_mixer v17 EOF".into());
        }
        let mut uarr = [0u8; 8];
        uarr.copy_from_slice(&data[*pos..*pos + 8]);
        let updates = u64::from_le_bytes(uarr);
        *pos += 8;

        let mut theta = [[0.0f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS];
        let mut grad_sq_theta = [[1.0f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS];
        let mut temp = [1.08f32; NUM_MIXER_CONTEXTS];
        let mut grad_sq_temp = [1.0f32; NUM_MIXER_CONTEXTS];

        for c in 0..NUM_MIXER_CONTEXTS {
            for slot in &mut theta[c] {
                *slot = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
                *pos += 4;
            }
            for slot in &mut grad_sq_theta[c] {
                *slot = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
                *pos += 4;
            }
            temp[c] = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
            *pos += 4;
            grad_sq_temp[c] = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
            *pos += 4;
        }
        Ok(Self {
            theta,
            temp,
            grad_sq_theta,
            grad_sq_temp,
            updates,
        })
    }
}

#[derive(Clone, Debug)]
struct ContextSlot {
    tag: u64,
    total: u32,
    last_seen: u64,
    counts: [(u8, u16); MAX_ENTRIES_PER_SLOT],
    len: u8,
}

impl ContextSlot {
    #[inline]
    fn empty() -> Self {
        Self {
            tag: 0,
            total: 0,
            last_seen: 0,
            counts: [(0, 0); MAX_ENTRIES_PER_SLOT],
            len: 0,
        }
    }

    #[inline]
    fn is_occupied(&self) -> bool {
        self.total > 0
    }

    fn reset_with(&mut self, tag: u64, byte: u8, clock: u64) {
        self.tag = tag;
        self.total = 1;
        self.last_seen = clock;
        self.counts[0] = (byte, 1);
        self.len = 1;
    }

    /// Maintains exact invariant `self.total == sum_{i=0..len} counts[i].1` (ZERO eviction drift!).
    fn observe(&mut self, byte: u8, clock: u64) {
        self.last_seen = clock;
        let n = self.len as usize;
        for i in 0..n {
            if self.counts[i].0 == byte {
                self.counts[i].1 = self.counts[i].1.saturating_add(1);
                self.total = self.total.saturating_add(1);
                if self.counts[i].1 > 4096 {
                    let mut nt = 0u32;
                    for j in 0..n {
                        self.counts[j].1 = (self.counts[j].1 / 2).max(1);
                        nt += self.counts[j].1 as u32;
                    }
                    self.total = nt;
                }
                return;
            }
        }
        if n < MAX_ENTRIES_PER_SLOT {
            self.counts[n] = (byte, 1);
            self.len += 1;
            self.total = self.total.saturating_add(1);
        } else {
            let mut min_i = 0usize;
            for i in 1..MAX_ENTRIES_PER_SLOT {
                if self.counts[i].1 < self.counts[min_i].1 {
                    min_i = i;
                }
            }
            let evicted = self.counts[min_i].1 as u32;
            self.counts[min_i] = (byte, 1);
            self.total = self.total.saturating_sub(evicted).saturating_add(1);
        }
    }

    /// Exact PPM-D / Kneser-Ney Unique-Symbol Escape Distribution backed by Adaptive Order-0 Unigram Prior `p_uni`!
    ///   `denom = C + base_esc`, `esc = (0.5 * u + base_esc) / denom`
    ///   `p(b) = max(c_b - 0.5, 0) / denom + esc * p_uni[b]`
    /// Exact probability invariant: `sum_b p(b) == ((C - 0.5*u) + (0.5*u + base_esc)) / (C + base_esc) == 1.0`!
    #[inline]
    fn fill_distribution_ppmd(&self, dist: &mut [f32; 256], p_uni: &[f32; 256], base_esc: f32) {
        let u = self.len as f32;
        let total = (self.total as f32).max(u);
        let denom = total + base_esc;
        let inv_denom = 1.0f32 / denom;
        let esc_mass = (0.5f32 * u + base_esc) * inv_denom;

        for b in 0..256 {
            dist[b] = esc_mass * p_uni[b];
        }
        let cnt_len = self.len as usize;
        for i in 0..cnt_len {
            let (b, c) = self.counts[i];
            let disc = (c as f32 - 0.5f32).max(0.0);
            dist[b as usize] += disc * inv_denom;
        }
    }
}

#[derive(Clone, Debug)]
struct LatentEntry {
    key: Vec<f32>,
    next_byte: u8,
}

#[derive(Clone, Debug)]
pub struct TropicalEpisodicMemory {
    ctw_table: Vec<ContextSlot>,
    occupied_slots: usize,
    // Adaptive Order-0 Byte Unigram Prior (backs PPM-D escape & MatchModel tail across text/code/binary!)
    uni_counts: [u32; 256],
    uni_total: u64,
    latent_cap: usize,
    latent_dim: usize,
    latent_ring: Vec<LatentEntry>,
    latent_cursor: usize,
    // 4-Way Order-∞ Exact Match-Model rolling history + 4-way 4-byte hash table!
    match_hist: Vec<u8>,
    match_hash: Vec<[u32; MATCH_WAYS]>,
    pub total_observations: u64,
}

#[inline(always)]
fn fnv1a_context(order: u8, suffix: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ((order as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    for &b in suffix {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    if h == 0 { 1 } else { h }
}

#[inline(always)]
fn fnv1a_word_bigram(order: u8, prev_word: &[u8], sep: &[u8], cur_word: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ((order as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    for &b in prev_word {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h ^= 0xFF;
    h = h.wrapping_mul(0x0000_0100_0000_01B3);
    for &b in sep {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h ^= 0xFE;
    h = h.wrapping_mul(0x0000_0100_0000_01B3);
    for &b in cur_word {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    if h == 0 { 1 } else { h }
}

#[inline(always)]
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn extract_word_tags(history_tail: &[u8]) -> (Option<u64>, Option<u64>) {
    let n = history_tail.len();
    if n < 2 {
        return (None, None);
    }

    let mut cur_start = n;
    let cur_floor = n.saturating_sub(24);
    while cur_start > cur_floor && is_word_byte(history_tail[cur_start - 1]) {
        cur_start -= 1;
    }
    let cur_word = &history_tail[cur_start..n];

    let mut sep_start = cur_start;
    let sep_floor = cur_start.saturating_sub(8);
    while sep_start > sep_floor && !is_word_byte(history_tail[sep_start - 1]) {
        sep_start -= 1;
    }
    let sep_slice = &history_tail[sep_start..cur_start];

    let mut prev_start = sep_start;
    if !sep_slice.is_empty() {
        let prev_floor = sep_start.saturating_sub(24);
        while prev_start > prev_floor && is_word_byte(history_tail[prev_start - 1]) {
            prev_start -= 1;
        }
    }
    let prev_word = &history_tail[prev_start..sep_start];

    let tag_bigram = if !prev_word.is_empty() && !sep_slice.is_empty() && sep_slice.len() <= 4 {
        Some(fnv1a_word_bigram(11, prev_word, sep_slice, cur_word))
    } else {
        None
    };

    let tag_prefix = if cur_word.len() >= 3 {
        Some(fnv1a_context(12, cur_word))
    } else if prev_word.len() >= 2 && !sep_slice.is_empty() {
        Some(fnv1a_word_bigram(13, prev_word, &sep_slice[sep_slice.len() - 1..], &[]))
    } else {
        None
    };

    (tag_bigram, tag_prefix)
}

/// Strictly causal Skip-1 / Gapped context tags (`orders 14 & 15`, disjoint from CTW `1..8` and Word `11..13`):
///   - `ord = 14`: `[b_{n-4}, b_{n-3}, b_{n-2}]` (skips `b_{n-1}` — captures `a[i]`, indexed arrays, struct slots!)
///   - `ord = 15`: `[b_{n-3}, b_{n-1}]` (skips `b_{n-2}` — captures wildcard-middle patterns!)
fn extract_skip_tags(history_tail: &[u8]) -> (Option<u64>, Option<u64>) {
    let n = history_tail.len();
    let tag_skip1 = if n >= 4 {
        Some(fnv1a_context(14, &history_tail[n - 4..n - 1]))
    } else {
        None
    };
    let tag_skip_mid = if n >= 3 {
        let pair = [history_tail[n - 3], history_tail[n - 1]];
        Some(fnv1a_context(15, &pair))
    } else {
        None
    };
    (tag_skip1, tag_skip_mid)
}

#[inline(always)]
fn hash4_u32(b0: u8, b1: u8, b2: u8, b3: u8) -> usize {
    let x = u32::from_le_bytes([b0, b1, b2, b3]);
    ((x.wrapping_mul(2654435761u32)) >> 16) as usize & MATCH_HASH_MASK
}

impl TropicalEpisodicMemory {
    pub fn new(latent_dim: usize, latent_cap: usize) -> Self {
        Self {
            ctw_table: vec![ContextSlot::empty(); CTW_SLOTS],
            occupied_slots: 0,
            uni_counts: [0u32; 256],
            uni_total: 0,
            latent_cap: latent_cap.max(32),
            latent_dim: latent_dim.max(8),
            latent_ring: Vec::with_capacity(latent_cap.min(2048)),
            latent_cursor: 0,
            match_hist: Vec::with_capacity(MATCH_HIST_CAP),
            match_hash: vec![[0u32; MATCH_WAYS]; MATCH_HASH_SIZE],
            total_observations: 0,
        }
    }

    pub fn bucket_count(&self) -> usize {
        self.occupied_slots
    }
    pub fn latent_count(&self) -> usize {
        self.latent_ring.len()
    }

    /// Adaptive Order-0 Byte Unigram Prior (exact sum == 1.0, adapts online to ANY text/code/binary stream!).
    #[inline]
    pub fn unigram_prior(&self) -> [f32; 256] {
        let mut p = [0.0f32; 256];
        let alpha = 0.15f32;
        let inv = 1.0f32 / (self.uni_total as f32 + 256.0 * alpha);
        for b in 0..256 {
            p[b] = (self.uni_counts[b] as f32 + alpha) * inv;
        }
        p
    }

    fn rebuild_match_hash(&mut self) {
        self.match_hash.fill([0u32; MATCH_WAYS]);
        if self.match_hist.len() < 4 {
            return;
        }
        for end_idx in 3..self.match_hist.len() {
            let h = hash4_u32(
                self.match_hist[end_idx - 3],
                self.match_hist[end_idx - 2],
                self.match_hist[end_idx - 1],
                self.match_hist[end_idx],
            );
            let slot = &mut self.match_hash[h];
            slot[3] = slot[2];
            slot[2] = slot[1];
            slot[1] = slot[0];
            slot[0] = (end_idx + 1) as u32;
        }
    }

    #[inline]
    fn find_slot(&self, tag: u64) -> Option<&ContextSlot> {
        let base = ((tag as usize) & CTW_SET_MASK) * CTW_WAYS;
        let s0 = &self.ctw_table[base];
        if s0.total > 0 && s0.tag == tag {
            return Some(s0);
        }
        let s1 = &self.ctw_table[base + 1];
        if s1.total > 0 && s1.tag == tag {
            return Some(s1);
        }
        None
    }

    #[inline]
    fn observe_ctw_tag(&mut self, tag: u64, actual_byte: u8, clock: u64) {
        let base = ((tag as usize) & CTW_SET_MASK) * CTW_WAYS;
        if self.ctw_table[base].total > 0 && self.ctw_table[base].tag == tag {
            self.ctw_table[base].observe(actual_byte, clock);
            return;
        }
        if self.ctw_table[base + 1].total > 0 && self.ctw_table[base + 1].tag == tag {
            self.ctw_table[base + 1].observe(actual_byte, clock);
            return;
        }
        if !self.ctw_table[base].is_occupied() {
            self.ctw_table[base].reset_with(tag, actual_byte, clock);
            self.occupied_slots += 1;
        } else if !self.ctw_table[base + 1].is_occupied() {
            self.ctw_table[base + 1].reset_with(tag, actual_byte, clock);
            self.occupied_slots += 1;
        } else {
            let evict = if self.ctw_table[base].last_seen <= self.ctw_table[base + 1].last_seen {
                base
            } else {
                base + 1
            };
            self.ctw_table[evict].reset_with(tag, actual_byte, clock);
        }
    }

    fn lookup_match_model(&self, history_tail: &[u8]) -> Option<(u8, usize)> {
        let n = history_tail.len();
        let hlen = self.match_hist.len();
        if n < 4 || hlen < 5 {
            return None;
        }
        let h = hash4_u32(
            history_tail[n - 4],
            history_tail[n - 3],
            history_tail[n - 2],
            history_tail[n - 1],
        );
        let cands = self.match_hash[h];

        let mut best_len = 0usize;
        let mut best_next = 0u8;
        for &pos_p1 in &cands {
            if pos_p1 == 0 {
                continue;
            }
            let end_idx = (pos_p1 - 1) as usize;
            if end_idx + 1 >= hlen {
                continue;
            }
            let max_check = n.min(end_idx + 1).min(128);
            let mut ml = 0usize;
            while ml < max_check && self.match_hist[end_idx - ml] == history_tail[n - 1 - ml] {
                ml += 1;
            }
            if ml >= 4 && ml > best_len {
                best_len = ml;
                best_next = self.match_hist[end_idx + 1];
            }
        }
        if best_len >= 4 {
            Some((best_next, best_len))
        } else {
            None
        }
    }

    /// Single-pass O(1) prediction of ALL 12 distinct normalized episodic experts + causal `match_len`!
    ///   - `experts[0..8]` : CTW orders 1..=8 (Exact PPM-D escape backed by adaptive `p_uni`!)
    ///   - `experts[8]`    : Tropical-L1 latent recall (backed by `p_uni`!)
    ///   - `experts[9]`    : 4-Way Order-∞ Exact Match-Model (unigram-backed non-match tail!)
    ///   - `experts[10]`   : Zero-Tokenizer WordBigram & Long-Identifier Prefix Expert (PPM-D + `p_uni`!)
    ///   - `experts[11]`   : Skip-1 / Gapped Context Expert (PPM-D + `p_uni`!)
    pub fn predict_expert_distributions(
        &self,
        history_tail: &[u8],
        latent_query: &[f32],
    ) -> ([[f32; 256]; NUM_EPISODIC_EXPERTS], [bool; NUM_EPISODIC_EXPERTS], usize) {
        let p_uni = self.unigram_prior();
        let mut experts = [p_uni; NUM_EPISODIC_EXPERTS];
        let mut active = [false; NUM_EPISODIC_EXPERTS];

        // Experts 0..8: CTW orders 1..=8 with Exact PPM-D / Witten-Bell Escape backed by `p_uni`!
        let n = history_tail.len();
        let max_ord = n.min(8) as u8;
        for ord in 1u8..=max_ord {
            let k = ord as usize;
            let tag = fnv1a_context(ord, &history_tail[n - k..n]);
            if let Some(slot) = self.find_slot(tag) {
                let idx = (ord - 1) as usize;
                active[idx] = true;
                let base_esc = if ord >= 5 { 0.08f32 } else { 0.16f32 };
                slot.fill_distribution_ppmd(&mut experts[idx], &p_uni, base_esc);
            }
        }

        // Expert 8: Tropical-L1 latent similarity recall backed by `p_uni`!
        if !self.latent_ring.is_empty() && latent_query.len() >= self.latent_dim {
            let q = &latent_query[..self.latent_dim];
            let mut lat_hits = [0.0f32; 256];
            let mut total_score = 0.0f32;
            let step = (self.latent_ring.len() / 64).max(1);
            let mut i = 0usize;
            while i < self.latent_ring.len() {
                let ent = &self.latent_ring[i];
                let mut l1 = 0.0f32;
                for d in 0..self.latent_dim {
                    l1 += (q[d] - ent.key[d]).abs();
                }
                let sim = -l1 / (self.latent_dim as f32);
                let wt = (sim * 4.5).clamp(-12.0, 0.0).exp();
                total_score += wt;
                lat_hits[ent.next_byte as usize] += wt;
                i += step;
            }
            if total_score > 1e-6 {
                active[8] = true;
                let prior_mass = 2.0f32;
                let inv_d = 1.0f32 / (total_score + prior_mass);
                let dist = &mut experts[8];
                for c in 0..256 {
                    dist[c] = (prior_mass * p_uni[c] + lat_hits[c]) * inv_d;
                }
            }
        }

        // Expert 9: 4-Way Order-∞ Exact Match-Model with unigram-backed non-match tail!
        let mut match_len = 0usize;
        if let Some((pred_b, ml)) = self.lookup_match_model(history_tail) {
            match_len = ml;
            active[9] = true;
            let p_hit = (1.0f32 - 1.0f32 / (ml as f32 + 1.5)).clamp(0.55, 0.995);
            let rem = 1.0 - p_hit;
            let uni_other = (1.0 - p_uni[pred_b as usize]).max(1e-6);
            let scale = rem / uni_other;
            let dist = &mut experts[9];
            for c in 0..256 {
                dist[c] = (p_uni[c] * scale).max(1e-7);
            }
            dist[pred_b as usize] = p_hit;
        }

        // Expert 10: Zero-Tokenizer WordBigram & Long-Identifier Prefix Expert (PPM-D + `p_uni`!)
        let (tag_bg, tag_pf) = extract_word_tags(history_tail);
        let hit_bg = tag_bg.and_then(|t| self.find_slot(t));
        let hit_pf = tag_pf.and_then(|t| self.find_slot(t));
        match (hit_bg, hit_pf) {
            (Some(s_bg), Some(s_pf)) => {
                let mut d_bg = [0.0f32; 256];
                let mut d_pf = [0.0f32; 256];
                s_bg.fill_distribution_ppmd(&mut d_bg, &p_uni, 0.08);
                s_pf.fill_distribution_ppmd(&mut d_pf, &p_uni, 0.10);
                let dist = &mut experts[10];
                for c in 0..256 {
                    dist[c] = 0.62 * d_bg[c] + 0.38 * d_pf[c];
                }
                active[10] = true;
            }
            (Some(s_bg), None) => {
                s_bg.fill_distribution_ppmd(&mut experts[10], &p_uni, 0.08);
                active[10] = true;
            }
            (None, Some(s_pf)) => {
                s_pf.fill_distribution_ppmd(&mut experts[10], &p_uni, 0.10);
                active[10] = true;
            }
            (None, None) => {}
        }

        // Expert 11: Skip-1 / Gapped Context Expert (PPM-D + `p_uni`!)
        let (tag_s1, tag_sm) = extract_skip_tags(history_tail);
        let hit_s1 = tag_s1.and_then(|t| self.find_slot(t));
        let hit_sm = tag_sm.and_then(|t| self.find_slot(t));
        match (hit_s1, hit_sm) {
            (Some(s1), Some(sm)) => {
                let mut d1 = [0.0f32; 256];
                let mut dm = [0.0f32; 256];
                s1.fill_distribution_ppmd(&mut d1, &p_uni, 0.12);
                sm.fill_distribution_ppmd(&mut dm, &p_uni, 0.16);
                let dist = &mut experts[11];
                for c in 0..256 {
                    dist[c] = 0.58 * d1[c] + 0.42 * dm[c];
                }
                active[11] = true;
            }
            (Some(s1), None) => {
                s1.fill_distribution_ppmd(&mut experts[11], &p_uni, 0.12);
                active[11] = true;
            }
            (None, Some(sm)) => {
                sm.fill_distribution_ppmd(&mut experts[11], &p_uni, 0.16);
                active[11] = true;
            }
            (None, None) => {}
        }

        (experts, active, match_len)
    }

    pub fn observe(&mut self, history_tail: &[u8], latent_state: &[f32], actual_byte: u8, surprise_bits: f32) {
        self.total_observations = self.total_observations.saturating_add(1);
        let clock = self.total_observations;

        // Update Adaptive Order-0 Byte Unigram Prior (halve gently above 1M obs to stay non-stationary!)
        self.uni_counts[actual_byte as usize] = self.uni_counts[actual_byte as usize].saturating_add(1);
        self.uni_total = self.uni_total.saturating_add(1);
        if self.uni_total > 1_000_000 {
            let mut nt = 0u64;
            for c in self.uni_counts.iter_mut() {
                *c /= 2;
                nt += *c as u64;
            }
            self.uni_total = nt;
        }

        let n = history_tail.len();
        let max_ord = n.min(8) as u8;
        for ord in 1u8..=max_ord {
            let k = ord as usize;
            let tag = fnv1a_context(ord, &history_tail[n - k..n]);
            self.observe_ctw_tag(tag, actual_byte, clock);
        }

        // Observe Zero-Tokenizer WordBigram & Long-Identifier Prefix tags!
        let (tag_bg, tag_pf) = extract_word_tags(history_tail);
        if let Some(t) = tag_bg {
            self.observe_ctw_tag(t, actual_byte, clock);
        }
        if let Some(t) = tag_pf {
            self.observe_ctw_tag(t, actual_byte, clock);
        }

        // Observe Skip-1 / Gapped Context tags!
        let (tag_s1, tag_sm) = extract_skip_tags(history_tail);
        if let Some(t) = tag_s1 {
            self.observe_ctw_tag(t, actual_byte, clock);
        }
        if let Some(t) = tag_sm {
            self.observe_ctw_tag(t, actual_byte, clock);
        }

        // Update 4-Way Order-∞ Match-Model rolling window + 4-way 4-byte hash index!
        if self.match_hist.len() >= MATCH_HIST_CAP {
            self.match_hist.drain(..MATCH_HIST_CAP / 2);
            self.rebuild_match_hash();
        }
        self.match_hist.push(actual_byte);
        let hlen = self.match_hist.len();
        if hlen >= 4 {
            let end_idx = hlen - 1;
            let h = hash4_u32(
                self.match_hist[end_idx - 3],
                self.match_hist[end_idx - 2],
                self.match_hist[end_idx - 1],
                self.match_hist[end_idx],
            );
            let slot = &mut self.match_hash[h];
            slot[3] = slot[2];
            slot[2] = slot[1];
            slot[1] = slot[0];
            slot[0] = (end_idx + 1) as u32;
        }

        if surprise_bits > 2.0 && latent_state.len() >= self.latent_dim {
            let entry = LatentEntry {
                key: latent_state[..self.latent_dim].to_vec(),
                next_byte: actual_byte,
            };
            if self.latent_ring.len() < self.latent_cap {
                self.latent_ring.push(entry);
            } else {
                self.latent_ring[self.latent_cursor] = entry;
                self.latent_cursor = (self.latent_cursor + 1) % self.latent_cap;
            }
        }
    }

    pub fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.latent_cap as u32).to_le_bytes());
        out.extend_from_slice(&(self.latent_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.latent_cursor as u32).to_le_bytes());
        out.extend_from_slice(&self.total_observations.to_le_bytes());

        out.extend_from_slice(&self.uni_total.to_le_bytes());
        for &c in &self.uni_counts {
            out.extend_from_slice(&c.to_le_bytes());
        }

        out.extend_from_slice(&(self.occupied_slots as u32).to_le_bytes());
        for (idx, sl) in self.ctw_table.iter().enumerate() {
            if sl.total == 0 {
                continue;
            }
            out.extend_from_slice(&(idx as u32).to_le_bytes());
            out.extend_from_slice(&sl.tag.to_le_bytes());
            out.extend_from_slice(&sl.total.to_le_bytes());
            out.extend_from_slice(&sl.last_seen.to_le_bytes());
            out.push(sl.len);
            for i in 0..sl.len as usize {
                out.push(sl.counts[i].0);
                out.extend_from_slice(&sl.counts[i].1.to_le_bytes());
            }
        }

        out.extend_from_slice(&(self.latent_ring.len() as u32).to_le_bytes());
        for ent in &self.latent_ring {
            out.push(ent.next_byte);
            for &v in &ent.key {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }

        out.extend_from_slice(&(self.match_hist.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.match_hist);
    }

    pub fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() {
                return Err("episodic EOF u32".into());
            }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() {
                return Err("episodic EOF u64".into());
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let take_f32 = |d: &[u8], p: &mut usize| -> Result<f32, String> {
            if *p + 4 > d.len() {
                return Err("episodic EOF f32".into());
            }
            let v = f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };

        let latent_cap = take_u32(data, pos)? as usize;
        let latent_dim = take_u32(data, pos)? as usize;
        let latent_cursor = take_u32(data, pos)? as usize;
        let total_observations = take_u64(data, pos)?;

        let uni_total = take_u64(data, pos)?;
        let mut uni_counts = [0u32; 256];
        for slot in &mut uni_counts {
            *slot = take_u32(data, pos)?;
        }

        let n_occ = take_u32(data, pos)? as usize;
        let mut ctw_table = vec![ContextSlot::empty(); CTW_SLOTS];
        for _ in 0..n_occ {
            let idx = take_u32(data, pos)? as usize;
            let tag = take_u64(data, pos)?;
            let total = take_u32(data, pos)?;
            let last_seen = take_u64(data, pos)?;
            if *pos >= data.len() {
                return Err("episodic EOF len".into());
            }
            let len = data[*pos];
            *pos += 1;
            let mut counts = [(0u8, 0u16); MAX_ENTRIES_PER_SLOT];
            for i in 0..(len as usize).min(MAX_ENTRIES_PER_SLOT) {
                if *pos + 3 > data.len() {
                    return Err("episodic EOF entry".into());
                }
                let byte = data[*pos];
                let cnt = u16::from_le_bytes([data[*pos + 1], data[*pos + 2]]);
                *pos += 3;
                counts[i] = (byte, cnt);
            }
            if idx < CTW_SLOTS {
                ctw_table[idx] = ContextSlot {
                    tag,
                    total,
                    last_seen,
                    counts,
                    len,
                };
            }
        }

        let n_lat = take_u32(data, pos)? as usize;
        let mut latent_ring = Vec::with_capacity(n_lat);
        for _ in 0..n_lat {
            if *pos >= data.len() {
                return Err("episodic EOF lat byte".into());
            }
            let next_byte = data[*pos];
            *pos += 1;
            let mut key = Vec::with_capacity(latent_dim);
            for _ in 0..latent_dim {
                key.push(take_f32(data, pos)?);
            }
            latent_ring.push(LatentEntry { key, next_byte });
        }

        let mh_len = take_u32(data, pos)? as usize;
        if *pos + mh_len > data.len() {
            return Err("episodic EOF match_hist".into());
        }
        let match_hist = data[*pos..*pos + mh_len].to_vec();
        *pos += mh_len;

        let mut mem = Self {
            ctw_table,
            occupied_slots: n_occ,
            uni_counts,
            uni_total,
            latent_cap,
            latent_dim,
            latent_ring,
            latent_cursor,
            match_hist,
            match_hash: vec![[0u32; MATCH_WAYS]; MATCH_HASH_SIZE],
            total_observations,
        };
        mem.rebuild_match_hash();
        Ok(mem)
    }
}
