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

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use verifier_abi::{
    AccountId, AssetId, FieldWireKind, LedgerTxKind, LegacyPlane, PlaneScoped, TlvError, TlvField,
    assert_tlv_schema_evolution, decode_tlv_stream, encode_tlv_stream,
};

pub const LEDGER_SNAPSHOT_SCHEMA_V1: &[(u16, FieldWireKind)] = &[
    (1, FieldWireKind::Fixed32),
    (2, FieldWireKind::Bytes),
    (3, FieldWireKind::Bytes),
    (4, FieldWireKind::Bytes),
    (5, FieldWireKind::Bytes),
    (6, FieldWireKind::Bytes),
    (7, FieldWireKind::Bytes),
    (8, FieldWireKind::Bytes),
    (9, FieldWireKind::Bytes),
];
assert_tlv_schema_evolution!(LEDGER_SNAPSHOT_SCHEMA_V1, LEDGER_SNAPSHOT_SCHEMA_V1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerError {
    NonPositiveAmount,
    SelfTransfer,
    InsufficientBalance,
    BalanceOverflowI64Max,
    ShardFaucetQuotaExceeded,
    EpochFaucetCapExceeded,
    DuplicateLegacyGrant,
    P2WPowerInfluenceForbidden,
    ArithmeticOverflow,
    ConservationMismatch,
}

/// Pure `#![no_std]` integer conservation & orthogonal seasonal/legacy ledger (`게임.md §3, §9.1`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerCore {
    pub season_id: u32,
    balances: BTreeMap<(AccountId, AssetId), i64>,
    seasonal_power: BTreeMap<AccountId, i64>,
    shard_faucet_remaining: BTreeMap<(u16, AssetId), i64>,
    epoch_faucet_caps: BTreeMap<AssetId, i128>,
    initial_supply: BTreeMap<AssetId, i128>,
    total_faucet_minted: BTreeMap<AssetId, i128>,
    total_sink_burned: BTreeMap<AssetId, i128>,
    legacy_vault: BTreeSet<(AccountId, u64)>,
    #[serde(default)]
    unknown_tlv_fields: Vec<TlvField>,
}

impl LedgerCore {
    pub fn new(season_id: u32) -> Self {
        Self {
            season_id,
            ..Self::default()
        }
    }

    /// Configures the epoch faucet cap for `asset`, rejecting any retroactive cap reduction below
    /// `total_faucet_minted + sum(shard_faucet_remaining)` so global conservation holds unconditionally.
    pub fn configure_asset_cap(&mut self, asset: AssetId, epoch_cap: i64) -> Result<(), LedgerError> {
        if epoch_cap < 0 {
            return Err(LedgerError::NonPositiveAmount);
        }
        let mut allocated_sum: i128 = 0;
        for (&(_, a), &rem) in &self.shard_faucet_remaining {
            if a == asset {
                allocated_sum = allocated_sum
                    .checked_add(rem as i128)
                    .ok_or(LedgerError::ArithmeticOverflow)?;
            }
        }
        let minted = self.total_faucet_minted.get(&asset).copied().unwrap_or(0);
        let committed = minted
            .checked_add(allocated_sum)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        let next_cap = epoch_cap as i128;
        if next_cap < committed {
            return Err(LedgerError::EpochFaucetCapExceeded);
        }
        self.epoch_faucet_caps.insert(asset, next_cap);
        Ok(())
    }

    pub fn allocate_shard_faucet_quota(
        &mut self,
        shard_id: u16,
        asset: AssetId,
        quota: i64,
    ) -> Result<(), LedgerError> {
        if quota < 0 {
            return Err(LedgerError::NonPositiveAmount);
        }
        let cap = self.epoch_faucet_caps.get(&asset).copied().unwrap_or(0);
        let mut allocated_sum: i128 = 0;
        for (&(_, a), &q) in &self.shard_faucet_remaining {
            if a == asset {
                allocated_sum = allocated_sum
                    .checked_add(q as i128)
                    .ok_or(LedgerError::ArithmeticOverflow)?;
            }
        }
        let minted = self.total_faucet_minted.get(&asset).copied().unwrap_or(0);
        let projected = minted
            .checked_add(allocated_sum)
            .and_then(|v| v.checked_add(quota as i128))
            .ok_or(LedgerError::ArithmeticOverflow)?;
        if projected > cap {
            return Err(LedgerError::EpochFaucetCapExceeded);
        }
        let cur = self.shard_faucet_remaining.get(&(shard_id, asset)).copied().unwrap_or(0);
        let next_q = (cur as i128)
            .checked_add(quota as i128)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        if next_q > (i64::MAX as i128) {
            return Err(LedgerError::BalanceOverflowI64Max);
        }
        self.shard_faucet_remaining.insert((shard_id, asset), next_q as i64);
        Ok(())
    }

