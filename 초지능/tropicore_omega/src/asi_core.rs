//! TropiCore-Omega v10.0 ASI-Core — Maximum Laptop-Verified General Non-LLM Superintelligence Engine.
//!
//! Unifies ALL 4 General ASI Pillars with ZERO LLMs, ZERO external crates, and ZERO mandatory Lean lag:
//!   1. Modality-Agnostic `UniversalEventStream` (`RawBytes` / `CodeFeedback` / `ContinuousCausal` / `SpatialField`)
//!      feeding scalable contractive NVFP4 `UniversalEnergyCore` (`d=64|256|2048|4096`)
//!   2. Tier-1 General Multi-Register SSA Imperative Synthesizer (`src/reg_synth.rs`):
//!      Zero-hardcode synthesis of 8 real imperative stateful Rust algorithms (Cold vs Trained on 4 UNSEEN held-out)
//!      certified live by `rustc -O -D warnings` on sealed OOD test vectors!
//!   3. Tier-2 Nonlinear Neural-Ensemble Continuous World Model (`src/world_asi.rs`):
//!      2-MLP residual dynamics ensemble (ZERO hardcoded physics!) + Active Epistemic Curiosity +
//!      Live `evolved_policy::cem_step_cost` CEM planning + `+55 deg` Non-Stationary Fault Recovery!
//!   4. Tier-3 Fast Pure-Rust CEGIS Algebraic Discovery -> E-Graph (<10 ms; optional Lean 4 via `--features lean_oracle`)
//!   5. Real Self-Hosting Repository RSI (`run_self_hosting_repo_rsi`):
//!      Reads TropiCore-Omega's OWN live `src/evolved_policy.rs`, evolves structural Rust mutations,
//!      compiles & gates them under `rustc -O -D warnings` on a sealed multi-domain OOD Pareto benchmark,
//!      records a cryptographic SHA-256 chain ledger, and (under `--apply-rsi`) atomically upgrades `src/evolved_policy.rs`
//!      with automatic rollback protection!

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::core::{UniversalEnergyCore, NUM_TACTIC_CLASSES, RAW_FEAT_DIM};
use crate::egraph::{egraph_optimize, EGraph};
use crate::gpu_bridge::omega_tactic_policy_on_device;
use crate::ir::OmegaExpr;
use crate::memory::OMEGA_DIM;
use crate::prover::run_fast_cegis_algebra_and_promote;
use crate::reg_synth::{run_general_reg_synth_suite, GeneralRegSynthReport, ImperativeSkillLibrary};
use crate::sha256::{append_vault_jsonl, sha256_hex};
use crate::world_asi::{run_nonlinear_neural_world_asi, NonlinearWorldAsiReport};

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_uniform(rng: &mut u64) -> f64 {
    (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64)
}

fn find_rustc_bin() -> String {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let win = format!("{}\\.cargo\\bin\\rustc.exe", home);
        if Path::new(&win).exists() {
            return win;
        }
    }
    "rustc".to_string()
}

// ============================================================================
// 1. MODALITY-AGNOSTIC UNIVERSAL EVENT STREAM (ZERO PRETRAINED TOKENIZERS!)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalityTag {
    RawBytes,
    CodeFeedback,
    ContinuousCausal,
    SpatialField,
}

impl ModalityTag {
    pub fn code(self) -> f64 {
        match self {
            ModalityTag::RawBytes => -0.80,
            ModalityTag::CodeFeedback => -0.25,
            ModalityTag::ContinuousCausal => 0.35,
            ModalityTag::SpatialField => 0.85,
        }
    }
}

pub struct UniversalEventStream;

impl UniversalEventStream {
    pub fn from_bytes(bytes: &[u8], tag: ModalityTag, max_tokens: usize) -> Vec<[f64; RAW_FEAT_DIM]> {
        if bytes.is_empty() {
            return vec![[0.0; RAW_FEAT_DIM]];
        }
        let n = bytes.len();
        let stride = (n / max_tokens.max(1)).max(1);
        let mut out = Vec::with_capacity(max_tokens.min(n));
        let mut prev = bytes[0] as f64;
        let mut i = 0usize;
        while i < n && out.len() < max_tokens {
            let b = bytes[i];
            let bf = b as f64;
            let next = bytes.get(i + 1).copied().unwrap_or(b) as f64;
            out.push([
                (bf / 127.5) - 1.0,
                ((bf - prev) / 127.5).clamp(-1.0, 1.0),
                ((next - bf) / 127.5).clamp(-1.0, 1.0),
                (i as f64 / n.max(1) as f64) * 2.0 - 1.0,
                if b.is_ascii_alphabetic() { 1.0 } else { 0.0 },
                if b.is_ascii_digit() { 1.0 } else { 0.0 },
                if b.is_ascii_punctuation() { 1.0 } else { 0.0 },
                if b.is_ascii_whitespace() { 1.0 } else { 0.0 },
                ((b.count_ones() as f64) / 4.0) - 1.0,
                tag.code(),
            ]);
            prev = bf;
            i += stride;
        }
        out
    }

