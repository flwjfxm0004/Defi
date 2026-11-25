//! Pure-Rust Zero-Dependency Exact 32-Bit Witten-Neal-Cleary Arithmetic Bitstream Codec (`TropiCore-Omega v12.0`).
//!
//! Provides IRREFUTABLE physical lossless compression & decompression (`.omg` files) using the
//! Deep Contractive Brain + Tropical Episodic Memory's strictly-causal 256-way byte distributions:
//!   - `compress_bytes_with_brain`: encodes ANY real file into `[OMGZ12][orig_len:u64][sha256:64B][bitstream]`
//!   - `decompress_bytes_with_brain`: decodes byte-by-byte and verifies bit-exact `SHA-256` match!

use std::fs;
use std::path::Path;

use crate::brain::{CausalStreamState, DeepContractiveBrain};
use crate::sha256::sha256_hex;

const OMG_MAGIC: &[u8; 6] = b"OMGZ12";
const FULL_RANGE: u64 = 0xFFFF_FFFF;
const HALF: u64 = 0x8000_0000;
const QUARTER: u64 = 0x4000_0000;
const THREE_QUARTERS: u64 = 0xC000_0000;
const TOTAL_FREQ: u64 = 16_384; // 2^14 -> range >= 2^30 gives >= 2^16 precision margin

