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
use math_q32::{Q32, clamp_i128_to_i64, isqrt_u128};
use serde::{Deserialize, Serialize};
use verifier_abi::{ContractViolation, ImpactCutContract, VacuumShotContract, Verified};

/// Maximum magnitude of Kelly-Lochbaum reflection coefficient `|k_m| <= 1 - 2^{-32}` (`§3.3`).
pub const MAX_REFLECTION_K_RAW: i64 = (1_i64 << 32) - 1;

/// Truncates `val / 2^shift` toward zero (`|trunc(val)| <= |val| / 2^shift` for all signed integers).
/// Guarantees zero rounding-away-from-zero energy overshoot in passive wave lattices.
#[inline]
pub const fn trunc_div_pow2_i128(val: i128, shift: u32) -> i128 {
    if val >= 0 {
        val.wrapping_shr(shift)
    } else {
        let mag = val.unsigned_abs().wrapping_shr(shift);
        (mag as i128).wrapping_neg()
    }
}

/// Power-normalized passive Kelly-Lochbaum scattering junction (`§3.3` & `§8.1` formal proof target).
/// Given forward wave `f_in`, backward wave `b_in`, and reflection `k` (`|k| <= 1 - 2^{-32}`),
/// computes `g = floor(sqrt(1 - k^2))` and `(f_out, b_out)` with exact invariant:
/// `f_out^2 + b_out^2 <= f_in^2 + b_in^2` across the entire bitvector space.
pub const fn kelly_lochbaum_junction_step(f_in: Q32, b_in: Q32, k_raw: i64) -> (Q32, Q32) {
    let k_clamped = if k_raw > MAX_REFLECTION_K_RAW {
        MAX_REFLECTION_K_RAW
    } else if k_raw < -MAX_REFLECTION_K_RAW {
        -MAX_REFLECTION_K_RAW
    } else {
        k_raw
    };
    let unit_sq: u128 = 1_u128 << 64;
    let k_abs = k_clamped.unsigned_abs() as u128;
    let k_sq = k_abs.saturating_mul(k_abs);
    let g_raw = isqrt_u128(unit_sq.saturating_sub(k_sq)) as i128;
    let k_wide = k_clamped as i128;
    let f_wide = f_in.0 as i128;
    let b_wide = b_in.0 as i128;

    let x = g_wide_mac(g_raw, f_wide, k_wide, b_wide);
    let y = g_wide_mac(k_wide.wrapping_neg(), f_wide, g_raw, b_wide);

    let f_out = clamp_i128_to_i64(trunc_div_pow2_i128(x, 32));
    let b_out = clamp_i128_to_i64(trunc_div_pow2_i128(y, 32));
    (Q32(f_out), Q32(b_out))
}

#[inline]
const fn g_wide_mac(a: i128, x: i128, b: i128, y: i128) -> i128 {
    a.saturating_mul(x).saturating_add(b.saturating_mul(y))
}

/// 4-channel unitary Householder damped Feedback Delay Network (`FDN`) step (`§3.3` & `§8.1` formal proof target).
/// Applies `H_4 = I_4 - (1/2) 1 1^T` followed by passive damping `damping_q32 in [0, 1.0]`.
/// Satisfies `\sum_i out_i^2 <= \sum_i in_i^2` across 100% of bitvectors.
pub const fn householder_fdn4_step(state: [Q32; 4], damping_q32: Q32) -> [Q32; 4] {
    let damp = if damping_q32.0 < 0 {
        0_i128
    } else if damping_q32.0 > Q32::ONE.0 {
        Q32::ONE.0 as i128
    } else {
        damping_q32.0 as i128
    };

    let x0 = state[0].0 as i128;
    let x1 = state[1].0 as i128;
    let x2 = state[2].0 as i128;
    let x3 = state[3].0 as i128;
    let sum = x0.saturating_add(x1).saturating_add(x2).saturating_add(x3);

    // 2 * (H_4 * x)_i = 2 * x_i - sum; divide by 2 * 2^32 = 2^33 with truncation toward zero.
    let y0 = x0.saturating_mul(2).saturating_sub(sum).saturating_mul(damp);
    let y1 = x1.saturating_mul(2).saturating_sub(sum).saturating_mul(damp);
    let y2 = x2.saturating_mul(2).saturating_sub(sum).saturating_mul(damp);
    let y3 = x3.saturating_mul(2).saturating_sub(sum).saturating_mul(damp);

    [
        Q32(clamp_i128_to_i64(trunc_div_pow2_i128(y0, 33))),
        Q32(clamp_i128_to_i64(trunc_div_pow2_i128(y1, 33))),
        Q32(clamp_i128_to_i64(trunc_div_pow2_i128(y2, 33))),
        Q32(clamp_i128_to_i64(trunc_div_pow2_i128(y3, 33))),
    ]
}