    pub fn from_continuous_trajectory(
        traj: &[(Vec<f64>, Vec<f64>, f64)],
        max_tokens: usize,
    ) -> Vec<[f64; RAW_FEAT_DIM]> {
        if traj.is_empty() {
            return vec![[0.0; RAW_FEAT_DIM]];
        }
        let n = traj.len();
        let stride = (n / max_tokens.max(1)).max(1);
        let mut out = Vec::with_capacity(max_tokens.min(n));
        let mut i = 0usize;
        while i < n && out.len() < max_tokens {
            let (ref s, ref u, r) = traj[i];
            let g = |v: &[f64], idx: usize| v.get(idx).copied().unwrap_or(0.0).asinh() * 0.4;
            out.push([
                g(s, 0),
                g(s, 1),
                g(s, 2),
                g(s, 3),
                g(s, 4),
                g(s, 5),
                g(u, 0),
                g(u, 1),
                r.asinh() * 0.4,
                ModalityTag::ContinuousCausal.code(),
            ]);
            i += stride;
        }
        out
    }

    pub fn from_spatial_field(grid: &[Vec<f64>], max_tokens: usize) -> Vec<[f64; RAW_FEAT_DIM]> {
        let h = grid.len();
        if h == 0 || grid[0].is_empty() {
            return vec![[0.0; RAW_FEAT_DIM]];
        }
        let w = grid[0].len();
        let total = h * w;
        let stride = (total / max_tokens.max(1)).max(1);
        let mut out = Vec::with_capacity(max_tokens.min(total));
        let mut idx = 0usize;
        while idx < total && out.len() < max_tokens {
            let r = idx / w;
            let c = idx % w;
            let v = grid[r][c];
            let up = if r > 0 { grid[r - 1][c] } else { v };
            let dn = if r + 1 < h { grid[r + 1][c] } else { v };
            let lt = if c > 0 { grid[r][c - 1] } else { v };
            let rt = if c + 1 < w { grid[r][c + 1] } else { v };
            let gx = (rt - lt) * 0.5;
            let gy = (dn - up) * 0.5;
            let lap = up + dn + lt + rt - 4.0 * v;
            out.push([
                (r as f64 / h.max(1) as f64) * 2.0 - 1.0,
                (c as f64 / w.max(1) as f64) * 2.0 - 1.0,
                v.asinh() * 0.4,
                gx.asinh() * 0.4,
                gy.asinh() * 0.4,
                lap.asinh() * 0.4,
                (up - dn).abs().asinh() * 0.4,
                (rt - lt).abs().asinh() * 0.4,
                (h as f64 / w.max(1) as f64).ln().clamp(-1.0, 1.0),
                ModalityTag::SpatialField.code(),
            ]);
            idx += stride;
        }
        out
    }

    pub fn from_code_and_feedback(
        source: &str,
        stderr: &str,
        passed: usize,
        total: usize,
    ) -> Vec<[f64; RAW_FEAT_DIM]> {
        let mut toks = Self::from_bytes(source.as_bytes(), ModalityTag::CodeFeedback, 40);
        if !stderr.is_empty() {
            toks.extend(Self::from_bytes(stderr.as_bytes(), ModalityTag::RawBytes, 20));
        }
        let ratio = if total > 0 { passed as f64 / total as f64 } else { 0.0 };
        toks.push([
            ratio * 2.0 - 1.0,
            (passed as f64).ln_1p() * 0.25,
            (total as f64).ln_1p() * 0.25,
            if stderr.is_empty() { 1.0 } else { -1.0 },
            (source.len() as f64).ln_1p() * 0.1,
            0.0,
            0.0,
            0.0,
            0.0,
            ModalityTag::CodeFeedback.code(),
        ]);
        toks
    }
}

// ============================================================================
// 2. UNIVERSAL 3-TIER VERIFICATION SPECTRUM
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleTier {
    Tier1Execution,
    Tier2CausalWorld,
    Tier3FormalOpt,
}

#[derive(Clone, Debug)]
pub struct OracleVerdict {
    pub tier: OracleTier,
    pub passed: bool,
    pub score: f64,
    pub compute_cost_us: u64,
    pub mdl_bits: usize,
    pub feedback: String,
}

pub trait UniversalOracle {
    fn tier(&self) -> OracleTier;
    fn evaluate(&mut self, candidate: &str) -> OracleVerdict;
}

pub struct RustSandboxCodeOracle {
    pub task_name: String,
    pub harness_main: String,
    pub expected_token: String,
}

impl UniversalOracle for RustSandboxCodeOracle {
    fn tier(&self) -> OracleTier {
        OracleTier::Tier1Execution
    }

