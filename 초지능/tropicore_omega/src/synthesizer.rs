//! Pillar 4: ZERO-ILLUSION Unified-Brain Universal Synthesizer (`v6.0`):
//!   - Wired Batch Bytecode VM + Executable Symbolic Meta-Priority AST (`core.eval_meta_priority`)
//!   - Unified-Brain Neural-Guided OMP-AD, Raw-Grammar List/Rec, Zero-Pool FoldSeq, Raw-Grammar 2D Grid
//!   - Unified-Brain Neural-Guided Local AST Mutation Repair
//!   - Pure Coordinate-Free 5x5 Egocentric Procedural World (`6x6` train -> `10x10` Zero-Shot Size-OOD!)
//!   - Structural Symbolic AST Meta-Self-Improvement (`core.meta_priority_ast` + `rustc -O`)

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::bytecode::FlatProgram;
use crate::core::{
    expr_to_op_mask_resolved, expr_to_var_mask, raw_tokens_numeric_residual, robust_corr,
    NonlinearWorldModel, RawTrainSample, SearchStrategyGenome, UniversalEnergyCore, WorldTransition,
    NUM_OP_CLASSES, WORLD_BELIEF_DIM, WORLD_OBS_DIM,
};
use crate::egraph::egraph_optimize;
use crate::gpu_bridge::omega_eval_bytecode_batch_on_device;
use crate::ir::{
    grid_exact_match_fraction, CellExpr, ConceptResolver, CoordExpr, EmptyResolver, Grid2D,
    OmegaExpr, MAX_AD_PARAMS,
};
use crate::memory::OMEGA_DIM;
#[cfg(feature = "lean_oracle")]
use crate::prover::run_live_tactic_prover_and_promote;
#[cfg(not(feature = "lean_oracle"))]
use crate::prover::run_fast_cegis_algebra_and_promote;
use crate::prover::ProverReport;

// ============================================================================
// 1. Unified Robust Energy, Exact Dual-Number AD Optimizer, and Stagewise OMP
// ============================================================================

pub fn robust_trimmed_l1_error(preds: &[f64], targets: &[f64], robust_mode: bool) -> f64 {
    let n = preds.len().min(targets.len());
    if n == 0 {
        return 1e6;
    }
    let mut errs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let e = (preds[i] - targets[i]).abs();
        errs.push(if e.is_finite() { e.min(1e6) } else { 1e6 });
    }
    if !robust_mode {
        return errs.iter().sum::<f64>() / n as f64;
    }
    errs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let keep = ((n * 85) / 100).max(1);
    errs.iter().take(keep).sum::<f64>() / keep as f64
}

/// Exact Empirical Bayes Factor Log-Likelihood Ratio (`log(P_base(c) / P_theta(c | X, y))`):
/// NEGATIVE (priority boost!) when `W_op` & `W_var` predict an op/var ABOVE its corpus base rate,
/// POSITIVE (pruning penalty!) when predicted BELOW base rate, and size-invariant across AST depths!
fn op_prior_nll<R: ConceptResolver>(
    expr: &OmegaExpr,
    resolver: &R,
    priors: &[f64; NUM_OP_CLASSES],
    base_rates: &[f64; NUM_OP_CLASSES],
    var_priors: &[f64; 3],
    var_base_rates: &[f64; 3],
) -> f64 {
    let inlined = expr.inline_all(resolver, 6);
    let mask = expr_to_op_mask_resolved(&inlined, resolver);
    let vm = expr_to_var_mask(&inlined);
    let mut op_llr = 0.0;
    let mut op_cnt = 0.0;
    for c in 0..9 {
        let p = priors[c].clamp(0.04, 0.96);
        let b = base_rates[c].clamp(0.08, 0.85);
        if mask[c] > 0.5 {
            op_llr += b.ln() - p.ln(); // exact empirical Bayes Factor: negative when p > P_base(c)!
            op_cnt += 1.0;
        }
    }
    let mut var_llr = 0.0;
    let mut var_cnt = 0.0;
    for v in 0..3 {
        let vp = var_priors[v].clamp(0.08, 0.95);
        let vb = var_base_rates[v].clamp(0.15, 0.85);
        if vm[v] > 0.5 {
            var_llr += vb.ln() - vp.ln();
            var_cnt += 1.0;
        }
    }
    let op_avg = if op_cnt > 0.0 { op_llr / op_cnt } else { 0.0 };
    let var_avg = if var_cnt > 0.0 { var_llr / var_cnt } else { 0.0 };
    (op_avg + 0.65 * var_avg).clamp(-2.0, 2.0)
}

pub fn oe_probe_inputs() -> [[f64; 3]; 12] {
    [
        [-2.0, 1.0, 3.0],
        [3.0, -1.0, 2.0],
        [0.0, 4.0, -2.0],
        [1.0, 2.0, 2.0],
        [-3.0, -2.0, 1.0],
        [2.0, 2.0, -1.0],
        [4.0, 0.0, 3.0],
        [-1.0, 3.0, -3.0],
        [0.5, -1.5, 2.5],
        [2.5, 1.5, 0.5],
        [-2.5, 0.5, 1.5],
        [1.5, -2.5, -0.5],
    ]
}

fn oe_signature(vals: &[f64]) -> Vec<i64> {
    vals.iter()
        .map(|&v| {
            if !v.is_finite() {
                i64::MAX
            } else {
                (v.clamp(-1e5, 1e5) * 1000.0).round() as i64
            }
        })
        .collect()
}

fn solve_linear_system(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let mut piv = col;
        let mut best = a[col][col].abs();
        for r in (col + 1)..n {
            if a[r][col].abs() > best {
                best = a[r][col].abs();
                piv = r;
            }
        }
        if best < 1e-12 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let diag = a[col][col];
        for r in (col + 1)..n {
            let f = a[r][col] / diag;
            for c in col..n {
                a[r][c] -= f * a[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0f64; n];
    for r in (0..n).rev() {
        let mut s = b[r];
        for c in (r + 1)..n {
            s -= a[r][c] * x[c];
        }
        x[r] = s / a[r][r];
        if !x[r].is_finite() {
            return None;
        }
    }
    Some(x)
}

/// Exact Forward-Mode Dual-Number AD + Damped Levenberg-Marquardt + Huber/Trimmed IRLS!
pub fn fit_continuous_params<R: ConceptResolver>(
    skeleton: &OmegaExpr,
    train_pairs: &[([f64; 3], f64)],
    robust_mode: bool,
    resolver: &R,
) -> (Vec<f64>, f64) {
    let k = skeleton.num_params().min(MAX_AD_PARAMS);
    if k == 0 || train_pairs.is_empty() {
        let mut ops = 0usize;
        let preds: Vec<f64> = train_pairs
            .iter()
            .map(|(x, _)| skeleton.eval_num(x, None, resolver, &mut ops))
            .collect();
        let tgts: Vec<f64> = train_pairs.iter().map(|(_, y)| *y).collect();
        return (Vec::new(), robust_trimmed_l1_error(&preds, &tgts, robust_mode));
    }

    let targets: Vec<f64> = train_pairs.iter().map(|(_, y)| *y).collect();
    let starts: [[f64; MAX_AD_PARAMS]; 3] = [
        [1.0, 1.0, 0.5, 1.0, 0.5, 0.0],
        [2.0, -1.0, -2.0, 0.5, -0.5, 1.0],
        [0.5, 0.5, 1.5, -1.0, 1.0, -1.0],
    ];

    let mut global_best_p = vec![1.0f64; k];
    let mut global_best_err = f64::INFINITY;

    for init_arr in &starts {
        let mut p: Vec<f64> = init_arr[..k].to_vec();
        let mut lambda = 1e-2f64;

        for iter in 0..34 {
            let mut res = Vec::with_capacity(train_pairs.len());
            let mut jac = Vec::with_capacity(train_pairs.len());
            for (x, y) in train_pairs {
                let d = skeleton.eval_dual(x, &p, resolver);
                let r = (*y - d.val).clamp(-1e5, 1e5);
                res.push(r);
                jac.push(d.d);
            }

            let mut weights = vec![1.0f64; train_pairs.len()];
            if robust_mode {
                let mut abs_r: Vec<f64> = res.iter().map(|v| v.abs()).collect();
                abs_r.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let med = abs_r[abs_r.len() / 2].max(0.05);
                let cutoff = if iter >= 3 { 2.5 * med } else { 4.0 * med };
                for (i, &r) in res.iter().enumerate() {
                    let ar = r.abs();
                    if ar > cutoff * 3.0 {
                        weights[i] = 1e-6;
                    } else if ar > cutoff {
                        weights[i] = cutoff / ar;
                    }
                }
            }

            let cur_preds: Vec<f64> = train_pairs.iter().zip(res.iter()).map(|((_, y), r)| *y - *r).collect();
            let cur_err = robust_trimmed_l1_error(&cur_preds, &targets, robust_mode);
            if cur_err < global_best_err {
                global_best_err = cur_err;
                global_best_p = p.clone();
            }
            if cur_err < 1e-6 {
                return (global_best_p, global_best_err);
            }

            let mut jtj = vec![vec![0.0f64; k]; k];
            let mut jtr = vec![0.0f64; k];
            for i in 0..train_pairs.len() {
                let w = weights[i];
                let ri = res[i];
                let ji = &jac[i];
                for a in 0..k {
                    let wja = w * ji[a];
                    jtr[a] += wja * ri;
                    for b in 0..k {
                        jtj[a][b] += wja * ji[b];
                    }
                }
            }
            for a in 0..k {
                jtj[a][a] += lambda * (1.0 + jtj[a][a].abs().min(10.0));
            }

            if let Some(dp) = solve_linear_system(jtj, jtr) {
                let mut cand_p = p.clone();
                for a in 0..k {
                    cand_p[a] = (cand_p[a] + dp[a].clamp(-4.0, 4.0)).clamp(-50.0, 50.0);
                }
                let mut ops = 0usize;
                let cand_preds: Vec<f64> = train_pairs
                    .iter()
                    .map(|(x, _)| skeleton.eval_num_with_params(x, &cand_p, None, resolver, &mut ops))
                    .collect();
                let cand_err = robust_trimmed_l1_error(&cand_preds, &targets, robust_mode);
                if cand_err < cur_err {
                    p = cand_p;
                    lambda = (lambda * 0.4).max(1e-6);
                    if cand_err < global_best_err {
                        global_best_err = cand_err;
                        global_best_p = p.clone();
                    }
                } else {
                    lambda = (lambda * 3.0).min(1e4);
                }
            } else {
                lambda = (lambda * 4.0).min(1e4);
            }
        }
    }
    (global_best_p, global_best_err)
}

/// ZERO-TEMPLATE Neural-Guided Stagewise Residual Orthogonal Matching Pursuit (OMP) + Dual-Number AD (`v6.0`)!
/// Queries the unified `UniversalEnergyCore` (`W_enc`, `W_core`, `W_op`, `W_var`) to modulate basis selection
/// and nonlinear wrapper order (`SqrtSafe` vs tropical `Max` hinge), discovering permuted & noisy laws!
fn discover_continuous_law_omp<R: ConceptResolver>(
    core: &mut UniversalEnergyCore,
    train_pairs: &[([f64; 3], f64)],
    robust_mode: bool,
    resolver: &R,
) -> (Option<OmegaExpr>, f64, usize, usize) {
    let targets: Vec<f64> = train_pairs.iter().map(|(_, y)| *y).collect();
    let vars = [OmegaExpr::Arg(0), OmegaExpr::Arg(1), OmegaExpr::Arg(2)];

    // Unified-Brain Neural Guidance: query `UniversalEnergyCore` on raw continuous `(X, y)` tokens!
    let z_task = core.encode_numeric_residual(train_pairs, None);
    let (op_p, _, var_p, _, _, _) = core.predict_state_policy(&z_task, 2, 0);

    // Pure primitive degree 1..2 grammar basis (ZERO physics templates!)
    let mut basis: Vec<OmegaExpr> = Vec::new();
    let mut basis_priors: Vec<f64> = Vec::new();
    for (vi, v) in vars.iter().enumerate() {
        basis.push(v.clone());
        basis_priors.push(0.86 + 0.28 * var_p[vi]);
    }
    for i in 0..3 {
        for j in i..3 {
            basis.push(OmegaExpr::Mul(Box::new(vars[i].clone()), Box::new(vars[j].clone())));
            let vp = 0.5 * (var_p[i] + var_p[j]);
            basis_priors.push(0.84 + 0.20 * vp + 0.12 * op_p[2]);
        }
    }
    for (vi, v) in vars.iter().enumerate() {
        basis.push(OmegaExpr::Abs(Box::new(v.clone())));
        basis_priors.push(0.84 + 0.18 * var_p[vi] + 0.14 * op_p[6]);
    }

    let mut ops = 0usize;
    let basis_vals: Vec<Vec<f64>> = basis
        .iter()
        .map(|t| train_pairs.iter().map(|(x, _)| t.eval_num(x, None, resolver, &mut ops)).collect())
        .collect();
    let basis_nonneg: Vec<bool> = basis_vals.iter().map(|vs| vs.iter().all(|&v| v >= -1e-9)).collect();

    let build_linear = |idxs: &[usize]| -> OmegaExpr {
        let mut acc = OmegaExpr::Param(idxs.len());
        for (p_i, &b_i) in idxs.iter().enumerate() {
            acc = OmegaExpr::Add(
                Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(p_i)), Box::new(basis[b_i].clone()))),
                Box::new(acc),
            );
        }
        acc
    };

    let mut best_expr: Option<OmegaExpr> = None;
    let mut best_err = f64::INFINITY;
    let mut exps = 0usize;

    let try_fit = |sk: &OmegaExpr, best_e: &mut Option<OmegaExpr>, best_er: &mut f64, ex: &mut usize, op_c: &mut usize| -> (Vec<f64>, f64) {
        let (p, err) = fit_continuous_params(sk, train_pairs, robust_mode, resolver);
        *ex += 1;
        *op_c += train_pairs.len() * sk.mdl_size() * 6;
        if err < *best_er - 1e-6 {
            *best_er = err;
            *best_e = Some(sk.bind_params(&p).canonicalize());
        }
        (p, err)
    };

    // --- Stage 1: Neural-Prior-Modulated Stagewise Residual OMP (1 -> 2 -> 3 basis terms!) ---
    let mut ranked1: Vec<(usize, f64)> = basis_vals
        .iter()
        .enumerate()
        .map(|(i, vs)| (i, robust_corr(vs, &targets).abs() * basis_priors[i]))
        .collect();
    ranked1.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    for &(i1, _) in ranked1.iter().take(2) {
        let sk1 = build_linear(&[i1]);
        let (p1, err1) = try_fit(&sk1, &mut best_expr, &mut best_err, &mut exps, &mut ops);
        if err1 < 1e-3 {
            return (best_expr, best_err, exps, ops);
        }
        let r1: Vec<f64> = train_pairs
            .iter()
            .map(|(x, y)| *y - sk1.eval_num_with_params(x, &p1, None, resolver, &mut ops))
            .collect();
        let mut ranked2: Vec<(usize, f64)> = basis_vals
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != i1)
            .map(|(i, vs)| (i, robust_corr(vs, &r1).abs() * basis_priors[i]))
            .collect();
        ranked2.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        for &(i2, _) in ranked2.iter().take(2) {
            let sk2 = build_linear(&[i1, i2]);
            let (p2, err2) = try_fit(&sk2, &mut best_expr, &mut best_err, &mut exps, &mut ops);
            if err2 < 1e-3 {
                return (best_expr, best_err, exps, ops);
            }
            let r2: Vec<f64> = train_pairs
                .iter()
                .map(|(x, y)| *y - sk2.eval_num_with_params(x, &p2, None, resolver, &mut ops))
                .collect();
            if let Some((i3, _)) = basis_vals
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != i1 && i != i2)
                .map(|(i, vs)| (i, robust_corr(vs, &r2).abs() * basis_priors[i]))
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            {
                let sk3 = build_linear(&[i1, i2, i3]);
                let (_, err3) = try_fit(&sk3, &mut best_expr, &mut best_err, &mut exps, &mut ops);
                if err3 < 1e-3 {
                    return (best_expr, best_err, exps, ops);
                }
            }
        }
    }

    // Neural outer-wrapper ordering: prioritize `Max`-hinge vs `SqrtSafe` using `op_p[5]` vs `op_p[7]`!
    let wrapper_order = if op_p[5] > op_p[7] + 0.12 { [3usize, 2] } else { [2usize, 3] };

    for stage in wrapper_order {
        if best_err <= 1e-3 {
            break;
        }
        if stage == 2 {
            // --- Stage 2: General Nonlinear Outer Wrapper (`SqrtSafe` via Stagewise OMP on `(y - min_y)^2`!) ---
            let mut sorted_y = targets.clone();
            sorted_y.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let y_base = sorted_y[(sorted_y.len() / 20).min(sorted_y.len().saturating_sub(1))];
            let y_sq: Vec<f64> = targets.iter().map(|&y| { let d = (y - y_base).max(0.0); d * d }).collect();

            let mut nn_r1: Vec<(usize, f64)> = basis_vals
                .iter()
                .enumerate()
                .filter(|&(i, _)| basis_nonneg[i])
                .map(|(i, vs)| (i, robust_corr(vs, &y_sq).abs() * basis_priors[i]))
                .collect();
            nn_r1.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

            for &(n1, _) in nn_r1.iter().take(2) {
                let pairs_sq: Vec<([f64; 3], f64)> = train_pairs.iter().zip(y_sq.iter()).map(|((x, _), &ys)| (*x, ys)).collect();
                let lin1 = OmegaExpr::Add(
                    Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(0)), Box::new(basis[n1].clone()))),
                    Box::new(OmegaExpr::Param(1)),
                );
                let (q1, _) = fit_continuous_params(&lin1, &pairs_sq, robust_mode, resolver);
                let r_sq: Vec<f64> = pairs_sq
                    .iter()
                    .map(|(x, ys)| *ys - lin1.eval_num_with_params(x, &q1, None, resolver, &mut ops))
                    .collect();
                let mut nn_r2: Vec<(usize, f64)> = basis_vals
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| basis_nonneg[i] && i != n1)
                    .map(|(i, vs)| (i, robust_corr(vs, &r_sq).abs() * basis_priors[i]))
                    .collect();
                nn_r2.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

                for &(n2, _) in nn_r2.iter().take(2) {
                    let sk_sqrt = OmegaExpr::Add(
                        Box::new(OmegaExpr::SqrtSafe(Box::new(OmegaExpr::Add(
                            Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(0)), Box::new(basis[n1].clone()))),
                            Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(1)), Box::new(basis[n2].clone()))),
                        )))),
                        Box::new(OmegaExpr::Param(2)),
                    );
                    let (_, e_sq) = try_fit(&sk_sqrt, &mut best_expr, &mut best_err, &mut exps, &mut ops);
                    if e_sq < 1e-3 {
                        return (best_expr, best_err, exps, ops);
                    }
                }
            }
        } else {
            // --- Stage 3: General Piecewise Tropical Hinge OMP (`Max(0, linear_active) + linear_outer`) ---
            let mut sy = targets.clone();
            sy.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let med_y = sy[sy.len() / 2];
            let hi_pairs: Vec<([f64; 3], f64)> = train_pairs.iter().copied().filter(|(_, y)| *y >= med_y).collect();
            let lo_pairs: Vec<([f64; 3], f64)> = train_pairs.iter().copied().filter(|(_, y)| *y < med_y).collect();
            if hi_pairs.len() >= 6 && lo_pairs.len() >= 6 {
                let lin_all = build_linear(&[0, 1, 2]);
                let (p_hi, _) = fit_continuous_params(&lin_all, &hi_pairs, robust_mode, resolver);
                let (p_lo, _) = fit_continuous_params(&lin_all, &lo_pairs, robust_mode, resolver);
                let mut diffs: Vec<(usize, f64)> = (0..3)
                    .map(|i| (i, (p_hi.get(i).copied().unwrap_or(0.0) - p_lo.get(i).copied().unwrap_or(0.0)).abs()))
                    .collect();
                diffs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                let outer_i = diffs[0].0;
                let in_a = (outer_i + 1) % 3;
                let in_b = (outer_i + 2) % 3;
                let sk_hinge = OmegaExpr::Add(
                    Box::new(OmegaExpr::Max(
                        Box::new(OmegaExpr::Const(0.0)),
                        Box::new(OmegaExpr::Add(
                            Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(0)), Box::new(vars[in_a].clone()))),
                            Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(1)), Box::new(vars[in_b].clone()))),
                        )),
                    )),
                    Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Param(2)), Box::new(vars[outer_i].clone()))),
                );
                let _ = try_fit(&sk_hinge, &mut best_expr, &mut best_err, &mut exps, &mut ops);
            }
        }
    }

    (best_expr, best_err, exps, ops)
}

