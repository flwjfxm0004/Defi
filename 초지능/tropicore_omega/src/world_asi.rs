//! Tier-2 Nonlinear Neural-Ensemble Continuous World Model + Active Epistemic Curiosity
//! + Online Non-Stationary Domain-Shift Adaptation (`TropiCore-Omega v10.0`).
//!
//! Dismantles the linear 6-parameter kinematics ceiling of v9.0:
//!   - Environment (`NonlinearVortexPhysicsEnv`):
//!       * Secret continuous 127-deg actuator rotation (and mid-flight +55-deg fault shift to 182-deg!)
//!       * NONLINEAR quadratic aerodynamic drag (`-0.16 * ||v|| * v`)
//!       * NONLINEAR swirling vortex force field (`0.26 * (-y, x) / (1 + 0.35 * r^2)`)
//!       * NONLINEAR repulsive obstacle force field (formula & radius NEVER given to the dynamics learner!)
//!       * Stochastic Gaussian actuator noise
//!   - Learner (`EnsembleNeuralWorldModel` — ZERO hardcoded physics equations!):
//!       * Ensemble of 2 independent Neural Residual-Dynamics MLPs (`10 -> 28 -> 4`, tanh hidden)
//!       * Active Epistemic Curiosity Exploration: selects actions maximizing ensemble disagreement
//!         `||f_1(s, u) - f_2(s, u)||^2` (true Active Inference / Optimal Experimental Design!)
//!       * Continuous CEM Shooting Planner calling live `evolved_policy::cem_step_cost(...)`
//!       * Surprise-Triggered Test-Time Online Adaptation: detects sudden +55-deg actuator shift
//!         via live prediction surprise and adapts the neural ensemble online to recover control!

use std::time::Instant;

use crate::asi_core::{OracleTier, UniversalEventStream};
use crate::core::UniversalEnergyCore;
use crate::evolved_policy::cem_step_cost;
use crate::memory::OMEGA_DIM;

const IN_DIM: usize = 10;
const HID_DIM: usize = 28;
const OUT_DIM: usize = 4;

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rand_uniform(rng: &mut u64) -> f64 {
    (splitmix64(rng) >> 11) as f64 / ((1u64 << 53) as f64)
}