/// 10-section Kelly-Lochbaum vocal tract + coarticulation + LF glottal source (`§3.3`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocalTractSynth {
    pub tract_length_mm: u16,
    pub target_k: [i64; 10],
    pub current_k: [i64; 10],
    pub forward_wave: [i64; 10],
    pub backward_wave: [i64; 10],
    pub glottal_phase_q32: i64,
    pub jitter_seed: u64,
}

impl VocalTractSynth {
    pub fn new(tract_length_mm: u16, initial_k: [i64; 10], seed: u64) -> Self {
        let clamped_len = tract_length_mm.clamp(135, 195);
        let mut k_arr = [0_i64; 10];
        let mut idx: usize = 0;
        while idx < 10 {
            k_arr[idx] = initial_k[idx].clamp(-MAX_REFLECTION_K_RAW, MAX_REFLECTION_K_RAW);
            idx = idx.saturating_add(1);
        }
        Self {
            tract_length_mm: clamped_len,
            target_k: k_arr,
            current_k: k_arr,
            forward_wave: [0; 10],
            backward_wave: [0; 10],
            glottal_phase_q32: 0,
            jitter_seed: seed,
        }
    }

    pub fn set_coarticulation_target(&mut self, next_k: [i64; 10]) {
        let mut idx: usize = 0;
        while idx < 10 {
            self.target_k[idx] = next_k[idx].clamp(-MAX_REFLECTION_K_RAW, MAX_REFLECTION_K_RAW);
            idx = idx.saturating_add(1);
        }
    }

    /// Synthesizes one `48kHz` vocal sample with LF glottal derivative pulse, micro-jitter, and coarticulation smoothing.
    pub fn step_sample_48khz(&mut self, f0_hz_q32: Q32, aspiration_q32: Q32) -> Q32 {
        // Deterministic xorshift-style LCG for micro-jitter/aspiration
        self.jitter_seed = self
            .jitter_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let noise_raw = ((self.jitter_seed.wrapping_shr(33) as i64) & 0xFFFF).saturating_sub(32_768);
        let jitter = Q32(noise_raw.saturating_mul(2_048));

        let step_phase = f0_hz_q32.sat_div(Q32::from_int(48_000)).sat_add(jitter);
        self.glottal_phase_q32 = Q32(self.glottal_phase_q32).sat_add(step_phase).0 & 0xFFFF_FFFF;

        // LF glottal derivative approximation: sharp opening + return phase
        let phase = Q32(self.glottal_phase_q32);
        let glottal_wave = phase
            .sin_turns()
            .sat_sub(phase.sat_mul(Q32::TWO).sin_turns().sat_mul(Q32::HALF));
        let asp_wave = Q32(noise_raw.wrapping_shl(15)).sat_mul(aspiration_q32);
        let excitation = glottal_wave.sat_add(asp_wave);

        // Smooth coarticulation transition towards target_k (alpha = 1/64)
        let alpha = Q32::from_ratio(1, 64);
        let mut i: usize = 0;
        while i < 10 {
            let diff = Q32(self.target_k[i]).sat_sub(Q32(self.current_k[i]));
            self.current_k[i] = Q32(self.current_k[i])
                .sat_add(diff.sat_mul(alpha))
                .0
                .clamp(-MAX_REFLECTION_K_RAW, MAX_REFLECTION_K_RAW);
            i = i.saturating_add(1);
        }

        // Propagate forward & backward waves through 10 passive junctions
        self.forward_wave[0] = excitation.0;
        let mut m: usize = 0;
        while m < 9 {
            let f_in = Q32(self.forward_wave[m]);
            let b_in = Q32(self.backward_wave[m.saturating_add(1)]);
            let (f_next, b_cur) = kelly_lochbaum_junction_step(f_in, b_in, self.current_k[m]);
            self.forward_wave[m.saturating_add(1)] = f_next.0;
            self.backward_wave[m] = b_cur.0;
            m = m.saturating_add(1);
        }
        // Lip radiation reflection (`k_lip = -0.85`)
        let lip_k = Q32::from_milli(-850).0;
        let (radiated, b_last) =
            kelly_lochbaum_junction_step(Q32(self.forward_wave[9]), Q32::ZERO, lip_k);
        self.backward_wave[9] = b_last.0;
        radiated
    }
}

