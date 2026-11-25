//! Stratified Multi-File & Single-Large-File `O(budget)` RAM Streaming Byte-Stream Solomonoff Learner (`TropiCore-Omega v16.0`).
//!
//! Works at fixed `O(max_train_bytes + max_heldout_bytes)` RAM on 100 KB repos OR a single 100 GB / 16 TB Gen5 NVMe file!
//!   - Single file (`files.len() == 1`): uses `File::open + Seek + Read` on both the 85% training prefix AND 15% held-out tail
//!     (ZERO full-file `fs::read()` OOM!).
//!   - Multi-file directory: deterministically holds out `files.last()` (bounded prefix read) and Fisher-Yates shuffles ALL
//!     remaining training files per cycle seed with balanced rotating windows across every file.
//!   - Runs lock-free Multi-Stream Data-Parallel Batch Training with Linear Warmup + Cosine LR Decay!

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::brain::{CausalStreamState, DeepContractiveBrain};

const SKIP_DIRS: &[&str] = &[
    ".git", "target", "out", "node_modules", ".venv", "__pycache__", "_cache", ".tmp",
];

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

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
                    if meta.len() >= 128 {
                        out.push(p);
                    }
                }
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Strictly bounded `O(len)` disk range reader (loops on short `read()` calls; NEVER reads full multi-GB file into RAM!).
fn read_exact_range(path: &Path, offset: u64, len: usize) -> Result<Vec<u8>, String> {
    let mut f = File::open(path).map_err(|e| format!("open failed: {}", e))?;
    if offset > 0 {
        f.seek(SeekFrom::Start(offset)).map_err(|e| format!("seek failed: {}", e))?;
    }
    let mut buf = vec![0u8; len];
    let mut filled = 0usize;
    while filled < len {
        let n = f.read(&mut buf[filled..]).map_err(|e| format!("read failed: {}", e))?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    buf.truncate(filled);
    Ok(buf)
}

fn read_window_from_file(path: &Path, max_bytes: usize, rng: &mut u64) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("stat failed: {}", e))?;
    let fsize = meta.len();
    if max_bytes == 0 || fsize <= max_bytes as u64 {
        return read_exact_range(path, 0, fsize as usize);
    }
    let max_off = fsize - (max_bytes as u64);
    let off = splitmix64(rng) % (max_off + 1);
    read_exact_range(path, off, max_bytes)
}

fn read_prefix_from_file(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let meta = fs::metadata(path).map_err(|e| format!("stat failed: {}", e))?;
    let fsize = meta.len();
    let take = if max_bytes > 0 {
        (max_bytes as u64).min(fsize) as usize
    } else {
        fsize as usize
    };
    read_exact_range(path, 0, take)
}

#[derive(Clone, Debug)]
pub struct CorpusSplit {
    pub train_bytes: Vec<u8>,
    pub heldout_bytes: Vec<u8>,
    pub train_files: usize,
    pub total_train_pool: usize,
    pub heldout_label: String,
}

