//! Pillar 3: UNIFIED-BRAIN State-Conditioned Neural Policy-Value-Tactic Core (Scalable d=64/256/2048)
//!           + Continuous 9-Shift Optical-Flow Action Lie Algebra World Model + Executable Meta-AST
//!           + Lossless Full-Brain SHA-256 Persistence (`v8.0`).

use std::fs;
use std::path::Path;

use crate::egraph::{Pattern, RewriteRule};
use crate::gpu_bridge::{
    omega_batched_nvfp4_gemm_on_device, omega_relax_particles_on_device,
    omega_upload_contractive_core_all_gpus,
};
#[cfg(any(test, feature = "lean_oracle"))]
use crate::gpu_bridge::omega_tactic_policy_on_device;
use crate::ir::{ConceptResolver, EmptyResolver, Grid2D, OmegaExpr};
use crate::memory::{UnboundedTropicalMemory, VerifiedConcept, OMEGA_DIM};
use crate::nvfp4::nvfp4_quantize;
use crate::sha256::sha256_hex;

pub const RAW_FEAT_DIM: usize = 10;
pub const ENC_MID: usize = OMEGA_DIM;        // 64 (default), 256 (`scale_256`), 2048 (`scale_2048`) — synced 1:1 with SM120 CUDA!
pub const ENC_HIDDEN: usize = OMEGA_DIM / 2; // Mean(d/2) + TropMax(d/2) = d == OMEGA_DIM!
pub const NUM_OP_CLASSES: usize = 12;
pub const NUM_TACTIC_CLASSES: usize = 8; // 0:assumption/exact, 1:omega, 2:simp, 3:apply, 4:induction, 5:constructor, 6:lemma/rw, 7:by_cases

// Pure Coordinate-Free Egocentric 5x5 World Observation (`ZERO` global `(r, c)` coordinates!):
//   25 cells (5x5 centered on agent) x 5 secretly permuted symbols + 1 inventory flag = 126 dims!
pub const WORLD_OBS_DIM: usize = 126;
pub const WORLD_BELIEF_DIM: usize = 16; // Action-driven path-integrating recurrent belief state!
pub const WORLD_CTX_DIM: usize = 8;     // Shared `UniversalEnergyCore` equilibrium latent context!
pub const WORLD_STATE_DIM: usize = WORLD_OBS_DIM + WORLD_BELIEF_DIM + WORLD_CTX_DIM; // 150 dims
pub const WORLD_HIDDEN: usize = 64;

/// Self-modifiable search heuristic genome evolved by TropiCore-Omega's recursive meta-self-improvement loop!
#[derive(Clone, Debug)]
pub struct SearchStrategyGenome {
    pub w_fit: f64,
    pub w_prior_nll: f64,
    pub w_next_op: f64,
    pub w_partner_need: f64,
    pub w_value: f64,
    pub w_align: f64,
    pub w_mdl: f64,
    pub w_depth: f64,
    pub neural_temp: f64, // calibrated mixing temperature (`0..1`) preventing small-sample overconfidence
    pub batch_pop_k: usize,
    pub partner_cap: usize,
}

impl Default for SearchStrategyGenome {
    fn default() -> Self {
        Self {
            w_fit: 1.0,
            w_prior_nll: 0.18,
            w_next_op: 0.22,
            w_partner_need: 0.42,
            w_value: 0.20,
            w_align: 0.04,
            w_mdl: 0.052,
            w_depth: 0.42,
            neural_temp: 0.35,
            batch_pop_k: 2,
            partner_cap: 12,
        }
    }
}

pub fn expr_to_op_mask_resolved<R: ConceptResolver>(expr: &OmegaExpr, resolver: &R) -> [f64; NUM_OP_CLASSES] {
    let inlined = expr.inline_all(resolver, 8);
    expr_to_op_mask(&inlined)
}

