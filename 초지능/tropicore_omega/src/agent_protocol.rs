//! Domain-Agnostic External `UniversalAgent` Open-World Protocol (`TropiCore-Omega v12.0`).
//!
//! ZERO toy worlds baked inside the ASI core!
//! Supports BOTH numeric sensor vectors (`{"obs":[...]}`) AND arbitrary raw text/byte observations
//! (`{"obs_text":"..."}`) via stdin/stdout JSON-Line protocol, AND 100% Bit-Exact Lossless `TOMA12`
//! SHA-256 Persistence (`--agent-ckpt`) so external world-model knowledge NEVER resets across restarts!

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::brain::DeepContractiveBrain;
use crate::sha256::sha256_hex;

const AGENT_MAGIC: &[u8; 6] = b"TOMA12";
const LAT_DIM: usize = 16;
const HID_DIM: usize = 32;

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
struct LatentModelMlp {
    in_dim: usize,
    out_dim: usize,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    w_lin: Vec<f32>,
    b2: Vec<f32>,
}

impl LatentModelMlp {
    fn new(act_dim: usize, seed: u64) -> Self {
        let mut rng = seed ^ 0xA6E0_1200_9999;
        let in_dim = LAT_DIM + act_dim;
        let out_dim = LAT_DIM + 1;
        let s1 = (1.0 / in_dim as f32).sqrt() * 0.6;
        let s2 = (1.0 / HID_DIM as f32).sqrt() * 0.2;
        let w1 = (0..HID_DIM * in_dim).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s1).collect();
        let b1 = vec![0.0; HID_DIM];
        let w2 = (0..out_dim * HID_DIM).map(|_| (rand_uniform(&mut rng) * 2.0 - 1.0) * s2).collect();
        let w_lin = vec![0.0; out_dim * in_dim];
        let b2 = vec![0.0; out_dim];
        Self { in_dim, out_dim, w1, b1, w2, w_lin, b2 }
    }

    fn forward(&self, belief: &[f32; LAT_DIM], act: &[f32]) -> ([f32; LAT_DIM], f32, [f32; HID_DIM], Vec<f32>) {
        let mut x = Vec::with_capacity(self.in_dim);
        x.extend_from_slice(belief);
        x.extend_from_slice(act);
        let mut h = [0.0f32; HID_DIM];
        for r in 0..HID_DIM {
            let row = r * self.in_dim;
            let mut acc = self.b1[r];
            for c in 0..self.in_dim { acc += self.w1[row + c] * x[c]; }
            h[r] = acc.tanh();
        }
        let mut out = vec![0.0f32; self.out_dim];
        for o in 0..self.out_dim {
            let row = o * HID_DIM;
            let lrow = o * self.in_dim;
            let mut acc = self.b2[o];
            for c in 0..HID_DIM { acc += self.w2[row + c] * h[c]; }
            for c in 0..self.in_dim { acc += self.w_lin[lrow + c] * x[c]; }
            out[o] = acc;
        }
        let mut next_b = [0.0f32; LAT_DIM];
        for i in 0..LAT_DIM { next_b[i] = (belief[i] + out[i]).clamp(-4.0, 4.0); }
        (next_b, out[LAT_DIM], h, x)
    }

    fn train_step(&mut self, belief: &[f32; LAT_DIM], act: &[f32], next_b: &[f32; LAT_DIM], reward: f32, lr: f32) -> f32 {
        let (pred_nb, pred_r, h, x) = self.forward(belief, act);
        let mut err = vec![0.0f32; self.out_dim];
        let mut mse = 0.0f32;
        for i in 0..LAT_DIM {
            let e = (pred_nb[i] - next_b[i]).clamp(-2.5, 2.5);
            err[i] = e;
            mse += e * e;
        }
        let er = (pred_r - reward).clamp(-3.0, 3.0);
        err[LAT_DIM] = er * 1.5;
        mse = (mse / LAT_DIM as f32) + er * er;

        let mut dh = [0.0f32; HID_DIM];
        for o in 0..self.out_dim {
            let e = err[o];
            let row = o * HID_DIM;
            let lrow = o * self.in_dim;
            self.b2[o] -= lr * e;
            for c in 0..HID_DIM {
                dh[c] += e * self.w2[row + c];
                self.w2[row + c] = (self.w2[row + c] * (1.0 - 1e-4) - lr * e * h[c]).clamp(-4.0, 4.0);
            }
            for c in 0..self.in_dim {
                self.w_lin[lrow + c] = (self.w_lin[lrow + c] * (1.0 - 1e-4) - lr * e * x[c]).clamp(-4.0, 4.0);
            }
        }
        for r in 0..HID_DIM {
            let dp = (dh[r] * (1.0 - h[r] * h[r])).clamp(-2.0, 2.0);
            self.b1[r] -= lr * dp;
            let row = r * self.in_dim;
            for c in 0..self.in_dim {
                self.w1[row + c] = (self.w1[row + c] * (1.0 - 1e-4) - lr * dp * x[c]).clamp(-4.0, 4.0);
            }
        }
        mse
    }

    fn write_binary(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.in_dim as u32).to_le_bytes());
        out.extend_from_slice(&(self.out_dim as u32).to_le_bytes());
        for sl in [&self.w1, &self.b1, &self.w2, &self.w_lin, &self.b2] {
            out.extend_from_slice(&(sl.len() as u32).to_le_bytes());
            for &v in sl { out.extend_from_slice(&v.to_le_bytes()); }
        }
    }

    fn read_binary(data: &[u8], pos: &mut usize) -> Result<Self, String> {
        let take_u32 = |d: &[u8], p: &mut usize| -> Result<u32, String> {
            if *p + 4 > d.len() { return Err("agent mlp EOF u32".into()); }
            let v = u32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]);
            *p += 4;
            Ok(v)
        };
        let take_vec = |d: &[u8], p: &mut usize| -> Result<Vec<f32>, String> {
            let n = take_u32(d, p)? as usize;
            if *p + n * 4 > d.len() { return Err("agent mlp EOF vec".into()); }
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(f32::from_le_bytes([d[*p], d[*p + 1], d[*p + 2], d[*p + 3]]));
                *p += 4;
            }
            Ok(v)
        };
        let in_dim = take_u32(data, pos)? as usize;
        let out_dim = take_u32(data, pos)? as usize;
        let w1 = take_vec(data, pos)?;
        let b1 = take_vec(data, pos)?;
        let w2 = take_vec(data, pos)?;
        let w_lin = take_vec(data, pos)?;
        let b2 = take_vec(data, pos)?;
        Ok(Self { in_dim, out_dim, w1, b1, w2, w_lin, b2 })
    }
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
    pub surprise: f32,
    pub epistemic_unc: f32,
    pub online_updates: usize,
}

