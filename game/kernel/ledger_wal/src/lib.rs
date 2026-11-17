#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use ledger_core::{LedgerCore, LedgerError};
use serde::{Deserialize, Serialize};
use verifier_abi::{
    AccountId, AssetId, FieldWireKind, LedgerTxKind, TlvField, assert_tlv_schema_evolution,
    decode_tlv_stream, encode_tlv_stream,
};

pub const MAX_BATCH_LATENCY_MICROS: u64 = 2_000;
pub const DEFAULT_BATCH_THRESHOLD: usize = 256;
pub const MAX_RING_CAPACITY: usize = 4_096;

const WAL_MAGIC: [u8; 8] = *b"GWAL0001";
const CHK_MAGIC: [u8; 8] = *b"GCHK0001";
const HEADER_LEN: usize = 88; // 8 magic + 8 seq + 32 prev_hash + 4 len + 4 crc32c + 32 chain_hash
const MAX_SEGMENT_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

// Immutable additive TLV tags (`게임.md §5.3, §9.2`)
const TAG_OP_KIND: u16 = 1;
const TAG_ACCOUNT_FROM: u16 = 2;
const TAG_ACCOUNT_TO: u16 = 3;
const TAG_ASSET_ID: u16 = 4;
const TAG_AMOUNT_I64: u16 = 5;
const TAG_SHARD_ID: u16 = 6;
const TAG_LEGACY_ITEM_ID: u16 = 7;
const TAG_POWER_BONUS_I64: u16 = 8;
const TAG_SEASON_ID: u16 = 9;
const TAG_BATCH_ENTRY: u16 = 100;

pub const WAL_OP_SCHEMA_V1: &[(u16, FieldWireKind)] = &[
    (TAG_OP_KIND, FieldWireKind::Bytes),
    (TAG_ACCOUNT_FROM, FieldWireKind::Fixed64),
    (TAG_ACCOUNT_TO, FieldWireKind::Fixed64),
    (TAG_ASSET_ID, FieldWireKind::Bytes),
    (TAG_AMOUNT_I64, FieldWireKind::Fixed64),
    (TAG_SHARD_ID, FieldWireKind::Bytes),
    (TAG_LEGACY_ITEM_ID, FieldWireKind::Fixed64),
    (TAG_POWER_BONUS_I64, FieldWireKind::Fixed64),
    (TAG_SEASON_ID, FieldWireKind::Fixed32),
];
assert_tlv_schema_evolution!(WAL_OP_SCHEMA_V1, WAL_OP_SCHEMA_V1);

pub const CHECKPOINT_SCHEMA_V1: &[(u16, FieldWireKind)] = &[
    (1, FieldWireKind::Fixed64),
    (2, FieldWireKind::Bytes),
    (3, FieldWireKind::Bytes),
];
pub const CHECKPOINT_SCHEMA_V2: &[(u16, FieldWireKind)] = &[
    (1, FieldWireKind::Fixed64),
    (2, FieldWireKind::Bytes),
    (3, FieldWireKind::Bytes),
    (4, FieldWireKind::Fixed64),
];
assert_tlv_schema_evolution!(CHECKPOINT_SCHEMA_V1, CHECKPOINT_SCHEMA_V2);

/// Pure-Rust Castagnoli `CRC32C` (`0x82F63B78` reflected polynomial).
#[inline]
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        let mut bit: u8 = 0;
        while bit < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0x82F6_3B78 & mask);
            bit = bit.saturating_add(1);
        }
    }
    !crc
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalOp {
    ConfigureAssetCap { asset: AssetId, epoch_cap: i64 },
    AllocateShardQuota { shard_id: u16, asset: AssetId, quota: i64 },
    ApplyTx(LedgerTxKind),
    GrantSeasonalPower { account: AccountId, delta: i64 },
    RolloverSeason { next_season: u32 },
}

