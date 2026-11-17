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

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;
use math_q32::{Q32, isqrt_u128, proper_time_step_schwarzschild, shannon_term_bits};
use serde::{Deserialize, Serialize};
use verifier_abi::{ContractViolation, SealedLedger, TimelineContract, Verified};

/// Reference corpus (`D_ref`) proper-noun ban list (`§5.2 G4`: zero lexical matches allowed).
pub const BANNED_DREF_TERMS: &[&str] = &[
    "데스노트",
    "야가미 라이토",
    "아야노코지",
    "시빌라",
    "스카이캐슬",
    "라모스카",
    "닥터 스트레인지",
    "스티븐 스트레인지",
    "도르마무",
    "아가모토",
    "타노스",
    "어벤져스",
    "김독자",
    "유중혁",
    "에드 로스테일러",
    "차현수",
    "오카베 린타로",
    "슈타인즈 게이트",
    "엘 프사이 콩그루",
    "에렌 예거",
    "리바이",
    "삼체",
    "물방울 탐사선",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerError {
    InsufficientReasoningDepth,
    PlotInducedSuboptimalAction,
    NoKillHypocrisyDetected,
    UncompensatedSacrificeDetected,
    TrollForcedLossDetected,
    BetrayerSpared,
    MissingHierarchyInversion,
    InsufficientEntropyAsymmetry,
    SafehouseBreached,
    NonZeroSumTransaction,
    BalanceOverflowOrNegative,
    ActorAlreadyDeadOrBankrupt,
    InvalidLifespanBurn,
    DuplicateVertexId,
    MissingEndpointVertex,
    CausalCycleDetected,
    RelativisticCausalityViolation,
    NovikovParadox { event_id: u32 },
    InsufficientForeshadowClues { payoff_id: u32 },
    MissingPriorEpisodeClue { payoff_id: u32 },
    SeasonFinaleOpenClueSinks { open_count: u32 },
    PlagiarizedReferenceTerm,
    InsufficientMechanismNovelty,
    InsufficientFiveAxisDiversity,
    MalformedTlvStream,
    AbiContract(ContractViolation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HierarchyInversion {
    PowerPlundered,
    HostileMnaBankrupted,
    LifespanConfiscated,
    LifeDeathSubjugated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionUtility {
    pub action_id: u32,
    pub expected_utility_q32: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyAgent {
    pub enemy_id: u32,
    pub reasoning_depth: u16,
    pub betrayal_weight_q32: i64,
    pub spared_or_forgiven: bool,
    pub hierarchy_inversion: Option<HierarchyInversion>,
}

/// Epistemic Kripke mind-game & asymmetric information state at decision node `d` (`§1.1`, `§1.2`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionNode {
    pub node_id: u32,
    pub protagonist_depth: u16,
    pub chosen_action_id: u32,
    pub candidate_actions: Vec<ActionUtility>,
    pub enemies: Vec<EnemyAgent>,
    pub defeated_apex_enemies: Vec<u32>,
    pub has_no_kill_hypocrisy: bool,
    pub has_uncompensated_sacrifice: bool,
    pub has_troll_forced_loss: bool,
    pub world_entropy_opp_q32: i64,
    pub world_entropy_protag_q32: i64,
    pub safehouse_breach_prob_q32: i64,
}

/// Computes Shannon entropy `H(X) = -sum_i p_i log2(p_i)` in `Q32.32` bits (`0 log2 0 := 0`).
pub fn compute_discrete_entropy_q32(counts: &[u64]) -> Q32 {
    let mut total: u64 = 0;
    for &c in counts {
        total = total.saturating_add(c);
    }
    if total == 0 {
        return Q32::ZERO;
    }
    let mut acc = Q32::ZERO;
    for &c in counts {
        acc = acc.sat_add(shannon_term_bits(c, total));
    }
    acc
}

/// Verifies `§1.1` epistemic dominance, optimal utility, merciless traitor handling, hierarchy inversion,
/// and `§1.2` `>= 1.0 bit` conditional entropy advantage + `0` safehouse breach probability.
pub fn verify_decision_node(node: &DecisionNode) -> Result<(), LedgerError> {
    if node.protagonist_depth < 2 {
        return Err(LedgerError::InsufficientReasoningDepth);
    }
    let mut max_enemy_depth: u16 = 0;
    for enemy in &node.enemies {
        if enemy.reasoning_depth > max_enemy_depth {
            max_enemy_depth = enemy.reasoning_depth;
        }
        if enemy.betrayal_weight_q32 > 0 && enemy.spared_or_forgiven {
            return Err(LedgerError::BetrayerSpared);
        }
    }
    let min_req_depth = max_enemy_depth.saturating_add(1);
    if node.protagonist_depth < min_req_depth {
        return Err(LedgerError::InsufficientReasoningDepth);
    }

    if node.has_no_kill_hypocrisy {
        return Err(LedgerError::NoKillHypocrisyDetected);
    }
    if node.has_uncompensated_sacrifice {
        return Err(LedgerError::UncompensatedSacrificeDetected);
    }
    if node.has_troll_forced_loss {
        return Err(LedgerError::TrollForcedLossDetected);
    }

    if node.candidate_actions.is_empty() {
        return Err(LedgerError::PlotInducedSuboptimalAction);
    }
    let mut max_u = i64::MIN;
    let mut chosen_u: Option<i64> = None;
    for act in &node.candidate_actions {
        if act.expected_utility_q32 > max_u {
            max_u = act.expected_utility_q32;
        }
        if act.action_id == node.chosen_action_id {
            chosen_u = Some(act.expected_utility_q32);
        }
    }
    match chosen_u {
        Some(u) if u == max_u => {}
        _ => return Err(LedgerError::PlotInducedSuboptimalAction),
    }

    for &defeated_id in &node.defeated_apex_enemies {
        let inverted = node
            .enemies
            .iter()
            .find(|e| e.enemy_id == defeated_id)
            .and_then(|e| e.hierarchy_inversion);
        if inverted.is_none() {
            return Err(LedgerError::MissingHierarchyInversion);
        }
    }

    let delta_h = node
        .world_entropy_opp_q32
        .saturating_sub(node.world_entropy_protag_q32);
    if delta_h < Q32::ONE.0 {
        return Err(LedgerError::InsufficientEntropyAsymmetry);
    }

    if node.safehouse_breach_prob_q32 != 0 {
        return Err(LedgerError::SafehouseBreached);
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AssetKind {
    Capital(u16),
    LifespanTicks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActorStatus {
    Active = 0,
    DeadOrBankrupt = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonopolyContract {
    pub protagonist_id: u32,
    pub recipient_id: u32,
    pub absorption_bps: u16,
}

/// Pure stateless 2-party `64-bit` zero-sum lifespan step check matching `§1.2` and `§8.1` CDCL proof target.
pub fn verify_two_party_lifespan_step(
    before: (i64, i64),
    dt: (i64, i64),
    core_burn: (i64, i64),
    tx: (i64, i64),
    after: (i64, i64),
) -> bool {
    let (b0, b1) = before;
    let (dt0, dt1) = dt;
    let (core0, core1) = core_burn;
    let (tx0, tx1) = tx;
    let (b0_next, b1_next) = after;
    if b0 < 0 || b1 < 0 || dt0 < 0 || dt1 < 0 || core0 < 0 || core1 < 0 {
        return false;
    }
    let tx_sum = (tx0 as i128).saturating_add(tx1 as i128);
    if tx_sum != 0 {
        return false;
    }
    let exp0 = (b0 as i128)
        .saturating_sub(dt0 as i128)
        .saturating_sub(core0 as i128)
        .saturating_add(tx0 as i128);
    let exp1 = (b1 as i128)
        .saturating_sub(dt1 as i128)
        .saturating_sub(core1 as i128)
        .saturating_add(tx1 as i128);
    if exp0 < 0 || exp0 > (i64::MAX as i128) || exp1 < 0 || exp1 > (i64::MAX as i128) {
        return false;
    }
    (exp0 as i64) == b0_next && (exp1 as i64) == b1_next
}

/// Deterministic zero-sum capital & biological lifespan ledger (`§1.2`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldLedger {
    balances: BTreeMap<(u32, AssetKind), i64>,
    statuses: BTreeMap<u32, ActorStatus>,
    monopoly_contracts: BTreeMap<u32, MonopolyContract>,
}

impl WorldLedger {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Genesis allocation (only allowed before actors exist; rejects negative initial balances).
    pub fn register_actor(
        &mut self,
        actor_id: u32,
        initial_capital: i64,
        initial_lifespan_ticks: i64,
    ) -> Result<(), LedgerError> {
        if initial_capital < 0 || initial_lifespan_ticks <= 0 {
            return Err(LedgerError::BalanceOverflowOrNegative);
        }
        self.balances
            .insert((actor_id, AssetKind::Capital(0)), initial_capital);
        self.balances.insert(
            (actor_id, AssetKind::LifespanTicks),
            initial_lifespan_ticks,
        );
        self.statuses.insert(actor_id, ActorStatus::Active);
        Ok(())
    }

    pub fn register_monopoly_contract(
        &mut self,
        contract: MonopolyContract,
    ) -> Result<(), LedgerError> {
        if contract.absorption_bps > 10_000 {
            return Err(LedgerError::BalanceOverflowOrNegative);
        }
        self.monopoly_contracts
            .insert(contract.recipient_id, contract);
        Ok(())
    }

    #[inline]
    pub fn balance(&self, actor_id: u32, kind: AssetKind) -> i64 {
        self.balances.get(&(actor_id, kind)).copied().unwrap_or(0)
    }

    #[inline]
    pub fn status(&self, actor_id: u32) -> ActorStatus {
        self.statuses
            .get(&actor_id)
            .copied()
            .unwrap_or(ActorStatus::DeadOrBankrupt)
    }

    /// Executes an atomic zero-sum `64-bit` transfer (`\sum_u \Delta B_{u,a}^{tx} == 0`).
    /// If a recipient is bound by an asymmetric `MonopolyContract`, absorbed share routes to `protagonist_id`.
    pub fn apply_zero_sum_transfer(
        &mut self,
        kind: AssetKind,
        raw_deltas: &[(u32, i64)],
    ) -> Result<(), LedgerError> {
        let mut net_deltas: BTreeMap<u32, i128> = BTreeMap::new();
        let mut sum_tx: i128 = 0;

        for &(actor_id, delta) in raw_deltas {
            if self.status(actor_id) != ActorStatus::Active {
                return Err(LedgerError::ActorAlreadyDeadOrBankrupt);
            }
            sum_tx = sum_tx.saturating_add(delta as i128);
            if delta > 0
                && let Some(c) = self.monopoly_contracts.get(&actor_id)
            {
                let absorbed = (delta as i128)
                    .saturating_mul(c.absorption_bps as i128)
                    .checked_div(10_000)
                    .unwrap_or(0);
                let remain = (delta as i128).saturating_sub(absorbed);
                let entry_rec = net_deltas.entry(actor_id).or_insert(0);
                *entry_rec = entry_rec.saturating_add(remain);
                let entry_p0 = net_deltas.entry(c.protagonist_id).or_insert(0);
                *entry_p0 = entry_p0.saturating_add(absorbed);
                continue;
            }
            let entry = net_deltas.entry(actor_id).or_insert(0);
            *entry = entry.saturating_add(delta as i128);
        }

        if sum_tx != 0 {
            return Err(LedgerError::NonZeroSumTransaction);
        }

        let mut staged: BTreeMap<(u32, AssetKind), i64> = BTreeMap::new();
        for (&actor_id, &delta_wide) in &net_deltas {
            if self.status(actor_id) != ActorStatus::Active {
                return Err(LedgerError::ActorAlreadyDeadOrBankrupt);
            }
            let cur = self.balance(actor_id, kind) as i128;
            let next = cur.saturating_add(delta_wide);
            if next < 0 || next > (i64::MAX as i128) {
                return Err(LedgerError::BalanceOverflowOrNegative);
            }
            staged.insert((actor_id, kind), next as i64);
        }

        for ((actor_id, k), next_bal) in staged {
            self.balances.insert((actor_id, k), next_bal);
            if k == AssetKind::LifespanTicks && next_bal == 0 {
                self.statuses.insert(actor_id, ActorStatus::DeadOrBankrupt);
            }
        }
        Ok(())
    }

    /// Applies biological proper-time aging, relic/core lifespan fuel burn, and zero-sum lifespan transfers:
    /// `dB_{u,\tau} = -d\tau_u - \Delta\tau_{core} + \Delta B_{u,\tau}^{tx}`, transitioning `B_{u,\tau} == 0` to `DeadOrBankrupt`.
    pub fn step_lifespan(
        &mut self,
        entries: &[(u32, i64, i64, i64)],
    ) -> Result<(), LedgerError> {
        let mut tx_sum: i128 = 0;
        let mut staged: Vec<(u32, i64)> = Vec::with_capacity(entries.len());

        for &(actor_id, d_tau, core_burn, tx_delta) in entries {
            if self.status(actor_id) != ActorStatus::Active {
                return Err(LedgerError::ActorAlreadyDeadOrBankrupt);
            }
            if d_tau < 0 || core_burn < 0 {
                return Err(LedgerError::InvalidLifespanBurn);
            }
            tx_sum = tx_sum.saturating_add(tx_delta as i128);
            let cur = self.balance(actor_id, AssetKind::LifespanTicks) as i128;
            let next = cur
                .saturating_sub(d_tau as i128)
                .saturating_sub(core_burn as i128)
                .saturating_add(tx_delta as i128);
            if next < 0 || next > (i64::MAX as i128) {
                return Err(LedgerError::BalanceOverflowOrNegative);
            }
            staged.push((actor_id, next as i64));
        }

        if tx_sum != 0 {
            return Err(LedgerError::NonZeroSumTransaction);
        }

        for (actor_id, next_bal) in staged {
            self.balances
                .insert((actor_id, AssetKind::LifespanTicks), next_bal);
            if next_bal == 0 {
                self.statuses.insert(actor_id, ActorStatus::DeadOrBankrupt);
            }
        }
        Ok(())
    }

    #[inline]
    pub fn capital_snapshot(&self) -> BTreeMap<u32, i64> {
        let mut out = BTreeMap::new();
        for (&(uid, kind), &bal) in &self.balances {
            if kind == AssetKind::Capital(0) {
                out.insert(uid, bal);
            }
        }
        out
    }

    #[inline]
    pub fn lifespan_snapshot(&self) -> BTreeMap<u32, i64> {
        let mut out = BTreeMap::new();
        for (&(uid, kind), &bal) in &self.balances {
            if kind == AssetKind::LifespanTicks {
                out.insert(uid, bal);
            }
        }
        out
    }

    #[inline]
    pub fn status_snapshot(&self) -> BTreeMap<u32, ActorStatus> {
        self.statuses.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Standard,
    Clue,
    Payoff,
    LoopAnchor,
    WallfacerCommit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicRelation {
    Copy,
    Invert,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelativisticStep {
    pub r_q32: i64,
    pub r_schwarzschild_q32: i64,
    pub dt_q32: i64,
    pub dr_q32: i64,
    pub v_tan_q32: i64,
    pub claimed_dtau_q32: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacetimeEvent {
    pub event_id: u32,
    pub episode: u16,
    pub kind: EventKind,
    pub state_bit: bool,
    pub rel_step: Option<RelativisticStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpacetimeEdge {
    pub from: u32,
    pub to: u32,
    pub relation: LogicRelation,
    pub is_retrocausal: bool,
}

/// Pure finite-lattice Novikov fixed-point checker (`§1.3` & `§8.1` CDCL soundness target).
/// Returns `true` iff every causal & retrocausal edge constraint is self-consistent on `state_bits`.
pub fn check_novikov_lattice(state_bits: &[bool], edges: &[(usize, usize, LogicRelation)]) -> bool {
    for &(u, v, rel) in edges {
        let Some(&su) = state_bits.get(u) else {
            return false;
        };
        let Some(&sv) = state_bits.get(v) else {
            return false;
        };
        let expected = match rel {
            LogicRelation::Copy => su,
            LogicRelation::Invert => !su,
        };
        if sv != expected {
            return false;
        }
    }
    true
}

/// Verifies `§1.3` spacetime event graph $G_{st} = (V, E_{causal} \sqcup E_{retro})$:
/// 1. $E_{causal}$ forms a strict acyclic DAG.
/// 2. Every `RelativisticStep` obeys Schwarzschild proper-time & light-cone causality (`ds^2 <= 0`).
/// 3. Novikov self-consistency fixed-point $\Phi(\mathcal{H}^*) = \mathcal{H}^*$ holds across all causal & retrocausal edges (`0` paradoxes).
/// 4. Every `Payoff` vertex has `>= 2` `Clue` ancestors at causal DAG distance `d_dag(u, v) >= 4`, with `>= 1` from a prior episode (`u.episode < v.episode`).
/// 5. At `is_season_finale` ($E_{12}$), open clue sinks (`deg^+(v) == 0`) equal `0`, yielding `TimelineContract<Verified>`.
pub fn verify_spacetime_graph(
    episode_index: u16,
    is_season_finale: bool,
    events: &[SpacetimeEvent],
    edges: &[SpacetimeEdge],
) -> Result<TimelineContract<Verified>, LedgerError> {
    let mut id_to_idx: BTreeMap<u32, usize> = BTreeMap::new();
    for (idx, ev) in events.iter().enumerate() {
        if id_to_idx.insert(ev.event_id, idx).is_some() {
            return Err(LedgerError::DuplicateVertexId);
        }
        if let Some(rel) = ev.rel_step {
            let (dtau, causal) = proper_time_step_schwarzschild(
                Q32(rel.r_q32),
                Q32(rel.r_schwarzschild_q32),
                Q32(rel.dt_q32),
                Q32(rel.dr_q32),
                Q32(rel.v_tan_q32),
            );
            if !causal {
                return Err(LedgerError::RelativisticCausalityViolation);
            }
            let diff = dtau.sat_sub(Q32(rel.claimed_dtau_q32)).sat_abs();
            // Tolerance <= 16 LSB in Q32.32
            if diff.0 > 16 {
                return Err(LedgerError::RelativisticCausalityViolation);
            }
        }
    }

    let n = events.len();
    let mut causal_adj: Vec<Vec<usize>> = alloc::vec![Vec::new(); n];
    let mut indegree: Vec<usize> = alloc::vec![0; n];
    let mut total_outdeg: Vec<usize> = alloc::vec![0; n];
    let mut lattice_edges: Vec<(usize, usize, LogicRelation)> = Vec::with_capacity(edges.len());

    for edge in edges {
        let Some(&u) = id_to_idx.get(&edge.from) else {
            return Err(LedgerError::MissingEndpointVertex);
        };
        let Some(&v) = id_to_idx.get(&edge.to) else {
            return Err(LedgerError::MissingEndpointVertex);
        };
        if let Some(od) = total_outdeg.get_mut(u) {
            *od = od.saturating_add(1);
        }
        if !edge.is_retrocausal {
            if let Some(adj_u) = causal_adj.get_mut(u) {
                adj_u.push(v);
            }
            if let Some(ind_v) = indegree.get_mut(v) {
                *ind_v = ind_v.saturating_add(1);
            }
        }
        lattice_edges.push((u, v, edge.relation));
    }

    // 1. Kahn topological sort on E_causal to reject causal cycles.
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &deg) in indegree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }
    let mut visited: usize = 0;
    while let Some(u) = queue.pop_front() {
        visited = visited.saturating_add(1);
        if let Some(neighbors) = causal_adj.get(u) {
            for &v in neighbors {
                if let Some(ind_v) = indegree.get_mut(v) {
                    *ind_v = ind_v.saturating_sub(1);
                    if *ind_v == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
    }
    if visited != n {
        return Err(LedgerError::CausalCycleDetected);
    }

    // 2. Novikov self-consistency fixed-point check on E_causal \sqcup E_retro.
    let state_bits: Vec<bool> = events.iter().map(|e| e.state_bit).collect();
    for &(u, v, rel) in &lattice_edges {
        let su = state_bits.get(u).copied().unwrap_or(false);
        let sv = state_bits.get(v).copied().unwrap_or(false);
        let expected = match rel {
            LogicRelation::Copy => su,
            LogicRelation::Invert => !su,
        };
        if sv != expected {
            let bad_id = events.get(v).map_or(0, |e| e.event_id);
            return Err(LedgerError::NovikovParadox { event_id: bad_id });
        }
    }

    // 3. Shortest causal DAG distance d_dag(u, v) >= 4 from Clue vertices to Payoff vertices.
    let mut clue_indices: Vec<usize> = Vec::new();
    for (idx, ev) in events.iter().enumerate() {
        if ev.kind == EventKind::Clue {
            clue_indices.push(idx);
        }
    }

    // Precompute shortest directed distances from each Clue vertex via BFS on unweighted E_causal.
    let mut clue_dists: Vec<(usize, Vec<Option<u32>>)> = Vec::with_capacity(clue_indices.len());
    for &c_idx in &clue_indices {
        let mut dist: Vec<Option<u32>> = alloc::vec![None; n];
        if let Some(d0) = dist.get_mut(c_idx) {
            *d0 = Some(0);
        }
        let mut bfs: VecDeque<usize> = VecDeque::new();
        bfs.push_back(c_idx);
        while let Some(cur) = bfs.pop_front() {
            let cur_d = dist.get(cur).and_then(|&x| x).unwrap_or(0);
            let next_d = cur_d.saturating_add(1);
            if let Some(neighbors) = causal_adj.get(cur) {
                for &nxt in neighbors {
                    if let Some(slot) = dist.get_mut(nxt)
                        && slot.is_none()
                    {
                        *slot = Some(next_d);
                        bfs.push_back(nxt);
                    }
                }
            }
        }
        clue_dists.push((c_idx, dist));
    }

    for (p_idx, ev) in events.iter().enumerate() {
        if ev.kind == EventKind::Payoff {
            let mut valid_clues: u32 = 0;
            let mut has_prior_ep = false;
            for (c_idx, dist_map) in &clue_dists {
                if let Some(Some(d)) = dist_map.get(p_idx)
                    && *d >= 4
                {
                    valid_clues = valid_clues.saturating_add(1);
                    if let Some(clue_ev) = events.get(*c_idx)
                        && clue_ev.episode < ev.episode
                    {
                        has_prior_ep = true;
                    }
                }
            }
            if valid_clues < 2 {
                return Err(LedgerError::InsufficientForeshadowClues {
                    payoff_id: ev.event_id,
                });
            }
            if !has_prior_ep {
                return Err(LedgerError::MissingPriorEpisodeClue {
                    payoff_id: ev.event_id,
                });
            }
        }
    }

    // 4. Open clue sinks count.
    let mut open_sinks: u32 = 0;
    for &c_idx in &clue_indices {
        let out_d = total_outdeg.get(c_idx).copied().unwrap_or(0);
        if out_d == 0 {
            open_sinks = open_sinks.saturating_add(1);
        }
    }

    if is_season_finale && open_sinks != 0 {
        return Err(LedgerError::SeasonFinaleOpenClueSinks {
            open_count: open_sinks,
        });
    }

    TimelineContract::new_unverified(episode_index, 0, open_sinks, is_season_finale)
        .verify()
        .map_err(LedgerError::AbiContract)
}

/// Deterministic `BLAKE3` helper for canonical `K_sim` state hashing (`§4.3`, `§9.1`).
#[inline]
pub fn hash_canonical_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Checks `§5.2 G4` zero lexical overlap with `D_ref` proper nouns.
pub fn verify_zero_dref_plagiarism(script_or_subtitle_text: &str) -> Result<(), LedgerError> {
    for &term in BANNED_DREF_TERMS {
        if script_or_subtitle_text.contains(term) {
            return Err(LedgerError::PlagiarizedReferenceTerm);
        }
    }
    Ok(())
}

/// Anonymous mechanism roles for `3-iter` Weisfeiler-Lehman novelty & synergy graph metrics (`§1.2`, `§5.2 G4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum MechanismRole {
    MindTrap = 1,
    HierarchyInvert = 2,
    MonopolyContract = 3,
    LifespanTx = 4,
    MythicSynergy = 5,
    SafehouseNode = 6,
    CausalClue = 7,
    RetroLoop = 8,
    PayoffConvergence = 9,
    MirrorFold = 10,
    PortalCut = 11,
    CoreSwap = 12,
    PhonkSync = 13,
    VocalFormant = 14,
    VacuumGate = 15,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismGraph {
    pub nodes: Vec<MechanismRole>,
    pub edges: Vec<(u32, u32)>,
}

/// Computes deterministic integer-count `3-iter` Weisfeiler-Lehman feature vector `\phi(x)` (`§5.2 G4`).
pub fn wl_3iter_features(graph: &MechanismGraph) -> BTreeMap<u64, i64> {
    let n = graph.nodes.len();
    let mut counts: BTreeMap<u64, i64> = BTreeMap::new();
    if n == 0 {
        return counts;
    }

    let mut adj: Vec<BTreeSet<usize>> = alloc::vec![BTreeSet::new(); n];
    for &(u, v) in &graph.edges {
        let ui = u as usize;
        let vi = v as usize;
        if ui < n && vi < n {
            if let Some(su) = adj.get_mut(ui) {
                su.insert(vi);
            }
            if let Some(sv) = adj.get_mut(vi) {
                sv.insert(ui);
            }
        }
    }

    let mut colors: Vec<u64> = graph.nodes.iter().map(|&r| r as u64).collect();
    for &c in &colors {
        let entry = counts.entry(c).or_insert(0);
        *entry = entry.saturating_add(1);
    }

    let mut iter_idx: u8 = 0;
    while iter_idx < 3 {
        let mut next_colors: Vec<u64> = Vec::with_capacity(n);
        for i in 0..n {
            let self_c = colors.get(i).copied().unwrap_or(0);
            let mut neigh_c: Vec<u64> = Vec::new();
            if let Some(ns) = adj.get(i) {
                for &nb in ns {
                    neigh_c.push(colors.get(nb).copied().unwrap_or(0));
                }
            }
            neigh_c.sort_unstable();
            let mut hasher = blake3::Hasher::new();
            hasher.update(&[iter_idx.saturating_add(1)]);
            hasher.update(&self_c.to_le_bytes());
            for nc in neigh_c {
                hasher.update(&nc.to_le_bytes());
            }
            let digest = hasher.finalize();
            let bytes = digest.as_bytes();
            let mut raw = [0_u8; 8];
            raw.copy_from_slice(&bytes[..8]);
            let next_c = u64::from_le_bytes(raw);
            next_colors.push(next_c);
            let entry = counts.entry(next_c).or_insert(0);
            *entry = entry.saturating_add(1);
        }
        colors = next_colors;
        iter_idx = iter_idx.saturating_add(1);
    }
    counts
}

/// Computes `d_mech(x, y) in [0, 1]` (`Q32.32`) per `§5.2 G4`:
/// `\phi(x) == \phi(y) => 0`, else `1 - <\phi(x), \phi(y)> / max(||\phi(x)||_2 * ||\phi(y)||_2, 1)`.
pub fn mechanism_distance_q32(a: &MechanismGraph, b: &MechanismGraph) -> Q32 {
    let fa = wl_3iter_features(a);
    let fb = wl_3iter_features(b);
    if fa == fb {
        return Q32::ZERO;
    }

    let mut dot: u128 = 0;
    let mut norm_a2: u128 = 0;
    let mut norm_b2: u128 = 0;

    for (&k, &va) in &fa {
        let ua = va.unsigned_abs() as u128;
        norm_a2 = norm_a2.saturating_add(ua.saturating_mul(ua));
        if let Some(&vb) = fb.get(&k) {
            let ub = vb.unsigned_abs() as u128;
            dot = dot.saturating_add(ua.saturating_mul(ub));
        }
    }
    for &vb in fb.values() {
        let ub = vb.unsigned_abs() as u128;
        norm_b2 = norm_b2.saturating_add(ub.saturating_mul(ub));
    }

    // Compute ||a||_2 * ||b||_2 = sqrt(norm_a2 * norm_b2) using Q32 precision (`<< 64` inside sqrt -> `<< 32`).
    let prod_sq = norm_a2.saturating_mul(norm_b2);
    if prod_sq == 0 {
        return Q32::ONE;
    }
    // Scale numerator and denominator safely without u128 overflow:
    // cos_q32 = (dot << 32) / isqrt(prod_sq), using high-precision shift when prod_sq fits.
    let denom_scaled = if prod_sq <= (u128::MAX >> 64) {
        isqrt_u128(prod_sq.wrapping_shl(64))
    } else {
        isqrt_u128(prod_sq).wrapping_shl(32)
    };
    let denom = if denom_scaled == 0 { 1_u128 } else { denom_scaled };
    let dot_q64 = dot.wrapping_shl(64);
    let cos_q32 = dot_q64.checked_div(denom).unwrap_or(0);
    let clamped_cos = if cos_q32 > (Q32::ONE.0 as u128) {
        Q32::ONE
    } else {
        Q32(cos_q32 as i64)
    };
    Q32::ONE.sat_sub(clamped_cos)
}

/// Computes `exp(-sum_{i=1}^5 q_i ln q_i)` in `Q32.32` (`§5.2 G4`: must be `>= 4.0`).
pub fn five_axis_diversity_q32(axis_counts: [u64; 5]) -> Q32 {
    let h_bits = compute_discrete_entropy_q32(&axis_counts);
    // exp(-sum q_i ln q_i) = 2^(H_bits)
    h_bits.exp2()
}

/// Additive `u16 TLV` episode manifest (`§4.3`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeManifest {
    pub episode_id: u16,
    pub worldline_id: u64,
    pub capital_balances: BTreeMap<u32, i64>,
    pub lifespan_balances: BTreeMap<u32, i64>,
    pub actor_statuses: BTreeMap<u32, ActorStatus>,
    pub planted_clues: Vec<u32>,
    pub resolved_clues: Vec<u32>,
    pub mkv_blake3: [u8; 32],
    pub unknown_tlvs: BTreeMap<u16, Vec<u8>>,
}

const TLV_EPISODE_ID: u16 = 1;
const TLV_WORLDLINE_ID: u16 = 2;
const TLV_CAPITAL_MAP: u16 = 3;
const TLV_LIFESPAN_MAP: u16 = 4;
const TLV_STATUS_MAP: u16 = 5;
const TLV_PLANTED_CLUES: u16 = 6;
const TLV_RESOLVED_CLUES: u16 = 7;
const TLV_MKV_BLAKE3: u16 = 8;

fn push_tlv(out: &mut Vec<u8>, tag: u16, payload: &[u8]) {
    out.extend_from_slice(&tag.to_le_bytes());
    let len = payload.len() as u32;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
}

fn encode_u32_i64_map(map: &BTreeMap<u32, i64>) -> Vec<u8> {
    let mut buf = Vec::with_capacity(map.len().saturating_mul(12));
    for (&k, &v) in map {
        buf.extend_from_slice(&k.to_le_bytes());
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

fn decode_u32_i64_map(bytes: &[u8]) -> Result<BTreeMap<u32, i64>, LedgerError> {
    if bytes.len().checked_rem(12) != Some(0) {
        return Err(LedgerError::MalformedTlvStream);
    }
    let mut map = BTreeMap::new();
    let mut offset: usize = 0;
    while offset < bytes.len() {
        let end_k = offset.saturating_add(4);
        let end_v = offset.saturating_add(12);
        let Some(ks) = bytes.get(offset..end_k) else {
            return Err(LedgerError::MalformedTlvStream);
        };
        let Some(vs) = bytes.get(end_k..end_v) else {
            return Err(LedgerError::MalformedTlvStream);
        };
        let mut kb = [0_u8; 4];
        kb.copy_from_slice(ks);
        let mut vb = [0_u8; 8];
        vb.copy_from_slice(vs);
        map.insert(u32::from_le_bytes(kb), i64::from_le_bytes(vb));
        offset = end_v;
    }
    Ok(map)
}

fn encode_u32_list(list: &[u32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(list.len().saturating_mul(4));
    for &id in list {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    buf
}

fn decode_u32_list(bytes: &[u8]) -> Result<Vec<u32>, LedgerError> {
    if bytes.len().checked_rem(4) != Some(0) {
        return Err(LedgerError::MalformedTlvStream);
    }
    let mut out = Vec::with_capacity(bytes.len().wrapping_shr(2));
    let mut offset: usize = 0;
    while offset < bytes.len() {
        let end = offset.saturating_add(4);
        let Some(slice) = bytes.get(offset..end) else {
            return Err(LedgerError::MalformedTlvStream);
        };
        let mut b = [0_u8; 4];
        b.copy_from_slice(slice);
        out.push(u32::from_le_bytes(b));
        offset = end;
    }
    Ok(out)
}

impl EpisodeManifest {
    /// Canonical deterministic `u16 TLV` serialization (`§4.3`).
    pub fn encode_tlv(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_tlv(&mut out, TLV_EPISODE_ID, &self.episode_id.to_le_bytes());
        push_tlv(&mut out, TLV_WORLDLINE_ID, &self.worldline_id.to_le_bytes());
        push_tlv(
            &mut out,
            TLV_CAPITAL_MAP,
            &encode_u32_i64_map(&self.capital_balances),
        );
        push_tlv(
            &mut out,
            TLV_LIFESPAN_MAP,
            &encode_u32_i64_map(&self.lifespan_balances),
        );

        let mut status_buf = Vec::with_capacity(self.actor_statuses.len().saturating_mul(5));
        for (&uid, &st) in &self.actor_statuses {
            status_buf.extend_from_slice(&uid.to_le_bytes());
            status_buf.push(st as u8);
        }
        push_tlv(&mut out, TLV_STATUS_MAP, &status_buf);
        push_tlv(
            &mut out,
            TLV_PLANTED_CLUES,
            &encode_u32_list(&self.planted_clues),
        );
        push_tlv(
            &mut out,
            TLV_RESOLVED_CLUES,
            &encode_u32_list(&self.resolved_clues),
        );
        push_tlv(&mut out, TLV_MKV_BLAKE3, &self.mkv_blake3);

        for (&tag, payload) in &self.unknown_tlvs {
            push_tlv(&mut out, tag, payload);
        }
        out
    }

    /// Additive `u16 TLV` deserialization preserving unknown future tags without data loss.
    pub fn decode_tlv(bytes: &[u8]) -> Result<Self, LedgerError> {
        let mut manifest = Self::default();
        let mut offset: usize = 0;
        while offset < bytes.len() {
            let hdr_end = offset.saturating_add(6);
            let Some(hdr) = bytes.get(offset..hdr_end) else {
                return Err(LedgerError::MalformedTlvStream);
            };
            let tag = u16::from_le_bytes([hdr[0], hdr[1]]);
            let len = u32::from_le_bytes([hdr[2], hdr[3], hdr[4], hdr[5]]) as usize;
            let pay_end = hdr_end.saturating_add(len);
            let Some(payload) = bytes.get(hdr_end..pay_end) else {
                return Err(LedgerError::MalformedTlvStream);
            };
            match tag {
                TLV_EPISODE_ID => {
                    if payload.len() != 2 {
                        return Err(LedgerError::MalformedTlvStream);
                    }
                    manifest.episode_id = u16::from_le_bytes([payload[0], payload[1]]);
                }
                TLV_WORLDLINE_ID => {
                    if payload.len() != 8 {
                        return Err(LedgerError::MalformedTlvStream);
                    }
                    let mut b = [0_u8; 8];
                    b.copy_from_slice(payload);
                    manifest.worldline_id = u64::from_le_bytes(b);
                }
                TLV_CAPITAL_MAP => {
                    manifest.capital_balances = decode_u32_i64_map(payload)?;
                }
                TLV_LIFESPAN_MAP => {
                    manifest.lifespan_balances = decode_u32_i64_map(payload)?;
                }
                TLV_STATUS_MAP => {
                    if payload.len().checked_rem(5) != Some(0) {
                        return Err(LedgerError::MalformedTlvStream);
                    }
                    let mut pos: usize = 0;
                    while pos < payload.len() {
                        let end = pos.saturating_add(5);
                        let Some(item) = payload.get(pos..end) else {
                            return Err(LedgerError::MalformedTlvStream);
                        };
                        let uid = u32::from_le_bytes([item[0], item[1], item[2], item[3]]);
                        let st = match item[4] {
                            0 => ActorStatus::Active,
                            1 => ActorStatus::DeadOrBankrupt,
                            _ => return Err(LedgerError::MalformedTlvStream),
                        };
                        manifest.actor_statuses.insert(uid, st);
                        pos = end;
                    }
                }
                TLV_PLANTED_CLUES => {
                    manifest.planted_clues = decode_u32_list(payload)?;
                }
                TLV_RESOLVED_CLUES => {
                    manifest.resolved_clues = decode_u32_list(payload)?;
                }
                TLV_MKV_BLAKE3 => {
                    if payload.len() != 32 {
                        return Err(LedgerError::MalformedTlvStream);
                    }
                    manifest.mkv_blake3.copy_from_slice(payload);
                }
                other => {
                    manifest.unknown_tlvs.insert(other, payload.to_vec());
                }
            }
            offset = pay_end;
        }
        Ok(manifest)
    }

    #[inline]
    pub fn blake3_digest(&self) -> [u8; 32] {
        let encoded = self.encode_tlv();
        *blake3::hash(&encoded).as_bytes()
    }

    #[inline]
    pub fn seal(self) -> SealedLedger<Self> {
        let digest = self.blake3_digest();
        SealedLedger::seal(self, digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mindgame_and_zero_sum_ledger() {
        let node = DecisionNode {
            node_id: 1,
            protagonist_depth: 4,
            chosen_action_id: 2,
            candidate_actions: alloc::vec![
                ActionUtility {
                    action_id: 1,
                    expected_utility_q32: Q32::from_int(10).0,
                },
                ActionUtility {
                    action_id: 2,
                    expected_utility_q32: Q32::from_int(25).0,
                },
            ],
            enemies: alloc::vec![EnemyAgent {
                enemy_id: 99,
                reasoning_depth: 3,
                betrayal_weight_q32: Q32::ONE.0,
                spared_or_forgiven: false,
                hierarchy_inversion: Some(HierarchyInversion::HostileMnaBankrupted),
            }],
            defeated_apex_enemies: alloc::vec![99],
            has_no_kill_hypocrisy: false,
            has_uncompensated_sacrifice: false,
            has_troll_forced_loss: false,
            world_entropy_opp_q32: Q32::from_int(3).0,
            world_entropy_protag_q32: Q32::ONE.0,
            safehouse_breach_prob_q32: 0,
        };
        assert_eq!(verify_decision_node(&node), Ok(()));

        let mut bad = node.clone();
        bad.enemies[0].spared_or_forgiven = true;
        assert_eq!(verify_decision_node(&bad), Err(LedgerError::BetrayerSpared));

        let mut ledger = WorldLedger::new();
        assert_eq!(ledger.register_actor(0, 1_000, 500), Ok(()));
        assert_eq!(ledger.register_actor(1, 1_000, 100), Ok(()));
        assert_eq!(
            ledger.apply_zero_sum_transfer(AssetKind::Capital(0), &[(0, 400), (1, -400)]),
            Ok(())
        );
        assert_eq!(
            ledger.apply_zero_sum_transfer(AssetKind::Capital(0), &[(0, 400), (1, -300)]),
            Err(LedgerError::NonZeroSumTransaction)
        );
        // Lifespan exhaustion transitions actor 1 to DeadOrBankrupt
        assert_eq!(
            ledger.step_lifespan(&[(0, 10, 5, 50), (1, 30, 20, -50)]),
            Ok(())
        );
        assert_eq!(ledger.status(1), ActorStatus::DeadOrBankrupt);
    }

    #[test]
    fn test_novikov_paradox_and_clue_distance() {
        // Grandfather paradox: 1 -> 2 (Copy), 2 --retro--> 1 (Invert)
        let paradox_events = [
            SpacetimeEvent {
                event_id: 1,
                episode: 1,
                kind: EventKind::LoopAnchor,
                state_bit: true,
                rel_step: None,
            },
            SpacetimeEvent {
                event_id: 2,
                episode: 1,
                kind: EventKind::Standard,
                state_bit: true,
                rel_step: None,
            },
        ];
        let paradox_edges = [
            SpacetimeEdge {
                from: 1,
                to: 2,
                relation: LogicRelation::Copy,
                is_retrocausal: false,
            },
            SpacetimeEdge {
                from: 2,
                to: 1,
                relation: LogicRelation::Invert,
                is_retrocausal: true,
            },
        ];
        assert_eq!(
            verify_spacetime_graph(1, false, &paradox_events, &paradox_edges),
            Err(LedgerError::NovikovParadox { event_id: 1 })
        );
    }

    #[test]
    fn test_wl_novelty_diversity_and_tlv_roundtrip() {
        let g1 = MechanismGraph {
            nodes: alloc::vec![
                MechanismRole::MindTrap,
                MechanismRole::MonopolyContract,
                MechanismRole::MirrorFold
            ],
            edges: alloc::vec![(0, 1), (1, 2)],
        };
        let g2 = MechanismGraph {
            nodes: alloc::vec![
                MechanismRole::RetroLoop,
                MechanismRole::PhonkSync,
                MechanismRole::VacuumGate,
                MechanismRole::PayoffConvergence
            ],
            edges: alloc::vec![(0, 1), (1, 2), (2, 3), (0, 3)],
        };
        assert_eq!(mechanism_distance_q32(&g1, &g1), Q32::ZERO);
        assert!(mechanism_distance_q32(&g1, &g2).0 >= Q32::from_milli(240).0);

        let div = five_axis_diversity_q32([20, 20, 20, 20, 20]);
        assert!(div.0 >= Q32::from_int(4).0);

        let mut m = EpisodeManifest {
            episode_id: 1,
            worldline_id: 777,
            ..EpisodeManifest::default()
        };
        m.unknown_tlvs.insert(9001, alloc::vec![1, 2, 3, 4]);
        let bytes = m.encode_tlv();
        let decoded = EpisodeManifest::decode_tlv(&bytes);
        assert_eq!(decoded, Ok(m));
    }
}