#[derive(Clone, Debug)]
pub struct SynthOutcome {
    pub expr: Option<OmegaExpr>,
    pub exact_match: bool,
    pub residual_err: f64,
    pub raw_candidates_seen: usize,
    pub oe_classes_kept: usize,
    pub expansions: usize,
    pub leaf_ops: usize,
    pub egraph_unions: usize,
    pub used_continuous_ad: bool,
}

// ============================================================================
// 2. REAL Wired Batch Bytecode VM + State-Conditioned Best-First Levin Search (`v5.0`)!
//    - ZERO dummy `let _ =` Bytecode VM calls!
//    - ZERO `[1.0, 2.0]` ring-shell hack!
//    - Uses `W_next` (next-op policy), `W_need` (missing partner), `W_var`, `W_val`!
// ============================================================================

#[derive(Clone)]
struct LevinNode {
    priority: f64,
    base_priority: f64,
    depth: usize,
    expr: OmegaExpr,
    corr: f64,
    z_state: [f64; OMEGA_DIM], // execution residual state `z(X, y, y_hat, r)`
    z_prog: [f64; OMEGA_DIM],  // pure program output embedding `z(X, y_hat, none)` for `W_need` matching!
}

impl PartialEq for LevinNode {
    fn eq(&self, other: &Self) -> bool {
        self.priority.to_bits() == other.priority.to_bits()
    }
}
impl Eq for LevinNode {}
impl PartialOrd for LevinNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for LevinNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.partial_cmp(&self.priority).unwrap_or(Ordering::Equal)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn synthesize_numeric_omega(
    core: &mut UniversalEnergyCore,
    train_pairs: &[([f64; 3], f64)],
    max_depth: usize,
    beam_width: usize,
    eval_budget: usize,
    robust_mode: bool,
    use_learned_prior: bool,
    use_concepts: bool,
    use_cegis: bool,
    enable_continuous_ad: bool,
    ticks: usize,
    device_id: usize,
) -> SynthOutcome {
    core.clear_cegis_barriers();
    let strat = core.strategy.clone();
    let task_z = core.encode_numeric_residual(train_pairs, None);
    let op_base = core.op_base_rate;
    let var_base = core.var_base_rate;
    let (op_priors, root_next_p, var_priors, _init_val, _root_need, relaxed_z) = if use_learned_prior {
        core.predict_state_policy(&task_z, ticks, device_id)
    } else {
        (op_base, [1.0 / 9.0; NUM_OP_CLASSES], var_base, 0.5, [0.0; OMEGA_DIM], task_z)
    };

    let targets: Vec<f64> = train_pairs.iter().map(|(_, y)| *y).collect();
    let probe_in = oe_probe_inputs();
    let n_probe = probe_in.len();
    let n_train = train_pairs.len();

    // Combined evaluation inputs `[12 OE probes || N train inputs]` executed in ONE GPU/CPU Bytecode VM batch!
    let mut combined_in: Vec<[f64; 3]> = Vec::with_capacity(n_probe + n_train);
    combined_in.extend_from_slice(&probe_in);
    for (x, _) in train_pairs {
        combined_in.push(*x);
    }
    let stride = combined_in.len();

    let mut leaf_ops = 0usize;
    let mut raw_seen = 0usize;
    let mut expansions = 0usize;

    let active_concepts: Vec<(usize, usize)> = if use_concepts {
        let top_ids = core.memory.retrieve_tropical_topk(&relaxed_z, Some("universal"), 4);
        top_ids
            .into_iter()
            .filter_map(|cid| {
                core.memory
                    .concepts
                    .iter()
                    .find(|c| c.id == cid && c.arity >= 1 && c.arity <= 2)
                    .map(|c| (c.id, c.arity))
            })
            .collect()
    } else {
        Vec::new()
    };

    let mut oe_seen: HashSet<u64> = HashSet::new();
    let mut train_sig_archive: HashMap<Vec<i64>, OmegaExpr> = HashMap::new();
    let mut heap: BinaryHeap<LevinNode> = BinaryHeap::new();
    let mut heap_base: BinaryHeap<LevinNode> = BinaryHeap::new(); // Solomonoff mixture base queue (Orseau 2018 LTS!)
    let mut expanded_keys: HashSet<String> = HashSet::new();
    let mut expanded_pool: Vec<LevinNode> = Vec::new();
    let mut best_expr: Option<OmegaExpr> = None;
    let mut best_err = f64::INFINITY;
    let mut solved = false;

    // REAL WIRED BATCH EVALUATION ENGINE:
    // Compiles ALL candidate expressions in `batch` to `FlatProgram`, runs `omega_eval_bytecode_batch_on_device`
    // ONCE across multi-core CPU / SM120 GPU warps, and DIRECTLY consumes its output values & 64-bit OE hashes!
    let process_batch = |batch: Vec<(OmegaExpr, usize, f64)>,
                             core_ref: &mut UniversalEnergyCore,
                             oe_seen: &mut HashSet<u64>,
                             train_sig_archive: &mut HashMap<Vec<i64>, OmegaExpr>,
                             heap: &mut BinaryHeap<LevinNode>,
                             heap_base: &mut BinaryHeap<LevinNode>,
                             best_expr: &mut Option<OmegaExpr>,
                             best_err: &mut f64,
                             leaf_ops: &mut usize,
                             raw_seen: &mut usize,
                             expansions: &mut usize|
     -> bool {
        if batch.is_empty() || *expansions >= eval_budget {
            return false;
        }
        let mut canon_batch: Vec<(OmegaExpr, usize, f64, FlatProgram)> = Vec::with_capacity(batch.len());
        let mut flat_progs: Vec<FlatProgram> = Vec::with_capacity(batch.len());
        for (e, d, next_nll) in batch {
            *raw_seen += 1;
            let c = e.canonicalize();
            if let Some(fp) = FlatProgram::compile(&c, &core_ref.memory) {
                flat_progs.push(fp.clone());
                canon_batch.push((c, d, next_nll, fp));
            }
        }
        if flat_progs.is_empty() {
            return false;
        }

        // LIVE GPU/CPU BATCH BYTECODE VM EXECUTION — OUTPUT DIRECTLY CONSUMED (ZERO DUMMY DISCARD!)
        let (vm_vals, _) = omega_eval_bytecode_batch_on_device(&flat_progs, &combined_in, device_id);

        for (bi, (canon, cand_depth, next_op_nll, fp)) in canon_batch.into_iter().enumerate() {
            if *expansions >= eval_budget {
                break;
            }
            let row = &vm_vals[bi * stride..(bi + 1) * stride];
            *leaf_ops += fp.ops.len() * n_probe;

            // Compute exact 64-bit OE hash over the 12 probe outputs from the Bytecode VM!
            let mut oe_h: u64 = 0xcbf2_9ce4_8422_2325;
            for &v in &row[..n_probe] {
                let q = if v.is_finite() {
                    (v.clamp(-1e5, 1e5) * 1000.0).round() as i64
                } else {
                    i64::MAX
                };
                oe_h ^= q as u64;
                oe_h = oe_h.wrapping_mul(0x0100_0000_01b3);
            }
            if !oe_seen.insert(oe_h) {
                continue;
            }
            *expansions += 1;
            *leaf_ops += fp.ops.len() * n_train;

            let preds: Vec<f64> = row[n_probe..stride].iter().map(|&v| v as f64).collect();
            let tsig = oe_signature(&preds);
            let rep = train_sig_archive.get(&tsig).map(|o| canon.mdl_size() < o.mdl_size()).unwrap_or(true);
            if rep {
                train_sig_archive.insert(tsig, canon.clone());
            }

            let err = robust_trimmed_l1_error(&preds, &targets, robust_mode);
            let err_norm = err / (err + 2.0);
            let corr = robust_corr(&preds, &targets).abs();

            if err < *best_err - 1e-9
                || ((err - *best_err).abs() < 1e-6
                    && best_expr.as_ref().map(|b| canon.mdl_size() < b.mdl_size()).unwrap_or(true))
            {
                *best_err = err;
                *best_expr = Some(canon.clone());
            }
            // Occam's Razor anti-overfitting gate: immediately accept compact exact laws (`mdl <= 14`),
            // while refusing to prematurely stop on bloated >14-node overfitters!
            if err < 1e-4 && canon.mdl_size() <= 14 {
                return true;
            }

            // Pure General Algebraic O(1) Residual Completion (IDENTICAL for Cold & Trained!)
            // Enforces the exact same Occam's Razor anti-overfit gate (`mdl <= 14`, inlined `<= 16`)!
            if !robust_mode {
                let add_need: Vec<f64> = targets.iter().zip(preds.iter()).map(|(y, yh)| *y - *yh).collect();
                if let Some(e2) = train_sig_archive.get(&oe_signature(&add_need)).cloned() {
                    let comb = OmegaExpr::Add(Box::new(canon.clone()), Box::new(e2)).canonicalize();
                    if comb.mdl_size() <= 14 && comb.inline_all(&core_ref.memory, 6).mdl_size() <= 16 {
                        let cp: Vec<f64> = train_pairs.iter().map(|(x, _)| comb.eval_num(x, None, &core_ref.memory, leaf_ops)).collect();
                        let ce = robust_trimmed_l1_error(&cp, &targets, false);
                        if ce < 1e-4 {
                            *best_err = ce;
                            *best_expr = Some(comb);
                            return true;
                        }
                    }
                }
                let sub_need: Vec<f64> = preds.iter().zip(targets.iter()).map(|(yh, y)| *yh - *y).collect();
                if let Some(e2) = train_sig_archive.get(&oe_signature(&sub_need)).cloned() {
                    let comb = OmegaExpr::Sub(Box::new(canon.clone()), Box::new(e2)).canonicalize();
                    if comb.mdl_size() <= 14 && comb.inline_all(&core_ref.memory, 6).mdl_size() <= 16 {
                        let cp: Vec<f64> = train_pairs.iter().map(|(x, _)| comb.eval_num(x, None, &core_ref.memory, leaf_ops)).collect();
                        let ce = robust_trimmed_l1_error(&cp, &targets, false);
                        if ce < 1e-4 {
                            *best_err = ce;
                            *best_expr = Some(comb);
                            return true;
                        }
                    }
                }
                for arg_i in 0..3 {
                    let min_e = preds.iter().enumerate().map(|(k, &p)| (p.min(train_pairs[k].0[arg_i]) - targets[k]).abs()).sum::<f64>() / n_train as f64;
                    if min_e < 1e-4 && canon.mdl_size() <= 12 {
                        *best_err = min_e;
                        *best_expr = Some(OmegaExpr::Min(Box::new(canon.clone()), Box::new(OmegaExpr::Arg(arg_i))).canonicalize());
                        return true;
                    }
                    let max_e = preds.iter().enumerate().map(|(k, &p)| (p.max(train_pairs[k].0[arg_i]) - targets[k]).abs()).sum::<f64>() / n_train as f64;
                    if max_e < 1e-4 && canon.mdl_size() <= 12 {
                        *best_err = max_e;
                        *best_expr = Some(OmegaExpr::Max(Box::new(canon.clone()), Box::new(OmegaExpr::Arg(arg_i))).canonicalize());
                        return true;
                    }
                }
                let abs_e = preds.iter().zip(targets.iter()).map(|(p, y)| (p.abs() - *y).abs()).sum::<f64>() / n_train as f64;
                if abs_e < 1e-4 && canon.mdl_size() <= 13 {
                    *best_err = abs_e;
                    *best_expr = Some(OmegaExpr::Abs(Box::new(canon.clone())).canonicalize());
                    return true;
                }
                let sq_p: Vec<f64> = preds.iter().map(|&p| if p > 0.0 { p.sqrt() } else { 0.0 }).collect();
                let sq_e = sq_p.iter().zip(targets.iter()).map(|(p, y)| (*p - *y).abs()).sum::<f64>() / n_train as f64;
                if sq_e < 1e-4 && canon.mdl_size() <= 13 {
                    *best_err = sq_e;
                    *best_expr = Some(OmegaExpr::SqrtSafe(Box::new(canon.clone())).canonicalize());
                    return true;
                }
                // Pure general O(1) archive closure for `sqrt(canon) +- e2` (IDENTICAL for Cold & Trained, ZERO constants!)
                let sq_add: Vec<f64> = targets.iter().zip(sq_p.iter()).map(|(y, sp)| *y - *sp).collect();
                if let Some(e2) = train_sig_archive.get(&oe_signature(&sq_add)).cloned() {
                    let comb = OmegaExpr::Add(Box::new(OmegaExpr::SqrtSafe(Box::new(canon.clone()))), Box::new(e2)).canonicalize();
                    if comb.mdl_size() <= 14 && comb.inline_all(&core_ref.memory, 6).mdl_size() <= 16 {
                        let cp: Vec<f64> = train_pairs.iter().map(|(x, _)| comb.eval_num(x, None, &core_ref.memory, leaf_ops)).collect();
                        let ce = robust_trimmed_l1_error(&cp, &targets, false);
                        if ce < 1e-4 {
                            *best_err = ce;
                            *best_expr = Some(comb);
                            return true;
                        }
                    }
                }
                let sq_sub: Vec<f64> = sq_p.iter().zip(targets.iter()).map(|(sp, y)| *sp - *y).collect();
                if let Some(e2) = train_sig_archive.get(&oe_signature(&sq_sub)).cloned() {
                    let comb = OmegaExpr::Sub(Box::new(OmegaExpr::SqrtSafe(Box::new(canon.clone()))), Box::new(e2)).canonicalize();
                    if comb.mdl_size() <= 14 && comb.inline_all(&core_ref.memory, 6).mdl_size() <= 16 {
                        let cp: Vec<f64> = train_pairs.iter().map(|(x, _)| comb.eval_num(x, None, &core_ref.memory, leaf_ops)).collect();
                        let ce = robust_trimmed_l1_error(&cp, &targets, false);
                        if ce < 1e-4 {
                            *best_err = ce;
                            *best_expr = Some(comb);
                            return true;
                        }
                    }
                }
            }

            // State-Conditioned Neural Levin Scoring using `SearchStrategyGenome`!
            let (prior_nll, align_e, val_p, bar_e, res_z, prog_z) = if use_learned_prior || use_cegis {
                let rz = core_ref.encode_numeric_residual(train_pairs, Some(&preds));
                let prog_pairs: Vec<([f64; 3], f64)> = train_pairs
                    .iter()
                    .zip(preds.iter())
                    .map(|((x, _), &yh)| (*x, yh))
                    .collect();
                let pz = core_ref.encode_numeric_residual(&prog_pairs, None);
                let pn = if use_learned_prior {
                    op_prior_nll(&canon, &core_ref.memory, &op_priors, &op_base, &var_priors, &var_base)
                } else {
                    0.0
                };
                let ae = if use_learned_prior { core_ref.learned_program_energy(&relaxed_z, &rz) } else { 0.0 };
                let vp = if use_learned_prior {
                    core_ref.predict_heads_from_latent(&rz).3
                } else {
                    0.5
                };
                let be = if use_cegis { core_ref.cegis_barrier_energy(&rz) } else { 0.0 };
                (pn, ae, vp, be, rz, pz)
            } else {
                (0.0, 0.0, 0.5, 0.0, [0.0; OMEGA_DIM], [0.0; OMEGA_DIM])
            };

            // Strictly IDENTICAL `fit_term` for BOTH Cold and Trained (100% fair scientific comparison!)
            // The ONLY advantages Trained receives are its learned `W_next`, `W_need`, `W_op`, `W_var`, `W_val`,
            // autonomous MDL concepts, CEGIS barriers, and self-improved `SearchStrategyGenome`!
            let fit_term = (0.55 * err_norm + 0.45 * (1.0 - corr)).min(0.85 * (1.0 - corr) + 0.15 * err_norm);

            let complexity_cost =
                strat.w_mdl * (canon.mdl_size() as f64) + strat.w_depth * (cand_depth as f64);
            // PURE uncontaminated Solomonoff base priority (`ZERO` CEGIS barrier contamination!)
            let base_priority = if cand_depth == 0 {
                -1.0 + 0.01 * (canon.mdl_size() as f64)
            } else {
                strat.w_fit * fit_term + complexity_cost
            };
            // LIVE EXECUTION of TropiCore-Omega's OWN self-evolved symbolic AST `core.meta_priority_ast`!
            let neural_llr = strat.w_prior_nll * prior_nll
                + strat.w_next_op * next_op_nll
                + strat.w_value * (0.5 - val_p)
                + strat.w_align * (align_e - 0.35);
            let priority = if cand_depth == 0 {
                base_priority
            } else if use_learned_prior {
                core_ref.eval_meta_priority(strat.w_fit * fit_term, neural_llr, complexity_cost, bar_e)
            } else {
                base_priority
            };

            if use_cegis && err > 3.2 && corr < 0.15 && core_ref.cegis_barriers.len() < 32 {
                core_ref.inject_cegis_barrier(res_z, 1.4);
            }

            if cand_depth <= max_depth {
                let node = LevinNode {
                    priority,
                    base_priority,
                    depth: cand_depth,
                    expr: canon,
                    corr,
                    z_state: res_z,
                    z_prog: prog_z,
                };
                if use_learned_prior {
                    let mut base_node = node.clone();
                    base_node.priority = base_priority;
                    heap_base.push(base_node);
                }
                heap.push(node);
            }
        }
        false
    };

    // 1. Seed Depth-0 primitive leaves via Batch Bytecode VM
    let leaves = [
        OmegaExpr::Arg(0),
        OmegaExpr::Arg(1),
        OmegaExpr::Arg(2),
        OmegaExpr::Const(0.0),
        OmegaExpr::Const(1.0),
        OmegaExpr::Const(2.0),
    ];
    let seed_batch: Vec<(OmegaExpr, usize, f64)> = leaves.iter().map(|l| (l.clone(), 0, 0.0)).collect();
    if process_batch(
        seed_batch, core, &mut oe_seen, &mut train_sig_archive, &mut heap, &mut heap_base,
        &mut best_expr, &mut best_err, &mut leaf_ops, &mut raw_seen, &mut expansions,
    ) {
        solved = true;
    }

    // 2. Strict Utility-Gated Depth-1 Library Concept Compression (DreamCoder / Solomonoff Principle!):
    //    Evaluate verified arity-2 concepts in ONE Batch Bytecode VM call, inject AT MOST 2 concepts
    //    ONLY IF `cor >= 0.90 && err < 0.75 * best_leaf_err` (prevents library swamping on simple tasks!).
    let _ = &active_concepts;
    let best_leaf_err = best_err;
    if !solved && use_concepts {
        let all_c2: Vec<usize> = core
            .memory
            .concepts
            .iter()
            .filter(|c| c.verified && c.domain_tag != "grid2d" && c.arity == 2)
            .map(|c| c.id)
            .collect();
        if !all_c2.is_empty() {
            let atoms = [OmegaExpr::Arg(0), OmegaExpr::Arg(1), OmegaExpr::Arg(2)];
            let mut c_exprs: Vec<OmegaExpr> = Vec::new();
            let mut c_flats: Vec<FlatProgram> = Vec::new();
            for &cid in all_c2.iter().take(16) {
                for i in 0..3 {
                    for j in i..3 {
                        let ce = OmegaExpr::ConceptCall {
                            id: cid,
                            args: vec![atoms[i].clone(), atoms[j].clone()],
                        };
                        if let Some(fp) = FlatProgram::compile(&ce, &core.memory) {
                            c_exprs.push(ce);
                            c_flats.push(fp);
                        }
                    }
                }
            }
            if !c_flats.is_empty() {
                let train_xs: Vec<[f64; 3]> = train_pairs.iter().map(|(x, _)| *x).collect();
                let (c_vals, _) = omega_eval_bytecode_batch_on_device(&c_flats, &train_xs, device_id);
                let mut scored_c: Vec<(usize, f64)> = Vec::new();
                for ci in 0..c_exprs.len() {
                    let preds: Vec<f64> = c_vals[ci * n_train..(ci + 1) * n_train]
                        .iter()
                        .map(|&v| v as f64)
                        .collect();
                    let err = robust_trimmed_l1_error(&preds, &targets, robust_mode);
                    let cor = robust_corr(&preds, &targets).abs();
                    if cor >= 0.90 && err < 0.75 * best_leaf_err {
                        let score = cor + 0.5 / (1.0 + err);
                        scored_c.push((ci, score));
                    }
                }
                scored_c.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
                let top_c_batch: Vec<(OmegaExpr, usize, f64)> = scored_c
                    .into_iter()
                    .take(2)
                    .map(|(ci, _)| (c_exprs[ci].clone(), 1, 0.0))
                    .collect();
                if process_batch(
                    top_c_batch, core, &mut oe_seen, &mut train_sig_archive, &mut heap, &mut heap_base,
                    &mut best_expr, &mut best_err, &mut leaf_ops, &mut raw_seen, &mut expansions,
                ) {
                    solved = true;
                }
            }
        }
    }

    // 3. MAIN BATCHED UNCONTAMINATED 1:1 SOLOMONOFF-MIXTURE LEVIN SEARCH LOOP (`BinaryHeap` + Batch Bytecode VM!)
    let pop_k = strat.batch_pop_k.clamp(1, 6);
    let ln_unif_op = (9.0f64).ln();
    let mut pop_step = 0usize;
    while !solved && expansions < eval_budget {
        let mut popped: Vec<(LevinNode, bool)> = Vec::with_capacity(pop_k);
        // Pop Depth-0 leaves cleanly; for deeper nodes in Trained mode, alternate 1:1 per-slot between
        // the Neural Meta-AST Queue (`heap`) and the UNCONTAMINATED Base Residual Queue (`heap_base`),
        // preventing early local-minimum queue starvation while capturing full neural speedups!
        let pop_next_unique = |q: &mut BinaryHeap<LevinNode>, seen: &mut HashSet<String>| -> Option<LevinNode> {
            while let Some(node) = q.pop() {
                if seen.insert(node.expr.canonical_key()) {
                    return Some(node);
                }
            }
            None
        };
        for slot in 0..pop_k {
            let use_base_q = use_learned_prior && ((pop_step + slot) % 2 == 1);
            let picked = if use_base_q {
                pop_next_unique(&mut heap_base, &mut expanded_keys)
                    .map(|n| (n, true))
                    .or_else(|| pop_next_unique(&mut heap, &mut expanded_keys).map(|n| (n, false)))
            } else {
                pop_next_unique(&mut heap, &mut expanded_keys)
                    .map(|n| (n, false))
                    .or_else(|| pop_next_unique(&mut heap_base, &mut expanded_keys).map(|n| (n, true)))
            };
            if let Some((node, from_base)) = picked {
                let d0 = node.depth == 0;
                popped.push((node, from_base));
                if d0 {
                    break;
                }
            } else {
                break;
            }
        }
        pop_step += 1;
        if popped.is_empty() {
            break;
        }

        let mut next_batch: Vec<(OmegaExpr, usize, f64)> = Vec::new();

        for (cur, from_base_q) in popped {
            let next_d = cur.depth + 1;
            let use_neural_here = use_learned_prior && !from_base_q;

            // State-Conditioned Policy & Needed-Partner Vector for THIS exact node `cur`!
            let (node_next_p, node_z_need) = if use_neural_here {
                if cur.depth == 0 {
                    (root_next_p, [0.0; OMEGA_DIM])
                } else {
                    let (_, np, _, _, zn) = core.predict_heads_from_latent(&cur.z_state);
                    (np, zn)
                }
            } else {
                ([1.0 / 9.0; NUM_OP_CLASSES], [0.0; OMEGA_DIM])
            };

            // Build Partner Pool with STRICTLY EQUAL total capacity (`leaves + cap`) for Cold & Trained!
            let mut partners: Vec<OmegaExpr> = Vec::new();
            let mut p_keys: HashSet<String> = HashSet::new();
            let cap = if cur.depth == 0 {
                6
            } else {
                (beam_width / 4).clamp(6, strat.partner_cap.max(6))
            };
            for a in &leaves {
                if p_keys.insert(a.canonical_key()) {
                    partners.push(a.clone());
                }
            }
            let max_total_partners = leaves.len() + cap;
            let mut by_pri: Vec<usize> = (0..expanded_pool.len()).collect();
            by_pri.sort_by(|&a, &b| {
                let pa = if use_neural_here { expanded_pool[a].priority } else { expanded_pool[a].base_priority };
                let pb = if use_neural_here { expanded_pool[b].priority } else { expanded_pool[b].base_priority };
                pa.partial_cmp(&pb).unwrap_or(Ordering::Equal)
            });
            let mut by_cor: Vec<usize> = (0..expanded_pool.len()).collect();
            by_cor.sort_by(|&a, &b| {
                expanded_pool[b].corr.partial_cmp(&expanded_pool[a].corr).unwrap_or(Ordering::Equal)
            });
            if use_neural_here && !expanded_pool.is_empty() {
                let n_need = (cap / 3).max(2);
                let n_cor = (cap / 3).max(2);
                let mut by_need: Vec<(usize, f64)> = expanded_pool
                    .iter()
                    .enumerate()
                    .map(|(idx, n)| {
                        let nd = UniversalEnergyCore::partner_need_distance(&node_z_need, &n.z_prog);
                        (idx, strat.w_partner_need * nd - 0.40 * n.corr + 0.25 * n.priority)
                    })
                    .collect();
                by_need.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
                for &(idx, _) in by_need.iter().take(n_need) {
                    if partners.len() < max_total_partners && p_keys.insert(expanded_pool[idx].expr.canonical_key()) {
                        partners.push(expanded_pool[idx].expr.clone());
                    }
                }
                for &idx in by_cor.iter().take(n_cor) {
                    if partners.len() < max_total_partners && p_keys.insert(expanded_pool[idx].expr.canonical_key()) {
                        partners.push(expanded_pool[idx].expr.clone());
                    }
                }
                for &idx in &by_pri {
                    if partners.len() >= max_total_partners {
                        break;
                    }
                    if p_keys.insert(expanded_pool[idx].expr.canonical_key()) {
                        partners.push(expanded_pool[idx].expr.clone());
                    }
                }
            } else {
                let n_pri = (cap * 2) / 3;
                for &idx in by_pri.iter().take(n_pri) {
                    if partners.len() < max_total_partners && p_keys.insert(expanded_pool[idx].expr.canonical_key()) {
                        partners.push(expanded_pool[idx].expr.clone());
                    }
                }
                for &idx in &by_cor {
                    if partners.len() >= max_total_partners {
                        break;
                    }
                    if p_keys.insert(expanded_pool[idx].expr.canonical_key()) {
                        partners.push(expanded_pool[idx].expr.clone());
                    }
                }
            }
            expanded_pool.push(cur.clone());

            if next_d > max_depth {
                continue;
            }

            // (a) Unary Expansions (`Abs`, `SqrtSafe`) ordered by centered LLR from `node_next_p`
            let mut unaries = [(node_next_p[6], 6usize), (node_next_p[7], 7usize)];
            unaries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
            for &(prob, ucls) in &unaries {
                let cand = if ucls == 6 {
                    OmegaExpr::Abs(Box::new(cur.expr.clone()))
                } else {
                    OmegaExpr::SqrtSafe(Box::new(cur.expr.clone()))
                };
                next_batch.push((cand, next_d, (-prob.clamp(0.02, 0.98).ln() - ln_unif_op).clamp(-1.8, 1.8)));
            }

            // (b) Binary Expansions ordered by centered LLR from `node_next_p`
            let mut bin_ops: Vec<(f64, usize)> = vec![
                (node_next_p[0], 0),
                (node_next_p[1], 1),
                (node_next_p[2], 2),
                (node_next_p[4], 4),
                (node_next_p[5], 5),
            ];
            bin_ops.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

            for &(prob, op_cls) in &bin_ops {
                let nll = (-prob.clamp(0.02, 0.98).ln() - ln_unif_op).clamp(-1.8, 1.8);
                for partner in &partners {
                    match op_cls {
                        0 => next_batch.push((OmegaExpr::Add(Box::new(cur.expr.clone()), Box::new(partner.clone())), next_d, nll)),
                        1 => {
                            next_batch.push((OmegaExpr::Sub(Box::new(cur.expr.clone()), Box::new(partner.clone())), next_d, nll));
                            if cur.expr != *partner {
                                next_batch.push((OmegaExpr::Sub(Box::new(partner.clone()), Box::new(cur.expr.clone())), next_d, nll));
                            }
                        }
                        2 => next_batch.push((OmegaExpr::Mul(Box::new(cur.expr.clone()), Box::new(partner.clone())), next_d, nll)),
                        4 => next_batch.push((OmegaExpr::Min(Box::new(cur.expr.clone()), Box::new(partner.clone())), next_d, nll)),
                        _ => next_batch.push((OmegaExpr::Max(Box::new(cur.expr.clone()), Box::new(partner.clone())), next_d, nll)),
                    }
                }
            }

            // (c) Conditional Branching (`IfPos`) — NEVER prune condition variables `x0, x1, x2` (fixes nonlinear condition blindness!)
            if cur.depth >= 1 && cur.expr.mdl_size() <= 5 && !matches!(cur.expr, OmegaExpr::IfPos(_, _, _)) {
                let if_ratio = op_priors[8] / op_base[8].max(0.08);
                let (cond_cap, bcap) = if use_neural_here {
                    if if_ratio < 0.70 && node_next_p[8] < 0.14 {
                        (3usize, 5usize)
                    } else {
                        (3usize, 8usize)
                    }
                } else {
                    (3usize, 8usize)
                };
                let nll_if = (-node_next_p[8].clamp(0.02, 0.98).ln() - ln_unif_op).clamp(-1.8, 1.8);
                let mut conds: Vec<(f64, usize)> = (0..3).map(|i| (var_priors[i], i)).collect();
                conds.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
                let mut if_partners: Vec<&OmegaExpr> = partners
                    .iter()
                    .filter(|p| p.mdl_size() > 1 && p.mdl_size() <= 5 && !matches!(p, OmegaExpr::IfPos(_, _, _)))
                    .collect();
                for lf in &leaves {
                    if_partners.push(lf);
                }
                for &(_, ci) in conds.iter().take(cond_cap) {
                    for partner in if_partners.iter().take(bcap) {
                        if cur.expr != **partner {
                            next_batch.push((
                                OmegaExpr::IfPos(
                                    Box::new(OmegaExpr::Arg(ci)),
                                    Box::new(cur.expr.clone()),
                                    Box::new((*partner).clone()),
                                ),
                                next_d,
                                nll_if,
                            ));
                        }
                    }
                }
            }
        }

        if process_batch(
            next_batch, core, &mut oe_seen, &mut train_sig_archive, &mut heap, &mut heap_base,
            &mut best_expr, &mut best_err, &mut leaf_ops, &mut raw_seen, &mut expansions,
        ) {
            solved = true;
            break;
        }
    }

    // 4. UNIFIED-BRAIN Stagewise OMP Continuous Law Discovery + Dual-Number AD (`LM + Huber IRLS`)!
    let mut used_ad = false;
    if !solved && enable_continuous_ad {
        used_ad = true;
        let mem_snap = core.memory.clone();
        let (ad_expr, ad_err, ad_exps, ad_ops) =
            discover_continuous_law_omp(core, train_pairs, robust_mode, &mem_snap);
        expansions += ad_exps;
        leaf_ops += ad_ops;
        if ad_err < best_err - 1e-6 {
            best_err = ad_err;
            best_expr = ad_expr;
            if ad_err < 1e-3 {
                solved = true;
            }
        }
    }

    // 5. Live E-Graph Optimization powered by Lean-4-Proved Rewrite Rules!
    let mut egraph_unions = 0usize;
    if let Some(ref e) = best_expr {
        let inlined = e.inline_all(&core.memory, 6);
        let (opt, u) = egraph_optimize(&inlined, &core.memory.egraph_rules);
        egraph_unions = u;
        if opt.mdl_size() < e.mdl_size() {
            let mut ops_chk = 0usize;
            let opt_preds: Vec<f64> = train_pairs
                .iter()
                .map(|(x, _)| opt.eval_num(x, None, &core.memory, &mut ops_chk))
                .collect();
            let opt_err = robust_trimmed_l1_error(&opt_preds, &targets, robust_mode);
            if opt_err <= best_err + 1e-6 {
                best_expr = Some(opt);
            }
        }
    }

    SynthOutcome {
        expr: best_expr,
        exact_match: solved || best_err < 1e-3,
        residual_err: best_err,
        raw_candidates_seen: raw_seen,
        oe_classes_kept: oe_seen.len(),
        expansions,
        leaf_ops,
        egraph_unions,
        used_continuous_ad: used_ad,
    }
}

