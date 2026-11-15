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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use ecs_soa::{CommandBuffer, EntityId, SimCommand, SoAView};
use math_q32::{Q32, QVec3};
use serde::{Deserialize, Serialize};
use verifier_abi::{AnonMechRole, CausalEdge, CausalGraphIR};

pub const MAX_REGS: usize = 64;
pub const MAX_CALL_DEPTH: usize = 8;
pub const MAX_LOOP_NESTING: usize = 4;
pub const MAX_STATIC_LOOP_ITERS: u16 = 256;
pub const INVOCATION_FUEL_CAP: u32 = 4_000;
pub const CLIENT_ROLLBACK_FRAME_FUEL_CAP: u32 = 50_000;
pub const SERVER_SHARD_TICK_FUEL_CAP: u32 = 200_000;

/// Copy-only value types in `qvm` (`f32`/`f64`, heap refs, closures physically absent).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum QvmType {
    Bool,
    I32,
    Q32,
    QVec3,
    EntityId,
    Unit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RegVal {
    #[default]
    Unit,
    Bool(bool),
    I32(i32),
    Q32(Q32),
    QVec3(QVec3),
    Entity(EntityId),
}

/// Fixed-depth register bytecode ISA including composite vector superinstructions (`게임.md §8.3`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Op {
    LoadConstQ32 { dst: u8, raw_q32: i64 },
    LoadConstI32 { dst: u8, val: i32 },
    LoadConstBool { dst: u8, val: bool },
    MoveReg { dst: u8, src: u8 },

    AddQ32 { dst: u8, a: u8, b: u8 },
    SubQ32 { dst: u8, a: u8, b: u8 },
    MulQ32 { dst: u8, a: u8, b: u8 },
    DivQ32 { dst: u8, a: u8, b: u8 },
    CmpLtQ32 { dst: u8, a: u8, b: u8 },
    CmpGeQ32 { dst: u8, a: u8, b: u8 },

    MakeQVec3 { dst: u8, x: u8, y: u8, z: u8 },
    AddQVec3 { dst: u8, a: u8, b: u8 },
    SubQVec3 { dst: u8, a: u8, b: u8 },
    ScaleQVec3 { dst: u8, v: u8, s: u8 },

    /// Superinstructions with fixed integer fuel weights (`게임.md §8.3`).
    FmaQVec3 { dst: u8, v: u8, s: u8, bias: u8 },
    ClampQ32 { dst: u8, val: u8, lo: u8, hi: u8 },
    DotQVec3 { dst: u8, a: u8, b: u8 },
    DistSqQVec3 { dst: u8, a: u8, b: u8 },

    /// Immutable `&SoAView` queries.
    ReadPos { dst: u8, entity: u8 },
    ReadVel { dst: u8, entity: u8 },
    ReadHp { dst: u8, entity: u8 },
    ReadShield { dst: u8, entity: u8 },
    ReadCharge { dst: u8, entity: u8 },
    HasTraitBit { dst: u8, entity: u8, bit: u8 },

    /// Acyclic control flow (`JumpIfFalse` forward only; `ForLoop` bounded by compile-time `max_iters`).
    JumpIfFalse { cond: u8, target_pc: u16 },
    JumpForward { target_pc: u16 },
    ForBegin { counter_reg: u8, max_iters: u16, exit_pc: u16 },
    ForStep { counter_reg: u8, max_iters: u16, body_pc: u16 },

    /// Acyclic `DAG` static call into disjoint callee register window `[caller_base + frame_base ..]`.
    CallStatic { func_idx: u16, arg0_src: u8, arg1_src: u8, ret_dst: u8, frame_base: u8 },
    Return { src: u8 },

    /// Deterministic `SimCommand` emitters (`CommandBuffer`).
    EmitDealDamage { src_ent: u8, dst_ent: u8, hp_dmg: u8, shatter_dmg: u8, charge_gain: u8 },
    EmitImpulse { dst_ent: u8, impulse_vec: u8 },
    EmitPhantasia { src_ent: u8, center_vec: u8, radius: u8, slow_scale: u8 },
    EmitClash { defender_ent: u8, boss_ent: u8, shatter_burst: u8 },
    EmitProjectile { owner_ent: u8, origin_vec: u8, vel_vec: u8, damage: u8 },
    EmitBossPattern { boss_ent: u8, pattern_id: u16 },
}

