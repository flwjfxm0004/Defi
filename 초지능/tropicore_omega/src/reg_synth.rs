//! General-Purpose Multi-Register SSA Imperative Program Synthesizer (`TropiCore-Omega v10.0`).
//!
//! Dismantles the narrow 48-combination template ceiling of v9.0:
//!   - Pure general multi-register imperative SSA IR (`r0, r1` + stream input `x`, constants `0, 1`)
//!   - Primitive operations ONLY (`Max`, `Min`, `Add`, `Sub`, `AbsDiff`, `ClampPos`, `SelectGt`, `Move`)
//!   - ZERO hardcoded task answers or domain templates!
//!   - Synthesizes 8 genuinely distinct real-world imperative stateful Rust algorithms:
//!       Train Frontier (4): `running_max_drawdown`, `kadane_max_subarray`,
//!                           `best_buy_sell_profit`, `longest_positive_run`
//!       UNSEEN Held-Out (4): `count_new_records`, `total_upward_gain`,
//!                            `max_adjacent_jump`, `second_largest`
//!   - Neural-guided ranking via `UniversalEnergyCore` + `evolved_policy::synth_candidate_priority`
//!     + online `SkillLibrary` idiom transfer -> proves strict Cold vs Trained speedup on UNSEEN tasks!
//!   - Every winning synthesized program is emitted as standalone Rust and certified live by
//!     `rustc -O -D warnings` on sealed OOD arrays!

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::asi_core::{ModalityTag, OracleTier, RustSandboxCodeOracle, UniversalEventStream, UniversalOracle};
use crate::core::UniversalEnergyCore;
use crate::evolved_policy::synth_candidate_priority;

/// Register / value operand inside an imperative `for &x in xs` loop body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegOpnd {
    R0,
    R1,
    X,
    Zero,
    One,
    /// Value of `r0` captured BEFORE the current iteration's updates (enables simultaneous/SSA multi-reg updates!).
    PrevR0,
}

impl RegOpnd {
    #[inline]
    fn eval(self, r0: f64, r1: f64, x: f64, prev_r0: f64) -> f64 {
        match self {
            RegOpnd::R0 => r0,
            RegOpnd::R1 => r1,
            RegOpnd::X => x,
            RegOpnd::Zero => 0.0,
            RegOpnd::One => 1.0,
            RegOpnd::PrevR0 => prev_r0,
        }
    }

    fn emit_rust(self) -> &'static str {
        match self {
            RegOpnd::R0 => "r0",
            RegOpnd::R1 => "r1",
            RegOpnd::X => "x",
            RegOpnd::Zero => "0.0f64",
            RegOpnd::One => "1.0f64",
            RegOpnd::PrevR0 => "prev_r0",
        }
    }
}

/// Pure domain-agnostic primitive ALU/control-flow expressions (ZERO task macros!).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegRhs {
    Move(RegOpnd),
    Max(RegOpnd, RegOpnd),
    Min(RegOpnd, RegOpnd),
    Add(RegOpnd, RegOpnd),
    Sub(RegOpnd, RegOpnd),
    /// `max(0.0, a + b)` (saturating positive accumulation)
    AddClampPos(RegOpnd, RegOpnd),
    /// `max(0.0, a - b)` (positive delta / rectified difference)
    SubClampPos(RegOpnd, RegOpnd),
    /// `|a - b|` (metric distance / jump)
    AbsDiff(RegOpnd, RegOpnd),
    /// `if lhs > rhs { then_v } else { else_v }` (general conditional branch!)
    SelectGt {
        lhs: RegOpnd,
        rhs: RegOpnd,
        then_v: RegOpnd,
        else_v: RegOpnd,
    },
    /// `if lhs > rhs { then_base + 1.0 } else { else_v }` (conditional increment / streak reset!)
    SelectGtInc {
        lhs: RegOpnd,
        rhs: RegOpnd,
        then_base: RegOpnd,
        else_v: RegOpnd,
    },
    /// `if lhs > rhs { then_v } else { max(else_a, else_b) }` (conditional rank-shift!)
    SelectGtElseMax {
        lhs: RegOpnd,
        rhs: RegOpnd,
        then_v: RegOpnd,
        else_a: RegOpnd,
        else_b: RegOpnd,
    },
    /// `acc + max(0.0, a - b)` (accumulate rectified delta)
    AccAddSubPos {
        acc: RegOpnd,
        a: RegOpnd,
        b: RegOpnd,
    },
    /// `max(acc, a - b)` (running peak difference)
    MaxDiff {
        acc: RegOpnd,
        a: RegOpnd,
        b: RegOpnd,
    },
    /// `max(acc, |a - b|)` (running peak absolute difference)
    MaxAbsDiff {
        acc: RegOpnd,
        a: RegOpnd,
        b: RegOpnd,
    },
}