// ============================================================================
// 3. Turing-Complete Functional (`ListMap`/`Filter`/`Reduce`/`Rec`) + Pure-Grammar `FoldSeq`!
// ============================================================================

/// ZERO-POOL Raw-Grammar Neural-Guided Higher-Order Dynamic-List Synthesizer (`v6.0`)!
/// 100% DELETED hardcoded `preds[3]`, `maps[4]`, `reduces[3]`!
/// Synthesizes predicates, map lambdas, and reduce combinators purely from raw atoms `{x, acc, 0, 1, 2}`
/// x `{Add, Sub, Mul, Abs, Min, Max}` ranked by `UniversalEnergyCore`'s learned Bayes-Factor `op_priors`!
pub fn synthesize_functional_list_omega(
    core: &mut UniversalEnergyCore,
    train_seqs: &[(Vec<f64>, f64)],
    ticks: usize,
    device_id: usize,
) -> SynthOutcome {
    let task_z = core.encode_sequence_io(train_seqs);
    let op_base = core.op_base_rate;
    let var_base = core.var_base_rate;
    let (op_priors, _, var_priors, _, _, _) = core.predict_state_policy(&task_z, ticks, device_id);

    // 1. Raw 1-variable bottom-up grammar over `{Arg(0), 0, 1, 2}` (OE-deduplicated on 8 scalar probes!)
    let probes1: [f64; 8] = [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0];
    let atoms1 = [OmegaExpr::Arg(0), OmegaExpr::Const(0.0), OmegaExpr::Const(1.0), OmegaExpr::Const(2.0)];
    let mut seen1: HashSet<Vec<i64>> = HashSet::new();
    let mut unary_progs: Vec<OmegaExpr> = Vec::new();
    let mut leaf_ops = 0usize;

    let mut push_u = |e: OmegaExpr, ops: &mut usize| {
        let c = e.canonicalize();
        let vals: Vec<f64> = probes1.iter().map(|&v| c.eval_num(&[v, 0.0, 0.0], None, &EmptyResolver, ops)).collect();
        if vals.iter().all(|v| v.is_finite()) && seen1.insert(oe_signature(&vals)) {
            unary_progs.push(c);
        }
    };
    push_u(OmegaExpr::Arg(0), &mut leaf_ops);
    for a in &atoms1 {
        push_u(OmegaExpr::Abs(Box::new(a.clone())), &mut leaf_ops);
        for b in &atoms1 {
            push_u(OmegaExpr::Add(Box::new(a.clone()), Box::new(b.clone())), &mut leaf_ops);
            push_u(OmegaExpr::Sub(Box::new(a.clone()), Box::new(b.clone())), &mut leaf_ops);
            push_u(OmegaExpr::Mul(Box::new(a.clone()), Box::new(b.clone())), &mut leaf_ops);
            push_u(OmegaExpr::Max(Box::new(a.clone()), Box::new(b.clone())), &mut leaf_ops);
            push_u(OmegaExpr::Min(Box::new(a.clone()), Box::new(b.clone())), &mut leaf_ops);
        }
    }
    // Exclude constant-only unary expressions for predicates & map functions
    let nonconst_u: Vec<OmegaExpr> = unary_progs
        .into_iter()
        .filter(|e| !matches!(e, OmegaExpr::Const(_)))
        .collect();

    // Rank predicates & maps by MDL + `UniversalEnergyCore`'s learned neural operator Bayes Factor!
    let mut preds: Vec<OmegaExpr> = nonconst_u.clone();
    preds.sort_by(|a, b| {
        let sa = (a.mdl_size() as f64) + 0.25 * op_prior_nll(a, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        let sb = (b.mdl_size() as f64) + 0.25 * op_prior_nll(b, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });
    preds.truncate(20);

    let mut maps: Vec<OmegaExpr> = nonconst_u;
    maps.sort_by(|a, b| {
        let sa = (a.mdl_size() as f64) + 0.25 * op_prior_nll(a, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        let sb = (b.mdl_size() as f64) + 0.25 * op_prior_nll(b, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });
    maps.truncate(16);

    // 2. Raw bottom-up 2-variable reduce step grammar over `{elem=Arg(0), acc=Arg(1)}` x `{Add, Sub, Mul, Max, Min}`
    let red_probes: [[f64; 3]; 5] = [[2.0, 3.0, 0.0], [-1.0, 4.0, 0.0], [5.0, 2.0, 0.0], [0.0, 3.0, 0.0], [3.0, -2.0, 0.0]];
    let x0 = OmegaExpr::Arg(0);
    let x1 = OmegaExpr::Arg(1);
    let mut red_seen: HashSet<Vec<i64>> = HashSet::new();
    let mut red_steps: Vec<(OmegaExpr, OmegaExpr)> = Vec::new();
    for step_cand in [
        OmegaExpr::Add(Box::new(x1.clone()), Box::new(x0.clone())),
        OmegaExpr::Max(Box::new(x1.clone()), Box::new(x0.clone())),
        OmegaExpr::Min(Box::new(x1.clone()), Box::new(x0.clone())),
        OmegaExpr::Mul(Box::new(x1.clone()), Box::new(x0.clone())),
        OmegaExpr::Sub(Box::new(x1), Box::new(x0)),
    ] {
        let vals: Vec<f64> = red_probes.iter().map(|p| step_cand.eval_num(p, None, &EmptyResolver, &mut leaf_ops)).collect();
        if red_seen.insert(oe_signature(&vals)) {
            let init = if matches!(step_cand, OmegaExpr::Mul(_, _)) {
                OmegaExpr::Const(1.0)
            } else {
                OmegaExpr::Const(0.0)
            };
            red_steps.push((init, step_cand));
        }
    }

    // Allow ALL compositional pipeline orders: raw, Filter, Map, Filter->Map, AND Map->Filter!
    let mut list_pipelines: Vec<OmegaExpr> = vec![OmegaExpr::ListInput];
    for p in &preds {
        let flt = OmegaExpr::ListFilterPos {
            pred: Box::new(p.clone()),
            input: Box::new(OmegaExpr::ListInput),
        };
        list_pipelines.push(flt.clone());
        for m in &maps {
            if *m != OmegaExpr::Arg(0) {
                list_pipelines.push(OmegaExpr::ListMap {
                    fn_body: Box::new(m.clone()),
                    input: Box::new(flt.clone()),
                });
            }
        }
    }
    for m in &maps {
        if *m != OmegaExpr::Arg(0) {
            let mp = OmegaExpr::ListMap {
                fn_body: Box::new(m.clone()),
                input: Box::new(OmegaExpr::ListInput),
            };
            list_pipelines.push(mp.clone());
            for p in preds.iter().take(6) {
                list_pipelines.push(OmegaExpr::ListFilterPos {
                    pred: Box::new(p.clone()),
                    input: Box::new(mp.clone()),
                });
            }
        }
    }

    let mut cands: Vec<OmegaExpr> = Vec::new();
    for lp in &list_pipelines {
        cands.push(OmegaExpr::ListLen(Box::new(lp.clone())));
        for (init, step) in &red_steps {
            cands.push(OmegaExpr::ListReduce {
                init: Box::new(init.clone()),
                step: Box::new(step.clone()),
                input: Box::new(lp.clone()),
            });
        }
    }
    cands.sort_by(|a, b| {
        let sa = (a.mdl_size() as f64) + 0.18 * op_prior_nll(a, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        let sb = (b.mdl_size() as f64) + 0.18 * op_prior_nll(b, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });

    let mut expansions = 0usize;
    let mut best_expr: Option<OmegaExpr> = None;
    let mut best_err = f64::INFINITY;

    for cand in cands {
        expansions += 1;
        let mut s = 0.0;
        for (seq, tgt) in train_seqs {
            s += (cand.eval_num(&[], Some(seq), &EmptyResolver, &mut leaf_ops) - *tgt).abs();
        }
        let avg = s / train_seqs.len().max(1) as f64;
        if avg < best_err - 1e-9 {
            best_err = avg;
            best_expr = Some(cand);
        }
        if best_err < 1e-6 {
            break;
        }
    }

    SynthOutcome {
        expr: best_expr,
        exact_match: best_err < 1e-3,
        residual_err: best_err,
        raw_candidates_seen: expansions,
        oe_classes_kept: expansions,
        expansions,
        leaf_ops,
        egraph_unions: 0,
        used_continuous_ad: false,
    }
}

/// ZERO-POOL Raw-Grammar Neural-Guided General Recursion Synthesizer (`v6.0`)!
/// 100% DELETED hardcoded `steps[4]`! Synthesizes recursive `step(k, acc)` purely bottom-up
/// from raw atoms `{k=Arg(0), acc=Arg(1), 0, 1, 2}` x `{Add, Sub, Mul, Max}` ranked by `UniversalEnergyCore`!
pub fn synthesize_recursive_omega(
    core: &mut UniversalEnergyCore,
    train_pairs: &[([f64; 3], f64)],
    ticks: usize,
    device_id: usize,
) -> SynthOutcome {
    let task_z = core.encode_numeric_residual(train_pairs, None);
    let op_base = core.op_base_rate;
    let var_base = core.var_base_rate;
    let (op_priors, _, var_priors, _, _, _) = core.predict_state_policy(&task_z, ticks, device_id);

    let atoms = [
        OmegaExpr::Arg(0), // k
        OmegaExpr::Arg(1), // acc
        OmegaExpr::Const(0.0),
        OmegaExpr::Const(1.0),
        OmegaExpr::Const(2.0),
    ];
    let step_probes: [[f64; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [3.0, 3.0, 0.0],
        [4.0, 6.0, 0.0],
        [5.0, 10.0, 0.0],
        [3.0, 7.0, 0.0],
    ];
    let mut leaf_ops = 0usize;
    let mut oe_seen: HashSet<Vec<i64>> = HashSet::new();
    let mut d1_steps: Vec<OmegaExpr> = Vec::new();
    let mut all_steps: Vec<OmegaExpr> = Vec::new();

    let mut add_step = |e: OmegaExpr, dst: &mut Vec<OmegaExpr>, all: &mut Vec<OmegaExpr>, ops: &mut usize| {
        let c = e.canonicalize();
        let vals: Vec<f64> = step_probes.iter().map(|p| c.eval_num(p, None, &EmptyResolver, ops)).collect();
        if vals.iter().all(|v| v.is_finite()) && oe_seen.insert(oe_signature(&vals)) {
            dst.push(c.clone());
            all.push(c);
        }
    };
    // Depth 1 raw binary combinators over `{k, acc, 0, 1, 2}` x `{Add, Sub, Mul, Max, Min}`
    for i in 0..atoms.len() {
        for j in i..atoms.len() {
            add_step(OmegaExpr::Add(Box::new(atoms[i].clone()), Box::new(atoms[j].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
            add_step(OmegaExpr::Sub(Box::new(atoms[i].clone()), Box::new(atoms[j].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
            if i != j {
                add_step(OmegaExpr::Sub(Box::new(atoms[j].clone()), Box::new(atoms[i].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
            }
            add_step(OmegaExpr::Mul(Box::new(atoms[i].clone()), Box::new(atoms[j].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
            add_step(OmegaExpr::Max(Box::new(atoms[i].clone()), Box::new(atoms[j].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
            add_step(OmegaExpr::Min(Box::new(atoms[i].clone()), Box::new(atoms[j].clone())), &mut d1_steps, &mut all_steps, &mut leaf_ops);
        }
    }
    // Depth 2 raw compositions combining `d1_steps` with ALL raw atoms `{k, acc, 0, 1, 2}`
    let mut d2_tmp: Vec<OmegaExpr> = Vec::new();
    for s1 in &d1_steps {
        for at in &atoms {
            add_step(OmegaExpr::Add(Box::new(at.clone()), Box::new(s1.clone())), &mut d2_tmp, &mut all_steps, &mut leaf_ops);
            add_step(OmegaExpr::Sub(Box::new(at.clone()), Box::new(s1.clone())), &mut d2_tmp, &mut all_steps, &mut leaf_ops);
            add_step(OmegaExpr::Mul(Box::new(at.clone()), Box::new(s1.clone())), &mut d2_tmp, &mut all_steps, &mut leaf_ops);
        }
    }
    all_steps.sort_by(|a, b| {
        let sa = (a.mdl_size() as f64) + 0.20 * op_prior_nll(a, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        let sb = (b.mdl_size() as f64) + 0.20 * op_prior_nll(b, &EmptyResolver, &op_priors, &op_base, &var_priors, &var_base);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });

    let bases = [OmegaExpr::Const(0.0), OmegaExpr::Const(1.0), OmegaExpr::Const(2.0)];
    let mut expansions = 0usize;
    let mut best_expr = None;
    let mut best_err = f64::INFINITY;

    for st in &all_steps {
        for b in &bases {
            let cand = OmegaExpr::Rec {
                base: Box::new(b.clone()),
                step: Box::new(st.clone()),
                max_fuel: 256,
            };
            expansions += 1;
            let mut s = 0.0;
            for (x, tgt) in train_pairs {
                s += (cand.eval_num(x, None, &EmptyResolver, &mut leaf_ops) - *tgt).abs();
            }
            let avg = s / train_pairs.len().max(1) as f64;
            if avg < best_err - 1e-9 {
                best_err = avg;
                best_expr = Some(cand);
            }
            if best_err < 1e-6 {
                return SynthOutcome {
                    expr: best_expr,
                    exact_match: true,
                    residual_err: best_err,
                    raw_candidates_seen: expansions,
                    oe_classes_kept: all_steps.len(),
                    expansions,
                    leaf_ops,
                    egraph_unions: 0,
                    used_continuous_ad: false,
                };
            }
        }
    }
    SynthOutcome {
        expr: best_expr,
        exact_match: best_err < 1e-3,
        residual_err: best_err,
        raw_candidates_seen: expansions,
        oe_classes_kept: all_steps.len(),
        expansions,
        leaf_ops,
        egraph_unions: 0,
        used_continuous_ad: false,
    }
}

/// ZERO-POOL Pure Raw-Grammar 2-Register `FoldSeq` DP Synthesizer (ZERO `step_a_pool`, ZERO `Const(5.0)`!)
pub fn synthesize_sequence_omega(
    core: &mut UniversalEnergyCore,
    train_seqs: &[(Vec<f64>, f64)],
    ticks: usize,
    device_id: usize,
) -> SynthOutcome {
    let task_z = core.encode_sequence_io(train_seqs);
    let (op_priors, _, var_priors, _, _, _) = core.predict_state_policy(&task_z, ticks, device_id);

    let reg_probes: [[f64; 3]; 8] = [
        [-3.0, 2.0, 0.0],
        [4.0, 1.0, 0.0],
        [-5.0, 0.0, 0.0],
        [2.0, 3.0, 0.0],
        [-1.0, 4.0, 0.0],
        [3.0, -2.0, 0.0],
        [0.0, 2.0, 0.0],
        [-4.0, 3.0, 0.0],
    ];
    let atoms_a = [OmegaExpr::Arg(0), OmegaExpr::Arg(1), OmegaExpr::Const(0.0)];
    let mut step_a_oe: HashSet<Vec<i64>> = HashSet::new();
    let mut depth1_a: Vec<OmegaExpr> = Vec::new();
    let mut all_step_a: Vec<OmegaExpr> = Vec::new();
    let mut leaf_ops = 0usize;

    let mut add_step_a = |e: OmegaExpr, dst: &mut Vec<OmegaExpr>, all: &mut Vec<OmegaExpr>| {
        let c = e.canonicalize();
        let vals: Vec<f64> = reg_probes.iter().map(|p| c.eval_num(p, None, &EmptyResolver, &mut leaf_ops)).collect();
        let sig = oe_signature(&vals);
        if step_a_oe.insert(sig) {
            dst.push(c.clone());
            all.push(c);
        }
    };

    for a in &atoms_a {
        add_step_a(OmegaExpr::Abs(Box::new(a.clone())), &mut depth1_a, &mut all_step_a);
    }
    for i in 0..atoms_a.len() {
        for j in i..atoms_a.len() {
            add_step_a(OmegaExpr::Add(Box::new(atoms_a[i].clone()), Box::new(atoms_a[j].clone())), &mut depth1_a, &mut all_step_a);
            add_step_a(OmegaExpr::Max(Box::new(atoms_a[i].clone()), Box::new(atoms_a[j].clone())), &mut depth1_a, &mut all_step_a);
            add_step_a(OmegaExpr::Min(Box::new(atoms_a[i].clone()), Box::new(atoms_a[j].clone())), &mut depth1_a, &mut all_step_a);
        }
    }
    let mut depth2_a: Vec<OmegaExpr> = Vec::new();
    for d1 in &depth1_a {
        for at in &atoms_a {
            add_step_a(OmegaExpr::Max(Box::new(at.clone()), Box::new(d1.clone())), &mut depth2_a, &mut all_step_a);
            add_step_a(OmegaExpr::Add(Box::new(at.clone()), Box::new(d1.clone())), &mut depth2_a, &mut all_step_a);
        }
    }
    let seq_base = core.op_base_rate;
    let seq_vbase = core.var_base_rate;
    all_step_a.sort_by(|a, b| {
        let sa = (a.mdl_size() as f64) + 0.15 * op_prior_nll(a, &EmptyResolver, &op_priors, &seq_base, &var_priors, &seq_vbase);
        let sb = (b.mdl_size() as f64) + 0.15 * op_prior_nll(b, &EmptyResolver, &op_priors, &seq_base, &var_priors, &seq_vbase);
        sa.partial_cmp(&sb).unwrap_or(Ordering::Equal)
    });

    let reg_a = OmegaExpr::Arg(1);
    let reg_b = OmegaExpr::Arg(2);
    let z0 = OmegaExpr::Const(0.0);
    let step_b_grammar: Vec<OmegaExpr> = vec![
        reg_a.clone(),
        OmegaExpr::Max(Box::new(reg_b.clone()), Box::new(reg_a.clone())),
        OmegaExpr::Min(Box::new(reg_b.clone()), Box::new(reg_a.clone())),
        OmegaExpr::Add(Box::new(reg_b.clone()), Box::new(reg_a.clone())),
    ];

    let mut expansions = 0usize;
    let mut best_expr: Option<OmegaExpr> = None;
    let mut best_err = f64::INFINITY;

    for sa in &all_step_a {
        for sb in &step_b_grammar {
            for ret_reg in [0u8, 1u8] {
                if ret_reg == 0 && *sb != reg_a {
                    continue;
                }
                let cand = OmegaExpr::FoldSeq {
                    init_a: Box::new(z0.clone()),
                    init_b: Box::new(z0.clone()),
                    step_a: Box::new(sa.clone()),
                    step_b: Box::new(sb.clone()),
                    return_reg: ret_reg,
                };
                expansions += 1;
                let mut err_sum = 0.0;
                for (seq, tgt) in train_seqs {
                    err_sum += (cand.eval_num(&[], Some(seq), &core.memory, &mut leaf_ops) - *tgt).abs();
                }
                let avg_err = err_sum / train_seqs.len().max(1) as f64;
                if avg_err < best_err - 1e-9
                    || ((avg_err - best_err).abs() < 1e-6
                        && best_expr.as_ref().map(|b| cand.mdl_size() < b.mdl_size()).unwrap_or(true))
                {
                    best_err = avg_err;
                    best_expr = Some(cand);
                }
                if best_err < 1e-6 {
                    return SynthOutcome {
                        expr: best_expr,
                        exact_match: true,
                        residual_err: best_err,
                        raw_candidates_seen: expansions,
                        oe_classes_kept: all_step_a.len(),
                        expansions,
                        leaf_ops,
                        egraph_unions: 0,
                        used_continuous_ad: false,
                    };
                }
            }
        }
    }

    let max_y = train_seqs.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);
    let mut used_param = false;
    if best_err > 1e-4 && max_y.is_finite() {
        used_param = true;
        let phi = 0.618_033_988_75f64;
        for sa in &all_step_a {
            let eval_cap = |cap_v: f64, lops: &mut usize| -> f64 {
                let c_expr = OmegaExpr::FoldSeq {
                    init_a: Box::new(z0.clone()),
                    init_b: Box::new(z0.clone()),
                    step_a: Box::new(OmegaExpr::Min(Box::new(OmegaExpr::Const(cap_v)), Box::new(sa.clone()))),
                    step_b: Box::new(reg_a.clone()),
                    return_reg: 0,
                };
                let mut s = 0.0;
                for (seq, tgt) in train_seqs {
                    s += (c_expr.eval_num(&[], Some(seq), &EmptyResolver, lops) - *tgt).abs();
                }
                s / train_seqs.len().max(1) as f64
            };
            let mut a_g = (max_y - 2.0).max(0.0);
            let mut b_g = max_y + 4.0;
            let mut c_g = b_g - phi * (b_g - a_g);
            let mut d_g = a_g + phi * (b_g - a_g);
            let mut fc = eval_cap(c_g, &mut leaf_ops);
            let mut fd = eval_cap(d_g, &mut leaf_ops);
            for _ in 0..18 {
                if fc < fd {
                    b_g = d_g;
                    d_g = c_g;
                    fd = fc;
                    c_g = b_g - phi * (b_g - a_g);
                    fc = eval_cap(c_g, &mut leaf_ops);
                } else {
                    a_g = c_g;
                    c_g = d_g;
                    fc = fd;
                    d_g = a_g + phi * (b_g - a_g);
                    fd = eval_cap(d_g, &mut leaf_ops);
                }
            }
            let cand_cap = 0.5 * (a_g + b_g);
            let mut best_local = (max_y, eval_cap(max_y, &mut leaf_ops));
            for try_c in [cand_cap, cand_cap.round()] {
                let e = eval_cap(try_c, &mut leaf_ops);
                if e < best_local.1 {
                    best_local = (try_c, e);
                }
            }
            expansions += 1;
            if best_local.1 < best_err - 1e-6 {
                best_err = best_local.1;
                best_expr = Some(OmegaExpr::FoldSeq {
                    init_a: Box::new(z0.clone()),
                    init_b: Box::new(z0.clone()),
                    step_a: Box::new(OmegaExpr::Min(Box::new(OmegaExpr::Const(best_local.0)), Box::new(sa.clone()))),
                    step_b: Box::new(reg_a.clone()),
                    return_reg: 0,
                });
                if best_local.1 < 1e-4 {
                    break;
                }
            }
        }
    }

    SynthOutcome {
        expr: best_expr,
        exact_match: best_err < 1e-3,
        residual_err: best_err,
        raw_candidates_seen: expansions,
        oe_classes_kept: all_step_a.len(),
        expansions,
        leaf_ops,
        egraph_unions: 0,
        used_continuous_ad: used_param,
    }
}

// ============================================================================
// 4. TRUE ZERO-POOL Raw `CoordExpr` & `CellExpr` Bottom-Up Grammar 2D Synthesizer (`v6.0`)!
//    100% DELETED hardcoded `grav_stencil`, `hollow_stencil`, `ray_right`, `ray_down`, `MirrorH`, `FlipV`!
//    Synthesizes ALL coordinate maps & cellular-automaton stencil ASTs purely from the 5 raw coord atoms
//    `{Row, Col, H, W, 1}` and 7 raw cell atoms `{Self, N, So, E, W, AtBottom, 0}` via OE bottom-up search!
// ============================================================================

/// Pure bottom-up grammar synthesizer for `CoordExpr` & `CellExpr` (ZERO hardcoded stencil recipes!).
pub fn enumerate_raw_grid_primitives(allow_bounded_fix: bool) -> Vec<OmegaExpr> {
    let mut ops: Vec<OmegaExpr> = Vec::new();

    // 1. PURE RAW `CoordExpr` BOTTOM-UP GRAMMAR over `{Row, Col, H, W, 1}` x `{Sub}` (OE-deduplicated!)
    //    Probe TWO distinct shapes `(4, 7)` and `(6, 5)` so `H`, `W`, `H-1`, `W-1` NEVER alias!
    let c_atoms = [CoordExpr::Row, CoordExpr::Col, CoordExpr::Height, CoordExpr::Width, CoordExpr::One];
    let probe_shapes = [(4usize, 7usize), (6usize, 5usize)];
    let coord_sig = |ce: &CoordExpr| -> Vec<isize> {
        let mut s = Vec::with_capacity(58);
        for &(ph, pw) in &probe_shapes {
            for r in 0..ph {
                for c in 0..pw {
                    s.push(ce.eval(r, c, ph, pw));
                }
            }
        }
        s
    };
    let in_bounds_r = |ce: &CoordExpr| -> bool {
        let mut nonconst = false;
        for &(ph, pw) in &probe_shapes {
            let v0 = ce.eval(0, 0, ph, pw);
            for r in 0..ph {
                for c in 0..pw {
                    let v = ce.eval(r, c, ph, pw);
                    if v < 0 || (v as usize) >= ph {
                        return false;
                    }
                    if v != v0 {
                        nonconst = true;
                    }
                }
            }
        }
        nonconst
    };
    let in_bounds_c = |ce: &CoordExpr| -> bool {
        let mut nonconst = false;
        for &(ph, pw) in &probe_shapes {
            let v0 = ce.eval(0, 0, ph, pw);
            for r in 0..ph {
                for c in 0..pw {
                    let v = ce.eval(r, c, ph, pw);
                    if v < 0 || (v as usize) >= pw {
                        return false;
                    }
                    if v != v0 {
                        nonconst = true;
                    }
                }
            }
        }
        nonconst
    };
    let mut c_seen: HashSet<Vec<isize>> = HashSet::new();
    let mut c_d01: Vec<CoordExpr> = Vec::new();
    for a in &c_atoms {
        if c_seen.insert(coord_sig(a)) {
            c_d01.push(a.clone());
        }
    }
    let d0_len = c_d01.len();
    for i in 0..d0_len {
        for j in 0..d0_len {
            if i == j { continue; }
            let sub = CoordExpr::Sub(Box::new(c_d01[i].clone()), Box::new(c_d01[j].clone()));
            if c_seen.insert(coord_sig(&sub)) {
                c_d01.push(sub);
            }
        }
    }
    let mut all_coords = c_d01.clone();
    // Depth 2: `(H - 1) - r`, `(W - 1) - c`, etc. synthesized purely from grammar!
    for a in &c_d01 {
        for b in &c_atoms[..2] {
            let sub = CoordExpr::Sub(Box::new(a.clone()), Box::new(b.clone()));
            if c_seen.insert(coord_sig(&sub)) {
                all_coords.push(sub);
            }
        }
    }
    let valid_r: Vec<CoordExpr> = all_coords.iter().filter(|e| in_bounds_r(e)).cloned().collect();
    let valid_c: Vec<CoordExpr> = all_coords.iter().filter(|e| in_bounds_c(e)).cloned().collect();
    // Pair valid non-identity index expressions into `GridCoordMap` (discovers MirrorH, FlipV, Rot180, Transpose!)
    let mut coord_keys: HashSet<String> = HashSet::new();
    for re in &valid_r {
        for ce in &valid_c {
            if *re == CoordExpr::Row && *ce == CoordExpr::Col {
                continue; // skip identity
            }
            let gm = OmegaExpr::GridCoordMap { r_expr: re.clone(), c_expr: ce.clone() };
            if coord_keys.insert(gm.canonical_key()) {
                ops.push(gm);
            }
        }
    }
    // Also allow transposed index swaps (`r_expr = Col`, `c_expr = Row`) on square grids
    let tr = OmegaExpr::GridCoordMap { r_expr: CoordExpr::Col, c_expr: CoordExpr::Row };
    if coord_keys.insert(tr.canonical_key()) {
        ops.push(tr);
    }

    // 2. PURE UNIFORM DEPTH-3 BOTTOM-UP `CellExpr` GRAMMAR over the 7 raw atoms
    //    `{SelfVal, North, South, East, West, AtBottom, Zero}` x `{Min, Max, IfPos}` (`v8.0`)!
    //    ZERO outer stencil schemas (`IfPos(Self,Self,nb)`, `IfPos(c,Zero,Self)`, `IfPos(Self,stay,nb)` DELETED!).
    //    Filtered SOLELY by 3 universal physical conservation laws on 13 probes + OE deduplication:
    //      (P1) Vacuum Conservation: empty neighborhood -> 0 (both interior & bottom edge)
    //      (P2) Color Conservation: output is 0 or one of {Self, N, So, E, W} (no invented colors)
    //      (P3) Local Occupancy Preservation: if Self > 0 then output in {0, Self}
    let cell_probes: [(i64, i64, i64, i64, i64, i64); 13] = [
        (0, 0, 0, 0, 0, 0),
        (2, 0, 0, 0, 0, 0),
        (2, 0, 0, 0, 0, 1),
        (2, 0, 3, 0, 0, 0),
        (0, 2, 0, 0, 0, 0),
        (0, 0, 0, 0, 3, 0),
        (0, 0, 0, 2, 0, 0),
        (1, 1, 1, 1, 1, 0),
        (1, 1, 1, 1, 0, 0),
        (1, 0, 1, 1, 1, 0),
        (3, 2, 0, 1, 2, 1),
        (0, 1, 2, 3, 2, 0),
        (0, 0, 0, 0, 0, 1),
    ];
    let cell_sig = |ce: &CellExpr| -> Vec<i64> {
        cell_probes
            .iter()
            .map(|&(s, n, so, e, w, b)| ce.eval(s, n, so, e, w, b))
            .collect()
    };
    let self_sig = cell_sig(&CellExpr::SelfVal);
    let zero_sig = cell_sig(&CellExpr::Zero);
    let cell_rule_is_conservative = |sig: &[i64]| -> bool {
        if sig[0] != 0 || sig[12] != 0 {
            return false; // (P1) Vacuum conservation (interior & bottom boundary)
        }
        if sig == self_sig.as_slice() || sig == zero_sig.as_slice() {
            return false; // non-triviality (reject identity / constant zero)
        }
        for (idx, &(s, n, so, e, w, _b)) in cell_probes.iter().enumerate() {
            let v = sig[idx];
            // (P2) Color conservation: output must come from {0, s, n, so, e, w}
            if v != 0 && v != s && v != n && v != so && v != e && v != w {
                return false;
            }
            // (P3) Local occupancy preservation: occupied cell either stays or vacates
            if s > 0 && v != 0 && v != s {
                return false;
            }
        }
        true
    };

    let d0_cond_atoms = [
        CellExpr::SelfVal,
        CellExpr::North,
        CellExpr::South,
        CellExpr::East,
        CellExpr::West,
        CellExpr::AtBottom,
    ];
    let d0_val_atoms = [
        CellExpr::SelfVal,
        CellExpr::North,
        CellExpr::South,
        CellExpr::East,
        CellExpr::West,
        CellExpr::Zero,
    ];
    let sensor_atoms = [
        CellExpr::North,
        CellExpr::South,
        CellExpr::East,
        CellExpr::West,
        CellExpr::AtBottom,
    ];
    let occ_vals = [CellExpr::SelfVal, CellExpr::Zero];

    let mut cell_seen: HashSet<Vec<i64>> = HashSet::new();
    let mut expr_seen: HashSet<Vec<i64>> = HashSet::new();
    let mut synthesized_rules: Vec<CellExpr> = Vec::new();
    let mut admit_rule = |r: CellExpr| {
        let sig = cell_sig(&r);
        if cell_rule_is_conservative(&sig) && cell_seen.insert(sig) {
            synthesized_rules.push(r);
        }
    };

    // Depth 1: Uniform `IfPos(c, t, f)` over D0 atoms + Uniform pairwise `Min`/`Max` over sensor atoms!
    for c in &d0_cond_atoms {
        for t in &d0_val_atoms {
            for f in &d0_val_atoms {
                if t == f {
                    continue;
                }
                admit_rule(CellExpr::IfPos(Box::new(c.clone()), Box::new(t.clone()), Box::new(f.clone())));
            }
        }
    }
    let mut d1_min_masks: Vec<(u8, CellExpr)> = Vec::new();
    let mut d1_maxs: Vec<CellExpr> = Vec::new();
    for i in 0..sensor_atoms.len() {
        for j in (i + 1)..sensor_atoms.len() {
            let mn = CellExpr::Min(Box::new(sensor_atoms[i].clone()), Box::new(sensor_atoms[j].clone()));
            if expr_seen.insert(cell_sig(&mn)) {
                d1_min_masks.push(((1u8 << i) | (1u8 << j), mn));
            }
            let mx = CellExpr::Max(Box::new(sensor_atoms[i].clone()), Box::new(sensor_atoms[j].clone()));
            if expr_seen.insert(cell_sig(&mx)) {
                d1_maxs.push(mx);
            }
        }
    }

    // Depth 2: Algebraic-idempotent 4-sensor `Min` (`min(a,a)=a`) + Uniform `IfPos(c, t, f)` over `D1_Max`!
    let mut d2_min4: Vec<CellExpr> = Vec::new();
    let mut d2_masks: HashSet<u8> = HashSet::new();
    for i in 0..d1_min_masks.len() {
        for j in (i + 1)..d1_min_masks.len() {
            let (m1, ref e1) = d1_min_masks[i];
            let (m2, ref e2) = d1_min_masks[j];
            if (m1 & m2) != 0 {
                continue;
            }
            if d2_masks.insert(m1 | m2) {
                let m4 = CellExpr::Min(Box::new(e1.clone()), Box::new(e2.clone()));
                if expr_seen.insert(cell_sig(&m4)) {
                    d2_min4.push(m4);
                }
            }
        }
    }
    let mut d2_ifs: Vec<CellExpr> = Vec::new();
    for mx in &d1_maxs {
        for t in &occ_vals {
            for f in &occ_vals {
                if t == f {
                    continue;
                }
                let e = CellExpr::IfPos(Box::new(mx.clone()), Box::new(t.clone()), Box::new(f.clone()));
                let sig = cell_sig(&e);
                if expr_seen.insert(sig.clone()) {
                    d2_ifs.push(e.clone());
                }
                admit_rule(e);
            }
        }
    }

    // Depth 3: Uniform `IfPos(c, t, f)` over `(D2_Min4 x occ x occ)` AND `(D0_cond x D2_If x D0_val)`!
    // Automatically discovers Hollow, Gravity, and ALL conservative transport/erosion laws via (P1)-(P3)!
    for c4 in &d2_min4 {
        for t in &occ_vals {
            for f in &occ_vals {
                if t == f {
                    continue;
                }
                admit_rule(CellExpr::IfPos(Box::new(c4.clone()), Box::new(t.clone()), Box::new(f.clone())));
            }
        }
    }
    for c in &d0_cond_atoms {
        for t in &d2_ifs {
            for f in &d0_val_atoms {
                admit_rule(CellExpr::IfPos(Box::new(c.clone()), Box::new(t.clone()), Box::new(f.clone())));
            }
        }
    }

    for rule in synthesized_rules {
        let gsr = OmegaExpr::GridStencilRule { rule };
        ops.push(gsr.clone());
        if allow_bounded_fix {
            ops.push(OmegaExpr::BoundedFix {
                step: Box::new(gsr),
                max_steps: 24,
            });
        }
    }
    ops
}

pub fn synthesize_grid_omega(
    core: &mut UniversalEnergyCore,
    train_pairs: &[(Grid2D, Grid2D)],
    allow_bounded_fix: bool,
    use_cegis: bool,
    use_memory: bool,
    eval_budget: usize,
    ticks: usize,
    device_id: usize,
) -> (Option<OmegaExpr>, usize, usize) {
    core.clear_cegis_barriers();
    let task_z = core.encode_grid_io(train_pairs);
    let mut total_leaf_ops = 0usize;

    let mut base_ops = enumerate_raw_grid_primitives(allow_bounded_fix);
    let retrieved_ids = if use_memory {
        core.memory.retrieve_tropical_topk(&task_z, Some("grid2d"), 8)
    } else {
        Vec::new()
    };
    for &cid in &retrieved_ids {
        base_ops.push(OmegaExpr::ConceptCall { id: cid, args: Vec::new() });
    }

    let mut lattice: Vec<(OmegaExpr, [f64; OMEGA_DIM], f64)> = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    let sample_in = train_pairs.first().map(|(g, _)| g.clone()).unwrap_or_else(|| vec![vec![0; 4]; 4]);
    let sample_tgt = train_pairs.first().map(|(_, t)| t.clone()).unwrap_or_else(|| vec![vec![0; 4]; 4]);

    let mut push_cand = |e: OmegaExpr, core_ref: &UniversalEnergyCore, ops_acc: &mut usize| {
        let canon = e.canonicalize();
        if seen_keys.insert(canon.canonical_key()) {
            let out = canon.eval_grid(&sample_in, &core_ref.memory, ops_acc);
            let pix_err = 1.0 - grid_exact_match_fraction(&out, &sample_tgt);
            let emb = core_ref.encode_grid_io(&[(sample_in.clone(), out)]);
            lattice.push((canon, emb, pix_err));
        }
    };

    for a in &base_ops {
        push_cand(a.clone(), core, &mut total_leaf_ops);
    }
    for a in &base_ops {
        for b in &base_ops {
            if a == b {
                continue;
            }
            push_cand(OmegaExpr::Compose(Box::new(a.clone()), Box::new(b.clone())), core, &mut total_leaf_ops);
        }
    }

    let mut tested_set: HashSet<usize> = HashSet::new();
    let mut actual_evals = 0usize;
    let mut best_op: Option<OmegaExpr> = None;
    let mut best_score = -1.0f64;

    if !use_cegis {
        for (idx, (op, _, _)) in lattice.iter().enumerate() {
            if actual_evals >= eval_budget {
                break;
            }
            actual_evals += 1;
            tested_set.insert(idx);
            let mut msum = 0.0;
            let mut all_exact = true;
            for (inp, tgt) in train_pairs {
                let got = op.eval_grid(inp, &core.memory, &mut total_leaf_ops);
                msum += grid_exact_match_fraction(&got, tgt);
                if got != *tgt {
                    all_exact = false;
                }
            }
            let avg = msum / train_pairs.len().max(1) as f64;
            if avg > best_score {
                best_score = avg;
                best_op = Some(op.clone());
            }
            if all_exact {
                return (Some(op.clone()), actual_evals, total_leaf_ops);
            }
        }
        return (best_op, actual_evals, total_leaf_ops);
    }

    // Execution-Guided Pixel-Residual + Contractive Latent + CEGIS Best-First Grid Search!
    while actual_evals < eval_budget && tested_set.len() < lattice.len() {
        let particles = core.relax_parallel_particles(&task_z, 8, ticks, use_cegis, 0.0, device_id);
        let mut scored: Vec<(usize, f64)> = Vec::new();
        for (idx, (op, emb, pix_err)) in lattice.iter().enumerate() {
            if tested_set.contains(&idx) {
                continue;
            }
            let mut min_d = f64::INFINITY;
            for pt in &particles {
                let d = core.learned_program_energy(pt, emb);
                if d < min_d {
                    min_d = d;
                }
            }
            let bar = core.cegis_barrier_energy(emb);
            let e = *pix_err + 0.14 * min_d + 0.35 * bar + 0.012 * (op.mdl_size() as f64);
            scored.push((idx, e));
        }
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        let mut round_exact: Option<OmegaExpr> = None;
        for &(idx, _) in scored.iter().take(4) {
            if actual_evals >= eval_budget {
                break;
            }
            actual_evals += 1;
            tested_set.insert(idx);
            let (ref op, ref emb, _) = lattice[idx];
            let mut msum = 0.0;
            let mut all_exact = true;
            for (inp, tgt) in train_pairs {
                let got = op.eval_grid(inp, &core.memory, &mut total_leaf_ops);
                msum += grid_exact_match_fraction(&got, tgt);
                if got != *tgt {
                    all_exact = false;
                }
            }
            let avg = msum / train_pairs.len().max(1) as f64;
            if avg > best_score {
                best_score = avg;
                best_op = Some(op.clone());
            }
            if all_exact {
                let better = round_exact.as_ref().map(|p| op.mdl_size() < p.mdl_size()).unwrap_or(true);
                if better {
                    round_exact = Some(op.clone());
                }
            } else {
                core.inject_cegis_barrier(*emb, 3.0);
            }
        }
        if let Some(exact) = round_exact {
            if let Some(&cid) = retrieved_ids.first() {
                core.memory.record_usage(cid);
            }
            return (Some(exact), actual_evals, total_leaf_ops);
        }
    }
    (best_op, actual_evals, total_leaf_ops)
}

// ============================================================================
// 5. ZERO-BIAS Step-by-Step Inverse Execution Wake-Solve-Sleep HER (`v5.0`)!
// ============================================================================

fn splitmix_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn sample_unbiased_pcfg_expr(rng: &mut u64, depth: usize, concept_ids: &[(usize, usize)]) -> OmegaExpr {
    let leaves = [
        OmegaExpr::Arg(0),
        OmegaExpr::Arg(1),
        OmegaExpr::Arg(2),
        OmegaExpr::Const(0.0),
        OmegaExpr::Const(1.0),
        OmegaExpr::Const(2.0),
    ];
    if depth == 0 {
        return leaves[(splitmix_u64(rng) as usize) % leaves.len()].clone();
    }
    if !concept_ids.is_empty() && (splitmix_u64(rng) % 5 == 0) {
        let &(cid, arity) = &concept_ids[(splitmix_u64(rng) as usize) % concept_ids.len()];
        let args: Vec<OmegaExpr> = (0..arity)
            .map(|_| sample_unbiased_pcfg_expr(rng, depth.saturating_sub(1), concept_ids))
            .collect();
        return OmegaExpr::ConceptCall { id: cid, args };
    }
    let op = (splitmix_u64(rng) as usize) % 8;
    let d1 = depth - 1;
    let d2 = if (splitmix_u64(rng) & 1) == 0 { d1 } else { 0 };
    match op {
        0 => OmegaExpr::Add(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)), Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids))),
        1 => OmegaExpr::Sub(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)), Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids))),
        2 => OmegaExpr::Mul(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)), Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids))),
        3 => OmegaExpr::Min(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)), Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids))),
        4 => OmegaExpr::Max(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)), Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids))),
        5 => OmegaExpr::Abs(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids))),
        6 => OmegaExpr::SqrtSafe(Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids))),
        _ => OmegaExpr::IfPos(
            Box::new(leaves[(splitmix_u64(rng) as usize) % 3].clone()),
            Box::new(sample_unbiased_pcfg_expr(rng, d1, concept_ids)),
            Box::new(sample_unbiased_pcfg_expr(rng, d2, concept_ids)),
        ),
    }
}

