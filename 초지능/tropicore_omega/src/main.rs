//! TropiCore-Omega ASI-Zero (`v11.0.0`) — Pure Zero-Toy Non-LLM General Superintelligence Engine.
//!
//! ZERO toy games, ZERO toy mazes, ZERO toy LeetCode menus, ZERO LLMs, ZERO external Rust crates!
//!
//! Built on 3 Pure Real-World General Intelligence Substrates + Real SM120 GPU Training (`ABI = 1100`):
//!   1. `DeepContractiveBrain` (`src/brain.rs` + `src/gpu_backend.rs` + `cuda/tropicore_omega_sm120.cu`):
//!      Scalable L-layer Contractive NVFP4 Equilibrium + Causal Tropical-Exponential SSM (`laptop` -> `blackwell-8b`)
//!      with REAL Forward + Backward + Fused AdamW + Contractive Projection + NVFP4 STE!
//!   2. `Universal Byte-Stream Solomonoff Learner` (`src/compressor.rs` + `src/episodic.rs`):
//!      Trains directly on ANY real files/directories on disk and measures exact out-of-sample `bits/byte`!
//!   3. `UniversalAgent` External Open-World Protocol (`src/agent_protocol.rs`) +
//!      `Real-Corpus-Gated Self-Optimizer` (`src/self_optimizer.rs` targeting live `src/asi_runtime_cfg.rs`)!

mod agent_protocol;
mod asi_runtime_cfg;
mod brain;
mod compressor;
mod episodic;
mod gpu_backend;
mod nvfp4;
mod self_optimizer;
mod sha256;

use std::env;
use std::path::{Path, PathBuf};

use crate::agent_protocol::serve_universal_agent_stdio;
use crate::brain::{BrainConfig, DeepContractiveBrain};
use crate::compressor::{load_real_corpus_split, run_real_byte_training};
use crate::gpu_backend::{backend_telemetry, verify_sm120_training_abi};
use crate::self_optimizer::run_real_self_optimization;
use crate::sha256::{append_vault_jsonl, sha256_hex};

fn default_threads() -> usize {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    // Laptop-kind zero-lag default (2 threads on <=16T laptops; unleash all 192T on Threadripper PRO!)
    if cpus <= 16 { 2 } else { cpus }
}

fn default_corpus_path() -> PathBuf {
    let local = PathBuf::from("src");
    if local.is_dir() {
        return local;
    }
    PathBuf::from(r"C:\Defi\초지능\tropicore_omega\src")
}

