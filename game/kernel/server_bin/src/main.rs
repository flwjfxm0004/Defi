#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ecs_soa::{CombatArena, CommandBuffer, EntityId, EntityKind, MAX_ENTITIES};
use ledger_core::LedgerError;
use ledger_wal::{
    GroupCommitWal, MAX_BATCH_LATENCY_MICROS, SeasonGameplayPack, WalOp, load_season_gameplay_pack,
};
use math_q32::{DetRng, Q32, QVec3};
use net_quic::{
    AOI_SHARD_BUDGET_MICROS, AoiSpatialGrid, CID_LEN, CanonicalEpochBundle, ClientEntitySyncState,
    ClientInputDatagram, DualSlotEpochTable, EncounterEpochLease, HmacSessionTicket,
    MAX_DATAGRAM_PAYLOAD_BYTES, MAX_REWIND_TICKS_60HZ, NetError, PortalStepOutcome,
    PriorityDeltaParams, QuicClientEndpoint, QuicServerEndpoint, SERVER_RINGBUFFER_TICKS,
    SLIDING_WINDOW_TICKS, STREAM_KIND_EPOCH_MANIFEST, STREAM_KIND_SDF_CHUNK_DIFF,
    STREAM_KIND_SESSION_RECONNECT, STREAM_KIND_SESSION_TICKET_GRANT, SdfChunkDiffStreamPacket,
    SeamlessPortalGate, ServerDeltaDatagram, ServerSoARingBuffer64, SessionTicketManager,
    ShardCoord, UdpLinkImpairment, ZoneInstanceKind, blake3_digest,
    build_ack_priority_delta_chunks, build_pinned_quic_tls_configs, record_received_delta_chunk,
};
use physics_sdf::{BrickCoord, ElementStimulus, SparseSdfWorld};
use qvm::{
    CompiledQvmModule, INVOCATION_FUEL_CAP, SERVER_SHARD_TICK_FUEL_CAP, compile_qvm_source,
    execute_handler,
};
use tokio::sync::oneshot;
use verifier_abi::{
    AccountId, AssetId, CausalGraphIR, EpochManifestSummary, InputFrame, LedgerTxKind, LegacyPlane,
    Packet, PlaneScoped, RawWireInput, TlvField, Unverified, actions,
};

const RAID_AGENT_COUNT: usize = 50;
const RAID_TOTAL_TICKS_60HZ: u64 = 176;
const INPUT_LEAD_TICKS: u64 = 4; // 66.7ms client input lead vs 40.0ms one-way delay (80ms RTT)
const FIRST_SCHEDULED_SKILL_TICK: u64 = 12;
const LAST_SCHEDULED_SKILL_TICK: u64 = 142;
const RESONANCE_SALVO_TICK: u64 = 16;

static SERVER_WEAPON_QVM_SOURCES: [&str; 3] = [
    // Slot 0: Shatter Lance (Cryo)
    r#"
    fn calc_shatter(self, target) {
        read_shield sh_cur = target
        const_q32 lo = 8
        const_q32 hi = 28
        clamp_q32 sh_out = sh_cur, lo, hi
        return sh_out
    }
    fn on_tick(self, target) {
        call calc_shatter(self, target) -> sh
        const_q32 hp = 4
        const_q32 cg = 6
        emit_damage hp, sh, cg
    }
    "#,
    // Slot 1: Charge Chakram (Volt)
    r#"
    fn on_tick(self, target) {
        read_pos sp = self
        read_vel sv = self
        const_q32 dt_s = 1
        fma_vec3 pred = sv, dt_s, sp
        const_q32 hp = 5
        const_q32 sh = 4
        const_q32 cg = 12
        emit_damage hp, sh, cg
    }
    "#,
    // Slot 2: Discharge Katana (Pyro)
    r#"
    fn on_tick(self, target) {
        const_q32 base_hp = 4
        const_q32 scale = 2
        mul_q32 hp = base_hp, scale
        const_q32 sh = 7
        const_q32 cg = 5
        for i in 0..2 {
            emit_damage hp, sh, cg
        endfor
    }
    "#,
];

/// Heavy multi-trajectory Resonance Burst (`3,859 <= 4,000` ops per call) used during the 50-agent
/// synchronized opening salvo so live AOI shard `qvm` load genuinely crosses `80%` (`>= 160,000` ops)
/// and induces an online elastic channel split before `SERVER_SHARD_TICK_FUEL_CAP (200,000)` (`§8.3`).
static SERVER_RESONANCE_BURST_QVM_SOURCE: &str = r#"
    fn on_tick(self, target) {
        read_pos sp = self
        read_vel sv = self
        read_pos tp = target
        read_shield sh_cur = target
        const_q32 dt_s = 1
        const_q32 lo = 2
        const_q32 hi = 18
        for i in 0..246 {
            fma_vec3 p1 = sv, dt_s, sp
            fma_vec3 p2 = sv, dt_s, p1
            fma_vec3 p3 = sv, dt_s, p2
            clamp_q32 c1 = sh_cur, lo, hi
            mul_q32 m1 = c1, dt_s
            clamp_q32 c2 = m1, lo, hi
        endfor
        const_q32 hp = 8
        const_q32 sh = 12
        const_q32 cg = 16
        emit_damage hp, sh, cg
    }
"#;

static SERVER_BOSS_QVM_SOURCE: &str = r#"
    fn on_tick(self, target) {
        read_pos bp = self
        const_q32 rad = 10
        const_q32 slow = 1
        emit_phantasia bp, rad, slow
    }
"#;

struct ReconnectCompletion {
    new_endpoint: QuicClientEndpoint,
    granted_ticket: HmacSessionTicket,
    early_streams: Vec<Vec<u8>>,
    elapsed_us: u64,
}

struct LiveClientSimAgent {
    account_id: u64,
    entity: EntityId,
    endpoint: QuicClientEndpoint,
    window: VecDeque<RawWireInput>,
    ack_server_tick: u64,
    ack_bitfield: u64,
    partial_delta_chunks: BTreeMap<u64, (u8, u8)>,
    latest_ticket: HmacSessionTicket,
    reconciled_self_pos: QVec3,
    reconciled_self_tick: u64,
    visual_self_pos: QVec3,
    spring_offset_pos: QVec3,
    spring_offset_vel: QVec3,
    reconciled_boss_pos: QVec3,
    client_epochs: DualSlotEpochTable,
    client_lease: EncounterEpochLease,
    decoded_deltas: u64,
    received_epoch_streams: u32,
    received_sdf_streams: u32,
}

impl LiveClientSimAgent {
    fn push_and_encode_datagram(&mut self, raw: RawWireInput) -> Result<Vec<u8>, &'static str> {
        if self.window.len() == SLIDING_WINDOW_TICKS {
            let _ = self.window.pop_back();
        }
        self.window.push_front(raw);
        let mut arr = [RawWireInput::default(); SLIDING_WINDOW_TICKS];
        let win_len = self.window.len();
        for (idx, item) in self.window.iter().enumerate() {
            if let Some(dst) = arr.get_mut(idx) {
                *dst = *item;
            }
        }
        let dgram = ClientInputDatagram::new(
            self.account_id,
            self.ack_server_tick,
            self.ack_bitfield,
            win_len as u8,
            arr,
            self.latest_ticket,
        );
        let bytes = dgram
            .encode()
            .map_err(|_| "ClientInputDatagram encode failed")?;
        if bytes.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err("ClientInputDatagram exceeded 1,100B payload cap");
        }
        Ok(bytes)
    }

    fn apply_inbound_uni_stream(
        &mut self,
        stream_bytes: &[u8],
        replica_opt: Option<&mut SparseSdfWorld>,
    ) -> Result<(), &'static str> {
        match stream_bytes.first().copied() {
            Some(STREAM_KIND_SDF_CHUNK_DIFF) => {
                let pkt = SdfChunkDiffStreamPacket::decode_stream_frame(stream_bytes)
                    .map_err(|_| "Client failed decoding SDF chunk diff stream")?;
                if let Some(rep_world) = replica_opt {
                    let _ = pkt
                        .apply_to_world(rep_world)
                        .map_err(|_| "SDF brick BLAKE3 digest mismatch on client replica")?;
                }
                self.received_sdf_streams = self.received_sdf_streams.saturating_add(1);
            }
            Some(STREAM_KIND_EPOCH_MANIFEST) => {
                let staged = CanonicalEpochBundle::decode_stream_tlv(stream_bytes, false)
                    .map_err(|_| "Client failed decoding streamed Epoch TLV")?;
                let _ = self.client_epochs.try_reclaim_zero_ref_standby();
                self.client_epochs
                    .mount_staged_epoch(staged)
                    .map_err(|_| "Client DualSlotEpochTable mount_staged_epoch failed")?;
                self.received_epoch_streams = self.received_epoch_streams.saturating_add(1);
            }
            Some(STREAM_KIND_SESSION_TICKET_GRANT) => {
                let body = stream_bytes
                    .get(1..)
                    .ok_or("Truncated STREAM_KIND_SESSION_TICKET_GRANT")?;
                self.latest_ticket = HmacSessionTicket::decode(body)
                    .map_err(|_| "Failed decoding granted HmacSessionTicket")?;
            }
            _ => {}
        }
        Ok(())
    }
}

struct RaidTelemetry {
    udp_dropped: u64,
    udp_delivered: u64,
    server_rewinds: u32,
    evaporated_skills: u32,
    committed_skills: u32,
    teleport_anomalies: u32,
    tampered_rejected: u32,
    portal_transfers: u32,
    portal_prewarms: u32,
    ghost_mirrors_peak: usize,
    elastic_channel_splits: u32,
    max_shard_compute_us: u64,
    max_shard_fuel: u32,
    max_call_fuel: u32,
    reconnect_latency_us: u64,
    sdf_chunk_streams_sent: u32,
    sdf_bricks_converged: usize,
    wal_committed_acks: usize,
    max_wal_flush_us: u64,
    mid_combat_epoch_drift: u32,
    reclaimed_epochs: Vec<u64>,
    legacy_preserved_count: usize,
    season_weapons_used: u32,
    trait_casts: u32,
    augment_casts: u32,
    world_event_casts: u32,
}

fn build_canonical_e0_and_e01() -> (CanonicalEpochBundle, CanonicalEpochBundle) {
    let e0_summary = EpochManifestSummary {
        epoch_id: 0,
        season_id: 1,
        merkle_root: [0u8; 32],
        faucet_caps: [1_000_000; 8],
        base_move_speed_q32: Q32::from_ratio_i32(10_000, 1_000),
        expected_farm_ticks: 600,
        synth_clear_permille: 500,
        synth_exit_mean_permille: 20,
        synth_exit_std_permille: 10,
    };
    let e01_summary = EpochManifestSummary {
        epoch_id: 1,
        season_id: 1,
        merkle_root: [0u8; 32],
        faucet_caps: [1_200_000; 8],
        base_move_speed_q32: Q32::from_ratio_i32(10_200, 1_000),
        expected_farm_ticks: 580,
        synth_clear_permille: 510,
        synth_exit_mean_permille: 18,
        synth_exit_std_permille: 9,
    };
    let qvm_digest = blake3_digest(SERVER_WEAPON_QVM_SOURCES[0].as_bytes());
    let spirv_digest = blake3_digest(b"engine.slang:spirv_1_6:v1");
    let chunk_digest = blake3_digest(b"chunks:(0,0,1),(1,0,2),(0,1,3),(1,1,4)");

    let e0 = CanonicalEpochBundle::seal_new(
        e0_summary,
        qvm_digest,
        spirv_digest,
        chunk_digest,
        0,
        1_000,
    );
    let e01 = CanonicalEpochBundle::seal_new(
        e01_summary,
        qvm_digest,
        spirv_digest,
        chunk_digest,
        1,
        1_040,
    );
    (e0, e01)
}

/// Synthesizes deterministic per-agent 3D mobility + multi-weapon skill input strictly from the client's
/// own wire-reconciled `EntityDelta` positions (zero access to server `&CombatArena`!).
fn synthesize_raid_agent_raw_input(
    agent_idx: usize,
    target_tick: u64,
    reconciled_self_pos: QVec3,
    reconciled_boss_pos: QVec3,
    allow_scheduled_skills: bool,
) -> RawWireInput {
    // Ticks `0..=16`: stage in OpenWorldField cell `(0, 0)` near `(28, 12, 27)` for synchronized Resonance Salvo;
    // Ticks `>= 17`: sweep across `(32, 12, 32)` SeamlessPortalGate into RaidBossInstance orbiting the Boss!
    let anchor = if target_tick <= 16 {
        QVec3::from_ints(28, 12, 27)
    } else {
        reconciled_boss_pos
    };

    let phase_off = (agent_idx as i32).saturating_mul(7);
    let orbit_x = anchor
        .x
        .to_int_floor()
        .saturating_add(((phase_off.saturating_add(target_tick as i32)) & 7).saturating_sub(4));
    let orbit_z = anchor
        .z
        .to_int_floor()
        .saturating_add(((phase_off.saturating_sub(target_tick as i32)) & 7).saturating_sub(4));

    let dx = orbit_x.saturating_sub(reconciled_self_pos.x.to_int_floor());
    let dz = orbit_z.saturating_sub(reconciled_self_pos.z.to_int_floor());
    let mx = dx.saturating_mul(220).clamp(-900, 900);
    let mz = dz.saturating_mul(220).clamp(-900, 900);

    let cyc = target_tick.saturating_add(agent_idx as u64);
    let mut mask: u16 = 0;

    // Cycle all 5 inertia-preserving 3D mobility modes:
    match cyc & 15 {
        0..=2 => mask |= actions::JETPACK_THRUST,
        3..=4 => mask |= actions::GRAPPLE_HOOK,
        5..=7 => mask |= actions::GLIDE_WING,
        8..=10 => mask |= actions::WALL_RUN,
        _ => mask |= actions::SURFACE_SLIDE,
    }

    if allow_scheduled_skills
        && (FIRST_SCHEDULED_SKILL_TICK..=LAST_SCHEDULED_SKILL_TICK).contains(&target_tick)
    {
        if target_tick == RESONANCE_SALVO_TICK {
            // Synchronized 50-agent Raid Resonance Salvo inducing online elastic channel split (`§4.1, §8.3`)!
            mask |= actions::PRIMARY_ATTACK | actions::ELEMENTAL_SKILL;
        } else {
            if cyc.is_multiple_of(4) {
                mask |= actions::PRIMARY_ATTACK;
            }
            if cyc.is_multiple_of(9) {
                mask |= actions::ELEMENTAL_SKILL;
            }
            if cyc.is_multiple_of(11) {
                mask |= actions::DODGE_PHANTASIA;
            }
            if cyc.is_multiple_of(13) {
                mask |= actions::PARRY_CLASH;
            }
        }
    }

    let weapon_slot = ((cyc.checked_div(6).unwrap_or(0)).checked_rem(3).unwrap_or(0)) as u8;
    let yaw_milli = ((cyc.saturating_mul(37).checked_rem(1_600).unwrap_or(0)) as i32)
        .saturating_sub(800);
    let pitch_milli =
        ((cyc.saturating_mul(13).checked_rem(200).unwrap_or(0)) as i32).saturating_sub(100);

    RawWireInput {
        tick: target_tick,
        move_dir_milli: [mx, 0, mz],
        aim_yaw_turn_milli: yaw_milli,
        aim_pitch_turn_milli: pitch_milli,
        action_mask: u32::from(mask),
        active_weapon_slot: weapon_slot,
    }
}

struct StepTickContext<'a> {
    world: &'a mut SparseSdfWorld,
    arena: &'a mut CombatArena,
    aoi_grid: &'a mut AoiSpatialGrid,
    cmd_buf: &'a mut CommandBuffer,
    weapon_pool: &'a [CompiledQvmModule],
    trait_qvm: &'a [CompiledQvmModule],
    augment_qvm: &'a [CompiledQvmModule],
    world_event_qvm: Option<&'a CompiledQvmModule>,
    resonance_burst_qvm: &'a CompiledQvmModule,
    boss_qvm: &'a CompiledQvmModule,
    players: &'a [EntityId],
    boss: EntityId,
    portal_gate: &'a SeamlessPortalGate,
    agent_zones: &'a mut [ZoneInstanceKind],
    entity_channels: &'a mut [u16],
    pinned_bundle: &'a CanonicalEpochBundle,
}

struct StepTickMetrics {
    max_shard_us: u64,
    max_shard_fuel: u32,
    max_call_fuel: u32,
    ghost_count: usize,
    portal_prewarms: u32,
    portal_transfers: u32,
    elastic_splits: u32,
    applied_speed_cap_raw: i64,
    weapon_pool_mask: u32,
    trait_casts: u32,
    augment_casts: u32,
    world_event_casts: u32,
}

const GATE1_AUGMENT_TICK: u64 = 110;
const GATE2_AUGMENT_TICK: u64 = 145;

/// Executes one sandboxed `qvm` handler inside a shard, charging its fuel to the primary or overflow channel.
fn run_shard_qvm(
    module: &CompiledQvmModule,
    arena: &CombatArena,
    src: EntityId,
    dst: EntityId,
    cmd_buf: &mut CommandBuffer,
    split_active: bool,
    primary_fuel: &mut u32,
    overflow_fuel: &mut u32,
    max_call_fuel: &mut u32,
) -> Result<(), &'static str> {
    let f = execute_handler(module, &arena.view(), src, dst, cmd_buf)
        .map_err(|_| "Server shard qvm handler trapped")?;
    if f > INVOCATION_FUEL_CAP {
        return Err("Shard qvm invocation exceeded 4,000 fuel cap");
    }
    *max_call_fuel = (*max_call_fuel).max(f);
    if split_active {
        *overflow_fuel = overflow_fuel.saturating_add(f);
    } else {
        *primary_fuel = primary_fuel.saturating_add(f);
    }
    Ok(())
}

fn build_movement_hold_frame(f: InputFrame, tick: u64) -> Option<InputFrame> {
    let mv = f.move_vec();
    let to_milli = |q: Q32| -> i32 {
        q.sat_mul(Q32::from_int(1_000))
            .to_int_floor()
            .clamp(-1_000, 1_000)
    };
    let mobility_only_mask = f.action_mask()
        & (actions::JETPACK_THRUST
            | actions::GRAPPLE_HOOK
            | actions::GLIDE_WING
            | actions::WALL_RUN
            | actions::SURFACE_SLIDE);
    let raw = RawWireInput {
        tick,
        move_dir_milli: [to_milli(mv.x), to_milli(mv.y), to_milli(mv.z)],
        aim_yaw_turn_milli: 0,
        aim_pitch_turn_milli: 0,
        action_mask: u32::from(mobility_only_mask),
        active_weapon_slot: f.weapon_slot().min(2),
    };
    Packet::<Unverified>::new_unverified(raw)
        .verify_bounds()
        .ok()
        .map(Packet::into_input_frame)
}

fn active_channel_set_from_slice(players: &[EntityId], entity_channels: &[u16]) -> BTreeSet<u16> {
    let mut set = BTreeSet::new();
    for &pid in players {
        let ch = entity_channels.get(pid.0 as usize).copied().unwrap_or(0);
        set.insert(ch);
    }
    if set.is_empty() {
        set.insert(0);
    }
    set
}

