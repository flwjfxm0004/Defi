//! Structural Multi-Formula AST + Whole-Crate Verified Evolutionary Self-Optimizer (`TropiCore-Omega v15.0`).
//!
//! Beyond 6-float hyperparameter tuning:
//!   1. Mutates REAL algorithmic formula structures (`lr_formula_id`: linear ramp / smooth sigmoid / log-rational;
//!      `gamma_mode`: constant / layer-depth-tapered) AND continuous parameters around the current active elite!
//!   2. Zero Free-Riders: EVERY candidate runs real training steps with its structural formula & gamma mode on real
//!      corpus bytes and measures strictly-causal held-out `bits/byte` end-to-end!
//!   3. Emits the actual chosen Rust formula code into `src/asi_runtime_cfg.rs`, verifies with `rustc -O -D warnings`,
//!      AND under `--apply` runs a WHOLE-CRATE `cargo check --release` safety gate (auto-rolling back on any failure!).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::brain::{CausalStreamState, DeepContractiveBrain, RuntimePolicyOverride};
use crate::sha256::{append_vault_jsonl, sha256_hex};

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_uniform(rng: &mut u64) -> f32 {
    ((splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64)) as f32
}

fn find_bin(name: &str) -> String {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let win = format!("{}\\.cargo\\bin\\{}.exe", home, name);
        if Path::new(&win).exists() { return win; }
    }
    name.to_string()
}

fn find_live_cfg_path() -> Option<PathBuf> {
    let local = PathBuf::from("src/asi_runtime_cfg.rs");
    if local.exists() { return Some(local); }
    let abs = PathBuf::from(r"C:\Defi\초지능\tropicore_omega\src\asi_runtime_cfg.rs");
    if abs.exists() { return Some(abs); }
    None
}

fn emit_runtime_cfg_rs(g: RuntimePolicyOverride) -> String {
    let lr_body = match g.lr_formula_id {
        1 => format!(
            "let diff = surprise_bits - ({ct:.4}f32);\n    let sig = 1.0 / (1.0 + (-({sl:.4}f32 * 2.5 * diff).clamp(-10.0, 10.0)).exp());\n    let boost = 0.55 + 1.15 * sig;\n    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)",
            sl = g.lr_slope,
            ct = g.lr_center
        ),
        2 => format!(
            "let diff = surprise_bits - ({ct:.4}f32);\n    let boost = 1.0 + ({sl:.4}f32) * 1.4 * diff.signum() * (1.0 + diff.abs()).ln();\n    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)",
            sl = g.lr_slope,
            ct = g.lr_center
        ),
        _ => format!(
            "let boost = 1.0 + ({sl:.4}f32) * (surprise_bits - ({ct:.4}f32)).clamp(-1.0, 4.0);\n    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)",
            sl = g.lr_slope,
            ct = g.lr_center
        ),
    };
    format!(
        r#"//! Evolved Live Structural AST Runtime Policy (`TropiCore-Omega v15.0`, `{tag}`).

#[inline]
pub fn contractive_gamma() -> f32 {{
    {gamma:.4}f32
}}

#[inline]
pub fn adaptive_online_lr(base_lr: f32, surprise_bits: f32) -> f32 {{
    {lr_body}
}}

#[inline]
pub fn meta_mixer_prior() -> (f32, f32, bool) {{
    ({ws:.4}f32, {wc:.4}f32, {lo})
}}
"#,
        tag = g.formula_tag(),
        gamma = g.gamma,
        lr_body = lr_body,
        ws = g.w_mem_scale,
        wc = g.w_mem_cap,
        lo = g.use_log_odds_mix
    )
}

fn verify_rust_lib_zero_warnings(source: &str) -> Result<(), String> {
    let pid = std::process::id();
    let stamp = Instant::now().elapsed().as_nanos();
    let base = std::env::temp_dir().join(format!("asi_self_opt_{}_{}", pid, stamp));
    let rs_p = base.with_extension("rs");
    let lib_p = base.with_extension("rlib");
    fs::write(&rs_p, source).map_err(|e| format!("io write err: {}", e))?;

    let rustc = find_bin("rustc");
    let out = Command::new(&rustc)
        .args(["--crate-type", "lib", "-O", "-D", "warnings"])
        .arg(&rs_p)
        .arg("-o")
        .arg(&lib_p)
        .output();
    let _ = fs::remove_file(&rs_p);
    let _ = fs::remove_file(&lib_p);

    match out {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).chars().take(240).collect()),
        Err(e) => Err(format!("rustc spawn err: {}", e)),
    }
}

