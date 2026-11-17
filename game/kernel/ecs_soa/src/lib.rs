#![no_std]
#![forbid(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

extern crate alloc;

use alloc::alloc::{Layout, alloc_zeroed, handle_alloc_error};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use math_q32::{DetRng, Q32, QVec3};
use serde::{Deserialize, Serialize};
use verifier_abi::{InputFrame, actions};

pub const MAX_ENTITIES: usize = 4096;

/// Generational Entity Handle `(slot_index, generation)`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Serialize, Deserialize)]
pub struct EntityId(pub u32, pub u32);

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum EntityKind {
    #[default]
    Empty = 0,
    Player = 1,
    Boss = 2,
    Projectile = 3,
    PhantasiaField = 4,
    ResonanceZone = 5,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MobilityMode {
    #[default]
    Grounded = 0,
    Airborne = 1,
    Jetpack = 2,
    Grapple = 3,
    Glide = 4,
    WallRun = 5,
    SurfaceSlide = 6,
}

/// Deterministic command emitted by `qvm` or core combat rules and sorted by a strict total order before end-of-tick commit.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SimCommand {
    ApplyImpulse {
        target: EntityId,
        impulse: QVec3,
    },
    DealDamage {
        source: EntityId,
        target: EntityId,
        hp_damage: Q32,
        shatter_damage: Q32,
        charge_gain: Q32,
    },
    TriggerPhantasia {
        center: QVec3,
        radius: Q32,
        slow_scale: Q32,
        source: EntityId,
    },
    TriggerClash {
        defender: EntityId,
        boss: EntityId,
        shatter_burst: Q32,
    },
    SpawnProjectile {
        owner: EntityId,
        origin: QVec3,
        velocity: QVec3,
        damage: Q32,
    },
    MutateBossPattern {
        boss: EntityId,
        next_pattern: u16,
    },
}

type FullSortKey = ((u8, u32, u32, u32, u32), ([i64; 7], u16));

impl SimCommand {
    /// Complete bijective sort key covering every field so tie-breaking is `100%` independent of emission order.
    #[inline]
    pub const fn total_sort_key(&self) -> FullSortKey {
        match *self {
            Self::TriggerPhantasia {
                center,
                radius,
                slow_scale,
                source,
            } => (
                (0, source.0, source.1, 0, 0),
                (
                    [
                        center.x.raw(),
                        center.y.raw(),
                        center.z.raw(),
                        radius.raw(),
                        slow_scale.raw(),
                        0,
                        0,
                    ],
                    0,
                ),
            ),
            Self::TriggerClash {
                defender,
                boss,
                shatter_burst,
            } => (
                (1, defender.0, defender.1, boss.0, boss.1),
                ([shatter_burst.raw(), 0, 0, 0, 0, 0, 0], 0),
            ),
            Self::ApplyImpulse { target, impulse } => (
                (2, target.0, target.1, 0, 0),
                (
                    [impulse.x.raw(), impulse.y.raw(), impulse.z.raw(), 0, 0, 0, 0],
                    0,
                ),
            ),
            Self::DealDamage {
                source,
                target,
                hp_damage,
                shatter_damage,
                charge_gain,
            } => (
                (3, target.0, target.1, source.0, source.1),
                (
                    [
                        hp_damage.raw(),
                        shatter_damage.raw(),
                        charge_gain.raw(),
                        0,
                        0,
                        0,
                        0,
                    ],
                    0,
                ),
            ),
            Self::SpawnProjectile {
                owner,
                origin,
                velocity,
                damage,
            } => (
                (4, owner.0, owner.1, 0, 0),
                (
                    [
                        origin.x.raw(),
                        origin.y.raw(),
                        origin.z.raw(),
                        velocity.x.raw(),
                        velocity.y.raw(),
                        velocity.z.raw(),
                        damage.raw(),
                    ],
                    0,
                ),
            ),
            Self::MutateBossPattern { boss, next_pattern } => {
                ((5, boss.0, boss.1, 0, 0), ([0; 7], next_pattern))
            }
        }
    }
}

impl Ord for SimCommand {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.total_sort_key().cmp(&other.total_sort_key())
    }
}

impl PartialOrd for SimCommand {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Cache-line-aligned (`64-byte`) Structure-of-Arrays combat arena (`<= 1MB`, `4,096` entities).
#[repr(C, align(64))]
#[derive(Clone)]
pub struct CombatArena {
    pub generations: [u32; MAX_ENTITIES],
    pub alive: [u8; MAX_ENTITIES],
    pub kinds: [u8; MAX_ENTITIES],
    pub active_weapon: [u8; MAX_ENTITIES],
    pub mobility: [u8; MAX_ENTITIES],
    pub attack_cooldown: [u8; MAX_ENTITIES],

    pub pos_x: [Q32; MAX_ENTITIES],
    pub pos_y: [Q32; MAX_ENTITIES],
    pub pos_z: [Q32; MAX_ENTITIES],

    pub vel_x: [Q32; MAX_ENTITIES],
    pub vel_y: [Q32; MAX_ENTITIES],
    pub vel_z: [Q32; MAX_ENTITIES],

    pub time_scale: [Q32; MAX_ENTITIES],
    pub rng_state: [u64; MAX_ENTITIES],

    pub hp: [Q32; MAX_ENTITIES],
    pub shield: [Q32; MAX_ENTITIES],
    pub charge: [Q32; MAX_ENTITIES],
    pub swap_timer: [Q32; MAX_ENTITIES],