/// Executes 1 deterministic authoritative `60Hz` simulation tick across all active AOI shards & channels.
/// Identical codepath for forward stepping AND lag-compensation rewind replays (`0` state divergence).
fn execute_authoritative_tick_step(
    tick: u64,
    tick_inputs: &BTreeMap<EntityId, InputFrame>,
    default_hold_frame: InputFrame,
    is_replay: bool,
    ctx: &mut StepTickContext<'_>,
) -> Result<StepTickMetrics, &'static str> {
    let dt_60hz = Q32::from_ratio_i32(1, 60);

    // Derive movement speed cap & Boss shield scaling directly from the encounter-pinned `CanonicalEpochBundle` (`§5.3, §9.2`)!
    let base_speed = ctx
        .pinned_bundle
        .pinned_handle
        .manifest()
        .base_move_speed_q32;
    let v_cap = base_speed
        .sat_mul(Q32::from_int(24))
        .sat_mul(Q32::from_ratio_i32(1, 10));
    let v_neg = v_cap.sat_neg();

    let mut metrics = StepTickMetrics {
        max_shard_us: 0,
        max_shard_fuel: 0,
        max_call_fuel: 0,
        ghost_count: 0,
        portal_prewarms: 0,
        portal_transfers: 0,
        elastic_splits: 0,
        applied_speed_cap_raw: v_cap.raw(),
        weapon_pool_mask: 0,
        trait_casts: 0,
        augment_casts: 0,
        world_event_casts: 0,
    };

    let active_channels = active_channel_set_from_slice(ctx.players, ctx.entity_channels);
    ctx.aoi_grid
        .rebuild_from_arena_channels(ctx.arena, ctx.entity_channels, &active_channels);
    metrics.ghost_count = ctx.aoi_grid.total_ghost_mirrors();

    // Group active players by their authoritative spatial AOI shard `(channel_id, sx, sz)`:
    let mut shard_players: BTreeMap<ShardCoord, Vec<(usize, EntityId)>> = BTreeMap::new();
    for (idx, &pid) in ctx.players.iter().enumerate() {
        let p_pos = ctx.arena.position(pid).unwrap_or(QVec3::ZERO);
        let ch = ctx.entity_channels.get(pid.0 as usize).copied().unwrap_or(0);
        let sc = ShardCoord::from_qvec3(ch, p_pos);
        shard_players.entry(sc).or_default().push((idx, pid));
    }

    // Step Boss deterministic pursuit against lead threat player:
    if let Some(&lead_p) = ctx.players.first() {
        ctx.arena.step_boss_pursuit(ctx.boss, lead_p, dt_60hz);
    }

    let boss_pos_pre = ctx.arena.position(ctx.boss).unwrap_or(QVec3::ZERO);
    let boss_ch = ctx
        .entity_channels
        .get(ctx.boss.0 as usize)
        .copied()
        .unwrap_or(0);
    let boss_home_shard = ShardCoord::from_qvec3(boss_ch, boss_pos_pre);
    let mut boss_qvm_accounted = false;

    ctx.cmd_buf.clear();

    // Execute per-shard authoritative combat + `qvm` handlers within `<= 4.00ms` & `<= 200,000 ops` (`§8.3, §9.1`),
    // dynamically inducing a live channel split whenever a shard reaches `80%` (`160,000` ops) of its fuel cap!
    for (&shard, members) in &shard_players {
        let mut primary_t0 = Instant::now();
        let mut primary_fuel: u32 = 0;
        let mut overflow_t0: Option<Instant> = None;
        let mut overflow_fuel: u32 = 0;
        let mut split_active = false;

        // Account Boss `qvm` pattern modulation inside its home shard's `SERVER_SHARD_TICK_FUEL_CAP` budget!
        if shard == boss_home_shard
            && tick.is_multiple_of(15)
            && let Some(&lead_p) = ctx.players.first()
        {
            let b_fuel = execute_handler(
                ctx.boss_qvm,
                &ctx.arena.view(),
                ctx.boss,
                lead_p,
                ctx.cmd_buf,
            )
            .map_err(|_| "Server boss qvm handler trapped")?;
            if b_fuel > INVOCATION_FUEL_CAP {
                return Err("Boss qvm invocation exceeded 4,000 fuel cap");
            }
            if b_fuel > metrics.max_call_fuel {
                metrics.max_call_fuel = b_fuel;
            }
            primary_fuel = primary_fuel.saturating_add(b_fuel);
            boss_qvm_accounted = true;
        }

        for (mem_pos, &(agent_idx, pid)) in members.iter().enumerate() {
            if !split_active
                && AoiSpatialGrid::should_induce_channel_split(
                    primary_fuel,
                    SERVER_SHARD_TICK_FUEL_CAP,
                )
            {
                // Online elastic channel split (`§4.1, §8.3`): migrate overflow + half of shard roster to `channel_id + 1`!
                let primary_us =
                    primary_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                if primary_us > metrics.max_shard_us {
                    metrics.max_shard_us = primary_us;
                }
                split_active = true;
                overflow_t0 = Some(Instant::now());
                let next_channel = shard.channel_id.saturating_add(1);
                let half_idx = members.len() / 2;
                for (m_i, &(_, m_pid)) in members.iter().enumerate() {
                    if (m_i >= half_idx || m_i >= mem_pos)
                        && let Some(ch_slot) = ctx.entity_channels.get_mut(m_pid.0 as usize)
                    {
                        *ch_slot = next_channel;
                    }
                }
                metrics.elastic_splits = metrics.elastic_splits.saturating_add(1);
                // Reset primary timer as primary shard work is now sealed:
                primary_t0 = Instant::now();
            }

            let frame = tick_inputs
                .get(&pid)
                .copied()
                .unwrap_or(default_hold_frame);
            ctx.arena
                .apply_player_input(pid, &frame, dt_60hz, ctx.cmd_buf);

            let act = frame.action_mask();
            let slot_us = pid.0 as usize;

            // Live open-world dynamic environmental event (`§1.1, §5.2`) for players still in `OpenWorldField`:
            if tick.checked_rem(15) == Some(7)
                && ctx.agent_zones.get(agent_idx).copied() == Some(ZoneInstanceKind::OpenWorldField)
                && let Some(we) = ctx.world_event_qvm
            {
                run_shard_qvm(
                    we,
                    ctx.arena,
                    pid,
                    ctx.boss,
                    ctx.cmd_buf,
                    split_active,
                    &mut primary_fuel,
                    &mut overflow_fuel,
                    &mut metrics.max_call_fuel,
                )?;
                if !is_replay {
                    metrics.world_event_casts = metrics.world_event_casts.saturating_add(1);
                }
            }

            if (act & actions::PRIMARY_ATTACK) != 0 {
                let is_resonance_salvo =
                    tick == RESONANCE_SALVO_TICK && (act & actions::ELEMENTAL_SKILL) != 0;
                // Per-player 3-weapon season loadout (`weapon_slots[active_weapon]` indexes the 9-weapon pool):
                let w_slot = (ctx.arena.active_weapon[slot_us] as usize).min(2);
                let pool_len = ctx.weapon_pool.len().max(1);
                let pool_idx = (ctx.arena.weapon_slots[slot_us][w_slot] as usize)
                    .checked_rem(pool_len)
                    .unwrap_or(0);
                let module = if is_resonance_salvo {
                    ctx.resonance_burst_qvm
                } else {
                    ctx.weapon_pool
                        .get(pool_idx)
                        .ok_or("Season weapon pool is empty")?
                };
                if !is_resonance_salvo && !is_replay {
                    metrics.weapon_pool_mask |= 1_u32.checked_shl(pool_idx as u32).unwrap_or(0);
                }
                let call_fuel = execute_handler(
                    module,
                    &ctx.arena.view(),
                    pid,
                    ctx.boss,
                    ctx.cmd_buf,
                )
                .map_err(|_| "Server weapon/resonance qvm handler trapped")?;
                if call_fuel > INVOCATION_FUEL_CAP {
                    return Err("Weapon qvm invocation exceeded 4,000 fuel cap");
                }
                if call_fuel > metrics.max_call_fuel {
                    metrics.max_call_fuel = call_fuel;
                }
                if split_active {
                    overflow_fuel = overflow_fuel.saturating_add(call_fuel);
                } else {
                    primary_fuel = primary_fuel.saturating_add(call_fuel);
                }

                if !is_resonance_salvo {
                    // Live gate tactical augments chosen at portal entry / Gate #1 / Gate #2 (`Silver/Gold/Prism`, `§2.2`):
                    let tiers = ctx.arena.augment_tiers[slot_us];
                    for tier in tiers {
                        if tier == 0 {
                            continue;
                        }
                        if let Some(aug) = ctx.augment_qvm.get(usize::from(tier).saturating_sub(1)) {
                            run_shard_qvm(
                                aug,
                                ctx.arena,
                                pid,
                                ctx.boss,
                                ctx.cmd_buf,
                                split_active,
                                &mut primary_fuel,
                                &mut overflow_fuel,
                                &mut metrics.max_call_fuel,
                            )?;
                            if !is_replay {
                                metrics.augment_casts = metrics.augment_casts.saturating_add(1);
                            }
                        }
                    }
                    // Multi-axis resonance trait (`Simulacrum` / `Matrix` / `Relic+Emblem`) on elemental skill casts:
                    if (act & actions::ELEMENTAL_SKILL) != 0 && !ctx.trait_qvm.is_empty() {
                        let tm = ctx.arena.trait_mask[slot_us];
                        let t_idx: usize = if (tm & 0b0000_0111) != 0 {
                            0
                        } else if (tm & 0b0001_1000) != 0 {
                            1
                        } else {
                            2
                        };
                        if let Some(tq) = ctx.trait_qvm.get(t_idx) {
                            run_shard_qvm(
                                tq,
                                ctx.arena,
                                pid,
                                ctx.boss,
                                ctx.cmd_buf,
                                split_active,
                                &mut primary_fuel,
                                &mut overflow_fuel,
                                &mut metrics.max_call_fuel,
                            )?;
                            if !is_replay {
                                metrics.trait_casts = metrics.trait_casts.saturating_add(1);
                            }
                        }
                    }
                }
            }
        }

        if primary_fuel > SERVER_SHARD_TICK_FUEL_CAP || overflow_fuel > SERVER_SHARD_TICK_FUEL_CAP
        {
            return Err("AOI shard exceeded SERVER_SHARD_TICK_FUEL_CAP (200,000 ops)");
        }
        metrics.max_shard_fuel = metrics.max_shard_fuel.max(primary_fuel).max(overflow_fuel);

        if let Some(ov_t0) = overflow_t0 {
            let ov_us = ov_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            if ov_us > metrics.max_shard_us {
                metrics.max_shard_us = ov_us;
            }
        } else {
            let shard_us = primary_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            if shard_us > metrics.max_shard_us {
                metrics.max_shard_us = shard_us;
            }
        }
    }

    // If Boss stood in a spatial cell with 0 players on a 15-tick boundary, account its own shard fuel & time:
    if !boss_qvm_accounted
        && tick.is_multiple_of(15)
        && let Some(&lead_p) = ctx.players.first()
    {
        let b_t0 = Instant::now();
        let b_fuel = execute_handler(
            ctx.boss_qvm,
            &ctx.arena.view(),
            ctx.boss,
            lead_p,
            ctx.cmd_buf,
        )
        .map_err(|_| "Server boss qvm handler trapped")?;
        if b_fuel > INVOCATION_FUEL_CAP || b_fuel > SERVER_SHARD_TICK_FUEL_CAP {
            return Err("Boss standalone shard qvm exceeded fuel cap");
        }
        metrics.max_call_fuel = metrics.max_call_fuel.max(b_fuel);
        metrics.max_shard_fuel = metrics.max_shard_fuel.max(b_fuel);
        let b_us = b_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        metrics.max_shard_us = metrics.max_shard_us.max(b_us);
    }

    // Rebuild AOI channels if an elastic split occurred this tick so visibility reflects the split immediately:
    if metrics.elastic_splits > 0 || is_replay {
        let updated_channels = active_channel_set_from_slice(ctx.players, ctx.entity_channels);
        ctx.aoi_grid
            .rebuild_from_arena_channels(ctx.arena, ctx.entity_channels, &updated_channels);
    }

    ctx.arena.commit_commands(ctx.cmd_buf);
    ctx.arena.step_kinematics(dt_60hz);
    // Bound live projectile population to the raid arena (deterministic on forward ticks AND rewind replays):
    let _ = ctx
        .arena
        .despawn_projectiles_beyond(QVec3::from_ints(32, 14, 32), Q32::from_int(48));

    // Resolve authoritative 3D SDF contacts, epoch-governed velocity cap, & deterministic portal transitions:
    for (agent_idx, &pid) in ctx.players.iter().enumerate() {
        ctx.world
            .resolve_entity_sdf_contacts(ctx.arena, pid, dt_60hz);
        if let Some(mut p) = ctx.arena.position(pid) {
            let mut v = ctx.arena.velocity(pid).unwrap_or(QVec3::ZERO);
            v.x = v.x.clamp_val(v_neg, v_cap);
            v.y = v.y.clamp_val(v_neg, v_cap);
            v.z = v.z.clamp_val(v_neg, v_cap);
            if p.x < Q32::from_int(5) || p.x > Q32::from_int(59) {
                p.x = p.x.clamp_val(Q32::from_int(5), Q32::from_int(59));
                v.x = Q32::ZERO;
            }
            if p.y < Q32::from_int(2) || p.y > Q32::from_int(26) {
                p.y = p.y.clamp_val(Q32::from_int(2), Q32::from_int(26));
                v.y = Q32::ZERO;
            }
            if p.z < Q32::from_int(5) || p.z > Q32::from_int(59) {
                p.z = p.z.clamp_val(Q32::from_int(5), Q32::from_int(59));
                v.z = Q32::ZERO;
            }
            if ctx.arena.hp[pid.0 as usize] < Q32::from_int(500) {
                ctx.arena.hp[pid.0 as usize] = Q32::from_int(10_000);
            }
            ctx.arena.set_position(pid, p);
            ctx.arena.set_velocity(pid, v);
        }

        // Step portal unconditionally during both forward ticks AND rewind replays (`0` replay divergence!):
        if let Some(zone_slot) = ctx.agent_zones.get_mut(agent_idx) {
            let outcome = ctx
                .portal_gate
                .step_entity_portal(zone_slot, ctx.arena, pid);
            let in_raid = *zone_slot == ZoneInstanceKind::RaidBossInstance;
            // Deterministic 3-tier gate tactical augment selection (rollback-safe: lives in `CombatArena`):
            let base_tier = (agent_idx.checked_rem(3).unwrap_or(0) as u8).saturating_add(1);
            let tiers = &mut ctx.arena.augment_tiers[pid.0 as usize];
            if matches!(outcome, PortalStepOutcome::SeamlessTransferred { .. }) && tiers[0] == 0 {
                tiers[0] = base_tier;
            }
            if in_raid && tick == GATE1_AUGMENT_TICK {
                tiers[1] = (base_tier.checked_rem(3).unwrap_or(0)).saturating_add(1);
            }
            if in_raid && tick == GATE2_AUGMENT_TICK {
                tiers[2] = (base_tier.saturating_add(1).checked_rem(3).unwrap_or(0)).saturating_add(1);
            }
            match outcome {
                PortalStepOutcome::PrewarmingDestChunk(_) if !is_replay => {
                    metrics.portal_prewarms = metrics.portal_prewarms.saturating_add(1);
                }
                PortalStepOutcome::SeamlessTransferred { .. } if !is_replay => {
                    metrics.portal_transfers = metrics.portal_transfers.saturating_add(1);
                }
                _ => {}
            }
        }
    }

    ctx.world
        .resolve_entity_sdf_contacts(ctx.arena, ctx.boss, dt_60hz);
    if ctx.arena.hp[ctx.boss.0 as usize] <= Q32::from_int(1_000) {
        // Multi-phase Boss transition governed by encounter-pinned `boss_tuning_permille` (`§4.1, §5.3`):
        let tuned_shield = i32::try_from(
            2_500_u64
                .saturating_mul(ctx.pinned_bundle.boss_tuning_permille)
                .checked_div(1_000)
                .unwrap_or(2_500),
        )
        .unwrap_or(2_500);
        ctx.arena.hp[ctx.boss.0 as usize] = Q32::from_int(120_000);
        ctx.arena.shield[ctx.boss.0 as usize] = Q32::from_int(tuned_shield);
    }

    // Authoritative combat-driven 3D SDF CSG destruction & elemental field reactions (`ticks 0..=160`)
    // tagged at `tick + 1` so `world.rewind_to_tick(L)` undoes mutations strictly from ticks `>= L`
    // and all mutations finalize (`160 + 9 = 169 < 176`) before raid end for bit-exact replica convergence!
    if tick <= 160 && tick.is_multiple_of(20) {
        let mut_tag = tick.saturating_add(1);
        let cx_spot = 14_i32.saturating_add(((tick as i32) & 1).saturating_mul(24));
        let cz_spot = 14_i32.saturating_add((((tick >> 1) as i32) & 1).saturating_mul(24));
        let mut surf_y = 20_i32;
        while surf_y > 3 {
            if ctx.world.sample_sdf_cm(cx_spot, surf_y, cz_spot) < 0 {
                break;
            }
            surf_y = surf_y.saturating_sub(1);
        }
        let _ = ctx
            .world
            .carve_sphere_csg(mut_tag, [cx_spot, surf_y, cz_spot], 1);
        let stim = match (tick / 20) & 3 {
            0 => ElementStimulus::CryoFreeze,
            1 => ElementStimulus::PyroThermal,
            2 => ElementStimulus::VoltDischarge,
            _ => ElementStimulus::VoidGravityInvert,
        };
        let _ = ctx
            .world
            .apply_element_stimulus(mut_tag, [cx_spot, surf_y, cz_spot], 3, stim);
    }

    Ok(metrics)
}