impl WalOp {
    pub fn encode_tlv(&self, unknown_passthrough: &[TlvField]) -> Option<Vec<u8>> {
        let mut recs: Vec<TlvField> = Vec::with_capacity(6usize.saturating_add(unknown_passthrough.len()));
        match *self {
            Self::ConfigureAssetCap { asset, epoch_cap } => {
                recs.push(TlvField { tag: TAG_OP_KIND, value: vec![1] });
                recs.push(TlvField { tag: TAG_ASSET_ID, value: asset.0.to_le_bytes().to_vec() });
                recs.push(TlvField { tag: TAG_AMOUNT_I64, value: epoch_cap.to_le_bytes().to_vec() });
            }
            Self::AllocateShardQuota { shard_id, asset, quota } => {
                recs.push(TlvField { tag: TAG_OP_KIND, value: vec![2] });
                recs.push(TlvField { tag: TAG_SHARD_ID, value: shard_id.to_le_bytes().to_vec() });
                recs.push(TlvField { tag: TAG_ASSET_ID, value: asset.0.to_le_bytes().to_vec() });
                recs.push(TlvField { tag: TAG_AMOUNT_I64, value: quota.to_le_bytes().to_vec() });
            }
            Self::ApplyTx(tx) => match tx {
                LedgerTxKind::Transfer { from, to, asset, amount } => {
                    recs.push(TlvField { tag: TAG_OP_KIND, value: vec![3] });
                    recs.push(TlvField { tag: TAG_ACCOUNT_FROM, value: from.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_ACCOUNT_TO, value: to.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_ASSET_ID, value: asset.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_AMOUNT_I64, value: amount.to_le_bytes().to_vec() });
                }
                LedgerTxKind::FaucetMint { shard_id, to, asset, amount } => {
                    recs.push(TlvField { tag: TAG_OP_KIND, value: vec![4] });
                    recs.push(TlvField { tag: TAG_SHARD_ID, value: shard_id.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_ACCOUNT_TO, value: to.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_ASSET_ID, value: asset.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_AMOUNT_I64, value: amount.to_le_bytes().to_vec() });
                }
                LedgerTxKind::SinkBurn { from, asset, amount } => {
                    recs.push(TlvField { tag: TAG_OP_KIND, value: vec![5] });
                    recs.push(TlvField { tag: TAG_ACCOUNT_FROM, value: from.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_ASSET_ID, value: asset.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_AMOUNT_I64, value: amount.to_le_bytes().to_vec() });
                }
                LedgerTxKind::GrantLegacy { account, legacy_item_id } => {
                    recs.push(TlvField { tag: TAG_OP_KIND, value: vec![6] });
                    recs.push(TlvField { tag: TAG_ACCOUNT_TO, value: account.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_LEGACY_ITEM_ID, value: legacy_item_id.to_le_bytes().to_vec() });
                }
                LedgerTxKind::ExternalCosmeticPurchase {
                    account,
                    cosmetic_id,
                    requested_power_bonus,
                } => {
                    recs.push(TlvField { tag: TAG_OP_KIND, value: vec![7] });
                    recs.push(TlvField { tag: TAG_ACCOUNT_TO, value: account.0.to_le_bytes().to_vec() });
                    recs.push(TlvField { tag: TAG_LEGACY_ITEM_ID, value: cosmetic_id.to_le_bytes().to_vec() });
                    recs.push(TlvField {
                        tag: TAG_POWER_BONUS_I64,
                        value: requested_power_bonus.to_le_bytes().to_vec(),
                    });
                }
            },
            Self::GrantSeasonalPower { account, delta } => {
                recs.push(TlvField { tag: TAG_OP_KIND, value: vec![8] });
                recs.push(TlvField { tag: TAG_ACCOUNT_TO, value: account.0.to_le_bytes().to_vec() });
                recs.push(TlvField { tag: TAG_AMOUNT_I64, value: delta.to_le_bytes().to_vec() });
            }
            Self::RolloverSeason { next_season } => {
                recs.push(TlvField { tag: TAG_OP_KIND, value: vec![9] });
                recs.push(TlvField { tag: TAG_SEASON_ID, value: next_season.to_le_bytes().to_vec() });
            }
        }
        for extra in unknown_passthrough {
            recs.push(extra.clone());
        }
        encode_tlv_stream(&recs).ok()
    }

    pub fn decode_tlv(bytes: &[u8]) -> Option<(Self, Vec<TlvField>)> {
        let recs = decode_tlv_stream(bytes).ok()?;
        let mut kind: u8 = 0;
        let mut acc_from: u64 = 0;
        let mut acc_to: u64 = 0;
        let mut asset_id: u16 = 0;
        let mut amount: i64 = 0;
        let mut shard_id: u16 = 0;
        let mut legacy_id: u64 = 0;
        let mut power_bonus: i64 = 0;
        let mut season_id: u32 = 0;
        let mut unknown: Vec<TlvField> = Vec::new();

        for r in recs {
            match r.tag {
                TAG_OP_KIND => {
                    kind = *r.value.first()?;
                }
                TAG_ACCOUNT_FROM => {
                    let b: [u8; 8] = r.value.as_slice().try_into().ok()?;
                    acc_from = u64::from_le_bytes(b);
                }
                TAG_ACCOUNT_TO => {
                    let b: [u8; 8] = r.value.as_slice().try_into().ok()?;
                    acc_to = u64::from_le_bytes(b);
                }
                TAG_ASSET_ID => {
                    let b: [u8; 2] = r.value.as_slice().try_into().ok()?;
                    asset_id = u16::from_le_bytes(b);
                }
                TAG_AMOUNT_I64 => {
                    let b: [u8; 8] = r.value.as_slice().try_into().ok()?;
                    amount = i64::from_le_bytes(b);
                }
                TAG_SHARD_ID => {
                    let b: [u8; 2] = r.value.as_slice().try_into().ok()?;
                    shard_id = u16::from_le_bytes(b);
                }
                TAG_LEGACY_ITEM_ID => {
                    let b: [u8; 8] = r.value.as_slice().try_into().ok()?;
                    legacy_id = u64::from_le_bytes(b);
                }
                TAG_POWER_BONUS_I64 => {
                    let b: [u8; 8] = r.value.as_slice().try_into().ok()?;
                    power_bonus = i64::from_le_bytes(b);
                }
                TAG_SEASON_ID => {
                    let b: [u8; 4] = r.value.as_slice().try_into().ok()?;
                    season_id = u32::from_le_bytes(b);
                }
                _ => unknown.push(r),
            }
        }

        let op = match kind {
            1 => Self::ConfigureAssetCap {
                asset: AssetId(asset_id),
                epoch_cap: amount,
            },
            2 => Self::AllocateShardQuota {
                shard_id,
                asset: AssetId(asset_id),
                quota: amount,
            },
            3 => Self::ApplyTx(LedgerTxKind::Transfer {
                from: AccountId(acc_from),
                to: AccountId(acc_to),
                asset: AssetId(asset_id),
                amount,
            }),
            4 => Self::ApplyTx(LedgerTxKind::FaucetMint {
                shard_id,
                to: AccountId(acc_to),
                asset: AssetId(asset_id),
                amount,
            }),
            5 => Self::ApplyTx(LedgerTxKind::SinkBurn {
                from: AccountId(acc_from),
                asset: AssetId(asset_id),
                amount,
            }),
            6 => Self::ApplyTx(LedgerTxKind::GrantLegacy {
                account: AccountId(acc_to),
                legacy_item_id: legacy_id,
            }),
            7 => Self::ApplyTx(LedgerTxKind::ExternalCosmeticPurchase {
                account: AccountId(acc_to),
                cosmetic_id: legacy_id,
                requested_power_bonus: power_bonus,
            }),
            8 => Self::GrantSeasonalPower {
                account: AccountId(acc_to),
                delta: amount,
            },
            9 => Self::RolloverSeason {
                next_season: season_id,
            },
            _ => return None,
        };
        Some((op, unknown))
    }

