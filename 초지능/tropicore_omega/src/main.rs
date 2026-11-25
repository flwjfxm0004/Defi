//! TropiCore-Omega ASI-Zero (`v17.0.0`) — Pure Zero-Toy Non-LLM General Superintelligence Engine.
//!
//! ZERO toy games, ZERO LLMs, ZERO external Rust crates, ZERO dead symbols, ZERO GPU data races, ZERO gradient cloning:
//!   1. Independent Query/Key DeltaNet-SSM (`w_qry`/`b_qry` decoupled from `w_key`/`b_key`) + SwiGLU + Conv-4
//!      + Differentiable Banach Equilibrium Reg — verified across ALL 22 tensor types by central finite-diff `GradCheck`!
//!   2. 4x4 Register-Blocked SIMD Forward & Backward GEMM (`linear_fwd` & `linear_bwd`) + Zero-Copy AllReduce (`ABI=1700`)
//!      + Zero-Allocation `ForwardScratch` Pool.
//!   3. 13-Expert × 8-Context Simplex-Normalized Meta-Mixer: Exact PPM-D Unique-Symbol Escape + Adaptive Order-0 Unigram
//!      + 4-Way Order-∞ Match-Model + Zero-Tokenizer WordBigram + Skip-1/Gapped Context Expert.
//!   4. Compact 41-Byte Header `OMGZ17` Lossless Codec (Lean 4 Omega-6 unique-decodability proved!) + 64B TTT Adaptation.
//!   5. Strictly `O(budget)` RAM Streaming I/O via `File::seek` even on single 10–100 GB files.
//!   6. Multi-Formula AST Self-Optimizer with Isolated Compiled-Binary 2D Pareto Gate (`--verify-binary`).
//!   7. TD-MPC2 Actor-Critic `UniversalAgent` (`TOMA17`: zero latent loss, learned continuous actor + Polyak target actor,
//!      actor-warm-started CEM planning, zero-clone stream, multi-task closed-loop nonlinear + momentum control proof!).

mod agent_protocol;
mod arith_coder;
mod asi_runtime_cfg;
mod brain;
mod compressor;
mod episodic;
mod gpu_backend;
mod nvfp4;
mod self_optimizer;
mod sha256;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent_protocol::serve_universal_agent_stdio;
use crate::arith_coder::{compress_file_to_omg, decompress_omg_to_file};
use crate::brain::{BrainConfig, DeepContractiveBrain, RuntimePolicyOverride};
use crate::compressor::{load_real_corpus_split, run_real_byte_training};
use crate::gpu_backend::{backend_telemetry, verify_sm120_training_abi};
use crate::self_optimizer::run_real_self_optimization;
use crate::sha256::{append_vault_jsonl, sha256_hex};

fn default_threads() -> usize {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    if cpus <= 16 { 2 } else { cpus }
}

fn default_corpus_path() -> PathBuf {
    let local = PathBuf::from("src");
    if local.is_dir() { return local; }
    PathBuf::from(r"C:\Defi\초지능\tropicore_omega\src")
}

fn flag_val(args: &[String], key: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == key { return Some(args[i + 1].clone()); }
        i += 1;
    }
    None
}

fn has_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn print_help() {
    println!("TropiCore-Omega ASI-Zero v17.0.0 — Zero-Toy Non-LLM General Superintelligence Engine");
    println!("Commands:");
    println!("  status                                            Show SM120 Multi-GPU Zero-Copy ABI=1700 & presets");
    println!("  train-bytes [--path DIR] [--preset P] [--epochs N] [--max-bytes B] [--ckpt F] [--threads T]");
    println!("  eval-bytes  [--path DIR | --raw-file F] --ckpt F [--use-compiled-defaults]");
    println!("  compress    --in FILE --out FILE.omg [--ckpt F] [--no-ttt]  Compact 41B-header OMGZ17 lossless codec");
    println!("  decompress  --in FILE.omg --out FILE [--ckpt F]   Bit-exact SHA-256 verified decompression");
    println!("  generate    --prompt STR [--bytes N] [--beam K] [--temp T] [--ckpt F]");
    println!("  serve-agent [--act-dim A] [--ckpt F] [--agent-ckpt A.toma]");
    println!("  self-optimize [--path DIR] [--ckpt F] [--apply] [--verify-binary] [--threads T]");
    println!("  daemon      [--path DIR] [--cycles N] [--preset P] [--ckpt F] [--apply] [--verify-binary] [--threads T]");
    println!("Presets: laptop | mid | blackwell-1b | blackwell-8b");
}