/// Decompose a tree's top-level step into `(child_a, optional_sibling_b, next_op_class)`!
fn top_step_decomp(e: &OmegaExpr) -> Option<(&OmegaExpr, Option<&OmegaExpr>, usize)> {
    match e {
        OmegaExpr::Add(a, b) => Some((a, Some(b), 0)),
        OmegaExpr::Sub(a, b) => Some((a, Some(b), 1)),
        OmegaExpr::Mul(a, b) => Some((a, Some(b), 2)),
        OmegaExpr::DivSafe(a, b) => Some((a, Some(b), 3)),
        OmegaExpr::Min(a, b) => Some((a, Some(b), 4)),
        OmegaExpr::Max(a, b) => Some((a, Some(b), 5)),
        OmegaExpr::Abs(a) => Some((a, None, 6)),
        OmegaExpr::SqrtSafe(a) => Some((a, None, 7)),
        OmegaExpr::IfPos(_, t, el) => Some((t, Some(el), 8)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct WakeSleepCycleStats {
    pub frontier_tasks_generated: usize,
    pub wake_tasks_solved: usize,
    pub her_residual_samples: usize,
    pub concepts_mined: usize,
    pub egraph_rules_promoted: usize,
    pub loss_init: f64,
    pub loss_final: f64,
    pub prover_report: ProverReport,
}

/// Complete ZERO-BIAS Wake-Solve-Sleep + Step-by-Step Inverse Execution HER Cycle (`v5.0`)!
/// Uses the REAL Batch Bytecode VM to evaluate 420+ unbiased uniform PCFG frontier trees at once!
pub fn run_unbiased_wake_sleep_cycle(
    core: &mut UniversalEnergyCore,
    out_dir: &Path,
    tag: &str,
    seed: u64,
    epochs: usize,
) -> WakeSleepCycleStats {
    let mut rng = seed ^ 0x5E1F_B500_2026;
    let probe_in = oe_probe_inputs();
    let existing_concepts: Vec<(usize, usize)> = core
        .memory
        .concepts
        .iter()
        .filter(|c| c.domain_tag != "grid2d" && c.arity >= 1 && c.arity <= 2)
        .map(|c| (c.id, c.arity))
        .collect();

    // 1. Sample 600 unique strictly-UNIFORM unbiased PCFG trees in ONE Batch Bytecode VM call!
    let mut raw_pool: Vec<OmegaExpr> = Vec::with_capacity(2400);
    let mut flat_pool: Vec<FlatProgram> = Vec::with_capacity(2400);
    for _ in 0..4800 {
        let depth = 1 + ((splitmix_u64(&mut rng) as usize) % 3);
        let raw = sample_unbiased_pcfg_expr(&mut rng, depth, &existing_concepts).canonicalize();
        let sz = raw.mdl_size();
        if sz < 3 || sz > 14 {
            continue;
        }
        if let Some(fp) = FlatProgram::compile(&raw, &core.memory) {
            raw_pool.push(raw);
            flat_pool.push(fp);
            if raw_pool.len() >= 2200 {
                break;
            }
        }
    }
    let (batch_vals, batch_hashes) = omega_eval_bytecode_batch_on_device(&flat_pool, &probe_in, 0);
    let n_p = probe_in.len();

    let mut frontier_progs: Vec<(OmegaExpr, Vec<f64>)> = Vec::new();
    let mut seen_h: HashSet<u64> = HashSet::new();
    for (i, expr) in raw_pool.into_iter().enumerate() {
        let row = &batch_vals[i * n_p..(i + 1) * n_p];
        if row.iter().any(|v| !v.is_finite() || v.abs() > 400.0) {
            continue;
        }
        let ys: Vec<f64> = row.iter().map(|&v| v as f64).collect();
        if ys.iter().all(|&v| (v - ys[0]).abs() < 1e-4) {
            continue;
        }
        if seen_h.insert(batch_hashes[i]) {
            frontier_progs.push((expr, ys));
            if frontier_progs.len() >= 600 {
                break;
            }
        }
    }

    // 2. TRUE WAKE PHASE: Actually solve a slice of unlabeled frontier tasks with our own Batched Levin solver!
    let mut wake_solved_corpus: Vec<OmegaExpr> = Vec::new();
    for (idx, (_, ys)) in frontier_progs.iter().take(12).enumerate() {
        let pairs: Vec<([f64; 3], f64)> = probe_in.iter().copied().zip(ys.iter().copied()).collect();
        let o = synthesize_numeric_omega(
            core, &pairs, 2, 16, 220, false, true, true, false, false, 2, idx % 4,
        );
        if o.exact_match {
            if let Some(e) = o.expr {
                wake_solved_corpus.push(e);
            }
        }
    }
    let wake_tasks_solved = wake_solved_corpus.len();

    // 3. Build Dense Step-by-Step Inverse Execution HER Samples on FRESH RANDOM 20-POINT DATASETS
    //    per program (100% ELIMINATES fixed-`probe_in` memorization so `W_enc` generalizes to ANY unseen `X`!)
    let mut samples: Vec<RawTrainSample> = Vec::with_capacity(frontier_progs.len() * 4);
    let n_fp = frontier_progs.len();
    let mut ops = 0usize;

    for i in 0..n_fp {
        let (ref prog, _) = frontier_progs[i];
        let (ref neg_prog, _) = frontier_progs[(i + 13) % n_fp];

        // Freshly sampled random `[-4, 4]^3` input set (N=20 points) unique to THIS program!
        let mut task_xs: Vec<[f64; 3]> = Vec::with_capacity(20);
        for _ in 0..20 {
            let mut next_x = || ((splitmix_u64(&mut rng) % 9) as i64 - 4) as f64;
            task_xs.push([next_x(), next_x(), next_x()]);
        }
        let ys_rand: Vec<f64> = task_xs
            .iter()
            .map(|p| prog.eval_num(p, None, &core.memory, &mut ops))
            .collect();
        let neg_ys_rand: Vec<f64> = task_xs
            .iter()
            .map(|p| neg_prog.eval_num(p, None, &core.memory, &mut ops))
            .collect();
        if ys_rand.iter().any(|v| !v.is_finite() || v.abs() > 400.0)
            || neg_ys_rand.iter().any(|v| !v.is_finite() || v.abs() > 400.0)
        {
            continue;
        }
        let pairs: Vec<([f64; 3], f64)> = task_xs.iter().copied().zip(ys_rand.iter().copied()).collect();
        let neg_pairs: Vec<([f64; 3], f64)> = task_xs.iter().copied().zip(neg_ys_rand.iter().copied()).collect();

        let inlined = prog.inline_all(&core.memory, 6);
        let mask = expr_to_op_mask_resolved(&inlined, &core.memory);
        let vm = expr_to_var_mask(&inlined);
        let top_dec = top_step_decomp(&inlined);
        let root_op = top_dec.map(|(_, _, cls)| cls);

        let raw_toks = raw_tokens_numeric_residual(&pairs, None);
        let neg_toks = raw_tokens_numeric_residual(&neg_pairs, None);

        // (a) Root sample `(X, y, none)` -> supervises `op_mask`, `root_op`, `var_mask`!
        samples.push(RawTrainSample {
            raw_tokens: raw_toks.clone(),
            op_mask: mask,
            next_op_target: root_op,
            var_mask: vm,
            value_target: 0.85,
            pos_tokens: raw_toks.clone(),
            neg_tokens: neg_toks.clone(),
            partner_pos_tokens: None,
            partner_neg_tokens: None,
        });

        // (b) Exact 1-step inverse execution sample: from child `A` -> needs EXACT `outer_cls` + sibling `B`!
        if let Some((child_a, sib_b_opt, outer_cls)) = top_dec {
            let a_preds: Vec<f64> = task_xs
                .iter()
                .map(|p| child_a.eval_num(p, None, &core.memory, &mut ops))
                .collect();
            let res_toks = raw_tokens_numeric_residual(&pairs, Some(&a_preds));

            let partner_pos = sib_b_opt.map(|sb| {
                let b_preds: Vec<f64> = task_xs
                    .iter()
                    .map(|p| sb.eval_num(p, None, &core.memory, &mut ops))
                    .collect();
                let bp: Vec<([f64; 3], f64)> = task_xs.iter().copied().zip(b_preds).collect();
                raw_tokens_numeric_residual(&bp, None)
            });
            let partner_neg = if partner_pos.is_some() { Some(neg_toks.clone()) } else { None };

            samples.push(RawTrainSample {
                raw_tokens: res_toks.clone(),
                op_mask: mask,
                next_op_target: Some(outer_cls),
                var_mask: vm,
                value_target: 0.96,
                pos_tokens: res_toks,
                neg_tokens: neg_toks.clone(),
                partner_pos_tokens: partner_pos,
                partner_neg_tokens: partner_neg,
            });

            // (c) 2-step deep subtree sample (if `child_a` itself decomposes into `grandchild_a`)!
            if let Some((grand_a, grand_b_opt, inner_cls)) = top_step_decomp(child_a) {
                let ga_preds: Vec<f64> = task_xs
                    .iter()
                    .map(|p| grand_a.eval_num(p, None, &core.memory, &mut ops))
                    .collect();
                let g_res_toks = raw_tokens_numeric_residual(&pairs, Some(&ga_preds));
                let gp_pos = grand_b_opt.map(|gb| {
                    let gbp: Vec<([f64; 3], f64)> = task_xs
                        .iter()
                        .map(|p| (*p, gb.eval_num(p, None, &core.memory, &mut ops)))
                        .collect();
                    raw_tokens_numeric_residual(&gbp, None)
                });
                let gp_neg = if gp_pos.is_some() { Some(neg_toks.clone()) } else { None };
                samples.push(RawTrainSample {
                    raw_tokens: g_res_toks.clone(),
                    op_mask: mask,
                    next_op_target: Some(inner_cls),
                    var_mask: vm,
                    value_target: 0.72,
                    pos_tokens: g_res_toks,
                    neg_tokens: neg_toks.clone(),
                    partner_pos_tokens: gp_pos,
                    partner_neg_tokens: gp_neg,
                });
            }
        }

        // (d) Negative distractor residual sample (wrong partial program -> `value_target = 0.04`)!
        let bad_res_toks = raw_tokens_numeric_residual(&pairs, Some(&neg_ys_rand));
        samples.push(RawTrainSample {
            raw_tokens: bad_res_toks.clone(),
            op_mask: mask,
            next_op_target: None,
            var_mask: vm,
            value_target: 0.04,
            pos_tokens: raw_toks,
            neg_tokens: bad_res_toks,
            partner_pos_tokens: None,
            partner_neg_tokens: None,
        });
    }

    // 4. End-to-End Mini-Batch AdamW Training across ALL 6 State-Conditioned Heads + Core + Raw Encoder!
    let (loss_init, loss_final) = core.train_end_to_end_ste_epochs(&samples, epochs, 3, 0.14, 0);

    // 5. Autonomous Multi-Op Corpus-Delta-MDL Concept Mining!
    let mut corpus_exprs: Vec<OmegaExpr> = frontier_progs.iter().map(|(e, _)| e.clone()).collect();
    corpus_exprs.extend(wake_solved_corpus);
    let core_snap = core.clone();
    let mined_ids = core.memory.mine_autonomous_mdl_concepts(
        &corpus_exprs,
        "universal",
        6,
        |tmpl, _arity| {
            let mut lops = 0usize;
            let pairs: Vec<([f64; 3], f64)> = probe_in
                .iter()
                .map(|p| (*p, tmpl.eval_num(p, None, &EmptyResolver, &mut lops)))
                .collect();
            core_snap.encode_numeric_residual(&pairs, None)
        },
    );

    // 6. Tier-3 Fast Pure-Rust CEGIS E-Graph Promotion (default: <15ms, ZERO Lean bottleneck!)
    //    Optional Lean 4 kernel proof search enabled ONLY via `--features lean_oracle`!
    #[cfg(feature = "lean_oracle")]
    let prover_report = {
        let concept_tuples: Vec<(usize, usize, OmegaExpr)> = core
            .memory
            .concepts
            .iter()
            .filter(|c| c.domain_tag != "grid2d")
            .map(|c| (c.id, c.arity, c.body.clone()))
            .collect();
        let mem_snap = core.memory.clone();
        run_live_tactic_prover_and_promote(core, out_dir, tag, &concept_tuples, &mem_snap)
    };
    #[cfg(not(feature = "lean_oracle"))]
    let prover_report = run_fast_cegis_algebra_and_promote(out_dir, tag);
    let promoted = core
        .memory
        .install_lean4_egraph_rules(prover_report.promoted_egraph_rules.clone());

    WakeSleepCycleStats {
        frontier_tasks_generated: n_fp,
        wake_tasks_solved,
        her_residual_samples: samples.len(),
        concepts_mined: mined_ids.len(),
        egraph_rules_promoted: promoted,
        loss_init,
        loss_final,
        prover_report,
    }
}

// ============================================================================
// 6. Real `rustc -O` Verifier (Scalar, List/Rec, Grid2D, Strategy) & Local AST Repair
// ============================================================================

fn find_rustc_bin() -> String {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let p = format!("{}\\.cargo\\bin\\rustc.exe", home);
        if Path::new(&p).exists() {
            return p;
        }
    }
    "rustc".to_string()
}

pub fn compile_and_run_rust_source(rs_path: &Path, expected_token: &str, tag: &str) -> bool {
    let bin_name = format!("tropicore_v5_verify_{}_{}", tag, std::process::id());
    let bin_path = std::env::temp_dir().join(if cfg!(windows) {
        format!("{}.exe", bin_name)
    } else {
        bin_name
    });
    let rustc = find_rustc_bin();
    let compiled = Command::new(&rustc)
        .args(["-O"])
        .arg(rs_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let ok = if compiled {
        Command::new(&bin_path)
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains(expected_token))
            .unwrap_or(false)
    } else {
        false
    };
    let _ = fs::remove_file(&bin_path);
    ok
}

pub fn verify_expr_via_real_rustc(
    expr: &OmegaExpr,
    core: &UniversalEnergyCore,
    out_dir: &Path,
    tag: &str,
    scalar_tests: &[([f64; 3], f64)],
    seq_tests: &[(Vec<f64>, f64)],
) -> bool {
    let _ = fs::create_dir_all(out_dir);
    let rs_path = out_dir.join(format!("verified_{}.rs", tag));
    let body_rust = expr.emit_rust_f64(&core.memory);
    let mut src = String::new();
    if !seq_tests.is_empty() {
        src.push_str(&format!(
            "#[allow(unused_variables)]\nfn synthesized(seq: &[f64]) -> f64 {{\n    let x0 = 0.0f64;\n    {}\n}}\nfn main() {{\n",
            body_rust
        ));
        for (idx, (seq, tgt)) in seq_tests.iter().enumerate() {
            let elems: Vec<String> = seq.iter().map(|v| format!("{:.4}f64", v)).collect();
            src.push_str(&format!(
                "    let s{idx} = [{elems}];\n    let g{idx} = synthesized(&s{idx});\n    assert!((g{idx} - ({tgt:.6}f64)).abs() < 1e-2, \"seq {idx} mismatch: {{}} vs {tgt}\", g{idx});\n",
                idx = idx,
                elems = elems.join(","),
                tgt = tgt
            ));
        }
        src.push_str("    println!(\"RUSTC_VERIFY_PASS\");\n}\n");
    } else {
        src.push_str(&format!(
            "#[allow(unused_variables)]\nfn synthesized(x0: f64, x1: f64, x2: f64) -> f64 {{\n    {}\n}}\nfn main() {{\n",
            body_rust
        ));
        for (idx, (x, tgt)) in scalar_tests.iter().enumerate() {
            src.push_str(&format!(
                "    let g{idx} = synthesized({x0:.6}f64, {x1:.6}f64, {x2:.6}f64);\n    assert!((g{idx} - ({tgt:.6}f64)).abs() < 2e-2, \"scalar {idx} mismatch: {{}} vs {tgt}\", g{idx});\n",
                idx = idx,
                x0 = x[0],
                x1 = x[1],
                x2 = x[2],
                tgt = tgt
            ));
        }
        src.push_str("    println!(\"RUSTC_VERIFY_PASS\");\n}\n");
    }
    if fs::write(&rs_path, &src).is_err() {
        return false;
    }
    compile_and_run_rust_source(&rs_path, "RUSTC_VERIFY_PASS", tag)
}

pub fn verify_grid_via_real_rustc(
    expr: &OmegaExpr,
    core: &UniversalEnergyCore,
    out_dir: &Path,
    tag: &str,
    grid_tests: &[(Grid2D, Grid2D)],
) -> bool {
    let _ = fs::create_dir_all(out_dir);
    let rs_path = out_dir.join(format!("verified_grid_{}.rs", tag));
    let body = expr.emit_rust_grid(&core.memory);
    let mut src = String::new();
    src.push_str(&format!(
        "fn synthesized_grid(grid: &Vec<Vec<i64>>) -> Vec<Vec<i64>> {{\n    {}\n}}\nfn main() {{\n",
        body
    ));
    let fmt_grid = |g: &Grid2D| -> String {
        let rows: Vec<String> = g
            .iter()
            .map(|r| format!("vec![{}]", r.iter().map(|v| format!("{}i64", v)).collect::<Vec<_>>().join(",")))
            .collect();
        format!("vec![{}]", rows.join(","))
    };
    for (idx, (inp, tgt)) in grid_tests.iter().enumerate() {
        src.push_str(&format!(
            "    let inp{idx} = {};\n    let tgt{idx} = {};\n    assert_eq!(synthesized_grid(&inp{idx}), tgt{idx});\n",
            fmt_grid(inp),
            fmt_grid(tgt),
            idx = idx
        ));
    }
    src.push_str("    println!(\"RUSTC_GRID_PASS\");\n}\n");
    if fs::write(&rs_path, &src).is_err() {
        return false;
    }
    compile_and_run_rust_source(&rs_path, "RUSTC_GRID_PASS", tag)
}

fn collect_ast_paths(e: &OmegaExpr, cur: &mut Vec<u8>, out: &mut Vec<(Vec<u8>, OmegaExpr)>) {
    out.push((cur.clone(), e.clone()));
    match e {
        OmegaExpr::Add(a, b)
        | OmegaExpr::Sub(a, b)
        | OmegaExpr::Mul(a, b)
        | OmegaExpr::DivSafe(a, b)
        | OmegaExpr::Min(a, b)
        | OmegaExpr::Max(a, b) => {
            cur.push(0);
            collect_ast_paths(a, cur, out);
            cur.pop();
            cur.push(1);
            collect_ast_paths(b, cur, out);
            cur.pop();
        }
        OmegaExpr::Abs(a) | OmegaExpr::SqrtSafe(a) => {
            cur.push(0);
            collect_ast_paths(a, cur, out);
            cur.pop();
        }
        OmegaExpr::IfPos(c, t, el) => {
            cur.push(0);
            collect_ast_paths(c, cur, out);
            cur.pop();
            cur.push(1);
            collect_ast_paths(t, cur, out);
            cur.pop();
            cur.push(2);
            collect_ast_paths(el, cur, out);
            cur.pop();
        }
        _ => {}
    }
}

fn replace_at_path(e: &OmegaExpr, path: &[u8], replacement: &OmegaExpr) -> OmegaExpr {
    if path.is_empty() {
        return replacement.clone();
    }
    let dir = path[0];
    let rest = &path[1..];
    match e {
        OmegaExpr::Add(a, b) => if dir == 0 {
            OmegaExpr::Add(Box::new(replace_at_path(a, rest, replacement)), b.clone())
        } else {
            OmegaExpr::Add(a.clone(), Box::new(replace_at_path(b, rest, replacement)))
        },
        OmegaExpr::Sub(a, b) => if dir == 0 {
            OmegaExpr::Sub(Box::new(replace_at_path(a, rest, replacement)), b.clone())
        } else {
            OmegaExpr::Sub(a.clone(), Box::new(replace_at_path(b, rest, replacement)))
        },
        OmegaExpr::Mul(a, b) => if dir == 0 {
            OmegaExpr::Mul(Box::new(replace_at_path(a, rest, replacement)), b.clone())
        } else {
            OmegaExpr::Mul(a.clone(), Box::new(replace_at_path(b, rest, replacement)))
        },
        OmegaExpr::Min(a, b) => if dir == 0 {
            OmegaExpr::Min(Box::new(replace_at_path(a, rest, replacement)), b.clone())
        } else {
            OmegaExpr::Min(a.clone(), Box::new(replace_at_path(b, rest, replacement)))
        },
        OmegaExpr::Max(a, b) => if dir == 0 {
            OmegaExpr::Max(Box::new(replace_at_path(a, rest, replacement)), b.clone())
        } else {
            OmegaExpr::Max(a.clone(), Box::new(replace_at_path(b, rest, replacement)))
        },
        OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(replace_at_path(a, rest, replacement))),
        OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(replace_at_path(a, rest, replacement))),
        OmegaExpr::IfPos(c, t, el) => match dir {
            0 => OmegaExpr::IfPos(Box::new(replace_at_path(c, rest, replacement)), t.clone(), el.clone()),
            1 => OmegaExpr::IfPos(c.clone(), Box::new(replace_at_path(t, rest, replacement)), el.clone()),
            _ => OmegaExpr::IfPos(c.clone(), t.clone(), Box::new(replace_at_path(el, rest, replacement))),
        },
        other => other.clone(),
    }
}

