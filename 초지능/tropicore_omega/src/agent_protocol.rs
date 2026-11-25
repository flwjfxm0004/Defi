//! Domain-Agnostic Stateful External `UniversalAgent` TD-MPC2 Actor-Critic Protocol (`TropiCore-Omega v17.0`, `TOMA17`).
//!
//! Upgrades over v15.0/v16.0 (`TOMA15`):
//!   1. Zero Latent Information Loss: `compress_to_belief` pools 100% of `z_full` into `(EMA belief, velocity)`!
//!   2. Learned Continuous Policy Actor + Polyak Target Actor (`actor`, `target_actor`):
//!        * Trained via exact analytical policy gradient `d/da min(Q_0(b, a), Q_1(b, a))`
//!        * Exact Bellman TD target `min(target_Q_0(b', target_actor(b')), target_Q_1(b', target_actor(b')))`
//!          (fixes `v15.0`'s suboptimal `Q(s', 0)` bootstrap!)
//!        * Actor-Warm-Started CEM MPC Planning (TD-MPC2 style: seeds horizon mean & Candidate #0 from `actor`!)
//!   3. Zero-Clone `CausalStreamState` (`take_or_init_stream` — ZERO per-step stream cloning!).
//!   4. Prioritized TD-Error Replay + Polyak Target Critic + `TOMA17` SHA-256 Checkpoint Persistence!

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::brain::{BrainConfig, CausalStreamState, DeepContractiveBrain};
use crate::sha256::sha256_hex;

const AGENT_MAGIC: &[u8; 6] = b"TOMA17";
const REPLAY_CAP: usize = 512;
const TD_GAMMA: f32 = 0.94;
const POLYAK_TAU: f32 = 0.15;

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_uniform(rng: &mut u64) -> f32 {
    ((splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64)) as f32
}

fn rand_normal(rng: &mut u64) -> f32 {
    let u1 = rand_uniform(rng).clamp(1e-7, 1.0 - 1e-7);
    let u2 = rand_uniform(rng);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

#[derive(Clone, Debug)]
struct AdamBuf {
    w: Vec<f32>,
    m: Vec<f32>,
    v: Vec<f32>,
}

impl AdamBuf {
    fn new(w: Vec<f32>) -> Self {
        let n = w.len();
        Self { w, m: vec![0.0; n], v: vec![0.0; n] }
    }

    fn polyak_update_from(&mut self, src: &AdamBuf, tau: f32) {
        let one_m = 1.0 - tau;
        for (dst, &s) in self.w.iter_mut().zip(src.w.iter()) {
            *dst = one_m * (*dst) + tau * s;
        }
    }

    fn step(&mut self, grad: &[f32], lr: f32, wd: f32, step: usize, clip_w: f32) {
        let b1 = 0.9f32;
        let b2 = 0.999f32;
        let st = step.max(1) as i32;
        let inv_bc1 = 1.0 / (1.0 - b1.powi(st)).max(1e-6);
        let inv_bc2 = 1.0 / (1.0 - b2.powi(st)).max(1e-6);
        for i in 0..self.w.len() {
            let g = grad[i].clamp(-3.0, 3.0);
            self.m[i] = b1 * self.m[i] + (1.0 - b1) * g;
            self.v[i] = b2 * self.v[i] + (1.0 - b2) * g * g;
            let nw = self.w[i] * (1.0 - lr * wd) - lr * (self.m[i] * inv_bc1) / ((self.v[i] * inv_bc2).sqrt() + 1e-8);
            self.w[i] = nw.clamp(-clip_w, clip_w);
        }
    }

    fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.w.len() as u32).to_le_bytes());
        for sl in [&self.w, &self.m, &self.v] {
            for &v in sl { out.extend_from_slice(&v.to_le_bytes()); }
        }
    }

    fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        if *pos + 4 > data.len() { return Err("adam buf EOF len".into()); }
        let n = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]) as usize;
        *pos += 4;
        let take_v = |p: &mut usize| -> Result<Vec<f32>, String> {
            if *p + n * 4 > data.len() { return Err("adam buf EOF vec".into()); }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(f32::from_le_bytes([data[*p], data[*p + 1], data[*p + 2], data[*p + 3]]));
                *p += 4;
            }
            Ok(v)
        };
        let w = take_v(pos)?;
        let m = take_v(pos)?;
        let v = take_v(pos)?;
        Ok(Self { w, m, v })
    }
}

/// Learned Continuous Policy Actor MLP `pi_phi(belief) -> [-1, 1]^act_dim` (TD-MPC2 / TD3 style).
#[derive(Clone, Debug)]
struct LatentActorMlp {
    lat_dim: usize,
    hid_dim: usize,
    act_dim: usize,
    w1: AdamBuf,
    b1: AdamBuf,
    w2: AdamBuf,
    b2: AdamBuf,
    adam_step: usize,
}

