#![no_std]
#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

extern crate alloc;

use alloc::vec::Vec;
use math_q32::Q32;
use serde::{Deserialize, Serialize};
use verifier_abi::{VacuumShotContract, Verified, mux_verified_vacuum_shot};

pub const EBML_HEADER_ID: u32 = 0x1A45_DFA3;

#[inline]
pub const fn verified_vacuum_shot_id(shot: &VacuumShotContract<Verified>) -> u32 {
    mux_verified_vacuum_shot(shot)
}
pub const MKV_SEGMENT_ID: u32 = 0x1853_8067;
pub const MKV_CLUSTER_ID: u32 = 0x1F43_B675;
pub const MKV_TIMECODE_ID: u8 = 0xE7;
pub const MKV_SIMPLE_BLOCK_ID: u8 = 0xA3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodecError {
    VintOverflow,
    UnexpectedEof,
    InvalidEbmlHeader,
    InvalidSegmentHeader,
    MalformedBlock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MkvPacket {
    pub track_number: u8,
    pub timestamp_ms: u32,
    pub is_keyframe: bool,
    pub payload: Vec<u8>,
}

/// Encodes an unsigned integer (`< 2^56 - 1`) as an `EBML` variable-size integer (`VINT`, `1..8` bytes).
pub fn encode_ebml_vint(value: u64) -> Result<Vec<u8>, CodecError> {
    let mut width: u32 = 1;
    while width <= 8 {
        let bits = width.saturating_mul(7);
        let limit = (1_u64.wrapping_shl(bits)).saturating_sub(1);
        if value < limit {
            let marker = 1_u64.wrapping_shl(bits);
            let combined = marker | value;
            let mut out = Vec::with_capacity(width as usize);
            let mut idx = width;
            while idx > 0 {
                idx = idx.saturating_sub(1);
                let shift = idx.saturating_mul(8);
                out.push((combined.wrapping_shr(shift) & 0xFF) as u8);
            }
            return Ok(out);
        }
        width = width.saturating_add(1);
    }
    Err(CodecError::VintOverflow)
}

/// Decodes an `EBML` `VINT` from the start of `bytes`, returning `(value, byte_length)`.
pub fn decode_ebml_vint(bytes: &[u8]) -> Result<(u64, usize), CodecError> {
    let Some(&first) = bytes.first() else {
        return Err(CodecError::UnexpectedEof);
    };
    if first == 0 {
        return Err(CodecError::VintOverflow);
    }
    let width = (first.leading_zeros() as usize).saturating_add(1);
    if width > 8 || bytes.len() < width {
        return Err(CodecError::UnexpectedEof);
    }
    let mask = if width == 8 {
        0_u8
    } else {
        0xFF_u8.wrapping_shr(width as u32)
    };
    let mut val = (first & mask) as u64;
    let mut i: usize = 1;
    while i < width {
        val = val.wrapping_shl(8) | (bytes[i] as u64);
        i = i.saturating_add(1);
    }
    Ok((val, width))
}

fn push_element_u32_id(out: &mut Vec<u8>, id: u32, payload: &[u8]) -> Result<(), CodecError> {
    out.extend_from_slice(&id.to_be_bytes());
    let vint = encode_ebml_vint(payload.len() as u64)?;
    out.extend_from_slice(&vint);
    out.extend_from_slice(payload);
    Ok(())
}

fn push_element_u8_id(out: &mut Vec<u8>, id: u8, payload: &[u8]) -> Result<(), CodecError> {
    out.push(id);
    let vint = encode_ebml_vint(payload.len() as u64)?;
    out.extend_from_slice(&vint);
    out.extend_from_slice(payload);
    Ok(())
}

/// Muxes video (`T.81 4:4:4`) and audio (`FLAC 24-bit`) packets into a canonical `EBML / Matroska (.mkv)` stream (`§4.2 ③`).
pub fn mux_mkv_stream(packets: &[MkvPacket]) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    // EBML Header payload: DocType (0x4282) = "matroska"
    let mut ebml_body = Vec::new();
    ebml_body.extend_from_slice(&[0x42, 0x82]);
    let doc_vint = encode_ebml_vint(8)?;
    ebml_body.extend_from_slice(&doc_vint);
    ebml_body.extend_from_slice(b"matroska");
    push_element_u32_id(&mut out, EBML_HEADER_ID, &ebml_body)?;

    // Segment body containing deterministic Clusters
    let mut seg_body = Vec::new();
    for pkt in packets {
        let mut cluster_body = Vec::new();
        push_element_u8_id(&mut cluster_body, MKV_TIMECODE_ID, &pkt.timestamp_ms.to_be_bytes())?;

        let mut sb = Vec::with_capacity(pkt.payload.len().saturating_add(4));
        let track_vint = encode_ebml_vint(pkt.track_number as u64)?;
        sb.extend_from_slice(&track_vint);
        // Relative timestamp = 0 (i16 BE)
        sb.extend_from_slice(&0_i16.to_be_bytes());
        sb.push(if pkt.is_keyframe { 0x80 } else { 0x00 });
        sb.extend_from_slice(&pkt.payload);
        push_element_u8_id(&mut cluster_body, MKV_SIMPLE_BLOCK_ID, &sb)?;

        push_element_u32_id(&mut seg_body, MKV_CLUSTER_ID, &cluster_body)?;
    }
    push_element_u32_id(&mut out, MKV_SEGMENT_ID, &seg_body)?;
    Ok(out)
}

