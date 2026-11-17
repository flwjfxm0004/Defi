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

use core::cmp::Ordering;

/// Deterministic Q32.32 signed fixed-point scalar (`i64` backing, 32 fractional bits).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Q32(pub i64);

/// Wide Q64.64 intermediate accumulator (`i128` backing, 64 fractional bits).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Q64(pub i128);

impl Q32 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_i64 << 32);
    pub const TWO: Self = Self(2_i64 << 32);
    pub const HALF: Self = Self(1_i64 << 31);
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);
    /// ln(2) in Q32.32: round(0.6931471805599453 * 2^32) = 2_977_044_472
    pub const LN2: Self = Self(2_977_044_472);
    /// 1 / 60s frame period in Q32.32: floor(2^32 / 60) = 71_582_788
    pub const DT_60HZ: Self = Self(71_582_788);
    /// Half-frame phase-lock tolerance (1 / 120s) in Q32.32 = 35_791_394
    pub const HALF_DT_60HZ: Self = Self(35_791_394);

    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn from_int(val: i32) -> Self {
        Self((val as i64) << 32)
    }

    #[inline]
    pub fn from_milli(milli: i64) -> Self {
        Self::from_ratio(milli, 1_000)
    }

    #[inline]
    pub fn from_ratio(num: i64, den: i64) -> Self {
        Self( raw_sat_div_q32(num, den) )
    }

    #[inline]
    pub const fn sat_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn sat_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Total saturating negation (`i64::MIN` saturates to `i64::MAX`).
    #[inline]
    pub const fn sat_neg(self) -> Self {
        Self(self.0.saturating_neg())
    }

    #[inline]
    pub const fn sat_abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    #[inline]
    pub fn sat_mul(self, rhs: Self) -> Self {
        let wide = (self.0 as i128).saturating_mul(rhs.0 as i128);
        let shifted = wide >> 32;
        Self(clamp_i128_to_i64(shifted))
    }

    /// Total saturating Q32.32 division (`0 / 0 -> 0`, `x / 0 -> ±MAX`, `MIN / -1` safe).
    #[inline]
    pub fn sat_div(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            return match self.0.cmp(&0) {
                Ordering::Equal => Self::ZERO,
                Ordering::Greater => Self::MAX,
                Ordering::Less => Self::MIN,
            };
        }
        let num_wide = (self.0 as i128) << 32;
        let den_wide = rhs.0 as i128;
        let quot = num_wide.checked_div(den_wide).unwrap_or(i128::MAX);
        Self(clamp_i128_to_i64(quot))
    }

    /// Non-negative square root in Q32.32 (`x <= 0 -> 0`).
    #[inline]
    pub fn sqrt(self) -> Self {
        if self.0 <= 0 {
            return Self::ZERO;
        }
        let radicand = (self.0 as u128) << 32;
        let root = isqrt_u128(radicand);
        Self(clamp_u128_to_i64(root))
    }

    /// Deterministic fixed-point base-2 logarithm for `self > 0` (`self <= 0 -> MIN`).
    pub fn log2(self) -> Self {
        if self.0 <= 0 {
            return Self::MIN;
        }
        let mut x = self.0 as u128;
        let lz = x.leading_zeros();
        // Position of MSB in u128 is 127 - lz; Q32.32 unit is bit 32.
        let msb_pos = 127_i64.saturating_sub(lz as i64);
        let int_part = msb_pos.saturating_sub(32);

        // Normalize `x` into [1.0, 2.0) in Q32.32 (`[2^32, 2^33)`).
        if msb_pos > 32 {
            let shift = (msb_pos.saturating_sub(32)) as u32;
            x = x.wrapping_shr(shift);
        } else if msb_pos < 32 {
            let shift = (32_i64.saturating_sub(msb_pos)) as u32;
            x = x.wrapping_shl(shift);
        }

        // Iterative squaring bit extraction for 32 fractional bits.
        let mut frac: i64 = 0;
        let mut bit: i64 = 1_i64 << 31;
        while bit > 0 {
            // x is in [2^32, 2^33); x*x >> 32 is in [2^32, 2^34).
            x = x.saturating_mul(x).wrapping_shr(32);
            if x >= (2_u128 << 32) {
                x = x.wrapping_shr(1);
                frac = frac.saturating_add(bit);
            }
            bit = bit.wrapping_shr(1);
        }
        Self(int_part.wrapping_shl(32).saturating_add(frac))
    }

    /// Deterministic fixed-point `2^x` for Q32.32 input.
    pub fn exp2(self) -> Self {
        let int_part = self.0.wrapping_shr(32);
        if int_part >= 31 {
            return Self::MAX;
        }
        if int_part < -32 {
            return Self::ZERO;
        }
        let frac = (self.0 & 0xFFFF_FFFF) as u64;
        // 4th-order minimax/Taylor polynomial for 2^f on f in [0, 1):
        // 2^f ≈ 1 + c1*f + c2*f^2 + c3*f^3 + c4*f^4 in Q32.32
        let f = Self(frac as i64);
        let c1 = Self(2_977_044_472); // ln(2)
        let c2 = Self(1_031_777_941); // ln(2)^2 / 2
        let c3 = Self(238_389_867);   // ln(2)^3 / 6
        let c4 = Self(41_309_185);    // ln(2)^4 / 24
        let p = c3.sat_add(c4.sat_mul(f));
        let p = c2.sat_add(p.sat_mul(f));
        let p = c1.sat_add(p.sat_mul(f));
        let base = Self::ONE.sat_add(p.sat_mul(f));

        if int_part >= 0 {
            let shifted = (base.0 as i128).wrapping_shl(int_part as u32);
            Self(clamp_i128_to_i64(shifted))
        } else {
            let shift = int_part.saturating_neg() as u32;
            Self(base.0.wrapping_shr(shift))
        }
    }

    /// Deterministic natural exponential `e^x = 2^(x / ln 2)`.
    #[inline]
    pub fn exp(self) -> Self {
        self.sat_div(Self::LN2).exp2()
    }

    /// Sine of `turns` where `1.0` (`Q32::ONE`) = full period `2π` radians.
    pub fn sin_turns(self) -> Self {
        // Wrap fractional turns into [0, 1) -> [0, 2^32)
        let phase = (self.0 as u64) & 0xFFFF_FFFF;
        // Quadrant in 0..4, local u in [0, 2^30)
        let quadrant = (phase >> 30) & 3;
        let local = phase & 0x3FFF_FFFF;
        let mapped = if quadrant == 1 || quadrant == 3 {
            0x4000_0000_u64.saturating_sub(local)
        } else {
            local
        };
        // Normalize mapped in [0, 2^30] to x in [0, 1] in Q32.32
        let x = Self((mapped << 2) as i64);
        // Bhaskara / 5th-order odd polynomial for sin(π/2 * x) on [0, 1]:
        // c1 = π/2 ≈ 6_746_518_852, c3 = -(π/2)^3/6 ≈ -2_774_394_461, c5 = (π/2)^5/120 ≈ 342_276_935
        let x2 = x.sat_mul(x);
        let c1 = Self(6_746_518_852);
        let c3 = Self(-2_774_394_461);
        let c5 = Self(342_276_935);
        let poly = c1.sat_add(x2.sat_mul(c3.sat_add(x2.sat_mul(c5))));
        let mag = x.sat_mul(poly).0.clamp(0, Self::ONE.0);
        if quadrant >= 2 {
            Self(mag.saturating_neg())
        } else {
            Self(mag)
        }
    }

    #[inline]
    pub fn cos_turns(self) -> Self {
        self.sat_add(Self(1_i64 << 30)).sin_turns()
    }
}