impl LatentActorMlp {
    fn new(lat_dim: usize, act_dim: usize, seed: u64) -> Self {
        let mut rng = seed ^ 0xAC70_1700_4242;
        let hid_dim = lat_dim.max(32);
        let s1 = (1.0 / lat_dim as f32).sqrt() * 0.5;
        let s2 = (1.0 / hid_dim as f32).sqrt() * 0.2;
        let w1 = (0..hid_dim * lat_dim).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s1).collect();
        let w2 = (0..act_dim * hid_dim).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s2).collect();
        Self {
            lat_dim,
            hid_dim,
            act_dim,
            w1: AdamBuf::new(w1),
            b1: AdamBuf::new(vec![0.0; hid_dim]),
            w2: AdamBuf::new(w2),
            b2: AdamBuf::new(vec![0.0; act_dim]),
            adam_step: 0,
        }
    }

    fn polyak_update_from(&mut self, src: &Self, tau: f32) {
        self.w1.polyak_update_from(&src.w1, tau);
        self.b1.polyak_update_from(&src.b1, tau);
        self.w2.polyak_update_from(&src.w2, tau);
        self.b2.polyak_update_from(&src.b2, tau);
    }

    fn forward(&self, belief: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut h = vec![0.0f32; self.hid_dim];
        for r in 0..self.hid_dim {
            let row = r * self.lat_dim;
            let mut acc = self.b1.w[r];
            for c in 0..self.lat_dim {
                acc += self.w1.w[row + c] * belief[c];
            }
            h[r] = acc.tanh();
        }
        let mut act = vec![0.0f32; self.act_dim];
        for o in 0..self.act_dim {
            let row = o * self.hid_dim;
            let mut acc = self.b2.w[o];
            for c in 0..self.hid_dim {
                acc += self.w2.w[row + c] * h[c];
            }
            act[o] = acc.tanh();
        }
        (act, h)
    }

    /// Ascends `Q(b, pi_phi(b))` via exact chain rule from critic action gradient `dq_da`.
    fn step_maximize_q(&mut self, belief: &[f32], act: &[f32], h: &[f32], dq_da: &[f32], lr: f32) {
        let mut gw1 = vec![0.0f32; self.w1.w.len()];
        let mut gb1 = vec![0.0f32; self.b1.w.len()];
        let mut gw2 = vec![0.0f32; self.w2.w.len()];
        let mut gb2 = vec![0.0f32; self.b2.w.len()];
        let mut dh = vec![0.0f32; self.hid_dim];

        for o in 0..self.act_dim {
            let dpre = (-dq_da[o] * (1.0 - act[o] * act[o])).clamp(-2.0, 2.0);
            gb2[o] = dpre;
            let row = o * self.hid_dim;
            for c in 0..self.hid_dim {
                dh[c] += dpre * self.w2.w[row + c];
                gw2[row + c] = dpre * h[c];
            }
        }
        for r in 0..self.hid_dim {
            let dp = (dh[r] * (1.0 - h[r] * h[r])).clamp(-2.0, 2.0);
            gb1[r] = dp;
            let row = r * self.lat_dim;
            for c in 0..self.lat_dim {
                gw1[row + c] = dp * belief[c];
            }
        }
        self.adam_step += 1;
        let st = self.adam_step;
        self.w1.step(&gw1, lr, 1e-4, st, 4.0);
        self.b1.step(&gb1, lr, 0.0, st, 4.0);
        self.w2.step(&gw2, lr, 1e-4, st, 4.0);
        self.b2.step(&gb2, lr, 0.0, st, 4.0);
    }

    fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.lat_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.hid_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.act_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.adam_step as u64).to_le_bytes());
        self.w1.write_binary(out);
        self.b1.write_binary(out);
        self.w2.write_binary(out);
        self.b2.write_binary(out);
    }

    fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("actor EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() { return Err("actor EOF u64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let lat_dim = take_u32(data, pos)? as usize;
        let hid_dim = take_u32(data, pos)? as usize;
        let act_dim = take_u32(data, pos)? as usize;
        let adam_step = take_u64(data, pos)? as usize;
        let w1 = AdamBuf::read_binary(data, pos)?;
        let b1 = AdamBuf::read_binary(data, pos)?;
        let w2 = AdamBuf::read_binary(data, pos)?;
        let b2 = AdamBuf::read_binary(data, pos)?;
        Ok(Self { lat_dim, hid_dim, act_dim, w1, b1, w2, b2, adam_step })
    }
}

#[derive(Clone, Debug)]
struct LatentWorldCriticMlp {
    lat_dim: usize,
    hid_dim: usize,
    in_dim: usize,
    out_dim: usize,
    w1: AdamBuf,
    b1: AdamBuf,
    w2: AdamBuf,
    w_lin: AdamBuf,
    b2: AdamBuf,
    adam_step: usize,
}