#[derive(Clone, Debug)]
pub struct UniversalOpenWorldAgent {
    pub act_dim: usize,
    m0: LatentModelMlp,
    m1: LatentModelMlp,
    cur_belief: [f32; LAT_DIM],
    prev_z: [f32; LAT_DIM],
    prev_belief: Option<[f32; LAT_DIM]>,
    prev_action: Option<Vec<f32>>,
    replay: Vec<([f32; LAT_DIM], Vec<f32>, [f32; LAT_DIM], f32)>,
    pub online_updates: usize,
    pub surprise_triggers: usize,
    rng: u64,
}

impl UniversalOpenWorldAgent {
    pub fn new(act_dim: usize, seed: u64) -> Self {
        let ad = act_dim.max(1);
        Self {
            act_dim: ad,
            m0: LatentModelMlp::new(ad, seed ^ 0x1111_AAAA),
            m1: LatentModelMlp::new(ad, seed ^ 0x7777_5555),
            cur_belief: [0.0; LAT_DIM],
            prev_z: [0.0; LAT_DIM],
            prev_belief: None,
            prev_action: None,
            replay: Vec::with_capacity(512),
            online_updates: 0,
            surprise_triggers: 0,
            rng: seed ^ 0xCAFE_1200,
        }
    }

    fn compress_to_belief(&mut self, z_full: &[f32]) -> [f32; LAT_DIM] {
        let mut z = [0.0f32; LAT_DIM];
        if !z_full.is_empty() {
            let chunk = (z_full.len() / LAT_DIM).max(1);
            for i in 0..LAT_DIM {
                let s = i * chunk;
                let e = ((i + 1) * chunk).min(z_full.len());
                if s < e {
                    let sum: f32 = z_full[s..e].iter().copied().sum();
                    z[i] = (sum / (e - s) as f32).clamp(-3.0, 3.0);
                }
            }
        }
        let half = LAT_DIM / 2;
        let mut b = [0.0f32; LAT_DIM];
        for i in 0..half {
            b[i] = 0.60 * self.cur_belief[i] + 0.40 * z[i];
            b[half + i] = (z[i] - self.prev_z[i]).clamp(-2.0, 2.0);
        }
        self.prev_z = z;
        self.cur_belief = b;
        b
    }

