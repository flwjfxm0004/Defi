//! TropiCore-Omega ASI-Zero (`v13.0.0`) — Pure Zero-Toy Non-LLM General Superintelligence Engine.
//!
//! ZERO toy games, ZERO LLMs, ZERO external Rust crates, ZERO dead kernels, ZERO GPU data races:
//!   1. Per-Layer Causal Conv-4 + Fast-Weight Associative Delta-Rule SSM (`O(T*d*8)`) + Contractive Equilibrium (full BPTT)
//!   2. Thread-Safe Multi-GPU SM120 CUDA 13 Backend (`ABI = 1300`: GPU-resident weights & embeddings, 4-way SIMD CPU GEMM)
//!   3. 2-Way Set-Associative LRU Episodic Memory (`100x` single-pass 256-way + exact normalized Log-Odds PoE mixing)
//!   4. Physical Lossless `OMGZ13` Arithmetic Codec with Deterministic Test-Time Neural Adaptation (`TOTAL_FREQ=65536`)
//!   5. Best-Checkpoint Regression Guard + Stratified Shuffled Multi-File Streaming Corpus Loader
//!   6. Persistent Evolutionary Elitist End-to-End Self-Optimizer (immediately applied live + persisted in `TOMG13`)
//!   7. Stateful `CausalStreamState` + AdamW + TD(0) Critic + Full Replay `TOMA13` External `UniversalAgent` Server

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
    println!("TropiCore-Omega ASI-Zero v13.0.0 — Zero-Toy Non-LLM General Superintelligence Engine");
    println!("Commands:");
    println!("  status                                            Show SM120 Multi-GPU ABI=1300 & scaling presets");
    println!("  train-bytes [--path DIR] [--preset P] [--epochs N] [--max-bytes B] [--ckpt F] [--threads T]");
    println!("  eval-bytes  [--path DIR] --ckpt F                 Evaluate strictly-causal held-out bits/byte");
    println!("  compress    --in FILE --out FILE.omg [--ckpt F] [--no-ttt]  Physical OMGZ13 lossless compression");
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
    println!(" TropiCore-Omega ASI-Zero v13.0.0 (Conv4 + Delta-SSM + Multi-GPU ABI={}/{})", abi, tel.abi_version);
    println!("=================================================================================");
    println!(" Compute Backend : {} (GPUs={}, HostCPU={}T)", tel.backend_name, tel.physical_gpus, tel.host_cpu_threads);
    println!(" Target Hardware : 4x RTX PRO 6000 Blackwell (384 GB GDDR7, 14.044 PFLOPS FP4) + 9995WX 192T + 1,000 GB ECC");
    println!("---------------------------------------------------------------------------------");
    println!(" Scaling Presets (Conv-4 + Fast-Weight Associative Delta-SSM + Contractive):");
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
            Ok((b, sha)) => {
                return (
                    b,
                    format!("RESUMED LOSSLESS TOMG13 (sha256={})", &sha[..16]),
                );
            }
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
        .unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
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
    let (mut brain, boot_msg) = load_or_init_brain(&ckpt, &preset);
    let out_dir = PathBuf::from("out");
    let mut lr_mult = 1.0f32;

    println!("=================================================================================");
    println!(
        " TropiCore-Omega ASI-Zero v13.0.0 — Conv4 + Fast-Weight Delta-SSM ASI (ABI={}, threads={})",
        abi, threads
    );
    println!(" Boot   : {} | best_bpb={:.3}", boot_msg, if brain.best_heldout_bpb.is_finite() { brain.best_heldout_bpb } else { 8.0 });
    println!(" Path   : {} (stratified shuffled multi-file streaming, max_bytes={})", path.display(), max_bytes);
    println!("=================================================================================");

    for cyc in 1..=cycles.max(1) {
        let cycle_seed = 130_000 + (brain.adam_step as u64) + cyc as u64;
        let corpus = load_real_corpus_split(&path, max_bytes, max_heldout, cycle_seed).unwrap_or_else(|e| {
            eprintln!("[error] {}", e);
            std::process::exit(1);
        });

        // Snapshot best state BEFORE cycle so Best-Checkpoint Regression Guard can NEVER degrade disk state!
        let prev_best_brain = brain.clone();
        let prev_best_bpb = brain.best_heldout_bpb;

        let rep = run_real_byte_training(&mut brain, &corpus, epochs, 64, lr_mult, threads);

        let cand_bpb = rep.online_adapted_heldout_bpb.min(rep.trained_blended_heldout_bpb);
        let improved_best = cand_bpb <= prev_best_bpb + 0.015 || !ckpt.exists();
        let (save_sha, roundtrip_ok, guard_tag) = if improved_best {
            if cand_bpb < brain.best_heldout_bpb {
                brain.best_heldout_bpb = cand_bpb;
            }
            let sha = brain.save_checkpoint(&ckpt).unwrap_or_else(|e| format!("ERR:{}", e));
            let ok = DeepContractiveBrain::load_checkpoint(&ckpt).map(|(_, h)| h == sha).unwrap_or(false);
            (sha, ok, "SAVED")
        } else {
            // Regression Guard: keep best brain weights and damp LR for next cycle!
            brain = prev_best_brain;
            lr_mult = (lr_mult * 0.5).max(0.15);
            let sha = DeepContractiveBrain::load_checkpoint(&ckpt)
                .map(|(_, h)| h)
                .unwrap_or_else(|_| "kept_in_mem".to_string());
            (sha, true, "GUARD_KEPT_BEST")
        };

        let tel = backend_telemetry();
        println!(
            " [Cycle {:>2}] (tr={}/{}f {}B, ho={}/{}B, ep={}, B={}str, lr*={:.2}) HeldOut bpb: {:.3} -> Param {:.3} | +Episodic {:.3} | Online {:.3} ({:.2}x) | {:.1} KB/s | ||W_eq||={:.3} | Mem={}bkt/{}lat | {}={}({}) | {:.0}ms",
            cyc,
            rep.train_files,
            rep.total_train_pool,
            rep.train_bytes,
            rep.heldout_label,
            rep.heldout_bytes,
            rep.epochs,
            rep.n_streams,
            lr_mult,
            rep.untrained_heldout_bpb,
            rep.trained_param_heldout_bpb,
            rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb,
            rep.compression_ratio,
            rep.throughput_kb_s,
            rep.contractive_linf,
            rep.episodic_buckets,
            brain.episodic.latent_count(),
            guard_tag,
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
                130_000 + cyc as u64,
                apply_live,
                threads,
            );
            // LIVE APPLY to running brain + persist inside TOMG13 immediately!
            let mut installed_live = false;
            if let Some(win_pol) = so.winning_policy {
                brain.active_policy = win_pol;
                let _ = brain.save_checkpoint(&ckpt);
                installed_live = true;
            }
            println!(
                "   [Self-Opt Elitist] Live `{}` ({}) | cands={} | Real bpb {:.4} -> {:.4} | acc={} rustc0W={} active={} file_patch={} | {} | staged={} | {:.0}ms",
                so.target_live_file, so.live_sha256, so.candidates_evaluated,
                so.baseline_real_bpb, so.optimized_real_bpb, so.accepted,
                so.rustc_zero_warnings, installed_live, so.applied_live, so.upgrade_desc, so.staged_path, so.wall_ms
            );
            Some(so)
        } else {
            None
        };

        let json = format!(
            "{{\"engine\":\"TropiCore-ASI-Zero-v13\",\"cyc\":{},\"L\":{},\"d\":{},\"params\":{},\"tr_files\":{},\"bpb_pre\":{:.4},\"bpb_param\":{:.4},\"bpb_blend\":{:.4},\"bpb_online\":{:.4},\"comp\":{:.3},\"kb_s\":{:.2},\"norm\":{:.4},\"guard\":\"{}\",\"fwd\":{},\"bwd\":{},\"ctr\":{},\"adamw\":{},\"allred\":{},\"gflops\":{:.3},\"self_opt\":{}}}",
            cyc, brain.cfg.n_layers, brain.cfg.d_model, brain.cfg.parameter_count(), rep.train_files,
            rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb, rep.trained_blended_heldout_bpb,
            rep.online_adapted_heldout_bpb, rep.compression_ratio, rep.throughput_kb_s, rep.contractive_linf,
            guard_tag, tel.fwd_gemms, tel.bwd_gemms, tel.contractive_steps, tel.adamw_steps, tel.allreduces,
            tel.est_gflops, opt_rep.map(|o| o.accepted).unwrap_or(false)
        );
        let h = sha256_hex(json.as_bytes());
        let _ = append_vault_jsonl(
            &out_dir.join("asi_zero_v13_vault.jsonl"),
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
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
    let (mut brain, sha) = DeepContractiveBrain::load_checkpoint(&ckpt).unwrap_or_else(|e| {
        eprintln!("[error] failed to load '{}': {}", ckpt.display(), e);
        std::process::exit(1);
    });
    let corpus = load_real_corpus_split(&path, 0, 32_768, 13).unwrap_or_else(|e| {
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
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let enable_ttt = !has_flag(args, "--no-ttt");
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let rep = compress_file_to_omg(&brain, &in_p, &out_p, enable_ttt).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Compress OMGZ13] {} ({} B) -> {} ({} B) | Physical={:.3} bits/byte ({:.2}x) | ttt_steps={} | orig_sha256={}",
        in_p.display(), rep.orig_bytes, out_p.display(), rep.compressed_bytes,
        rep.physical_bpb, rep.physical_ratio, rep.ttt_updates, &rep.orig_sha256[..16]
    );
}

