//! TropiCore-Omega v10.0 ASI-Core — Zero-Crutch Non-LLM General Superintelligence Architecture
//! Target Hardware (`장비.md` Track A):
//!   - 4x NVIDIA RTX PRO 6000 Blackwell Max-Q 96GB (SM120, 384 GB GDDR7, 7.168 TB/s, 14.044 PFLOPS FP4 2:4)
//!   - AMD Ryzen Threadripper PRO 9995WX (96C/192T Zen 5, 384 MB L3) + 1,000 GB DDR5-6400 ECC + 16 TB Gen5 NVMe
//!   - STRICT ZERO-CRUTCH POLICY:
//!       0 LLMs | 0 External APIs | 0 Pretraining | 0 External Rust Crates | 0 Mandatory Lean Bottleneck
//!       Tier-1 Zero-Peek Multi-Register SSA Imperative Rust Synthesis (8 algorithms, Cold vs Trained on 4 UNSEEN)
//!       Tier-2 Nonlinear 2-MLP Neural-Ensemble World Model + Active Epistemic Curiosity + +55-deg Fault Recovery
//!       Tier-3 Fast Pure-Rust CEGIS -> E-Graph (<10ms)
//!       Self-Hosting Repository RSI targeting live `src/evolved_policy.rs` (`--apply-rsi` atomic commit + rollback)

mod asi_core;
mod benchmark;
mod bytecode;
mod core;
mod egraph;
mod evolved_policy;
mod gpu_bridge;
mod ir;
mod memory;
mod nvfp4;
mod prover;
mod reg_synth;
mod sha256;
mod synthesizer;
mod world_asi;

use std::env;
use std::path::{Path, PathBuf};

use crate::asi_core::{run_asi_forever_daemon, run_asi_general_cycle, OpenEndedFrontierState};
use crate::benchmark::{run_continuous_learning_daemon, run_omega_v6_master_suite};
use crate::core::{
    UniversalEnergyCore, NUM_TACTIC_CLASSES, WORLD_BELIEF_DIM, WORLD_OBS_DIM, WORLD_STATE_DIM,
};
use crate::gpu_bridge::{detect_hardware_profile, omega_verify_sm120_abi_v8};
use crate::memory::OMEGA_DIM;
use crate::reg_synth::ImperativeSkillLibrary;
use crate::sha256::{append_vault_jsonl, sha256_hex};

fn find_core_proof() -> PathBuf {
    let local = PathBuf::from("proofs/omega_proof.lean");
    if local.exists() {
        return local;
    }
    PathBuf::from(r"C:\Defi\초지능\tropicore_omega\proofs\omega_proof.lean")
}

fn append_sealed_vault(out_dir: &Path, filename: &str, payload_json: &str) -> Result<String, String> {
    let vault = out_dir.join(filename);
    let digest = sha256_hex(payload_json.as_bytes());
    let line = format!("{{\"sha256\":\"{}\",\"report\":{}}}", digest, payload_json);
    append_vault_jsonl(&vault, &line)?;
    Ok(digest)
}

fn parse_daemon_flags(args: &[String], default_cycles: usize, default_ckpt: &str) -> (usize, PathBuf, bool, usize) {
    let mut cycles = default_cycles;
    let mut ckpt = PathBuf::from(default_ckpt);
    let mut apply_rsi = false;
    let hw_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    // Laptop-kind default: cap at 2 threads on <=16T laptops, unleash all threads on 64T-192T workstations!
    let mut threads = if hw_threads <= 16 { 2 } else { hw_threads };
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--cycles" && i + 1 < args.len() {
            cycles = args[i + 1].parse().unwrap_or(default_cycles);
            i += 2;
        } else if args[i] == "--ckpt" && i + 1 < args.len() {
            ckpt = PathBuf::from(&args[i + 1]);
            i += 2;
        } else if args[i] == "--threads" && i + 1 < args.len() {
            threads = args[i + 1].parse().unwrap_or(threads).max(1);
            i += 2;
        } else if args[i] == "--apply-rsi" {
            apply_rsi = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    (cycles, ckpt, apply_rsi, threads)
}

