//! TropiCore-Omega ASI-Zero (`v12.0.0`) — Pure Zero-Toy Non-LLM General Superintelligence Engine.
//!
//! ZERO toy games, ZERO toy mazes, ZERO toy LeetCode menus, ZERO LLMs, ZERO external Rust crates, ZERO dead kernels!
//!   1. Per-Layer Learned Selective Gated Causal SSM + Banach Contractive Equilibrium + Multi-Stream Data-Parallel BPTT
//!   2. GPU-Resident SM120 CUDA 13 Training Backend (`ABI = 1200`: persistent GDDR7 tensor handles, zero PCIe weight copies!)
//!   3. Real Physical Lossless 32-Bit Arithmetic Codec (`compress` / `decompress` `.omg` with SHA-256 bit-exact verification!)
//!   4. Autonomous Zero-LLM Byte Continuation (`generate`)
//!   5. Zero-Free-Rider Live-Training Self-Optimizer targeting live `src/asi_runtime_cfg.rs` (`--apply` atomic patch + rollback)
//!   6. Persistent Lossless `TOMA12` External `UniversalAgent` JSON-Line Server (`serve-agent`, `obs` + `obs_text`)

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
use std::path::{Path, PathBuf};

use crate::agent_protocol::serve_universal_agent_stdio;
use crate::arith_coder::{compress_file_to_omg, decompress_omg_to_file};
use crate::brain::{BrainConfig, DeepContractiveBrain};
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
    println!("TropiCore-Omega ASI-Zero v12.0.0 — Zero-Toy Non-LLM General Superintelligence Engine");
    println!("Commands:");
    println!("  status                                            Show SM120 GPU-Resident ABI=1200 & scaling presets");
    println!("  train-bytes [--path DIR] [--preset P] [--epochs N] [--max-bytes B] [--ckpt F] [--threads T]");
    println!("  eval-bytes  [--path DIR] --ckpt F                 Evaluate strictly-causal held-out bits/byte");
    println!("  compress    --in FILE --out FILE.omg [--ckpt F]   Physical lossless arithmetic compression (.omg)");
    println!("  decompress  --in FILE.omg --out FILE [--ckpt F]   Bit-exact SHA-256 verified decompression");
    println!("  generate    --prompt STR [--bytes N] [--temp T] [--ckpt F]");
    println!("  serve-agent [--act-dim A] [--ckpt F] [--agent-ckpt A.toma]");
    println!("  self-optimize [--path DIR] [--ckpt F] [--apply] [--threads T]");
    println!("  daemon      [--path DIR] [--cycles N] [--preset P] [--ckpt F] [--apply] [--threads T]");
    println!("Presets: laptop | mid | blackwell-1b | blackwell-8b");
}

