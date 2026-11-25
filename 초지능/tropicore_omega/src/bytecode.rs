//! Shared CPU/GPU Flat Stack Bytecode VM for High-Throughput Batch OE Evaluation.
//!
//! Instruction encoding matches `cuda/tropicore_omega_sm120.cu` (`omega_eval_bytecode_batch_kernel`)
//! 1:1 so millions of candidate `OmegaExpr` programs compile to compact flat bytecode and evaluate
//! identically across multi-core CPU threads today AND 4x RTX PRO 6000 Blackwell warps on Day-1!

use crate::ir::{ConceptResolver, OmegaExpr};

pub const OP_ARG0: u8 = 0;
pub const OP_ARG1: u8 = 1;
pub const OP_ARG2: u8 = 2;
pub const OP_CONST: u8 = 3; // followed by f32 literal index in `consts`
pub const OP_ADD: u8 = 4;
pub const OP_SUB: u8 = 5;
pub const OP_MUL: u8 = 6;
pub const OP_DIVSAFE: u8 = 7;
pub const OP_MIN: u8 = 8;
pub const OP_MAX: u8 = 9;
pub const OP_ABS: u8 = 10;
pub const OP_SQRTSAFE: u8 = 11;
pub const OP_IFPOS: u8 = 12; // pops [cond, then, else] -> pushes if cond > 0 { then } else { else }
pub const OP_END: u8 = 255;

#[derive(Clone, Debug)]
pub struct FlatProgram {
    pub ops: Vec<u8>,
    pub consts: Vec<f32>,
}

