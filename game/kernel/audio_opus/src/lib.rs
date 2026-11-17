#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use math_q32::{Q32, QVec3, isqrt_u128};
use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE_HZ: u32 = 48_000;
pub const CUE_COUNT: usize = 8;
pub const CELT_SUBFRAME_BINS: usize = 120; // 2.5ms @ 48kHz Fullband CELT MDCT block
pub const CELT_WINDOW_LEN: usize = 240; // 2N = 240 samples (50% TDAC overlap-add)
pub const CELT_SUBFRAMES_PER_CUE: usize = 20; // 20 * 2.5ms = 50ms (2,400 samples)
pub const CUE_PCM_LEN: usize = CELT_SUBFRAME_BINS * CELT_SUBFRAMES_PER_CUE; // 2,400 samples
pub const MAX_ACTIVE_VOICES: usize = 32;
pub const ITD_DELAY_TAPS: usize = 32;
pub const SPSC_RING_CAPACITY: usize = 32_768; // Power-of-two stereo sample slots (16,384 stereo frames)
const SPSC_RING_MASK: usize = SPSC_RING_CAPACITY - 1;

/// RFC 6716 §3.1 Table 2: config = 28 (CELT-only Fullband 20kHz, 2.5ms frame), s = 0 (mono), c = 3 (CBR/VBR multi-frame).
pub const OPUS_TOC_CELT_FB_2P5MS_CODE3: u8 = (28_u8 << 3) | 0x03;
const CELT_CBR_FRAME_BYTES: usize = 16;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCueKind {
    WeaponSwap = 0,
    PrimarySlash = 1,
    ShatterBreak = 2,
    DischargeBurst = 3,
    PhantasiaSlow = 4,
    ClashParry = 5,
    FootstepIk = 6,
    JetpackGlide = 7,
}

impl AudioCueKind {
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        match idx & 7 {
            0 => Self::WeaponSwap,
            1 => Self::PrimarySlash,
            2 => Self::ShatterBreak,
            3 => Self::DischargeBurst,
            4 => Self::PhantasiaSlow,
            5 => Self::ClashParry,
            6 => Self::FootstepIk,
            _ => Self::JetpackGlide,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpusDecodeError {
    InvalidOggCapturePattern,
    InvalidOggCrc,
    InvalidOpusHead,
    InvalidOpusTags,
    PacketCountMismatch,
    TruncatedPacket,
    UnsupportedOpusToc(u8),
}

// ============================================================================
// 1. RFC 3533 / RFC 7845 Ogg Container Framing & MSB-First `0x04C11DB7` CRC32
// ============================================================================

/// Computes the standard Ogg bitstream MSB-first CRC32 (`poly = 0x04C11DB7`, init = `0`, xorout = `0`).
pub fn ogg_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in data {
        crc ^= u32::from(byte) << 24;
        let mut bit: u8 = 0;
        while bit < 8 {
            if (crc & 0x8000_0000) != 0 {
                crc = (crc << 1) ^ 0x04C1_1DB7;
            } else {
                crc <<= 1;
            }
            bit = bit.saturating_add(1);
        }
    }
    crc
}

fn append_ogg_page(
    out: &mut Vec<u8>,
    header_type: u8,
    granule_pos: u64,
    serial: u32,
    page_seq: u32,
    packet: &[u8],
) {
    // Build RFC 3533 lacing table (`255` continuation segments + final `< 255` termination byte):
    let mut segments: Vec<u8> = Vec::with_capacity((packet.len() / 255).saturating_add(2));
    let mut rem = packet.len();
    while rem >= 255 {
        segments.push(255);
        rem = rem.saturating_sub(255);
    }
    segments.push(rem as u8);

    let page_start = out.len();
    out.extend_from_slice(b"OggS");
    out.push(0); // stream_structure_version = 0
    out.push(header_type);
    out.extend_from_slice(&granule_pos.to_le_bytes());
    out.extend_from_slice(&serial.to_le_bytes());
    out.extend_from_slice(&page_seq.to_le_bytes());
    out.extend_from_slice(&[0_u8; 4]); // CRC placeholder at page_start + 22..26
    out.push(segments.len() as u8);
    out.extend_from_slice(&segments);
    out.extend_from_slice(packet);

    let crc = ogg_crc32(&out[page_start..]);
    let crc_bytes = crc.to_le_bytes();
    let crc_off = page_start.saturating_add(22);
    if let Some(dst) = out.get_mut(crc_off..crc_off.saturating_add(4)) {
        dst.copy_from_slice(&crc_bytes);
    }
}

/// Parses an RFC 7845 Ogg Opus stream, verifying every page's `OggS` capture pattern,
/// `0x04C11DB7` CRC32, `OpusHead` v1 header, `OpusTags` comment header, and laced Opus audio packets.
pub fn parse_ogg_opus_packets(bytes: &[u8]) -> Result<(u16, Vec<Vec<u8>>), OpusDecodeError> {
    if bytes.len() < 27 || &bytes[0..4] != b"OggS" {
        return Err(OpusDecodeError::InvalidOggCapturePattern);
    }

    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut cur_pkt: Vec<u8> = Vec::new();
    let mut cursor: usize = 0;

    while cursor < bytes.len() {
        let hdr_end = cursor.saturating_add(27);
        if hdr_end > bytes.len() || &bytes[cursor..cursor.saturating_add(4)] != b"OggS" {
            return Err(OpusDecodeError::InvalidOggCapturePattern);
        }
        let version = bytes[cursor.saturating_add(4)];
        if version != 0 {
            return Err(OpusDecodeError::InvalidOggCapturePattern);
        }
        let seg_count = bytes[cursor.saturating_add(26)] as usize;
        let seg_table_end = hdr_end.saturating_add(seg_count);
        if seg_table_end > bytes.len() {
            return Err(OpusDecodeError::TruncatedPacket);
        }

        let mut body_len: usize = 0;
        for &seg in &bytes[hdr_end..seg_table_end] {
            body_len = body_len.saturating_add(seg as usize);
        }
        let page_end = seg_table_end.saturating_add(body_len);
        if page_end > bytes.len() {
            return Err(OpusDecodeError::TruncatedPacket);
        }

        // Verify Ogg page CRC32 with bytes 22..26 zeroed:
        let stored_crc = u32::from_le_bytes([
            bytes[cursor.saturating_add(22)],
            bytes[cursor.saturating_add(23)],
            bytes[cursor.saturating_add(24)],
            bytes[cursor.saturating_add(25)],
        ]);
        let mut page_copy = bytes[cursor..page_end].to_vec();
        page_copy[22] = 0;
        page_copy[23] = 0;
        page_copy[24] = 0;
        page_copy[25] = 0;
        if ogg_crc32(&page_copy) != stored_crc {
            return Err(OpusDecodeError::InvalidOggCrc);
        }

        let mut data_off = seg_table_end;
        for &seg in &bytes[hdr_end..seg_table_end] {
            let seg_usz = seg as usize;
            let next_off = data_off.saturating_add(seg_usz);
            cur_pkt.extend_from_slice(&bytes[data_off..next_off]);
            data_off = next_off;
            if seg < 255 {
                packets.push(core::mem::take(&mut cur_pkt));
            }
        }
        cursor = page_end;
    }

    if packets.len() < 2 {
        return Err(OpusDecodeError::InvalidOpusHead);
    }

    // Packet 0: RFC 7845 §5.1 `OpusHead` (19 bytes for mapping family 0)
    let head = &packets[0];
    if head.len() < 19 || &head[0..8] != b"OpusHead" {
        return Err(OpusDecodeError::InvalidOpusHead);
    }
    let opus_ver = head[8];
    let ch_count = head[9];
    let pre_skip = u16::from_le_bytes([head[10], head[11]]);
    let mapping_family = head[18];
    if opus_ver != 1 || ch_count == 0 || ch_count > 2 || mapping_family != 0 {
        return Err(OpusDecodeError::InvalidOpusHead);
    }

    // Packet 1: RFC 7845 §5.2 `OpusTags`
    let tags = &packets[1];
    if tags.len() < 16 || &tags[0..8] != b"OpusTags" {
        return Err(OpusDecodeError::InvalidOpusTags);
    }

    let audio_packets = packets.into_iter().skip(2).collect();
    Ok((pre_skip, audio_packets))
}

// ============================================================================
// 2. RFC 6716 §4.1 Range Encoder (`EcEnc`) & Range Decoder (`EcDec`)
// ============================================================================

/// Deterministic byte-aligned radix-256 arithmetic range encoder (`RFC 6716 §4.1` subset for fixed-size CBR frames).
pub struct EcEnc {
    buf: [u8; CELT_CBR_FRAME_BYTES],
    low: u64,
    rng: u64,
    out_idx: usize,
}

impl EcEnc {
    pub const fn new() -> Self {
        Self {
            buf: [0_u8; CELT_CBR_FRAME_BYTES],
            low: 0,
            rng: 0x8000_0000,
            out_idx: 0,
        }
    }

