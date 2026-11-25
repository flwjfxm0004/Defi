//! Live Self-Hosting RSI Target Module (`TropiCore-Omega v10.0`).
//!
//! This module is compiled directly into `tropicore_omega` and called LIVE inside:
//!   1. General SSA Register-Machine Imperative Synthesizer (`src/reg_synth.rs`)
//!   2. Nonlinear Neural-Ensemble Continuous CEM Planner (`src/world_asi.rs`)
//!
//! The `SelfHostingRepoRsi` engine reads THIS exact source file into `UniversalEnergyCore`,
//! proposes structural Rust mutations, compiles and evaluates them under `rustc -O -D warnings`
//! against a sealed multi-domain OOD Pareto benchmark, and (under `--apply-rsi`) atomically
//! upgrades this file with SHA-256 chain provenance and automatic rollback protection!

/// Priority score for ranking imperative SSA register-machine candidates (LOWER = search earlier).
#[inline]
pub fn synth_candidate_priority(
    train_err: f64,
    neural_val_prob: f64,
    prog_len: usize,
    skill_bonus: f64,
) -> f64 {
    let err_term = (1.0 + train_err.abs()).ln();
    let neural_nll = -(neural_val_prob.clamp(0.02, 0.98)).ln();
    let len_pen = 0.28 * (prog_len as f64);
    err_term + 0.55 * neural_nll + len_pen - 0.35 * skill_bonus
}

/// Per-step receding-horizon trajectory cost for Nonlinear Neural-Ensemble Continuous CEM planning
/// (LOWER = preferred trajectory).
#[inline]
pub fn cem_step_cost(
    goal_dist: f64,
    obs_dist: f64,
    vel_norm: f64,
    u_norm_sq: f64,
    epistemic_disagreement: f64,
    is_terminal: bool,
) -> f64 {
    let w_goal = if is_terminal { 2.8 } else { 1.0 };
    let obs_pen = if obs_dist < 1.05 {
        let m = 1.05 - obs_dist;
        9.5 * m + 6.0 * m * m
    } else {
        0.0
    };
    let unc_pen = 0.35 * epistemic_disagreement;
    let speed_brake = if goal_dist < 0.85 { 0.14 * vel_norm } else { 0.0 };
    w_goal * goal_dist + obs_pen + unc_pen + speed_brake + 0.02 * u_norm_sq
}
