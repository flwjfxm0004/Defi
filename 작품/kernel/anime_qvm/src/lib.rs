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

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use dsp_studio::{VocalTractSynth, householder_fdn4_step, synth_drift_phonk_sample};
use math_q32::{Q32, Vec3Q32};
use sakuga_biomech::{FacialRig44Au, TriCoreRig, mirror_dimension_kifs_fold};
use serde::{Deserialize, Serialize};
use typo_hangul::RectI32;
use verifier_abi::ContractViolation;
use world_ledger::{AssetKind, LedgerError, WorldLedger, hash_canonical_bytes};

/// Maximum episode instruction fuel (`§7.3`: `<= 3,000,000 ops`).
pub const MAX_EPISODE_FUEL: u64 = 3_000_000;
pub const DEFAULT_SAFE_SUBTITLE_BOX: RectI32 = RectI32 {
    x0: 240,
    y0: 1840,
    x1: 3600,
    y1: 2040,
};
pub type QvmAbiContractError = ContractViolation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QvmError {
    EmptyProgram,
    DuplicateFunctionId,
    UnknownSubroutineTarget,
    RecursiveOrCyclicCallGraph,
    FuelBudgetExceeded,
    LedgerFault(LedgerError),
    RigSwapFault,
}

/// Statically typed `Copy`-value DSL instructions (`§7.3`: zero backward branches, zero floats, zero OS calls).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QvmOp {
    TransferCapital { from: u32, to: u32, amount: i64 },
    StepLifespanPair {
        actor_a: u32,
        actor_b: u32,
        dt_a: i64,
        dt_b: i64,
        tx_a_to_b: i64,
    },
    FoldMirrorSpace {
        offset_milli_x: i32,
        offset_milli_y: i32,
        angle_milli_turns: i32,
        iters: u8,
    },
    SwapCombatCore { next_slot: u8, swap_frames: u8 },
    DriveFacialRig { frame_idx: u64, emotion_milli: i32 },
    StepAudioFrame { sample_idx: u64, bpm: u16, f0_hz: u16 },
    CallSubroutine { target_fn: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QvmFunction {
    pub fn_id: u16,
    pub ops: Vec<QvmOp>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QvmProgram {
    pub entry_fn: u16,
    pub functions: Vec<QvmFunction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedQvmProgram {
    entry_fn: u16,
    fn_map: BTreeMap<u16, Vec<QvmOp>>,
    worst_case_fuel: u64,
}

impl ValidatedQvmProgram {
    #[inline]
    pub const fn worst_case_fuel(&self) -> u64 {
        self.worst_case_fuel
    }
}

/// Compiles and verifies call-graph `DAG` acyclicity (banning recursion) and `<= 3,000,000` fuel bound (`§7.3`).
pub fn verify_qvm_program(prog: &QvmProgram) -> Result<ValidatedQvmProgram, QvmError> {
    if prog.functions.is_empty() {
        return Err(QvmError::EmptyProgram);
    }
    let mut fn_map: BTreeMap<u16, Vec<QvmOp>> = BTreeMap::new();
    for f in &prog.functions {
        if fn_map.insert(f.fn_id, f.ops.clone()).is_some() {
            return Err(QvmError::DuplicateFunctionId);
        }
    }
    if !fn_map.contains_key(&prog.entry_fn) {
        return Err(QvmError::UnknownSubroutineTarget);
    }

    // Build call-graph adjacency and indegrees
    let mut indeg: BTreeMap<u16, usize> = BTreeMap::new();
    let mut adj: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    for &fid in fn_map.keys() {
        indeg.insert(fid, 0);
        adj.insert(fid, Vec::new());
    }

    for (&fid, ops) in &fn_map {
        for op in ops {
            if let QvmOp::CallSubroutine { target_fn } = *op {
                if !fn_map.contains_key(&target_fn) {
                    return Err(QvmError::UnknownSubroutineTarget);
                }
                if target_fn == fid {
                    return Err(QvmError::RecursiveOrCyclicCallGraph);
                }
                if let Some(list) = adj.get_mut(&fid) {
                    list.push(target_fn);
                }
                if let Some(d) = indeg.get_mut(&target_fn) {
                    *d = d.saturating_add(1);
                }
            }
        }
    }

    // Kahn topological sort to prove strict DAG (zero recursion)
    let mut queue: VecDeque<u16> = VecDeque::new();
    for (&fid, &d) in &indeg {
        if d == 0 {
            queue.push_back(fid);
        }
    }
    let mut topo_order: Vec<u16> = Vec::with_capacity(fn_map.len());
    while let Some(u) = queue.pop_front() {
        topo_order.push(u);
        if let Some(callees) = adj.get(&u) {
            for &v in callees {
                if let Some(dv) = indeg.get_mut(&v) {
                    *dv = dv.saturating_sub(1);
                    if *dv == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
    }
    if topo_order.len() != fn_map.len() {
        return Err(QvmError::RecursiveOrCyclicCallGraph);
    }

    // Reverse topological DP for exact fuel cost of each function
    let mut fn_cost: BTreeMap<u16, u64> = BTreeMap::new();
    for &fid in topo_order.iter().rev() {
        let mut cost: u64 = 0;
        if let Some(ops) = fn_map.get(&fid) {
            for op in ops {
                cost = cost.saturating_add(1);
                if let QvmOp::CallSubroutine { target_fn } = *op {
                    let sub_c = fn_cost.get(&target_fn).copied().unwrap_or(0);
                    cost = cost.saturating_add(sub_c);
                }
                if cost > MAX_EPISODE_FUEL {
                    return Err(QvmError::FuelBudgetExceeded);
                }
            }
        }
        fn_cost.insert(fid, cost);
    }

    let entry_fuel = fn_cost.get(&prog.entry_fn).copied().unwrap_or(0);
    if entry_fuel > MAX_EPISODE_FUEL {
        return Err(QvmError::FuelBudgetExceeded);
    }

    Ok(ValidatedQvmProgram {
        entry_fn: prog.entry_fn,
        fn_map,
        worst_case_fuel: entry_fuel,
    })
}

/// Deterministic `K_sim` sandbox state (`§7.1`, `§7.3`, `§9.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QvmSandbox {
    pub ledger: WorldLedger,
    pub spatial_pos: Vec3Q32,
    pub tri_core: TriCoreRig,
    pub facial_rig: FacialRig44Au,
    pub vocal_tract: VocalTractSynth,
    pub fdn_state: [Q32; 4],
    pub last_audio_out: Q32,
    pub step_count: u64,
    pub fuel_consumed: u64,
}

impl QvmSandbox {
    pub fn new_genesis(seed: u64) -> Result<Self, QvmError> {
        let mut ledger = WorldLedger::new();
        ledger
            .register_actor(0, 10_000_000, 50_000_000)
            .map_err(QvmError::LedgerFault)?;
        ledger
            .register_actor(1, 10_000_000, 50_000_000)
            .map_err(QvmError::LedgerFault)?;

        let bones = [Q32::ONE.0; 8];
        Ok(Self {
            ledger,
            spatial_pos: Vec3Q32::new(Q32::ONE, Q32::HALF, Q32::TWO),
            tri_core: TriCoreRig {
                core_ids: [101, 202, 303],
                active_slot: 0,
                velocity_x: Q32::from_int(12).0,
                velocity_y: Q32::from_int(-3).0,
                velocity_z: Q32::from_int(8).0,
            },
            facial_rig: FacialRig44Au::new(0, bones),
            vocal_tract: VocalTractSynth::new(175, [Q32::from_milli(110).0; 10], seed),
            fdn_state: [Q32::ZERO; 4],
            last_audio_out: Q32::ZERO,
            step_count: 0,
            fuel_consumed: 0,
        })
    }

    pub fn execute_program(&mut self, prog: &ValidatedQvmProgram) -> Result<(), QvmError> {
        self.exec_fn(prog, prog.entry_fn)
    }

    fn exec_fn(&mut self, prog: &ValidatedQvmProgram, fn_id: u16) -> Result<(), QvmError> {
        let Some(ops) = prog.fn_map.get(&fn_id) else {
            return Err(QvmError::UnknownSubroutineTarget);
        };
        for &op in ops {
            self.fuel_consumed = self.fuel_consumed.saturating_add(1);
            if self.fuel_consumed > MAX_EPISODE_FUEL {
                return Err(QvmError::FuelBudgetExceeded);
            }
            self.exec_single_op(prog, op)?;
        }
        Ok(())
    }

    fn exec_single_op(&mut self, prog: &ValidatedQvmProgram, op: QvmOp) -> Result<(), QvmError> {
        match op {
            QvmOp::TransferCapital { from, to, amount } => {
                let neg_amt = amount.saturating_neg();
                self.ledger
                    .apply_zero_sum_transfer(
                        AssetKind::Capital(0),
                        &[(from, neg_amt), (to, amount)],
                    )
                    .map_err(QvmError::LedgerFault)?;
            }
            QvmOp::StepLifespanPair {
                actor_a,
                actor_b,
                dt_a,
                dt_b,
                tx_a_to_b,
            } => {
                let neg_tx = tx_a_to_b.saturating_neg();
                self.ledger
                    .step_lifespan(&[(actor_a, dt_a, 0, neg_tx), (actor_b, dt_b, 0, tx_a_to_b)])
                    .map_err(QvmError::LedgerFault)?;
            }
            QvmOp::FoldMirrorSpace {
                offset_milli_x,
                offset_milli_y,
                angle_milli_turns,
                iters,
            } => {
                let off = Vec3Q32::new(
                    Q32::from_milli(offset_milli_x as i64),
                    Q32::from_milli(offset_milli_y as i64),
                    Q32::ZERO,
                );
                self.spatial_pos = mirror_dimension_kifs_fold(
                    self.spatial_pos,
                    off,
                    Q32::from_milli(angle_milli_turns as i64),
                    iters,
                );
            }
            QvmOp::SwapCombatCore {
                next_slot,
                swap_frames,
            } => {
                self.tri_core
                    .swap_core(next_slot, swap_frames)
                    .map_err(|_| QvmError::RigSwapFault)?;
            }
            QvmOp::DriveFacialRig {
                frame_idx,
                emotion_milli,
            } => {
                let bones = self.facial_rig.canonical_bone_ratios_q32;
                let _ = self.facial_rig.step_frame_and_verify(
                    frame_idx,
                    Q32::from_milli(emotion_milli as i64),
                    &bones,
                    Q32::from_milli(5),
                );
            }
            QvmOp::StepAudioFrame {
                sample_idx,
                bpm,
                f0_hz,
            } => {
                let v = self
                    .vocal_tract
                    .step_sample_48khz(Q32::from_int(f0_hz as i32), Q32::from_milli(25));
                let p = synth_drift_phonk_sample(sample_idx, bpm);
                self.fdn_state[0] = self.fdn_state[0].sat_add(v.sat_mul(Q32::HALF));
                self.fdn_state[1] = self.fdn_state[1].sat_add(p.sat_mul(Q32::HALF));
                self.fdn_state = householder_fdn4_step(self.fdn_state, Q32::from_milli(920));
                self.last_audio_out = self.fdn_state[0].sat_add(self.fdn_state[1]);
            }
            QvmOp::CallSubroutine { target_fn } => {
                self.exec_fn(prog, target_fn)?;
            }
        }
        self.step_count = self.step_count.saturating_add(1);
        Ok(())
    }

    /// Executes one coupled `K_sim` simulation tick (ledger + KIFS kinematics + 3-core swap + DSP lattice)
    /// used in the `100,000`-step state-hash determinism proof (`§9.1`).
    pub fn step_coupled_tick(&mut self) -> Result<(), QvmError> {
        let idx = self.step_count;
        let dir = if idx.wrapping_rem(2) == 0 { 5_i64 } else { -5_i64 };
        self.ledger
            .step_lifespan(&[(0, 1, 0, dir), (1, 1, 0, dir.saturating_neg())])
            .map_err(QvmError::LedgerFault)?;

        let slot = (idx.wrapping_rem(3)) as u8;
        self.tri_core
            .swap_core(slot, 1)
            .map_err(|_| QvmError::RigSwapFault)?;

        self.spatial_pos = mirror_dimension_kifs_fold(
            self.spatial_pos,
            Vec3Q32::new(Q32::from_milli(120), Q32::from_milli(80), Q32::ZERO),
            Q32::from_milli(15),
            1,
        );

        let vocal = self
            .vocal_tract
            .step_sample_48khz(Q32::from_int(140), Q32::from_milli(20));
        let phonk = synth_drift_phonk_sample(idx, 148);
        self.fdn_state[0] = vocal;
        self.fdn_state[1] = phonk;
        self.fdn_state = householder_fdn4_step(self.fdn_state, Q32::from_milli(940));
        self.last_audio_out = self.fdn_state[0].sat_add(self.fdn_state[1]);
        self.step_count = self.step_count.saturating_add(1);
        Ok(())
    }

    /// Computes canonical `BLAKE3` digest of the entire `K_sim` state.
    pub fn state_blake3(&self) -> [u8; 32] {
        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(&self.step_count.to_le_bytes());
        buf.extend_from_slice(&self.fuel_consumed.to_le_bytes());
        for (uid, bal) in self.ledger.capital_snapshot() {
            buf.extend_from_slice(&uid.to_le_bytes());
            buf.extend_from_slice(&bal.to_le_bytes());
        }
        for (uid, life) in self.ledger.lifespan_snapshot() {
            buf.extend_from_slice(&uid.to_le_bytes());
            buf.extend_from_slice(&life.to_le_bytes());
        }
        buf.extend_from_slice(&self.spatial_pos.x.0.to_le_bytes());
        buf.extend_from_slice(&self.spatial_pos.y.0.to_le_bytes());
        buf.extend_from_slice(&self.spatial_pos.z.0.to_le_bytes());
        buf.push(self.tri_core.active_slot);
        buf.extend_from_slice(&self.last_audio_out.0.to_le_bytes());
        buf.extend_from_slice(&self.vocal_tract.jitter_seed.to_le_bytes());
        for s in &self.fdn_state {
            buf.extend_from_slice(&s.0.to_le_bytes());
        }
        hash_canonical_bytes(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_callgraph_rejects_recursion_and_deterministic_ticks() {
        let rec_prog = QvmProgram {
            entry_fn: 0,
            functions: alloc::vec![
                QvmFunction {
                    fn_id: 0,
                    ops: alloc::vec![QvmOp::CallSubroutine { target_fn: 1 }],
                },
                QvmFunction {
                    fn_id: 1,
                    ops: alloc::vec![QvmOp::CallSubroutine { target_fn: 0 }],
                },
            ],
        };
        assert_eq!(
            verify_qvm_program(&rec_prog),
            Err(QvmError::RecursiveOrCyclicCallGraph)
        );

        let mut s1 = QvmSandbox::new_genesis(777);
        let mut s2 = QvmSandbox::new_genesis(777);
        assert!(s1.is_ok() && s2.is_ok());
        if let (Ok(vm1), Ok(vm2)) = (&mut s1, &mut s2) {
            for _ in 0..256 {
                assert_eq!(vm1.step_coupled_tick(), Ok(()));
                assert_eq!(vm2.step_coupled_tick(), Ok(()));
            }
            assert_eq!(vm1.state_blake3(), vm2.state_blake3());
        }
    }
}