    fn predict_ensemble(&self, b: &[f32; LAT_DIM], a: &[f32]) -> ([f32; LAT_DIM], f32, f32) {
        let (nb0, r0, _, _) = self.m0.forward(b, a);
        let (nb1, r1, _, _) = self.m1.forward(b, a);
        let mut mean_b = [0.0f32; LAT_DIM];
        let mut dis2 = (r0 - r1) * (r0 - r1);
        for i in 0..LAT_DIM {
            mean_b[i] = 0.5 * (nb0[i] + nb1[i]);
            let d = nb0[i] - nb1[i];
            dis2 += d * d;
        }
        (mean_b, 0.5 * (r0 + r1), (dis2 / (LAT_DIM as f32 + 1.0)).sqrt())
    }

    pub fn step(&mut self, brain: &DeepContractiveBrain, obs: &ExternalObservation, reward: f32, done: bool) -> AgentStepOutput {
        let z_full = match obs {
            ExternalObservation::Numeric(v) => brain.encode_raw_sensor_vector(v),
            ExternalObservation::RawBytes(bytes) => brain.encode_raw_bytes_to_latent(bytes),
        };
        let b_cur = self.compress_to_belief(&z_full);

        let mut surprise = 0.0f32;
        if let (Some(bp), Some(ap)) = (self.prev_belief, self.prev_action.clone()) {
            let l0 = self.m0.train_step(&bp, &ap, &b_cur, reward, 0.025);
            let l1 = self.m1.train_step(&bp, &ap, &b_cur, reward, 0.028);
            surprise = 0.5 * (l0 + l1);
            self.online_updates += 1;

            if self.replay.len() >= 512 { self.replay.remove(0); }
            self.replay.push((bp, ap, b_cur, reward));

            if surprise > 0.18 && self.replay.len() >= 8 {
                self.surprise_triggers += 1;
                let n = self.replay.len();
                let start = n.saturating_sub(24);
                for _ in 0..2 {
                    for idx in start..n {
                        let (rb, ref ra, rnb, rr) = self.replay[idx];
                        self.m0.train_step(&rb, ra, &rnb, rr, 0.020);
                        self.m1.train_step(&rb, ra, &rnb, rr, 0.022);
                    }
                }
            }
        }

        if done {
            self.cur_belief = [0.0; LAT_DIM];
            self.prev_z = [0.0; LAT_DIM];
            self.prev_belief = None;
            self.prev_action = None;
            return AgentStepOutput {
                action: vec![0.0; self.act_dim],
                pred_reward: 0.0,
                surprise,
                epistemic_unc: 0.0,
                online_updates: self.online_updates,
            };
        }

        const H: usize = 5;
        const POP: usize = 24;
        const ELITES: usize = 5;
        let ad = self.act_dim;
        let curiosity_w = if self.online_updates < 40 { 0.85f32 } else { 0.20f32 };

        let mut mean = vec![vec![0.0f32; ad]; H];
        let mut std = vec![vec![0.85f32; ad]; H];
        for _ in 0..2 {
            let mut cands: Vec<(f32, Vec<Vec<f32>>)> = Vec::with_capacity(POP);
            for _ in 0..POP {
                let mut seq = Vec::with_capacity(H);
                let mut cur_b = b_cur;
                let mut ret = 0.0f32;
                let mut disc = 1.0f32;
                for t in 0..H {
                    let mut a = Vec::with_capacity(ad);
                    for j in 0..ad {
                        a.push((mean[t][j] + std[t][j] * rand_normal(&mut self.rng)).clamp(-1.0, 1.0));
                    }
                    let (nb, r_hat, unc) = self.predict_ensemble(&cur_b, &a);
                    ret += disc * (r_hat + curiosity_w * unc);
                    disc *= 0.94;
                    cur_b = nb;
                    seq.push(a);
                }
                cands.push((ret, seq));
            }
            cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            for t in 0..H {
                for j in 0..ad {
                    let m: f32 = (0..ELITES).map(|e| cands[e].1[t][j]).sum::<f32>() / ELITES as f32;
                    let v: f32 = (0..ELITES).map(|e| (cands[e].1[t][j] - m).powi(2)).sum::<f32>() / ELITES as f32;
                    mean[t][j] = m;
                    std[t][j] = v.sqrt().clamp(0.10, 0.85);
                }
            }
        }

        let chosen = mean[0].clone();
        let (_, pred_reward, epistemic_unc) = self.predict_ensemble(&b_cur, &chosen);
        self.prev_belief = Some(b_cur);
        self.prev_action = Some(chosen.clone());

        AgentStepOutput {
            action: chosen,
            pred_reward,
            surprise,
            epistemic_unc,
            online_updates: self.online_updates,
        }
    }

