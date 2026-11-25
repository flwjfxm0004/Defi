//! Real-World Universal Byte-Stream Solomonoff Learner & Multi-Stream Trainer (`TropiCore-Omega v12.0`).
//!
//! ZERO toy datasets, ZERO pretrained tokenizers, ZERO LLMs:
//!   - Reads ANY real file or directory on disk (source code, logs, financial ticks, scientific data, binaries)
//!   - Splits cleanly into Train vs Unseen Held-Out real bytes/files
//!   - Runs lock-free Multi-Stream Data-Parallel Batch Training (`B` independent sequences across `threads`)
//!     through `DeepContractiveBrain::train_batch_streams` (calling `allreduce_grad_shards`!)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::brain::{CausalStreamState, DeepContractiveBrain};

const SKIP_DIRS: &[&str] = &[
    ".git", "target", "out", "node_modules", ".venv", "__pycache__", "_cache", ".tmp",
];

pub fn collect_real_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|x| x.path())).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                let nm = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !SKIP_DIRS.contains(&nm) {
                    walk(&p, out);
                }
            } else if p.is_file() {
                if let Ok(meta) = p.metadata() {
                    if meta.len() >= 128 && meta.len() <= 32 * 1024 * 1024 {
                        out.push(p);
                    }
                }
            }
        }
    }
    walk(root, &mut out);
    out
}

#[derive(Clone, Debug)]
pub struct CorpusSplit {
    pub train_bytes: Vec<u8>,
    pub heldout_bytes: Vec<u8>,
    pub train_files: usize,
    pub heldout_label: String,
}

pub fn load_real_corpus_split(path: &Path, max_train_bytes: usize, max_heldout_bytes: usize) -> Result<CorpusSplit, String> {
    let files = collect_real_files(path);
    if files.is_empty() {
        return Err(format!("No readable real files found under '{}'", path.display()));
    }

    if files.len() == 1 {
        let raw = fs::read(&files[0]).map_err(|e| format!("read failed: {}", e))?;
        if raw.len() < 256 {
            return Err("File too small (<256 bytes) for train/heldout split".into());
        }
        let split = (raw.len() * 85) / 100;
        let mut tr = raw[..split].to_vec();
        let mut ho = raw[split..].to_vec();
        if max_train_bytes > 0 && tr.len() > max_train_bytes { tr.truncate(max_train_bytes); }
        if max_heldout_bytes > 0 && ho.len() > max_heldout_bytes { ho.truncate(max_heldout_bytes); }
        return Ok(CorpusSplit {
            train_bytes: tr,
            heldout_bytes: ho,
            train_files: 1,
            heldout_label: format!("tail-15% of {}", files[0].display()),
        });
    }

    let held_path = files.last().unwrap().clone();
    let mut train_bytes = Vec::new();
    let mut train_files = 0usize;
    for f in &files[..files.len() - 1] {
        if let Ok(b) = fs::read(f) {
            train_bytes.extend_from_slice(&b);
            train_bytes.push(b'\n');
            train_files += 1;
            if max_train_bytes > 0 && train_bytes.len() >= max_train_bytes {
                train_bytes.truncate(max_train_bytes);
                break;
            }
        }
    }
    let mut heldout_bytes = fs::read(&held_path).map_err(|e| format!("heldout read failed: {}", e))?;
    if max_heldout_bytes > 0 && heldout_bytes.len() > max_heldout_bytes {
        heldout_bytes.truncate(max_heldout_bytes);
    }
    Ok(CorpusSplit {
        train_bytes,
        heldout_bytes,
        train_files,
        heldout_label: held_path.display().to_string(),
    })
}