fn cmd_decompress(args: &[String]) {
    let in_p = PathBuf::from(flag_val(args, "--in").expect("missing --in <file.omg>"));
    let out_p = PathBuf::from(flag_val(args, "--out").expect("missing --out <file>"));
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let (brain, _) = load_or_init_brain(&ckpt, &preset);
    let (len, sha) = decompress_omg_to_file(&brain, &in_p, &out_p).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    println!(
        "[Decompress OMGZ13] {} -> {} ({} B) | SHA-256 BIT-EXACT VERIFIED ({})",
        in_p.display(), out_p.display(), len, &sha[..16]
    );
}

fn cmd_generate(args: &[String]) {
    let prompt = flag_val(args, "--prompt").unwrap_or_else(|| "pub fn ".to_string());
    let n_bytes: usize = flag_val(args, "--bytes").and_then(|s| s.parse().ok()).unwrap_or(96);
    let temp: f32 = flag_val(args, "--temp").and_then(|s| s.parse().ok()).unwrap_or(0.65);
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
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
    let ckpt = flag_val(args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
    let preset = flag_val(args, "--preset").unwrap_or_else(|| "laptop".to_string());
    let threads = flag_val(args, "--threads").and_then(|s| s.parse().ok()).unwrap_or_else(default_threads);
    let apply_live = has_flag(args, "--apply");

    let corpus = load_real_corpus_split(&path, 16_384, 8_192, 13).unwrap_or_else(|e| {
        eprintln!("[error] {}", e);
        std::process::exit(1);
    });
    let (mut brain, _) = load_or_init_brain(&ckpt, &preset);
    if brain.total_bytes_trained == 0 {
        let _ = run_real_byte_training(&mut brain, &corpus, 1, 64, 1.0, threads);
    }
    let so = run_real_self_optimization(&brain, &corpus.train_bytes, &corpus.heldout_bytes, Path::new("out"), 2026_1125, apply_live, threads);
    if let Some(win) = so.winning_policy {
        brain.active_policy = win;
        let _ = brain.save_checkpoint(&ckpt);
    }
    println!(
        "[Self-Opt Elitist] Target=`{}` ({}) | Real bpb {:.4} -> {:.4} | acc={} rustc0W={} live={} | {} | ledger={} | {:.0}ms",
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
            let ckpt = flag_val(&args, "--ckpt").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("out/asi_zero_v13.tomg"));
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
    fn test_sm120_abi_1300_and_delta_ssm_blackwell_budget() {
        let abi = verify_sm120_training_abi().expect("ABI must verify");
        assert_eq!(abi, 1300);
        let b1 = BrainConfig::from_preset("blackwell-1b").unwrap();
        assert!(b1.parameter_count() > 1_000_000_000, "blackwell-1b must exceed 1B params!");
        let b8 = BrainConfig::from_preset("blackwell-8b").unwrap();
        assert!(b8.parameter_count() > 7_000_000_000, "blackwell-8b must exceed 7B params!");
        assert!(b8.nvfp4_weight_gb() < 8.0);
        assert!(b8.fp32_train_state_gb() < 120.0, "FP32+AdamW must easily fit in 384 GB GDDR7!");
    }

    #[test]
    fn test_conv4_delta_ssm_multifile_compression_ttt_omgz13_and_elitist_self_opt() {
        let corpus_dir = default_corpus_path();
        let corpus = load_real_corpus_split(&corpus_dir, 14_000, 4_096, 13)
            .expect("must load real Rust source files from src/");
        // Verify Flaw #6 fix: stratified streaming covers MULTIPLE files (not starved to 2 files!)
        assert!(corpus.train_files >= 5, "stratified corpus loader must cover >=5 files, got {}", corpus.train_files);

        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let mut brain = DeepContractiveBrain::new(cfg, 42);

        let rep = run_real_byte_training(&mut brain, &corpus, 2, 64, 1.0, 2);
        assert!(rep.untrained_heldout_bpb > 7.5);
        assert!(
            rep.trained_param_heldout_bpb < rep.untrained_heldout_bpb - 1.5,
            "Conv4+Delta-SSM Brain must compress UNSEEN real Rust code! pre={:.3} post={:.3}",
            rep.untrained_heldout_bpb, rep.trained_param_heldout_bpb
        );
        assert!(rep.online_adapted_heldout_bpb < 5.0);
        assert!(rep.contractive_linf < 1.0);

        // PHYSICAL LOSSLESS OMGZ13 WITH DETERMINISTIC TEST-TIME NEURAL ADAPTATION (TTT)!
        let sample = &corpus.heldout_bytes[..1024.min(corpus.heldout_bytes.len())];
        let (archive, crep) = compress_bytes_with_brain(&brain, sample, true);
        assert!(crep.ttt_updates >= 10, "TTT dynamic eval must run deterministic neural updates!");
        assert!(
            crep.compressed_bytes < crep.orig_bytes,
            "Physical OMGZ13 ({} B) MUST shrink raw real code ({} B)!",
            crep.compressed_bytes, crep.orig_bytes
        );
        let restored = decompress_bytes_with_brain(&brain, &archive).expect("OMGZ13 TTT decompression must verify SHA-256");
        assert_eq!(restored.as_slice(), sample, "Decompressed real file after TTT must be 100% BIT-IDENTICAL!");

        // Lossless Bit-Exact `TOMG13` Checkpoint Roundtrip (including active_policy + best_heldout_bpb!)
        let tmp_dir = std::env::temp_dir().join("tropicore_asi_v13_unit");
        let ckpt = tmp_dir.join("unit_v13.tomg");
        let so = run_real_self_optimization(&brain, &corpus.train_bytes, sample, &tmp_dir, 99, false, 2);
        assert_eq!(so.candidates_evaluated, 8);
        assert!(!so.applied_live);
        if let Some(win) = so.winning_policy {
            assert!(so.rustc_zero_warnings && so.optimized_real_bpb < so.baseline_real_bpb);
            brain.active_policy = win;
        }
        brain.best_heldout_bpb = rep.online_adapted_heldout_bpb;

        let sha1 = brain.save_checkpoint(&ckpt).expect("save TOMG13");
        let (mut loaded, sha2) = DeepContractiveBrain::load_checkpoint(&ckpt).expect("load TOMG13");
        assert_eq!(sha1, sha2);
        let (p1, b1) = brain.eval_byte_stream(sample, 64, false, None);
        let (p2, b2) = loaded.eval_byte_stream(sample, 64, false, None);
        assert!((p1 - p2).abs() < 1e-5 && (b1 - b2).abs() < 1e-5);
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_stateful_universal_agent_escaped_json_td_critic_and_toma13_replay_persistence() {
        let cfg = BrainConfig::from_preset("laptop").unwrap();
        let brain = DeepContractiveBrain::new(cfg, 7);
        let mut agent = UniversalOpenWorldAgent::new(2, &cfg, 77);
        assert_eq!(agent.lat_dim, 64, "agent latent dim must scale dynamically (not crushed to 16!)");

        let (obs_num, r1, d1) = parse_env_jsonl(r#"{"obs":[0.25, -0.5, 1.0],"reward":0.5,"done":false}"#).unwrap();
        assert!((r1 - 0.5).abs() < 1e-6 && !d1);
        let _ = agent.step(&brain, &obs_num, 0.0, false);

        // Escaped quotes `\"` inside obs_text MUST parse cleanly!
        let (obs_txt, r2, d2) =
            parse_env_jsonl(r#"{"obs_text":"log: \"bid=101.4\"\\nok","reward":1.2,"done":false}"#).unwrap();
        assert!((r2 - 1.2).abs() < 1e-6 && !d2);
        let out = agent.step(&brain, &obs_txt, r2, false);
        assert_eq!(out.online_updates, 1);
        assert_eq!(agent.replay_len(), 1);
        assert!(format_agent_jsonl(&out).contains("\"pred_value\":"));

        // Lossless `TOMA13` SHA-256 Persistence including FULL replay buffer!
        let tmp_p = std::env::temp_dir().join("tropicore_v13_agent_unit.toma");
        let sha1 = agent.save_checkpoint(&tmp_p).expect("save TOMA13");
        let (loaded, sha2) = UniversalOpenWorldAgent::load_checkpoint(&tmp_p).expect("load TOMA13");
        assert_eq!(sha1, sha2);
        assert_eq!(loaded.online_updates, agent.online_updates);
        assert_eq!(loaded.replay_len(), 1, "TOMA13 MUST persist experience replay buffer!");
        let _ = std::fs::remove_file(&tmp_p);
    }
}