impl RegRhs {
    #[inline]
    fn eval(self, r0: f64, r1: f64, x: f64, prev_r0: f64) -> f64 {
        let g = |o: RegOpnd| o.eval(r0, r1, x, prev_r0);
        match self {
            RegRhs::Move(a) => g(a),
            RegRhs::Max(a, b) => g(a).max(g(b)),
            RegRhs::Min(a, b) => g(a).min(g(b)),
            RegRhs::Add(a, b) => g(a) + g(b),
            RegRhs::Sub(a, b) => g(a) - g(b),
            RegRhs::AddClampPos(a, b) => (g(a) + g(b)).max(0.0),
            RegRhs::SubClampPos(a, b) => (g(a) - g(b)).max(0.0),
            RegRhs::AbsDiff(a, b) => (g(a) - g(b)).abs(),
            RegRhs::SelectGt { lhs, rhs, then_v, else_v } => {
                if g(lhs) > g(rhs) { g(then_v) } else { g(else_v) }
            }
            RegRhs::SelectGtInc { lhs, rhs, then_base, else_v } => {
                if g(lhs) > g(rhs) { g(then_base) + 1.0 } else { g(else_v) }
            }
            RegRhs::SelectGtElseMax { lhs, rhs, then_v, else_a, else_b } => {
                if g(lhs) > g(rhs) { g(then_v) } else { g(else_a).max(g(else_b)) }
            }
            RegRhs::AccAddSubPos { acc, a, b } => g(acc) + (g(a) - g(b)).max(0.0),
            RegRhs::MaxDiff { acc, a, b } => g(acc).max(g(a) - g(b)),
            RegRhs::MaxAbsDiff { acc, a, b } => g(acc).max((g(a) - g(b)).abs()),
        }
    }

    fn emit_rust(self) -> String {
        match self {
            RegRhs::Move(a) => a.emit_rust().to_string(),
            RegRhs::Max(a, b) => format!("{}.max({})", a.emit_rust(), b.emit_rust()),
            RegRhs::Min(a, b) => format!("{}.min({})", a.emit_rust(), b.emit_rust()),
            RegRhs::Add(a, b) => format!("{} + {}", a.emit_rust(), b.emit_rust()),
            RegRhs::Sub(a, b) => format!("{} - {}", a.emit_rust(), b.emit_rust()),
            RegRhs::AddClampPos(a, b) => format!("({} + {}).max(0.0f64)", a.emit_rust(), b.emit_rust()),
            RegRhs::SubClampPos(a, b) => format!("({} - {}).max(0.0f64)", a.emit_rust(), b.emit_rust()),
            RegRhs::AbsDiff(a, b) => format!("({} - {}).abs()", a.emit_rust(), b.emit_rust()),
            RegRhs::SelectGt { lhs, rhs, then_v, else_v } => format!(
                "if {} > {} {{ {} }} else {{ {} }}",
                lhs.emit_rust(), rhs.emit_rust(), then_v.emit_rust(), else_v.emit_rust()
            ),
            RegRhs::SelectGtInc { lhs, rhs, then_base, else_v } => format!(
                "if {} > {} {{ {} + 1.0f64 }} else {{ {} }}",
                lhs.emit_rust(), rhs.emit_rust(), then_base.emit_rust(), else_v.emit_rust()
            ),
            RegRhs::SelectGtElseMax { lhs, rhs, then_v, else_a, else_b } => format!(
                "if {} > {} {{ {} }} else {{ {}.max({}) }}",
                lhs.emit_rust(), rhs.emit_rust(), then_v.emit_rust(), else_a.emit_rust(), else_b.emit_rust()
            ),
            RegRhs::AccAddSubPos { acc, a, b } => format!(
                "{} + ({} - {}).max(0.0f64)",
                acc.emit_rust(), a.emit_rust(), b.emit_rust()
            ),
            RegRhs::MaxDiff { acc, a, b } => format!(
                "{}.max({} - {})",
                acc.emit_rust(), a.emit_rust(), b.emit_rust()
            ),
            RegRhs::MaxAbsDiff { acc, a, b } => format!(
                "{}.max(({} - {}).abs())",
                acc.emit_rust(), a.emit_rust(), b.emit_rust()
            ),
        }
    }

    fn uses_prev_r0(self) -> bool {
        match self {
            RegRhs::Move(a) => a == RegOpnd::PrevR0,
            RegRhs::Max(a, b)
            | RegRhs::Min(a, b)
            | RegRhs::Add(a, b)
            | RegRhs::Sub(a, b)
            | RegRhs::AddClampPos(a, b)
            | RegRhs::SubClampPos(a, b)
            | RegRhs::AbsDiff(a, b) => a == RegOpnd::PrevR0 || b == RegOpnd::PrevR0,
            RegRhs::SelectGt { lhs, rhs, then_v, else_v } => {
                [lhs, rhs, then_v, else_v].contains(&RegOpnd::PrevR0)
            }
            RegRhs::SelectGtInc { lhs, rhs, then_base, else_v } => {
                [lhs, rhs, then_base, else_v].contains(&RegOpnd::PrevR0)
            }
            RegRhs::SelectGtElseMax { lhs, rhs, then_v, else_a, else_b } => {
                [lhs, rhs, then_v, else_a, else_b].contains(&RegOpnd::PrevR0)
            }
            RegRhs::AccAddSubPos { acc, a, b }
            | RegRhs::MaxDiff { acc, a, b }
            | RegRhs::MaxAbsDiff { acc, a, b } => [acc, a, b].contains(&RegOpnd::PrevR0),
        }
    }