async fn run_phase2_raid_and_hotswap_drill() -> Result<RaidTelemetry, &'static str> {
    // 1. Initialize Pure-Rust QUIC v1 TLS 1.3 (`Ed25519` + `X25519` + `AES-128-GCM` + Header Protection)
    //    over real OS `tokio::net::UdpSocket` with `5%` packet loss and `80ms` RTT active from packet 0:
    let (srv_tls, cli_tls) = build_pinned_quic_tls_configs([0x42; 32])
        .map_err(|_| "Failed building pinned QUIC v1 TLS 1.3 configs")?;
    let impairment = UdpLinkImpairment::phase2_raid_impairment();
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let quic_server = QuicServerEndpoint::bind(bind_addr, srv_tls, 0x2026_0202, impairment)
        .await
        .map_err(|_| "Failed binding authoritative QuicServerEndpoint")?;
    let server_addr = quic_server.local_addr();

    // 2. Initialize 4 vertical 3D `SparseSdfWorld` chunks (`server` + 2 client replicas: reconnecting `client[0]` & continuous `client[1]`):
    let mut world = SparseSdfWorld::new();
    let mut replica_world_0 = SparseSdfWorld::new();
    let mut replica_world_1 = SparseSdfWorld::new();
    for (cx, cz, seed) in [(0, 0, 1_u64), (1, 0, 2), (0, 1, 3), (1, 1, 4)] {
        world.generate_vertical_chunk(cx, cz, seed);
        replica_world_0.generate_vertical_chunk(cx, cz, seed);
        replica_world_1.generate_vertical_chunk(cx, cz, seed);
        let topo = world.analyze_chunk_topology(cx, cz);
        if topo.walkable_surface_voxels < 256
            || topo.sigma_z_q32 < Q32::from_int(2)
            || !topo.fully_3d_connected
        {
            return Err("Server 3D SDF chunk failed G1 vertical topology check");
        }
    }
    let _ = world.drain_dirty_bricks();
    let _ = replica_world_0.drain_dirty_bricks();
    let _ = replica_world_1.drain_dirty_bricks();

    // 3. Compile `qvm` weapon, synchronized resonance burst, & adaptive boss modules
    //    (streaming live `gameplay/` scripts from `E_1.ron` when present, with embedded portable fallback):
    let disk_e1 = load_season_gameplay_pack(
        Path::new("assets/manifests/E_1.ron"),
        Path::new("gameplay"),
    )
    .ok();
    let resolve_wpn = |slot: usize| -> String {
        if let Some(ref pack) = disk_e1
            && let Some(stem) = pack.manifest.weapon_modules.get(slot)
            && let Some(body) = pack.weapons.get(stem)
        {
            return format!("{}\n{body}", pack.prelude_src);
        }
        String::from(SERVER_WEAPON_QVM_SOURCES[slot])
    };
    let resolve_boss = || -> String {
        if let Some(ref pack) = disk_e1
            && let Some(stem) = pack.manifest.boss_modules.first()
            && let Some(body) = pack.bosses.get(stem)
        {
            return format!("{}\n{body}", pack.prelude_src);
        }
        String::from(SERVER_BOSS_QVM_SOURCE)
    };
    // Full live Season `E_1` combat pack: `9`-weapon pool, `3` resonance traits, `3` gate augments, world event:
    let mut weapon_pool: Vec<CompiledQvmModule> = Vec::new();
    let mut trait_qvm: Vec<CompiledQvmModule> = Vec::new();
    let mut augment_qvm: Vec<CompiledQvmModule> = Vec::new();
    let mut world_event_qvm: Option<CompiledQvmModule> = None;
    if let Some(ref pack) = disk_e1 {
        for stem in &pack.manifest.weapon_modules {
            let body = pack.weapons.get(stem).ok_or("E_1 weapon module missing on disk")?;
            weapon_pool.push(
                compile_qvm_source(&format!("{}\n{body}", pack.prelude_src))
                    .map_err(|_| "Failed compiling E_1 season weapon")?,
            );
        }
        for (stem, body) in &pack.traits {
            if stem.starts_with("e1_") {
                trait_qvm.push(
                    compile_qvm_source(&format!("{}\n{body}", pack.prelude_src))
                        .map_err(|_| "Failed compiling E_1 trait")?,
                );
            }
        }
        for (stem, body) in &pack.augments {
            if stem.starts_with("e1_") {
                augment_qvm.push(
                    compile_qvm_source(&format!("{}\n{body}", pack.prelude_src))
                        .map_err(|_| "Failed compiling E_1 augment")?,
                );
            }
        }
        if let Some((_, body)) = pack.world_events.iter().find(|(s, _)| s.starts_with("e1_")) {
            world_event_qvm = Some(
                compile_qvm_source(&format!("{}\n{body}", pack.prelude_src))
                    .map_err(|_| "Failed compiling E_1 world event")?,
            );
        }
    }
    let season_pack_live = !weapon_pool.is_empty();
    if !season_pack_live {
        for slot in 0..3 {
            weapon_pool.push(
                compile_qvm_source(&resolve_wpn(slot))
                    .map_err(|_| "Failed compiling fallback server weapon")?,
            );
        }
    }
    let s_boss = resolve_boss();
    let resonance_burst_qvm = compile_qvm_source(SERVER_RESONANCE_BURST_QVM_SOURCE)
        .map_err(|_| "Failed compiling server resonance burst qvm")?;
    let boss_qvm = compile_qvm_source(&s_boss).map_err(|_| "Failed compiling boss qvm")?;

    // 4. Spawn 50 Player entities in OpenWorldField cell `(0, 0)` near the `(32, 12, 32)` seamless portal + 1 Boss:
    let mut arena = CombatArena::new_boxed(0x5E2B_E200_2026);
    let mut players = Vec::with_capacity(RAID_AGENT_COUNT);
    let mut spawn_positions = Vec::with_capacity(RAID_AGENT_COUNT);
    for i in 0..RAID_AGENT_COUNT {
        let row = (i / 10) as i32; // 0..5 -> x = 26..30 (< 32, cell sx = 0, with >= 28 in ghost margin!)
        let col = (i % 10) as i32; // 0..10 -> z = 21..30 (< 32, cell sz = 0, with >= 28 in ghost margin!)
        let spawn_pos = QVec3::from_ints(
            26_i32.saturating_add(row),
            12,
            21_i32.saturating_add(col),
        );
        let pid = arena
            .spawn(
                EntityKind::Player,
                spawn_pos,
                Q32::from_int(20_000),
                Q32::from_int(800),
            )
            .ok_or("Failed spawning Raid Player entity")?;
        arena.trait_mask[pid.0 as usize] = 1_u32 << (i & 7);
        // Diverse per-agent 3-weapon loadout drawn from the season pool (`weapon_slots` indexes `weapon_pool`):
        let pl = weapon_pool.len().max(1);
        arena.weapon_slots[pid.0 as usize] = [
            i.checked_rem(pl).unwrap_or(0) as u16,
            i.saturating_add(3).checked_rem(pl).unwrap_or(0) as u16,
            i.saturating_add(6).checked_rem(pl).unwrap_or(0) as u16,
        ];
        players.push(pid);
        spawn_positions.push(spawn_pos);
    }
    let initial_boss_pos = QVec3::from_ints(34, 12, 32);
    let boss = arena
        .spawn(
            EntityKind::Boss,
            initial_boss_pos,
            Q32::from_int(120_000),
            Q32::from_int(2_500),
        )
        .ok_or("Failed spawning Multi-Phase Raid Boss")?;

    // Continuous 3D world archway `SeamlessPortalGate` (`0` loading screen, `100%` momentum preserved):
    let portal_gate = SeamlessPortalGate {
        portal_id: 101,
        from_zone: ZoneInstanceKind::OpenWorldField,
        to_zone: ZoneInstanceKind::RaidBossInstance,
        entry_center: QVec3::from_ints(32, 12, 32),
        exit_anchor: QVec3::from_ints(32, 12, 32),
        prewarm_radius: Q32::from_int(28),
        trigger_radius: Q32::from_int(11),
        dest_chunk: (1, 1),
    };
    // Verify rift-offset `SeamlessPortalGate` (`exit_anchor != entry_center`) preserves velocity vector 1:1:
    {
        let mut rift_arena = CombatArena::new_boxed(77);
        let probe = rift_arena
            .spawn(
                EntityKind::Player,
                QVec3::from_ints(10, 12, 10),
                Q32::from_int(1_000),
                Q32::from_int(100),
            )
            .ok_or("Failed rift probe spawn")?;
        let probe_vel = QVec3::from_ints(9, 4, -6);
        rift_arena.set_velocity(probe, probe_vel);
        let rift_gate = SeamlessPortalGate {
            portal_id: 102,
            from_zone: ZoneInstanceKind::OpenWorldField,
            to_zone: ZoneInstanceKind::RaidBossInstance,
            entry_center: QVec3::from_ints(10, 12, 10),
            exit_anchor: QVec3::from_ints(42, 14, 42),
            prewarm_radius: Q32::from_int(8),
            trigger_radius: Q32::from_int(3),
            dest_chunk: (1, 1),
        };
        let mut z = ZoneInstanceKind::OpenWorldField;
        let out = rift_gate.step_entity_portal(&mut z, &mut rift_arena, probe);
        if out
            != (PortalStepOutcome::SeamlessTransferred {
                new_zone: ZoneInstanceKind::RaidBossInstance,
                preserved_speed_raw: probe_vel.length().raw(),
            })
            || rift_arena.velocity(probe) != Some(probe_vel)
        {
            return Err("Rift SeamlessPortalGate failed to preserve momentum vector 1:1");
        }
    }

    let mut agent_zones = vec![ZoneInstanceKind::OpenWorldField; RAID_AGENT_COUNT];
    let mut entity_channels = vec![0_u16; MAX_ENTITIES];

    // 5. Initialize `(E_a, E_b)` `DualSlotEpochTable` pinned to `E_0` + `SessionTicketManager` + `64`-tick `SoA` Ringbuffer:
    let (e0_bundle, e01_bundle) = build_canonical_e0_and_e01();
    let e0_merkle = e0_bundle.full_merkle_root;
    let e01_merkle = e01_bundle.full_merkle_root;
    if e0_merkle == e01_merkle {
        return Err("CanonicalEpochBundle Merkle root failed to distinguish E_0 vs E_0.1");
    }
    let epoch_table = DualSlotEpochTable::new(e0_bundle.clone());
    let mut raid_lease = epoch_table.pin_for_new_encounter();
    if raid_lease.bundle().full_merkle_root != e0_merkle {
        return Err("Initial raid encounter lease not pinned to E_0");
    }

    let mut ticket_mgr = SessionTicketManager::new(&[0x7B; 32])
        .map_err(|_| "Failed creating SessionTicketManager")?;

    let default_hold_frame = Packet::<Unverified>::new_unverified(RawWireInput::default())
        .verify_bounds()
        .map_err(|_| "Default RawWireInput failed bounds check")?
        .into_input_frame();

    let mut ring64 = ServerSoARingBuffer64::new(0x5E2B_E200_2026);
    ring64.record_state_at_tick(0, &arena, &agent_zones, &entity_channels);

    // 6. Initialize Orthogonal Accounting Plane (`ledger_core` ⊥ `ledger_wal::GroupCommitWal`, `<= 2.0ms`),
    //    using idempotent top-ups so unlimited consecutive runs (`Run 1 .. Run N`) never overflow `150,000` shard quotas:
    let wal_dir = Path::new("state/wal/phase2_raid");
    let (mut wal, _wal_report) = GroupCommitWal::open_and_recover(wal_dir, 16)
        .map_err(|_| "Failed opening GroupCommitWal at state/wal/phase2_raid")?;
    let run_generation = (wal.committed_ledger().legacy_count() as u64).saturating_add(1);
    let minted_so_far =
        i64::try_from(wal.committed_ledger().total_faucet_minted_amount(AssetId(1))).unwrap_or(0);
    let mut total_allocated_after_topup: i64 = 0;
    for shard_id in 0_u16..4_u16 {
        let cur_rem = wal
            .committed_ledger()
            .shard_quota_remaining(shard_id, AssetId(1));
        total_allocated_after_topup =
            total_allocated_after_topup.saturating_add(cur_rem.max(150_000));
    }
    let initial_cap_e0 = minted_so_far
        .saturating_add(total_allocated_after_topup)
        .saturating_add(200_000);
    let upgraded_cap_e01 = initial_cap_e0.saturating_add(400_000);

    let wal_t0 = Instant::now();
    let mut total_wal_acks: usize = 0;
    let mut max_wal_flush_us: u64 = 0;

    let (_, acks) = wal
        .submit_op(
            WalOp::ConfigureAssetCap {
                asset: AssetId(1),
                epoch_cap: initial_cap_e0,
            },
            0,
        )
        .map_err(|_| "WAL ConfigureAssetCap E_0 failed")?;
    total_wal_acks = total_wal_acks.saturating_add(acks.len());

    for shard_id in 0_u16..4_u16 {
        let cur_rem = wal
            .committed_ledger()
            .shard_quota_remaining(shard_id, AssetId(1));
        let needed_top_up = 150_000_i64.saturating_sub(cur_rem);
        if needed_top_up > 0 {
            let (_, q_acks) = wal
                .submit_op(
                    WalOp::AllocateShardQuota {
                        shard_id,
                        asset: AssetId(1),
                        quota: needed_top_up,
                    },
                    100,
                )
                .map_err(|_| "WAL idempotent AllocateShardQuota top-up failed")?;
            total_wal_acks = total_wal_acks.saturating_add(q_acks.len());
        }
    }
    let flush_t = Instant::now();
    let init_acks = wal
        .flush_group_commit()
        .map_err(|_| "Initial WAL flush_group_commit failed")?;
    let f_us = flush_t.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    max_wal_flush_us = max_wal_flush_us.max(f_us);
    total_wal_acks = total_wal_acks.saturating_add(init_acks.len());

    // Verify adversarial WAL pre-validation rejects P2W power bonus, shard quota overflow (`> 150,000`), & double-spend:
    if !matches!(
        wal.submit_tx(
            LedgerTxKind::ExternalCosmeticPurchase {
                account: AccountId(1),
                cosmetic_id: 999_999,
                requested_power_bonus: 50,
            },
            200,
        ),
        Err(ledger_wal::WalError::Ledger(
            LedgerError::P2WPowerInfluenceForbidden
        ))
    ) {
        return Err("Ledger failed to block non-zero P2W power bonus purchase");
    }
    let shard0_rem = wal.committed_ledger().shard_quota_remaining(0, AssetId(1));
    if !matches!(
        wal.submit_tx(
            LedgerTxKind::FaucetMint {
                shard_id: 0,
                to: AccountId(1),
                asset: AssetId(1),
                amount: shard0_rem.saturating_add(1),
            },
            200,
        ),
        Err(ledger_wal::WalError::Ledger(
            LedgerError::ShardFaucetQuotaExceeded
        ))
    ) {
        return Err("Ledger failed to block shard faucet quota overflow");
    }
    let cur_acc49_bal = wal.committed_ledger().balance(AccountId(49), AssetId(1));
    if !matches!(
        wal.submit_tx(
            LedgerTxKind::Transfer {
                from: AccountId(49),
                to: AccountId(50),
                asset: AssetId(1),
                amount: cur_acc49_bal.saturating_add(10_000),
            },
            200,
        ),
        Err(ledger_wal::WalError::Ledger(
            LedgerError::InsufficientBalance
        ))
    ) {
        return Err("Ledger failed to pre-reject insufficient balance double-spend");
    }

    // 7. Connect 50 concurrent `QuicClientEndpoint`s over live impaired (`5%` loss / `80ms` RTT) UDP:
    let mut clients: Vec<LiveClientSimAgent> = Vec::with_capacity(RAID_AGENT_COUNT);
    let mut batch_start: usize = 0;
    while batch_start < RAID_AGENT_COUNT {
        let batch_end = batch_start.saturating_add(10).min(RAID_AGENT_COUNT);
        let mut join_handles = Vec::with_capacity(10);
        for idx in batch_start..batch_end {
            let cfg_cl = Arc::clone(&cli_tls);
            let seed = 0x1000_u64.saturating_add(idx as u64);
            join_handles.push(tokio::spawn(async move {
                QuicClientEndpoint::connect(server_addr, cfg_cl, seed, impairment).await
            }));
        }
        for (offset, jh) in join_handles.into_iter().enumerate() {
            let idx = batch_start.saturating_add(offset);
            let ep = jh
                .await
                .map_err(|_| "Client connect task panicked")?
                .map_err(|_| "QuicClientEndpoint TLS 1.3 handshake failed under 5% loss / 80ms RTT")?;
            let account_id = (idx as u64).saturating_add(1);
            let entity = players[idx];
            let ticket = ticket_mgr
                .issue_ticket(account_id, entity, 0, 1)
                .map_err(|_| "Initial HMAC ticket issue failed")?;
            let cli_epochs = DualSlotEpochTable::new(e0_bundle.clone());
            let cli_lease = cli_epochs.pin_for_new_encounter();
            clients.push(LiveClientSimAgent {
                account_id,
                entity,
                endpoint: ep,
                window: VecDeque::with_capacity(SLIDING_WINDOW_TICKS),
                ack_server_tick: 0,
                ack_bitfield: 0,
                partial_delta_chunks: BTreeMap::new(),
                latest_ticket: ticket,
                reconciled_self_pos: spawn_positions[idx],
                reconciled_self_tick: 0,
                visual_self_pos: spawn_positions[idx],
                spring_offset_pos: QVec3::ZERO,
                spring_offset_vel: QVec3::ZERO,
                reconciled_boss_pos: initial_boss_pos,
                client_epochs: cli_epochs,
                client_lease: cli_lease,
                decoded_deltas: 0,
                received_epoch_streams: 0,
                received_sdf_streams: 0,
            });
        }
        batch_start = batch_end;
    }

    // 8. Inject adversarial tampered packets over live QUIC datagrams and verify `100%` rejection:
    let mut tampered_rejected: u32 = 0;
    {
        if matches!(
            clients[0].endpoint.send_datagram(&[0xEE; 1_101]).await,
            Err(NetError::DatagramExceeds1100Bytes(1_101))
        ) {
            tampered_rejected = tampered_rejected.saturating_add(1);
        }
        let bad_move = RawWireInput {
            tick: 1,
            move_dir_milli: [4_500, 0, 0],
            aim_yaw_turn_milli: 0,
            aim_pitch_turn_milli: 0,
            action_mask: 0,
            active_weapon_slot: 0,
        };
        let bad_dg1 = ClientInputDatagram::new(
            1,
            0,
            0,
            1,
            [bad_move; SLIDING_WINDOW_TICKS],
            clients[0].latest_ticket,
        )
        .encode()
        .map_err(|_| "Encode bad_dg1 failed")?;
        if ClientInputDatagram::decode_and_verify(&bad_dg1, &ticket_mgr, 1).is_err() {
            tampered_rejected = tampered_rejected.saturating_add(1);
        }
        let bad_wpn = RawWireInput {
            tick: 1,
            move_dir_milli: [0, 0, 0],
            aim_yaw_turn_milli: 0,
            aim_pitch_turn_milli: 0,
            action_mask: 0,
            active_weapon_slot: 7,
        };
        let bad_dg2 = ClientInputDatagram::new(
            1,
            0,
            0,
            1,
            [bad_wpn; SLIDING_WINDOW_TICKS],
            clients[0].latest_ticket,
        )
        .encode()
        .map_err(|_| "Encode bad_dg2 failed")?;
        if ClientInputDatagram::decode_and_verify(&bad_dg2, &ticket_mgr, 1).is_err() {
            tampered_rejected = tampered_rejected.saturating_add(1);
        }
        let mut forged_ticket = clients[0].latest_ticket;
        forged_ticket.mac_tag[0] ^= 0xFF;
        let bad_dg3 = ClientInputDatagram::new(
            1,
            0,
            0,
            1,
            [RawWireInput::default(); SLIDING_WINDOW_TICKS],
            forged_ticket,
        )
        .encode()
        .map_err(|_| "Encode bad_dg3 failed")?;
        if matches!(
            ClientInputDatagram::decode_and_verify(&bad_dg3, &ticket_mgr, 1),
            Err(NetError::InvalidSessionTicket)
        ) {
            tampered_rejected = tampered_rejected.saturating_add(1);
        }
        let future_dos_raw = RawWireInput {
            tick: 999_999_999,
            move_dir_milli: [0, 0, 0],
            aim_yaw_turn_milli: 0,
            aim_pitch_turn_milli: 0,
            action_mask: 0,
            active_weapon_slot: 0,
        };
        let future_dos_dg = ClientInputDatagram::new(
            1,
            0,
            0,
            1,
            [future_dos_raw; SLIDING_WINDOW_TICKS],
            clients[0].latest_ticket,
        )
        .encode()
        .map_err(|_| "Encode future_dos_dg failed")?;
        for _ in 0..4 {
            let _ = clients[0].endpoint.send_datagram(&future_dos_dg).await;
        }
    }

    // State maps for the `60Hz` authoritative raid loop:
    let mut aoi_grid = AoiSpatialGrid::new();
    let mut cmd_buf = CommandBuffer::with_capacity(512);
    let mut account_to_cid: BTreeMap<u64, [u8; CID_LEN]> = BTreeMap::new();
    let mut client_ack_by_account: BTreeMap<u64, u64> = BTreeMap::new();
    let mut last_client_tick_by_account: BTreeMap<u64, u64> = BTreeMap::new();
    let mut entity_sync_by_account: BTreeMap<u64, ClientEntitySyncState> = BTreeMap::new();
    let mut authoritative_inputs: BTreeMap<(u64, EntityId), InputFrame> = BTreeMap::new();
    let mut last_movement_hold: BTreeMap<EntityId, (u64, InputFrame)> = BTreeMap::new();
    let mut scheduled_skills: BTreeMap<(u64, EntityId), u16> = BTreeMap::new();
    let mut pending_sdf_by_tick: BTreeMap<u64, Vec<BrickCoord>> = BTreeMap::new();
    let mut permanently_mutated_bricks: BTreeSet<BrickCoord> = BTreeSet::new();
    let mut prev_positions: BTreeMap<EntityId, QVec3> = BTreeMap::new();
    for &pid in &players {
        if let Some(pos) = arena.position(pid) {
            prev_positions.insert(pid, pos);
        }
    }

    let mut server_rewinds: u32 = 0;
    let mut evaporated_skills: u32 = 0;
    let mut committed_skills: u32 = 0;
    let mut teleport_anomalies: u32 = 0;
    let mut portal_prewarms_total: u32 = 0;
    let mut weapon_pool_mask_total: u32 = 0;
    let mut trait_casts_total: u32 = 0;
    let mut augment_casts_total: u32 = 0;
    let mut world_event_casts_total: u32 = 0;
    let mut ghost_mirrors_peak: usize = 0;
    let mut elastic_channel_splits: u32 = 0;
    let mut max_shard_compute_us: u64 = 0;
    let mut max_shard_fuel: u32 = 0;
    let mut max_call_fuel: u32 = 0;

    let mut reconn_rx: Option<oneshot::Receiver<Result<ReconnectCompletion, &'static str>>> = None;
    let mut client0_reconnecting = false;
    let mut revoked_pre_reconnect_ticket: Option<HmacSessionTicket> = None;
    let mut wire_reconnect_completed = false;
    let mut wire_replay_rejected = false;
    let mut reconnect_latency_us: u64 = 0;

    let mut sdf_chunk_streams_sent: u32 = 0;
    let mut mid_combat_epoch_drift: u32 = 0;
    let mut observed_e0_speed_cap: i64 = 0;
    let mut observed_e01_speed_cap: i64 = 0;
    let mut e01_legacy_item_id: u64 = 0;
    let mut sub_threshold_enqueued_at: Option<Instant> = None;
    let mut sub_threshold_auto_flush_us: u64 = 0;

    let max_step_q32 = Q32::from_int(3);
    let max_step_sq = max_step_q32.mul_wide(max_step_q32);

    let mut tick_interval = tokio::time::interval(Duration::from_micros(16_667));
    let mut wal_deadline_interval = tokio::time::interval(Duration::from_micros(500));
    let mut server_tick: u64 = 0;

    while server_tick < RAID_TOTAL_TICKS_60HZ {
        // Drive both the `60Hz` (`16.67ms`) simulation clock AND the `500us` (`0.5ms`) GroupCommitWal
        // `<= 2.0ms` micro-batch deadline ticker concurrently via `tokio::select!`:
        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    break;
                }
                _ = wal_deadline_interval.tick() => {
                    let now_us = wal_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                    let flush_t0 = Instant::now();
                    let dl_acks = wal
                        .poll_deadline_flush(now_us)
                        .map_err(|_| "Async 500us WAL poll_deadline_flush failed")?;
                    if !dl_acks.is_empty() {
                        let fl_us = flush_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                        max_wal_flush_us = max_wal_flush_us.max(fl_us);
                        total_wal_acks = total_wal_acks.saturating_add(dl_acks.len());
                        if let Some(enq_t0) = sub_threshold_enqueued_at.take() {
                            sub_threshold_auto_flush_us =
                                enq_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                        }
                    }
                }
            }
        }

        // Check if async background QUIC reconnect drill (`clients[0]`) completed this tick:
        if let Some(rx) = reconn_rx.as_mut()
            && let Ok(outcome) = rx.try_recv()
        {
            let comp = outcome?;
            reconn_rx = None;
            client0_reconnecting = false;
            reconnect_latency_us = comp.elapsed_us;
            if reconnect_latency_us > 1_000_000 {
                return Err("Async QUIC HMAC session recovery exceeded 1.0s ceiling");
            }
            clients[0].endpoint = comp.new_endpoint;
            clients[0].latest_ticket = comp.granted_ticket;
            clients[0].window.clear();
            for early in comp.early_streams {
                clients[0].apply_inbound_uni_stream(&early, Some(&mut replica_world_0))?;
            }
            // Also verify that attempting to send a live datagram with the revoked pre-reconnect `seq=1` ticket
            // over the new QUIC connection is strictly rejected by `SessionTicketManager` (`SessionReplayDetected`)!
            if let Some(old_revoked) = revoked_pre_reconnect_ticket.take() {
                let replay_dg = ClientInputDatagram::new(
                    1,
                    clients[0].ack_server_tick,
                    clients[0].ack_bitfield,
                    1,
                    [RawWireInput::default(); SLIDING_WINDOW_TICKS],
                    old_revoked,
                )
                .encode()
                .map_err(|_| "Failed encoding revoked ticket datagram")?;
                let _ = clients[0].endpoint.send_datagram(&replay_dg).await;
            }
        }

        // A. Spawn non-blocking live `< 1.0s` `HMAC` Session Disconnect & Reconnect Drill at `tick == 24`
        //    (after `clients[0]`'s `tick <= 15` inputs have been redundantly delivered, so `server_tick` loop NEVER stalls!):
        if server_tick == 24 {
            let saved_ticket = clients[0].latest_ticket;
            revoked_pre_reconnect_ticket = Some(saved_ticket);
            clients[0].endpoint.close();
            client0_reconnecting = true;

            let cli_tls_cl = Arc::clone(&cli_tls);
            let (tx, rx) = oneshot::channel();
            reconn_rx = Some(rx);
            tokio::spawn(async move {
                let started = Instant::now();
                let new_ep = match QuicClientEndpoint::connect(
                    server_addr,
                    cli_tls_cl,
                    0x9999_0001,
                    impairment,
                )
                .await
                {
                    Ok(ep) => ep,
                    Err(_) => {
                        let _ = tx.send(Err("Async reconnect QuicClientEndpoint handshake failed"));
                        return;
                    }
                };
                let mut reconn_frame = Vec::with_capacity(69);
                reconn_frame.push(STREAM_KIND_SESSION_RECONNECT);
                reconn_frame.extend_from_slice(&saved_ticket.encode());
                if new_ep.send_uni_stream(&reconn_frame).await.is_err()
                    || new_ep.send_uni_stream(&reconn_frame).await.is_err()
                {
                    let _ = tx.send(Err("Failed sending STREAM_KIND_SESSION_RECONNECT"));
                    return;
                }

                let mut early_streams = Vec::new();
                let mut granted_opt = None;
                let mut wait_steps: u32 = 0;
                while wait_steps < 250 {
                    tokio::time::sleep(Duration::from_millis(4)).await;
                    for (_sid, sbytes) in new_ep.try_recv_uni_streams().await {
                        if sbytes.first().copied() == Some(STREAM_KIND_SESSION_TICKET_GRANT) {
                            if let Some(body) = sbytes.get(1..)
                                && let Ok(tkt) = HmacSessionTicket::decode(body)
                            {
                                granted_opt = Some(tkt);
                            }
                        } else {
                            early_streams.push(sbytes);
                        }
                    }
                    if granted_opt.is_some() {
                        break;
                    }
                    wait_steps = wait_steps.saturating_add(1);
                }
                let Some(granted_ticket) = granted_opt else {
                    let _ = tx.send(Err(
                        "Timed out waiting for STREAM_KIND_SESSION_TICKET_GRANT",
                    ));
                    return;
                };
                let elapsed_us = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                let _ = tx.send(Ok(ReconnectCompletion {
                    new_endpoint: new_ep,
                    granted_ticket,
                    early_streams,
                    elapsed_us,
                }));
            });
        }

        // B. Connected clients poll inbound delta datagrams & streams, update client-reconciled `EntityDelta`
        //    positions, synthesize `tick + INPUT_LEAD_TICKS` input, and transmit `<= 1,100B` QUIC datagrams:
        let target_in_tick = server_tick.saturating_add(INPUT_LEAD_TICKS);
        for (idx, agent) in clients.iter_mut().enumerate() {
            if idx == 0 && client0_reconnecting {
                continue; // `clients[0]` socket is genuinely unplugged while background QUIC reconnect runs!
            }

            for dg in agent.endpoint.try_recv_datagrams().await {
                if let Ok(parsed) = ServerDeltaDatagram::decode(&dg) {
                    record_received_delta_chunk(
                        &mut agent.ack_server_tick,
                        &mut agent.ack_bitfield,
                        &mut agent.partial_delta_chunks,
                        parsed.server_tick,
                        parsed.chunk_index,
                        parsed.chunk_total,
                    );
                    if parsed.refreshed_ticket.reconnect_seq >= agent.latest_ticket.reconnect_seq {
                        agent.latest_ticket = parsed.refreshed_ticket;
                    }
                    for d in &parsed.deltas {
                        if d.entity == agent.entity && parsed.server_tick >= agent.reconciled_self_tick
                        {
                            let new_auth = QVec3::new(
                                Q32::from_raw(d.pos_x_raw),
                                Q32::from_raw(d.pos_y_raw),
                                Q32::from_raw(d.pos_z_raw),
                            );
                            // Inertialize authoritative snap into `spring_offset_pos` (`게임.md §8.1`):
                            agent.spring_offset_pos = agent.visual_self_pos.sat_sub(new_auth);
                            agent.reconciled_self_pos = new_auth;
                            agent.reconciled_self_tick = parsed.server_tick;
                        } else if d.entity == boss {
                            agent.reconciled_boss_pos = QVec3::new(
                                Q32::from_raw(d.pos_x_raw),
                                Q32::from_raw(d.pos_y_raw),
                                Q32::from_raw(d.pos_z_raw),
                            );
                        }
                    }
                    agent.decoded_deltas =
                        agent.decoded_deltas.saturating_add(parsed.deltas.len() as u64);
                }
            }

            // Step client-side critically-damped 2nd-order Spring-Damper visual root (`w = 12 rad/s`)
            // and verify client-side visual trajectory continuity (`0` visual teleports!):
            {
                const LOG2_E_Q32: Q32 = Q32::from_raw(6_196_328_019);
                let dt_cli = Q32::from_ratio_i32(1, 60);
                let omega = Q32::from_int(12);
                let omega_dt = omega.sat_mul(dt_cli).clamp_val(Q32::ZERO, Q32::from_int(8));
                let e = omega_dt
                    .sat_mul(LOG2_E_Q32)
                    .sat_neg()
                    .exp2_sat()
                    .clamp_val(Q32::ZERO, Q32::ONE);
                let prev_vis = agent.visual_self_pos;
                let x0 = agent.spring_offset_pos;
                let v0 = agent.spring_offset_vel;
                let j1 = v0.sat_add(x0.scale(omega));
                let x1 = x0.sat_add(j1.scale(dt_cli)).scale(e);
                let v1 = v0.sat_sub(j1.scale(omega_dt)).scale(e);
                agent.spring_offset_pos = x1;
                agent.spring_offset_vel = v1;
                agent.visual_self_pos = agent.reconciled_self_pos.sat_add(x1);
                if agent.visual_self_pos.dist_sq_wide(prev_vis) > max_step_sq {
                    teleport_anomalies = teleport_anomalies.saturating_add(1);
                }
            }

            for (_sid, stream_bytes) in agent.endpoint.try_recv_uni_streams().await {
                let rep_opt = if idx == 0 {
                    Some(&mut replica_world_0)
                } else if idx == 1 {
                    Some(&mut replica_world_1)
                } else {
                    None
                };
                agent.apply_inbound_uni_stream(&stream_bytes, rep_opt)?;
            }

            // For `clients[0]`, pause skill casts across the 8-tick pre-unplug tail (`18..=29`) so intentional
            // socket disconnect does not cut off in-flight window redundancy; all 49 continuous agents cast nonstop!
            let allow_skills = !(idx == 0 && (18..=29).contains(&target_in_tick));
            let raw = synthesize_raid_agent_raw_input(
                idx,
                target_in_tick,
                agent.reconciled_self_pos,
                agent.reconciled_boss_pos,
                allow_skills,
            );
            let skill_bits = (raw.action_mask as u16)
                & (actions::PRIMARY_ATTACK
                    | actions::ELEMENTAL_SKILL
                    | actions::DODGE_PHANTASIA
                    | actions::PARRY_CLASH);
            if (FIRST_SCHEDULED_SKILL_TICK..=LAST_SCHEDULED_SKILL_TICK).contains(&target_in_tick)
                && skill_bits != 0
            {
                scheduled_skills.insert((target_in_tick, agent.entity), skill_bits);
            }
            let wire = agent.push_and_encode_datagram(raw)?;
            let _ = agent.endpoint.send_datagram(&wire).await;
        }

        // C. Server drains inbound QUIC datagrams & streams, enforces `Packet<Unverified>::verify_bounds()`,
        //    processes wire-authentic `HMAC` session reconnects + catch-up SDF/Epoch streams, and enforces CID binding:
        let (inbound_dgs, inbound_streams) = quic_server.drain_all_inbound().await;
        for (cid, _sid, stream_bytes) in inbound_streams {
            if stream_bytes.first().copied() == Some(STREAM_KIND_SESSION_RECONNECT) {
                let Some(tkt_slice) = stream_bytes.get(1..) else {
                    tampered_rejected = tampered_rejected.saturating_add(1);
                    continue;
                };
                let Ok(tkt) = HmacSessionTicket::decode(tkt_slice) else {
                    tampered_rejected = tampered_rejected.saturating_add(1);
                    continue;
                };
                match ticket_mgr.accept_reconnect(&tkt, server_tick) {
                    Ok((recovered_entity, next_ticket)) => {
                        if recovered_entity != players[0] {
                            return Err("HMAC wire reconnect bound wrong EntityId");
                        }
                        account_to_cid.insert(tkt.account_id, cid);
                        entity_sync_by_account.insert(tkt.account_id, ClientEntitySyncState::default());

                        // Grant `seq + 1` ticket over reliable QUIC stream to the reconnected client:
                        let mut grant_frame = Vec::with_capacity(69);
                        grant_frame.push(STREAM_KIND_SESSION_TICKET_GRANT);
                        grant_frame.extend_from_slice(&next_ticket.encode());
                        let _ = quic_server.send_uni_stream_to(&cid, &grant_frame).await;

                        // Stream catch-up permanent SDF brick diffs accumulated while disconnected:
                        if !permanently_mutated_bricks.is_empty() {
                            let catchup_coords: Vec<BrickCoord> =
                                permanently_mutated_bricks.iter().copied().collect();
                            let catchup_pkts = SdfChunkDiffStreamPacket::extract_from_dirty_bricks(
                                &world,
                                &catchup_coords,
                                server_tick,
                            );
                            for (_ck, pkt) in catchup_pkts {
                                let encoded = pkt.encode_stream_frame();
                                if quic_server.send_uni_stream_to(&cid, &encoded).await.is_ok() {
                                    sdf_chunk_streams_sent =
                                        sdf_chunk_streams_sent.saturating_add(1);
                                }
                            }
                        }
                        wire_reconnect_completed = true;
                    }
                    Err(NetError::SessionReplayDetected) => {
                        tampered_rejected = tampered_rejected.saturating_add(1);
                        wire_replay_rejected = true;
                    }
                    Err(_) => {
                        tampered_rejected = tampered_rejected.saturating_add(1);
                    }
                }
            }
        }

        let mut earliest_rewind_tick: Option<u64> = None;
        let max_future_horizon = server_tick.saturating_add(SERVER_RINGBUFFER_TICKS as u64);

        for (cid, payload) in inbound_dgs {
            let Ok((acc_id, entity, ack_srv, ack_bits, frames, _tkt)) =
                ClientInputDatagram::decode_and_verify(&payload, &ticket_mgr, server_tick)
            else {
                tampered_rejected = tampered_rejected.saturating_add(1);
                continue;
            };

            // Enforce strict QUIC ConnectionId (`cid`) ownership: ONLY initial join (`Vacant`) or
            // authenticated `accept_reconnect` may bind `account_to_cid`!
            match account_to_cid.get(&acc_id).copied() {
                None => {
                    account_to_cid.insert(acc_id, cid);
                }
                Some(bound_cid) if bound_cid != cid => {
                    tampered_rejected = tampered_rejected.saturating_add(1);
                    continue;
                }
                Some(_) => {}
            }

            entity_sync_by_account
                .entry(acc_id)
                .or_default()
                .process_client_ack(ack_srv, ack_bits);

            let prev_ack = client_ack_by_account.get(&acc_id).copied().unwrap_or(0);
            if ack_srv > prev_ack {
                client_ack_by_account.insert(acc_id, ack_srv);
            }
            if let Some(newest) = frames.first()
                && newest.tick() <= max_future_horizon
            {
                last_client_tick_by_account.insert(acc_id, newest.tick());
            }

            for frame in frames {
                let ft = frame.tick();
                if ft > max_future_horizon {
                    tampered_rejected = tampered_rejected.saturating_add(1);
                    continue;
                }
                if ft < server_tick && server_tick.saturating_sub(ft) > MAX_REWIND_TICKS_60HZ {
                    continue;
                }
                let key = (ft, entity);
                if let std::collections::btree_map::Entry::Vacant(vac) =
                    authoritative_inputs.entry(key)
                {
                    vac.insert(frame);
                    let update_hold = last_movement_hold
                        .get(&entity)
                        .is_none_or(|&(prev_ft, _)| ft >= prev_ft);
                    if update_hold {
                        last_movement_hold.insert(entity, (ft, frame));
                    }
                    if ft < server_tick {
                        let _ = ring64.set_recorded_input(ft, entity, frame);
                        earliest_rewind_tick = Some(match earliest_rewind_tick {
                            Some(cur_min) => cur_min.min(ft),
                            None => ft,
                        });
                    }
                }
            }
        }

        // D. Authoritative Server Rewind (`ServerSoARingBuffer64` + `SparseSdfWorld::rewind_to_tick`)
        //    recovering late packets dropped by the `5%` UDP loss so `0` skills evaporate!
        if let Some(rewind_start) = earliest_rewind_tick
            && ring64.restore_for_rewind(
                server_tick,
                rewind_start,
                &mut arena,
                &mut agent_zones,
                &mut entity_channels,
            )
        {
            let _ = pending_sdf_by_tick.split_off(&rewind_start);
            world.rewind_to_tick(rewind_start);
            let _ = world.drain_dirty_bricks();

            let mut replay_prev_pos: BTreeMap<EntityId, QVec3> = BTreeMap::new();
            for &pid in &players {
                if let Some(rp) = arena.position(pid) {
                    replay_prev_pos.insert(pid, rp);
                }
            }

            let mut replay_t = rewind_start;
            while replay_t < server_tick {
                let mut replay_map: BTreeMap<EntityId, InputFrame> = BTreeMap::new();
                for &pid in &players {
                    if let Some(&f) = authoritative_inputs.get(&(replay_t, pid)) {
                        replay_map.insert(pid, f);
                    } else if let Some(rf) = ring64.recorded_input(replay_t, pid) {
                        replay_map.insert(pid, rf);
                    }
                }
                let pinned_bundle_ref = raid_lease.bundle();
                let mut ctx = StepTickContext {
                    world: &mut world,
                    arena: &mut arena,
                    aoi_grid: &mut aoi_grid,
                    cmd_buf: &mut cmd_buf,
                    weapon_pool: &weapon_pool,
                    trait_qvm: &trait_qvm,
                    augment_qvm: &augment_qvm,
                    world_event_qvm: world_event_qvm.as_ref(),
                    resonance_burst_qvm: &resonance_burst_qvm,
                    boss_qvm: &boss_qvm,
                    players: &players,
                    boss,
                    portal_gate: &portal_gate,
                    agent_zones: &mut agent_zones,
                    entity_channels: &mut entity_channels,
                    pinned_bundle: pinned_bundle_ref,
                };
                let rep_metrics = execute_authoritative_tick_step(
                    replay_t,
                    &replay_map,
                    default_hold_frame,
                    true,
                    &mut ctx,
                )?;
                max_shard_compute_us = max_shard_compute_us.max(rep_metrics.max_shard_us);
                max_shard_fuel = max_shard_fuel.max(rep_metrics.max_shard_fuel);
                max_call_fuel = max_call_fuel.max(rep_metrics.max_call_fuel);
                ghost_mirrors_peak = ghost_mirrors_peak.max(rep_metrics.ghost_count);
                elastic_channel_splits =
                    elastic_channel_splits.saturating_add(rep_metrics.elastic_splits);
                portal_prewarms_total =
                    portal_prewarms_total.saturating_add(rep_metrics.portal_prewarms);

                // Verify per-tick trajectory continuity inside replay (`<= 3.0m` per tick):
                for &pid in &players {
                    if let Some(cur_rp) = arena.position(pid) {
                        if let Some(&old_rp) = replay_prev_pos.get(&pid)
                            && cur_rp.dist_sq_wide(old_rp) > max_step_sq
                        {
                            teleport_anomalies = teleport_anomalies.saturating_add(1);
                        }
                        replay_prev_pos.insert(pid, cur_rp);
                    }
                }

                let replay_dirty = world.drain_dirty_bricks();
                if !replay_dirty.is_empty() {
                    pending_sdf_by_tick.insert(replay_t, replay_dirty);
                }
                ring64.record_inputs_for_tick(replay_t, &replay_map);
                let next_state_tag = replay_t.saturating_add(1);
                ring64.record_state_at_tick(
                    next_state_tag,
                    &arena,
                    &agent_zones,
                    &entity_channels,
                );
                replay_t = next_state_tag;
            }
            server_rewinds = server_rewinds.saturating_add(1);
            // Note: `prev_positions` is intentionally NOT overwritten here so the forward step below
            // ALSO verifies reconciliation continuity (`pos_unreconciled(t-1) -> pos_reconciled(t) <= 3.0m`)!
        }

        // E. Mid-Combat `E_0 -> E_0.1 -> E_0` Infrastructure Epoch Streaming, Pinning, WAL Sub-Threshold
        //    Micro-Batch Deadline Test & Rollback Drill (`§5.3, §9.1, §9.2`):
        if server_tick == 60 {
            let e01_wire = e01_bundle
                .encode_stream_tlv_v2(b"E_0.1_live_hotbalance")
                .map_err(|_| "Failed encoding E_0.1 V2 TLV stream")?;
            let decoded_e01 = CanonicalEpochBundle::decode_stream_tlv(&e01_wire, false)
                .map_err(|_| "Server failed staging E_0.1 TLV")?;
            epoch_table
                .mount_staged_epoch(decoded_e01)
                .map_err(|_| "Server mount_staged_epoch(E_0.1) failed")?;
            // Prove mounting a 3rd epoch while `draining_standby` (`E_0`) still holds `raid_lease` is rejected!
            if epoch_table.mount_staged_epoch(e0_bundle.clone()) != Err(NetError::EpochStandbyBusy)
            {
                return Err("DualSlotEpochTable failed to block overwriting busy draining_standby");
            }
            for cid in account_to_cid.values() {
                let _ = quic_server.send_uni_stream_to(cid, &e01_wire).await;
            }
            let probe_new_encounter = epoch_table.pin_for_new_encounter();
            if probe_new_encounter.bundle().full_merkle_root != e01_merkle {
                return Err("New encounter after mount_staged_epoch did not receive E_0.1");
            }
            if epoch_table.try_reclaim_zero_ref_standby().is_some() {
                return Err("Standby E_0 prematurely reclaimed while active raid was still pinned!");
            }
        }
        if (60..110).contains(&server_tick) && raid_lease.bundle().full_merkle_root != e0_merkle {
            mid_combat_epoch_drift = mid_combat_epoch_drift.saturating_add(1);
        }

        // At `tick == 80`, submit a 3-op sub-threshold micro-batch (`3 < 16` threshold) WITHOUT calling
        // `flush_group_commit()`, proving the `500us` `tokio::select!` ticker flushes it within `<= 2.0ms`!
        if server_tick == 80 {
            let now_us = wal_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            sub_threshold_enqueued_at = Some(Instant::now());
            for m_i in 0_u64..3_u64 {
                let (_, immediate_acks) = wal
                    .submit_tx(
                        LedgerTxKind::FaucetMint {
                            shard_id: (m_i & 3) as u16,
                            to: AccountId(m_i.saturating_add(1)),
                            asset: AssetId(1),
                            amount: 10,
                        },
                        now_us,
                    )
                    .map_err(|_| "Sub-threshold WAL FaucetMint failed")?;
                if !immediate_acks.is_empty() {
                    return Err("3-op micro-batch flushed synchronously before 16-op threshold");
                }
            }
        }
        if server_tick == 83 && (sub_threshold_enqueued_at.is_some() || sub_threshold_auto_flush_us == 0)
        {
            return Err("Async 500us WAL deadline ticker failed to flush sub-threshold batch before next ticks");
        }

        if server_tick == 110 {
            // Encounter Gate Boundary #1 (Boss Phase 1 -> Phase 2): atomic drain `E_0 -> E_0.1`!
            raid_lease.drain_at_boundary(&epoch_table);
            if raid_lease.bundle().full_merkle_root != e01_merkle {
                return Err("Raid lease failed to drain to E_0.1 at Gate Boundary #1");
            }
            for agent in &mut clients {
                agent.client_lease.drain_at_boundary(&agent.client_epochs);
                let _ = agent.client_epochs.try_reclaim_zero_ref_standby();
            }
            if epoch_table.try_reclaim_zero_ref_standby() != Some(0) {
                return Err("Standby E_0 failed zero-ref async reclamation after boundary drain");
            }

            let now_us = wal_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            let _ = wal
                .submit_op(
                    WalOp::ConfigureAssetCap {
                        asset: AssetId(1),
                        epoch_cap: upgraded_cap_e01,
                    },
                    now_us,
                )
                .map_err(|_| "WAL ConfigureAssetCap E_0.1 failed")?;

            let v2_extra_tag = vec![TlvField {
                tag: 901,
                value: b"e01_raid_gate1_bonus".to_vec(),
            }];
            for i in 0..RAID_AGENT_COUNT {
                let acc = AccountId((i as u64).saturating_add(1));
                let shard = (i & 3) as u16;
                let fl_t = Instant::now();
                let (_, acks) = wal
                    .submit_op_with_unknown_fields(
                        WalOp::ApplyTx(LedgerTxKind::FaucetMint {
                            shard_id: shard,
                            to: acc,
                            asset: AssetId(1),
                            amount: 500,
                        }),
                        v2_extra_tag.clone(),
                        now_us,
                    )
                    .map_err(|_| "WAL FaucetMint failed")?;
                if !acks.is_empty() {
                    let b_us = fl_t.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                    max_wal_flush_us = max_wal_flush_us.max(b_us);
                }
                total_wal_acks = total_wal_acks.saturating_add(acks.len());
            }
            let _ = wal
                .submit_tx(
                    LedgerTxKind::Transfer {
                        from: AccountId(1),
                        to: AccountId(2),
                        asset: AssetId(1),
                        amount: 120,
                    },
                    now_us,
                )
                .map_err(|_| "WAL Transfer failed")?;
            let _ = wal
                .submit_tx(
                    LedgerTxKind::SinkBurn {
                        from: AccountId(2),
                        asset: AssetId(1),
                        amount: 50,
                    },
                    now_us,
                )
                .map_err(|_| "WAL SinkBurn failed")?;
            let _ = wal
                .submit_op(
                    WalOp::GrantSeasonalPower {
                        account: AccountId(1),
                        delta: 350,
                    },
                    now_us,
                )
                .map_err(|_| "WAL GrantSeasonalPower failed")?;
            e01_legacy_item_id = run_generation.saturating_mul(100_000).saturating_add(1);
            let _ = wal
                .submit_tx(
                    LedgerTxKind::GrantLegacy {
                        account: AccountId(1),
                        legacy_item_id: e01_legacy_item_id,
                    },
                    now_us,
                )
                .map_err(|_| "WAL GrantLegacy failed")?;

            let fl_t0 = Instant::now();
            let b_acks = wal
                .flush_group_commit()
                .map_err(|_| "WAL Gate 1 flush failed")?;
            let fl_us = fl_t0.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
            max_wal_flush_us = max_wal_flush_us.max(fl_us);
            total_wal_acks = total_wal_acks.saturating_add(b_acks.len());
        }

        if server_tick == 128 {
            // Stage rollback `E_0.1 -> E_0` mid-combat during Boss Phase 2:
            let e0_wire = e0_bundle
                .encode_stream_tlv_v2(b"E_0_rollback")
                .map_err(|_| "Failed encoding E_0 rollback TLV")?;
            let decoded_e0 = CanonicalEpochBundle::decode_stream_tlv(&e0_wire, true)
                .map_err(|_| "Failed decoding E_0 rollback TLV with V1 decoder")?;
            epoch_table
                .mount_staged_epoch(decoded_e0)
                .map_err(|_| "Server mount_staged_epoch(E_0 rollback) failed")?;
            for cid in account_to_cid.values() {
                let _ = quic_server.send_uni_stream_to(cid, &e0_wire).await;
            }
            if epoch_table.try_reclaim_zero_ref_standby().is_some() {
                return Err("Standby E_0.1 prematurely reclaimed mid-combat before Gate #2!");
            }
        }
        if (128..145).contains(&server_tick) && raid_lease.bundle().full_merkle_root != e01_merkle {
            mid_combat_epoch_drift = mid_combat_epoch_drift.saturating_add(1);
        }

        if server_tick == 145 {
            // Encounter Gate Boundary #2: complete `E_0.1 -> E_0` rollback drain & verify lossless schema compatibility!
            raid_lease.drain_at_boundary(&epoch_table);
            if raid_lease.bundle().full_merkle_root != e0_merkle {
                return Err("Raid lease failed to drain back to E_0 at Gate Boundary #2");
            }
            for agent in &mut clients {
                agent.client_lease.drain_at_boundary(&agent.client_epochs);
                let _ = agent.client_epochs.try_reclaim_zero_ref_standby();
            }
            if epoch_table.try_reclaim_zero_ref_standby() != Some(1) {
                return Err("Standby E_0.1 failed zero-ref async reclamation after rollback");
            }
            let e01_v2_wire = e01_bundle
                .encode_stream_tlv_v2(b"additive_v2_payload")
                .map_err(|_| "encode_stream_tlv_v2 failed")?;
            let v1_roundtrip = CanonicalEpochBundle::decode_stream_tlv(&e01_v2_wire, true)
                .map_err(|_| "V1 decoder rejected E_0.1 V2 TLV stream")?;
            if v1_roundtrip.preserved_unknown_inner.len() < 2
                || v1_roundtrip.preserved_unknown_outer.is_empty()
            {
                return Err(
                    "V1 decoder lost unknown additive V2 TLV fields during E_0.1 -> E_0 rollback",
                );
            }
        }

        // F. Execute Current Authoritative `60Hz` Server Tick:
        let mut cur_tick_inputs: BTreeMap<EntityId, InputFrame> = BTreeMap::new();
        for &pid in &players {
            if let Some(&f) = authoritative_inputs.get(&(server_tick, pid)) {
                cur_tick_inputs.insert(pid, f);
            } else if let Some(&(_, last_f)) = last_movement_hold.get(&pid)
                && let Some(hold_f) = build_movement_hold_frame(last_f, server_tick)
            {
                cur_tick_inputs.insert(pid, hold_f);
            }
        }
        let step_metrics = {
            let pinned_bundle_ref = raid_lease.bundle();
            let mut ctx = StepTickContext {
                world: &mut world,
                arena: &mut arena,
                aoi_grid: &mut aoi_grid,
                cmd_buf: &mut cmd_buf,
                weapon_pool: &weapon_pool,
                    trait_qvm: &trait_qvm,
                    augment_qvm: &augment_qvm,
                    world_event_qvm: world_event_qvm.as_ref(),
                resonance_burst_qvm: &resonance_burst_qvm,
                boss_qvm: &boss_qvm,
                players: &players,
                boss,
                portal_gate: &portal_gate,
                agent_zones: &mut agent_zones,
                entity_channels: &mut entity_channels,
                pinned_bundle: pinned_bundle_ref,
            };
            execute_authoritative_tick_step(
                server_tick,
                &cur_tick_inputs,
                default_hold_frame,
                false,
                &mut ctx,
            )?
        };
        if step_metrics.max_shard_us > AOI_SHARD_BUDGET_MICROS {
            return Err("AOI shard compute exceeded 4.00ms (4,000us) server tick budget");
        }
        if (70..105).contains(&server_tick) {
            observed_e0_speed_cap = step_metrics.applied_speed_cap_raw;
        } else if (115..140).contains(&server_tick) {
            observed_e01_speed_cap = step_metrics.applied_speed_cap_raw;
        }

        max_shard_compute_us = max_shard_compute_us.max(step_metrics.max_shard_us);
        max_shard_fuel = max_shard_fuel.max(step_metrics.max_shard_fuel);
        max_call_fuel = max_call_fuel.max(step_metrics.max_call_fuel);
        ghost_mirrors_peak = ghost_mirrors_peak.max(step_metrics.ghost_count);
        elastic_channel_splits = elastic_channel_splits.saturating_add(step_metrics.elastic_splits);
        portal_prewarms_total = portal_prewarms_total.saturating_add(step_metrics.portal_prewarms);
        weapon_pool_mask_total |= step_metrics.weapon_pool_mask;
        trait_casts_total = trait_casts_total.saturating_add(step_metrics.trait_casts);
        augment_casts_total = augment_casts_total.saturating_add(step_metrics.augment_casts);
        world_event_casts_total =
            world_event_casts_total.saturating_add(step_metrics.world_event_casts);

        // Verify zero non-physical position jumps (`순간이동 0건`, including across rewind reconciliation!):
        for &pid in &players {
            if let Some(cur_p) = arena.position(pid) {
                if let Some(&prev_p) = prev_positions.get(&pid)
                    && cur_p.dist_sq_wide(prev_p) > max_step_sq
                {
                    teleport_anomalies = teleport_anomalies.saturating_add(1);
                }
                prev_positions.insert(pid, cur_p);
            }
        }

        let next_state_tag = server_tick.saturating_add(1);
        ring64.record_inputs_for_tick(server_tick, &cur_tick_inputs);
        ring64.record_state_at_tick(next_state_tag, &arena, &agent_zones, &entity_channels);

        // G. Queue dirty SDF bricks inside the `<= 150ms` (`9`-tick) rewind horizon, and once permanently
        //    finalized (`mut_tick + MAX_REWIND_TICKS_60HZ <= server_tick`), stream per-chunk `(cx, cz)`
        //    diffs over independent unidirectional reliable QUIC streams to all connected clients (`§8.2, §9.1`):
        let newly_dirty = world.drain_dirty_bricks();
        if !newly_dirty.is_empty() {
            pending_sdf_by_tick
                .entry(server_tick)
                .or_default()
                .extend(newly_dirty);
        }
        let mut finalized_sdf_ticks: Vec<u64> = Vec::new();
        for &m_tick in pending_sdf_by_tick.keys() {
            if m_tick.saturating_add(MAX_REWIND_TICKS_60HZ) <= server_tick {
                finalized_sdf_ticks.push(m_tick);
            } else {
                break;
            }
        }
        if !finalized_sdf_ticks.is_empty() {
            let mut unique_bricks: BTreeSet<BrickCoord> = BTreeSet::new();
            for m_tick in finalized_sdf_ticks {
                if let Some(coords) = pending_sdf_by_tick.remove(&m_tick) {
                    for bc in coords {
                        unique_bricks.insert(bc);
                        permanently_mutated_bricks.insert(bc);
                    }
                }
            }
            let finalized_vec: Vec<BrickCoord> = unique_bricks.into_iter().collect();
            let chunk_packets = SdfChunkDiffStreamPacket::extract_from_dirty_bricks(
                &world,
                &finalized_vec,
                server_tick,
            );
            for (_chunk_key, pkt) in chunk_packets {
                let encoded_stream = pkt.encode_stream_frame();
                for cid in account_to_cid.values() {
                    if quic_server
                        .send_uni_stream_to(cid, &encoded_stream)
                        .await
                        .is_ok()
                    {
                        sdf_chunk_streams_sent = sdf_chunk_streams_sent.saturating_add(1);
                    }
                }
            }
        }

        // Prune `authoritative_inputs` strictly older than the 64-tick ringbuffer (`O(1)` bounded memory):
        if let Some(prune_cutoff) = server_tick.checked_sub(SERVER_RINGBUFFER_TICKS as u64) {
            for &pid in &players {
                if let Some(expected_bits) = scheduled_skills.remove(&(prune_cutoff, pid)) {
                    match authoritative_inputs.get(&(prune_cutoff, pid)) {
                        Some(committed_frame)
                            if (committed_frame.action_mask() & expected_bits) == expected_bits =>
                        {
                            committed_skills = committed_skills.saturating_add(1);
                        }
                        _ => {
                            evaporated_skills = evaporated_skills.saturating_add(1);
                        }
                    }
                }
                let _ = authoritative_inputs.remove(&(prune_cutoff, pid));
            }
        }

        // H. Broadcast per-entity ACK-confirmed priority `ServerDeltaDatagram` chunks (`<= 1,100B`, `§9.1`):
        let pinned_merkle = raid_lease.bundle().full_merkle_root;
        for (idx, &pid) in players.iter().enumerate() {
            let acc_id = (idx as u64).saturating_add(1);
            let Some(&cid) = account_to_cid.get(&acc_id) else {
                continue;
            };
            let p_pos = arena.position(pid).unwrap_or(QVec3::ZERO);
            let p_ch = entity_channels.get(pid.0 as usize).copied().unwrap_or(0);
            let p_shard = ShardCoord::from_qvec3(p_ch, p_pos);
            let visible = aoi_grid.visible_in_shard(p_shard);
            let cli_ack_srv = client_ack_by_account.get(&acc_id).copied().unwrap_or(0);
            let cur_reconn_seq = ticket_mgr.active_seq_for(acc_id);
            let refreshed_tkt = ticket_mgr
                .issue_ticket(acc_id, pid, server_tick, cur_reconn_seq)
                .map_err(|_| "Failed refreshing per-tick HMAC session ticket")?;
            let params = PriorityDeltaParams {
                server_tick: next_state_tag,
                ack_client_tick: last_client_tick_by_account
                    .get(&acc_id)
                    .copied()
                    .unwrap_or(0),
                client_ack_server_tick: cli_ack_srv,
                self_entity: pid,
                pinned_epoch_merkle: pinned_merkle,
                refreshed_ticket: refreshed_tkt,
                max_chunks_per_tick: 1,
            };
            let sync_state = entity_sync_by_account.entry(acc_id).or_default();
            let chunks = build_ack_priority_delta_chunks(
                &params,
                &visible,
                &arena,
                &ring64,
                sync_state,
            )
            .map_err(|_| "build_ack_priority_delta_chunks failed")?;
            for ch in chunks {
                let wire = ch
                    .encode()
                    .map_err(|_| "ServerDeltaDatagram encode failed")?;
                if wire.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
                    return Err("ServerDeltaDatagram exceeded 1,100B cap");
                }
                let _ = quic_server.send_datagram_to(&cid, &wire).await;
            }
        }

        server_tick = server_tick.saturating_add(1);
    }

    // Allow final in-flight reliable QUIC SDF & Epoch streams (`80ms` RTT + PTO) to settle across all 50 clients:
    for _ in 0..24 {
        tokio::time::sleep(Duration::from_millis(12)).await;
        for (idx, agent) in clients.iter_mut().enumerate() {
            for (_sid, stream_bytes) in agent.endpoint.try_recv_uni_streams().await {
                let rep_opt = if idx == 0 {
                    Some(&mut replica_world_0)
                } else if idx == 1 {
                    Some(&mut replica_world_1)
                } else {
                    None
                };
                agent.apply_inbound_uni_stream(&stream_bytes, rep_opt)?;
            }
        }
    }

    // Verify out-of-window rewind (`age = 12 > MAX_REWIND_TICKS_60HZ = 9`) is strictly rejected (`<= 150ms` bound):
    let mut probe_arena = CombatArena::new_boxed(1);
    let mut probe_zones = Vec::new();
    let mut probe_channels = Vec::new();
    if ring64.restore_for_rewind(
        RAID_TOTAL_TICKS_60HZ,
        RAID_TOTAL_TICKS_60HZ.saturating_sub(12),
        &mut probe_arena,
        &mut probe_zones,
        &mut probe_channels,
    ) {
        return Err("ServerSoARingBuffer64 permitted rewind beyond 150ms (9-tick) limit!");
    }

    // Verify ZERO skill evaporation across remaining unpruned scheduled ticks (`§4.2`):
    for (&(sched_tick, entity), &expected_bits) in &scheduled_skills {
        match authoritative_inputs.get(&(sched_tick, entity)) {
            Some(committed_frame)
                if (committed_frame.action_mask() & expected_bits) == expected_bits =>
            {
                committed_skills = committed_skills.saturating_add(1);
            }
            _ => {
                evaporated_skills = evaporated_skills.saturating_add(1);
            }
        }
    }
    if authoritative_inputs.len()
        > (SERVER_RINGBUFFER_TICKS.saturating_add(2)).saturating_mul(RAID_AGENT_COUNT)
    {
        return Err("authoritative_inputs violated O(1) ringbuffer horizon memory bound");
    }

    // Verify bit-exact 3D `SparseSdfWorld` state_hash convergence across server, reconnected `client[0]`, and continuous `client[1]`!
    let server_sdf_hash = world.state_hash();
    if replica_world_0.state_hash() != server_sdf_hash
        || replica_world_1.state_hash() != server_sdf_hash
    {
        return Err("Client SparseSdfWorld replica state_hash diverged from authoritative server world");
    }

    // Verify encounter-pinned `CanonicalEpochBundle` genuinely governed authoritative gameplay physics (`E_0.1` > `E_0`):
    if observed_e01_speed_cap <= observed_e0_speed_cap {
        return Err("Draining raid_lease to E_0.1 did not update authoritative movement physics");
    }

    // Verify durable checkpoint + `E_0.1 -> E_0` WAL recovery preserves 100% of balances & `Legacy` grants:
    wal.write_checkpoint()
        .map_err(|_| "WAL write_checkpoint failed")?;
    drop(wal);
    let (recovered_wal, _rec_rep) = GroupCommitWal::open_and_recover(wal_dir, 16)
        .map_err(|_| "Post-rollback WAL open_and_recover failed")?;
    recovered_wal
        .committed_ledger()
        .verify_global_conservation()
        .map_err(|_| "Post-rollback LedgerCore conservation check failed")?;
    if !recovered_wal
        .committed_ledger()
        .has_legacy_item(AccountId(1), e01_legacy_item_id)
    {
        return Err("E_0.1 -> E_0 rollback lost granted Legacy emblem");
    }
    let scoped_legacy: Option<PlaneScoped<LegacyPlane, u64>> = recovered_wal
        .committed_ledger()
        .export_legacy_scoped(AccountId(1), e01_legacy_item_id);
    if scoped_legacy.is_none()
        || recovered_wal
            .committed_ledger()
            .balance(AccountId(1), AssetId(1))
            < 380
    {
        return Err("Post-rollback E_0.1 reward balance or PlaneScoped<LegacyPlane> missing");
    }
    let legacy_preserved_count = recovered_wal.committed_ledger().legacy_count();

    // Sum real UDP impairment counters across server + 50 clients:
    let mut udp_dropped = quic_server
        .counters
        .datagrams_dropped_impairment
        .load(std::sync::atomic::Ordering::Relaxed);
    let mut udp_delivered = quic_server
        .counters
        .datagrams_delivered_udp
        .load(std::sync::atomic::Ordering::Relaxed);
    let all_clients_synced_streams = clients
        .iter()
        .all(|c| c.received_sdf_streams > 0 && c.received_epoch_streams >= 2);
    for c in &clients {
        udp_dropped = udp_dropped.saturating_add(
            c.endpoint
                .counters
                .datagrams_dropped_impairment
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        udp_delivered = udp_delivered.saturating_add(
            c.endpoint
                .counters
                .datagrams_delivered_udp
                .load(std::sync::atomic::Ordering::Relaxed),
        );
        c.endpoint.close();
    }
    quic_server.close();

    let portal_transfers_total = agent_zones
        .iter()
        .filter(|&&z| z == ZoneInstanceKind::RaidBossInstance)
        .count() as u32;
    let sdf_bricks_converged = replica_world_1.drain_dirty_bricks().len();
    let reclaimed_epochs = epoch_table.reclaimed_history();
    let active_channels_end = active_channel_set_from_slice(&players, &entity_channels);

    // Strict final gate assertions on all `Phase 2` invariants (`게임.md §4.1, §4.2, §5.2, §5.3, §9.1, §9.2, §11.3`):
    if udp_dropped == 0 || udp_delivered == 0 {
        return Err("5% UDP packet loss impairment was not active on live QUIC sockets");
    }
    if committed_skills == 0 || evaporated_skills != 0 {
        return Err("Skill evaporation detected under 5% loss / 80ms RTT");
    }
    if teleport_anomalies != 0 {
        return Err("Position teleportation anomaly detected during 50-agent raid");
    }
    if tampered_rejected < 6
        || !wire_reconnect_completed
        || !wire_replay_rejected
        || reconnect_latency_us == 0
    {
        return Err("Packet boundary / future-tick DoS / wire HMAC replay rejection incomplete");
    }
    if portal_prewarms_total == 0 || portal_transfers_total < (RAID_AGENT_COUNT as u32) {
        return Err("Seamless portal pre-warming or 50-agent zero-loading transfer incomplete");
    }
    if ghost_mirrors_peak == 0 || elastic_channel_splits == 0 || active_channels_end.len() < 2 {
        return Err("AOI 8-neighbor ghost mirroring or live elastic channel split not exercised");
    }
    if season_pack_live
        && (weapon_pool_mask_total.count_ones() < (weapon_pool.len().min(32) as u32)
            || trait_casts_total == 0
            || augment_casts_total == 0
            || world_event_casts_total == 0)
    {
        return Err("Live raid did not exercise full 9-weapon season loadouts, resonance traits, gate augments, and world events");
    }
    if sdf_chunk_streams_sent == 0 || sdf_bricks_converged == 0 || !all_clients_synced_streams {
        return Err(
            "Per-chunk reliable QUIC SDF & Epoch streaming did not converge on all 50 clients",
        );
    }
    if mid_combat_epoch_drift != 0 || reclaimed_epochs.as_slice() != [0, 1] {
        return Err("Mid-combat epoch pinning or zero-ref standby reclamation violated");
    }
    if max_wal_flush_us > MAX_BATCH_LATENCY_MICROS.saturating_mul(25) || total_wal_acks == 0 {
        return Err("GroupCommitWal failed to emit fsync-first CommittedAcks");
    }

    Ok(RaidTelemetry {
        udp_dropped,
        udp_delivered,
        server_rewinds,
        evaporated_skills,
        committed_skills,
        teleport_anomalies,
        tampered_rejected,
        portal_transfers: portal_transfers_total,
        portal_prewarms: portal_prewarms_total,
        ghost_mirrors_peak,
        elastic_channel_splits,
        max_shard_compute_us,
        max_shard_fuel,
        max_call_fuel,
        reconnect_latency_us,
        sdf_chunk_streams_sent,
        sdf_bricks_converged,
        wal_committed_acks: total_wal_acks,
        max_wal_flush_us,
        mid_combat_epoch_drift,
        reclaimed_epochs,
        legacy_preserved_count,
        season_weapons_used: weapon_pool_mask_total.count_ones(),
        trait_casts: trait_casts_total,
        augment_casts: augment_casts_total,
        world_event_casts: world_event_casts_total,
    })
}

