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

use serde::{Deserialize, Serialize};

/// Fixed-point signed scalar `Q32.32` backed by `i64`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Serialize, Deserialize)]
pub struct Q32(pub i64);

/// Extended fixed-point signed accumulator `Q64.64` backed by `i128`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Serialize, Deserialize)]
pub struct Q64(pub i128);

/// 65-entry exact `<= 0.5 ULP` rounded quarter-wave sine LUT in `Q32.32` (`round(sin(pi * i / 128) * 2^32)`).
const QUARTER_SIN_Q32: [i64; 65] = [
    0,
    105_403_774,
    210_744_057,
    315_957_395,
    420_980_412,
    525_749_847,
    630_202_589,
    734_275_721,
    837_906_553,
    941_032_661,
    1_043_591_926,
    1_145_522_571,
    1_246_763_195,
    1_347_252_816,
    1_446_930_903,
    1_545_737_412,
    1_643_612_827,
    1_740_498_191,
    1_836_335_144,
    1_931_065_957,
    2_024_633_568,
    2_116_981_616,
    2_208_054_473,
    2_297_797_281,
    2_386_155_981,
    2_473_077_351,
    2_558_509_031,
    2_642_399_561,
    2_724_698_408,
    2_805_355_999,
    2_884_323_748,
    2_961_554_089,
    3_037_000_500,
    3_110_617_535,
    3_182_360_851,
    3_252_187_232,
    3_320_054_617,
    3_385_922_125,
    3_449_750_080,
    3_511_500_034,
    3_571_134_792,
    3_628_618_433,
    3_683_916_329,
    3_736_995_171,
    3_787_822_988,
    3_836_369_162,
    3_882_604_450,
    3_926_501_002,
    3_968_032_378,
    4_007_173_558,
    4_043_900_968,
    4_078_192_482,
    4_110_027_446,
    4_139_386_683,
    4_166_252_509,
    4_190_608_739,
    4_212_440_704,
    4_231_735_252,
    4_248_480_760,
    4_262_667_143,
    4_274_285_855,
    4_283_329_896,
    4_289_793_820,
    4_293_673_732,
    4_294_967_296,
];

/// `1 / (2 * pi)` in `Q32.32`.
const INV_TAU_Q32: i64 = 683_565_276;