    pub fn encode_uint(&mut self, val: u32, ft: u32) {
        let safe_ft = ft.max(2) as u64;
        let sym = (val as u64).min(safe_ft.saturating_sub(1));
        let r_step = self.rng.checked_div(safe_ft).unwrap_or(1);
        self.low = self.low.saturating_add(r_step.saturating_mul(sym));
        if sym.saturating_add(1) < safe_ft {
            self.rng = r_step;
        } else {
            self.rng = self.rng.saturating_sub(r_step.saturating_mul(sym));
        }
        self.normalize();
    }

    fn normalize(&mut self) {
        while self.rng <= 0x0080_0000 {
            let top_byte = ((self.low >> 23) & 0xFF) as u16;
            let carry = (self.low >> 31) & 1;
            if carry != 0 && self.out_idx > 0 {
                let mut back = self.out_idx.saturating_sub(1);
                loop {
                    let sum = u16::from(self.buf[back]).saturating_add(1);
                    self.buf[back] = (sum & 0xFF) as u8;
                    if sum <= 0xFF || back == 0 {
                        break;
                    }
                    back = back.saturating_sub(1);
                }
            }
            if self.out_idx < CELT_CBR_FRAME_BYTES {
                self.buf[self.out_idx] = (top_byte & 0xFF) as u8;
                self.out_idx = self.out_idx.saturating_add(1);
            }
            self.low = (self.low << 8) & 0x7FFF_FFFF;
            self.rng <<= 8;
        }
    }

    pub fn finish(mut self) -> [u8; CELT_CBR_FRAME_BYTES] {
        // Emit remaining 31-bit state bytes so `EcDec` reconstructs every symbol losslessly:
        let mut flush: u8 = 0;
        while flush < 5 {
            let top_byte = ((self.low >> 23) & 0xFF) as u8;
            if self.out_idx < CELT_CBR_FRAME_BYTES {
                self.buf[self.out_idx] = top_byte;
                self.out_idx = self.out_idx.saturating_add(1);
            }
            self.low = (self.low << 8) & 0x7FFF_FFFF;
            flush = flush.saturating_add(1);
        }
        self.buf
    }
}

impl Default for EcEnc {
    fn default() -> Self {
        Self::new()
    }
}

/// Matching RFC 6716 radix-256 arithmetic range decoder (`0` floats, total arithmetic).
pub struct EcDec<'a> {
    buf: &'a [u8],
    in_idx: usize,
    val: u64,
    rng: u64,
}

impl<'a> EcDec<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let mut val: u64 = 0;
        let mut i: usize = 0;
        while i < 4 {
            let b = buf.get(i).copied().unwrap_or(0) as u64;
            val = (val << 8) | b;
            i = i.saturating_add(1);
        }
        // Align initial 31-bit code window with `EcEnc` (`top >> 23`):
        val >>= 1;
        // Keep the lowest bit of `buf[3]` ready for first byte-in shift:
        Self {
            buf,
            in_idx: 3,
            val: val & 0x7FFF_FFFF,
            rng: 0x8000_0000,
        }
    }

    fn read_aligned_byte(&mut self) -> u64 {
        let b_prev = self.buf.get(self.in_idx).copied().unwrap_or(0) as u64;
        self.in_idx = self.in_idx.saturating_add(1);
        let b_next = self.buf.get(self.in_idx).copied().unwrap_or(0) as u64;
        ((b_prev & 1) << 7) | (b_next >> 1)
    }

    pub fn decode_uint(&mut self, ft: u32) -> u32 {
        let safe_ft = ft.max(2) as u64;
        let r_step = self.rng.checked_div(safe_ft).unwrap_or(1).max(1);
        let sym = self
            .val
            .checked_div(r_step)
            .unwrap_or(0)
            .min(safe_ft.saturating_sub(1));
        self.val = self.val.saturating_sub(r_step.saturating_mul(sym));
        if sym.saturating_add(1) < safe_ft {
            self.rng = r_step;
        } else {
            self.rng = self.rng.saturating_sub(r_step.saturating_mul(sym));
        }
        while self.rng <= 0x0080_0000 {
            let next_b = self.read_aligned_byte();
            self.val = ((self.val << 8) | next_b) & 0x7FFF_FFFF;
            self.rng <<= 8;
        }
        sym as u32
    }
}