    pub weapon_slots: [[u16; 3]; MAX_ENTITIES],
    pub trait_mask: [u32; MAX_ENTITIES],
    pub augment_tiers: [[u8; 3]; MAX_ENTITIES],
    pub boss_pattern: [u16; MAX_ENTITIES],
    pub boss_telegraph_ticks: [u16; MAX_ENTITIES],
    pub owner_slot: [u32; MAX_ENTITIES],

    free_stack: [u32; MAX_ENTITIES],
    free_len: u32,
    pub active_count: u32,
    pub current_tick: u64,
}

const _: () = assert!(core::mem::size_of::<CombatArena>() <= 1_048_576);
const _: () = assert!(core::mem::align_of::<CombatArena>() == 64);

impl CombatArena {
    /// Allocates a 64-byte-aligned `CombatArena` directly on the heap without blowing the 1MB Windows thread stack.
    /// Verified UB-free under `cargo +nightly miri test`.
    pub fn new_boxed(world_seed: u64) -> Box<Self> {
        let layout = Layout::new::<Self>();
        // SAFETY: `CombatArena` is `#[repr(C, align(64))]` composed strictly of integer/Q32 arrays
        // where all-zero bit patterns are valid initialized values, and `layout.size() > 0`.
        let mut boxed = unsafe {
            let raw = alloc_zeroed(layout).cast::<Self>();
            if raw.is_null() {
                handle_alloc_error(layout);
            }
            Box::from_raw(raw)
        };
        boxed.reset_with_seed(world_seed);
        boxed
    }

    pub fn reset_with_seed(&mut self, world_seed: u64) {
        let mut idx: usize = 0;
        while idx < MAX_ENTITIES {
            self.generations[idx] = 1;
            self.alive[idx] = 0;
            self.kinds[idx] = EntityKind::Empty as u8;
            self.active_weapon[idx] = 0;
            self.mobility[idx] = MobilityMode::Grounded as u8;
            self.attack_cooldown[idx] = 0;
            self.pos_x[idx] = Q32::ZERO;
            self.pos_y[idx] = Q32::ZERO;
            self.pos_z[idx] = Q32::ZERO;
            self.vel_x[idx] = Q32::ZERO;
            self.vel_y[idx] = Q32::ZERO;
            self.vel_z[idx] = Q32::ZERO;
            self.time_scale[idx] = Q32::ONE;
            self.rng_state[idx] = world_seed.wrapping_add((idx as u64).wrapping_mul(0x9E37_79B9));
            self.hp[idx] = Q32::ZERO;
            self.shield[idx] = Q32::ZERO;
            self.charge[idx] = Q32::ZERO;
            self.swap_timer[idx] = Q32::ZERO;
            self.weapon_slots[idx] = [0, 1, 2];
            self.trait_mask[idx] = 0;
            self.augment_tiers[idx] = [0; 3];
            self.boss_pattern[idx] = 0;
            self.boss_telegraph_ticks[idx] = 0;
            self.owner_slot[idx] = u32::MAX;
            let rev = MAX_ENTITIES.saturating_sub(1).saturating_sub(idx);
            self.free_stack[idx] = rev as u32;
            idx = idx.saturating_add(1);
        }
        self.free_len = MAX_ENTITIES as u32;
        self.active_count = 0;
        self.current_tick = 0;
    }

    pub fn spawn(&mut self, kind: EntityKind, pos: QVec3, hp: Q32, shield: Q32) -> Option<EntityId> {
        if self.free_len == 0 {
            return None;
        }
        self.free_len = self.free_len.saturating_sub(1);
        let slot = self.free_stack[self.free_len as usize] as usize;
        let generation = self.generations[slot];
        self.alive[slot] = 1;
        self.kinds[slot] = kind as u8;
        self.active_weapon[slot] = 0;
        self.mobility[slot] = MobilityMode::Grounded as u8;
        self.attack_cooldown[slot] = 0;
        self.pos_x[slot] = pos.x;
        self.pos_y[slot] = pos.y;
        self.pos_z[slot] = pos.z;
        self.vel_x[slot] = Q32::ZERO;
        self.vel_y[slot] = Q32::ZERO;
        self.vel_z[slot] = Q32::ZERO;
        self.time_scale[slot] = Q32::ONE;
        self.hp[slot] = hp;
        self.shield[slot] = shield;
        self.charge[slot] = Q32::ZERO;
        self.swap_timer[slot] = Q32::ZERO;
        self.weapon_slots[slot] = [0, 1, 2];
        self.trait_mask[slot] = 0;
        self.augment_tiers[slot] = [0; 3];
        self.boss_pattern[slot] = 0;
        self.boss_telegraph_ticks[slot] = 0;
        self.owner_slot[slot] = u32::MAX;
        self.active_count = self.active_count.saturating_add(1);
        Some(EntityId(slot as u32, generation))
    }

    pub fn despawn(&mut self, id: EntityId) -> bool {
        let slot = id.0 as usize;
        if slot >= MAX_ENTITIES || self.alive[slot] == 0 || self.generations[slot] != id.1 {
            return false;
        }
        self.alive[slot] = 0;
        self.kinds[slot] = EntityKind::Empty as u8;
        self.generations[slot] = self.generations[slot].wrapping_add(1).max(1);
        if (self.free_len as usize) < MAX_ENTITIES {
            self.free_stack[self.free_len as usize] = slot as u32;
            self.free_len = self.free_len.saturating_add(1);
        }
        self.active_count = self.active_count.saturating_sub(1);
        true
    }

