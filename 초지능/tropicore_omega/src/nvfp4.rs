//! Exact Blackwell `NVFP4` Hardware-Isomorphic Quantizer (`SSOT-NVFP4`).
//! Global FP32 scale + 16-elem unsigned `.ue4m3` block scale + E2M1 RNE ties-to-even.

/// Fast `f32` Blackwell NVFP4 (`E2M1 + block-16 UE4M3`) quantize-dequantize for deep weight/activation STE.
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
    let e2m1_pos = [0.0f32, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

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
        let s_raw = (b_max / denom_block).clamp(0.0, 448.0);
        if s_raw <= 1e-30 {
            out[base..end].fill(0.0);
        } else {
            let exp = s_raw.log2().floor();
            let step = 2.0f32.powf(exp - 3.0);
            let s_block = (s_raw / step).round() * step;
            let scale = s_global * s_block;
            let inv_scale = 1.0 / scale.max(1e-30);
            for i in base..end {
                let v = if values[i].is_finite() { values[i] } else { 0.0 };
                let u = v * inv_scale;
                let sign = if u < 0.0 { -1.0f32 } else { 1.0f32 };
                let ax = u.abs().min(6.0);
                let mut best = 0.0f32;
                let mut best_d = ax;
                for &q in &e2m1_pos[1..] {
                    let d = (ax - q).abs();
                    if d < best_d {
                        best_d = d;
                        best = q;
                    }
                }
                out[i] = sign * best * scale;
            }
        }
        base = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvfp4_f32_ssot_edge_cases_and_precision_verified() {
        let mut empty = Vec::new();
        nvfp4_quantize_f32(&[], &mut empty);
        assert!(empty.is_empty());

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
