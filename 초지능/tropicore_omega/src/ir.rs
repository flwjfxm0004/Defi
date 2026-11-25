//! Pillar 1: Universal Turing-Complete Hybrid Symbolic x Functional x Cellular-Stencil x Dual-AD IR (`OmegaExpr` v5.0).
//!
//! 100% DELETED from v4.0:
//!   - ZERO hardcoded `GridGravityStep`, `GridRayRightStep`, `GridMirrorH`, `GridHollow` domain macros!
//!     Replaced by pure raw cellular-automaton stencil rules (`CellExpr` + `GridStencilRule`) and raw
//!     index coordinate maps (`CoordExpr` + `GridCoordMap`) + `BoundedFix`!
//!
//! NEW in v5.0 (Turing-Complete Functional & Dynamic-List Primitives):
//!   - `Lam(body)` & `App(func, arg)`: First-class anonymous functions & general application
//!   - `Rec { base, step, max_fuel }`: General bounded-fuel recursion (Turing-complete as `max_fuel -> inf`)
//!   - `ListInput`, `ListMap`, `ListFilterPos`, `ListReduce`, `ListLen`: Arbitrary-length dynamic array combinators
//!   - `Param(k)` with exact Forward-Mode Dual-Number AD (`eval_dual`)
//!   - Native `rustc -O` emitter for scalars, dynamic lists, recursive functions, AND synthesized 2D grids!
//!   - Live `Lean 4.36.0` emitter + `ExactFraction` Exact 3D Pareto HV

use std::collections::HashMap;

pub type Grid2D = Vec<Vec<i64>>;

/// Pure raw 2D index arithmetic for synthesizing coordinate transforms (MirrorH, FlipV, Transpose, etc.)
/// with ZERO hardcoded transform macros!
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordExpr {
    Row,
    Col,
    Height,
    Width,
    One,
    Sub(Box<CoordExpr>, Box<CoordExpr>),
}

impl CoordExpr {
    pub fn mdl_size(&self) -> usize {
        match self {
            CoordExpr::Row | CoordExpr::Col | CoordExpr::Height | CoordExpr::Width | CoordExpr::One => 1,
            CoordExpr::Sub(a, b) => 1 + a.mdl_size() + b.mdl_size(),
        }
    }

    pub fn eval(&self, r: usize, c: usize, h: usize, w: usize) -> isize {
        match self {
            CoordExpr::Row => r as isize,
            CoordExpr::Col => c as isize,
            CoordExpr::Height => h as isize,
            CoordExpr::Width => w as isize,
            CoordExpr::One => 1,
            CoordExpr::Sub(a, b) => a.eval(r, c, h, w) - b.eval(r, c, h, w),
        }
    }

    pub fn key(&self) -> String {
        match self {
            CoordExpr::Row => "r".to_string(),
            CoordExpr::Col => "c".to_string(),
            CoordExpr::Height => "H".to_string(),
            CoordExpr::Width => "W".to_string(),
            CoordExpr::One => "1".to_string(),
            CoordExpr::Sub(a, b) => format!("(- {} {})", a.key(), b.key()),
        }
    }

    pub fn emit_rust(&self) -> String {
        match self {
            CoordExpr::Row => "(r as isize)".to_string(),
            CoordExpr::Col => "(c as isize)".to_string(),
            CoordExpr::Height => "(h as isize)".to_string(),
            CoordExpr::Width => "(w as isize)".to_string(),
            CoordExpr::One => "1isize".to_string(),
            CoordExpr::Sub(a, b) => format!("({} - {})", a.emit_rust(), b.emit_rust()),
        }
    }
}

/// Pure raw 4-neighbor cellular stencil expression over local pixel observables (ZERO domain macros!).
/// Expresses Hollow, RayRight, RayDown, GravityStep, BorderDetect, etc. purely from local cell algebra!
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellExpr {
    SelfVal,
    North,
    South,
    East,
    West,
    AtBottom, // pure geometric boundary observable: `1` if `r + 1 == h` else `0`
    Zero,
    Min(Box<CellExpr>, Box<CellExpr>),
    Max(Box<CellExpr>, Box<CellExpr>),
    IfPos(Box<CellExpr>, Box<CellExpr>, Box<CellExpr>),
}

impl CellExpr {
    pub fn mdl_size(&self) -> usize {
        match self {
            CellExpr::SelfVal
            | CellExpr::North
            | CellExpr::South
            | CellExpr::East
            | CellExpr::West
            | CellExpr::AtBottom
            | CellExpr::Zero => 1,
            CellExpr::Min(a, b) | CellExpr::Max(a, b) => 1 + a.mdl_size() + b.mdl_size(),
            CellExpr::IfPos(c, t, e) => 1 + c.mdl_size() + t.mdl_size() + e.mdl_size(),
        }
    }

    pub fn op_count(&self) -> usize {
        match self {
            CellExpr::Min(a, b) | CellExpr::Max(a, b) => 1 + a.op_count() + b.op_count(),
            CellExpr::IfPos(c, t, e) => 1 + c.op_count() + t.op_count() + e.op_count(),
            _ => 0,
        }
    }

    #[inline]
    pub fn eval(&self, s: i64, n: i64, so: i64, e: i64, w: i64, at_bot: i64) -> i64 {
        match self {
            CellExpr::SelfVal => s,
            CellExpr::North => n,
            CellExpr::South => so,
            CellExpr::East => e,
            CellExpr::West => w,
            CellExpr::AtBottom => at_bot,
            CellExpr::Zero => 0,
            CellExpr::Min(a, b) => a.eval(s, n, so, e, w, at_bot).min(b.eval(s, n, so, e, w, at_bot)),
            CellExpr::Max(a, b) => a.eval(s, n, so, e, w, at_bot).max(b.eval(s, n, so, e, w, at_bot)),
            CellExpr::IfPos(c, t, el) => {
                if c.eval(s, n, so, e, w, at_bot) > 0 {
                    t.eval(s, n, so, e, w, at_bot)
                } else {
                    el.eval(s, n, so, e, w, at_bot)
                }
            }
        }
    }

    pub fn key(&self) -> String {
        match self {
            CellExpr::SelfVal => "S".to_string(),
            CellExpr::North => "N".to_string(),
            CellExpr::South => "So".to_string(),
            CellExpr::East => "E".to_string(),
            CellExpr::West => "W".to_string(),
            CellExpr::AtBottom => "Bot".to_string(),
            CellExpr::Zero => "0".to_string(),
            CellExpr::Min(a, b) => format!("(min {} {})", a.key(), b.key()),
            CellExpr::Max(a, b) => format!("(max {} {})", a.key(), b.key()),
            CellExpr::IfPos(c, t, e) => format!("(if>0 {} {} {})", c.key(), t.key(), e.key()),
        }
    }

