//! Pure-Rust Zero-Dependency Exact 32-Bit Arithmetic Bitstream Codec (`OMGZ15`)
//! Powered by 11-Expert × 8-Context Online Convex Meta-Mixer (Order-∞ Match-Model + CTW 1..8 + Tropical-L1
//! + Deep SwiGLU Conv4 Delta-SSM Brain) + Deterministic 64-Byte Test-Time Neural Adaptation!

use std::fs;
use std::path::Path;

use crate::brain::{CausalStreamState, DeepContractiveBrain};
use crate::episodic::NUM_EPISODIC_EXPERTS;
use crate::sha256::sha256_hex;

const OMG_MAGIC: &[u8; 6] = b"OMGZ15";
const FULL_RANGE: u64 = 0xFFFF_FFFF;
const HALF: u64 = 0x8000_0000;
const QUARTER: u64 = 0x4000_0000;
const THREE_QUARTERS: u64 = 0xC000_0000;
const TOTAL_FREQ: u64 = 65_536;
const TTT_BLOCK: usize = 64;
const TTT_LR_SCALE: f32 = 0.55;

pub fn probs_to_cdf(probs: &[f32; 256]) -> [u64; 257] {
    let mut freqs = [1u64; 256];
    let budget = TOTAL_FREQ - 256;
    let mut sum_p = 0.0f64;
    for &p in probs {
        sum_p += if p.is_finite() && p > 0.0 { p as f64 } else { 0.0 };
    }
    if sum_p <= 1e-12 { sum_p = 1.0; }
    let mut assigned = 0u64;
    let mut max_i = 0usize;
    let mut max_f = 0u64;
    for i in 0..256 {
        let p = if probs[i].is_finite() && probs[i] > 0.0 { probs[i] as f64 / sum_p } else { 0.0 };
        let add = (p * budget as f64).floor() as u64;
        freqs[i] += add;
        assigned += add;
        if freqs[i] > max_f { max_f = freqs[i]; max_i = i; }
    }
    if assigned < budget { freqs[max_i] += budget - assigned; }
    let mut cdf = [0u64; 257];
    for i in 0..256 { cdf[i + 1] = cdf[i] + freqs[i]; }
    cdf
}

pub struct ArithmeticEncoder {
    low: u64,
    high: u64,
    pending: usize,
    cur_byte: u8,
    bit_pos: u8,
    out: Vec<u8>,
}

impl ArithmeticEncoder {
    pub fn new(capacity_hint: usize) -> Self {
        Self { low: 0, high: FULL_RANGE, pending: 0, cur_byte: 0, bit_pos: 0, out: Vec::with_capacity(capacity_hint) }
    }

    #[inline]
    fn write_bit(&mut self, bit: u8) {
        self.cur_byte = (self.cur_byte << 1) | (bit & 1);
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.out.push(self.cur_byte);
            self.cur_byte = 0;
            self.bit_pos = 0;
        }
    }

    #[inline]
    fn emit_bit_and_pending(&mut self, bit: u8) {
        self.write_bit(bit);
        let opp = bit ^ 1;
        while self.pending > 0 {
            self.write_bit(opp);
            self.pending -= 1;
        }
    }

    pub fn encode_symbol(&mut self, sym: u8, cdf: &[u64; 257]) {
        let s = sym as usize;
        let range = self.high - self.low + 1;
        self.high = self.low + (range * cdf[s + 1]) / TOTAL_FREQ - 1;
        self.low = self.low + (range * cdf[s]) / TOTAL_FREQ;

        loop {
            if self.high < HALF {
                self.emit_bit_and_pending(0);
            } else if self.low >= HALF {
                self.emit_bit_and_pending(1);
                self.low -= HALF;
                self.high -= HALF;
            } else if self.low >= QUARTER && self.high < THREE_QUARTERS {
                self.pending += 1;
                self.low -= QUARTER;
                self.high -= QUARTER;
            } else {
                break;
            }
            self.low = (self.low << 1) & FULL_RANGE;
            self.high = ((self.high << 1) | 1) & FULL_RANGE;
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.pending += 1;
        if self.low < QUARTER { self.emit_bit_and_pending(0); } else { self.emit_bit_and_pending(1); }
        if self.bit_pos > 0 {
            self.cur_byte <<= 8 - self.bit_pos;
            self.out.push(self.cur_byte);
        }
        self.out
    }
}

pub struct ArithmeticDecoder<'a> {
    data: &'a [u8],
    byte_idx: usize,
    bit_pos: u8,
    low: u64,
    high: u64,
    value: u64,
}