fn cmd_status() {
    let abi = verify_sm120_training_abi().expect("SM120 training ABI verification failed");
    let tel = backend_telemetry();
    println!("=================================================================================");
    println!(" TropiCore-Omega ASI-Zero v12.0.0 (Per-Layer Selective SSM | GPU-Resident ABI={}/{})", abi, tel.abi_version);
    println!("=================================================================================");
    println!(" Compute Backend : {} (GPUs={}, HostCPU={}T)", tel.backend_name, tel.physical_gpus, tel.host_cpu_threads);
    println!(" Target Hardware : 4x RTX PRO 6000 Blackwell (384 GB GDDR7, 14.044 PFLOPS FP4) + 9995WX 192T + 1,000 GB ECC");
    println!("---------------------------------------------------------------------------------");
    println!(" Scaling Presets (Exact Dynamic Selective-SSM + Contractive Architecture):");
    for name in ["laptop", "mid", "blackwell-1b", "blackwell-8b"] {
        let cfg = BrainConfig::from_preset(name).unwrap();
        println!(
            "   - {:<13}: L={:>2}, d={:>4}, ff={:>5}, ticks={} | {:>10.2}M params | NVFP4={:.3} GB | FP32+AdamW={:.2} GB",
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
            Ok((b, sha)) => return (b, format!("RESUMED LOSSLESS TOMG12 (sha256={})", &sha[..16])),
            Err(e) => eprintln!(" [warn] checkpoint load failed ({}), initializing fresh...", e),
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
        .unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
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
        " TropiCore-Omega ASI-Zero v12.0.0 — Per-Layer Selective-SSM General ASI (ABI={}, threads={})",
        abi, threads
    );
    println!(" Boot   : {}", boot_msg);
    println!(
        " Corpus : {} (train={} files / {} bytes | UNSEEN heldout={} / {} bytes)",
        path.display(), corpus.train_files, corpus.train_bytes.len(), corpus.heldout_label, corpus.heldout_bytes.len()
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
            " [Cycle {:>2}] (tr={}f/{}B, ho={}/{}B, ep={}, B={}str) HeldOut bpb: {:.3} -> Param {:.3} | +Episodic {:.3} | Online {:.3} ({:.2}x) | {:.1} KB/s | ||W_eq||={:.3} | Mem={}bkt/{}lat | ckpt={}({}) | {:.0}ms",
            cyc,
            rep.train_files,
            rep.train_bytes,
            rep.heldout_label,
            rep.heldout_bytes,
            rep.epochs,
            rep.n_streams,
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
                &corpus.train_bytes,
                &corpus.heldout_bytes,
                &out_dir,
                120_000 + cyc as u64,
                apply_live,
                threads,
            );
            println!(
                "   [Self-Opt End2End] Live `{}` ({}) | cands={} | Real bpb {:.4} -> {:.4} | acc={} rustc0W={} live={} | {} | staged={} | {:.0}ms",
                so.target_live_file, so.live_sha256, so.candidates_evaluated,
                so.baseline_real_bpb, so.optimized_real_bpb, so.accepted,
                so.rustc_zero_warnings, so.applied_live, so.upgrade_desc, so.staged_path, so.wall_ms
            );
            Some(so)
        } else {
            None
        };

        let json = format!(
            "{{\"engine\":\"TropiCore-ASI-Zero-v12\",\"cyc\":{},\"L\":{},\"d\":{},\"params\":{},\"bpb_pre\":{:.4},\"bpb_param\":{:.4},\"bpb_blend\":{:.4},\"bpb_online\":{:.4},\"comp\":{:.3},\"kb_s\":{:.2},\"norm\":{:.4},\"fwd\":{},\"bwd\":{},\"ctr\":{},\"adamw\":{},\"allred\":{},\"gflops\":{:.3},\"ckpt_ok\":{},\"self_opt\":{}}}",
            cyc, brain.cfg.n_layers, brain.cfg.d_model, brain.cfg.parameter_count(),
            rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb, rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb, rep.compression_ratio, rep.throughput_kb_s, rep.contractive_linf,
            tel.fwd_gemms, tel.bwd_gemms, tel.contractive_steps, tel.adamw_steps, tel.allreduces,
            tel.est_gflops, roundtrip_ok, opt_rep.map(|o| o.accepted).unwrap_or(false)
        );
        let h = sha256_hex(json.as_bytes());
        let _ = append_vault_jsonl(
            &out_dir.join("asi_zero_v12_vault.jsonl"),
            &format!("{{\"sha256\":\"{}\",\"entry\":{}}}", h, json),
        );
    }

    let tel = backend_telemetry();
    println!("---------------------------------------------------------------------------------");
    println!(
        " Telemetry : Backend={} | Fwd={} Bwd={} Contractive={} AdamW={} AllReduce={} | Est={:.3} GFLOPs | TotalBytes={}",
        tel.backend_name, tel.fwd_gemms, tel.bwd_gemms, tel.contractive_steps,
        tel.adamw_steps, tel.allreduces, tel.est_gflops, brain.total_bytes_trained
    );
    println!("=================================================================================");
}

fn cmd_eval_bytes(args: &[String]) {
    let path = flag_val(args, "--path").map(PathBuf::from).unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
    let (mut brain, sha) = DeepContractiveBrain::load_checkpoint(&ckpt).unwrap_or_else(|e| {
        eprintln!("[error] failed to load '{}': {}", ckpt.display(), e);
        std::process::exit(1);
    });
    let corpus = load_real_corpus_split(&path, 0, 32_768).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (param_bpb, blend_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, 64, false, None);
    let (_, online_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, 64, true, None);
    println!(
        "Eval `{}` (ckpt={}...): Param={:.4} bpb | +Episodic={:.4} bpb | Online={:.4} bpb ({:.2}x comp)",
        corpus.heldout_label, &sha[..16], param_bpb, blend_bpb, online_bpb, 8.0 / online_bpb.max(0.05)
    );
}

