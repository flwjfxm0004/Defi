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

//! Zero-cost `#[repr(C)]` `PhantomData` typestate contracts (`§6.1`).
//! Unverified projection, unproven Novikov timeline mastering, unlocked beat-impact cuts,
//! vacuum sound leaks, ROI-overlapping subtitles, and sealed ledger mutations are rejected at compile time.

use core::marker::PhantomData;
use math_q32::Q32;

mod private {
    pub trait Sealed {}
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unverified;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verified;

impl private::Sealed for Unverified {}
impl private::Sealed for Verified {}

pub trait ContractState: private::Sealed {}
impl ContractState for Unverified {}
impl ContractState for Verified {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractViolation {
    SheetGeometryOrColorDrift,
    MannequinZeroMicroActing,
    TimelineParadoxOrOpenClueSink,
    BeatImpactPhaseUnlocked,
    VacuumAirborneSoundLeak,
    SubtitleRoiOrContrastViolation,
}

/// 1. Character anatomical & color sheet contract (`§3.1`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetContract<S: ContractState> {
    pub cast_id: u32,
    pub bone_ratio_error_q32: Q32,
    pub delta_e_ok_q32: Q32,
    pub au_velocity_var_q32: Q32,
    _state: PhantomData<S>,
}

impl SheetContract<Unverified> {
    #[inline]
    pub const fn new_unverified(
        cast_id: u32,
        bone_ratio_error_q32: Q32,
        delta_e_ok_q32: Q32,
        au_velocity_var_q32: Q32,
    ) -> Self {
        Self {
            cast_id,
            bone_ratio_error_q32,
            delta_e_ok_q32,
            au_velocity_var_q32,
            _state: PhantomData,
        }
    }

    pub fn verify(self) -> Result<SheetContract<Verified>, ContractViolation> {
        // delta_E_OK <= 0.02 in Q32.32 (round(0.02 * 2^32) = 85_899_345)
        const MAX_DELTA_E_OK: i64 = 85_899_345;
        if self.bone_ratio_error_q32.0 != 0 || self.delta_e_ok_q32.0 < 0 || self.delta_e_ok_q32.0 > MAX_DELTA_E_OK {
            return Err(ContractViolation::SheetGeometryOrColorDrift);
        }
        if self.au_velocity_var_q32.0 <= 0 {
            return Err(ContractViolation::MannequinZeroMicroActing);
        }
        Ok(SheetContract {
            cast_id: self.cast_id,
            bone_ratio_error_q32: self.bone_ratio_error_q32,
            delta_e_ok_q32: self.delta_e_ok_q32,
            au_velocity_var_q32: self.au_velocity_var_q32,
            _state: PhantomData,
        })
    }
}

/// Projects a character sheet into camera space. Rejects `SheetContract<Unverified>` at compile time:
/// ```compile_fail
/// use math_q32::Q32;
/// use verifier_abi::{SheetContract, project_verified_sheet};
/// let raw = SheetContract::new_unverified(1, Q32::ZERO, Q32::ZERO, Q32::ONE);
/// let _ = project_verified_sheet(&raw);
/// ```
#[inline]
pub const fn project_verified_sheet(sheet: &SheetContract<Verified>) -> u32 {
    sheet.cast_id
}

/// 2. Spacetime Novikov fixed-point & clue-sink closure contract (`§1.3`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimelineContract<S: ContractState> {
    pub episode_index: u16,
    pub paradox_count: u32,
    pub open_clue_sinks: u32,
    pub is_season_finale: bool,
    _state: PhantomData<S>,
}

impl TimelineContract<Unverified> {
    #[inline]
    pub const fn new_unverified(
        episode_index: u16,
        paradox_count: u32,
        open_clue_sinks: u32,
        is_season_finale: bool,
    ) -> Self {
        Self {
            episode_index,
            paradox_count,
            open_clue_sinks,
            is_season_finale,
            _state: PhantomData,
        }
    }

    pub const fn verify(self) -> Result<TimelineContract<Verified>, ContractViolation> {
        if self.paradox_count != 0 {
            return Err(ContractViolation::TimelineParadoxOrOpenClueSink);
        }
        if self.is_season_finale && self.open_clue_sinks != 0 {
            return Err(ContractViolation::TimelineParadoxOrOpenClueSink);
        }
        Ok(TimelineContract {
            episode_index: self.episode_index,
            paradox_count: self.paradox_count,
            open_clue_sinks: self.open_clue_sinks,
            is_season_finale: self.is_season_finale,
            _state: PhantomData,
        })
    }
}

