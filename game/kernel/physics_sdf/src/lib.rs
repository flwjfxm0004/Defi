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

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use ecs_soa::{CombatArena, EntityId};
use math_q32::{Q32, QVec3};
use serde::{Deserialize, Serialize};

/// Interior `8x8x8` voxels + `1-voxel` normal-continuity padding on all 6 faces = `10x10x10`.
pub const BRICK_INNER_DIM: i32 = 8;
pub const BRICK_PADDED_DIM: usize = 10;
pub const BRICK_CELL_COUNT: usize = 1000;
pub const UNDO_RING_TICKS: usize = 128;
pub const MAX_DIRTY_BRICKS_PER_TICK: usize = 256;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ElementCellState {
    #[default]
    Neutral = 0,
    Water = 1,
    FrozenIce = 2,
    UpdraftColumn = 3,
    ConductiveOre = 4,
    ElectroCharged = 5,
    InvertedGravity = 6,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ElementStimulus {
    CryoFreeze = 0,
    PyroThermal = 1,
    VoltDischarge = 2,
    VoidGravityInvert = 3,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug, Serialize, Deserialize)]
pub struct BrickCoord {
    pub bx: i32,
    pub by: i32,
    pub bz: i32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SdfBrick {
    /// Signed distance in centimeters (`< 0` solid, `>= 0` air).
    pub sdf_cm: [i16; BRICK_CELL_COUNT],
    /// Reactive environmental field state per voxel (`게임.md §1.1`).
    pub field_state: [u8; BRICK_CELL_COUNT],
}

impl Default for SdfBrick {
    fn default() -> Self {
        Self {
            sdf_cm: [1000; BRICK_CELL_COUNT],
            field_state: [ElementCellState::Neutral as u8; BRICK_CELL_COUNT],
        }
    }
}

impl SdfBrick {
    #[inline]
    pub const fn cell_index(lx: usize, ly: usize, lz: usize) -> usize {
        lz.saturating_mul(100)
            .saturating_add(ly.saturating_mul(10))
            .saturating_add(lx)
    }
}

#[derive(Clone)]
struct TickUndoSlot {
    tick: u64,
    len: u16,
    coords: [BrickCoord; MAX_DIRTY_BRICKS_PER_TICK],
    priors: [Option<Arc<SdfBrick>>; MAX_DIRTY_BRICKS_PER_TICK],
}

impl Default for TickUndoSlot {
    fn default() -> Self {
        Self {
            tick: u64::MAX,
            len: 0,
            coords: [BrickCoord::default(); MAX_DIRTY_BRICKS_PER_TICK],
            priors: core::array::from_fn(|_| None),
        }
    }
}

/// CPU-authoritative Sparse SDF World with Copy-on-Write pages, bounded `128-tick` `BrickUndoLog` ringbuffer
/// (`0` heap reallocations in tick loop), and `DirtyBrickQueue` (`게임.md §8.1`).
#[derive(Clone)]
pub struct SparseSdfWorld {
    bricks: BTreeMap<BrickCoord, Arc<SdfBrick>>,
    undo_ring: Box<[TickUndoSlot]>,
    dirty_queue: BTreeSet<BrickCoord>,
}

impl Default for SparseSdfWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChunkTopologyMetrics {
    pub walkable_surface_voxels: u32,
    pub sigma_z_q32: Q32,
    pub fully_3d_connected: bool,
}

impl SparseSdfWorld {
    pub fn new() -> Self {
        let mut ring = Vec::with_capacity(UNDO_RING_TICKS);
        let mut i: usize = 0;
        while i < UNDO_RING_TICKS {
            ring.push(TickUndoSlot::default());
            i = i.saturating_add(1);
        }
        Self {
            bricks: BTreeMap::new(),
            undo_ring: ring.into_boxed_slice(),
            dirty_queue: BTreeSet::new(),
        }
    }