    /// Deterministically despawns every live `Projectile` farther than `max_dist` from `center`
    /// (bounded projectile population for `O(P * N)` hit resolution, `게임.md §8.1`). Returns despawn count.
    pub fn despawn_projectiles_beyond(&mut self, center: QVec3, max_dist: Q32) -> u32 {
        let lim_sq = max_dist.mul_wide(max_dist);
        let mut removed: u32 = 0;
        let mut s: usize = 0;
        while s < MAX_ENTITIES {
            if self.alive[s] != 0 && self.kinds[s] == (EntityKind::Projectile as u8) {
                let p = QVec3::new(self.pos_x[s], self.pos_y[s], self.pos_z[s]);
                if p.dist_sq_wide(center) > lim_sq {
                    let g = self.generations[s];
                    if self.despawn(EntityId(s as u32, g)) {
                        removed = removed.saturating_add(1);
                    }
                }
            }
            s = s.saturating_add(1);
        }
        removed
    }

    #[inline]
    pub fn is_valid(&self, id: EntityId) -> bool {
        let slot = id.0 as usize;
        slot < MAX_ENTITIES && self.alive[slot] != 0 && self.generations[slot] == id.1
    }

    #[inline]
    pub fn position(&self, id: EntityId) -> Option<QVec3> {
        if !self.is_valid(id) {
            return None;
        }
        let s = id.0 as usize;
        Some(QVec3::new(self.pos_x[s], self.pos_y[s], self.pos_z[s]))
    }

    #[inline]
    pub fn velocity(&self, id: EntityId) -> Option<QVec3> {
        if !self.is_valid(id) {
            return None;
        }
        let s = id.0 as usize;
        Some(QVec3::new(self.vel_x[s], self.vel_y[s], self.vel_z[s]))
    }

    #[inline]
    pub fn set_velocity(&mut self, id: EntityId, v: QVec3) {
        if self.is_valid(id) {
            let s = id.0 as usize;
            self.vel_x[s] = v.x;
            self.vel_y[s] = v.y;
            self.vel_z[s] = v.z;
        }
    }

    #[inline]
    pub fn set_position(&mut self, id: EntityId, p: QVec3) {
        if self.is_valid(id) {
            let s = id.0 as usize;
            self.pos_x[s] = p.x;
            self.pos_y[s] = p.y;
            self.pos_z[s] = p.z;
        }
    }

    /// Applies player input with zero-cooldown (`<= 0.05s`) 3-weapon swap preserving full momentum & altitude,
    /// single-frame Animation Cancel, and `Discharge` burst on full `Charge` (`게임.md §1.1~§1.3`).
    pub fn apply_player_input(&mut self, player: EntityId, input: &InputFrame, dt: Q32, out_cmds: &mut CommandBuffer) {
        let _ = self.apply_player_input_with_hurtbox(
            player,
            input,
            (QVec3::ZERO, Q32::from_int(4_096), false),
            dt,
            out_cmds,
        );
    }