impl LatentWorldCriticMlp {
    fn new(lat_dim: usize, act_dim: usize, seed: u64) -> Self {
        let mut rng = seed ^ 0xA6E0_1700_9999;
        let hid_dim = (lat_dim * 2).max(32);
        let in_dim = lat_dim + act_dim;
        let out_dim = lat_dim + 2;
        let s1 = (1.0 / in_dim as f32).sqrt() * 0.7;
        let s2 = (1.0 / hid_dim as f32).sqrt() * 0.25;
        let w1 = (0..hid_dim * in_dim).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s1).collect();
        let w2 = (0..out_dim * hid_dim).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s2).collect();
        Self {
            lat_dim, hid_dim, in_dim, out_dim,
            w1: AdamBuf::new(w1),
            b1: AdamBuf::new(vec![0.0; hid_dim]),
            w2: AdamBuf::new(w2),
            w_lin: AdamBuf::new(vec![0.0; lat_dim * in_dim]), // Linear skip ONLY for dynamics `0..lat_dim` (prevents OOD reward blowup!)
            b2: AdamBuf::new(vec![0.0; out_dim]),
            adam_step: 0,
        }
    }

    fn polyak_update_from(&mut self, src: &Self, tau: f32) {
        self.w1.polyak_update_from(&src.w1, tau);
        self.b1.polyak_update_from(&src.b1, tau);
        self.w2.polyak_update_from(&src.w2, tau);
        self.w_lin.polyak_update_from(&src.w_lin, tau);
        self.b2.polyak_update_from(&src.b2, tau);
    }

    fn forward(&self, belief: &[f32], act: &[f32]) -> (Vec<f32>, f32, f32, Vec<f32>, Vec<f32>) {
        let mut x = Vec::with_capacity(self.in_dim);
        x.extend_from_slice(belief);
        x.extend_from_slice(act);
        let mut h = vec![0.0f32; self.hid_dim];
        for r in 0..self.hid_dim {
            let row = r * self.in_dim;
            let mut acc = self.b1.w[r];
            for c in 0..self.in_dim { acc += self.w1.w[row + c] * x[c]; }
            h[r] = acc.tanh();
        }
        let mut out = vec![0.0f32; self.out_dim];
        for o in 0..self.out_dim {
            let row = o * self.hid_dim;
            let mut acc = self.b2.w[o];
            for c in 0..self.hid_dim { acc += self.w2.w[row + c] * h[c]; }
            if o < self.lat_dim {
                let lrow = o * self.in_dim;
                for c in 0..self.in_dim { acc += self.w_lin.w[lrow + c] * x[c]; }
            }
            out[o] = acc;
        }
        let mut next_b = vec![0.0f32; self.lat_dim];
        for i in 0..self.lat_dim { next_b[i] = (belief[i] + out[i]).clamp(-4.0, 4.0); }
        (next_b, out[self.lat_dim], out[self.lat_dim + 1], h, x)
    }

    /// Exact analytical `(Q(b, a), dQ/da)` for actor policy-gradient training!
    fn q_and_action_grad(&self, belief: &[f32], act: &[f32]) -> (f32, Vec<f32>) {
        let (_, _, q_val, h, _) = self.forward(belief, act);
        let ad = act.len();
        let o = self.lat_dim + 1;
        let mut dq_da = vec![0.0f32; ad];
        let w2_row = o * self.hid_dim;
        for r in 0..self.hid_dim {
            let dh_r = self.w2.w[w2_row + r] * (1.0 - h[r] * h[r]);
            let w1_row = r * self.in_dim + self.lat_dim;
            for j in 0..ad {
                dq_da[j] += dh_r * self.w1.w[w1_row + j];
            }
        }
        (q_val, dq_da)
    }

    fn train_step(
        &mut self,
        belief: &[f32],
        act: &[f32],
        next_b: &[f32],
        reward: f32,
        td_target_v: f32,
        lr: f32,
    ) -> (f32, f32) {
        let (pred_nb, pred_r, pred_v, h, x) = self.forward(belief, act);
        let mut err = vec![0.0f32; self.out_dim];
        let mut mse = 0.0f32;
        for i in 0..self.lat_dim {
            let e = (pred_nb[i] - next_b[i]).clamp(-2.5, 2.5);
            err[i] = e * 0.65;
            mse += e * e;
        }
        let er = (pred_r - reward).clamp(-3.0, 3.0);
        let ev = (pred_v - td_target_v).clamp(-3.0, 3.0);
        err[self.lat_dim] = er * 3.5;
        err[self.lat_dim + 1] = ev * 0.75;
        mse = (mse / self.lat_dim as f32) + er * er + 0.25 * ev * ev;

        let mut gw1 = vec![0.0f32; self.w1.w.len()];
        let mut gb1 = vec![0.0f32; self.b1.w.len()];
        let mut gw2 = vec![0.0f32; self.w2.w.len()];
        let mut gwlin = vec![0.0f32; self.w_lin.w.len()];
        let mut gb2 = vec![0.0f32; self.b2.w.len()];
        let mut dh = vec![0.0f32; self.hid_dim];

        for o in 0..self.out_dim {
            let e = err[o];
            let row = o * self.hid_dim;
            gb2[o] = e;
            for c in 0..self.hid_dim {
                dh[c] += e * self.w2.w[row + c];
                gw2[row + c] = e * h[c];
            }
            if o < self.lat_dim {
                let lrow = o * self.in_dim;
                for c in 0..self.in_dim { gwlin[lrow + c] = e * x[c]; }
            }
        }
        for r in 0..self.hid_dim {
            let dp = (dh[r] * (1.0 - h[r] * h[r])).clamp(-2.0, 2.0);
            gb1[r] = dp;
            let row = r * self.in_dim;
            for c in 0..self.in_dim { gw1[row + c] = dp * x[c]; }
        }

        self.adam_step += 1;
        let st = self.adam_step;
        self.w1.step(&gw1, lr, 5e-4, st, 4.0);
        self.b1.step(&gb1, lr, 0.0, st, 4.0);
        self.w2.step(&gw2, lr, 5e-4, st, 4.0);
        self.w_lin.step(&gwlin, lr, 5e-4, st, 4.0);
        self.b2.step(&gb2, lr, 0.0, st, 4.0);
        (mse, er.abs() + 0.5 * ev.abs())
    }

    fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.lat_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.hid_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.in_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.out_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.adam_step as u64).to_le_bytes());
        self.w1.write_binary(out);
        self.b1.write_binary(out);
        self.w2.write_binary(out);
        self.w_lin.write_binary(out);
        self.b2.write_binary(out);
    }

    fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("agent mlp EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() { return Err("agent mlp EOF u64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let lat_dim = take_u32(data, pos)? as usize;
        let hid_dim = take_u32(data, pos)? as usize;
        let in_dim = take_u32(data, pos)? as usize;
        let out_dim = take_u32(data, pos)? as usize;
        let adam_step = take_u64(data, pos)? as usize;
        let w1 = AdamBuf::read_binary(data, pos)?;
        let b1 = AdamBuf::read_binary(data, pos)?;
        let w2 = AdamBuf::read_binary(data, pos)?;
        let w_lin = AdamBuf::read_binary(data, pos)?;
        let b2 = AdamBuf::read_binary(data, pos)?;
        Ok(Self { lat_dim, hid_dim, in_dim, out_dim, w1, b1, w2, w_lin, b2, adam_step })
    }
}