    #[inline]
    pub fn balance(&self, account: AccountId, asset: AssetId) -> i64 {
        self.balances.get(&(account, asset)).copied().unwrap_or(0)
    }

    #[inline]
    pub fn seasonal_power(&self, account: AccountId) -> i64 {
        self.seasonal_power.get(&account).copied().unwrap_or(0)
    }

    #[inline]
    pub fn has_legacy_item(&self, account: AccountId, item_id: u64) -> bool {
        self.legacy_vault.contains(&(account, item_id))
    }

    #[inline]
    pub fn legacy_count(&self) -> usize {
        self.legacy_vault.len()
    }

    #[inline]
    pub fn unknown_tlv_fields(&self) -> &[TlvField] {
        &self.unknown_tlv_fields
    }

    pub fn export_legacy_scoped(&self, account: AccountId, item_id: u64) -> Option<PlaneScoped<LegacyPlane, u64>> {
        if self.has_legacy_item(account, item_id) {
            Some(PlaneScoped::new(item_id))
        } else {
            None
        }
    }

    /// Applies a single transaction atomically using `i128` `checked_add` / `checked_sub` only.
    pub fn apply_tx(&mut self, tx: LedgerTxKind) -> Result<(), LedgerError> {
        match tx {
            LedgerTxKind::Transfer { from, to, asset, amount } => {
                if amount <= 0 {
                    return Err(LedgerError::NonPositiveAmount);
                }
                if from == to {
                    return Err(LedgerError::SelfTransfer);
                }
                let src_bal = self.balance(from, asset) as i128;
                let dst_bal = self.balance(to, asset) as i128;
                let next_src = src_bal.checked_sub(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_src < 0 {
                    return Err(LedgerError::InsufficientBalance);
                }
                let next_dst = dst_bal.checked_add(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_dst > (i64::MAX as i128) {
                    return Err(LedgerError::BalanceOverflowI64Max);
                }
                self.balances.insert((from, asset), next_src as i64);
                self.balances.insert((to, asset), next_dst as i64);
                Ok(())
            }
            LedgerTxKind::FaucetMint {
                shard_id,
                to,
                asset,
                amount,
            } => {
                if amount <= 0 {
                    return Err(LedgerError::NonPositiveAmount);
                }
                let quota = self.shard_faucet_remaining.get(&(shard_id, asset)).copied().unwrap_or(0) as i128;
                let next_quota = quota.checked_sub(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_quota < 0 {
                    return Err(LedgerError::ShardFaucetQuotaExceeded);
                }
                let cap = self.epoch_faucet_caps.get(&asset).copied().unwrap_or(0);
                let prev_minted = self.total_faucet_minted.get(&asset).copied().unwrap_or(0);
                let next_minted = prev_minted.checked_add(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_minted > cap {
                    return Err(LedgerError::EpochFaucetCapExceeded);
                }
                let dst_bal = self.balance(to, asset) as i128;
                let next_dst = dst_bal.checked_add(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_dst > (i64::MAX as i128) {
                    return Err(LedgerError::BalanceOverflowI64Max);
                }
                self.shard_faucet_remaining.insert((shard_id, asset), next_quota as i64);
                self.total_faucet_minted.insert(asset, next_minted);
                self.balances.insert((to, asset), next_dst as i64);
                Ok(())
            }
            LedgerTxKind::SinkBurn { from, asset, amount } => {
                if amount <= 0 {
                    return Err(LedgerError::NonPositiveAmount);
                }
                let src_bal = self.balance(from, asset) as i128;
                let next_src = src_bal.checked_sub(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                if next_src < 0 {
                    return Err(LedgerError::InsufficientBalance);
                }
                let prev_burned = self.total_sink_burned.get(&asset).copied().unwrap_or(0);
                let next_burned = prev_burned.checked_add(amount as i128).ok_or(LedgerError::ArithmeticOverflow)?;
                self.balances.insert((from, asset), next_src as i64);
                self.total_sink_burned.insert(asset, next_burned);
                Ok(())
            }
            LedgerTxKind::GrantLegacy {
                account,
                legacy_item_id,
            } => {
                if self.legacy_vault.contains(&(account, legacy_item_id)) {
                    return Err(LedgerError::DuplicateLegacyGrant);
                }
                self.legacy_vault.insert((account, legacy_item_id));
                Ok(())
            }
            LedgerTxKind::ExternalCosmeticPurchase {
                account,
                cosmetic_id,
                requested_power_bonus,
            } => {
                // 게임.md §3.3: P2W influence coefficient strictly 0!
                if requested_power_bonus != 0 {
                    return Err(LedgerError::P2WPowerInfluenceForbidden);
                }
                if self.legacy_vault.contains(&(account, cosmetic_id)) {
                    return Err(LedgerError::DuplicateLegacyGrant);
                }
                self.legacy_vault.insert((account, cosmetic_id));
                Ok(())
            }
        }
    }

    /// Updates gameplay-earned `Power_S` (`0` dependency on `legacy_vault`).
    pub fn grant_gameplay_seasonal_power(&mut self, account: AccountId, delta: i64) -> Result<(), LedgerError> {
        if delta < 0 {
            return Err(LedgerError::NonPositiveAmount);
        }
        let cur = self.seasonal_power(account) as i128;
        let next = cur.checked_add(delta as i128).ok_or(LedgerError::ArithmeticOverflow)?;
        if next > (i64::MAX as i128) {
            return Err(LedgerError::BalanceOverflowI64Max);
        }
        self.seasonal_power.insert(account, next as i64);
        Ok(())
    }

    /// Orthogonal Season Set Transition (`게임.md §3.1`):
    /// Resets `Power_S` to `0` (`d Power_{S+1} / d Legacy_S == 0`) while preserving `100%` of `Legacy`.
    pub fn rollover_season(&mut self, next_season: u32) {
        self.season_id = next_season;
        self.seasonal_power.clear();
    }

    /// Verifies exact `i128` global conservation equation across all accounts & assets (`게임.md §3.2`).
    pub fn verify_global_conservation(&self) -> Result<(), LedgerError> {
        let mut sums: BTreeMap<AssetId, i128> = BTreeMap::new();
        for (&(_, asset), &bal) in &self.balances {
            if bal < 0 {
                return Err(LedgerError::ConservationMismatch);
            }
            let cur = sums.get(&asset).copied().unwrap_or(0);
            let next = cur.checked_add(bal as i128).ok_or(LedgerError::ArithmeticOverflow)?;
            sums.insert(asset, next);
        }

        let mut all_assets: BTreeSet<AssetId> = BTreeSet::new();
        for &a in sums.keys() {
            all_assets.insert(a);
        }
        for &a in self.shard_faucet_remaining.iter().map(|((_, a), _)| a) {
            all_assets.insert(a);
        }
        for &a in self.epoch_faucet_caps.keys() {
            all_assets.insert(a);
        }
        for &a in self.initial_supply.keys() {
            all_assets.insert(a);
        }
        for &a in self.total_faucet_minted.keys() {
            all_assets.insert(a);
        }
        for &a in self.total_sink_burned.keys() {
            all_assets.insert(a);
        }

        for asset in all_assets {
            let total_bal = sums.get(&asset).copied().unwrap_or(0);
            let init = self.initial_supply.get(&asset).copied().unwrap_or(0);
            let faucet = self.total_faucet_minted.get(&asset).copied().unwrap_or(0);
            let sink = self.total_sink_burned.get(&asset).copied().unwrap_or(0);
            let cap = self.epoch_faucet_caps.get(&asset).copied().unwrap_or(0);

            let mut shard_rem_sum: i128 = 0;
            for (&(_, a), &rem) in &self.shard_faucet_remaining {
                if a == asset {
                    if rem < 0 {
                        return Err(LedgerError::ConservationMismatch);
                    }
                    shard_rem_sum = shard_rem_sum
                        .checked_add(rem as i128)
                        .ok_or(LedgerError::ArithmeticOverflow)?;
                }
            }
            let committed = faucet
                .checked_add(shard_rem_sum)
                .ok_or(LedgerError::ArithmeticOverflow)?;

            if faucet < 0 || committed > cap || sink < 0 {
                return Err(LedgerError::ConservationMismatch);
            }
            let delta_bal = total_bal.checked_sub(init).ok_or(LedgerError::ArithmeticOverflow)?;
            let net_flow = faucet.checked_sub(sink).ok_or(LedgerError::ArithmeticOverflow)?;
            if delta_bal != net_flow {
                return Err(LedgerError::ConservationMismatch);
            }
        }
        Ok(())
    }

    /// Domain-separated canonical BLAKE3 digest covering every state map/set and explicit cardinality.
    pub fn state_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"LEDGER_V1");
        hasher.update(&self.season_id.to_le_bytes());

        hasher.update(b"BAL");
        hasher.update(&(self.balances.len() as u64).to_le_bytes());
        for (&(acc, asset), &bal) in &self.balances {
            hasher.update(&acc.0.to_le_bytes());
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&bal.to_le_bytes());
        }

        hasher.update(b"POW");
        hasher.update(&(self.seasonal_power.len() as u64).to_le_bytes());
        for (&acc, &pow) in &self.seasonal_power {
            hasher.update(&acc.0.to_le_bytes());
            hasher.update(&pow.to_le_bytes());
        }

        hasher.update(b"SHARD");
        hasher.update(&(self.shard_faucet_remaining.len() as u64).to_le_bytes());
        for (&(shard, asset), &rem) in &self.shard_faucet_remaining {
            hasher.update(&shard.to_le_bytes());
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&rem.to_le_bytes());
        }

        hasher.update(b"CAP");
        hasher.update(&(self.epoch_faucet_caps.len() as u64).to_le_bytes());
        for (&asset, &cap) in &self.epoch_faucet_caps {
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&cap.to_le_bytes());
        }

        hasher.update(b"INIT");
        hasher.update(&(self.initial_supply.len() as u64).to_le_bytes());
        for (&asset, &sup) in &self.initial_supply {
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&sup.to_le_bytes());
        }

        hasher.update(b"MINT");
        hasher.update(&(self.total_faucet_minted.len() as u64).to_le_bytes());
        for (&asset, &mint) in &self.total_faucet_minted {
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&mint.to_le_bytes());
        }

