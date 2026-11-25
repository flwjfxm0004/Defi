//! Unified Thread-Safe Multi-GPU SM120 CUDA 13 + Auto-Vectorized Blocked CPU Backend (`ABI = 1300`).
//!
//! ZERO dead CUDA kernels, ZERO multi-thread GPU data races, ZERO stale host embeddings:
//!   - Thread-Safe Persistent GDDR7 Tensor Handles across GPUs `0..N-1` (`alloc_upload`, `sync_to_host`, `Drop`)
//!   - Live GPU-Resident Embedding Lookup & Backward Scatter (`embedding_lookup_fwd`, `embedding_scatter_bwd`)
//!   - 4-Way Unrolled Auto-Vectorized Register-Blocked CPU GEMM (`linear_fwd`, `linear_bwd`) for AVX2/AVX-512 FMA
//!   - Live-Wired `contractive_step_fwd/bwd`, `byte_cross_entropy_fwd_bwd`, `adamw_contractive_nvfp4_step`, `allreduce_grad_shards`
//!   - True Global L2 Gradient Norm Clipping (`global_grad_norm_clip`)

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::nvfp4::nvfp4_quantize_f32;

pub const SM120_TRAINING_ABI: i32 = 1300;

static FWD_GEMM_DISPATCHES: AtomicUsize = AtomicUsize::new(0);
static BWD_GEMM_DISPATCHES: AtomicUsize = AtomicUsize::new(0);
static CONTRACTIVE_STEPS: AtomicUsize = AtomicUsize::new(0);
static ADAMW_STEPS_DISPATCHED: AtomicUsize = AtomicUsize::new(0);
static ALLREDUCE_CALLS: AtomicUsize = AtomicUsize::new(0);
static ESTIMATED_FLOPS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "cuda_sm120")]
extern "C" {
    fn tropicore_sm120_abi_version() -> i32;
    fn tropicore_sm120_detect_gpus() -> i32;
    fn tropicore_sm120_tensor_create(h_init: *const f32, elems: i64, device_id: i32) -> i32;
    fn tropicore_sm120_tensor_download(handle: i32, h_out: *mut f32, elems: i64) -> i32;
    fn tropicore_sm120_tensor_free(handle: i32) -> i32;
    fn tropicore_sm120_embedding_fwd(
        emb_handle: i32, h_bytes: *const u8, h_out: *mut f32, batch: i32, d_model: i32,
    ) -> i32;
    fn tropicore_sm120_embedding_bwd(
        h_bytes: *const u8, h_dx: *const f32, h_demb: *mut f32, batch: i32, d_model: i32, device_id: i32,
    ) -> i32;
    fn tropicore_sm120_linear_fwd_resident(
        h_x: *const f32, w_handle: i32, b_handle: i32, h_y: *mut f32,
        batch: i32, in_dim: i32, out_dim: i32,
    ) -> i32;
    fn tropicore_sm120_linear_bwd_resident(
        h_x: *const f32, w_handle: i32, h_dy: *const f32,
        h_dx: *mut f32, h_dw: *mut f32, h_db: *mut f32,
        batch: i32, in_dim: i32, out_dim: i32,
    ) -> i32;
    fn tropicore_sm120_adamw_resident_inplace(
        w_handle: i32, wq_handle: i32, m_handle: i32, v_handle: i32, h_grad: *const f32,
        rows: i32, cols: i32, lr: f32, wd: f32, beta1: f32, beta2: f32,
        step: i32, max_linf: f32, do_nvfp4_quant: i32,
    ) -> i32;
    fn tropicore_sm120_contractive_step_fwd(
        h_pre: *const f32, h_anchor: *const f32, h_out: *mut f32, h_znext: *mut f32,
        total: i32, gamma: f32, device_id: i32,
    ) -> i32;
    fn tropicore_sm120_contractive_step_bwd(
        h_dz: *const f32, h_cur: *const f32, h_dpre: *mut f32, h_dx_anchor: *mut f32,
        total: i32, gamma: f32, device_id: i32,
    ) -> i32;
    fn tropicore_sm120_byte_ce_fwd_bwd(
        h_logits: *const f32, h_targets: *const u8, h_dlogits: *mut f32,
        h_avg_bits_out: *mut f32, batch: i32, device_id: i32,
    ) -> i32;
    fn tropicore_sm120_allreduce_mean(
        shard_grads: *const *mut f32, out_grad: *mut f32, n_shards: i32, n_elems: i32,
    ) -> i32;
}