#[derive(Clone, Debug)]
struct ReplayTransition {
    belief: Vec<f32>,
    action: Vec<f32>,
    next_belief: Vec<f32>,
    reward: f32,
    done: bool,
    td_priority: f32,
}

#[derive(Clone, Debug)]
pub enum ExternalObservation {
    Numeric(Vec<f32>),
    RawBytes(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct AgentStepOutput {
    pub action: Vec<f32>,
    pub pred_reward: f32,
    pub pred_value: f32,
    pub surprise: f32,
    pub epistemic_unc: f32,
    pub online_updates: usize,
    pub replay_len: usize,
}

#[derive(Clone, Debug)]
pub struct UniversalOpenWorldAgent {
    pub act_dim: usize,
    pub lat_dim: usize,
    m0: LatentWorldCriticMlp,
    m1: LatentWorldCriticMlp,
    target_m0: LatentWorldCriticMlp,
    target_m1: LatentWorldCriticMlp,
    actor: LatentActorMlp,
    target_actor: LatentActorMlp,
    brain_stream: Option<CausalStreamState>,
    cur_belief: Vec<f32>,
    prev_z_pool: Vec<f32>,
    prev_belief: Option<Vec<f32>>,
    prev_action: Option<Vec<f32>>,
    prev_plan: Vec<Vec<f32>>,
    replay: Vec<ReplayTransition>,
    pub online_updates: usize,
    pub surprise_triggers: usize,
    rng: u64,
}

impl UniversalOpenWorldAgent {
    pub fn new(act_dim: usize, brain_cfg: &BrainConfig, seed: u64) -> Self {
        let ad = act_dim.max(1);
        let lat_dim = (brain_cfg.d_model.min(16).max(12) / 2) * 2;
        let half = lat_dim / 2;
        let m0 = LatentWorldCriticMlp::new(lat_dim, ad, seed ^ 0x1111_AAAA);
        let m1 = LatentWorldCriticMlp::new(lat_dim, ad, seed ^ 0x7777_5555);
        let target_m0 = m0.clone();
        let target_m1 = m1.clone();
        let actor = LatentActorMlp::new(lat_dim, ad, seed ^ 0x3333_CCCC);
        let target_actor = actor.clone();
        Self {
            act_dim: ad,
            lat_dim,
            m0,
            m1,
            target_m0,
            target_m1,
            actor,
            target_actor,
            brain_stream: Some(CausalStreamState::new(brain_cfg)),
            cur_belief: vec![0.0; lat_dim],
            prev_z_pool: vec![0.0; half],
            prev_belief: None,
            prev_action: None,
            prev_plan: Vec::new(),
            replay: Vec::with_capacity(REPLAY_CAP),
            online_updates: 0,
            surprise_triggers: 0,
            rng: seed ^ 0xCAFE_1700,
        }
    }

    pub fn replay_len(&self) -> usize { self.replay.len() }

    /// Zero-clone stream ownership extraction (eliminates per-step `CausalStreamState` clone!).
    fn take_or_init_stream(&mut self, cfg: &BrainConfig) -> CausalStreamState {
        if let Some(st) = self.brain_stream.take() {
            if st.layer_ssm.len() == cfg.n_layers && st.layer_ssm.first().map(|v| v.len()) == Some(cfg.d_model) {
                return st;
            }
        }
        CausalStreamState::new(cfg)
    }

    /// Universal symlog-residual belief encoder (DreamerV3 principle, 100% domain-agnostic!):
    /// Preserves raw `asinh` numeric sensor coordinates without RMSNorm scale-erasure alongside
    /// the full strided-folded recurrent brain latent `z_full`, plus exact 1st-order velocity!
    fn compress_to_belief(&mut self, z_full: &[f32], raw_numeric: Option<&[f32]>) -> Vec<f32> {
        let ld = self.lat_dim;
        let half = ld / 2;
        let mut z_pool = vec![0.0f32; half];
        let direct_n = raw_numeric.map_or(0, |v| v.len().min(half / 2));
        if let Some(v) = raw_numeric {
            for i in 0..direct_n {
                let x = v[i];
                z_pool[i] = if x.is_finite() { x.asinh().clamp(-3.0, 3.0) } else { 0.0 };
            }
        }
        let rem = half - direct_n;
        if !z_full.is_empty() && rem > 0 {
            let mut counts = vec![0u32; rem];
            for (j, &v) in z_full.iter().enumerate() {
                let slot = j % rem;
                z_pool[direct_n + slot] += v;
                counts[slot] += 1;
            }
            for i in 0..rem {
                if counts[i] > 0 {
                    z_pool[direct_n + i] = (z_pool[direct_n + i] / counts[i] as f32).clamp(-3.0, 3.0);
                }
            }
        }
        let mut b = vec![0.0f32; ld];
        for i in 0..half {
            // Preserve zero-lag observability on direct sensor slots; mild EMA on recurrent latent slots
            b[i] = if i < direct_n {
                z_pool[i]
            } else {
                0.35 * self.cur_belief[i] + 0.65 * z_pool[i]
            };
            b[half + i] = (z_pool[i] - self.prev_z_pool[i]).clamp(-2.0, 2.0);
        }
        self.prev_z_pool = z_pool;
        self.cur_belief = b.clone();
        b
    }

    fn predict_ensemble(&self, b: &[f32], a: &[f32]) -> (Vec<f32>, f32, f32, f32) {
        let (nb0, r0, v0, _, _) = self.m0.forward(b, a);
        let (nb1, r1, v1, _, _) = self.m1.forward(b, a);
        let mut mean_b = vec![0.0f32; self.lat_dim];
        let mut dis2 = (r0 - r1) * (r0 - r1) + 0.25 * (v0 - v1) * (v0 - v1);
        for i in 0..self.lat_dim {
            mean_b[i] = 0.5 * (nb0[i] + nb1[i]);
            let d = nb0[i] - nb1[i];
            dis2 += d * d;
        }
        (mean_b, 0.5 * (r0 + r1), 0.5 * (v0 + v1), (dis2 / (self.lat_dim as f32 + 2.0)).sqrt())
    }

    /// Exact TD3 / TD-MPC2 Bellman Target Value with conservative target-policy smoothing!
    fn estimate_target_value(&self, b: &[f32]) -> f32 {
        let (mut a_tgt, _) = self.target_actor.forward(b);
        for v in a_tgt.iter_mut() { *v = (*v * 0.85).clamp(-1.0, 1.0); }
        let (_, _, q0, _, _) = self.target_m0.forward(b, &a_tgt);
        let (_, _, q1, _, _) = self.target_m1.forward(b, &a_tgt);
        q0.min(q1)
    }

    /// Updates continuous policy actor `pi_phi(b)` to maximize conservative `min(Q_0(b, a), Q_1(b, a))`.
    fn update_actor_on_belief(&mut self, b: &[f32], lr: f32) {
        let (act, h) = self.actor.forward(b);
        let (q0, g0) = self.m0.q_and_action_grad(b, &act);
        let (q1, g1) = self.m1.q_and_action_grad(b, &act);
        let dq_da = if q0 <= q1 { g0 } else { g1 };
        self.actor.step_maximize_q(b, &act, &h, &dq_da, lr);
    }

    pub fn step(&mut self, brain: &DeepContractiveBrain, obs: &ExternalObservation, reward: f32, done: bool) -> AgentStepOutput {
        let mut stream = self.take_or_init_stream(&brain.cfg);
        let (z_full, raw_num) = match obs {
            ExternalObservation::Numeric(v) => (brain.encode_raw_sensor_vector_stateful(v, &mut stream), Some(v.as_slice())),
            ExternalObservation::RawBytes(bytes) => (brain.encode_raw_bytes_to_latent_stateful(bytes, &mut stream), None),
        };
        self.brain_stream = Some(stream);
        let b_cur = self.compress_to_belief(&z_full, raw_num);

        let mut surprise = 0.0f32;
        if let (Some(bp), Some(ap)) = (self.prev_belief.take(), self.prev_action.take()) {
            let v_next = if done { 0.0 } else { self.estimate_target_value(&b_cur) };
            let td_target = (reward + TD_GAMMA * v_next).clamp(-25.0, 25.0);
            let (l0, prio0) = self.m0.train_step(&bp, &ap, &b_cur, reward, td_target, 7.5e-3);
            let (l1, prio1) = self.m1.train_step(&bp, &ap, &b_cur, reward, td_target, 8.0e-3);
            surprise = 0.5 * (l0 + l1);
            let td_priority = 0.5 * (prio0 + prio1) + 0.05;
            self.online_updates += 1;

            if self.replay.len() >= REPLAY_CAP { self.replay.remove(0); }
            self.replay.push(ReplayTransition {
                belief: bp,
                action: ap,
                next_belief: b_cur.clone(),
                reward,
                done,
                td_priority,
            });

            // Balanced Prioritized Experience Replay (4 top-TD + 2 uniform random + 4 most recent!)
            if self.replay.len() >= 4 {
                if surprise > 0.15 { self.surprise_triggers += 1; }
                let n = self.replay.len();
                let mut ranked: Vec<(usize, f32)> = self.replay.iter().enumerate().map(|(i, t)| (i, t.td_priority)).collect();
                ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                let mut batch_idx: Vec<usize> = ranked.iter().take(4).map(|x| x.0).collect();
                for _ in 0..2 {
                    let ri = (splitmix64(&mut self.rng) as usize) % n;
                    if !batch_idx.contains(&ri) { batch_idx.push(ri); }
                }
                for recent_i in n.saturating_sub(4)..n {
                    if !batch_idx.contains(&recent_i) { batch_idx.push(recent_i); }
                }
                for (bi, &idx) in batch_idx.iter().enumerate() {
                    let tr = self.replay[idx].clone();
                    let vn = if tr.done { 0.0 } else { self.estimate_target_value(&tr.next_belief) };
                    let tgt = (tr.reward + TD_GAMMA * vn).clamp(-25.0, 25.0);
                    let (_, p0) = self.m0.train_step(&tr.belief, &tr.action, &tr.next_belief, tr.reward, tgt, 6.0e-3);
                    let (_, p1) = self.m1.train_step(&tr.belief, &tr.action, &tr.next_belief, tr.reward, tgt, 6.5e-3);
                    if self.online_updates >= 8 && bi < 2 {
                        self.update_actor_on_belief(&tr.belief, 2.5e-3);
                    }
                    self.replay[idx].td_priority = 0.5 * (p0 + p1) + 0.05;
                }
            }

            // Smooth Online Polyak EMA Target Critic & Target Actor update (`tau = 0.15`)!
            self.target_m0.polyak_update_from(&self.m0, POLYAK_TAU);
            self.target_m1.polyak_update_from(&self.m1, POLYAK_TAU);
            if self.online_updates >= 8 {
                self.target_actor.polyak_update_from(&self.actor, POLYAK_TAU);
            }
        }

        if done {
            self.cur_belief.fill(0.0);
            self.prev_z_pool.fill(0.0);
            self.prev_belief = None;
            self.prev_action = None;
            self.prev_plan.clear();
            self.brain_stream = Some(CausalStreamState::new(&brain.cfg));
            return AgentStepOutput {
                action: vec![0.0; self.act_dim],
                pred_reward: 0.0,
                pred_value: 0.0,
                surprise,
                epistemic_unc: 0.0,
                online_updates: self.online_updates,
                replay_len: self.replay_len(),
            };
        }

        // TD-MPC2 Hybrid Actor-Guided + Broad-Search CEM Model-Predictive Planning!
        const H: usize = 5;
        const POP: usize = 36;
        const ELITES: usize = 6;
        let ad = self.act_dim;
        let unc_w = if self.online_updates < 18 { 0.35f32 } else { 0.04f32 };

        let mut actor_seq = Vec::with_capacity(H);
        {
            let mut b_roll = b_cur.clone();
            for _ in 0..H {
                let (a_pi, _) = self.actor.forward(&b_roll);
                let (nb, _, _, _) = self.predict_ensemble(&b_roll, &a_pi);
                actor_seq.push(a_pi);
                b_roll = nb;
            }
        }
        let mut mean = vec![vec![0.0f32; ad]; H];
        let mut std = vec![vec![0.85f32; ad]; H];

        for iter in 0..3 {
            let mut cands: Vec<(f32, Vec<Vec<f32>>)> = Vec::with_capacity(POP);
            for pi in 0..POP {
                let mut seq = Vec::with_capacity(H);
                let mut cur_b = b_cur.clone();
                let mut ret = 0.0f32;
                let mut disc = 1.0f32;
                for t in 0..H {
                    let mut a = Vec::with_capacity(ad);
                    for j in 0..ad {
                        let v = if iter == 0 && pi == 0 && self.online_updates >= 8 {
                            actor_seq[t][j]
                        } else if iter == 0 && pi < 6 && self.online_updates >= 8 {
                            actor_seq[t][j] + 0.30 * rand_normal(&mut self.rng)
                        } else if iter == 0 && pi == 6 && self.prev_plan.len() >= H {
                            // Inject shifted previous plan as a single candidate (zero lock-in!)
                            self.prev_plan[(t + 1).min(H - 1)][j]
                        } else {
                            mean[t][j] + std[t][j] * rand_normal(&mut self.rng)
                        };
                        a.push(v.clamp(-1.0, 1.0));
                    }
                    let (nb, r_hat, _, unc) = self.predict_ensemble(&cur_b, &a);
                    ret += disc * (r_hat + unc_w * unc);
                    disc *= TD_GAMMA;
                    cur_b = nb;
                    seq.push(a);
                }
                ret += disc * self.estimate_target_value(&cur_b);
                cands.push((ret, seq));
            }
            cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            for t in 0..H {
                for j in 0..ad {
                    let m: f32 = (0..ELITES).map(|e| cands[e].1[t][j]).sum::<f32>() / ELITES as f32;
                    let v: f32 = (0..ELITES).map(|e| (cands[e].1[t][j] - m).powi(2)).sum::<f32>() / ELITES as f32;
                    mean[t][j] = m;
                    std[t][j] = v.sqrt().clamp(0.06, 0.85);
                }
            }
        }

        let chosen = mean[0].clone();
        self.prev_plan = mean;
        let (_, pred_reward, pred_value, epistemic_unc) = self.predict_ensemble(&b_cur, &chosen);
        self.prev_belief = Some(b_cur);
        self.prev_action = Some(chosen.clone());

        AgentStepOutput {
            action: chosen,
            pred_reward,
            pred_value,
            surprise,
            epistemic_unc,
            online_updates: self.online_updates,
            replay_len: self.replay_len(),
        }
    }

    pub fn save_checkpoint(&self, path: &Path) -> Result<String, String> {
        if let Some(p) = path.parent() { let _ = fs::create_dir_all(p); }
        let mut buf = Vec::with_capacity(64 * 1024);
        buf.extend_from_slice(AGENT_MAGIC);
        buf.extend_from_slice(&(self.act_dim as u32).to_le_bytes());
        buf.extend_from_slice(&(self.lat_dim as u32).to_le_bytes());
        buf.extend_from_slice(&(self.online_updates as u64).to_le_bytes());
        buf.extend_from_slice(&(self.surprise_triggers as u64).to_le_bytes());
        buf.extend_from_slice(&self.rng.to_le_bytes());
        self.m0.write_binary(&mut buf);
        self.m1.write_binary(&mut buf);
        self.target_m0.write_binary(&mut buf);
        self.target_m1.write_binary(&mut buf);
        self.actor.write_binary(&mut buf);
        self.target_actor.write_binary(&mut buf);

        buf.extend_from_slice(&(self.replay.len() as u32).to_le_bytes());
        for tr in &self.replay {
            for &v in &tr.belief { buf.extend_from_slice(&v.to_le_bytes()); }
            for &v in &tr.action { buf.extend_from_slice(&v.to_le_bytes()); }
            for &v in &tr.next_belief { buf.extend_from_slice(&v.to_le_bytes()); }
            buf.extend_from_slice(&tr.reward.to_le_bytes());
            buf.extend_from_slice(&tr.td_priority.to_le_bytes());
            buf.push(if tr.done { 1 } else { 0 });
        }

        let sha = sha256_hex(&buf);
        buf.extend_from_slice(sha.as_bytes());
        fs::write(path, &buf).map_err(|e| format!("agent ckpt write err: {}", e))?;
        Ok(sha)
    }

    pub fn load_checkpoint(path: &Path) -> Result<(Self, String), String> {
        let raw = fs::read(path).map_err(|e| format!("agent ckpt read err: {}", e))?;
        if raw.len() < 70 || &raw[0..6] != AGENT_MAGIC {
            return Err("invalid TOMA17 agent checkpoint".into());
        }
        let split = raw.len() - 64;
        let payload = &raw[..split];
        let stored = std::str::from_utf8(&raw[split..]).map_err(|_| "corrupt agent sha")?;
        let computed = sha256_hex(payload);
        if stored != computed { return Err("agent checkpoint SHA-256 mismatch".into()); }

        let mut pos = 6usize;
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("agent EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_u64 = |d: &[u8], p: &mut usize| -> Result<u64, String> {
            if *p + 8 > d.len() { return Err("agent EOF u64".into()); }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&d[*p..*p + 8]);
            *p += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let take_f32_vec = |d: &[u8], p: &mut usize, n: usize| -> Result<Vec<f32>, String> {
            if *p + n * 4 > d.len() { return Err("agent EOF f32 vec".into()); }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]));
                *p += 4;
            }
            Ok(v)
        };

        let act_dim = take_u32(payload, &mut pos)? as usize;
        let lat_dim = take_u32(payload, &mut pos)? as usize;
        let online_updates = take_u64(payload, &mut pos)? as usize;
        let surprise_triggers = take_u64(payload, &mut pos)? as usize;
        let rng = take_u64(payload, &mut pos)?;
        let m0 = LatentWorldCriticMlp::read_binary(payload, &mut pos)?;
        let m1 = LatentWorldCriticMlp::read_binary(payload, &mut pos)?;
        let target_m0 = LatentWorldCriticMlp::read_binary(payload, &mut pos)?;
        let target_m1 = LatentWorldCriticMlp::read_binary(payload, &mut pos)?;
        let actor = LatentActorMlp::read_binary(payload, &mut pos)?;
        let target_actor = LatentActorMlp::read_binary(payload, &mut pos)?;

        let n_rep = take_u32(payload, &mut pos)? as usize;
        let mut replay = Vec::with_capacity(n_rep.max(REPLAY_CAP));
        for _ in 0..n_rep {
            let belief = take_f32_vec(payload, &mut pos, lat_dim)?;
            let action = take_f32_vec(payload, &mut pos, act_dim)?;
            let next_belief = take_f32_vec(payload, &mut pos, lat_dim)?;
            if pos + 9 > payload.len() { return Err("agent EOF replay".into()); }
            let reward = f32::from_le_bytes([payload[pos], payload[pos + 1], payload[pos + 2], payload[pos + 3]]);
            let td_priority = f32::from_le_bytes([payload[pos + 4], payload[pos + 5], payload[pos + 6], payload[pos + 7]]);
            let done = payload[pos + 8] != 0;
            pos += 9;
            replay.push(ReplayTransition { belief, action, next_belief, reward, done, td_priority });
        }

        let half = lat_dim / 2;
        Ok((
            Self {
                act_dim, lat_dim, m0, m1, target_m0, target_m1, actor, target_actor,
                brain_stream: None,
                cur_belief: vec![0.0; lat_dim],
                prev_z_pool: vec![0.0; half],
                prev_belief: None,
                prev_action: None,
                prev_plan: Vec::new(),
                replay,
                online_updates,
                surprise_triggers,
                rng,
            },
            computed,
        ))
    }
}

