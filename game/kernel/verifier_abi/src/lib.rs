#![no_std]
#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

extern crate alloc;

use alloc::vec::Vec;
use core::marker::PhantomData;
use math_q32::{Q32, QVec3};
use serde::{Deserialize, Serialize};

mod private {
    pub trait Sealed {}
}

// ============================================================================
// 1. Zero-Cost Typestate Contract #1: Packet Boundary Validation
// ============================================================================

pub trait PacketState: private::Sealed {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unverified;
impl private::Sealed for Unverified {}
impl PacketState for Unverified {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verified;
impl private::Sealed for Verified {}
impl PacketState for Verified {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RawWireInput {
    pub tick: u64,
    pub move_dir_milli: [i32; 3],
    pub aim_yaw_turn_milli: i32,
    pub aim_pitch_turn_milli: i32,
    pub action_mask: u32,
    pub active_weapon_slot: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketReject {
    MoveComponentOutOfBounds,
    AimPitchOutOfBounds,
    InvalidWeaponSlot,
    ReservedActionBitsSet,
}

/// Typestate-wrapped network input packet (`#[repr(C)]` + zero-cost `PhantomData`).
///
/// Attempting to convert an `Unverified` packet into `InputFrame` is rejected at compile time:
/// ```compile_fail
/// use verifier_abi::{Packet, RawWireInput, Unverified};
/// let raw = RawWireInput::default();
/// let unverified: Packet<Unverified> = Packet::new_unverified(raw);
/// let _frame = unverified.into_input_frame();
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet<S: PacketState> {
    raw: RawWireInput,
    _state: PhantomData<S>,
}

impl Packet<Unverified> {
    #[inline]
    pub const fn new_unverified(raw: RawWireInput) -> Self {
        Self { raw, _state: PhantomData }
    }