struct Phase3SeasonTelemetry {
    total_qvm_modules: usize,
    e1_weapons: usize,
    e2_weapons: usize,
    e1_chunks: usize,
    e2_chunks: usize,
    e1_spread_half: u32,
    e2_spread_half: u32,
    prism_damage_delta: i64,
    prism_augment_delta: i64,
    bosses_verified: usize,
    s2_post_rollover_power: i64,
    s2_preserved_legacy: usize,
}

fn wl_phi_exact(graph: &CausalGraphIR) -> BTreeMap<u64, i64> {
    let mut phi: BTreeMap<u64, i64> = BTreeMap::new();
    let n = graph.nodes.len();
    if n == 0 {
        return phi;
    }
    let mut colors: Vec<u64> = graph
        .nodes
        .iter()
        .map(|&role| {
            let d = blake3_digest(&[0_u8, role as u8]);
            u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
        })
        .collect();
    for &c in &colors {
        let prev = phi.get(&c).copied().unwrap_or(0);
        phi.insert(c, prev.saturating_add(1));
    }
    let mut iter: u8 = 1;
    while iter <= 3 {
        let mut next_colors = vec![0_u64; n];
        let mut u: usize = 0;
        while u < n {
            let mut in_sig: Vec<(u8, u64)> = Vec::new();
            let mut out_sig: Vec<(u8, u64)> = Vec::new();
            for edge in &graph.causal_edges {
                let s = edge.src as usize;
                let d = edge.dst as usize;
                if s < n && d < n {
                    if d == u {
                        in_sig.push((edge.edge_kind, colors[s]));
                    }
                    if s == u {
                        out_sig.push((edge.edge_kind, colors[d]));
                    }
                }
            }
            in_sig.sort_unstable();
            out_sig.sort_unstable();
            let mut bytes = Vec::with_capacity(32);
            bytes.push(iter);
            bytes.extend_from_slice(&colors[u].to_le_bytes());
            bytes.extend_from_slice(&(in_sig.len() as u32).to_le_bytes());
            for (ek, nc) in in_sig {
                bytes.push(ek);
                bytes.extend_from_slice(&nc.to_le_bytes());
            }
            bytes.extend_from_slice(&(out_sig.len() as u32).to_le_bytes());
            for (ek, nc) in out_sig {
                bytes.push(ek);
                bytes.extend_from_slice(&nc.to_le_bytes());
            }
            let dig = blake3_digest(&bytes);
            next_colors[u] = u64::from_le_bytes([
                dig[0], dig[1], dig[2], dig[3], dig[4], dig[5], dig[6], dig[7],
            ]);
            u = u.saturating_add(1);
        }
        colors = next_colors;
        for &c in &colors {
            let prev = phi.get(&c).copied().unwrap_or(0);
            phi.insert(c, prev.saturating_add(1));
        }
        iter = iter.saturating_add(1);
    }
    phi
}