impl Op {
    /// Fixed integer fuel weight table (`>= 1` for every instruction -> strict termination).
    #[inline]
    pub const fn fuel_cost(self) -> u32 {
        match self {
            Self::LoadConstQ32 { .. }
            | Self::LoadConstI32 { .. }
            | Self::LoadConstBool { .. }
            | Self::MoveReg { .. } => 1,
            Self::AddQ32 { .. }
            | Self::SubQ32 { .. }
            | Self::CmpLtQ32 { .. }
            | Self::CmpGeQ32 { .. } => 1,
            Self::MulQ32 { .. } | Self::DivQ32 { .. } | Self::ClampQ32 { .. } => 2,
            Self::MakeQVec3 { .. }
            | Self::AddQVec3 { .. }
            | Self::SubQVec3 { .. }
            | Self::ScaleQVec3 { .. } => 2,
            Self::FmaQVec3 { .. } | Self::DotQVec3 { .. } | Self::DistSqQVec3 { .. } => 3,
            Self::ReadPos { .. }
            | Self::ReadVel { .. }
            | Self::ReadHp { .. }
            | Self::ReadShield { .. }
            | Self::ReadCharge { .. }
            | Self::HasTraitBit { .. } => 2,
            Self::JumpIfFalse { .. }
            | Self::JumpForward { .. }
            | Self::ForBegin { .. }
            | Self::ForStep { .. } => 1,
            Self::CallStatic { .. } | Self::Return { .. } => 3,
            Self::EmitDealDamage { .. }
            | Self::EmitImpulse { .. }
            | Self::EmitPhantasia { .. }
            | Self::EmitClash { .. }
            | Self::EmitProjectile { .. }
            | Self::EmitBossPattern { .. } => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledFunc {
    pub name: String,
    pub topo_rank: u16,
    pub reg_window_size: u8,
    pub code: Vec<Op>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledQvmModule {
    pub funcs: Vec<CompiledFunc>,
    pub entry_func: u16,
    pub max_static_reg_depth: u8,
    pub causal_graph: CausalGraphIR,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QvmError {
    ForbiddenConstruct(&'static str),
    SyntaxError(&'static str),
    RecursiveCallCycle,
    RegisterDepthExceeded,
    StaticLoopBoundExceeded,
    InvalidRegisterIndex,
    BackwardUnstructuredJump,
    TopologicalOrderViolation,
    InvocationFuelExceeded,
    CommandBufferOverflow,
}

/// Static bytecode & acyclic `DAG` verifier (`게임.md §8.3, §10.1 G1`).
/// Recomputes full transitive register stack depth from scratch without trusting serialized summary fields.
pub fn verify_compiled_module(module: &CompiledQvmModule) -> Result<(), QvmError> {
    let n = module.funcs.len();
    if n == 0 || (module.entry_func as usize) >= n {
        return Err(QvmError::SyntaxError("Missing entry function"));
    }
    if (module.max_static_reg_depth as usize) > MAX_REGS {
        return Err(QvmError::RegisterDepthExceeded);
    }

    for (f_idx, func) in module.funcs.iter().enumerate() {
        let code_len = func.code.len();
        // Every function takes `(self, target)` in registers `[0, 1]`, so `reg_window_size >= 2`.
        if code_len == 0 || func.reg_window_size < 2 || (func.reg_window_size as usize) > MAX_REGS {
            return Err(QvmError::RegisterDepthExceeded);
        }
        for (pc, &op) in func.code.iter().enumerate() {
            if op.fuel_cost() == 0 {
                return Err(QvmError::ForbiddenConstruct("Zero-fuel instruction"));
            }
            check_op_registers(op, func.reg_window_size)?;
            match op {
                Op::JumpIfFalse { target_pc, .. } | Op::JumpForward { target_pc } => {
                    if (target_pc as usize) <= pc || (target_pc as usize) > code_len {
                        return Err(QvmError::BackwardUnstructuredJump);
                    }
                }
                Op::ForBegin { max_iters, exit_pc, .. } => {
                    if max_iters == 0 || max_iters > MAX_STATIC_LOOP_ITERS {
                        return Err(QvmError::StaticLoopBoundExceeded);
                    }
                    if (exit_pc as usize) <= pc || (exit_pc as usize) > code_len {
                        return Err(QvmError::BackwardUnstructuredJump);
                    }
                }
                Op::ForStep { max_iters, body_pc, .. } => {
                    if max_iters == 0 || max_iters > MAX_STATIC_LOOP_ITERS {
                        return Err(QvmError::StaticLoopBoundExceeded);
                    }
                    if (body_pc as usize) >= pc {
                        return Err(QvmError::SyntaxError("Invalid ForStep target"));
                    }
                }
                Op::CallStatic {
                    func_idx,
                    frame_base,
                    ..
                } => {
                    let callee_i = func_idx as usize;
                    if callee_i >= n || callee_i == f_idx {
                        return Err(QvmError::RecursiveCallCycle);
                    }
                    // Disjoint register window invariant: `frame_base >= caller.reg_window_size`!
                    if frame_base < func.reg_window_size {
                        return Err(QvmError::InvalidRegisterIndex);
                    }
                    let callee = &module.funcs[callee_i];
                    // Strict DAG topological rank: caller.topo_rank > callee.topo_rank -> cycle impossible!
                    if func.topo_rank <= callee.topo_rank {
                        return Err(QvmError::TopologicalOrderViolation);
                    }
                }
                _ => {}
            }
        }
    }

    // Recompute exact transitive peak register depth & call depth across the DAG in ascending `topo_rank`!
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| module.funcs[i].topo_rank);
    let mut peak_regs: Vec<usize> = alloc::vec![0; n];
    let mut peak_calls: Vec<usize> = alloc::vec![1; n];

    for &f_idx in &order {
        let func = &module.funcs[f_idx];
        let mut r_peak = func.reg_window_size as usize;
        let mut c_peak: usize = 1;
        for &op in &func.code {
            if let Op::CallStatic {
                func_idx,
                frame_base,
                ..
            } = op
            {
                let callee_i = func_idx as usize;
                let sub_regs = (frame_base as usize).saturating_add(peak_regs[callee_i]);
                if sub_regs > MAX_REGS {
                    return Err(QvmError::RegisterDepthExceeded);
                }
                if sub_regs > r_peak {
                    r_peak = sub_regs;
                }
                let sub_calls = 1_usize.saturating_add(peak_calls[callee_i]);
                if sub_calls > MAX_CALL_DEPTH {
                    return Err(QvmError::RegisterDepthExceeded);
                }
                if sub_calls > c_peak {
                    c_peak = sub_calls;
                }
            }
        }
        peak_regs[f_idx] = r_peak;
        peak_calls[f_idx] = c_peak;
    }

    let entry_peak = peak_regs[module.entry_func as usize];
    if entry_peak > MAX_REGS || (module.max_static_reg_depth as usize) != entry_peak {
        return Err(QvmError::RegisterDepthExceeded);
    }

    Ok(())
}

fn check_op_registers(op: Op, win: u8) -> Result<(), QvmError> {
    let chk = |r: u8| -> Result<(), QvmError> {
        if r < win && (r as usize) < MAX_REGS {
            Ok(())
        } else {
            Err(QvmError::InvalidRegisterIndex)
        }
    };
    match op {
        Op::LoadConstQ32 { dst, .. } | Op::LoadConstI32 { dst, .. } | Op::LoadConstBool { dst, .. } => chk(dst),
        Op::MoveReg { dst, src } => {
            chk(dst)?;
            chk(src)
        }
        Op::AddQ32 { dst, a, b }
        | Op::SubQ32 { dst, a, b }
        | Op::MulQ32 { dst, a, b }
        | Op::DivQ32 { dst, a, b }
        | Op::CmpLtQ32 { dst, a, b }
        | Op::CmpGeQ32 { dst, a, b }
        | Op::AddQVec3 { dst, a, b }
        | Op::SubQVec3 { dst, a, b }
        | Op::DotQVec3 { dst, a, b }
        | Op::DistSqQVec3 { dst, a, b } => {
            chk(dst)?;
            chk(a)?;
            chk(b)
        }
        Op::MakeQVec3 { dst, x, y, z } => {
            chk(dst)?;
            chk(x)?;
            chk(y)?;
            chk(z)
        }
        Op::ScaleQVec3 { dst, v, s } => {
            chk(dst)?;
            chk(v)?;
            chk(s)
        }
        Op::FmaQVec3 { dst, v, s, bias } => {
            chk(dst)?;
            chk(v)?;
            chk(s)?;
            chk(bias)
        }
        Op::ClampQ32 { dst, val, lo, hi } => {
            chk(dst)?;
            chk(val)?;
            chk(lo)?;
            chk(hi)
        }
        Op::ReadPos { dst, entity }
        | Op::ReadVel { dst, entity }
        | Op::ReadHp { dst, entity }
        | Op::ReadShield { dst, entity }
        | Op::ReadCharge { dst, entity }
        | Op::HasTraitBit { dst, entity, .. } => {
            chk(dst)?;
            chk(entity)
        }
        Op::JumpIfFalse { cond, .. } => chk(cond),
        Op::JumpForward { .. } => Ok(()),
        Op::ForBegin { counter_reg, .. } | Op::ForStep { counter_reg, .. } => chk(counter_reg),
        Op::CallStatic {
            arg0_src,
            arg1_src,
            ret_dst,
            ..
        } => {
            chk(arg0_src)?;
            chk(arg1_src)?;
            chk(ret_dst)
        }
        Op::Return { src } => chk(src),
        Op::EmitDealDamage {
            src_ent,
            dst_ent,
            hp_dmg,
            shatter_dmg,
            charge_gain,
        } => {
            chk(src_ent)?;
            chk(dst_ent)?;
            chk(hp_dmg)?;
            chk(shatter_dmg)?;
            chk(charge_gain)
        }
        Op::EmitImpulse { dst_ent, impulse_vec } => {
            chk(dst_ent)?;
            chk(impulse_vec)
        }
        Op::EmitPhantasia {
            src_ent,
            center_vec,
            radius,
            slow_scale,
        } => {
            chk(src_ent)?;
            chk(center_vec)?;
            chk(radius)?;
            chk(slow_scale)
        }
        Op::EmitClash {
            defender_ent,
            boss_ent,
            shatter_burst,
        } => {
            chk(defender_ent)?;
            chk(boss_ent)?;
            chk(shatter_burst)
        }
        Op::EmitProjectile {
            owner_ent,
            origin_vec,
            vel_vec,
            damage,
        } => {
            chk(owner_ent)?;
            chk(origin_vec)?;
            chk(vel_vec)?;
            chk(damage)
        }
        Op::EmitBossPattern { boss_ent, .. } => chk(boss_ent),
    }
}

#[derive(Clone, Copy)]
struct ActiveFrame {
    func_idx: u16,
    pc: usize,
    base_reg: u8,
    ret_dst_abs: u8,
}

/// Executes a verified `qvm` handler on a flat pre-allocated `64`-register array (`0` host recursion!).
/// Enforces per-invocation `<= 4,000 ops` fuel ceiling; on fuel trap, atomically rolls back only
/// commands emitted during this invocation so client rollback and server remain `100%` identical.
pub fn execute_handler(
    module: &CompiledQvmModule,
    soa: &SoAView<'_>,
    self_entity: EntityId,
    target_entity: EntityId,
    out_cmds: &mut CommandBuffer,
) -> Result<u32, QvmError> {
    let checkpoint_cmd_len = out_cmds.len();
    let mut regs = [RegVal::Unit; MAX_REGS];
    regs[0] = RegVal::Entity(self_entity);
    regs[1] = RegVal::Entity(target_entity);

    let mut frames = [ActiveFrame {
        func_idx: module.entry_func,
        pc: 0,
        base_reg: 0,
        ret_dst_abs: 0,
    }; MAX_CALL_DEPTH];
    let mut depth: usize = 1;
    let mut fuel_used: u32 = 0;

    while depth > 0 {
        let top = depth.saturating_sub(1);
        let cur = frames[top];
        let Some(func) = module.funcs.get(cur.func_idx as usize) else {
            out_cmds.truncate(checkpoint_cmd_len);
            return Err(QvmError::SyntaxError("Invalid function index"));
        };
        if cur.pc >= func.code.len() {
            depth = depth.saturating_sub(1);
            continue;
        }
        let op = func.code[cur.pc];
        frames[top].pc = cur.pc.saturating_add(1);

        fuel_used = fuel_used.saturating_add(op.fuel_cost());
        if fuel_used > INVOCATION_FUEL_CAP {
            out_cmds.truncate(checkpoint_cmd_len);
            return Err(QvmError::InvocationFuelExceeded);
        }

        let b = cur.base_reg as usize;
        let end_win = b.saturating_add(func.reg_window_size as usize);
        if end_win > MAX_REGS {
            out_cmds.truncate(checkpoint_cmd_len);
            return Err(QvmError::RegisterDepthExceeded);
        }

        match op {
            Op::LoadConstQ32 { dst, raw_q32 } => {
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(Q32::from_raw(raw_q32));
            }
            Op::LoadConstI32 { dst, val } => {
                regs[b.saturating_add(dst as usize)] = RegVal::I32(val);
            }
            Op::LoadConstBool { dst, val } => {
                regs[b.saturating_add(dst as usize)] = RegVal::Bool(val);
            }
            Op::MoveReg { dst, src } => {
                regs[b.saturating_add(dst as usize)] = regs[b.saturating_add(src as usize)];
            }
            Op::AddQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(va.sat_add(vb));
            }
            Op::SubQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(va.sat_sub(vb));
            }
            Op::MulQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(va.sat_mul(vb));
            }
            Op::DivQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(va.sat_div(vb));
            }
            Op::CmpLtQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Bool(va < vb);
            }
            Op::CmpGeQ32 { dst, a, b: rb } => {
                let va = as_q32(regs[b.saturating_add(a as usize)]);
                let vb = as_q32(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Bool(va >= vb);
            }
            Op::MakeQVec3 { dst, x, y, z } => {
                let vx = as_q32(regs[b.saturating_add(x as usize)]);
                let vy = as_q32(regs[b.saturating_add(y as usize)]);
                let vz = as_q32(regs[b.saturating_add(z as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(QVec3::new(vx, vy, vz));
            }
            Op::AddQVec3 { dst, a, b: rb } => {
                let va = as_qvec3(regs[b.saturating_add(a as usize)]);
                let vb = as_qvec3(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(va.sat_add(vb));
            }
            Op::SubQVec3 { dst, a, b: rb } => {
                let va = as_qvec3(regs[b.saturating_add(a as usize)]);
                let vb = as_qvec3(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(va.sat_sub(vb));
            }
            Op::ScaleQVec3 { dst, v, s } => {
                let vv = as_qvec3(regs[b.saturating_add(v as usize)]);
                let vs = as_q32(regs[b.saturating_add(s as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(vv.scale(vs));
            }
            Op::FmaQVec3 { dst, v, s, bias } => {
                let vv = as_qvec3(regs[b.saturating_add(v as usize)]);
                let vs = as_q32(regs[b.saturating_add(s as usize)]);
                let vb = as_qvec3(regs[b.saturating_add(bias as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(vv.fma(vs, vb));
            }
            Op::ClampQ32 { dst, val, lo, hi } => {
                let vv = as_q32(regs[b.saturating_add(val as usize)]);
                let vlo = as_q32(regs[b.saturating_add(lo as usize)]);
                let vhi = as_q32(regs[b.saturating_add(hi as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(vv.clamp_val(vlo, vhi));
            }
            Op::DotQVec3 { dst, a, b: rb } => {
                let va = as_qvec3(regs[b.saturating_add(a as usize)]);
                let vb = as_qvec3(regs[b.saturating_add(rb as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(va.dot_q32(vb));
            }
            Op::DistSqQVec3 { dst, a, b: rb } => {
                let va = as_qvec3(regs[b.saturating_add(a as usize)]);
                let vb = as_qvec3(regs[b.saturating_add(rb as usize)]);
                let d = Q32::saturate_i128(va.dist_sq_wide(vb).0 >> 32);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(d);
            }
            Op::ReadPos { dst, entity } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(soa.pos(ent));
            }
            Op::ReadVel { dst, entity } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::QVec3(soa.vel(ent));
            }
            Op::ReadHp { dst, entity } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(soa.hp(ent));
            }
            Op::ReadShield { dst, entity } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(soa.shield(ent));
            }
            Op::ReadCharge { dst, entity } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                regs[b.saturating_add(dst as usize)] = RegVal::Q32(soa.charge(ent));
            }
            Op::HasTraitBit { dst, entity, bit } => {
                let ent = as_entity(regs[b.saturating_add(entity as usize)]);
                let mask = soa.trait_mask(ent);
                let has = (mask & (1_u32.wrapping_shl((bit & 31) as u32))) != 0;
                regs[b.saturating_add(dst as usize)] = RegVal::Bool(has);
            }
            Op::JumpIfFalse { cond, target_pc } => {
                if !as_bool(regs[b.saturating_add(cond as usize)]) {
                    frames[top].pc = target_pc as usize;
                }
            }
            Op::JumpForward { target_pc } => {
                frames[top].pc = target_pc as usize;
            }
            Op::ForBegin {
                counter_reg,
                max_iters,
                exit_pc,
            } => {
                if max_iters == 0 {
                    frames[top].pc = exit_pc as usize;
                } else {
                    regs[b.saturating_add(counter_reg as usize)] = RegVal::I32(0);
                }
            }
            Op::ForStep {
                counter_reg,
                max_iters,
                body_pc,
            } => {
                let next_i = as_i32(regs[b.saturating_add(counter_reg as usize)]).saturating_add(1);
                regs[b.saturating_add(counter_reg as usize)] = RegVal::I32(next_i);
                if next_i < (max_iters as i32) {
                    frames[top].pc = body_pc as usize;
                }
            }
            Op::CallStatic {
                func_idx,
                arg0_src,
                arg1_src,
                ret_dst,
                frame_base,
            } => {
                let callee_base = b.saturating_add(frame_base as usize);
                if depth >= MAX_CALL_DEPTH || callee_base.saturating_add(1) >= MAX_REGS {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::RegisterDepthExceeded);
                }
                let a0 = regs[b.saturating_add(arg0_src as usize)];
                let a1 = regs[b.saturating_add(arg1_src as usize)];
                regs[callee_base] = a0;
                regs[callee_base.saturating_add(1)] = a1;
                frames[depth] = ActiveFrame {
                    func_idx,
                    pc: 0,
                    base_reg: callee_base as u8,
                    ret_dst_abs: b.saturating_add(ret_dst as usize) as u8,
                };
                depth = depth.saturating_add(1);
            }
            Op::Return { src } => {
                let ret_val = regs[b.saturating_add(src as usize)];
                depth = depth.saturating_sub(1);
                if depth > 0 {
                    regs[cur.ret_dst_abs as usize] = ret_val;
                }
            }
            Op::EmitDealDamage {
                src_ent,
                dst_ent,
                hp_dmg,
                shatter_dmg,
                charge_gain,
            } => {
                let cmd = SimCommand::DealDamage {
                    source: as_entity(regs[b.saturating_add(src_ent as usize)]),
                    target: as_entity(regs[b.saturating_add(dst_ent as usize)]),
                    hp_damage: as_q32(regs[b.saturating_add(hp_dmg as usize)]),
                    shatter_damage: as_q32(regs[b.saturating_add(shatter_dmg as usize)]),
                    charge_gain: as_q32(regs[b.saturating_add(charge_gain as usize)]),
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
            Op::EmitImpulse { dst_ent, impulse_vec } => {
                let cmd = SimCommand::ApplyImpulse {
                    target: as_entity(regs[b.saturating_add(dst_ent as usize)]),
                    impulse: as_qvec3(regs[b.saturating_add(impulse_vec as usize)]),
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
            Op::EmitPhantasia {
                src_ent,
                center_vec,
                radius,
                slow_scale,
            } => {
                let cmd = SimCommand::TriggerPhantasia {
                    source: as_entity(regs[b.saturating_add(src_ent as usize)]),
                    center: as_qvec3(regs[b.saturating_add(center_vec as usize)]),
                    radius: as_q32(regs[b.saturating_add(radius as usize)]),
                    slow_scale: as_q32(regs[b.saturating_add(slow_scale as usize)]),
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
            Op::EmitClash {
                defender_ent,
                boss_ent,
                shatter_burst,
            } => {
                let cmd = SimCommand::TriggerClash {
                    defender: as_entity(regs[b.saturating_add(defender_ent as usize)]),
                    boss: as_entity(regs[b.saturating_add(boss_ent as usize)]),
                    shatter_burst: as_q32(regs[b.saturating_add(shatter_burst as usize)]),
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
            Op::EmitProjectile {
                owner_ent,
                origin_vec,
                vel_vec,
                damage,
            } => {
                let cmd = SimCommand::SpawnProjectile {
                    owner: as_entity(regs[b.saturating_add(owner_ent as usize)]),
                    origin: as_qvec3(regs[b.saturating_add(origin_vec as usize)]),
                    velocity: as_qvec3(regs[b.saturating_add(vel_vec as usize)]),
                    damage: as_q32(regs[b.saturating_add(damage as usize)]),
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
            Op::EmitBossPattern { boss_ent, pattern_id } => {
                let cmd = SimCommand::MutateBossPattern {
                    boss: as_entity(regs[b.saturating_add(boss_ent as usize)]),
                    next_pattern: pattern_id,
                };
                if out_cmds.push(cmd).is_err() {
                    out_cmds.truncate(checkpoint_cmd_len);
                    return Err(QvmError::CommandBufferOverflow);
                }
            }
        }
    }
    Ok(fuel_used)
}

#[inline]
const fn as_q32(v: RegVal) -> Q32 {
    match v {
        RegVal::Q32(q) => q,
        _ => Q32::ZERO,
    }
}

#[inline]
const fn as_qvec3(v: RegVal) -> QVec3 {
    match v {
        RegVal::QVec3(vec) => vec,
        _ => QVec3::ZERO,
    }
}

#[inline]
const fn as_entity(v: RegVal) -> EntityId {
    match v {
        RegVal::Entity(e) => e,
        _ => EntityId(u32::MAX, 0),
    }
}

#[inline]
const fn as_bool(v: RegVal) -> bool {
    match v {
        RegVal::Bool(b) => b,
        _ => false,
    }
}

#[inline]
const fn as_i32(v: RegVal) -> i32 {
    match v {
        RegVal::I32(i) => i,
        _ => 0,
    }
}

// ============================================================================
// Fast (`<= 0.5ms`) Pure-Rust `qvm` Source Compiler + Acyclic `DAG` Checker
// ============================================================================

/// Compiles `.qvm` declarative source text into verified fixed-depth register bytecode (`<= 0.5ms`),
/// enforcing zero `while`/`loop`, zero floats, zero OS symbols, and strict `DAG` call acyclicity.
pub fn compile_qvm_source(src: &str) -> Result<CompiledQvmModule, QvmError> {
    for line in src.lines() {
        let trimmed = line.split("//").next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        for tok in trimmed.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            match tok {
                "while" | "loop" => return Err(QvmError::ForbiddenConstruct("Unbounded loop keyword forbidden")),
                "static" | "global" => return Err(QvmError::ForbiddenConstruct("Mutable/global state forbidden")),
                "closure" | "move" => return Err(QvmError::ForbiddenConstruct("Closure upvalue forbidden")),
                "float" | "double" => return Err(QvmError::ForbiddenConstruct("Floating-point type forbidden")),
                "clock" | "time" | "env" | "fs" | "os" => {
                    return Err(QvmError::ForbiddenConstruct("Host OS/clock symbol forbidden"))
                }
                _ => {}
            }
        }
    }

    let mut raw_funcs: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut cur_name: Option<&str> = None;
    let mut cur_lines: Vec<&str> = Vec::new();

    for raw_line in src.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("fn ") {
            if cur_name.is_some() {
                return Err(QvmError::SyntaxError("Nested function forbidden"));
            }
            let fname = rest
                .split(|c: char| c == '(' || c == '{' || c.is_ascii_whitespace())
                .next()
                .unwrap_or("");
            if fname.is_empty() {
                return Err(QvmError::SyntaxError("Empty function name"));
            }
            cur_name = Some(fname);
            cur_lines.clear();
        } else if line == "}" {
            let Some(name) = cur_name.take() else {
                return Err(QvmError::SyntaxError("Unmatched closing brace"));
            };
            raw_funcs.push((name, cur_lines.clone()));
            cur_lines.clear();
        } else if cur_name.is_some() {
            cur_lines.push(line);
        } else {
            return Err(QvmError::SyntaxError("Top-level statement outside fn"));
        }
    }
    if cur_name.is_some() || raw_funcs.is_empty() {
        return Err(QvmError::SyntaxError("Incomplete or empty qvm module"));
    }

    let mut name_to_idx: BTreeMap<&str, usize> = BTreeMap::new();
    for (idx, &(name, _)) in raw_funcs.iter().enumerate() {
        if name_to_idx.insert(name, idx).is_some() {
            return Err(QvmError::SyntaxError("Duplicate function name"));
        }
    }

    let n = raw_funcs.len();
    let mut adj: Vec<BTreeSet<usize>> = alloc::vec![BTreeSet::new(); n];
    for (u, (_, body)) in raw_funcs.iter().enumerate() {
        for stmt in body {
            if let Some(call_rest) = stmt.strip_prefix("call ") {
                let callee_name = call_rest
                    .split(|c: char| c == '(' || c.is_ascii_whitespace())
                    .next()
                    .unwrap_or("");
                let Some(&v) = name_to_idx.get(callee_name) else {
                    return Err(QvmError::SyntaxError("Unknown callee function"));
                };
                if u == v {
                    return Err(QvmError::RecursiveCallCycle);
                }
                adj[u].insert(v);
            }
        }
    }

    let mut color: Vec<u8> = alloc::vec![0; n];
    let mut post_order: Vec<usize> = Vec::with_capacity(n);
    for start in 0..n {
        if color[start] == 0 {
            dfs_dag(start, &adj, &mut color, &mut post_order)?;
        }
    }

    let mut topo_ranks: Vec<u16> = alloc::vec![0; n];
    for (rank, &f_idx) in post_order.iter().enumerate() {
        topo_ranks[f_idx] = rank as u16;
    }

    let entry = *name_to_idx.get("on_tick").unwrap_or(&0);
    let mut reachable: Vec<bool> = alloc::vec![false; n];
    reachable[entry] = true;
    let mut r_stack: Vec<usize> = alloc::vec![entry];
    while let Some(cur) = r_stack.pop() {
        for &nxt in &adj[cur] {
            if !reachable[nxt] {
                reachable[nxt] = true;
                r_stack.push(nxt);
            }
        }
    }

    let mut compiled_funcs: Vec<CompiledFunc> = alloc::vec![
        CompiledFunc {
            name: String::new(),
            topo_rank: 0,
            reg_window_size: 2,
            code: Vec::new(),
        };
        n
    ];
    let mut causal_nodes: Vec<AnonMechRole> = alloc::vec![AnonMechRole::InputTrigger];
    let mut causal_edges: Vec<CausalEdge> = Vec::new();

    for &f_idx in &post_order {
        let emit_causal = reachable[f_idx];
        let (fname, ref body) = raw_funcs[f_idx];
        let mut ops: Vec<Op> = Vec::new();
        let mut reg_map: BTreeMap<&str, u8> = BTreeMap::new();
        reg_map.insert("self", 0);
        reg_map.insert("target", 1);
        let mut next_reg: u8 = 2;
        let mut loop_stack: Vec<(u8, u16, usize)> = Vec::new();

        for &stmt in body {
            if let Some(rest) = stmt.strip_prefix("const_q32 ") {
                let (vname, val_str) = split_assign(rest)?;
                let int_val = parse_i32(val_str)?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::LoadConstQ32 {
                    dst,
                    raw_q32: Q32::from_int(int_val).raw(),
                });
            } else if let Some(rest) = stmt.strip_prefix("read_shield ") {
                let (vname, ent_name) = split_assign(rest)?;
                let ent = lookup_reg(&reg_map, ent_name)?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::ReadShield { dst, entity: ent });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::StateGate, 1);
                }
            } else if let Some(rest) = stmt.strip_prefix("read_pos ") {
                let (vname, ent_name) = split_assign(rest)?;
                let ent = lookup_reg(&reg_map, ent_name)?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::ReadPos { dst, entity: ent });
            } else if let Some(rest) = stmt.strip_prefix("read_vel ") {
                let (vname, ent_name) = split_assign(rest)?;
                let ent = lookup_reg(&reg_map, ent_name)?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::ReadVel { dst, entity: ent });
            } else if let Some(rest) = stmt.strip_prefix("fma_vec3 ") {
                let (vname, rhs) = split_assign(rest)?;
                let parts = split_csv3(rhs)?;
                let v = lookup_reg(&reg_map, parts[0])?;
                let s = lookup_reg(&reg_map, parts[1])?;
                let bias = lookup_reg(&reg_map, parts[2])?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::FmaQVec3 { dst, v, s, bias });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::SpatialImpulse, 2);
                }
            } else if let Some(rest) = stmt.strip_prefix("clamp_q32 ") {
                let (vname, rhs) = split_assign(rest)?;
                let parts = split_csv3(rhs)?;
                let val = lookup_reg(&reg_map, parts[0])?;
                let lo = lookup_reg(&reg_map, parts[1])?;
                let hi = lookup_reg(&reg_map, parts[2])?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::ClampQ32 { dst, val, lo, hi });
            } else if let Some(rest) = stmt.strip_prefix("add_q32 ") {
                let (vname, rhs) = split_assign(rest)?;
                let parts = split_csv2(rhs)?;
                let a = lookup_reg(&reg_map, parts[0])?;
                let b = lookup_reg(&reg_map, parts[1])?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::AddQ32 { dst, a, b });
            } else if let Some(rest) = stmt.strip_prefix("mul_q32 ") {
                let (vname, rhs) = split_assign(rest)?;
                let parts = split_csv2(rhs)?;
                let a = lookup_reg(&reg_map, parts[0])?;
                let b = lookup_reg(&reg_map, parts[1])?;
                let dst = alloc_reg(&mut reg_map, vname, &mut next_reg)?;
                ops.push(Op::MulQ32 { dst, a, b });
            } else if let Some(rest) = stmt.strip_prefix("for ") {
                if loop_stack.len() >= MAX_LOOP_NESTING {
                    return Err(QvmError::StaticLoopBoundExceeded);
                }
                let hdr = rest.trim_end_matches('{').trim();
                let mut it = hdr.split(" in 0..");
                let cname = it.next().unwrap_or("").trim();
                let bound_str = it.next().unwrap_or("").trim();
                let bound_i = parse_i32(bound_str)?;
                if bound_i <= 0 || bound_i > (MAX_STATIC_LOOP_ITERS as i32) {
                    return Err(QvmError::StaticLoopBoundExceeded);
                }
                let creg = alloc_reg(&mut reg_map, cname, &mut next_reg)?;
                let begin_idx = ops.len();
                ops.push(Op::ForBegin {
                    counter_reg: creg,
                    max_iters: bound_i as u16,
                    exit_pc: 0,
                });
                loop_stack.push((creg, bound_i as u16, begin_idx));
            } else if stmt == "endfor" {
                let Some((creg, max_iters, begin_idx)) = loop_stack.pop() else {
                    return Err(QvmError::SyntaxError("Unmatched endfor"));
                };
                let body_pc = begin_idx.saturating_add(1) as u16;
                ops.push(Op::ForStep {
                    counter_reg: creg,
                    max_iters,
                    body_pc,
                });
                let exit_pc = ops.len() as u16;
                ops[begin_idx] = Op::ForBegin {
                    counter_reg: creg,
                    max_iters,
                    exit_pc,
                };
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::ProjectileSplit, 3);
                }
            } else if let Some(rest) = stmt.strip_prefix("call ") {
                let mut arrow = rest.split("->");
                let lhs = arrow.next().unwrap_or("").trim();
                let dst_name = arrow.next().unwrap_or("r_ret").trim();
                let cname = lhs.split('(').next().unwrap_or("").trim();
                let callee_idx = *name_to_idx
                    .get(cname)
                    .ok_or(QvmError::SyntaxError("Unknown callee"))?;
                let dst = alloc_reg(&mut reg_map, dst_name, &mut next_reg)?;
                ops.push(Op::CallStatic {
                    func_idx: callee_idx as u16,
                    arg0_src: 0,
                    arg1_src: 1,
                    ret_dst: dst,
                    frame_base: 0, // Patched below to final `next_reg` (`caller_reg_window_size`)!
                });
            } else if let Some(rest) = stmt.strip_prefix("emit_damage ") {
                let parts = split_csv3(rest)?;
                let hp_r = lookup_reg(&reg_map, parts[0])?;
                let sh_r = lookup_reg(&reg_map, parts[1])?;
                let ch_r = lookup_reg(&reg_map, parts[2])?;
                ops.push(Op::EmitDealDamage {
                    src_ent: 0,
                    dst_ent: 1,
                    hp_dmg: hp_r,
                    shatter_dmg: sh_r,
                    charge_gain: ch_r,
                });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::ShatterBreak, 4);
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::WeaponCharge, 5);
                }
            } else if let Some(rest) = stmt.strip_prefix("emit_phantasia ") {
                let parts = split_csv3(rest)?;
                let c_vec = lookup_reg(&reg_map, parts[0])?;
                let rad = lookup_reg(&reg_map, parts[1])?;
                let slow = lookup_reg(&reg_map, parts[2])?;
                ops.push(Op::EmitPhantasia {
                    src_ent: 0,
                    center_vec: c_vec,
                    radius: rad,
                    slow_scale: slow,
                });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::PhantasiaSlowField, 6);
                }
            } else if let Some(rest) = stmt.strip_prefix("emit_clash ") {
                let sh_r = lookup_reg(&reg_map, rest.trim())?;
                ops.push(Op::EmitClash {
                    defender_ent: 0,
                    boss_ent: 1,
                    shatter_burst: sh_r,
                });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::ClashParry, 7);
                }
            } else if let Some(rest) = stmt.strip_prefix("emit_projectile ") {
                let parts = split_csv3(rest)?;
                let orig = lookup_reg(&reg_map, parts[0])?;
                let vel = lookup_reg(&reg_map, parts[1])?;
                let dmg = lookup_reg(&reg_map, parts[2])?;
                ops.push(Op::EmitProjectile {
                    owner_ent: 0,
                    origin_vec: orig,
                    vel_vec: vel,
                    damage: dmg,
                });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::ProjectileSplit, 8);
                }
            } else if let Some(rest) = stmt.strip_prefix("emit_impulse ") {
                let imp = lookup_reg(&reg_map, rest.trim())?;
                ops.push(Op::EmitImpulse {
                    dst_ent: 0,
                    impulse_vec: imp,
                });
                if emit_causal {
                    push_causal(&mut causal_nodes, &mut causal_edges, AnonMechRole::SpatialImpulse, 9);
                }
            } else if let Some(rest) = stmt.strip_prefix("return ") {
                let src_r = lookup_reg(&reg_map, rest.trim())?;
                ops.push(Op::Return { src: src_r });
            } else {
                return Err(QvmError::SyntaxError("Unrecognized qvm statement"));
            }
        }
        if !loop_stack.is_empty() {
            return Err(QvmError::SyntaxError("Unclosed for loop"));
        }
        ops.push(Op::Return { src: 0 });