fn flag_val(args: &[String], key: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == key {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn has_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn print_help() {
    println!("TropiCore-Omega ASI-Zero v11.0.0 — Zero-Toy Non-LLM General Superintelligence Engine");
    println!("Commands:");
    println!("  status                                            Show hardware, SM120 ABI=1100 & scaling presets");
    println!("  train-bytes [--path DIR] [--preset P] [--epochs N] [--max-bytes B] [--ckpt F] [--threads T]");
    println!("                                                    Train directly on real files/code & measure held-out bits/byte");
    println!("  eval-bytes  [--path DIR] --ckpt F [--threads T]   Evaluate causal held-out bits/byte from checkpoint");
    println!("  serve-agent [--act-dim A] [--ckpt F] [--threads T] Run external UniversalAgent JSON-Line stdio server");
    println!("  self-optimize [--path DIR] [--ckpt F] [--apply] [--threads T]");
    println!("                                                    Real-corpus-gated self-optimization of live `src/asi_runtime_cfg.rs`");
    println!("  daemon      [--path DIR] [--cycles N] [--preset P] [--ckpt F] [--apply] [--threads T]");
    println!("                                                    Autonomous lossless General ASI loop (Day-1 4x Blackwell)");
    println!("Presets: laptop (0.28M) | mid (14.9M) | blackwell-1b (0.71B) | blackwell-8b (4.70B)");
}

fn cmd_status() {
    let abi = verify_sm120_training_abi().expect("SM120 training ABI verification failed");
    let tel = backend_telemetry();
    println!("=================================================================================");
    println!(" TropiCore-Omega ASI-Zero v11.0.0 (Zero-Toy | Zero-LLM | Zero-Crates | ABI={}/{})", abi, tel.abi_version);
    println!("=================================================================================");
    println!(" Compute Backend : {} (GPUs={}, HostCPU={}T)", tel.backend_name, tel.physical_gpus, tel.host_cpu_threads);
    println!(" Target Hardware : 4x RTX PRO 6000 Blackwell (384 GB GDDR7, 14.044 PFLOPS FP4) + 9995WX 192T + 1,000 GB ECC");
    println!("---------------------------------------------------------------------------------");
    println!(" Scaling Presets (Exact Dynamic Architecture — Same Code Laptop -> 4x Blackwell):");
    for name in ["laptop", "mid", "blackwell-1b", "blackwell-8b"] {
        let cfg = BrainConfig::from_preset(name).unwrap();
        println!(
            "   - {:<13}: L={:>2}, d={:>4}, ff={:>4}, ticks={} | {:>10.2}M params | NVFP4={:.3} GB | FP32+AdamW={:.2} GB",
            name,
            cfg.n_layers,
            cfg.d_model,
            cfg.d_ff,
            cfg.n_ticks,
            cfg.parameter_count() as f64 / 1e6,
            cfg.nvfp4_weight_gb(),
            cfg.fp32_train_state_gb()
        );
    }
    println!("=================================================================================");
}

fn load_or_init_brain(ckpt: &Path, preset: &str) -> (DeepContractiveBrain, String) {
    if ckpt.exists() {
        match DeepContractiveBrain::load_checkpoint(ckpt) {
            Ok((b, sha)) => {
                return (
                    b,
                    format!("RESUMED LOSSLESS TOMG11 (sha256={})", &sha[..16]),
                );
            }
            Err(e) => {
                eprintln!(" [warn] checkpoint load failed ({}), initializing fresh...", e);
            }
        }
    }
    let cfg = BrainConfig::from_preset(preset).unwrap_or_else(|e| {
        eprintln!(" [error] {}", e);
        std::process::exit(1);
    });
    (
        DeepContractiveBrain::new(cfg, 2026_1125),
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
        .unwrap_or_else(|| PathBuf::from("out/asi_zero.tomg"));
    let epochs: usize = flag_val(args, "--epochs").and_then(|s| s.parse().ok()).unwrap_or(2);
    let max_bytes: usize = flag_val(args, "--max-bytes")
        .and_then(|s| s.parse().ok())
        .unwrap_or(if preset == "laptop" { 24_576 } else { 0 });
    let max_heldout: usize = if max_bytes > 0 { (max_bytes / 3).max(4096) } else { 0 };
    let threads: usize = flag_val(args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply") || has_flag(args, "--apply-rsi");

    let abi = verify_sm120_training_abi().expect("SM120 ABI mismatch");
    let corpus = load_real_corpus_split(&path, max_bytes, max_heldout).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (mut brain, boot_msg) = load_or_init_brain(&ckpt, &preset);
    let out_dir = PathBuf::from("out");

    println!("=================================================================================");
    println!(
        " TropiCore-Omega ASI-Zero v11.0.0 — Real-World General ASI (ABI={}, threads={})",
        abi, threads
    );
    println!(" Boot   : {}", boot_msg);
    println!(
        " Corpus : {} (train={} files / {} bytes | UNSEEN heldout={} / {} bytes)",
        path.display(),
        corpus.train_files,
        corpus.train_bytes.len(),
        corpus.heldout_label,
        corpus.heldout_bytes.len()
    );
    println!("=================================================================================");

    for cyc in 1..=cycles.max(1) {
        let rep = run_real_byte_training(&mut brain, &corpus, epochs, 64, threads);
        let save_sha = brain.save_checkpoint(&ckpt).unwrap_or_else(|e| format!("ERR:{}", e));
        let roundtrip_ok = DeepContractiveBrain::load_checkpoint(&ckpt)
            .map(|(_, h)| h == save_sha)
            .unwrap_or(false);
        let tel = backend_telemetry();

        println!(
            " [Cycle {:>2}] (tr={}f/{}B, ho={}/{}B, ep={}) HeldOut bits/byte: {:.3} -> Param {:.3} | +Episodic {:.3} | Online {:.3} ({:.2}x comp) | {:.1} KB/s | ||W_eq||={:.3} | Mem={}bkt/{}lat | ckpt={}({}) | {:.0}ms",
            cyc,
            rep.train_files,
            rep.train_bytes,
            rep.heldout_label,
            rep.heldout_bytes,
            rep.epochs,
            rep.untrained_heldout_bpb,
            rep.trained_param_heldout_bpb,
            rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb,
            rep.compression_ratio,
            rep.throughput_kb_s,
            rep.contractive_linf,
            rep.episodic_buckets,
            brain.episodic.latent_count(),
            &save_sha[..16.min(save_sha.len())],
            roundtrip_ok,
            rep.wall_ms
        );

        let opt_rep = if run_self_opt {
            let so = run_real_self_optimization(
                &brain,
                &corpus.heldout_bytes,
                &out_dir,
                110_000 + cyc as u64,
                apply_live,
                threads,
            );
            println!(
                "   [Self-Opt] Live `{}` ({}) | cands={} | Real HeldOut bpb {:.4} -> {:.4} | acc={} rustc0W={} live={} | {} | staged={} | {:.0}ms",
                so.target_live_file,
                so.live_sha256,
                so.candidates_evaluated,
                so.baseline_real_bpb,
                so.optimized_real_bpb,
                so.accepted,
                so.rustc_zero_warnings,
                so.applied_live,
                so.upgrade_desc,
                so.staged_path,
                so.wall_ms
            );
            Some(so)
        } else {
            None
        };

        let json = format!(
            "{{\"engine\":\"TropiCore-ASI-Zero-v11\",\"cyc\":{},\"L\":{},\"d\":{},\"params\":{},\"bpb_pre\":{:.4},\"bpb_param\":{:.4},\"bpb_blend\":{:.4},\"bpb_online\":{:.4},\"comp\":{:.3},\"kb_s\":{:.2},\"norm\":{:.4},\"fwd_gemms\":{},\"bwd_gemms\":{},\"adamw\":{},\"gflops\":{:.3},\"ckpt_ok\":{},\"self_opt_acc\":{}}}",
            cyc,
            brain.cfg.n_layers,
            brain.cfg.d_model,
            brain.cfg.parameter_count(),
            rep.untrained_heldout_bpb,
            rep.trained_param_heldout_bpb,
            rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb,
            rep.compression_ratio,
            rep.throughput_kb_s,
            rep.contractive_linf,
            tel.fwd_gemms,
            tel.bwd_gemms,
            tel.adamw_steps,
            tel.est_gflops,
            roundtrip_ok,
            opt_rep.map(|o| o.accepted).unwrap_or(false)
        );
        let h = sha256_hex(json.as_bytes());
        let _ = append_vault_jsonl(
            &out_dir.join("asi_zero_vault.jsonl"),
            &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", h, json),
        );
    }

    let tel = backend_telemetry();
    println!("---------------------------------------------------------------------------------");
    println!(
        " Telemetry : Backend={} | FwdGEMMs={} | BwdGEMMs={} | AdamW={} | Est={:.3} GFLOPs | TotalBytesTrained={}",
        tel.backend_name, tel.fwd_gemms, tel.bwd_gemms, tel.adamw_steps, tel.est_gflops, brain.total_bytes_trained
    );
    println!("=================================================================================");
}

fn cmd_eval_bytes(args: &[String]) {
    let path = flag_val(args, "--path")
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("out/asi_zero.tomg"));
    let threads: usize = flag_val(args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(default_threads);

    let (mut brain, sha) = DeepContractiveBrain::load_checkpoint(&ckpt).unwrap_or_else(|e| {
        eprintln!("[error] failed to load checkpoint '{}': {}", ckpt.display(), e);
        std::process::exit(1);
    });
    let corpus = load_real_corpus_split(&path, 0, 32_768).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (param_bpb, blend_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, 64, false, threads);
    let (_, online_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, 64, true, threads);
    println!(
        "Eval `{}` (ckpt={}...): Param={:.4} bpb | +Episodic={:.4} bpb | Online={:.4} bpb ({:.2}x comp)",
        corpus.heldout_label,
        &sha[..16],
        param_bpb,
        blend_bpb,
        online_bpb,
        8.0 / online_bpb.max(0.05)
    );
}

fn cmd_self_optimize(args: &[String]) {
    let path = flag_val(args, "--path")
        .or_else(|| flag_val(args, "--corpus"))
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("out/asi_zero.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let threads: usize = flag_val(args, "--threads")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply");

    let corpus = load_real_corpus_split(&path, 16_384, 8_192).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (mut brain, _) = load_or_init_brain(&ckpt, &preset);
    if brain.total_bytes_trained == 0 {
        let _ = run_real_byte_training(&mut brain, &corpus, 1, 64, threads);
    }
    let so = run_real_self_optimization(&brain, &corpus.heldout_bytes, Path::new("out"), 2026_1125, apply_live, threads);
    println!(
        "[Self-Opt] Target=`{}` ({}) | Real HeldOut bpb {:.4} -> {:.4} | accepted={} rustc0W={} applied_live={} | {} | ledger={} | {:.0}ms",
        so.target_live_file,
        so.live_sha256,
        so.baseline_real_bpb,
        so.optimized_real_bpb,
        so.accepted,
        so.rustc_zero_warnings,
        so.applied_live,
        so.upgrade_desc,
        so.ledger_sha256,
        so.wall_ms
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
        "self-optimize" => cmd_self_optimize(&args),
        "daemon" => {
            let cycles = flag_val(&args, "--cycles").and_then(|s| s.parse().ok()).unwrap_or(3);
            cmd_train_or_daemon(&args, cycles, true);
        }
        "serve-agent" => {
            let act_dim = flag_val(&args, "--act-dim").and_then(|s| s.parse().ok()).unwrap_or(2);
            let preset = flag_val(&args, "--preset").unwrap_or_else(|| "laptop".to_string());
            let ckpt = flag_val(&args, "--ckpt")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("out/asi_zero.tomg"));
            let threads = flag_val(&args, "--threads")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(default_threads);
            let (brain, _) = load_or_init_brain(&ckpt, &preset);
            serve_universal_agent_stdio(&brain, act_dim, threads);
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
    use crate::agent_protocol::{format_agent_jsonl, parse_env_jsonl, UniversalOpenWorldAgent};

    #[test]
    fn test_sm120_training_abi_and_scaling_presets_match_blackwell_budget() {
        let abi = verify_sm120_training_abi().expect("ABI must verify");
        assert_eq!(abi, 1100);
        let b8 = BrainConfig::from_preset("blackwell-8b").unwrap();
        assert!(b8.parameter_count() > 4_000_000_000, "blackwell-8b preset must exceed 4B params!");
        assert!(b8.nvfp4_weight_gb() < 10.0, "NVFP4 weights must easily fit inside 384 GB GDDR7!");
        assert!(b8.fp32_train_state_gb() < 300.0, "FP32+AdamW master state must fit in 384 GB GDDR7 / 1,000 GB ECC!");
    }

    #[test]
    fn test_real_source_code_compression_checkpoint_roundtrip_and_self_optimizer() {
        let corpus_dir = default_corpus_path();
        let corpus = load_real_corpus_split(&corpus_dir, 14_000, 4_096)
            .expect("must load real Rust source files from src/");
        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let mut brain = DeepContractiveBrain::new(cfg, 42);

        let rep = run_real_byte_training(&mut brain, &corpus, 2, 64, 2);
        assert!(
            rep.untrained_heldout_bpb > 7.5,
            "Untrained random brain must start near ~8.0 bits/byte! got {:.3}",
            rep.untrained_heldout_bpb
        );
        assert!(
            rep.trained_param_heldout_bpb < rep.untrained_heldout_bpb - 1.5,
            "Pure parametric Deep Contractive Brain must genuinely compress UNSEEN real Rust code! pre={:.3} post={:.3}",
            rep.untrained_heldout_bpb,
            rep.trained_param_heldout_bpb
        );
        assert!(
            rep.online_adapted_heldout_bpb < 5.0,
            "Brain + Tropical Episodic Memory must compress unseen real Rust code below 5.0 bpb! got {:.3}",
            rep.online_adapted_heldout_bpb
        );
        assert!(
            rep.contractive_linf < 1.0,
            "Banach contractive core must strictly hold ||W_eq||_inf < 1.0! got {:.4}",
            rep.contractive_linf
        );

        // Lossless Bit-Exact `TOMG11` SHA-256 Checkpoint Roundtrip (weights + AdamW + Episodic!)
        let tmp_dir = std::env::temp_dir().join("tropicore_asi_zero_unit");
        let ckpt = tmp_dir.join("unit.tomg");
        let sha1 = brain.save_checkpoint(&ckpt).expect("save ckpt");
        let (mut loaded, sha2) = DeepContractiveBrain::load_checkpoint(&ckpt).expect("load ckpt");
        assert_eq!(sha1, sha2, "TOMG11 SHA-256 roundtrip must match bit-exact!");
        assert_eq!(loaded.adam_step, brain.adam_step);
        assert_eq!(loaded.total_bytes_trained, brain.total_bytes_trained);
        assert_eq!(loaded.episodic.bucket_count(), brain.episodic.bucket_count());
        let (p1, b1) = brain.eval_byte_stream(&corpus.heldout_bytes[..1024.min(corpus.heldout_bytes.len())], 64, false, 2);
        let (p2, b2) = loaded.eval_byte_stream(&corpus.heldout_bytes[..1024.min(corpus.heldout_bytes.len())], 64, false, 2);
        assert!((p1 - p2).abs() < 1e-5 && (b1 - b2).abs() < 1e-5, "Loaded checkpoint must reproduce exact held-out bits/byte!");

        // Real-Corpus-Gated Self-Optimizer (no proxy 6-tuples!)
        let so = run_real_self_optimization(&brain, &corpus.heldout_bytes, &tmp_dir, 99, false, 2);
        assert_eq!(so.candidates_evaluated, 12);
        assert!(!so.applied_live, "Unit test must never mutate live repo!");
        if so.accepted {
            assert!(so.rustc_zero_warnings && so.optimized_real_bpb < so.baseline_real_bpb);
        }
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_universal_external_agent_jsonl_protocol_and_online_prediction() {
        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let brain = DeepContractiveBrain::new(cfg, 7);
        let mut agent = UniversalOpenWorldAgent::new(2, 77);

        // Verify JSONL parser + formatter cleanly handle arbitrary external sensor streams
        let (obs, rew, done) = parse_env_jsonl(r#"{"obs":[0.25, -0.5, 1.0, -0.1],"reward":0.5,"done":false}"#).unwrap();
        assert_eq!(obs.len(), 4);
        assert!((rew - 0.5).abs() < 1e-6);
        assert!(!done);

        // Stream 12 external transitions and verify finite bounded actions + online latent updates
        let mut last_out = agent.step(&brain, &obs, 0.0, false, 2);
        for t in 1..=12 {
            let phase = (t as f32) * 0.35;
            let raw = [phase.sin(), phase.cos(), last_out.action[0], last_out.action[1]];
            let r = 1.0 - (raw[0] - raw[2]).abs();
            last_out = agent.step(&brain, &raw, r, false, 2);
            assert_eq!(last_out.action.len(), 2);
            assert!(last_out.action.iter().all(|a| a.is_finite() && *a >= -1.0 && *a <= 1.0));
        }
        assert_eq!(last_out.online_updates, 12);
        let json_line = format_agent_jsonl(&last_out);
        assert!(json_line.contains("\"action\":") && json_line.contains("\"updates\":12"));
    }
}