impl<'a> ArithmeticDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut dec = Self { data, byte_idx: 0, bit_pos: 0, low: 0, high: FULL_RANGE, value: 0 };
        for _ in 0..32 { dec.value = (dec.value << 1) | (dec.read_bit() as u64); }
        dec
    }

    #[inline]
    fn read_bit(&mut self) -> u8 {
        if self.byte_idx >= self.data.len() { return 0; }
        let b = (self.data[self.byte_idx] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 { self.bit_pos = 0; self.byte_idx += 1; }
        b
    }

    pub fn decode_symbol(&mut self, cdf: &[u64; 257]) -> u8 {
        let range = self.high - self.low + 1;
        let scaled = ((self.value - self.low + 1) * TOTAL_FREQ - 1) / range;

        let mut lo = 0usize;
        let mut hi = 256usize;
        while lo + 1 < hi {
            let mid = (lo + hi) >> 1;
            if cdf[mid] <= scaled { lo = mid; } else { hi = mid; }
        }
        let s = lo;
        self.high = self.low + (range * cdf[s + 1]) / TOTAL_FREQ - 1;
        self.low = self.low + (range * cdf[s]) / TOTAL_FREQ;

        loop {
            if self.high < HALF {
                // no shift subtract
            } else if self.low >= HALF {
                self.value -= HALF;
                self.low -= HALF;
                self.high -= HALF;
            } else if self.low >= QUARTER && self.high < THREE_QUARTERS {
                self.value -= QUARTER;
                self.low -= QUARTER;
                self.high -= QUARTER;
            } else {
                break;
            }
            self.low = (self.low << 1) & FULL_RANGE;
            self.high = ((self.high << 1) | 1) & FULL_RANGE;
            self.value = ((self.value << 1) | (self.read_bit() as u64)) & FULL_RANGE;
        }
        s as u8
    }
}

#[derive(Clone, Debug)]
pub struct PhysicalCompressReport {
    pub orig_bytes: usize,
    pub compressed_bytes: usize,
    pub physical_bpb: f64,
    pub physical_ratio: f64,
    pub ttt_updates: usize,
    pub orig_sha256: String,
}

pub fn compress_bytes_with_brain(
    brain: &DeepContractiveBrain,
    raw: &[u8],
    enable_ttt: bool,
) -> (Vec<u8>, PhysicalCompressReport) {
    let orig_sha = sha256_hex(raw);
    let mut work_brain = brain.clone();
    let mut stream = CausalStreamState::new(&work_brain.cfg);
    let mut ttt_stream = CausalStreamState::new(&work_brain.cfg);
    let mut enc = ArithmeticEncoder::new(raw.len() / 2 + 64);

    let use_lo = work_brain.active_policy.use_log_odds_mix;
    let mut blk_in = Vec::with_capacity(TTT_BLOCK);
    let mut blk_tgt = Vec::with_capacity(TTT_BLOCK);
    let mut ttt_updates = 0usize;
    let mut prev_byte = 0u8;

    for (idx, &actual) in raw.iter().enumerate() {
        let (p_mix, p_brain, experts, active, ctx, lat) = if idx == 0 {
            let unif = [1.0f32 / 256.0; 256];
            (unif, unif, [unif; NUM_EPISODIC_EXPERTS], [false; NUM_EPISODIC_EXPERTS], 0usize, vec![0.0f32; work_brain.cfg.d_model])
        } else {
            work_brain.predict_next_byte_full(prev_byte, &mut stream, None)
        };
        let cdf = probs_to_cdf(&p_mix);
        enc.encode_symbol(actual, &cdf);

        if idx > 0 {
            let surprise = -(p_mix[actual as usize].max(1e-7).log2());
            work_brain.meta_mixer.observe_online(ctx, &p_brain, &experts, &active, &p_mix, actual, use_lo, 0.16);
            work_brain.episodic.observe(&stream.recent_bytes, &lat, actual, surprise);
            if enable_ttt {
                blk_in.push(prev_byte);
                blk_tgt.push(actual);
                if blk_in.len() == TTT_BLOCK {
                    work_brain.ttt_adapt_block(&blk_in, &blk_tgt, &mut ttt_stream, TTT_LR_SCALE);
                    blk_in.clear();
                    blk_tgt.clear();
                    ttt_updates += 1;
                }
            }
        }
        prev_byte = actual;
    }

    let payload = enc.finish();
    let mut archive = Vec::with_capacity(6 + 1 + 8 + 64 + payload.len());
    archive.extend_from_slice(OMG_MAGIC);
    archive.push(if enable_ttt { 1u8 } else { 0u8 });
    archive.extend_from_slice(&(raw.len() as u64).to_le_bytes());
    archive.extend_from_slice(orig_sha.as_bytes());
    archive.extend_from_slice(&payload);

    let comp_len = archive.len();
    let phys_bpb = (comp_len as f64 * 8.0) / raw.len().max(1) as f64;
    let rep = PhysicalCompressReport {
        orig_bytes: raw.len(),
        compressed_bytes: comp_len,
        physical_bpb: phys_bpb,
        physical_ratio: raw.len() as f64 / comp_len.max(1) as f64,
        ttt_updates,
        orig_sha256: orig_sha,
    };
    (archive, rep)
}