/// Losslessly demuxes a `Matroska (.mkv)` stream produced by `mux_mkv_stream` (`§5.2 G1` verification).
pub fn demux_mkv_stream(bytes: &[u8]) -> Result<Vec<MkvPacket>, CodecError> {
    if bytes.len() < 8 {
        return Err(CodecError::UnexpectedEof);
    }
    let hdr_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if hdr_id != EBML_HEADER_ID {
        return Err(CodecError::InvalidEbmlHeader);
    }
    let (hdr_len, hdr_vlen) = decode_ebml_vint(&bytes[4..])?;
    let seg_pos = 4_usize
        .saturating_add(hdr_vlen)
        .saturating_add(hdr_len as usize);
    let Some(seg_hdr) = bytes.get(seg_pos..seg_pos.saturating_add(4)) else {
        return Err(CodecError::UnexpectedEof);
    };
    if u32::from_be_bytes([seg_hdr[0], seg_hdr[1], seg_hdr[2], seg_hdr[3]]) != MKV_SEGMENT_ID {
        return Err(CodecError::InvalidSegmentHeader);
    }
    let after_seg_id = seg_pos.saturating_add(4);
    let (seg_len, seg_vlen) = decode_ebml_vint(bytes.get(after_seg_id..).ok_or(CodecError::UnexpectedEof)?)?;
    let seg_start = after_seg_id.saturating_add(seg_vlen);
    let seg_end = seg_start.saturating_add(seg_len as usize);
    let Some(seg_slice) = bytes.get(seg_start..seg_end) else {
        return Err(CodecError::UnexpectedEof);
    };

    let mut packets = Vec::new();
    let mut pos: usize = 0;
    while pos < seg_slice.len() {
        let Some(cid) = seg_slice.get(pos..pos.saturating_add(4)) else {
            return Err(CodecError::UnexpectedEof);
        };
        if u32::from_be_bytes([cid[0], cid[1], cid[2], cid[3]]) != MKV_CLUSTER_ID {
            return Err(CodecError::MalformedBlock);
        }
        let c_after = pos.saturating_add(4);
        let (c_len, c_vlen) = decode_ebml_vint(seg_slice.get(c_after..).ok_or(CodecError::UnexpectedEof)?)?;
        let c_start = c_after.saturating_add(c_vlen);
        let c_end = c_start.saturating_add(c_len as usize);
        let Some(c_body) = seg_slice.get(c_start..c_end) else {
            return Err(CodecError::UnexpectedEof);
        };

        // Parse Timecode (0xE7) + SimpleBlock (0xA3)
        if c_body.first().copied() != Some(MKV_TIMECODE_ID) {
            return Err(CodecError::MalformedBlock);
        }
        let (tc_len, tc_vlen) = decode_ebml_vint(c_body.get(1..).ok_or(CodecError::UnexpectedEof)?)?;
        let tc_start = 1_usize.saturating_add(tc_vlen);
        let tc_end = tc_start.saturating_add(tc_len as usize);
        let Some(tc_bytes) = c_body.get(tc_start..tc_end) else {
            return Err(CodecError::UnexpectedEof);
        };
        if tc_bytes.len() != 4 {
            return Err(CodecError::MalformedBlock);
        }
        let ts_ms = u32::from_be_bytes([tc_bytes[0], tc_bytes[1], tc_bytes[2], tc_bytes[3]]);

        let sb_pos = tc_end;
        if c_body.get(sb_pos).copied() != Some(MKV_SIMPLE_BLOCK_ID) {
            return Err(CodecError::MalformedBlock);
        }
        let (sb_len, sb_vlen) = decode_ebml_vint(
            c_body
                .get(sb_pos.saturating_add(1)..)
                .ok_or(CodecError::UnexpectedEof)?,
        )?;
        let sb_start = sb_pos.saturating_add(1).saturating_add(sb_vlen);
        let sb_end = sb_start.saturating_add(sb_len as usize);
        let Some(sb_body) = c_body.get(sb_start..sb_end) else {
            return Err(CodecError::UnexpectedEof);
        };
        let (track_num, trk_vlen) = decode_ebml_vint(sb_body)?;
        let hdr_fixed_end = trk_vlen.saturating_add(3);
        let Some(flags_slice) = sb_body.get(trk_vlen..hdr_fixed_end) else {
            return Err(CodecError::MalformedBlock);
        };
        let is_key = (flags_slice[2] & 0x80) != 0;
        let payload = sb_body
            .get(hdr_fixed_end..)
            .ok_or(CodecError::MalformedBlock)?
            .to_vec();

        packets.push(MkvPacket {
            track_number: track_num as u8,
            timestamp_ms: ts_ms,
            is_keyframe: is_key,
            payload,
        });
        pos = c_end;
    }
    Ok(packets)
}

