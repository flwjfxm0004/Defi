//! Non-Parametric 2-Way LRU Context-Tree (CTW Ord 1..=8) + Tropical-L1 + Order-∞ Exact Match-Model
//! + 8-Context-Bucket Online Convex Logistic Meta-Mixer (`TropiCore-Omega v15.0`, PAQ/CMIX/Hedge Principle!).
//!
//! Total 11 Experts unified in an 8-context-bucket convex log-opinion meta-mixer:
//!   - Expert 0    : Deep SwiGLU + Conv-4 + Fast-Weight Associative Delta-SSM Neural Brain
//!   - Experts 1..8: 2-Way Set-Associative LRU Exact Suffix CTW (`orders 1..=8`, NEVER freezes!)
//!   - Expert 9    : Tropical-L1 Latent Key-Value Recall
//!   - Expert 10   : Order-∞ Exact Match-Model (`O(1)` 2-way 4-byte hash chain over 64 KB rolling window,
//!                   detects 4..128 byte exact copies across arbitrary unseen files/code/binaries!)

pub const NUM_EPISODIC_EXPERTS: usize = 10; // 0..8 = CTW Ord 1..8, 8 = Tropical-L1, 9 = Order-∞ Match-Model
pub const NUM_TOTAL_EXPERTS: usize = 11;    // 0 = Neural Brain, 1..=10 = Episodic Experts
pub const NUM_MIXER_CONTEXTS: usize = 8;

const CTW_NUM_SETS: usize = 32_768;
const CTW_SET_MASK: usize = CTW_NUM_SETS - 1;
const CTW_WAYS: usize = 2;
const CTW_SLOTS: usize = CTW_NUM_SETS * CTW_WAYS;
const MAX_ENTRIES_PER_SLOT: usize = 8;

const MATCH_HIST_CAP: usize = 65_536;
const MATCH_HASH_SIZE: usize = 65_536;
const MATCH_HASH_MASK: usize = MATCH_HASH_SIZE - 1;