fn print_help() {
    println!("TropiCore-Omega v10.0 ASI-Core (d={}) — Zero-Crutch Non-LLM General ASI", OMEGA_DIM);
    println!("Modes:");
    println!("  (default) | --asi-smoke                         Fast (<10s) v10.0 4-Pillar General ASI verification");
    println!("  --asi-forever [--cycles N] [--ckpt P]           Autonomous Open-Ended General ASI daemon");
    println!("                [--apply-rsi] [--threads N]       (--apply-rsi enables atomic live `src/evolved_policy.rs` self-upgrade)");
    println!("  --learn-forever [--cycles N] [--ckpt P]         Wake-Sleep + E-Graph + Meta-AST lossless daemon");
    println!("  --legacy-8axis                                  Full v8.0 8-axis symbolic/world-model verification suite");
    println!("Compile Features:");
    println!("  --features scale_256 | scale_2048 | scale_4096 | cuda_sm120 | lean_oracle (optional)");
}

fn run_asi_smoke_mode(apply_rsi: bool, worker_threads: usize) {
    let hw = detect_hardware_profile();
    let abi_ver = omega_verify_sm120_abi_v8(
        OMEGA_DIM,
        NUM_TACTIC_CLASSES,
        WORLD_OBS_DIM,
        WORLD_BELIEF_DIM,
        WORLD_STATE_DIM,
    )
    .expect("SM120 ABI parity verification failed");

    let out_dir = PathBuf::from("out");
    let asi_dir = out_dir.join("asi_v10");
    let ckpt_path = out_dir.join("asi_v10_smoke.ckpt");
    let mut core = UniversalEnergyCore::new(20261125);
    let mut skills = ImperativeSkillLibrary::default();
    let mut frontier = OpenEndedFrontierState::default();

    println!("=================================================================================");
    println!(
        " TropiCore-Omega v10.0 ASI-Core — Zero-Crutch General ASI Smoke (d={}, ABI={}, threads={})",
        OMEGA_DIM, abi_ver, worker_threads
    );
    println!(" (0 LLMs | 0 Crates | 8-Algo SSA Synth | 2-MLP Nonlinear Vortex World | +55deg Fault Adapt | Self-Hosting Repo RSI)");
    println!("=================================================================================");
    println!(" Host Backend  : {} (GPUs={}, CPU={}T)", hw.backend, hw.physical_gpus, hw.cpu_threads);
    println!(
        " Target Node   : 4x RTX PRO 6000 Blackwell ({} GB GDDR7, {:.3} TB/s, {:.3} PFLOPS FP4) + {} GB ECC",
        hw.target_vram_gb, hw.target_gddr7_tb_s, hw.target_fp4_sparse_pflops, hw.target_dram_gb
    );
    println!(
        " Power / Cache : L2 CEGIS <= {} MiB/GPU | Compute Peak {}W (Full) / {}W (Quiet)",
        hw.l2_cegis_cap_mib_per_gpu, hw.full_load_compute_watts, hw.quiet_mode_compute_watts
    );
    println!(
        " Lean 4 Oracle : {}",
        if cfg!(feature = "lean_oracle") {
            "ENABLED (--features lean_oracle)"
        } else {
            "DECOUPLED / OPTIONAL (zero laptop lag; enable via `--features lean_oracle`)"
        }
    );
    println!("---------------------------------------------------------------------------------");

    let rep = run_asi_general_cycle(&mut core, &mut skills, &asi_dir, &ckpt_path, &mut frontier, 100_909, apply_rsi);

    println!(
        "[Tier-1] General Multi-Register SSA Imperative Synthesis (Space={}, ZERO hardcoded solutions, live `rustc -O -D warnings`)",
        rep.tier1.space_size
    );
    println!(
        "  Train Frontier = {}/{} | UNSEEN Held-Out = Cold {}/{} (avg {:.1} exp) -> Trained {}/{} (avg {:.1} exp, {:.2}x speedup!)",
        rep.tier1.train_solved,
        rep.tier1.train_total,
        rep.tier1.heldout_cold_solved,
        rep.tier1.heldout_total,
        rep.tier1.heldout_cold_avg_exp,
        rep.tier1.heldout_trained_solved,
        rep.tier1.heldout_total,
        rep.tier1.heldout_trained_avg_exp,
        rep.tier1.heldout_cold_avg_exp / rep.tier1.heldout_trained_avg_exp.max(1.0)
    );
    println!(
        "  Real `rustc -O -D warnings` OOD Certified = {}/8 | SkillLibrary Verified Idioms = {} | Time = {:.1} ms",
        rep.tier1.rustc_verified_total, rep.tier1.skills_learned, rep.tier1.wall_ms
    );
    for d in &rep.tier1.details {
        println!(
            "    - {:<21} ({:<7}) | solved={:<5} rustc={:<5} | exp={:>4} | run={:>6}us | mdl={}b",
            d.name,
            if d.is_heldout { "UNSEEN" } else { "train" },
            d.solved,
            d.rustc_verified,
            d.expansions,
            d.runtime_us,
            d.mdl_bits
        );
    }

    println!(
        "\n[Tier-2] {:?}: Nonlinear 2-MLP Neural-Ensemble World Model (Secret {:.1}-deg Rot + Vortex + QuadDrag + Hidden Obstacle)",
        rep.tier2.tier, rep.tier2.secret_angle_deg
    );
    println!(
        "  Active Epistemic Curiosity : {} disagreement probes | Solomonoff Compression = {:.1}x | StreamNorm = {:.4} | {:.1} ms",
        rep.tier2.epistemic_probes_taken, rep.tier2.compression_ratio, rep.tier2.brain_stream_norm, rep.tier2.wall_ms
    );
    println!(
        "  Unseen Nonlinear OOD Eval  : Random {}/{} -> 2-MLP Ensemble CEM {}/{} | Frontier -> (sig={:.3}, r={:.2})",
        rep.tier2.random_success,
        rep.tier2.eval_episodes,
        rep.tier2.ensemble_cem_success,
        rep.tier2.eval_episodes,
        frontier.physics_noise_std,
        frontier.physics_obs_radius
    );
    println!(
        "  Non-Stationary +{:.0}-deg Fault : Frozen Ensemble {}/{} -> Online Surprise-Adapting {}/{} (triggers={})",
        rep.tier2.fault_shift_deg,
        rep.tier2.fault_frozen_success,
        rep.tier2.fault_eval_episodes,
        rep.tier2.fault_online_adapted_success,
        rep.tier2.fault_eval_episodes,
        rep.tier2.surprise_adapt_triggers
    );

    println!("\n[Tier-3] {:?}: Fast Pure-Rust CEGIS Algebraic Discovery -> E-Graph (ZERO Lean Lag!)", rep.tier3.tier);
    println!(
        "  Proposed = {} | Refuted = {} | Rules Promoted = +{} (E-Graph total={}) | Time = {:.2} ms | {}",
        rep.tier3.proposed_pairs,
        rep.tier3.cegis_refuted,
        rep.tier3.rules_promoted,
        rep.tier3.egraph_rules_total,
        rep.tier3.wall_ms,
        rep.tier3.status
    );

    println!("\n[Self-Hosting Repo RSI & Unified Brain] Targets LIVE `{}` (`sha256={}`)", rep.rsi.target_live_file, rep.rsi.live_source_sha256);
    println!(
        "  Sealed OOD Pareto : {:.4} -> {:.4} (accepted={}, applied_live={}, cands={}, upgrade={}, ledger={})",
        rep.rsi.baseline_pareto_score,
        rep.rsi.evolved_pareto_score,
        rep.rsi.accepted,
        rep.rsi.applied_to_live_repo,
        rep.rsi.candidates_evaluated,
        rep.rsi.structural_upgrade,
        rep.rsi.ledger_sha256
    );
    println!("  Staged Candidate  : {}", rep.rsi.staged_path);
    println!(
        "  Unified Brain     : ||W_core||_inf = {:.4} (<1.0) | W_core L1 Drift = {:.4} | Online Steps = {} | Full-Brain SHA-256 = {} ({})",
        rep.contractive_linf_norm,
        rep.w_core_l1_drift,
        rep.train_steps_total,
        rep.ckpt_sha256,
        rep.ckpt_roundtrip_ok
    );
    println!("---------------------------------------------------------------------------------");
    println!(" Total Wall Time : {:.2} ms", rep.wall_ms);

    let json = format!(
        "{{\"engine\":\"TropiCore-Omega-v10.0-ASI\",\"dim\":{},\"abi\":{},\"ms\":{:.2},\"t1_rustc\":{},\"t1_cold_exp\":{:.1},\"t1_tr_exp\":{:.1},\"t2_comp\":{:.2},\"t2_cem\":[{},{}],\"t2_fault\":[{},{}],\"t3_rules\":{},\"t3_ms\":{:.2},\"rsi\":[{:.4},{:.4},{},{}],\"norm\":{:.4},\"dw\":{:.4},\"ckpt_ok\":{}}}",
        OMEGA_DIM,
        abi_ver,
        rep.wall_ms,
        rep.tier1.rustc_verified_total,
        rep.tier1.heldout_cold_avg_exp,
        rep.tier1.heldout_trained_avg_exp,
        rep.tier2.compression_ratio,
        rep.tier2.random_success,
        rep.tier2.ensemble_cem_success,
        rep.tier2.fault_frozen_success,
        rep.tier2.fault_online_adapted_success,
        rep.tier3.rules_promoted,
        rep.tier3.wall_ms,
        rep.rsi.baseline_pareto_score,
        rep.rsi.evolved_pareto_score,
        rep.rsi.accepted,
        rep.rsi.applied_to_live_repo,
        rep.contractive_linf_norm,
        rep.w_core_l1_drift,
        rep.ckpt_roundtrip_ok
    );
    match append_sealed_vault(&out_dir, "asi_v10_vault.jsonl", &json) {
        Ok(h) => println!(" Sealed ASI Vault: out/asi_v10_vault.jsonl (sha256={})", &h[..24]),
        Err(e) => println!(" Sealed ASI Vault Warning: {}", e),
    }
    println!("=================================================================================");
}