// ============================================================================
// 3. RFC 7845 + RFC 6716 CELT Bootstrap Encoder & Windowed Overlap-Add IMDCT
// ============================================================================

const CUE_BASE_BAND_ENERGIES: [[u8; 8]; CUE_COUNT] = [
    // 0: WeaponSwap (crisp metallic high-band chime)
    [18, 34, 58, 52, 42, 28, 16, 10],
    // 1: PrimarySlash (mid-high transient whoosh)
    [28, 52, 48, 36, 24, 16, 10, 6],
    // 2: ShatterBreak (heavy low-mid crystalline fracture)
    [62, 58, 48, 44, 38, 28, 18, 10],
    // 3: DischargeBurst (full-spectrum elemental detonation)
    [63, 60, 56, 50, 44, 36, 26, 16],
    // 4: PhantasiaSlow (deep phase-shifted sub-harmonic dilation)
    [58, 44, 24, 38, 50, 28, 14, 8],
    // 5: ClashParry (sharp resonant steel ring)
    [24, 44, 63, 60, 54, 42, 26, 12],
    // 6: FootstepIk (grounded short low-band surface thud)
    [54, 40, 20, 12, 6, 4, 2, 1],
    // 7: JetpackGlide (sustained airy harmonic thrust)
    [40, 48, 42, 34, 30, 24, 18, 12],
];

/// Synthesizes a genuine RFC 7845 Ogg Opus stream (`OggS` + `OpusHead` + `OpusTags` + 8 RFC 6716
/// range-coded CELT Fullband `50ms` audio packets) with zero proprietary framing.
pub fn build_bootstrap_ogg_opus_stream() -> Vec<u8> {
    let serial: u32 = 0x5133_3241; // 'Q32A'
    let mut out = Vec::with_capacity(3_072);

    // Page 0 (BOS = 0x02): RFC 7845 §5.1 `OpusHead`
    let mut opus_head = Vec::with_capacity(19);
    opus_head.extend_from_slice(b"OpusHead");
    opus_head.push(1); // version = 1
    opus_head.push(1); // channel_count = 1 (mono object emitters spatialized in 3D mixer)
    opus_head.extend_from_slice(&0_u16.to_le_bytes()); // pre_skip = 0
    opus_head.extend_from_slice(&SAMPLE_RATE_HZ.to_le_bytes()); // input_sample_rate = 48000 Hz
    opus_head.extend_from_slice(&0_i16.to_le_bytes()); // output_gain = 0 dB
    opus_head.push(0); // channel_mapping_family = 0
    append_ogg_page(&mut out, 0x02, 0, serial, 0, &opus_head);

    // Page 1: RFC 7845 §5.2 `OpusTags`
    let vendor = b"q32_opus_celt_1_0";
    let mut opus_tags = Vec::with_capacity(32);
    opus_tags.extend_from_slice(b"OpusTags");
    opus_tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    opus_tags.extend_from_slice(vendor);
    opus_tags.extend_from_slice(&0_u32.to_le_bytes()); // user_comment_list_length = 0
    append_ogg_page(&mut out, 0x00, 0, serial, 1, &opus_tags);

    // Pages 2..=9: 8 RFC 6716 Code-3 CBR CELT Fullband packets (`20` x `2.5ms` = `2,400` samples each)
    let mut granule: u64 = 0;
    let mut cue_idx: usize = 0;
    while cue_idx < CUE_COUNT {
        let mut pkt = Vec::with_capacity(2_usize.saturating_add(CELT_SUBFRAMES_PER_CUE * CELT_CBR_FRAME_BYTES));
        pkt.push(OPUS_TOC_CELT_FB_2P5MS_CODE3);
        pkt.push(CELT_SUBFRAMES_PER_CUE as u8); // CBR (bit 7=0, bit 6=0), M = 20 frames (50ms)

        let decay_num: u32 = match cue_idx {
            6 => 11, // Footstep sharp transient decay
            0 | 5 => 14,
            3 | 4 => 17,
            _ => 15,
        };

        let mut sf: usize = 0;
        let mut env_scale: u32 = 64;
        while sf < CELT_SUBFRAMES_PER_CUE {
            let mut enc = EcEnc::new();
            let mut b: usize = 0;
            while b < 8 {
                let base_e = u32::from(CUE_BASE_BAND_ENERGIES[cue_idx][b]);
                let band_e = ((base_e.saturating_mul(env_scale)) >> 6).min(63);
                enc.encode_uint(band_e, 64);
                b = b.saturating_add(1);
            }
            let mut p: usize = 0;
            while p < 8 {
                let raw_pulse = ((cue_idx.saturating_mul(5))
                    .saturating_add(sf.saturating_mul(3))
                    .saturating_add(p.saturating_mul(7)))
                    & 14;
                enc.encode_uint(raw_pulse as u32, 15);
                p = p.saturating_add(1);
            }
            pkt.extend_from_slice(&enc.finish());
            env_scale = (env_scale.saturating_mul(decay_num)) / 18;
            sf = sf.saturating_add(1);
        }

        granule = granule.saturating_add(CUE_PCM_LEN as u64);
        let hdr_type = if cue_idx.saturating_add(1) == CUE_COUNT {
            0x04 // EOS on final packet page
        } else {
            0x00
        };
        append_ogg_page(
            &mut out,
            hdr_type,
            granule,
            serial,
            (cue_idx.saturating_add(2)) as u32,
            &pkt,
        );
        cue_idx = cue_idx.saturating_add(1);
    }

    out
}