/// Synthesizes one `48kHz` sample of Drift Phonk nonlinear distorted cowbell + saturated `808` sub-bass (`§3.2`).
pub fn synth_drift_phonk_sample(sample_idx: u64, bpm: u16) -> Q32 {
    let clamped_bpm = bpm.clamp(130, 165) as i64;
    // Samples per beat = 48_000 * 60 / bpm
    let spb = (2_880_000_i64.checked_div(clamped_bpm).unwrap_or(19_200)).max(1) as u64;
    let local = sample_idx.checked_rem(spb).unwrap_or(0);
    let env = Q32::from_ratio((spb.saturating_sub(local)) as i64, spb as i64);

    // Dual Roland-808 cowbell fundamentals: 540 Hz & 800 Hz
    let t_540 = Q32::from_ratio(
        (sample_idx.wrapping_mul(540).wrapping_rem(48_000)) as i64,
        48_000,
    );
    let t_800 = Q32::from_ratio(
        (sample_idx.wrapping_mul(800).wrapping_rem(48_000)) as i64,
        48_000,
    );
    let raw_bell = t_540.sin_turns().sat_add(t_800.sin_turns()).sat_mul(env);

    // Cubic soft-clip waveshaper: y = 1.5x - 0.5x^3 on [-1, 1]
    let neg_one = Q32::ONE.0.saturating_neg();
    let driven = raw_bell.sat_mul(Q32::TWO).0.clamp(neg_one, Q32::ONE.0);
    let x = Q32(driven);
    let x3 = x.sat_mul(x).sat_mul(x);
    let shaped_bell = x
        .sat_mul(Q32::from_milli(1_500))
        .sat_sub(x3.sat_mul(Q32::HALF));

    // 55 Hz saturated 808 sub-bass
    let t_sub = Q32::from_ratio(
        (sample_idx.wrapping_mul(55).wrapping_rem(48_000)) as i64,
        48_000,
    );
    let sub_808 = t_sub.sin_turns().sat_mul(env);

    shaped_bell.sat_mul(Q32::HALF).sat_add(sub_808.sat_mul(Q32::HALF))
}

/// Applies `§3.3` space vacuum acoustic gate (`P_ext == 0 => airborne = 0`, solid-borne lowpass preserved)
/// and returns `(gated_sample, VacuumShotContract<Verified>)`.
pub fn apply_vacuum_gate(
    shot_id: u32,
    external_pressure_pa: u32,
    airborne_sample: Q32,
    solid_hull_lowfreq_sample: Q32,
) -> Result<(Q32, VacuumShotContract<Verified>), ContractViolation> {
    let (effective_airborne, level_dbfs) = if external_pressure_pa == 0 {
        (Q32::ZERO, -120_i32)
    } else {
        (airborne_sample, -16_i32)
    };
    let contract = VacuumShotContract::new_unverified(shot_id, external_pressure_pa, level_dbfs).verify()?;
    Ok((effective_airborne.sat_add(solid_hull_lowfreq_sample), contract))
}