fn rand_normal(rng: &mut u64) -> f64 {
    let u1 = rand_uniform(rng).clamp(1e-12, 1.0 - 1e-12);
    let u2 = rand_uniform(rng);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Nonlinear, stochastic, swirling-vortex + repulsive-obstacle continuous physics world.
#[derive(Clone, Debug)]
pub struct NonlinearVortexPhysicsEnv {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub gx: f64,
    pub gy: f64,
    pub ox: f64,
    pub oy: f64,
    pub obs_radius: f64,
    pub secret_angle: f64,
    pub vortex_strength: f64,
    pub noise_std: f64,
    pub dt: f64,
    pub steps: usize,
    pub max_steps: usize,
    pub rng: u64,
}

impl NonlinearVortexPhysicsEnv {
    pub fn new(seed: u64, secret_angle: f64, noise_std: f64, obs_radius: f64) -> Self {
        let mut rng = seed ^ 0xA510_1000_7777;
        let gx = -3.0 + 6.0 * rand_uniform(&mut rng);
        let gy = -3.0 + 6.0 * rand_uniform(&mut rng);
        let mut x = -3.0 + 6.0 * rand_uniform(&mut rng);
        let mut y = -3.0 + 6.0 * rand_uniform(&mut rng);
        for _ in 0..16 {
            if ((x - gx).powi(2) + (y - gy).powi(2)).sqrt() > 2.8 {
                break;
            }
            x = -3.0 + 6.0 * rand_uniform(&mut rng);
            y = -3.0 + 6.0 * rand_uniform(&mut rng);
        }
        let ox = 0.5 * (x + gx) + (rand_uniform(&mut rng) - 0.5) * 0.7;
        let oy = 0.5 * (y + gy) + (rand_uniform(&mut rng) - 0.5) * 0.7;
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            gx,
            gy,
            ox,
            oy,
            obs_radius,
            secret_angle,
            vortex_strength: 0.26,
            noise_std,
            dt: 0.18,
            steps: 0,
            max_steps: 64,
            rng,
        }
    }

    pub fn state_vec(&self) -> [f64; 8] {
        [self.x, self.y, self.vx, self.vy, self.gx, self.gy, self.ox, self.oy]
    }

    pub fn dist_to_goal(&self) -> f64 {
        ((self.x - self.gx).powi(2) + (self.y - self.gy).powi(2)).sqrt()
    }

    pub fn sample_spatial_field(&self) -> Vec<Vec<f64>> {
        let mut g = vec![vec![0.0f64; 8]; 8];
        for r in 0..8 {
            let py = -4.0 + (r as f64) * (8.0 / 7.0);
            for c in 0..8 {
                let px = -4.0 + (c as f64) * (8.0 / 7.0);
                let dg = ((px - self.gx).powi(2) + (py - self.gy).powi(2)).sqrt().max(0.1);
                let dobs = ((px - self.ox).powi(2) + (py - self.oy).powi(2)).sqrt().max(0.1);
                g[r][c] = (1.0 / dg) - (self.obs_radius / dobs);
            }
        }
        g
    }

    pub fn step(&mut self, u: [f64; 2]) -> ([f64; 8], f64, bool) {
        let u0 = u[0].clamp(-1.0, 1.0);
        let u1 = u[1].clamp(-1.0, 1.0);
        let c = self.secret_angle.cos();
        let s = self.secret_angle.sin();
        let fx = (c * u0 - s * u1) * 1.55 + self.noise_std * rand_normal(&mut self.rng);
        let fy = (s * u0 + c * u1) * 1.55 + self.noise_std * rand_normal(&mut self.rng);

        // NONLINEAR swirling vortex field (position-coupled cross force!)
        let r2 = self.x * self.x + self.y * self.y;
        let v_scale = self.vortex_strength / (1.0 + 0.35 * r2);
        let vfx = -self.y * v_scale;
        let vfy = self.x * v_scale;

        // NONLINEAR repulsive obstacle force field
        let dxo = self.x - self.ox;
        let dyo = self.y - self.oy;
        let d_obs = (dxo * dxo + dyo * dyo).sqrt().max(1e-4);
        let (rx, ry) = if d_obs < self.obs_radius * 1.65 {
            let push = ((self.obs_radius * 1.65 - d_obs) / self.obs_radius).min(2.4) * 1.65;
            (push * dxo / d_obs, push * dyo / d_obs)
        } else {
            (0.0, 0.0)
        };

        // NONLINEAR quadratic + linear aerodynamic drag
        let v_norm = (self.vx * self.vx + self.vy * self.vy).sqrt();
        let drag_factor = (1.0 - 0.10 - 0.16 * v_norm * self.dt).clamp(0.45, 0.95);

        self.vx = (self.vx * drag_factor + (fx + vfx + rx) * self.dt).clamp(-3.5, 3.5);
        self.vy = (self.vy * drag_factor + (fy + vfy + ry) * self.dt).clamp(-3.5, 3.5);
        self.x = (self.x + self.vx * self.dt).clamp(-5.0, 5.0);
        self.y = (self.y + self.vy * self.dt).clamp(-5.0, 5.0);
        self.steps += 1;

        let dist = self.dist_to_goal();
        let hit_obs = d_obs < self.obs_radius;
        let reached = dist < 0.38;
        let reward = -0.25 * dist - if hit_obs { 1.5 } else { 0.0 } + if reached { 5.0 } else { 0.0 };
        let done = reached || self.steps >= self.max_steps;
        (self.state_vec(), reward, done)
    }
}

/// Single General Residual Neural Dynamics MLP (`10 -> 28 -> 4` + learnable `W_lin` skip) — ZERO hardcoded physics!
#[derive(Clone, Debug)]
struct DynamicsMlp {
    w1: [f64; HID_DIM * IN_DIM],
    b1: [f64; HID_DIM],
    w2: [f64; OUT_DIM * HID_DIM],
    w_lin: [f64; OUT_DIM * IN_DIM],
    b2: [f64; OUT_DIM],
}