    /// Generates a procedural multi-elevation 3D chunk (`4x4x4` bricks = `32x32x32m`) with cliffs,
    /// platforms, water basin, conductive ore veins, and updraft vents (`게임.md §1.1`).
    pub fn generate_vertical_chunk(&mut self, chunk_cx: i32, chunk_cz: i32, seed: u64) {
        let base_bx = chunk_cx.saturating_mul(4);
        let base_bz = chunk_cz.saturating_mul(4);
        let seed_bias = ((seed & 0x7) as i32).saturating_sub(3);

        let mut dbx: i32 = 0;
        while dbx < 4 {
            let mut dbz: i32 = 0;
            while dbz < 4 {
                let mut dby: i32 = 0;
                while dby < 4 {
                    let coord = BrickCoord {
                        bx: base_bx.saturating_add(dbx),
                        by: dby,
                        bz: base_bz.saturating_add(dbz),
                    };
                    let mut brick = SdfBrick::default();
                    let mut lz: usize = 0;
                    while lz < BRICK_PADDED_DIM {
                        let mut ly: usize = 0;
                        while ly < BRICK_PADDED_DIM {
                            let mut lx: usize = 0;
                            while lx < BRICK_PADDED_DIM {
                                let wx = coord
                                    .bx
                                    .saturating_mul(BRICK_INNER_DIM)
                                    .saturating_add((lx as i32).saturating_sub(1));
                                let wy = coord
                                    .by
                                    .saturating_mul(BRICK_INNER_DIM)
                                    .saturating_add((ly as i32).saturating_sub(1));
                                let wz = coord
                                    .bz
                                    .saturating_mul(BRICK_INNER_DIM)
                                    .saturating_add((lz as i32).saturating_sub(1));

                                let ridge_x = (wx.rem_euclid(16)).saturating_sub(8).abs();
                                let ridge_z = (wz.rem_euclid(16)).saturating_sub(8).abs();
                                let terrain_h = 4_i32
                                    .saturating_add(ridge_x)
                                    .saturating_add(ridge_z / 2)
                                    .saturating_add(seed_bias)
                                    .clamp(2, 22);

                                let dist_cm = (wy.saturating_sub(terrain_h))
                                    .saturating_mul(100)
                                    .clamp(-30_000, 30_000) as i16;
                                let idx = SdfBrick::cell_index(lx, ly, lz);
                                brick.sdf_cm[idx] = dist_cm;

                                if wy <= 9 && dist_cm >= 0 {
                                    brick.field_state[idx] = ElementCellState::Water as u8;
                                } else if ridge_x >= 6 && dist_cm >= 0 && wy <= terrain_h.saturating_add(2) {
                                    brick.field_state[idx] = ElementCellState::ConductiveOre as u8;
                                }
                                lx = lx.saturating_add(1);
                            }
                            ly = ly.saturating_add(1);
                        }
                        lz = lz.saturating_add(1);
                    }
                    self.bricks.insert(coord, Arc::new(brick));
                    dby = dby.saturating_add(1);
                }
                dbz = dbz.saturating_add(1);
            }
            dbx = dbx.saturating_add(1);
        }
    }

    /// Records prior brick state in the fixed `128-tick` undo ringbuffer before mutation (`0` heap allocs).
    /// Returns `true` if `coord` is already snapshotted for `tick` or newly recorded within capacity;
    /// returns `false` if `MAX_DIRTY_BRICKS_PER_TICK` is full so callers never mutate an unsnapshotted brick!
    fn snapshot_brick_for_undo(&mut self, tick: u64, coord: BrickCoord) -> bool {
        const UNDO_RING_MASK: u64 = (UNDO_RING_TICKS as u64).saturating_sub(1);
        let ring_idx = (tick & UNDO_RING_MASK) as usize;
        let slot = &mut self.undo_ring[ring_idx];
        if slot.tick != tick {
            let prev_len = (slot.len as usize).min(MAX_DIRTY_BRICKS_PER_TICK);
            let mut k: usize = 0;
            while k < prev_len {
                slot.priors[k] = None;
                k = k.saturating_add(1);
            }
            slot.tick = tick;
            slot.len = 0;
        }
        let cur_len = (slot.len as usize).min(MAX_DIRTY_BRICKS_PER_TICK);
        let mut i: usize = 0;
        while i < cur_len {
            if slot.coords[i] == coord {
                return true;
            }
            i = i.saturating_add(1);
        }
        if cur_len < MAX_DIRTY_BRICKS_PER_TICK {
            slot.coords[cur_len] = coord;
            slot.priors[cur_len] = self.bricks.get(&coord).cloned();
            slot.len = (cur_len.saturating_add(1)) as u16;
            true
        } else {
            false
        }
    }

    fn brick_would_carve(brick: &SdfBrick, bx: i32, by: i32, bz: i32, center_voxel: [i32; 3], r_cm: i32) -> bool {
        let mut lz: usize = 0;
        while lz < BRICK_PADDED_DIM {
            let mut ly: usize = 0;
            while ly < BRICK_PADDED_DIM {
                let mut lx: usize = 0;
                while lx < BRICK_PADDED_DIM {
                    let wx = bx
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((lx as i32).saturating_sub(1));
                    let wy = by
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((ly as i32).saturating_sub(1));
                    let wz = bz
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((lz as i32).saturating_sub(1));
                    let dx = wx.saturating_sub(center_voxel[0]);
                    let dy = wy.saturating_sub(center_voxel[1]);
                    let dz = wz.saturating_sub(center_voxel[2]);
                    let dist_sq_cm = (dx
                        .saturating_mul(dx)
                        .saturating_add(dy.saturating_mul(dy))
                        .saturating_add(dz.saturating_mul(dz)))
                    .saturating_mul(10_000);
                    let dist_cm = math_q32::isqrt_u128(dist_sq_cm.max(0) as u128) as i32;
                    let carved_sdf = r_cm.saturating_sub(dist_cm).clamp(-30_000, 30_000) as i16;
                    let idx = SdfBrick::cell_index(lx, ly, lz);
                    if carved_sdf > brick.sdf_cm[idx] {
                        return true;
                    }
                    lx = lx.saturating_add(1);
                }
                ly = ly.saturating_add(1);
            }
            lz = lz.saturating_add(1);
        }
        false
    }