        // Patch every `CallStatic.frame_base` to `next_reg` (`caller_reg_window_size`) so callee window
        // `[caller_base + next_reg ..]` is strictly disjoint from all caller registers `[caller_base .. caller_base + next_reg)`!
        for op in &mut ops {
            if let Op::CallStatic { frame_base, .. } = op {
                *frame_base = next_reg;
            }
        }

        compiled_funcs[f_idx] = CompiledFunc {
            name: String::from(fname),
            topo_rank: topo_ranks[f_idx],
            reg_window_size: next_reg,
            code: ops,
        };
    }

    let mut max_depth_for_func: Vec<usize> = alloc::vec![0; n];
    for &f_idx in &post_order {
        let base_win = compiled_funcs[f_idx].reg_window_size as usize;
        let mut peak = base_win;
        for &op in &compiled_funcs[f_idx].code {
            if let Op::CallStatic {
                func_idx,
                frame_base,
                ..
            } = op
            {
                let sub = (frame_base as usize).saturating_add(max_depth_for_func[func_idx as usize]);
                if sub > MAX_REGS {
                    return Err(QvmError::RegisterDepthExceeded);
                }
                if sub > peak {
                    peak = sub;
                }
            }
        }
        max_depth_for_func[f_idx] = peak;
    }

    let module = CompiledQvmModule {
        max_static_reg_depth: max_depth_for_func[entry] as u8,
        entry_func: entry as u16,
        funcs: compiled_funcs,
        causal_graph: CausalGraphIR {
            nodes: causal_nodes,
            causal_edges,
        },
    };
    verify_compiled_module(&module)?;
    Ok(module)
}