impl Q32 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_i64 << 32);
    pub const HALF: Self = Self(1_i64 << 31);
    pub const NEG_ONE: Self = Self(-(1_i64 << 32));
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);
    /// `60Hz` server tick step `1/60 s` in `Q32.32`.
    pub const DT_60HZ: Self = Self(71_582_788);
    /// `120Hz` client sub-tick step `1/120 s` in `Q32.32`.
    pub const DT_120HZ: Self = Self(35_791_394);
    /// Maximum weapon swap completion time `0.05 s` in `Q32.32`.
    pub const MAX_SWAP_SEC: Self = Self(214_748_364);

    #[inline]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    #[inline]
    pub const fn from_int(v: i32) -> Self {
        Self((v as i64) << 32)
    }

    #[inline]
    pub const fn from_ratio_i32(num: i32, den: i32) -> Self {
        if den == 0 {
            if num == 0 {
                return Self::ZERO;
            } else if num > 0 {
                return Self::MAX;
            } else {
                return Self::MIN;
            }
        }
        let wide_num = (num as i128) << 32;
        let Some(q) = wide_num.checked_div(den as i128) else {
            return if num >= 0 { Self::MAX } else { Self::MIN };
        };
        Self::saturate_i128(q)
    }

    #[inline]
    pub const fn to_int_floor(self) -> i32 {
        (self.0 >> 32) as i32
    }

    #[inline]
    pub const fn saturate_i128(v: i128) -> Self {
        if v > (i64::MAX as i128) {
            Self::MAX
        } else if v < (i64::MIN as i128) {
            Self::MIN
        } else {
            Self(v as i64)
        }
    }

    #[inline]
    pub const fn sat_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn sat_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    #[inline]
    pub const fn sat_neg(self) -> Self {
        Self(self.0.saturating_neg())
    }

    #[inline]
    pub const fn sat_abs(self) -> Self {
        Self(self.0.saturating_abs())
    }

    #[inline]
    pub const fn clamp_val(self, lo: Self, hi: Self) -> Self {
        let (actual_lo, actual_hi) = if lo.0 <= hi.0 { (lo.0, hi.0) } else { (hi.0, lo.0) };
        if self.0 < actual_lo {
            Self(actual_lo)
        } else if self.0 > actual_hi {
            Self(actual_hi)
        } else {
            self
        }
    }

    #[inline]
    pub const fn mul_wide(self, rhs: Self) -> Q64 {
        Q64((self.0 as i128).saturating_mul(rhs.0 as i128))
    }

    #[inline]
    pub const fn sat_mul(self, rhs: Self) -> Self {
        let prod = (self.0 as i128).saturating_mul(rhs.0 as i128);
        Self::saturate_i128(prod >> 32)
    }

    /// Total saturating division: `0` divisor and `i64::MIN / -1` saturate without trapping.
    #[inline]
    pub const fn sat_div(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            if self.0 == 0 {
                return Self::ZERO;
            } else if self.0 > 0 {
                return Self::MAX;
            } else {
                return Self::MIN;
            }
        }
        let num = (self.0 as i128) << 32;
        match num.checked_div(rhs.0 as i128) {
            Some(q) => Self::saturate_i128(q),
            None => Self::MAX,
        }
    }

    #[inline]
    pub const fn sqrt_nonneg(self) -> Self {
        if self.0 <= 0 {
            return Self::ZERO;
        }
        let wide = (self.0 as u128) << 32;
        let r = isqrt_u128(wide);
        if r > (i64::MAX as u64) {
            Self::MAX
        } else {
            Self(r as i64)
        }
    }

    /// Deterministic LUT sine with `angle_turns` in `Q32.32` (`1.0 == 2*pi` radians).
    #[inline]
    pub const fn sin_turn(self) -> Self {
        let frac = (self.0 as u64) & 0xFFFF_FFFF;
        // Split [0, 2^32) into 4 quadrants (top 2 bits) and 64 segments per quadrant (next 6 bits).
        let quadrant = (frac >> 30) & 3;
        let in_quad = frac & 0x3FFF_FFFF; // 30 bits
        let seg = (in_quad >> 24) as usize; // 0..63
        let rem = (in_quad & 0x00FF_FFFF) as i128; // 24-bit interpolation weight

        let (idx0, idx1) = if quadrant == 0 || quadrant == 2 {
            (seg, seg.saturating_add(1))
        } else {
            (64_usize.saturating_sub(seg), 63_usize.saturating_sub(seg))
        };

        let y0 = QUARTER_SIN_Q32[idx0] as i128;
        let y1 = QUARTER_SIN_Q32[idx1] as i128;
        let diff = y1.saturating_sub(y0);
        let interp = y0.saturating_add((diff.saturating_mul(rem)) >> 24);
        let signed = if quadrant >= 2 { interp.saturating_neg() } else { interp };
        Self::saturate_i128(signed)
    }

    #[inline]
    pub const fn cos_turn(self) -> Self {
        let quarter = Self(1_i64 << 30);
        Self(self.0.wrapping_add(quarter.0)).sin_turn()
    }

    #[inline]
    pub const fn sin_rad(self) -> Self {
        self.sat_mul(Self(INV_TAU_Q32)).sin_turn()
    }

    #[inline]
    pub const fn cos_rad(self) -> Self {
        self.sat_mul(Self(INV_TAU_Q32)).cos_turn()
    }

    /// Deterministic endpoint-exact 5th-order polynomial `exp2` in `Q32.32` (`p(0)=1`, `p(1)=2`, saturates outside `[-32, 30]`).
    #[inline]
    pub const fn exp2_sat(self) -> Self {
        if self.0 <= -(32_i64 << 32) {
            return Self::ZERO;
        }
        if self.0 >= (31_i64 << 32) {
            return Self::MAX;
        }
        let int_part = self.0 >> 32;
        let frac = Self(self.0 & 0xFFFF_FFFF);
        // 5th-order endpoint-pinned polynomial for 2^f on f in [0, 1) (`c1+c2+c3+c4+c5 == 2^32`):
        let c1 = Self(2_977_044_472); // ln(2)
        let c2 = Self(1_031_765_476); // ln(2)^2 / 2
        let c3 = Self(238_387_485); // ln(2)^3 / 6
        let c4 = Self(41_309_253); // ln(2)^4 / 24
        let c5 = Self(6_460_610); // 2^32 - (c1 + c2 + c3 + c4) for C^0 continuity at integer boundaries
        let p = c5
            .sat_mul(frac)
            .sat_add(c4)
            .sat_mul(frac)
            .sat_add(c3)
            .sat_mul(frac)
            .sat_add(c2)
            .sat_mul(frac)
            .sat_add(c1)
            .sat_mul(frac)
            .sat_add(Self::ONE);
        if int_part >= 0 {
            let shifted = (p.0 as i128) << (int_part as u32);
            Self::saturate_i128(shifted)
        } else {
            let shift = int_part.saturating_neg() as u32;
            Self(p.0 >> shift)
        }
    }

    /// Promotes `Int8` weight in `[-128, 127]` (`Q1.7` normalized) losslessly to `Q32.32`.
    #[inline]
    pub const fn promote_i8_q7(w: i8) -> Self {
        Self((w as i64) << 25)
    }

    /// Promotes `Q16.16` compressed weight (`i32`) losslessly (`<< 16`) to `Q32.32`.
    #[inline]
    pub const fn promote_q16_16(w: i32) -> Self {
        Self((w as i64) << 16)
    }
}