    fn brick_would_react(
        brick: &SdfBrick,
        bx: i32,
        by: i32,
        bz: i32,
        center_voxel: [i32; 3],
        r_sq: i32,
        stimulus: ElementStimulus,
    ) -> bool {
        let mut lz: usize = 0;
        while lz < BRICK_PADDED_DIM {
            let mut ly: usize = 0;
            while ly < BRICK_PADDED_DIM {
                let mut lx: usize = 0;
                while lx < BRICK_PADDED_DIM {
                    let wx = bx
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((lx as i32).saturating_sub(1));
                    let wy = by
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((ly as i32).saturating_sub(1));
                    let wz = bz
                        .saturating_mul(BRICK_INNER_DIM)
                        .saturating_add((lz as i32).saturating_sub(1));
                    let dx = wx.saturating_sub(center_voxel[0]);
                    let dy = wy.saturating_sub(center_voxel[1]);
                    let dz = wz.saturating_sub(center_voxel[2]);
                    let d_sq = dx
                        .saturating_mul(dx)
                        .saturating_add(dy.saturating_mul(dy))
                        .saturating_add(dz.saturating_mul(dz));
                    if d_sq <= r_sq {
                        let i = SdfBrick::cell_index(lx, ly, lz);
                        let cur = brick.field_state[i];
                        let next = match stimulus {
                            ElementStimulus::CryoFreeze if cur == (ElementCellState::Water as u8) => {
                                ElementCellState::FrozenIce as u8
                            }
                            ElementStimulus::PyroThermal if brick.sdf_cm[i] >= 0 => {
                                ElementCellState::UpdraftColumn as u8
                            }
                            ElementStimulus::VoltDischarge
                                if cur == (ElementCellState::Water as u8)
                                    || cur == (ElementCellState::ConductiveOre as u8) =>
                            {
                                ElementCellState::ElectroCharged as u8
                            }
                            ElementStimulus::VoidGravityInvert if brick.sdf_cm[i] >= 0 => {
                                ElementCellState::InvertedGravity as u8
                            }
                            _ => cur,
                        };
                        if next != cur {
                            return true;
                        }
                    }
                    lx = lx.saturating_add(1);
                }
                ly = ly.saturating_add(1);
            }
            lz = lz.saturating_add(1);
        }
        false
    }