fn cmd_compress(args: &[String]) {
    let in_p = PathBuf::from(flag_val(args, "--in").expect("missing --in <file>"));
    let out_p = PathBuf::from(flag_val(args, "--out").expect("missing --out <file.omg>"));
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let rep = compress_file_to_omg(&brain, &in_p, &out_p).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Compress] {} ({} B) -> {} ({} B) | Physical={:.3} bits/byte ({:.2}x) | orig_sha256={}",
        in_p.display(), rep.orig_bytes, out_p.display(), rep.compressed_bytes,
        rep.physical_bpb, rep.physical_ratio, &rep.orig_sha256[..16]
    );
}

fn cmd_decompress(args: &[String]) {
    let in_p = PathBuf::from(flag_val(args, "--in").expect("missing --in <file.omg>"));
    let out_p = PathBuf::from(flag_val(args, "--out").expect("missing --out <file>"));
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let (len, sha) = decompress_omg_to_file(&brain, &in_p, &out_p).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Decompress] {} -> {} ({} B) | SHA-256 BIT-EXACT VERIFIED ({})",
        in_p.display(), out_p.display(), len, &sha[..16]
    );
}

fn cmd_generate(args: &[String]) {
    let prompt = flag_val(args, "--prompt").unwrap_or_else(|| "pub fn ".to_string());
    let n_bytes: usize = flag_val(args, "--bytes").and_then(|s| s.parse().ok()).unwrap_or(96);
    let temp: f32 = flag_val(args, "--temp").and_then(|s| s.parse().ok()).unwrap_or(0.65);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, boot) = load_or_init_brain(&ckpt, &preset);
    let out = brain.generate_bytes(prompt.as_bytes(), n_bytes, temp, 2026_1125);
    println!("[Generate | {} | temp={:.2}]", boot, temp);
    println!("{}{}", prompt, String::from_utf8_lossy(&out));
}