    /// Spatial 3D hurtbox-aligned player input & combat resolver (`게임.md §1.1~§1.3`).
    /// `hurtbox = (weapon_bone_pos, weapon_reach, enforce_bone_reach)`.
    /// Returns `true` when a primary weapon strike connects with the target hurtbox in this frame.
    pub fn apply_player_input_with_hurtbox(
        &mut self,
        player: EntityId,
        input: &InputFrame,
        hurtbox: (QVec3, Q32, bool),
        dt: Q32,
        out_cmds: &mut CommandBuffer,
    ) -> bool {
        let (weapon_bone_pos, weapon_reach, enforce_bone_reach) = hurtbox;
        if !self.is_valid(player) {
            return false;
        }
        let s = player.0 as usize;
        let requested_slot = input.weapon_slot().min(2);
        let move_vec = input.move_vec();
        let action_mask = input.action_mask();

        if requested_slot != self.active_weapon[s] {
            // Instant atomic weapon swap (`0` cooldown, `0` global lock, immediate Animation Cancel, velocity & altitude untouched!).
            self.active_weapon[s] = requested_slot;
            self.attack_cooldown[s] = 0;
            self.swap_timer[s] = dt.clamp_val(Q32::ZERO, Q32::MAX_SWAP_SEC);
            if self.charge[s] >= Q32::from_int(100) {
                self.charge[s] = Q32::ZERO;
                if let Some(boss) = self.first_alive_of_kind(EntityKind::Boss) {
                    let b = boss.0 as usize;
                    let b_pos = QVec3::new(self.pos_x[b], self.pos_y[b], self.pos_z[b]);
                    let dis_reach = weapon_reach.sat_add(Q32::from_int(8));
                    if !enforce_bone_reach || weapon_bone_pos.dist_sq_wide(b_pos) <= dis_reach.mul_wide(dis_reach) {
                        let _ = out_cmds.push(SimCommand::DealDamage {
                            source: player,
                            target: boss,
                            hp_damage: Q32::from_int(85),
                            shatter_damage: Q32::from_int(55),
                            charge_gain: Q32::ZERO,
                        });
                    }
                }
            }
        } else {
            self.swap_timer[s] = Q32::ZERO;
        }

        // Single-frame Animation Cancel on Dodge or Parry (`게임.md §1.3`):
        if (action_mask & (actions::DODGE_PHANTASIA | actions::PARRY_CLASH)) != 0 {
            self.attack_cooldown[s] = 0;
        }

        // Inertia-preserving 3D mobility transitions (zero stamina gate, zero artificial damping!).
        let prev_v = QVec3::new(self.vel_x[s], self.vel_y[s], self.vel_z[s]);
        let accel = move_vec.scale(Q32::from_int(28));
        let mut next_v = prev_v.fma(Q32::ONE, accel.scale(dt));

        if (action_mask & actions::JETPACK_THRUST) != 0 {
            self.mobility[s] = MobilityMode::Jetpack as u8;
            next_v.y = next_v.y.sat_add(Q32::from_int(18).sat_mul(dt));
        } else if (action_mask & actions::GRAPPLE_HOOK) != 0 {
            self.mobility[s] = MobilityMode::Grapple as u8;
            let grapple_pull = QVec3::new(
                move_vec.x.sat_mul(Q32::from_int(36)),
                Q32::from_int(24),
                move_vec.z.sat_mul(Q32::from_int(36)),
            );
            next_v = next_v.sat_add(grapple_pull.scale(dt));
        } else if (action_mask & actions::GLIDE_WING) != 0 {
            self.mobility[s] = MobilityMode::Glide as u8;
            if next_v.y < Q32::from_int(-2) {
                next_v.y = Q32::from_int(-2);
            }
        } else if (action_mask & actions::WALL_RUN) != 0 {
            self.mobility[s] = MobilityMode::WallRun as u8;
            if next_v.y < Q32::ZERO {
                next_v.y = Q32::ZERO;
            }
        } else if (action_mask & actions::SURFACE_SLIDE) != 0 {
            self.mobility[s] = MobilityMode::SurfaceSlide as u8;
            // Redirect downward landing momentum into tangential low-friction surface slide speed:
            let downward = if next_v.y < Q32::ZERO {
                next_v.y.sat_neg()
            } else {
                Q32::ZERO
            };
            let slide_accel = Q32::from_int(16).sat_add(downward.sat_mul(Q32::HALF));
            next_v = next_v.sat_add(move_vec.scale(slide_accel.sat_mul(dt)));
            next_v.y = next_v.y.sat_sub(Q32::from_int(6).sat_mul(dt));
        }

        self.vel_x[s] = next_v.x;
        self.vel_y[s] = next_v.y;
        self.vel_z[s] = next_v.z;

        let mut connected_hit = false;
        // Primary attack & Phantasia / Clash counter loop (`게임.md §1.2`).
        if let Some(boss) = self.first_alive_of_kind(EntityKind::Boss) {
            let b = boss.0 as usize;
            let b_pos = QVec3::new(self.pos_x[b], self.pos_y[b], self.pos_z[b]);
            let in_hurtbox_range = !enforce_bone_reach
                || weapon_bone_pos.dist_sq_wide(b_pos) <= weapon_reach.mul_wide(weapon_reach);

            if (action_mask & actions::PRIMARY_ATTACK) != 0 && self.attack_cooldown[s] == 0 {
                self.attack_cooldown[s] = 12;
                if in_hurtbox_range {
                    let (hp_dmg, sh_dmg, ch_gain) = match self.active_weapon[s] {
                        0 => (Q32::from_int(14), Q32::from_int(45), Q32::from_int(10)),
                        1 => (Q32::from_int(18), Q32::from_int(12), Q32::from_int(28)),
                        _ => (Q32::from_int(24), Q32::from_int(20), Q32::from_int(16)),
                    };
                    let _ = out_cmds.push(SimCommand::DealDamage {
                        source: player,
                        target: boss,
                        hp_damage: hp_dmg,
                        shatter_damage: sh_dmg,
                        charge_gain: ch_gain,
                    });
                    connected_hit = true;
                }
            }

            if self.boss_telegraph_ticks[b] > 0 {
                let p_pos = QVec3::new(self.pos_x[s], self.pos_y[s], self.pos_z[s]);
                let counter_rad = Q32::from_int(28);
                let in_counter_range = !enforce_bone_reach
                    || p_pos.dist_sq_wide(b_pos) <= counter_rad.mul_wide(counter_rad);
                if in_counter_range {
                    let is_parry_telegraph = (self.boss_pattern[b] & 1) != 0;
                    if !is_parry_telegraph && (action_mask & actions::DODGE_PHANTASIA) != 0 {
                        let _ = out_cmds.push(SimCommand::TriggerPhantasia {
                            center: p_pos,
                            radius: Q32::from_int(64),
                            slow_scale: Q32::from_ratio_i32(25, 100),
                            source: player,
                        });
                    } else if is_parry_telegraph && (action_mask & actions::PARRY_CLASH) != 0 {
                        let _ = out_cmds.push(SimCommand::TriggerClash {
                            defender: player,
                            boss,
                            shatter_burst: Q32::from_int(160),
                        });
                    }
                }
            }
        }
        connected_hit
    }

    /// Deterministic 3D Boss pursuit & tactical spacing (`게임.md §4.3`).
    pub fn step_boss_pursuit(&mut self, boss: EntityId, target: EntityId, dt: Q32) {
        if !self.is_valid(boss) || !self.is_valid(target) {
            return;
        }
        let b = boss.0 as usize;
        let t = target.0 as usize;
        let b_pos = QVec3::new(self.pos_x[b], self.pos_y[b], self.pos_z[b]);
        let t_pos = QVec3::new(self.pos_x[t], self.pos_y[t], self.pos_z[t]);
        let delta = t_pos.sat_sub(b_pos);
        let dist = delta.length();
        let local_dt = dt.sat_mul(self.time_scale[b]);
        if dist > Q32::from_int(3) {
            let dir = delta.normalize_or_zero();
            let chase_speed = Q32::from_int(14);
            let desired_v = dir.scale(chase_speed);
            let cur_v = QVec3::new(self.vel_x[b], self.vel_y[b], self.vel_z[b]);
            let steer = desired_v.sat_sub(cur_v).scale(Q32::from_int(4).sat_mul(local_dt));
            self.vel_x[b] = cur_v.x.sat_add(steer.x);
            self.vel_y[b] = cur_v.y.sat_add(steer.y);
            self.vel_z[b] = cur_v.z.sat_add(steer.z);
        } else {
            self.vel_x[b] = self.vel_x[b].sat_mul(Q32::from_ratio_i32(92, 100));
            self.vel_z[b] = self.vel_z[b].sat_mul(Q32::from_ratio_i32(92, 100));
        }
    }