#[inline]
fn raw_sat_div_q32(num: i64, den: i64) -> i64 {
    if den == 0 {
        return match num.cmp(&0) {
            Ordering::Equal => 0,
            Ordering::Greater => i64::MAX,
            Ordering::Less => i64::MIN,
        };
    }
    let wide = (num as i128) << 32;
    let quot = wide.checked_div(den as i128).unwrap_or(i128::MAX);
    clamp_i128_to_i64(quot)
}

#[inline]
pub const fn clamp_i128_to_i64(v: i128) -> i64 {
    if v > (i64::MAX as i128) {
        i64::MAX
    } else if v < (i64::MIN as i128) {
        i64::MIN
    } else {
        v as i64
    }
}

#[inline]
pub const fn clamp_u128_to_i64(v: u128) -> i64 {
    if v > (i64::MAX as u128) {
        i64::MAX
    } else {
        v as i64
    }
}

/// Exact `8-bit` tile decomposition of `64 × 64 -> 128` unsigned multiplication (`§8.1` reduction target).
pub fn mul_u64_tiled_8bit(a: u64, b: u64) -> u128 {
    let mut acc: u128 = 0;
    let mut i: u32 = 0;
    while i < 8 {
        let ai = (a.wrapping_shr(i.saturating_mul(8)) & 0xFF) as u16;
        let mut j: u32 = 0;
        while j < 8 {
            let bj = (b.wrapping_shr(j.saturating_mul(8)) & 0xFF) as u16;
            let prod_tile = ai.wrapping_mul(bj) as u128;
            let shift = i.saturating_add(j).saturating_mul(8);
            acc = acc.wrapping_add(prod_tile.wrapping_shl(shift));
            j = j.saturating_add(1);
        }
        i = i.saturating_add(1);
    }
    acc
}

