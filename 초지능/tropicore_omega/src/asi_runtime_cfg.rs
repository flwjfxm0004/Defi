//! Live Self-Optimizing Runtime Policy & Mixing Law (`TropiCore-Omega ASI-Zero`).
//!
//! This module is compiled directly into `tropicore_omega` and governs LIVE:
//!   1. Parametric Deep Brain vs Tropical Episodic Memory probability blending (`blend_byte_probs`)
//!   2. Online learning-rate & surprise-gated adaptation scaling (`adaptive_online_lr`)
//!   3. Contractive relaxation damping (`contractive_gamma`)
//!
//! `src/self_optimizer.rs` reads THIS exact file, proposes real algorithmic mutations,
//! compiles candidates via `rustc -O -D warnings`, benchmarks them on REAL held-out files
//! (actual measured `bits/byte` compression + actual throughput), and under `--apply`
//! atomically patches this file with SHA-256 ledger provenance & rollback safety!

/// Contractive equilibrium mixing factor `gamma in (0.15, 0.85)` for `z_{k+1} = gamma*tanh(Wz+b) + (1-gamma)*x`.
#[inline]
pub fn contractive_gamma() -> f32 {
    0.58
}

/// Adaptive online learning-rate multiplier driven by live Shannon surprise (`bits` = `-log2 P(true_byte)`).
/// Scales up gently on novel real-world regimes while clamping against instability.
#[inline]
pub fn adaptive_online_lr(base_lr: f32, surprise_bits: f32) -> f32 {
    let boost = 1.0 + 0.14 * (surprise_bits - 3.0).clamp(-1.0, 4.0);
    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)
}

/// Blend parametric Deep Contractive Brain distribution `p_brain` with Non-Parametric Tropical
/// Episodic Memory distribution `p_mem` given retrieval confidence `mem_conf in [0, 1]`.
#[inline]
pub fn blend_byte_probs(p_brain: f32, p_mem: f32, mem_conf: f32) -> f32 {
    let w_mem = (0.42 * mem_conf).clamp(0.0, 0.65);
    ((1.0 - w_mem) * p_brain + w_mem * p_mem).max(1e-7)
}