impl Q64 {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn sat_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline]
    pub const fn sat_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Reduces `Q64.64` non-negative squared magnitude to `Q32.32` via exact `isqrt_u128`.
    #[inline]
    pub const fn sqrt_to_q32(self) -> Q32 {
        if self.0 <= 0 {
            return Q32::ZERO;
        }
        let r = isqrt_u128(self.0 as u128);
        if r > (i64::MAX as u64) {
            Q32::MAX
        } else {
            Q32(r as i64)
        }
    }
}

/// Exact shift-subtract integer square root `floor(sqrt(x))` over all `u128`.
/// Maintains linear step invariant `0 <= R_k <= 2 * Y_k` with zero multipliers.
#[inline]
pub const fn isqrt_u128(x: u128) -> u64 {
    let mut rem: u128 = 0;
    let mut root: u128 = 0;
    let mut step: u32 = 0;
    while step < 64 {
        let shift = 126_u32.saturating_sub(step.saturating_mul(2));
        let top2 = (x >> shift) & 3;
        let cand_rem = (rem << 2) | top2;
        let trial = (root << 2) | 1;
        if cand_rem >= trial {
            rem = cand_rem.wrapping_sub(trial);
            root = (root << 1) | 1;
        } else {
            rem = cand_rem;
            root <<= 1;
        }
        step = step.saturating_add(1);
    }
    root as u64
}

/// 3D Fixed-Point Vector (`Q32.32` components).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub struct QVec3 {
    pub x: Q32,
    pub y: Q32,
    pub z: Q32,
}

impl QVec3 {
    pub const ZERO: Self = Self { x: Q32::ZERO, y: Q32::ZERO, z: Q32::ZERO };
    pub const UP: Self = Self { x: Q32::ZERO, y: Q32::ONE, z: Q32::ZERO };