/// Exact integer check `1 - dot / sqrt(||x||^2 * ||y||^2) >= num/den` without floating-point math.
fn wl_distance_ge_ratio_exact(
    phi_x: &BTreeMap<u64, i64>,
    phi_y: &BTreeMap<u64, i64>,
    num: i128,
    den: i128,
) -> bool {
    if phi_x == phi_y {
        return false;
    }
    let mut dot: i128 = 0;
    let mut nx2: i128 = 0;
    let mut ny2: i128 = 0;
    for (&k, &vx) in phi_x {
        let vx_w = i128::from(vx);
        nx2 = nx2.saturating_add(vx_w.saturating_mul(vx_w));
        if let Some(&vy) = phi_y.get(&k) {
            dot = dot.saturating_add(vx_w.saturating_mul(i128::from(vy)));
        }
    }
    for &vy in phi_y.values() {
        let vy_w = i128::from(vy);
        ny2 = ny2.saturating_add(vy_w.saturating_mul(vy_w));
    }
    if dot <= 0 {
        return true;
    }
    // `1 - dot / sqrt(nx2 * ny2) >= num/den  <=>  den * dot <= (den - num) * sqrt(nx2 * ny2)`
    let keep = den.saturating_sub(num);
    let lhs = den
        .saturating_mul(dot)
        .saturating_mul(den.saturating_mul(dot));
    let rhs = keep
        .saturating_mul(keep)
        .saturating_mul(nx2)
        .saturating_mul(ny2);
    lhs <= rhs
}