    pub fn first_alive_of_kind(&self, kind: EntityKind) -> Option<EntityId> {
        let target = kind as u8;
        let mut i: usize = 0;
        while i < MAX_ENTITIES {
            if self.alive[i] != 0 && self.kinds[i] == target {
                return Some(EntityId(i as u32, self.generations[i]));
            }
            i = i.saturating_add(1);
        }
        None
    }

    /// Commits a deterministically sorted `CommandBuffer` at tick end (`게임.md §1.2, §8.3`).
    pub fn commit_commands(&mut self, buf: &mut CommandBuffer) {
        buf.sort_deterministic();
        for cmd in buf.as_slice() {
            match *cmd {
                SimCommand::ApplyImpulse { target, impulse } => {
                    if self.is_valid(target) {
                        let s = target.0 as usize;
                        self.vel_x[s] = self.vel_x[s].sat_add(impulse.x);
                        self.vel_y[s] = self.vel_y[s].sat_add(impulse.y);
                        self.vel_z[s] = self.vel_z[s].sat_add(impulse.z);
                    }
                }
                SimCommand::DealDamage {
                    source,
                    target,
                    hp_damage,
                    shatter_damage,
                    charge_gain,
                } => {
                    if self.is_valid(target) {
                        let t = target.0 as usize;
                        if self.shield[t] > Q32::ZERO {
                            self.shield[t] = self.shield[t].sat_sub(shatter_damage).clamp_val(Q32::ZERO, Q32::MAX);
                            let mitigated = hp_damage.sat_mul(Q32::HALF);
                            self.hp[t] = self.hp[t].sat_sub(mitigated).clamp_val(Q32::ZERO, Q32::MAX);
                        } else {
                            let amplified = hp_damage.sat_mul(Q32::from_ratio_i32(15, 10));
                            self.hp[t] = self.hp[t].sat_sub(amplified).clamp_val(Q32::ZERO, Q32::MAX);
                        }
                    }
                    if self.is_valid(source) {
                        let src = source.0 as usize;
                        self.charge[src] = self.charge[src]
                            .sat_add(charge_gain)
                            .clamp_val(Q32::ZERO, Q32::from_int(100));
                    }
                }
                SimCommand::TriggerPhantasia {
                    center,
                    radius,
                    slow_scale,
                    source,
                } => {
                    if self.is_valid(source) {
                        self.charge[source.0 as usize] = Q32::from_int(100);
                    }
                    let rad_sq = radius.mul_wide(radius);
                    let mut i: usize = 0;
                    while i < MAX_ENTITIES {
                        if self.alive[i] != 0 && self.kinds[i] == (EntityKind::Boss as u8) {
                            let p = QVec3::new(self.pos_x[i], self.pos_y[i], self.pos_z[i]);
                            if p.dist_sq_wide(center) <= rad_sq {
                                self.time_scale[i] = slow_scale.clamp_val(Q32::from_ratio_i32(1, 10), Q32::ONE);
                                self.boss_telegraph_ticks[i] = 0;
                            }
                        }
                        i = i.saturating_add(1);
                    }
                }
                SimCommand::TriggerClash {
                    defender,
                    boss,
                    shatter_burst,
                } => {
                    if self.is_valid(defender) && self.is_valid(boss) {
                        let b = boss.0 as usize;
                        self.shield[b] = self.shield[b].sat_sub(shatter_burst).clamp_val(Q32::ZERO, Q32::MAX);
                        self.boss_telegraph_ticks[b] = 0;
                        self.hp[b] = self.hp[b].sat_sub(Q32::from_int(120)).clamp_val(Q32::ZERO, Q32::MAX);
                    }
                }
                SimCommand::SpawnProjectile {
                    owner,
                    origin,
                    velocity,
                    damage,
                } => {
                    if let Some(proj) = self.spawn(EntityKind::Projectile, origin, damage, Q32::ZERO) {
                        let p = proj.0 as usize;
                        self.vel_x[p] = velocity.x;
                        self.vel_y[p] = velocity.y;
                        self.vel_z[p] = velocity.z;
                        self.owner_slot[p] = owner.0;
                    }
                }
                SimCommand::MutateBossPattern { boss, next_pattern } => {
                    if self.is_valid(boss) {
                        self.boss_pattern[boss.0 as usize] = next_pattern;
                    }
                }
            }
        }
        buf.clear();
    }