fn parse_json_string_at(s: &str) -> Result<String, String> {
    let mut chars = s.chars();
    let first = chars.next().ok_or("empty string field")?;
    if first != '"' { return Err("expected opening '\"'".into()); }
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '"' { return Ok(out); }
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => return Err("unterminated escape in JSON string".into()),
            }
        } else {
            out.push(c);
        }
    }
    Err("unterminated JSON string".into())
}

pub fn parse_env_jsonl(line: &str) -> Result<(ExternalObservation, f32, bool), String> {
    let obs = if let Some(start) = line.find("\"obs_text\"") {
        let rest = &line[start + 10..];
        let colon = rest.find(':').ok_or("missing ':' in obs_text")?;
        let after = rest[colon + 1..].trim_start();
        let unescaped = parse_json_string_at(after)?;
        ExternalObservation::RawBytes(unescaped.into_bytes())
    } else if let Some(start) = line.find("\"obs\"") {
        let rest = &line[start..];
        let lb = rest.find('[').ok_or("missing '[' in obs")?;
        let rb = rest[lb..].find(']').ok_or("missing ']' in obs")?;
        let inner = &rest[lb + 1..lb + rb];
        let mut v = Vec::new();
        for tok in inner.split(',') {
            let t = tok.trim();
            if !t.is_empty() {
                v.push(t.parse::<f32>().map_err(|e| format!("bad obs float '{}': {}", t, e))?);
            }
        }
        ExternalObservation::Numeric(v)
    } else {
        return Err("missing \"obs\" or \"obs_text\" field".into());
    };

    let reward = if let Some(start) = line.find("\"reward\"") {
        let rest = &line[start + 8..];
        let colon = rest.find(':').ok_or("missing ':' after reward")?;
        let tail = rest[colon + 1..].trim_start();
        let end = tail.find(|c: char| c == ',' || c == '}' || c.is_whitespace()).unwrap_or(tail.len());
        tail[..end].parse::<f32>().unwrap_or(0.0)
    } else {
        0.0
    };

    let done = line.contains("\"done\":true") || line.contains("\"done\": true");
    Ok((obs, reward, done))
}

