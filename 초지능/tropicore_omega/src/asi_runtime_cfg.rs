//! Evolved Live Structural AST Runtime Policy (`TropiCore-Omega v16.0`, `log/taper`).

#[inline]
pub fn contractive_gamma() -> f32 {
    0.6512f32
}

#[inline]
pub fn gamma_depth_taper() -> bool {
    true
}

#[inline]
pub fn adaptive_online_lr(base_lr: f32, surprise_bits: f32) -> f32 {
    let diff = surprise_bits - (1.9287f32);
    let boost = 1.0 + (0.0300f32) * 1.4 * diff.signum() * (1.0 + diff.abs()).ln();
    (base_lr * boost).clamp(base_lr * 0.4, base_lr * 1.8)
}

#[inline]
pub fn meta_mixer_prior() -> (f32, f32, bool) {
    (1.1099f32, 0.8542f32, true)
}