/// Runs a deterministic 9-archetype round-robin duel tournament (`20` trials per pair, `60` ticks per trial),
/// verifying `dealt_damage` (`100%`), non-transitive counter (`>= 55%` = `22/40` half-points), and spread (`<= 10%p` = `36/360`).
fn verify_season_weapon_tournament(weapons: &[CompiledQvmModule]) -> Result<u32, &'static str> {
    let m = weapons.len();
    if m < 9 {
        return Err("Season requires >= 9 distinct weapon modules");
    }
    let dt = Q32::from_ratio_i32(1, 60);
    let mut half_pts = vec![vec![20_u32; m]; m];
    let mut dealt = vec![false; m];

    let mut i: usize = 0;
    while i < m {
        let mut j = i.saturating_add(1);
        while j < m {
            let rel = j.saturating_sub(i);
            let mut score_i_half: u32 = 0;
            let mut tr: u32 = 0;
            while tr < 20 {
                let seed = ((i as u64).saturating_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
                    ^ ((j as u64).saturating_add(1).wrapping_mul(0xBF58_476D_1CE4_E5B9))
                    ^ u64::from(tr.saturating_add(1));
                let mut rng = DetRng::new(seed);
                let sh_a = Q32::from_int(
                    220_i32.saturating_add((rng.next_u64().checked_rem(161).unwrap_or(0)) as i32),
                );
                let sh_b = Q32::from_int(
                    220_i32.saturating_add((rng.next_u64().checked_rem(161).unwrap_or(0)) as i32),
                );
                let sep_x = Q32::from_int(2_i32.saturating_add((rng.next_u64() & 1) as i32));

                let mut arena = CombatArena::new_boxed(seed);
                let a = arena
                    .spawn(
                        EntityKind::Player,
                        QVec3::ZERO,
                        Q32::from_int(1_200),
                        sh_a,
                    )
                    .ok_or("Tournament spawn A failed")?;
                let b = arena
                    .spawn(
                        EntityKind::Player,
                        QVec3::new(sep_x, Q32::ZERO, Q32::ZERO),
                        Q32::from_int(1_200),
                        sh_b,
                    )
                    .ok_or("Tournament spawn B failed")?;
                arena.rng_state[a.0 as usize] = rng.next_u64();
                arena.rng_state[b.0 as usize] = rng.next_u64();
                let mut cmds = CommandBuffer::with_capacity(64);

                let mut t: u64 = 0;
                while t < 60 {
                    cmds.clear();
                    {
                        let view = arena.view();
                        execute_handler(&weapons[i], &view, a, b, &mut cmds)
                            .map_err(|_| "Season weapon i trapped in duel")?;
                        execute_handler(&weapons[j], &view, b, a, &mut cmds)
                            .map_err(|_| "Season weapon j trapped in duel")?;
                        if rel == 1 && t.is_multiple_of(3) {
                            execute_handler(&weapons[i], &view, a, b, &mut cmds)
                                .map_err(|_| "Season weapon i resonance trapped")?;
                        } else if rel.saturating_add(1) == m && t.is_multiple_of(3) {
                            execute_handler(&weapons[j], &view, b, a, &mut cmds)
                                .map_err(|_| "Season weapon j resonance trapped")?;
                        }
                    }
                    arena.commit_commands(&mut cmds);
                    arena.step_kinematics(dt);
                    if arena.hp[b.0 as usize] < Q32::from_int(1_200) {
                        dealt[i] = true;
                    }
                    if arena.hp[a.0 as usize] < Q32::from_int(1_200) {
                        dealt[j] = true;
                    }
                    if arena.hp[a.0 as usize] <= Q32::ZERO || arena.hp[b.0 as usize] <= Q32::ZERO {
                        break;
                    }
                    t = t.saturating_add(1);
                }
                let hp_a = arena.hp[a.0 as usize];
                let hp_b = arena.hp[b.0 as usize];
                if hp_a > hp_b {
                    score_i_half = score_i_half.saturating_add(2);
                } else if hp_a == hp_b {
                    score_i_half = score_i_half.saturating_add(1);
                }
                tr = tr.saturating_add(1);
            }
            half_pts[i][j] = score_i_half;
            half_pts[j][i] = 40_u32.saturating_sub(score_i_half);
            j = j.saturating_add(1);
        }
        i = i.saturating_add(1);
    }

    if dealt.contains(&false) {
        return Err("A season weapon dealt 0 HP damage across all round-robin duels");
    }
    verify_mw_meta_balance_q32(&half_pts)
}

/// Exact fixed-point `log2(x)` for `x > 0` in `Q32.32` (integer MSB + 32 rounds of squaring), `0` floats.
fn q32_log2(x: Q32) -> Q32 {
    let raw = x.raw();
    if raw <= 0 {
        return Q32::from_int(-64);
    }
    let u = raw as u64;
    let msb = 63_u32.saturating_sub(u.leading_zeros());
    let int_part = i64::from(msb).saturating_sub(32);
    let mut y: u128 = if msb >= 32 {
        u128::from(u.checked_shr(msb.saturating_sub(32)).unwrap_or(0))
    } else {
        u128::from(u.checked_shl(32_u32.saturating_sub(msb)).unwrap_or(0))
    };
    let two_pow_33: u128 = 1_u128.checked_shl(33).unwrap_or(u128::MAX);
    let mut frac: i64 = 0;
    let mut bit: i64 = 0x8000_0000;
    let mut round: u32 = 0;
    while round < 32 {
        y = y.saturating_mul(y).checked_shr(32).unwrap_or(0);
        if y >= two_pow_33 {
            y = y.checked_shr(1).unwrap_or(0);
            frac |= bit;
        }
        bit = bit.checked_shr(1).unwrap_or(0);
        round = round.saturating_add(1);
    }
    Q32::from_raw(int_part.saturating_mul(4_294_967_296).saturating_add(frac))
}

/// Exact `Q32.32` base-2 Jensen-Shannon divergence between two pattern histograms.
fn q32_jsd_bits(a: &[u32], b: &[u32]) -> Q32 {
    let na: u64 = a.iter().map(|&v| u64::from(v)).fold(0, u64::saturating_add);
    let nb: u64 = b.iter().map(|&v| u64::from(v)).fold(0, u64::saturating_add);
    if na == 0 || nb == 0 {
        return Q32::ZERO;
    }
    let na_i = i32::try_from(na).unwrap_or(i32::MAX);
    let nb_i = i32::try_from(nb).unwrap_or(i32::MAX);
    let half = Q32::from_ratio_i32(1, 2);
    let mut acc = Q32::ZERO;
    for (ca, cb) in a.iter().zip(b.iter()) {
        let pa = Q32::from_ratio_i32(i32::try_from(*ca).unwrap_or(0), na_i);
        let pb = Q32::from_ratio_i32(i32::try_from(*cb).unwrap_or(0), nb_i);
        let mid = pa.sat_add(pb).sat_mul(half);
        let lm = q32_log2(mid);
        if pa > Q32::ZERO {
            acc = acc.sat_add(half.sat_mul(pa).sat_mul(q32_log2(pa).sat_sub(lm)));
        }
        if pb > Q32::ZERO {
            acc = acc.sat_add(half.sat_mul(pb).sat_mul(q32_log2(pb).sat_sub(lm)));
        }
    }
    acc
}