    pub fn emit_rust(&self) -> String {
        match self {
            CellExpr::SelfVal => "s".to_string(),
            CellExpr::North => "n".to_string(),
            CellExpr::South => "so".to_string(),
            CellExpr::East => "e".to_string(),
            CellExpr::West => "w_nb".to_string(),
            CellExpr::AtBottom => "at_bot".to_string(),
            CellExpr::Zero => "0i64".to_string(),
            CellExpr::Min(a, b) => format!("({}).min({})", a.emit_rust(), b.emit_rust()),
            CellExpr::Max(a, b) => format!("({}).max({})", a.emit_rust(), b.emit_rust()),
            CellExpr::IfPos(c, t, e) => {
                format!("(if {} > 0 {{ {} }} else {{ {} }})", c.emit_rust(), t.emit_rust(), e.emit_rust())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OmegaExpr {
    /// Positional input / bound variable `x0, x1, x2, ...`
    Arg(usize),
    /// Discrete scalar constant or fitted real constant
    Const(f64),
    /// Continuous differentiable real parameter slot `p[idx]` fitted via Dual-Number AD
    Param(usize),
    /// Arithmetic & Tropical primitives
    Add(Box<OmegaExpr>, Box<OmegaExpr>),
    Sub(Box<OmegaExpr>, Box<OmegaExpr>),
    Mul(Box<OmegaExpr>, Box<OmegaExpr>),
    DivSafe(Box<OmegaExpr>, Box<OmegaExpr>),
    Min(Box<OmegaExpr>, Box<OmegaExpr>),
    Max(Box<OmegaExpr>, Box<OmegaExpr>),
    Abs(Box<OmegaExpr>),
    SqrtSafe(Box<OmegaExpr>),
    /// Local binding (`Arg(0)` inside `body` is `bound`, outer args shifted +1)
    Let(Box<OmegaExpr>, Box<OmegaExpr>),
    /// Conditional branch: `if cond > 0.0 { then_e } else { else_e }`
    IfPos(Box<OmegaExpr>, Box<OmegaExpr>, Box<OmegaExpr>),
    /// First-class anonymous function `lambda v. body` (`Arg(0)` is parameter `v`)
    Lam(Box<OmegaExpr>),
    /// Function application `func(arg)` (beta-reduces `Lam` or applies closure!)
    App(Box<OmegaExpr>, Box<OmegaExpr>),
    /// General bounded-fuel recursion on `n = round(Arg(0))`:
    /// `if n <= 0 { base } else { step(n, rec(n - 1), outer...) }`
    Rec {
        base: Box<OmegaExpr>,
        step: Box<OmegaExpr>,
        max_fuel: usize,
    },
    /// Dynamic arbitrary-length list / higher-order functional combinators!
    ListInput,
    ListMap {
        fn_body: Box<OmegaExpr>, // `Arg(0)` = element `x`
        input: Box<OmegaExpr>,
    },
    ListFilterPos {
        pred: Box<OmegaExpr>,    // keep `x` iff `pred(x) > 0`
        input: Box<OmegaExpr>,
    },
    ListReduce {
        init: Box<OmegaExpr>,
        step: Box<OmegaExpr>,    // `Arg(0)` = elem `x`, `Arg(1)` = accumulator `acc`
        input: Box<OmegaExpr>,
    },
    ListLen(Box<OmegaExpr>),
    /// 2-register stateful sequence fold over `seq`: `(a, b) = (init_a, init_b)`
    FoldSeq {
        init_a: Box<OmegaExpr>,
        init_b: Box<OmegaExpr>,
        step_a: Box<OmegaExpr>,
        step_b: Box<OmegaExpr>,
        return_reg: u8,
    },
    /// Pure raw 2D coordinate transform (ZERO hardcoded MirrorH/FlipV macros!)
    GridCoordMap {
        r_expr: CoordExpr,
        c_expr: CoordExpr,
    },
    /// Pure raw 2D 4-neighbor cellular automaton rule (ZERO hardcoded Gravity/Ray/Hollow macros!)
    GridStencilRule {
        rule: CellExpr,
    },
    /// Sequential 2D composition
    Compose(Box<OmegaExpr>, Box<OmegaExpr>),
    /// Input-adaptive bounded fixed-point loop (`4x -> 16x` size extrapolation)
    BoundedFix {
        step: Box<OmegaExpr>,
        max_steps: usize,
    },
    /// Call to an autonomously invented concept stored in `UnboundedTropicalMemory`
    ConceptCall {
        id: usize,
        args: Vec<OmegaExpr>,
    },
}

/// Read-only concept lookup table for executing, inlining, compiling, and differentiating `ConceptCall`.
pub trait ConceptResolver {
    fn get_concept_body(&self, id: usize) -> Option<&OmegaExpr>;
    fn get_concept_name(&self, id: usize) -> Option<&str>;
}

pub struct EmptyResolver;
impl ConceptResolver for EmptyResolver {
    fn get_concept_body(&self, _id: usize) -> Option<&OmegaExpr> {
        None
    }
    fn get_concept_name(&self, _id: usize) -> Option<&str> {
        None
    }
}

/// Forward-mode dual number `(val, grad)` with up to `MAX_AD_PARAMS` continuous parameter partials.
pub const MAX_AD_PARAMS: usize = 6;

#[derive(Clone, Copy, Debug)]
pub struct Dual {
    pub val: f64,
    pub d: [f64; MAX_AD_PARAMS],
}

impl Dual {
    pub fn constant(v: f64) -> Self {
        Self { val: v, d: [0.0; MAX_AD_PARAMS] }
    }
    pub fn param(v: f64, idx: usize) -> Self {
        let mut d = [0.0; MAX_AD_PARAMS];
        if idx < MAX_AD_PARAMS {
            d[idx] = 1.0;
        }
        Self { val: v, d }
    }
    pub fn add(self, rhs: Self) -> Self {
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = self.d[i] + rhs.d[i];
        }
        Self { val: (self.val + rhs.val).clamp(-1e9, 1e9), d }
    }
    pub fn sub(self, rhs: Self) -> Self {
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = self.d[i] - rhs.d[i];
        }
        Self { val: (self.val - rhs.val).clamp(-1e9, 1e9), d }
    }
    pub fn mul(self, rhs: Self) -> Self {
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = (self.d[i] * rhs.val + self.val * rhs.d[i]).clamp(-1e9, 1e9);
        }
        Self { val: (self.val * rhs.val).clamp(-1e9, 1e9), d }
    }
    pub fn div_safe(self, rhs: Self) -> Self {
        let den = if rhs.val.abs() < 1e-3 {
            if rhs.val >= 0.0 { 1e-3 } else { -1e-3 }
        } else {
            rhs.val
        };
        let inv = 1.0 / den;
        let inv2 = inv * inv;
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = ((self.d[i] * den - self.val * rhs.d[i]) * inv2).clamp(-1e9, 1e9);
        }
        Self { val: (self.val * inv).clamp(-1e9, 1e9), d }
    }
    pub fn min(self, rhs: Self) -> Self {
        if self.val <= rhs.val { self } else { rhs }
    }
    pub fn max(self, rhs: Self) -> Self {
        if self.val >= rhs.val { self } else { rhs }
    }
    pub fn abs(self) -> Self {
        let s = if self.val >= 0.0 { 1.0 } else { -1.0 };
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = s * self.d[i];
        }
        Self { val: self.val.abs(), d }
    }
    pub fn sqrt_safe(self) -> Self {
        if self.val <= 1e-8 {
            return Self::constant(0.0);
        }
        let r = self.val.sqrt();
        let coeff = 0.5 / r;
        let mut d = [0.0; MAX_AD_PARAMS];
        for i in 0..MAX_AD_PARAMS {
            d[i] = (coeff * self.d[i]).clamp(-1e9, 1e9);
        }
        Self { val: r, d }
    }
}