impl DynamicsMlp {
    fn new(seed: u64) -> Self {
        let mut rng = seed ^ 0xD1A0_5555_9999;
        let s1 = (1.0 / IN_DIM as f64).sqrt() * 0.65;
        let s2 = (1.0 / HID_DIM as f64).sqrt() * 0.20;
        let mut w1 = [0.0f64; HID_DIM * IN_DIM];
        let mut b1 = [0.0f64; HID_DIM];
        let mut w2 = [0.0f64; OUT_DIM * HID_DIM];
        let w_lin = [0.0f64; OUT_DIM * IN_DIM]; // initialized to ZERO (zero physics priors!)
        let b2 = [0.0f64; OUT_DIM];
        for v in w1.iter_mut() {
            *v = (rand_uniform(&mut rng) * 2.0 - 1.0) * s1;
        }
        for v in b1.iter_mut() {
            *v = (rand_uniform(&mut rng) * 2.0 - 1.0) * 0.05;
        }
        for v in w2.iter_mut() {
            *v = (rand_uniform(&mut rng) * 2.0 - 1.0) * s2;
        }
        Self { w1, b1, w2, w_lin, b2 }
    }

    #[inline]
    fn encode_input(s: &[f64; 8], u: [f64; 2]) -> [f64; IN_DIM] {
        let dxo = s[0] - s[6];
        let dyo = s[1] - s[7];
        let dobs = (dxo * dxo + dyo * dyo).sqrt().max(0.05);
        let v_norm = (s[2] * s[2] + s[3] * s[3]).sqrt();
        [
            s[0] * 0.25,
            s[1] * 0.25,
            s[2] * 0.35,
            s[3] * 0.35,
            (dxo / (dobs + 0.35)).clamp(-1.5, 1.5),
            (dyo / (dobs + 0.35)).clamp(-1.5, 1.5),
            (1.0 / (0.45 + dobs)).clamp(0.0, 2.0),
            u[0],
            u[1],
            v_norm * 0.35,
        ]
    }

    #[inline]
    fn forward(&self, x_in: &[f64; IN_DIM]) -> ([f64; OUT_DIM], [f64; HID_DIM]) {
        let mut h = [0.0f64; HID_DIM];
        for r in 0..HID_DIM {
            let row = r * IN_DIM;
            let mut acc = self.b1[r];
            for c in 0..IN_DIM {
                acc += self.w1[row + c] * x_in[c];
            }
            h[r] = acc.tanh();
        }
        let mut out = [0.0f64; OUT_DIM];
        for o in 0..OUT_DIM {
            let row = o * HID_DIM;
            let lrow = o * IN_DIM;
            let mut acc = self.b2[o];
            for c in 0..HID_DIM {
                acc += self.w2[row + c] * h[c];
            }
            for c in 0..IN_DIM {
                acc += self.w_lin[lrow + c] * x_in[c];
            }
            out[o] = acc;
        }
        (out, h)
    }

    #[inline]
    fn predict_next(&self, s: &[f64; 8], u: [f64; 2]) -> [f64; 4] {
        let x_in = Self::encode_input(s, u);
        let (delta, _) = self.forward(&x_in);
        [
            (s[0] + delta[0]).clamp(-5.0, 5.0),
            (s[1] + delta[1]).clamp(-5.0, 5.0),
            (s[2] + delta[2]).clamp(-3.5, 3.5),
            (s[3] + delta[3]).clamp(-3.5, 3.5),
        ]
    }

