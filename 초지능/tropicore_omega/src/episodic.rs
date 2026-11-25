//! Non-Parametric Tropical-L1 + Multi-Order Context-Tree (CTW) Episodic Memory (`TropiCore-Omega ASI-Zero`).
//!
//! 100% Domain-Agnostic, ZERO toy priors, ZERO pretrained tokenizers:
//!   - Combines:
//!       1) Sparse Multi-Order Exact Suffix Context Memory (`orders 1..=6` over raw bytes, Solomonoff/CTW principle)
//!          for instant 1-shot adaptation to new identifiers, real files, and unseen byte patterns.
//!       2) Tropical-L1 Latent Key-Value Episodic Cache (`-||q - k_i||_1`) over the Deep Contractive Brain's
//!          latent representations for smooth cross-context recall.
//!   - Bit-exact binary serialization inside the `TOMG11` SHA-256 checkpoint!

use std::collections::HashMap;

const MAX_CONTEXT_BUCKETS: usize = 65_536;
const MAX_ENTRIES_PER_BUCKET: usize = 8;

#[derive(Clone, Debug)]
struct ContextBucket {
    counts: Vec<(u8, u16)>,
    total: u32,
}

impl ContextBucket {
    fn new() -> Self {
        Self {
            counts: Vec::new(),
            total: 0,
        }
    }

    fn observe(&mut self, byte: u8) {
        self.total = self.total.saturating_add(1);
        for (b, c) in self.counts.iter_mut() {
            if *b == byte {
                *c = c.saturating_add(1);
                if *c > 4096 {
                    // Exponential recency halving prevents saturation on infinite streams!
                    let mut nt = 0u32;
                    for (_, cv) in self.counts.iter_mut() {
                        *cv = (*cv / 2).max(1);
                        nt += *cv as u32;
                    }
                    self.total = nt;
                }
                return;
            }
        }
        if self.counts.len() < MAX_ENTRIES_PER_BUCKET {
            self.counts.push((byte, 1));
        } else {
            // Evict lowest-count byte to keep memory strictly bounded
            let mut min_i = 0usize;
            for i in 1..self.counts.len() {
                if self.counts[i].1 < self.counts[min_i].1 {
                    min_i = i;
                }
            }
            self.total = self.total.saturating_sub(self.counts[min_i].1 as u32);
            self.counts[min_i] = (byte, 1);
            self.total = self.total.saturating_add(1);
        }
    }

    fn prob_of(&self, byte: u8) -> f32 {
        if self.total == 0 {
            return 1.0 / 256.0;
        }
        let mut c_hit = 0u16;
        for &(b, c) in &self.counts {
            if b == byte {
                c_hit = c;
                break;
            }
        }
        // KT/Dirichlet smoothed estimator `(c + 0.08) / (total + 256*0.08)`
        (c_hit as f32 + 0.08) / (self.total as f32 + 20.48)
    }
}

#[derive(Clone, Debug)]
struct LatentEntry {
    key: Vec<f32>,
    next_byte: u8,
}

#[derive(Clone, Debug)]
pub struct TropicalEpisodicMemory {
    ctw_buckets: HashMap<u64, ContextBucket>,
    latent_cap: usize,
    latent_dim: usize,
    latent_ring: Vec<LatentEntry>,
    latent_cursor: usize,
    pub total_observations: u64,
}

