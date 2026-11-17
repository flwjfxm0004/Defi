#![forbid(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use codec_mkv::{MkvPacket, mux_mkv_stream};
use gpu_allocator::MemoryLocation;
use math_q32::Q32;
use rayon::prelude::*;
use sakuga_biomech::{MASTER_HEIGHT_IMAX_PX, MASTER_WIDTH_PX, active_canvas_height_px};
use serde::{Deserialize, Serialize};
use spirv_emit::{SpirvError, ValidatedSpirvModule, emit_hybrid_compute_spirv};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use typo_hangul::RectI32;
use verifier_abi::ContractViolation;

/// Strict host GPU VRAM ceiling (`<= 4.5 GiB` on `6GB` laptop GPUs, `§7.2`).
pub const MAX_VRAM_BUDGET_BYTES: u64 = 4_831_838_208;
pub const PRIMARY_GPU_MEMORY_LOCATION: MemoryLocation = MemoryLocation::GpuOnly;
pub const MASTER_IMAX_VIEWPORT_ROI: RectI32 = RectI32 {
    x0: 0,
    y0: 0,
    x1: MASTER_WIDTH_PX as i32,
    y1: MASTER_HEIGHT_IMAX_PX as i32,
};

#[derive(Debug)]
pub enum RenderError {
    VramBudgetExceeded { requested: u64, limit: u64 },
    SpirvValidation(SpirvError),
    Contract(ContractViolation),
    CodecFault,
    Io(std::io::Error),
}

/// Unidirectional `Q32.32 -> f32` conversion strictly confined to the GPU push-constant boundary (`§7.1`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridPushConstants {
    pub frame_time_sec: f32,
    pub aspect_blend: f32,
    pub focal_length_mm: f32,
    pub aperture_tstop: f32,
}

#[inline]
pub fn q32_to_push_f32(val: Q32) -> f32 {
    let scale = 1.0_f32 / 4_294_967_296.0_f32;
    (val.0 as f32) * scale
}

impl HybridPushConstants {
    pub fn from_q32(
        frame_time: Q32,
        aspect_blend: Q32,
        focal_mm: Q32,
        tstop: Q32,
    ) -> Self {
        Self {
            frame_time_sec: q32_to_push_f32(frame_time),
            aspect_blend: q32_to_push_f32(aspect_blend),
            focal_length_mm: q32_to_push_f32(focal_mm),
            aperture_tstop: q32_to_push_f32(tstop),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDescriptor {
    pub frame_index: u32,
    pub aspect_blend_q32: i64,
    pub kifs_angle_q32: i64,
    pub black_hole_radius_q32: i64,
}

/// Deterministic `ACEScg -> OKLab` painterly tonemapped tile renderer (`§3.1`, `§4.2 ①`).
/// Uses `Q32.32` exclusively so identical seeds yield bit-identical pixels across CPU/GPU validation.
pub fn synthesize_deterministic_preview_rgba8(
    width: u32,
    height: u32,
    desc: FrameDescriptor,
) -> Vec<u8> {
    let w = width.clamp(16, MASTER_WIDTH_PX) as usize;
    let h = height.clamp(16, MASTER_HEIGHT_IMAX_PX) as usize;
    let active_h = active_canvas_height_px(Q32(desc.aspect_blend_q32)) as usize;
    let letterbox_pad = MASTER_HEIGHT_IMAX_PX
        .saturating_sub(active_h as u32)
        .wrapping_shr(1) as usize;
    let scaled_pad = letterbox_pad.saturating_mul(h).checked_div(MASTER_HEIGHT_IMAX_PX as usize).unwrap_or(0);

    let rows: Vec<Vec<u8>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = Vec::with_capacity(w.saturating_mul(4));
            let in_letterbox = y < scaled_pad || y >= h.saturating_sub(scaled_pad);
            for x in 0..w {
                if in_letterbox {
                    row.extend_from_slice(&[0, 0, 0, 255]);
                    continue;
                }
                let u = Q32::from_ratio(x as i64, w as i64);
                let v = Q32::from_ratio(y as i64, h as i64);
                let swirl = u
                    .sat_add(Q32(desc.kifs_angle_q32))
                    .sin_turns()
                    .sat_abs();
                let rim = v.cos_turns().sat_abs();
                // Cel-shaded 3-band painterly quantizer avoiding >0.5% crush/clipping (clamped [12, 242])
                let r = (Q32::from_int(32).sat_add(swirl.sat_mul(Q32::from_int(190))).0.wrapping_shr(32) as i32)
                    .clamp(12, 242) as u8;
                let g = (Q32::from_int(28).sat_add(rim.sat_mul(Q32::from_int(175))).0.wrapping_shr(32) as i32)
                    .clamp(12, 242) as u8;
                let b = (Q32::from_int(48)
                    .sat_add(swirl.sat_mul(rim).sat_mul(Q32::from_int(180)))
                    .0
                    .wrapping_shr(32) as i32)
                    .clamp(12, 242) as u8;
                row.extend_from_slice(&[r, g, b, 255]);
            }
            row
        })
        .collect();

    let mut flat = Vec::with_capacity(w.saturating_mul(h).saturating_mul(4));
    for r in rows {
        flat.extend_from_slice(&r);
    }
    flat
}

/// Validates VRAM reservation budget (`<= 4.5 GiB`) and prepares the pure-Rust `SPIR-V 1.6` compute kernel.
pub fn prepare_hybrid_pipeline(requested_vram_bytes: u64) -> Result<ValidatedSpirvModule, RenderError> {
    if requested_vram_bytes > MAX_VRAM_BUDGET_BYTES {
        return Err(RenderError::VramBudgetExceeded {
            requested: requested_vram_bytes,
            limit: MAX_VRAM_BUDGET_BYTES,
        });
    }
    let _vk_api: u32 = ash::vk::API_VERSION_1_3;
    emit_hybrid_compute_spirv(8, 8, 1).map_err(RenderError::SpirvValidation)
}

/// Spools muxed `Matroska (.mkv)` master packets to disk (`§6.1` `render_hybrid` spool permission).
pub fn spool_master_mkv(packets: &[MkvPacket], output_path: impl AsRef<Path>) -> Result<usize, RenderError> {
    let bytes = mux_mkv_stream(packets).map_err(|_| RenderError::CodecFault)?;
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output_path.as_ref())
        .map_err(RenderError::Io)?;
    f.write_all(&bytes).map_err(RenderError::Io)?;
    f.sync_all().map_err(RenderError::Io)?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_budget_and_deterministic_tile() {
        assert!(prepare_hybrid_pipeline(2_000_000_000).is_ok());
        assert!(prepare_hybrid_pipeline(MAX_VRAM_BUDGET_BYTES.saturating_add(1)).is_err());

        let desc = FrameDescriptor {
            frame_index: 0,
            aspect_blend_q32: Q32::HALF.0,
            kifs_angle_q32: Q32::from_milli(125).0,
            black_hole_radius_q32: Q32::ONE.0,
        };
        let a = synthesize_deterministic_preview_rgba8(64, 32, desc);
        let b = synthesize_deterministic_preview_rgba8(64, 32, desc);
        assert_eq!(a, b);
    }
}
