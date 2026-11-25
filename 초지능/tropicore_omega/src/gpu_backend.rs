//! Unified SM120 CUDA 13 + Multi-Threaded CPU Training & Inference Compute Backend (`ABI = 1100`).
//!
//! Dispatches Forward GEMM, Backward `dX`/`dW`/`db` GEMMs, 256-way Byte Cross-Entropy,
//! and Fused AdamW + Banach Contractive `L_∞` Projection + NVFP4 STE directly to:
//!   - `4x NVIDIA RTX PRO 6000 Blackwell` (`--features cuda_sm120`) on Linux workstation
//!   - True `std::thread::scope` multi-threaded CPU (`threads` configurable: `2` on laptop, `192` on 9995WX)

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::nvfp4::nvfp4_quantize_f32;

pub const SM120_TRAINING_ABI: i32 = 1100;

static FWD_GEMM_DISPATCHES: AtomicUsize = AtomicUsize::new(0);
static BWD_GEMM_DISPATCHES: AtomicUsize = AtomicUsize::new(0);
static ADAMW_STEPS_DISPATCHED: AtomicUsize = AtomicUsize::new(0);
static ESTIMATED_FLOPS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "cuda_sm120")]
extern "C" {
    fn tropicore_sm120_abi_version() -> i32;
    fn tropicore_sm120_detect_gpus() -> i32;
    fn tropicore_sm120_linear_fwd(
        h_x: *const f32,
        h_w: *const f32,
        h_b: *const f32,
        h_y: *mut f32,
        batch: i32,
        in_dim: i32,
        out_dim: i32,
        device_id: i32,
    ) -> i32;
    fn tropicore_sm120_linear_bwd(
        h_x: *const f32,
        h_w: *const f32,
        h_dy: *const f32,
        h_dx: *mut f32,
        h_dw: *mut f32,
        h_db: *mut f32,
        batch: i32,
        in_dim: i32,
        out_dim: i32,
        device_id: i32,
    ) -> i32;
    fn tropicore_sm120_byte_ce_fwd_bwd(
        h_logits: *const f32,
        h_targets: *const u8,
        h_dlogits: *mut f32,
        h_avg_bits_out: *mut f32,
        batch: i32,
        device_id: i32,
    ) -> i32;
    fn tropicore_sm120_adamw_contractive_nvfp4_step(
        h_w: *mut f32,
        h_w_q: *mut f32,
        h_grad: *const f32,
        h_m: *mut f32,
        h_v: *mut f32,
        rows: i32,
        cols: i32,
        lr: f32,
        wd: f32,
        beta1: f32,
        beta2: f32,
        step: i32,
        max_linf: f32,
        do_nvfp4_quant: i32,
        device_id: i32,
    ) -> i32;
}

#[derive(Clone, Debug)]
pub struct BackendTelemetry {
    pub backend_name: String,
    pub physical_gpus: usize,
    pub host_cpu_threads: usize,
    pub abi_version: i32,
    pub fwd_gemms: usize,
    pub bwd_gemms: usize,
    pub adamw_steps: usize,
    pub est_gflops: f64,
}

pub fn verify_sm120_training_abi() -> Result<i32, String> {
    #[cfg(feature = "cuda_sm120")]
    {
        let v = unsafe { tropicore_sm120_abi_version() };
        if v != SM120_TRAINING_ABI {
            return Err(format!("SM120 ABI mismatch: CUDA={} vs Rust={}", v, SM120_TRAINING_ABI));
        }
        return Ok(v);
    }
    #[cfg(not(feature = "cuda_sm120"))]
    {
        Ok(SM120_TRAINING_ABI)
    }
}

pub fn detect_physical_gpus() -> usize {
    #[cfg(feature = "cuda_sm120")]
    {
        let n = unsafe { tropicore_sm120_detect_gpus() };
        return n.max(0) as usize;
    }
    #[cfg(not(feature = "cuda_sm120"))]
    {
        0
    }
}