#[inline]
fn fnv1a_context(order: u8, suffix: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ((order as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    for &b in suffix {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

impl TropicalEpisodicMemory {
    pub fn new(latent_dim: usize, latent_cap: usize) -> Self {
        Self {
            ctw_buckets: HashMap::with_capacity(4096),
            latent_cap: latent_cap.max(32),
            latent_dim: latent_dim.max(8),
            latent_ring: Vec::with_capacity(latent_cap.min(2048)),
            latent_cursor: 0,
            total_observations: 0,
        }
    }

    pub fn bucket_count(&self) -> usize {
        self.ctw_buckets.len()
    }

    pub fn latent_count(&self) -> usize {
        self.latent_ring.len()
    }

    /// Predict next-byte probability & retrieval confidence `(p_mem, confidence in [0, 1])`
    /// purely from causal past byte suffix + Tropical-L1 latent similarity (ZERO peeking at `target`!).
    pub fn predict_byte_prob(&self, history_tail: &[u8], latent_query: &[f32], target: u8) -> (f32, f32) {
        let mut weighted_p = 0.0f32;
        let mut w_sum = 0.0f32;
        let mut max_order_hit = 0u8;

        // 1. Multi-order exact suffix CTW mixture (`orders 1..=6`, higher orders weighted exponentially stronger)
        let n = history_tail.len();
        for ord in 1u8..=6u8 {
            let k = ord as usize;
            if n < k {
                break;
            }
            let key = fnv1a_context(ord, &history_tail[n - k..n]);
            if let Some(bucket) = self.ctw_buckets.get(&key) {
                if bucket.total > 0 {
                    let p = bucket.prob_of(target);
                    let w = (1.85f32).powi(ord as i32) * (bucket.total as f32).ln_1p();
                    weighted_p += w * p;
                    w_sum += w;
                    max_order_hit = ord;
                }
            }
        }

        // 2. Tropical-L1 latent similarity recall over recent surprising neural states (sample-bounded for O(1) speed)
        if !self.latent_ring.is_empty() && latent_query.len() >= self.latent_dim {
            let q = &latent_query[..self.latent_dim];
            let mut best_sim = f32::NEG_INFINITY;
            let mut hit_score = 0.0f32;
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
                if sim > best_sim {
                    best_sim = sim;
                }
                let wt = (sim * 4.5).clamp(-12.0, 0.0).exp();
                total_score += wt;
                if ent.next_byte == target {
                    hit_score += wt;
                }
                i += step;
            }
            if total_score > 1e-6 {
                let p_lat = (hit_score + 0.02) / (total_score + 5.12);
                let w_lat = 1.2 * (best_sim * 3.0).clamp(-6.0, 0.0).exp();
                weighted_p += w_lat * p_lat;
                w_sum += w_lat;
            }
        }

        if w_sum <= 1e-6 {
            return (1.0 / 256.0, 0.0);
        }
        let p_mem = (weighted_p / w_sum).clamp(1e-6, 0.9999);
        let conf = ((max_order_hit as f32) / 6.0 * (w_sum / (w_sum + 3.0))).clamp(0.0, 1.0);
        (p_mem, conf)
    }

    /// Observe causal `(history_tail, latent_state, actual_byte)` AFTER prediction (strict online causality!).
    pub fn observe(&mut self, history_tail: &[u8], latent_state: &[f32], actual_byte: u8, surprise_bits: f32) {
        self.total_observations = self.total_observations.saturating_add(1);
        let n = history_tail.len();
        for ord in 1u8..=6u8 {
            let k = ord as usize;
            if n < k {
                break;
            }
            let key = fnv1a_context(ord, &history_tail[n - k..n]);
            if let Some(b) = self.ctw_buckets.get_mut(&key) {
                b.observe(actual_byte);
            } else if self.ctw_buckets.len() < MAX_CONTEXT_BUCKETS {
                let mut b = ContextBucket::new();
                b.observe(actual_byte);
                self.ctw_buckets.insert(key, b);
            }
        }

        // Only store high-surprise states in Tropical-L1 latent ring (information-theoretic memory filtering!)
        if surprise_bits > 2.2 && latent_state.len() >= self.latent_dim {
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

        // Deterministic sorted order so checkpoint SHA-256 is 100% reproducible!
        let mut keys: Vec<u64> = self.ctw_buckets.keys().copied().collect();
        keys.sort_unstable();
        out.extend_from_slice(&(keys.len() as u32).to_le_bytes());
        for k in keys {
            let b = &self.ctw_buckets[&k];
            out.extend_from_slice(&k.to_le_bytes());
            out.extend_from_slice(&b.total.to_le_bytes());
            out.push(b.counts.len() as u8);
            for &(byte, cnt) in &b.counts {
                out.push(byte);
                out.extend_from_slice(&cnt.to_le_bytes());
            }
        }

        out.extend_from_slice(&(self.latent_ring.len() as u32).to_le_bytes());
        for ent in &self.latent_ring {
            out.push(ent.next_byte);
            for &v in &ent.key {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
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

        let n_buckets = take_u32(data, pos)? as usize;
        let mut ctw_buckets = HashMap::with_capacity(n_buckets);
        for _ in 0..n_buckets {
            let k = take_u64(data, pos)?;
            let total = take_u32(data, pos)?;
            if *pos >= data.len() { return Err("episodic EOF bucket len".into()); }
            let n_cnt = data[*pos] as usize;
            *pos += 1;
            let mut counts = Vec::with_capacity(n_cnt);
            for _ in 0..n_cnt {
                if *pos + 3 > data.len() { return Err("episodic EOF count".into()); }
                let byte = data[*pos];
                let cnt = u16::from_le_bytes([data[*pos + 1], data[*pos + 2]]);
                *pos += 3;
                counts.push((byte, cnt));
            }
            ctw_buckets.insert(k, ContextBucket { counts, total });
        }

        let n_lat = take_u32(data, pos)? as usize;
        let mut latent_ring = Vec::with_capacity(n_lat);
        for _ in 0..n_lat {
            if *pos >= data.len() { return Err("episodic EOF lat byte".into()); }
            let next_byte = data[*pos];
            *pos += 1;
            let mut key = Vec::with_capacity(latent_dim);
            for _ in 0..latent_dim {
                key.push(take_f32(data, pos)?);
            }
            latent_ring.push(LatentEntry { key, next_byte });
        }

        Ok(Self {
            ctw_buckets,
            latent_cap,
            latent_dim,
            latent_ring,
            latent_cursor,
            total_observations,
        })
    }
}