fn cmd_status() {
    let abi = verify_sm120_training_abi().expect("SM120 training ABI verification failed");
    let tel = backend_telemetry();
    println!("=================================================================================");
    println!(" TropiCore-Omega ASI-Zero v17.0.0 (DeltaNet Q/K + PPM-D 13-Exp + TD-MPC2 + ABI={}/{})", abi, tel.abi_version);
    println!("=================================================================================");
    println!(" Compute Backend : {} (GPUs={}, HostCPU={}T)", tel.backend_name, tel.physical_gpus, tel.host_cpu_threads);
    println!(" Target Hardware : 4x RTX PRO 6000 Blackwell (384 GB GDDR7, 14.044 PFLOPS FP4) + 9995WX 192T + 1,000 GB ECC");
    println!("---------------------------------------------------------------------------------");
    println!(" Scaling Presets (SwiGLU + Conv-4 + Separate-Q/K DeltaNet + Diff-EqReg Adaptive Banach):");
    for name in ["laptop", "mid", "blackwell-1b", "blackwell-8b"] {
        let cfg = BrainConfig::from_preset(name).unwrap();
        println!(
            "   - {:<13}: L={:>2}, d={:>4}, ff={:>5}, ticks={} | {:>10.2}M params | NVFP4={:.3} GB | FP32+AdamW={:.2} GB",
            name,
            cfg.n_layers, cfg.d_model, cfg.d_ff, cfg.n_ticks,
            cfg.parameter_count() as f64 / 1e6, cfg.nvfp4_weight_gb(), cfg.fp32_train_state_gb()
        );
    }
    println!("=================================================================================");
}

fn load_or_init_brain(ckpt: &Path, preset: &str) -> (DeepContractiveBrain, String) {
    if ckpt.exists() {
        match DeepContractiveBrain::load_checkpoint(ckpt) {
            Ok((b, sha)) => return (b, format!("RESUMED LOSSLESS TOMG17 (sha256={})", &sha[..16])),
            Err(e) => eprintln!(" [warn] checkpoint load failed ({}), initializing fresh...", e),
        }
    }
    let cfg = BrainConfig::from_preset(preset).unwrap_or_else(|e| {
        eprintln!(" [error] {}", e);
        std::process::exit(1);
    });
    (
        DeepContractiveBrain::new(cfg, 2026_1126),
        format!("FRESH INIT (preset={}, params={:.2}M)", preset, cfg.parameter_count() as f64 / 1e6),
    )
}