fn run_legacy_8axis_mode() {
    let hw = detect_hardware_profile();
    let abi_ver = omega_verify_sm120_abi_v8(
        OMEGA_DIM,
        NUM_TACTIC_CLASSES,
        WORLD_OBS_DIM,
        WORLD_BELIEF_DIM,
        WORLD_STATE_DIM,
    )
    .expect("SM120 ABI parity verification failed");
    let out_dir = PathBuf::from("out");
    let proof_path = find_core_proof();

    println!("=================================================================================");
    println!(" TropiCore-Omega v10.0 (--legacy-8axis) — Unified-Brain 8-Axis Suite (d={}, ABI={})", OMEGA_DIM, abi_ver);
    println!(" Host Backend  : {} (GPUs={}, CPU={}T)", hw.backend, hw.physical_gpus, hw.cpu_threads);
    println!("=================================================================================");

    let r = run_omega_v6_master_suite(&out_dir, &proof_path);

    println!("[Axis 1] Unified-Brain Raw Deep-Set + Contractive Core + Full-Brain Persistence");
    println!(
        "  ||W_core||_inf = {:.4} (<1.0) | W_enc Drift = {:.4} | W_core Drift = {:.4}",
        r.contractive_linf_norm, r.w_enc_l1_drift, r.w_core_l1_drift
    );
    println!(
        "  PCFG Frontier = {} (Wake solved = {}/24) -> {} Step-HER samples | STE Loss {:.4} -> {:.4}",
        r.wake_sleep_frontier_tasks, r.wake_tasks_solved, r.her_residual_samples, r.ste_loss_init, r.ste_loss_final
    );
    println!(
        "  GEMMs = {} | Ticks = {:?} | LIVE VM = {} | TacticFFI = {} | pert_L1 = {:.4}",
        r.gemm_dispatches, r.relax_ticks_4lanes, r.bytecode_vm_evals, r.tactic_policy_dispatches, r.nvfp4_perturbation_l1
    );
    println!(
        "  Autonomous Multi-Op Concepts = +{} (total={}) | Full-Brain SHA-256 Roundtrip = {}",
        r.autonomous_concepts_mined, r.memory_total_concepts, r.full_brain_ckpt_bit_exact
    );

    println!("\n[Axis 2] Cold vs Unified-Brain Trained + Live Meta-AST (6 Sealed Held-Out Tasks)");
    println!(
        "  Cold {}/{} ({:.1}) vs Trained {}/{} ({:.1}, Real rustc -O = {}/{})",
        r.heldout_cold_solved, r.heldout_total, r.heldout_cold_avg_exp,
        r.heldout_trained_solved, r.heldout_total, r.heldout_trained_avg_exp,
        r.heldout_rustc_verified, r.heldout_total
    );
    for d in &r.heldout_details {
        println!(
            "    - {:<21} ({:<10}) | Cold={}({:>4}) vs Tr={}({:>4}) | OE={:>4}/{:<5} | ops={} err={:.1e} eg={} | rustc={} | {}",
            d.name, d.domain, d.cold_solved, d.cold_expansions, d.trained_solved, d.trained_expansions,
            d.trained_oe_kept, d.trained_raw_seen, d.trained_leaf_ops, d.trained_residual_err, d.trained_egraph_unions,
            d.rustc_ood_verified, d.synthesized_pretty
        );
    }

    println!("\n[Axis 3] Neural-Guided Stagewise OMP + Dual-Number AD (Permuted + 10% Outliers)");
    println!(
        "  Clean {}/{} | 10% Outliers NonRob {}/{} -> Huber-OMP {}/{} | rustc={}/{}",
        r.continuous_clean_solved, r.continuous_clean_total,
        r.continuous_noisy_nonrobust_solved, r.continuous_noisy_total,
        r.continuous_noisy_robust_solved, r.continuous_noisy_total,
        r.continuous_rustc_verified, r.continuous_clean_total + r.continuous_noisy_total
    );
    for c in &r.continuous_details {
        println!(
            "    - {:<26} (noisy={:<5} ad={:<5}) | nonRob={:.3e} | OMP-AD={:.3e} | rustc={} | {}",
            c.name, c.noisy, c.used_continuous_ad, c.non_robust_ood_err, c.omega_ad_ood_err, c.rustc_ood_verified, c.synthesized_pretty
        );
    }

    println!("\n[Axis 4] Raw-Grammar Functional/Rec + `FoldSeq` 1,000-Elem DP");
    println!(
        "  VM diff={:.2e} | Func/Rec {}/{} (rustc={}/{}) | FoldSeq 1000-DP {}/{} (rustc={}/{})",
        r.bytecode_vm_max_diff, r.func_rec_solved, r.func_rec_total, r.func_rec_rustc_verified, r.func_rec_total,
        r.seq_dp_solved, r.seq_dp_total, r.seq_dp_rustc_1000_verified, r.seq_dp_total
    );

    println!("\n[Axis 5] Uniform Depth-3 Conservation `CellExpr`/`CoordExpr`");
    println!(
        "  DAG {:.1}% -> Fix {:.1}% | FlipV={} RayLeft={} | CEGIS {}->{} | Mem {:.1}%->{:.1}%",
        r.dag_4x_acc * 100.0, r.omega_4x_acc * 100.0, r.unseen_flipv_rustc_ok, r.unseen_rayleft_rustc_ok,
        r.cegis_off_evals, r.cegis_on_evals, r.grid_mem_off_acc * 100.0, r.grid_mem_on_acc * 100.0
    );

    println!("\n[Axis 6] Algebra / Optional Lean 4 Oracle -> E-Graph");
    println!(
        "  Goals={} States={} Tactics={} Reused={} W_tac_q={} ({:.3}->{:.3}) | Alg={} Concept={} Nonlin={} ({} pairs={} ref={}) | File={}",
        r.prover_goals_attempted, r.prover_states_inspected, r.prover_tactics_tried, r.prover_lemmas_reused,
        r.prover_neural_queries, r.prover_tactic_loss_init, r.prover_tactic_loss_final,
        r.prover_alg_theorems, r.prover_concept_props, r.prover_nonlinear_induction, r.prover_lean4_status,
        r.prover_proposed, r.prover_refuted, r.prover_lean4_file
    );
    println!(
        "  E-Graph Rules {}->{} (+{}) | MDL {}->{} (unions={})",
        r.egraph_rules_before, r.egraph_rules_after, r.egraph_rules_promoted,
        r.egraph_mdl_before, r.egraph_mdl_after, r.egraph_unions_after
    );

    println!("\n[Axis 7] Local AST Repair + Double-Permuted Egocentric World");
    println!(
        "  Repair {}/{} ({} patches) | 6x6 Rand {}/{} Early {}/{} -> Learned {}/{} (tr={}, dynMSE={:.4}, rewMAE={:.4}, lieInv={:.4}) | 10x10 OOD Rand {}/{} -> Learned {}/{}",
        r.repair_solved, r.repair_total, r.repair_local_patches,
        r.world_report.random_wins_6x6, r.world_report.eval_episodes,
        r.world_report.early_wins_6x6, r.world_report.eval_episodes,
        r.world_report.learned_wins_6x6, r.world_report.eval_episodes,
        r.world_report.total_transitions_learned, r.world_report.dyn_mse_final, r.world_report.rew_mae_final, r.world_report.act_inverse_err,
        r.world_report.random_wins_10x10_ood, r.world_report.eval_episodes,
        r.world_report.learned_wins_10x10_ood, r.world_report.eval_episodes
    );

    println!("\n[Axis 8] Koza GP Crossover Meta-AST + Scaling + Exact 3D HV");
    println!(
        "  Meta-AST ValSolved {}->{} | ValExp {}->{} (cands={}, xover={}, acc={}, temp={:.2}, rustc={}) | [mdl={}] {} | path={}",
        r.meta_report.baseline_val_solved, r.meta_report.improved_val_solved,
        r.meta_report.baseline_val_expansions, r.meta_report.improved_val_expansions,
        r.meta_report.candidates_evaluated, r.meta_report.crossover_children_evaluated,
        r.meta_report.accepted_improvement, r.meta_report.winning_strategy.neural_temp,
        r.meta_report.emitted_rust_verified, r.meta_report.winning_meta_ast_mdl,
        r.meta_report.winning_meta_ast_pretty, r.meta_report.emitted_rust_path
    );
    println!(
        "  Ops {}<{}<{} | Exact HV {:.6}<={:.6}<={:.6} (mono={}) | CoreProof={}",
        r.ops_b1, r.ops_b2, r.ops_b4, r.hv_b1, r.hv_b2, r.hv_b4, r.hv_strict_mono, r.core_proof_lean4_status
    );
    println!(" Total Wall Time : {:.2} ms", r.wall_ms);

    let json = format!(
        "{{\"engine\":\"TropiCore-Omega-v10.0-8axis\",\"dim\":{},\"ms\":{:.2},\"norm\":{:.4},\"cold\":[{},{:.1}],\"trained\":[{},{:.1}],\"egraph_mdl\":[{},{}],\"world_10x10\":{},\"meta_acc\":{}}}",
        OMEGA_DIM, r.wall_ms, r.contractive_linf_norm,
        r.heldout_cold_solved, r.heldout_cold_avg_exp,
        r.heldout_trained_solved, r.heldout_trained_avg_exp,
        r.egraph_mdl_before, r.egraph_mdl_after,
        r.world_report.learned_wins_10x10_ood, r.meta_report.accepted_improvement
    );
    match append_sealed_vault(&out_dir, "omega_v8_vault.jsonl", &json) {
        Ok(h) => println!(" Sealed Vault    : out/omega_v8_vault.jsonl (sha256={})", &h[..24]),
        Err(e) => println!(" Sealed Vault Warning: {}", e),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }
    let (cycles, ckpt, apply_rsi, worker_threads) = parse_daemon_flags(&args, 5, "out/asi_v10_daemon.ckpt");
    if args.iter().any(|a| a == "--asi-forever") {
        run_asi_forever_daemon(&ckpt, cycles, apply_rsi, worker_threads);
        return;
    }
    if args.iter().any(|a| a == "--learn-forever") {
        let (_, lckpt, _, _) = parse_daemon_flags(&args, 5, "out/omega_v10_daemon.ckpt");
        run_continuous_learning_daemon(&lckpt, cycles);
        return;
    }
    if args.iter().any(|a| a == "--legacy-8axis") {
        run_legacy_8axis_mode();
        return;
    }
    run_asi_smoke_mode(apply_rsi, worker_threads);
}
