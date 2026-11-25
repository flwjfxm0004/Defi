//! Unified GPU-Resident SM120 CUDA 13 + Multi-Threaded CPU Training Backend (`ABI = 1200`).
//!
//! ZERO dead CUDA kernels — every single `.cu` entrypoint is live-wired:
//!   - Persistent GDDR7 Tensor Handles (`DeviceTensorHandle`: upload ONCE, update IN-PLACE on GPU!)
//!   - `linear_fwd` / `linear_bwd` (reads GPU-resident weight handles directly with ZERO PCIe weight copies!)
//!   - `contractive_step_fwd` / `contractive_step_bwd` (live-wired Banach equilibrium ticks!)
//!   - `byte_cross_entropy_fwd_bwd` (exact 256-way `bits/byte` + softmax backward)
//!   - `adamw_contractive_nvfp4_step` (in-place GPU AdamW + Banach Row-L_inf + Block-16 NVFP4 STE)
//!   - `allreduce_grad_shards` (live-wired multi-stream / 4-GPU gradient averaging!)

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::nvfp4::nvfp4_quantize_f32;

pub const SM120_TRAINING_ABI: i32 = 1200;

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
}

impl DeviceTensorHandle {
    pub fn alloc_upload(init: &[f32], device_id: usize) -> Self {
        #[cfg(feature = "cuda_sm120")]
        {
            if detect_physical_gpus() > 0 && !init.is_empty() {
                let id = unsafe { tropicore_sm120_tensor_create(init.as_ptr(), init.len() as i64, device_id as i32) };
                return Self { id };
            }
        }
        let _ = (init, device_id);
        Self { id: -1 }
    }

    pub fn sync_to_host(&self, host_out: &mut [f32]) {
        #[cfg(feature = "cuda_sm120")]
        {
            if self.id >= 0 && !host_out.is_empty() {
                let _ = unsafe { tropicore_sm120_tensor_download(self.id, host_out.as_mut_ptr(), host_out.len() as i64) };
            }
        }
        let _ = host_out;
    }
}

impl Clone for DeviceTensorHandle {
    fn clone(&self) -> Self {
        // Cloned CPU brains lazily re-bind or run CPU fallback; prevents double-free of GPU handle!
        Self { id: -1 }
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
        format!("CUDA13_SM120_GPU_RESIDENT ({}x GPU, ABI=1200)", gpus)
    } else {
        format!("CPU_MULTISTREAM_NVFP4 ({}T; SM120 GPU-Resident ABI=1200 ready)", cpus)
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

/// Batched Linear Forward (`Y[B, O] = X[B, I] * W[O, I]^T + b[O]`): uses GPU-resident handles when available!
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
        for o in 0..out_dim {
            let w_row = &w[o * in_dim..(o + 1) * in_dim];
            let mut acc = b.map_or(0.0f32, |bv| bv[o]);
            for i in 0..in_dim {
                acc += x_row[i] * w_row[i];
            }
            y_row[o] = acc;
        }
    }
}

/// Batched Linear Backward (`dX`, `dW +=`, `db +=`): uses GPU-resident handles when available!
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
                    let w_row = &w[o * in_dim..(o + 1) * in_dim];
                    for i in 0..in_dim {
                        dx_row[i] += g * w_row[i];
                    }
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
                    let x_row = &x[bi * in_dim..(bi + 1) * in_dim];
                    for i in 0..in_dim {
                        dw_row[i] += g * x_row[i];
                    }
                }
            }
        }
    }
    if let Some(ref mut db_s) = db {
        for bi in 0..batch {
            let dy_row = &dy[bi * out_dim..(bi + 1) * out_dim];
            for o in 0..out_dim {
                db_s[o] += dy_row[o];
            }
        }
    }
}

/// Live-wired Contractive Equilibrium Forward Step (`h = tanh(pre); z_next = gamma*h + (1-gamma)*anchor`).
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
    for i in 0..n {
        let h = pre[i].tanh();
        h_out[i] = h;
        z_next[i] = gamma * h + (1.0 - gamma) * anchor[i];
    }
}

/// Live-wired Contractive Equilibrium Backward Step (`dpre = dz*gamma*(1-h^2); dx_anchor += dz*(1-gamma)`).
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
    for i in 0..n {
        let g = dz[i];
        let h = h_cur[i];
        dpre[i] = (g * gamma * (1.0 - h * h)).clamp(-4.0, 4.0);
        dx_anchor[i] += g * (1.0 - gamma);
    }
}

/// Exact 256-way Byte Cross-Entropy (`bits/byte`) + Softmax Backward Gradient.
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

/// Fused GPU-Resident In-Place (or CPU) AdamW + Banach Row-`L_∞` Projection + Block-16 NVFP4 STE!
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

/// Live-wired Multi-Stream / 4-GPU Data-Parallel Gradient AllReduce Mean!
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
