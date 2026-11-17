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
use verifier_abi::{ContractViolation, SubtitleContract, Verified};

/// Decomposed Unicode Hangul syllable (`0xAC00..=0xD7A3`, `§4.2 ③`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HangulJamo {
    pub choseong: u8,
    pub jungseong: u8,
    pub jongseong: u8,
}

/// Exact algorithmic decomposition of a Unicode Hangul syllable block (`SBase = 0xAC00`).
pub const fn decompose_hangul_syllable(ch: char) -> Option<HangulJamo> {
    let code = ch as u32;
    if code < 0xAC00 || code > 0xD7A3 {
        return None;
    }
    let s_index = code.saturating_sub(0xAC00);
    let choseong = s_index.wrapping_div(588) as u8;
    let rem = s_index.wrapping_rem(588);
    let jungseong = rem.wrapping_div(28) as u8;
    let jongseong = rem.wrapping_rem(28) as u8;
    Some(HangulJamo {
        choseong,
        jungseong,
        jongseong,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point2Q32 {
    pub x: i64,
    pub y: i64,
}

impl Point2Q32 {
    #[inline]
    pub const fn new(x: Q32, y: Q32) -> Self {
        Self { x: x.0, y: y.0 }
    }

    #[inline]
    pub fn lerp(self, rhs: Self, t: Q32) -> Self {
        let dx = Q32(rhs.x).sat_sub(Q32(self.x)).sat_mul(t);
        let dy = Q32(rhs.y).sat_sub(Q32(self.y)).sat_mul(t);
        Self {
            x: Q32(self.x).sat_add(dx).0,
            y: Q32(self.y).sat_add(dy).0,
        }
    }

    #[inline]
    pub fn dist(self, rhs: Self) -> Q32 {
        let dx = Q32(self.x).sat_sub(Q32(rhs.x));
        let dy = Q32(self.y).sat_sub(Q32(rhs.y));
        dx.sat_mul(dx).sat_add(dy.sat_mul(dy)).sqrt()
    }
}

/// Analytic 2D Cubic Bézier stroke primitive (`§4.2 ③`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CubicBezier2D {
    pub p0: Point2Q32,
    pub p1: Point2Q32,
    pub p2: Point2Q32,
    pub p3: Point2Q32,
    pub half_width_q32: i64,
}

impl CubicBezier2D {
    /// De Casteljau evaluation at `t in [0, 1]` (`Q32.32`).
    pub fn eval(&self, t: Q32) -> Point2Q32 {
        let a = self.p0.lerp(self.p1, t);
        let b = self.p1.lerp(self.p2, t);
        let c = self.p2.lerp(self.p3, t);
        let d = a.lerp(b, t);
        let e = b.lerp(c, t);
        d.lerp(e, t)
    }

    /// Deterministic 16-subdivision signed distance field (`< 0` inside stroke).
    pub fn signed_distance(&self, sample: Point2Q32) -> Q32 {
        let mut min_d = Q32::MAX;
        let mut step: i32 = 0;
        while step <= 16 {
            let t = Q32::from_ratio(step as i64, 16);
            let pt = self.eval(t);
            let d = sample.dist(pt);
            if d.0 < min_d.0 {
                min_d = d;
            }
            step = step.saturating_add(1);
        }
        min_d.sat_sub(Q32(self.half_width_q32))
    }
}

/// Synthesizes procedural Hangul Jamo cubic Bézier strokes inside unit glyph cell `[0, 1]^2`.
pub fn synthesize_jamo_strokes(jamo: HangulJamo) -> Vec<CubicBezier2D> {
    let hw = Q32::from_milli(45).0;
    let mut strokes = Vec::with_capacity(4);
    let cho_offset = Q32::from_ratio((jamo.choseong.wrapping_rem(5)) as i64, 40);
    // Choseong primary stroke
    strokes.push(CubicBezier2D {
        p0: Point2Q32::new(Q32::from_milli(150).sat_add(cho_offset), Q32::from_milli(200)),
        p1: Point2Q32::new(Q32::from_milli(350), Q32::from_milli(180)),
        p2: Point2Q32::new(Q32::from_milli(450), Q32::from_milli(320)),
        p3: Point2Q32::new(Q32::from_milli(220), Q32::from_milli(480)),
        half_width_q32: hw,
    });
    // Jungseong vowel axis
    let is_vertical_vowel = jamo.jungseong.wrapping_rem(2) == 0;
    if is_vertical_vowel {
        strokes.push(CubicBezier2D {
            p0: Point2Q32::new(Q32::from_milli(720), Q32::from_milli(140)),
            p1: Point2Q32::new(Q32::from_milli(720), Q32::from_milli(340)),
            p2: Point2Q32::new(Q32::from_milli(720), Q32::from_milli(460)),
            p3: Point2Q32::new(Q32::from_milli(720), Q32::from_milli(620)),
            half_width_q32: hw,
        });
    } else {
        strokes.push(CubicBezier2D {
            p0: Point2Q32::new(Q32::from_milli(160), Q32::from_milli(580)),
            p1: Point2Q32::new(Q32::from_milli(380), Q32::from_milli(580)),
            p2: Point2Q32::new(Q32::from_milli(600), Q32::from_milli(580)),
            p3: Point2Q32::new(Q32::from_milli(820), Q32::from_milli(580)),
            half_width_q32: hw,
        });
    }
    // Optional Jongseong batchim stroke
    if jamo.jongseong != 0 {
        strokes.push(CubicBezier2D {
            p0: Point2Q32::new(Q32::from_milli(240), Q32::from_milli(740)),
            p1: Point2Q32::new(Q32::from_milli(420), Q32::from_milli(860)),
            p2: Point2Q32::new(Q32::from_milli(620), Q32::from_milli(860)),
            p3: Point2Q32::new(Q32::from_milli(760), Q32::from_milli(740)),
            half_width_q32: hw,
        });
    }
    strokes
}

/// Integer pixel axis-aligned bounding box `[x0, x1) × [y0, y1)` (`§3.1`, `§8.1`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectI32 {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl RectI32 {
    #[inline]
    pub const fn is_well_formed(self) -> bool {
        self.x0 < self.x1 && self.y0 < self.y1
    }
}

/// Pure ROI non-overlap predicate (`§3.1` & `§8.1` CDCL bit-blasting formal proof target).
/// Returns `true` iff both boxes are well-formed (`x0 < x1`, `y0 < y1`) and share `0` interior pixels.
#[inline]
pub const fn check_roi_non_overlap(sub_box: RectI32, protected_roi: RectI32) -> bool {
    if !sub_box.is_well_formed() || !protected_roi.is_well_formed() {
        return false;
    }
    sub_box.x1 <= protected_roi.x0
        || protected_roi.x1 <= sub_box.x0
        || sub_box.y1 <= protected_roi.y0
        || protected_roi.y1 <= sub_box.y0
}

/// Computes exact overlap area in square pixels between two boxes.
pub fn rect_overlap_area_px(a: RectI32, b: RectI32) -> u64 {
    if !a.is_well_formed() || !b.is_well_formed() {
        return 0;
    }
    let ix0 = a.x0.max(b.x0) as i64;
    let iy0 = a.y0.max(b.y0) as i64;
    let ix1 = a.x1.min(b.x1) as i64;
    let iy1 = a.y1.min(b.y1) as i64;
    let w = ix1.saturating_sub(ix0).max(0) as u64;
    let h = iy1.saturating_sub(iy0).max(0) as u64;
    w.saturating_mul(h)
}

/// Relative luminance contrast ratio `(Y_max + 0.05) / (Y_min + 0.05)` in `Q32.32` (`§3.1`).
pub fn relative_luminance_contrast_q32(fg_lum: Q32, bg_lum: Q32) -> Q32 {
    let offset = Q32::from_milli(50);
    let y_max = if fg_lum.0 >= bg_lum.0 { fg_lum } else { bg_lum };
    let y_min = if fg_lum.0 >= bg_lum.0 { bg_lum } else { fg_lum };
    y_max.sat_add(offset).sat_div(y_min.sat_add(offset))
}

/// Verifies a subtitle/HUD line against all `§3.1` invariants (`0px` ROI overlap, contrast `>= 7.0`, `<= 26` chars)
/// and seals it into a `SubtitleContract<Verified>`.
pub fn verify_subtitle_layout(
    cue_id: u32,
    text: &str,
    sub_box: RectI32,
    eye_and_impact_rois: &[RectI32],
    fg_lum: Q32,
    bg_lum: Q32,
) -> Result<SubtitleContract<Verified>, ContractViolation> {
    if !sub_box.is_well_formed() {
        return Err(ContractViolation::SubtitleRoiOrContrastViolation);
    }
    let mut total_overlap: u32 = 0;
    for &roi in eye_and_impact_rois {
        if !check_roi_non_overlap(sub_box, roi) {
            let area = rect_overlap_area_px(sub_box, roi);
            let add_px = if area == 0 {
                1_u32
            } else if area > (u32::MAX as u64) {
                u32::MAX
            } else {
                area as u32
            };
            total_overlap = total_overlap.saturating_add(add_px);
        }
    }
    let char_count_usize = text.chars().count();
    let char_count = if char_count_usize > (u16::MAX as usize) {
        u16::MAX
    } else {
        char_count_usize as u16
    };
    let contrast = relative_luminance_contrast_q32(fg_lum, bg_lum);
    SubtitleContract::new_unverified(cue_id, total_overlap, contrast, char_count).verify()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hangul_decomposition_and_sdf() {
        // '한' = U+D55C: choseong 18 ('ㅎ'), jungseong 0 ('ㅏ'), jongseong 4 ('ㄴ')
        let jamo = decompose_hangul_syllable('한');
        assert_eq!(
            jamo,
            Some(HangulJamo {
                choseong: 18,
                jungseong: 0,
                jongseong: 4
            })
        );
        if let Some(j) = jamo {
            let strokes = synthesize_jamo_strokes(j);
            assert_eq!(strokes.len(), 3);
            let d = strokes[0].signed_distance(Point2Q32::new(Q32::HALF, Q32::HALF));
            assert!(d.0 < Q32::ONE.0);
        }
    }

    #[test]
    fn test_roi_and_contrast_contract() {
        let sub = RectI32 {
            x0: 600,
            y0: 1750,
            x1: 3240,
            y1: 1920,
        };
        let eye_roi = RectI32 {
            x0: 1500,
            y0: 500,
            x1: 2300,
            y1: 950,
        };
        let ok = verify_subtitle_layout(
            1,
            "시간의 판돈을 전부 회수한다.",
            sub,
            &[eye_roi],
            Q32::ONE,
            Q32::from_milli(20),
        );
        assert!(ok.is_ok());

        let overlapping_roi = RectI32 {
            x0: 1800,
            y0: 1700,
            x1: 2100,
            y1: 1850,
        };
        let bad = verify_subtitle_layout(
            2,
            "충돌 자막",
            sub,
            &[overlapping_roi],
            Q32::ONE,
            Q32::from_milli(20),
        );
        assert_eq!(bad, Err(ContractViolation::SubtitleRoiOrContrastViolation));
    }
}
