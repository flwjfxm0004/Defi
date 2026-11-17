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

use math_q32::{Q32, Vec3Q32};
use serde::{Deserialize, Serialize};
use verifier_abi::{ContractViolation, SheetContract, Verified};

pub const MASTER_WIDTH_PX: u32 = 3_840;
pub const MASTER_HEIGHT_IMAX_PX: u32 = 2_020;
pub const MASTER_HEIGHT_SCOPE_PX: u32 = 1_600;

/// Aspect ratio mode (`§2.1`: `2.40:1` noir scope ↔ `1.90:1` full IMAX).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectMode {
    Scope240,
    Imax190,
}

/// Active canvas height in pixels during seamless `2.40:1 -> 1.90:1` expansion (`blend_q32 in [0, 1]`).
pub fn active_canvas_height_px(blend_q32: Q32) -> u32 {
    let t = Q32(blend_q32.0.clamp(0, Q32::ONE.0));
    let delta = Q32::from_int(420).sat_mul(t).0.wrapping_shr(32) as i32;
    (MASTER_HEIGHT_SCOPE_PX as i32)
        .saturating_add(delta)
        .clamp(MASTER_HEIGHT_SCOPE_PX as i32, MASTER_HEIGHT_IMAX_PX as i32) as u32
}

/// Evaluates a $C^2$-continuous uniform cubic B-spline camera segment for `u in [0, 1]` (`§2.1`).
pub fn eval_c2_camera_spline(
    p0: Vec3Q32,
    p1: Vec3Q32,
    p2: Vec3Q32,
    p3: Vec3Q32,
    u_q32: Q32,
) -> Vec3Q32 {
    let u = Q32(u_q32.0.clamp(0, Q32::ONE.0));
    let u2 = u.sat_mul(u);
    let u3 = u2.sat_mul(u);
    let one_minus_u = Q32::ONE.sat_sub(u);
    let inv6 = Q32::from_ratio(1, 6);

    // B0 = (1 - u)^3 / 6
    let b0 = one_minus_u
        .sat_mul(one_minus_u)
        .sat_mul(one_minus_u)
        .sat_mul(inv6);
    // B1 = (3u^3 - 6u^2 + 4) / 6
    let b1 = u3
        .sat_mul(Q32::from_int(3))
        .sat_sub(u2.sat_mul(Q32::from_int(6)))
        .sat_add(Q32::from_int(4))
        .sat_mul(inv6);
    // B2 = (-3u^3 + 3u^2 + 3u + 1) / 6
    let b2 = u2
        .sat_mul(Q32::from_int(3))
        .sat_sub(u3.sat_mul(Q32::from_int(3)))
        .sat_add(u.sat_mul(Q32::from_int(3)))
        .sat_add(Q32::ONE)
        .sat_mul(inv6);
    // B3 = u^3 / 6
    let b3 = u3.sat_mul(inv6);

    p0.scale(b0)
        .sat_add(p1.scale(b1))
        .sat_add(p2.scale(b2))
        .sat_add(p3.scale(b3))
}

/// Mirror Dimension Kaleidoscopic IFS (`KIFS`) space-folding step (`§2.2`):
/// `p_{k+1} = R_z(theta) |p_k| - c_k`.
pub fn mirror_dimension_kifs_fold(
    p: Vec3Q32,
    fold_offset: Vec3Q32,
    angle_turns: Q32,
    iterations: u8,
) -> Vec3Q32 {
    let s = angle_turns.sin_turns();
    let c = angle_turns.cos_turns();
    let mut cur = p;
    let mut k: u8 = 0;
    let max_iter = iterations.min(12);
    while k < max_iter {
        let folded = cur.abs_components();
        let rx = folded.x.sat_mul(c).sat_sub(folded.y.sat_mul(s));
        let ry = folded.x.sat_mul(s).sat_add(folded.y.sat_mul(c));
        cur = Vec3Q32::new(rx, ry, folded.z).sat_sub(fold_offset);
        k = k.saturating_add(1);
    }
    cur
}

/// Bidirectional dimensional ring portal momentum & position transfer (`§2.2`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RingPortalPair {
    pub entry_center_x: i64,
    pub entry_center_y: i64,
    pub entry_center_z: i64,
    pub exit_center_x: i64,
    pub exit_center_y: i64,
    pub exit_center_z: i64,
    pub yaw_delta_turns: i64,
}

impl RingPortalPair {
    /// Teleports position and rotates momentum across exit portal while conserving kinetic speed.
    pub fn traverse(&self, pos: Vec3Q32, vel: Vec3Q32) -> (Vec3Q32, Vec3Q32) {
        let entry = Vec3Q32::new(
            Q32(self.entry_center_x),
            Q32(self.entry_center_y),
            Q32(self.entry_center_z),
        );
        let exit = Vec3Q32::new(
            Q32(self.exit_center_x),
            Q32(self.exit_center_y),
            Q32(self.exit_center_z),
        );
        let rel = pos.sat_sub(entry);
        let ang = Q32(self.yaw_delta_turns);
        let s = ang.sin_turns();
        let c = ang.cos_turns();

        let rot_rel = Vec3Q32::new(
            rel.x.sat_mul(c).sat_sub(rel.z.sat_mul(s)),
            rel.y,
            rel.x.sat_mul(s).sat_add(rel.z.sat_mul(c)),
        );
        let rot_vel = Vec3Q32::new(
            vel.x.sat_mul(c).sat_sub(vel.z.sat_mul(s)),
            vel.y,
            vel.x.sat_mul(s).sat_add(vel.z.sat_mul(c)),
        );
        (exit.sat_add(rot_rel), rot_vel)
    }
}