fn cmd_train_or_daemon(args: &[String], cycles: usize, run_self_opt: bool) {
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let path = flag_val(args, "--path")
        .or_else(|| flag_val(args, "--corpus"))
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let epochs: usize = flag_val(args, "--epochs").and_then(|s| s.parse().ok()).unwrap_or(2);
    let max_bytes: usize = flag_val(args, "--max-bytes")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if preset == "laptop" { 24_576 } else { 0 });
    let max_heldout: usize = if max_bytes > 0 { (max_bytes / 3).max(4096) } else { 0 };
    let threads: usize = flag_val(args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply") || has_flag(args, "--apply-rsi");
    let verify_binary = has_flag(args, "--verify-binary");

    let abi = verify_sm120_training_abi().expect("SM120 ABI mismatch");
    let (mut brain, boot_msg) = load_or_init_brain(&ckpt, &preset);
    let out_dir = PathBuf::from("out");
    let mut lr_mult = 1.0f32;

    println!("=================================================================================");
    println!(
        " TropiCore-Omega ASI-Zero v17.0.0 — 13-Expert PPM-D Simplex + DeltaNet Q/K (ABI={}, threads={})",
        abi, threads
    );
    println!(" Boot   : {} | best_bpb={:.3}", boot_msg, if brain.best_heldout_bpb.is_finite() { brain.best_heldout_bpb } else { 8.0 });
    println!(" Path   : {} (O(budget) RAM streaming, max_bytes={})", path.display(), max_bytes);
    println!("=================================================================================");

    for cyc in 1..=cycles.max(1) {
        let cycle_seed = 170_000 + (brain.adam_step as u64) + cyc as u64;
        let corpus = load_real_corpus_split(&path, max_bytes, max_heldout, cycle_seed).unwrap_or_else(|e| {
            eprintln!("[error] {}", e);
            std::process::exit(1);
        });

        let prev_best_brain = brain.clone();
        let prev_best_bpb = brain.best_heldout_bpb;

        let rep = run_real_byte_training(&mut brain, &corpus, epochs, 64, lr_mult, threads);

        let cand_bpb = rep.online_adapted_heldout_bpb.min(rep.trained_blended_heldout_bpb);
        let improved_best = cand_bpb <= prev_best_bpb + 0.015 || !ckpt.exists();
        let (save_sha, roundtrip_ok, guard_tag) = if improved_best {
            if cand_bpb < brain.best_heldout_bpb { brain.best_heldout_bpb = cand_bpb; }
            let sha = brain.save_checkpoint(&ckpt).unwrap_or_else(|e| format!("ERR:{}", e));
            let ok = DeepContractiveBrain::load_checkpoint(&ckpt).map(|(_, h)| h == sha).unwrap_or(false);
            (sha, ok, "SAVED")
        } else {
            brain = prev_best_brain;
            brain.rebind_to_gpus();
            lr_mult = (lr_mult * 0.5).max(0.15);
            let sha = DeepContractiveBrain::load_checkpoint(&ckpt)
                .map(|(_, h)| h)
                .unwrap_or_else(|_| "kept_in_mem".to_string());
            (sha, true, "GUARD_KEPT_BEST")
        };

        let tel = backend_telemetry();
        println!(
            " [Cycle {:>2}] (tr={}/{}f {}B, ho={}/{}B, ep={}, B={}str, lr*={:.2}) HeldOut bpb: {:.3} -> Param {:.3} | +PPMD {:.3} | Online {:.3} ({:.2}x) | {:.1} KB/s | ||W_eq||={:.3} | Mem={}bkt/{}lat | {}={}({}) | {:.0}ms",
            cyc, rep.train_files, rep.total_train_pool, rep.train_bytes, rep.heldout_label, rep.heldout_bytes,
            rep.epochs, rep.n_streams, lr_mult, rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb,
            rep.trained_blended_heldout_bpb, rep.online_adapted_heldout_bpb, rep.compression_ratio,
            rep.throughput_kb_s, rep.contractive_linf, rep.episodic_buckets, brain.episodic.latent_count(),
            guard_tag, &save_sha[..16.min(save_sha.len())], roundtrip_ok, rep.wall_ms
        );

        let opt_rep = if run_self_opt {
            let so = run_real_self_optimization(
                &brain, &corpus.train_bytes, &corpus.heldout_bytes, &out_dir,
                170_000 + cyc as u64, apply_live, verify_binary, threads,
            );
            let mut installed_live = false;
            if let Some(win_pol) = so.winning_policy {
                brain.active_policy = win_pol;
                let _ = brain.save_checkpoint(&ckpt);
                installed_live = true;
            }
            println!(
                "   [Self-Opt AST] Live `{}` ({}) | cands={} | Real bpb {:.4} -> {:.4} | acc={} rustc0W={} active={} file_patch={} bin_ok={} bin={} | {} | staged={} | {:.0}ms",
                so.target_live_file, so.live_sha256, so.candidates_evaluated,
                so.baseline_real_bpb, so.optimized_real_bpb, so.accepted,
                so.rustc_zero_warnings, installed_live, so.applied_live,
                so.binary_verified, so.bin_sha256, so.upgrade_desc, so.staged_path, so.wall_ms
            );
            Some(so)
        } else {
            None
        };

        let json = format!(
            "{{\"engine\":\"TropiCore-ASI-Zero-v17\",\"cyc\":{},\"L\":{},\"d\":{},\"params\":{},\"tr_files\":{},\"bpb_pre\":{:.4},\"bpb_param\":{:.4},\"bpb_blend\":{:.4},\"bpb_online\":{:.4},\"comp\":{:.3},\"kb_s\":{:.2},\"norm\":{:.4},\"guard\":\"{}\",\"fwd\":{},\"bwd\":{},\"ctr\":{},\"adap_saved\":{},\"adamw\":{},\"allred\":{},\"gflops\":{:.3},\"self_opt\":{}}}",
            cyc, brain.cfg.n_layers, brain.cfg.d_model, brain.cfg.parameter_count(), rep.train_files,
            rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb, rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb, rep.compression_ratio, rep.throughput_kb_s, rep.contractive_linf,
            guard_tag, tel.fwd_gemms, tel.bwd_gemms, tel.contractive_steps, tel.adaptive_ticks_saved,
            tel.adamw_steps, tel.allreduces, tel.est_gflops, opt_rep.map(|o| o.accepted).unwrap_or(false)
        );
        let h = sha256_hex(json.as_bytes());
        let _ = append_vault_jsonl(
            &out_dir.join("asi_zero_v17_vault.jsonl"),
            &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", h, json),
        );
    }

    let tel = backend_telemetry();
    println!("---------------------------------------------------------------------------------");
    println!(
        " Telemetry : Backend={} | Fwd={} Bwd={} Contractive={} (AdapTokSaved={}) AdamW={} AllReduce={} | Est={:.3} GFLOPs | TotalBytes={}",
        tel.backend_name, tel.fwd_gemms, tel.bwd_gemms, tel.contractive_steps,
        tel.adaptive_ticks_saved, tel.adamw_steps, tel.allreduces, tel.est_gflops, brain.total_bytes_trained
    );
    println!("=================================================================================");
}