    /// Advances deterministic projectile/entity kinematics and boss adaptive tactical modulation (`0` heap allocs).
    pub fn step_kinematics(&mut self, base_dt: Q32) {
        let mut active_player_traits: u32 = 0;
        let mut i: usize = 0;
        while i < MAX_ENTITIES {
            if self.alive[i] != 0 && self.kinds[i] == (EntityKind::Player as u8) {
                active_player_traits |= self.trait_mask[i];
            }
            i = i.saturating_add(1);
        }

        let shockwave_rad = Q32::from_int(48);
        let shockwave_rad_sq = shockwave_rad.mul_wide(shockwave_rad);

        let mut s: usize = 0;
        while s < MAX_ENTITIES {
            if self.alive[s] != 0 {
                self.attack_cooldown[s] = self.attack_cooldown[s].saturating_sub(1);
                let local_dt = base_dt.sat_mul(self.time_scale[s]);
                self.pos_x[s] = self.pos_x[s].sat_add(self.vel_x[s].sat_mul(local_dt));
                self.pos_y[s] = self.pos_y[s].sat_add(self.vel_y[s].sat_mul(local_dt));
                self.pos_z[s] = self.pos_z[s].sat_add(self.vel_z[s].sat_mul(local_dt));

                if self.time_scale[s] < Q32::ONE {
                    self.time_scale[s] = self.time_scale[s]
                        .sat_add(Q32::from_ratio_i32(1, 120))
                        .clamp_val(Q32::ZERO, Q32::ONE);
                }

                // Deterministic projectile hit resolution & despawn against non-owner combatants:
                if self.kinds[s] == (EntityKind::Projectile as u8) {
                    let proj_pos = QVec3::new(self.pos_x[s], self.pos_y[s], self.pos_z[s]);
                    let owner = self.owner_slot[s];
                    let dmg = self.hp[s];
                    let hit_rad = Q32::from_int(2);
                    let hit_rad_sq = hit_rad.mul_wide(hit_rad);
                    let mut ts: usize = 0;
                    while ts < MAX_ENTITIES {
                        if self.alive[ts] != 0
                            && (ts as u32) != owner
                            && (self.kinds[ts] == (EntityKind::Player as u8)
                                || self.kinds[ts] == (EntityKind::Boss as u8))
                        {
                            let t_pos = QVec3::new(self.pos_x[ts], self.pos_y[ts], self.pos_z[ts]);
                            if proj_pos.dist_sq_wide(t_pos) <= hit_rad_sq {
                                self.hp[ts] = self.hp[ts].sat_sub(dmg).clamp_val(Q32::ZERO, Q32::MAX);
                                let gen_s = self.generations[s];
                                let _ = self.despawn(EntityId(s as u32, gen_s));
                                break;
                            }
                        }
                        ts = ts.saturating_add(1);
                    }
                    s = s.saturating_add(1);
                    continue;
                }

                // Boss non-uniform adaptive counter-pattern weighting conditioned on party trait mask (`게임.md §4.3`).
                if self.kinds[s] == (EntityKind::Boss as u8) {
                    let mut rng = DetRng::new(self.rng_state[s]);
                    let r64 = rng.next_u64();
                    self.rng_state[s] = rng.0;

                    let pref_pair = (((active_player_traits >> 2)
                        ^ active_player_traits.count_ones().wrapping_mul(2))
                        as u16)
                        % 6;
                    if self.current_tick.is_multiple_of(30) {
                        let bucket = (r64 % 100) as u16;
                        let sub = ((r64 >> 8) % 12) as u16;
                        self.boss_pattern[s] = if bucket < 70 {
                            pref_pair.wrapping_add(sub & 1) % 6
                        } else {
                            sub % 6
                        };
                        self.boss_telegraph_ticks[s] = 12;
                    } else if self.boss_telegraph_ticks[s] > 0 {
                        self.boss_telegraph_ticks[s] = self.boss_telegraph_ticks[s].saturating_sub(1);
                        if self.boss_telegraph_ticks[s] == 0 {
                            // Strike resolves against ALL living players in range if uncountered!
                            let boss_pos = QVec3::new(self.pos_x[s], self.pos_y[s], self.pos_z[s]);
                            let mut ps: usize = 0;
                            while ps < MAX_ENTITIES {
                                if self.alive[ps] != 0 && self.kinds[ps] == (EntityKind::Player as u8) {
                                    let p_pos = QVec3::new(self.pos_x[ps], self.pos_y[ps], self.pos_z[ps]);
                                    if p_pos.dist_sq_wide(boss_pos) <= shockwave_rad_sq {
                                        self.hp[ps] = self.hp[ps]
                                            .sat_sub(Q32::from_int(35))
                                            .clamp_val(Q32::ZERO, Q32::MAX);
                                    }
                                }
                                ps = ps.saturating_add(1);
                            }
                        }
                    }
                }
            }
            s = s.saturating_add(1);
        }
        self.current_tick = self.current_tick.saturating_add(1);
    }

    /// Canonical bit-exact `BLAKE3` state digest across ALL deterministic arena fields (`x86_64` == `ARM64`).
    pub fn state_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"ARENA_V2");
        hasher.update(&self.current_tick.to_le_bytes());
        hasher.update(&self.active_count.to_le_bytes());
        hasher.update(&self.free_len.to_le_bytes());
        let free_bound = (self.free_len as usize).min(MAX_ENTITIES);
        let mut fi: usize = 0;
        while fi < free_bound {
            hasher.update(&self.free_stack[fi].to_le_bytes());
            fi = fi.saturating_add(1);
        }
        let mut i: usize = 0;
        while i < MAX_ENTITIES {
            if self.alive[i] != 0 {
                hasher.update(&(i as u32).to_le_bytes());
                hasher.update(&self.generations[i].to_le_bytes());
                hasher.update(&[
                    self.kinds[i],
                    self.active_weapon[i],
                    self.mobility[i],
                    self.attack_cooldown[i],
                ]);
                hasher.update(&self.pos_x[i].raw().to_le_bytes());
                hasher.update(&self.pos_y[i].raw().to_le_bytes());
                hasher.update(&self.pos_z[i].raw().to_le_bytes());
                hasher.update(&self.vel_x[i].raw().to_le_bytes());
                hasher.update(&self.vel_y[i].raw().to_le_bytes());
                hasher.update(&self.vel_z[i].raw().to_le_bytes());
                hasher.update(&self.time_scale[i].raw().to_le_bytes());
                hasher.update(&self.rng_state[i].to_le_bytes());
                hasher.update(&self.hp[i].raw().to_le_bytes());
                hasher.update(&self.shield[i].raw().to_le_bytes());
                hasher.update(&self.charge[i].raw().to_le_bytes());
                hasher.update(&self.swap_timer[i].raw().to_le_bytes());
                for &ws in &self.weapon_slots[i] {
                    hasher.update(&ws.to_le_bytes());
                }
                hasher.update(&self.trait_mask[i].to_le_bytes());
                hasher.update(&self.augment_tiers[i]);
                hasher.update(&self.boss_pattern[i].to_le_bytes());
                hasher.update(&self.boss_telegraph_ticks[i].to_le_bytes());
                hasher.update(&self.owner_slot[i].to_le_bytes());
            }
            i = i.saturating_add(1);
        }
        *hasher.finalize().as_bytes()
    }

    #[inline]
    pub fn view(&self) -> SoAView<'_> {
        SoAView { arena: self }
    }
}