    fn train_step(&mut self, s: &[f64; 8], u: [f64; 2], s_next: &[f64; 8], lr: f64) -> f64 {
        let x_in = Self::encode_input(s, u);
        let (pred_d, h) = self.forward(&x_in);
        let tgt_d = [s_next[0] - s[0], s_next[1] - s[1], s_next[2] - s[2], s_next[3] - s[3]];
        let mut err = [0.0f64; OUT_DIM];
        let mut mse = 0.0;
        for o in 0..OUT_DIM {
            err[o] = (pred_d[o] - tgt_d[o]).clamp(-2.0, 2.0);
            mse += err[o] * err[o];
        }
        mse *= 0.25;

        let mut dh = [0.0f64; HID_DIM];
        for o in 0..OUT_DIM {
            let e = err[o];
            let row = o * HID_DIM;
            let lrow = o * IN_DIM;
            self.b2[o] -= lr * e;
            for c in 0..HID_DIM {
                dh[c] += e * self.w2[row + c];
                self.w2[row + c] = (self.w2[row + c] * (1.0 - 1e-4) - lr * e * h[c]).clamp(-4.0, 4.0);
            }
            for c in 0..IN_DIM {
                self.w_lin[lrow + c] = (self.w_lin[lrow + c] * (1.0 - 1e-4) - lr * 1.2 * e * x_in[c]).clamp(-4.0, 4.0);
            }
        }
        for r in 0..HID_DIM {
            let dp = (dh[r] * (1.0 - h[r] * h[r])).clamp(-2.0, 2.0);
            self.b1[r] -= lr * dp;
            let row = r * IN_DIM;
            for c in 0..IN_DIM {
                self.w1[row + c] = (self.w1[row + c] * (1.0 - 1e-4) - lr * dp * x_in[c]).clamp(-4.0, 4.0);
            }
        }
        mse
    }
}

/// 2-MLP Nonlinear Neural Ensemble World Model with Epistemic Disagreement & Online Surprise Adaptation!
#[derive(Clone, Debug)]
pub struct EnsembleNeuralWorldModel {
    m0: DynamicsMlp,
    m1: DynamicsMlp,
    pub transitions_seen: usize,
    pub epistemic_probes_taken: usize,
    pub surprise_adapt_triggers: usize,
}

impl EnsembleNeuralWorldModel {
    pub fn new(seed: u64) -> Self {
        Self {
            m0: DynamicsMlp::new(seed ^ 0x1111_2222),
            m1: DynamicsMlp::new(seed ^ 0x9999_AAAA),
            transitions_seen: 0,
            epistemic_probes_taken: 0,
            surprise_adapt_triggers: 0,
        }
    }

    #[inline]
    pub fn predict_with_uncertainty(&self, s: &[f64; 8], u: [f64; 2]) -> ([f64; 4], f64) {
        let p0 = self.m0.predict_next(s, u);
        let p1 = self.m1.predict_next(s, u);
        let mean = [
            0.5 * (p0[0] + p1[0]),
            0.5 * (p0[1] + p1[1]),
            0.5 * (p0[2] + p1[2]),
            0.5 * (p0[3] + p1[3]),
        ];
        let dis = ((p0[0] - p1[0]).powi(2)
            + (p0[1] - p1[1]).powi(2)
            + (p0[2] - p1[2]).powi(2)
            + (p0[3] - p1[3]).powi(2))
            .sqrt();
        (mean, dis)
    }

    pub fn train_transition(&mut self, s: &[f64; 8], u: [f64; 2], s_next: &[f64; 8], lr: f64) -> f64 {
        let l0 = self.m0.train_step(s, u, s_next, lr);
        let l1 = self.m1.train_step(s, u, s_next, lr * 1.05);
        self.transitions_seen += 1;
        0.5 * (l0 + l1)
    }

    /// Active Epistemic Curiosity Action Selection (ZERO hardcoded probe schedule!):
    /// Samples candidate continuous actions and executes the action maximizing ensemble disagreement `||f_0 - f_1||`!
    pub fn choose_active_curiosity_action(&mut self, s: &[f64; 8], rng: &mut u64) -> [f64; 2] {
        let mut best_u = [0.0, 0.0];
        let mut best_dis = -1.0;
        for _ in 0..12 {
            let u = [rand_uniform(rng) * 2.0 - 1.0, rand_uniform(rng) * 2.0 - 1.0];
            let (_, dis) = self.predict_with_uncertainty(s, u);
            if dis > best_dis {
                best_dis = dis;
                best_u = u;
            }
        }
        self.epistemic_probes_taken += 1;
        best_u
    }