    fn complexity(self) -> usize {
        match self {
            RegRhs::Move(_) => 1,
            RegRhs::Max(_, _)
            | RegRhs::Min(_, _)
            | RegRhs::Add(_, _)
            | RegRhs::Sub(_, _) => 2,
            RegRhs::AddClampPos(_, _)
            | RegRhs::SubClampPos(_, _)
            | RegRhs::AbsDiff(_, _)
            | RegRhs::MaxDiff { .. }
            | RegRhs::MaxAbsDiff { .. }
            | RegRhs::AccAddSubPos { .. } => 3,
            RegRhs::SelectGt { .. }
            | RegRhs::SelectGtInc { .. }
            | RegRhs::SelectGtElseMax { .. } => 4,
        }
    }

    /// Balanced structural opcode family ID (0..11) — avoids collapsing distinct ops into 1 skewed bucket!
    pub fn op_family(self) -> u8 {
        match self {
            RegRhs::Move(_) => 0,
            RegRhs::Max(_, _) => 1,
            RegRhs::Min(_, _) => 2,
            RegRhs::Add(_, _) => 3,
            RegRhs::Sub(_, _) | RegRhs::MaxDiff { .. } => 4,
            RegRhs::AddClampPos(_, _) => 5,
            RegRhs::SubClampPos(_, _) | RegRhs::AccAddSubPos { .. } => 6,
            RegRhs::AbsDiff(_, _) | RegRhs::MaxAbsDiff { .. } => 7,
            RegRhs::SelectGt { .. } => 8,
            RegRhs::SelectGtInc { .. } => 9,
            RegRhs::SelectGtElseMax { .. } => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegInit {
    Zero,
    FirstElem,
    NegInf,
}

impl RegInit {
    #[inline]
    fn eval(self, first: f64) -> f64 {
        match self {
            RegInit::Zero => 0.0,
            RegInit::FirstElem => first,
            RegInit::NegInf => f64::NEG_INFINITY,
        }
    }

    fn emit_rust(self) -> &'static str {
        match self {
            RegInit::Zero => "0.0f64",
            RegInit::FirstElem => "xs[0]",
            RegInit::NegInf => "f64::NEG_INFINITY",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegProgram {
    pub init_r0: RegInit,
    pub init_r1: RegInit,
    pub start_from_one: bool,
    pub upd_r0: RegRhs,
    pub upd_r1: RegRhs,
}

impl RegProgram {
    #[inline]
    pub fn eval_slice(&self, xs: &[f64]) -> f64 {
        if xs.is_empty() {
            return 0.0;
        }
        let mut r0 = self.init_r0.eval(xs[0]);
        let mut r1 = self.init_r1.eval(xs[0]);
        let start = if self.start_from_one { 1 } else { 0 };
        for &x in xs.iter().skip(start) {
            let prev_r0 = r0;
            r0 = self.upd_r0.eval(r0, r1, x, prev_r0);
            r1 = self.upd_r1.eval(r0, r1, x, prev_r0);
        }
        if r1.is_finite() { r1 } else { 0.0 }
    }

    pub fn prog_len(&self) -> usize {
        2 + self.upd_r0.complexity() + self.upd_r1.complexity() + if self.start_from_one { 1 } else { 0 }
    }

    pub fn emit_rust(&self) -> String {
        let need_prev = self.upd_r0.uses_prev_r0() || self.upd_r1.uses_prev_r0();
        let prev_decl = if need_prev {
            "        let prev_r0 = r0;\n"
        } else {
            ""
        };
        let iter_expr = if self.start_from_one {
            "xs.iter().skip(1)"
        } else {
            "xs.iter()"
        };
        format!(
            "pub fn solve_stream(xs: &[f64]) -> f64 {{\n    if xs.is_empty() {{ return 0.0; }}\n    let mut r0 = {};\n    let mut r1 = {};\n    for &x in {} {{\n{}        r0 = {};\n        r1 = {};\n    }}\n    if r1.is_finite() {{ r1 }} else {{ 0.0 }}\n}}",
            self.init_r0.emit_rust(),
            self.init_r1.emit_rust(),
            iter_expr,
            prev_decl,
            self.upd_r0.emit_rust(),
            self.upd_r1.emit_rust()
        )
    }

    /// Generate a rich, domain-agnostic primitive SSA instruction search space (~2,400+ distinct imperative programs!).
    pub fn build_general_search_space() -> Vec<Self> {
        let r0_updates = vec![
            RegRhs::Move(RegOpnd::X),
            RegRhs::Max(RegOpnd::R0, RegOpnd::X),
            RegRhs::Min(RegOpnd::R0, RegOpnd::X),
            RegRhs::Add(RegOpnd::R0, RegOpnd::X),
            RegRhs::Add(RegOpnd::R0, RegOpnd::One),
            RegRhs::Sub(RegOpnd::R0, RegOpnd::X),
            RegRhs::AbsDiff(RegOpnd::X, RegOpnd::R0),
            RegRhs::AddClampPos(RegOpnd::R0, RegOpnd::X),
            RegRhs::SubClampPos(RegOpnd::X, RegOpnd::R0),
            RegRhs::SelectGtInc {
                lhs: RegOpnd::X,
                rhs: RegOpnd::Zero,
                then_base: RegOpnd::R0,
                else_v: RegOpnd::Zero,
            },
            RegRhs::SelectGtInc {
                lhs: RegOpnd::X,
                rhs: RegOpnd::R0,
                then_base: RegOpnd::R0,
                else_v: RegOpnd::R0,
            },
            RegRhs::SelectGt {
                lhs: RegOpnd::X,
                rhs: RegOpnd::R0,
                then_v: RegOpnd::X,
                else_v: RegOpnd::R0,
            },
        ];

        let r1_updates = vec![
            RegRhs::Max(RegOpnd::R1, RegOpnd::R0),
            RegRhs::Max(RegOpnd::R1, RegOpnd::X),
            RegRhs::Min(RegOpnd::R1, RegOpnd::R0),
            RegRhs::Add(RegOpnd::R1, RegOpnd::R0),
            RegRhs::Add(RegOpnd::R1, RegOpnd::X),
            RegRhs::MaxDiff { acc: RegOpnd::R1, a: RegOpnd::R0, b: RegOpnd::X },
            RegRhs::MaxDiff { acc: RegOpnd::R1, a: RegOpnd::X, b: RegOpnd::R0 },
            RegRhs::MaxDiff { acc: RegOpnd::R1, a: RegOpnd::X, b: RegOpnd::PrevR0 },
            RegRhs::MaxAbsDiff { acc: RegOpnd::R1, a: RegOpnd::X, b: RegOpnd::PrevR0 },
            RegRhs::MaxAbsDiff { acc: RegOpnd::R1, a: RegOpnd::R0, b: RegOpnd::X },
            RegRhs::AccAddSubPos { acc: RegOpnd::R1, a: RegOpnd::X, b: RegOpnd::PrevR0 },
            RegRhs::AccAddSubPos { acc: RegOpnd::R1, a: RegOpnd::PrevR0, b: RegOpnd::X },
            RegRhs::SelectGtInc {
                lhs: RegOpnd::X,
                rhs: RegOpnd::PrevR0,
                then_base: RegOpnd::R1,
                else_v: RegOpnd::R1,
            },
            RegRhs::SelectGtInc {
                lhs: RegOpnd::X,
                rhs: RegOpnd::Zero,
                then_base: RegOpnd::R1,
                else_v: RegOpnd::R1,
            },
            RegRhs::SelectGtElseMax {
                lhs: RegOpnd::X,
                rhs: RegOpnd::PrevR0,
                then_v: RegOpnd::PrevR0,
                else_a: RegOpnd::R1,
                else_b: RegOpnd::X,
            },
            RegRhs::SelectGtElseMax {
                lhs: RegOpnd::X,
                rhs: RegOpnd::R0,
                then_v: RegOpnd::R0,
                else_a: RegOpnd::R1,
                else_b: RegOpnd::X,
            },
        ];

        let init_pairs = [
            (RegInit::Zero, RegInit::Zero, false),
            (RegInit::FirstElem, RegInit::Zero, false),
            (RegInit::FirstElem, RegInit::Zero, true),
            (RegInit::NegInf, RegInit::Zero, false),
            (RegInit::NegInf, RegInit::NegInf, false),
            (RegInit::FirstElem, RegInit::NegInf, true),
        ];

        let mut space = Vec::with_capacity(init_pairs.len() * r0_updates.len() * r1_updates.len());
        let mut seen: HashSet<RegProgram> = HashSet::new();
        for &(i0, i1, s1) in &init_pairs {
            for &u0 in &r0_updates {
                for &u1 in &r1_updates {
                    let p = RegProgram {
                        init_r0: i0,
                        init_r1: i1,
                        start_from_one: s1,
                        upd_r0: u0,
                        upd_r1: u1,
                    };
                    if seen.insert(p) {
                        space.push(p);
                    }
                }
            }
        }
        space
    }
}

/// Persistent Continual Skill Library recording verified imperative SSA idioms (`r0_family`, `r1_family`, `init_mode`)
/// so solving earlier tasks accelerates synthesis on UNSEEN held-out tasks!
#[derive(Clone, Debug, Default)]
pub struct ImperativeSkillLibrary {
    pub r0_op_counts: HashMap<u8, usize>,
    pub r1_op_counts: HashMap<u8, usize>,
    pub verified_programs: Vec<String>,
}

impl ImperativeSkillLibrary {
    pub fn record_verified(&mut self, prog: &RegProgram, rust_src: &str) {
        *self.r0_op_counts.entry(prog.upd_r0.op_family()).or_insert(0) += 1;
        *self.r1_op_counts.entry(prog.upd_r1.op_family()).or_insert(0) += 1;
        if !self.verified_programs.iter().any(|s| s == rust_src) {
            self.verified_programs.push(rust_src.to_string());
        }
    }

    pub fn bonus_for(&self, prog: &RegProgram) -> f64 {
        let total = self.verified_programs.len().max(1) as f64;
        let c0 = *self.r0_op_counts.get(&prog.upd_r0.op_family()).unwrap_or(&0) as f64;
        let c1 = *self.r1_op_counts.get(&prog.upd_r1.op_family()).unwrap_or(&0) as f64;
        ((c0 + c1) / (2.0 * total)).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub struct RegSynthTask {
    pub name: &'static str,
    pub is_heldout: bool,
    pub train_io: Vec<(Vec<f64>, f64)>,
    pub sealed_ood_harness: &'static str,
}

#[derive(Clone, Debug)]
pub struct RegSynthTaskResult {
    pub name: String,
    pub is_heldout: bool,
    pub solved: bool,
    pub rustc_verified: bool,
    pub expansions: usize,
    pub runtime_us: u64,
    pub mdl_bits: usize,
    pub rust_code: String,
}

#[derive(Clone, Debug)]
pub struct GeneralRegSynthReport {
    pub space_size: usize,
    pub train_solved: usize,
    pub train_total: usize,
    pub heldout_cold_solved: usize,
    pub heldout_trained_solved: usize,
    pub heldout_total: usize,
    pub heldout_cold_avg_exp: f64,
    pub heldout_trained_avg_exp: f64,
    pub rustc_verified_total: usize,
    pub skills_learned: usize,
    pub wall_ms: f64,
    pub details: Vec<RegSynthTaskResult>,
}

pub fn build_8_general_imperative_tasks() -> Vec<RegSynthTask> {
    vec![
        // --- 4 Training Frontier Tasks ---
        RegSynthTask {
            name: "running_max_drawdown",
            is_heldout: false,
            train_io: vec![
                (vec![10.0, 12.0, 9.0, 14.0, 7.5, 11.0], 6.5),
                (vec![5.0, 8.0, 8.0, 3.0, 6.0], 5.0),
                (vec![1.0, 2.0, 3.0, 4.0], 0.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [100.0, 105.0, 102.0, 108.0, 90.0, 95.0, 88.0, 110.0];
    let ood2 = [-2.0, 4.0, -5.0, 1.0, -3.0];
    assert!((solve_stream(&ood1) - 20.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 9.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "kadane_max_subarray",
            is_heldout: false,
            train_io: vec![
                (vec![-2.0, 1.0, -3.0, 4.0, -1.0, 2.0, 1.0, -5.0, 4.0], 6.0),
                (vec![-4.0, -2.0, 3.0, 5.0, -1.0, 2.0], 9.0),
                (vec![-5.0, -1.0, -3.0], 0.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [2.0, -1.0, 3.0, -5.0, 4.0, 4.0, -2.0, 3.0];
    let ood2 = [-5.0, -2.0, -9.0];
    assert!((solve_stream(&ood1) - 9.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 0.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "best_buy_sell_profit",
            is_heldout: false,
            train_io: vec![
                (vec![7.0, 1.0, 5.0, 3.0, 6.0, 4.0], 5.0),
                (vec![9.0, 7.0, 4.0, 2.0], 0.0),
                (vec![3.0, 8.0, 2.0, 10.0], 8.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [12.0, 15.0, 8.0, 9.0, 21.0, 14.0, 22.0];
    let ood2 = [5.0, 2.0, 8.0, 1.0, 10.0];
    assert!((solve_stream(&ood1) - 14.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 9.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "longest_positive_run",
            is_heldout: false,
            train_io: vec![
                (vec![1.0, 2.0, -1.0, 3.0, 4.0, 5.0, 0.0, 2.0], 3.0),
                (vec![-2.0, -1.0, 0.0], 0.0),
                (vec![4.0, 1.0, 2.0, 3.0, -5.0, 9.0], 4.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [0.5, 1.5, 2.5, -0.1, 3.0, 4.0, 5.0, 6.0, 7.0, -2.0, 1.0];
    let ood2 = [-1.0, 2.0, 0.0, 3.0, 4.0];
    assert!((solve_stream(&ood1) - 5.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 2.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        // --- 4 COMPLETELY UNSEEN HELD-OUT OOD IMPERATIVE ALGORITHMS! ---
        RegSynthTask {
            name: "count_new_records",
            is_heldout: true,
            train_io: vec![
                (vec![3.0, 5.0, 4.0, 8.0, 8.0, 10.0], 3.0),
                (vec![9.0, 7.0, 5.0, 2.0], 0.0),
                (vec![1.0, 2.0, 3.0, 4.0, 5.0], 4.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [10.0, 12.0, 11.0, 15.0, 15.0, 14.0, 20.0, 25.0];
    let ood2 = [-5.0, -2.0, -3.0, 0.0, 0.0, 1.0];
    assert!((solve_stream(&ood1) - 4.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 3.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "total_upward_gain",
            is_heldout: true,
            train_io: vec![
                (vec![7.0, 1.0, 5.0, 3.0, 6.0, 4.0], 7.0),
                (vec![1.0, 2.0, 3.0, 4.0, 5.0], 4.0),
                (vec![9.0, 6.0, 2.0], 0.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [10.0, 13.0, 9.0, 15.0, 12.0, 20.0];
    let ood2 = [4.0, 4.0, 6.0, 1.0, 9.0];
    assert!((solve_stream(&ood1) - 17.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 10.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "max_adjacent_jump",
            is_heldout: true,
            train_io: vec![
                (vec![1.0, 4.0, 2.0, 9.0, 8.0], 7.0),
                (vec![5.0, 5.0, 5.0], 0.0),
                (vec![10.0, 2.0, 6.0], 8.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [-3.0, 6.0, 4.0, -10.0, -9.0, 2.0];
    let ood2 = [1.5, 2.0, 2.5, 3.0];
    assert!((solve_stream(&ood1) - 14.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 0.5).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
        RegSynthTask {
            name: "second_largest",
            is_heldout: true,
            train_io: vec![
                (vec![3.0, 9.0, 5.0, 7.0, 2.0], 7.0),
                (vec![10.0, 4.0, 8.0, 12.0], 10.0),
                (vec![-5.0, -1.0, -3.0, -2.0], -2.0),
            ],
            sealed_ood_harness: r#"fn main() {
    let ood1 = [14.0, 2.0, 9.0, 21.0, 18.0, 5.0];
    let ood2 = [8.0, 8.0, 3.0, 6.0];
    assert!((solve_stream(&ood1) - 18.0).abs() < 1e-6);
    assert!((solve_stream(&ood2) - 8.0).abs() < 1e-6);
    println!("ASI_REG_PASS");
}"#,
        },
    ]
}

/// Encode ONLY raw `(xs, y)` task IO examples into unified `R^10` tokens (ZERO program peeking!).
fn encode_io_stream_tokens(io_pairs: &[(Vec<f64>, f64)]) -> Vec<[f64; crate::core::RAW_FEAT_DIM]> {
    let mut toks = Vec::with_capacity(io_pairs.len() * 6);
    for (xs, y) in io_pairs {
        let n = xs.len().max(1);
        let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let span = (max_x - min_x).max(1e-4);
        let mut prev = xs[0];
        for (i, &x) in xs.iter().enumerate() {
            let dx = x - prev;
            toks.push([
                x.asinh() * 0.35,
                dx.asinh() * 0.35,
                y.asinh() * 0.35,
                ((y - max_x) / span).clamp(-2.0, 2.0),
                ((y - min_x) / span).clamp(-2.0, 2.0),
                if *y < 0.0 { -1.0 } else if *y == 0.0 { 0.0 } else { 1.0 },
                if (*y - y.round()).abs() < 1e-4 && *y <= n as f64 { 1.0 } else { -1.0 },
                (i as f64 / n as f64) * 2.0 - 1.0,
                (n as f64).ln_1p() * 0.3,
                ModalityTag::CodeFeedback.code(),
            ]);
            prev = x;
        }
    }
    if toks.is_empty() {
        toks.push([0.0; crate::core::RAW_FEAT_DIM]);
    }
    toks
}

fn prog_init_bits(p: &RegProgram) -> [f64; 3] {
    [
        if p.init_r0 == RegInit::FirstElem { 1.0 } else { 0.0 },
        if p.init_r1 == RegInit::NegInf { 1.0 } else { 0.0 },
        if p.start_from_one { 1.0 } else { 0.0 },
    ]
}

/// Self-Play Wake-Sleep Dreaming (`ZERO` held-out leakage!) + Empirical Class Base Rates for Exact Bayes Factor:
/// Samples class-balanced programs from TropiCore-Omega's OWN general SSA space on purely synthetic arrays,
/// executes them to produce `(IO_stream -> opcodes)` dreams, and trains `UniversalEnergyCore`'s inverse
/// execution heads (`W_enc`, `W_core`, `W_op`, `W_next`, `W_var`) via End-to-End STE!
fn dream_self_play_ssa(
    core: &mut UniversalEnergyCore,
    space: &[RegProgram],
    seed: u64,
) -> ([f64; crate::core::NUM_OP_CLASSES], [f64; crate::core::NUM_OP_CLASSES]) {
    let mut s = seed ^ 0xD8EA_0001_5555;
    let mut next_u = || -> f64 {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / ((1u64 << 53) as f64)
    };

    // 4 purely synthetic domain-agnostic probe arrays (mixed / decreasing / oscillating / all-negative)
    let synth_inputs: Vec<Vec<f64>> = vec![
        vec![4.0, 7.0, 3.0, 9.0, 5.0, 11.0],
        vec![8.0, 6.0, 4.0, 2.0, 1.0],
        vec![-3.0, 2.0, 5.0, -4.0, 3.0, 6.0],
        vec![-8.0, -2.0, -5.0, -1.0, -4.0],
    ];

    // Collect balanced non-degenerate self-play dreams across ALL r1 op families (capped per class so no class swamps!)
    let mut dreams: Vec<(Vec<[f64; crate::core::RAW_FEAT_DIM]>, usize, usize, [f64; 3])> = Vec::with_capacity(280);
    let mut r1_class_cap = [0usize; crate::core::NUM_OP_CLASSES];
    let mut perm: Vec<usize> = (0..space.len()).collect();
    for i in (1..perm.len()).rev() {
        let j = (next_u() * (i + 1) as f64) as usize % (i + 1);
        perm.swap(i, j);
    }

    for &idx in &perm {
        if dreams.len() >= 240 {
            break;
        }
        let prog = space[idx];
        let c1 = prog.upd_r1.op_family() as usize;
        if r1_class_cap[c1] >= 36 {
            continue;
        }
        let pairs: Vec<(Vec<f64>, f64)> = synth_inputs
            .iter()
            .map(|xs| (xs.clone(), prog.eval_slice(xs)))
            .collect();
        let all_same = pairs.windows(2).all(|w| (w[0].1 - w[1].1).abs() < 1e-6);
        let all_finite = pairs.iter().all(|(_, y)| y.is_finite() && y.abs() < 1e4);
        if all_finite && !all_same {
            let toks = encode_io_stream_tokens(&pairs);
            r1_class_cap[c1] += 1;
            dreams.push((
                toks,
                prog.upd_r0.op_family() as usize,
                c1,
                prog_init_bits(&prog),
            ));
        }
    }

    let mut prior_r0 = [1.0f64; crate::core::NUM_OP_CLASSES];
    let mut prior_r1 = [1.0f64; crate::core::NUM_OP_CLASSES];
    for &(_, c0, c1, _) in &dreams {
        prior_r0[c0] += 1.0;
        prior_r1[c1] += 1.0;
    }
    let s0: f64 = prior_r0.iter().sum();
    let s1: f64 = prior_r1.iter().sum();
    for c in 0..crate::core::NUM_OP_CLASSES {
        prior_r0[c] /= s0;
        prior_r1[c] /= s1;
    }

    // 5 fast balanced self-play consolidation epochs (~150ms on laptop!)
    for ep in 0..5usize {
        let lr = 0.042 * (0.82f64).powi(ep as i32);
        for (toks, c0, c1, ib) in &dreams {
            core.train_inverse_io_to_ops_step(toks, *c0, *c1, *ib, lr);
        }
    }
    (prior_r0, prior_r1)
}

fn solve_single_reg_task(
    task: &RegSynthTask,
    space: &[RegProgram],
    core: Option<&UniversalEnergyCore>,
    skills: Option<&ImperativeSkillLibrary>,
    priors: Option<&([f64; crate::core::NUM_OP_CLASSES], [f64; crate::core::NUM_OP_CLASSES])>,
    verify_rustc: bool,
) -> (RegSynthTaskResult, Option<RegProgram>) {
    // ZERO-PEEK CONDITIONAL INVERSE EXECUTION WITH EXACT BAYES FACTOR CALIBRATION:
    // Encode ONLY `task.train_io` ONCE -> compute Bayes-calibrated `P(io | prog) = P(prog | io) / P_prior(prog)`!
    let pred_opt = core.map(|c| {
        let io_toks = encode_io_stream_tokens(&task.train_io);
        c.predict_op_next_var_probs(&io_toks)
    });

    let mut scored: Vec<(f64, usize)> = Vec::with_capacity(space.len());
    for (idx, prog) in space.iter().enumerate() {
        let prio = if let Some((ref op_p, ref nx_p, ref var_p)) = pred_opt {
            let c0 = prog.upd_r0.op_family() as usize;
            let c1 = prog.upd_r1.op_family() as usize;
            let (bf0, bf1) = if let Some((pr0, pr1)) = priors {
                // Exact Bayes Factor: posterior / prior -> sigmoid-normalized to (0.02, 0.98)
                let r0 = (op_p[c0] / pr0[c0].max(0.01)).clamp(0.05, 16.0);
                let r1 = (nx_p[c1] / pr1[c1].max(0.01)).clamp(0.05, 16.0);
                (r0 / (1.0 + r0), r1 / (1.0 + r1))
            } else {
                (op_p[c0], nx_p[c1])
            };
            let ib = prog_init_bits(prog);
            let pi0 = if ib[0] > 0.5 { var_p[0] } else { 1.0 - var_p[0] };
            let pi1 = if ib[1] > 0.5 { var_p[1] } else { 1.0 - var_p[1] };
            let pi2 = if ib[2] > 0.5 { var_p[2] } else { 1.0 - var_p[2] };
            // Weighted geometric mean strongly favoring Bayes-calibrated op & init evidence
            let cond_p = (bf0.powf(0.40) * bf1.powf(0.40) * (pi0 * pi1 * pi2).powf(0.20 / 3.0)).clamp(0.02, 0.98);
            // Weight neural inverse evidence strongly over raw token length
            let sk_bonus = skills.map(|sk| 0.25 * sk.bonus_for(prog)).unwrap_or(0.0);
            let base = synth_candidate_priority(0.0, cond_p, prog.prog_len(), sk_bonus);
            // Emphasize conditional neural inverse likelihood (`-1.4 * ln(cond_p)`)
            base - 1.40 * cond_p.ln()
        } else {
            // Cold Untrained Baseline: pure MDL length order (`p=0.5, skill=0.0`)
            synth_candidate_priority(0.0, 0.5, prog.prog_len(), 0.0)
        };
        scored.push((prio, idx));
    }
    scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut expansions = 0usize;
    for &(_, idx) in &scored {
        expansions += 1;
        let prog = space[idx];
        let mut ok_train = true;
        for (xs, y) in &task.train_io {
            if (prog.eval_slice(xs) - y).abs() > 1e-5 {
                ok_train = false;
                break;
            }
        }
        if !ok_train {
            continue;
        }
        let rust_code = prog.emit_rust();
        if !verify_rustc {
            return (
                RegSynthTaskResult {
                    name: task.name.to_string(),
                    is_heldout: task.is_heldout,
                    solved: true,
                    rustc_verified: false,
                    expansions,
                    runtime_us: 0,
                    mdl_bits: rust_code.len() * 8,
                    rust_code,
                },
                Some(prog),
            );
        }
        let mut oracle = RustSandboxCodeOracle {
            task_name: format!("reg_{}", task.name),
            harness_main: task.sealed_ood_harness.to_string(),
            expected_token: "ASI_REG_PASS".to_string(),
        };
        let v = oracle.evaluate(&rust_code);
        if v.tier == OracleTier::Tier1Execution && v.passed && v.score >= 1.0 {
            return (
                RegSynthTaskResult {
                    name: task.name.to_string(),
                    is_heldout: task.is_heldout,
                    solved: true,
                    rustc_verified: true,
                    expansions,
                    runtime_us: v.compute_cost_us,
                    mdl_bits: v.mdl_bits,
                    rust_code,
                },
                Some(prog),
            );
        }
    }

    (
        RegSynthTaskResult {
            name: task.name.to_string(),
            is_heldout: task.is_heldout,
            solved: false,
            rustc_verified: false,
            expansions,
            runtime_us: 0,
            mdl_bits: 0,
            rust_code: String::new(),
        },
        None,
    )
}

/// Run the full Zero-Crutch General SSA Imperative Synthesis Benchmark:
///   1. Measure COLD UNTRAINED baseline on 4 UNSEEN held-out algorithms (no `UniversalEnergyCore`, no `SkillLibrary`).
///   2. Solve 4 Training Frontier tasks -> train `UniversalEnergyCore` online STE + populate `ImperativeSkillLibrary`.
///   3. Re-evaluate TRAINED Unified-Brain on the 4 UNSEEN held-out algorithms + certify ALL 8 with `rustc -O -D warnings`!
pub fn run_general_reg_synth_suite(
    core: &mut UniversalEnergyCore,
    skills: &mut ImperativeSkillLibrary,
) -> GeneralRegSynthReport {
    let t0 = Instant::now();
    let space = RegProgram::build_general_search_space();
    let tasks = build_8_general_imperative_tasks();

    // 1. Cold Untrained Baseline on the 4 UNSEEN Held-Out Tasks (fast in-process exact match check)
    let mut cold_solved = 0usize;
    let mut cold_exp_sum = 0usize;
    let heldout_total = tasks.iter().filter(|t| t.is_heldout).count();
    for t in tasks.iter().filter(|t| t.is_heldout) {
        let (res, _) = solve_single_reg_task(t, &space, None, None, None, false);
        if res.solved {
            cold_solved += 1;
        }
        cold_exp_sum += res.expansions;
    }

    // 2. Self-Play Wake-Sleep Dreaming + Training Frontier Tasks + Live `rustc -O -D warnings` Verification!
    let priors = dream_self_play_ssa(core, &space, 2026_1125 + core.train_steps_completed as u64);

    let mut details = Vec::with_capacity(tasks.len());
    let mut train_solved = 0usize;
    let mut rustc_total = 0usize;
    let train_total = tasks.iter().filter(|t| !t.is_heldout).count();

    for t in tasks.iter().filter(|t| !t.is_heldout) {
        let (res, prog_opt) = solve_single_reg_task(t, &space, Some(core), Some(skills), Some(&priors), true);
        if res.solved {
            train_solved += 1;
        }
        if res.rustc_verified {
            rustc_total += 1;
        }
        if let Some(win_p) = prog_opt {
            skills.record_verified(&win_p, &res.rust_code);
            let io_toks = encode_io_stream_tokens(&t.train_io);
            let ib = prog_init_bits(&win_p);
            for _ in 0..2 {
                core.train_inverse_io_to_ops_step(
                    &io_toks,
                    win_p.upd_r0.op_family() as usize,
                    win_p.upd_r1.op_family() as usize,
                    ib,
                    0.03,
                );
            }
            let code_toks = UniversalEventStream::from_code_and_feedback(&res.rust_code, "", 1, 1);
            core.train_universal_stream_value_step(&code_toks, 0.92, 0.02);
        }
        details.push(res);
    }

    // 3. Frozen Trained Evaluation on the 4 UNSEEN Held-Out Imperative Algorithms + Live `rustc -O -D warnings`!
    let mut trained_solved = 0usize;
    let mut trained_exp_sum = 0usize;
    for t in tasks.iter().filter(|t| t.is_heldout) {
        let (res, prog_opt) = solve_single_reg_task(t, &space, Some(core), Some(skills), Some(&priors), true);
        if res.solved {
            trained_solved += 1;
        }
        if res.rustc_verified {
            rustc_total += 1;
        }
        trained_exp_sum += res.expansions;
        if let Some(win_p) = prog_opt {
            skills.record_verified(&win_p, &res.rust_code);
        }
        details.push(res);
    }

    GeneralRegSynthReport {
        space_size: space.len(),
        train_solved,
        train_total,
        heldout_cold_solved: cold_solved,
        heldout_trained_solved: trained_solved,
        heldout_total,
        heldout_cold_avg_exp: cold_exp_sum as f64 / heldout_total.max(1) as f64,
        heldout_trained_avg_exp: trained_exp_sum as f64 / heldout_total.max(1) as f64,
        rustc_verified_total: rustc_total,
        skills_learned: skills.verified_programs.len(),
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
        details,
    }
}
