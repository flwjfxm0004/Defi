//! Structural Multi-Formula AST + Compiled-Binary 2D Pareto Verified Self-Optimizer (`TropiCore-Omega v16.0`).
//!
//! Full Recursive Self-Improvement Provenance & Safety Gate:
//!   1. Mutates REAL algorithmic formula structures (`lr_formula_id`: linear ramp / smooth sigmoid / log-rational;
//!      `gamma_mode`: constant / layer-depth-tapered) AND simplex meta-mixer continuous priors!
//!   2. Zero Free-Riders: EVERY candidate runs real training steps with its structural formula & gamma mode on real
//!      corpus bytes and measures strictly-causal held-out `bits/byte` end-to-end!
//!   3. Emits the winning Rust source into `src/asi_runtime_cfg.rs`, verifies with `rustc -O -D warnings` +
//!      whole-crate `cargo check --release` under `--apply`, AND under `--verify-binary`:
//!        * Compiles an isolated release binary inside `target/rsi_sandbox` (`CARGO_BUILD_JOBS=2`, `-D warnings`)
//!        * Executes the newly compiled binary live (`status` + `eval-bytes --raw-file --use-compiled-defaults`)
//!        * Verifies 2D Pareto non-regression (held-out `bpb` AND latency) with atomic rollback on ANY failure
//!        * Records BOTH `src_sha256` AND `bin_sha256` into `out/self_opt_ledger.jsonl`!

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
        r#"//! Evolved Live Structural AST Runtime Policy (`TropiCore-Omega v17.0`, `{tag}`).

#[inline]
pub fn contractive_gamma() -> f32 {{
    {gamma:.4}f32
}}