    /// Continuous CEM Receding-Horizon Shooting Planner using LIVE `evolved_policy::cem_step_cost`!
    pub fn plan_cem_action(&self, s0: &[f64; 8], rng: &mut u64) -> [f64; 2] {
        const H: usize = 7;
        const POP: usize = 28;
        const ELITES: usize = 5;
        let mut mean = [[0.0f64; 2]; H];
        let mut std = [[0.85f64; 2]; H];

        for _iter in 0..3 {
            let mut cands: Vec<(f64, [[f64; 2]; H])> = Vec::with_capacity(POP);
            for _ in 0..POP {
                let mut seq = [[0.0f64; 2]; H];
                let mut cur = *s0;
                let mut total_cost = 0.0;
                for t in 0..H {
                    let u0 = (mean[t][0] + std[t][0] * rand_normal(rng)).clamp(-1.0, 1.0);
                    let u1 = (mean[t][1] + std[t][1] * rand_normal(rng)).clamp(-1.0, 1.0);
                    seq[t] = [u0, u1];
                    let (nxt, dis) = self.predict_with_uncertainty(&cur, [u0, u1]);
                    cur[0] = nxt[0];
                    cur[1] = nxt[1];
                    cur[2] = nxt[2];
                    cur[3] = nxt[3];
                    let dg = ((cur[0] - cur[4]).powi(2) + (cur[1] - cur[5]).powi(2)).sqrt();
                    let dobs = ((cur[0] - cur[6]).powi(2) + (cur[1] - cur[7]).powi(2)).sqrt();
                    let vnorm = (cur[2] * cur[2] + cur[3] * cur[3]).sqrt();
                    total_cost += cem_step_cost(dg, dobs, vnorm, u0 * u0 + u1 * u1, dis, t + 1 == H);
                }
                cands.push((total_cost, seq));
            }
            cands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for t in 0..H {
                let (mut m0, mut m1) = (0.0, 0.0);
                for e in 0..ELITES {
                    m0 += cands[e].1[t][0];
                    m1 += cands[e].1[t][1];
                }
                m0 /= ELITES as f64;
                m1 /= ELITES as f64;
                let (mut v0, mut v1) = (0.0, 0.0);
                for e in 0..ELITES {
                    v0 += (cands[e].1[t][0] - m0).powi(2);
                    v1 += (cands[e].1[t][1] - m1).powi(2);
                }
                mean[t] = [m0, m1];
                std[t] = [
                    (v0 / ELITES as f64).sqrt().clamp(0.12, 0.85),
                    (v1 / ELITES as f64).sqrt().clamp(0.12, 0.85),
                ];
            }
        }
        mean[0]
    }
}

#[derive(Clone, Debug)]
pub struct NonlinearWorldAsiReport {
    pub tier: OracleTier,
    pub secret_angle_deg: f64,
    pub fault_shift_deg: f64,
    pub compression_ratio: f64,
    pub random_success: usize,
    pub ensemble_cem_success: usize,
    pub eval_episodes: usize,
    pub fault_frozen_success: usize,
    pub fault_online_adapted_success: usize,
    pub fault_eval_episodes: usize,
    pub epistemic_probes_taken: usize,
    pub surprise_adapt_triggers: usize,
    pub brain_stream_norm: f64,
    pub wall_ms: f64,
}