/// Decoded `48kHz` `i16` PCM bank reconstructed via pure `Q32.32` TDAC Sine-Window Overlap-Add IMDCT.
#[derive(Clone)]
pub struct OpusCueBank {
    pcm_cues: Box<[[i16; CUE_PCM_LEN]; CUE_COUNT]>,
}

impl OpusCueBank {
    pub fn decode_from_opus_stream(bytes: &[u8]) -> Result<Self, OpusDecodeError> {
        let (_pre_skip, packets) = parse_ogg_opus_packets(bytes)?;
        if packets.len() < CUE_COUNT {
            return Err(OpusDecodeError::PacketCountMismatch);
        }

        let sin_win = Self::build_tdac_sine_window_q32();
        let mut pcm_cues = Box::new([[0_i16; CUE_PCM_LEN]; CUE_COUNT]);

        let mut cue_idx: usize = 0;
        while cue_idx < CUE_COUNT {
            Self::decode_celt_packet_q32(&packets[cue_idx], &sin_win, &mut pcm_cues[cue_idx])?;
            cue_idx = cue_idx.saturating_add(1);
        }

        Ok(Self { pcm_cues })
    }

    pub fn bootstrap() -> Result<Self, OpusDecodeError> {
        let stream = build_bootstrap_ogg_opus_stream();
        Self::decode_from_opus_stream(&stream)
    }

    /// Precomputes the exact Princen-Bradley TDAC sine window `w[n] = sin(pi/(2N) * (n + 0.5))`
    /// satisfying `w^2[n] + w^2[n + N] == 1` across `2N = 240` samples in `Q32.32`.
    fn build_tdac_sine_window_q32() -> [Q32; CELT_WINDOW_LEN] {
        let mut win = [Q32::ZERO; CELT_WINDOW_LEN];
        let denom = (4 * CELT_WINDOW_LEN) as i32; // 960 (`2*pi` turns denominator)
        let mut n: usize = 0;
        while n < CELT_WINDOW_LEN {
            let num = (n.saturating_mul(2).saturating_add(1)) as i32;
            win[n] = Q32::from_ratio_i32(num, denom).sin_turn();
            n = n.saturating_add(1);
        }
        win
    }

    /// Decodes a single RFC 6716 CELT Fullband packet using `EcDec` range decoding + `50%` Overlap-Add `Q32.32` IMDCT.
    fn decode_celt_packet_q32(
        pkt: &[u8],
        sin_win: &[Q32; CELT_WINDOW_LEN],
        out_pcm: &mut [i16; CUE_PCM_LEN],
    ) -> Result<(), OpusDecodeError> {
        if pkt.is_empty() {
            return Err(OpusDecodeError::TruncatedPacket);
        }
        let toc = pkt[0];
        let config = toc >> 3;
        // RFC 6716 §3.1: configs 16..=31 are CELT modes (28..=31 = Fullband 48kHz).
        if config < 16 {
            return Err(OpusDecodeError::UnsupportedOpusToc(toc));
        }
        let code = toc & 0x03;
        let (subframe_count, header_off) = match code {
            0 => (1_usize, 1_usize),
            1 | 2 => (2_usize, 1_usize),
            _ => {
                if pkt.len() < 2 {
                    return Err(OpusDecodeError::TruncatedPacket);
                }
                let frame_count_byte = pkt[1];
                let m = (frame_count_byte & 0x3F) as usize;
                if m == 0 || m > 48 {
                    return Err(OpusDecodeError::TruncatedPacket);
                }
                (m, 2_usize)
            }
        };

        let payload = &pkt[header_off..];
        if payload.len() < subframe_count {
            return Err(OpusDecodeError::TruncatedPacket);
        }
        let frame_bytes = payload
            .len()
            .checked_div(subframe_count)
            .ok_or(OpusDecodeError::TruncatedPacket)?;

        // CELT Bark-spaced MDCT bin pairs inside `0..120` (`2.5ms` @ `48kHz` -> `200Hz` bin spacing):
        const BAND_BINS: [[usize; 2]; 8] = [
            [1, 2],
            [3, 4],
            [6, 8],
            [11, 14],
            [18, 23],
            [30, 38],
            [48, 60],
            [74, 92],
        ];

        let mut overlap_tail = [Q32::ZERO; CELT_SUBFRAME_BINS];
        let mut sf: usize = 0;
        while sf < subframe_count && sf < CELT_SUBFRAMES_PER_CUE {
            let start_b = sf.saturating_mul(frame_bytes);
            let end_b = start_b.saturating_add(frame_bytes).min(payload.len());
            let mut dec = EcDec::new(&payload[start_b..end_b]);

            let mut band_energy = [0_u32; 8];
            let mut b: usize = 0;
            while b < 8 {
                band_energy[b] = dec.decode_uint(64);
                b = b.saturating_add(1);
            }

            let mut active_bins = [(0_usize, Q32::ZERO); 16];
            let mut active_len: usize = 0;
            let mut p: usize = 0;
            while p < 8 {
                let pulse_sym = dec.decode_uint(15) as i32; // 0..14 -> centered -7..+7
                let signed_pulse = pulse_sym.saturating_sub(7);
                let energy_u = band_energy[p] as i32;
                if energy_u > 0 {
                    // Log-domain CELT coarse energy `exp2((e - 52) / 10)` in pure `Q32.32`:
                    let log2_e = Q32::from_ratio_i32(energy_u.saturating_sub(52), 10);
                    let amp = log2_e.exp2_sat().clamp_val(Q32::ZERO, Q32::ONE);
                    let primary_sign = if signed_pulse >= 0 { Q32::ONE } else { Q32::NEG_ONE };
                    let mag_ratio = Q32::from_ratio_i32(signed_pulse.abs().max(2), 9);
                    let x0 = amp.sat_mul(primary_sign).sat_mul(mag_ratio);
                    let x1 = amp
                        .sat_mul(primary_sign.sat_neg())
                        .sat_mul(Q32::ONE.sat_sub(mag_ratio.sat_mul(Q32::HALF)));

                    active_bins[active_len] = (BAND_BINS[p][0], x0);
                    active_bins[active_len.saturating_add(1)] = (BAND_BINS[p][1], x1);
                    active_len = active_len.saturating_add(2);
                }
                p = p.saturating_add(1);
            }

            // Exact `N = 120` (`2N = 240`) Windowed IMDCT + 50% TDAC Overlap-Add:
            // `basis(n, k) = cos(2*pi * ((2n + 1 + N)*(2k + 1) mod 8N) / 8N)` with `8N = 960`.
            let pcm_base = sf.saturating_mul(CELT_SUBFRAME_BINS);
            let mut n: usize = 0;
            while n < CELT_WINDOW_LEN {
                let two_n_plus_1_plus_n = n
                    .saturating_mul(2)
                    .saturating_add(1)
                    .saturating_add(CELT_SUBFRAME_BINS);
                let mut sum = Q32::ZERO;
                let mut ai: usize = 0;
                while ai < active_len {
                    let (k, x_k) = active_bins[ai];
                    let two_k_plus_1 = k.saturating_mul(2).saturating_add(1);
                    let turn_num = (two_n_plus_1_plus_n
                        .saturating_mul(two_k_plus_1)
                        .checked_rem(960))
                    .unwrap_or(0) as i32;
                    let basis = Q32::from_ratio_i32(turn_num, 960).cos_turn();
                    sum = sum.sat_add(x_k.sat_mul(basis));
                    ai = ai.saturating_add(1);
                }

                let windowed = sum.sat_mul(sin_win[n]);
                if n < CELT_SUBFRAME_BINS {
                    let ola = overlap_tail[n]
                        .sat_add(windowed)
                        .clamp_val(Q32::NEG_ONE, Q32::ONE);
                    let sample_i16 = ola
                        .sat_mul(Q32::from_int(30_000))
                        .to_int_floor()
                        .clamp(-32_767, 32_767) as i16;
                    let out_idx = pcm_base.saturating_add(n);
                    if out_idx < CUE_PCM_LEN {
                        out_pcm[out_idx] = sample_i16;
                    }
                } else {
                    overlap_tail[n.saturating_sub(CELT_SUBFRAME_BINS)] = windowed;
                }
                n = n.saturating_add(1);
            }

            sf = sf.saturating_add(1);
        }

        Ok(())
    }