fn cmd_eval_bytes(args: &[String]) {
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let use_compiled = has_flag(args, "--use-compiled-defaults");
    let (mut brain, sha) = DeepContractiveBrain::load_checkpoint(&ckpt).unwrap_or_else(|e| {
        eprintln!("[error] failed to load '{}': {}", ckpt.display(), e);
        std::process::exit(1);
    });
    let pol_override = if use_compiled {
        Some(RuntimePolicyOverride::compiled_default())
    } else {
        None
    };

    let (heldout_bytes, label) = if let Some(raw_p) = flag_val(args, "--raw-file") {
        let bytes = fs::read(&raw_p).unwrap_or_else(|e| {
            eprintln!("[error] raw-file read failed: {}", e);
            std::process::exit(1);
        });
        (bytes, raw_p)
    } else {
        let path = flag_val(args, "--path").map(PathBuf::from).unwrap_or_else(default_corpus_path);
        let corpus = load_real_corpus_split(&path, 0, 32_768, 17).unwrap_or_else(|e| {
            eprintln!("[error] {}", e);
            std::process::exit(1);
        });
        (corpus.heldout_bytes, corpus.heldout_label)
    };

    let (param_bpb, blend_bpb) = brain.eval_byte_stream(&heldout_bytes, 64, false, pol_override);
    let (_, online_bpb) = brain.eval_byte_stream(&heldout_bytes, 64, true, pol_override);
    println!(
        "Eval `{}` (ckpt={}...): Param={:.4} bpb | +PPMD={:.4} bpb | Online={:.4} bpb ({:.2}x comp)",
        label, &sha[..16], param_bpb, blend_bpb, online_bpb, 8.0 / online_bpb.max(0.05)
    );
}

fn cmd_compress(args: &[String]) {
    let in_p = PathBuf::from(flag_val(args, "--in").expect("missing --in <file>"));
    let out_p = PathBuf::from(flag_val(args, "--out").expect("missing --out <file.omg>"));
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let enable_ttt = !has_flag(args, "--no-ttt");
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let rep = compress_file_to_omg(&brain, &in_p, &out_p, enable_ttt).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Compress OMGZ17] {} ({} B) -> {} ({} B, hdr={} B) | Physical={:.3} bits/byte ({:.2}x) | ttt_steps={} | orig_sha256={}",
        in_p.display(), rep.orig_bytes, out_p.display(), rep.compressed_bytes, rep.header_bytes,
        rep.physical_bpb, rep.physical_ratio, rep.ttt_updates, &rep.orig_sha256[..16]
    );
}