/// Run the full v10.0 Nonlinear Neural-Ensemble Continuous World Benchmark:
///   1. Measure Untrained Baseline MSE & Random Policy Success on 16 unseen nonlinear vortex+obstacle worlds.
///   2. Pure Active Epistemic Curiosity Exploration (ZERO hardcoded probe schedule!) + replay SGD.
///   3. Frozen OOD Evaluation on 16 UNSEEN nonlinear vortex+obstacle worlds (`Random` vs `Neural Ensemble CEM`).
///   4. Non-Stationary Domain Shift (`+55 deg` sudden actuator rotation fault!):
///      Compare `Frozen Ensemble` vs `Online Surprise-Adapting Ensemble` on 12 unseen faulted worlds!
pub fn run_nonlinear_neural_world_asi(
    core: &mut UniversalEnergyCore,
    seed: u64,
    noise_std: f64,
    obs_radius: f64,
) -> NonlinearWorldAsiReport {
    let t0 = Instant::now();
    let secret_angle = 2.216_568f64; // 127.0 deg
    let fault_shift = 0.959_931f64;  // +55.0 deg sudden non-stationary actuator fault -> 182.0 deg!
    let mut rng = seed ^ 0xE5EB_1000_3333;
    let mut wm = EnsembleNeuralWorldModel::new(seed);
    let wm_untrained = wm.clone();
    let eval_eps = 16usize;
    let fault_eps = 12usize;

    // 1. Random Policy Baseline Success on 16 unseen nonlinear vortex+obstacle worlds
    let mut random_success = 0usize;
    for ep in 0..eval_eps {
        let mut env = NonlinearVortexPhysicsEnv::new(seed + 100 + ep as u64, secret_angle, noise_std, obs_radius);
        while env.steps < env.max_steps {
            let u = [rand_uniform(&mut rng) * 2.0 - 1.0, rand_uniform(&mut rng) * 2.0 - 1.0];
            let (_, _, done) = env.step(u);
            if done {
                if env.dist_to_goal() < 0.38 {
                    random_success += 1;
                }
                break;
            }
        }
    }

    // 2. Active Epistemic Curiosity Exploration + Replay Training (ZERO physics priors!)
    let mut replay: Vec<([f64; 8], [f64; 2], [f64; 8])> = Vec::with_capacity(1024);
    let mut brain_traj: Vec<(Vec<f64>, Vec<f64>, f64)> = Vec::with_capacity(64);
    let mut sample_field: Vec<Vec<f64>> = Vec::new();

    for ep in 0..24usize {
        let mut env = NonlinearVortexPhysicsEnv::new(seed + 5000 + ep as u64, secret_angle, noise_std, obs_radius);
        if ep == 0 {
            sample_field = env.sample_spatial_field();
        }
        for _ in 0..36usize {
            let s = env.state_vec();
            let u = wm.choose_active_curiosity_action(&s, &mut rng);
            let (sn, r, done) = env.step(u);
            wm.train_transition(&s, u, &sn, 0.025);
            replay.push((s, u, sn));
            if brain_traj.len() < 48 {
                brain_traj.push((s.to_vec(), u.to_vec(), r));
            }
            if done {
                break;
            }
        }
    }
    // 8 fast shuffled replay consolidation epochs with smooth decaying lr (0.025 -> 0.005)
    let mut order: Vec<usize> = (0..replay.len()).collect();
    for epoch in 0..8usize {
        for i in (1..order.len()).rev() {
            let j = (splitmix64(&mut rng) as usize) % (i + 1);
            order.swap(i, j);
        }
        let lr = 0.025 * (0.80f64).powi(epoch as i32);
        for &idx in &order {
            let (s, u, sn) = replay[idx];
            wm.train_transition(&s, u, &sn, lr);
        }
    }

    // Encode continuous trajectory + spatial potential field into Unified `UniversalEnergyCore`!
    let c_toks = UniversalEventStream::from_continuous_trajectory(&brain_traj, 36);
    let sp_toks = UniversalEventStream::from_spatial_field(&sample_field, 32);
    let z_phys = core.encode_raw_tokens(&c_toks);
    let brain_stream_norm: f64 = z_phys.iter().map(|v| v.abs()).sum::<f64>() / OMEGA_DIM as f64;

    // 3. Frozen Evaluation on 16 UNSEEN Nonlinear Vortex+Obstacle Worlds
    //    Apple-to-apple Solomonoff compression: compare `wm_untrained` vs `wm` on the EXACT SAME eval transitions!
    let mut ensemble_cem_success = 0usize;
    let mut untrained_mse_sum = 0.0;
    let mut learned_mse_sum = 0.0;
    let mut eval_trans_cnt = 0usize;
    for ep in 0..eval_eps {
        let mut env = NonlinearVortexPhysicsEnv::new(seed + 90_000 + ep as u64, secret_angle, noise_std, obs_radius);
        while env.steps < env.max_steps {
            let s = env.state_vec();
            let u = wm.plan_cem_action(&s, &mut rng);
            let (sn, _, done) = env.step(u);
            let (p0, _) = wm_untrained.predict_with_uncertainty(&s, u);
            let (p1, _) = wm.predict_with_uncertainty(&s, u);
            untrained_mse_sum += 0.25
                * ((p0[0] - sn[0]).powi(2)
                    + (p0[1] - sn[1]).powi(2)
                    + (p0[2] - sn[2]).powi(2)
                    + (p0[3] - sn[3]).powi(2));
            learned_mse_sum += 0.25
                * ((p1[0] - sn[0]).powi(2)
                    + (p1[1] - sn[1]).powi(2)
                    + (p1[2] - sn[2]).powi(2)
                    + (p1[3] - sn[3]).powi(2));
            eval_trans_cnt += 1;
            if done {
                if env.dist_to_goal() < 0.38 {
                    ensemble_cem_success += 1;
                }
                break;
            }
        }
    }
    let untrained_mse = (untrained_mse_sum / eval_trans_cnt.max(1) as f64).max(1e-6);
    let learned_mse = (learned_mse_sum / eval_trans_cnt.max(1) as f64).max(1e-7);

    // 4. Non-Stationary Domain-Shift Test (+55 deg SUDDEN ACTUATOR ROTATION FAULT!)
    let faulted_angle = secret_angle + fault_shift;
    // 4A. Frozen Ensemble (NO test-time adaptation)
    let mut fault_frozen_success = 0usize;
    for ep in 0..fault_eps {
        let mut env = NonlinearVortexPhysicsEnv::new(seed + 150_000 + ep as u64, faulted_angle, noise_std, obs_radius);
        while env.steps < env.max_steps {
            let s = env.state_vec();
            let u = wm.plan_cem_action(&s, &mut rng);
            let (_, _, done) = env.step(u);
            if done {
                if env.dist_to_goal() < 0.38 {
                    fault_frozen_success += 1;
                }
                break;
            }
        }
    }

    // 4B. Surprise-Triggered Online Adapting Ensemble (adapts live during faulted episodes!)
    let mut wm_adapt = wm.clone();
    let mut fault_online_adapted_success = 0usize;
    let mut recent: Vec<([f64; 8], [f64; 2], [f64; 8])> = Vec::with_capacity(64);
    for ep in 0..fault_eps {
        let mut env = NonlinearVortexPhysicsEnv::new(seed + 150_000 + ep as u64, faulted_angle, noise_std, obs_radius);
        while env.steps < env.max_steps {
            let s = env.state_vec();
            let u = wm_adapt.plan_cem_action(&s, &mut rng);
            let (sn, _, done) = env.step(u);
            let (p, _) = wm_adapt.predict_with_uncertainty(&s, u);
            let step_mse = 0.25
                * ((p[0] - sn[0]).powi(2)
                    + (p[1] - sn[1]).powi(2)
                    + (p[2] - sn[2]).powi(2)
                    + (p[3] - sn[3]).powi(2));
            if recent.len() >= 48 {
                recent.remove(0);
            }
            recent.push((s, u, sn));
            // Surprise trigger -> rapid online gradient re-identification of the faulted actuator!
            if step_mse > 0.015 {
                wm_adapt.surprise_adapt_triggers += 1;
                for _ in 0..3 {
                    for &(rs, ru, rsn) in recent.iter().rev().take(24) {
                        wm_adapt.train_transition(&rs, ru, &rsn, 0.045);
                    }
                }
            }
            if done {
                if env.dist_to_goal() < 0.38 {
                    fault_online_adapted_success += 1;
                }
                break;
            }
        }
    }

    let mastery = ensemble_cem_success as f64 / eval_eps as f64;
    core.train_universal_stream_value_step(&c_toks, mastery, 0.025);
    core.train_universal_stream_value_step(&sp_toks, mastery, 0.02);

    NonlinearWorldAsiReport {
        tier: OracleTier::Tier2CausalWorld,
        secret_angle_deg: secret_angle.to_degrees(),
        fault_shift_deg: fault_shift.to_degrees(),
        compression_ratio: untrained_mse / learned_mse,
        random_success,
        ensemble_cem_success,
        eval_episodes: eval_eps,
        fault_frozen_success,
        fault_online_adapted_success,
        fault_eval_episodes: fault_eps,
        epistemic_probes_taken: wm.epistemic_probes_taken,
        surprise_adapt_triggers: wm_adapt.surprise_adapt_triggers,
        brain_stream_norm,
        wall_ms: t0.elapsed().as_secs_f64() * 1000.0,
    }
}
