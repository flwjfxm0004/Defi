//! Hardware Bit-Isomorphic Blackwell `NVFP4` Quantizer (`SSOT-NVFP4 v14.0`).
//!
//! 100% IEEE-754 / NVIDIA SM120 PTX `cvt.rn.satfinite.e2m1x2` + `.ue4m3` Exact RNE (Round-to-Nearest-Even):
//!   - E2M1 positive grid: `{0.0 [0], 0.5 [1], 1.0 [2], 1.5 [3], 2.0 [4], 3.0 [5], 4.0 [6], 6.0 [7]}`
//!     Exact midpoint ties (`0.25, 0.75, 1.25, 1.75, 2.5, 3.5, 5.0`) ALWAYS round to EVEN grid index (`0, 2, 4, 6`)!
//!   - Block-16 unsigned `.ue4m3` scale (bias=7, min subnormal `2^-9 = 1/512`, min normal `2^-6 = 1/64`, max `448.0`)
//!     using exact `round_ties_even()` matching CUDA `rintf`!

/// Exact IEEE-754 / SM120 PTX `cvt.rn.satfinite.e2m1x2` scalar quantization with ties-to-even!
#[inline(always)]
pub fn quantize_e2m1_rne(u: f32) -> f32 {
    if !u.is_finite() {
        return 0.0;
    }
    let sign = if u < 0.0 { -1.0f32 } else { 1.0f32 };
    let ax = u.abs();
    // Exact midpoint tie rules -> even index {0: 0.0, 2: 1.0, 4: 2.0, 6: 4.0}:
    let q = if ax <= 0.25 {
        0.0
    } else if ax < 0.75 {
        0.5
    } else if ax <= 1.25 {
        1.0
    } else if ax < 1.75 {
        1.5
    } else if ax <= 2.50 {
        2.0
    } else if ax < 3.50 {
        3.0
    } else if ax <= 5.00 {
        4.0
    } else {
        6.0
    };
    sign * q
}

/// Exact unsigned FP8 `.ue4m3` block-scale quantization (`[0.0, 448.0]`, min subnormal `2^-9`, RNE ties-to-even).
#[inline(always)]
pub fn quantize_ue4m3_rne(s_raw: f32) -> f32 {
    if !s_raw.is_finite() || s_raw <= 0.0 {
        return 0.0;
    }
    let clamped = s_raw.min(448.0);
    if clamped < (1.0 / 64.0) {
        // Subnormal regime: exact multiples of 2^-9 (`1/512`) with ties-to-even!
        (clamped * 512.0).round_ties_even() * (1.0 / 512.0)
    } else {
        let exp = clamped.log2().floor().clamp(-6.0, 8.0);
        let step = 2.0f32.powf(exp - 3.0);
        ((clamped / step).round_ties_even() * step).min(448.0)
    }
}

/// Hardware-isomorphic Blackwell NVFP4 (`E2M1 + block-16 UE4M3`) quantize-dequantize for deep STE training.
pub fn nvfp4_quantize_f32(values: &[f32], out: &mut [f32]) {
    let n = values.len().min(out.len());
    if n == 0 {
        return;
    }
    let mut a_max = 0.0f32;
    for &v in &values[..n] {
        let av = if v.is_finite() { v.abs() } else { 0.0 };
        if av > a_max {
            a_max = av;
        }
    }
    if a_max <= 1e-30 {
        out[..n].fill(0.0);
        return;
    }
    let s_global = (a_max / (448.0 * 6.0)).max(1e-30);
    let denom_block = 6.0 * s_global;

    let mut base = 0usize;
    while base < n {
        let end = (base + 16).min(n);
        let mut b_max = 0.0f32;
        for &v in &values[base..end] {
            let av = if v.is_finite() { v.abs() } else { 0.0 };
            if av > b_max {
                b_max = av;
            }
        }
        let s_block = quantize_ue4m3_rne(b_max / denom_block);
        if s_block <= 0.0 {
            out[base..end].fill(0.0);
        } else {
            let scale = s_global * s_block;
            let inv_scale = 1.0 / scale.max(1e-30);
            for i in base..end {
                let v = if values[i].is_finite() { values[i] } else { 0.0 };
                out[i] = quantize_e2m1_rne(v * inv_scale) * scale;
            }
        }
        base = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvfp4_f32_ssot_edge_cases_and_exact_rne_ties_to_even_verified() {
        let mut empty = Vec::new();
        nvfp4_quantize_f32(&[], &mut empty);
        assert!(empty.is_empty());

        // Verify ALL 7 exact E2M1 midpoint ties round to EVEN grid indices `{0.0, 1.0, 2.0, 4.0}`!
        assert_eq!(quantize_e2m1_rne(0.25), 0.0);
        assert_eq!(quantize_e2m1_rne(0.75), 1.0);
        assert_eq!(quantize_e2m1_rne(1.25), 1.0);
        assert_eq!(quantize_e2m1_rne(1.75), 2.0);
        assert_eq!(quantize_e2m1_rne(2.50), 2.0);
        assert_eq!(quantize_e2m1_rne(3.50), 4.0);
        assert_eq!(quantize_e2m1_rne(5.00), 4.0);
        assert_eq!(quantize_e2m1_rne(-0.75), -1.0);
        assert_eq!(quantize_e2m1_rne(-3.50), -4.0);

        // Verify `.ue4m3` subnormal floor (`2^-9 = 1/512`) and max clamp (`448.0`)!
        assert_eq!(quantize_ue4m3_rne(1.0 / 1024.0), 0.0); // tie between 0 (even) and 1/512 (odd) -> 0!
        assert_eq!(quantize_ue4m3_rne(3.0 / 1024.0), 2.0 / 512.0); // tie between 1/512 (odd) and 2/512 (even) -> 2/512!
        assert_eq!(quantize_ue4m3_rne(999.0), 448.0);

        let edge_in = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -0.0];
        let mut edge_out = [0.0f32; 5];
        nvfp4_quantize_f32(&edge_in, &mut edge_out);
        assert!(edge_out.iter().all(|v| v.is_finite()));

        let v = [1.0f32, 2.0, -3.0, 4.0, 6.0, -0.5, 0.0, 1.5];
        let mut q = [0.0f32; 8];
        nvfp4_quantize_f32(&v, &mut q);
        for (a, b) in v.iter().zip(q.iter()) {
            assert!((a - b).abs() <= 0.25, "NVFP4 error too high: {} vs {}", a, b);
        }
    }
}