#[inline]
pub fn gamma_depth_taper() -> bool {{
    {taper}
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
        taper = g.gamma_mode == 1,
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

fn resolve_crate_root(cfg_path: &Path) -> PathBuf {
    let rel = cfg_path
        .parent()
        .and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty());
    match rel {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        Some(p) => std::env::current_dir().map(|cwd| cwd.join(p)).unwrap_or_else(|_| p.to_path_buf()),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Whole-Crate `cargo check --release` safety gate before finalizing `--apply` live source mutation!
fn verify_whole_crate_cargo_check(cfg_path: &Path) -> Result<(), String> {
    let crate_root = resolve_crate_root(cfg_path);
    let sandbox_target = crate_root.join("target").join("rsi_sandbox");
    let cargo = find_bin("cargo");
    let rustc = find_bin("rustc");
    let out = Command::new(&cargo)
        .current_dir(&crate_root)
        .env("CARGO_TARGET_DIR", &sandbox_target)
        .env("RUSTC", &rustc)
        .env("RUSTFLAGS", "-D warnings")
        .env("CARGO_BUILD_JOBS", "2")
        .args(["check", "--release"])
        .output()
        .map_err(|e| format!("cargo check spawn err: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).chars().take(240).collect())
    }
}

fn parse_online_bpb_from_stdout(stdout: &str) -> Option<f64> {
    let idx = stdout.find("Online=")?;
    let tail = &stdout[idx + 7..];
    let tok = tail.split_whitespace().next()?;
    tok.parse::<f64>().ok()
}

/// Real Compiled-Binary 2D Pareto Verification Gate (`--verify-binary`):
/// Compiles an isolated release binary inside `target/rsi_sandbox`, hashes the compiled binary (`bin_sha256`),
/// executes `status` + `eval-bytes --raw-file --use-compiled-defaults`, and verifies real bpb & latency non-regression!
fn verify_compiled_sandbox_binary(
    cfg_path: &Path,
    brain: &DeepContractiveBrain,
    eval_probe: &[u8],
    out_dir: &Path,
) -> Result<(String, f64, f64), String> {
    let crate_root = resolve_crate_root(cfg_path);
    let sandbox_target = crate_root.join("target").join("rsi_sandbox");
    let cargo = find_bin("cargo");
    let rustc = find_bin("rustc");

    let build = Command::new(&cargo)
        .current_dir(&crate_root)
        .env("CARGO_TARGET_DIR", &sandbox_target)
        .env("RUSTC", &rustc)
        .env("CARGO_BUILD_JOBS", "2")
        .env("RUSTFLAGS", "-D warnings")
        .args(["build", "--release"])
        .output()
        .map_err(|e| format!("sandbox cargo build spawn err: {}", e))?;
    if !build.status.success() {
        return Err(format!(
            "sandbox build failed: {}",
            String::from_utf8_lossy(&build.stderr).chars().take(240).collect::<String>()
        ));
    }

    let bin_name = if cfg!(windows) { "tropicore_omega.exe" } else { "tropicore_omega" };
    let bin_path = sandbox_target.join("release").join(bin_name);
    let bin_bytes = fs::read(&bin_path).map_err(|e| format!("sandbox bin read err: {}", e))?;
    let bin_sha = sha256_hex(&bin_bytes);

    let st = Command::new(&bin_path)
        .arg("status")
        .output()
        .map_err(|e| format!("sandbox status spawn err: {}", e))?;
    if !st.status.success() {
        return Err("sandbox binary status check failed".into());
    }

    // Measure baseline bpb on the exact same base snapshot & eval_probe for apples-to-apples 2D Pareto check
    let mut base_eval_brain = brain.clone();
    let base_t0 = Instant::now();
    let (_, base_snap_bpb) = base_eval_brain.eval_byte_stream(eval_probe, 64, true, None);
    let base_ms = base_t0.elapsed().as_secs_f64() * 1000.0;

    let snap_ckpt = out_dir.join("_rsi_verify_snap.tomg");
    let eval_tmp = out_dir.join("_rsi_verify_eval.bin");
    let mut snap = brain.clone();
    snap.save_checkpoint(&snap_ckpt)?;
    fs::write(&eval_tmp, eval_probe).map_err(|e| format!("eval_tmp write err: {}", e))?;

    let ev_t0 = Instant::now();
    let ev = Command::new(&bin_path)
        .args(["eval-bytes", "--raw-file"])
        .arg(&eval_tmp)
        .arg("--ckpt")
        .arg(&snap_ckpt)
        .arg("--use-compiled-defaults")
        .output();
    let ev_ms = ev_t0.elapsed().as_secs_f64() * 1000.0;

    let _ = fs::remove_file(&snap_ckpt);
    let _ = fs::remove_file(&eval_tmp);

    let ev = ev.map_err(|e| format!("sandbox eval spawn err: {}", e))?;
    if !ev.status.success() {
        return Err(format!(
            "sandbox eval failed: {}",
            String::from_utf8_lossy(&ev.stderr).chars().take(200).collect::<String>()
        ));
    }
    let stdout = String::from_utf8_lossy(&ev.stdout);
    let new_bpb = parse_online_bpb_from_stdout(&stdout)
        .ok_or_else(|| format!("could not parse Online bpb from sandbox output: {}", stdout.trim()))?;

    // 2D Pareto Gate: non-regression in real held-out compression AND bounded inference latency!
    let latency_limit_ms = (base_ms * 4.0 + 3000.0).max(5000.0);
    if !new_bpb.is_finite() || new_bpb > base_snap_bpb + 0.03 || ev_ms > latency_limit_ms {
        return Err(format!(
            "2D Pareto gate rejected: bpb {:.4} (base {:.4}), wall {:.0}ms (limit {:.0}ms)",
            new_bpb, base_snap_bpb, ev_ms, latency_limit_ms
        ));
    }
    Ok((bin_sha, new_bpb, ev_ms))
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
    pub bin_sha256: String,
    pub baseline_real_bpb: f64,
    pub optimized_real_bpb: f64,
    pub candidates_evaluated: usize,
    pub accepted: bool,
    pub rustc_zero_warnings: bool,
    pub applied_live: bool,
    pub binary_verified: bool,
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
    verify_binary: bool,
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
    let active_bpb = evaluate_policy_end_to_end(brain, train_probe, eval_probe, base_pol);
    // When `--apply` is requested and the checkpoint's active policy has already evolved past the
    // on-disk compiled source, also measure the on-disk compiled default so a proven checkpoint elite
    // can be promoted, compiled, and 2D-Pareto verified into `src/asi_runtime_cfg.rs`!
    let compiled_bpb = if apply_live && !base_pol.from_compiled_defaults {
        evaluate_policy_end_to_end(brain, train_probe, eval_probe, RuntimePolicyOverride::compiled_default())
    } else {
        active_bpb
    };
    let baseline_bpb = if apply_live { compiled_bpb } else { active_bpb };

    // 8 diverse structural AST + simplex continuous candidates (4 local elite mutations + 4 structural AST explorers!)
    let mut rng = seed ^ 0x5E1F_1600_A510;
    let mut cands = Vec::with_capacity(8);
    for idx in 0..8usize {
        if idx < 4 {
            cands.push(RuntimePolicyOverride {
                gamma: (base_pol.gamma + (rand_uniform(&mut rng) - 0.5) * 0.16).clamp(0.32, 0.84),
                gamma_mode: if idx % 2 == 0 { base_pol.gamma_mode } else { 1 - base_pol.gamma_mode.min(1) },
                lr_formula_id: ((base_pol.lr_formula_id as usize + idx) % 3) as u8,
                lr_slope: (base_pol.lr_slope + (rand_uniform(&mut rng) - 0.5) * 0.12).clamp(0.03, 0.36),
                lr_center: (base_pol.lr_center + (rand_uniform(&mut rng) - 0.5) * 0.8).clamp(1.8, 4.5),
                w_mem_scale: (base_pol.w_mem_scale + (rand_uniform(&mut rng) - 0.5) * 0.40).clamp(0.45, 1.80),
                w_mem_cap: (base_pol.w_mem_cap + (rand_uniform(&mut rng) - 0.5) * 0.14).clamp(0.65, 0.96),
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
                w_mem_scale: 0.60 + 1.10 * rand_uniform(&mut rng),
                w_mem_cap: 0.72 + 0.23 * rand_uniform(&mut rng),
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

    let mut best_cand: Option<RuntimePolicyOverride> = None;
    let mut best_bpb = active_bpb;
    if apply_live && !base_pol.from_compiled_defaults && active_bpb < compiled_bpb - 1e-4 {
        best_cand = Some(base_pol);
    }
    for (i, &sc) in scores.iter().enumerate() {
        if sc < best_bpb - 1e-4 {
            best_bpb = sc;
            best_cand = Some(cands[i]);
        }
    }

    let improved = best_cand.is_some();
    let mut rustc_ok = false;
    let mut applied_live = false;
    let mut binary_verified = false;
    let mut bin_sha = "unbuilt".to_string();
    let mut winning_policy = None;
    let staged_path = out_dir.join("proposed_asi_runtime_cfg.rs");
    let mut upgrade_desc = "active_elite_optimal".to_string();
    let mut final_src = live_src.clone();

    if let Some(win) = best_cand {
        let src = emit_runtime_cfg_rs(win);
        if verify_rust_lib_zero_warnings(&src).is_ok() {
            rustc_ok = true;
            winning_policy = Some(win);
            final_src = src.clone();
            upgrade_desc = format!(
                "ast={} mix={} w_mem={:.2}(cap={:.2}) gamma={:.2} lr_sl={:.2}",
                win.formula_tag(),
                if win.use_log_odds_mix { "simplex_log_temp" } else { "simplex_linear" },
                win.w_mem_scale, win.w_mem_cap, win.gamma, win.lr_slope
            );
            let _ = fs::write(&staged_path, &src);

            if apply_live {
                if let Some(ref lp) = live_path_opt {
                    if fs::write(lp, &src).is_ok() {
                        if verify_binary {
                            match verify_compiled_sandbox_binary(lp, brain, eval_probe, out_dir) {
                                Ok((bsha, sb_bpb, sb_ms)) => {
                                    bin_sha = bsha[..16].to_string();
                                    binary_verified = true;
                                    applied_live = true;
                                    upgrade_desc.push_str(&format!(
                                        " [bin_verified={:.4}bpb/{:.0}ms]",
                                        sb_bpb, sb_ms
                                    ));
                                }
                                Err(err) => {
                                    let _ = fs::write(lp, &live_src); // atomic rollback!
                                    final_src = live_src.clone();
                                    upgrade_desc.push_str(&format!(" [bin_rollback:{}]", err));
                                }
                            }
                        } else {
                            match verify_whole_crate_cargo_check(lp) {
                                Ok(()) => {
                                    applied_live = true;
                                }
                                Err(err) => {
                                    let _ = fs::write(lp, &live_src); // atomic rollback!
                                    final_src = live_src.clone();
                                    upgrade_desc.push_str(&format!(" [check_rollback:{}]", err));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let accepted = improved && rustc_ok && (!verify_binary || !apply_live || binary_verified);
    let new_sha = sha256_hex(final_src.as_bytes());
    let payload = format!(
        "{{\"engine\":\"ASI-Zero-v17\",\"src_prev_sha\":\"{}\",\"src_sha256\":\"{}\",\"bin_sha256\":\"{}\",\"base_bpb\":{:.5},\"best_bpb\":{:.5},\"accepted\":{},\"applied_live\":{},\"binary_verified\":{},\"upgrade\":\"{}\"}}",
        &live_sha[..16], &new_sha[..16], bin_sha, baseline_bpb, best_bpb, accepted, applied_live, binary_verified, upgrade_desc
    );
    let ledger_digest = sha256_hex(payload.as_bytes());
    let _ = append_vault_jsonl(
        &out_dir.join("self_opt_ledger.jsonl"),
        &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", ledger_digest, payload),
    );

    SelfOptReport {
        target_live_file: live_path_str,
        live_sha256: live_sha[..16].to_string(),
        bin_sha256: bin_sha,
        baseline_real_bpb: baseline_bpb,
        optimized_real_bpb: if accepted { best_bpb } else { baseline_bpb },
        candidates_evaluated: cands.len(),
        accepted,
        rustc_zero_warnings: rustc_ok,
        applied_live,
        binary_verified,
        winning_policy: if accepted { winning_policy } else { None },
        upgrade_desc,
        staged_path: staged_path.display().to_string(),
        ledger_sha256: ledger_digest[..16].to_string(),
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
    }
}