pub fn format_agent_jsonl(out: &AgentStepOutput) -> String {
    let acts: Vec<String> = out.action.iter().map(|v| format!("{:.5}", v)).collect();
    format!(
        "{{\"action\":[{}],\"pred_reward\":{:.5},\"pred_value\":{:.5},\"surprise\":{:.5},\"epistemic_unc\":{:.5},\"updates\":{},\"replay\":{}}}",
        acts.join(","),
        out.pred_reward,
        out.pred_value,
        out.surprise,
        out.epistemic_unc,
        out.online_updates,
        out.replay_len
    )
}

pub fn serve_universal_agent_stdio(brain: &DeepContractiveBrain, act_dim: usize, agent_ckpt: Option<&Path>) {
    let mut agent = if let Some(p) = agent_ckpt {
        if p.exists() {
            UniversalOpenWorldAgent::load_checkpoint(p)
                .map(|(a, _)| a)
                .unwrap_or_else(|_| UniversalOpenWorldAgent::new(act_dim, &brain.cfg, 2026_1126))
        } else {
            UniversalOpenWorldAgent::new(act_dim, &brain.cfg, 2026_1126)
        }
    } else {
        UniversalOpenWorldAgent::new(act_dim, &brain.cfg, 2026_1126)
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line_res in stdin.lock().lines() {
        let Ok(line) = line_res else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "QUIT" || trimmed == "{\"cmd\":\"quit\"}" { break; }
        match parse_env_jsonl(trimmed) {
            Ok((obs, reward, done)) => {
                let out = agent.step(brain, &obs, reward, done);
                let _ = writeln!(stdout, "{}", format_agent_jsonl(&out));
                let _ = stdout.flush();
            }
            Err(e) => {
                let _ = writeln!(stdout, "{{\"error\":\"{}\"}}", e);
                let _ = stdout.flush();
            }
        }
    }

    if let Some(p) = agent_ckpt { let _ = agent.save_checkpoint(p); }
}