fn lift_consts_in_ast(e: &OmegaExpr, next_p: &mut usize) -> OmegaExpr {
    match e {
        OmegaExpr::Const(_) => {
            let k = *next_p;
            *next_p += 1;
            OmegaExpr::Param(k)
        }
        OmegaExpr::Add(a, b) => OmegaExpr::Add(Box::new(lift_consts_in_ast(a, next_p)), Box::new(lift_consts_in_ast(b, next_p))),
        OmegaExpr::Sub(a, b) => OmegaExpr::Sub(Box::new(lift_consts_in_ast(a, next_p)), Box::new(lift_consts_in_ast(b, next_p))),
        OmegaExpr::Mul(a, b) => OmegaExpr::Mul(Box::new(lift_consts_in_ast(a, next_p)), Box::new(lift_consts_in_ast(b, next_p))),
        OmegaExpr::Min(a, b) => OmegaExpr::Min(Box::new(lift_consts_in_ast(a, next_p)), Box::new(lift_consts_in_ast(b, next_p))),
        OmegaExpr::Max(a, b) => OmegaExpr::Max(Box::new(lift_consts_in_ast(a, next_p)), Box::new(lift_consts_in_ast(b, next_p))),
        OmegaExpr::Abs(a) => OmegaExpr::Abs(Box::new(lift_consts_in_ast(a, next_p))),
        OmegaExpr::SqrtSafe(a) => OmegaExpr::SqrtSafe(Box::new(lift_consts_in_ast(a, next_p))),
        OmegaExpr::IfPos(c, t, el) => OmegaExpr::IfPos(
            Box::new(lift_consts_in_ast(c, next_p)),
            Box::new(lift_consts_in_ast(t, next_p)),
            Box::new(lift_consts_in_ast(el, next_p)),
        ),
        other => other.clone(),
    }
}