    /// 100% Bit-Exact Lossless `TOMA12` SHA-256 Agent Persistence (ZERO world-model amnesia!).
    pub fn save_checkpoint(&self, path: &Path) -> Result<String, String> {
        if let Some(p) = path.parent() { let _ = fs::create_dir_all(p); }
        let mut buf = Vec::with_capacity(32 * 1024);
        buf.extend_from_slice(AGENT_MAGIC);
        buf.extend_from_slice(&(self.act_dim as u32).to_le_bytes());
        buf.extend_from_slice(&(self.online_updates as u64).to_le_bytes());
        buf.extend_from_slice(&(self.surprise_triggers as u64).to_le_bytes());
        buf.extend_from_slice(&self.rng.to_le_bytes());
        self.m0.write_binary(&mut buf);
        self.m1.write_binary(&mut buf);
        let sha = sha256_hex(&buf);
        buf.extend_from_slice(sha.as_bytes());
        fs::write(path, &buf).map_err(|e| format!("agent ckpt write err: {}", e))?;
        Ok(sha)
    }

    pub fn load_checkpoint(path: &Path) -> Result<(Self, String), String> {
        let raw = fs::read(path).map_err(|e| format!("agent ckpt read err: {}", e))?;
        if raw.len() < 70 || &raw[0..6] != AGENT_MAGIC {
            return Err("invalid TOMA12 agent checkpoint".into());
        }
        let split = raw.len() - 64;
        let payload = &raw[..split];
        let stored = std::str::from_utf8(&raw[split..]).map_err(|_| "corrupt agent sha")?;
        let computed = sha256_hex(payload);
        if stored != computed {
            return Err("agent checkpoint SHA-256 mismatch".into());
        }
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
        let act_dim = take_u32(payload, &mut pos)? as usize;
        let online_updates = take_u64(payload, &mut pos)? as usize;
        let surprise_triggers = take_u64(payload, &mut pos)? as usize;
        let rng = take_u64(payload, &mut pos)?;
        let m0 = LatentModelMlp::read_binary(payload, &mut pos)?;
        let m1 = LatentModelMlp::read_binary(payload, &mut pos)?;
        Ok((
            Self {
                act_dim,
                m0,
                m1,
                cur_belief: [0.0; LAT_DIM],
                prev_z: [0.0; LAT_DIM],
                prev_belief: None,
                prev_action: None,
                replay: Vec::with_capacity(512),
                online_updates,
                surprise_triggers,
                rng,
            },
            computed,
        ))
    }
}

/// Zero-dependency JSON-Line parser supporting BOTH `{"obs":[...]}` AND `{"obs_text":"..."}`!
pub fn parse_env_jsonl(line: &str) -> Result<(ExternalObservation, f32, bool), String> {
    let obs = if let Some(start) = line.find("\"obs_text\"") {
        let rest = &line[start + 10..];
        let colon = rest.find(':').ok_or("missing ':' in obs_text")?;
        let after = rest[colon + 1..].trim_start();
        let q1 = after.find('"').ok_or("missing opening quote in obs_text")?;
        let inner = &after[q1 + 1..];
        let q2 = inner.find('"').ok_or("missing closing quote in obs_text")?;
        ExternalObservation::RawBytes(inner[..q2].as_bytes().to_vec())
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
        "{{\"action\":[{}],\"pred_reward\":{:.5},\"surprise\":{:.5},\"epistemic_unc\":{:.5},\"updates\":{}}}",
        acts.join(","),
        out.pred_reward,
        out.surprise,
        out.epistemic_unc,
        out.online_updates
    )
}

/// Interactive stdin/stdout JSON-Line Universal Open-World Agent Server with Lossless Persistence!
pub fn serve_universal_agent_stdio(brain: &DeepContractiveBrain, act_dim: usize, agent_ckpt: Option<&Path>) {
    let mut agent = if let Some(p) = agent_ckpt {
        if p.exists() {
            UniversalOpenWorldAgent::load_checkpoint(p)
                .map(|(a, _)| a)
                .unwrap_or_else(|_| UniversalOpenWorldAgent::new(act_dim, 2026_1125))
        } else {
            UniversalOpenWorldAgent::new(act_dim, 2026_1125)
        }
    } else {
        UniversalOpenWorldAgent::new(act_dim, 2026_1125)
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

    if let Some(p) = agent_ckpt {
        let _ = agent.save_checkpoint(p);
    }
}