    #[inline]
    pub const fn new(x: Q32, y: Q32, z: Q32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn from_ints(x: i32, y: i32, z: i32) -> Self {
        Self {
            x: Q32::from_int(x),
            y: Q32::from_int(y),
            z: Q32::from_int(z),
        }
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
    pub const fn scale(self, s: Q32) -> Self {
        Self {
            x: self.x.sat_mul(s),
            y: self.y.sat_mul(s),
            z: self.z.sat_mul(s),
        }
    }

    #[inline]
    pub const fn fma(self, scale: Q32, bias: Self) -> Self {
        self.scale(scale).sat_add(bias)
    }

    /// Exact `Q64.64` inner product avoiding intermediate `Q32.32` truncation/overflow.
    #[inline]
    pub const fn dot_wide(self, rhs: Self) -> Q64 {
        self.x
            .mul_wide(rhs.x)
            .sat_add(self.y.mul_wide(rhs.y))
            .sat_add(self.z.mul_wide(rhs.z))
    }

    #[inline]
    pub const fn dot_q32(self, rhs: Self) -> Q32 {
        Q32::saturate_i128(self.dot_wide(rhs).0 >> 32)
    }

    #[inline]
    pub const fn len_sq_wide(self) -> Q64 {
        self.dot_wide(self)
    }

    #[inline]
    pub const fn dist_sq_wide(self, rhs: Self) -> Q64 {
        self.sat_sub(rhs).len_sq_wide()
    }

    #[inline]
    pub const fn length(self) -> Q32 {
        self.len_sq_wide().sqrt_to_q32()
    }

    #[inline]
    pub const fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len.0 <= 0 {
            Self::ZERO
        } else {
            Self {
                x: self.x.sat_div(len),
                y: self.y.sat_div(len),
                z: self.z.sat_div(len),
            }
        }
    }

    #[inline]
    pub const fn cross(self, rhs: Self) -> Self {
        let cx = self.y.mul_wide(rhs.z).sat_sub(self.z.mul_wide(rhs.y));
        let cy = self.z.mul_wide(rhs.x).sat_sub(self.x.mul_wide(rhs.z));
        let cz = self.x.mul_wide(rhs.y).sat_sub(self.y.mul_wide(rhs.x));
        Self {
            x: Q32::saturate_i128(cx.0 >> 32),
            y: Q32::saturate_i128(cy.0 >> 32),
            z: Q32::saturate_i128(cz.0 >> 32),
        }
    }
}

/// Deterministic SplitMix64 generator (`rng_state: u64`).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub struct DetRng(pub u64);

impl DetRng {
    #[inline]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform deterministic scalar in `[0, 1)` (`Q32.32`).
    #[inline]
    pub fn next_unit_q32(&mut self) -> Q32 {
        let bits = (self.next_u64() >> 32) as i64;
        Q32(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saturation_extremes() {
        assert_eq!(Q32::MIN.sat_neg(), Q32::MAX);
        assert_eq!(Q32::MIN.sat_abs(), Q32::MAX);
        assert_eq!(Q32::MIN.sat_div(Q32::from_raw(-1)), Q32::MAX);
        assert_eq!(Q32::ONE.sat_div(Q32::ZERO), Q32::MAX);
        assert_eq!(Q32::NEG_ONE.sat_div(Q32::ZERO), Q32::MIN);
        assert_eq!(Q32::ZERO.sat_div(Q32::ZERO), Q32::ZERO);
    }

    #[test]
    fn test_isqrt_u128_boundaries() {
        assert_eq!(isqrt_u128(0), 0);
        assert_eq!(isqrt_u128(1), 1);
        assert_eq!(isqrt_u128(3), 1);
        assert_eq!(isqrt_u128(4), 2);
        assert_eq!(isqrt_u128(u128::MAX), u64::MAX);
        let near_max = (u64::MAX as u128).wrapping_mul(u64::MAX as u128);
        assert_eq!(isqrt_u128(near_max), u64::MAX);
        assert_eq!(isqrt_u128(near_max.wrapping_sub(1)), u64::MAX.wrapping_sub(1));
    }

    #[test]
    fn test_trig_and_vec3_identities() {
        let mut i = 0_usize;
        while i < 64 {
            assert!(QUARTER_SIN_Q32[i] < QUARTER_SIN_Q32[i + 1]);
            i += 1;
        }
        assert_eq!(QUARTER_SIN_Q32[64], Q32::ONE.0);

        let mut k = 0_i32;
        while k < 1024 {
            let turn = Q32::from_ratio_i32(k, 1024);
            let s = turn.sin_turn();
            let c = turn.cos_turn();
            assert!(s >= Q32::NEG_ONE && s <= Q32::ONE);
            assert!(c >= Q32::NEG_ONE && c <= Q32::ONE);
            k += 1;
        }

        let s0 = Q32::ZERO.sin_turn();
        let c0 = Q32::ZERO.cos_turn();
        assert_eq!(s0, Q32::ZERO);
        assert_eq!(c0, Q32::ONE);

        // Verify exp2_sat endpoint continuity across [0, 1) -> 1:
        let just_below_one = Q32::from_raw(0xFFFF_FFFF);
        let at_one = Q32::ONE;
        let diff = at_one.exp2_sat().sat_sub(just_below_one.exp2_sat()).sat_abs();
        assert!(diff.0 < 32);

        let v = QVec3::from_ints(3, 4, 12);
        assert_eq!(v.length(), Q32::from_int(13));
    }
}