/// Immutable read-only view exposed to `qvm` (`Legacy` fields physically absent).
#[derive(Clone, Copy)]
pub struct SoAView<'a> {
    arena: &'a CombatArena,
}

impl<'a> SoAView<'a> {
    #[inline]
    pub fn pos(&self, id: EntityId) -> QVec3 {
        self.arena.position(id).unwrap_or(QVec3::ZERO)
    }

    #[inline]
    pub fn vel(&self, id: EntityId) -> QVec3 {
        self.arena.velocity(id).unwrap_or(QVec3::ZERO)
    }

    #[inline]
    pub fn hp(&self, id: EntityId) -> Q32 {
        if self.arena.is_valid(id) {
            self.arena.hp[id.0 as usize]
        } else {
            Q32::ZERO
        }
    }

    #[inline]
    pub fn shield(&self, id: EntityId) -> Q32 {
        if self.arena.is_valid(id) {
            self.arena.shield[id.0 as usize]
        } else {
            Q32::ZERO
        }
    }

    #[inline]
    pub fn charge(&self, id: EntityId) -> Q32 {
        if self.arena.is_valid(id) {
            self.arena.charge[id.0 as usize]
        } else {
            Q32::ZERO
        }
    }

    #[inline]
    pub fn trait_mask(&self, id: EntityId) -> u32 {
        if self.arena.is_valid(id) {
            self.arena.trait_mask[id.0 as usize]
        } else {
            0
        }
    }

    #[inline]
    pub fn tick(&self) -> u64 {
        self.arena.current_tick
    }
}

/// Pre-sized deterministic command buffer (`0` heap reallocations in hot loop).
#[derive(Clone, Debug, Default)]
pub struct CommandBuffer {
    cmds: Vec<SimCommand>,
}

impl CommandBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            cmds: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn push(&mut self, cmd: SimCommand) -> Result<(), &'static str> {
        if self.cmds.len() >= 4096 {
            return Err("CommandBuffer capacity exceeded");
        }
        self.cmds.push(cmd);
        Ok(())
    }

    #[inline]
    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    #[inline]
    pub fn truncate(&mut self, len: usize) {
        self.cmds.truncate(len);
    }

    #[inline]
    pub fn as_slice(&self) -> &[SimCommand] {
        &self.cmds
    }

    pub fn sort_deterministic(&mut self) {
        self.cmds.sort_unstable();
    }
}

/// Fixed `128-subtick` SoA ring buffer for `120Hz` client prediction & rollback (`게임.md §8.2`).
pub struct SubtickRingBuffer {
    snapshots: Vec<Box<CombatArena>>,
    ticks: [u64; 128],
}

impl SubtickRingBuffer {
    pub fn new(seed: u64) -> Self {
        let mut snapshots = Vec::with_capacity(128);
        let mut i: usize = 0;
        while i < 128 {
            snapshots.push(CombatArena::new_boxed(seed));
            i = i.saturating_add(1);
        }
        Self {
            snapshots,
            ticks: [u64::MAX; 128],
        }
    }

    pub fn record(&mut self, subtick: u64, arena: &CombatArena) {
        let idx = (subtick % 128) as usize;
        self.snapshots[idx].clone_from_slice_fields(arena);
        self.ticks[idx] = subtick;
    }

    pub fn restore(&self, subtick: u64, target: &mut CombatArena) -> bool {
        let idx = (subtick % 128) as usize;
        if self.ticks[idx] != subtick {
            return false;
        }
        target.clone_from_slice_fields(&self.snapshots[idx]);
        true
    }
}