/// Stratified Shuffled Multi-File & Single-Large-File Streaming Loader (`O(budget)` RAM ALWAYS!).
pub fn load_real_corpus_split(
    path: &Path,
    max_train_bytes: usize,
    max_heldout_bytes: usize,
    cycle_seed: u64,
) -> Result<CorpusSplit, String> {
    let files = collect_real_files(path);
    if files.is_empty() {
        return Err(format!("No readable real files found under '{}'", path.display()));
    }

    if files.len() == 1 {
        let fpath = &files[0];
        let meta = fs::metadata(fpath).map_err(|e| format!("stat failed: {}", e))?;
        let fsize = meta.len();
        if fsize < 256 {
            return Err("File too small (<256 bytes) for train/heldout split".into());
        }
        let split = ((fsize as u128 * 85) / 100) as u64;
        let train_avail = split;
        let held_avail = fsize - split;

        let tr_len = if max_train_bytes > 0 {
            (max_train_bytes as u64).min(train_avail) as usize
        } else {
            train_avail as usize
        };
        let tr_off = if (tr_len as u64) < train_avail {
            let mut rng = cycle_seed ^ 0x517C_C1B7;
            splitmix64(&mut rng) % (train_avail - (tr_len as u64) + 1)
        } else {
            0u64
        };
        let tr = read_exact_range(fpath, tr_off, tr_len)?;

        let ho_len = if max_heldout_bytes > 0 {
            (max_heldout_bytes as u64).min(held_avail) as usize
        } else {
            held_avail as usize
        };
        let ho = read_exact_range(fpath, split, ho_len)?;

        return Ok(CorpusSplit {
            train_bytes: tr,
            heldout_bytes: ho,
            train_files: 1,
            total_train_pool: 1,
            heldout_label: format!("tail-15% of {}", fpath.display()),
        });
    }

    // Hold out the last sorted file strictly unseen (`src/sha256.rs` on `src/`)
    let held_path = files.last().unwrap().clone();
    let mut train_pool: Vec<PathBuf> = files[..files.len() - 1].to_vec();
    let total_train_pool = train_pool.len();

    // Deterministic Fisher-Yates shuffle of ALL training files using `cycle_seed`!
    let mut rng = cycle_seed ^ 0xC099_1600_2026;
    for i in (1..train_pool.len()).rev() {
        let j = (splitmix64(&mut rng) as usize) % (i + 1);
        train_pool.swap(i, j);
    }

    let per_file_cap = if max_train_bytes > 0 {
        (max_train_bytes / train_pool.len().max(1)).max(1024)
    } else {
        0
    };

    let mut train_bytes = Vec::with_capacity(if max_train_bytes > 0 { max_train_bytes } else { 256 * 1024 });
    let mut train_files = 0usize;
    for f in &train_pool {
        let remaining = if max_train_bytes > 0 {
            if train_bytes.len() >= max_train_bytes { break; }
            (max_train_bytes - train_bytes.len()).min(per_file_cap)
        } else {
            0
        };
        if let Ok(chunk) = read_window_from_file(f, remaining, &mut rng) {
            if !chunk.is_empty() {
                train_bytes.extend_from_slice(&chunk);
                train_bytes.push(b'\n');
                train_files += 1;
            }
        }
    }
    if max_train_bytes > 0 && train_bytes.len() > max_train_bytes {
        train_bytes.truncate(max_train_bytes);
    }

    let heldout_bytes = read_prefix_from_file(&held_path, max_heldout_bytes)?;

    Ok(CorpusSplit {
        train_bytes,
        heldout_bytes,
        train_files,
        total_train_pool,
        heldout_label: held_path.display().to_string(),
    })
}

#[derive(Clone, Debug)]
pub struct CompressionRunReport {
    pub train_files: usize,
    pub total_train_pool: usize,
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

/// Multi-Stream Data-Parallel Training with Warmup + Cosine LR Schedule on REAL files on disk!
pub fn run_real_byte_training(
    brain: &mut DeepContractiveBrain,
    corpus: &CorpusSplit,
    epochs: usize,
    chunk_len: usize,
    lr_mult: f32,
    threads: usize,
) -> CompressionRunReport {
    let t0 = Instant::now();
    let cl = chunk_len.max(16);

    let (pre_param_bpb, _) = brain.eval_byte_stream(&corpus.heldout_bytes, cl, false, None);

    let n_streams = (threads.max(1) * 2).min(corpus.train_bytes.len() / (cl * 2)).clamp(1, 16);
    let stride = corpus.train_bytes.len() / n_streams;
    let steps_per_epoch = ((stride.saturating_sub(1)) + cl - 1) / cl;
    let total_steps = (steps_per_epoch * epochs.max(1)).max(1);
    let warmup_steps = (total_steps / 10).max(2);
    let mut step_idx = 0usize;
    let mut total_processed = 0usize;

    for ep in 0..epochs.max(1) {
        let mut streams: Vec<CausalStreamState> = (0..n_streams)
            .map(|_| CausalStreamState::new(&brain.cfg))
            .collect();
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
                step_idx += 1;
                let sched = if step_idx <= warmup_steps {
                    (step_idx as f32 / warmup_steps as f32).clamp(0.2, 1.0)
                } else {
                    let p = ((step_idx - warmup_steps) as f32 / (total_steps.saturating_sub(warmup_steps)).max(1) as f32)
                        .clamp(0.0, 1.0);
                    0.20 + 0.80 * 0.5 * (1.0 + (std::f32::consts::PI * p).cos())
                };
                let lr = brain.cfg.lr * lr_mult * sched;
                brain.train_batch_streams(&batch_io, &mut streams, lr, None, update_ep, threads);
            }
            off += step_len;
        }
    }

    let (post_param_bpb, post_blend_bpb) = brain.eval_byte_stream(&corpus.heldout_bytes, cl, false, None);
    let mut online_brain = brain.clone();
    let (_, online_adapted_bpb) = online_brain.eval_byte_stream(&corpus.heldout_bytes, cl, true, None);

    let wall_s = t0.elapsed().as_secs_f64().max(1e-6);
    let throughput_kb_s = ((total_processed + corpus.heldout_bytes.len()) as f64 / 1024.0) / wall_s;
    let best_bpb = post_blend_bpb.min(online_adapted_bpb).max(0.05);

    CompressionRunReport {
        train_files: corpus.train_files,
        total_train_pool: corpus.total_train_pool,
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