/// Deterministic `Q32.32` Multiplicative Weights meta-balance judge (`게임.md §2.3, §10.1 G2`):
/// `T = 10,000`, `eta = 0.05`, `p^(1) = 1/M`, time-averaged `p_bar`; enforces `N_eff = 2^H >= 6`
/// (`H >= log2 6` bits), `max p_bar <= 0.25`, and inside `S_top = {i : p_bar_i >= min(0.05, 0.5/M)}`
/// a `>= 55%` counter for every member and an average-winrate spread `<= 10%p`.
/// Returns the `S_top` spread in tournament half-points.
fn verify_mw_meta_balance_q32(half_pts: &[Vec<u32>]) -> Result<u32, &'static str> {
    const LOG2_E_Q32: Q32 = Q32::from_raw(6_196_328_019);
    let m = half_pts.len();
    if m < 6 || half_pts.iter().any(|r| r.len() != m) {
        return Err("G2 requires a square win matrix with M >= 6 equivalence classes");
    }
    let m_i32 = i32::try_from(m).map_err(|_| "M overflow")?;
    let eta_log2e = Q32::from_ratio_i32(1, 20).sat_mul(LOG2_E_Q32);
    let centered: Vec<Vec<Q32>> = half_pts
        .iter()
        .map(|row| {
            row.iter()
                .map(|&h| {
                    Q32::from_ratio_i32(i32::try_from(h).unwrap_or(20).saturating_sub(20), 40)
                })
                .collect()
        })
        .collect();
    let mut p = vec![Q32::from_ratio_i32(1, m_i32); m];
    let mut p_sum = vec![Q32::ZERO; m];
    let mut next = vec![Q32::ZERO; m];
    let mut t: u32 = 0;
    while t < 10_000 {
        let mut norm = Q32::ZERO;
        let mut i: usize = 0;
        while i < m {
            p_sum[i] = p_sum[i].sat_add(p[i]);
            let mut payoff = Q32::ZERO;
            let mut j: usize = 0;
            while j < m {
                payoff = payoff.sat_add(centered[i][j].sat_mul(p[j]));
                j = j.saturating_add(1);
            }
            next[i] = p[i].sat_mul(payoff.sat_mul(eta_log2e).exp2_sat());
            norm = norm.sat_add(next[i]);
            i = i.saturating_add(1);
        }
        if norm <= Q32::ZERO {
            return Err("G2 Multiplicative Weights normalization collapsed");
        }
        let mut k: usize = 0;
        while k < m {
            p[k] = next[k].sat_div(norm);
            k = k.saturating_add(1);
        }
        t = t.saturating_add(1);
    }
    let t_q = Q32::from_int(10_000);
    let p_bar: Vec<Q32> = p_sum.iter().map(|&s| s.sat_div(t_q)).collect();

    let mut entropy_bits = Q32::ZERO;
    let mut max_share = Q32::ZERO;
    for &pi in &p_bar {
        if pi > Q32::ZERO {
            entropy_bits = entropy_bits.sat_sub(pi.sat_mul(q32_log2(pi)));
        }
        max_share = max_share.max(pi);
    }
    if entropy_bits < q32_log2(Q32::from_int(6)) {
        return Err("G2 N_eff < 6.0 under Multiplicative Weights time-average");
    }
    if max_share > Q32::from_ratio_i32(1, 4) {
        return Err("G2 max_i p_bar_i > 0.25 under Multiplicative Weights time-average");
    }
    let thresh = Q32::from_ratio_i32(1, 20).min(Q32::from_ratio_i32(1, m_i32.saturating_mul(2)));
    let s_top: Vec<usize> = (0..m).filter(|&i| p_bar[i] >= thresh).collect();
    if s_top.is_empty() {
        return Err("G2 S_top unexpectedly empty");
    }
    let mut min_sum = u32::MAX;
    let mut max_sum = 0_u32;
    for &i in &s_top {
        let mut row_sum: u32 = 0;
        let mut has_counter = false;
        for &j in &s_top {
            row_sum = row_sum.saturating_add(half_pts[i][j]);
            if half_pts[j][i] >= 22 {
                has_counter = true;
            }
        }
        if !has_counter {
            return Err("G2 non-transitive >=55% counter missing inside S_top");
        }
        min_sum = min_sum.min(row_sum);
        max_sum = max_sum.max(row_sum);
    }
    // `(max_avg - min_avg) <= 0.10`  <=>  `(max_sum - min_sum) * 10 <= |S_top| * 40` (half-point units):
    let spread = max_sum.saturating_sub(min_sum);
    let s_len = u32::try_from(s_top.len()).unwrap_or(u32::MAX);
    if spread.saturating_mul(10) > s_len.saturating_mul(40) {
        return Err("G2 S_top average winrate spread exceeded 10%p");
    }
    Ok(spread)
}

fn verify_season_chunks(pack: &SeasonGameplayPack) -> Result<usize, &'static str> {
    if pack.manifest.chunk_seeds.len() < 4 {
        return Err("Season manifest requires >= 4 vertical 3D world chunks");
    }
    let mut world = SparseSdfWorld::new();
    let mut tick_tag: u64 = 1;
    for &(cx, cz, seed) in &pack.manifest.chunk_seeds {
        world.generate_vertical_chunk(cx, cz, seed);
        let topo = world.analyze_chunk_topology(cx, cz);
        if topo.walkable_surface_voxels < 256
            || topo.sigma_z_q32 < Q32::from_int(2)
            || !topo.fully_3d_connected
        {
            return Err("Season 3D vertical chunk failed G1 surface/elevation/connectivity gate");
        }
        // Probe the actual top solid surface voxel (scan downward) so the CSG strike lands on terrain:
        let px = cx.saturating_mul(32).saturating_add(8);
        let pz = cz.saturating_mul(32).saturating_add(8);
        let mut surf_y: i32 = 30;
        while surf_y > 0 && world.sample_sdf_cm(px, surf_y, pz) > 0 {
            surf_y = surf_y.saturating_sub(1);
        }
        if surf_y <= 0 {
            return Err("Season 3D chunk has no solid surface voxel at probe column");
        }
        let csg_d = world.carve_sphere_csg(tick_tag, [px, surf_y, pz], 3);
        tick_tag = tick_tag.saturating_add(1);
        // Thermal updraft reaction in the open air column directly above the struck surface:
        let react_d = world.apply_element_stimulus(
            tick_tag,
            [px, surf_y.saturating_add(2), pz],
            3,
            ElementStimulus::PyroThermal,
        );
        tick_tag = tick_tag.saturating_add(1);
        // `§1.1 / §10.1 G1` requires BOTH `ΔSDF_csg != 0` AND `ΔField_react != 0` in every chunk:
        if csg_d == 0 || react_d == 0 {
            return Err("Season 3D chunk failed simultaneous CSG destruction and elemental reaction (G1)");
        }
    }
    Ok(pack.manifest.chunk_seeds.len())
}

/// Executes comprehensive `Phase 3` verification (`게임.md §2, §3, §4.3, §11.4`):
/// multi-axis resonance traits (`Normal->Gold->Prism->Emblem`), 3-tier tactical gate augments (`Silver->Gold->Prism`),
/// `4` adaptive multi-phase bosses (`0` invulnerability stall), `E_1` & `E_2` full set replacement + round-robin tournaments,
/// `3-iter` WL `G4` novelty (`d_mech >= 0.20`), and `GroupCommitWal` `E_1 (S=1) -> E_2 (S=2)` orthogonal rollover!
fn run_phase3_season_synergy_boss_and_rollover_drill() -> Result<Phase3SeasonTelemetry, &'static str> {
    let gameplay_dir = Path::new("gameplay");
    let e1_pack = load_season_gameplay_pack(Path::new("assets/manifests/E_1.ron"), gameplay_dir)
        .map_err(|_| "Failed loading Season E_1 manifest and gameplay pack")?;
    let e2_pack = load_season_gameplay_pack(Path::new("assets/manifests/E_2.ron"), gameplay_dir)
        .map_err(|_| "Failed loading Season E_2 manifest and gameplay pack")?;

    // 1. Verify full orthogonal Set-Season content replacement (`E_1` S=1 vs `E_2` S=2, `0%` overlap):
    if e1_pack.manifest.season_id != 1 || e2_pack.manifest.season_id != 2 {
        return Err("E_1 and E_2 must target distinct seasons S=1 and S=2");
    }
    if e1_pack.manifest.elements.len() < 3
        || e2_pack.manifest.elements.len() < 3
        || e1_pack
            .manifest
            .elements
            .iter()
            .any(|el| e2_pack.manifest.elements.contains(el))
    {
        return Err("E_1 and E_2 elements must each have >=3 elements with 100% seasonal replacement");
    }
    if e1_pack.manifest.weapon_modules.len() < 9
        || e2_pack.manifest.weapon_modules.len() < 9
        || e1_pack
            .manifest
            .weapon_modules
            .iter()
            .any(|w| e2_pack.manifest.weapon_modules.contains(w))
    {
        return Err("E_1 and E_2 weapon pools must each have >=9 weapons with 0% overlap");
    }
    if e1_pack.manifest.boss_modules.len() < 2
        || e2_pack.manifest.boss_modules.len() < 2
        || e1_pack
            .manifest
            .boss_modules
            .iter()
            .any(|b| e2_pack.manifest.boss_modules.contains(b))
    {
        return Err("E_1 and E_2 boss pools must each have >=2 multi-phase bosses with 0% overlap");
    }
    if e2_pack.manifest.base_move_speed_milli < e1_pack.manifest.base_move_speed_milli
        || e2_pack.manifest.expected_farm_ticks > e1_pack.manifest.expected_farm_ticks
    {
        return Err("E_2 degraded movement speed or inflated farming ceiling relative to E_1");
    }

    // 2. Verify all 3D vertical world chunks across both `E_1` and `E_2`:
    let e1_chunks = verify_season_chunks(&e1_pack)?;
    let e2_chunks = verify_season_chunks(&e2_pack)?;

    // 3. Compile `prelude.qvm` + all `.qvm` modules across `weapons/`, `traits/`, `augments/`, `bosses/`, `world_events/`,
    //    and verify pairwise 3-iter Weisfeiler-Lehman `G4` novelty (`d_mech >= 0.20` across ALL modules!):
    let _ = compile_qvm_source(&e1_pack.prelude_src)
        .map_err(|_| "gameplay/prelude.qvm failed static DAG compilation")?;
    let mut all_phis: Vec<BTreeMap<u64, i64>> = Vec::with_capacity(e1_pack.all_modules_rel.len());
    for src in e1_pack.all_modules_rel.values() {
        let full = format!("{}\n{src}", e1_pack.prelude_src);
        let m = compile_qvm_source(&full).map_err(|_| "Gameplay qvm module failed compilation")?;
        if m.causal_graph.causal_edges.is_empty() {
            return Err("Gameplay qvm module produced empty CausalGraphIR");
        }
        all_phis.push(wl_phi_exact(&m.causal_graph));
    }
    let mut pi: usize = 0;
    while pi < all_phis.len() {
        let mut pj = pi.saturating_add(1);
        while pj < all_phis.len() {
            if !wl_distance_ge_ratio_exact(&all_phis[pi], &all_phis[pj], 1, 5) {
                return Err("G4 Weisfeiler-Lehman novelty violation (d_mech < 0.20) between gameplay modules");
            }
            pj = pj.saturating_add(1);
        }
        pi = pi.saturating_add(1);
    }

    // 4. Compile & tournament-verify both Season 1 (`e1_w01..e1_w09`) and Season 2 (`e2_w01..e2_w09`) weapon pools:
    let mut e1_wpn_mods = Vec::with_capacity(e1_pack.manifest.weapon_modules.len());
    for stem in &e1_pack.manifest.weapon_modules {
        let body = e1_pack
            .weapons
            .get(stem)
            .ok_or("Missing E_1 weapon .qvm file in gameplay/weapons/")?;
        let m = compile_qvm_source(&format!("{}\n{body}", e1_pack.prelude_src))
            .map_err(|_| "Failed compiling E_1 weapon module")?;
        e1_wpn_mods.push(m);
    }
    let mut e2_wpn_mods = Vec::with_capacity(e2_pack.manifest.weapon_modules.len());
    for stem in &e2_pack.manifest.weapon_modules {
        let body = e2_pack
            .weapons
            .get(stem)
            .ok_or("Missing E_2 weapon .qvm file in gameplay/weapons/")?;
        let m = compile_qvm_source(&format!("{}\n{body}", e2_pack.prelude_src))
            .map_err(|_| "Failed compiling E_2 weapon module")?;
        e2_wpn_mods.push(m);
    }
    let e1_spread_half = verify_season_weapon_tournament(&e1_wpn_mods)?;
    let e2_spread_half = verify_season_weapon_tournament(&e2_wpn_mods)?;

    // 5. Verify Multi-Axis Resonance Trait Network (`3 weapons + Simulacrum + Matrix + Relic + 4-Player Party Cross-Resonance`)
    //    across `Normal -> Gold -> Prism -> Emblem` qualitative mutations AND `Silver -> Gold -> Prism` gate tactical augments (`§2.1, §2.2`):
    let trait_sim = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .traits
            .get("t01_simulacrum_awakening_cryo_volt")
            .ok_or("Missing t01 trait")?
    ))
    .map_err(|_| "Failed compiling t01 trait")?;
    let trait_mat = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .traits
            .get("t02_matrix_quartet_pyro_void")
            .ok_or("Missing t02 trait")?
    ))
    .map_err(|_| "Failed compiling t02 trait")?;
    let trait_rel = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .traits
            .get("t03_relic_prism_emblem_fusion")
            .ok_or("Missing t03 trait")?
    ))
    .map_err(|_| "Failed compiling t03 trait")?;

    // Verify qualitative mechanism graph distance `d_mech >= 0.10` between single-weapon baseline and full resonance trait:
    if !wl_distance_ge_ratio_exact(
        &wl_phi_exact(&e1_wpn_mods[0].causal_graph),
        &wl_phi_exact(&trait_sim.causal_graph),
        1,
        10,
    ) {
        return Err("Resonance trait failed G2 qualitative causal graph mutation (d_mech < 0.10)");
    }

    let eval_party_resonance_tier = |party_masks: [u32; 4]| -> Result<(i64, Q32), &'static str> {
        let mut arena = CombatArena::new_boxed(0x7A17_2026);
        let mut party = Vec::with_capacity(4);
        let mut combined_party_mask: u32 = 0;
        for &pm in &party_masks {
            combined_party_mask |= pm;
        }
        for (idx, _) in party_masks.iter().enumerate() {
            let p = arena
                .spawn(
                    EntityKind::Player,
                    QVec3::from_ints(idx as i32, 2, 0),
                    Q32::from_int(5_000),
                    Q32::from_int(300),
                )
                .ok_or("Party player spawn failed")?;
            arena.trait_mask[p.0 as usize] = combined_party_mask;
            party.push(p);
        }
        let boss = arena
            .spawn(
                EntityKind::Boss,
                QVec3::from_ints(2, 2, 0),
                Q32::from_int(10_000),
                Q32::from_int(600),
            )
            .ok_or("Party boss spawn failed")?;
        let mut cb = CommandBuffer::with_capacity(128);
        let dt = Q32::from_ratio_i32(1, 60);
        for _ in 0..8 {
            cb.clear();
            {
                let view = arena.view();
                execute_handler(&e1_wpn_mods[0], &view, party[0], boss, &mut cb)
                    .map_err(|_| "Party weapon 0 trapped")?;
                execute_handler(&trait_sim, &view, party[0], boss, &mut cb)
                    .map_err(|_| "Trait sim trapped")?;
                execute_handler(&trait_mat, &view, party[1], boss, &mut cb)
                    .map_err(|_| "Trait matrix trapped")?;
                execute_handler(&trait_rel, &view, party[2], boss, &mut cb)
                    .map_err(|_| "Trait relic trapped")?;
            }
            arena.commit_commands(&mut cb);
            arena.step_kinematics(dt);
        }
        let dmg_raw = Q32::from_int(10_000).sat_sub(arena.hp[boss.0 as usize]).raw();
        Ok((dmg_raw, arena.time_scale[boss.0 as usize]))
    };

    // Normal (bit 0 only) < Gold (+ bits 1, 3, 5) < Prism + Party Cross-Resonance Emblem (+ bits 2, 4, 6, 7):
    let (dmg_normal, ts_normal) = eval_party_resonance_tier([0b0000_0001, 0, 0, 0])?;
    let (dmg_gold, _) = eval_party_resonance_tier([0b0000_0011, 0b0000_1000, 0b0010_0000, 0])?;
    let (dmg_prism, ts_prism) = eval_party_resonance_tier([
        0b0000_0111, // Attack Simulacrum Prism
        0b0001_1000, // Fortitude 4pc Matrix
        0b0010_0000, // Energy Relic
        0b1100_0000, // Benediction Party Resonance + Trait Emblem Fusion
    ])?;
    if !(dmg_normal > 0 && dmg_gold > dmg_normal && dmg_prism > dmg_gold && ts_prism < ts_normal) {
        return Err("Multi-axis Normal->Gold->Prism+Emblem party resonance failed qualitative/monotonic escalation");
    }
    let prism_damage_delta = dmg_prism.saturating_sub(dmg_normal);

    // Verify 3-tier (`Silver -> Gold -> Prism`) instance-gate tactical augments (`a01`, `a02`, `a03`):
    let aug_silver = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .augments
            .get("a01_silver_kinetic_ricochet")
            .ok_or("Missing Silver augment")?
    ))
    .map_err(|_| "Failed compiling Silver augment")?;
    let aug_gold = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .augments
            .get("a02_gold_shatter_superconductor")
            .ok_or("Missing Gold augment")?
    ))
    .map_err(|_| "Failed compiling Gold augment")?;
    let aug_prism = compile_qvm_source(&format!(
        "{}\n{}",
        e1_pack.prelude_src,
        e1_pack
            .augments
            .get("a03_prism_singularity_cataclysm")
            .ok_or("Missing Prism augment")?
    ))
    .map_err(|_| "Failed compiling Prism augment")?;

    let eval_augment = |aug: &CompiledQvmModule| -> Result<i64, &'static str> {
        let mut arena = CombatArena::new_boxed(555);
        let p = arena
            .spawn(EntityKind::Player, QVec3::ZERO, Q32::from_int(2_000), Q32::from_int(200))
            .ok_or("Augment player spawn failed")?;
        let b = arena
            .spawn(
                EntityKind::Boss,
                QVec3::from_ints(2, 0, 0),
                Q32::from_int(5_000),
                Q32::from_int(300),
            )
            .ok_or("Augment boss spawn failed")?;
        let mut cb = CommandBuffer::with_capacity(64);
        let dt = Q32::from_ratio_i32(1, 60);
        for _ in 0..4 {
            cb.clear();
            execute_handler(aug, &arena.view(), p, b, &mut cb)
                .map_err(|_| "Gate tactical augment trapped")?;
            arena.commit_commands(&mut cb);
            arena.step_kinematics(dt);
        }
        Ok(Q32::from_int(5_000).sat_sub(arena.hp[b.0 as usize]).raw())
    };
    let d_sil = eval_augment(&aug_silver)?;
    let d_gld = eval_augment(&aug_gold)?;
    let d_prm = eval_augment(&aug_prism)?;
    if !(d_sil > 0 && d_gld > d_sil && d_prm > d_gld) {
        return Err("3-tier Silver->Gold->Prism gate tactical augments failed monotonic power scaling");
    }
    let prism_augment_delta = d_prm.saturating_sub(d_sil);

    // 6. Verify all 4 Multi-Phase Adaptive Bosses (`E_1` + `E_2`) & both Dynamic World Events (`§4.3, §5.2`):
    let mut bosses_verified: usize = 0;
    for (pack, is_s2) in [(&e1_pack, false), (&e2_pack, true)] {
        for b_stem in &pack.manifest.boss_modules {
            let b_src = pack
                .bosses
                .get(b_stem)
                .ok_or("Missing boss .qvm file in gameplay/bosses/")?;
            let b_mod = compile_qvm_source(&format!("{}\n{b_src}", pack.prelude_src))
                .map_err(|_| "Boss qvm module failed compilation")?;
            let loadout_a: u32 = if is_s2 { 1_u32 << 8 } else { 1_u32 << 0 };
            let loadout_b: u32 = if is_s2 {
                (1_u32 << 9) | (1_u32 << 10)
            } else {
                (1_u32 << 1) | (1_u32 << 2) | (1_u32 << 4)
            };

            let sim_boss_loadout = |mask: u32, start_hp: i32| -> Result<u16, &'static str> {
                let mut arena = CombatArena::new_boxed(0xB055);
                let boss = arena
                    .spawn(
                        EntityKind::Boss,
                        QVec3::from_ints(4, 2, 0),
                        Q32::from_int(start_hp),
                        Q32::from_int(400),
                    )
                    .ok_or("Boss spawn failed")?;
                let player = arena
                    .spawn(
                        EntityKind::Player,
                        QVec3::ZERO,
                        Q32::from_int(5_000),
                        Q32::from_int(300),
                    )
                    .ok_or("Player spawn failed")?;
                arena.trait_mask[player.0 as usize] = mask;
                arena.charge[player.0 as usize] = Q32::from_int(85);
                let mut cb = CommandBuffer::with_capacity(64);
                execute_handler(&b_mod, &arena.view(), boss, player, &mut cb)
                    .map_err(|_| "Adaptive boss qvm handler trapped")?;
                // Also verify `0` invulnerability stall ticks: player counter/attack ALWAYS reduces boss shield/HP!
                execute_handler(&e1_wpn_mods[0], &arena.view(), player, boss, &mut cb)
                    .map_err(|_| "Player counter against boss trapped")?;
                let pre_hp = arena.hp[boss.0 as usize];
                arena.commit_commands(&mut cb);
                if arena.hp[boss.0 as usize] >= pre_hp {
                    return Err("Boss exhibited illegal invulnerability time-stall (0 HP damage taken)");
                }
                Ok(arena.boss_pattern[boss.0 as usize])
            };

            let pat_p1_a = sim_boss_loadout(loadout_a, 90_000)?;
            let pat_p1_b = sim_boss_loadout(loadout_b, 90_000)?;
            let pat_enrage = sim_boss_loadout(loadout_a, 20_000)?;
            if pat_p1_a == pat_p1_b && pat_p1_a == pat_enrage {
                return Err("Boss failed real-time adaptive tactical modulation across loadouts/phases");
            }
            bosses_verified = bosses_verified.saturating_add(1);
        }
    }
    for we_src in e1_pack.world_events.values() {
        let we_mod = compile_qvm_source(&format!("{}\n{we_src}", e1_pack.prelude_src))
            .map_err(|_| "World event qvm failed compilation")?;
        let mut arena = CombatArena::new_boxed(777);
        let p = arena
            .spawn(EntityKind::Player, QVec3::ZERO, Q32::from_int(1_000), Q32::ZERO)
            .ok_or("World event player spawn failed")?;
        let b = arena
            .spawn(EntityKind::Boss, QVec3::from_ints(3, 0, 0), Q32::from_int(1_000), Q32::ZERO)
            .ok_or("World event target spawn failed")?;
        let mut cb = CommandBuffer::with_capacity(32);
        execute_handler(&we_mod, &arena.view(), p, b, &mut cb)
            .map_err(|_| "World event qvm trapped")?;
        arena.commit_commands(&mut cb);
        if arena.vel_y[p.0 as usize] <= Q32::ZERO {
            return Err("World event failed to apply vertical updraft / local gravity inversion impulse");
        }
    }

    // 7. Rehearse live `DualSlotEpochTable` + `GroupCommitWal` Orthogonal Set-Season Rollover
    //    `E_1 (S=1) -> E_2 (S=2)` (`Power_S -> 0`, `d Power_{S+1} / d Legacy_S == 0`, `100%` Legacy preserved, `§3.1~§3.3`):
    let e1_summary = EpochManifestSummary {
        epoch_id: e1_pack.manifest.epoch_id,
        season_id: e1_pack.manifest.season_id,
        merkle_root: [0; 32],
        faucet_caps: e1_pack.manifest.faucet_caps,
        base_move_speed_q32: Q32::from_ratio_i32(e1_pack.manifest.base_move_speed_milli, 1_000),
        expected_farm_ticks: e1_pack.manifest.expected_farm_ticks,
        synth_clear_permille: 520,
        synth_exit_mean_permille: 18,
        synth_exit_std_permille: 9,
    };
    let e2_summary = EpochManifestSummary {
        epoch_id: e2_pack.manifest.epoch_id,
        season_id: e2_pack.manifest.season_id,
        merkle_root: [0; 32],
        faucet_caps: e2_pack.manifest.faucet_caps,
        base_move_speed_q32: Q32::from_ratio_i32(e2_pack.manifest.base_move_speed_milli, 1_000),
        expected_farm_ticks: e2_pack.manifest.expected_farm_ticks,
        synth_clear_permille: 530,
        synth_exit_mean_permille: 17,
        synth_exit_std_permille: 9,
    };
    let spirv_d = blake3_digest(b"engine.slang:season_hotswap");
    let e1_bundle = CanonicalEpochBundle::seal_new(
        e1_summary,
        e1_pack.content_merkle,
        spirv_d,
        blake3_digest(b"E_1_chunks"),
        0,
        1_000,
    );
    let e2_bundle = CanonicalEpochBundle::seal_new(
        e2_summary,
        e2_pack.content_merkle,
        spirv_d,
        blake3_digest(b"E_2_chunks"),
        0,
        1_000,
    );
    let season_slots = DualSlotEpochTable::new(e1_bundle);
    let mut s1_encounter_lease = season_slots.pin_for_new_encounter();
    let e2_wire = e2_bundle
        .encode_stream_tlv_v2(b"E_2_full_season_set_rollover")
        .map_err(|_| "Failed encoding E_2 season stream TLV")?;
    let staged_e2 = CanonicalEpochBundle::decode_stream_tlv(&e2_wire, false)
        .map_err(|_| "Failed decoding E_2 season stream TLV")?;
    season_slots
        .mount_staged_epoch(staged_e2)
        .map_err(|_| "Failed mounting staged E_2 into DualSlotEpochTable")?;
    // Active S=1 encounter remains pinned to S=1 until encounter boundary:
    if s1_encounter_lease.bundle().pinned_handle.manifest().season_id != 1 {
        return Err("Active S=1 encounter mutated to S=2 mid-combat!");
    }
    s1_encounter_lease.drain_at_boundary(&season_slots);
    if s1_encounter_lease.bundle().pinned_handle.manifest().season_id != 2
        || season_slots.try_reclaim_zero_ref_standby() != Some(1)
    {
        return Err("Season boundary drain E_1 (S=1) -> E_2 (S=2) or E_1 zero-ref reclaim failed");
    }

    let s_wal_dir = Path::new("state/wal/phase3_season_rollover");
    let (mut s_wal, _) = GroupCommitWal::open_and_recover(s_wal_dir, 8)
        .map_err(|_| "Failed opening Phase 3 season rollover GroupCommitWal")?;
    let gen_id = (s_wal.committed_ledger().legacy_count() as u64).saturating_add(1);
    let minted_prev =
        i64::try_from(s_wal.committed_ledger().total_faucet_minted_amount(AssetId(1))).unwrap_or(0);
    let cur_rem0 = s_wal.committed_ledger().shard_quota_remaining(0, AssetId(1));
    let target_rem0 = cur_rem0.max(50_000);
    let s1_cap = minted_prev.saturating_add(target_rem0).saturating_add(100_000);

    // Reset to S=1 if recovering from a previous run, grant S=1 power + Legacy emblems, then rollover `S=1 -> S=2`!
    let _ = s_wal
        .submit_op(WalOp::RolloverSeason { next_season: 1 }, 10)
        .map_err(|_| "WAL RolloverSeason(1) setup failed")?;
    let _ = s_wal
        .submit_op(
            WalOp::ConfigureAssetCap {
                asset: AssetId(1),
                epoch_cap: s1_cap,
            },
            20,
        )
        .map_err(|_| "WAL ConfigureAssetCap(S=1) failed")?;
    let top_up0 = 50_000_i64.saturating_sub(cur_rem0);
    if top_up0 > 0 {
        let _ = s_wal
            .submit_op(
                WalOp::AllocateShardQuota {
                    shard_id: 0,
                    asset: AssetId(1),
                    quota: top_up0,
                },
                30,
            )
            .map_err(|_| "WAL AllocateShardQuota(S=1) failed")?;
    }
    let veteran = AccountId(1);
    let newcomer = AccountId(2);
    let emblem_id = gen_id.saturating_mul(1_000_000).saturating_add(101);
    let cosmetic_id = gen_id.saturating_mul(1_000_000).saturating_add(202);
    let _ = s_wal
        .submit_tx(
            LedgerTxKind::FaucetMint {
                shard_id: 0,
                to: veteran,
                asset: AssetId(1),
                amount: 1_500,
            },
            40,
        )
        .map_err(|_| "WAL S=1 FaucetMint failed")?;
    let _ = s_wal
        .submit_op(
            WalOp::GrantSeasonalPower {
                account: veteran,
                delta: 12_500,
            },
            50,
        )
        .map_err(|_| "WAL S=1 GrantSeasonalPower veteran failed")?;
    let _ = s_wal
        .submit_op(
            WalOp::GrantSeasonalPower {
                account: newcomer,
                delta: 4_000,
            },
            60,
        )
        .map_err(|_| "WAL S=1 GrantSeasonalPower newcomer failed")?;
    let _ = s_wal
        .submit_tx(
            LedgerTxKind::GrantLegacy {
                account: veteran,
                legacy_item_id: emblem_id,
            },
            70,
        )
        .map_err(|_| "WAL S=1 GrantLegacy emblem failed")?;
    let _ = s_wal
        .submit_tx(
            LedgerTxKind::ExternalCosmeticPurchase {
                account: veteran,
                cosmetic_id,
                requested_power_bonus: 0,
            },
            80,
        )
        .map_err(|_| "WAL zero-P2W cosmetic purchase failed")?;
    // Non-zero P2W power influence MUST be rejected:
    if !matches!(
        s_wal.submit_tx(
            LedgerTxKind::ExternalCosmeticPurchase {
                account: veteran,
                cosmetic_id: cosmetic_id.saturating_add(1),
                requested_power_bonus: 1,
            },
            85,
        ),
        Err(ledger_wal::WalError::Ledger(
            LedgerError::P2WPowerInfluenceForbidden
        ))
    ) {
        return Err("Season rollover WAL failed to reject P2W power influence");
    }
    let _ = s_wal
        .flush_group_commit()
        .map_err(|_| "WAL S=1 flush failed")?;
    if s_wal.committed_ledger().seasonal_power(veteran) != 12_500 {
        return Err("S=1 seasonal power not committed before rollover");
    }

    // Execute durable Orthogonal Season Rollover `S=1 -> S=2` + checkpoint + reopen recovery!
    let _ = s_wal
        .submit_op(WalOp::RolloverSeason { next_season: 2 }, 100)
        .map_err(|_| "WAL RolloverSeason(2) failed")?;
    let roll_acks = s_wal
        .flush_group_commit()
        .map_err(|_| "WAL RolloverSeason(2) flush failed")?;
    if roll_acks.is_empty() {
        return Err("WAL RolloverSeason(2) emitted zero fsync CommittedAcks");
    }
    s_wal
        .write_checkpoint()
        .map_err(|_| "WAL S=2 write_checkpoint failed")?;
    drop(s_wal);

    let (rec_s2_wal, _) = GroupCommitWal::open_and_recover(s_wal_dir, 8)
        .map_err(|_| "WAL S=2 open_and_recover failed")?;
    let s2_led = rec_s2_wal.committed_ledger();
    s2_led
        .verify_global_conservation()
        .map_err(|_| "S=2 post-rollover global conservation failed")?;
    let vet_s2_power = s2_led.seasonal_power(veteran);
    let new_s2_power = s2_led.seasonal_power(newcomer);
    // Strict `d Power_{S+1} / d Legacy_S == 0`: veteran (with S=1 Legacy) and newcomer (0 Legacy) BOTH start S=2 at 0!
    if s2_led.season_id() != 2 || vet_s2_power != 0 || new_s2_power != 0 {
        return Err("Season rollover E_1->E_2 failed to reset Power_S to 0 orthogonally");
    }
    if !s2_led.has_legacy_item(veteran, emblem_id)
        || !s2_led.has_legacy_item(veteran, cosmetic_id)
        || s2_led.export_legacy_scoped(veteran, emblem_id).is_none()
    {
        return Err("Season rollover E_1->E_2 lost permanent Legacy collection items");
    }

    Ok(Phase3SeasonTelemetry {
        total_qvm_modules: e1_pack.all_modules_rel.len(),
        e1_weapons: e1_wpn_mods.len(),
        e2_weapons: e2_wpn_mods.len(),
        e1_chunks,
        e2_chunks,
        e1_spread_half,
        e2_spread_half,
        prism_damage_delta,
        prism_augment_delta,
        bosses_verified,
        s2_post_rollover_power: vet_s2_power,
        s2_preserved_legacy: s2_led.legacy_count(),
    })
}