    /// `O(1)` local CSG spherical subtraction (`ΔSDF_csg != 0` destructive terrain carving).
    /// Uses Copy-on-Write (`Arc::make_mut`) and records undo history in the fixed `BrickUndoLog` ringbuffer.
    pub fn carve_sphere_csg(&mut self, tick: u64, center_voxel: [i32; 3], radius_voxels: i32) -> u32 {
        let r = radius_voxels.clamp(1, 8);
        let r_cm = r.saturating_mul(100);
        let min_bx = (center_voxel[0].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_bx = (center_voxel[0].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);
        let min_by = (center_voxel[1].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_by = (center_voxel[1].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);
        let min_bz = (center_voxel[2].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_bz = (center_voxel[2].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);

        let mut modified_cells: u32 = 0;
        let mut bx = min_bx;
        while bx <= max_bx {
            let mut by = min_by;
            while by <= max_by {
                let mut bz = min_bz;
                while bz <= max_bz {
                    let coord = BrickCoord { bx, by, bz };
                    let will_modify = self
                        .bricks
                        .get(&coord)
                        .is_some_and(|b| Self::brick_would_carve(b, bx, by, bz, center_voxel, r_cm));
                    if will_modify
                        && self.snapshot_brick_for_undo(tick, coord)
                        && let Some(brick_arc) = self.bricks.get_mut(&coord)
                    {
                        let brick = Arc::make_mut(brick_arc);
                        let mut changed = false;
                        let mut lz: usize = 0;
                        while lz < BRICK_PADDED_DIM {
                            let mut ly: usize = 0;
                            while ly < BRICK_PADDED_DIM {
                                let mut lx: usize = 0;
                                while lx < BRICK_PADDED_DIM {
                                    let wx = bx
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((lx as i32).saturating_sub(1));
                                    let wy = by
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((ly as i32).saturating_sub(1));
                                    let wz = bz
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((lz as i32).saturating_sub(1));
                                    let dx = wx.saturating_sub(center_voxel[0]);
                                    let dy = wy.saturating_sub(center_voxel[1]);
                                    let dz = wz.saturating_sub(center_voxel[2]);
                                    let dist_sq_cm = (dx
                                        .saturating_mul(dx)
                                        .saturating_add(dy.saturating_mul(dy))
                                        .saturating_add(dz.saturating_mul(dz)))
                                    .saturating_mul(10_000);
                                    let dist_cm = math_q32::isqrt_u128(dist_sq_cm.max(0) as u128) as i32;
                                    let carved_sdf = r_cm.saturating_sub(dist_cm).clamp(-30_000, 30_000) as i16;
                                    let idx = SdfBrick::cell_index(lx, ly, lz);
                                    if carved_sdf > brick.sdf_cm[idx] {
                                        brick.sdf_cm[idx] = carved_sdf;
                                        changed = true;
                                        modified_cells = modified_cells.saturating_add(1);
                                    }
                                    lx = lx.saturating_add(1);
                                }
                                ly = ly.saturating_add(1);
                            }
                            lz = lz.saturating_add(1);
                        }
                        if changed {
                            self.dirty_queue.insert(coord);
                        }
                    }
                    bz = bz.saturating_add(1);
                }
                by = by.saturating_add(1);
            }
            bx = bx.saturating_add(1);
        }
        modified_cells
    }

    /// Euclidean-bounded cellular elemental field reaction (`ΔField_react != 0`), updating `1-voxel`
    /// neighbor padding identically across brick boundaries for normal continuity (`게임.md §1.1, §8.1`).
    pub fn apply_element_stimulus(
        &mut self,
        tick: u64,
        center_voxel: [i32; 3],
        radius_voxels: i32,
        stimulus: ElementStimulus,
    ) -> u32 {
        let r = radius_voxels.clamp(1, 8);
        let r_sq = r.saturating_mul(r);
        let min_bx = (center_voxel[0].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_bx = (center_voxel[0].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);
        let min_by = (center_voxel[1].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_by = (center_voxel[1].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);
        let min_bz = (center_voxel[2].saturating_sub(r).saturating_sub(1)).div_euclid(BRICK_INNER_DIM);
        let max_bz = (center_voxel[2].saturating_add(r).saturating_add(1)).div_euclid(BRICK_INNER_DIM);

        let mut transitioned: u32 = 0;
        let mut bx = min_bx;
        while bx <= max_bx {
            let mut by = min_by;
            while by <= max_by {
                let mut bz = min_bz;
                while bz <= max_bz {
                    let coord = BrickCoord { bx, by, bz };
                    let will_react = self
                        .bricks
                        .get(&coord)
                        .is_some_and(|b| Self::brick_would_react(b, bx, by, bz, center_voxel, r_sq, stimulus));
                    if will_react
                        && self.snapshot_brick_for_undo(tick, coord)
                        && let Some(brick_arc) = self.bricks.get_mut(&coord)
                    {
                        let brick = Arc::make_mut(brick_arc);
                        let mut changed = false;
                        let mut lz: usize = 0;
                        while lz < BRICK_PADDED_DIM {
                            let mut ly: usize = 0;
                            while ly < BRICK_PADDED_DIM {
                                let mut lx: usize = 0;
                                while lx < BRICK_PADDED_DIM {
                                    let wx = bx
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((lx as i32).saturating_sub(1));
                                    let wy = by
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((ly as i32).saturating_sub(1));
                                    let wz = bz
                                        .saturating_mul(BRICK_INNER_DIM)
                                        .saturating_add((lz as i32).saturating_sub(1));
                                    let dx = wx.saturating_sub(center_voxel[0]);
                                    let dy = wy.saturating_sub(center_voxel[1]);
                                    let dz = wz.saturating_sub(center_voxel[2]);
                                    let d_sq = dx
                                        .saturating_mul(dx)
                                        .saturating_add(dy.saturating_mul(dy))
                                        .saturating_add(dz.saturating_mul(dz));

                                    if d_sq <= r_sq {
                                        let i = SdfBrick::cell_index(lx, ly, lz);
                                        let cur = brick.field_state[i];
                                        let next = match stimulus {
                                            ElementStimulus::CryoFreeze
                                                if cur == (ElementCellState::Water as u8) =>
                                            {
                                                if brick.sdf_cm[i] > -50 {
                                                    brick.sdf_cm[i] = -50;
                                                }
                                                ElementCellState::FrozenIce as u8
                                            }
                                            ElementStimulus::PyroThermal if brick.sdf_cm[i] >= 0 => {
                                                ElementCellState::UpdraftColumn as u8
                                            }
                                            ElementStimulus::VoltDischarge
                                                if cur == (ElementCellState::Water as u8)
                                                    || cur == (ElementCellState::ConductiveOre as u8) =>
                                            {
                                                ElementCellState::ElectroCharged as u8
                                            }
                                            ElementStimulus::VoidGravityInvert if brick.sdf_cm[i] >= 0 => {
                                                ElementCellState::InvertedGravity as u8
                                            }
                                            _ => cur,
                                        };
                                        if next != cur {
                                            brick.field_state[i] = next;
                                            changed = true;
                                            transitioned = transitioned.saturating_add(1);
                                        }
                                    }
                                    lx = lx.saturating_add(1);
                                }
                                ly = ly.saturating_add(1);
                            }
                            lz = lz.saturating_add(1);
                        }
                        if changed {
                            self.dirty_queue.insert(coord);
                        }
                    }
                    bz = bz.saturating_add(1);
                }
                by = by.saturating_add(1);
            }
            bx = bx.saturating_add(1);
        }
        transitioned
    }

    /// Rewinds modified bricks strictly back to `target_tick` in `O(dirty)` (`0` heap allocs, `게임.md §8.1`).
    pub fn rewind_to_tick(&mut self, target_tick: u64) {
        let mut to_rewind = [(0_u64, 0_usize); UNDO_RING_TICKS];
        let mut count: usize = 0;
        let mut idx: usize = 0;
        while idx < UNDO_RING_TICKS {
            let t = self.undo_ring[idx].tick;
            if t != u64::MAX && t > target_tick {
                to_rewind[count] = (t, idx);
                count = count.saturating_add(1);
            }
            idx = idx.saturating_add(1);
        }

        // Insertion sort descending by tick (`count <= 128`, stack-only, zero heap allocation!).
        let mut a: usize = 1;
        while a < count {
            let key = to_rewind[a];
            let mut b = a;
            while b > 0 && to_rewind[b.saturating_sub(1)].0 < key.0 {
                to_rewind[b] = to_rewind[b.saturating_sub(1)];
                b = b.saturating_sub(1);
            }
            to_rewind[b] = key;
            a = a.saturating_add(1);
        }

        let mut r_i: usize = 0;
        while r_i < count {
            let slot_idx = to_rewind[r_i].1;
            let slot_len = (self.undo_ring[slot_idx].len as usize).min(MAX_DIRTY_BRICKS_PER_TICK);
            let mut e_i = slot_len;
            while e_i > 0 {
                e_i = e_i.saturating_sub(1);
                let coord = self.undo_ring[slot_idx].coords[e_i];
                let prior = self.undo_ring[slot_idx].priors[e_i].take();
                match prior {
                    Some(prev_brick) => {
                        self.bricks.insert(coord, prev_brick);
                    }
                    None => {
                        self.bricks.remove(&coord);
                    }
                }
                self.dirty_queue.insert(coord);
            }
            self.undo_ring[slot_idx].tick = u64::MAX;
            self.undo_ring[slot_idx].len = 0;
            r_i = r_i.saturating_add(1);
        }
    }

    /// Drains `DirtyBrickQueue` for per-frame GPU `<= 0.15ms` delta staging (`게임.md §8.1~§8.2`).
    pub fn drain_dirty_bricks(&mut self) -> Vec<BrickCoord> {
        let out: Vec<BrickCoord> = self.dirty_queue.iter().copied().collect();
        self.dirty_queue.clear();
        out
    }

    #[inline]
    pub fn sample_sdf_cm(&self, wx: i32, wy: i32, wz: i32) -> i16 {
        let bx = wx.div_euclid(BRICK_INNER_DIM);
        let by = wy.div_euclid(BRICK_INNER_DIM);
        let bz = wz.div_euclid(BRICK_INNER_DIM);
        let lx = (wx.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        let ly = (wy.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        let lz = (wz.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        match self.bricks.get(&BrickCoord { bx, by, bz }) {
            Some(brick) => brick.sdf_cm[SdfBrick::cell_index(lx, ly, lz)],
            None => 1000,
        }
    }

    #[inline]
    pub fn sample_field(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        let bx = wx.div_euclid(BRICK_INNER_DIM);
        let by = wy.div_euclid(BRICK_INNER_DIM);
        let bz = wz.div_euclid(BRICK_INNER_DIM);
        let lx = (wx.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        let ly = (wy.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        let lz = (wz.rem_euclid(BRICK_INNER_DIM).saturating_add(1)) as usize;
        match self.bricks.get(&BrickCoord { bx, by, bz }) {
            Some(brick) => brick.field_state[SdfBrick::cell_index(lx, ly, lz)],
            None => ElementCellState::Neutral as u8,
        }
    }

    /// Resolves player/entity SDF ground collision & environmental field impulses (`Updraft`, `InvertedGravity`).
    pub fn resolve_entity_sdf_contacts(&self, arena: &mut CombatArena, id: EntityId, dt: Q32) {
        let Some(mut pos) = arena.position(id) else {
            return;
        };
        let Some(mut vel) = arena.velocity(id) else {
            return;
        };
        let wx = pos.x.to_int_floor();
        let wy = pos.y.to_int_floor();
        let wz = pos.z.to_int_floor();

        let field = self.sample_field(wx, wy, wz);
        let gravity = if field == (ElementCellState::InvertedGravity as u8) {
            Q32::from_int(15)
        } else if field == (ElementCellState::UpdraftColumn as u8) {
            Q32::from_int(22)
        } else {
            Q32::from_int(-20)
        };
        vel.y = vel.y.sat_add(gravity.sat_mul(dt));

        let sdf_cm = self.sample_sdf_cm(wx, wy, wz);
        if sdf_cm < 0 {
            let push_m = Q32::from_ratio_i32((sdf_cm as i32).saturating_neg(), 100);
            pos.y = pos.y.sat_add(push_m);
            if vel.y < Q32::ZERO {
                vel.y = Q32::ZERO;
            }
        }
        arena.set_position(id, pos);
        arena.set_velocity(id, vel);
    }

    /// Computes exact `G1` chunk geometric invariants (`N_surf >= 256`, `sigma_z >= 2.0m`, 3D reachability).
    pub fn analyze_chunk_topology(&self, chunk_cx: i32, chunk_cz: i32) -> ChunkTopologyMetrics {
        let min_x = chunk_cx.saturating_mul(32);
        let min_z = chunk_cz.saturating_mul(32);
        let mut surface_coords: Vec<[i32; 3]> = Vec::new();

        let mut x = min_x;
        while x < min_x.saturating_add(32) {
            let mut z = min_z;
            while z < min_z.saturating_add(32) {
                let mut y: i32 = 0;
                while y < 31 {
                    let solid_here = self.sample_sdf_cm(x, y, z) <= 0;
                    let air_above = self.sample_sdf_cm(x, y.saturating_add(1), z) > 0;
                    if solid_here && air_above {
                        surface_coords.push([x, y.saturating_add(1), z]);
                    }
                    y = y.saturating_add(1);
                }
                z = z.saturating_add(1);
            }
            x = x.saturating_add(1);
        }

        let n = surface_coords.len();
        if n == 0 {
            return ChunkTopologyMetrics::default();
        }

        let mut sum_y: i128 = 0;
        for c in &surface_coords {
            sum_y = sum_y.saturating_add(c[1] as i128);
        }
        let n_i128 = (n as i128).max(1);
        let mean_q32 = Q32::saturate_i128((sum_y << 32).checked_div(n_i128).unwrap_or(0));
        let mut var_acc_q64: i128 = 0;
        for c in &surface_coords {
            let diff = Q32::from_int(c[1]).sat_sub(mean_q32);
            let sq = diff.mul_wide(diff).0.checked_div(n_i128).unwrap_or(0);
            var_acc_acc(&mut var_acc_q64, sq);
        }
        let sigma_z_q32 = Q32::saturate_i128(math_q32::isqrt_u128(var_acc_q64.max(0) as u128) as i128);

        let surface_set: BTreeSet<[i32; 3]> = surface_coords.iter().copied().collect();
        let mut visited: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
        let start = surface_coords[0];
        visited.insert(start);
        queue.push_back(start);

        while let Some([cx, cy, cz]) = queue.pop_front() {
            let mut dx: i32 = -1;
            while dx <= 1 {
                let mut dz: i32 = -1;
                while dz <= 1 {
                    let mut dy: i32 = -6;
                    while dy <= 6 {
                        let cand = [cx.saturating_add(dx), cy.saturating_add(dy), cz.saturating_add(dz)];
                        if surface_set.contains(&cand) && !visited.contains(&cand) {
                            visited.insert(cand);
                            queue.push_back(cand);
                        }
                        dy = dy.saturating_add(1);
                    }
                    dz = dz.saturating_add(1);
                }
                dx = dx.saturating_add(1);
            }
        }

        ChunkTopologyMetrics {
            walkable_surface_voxels: n as u32,
            sigma_z_q32,
            fully_3d_connected: visited.len() == surface_set.len(),
        }
    }

    pub fn state_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        for (coord, brick) in &self.bricks {
            hasher.update(&coord.bx.to_le_bytes());
            hasher.update(&coord.by.to_le_bytes());
            hasher.update(&coord.bz.to_le_bytes());
            for &d in &brick.sdf_cm {
                hasher.update(&d.to_le_bytes());
            }
            hasher.update(&brick.field_state);
        }
        *hasher.finalize().as_bytes()
    }
}

#[inline]
fn var_acc_acc(acc: &mut i128, val: i128) {
    *acc = acc.saturating_add(val);
}

// ============================================================================
// L2-Resident (`<= 256KB`) Compressed `Int8`/`Q16.16` Learned Motion Matching
// + Critically-Damped 2nd-Order `Q32.32` Spring-Damper Inertialization (`게임.md §8.1`)
// ============================================================================

pub const MOTION_CLIPS: usize = 64;
pub const COMBAT_BONES: usize = 6;
pub const COSMETIC_BONES: usize = 12;

/// `log2(e)` in `Q32.32` (`round(1.4426950408889634 * 2^32) = 6_196_328_019`).
const LOG2_E_Q32: Q32 = Q32::from_raw(6_196_328_019);

/// Compressed motion database (`Int8` feature projections + `Q16.16` bone offsets, `<= 256KB` L2-resident).
#[repr(C)]
#[derive(Clone)]
pub struct CompressedMotionDb {
    pub feature_codes_i8: [[i8; 4]; MOTION_CLIPS],
    pub combat_poses_q16: [[[i32; 3]; COMBAT_BONES]; MOTION_CLIPS],
    pub cosmetic_poses_q16: [[[i32; 3]; COSMETIC_BONES]; MOTION_CLIPS],
}

const _: () = assert!(core::mem::size_of::<CompressedMotionDb>() <= 262_144);

impl Default for CompressedMotionDb {
    fn default() -> Self {
        let mut db = Self {
            feature_codes_i8: [[0; 4]; MOTION_CLIPS],
            combat_poses_q16: [[[0; 3]; COMBAT_BONES]; MOTION_CLIPS],
            cosmetic_poses_q16: [[[0; 3]; COSMETIC_BONES]; MOTION_CLIPS],
        };
        let mut c: usize = 0;
        while c < MOTION_CLIPS {
            let ci = c as i32;
            db.feature_codes_i8[c] = [
                ((ci.saturating_mul(7) % 120).saturating_sub(60)) as i8,
                ((ci.saturating_mul(5) % 100).saturating_sub(50)) as i8,
                ((ci.saturating_mul(11) % 120).saturating_sub(60)) as i8,
                ((ci % 3).saturating_mul(32)) as i8,
            ];
            let mut b: usize = 0;
            while b < COMBAT_BONES {
                let bi = b as i32;
                db.combat_poses_q16[c][b] = [
                    (ci.saturating_sub(32)).saturating_mul(512),
                    (bi.saturating_add(1)).saturating_mul(19_660),
                    ((ci.saturating_add(bi)) % 16).saturating_mul(1024),
                ];
                b = b.saturating_add(1);
            }
            let mut k: usize = 0;
            while k < COSMETIC_BONES {
                db.cosmetic_poses_q16[c][k] = [
                    ((k as i32).saturating_mul(256)).saturating_add(ci.saturating_mul(128)),
                    32_768,
                    0,
                ];
                k = k.saturating_add(1);
            }
            c = c.saturating_add(1);
        }
        db
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InertialMotionState {
    pub initialized: bool,
    pub active_clip: u16,
    pub offset_pos: [QVec3; COMBAT_BONES],
    pub offset_vel: [QVec3; COMBAT_BONES],
    pub combat_world_bones: [QVec3; COMBAT_BONES],
    pub cosmetic_world_bones: [QVec3; COSMETIC_BONES],
}

impl CompressedMotionDb {
    /// Deterministic every-tick combat hurtbox skeleton FK in pure `Q32.32` with analytical
    /// critically-damped 2nd-order Spring-Damper Inertialization (`x'' + 2*w*x' + w^2*x = 0`, `w = 20 rad/s`).
    pub fn step_combat_skeleton_q32(
        &self,
        root_pos: QVec3,
        root_vel: QVec3,
        weapon_slot: u8,
        dt: Q32,
        state: &mut InertialMotionState,
    ) {
        let query = [
            root_vel.x.clamp_val(Q32::NEG_ONE, Q32::ONE),
            root_vel.y.clamp_val(Q32::NEG_ONE, Q32::ONE),
            root_vel.z.clamp_val(Q32::NEG_ONE, Q32::ONE),
            Q32::from_ratio_i32(weapon_slot as i32, 4),
        ];

        let mut best_clip: usize = 0;
        let mut best_cost = i128::MAX;
        let mut c: usize = 0;
        while c < MOTION_CLIPS {
            let f0 = Q32::promote_i8_q7(self.feature_codes_i8[c][0]).sat_sub(query[0]);
            let f1 = Q32::promote_i8_q7(self.feature_codes_i8[c][1]).sat_sub(query[1]);
            let f2 = Q32::promote_i8_q7(self.feature_codes_i8[c][2]).sat_sub(query[2]);
            let f3 = Q32::promote_i8_q7(self.feature_codes_i8[c][3]).sat_sub(query[3]);
            let cost = f0
                .mul_wide(f0)
                .sat_add(f1.mul_wide(f1))
                .sat_add(f2.mul_wide(f2))
                .sat_add(f3.mul_wide(f3))
                .0;
            if cost < best_cost {
                best_cost = cost;
                best_clip = c;
            }
            c = c.saturating_add(1);
        }

        if !state.initialized {
            state.initialized = true;
            state.active_clip = best_clip as u16;
            state.offset_pos = [QVec3::ZERO; COMBAT_BONES];
            state.offset_vel = [QVec3::ZERO; COMBAT_BONES];
            let mut b: usize = 0;
            while b < COMBAT_BONES {
                let target_local = self.unpack_combat_bone_q32(best_clip, b);
                state.combat_world_bones[b] = root_pos.sat_add(target_local);
                b = b.saturating_add(1);
            }
            return;
        }

        let omega = Q32::from_int(20);
        if (best_clip as u16) != state.active_clip {
            let prev_clip = (state.active_clip as usize).min(MOTION_CLIPS.saturating_sub(1));
            let mut b: usize = 0;
            while b < COMBAT_BONES {
                let target_local = self.unpack_combat_bone_q32(best_clip, b);
                let prev_target_local = self.unpack_combat_bone_q32(prev_clip, b);
                let cur_local = prev_target_local.sat_add(state.offset_pos[b]);
                let delta_x = cur_local.sat_sub(target_local);
                // Preserve velocity continuity while bounding counter-impulse cleanly:
                state.offset_pos[b] = delta_x;
                state.offset_vel[b] = state.offset_vel[b].sat_sub(delta_x.scale(omega.sat_mul(Q32::HALF)));
                b = b.saturating_add(1);
            }
            state.active_clip = best_clip as u16;
        }

        // Exact critically-damped closed-form update:
        // `e = exp(-omega * dt) = exp2(-omega * dt * log2(e))`
        // `j1 = v0 + omega * x0`
        // `x1 = e * (x0 + j1 * dt)`
        // `v1 = e * (v0 - j1 * omega * dt)`
        let omega_dt = omega.sat_mul(dt).clamp_val(Q32::ZERO, Q32::from_int(8));
        let neg_exp_arg = omega_dt.sat_mul(LOG2_E_Q32).sat_neg();
        let e = neg_exp_arg.exp2_sat().clamp_val(Q32::ZERO, Q32::ONE);

        let mut b: usize = 0;
        while b < COMBAT_BONES {
            let x0 = state.offset_pos[b];
            let v0 = state.offset_vel[b];
            let j1 = v0.sat_add(x0.scale(omega));
            let x1 = x0.sat_add(j1.scale(dt)).scale(e);
            let v1 = v0.sat_sub(j1.scale(omega_dt)).scale(e);
            state.offset_pos[b] = x1;
            state.offset_vel[b] = v1;

            let target_local = self.unpack_combat_bone_q32(best_clip, b);
            state.combat_world_bones[b] = root_pos.sat_add(target_local).sat_add(x1);
            b = b.saturating_add(1);
        }
    }

    /// Synthesizes secondary cosmetic bones & foot-IK once on the final post-rollback sub-tick (`게임.md §8.1`).
    pub fn synthesize_final_subtick_cosmetics(&self, root_pos: QVec3, ground_sdf_cm: i16, state: &mut InertialMotionState) {
        let clip = (state.active_clip as usize).min(MOTION_CLIPS.saturating_sub(1));
        let ik_lift = Q32::from_ratio_i32((ground_sdf_cm as i32).saturating_neg().max(0), 100);
        let mut k: usize = 0;
        while k < COSMETIC_BONES {
            let raw = self.cosmetic_poses_q16[clip][k];
            let local = QVec3::new(
                Q32::promote_q16_16(raw[0]),
                Q32::promote_q16_16(raw[1]).sat_add(if k < 2 { ik_lift } else { Q32::ZERO }),
                Q32::promote_q16_16(raw[2]),
            );
            state.cosmetic_world_bones[k] = root_pos.sat_add(local);
            k = k.saturating_add(1);
        }
    }

    #[inline]
    fn unpack_combat_bone_q32(&self, clip: usize, bone: usize) -> QVec3 {
        let raw = self.combat_poses_q16[clip][bone];
        QVec3::new(
            Q32::promote_q16_16(raw[0]),
            Q32::promote_q16_16(raw[1]),
            Q32::promote_q16_16(raw[2]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spherical_stimulus_multi_call_and_ring_undo_rewind() {
        let mut world = SparseSdfWorld::new();
        // Populate a compact 4-brick (`2x1x2`) surface + water/ore neighborhood (fast under Miri & native):
        for &(bx, bz) in &[(0_i32, 0_i32), (1, 0), (0, 1), (1, 1)] {
            let mut brick = SdfBrick::default();
            let mut lz: usize = 0;
            while lz < BRICK_PADDED_DIM {
                let mut ly: usize = 0;
                while ly < BRICK_PADDED_DIM {
                    let wy = (ly as i32).saturating_sub(1);
                    let mut lx: usize = 0;
                    while lx < BRICK_PADDED_DIM {
                        let idx = SdfBrick::cell_index(lx, ly, lz);
                        brick.sdf_cm[idx] = (wy.saturating_sub(4)).saturating_mul(100) as i16;
                        if wy <= 5 {
                            brick.field_state[idx] = ElementCellState::Water as u8;
                        } else {
                            brick.field_state[idx] = ElementCellState::ConductiveOre as u8;
                        }
                        lx = lx.saturating_add(1);
                    }
                    ly = ly.saturating_add(1);
                }
                lz = lz.saturating_add(1);
            }
            world.bricks.insert(BrickCoord { bx, by: 0, bz }, Arc::new(brick));
        }

        let hash_t0 = world.state_hash();
        // Multiple CSG + elemental calls in the SAME tick `10` AND next tick `11`:
        let carved1 = world.carve_sphere_csg(10, [4, 4, 4], 3);
        let reacted1 = world.apply_element_stimulus(10, [4, 4, 4], 3, ElementStimulus::CryoFreeze);
        let carved2 = world.carve_sphere_csg(10, [11, 4, 11], 2);
        let reacted2 = world.apply_element_stimulus(11, [11, 6, 4], 2, ElementStimulus::VoltDischarge);
        assert!(carved1 > 0 && carved2 > 0);
        assert!(reacted1 > 0 && reacted2 > 0);
        // Far voxel `[15, 4, 0]` outside Euclidean radius of `[4, 4, 4]` (`r=3`) and `[11, 4, 11]` (`r=2`) remains Water!
        assert_eq!(world.sample_field(15, 4, 0), ElementCellState::Water as u8);
        assert_ne!(world.state_hash(), hash_t0);

        world.rewind_to_tick(9);
        assert_eq!(world.state_hash(), hash_t0);
    }

    #[test]
    fn test_critically_damped_spring_damper_inertialization() {
        let db = Box::new(CompressedMotionDb::default());
        let mut st = InertialMotionState::default();
        let root = QVec3::from_ints(2, 4, -1);
        db.step_combat_skeleton_q32(root, QVec3::ZERO, 0, Q32::DT_120HZ, &mut st);
        assert!(st.initialized);
        assert_eq!(st.offset_pos[0], QVec3::ZERO);

        // Trigger weapon/velocity switch -> non-zero offset & velocity, then decay smoothly toward 0!
        db.step_combat_skeleton_q32(root, QVec3::from_ints(1, 0, -1), 2, Q32::DT_120HZ, &mut st);
        let initial_sq = st.offset_pos[0].dist_sq_wide(QVec3::ZERO);
        assert!(initial_sq.0 > 0);
        assert!(st.offset_vel[0].dist_sq_wide(QVec3::ZERO).0 > 0);
        let mut step: usize = 0;
        while step < 30 {
            db.step_combat_skeleton_q32(root, QVec3::from_ints(1, 0, -1), 2, Q32::DT_120HZ, &mut st);
            step = step.saturating_add(1);
        }
        assert!(st.offset_pos[0].dist_sq_wide(QVec3::ZERO) < initial_sq);
    }
}