/// Exact digit-by-digit shift-subtract integer square root satisfying `r^2 <= x < (r + 1)^2` for all `u128`.
pub const fn isqrt_u128(x: u128) -> u128 {
    let mut rem = x;
    let mut root: u128 = 0;
    let mut bit: u128 = 1_u128 << 126;

    while bit > rem {
        bit >>= 2;
    }

    while bit != 0 {
        let cand = root.wrapping_add(bit);
        if rem >= cand {
            rem = rem.wrapping_sub(cand);
            root = (root >> 1).wrapping_add(bit);
        } else {
            root >>= 1;
        }
        bit >>= 2;
    }
    root
}

/// Exact `-p * log2(p)` Shannon entropy term in bits (`Q32.32`) for `p = count / total`, with `0 * log2(0) := 0`.
pub fn shannon_term_bits(count: u64, total: u64) -> Q32 {
    if count == 0 || total == 0 || count >= total {
        return Q32::ZERO;
    }
    let p = Q32::from_ratio(count as i64, total as i64);
    let log_p = p.log2();
    p.sat_mul(log_p).sat_neg()
}

/// Deterministic 3D vector in Q32.32.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vec3Q32 {
    pub x: Q32,
    pub y: Q32,
    pub z: Q32,
}

impl Vec3Q32 {
    pub const ZERO: Self = Self {
        x: Q32::ZERO,
        y: Q32::ZERO,
        z: Q32::ZERO,
    };

    #[inline]
    pub const fn new(x: Q32, y: Q32, z: Q32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn sat_add(self, rhs: Self) -> Self {
        Self {
            x: self.x.sat_add(rhs.x),
            y: self.y.sat_add(rhs.y),
            z: self.z.sat_add(rhs.z),
        }
    }

    #[inline]
    pub const fn sat_sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.sat_sub(rhs.x),
            y: self.y.sat_sub(rhs.y),
            z: self.z.sat_sub(rhs.z),
        }
    }

    #[inline]
    pub fn scale(self, s: Q32) -> Self {
        Self {
            x: self.x.sat_mul(s),
            y: self.y.sat_mul(s),
            z: self.z.sat_mul(s),
        }
    }

    #[inline]
    pub const fn abs_components(self) -> Self {
        Self {
            x: self.x.sat_abs(),
            y: self.y.sat_abs(),
            z: self.z.sat_abs(),
        }
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> Q32 {
        let wx = (self.x.0 as i128).saturating_mul(rhs.x.0 as i128);
        let wy = (self.y.0 as i128).saturating_mul(rhs.y.0 as i128);
        let wz = (self.z.0 as i128).saturating_mul(rhs.z.0 as i128);
        let sum = wx.saturating_add(wy).saturating_add(wz) >> 32;
        Q32(clamp_i128_to_i64(sum))
    }

    #[inline]
    pub fn norm(self) -> Q32 {
        self.dot(self).sqrt()
    }
}