fn dfs_dag(
    u: usize,
    adj: &[BTreeSet<usize>],
    color: &mut [u8],
    post_order: &mut Vec<usize>,
) -> Result<(), QvmError> {
    color[u] = 1;
    for &v in &adj[u] {
        if color[v] == 1 {
            return Err(QvmError::RecursiveCallCycle);
        }
        if color[v] == 0 {
            dfs_dag(v, adj, color, post_order)?;
        }
    }
    color[u] = 2;
    post_order.push(u);
    Ok(())
}

fn push_causal(nodes: &mut Vec<AnonMechRole>, edges: &mut Vec<CausalEdge>, role: AnonMechRole, kind: u8) {
    let prev = (nodes.len().saturating_sub(1)) as u16;
    let cur = nodes.len() as u16;
    nodes.push(role);
    edges.push(CausalEdge {
        src: prev,
        dst: cur,
        edge_kind: kind,
    });
}

fn split_assign(s: &str) -> Result<(&str, &str), QvmError> {
    let mut it = s.splitn(2, '=');
    let lhs = it.next().unwrap_or("").trim();
    let rhs = it.next().ok_or(QvmError::SyntaxError("Expected '='"))?.trim();
    if lhs.is_empty() || rhs.is_empty() {
        return Err(QvmError::SyntaxError("Empty assignment operand"));
    }
    Ok((lhs, rhs))
}