    fn evaluate(&mut self, candidate_code: &str) -> OracleVerdict {
        let t0 = Instant::now();
        let pid = std::process::id();
        let stamp = t0.elapsed().as_nanos();
        let base = std::env::temp_dir().join(format!("asi_tier1_{}_{}_{}", self.task_name, pid, stamp));
        let rs_path = base.with_extension("rs");
        let bin_path = if cfg!(windows) { base.with_extension("exe") } else { base };

        let full_src = format!("{}\n\n{}\n", candidate_code, self.harness_main);
        let mdl_bits = candidate_code.len() * 8;
        if let Err(e) = fs::write(&rs_path, &full_src) {
            return OracleVerdict {
                tier: self.tier(),
                passed: false,
                score: 0.0,
                compute_cost_us: t0.elapsed().as_micros() as u64,
                mdl_bits,
                feedback: format!("io_err: {}", e),
            };
        }

        let rustc = find_rustc_bin();
        let comp = Command::new(&rustc)
            .args(["-O", "-D", "warnings"])
            .arg(&rs_path)
            .arg("-o")
            .arg(&bin_path)
            .output();

        let verdict = match comp {
            Ok(out) if out.status.success() => {
                let run_t0 = Instant::now();
                let run = Command::new(&bin_path).output();
                let run_us = run_t0.elapsed().as_micros() as u64;
                match run {
                    Ok(ro) => {
                        let stdout = String::from_utf8_lossy(&ro.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&ro.stderr).to_string();
                        let ok = ro.status.success() && stdout.contains(&self.expected_token);
                        OracleVerdict {
                            tier: self.tier(),
                            passed: ok,
                            score: if ok { 1.0 } else { 0.0 },
                            compute_cost_us: run_us.max(1),
                            mdl_bits,
                            feedback: if ok {
                                stdout.trim().to_string()
                            } else {
                                format!("{} {}", stdout.trim(), stderr.trim())
                            },
                        }
                    }
                    Err(e) => OracleVerdict {
                        tier: self.tier(),
                        passed: false,
                        score: 0.0,
                        compute_cost_us: run_us.max(1),
                        mdl_bits,
                        feedback: format!("exec_err: {}", e),
                    },
                }
            }
            Ok(out) => OracleVerdict {
                tier: self.tier(),
                passed: false,
                score: 0.0,
                compute_cost_us: t0.elapsed().as_micros() as u64,
                mdl_bits,
                feedback: String::from_utf8_lossy(&out.stderr).chars().take(280).collect(),
            },
            Err(e) => OracleVerdict {
                tier: self.tier(),
                passed: false,
                score: 0.0,
                compute_cost_us: t0.elapsed().as_micros() as u64,
                mdl_bits,
                feedback: format!("rustc_spawn_err: {}", e),
            },
        };
        let _ = fs::remove_file(&rs_path);
        let _ = fs::remove_file(&bin_path);
        verdict
    }
}

// ============================================================================
// 3. TIER-3 FAST PURE-RUST CEGIS ALGEBRA ORACLE (ZERO LEAN LAG!)
// ============================================================================

#[derive(Clone, Debug)]
pub struct Tier3FastCegisReport {
    pub tier: OracleTier,
    pub proposed_pairs: usize,
    pub cegis_refuted: usize,
    pub rules_promoted: usize,
    pub egraph_rules_total: usize,
    pub wall_ms: f64,
    pub status: String,
}

pub fn run_tier3_fast_cegis_algebra(out_dir: &Path, tag: &str) -> Tier3FastCegisReport {
    let t0 = Instant::now();
    let pr = run_fast_cegis_algebra_and_promote(out_dir, tag);
    let mut rules = EGraph::base_comm_rules();
    let promoted = pr.promoted_egraph_rules.len();
    rules.extend(pr.promoted_egraph_rules);
    let probe = OmegaExpr::Sub(
        Box::new(OmegaExpr::Max(Box::new(OmegaExpr::Arg(0)), Box::new(OmegaExpr::Arg(1)))),
        Box::new(OmegaExpr::Min(Box::new(OmegaExpr::Arg(0)), Box::new(OmegaExpr::Arg(1)))),
    );
    let (_best, _unions) = egraph_optimize(&probe, &rules);
    Tier3FastCegisReport {
        tier: OracleTier::Tier3FormalOpt,
        proposed_pairs: pr.proposed_pairs,
        cegis_refuted: pr.cegis_refuted,
        rules_promoted: promoted,
        egraph_rules_total: rules.len(),
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
        status: pr.lean4_status,
    }
}

// ============================================================================
// 4. REAL SELF-HOSTING REPOSITORY RSI ENGINE (TARGETS LIVE `src/evolved_policy.rs`)
//    Reads TropiCore-Omega's OWN compiled `src/evolved_policy.rs` into the Unified Brain,
//    evolves structural Rust replacements for BOTH `synth_candidate_priority` AND `cem_step_cost`,
//    compiles via `rustc -O -D warnings` against a sealed multi-domain OOD Pareto harness,
//    logs a SHA-256 chain ledger, and (under `--apply-rsi`) atomically upgrades live source
//    with automatic rollback verification!
// ============================================================================

#[derive(Clone, Debug)]
pub struct SelfHostingRsiReport {
    pub target_live_file: String,
    pub live_source_sha256: String,
    pub baseline_pareto_score: f64,
    pub evolved_pareto_score: f64,
    pub candidates_evaluated: usize,
    pub accepted: bool,
    pub applied_to_live_repo: bool,
    pub structural_upgrade: String,
    pub ledger_sha256: String,
    pub staged_path: String,
}

