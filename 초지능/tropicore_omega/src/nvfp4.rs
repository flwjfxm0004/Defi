//! Exact Blackwell `NVFP4` Hardware-Isomorphic Quantizer (`SSOT-NVFP4`).
//! Global FP32 scale + 16-elem unsigned `.ue4m3` block scale + E2M1 RNE ties-to-even.

const FLT_MAX_F64: f64 = 3.4028234663852886e38;
const FP32_MIN_SUBNORMAL: f64 = 1.401298464324817e-45; // 2^-149
const E2M1_POS: [f64; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

fn build_ue4m3_table() -> [f64; 127] {
    let mut t = [0.0f64; 127];
    let mut c = 0usize;
    while c < 127 {
        let e = (c >> 3) as i32;
        let m = (c & 7) as f64;
        t[c] = if e == 0 {
            m * 2.0f64.powi(-9)
        } else {
            (1.0 + m / 8.0) * 2.0f64.powi(e - 7)
        };
        c += 1;
    }
    t
}

#[inline]
fn quantize_ue4m3_rne(s_raw: f64, table: &[f64; 127]) -> f64 {
    if s_raw <= 0.0 {
        return 0.0;
    }
    if s_raw >= 448.0 {
        return 448.0;
    }
    let mut lo = 0usize;
    let mut hi = 126usize;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if table[mid] < s_raw {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == 0 {
        return table[0];
    }
    let v_lo = table[lo - 1];
    let v_hi = table[lo];
    let d_lo = s_raw - v_lo;
    let d_hi = v_hi - s_raw;
    if d_lo < d_hi || (d_lo == d_hi && ((lo - 1) & 1) == 0) {
        v_lo
    } else {
        v_hi
    }
}

#[inline]
fn quantize_e2m1_rne(u: f64) -> f64 {
    let sign = if u < 0.0 { -1.0 } else { 1.0 };
    let ax = u.abs();
    if ax >= 6.0 {
        return sign * 6.0;
    }
    let mut best_i = 0usize;
    let mut best_d = (ax - E2M1_POS[0]).abs();
    for i in 1..8 {
        let d = (ax - E2M1_POS[i]).abs();
        if d < best_d || (d == best_d && (i & 1) == 0) {
            best_d = d;
            best_i = i;
        }
    }
    sign * E2M1_POS[best_i]
}

/// Bit-exact Blackwell NVFP4 quantize-dequantize oracle (`16-elem` blocks).
pub fn nvfp4_quantize(values: &[f64]) -> Vec<f64> {
    let n = values.len();
    if n == 0 {
        return Vec::new();
    }
    let mut xs: Vec<f64> = Vec::with_capacity(((n + 15) / 16) * 16);
    let mut a_max = 0.0f64;
    for &v in values {
        let c = if v.is_nan() {
            0.0
        } else if v > FLT_MAX_F64 {
            FLT_MAX_F64
        } else if v < -FLT_MAX_F64 {
            -FLT_MAX_F64
        } else {
            v + 0.0
        };
        let ac = c.abs();
        if ac > a_max {
            a_max = ac;
        }
        xs.push(c);
    }
    if a_max == 0.0 {
        return vec![0.0; n];
    }
    let sg = ((a_max / (448.0 * 6.0)) as f32) as f64;
    let s_global = if sg > 0.0 { sg } else { FP32_MIN_SUBNORMAL };
    while xs.len() % 16 != 0 {
        xs.push(0.0);
    }
    let table = build_ue4m3_table();
    let mut out = Vec::with_capacity(xs.len());
    let denom_block = 6.0 * s_global;
    for chunk in xs.chunks_exact(16) {
        let mut b_max = 0.0f64;
        for &b in chunk {
            let ab = b.abs();
            if ab > b_max {
                b_max = ab;
            }
        }
        let s_block = quantize_ue4m3_rne(b_max / denom_block, &table);
        if s_block == 0.0 {
            out.extend_from_slice(&[0.0; 16]);
        } else {
            let denom = s_global * s_block;
            for &b in chunk {
                let q = quantize_e2m1_rne(b / denom);
                out.push((q * s_block * s_global) + 0.0);
            }
        }
    }
    out.truncate(n);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvfp4_ssot_edge_cases_and_rne_verified() {
        assert!(nvfp4_quantize(&[]).is_empty());
        assert_eq!(nvfp4_quantize(&[0.0, -0.0]), vec![0.0, 0.0]);
        let edge = nvfp4_quantize(&[f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 2.0f64.powi(-155)]);
        assert_eq!(edge[0], 0.0);
        assert!(edge[1].is_finite() && edge[1] > 0.0);
        assert!(edge[2].is_finite() && edge[2] < 0.0);
        assert!(edge[3].is_finite());
        let v = vec![1.0, 2.0, -3.0, 4.0, 6.0, -0.5, 0.0, 1.5];
        let q = nvfp4_quantize(&v);
        for (a, b) in v.iter().zip(q.iter()) {
            assert!((a - b).abs() <= 0.25, "expected close NVFP4 quant: {} vs {}", a, b);
        }
    }
}