pub fn backend_telemetry() -> BackendTelemetry {
    let gpus = detect_physical_gpus();
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let backend_name = if cfg!(feature = "cuda_sm120") && gpus > 0 {
        format!("CUDA13_SM120_BLACKWELL_TRAIN+INFER ({}x GPU)", gpus)
    } else {
        format!("CPU_MULTITHREAD_NVFP4_TRAIN+INFER ({}T; SM120 ABI=1100 ready)", cpus)
    };
    BackendTelemetry {
        backend_name,
        physical_gpus: gpus,
        host_cpu_threads: cpus,
        abi_version: verify_sm120_training_abi().unwrap_or(SM120_TRAINING_ABI),
        fwd_gemms: FWD_GEMM_DISPATCHES.load(Ordering::Relaxed),
        bwd_gemms: BWD_GEMM_DISPATCHES.load(Ordering::Relaxed),
        adamw_steps: ADAMW_STEPS_DISPATCHED.load(Ordering::Relaxed),
        est_gflops: ESTIMATED_FLOPS.load(Ordering::Relaxed) as f64 / 1e9,
    }
}

/// Batched Linear Forward: `Y[B, O] = X[B, I] * W[O, I]^T + b[O]`.
pub fn linear_fwd(
    x: &[f32],
    w: &[f32],
    b: Option<&[f32]>,
    y: &mut [f32],
    batch: usize,
    in_dim: usize,
    out_dim: usize,
    threads: usize,
    device_id: usize,
) {
    FWD_GEMM_DISPATCHES.fetch_add(1, Ordering::Relaxed);
    ESTIMATED_FLOPS.fetch_add((2u64) * (batch as u64) * (in_dim as u64) * (out_dim as u64), Ordering::Relaxed);

    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && batch * out_dim >= 2048 {
            let b_ptr = b.map_or(std::ptr::null(), |s| s.as_ptr());
            let rc = unsafe {
                tropicore_sm120_linear_fwd(
                    x.as_ptr(),
                    w.as_ptr(),
                    b_ptr,
                    y.as_mut_ptr(),
                    batch as i32,
                    in_dim as i32,
                    out_dim as i32,
                    device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;

    let n_threads = threads.max(1).min(batch.max(1));
    if n_threads <= 1 || batch < 4 {
        for bi in 0..batch {
            let x_row = &x[bi * in_dim..(bi + 1) * in_dim];
            let y_row = &mut y[bi * out_dim..(bi + 1) * out_dim];
            for o in 0..out_dim {
                let w_row = &w[o * in_dim..(o + 1) * in_dim];
                let mut acc = b.map_or(0.0f32, |bv| bv[o]);
                for i in 0..in_dim {
                    acc += x_row[i] * w_row[i];
                }
                y_row[o] = acc;
            }
        }
        return;
    }

    let chunk_rows = (batch + n_threads - 1) / n_threads;
    std::thread::scope(|s| {
        for (ti, y_chunk) in y.chunks_mut(chunk_rows * out_dim).enumerate() {
            let b_start = ti * chunk_rows;
            let b_len = y_chunk.len() / out_dim;
            let x_slice = &x[b_start * in_dim..(b_start + b_len) * in_dim];
            s.spawn(move || {
                for bi in 0..b_len {
                    let x_row = &x_slice[bi * in_dim..(bi + 1) * in_dim];
                    let y_row = &mut y_chunk[bi * out_dim..(bi + 1) * out_dim];
                    for o in 0..out_dim {
                        let w_row = &w[o * in_dim..(o + 1) * in_dim];
                        let mut acc = b.map_or(0.0f32, |bv| bv[o]);
                        for i in 0..in_dim {
                            acc += x_row[i] * w_row[i];
                        }
                        y_row[o] = acc;
                    }
                }
            });
        }
    });
}

/// Batched Linear Backward:
///   `dX[B, I]  = dY[B, O] * W[O, I]`
///   `dW[O, I] += dY[B, O]^T * X[B, I]`
///   `db[O]    += sum_b dY[b, O]`
pub fn linear_bwd(
    x: &[f32],
    w: &[f32],
    dy: &[f32],
    mut dx: Option<&mut [f32]>,
    mut dw: Option<&mut [f32]>,
    mut db: Option<&mut [f32]>,
    batch: usize,
    in_dim: usize,
    out_dim: usize,
    threads: usize,
    device_id: usize,
) {
    BWD_GEMM_DISPATCHES.fetch_add(1, Ordering::Relaxed);
    ESTIMATED_FLOPS.fetch_add((4u64) * (batch as u64) * (in_dim as u64) * (out_dim as u64), Ordering::Relaxed);

    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && batch * out_dim >= 2048 {
            let dx_p = dx.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let dw_p = dw.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let db_p = db.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let rc = unsafe {
                tropicore_sm120_linear_bwd(
                    x.as_ptr(),
                    w.as_ptr(),
                    dy.as_ptr(),
                    dx_p,
                    dw_p,
                    db_p,
                    batch as i32,
                    in_dim as i32,
                    out_dim as i32,
                    device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;

    // 1. dX[B, I] = dY[B, O] * W[O, I] (parallel across batch)
    if let Some(ref mut dx_s) = dx {
        dx_s.fill(0.0);
        let nt = threads.max(1).min(batch.max(1));
        if nt <= 1 || batch < 4 {
            for bi in 0..batch {
                let dy_row = &dy[bi * out_dim..(bi + 1) * out_dim];
                let dx_row = &mut dx_s[bi * in_dim..(bi + 1) * in_dim];
                for o in 0..out_dim {
                    let g = dy_row[o];
                    if g != 0.0 {
                        let w_row = &w[o * in_dim..(o + 1) * in_dim];
                        for i in 0..in_dim {
                            dx_row[i] += g * w_row[i];
                        }
                    }
                }
            }
        } else {
            let chunk_b = (batch + nt - 1) / nt;
            std::thread::scope(|s| {
                for (ti, dx_chunk) in dx_s.chunks_mut(chunk_b * in_dim).enumerate() {
                    let b0 = ti * chunk_b;
                    let blen = dx_chunk.len() / in_dim;
                    let dy_slice = &dy[b0 * out_dim..(b0 + blen) * out_dim];
                    s.spawn(move || {
                        for bi in 0..blen {
                            let dy_row = &dy_slice[bi * out_dim..(bi + 1) * out_dim];
                            let dx_row = &mut dx_chunk[bi * in_dim..(bi + 1) * in_dim];
                            for o in 0..out_dim {
                                let g = dy_row[o];
                                if g != 0.0 {
                                    let w_row = &w[o * in_dim..(o + 1) * in_dim];
                                    for i in 0..in_dim {
                                        dx_row[i] += g * w_row[i];
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }
    }

    // 2. dW[O, I] += dY[B, O]^T * X[B, I] (parallel across output rows `O`!)
    if let Some(ref mut dw_s) = dw {
        let nt = threads.max(1).min(out_dim.max(1));
        if nt <= 1 || out_dim < 32 {
            for o in 0..out_dim {
                let dw_row = &mut dw_s[o * in_dim..(o + 1) * in_dim];
                for bi in 0..batch {
                    let g = dy[bi * out_dim + o];
                    if g != 0.0 {
                        let x_row = &x[bi * in_dim..(bi + 1) * in_dim];
                        for i in 0..in_dim {
                            dw_row[i] += g * x_row[i];
                        }
                    }
                }
            }
        } else {
            let chunk_o = (out_dim + nt - 1) / nt;
            std::thread::scope(|s| {
                for (ti, dw_chunk) in dw_s.chunks_mut(chunk_o * in_dim).enumerate() {
                    let o0 = ti * chunk_o;
                    let olen = dw_chunk.len() / in_dim;
                    s.spawn(move || {
                        for lo in 0..olen {
                            let o = o0 + lo;
                            let dw_row = &mut dw_chunk[lo * in_dim..(lo + 1) * in_dim];
                            for bi in 0..batch {
                                let g = dy[bi * out_dim + o];
                                if g != 0.0 {
                                    let x_row = &x[bi * in_dim..(bi + 1) * in_dim];
                                    for i in 0..in_dim {
                                        dw_row[i] += g * x_row[i];
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }
    }

    // 3. db[O] += sum_b dY[b, O]
    if let Some(ref mut db_s) = db {
        for bi in 0..batch {
            let dy_row = &dy[bi * out_dim..(bi + 1) * out_dim];
            for o in 0..out_dim {
                db_s[o] += dy_row[o];
            }
        }
    }
}

/// Exact 256-way Byte Cross-Entropy (`bits/byte`) + Softmax Backward Gradient (`dLogits`).
pub fn byte_cross_entropy_fwd_bwd(
    logits: &[f32],
    targets: &[u8],
    dlogits: &mut [f32],
    batch: usize,
    device_id: usize,
) -> f32 {
    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && batch >= 16 {
            let mut avg_bits = 0.0f32;
            let rc = unsafe {
                tropicore_sm120_byte_ce_fwd_bwd(
                    logits.as_ptr(),
                    targets.as_ptr(),
                    dlogits.as_mut_ptr(),
                    &mut avg_bits,
                    batch as i32,
                    device_id as i32,
                )
            };
            if rc == 0 {
                return avg_bits;
            }
        }
    }
    let _ = device_id;

    let inv_b = 1.0f32 / (batch.max(1) as f32);
    let mut sum_bits = 0.0f64;
    for b in 0..batch {
        let row = &logits[b * 256..(b + 1) * 256];
        let drow = &mut dlogits[b * 256..(b + 1) * 256];
        let mut mx = f32::NEG_INFINITY;
        for &v in row {
            if v > mx {
                mx = v;
            }
        }
        let mut sum_exp = 0.0f32;
        for c in 0..256 {
            let ex = (row[c] - mx).max(-40.0).exp();
            drow[c] = ex;
            sum_exp += ex;
        }
        let inv_s = 1.0f32 / sum_exp.max(1e-12);
        let tgt = targets[b] as usize;
        for c in 0..256 {
            let p = drow[c] * inv_s;
            if c == tgt {
                sum_bits += -(p.max(1e-9) as f64).log2();
                drow[c] = (p - 1.0) * inv_b;
            } else {
                drow[c] = p * inv_b;
            }
        }
    }
    (sum_bits / batch.max(1) as f64) as f32
}

/// Fused AdamW + optional Banach Row-`L_∞` Contractive Projection (`||row||_1 <= max_linf`)
/// + optional NVFP4 STE shadow-weight quantization!
pub fn adamw_contractive_nvfp4_step(
    w: &mut [f32],
    mut w_q: Option<&mut [f32]>,
    grad: &[f32],
    m: &mut [f32],
    v: &mut [f32],
    rows: usize,
    cols: usize,
    lr: f32,
    wd: f32,
    step: usize,
    max_linf: Option<f32>,
    do_nvfp4: bool,
    device_id: usize,
) {
    ADAMW_STEPS_DISPATCHED.fetch_add(1, Ordering::Relaxed);
    let beta1 = 0.9f32;
    let beta2 = 0.999f32;
    let st = step.max(1) as i32;

    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && rows * cols >= 16384 {
            let wq_p = w_q.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let rc = unsafe {
                tropicore_sm120_adamw_contractive_nvfp4_step(
                    w.as_mut_ptr(),
                    wq_p,
                    grad.as_ptr(),
                    m.as_mut_ptr(),
                    v.as_mut_ptr(),
                    rows as i32,
                    cols as i32,
                    lr,
                    wd,
                    beta1,
                    beta2,
                    st,
                    max_linf.unwrap_or(0.0),
                    if do_nvfp4 { 1 } else { 0 },
                    device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;

    let inv_bc1 = 1.0f32 / (1.0 - beta1.powi(st)).max(1e-6);
    let inv_bc2 = 1.0f32 / (1.0 - beta2.powi(st)).max(1e-6);

    for r in 0..rows {
        let base = r * cols;
        let mut l1 = 0.0f32;
        for c in 0..cols {
            let idx = base + c;
            let g = grad[idx].clamp(-5.0, 5.0);
            let mi = beta1 * m[idx] + (1.0 - beta1) * g;
            let vi = beta2 * v[idx] + (1.0 - beta2) * g * g;
            m[idx] = mi;
            v[idx] = vi;
            let mh = mi * inv_bc1;
            let vh = vi * inv_bc2;
            let wi = w[idx] * (1.0 - lr * wd) - lr * mh / (vh.sqrt() + 1e-8);
            w[idx] = wi;
            l1 += wi.abs();
        }
        if let Some(rho) = max_linf {
            if l1 > rho + 1e-6 {
                let sc = (rho - 1e-6) / l1;
                for c in 0..cols {
                    w[base + c] *= sc;
                }
            }
        }
        if let Some(ref mut wqs) = w_q {
            if do_nvfp4 {
                nvfp4_quantize_f32(&w[base..base + cols], &mut wqs[base..base + cols]);
            } else {
                wqs[base..base + cols].copy_from_slice(&w[base..base + cols]);
            }
        }
    }
}