pub fn expr_to_op_mask(expr: &OmegaExpr) -> [f64; NUM_OP_CLASSES] {
    let mut mask = [0.0f64; NUM_OP_CLASSES];
    fn walk(e: &OmegaExpr, m: &mut [f64; NUM_OP_CLASSES]) {
        match e {
            OmegaExpr::Add(a, b) => { m[0] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::Sub(a, b) => { m[1] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::Mul(a, b) => { m[2] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::DivSafe(a, b) => { m[3] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::Min(a, b) => { m[4] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::Max(a, b) => { m[5] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::Abs(a) => { m[6] = 1.0; walk(a, m); }
            OmegaExpr::SqrtSafe(a) => { m[7] = 1.0; walk(a, m); }
            OmegaExpr::IfPos(c, t, el) => { m[8] = 1.0; walk(c, m); walk(t, m); walk(el, m); }
            OmegaExpr::FoldSeq { init_a, init_b, step_a, step_b, .. } => {
                m[9] = 1.0;
                walk(init_a, m); walk(init_b, m); walk(step_a, m); walk(step_b, m);
            }
            OmegaExpr::ListInput | OmegaExpr::ListLen(_) => { m[9] = 1.0; }
            OmegaExpr::ListMap { fn_body, input } => { m[9] = 1.0; walk(fn_body, m); walk(input, m); }
            OmegaExpr::ListFilterPos { pred, input } => { m[9] = 1.0; walk(pred, m); walk(input, m); }
            OmegaExpr::ListReduce { init, step, input } => {
                m[9] = 1.0; walk(init, m); walk(step, m); walk(input, m);
            }
            OmegaExpr::GridCoordMap { .. } | OmegaExpr::GridStencilRule { .. } => { m[10] = 1.0; }
            OmegaExpr::Compose(a, b) => { m[10] = 1.0; walk(a, m); walk(b, m); }
            OmegaExpr::BoundedFix { step, .. } => { m[10] = 1.0; walk(step, m); }
            OmegaExpr::Param(_) => { m[11] = 1.0; }
            OmegaExpr::Let(b, body) | OmegaExpr::App(b, body) => { m[11] = 1.0; walk(b, m); walk(body, m); }
            OmegaExpr::Lam(body) => { m[11] = 1.0; walk(body, m); }
            OmegaExpr::Rec { base, step, .. } => { m[11] = 1.0; walk(base, m); walk(step, m); }
            OmegaExpr::ConceptCall { args, .. } => {
                for a in args { walk(a, m); }
            }
            OmegaExpr::Arg(_) | OmegaExpr::Const(_) => {}
        }
    }
    walk(expr, &mut mask);
    mask
}

pub fn expr_to_var_mask(expr: &OmegaExpr) -> [f64; 3] {
    let mut vm = [0.0f64; 3];
    fn walk(e: &OmegaExpr, vm: &mut [f64; 3]) {
        match e {
            OmegaExpr::Arg(i) if *i < 3 => vm[*i] = 1.0,
            OmegaExpr::Add(a, b)
            | OmegaExpr::Sub(a, b)
            | OmegaExpr::Mul(a, b)
            | OmegaExpr::DivSafe(a, b)
            | OmegaExpr::Min(a, b)
            | OmegaExpr::Max(a, b)
            | OmegaExpr::Let(a, b)
            | OmegaExpr::App(a, b) => {
                walk(a, vm);
                walk(b, vm);
            }
            OmegaExpr::Abs(a) | OmegaExpr::SqrtSafe(a) | OmegaExpr::Lam(a) => walk(a, vm),
            OmegaExpr::IfPos(c, t, el) => {
                walk(c, vm);
                walk(t, vm);
                walk(el, vm);
            }
            OmegaExpr::ConceptCall { args, .. } => {
                for a in args {
                    walk(a, vm);
                }
            }
            _ => {}
        }
    }
    walk(expr, &mut vm);
    vm
}

/// Pure raw numeric/residual set tokenization (ZERO derived formulas on `x0, x1, x2`!).
pub fn raw_tokens_numeric_residual(
    pairs: &[([f64; 3], f64)],
    cur_preds: Option<&[f64]>,
) -> Vec<[f64; RAW_FEAT_DIM]> {
    let has_cur = if cur_preds.is_some() { 1.0 } else { 0.0 };
    pairs
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let yh = cur_preds.and_then(|p| p.get(i).copied()).unwrap_or(0.0);
            let res = if cur_preds.is_some() { *y - yh } else { *y };
            let sign_y = if *y > 1e-6 {
                1.0
            } else if *y < -1e-6 {
                -1.0
            } else {
                0.0
            };
            let frac_y = ((*y - y.round()).abs() * 2.0).clamp(0.0, 1.0);
            [
                (x[0] * 0.25).clamp(-2.5, 2.5),
                (x[1] * 0.25).clamp(-2.5, 2.5),
                (x[2] * 0.25).clamp(-2.5, 2.5),
                y.asinh() * 0.35,
                yh.asinh() * 0.35,
                res.asinh() * 0.35,
                sign_y,
                frac_y,
                has_cur,
                0.25,
            ]
        })
        .collect()
}

pub fn raw_tokens_sequence(pairs: &[(Vec<f64>, f64)]) -> Vec<[f64; RAW_FEAT_DIM]> {
    let mut toks = Vec::new();
    for (p_idx, (seq, y)) in pairs.iter().take(10).enumerate() {
        let len = seq.len().max(1) as f64;
        let sign_y = if *y > 1e-6 { 1.0 } else if *y < -1e-6 { -1.0 } else { 0.0 };
        for (t, &xt) in seq.iter().take(16).enumerate() {
            toks.push([
                xt.asinh() * 0.35,
                (t as f64) / len,
                len.ln_1p() * 0.25,
                y.asinh() * 0.35,
                (p_idx as f64) / 10.0,
                0.0,
                sign_y,
                0.0,
                0.0,
                0.50,
            ]);
        }
    }
    toks
}

pub fn raw_tokens_grid(pairs: &[(Grid2D, Grid2D)]) -> Vec<[f64; RAW_FEAT_DIM]> {
    let mut toks = Vec::new();
    for (inp, out) in pairs.iter().take(4) {
        let h = inp.len().max(1);
        let w = inp.first().map(|r| r.len()).unwrap_or(1).max(1);
        let stride_r = (h / 8).max(1);
        let stride_c = (w / 8).max(1);
        let mut r = 0;
        while r < h {
            let mut c = 0;
            while c < w {
                let vi = inp[r][c] as f64;
                let vo = out.get(r).and_then(|row| row.get(c)).copied().unwrap_or(0) as f64;
                toks.push([
                    (r as f64 / h as f64) * 2.0 - 1.0,
                    (c as f64 / w as f64) * 2.0 - 1.0,
                    (vi / 4.0).clamp(-1.0, 1.0),
                    (vo / 4.0).clamp(-1.0, 1.0),
                    ((vo - vi) / 4.0).clamp(-1.0, 1.0),
                    if vi != 0.0 { 1.0 } else { -1.0 },
                    if vo != 0.0 { 1.0 } else { -1.0 },
                    if (vo - vi).abs() > 1e-6 { 1.0 } else { 0.0 },
                    0.0,
                    0.75,
                ]);
                c += stride_c;
            }
            r += stride_r;
        }
    }
    toks
}

/// Pure structural tokenization of a LIVE Lean 4 `trace_state` subgoal into raw `R^10` tokens
/// (ZERO theorem-specific variable names! Feeds straight into `UniversalEnergyCore`'s `W_enc` + `W_core`!).
#[cfg(any(test, feature = "lean_oracle"))]
pub fn raw_tokens_proof_state(
    hyps: &[(String, String)],
    target: &str,
    def_names: &[String],
    depth: usize,
) -> Vec<[f64; RAW_FEAT_DIM]> {
    let mut toks = Vec::with_capacity(hyps.len() + 1);
    let has_def = def_names.iter().any(|d| !d.is_empty() && target.contains(d.as_str()));
    let concrete = target.contains("[]") || target.contains("::") || target.contains(" 0 ") || target.contains("+ 1");
    // Token 0: Target proposition structural observables
    toks.push([
        1.0, // is_target
        if target.contains(" * ") { 1.0 } else { 0.0 },
        if target.contains(" ∧ ") { 1.0 } else { 0.0 },
        if target.contains("≤") || target.contains("<=") { 1.0 } else { 0.0 },
        if target.contains(" = ") { 1.0 } else { 0.0 },
        if has_def { 1.0 } else { 0.0 },
        if concrete { 1.0 } else { 0.0 },
        if target.contains("max") || target.contains("min") || target.contains("if ") { 1.0 } else { 0.0 },
        (depth as f64 / 8.0).clamp(0.0, 1.0),
        1.0, // proof modality tag
    ]);
    // Tokens 1..N: Live hypothesis structural observables
    for (nm, ty) in hyps {
        let exact_match = !nm.contains('✝') && ty.trim() == target.trim();
        toks.push([
            -1.0, // is_hyp
            if ty.contains(" * ") { 1.0 } else { 0.0 },
            if ty.contains('→') || ty.contains('∀') { 1.0 } else { 0.0 },
            if ty.contains("≤") || ty.contains("<=") { 1.0 } else { 0.0 },
            if ty.contains(" = ") { 1.0 } else { 0.0 },
            if ty.trim().starts_with("List ") || ty.trim() == "Nat" { 1.0 } else { 0.0 },
            if ty.trim() == "Int" { 1.0 } else { 0.0 },
            if exact_match { 1.0 } else { 0.0 },
            if ty.contains(" - ") && ty.contains('*') { 1.0 } else { 0.0 },
            1.0,
        ]);
    }
    toks
}

/// Pure structural tokenization of a coordinate-free 5x5 egocentric world patch for shared `UniversalEnergyCore`!
pub fn raw_tokens_world_egocentric(obs: &[f64; WORLD_OBS_DIM]) -> Vec<[f64; RAW_FEAT_DIM]> {
    let inv = obs[125];
    let mut toks = Vec::with_capacity(25);
    for cell_i in 0..25 {
        let dr = ((cell_i / 5) as f64 - 2.0) * 0.5;
        let dc = ((cell_i % 5) as f64 - 2.0) * 0.5;
        let base = cell_i * 5;
        toks.push([
            dr,
            dc,
            obs[base],
            obs[base + 1],
            obs[base + 2],
            obs[base + 3],
            obs[base + 4],
            inv,
            (dr.abs() + dc.abs()) * 0.5,
            -0.5, // egocentric world modality tag
        ]);
    }
    toks
}

#[derive(Clone, Debug)]
pub struct CegisBarrier {
    pub center: [f64; OMEGA_DIM],
    pub strength: f64,
}

#[derive(Clone, Debug)]
pub struct RawTrainSample {
    pub raw_tokens: Vec<[f64; RAW_FEAT_DIM]>,
    pub op_mask: [f64; NUM_OP_CLASSES],
    /// State-conditioned EXACT NEXT TOP-LEVEL OP needed from this execution state (None if distractor)
    pub next_op_target: Option<usize>,
    /// Which variables `x0, x1, x2` the target expression depends on
    pub var_mask: [f64; 3],
    pub value_target: f64,
    pub pos_tokens: Vec<[f64; RAW_FEAT_DIM]>,
    pub neg_tokens: Vec<[f64; RAW_FEAT_DIM]>,
    /// True missing sibling partner tokens (`B(X)`) for training `W_need` partner retrieval!
    pub partner_pos_tokens: Option<Vec<[f64; RAW_FEAT_DIM]>>,
    pub partner_neg_tokens: Option<Vec<[f64; RAW_FEAT_DIM]>>,
}

#[derive(Clone, Debug)]
struct EncoderForwardCache {
    u_kept: Vec<[f64; RAW_FEAT_DIM]>,
    a_kept: Vec<[f64; ENC_MID]>,
    h_kept: Vec<[f64; ENC_HIDDEN]>,
    argmax_idx: [usize; ENC_HIDDEN],
    z0: [f64; OMEGA_DIM],
}

// ============================================================================
// ZERO-RULE Recurrent-Belief Nonlinear World Model + TD(lambda) + Latent MCTS (`v5.1`)
// ============================================================================

#[derive(Clone, Debug)]
pub struct WorldTransition {
    pub s: [f64; WORLD_STATE_DIM],
    pub prev_belief: [f64; WORLD_BELIEF_DIM],
    pub raw_obs: [f64; WORLD_OBS_DIM],
    pub a: usize,
    pub r: f64,
    pub s_next: [f64; WORLD_STATE_DIM],
    pub done: bool,
    pub ret_3step: f64, // low-variance 3-step discounted return (no long-horizon exploration blame!)
}

const WORLD_DYN_H: usize = 32; // per-action dynamics hidden width (4 independent action heads!)
const WORLD_REW_H: usize = 24; // per-action reward/terminal hidden width

/// 100% ZERO-RULE, COORDINATE-FREE Egocentric 5x5 Action-Conditioned Recurrent-Belief World Model (`v6.0`):
/// Receives ONLY a 5x5 egocentric anonymized symbol patch (`25x5 = 125`) + `inv_flag` (`1`) = `R^126`
/// (ZERO global `(r, c)` coordinates! Works identically on `6x6` training AND `10x10` zero-shot OOD mazes!),
/// augmented with:
///   - Action-driven path-integrating recurrent belief `b_t ∈ R^16` (trained via 1-step BPTT!)
///   - Shared `UniversalEnergyCore` equilibrium latent context `ctx_t ∈ R^8`
/// Plans via exact 2-step all-branch (`4x4=16`) + 3rd-step latent tree search grounded by
/// pure egocentric observation/belief episodic transition memory & anti-cycle revisit tracking!
#[derive(Clone, Debug)]
pub struct NonlinearWorldModel {
    // Learned action-driven path-integrating recurrent belief (`16 x 126` + `16 x 4` action weights)
    // + SELF-LEARNED 2D Action-Displacement Lie Algebra (`act_disp[a] = [dr, dc]` initialized to ZERO!)
    pub w_belief: Vec<f64>,
    pub w_belief_act: Vec<f64>,
    pub act_disp: [[f64; 2]; 4],

    // Per-Action Dynamics MLPs (`4` x `obs_dim -> WORLD_DYN_H -> obs_dim`)
    pub w_d1: Vec<Vec<f64>>,
    pub b_d1: Vec<Vec<f64>>,
    pub w_ds: Vec<Vec<f64>>,
    pub b_ds: Vec<Vec<f64>>,

    // Per-Action Reward & Terminal MLPs (`4` x `obs_dim -> WORLD_REW_H -> 2`)
    pub w_r1: Vec<Vec<f64>>,
    pub b_r1: Vec<Vec<f64>>,
    pub w_r: Vec<Vec<f64>>,
    pub b_r: [f64; 4],
    pub w_term: Vec<Vec<f64>>,
    pub b_term: [f64; 4],

    // Dedicated Recurrent-Belief + Shared-Core Context TD-Q MLP (`WORLD_STATE_DIM -> WORLD_HIDDEN -> 4 actions`)
    // + Polyak-EMA Target Q-Network for non-divergent Bellman bootstrapping!
    pub w_q1: Vec<f64>,
    pub b_q1: Vec<f64>,
    pub w_q2: Vec<f64>,
    pub b_q2: [f64; 4],
    pub w_q1_tgt: Vec<f64>,
    pub b_q1_tgt: Vec<f64>,
    pub w_q2_tgt: Vec<f64>,
    pub b_q2_tgt: [f64; 4],

    pub replay: Vec<WorldTransition>,
    pub pos_replay: Vec<WorldTransition>,
    pub neg_replay: Vec<WorldTransition>,
    pub updates: usize,
    pub last_dyn_mse: f64,
    pub last_rew_mae: f64,
}

impl NonlinearWorldModel {
    pub fn new(seed: u64) -> Self {
        let mut rng = seed ^ 0xB10C_9999_5555_1111;
        let s_obs = (1.0 / WORLD_OBS_DIM as f64).sqrt() * 0.7;
        let s_dh = (1.0 / WORLD_DYN_H as f64).sqrt() * 0.3;
        let s_rh = (1.0 / WORLD_REW_H as f64).sqrt() * 0.3;
        let s_h = (1.0 / WORLD_HIDDEN as f64).sqrt() * 0.3;
        let s_q1 = (1.0 / WORLD_STATE_DIM as f64).sqrt() * 0.7;

        let mut make_v = |n: usize, sc: f64| -> Vec<f64> {
            (0..n).map(|_| splitmix_f64(&mut rng) * sc).collect()
        };

        let w_d1 = (0..4).map(|_| make_v(WORLD_DYN_H * WORLD_OBS_DIM, s_obs)).collect();
        let b_d1 = (0..4).map(|_| vec![0.0; WORLD_DYN_H]).collect();
        let w_ds = (0..4).map(|_| make_v(WORLD_OBS_DIM * WORLD_DYN_H, s_dh)).collect();
        let b_ds = (0..4).map(|_| vec![0.0; WORLD_OBS_DIM]).collect();

        let w_r1 = (0..4).map(|_| make_v(WORLD_REW_H * WORLD_OBS_DIM, s_obs)).collect();
        let b_r1 = (0..4).map(|_| vec![0.0; WORLD_REW_H]).collect();
        let w_r = (0..4).map(|_| make_v(WORLD_REW_H, s_rh)).collect();
        let w_term = (0..4).map(|_| make_v(WORLD_REW_H, s_rh)).collect();

        let w_q1 = make_v(WORLD_HIDDEN * WORLD_STATE_DIM, s_q1);
        let b_q1 = vec![0.0; WORLD_HIDDEN];
        let w_q2 = make_v(4 * WORLD_HIDDEN, s_h);
        let b_q2 = [0.0; 4];

        Self {
            w_belief: make_v(WORLD_BELIEF_DIM * WORLD_OBS_DIM, s_obs * 1.2),
            w_belief_act: make_v(WORLD_BELIEF_DIM * 4, 0.55),
            act_disp: [[0.0; 2]; 4], // ZERO prior on action semantics — self-discovered from optical flow!
            w_d1,
            b_d1,
            w_ds,
            b_ds,
            w_r1,
            b_r1,
            w_r,
            b_r: [0.0; 4],
            w_term,
            b_term: [-1.8; 4],
            w_q1_tgt: w_q1.clone(),
            b_q1_tgt: b_q1.clone(),
            w_q2_tgt: w_q2.clone(),
            b_q2_tgt: b_q2,
            w_q1,
            b_q1,
            w_q2,
            b_q2,
            replay: Vec::with_capacity(4096),
            pos_replay: Vec::with_capacity(2000),
            neg_replay: Vec::with_capacity(1024),
            updates: 0,
            last_dyn_mse: 1.0,
            last_rew_mae: 1.0,
        }
    }

    /// Encode PURE COORDINATE-FREE 5x5 egocentric patch (secretly permuted symbols!) + `inv_flag` into `R^126`.
    /// ZERO global `(r, c)` coordinates! 100% translation-invariant & grid-size-invariant (`6x6 -> 10x10`)!
    pub fn encode_egocentric_5x5(patch_5x5_sym: &[usize; 25], inv_flag: bool) -> [f64; WORLD_OBS_DIM] {
        let mut s = [0.0f64; WORLD_OBS_DIM];
        for cell_i in 0..25 {
            let sym = patch_5x5_sym[cell_i] % 5;
            s[cell_i * 5 + sym] = 1.0;
        }
        s[125] = if inv_flag { 1.0 } else { -1.0 };
        s
    }

    /// Dimensionless mean learned action displacement magnitude (adapts automatically to arbitrary
    /// step sizes and stochastic slip rates — ZERO hardcoded `0.09` magic constant!).
    pub fn mean_action_step_norm(&self) -> f64 {
        let mut sum = 0.0;
        let mut cnt = 0.0;
        for a in 0..4 {
            let n = (self.act_disp[a][0] * self.act_disp[a][0] + self.act_disp[a][1] * self.act_disp[a][1]).sqrt();
            if n > 0.15 {
                sum += n;
                cnt += 1.0;
            }
        }
        if cnt > 0.0 { sum / cnt } else { 1.0 }
    }

    /// GENERAL CONTINUOUS 9-SHIFT SUB-PIXEL OPTICAL-FLOW ACTION-GROUP DISCOVERY (`v8.0`):
    /// Evaluates normalized cross-correlation across ALL 9 shifts `(-1..=1)^2` (cardinals + diagonals + zero;
    /// ZERO 4-cardinal prior and ZERO hardcoded `0.09` scale!) and updates `act_disp[a]` via softmax-weighted
    /// continuous optical-flow centroid estimation (robust under `4!x5!` double permutation AND stochastic slip!).
    pub fn learn_action_lie_algebra(
        &mut self,
        obs_cur: &[f64; WORLD_OBS_DIM],
        action: usize,
        obs_next: &[f64; WORLD_OBS_DIM],
    ) {
        if obs_cur == obs_next {
            return; // pure sensory invariance (stall) -> zero displacement evidence
        }
        let a = action % 4;
        let inv_changed = (obs_cur[125] - obs_next[125]).abs() > 0.5;
        let sym_at = |o: &[f64; WORLD_OBS_DIM], r: usize, c: usize| -> usize {
            let base = (r * 5 + c) * 5;
            for k in 0..5 {
                if o[base + k] > 0.5 {
                    return k;
                }
            }
            0
        };
        let mut rhos: [(isize, isize, f64); 9] = [(0, 0, 0.0); 9];
        let mut idx = 0usize;
        let mut max_rho = -1.0f64;
        let mut second_rho = -1.0f64;
        let mut best_sh = (0isize, 0isize);
        for dr in -1isize..=1isize {
            for dc in -1isize..=1isize {
                let mut matches = 0usize;
                let mut overlap = 0usize;
                for r in 0..5isize {
                    for c in 0..5isize {
                        let sr = r + dr;
                        let sc = c + dc;
                        if sr >= 0 && sr < 5 && sc >= 0 && sc < 5 {
                            if inv_changed && r == 2 && c == 2 {
                                continue;
                            }
                            overlap += 1;
                            if sym_at(obs_cur, sr as usize, sc as usize) == sym_at(obs_next, r as usize, c as usize) {
                                matches += 1;
                            }
                        }
                    }
                }
                let rho = if overlap > 0 { matches as f64 / overlap as f64 } else { 0.0 };
                rhos[idx] = (dr, dc, rho);
                idx += 1;
                if rho > max_rho {
                    second_rho = max_rho;
                    max_rho = rho;
                    best_sh = (dr, dc);
                } else if rho > second_rho {
                    second_rho = rho;
                }
            }
        }
        // Only update when continuous optical-flow peak is strictly identifiable & non-zero!
        if best_sh != (0, 0) && max_rho > 0.72 && (max_rho - second_rho) > 0.08 {
            let mut w_sum = 0.0;
            let mut flow_r = 0.0;
            let mut flow_c = 0.0;
            for &(dr, dc, rho) in &rhos {
                let w = (18.0 * (rho - max_rho)).exp();
                w_sum += w;
                flow_r += w * (dr as f64);
                flow_c += w * (dc as f64);
            }
            flow_r /= w_sum.max(1e-9);
            flow_c /= w_sum.max(1e-9);
            let lr_lie = 0.12;
            self.act_disp[a][0] = (1.0 - lr_lie) * self.act_disp[a][0] + lr_lie * flow_r;
            self.act_disp[a][1] = (1.0 - lr_lie) * self.act_disp[a][1] + lr_lie * flow_c;
        }
    }

    /// Update learned recurrent belief (`b[0..12]`) + SELF-LEARNED Continuous Action-Algebra path integrator (`b[12..16]`)
    /// (ZERO coordinates from env, ZERO hardcoded action directions, ZERO `0.09` magic scale!)
    /// and fuse `[obs_t (126); b_t (16); core_ctx (8)] in R^150`!
    pub fn update_belief_state(
        &self,
        prev_belief: &[f64; WORLD_BELIEF_DIM],
        obs: &[f64; WORLD_OBS_DIM],
        last_action: Option<usize>,
        core_ctx: &[f64; WORLD_CTX_DIM],
    ) -> ([f64; WORLD_BELIEF_DIM], [f64; WORLD_STATE_DIM]) {
        let mut b = [0.0f64; WORLD_BELIEF_DIM];
        for r in 0..12 {
            let base = r * WORLD_OBS_DIM;
            let mut acc = 0.0;
            for c in 0..WORLD_OBS_DIM {
                if obs[c] != 0.0 {
                    acc += self.w_belief[base + c] * obs[c];
                }
            }
            if let Some(la) = last_action {
                acc += self.w_belief_act[r * 4 + (la % 4)];
            }
            b[r] = 0.84 * prev_belief[r] + 0.16 * acc.tanh();
        }
        // Self-Learned Continuous Action-Displacement Lie-Algebra Integrator (natural optical-flow units!)
        let step_n = self.mean_action_step_norm().max(0.15);
        b[12] = prev_belief[12];
        b[13] = prev_belief[13];
        b[14] = 0.70 * prev_belief[14];
        b[15] = 0.70 * prev_belief[15];
        if let Some(la) = last_action {
            let d = self.act_disp[la % 4];
            b[12] = (b[12] + d[0]).clamp(-12.0, 12.0);
            b[13] = (b[13] + d[1]).clamp(-12.0, 12.0);
            b[14] = (b[14] + 0.30 * (d[0] / step_n)).clamp(-1.0, 1.0);
            b[15] = (b[15] + 0.30 * (d[1] / step_n)).clamp(-1.0, 1.0);
        }
        let mut st = [0.0f64; WORLD_STATE_DIM];
        st[..WORLD_OBS_DIM].copy_from_slice(obs);
        st[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM].copy_from_slice(&b);
        st[WORLD_OBS_DIM + WORLD_BELIEF_DIM..].copy_from_slice(core_ctx);
        (b, st)
    }

    /// Mean L1 distance over the observable inner 3x3 patch (9 cells always inside previous 5x5 view -> zero off-screen noise!).
    fn inner_3x3_l1_dist(a: &[f64], b: &[f64]) -> f64 {
        let mut s = 0.0f64;
        for dr in 1..=3 {
            for dc in 1..=3 {
                let base = (dr * 5 + dc) * 5;
                for k in 0..5 {
                    s += (a[base + k] - b[base + k]).abs();
                }
            }
        }
        s / 18.0
    }

    /// Pure internal path-integrator L_inf displacement between two belief states (ZERO env coords!).
    fn path_disp_dist(a: &[f64], b: &[f64]) -> f64 {
        (a[12] - b[12]).abs().max((a[13] - b[13]).abs())
    }

    /// Radial foveal weight for cell `cell_i in 0..25` (1.0 on inner 3x3, 0.55 on outer ring -> low size-OOD variance).
    #[inline]
    fn foveal_weight(c: usize) -> f64 {
        if c >= 125 {
            return 1.0;
        }
        let cell = c / 5;
        let r = cell / 5;
        let col = cell % 5;
        if r >= 1 && r <= 3 && col >= 1 && col <= 3 { 1.0 } else { 0.55 }
    }

    /// Whether obs channel `c` lies inside the inner 3x3 immediate neighborhood (or inv_flag).
    #[inline]
    fn is_inner_3x3_channel(c: usize) -> bool {
        if c == 125 {
            return true;
        }
        let cell = c / 5;
        let r = cell / 5;
        let col = cell % 5;
        r >= 1 && r <= 3 && col >= 1 && col <= 3
    }

    /// Forward action-conditioned decoupled dynamics `f_{theta,a}(obs_t) -> obs_{t+1}` + exact belief propagation
    /// + action-conditioned reward/terminal `(r_{theta,a}, p_{term,a})`.
    pub fn imagine_step(
        &self,
        s: &[f64; WORLD_STATE_DIM],
        action: usize,
    ) -> ([f64; WORLD_STATE_DIM], f64, f64, [f64; WORLD_DYN_H], [f64; WORLD_REW_H]) {
        let a = action.min(3);

        // 1. Per-Action Dedicated Dynamics MLP (`ZERO` cross-action interference!)
        let wd1 = &self.w_d1[a];
        let bd1 = &self.b_d1[a];
        let wds = &self.w_ds[a];
        let bds = &self.b_ds[a];
        let mut h_d = [0.0f64; WORLD_DYN_H];
        for r in 0..WORLD_DYN_H {
            let base = r * WORLD_OBS_DIM;
            let mut acc = bd1[r];
            for c in 0..WORLD_OBS_DIM {
                if s[c] != 0.0 {
                    acc += wd1[base + c] * s[c];
                }
            }
            h_d[r] = acc.tanh();
        }
        let mut obs_next = [0.0f64; WORLD_OBS_DIM];
        for d in 0..WORLD_OBS_DIM {
            let base = d * WORLD_DYN_H;
            let mut delta = bds[d];
            for r in 0..WORLD_DYN_H {
                delta += wds[base + r] * h_d[r];
            }
            obs_next[d] = (s[d] + delta).clamp(-1.05, 1.05);
        }

        // 2. Per-Action Dedicated Observable-Phase-Gated Reward & Terminal MLP (reads inner 3x3 + inv_flag -> ZERO 6x6->10x10 shift!)
        let wr1 = &self.w_r1[a];
        let br1 = &self.b_r1[a];
        let wr = &self.w_r[a];
        let wt = &self.w_term[a];
        let phase_hi = s[125] >= 0.0;
        let r_half = WORLD_REW_H / 2;
        let mut h_r = [0.0f64; WORLD_REW_H];
        for r in 0..WORLD_REW_H {
            if (r < r_half) == phase_hi {
                continue;
            }
            let base = r * WORLD_OBS_DIM;
            let mut acc = br1[r];
            for c in 0..WORLD_OBS_DIM {
                if s[c] != 0.0 && Self::is_inner_3x3_channel(c) {
                    acc += wr1[base + c] * s[c];
                }
            }
            h_r[r] = acc.tanh();
        }
        let mut r_pred = self.b_r[a];
        let mut t_logit = self.b_term[a];
        for r in 0..WORLD_REW_H {
            if h_r[r] != 0.0 {
                r_pred += wr[r] * h_r[r];
                t_logit += wt[r] * h_r[r];
            }
        }
        let p_term = 1.0 / (1.0 + (-t_logit.clamp(-10.0, 10.0)).exp());
        let r_clamped = r_pred.clamp(-2.0, 2.0);

        // PURE sensory-prediction stall check (`ZERO` hardcoded reward threshold `-0.036`!)
        let mut prev_b = [0.0f64; WORLD_BELIEF_DIM];
        prev_b.copy_from_slice(&s[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM]);
        let mut ctx = [0.0f64; WORLD_CTX_DIM];
        ctx.copy_from_slice(&s[WORLD_OBS_DIM + WORLD_BELIEF_DIM..]);
        let stalled_pred = Self::inner_3x3_l1_dist(&obs_next, &s[..WORLD_OBS_DIM]) < 0.085;
        let eff_act = if stalled_pred { None } else { Some(a) };
        let (_, s_next) = self.update_belief_state(&prev_b, &obs_next, eff_act, &ctx);

        (s_next, r_clamped, p_term, h_d, h_r)
    }

    /// Grounded imagination: if `(phase, inner_3x3, path_disp, a)` was ALREADY experienced in THIS episode (`ep_mem`),
    /// recall the exact experienced episodic transition (ZERO wall-clipping hallucination!);
    /// otherwise imagine via the learned action-conditioned neural world model `f_{theta,a}`!
    fn imagine_grounded(
        &self,
        s: &[f64; WORLD_STATE_DIM],
        action: usize,
        ep_mem: &[([f64; WORLD_OBS_DIM], [f64; WORLD_BELIEF_DIM], usize, [f64; WORLD_OBS_DIM], f64, bool)],
    ) -> ([f64; WORLD_STATE_DIM], f64, f64) {
        let a = action.min(3);
        let b_cur = &s[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM];
        let step_n = self.mean_action_step_norm().max(0.15);
        for (o_prev, b_prev, act, o_nx, r_real, d_real) in ep_mem.iter().rev() {
            if *act == a
                && (s[125] - o_prev[125]).abs() < 0.5
                && Self::path_disp_dist(b_cur, b_prev) < 0.55 * step_n
                && Self::inner_3x3_l1_dist(&s[..WORLD_OBS_DIM], o_prev) < 0.08
            {
                let mut pb = [0.0f64; WORLD_BELIEF_DIM];
                pb.copy_from_slice(b_cur);
                let mut ctx = [0.0f64; WORLD_CTX_DIM];
                ctx.copy_from_slice(&s[WORLD_OBS_DIM + WORLD_BELIEF_DIM..]);
                let stalled = *o_nx == *o_prev;
                let (_, s_nx) = self.update_belief_state(&pb, o_nx, if stalled { None } else { Some(a) }, &ctx);
                return (s_nx, *r_real, if *d_real { 1.0 } else { 0.0 });
            }
        }
        let (sn, rp, tp, _, _) = self.imagine_step(s, a);
        (sn, rp, tp)
    }

    pub fn predict_q(&self, s: &[f64; WORLD_STATE_DIM]) -> ([f64; 4], [f64; WORLD_HIDDEN]) {
        let phase_hi = s[125] >= 0.0;
        let q_half = WORLD_HIDDEN / 2;
        let mut h = [0.0f64; WORLD_HIDDEN];
        for r in 0..WORLD_HIDDEN {
            if (r < q_half) == phase_hi {
                continue;
            }
            let base = r * WORLD_STATE_DIM;
            let mut acc = self.b_q1[r];
            for c in 0..WORLD_STATE_DIM {
                // Exclude displacement slots `b[12..16]` from Q so Q is 100% translation & size-OOD invariant!
                if c >= WORLD_OBS_DIM + 12 && c < WORLD_OBS_DIM + WORLD_BELIEF_DIM {
                    continue;
                }
                if s[c] != 0.0 {
                    let w_in = if c < WORLD_OBS_DIM { Self::foveal_weight(c) } else { 1.0 };
                    acc += self.w_q1[base + c] * (s[c] * w_in);
                }
            }
            h[r] = acc.tanh();
        }
        let mut q = [0.0f64; 4];
        for a in 0..4 {
            let base = a * WORLD_HIDDEN;
            let mut acc = self.b_q2[a];
            for r in 0..WORLD_HIDDEN {
                if h[r] != 0.0 {
                    acc += self.w_q2[base + r] * h[r];
                }
            }
            q[a] = acc.clamp(-5.0, 5.0);
        }
        (q, h)
    }

    fn predict_q_target(&self, s: &[f64; WORLD_STATE_DIM]) -> [f64; 4] {
        let phase_hi = s[125] >= 0.0;
        let q_half = WORLD_HIDDEN / 2;
        let mut h = [0.0f64; WORLD_HIDDEN];
        for r in 0..WORLD_HIDDEN {
            if (r < q_half) == phase_hi {
                continue;
            }
            let base = r * WORLD_STATE_DIM;
            let mut acc = self.b_q1_tgt[r];
            for c in 0..WORLD_STATE_DIM {
                if c >= WORLD_OBS_DIM + 12 && c < WORLD_OBS_DIM + WORLD_BELIEF_DIM {
                    continue;
                }
                if s[c] != 0.0 {
                    let w_in = if c < WORLD_OBS_DIM { Self::foveal_weight(c) } else { 1.0 };
                    acc += self.w_q1_tgt[base + c] * (s[c] * w_in);
                }
            }
            h[r] = acc.tanh();
        }
        let mut q = [0.0f64; 4];
        for a in 0..4 {
            let base = a * WORLD_HIDDEN;
            let mut acc = self.b_q2_tgt[a];
            for r in 0..WORLD_HIDDEN {
                if h[r] != 0.0 {
                    acc += self.w_q2_tgt[base + r] * h[r];
                }
            }
            q[a] = acc.clamp(-5.0, 5.0);
        }
        q
    }

    fn train_single_transition(&mut self, sample: &WorldTransition, use_nstep: bool) -> (f64, f64) {
        let gamma = 0.94;
        // Strictly within `eta * ||h||^2 < 0.45 << 2.0` linear stability region!
        let lr_dyn = 0.045;
        let lr_rew = 0.018;
        let lr_q = 0.011;
        let lr_b = 0.004;
        let wd = 2e-4;
        let a = sample.a.min(3);
        let phase_hi = sample.s[125] >= 0.0;
        let r_half = WORLD_REW_H / 2;
        let q_half = WORLD_HIDDEN / 2;

        let (s_p, r_p, t_p, h_d, h_r) = self.imagine_step(&sample.s, a);

        // 1. Per-Action Dedicated Dynamics Update
        let mut dyn_mse = 0.0;
        let mut dh_d = [0.0f64; WORLD_DYN_H];
        {
            let bds = &mut self.b_ds[a];
            let wds = &mut self.w_ds[a];
            for d in 0..WORLD_OBS_DIM {
                let ds_err = (s_p[d] - sample.s_next[d]).clamp(-2.0, 2.0);
                dyn_mse += ds_err * ds_err;
                let g_d = ds_err * 0.35;
                bds[d] -= lr_dyn * g_d;
                let base = d * WORLD_DYN_H;
                for r in 0..WORLD_DYN_H {
                    dh_d[r] += g_d * wds[base + r];
                    wds[base + r] = (wds[base + r] * (1.0 - wd) - lr_dyn * g_d * h_d[r]).clamp(-4.0, 4.0);
                }
            }
            let bd1 = &mut self.b_d1[a];
            let wd1 = &mut self.w_d1[a];
            for r in 0..WORLD_DYN_H {
                let dpre = (dh_d[r] * (1.0 - h_d[r] * h_d[r])).clamp(-2.0, 2.0);
                bd1[r] -= lr_dyn * dpre;
                let base = r * WORLD_OBS_DIM;
                for c in 0..WORLD_OBS_DIM {
                    if sample.s[c] != 0.0 {
                        wd1[base + c] = (wd1[base + c] * (1.0 - wd) - lr_dyn * dpre * sample.s[c]).clamp(-4.0, 4.0);
                    }
                }
            }
        }

        // 2. Per-Action Dedicated Observable-Phase-Gated Reward & Terminal Update
        let salience = if sample.r.abs() > 0.2 || sample.done { 2.2 } else { 1.0 };
        let r_err = (r_p - sample.r).clamp(-2.0, 2.0);
        let t_tgt = if sample.done { 1.0 } else { 0.0 };
        let t_err = t_p - t_tgt;
        let gr = salience * r_err;
        let gt = salience * 0.75 * t_err;
        self.b_r[a] -= lr_rew * gr;
        self.b_term[a] -= lr_rew * gt;
        let mut dh_r = [0.0f64; WORLD_REW_H];
        {
            let wr = &mut self.w_r[a];
            let wt = &mut self.w_term[a];
            for r in 0..WORLD_REW_H {
                if (r < r_half) == phase_hi {
                    continue;
                }
                dh_r[r] = gr * wr[r] + gt * wt[r];
                wr[r] = (wr[r] * (1.0 - wd) - lr_rew * gr * h_r[r]).clamp(-4.0, 4.0);
                wt[r] = (wt[r] * (1.0 - wd) - lr_rew * gt * h_r[r]).clamp(-4.0, 4.0);
            }
            let br1 = &mut self.b_r1[a];
            let wr1 = &mut self.w_r1[a];
            for r in 0..WORLD_REW_H {
                if (r < r_half) == phase_hi {
                    continue;
                }
                let dpre = (dh_r[r] * (1.0 - h_r[r] * h_r[r])).clamp(-2.0, 2.0);
                br1[r] -= lr_rew * dpre;
                let base = r * WORLD_OBS_DIM;
                for c in 0..WORLD_OBS_DIM {
                    if sample.s[c] != 0.0 && Self::is_inner_3x3_channel(c) {
                        wr1[base + c] = (wr1[base + c] * (1.0 - wd) - lr_rew * dpre * sample.s[c]).clamp(-4.0, 4.0);
                    }
                }
            }
        }

        // 3. Dedicated Observable-Phase-Gated Double-Q Update using Polyak-EMA Target Network!
        let (q_cur, h_q) = self.predict_q(&sample.s);
        let max_next_q = if sample.done {
            0.0
        } else {
            let (q_online_n, _) = self.predict_q(&sample.s_next);
            let mut best_an = 0usize;
            for an in 1..4 {
                if q_online_n[an] > q_online_n[best_an] {
                    best_an = an;
                }
            }
            self.predict_q_target(&sample.s_next)[best_an]
        };
        let td0_tgt = sample.r + gamma * max_next_q;
        let tgt = if use_nstep {
            0.55 * td0_tgt + 0.45 * sample.ret_3step
        } else {
            td0_tgt
        };
        let w_td = if sample.r > 0.25 { 1.7 } else if sample.r < -0.25 { 1.4 } else { 1.0 };
        let td_err = (q_cur[a] - tgt).clamp(-2.0, 2.0) * w_td;

        self.b_q2[a] -= lr_q * td_err;
        let q2_base = a * WORLD_HIDDEN;
        let mut dhq = [0.0f64; WORLD_HIDDEN];
        for r in 0..WORLD_HIDDEN {
            if (r < q_half) == phase_hi {
                continue;
            }
            dhq[r] = td_err * self.w_q2[q2_base + r];
            self.w_q2[q2_base + r] = (self.w_q2[q2_base + r] * (1.0 - wd) - lr_q * td_err * h_q[r]).clamp(-4.0, 4.0);
        }
        let mut db_grad = [0.0f64; WORLD_BELIEF_DIM];
        for r in 0..WORLD_HIDDEN {
            if (r < q_half) == phase_hi {
                continue;
            }
            let dpre = (dhq[r] * (1.0 - h_q[r] * h_q[r])).clamp(-2.0, 2.0);
            self.b_q1[r] -= lr_q * dpre;
            let base = r * WORLD_STATE_DIM;
            for c in 0..WORLD_STATE_DIM {
                if c >= WORLD_OBS_DIM + 12 && c < WORLD_OBS_DIM + WORLD_BELIEF_DIM {
                    continue;
                }
                if c >= WORLD_OBS_DIM && c < WORLD_OBS_DIM + 12 {
                    db_grad[c - WORLD_OBS_DIM] += dpre * self.w_q1[base + c];
                }
                if sample.s[c] != 0.0 {
                    let w_in = if c < WORLD_OBS_DIM { Self::foveal_weight(c) } else { 1.0 };
                    self.w_q1[base + c] = (self.w_q1[base + c] * (1.0 - wd) - lr_q * dpre * (sample.s[c] * w_in)).clamp(-4.0, 4.0);
                }
            }
        }

        // Polyak-EMA update of Target Q-Network (`tau = 0.02`)
        let tau = 0.02;
        for i in 0..self.w_q1.len() {
            self.w_q1_tgt[i] += tau * (self.w_q1[i] - self.w_q1_tgt[i]);
        }
        for i in 0..self.b_q1.len() {
            self.b_q1_tgt[i] += tau * (self.b_q1[i] - self.b_q1_tgt[i]);
        }
        for i in 0..self.w_q2.len() {
            self.w_q2_tgt[i] += tau * (self.w_q2[i] - self.w_q2_tgt[i]);
        }
        for i in 0..4 {
            self.b_q2_tgt[i] += tau * (self.b_q2[i] - self.b_q2_tgt[i]);
        }

        // 4. 1-Step Truncated BPTT into `W_belief` from Q-value credit signal!
        for r in 0..WORLD_BELIEF_DIM {
            let tb = ((sample.s[WORLD_OBS_DIM + r] - 0.84 * sample.prev_belief[r]) / 0.16).clamp(-0.99, 0.99);
            let g_pre = (db_grad[r] * 0.16 * (1.0 - tb * tb)).clamp(-1.0, 1.0);
            let base = r * WORLD_OBS_DIM;
            for c in 0..WORLD_OBS_DIM {
                if sample.raw_obs[c] != 0.0 {
                    self.w_belief[base + c] =
                        (self.w_belief[base + c] - lr_b * g_pre * sample.raw_obs[c]).clamp(-2.5, 2.5);
                }
            }
        }
        self.updates += 1;
        (dyn_mse / WORLD_OBS_DIM as f64, r_err.abs())
    }

    /// Online per-step update (trains immediately on current step + 4 balanced replay samples!)
    pub fn train_online_step(&mut self, tr: &WorldTransition, rng: &mut u64) {
        let (dm0, rm0) = self.train_single_transition(tr, false);
        let mut dm_sum = dm0;
        let mut rm_sum = rm0;
        let mut cnt = 1.0;

        for k in 0..4usize {
            *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = *rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let pick = (z ^ (z >> 31)) as usize;
            let sample = if k == 0 && !self.pos_replay.is_empty() {
                self.pos_replay[pick % self.pos_replay.len()].clone()
            } else if k == 1 && !self.neg_replay.is_empty() {
                self.neg_replay[pick % self.neg_replay.len()].clone()
            } else if !self.replay.is_empty() {
                self.replay[pick % self.replay.len()].clone()
            } else {
                continue;
            };
            let (dm, rm) = self.train_single_transition(&sample, true);
            dm_sum += dm;
            rm_sum += rm;
            cnt += 1.0;
        }
        self.last_dyn_mse = 0.90 * self.last_dyn_mse + 0.10 * (dm_sum / cnt);
        self.last_rew_mae = 0.90 * self.last_rew_mae + 0.10 * (rm_sum / cnt);
    }

    /// Commit completed episode with low-variance 3-step bootstrapped returns into 3-bucket balanced replay!
    pub fn commit_episode_and_train(&mut self, mut ep: Vec<WorldTransition>, won: bool, rng: &mut u64) {
        if ep.is_empty() {
            return;
        }
        let gamma = 0.94;
        let len = ep.len();
        for t in 0..len {
            let mut r3 = 0.0;
            let mut disc = 1.0;
            let mut ended = false;
            let end_k = (t + 3).min(len);
            for k in t..end_k {
                r3 += disc * ep[k].r;
                disc *= gamma;
                if ep[k].done {
                    ended = true;
                    break;
                }
            }
            if !ended && end_k < len {
                let (qn, _) = self.predict_q(&ep[end_k].s);
                let mq = qn.into_iter().fold(f64::NEG_INFINITY, f64::max);
                r3 += disc * mq;
            }
            ep[t].ret_3step = r3;
        }
        for tr in ep {
            if won || tr.r > 0.25 {
                if self.pos_replay.len() >= 1800 {
                    self.pos_replay.remove(0);
                }
                self.pos_replay.push(tr.clone());
            } else if tr.r < -0.25 {
                if self.neg_replay.len() >= 900 {
                    self.neg_replay.remove(0);
                }
                self.neg_replay.push(tr.clone());
            }
            if self.replay.len() >= 3600 {
                self.replay.remove(0);
            }
            self.replay.push(tr);
        }

        // Focused end-of-episode replay consolidation (24 balanced samples)
        let mut dm_sum = 0.0;
        let mut rm_sum = 0.0;
        let mut cnt = 0.0;
        for step_i in 0..24usize {
            *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = *rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            let pick = (z ^ (z >> 31)) as usize;
            let sample = if step_i % 4 == 0 && !self.pos_replay.is_empty() {
                self.pos_replay[pick % self.pos_replay.len()].clone()
            } else if step_i % 4 == 1 && !self.neg_replay.is_empty() {
                self.neg_replay[pick % self.neg_replay.len()].clone()
            } else if !self.replay.is_empty() {
                self.replay[pick % self.replay.len()].clone()
            } else {
                break;
            };
            let (dm, rm) = self.train_single_transition(&sample, true);
            dm_sum += dm;
            rm_sum += rm;
            cnt += 1.0;
        }
        if cnt > 0.0 {
            self.last_dyn_mse = 0.75 * self.last_dyn_mse + 0.25 * (dm_sum / cnt);
            self.last_rew_mae = 0.75 * self.last_rew_mae + 0.25 * (rm_sum / cnt);
        }
    }

    /// Pure Learned Coordinate-Free Egocentric Exact 2-Step (`4x4=16`) + 3rd-Step Latent Tree Search Planner!
    /// ZERO global `(r, c)` coordinates! ZERO tile-ID rules! ZERO Manhattan heuristic! ZERO BFS!
    pub fn plan_latent_mcts_action(
        &self,
        s0: &[f64; WORLD_STATE_DIM],
        episodic_states: &[([f64; WORLD_OBS_DIM], [f64; WORLD_BELIEF_DIM])],
        ep_mem: &[([f64; WORLD_OBS_DIM], [f64; WORLD_BELIEF_DIM], usize, [f64; WORLD_OBS_DIM], f64, bool)],
        eps: f64,
        curiosity_w: f64,
        rng: &mut u64,
    ) -> usize {
        *rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let u = (((*rng ^ (*rng >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9)) >> 11) as f64 / ((1u64 << 53) as f64);
        if u < eps {
            return ((*rng >> 4) as usize) % 4;
        }

        let gamma = 0.94;
        let step_n = self.mean_action_step_norm().max(0.15);
        let revisit_tol = 0.50 * step_n;
        let stall_tol = 0.38 * step_n;
        let nov_scale = 1.55 * step_n;
        let (q_root, _) = self.predict_q(s0);
        let mut best_a = 0usize;
        let mut best_score = f64::NEG_INFINITY;

        let prev_b_opt = if episodic_states.len() >= 2 {
            Some(&episodic_states[episodic_states.len() - 2].1)
        } else {
            None
        };

        for a0 in 0..4 {
            let (s1, r1, t1) = self.imagine_grounded(s0, a0, ep_mem);
            let b1 = &s1[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM];
            let b0 = &s0[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM];
            // Pure dimensionless internal path-integrator check (ZERO env coords & ZERO magic step scale!)
            let step_p1 = Self::path_disp_dist(b1, b0);
            let mut min_p1 = 2.0 * nov_scale;
            let mut revisits1 = 0usize;
            for (eo, eb) in episodic_states {
                // Only count revisits in the SAME observable inventory phase (`inv_flag`)!
                if (s1[125] - eo[125]).abs() < 0.5 {
                    let dp = Self::path_disp_dist(b1, eb);
                    if dp < min_p1 {
                        min_p1 = dp;
                    }
                    if dp < revisit_tol {
                        revisits1 += 1;
                    }
                }
            }
            // Pure self-learned displacement stall check (`ZERO` hardcoded reward threshold or `0.09`!)
            let act_norm0 = (self.act_disp[a0][0].powi(2) + self.act_disp[a0][1].powi(2)).sqrt();
            let stall_pen = if act_norm0 > 0.35 * step_n && step_p1 < stall_tol { -0.75 } else { 0.0 };
            let backtrack_pen = if let Some(pb) = prev_b_opt {
                if act_norm0 > 0.35 * step_n && r1 < 0.20 && Self::path_disp_dist(b1, pb) < revisit_tol {
                    -0.36
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let nov1 = if revisits1 == 0 {
                curiosity_w * (min_p1 / nov_scale).clamp(0.0, 1.0)
            } else {
                -0.46 * (revisits1.min(6) as f64)
            };

            let tree_val = if t1 > 0.68 {
                r1 + stall_pen + backtrack_pen + nov1
            } else {
                let mut best_v1 = f64::NEG_INFINITY;
                for a1 in 0..4 {
                    let (s2, r2, t2) = self.imagine_grounded(&s1, a1, ep_mem);
                    let b2 = &s2[WORLD_OBS_DIM..WORLD_OBS_DIM + WORLD_BELIEF_DIM];
                    let step_p2 = Self::path_disp_dist(b2, b1);
                    let back_p2 = Self::path_disp_dist(b2, b0);
                    let mut min_p2 = 2.0 * nov_scale;
                    let mut revisits2 = if back_p2 < revisit_tol && (s2[125] - s0[125]).abs() < 0.5 { 1usize } else { 0usize };
                    for (eo, eb) in episodic_states {
                        if (s2[125] - eo[125]).abs() < 0.5 {
                            let dp = Self::path_disp_dist(b2, eb);
                            if dp < min_p2 {
                                min_p2 = dp;
                            }
                            if dp < revisit_tol {
                                revisits2 += 1;
                            }
                        }
                    }
                    let nov2 = if revisits2 == 0 {
                        0.60 * curiosity_w * (min_p2 / nov_scale).clamp(0.0, 1.0)
                    } else {
                        -0.22 * (revisits2.min(4) as f64)
                    };
                    let act_norm1 = (self.act_disp[a1][0].powi(2) + self.act_disp[a1][1].powi(2)).sqrt();
                    let pen2 = if act_norm1 > 0.35 * step_n && step_p2 < stall_tol { -0.32 } else { 0.0 };

                    let v2 = if t2 > 0.68 {
                        r2 + pen2 + nov2
                    } else {
                        let (q2, _) = self.predict_q(&s2);
                        let mut a2_best = 0usize;
                        let mut q2_max = f64::NEG_INFINITY;
                        for a2 in 0..4 {
                            if q2[a2] > q2_max {
                                q2_max = q2[a2];
                                a2_best = a2;
                            }
                        }
                        let (s3, r3, t3) = self.imagine_grounded(&s2, a2_best, ep_mem);
                        let v3 = if t3 > 0.68 {
                            r3
                        } else {
                            let (q3, _) = self.predict_q(&s3);
                            let q3_max = q3.into_iter().fold(f64::NEG_INFINITY, f64::max);
                            r3 + gamma * q3_max
                        };
                        r2 + pen2 + nov2 + gamma * (0.55 * v3 + 0.45 * q2_max)
                    };
                    if v2 > best_v1 {
                        best_v1 = v2;
                    }
                }
                r1 + stall_pen + backtrack_pen + nov1 + gamma * best_v1
            };

            let sc = 0.62 * tree_val + 0.38 * (q_root[a0] + nov1 + stall_pen + backtrack_pen);
            if sc > best_score {
                best_score = sc;
                best_a = a0;
            }
        }
        best_a
    }
}

#[derive(Clone, Debug)]
pub struct UniversalEnergyCore {
    // --- End-to-End Trainable Raw Tropical Deep-Set Encoder (`10 -> 64 -> 32`, pooled to `64`) ---
    pub w_enc1: Vec<f64>,
    pub w_enc1_init: Vec<f64>,
    pub b_enc1: Vec<f64>,
    pub w_enc2: Vec<f64>,
    pub w_enc2_init: Vec<f64>,
    pub b_enc2: Vec<f64>,

    // --- Weight-Tied Input-Anchored Contractive Equilibrium Core (`||W_core||_inf <= 0.75`) ---
    pub w_core: Vec<f64>,
    pub w_core_init: Vec<f64>,
    pub w_core_q: Vec<f64>,
    pub b_core: Vec<f64>,

    // --- SIX State-Conditioned Learned Heads (v5.1) + Empirical Base-Rate Calibration ---
    pub w_op: Vec<f64>,    // [12 x 64] full-tree operator presence
    pub b_op: Vec<f64>,
    pub op_base_rate: [f64; NUM_OP_CLASSES], // empirical P_base(op) from training corpus for exact Bayes Factor!
    pub w_next: Vec<f64>,  // [12 x 64] EXACT NEXT-OP conditional inverse execution policy!
    pub b_next: Vec<f64>,
    pub w_var: Vec<f64>,   // [3 x 64]  input variable relevance `x0, x1, x2`
    pub b_var: [f64; 3],
    pub var_base_rate: [f64; 3], // empirical P_base(var) for exact Bayes Factor!
    pub w_val: Vec<f64>,   // [64]      residual progress-to-goal value head
    pub b_val: f64,
    pub w_align: Vec<f64>, // [64 x 64] contrastive task-program alignment
    pub w_need: Vec<f64>,  // [64 x 64] predicts latent embedding of NEEDED SIBLING PARTNER!
    pub w_tactic: Vec<f64>, // [8 x 64]  NEURAL LEAN 4 PROOF-STATE TACTIC POLICY (Expert Iteration!)
    pub b_tactic: Vec<f64>,

    // --- ZERO-RULE Coordinate-Free Egocentric World Model + Replay EMA Double-Q ---
    pub world_model: NonlinearWorldModel,

    // --- Executable AST-Level Self-Modifying Meta-Priority Program (`OmegaExpr`!) + Strategy Genome ---
    pub strategy: SearchStrategyGenome,
    pub meta_priority_ast: OmegaExpr,

    pub cegis_barriers: Vec<CegisBarrier>,
    pub memory: UnboundedTropicalMemory,
    pub gamma: f64,
    pub eta_cegis: f64,
    pub inv_tau: f64,
    pub rng_state: u64,
    pub train_steps_completed: usize,
    pub last_train_loss: f64,
}

fn splitmix_f64(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 11) as f64 / ((1u64 << 53) as f64)) * 2.0 - 1.0
}

fn median_f64(vals: &mut [f64]) -> f64 {
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = vals.len();
    if n % 2 == 1 { vals[n / 2] } else { 0.5 * (vals[n / 2 - 1] + vals[n / 2]) }
}

pub fn robust_corr(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len().min(ys.len());
    if n < 3 {
        return 0.0;
    }
    let mut xv = xs[..n].to_vec();
    let mut yv = ys[..n].to_vec();
    let mx = median_f64(&mut xv);
    let my = median_f64(&mut yv);
    let mut abs_dy: Vec<f64> = (0..n).map(|i| (ys[i] - my).abs()).collect();
    let mad_y = median_f64(&mut abs_dy).max(1e-4);
    let cap_y = 3.5 * mad_y;
    let mut num = 0.0;
    let mut dx2 = 0.0;
    let mut dy2 = 0.0;
    for i in 0..n {
        let dx = xs[i] - mx;
        let dy = (ys[i] - my).clamp(-cap_y, cap_y);
        num += dx * dy;
        dx2 += dx * dx;
        dy2 += dy * dy;
    }
    let den = (dx2 * dy2).sqrt();
    if den < 1e-9 { 0.0 } else { (num / den).clamp(-1.0, 1.0) }
}

impl UniversalEnergyCore {
    pub fn new(seed: u64) -> Self {
        let mut rng = seed ^ 0x0E6A_B500_2026_1125;

        let s1 = (1.0 / RAW_FEAT_DIM as f64).sqrt();
        let w_enc1: Vec<f64> = (0..ENC_MID * RAW_FEAT_DIM).map(|_| splitmix_f64(&mut rng) * s1 * 0.9).collect();
        let b_enc1 = (0..ENC_MID).map(|_| splitmix_f64(&mut rng) * 0.08).collect();

        let s2 = (1.0 / ENC_MID as f64).sqrt();
        let w_enc2: Vec<f64> = (0..ENC_HIDDEN * ENC_MID).map(|_| splitmix_f64(&mut rng) * s2 * 0.9).collect();
        let b_enc2 = (0..ENC_HIDDEN).map(|_| splitmix_f64(&mut rng) * 0.05).collect();

        let mut w_core = vec![0.0; OMEGA_DIM * OMEGA_DIM];
        for r in 0..OMEGA_DIM {
            let mut row_l1 = 0.0;
            for c in 0..OMEGA_DIM {
                let v = if r == c {
                    0.40 + 0.08 * splitmix_f64(&mut rng)
                } else {
                    0.025 * splitmix_f64(&mut rng)
                };
                w_core[r * OMEGA_DIM + c] = v;
                row_l1 += v.abs();
            }
            if row_l1 > 0.72 {
                let sc = 0.72 / row_l1;
                for c in 0..OMEGA_DIM {
                    w_core[r * OMEGA_DIM + c] *= sc;
                }
            }
        }
        let b_core = vec![0.0; OMEGA_DIM];

        let w_op: Vec<f64> = (0..NUM_OP_CLASSES * OMEGA_DIM).map(|_| 0.08 * splitmix_f64(&mut rng)).collect();
        let b_op = vec![0.0; NUM_OP_CLASSES];
        let w_next: Vec<f64> = (0..NUM_OP_CLASSES * OMEGA_DIM).map(|_| 0.08 * splitmix_f64(&mut rng)).collect();
        let b_next = vec![0.0; NUM_OP_CLASSES];
        let w_var: Vec<f64> = (0..3 * OMEGA_DIM).map(|_| 0.08 * splitmix_f64(&mut rng)).collect();
        let b_var = [0.0f64; 3];
        let w_val: Vec<f64> = (0..OMEGA_DIM).map(|_| 0.08 * splitmix_f64(&mut rng)).collect();

        let mut w_align = vec![0.0; OMEGA_DIM * OMEGA_DIM];
        let mut w_need = vec![0.0; OMEGA_DIM * OMEGA_DIM];
        for r in 0..OMEGA_DIM {
            w_align[r * OMEGA_DIM + r] = 1.0;
            w_need[r * OMEGA_DIM + r] = 0.5;
        }
        let w_tactic: Vec<f64> = (0..NUM_TACTIC_CLASSES * OMEGA_DIM).map(|_| 0.08 * splitmix_f64(&mut rng)).collect();
        let b_tactic = vec![0.0; NUM_TACTIC_CLASSES];

        // Default executable AST meta-priority program over `(x0=fit_term, x1=neural_llr, x2=complexity)`:
        //   `(x0 + x2) + (0.35 * x1)` — structurally evolvable into nonlinear/conditional search programs!
        let default_meta_ast = OmegaExpr::Add(
            Box::new(OmegaExpr::Add(Box::new(OmegaExpr::Arg(0)), Box::new(OmegaExpr::Arg(2)))),
            Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Const(0.35)), Box::new(OmegaExpr::Arg(1)))),
        );

        let mut core = Self {
            w_enc1_init: w_enc1.clone(),
            w_enc1,
            b_enc1,
            w_enc2_init: w_enc2.clone(),
            w_enc2,
            b_enc2,
            w_core_init: w_core.clone(),
            w_core_q: vec![0.0; OMEGA_DIM * OMEGA_DIM],
            w_core,
            b_core,
            w_op,
            b_op,
            op_base_rate: [0.5; NUM_OP_CLASSES],
            w_next,
            b_next,
            w_var,
            b_var,
            var_base_rate: [0.5; 3],
            w_val,
            b_val: 0.0,
            w_align,
            w_need,
            w_tactic,
            b_tactic,
            world_model: NonlinearWorldModel::new(seed),
            strategy: SearchStrategyGenome::default(),
            meta_priority_ast: default_meta_ast,
            cegis_barriers: Vec::new(),
            memory: UnboundedTropicalMemory::new(),
            gamma: 0.75,
            eta_cegis: 0.25,
            inv_tau: 0.15,
            rng_state: rng,
            train_steps_completed: 0,
            last_train_loss: 0.0,
        };
        core.sync_nvfp4_core_weights();
        core
    }

    pub fn sync_nvfp4_core_weights(&mut self) {
        self.project_contractive_linf(0.75);
        self.w_core_q = nvfp4_quantize(&self.w_core);
        omega_upload_contractive_core_all_gpus(&self.w_core_q, &self.b_core, OMEGA_DIM);
    }

    pub fn project_contractive_linf(&mut self, max_linf: f64) {
        // Strictly idempotent (`P(P(W)) == P(W)` bit-for-bit!) via 1e-12 guard against IEEE-754 summation rounding
        for r in 0..OMEGA_DIM {
            let mut l1 = 0.0;
            for c in 0..OMEGA_DIM {
                l1 += self.w_core[r * OMEGA_DIM + c].abs();
            }
            if l1 > max_linf + 1e-12 {
                let sc = (max_linf - 1e-12) / l1;
                for c in 0..OMEGA_DIM {
                    self.w_core[r * OMEGA_DIM + c] *= sc;
                }
            }
        }
    }

    pub fn w_core_linf_norm(&self) -> f64 {
        let mut mx = 0.0f64;
        for r in 0..OMEGA_DIM {
            let mut s = 0.0;
            for c in 0..OMEGA_DIM {
                s += self.w_core_q[r * OMEGA_DIM + c].abs();
            }
            if s > mx {
                mx = s;
            }
        }
        mx
    }

    pub fn w_core_l1_drift(&self) -> f64 {
        self.w_core.iter().zip(self.w_core_init.iter()).map(|(a, b)| (a - b).abs()).sum()
    }

    pub fn w_enc_l1_drift(&self) -> f64 {
        let d1: f64 = self.w_enc1.iter().zip(self.w_enc1_init.iter()).map(|(a, b)| (a - b).abs()).sum();
        let d2: f64 = self.w_enc2.iter().zip(self.w_enc2_init.iter()).map(|(a, b)| (a - b).abs()).sum();
        d1 + d2
    }

    pub fn clear_cegis_barriers(&mut self) {
        self.cegis_barriers.clear();
    }

    pub fn inject_cegis_barrier(&mut self, center: [f64; OMEGA_DIM], strength: f64) {
        if self.cegis_barriers.len() >= 1024 {
            self.cegis_barriers.remove(0);
        }
        self.cegis_barriers.push(CegisBarrier { center, strength });
    }

    pub fn cegis_barrier_energy(&self, z: &[f64; OMEGA_DIM]) -> f64 {
        let mut e = 0.0;
        for b in &self.cegis_barriers {
            let mut l1 = 0.0;
            for d in 0..OMEGA_DIM {
                l1 += (z[d] - b.center[d]).abs();
            }
            e += b.strength * (-l1 * self.inv_tau).exp();
        }
        e
    }

    fn encode_raw_tokens_cached(&self, tokens: &[[f64; RAW_FEAT_DIM]]) -> EncoderForwardCache {
        if tokens.is_empty() {
            return EncoderForwardCache {
                u_kept: Vec::new(),
                a_kept: Vec::new(),
                h_kept: Vec::new(),
                argmax_idx: [0; ENC_HIDDEN],
                z0: [0.0; OMEGA_DIM],
            };
        }

        // Preserve all raw tokens (`asinh` in raw_tokens already compresses outliers smoothly without losing
        // high-signal quadratic/branching boundary points!).
        let keep_n = tokens.len();
        let mut u_kept = Vec::with_capacity(keep_n);
        let mut a_kept = Vec::with_capacity(keep_n);
        let mut h_kept = Vec::with_capacity(keep_n);

        for &u in tokens.iter() {
            let mut a = [0.0f64; ENC_MID];
            for r in 0..ENC_MID {
                let mut acc = self.b_enc1[r];
                let row = r * RAW_FEAT_DIM;
                for c in 0..RAW_FEAT_DIM {
                    acc += self.w_enc1[row + c] * u[c];
                }
                a[r] = acc.tanh();
            }
            let mut h = [0.0f64; ENC_HIDDEN];
            for r in 0..ENC_HIDDEN {
                let mut acc = self.b_enc2[r];
                let row = r * ENC_MID;
                for c in 0..ENC_MID {
                    acc += self.w_enc2[row + c] * a[c];
                }
                h[r] = acc.tanh();
            }
            u_kept.push(u);
            a_kept.push(a);
            h_kept.push(h);
        }

        let inv_n = 1.0 / (h_kept.len().max(1) as f64);
        let mut z_raw = [0.0f64; OMEGA_DIM];
        let mut argmax_idx = [0usize; ENC_HIDDEN];
        for k in 0..ENC_HIDDEN {
            let mut mean_k = 0.0;
            let mut max_k = f64::NEG_INFINITY;
            let mut best_i = 0usize;
            for (i, h) in h_kept.iter().enumerate() {
                mean_k += h[k];
                if h[k] > max_k {
                    max_k = h[k];
                    best_i = i;
                }
            }
            z_raw[k] = mean_k * inv_n;
            z_raw[ENC_HIDDEN + k] = max_k;
            argmax_idx[k] = best_i;
        }
        let q = nvfp4_quantize(&z_raw);
        let mut z0 = [0.0f64; OMEGA_DIM];
        z0.copy_from_slice(&q[..OMEGA_DIM]);

        EncoderForwardCache { u_kept, a_kept, h_kept, argmax_idx, z0 }
    }

    pub fn encode_raw_tokens(&self, tokens: &[[f64; RAW_FEAT_DIM]]) -> [f64; OMEGA_DIM] {
        self.encode_raw_tokens_cached(tokens).z0
    }

    pub fn encode_numeric_residual(
        &self,
        pairs: &[([f64; 3], f64)],
        cur_preds: Option<&[f64]>,
    ) -> [f64; OMEGA_DIM] {
        let toks = raw_tokens_numeric_residual(pairs, cur_preds);
        self.encode_raw_tokens(&toks)
    }

    pub fn encode_sequence_io(&self, pairs: &[(Vec<f64>, f64)]) -> [f64; OMEGA_DIM] {
        let toks = raw_tokens_sequence(pairs);
        self.encode_raw_tokens(&toks)
    }

    pub fn encode_grid_io(&self, pairs: &[(Grid2D, Grid2D)]) -> [f64; OMEGA_DIM] {
        let toks = raw_tokens_grid(pairs);
        self.encode_raw_tokens(&toks)
    }

    pub fn relax_parallel_particles(
        &mut self,
        z_anchor: &[f64; OMEGA_DIM],
        n_particles: usize,
        ticks: usize,
        use_cegis: bool,
        perturbation_noise: f64,
        device_id: usize,
    ) -> Vec<[f64; OMEGA_DIM]> {
        let np = n_particles.max(1);
        let mut z_init = vec![0.0f64; np * OMEGA_DIM];
        for p in 0..np {
            for d in 0..OMEGA_DIM {
                let jitter = if p == 0 {
                    0.0
                } else {
                    0.04 * splitmix_f64(&mut self.rng_state)
                };
                z_init[p * OMEGA_DIM + d] = (z_anchor[d] + jitter).clamp(-3.0, 3.0);
            }
        }
        let (cuts_flat, cut_w) = if use_cegis && !self.cegis_barriers.is_empty() {
            let mut cf = Vec::with_capacity(self.cegis_barriers.len() * OMEGA_DIM);
            let mut cw = Vec::with_capacity(self.cegis_barriers.len());
            for b in &self.cegis_barriers {
                cf.extend_from_slice(&b.center);
                cw.push(b.strength);
            }
            (cf, cw)
        } else {
            (Vec::new(), Vec::new())
        };

        let out_flat = omega_relax_particles_on_device(
            &self.w_core_q,
            &self.b_core,
            &z_init,
            np,
            OMEGA_DIM,
            &cuts_flat,
            &cut_w,
            ticks,
            self.gamma,
            self.eta_cegis,
            self.inv_tau,
            perturbation_noise,
            device_id,
        );

        let mut res = Vec::with_capacity(np);
        for p in 0..np {
            let mut v = [0.0f64; OMEGA_DIM];
            v.copy_from_slice(&out_flat[p * OMEGA_DIM..(p + 1) * OMEGA_DIM]);
            res.push(v);
        }
        res
    }

    /// Predict full state-conditioned outputs from equilibrium `z*`:
    /// `(op_presence[12], next_op_probs[12], var_rel[3], val_prob, z_need[64], z_star[64])`.
    pub fn predict_state_policy(
        &mut self,
        z_anchor: &[f64; OMEGA_DIM],
        ticks: usize,
        device_id: usize,
    ) -> ([f64; NUM_OP_CLASSES], [f64; NUM_OP_CLASSES], [f64; 3], f64, [f64; OMEGA_DIM], [f64; OMEGA_DIM]) {
        let z_star = self.relax_parallel_particles(z_anchor, 1, ticks, false, 0.0, device_id)[0];
        let (op_p, next_p, var_p, val_p, z_need) = self.predict_heads_from_latent(&z_star);
        (op_p, next_p, var_p, val_p, z_need, z_star)
    }

    /// Fast read-only head projection directly from a latent state `z` (zero device sync overhead!).
    pub fn predict_heads_from_latent(
        &self,
        z: &[f64; OMEGA_DIM],
    ) -> ([f64; NUM_OP_CLASSES], [f64; NUM_OP_CLASSES], [f64; 3], f64, [f64; OMEGA_DIM]) {
        let mut op_p = [0.5f64; NUM_OP_CLASSES];
        let mut next_logits = [0.0f64; NUM_OP_CLASSES];
        let mut max_nl = f64::NEG_INFINITY;
        for c in 0..NUM_OP_CLASSES {
            let row = c * OMEGA_DIM;
            let mut l_op = self.b_op[c];
            let mut l_nx = self.b_next[c];
            for d in 0..OMEGA_DIM {
                l_op += self.w_op[row + d] * z[d];
                l_nx += self.w_next[row + d] * z[d];
            }
            op_p[c] = 1.0 / (1.0 + (-l_op.clamp(-10.0, 10.0)).exp());
            let cl = l_nx.clamp(-10.0, 10.0);
            next_logits[c] = cl;
            if cl > max_nl {
                max_nl = cl;
            }
        }
        // Softmax over 9 symbolic next-op combinators (0..9)
        let mut next_p = [1.0 / 9.0; NUM_OP_CLASSES];
        let mut sum_exp = 0.0;
        for c in 0..9 {
            let ex = (next_logits[c] - max_nl).exp();
            next_p[c] = ex;
            sum_exp += ex;
        }
        for c in 0..9 {
            next_p[c] = (next_p[c] / sum_exp.max(1e-9)).clamp(0.01, 0.95);
        }

        let mut var_p = [0.5f64; 3];
        for v in 0..3 {
            let row = v * OMEGA_DIM;
            let mut lv = self.b_var[v];
            for d in 0..OMEGA_DIM {
                lv += self.w_var[row + d] * z[d];
            }
            var_p[v] = 1.0 / (1.0 + (-lv.clamp(-10.0, 10.0)).exp());
        }

        let mut vl = self.b_val;
        for d in 0..OMEGA_DIM {
            vl += self.w_val[d] * z[d];
        }
        let val_p = 1.0 / (1.0 + (-vl.clamp(-10.0, 10.0)).exp());

        let mut z_need = [0.0f64; OMEGA_DIM];
        for r in 0..OMEGA_DIM {
            let row = r * OMEGA_DIM;
            let mut acc = 0.0;
            for c in 0..OMEGA_DIM {
                acc += self.w_need[row + c] * z[c];
            }
            z_need[r] = acc.tanh();
        }
        (op_p, next_p, var_p, val_p, z_need)
    }

    pub fn learned_program_energy(&self, relaxed_task_z: &[f64; OMEGA_DIM], prog_z: &[f64; OMEGA_DIM]) -> f64 {
        let mut l1 = 0.0;
        for r in 0..OMEGA_DIM {
            let mut proj = 0.0;
            let row = r * OMEGA_DIM;
            for c in 0..OMEGA_DIM {
                proj += self.w_align[row + c] * prog_z[c];
            }
            l1 += (relaxed_task_z[r] - proj).abs();
        }
        l1 / OMEGA_DIM as f64
    }

    pub fn partner_need_distance(z_need: &[f64; OMEGA_DIM], cand_z: &[f64; OMEGA_DIM]) -> f64 {
        let mut l1 = 0.0;
        for d in 0..OMEGA_DIM {
            l1 += (z_need[d] - cand_z[d]).abs();
        }
        l1 / OMEGA_DIM as f64
    }

    /// End-to-End Mini-Batch AdamW STE Backpropagation across ALL 6 heads + Contractive Core + Raw Encoder!
    pub fn train_end_to_end_ste_epochs(
        &mut self,
        samples: &[RawTrainSample],
        epochs: usize,
        ticks: usize,
        lr: f64,
        device_id: usize,
    ) -> (f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0);
        }
        // Record exact empirical class base rates `P_base(op)`, `P_base(var)` for Bayes Factor calibration!
        let mut op_sum = [0.0f64; NUM_OP_CLASSES];
        let mut var_sum = [0.0f64; 3];
        for s in samples {
            for c in 0..NUM_OP_CLASSES {
                op_sum[c] += s.op_mask[c];
            }
            for v in 0..3 {
                var_sum[v] += s.var_mask[v];
            }
        }
        let sn = samples.len() as f64;
        for c in 0..NUM_OP_CLASSES {
            let emp = (op_sum[c] / sn).clamp(0.05, 0.90);
            self.op_base_rate[c] = if self.train_steps_completed == 0 { emp } else { 0.5 * self.op_base_rate[c] + 0.5 * emp };
        }
        for v in 0..3 {
            let emp = (var_sum[v] / sn).clamp(0.10, 0.92);
            self.var_base_rate[v] = if self.train_steps_completed == 0 { emp } else { 0.5 * self.var_base_rate[v] + 0.5 * emp };
        }
        let mut init_loss = 0.0;
        let mut final_loss = 0.0;
        let t_steps = ticks.clamp(2, 5);

        let mut m_w_op = vec![0.0; self.w_op.len()];
        let mut v_w_op = vec![0.0; self.w_op.len()];
        let mut m_b_op = vec![0.0; self.b_op.len()];
        let mut v_b_op = vec![0.0; self.b_op.len()];
        let mut m_w_nx = vec![0.0; self.w_next.len()];
        let mut v_w_nx = vec![0.0; self.w_next.len()];
        let mut m_b_nx = vec![0.0; self.b_next.len()];
        let mut v_b_nx = vec![0.0; self.b_next.len()];
        let mut m_w_vr = vec![0.0; self.w_var.len()];
        let mut v_w_vr = vec![0.0; self.w_var.len()];
        let mut m_b_vr = [0.0f64; 3];
        let mut v_b_vr = [0.0f64; 3];
        let mut m_w_val = vec![0.0; self.w_val.len()];
        let mut v_w_val = vec![0.0; self.w_val.len()];
        let mut m_b_val = 0.0f64;
        let mut v_b_val = 0.0f64;
        let mut m_w_align = vec![0.0; self.w_align.len()];
        let mut v_w_align = vec![0.0; self.w_align.len()];
        let mut m_w_need = vec![0.0; self.w_need.len()];
        let mut v_w_need = vec![0.0; self.w_need.len()];
        let mut m_w_core = vec![0.0; self.w_core.len()];
        let mut v_w_core = vec![0.0; self.w_core.len()];
        let mut m_b_core = vec![0.0; self.b_core.len()];
        let mut v_b_core = vec![0.0; self.b_core.len()];
        let mut m_w_enc2 = vec![0.0; self.w_enc2.len()];
        let mut v_w_enc2 = vec![0.0; self.w_enc2.len()];
        let mut m_b_enc2 = vec![0.0; self.b_enc2.len()];
        let mut v_b_enc2 = vec![0.0; self.b_enc2.len()];
        let mut m_w_enc1 = vec![0.0; self.w_enc1.len()];
        let mut v_w_enc1 = vec![0.0; self.w_enc1.len()];
        let mut m_b_enc1 = vec![0.0; self.b_enc1.len()];
        let mut v_b_enc1 = vec![0.0; self.b_enc1.len()];

        let adamw_update = |w: &mut [f64], g: &[f64], m: &mut [f64], v: &mut [f64], step_lr: f64, wd: f64, bc1: f64, bc2: f64| {
            for k in 0..w.len() {
                let gk = g[k].clamp(-3.0, 3.0);
                m[k] = 0.9 * m[k] + 0.1 * gk;
                v[k] = 0.999 * v[k] + 0.001 * gk * gk;
                let mh = m[k] / bc1;
                let vh = v[k] / bc2;
                w[k] = w[k] * (1.0 - step_lr * wd) - step_lr * mh / (vh.sqrt() + 1e-8);
            }
        };

        let mut step_counter = 0i32;
        let batch_size = 48usize;

        for ep in 0..epochs {
            let mut ep_loss = 0.0;
            let total_n = samples.len();

            let mut order: Vec<usize> = (0..total_n).collect();
            let mut perm_rng = (ep as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5_1234_5678;
            for i in (1..total_n).rev() {
                perm_rng = perm_rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = perm_rng;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                let j = ((z ^ (z >> 31)) as usize) % (i + 1);
                order.swap(i, j);
            }

            for chunk in order.chunks(batch_size) {
                let n = chunk.len();
                if n == 0 {
                    continue;
                }
                let mut enc_caches: Vec<EncoderForwardCache> = Vec::with_capacity(n);
                let mut pos_zs: Vec<[f64; OMEGA_DIM]> = Vec::with_capacity(n);
                let mut neg_zs: Vec<[f64; OMEGA_DIM]> = Vec::with_capacity(n);
                let mut p_pos_zs: Vec<Option<[f64; OMEGA_DIM]>> = Vec::with_capacity(n);
                let mut p_neg_zs: Vec<Option<[f64; OMEGA_DIM]>> = Vec::with_capacity(n);

                for &idx in chunk {
                    let s = &samples[idx];
                    enc_caches.push(self.encode_raw_tokens_cached(&s.raw_tokens));
                    pos_zs.push(self.encode_raw_tokens(&s.pos_tokens));
                    neg_zs.push(self.encode_raw_tokens(&s.neg_tokens));
                    p_pos_zs.push(s.partner_pos_tokens.as_ref().map(|t| self.encode_raw_tokens(t)));
                    p_neg_zs.push(s.partner_neg_tokens.as_ref().map(|t| self.encode_raw_tokens(t)));
                }

                let mut z_hist: Vec<Vec<f64>> = Vec::with_capacity(t_steps + 1);
                let mut h_hist: Vec<Vec<f64>> = Vec::with_capacity(t_steps);
                let mut z_anchor_mat = vec![0.0f64; n * OMEGA_DIM];
                for (i, c) in enc_caches.iter().enumerate() {
                    z_anchor_mat[i * OMEGA_DIM..(i + 1) * OMEGA_DIM].copy_from_slice(&c.z0);
                }
                z_hist.push(z_anchor_mat.clone());

                for _ in 0..t_steps {
                    let z_cur = z_hist.last().unwrap();
                    let mut z_t = vec![0.0f64; OMEGA_DIM * n];
                    for i in 0..n {
                        for d in 0..OMEGA_DIM {
                            z_t[d * n + i] = z_cur[i * OMEGA_DIM + d];
                        }
                    }
                    let wz_t = omega_batched_nvfp4_gemm_on_device(
                        &self.w_core_q,
                        &z_t,
                        OMEGA_DIM,
                        n,
                        OMEGA_DIM,
                        device_id,
                    );
                    let mut h_cur = vec![0.0f64; n * OMEGA_DIM];
                    let mut z_next = vec![0.0f64; n * OMEGA_DIM];
                    for i in 0..n {
                        let mut row_buf = [0.0f64; OMEGA_DIM];
                        for d in 0..OMEGA_DIM {
                            let pre = wz_t[d * n + i] + self.b_core[d];
                            let th = pre.tanh();
                            h_cur[i * OMEGA_DIM + d] = th;
                            row_buf[d] = (self.gamma * th + (1.0 - self.gamma) * z_anchor_mat[i * OMEGA_DIM + d]).clamp(-4.0, 4.0);
                        }
                        let q = nvfp4_quantize(&row_buf);
                        z_next[i * OMEGA_DIM..(i + 1) * OMEGA_DIM].copy_from_slice(&q[..OMEGA_DIM]);
                    }
                    h_hist.push(h_cur);
                    z_hist.push(z_next);
                }

                let mut g_w_op = vec![0.0f64; NUM_OP_CLASSES * OMEGA_DIM];
                let mut g_b_op = vec![0.0f64; NUM_OP_CLASSES];
                let mut g_w_nx = vec![0.0f64; NUM_OP_CLASSES * OMEGA_DIM];
                let mut g_b_nx = vec![0.0f64; NUM_OP_CLASSES];
                let mut g_w_vr = vec![0.0f64; 3 * OMEGA_DIM];
                let mut g_b_vr = [0.0f64; 3];
                let mut g_w_val = vec![0.0f64; OMEGA_DIM];
                let mut g_b_val = 0.0f64;
                let mut g_w_align = vec![0.0f64; OMEGA_DIM * OMEGA_DIM];
                let mut g_w_need = vec![0.0f64; OMEGA_DIM * OMEGA_DIM];
                let mut g_w_core = vec![0.0f64; OMEGA_DIM * OMEGA_DIM];
                let mut g_b_core = vec![0.0f64; OMEGA_DIM];
                let mut g_w_enc2 = vec![0.0f64; ENC_HIDDEN * ENC_MID];
                let mut g_b_enc2 = vec![0.0f64; ENC_HIDDEN];
                let mut g_w_enc1 = vec![0.0f64; ENC_MID * RAW_FEAT_DIM];
                let mut g_b_enc1 = vec![0.0f64; ENC_MID];

                let z_final = &z_hist[t_steps];
                let inv_n = 1.0 / (n as f64);

                for (i, &orig_idx) in chunk.iter().enumerate() {
                    let s = &samples[orig_idx];
                    let zf = &z_final[i * OMEGA_DIM..(i + 1) * OMEGA_DIM];
                    let mut dz = [0.0f64; OMEGA_DIM];

                    // 1. Operator presence BCE
                    for c in 0..NUM_OP_CLASSES {
                        let row = c * OMEGA_DIM;
                        let mut logit = self.b_op[c];
                        for d in 0..OMEGA_DIM {
                            logit += self.w_op[row + d] * zf[d];
                        }
                        let p = 1.0 / (1.0 + (-logit.clamp(-10.0, 10.0)).exp());
                        let y = s.op_mask[c];
                        ep_loss += 0.35 * (-(y * (p + 1e-7).ln() + (1.0 - y) * (1.0 - p + 1e-7).ln()));
                        let dl = 0.35 * (p - y) * inv_n;
                        g_b_op[c] += dl;
                        for d in 0..OMEGA_DIM {
                            g_w_op[row + d] += dl * zf[d];
                            dz[d] += dl * self.w_op[row + d];
                        }
                    }

                    // 2. State-Conditioned EXACT NEXT-OP Cross-Entropy (9 symbolic combinators)
                    if let Some(tgt_cls) = s.next_op_target {
                        if tgt_cls < 9 {
                            let mut nl = [0.0f64; 9];
                            let mut mx = f64::NEG_INFINITY;
                            for c in 0..9 {
                                let row = c * OMEGA_DIM;
                                let mut v = self.b_next[c];
                                for d in 0..OMEGA_DIM {
                                    v += self.w_next[row + d] * zf[d];
                                }
                                nl[c] = v.clamp(-10.0, 10.0);
                                if nl[c] > mx {
                                    mx = nl[c];
                                }
                            }
                            let mut ex = [0.0f64; 9];
                            let mut se = 0.0;
                            for c in 0..9 {
                                ex[c] = (nl[c] - mx).exp();
                                se += ex[c];
                            }
                            let p_tgt = (ex[tgt_cls] / se.max(1e-9)).max(1e-7);
                            ep_loss += 1.2 * (-p_tgt.ln());
                            for c in 0..9 {
                                let pc = ex[c] / se.max(1e-9);
                                let yc = if c == tgt_cls { 1.0 } else { 0.0 };
                                let dl = 1.2 * (pc - yc) * inv_n;
                                g_b_nx[c] += dl;
                                let row = c * OMEGA_DIM;
                                for d in 0..OMEGA_DIM {
                                    g_w_nx[row + d] += dl * zf[d];
                                    dz[d] += dl * self.w_next[row + d];
                                }
                            }
                        }
                    }

                    // 3. Variable Relevance BCE (`x0, x1, x2`)
                    for v in 0..3 {
                        let row = v * OMEGA_DIM;
                        let mut lv = self.b_var[v];
                        for d in 0..OMEGA_DIM {
                            lv += self.w_var[row + d] * zf[d];
                        }
                        let p = 1.0 / (1.0 + (-lv.clamp(-10.0, 10.0)).exp());
                        let y = s.var_mask[v];
                        ep_loss += 0.45 * (-(y * (p + 1e-7).ln() + (1.0 - y) * (1.0 - p + 1e-7).ln()));
                        let dl = 0.45 * (p - y) * inv_n;
                        g_b_vr[v] += dl;
                        for d in 0..OMEGA_DIM {
                            g_w_vr[row + d] += dl * zf[d];
                            dz[d] += dl * self.w_var[row + d];
                        }
                    }

                    // 4. Residual Progress-to-Goal Value BCE
                    let mut v_logit = self.b_val;
                    for d in 0..OMEGA_DIM {
                        v_logit += self.w_val[d] * zf[d];
                    }
                    let vp = 1.0 / (1.0 + (-v_logit.clamp(-10.0, 10.0)).exp());
                    let vy = s.value_target.clamp(0.0, 1.0);
                    ep_loss += 0.9 * (-(vy * (vp + 1e-7).ln() + (1.0 - vy) * (1.0 - vp + 1e-7).ln()));
                    let dvl = 0.9 * (vp - vy) * inv_n;
                    g_b_val += dvl;
                    for d in 0..OMEGA_DIM {
                        g_w_val[d] += dvl * zf[d];
                        dz[d] += dvl * self.w_val[d];
                    }

                    // 5. Contrastive Task-Program Alignment Hinge
                    let mut pos_proj = [0.0f64; OMEGA_DIM];
                    let mut neg_proj = [0.0f64; OMEGA_DIM];
                    let mut e_pos = 0.0;
                    let mut e_neg = 0.0;
                    for r in 0..OMEGA_DIM {
                        let row = r * OMEGA_DIM;
                        let mut pp = 0.0;
                        let mut np = 0.0;
                        for c in 0..OMEGA_DIM {
                            pp += self.w_align[row + c] * pos_zs[i][c];
                            np += self.w_align[row + c] * neg_zs[i][c];
                        }
                        pos_proj[r] = pp;
                        neg_proj[r] = np;
                        e_pos += (zf[r] - pp).abs();
                        e_neg += (zf[r] - np).abs();
                    }
                    let hinge = (0.25 + (e_pos - e_neg) / OMEGA_DIM as f64).max(0.0);
                    ep_loss += 0.30 * hinge;
                    if hinge > 0.0 {
                        let sc = 0.30 * inv_n / OMEGA_DIM as f64;
                        for r in 0..OMEGA_DIM {
                            let sp = if zf[r] >= pos_proj[r] { 1.0 } else { -1.0 };
                            let sn = if zf[r] >= neg_proj[r] { 1.0 } else { -1.0 };
                            dz[r] += sc * (sp - sn);
                            let row = r * OMEGA_DIM;
                            for c in 0..OMEGA_DIM {
                                g_w_align[row + c] += sc * (-sp * pos_zs[i][c] + sn * neg_zs[i][c]);
                            }
                        }
                    }

                    // 6. Contrastive Missing-Partner Need Head (`W_need * zf` -> closer to true sibling `B` than distractor!)
                    if let (Some(ref bp), Some(ref bn)) = (p_pos_zs[i], p_neg_zs[i]) {
                        let mut zn = [0.0f64; OMEGA_DIM];
                        let mut d_pos = 0.0;
                        let mut d_neg = 0.0;
                        for r in 0..OMEGA_DIM {
                            let row = r * OMEGA_DIM;
                            let mut acc = 0.0;
                            for c in 0..OMEGA_DIM {
                                acc += self.w_need[row + c] * zf[c];
                            }
                            let th = acc.tanh();
                            zn[r] = th;
                            d_pos += (th - bp[r]).abs();
                            d_neg += (th - bn[r]).abs();
                        }
                        let ph = (0.30 + (d_pos - d_neg) / OMEGA_DIM as f64).max(0.0);
                        ep_loss += 0.55 * ph;
                        if ph > 0.0 {
                            let sc = 0.55 * inv_n / OMEGA_DIM as f64;
                            for r in 0..OMEGA_DIM {
                                let sp = if zn[r] >= bp[r] { 1.0 } else { -1.0 };
                                let sn = if zn[r] >= bn[r] { 1.0 } else { -1.0 };
                                let dpre = sc * (sp - sn) * (1.0 - zn[r] * zn[r]);
                                let row = r * OMEGA_DIM;
                                for c in 0..OMEGA_DIM {
                                    g_w_need[row + c] += dpre * zf[c];
                                    dz[c] += dpre * self.w_need[row + c];
                                }
                            }
                        }
                    }

                    // Backprop through contractive equilibrium ticks
                    let mut dz_anchor = [0.0f64; OMEGA_DIM];
                    for t in (0..t_steps).rev() {
                        let z_prev = &z_hist[t][i * OMEGA_DIM..(i + 1) * OMEGA_DIM];
                        let h_cur = &h_hist[t][i * OMEGA_DIM..(i + 1) * OMEGA_DIM];
                        let mut dpre = [0.0f64; OMEGA_DIM];
                        for d in 0..OMEGA_DIM {
                            dpre[d] = dz[d] * self.gamma * (1.0 - h_cur[d] * h_cur[d]);
                            g_b_core[d] += dpre[d];
                            dz_anchor[d] += dz[d] * (1.0 - self.gamma);
                        }
                        let mut dz_prev = [0.0f64; OMEGA_DIM];
                        for r in 0..OMEGA_DIM {
                            let dp = dpre[r];
                            let row = r * OMEGA_DIM;
                            for c in 0..OMEGA_DIM {
                                g_w_core[row + c] += dp * z_prev[c];
                                dz_prev[c] += dp * self.w_core[row + c];
                            }
                        }
                        dz = dz_prev;
                    }
                    for d in 0..OMEGA_DIM {
                        dz_anchor[d] += dz[d];
                    }

                    // Backprop into 2-Layer Raw Tropical Deep-Set Encoder (`W_enc2`, `W_enc1`)
                    let cache = &enc_caches[i];
                    let m = cache.h_kept.len();
                    if m > 0 {
                        let inv_m = 1.0 / (m as f64);
                        for s_idx in 0..m {
                            let mut dh = [0.0f64; ENC_HIDDEN];
                            for k in 0..ENC_HIDDEN {
                                dh[k] = dz_anchor[k] * inv_m;
                                if cache.argmax_idx[k] == s_idx {
                                    dh[k] += dz_anchor[ENC_HIDDEN + k];
                                }
                            }
                            let h = &cache.h_kept[s_idx];
                            let a = &cache.a_kept[s_idx];
                            let u = &cache.u_kept[s_idx];

                            let mut da = [0.0f64; ENC_MID];
                            for r in 0..ENC_HIDDEN {
                                let dpre2 = dh[r] * (1.0 - h[r] * h[r]);
                                g_b_enc2[r] += dpre2;
                                let row = r * ENC_MID;
                                for c in 0..ENC_MID {
                                    g_w_enc2[row + c] += dpre2 * a[c];
                                    da[c] += dpre2 * self.w_enc2[row + c];
                                }
                            }
                            for r in 0..ENC_MID {
                                let dpre1 = da[r] * (1.0 - a[r] * a[r]);
                                g_b_enc1[r] += dpre1;
                                let row = r * RAW_FEAT_DIM;
                                for c in 0..RAW_FEAT_DIM {
                                    g_w_enc1[row + c] += dpre1 * u[c];
                                }
                            }
                        }
                    }
                }

                step_counter += 1;
                let bc1 = 1.0 - 0.9f64.powi(step_counter);
                let bc2 = 1.0 - 0.999f64.powi(step_counter);
                let adam_lr = (lr * 0.045) * (0.9992f64).powi(step_counter);
                let wd = 2e-3;

                adamw_update(&mut self.w_op, &g_w_op, &mut m_w_op, &mut v_w_op, adam_lr, wd, bc1, bc2);
                adamw_update(&mut self.b_op, &g_b_op, &mut m_b_op, &mut v_b_op, adam_lr, 0.0, bc1, bc2);
                adamw_update(&mut self.w_next, &g_w_nx, &mut m_w_nx, &mut v_w_nx, adam_lr, wd, bc1, bc2);
                adamw_update(&mut self.b_next, &g_b_nx, &mut m_b_nx, &mut v_b_nx, adam_lr, 0.0, bc1, bc2);
                adamw_update(&mut self.w_var, &g_w_vr, &mut m_w_vr, &mut v_w_vr, adam_lr, wd, bc1, bc2);
                adamw_update(&mut self.b_var, &g_b_vr, &mut m_b_vr, &mut v_b_vr, adam_lr, 0.0, bc1, bc2);
                adamw_update(&mut self.w_val, &g_w_val, &mut m_w_val, &mut v_w_val, adam_lr, wd, bc1, bc2);
                let gv = g_b_val.clamp(-3.0, 3.0);
                m_b_val = 0.9 * m_b_val + 0.1 * gv;
                v_b_val = 0.999 * v_b_val + 0.001 * gv * gv;
                self.b_val -= adam_lr * (m_b_val / bc1) / ((v_b_val / bc2).sqrt() + 1e-8);
                adamw_update(&mut self.w_align, &g_w_align, &mut m_w_align, &mut v_w_align, adam_lr * 0.4, wd, bc1, bc2);
                adamw_update(&mut self.w_need, &g_w_need, &mut m_w_need, &mut v_w_need, adam_lr * 0.5, wd, bc1, bc2);
                adamw_update(&mut self.w_core, &g_w_core, &mut m_w_core, &mut v_w_core, adam_lr * 0.7, wd, bc1, bc2);
                adamw_update(&mut self.b_core, &g_b_core, &mut m_b_core, &mut v_b_core, adam_lr * 0.7, 0.0, bc1, bc2);
                adamw_update(&mut self.w_enc2, &g_w_enc2, &mut m_w_enc2, &mut v_w_enc2, adam_lr * 0.85, wd, bc1, bc2);
                adamw_update(&mut self.b_enc2, &g_b_enc2, &mut m_b_enc2, &mut v_b_enc2, adam_lr * 0.85, 0.0, bc1, bc2);
                adamw_update(&mut self.w_enc1, &g_w_enc1, &mut m_w_enc1, &mut v_w_enc1, adam_lr * 0.85, wd, bc1, bc2);
                adamw_update(&mut self.b_enc1, &g_b_enc1, &mut m_b_enc1, &mut v_b_enc1, adam_lr * 0.85, 0.0, bc1, bc2);
                self.sync_nvfp4_core_weights();
                self.train_steps_completed += 1;
            }

            let avg_loss = ep_loss / (total_n as f64);
            if ep == 0 {
                init_loss = avg_loss;
            }
            final_loss = avg_loss;
            self.last_train_loss = avg_loss;
        }
        (init_loss, final_loss)
    }

    /// Online Modality-Agnostic STE Learning Step (`v9.0 ASI-Core`):
    /// Updates `w_val`, `b_val`, contractive `w_core`/`b_core`, and raw Deep-Set `w_enc2`/`w_enc1`
    /// from ANY `UniversalEventStream` (`CodeFeedback`, `ContinuousCausal`, `SpatialField`, `RawBytes`)
    /// while strictly preserving `||W_core||_inf <= 0.75` and NVFP4 shadow-weight sync!
    pub fn train_universal_stream_value_step(
        &mut self,
        tokens: &[[f64; RAW_FEAT_DIM]],
        target_val: f64,
        lr: f64,
    ) -> f64 {
        let cache = self.encode_raw_tokens_cached(tokens);
        let z0 = cache.z0;

        // 2 contractive equilibrium ticks
        let mut z_cur = z0;
        let mut h1 = [0.0f64; OMEGA_DIM];
        let mut z1 = [0.0f64; OMEGA_DIM];
        for r in 0..OMEGA_DIM {
            let row = r * OMEGA_DIM;
            let mut acc = self.b_core[r];
            for c in 0..OMEGA_DIM {
                acc += self.w_core_q[row + c] * z_cur[c];
            }
            h1[r] = acc.tanh();
            z1[r] = self.gamma * h1[r] + (1.0 - self.gamma) * z0[r];
        }
        z_cur = z1;
        let mut h2 = [0.0f64; OMEGA_DIM];
        let mut zf = [0.0f64; OMEGA_DIM];
        for r in 0..OMEGA_DIM {
            let row = r * OMEGA_DIM;
            let mut acc = self.b_core[r];
            for c in 0..OMEGA_DIM {
                acc += self.w_core_q[row + c] * z_cur[c];
            }
            h2[r] = acc.tanh();
            zf[r] = self.gamma * h2[r] + (1.0 - self.gamma) * z0[r];
        }

        let mut v_logit = self.b_val;
        for d in 0..OMEGA_DIM {
            v_logit += self.w_val[d] * zf[d];
        }
        let vp = 1.0 / (1.0 + (-v_logit.clamp(-10.0, 10.0)).exp());
        let vy = target_val.clamp(0.0, 1.0);
        let loss = -(vy * (vp + 1e-7).ln() + (1.0 - vy) * (1.0 - vp + 1e-7).ln());
        let dvl = (vp - vy).clamp(-1.0, 1.0);

        let mut dz = [0.0f64; OMEGA_DIM];
        self.b_val -= lr * dvl;
        for d in 0..OMEGA_DIM {
            dz[d] = dvl * self.w_val[d];
            self.w_val[d] -= lr * dvl * zf[d];
        }

        // 2-tick BPTT into contractive core (STE)
        let mut dz_anchor = [0.0f64; OMEGA_DIM];
        let ticks_hist = [(&z1, &h2), (&z0, &h1)];
        for &(z_prev, h_cur) in &ticks_hist {
            let mut dpre = [0.0f64; OMEGA_DIM];
            for d in 0..OMEGA_DIM {
                dpre[d] = (dz[d] * self.gamma * (1.0 - h_cur[d] * h_cur[d])).clamp(-1.5, 1.5);
                self.b_core[d] -= (lr * 0.35) * dpre[d];
                dz_anchor[d] += dz[d] * (1.0 - self.gamma);
            }
            let mut dz_prev = [0.0f64; OMEGA_DIM];
            for r in 0..OMEGA_DIM {
                let dp = dpre[r];
                let row = r * OMEGA_DIM;
                for c in 0..OMEGA_DIM {
                    dz_prev[c] += dp * self.w_core[row + c];
                    self.w_core[row + c] -= (lr * 0.35) * dp * z_prev[c];
                }
            }
            dz = dz_prev;
        }
        for d in 0..OMEGA_DIM {
            dz_anchor[d] += dz[d];
        }

        // Backprop into Raw Deep-Set Encoder (`W_enc2`, `W_enc1`)
        let m = cache.h_kept.len();
        if m > 0 {
            let inv_m = 1.0 / (m as f64);
            let enc_lr = lr * 0.45 * inv_m.sqrt();
            for s_idx in 0..m {
                let mut dh = [0.0f64; ENC_HIDDEN];
                for k in 0..ENC_HIDDEN {
                    dh[k] = dz_anchor[k] * inv_m;
                    if cache.argmax_idx[k] == s_idx {
                        dh[k] += dz_anchor[ENC_HIDDEN + k];
                    }
                }
                let h = &cache.h_kept[s_idx];
                let a = &cache.a_kept[s_idx];
                let u = &cache.u_kept[s_idx];
                let mut da = [0.0f64; ENC_MID];
                for r in 0..ENC_HIDDEN {
                    let dpre2 = (dh[r] * (1.0 - h[r] * h[r])).clamp(-1.5, 1.5);
                    self.b_enc2[r] -= enc_lr * dpre2;
                    let row = r * ENC_MID;
                    for c in 0..ENC_MID {
                        da[c] += dpre2 * self.w_enc2[row + c];
                        self.w_enc2[row + c] -= enc_lr * dpre2 * a[c];
                    }
                }
                for r in 0..ENC_MID {
                    let dpre1 = (da[r] * (1.0 - a[r] * a[r])).clamp(-1.5, 1.5);
                    self.b_enc1[r] -= enc_lr * dpre1;
                    let row = r * RAW_FEAT_DIM;
                    for c in 0..RAW_FEAT_DIM {
                        self.w_enc1[row + c] -= enc_lr * dpre1 * u[c];
                    }
                }
            }
        }

        self.sync_nvfp4_core_weights();
        self.train_steps_completed += 1;
        self.last_train_loss = loss;
        loss
    }

    // ========================================================================
    // v6.0 UNIFIED-BRAIN EXTENSIONS:
    //   1. Executable Meta-Priority AST (`meta_priority_ast: OmegaExpr`)
    //   2. Neural Proof-State Tactic Policy (`W_tactic`) + Expert Iteration
    //   3. Shared-Brain Egocentric World Context (`encode_world_core_context`)
    //   4. Full-Brain Bit-Exact SHA-256 Persistence (`save/load_full_brain_checkpoint`)
    // ========================================================================

    /// Execute TropiCore-Omega's OWN self-evolved symbolic priority program `meta_priority_ast`
    /// LIVE inside the Levin search hot-loop (`x0=fit_term, x1=neural_llr, x2=complexity`)!
    #[inline]
    pub fn eval_meta_priority(
        &self,
        fit_term: f64,
        neural_llr: f64,
        complexity_cost: f64,
        cegis_bar: f64,
    ) -> f64 {
        let mut ops = 0usize;
        let v = self
            .meta_priority_ast
            .eval_num(&[fit_term, neural_llr, complexity_cost], None, &EmptyResolver, &mut ops);
        let safe_v = if v.is_finite() { v } else { fit_term + complexity_cost };
        safe_v + 0.06 * cegis_bar
    }

    /// Emit the ACTUAL evolved symbolic AST `self.meta_priority_ast` as standalone compilable Rust code for `rustc -O`!
    pub fn emit_meta_priority_rust_verifier(&self) -> String {
        let body = self.meta_priority_ast.emit_rust_f64(&EmptyResolver);
        format!(
            r#"// TropiCore-Omega v8.0: Autonomously Evolved Koza GP Symbolic AST Search Priority Program (Verified by rustc -O)
// AST: {pretty}
#[allow(unused_variables)]
pub fn omega_meta_priority_ast(x0: f64, x1: f64, x2: f64, cegis_bar: f64) -> f64 {{
    ({body}) + 0.06f64 * cegis_bar
}}

fn main() {{
    let p_good = omega_meta_priority_ast(0.08, -0.60, 0.45, 0.0);
    let p_bad  = omega_meta_priority_ast(0.88,  0.80, 1.50, 1.0);
    assert!(p_good.is_finite() && p_bad.is_finite() && p_good < p_bad, "AST priority monotonicity failed: {{}} vs {{}}", p_good, p_bad);
    println!("SELF_IMPROVED_AST_STRATEGY_RUSTC_PASS");
}}
"#,
            pretty = self.meta_priority_ast.pretty(&EmptyResolver),
            body = body
        )
    }

    /// Shared-Brain Egocentric World Context (`R^8`): encodes the 5x5 egocentric patch via
    /// `UniversalEnergyCore`'s OWN `W_enc` + 1-tick contractive `W_core`!
    pub fn encode_world_core_context(&self, obs: &[f64; WORLD_OBS_DIM]) -> [f64; WORLD_CTX_DIM] {
        let toks = raw_tokens_world_egocentric(obs);
        let z0 = self.encode_raw_tokens(&toks);
        let mut ctx = [0.0f64; WORLD_CTX_DIM];
        for r in 0..WORLD_CTX_DIM {
            let row = r * OMEGA_DIM;
            let mut acc = self.b_core[r];
            for c in 0..OMEGA_DIM {
                acc += self.w_core_q[row + c] * z0[c];
            }
            ctx[r] = (self.gamma * acc.tanh() + (1.0 - self.gamma) * z0[r]).clamp(-1.5, 1.5);
        }
        ctx
    }

    /// Predict live Lean 4 proof-state tactic probabilities `pi_theta(tactic_cls | trace_state)`
    /// through `UniversalEnergyCore`'s unified `W_enc` + `W_core` + live-wired SM120 `omega_tactic_policy_on_device`!
    #[cfg(any(test, feature = "lean_oracle"))]
    pub fn predict_tactic_policy(
        &self,
        hyps: &[(String, String)],
        target: &str,
        def_names: &[String],
        depth: usize,
    ) -> ([f64; NUM_TACTIC_CLASSES], [f64; OMEGA_DIM]) {
        let toks = raw_tokens_proof_state(hyps, target, def_names, depth);
        let z0 = self.encode_raw_tokens(&toks);
        // Fast 2-tick local contractive relaxation through `W_core_q`
        let mut z = z0;
        for _ in 0..2 {
            let mut zn = [0.0f64; OMEGA_DIM];
            for r in 0..OMEGA_DIM {
                let row = r * OMEGA_DIM;
                let mut acc = self.b_core[r];
                for c in 0..OMEGA_DIM {
                    acc += self.w_core_q[row + c] * z[c];
                }
                zn[r] = (self.gamma * acc.tanh() + (1.0 - self.gamma) * z0[r]).clamp(-4.0, 4.0);
            }
            z = zn;
        }
        let probs_v = omega_tactic_policy_on_device(
            &z,
            &self.w_tactic,
            &self.b_tactic,
            NUM_TACTIC_CLASSES,
            OMEGA_DIM,
            depth % 4,
        );
        let mut probs = [1.0 / NUM_TACTIC_CLASSES as f64; NUM_TACTIC_CLASSES];
        probs.copy_from_slice(&probs_v[..NUM_TACTIC_CLASSES]);
        (probs, z)
    }

    /// Online Expert Iteration for the Neural Proof-State Tactic Policy (`W_tactic` + `W_core` + `W_enc`)!
    #[cfg(feature = "lean_oracle")]
    pub fn train_tactic_expert_iteration(
        &mut self,
        traces: &[(Vec<[f64; RAW_FEAT_DIM]>, usize)],
        epochs: usize,
        lr: f64,
    ) -> (f64, f64) {
        if traces.is_empty() {
            return (0.0, 0.0);
        }
        let mut init_loss = 0.0;
        let mut final_loss = 0.0;
        let inv_n = 1.0 / (traces.len() as f64);

        for ep in 0..epochs {
            let mut ep_loss = 0.0;
            let mut g_wt = vec![0.0f64; NUM_TACTIC_CLASSES * OMEGA_DIM];
            let mut g_bt = vec![0.0f64; NUM_TACTIC_CLASSES];
            let mut g_wc = vec![0.0f64; OMEGA_DIM * OMEGA_DIM];

            for (toks, tgt_cls) in traces {
                if *tgt_cls >= NUM_TACTIC_CLASSES || toks.is_empty() {
                    continue;
                }
                let z0 = self.encode_raw_tokens(toks);
                let mut h1 = [0.0f64; OMEGA_DIM];
                let mut z1 = [0.0f64; OMEGA_DIM];
                for r in 0..OMEGA_DIM {
                    let row = r * OMEGA_DIM;
                    let mut acc = self.b_core[r];
                    for c in 0..OMEGA_DIM {
                        acc += self.w_core_q[row + c] * z0[c];
                    }
                    h1[r] = acc.tanh();
                    z1[r] = (self.gamma * h1[r] + (1.0 - self.gamma) * z0[r]).clamp(-4.0, 4.0);
                }
                let mut logits = [0.0f64; NUM_TACTIC_CLASSES];
                let mut mx = f64::NEG_INFINITY;
                for c in 0..NUM_TACTIC_CLASSES {
                    let row = c * OMEGA_DIM;
                    let mut acc = self.b_tactic[c];
                    for d in 0..OMEGA_DIM {
                        acc += self.w_tactic[row + d] * z1[d];
                    }
                    logits[c] = acc.clamp(-10.0, 10.0);
                    if logits[c] > mx {
                        mx = logits[c];
                    }
                }
                let mut ex = [0.0f64; NUM_TACTIC_CLASSES];
                let mut se = 0.0;
                for c in 0..NUM_TACTIC_CLASSES {
                    ex[c] = (logits[c] - mx).exp();
                    se += ex[c];
                }
                let pt = (ex[*tgt_cls] / se.max(1e-9)).max(1e-7);
                ep_loss += -pt.ln();

                let mut dz1 = [0.0f64; OMEGA_DIM];
                for c in 0..NUM_TACTIC_CLASSES {
                    let pc = ex[c] / se.max(1e-9);
                    let yc = if c == *tgt_cls { 1.0 } else { 0.0 };
                    let dl = (pc - yc) * inv_n;
                    g_bt[c] += dl;
                    let row = c * OMEGA_DIM;
                    for d in 0..OMEGA_DIM {
                        g_wt[row + d] += dl * z1[d];
                        dz1[d] += dl * self.w_tactic[row + d];
                    }
                }
                for r in 0..OMEGA_DIM {
                    let dp = dz1[r] * self.gamma * (1.0 - h1[r] * h1[r]);
                    let row = r * OMEGA_DIM;
                    for c in 0..OMEGA_DIM {
                        g_wc[row + c] += dp * z0[c];
                    }
                }
            }
            for k in 0..self.w_tactic.len() {
                self.w_tactic[k] = (self.w_tactic[k] - lr * g_wt[k].clamp(-2.0, 2.0)).clamp(-5.0, 5.0);
            }
            for k in 0..self.b_tactic.len() {
                self.b_tactic[k] = (self.b_tactic[k] - lr * g_bt[k].clamp(-2.0, 2.0)).clamp(-5.0, 5.0);
            }
            for k in 0..self.w_core.len() {
                self.w_core[k] -= (lr * 0.008) * g_wc[k].clamp(-2.0, 2.0);
            }
            self.sync_nvfp4_core_weights();
            let avg = ep_loss * inv_n;
            if ep == 0 {
                init_loss = avg;
            }
            final_loss = avg;
        }
        (init_loss, final_loss)
    }

    /// Save BIT-EXACT FULL-BRAIN Tamper-Evident SHA-256 Checkpoint (`v6.0`):
    /// Persists ALL neural weights (`W_enc`, `W_core`, 7 heads, `world_model`, `meta_priority_ast`,
    /// `strategy`, AND `memory` concepts with full `OmegaExpr` bodies) -> ZERO daemon amnesia!
    pub fn save_full_brain_checkpoint(&self, path: &Path) -> Result<String, String> {
        if let Some(p) = path.parent() {
            let _ = fs::create_dir_all(p);
        }
        let enc_slice = |v: &[f64]| -> String {
            v.iter().map(|x| format!("{:016x}", x.to_bits())).collect::<Vec<_>>().join(",")
        };
        let mut body = String::with_capacity(262_144);
        body.push_str(&format!(
            "TROPICORE_OMEGA_V6_FULL_BRAIN|steps={}|loss={:016x}|rng={:016x}|concepts={}|next_id={}|rules={}\n",
            self.train_steps_completed,
            self.last_train_loss.to_bits(),
            self.rng_state,
            self.memory.concepts.len(),
            self.memory.next_id,
            self.memory.egraph_rules.len(),
        ));
        body.push_str(&format!("META_AST|{}\n", self.meta_priority_ast.to_sexp()));
        let s = &self.strategy;
        body.push_str(&format!(
            "STRAT|{}|{}|{}\n",
            enc_slice(&[s.w_fit, s.w_prior_nll, s.w_next_op, s.w_partner_need, s.w_value, s.w_align, s.w_mdl, s.w_depth, s.neural_temp]),
            s.batch_pop_k,
            s.partner_cap
        ));
        body.push_str(&format!("W|w_enc1|{}\n", enc_slice(&self.w_enc1)));
        body.push_str(&format!("W|b_enc1|{}\n", enc_slice(&self.b_enc1)));
        body.push_str(&format!("W|w_enc2|{}\n", enc_slice(&self.w_enc2)));
        body.push_str(&format!("W|b_enc2|{}\n", enc_slice(&self.b_enc2)));
        body.push_str(&format!("W|w_core|{}\n", enc_slice(&self.w_core)));
        body.push_str(&format!("W|b_core|{}\n", enc_slice(&self.b_core)));
        body.push_str(&format!("W|w_op|{}\n", enc_slice(&self.w_op)));
        body.push_str(&format!("W|b_op|{}\n", enc_slice(&self.b_op)));
        body.push_str(&format!("W|op_base|{}\n", enc_slice(&self.op_base_rate)));
        body.push_str(&format!("W|w_next|{}\n", enc_slice(&self.w_next)));
        body.push_str(&format!("W|b_next|{}\n", enc_slice(&self.b_next)));
        body.push_str(&format!("W|w_var|{}\n", enc_slice(&self.w_var)));
        body.push_str(&format!("W|b_var|{}\n", enc_slice(&self.b_var)));
        body.push_str(&format!("W|var_base|{}\n", enc_slice(&self.var_base_rate)));
        body.push_str(&format!("W|w_val|{}\n", enc_slice(&self.w_val)));
        body.push_str(&format!("W|b_val|{}\n", enc_slice(&[self.b_val])));
        body.push_str(&format!("W|w_align|{}\n", enc_slice(&self.w_align)));
        body.push_str(&format!("W|w_need|{}\n", enc_slice(&self.w_need)));
        body.push_str(&format!("W|w_tactic|{}\n", enc_slice(&self.w_tactic)));
        body.push_str(&format!("W|b_tactic|{}\n", enc_slice(&self.b_tactic)));

        // Persist Action-Conditioned Egocentric World Model weights + Self-Learned Action Lie Algebra + Stats!
        let wm = &self.world_model;
        let act_disp_flat: Vec<f64> = wm.act_disp.iter().flat_map(|p| p.iter().copied()).collect();
        body.push_str(&format!("W|wm_act_disp|{}\n", enc_slice(&act_disp_flat)));
        body.push_str(&format!(
            "WM_META|{}|{:016x}|{:016x}\n",
            wm.updates,
            wm.last_dyn_mse.to_bits(),
            wm.last_rew_mae.to_bits()
        ));
        body.push_str(&format!("W|wm_belief|{}\n", enc_slice(&wm.w_belief)));
        body.push_str(&format!("W|wm_belief_act|{}\n", enc_slice(&wm.w_belief_act)));
        for a in 0..4 {
            body.push_str(&format!("W|wm_d1_{}|{}\n", a, enc_slice(&wm.w_d1[a])));
            body.push_str(&format!("W|wm_bd1_{}|{}\n", a, enc_slice(&wm.b_d1[a])));
            body.push_str(&format!("W|wm_ds_{}|{}\n", a, enc_slice(&wm.w_ds[a])));
            body.push_str(&format!("W|wm_bds_{}|{}\n", a, enc_slice(&wm.b_ds[a])));
            body.push_str(&format!("W|wm_r1_{}|{}\n", a, enc_slice(&wm.w_r1[a])));
            body.push_str(&format!("W|wm_br1_{}|{}\n", a, enc_slice(&wm.b_r1[a])));
            body.push_str(&format!("W|wm_r_{}|{}\n", a, enc_slice(&wm.w_r[a])));
            body.push_str(&format!("W|wm_t_{}|{}\n", a, enc_slice(&wm.w_term[a])));
        }
        body.push_str(&format!("W|wm_br|{}\n", enc_slice(&wm.b_r)));
        body.push_str(&format!("W|wm_bt|{}\n", enc_slice(&wm.b_term)));
        body.push_str(&format!("W|wm_q1|{}\n", enc_slice(&wm.w_q1)));
        body.push_str(&format!("W|wm_bq1|{}\n", enc_slice(&wm.b_q1)));
        body.push_str(&format!("W|wm_q2|{}\n", enc_slice(&wm.w_q2)));
        body.push_str(&format!("W|wm_bq2|{}\n", enc_slice(&wm.b_q2)));

        // Persist ALL Lean-4-Verified E-Graph Rewrite Rules bit-exact (`ZERO` rule amnesia on daemon resume!)
        for rule in &self.memory.egraph_rules {
            body.push_str(&format!(
                "EGRULE|{}|{}|{}|{}\n",
                rule.name,
                if rule.proved_by_lean4 { 1 } else { 0 },
                rule.lhs.to_sexp(),
                rule.rhs.to_sexp()
            ));
        }

        // Persist verified concepts with FULL bit-exact `OmegaExpr` S-expression bodies!
        for c in &self.memory.concepts {
            let sig_s: Vec<String> = c.semantic_sig.iter().map(|v| v.to_string()).collect();
            body.push_str(&format!(
                "CONCEPT|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                c.id,
                c.name,
                c.domain_tag,
                c.arity,
                c.mdl,
                c.uses,
                c.delta_mdl_gain,
                c.body.to_sexp(),
                sig_s.join(","),
                enc_slice(&c.embedding)
            ));
        }

        let digest = sha256_hex(body.as_bytes());
        let full = format!("SHA256:{}\n{}", digest, body);
        fs::write(path, full).map_err(|e| format!("write full brain ckpt failed: {}", e))?;
        Ok(digest)
    }

    /// Load & Verify BIT-EXACT FULL-BRAIN Tamper-Evident SHA-256 Checkpoint (`v6.0`)!
    pub fn load_full_brain_checkpoint(path: &Path) -> Result<(Self, String), String> {
        let raw = fs::read_to_string(path).map_err(|e| format!("read full brain ckpt failed: {}", e))?;
        let (first, rest) = raw.split_once('\n').ok_or("malformed v6 ckpt header")?;
        let expected = first.strip_prefix("SHA256:").ok_or("missing SHA256 prefix")?;
        let actual = sha256_hex(rest.as_bytes());
        if expected != actual {
            return Err(format!("TAMPER_DETECTED: expected {} != actual {}", expected, actual));
        }

        let dec_slice = |s: &str| -> Result<Vec<f64>, String> {
            if s.is_empty() {
                return Ok(Vec::new());
            }
            s.split(',')
                .map(|h| u64::from_str_radix(h, 16).map(f64::from_bits).map_err(|e| e.to_string()))
                .collect()
        };

        let mut core = Self::new(20261125);
        core.memory = UnboundedTropicalMemory::new();
        let mut saw_egrule = false;

        for line in rest.lines() {
            if let Some(hdr) = line.strip_prefix("TROPICORE_OMEGA_V6_FULL_BRAIN|") {
                for kv in hdr.split('|') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k {
                            "steps" => core.train_steps_completed = v.parse().unwrap_or(0),
                            "loss" => {
                                if let Ok(bits) = u64::from_str_radix(v, 16) {
                                    core.last_train_loss = f64::from_bits(bits);
                                }
                            }
                            "rng" => {
                                if let Ok(r) = u64::from_str_radix(v, 16) {
                                    core.rng_state = r;
                                }
                            }
                            "next_id" => core.memory.next_id = v.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
            } else if let Some(wm_meta) = line.strip_prefix("WM_META|") {
                let parts: Vec<&str> = wm_meta.split('|').collect();
                if parts.len() == 3 {
                    core.world_model.updates = parts[0].parse().unwrap_or(0);
                    if let Ok(b1) = u64::from_str_radix(parts[1], 16) {
                        core.world_model.last_dyn_mse = f64::from_bits(b1);
                    }
                    if let Ok(b2) = u64::from_str_radix(parts[2], 16) {
                        core.world_model.last_rew_mae = f64::from_bits(b2);
                    }
                }
            } else if let Some(egline) = line.strip_prefix("EGRULE|") {
                if !saw_egrule {
                    core.memory.egraph_rules.clear();
                    saw_egrule = true;
                }
                let parts: Vec<&str> = egline.split('|').collect();
                if parts.len() == 4 {
                    if let (Some(lhs), Some(rhs)) = (Pattern::from_sexp(parts[2]), Pattern::from_sexp(parts[3])) {
                        core.memory.egraph_rules.push(RewriteRule {
                            name: parts[0].to_string(),
                            lhs,
                            rhs,
                            proved_by_lean4: parts[1] == "1",
                        });
                    }
                }
            } else if let Some(sexp) = line.strip_prefix("META_AST|") {
                if let Some(ast) = OmegaExpr::from_sexp(sexp) {
                    core.meta_priority_ast = ast;
                }
            } else if let Some(sline) = line.strip_prefix("STRAT|") {
                let parts: Vec<&str> = sline.split('|').collect();
                if parts.len() == 3 {
                    let vals = dec_slice(parts[0])?;
                    if vals.len() == 9 {
                        core.strategy.w_fit = vals[0];
                        core.strategy.w_prior_nll = vals[1];
                        core.strategy.w_next_op = vals[2];
                        core.strategy.w_partner_need = vals[3];
                        core.strategy.w_value = vals[4];
                        core.strategy.w_align = vals[5];
                        core.strategy.w_mdl = vals[6];
                        core.strategy.w_depth = vals[7];
                        core.strategy.neural_temp = vals[8];
                    }
                    core.strategy.batch_pop_k = parts[1].parse().unwrap_or(2);
                    core.strategy.partner_cap = parts[2].parse().unwrap_or(12);
                }
            } else if let Some(wline) = line.strip_prefix("W|") {
                if let Some((tag, data)) = wline.split_once('|') {
                    let v = dec_slice(data)?;
                    match tag {
                        "w_enc1" => core.w_enc1 = v,
                        "b_enc1" => core.b_enc1 = v,
                        "w_enc2" => core.w_enc2 = v,
                        "b_enc2" => core.b_enc2 = v,
                        "w_core" => core.w_core = v,
                        "b_core" => core.b_core = v,
                        "w_op" => core.w_op = v,
                        "b_op" => core.b_op = v,
                        "op_base" if v.len() == NUM_OP_CLASSES => core.op_base_rate.copy_from_slice(&v),
                        "w_next" => core.w_next = v,
                        "b_next" => core.b_next = v,
                        "w_var" => core.w_var = v,
                        "b_var" if v.len() == 3 => core.b_var.copy_from_slice(&v),
                        "var_base" if v.len() == 3 => core.var_base_rate.copy_from_slice(&v),
                        "w_val" => core.w_val = v,
                        "b_val" if !v.is_empty() => core.b_val = v[0],
                        "w_align" => core.w_align = v,
                        "w_need" => core.w_need = v,
                        "w_tactic" => core.w_tactic = v,
                        "b_tactic" => core.b_tactic = v,
                        "wm_act_disp" if v.len() == 8 => {
                            for a in 0..4 {
                                core.world_model.act_disp[a] = [v[a * 2], v[a * 2 + 1]];
                            }
                        }
                        "wm_belief" => core.world_model.w_belief = v,
                        "wm_belief_act" => core.world_model.w_belief_act = v,
                        "wm_br" if v.len() == 4 => core.world_model.b_r.copy_from_slice(&v),
                        "wm_bt" if v.len() == 4 => core.world_model.b_term.copy_from_slice(&v),
                        "wm_q1" => { core.world_model.w_q1_tgt = v.clone(); core.world_model.w_q1 = v; }
                        "wm_bq1" => { core.world_model.b_q1_tgt = v.clone(); core.world_model.b_q1 = v; }
                        "wm_q2" => { core.world_model.w_q2_tgt = v.clone(); core.world_model.w_q2 = v; }
                        "wm_bq2" if v.len() == 4 => {
                            core.world_model.b_q2.copy_from_slice(&v);
                            core.world_model.b_q2_tgt.copy_from_slice(&v);
                        }
                        other => {
                            if let Some((prefix, idx_s)) = other.rsplit_once('_') {
                                if let Ok(a) = idx_s.parse::<usize>() {
                                    if a < 4 {
                                        match prefix {
                                            "wm_d1" => core.world_model.w_d1[a] = v,
                                            "wm_bd1" => core.world_model.b_d1[a] = v,
                                            "wm_ds" => core.world_model.w_ds[a] = v,
                                            "wm_bds" => core.world_model.b_ds[a] = v,
                                            "wm_r1" => core.world_model.w_r1[a] = v,
                                            "wm_br1" => core.world_model.b_r1[a] = v,
                                            "wm_r" => core.world_model.w_r[a] = v,
                                            "wm_t" => core.world_model.w_term[a] = v,
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if let Some(cline) = line.strip_prefix("CONCEPT|") {
                let parts: Vec<&str> = cline.split('|').collect();
                if parts.len() == 10 {
                    if let Some(body) = OmegaExpr::from_sexp(parts[7]) {
                        let id: usize = parts[0].parse().unwrap_or(0);
                        let arity: usize = parts[3].parse().unwrap_or(2);
                        let mdl: usize = parts[4].parse().unwrap_or(body.mdl_size());
                        let uses: usize = parts[5].parse().unwrap_or(0);
                        let gain: isize = parts[6].parse().unwrap_or(0);
                        let sig: Vec<i64> = parts[8].split(',').filter_map(|x| x.parse().ok()).collect();
                        let emb_v = dec_slice(parts[9])?;
                        let mut emb = [0.0f64; OMEGA_DIM];
                        if emb_v.len() == OMEGA_DIM {
                            emb.copy_from_slice(&emb_v);
                        }
                        let key = body.canonical_key();
                        let idx = core.memory.concepts.len();
                        core.memory.sig_to_idx.insert(sig.clone(), idx);
                        core.memory.key_to_idx.insert(key.clone(), idx);
                        core.memory.concepts.push(VerifiedConcept {
                            id,
                            name: parts[1].to_string(),
                            domain_tag: parts[2].to_string(),
                            arity,
                            body,
                            canonical_key: key,
                            mdl,
                            semantic_sig: sig,
                            embedding: emb,
                            uses,
                            verified: true,
                            delta_mdl_gain: gain,
                        });
                    }
                }
            }
        }
        core.sync_nvfp4_core_weights();
        Ok((core, actual))
    }
}