    #[inline]
    pub const fn verify_bounds(self) -> Result<Packet<Verified>, PacketReject> {
        let m = self.raw.move_dir_milli;
        if m[0] < -1_000 || m[0] > 1_000 || m[1] < -1_000 || m[1] > 1_000 || m[2] < -1_000 || m[2] > 1_000 {
            return Err(PacketReject::MoveComponentOutOfBounds);
        }
        if self.raw.aim_pitch_turn_milli < -250 || self.raw.aim_pitch_turn_milli > 250 {
            return Err(PacketReject::AimPitchOutOfBounds);
        }
        if self.raw.active_weapon_slot >= 3 {
            return Err(PacketReject::InvalidWeaponSlot);
        }
        if (self.raw.action_mask & !0xFFFF) != 0 {
            return Err(PacketReject::ReservedActionBitsSet);
        }
        Ok(Packet {
            raw: self.raw,
            _state: PhantomData,
        })
    }
}

impl Packet<Verified> {
    #[inline]
    pub const fn into_input_frame(self) -> InputFrame {
        let move_vec = QVec3::new(
            Q32::from_ratio_i32(self.raw.move_dir_milli[0], 1_000),
            Q32::from_ratio_i32(self.raw.move_dir_milli[1], 1_000),
            Q32::from_ratio_i32(self.raw.move_dir_milli[2], 1_000),
        );
        InputFrame {
            tick: self.raw.tick,
            move_vec,
            aim_yaw_turn: Q32::from_ratio_i32(self.raw.aim_yaw_turn_milli, 1_000),
            aim_pitch_turn: Q32::from_ratio_i32(self.raw.aim_pitch_turn_milli, 1_000),
            action_mask: self.raw.action_mask as u16,
            weapon_slot: self.raw.active_weapon_slot,
            _seal: VerifiedInputSeal(()),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VerifiedInputSeal(());

/// Action bitflags inside `InputFrame::action_mask`.
pub mod actions {
    pub const PRIMARY_ATTACK: u16 = 1 << 0;
    pub const DODGE_PHANTASIA: u16 = 1 << 1;
    pub const PARRY_CLASH: u16 = 1 << 2;
    pub const SWAP_DISCHARGE: u16 = 1 << 3;
    pub const JETPACK_THRUST: u16 = 1 << 4;
    pub const GRAPPLE_HOOK: u16 = 1 << 5;
    pub const GLIDE_WING: u16 = 1 << 6;
    pub const WALL_RUN: u16 = 1 << 7;
    pub const SURFACE_SLIDE: u16 = 1 << 8;
    pub const ELEMENTAL_SKILL: u16 = 1 << 9;
}

/// Canonical validated simulation input frame (`SoA` & headless `G2~G4` bots share this exact type).
/// All fields are strictly private and immutable outside `Packet<Verified>::into_input_frame` / `InputFrame::idle`:
/// ```compile_fail
/// use verifier_abi::InputFrame;
/// let mut f = InputFrame::idle(0);
/// f.weapon_slot = 99;
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFrame {
    tick: u64,
    move_vec: QVec3,
    aim_yaw_turn: Q32,
    aim_pitch_turn: Q32,
    action_mask: u16,
    weapon_slot: u8,
    _seal: VerifiedInputSeal,
}

impl InputFrame {
    #[inline]
    pub const fn idle(tick: u64) -> Self {
        Self {
            tick,
            move_vec: QVec3::ZERO,
            aim_yaw_turn: Q32::ZERO,
            aim_pitch_turn: Q32::ZERO,
            action_mask: 0,
            weapon_slot: 0,
            _seal: VerifiedInputSeal(()),
        }
    }

    #[inline]
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    #[inline]
    pub const fn move_vec(&self) -> QVec3 {
        self.move_vec
    }

    #[inline]
    pub const fn aim_yaw_turn(&self) -> Q32 {
        self.aim_yaw_turn
    }

    #[inline]
    pub const fn aim_pitch_turn(&self) -> Q32 {
        self.aim_pitch_turn
    }

    #[inline]
    pub const fn action_mask(&self) -> u16 {
        self.action_mask
    }

    #[inline]
    pub const fn weapon_slot(&self) -> u8 {
        self.weapon_slot
    }
}

// ============================================================================
// 2. Zero-Cost Typestate Contract #2: Power_S ⊥ Legacy Plane Isolation
// ============================================================================

pub trait PlaneMarker: private::Sealed {}
pub trait CombatAllowedPlane: PlaneMarker {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatPlane;
impl private::Sealed for CombatPlane {}
impl PlaneMarker for CombatPlane {}
impl CombatAllowedPlane for CombatPlane {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyPlane;
impl private::Sealed for LegacyPlane {}
impl PlaneMarker for LegacyPlane {}

/// Plane-tagged value: `LegacyPlane` values CANNOT be read via combat accessors (`rustc` compile error):
/// ```compile_fail
/// use verifier_abi::{LegacyPlane, PlaneScoped};
/// let legacy_val: PlaneScoped<LegacyPlane, u64> = PlaneScoped::new(77);
/// let _ = legacy_val.get_combat();
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaneScoped<P: PlaneMarker, T> {
    value: T,
    _plane: PhantomData<P>,
}

impl<P: PlaneMarker, T> PlaneScoped<P, T> {
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value, _plane: PhantomData }
    }
}

impl<P: CombatAllowedPlane, T> PlaneScoped<P, T> {
    #[inline]
    pub const fn get_combat(&self) -> &T {
        &self.value
    }
}

impl<T> PlaneScoped<LegacyPlane, T> {
    #[inline]
    pub const fn get_legacy_cosmetic_only(&self) -> &T {
        &self.value
    }
}

/// Combat stat binding restricted strictly to `CombatAllowedPlane` (`LegacyPlane` rejected by `rustc`).
///
/// ```compile_fail
/// use verifier_abi::{CombatStatBinding, LegacyPlane, PlaneScoped};
/// let legacy_item: PlaneScoped<LegacyPlane, u64> = PlaneScoped::new(999);
/// let _illegal = CombatStatBinding::bind_to_combat(legacy_item);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatStatBinding<T> {
    pub stat: T,
}

impl<T> CombatStatBinding<T> {
    #[inline]
    pub fn bind_to_combat<P: CombatAllowedPlane>(scoped: PlaneScoped<P, T>) -> Self {
        Self { stat: scoped.value }
    }
}

// ============================================================================
// 3. Zero-Cost Typestate Contract #3: Encounter Atomic Epoch Pinning
// ============================================================================

pub trait EpochState: private::Sealed {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Staging;
impl private::Sealed for Staging {}
impl EpochState for Staging {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pinned;
impl private::Sealed for Pinned {}
impl EpochState for Pinned {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochManifestSummary {
    pub epoch_id: u64,
    pub season_id: u32,
    pub merkle_root: [u8; 32],
    pub faucet_caps: [i64; 8],
    pub base_move_speed_q32: Q32,
    pub expected_farm_ticks: u64,
    pub synth_clear_permille: u16,
    pub synth_exit_mean_permille: u16,
    pub synth_exit_std_permille: u16,
}

impl EpochManifestSummary {
    pub fn compute_merkle_root(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.epoch_id.to_le_bytes());
        hasher.update(&self.season_id.to_le_bytes());
        for cap in &self.faucet_caps {
            hasher.update(&cap.to_le_bytes());
        }
        hasher.update(&self.base_move_speed_q32.raw().to_le_bytes());
        hasher.update(&self.expected_farm_ticks.to_le_bytes());
        hasher.update(&self.synth_clear_permille.to_le_bytes());
        hasher.update(&self.synth_exit_mean_permille.to_le_bytes());
        hasher.update(&self.synth_exit_std_permille.to_le_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Encounter epoch handle: `Pinned` handles cannot be mutated mid-encounter.
///
/// ```compile_fail
/// use math_q32::Q32;
/// use verifier_abi::{EpochHandle, EpochManifestSummary, Staging};
/// let summary = EpochManifestSummary {
///     epoch_id: 1,
///     season_id: 1,
///     merkle_root: [0; 32],
///     faucet_caps: [1000; 8],
///     base_move_speed_q32: Q32::ONE,
///     expected_farm_ticks: 600,
///     synth_clear_permille: 500,
///     synth_exit_mean_permille: 20,
///     synth_exit_std_permille: 10,
/// };
/// let pinned = EpochHandle::<Staging>::new_staging(summary).seal_and_pin();
/// pinned.set_base_move_speed(Q32::HALF);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpochHandle<S: EpochState> {
    summary: EpochManifestSummary,
    _state: PhantomData<S>,
}

impl EpochHandle<Staging> {
    #[inline]
    pub const fn new_staging(summary: EpochManifestSummary) -> Self {
        Self { summary, _state: PhantomData }
    }

    #[inline]
    pub fn set_base_move_speed(&mut self, speed: Q32) {
        self.summary.base_move_speed_q32 = speed;
    }

    #[inline]
    pub fn seal_and_pin(mut self) -> EpochHandle<Pinned> {
        self.summary.merkle_root = self.summary.compute_merkle_root();
        EpochHandle {
            summary: self.summary,
            _state: PhantomData,
        }
    }
}

impl EpochHandle<Pinned> {
    #[inline]
    pub const fn manifest(&self) -> &EpochManifestSummary {
        &self.summary
    }

    /// Atomic boundary drain transition (`게임.md §5.3`).
    #[inline]
    pub const fn drain_at_encounter_boundary(self, next: EpochHandle<Pinned>) -> EpochHandle<Pinned> {
        next
    }
}

// ============================================================================
// 4. Additive Backward-Compatible `u16` Tag-Length-Value (TLV) Wire Format
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldWireKind {
    VarInt64,
    Fixed32,
    Fixed64,
    Fixed128,
    Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlvField {
    pub tag: u16,
    pub value: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlvError {
    TruncatedHeader,
    TruncatedPayload,
    PayloadTooLarge,
    DeletedTag(u16),
    MutatedTagWireKind(u16),
}

pub fn encode_tlv_stream(fields: &[TlvField]) -> Result<Vec<u8>, TlvError> {
    let mut out = Vec::new();
    for f in fields {
        let len_u32 = u32::try_from(f.value.len()).map_err(|_| TlvError::PayloadTooLarge)?;
        out.extend_from_slice(&f.tag.to_le_bytes());
        out.extend_from_slice(&len_u32.to_le_bytes());
        out.extend_from_slice(&f.value);
    }
    Ok(out)
}

pub fn decode_tlv_stream(bytes: &[u8]) -> Result<Vec<TlvField>, TlvError> {
    let mut cursor: usize = 0;
    let mut out = Vec::new();
    while cursor < bytes.len() {
        let header_end = cursor.checked_add(6).ok_or(TlvError::TruncatedHeader)?;
        if header_end > bytes.len() {
            return Err(TlvError::TruncatedHeader);
        }
        let tag = u16::from_le_bytes([bytes[cursor], bytes[cursor.saturating_add(1)]]);
        let len = u32::from_le_bytes([
            bytes[cursor.saturating_add(2)],
            bytes[cursor.saturating_add(3)],
            bytes[cursor.saturating_add(4)],
            bytes[cursor.saturating_add(5)],
        ]) as usize;
        let payload_end = header_end.checked_add(len).ok_or(TlvError::TruncatedPayload)?;
        if payload_end > bytes.len() {
            return Err(TlvError::TruncatedPayload);
        }
        out.push(TlvField {
            tag,
            value: bytes[header_end..payload_end].to_vec(),
        });
        cursor = payload_end;
    }
    Ok(out)
}

/// Verifies additive-only backward-compatible schema evolution (`게임.md §5.3, §9.2`).
pub fn verify_additive_schema(
    prev_schema: &[(u16, FieldWireKind)],
    next_schema: &[(u16, FieldWireKind)],
) -> Result<(), TlvError> {
    for &(prev_tag, prev_kind) in prev_schema {
        let mut found = None;
        for &(next_tag, next_kind) in next_schema {
            if next_tag == prev_tag {
                found = Some(next_kind);
                break;
            }
        }
        match found {
            None => return Err(TlvError::DeletedTag(prev_tag)),
            Some(k) if k != prev_kind => return Err(TlvError::MutatedTagWireKind(prev_tag)),
            Some(_) => {}
        }
    }
    Ok(())
}

/// Compile-time (`const`) verifier for additive-only `u16` TLV schema evolution AND unique tags (`게임.md §9.2`).
pub const fn is_valid_additive_schema_const(
    prev_schema: &[(u16, FieldWireKind)],
    next_schema: &[(u16, FieldWireKind)],
) -> bool {
    // 1. Ensure `next_schema` has strictly unique `u16` tags:
    let mut i = 0_usize;
    while i < next_schema.len() {
        let mut j = i.saturating_add(1);
        while j < next_schema.len() {
            if next_schema[i].0 == next_schema[j].0 {
                return false;
            }
            j = j.saturating_add(1);
        }
        i = i.saturating_add(1);
    }
    // 2. Ensure every `(tag, kind)` in `prev_schema` is preserved identically in `next_schema`:
    let mut p = 0_usize;
    while p < prev_schema.len() {
        let (ptag, pkind) = prev_schema[p];
        let mut found = false;
        let mut n = 0_usize;
        while n < next_schema.len() {
            let (ntag, nkind) = next_schema[n];
            if ntag == ptag {
                if (pkind as u8) != (nkind as u8) {
                    return false;
                }
                found = true;
                break;
            }
            n = n.saturating_add(1);
        }
        if !found {
            return false;
        }
        p = p.saturating_add(1);
    }
    true
}

/// Compile-time schema evolution guard: fails `rustc` type-checking if any tag in `$prev` is deleted or changes wire kind in `$next`!
/// ```compile_fail
/// use verifier_abi::{FieldWireKind, assert_tlv_schema_evolution};
/// const V1: &[(u16, FieldWireKind)] = &[(1, FieldWireKind::Fixed64), (2, FieldWireKind::Bytes)];
/// const V2_BAD: &[(u16, FieldWireKind)] = &[(1, FieldWireKind::Fixed32)];
/// assert_tlv_schema_evolution!(V1, V2_BAD);
/// ```
#[macro_export]
macro_rules! assert_tlv_schema_evolution {
    ($prev:expr, $next:expr) => {
        const _: [(); 1] = [(); $crate::is_valid_additive_schema_const($prev, $next) as usize];
    };
}

/// Declarative backward-compatible `u16` TLV record serializer/deserializer macro (`게임.md §5.3, §9.2`).
/// Automatically defaults missing new tags on decode AND preserves unrecognized future tags lossless round-trip!
#[macro_export]
macro_rules! impl_tlv_u64_record {
    (
        $ty:ident,
        prev_schema: $prev_schema:expr,
        schema_const: $schema_ident:ident,
        fields: [ $( ($tag:literal, $field:ident, $default:expr) ),+ $(,)? ],
        unknown_field: $unk_field:ident
    ) => {
        pub const $schema_ident: &[(u16, $crate::FieldWireKind)] = &[
            $( ($tag, $crate::FieldWireKind::Fixed64) ),+
        ];
        $crate::assert_tlv_schema_evolution!($prev_schema, $schema_ident);

        impl $ty {
            pub fn encode_tlv_bytes(&self) -> Result<alloc::vec::Vec<u8>, $crate::TlvError> {
                let mut fields = alloc::vec::Vec::new();
                $(
                    fields.push($crate::TlvField {
                        tag: $tag,
                        value: (self.$field as u64).to_le_bytes().to_vec(),
                    });
                )+
                for unk in &self.$unk_field {
                    fields.push(unk.clone());
                }
                $crate::encode_tlv_stream(&fields)
            }

            pub fn decode_tlv_bytes(bytes: &[u8]) -> Result<Self, $crate::TlvError> {
                let raw_fields = $crate::decode_tlv_stream(bytes)?;
                $( let mut $field: u64 = $default; )+
                let mut unknown = alloc::vec::Vec::new();
                for f in raw_fields {
                    match f.tag {
                        $(
                            $tag => {
                                if f.value.len() != 8 {
                                    return Err($crate::TlvError::TruncatedPayload);
                                }
                                let mut arr = [0_u8; 8];
                                arr.copy_from_slice(&f.value);
                                $field = u64::from_le_bytes(arr);
                            }
                        )+
                        _ => unknown.push(f),
                    }
                }
                Ok(Self {
                    $( $field, )+
                    $unk_field: unknown,
                })
            }
        }
    };
}

// ============================================================================
// 5. Offscreen GPU Profiler Opaque Handles (`VkQueryPool` Private to Verifier)
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffscreenGpuContextHandles {
    pub instance_raw: u64,
    pub physical_device_raw: u64,
    pub device_raw: u64,
    pub queue_raw: u64,
    pub queue_family_index: u32,
    pub command_buffer_raw: u64,
    pub target_image_raw: u64,
    pub target_view_raw: u64,
    pub width: u32,
    pub height: u32,
    pub vulkan_minor_version: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RenderRecordReport {
    pub cpu_record_ns: u64,
    pub brick_staging_ns: u64,
    pub used_ssbo_vis_buffer_64: bool,
    pub used_host_image_copy: bool,
}

// ============================================================================
// 6. Anonymous Causal Mechanism Graph (`G2` Trait Mutation & `G4` WL Kernel)
// ============================================================================

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AnonMechRole {
    InputTrigger = 0,
    WeaponSwap = 1,
    ShatterBreak = 2,
    WeaponCharge = 3,
    DischargeBurst = 4,
    PhantasiaSlowField = 5,
    ClashParry = 6,
    ProjectileSplit = 7,
    ResonanceField = 8,
    ElementCellReact = 9,
    TerrainCsgCut = 10,
    BossAdaptiveShift = 11,
    SpatialImpulse = 12,
    StateGate = 13,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CausalEdge {
    pub src: u16,
    pub dst: u16,
    pub edge_kind: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalGraphIR {
    pub nodes: Vec<AnonMechRole>,
    pub causal_edges: Vec<CausalEdge>,
}

// ============================================================================
// 7. Ledger, Telemetry Oracle, and Canary Watchdog Schemas
// ============================================================================

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct AccountId(pub u64);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct AssetId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerTxKind {
    Transfer {
        from: AccountId,
        to: AccountId,
        asset: AssetId,
        amount: i64,
    },
    FaucetMint {
        shard_id: u16,
        to: AccountId,
        asset: AssetId,
        amount: i64,
    },
    SinkBurn {
        from: AccountId,
        asset: AssetId,
        amount: i64,
    },
    GrantLegacy {
        account: AccountId,
        legacy_item_id: u64,
    },
    ExternalCosmeticPurchase {
        account: AccountId,
        cosmetic_id: u64,
        requested_power_bonus: i64,
    },
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RationalCounter {
    pub sum_num: i64,
    pub sum_den: i64,
}

impl RationalCounter {
    /// Exact `i128` cross-multiplication comparison (`self >= reference`) without floating-point rounding.
    #[inline]
    pub const fn ge_exact(self, reference: Self) -> Result<bool, &'static str> {
        if self.sum_den <= 0 || reference.sum_den <= 0 {
            return Err("RationalCounter denominator must be strictly positive");
        }
        let lhs = (self.sum_num as i128).saturating_mul(reference.sum_den as i128);
        let rhs = (reference.sum_num as i128).saturating_mul(self.sum_den as i128);
        Ok(lhs >= rhs)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OracleEpochTelemetry {
    pub voluntary_return: RationalCounter,
    pub active_combat_explore: RationalCounter,
    pub build_experiment_breadth: RationalCounter,
    pub party_coop_completion: RationalCounter,
    pub measured_move_speed_q32: Q32,
    pub measured_expected_farm_ticks: u64,
    pub has_wireheading_gate_or_manipulation: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanarySlidingTelemetry {
    pub panic_or_crash_count: u64,
    pub ledger_error_count: u64,
    pub total_frames: u64,
    pub over_budget_frames: u64,
    pub active_sessions: u64,
    pub abnormal_exits: u64,
    pub encounters_per_quantile: [u64; 4],
    pub clears_per_quantile: [u64; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_typestate_bounds() {
        let valid = RawWireInput {
            tick: 1,
            move_dir_milli: [1000, 0, -500],
            aim_yaw_turn_milli: 250,
            aim_pitch_turn_milli: -120,
            action_mask: actions::PRIMARY_ATTACK as u32,
            active_weapon_slot: 2,
        };
        let verified = Packet::new_unverified(valid).verify_bounds();
        assert!(verified.is_ok());

        let bad_slot = RawWireInput {
            active_weapon_slot: 3,
            ..valid
        };
        assert_eq!(
            Packet::new_unverified(bad_slot).verify_bounds(),
            Err(PacketReject::InvalidWeaponSlot)
        );
    }

    #[test]
    fn test_tlv_roundtrip_and_unknown_tag_preservation() {
        let fields = alloc::vec![
            TlvField { tag: 1, value: alloc::vec![10, 20] },
            TlvField { tag: 99, value: alloc::vec![1, 2, 3, 4] },
        ];
        let encoded = encode_tlv_stream(&fields);
        assert!(encoded.is_ok());
        if let Ok(bytes) = encoded {
            let decoded = decode_tlv_stream(&bytes);
            assert_eq!(decoded, Ok(fields));
        }

        let s1 = [(1_u16, FieldWireKind::Fixed64), (2_u16, FieldWireKind::Bytes)];
        let s2 = [
            (1_u16, FieldWireKind::Fixed64),
            (2_u16, FieldWireKind::Bytes),
            (3_u16, FieldWireKind::VarInt64),
        ];
        assert_eq!(verify_additive_schema(&s1, &s2), Ok(()));
        assert_eq!(
            verify_additive_schema(&s2, &s1),
            Err(TlvError::DeletedTag(3))
        );
    }
}