/// 3-core zero-stagger weapon/relic/possession switching controller (`§2.2`: `\Delta t_{swap} <= 3 \Delta t_f`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriCoreRig {
    pub core_ids: [u32; 3],
    pub active_slot: u8,
    pub velocity_x: i64,
    pub velocity_y: i64,
    pub velocity_z: i64,
}

impl TriCoreRig {
    pub fn swap_core(
        &mut self,
        next_slot: u8,
        swap_frames_60hz: u8,
    ) -> Result<Vec3Q32, &'static str> {
        if next_slot >= 3 {
            return Err("Core slot index out of bounds");
        }
        if swap_frames_60hz > 3 {
            return Err("Core swap exceeded 3 frames (zero-stagger invariant violated)");
        }
        self.active_slot = next_slot;
        // Momentum vector inherited 100% continuously without stagger decay
        Ok(Vec3Q32::new(
            Q32(self.velocity_x),
            Q32(self.velocity_y),
            Q32(self.velocity_z),
        ))
    }
}

/// `44-AU` Facial Action Coding System state + `1..3Hz` ocular micro-saccade rig (`§3.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacialRig44Au {
    pub cast_id: u32,
    pub canonical_bone_ratios_q32: [i64; 8],
    pub au_activations_q32: [i64; 44],
    pub prev_au_activations_q32: [i64; 44],
    pub saccade_x_q32: i64,
    pub saccade_y_q32: i64,
}

impl FacialRig44Au {
    pub fn new(cast_id: u32, canonical_bone_ratios_q32: [i64; 8]) -> Self {
        Self {
            cast_id,
            canonical_bone_ratios_q32,
            au_activations_q32: [0; 44],
            prev_au_activations_q32: [0; 44],
            saccade_x_q32: 0,
            saccade_y_q32: 0,
        }
    }

    /// Advances micro-expression & `2Hz` ocular saccade at `frame_idx` (`60Hz`), returning verified `SheetContract`.
    pub fn step_frame_and_verify(
        &mut self,
        frame_idx: u64,
        emotion_drive: Q32,
        measured_bone_ratios_q32: &[i64; 8],
        delta_e_ok: Q32,
    ) -> Result<SheetContract<Verified>, ContractViolation> {
        self.prev_au_activations_q32 = self.au_activations_q32;

        // 2Hz micro-saccade (period = 30 frames at 60Hz)
        let phase = Q32::from_ratio((frame_idx.wrapping_rem(30)) as i64, 30);
        self.saccade_x_q32 = phase.sin_turns().sat_mul(Q32::from_milli(18)).0;
        self.saccade_y_q32 = phase.cos_turns().sat_mul(Q32::from_milli(12)).0;

        let mut i: usize = 0;
        let mut vel_sq_acc: i64 = 0;
        while i < 44 {
            let phase_au = Q32::from_ratio(
                (frame_idx
                    .wrapping_add((i as u64).wrapping_mul(7))
                    .wrapping_rem(60)) as i64,
                60,
            );
            let micro = phase_au.sin_turns().sat_mul(Q32::from_milli(35));
            let next_val = emotion_drive
                .sat_add(micro)
                .0
                .clamp(0, Q32::ONE.0);
            self.au_activations_q32[i] = next_val;
            let dv = Q32(next_val).sat_sub(Q32(self.prev_au_activations_q32[i]));
            vel_sq_acc = vel_sq_acc.saturating_add(dv.sat_mul(dv).0);
            i = i.saturating_add(1);
        }

        let mut bone_err: i64 = 0;
        let mut b: usize = 0;
        while b < 8 {
            let diff = self.canonical_bone_ratios_q32[b]
                .saturating_sub(measured_bone_ratios_q32[b])
                .saturating_abs();
            bone_err = bone_err.saturating_add(diff);
            b = b.saturating_add(1);
        }

        SheetContract::new_unverified(
            self.cast_id,
            Q32(bone_err),
            delta_e_ok,
            Q32(vel_sq_acc),
        )
        .verify()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kifs_portal_core_swap_and_44au() {
        assert_eq!(active_canvas_height_px(Q32::ZERO), 1_600);
        assert_eq!(active_canvas_height_px(Q32::ONE), 2_020);

        let folded = mirror_dimension_kifs_fold(
            Vec3Q32::new(Q32::ONE, Q32::from_int(-2), Q32::HALF),
            Vec3Q32::new(Q32::HALF, Q32::HALF, Q32::ZERO),
            Q32::from_milli(125),
            4,
        );
        assert_ne!(folded, Vec3Q32::ZERO);

        let portal = RingPortalPair {
            entry_center_x: 0,
            entry_center_y: 0,
            entry_center_z: 0,
            exit_center_x: Q32::from_int(100).0,
            exit_center_y: Q32::from_int(50).0,
            exit_center_z: 0,
            yaw_delta_turns: Q32::from_milli(250).0,
        };
        let v_in = Vec3Q32::new(Q32::from_int(10), Q32::ZERO, Q32::ZERO);
        let (_, v_out) = portal.traverse(Vec3Q32::ZERO, v_in);
        let diff_speed = v_out.norm().sat_sub(v_in.norm()).sat_abs();
        assert!(diff_speed.0 < 1_000);

        let bones = [Q32::ONE.0; 8];
        let mut rig = FacialRig44Au::new(1, bones);
        let verified = rig.step_frame_and_verify(1, Q32::HALF, &bones, Q32::from_milli(10));
        assert!(verified.is_ok());
    }
}