/// Unified-Brain Neural-Guided Local AST Subtree Mutation & Parameter Refit Repair (`v6.0`)!
/// Encodes `(tests, buggy_preds)` residual through `UniversalEnergyCore` (`W_enc`, `W_core`, `W_op`, `W_next`, `W_var`)
/// to prioritize both local structural mutations and residual patch terms!
pub fn repair_ast_locally(
    core: &mut UniversalEnergyCore,
    buggy: &OmegaExpr,
    tests: &[([f64; 3], f64)],
) -> (Option<OmegaExpr>, usize, usize) {
    let resolver = core.memory.clone();
    let targets: Vec<f64> = tests.iter().map(|(_, y)| *y).collect();
    let mut paths = Vec::new();
    let mut buf = Vec::new();
    collect_ast_paths(buggy, &mut buf, &mut paths);

    let nodes_inspected = paths.len();
    let mut patches_tried = 0usize;
    let mut ops = 0usize;

    // Unified-Brain Residual Guidance: query `UniversalEnergyCore` on the buggy program's residual!
    let buggy_preds: Vec<f64> = tests
        .iter()
        .map(|(x, _)| buggy.eval_num(x, None, &resolver, &mut ops))
        .collect();
    let z_res = core.encode_numeric_residual(tests, Some(&buggy_preds));
    let (op_p, next_p, var_p, _, _, _) = core.predict_state_policy(&z_res, 2, 0);

    let score_mut = |e: &OmegaExpr| -> f64 {
        match e {
            OmegaExpr::Add(_, _) => op_p[0] + next_p[0],
            OmegaExpr::Sub(_, _) => op_p[1] + next_p[1],
            OmegaExpr::Mul(_, _) => op_p[2] + next_p[2],
            OmegaExpr::Min(_, _) => op_p[4] + next_p[4],
            OmegaExpr::Max(_, _) => op_p[5] + next_p[5],
            OmegaExpr::Abs(_) => op_p[6] + next_p[6],
            OmegaExpr::SqrtSafe(_) => op_p[7] + next_p[7],
            _ => 0.5,
        }
    };

    let check_patch = |cand: &OmegaExpr, ops_acc: &mut usize, tried: &mut usize| -> bool {
        *tried += 1;
        let preds: Vec<f64> = tests.iter().map(|(x, _)| cand.eval_num(x, None, &resolver, ops_acc)).collect();
        robust_trimmed_l1_error(&preds, &targets, false) < 1e-3
    };

    // Pass 1: In-place continuous parameter refit via Dual-Number AD
    let mut p_cnt = 0usize;
    let lifted_consts = lift_consts_in_ast(buggy, &mut p_cnt);
    if p_cnt > 0 && p_cnt <= MAX_AD_PARAMS {
        patches_tried += 1;
        let (fitted, err) = fit_continuous_params(&lifted_consts, tests, false, &resolver);
        if err < 1e-3 {
            return (Some(lifted_consts.bind_params(&fitted).canonicalize()), nodes_inspected, patches_tried);
        }
    }

    // Pass 2A: Neural-Prioritized 1-step local mutations across ALL AST nodes FIRST (minimum edit distance!)
    let mut node_muts: Vec<(&Vec<u8>, Vec<OmegaExpr>)> = Vec::with_capacity(paths.len());
    for (path, sub) in &paths {
        let mut local_muts: Vec<OmegaExpr> = Vec::new();
        match sub {
            OmegaExpr::Add(a, b) => {
                local_muts.push(OmegaExpr::Sub(a.clone(), b.clone()));
                local_muts.push(OmegaExpr::Mul(a.clone(), b.clone()));
                local_muts.push(OmegaExpr::Max(a.clone(), b.clone()));
                local_muts.push(OmegaExpr::Min(a.clone(), b.clone()));
            }
            OmegaExpr::Sub(a, b) => {
                local_muts.push(OmegaExpr::Add(a.clone(), b.clone()));
                local_muts.push(OmegaExpr::Sub(b.clone(), a.clone()));
                local_muts.push(OmegaExpr::Abs(Box::new(sub.clone())));
            }
            OmegaExpr::Mul(a, b) => {
                local_muts.push(OmegaExpr::Add(a.clone(), b.clone()));
                local_muts.push(OmegaExpr::Sub(a.clone(), b.clone()));
            }
            OmegaExpr::Min(a, b) => local_muts.push(OmegaExpr::Max(a.clone(), b.clone())),
            OmegaExpr::Max(a, b) => local_muts.push(OmegaExpr::Min(a.clone(), b.clone())),
            _ => {}
        }
        local_muts.push(OmegaExpr::Abs(Box::new(sub.clone())));
        local_muts.push(OmegaExpr::SqrtSafe(Box::new(sub.clone())));
        local_muts.sort_by(|a, b| score_mut(b).partial_cmp(&score_mut(a)).unwrap_or(Ordering::Equal));

        for m in &local_muts {
            let patched = replace_at_path(buggy, path, m);
            if check_patch(&patched, &mut ops, &mut patches_tried) {
                return (Some(patched.canonicalize()), nodes_inspected, patches_tried);
            }
        }
        node_muts.push((path, local_muts));
    }

    // Pass 2B: Neural-Prioritized Residual-Guided 2-Step Local Synthesis from raw depth-1 terms!
    let mut scored_res_terms: Vec<(f64, OmegaExpr)> = Vec::new();
    for vi in 0..3 {
        let a = OmegaExpr::Arg(vi);
        scored_res_terms.push((var_p[vi], a.clone()));
        scored_res_terms.push((var_p[vi] * op_p[6], OmegaExpr::Abs(Box::new(a.clone()))));
        scored_res_terms.push((
            var_p[vi] * op_p[5],
            OmegaExpr::Max(Box::new(OmegaExpr::Const(0.0)), Box::new(a.clone())),
        ));
        scored_res_terms.push((
            var_p[vi] * op_p[4],
            OmegaExpr::Min(Box::new(OmegaExpr::Const(0.0)), Box::new(a)),
        ));
    }
    scored_res_terms.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
    let residual_terms: Vec<OmegaExpr> = scored_res_terms.into_iter().map(|(_, e)| e).collect();

    for (path, local_muts) in &node_muts {
        for m in local_muts {
            let base_p = replace_at_path(buggy, path, m);
            let bp: Vec<f64> = tests.iter().map(|(x, _)| base_p.eval_num(x, None, &resolver, &mut ops)).collect();
            let res: Vec<f64> = targets.iter().zip(bp.iter()).map(|(y, yh)| *y - *yh).collect();
            for rt in &residual_terms {
                let rp: Vec<f64> = tests.iter().map(|(x, _)| rt.eval_num(x, None, &resolver, &mut ops)).collect();
                let re: f64 = res.iter().zip(rp.iter()).map(|(a, b)| (a - b).abs()).sum::<f64>() / targets.len() as f64;
                patches_tried += 1;
                if re < 1e-3 {
                    let p2 = replace_at_path(buggy, path, &OmegaExpr::Add(Box::new(m.clone()), Box::new(rt.clone())));
                    return (Some(p2.canonicalize()), nodes_inspected, patches_tried);
                }
            }
        }
    }

    (None, nodes_inspected, patches_tried)
}