impl FlatProgram {
    /// Compile any scalar `OmegaExpr` (inlining `ConceptCall` & `Let`) into flat stack bytecode.
    pub fn compile<R: ConceptResolver>(expr: &OmegaExpr, resolver: &R) -> Option<Self> {
        let inlined = expr.inline_all(resolver, 8);
        let mut ops = Vec::with_capacity(32);
        let mut consts = Vec::with_capacity(8);

        fn expand_lets(e: &OmegaExpr) -> OmegaExpr {
            match e {
                OmegaExpr::Let(b, body) => {
                    let eb = expand_lets(b);
                    let mut sub = vec![eb];
                    sub.push(OmegaExpr::Arg(0));
                    sub.push(OmegaExpr::Arg(1));
                    sub.push(OmegaExpr::Arg(2));
                    expand_lets(&body.substitute(&sub))
                }
                OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(expand_lets(a)), Box::new(expand_lets(b))),
                OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(expand_lets(a))),
                OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(expand_lets(a))),
                OmegaExpr::IfPos(c, t, el) => OmegaExpr::IfPos(
                    Box::new(expand_lets(c)),
                    Box::new(expand_lets(t)),
                    Box::new(expand_lets(el)),
                ),
                other => other.clone(),
            }
        }

        fn emit(e: &OmegaExpr, ops: &mut Vec<u8>, consts: &mut Vec<f32>) -> bool {
            if ops.len() > 240 {
                return false;
            }
            match e {
                OmegaExpr::Arg(0) => ops.push(OP_ARG0),
                OmegaExpr::Arg(1) => ops.push(OP_ARG1),
                OmegaExpr::Arg(2) => ops.push(OP_ARG2),
                OmegaExpr::Const(c) => {
                    if consts.len() >= 250 {
                        return false;
                    }
                    let idx = consts.len() as u8;
                    consts.push(*c as f32);
                    ops.push(OP_CONST);
                    ops.push(idx);
                }
                OmegaExpr::Add(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_ADD);
                }
                OmegaExpr::Sub(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_SUB);
                }
                OmegaExpr::Mul(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_MUL);
                }
                OmegaExpr::DivSafe(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_DIVSAFE);
                }
                OmegaExpr::Min(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_MIN);
                }
                OmegaExpr::Max(a, b) => {
                    if !emit(a, ops, consts) || !emit(b, ops, consts) { return false; }
                    ops.push(OP_MAX);
                }
                OmegaExpr::Abs(a) => {
                    if !emit(a, ops, consts) { return false; }
                    ops.push(OP_ABS);
                }
                OmegaExpr::SqrtSafe(a) => {
                    if !emit(a, ops, consts) { return false; }
                    ops.push(OP_SQRTSAFE);
                }
                OmegaExpr::IfPos(c, t, el) => {
                    if !emit(c, ops, consts) || !emit(t, ops, consts) || !emit(el, ops, consts) { return false; }
                    ops.push(OP_IFPOS);
                }
                _ => return false,
            }
            true
        }

        let flat_ast = expand_lets(&inlined);
        if !emit(&flat_ast, &mut ops, &mut consts) {
            return None;
        }
        ops.push(OP_END);
        Some(Self { ops, consts })
    }

    /// Execute flat bytecode on `(x0, x1, x2)` matching the SM120 GPU warp interpreter 1:1.
    #[inline]
    pub fn eval_f32(&self, x0: f32, x1: f32, x2: f32) -> f32 {
        let mut st = [0.0f32; 32];
        let mut sp = 0usize;
        let mut pc = 0usize;
        let n = self.ops.len();
        while pc < n {
            let op = self.ops[pc];
            pc += 1;
            match op {
                OP_ARG0 => { if sp < 32 { st[sp] = x0; sp += 1; } }
                OP_ARG1 => { if sp < 32 { st[sp] = x1; sp += 1; } }
                OP_ARG2 => { if sp < 32 { st[sp] = x2; sp += 1; } }
                OP_CONST => {
                    if pc < n {
                        let idx = self.ops[pc] as usize;
                        pc += 1;
                        if sp < 32 { st[sp] = *self.consts.get(idx).unwrap_or(&0.0); sp += 1; }
                    }
                }
                OP_ADD => if sp >= 2 { sp -= 1; st[sp - 1] = (st[sp - 1] + st[sp]).clamp(-1e9, 1e9); },
                OP_SUB => if sp >= 2 { sp -= 1; st[sp - 1] = (st[sp - 1] - st[sp]).clamp(-1e9, 1e9); },
                OP_MUL => if sp >= 2 { sp -= 1; st[sp - 1] = (st[sp - 1] * st[sp]).clamp(-1e9, 1e9); },
                OP_DIVSAFE => if sp >= 2 {
                    sp -= 1;
                    let d = st[sp];
                    let sd = if d.abs() < 1e-3 { if d >= 0.0 { 1e-3 } else { -1e-3 } } else { d };
                    st[sp - 1] = (st[sp - 1] / sd).clamp(-1e9, 1e9);
                },
                OP_MIN => if sp >= 2 { sp -= 1; st[sp - 1] = st[sp - 1].min(st[sp]); },
                OP_MAX => if sp >= 2 { sp -= 1; st[sp - 1] = st[sp - 1].max(st[sp]); },
                OP_ABS => if sp >= 1 { st[sp - 1] = st[sp - 1].abs(); },
                OP_SQRTSAFE => if sp >= 1 {
                    let v = st[sp - 1];
                    st[sp - 1] = if v > 0.0 { v.sqrt() } else { 0.0 };
                },
                OP_IFPOS => if sp >= 3 {
                    let el = st[sp - 1];
                    let th = st[sp - 2];
                    let c = st[sp - 3];
                    sp -= 2;
                    st[sp - 1] = if c > 0.0 { th } else { el };
                },
                OP_END => break,
                _ => break,
            }
        }
        if sp > 0 && st[sp - 1].is_finite() { st[sp - 1] } else { 0.0 }
    }
}

/// Pack a batch of `FlatProgram`s into contiguous device-ready buffers for SM120 / multi-thread CPU execution.
#[cfg_attr(not(feature = "cuda_sm120"), allow(dead_code))]
pub struct PackedBytecodeBatch {
    pub ops: Vec<u8>,
    pub op_offsets: Vec<u32>,
    pub consts: Vec<f32>,
    pub const_offsets: Vec<u32>,
    pub num_progs: usize,
}

impl PackedBytecodeBatch {
    pub fn pack(progs: &[FlatProgram]) -> Self {
        let mut ops = Vec::new();
        let mut op_offsets = Vec::with_capacity(progs.len());
        let mut consts = Vec::new();
        let mut const_offsets = Vec::with_capacity(progs.len());
        for p in progs {
            op_offsets.push(ops.len() as u32);
            ops.extend_from_slice(&p.ops);
            const_offsets.push(consts.len() as u32);
            consts.extend_from_slice(&p.consts);
        }
        Self { ops, op_offsets, consts, const_offsets, num_progs: progs.len() }
    }
}