pub fn decompress_bytes_with_brain(brain: &DeepContractiveBrain, archive: &[u8]) -> Result<Vec<u8>, String> {
    if archive.len() < 79 || &archive[0..6] != OMG_MAGIC {
        return Err("invalid OMGZ15 archive header".into());
    }
    let enable_ttt = (archive[6] & 1) != 0;
    let mut len_arr = [0u8; 8];
    len_arr.copy_from_slice(&archive[7..15]);
    let orig_len = u64::from_le_bytes(len_arr) as usize;
    let expected_sha = std::str::from_utf8(&archive[15..79]).map_err(|_| "corrupt sha256 header")?;
    let bitstream = &archive[79..];

    let mut work_brain = brain.clone();
    let mut stream = CausalStreamState::new(&work_brain.cfg);
    let mut ttt_stream = CausalStreamState::new(&work_brain.cfg);
    let mut dec = ArithmeticDecoder::new(bitstream);
    let mut restored = Vec::with_capacity(orig_len);

    let use_lo = work_brain.active_policy.use_log_odds_mix;
    let mut blk_in = Vec::with_capacity(TTT_BLOCK);
    let mut blk_tgt = Vec::with_capacity(TTT_BLOCK);
    let mut prev_byte = 0u8;

    for idx in 0..orig_len {
        let (p_mix, p_brain, experts, active, ctx, lat) = if idx == 0 {
            let unif = [1.0f32 / 256.0; 256];
            (unif, unif, [unif; NUM_EPISODIC_EXPERTS], [false; NUM_EPISODIC_EXPERTS], 0usize, vec![0.0f32; work_brain.cfg.d_model])
        } else {
            work_brain.predict_next_byte_full(prev_byte, &mut stream, None)
        };
        let cdf = probs_to_cdf(&p_mix);
        let sym = dec.decode_symbol(&cdf);
        restored.push(sym);

        if idx > 0 {
            let surprise = -(p_mix[sym as usize].max(1e-7).log2());
            work_brain.meta_mixer.observe_online(ctx, &p_brain, &experts, &active, &p_mix, sym, use_lo, 0.16);
            work_brain.episodic.observe(&stream.recent_bytes, &lat, sym, surprise);
            if enable_ttt {
                blk_in.push(prev_byte);
                blk_tgt.push(sym);
                if blk_in.len() == TTT_BLOCK {
                    work_brain.ttt_adapt_block(&blk_in, &blk_tgt, &mut ttt_stream, TTT_LR_SCALE);
                    blk_in.clear();
                    blk_tgt.clear();
                }
            }
        }
        prev_byte = sym;
    }

    let actual_sha = sha256_hex(&restored);
    if actual_sha != expected_sha {
        return Err(format!("bitstream SHA-256 mismatch! expected={} got={}", expected_sha, actual_sha));
    }
    Ok(restored)
}

pub fn compress_file_to_omg(
    brain: &DeepContractiveBrain,
    in_path: &Path,
    out_path: &Path,
    enable_ttt: bool,
) -> Result<PhysicalCompressReport, String> {
    let raw = fs::read(in_path).map_err(|e| format!("read failed: {}", e))?;
    let (archive, rep) = compress_bytes_with_brain(brain, &raw, enable_ttt);
    if let Some(p) = out_path.parent() { let _ = fs::create_dir_all(p); }
    fs::write(out_path, &archive).map_err(|e| format!("write failed: {}", e))?;
    Ok(rep)
}

pub fn decompress_omg_to_file(brain: &DeepContractiveBrain, in_path: &Path, out_path: &Path) -> Result<(usize, String), String> {
    let archive = fs::read(in_path).map_err(|e| format!("read failed: {}", e))?;
    let restored = decompress_bytes_with_brain(brain, &archive)?;
    let sha = sha256_hex(&restored);
    if let Some(p) = out_path.parent() { let _ = fs::create_dir_all(p); }
    fs::write(out_path, &restored).map_err(|e| format!("write failed: {}", e))?;
    Ok((restored.len(), sha))
}