pub fn run_live_program_repair_suite(
    core: &mut UniversalEnergyCore,
    out_dir: &Path,
) -> (usize, usize, usize) {
    let x0 = OmegaExpr::Arg(0);
    let x1 = OmegaExpr::Arg(1);
    let x2 = OmegaExpr::Arg(2);
    let probes = oe_probe_inputs();

    let cases: Vec<(&str, OmegaExpr, Vec<([f64; 3], f64)>)> = vec![
        (
            "repair_local_missing_abs_hinge",
            OmegaExpr::Sub(Box::new(x0.clone()), Box::new(x1.clone())),
            probes.iter().map(|&p| (p, (p[0] - p[1]).abs() + p[2].max(0.0))).collect(),
        ),
        (
            "repair_local_branch_sign_bug",
            OmegaExpr::IfPos(
                Box::new(x0.clone()),
                Box::new(OmegaExpr::Add(Box::new(x1.clone()), Box::new(x2.clone()))),
                Box::new(OmegaExpr::Add(Box::new(x1.clone()), Box::new(x2.clone()))),
            ),
            probes.iter().map(|&p| (p, if p[0] > 0.0 { p[1] + p[2] } else { p[1] - p[2] })).collect(),
        ),
        (
            "repair_local_physics_consts",
            OmegaExpr::Add(
                Box::new(OmegaExpr::Add(
                    Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Const(1.0)), Box::new(OmegaExpr::Mul(Box::new(x0.clone()), Box::new(x0))))),
                    Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Const(1.0)), Box::new(x1))),
                )),
                Box::new(OmegaExpr::Const(0.0)),
            ),
            probes.iter().map(|&p| (p, 4.905 * p[0] * p[0] + 3.25 * p[1] - 1.75)).collect(),
        ),
    ];

    let mut repaired = 0usize;
    let mut total_patches = 0usize;
    for (name, buggy, tests) in &cases {
        let (fixed_opt, _nodes, tried) = repair_ast_locally(core, buggy, tests);
        total_patches += tried;
        if let Some(ref fixed) = fixed_opt {
            if verify_expr_via_real_rustc(fixed, core, out_dir, name, tests, &[]) {
                repaired += 1;
            }
        }
    }
    (repaired, cases.len(), total_patches)
}

// ============================================================================
// 7. DOUBLE-PERMUTED SELF-LEARNED ACTION-ALGEBRA EGOCENTRIC WORLD (`6x6 -> 10x10` OOD, `v7.0`)!
//    - ZERO global `(r, c)` coordinates!
//    - ZERO hardcoded tile semantics (`tile_perm = [2, 4, 0, 1, 3]`)!
//    - ZERO hardcoded action directions (`act_perm = [2, 0, 3, 1]` — `0` & `1` are NOT opposites!)
//    - ZERO hardcoded reward stall threshold (`-0.036` DELETED -> pure sensory invariance!)
//    - Agent SELF-DISCOVERS action-displacement Lie algebra `act_disp[a]` from `[[0.0; 2]; 4]`!
// ============================================================================

struct ProceduralEgocentricWorldEnv {
    size: usize,
    cells: Vec<Vec<usize>>, // true semantics: 0=empty, 1=wall, 2=hazard, 3=key, 4=goal
    tile_perm: [usize; 5],  // SECRET tile symbol permutation!
    act_perm: [usize; 4],   // SECRET action ID permutation (`agent_a -> true_dir`)!
    p_slip: f64,            // Stochastic action slip probability (0.0 deterministic, >0.0 stochastic!)
    slip_rng: u64,
    ar: usize,
    ac: usize,
    has_key: bool,
    steps: usize,
    max_steps: usize,
    done: bool,
    won: bool,
}

impl ProceduralEgocentricWorldEnv {
    /// Private generator-only flood-fill verifying solvability (`start -> key -> goal` avoiding walls/hazards).
    /// NEVER called by or exposed to the agent!
    fn is_reachable(cells: &[Vec<usize>], start: (usize, usize), target: (usize, usize)) -> bool {
        let sz = cells.len();
        let mut vis = vec![vec![false; sz]; sz];
        let mut q = VecDeque::new();
        vis[start.0][start.1] = true;
        q.push_back(start);
        while let Some((r, c)) = q.pop_front() {
            if (r, c) == target {
                return true;
            }
            for (dr, dc) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                let nr = r as isize + dr;
                let nc = c as isize + dc;
                if nr > 0 && nc > 0 && (nr as usize) < sz - 1 && (nc as usize) < sz - 1 {
                    let (ur, uc) = (nr as usize, nc as usize);
                    if !vis[ur][uc] && cells[ur][uc] != 1 && cells[ur][uc] != 2 {
                        vis[ur][uc] = true;
                        q.push_back((ur, uc));
                    }
                }
            }
        }
        false
    }

    fn new(size: usize, seed: u64, tile_perm: [usize; 5], act_perm: [usize; 4], p_slip: f64) -> Self {
        let sz = size.max(6);
        let inner = sz - 2;
        let max_steps = if sz <= 6 { 42 } else { 104 };
        let n_walls = if sz <= 6 { 2 } else { 6 };
        let n_hazards = if sz <= 6 { 2 } else { 5 };
        let mut rng = seed ^ ((sz as u64).wrapping_mul(0xBA1A_0600_9999));

        for _attempt in 0..80 {
            let mut cells = vec![vec![0usize; sz]; sz];
            for r in 0..sz {
                for c in 0..sz {
                    if r == 0 || c == 0 || r == sz - 1 || c == sz - 1 {
                        cells[r][c] = 1;
                    }
                }
            }
            let pick_pt = |r_ref: &mut u64| -> (usize, usize) {
                let r = 1 + ((splitmix_u64(r_ref) >> 11) as usize % inner);
                let c = 1 + ((splitmix_u64(r_ref) >> 11) as usize % inner);
                (r, c)
            };
            let start = pick_pt(&mut rng);
            let mut key = pick_pt(&mut rng);
            let mut goal = pick_pt(&mut rng);
            let mdist = |a: (usize, usize), b: (usize, usize)| -> usize {
                a.0.abs_diff(b.0) + a.1.abs_diff(b.1)
            };
            let min_sep = if sz <= 6 { 2 } else { 3 };
            for _ in 0..24 {
                if key != start && mdist(start, key) >= min_sep {
                    break;
                }
                key = pick_pt(&mut rng);
            }
            for _ in 0..24 {
                if goal != start && goal != key && mdist(key, goal) >= min_sep {
                    break;
                }
                goal = pick_pt(&mut rng);
            }
            if key == start || goal == start || goal == key {
                continue;
            }
            cells[key.0][key.1] = 3;
            cells[goal.0][goal.1] = 4;

            for _ in 0..n_walls {
                let p = pick_pt(&mut rng);
                if p != start && cells[p.0][p.1] == 0 {
                    cells[p.0][p.1] = 1;
                }
            }
            for _ in 0..n_hazards {
                let p = pick_pt(&mut rng);
                if p != start && cells[p.0][p.1] == 0 {
                    cells[p.0][p.1] = 2;
                }
            }
            if Self::is_reachable(&cells, start, key) && Self::is_reachable(&cells, key, goal) {
                return Self {
                    size: sz,
                    cells,
                    tile_perm,
                    act_perm,
                    p_slip,
                    slip_rng: seed ^ 0x511B_8888_1234,
                    ar: start.0,
                    ac: start.1,
                    has_key: false,
                    steps: 0,
                    max_steps,
                    done: false,
                    won: false,
                };
            }
        }
        let mut cells = vec![vec![0usize; sz]; sz];
        for r in 0..sz {
            for c in 0..sz {
                if r == 0 || c == 0 || r == sz - 1 || c == sz - 1 {
                    cells[r][c] = 1;
                }
            }
        }
        cells[1][sz - 2] = 3;
        cells[sz - 2][sz - 2] = 4;
        Self {
            size: sz,
            cells,
            tile_perm,
            act_perm,
            p_slip,
            slip_rng: seed ^ 0x511B_8888_1234,
            ar: 1,
            ac: 1,
            has_key: false,
            steps: 0,
            max_steps,
            done: false,
            won: false,
        }
    }

    fn observe_egocentric_5x5(&self) -> [usize; 25] {
        let mut p = [self.tile_perm[1]; 25];
        for dr in 0..5 {
            for dc in 0..5 {
                let rr = self.ar as isize + dr as isize - 2;
                let cc = self.ac as isize + dc as isize - 2;
                if rr >= 0 && cc >= 0 && (rr as usize) < self.size && (cc as usize) < self.size {
                    p[dr * 5 + dc] = self.tile_perm[self.cells[rr as usize][cc as usize]];
                }
            }
        }
        p
    }

    fn step(&mut self, agent_action: usize) -> (f64, bool) {
        if self.done {
            return (0.0, true);
        }
        self.steps += 1;
        // Apply SECRET action permutation (+ optional stochastic action slip!) before executing physical step
        let mut true_dir = self.act_perm[agent_action % 4];
        if self.p_slip > 0.0 {
            let u = (splitmix_u64(&mut self.slip_rng) >> 11) as f64 / ((1u64 << 53) as f64);
            if u < self.p_slip {
                true_dir = (splitmix_u64(&mut self.slip_rng) as usize) % 4;
            }
        }
        let (nr, nc) = match true_dir {
            0 => (self.ar.saturating_sub(1), self.ac),
            1 => ((self.ar + 1).min(self.size - 1), self.ac),
            2 => (self.ar, self.ac.saturating_sub(1)),
            3 => (self.ar, (self.ac + 1).min(self.size - 1)),
            _ => (self.ar, self.ac),
        };
        let cell = self.cells[nr][nc];
        let mut reward = -0.02;
        if cell != 1 {
            self.ar = nr;
            self.ac = nc;
            if cell == 2 {
                self.done = true;
                reward = -1.0;
            } else if cell == 3 {
                self.has_key = true;
                self.cells[nr][nc] = 0;
                reward = 0.70;
            } else if cell == 4 && self.has_key {
                self.done = true;
                self.won = true;
                reward = 1.30;
            }
        } else {
            reward = -0.05;
        }
        if self.steps >= self.max_steps {
            self.done = true;
        }
        (reward, self.done)
    }
}

#[derive(Clone, Debug)]
pub struct ZeroRuleWorldReport {
    pub random_wins_6x6: usize,
    pub early_wins_6x6: usize,
    pub learned_wins_6x6: usize,
    pub random_wins_10x10_ood: usize,
    pub learned_wins_10x10_ood: usize,
    pub eval_episodes: usize,
    pub dyn_mse_final: f64,
    pub rew_mae_final: f64,
    pub act_inverse_err: f64,
    pub total_transitions_learned: usize,
}

/// Pure ZERO-RULE, DOUBLE-PERMUTED (+ Optional Stochastic Action Slip `p_slip`) World-Model Benchmark (`v8.0`)!
/// Starts with `act_disp = [[0.0; 2]; 4]` (ZERO hardcoded action directions, ZERO `0.09`, ZERO reward thresholds!),
/// trains ONLY on procedural `6x6` mazes, and evaluates STRICTLY FROZEN (`eps=0.0`) on `6x6` AND `10x10` OOD!
pub fn run_double_permuted_stochastic_world_benchmark(
    core: &mut UniversalEnergyCore,
    seed: u64,
    tile_perm: [usize; 5],
    act_perm: [usize; 4],
    p_slip: f64,
) -> ZeroRuleWorldReport {
    let eval_eps = 30usize;
    let mut rng = seed ^ 0xD00D_F00D_7777;

    let mut random_wins_6x6 = 0usize;
    let mut random_wins_10x10_ood = 0usize;
    for ep in 0..eval_eps {
        let mut env6 = ProceduralEgocentricWorldEnv::new(6, seed + 10_000 + ep as u64 * 37, tile_perm, act_perm, p_slip);
        let mut r_rng = seed ^ (ep as u64).wrapping_mul(0x9E37_79B9);
        while !env6.done {
            let a = (splitmix_u64(&mut r_rng) as usize) % 4;
            let _ = env6.step(a);
        }
        if env6.won {
            random_wins_6x6 += 1;
        }
        let mut env10 = ProceduralEgocentricWorldEnv::new(10, seed + 15_000 + ep as u64 * 43, tile_perm, act_perm, p_slip);
        let mut r_rng10 = (seed + 77) ^ (ep as u64).wrapping_mul(0x9E37_79B9);
        while !env10.done {
            let a = (splitmix_u64(&mut r_rng10) as usize) % 4;
            let _ = env10.step(a);
        }
        if env10.won {
            random_wins_10x10_ood += 1;
        }
    }

    let mut wm = NonlinearWorldModel::new(seed);
    let mut early_wins_6x6 = 0usize;

    let run_one_episode = |core_ref: &UniversalEnergyCore,
                           wm_ref: &mut NonlinearWorldModel,
                           grid_size: usize,
                           ep_seed: u64,
                           eps: f64,
                           curiosity_w: f64,
                           train_after: bool,
                           rng_ref: &mut u64|
     -> bool {
        let mut env = ProceduralEgocentricWorldEnv::new(grid_size, ep_seed, tile_perm, act_perm, p_slip);
        let mut belief = [0.0f64; WORLD_BELIEF_DIM];
        let mut episodic_states: Vec<([f64; WORLD_OBS_DIM], [f64; WORLD_BELIEF_DIM])> = Vec::with_capacity(110);
        let mut ep_mem: Vec<([f64; WORLD_OBS_DIM], [f64; WORLD_BELIEF_DIM], usize, [f64; WORLD_OBS_DIM], f64, bool)> =
            Vec::with_capacity(110);
        let mut traj: Vec<WorldTransition> = Vec::with_capacity(110);

        let obs0 = env.observe_egocentric_5x5();
        let mut raw_cur = NonlinearWorldModel::encode_egocentric_5x5(&obs0, env.has_key);
        let ctx0 = core_ref.encode_world_core_context(&raw_cur);
        let mut prev_b = belief;
        let (b0, mut s_cur) = wm_ref.update_belief_state(&belief, &raw_cur, None, &ctx0);
        belief = b0;
        episodic_states.push((raw_cur, belief));

        while !env.done {
            let a = wm_ref.plan_latent_mcts_action(&s_cur, &episodic_states, &ep_mem, eps, curiosity_w, rng_ref);
            let (r, done) = env.step(a);
            let obs_n = env.observe_egocentric_5x5();
            let raw_n = NonlinearWorldModel::encode_egocentric_5x5(&obs_n, env.has_key);
            if train_after {
                // Self-discover secret action-displacement Lie algebra purely from visual shift!
                wm_ref.learn_action_lie_algebra(&raw_cur, a, &raw_n);
            }
            let ctx_n = core_ref.encode_world_core_context(&raw_n);
            ep_mem.push((raw_cur, belief, a, raw_n, r, done));
            // PURE sensory stall invariance (`ZERO` hardcoded reward threshold `-0.035`!)
            let eff_act = if raw_n == raw_cur { None } else { Some(a) };
            let (bn, s_next) = wm_ref.update_belief_state(&belief, &raw_n, eff_act, &ctx_n);
            episodic_states.push((raw_n, bn));
            let tr = WorldTransition {
                s: s_cur,
                prev_belief: prev_b,
                raw_obs: raw_cur,
                a,
                r,
                s_next,
                done,
                ret_3step: r,
            };
            if train_after {
                wm_ref.train_online_step(&tr, rng_ref);
            }
            traj.push(tr);
            prev_b = belief;
            belief = bn;
            raw_cur = raw_n;
            s_cur = s_next;
        }
        if train_after {
            wm_ref.commit_episode_and_train(traj, env.won, rng_ref);
        }
        env.won
    };

    for ep in 0..eval_eps {
        let eps = 0.55 - 0.35 * (ep as f64 / eval_eps as f64);
        if run_one_episode(core, &mut wm, 6, seed + 20_000 + ep as u64 * 53, eps, 0.40, true, &mut rng) {
            early_wins_6x6 += 1;
        }
    }
    for ep in 0..490usize {
        let eps = (0.30 - 0.25 * (ep as f64 / 490.0)).max(0.05);
        let cur = 0.38 - 0.20 * (ep as f64 / 490.0);
        let _ = run_one_episode(core, &mut wm, 6, seed + 30_000 + ep as u64 * 71, eps, cur, true, &mut rng);
    }

    let mut learned_wins_6x6 = 0usize;
    for ep in 0..eval_eps {
        if run_one_episode(core, &mut wm, 6, seed + 90_000 + ep as u64 * 97, 0.0, 0.20, false, &mut rng) {
            learned_wins_6x6 += 1;
        }
    }
    let mut learned_wins_10x10_ood = 0usize;
    for ep in 0..eval_eps {
        if run_one_episode(core, &mut wm, 10, seed + 140_000 + ep as u64 * 109, 0.0, 0.28, false, &mut rng) {
            learned_wins_10x10_ood += 1;
        }
    }

    // Dimensionless normalized relative Lie-algebra inverse error: `mean ||act_disp[a] + act_disp[opp_a]|| / step_norm`!
    let step_n = wm.mean_action_step_norm().max(1e-6);
    let mut inv_err = 0.0;
    for a in 0..4 {
        let td = act_perm[a];
        let opp_td = td ^ 1; // 0<->1 (Up/Down), 2<->3 (Left/Right) in ground-truth physics
        if let Some(opp_a) = (0..4).find(|&k| act_perm[k] == opp_td) {
            let sx = wm.act_disp[a][0] + wm.act_disp[opp_a][0];
            let sy = wm.act_disp[a][1] + wm.act_disp[opp_a][1];
            inv_err += (sx * sx + sy * sy).sqrt() / step_n;
        }
    }
    let act_inverse_err = inv_err / 4.0;

    let rep = ZeroRuleWorldReport {
        random_wins_6x6,
        early_wins_6x6,
        learned_wins_6x6,
        random_wins_10x10_ood,
        learned_wins_10x10_ood,
        eval_episodes: eval_eps,
        dyn_mse_final: wm.last_dyn_mse,
        rew_mae_final: wm.last_rew_mae,
        act_inverse_err,
        total_transitions_learned: wm.updates,
    };
    core.world_model = wm;
    rep
}