fn split_csv2(s: &str) -> Result<[&str; 2], QvmError> {
    let mut it = s.split(',');
    let a = it.next().ok_or(QvmError::SyntaxError("Expected 2 args"))?.trim();
    let b = it.next().ok_or(QvmError::SyntaxError("Expected 2 args"))?.trim();
    Ok([a, b])
}

fn split_csv3(s: &str) -> Result<[&str; 3], QvmError> {
    let mut it = s.split(',');
    let a = it.next().ok_or(QvmError::SyntaxError("Expected 3 args"))?.trim();
    let b = it.next().ok_or(QvmError::SyntaxError("Expected 3 args"))?.trim();
    let c = it.next().ok_or(QvmError::SyntaxError("Expected 3 args"))?.trim();
    Ok([a, b, c])
}

fn parse_i32(s: &str) -> Result<i32, QvmError> {
    if s.contains('.') {
        return Err(QvmError::ForbiddenConstruct("Floating-point literal forbidden"));
    }
    s.trim()
        .parse::<i32>()
        .map_err(|_| QvmError::SyntaxError("Invalid integer literal"))
}

fn alloc_reg<'a>(map: &mut BTreeMap<&'a str, u8>, name: &'a str, next_reg: &mut u8) -> Result<u8, QvmError> {
    if let Some(&existing) = map.get(name) {
        return Ok(existing);
    }
    if (*next_reg as usize) >= MAX_REGS {
        return Err(QvmError::RegisterDepthExceeded);
    }
    let r = *next_reg;
    *next_reg = next_reg.saturating_add(1);
    map.insert(name, r);
    Ok(r)
}