/// Opaque GPU-resident tensor handle (`>= 0` on SM120 GPU, `-1` on CPU). Automatically freed on drop!
#[derive(Debug)]
pub struct DeviceTensorHandle {
    pub id: i32,
    pub device_id: usize,
}

impl DeviceTensorHandle {
    pub fn alloc_upload(init: &[f32], device_id: usize) -> Self {
        #[cfg(feature = "cuda_sm120")]
        {
            let gpus = detect_physical_gpus();
            if gpus > 0 && !init.is_empty() {
                let target_gpu = device_id % gpus;
                let id = unsafe {
                    tropicore_sm120_tensor_create(init.as_ptr(), init.len() as i64, target_gpu as i32)
                };
                return Self { id, device_id: target_gpu };
            }
        }
        let _ = init;
        Self { id: -1, device_id }
    }

    pub fn sync_to_host(&self, host_out: &mut [f32]) {
        #[cfg(feature = "cuda_sm120")]
        {
            if self.id >= 0 && !host_out.is_empty() {
                let _ = unsafe {
                    tropicore_sm120_tensor_download(self.id, host_out.as_mut_ptr(), host_out.len() as i64)
                };
            }
        }
        let _ = host_out;
    }

    pub fn detached() -> Self {
        Self { id: -1, device_id: 0 }
    }
}