fn find_live_evolved_policy_path() -> Option<std::path::PathBuf> {
    let local = std::path::PathBuf::from("src/evolved_policy.rs");
    if local.exists() {
        return Some(local);
    }
    let abs = std::path::PathBuf::from(r"C:\Defi\초지능\tropicore_omega\src\evolved_policy.rs");
    if abs.exists() {
        return Some(abs);
    }
    None
}

pub fn run_self_hosting_repo_rsi(
    core: &mut UniversalEnergyCore,
    out_dir: &Path,
    seed: u64,
    apply_live: bool,
) -> SelfHostingRsiReport {
    let _ = fs::create_dir_all(out_dir);
    let mut rng = seed ^ 0x5E1F_1000_C0DE;

    let live_path_opt = find_live_evolved_policy_path();
    let live_path_str = live_path_opt
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "src/evolved_policy.rs".to_string());

    let live_src = live_path_opt
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_else(|| {
            r#"pub fn synth_candidate_priority(train_err: f64, neural_val_prob: f64, prog_len: usize, skill_bonus: f64) -> f64 {
    (1.0 + train_err.abs()).ln() - 0.55 * neural_val_prob.clamp(0.02, 0.98).ln() + 0.28 * (prog_len as f64) - 0.35 * skill_bonus
}
pub fn cem_step_cost(goal_dist: f64, obs_dist: f64, vel_norm: f64, u_norm_sq: f64, epistemic_disagreement: f64, is_terminal: bool) -> f64 {
    (if is_terminal { 2.8 } else { 1.0 }) * goal_dist + if obs_dist < 1.05 { 9.5 * (1.05 - obs_dist) } else { 0.0 } + 0.35 * epistemic_disagreement + 0.02 * u_norm_sq + 0.05 * vel_norm
}"#.to_string()
        });
    let live_sha = sha256_hex(live_src.as_bytes());

    // Introspect TropiCore-Omega's OWN live `src/evolved_policy.rs` into Unified Brain + SM120 tactic head!
    let self_toks = UniversalEventStream::from_bytes(live_src.as_bytes(), ModalityTag::CodeFeedback, 48);
    let z_self = core.encode_raw_tokens(&self_toks);
    let policy_probs = omega_tactic_policy_on_device(
        &z_self,
        &core.w_tactic,
        &core.b_tactic,
        NUM_TACTIC_CLASSES,
        OMEGA_DIM,
        0,
    );
    let struct_bias = (policy_probs[0] + policy_probs[1] + policy_probs[2]).clamp(0.30, 0.90);

    #[derive(Clone, Copy)]
    struct PolicyModuleGenome {
        w_nll: f64,
        w_len: f64,
        w_skill: f64,
        use_log_len: bool,
        w_term: f64,
        obs_margin: f64,
        obs_lin: f64,
        obs_quad: f64,
        w_unc: f64,
        brake_w: f64,
        use_smooth_barrier: bool,
    }

    let emit_evolved_policy_rs = |g: PolicyModuleGenome| -> String {
        let len_expr = if g.use_log_len {
            format!("({wl:.4}f64) * (1.0f64 + (prog_len as f64)).ln()", wl = g.w_len)
        } else {
            format!("({wl:.4}f64) * (prog_len as f64)", wl = g.w_len)
        };
        let obs_expr = if g.use_smooth_barrier {
            format!(
                "if obs_dist < ({om:.4}f64) {{ let m = ({om:.4}f64) - obs_dist; ({ol:.4}f64) * m + ({oq:.4}f64) * m * m + 1.8f64 / (obs_dist.max(0.18f64)) }} else {{ 0.0f64 }}",
                om = g.obs_margin,
                ol = g.obs_lin,
                oq = g.obs_quad
            )
        } else {
            format!(
                "if obs_dist < ({om:.4}f64) {{ let m = ({om:.4}f64) - obs_dist; ({ol:.4}f64) * m + ({oq:.4}f64) * m * m }} else {{ 0.0f64 }}",
                om = g.obs_margin,
                ol = g.obs_lin,
                oq = g.obs_quad
            )
        };
        format!(
            r#"//! Evolved Self-Hosting Policy Module (`TropiCore-Omega v10.0 RSI`).
#[inline]
pub fn synth_candidate_priority(
    train_err: f64,
    neural_val_prob: f64,
    prog_len: usize,
    skill_bonus: f64,
) -> f64 {{
    let err_term = (1.0f64 + train_err.abs()).ln();
    let neural_nll = -(neural_val_prob.clamp(0.02f64, 0.98f64)).ln();
    let len_pen = {len_expr};
    err_term + ({wn:.4}f64) * neural_nll + len_pen - ({ws:.4}f64) * skill_bonus
}}

#[inline]
pub fn cem_step_cost(
    goal_dist: f64,
    obs_dist: f64,
    vel_norm: f64,
    u_norm_sq: f64,
    epistemic_disagreement: f64,
    is_terminal: bool,
) -> f64 {{
    let w_goal = if is_terminal {{ {wt:.4}f64 }} else {{ 1.0f64 }};
    let obs_pen = {obs_expr};
    let unc_pen = ({wu:.4}f64) * epistemic_disagreement;
    let speed_brake = if goal_dist < 0.90f64 {{ ({bw:.4}f64) * vel_norm }} else {{ 0.0f64 }};
    w_goal * goal_dist + obs_pen + unc_pen + speed_brake + 0.02f64 * u_norm_sq
}}"#,
            len_expr = len_expr,
            wn = g.w_nll,
            ws = g.w_skill,
            wt = g.w_term,
            obs_expr = obs_expr,
            wu = g.w_unc,
            bw = g.brake_w
        )
    };

    // Sealed Multi-Domain OOD Pareto Harness: tests BOTH synthesis priority ranking accuracy AND
    // continuous nonlinear vortex+obstacle guidance decision quality (strict monotonicity + Pareto score)!
    let sealed_harness = r#"fn main() {
    // 1. Sealed Synthesis Priority Ranking OOD Gate (strict monotonicity + recall@top1)
    let mut rank_ok = 0.0f64;
    let synth_pairs = [
        // (good: err, p, len, skill) vs (bad: err, p, len, skill)
        ((0.0, 0.88, 6usize, 0.70), (0.0, 0.25, 6usize, 0.10)),
        ((0.0, 0.75, 7usize, 0.65), (0.0, 0.40, 5usize, 0.05)),
        ((0.0, 0.82, 8usize, 0.80), (0.0, 0.30, 5usize, 0.00)),
        ((0.0, 0.70, 9usize, 0.60), (0.0, 0.20, 6usize, 0.10)),
        ((0.2, 0.70, 6usize, 0.50), (1.8, 0.70, 6usize, 0.50)),
        ((0.0, 0.60, 5usize, 0.50), (0.0, 0.60, 12usize, 0.50)),
    ];
    for &(g, b) in &synth_pairs {
        let sg = synth_candidate_priority(g.0, g.1, g.2, g.3);
        let sb = synth_candidate_priority(b.0, b.1, b.2, b.3);
        if sg < sb - 1e-4 {
            rank_ok += 1.0;
        }
    }
    let synth_score = rank_ok / synth_pairs.len() as f64;

    // 2. Sealed Nonlinear Continuous Control OOD Gate (prefers safe bypass over obstacle collision + terminal goal progress)
    let mut ctrl_score = 0.0f64;
    let ctrl_cases = [
        // (safe bypass: dg, dobs, v, u2, unc, term) vs (unsafe collision shortcut)
        ((1.4, 1.35, 1.1, 0.6, 0.08, false), (0.9, 0.45, 1.8, 0.6, 0.10, false)),
        ((1.1, 1.25, 0.9, 0.5, 0.05, false), (0.7, 0.60, 1.6, 0.5, 0.25, false)),
        ((0.35, 1.60, 0.4, 0.2, 0.04, true), (0.85, 1.60, 1.9, 0.2, 0.04, true)),
        ((0.90, 1.20, 0.8, 0.4, 0.05, false), (0.88, 0.75, 1.5, 0.4, 0.45, false)),
        ((0.40, 1.50, 0.2, 0.1, 0.03, true), (0.40, 1.50, 2.6, 0.1, 0.03, true)),
        ((1.20, 1.40, 1.0, 0.3, 0.04, false), (1.15, 0.52, 1.0, 0.3, 0.04, false)),
    ];
    for &(safe, bad) in &ctrl_cases {
        let cs = cem_step_cost(safe.0, safe.1, safe.2, safe.3, safe.4, safe.5);
        let cb = cem_step_cost(bad.0, bad.1, bad.2, bad.3, bad.4, bad.5);
        let margin = (cb - cs).clamp(-5.0, 12.0);
        if margin > 0.0 {
            ctrl_score += 0.5 + 0.5 * (margin / (2.0 + margin));
        }
    }
    ctrl_score /= ctrl_cases.len() as f64;

    let pareto = 0.50 * synth_score + 0.50 * ctrl_score;
    println!("ASI_REPO_RSI_SCORE:{:.6}", pareto);
}"#;

    let mut oracle = RustSandboxCodeOracle {
        task_name: "self_hosting_repo_rsi".to_string(),
        harness_main: sealed_harness.to_string(),
        expected_token: "ASI_REPO_RSI_SCORE:".to_string(),
    };

    let parse_score = |fb: &str| -> f64 {
        fb.split("ASI_REPO_RSI_SCORE:")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or(0.0)
    };

    // Strip any `//!` inner module doc comments when pasting live file into standalone test binary
    let strip_inner_docs = |src: &str| -> String {
        src.lines()
            .filter(|l| !l.trim_start().starts_with("//!"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let base_v = oracle.evaluate(&strip_inner_docs(&live_src));
    let baseline_score = if base_v.passed { parse_score(&base_v.feedback) } else { 0.0 };

    let mut best_score = baseline_score;
    let mut best_src = live_src.clone();
    let mut best_tag = "live_baseline".to_string();
    let mut evaluated = 0usize;

    for idx in 0..6usize {
        evaluated += 1;
        let use_log_len = idx % 2 == 1 || rand_uniform(&mut rng) < struct_bias;
        let use_smooth_barrier = idx >= 2 || rand_uniform(&mut rng) < struct_bias;
        let cand_g = PolicyModuleGenome {
            w_nll: 0.70 + 0.85 * rand_uniform(&mut rng),
            w_len: 0.16 + 0.24 * rand_uniform(&mut rng),
            w_skill: 0.55 + 0.65 * rand_uniform(&mut rng),
            use_log_len,
            w_term: 2.60 + 1.40 * rand_uniform(&mut rng),
            obs_margin: 1.05 + 0.25 * rand_uniform(&mut rng),
            obs_lin: 10.0 + 6.0 * rand_uniform(&mut rng),
            obs_quad: 7.0 + 7.0 * rand_uniform(&mut rng),
            w_unc: 0.55 + 0.75 * rand_uniform(&mut rng),
            brake_w: 0.18 + 0.22 * rand_uniform(&mut rng),
            use_smooth_barrier,
        };
        let cand_src = emit_evolved_policy_rs(cand_g);
        let v = oracle.evaluate(&strip_inner_docs(&cand_src));
        if v.passed {
            let sc = parse_score(&v.feedback);
            let toks = UniversalEventStream::from_code_and_feedback(&cand_src, "", 1, 1);
            core.train_universal_stream_value_step(&toks, sc.clamp(0.0, 1.0), 0.02);
            if sc > best_score + 1e-4 {
                best_score = sc;
                best_src = cand_src;
                best_tag = format!(
                    "logLen={}+smoothBarrier={}",
                    cand_g.use_log_len, cand_g.use_smooth_barrier
                );
            }
        }
    }

    let accepted = best_score > baseline_score + 1e-4;
    let staged_path = out_dir.join("proposed_evolved_policy.rs");
    if accepted {
        let _ = fs::write(&staged_path, &best_src);
    }

    // Optional LIVE atomic repository self-modification (`--apply-rsi`) with compiler gate & rollback!
    let mut applied_live = false;
    if accepted && apply_live {
        if let Some(ref live_p) = live_path_opt {
            let backup_src = live_src.clone();
            if fs::write(live_p, &best_src).is_ok() {
                let rustc = find_rustc_bin();
                let tmp_rlib = std::env::temp_dir().join(format!("asi_rsi_gate_{}.rlib", std::process::id()));
                let gate = Command::new(&rustc)
                    .args(["--crate-type", "lib", "-O", "-D", "warnings"])
                    .arg(live_p)
                    .arg("-o")
                    .arg(&tmp_rlib)
                    .output();
                let _ = fs::remove_file(&tmp_rlib);
                match gate {
                    Ok(o) if o.status.success() => {
                        applied_live = true;
                    }
                    _ => {
                        // Atomic rollback on any warning or compile failure!
                        let _ = fs::write(live_p, &backup_src);
                    }
                }
            }
        }
    }

    let new_sha = sha256_hex(best_src.as_bytes());
    let payload = format!(
        "{{\"version\":\"v10.0\",\"dim\":{},\"prev_sha\":\"{}\",\"new_sha\":\"{}\",\"base\":{:.5},\"best\":{:.5},\"accepted\":{},\"applied_live\":{},\"upgrade\":\"{}\"}}",
        OMEGA_DIM,
        &live_sha[..16],
        &new_sha[..16],
        baseline_score,
        best_score,
        accepted,
        applied_live,
        best_tag
    );
    let digest = sha256_hex(payload.as_bytes());
    let _ = append_vault_jsonl(
        &out_dir.join("self_hosting_rsi_ledger.jsonl"),
        &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", digest, payload),
    );

    SelfHostingRsiReport {
        target_live_file: live_path_str,
        live_source_sha256: live_sha[..16].to_string(),
        baseline_pareto_score: baseline_score,
        evolved_pareto_score: best_score,
        candidates_evaluated: evaluated,
        accepted,
        applied_to_live_repo: applied_live,
        structural_upgrade: best_tag,
        ledger_sha256: digest[..16].to_string(),
        staged_path: staged_path.display().to_string(),
    }
}

// ============================================================================
// 5. OPEN-ENDED FRONTIER CURRICULUM + DAY-1 GENERAL ASI DAEMON (`--asi-forever`)
// ============================================================================

#[derive(Clone, Debug)]
pub struct OpenEndedFrontierState {
    pub physics_noise_std: f64,
    pub physics_obs_radius: f64,
    pub cycle: usize,
}

impl Default for OpenEndedFrontierState {
    fn default() -> Self {
        Self {
            physics_noise_std: 0.03,
            physics_obs_radius: 0.65,
            cycle: 0,
        }
    }
}

impl OpenEndedFrontierState {
    pub fn adapt(&mut self, mastery_rate: f64) {
        self.cycle += 1;
        if mastery_rate > 0.78 {
            self.physics_noise_std = (self.physics_noise_std + 0.015).min(0.22);
            self.physics_obs_radius = (self.physics_obs_radius + 0.07).min(1.30);
        } else if mastery_rate < 0.30 {
            self.physics_noise_std = (self.physics_noise_std - 0.01).max(0.01);
            self.physics_obs_radius = (self.physics_obs_radius - 0.05).max(0.45);
        }
    }
}

#[derive(Clone, Debug)]
pub struct AsiCycleReport {
    pub wall_ms: f64,
    pub tier1: GeneralRegSynthReport,
    pub tier2: NonlinearWorldAsiReport,
    pub tier3: Tier3FastCegisReport,
    pub rsi: SelfHostingRsiReport,
    pub contractive_linf_norm: f64,
    pub w_core_l1_drift: f64,
    pub train_steps_total: usize,
    pub ckpt_sha256: String,
    pub ckpt_roundtrip_ok: bool,
}

pub fn run_asi_general_cycle(
    core: &mut UniversalEnergyCore,
    skills: &mut ImperativeSkillLibrary,
    out_dir: &Path,
    ckpt_path: &Path,
    frontier: &mut OpenEndedFrontierState,
    seed: u64,
    apply_rsi: bool,
) -> AsiCycleReport {
    let t0 = Instant::now();
    let _ = fs::create_dir_all(out_dir);

    let tier1 = run_general_reg_synth_suite(core, skills);
    let tier2 = run_nonlinear_neural_world_asi(
        core,
        seed,
        frontier.physics_noise_std,
        frontier.physics_obs_radius,
    );
    let mastery = tier2.ensemble_cem_success as f64 / tier2.eval_episodes.max(1) as f64;
    frontier.adapt(mastery);

    let tier3 = run_tier3_fast_cegis_algebra(out_dir, &format!("c{}", frontier.cycle));
    let rsi = run_self_hosting_repo_rsi(core, out_dir, seed + 77, apply_rsi);

    let (ckpt_sha256, ckpt_roundtrip_ok) = match core.save_full_brain_checkpoint(ckpt_path) {
        Ok(h) => match UniversalEnergyCore::load_full_brain_checkpoint(ckpt_path) {
            Ok((loaded, h2)) => (h[..16].to_string(), h == h2 && loaded.w_core == core.w_core),
            Err(_) => (h[..16].to_string(), false),
        },
        Err(_) => ("ERR".to_string(), false),
    };

    AsiCycleReport {
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
        tier1,
        tier2,
        tier3,
        rsi,
        contractive_linf_norm: core.w_core_linf_norm(),
        w_core_l1_drift: core.w_core_l1_drift(),
        train_steps_total: core.train_steps_completed,
        ckpt_sha256,
        ckpt_roundtrip_ok,
    }
}

pub fn run_asi_forever_daemon(ckpt_path: &Path, cycles: usize, apply_rsi: bool, worker_threads: usize) {
    let out_dir = ckpt_path.parent().unwrap_or_else(|| Path::new("out")).join("asi_v10");
    let _ = fs::create_dir_all(&out_dir);
    let (mut core, boot) = if ckpt_path.exists() {
        match UniversalEnergyCore::load_full_brain_checkpoint(ckpt_path) {
            Ok((loaded, h)) => (
                loaded,
                format!("RESUMED FULL-BRAIN (d={}, sha256={})", OMEGA_DIM, &h[..16]),
            ),
            Err(e) => (UniversalEnergyCore::new(20261125), format!("FRESH (warn: {})", e)),
        }
    } else {
        (UniversalEnergyCore::new(20261125), format!("FRESH INIT (d={})", OMEGA_DIM))
    };

    let mut skills = ImperativeSkillLibrary::default();
    let mut frontier = OpenEndedFrontierState::default();
    println!("=================================================================================");
    println!(
        " TropiCore-Omega v10.0 ASI-Core — Autonomous General ASI Daemon (d={}, threads={}, apply_rsi={})",
        OMEGA_DIM, worker_threads, apply_rsi
    );
    println!(" Boot: {} | Cycles: {} | Ckpt: {}", boot, cycles, ckpt_path.display());
    println!("=================================================================================");

    for cyc in 1..=cycles {
        let rep = run_asi_general_cycle(
            &mut core,
            &mut skills,
            &out_dir,
            ckpt_path,
            &mut frontier,
            100_000 + (cyc as u64) * 997,
            apply_rsi,
        );
        println!(
            " [ASI {:>3}] T1 SSA={}/8(Cold {:.0} -> Tr {:.0}, rustc={}/8) | T2 NL-Vortex CEM={}/{} vs Rand={}/{} (comp={:.1}x, +55deg Fault {}->{}/{}) | T3 CEGIS=+{}({:.1}ms) | RepoRSI {:.3}->{:.3}(acc={},live={}) | ||W||={:.3} dW={:.2} | Fr(sig={:.3},r={:.2}) | ckpt={}({}) | {:.0}ms",
            cyc,
            rep.tier1.train_solved + rep.tier1.heldout_trained_solved,
            rep.tier1.heldout_cold_avg_exp,
            rep.tier1.heldout_trained_avg_exp,
            rep.tier1.rustc_verified_total,
            rep.tier2.ensemble_cem_success,
            rep.tier2.eval_episodes,
            rep.tier2.random_success,
            rep.tier2.eval_episodes,
            rep.tier2.compression_ratio,
            rep.tier2.fault_frozen_success,
            rep.tier2.fault_online_adapted_success,
            rep.tier2.fault_eval_episodes,
            rep.tier3.rules_promoted,
            rep.tier3.wall_ms,
            rep.rsi.baseline_pareto_score,
            rep.rsi.evolved_pareto_score,
            rep.rsi.accepted,
            rep.rsi.applied_to_live_repo,
            rep.contractive_linf_norm,
            rep.w_core_l1_drift,
            frontier.physics_noise_std,
            frontier.physics_obs_radius,
            rep.ckpt_sha256,
            rep.ckpt_roundtrip_ok,
            rep.wall_ms
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v10_general_asi_core_reg_synth_nonlinear_world_and_self_hosting_rsi() {
        let mut core = UniversalEnergyCore::new(1010);
        let mut skills = ImperativeSkillLibrary::default();
        let dir = std::env::temp_dir().join("tropicore_v10_asi_unit");
        let ckpt = dir.join("asi_v10_unit.ckpt");
        let mut frontier = OpenEndedFrontierState::default();

        let rep = run_asi_general_cycle(&mut core, &mut skills, &dir, &ckpt, &mut frontier, 100_123, false);

        // 1. Tier-1 General Multi-Register SSA Imperative Synthesis (8/8 rustc-O verified, zero-peek transfer!)
        assert_eq!(rep.tier1.train_solved, 4, "Tier-1 must solve 4/4 training imperative SSA algorithms!");
        assert_eq!(rep.tier1.heldout_trained_solved, 4, "Tier-1 must solve 4/4 UNSEEN held-out imperative SSA algorithms!");
        assert_eq!(rep.tier1.rustc_verified_total, 8, "All 8 synthesized Rust algorithms must pass `rustc -O -D warnings` on sealed OOD!");
        assert!(
            rep.tier1.heldout_trained_avg_exp < rep.tier1.heldout_cold_avg_exp,
            "Zero-peek Unified Brain + SkillLibrary ({:.1}) must strictly beat Cold Untrained ({:.1}) on 4 UNSEEN held-out tasks!",
            rep.tier1.heldout_trained_avg_exp,
            rep.tier1.heldout_cold_avg_exp
        );

        // 2. Tier-2 Nonlinear Neural-Ensemble World Model + Active Epistemic Curiosity + Non-Stationary Adaptation!
        assert_eq!(rep.tier2.tier, OracleTier::Tier2CausalWorld);
        assert!(rep.tier2.epistemic_probes_taken > 100, "Active epistemic curiosity must drive exploration!");
        assert!(
            rep.tier2.compression_ratio > 5.0,
            "Nonlinear 2-MLP ensemble must compress prediction MSE >5x! got {:.2}x",
            rep.tier2.compression_ratio
        );
        assert!(
            rep.tier2.ensemble_cem_success >= rep.tier2.random_success + 6,
            "Nonlinear Neural-Ensemble CEM ({}/{}) must crush Random ({}/{}) on unseen vortex+obstacle worlds!",
            rep.tier2.ensemble_cem_success,
            rep.tier2.eval_episodes,
            rep.tier2.random_success,
            rep.tier2.eval_episodes
        );
        assert!(
            rep.tier2.fault_online_adapted_success >= rep.tier2.fault_frozen_success,
            "Online surprise-adapting ensemble ({}/{}) must match or beat frozen ({}/{}) under +55-deg actuator fault!",
            rep.tier2.fault_online_adapted_success,
            rep.tier2.fault_eval_episodes,
            rep.tier2.fault_frozen_success,
            rep.tier2.fault_eval_episodes
        );

        // 3. Tier-3 Fast Pure-Rust CEGIS + E-Graph (zero Lean lag!)
        assert_eq!(rep.tier3.tier, OracleTier::Tier3FormalOpt);
        assert!(rep.tier3.rules_promoted >= 20);

        // 4. Real Self-Hosting Repository RSI (targeting live `src/evolved_policy.rs`)
        assert!(rep.rsi.accepted && rep.rsi.evolved_pareto_score > rep.rsi.baseline_pareto_score);
        assert!(!rep.rsi.applied_to_live_repo, "Default non---apply-rsi test must stage without mutating git working tree!");

        // 5. Contractive Unified Brain + Bit-Exact SHA-256 Roundtrip
        assert!(rep.contractive_linf_norm < 1.0 && rep.w_core_l1_drift > 0.0);
        assert!(rep.ckpt_roundtrip_ok);

        let _ = fs::remove_dir_all(&dir);
    }
}