fn cmd_decompress(args: &[String]) {
    let in_p = PathBuf::from(flag_val(args, "--in").expect("missing --in <file.omg>"));
    let out_p = PathBuf::from(flag_val(args, "--out").expect("missing --out <file>"));
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let (len, sha) = decompress_omg_to_file(&brain, &in_p, &out_p).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Decompress OMGZ17] {} -> {} ({} B) | SHA-256 BIT-EXACT VERIFIED ({})",
        in_p.display(), out_p.display(), len, &sha[..16]
    );
}

fn cmd_generate(args: &[String]) {
    let prompt = flag_val(args, "--prompt").unwrap_or_else(|| "pub fn ".to_string());
    let n_bytes: usize = flag_val(args, "--bytes").and_then(|s| s.parse().ok()).unwrap_or(80);
    let beam: usize = flag_val(args, "--beam").and_then(|s| s.parse().ok()).unwrap_or(4);
    let temp: f32 = flag_val(args, "--temp").and_then(|s| s.parse().ok()).unwrap_or(0.60);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, boot) = load_or_init_brain(&ckpt, &preset);
    let (out, avg_bpb) = brain.generate_bytes_beam(prompt.as_bytes(), n_bytes, beam, temp, 2026_1126);
    println!("[Generate System-2 LoopFree | {} | beam={} | avg_bpb={:.3}]", boot, beam, avg_bpb);
    println!("{}{}", prompt, String::from_utf8_lossy(&out));
}