/// Emits a master timeline. Rejects `TimelineContract<Unverified>` at compile time:
/// ```compile_fail
/// use verifier_abi::{TimelineContract, master_verified_timeline};
/// let raw = TimelineContract::new_unverified(12, 0, 0, true);
/// let _ = master_verified_timeline(&raw);
/// ```
#[inline]
pub const fn master_verified_timeline(tl: &TimelineContract<Verified>) -> u16 {
    tl.episode_index
}

/// 3. Half-frame audio beat-lock impact cut contract (`§3.2`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImpactCutContract<S: ContractState> {
    pub shot_id: u32,
    pub visual_time_q32: Q32,
    pub nearest_beat_time_q32: Q32,
    _state: PhantomData<S>,
}

impl ImpactCutContract<Unverified> {
    #[inline]
    pub const fn new_unverified(shot_id: u32, visual_time_q32: Q32, nearest_beat_time_q32: Q32) -> Self {
        Self {
            shot_id,
            visual_time_q32,
            nearest_beat_time_q32,
            _state: PhantomData,
        }
    }

    pub fn verify(self) -> Result<ImpactCutContract<Verified>, ContractViolation> {
        let phase_err = self.visual_time_q32.sat_sub(self.nearest_beat_time_q32).sat_abs();
        if phase_err.0 > Q32::HALF_DT_60HZ.0 {
            return Err(ContractViolation::BeatImpactPhaseUnlocked);
        }
        Ok(ImpactCutContract {
            shot_id: self.shot_id,
            visual_time_q32: self.visual_time_q32,
            nearest_beat_time_q32: self.nearest_beat_time_q32,
            _state: PhantomData,
        })
    }
}

/// Commits an impact cut. Rejects `ImpactCutContract<Unverified>` at compile time:
/// ```compile_fail
/// use math_q32::Q32;
/// use verifier_abi::{ImpactCutContract, commit_verified_impact_cut};
/// let raw = ImpactCutContract::new_unverified(7, Q32::ONE, Q32::ONE);
/// let _ = commit_verified_impact_cut(&raw);
/// ```
#[inline]
pub const fn commit_verified_impact_cut(cut: &ImpactCutContract<Verified>) -> u32 {
    cut.shot_id
}

/// 4. Space vacuum acoustic gate contract (`§3.3`: `P_ext == 0 => airborne <= -90 dBFS`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VacuumShotContract<S: ContractState> {
    pub shot_id: u32,
    pub external_pressure_pa: u32,
    pub airborne_level_dbfs: i32,
    _state: PhantomData<S>,
}

impl VacuumShotContract<Unverified> {
    #[inline]
    pub const fn new_unverified(shot_id: u32, external_pressure_pa: u32, airborne_level_dbfs: i32) -> Self {
        Self {
            shot_id,
            external_pressure_pa,
            airborne_level_dbfs,
            _state: PhantomData,
        }
    }

    pub const fn verify(self) -> Result<VacuumShotContract<Verified>, ContractViolation> {
        if self.external_pressure_pa == 0 && self.airborne_level_dbfs > -90 {
            return Err(ContractViolation::VacuumAirborneSoundLeak);
        }
        Ok(VacuumShotContract {
            shot_id: self.shot_id,
            external_pressure_pa: self.external_pressure_pa,
            airborne_level_dbfs: self.airborne_level_dbfs,
            _state: PhantomData,
        })
    }
}

/// Muxes a shot's audio track. Rejects `VacuumShotContract<Unverified>` at compile time:
/// ```compile_fail
/// use verifier_abi::{VacuumShotContract, mux_verified_vacuum_shot};
/// let raw = VacuumShotContract::new_unverified(3, 0, -96);
/// let _ = mux_verified_vacuum_shot(&raw);
/// ```
#[inline]
pub const fn mux_verified_vacuum_shot(shot: &VacuumShotContract<Verified>) -> u32 {
    shot.shot_id
}

/// 5. Subtitle ROI non-overlap & luminance contrast contract (`§3.1`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubtitleContract<S: ContractState> {
    pub cue_id: u32,
    pub roi_overlap_px: u32,
    pub contrast_ratio_q32: Q32,
    pub char_count: u16,
    _state: PhantomData<S>,
}