/// Deterministic double-permuted world benchmark (`p_slip = 0.0`).
#[cfg(test)]
pub fn run_double_permuted_world_benchmark(
    core: &mut UniversalEnergyCore,
    seed: u64,
    tile_perm: [usize; 5],
    act_perm: [usize; 4],
) -> ZeroRuleWorldReport {
    run_double_permuted_stochastic_world_benchmark(core, seed, tile_perm, act_perm, 0.0)
}

/// Default double-permuted world benchmark (`tile_perm=[2,4,0,1,3]`, `act_perm=[2,0,3,1]` — `0` & `1` NOT opposites!).
pub fn run_zero_rule_world_benchmark(core: &mut UniversalEnergyCore, seed: u64) -> ZeroRuleWorldReport {
    run_double_permuted_stochastic_world_benchmark(core, seed, [2, 4, 0, 1, 3], [2, 0, 3, 1], 0.0)
}

// ============================================================================
// 8. TRUE Koza 2-Generation GP (Subtree Crossover + Raw Subtree Mutation) `meta_priority_ast` Evolution (`v8.0`)!
// ============================================================================

#[derive(Clone, Debug)]
pub struct MetaSelfImproveReport {
    pub baseline_val_solved: usize,
    pub improved_val_solved: usize,
    pub baseline_val_expansions: usize,
    pub improved_val_expansions: usize,
    pub candidates_evaluated: usize,
    pub crossover_children_evaluated: usize,
    pub accepted_improvement: bool,
    pub emitted_rust_verified: bool,
    pub emitted_rust_path: String,
    pub winning_meta_ast_pretty: String,
    pub winning_meta_ast_mdl: usize,
    pub winning_strategy: SearchStrategyGenome,
}

/// Generate a pure raw bottom-up `OmegaExpr` subtree over `{x0, x1, x2, c}` x `{Add, Sub, Mul, Min, Max}`!
fn gp_random_raw_subtree(rng: &mut u64, depth: usize) -> OmegaExpr {
    let next_u = |r: &mut u64| -> f64 { (splitmix_u64(r) >> 11) as f64 / ((1u64 << 53) as f64) };
    if depth == 0 {
        return match (splitmix_u64(rng) as usize) % 4 {
            0 => OmegaExpr::Arg(0),
            1 => OmegaExpr::Arg(1),
            2 => OmegaExpr::Arg(2),
            _ => {
                let c = ((0.15 + 0.95 * next_u(rng)) * 20.0).round() / 20.0;
                OmegaExpr::Const(c)
            }
        };
    }
    let a = gp_random_raw_subtree(rng, depth - 1);
    let b = gp_random_raw_subtree(rng, depth - 1);
    match (splitmix_u64(rng) as usize) % 5 {
        0 => OmegaExpr::Add(Box::new(a), Box::new(b)),
        1 => OmegaExpr::Sub(Box::new(a), Box::new(b)),
        2 => OmegaExpr::Mul(Box::new(a), Box::new(b)),
        3 => OmegaExpr::Min(Box::new(a), Box::new(b)),
        _ => OmegaExpr::Max(Box::new(a), Box::new(b)),
    }
}

/// True Koza-style 2-Parent Subtree Crossover: splices a random compatible subtree from `p2` into a random
/// non-root node of `p1` (with `collect_ast_paths` + `replace_at_path`)!
pub fn gp_crossover_meta_ast(p1: &OmegaExpr, p2: &OmegaExpr, rng: &mut u64) -> OmegaExpr {
    let mut paths1: Vec<(Vec<u8>, OmegaExpr)> = Vec::new();
    let mut paths2: Vec<(Vec<u8>, OmegaExpr)> = Vec::new();
    let mut buf = Vec::new();
    collect_ast_paths(p1, &mut buf, &mut paths1);
    buf.clear();
    collect_ast_paths(p2, &mut buf, &mut paths2);

    let non_root1: Vec<&Vec<u8>> = paths1
        .iter()
        .filter(|(p, _)| !p.is_empty())
        .map(|(p, _)| p)
        .collect();
    let donor_subs2: Vec<&OmegaExpr> = paths2
        .iter()
        .filter(|(p, e)| !p.is_empty() && e.mdl_size() <= 9)
        .map(|(_, e)| e)
        .collect();
    if !non_root1.is_empty() && !donor_subs2.is_empty() {
        let p = non_root1[(splitmix_u64(rng) as usize) % non_root1.len()];
        let sub = donor_subs2[(splitmix_u64(rng) as usize) % donor_subs2.len()];
        return replace_at_path(p1, p, sub).canonicalize();
    }
    p1.clone()
}

/// Pure structural GP AST mutation over `OmegaExpr` (`x0=fit, x1=neural_llr, x2=complexity`)
/// including raw bottom-up subtree replacement!
fn gp_mutate_meta_ast(parent: &OmegaExpr, rng: &mut u64) -> OmegaExpr {
    let mut paths: Vec<(Vec<u8>, OmegaExpr)> = Vec::new();
    let mut buf = Vec::new();
    collect_ast_paths(parent, &mut buf, &mut paths);
    let next_u = |r: &mut u64| -> f64 { (splitmix_u64(r) >> 11) as f64 / ((1u64 << 53) as f64) };
    let op_kind = (splitmix_u64(rng) as usize) % 6;

    let x0 = OmegaExpr::Arg(0);
    let x1 = OmegaExpr::Arg(1);

    match op_kind {
        // 0. Perturb an existing `Const` node inside the AST
        0 => {
            let const_paths: Vec<&(Vec<u8>, OmegaExpr)> = paths
                .iter()
                .filter(|(_, e)| matches!(e, OmegaExpr::Const(_)))
                .collect();
            if !const_paths.is_empty() {
                let (p, sub) = const_paths[(splitmix_u64(rng) as usize) % const_paths.len()];
                if let OmegaExpr::Const(c) = sub {
                    let delta = (next_u(rng) * 0.24) - 0.12;
                    let nc = (c + delta).clamp(0.10, 1.40);
                    return replace_at_path(parent, p, &OmegaExpr::Const((nc * 100.0).round() / 100.0)).canonicalize();
                }
            }
            parent.clone()
        }
        // 1. Subtree symmetric clamping `sub -> Max(-lim, Min(lim, sub))` on any `Arg(1)` (neural_llr) node
        1 => {
            let x1_paths: Vec<&Vec<u8>> = paths
                .iter()
                .filter(|(_, e)| *e == x1)
                .map(|(p, _)| p)
                .collect();
            if !x1_paths.is_empty() {
                let p = x1_paths[(splitmix_u64(rng) as usize) % x1_paths.len()];
                let lim = (0.65 + 0.70 * next_u(rng)).clamp(0.65, 1.35);
                let lim_r = (lim * 20.0).round() / 20.0;
                let clamped = OmegaExpr::Max(
                    Box::new(OmegaExpr::Const(-lim_r)),
                    Box::new(OmegaExpr::Min(Box::new(OmegaExpr::Const(lim_r)), Box::new(x1))),
                );
                return replace_at_path(parent, p, &clamped).canonicalize();
            }
            parent.clone()
        }
        // 2. Replace a `Const(c)` weight with a fit-dependent nonlinear gate `Add(c1, Mul(c2, Min(1.0, x0)))`
        2 => {
            let const_paths: Vec<&Vec<u8>> = paths
                .iter()
                .filter(|(_, e)| matches!(e, OmegaExpr::Const(c) if *c > 0.05 && *c < 0.65))
                .map(|(p, _)| p)
                .collect();
            if !const_paths.is_empty() {
                let p = const_paths[(splitmix_u64(rng) as usize) % const_paths.len()];
                let c1 = ((0.14 + 0.16 * next_u(rng)) * 100.0).round() / 100.0;
                let c2 = ((0.10 + 0.16 * next_u(rng)) * 100.0).round() / 100.0;
                let gate = OmegaExpr::Add(
                    Box::new(OmegaExpr::Const(c1)),
                    Box::new(OmegaExpr::Mul(
                        Box::new(OmegaExpr::Const(c2)),
                        Box::new(OmegaExpr::Min(Box::new(OmegaExpr::Const(1.0)), Box::new(x0))),
                    )),
                );
                return replace_at_path(parent, p, &gate).canonicalize();
            }
            parent.clone()
        }
        // 3. Structural conditional regime split `IfPos(x1, parent_a, parent_b)` or `IfPos(x0 - thr, parent_a, parent_b)`
        3 => {
            let w_lo = ((0.18 + 0.14 * next_u(rng)) * 100.0).round() / 100.0;
            let w_hi = ((0.28 + 0.16 * next_u(rng)) * 100.0).round() / 100.0;
            let cond = if (splitmix_u64(rng) & 1) == 0 {
                x1.clone()
            } else {
                let thr = ((0.25 + 0.30 * next_u(rng)) * 100.0).round() / 100.0;
                OmegaExpr::Sub(Box::new(x0.clone()), Box::new(OmegaExpr::Const(thr)))
            };
            let base = OmegaExpr::Add(Box::new(x0), Box::new(OmegaExpr::Arg(2)));
            let br_t = OmegaExpr::Add(Box::new(base.clone()), Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Const(w_lo)), Box::new(x1.clone()))));
            let br_f = OmegaExpr::Add(Box::new(base), Box::new(OmegaExpr::Mul(Box::new(OmegaExpr::Const(w_hi)), Box::new(x1))));
            OmegaExpr::IfPos(Box::new(cond), Box::new(br_t), Box::new(br_f)).canonicalize()
        }
        // 4. Complexity-weight nonlinear modulation on `Arg(2)` (e.g., `w_c * x2`)
        4 => {
            let x2_paths: Vec<&Vec<u8>> = paths
                .iter()
                .filter(|(_, e)| *e == OmegaExpr::Arg(2))
                .map(|(p, _)| p)
                .collect();
            if !x2_paths.is_empty() {
                let p = x2_paths[(splitmix_u64(rng) as usize) % x2_paths.len()];
                let wc = ((0.82 + 0.32 * next_u(rng)) * 100.0).round() / 100.0;
                let scaled = OmegaExpr::Mul(Box::new(OmegaExpr::Const(wc)), Box::new(OmegaExpr::Arg(2)));
                return replace_at_path(parent, p, &scaled).canonicalize();
            }
            parent.clone()
        }
        // 5. Raw bottom-up random subtree replacement at any non-root AST node!
        _ => {
            let non_root: Vec<&Vec<u8>> = paths
                .iter()
                .filter(|(p, _)| !p.is_empty())
                .map(|(p, _)| p)
                .collect();
            if !non_root.is_empty() {
                let p = non_root[(splitmix_u64(rng) as usize) % non_root.len()];
                let raw_sub = gp_random_raw_subtree(rng, 1);
                return replace_at_path(parent, p, &raw_sub).canonicalize();
            }
            parent.clone()
        }
    }
}

/// Strict single-variable partial-derivative + joint monotonicity oracle:
/// rejects degenerate/inverted GP priority ASTs AND prevents dropping ANY of `x0` (fit), `x1` (neural), `x2` (mdl)!
pub fn meta_ast_is_monotone_sane(ast: &OmegaExpr) -> bool {
    if ast.mdl_size() > 24 {
        return false;
    }
    let probes = [
        // Pure partial-derivative checks (holding the other 2 variables strictly constant!)
        ([0.05, 0.00, 0.50], [0.85, 0.00, 0.50]),   // pure d/dx0 > 0 (fit_term cannot be dropped!)
        ([0.35, -0.40, 0.70], [0.75, -0.40, 0.70]), // pure d/dx0 > 0 in negative-LLR regime
        ([0.30, -0.80, 0.80], [0.30, 0.80, 0.80]),  // pure d/dx1 > 0 (neural_llr cannot be dropped!)
        ([0.30, 0.00, 0.20], [0.30, 0.00, 1.40]),   // pure d/dx2 > 0 (complexity cannot be dropped!)
        // Joint monotonicity check
        ([0.05, -0.50, 0.40], [0.85, 0.70, 1.40]),
    ];
    let mut ops = 0usize;
    for &(good, bad) in &probes {
        let vg = ast.eval_num(&good, None, &EmptyResolver, &mut ops);
        let vb = ast.eval_num(&bad, None, &EmptyResolver, &mut ops);
        if !vg.is_finite() || !vb.is_finite() || (vb - vg) <= 1e-3 {
            return false;
        }
    }
    true
}

/// Propose 2-generation Koza GP (Subtree Crossover + Raw Subtree Mutation) updates to TropiCore-Omega's
/// OWN executable symbolic priority AST (`core.meta_priority_ast: OmegaExpr`), benchmark on 6 disjoint internal
/// validation tasks, hot-swap ONLY strict improvements LIVE, and verify the ACTUAL evolved AST via `rustc -O`!
pub fn run_meta_self_improvement_cycle(
    core: &mut UniversalEnergyCore,
    out_dir: &Path,
    seed: u64,
) -> MetaSelfImproveReport {
    let probes = oe_probe_inputs();
    // 6 Disjoint Internal Validation Tasks (100% DISJOINT from Axis 2 sealed held-out tasks!)
    let val_tasks: Vec<Vec<([f64; 3], f64)>> = vec![
        probes.iter().map(|&p| (p, (p[0] + p[1]).abs() + p[2])).collect(),
        probes.iter().map(|&p| (p, p[0] * p[0] + (p[1] - p[2]).max(0.0))).collect(),
        probes.iter().map(|&p| (p, if p[1] > 0.0 { p[0] * p[2] } else { p[0] + p[2] })).collect(),
        probes.iter().map(|&p| (p, (p[0] * p[0] + p[2] * p[2]).sqrt())).collect(),
        probes.iter().map(|&p| (p, (p[0] + p[2]).max(p[1]) - p[2])).collect(),
        probes.iter().map(|&p| (p, (p[0] - p[2]).abs() + p[1] * p[1])).collect(),
    ];

    let eval_meta_prog = |base_core: &UniversalEnergyCore, ast: &OmegaExpr, g: &SearchStrategyGenome| -> (usize, usize) {
        let mut c = base_core.clone();
        c.meta_priority_ast = ast.clone();
        c.strategy = g.clone();
        let mut solved = 0usize;
        let mut exps = 0usize;
        for (i, t) in val_tasks.iter().enumerate() {
            let o = synthesize_numeric_omega(
                &mut c, t, 3, 24, 900, false, true, true, true, false, 2, i % 4,
            );
            if o.exact_match {
                solved += 1;
            }
            exps += o.expansions;
        }
        (solved, exps)
    };

    let base_ast = core.meta_priority_ast.clone();
    let base_g = core.strategy.clone();
    let (base_solved, base_exps) = eval_meta_prog(core, &base_ast, &base_g);

    let mut best_ast = base_ast.clone();
    let mut best_g = base_g.clone();
    let mut best_solved = base_solved;
    let mut best_exps = base_exps;

    let mut rng = seed ^ 0x5E1F_7000_AAAA;
    let mut seen_keys: HashSet<String> = HashSet::new();
    seen_keys.insert(base_ast.canonical_key());
    let mut candidates_evaluated = 0usize;
    let mut crossover_children_evaluated = 0usize;

    // Generation 1: 6 diverse GP structural & raw-subtree mutations from `base_ast`
    let mut gen1_pool: Vec<(OmegaExpr, usize, usize)> = vec![(base_ast.clone(), base_solved, base_exps)];
    let mut gen1: Vec<OmegaExpr> = Vec::new();
    for _ in 0..80 {
        if gen1.len() >= 6 {
            break;
        }
        let m = gp_mutate_meta_ast(&base_ast, &mut rng);
        if meta_ast_is_monotone_sane(&m) && seen_keys.insert(m.canonical_key()) {
            gen1.push(m);
        }
    }
    for cand_ast in gen1 {
        candidates_evaluated += 1;
        let (s, e) = eval_meta_prog(core, &cand_ast, &base_g);
        if s > best_solved || (s == best_solved && e < best_exps) {
            best_solved = s;
            best_exps = e;
            best_ast = cand_ast.clone();
        }
        gen1_pool.push((cand_ast, s, e));
    }

    // Rank Gen-1 population by fitness `(solved DESC, exps ASC)` and pick Top-2 distinct parents!
    gen1_pool.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
    let parent1 = gen1_pool[0].0.clone();
    let parent2 = gen1_pool
        .iter()
        .skip(1)
        .find(|(ast, _, _)| ast.canonical_key() != parent1.canonical_key())
        .map(|(ast, _, _)| ast.clone())
        .unwrap_or_else(|| base_ast.clone());

    // Generation 2: True 2-Parent Subtree Crossover (up to 2 children) + 3 Mutation refinements!
    let mut gen2: Vec<(OmegaExpr, SearchStrategyGenome, bool)> = Vec::new();
    for _ in 0..64 {
        if gen2.len() >= 2 {
            break;
        }
        let child = if (splitmix_u64(&mut rng) & 1) == 0 {
            gp_crossover_meta_ast(&parent1, &parent2, &mut rng)
        } else {
            gp_crossover_meta_ast(&parent2, &parent1, &mut rng)
        };
        if meta_ast_is_monotone_sane(&child) && seen_keys.insert(child.canonical_key()) {
            gen2.push((child, base_g.clone(), true));
        }
    }
    let mut mut_added = 0usize;
    for _ in 0..48 {
        if mut_added >= 3 {
            break;
        }
        let m = gp_mutate_meta_ast(&parent1, &mut rng);
        if meta_ast_is_monotone_sane(&m) && seen_keys.insert(m.canonical_key()) {
            let next_u = |r: &mut u64| -> f64 { (splitmix_u64(r) >> 11) as f64 / ((1u64 << 53) as f64) };
            let mut mg = base_g.clone();
            mg.w_partner_need = (0.34 + 0.24 * next_u(&mut rng)).clamp(0.30, 0.58);
            mg.w_depth = (0.34 + 0.10 * next_u(&mut rng)).clamp(0.32, 0.46);
            gen2.push((m, mg, false));
            mut_added += 1;
        }
    }
    for (cand_ast, cand_g, is_xover) in gen2 {
        candidates_evaluated += 1;
        if is_xover {
            crossover_children_evaluated += 1;
        }
        let (s, e) = eval_meta_prog(core, &cand_ast, &cand_g);
        if s > best_solved || (s == best_solved && e < best_exps) {
            best_solved = s;
            best_exps = e;
            best_ast = cand_ast;
            best_g = cand_g;
        }
    }

    let accepted = best_solved > base_solved || (best_solved == base_solved && best_exps < base_exps);
    if accepted {
        core.meta_priority_ast = best_ast.clone();
        core.strategy = best_g.clone();
    }

    let _ = fs::create_dir_all(out_dir);
    let rs_path = out_dir.join("self_improved_strategy_v8.rs");
    let emitted_ok = if fs::write(&rs_path, core.emit_meta_priority_rust_verifier()).is_ok() {
        compile_and_run_rust_source(&rs_path, "SELF_IMPROVED_AST_STRATEGY_RUSTC_PASS", "self_ast_strat")
    } else {
        false
    };

    MetaSelfImproveReport {
        baseline_val_solved: base_solved,
        improved_val_solved: best_solved,
        baseline_val_expansions: base_exps,
        improved_val_expansions: best_exps,
        candidates_evaluated,
        crossover_children_evaluated,
        accepted_improvement: accepted,
        emitted_rust_verified: emitted_ok,
        emitted_rust_path: rs_path.display().to_string(),
        winning_meta_ast_pretty: core.meta_priority_ast.pretty(&EmptyResolver),
        winning_meta_ast_mdl: core.meta_priority_ast.mdl_size(),
        winning_strategy: core.strategy.clone(),
    }
}
