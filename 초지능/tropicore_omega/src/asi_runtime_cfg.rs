//! Live Self-Optimizing Runtime Policy (`TropiCore-Omega ASI-Zero v14.0`).
//!
//! Compiled directly into `tropicore_omega` and governs LIVE:
//!   1. Banach Contractive relaxation damping (`contractive_gamma`)
//!   2. Surprise-gated online learning-rate schedule (`adaptive_online_lr`)
//!   3. 10-Expert Online Convex Meta-Mixer prior `(w_mem_scale, w_mem_cap, use_log_odds)` (`meta_mixer_prior`)

#[inline]
pub fn contractive_gamma() -> f32 {
    0.58f32
}

#[inline]
pub fn adaptive_online_lr(base_lr: f32, surprise_bits: f32) -> f32 {
    let boost = 1.0 + 0.14f32 * (surprise_bits - 3.0f32).clamp(-1.0, 4.0);
    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)
}

#[inline]
pub fn meta_mixer_prior() -> (f32, f32, bool) {
    (0.85f32, 0.72f32, true)
}
