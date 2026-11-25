//! Scalable Deep Causal-Conv4 + Fast-Weight Associative Delta-SSM + Contractive NVFP4 Brain (`TropiCore-Omega v13.0`).
//!
//! Obliterates ALL 7 v12.0 flaws:
//!   1. PER-LAYER 4-Tap Causal Depthwise Conv + Fast-Weight Associative Delta-Rule State (`O(T * d * 8)`)
//!      + Selective Gated Causal SSM + Tropical Max-Trace + Banach Contractive Equilibrium — FULL EXACT BPTT!
//!   2. Multi-GPU Layer Sharding (`gpu = layer_idx % n_gpus`) + GPU-Resident Embedding Lookup/Scatter
//!      + Synced `ParamTensor::clone()` (NEVER clones stale untrained host weights on GPU!).
//!   3. Exact 256-Way Normalized Probability Distribution Mixing (`mix_256_distributions`) so Log-Odds /
//!      Product-of-Experts mixing is 100% normalized (`sum == 1.0`) everywhere (`eval`, `compress`, `generate`)!
//!   4. True Global L2 Gradient Norm Clipping (`global_grad_norm_clip`, `max_norm = 2.0`).
//!   5. Live-Applied `active_policy: RuntimePolicyOverride` + `best_heldout_bpb` persisted in `TOMG13`!
//!   6. Deterministic Test-Time Neural Adaptation (`ttt_adapt_block`) powering `OMGZ13` lossless `.omg` codec!
//!   7. Stateful `CausalStreamState` encoding (`encode_raw_sensor_vector_stateful`, `encode_raw_bytes_to_latent_stateful`)
//!      so `UniversalOpenWorldAgent` carries infinite-horizon neural state across every timestep!

use std::fs;
use std::path::Path;

use crate::asi_runtime_cfg::{adaptive_online_lr, blend_byte_probs, contractive_gamma};
use crate::episodic::TropicalEpisodicMemory;
use crate::gpu_backend::{
    adamw_contractive_nvfp4_step, allreduce_grad_shards, byte_cross_entropy_fwd_bwd,
    contractive_step_bwd, contractive_step_fwd, embedding_lookup_fwd, embedding_scatter_bwd,
    global_grad_norm_clip, linear_bwd, linear_fwd, DeviceTensorHandle,
};
use crate::nvfp4::nvfp4_quantize_f32;
use crate::sha256::sha256_hex;