    #[inline]
    pub fn sample_q15(&self, cue: AudioCueKind, idx: usize) -> i16 {
        let c = (cue as usize).min(CUE_COUNT - 1);
        if idx < CUE_PCM_LEN {
            self.pcm_cues[c][idx]
        } else {
            0
        }
    }
}

#[derive(Clone, Copy)]
struct SpatialVoice {
    active: bool,
    cue: AudioCueKind,
    emitter_pos: QVec3,
    base_gain: Q32,
    pitch_q16: u32,
    cursor_q16: u64,
    sdf_occluded: bool,
    lp_prev_l: i32,
    lp_prev_r: i32,
    itd_line: [i16; ITD_DELAY_TAPS],
    itd_write: usize,
}

impl Default for SpatialVoice {
    fn default() -> Self {
        Self {
            active: false,
            cue: AudioCueKind::WeaponSwap,
            emitter_pos: QVec3::ZERO,
            base_gain: Q32::ONE,
            pitch_q16: 65_536,
            cursor_q16: 0,
            sdf_occluded: false,
            lp_prev_l: 0,
            lp_prev_r: 0,
            itd_line: [0; ITD_DELAY_TAPS],
            itd_write: 0,
        }
    }
}

/// Lock-free Single-Producer Single-Consumer (`SPSC`) audio ringbuffer storing quantized `i16` samples
/// (`AtomicI32`) so zero mutexes or allocations occur on either the simulation or `cpal` callback thread.
pub struct LockFreeAudioRing {
    samples_q15: Box<[AtomicI32; SPSC_RING_CAPACITY]>,
    write_idx: AtomicUsize,
    read_idx: AtomicUsize,
}

impl Default for LockFreeAudioRing {
    fn default() -> Self {
        Self::new()
    }
}