/// General-relativistic proper-time increment `dτ` (`-,+,+,+` Schwarzschild metric, `c = 1` normalized units)
/// Returns `(d_tau, is_causal_timelike_or_null)` for `§1.3` validation.
pub fn proper_time_step_schwarzschild(
    r: Q32,
    r_schwarzschild: Q32,
    dt: Q32,
    dr: Q32,
    v_tan: Q32,
) -> (Q32, bool) {
    if r.0 <= r_schwarzschild.0 || dt.0 < 0 {
        return (Q32::ZERO, false);
    }
    // f(r) = 1 - r_s / r
    let f_r = Q32::ONE.sat_sub(r_schwarzschild.sat_div(r));
    if f_r.0 <= 0 {
        return (Q32::ZERO, false);
    }
    // -ds^2 = f(r) dt^2 - dr^2 / f(r) - (v_tan * dt)^2
    let dt2 = dt.sat_mul(dt);
    let term_t = f_r.sat_mul(dt2);
    let term_r = dr.sat_mul(dr).sat_div(f_r);
    let d_arc = v_tan.sat_mul(dt);
    let term_ang = d_arc.sat_mul(d_arc);
    let neg_ds2 = term_t.sat_sub(term_r).sat_sub(term_ang);
    if neg_ds2.0 < 0 {
        // Spacelike violation of light-cone causality (ds^2 > 0)
        return (Q32::ZERO, false);
    }
    (neg_ds2.sqrt(), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saturation_extremes() {
        assert_eq!(Q32::MIN.sat_neg(), Q32::MAX);
        assert_eq!(Q32::MAX.sat_add(Q32::ONE), Q32::MAX);
        assert_eq!(Q32::MIN.sat_sub(Q32::ONE), Q32::MIN);
        assert_eq!(Q32::ONE.sat_div(Q32::ZERO), Q32::MAX);
        assert_eq!(Q32::ZERO.sat_div(Q32::ZERO), Q32::ZERO);
        assert_eq!(Q32::MIN.sat_div(Q32::from_int(-1)), Q32::MAX);
    }

    #[test]
    fn test_tiled_mul_and_isqrt() {
        let samples: [u64; 5] = [
            0,
            1,
            0xDEAD_BEEF_CAFE_BABE,
            u64::MAX.saturating_sub(1),
            u64::MAX,
        ];
        for &a in &samples {
            for &b in &samples {
                assert_eq!(mul_u64_tiled_8bit(a, b), (a as u128).wrapping_mul(b as u128));
            }
        }
        for &x in &[
            0_u128,
            1,
            2,
            3,
            4,
            15,
            16,
            (1_u128 << 64).saturating_add(7),
            u128::MAX,
        ] {
            let r = isqrt_u128(x);
            assert!(r.saturating_mul(r) <= x);
            if let Some(next_sq) = r.checked_add(1).and_then(|n| n.checked_mul(n)) {
                assert!(x < next_sq);
            }
        }
    }

    #[test]
    fn test_entropy_and_relativity() {
        // Fair coin: 2 * (-0.5 * log2(0.5)) = 1.0 bit
        let h_half = shannon_term_bits(1, 2);
        let h_total = h_half.sat_add(h_half);
        let diff = h_total.sat_sub(Q32::ONE).sat_abs();
        assert!(diff.0 < 16);
        assert_eq!(shannon_term_bits(0, 10), Q32::ZERO);

        // Relativistic time dilation near r = 2 * r_s: dτ < dt
        let (dtau, causal) = proper_time_step_schwarzschild(
            Q32::from_int(4),
            Q32::from_int(2),
            Q32::ONE,
            Q32::ZERO,
            Q32::ZERO,
        );
        assert!(causal);
        assert!(dtau.0 > 0 && dtau.0 < Q32::ONE.0);
    }
}