impl SubtitleContract<Unverified> {
    #[inline]
    pub const fn new_unverified(
        cue_id: u32,
        roi_overlap_px: u32,
        contrast_ratio_q32: Q32,
        char_count: u16,
    ) -> Self {
        Self {
            cue_id,
            roi_overlap_px,
            contrast_ratio_q32,
            char_count,
            _state: PhantomData,
        }
    }

    pub const fn verify(self) -> Result<SubtitleContract<Verified>, ContractViolation> {
        const MIN_CONTRAST_7_0: i64 = 7_i64 << 32;
        if self.roi_overlap_px != 0 || self.contrast_ratio_q32.0 < MIN_CONTRAST_7_0 || self.char_count > 26 {
            return Err(ContractViolation::SubtitleRoiOrContrastViolation);
        }
        Ok(SubtitleContract {
            cue_id: self.cue_id,
            roi_overlap_px: self.roi_overlap_px,
            contrast_ratio_q32: self.contrast_ratio_q32,
            char_count: self.char_count,
            _state: PhantomData,
        })
    }
}

/// Composites a verified subtitle glyph run. Rejects `SubtitleContract<Unverified>` at compile time:
/// ```compile_fail
/// use math_q32::Q32;
/// use verifier_abi::{SubtitleContract, composite_verified_subtitle};
/// let raw = SubtitleContract::new_unverified(1, 0, Q32::from_int(8), 12);
/// let _ = composite_verified_subtitle(&raw);
/// ```
#[inline]
pub const fn composite_verified_subtitle(sub: &SubtitleContract<Verified>) -> u32 {
    sub.cue_id
}

/// Contract 6: Immutable sealed ledger wrapper (`§4.3`, `§6.1`). Zero `&mut` accessors exist.
///
/// Attempting mutable access fails at compile time:
/// ```compile_fail
/// use verifier_abi::SealedLedger;
/// let mut sealed = SealedLedger::seal(42_u64, [0_u8; 32]);
/// let r: &mut u64 = sealed.as_mut();
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SealedLedger<T> {
    payload: T,
    blake3_digest: [u8; 32],
}

impl<T> SealedLedger<T> {
    #[inline]
    pub const fn seal(payload: T, blake3_digest: [u8; 32]) -> Self {
        Self {
            payload,
            blake3_digest,
        }
    }

    #[inline]
    pub const fn get(&self) -> &T {
        &self.payload
    }

    #[inline]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.blake3_digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contracts_accept_valid_and_reject_invalid() {
        let ok_sheet = SheetContract::new_unverified(1, Q32::ZERO, Q32::from_milli(15), Q32::ONE).verify();
        assert!(ok_sheet.is_ok());
        let bad_sheet = SheetContract::new_unverified(1, Q32::ONE, Q32::ZERO, Q32::ONE).verify();
        assert_eq!(bad_sheet, Err(ContractViolation::SheetGeometryOrColorDrift));

        let ok_tl = TimelineContract::new_unverified(12, 0, 0, true).verify();
        assert!(ok_tl.is_ok());
        let bad_tl = TimelineContract::new_unverified(12, 0, 1, true).verify();
        assert_eq!(bad_tl, Err(ContractViolation::TimelineParadoxOrOpenClueSink));

        let ok_cut = ImpactCutContract::new_unverified(1, Q32::ONE, Q32::ONE).verify();
        assert!(ok_cut.is_ok());
        let bad_cut = ImpactCutContract::new_unverified(1, Q32::ONE, Q32::TWO).verify();
        assert_eq!(bad_cut, Err(ContractViolation::BeatImpactPhaseUnlocked));

        let ok_vac = VacuumShotContract::new_unverified(2, 0, -95).verify();
        assert!(ok_vac.is_ok());
        let bad_vac = VacuumShotContract::new_unverified(2, 0, -40).verify();
        assert_eq!(bad_vac, Err(ContractViolation::VacuumAirborneSoundLeak));

        let ok_sub = SubtitleContract::new_unverified(3, 0, Q32::from_int(8), 20).verify();
        assert!(ok_sub.is_ok());
        let bad_sub = SubtitleContract::new_unverified(3, 4, Q32::from_int(8), 20).verify();
        assert_eq!(bad_sub, Err(ContractViolation::SubtitleRoiOrContrastViolation));
    }
}