        hasher.update(b"BURN");
        hasher.update(&(self.total_sink_burned.len() as u64).to_le_bytes());
        for (&asset, &burn) in &self.total_sink_burned {
            hasher.update(&asset.0.to_le_bytes());
            hasher.update(&burn.to_le_bytes());
        }

        hasher.update(b"LEG");
        hasher.update(&(self.legacy_vault.len() as u64).to_le_bytes());
        for &(acc, item) in &self.legacy_vault {
            hasher.update(&acc.0.to_le_bytes());
            hasher.update(&item.to_le_bytes());
        }

        hasher.update(b"UNK");
        hasher.update(&(self.unknown_tlv_fields.len() as u64).to_le_bytes());
        for f in &self.unknown_tlv_fields {
            hasher.update(&f.tag.to_le_bytes());
            hasher.update(&(f.value.len() as u64).to_le_bytes());
            hasher.update(&f.value);
        }

        *hasher.finalize().as_bytes()
    }

    /// Canonical additive `u16` TLV snapshot encoder (`게임.md §5.3, §9.2`).
    pub fn encode_tlv(&self) -> Result<Vec<u8>, TlvError> {
        let mut fields = Vec::new();
        fields.push(TlvField {
            tag: 1,
            value: self.season_id.to_le_bytes().to_vec(),
        });
        for (&(acc, asset), &bal) in &self.balances {
            let mut v = Vec::with_capacity(18);
            v.extend_from_slice(&acc.0.to_le_bytes());
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&bal.to_le_bytes());
            fields.push(TlvField { tag: 2, value: v });
        }
        for (&acc, &pow) in &self.seasonal_power {
            let mut v = Vec::with_capacity(16);
            v.extend_from_slice(&acc.0.to_le_bytes());
            v.extend_from_slice(&pow.to_le_bytes());
            fields.push(TlvField { tag: 3, value: v });
        }
        for (&(shard, asset), &rem) in &self.shard_faucet_remaining {
            let mut v = Vec::with_capacity(12);
            v.extend_from_slice(&shard.to_le_bytes());
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&rem.to_le_bytes());
            fields.push(TlvField { tag: 4, value: v });
        }
        for (&asset, &cap) in &self.epoch_faucet_caps {
            let mut v = Vec::with_capacity(18);
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&cap.to_le_bytes());
            fields.push(TlvField { tag: 5, value: v });
        }
        for (&asset, &sup) in &self.initial_supply {
            let mut v = Vec::with_capacity(18);
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&sup.to_le_bytes());
            fields.push(TlvField { tag: 6, value: v });
        }
        for (&asset, &mint) in &self.total_faucet_minted {
            let mut v = Vec::with_capacity(18);
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&mint.to_le_bytes());
            fields.push(TlvField { tag: 7, value: v });
        }
        for (&asset, &burn) in &self.total_sink_burned {
            let mut v = Vec::with_capacity(18);
            v.extend_from_slice(&asset.0.to_le_bytes());
            v.extend_from_slice(&burn.to_le_bytes());
            fields.push(TlvField { tag: 8, value: v });
        }
        for &(acc, item) in &self.legacy_vault {
            let mut v = Vec::with_capacity(16);
            v.extend_from_slice(&acc.0.to_le_bytes());
            v.extend_from_slice(&item.to_le_bytes());
            fields.push(TlvField { tag: 9, value: v });
        }
        for unk in &self.unknown_tlv_fields {
            fields.push(unk.clone());
        }
        encode_tlv_stream(&fields)
    }

    /// Canonical additive `u16` TLV snapshot decoder (preserves unknown future tags lossless round-trip).
    pub fn decode_tlv(bytes: &[u8]) -> Result<Self, TlvError> {
        let fields = decode_tlv_stream(bytes)?;
        let mut out = Self::default();
        for f in fields {
            match f.tag {
                1 => {
                    let b: [u8; 4] = f.value.as_slice().try_into().map_err(|_| TlvError::TruncatedPayload)?;
                    out.season_id = u32::from_le_bytes(b);
                }
                2 => {
                    if f.value.len() != 18 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let acc = u64::from_le_bytes(f.value[0..8].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let asset = u16::from_le_bytes(f.value[8..10].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let bal = i64::from_le_bytes(f.value[10..18].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.balances.insert((AccountId(acc), AssetId(asset)), bal);
                }
                3 => {
                    if f.value.len() != 16 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let acc = u64::from_le_bytes(f.value[0..8].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let pow = i64::from_le_bytes(f.value[8..16].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.seasonal_power.insert(AccountId(acc), pow);
                }
                4 => {
                    if f.value.len() != 12 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let shard = u16::from_le_bytes(f.value[0..2].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let asset = u16::from_le_bytes(f.value[2..4].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let rem = i64::from_le_bytes(f.value[4..12].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.shard_faucet_remaining.insert((shard, AssetId(asset)), rem);
                }
                5 => {
                    if f.value.len() != 18 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let asset = u16::from_le_bytes(f.value[0..2].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let cap = i128::from_le_bytes(f.value[2..18].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.epoch_faucet_caps.insert(AssetId(asset), cap);
                }
                6 => {
                    if f.value.len() != 18 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let asset = u16::from_le_bytes(f.value[0..2].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let sup = i128::from_le_bytes(f.value[2..18].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.initial_supply.insert(AssetId(asset), sup);
                }
                7 => {
                    if f.value.len() != 18 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let asset = u16::from_le_bytes(f.value[0..2].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let mint = i128::from_le_bytes(f.value[2..18].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.total_faucet_minted.insert(AssetId(asset), mint);
                }
                8 => {
                    if f.value.len() != 18 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let asset = u16::from_le_bytes(f.value[0..2].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let burn = i128::from_le_bytes(f.value[2..18].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.total_sink_burned.insert(AssetId(asset), burn);
                }
                9 => {
                    if f.value.len() != 16 {
                        return Err(TlvError::TruncatedPayload);
                    }
                    let acc = u64::from_le_bytes(f.value[0..8].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    let item = u64::from_le_bytes(f.value[8..16].try_into().map_err(|_| TlvError::TruncatedPayload)?);
                    out.legacy_vault.insert((AccountId(acc), item));
                }
                _ => out.unknown_tlv_fields.push(f),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservation_and_season_orthogonality() {
        let mut ledger = LedgerCore::new(1);
        let gold = AssetId(1);
        assert_eq!(ledger.configure_asset_cap(gold, 1_000_000), Ok(()));
        assert_eq!(ledger.allocate_shard_faucet_quota(0, gold, 500_000), Ok(()));

        assert_eq!(
            ledger.apply_tx(LedgerTxKind::FaucetMint {
                shard_id: 0,
                to: AccountId(10),
                asset: gold,
                amount: 1000,
            }),
            Ok(())
        );
        assert_eq!(
            ledger.apply_tx(LedgerTxKind::Transfer {
                from: AccountId(10),
                to: AccountId(20),
                asset: gold,
                amount: 400,
            }),
            Ok(())
        );
        assert_eq!(
            ledger.apply_tx(LedgerTxKind::SinkBurn {
                from: AccountId(20),
                asset: gold,
                amount: 150,
            }),
            Ok(())
        );
        assert_eq!(ledger.verify_global_conservation(), Ok(()));

        // Retroactive cap reduction below minted + remaining shard quota MUST fail (`CE-12`)!
        assert_eq!(
            ledger.configure_asset_cap(gold, 100),
            Err(LedgerError::EpochFaucetCapExceeded)
        );
        assert_eq!(ledger.verify_global_conservation(), Ok(()));

        // P2W rejection
        assert_eq!(
            ledger.apply_tx(LedgerTxKind::ExternalCosmeticPurchase {
                account: AccountId(10),
                cosmetic_id: 77,
                requested_power_bonus: 1,
            }),
            Err(LedgerError::P2WPowerInfluenceForbidden)
        );
        assert_eq!(
            ledger.apply_tx(LedgerTxKind::ExternalCosmeticPurchase {
                account: AccountId(10),
                cosmetic_id: 77,
                requested_power_bonus: 0,
            }),
            Ok(())
        );
        assert_eq!(ledger.grant_gameplay_seasonal_power(AccountId(10), 500), Ok(()));
        ledger.rollover_season(2);
        assert_eq!(ledger.seasonal_power(AccountId(10)), 0);
        assert!(ledger.has_legacy_item(AccountId(10), 77));
    }

    #[test]
    fn test_state_hash_domain_separation_and_unknown_tlv_roundtrip() {
        let mut a = LedgerCore::new(1);
        let mut b = LedgerCore::new(1);
        assert_eq!(a.grant_gameplay_seasonal_power(AccountId(5), 77), Ok(()));
        assert_eq!(
            b.apply_tx(LedgerTxKind::GrantLegacy {
                account: AccountId(5),
                legacy_item_id: 77,
            }),
            Ok(())
        );
        assert_ne!(a.state_hash(), b.state_hash());

        let mut encoded = a.encode_tlv().unwrap_or_default();
        let future_ext = encode_tlv_stream(&[TlvField {
            tag: 999,
            value: alloc::vec![0xDE, 0xAD, 0xBE, 0xEF],
        }])
        .unwrap_or_default();
        encoded.extend_from_slice(&future_ext);

        let decoded = LedgerCore::decode_tlv(&encoded).unwrap_or_default();
        assert_eq!(decoded.seasonal_power(AccountId(5)), 77);
        assert_eq!(decoded.unknown_tlv_fields().len(), 1);
        let reencoded = decoded.encode_tlv().unwrap_or_default();
        assert_eq!(reencoded, encoded);
    }
}