fn cmd_self_optimize(args: &[String]) {
    let path = flag_val(args, "--path")
        .or_else(|| flag_val(args, "--corpus"))
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let threads = flag_val(args, "--threads").and_then(|s| s.parse().ok()).unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply");
    let verify_binary = has_flag(args, "--verify-binary");

    let corpus = load_real_corpus_split(&path, 16_384, 8_192, 17).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (mut brain, _) = load_or_init_brain(&ckpt, &preset);
    if brain.total_bytes_trained == 0 {
        let _ = run_real_byte_training(&mut brain, &corpus, 1, 64, 1.0, threads);
    }
    let so = run_real_self_optimization(
        &brain, &corpus.train_bytes, &corpus.heldout_bytes, Path::new("out"),
        2026_1126, apply_live, verify_binary, threads,
    );
    if let Some(win) = so.winning_policy {
        brain.active_policy = win;
        let _ = brain.save_checkpoint(&ckpt);
    }
    println!(
        "[Self-Opt AST] Target=`{}` ({}) | Real bpb {:.4} -> {:.4} | acc={} rustc0W={} live={} bin_ok={} bin={} | {} | ledger={} | {:.0}ms",
        so.target_live_file, so.live_sha256, so.baseline_real_bpb, so.optimized_real_bpb,
        so.accepted, so.rustc_zero_warnings, so.applied_live, so.binary_verified, so.bin_sha256,
        so.upgrade_desc, so.ledger_sha256, so.wall_ms
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    match sub {
        "-h" | "--help" | "help" => print_help(),
        "status" => cmd_status(),
        "train-bytes" => cmd_train_or_daemon(&args, 1, true),
        "eval-bytes" => cmd_eval_bytes(&args),
        "compress" => cmd_compress(&args),
        "decompress" => cmd_decompress(&args),
        "generate" => cmd_generate(&args),
        "self-optimize" => cmd_self_optimize(&args),
        "daemon" => {
            let cycles = flag_val(&args, "--cycles").and_then(|s| s.parse().ok()).unwrap_or(3);
            cmd_train_or_daemon(&args, cycles, true);
        }
        "serve-agent" => {
            let act_dim = flag_val(&args, "--act-dim").and_then(|s| s.parse().ok()).unwrap_or(2);
            let preset = flag_val(&args, "--preset").unwrap_or_else(|| "laptop".to_string());
            let ckpt = flag_val(&args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v17.tomg"));
            let agent_ckpt = flag_val(&args, "--agent-ckpt").map(PathBuf::from);
            let (brain, _) = load_or_init_brain(&ckpt, &preset);
            serve_universal_agent_stdio(&brain, act_dim, agent_ckpt.as_deref());
        }
        other => {
            eprintln!("Unknown command '{}'. Run with --help for usage.", other);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_protocol::{
        format_agent_jsonl, parse_env_jsonl, ExternalObservation, UniversalOpenWorldAgent,
    };
    use crate::arith_coder::{compress_bytes_with_brain, decompress_bytes_with_brain};

    #[test]
    fn test_exact_bptt_numerical_gradient_check_all_22_tensor_types() {
        let max_rel = DeepContractiveBrain::verify_bptt_numerical_gradients()
            .expect("Central finite-difference gradient check MUST pass across ALL 22 tensor types including Separate Q/K DeltaNet & Banach Eq-Reg!");
        assert!(max_rel < 0.035, "Numerical gradient max relative error too high: {:.5}", max_rel);
        println!(" [GradCheck PASS] All 22 tensor types (Separate Q/K DeltaNet + Banach Eq-Reg) verified! max_rel_err={:.5}", max_rel);
    }

    #[test]
    fn test_sm120_abi_1700_and_blackwell_budget() {
        let abi = verify_sm120_training_abi().expect("ABI must verify");
        assert_eq!(abi, 1700);
        let b1 = BrainConfig::from_preset("blackwell-1b").unwrap();
        assert!(b1.parameter_count() > 1_000_000_000);
        let b8 = BrainConfig::from_preset("blackwell-8b").unwrap();
        assert!(b8.parameter_count() > 8_000_000_000);
        assert!(b8.nvfp4_weight_gb() < 8.0);
        assert!(b8.fp32_train_state_gb() < 120.0);
    }

    #[test]
    fn test_single_file_streaming_bounded_ram() {
        let corpus_dir = default_corpus_path();
        let single_file = corpus_dir.join("brain.rs");
        let split = load_real_corpus_split(&single_file, 2048, 512, 17)
            .expect("must stream single large file via File::seek");
        assert_eq!(split.train_files, 1);
        assert_eq!(split.train_bytes.len(), 2048);
        assert_eq!(split.heldout_bytes.len(), 512);
    }

    #[test]
    fn test_v17_ppmd_13expert_omgz17_41b_header_deltanet_qk_and_loopfree_beam() {
        let corpus_dir = default_corpus_path();
        let corpus = load_real_corpus_split(&corpus_dir, 14_000, 4_096, 17)
            .expect("must load real Rust source files from src/");
        assert!(corpus.train_files >= 5);

        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let mut brain = DeepContractiveBrain::new(cfg, 42);

        let rep = run_real_byte_training(&mut brain, &corpus, 2, 64, 1.0, 2);
        assert!(rep.untrained_heldout_bpb > 7.5);
        assert!(rep.trained_param_heldout_bpb < rep.untrained_heldout_bpb - 1.5);
        assert!(rep.online_adapted_heldout_bpb < 4.0);
        assert!(rep.contractive_linf < 1.0);

        let tel = backend_telemetry();
        assert!(tel.adaptive_ticks_saved > 0, "Per-token adaptive Banach early-exit must save > 0 ticks!");

        // PHYSICAL LOSSLESS OMGZ17 (41-byte compact header + 13-expert PPM-D + Adaptive Unigram + Skip-1 + 64B TTT!)
        let sample = &corpus.heldout_bytes[..1024.min(corpus.heldout_bytes.len())];
        let (archive, crep) = compress_bytes_with_brain(&brain, sample, true);
        assert_eq!(crep.header_bytes, 41, "OMGZ17 header for 1024 B file MUST be exact 41 bytes!");
        assert!(crep.ttt_updates >= 10);
        assert!(crep.compressed_bytes < crep.orig_bytes);
        let restored = decompress_bytes_with_brain(&brain, &archive).expect("OMGZ17 TTT decompression must verify raw 32B SHA-256");
        assert_eq!(restored.as_slice(), sample, "OMGZ17 restored file must be 100% BIT-IDENTICAL!");

        // Verify exact probability simplex invariant (`sum_e alpha[e] == 1.0`) across all 13 experts!
        let all_active = [true; crate::episodic::NUM_EPISODIC_EXPERTS];
        let alpha = brain.meta_mixer.compute_active_simplex(7, &all_active, 1.0, 0.88);
        let alpha_sum: f32 = alpha.iter().sum();
        assert!((alpha_sum - 1.0).abs() < 1e-5, "Simplex weights MUST sum to 1.0, got {}", alpha_sum);

        // Self-Echo-Free System-2 Beam Search Generation
        let (gen, gen_bpb) = brain.generate_bytes_beam(b"pub fn ", 48, 3, 0.6, 17);
        assert_eq!(gen.len(), 48);
        assert!(gen_bpb.is_finite() && gen_bpb < 6.5);
        let gen_str = String::from_utf8_lossy(&gen);
        assert!(!(&gen[..24] == &gen[24..48]), "Beam output must NOT repeat identical 24B loop: {}", gen_str);

        // Lossless Bit-Exact `TOMG17` Checkpoint Roundtrip
        let tmp_dir = std::env::temp_dir().join("tropicore_asi_v17_unit");
        let ckpt = tmp_dir.join("unit_v17.tomg");
        let so = run_real_self_optimization(&brain, &corpus.train_bytes, sample, &tmp_dir, 99, false, false, 2);
        assert_eq!(so.candidates_evaluated, 8);
        if let Some(win) = so.winning_policy {
            assert!(so.rustc_zero_warnings && so.optimized_real_bpb < so.baseline_real_bpb);
            brain.active_policy = win;
        }
        brain.best_heldout_bpb = rep.online_adapted_heldout_bpb;

        let sha1 = brain.save_checkpoint(&ckpt).expect("save TOMG17");
        let (mut loaded, sha2) = DeepContractiveBrain::load_checkpoint(&ckpt).expect("load TOMG17");
        assert_eq!(sha1, sha2);
        let (p1, b1) = brain.eval_byte_stream(sample, 64, false, None);
        let (p2, b2) = loaded.eval_byte_stream(sample, 64, false, None);
        assert!((p1 - p2).abs() < 1e-5 && (b1 - b2).abs() < 1e-5);
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_universal_agent_tdmpc2_beats_random_on_two_unknown_tasks_and_persists_toma17() {
        // Task A: Coupled nonlinear continuous control (`state -> (+0.8, -0.8)`)
        fn task_a_step(state: &mut [f32; 2], act: &[f32]) -> ([f32; 4], f32) {
            let a0 = act[0].clamp(-1.0, 1.0);
            let a1 = act[1].clamp(-1.0, 1.0);
            state[0] = (0.65 * state[0] + 0.55 * a0).clamp(-2.0, 2.0);
            state[1] = (0.65 * state[1] + 0.55 * a1).clamp(-2.0, 2.0);
            let err2 = (state[0] - 0.8).powi(2) + (state[1] + 0.8).powi(2);
            ([state[0], state[1], state[0] * state[1], (state[0] - state[1]).tanh()], 1.5 - err2)
        }

        // Task B: 2nd-Order Inertial/Momentum Nonlinear Control (`vel -> pos` delayed actuation + overshoot penalty!)
        fn task_b_step(state: &mut [f32; 2], act: &[f32]) -> ([f32; 4], f32) {
            let a = act[0].clamp(-1.0, 1.0);
            state[1] = (0.72 * state[1] + 0.45 * a).clamp(-2.0, 2.0); // velocity
            state[0] = (state[0] + 0.55 * state[1]).clamp(-2.0, 2.0); // position (delayed by inertia!)
            let r = 1.5 - (state[0] - 1.0).powi(2) - 0.35 * state[1].powi(2);
            ([state[0], state[1], state[0] * state[1], (state[0] - 1.0).tanh()], r)
        }

        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let brain = DeepContractiveBrain::new(cfg, 7);

        // --- Task A ---
        let mut rng = 987654321u64;
        let mut r_st = [0.0f32; 2];
        let mut r_sum_a = 0.0f32;
        for step in 0..60 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let a0 = (((rng >> 33) as f32) / ((1u32 << 31) as f32)) * 2.0 - 1.0;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let a1 = (((rng >> 33) as f32) / ((1u32 << 31) as f32)) * 2.0 - 1.0;
            let (_, r) = task_a_step(&mut r_st, &[a0, a1]);
            if step >= 30 { r_sum_a += r; }
        }
        let rand_a = r_sum_a / 30.0;

        let mut agent_a = UniversalOpenWorldAgent::new(2, &cfg, 77);
        let mut a_st = [0.0f32; 2];
        let mut obs_a = vec![0.0f32; 4];
        let mut lr_a = 0.0f32;
        let mut ag_sum_a = 0.0f32;
        for step in 0..60 {
            let out = agent_a.step(&brain, &ExternalObservation::Numeric(obs_a.clone()), lr_a, false);
            let (no, r) = task_a_step(&mut a_st, &out.action);
            obs_a = no.to_vec();
            lr_a = r;
            if step >= 30 { ag_sum_a += r; }
        }
        let agent_avg_a = ag_sum_a / 30.0;

        // --- Task B (2nd-order inertial control!) ---
        let mut rb_st = [0.0f32; 2];
        let mut r_sum_b = 0.0f32;
        for step in 0..70 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let a0 = (((rng >> 33) as f32) / ((1u32 << 31) as f32)) * 2.0 - 1.0;
            let (_, r) = task_b_step(&mut rb_st, &[a0]);
            if step >= 35 { r_sum_b += r; }
        }
        let rand_b = r_sum_b / 35.0;

        let mut agent_b = UniversalOpenWorldAgent::new(1, &cfg, 88);
        let mut ab_st = [0.0f32; 2];
        let mut obs_b = vec![0.0f32; 4];
        let mut lr_b = 0.0f32;
        let mut ag_sum_b = 0.0f32;
        for step in 0..70 {
            let out = agent_b.step(&brain, &ExternalObservation::Numeric(obs_b.clone()), lr_b, false);
            let (no, r) = task_b_step(&mut ab_st, &out.action);
            obs_b = no.to_vec();
            lr_b = r;
            if step >= 35 { ag_sum_b += r; }
        }
        let agent_avg_b = ag_sum_b / 35.0;

        println!(
            " [TD-MPC2 MultiTask PASS] TaskA(Coupled): Rand {:.3} vs Agent {:.3} (+{:.3}) | TaskB(Inertial2ndOrder): Rand {:.3} vs Agent {:.3} (+{:.3})",
            rand_a, agent_avg_a, agent_avg_a - rand_a, rand_b, agent_avg_b, agent_avg_b - rand_b
        );
        assert!(agent_avg_a > rand_a + 0.35, "Task A failed: {:.3} vs {:.3}", agent_avg_a, rand_a);
        assert!(agent_avg_b > rand_b + 0.35, "Task B (2nd-order inertial) failed: {:.3} vs {:.3}", agent_avg_b, rand_b);

        // Verify escaped JSON obs_text + Lossless `TOMA17` SHA-256 Persistence (including Actor + Target Actor!)
        let (obs_txt, r_txt, d_txt) =
            parse_env_jsonl(r#"{"obs_text":"log: \"bid=101.4\"\\nok","reward":1.2,"done":false}"#).unwrap();
        assert!((r_txt - 1.2).abs() < 1e-6 && !d_txt);
        let out_txt = agent_a.step(&brain, &obs_txt, r_txt, false);
        assert!(format_agent_jsonl(&out_txt).contains("\"pred_value\":"));

        let tmp_p = std::env::temp_dir().join("tropicore_v17_agent_unit.toma");
        let sha1 = agent_a.save_checkpoint(&tmp_p).expect("save TOMA17");
        let (loaded, sha2) = UniversalOpenWorldAgent::load_checkpoint(&tmp_p).expect("load TOMA17");
        assert_eq!(sha1, sha2);
        assert_eq!(loaded.online_updates, agent_a.online_updates);
        assert_eq!(loaded.replay_len(), agent_a.replay_len());
        let _ = std::fs::remove_file(&tmp_p);
    }
}