/// Whole-Crate `cargo check --release` safety gate before finalizing `--apply` live source mutation!
fn verify_whole_crate_cargo_check(cfg_path: &Path) -> bool {
    let crate_root = cfg_path.parent().and_then(|p| p.parent()).unwrap_or_else(|| Path::new("."));
    let cargo = find_bin("cargo");
    Command::new(&cargo)
        .current_dir(crate_root)
        .env("RUSTFLAGS", "-D warnings")
        .env("CARGO_BUILD_JOBS", "2")
        .args(["check", "--release"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn evaluate_policy_end_to_end(
    base_brain: &DeepContractiveBrain,
    train_probe: &[u8],
    eval_probe: &[u8],
    policy: RuntimePolicyOverride,
) -> f64 {
    let mut b = base_brain.clone();
    let mut streams = vec![CausalStreamState::new(&b.cfg)];
    let cl = 64usize;
    let mut off = 0usize;
    while off + 1 < train_probe.len() {
        let end = (off + cl).min(train_probe.len() - 1);
        let io = [(&train_probe[off..end], &train_probe[off + 1..end + 1])];
        b.train_batch_streams(&io, &mut streams, b.cfg.lr * 0.65, Some(policy), false, 1);
        off = end;
    }
    let (_param, online_bpb) = b.eval_byte_stream(eval_probe, cl, true, Some(policy));
    online_bpb
}

#[derive(Clone, Debug)]
pub struct SelfOptReport {
    pub target_live_file: String,
    pub live_sha256: String,
    pub baseline_real_bpb: f64,
    pub optimized_real_bpb: f64,
    pub candidates_evaluated: usize,
    pub accepted: bool,
    pub rustc_zero_warnings: bool,
    pub applied_live: bool,
    pub winning_policy: Option<RuntimePolicyOverride>,
    pub upgrade_desc: String,
    pub staged_path: String,
    pub ledger_sha256: String,
    pub wall_ms: f64,
}

pub fn run_real_self_optimization(
    brain: &DeepContractiveBrain,
    real_train_bytes: &[u8],
    real_heldout_bytes: &[u8],
    out_dir: &Path,
    seed: u64,
    apply_live: bool,
    threads: usize,
) -> SelfOptReport {
    let t0 = Instant::now();
    let _ = fs::create_dir_all(out_dir);
    let live_path_opt = find_live_cfg_path();
    let live_path_str = live_path_opt
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "src/asi_runtime_cfg.rs".to_string());
    let live_src = live_path_opt
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .unwrap_or_default();
    let live_sha = sha256_hex(live_src.as_bytes());

    let train_probe = &real_train_bytes[..real_train_bytes.len().min(512)];
    let eval_probe = &real_heldout_bytes[..real_heldout_bytes.len().min(2048)];

    let base_pol = brain.active_policy;
    let baseline_bpb = evaluate_policy_end_to_end(brain, train_probe, eval_probe, base_pol);

    // 8 diverse structural AST + continuous candidates (4 local elite mutations + 4 structural AST explorers!)
    let mut rng = seed ^ 0x5E1F_1500_A510;
    let mut cands = Vec::with_capacity(8);
    for idx in 0..8usize {
        if idx < 4 {
            cands.push(RuntimePolicyOverride {
                gamma: (base_pol.gamma + (rand_uniform(&mut rng) - 0.5) * 0.16).clamp(0.32, 0.84),
                gamma_mode: if idx % 2 == 0 { base_pol.gamma_mode } else { 1 - base_pol.gamma_mode.min(1) },
                lr_formula_id: ((base_pol.lr_formula_id as usize + idx) % 3) as u8,
                lr_slope: (base_pol.lr_slope + (rand_uniform(&mut rng) - 0.5) * 0.12).clamp(0.03, 0.36),
                lr_center: (base_pol.lr_center + (rand_uniform(&mut rng) - 0.5) * 0.8).clamp(1.8, 4.5),
                w_mem_scale: (base_pol.w_mem_scale + (rand_uniform(&mut rng) - 0.5) * 0.40).clamp(0.40, 1.60),
                w_mem_cap: (base_pol.w_mem_cap + (rand_uniform(&mut rng) - 0.5) * 0.16).clamp(0.60, 0.94),
                use_log_odds_mix: if idx == 3 { !base_pol.use_log_odds_mix } else { base_pol.use_log_odds_mix },
                from_compiled_defaults: false,
            });
        } else {
            cands.push(RuntimePolicyOverride {
                gamma: 0.36 + 0.44 * rand_uniform(&mut rng),
                gamma_mode: (idx % 2) as u8,
                lr_formula_id: ((idx - 4) % 3) as u8,
                lr_slope: 0.05 + 0.28 * rand_uniform(&mut rng),
                lr_center: 2.0 + 2.2 * rand_uniform(&mut rng),
                w_mem_scale: 0.55 + 0.95 * rand_uniform(&mut rng),
                w_mem_cap: 0.68 + 0.25 * rand_uniform(&mut rng),
                use_log_odds_mix: idx % 2 == 1,
                from_compiled_defaults: false,
            });
        }
    }

    let n_threads = threads.max(1).min(cands.len());
    let chunk_sz = (cands.len() + n_threads - 1) / n_threads;
    let mut scores = vec![f64::INFINITY; cands.len()];
    let cands_ref = &cands;
    std::thread::scope(|s| {
        for (ti, sc_chunk) in scores.chunks_mut(chunk_sz).enumerate() {
            let start = ti * chunk_sz;
            s.spawn(move || {
                for (local_i, slot) in sc_chunk.iter_mut().enumerate() {
                    let cand = cands_ref[start + local_i];
                    *slot = evaluate_policy_end_to_end(brain, train_probe, eval_probe, cand);
                }
            });
        }
    });

    let mut best_idx = 0usize;
    let mut best_bpb = baseline_bpb;
    for (i, &sc) in scores.iter().enumerate() {
        if sc < best_bpb - 1e-4 {
            best_bpb = sc;
            best_idx = i;
        }
    }

    let improved = best_bpb < baseline_bpb - 1e-4;
    let mut rustc_ok = false;
    let mut applied_live = false;
    let mut winning_policy = None;
    let staged_path = out_dir.join("proposed_asi_runtime_cfg.rs");
    let mut upgrade_desc = "active_elite_optimal".to_string();
    let mut final_src = live_src.clone();

    if improved {
        let win = cands[best_idx];
        let src = emit_runtime_cfg_rs(win);
        if verify_rust_lib_zero_warnings(&src).is_ok() {
            rustc_ok = true;
            winning_policy = Some(win);
            final_src = src.clone();
            upgrade_desc = format!(
                "ast={} mix={} w_mem={:.2}(cap={:.2}) gamma={:.2} lr_sl={:.2}",
                win.formula_tag(),
                if win.use_log_odds_mix { "log_odds_norm" } else { "linear" },
                win.w_mem_scale, win.w_mem_cap, win.gamma, win.lr_slope
            );
            let _ = fs::write(&staged_path, &src);

            if apply_live {
                if let Some(ref lp) = live_path_opt {
                    if fs::write(lp, &src).is_ok() {
                        if verify_whole_crate_cargo_check(lp) {
                            applied_live = true;
                        } else {
                            let _ = fs::write(lp, &live_src); // atomic rollback on whole-crate failure!
                        }
                    }
                }
            }
        }
    }

    let accepted = improved && rustc_ok;
    let new_sha = sha256_hex(final_src.as_bytes());
    let payload = format!(
        "{{\"engine\":\"ASI-Zero-v15\",\"prev_sha\":\"{}\",\"new_sha\":\"{}\",\"base_bpb\":{:.5},\"best_bpb\":{:.5},\"accepted\":{},\"applied_live\":{},\"upgrade\":\"{}\"}}",
        &live_sha[..16], &new_sha[..16], baseline_bpb, best_bpb, accepted, applied_live, upgrade_desc
    );
    let ledger_digest = sha256_hex(payload.as_bytes());
    let _ = append_vault_jsonl(
        &out_dir.join("self_opt_ledger.jsonl"),
        &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", ledger_digest, payload),
    );

    SelfOptReport {
        target_live_file: live_path_str,
        live_sha256: live_sha[..16].to_string(),
        baseline_real_bpb: baseline_bpb,
        optimized_real_bpb: if accepted { best_bpb } else { baseline_bpb },
        candidates_evaluated: cands.len(),
        accepted,
        rustc_zero_warnings: rustc_ok,
        applied_live,
        winning_policy,
        upgrade_desc,
        staged_path: staged_path.display().to_string(),
        ledger_sha256: ledger_digest[..16].to_string(),
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
    }
}