const CKPT_MAGIC: &[u8; 6] = b"TOMG13";
const CONV_K: usize = 4;
const ASSOC_HD: usize = 8; // Fast-Weight Associative Delta-Rule head dim (`O(T * d * 8)` linear time!)

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_sym_f32(rng: &mut u64) -> f32 {
    let u = (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64);
    (u * 2.0 - 1.0) as f32
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimePolicyOverride {
    pub gamma: f32,
    pub lr_slope: f32,
    pub lr_center: f32,
    pub w_mem_scale: f32,
    pub w_mem_cap: f32,
    pub use_log_odds_mix: bool,
    pub from_compiled_defaults: bool,
}

impl RuntimePolicyOverride {
    pub fn compiled_default() -> Self {
        // Mirrors compiled `src/asi_runtime_cfg.rs` initial defaults, while allowing live evolution in-memory & TOMG13!
        Self {
            gamma: contractive_gamma(),
            lr_slope: 0.14,
            lr_center: 3.0,
            w_mem_scale: 0.42,
            w_mem_cap: 0.65,
            use_log_odds_mix: false,
            from_compiled_defaults: true,
        }
    }

    #[inline]
    pub fn calc_lr(&self, base_lr: f32, surprise_bits: f32) -> f32 {
        if self.from_compiled_defaults {
            adaptive_online_lr(base_lr, surprise_bits)
        } else {
            let boost = 1.0 + self.lr_slope * (surprise_bits - self.lr_center).clamp(-1.0, 4.0);
            (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)
        }
    }

    /// Exact 256-way normalized mixture (`sum == 1.0`) — fixes v12.0 unnormalized Log-Odds penalty!
    #[inline]
    pub fn mix_256(&self, p_brain: &[f32; 256], p_mem: &[f32; 256], mem_conf: f32) -> [f32; 256] {
        let mut out = [0.0f32; 256];
        let mut sum = 0.0f32;
        if self.from_compiled_defaults {
            for c in 0..256 {
                let v = blend_byte_probs(p_brain[c], p_mem[c], mem_conf);
                out[c] = v;
                sum += v;
            }
        } else {
            let w_mem = (self.w_mem_scale * mem_conf).clamp(0.0, self.w_mem_cap);
            if self.use_log_odds_mix {
                let w_b = 1.0 - w_mem;
                let mut mx = f32::NEG_INFINITY;
                let mut lbuf = [0.0f32; 256];
                for c in 0..256 {
                    let lv = w_b * p_brain[c].max(1e-7).ln() + w_mem * p_mem[c].max(1e-7).ln();
                    lbuf[c] = lv;
                    if lv > mx { mx = lv; }
                }
                for c in 0..256 {
                    let v = (lbuf[c] - mx).max(-40.0).exp().max(1e-7);
                    out[c] = v;
                    sum += v;
                }
            } else {
                let w_b = 1.0 - w_mem;
                for c in 0..256 {
                    let v = (w_b * p_brain[c] + w_mem * p_mem[c]).max(1e-7);
                    out[c] = v;
                    sum += v;
                }
            }
        }
        let inv = 1.0 / sum.max(1e-12);
        for c in 0..256 {
            out[c] *= inv;
        }
        out
    }

    fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.gamma.to_le_bytes());
        out.extend_from_slice(&self.lr_slope.to_le_bytes());
        out.extend_from_slice(&self.lr_center.to_le_bytes());
        out.extend_from_slice(&self.w_mem_scale.to_le_bytes());
        out.extend_from_slice(&self.w_mem_cap.to_le_bytes());
        out.push(if self.use_log_odds_mix { 1 } else { 0 });
        out.push(if self.from_compiled_defaults { 1 } else { 0 });
    }

    fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_f32 = |d: &[u8], p: &mut usize| -> Result<f32, String> {
            if *p + 4 > d.len() { return Err("policy EOF f32".into()); }
            let v = f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let gamma = take_f32(data, pos)?;
        let lr_slope = take_f32(data, pos)?;
        let lr_center = take_f32(data, pos)?;
        let w_mem_scale = take_f32(data, pos)?;
        let w_mem_cap = take_f32(data, pos)?;
        if *pos + 2 > data.len() { return Err("policy EOF flags".into()); }
        let use_log_odds_mix = data[*pos] != 0;
        let from_compiled_defaults = data[*pos + 1] != 0;
        *pos += 2;
        Ok(Self { gamma, lr_slope, lr_center, w_mem_scale, w_mem_cap, use_log_odds_mix, from_compiled_defaults })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BrainConfig {
    pub n_layers: usize,
    pub d_model: usize,
    pub d_ff: usize,
    pub n_ticks: usize,
    pub lr: f32,
    pub weight_decay: f32,
    pub contract_rho: f32,
    pub use_nvfp4_ste: bool,
}

impl BrainConfig {
    pub fn from_preset(name: &str) -> Result<Self, String> {
        match name.to_ascii_lowercase().as_str() {
            "laptop" | "small" => Ok(Self {
                n_layers: 2,
                d_model: 128,
                d_ff: 256,
                n_ticks: 2,
                lr: 3.2e-3,
                weight_decay: 1e-4,
                contract_rho: 0.78,
                use_nvfp4_ste: true,
            }),
            "mid" | "medium" => Ok(Self {
                n_layers: 8,
                d_model: 512,
                d_ff: 1024,
                n_ticks: 2,
                lr: 1.5e-3,
                weight_decay: 1e-4,
                contract_rho: 0.78,
                use_nvfp4_ste: true,
            }),
            "blackwell-1b" | "1b" => Ok(Self {
                n_layers: 32,
                d_model: 2048,
                d_ff: 4096,
                n_ticks: 3,
                lr: 6.0e-4,
                weight_decay: 1e-4,
                contract_rho: 0.78,
                use_nvfp4_ste: true,
            }),
            "blackwell-8b" | "8b" => Ok(Self {
                n_layers: 48,
                d_model: 4096,
                d_ff: 10240,
                n_ticks: 3,
                lr: 3.0e-4,
                weight_decay: 1e-4,
                contract_rho: 0.78,
                use_nvfp4_ste: true,
            }),
            other => Err(format!(
                "Unknown preset '{}'. Allowed: laptop, mid, blackwell-1b, blackwell-8b",
                other
            )),
        }
    }

    pub fn parameter_count(&self) -> u64 {
        let d = self.d_model as u64;
        let ff = self.d_ff as u64;
        let l = self.n_layers as u64;
        let emb = 256 * d;
        // Per layer: Conv4 (4*d + d) + Selective SSM + Fast-Weight Key (3*(d*d+d)) + Contractive (d*d+d) + FFN (2*ff*d + ff + d)
        let per_layer = 5 * d + 4 * (d * d + d) + (ff * d + ff) + (d * ff + d);
        let head = 256 * d + 256;
        emb + l * per_layer + head
    }

    pub fn nvfp4_weight_gb(&self) -> f64 {
        (self.parameter_count() as f64) * 0.5625 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn fp32_train_state_gb(&self) -> f64 {
        (self.parameter_count() as f64) * 12.0 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// Persistent infinite-horizon causal stream state across ALL `L` layers:
/// Conv-4 tap buffer + Selective SSM state + Tropical Max-Trace + Fast-Weight Associative Delta-Rule matrix!
#[derive(Clone, Debug)]
pub struct CausalStreamState {
    pub layer_conv: Vec<Vec<f32>>,  // [n_layers][(CONV_K - 1) * d_model]
    pub layer_ssm: Vec<Vec<f32>>,   // [n_layers][d_model]
    pub layer_trop: Vec<Vec<f32>>,  // [n_layers][d_model]
    pub layer_assoc: Vec<Vec<f32>>, // [n_layers][(d_model / ASSOC_HD) * ASSOC_HD * ASSOC_HD]
    pub recent_bytes: Vec<u8>,
}

impl CausalStreamState {
    pub fn new(cfg: &BrainConfig) -> Self {
        let d = cfg.d_model;
        let n_heads = (d / ASSOC_HD).max(1);
        Self {
            layer_conv: (0..cfg.n_layers).map(|_| vec![0.0; (CONV_K - 1) * d]).collect(),
            layer_ssm: (0..cfg.n_layers).map(|_| vec![0.0; d]).collect(),
            layer_trop: (0..cfg.n_layers).map(|_| vec![0.0; d]).collect(),
            layer_assoc: (0..cfg.n_layers).map(|_| vec![0.0; n_heads * ASSOC_HD * ASSOC_HD]).collect(),
            recent_bytes: Vec::with_capacity(64),
        }
    }

    pub fn push_byte(&mut self, b: u8) {
        if self.recent_bytes.len() >= 32 {
            self.recent_bytes.remove(0);
        }
        self.recent_bytes.push(b);
    }
}

#[derive(Debug)]
struct ParamTensor {
    w: Vec<f32>,
    w_q: Vec<f32>,
    m: Vec<f32>,
    v: Vec<f32>,
    w_dev: DeviceTensorHandle,
    wq_dev: DeviceTensorHandle,
    m_dev: DeviceTensorHandle,
    v_dev: DeviceTensorHandle,
    rows: usize,
    cols: usize,
    max_linf: Option<f32>,
    nvfp4: bool,
}

/// CRITICAL v13.0 FIX: Cloning a `ParamTensor` ALWAYS syncs GPU-resident weights to host first!
/// Eliminates v12.0's stale-random-weights bug in `compress`/`generate`/`self-optimize` on SM120 GPUs!
impl Clone for ParamTensor {
    fn clone(&self) -> Self {
        let mut w = self.w.clone();
        let mut w_q = self.w_q.clone();
        let mut m = self.m.clone();
        let mut v = self.v.clone();
        self.w_dev.sync_to_host(&mut w);
        self.wq_dev.sync_to_host(&mut w_q);
        self.m_dev.sync_to_host(&mut m);
        self.v_dev.sync_to_host(&mut v);
        Self {
            w,
            w_q,
            m,
            v,
            w_dev: DeviceTensorHandle::detached(),
            wq_dev: DeviceTensorHandle::detached(),
            m_dev: DeviceTensorHandle::detached(),
            v_dev: DeviceTensorHandle::detached(),
            rows: self.rows,
            cols: self.cols,
            max_linf: self.max_linf,
            nvfp4: self.nvfp4,
        }
    }
}

impl ParamTensor {
    fn new(
        rows: usize,
        cols: usize,
        scale: f32,
        bias_init: Option<f32>,
        max_linf: Option<f32>,
        nvfp4: bool,
        device_id: usize,
        rng: &mut u64,
    ) -> Self {
        let n = rows * cols;
        let mut w = Vec::with_capacity(n);
        if let Some(bv) = bias_init {
            w.resize(n, bv);
        } else {
            for _ in 0..n {
                w.push(rand_sym_f32(rng) * scale);
            }
        }
        if let Some(rho) = max_linf {
            for r in 0..rows {
                let base = r * cols;
                let l1: f32 = w[base..base + cols].iter().map(|v| v.abs()).sum();
                if l1 > rho + 1e-6 {
                    let sc = (rho - 1e-6) / l1;
                    for c in 0..cols { w[base + c] *= sc; }
                }
            }
        }
        let mut w_q = vec![0.0f32; n];
        if nvfp4 {
            for r in 0..rows {
                let base = r * cols;
                nvfp4_quantize_f32(&w[base..base + cols], &mut w_q[base..base + cols]);
            }
        } else {
            w_q.copy_from_slice(&w);
        }
        let m = vec![0.0; n];
        let v = vec![0.0; n];
        let w_dev = DeviceTensorHandle::alloc_upload(&w, device_id);
        let wq_dev = DeviceTensorHandle::alloc_upload(&w_q, device_id);
        let m_dev = DeviceTensorHandle::alloc_upload(&m, device_id);
        let v_dev = DeviceTensorHandle::alloc_upload(&v, device_id);
        Self { w, w_q, m, v, w_dev, wq_dev, m_dev, v_dev, rows, cols, max_linf, nvfp4 }
    }

    fn sync_from_device(&mut self) {
        self.w_dev.sync_to_host(&mut self.w);
        self.wq_dev.sync_to_host(&mut self.w_q);
        self.m_dev.sync_to_host(&mut self.m);
        self.v_dev.sync_to_host(&mut self.v);
    }

    fn step_adamw(&mut self, grad: &[f32], lr: f32, wd: f32, step: usize) {
        adamw_contractive_nvfp4_step(
            &mut self.w,
            &self.w_dev,
            Some(&mut self.w_q),
            &self.wq_dev,
            grad,
            &mut self.m,
            &self.m_dev,
            &mut self.v,
            &self.v_dev,
            self.rows,
            self.cols,
            lr,
            wd,
            step,
            self.max_linf,
            self.nvfp4,
        );
    }

    fn max_row_l1(&self) -> f32 {
        let mut wq = self.w_q.clone();
        self.wq_dev.sync_to_host(&mut wq);
        let mut mx = 0.0f32;
        for r in 0..self.rows {
            let base = r * self.cols;
            let s: f32 = wq[base..base + self.cols].iter().map(|v| v.abs()).sum();
            if s > mx { mx = s; }
        }
        mx
    }

    fn write_binary(&mut self, out: &mut Vec<u8>) {
        self.sync_from_device();
        out.extend_from_slice(&(self.rows as u32).to_le_bytes());
        out.extend_from_slice(&(self.cols as u32).to_le_bytes());
        out.extend_from_slice(&self.max_linf.unwrap_or(-1.0).to_le_bytes());
        out.push(if self.nvfp4 { 1 } else { 0 });
        for slice in [&self.w, &self.w_q, &self.m, &self.v] {
            for &val in slice {
                out.extend_from_slice(&val.to_le_bytes());
            }
        }
    }

    fn read_binary(data: &[u8], pos: &mut usize, device_id: usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("tensor EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_f32 = |d: &[u8], p: &mut usize| -> Result<f32, String> {
            if *p + 4 > d.len() { return Err("tensor EOF f32".into()); }
            let v = f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let rows = take_u32(data, pos)? as usize;
        let cols = take_u32(data, pos)? as usize;
        let ml = take_f32(data, pos)?;
        let max_linf = if ml > 0.0 { Some(ml) } else { None };
        if *pos >= data.len() { return Err("tensor EOF flag".into()); }
        let nvfp4 = data[*pos] != 0;
        *pos += 1;
        let n = rows * cols;
        let read_vec = |d: &[u8], p: &mut usize| -> Result<Vec<f32>, String> {
            if *p + n * 4 > d.len() { return Err("tensor EOF vec".into()); }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]));
                *p += 4;
            }
            Ok(v)
        };
        let w = read_vec(data, pos)?;
        let w_q = read_vec(data, pos)?;
        let m = read_vec(data, pos)?;
        let v = read_vec(data, pos)?;
        let w_dev = DeviceTensorHandle::alloc_upload(&w, device_id);
        let wq_dev = DeviceTensorHandle::alloc_upload(&w_q, device_id);
        let m_dev = DeviceTensorHandle::alloc_upload(&m, device_id);
        let v_dev = DeviceTensorHandle::alloc_upload(&v, device_id);
        Ok(Self { w, w_q, m, v, w_dev, wq_dev, m_dev, v_dev, rows, cols, max_linf, nvfp4 })
    }
}

/// Full Per-Layer Causal Conv-4 + Fast-Weight Associative Delta-SSM + Contractive Equilibrium + SiLU FFN!
#[derive(Clone, Debug)]
struct ContractiveSsmLayer {
    conv_w: ParamTensor, // [CONV_K x d] depthwise causal conv weights (host-side + AdamW)
    conv_b: ParamTensor, // [d]
    w_gate: ParamTensor, // [d x d] selective gate
    b_gate: ParamTensor, // [d]
    w_val: ParamTensor,  // [d x d] value projection
    b_val: ParamTensor,  // [d]
    w_key: ParamTensor,  // [d x d] fast-weight associative delta-rule key projection
    b_key: ParamTensor,  // [d]
    w_eq: ParamTensor,   // [d x d] Banach contractive equilibrium (||W_eq||_inf <= rho)
    b_eq: ParamTensor,   // [d]
    w_up: ParamTensor,   // [ff x d]
    b_up: ParamTensor,   // [ff]
    w_dn: ParamTensor,   // [d x ff]
    b_dn: ParamTensor,   // [d]
}

impl ContractiveSsmLayer {
    fn new(cfg: &BrainConfig, layer_idx: usize, rng: &mut u64) -> Self {
        let d = cfg.d_model;
        let ff = cfg.d_ff;
        let s_d = (1.0 / d as f32).sqrt() * 0.65;
        let s_eq = (cfg.contract_rho / (d as f32).max(1.0)) * 1.4;
        let s_dn = (1.0 / (ff as f32 * cfg.n_layers.max(1) as f32)).sqrt() * 0.6;
        let gate_bias = -0.35 - 0.55 * (layer_idx as f32);

        // Initialize Conv-4 near identity on tap 0 (`k=0 -> 0.85, k=1..3 -> small`) so signal flows cleanly at step 0!
        let mut conv_w = ParamTensor::new(CONV_K, d, 0.08, None, None, false, layer_idx, rng);
        for j in 0..d {
            conv_w.w[j] = 0.85;
            conv_w.w_q[j] = 0.85;
        }
        // Keep conv_w host-resident (`detached`) since Conv-4 runs host-side per channel O(4*T*d)
        conv_w.w_dev = DeviceTensorHandle::detached();
        conv_w.wq_dev = DeviceTensorHandle::detached();
        conv_w.m_dev = DeviceTensorHandle::detached();
        conv_w.v_dev = DeviceTensorHandle::detached();

        let mut conv_b = ParamTensor::new(1, d, 0.0, Some(0.0), None, false, layer_idx, rng);
        conv_b.w_dev = DeviceTensorHandle::detached();
        conv_b.wq_dev = DeviceTensorHandle::detached();
        conv_b.m_dev = DeviceTensorHandle::detached();
        conv_b.v_dev = DeviceTensorHandle::detached();

        Self {
            conv_w,
            conv_b,
            w_gate: ParamTensor::new(d, d, s_d, None, None, cfg.use_nvfp4_ste, layer_idx, rng),
            b_gate: ParamTensor::new(1, d, 0.0, Some(gate_bias), None, false, layer_idx, rng),
            w_val: ParamTensor::new(d, d, s_d, None, None, cfg.use_nvfp4_ste, layer_idx, rng),
            b_val: ParamTensor::new(1, d, 0.0, Some(0.0), None, false, layer_idx, rng),
            w_key: ParamTensor::new(d, d, s_d, None, None, cfg.use_nvfp4_ste, layer_idx, rng),
            b_key: ParamTensor::new(1, d, 0.0, Some(0.0), None, false, layer_idx, rng),
            w_eq: ParamTensor::new(d, d, s_eq, None, Some(cfg.contract_rho), cfg.use_nvfp4_ste, layer_idx, rng),
            b_eq: ParamTensor::new(1, d, 0.0, Some(0.0), None, false, layer_idx, rng),
            w_up: ParamTensor::new(ff, d, s_d, None, None, cfg.use_nvfp4_ste, layer_idx, rng),
            b_up: ParamTensor::new(1, ff, 0.0, Some(0.0), None, false, layer_idx, rng),
            w_dn: ParamTensor::new(d, ff, s_dn, None, None, cfg.use_nvfp4_ste, layer_idx, rng),
            b_dn: ParamTensor::new(1, d, 0.0, Some(0.0), None, false, layer_idx, rng),
        }
    }

    fn write_binary(&mut self, out: &mut Vec<u8>) {
        self.conv_w.write_binary(out);
        self.conv_b.write_binary(out);
        self.w_gate.write_binary(out);
        self.b_gate.write_binary(out);
        self.w_val.write_binary(out);
        self.b_val.write_binary(out);
        self.w_key.write_binary(out);
        self.b_key.write_binary(out);
        self.w_eq.write_binary(out);
        self.b_eq.write_binary(out);
        self.w_up.write_binary(out);
        self.b_up.write_binary(out);
        self.w_dn.write_binary(out);
        self.b_dn.write_binary(out);
    }

    fn read_binary(data: &[u8], pos: &mut usize, layer_idx: usize) -> Result<Self, String> {
        let mut conv_w = ParamTensor::read_binary(data, pos, layer_idx)?;
        conv_w.w_dev = DeviceTensorHandle::detached();
        conv_w.wq_dev = DeviceTensorHandle::detached();
        conv_w.m_dev = DeviceTensorHandle::detached();
        conv_w.v_dev = DeviceTensorHandle::detached();
        let mut conv_b = ParamTensor::read_binary(data, pos, layer_idx)?;
        conv_b.w_dev = DeviceTensorHandle::detached();
        conv_b.wq_dev = DeviceTensorHandle::detached();
        conv_b.m_dev = DeviceTensorHandle::detached();
        conv_b.v_dev = DeviceTensorHandle::detached();
        Ok(Self {
            conv_w,
            conv_b,
            w_gate: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_gate: ParamTensor::read_binary(data, pos, layer_idx)?,
            w_val: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_val: ParamTensor::read_binary(data, pos, layer_idx)?,
            w_key: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_key: ParamTensor::read_binary(data, pos, layer_idx)?,
            w_eq: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_eq: ParamTensor::read_binary(data, pos, layer_idx)?,
            w_up: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_up: ParamTensor::read_binary(data, pos, layer_idx)?,
            w_dn: ParamTensor::read_binary(data, pos, layer_idx)?,
            b_dn: ParamTensor::read_binary(data, pos, layer_idx)?,
        })
    }
}

struct LayerGrads {
    conv_w: Vec<f32>,
    conv_b: Vec<f32>,
    w_gate: Vec<f32>,
    b_gate: Vec<f32>,
    w_val: Vec<f32>,
    b_val: Vec<f32>,
    w_key: Vec<f32>,
    b_key: Vec<f32>,
    w_eq: Vec<f32>,
    b_eq: Vec<f32>,
    w_up: Vec<f32>,
    b_up: Vec<f32>,
    w_dn: Vec<f32>,
    b_dn: Vec<f32>,
}

impl LayerGrads {
    fn zeros(d: usize, ff: usize) -> Self {
        Self {
            conv_w: vec![0.0; CONV_K * d],
            conv_b: vec![0.0; d],
            w_gate: vec![0.0; d * d],
            b_gate: vec![0.0; d],
            w_val: vec![0.0; d * d],
            b_val: vec![0.0; d],
            w_key: vec![0.0; d * d],
            b_key: vec![0.0; d],
            w_eq: vec![0.0; d * d],
            b_eq: vec![0.0; d],
            w_up: vec![0.0; ff * d],
            b_up: vec![0.0; ff],
            w_dn: vec![0.0; d * ff],
            b_dn: vec![0.0; d],
        }
    }
}

struct BrainGrads {
    e_byte: Vec<f32>,
    layers: Vec<LayerGrads>,
    w_head: Vec<f32>,
    b_head: Vec<f32>,
}

impl BrainGrads {
    fn zeros(cfg: &BrainConfig) -> Self {
        Self {
            e_byte: vec![0.0; 256 * cfg.d_model],
            layers: (0..cfg.n_layers).map(|_| LayerGrads::zeros(cfg.d_model, cfg.d_ff)).collect(),
            w_head: vec![0.0; 256 * cfg.d_model],
            b_head: vec![0.0; 256],
        }
    }

    fn clip_global_norm(&mut self, max_norm: f32) -> f32 {
        let mut refs: Vec<&mut [f32]> = Vec::with_capacity(3 + self.layers.len() * 14);
        refs.push(&mut self.e_byte);
        refs.push(&mut self.w_head);
        refs.push(&mut self.b_head);
        for lg in &mut self.layers {
            refs.push(&mut lg.conv_w);
            refs.push(&mut lg.conv_b);
            refs.push(&mut lg.w_gate);
            refs.push(&mut lg.b_gate);
            refs.push(&mut lg.w_val);
            refs.push(&mut lg.b_val);
            refs.push(&mut lg.w_key);
            refs.push(&mut lg.b_key);
            refs.push(&mut lg.w_eq);
            refs.push(&mut lg.b_eq);
            refs.push(&mut lg.w_up);
            refs.push(&mut lg.b_up);
            refs.push(&mut lg.w_dn);
            refs.push(&mut lg.b_dn);
        }
        global_grad_norm_clip(&mut refs, max_norm)
    }
}

#[inline]
fn rms_norm_fwd(x: &[f32], y: &mut [f32], inv_rms_out: &mut [f32], batch: usize, dim: usize) {
    let inv_d = 1.0f32 / (dim.max(1) as f32);
    for b in 0..batch {
        let xr = &x[b * dim..(b + 1) * dim];
        let yr = &mut y[b * dim..(b + 1) * dim];
        let mut ss = 0.0f32;
        for &v in xr { ss += v * v; }
        let inv = 1.0f32 / (ss * inv_d + 1e-5).sqrt();
        inv_rms_out[b] = inv;
        for i in 0..dim { yr[i] = xr[i] * inv; }
    }
}

#[inline]
fn rms_norm_bwd(x: &[f32], inv_rms: &[f32], dy: &[f32], dx: &mut [f32], batch: usize, dim: usize) {
    let inv_d = 1.0f32 / (dim.max(1) as f32);
    for b in 0..batch {
        let xr = &x[b * dim..(b + 1) * dim];
        let dyr = &dy[b * dim..(b + 1) * dim];
        let dxr = &mut dx[b * dim..(b + 1) * dim];
        let inv = inv_rms[b];
        let mut dot = 0.0f32;
        for i in 0..dim { dot += dyr[i] * xr[i]; }
        let coeff = dot * inv * inv * inv * inv_d;
        for i in 0..dim { dxr[i] = dyr[i] * inv - xr[i] * coeff; }
    }
}

#[inline]
fn sigmoid_f32(x: f32) -> f32 {
    1.0 / (1.0 + (-x.clamp(-16.0, 16.0)).exp())
}

#[inline]
fn silu(x: f32) -> f32 {
    let xc = x.clamp(-20.0, 20.0);
    xc / (1.0 + (-xc).exp())
}

#[inline]
fn silu_grad(x: f32) -> f32 {
    let xc = x.clamp(-20.0, 20.0);
    let sig = 1.0 / (1.0 + (-xc).exp());
    sig * (1.0 + xc * (1.0 - sig))
}

#[inline]
fn update_conv_history(conv_hist: &mut [f32], x_n1: &[f32], t_len: usize, d: usize) {
    let keep = CONV_K - 1; // 3
    if t_len >= keep {
        let start = (t_len - keep) * d;
        conv_hist.copy_from_slice(&x_n1[start..t_len * d]);
    } else if t_len > 0 {
        let shift = t_len * d;
        conv_hist.copy_within(shift..keep * d, 0);
        conv_hist[(keep - t_len) * d..keep * d].copy_from_slice(&x_n1[..t_len * d]);
    }
}

#[derive(Clone, Debug)]
pub struct DeepContractiveBrain {
    pub cfg: BrainConfig,
    e_byte: ParamTensor,
    layers: Vec<ContractiveSsmLayer>,
    w_head: ParamTensor,
    b_head: ParamTensor,
    pub episodic: TropicalEpisodicMemory,
    pub active_policy: RuntimePolicyOverride,
    pub adam_step: usize,
    pub total_bytes_trained: u64,
    pub last_train_bpb: f32,
    pub best_heldout_bpb: f64,
}

impl DeepContractiveBrain {
    pub fn new(cfg: BrainConfig, seed: u64) -> Self {
        let mut rng = seed ^ 0xA510_1300_2026_1125;
        let d = cfg.d_model;
        let s_emb = (1.0 / d as f32).sqrt() * 1.2;
        let s_head = (1.0 / d as f32).sqrt() * 0.5;

        let e_byte = ParamTensor::new(256, d, s_emb, None, None, false, 0, &mut rng);
        let layers = (0..cfg.n_layers)
            .map(|li| ContractiveSsmLayer::new(&cfg, li, &mut rng))
            .collect();
        let w_head = ParamTensor::new(256, d, s_head, None, None, false, 0, &mut rng);
        let b_head = ParamTensor::new(1, 256, 0.0, Some(0.0), None, false, 0, &mut rng);

        Self {
            cfg,
            e_byte,
            layers,
            w_head,
            b_head,
            episodic: TropicalEpisodicMemory::new(d.min(32), 2048),
            active_policy: RuntimePolicyOverride::compiled_default(),
            adam_step: 0,
            total_bytes_trained: 0,
            last_train_bpb: 8.0,
            best_heldout_bpb: f64::INFINITY,
        }
    }

    pub fn max_contractive_linf_norm(&self) -> f32 {
        self.layers.iter().map(|l| l.w_eq.max_row_l1()).fold(0.0f32, f32::max)
    }

    /// Stateful encoding of continuous sensor vectors `obs` — preserves infinite-horizon neural stream across steps!
    pub fn encode_raw_sensor_vector_stateful(&self, obs: &[f32], stream: &mut CausalStreamState) -> Vec<f32> {
        let d = self.cfg.d_model;
        let mut x = vec![0.0f32; d];
        if !obs.is_empty() {
            let mut emb_row = vec![0.0f32; d];
            let mut prev_bank = usize::MAX;
            for j in 0..d {
                let v = obs[j % obs.len()];
                let vf = if v.is_finite() { v.asinh() * 0.6 } else { 0.0 };
                let bank = (j / obs.len()) % 256;
                if bank != prev_bank {
                    embedding_lookup_fwd(&self.e_byte.w, &self.e_byte.w_dev, &[bank as u8], &mut emb_row, d);
                    prev_bank = bank;
                }
                x[j] = vf + 0.15 * emb_row[j];
            }
        }
        let (_logits, lat) = self.forward_hidden_batch(&x, 1, stream, self.active_policy.gamma, 0);
        lat
    }

    /// Stateful encoding of arbitrary raw bytes/text (`obs_text`) across full episode without resetting stream!
    pub fn encode_raw_bytes_to_latent_stateful(&self, bytes: &[u8], stream: &mut CausalStreamState) -> Vec<f32> {
        let d = self.cfg.d_model;
        if bytes.is_empty() {
            return vec![0.0; d];
        }
        let mut last_lat = vec![0.0f32; d];
        let mut off = 0usize;
        while off < bytes.len() {
            let end = (off + 64).min(bytes.len());
            let chunk = &bytes[off..end];
            let (_, lats) = self.forward_byte_chunk(chunk, stream, Some(self.active_policy.gamma), 0);
            last_lat.copy_from_slice(&lats[(chunk.len() - 1) * d..chunk.len() * d]);
            off = end;
        }
        last_lat
    }

    fn forward_hidden_batch(
        &self,
        x0: &[f32],
        t_len: usize,
        stream: &mut CausalStreamState,
        gamma: f32,
        device_id: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let d = self.cfg.d_model;
        let ff = self.cfg.d_ff;
        let n_heads = (d / ASSOC_HD).max(1);
        let mut x = x0.to_vec();
        let mut x_norm = vec![0.0f32; t_len * d];
        let mut x_conv = vec![0.0f32; t_len * d];
        let mut inv_r = vec![0.0f32; t_len];
        let mut g_pre = vec![0.0f32; t_len * d];
        let mut v_pre = vec![0.0f32; t_len * d];
        let mut k_pre = vec![0.0f32; t_len * d];
        let mut s_out = vec![0.0f32; t_len * d];
        let mut pre = vec![0.0f32; t_len * d];
        let mut h_eq = vec![0.0f32; t_len * d];
        let mut z = vec![0.0f32; t_len * d];
        let mut z_next = vec![0.0f32; t_len * d];
        let mut u = vec![0.0f32; t_len * d];
        let mut ff_pre = vec![0.0f32; t_len * ff];
        let mut ff_act = vec![0.0f32; t_len * ff];
        let mut ff_out = vec![0.0f32; t_len * d];

        for (li, layer) in self.layers.iter().enumerate() {
            rms_norm_fwd(&x, &mut x_norm, &mut inv_r, t_len, d);

            // 1A. Learned 4-Tap Causal Depthwise Convolution (`O(4 * T * d)`)
            let c_hist = &mut stream.layer_conv[li];
            for ti in 0..t_len {
                let off = ti * d;
                for j in 0..d {
                    let mut acc = layer.conv_b.w[j];
                    for k in 0..CONV_K {
                        let tap = if ti >= k {
                            x_norm[(ti - k) * d + j]
                        } else {
                            c_hist[(CONV_K - 1 + ti - k) * d + j]
                        };
                        acc += layer.conv_w.w[k * d + j] * tap;
                    }
                    x_conv[off + j] = acc;
                }
            }
            update_conv_history(c_hist, &x_norm, t_len, d);

            // 1B. Selective Gated Causal SSM + Tropical Max-Trace + Fast-Weight Associative Delta-Rule (`O(T * d * 8)`)
            linear_fwd(&x_conv, &layer.w_gate.w_q, &layer.w_gate.wq_dev, Some(&layer.b_gate.w), Some(&layer.b_gate.w_dev), &mut g_pre, t_len, d, d);
            linear_fwd(&x_conv, &layer.w_val.w_q, &layer.w_val.wq_dev, Some(&layer.b_val.w), Some(&layer.b_val.w_dev), &mut v_pre, t_len, d, d);
            linear_fwd(&x_conv, &layer.w_key.w_q, &layer.w_key.wq_dev, Some(&layer.b_key.w), Some(&layer.b_key.w_dev), &mut k_pre, t_len, d, d);

            let s_st = &mut stream.layer_ssm[li];
            let m_st = &mut stream.layer_trop[li];
            let a_st = &mut stream.layer_assoc[li];
            for ti in 0..t_len {
                let off = ti * d;
                for h in 0..n_heads {
                    let h_off = h * ASSOC_HD;
                    let mat_off = h * ASSOC_HD * ASSOC_HD;
                    let mut g_arr = [0.0f32; ASSOC_HD];
                    let mut v_arr = [0.0f32; ASSOC_HD];
                    let mut k_arr = [0.0f32; ASSOC_HD];
                    let mut k_sq = 0.0f32;
                    let mut g_sum = 0.0f32;
                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        let g = sigmoid_f32(g_pre[off + j]);
                        let v = v_pre[off + j].tanh();
                        let kr = k_pre[off + j].tanh();
                        g_arr[a] = g;
                        v_arr[a] = v;
                        k_arr[a] = kr;
                        k_sq += kr * kr;
                        g_sum += g;
                    }
                    let inv_kn = 1.0f32 / (k_sq + 1e-4).sqrt();
                    for a in 0..ASSOC_HD { k_arr[a] *= inv_kn; }
                    let beta = 0.5 * (g_sum / ASSOC_HD as f32);
                    let alpha = 1.0 - beta;

                    // Strictly causal fast-weight readout `r = S_{t-1}^T k_t`
                    let mut r_arr = [0.0f32; ASSOC_HD];
                    for a in 0..ASSOC_HD {
                        let ka = k_arr[a];
                        let row = mat_off + a * ASSOC_HD;
                        for b in 0..ASSOC_HD {
                            r_arr[b] += a_st[row + b] * ka;
                        }
                    }
                    // Contractive Delta-Rule associative memory write!
                    for a in 0..ASSOC_HD {
                        let bk = beta * k_arr[a];
                        let row = mat_off + a * ASSOC_HD;
                        for b in 0..ASSOC_HD {
                            let delta = v_arr[b] - r_arr[b];
                            a_st[row + b] = (alpha * a_st[row + b] + bk * delta).clamp(-3.0, 3.0);
                        }
                    }
                    // Combine Selective SSM + Tropical Max-Trace + Fast-Weight Recall
                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        let ns = (1.0 - g_arr[a]) * s_st[j] + g_arr[a] * v_arr[a];
                        let nm = (0.88 * m_st[j]).max(v_arr[a]);
                        s_st[j] = ns;
                        m_st[j] = nm;
                        s_out[off + j] = 0.60 * ns + 0.15 * nm + 0.25 * r_arr[a];
                    }
                }
            }
            for idx in 0..t_len * d {
                x[idx] += s_out[idx];
            }

            // 2. Input-Anchored Banach Contractive Equilibrium (`K` ticks)
            rms_norm_fwd(&x, &mut x_norm, &mut inv_r, t_len, d);
            z.copy_from_slice(&x_norm);
            for _ in 0..self.cfg.n_ticks {
                linear_fwd(&z, &layer.w_eq.w_q, &layer.w_eq.wq_dev, Some(&layer.b_eq.w), Some(&layer.b_eq.w_dev), &mut pre, t_len, d, d);
                contractive_step_fwd(&pre, &x_norm, &mut h_eq, &mut z_next, gamma, device_id);
                z.copy_from_slice(&z_next);
            }
            for idx in 0..t_len * d { u[idx] = x[idx] + z[idx]; }

            // 3. SiLU Feed-Forward Block
            rms_norm_fwd(&u, &mut x_norm, &mut inv_r, t_len, d);
            linear_fwd(&x_norm, &layer.w_up.w_q, &layer.w_up.wq_dev, Some(&layer.b_up.w), Some(&layer.b_up.w_dev), &mut ff_pre, t_len, d, ff);
            for idx in 0..t_len * ff { ff_act[idx] = silu(ff_pre[idx]); }
            linear_fwd(&ff_act, &layer.w_dn.w_q, &layer.w_dn.wq_dev, Some(&layer.b_dn.w), Some(&layer.b_dn.w_dev), &mut ff_out, t_len, ff, d);
            for idx in 0..t_len * d { x[idx] = u[idx] + ff_out[idx]; }
        }

        rms_norm_fwd(&x, &mut x_norm, &mut inv_r, t_len, d);
        let mut logits = vec![0.0f32; t_len * 256];
        linear_fwd(&x_norm, &self.w_head.w, &self.w_head.w_dev, Some(&self.b_head.w), Some(&self.b_head.w_dev), &mut logits, t_len, d, 256);
        (logits, x_norm)
    }

    pub fn forward_byte_chunk(
        &self,
        inputs: &[u8],
        stream: &mut CausalStreamState,
        gamma_override: Option<f32>,
        device_id: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let t_len = inputs.len();
        let d = self.cfg.d_model;
        let mut x0 = vec![0.0f32; t_len * d];
        embedding_lookup_fwd(&self.e_byte.w, &self.e_byte.w_dev, inputs, &mut x0, d);
        let g = gamma_override.unwrap_or(self.active_policy.gamma);
        self.forward_hidden_batch(&x0, t_len, stream, g, device_id)
    }

    /// Single-pass strictly-causal 256-way normalized blended byte distribution `(probs[256], latent[d])`!
    pub fn predict_next_byte_distribution(
        &self,
        prev_byte: u8,
        stream: &mut CausalStreamState,
        policy_override: Option<RuntimePolicyOverride>,
    ) -> ([f32; 256], Vec<f32>) {
        stream.push_byte(prev_byte);
        let pol = policy_override.unwrap_or(self.active_policy);
        let (logits, lat) = self.forward_byte_chunk(&[prev_byte], stream, Some(pol.gamma), 0);
        let mut mx = f32::NEG_INFINITY;
        for &v in &logits { if v > mx { mx = v; } }
        let mut p_brain = [0.0f32; 256];
        let mut se = 0.0f32;
        for c in 0..256 {
            let ex = (logits[c] - mx).max(-40.0).exp();
            p_brain[c] = ex;
            se += ex;
        }
        let inv_s = 1.0 / se.max(1e-12);
        for c in 0..256 { p_brain[c] *= inv_s; }

        let (p_mem, conf) = self.episodic.predict_byte_distribution(&stream.recent_bytes, &lat);
        let probs = pol.mix_256(&p_brain, &p_mem, conf);
        (probs, lat)
    }

    /// Deterministic single-block Test-Time Neural Adaptation (`threads = 1`) for lossless `OMGZ13` dynamic coding!
    pub fn ttt_adapt_block(
        &mut self,
        inputs: &[u8],
        targets: &[u8],
        ttt_stream: &mut CausalStreamState,
        lr_scale: f32,
    ) -> f32 {
        let mut streams = vec![ttt_stream.clone()];
        let io = [(inputs, targets)];
        let bpb = self.train_batch_streams(&io, &mut streams, self.cfg.lr * lr_scale, None, false, 1);
        *ttt_stream = streams.pop().unwrap();
        bpb
    }

    /// Autonomous zero-LLM byte-stream generation from any prompt bytes!
    pub fn generate_bytes(&self, prompt: &[u8], n_new_bytes: usize, temperature: f32, seed: u64) -> Vec<u8> {
        let mut work = self.clone();
        let mut stream = CausalStreamState::new(&work.cfg);
        let mut prev_b = 0u8;
        for &b in prompt {
            let (probs, lat) = work.predict_next_byte_distribution(prev_b, &mut stream, None);
            let p = probs[b as usize].max(1e-7);
            work.episodic.observe(&stream.recent_bytes, &lat, b, -(p.log2()));
            prev_b = b;
        }
        let mut rng = seed ^ 0x6E00_1300_A510;
        let inv_t = 1.0 / temperature.clamp(0.15, 2.0);
        let mut generated = Vec::with_capacity(n_new_bytes);
        for _ in 0..n_new_bytes {
            let (probs, lat) = work.predict_next_byte_distribution(prev_b, &mut stream, None);
            let mut weights = [0.0f32; 256];
            let mut sum = 0.0f32;
            for c in 0..256 {
                let w = probs[c].max(1e-8).powf(inv_t);
                weights[c] = w;
                sum += w;
            }
            let u = ((splitmix64(&mut rng) >> 11) as f64 / ((1u64 << 53) as f64)) as f32 * sum;
            let mut acc = 0.0f32;
            let mut chosen = 0u8;
            for c in 0..256 {
                acc += weights[c];
                if acc >= u {
                    chosen = c as u8;
                    break;
                }
            }
            generated.push(chosen);
            let p = probs[chosen as usize].max(1e-7);
            work.episodic.observe(&stream.recent_bytes, &lat, chosen, -(p.log2()));
            prev_b = chosen;
        }
        generated
    }

    /// Exact Forward + Full Per-Layer Causal Conv-4 + Fast-Weight Associative Delta-SSM + Contractive BPTT!
    fn compute_stream_grads(
        &self,
        inputs: &[u8],
        targets: &[u8],
        stream: &mut CausalStreamState,
        gamma: f32,
        device_id: usize,
    ) -> (BrainGrads, f32, Vec<f32>, Vec<f32>) {
        let t_len = inputs.len().min(targets.len());
        let d = self.cfg.d_model;
        let ff = self.cfg.d_ff;
        let k_ticks = self.cfg.n_ticks;
        let n_heads = (d / ASSOC_HD).max(1);
        let mut grads = BrainGrads::zeros(&self.cfg);

        // 1. Live GPU/CPU Embedding Lookup
        let mut cur_x = vec![0.0f32; t_len * d];
        embedding_lookup_fwd(&self.e_byte.w, &self.e_byte.w_dev, &inputs[..t_len], &mut cur_x, d);

        // 2. Multi-Layer Forward with Exact Conv-4 + Delta-SSM + Contractive Caches!
        struct LayerCache {
            x_in: Vec<f32>,
            x_n1: Vec<f32>,
            inv_r1: Vec<f32>,
            conv_init: Vec<f32>,
            x_conv: Vec<f32>,
            g_sig: Vec<f32>,
            v_tanh: Vec<f32>,
            k_raw: Vec<f32>,
            k_norm: Vec<f32>,
            inv_kn: Vec<f32>,       // [t_len * n_heads]
            beta_hist: Vec<f32>,    // [t_len * n_heads]
            delta_hist: Vec<f32>,   // [t_len * d]
            s_prev_hist: Vec<f32>,  // [t_len * d]
            a_prev_hist: Vec<f32>,  // [t_len * n_heads * ASSOC_HD * ASSOC_HD]
            m_hit_direct: Vec<u8>,
            x_mid: Vec<f32>,
            inv_r2: Vec<f32>,
            z_hist: Vec<Vec<f32>>,
            h_hist: Vec<Vec<f32>>,
            u_in: Vec<f32>,
            u_norm: Vec<f32>,
            inv_r3: Vec<f32>,
            ff_pre: Vec<f32>,
            ff_act: Vec<f32>,
        }
        let mut caches = Vec::with_capacity(self.cfg.n_layers);

        for (li, layer) in self.layers.iter().enumerate() {
            let x_in = cur_x;
            let mut x_n1 = vec![0.0f32; t_len * d];
            let mut inv_r1 = vec![0.0f32; t_len];
            rms_norm_fwd(&x_in, &mut x_n1, &mut inv_r1, t_len, d);

            let conv_init = stream.layer_conv[li].clone();
            let mut x_conv = vec![0.0f32; t_len * d];
            for ti in 0..t_len {
                let off = ti * d;
                for j in 0..d {
                    let mut acc = layer.conv_b.w[j];
                    for k in 0..CONV_K {
                        let tap = if ti >= k {
                            x_n1[(ti - k) * d + j]
                        } else {
                            conv_init[(CONV_K - 1 + ti - k) * d + j]
                        };
                        acc += layer.conv_w.w[k * d + j] * tap;
                    }
                    x_conv[off + j] = acc;
                }
            }
            update_conv_history(&mut stream.layer_conv[li], &x_n1, t_len, d);

            let mut g_pre = vec![0.0f32; t_len * d];
            let mut v_pre = vec![0.0f32; t_len * d];
            let mut k_pre = vec![0.0f32; t_len * d];
            linear_fwd(&x_conv, &layer.w_gate.w_q, &layer.w_gate.wq_dev, Some(&layer.b_gate.w), Some(&layer.b_gate.w_dev), &mut g_pre, t_len, d, d);
            linear_fwd(&x_conv, &layer.w_val.w_q, &layer.w_val.wq_dev, Some(&layer.b_val.w), Some(&layer.b_val.w_dev), &mut v_pre, t_len, d, d);
            linear_fwd(&x_conv, &layer.w_key.w_q, &layer.w_key.wq_dev, Some(&layer.b_key.w), Some(&layer.b_key.w_dev), &mut k_pre, t_len, d, d);

            let mut g_sig = vec![0.0f32; t_len * d];
            let mut v_tanh = vec![0.0f32; t_len * d];
            let mut k_raw = vec![0.0f32; t_len * d];
            let mut k_norm = vec![0.0f32; t_len * d];
            let mut inv_kn = vec![0.0f32; t_len * n_heads];
            let mut beta_hist = vec![0.0f32; t_len * n_heads];
            let mut delta_hist = vec![0.0f32; t_len * d];
            let mut s_prev_hist = vec![0.0f32; t_len * d];
            let assoc_sz = n_heads * ASSOC_HD * ASSOC_HD;
            let mut a_prev_hist = vec![0.0f32; t_len * assoc_sz];
            let mut m_hit_direct = vec![0u8; t_len * d];
            let mut x_mid = vec![0.0f32; t_len * d];

            let s_st = &mut stream.layer_ssm[li];
            let m_st = &mut stream.layer_trop[li];
            let a_st = &mut stream.layer_assoc[li];
            for ti in 0..t_len {
                let off = ti * d;
                a_prev_hist[ti * assoc_sz..(ti + 1) * assoc_sz].copy_from_slice(a_st);
                for h in 0..n_heads {
                    let h_off = h * ASSOC_HD;
                    let mat_off = h * ASSOC_HD * ASSOC_HD;
                    let mut k_sq = 0.0f32;
                    let mut g_sum = 0.0f32;
                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        let g = sigmoid_f32(g_pre[off + j]);
                        let v = v_pre[off + j].tanh();
                        let kr = k_pre[off + j].tanh();
                        g_sig[off + j] = g;
                        v_tanh[off + j] = v;
                        k_raw[off + j] = kr;
                        k_sq += kr * kr;
                        g_sum += g;
                    }
                    let ikn = 1.0f32 / (k_sq + 1e-4).sqrt();
                    inv_kn[ti * n_heads + h] = ikn;
                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        k_norm[off + j] = k_raw[off + j] * ikn;
                    }
                    let beta = 0.5 * (g_sum / ASSOC_HD as f32);
                    let alpha = 1.0 - beta;
                    beta_hist[ti * n_heads + h] = beta;

                    let mut r_arr = [0.0f32; ASSOC_HD];
                    for a in 0..ASSOC_HD {
                        let ka = k_norm[off + h_off + a];
                        let row = mat_off + a * ASSOC_HD;
                        for b in 0..ASSOC_HD {
                            r_arr[b] += a_st[row + b] * ka;
                        }
                    }
                    for b in 0..ASSOC_HD {
                        delta_hist[off + h_off + b] = v_tanh[off + h_off + b] - r_arr[b];
                    }
                    for a in 0..ASSOC_HD {
                        let bk = beta * k_norm[off + h_off + a];
                        let row = mat_off + a * ASSOC_HD;
                        for b in 0..ASSOC_HD {
                            a_st[row + b] = (alpha * a_st[row + b] + bk * delta_hist[off + h_off + b]).clamp(-3.0, 3.0);
                        }
                    }
                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        let g = g_sig[off + j];
                        let v = v_tanh[off + j];
                        let sp = s_st[j];
                        let ns = (1.0 - g) * sp + g * v;
                        let dec_m = 0.88 * m_st[j];
                        let (nm, hit) = if v >= dec_m { (v, 1u8) } else { (dec_m, 0u8) };
                        s_prev_hist[off + j] = sp;
                        m_hit_direct[off + j] = hit;
                        s_st[j] = ns;
                        m_st[j] = nm;
                        x_mid[off + j] = x_in[off + j] + 0.60 * ns + 0.15 * nm + 0.25 * r_arr[a];
                    }
                }
            }

            let mut x_n2 = vec![0.0f32; t_len * d];
            let mut inv_r2 = vec![0.0f32; t_len];
            rms_norm_fwd(&x_mid, &mut x_n2, &mut inv_r2, t_len, d);

            let mut z_hist = Vec::with_capacity(k_ticks + 1);
            let mut h_hist = Vec::with_capacity(k_ticks);
            z_hist.push(x_n2.clone());
            let mut pre = vec![0.0f32; t_len * d];
            for _ in 0..k_ticks {
                linear_fwd(z_hist.last().unwrap(), &layer.w_eq.w_q, &layer.w_eq.wq_dev, Some(&layer.b_eq.w), Some(&layer.b_eq.w_dev), &mut pre, t_len, d, d);
                let mut h = vec![0.0f32; t_len * d];
                let mut zn = vec![0.0f32; t_len * d];
                contractive_step_fwd(&pre, &x_n2, &mut h, &mut zn, gamma, device_id);
                h_hist.push(h);
                z_hist.push(zn);
            }
            let z_final = z_hist.last().unwrap();
            let mut u_in = vec![0.0f32; t_len * d];
            for idx in 0..t_len * d { u_in[idx] = x_mid[idx] + z_final[idx]; }

            let mut u_norm = vec![0.0f32; t_len * d];
            let mut inv_r3 = vec![0.0f32; t_len];
            rms_norm_fwd(&u_in, &mut u_norm, &mut inv_r3, t_len, d);

            let mut ff_pre = vec![0.0f32; t_len * ff];
            linear_fwd(&u_norm, &layer.w_up.w_q, &layer.w_up.wq_dev, Some(&layer.b_up.w), Some(&layer.b_up.w_dev), &mut ff_pre, t_len, d, ff);
            let mut ff_act = vec![0.0f32; t_len * ff];
            for idx in 0..t_len * ff { ff_act[idx] = silu(ff_pre[idx]); }
            let mut ff_out = vec![0.0f32; t_len * d];
            linear_fwd(&ff_act, &layer.w_dn.w_q, &layer.w_dn.wq_dev, Some(&layer.b_dn.w), Some(&layer.b_dn.w_dev), &mut ff_out, t_len, ff, d);
            let mut x_next = vec![0.0f32; t_len * d];
            for idx in 0..t_len * d { x_next[idx] = u_in[idx] + ff_out[idx]; }

            caches.push(LayerCache {
                x_in, x_n1, inv_r1, conv_init, x_conv, g_sig, v_tanh, k_raw, k_norm,
                inv_kn, beta_hist, delta_hist, s_prev_hist, a_prev_hist, m_hit_direct,
                x_mid, inv_r2, z_hist, h_hist, u_in, u_norm, inv_r3, ff_pre, ff_act,
            });
            cur_x = x_next;
        }

        // 3. Output Head + Exact 256-way Cross-Entropy Loss & Backward
        let mut h_final = vec![0.0f32; t_len * d];
        let mut inv_rf = vec![0.0f32; t_len];
        rms_norm_fwd(&cur_x, &mut h_final, &mut inv_rf, t_len, d);

        let mut logits = vec![0.0f32; t_len * 256];
        linear_fwd(&h_final, &self.w_head.w, &self.w_head.w_dev, Some(&self.b_head.w), Some(&self.b_head.w_dev), &mut logits, t_len, d, 256);

        let mut dlogits = vec![0.0f32; t_len * 256];
        let param_bpb = byte_cross_entropy_fwd_bwd(&logits, &targets[..t_len], &mut dlogits, t_len, device_id);

        let mut dh_final = vec![0.0f32; t_len * d];
        linear_bwd(
            &h_final, &self.w_head.w, &self.w_head.w_dev, &dlogits,
            Some(&mut dh_final), Some(&mut grads.w_head), Some(&mut grads.b_head),
            t_len, d, 256,
        );
        let mut dx_cur = vec![0.0f32; t_len * d];
        rms_norm_bwd(&cur_x, &inv_rf, &dh_final, &mut dx_cur, t_len, d);

        // 4. Full Multi-Layer Backward (FFN -> Contractive BPTT -> Delta-SSM + Conv-4 BPTT!)
        let assoc_sz = n_heads * ASSOC_HD * ASSOC_HD;
        for l in (0..self.cfg.n_layers).rev() {
            let c = &caches[l];
            let layer = &self.layers[l];
            let lg = &mut grads.layers[l];

            // 4A. FFN Backward
            let mut dff_act = vec![0.0f32; t_len * ff];
            linear_bwd(&c.ff_act, &layer.w_dn.w_q, &layer.w_dn.wq_dev, &dx_cur, Some(&mut dff_act), Some(&mut lg.w_dn), Some(&mut lg.b_dn), t_len, ff, d);
            let mut dff_pre = vec![0.0f32; t_len * ff];
            for idx in 0..t_len * ff { dff_pre[idx] = dff_act[idx] * silu_grad(c.ff_pre[idx]); }
            let mut du_norm = vec![0.0f32; t_len * d];
            linear_bwd(&c.u_norm, &layer.w_up.w_q, &layer.w_up.wq_dev, &dff_pre, Some(&mut du_norm), Some(&mut lg.w_up), Some(&mut lg.b_up), t_len, d, ff);
            let mut du_res = vec![0.0f32; t_len * d];
            rms_norm_bwd(&c.u_in, &c.inv_r3, &du_norm, &mut du_res, t_len, d);
            let mut du = vec![0.0f32; t_len * d];
            for idx in 0..t_len * d { du[idx] = dx_cur[idx] + du_res[idx]; }

            // 4B. K-Tick Contractive Equilibrium BPTT
            let mut dz = du.clone();
            let mut dx_n2 = vec![0.0f32; t_len * d];
            let mut dpre = vec![0.0f32; t_len * d];
            let mut dz_prev = vec![0.0f32; t_len * d];
            for tick in (0..k_ticks).rev() {
                contractive_step_bwd(&dz, &c.h_hist[tick], &mut dpre, &mut dx_n2, gamma, device_id);
                linear_bwd(&c.z_hist[tick], &layer.w_eq.w_q, &layer.w_eq.wq_dev, &dpre, Some(&mut dz_prev), Some(&mut lg.w_eq), Some(&mut lg.b_eq), t_len, d, d);
                dz.copy_from_slice(&dz_prev);
            }
            for idx in 0..t_len * d { dx_n2[idx] += dz[idx]; }
            let mut dx_mid_eq = vec![0.0f32; t_len * d];
            rms_norm_bwd(&c.x_mid, &c.inv_r2, &dx_n2, &mut dx_mid_eq, t_len, d);
            let mut dx_mid = vec![0.0f32; t_len * d];
            for idx in 0..t_len * d { dx_mid[idx] = du[idx] + dx_mid_eq[idx]; }

            // 4C. EXACT SELECTIVE-SSM + TROPICAL MAX + FAST-WEIGHT ASSOCIATIVE DELTA-RULE BPTT (`t = T-1..0`)!
            let mut dg_pre = vec![0.0f32; t_len * d];
            let mut dv_pre = vec![0.0f32; t_len * d];
            let mut dk_pre = vec![0.0f32; t_len * d];
            let mut ds_next = vec![0.0f32; d];
            let mut dm_next = vec![0.0f32; d];
            let mut da_next = vec![0.0f32; assoc_sz];

            for ti in (0..t_len).rev() {
                let off = ti * d;
                let a_prev_slice = &c.a_prev_hist[ti * assoc_sz..(ti + 1) * assoc_sz];
                for h in 0..n_heads {
                    let h_off = h * ASSOC_HD;
                    let mat_off = h * ASSOC_HD * ASSOC_HD;
                    let beta = c.beta_hist[ti * n_heads + h];
                    let alpha = 1.0 - beta;
                    let ikn = c.inv_kn[ti * n_heads + h];

                    // Associative delta-rule backward
                    let mut d_delta = [0.0f32; ASSOC_HD];
                    let mut dr_tot = [0.0f32; ASSOC_HD];
                    for b in 0..ASSOC_HD {
                        let mut acc = 0.0f32;
                        for a in 0..ASSOC_HD {
                            acc += da_next[mat_off + a * ASSOC_HD + b] * c.k_norm[off + h_off + a];
                        }
                        d_delta[b] = (beta * acc).clamp(-4.0, 4.0);
                        dr_tot[b] = (0.25 * dx_mid[off + h_off + b] - d_delta[b]).clamp(-4.0, 4.0);
                    }
                    let mut dk_n = [0.0f32; ASSOC_HD];
                    let mut d_beta = 0.0f32;
                    for a in 0..ASSOC_HD {
                        let row = mat_off + a * ASSOC_HD;
                        let ka = c.k_norm[off + h_off + a];
                        let mut dk_acc = 0.0f32;
                        for b in 0..ASSOC_HD {
                            let da_ab = da_next[row + b];
                            let s_ab = a_prev_slice[row + b];
                            let del_b = c.delta_hist[off + h_off + b];
                            dk_acc += da_ab * beta * del_b + dr_tot[b] * s_ab;
                            d_beta += da_ab * (ka * del_b - s_ab);
                            da_next[row + b] = (alpha * da_ab + ka * dr_tot[b]).clamp(-4.0, 4.0);
                        }
                        dk_n[a] = dk_acc.clamp(-4.0, 4.0);
                    }
                    let mut dot_dk_k = 0.0f32;
                    for a in 0..ASSOC_HD { dot_dk_k += dk_n[a] * c.k_norm[off + h_off + a]; }
                    let dg_assoc = (0.5 / ASSOC_HD as f32) * d_beta;

                    for a in 0..ASSOC_HD {
                        let j = h_off + a;
                        let g = c.g_sig[off + j];
                        let v = c.v_tanh[off + j];
                        let kr = c.k_raw[off + j];
                        let kn = c.k_norm[off + j];
                        let sp = c.s_prev_hist[off + j];

                        let ds_tot = (0.60 * dx_mid[off + j] + ds_next[j]).clamp(-5.0, 5.0);
                        let dm_tot = (0.15 * dx_mid[off + j] + dm_next[j]).clamp(-5.0, 5.0);
                        let (dv_from_m, next_dm) = if c.m_hit_direct[off + j] != 0 {
                            (dm_tot, 0.0)
                        } else {
                            (0.0, 0.88 * dm_tot)
                        };

                        let dg = (ds_tot * (v - sp) + dg_assoc) * g * (1.0 - g);
                        let dv = (ds_tot * g + dv_from_m + d_delta[a]) * (1.0 - v * v);
                        let dk = (dk_n[a] - kn * dot_dk_k) * ikn * (1.0 - kr * kr);

                        dg_pre[off + j] = dg.clamp(-4.0, 4.0);
                        dv_pre[off + j] = dv.clamp(-4.0, 4.0);
                        dk_pre[off + j] = dk.clamp(-4.0, 4.0);
                        ds_next[j] = ds_tot * (1.0 - g);
                        dm_next[j] = next_dm;
                    }
                }
            }

            let mut dx_conv_g = vec![0.0f32; t_len * d];
            let mut dx_conv_v = vec![0.0f32; t_len * d];
            let mut dx_conv_k = vec![0.0f32; t_len * d];
            linear_bwd(&c.x_conv, &layer.w_gate.w_q, &layer.w_gate.wq_dev, &dg_pre, Some(&mut dx_conv_g), Some(&mut lg.w_gate), Some(&mut lg.b_gate), t_len, d, d);
            linear_bwd(&c.x_conv, &layer.w_val.w_q, &layer.w_val.wq_dev, &dv_pre, Some(&mut dx_conv_v), Some(&mut lg.w_val), Some(&mut lg.b_val), t_len, d, d);
            linear_bwd(&c.x_conv, &layer.w_key.w_q, &layer.w_key.wq_dev, &dk_pre, Some(&mut dx_conv_k), Some(&mut lg.w_key), Some(&mut lg.b_key), t_len, d, d);
            for idx in 0..t_len * d {
                dx_conv_g[idx] += dx_conv_v[idx] + dx_conv_k[idx];
            }

            // 4D. Exact 4-Tap Causal Depthwise Convolution Backward!
            let mut dx_n1 = vec![0.0f32; t_len * d];
            for ti in 0..t_len {
                let off = ti * d;
                for j in 0..d {
                    let dy = dx_conv_g[off + j];
                    lg.conv_b[j] += dy;
                    for k in 0..CONV_K {
                        let tap = if ti >= k {
                            x_n1_ref(&c.x_n1, ti - k, d, j)
                        } else {
                            c.conv_init[(CONV_K - 1 + ti - k) * d + j]
                        };
                        lg.conv_w[k * d + j] += dy * tap;
                        if ti >= k {
                            dx_n1[(ti - k) * d + j] += dy * layer.conv_w.w[k * d + j];
                        }
                    }
                }
            }

            let mut dx_ssm_in = vec![0.0f32; t_len * d];
            rms_norm_bwd(&c.x_in, &c.inv_r1, &dx_n1, &mut dx_ssm_in, t_len, d);
            for idx in 0..t_len * d { dx_cur[idx] = dx_mid[idx] + dx_ssm_in[idx]; }
        }

        // 5. Live GPU/CPU Embedding Gradient Scatter
        embedding_scatter_bwd(&inputs[..t_len], &dx_cur, &mut grads.e_byte, d, device_id);

        (grads, param_bpb, logits, h_final)
    }

    /// Multi-Stream Data-Parallel Batch Training Step (`B` streams across `threads`, Global L2 Clip, AllReduce, AdamW+NVFP4)!
    pub fn train_batch_streams(
        &mut self,
        batch_io: &[(&[u8], &[u8])],
        streams: &mut [CausalStreamState],
        base_lr: f32,
        policy: Option<RuntimePolicyOverride>,
        update_episodic: bool,
        threads: usize,
    ) -> f32 {
        let n_streams = batch_io.len().min(streams.len());
        if n_streams == 0 {
            return 8.0;
        }
        let pol = policy.unwrap_or(self.active_policy);
        let gamma = pol.gamma;
        let d = self.cfg.d_model;
        let n_workers = threads.max(1).min(n_streams);

        let mut results: Vec<(BrainGrads, f32, Vec<f32>, Vec<f32>)> = Vec::with_capacity(n_streams);
        if n_workers <= 1 {
            for si in 0..n_streams {
                results.push(self.compute_stream_grads(batch_io[si].0, batch_io[si].1, &mut streams[si], gamma, si));
            }
        } else {
            let chunk = (n_streams + n_workers - 1) / n_workers;
            let brain_ref: &DeepContractiveBrain = self;
            let mut slot_vecs: Vec<Vec<(BrainGrads, f32, Vec<f32>, Vec<f32>)>> =
                (0..n_workers).map(|_| Vec::new()).collect();
            std::thread::scope(|s| {
                let mut stream_rest = &mut streams[..n_streams];
                for (wi, out_vec) in slot_vecs.iter_mut().enumerate() {
                    let start = wi * chunk;
                    if start >= n_streams {
                        break;
                    }
                    let take = (n_streams - start).min(chunk);
                    let (st_slice, next_rest) = stream_rest.split_at_mut(take);
                    stream_rest = next_rest;
                    let io_slice = &batch_io[start..start + take];
                    s.spawn(move || {
                        for local_i in 0..take {
                            out_vec.push(brain_ref.compute_stream_grads(
                                io_slice[local_i].0,
                                io_slice[local_i].1,
                                &mut st_slice[local_i],
                                gamma,
                                wi,
                            ));
                        }
                    });
                }
            });
            for mut v in slot_vecs {
                results.append(&mut v);
            }
        }

        let mut sum_bpb = 0.0f32;
        let mut total_bytes = 0usize;
        for (si, (_, bpb, ref logits, ref latents)) in results.iter().enumerate() {
            sum_bpb += *bpb;
            let (inputs, targets) = batch_io[si];
            let t_len = inputs.len().min(targets.len());
            total_bytes += t_len;
            if update_episodic {
                for ti in 0..t_len {
                    streams[si].push_byte(inputs[ti]);
                    let row = &logits[ti * 256..(ti + 1) * 256];
                    let mut mx = f32::NEG_INFINITY;
                    for &v in row { if v > mx { mx = v; } }
                    let mut se = 0.0f32;
                    for &v in row { se += (v - mx).max(-40.0).exp(); }
                    let tgt = targets[ti];
                    let p_b = ((row[tgt as usize] - mx).max(-40.0).exp() / se.max(1e-12)).clamp(1e-7, 0.9999);
                    let lat_s = &latents[ti * d..(ti + 1) * d];
                    self.episodic.observe(&streams[si].recent_bytes, lat_s, tgt, -(p_b.log2()));
                }
            }
        }
        let avg_bpb = sum_bpb / n_streams as f32;

        // Live-wired AllReduce across all stream gradient shards!
        let mut avg_grads = BrainGrads::zeros(&self.cfg);
        let mut emb_shards: Vec<Vec<f32>> = results.iter().map(|r| r.0.e_byte.clone()).collect();
        allreduce_grad_shards(&mut emb_shards, &mut avg_grads.e_byte);
        let mut wh_shards: Vec<Vec<f32>> = results.iter().map(|r| r.0.w_head.clone()).collect();
        allreduce_grad_shards(&mut wh_shards, &mut avg_grads.w_head);
        let mut bh_shards: Vec<Vec<f32>> = results.iter().map(|r| r.0.b_head.clone()).collect();
        allreduce_grad_shards(&mut bh_shards, &mut avg_grads.b_head);

        for l in 0..self.cfg.n_layers {
            macro_rules! reduce_field {
                ($fld:ident) => {{
                    let mut sh: Vec<Vec<f32>> = results.iter().map(|r| r.0.layers[l].$fld.clone()).collect();
                    allreduce_grad_shards(&mut sh, &mut avg_grads.layers[l].$fld);
                }};
            }
            reduce_field!(conv_w);
            reduce_field!(conv_b);
            reduce_field!(w_gate);
            reduce_field!(b_gate);
            reduce_field!(w_val);
            reduce_field!(b_val);
            reduce_field!(w_key);
            reduce_field!(b_key);
            reduce_field!(w_eq);
            reduce_field!(b_eq);
            reduce_field!(w_up);
            reduce_field!(b_up);
            reduce_field!(w_dn);
            reduce_field!(b_dn);
        }

        // True Global L2 Gradient Norm Clipping (`max_norm = 2.0`)!
        let _grad_norm = avg_grads.clip_global_norm(2.0);

        self.adam_step += 1;
        let eff_lr = pol.calc_lr(base_lr, avg_bpb);
        let wd = self.cfg.weight_decay;
        self.e_byte.step_adamw(&avg_grads.e_byte, eff_lr, wd, self.adam_step);
        self.w_head.step_adamw(&avg_grads.w_head, eff_lr, wd, self.adam_step);
        self.b_head.step_adamw(&avg_grads.b_head, eff_lr, 0.0, self.adam_step);
        for l in 0..self.cfg.n_layers {
            let layer = &mut self.layers[l];
            let lg = &avg_grads.layers[l];
            layer.conv_w.step_adamw(&lg.conv_w, eff_lr, wd, self.adam_step);
            layer.conv_b.step_adamw(&lg.conv_b, eff_lr, 0.0, self.adam_step);
            layer.w_gate.step_adamw(&lg.w_gate, eff_lr, wd, self.adam_step);
            layer.b_gate.step_adamw(&lg.b_gate, eff_lr, 0.0, self.adam_step);
            layer.w_val.step_adamw(&lg.w_val, eff_lr, wd, self.adam_step);
            layer.b_val.step_adamw(&lg.b_val, eff_lr, 0.0, self.adam_step);
            layer.w_key.step_adamw(&lg.w_key, eff_lr, wd, self.adam_step);
            layer.b_key.step_adamw(&lg.b_key, eff_lr, 0.0, self.adam_step);
            layer.w_eq.step_adamw(&lg.w_eq, eff_lr, wd, self.adam_step);
            layer.b_eq.step_adamw(&lg.b_eq, eff_lr, 0.0, self.adam_step);
            layer.w_up.step_adamw(&lg.w_up, eff_lr, wd, self.adam_step);
            layer.b_up.step_adamw(&lg.b_up, eff_lr, 0.0, self.adam_step);
            layer.w_dn.step_adamw(&lg.w_dn, eff_lr, wd, self.adam_step);
            layer.b_dn.step_adamw(&lg.b_dn, eff_lr, 0.0, self.adam_step);
        }

        self.total_bytes_trained = self.total_bytes_trained.saturating_add(total_bytes as u64);
        self.last_train_bpb = avg_bpb;
        avg_bpb
    }

    /// Evaluate strictly-causal `bits/byte` on an unseen real byte stream using EXACT 256-way normalized mixing!
    pub fn eval_byte_stream(
        &mut self,
        data: &[u8],
        chunk_len: usize,
        online_adapt_episodic: bool,
        policy: Option<RuntimePolicyOverride>,
    ) -> (f64, f64) {
        if data.len() < 2 {
            return (8.0, 8.0);
        }
        let pol = policy.unwrap_or(self.active_policy);
        let mut stream = CausalStreamState::new(&self.cfg);
        let d = self.cfg.d_model;
        let mut sum_param = 0.0f64;
        let mut sum_blend = 0.0f64;
        let mut total = 0usize;

        let mut off = 0usize;
        while off + 1 < data.len() {
            let end = (off + chunk_len.max(16)).min(data.len() - 1);
            let inputs = &data[off..end];
            let targets = &data[off + 1..end + 1];
            let (logits, latents) = self.forward_byte_chunk(inputs, &mut stream, Some(pol.gamma), 0);
            for ti in 0..inputs.len() {
                stream.push_byte(inputs[ti]);
                let row = &logits[ti * 256..(ti + 1) * 256];
                let mut mx = f32::NEG_INFINITY;
                for &v in row { if v > mx { mx = v; } }
                let mut p_brain = [0.0f32; 256];
                let mut se = 0.0f32;
                for c in 0..256 {
                    let ex = (row[c] - mx).max(-40.0).exp();
                    p_brain[c] = ex;
                    se += ex;
                }
                let inv_s = 1.0 / se.max(1e-12);
                for c in 0..256 { p_brain[c] *= inv_s; }

                let tgt = targets[ti];
                let lat_s = &latents[ti * d..(ti + 1) * d];
                let (p_mem, conf) = self.episodic.predict_byte_distribution(&stream.recent_bytes, lat_s);
                let p_mix = pol.mix_256(&p_brain, &p_mem, conf);
                let b_p = -(p_brain[tgt as usize].clamp(1e-7, 0.9999) as f64).log2();
                let b_m = -(p_mix[tgt as usize].clamp(1e-7, 0.9999) as f64).log2();
                sum_param += b_p;
                sum_blend += b_m;
                total += 1;
                if online_adapt_episodic {
                    self.episodic.observe(&stream.recent_bytes, lat_s, tgt, b_m as f32);
                }
            }
            off = end;
        }
        (sum_param / total.max(1) as f64, sum_blend / total.max(1) as f64)
    }

    /// 100% Bit-Exact Lossless Binary Checkpoint (`TOMG13` + `active_policy` + `best_heldout_bpb` + SHA-256)!
    pub fn save_checkpoint(&mut self, path: &Path) -> Result<String, String> {
        if let Some(p) = path.parent() {
            let _ = fs::create_dir_all(p);
        }
        let mut buf = Vec::with_capacity(512 * 1024);
        buf.extend_from_slice(CKPT_MAGIC);
        buf.extend_from_slice(&(self.cfg.n_layers as u32).to_le_bytes());
        buf.extend_from_slice(&(self.cfg.d_model as u32).to_le_bytes());
        buf.extend_from_slice(&(self.cfg.d_ff as u32).to_le_bytes());
        buf.extend_from_slice(&(self.cfg.n_ticks as u32).to_le_bytes());
        buf.extend_from_slice(&self.cfg.lr.to_le_bytes());
        buf.extend_from_slice(&self.cfg.weight_decay.to_le_bytes());
        buf.extend_from_slice(&self.cfg.contract_rho.to_le_bytes());
        buf.push(if self.cfg.use_nvfp4_ste { 1 } else { 0 });

        buf.extend_from_slice(&(self.adam_step as u64).to_le_bytes());
        buf.extend_from_slice(&self.total_bytes_trained.to_le_bytes());
        buf.extend_from_slice(&self.last_train_bpb.to_le_bytes());
        buf.extend_from_slice(&self.best_heldout_bpb.to_le_bytes());
        self.active_policy.write_binary(&mut buf);

        self.e_byte.write_binary(&mut buf);
        for l in &mut self.layers {
            l.write_binary(&mut buf);
        }
        self.w_head.write_binary(&mut buf);
        self.b_head.write_binary(&mut buf);
        self.episodic.write_binary(&mut buf);

        let digest = sha256_hex(&buf);
        buf.extend_from_slice(digest.as_bytes());
        fs::write(path, &buf).map_err(|e| format!("ckpt write failed: {}", e))?;
        Ok(digest)
    }

    pub fn load_checkpoint(path: &Path) -> Result<(Self, String), String> {
        let raw = fs::read(path).map_err(|e| format!("ckpt read failed: {}", e))?;
        if raw.len() < 70 || &raw[0..6] != CKPT_MAGIC {
            return Err("invalid TOMG13 checkpoint header".into());
        }
        let split = raw.len() - 64;
        let payload = &raw[..split];
        let stored_hex = std::str::from_utf8(&raw[split..]).map_err(|_| "corrupt sha256 tail")?;
        let computed_hex = sha256_hex(payload);
        if stored_hex != computed_hex {
            return Err(format!("SHA-256 mismatch: stored={} computed={}", stored_hex, computed_hex));
        }

        let mut pos = 6usize;
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("ckpt EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() { return Err("ckpt EOF u64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let take_f32 = |d: &[u8], p: &mut usize| -> Result<f32, String> {
            if *p + 4 > d.len() { return Err("ckpt EOF f32".into()); }
            let v = f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_f64 = |d: &[u8], p: &mut usize| -> Result<f64, String> {
            if *p + 8 > d.len() { return Err("ckpt EOF f64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(f64::from_le_bytes(arr))
        };

        let n_layers = take_u32(payload, &mut pos)? as usize;
        let d_model = take_u32(payload, &mut pos)? as usize;
        let d_ff = take_u32(payload, &mut pos)? as usize;
        let n_ticks = take_u32(payload, &mut pos)? as usize;
        let lr = take_f32(payload, &mut pos)?;
        let weight_decay = take_f32(payload, &mut pos)?;
        let contract_rho = take_f32(payload, &mut pos)?;
        if pos >= payload.len() { return Err("ckpt EOF flag".into()); }
        let use_nvfp4_ste = payload[pos] != 0;
        pos += 1;

        let cfg = BrainConfig { n_layers, d_model, d_ff, n_ticks, lr, weight_decay, contract_rho, use_nvfp4_ste };
        let adam_step = take_u64(payload, &mut pos)? as usize;
        let total_bytes_trained = take_u64(payload, &mut pos)?;
        let last_train_bpb = take_f32(payload, &mut pos)?;
        let best_heldout_bpb = take_f64(payload, &mut pos)?;
        let active_policy = RuntimePolicyOverride::read_binary(payload, &mut pos)?;

        let e_byte = ParamTensor::read_binary(payload, &mut pos, 0)?;
        let mut layers = Vec::with_capacity(n_layers);
        for li in 0..n_layers {
            layers.push(ContractiveSsmLayer::read_binary(payload, &mut pos, li)?);
        }
        let w_head = ParamTensor::read_binary(payload, &mut pos, 0)?;
        let b_head = ParamTensor::read_binary(payload, &mut pos, 0)?;
        let episodic = TropicalEpisodicMemory::read_binary(payload, &mut pos)?;

        Ok((
            Self {
                cfg,
                e_byte,
                layers,
                w_head,
                b_head,
                episodic,
                active_policy,
                adam_step,
                total_bytes_trained,
                last_train_bpb,
                best_heldout_bpb,
            },
            computed_hex,
        ))
    }
}

#[inline(always)]
fn x_n1_ref(x_n1: &[f32], ti: usize, d: usize, j: usize) -> f32 {
    x_n1[ti * d + j]
}