#[derive(Clone, Debug)]
pub struct CompressionRunReport {
    pub train_files: usize,
    pub train_bytes: usize,
    pub heldout_bytes: usize,
    pub heldout_label: String,
    pub epochs: usize,
    pub n_streams: usize,
    pub untrained_heldout_bpb: f64,
    pub trained_param_heldout_bpb: f64,
    pub trained_blended_heldout_bpb: f64,
    pub online_adapted_heldout_bpb: f64,
    pub compression_ratio: f64,
    pub throughput_kb_s: f64,
    pub contractive_linf: f32,
    pub episodic_buckets: usize,
    pub wall_ms: f64,
}

/// Multi-Stream Data-Parallel Training on REAL files on disk!
/// Splits `corpus.train_bytes` into `B` independent streams trained concurrently across `threads` workers,
/// then measures strictly-causal out-of-sample `bits/byte` on completely unseen real held-out code/data!
pub fn run_real_byte_training(
    brain: &mut DeepContractiveBrain,
    corpus: &CorpusSplit,
    epochs: usize,
    chunk_len: usize,
    threads: usize,
) -> CompressionRunReport {
    let t0 = Instant::now();
    let cl = chunk_len.max(16);

    // 1. Measure baseline `bits/byte` on UNSEEN held-out file BEFORE this training phase
    let (pre_param_bpb, _) = brain.eval_byte_stream(&corpus.heldout_bytes, cl, false, None);

    // 2. Multi-Stream Data-Parallel Batch Training (`B` parallel streams across `threads`)!
    let n_streams = (threads.max(1) * 2).min(corpus.train_bytes.len() / (cl * 2)).clamp(1, 16);
    let stride = corpus.train_bytes.len() / n_streams;
    let mut total_processed = 0usize;

    for ep in 0..epochs.max(1) {
        let mut streams: Vec<CausalStreamState> = (0..n_streams)
            .map(|_| CausalStreamState::new(&brain.cfg))
            .collect();
        let decay = (0.88f32).powi(ep as i32);
        let lr = brain.cfg.lr * decay;
        let update_ep = ep == epochs.saturating_sub(1);

        let mut off = 0usize;
        while off + 1 < stride {
            let step_len = cl.min(stride - 1 - off);
            let mut batch_io: Vec<(&[u8], &[u8])> = Vec::with_capacity(n_streams);
            for si in 0..n_streams {
                let base = si * stride + off;
                if base + step_len < corpus.train_bytes.len() {
                    batch_io.push((
                        &corpus.train_bytes[base..base + step_len],
                        &corpus.train_bytes[base + 1..base + 1 + step_len],
                    ));
                    total_processed += step_len;
                }
            }
            if !batch_io.is_empty() {
                brain.train_batch_streams(&batch_io, &mut streams, lr, None, update_ep, threads);
            }
            off += step_len;
        }
    }

    // 3. Strictly-Causal Evaluation on COMPLETELY UNSEEN real held-out file
    let (post_param_bpb, post_blend_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, cl, false, None);
    let mut online_brain = brain.clone();
    let (_, online_adapted_bpb) = online_brain.eval_byte_stream(&corpus.heldout_bytes, cl, true, None);

    let wall_s = t0.elapsed().as_secs_f64().max(1e-6);
    let throughput_kb_s = ((total_processed + corpus.heldout_bytes.len()) as f64 / 1024.0) / wall_s;
    let best_bpb = post_blend_bpb.min(online_adapted_bpb).max(0.05);

    CompressionRunReport {
        train_files: corpus.train_files,
        train_bytes: corpus.train_bytes.len(),
        heldout_bytes: corpus.heldout_bytes.len(),
        heldout_label: corpus.heldout_label.clone(),
        epochs,
        n_streams,
        untrained_heldout_bpb: pre_param_bpb,
        trained_param_heldout_bpb: post_param_bpb,
        trained_blended_heldout_bpb: post_blend_bpb,
        online_adapted_heldout_bpb: online_adapted_bpb,
        compression_ratio: 8.0 / best_bpb,
        throughput_kb_s,
        contractive_linf: brain.max_contractive_linf_norm(),
        episodic_buckets: brain.episodic.bucket_count(),
        wall_ms: wall_s * 1000.0,
    }
}