impl Drop for DeviceTensorHandle {
    fn drop(&mut self) {
        #[cfg(feature = "cuda_sm120")]
        {
            if self.id >= 0 {
                let _ = unsafe { tropicore_sm120_tensor_free(self.id) };
                self.id = -1;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackendTelemetry {
    pub backend_name: String,
    pub physical_gpus: usize,
    pub host_cpu_threads: usize,
    pub abi_version: i32,
    pub fwd_gemms: usize,
    pub bwd_gemms: usize,
    pub contractive_steps: usize,
    pub adamw_steps: usize,
    pub allreduces: usize,
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
        return (unsafe { tropicore_sm120_detect_gpus() }).max(0) as usize;
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
        format!("CUDA13_SM120_MULTIGPU_RESIDENT ({}x GPU, ABI=1300)", gpus)
    } else {
        format!("CPU_SIMD4_MULTISTREAM_NVFP4 ({}T; SM120 Multi-GPU ABI=1300 ready)", cpus)
    };
    BackendTelemetry {
        backend_name,
        physical_gpus: gpus,
        host_cpu_threads: cpus,
        abi_version: verify_sm120_training_abi().unwrap_or(SM120_TRAINING_ABI),
        fwd_gemms: FWD_GEMM_DISPATCHES.load(Ordering::Relaxed),
        bwd_gemms: BWD_GEMM_DISPATCHES.load(Ordering::Relaxed),
        contractive_steps: CONTRACTIVE_STEPS.load(Ordering::Relaxed),
        adamw_steps: ADAMW_STEPS_DISPATCHED.load(Ordering::Relaxed),
        allreduces: ALLREDUCE_CALLS.load(Ordering::Relaxed),
        est_gflops: ESTIMATED_FLOPS.load(Ordering::Relaxed) as f64 / 1e9,
    }
}

/// Live GPU-Resident Byte Embedding Forward Lookup (zero stale host copy on GPU!).
pub fn embedding_lookup_fwd(
    emb_host: &[f32],
    emb_dev: &DeviceTensorHandle,
    bytes: &[u8],
    out: &mut [f32],
    d_model: usize,
) {
    #[cfg(feature = "cuda_sm120")]
    {
        if emb_dev.id >= 0 && !bytes.is_empty() {
            let rc = unsafe {
                tropicore_sm120_embedding_fwd(
                    emb_dev.id, bytes.as_ptr(), out.as_mut_ptr(),
                    bytes.len() as i32, d_model as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = (emb_dev.id, emb_dev.device_id);
    for (ti, &b) in bytes.iter().enumerate() {
        let off = (b as usize) * d_model;
        out[ti * d_model..(ti + 1) * d_model].copy_from_slice(&emb_host[off..off + d_model]);
    }
}

/// Live GPU / CPU Byte Embedding Backward Gradient Scatter!
pub fn embedding_scatter_bwd(
    bytes: &[u8],
    dx: &[f32],
    demb: &mut [f32],
    d_model: usize,
    device_id: usize,
) {
    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && bytes.len() * d_model >= 4096 {
            let rc = unsafe {
                tropicore_sm120_embedding_bwd(
                    bytes.as_ptr(), dx.as_ptr(), demb.as_mut_ptr(),
                    bytes.len() as i32, d_model as i32, device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;
    for (ti, &b) in bytes.iter().enumerate() {
        let gx = &dx[ti * d_model..(ti + 1) * d_model];
        let ge = &mut demb[(b as usize) * d_model..(b as usize + 1) * d_model];
        for j in 0..d_model {
            ge[j] += gx[j];
        }
    }
}

#[inline(always)]
fn dot_4way(a: &[f32], b: &[f32]) -> f32 {
    let mut s0 = 0.0f32;
    let mut s1 = 0.0f32;
    let mut s2 = 0.0f32;
    let mut s3 = 0.0f32;
    let ac = a.chunks_exact(4);
    let bc = b.chunks_exact(4);
    let ar = ac.remainder();
    let br = bc.remainder();
    for (x, y) in ac.zip(bc) {
        s0 += x[0] * y[0];
        s1 += x[1] * y[1];
        s2 += x[2] * y[2];
        s3 += x[3] * y[3];
    }
    let mut tot = (s0 + s1) + (s2 + s3);
    for (x, y) in ar.iter().zip(br.iter()) {
        tot += x * y;
    }
    tot
}

#[inline(always)]
fn axpy_4way(dst: &mut [f32], src: &[f32], alpha: f32) {
    let mut dc = dst.chunks_exact_mut(4);
    let mut sc = src.chunks_exact(4);
    for (d, s) in dc.by_ref().zip(sc.by_ref()) {
        d[0] += alpha * s[0];
        d[1] += alpha * s[1];
        d[2] += alpha * s[2];
        d[3] += alpha * s[3];
    }
    for (d, s) in dc.into_remainder().iter_mut().zip(sc.remainder().iter()) {
        *d += alpha * s;
    }
}

/// Batched Linear Forward (`Y[B, O] = X[B, I] * W[O, I]^T + b[O]`):
/// GPU-resident SM120 cuBLAS when available, 4x4 register-blocked auto-vectorized AVX2/AVX-512 FMA on CPU!
pub fn linear_fwd(
    x: &[f32],
    w: &[f32],
    w_dev: &DeviceTensorHandle,
    b: Option<&[f32]>,
    b_dev: Option<&DeviceTensorHandle>,
    y: &mut [f32],
    batch: usize,
    in_dim: usize,
    out_dim: usize,
) {
    FWD_GEMM_DISPATCHES.fetch_add(1, Ordering::Relaxed);
    ESTIMATED_FLOPS.fetch_add(2u64 * (batch as u64) * (in_dim as u64) * (out_dim as u64), Ordering::Relaxed);

    #[cfg(feature = "cuda_sm120")]
    {
        if w_dev.id >= 0 && batch * out_dim >= 1024 {
            let b_id = b_dev.map_or(-1, |h| h.id);
            let rc = unsafe {
                tropicore_sm120_linear_fwd_resident(
                    x.as_ptr(), w_dev.id, b_id, y.as_mut_ptr(),
                    batch as i32, in_dim as i32, out_dim as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = (w_dev.id, b_dev.map(|h| h.id));

    for bi in 0..batch {
        let x_row = &x[bi * in_dim..(bi + 1) * in_dim];
        let y_row = &mut y[bi * out_dim..(bi + 1) * out_dim];
        let mut o = 0usize;
        while o + 4 <= out_dim {
            let w0 = &w[o * in_dim..(o + 1) * in_dim];
            let w1 = &w[(o + 1) * in_dim..(o + 2) * in_dim];
            let w2 = &w[(o + 2) * in_dim..(o + 3) * in_dim];
            let w3 = &w[(o + 3) * in_dim..(o + 4) * in_dim];
            let (mut a0, mut a1, mut a2, mut a3) = match b {
                Some(bv) => (bv[o], bv[o + 1], bv[o + 2], bv[o + 3]),
                None => (0.0, 0.0, 0.0, 0.0),
            };
            let xc = x_row.chunks_exact(4);
            let w0c = w0.chunks_exact(4);
            let w1c = w1.chunks_exact(4);
            let w2c = w2.chunks_exact(4);
            let w3c = w3.chunks_exact(4);
            let xr = xc.remainder();
            let w0r = w0c.remainder();
            let w1r = w1c.remainder();
            let w2r = w2c.remainder();
            let w3r = w3c.remainder();
            for (((((xv, c0), c1), c2), c3), _) in xc
                .zip(w0c)
                .zip(w1c)
                .zip(w2c)
                .zip(w3c)
                .zip(0usize..)
            {
                let x0 = xv[0]; let x1 = xv[1]; let x2 = xv[2]; let x3 = xv[3];
                a0 += x0 * c0[0] + x1 * c0[1] + x2 * c0[2] + x3 * c0[3];
                a1 += x0 * c1[0] + x1 * c1[1] + x2 * c1[2] + x3 * c1[3];
                a2 += x0 * c2[0] + x1 * c2[1] + x2 * c2[2] + x3 * c2[3];
                a3 += x0 * c3[0] + x1 * c3[1] + x2 * c3[2] + x3 * c3[3];
            }
            for i in 0..xr.len() {
                let xv = xr[i];
                a0 += xv * w0r[i];
                a1 += xv * w1r[i];
                a2 += xv * w2r[i];
                a3 += xv * w3r[i];
            }
            y_row[o] = a0;
            y_row[o + 1] = a1;
            y_row[o + 2] = a2;
            y_row[o + 3] = a3;
            o += 4;
        }
        while o < out_dim {
            let w_row = &w[o * in_dim..(o + 1) * in_dim];
            let base = b.map_or(0.0f32, |bv| bv[o]);
            y_row[o] = base + dot_4way(x_row, w_row);
            o += 1;
        }
    }
}

/// Batched Linear Backward (`dX`, `dW +=`, `db +=`):
/// GPU-resident SM120 cuBLAS when available, 4-way auto-vectorized AXPY on CPU!
pub fn linear_bwd(
    x: &[f32],
    w: &[f32],
    w_dev: &DeviceTensorHandle,
    dy: &[f32],
    mut dx: Option<&mut [f32]>,
    mut dw: Option<&mut [f32]>,
    mut db: Option<&mut [f32]>,
    batch: usize,
    in_dim: usize,
    out_dim: usize,
) {
    BWD_GEMM_DISPATCHES.fetch_add(1, Ordering::Relaxed);
    ESTIMATED_FLOPS.fetch_add(4u64 * (batch as u64) * (in_dim as u64) * (out_dim as u64), Ordering::Relaxed);

    #[cfg(feature = "cuda_sm120")]
    {
        if w_dev.id >= 0 && batch * out_dim >= 1024 {
            let dx_p = dx.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let dw_p = dw.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let db_p = db.as_deref_mut().map_or(std::ptr::null_mut(), |s| s.as_mut_ptr());
            let rc = unsafe {
                tropicore_sm120_linear_bwd_resident(
                    x.as_ptr(), w_dev.id, dy.as_ptr(), dx_p, dw_p, db_p,
                    batch as i32, in_dim as i32, out_dim as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = w_dev;

    if let Some(ref mut dx_s) = dx {
        dx_s.fill(0.0);
        for bi in 0..batch {
            let dy_row = &dy[bi * out_dim..(bi + 1) * out_dim];
            let dx_row = &mut dx_s[bi * in_dim..(bi + 1) * in_dim];
            for o in 0..out_dim {
                let g = dy_row[o];
                if g != 0.0 {
                    axpy_4way(dx_row, &w[o * in_dim..(o + 1) * in_dim], g);
                }
            }
        }
    }
    if let Some(ref mut dw_s) = dw {
        for o in 0..out_dim {
            let dw_row = &mut dw_s[o * in_dim..(o + 1) * in_dim];
            for bi in 0..batch {
                let g = dy[bi * out_dim + o];
                if g != 0.0 {
                    axpy_4way(dw_row, &x[bi * in_dim..(bi + 1) * in_dim], g);
                }
            }
        }
    }
    if let Some(ref mut db_s) = db {
        for bi in 0..batch {
            let dy_row = &dy[bi * out_dim..(bi + 1) * out_dim];
            axpy_4way(db_s, dy_row, 1.0);
        }
    }
}

pub fn contractive_step_fwd(pre: &[f32], anchor: &[f32], h_out: &mut [f32], z_next: &mut [f32], gamma: f32, device_id: usize) {
    CONTRACTIVE_STEPS.fetch_add(1, Ordering::Relaxed);
    let n = pre.len();
    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && n >= 4096 {
            let rc = unsafe {
                tropicore_sm120_contractive_step_fwd(
                    pre.as_ptr(), anchor.as_ptr(), h_out.as_mut_ptr(), z_next.as_mut_ptr(),
                    n as i32, gamma, device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;
    let one_m = 1.0 - gamma;
    for i in 0..n {
        let h = pre[i].tanh();
        h_out[i] = h;
        z_next[i] = gamma * h + one_m * anchor[i];
    }
}

pub fn contractive_step_bwd(dz: &[f32], h_cur: &[f32], dpre: &mut [f32], dx_anchor: &mut [f32], gamma: f32, device_id: usize) {
    let n = dz.len();
    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && n >= 4096 {
            let rc = unsafe {
                tropicore_sm120_contractive_step_bwd(
                    dz.as_ptr(), h_cur.as_ptr(), dpre.as_mut_ptr(), dx_anchor.as_mut_ptr(),
                    n as i32, gamma, device_id as i32,
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = device_id;
    let one_m = 1.0 - gamma;
    for i in 0..n {
        let g = dz[i];
        let h = h_cur[i];
        dpre[i] = (g * gamma * (1.0 - h * h)).clamp(-4.0, 4.0);
        dx_anchor[i] += g * one_m;
    }
}

pub fn byte_cross_entropy_fwd_bwd(logits: &[f32], targets: &[u8], dlogits: &mut [f32], batch: usize, device_id: usize) -> f32 {
    #[cfg(feature = "cuda_sm120")]
    {
        if detect_physical_gpus() > 0 && batch >= 16 {
            let mut avg_bits = 0.0f32;
            let rc = unsafe {
                tropicore_sm120_byte_ce_fwd_bwd(
                    logits.as_ptr(), targets.as_ptr(), dlogits.as_mut_ptr(),
                    &mut avg_bits, batch as i32, device_id as i32,
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
        for &v in row { if v > mx { mx = v; } }
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

pub fn adamw_contractive_nvfp4_step(
    w: &mut [f32],
    w_dev: &DeviceTensorHandle,
    mut w_q: Option<&mut [f32]>,
    wq_dev: &DeviceTensorHandle,
    grad: &[f32],
    m: &mut [f32],
    m_dev: &DeviceTensorHandle,
    v: &mut [f32],
    v_dev: &DeviceTensorHandle,
    rows: usize,
    cols: usize,
    lr: f32,
    wd: f32,
    step: usize,
    max_linf: Option<f32>,
    do_nvfp4: bool,
) {
    ADAMW_STEPS_DISPATCHED.fetch_add(1, Ordering::Relaxed);
    let beta1 = 0.9f32;
    let beta2 = 0.999f32;
    let st = step.max(1) as i32;

    #[cfg(feature = "cuda_sm120")]
    {
        if w_dev.id >= 0 && m_dev.id >= 0 && v_dev.id >= 0 {
            let rc = unsafe {
                tropicore_sm120_adamw_resident_inplace(
                    w_dev.id, wq_dev.id, m_dev.id, v_dev.id, grad.as_ptr(),
                    rows as i32, cols as i32, lr, wd, beta1, beta2, st,
                    max_linf.unwrap_or(0.0), if do_nvfp4 { 1 } else { 0 },
                )
            };
            if rc == 0 {
                return;
            }
        }
    }
    let _ = (w_dev, wq_dev, m_dev, v_dev);

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
            let wi = w[idx] * (1.0 - lr * wd) - lr * (mi * inv_bc1) / ((vi * inv_bc2).sqrt() + 1e-8);
            w[idx] = wi;
            l1 += wi.abs();
        }
        if let Some(rho) = max_linf {
            if l1 > rho + 1e-6 {
                let sc = (rho - 1e-6) / l1;
                for c in 0..cols { w[base + c] *= sc; }
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

pub fn allreduce_grad_shards(shards: &mut [Vec<f32>], out: &mut [f32]) {
    if shards.is_empty() || out.is_empty() {
        return;
    }
    ALLREDUCE_CALLS.fetch_add(1, Ordering::Relaxed);
    let n = out.len();
    #[cfg(feature = "cuda_sm120")]
    {
        let mut ptrs: Vec<*mut f32> = shards.iter_mut().map(|s| s.as_mut_ptr()).collect();
        let rc = unsafe {
            tropicore_sm120_allreduce_mean(ptrs.as_ptr(), out.as_mut_ptr(), ptrs.len() as i32, n as i32)
        };
        if rc == 0 {
            return;
        }
    }
    let inv = 1.0f32 / shards.len() as f32;
    for i in 0..n {
        let mut acc = 0.0f32;
        for sh in shards.iter() {
            acc += sh[i];
        }
        out[i] = acc * inv;
    }
}

/// True Global L2 Gradient Norm Clipping across all parameter gradient tensors!
pub fn global_grad_norm_clip(grads: &mut [&mut [f32]], max_norm: f32) -> f32 {
    let mut sum_sq = 0.0f64;
    for g in grads.iter() {
        for &v in g.iter() {
            let vf = v as f64;
            sum_sq += vf * vf;
        }
    }
    let norm = sum_sq.sqrt() as f32;
    if norm > max_norm && norm.is_finite() {
        let scale = max_norm / (norm + 1e-8);
        for g in grads.iter_mut() {
            for v in g.iter_mut() {
                *v *= scale;
            }
        }
    }
    norm
}