/// Deterministically quantize a 256-way probability distribution into exact positive integer CDF (`sum == TOTAL_FREQ`).
pub fn probs_to_cdf(probs: &[f32; 256]) -> [u64; 257] {
    let mut freqs = [1u64; 256]; // every byte guaranteed >= 1 (zero chance of zero-width interval!)
    let budget = TOTAL_FREQ - 256;
    let mut sum_p = 0.0f64;
    for &p in probs {
        sum_p += if p.is_finite() && p > 0.0 { p as f64 } else { 0.0 };
    }
    if sum_p <= 1e-12 {
        sum_p = 1.0;
    }
    let mut assigned = 0u64;
    let mut max_i = 0usize;
    let mut max_f = 0u64;
    for i in 0..256 {
        let p = if probs[i].is_finite() && probs[i] > 0.0 { probs[i] as f64 / sum_p } else { 0.0 };
        let add = (p * budget as f64).floor() as u64;
        freqs[i] += add;
        assigned += add;
        if freqs[i] > max_f {
            max_f = freqs[i];
            max_i = i;
        }
    }
    if assigned < budget {
        freqs[max_i] += budget - assigned;
    }
    let mut cdf = [0u64; 257];
    for i in 0..256 {
        cdf[i + 1] = cdf[i] + freqs[i];
    }
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
        Self {
            low: 0,
            high: FULL_RANGE,
            pending: 0,
            cur_byte: 0,
            bit_pos: 0,
            out: Vec::with_capacity(capacity_hint),
        }
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
        if self.low < QUARTER {
            self.emit_bit_and_pending(0);
        } else {
            self.emit_bit_and_pending(1);
        }
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
        let mut dec = Self {
            data,
            byte_idx: 0,
            bit_pos: 0,
            low: 0,
            high: FULL_RANGE,
            value: 0,
        };
        for _ in 0..32 {
            dec.value = (dec.value << 1) | (dec.read_bit() as u64);
        }
        dec
    }

    #[inline]
    fn read_bit(&mut self) -> u8 {
        if self.byte_idx >= self.data.len() {
            return 0;
        }
        let b = (self.data[self.byte_idx] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_idx += 1;
        }
        b
    }

    pub fn decode_symbol(&mut self, cdf: &[u64; 257]) -> u8 {
        let range = self.high - self.low + 1;
        let scaled = ((self.value - self.low + 1) * TOTAL_FREQ - 1) / range;

        // Binary search `cdf[s] <= scaled < cdf[s + 1]`
        let mut lo = 0usize;
        let mut hi = 256usize;
        while lo + 1 < hi {
            let mid = (lo + hi) >> 1;
            if cdf[mid] <= scaled {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let s = lo;
        self.high = self.low + (range * cdf[s + 1]) / TOTAL_FREQ - 1;
        self.low = self.low + (range * cdf[s]) / TOTAL_FREQ;

        loop {
            if self.high < HALF {
                // nothing to subtract
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
    pub orig_sha256: String,
}

/// Physically compress `raw` into a self-contained `.omg` archive using strictly-causal Brain + Episodic predictions!
pub fn compress_bytes_with_brain(brain: &DeepContractiveBrain, raw: &[u8]) -> (Vec<u8>, PhysicalCompressReport) {
    let orig_sha = sha256_hex(raw);
    let mut work_brain = brain.clone();
    let mut stream = CausalStreamState::new(&work_brain.cfg);
    let mut enc = ArithmeticEncoder::new(raw.len() / 2 + 64);

    let mut prev_byte = 0u8;
    for &actual in raw {
        let (probs, lat) = work_brain.predict_next_byte_distribution(prev_byte, &mut stream);
        let cdf = probs_to_cdf(&probs);
        enc.encode_symbol(actual, &cdf);
        let p = probs[actual as usize].max(1e-7);
        let surprise = -(p.log2());
        work_brain.episodic.observe(&stream.recent_bytes, &lat, actual, surprise);
        prev_byte = actual;
    }

    let payload = enc.finish();
    let mut archive = Vec::with_capacity(6 + 8 + 64 + payload.len());
    archive.extend_from_slice(OMG_MAGIC);
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
        orig_sha256: orig_sha,
    };
    (archive, rep)
}

/// Losslessly decompress an `.omg` archive and verify bit-exact `SHA-256` against the original file!
pub fn decompress_bytes_with_brain(brain: &DeepContractiveBrain, archive: &[u8]) -> Result<Vec<u8>, String> {
    if archive.len() < 78 || &archive[0..6] != OMG_MAGIC {
        return Err("invalid OMGZ12 archive header".into());
    }
    let mut len_arr = [0u8; 8];
    len_arr.copy_from_slice(&archive[6..14]);
    let orig_len = u64::from_le_bytes(len_arr) as usize;
    let expected_sha = std::str::from_utf8(&archive[14..78]).map_err(|_| "corrupt sha256 header")?;
    let bitstream = &archive[78..];

    let mut work_brain = brain.clone();
    let mut stream = CausalStreamState::new(&work_brain.cfg);
    let mut dec = ArithmeticDecoder::new(bitstream);
    let mut restored = Vec::with_capacity(orig_len);

    let mut prev_byte = 0u8;
    for _ in 0..orig_len {
        let (probs, lat) = work_brain.predict_next_byte_distribution(prev_byte, &mut stream);
        let cdf = probs_to_cdf(&probs);
        let sym = dec.decode_symbol(&cdf);
        restored.push(sym);
        let p = probs[sym as usize].max(1e-7);
        let surprise = -(p.log2());
        work_brain.episodic.observe(&stream.recent_bytes, &lat, sym, surprise);
        prev_byte = sym;
    }

    let actual_sha = sha256_hex(&restored);
    if actual_sha != expected_sha {
        return Err(format!("bitstream SHA-256 mismatch! expected={} got={}", expected_sha, actual_sha));
    }
    Ok(restored)
}

pub fn compress_file_to_omg(brain: &DeepContractiveBrain, in_path: &Path, out_path: &Path) -> Result<PhysicalCompressReport, String> {
    let raw = fs::read(in_path).map_err(|e| format!("read failed: {}", e))?;
    let (archive, rep) = compress_bytes_with_brain(brain, &raw);
    if let Some(p) = out_path.parent() {
        let _ = fs::create_dir_all(p);
    }
    fs::write(out_path, &archive).map_err(|e| format!("write failed: {}", e))?;
    Ok(rep)
}

pub fn decompress_omg_to_file(brain: &DeepContractiveBrain, in_path: &Path, out_path: &Path) -> Result<(usize, String), String> {
    let archive = fs::read(in_path).map_err(|e| format!("read failed: {}", e))?;
    let restored = decompress_bytes_with_brain(brain, &archive)?;
    let sha = sha256_hex(&restored);
    if let Some(p) = out_path.parent() {
        let _ = fs::create_dir_all(p);
    }
    fs::write(out_path, &restored).map_err(|e| format!("write failed: {}", e))?;
    Ok((restored.len(), sha))
}