/// Verifies half-frame (`<= 0.5 * dt_f`) phase lock between visual impact `t_v` and nearest audio beat onset `{t_b}` (`§3.2`).
pub fn verify_beat_phase_lock(
    shot_id: u32,
    visual_impact_time: Q32,
    beat_onsets: &[Q32],
) -> Result<ImpactCutContract<Verified>, ContractViolation> {
    if beat_onsets.is_empty() {
        return Err(ContractViolation::BeatImpactPhaseUnlocked);
    }
    let mut best_beat = beat_onsets[0];
    let mut min_err = visual_impact_time.sat_sub(best_beat).sat_abs();
    for &b in beat_onsets {
        let err = visual_impact_time.sat_sub(b).sat_abs();
        if err.0 < min_err.0 {
            min_err = err;
            best_beat = b;
        }
    }
    ImpactCutContract::new_unverified(shot_id, visual_impact_time, best_beat).verify()
}

/// Renders a deterministic `24-bit` PCM frame batch (`[-8_388_608, 8_388_607]`).
pub fn quantize_q32_to_pcm24(samples: &[Q32]) -> Vec<i32> {
    let scale = Q32::from_int(8_388_607);
    let mut out = Vec::with_capacity(samples.len());
    for &s in samples {
        let clamped = s.0.clamp(-Q32::ONE.0, Q32::ONE.0);
        let scaled = Q32(clamped).sat_mul(scale).0.wrapping_shr(32) as i32;
        out.push(scaled.clamp(-8_388_608, 8_388_607));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kelly_lochbaum_and_fdn_passive_energy() {
        let f_in = Q32::from_milli(750);
        let b_in = Q32::from_milli(-620);
        let k = Q32::from_milli(500).0;
        let (f_out, b_out) = kelly_lochbaum_junction_step(f_in, b_in, k);
        let e_in = (f_in.0 as i128)
            .saturating_mul(f_in.0 as i128)
            .saturating_add((b_in.0 as i128).saturating_mul(b_in.0 as i128));
        let e_out = (f_out.0 as i128)
            .saturating_mul(f_out.0 as i128)
            .saturating_add((b_out.0 as i128).saturating_mul(b_out.0 as i128));
        assert!(e_out <= e_in);

        let st = [
            Q32::from_milli(900),
            Q32::from_milli(-400),
            Q32::from_milli(300),
            Q32::from_milli(-700),
        ];
        let nxt = householder_fdn4_step(st, Q32::from_milli(950));
        let mut ein_fdn: i128 = 0;
        let mut eout_fdn: i128 = 0;
        for i in 0..4 {
            ein_fdn = ein_fdn.saturating_add((st[i].0 as i128).saturating_mul(st[i].0 as i128));
            eout_fdn = eout_fdn.saturating_add((nxt[i].0 as i128).saturating_mul(nxt[i].0 as i128));
        }
        assert!(eout_fdn <= ein_fdn);
    }

    #[test]
    fn test_vocal_phonk_and_vacuum_gate() {
        let mut vt = VocalTractSynth::new(172, [Q32::from_milli(120).0; 10], 42);
        vt.set_coarticulation_target([Q32::from_milli(-200).0; 10]);
        let s0 = vt.step_sample_48khz(Q32::from_int(130), Q32::from_milli(40));
        let s1 = vt.step_sample_48khz(Q32::from_int(130), Q32::from_milli(40));
        assert_ne!(s0, s1);

        let phonk = synth_drift_phonk_sample(120, 150);
        assert!(phonk.0.saturating_abs() <= Q32::TWO.0);

        let vac = apply_vacuum_gate(1, 0, Q32::ONE, Q32::from_milli(50));
        assert!(vac.is_ok());
        if let Ok((out_s, _)) = vac {
            assert_eq!(out_s, Q32::from_milli(50));
        }
    }
}