impl OmegaExpr {
    pub fn mdl_size(&self) -> usize {
        match self {
            OmegaExpr::Arg(_) | OmegaExpr::Const(_) | OmegaExpr::ListInput => 1,
            OmegaExpr::Param(_) => 2,
            OmegaExpr::Abs(a)
            | OmegaExpr::SqrtSafe(a)
            | OmegaExpr::Lam(a)
            | OmegaExpr::ListLen(a) => 1 + a.mdl_size(),
            OmegaExpr::Add(a, b)
            | OmegaExpr::Sub(a, b)
            | OmegaExpr::Mul(a, b)
            | OmegaExpr::DivSafe(a, b)
            | OmegaExpr::Min(a, b)
            | OmegaExpr::Max(a, b)
            | OmegaExpr::Let(a, b)
            | OmegaExpr::App(a, b)
            | OmegaExpr::Compose(a, b) => 1 + a.mdl_size() + b.mdl_size(),
            OmegaExpr::IfPos(c, t, e) => 1 + c.mdl_size() + t.mdl_size() + e.mdl_size(),
            OmegaExpr::Rec { base, step, .. } => 2 + base.mdl_size() + step.mdl_size(),
            OmegaExpr::ListMap { fn_body, input } => 1 + fn_body.mdl_size() + input.mdl_size(),
            OmegaExpr::ListFilterPos { pred, input } => 1 + pred.mdl_size() + input.mdl_size(),
            OmegaExpr::ListReduce { init, step, input } => 2 + init.mdl_size() + step.mdl_size() + input.mdl_size(),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, .. } => {
                2 + init_a.mdl_size() + init_b.mdl_size() + step_a.mdl_size() + step_b.mdl_size()
            }
            OmegaExpr::GridCoordMap { r_expr, c_expr } => 1 + r_expr.mdl_size() + c_expr.mdl_size(),
            OmegaExpr::GridStencilRule { rule } => 1 + rule.mdl_size(),
            OmegaExpr::BoundedFix { step, .. } => 2 + step.mdl_size(),
            OmegaExpr::ConceptCall { args, .. } => 1 + args.iter().map(|a| a.mdl_size()).sum::<usize>(),
        }
    }

    pub fn num_params(&self) -> usize {
        fn walk(e: &OmegaExpr, max_p: &mut usize) {
            match e {
                OmegaExpr::Param(k) => *max_p = (*max_p).max(*k + 1),
                OmegaExpr::Abs(a)
                | OmegaExpr::SqrtSafe(a)
                | OmegaExpr::Lam(a)
                | OmegaExpr::ListLen(a) => walk(a, max_p),
                OmegaExpr::Add(a, b)
                | OmegaExpr::Sub(a, b)
                | OmegaExpr::Mul(a, b)
                | OmegaExpr::DivSafe(a, b)
                | OmegaExpr::Min(a, b)
                | OmegaExpr::Max(a, b)
                | OmegaExpr::Let(a, b)
                | OmegaExpr::App(a, b)
                | OmegaExpr::Compose(a, b) => {
                    walk(a, max_p);
                    walk(b, max_p);
                }
                OmegaExpr::IfPos(c, t, el) => {
                    walk(c, max_p);
                    walk(t, max_p);
                    walk(el, max_p);
                }
                OmegaExpr::Rec { base, step, .. } => {
                    walk(base, max_p);
                    walk(step, max_p);
                }
                OmegaExpr::ListMap { fn_body, input } => {
                    walk(fn_body, max_p);
                    walk(input, max_p);
                }
                OmegaExpr::ListFilterPos { pred, input } => {
                    walk(pred, max_p);
                    walk(input, max_p);
                }
                OmegaExpr::ListReduce { init, step, input } => {
                    walk(init, max_p);
                    walk(step, max_p);
                    walk(input, max_p);
                }
                OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, .. } => {
                    walk(init_a, max_p);
                    walk(init_b, max_p);
                    walk(step_a, max_p);
                    walk(step_b, max_p);
                }
                OmegaExpr::BoundedFix { step, .. } => walk(step, max_p),
                OmegaExpr::ConceptCall { args, .. } => {
                    for a in args {
                        walk(a, max_p);
                    }
                }
                _ => {}
            }
        }
        let mut m = 0;
        walk(self, &mut m);
        m
    }

    pub fn bind_params(&self, params: &[f64]) -> OmegaExpr {
        match self {
            OmegaExpr::Param(k) => OmegaExpr::Const(*params.get(*k).unwrap_or(&0.0)),
            OmegaExpr::Arg(i) => OmegaExpr::Arg(*i),
            OmegaExpr::Const(c) => OmegaExpr::Const(*c),
            OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(a.bind_params(params))),
            OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(a.bind_params(params))),
            OmegaExpr::Let(a, b) => OmegaExpr::Let(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Lam(a) => OmegaExpr::Lam(Box::new(a.bind_params(params))),
            OmegaExpr::App(a, b) => OmegaExpr::App(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::Rec { base, step, max_fuel } => OmegaExpr::Rec {
                base: Box::new(base.bind_params(params)),
                step: Box::new(step.bind_params(params)),
                max_fuel: *max_fuel,
            },
            OmegaExpr::ListMap { fn_body, input } => OmegaExpr::ListMap {
                fn_body: Box::new(fn_body.bind_params(params)),
                input: Box::new(input.bind_params(params)),
            },
            OmegaExpr::ListFilterPos { pred, input } => OmegaExpr::ListFilterPos {
                pred: Box::new(pred.bind_params(params)),
                input: Box::new(input.bind_params(params)),
            },
            OmegaExpr::ListReduce { init, step, input } => OmegaExpr::ListReduce {
                init: Box::new(init.bind_params(params)),
                step: Box::new(step.bind_params(params)),
                input: Box::new(input.bind_params(params)),
            },
            OmegaExpr::ListLen(a) => OmegaExpr::ListLen(Box::new(a.bind_params(params))),
            OmegaExpr::IfPos(c, t, e) => OmegaExpr::IfPos(
                Box::new(c.bind_params(params)),
                Box::new(t.bind_params(params)),
                Box::new(e.bind_params(params)),
            ),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => OmegaExpr::FoldSeq {
                init_a: Box::new(init_a.bind_params(params)),
                init_b: Box::new(init_b.bind_params(params)),
                step_a: Box::new(step_a.bind_params(params)),
                step_b: Box::new(step_b.bind_params(params)),
                return_reg: *return_reg,
            },
            OmegaExpr::Compose(a, b) => OmegaExpr::Compose(Box::new(a.bind_params(params)), Box::new(b.bind_params(params))),
            OmegaExpr::BoundedFix { step, max_steps } => OmegaExpr::BoundedFix {
                step: Box::new(step.bind_params(params)),
                max_steps: *max_steps,
            },
            OmegaExpr::ConceptCall { id, args } => OmegaExpr::ConceptCall {
                id: *id,
                args: args.iter().map(|a| a.bind_params(params)).collect(),
            },
            other => other.clone(),
        }
    }

    pub fn substitute(&self, args: &[OmegaExpr]) -> OmegaExpr {
        fn shift(e: &OmegaExpr, delta: usize, cutoff: usize) -> OmegaExpr {
            match e {
                OmegaExpr::Arg(i) => OmegaExpr::Arg(if *i >= cutoff { *i + delta } else { *i }),
                OmegaExpr::Const(c) => OmegaExpr::Const(*c),
                OmegaExpr::Param(k) => OmegaExpr::Param(*k),
                OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(shift(a, delta, cutoff)), Box::new(shift(b, delta, cutoff))),
                OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(shift(a, delta, cutoff))),
                OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(shift(a, delta, cutoff))),
                OmegaExpr::Let(b, body) => OmegaExpr::Let(Box::new(shift(b, delta, cutoff)), Box::new(shift(body, delta, cutoff + 1))),
                OmegaExpr::Lam(body) => OmegaExpr::Lam(Box::new(shift(body, delta, cutoff + 1))),
                OmegaExpr::App(f, a) => OmegaExpr::App(Box::new(shift(f, delta, cutoff)), Box::new(shift(a, delta, cutoff))),
                OmegaExpr::IfPos(c, t, el) => OmegaExpr::IfPos(
                    Box::new(shift(c, delta, cutoff)),
                    Box::new(shift(t, delta, cutoff)),
                    Box::new(shift(el, delta, cutoff)),
                ),
                OmegaExpr::ConceptCall { id, args } => OmegaExpr::ConceptCall {
                    id: *id,
                    args: args.iter().map(|a| shift(a, delta, cutoff)).collect(),
                },
                other => other.clone(),
            }
        }
        fn sub_rec(e: &OmegaExpr, args: &[OmegaExpr], depth: usize) -> OmegaExpr {
            match e {
                OmegaExpr::Arg(i) => {
                    if *i < depth {
                        OmegaExpr::Arg(*i)
                    } else if let Some(rep) = args.get(*i - depth) {
                        shift(rep, depth, 0)
                    } else {
                        OmegaExpr::Arg(*i)
                    }
                }
                OmegaExpr::Const(c) => OmegaExpr::Const(*c),
                OmegaExpr::Param(k) => OmegaExpr::Param(*k),
                OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(sub_rec(a, args, depth))),
                OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(sub_rec(a, args, depth))),
                OmegaExpr::Let(b, body) => OmegaExpr::Let(
                    Box::new(sub_rec(b, args, depth)),
                    Box::new(sub_rec(body, args, depth + 1)),
                ),
                OmegaExpr::Lam(body) => OmegaExpr::Lam(Box::new(sub_rec(body, args, depth + 1))),
                OmegaExpr::App(f, a) => OmegaExpr::App(Box::new(sub_rec(f, args, depth)), Box::new(sub_rec(a, args, depth))),
                OmegaExpr::IfPos(c, t, el) => OmegaExpr::IfPos(
                    Box::new(sub_rec(c, args, depth)),
                    Box::new(sub_rec(t, args, depth)),
                    Box::new(sub_rec(el, args, depth)),
                ),
                OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => OmegaExpr::FoldSeq {
                    init_a: Box::new(sub_rec(init_a, args, depth)),
                    init_b: Box::new(sub_rec(init_b, args, depth)),
                    step_a: step_a.clone(),
                    step_b: step_b.clone(),
                    return_reg: *return_reg,
                },
                OmegaExpr::Compose(a, b) => OmegaExpr::Compose(Box::new(sub_rec(a, args, depth)), Box::new(sub_rec(b, args, depth))),
                OmegaExpr::BoundedFix { step, max_steps } => OmegaExpr::BoundedFix {
                    step: Box::new(sub_rec(step, args, depth)),
                    max_steps: *max_steps,
                },
                OmegaExpr::ConceptCall { id, args: cargs } => OmegaExpr::ConceptCall {
                    id: *id,
                    args: cargs.iter().map(|a| sub_rec(a, args, depth)).collect(),
                },
                other => other.clone(),
            }
        }
        sub_rec(self, args, 0)
    }

    pub fn inline_all<R: ConceptResolver>(&self, resolver: &R, depth_limit: usize) -> OmegaExpr {
        if depth_limit == 0 {
            return self.clone();
        }
        match self {
            OmegaExpr::ConceptCall { id, args } => {
                let inlined_args: Vec<OmegaExpr> = args.iter().map(|a| a.inline_all(resolver, depth_limit - 1)).collect();
                if let Some(body) = resolver.get_concept_body(*id) {
                    body.substitute(&inlined_args).inline_all(resolver, depth_limit - 1)
                } else {
                    OmegaExpr::ConceptCall { id: *id, args: inlined_args }
                }
            }
            OmegaExpr::App(f, a) => {
                let fi = f.inline_all(resolver, depth_limit - 1);
                let ai = a.inline_all(resolver, depth_limit - 1);
                if let OmegaExpr::Lam(body) = fi {
                    let mut sub = vec![ai, OmegaExpr::Arg(0), OmegaExpr::Arg(1), OmegaExpr::Arg(2)];
                    let _ = &mut sub;
                    body.substitute(&sub).inline_all(resolver, depth_limit - 1)
                } else {
                    OmegaExpr::App(Box::new(fi), Box::new(ai))
                }
            }
            OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(a.inline_all(resolver, depth_limit))),
            OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(a.inline_all(resolver, depth_limit))),
            OmegaExpr::Let(a, b) => OmegaExpr::Let(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::Lam(a) => OmegaExpr::Lam(Box::new(a.inline_all(resolver, depth_limit))),
            OmegaExpr::Rec { base, step, max_fuel } => OmegaExpr::Rec {
                base: Box::new(base.inline_all(resolver, depth_limit)),
                step: Box::new(step.inline_all(resolver, depth_limit)),
                max_fuel: *max_fuel,
            },
            OmegaExpr::ListMap { fn_body, input } => OmegaExpr::ListMap {
                fn_body: Box::new(fn_body.inline_all(resolver, depth_limit)),
                input: Box::new(input.inline_all(resolver, depth_limit)),
            },
            OmegaExpr::ListFilterPos { pred, input } => OmegaExpr::ListFilterPos {
                pred: Box::new(pred.inline_all(resolver, depth_limit)),
                input: Box::new(input.inline_all(resolver, depth_limit)),
            },
            OmegaExpr::ListReduce { init, step, input } => OmegaExpr::ListReduce {
                init: Box::new(init.inline_all(resolver, depth_limit)),
                step: Box::new(step.inline_all(resolver, depth_limit)),
                input: Box::new(input.inline_all(resolver, depth_limit)),
            },
            OmegaExpr::ListLen(a) => OmegaExpr::ListLen(Box::new(a.inline_all(resolver, depth_limit))),
            OmegaExpr::IfPos(c, t, e) => OmegaExpr::IfPos(
                Box::new(c.inline_all(resolver, depth_limit)),
                Box::new(t.inline_all(resolver, depth_limit)),
                Box::new(e.inline_all(resolver, depth_limit)),
            ),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => OmegaExpr::FoldSeq {
                init_a: Box::new(init_a.inline_all(resolver, depth_limit)),
                init_b: Box::new(init_b.inline_all(resolver, depth_limit)),
                step_a: Box::new(step_a.inline_all(resolver, depth_limit)),
                step_b: Box::new(step_b.inline_all(resolver, depth_limit)),
                return_reg: *return_reg,
            },
            OmegaExpr::Compose(a, b) => OmegaExpr::Compose(Box::new(a.inline_all(resolver, depth_limit)), Box::new(b.inline_all(resolver, depth_limit))),
            OmegaExpr::BoundedFix { step, max_steps } => OmegaExpr::BoundedFix {
                step: Box::new(step.inline_all(resolver, depth_limit)),
                max_steps: *max_steps,
            },
            other => other.clone(),
        }
    }

    pub fn canonicalize(&self) -> OmegaExpr {
        let mut cur = self.clone();
        for _ in 0..4 {
            let next = cur.canonicalize_once();
            if next == cur {
                break;
            }
            cur = next;
        }
        cur
    }

    fn canonicalize_once(&self) -> OmegaExpr {
        match self {
            OmegaExpr::Add(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) => OmegaExpr::Const(x + y),
                    (OmegaExpr::Const(x), _) if x.abs() < 1e-12 => cb,
                    (_, OmegaExpr::Const(y)) if y.abs() < 1e-12 => ca,
                    _ => {
                        if ca.canonical_key() <= cb.canonical_key() {
                            OmegaExpr::Add(Box::new(ca), Box::new(cb))
                        } else {
                            OmegaExpr::Add(Box::new(cb), Box::new(ca))
                        }
                    }
                }
            }
            OmegaExpr::Sub(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                if ca == cb {
                    return OmegaExpr::Const(0.0);
                }
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) => OmegaExpr::Const(x - y),
                    (_, OmegaExpr::Const(y)) if y.abs() < 1e-12 => ca,
                    _ => OmegaExpr::Sub(Box::new(ca), Box::new(cb)),
                }
            }
            OmegaExpr::Mul(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) => OmegaExpr::Const(x * y),
                    (OmegaExpr::Const(x), _) if x.abs() < 1e-12 => OmegaExpr::Const(0.0),
                    (_, OmegaExpr::Const(y)) if y.abs() < 1e-12 => OmegaExpr::Const(0.0),
                    (OmegaExpr::Const(x), _) if (*x - 1.0).abs() < 1e-12 => cb,
                    (_, OmegaExpr::Const(y)) if (*y - 1.0).abs() < 1e-12 => ca,
                    _ => {
                        if ca.canonical_key() <= cb.canonical_key() {
                            OmegaExpr::Mul(Box::new(ca), Box::new(cb))
                        } else {
                            OmegaExpr::Mul(Box::new(cb), Box::new(ca))
                        }
                    }
                }
            }
            OmegaExpr::DivSafe(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), _) if x.abs() < 1e-12 => OmegaExpr::Const(0.0),
                    (_, OmegaExpr::Const(y)) if (*y - 1.0).abs() < 1e-12 => ca,
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) if y.abs() >= 1e-3 => OmegaExpr::Const(x / y),
                    _ => OmegaExpr::DivSafe(Box::new(ca), Box::new(cb)),
                }
            }
            OmegaExpr::Min(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                if ca == cb {
                    return ca;
                }
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) => OmegaExpr::Const(x.min(*y)),
                    _ => {
                        if ca.canonical_key() <= cb.canonical_key() {
                            OmegaExpr::Min(Box::new(ca), Box::new(cb))
                        } else {
                            OmegaExpr::Min(Box::new(cb), Box::new(ca))
                        }
                    }
                }
            }
            OmegaExpr::Max(a, b) => {
                let ca = a.canonicalize_once();
                let cb = b.canonicalize_once();
                if ca == cb {
                    return ca;
                }
                match (&ca, &cb) {
                    (OmegaExpr::Const(x), OmegaExpr::Const(y)) => OmegaExpr::Const(x.max(*y)),
                    _ => {
                        if ca.canonical_key() <= cb.canonical_key() {
                            OmegaExpr::Max(Box::new(ca), Box::new(cb))
                        } else {
                            OmegaExpr::Max(Box::new(cb), Box::new(ca))
                        }
                    }
                }
            }
            OmegaExpr::Abs(a) => {
                let ca = a.canonicalize_once();
                match ca {
                    OmegaExpr::Const(x) => OmegaExpr::Const(x.abs()),
                    OmegaExpr::Abs(inner) => OmegaExpr::Abs(inner),
                    OmegaExpr::SqrtSafe(inner) => OmegaExpr::SqrtSafe(inner),
                    other => OmegaExpr::Abs(Box::new(other)),
                }
            }
            OmegaExpr::SqrtSafe(a) => {
                let ca = a.canonicalize_once();
                match ca {
                    OmegaExpr::Const(x) => OmegaExpr::Const(if x > 0.0 { x.sqrt() } else { 0.0 }),
                    other => OmegaExpr::SqrtSafe(Box::new(other)),
                }
            }
            OmegaExpr::Let(b, body) => OmegaExpr::Let(Box::new(b.canonicalize_once()), Box::new(body.canonicalize_once())),
            OmegaExpr::Lam(body) => OmegaExpr::Lam(Box::new(body.canonicalize_once())),
            OmegaExpr::App(f, a) => {
                let cf = f.canonicalize_once();
                let ca = a.canonicalize_once();
                if let OmegaExpr::Lam(body) = cf {
                    let sub = [ca, OmegaExpr::Arg(0), OmegaExpr::Arg(1), OmegaExpr::Arg(2)];
                    body.substitute(&sub).canonicalize_once()
                } else {
                    OmegaExpr::App(Box::new(cf), Box::new(ca))
                }
            }
            OmegaExpr::Rec { base, step, max_fuel } => OmegaExpr::Rec {
                base: Box::new(base.canonicalize_once()),
                step: Box::new(step.canonicalize_once()),
                max_fuel: *max_fuel,
            },
            OmegaExpr::ListMap { fn_body, input } => OmegaExpr::ListMap {
                fn_body: Box::new(fn_body.canonicalize_once()),
                input: Box::new(input.canonicalize_once()),
            },
            OmegaExpr::ListFilterPos { pred, input } => OmegaExpr::ListFilterPos {
                pred: Box::new(pred.canonicalize_once()),
                input: Box::new(input.canonicalize_once()),
            },
            OmegaExpr::ListReduce { init, step, input } => OmegaExpr::ListReduce {
                init: Box::new(init.canonicalize_once()),
                step: Box::new(step.canonicalize_once()),
                input: Box::new(input.canonicalize_once()),
            },
            OmegaExpr::ListLen(a) => OmegaExpr::ListLen(Box::new(a.canonicalize_once())),
            OmegaExpr::IfPos(c, t, e) => {
                let cc = c.canonicalize_once();
                let ct = t.canonicalize_once();
                let ce = e.canonicalize_once();
                if ct == ce {
                    return ct;
                }
                match cc {
                    OmegaExpr::Const(v) => if v > 0.0 { ct } else { ce },
                    other => OmegaExpr::IfPos(Box::new(other), Box::new(ct), Box::new(ce)),
                }
            }
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => OmegaExpr::FoldSeq {
                init_a: Box::new(init_a.canonicalize_once()),
                init_b: Box::new(init_b.canonicalize_once()),
                step_a: Box::new(step_a.canonicalize_once()),
                step_b: Box::new(step_b.canonicalize_once()),
                return_reg: *return_reg,
            },
            OmegaExpr::Compose(a, b) => OmegaExpr::Compose(Box::new(a.canonicalize_once()), Box::new(b.canonicalize_once())),
            OmegaExpr::BoundedFix { step, max_steps } => OmegaExpr::BoundedFix {
                step: Box::new(step.canonicalize_once()),
                max_steps: *max_steps,
            },
            OmegaExpr::ConceptCall { id, args } => OmegaExpr::ConceptCall {
                id: *id,
                args: args.iter().map(|a| a.canonicalize_once()).collect(),
            },
            other => other.clone(),
        }
    }

    pub fn canonical_key(&self) -> String {
        match self {
            OmegaExpr::Arg(i) => format!("x{}", i),
            OmegaExpr::Const(c) => {
                if (c - c.round()).abs() < 1e-6 {
                    format!("{}", c.round() as i64)
                } else {
                    format!("{:.4}", c)
                }
            }
            OmegaExpr::Param(k) => format!("p{}", k),
            OmegaExpr::Add(a, b) => format!("(+ {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::Sub(a, b) => format!("(- {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::Mul(a, b) => format!("(* {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::DivSafe(a, b) => format!("(/ {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::Min(a, b) => format!("(min {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::Max(a, b) => format!("(max {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::Abs(a) => format!("(abs {})", a.canonical_key()),
            OmegaExpr::SqrtSafe(a) => format!("(sqrt {})", a.canonical_key()),
            OmegaExpr::Let(b, body) => format!("(let {} {})", b.canonical_key(), body.canonical_key()),
            OmegaExpr::Lam(body) => format!("(lam {})", body.canonical_key()),
            OmegaExpr::App(f, a) => format!("(app {} {})", f.canonical_key(), a.canonical_key()),
            OmegaExpr::Rec { base, step, .. } => format!("(rec {} {})", base.canonical_key(), step.canonical_key()),
            OmegaExpr::ListInput => "seq".to_string(),
            OmegaExpr::ListMap { fn_body, input } => format!("(map {} {})", fn_body.canonical_key(), input.canonical_key()),
            OmegaExpr::ListFilterPos { pred, input } => format!("(filter>0 {} {})", pred.canonical_key(), input.canonical_key()),
            OmegaExpr::ListReduce { init, step, input } => {
                format!("(reduce {} {} {})", init.canonical_key(), step.canonical_key(), input.canonical_key())
            }
            OmegaExpr::ListLen(a) => format!("(len {})", a.canonical_key()),
            OmegaExpr::IfPos(c, t, e) => format!("(if>0 {} {} {})", c.canonical_key(), t.canonical_key(), e.canonical_key()),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => format!(
                "(fold r{} ({},{}) [{} ; {}])",
                return_reg,
                init_a.canonical_key(),
                init_b.canonical_key(),
                step_a.canonical_key(),
                step_b.canonical_key()
            ),
            OmegaExpr::GridCoordMap { r_expr, c_expr } => format!("(gmap {} {})", r_expr.key(), c_expr.key()),
            OmegaExpr::GridStencilRule { rule } => format!("(gstencil {})", rule.key()),
            OmegaExpr::Compose(a, b) => format!("(seq {} {})", a.canonical_key(), b.canonical_key()),
            OmegaExpr::BoundedFix { step, max_steps } => format!("(fix[{}] {})", max_steps, step.canonical_key()),
            OmegaExpr::ConceptCall { id, args } => {
                let s: Vec<String> = args.iter().map(|a| a.canonical_key()).collect();
                format!("(C{} {})", id, s.join(" "))
            }
        }
    }

    pub fn pretty<R: ConceptResolver>(&self, resolver: &R) -> String {
        match self {
            OmegaExpr::Arg(i) => format!("x{}", i),
            OmegaExpr::Const(c) => {
                if (c - c.round()).abs() < 1e-6 {
                    format!("{}", c.round() as i64)
                } else {
                    format!("{:.4}", c)
                }
            }
            OmegaExpr::Param(k) => format!("p{}", k),
            OmegaExpr::Add(a, b) => format!("({} + {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::Sub(a, b) => format!("({} - {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::Mul(a, b) => format!("({} * {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::DivSafe(a, b) => format!("({} / {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::Min(a, b) => format!("min({}, {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::Max(a, b) => format!("max({}, {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::Abs(a) => format!("abs({})", a.pretty(resolver)),
            OmegaExpr::SqrtSafe(a) => format!("sqrt({})", a.pretty(resolver)),
            OmegaExpr::Let(b, body) => format!("(let v={} in {})", b.pretty(resolver), body.pretty(resolver)),
            OmegaExpr::Lam(body) => format!("(λ. {})", body.pretty(resolver)),
            OmegaExpr::App(f, a) => format!("({} {})", f.pretty(resolver), a.pretty(resolver)),
            OmegaExpr::Rec { base, step, .. } => format!("Rec(base={}, step={})", base.pretty(resolver), step.pretty(resolver)),
            OmegaExpr::ListInput => "seq".to_string(),
            OmegaExpr::ListMap { fn_body, input } => format!("map(λx0. {}, {})", fn_body.pretty(resolver), input.pretty(resolver)),
            OmegaExpr::ListFilterPos { pred, input } => format!("filter(λx0. {} > 0, {})", pred.pretty(resolver), input.pretty(resolver)),
            OmegaExpr::ListReduce { init, step, input } => format!(
                "reduce(init={}, λ(x0,x1). {}, {})",
                init.pretty(resolver),
                step.pretty(resolver),
                input.pretty(resolver)
            ),
            OmegaExpr::ListLen(a) => format!("len({})", a.pretty(resolver)),
            OmegaExpr::IfPos(c, t, e) => format!("if({} > 0 ? {} : {})", c.pretty(resolver), t.pretty(resolver), e.pretty(resolver)),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => format!(
                "FoldSeq(init=({},{}), a'={}, b'={}, ret={})",
                init_a.pretty(resolver),
                init_b.pretty(resolver),
                step_a.pretty(resolver),
                step_b.pretty(resolver),
                if *return_reg == 0 { "a" } else { "b" }
            ),
            OmegaExpr::GridCoordMap { r_expr, c_expr } => format!("CoordMap({}, {})", r_expr.key(), c_expr.key()),
            OmegaExpr::GridStencilRule { rule } => format!("Stencil({})", rule.key()),
            OmegaExpr::Compose(a, b) => format!("({} |> {})", a.pretty(resolver), b.pretty(resolver)),
            OmegaExpr::BoundedFix { step, max_steps } => format!("BoundedFix[{}]({})", max_steps, step.pretty(resolver)),
            OmegaExpr::ConceptCall { id, args } => {
                let name = resolver
                    .get_concept_name(*id)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("C{}", id));
                let a: Vec<String> = args.iter().map(|e| e.pretty(resolver)).collect();
                format!("{}({})", name, a.join(", "))
            }
        }
    }

    /// Evaluate dynamic list-producing expressions (`ListInput`, `ListMap`, `ListFilterPos`).
    pub fn eval_list<R: ConceptResolver>(
        &self,
        args: &[f64],
        params: &[f64],
        seq_input: Option<&[f64]>,
        resolver: &R,
        leaf_ops: &mut usize,
    ) -> Vec<f64> {
        match self {
            OmegaExpr::ListInput => seq_input.map(|s| s.to_vec()).unwrap_or_default(),
            OmegaExpr::ListMap { fn_body, input } => {
                let xs = input.eval_list(args, params, seq_input, resolver, leaf_ops);
                xs.into_iter()
                    .map(|x| fn_body.eval_num_with_params(&[x, args.first().copied().unwrap_or(0.0), 0.0], params, None, resolver, leaf_ops))
                    .collect()
            }
            OmegaExpr::ListFilterPos { pred, input } => {
                let xs = input.eval_list(args, params, seq_input, resolver, leaf_ops);
                xs.into_iter()
                    .filter(|&x| pred.eval_num_with_params(&[x, args.first().copied().unwrap_or(0.0), 0.0], params, None, resolver, leaf_ops) > 0.0)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Evaluate scalar / functional / recursive / dynamic-list `OmegaExpr` with truthful leaf-op accounting.
    pub fn eval_num<R: ConceptResolver>(
        &self,
        args: &[f64],
        seq_input: Option<&[f64]>,
        resolver: &R,
        leaf_ops: &mut usize,
    ) -> f64 {
        self.eval_num_with_params(args, &[], seq_input, resolver, leaf_ops)
    }

    pub fn eval_num_with_params<R: ConceptResolver>(
        &self,
        args: &[f64],
        params: &[f64],
        seq_input: Option<&[f64]>,
        resolver: &R,
        leaf_ops: &mut usize,
    ) -> f64 {
        fn rec<R: ConceptResolver>(
            e: &OmegaExpr,
            args: &[f64],
            params: &[f64],
            seq_input: Option<&[f64]>,
            resolver: &R,
            leaf_ops: &mut usize,
            depth: usize,
        ) -> f64 {
            if depth > 48 {
                return 0.0;
            }
            *leaf_ops += 1;
            let v = match e {
                OmegaExpr::Arg(i) => *args.get(*i).unwrap_or(&0.0),
                OmegaExpr::Const(c) => *c,
                OmegaExpr::Param(k) => *params.get(*k).unwrap_or(&1.0),
                OmegaExpr::Add(a, b) => {
                    rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1)
                        + rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1)
                }
                OmegaExpr::Sub(a, b) => {
                    rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1)
                        - rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1)
                }
                OmegaExpr::Mul(a, b) => {
                    rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1)
                        * rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1)
                }
                OmegaExpr::DivSafe(a, b) => {
                    let num = rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    let den = rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    let safe_den = if den.abs() < 1e-3 {
                        if den >= 0.0 { 1e-3 } else { -1e-3 }
                    } else {
                        den
                    };
                    num / safe_den
                }
                OmegaExpr::Min(a, b) => rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1)
                    .min(rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1)),
                OmegaExpr::Max(a, b) => rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1)
                    .max(rec(b, args, params, seq_input, resolver, leaf_ops, depth + 1)),
                OmegaExpr::Abs(a) => rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1).abs(),
                OmegaExpr::SqrtSafe(a) => {
                    let x = rec(a, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    if x > 0.0 { x.sqrt() } else { 0.0 }
                }
                OmegaExpr::Let(bound, body) => {
                    let bv = rec(bound, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    let mut next_args = Vec::with_capacity(args.len() + 1);
                    next_args.push(bv);
                    next_args.extend_from_slice(args);
                    rec(body, &next_args, params, seq_input, resolver, leaf_ops, depth + 1)
                }
                OmegaExpr::App(f, arg) => {
                    let av = rec(arg, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    if let OmegaExpr::Lam(body) = f.as_ref() {
                        let mut next_args = Vec::with_capacity(args.len() + 1);
                        next_args.push(av);
                        next_args.extend_from_slice(args);
                        rec(body, &next_args, params, seq_input, resolver, leaf_ops, depth + 1)
                    } else {
                        rec(f, &[av], params, seq_input, resolver, leaf_ops, depth + 1)
                    }
                }
                OmegaExpr::Rec { base, step, max_fuel } => {
                    let n_raw = args.first().copied().unwrap_or(0.0).round() as i64;
                    let n = n_raw.clamp(0, (*max_fuel) as i64) as usize;
                    let mut acc = rec(base, args, params, seq_input, resolver, leaf_ops, depth + 1);
                    let x1 = args.get(1).copied().unwrap_or(0.0);
                    for k in 1..=n {
                        acc = rec(step, &[k as f64, acc, x1], params, seq_input, resolver, leaf_ops, depth + 1);
                    }
                    acc
                }
                OmegaExpr::ListReduce { init, step, input } => {
                    let xs = input.eval_list(args, params, seq_input, resolver, leaf_ops);
                    let mut acc = rec(init, args, params, None, resolver, leaf_ops, depth + 1);
                    for x in xs {
                        acc = rec(step, &[x, acc, 0.0], params, None, resolver, leaf_ops, depth + 1);
                    }
                    acc
                }
                OmegaExpr::ListLen(input) => {
                    input.eval_list(args, params, seq_input, resolver, leaf_ops).len() as f64
                }
                OmegaExpr::IfPos(c, t, el) => {
                    if rec(c, args, params, seq_input, resolver, leaf_ops, depth + 1) > 0.0 {
                        rec(t, args, params, seq_input, resolver, leaf_ops, depth + 1)
                    } else {
                        rec(el, args, params, seq_input, resolver, leaf_ops, depth + 1)
                    }
                }
                OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => {
                    let mut a = rec(init_a, args, params, None, resolver, leaf_ops, depth + 1);
                    let mut b = rec(init_b, args, params, None, resolver, leaf_ops, depth + 1);
                    if let Some(seq) = seq_input {
                        for &xt in seq {
                            let s1 = [xt, a, b];
                            a = rec(step_a, &s1, params, None, resolver, leaf_ops, depth + 1);
                            let s2 = [xt, a, b];
                            b = rec(step_b, &s2, params, None, resolver, leaf_ops, depth + 1);
                        }
                    }
                    if *return_reg == 0 { a } else { b }
                }
                OmegaExpr::ConceptCall { id, args: cargs } => {
                    let evaled: Vec<f64> = cargs
                        .iter()
                        .map(|ca| rec(ca, args, params, seq_input, resolver, leaf_ops, depth + 1))
                        .collect();
                    if let Some(body) = resolver.get_concept_body(*id) {
                        rec(body, &evaled, params, seq_input, resolver, leaf_ops, depth + 1)
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            if v.is_finite() { v.clamp(-1e9, 1e9) } else { 0.0 }
        }
        rec(self, args, params, seq_input, resolver, leaf_ops, 0)
    }

    /// Exact Forward-Mode Dual-Number Automatic Differentiation w.r.t. `Param(0..MAX_AD_PARAMS)`.
    pub fn eval_dual<R: ConceptResolver>(&self, args: &[f64], params: &[f64], resolver: &R) -> Dual {
        fn rec<R: ConceptResolver>(
            e: &OmegaExpr,
            args: &[f64],
            params: &[f64],
            resolver: &R,
            depth: usize,
        ) -> Dual {
            if depth > 40 {
                return Dual::constant(0.0);
            }
            match e {
                OmegaExpr::Arg(i) => Dual::constant(*args.get(*i).unwrap_or(&0.0)),
                OmegaExpr::Const(c) => Dual::constant(*c),
                OmegaExpr::Param(k) => Dual::param(*params.get(*k).unwrap_or(&1.0), *k),
                OmegaExpr::Add(a, b) => rec(a, args, params, resolver, depth + 1).add(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::Sub(a, b) => rec(a, args, params, resolver, depth + 1).sub(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::Mul(a, b) => rec(a, args, params, resolver, depth + 1).mul(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::DivSafe(a, b) => rec(a, args, params, resolver, depth + 1).div_safe(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::Min(a, b) => rec(a, args, params, resolver, depth + 1).min(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::Max(a, b) => rec(a, args, params, resolver, depth + 1).max(rec(b, args, params, resolver, depth + 1)),
                OmegaExpr::Abs(a) => rec(a, args, params, resolver, depth + 1).abs(),
                OmegaExpr::SqrtSafe(a) => rec(a, args, params, resolver, depth + 1).sqrt_safe(),
                OmegaExpr::Let(bound, body) => {
                    let mut sub_args = vec![(**bound).clone()];
                    for idx in 0..args.len() {
                        sub_args.push(OmegaExpr::Arg(idx));
                    }
                    let expanded = body.substitute(&sub_args);
                    rec(&expanded, args, params, resolver, depth + 1)
                }
                OmegaExpr::IfPos(c, t, el) => {
                    if rec(c, args, params, resolver, depth + 1).val > 0.0 {
                        rec(t, args, params, resolver, depth + 1)
                    } else {
                        rec(el, args, params, resolver, depth + 1)
                    }
                }
                OmegaExpr::ConceptCall { id, args: cargs } => {
                    if let Some(body) = resolver.get_concept_body(*id) {
                        let expanded = body.substitute(cargs);
                        rec(&expanded, args, params, resolver, depth + 1)
                    } else {
                        Dual::constant(0.0)
                    }
                }
                _ => Dual::constant(0.0),
            }
        }
        rec(self, args, params, resolver, 0)
    }

    /// Execute pure raw 2D spatial cellular-stencil & coordinate-map `OmegaExpr` (ZERO hardcoded grid macros!).
    pub fn eval_grid<R: ConceptResolver>(&self, g: &Grid2D, resolver: &R, leaf_ops: &mut usize) -> Grid2D {
        match self {
            OmegaExpr::Arg(_) => g.clone(),
            OmegaExpr::GridCoordMap { r_expr, c_expr } => {
                let h = g.len();
                let w = g.first().map(|r| r.len()).unwrap_or(0);
                *leaf_ops += h * w;
                let mut out = vec![vec![0i64; w]; h];
                for r in 0..h {
                    for c in 0..w {
                        let sr = r_expr.eval(r, c, h, w);
                        let sc = c_expr.eval(r, c, h, w);
                        if sr >= 0 && sc >= 0 && (sr as usize) < h && (sc as usize) < w {
                            out[r][c] = g[sr as usize][sc as usize];
                        }
                    }
                }
                out
            }
            OmegaExpr::GridStencilRule { rule } => {
                let h = g.len();
                let w = g.first().map(|r| r.len()).unwrap_or(0);
                *leaf_ops += h * w * rule.mdl_size();
                let mut out = vec![vec![0i64; w]; h];
                for r in 0..h {
                    let at_bot = if r + 1 == h { 1i64 } else { 0i64 };
                    for c in 0..w {
                        let s = g[r][c];
                        let n = if r > 0 { g[r - 1][c] } else { 0 };
                        let so = if r + 1 < h { g[r + 1][c] } else { 0 };
                        let e = if c + 1 < w { g[r][c + 1] } else { 0 };
                        let w_nb = if c > 0 { g[r][c - 1] } else { 0 };
                        out[r][c] = rule.eval(s, n, so, e, w_nb, at_bot);
                    }
                }
                out
            }
            OmegaExpr::Compose(a, b) => {
                let mid = a.eval_grid(g, resolver, leaf_ops);
                b.eval_grid(&mid, resolver, leaf_ops)
            }
            OmegaExpr::BoundedFix { step, max_steps } => {
                let h = g.len();
                let w = g.first().map(|r| r.len()).unwrap_or(0);
                let cap = (*max_steps).max(h + w + 2);
                let mut cur = g.clone();
                for _ in 0..cap {
                    let nxt = step.eval_grid(&cur, resolver, leaf_ops);
                    if nxt == cur {
                        break;
                    }
                    cur = nxt;
                }
                cur
            }
            OmegaExpr::ConceptCall { id, .. } => {
                if let Some(body) = resolver.get_concept_body(*id) {
                    body.eval_grid(g, resolver, leaf_ops)
                } else {
                    g.clone()
                }
            }
            _ => g.clone(),
        }
    }

    /// Emit executable native Rust (`f64`) code for real `rustc -O` verification (scalars, lists, recursion!).
    pub fn emit_rust_f64<R: ConceptResolver>(&self, resolver: &R) -> String {
        let inlined = self.inline_all(resolver, 8);
        fn emit_list_rust(e: &OmegaExpr, env: &[String], let_depth: usize) -> String {
            match e {
                OmegaExpr::ListInput => "seq.to_vec()".to_string(),
                OmegaExpr::ListMap { fn_body, input } => {
                    let inp = emit_list_rust(input, env, let_depth + 1);
                    let base = vec!["el".to_string(), "x0".to_string(), "0.0f64".to_string()];
                    let body = rec(fn_body, &base, let_depth + 1);
                    format!("({}).into_iter().map(|el| {}).collect::<Vec<f64>>()", inp, body)
                }
                OmegaExpr::ListFilterPos { pred, input } => {
                    let inp = emit_list_rust(input, env, let_depth + 1);
                    let base = vec!["el".to_string(), "x0".to_string(), "0.0f64".to_string()];
                    let pcode = rec(pred, &base, let_depth + 1);
                    format!("({}).into_iter().filter(|&el| ({}) > 0.0f64).collect::<Vec<f64>>()", inp, pcode)
                }
                _ => "Vec::<f64>::new()".to_string(),
            }
        }
        fn rec(e: &OmegaExpr, env: &[String], let_depth: usize) -> String {
            match e {
                OmegaExpr::Arg(i) => env.get(*i).cloned().unwrap_or_else(|| format!("x{}", i)),
                OmegaExpr::Const(c) => format!("({:.8}f64)", c),
                OmegaExpr::Param(k) => format!("p{}", k),
                OmegaExpr::Add(a, b) => format!("({} + {})", rec(a, env, let_depth), rec(b, env, let_depth)),
                OmegaExpr::Sub(a, b) => format!("({} - {})", rec(a, env, let_depth), rec(b, env, let_depth)),
                OmegaExpr::Mul(a, b) => format!("({} * {})", rec(a, env, let_depth), rec(b, env, let_depth)),
                OmegaExpr::DivSafe(a, b) => format!(
                    "({{ let _n = {}; let _d = {}; let _sd = if _d.abs() < 1e-3 {{ if _d >= 0.0 {{ 1e-3 }} else {{ -1e-3 }} }} else {{ _d }}; _n / _sd }})",
                    rec(a, env, let_depth),
                    rec(b, env, let_depth)
                ),
                OmegaExpr::Min(a, b) => format!("({}).min({})", rec(a, env, let_depth), rec(b, env, let_depth)),
                OmegaExpr::Max(a, b) => format!("({}).max({})", rec(a, env, let_depth), rec(b, env, let_depth)),
                OmegaExpr::Abs(a) => format!("({}).abs()", rec(a, env, let_depth)),
                OmegaExpr::SqrtSafe(a) => format!("({}).max(0.0f64).sqrt()", rec(a, env, let_depth)),
                OmegaExpr::Let(bound, body) => {
                    let vname = format!("lv{}", let_depth);
                    let b_code = rec(bound, env, let_depth + 1);
                    let mut next_env = Vec::with_capacity(env.len() + 1);
                    next_env.push(vname.clone());
                    next_env.extend_from_slice(env);
                    let body_code = rec(body, &next_env, let_depth + 1);
                    format!("({{ let {} = {}; {} }})", vname, b_code, body_code)
                }
                OmegaExpr::Rec { base, step, max_fuel } => {
                    let base_c = rec(base, env, let_depth + 1);
                    let step_env = vec!["(k as f64)".to_string(), "acc".to_string(), env.get(1).cloned().unwrap_or_else(|| "0.0f64".to_string())];
                    let step_c = rec(step, &step_env, let_depth + 1);
                    let n_expr = env.first().cloned().unwrap_or_else(|| "x0".to_string());
                    format!(
                        "{{ let _n = ({}.round() as i64).clamp(0, {}i64) as usize; let mut acc: f64 = {}; for k in 1..=_n {{ acc = {}; }} acc }}",
                        n_expr, max_fuel, base_c, step_c
                    )
                }
                OmegaExpr::ListReduce { init, step, input } => {
                    let lst = emit_list_rust(input, env, let_depth + 1);
                    let init_c = rec(init, env, let_depth + 1);
                    let s_env = vec!["el".to_string(), "acc".to_string(), "0.0f64".to_string()];
                    let step_c = rec(step, &s_env, let_depth + 1);
                    format!(
                        "{{ let mut acc: f64 = {}; for el in {} {{ acc = {}; }} acc }}",
                        init_c, lst, step_c
                    )
                }
                OmegaExpr::ListLen(input) => format!("(({}).len() as f64)", emit_list_rust(input, env, let_depth + 1)),
                OmegaExpr::IfPos(c, t, el) => format!(
                    "(if {} > 0.0f64 {{ {} }} else {{ {} }})",
                    rec(c, env, let_depth),
                    rec(t, env, let_depth),
                    rec(el, env, let_depth)
                ),
                OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => {
                    let base = vec!["x0".to_string(), "x1".to_string(), "x2".to_string()];
                    format!(
                        "{{ let mut a: f64 = {}; let mut b: f64 = {}; for &x0 in seq {{ let x1 = a; let x2 = b; a = {}; let x1 = a; let x2 = b; b = {}; }} {} }}",
                        rec(init_a, env, let_depth),
                        rec(init_b, env, let_depth),
                        rec(step_a, &base, let_depth),
                        rec(step_b, &base, let_depth),
                        if *return_reg == 0 { "a" } else { "b" }
                    )
                }
                _ => "0.0f64".to_string(),
            }
        }
        let base_env = vec!["x0".to_string(), "x1".to_string(), "x2".to_string()];
        rec(&inlined, &base_env, 0)
    }

    /// Emit executable native Rust code for synthesized 2D cellular-stencil / coord-map grid programs!
    pub fn emit_rust_grid<R: ConceptResolver>(&self, resolver: &R) -> String {
        let inlined = self.inline_all(resolver, 8);
        fn rec(e: &OmegaExpr, var: &str, depth: usize) -> String {
            match e {
                OmegaExpr::GridCoordMap { r_expr, c_expr } => format!(
                    "{{ let g = &{var}; let h = g.len(); let w = g[0].len(); let mut out = vec![vec![0i64; w]; h]; for r in 0..h {{ for c in 0..w {{ let sr = {re}; let sc = {ce}; if sr >= 0 && sc >= 0 && (sr as usize) < h && (sc as usize) < w {{ out[r][c] = g[sr as usize][sc as usize]; }} }} }} out }}",
                    var = var,
                    re = r_expr.emit_rust(),
                    ce = c_expr.emit_rust()
                ),
                OmegaExpr::GridStencilRule { rule } => format!(
                    "{{ let g = &{var}; let h = g.len(); let w = g[0].len(); let mut out = vec![vec![0i64; w]; h]; for r in 0..h {{ let at_bot = if r + 1 == h {{ 1i64 }} else {{ 0i64 }}; for c in 0..w {{ let s = g[r][c]; let n = if r > 0 {{ g[r-1][c] }} else {{ 0 }}; let so = if r + 1 < h {{ g[r+1][c] }} else {{ 0 }}; let e = if c + 1 < w {{ g[r][c+1] }} else {{ 0 }}; let w_nb = if c > 0 {{ g[r][c-1] }} else {{ 0 }}; out[r][c] = {rl}; }} }} out }}",
                    var = var,
                    rl = rule.emit_rust()
                ),
                OmegaExpr::Compose(a, b) => {
                    let mid = format!("g_mid_{}", depth);
                    format!(
                        "{{ let {mid} = {fa}; {fb} }}",
                        mid = mid,
                        fa = rec(a, var, depth + 1),
                        fb = rec(b, &mid, depth + 1)
                    )
                }
                OmegaExpr::BoundedFix { step, max_steps } => {
                    let cur = format!("g_fix_{}", depth);
                    format!(
                        "{{ let mut {cur} = {var}.clone(); let _cap = ({ms}usize).max({cur}.len() + {cur}[0].len() + 2); for _ in 0.._cap {{ let _nxt = {st}; if _nxt == {cur} {{ break; }} {cur} = _nxt; }} {cur} }}",
                        cur = cur,
                        var = var,
                        ms = max_steps,
                        st = rec(step, &cur, depth + 1)
                    )
                }
                _ => format!("{}.clone()", var),
            }
        }
        rec(&inlined, "grid", 0)
    }

    #[cfg(feature = "lean_oracle")]
    pub fn emit_lean4_int<R: ConceptResolver>(&self, resolver: &R) -> Option<String> {
        let inlined = self.inline_all(resolver, 8);
        fn rec(e: &OmegaExpr) -> Option<String> {
            match e {
                OmegaExpr::Arg(i) => Some(format!("x{}", i)),
                OmegaExpr::Const(c) => {
                    if (c - c.round()).abs() < 1e-6 {
                        Some(format!("({} : Int)", c.round() as i64))
                    } else {
                        None
                    }
                }
                OmegaExpr::Add(a, b) => Some(format!("({} + {})", rec(a)?, rec(b)?)),
                OmegaExpr::Sub(a, b) => Some(format!("({} - {})", rec(a)?, rec(b)?)),
                OmegaExpr::Mul(a, b) => Some(format!("({} * {})", rec(a)?, rec(b)?)),
                OmegaExpr::Min(a, b) => Some(format!("(min {} {})", rec(a)?, rec(b)?)),
                OmegaExpr::Max(a, b) => Some(format!("(max {} {})", rec(a)?, rec(b)?)),
                OmegaExpr::Abs(a) => {
                    let s = rec(a)?;
                    Some(format!("(if {} >= (0 : Int) then {} else (-{}))", s, s, s))
                }
                OmegaExpr::IfPos(c, t, el) => Some(format!("(if {} > (0 : Int) then {} else {})", rec(c)?, rec(t)?, rec(el)?)),
                _ => None,
            }
        }
        rec(&inlined)
    }

    pub fn collect_subtrees(&self, out: &mut Vec<OmegaExpr>) {
        if self.mdl_size() >= 3 {
            out.push(self.clone());
        }
        match self {
            OmegaExpr::Add(a, b)
            | OmegaExpr::Sub(a, b)
            | OmegaExpr::Mul(a, b)
            | OmegaExpr::DivSafe(a, b)
            | OmegaExpr::Min(a, b)
            | OmegaExpr::Max(a, b)
            | OmegaExpr::Let(a, b)
            | OmegaExpr::App(a, b)
            | OmegaExpr::Compose(a, b) => {
                a.collect_subtrees(out);
                b.collect_subtrees(out);
            }
            OmegaExpr::Abs(a) | OmegaExpr::SqrtSafe(a) | OmegaExpr::Lam(a) | OmegaExpr::ListLen(a) => {
                a.collect_subtrees(out)
            }
            OmegaExpr::IfPos(c, t, el) => {
                c.collect_subtrees(out);
                t.collect_subtrees(out);
                el.collect_subtrees(out);
            }
            OmegaExpr::Rec { base, step, .. } => {
                base.collect_subtrees(out);
                step.collect_subtrees(out);
            }
            OmegaExpr::ListMap { fn_body, input } => {
                fn_body.collect_subtrees(out);
                input.collect_subtrees(out);
            }
            OmegaExpr::ListFilterPos { pred, input } => {
                pred.collect_subtrees(out);
                input.collect_subtrees(out);
            }
            OmegaExpr::ListReduce { init, step, input } => {
                init.collect_subtrees(out);
                step.collect_subtrees(out);
                input.collect_subtrees(out);
            }
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, .. } => {
                init_a.collect_subtrees(out);
                init_b.collect_subtrees(out);
                step_a.collect_subtrees(out);
                step_b.collect_subtrees(out);
            }
            OmegaExpr::BoundedFix { step, .. } => step.collect_subtrees(out),
            OmegaExpr::ConceptCall { args, .. } => {
                for a in args {
                    a.collect_subtrees(out);
                }
            }
            _ => {}
        }
    }

    pub fn normalize_as_template(&self) -> (OmegaExpr, usize) {
        let mut map: HashMap<usize, usize> = HashMap::new();
        fn rec(e: &OmegaExpr, map: &mut HashMap<usize, usize>) -> OmegaExpr {
            match e {
                OmegaExpr::Arg(i) => {
                    let next_id = map.len();
                    let mapped = *map.entry(*i).or_insert(next_id);
                    OmegaExpr::Arg(mapped)
                }
                OmegaExpr::Const(c) => OmegaExpr::Const(*c),
                OmegaExpr::Param(k) => OmegaExpr::Param(*k),
                OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::DivSafe(a, b) => OmegaExpr::DivSafe(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(rec(a, map))),
                OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(rec(a, map))),
                OmegaExpr::Let(b, body) => OmegaExpr::Let(Box::new(rec(b, map)), Box::new(rec(body, map))),
                OmegaExpr::IfPos(c, t, el) => OmegaExpr::IfPos(Box::new(rec(c, map)), Box::new(rec(t, map)), Box::new(rec(el, map))),
                OmegaExpr::Compose(a, b) => OmegaExpr::Compose(Box::new(rec(a, map)), Box::new(rec(b, map))),
                OmegaExpr::BoundedFix { step, max_steps } => OmegaExpr::BoundedFix {
                    step: Box::new(rec(step, map)),
                    max_steps: *max_steps,
                },
                other => other.clone(),
            }
        }
        let tmpl = rec(self, &mut map).canonicalize();
        (tmpl, map.len())
    }
}

pub fn grid_exact_match_fraction(a: &Grid2D, b: &Grid2D) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut tot = 0usize;
    let mut eq = 0usize;
    for (ra, rb) in a.iter().zip(b.iter()) {
        if ra.len() != rb.len() {
            return 0.0;
        }
        for (&va, &vb) in ra.iter().zip(rb.iter()) {
            tot += 1;
            if va == vb {
                eq += 1;
            }
        }
    }
    if tot == 0 { 0.0 } else { eq as f64 / tot as f64 }
}

// ============================================================================
// Exact Rational Arithmetic & Exact 3D Pareto Hypervolume (`omega_proof.lean` Omega-4)
// ============================================================================

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactFraction {
    pub num: i128,
    pub den: i128,
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    if a == 0 { 1 } else { a }
}

impl ExactFraction {
    pub fn new(num: i128, den: i128) -> Self {
        assert!(den != 0, "ExactFraction denominator cannot be zero");
        let (n, d) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd_i128(n, d);
        Self { num: n / g, den: d / g }
    }
    pub fn zero() -> Self {
        Self { num: 0, den: 1 }
    }
    pub fn add(self, o: Self) -> Self {
        Self::new(self.num * o.den + o.num * self.den, self.den * o.den)
    }
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.num * o.den - o.num * self.den, self.den * o.den)
    }
    pub fn mul(self, o: Self) -> Self {
        Self::new(self.num * o.num, self.den * o.den)
    }
    pub fn le(self, o: Self) -> bool {
        self.num * o.den <= o.num * self.den
    }
    pub fn lt(self, o: Self) -> bool {
        self.num * o.den < o.num * self.den
    }
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

pub fn exact_pareto_hv_3d(pts: &[[ExactFraction; 3]]) -> ExactFraction {
    let valid: Vec<[ExactFraction; 3]> = pts
        .iter()
        .copied()
        .filter(|p| p[0].num > 0 && p[1].num > 0 && p[2].num > 0)
        .collect();
    if valid.is_empty() {
        return ExactFraction::zero();
    }
    let mut zs: Vec<ExactFraction> = valid.iter().map(|p| p[2]).collect();
    zs.push(ExactFraction::zero());
    zs.sort_by(|a, b| (a.num * b.den).cmp(&(b.num * a.den)));
    zs.dedup();

    let mut hv = ExactFraction::zero();
    for w in zs.windows(2) {
        let z_lo = w[0];
        let z_hi = w[1];
        let dz = z_hi.sub(z_lo);
        if dz.num <= 0 {
            continue;
        }
        let mut active: Vec<[ExactFraction; 2]> = valid
            .iter()
            .filter(|p| z_hi.le(p[2]))
            .map(|p| [p[0], p[1]])
            .collect();
        if active.is_empty() {
            continue;
        }
        active.sort_by(|a, b| (b[0].num * a[0].den).cmp(&(a[0].num * b[0].den)));
        let mut area = ExactFraction::zero();
        let mut max_y = ExactFraction::zero();
        let mut i = 0usize;
        while i < active.len() {
            let x_cur = active[i][0];
            while i < active.len() && active[i][0] == x_cur {
                if max_y.lt(active[i][1]) {
                    max_y = active[i][1];
                }
                i += 1;
            }
            let x_next = if i < active.len() { active[i][0] } else { ExactFraction::zero() };
            let dx = x_cur.sub(x_next);
            if dx.num > 0 && max_y.num > 0 {
                area = area.add(dx.mul(max_y));
            }
        }
        hv = hv.add(area.mul(dz));
    }
    hv
}

// ============================================================================
// Bit-Exact S-Expression Serialization/Deserialization for Full-Brain Checkpoints (`v6.0`)
// ============================================================================

impl CoordExpr {
    pub fn to_sexp(&self) -> String {
        match self {
            CoordExpr::Row => "CR".to_string(),
            CoordExpr::Col => "CC".to_string(),
            CoordExpr::Height => "CH".to_string(),
            CoordExpr::Width => "CW".to_string(),
            CoordExpr::One => "C1".to_string(),
            CoordExpr::Sub(a, b) => format!("(CS {} {})", a.to_sexp(), b.to_sexp()),
        }
    }
}

impl CellExpr {
    pub fn to_sexp(&self) -> String {
        match self {
            CellExpr::SelfVal => "KS".to_string(),
            CellExpr::North => "KN".to_string(),
            CellExpr::South => "KSo".to_string(),
            CellExpr::East => "KE".to_string(),
            CellExpr::West => "KW".to_string(),
            CellExpr::AtBottom => "KB".to_string(),
            CellExpr::Zero => "KZ".to_string(),
            CellExpr::Min(a, b) => format!("(KMn {} {})", a.to_sexp(), b.to_sexp()),
            CellExpr::Max(a, b) => format!("(KMx {} {})", a.to_sexp(), b.to_sexp()),
            CellExpr::IfPos(c, t, e) => format!("(KIf {} {} {})", c.to_sexp(), t.to_sexp(), e.to_sexp()),
        }
    }
}

fn sexp_tokenize(s: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '(' || ch == ')' {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            toks.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks
}

fn parse_coord_sexp(toks: &[String], pos: &mut usize) -> Option<CoordExpr> {
    let t = toks.get(*pos)?.as_str();
    *pos += 1;
    match t {
        "CR" => Some(CoordExpr::Row),
        "CC" => Some(CoordExpr::Col),
        "CH" => Some(CoordExpr::Height),
        "CW" => Some(CoordExpr::Width),
        "C1" => Some(CoordExpr::One),
        "(" => {
            let op = toks.get(*pos)?.as_str();
            *pos += 1;
            let res = if op == "CS" {
                let a = parse_coord_sexp(toks, pos)?;
                let b = parse_coord_sexp(toks, pos)?;
                Some(CoordExpr::Sub(Box::new(a), Box::new(b)))
            } else {
                None
            };
            if toks.get(*pos).map(|s| s.as_str()) == Some(")") {
                *pos += 1;
            }
            res
        }
        _ => None,
    }
}

fn parse_cell_sexp(toks: &[String], pos: &mut usize) -> Option<CellExpr> {
    let t = toks.get(*pos)?.as_str();
    *pos += 1;
    match t {
        "KS" => Some(CellExpr::SelfVal),
        "KN" => Some(CellExpr::North),
        "KSo" => Some(CellExpr::South),
        "KE" => Some(CellExpr::East),
        "KW" => Some(CellExpr::West),
        "KB" => Some(CellExpr::AtBottom),
        "KZ" => Some(CellExpr::Zero),
        "(" => {
            let op = toks.get(*pos)?.as_str();
            *pos += 1;
            let res = match op {
                "KMn" => {
                    let a = parse_cell_sexp(toks, pos)?;
                    let b = parse_cell_sexp(toks, pos)?;
                    Some(CellExpr::Min(Box::new(a), Box::new(b)))
                }
                "KMx" => {
                    let a = parse_cell_sexp(toks, pos)?;
                    let b = parse_cell_sexp(toks, pos)?;
                    Some(CellExpr::Max(Box::new(a), Box::new(b)))
                }
                "KIf" => {
                    let c = parse_cell_sexp(toks, pos)?;
                    let a = parse_cell_sexp(toks, pos)?;
                    let b = parse_cell_sexp(toks, pos)?;
                    Some(CellExpr::IfPos(Box::new(c), Box::new(a), Box::new(b)))
                }
                _ => None,
            };
            if toks.get(*pos).map(|s| s.as_str()) == Some(")") {
                *pos += 1;
            }
            res
        }
        _ => None,
    }
}

fn parse_expr_sexp(toks: &[String], pos: &mut usize) -> Option<OmegaExpr> {
    let t = toks.get(*pos)?.as_str();
    *pos += 1;
    if t == "LI" {
        return Some(OmegaExpr::ListInput);
    }
    if let Some(rest) = t.strip_prefix("A:") {
        return Some(OmegaExpr::Arg(rest.parse().ok()?));
    }
    if let Some(rest) = t.strip_prefix("C:") {
        let bits = u64::from_str_radix(rest, 16).ok()?;
        return Some(OmegaExpr::Const(f64::from_bits(bits)));
    }
    if let Some(rest) = t.strip_prefix("P:") {
        return Some(OmegaExpr::Param(rest.parse().ok()?));
    }
    if t != "(" {
        return None;
    }
    let op = toks.get(*pos)?.as_str();
    *pos += 1;
    let res = match op {
        "Add" => Some(OmegaExpr::Add(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Sub" => Some(OmegaExpr::Sub(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Mul" => Some(OmegaExpr::Mul(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Div" => Some(OmegaExpr::DivSafe(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Min" => Some(OmegaExpr::Min(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Max" => Some(OmegaExpr::Max(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Abs" => Some(OmegaExpr::Abs(Box::new(parse_expr_sexp(toks, pos)?))),
        "Sq" => Some(OmegaExpr::SqrtSafe(Box::new(parse_expr_sexp(toks, pos)?))),
        "Let" => Some(OmegaExpr::Let(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "If" => Some(OmegaExpr::IfPos(
            Box::new(parse_expr_sexp(toks, pos)?),
            Box::new(parse_expr_sexp(toks, pos)?),
            Box::new(parse_expr_sexp(toks, pos)?),
        )),
        "Lam" => Some(OmegaExpr::Lam(Box::new(parse_expr_sexp(toks, pos)?))),
        "App" => Some(OmegaExpr::App(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Rec" => {
            let mf: usize = toks.get(*pos)?.parse().ok()?;
            *pos += 1;
            let b = parse_expr_sexp(toks, pos)?;
            let s = parse_expr_sexp(toks, pos)?;
            Some(OmegaExpr::Rec { base: Box::new(b), step: Box::new(s), max_fuel: mf })
        }
        "LMap" => Some(OmegaExpr::ListMap { fn_body: Box::new(parse_expr_sexp(toks, pos)?), input: Box::new(parse_expr_sexp(toks, pos)?) }),
        "LFlt" => Some(OmegaExpr::ListFilterPos { pred: Box::new(parse_expr_sexp(toks, pos)?), input: Box::new(parse_expr_sexp(toks, pos)?) }),
        "LRed" => Some(OmegaExpr::ListReduce {
            init: Box::new(parse_expr_sexp(toks, pos)?),
            step: Box::new(parse_expr_sexp(toks, pos)?),
            input: Box::new(parse_expr_sexp(toks, pos)?),
        }),
        "LLen" => Some(OmegaExpr::ListLen(Box::new(parse_expr_sexp(toks, pos)?))),
        "Fold" => {
            let reg: u8 = toks.get(*pos)?.parse().ok()?;
            *pos += 1;
            Some(OmegaExpr::FoldSeq {
                init_a: Box::new(parse_expr_sexp(toks, pos)?),
                init_b: Box::new(parse_expr_sexp(toks, pos)?),
                step_a: Box::new(parse_expr_sexp(toks, pos)?),
                step_b: Box::new(parse_expr_sexp(toks, pos)?),
                return_reg: reg,
            })
        }
        "GC" => Some(OmegaExpr::GridCoordMap {
            r_expr: parse_coord_sexp(toks, pos)?,
            c_expr: parse_coord_sexp(toks, pos)?,
        }),
        "GS" => Some(OmegaExpr::GridStencilRule { rule: parse_cell_sexp(toks, pos)? }),
        "Comp" => Some(OmegaExpr::Compose(Box::new(parse_expr_sexp(toks, pos)?), Box::new(parse_expr_sexp(toks, pos)?))),
        "Fix" => {
            let ms: usize = toks.get(*pos)?.parse().ok()?;
            *pos += 1;
            Some(OmegaExpr::BoundedFix { step: Box::new(parse_expr_sexp(toks, pos)?), max_steps: ms })
        }
        "Call" => {
            let id: usize = toks.get(*pos)?.parse().ok()?;
            *pos += 1;
            let mut args = Vec::new();
            while *pos < toks.len() && toks[*pos] != ")" {
                args.push(parse_expr_sexp(toks, pos)?);
            }
            Some(OmegaExpr::ConceptCall { id, args })
        }
        _ => None,
    };
    if toks.get(*pos).map(|s| s.as_str()) == Some(")") {
        *pos += 1;
    }
    res
}

impl OmegaExpr {
    /// Bit-exact S-expression serializer for full-brain persistence (`v6.0`).
    pub fn to_sexp(&self) -> String {
        match self {
            OmegaExpr::Arg(i) => format!("A:{}", i),
            OmegaExpr::Const(v) => format!("C:{:016x}", v.to_bits()),
            OmegaExpr::Param(k) => format!("P:{}", k),
            OmegaExpr::Add(a, b) => format!("(Add {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Sub(a, b) => format!("(Sub {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Mul(a, b) => format!("(Mul {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::DivSafe(a, b) => format!("(Div {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Min(a, b) => format!("(Min {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Max(a, b) => format!("(Max {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Abs(a) => format!("(Abs {})", a.to_sexp()),
            OmegaExpr::SqrtSafe(a) => format!("(Sq {})", a.to_sexp()),
            OmegaExpr::Let(a, b) => format!("(Let {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::IfPos(c, t, e) => format!("(If {} {} {})", c.to_sexp(), t.to_sexp(), e.to_sexp()),
            OmegaExpr::Lam(a) => format!("(Lam {})", a.to_sexp()),
            OmegaExpr::App(a, b) => format!("(App {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::Rec { base, step, max_fuel } => format!("(Rec {} {} {})", max_fuel, base.to_sexp(), step.to_sexp()),
            OmegaExpr::ListInput => "LI".to_string(),
            OmegaExpr::ListMap { fn_body, input } => format!("(LMap {} {})", fn_body.to_sexp(), input.to_sexp()),
            OmegaExpr::ListFilterPos { pred, input } => format!("(LFlt {} {})", pred.to_sexp(), input.to_sexp()),
            OmegaExpr::ListReduce { init, step, input } => format!("(LRed {} {} {})", init.to_sexp(), step.to_sexp(), input.to_sexp()),
            OmegaExpr::ListLen(a) => format!("(LLen {})", a.to_sexp()),
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, return_reg } => format!(
                "(Fold {} {} {} {} {})",
                return_reg, init_a.to_sexp(), init_b.to_sexp(), step_a.to_sexp(), step_b.to_sexp()
            ),
            OmegaExpr::GridCoordMap { r_expr, c_expr } => format!("(GC {} {})", r_expr.to_sexp(), c_expr.to_sexp()),
            OmegaExpr::GridStencilRule { rule } => format!("(GS {})", rule.to_sexp()),
            OmegaExpr::Compose(a, b) => format!("(Comp {} {})", a.to_sexp(), b.to_sexp()),
            OmegaExpr::BoundedFix { step, max_steps } => format!("(Fix {} {})", max_steps, step.to_sexp()),
            OmegaExpr::ConceptCall { id, args } => {
                if args.is_empty() {
                    format!("(Call {})", id)
                } else {
                    let s: Vec<String> = args.iter().map(|a| a.to_sexp()).collect();
                    format!("(Call {} {})", id, s.join(" "))
                }
            }
        }
    }

    pub fn from_sexp(s: &str) -> Option<OmegaExpr> {
        let toks = sexp_tokenize(s);
        let mut pos = 0usize;
        parse_expr_sexp(&toks, &mut pos)
    }
}