    pub fn apply_to(&self, ledger: &mut LedgerCore) -> Result<(), LedgerError> {
        match *self {
            Self::ConfigureAssetCap { asset, epoch_cap } => ledger.configure_asset_cap(asset, epoch_cap),
            Self::AllocateShardQuota { shard_id, asset, quota } => {
                ledger.allocate_shard_faucet_quota(shard_id, asset, quota)
            }
            Self::ApplyTx(tx) => ledger.apply_tx(tx),
            Self::GrantSeasonalPower { account, delta } => ledger.grant_gameplay_seasonal_power(account, delta),
            Self::RolloverSeason { next_season } => {
                ledger.rollover_season(next_season);
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub enum WalError {
    Ledger(LedgerError),
    RingBufferFull,
    EncodingError,
    CorruptCheckpoint,
    ManifestParseError,
    Io(std::io::Error),
}

impl From<LedgerError> for WalError {
    fn from(v: LedgerError) -> Self {
        Self::Ledger(v)
    }
}

impl From<std::io::Error> for WalError {
    fn from(v: std::io::Error) -> Self {
        Self::Io(v)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagedTicket {
    pub tx_seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommittedAck {
    pub tx_seq: u64,
    pub segment_seq: u64,
    pub chain_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalRecoveryReport {
    pub replayed_segments: u64,
    pub replayed_ops: u64,
    pub truncated_tail_bytes: u64,
    pub loaded_checkpoint: bool,
    pub head_chain_hash: [u8; 32],
}

#[derive(Clone, Debug)]
struct StagedWalEntry {
    tx_seq: u64,
    op: WalOp,
    unknown_fields: Vec<TlvField>,
}

pub struct CheckpointEnvelope {
    pub next_segment_seq: u64,
    pub prev_chain_hash: [u8; 32],
    pub ledger: LedgerCore,
    pub next_tx_seq: u64,
    pub unknown_fields: Vec<TlvField>,
}

impl CheckpointEnvelope {
    pub fn encode_tlv(&self) -> Result<Vec<u8>, WalError> {
        let ledger_bytes = self.ledger.encode_tlv().map_err(|_| WalError::EncodingError)?;
        let mut fields = vec![
            TlvField {
                tag: 1,
                value: self.next_segment_seq.to_le_bytes().to_vec(),
            },
            TlvField {
                tag: 2,
                value: self.prev_chain_hash.to_vec(),
            },
            TlvField {
                tag: 3,
                value: ledger_bytes,
            },
            TlvField {
                tag: 4,
                value: self.next_tx_seq.to_le_bytes().to_vec(),
            },
        ];
        for unk in &self.unknown_fields {
            fields.push(unk.clone());
        }
        encode_tlv_stream(&fields).map_err(|_| WalError::EncodingError)
    }

    pub fn decode_tlv(bytes: &[u8]) -> Result<Self, WalError> {
        let fields = decode_tlv_stream(bytes).map_err(|_| WalError::CorruptCheckpoint)?;
        let mut next_segment_seq: Option<u64> = None;
        let mut prev_chain_hash: Option<[u8; 32]> = None;
        let mut ledger: Option<LedgerCore> = None;
        let mut next_tx_seq: u64 = 1;
        let mut unknown_fields: Vec<TlvField> = Vec::new();
        for f in fields {
            match f.tag {
                1 => {
                    let b: [u8; 8] = f.value.as_slice().try_into().map_err(|_| WalError::CorruptCheckpoint)?;
                    next_segment_seq = Some(u64::from_le_bytes(b));
                }
                2 => {
                    let b: [u8; 32] = f.value.as_slice().try_into().map_err(|_| WalError::CorruptCheckpoint)?;
                    prev_chain_hash = Some(b);
                }
                3 => {
                    ledger = Some(LedgerCore::decode_tlv(&f.value).map_err(|_| WalError::CorruptCheckpoint)?);
                }
                4 => {
                    let b: [u8; 8] = f.value.as_slice().try_into().map_err(|_| WalError::CorruptCheckpoint)?;
                    next_tx_seq = u64::from_le_bytes(b).max(1);
                }
                _ => unknown_fields.push(f),
            }
        }
        Ok(Self {
            next_segment_seq: next_segment_seq.ok_or(WalError::CorruptCheckpoint)?,
            prev_chain_hash: prev_chain_hash.ok_or(WalError::CorruptCheckpoint)?,
            ledger: ledger.ok_or(WalError::CorruptCheckpoint)?,
            next_tx_seq,
            unknown_fields,
        })
    }
}

/// Bounded-latency (`<= 2.0ms` or threshold) Micro-batch Group Commit RingBuffer WAL (`게임.md §9.1`).
///
/// Guarantees:
/// 1. Sequential `staged` (`B_staged`) pre-validation rejects intra-batch conflicting/double-spend
///    transactions immediately before enqueueing into the ringbuffer.
/// 2. `committed` (`B_committed`) is NEVER mutated and external `CommittedAck`s are NEVER emitted
///    until the single `CRC32C` + `BLAKE3` segment is durably `fsync`ed (`File::sync_all()`).
/// 3. Failed `write_and_fsync_frame` calls immediately truncate back to `prev_valid_len` before
///    rolling back `staged`, preventing partial-byte misalignment on subsequent flushes.
/// 4. Uncommitted/corrupted torn tail segments after power loss or `kill -9` are automatically
///    detected and truncated on `open_and_recover`, supporting both unrotated and post-checkpoint
///    rotated `wal.log` files cleanly.
pub struct GroupCommitWal {
    wal_path: PathBuf,
    checkpoint_path: PathBuf,
    wal_file: File,
    committed: LedgerCore,
    staged: LedgerCore,
    ring: VecDeque<StagedWalEntry>,
    oldest_enqueued_micros: Option<u64>,
    batch_threshold: usize,
    next_tx_seq: u64,
    next_segment_seq: u64,
    prev_chain_hash: [u8; 32],
    checkpoint_unknown_fields: Vec<TlvField>,
}

impl GroupCommitWal {
    pub fn open_and_recover(
        dir: &Path,
        batch_threshold: usize,
    ) -> Result<(Self, WalRecoveryReport), WalError> {
        fs::create_dir_all(dir)?;
        let wal_path = dir.join("wal.log");
        let checkpoint_path = dir.join("checkpoint.bin");

        let mut committed = LedgerCore::new(1);
        let mut checkpoint_next_seq: u64 = 0;
        let mut checkpoint_chain_hash = [0u8; 32];
        let mut checkpoint_next_tx_seq: u64 = 1;
        let mut checkpoint_unknown_fields: Vec<TlvField> = Vec::new();
        let mut loaded_checkpoint = false;

        if checkpoint_path.exists() {
            let raw = fs::read(&checkpoint_path)?;
            if raw.len() < 12 || raw[..8] != CHK_MAGIC {
                return Err(WalError::CorruptCheckpoint);
            }
            let stored_crc = u32::from_le_bytes(
                raw[8..12].try_into().map_err(|_| WalError::CorruptCheckpoint)?,
            );
            let body = &raw[12..];
            if crc32c(body) != stored_crc {
                return Err(WalError::CorruptCheckpoint);
            }
            let env = CheckpointEnvelope::decode_tlv(body)?;
            env.ledger.verify_global_conservation()?;
            committed = env.ledger;
            checkpoint_next_seq = env.next_segment_seq;
            checkpoint_chain_hash = env.prev_chain_hash;
            checkpoint_next_tx_seq = env.next_tx_seq;
            checkpoint_unknown_fields = env.unknown_fields;
            loaded_checkpoint = true;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&wal_path)?;

        let file_len = file.metadata()?.len();
        file.seek(SeekFrom::Start(0))?;

        let mut valid_offset: u64 = 0;
        let mut expected_seq: u64 = 0;
        let mut running_hash = [0u8; 32];
        let mut replayed_segments: u64 = 0;
        let mut replayed_ops: u64 = 0;
        let mut next_tx_seq: u64 = 1;

        while valid_offset < file_len {
            let remaining = file_len.saturating_sub(valid_offset);
            if remaining < (HEADER_LEN as u64) {
                break;
            }
            let mut hdr = [0u8; HEADER_LEN];
            if file.read_exact(&mut hdr).is_err() {
                break;
            }
            if hdr[0..8] != WAL_MAGIC {
                break;
            }
            let seg_seq = u64::from_le_bytes(
                hdr[8..16].try_into().map_err(|_| WalError::EncodingError)?,
            );
            let mut prev_hash = [0u8; 32];
            prev_hash.copy_from_slice(&hdr[16..48]);
            let payload_len = u32::from_le_bytes(
                hdr[48..52].try_into().map_err(|_| WalError::EncodingError)?,
            ) as usize;
            let stored_crc = u32::from_le_bytes(
                hdr[52..56].try_into().map_err(|_| WalError::EncodingError)?,
            );
            let mut stored_chain_hash = [0u8; 32];
            stored_chain_hash.copy_from_slice(&hdr[56..88]);

            // Support post-checkpoint rotated `wal.log` whose first frame starts at `checkpoint_next_seq`.
            if valid_offset == 0
                && loaded_checkpoint
                && seg_seq == checkpoint_next_seq
                && prev_hash == checkpoint_chain_hash
            {
                expected_seq = checkpoint_next_seq;
                running_hash = checkpoint_chain_hash;
                next_tx_seq = checkpoint_next_tx_seq;
            }

            if seg_seq != expected_seq
                || prev_hash != running_hash
                || payload_len > MAX_SEGMENT_PAYLOAD_BYTES
            {
                break;
            }
            let needed = (HEADER_LEN as u64).saturating_add(payload_len as u64);
            if remaining < needed {
                break;
            }
            let mut payload = vec![0u8; payload_len];
            if file.read_exact(&mut payload).is_err() {
                break;
            }

            let computed_crc = compute_frame_crc(seg_seq, &prev_hash, payload_len as u32, &payload);
            if computed_crc != stored_crc {
                break;
            }
            let computed_chain = compute_chain_hash(&prev_hash, seg_seq, computed_crc, &payload);
            if computed_chain != stored_chain_hash {
                break;
            }

            let Ok(batch_recs) = decode_tlv_stream(&payload) else {
                break;
            };
            let mut decoded_ops: Vec<WalOp> = Vec::with_capacity(batch_recs.len());
            let mut decode_ok = true;
            for r in batch_recs {
                if r.tag == TAG_BATCH_ENTRY {
                    if let Some((op, _unknown)) = WalOp::decode_tlv(&r.value) {
                        decoded_ops.push(op);
                    } else {
                        decode_ok = false;
                        break;
                    }
                }
            }
            if !decode_ok {
                break;
            }

            if seg_seq >= checkpoint_next_seq {
                if seg_seq == checkpoint_next_seq && loaded_checkpoint && prev_hash != checkpoint_chain_hash {
                    return Err(WalError::CorruptCheckpoint);
                }
                for op in &decoded_ops {
                    op.apply_to(&mut committed)?;
                    replayed_ops = replayed_ops.saturating_add(1);
                }
                replayed_segments = replayed_segments.saturating_add(1);
            }

            next_tx_seq = next_tx_seq.saturating_add(decoded_ops.len() as u64);
            running_hash = computed_chain;
            expected_seq = expected_seq.saturating_add(1);
            valid_offset = valid_offset.saturating_add(needed);
        }

        let truncated_tail_bytes = file_len.saturating_sub(valid_offset);
        if truncated_tail_bytes > 0 {
            file.set_len(valid_offset)?;
            file.sync_all()?;
        }
        file.seek(SeekFrom::End(0))?;

        if loaded_checkpoint && expected_seq < checkpoint_next_seq {
            expected_seq = checkpoint_next_seq;
            running_hash = checkpoint_chain_hash;
            next_tx_seq = checkpoint_next_tx_seq;
        }

        committed.verify_global_conservation()?;
        let staged = committed.clone();

        let wal = Self {
            wal_path,
            checkpoint_path,
            wal_file: file,
            committed,
            staged,
            ring: VecDeque::with_capacity(batch_threshold.clamp(1, MAX_RING_CAPACITY)),
            oldest_enqueued_micros: None,
            batch_threshold: batch_threshold.clamp(1, MAX_RING_CAPACITY),
            next_tx_seq,
            next_segment_seq: expected_seq,
            prev_chain_hash: running_hash,
            checkpoint_unknown_fields,
        };
        let report = WalRecoveryReport {
            replayed_segments,
            replayed_ops,
            truncated_tail_bytes,
            loaded_checkpoint,
            head_chain_hash: running_hash,
        };
        Ok((wal, report))
    }

    #[inline]
    pub fn committed_ledger(&self) -> &LedgerCore {
        &self.committed
    }

    #[inline]
    pub fn staged_ledger(&self) -> &LedgerCore {
        &self.staged
    }

    #[inline]
    pub fn pending_count(&self) -> usize {
        self.ring.len()
    }

    #[inline]
    pub fn wal_path(&self) -> &Path {
        &self.wal_path
    }

    #[inline]
    pub fn head_chain_hash(&self) -> [u8; 32] {
        self.prev_chain_hash
    }

    /// Pre-validates `op` sequentially against `B_staged`, appends to the Group Commit ringbuffer,
    /// and automatically flushes (`fsync`-first) if batch size >= threshold or latency >= `2.0ms`.
    pub fn submit_op(
        &mut self,
        op: WalOp,
        now_micros: u64,
    ) -> Result<(StagedTicket, Vec<CommittedAck>), WalError> {
        self.submit_op_with_unknown_fields(op, Vec::new(), now_micros)
    }

    /// Submits `op` while preserving additive unknown future TLV fields lossless through the WAL segment.
    pub fn submit_op_with_unknown_fields(
        &mut self,
        op: WalOp,
        unknown_fields: Vec<TlvField>,
        now_micros: u64,
    ) -> Result<(StagedTicket, Vec<CommittedAck>), WalError> {
        if self.ring.len() >= MAX_RING_CAPACITY {
            return Err(WalError::RingBufferFull);
        }
        op.apply_to(&mut self.staged)?;

        let tx_seq = self.next_tx_seq;
        self.next_tx_seq = self.next_tx_seq.saturating_add(1);
        if self.oldest_enqueued_micros.is_none() {
            self.oldest_enqueued_micros = Some(now_micros);
        }
        self.ring.push_back(StagedWalEntry {
            tx_seq,
            op,
            unknown_fields,
        });

        let oldest = self.oldest_enqueued_micros.unwrap_or(now_micros);
        let elapsed = now_micros.saturating_sub(oldest);
        let acks = if self.ring.len() >= self.batch_threshold || elapsed >= MAX_BATCH_LATENCY_MICROS {
            self.flush_group_commit()?
        } else {
            Vec::new()
        };

        Ok((StagedTicket { tx_seq }, acks))
    }

    /// Convenience wrapper for submitting a `LedgerTxKind`.
    pub fn submit_tx(
        &mut self,
        tx: LedgerTxKind,
        now_micros: u64,
    ) -> Result<(StagedTicket, Vec<CommittedAck>), WalError> {
        self.submit_op(WalOp::ApplyTx(tx), now_micros)
    }

    /// Polls time-bounded flush (`<= 2.0ms` upper latency bound even when idle).
    pub fn poll_deadline_flush(&mut self, now_micros: u64) -> Result<Vec<CommittedAck>, WalError> {
        if let Some(oldest) = self.oldest_enqueued_micros
            && now_micros.saturating_sub(oldest) >= MAX_BATCH_LATENCY_MICROS
        {
            return self.flush_group_commit();
        }
        Ok(Vec::new())
    }

    /// Writes a single `CRC32C` + `BLAKE3` chained segment, executes `File::sync_all()` (`fsync`),
    /// and ONLY THEN promotes `B_staged -> B_committed` and returns external `CommittedAck`s.
    pub fn flush_group_commit(&mut self) -> Result<Vec<CommittedAck>, WalError> {
        if self.ring.is_empty() {
            self.oldest_enqueued_micros = None;
            return Ok(Vec::new());
        }

        let mut batch_tlv: Vec<TlvField> = Vec::with_capacity(self.ring.len());
        for entry in &self.ring {
            let encoded = match entry.op.encode_tlv(&entry.unknown_fields) {
                Some(b) => b,
                None => {
                    self.rollback_staged();
                    return Err(WalError::EncodingError);
                }
            };
            batch_tlv.push(TlvField {
                tag: TAG_BATCH_ENTRY,
                value: encoded,
            });
        }
        let payload = match encode_tlv_stream(&batch_tlv) {
            Ok(p) => p,
            Err(_) => {
                self.rollback_staged();
                return Err(WalError::EncodingError);
            }
        };
        if payload.len() > MAX_SEGMENT_PAYLOAD_BYTES {
            self.rollback_staged();
            return Err(WalError::EncodingError);
        }

        let seg_seq = self.next_segment_seq;
        let prev_hash = self.prev_chain_hash;
        let crc = compute_frame_crc(seg_seq, &prev_hash, payload.len() as u32, &payload);
        let chain_hash = compute_chain_hash(&prev_hash, seg_seq, crc, &payload);

        let mut frame = Vec::with_capacity(HEADER_LEN.saturating_add(payload.len()));
        frame.extend_from_slice(&WAL_MAGIC);
        frame.extend_from_slice(&seg_seq.to_le_bytes());
        frame.extend_from_slice(&prev_hash);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&crc.to_le_bytes());
        frame.extend_from_slice(&chain_hash);
        frame.extend_from_slice(&payload);

        let prev_valid_len = match self.wal_file.metadata() {
            Ok(m) => m.len(),
            Err(io_err) => {
                self.rollback_staged();
                return Err(WalError::Io(io_err));
            }
        };

        // Disk `fsync` MUST succeed before `self.committed` promotion or ACK emission!
        // On partial write or `fsync` failure, restore `wal_file` length to `prev_valid_len` immediately.
        if let Err(io_err) = self.write_and_fsync_frame(&frame) {
            let _ = self.wal_file.set_len(prev_valid_len);
            let _ = self.wal_file.seek(SeekFrom::Start(prev_valid_len));
            self.rollback_staged();
            return Err(WalError::Io(io_err));
        }

        // Atomic in-memory batch promotion (`B_staged -> B_committed`).
        self.committed = self.staged.clone();
        self.prev_chain_hash = chain_hash;
        self.next_segment_seq = self.next_segment_seq.saturating_add(1);
        self.oldest_enqueued_micros = None;

        let mut acks = Vec::with_capacity(self.ring.len());
        while let Some(entry) = self.ring.pop_front() {
            acks.push(CommittedAck {
                tx_seq: entry.tx_seq,
                segment_seq: seg_seq,
                chain_hash,
            });
        }
        Ok(acks)
    }

    /// Writes a durable periodic checkpoint (`checkpoint.bin`) after flushing any staged entries.
    pub fn write_checkpoint(&mut self) -> Result<(), WalError> {
        let _ = self.flush_group_commit()?;
        self.committed.verify_global_conservation()?;

        let env = CheckpointEnvelope {
            next_segment_seq: self.next_segment_seq,
            prev_chain_hash: self.prev_chain_hash,
            ledger: self.committed.clone(),
            next_tx_seq: self.next_tx_seq,
            unknown_fields: self.checkpoint_unknown_fields.clone(),
        };
        let body_bytes = env.encode_tlv()?;
        let crc = crc32c(&body_bytes);

        let tmp_path = self.checkpoint_path.with_extension("bin.tmp");
        {
            let mut tmp_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            tmp_file.write_all(&CHK_MAGIC)?;
            tmp_file.write_all(&crc.to_le_bytes())?;
            tmp_file.write_all(&body_bytes)?;
            tmp_file.sync_all()?;
        }
        fs::rename(&tmp_path, &self.checkpoint_path)?;
        Ok(())
    }

    /// Writes a durable checkpoint and rotates (`truncates`) `wal.log` cleanly so subsequent
    /// segments start directly at `(next_segment_seq, prev_chain_hash)`.
    pub fn write_checkpoint_and_rotate(&mut self) -> Result<(), WalError> {
        self.write_checkpoint()?;
        self.wal_file.set_len(0)?;
        self.wal_file.seek(SeekFrom::Start(0))?;
        self.wal_file.sync_all()?;
        Ok(())
    }

    fn write_and_fsync_frame(&mut self, frame: &[u8]) -> std::io::Result<()> {
        self.wal_file.seek(SeekFrom::End(0))?;
        self.wal_file.write_all(frame)?;
        self.wal_file.sync_all()
    }

    fn rollback_staged(&mut self) {
        self.staged = self.committed.clone();
        self.ring.clear();
        self.oldest_enqueued_micros = None;
    }
}

fn compute_frame_crc(seg_seq: u64, prev_hash: &[u8; 32], payload_len: u32, payload: &[u8]) -> u32 {
    let mut buf = Vec::with_capacity(52usize.saturating_add(payload.len()));
    buf.extend_from_slice(&WAL_MAGIC);
    buf.extend_from_slice(&seg_seq.to_le_bytes());
    buf.extend_from_slice(prev_hash);
    buf.extend_from_slice(&payload_len.to_le_bytes());
    buf.extend_from_slice(payload);
    crc32c(&buf)
}

fn compute_chain_hash(
    prev_hash: &[u8; 32],
    seg_seq: u64,
    crc: u32,
    payload: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(prev_hash);
    hasher.update(&seg_seq.to_le_bytes());
    hasher.update(&crc.to_le_bytes());
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

/// Declarative RON season/epoch manifest schema (`assets/manifests/E_<id>.ron`, `게임.md §3.1, §7.1, §9.2`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonEpochDiskManifest {
    pub epoch_id: u64,
    pub season_id: u32,
    pub elements: Vec<String>,
    pub weapon_modules: Vec<String>,
    pub boss_modules: Vec<String>,
    pub chunk_seeds: Vec<(i32, i32, u64)>,
    pub base_move_speed_milli: i32,
    pub expected_farm_ticks: u64,
    pub faucet_caps: [i64; 8],
}

/// Complete deterministically sorted season manifest + `gameplay/` `.qvm` source tree loaded from disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeasonGameplayPack {
    pub manifest: SeasonEpochDiskManifest,
    pub prelude_src: String,
    pub weapons: BTreeMap<String, String>,
    pub traits: BTreeMap<String, String>,
    pub augments: BTreeMap<String, String>,
    pub bosses: BTreeMap<String, String>,
    pub world_events: BTreeMap<String, String>,
    pub all_modules_rel: BTreeMap<String, String>,
    pub content_merkle: [u8; 32],
}

fn collect_qvm_subdir(
    gameplay_root: &Path,
    subdir_name: &str,
    out_cat: &mut BTreeMap<String, String>,
    out_all: &mut BTreeMap<String, String>,
) -> Result<(), WalError> {
    let sub = gameplay_root.join(subdir_name);
    if !sub.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = Vec::new();
    for ent_res in fs::read_dir(&sub)? {
        let ent = ent_res?;
        entries.push(ent.path());
    }
    entries.sort();
    for p in entries {
        if p.is_file() && p.extension().is_some_and(|e| e == "qvm") {
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let rel = format!("gameplay/{subdir_name}/{stem}.qvm");
            let src = fs::read_to_string(&p)?;
            out_cat.insert(stem, src.clone());
            out_all.insert(rel, src);
        }
    }
    Ok(())
}

/// Loads `assets/manifests/E_<id>.ron` and all sorted `gameplay/{prelude,weapons,traits,augments,bosses,world_events}/*.qvm`
/// scripts, computing their canonical `BLAKE3` content digest (`게임.md §5.2, §7.1, §9.2`).
pub fn load_season_gameplay_pack(
    manifest_path: &Path,
    gameplay_root: &Path,
) -> Result<SeasonGameplayPack, WalError> {
    let raw_ron = fs::read_to_string(manifest_path)?;
    let manifest: SeasonEpochDiskManifest =
        ron::from_str(&raw_ron).map_err(|_| WalError::ManifestParseError)?;
    let prelude_src = fs::read_to_string(gameplay_root.join("prelude.qvm"))?;

    let mut weapons = BTreeMap::new();
    let mut traits = BTreeMap::new();
    let mut augments = BTreeMap::new();
    let mut bosses = BTreeMap::new();
    let mut world_events = BTreeMap::new();
    let mut all_modules_rel = BTreeMap::new();

    collect_qvm_subdir(gameplay_root, "weapons", &mut weapons, &mut all_modules_rel)?;
    collect_qvm_subdir(gameplay_root, "traits", &mut traits, &mut all_modules_rel)?;
    collect_qvm_subdir(gameplay_root, "augments", &mut augments, &mut all_modules_rel)?;
    collect_qvm_subdir(gameplay_root, "bosses", &mut bosses, &mut all_modules_rel)?;
    collect_qvm_subdir(
        gameplay_root,
        "world_events",
        &mut world_events,
        &mut all_modules_rel,
    )?;

    let mut h = blake3::Hasher::new();
    h.update(b"SEASON_GAMEPLAY_PACK_V1");
    h.update(raw_ron.as_bytes());
    h.update(prelude_src.as_bytes());
    for (rel, src) in &all_modules_rel {
        h.update(rel.as_bytes());
        h.update(src.as_bytes());
    }
    let content_merkle = *h.finalize().as_bytes();

    Ok(SeasonGameplayPack {
        manifest,
        prelude_src,
        weapons,
        traits,
        augments,
        bosses,
        world_events,
        all_modules_rel,
        content_merkle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_wal_dir(label: &str) -> PathBuf {
        let mut p = if Path::new("../../Cargo.toml").is_file() {
            PathBuf::from("../../target")
        } else {
            PathBuf::from("target")
        };
        p.push("wal_unit_tests");
        p.push(label);
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn test_crc32c_standard_check_vector() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn test_intra_batch_double_spend_rejected_and_unflushed_invisible() {
        let dir = temp_wal_dir("double_spend");
        let (mut wal, _) = GroupCommitWal::open_and_recover(&dir, 8).unwrap_or_else(|_| unreachable!());
        let asset = AssetId(1);
        let u1 = AccountId(10);
        let u2 = AccountId(20);
        let u3 = AccountId(30);

        assert!(wal.submit_op(WalOp::ConfigureAssetCap { asset, epoch_cap: 1_000 }, 100).is_ok());
        assert!(wal.submit_op(WalOp::AllocateShardQuota { shard_id: 0, asset, quota: 500 }, 105).is_ok());
        assert!(wal.submit_tx(LedgerTxKind::FaucetMint { shard_id: 0, to: u1, asset, amount: 100 }, 110).is_ok());

        assert_eq!(wal.committed_ledger().balance(u1, asset), 0);
        assert_eq!(wal.staged_ledger().balance(u1, asset), 100);

        assert!(wal.submit_tx(LedgerTxKind::Transfer { from: u1, to: u2, asset, amount: 80 }, 120).is_ok());

        let conflict = wal.submit_tx(LedgerTxKind::Transfer { from: u1, to: u3, asset, amount: 50 }, 125);
        assert!(matches!(conflict, Err(WalError::Ledger(LedgerError::InsufficientBalance))));

        let acks = wal.poll_deadline_flush(2_200).unwrap_or_default();
        assert_eq!(acks.len(), 4);
        assert_eq!(wal.committed_ledger().balance(u1, asset), 20);
        assert_eq!(wal.committed_ledger().balance(u2, asset), 80);
        assert_eq!(wal.committed_ledger().balance(u3, asset), 0);
        assert!(wal.committed_ledger().verify_global_conservation().is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_crash_unflushed_drop_and_torn_tail_truncation_recovery() {
        let dir = temp_wal_dir("torn_tail");
        let asset = AssetId(7);
        let u1 = AccountId(1);
        let u2 = AccountId(2);
        let committed_hash_before_crash: [u8; 32];
        {
            let (mut wal, _) = GroupCommitWal::open_and_recover(&dir, 4).unwrap_or_else(|_| unreachable!());
            assert!(wal.submit_op(WalOp::ConfigureAssetCap { asset, epoch_cap: 10_000 }, 0).is_ok());
            assert!(wal.submit_op(WalOp::AllocateShardQuota { shard_id: 1, asset, quota: 5_000 }, 10).is_ok());
            assert!(wal.submit_tx(LedgerTxKind::FaucetMint { shard_id: 1, to: u1, asset, amount: 1_000 }, 20).is_ok());
            let (_, acks) = wal
                .submit_tx(LedgerTxKind::Transfer { from: u1, to: u2, asset, amount: 400 }, 30)
                .unwrap_or_else(|_| unreachable!());
            assert_eq!(acks.len(), 4);
            committed_hash_before_crash = wal.committed_ledger().state_hash();

            assert!(wal.write_checkpoint().is_ok());
            assert!(wal.submit_tx(LedgerTxKind::Transfer { from: u1, to: u2, asset, amount: 100 }, 50).is_ok());
            let acks2 = wal.flush_group_commit().unwrap_or_default();
            assert_eq!(acks2.len(), 1);

            assert!(wal.submit_tx(LedgerTxKind::Transfer { from: u2, to: u1, asset, amount: 250 }, 60).is_ok());
            assert_ne!(wal.committed_ledger().state_hash(), committed_hash_before_crash);
        }

        let wal_file_path = dir.join("wal.log");
        {
            let mut f = OpenOptions::new().append(true).open(&wal_file_path).unwrap_or_else(|_| unreachable!());
            assert!(f.write_all(b"GWAL0001\x99\x00\x00\x00TORN_CRASH_BYTES_UNFINISHED").is_ok());
            assert!(f.sync_all().is_ok());
        }

        let (recovered, rep) = GroupCommitWal::open_and_recover(&dir, 4).unwrap_or_else(|_| unreachable!());
        assert!(rep.loaded_checkpoint);
        assert_eq!(rep.truncated_tail_bytes, 39);
        assert_eq!(rep.replayed_segments, 1);
        assert_eq!(recovered.committed_ledger().balance(u1, asset), 500);
        assert_eq!(recovered.committed_ledger().balance(u2, asset), 500);
        assert!(recovered.committed_ledger().verify_global_conservation().is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rotated_wal_post_checkpoint_replay_and_unknown_tlv_preservation() {
        let dir = temp_wal_dir("rotated_post_checkpoint");
        let asset = AssetId(3);
        let u1 = AccountId(100);
        let u2 = AccountId(200);
        {
            let (mut wal, _) = GroupCommitWal::open_and_recover(&dir, 2).unwrap_or_else(|_| unreachable!());
            assert!(wal.submit_op(WalOp::ConfigureAssetCap { asset, epoch_cap: 5_000 }, 0).is_ok());
            assert!(wal.submit_op(WalOp::AllocateShardQuota { shard_id: 0, asset, quota: 2_000 }, 5).is_ok());
            assert!(wal.submit_tx(LedgerTxKind::FaucetMint { shard_id: 0, to: u1, asset, amount: 800 }, 10).is_ok());
            assert!(wal.write_checkpoint_and_rotate().is_ok());

            // Now `wal.log` is rotated (`0` bytes) and next segment starts at `seg_seq >= 2`!
            let extra = vec![TlvField { tag: 777, value: vec![1, 2, 3, 4] }];
            assert!(
                wal.submit_op_with_unknown_fields(
                    WalOp::ApplyTx(LedgerTxKind::Transfer {
                        from: u1,
                        to: u2,
                        asset,
                        amount: 300,
                    }),
                    extra,
                    20,
                )
                .is_ok()
            );
            assert!(wal.flush_group_commit().is_ok());
        }

        let (recovered, rep) = GroupCommitWal::open_and_recover(&dir, 2).unwrap_or_else(|_| unreachable!());
        assert!(rep.loaded_checkpoint);
        assert_eq!(rep.truncated_tail_bytes, 0);
        assert_eq!(rep.replayed_segments, 1);
        assert_eq!(rep.replayed_ops, 1);
        assert_eq!(recovered.committed_ledger().balance(u1, asset), 500);
        assert_eq!(recovered.committed_ledger().balance(u2, asset), 300);
        assert!(recovered.committed_ledger().verify_global_conservation().is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