/// Pure fixed-point `ITU-T T.81` separable `8×8` forward DCT (`Q32.32`, zero floats, `§4.2 ③`).
pub fn fdct_8x8_q32(block: &[i32; 64]) -> [i32; 64] {
    let mut temp = [Q32::ZERO; 64];
    let half = Q32::HALF;
    let inv_sqrt2 = Q32::from_int(2).sqrt().sat_div(Q32::TWO);

    // Row 1D DCT
    let mut row: usize = 0;
    while row < 8 {
        let mut u: usize = 0;
        while u < 8 {
            let cu = if u == 0 { inv_sqrt2 } else { Q32::ONE };
            let mut sum = Q32::ZERO;
            let mut x: usize = 0;
            while x < 8 {
                let num = (x.saturating_mul(2).saturating_add(1)).saturating_mul(u) as i64;
                let angle_turns = Q32::from_ratio(num, 32);
                let val = Q32::from_int(block[row.saturating_mul(8).saturating_add(x)]);
                sum = sum.sat_add(val.sat_mul(angle_turns.cos_turns()));
                x = x.saturating_add(1);
            }
            temp[row.saturating_mul(8).saturating_add(u)] = sum.sat_mul(cu).sat_mul(half);
            u = u.saturating_add(1);
        }
        row = row.saturating_add(1);
    }

    // Column 1D DCT
    let mut out = [0_i32; 64];
    let mut col: usize = 0;
    while col < 8 {
        let mut v: usize = 0;
        while v < 8 {
            let cv = if v == 0 { inv_sqrt2 } else { Q32::ONE };
            let mut sum = Q32::ZERO;
            let mut y: usize = 0;
            while y < 8 {
                let num = (y.saturating_mul(2).saturating_add(1)).saturating_mul(v) as i64;
                let angle_turns = Q32::from_ratio(num, 32);
                let val = temp[y.saturating_mul(8).saturating_add(col)];
                sum = sum.sat_add(val.sat_mul(angle_turns.cos_turns()));
                y = y.saturating_add(1);
            }
            let coeff = sum.sat_mul(cv).sat_mul(half);
            out[v.saturating_mul(8).saturating_add(col)] = (coeff.0.wrapping_shr(32)) as i32;
            v = v.saturating_add(1);
        }
        col = col.saturating_add(1);
    }
    out
}

/// `RFC 9639 FLAC` order-2 fixed linear predictor (`e[n] = x[n] - 2*x[n-1] + x[n-2]`) for lossless `24-bit` audio (`§4.2 ③`).
pub fn flac_encode_order2_residuals(pcm24: &[i32]) -> Vec<i64> {
    let mut res = Vec::with_capacity(pcm24.len());
    for (idx, &sample) in pcm24.iter().enumerate() {
        let s = sample as i64;
        if idx < 2 {
            res.push(s);
        } else {
            let s1 = pcm24[idx.saturating_sub(1)] as i64;
            let s2 = pcm24[idx.saturating_sub(2)] as i64;
            let pred = s1.saturating_mul(2).saturating_sub(s2);
            res.push(s.saturating_sub(pred));
        }
    }
    res
}

/// Bit-exact reconstruction of `24-bit` PCM from `FLAC` order-2 residuals.
pub fn flac_decode_order2_residuals(residuals: &[i64]) -> Vec<i32> {
    let mut pcm = Vec::with_capacity(residuals.len());
    for (idx, &r) in residuals.iter().enumerate() {
        if idx < 2 {
            pcm.push(r as i32);
        } else {
            let s1 = pcm[idx.saturating_sub(1)] as i64;
            let s2 = pcm[idx.saturating_sub(2)] as i64;
            let pred = s1.saturating_mul(2).saturating_sub(s2);
            pcm.push(pred.saturating_add(r) as i32);
        }
    }
    pcm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mkv_mux_demux_and_flac_lossless() {
        let pkts = alloc::vec![
            MkvPacket {
                track_number: 1,
                timestamp_ms: 0,
                is_keyframe: true,
                payload: alloc::vec![10, 20, 30, 40],
            },
            MkvPacket {
                track_number: 2,
                timestamp_ms: 16,
                is_keyframe: true,
                payload: alloc::vec![99, 88, 77],
            },
        ];
        let stream = mux_mkv_stream(&pkts);
        assert!(stream.is_ok());
        if let Ok(bytes) = stream {
            let parsed = demux_mkv_stream(&bytes);
            assert_eq!(parsed, Ok(pkts));
        }

        let pcm = [0_i32, 12_345, 24_000, -15_432, -8_388_600, 8_388_607];
        let res = flac_encode_order2_residuals(&pcm);
        let rec = flac_decode_order2_residuals(&res);
        assert_eq!(rec.as_slice(), &pcm);
    }
}