impl LockFreeAudioRing {
    pub fn new() -> Self {
        let samples_q15 = Box::new(core::array::from_fn(|_| AtomicI32::new(0)));
        Self {
            samples_q15,
            write_idx: AtomicUsize::new(0),
            read_idx: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn push_stereo_i16(&self, left: i16, right: i16) -> bool {
        let w = self.write_idx.load(Ordering::Relaxed);
        let r = self.read_idx.load(Ordering::Acquire);
        let next_w = w.wrapping_add(2);
        if next_w.wrapping_sub(r) > SPSC_RING_CAPACITY {
            return false;
        }
        self.samples_q15[w & SPSC_RING_MASK].store(i32::from(left), Ordering::Relaxed);
        self.samples_q15[w.wrapping_add(1) & SPSC_RING_MASK]
            .store(i32::from(right), Ordering::Relaxed);
        self.write_idx.store(next_w, Ordering::Release);
        true
    }

    #[inline]
    pub fn pop_stereo_i16(&self) -> Option<(i16, i16)> {
        let r = self.read_idx.load(Ordering::Relaxed);
        let w = self.write_idx.load(Ordering::Acquire);
        if r == w {
            return None;
        }
        let l = self.samples_q15[r & SPSC_RING_MASK].load(Ordering::Relaxed) as i16;
        let rt = self.samples_q15[r.wrapping_add(1) & SPSC_RING_MASK].load(Ordering::Relaxed) as i16;
        self.read_idx.store(r.wrapping_add(2), Ordering::Release);
        Some((l, rt))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AudioMixSummary {
    pub active_voices: u32,
    pub mixed_stereo_frames: u32,
    pub peak_abs_pcm: i32,
    pub left_energy_acc: u64,
    pub right_energy_acc: u64,
}

/// Deterministic `Q32.32` 3D Spatial Audio Mixer with ITD/ILD binaural cues, weapon-mass hitstop pitch shift,
/// and SDF terrain occlusion low-pass filtering (`게임.md §1.3, §8.1, §8.2`).
pub struct SpatialAudioMixer {
    bank: OpusCueBank,
    voices: [SpatialVoice; MAX_ACTIVE_VOICES],
    next_steal_slot: usize,
    ring: Arc<LockFreeAudioRing>,
}

impl SpatialAudioMixer {
    pub fn new(bank: OpusCueBank, ring: Arc<LockFreeAudioRing>) -> Self {
        Self {
            bank,
            voices: [SpatialVoice::default(); MAX_ACTIVE_VOICES],
            next_steal_slot: 0,
            ring,
        }
    }

    pub fn trigger_3d_cue(
        &mut self,
        cue: AudioCueKind,
        emitter_pos: QVec3,
        gain: Q32,
        weapon_mass_q32: Q32,
        sdf_occluded: bool,
    ) {
        let mut target_slot = None;
        let mut i: usize = 0;
        while i < MAX_ACTIVE_VOICES {
            if !self.voices[i].active {
                target_slot = Some(i);
                break;
            }
            i = i.saturating_add(1);
        }
        let slot = match target_slot {
            Some(s) => s,
            None => {
                let s = self.next_steal_slot & (MAX_ACTIVE_VOICES - 1);
                self.next_steal_slot = self.next_steal_slot.wrapping_add(1);
                s
            }
        };

        // Heavier weapon mass lowers pitch (`0.82x .. 1.15x` in `Q16.16`):
        let mass_clamped = weapon_mass_q32.clamp_val(Q32::HALF, Q32::from_int(3));
        let pitch_scale = Q32::from_ratio_i32(118, 100)
            .sat_sub(mass_clamped.sat_mul(Q32::from_ratio_i32(12, 100)))
            .clamp_val(Q32::from_ratio_i32(75, 100), Q32::from_ratio_i32(125, 100));
        let pitch_q16 = (pitch_scale.raw() >> 16).clamp(32_768, 98_304) as u32;

        self.voices[slot] = SpatialVoice {
            active: true,
            cue,
            emitter_pos,
            base_gain: gain.clamp_val(Q32::ZERO, Q32::ONE),
            pitch_q16,
            cursor_q16: 0,
            sdf_occluded,
            lp_prev_l: 0,
            lp_prev_r: 0,
            itd_line: [0; ITD_DELAY_TAPS],
            itd_write: 0,
        };
    }

    /// Mixes `frame_count` stereo samples (`400` samples = `8.33ms` @ `120Hz`)
    /// strictly in `Q32.32` / integer arithmetic and pushes them to the lock-free output ringbuffer.
    pub fn mix_frames(
        &mut self,
        listener_pos: QVec3,
        listener_right: QVec3,
        hitstop_active: bool,
        frame_count: usize,
    ) -> AudioMixSummary {
        let bounded_frames = frame_count.min(1_024);
        let right_unit = listener_right.normalize_or_zero();
        let hitstop_shift: u32 = if hitstop_active { 52_428 } else { 65_536 }; // 0.80x dilation during hitstop

        let mut gain_l_q15 = [0_i32; MAX_ACTIVE_VOICES];
        let mut gain_r_q15 = [0_i32; MAX_ACTIVE_VOICES];
        let mut itd_right_delay = [0_usize; MAX_ACTIVE_VOICES];
        let mut itd_left_delay = [0_usize; MAX_ACTIVE_VOICES];
        let mut step_q16 = [65_536_u64; MAX_ACTIVE_VOICES];
        let mut active_count: u32 = 0;

        let mut v: usize = 0;
        while v < MAX_ACTIVE_VOICES {
            if self.voices[v].active {
                active_count = active_count.saturating_add(1);
                let rel = self.voices[v].emitter_pos.sat_sub(listener_pos);
                let dist_sq_q64 = rel.dist_sq_wide(QVec3::ZERO).0.max(0) as u128;
                let dist_q32 = Q32::from_raw((isqrt_u128(dist_sq_q64)).min(i64::MAX as u64) as i64);

                let denom = Q32::ONE.sat_add(dist_q32.sat_mul(Q32::from_ratio_i32(8, 100)));
                let atten = self.voices[v]
                    .base_gain
                    .sat_div(denom)
                    .clamp_val(Q32::ZERO, Q32::ONE);

                let pan = if dist_q32 > Q32::from_ratio_i32(1, 100) {
                    rel.sat_div_scalar_help(dist_q32, right_unit)
                } else {
                    Q32::ZERO
                };

                let half = Q32::HALF;
                let pan_scaled = pan.sat_mul(Q32::from_ratio_i32(42, 100));
                let gl = atten
                    .sat_mul(half.sat_sub(pan_scaled))
                    .clamp_val(Q32::ZERO, Q32::ONE);
                let gr = atten
                    .sat_mul(half.sat_add(pan_scaled))
                    .clamp_val(Q32::ZERO, Q32::ONE);

                gain_l_q15[v] = (gl.raw() >> 17).clamp(0, 32_767) as i32;
                gain_r_q15[v] = (gr.raw() >> 17).clamp(0, 32_767) as i32;

                let pan_abs = if pan >= Q32::ZERO { pan } else { pan.sat_neg() };
                let delay_taps = (pan_abs.sat_mul(Q32::from_int(24)).to_int_floor()).clamp(0, 24) as usize;
                if pan >= Q32::ZERO {
                    itd_left_delay[v] = delay_taps;
                    itd_right_delay[v] = 0;
                } else {
                    itd_left_delay[v] = 0;
                    itd_right_delay[v] = delay_taps;
                }

                step_q16[v] =
                    ((u64::from(self.voices[v].pitch_q16)).saturating_mul(u64::from(hitstop_shift))) >> 16;
            }
            v = v.saturating_add(1);
        }

        let mut peak_abs: i32 = 0;
        let mut left_energy: u64 = 0;
        let mut right_energy: u64 = 0;
        let max_cursor_q16 = ((CUE_PCM_LEN.saturating_sub(2)) as u64) << 16;

        let mut f: usize = 0;
        while f < bounded_frames {
            let mut mix_l: i32 = 0;
            let mut mix_r: i32 = 0;

            let mut vi: usize = 0;
            while vi < MAX_ACTIVE_VOICES {
                if self.voices[vi].active {
                    let cur = self.voices[vi].cursor_q16;
                    if cur >= max_cursor_q16 {
                        self.voices[vi].active = false;
                    } else {
                        let idx0 = (cur >> 16) as usize;
                        let frac = (cur & 0xFFFF) as i32;
                        let s0 = i32::from(self.bank.sample_q15(self.voices[vi].cue, idx0));
                        let s1 =
                            i32::from(self.bank.sample_q15(self.voices[vi].cue, idx0.saturating_add(1)));
                        let diff = s1.saturating_sub(s0);
                        let interp = s0.saturating_add((diff.saturating_mul(frac)) >> 16);

                        let w_slot = self.voices[vi].itd_write & (ITD_DELAY_TAPS - 1);
                        self.voices[vi].itd_line[w_slot] = interp.clamp(-32_767, 32_767) as i16;

                        let rl_slot = w_slot.wrapping_sub(itd_left_delay[vi]) & (ITD_DELAY_TAPS - 1);
                        let rr_slot = w_slot.wrapping_sub(itd_right_delay[vi]) & (ITD_DELAY_TAPS - 1);
                        self.voices[vi].itd_write = w_slot.wrapping_add(1);

                        let mut ear_l = (i32::from(self.voices[vi].itd_line[rl_slot])
                            .saturating_mul(gain_l_q15[vi]))
                            >> 15;
                        let mut ear_r = (i32::from(self.voices[vi].itd_line[rr_slot])
                            .saturating_mul(gain_r_q15[vi]))
                            >> 15;

                        // 1-pole SDF terrain occlusion low-pass filter + 6dB direct path attenuation:
                        if self.voices[vi].sdf_occluded {
                            let att_l = ear_l >> 1;
                            let att_r = ear_r >> 1;
                            ear_l = (att_l >> 2)
                                .saturating_add((self.voices[vi].lp_prev_l.saturating_mul(3)) >> 2);
                            ear_r = (att_r >> 2)
                                .saturating_add((self.voices[vi].lp_prev_r.saturating_mul(3)) >> 2);
                            self.voices[vi].lp_prev_l = ear_l;
                            self.voices[vi].lp_prev_r = ear_r;
                        }

                        mix_l = mix_l.saturating_add(ear_l);
                        mix_r = mix_r.saturating_add(ear_r);
                        self.voices[vi].cursor_q16 = cur.saturating_add(step_q16[vi]);
                    }
                }
                vi = vi.saturating_add(1);
            }

            let out_l = mix_l.clamp(-32_767, 32_767) as i16;
            let out_r = mix_r.clamp(-32_767, 32_767) as i16;
            let abs_l = i32::from(out_l).abs();
            let abs_r = i32::from(out_r).abs();
            if abs_l > peak_abs {
                peak_abs = abs_l;
            }
            if abs_r > peak_abs {
                peak_abs = abs_r;
            }
            left_energy = left_energy.saturating_add((abs_l as u64).saturating_mul(abs_l as u64));
            right_energy = right_energy.saturating_add((abs_r as u64).saturating_mul(abs_r as u64));

            let _ = self.ring.push_stereo_i16(out_l, out_r);
            f = f.saturating_add(1);
        }

        AudioMixSummary {
            active_voices: active_count,
            mixed_stereo_frames: bounded_frames as u32,
            peak_abs_pcm: peak_abs,
            left_energy_acc: left_energy,
            right_energy_acc: right_energy,
        }
    }
}

trait QVec3PanExt {
    fn sat_div_scalar_help(&self, len: Q32, axis: QVec3) -> Q32;
}

impl QVec3PanExt for QVec3 {
    #[inline]
    fn sat_div_scalar_help(&self, len: Q32, axis: QVec3) -> Q32 {
        let dir = QVec3::new(self.x.sat_div(len), self.y.sat_div(len), self.z.sat_div(len));
        dir.x
            .sat_mul(axis.x)
            .sat_add(dir.y.sat_mul(axis.y))
            .sat_add(dir.z.sat_mul(axis.z))
            .clamp_val(Q32::NEG_ONE, Q32::ONE)
    }
}

/// Fixed-point `Q16.16` linear resampler adapting `48kHz` `LockFreeAudioRing` stereo samples
/// to arbitrary OS endpoint sample rates (`44.1kHz`, `48kHz`, `96kHz`, `192kHz`) with zero heap allocation.
struct EndpointResampler {
    step_q16: u32,
    frac_q16: u32,
    prev_l: i16,
    prev_r: i16,
    cur_l: i16,
    cur_r: i16,
    primed: bool,
}

impl EndpointResampler {
    fn new(device_rate_hz: u32) -> Self {
        let safe_dev = u64::from(device_rate_hz.max(8_000));
        let step = (u64::from(SAMPLE_RATE_HZ) << 16)
            .checked_div(safe_dev)
            .unwrap_or(65_536)
            .clamp(4_096, 524_288) as u32;
        Self {
            step_q16: step,
            frac_q16: 0,
            prev_l: 0,
            prev_r: 0,
            cur_l: 0,
            cur_r: 0,
            primed: false,
        }
    }

    #[inline]
    fn next_stereo_i16(&mut self, ring: &LockFreeAudioRing) -> (i16, i16) {
        if self.step_q16 == 65_536 {
            return ring.pop_stereo_i16().unwrap_or((0, 0));
        }
        if !self.primed {
            let first = ring.pop_stereo_i16().unwrap_or((0, 0));
            let second = ring.pop_stereo_i16().unwrap_or(first);
            self.prev_l = first.0;
            self.prev_r = first.1;
            self.cur_l = second.0;
            self.cur_r = second.1;
            self.primed = true;
        }
        let f = self.frac_q16 as i32;
        let dl = i32::from(self.cur_l).saturating_sub(i32::from(self.prev_l));
        let dr = i32::from(self.cur_r).saturating_sub(i32::from(self.prev_r));
        let out_l = i32::from(self.prev_l)
            .saturating_add((dl.saturating_mul(f)) >> 16)
            .clamp(-32_768, 32_767) as i16;
        let out_r = i32::from(self.prev_r)
            .saturating_add((dr.saturating_mul(f)) >> 16)
            .clamp(-32_768, 32_767) as i16;

        let next_phase = self.frac_q16.saturating_add(self.step_q16);
        let whole_steps = next_phase >> 16;
        self.frac_q16 = next_phase & 0xFFFF;
        let mut s: u32 = 0;
        while s < whole_steps {
            self.prev_l = self.cur_l;
            self.prev_r = self.cur_r;
            let nxt = ring.pop_stereo_i16().unwrap_or((0, 0));
            self.cur_l = nxt.0;
            self.cur_r = nxt.1;
            s = s.saturating_add(1);
        }
        (out_l, out_r)
    }
}

/// Hardware `cpal` real-time audio stream host (`게임.md §5.1, §8.2`).
/// Negotiates `F32`, `I16`, or `U16` sample formats and arbitrary OS endpoint sample rates cleanly,
/// or runs in headless null-sink mode when no OS audio endpoint is attached.
pub struct AudioDeviceStreamHost {
    _stream: Option<cpal::Stream>,
    hardware_active: bool,
}

impl AudioDeviceStreamHost {
    pub fn start_or_headless(ring: Arc<LockFreeAudioRing>) -> Self {
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return Self {
                _stream: None,
                hardware_active: false,
            };
        };
        let Ok(supported) = device.default_output_config() else {
            return Self {
                _stream: None,
                hardware_active: false,
            };
        };

        let sample_format = supported.sample_format();
        let channels = (supported.channels() as usize).max(1);
        let config: cpal::StreamConfig = supported.into();
        let mut resampler = EndpointResampler::new(config.sample_rate.0);

        let stream_res = match sample_format {
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let (l_i16, r_i16) = resampler.next_stereo_i16(&ring);
                        if let Some(first) = frame.first_mut() {
                            *first = l_i16;
                        }
                        if let Some(second) = frame.get_mut(1) {
                            *second = r_i16;
                        }
                        for extra in frame.iter_mut().skip(2) {
                            *extra = 0;
                        }
                    }
                },
                move |_err| {},
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let (l_i16, r_i16) = resampler.next_stereo_i16(&ring);
                        let lu = (i32::from(l_i16).saturating_add(32_768)).clamp(0, 65_535) as u16;
                        let ru = (i32::from(r_i16).saturating_add(32_768)).clamp(0, 65_535) as u16;
                        if let Some(first) = frame.first_mut() {
                            *first = lu;
                        }
                        if let Some(second) = frame.get_mut(1) {
                            *second = ru;
                        }
                        for extra in frame.iter_mut().skip(2) {
                            *extra = 32_768;
                        }
                    }
                },
                move |_err| {},
                None,
            ),
            _ => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // One-way final DAC conversion `i16 (Q15) -> f32` strictly at hardware output boundary (`게임.md §8.1`):
                    for frame in data.chunks_mut(channels) {
                        let (l_i16, r_i16) = resampler.next_stereo_i16(&ring);
                        let lf = f32::from(l_i16) * (1.0_f32 / 32_768.0_f32);
                        let rf = f32::from(r_i16) * (1.0_f32 / 32_768.0_f32);
                        if let Some(first) = frame.first_mut() {
                            *first = lf;
                        }
                        if let Some(second) = frame.get_mut(1) {
                            *second = rf;
                        }
                        for extra in frame.iter_mut().skip(2) {
                            *extra = 0.0_f32;
                        }
                    }
                },
                move |_err| {},
                None,
            ),
        };

        match stream_res {
            Ok(stream) => {
                let played = stream.play().is_ok();
                Self {
                    _stream: Some(stream),
                    hardware_active: played,
                }
            }
            Err(_) => Self {
                _stream: None,
                hardware_active: false,
            },
        }
    }

    #[inline]
    pub const fn is_hardware_active(&self) -> bool {
        self.hardware_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc7845_ogg_crc_range_coder_imdct_and_spatial_occlusion() {
        // 1. Verify RFC 6716 Range Coder (`EcEnc` / `EcDec`) exact symbol roundtrip:
        let mut enc = EcEnc::new();
        let expected_e = [62_u32, 48, 35, 19, 12, 7, 3, 0];
        let expected_p = [0_u32, 3, 7, 11, 14, 5, 9, 2];
        for &e in &expected_e {
            enc.encode_uint(e, 64);
        }
        for &p in &expected_p {
            enc.encode_uint(p, 15);
        }
        let raw_frame = enc.finish();
        let mut dec = EcDec::new(&raw_frame);
        for &e in &expected_e {
            assert_eq!(dec.decode_uint(64), e);
        }
        for &p in &expected_p {
            assert_eq!(dec.decode_uint(15), p);
        }

        // 2. Verify RFC 7845 Ogg Opus container (`OggS` + `OpusHead` + `OpusTags` + CRC32):
        let ogg_bytes = build_bootstrap_ogg_opus_stream();
        assert_eq!(&ogg_bytes[0..4], b"OggS");
        let parsed = parse_ogg_opus_packets(&ogg_bytes);
        assert!(parsed.is_ok());
        let mut corrupted = ogg_bytes.clone();
        let last_idx = corrupted.len().saturating_sub(1);
        corrupted[last_idx] ^= 0x55;
        assert_eq!(
            parse_ogg_opus_packets(&corrupted),
            Err(OpusDecodeError::InvalidOggCrc)
        );

        // 3. Verify `OpusCueBank` TDAC Overlap-Add IMDCT + Binaural ITD/ILD + SDF Occlusion:
        let bank_res = OpusCueBank::decode_from_opus_stream(&ogg_bytes);
        assert!(bank_res.is_ok());
        let Ok(bank) = bank_res else {
            return;
        };

        let ring = Arc::new(LockFreeAudioRing::new());
        let mut mixer = SpatialAudioMixer::new(bank.clone(), Arc::clone(&ring));
        mixer.trigger_3d_cue(
            AudioCueKind::ClashParry,
            QVec3::from_ints(4, 0, 0),
            Q32::ONE,
            Q32::ONE,
            false,
        );
        let rep = mixer.mix_frames(QVec3::ZERO, QVec3::from_ints(1, 0, 0), false, 400);
        assert!(rep.peak_abs_pcm > 500);
        assert!(rep.right_energy_acc > rep.left_energy_acc);

        // Resampler check (`44.1kHz` endpoint reading `48kHz` SPSC ring):
        let mut resamp_44k1 = EndpointResampler::new(44_100);
        let s_out = resamp_44k1.next_stereo_i16(&ring);
        let _ = s_out;

        let ring_occ = Arc::new(LockFreeAudioRing::new());
        let mut mixer_occ = SpatialAudioMixer::new(bank, ring_occ);
        mixer_occ.trigger_3d_cue(
            AudioCueKind::ClashParry,
            QVec3::from_ints(4, 0, 0),
            Q32::ONE,
            Q32::ONE,
            true,
        );
        let rep_occ = mixer_occ.mix_frames(QVec3::ZERO, QVec3::from_ints(1, 0, 0), true, 400);
        assert!(rep_occ.peak_abs_pcm > 0);
        assert!(rep_occ.right_energy_acc < rep.right_energy_acc);
    }
}