/// Strictly causal context classifier (`0..8`) driven ONLY by previous observed byte & causal match length!
#[inline]
pub fn mixer_context_bucket(prev_byte: u8, match_len: usize) -> usize {
    if match_len >= 12 {
        return 7; // Long exact copy regime (Order-∞ Match-Model dominates!)
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

/// 8-Context-Bucket × 11-Expert Online Convex Logistic Meta-Mixer (`O(sqrt(T))` Regret Guarantee per Context!).
#[derive(Clone, Debug)]
pub struct OnlineLogisticMetaMixer {
    pub weights: [[f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
    pub grad_sq: [[f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
    pub updates: u64,
}

impl OnlineLogisticMetaMixer {
    pub fn new() -> Self {
        let base_w = [1.00f32, 0.14, 0.24, 0.36, 0.50, 0.65, 0.80, 0.95, 1.08, 0.25, 1.35];
        let mut weights = [base_w; NUM_MIXER_CONTEXTS];
        // Context 6 (medium match) & Context 7 (long match) start with stronger Match-Model prior
        weights[6][10] = 1.65;
        weights[7][10] = 2.15;
        Self {
            weights,
            grad_sq: [[1.0; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS],
            updates: 0,
        }
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
        for &a in active { if a { any_active = true; break; } }
        if !any_active { return *p_brain; }

        let c_idx = ctx.min(NUM_MIXER_CONTEXTS - 1);
        let cw = &self.weights[c_idx];

        let mut ep_w_sum = 0.0f32;
        for k in 0..NUM_EPISODIC_EXPERTS {
            if active[k] { ep_w_sum += cw[k + 1].max(0.0); }
        }
        // Allow higher episodic mass when Match-Model (Expert 9) is active on a long exact copy!
        let eff_cap = if active[9] { w_mem_cap.max(0.94) } else { w_mem_cap };
        let max_ep_mass = (eff_cap / (1.0 - eff_cap).max(0.04)).min(14.0) * cw[0].max(0.15);
        let ep_gain = if ep_w_sum > 1e-6 {
            let scaled = ep_w_sum * w_mem_scale;
            if scaled > max_ep_mass { max_ep_mass / ep_w_sum } else { w_mem_scale }
        } else {
            0.0
        };

        let mut out = [0.0f32; 256];
        let wb = cw[0].max(0.15);
        if use_log_odds {
            let mut lbuf = [0.0f32; 256];
            for c in 0..256 { lbuf[c] = wb * p_brain[c].max(1e-7).ln(); }
            for k in 0..NUM_EPISODIC_EXPERTS {
                if active[k] {
                    let wk = cw[k + 1] * ep_gain;
                    if wk > 1e-5 {
                        let ek = &experts[k];
                        for c in 0..256 { lbuf[c] += wk * ek[c].max(1e-7).ln(); }
                    }
                }
            }
            let mut mx = f32::NEG_INFINITY;
            for &v in &lbuf { if v > mx { mx = v; } }
            let mut sum = 0.0f32;
            for c in 0..256 {
                let v = (lbuf[c] - mx).max(-40.0).exp().max(1e-7);
                out[c] = v;
                sum += v;
            }
            let inv = 1.0 / sum.max(1e-12);
            for c in 0..256 { out[c] *= inv; }
        } else {
            for c in 0..256 { out[c] = wb * p_brain[c]; }
            for k in 0..NUM_EPISODIC_EXPERTS {
                if active[k] {
                    let wk = cw[k + 1] * ep_gain;
                    if wk > 1e-5 {
                        let ek = &experts[k];
                        for c in 0..256 { out[c] += wk * ek[c]; }
                    }
                }
            }
            let mut sum = 0.0f32;
            for c in 0..256 { out[c] = out[c].max(1e-7); sum += out[c]; }
            let inv = 1.0 / sum.max(1e-12);
            for c in 0..256 { out[c] *= inv; }
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
        use_log_odds: bool,
        lr: f32,
    ) {
        self.updates = self.updates.saturating_add(1);
        let c_idx = ctx.min(NUM_MIXER_CONTEXTS - 1);
        let cw = &mut self.weights[c_idx];
        let cg = &mut self.grad_sq[c_idx];
        let y = actual_byte as usize;

        if use_log_odds {
            let mut exp_b = 0.0f32;
            for c in 0..256 { exp_b += p_mix[c] * p_brain[c].max(1e-7).ln(); }
            let g0 = (exp_b - p_brain[y].max(1e-7).ln()).clamp(-4.0, 4.0);
            cg[0] = (cg[0] * 0.992 + g0 * g0).max(0.25);
            cw[0] = (cw[0] - lr * g0 / cg[0].sqrt()).clamp(0.18, 3.0);

            for k in 0..NUM_EPISODIC_EXPERTS {
                if !active[k] { continue; }
                let ek = &experts[k];
                let mut exp_k = 0.0f32;
                for c in 0..256 { exp_k += p_mix[c] * ek[c].max(1e-7).ln(); }
                let gk = (exp_k - ek[y].max(1e-7).ln()).clamp(-4.0, 4.0);
                let idx = k + 1;
                cg[idx] = (cg[idx] * 0.992 + gk * gk).max(0.25);
                cw[idx] = (cw[idx] - lr * gk / cg[idx].sqrt()).clamp(0.0, 3.5);
            }
        } else {
            let pm_y = p_mix[y].max(1e-6);
            let g0 = (1.0 - p_brain[y] / pm_y).clamp(-4.0, 4.0);
            cg[0] = (cg[0] * 0.992 + g0 * g0).max(0.25);
            cw[0] = (cw[0] - lr * g0 / cg[0].sqrt()).clamp(0.18, 3.0);
            for k in 0..NUM_EPISODIC_EXPERTS {
                if !active[k] { continue; }
                let gk = (1.0 - experts[k][y] / pm_y).clamp(-4.0, 4.0);
                let idx = k + 1;
                cg[idx] = (cg[idx] * 0.992 + gk * gk).max(0.25);
                cw[idx] = (cw[idx] - lr * gk / cg[idx].sqrt()).clamp(0.0, 3.5);
            }
        }
    }

    pub fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.updates.to_le_bytes());
        for c in 0..NUM_MIXER_CONTEXTS {
            for &w in &self.weights[c] { out.extend_from_slice(&w.to_le_bytes()); }
            for &g in &self.grad_sq[c] { out.extend_from_slice(&g.to_le_bytes()); }
        }
    }

    pub fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let need = 8 + NUM_MIXER_CONTEXTS * NUM_TOTAL_EXPERTS * 8;
        if *pos + need > data.len() { return Err("meta_mixer EOF".into()); }
        let mut uarr = [0u8; 8];
        uarr.copy_from_slice(&data[*pos..*pos + 8]);
        let updates = u64::from_le_bytes(uarr);
        *pos += 8;
        let mut weights = [[0.0f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS];
        let mut grad_sq = [[1.0f32; NUM_TOTAL_EXPERTS]; NUM_MIXER_CONTEXTS];
        for c in 0..NUM_MIXER_CONTEXTS {
            for slot in &mut weights[c] {
                *slot = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
                *pos += 4;
            }
            for slot in &mut grad_sq[c] {
                *slot = f32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
                *pos += 4;
            }
        }
        Ok(Self { weights, grad_sq, updates })
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
        Self { tag: 0, total: 0, last_seen: 0, counts: [(0, 0); MAX_ENTRIES_PER_SLOT], len: 0 }
    }

    #[inline]
    fn is_occupied(&self) -> bool { self.total > 0 }

    fn reset_with(&mut self, tag: u64, byte: u8, clock: u64) {
        self.tag = tag;
        self.total = 1;
        self.last_seen = clock;
        self.counts[0] = (byte, 1);
        self.len = 1;
    }

    fn observe(&mut self, byte: u8, clock: u64) {
        self.last_seen = clock;
        self.total = self.total.saturating_add(1);
        let n = self.len as usize;
        for i in 0..n {
            if self.counts[i].0 == byte {
                self.counts[i].1 = self.counts[i].1.saturating_add(1);
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
        } else {
            let mut min_i = 0usize;
            for i in 1..MAX_ENTRIES_PER_SLOT {
                if self.counts[i].1 < self.counts[min_i].1 { min_i = i; }
            }
            self.total = self.total.saturating_sub(self.counts[min_i].1 as u32);
            self.counts[min_i] = (byte, 1);
            self.total = self.total.saturating_add(1);
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
    latent_cap: usize,
    latent_dim: usize,
    latent_ring: Vec<LatentEntry>,
    latent_cursor: usize,
    // Order-∞ Exact Match-Model rolling history + 2-way 4-byte hash table!
    match_hist: Vec<u8>,
    match_hash: Vec<[u32; 2]>,
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
fn hash4_u32(b0: u8, b1: u8, b2: u8, b3: u8) -> usize {
    let x = u32::from_le_bytes([b0, b1, b2, b3]);
    ((x.wrapping_mul(2654435761u32)) >> 16) as usize & MATCH_HASH_MASK
}

impl TropicalEpisodicMemory {
    pub fn new(latent_dim: usize, latent_cap: usize) -> Self {
        Self {
            ctw_table: vec![ContextSlot::empty(); CTW_SLOTS],
            occupied_slots: 0,
            latent_cap: latent_cap.max(32),
            latent_dim: latent_dim.max(8),
            latent_ring: Vec::with_capacity(latent_cap.min(2048)),
            latent_cursor: 0,
            match_hist: Vec::with_capacity(MATCH_HIST_CAP),
            match_hash: vec![[0u32; 2]; MATCH_HASH_SIZE],
            total_observations: 0,
        }
    }

    pub fn bucket_count(&self) -> usize { self.occupied_slots }
    pub fn latent_count(&self) -> usize { self.latent_ring.len() }

    fn rebuild_match_hash(&mut self) {
        self.match_hash.fill([0u32; 2]);
        if self.match_hist.len() < 4 { return; }
        for end_idx in 3..self.match_hist.len() {
            let h = hash4_u32(
                self.match_hist[end_idx - 3],
                self.match_hist[end_idx - 2],
                self.match_hist[end_idx - 1],
                self.match_hist[end_idx],
            );
            let slot = &mut self.match_hash[h];
            slot[1] = slot[0];
            slot[0] = (end_idx + 1) as u32;
        }
    }

    #[inline]
    fn find_slot(&self, tag: u64) -> Option<&ContextSlot> {
        let base = ((tag as usize) & CTW_SET_MASK) * CTW_WAYS;
        let s0 = &self.ctw_table[base];
        if s0.total > 0 && s0.tag == tag { return Some(s0); }
        let s1 = &self.ctw_table[base + 1];
        if s1.total > 0 && s1.tag == tag { return Some(s1); }
        None
    }

    /// Order-∞ Exact Match-Model lookup: returns `(predicted_next_byte, match_len)` if a >=4-byte exact suffix match exists!
    fn lookup_match_model(&self, history_tail: &[u8]) -> Option<(u8, usize)> {
        let n = history_tail.len();
        let hlen = self.match_hist.len();
        if n < 4 || hlen < 5 { return None; }
        let h = hash4_u32(history_tail[n - 4], history_tail[n - 3], history_tail[n - 2], history_tail[n - 1]);
        let cands = self.match_hash[h];

        let mut best_len = 0usize;
        let mut best_next = 0u8;
        for &pos_p1 in &cands {
            if pos_p1 == 0 { continue; }
            let end_idx = (pos_p1 - 1) as usize;
            // Must have a known next byte strictly in the past!
            if end_idx + 1 >= hlen { continue; }
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
        if best_len >= 4 { Some((best_next, best_len)) } else { None }
    }

    /// Single-pass O(1) prediction of ALL 10 distinct normalized episodic experts + causal `match_len`!
    ///   - `experts[0..8]` : CTW orders 1..=8
    ///   - `experts[8]`    : Tropical-L1 latent recall
    ///   - `experts[9]`    : Order-∞ Exact Match-Model!
    pub fn predict_expert_distributions(
        &self,
        history_tail: &[u8],
        latent_query: &[f32],
    ) -> ([[f32; 256]; NUM_EPISODIC_EXPERTS], [bool; NUM_EPISODIC_EXPERTS], usize) {
        let mut experts = [[1.0f32 / 256.0; 256]; NUM_EPISODIC_EXPERTS];
        let mut active = [false; NUM_EPISODIC_EXPERTS];

        // Experts 0..8: CTW orders 1..=8
        let n = history_tail.len();
        let max_ord = n.min(8) as u8;
        for ord in 1u8..=max_ord {
            let k = ord as usize;
            let tag = fnv1a_context(ord, &history_tail[n - k..n]);
            if let Some(slot) = self.find_slot(tag) {
                let idx = (ord - 1) as usize;
                active[idx] = true;
                let alpha = if ord >= 5 { 0.04f32 } else { 0.08f32 };
                let inv_denom = 1.0f32 / (slot.total as f32 + 256.0 * alpha);
                let base_p = alpha * inv_denom;
                let dist = &mut experts[idx];
                dist.fill(base_p);
                let cnt_len = slot.len as usize;
                for i in 0..cnt_len {
                    let (b, c) = slot.counts[i];
                    dist[b as usize] += c as f32 * inv_denom;
                }
            }
        }

        // Expert 8: Tropical-L1 latent similarity recall
        if !self.latent_ring.is_empty() && latent_query.len() >= self.latent_dim {
            let q = &latent_query[..self.latent_dim];
            let mut lat_hits = [0.0f32; 256];
            let mut total_score = 0.0f32;
            let step = (self.latent_ring.len() / 64).max(1);
            let mut i = 0usize;
            while i < self.latent_ring.len() {
                let ent = &self.latent_ring[i];
                let mut l1 = 0.0f32;
                for d in 0..self.latent_dim { l1 += (q[d] - ent.key[d]).abs(); }
                let sim = -l1 / (self.latent_dim as f32);
                let wt = (sim * 4.5).clamp(-12.0, 0.0).exp();
                total_score += wt;
                lat_hits[ent.next_byte as usize] += wt;
                i += step;
            }
            if total_score > 1e-6 {
                active[8] = true;
                let inv_d = 1.0f32 / (total_score + 5.12);
                let base_p = 0.02 * inv_d;
                let dist = &mut experts[8];
                for c in 0..256 { dist[c] = base_p + lat_hits[c] * inv_d; }
            }
        }

        // Expert 9: Order-∞ Exact Match-Model!
        let mut match_len = 0usize;
        if let Some((pred_b, ml)) = self.lookup_match_model(history_tail) {
            match_len = ml;
            active[9] = true;
            let p_hit = (1.0f32 - 1.0f32 / (ml as f32 + 1.5)).clamp(0.55, 0.992);
            let p_other = (1.0 - p_hit) / 255.0;
            let dist = &mut experts[9];
            dist.fill(p_other);
            dist[pred_b as usize] = p_hit;
        }

        (experts, active, match_len)
    }

    pub fn observe(&mut self, history_tail: &[u8], latent_state: &[f32], actual_byte: u8, surprise_bits: f32) {
        self.total_observations = self.total_observations.saturating_add(1);
        let clock = self.total_observations;
        let n = history_tail.len();
        let max_ord = n.min(8) as u8;
        for ord in 1u8..=max_ord {
            let k = ord as usize;
            let tag = fnv1a_context(ord, &history_tail[n - k..n]);
            let base = ((tag as usize) & CTW_SET_MASK) * CTW_WAYS;

            if self.ctw_table[base].total > 0 && self.ctw_table[base].tag == tag {
                self.ctw_table[base].observe(actual_byte, clock);
                continue;
            }
            if self.ctw_table[base + 1].total > 0 && self.ctw_table[base + 1].tag == tag {
                self.ctw_table[base + 1].observe(actual_byte, clock);
                continue;
            }
            if !self.ctw_table[base].is_occupied() {
                self.ctw_table[base].reset_with(tag, actual_byte, clock);
                self.occupied_slots += 1;
            } else if !self.ctw_table[base + 1].is_occupied() {
                self.ctw_table[base + 1].reset_with(tag, actual_byte, clock);
                self.occupied_slots += 1;
            } else {
                let evict = if self.ctw_table[base].last_seen <= self.ctw_table[base + 1].last_seen { base } else { base + 1 };
                self.ctw_table[evict].reset_with(tag, actual_byte, clock);
            }
        }

        // Update Order-∞ Match-Model rolling window + 2-way 4-byte hash index!
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

        out.extend_from_slice(&(self.occupied_slots as u32).to_le_bytes());
        for (idx, sl) in self.ctw_table.iter().enumerate() {
            if sl.total == 0 { continue; }
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
            for &v in &ent.key { out.extend_from_slice(&v.to_le_bytes()); }
        }

        out.extend_from_slice(&(self.match_hist.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.match_hist);
    }

    pub fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("episodic EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() { return Err("episodic EOF u64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let take_f32 = |d: &[u8], p: &mut usize| -> Result<f32, String> {
            if *p + 4 > d.len() { return Err("episodic EOF f32".into()); }
            let v = f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };

        let latent_cap = take_u32(data, pos)? as usize;
        let latent_dim = take_u32(data, pos)? as usize;
        let latent_cursor = take_u32(data, pos)? as usize;
        let total_observations = take_u64(data, pos)?;

        let n_occ = take_u32(data, pos)? as usize;
        let mut ctw_table = vec![ContextSlot::empty(); CTW_SLOTS];
        for _ in 0..n_occ {
            let idx = take_u32(data, pos)? as usize;
            let tag = take_u64(data, pos)?;
            let total = take_u32(data, pos)?;
            let last_seen = take_u64(data, pos)?;
            if *pos >= data.len() { return Err("episodic EOF len".into()); }
            let len = data[*pos];
            *pos += 1;
            let mut counts = [(0u8, 0u16); MAX_ENTRIES_PER_SLOT];
            for i in 0..(len as usize).min(MAX_ENTRIES_PER_SLOT) {
                if *pos + 3 > data.len() { return Err("episodic EOF entry".into()); }
                let byte = data[*pos];
                let cnt = u16::from_le_bytes([data[*pos + 1], data[*pos + 2]]);
                *pos += 3;
                counts[i] = (byte, cnt);
            }
            if idx < CTW_SLOTS {
                ctw_table[idx] = ContextSlot { tag, total, last_seen, counts, len };
            }
        }

        let n_lat = take_u32(data, pos)? as usize;
        let mut latent_ring = Vec::with_capacity(n_lat);
        for _ in 0..n_lat {
            if *pos >= data.len() { return Err("episodic EOF lat byte".into()); }
            let next_byte = data[*pos];
            *pos += 1;
            let mut key = Vec::with_capacity(latent_dim);
            for _ in 0..latent_dim { key.push(take_f32(data, pos)?); }
            latent_ring.push(LatentEntry { key, next_byte });
        }

        let mh_len = take_u32(data, pos)? as usize;
        if *pos + mh_len > data.len() { return Err("episodic EOF match_hist".into()); }
        let match_hist = data[*pos..*pos + mh_len].to_vec();
        *pos += mh_len;

        let mut mem = Self {
            ctw_table,
            occupied_slots: n_occ,
            latent_cap,
            latent_dim,
            latent_ring,
            latent_cursor,
            match_hist,
            match_hash: vec![[0u32; 2]; MATCH_HASH_SIZE],
            total_observations,
        };
        mem.rebuild_match_hash();
        Ok(mem)
    }
}