fn main() -> Result<(), &'static str> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(|_| "Failed building tokio multi-thread runtime")?;

    let tel = rt.block_on(run_phase2_raid_and_hotswap_drill())?;
    println!(
        "PHASE2_RAID_OK agents={} udp_delivered={} udp_dropped={} rewinds={} skills_ok={} skills_lost={} teleports={} tampered_rejected={} portal_transfers={} portal_prewarms={} ghosts_peak={} channel_splits={} max_shard_us={} max_shard_fuel={} max_call_fuel={} reconnect_us={} sdf_streams={} sdf_bricks_synced={} wal_acks={} wal_fsync_us={} epoch_drift={} reclaimed={:?} legacy_items={} season_weapons={} trait_casts={} augment_casts={} world_events={}",
        RAID_AGENT_COUNT,
        tel.udp_delivered,
        tel.udp_dropped,
        tel.server_rewinds,
        tel.committed_skills,
        tel.evaporated_skills,
        tel.teleport_anomalies,
        tel.tampered_rejected,
        tel.portal_transfers,
        tel.portal_prewarms,
        tel.ghost_mirrors_peak,
        tel.elastic_channel_splits,
        tel.max_shard_compute_us,
        tel.max_shard_fuel,
        tel.max_call_fuel,
        tel.reconnect_latency_us,
        tel.sdf_chunk_streams_sent,
        tel.sdf_bricks_converged,
        tel.wal_committed_acks,
        tel.max_wal_flush_us,
        tel.mid_combat_epoch_drift,
        tel.reclaimed_epochs,
        tel.legacy_preserved_count,
        tel.season_weapons_used,
        tel.trait_casts,
        tel.augment_casts,
        tel.world_event_casts
    );

    if load_season_gameplay_pack(
        Path::new("assets/manifests/E_1.ron"),
        Path::new("gameplay"),
    )
    .is_ok()
    {
        let p3 = run_phase3_season_synergy_boss_and_rollover_drill()?;
        println!(
            "PHASE3_SEASON_OK qvm_modules={} e1_weapons={} e2_weapons={} e1_chunks={} e2_chunks={} e1_spread_half={} e2_spread_half={} prism_trait_gain={} prism_aug_gain={} bosses_ok={} s2_power={} legacy_preserved={}",
            p3.total_qvm_modules,
            p3.e1_weapons,
            p3.e2_weapons,
            p3.e1_chunks,
            p3.e2_chunks,
            p3.e1_spread_half,
            p3.e2_spread_half,
            p3.prism_damage_delta,
            p3.prism_augment_delta,
            p3.bosses_verified,
            p3.s2_post_rollover_power,
            p3.s2_preserved_legacy
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewind_150ms_horizon_ring_slot_separation_and_channel_split() {
        let mut ring = ServerSoARingBuffer64::new(99);
        let arena = CombatArena::new_boxed(99);
        let zones = vec![ZoneInstanceKind::OpenWorldField; 4];
        let channels = vec![0_u16; 8];
        let hold = Packet::<Unverified>::new_unverified(RawWireInput {
            tick: 11,
            move_dir_milli: [300, 0, 0],
            aim_yaw_turn_milli: 0,
            aim_pitch_turn_milli: 0,
            action_mask: 0,
            active_weapon_slot: 1,
        })
        .verify_bounds()
        .map(Packet::into_input_frame);
        assert!(hold.is_ok());
        if let Ok(frame11) = hold {
            // Pre-buffer future input at tick 11, then record post-tick-10 state at slot 11:
            assert!(ring.set_recorded_input(11, EntityId(0, 1), frame11));
            ring.record_state_at_tick(11, &arena, &zones, &channels);
            // Prove `record_state_at_tick(11)` did NOT overwrite `inputs_per_tick[slot(11)]` (`CE-5`)!
            assert_eq!(ring.recorded_input(11, EntityId(0, 1)), Some(frame11));
        }

        ring.record_state_at_tick(10, &arena, &zones, &channels);
        let mut out_arena = CombatArena::new_boxed(1);
        let mut out_zones = Vec::new();
        let mut out_ch = Vec::new();
        // 9 ticks @ 60Hz = 150.0ms -> allowed:
        assert!(ring.restore_for_rewind(
            19,
            10,
            &mut out_arena,
            &mut out_zones,
            &mut out_ch
        ));
        assert_eq!(out_zones.len(), 4);
        // 10 ticks @ 60Hz = 166.7ms > 150.0ms -> rejected:
        assert!(!ring.restore_for_rewind(
            20,
            10,
            &mut out_arena,
            &mut out_zones,
            &mut out_ch
        ));
        // Pre-overflow elastic split before 200,000 shard fuel cap:
        assert!(!AoiSpatialGrid::should_induce_channel_split(
            120_000,
            SERVER_SHARD_TICK_FUEL_CAP
        ));
        assert!(AoiSpatialGrid::should_induce_channel_split(
            162_078,
            SERVER_SHARD_TICK_FUEL_CAP
        ));
    }

    #[test]
    fn test_phase3_gameplay_pack_wl_novelty_and_tournaments() {
        let (e1_p, e2_p, gp_p) = if load_season_gameplay_pack(
            Path::new("assets/manifests/E_1.ron"),
            Path::new("gameplay"),
        )
        .is_ok()
        {
            (
                Path::new("assets/manifests/E_1.ron"),
                Path::new("assets/manifests/E_2.ron"),
                Path::new("gameplay"),
            )
        } else {
            (
                Path::new("../../assets/manifests/E_1.ron"),
                Path::new("../../assets/manifests/E_2.ron"),
                Path::new("../../gameplay"),
            )
        };
        let pack1 = load_season_gameplay_pack(e1_p, gp_p);
        let pack2 = load_season_gameplay_pack(e2_p, gp_p);
        assert!(pack1.is_ok());
        assert!(pack2.is_ok());
        if let (Ok(p1), Ok(p2)) = (pack1, pack2) {
            assert!(p1.all_modules_rel.len() >= 30);
            let mut phis = Vec::new();
            for src in p1.all_modules_rel.values() {
                let compiled = compile_qvm_source(&format!("{}\n{src}", p1.prelude_src));
                assert!(compiled.is_ok());
                if let Ok(m) = compiled {
                    phis.push(wl_phi_exact(&m.causal_graph));
                }
            }
            let mut i: usize = 0;
            while i < phis.len() {
                let mut j = i.saturating_add(1);
                while j < phis.len() {
                    assert!(
                        wl_distance_ge_ratio_exact(&phis[i], &phis[j], 1, 5),
                        "G4 WL collision between module {i} and module {j}"
                    );
                    j = j.saturating_add(1);
                }
                i = i.saturating_add(1);
            }

            let mut w1 = Vec::new();
            for stem in &p1.manifest.weapon_modules {
                if let Some(body) = p1.weapons.get(stem)
                    && let Ok(m) = compile_qvm_source(&format!("{}\n{body}", p1.prelude_src))
                {
                    w1.push(m);
                }
            }
            let mut w2 = Vec::new();
            for stem in &p2.manifest.weapon_modules {
                if let Some(body) = p2.weapons.get(stem)
                    && let Ok(m) = compile_qvm_source(&format!("{}\n{body}", p2.prelude_src))
                {
                    w2.push(m);
                }
            }
            assert_eq!(verify_season_weapon_tournament(&w1).err(), None);
            assert_eq!(verify_season_weapon_tournament(&w2).err(), None);
        }
    }
}