fn cmd_self_optimize(args: &[String]) {
    let path = flag_val(args, "--path")
        .or_else(|| flag_val(args, "--corpus"))
        .map(PathBuf::from)
        .unwrap_or_else(default_corpus_path);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let threads = flag_val(args, "--threads").and_then(|s| s.parse().ok()).unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply");

    let corpus = load_real_corpus_split(&path, 16_384, 8_192).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (mut brain, _) = load_or_init_brain(&ckpt, &preset);
    if brain.total_bytes_trained == 0 {
        let _ = run_real_byte_training(&mut brain, &corpus, 1, 64, threads);
    }
    let so = run_real_self_optimization(&brain, &corpus.train_bytes, &corpus.heldout_bytes, Path::new("out"), 2026_1125, apply_live, threads);
    println!(
        "[Self-Opt End2End] Target=`{}` ({}) | Real bpb {:.4} -> {:.4} | accepted={} rustc0W={} live={} | {} | ledger={} | {:.0}ms",
        so.target_live_file, so.live_sha256, so.baseline_real_bpb, so.optimized_real_bpb,
        so.accepted, so.rustc_zero_warnings, so.applied_live, so.upgrade_desc, so.ledger_sha256, so.wall_ms
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
            let ckpt = flag_val(&args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v12.tomg"));
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
    use crate::agent_protocol::{format_agent_jsonl, parse_env_jsonl, UniversalOpenWorldAgent};
    use crate::arith_coder::{compress_bytes_with_brain, decompress_bytes_with_brain};

    #[test]
    fn test_sm120_abi_1200_and_selective_ssm_blackwell_budget() {
        let abi = verify_sm120_training_abi().expect("ABI must verify");
        assert_eq!(abi, 1200);
        let b8 = BrainConfig::from_preset("blackwell-8b").unwrap();
        assert!(b8.parameter_count() > 7_000_000_000, "blackwell-8b preset must exceed 7B params!");
        assert!(b8.nvfp4_weight_gb() < 8.0);
        assert!(b8.fp32_train_state_gb() < 120.0, "FP32+AdamW master state must easily fit in 384 GB GDDR7!");
    }

    #[test]
    fn test_selective_ssm_real_code_compression_physical_omg_codec_and_live_self_opt() {
        let corpus_dir = default_corpus_path();
        let corpus = load_real_corpus_split(&corpus_dir, 14_000, 4_096)
            .expect("must load real Rust source files from src/");
        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let mut brain = DeepContractiveBrain::new(cfg, 42);

        let rep = run_real_byte_training(&mut brain, &corpus, 2, 64, 2);
        assert!(rep.untrained_heldout_bpb > 7.5);
        assert!(
            rep.trained_param_heldout_bpb < rep.untrained_heldout_bpb - 1.5,
            "Selective-SSM Deep Brain must compress UNSEEN real Rust code! pre={:.3} post={:.3}",
            rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb
        );
        assert!(rep.online_adapted_heldout_bpb < 5.0);
        assert!(rep.contractive_linf < 1.0);

        // PHYSICAL LOSSLESS ARITHMETIC COMPRESSION -> DECOMPRESSION BIT-EXACT SHA-256 PROOF!
        let sample = &corpus.heldout_bytes[..1024.min(corpus.heldout_bytes.len())];
        let (archive, crep) = compress_bytes_with_brain(&brain, sample);
        assert!(
            crep.compressed_bytes < crep.orig_bytes,
            "Physical .omg archive ({} B) MUST be strictly smaller than raw real code ({} B)!",
            crep.compressed_bytes, crep.orig_bytes
        );
        let restored = decompress_bytes_with_brain(&brain, &archive).expect("decompression must verify SHA-256");
        assert_eq!(restored.as_slice(), sample, "Decompressed real source file must be BIT-IDENTICAL!");

        // Lossless Bit-Exact `TOMG12` Checkpoint Roundtrip (Selective-SSM + Contractive + AdamW + Episodic)
        let tmp_dir = std::env::temp_dir().join("tropicore_asi_v12_unit");
        let ckpt = tmp_dir.join("unit_v12.tomg");
        let sha1 = brain.save_checkpoint(&ckpt).expect("save TOMG12");
        let (mut loaded, sha2) = DeepContractiveBrain::load_checkpoint(&ckpt).expect("load TOMG12");
        assert_eq!(sha1, sha2);
        let (p1, b1) = brain.eval_byte_stream(sample, 64, false, None);
        let (p2, b2) = loaded.eval_byte_stream(sample, 64, false, None);
        assert!((p1 - p2).abs() < 1e-5 && (b1 - b2).abs() < 1e-5);

        // Zero-Free-Rider Live End-to-End Self-Optimizer
        let so = run_real_self_optimization(&brain, &corpus.train_bytes, sample, &tmp_dir, 99, false, 2);
        assert_eq!(so.candidates_evaluated, 6);
        assert!(!so.applied_live);
        if so.accepted {
            assert!(so.rustc_zero_warnings && so.optimized_real_bpb < so.baseline_real_bpb);
        }
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_universal_agent_numeric_and_raw_text_obs_with_lossless_toma12_persistence() {
        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let brain = DeepContractiveBrain::new(cfg, 7);
        let mut agent = UniversalOpenWorldAgent::new(2, 77);

        let (obs_num, r1, d1) = parse_env_jsonl(r#"{"obs":[0.25, -0.5, 1.0],"reward":0.5,"done":false}"#).unwrap();
        assert!((r1 - 0.5).abs() < 1e-6 && !d1);
        let _ = agent.step(&brain, &obs_num, 0.0, false);

        let (obs_txt, r2, d2) = parse_env_jsonl(r#"{"obs_text":"market_tick bid=101.4 ask=101.5","reward":1.2,"done":false}"#).unwrap();
        assert!((r2 - 1.2).abs() < 1e-6 && !d2);
        let out = agent.step(&brain, &obs_txt, r2, false);
        assert_eq!(out.online_updates, 1);
        assert!(format_agent_jsonl(&out).contains("\"updates\":1"));

        // Lossless `TOMA12` SHA-256 Persistence Roundtrip
        let tmp_p = std::env::temp_dir().join("tropicore_v12_agent_unit.toma");
        let sha1 = agent.save_checkpoint(&tmp_p).expect("save TOMA12");
        let (loaded, sha2) = UniversalOpenWorldAgent::load_checkpoint(&tmp_p).expect("load TOMA12");
        assert_eq!(sha1, sha2);
        assert_eq!(loaded.online_updates, agent.online_updates);
        assert_eq!(loaded.act_dim, agent.act_dim);
        let _ = std::fs::remove_file(&tmp_p);
    }
}