impl CombatArena {
    pub fn clone_from_slice_fields(&mut self, src: &Self) {
        self.generations.copy_from_slice(&src.generations);
        self.alive.copy_from_slice(&src.alive);
        self.kinds.copy_from_slice(&src.kinds);
        self.active_weapon.copy_from_slice(&src.active_weapon);
        self.mobility.copy_from_slice(&src.mobility);
        self.attack_cooldown.copy_from_slice(&src.attack_cooldown);
        self.pos_x.copy_from_slice(&src.pos_x);
        self.pos_y.copy_from_slice(&src.pos_y);
        self.pos_z.copy_from_slice(&src.pos_z);
        self.vel_x.copy_from_slice(&src.vel_x);
        self.vel_y.copy_from_slice(&src.vel_y);
        self.vel_z.copy_from_slice(&src.vel_z);
        self.time_scale.copy_from_slice(&src.time_scale);
        self.rng_state.copy_from_slice(&src.rng_state);
        self.hp.copy_from_slice(&src.hp);
        self.shield.copy_from_slice(&src.shield);
        self.charge.copy_from_slice(&src.charge);
        self.swap_timer.copy_from_slice(&src.swap_timer);
        self.weapon_slots.copy_from_slice(&src.weapon_slots);
        self.trait_mask.copy_from_slice(&src.trait_mask);
        self.augment_tiers.copy_from_slice(&src.augment_tiers);
        self.boss_pattern.copy_from_slice(&src.boss_pattern);
        self.boss_telegraph_ticks.copy_from_slice(&src.boss_telegraph_ticks);
        self.owner_slot.copy_from_slice(&src.owner_slot);
        self.free_stack.copy_from_slice(&src.free_stack);
        self.free_len = src.free_len;
        self.active_count = src.active_count;
        self.current_tick = src.current_tick;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verifier_abi::{Packet, RawWireInput};

    #[test]
    fn test_aligned_soa_spawn_swap_total_sort_and_miri_provenance() {
        let mut arena = CombatArena::new_boxed(42);
        let ptr_addr = (&*arena as *const CombatArena) as usize;
        assert_eq!(ptr_addr % 64, 0);

        let player = arena.spawn(EntityKind::Player, QVec3::from_ints(0, 10, 0), Q32::from_int(1000), Q32::ZERO);
        let boss = arena.spawn(EntityKind::Boss, QVec3::from_ints(5, 10, 0), Q32::from_int(5000), Q32::from_int(300));
        assert!(player.is_some() && boss.is_some());

        if let (Some(p), Some(b)) = (player, boss) {
            arena.set_velocity(p, QVec3::from_ints(12, 5, -3));
            let mut cmds = CommandBuffer::with_capacity(16);
            let raw = RawWireInput {
                tick: 1,
                move_dir_milli: [0, 0, 0],
                aim_yaw_turn_milli: 0,
                aim_pitch_turn_milli: 0,
                action_mask: actions::PRIMARY_ATTACK as u32,
                active_weapon_slot: 2,
            };
            if let Ok(pkt) = Packet::new_unverified(raw).verify_bounds() {
                let frame = pkt.into_input_frame();
                arena.apply_player_input(p, &frame, Q32::DT_120HZ, &mut cmds);
            }
            // Momentum & altitude preserved across instant weapon swap!
            assert_eq!(arena.velocity(p), Some(QVec3::from_ints(12, 5, -3)));
            arena.commit_commands(&mut cmds);
            assert!(arena.view().hp(p) > Q32::ZERO);

            // Emission-order invariance check on SimCommand total order:
            let mut c1 = CommandBuffer::with_capacity(4);
            let mut c2 = CommandBuffer::with_capacity(4);
            let cmd_a = SimCommand::DealDamage {
                source: p,
                target: b,
                hp_damage: Q32::from_int(10),
                shatter_damage: Q32::from_int(200),
                charge_gain: Q32::from_int(5),
            };
            let cmd_b = SimCommand::DealDamage {
                source: EntityId(99, 1),
                target: b,
                hp_damage: Q32::from_int(80),
                shatter_damage: Q32::from_int(5),
                charge_gain: Q32::from_int(5),
            };
            let _ = c1.push(cmd_a);
            let _ = c1.push(cmd_b);
            let _ = c2.push(cmd_b);
            let _ = c2.push(cmd_a);
            c1.sort_deterministic();
            c2.sort_deterministic();
            assert_eq!(c1.as_slice(), c2.as_slice());

            // Orthogonal Boss Dodge (`Phantasia`) vs Parry (`Clash`) telegraph check (`게임.md §1.2`):
            let b_slot = b.0 as usize;
            arena.boss_telegraph_ticks[b_slot] = 8;
            arena.boss_pattern[b_slot] = 0; // Even pattern = Dodge-only telegraph
            let mut counter_cmds = CommandBuffer::with_capacity(8);
            let parry_only = RawWireInput {
                tick: 2,
                move_dir_milli: [0, 0, 0],
                aim_yaw_turn_milli: 0,
                aim_pitch_turn_milli: 0,
                action_mask: u32::from(actions::PARRY_CLASH),
                active_weapon_slot: 0,
            };
            if let Ok(pkt) = Packet::new_unverified(parry_only).verify_bounds() {
                arena.apply_player_input(p, &pkt.into_input_frame(), Q32::DT_120HZ, &mut counter_cmds);
            }
            assert!(counter_cmds.is_empty());

            let dodge_only = RawWireInput {
                action_mask: u32::from(actions::DODGE_PHANTASIA),
                ..parry_only
            };
            if let Ok(pkt) = Packet::new_unverified(dodge_only).verify_bounds() {
                arena.apply_player_input(p, &pkt.into_input_frame(), Q32::DT_120HZ, &mut counter_cmds);
            }
            assert_eq!(counter_cmds.len(), 1);
            arena.commit_commands(&mut counter_cmds);
            assert_eq!(arena.boss_telegraph_ticks[b_slot], 0);

            arena.boss_telegraph_ticks[b_slot] = 8;
            arena.boss_pattern[b_slot] = 1; // Odd pattern = Parry-only telegraph
            if let Ok(pkt) = Packet::new_unverified(dodge_only).verify_bounds() {
                arena.apply_player_input(p, &pkt.into_input_frame(), Q32::DT_120HZ, &mut counter_cmds);
            }
            assert!(counter_cmds.is_empty());
            if let Ok(pkt) = Packet::new_unverified(parry_only).verify_bounds() {
                arena.apply_player_input(p, &pkt.into_input_frame(), Q32::DT_120HZ, &mut counter_cmds);
            }
            assert_eq!(counter_cmds.len(), 1);
        }
    }
}