fn lookup_reg(map: &BTreeMap<&str, u8>, name: &str) -> Result<u8, QvmError> {
    map.get(name)
        .copied()
        .ok_or(QvmError::SyntaxError("Undefined register variable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecs_soa::{CombatArena, EntityKind};

    #[test]
    fn test_dag_rejects_direct_and_mutual_recursion() {
        let self_rec = "
            fn on_tick(self, target) {
                call on_tick(self, target) -> r0
            }
        ";
        assert_eq!(compile_qvm_source(self_rec), Err(QvmError::RecursiveCallCycle));

        let mutual_rec = "
            fn a(self, target) {
                call b(self, target) -> r0
            }
            fn b(self, target) {
                call a(self, target) -> r0
            }
        ";
        assert_eq!(compile_qvm_source(mutual_rec), Err(QvmError::RecursiveCallCycle));

        let while_src = "
            fn on_tick(self, target) {
                while true {}
            }
        ";
        assert!(matches!(
            compile_qvm_source(while_src),
            Err(QvmError::ForbiddenConstruct(_))
        ));
    }

    #[test]
    fn test_multi_level_call_windows_and_nested_for_loops_zero_clobber() {
        let src = "
            fn leaf_fn(self, target) {
                const_q32 c1 = 10
                const_q32 c2 = 3
                mul_q32 res = c1, c2
                return res
            }
            fn mid_fn(self, target) {
                const_q32 pre_local = 50
                call leaf_fn(self, target) -> sub_res
                const_q32 post_local = 20
                add_q32 tmp = pre_local, sub_res
                add_q32 out = tmp, post_local
                return out
            }
            fn on_tick(self, target) {
                const_q32 outer_keep = 5
                call mid_fn(self, target) -> base_hp
                const_q32 sh_dmg = 35
                add_q32 hp_dmg = base_hp, outer_keep
                const_q32 ch_gain = 15
                for i in 0..2 {
                    for j in 0..2 {
                        emit_damage hp_dmg, sh_dmg, ch_gain
                    endfor
                endfor
            }
        ";
        let compiled = compile_qvm_source(src);
        assert!(compiled.is_ok());
        if let Ok(module) = compiled {
            let mut arena = CombatArena::new_boxed(99);
            let p = arena.spawn(EntityKind::Player, QVec3::from_ints(0, 5, 0), Q32::from_int(1000), Q32::ZERO);
            let b = arena.spawn(EntityKind::Boss, QVec3::from_ints(3, 5, 0), Q32::from_int(2000), Q32::from_int(300));
            if let (Some(pe), Some(be)) = (p, b) {
                let mut cmds = CommandBuffer::with_capacity(8);
                let fuel = execute_handler(&module, &arena.view(), pe, be, &mut cmds);
                assert!(fuel.is_ok());
                // 2 * 2 = 4 emitted commands, each with sh_dmg=35 -> shield 300 - 140 = 160,
                // and hp_dmg = (50 + (10*3) + 20) + 5 = 105 (shielded 0.5x = 52.5 * 4 = 210 -> hp 1790)!
                assert_eq!(cmds.len(), 4);
                arena.commit_commands(&mut cmds);
                assert_eq!(arena.view().shield(be), Q32::from_int(160));
                assert_eq!(arena.view().hp(be), Q32::from_int(1790));
            }
        }
    }
}
