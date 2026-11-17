#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use verifier_abi::SealedLedger;
use world_ledger::{EpisodeManifest, LedgerError};

const WAL_FILE_HEADER: &[u8; 8] = b"AGYWAL01";
const RECORD_MAGIC: u32 = 0x5741_4C31; // "WAL1"
// Record header layout (84 bytes):
// [0..4]   magic: u32 LE
// [4..12]  seq: u64 LE
// [12..44] prev_blake3: [u8; 32]
// [44..48] payload_len: u32 LE
// [48..52] crc32c: u32 LE
// [52..84] record_blake3: [u8; 32]
const RECORD_HEADER_LEN: usize = 84;

#[derive(Debug)]
pub enum WalError {
    Io(std::io::Error),
    InvalidFileHeader,
    MidStreamChainTampered { seq: u64 },
    ManifestDecode(LedgerError),
    RonEncode(ron::Error),
    RonDecode(ron::error::SpannedError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub valid_records: u64,
    pub truncated_tail_bytes: u64,
    pub head_blake3: [u8; 32],
}

/// Pure-Rust Castagnoli `CRC32C` (`0x82F63B78` reflected polynomial).
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        let mut bit: u8 = 0;
        while bit < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = crc.wrapping_shr(1) ^ (0x82F6_3B78 & mask);
            bit = bit.saturating_add(1);
        }
    }
    !crc
}

fn compute_record_digests(seq: u64, prev_hash: &[u8; 32], payload: &[u8]) -> (u32, [u8; 32]) {
    let mut crc_buf = Vec::with_capacity(payload.len().saturating_add(44));
    crc_buf.extend_from_slice(&seq.to_le_bytes());
    crc_buf.extend_from_slice(prev_hash);
    crc_buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    crc_buf.extend_from_slice(payload);
    let crc = crc32c(&crc_buf);

    let mut hasher = blake3::Hasher::new();
    hasher.update(&seq.to_le_bytes());
    hasher.update(prev_hash);
    hasher.update(payload);
    let rec_hash = *hasher.finalize().as_bytes();
    (crc, rec_hash)
}

/// Append-Only Write-Ahead Log with `CRC32C` + `BLAKE3` hash chain, automatic torn-tail recovery,
/// and lock-free `ArcSwap` dual-slot sealed manifest mount (`§4.3`, `§9.1`).
pub struct LedgerWal {
    path: PathBuf,
    next_seq: u64,
    head_hash: [u8; 32],
    active_manifest: ArcSwap<SealedLedger<EpisodeManifest>>,
}

impl LedgerWal {
    /// Opens an existing WAL or initializes a new one, automatically truncating any torn/incomplete trailing segment
    /// with `0` error on committed history (`§9.1`).
    pub fn open_or_recover(path: impl AsRef<Path>) -> Result<(Self, RecoveryReport), WalError> {
        let wal_path = path.as_ref().to_path_buf();
        if !wal_path.exists() {
            if let Some(parent) = wal_path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).map_err(WalError::Io)?;
            }
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&wal_path)
                .map_err(WalError::Io)?;
            f.write_all(WAL_FILE_HEADER).map_err(WalError::Io)?;
            f.sync_all().map_err(WalError::Io)?;

            let genesis = EpisodeManifest::default().seal();
            let report = RecoveryReport {
                valid_records: 0,
                truncated_tail_bytes: 0,
                head_blake3: [0_u8; 32],
            };
            return Ok((
                Self {
                    path: wal_path,
                    next_seq: 0,
                    head_hash: [0_u8; 32],
                    active_manifest: ArcSwap::from_pointee(genesis),
                },
                report,
            ));
        }

        let mut raw = Vec::new();
        {
            let mut f = File::open(&wal_path).map_err(WalError::Io)?;
            f.read_to_end(&mut raw).map_err(WalError::Io)?;
        }

        if raw.len() < WAL_FILE_HEADER.len() {
            return Err(WalError::InvalidFileHeader);
        }
        if raw.get(..8) != Some(WAL_FILE_HEADER.as_slice()) {
            return Err(WalError::InvalidFileHeader);
        }

        let mut valid_offset: usize = 8;
        let mut expected_seq: u64 = 0;
        let mut prev_hash = [0_u8; 32];
        let mut latest_sealed = EpisodeManifest::default().seal();

        while valid_offset < raw.len() {
            let hdr_end = valid_offset.saturating_add(RECORD_HEADER_LEN);
            let Some(hdr) = raw.get(valid_offset..hdr_end) else {
                // Incomplete trailing header -> torn tail
                break;
            };
            let magic = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
            if magic != RECORD_MAGIC {
                // Torn tail garbage at end of log
                break;
            }
            let seq = u64::from_le_bytes([
                hdr[4], hdr[5], hdr[6], hdr[7], hdr[8], hdr[9], hdr[10], hdr[11],
            ]);
            let mut rec_prev = [0_u8; 32];
            rec_prev.copy_from_slice(&hdr[12..44]);
            let payload_len = u32::from_le_bytes([hdr[44], hdr[45], hdr[46], hdr[47]]) as usize;
            let expected_crc = u32::from_le_bytes([hdr[48], hdr[49], hdr[50], hdr[51]]);
            let mut expected_b3 = [0_u8; 32];
            expected_b3.copy_from_slice(&hdr[52..84]);

            let rec_end = hdr_end.saturating_add(payload_len);
            let Some(payload) = raw.get(hdr_end..rec_end) else {
                // Truncated trailing payload -> torn tail
                break;
            };

            let (actual_crc, actual_b3) = compute_record_digests(seq, &rec_prev, payload);
            if actual_crc != expected_crc || actual_b3 != expected_b3 {
                // Check whether another valid record header follows: if so, mid-stream tampering; else torn tail!
                let has_subsequent_magic = raw
                    .get(rec_end..rec_end.saturating_add(4))
                    .is_some_and(|m| {
                        u32::from_le_bytes([m[0], m[1], m[2], m[3]]) == RECORD_MAGIC
                    });
                if has_subsequent_magic {
                    return Err(WalError::MidStreamChainTampered { seq });
                }
                break;
            }

            if seq != expected_seq || rec_prev != prev_hash {
                return Err(WalError::MidStreamChainTampered { seq });
            }

            let decoded = EpisodeManifest::decode_tlv(payload).map_err(WalError::ManifestDecode)?;
            latest_sealed = decoded.seal();
            prev_hash = actual_b3;
            expected_seq = expected_seq.saturating_add(1);
            valid_offset = rec_end;
        }

        let truncated_bytes = (raw.len().saturating_sub(valid_offset)) as u64;
        if truncated_bytes > 0 {
            let f = OpenOptions::new()
                .write(true)
                .open(&wal_path)
                .map_err(WalError::Io)?;
            f.set_len(valid_offset as u64).map_err(WalError::Io)?;
            f.sync_all().map_err(WalError::Io)?;
        }

        let report = RecoveryReport {
            valid_records: expected_seq,
            truncated_tail_bytes: truncated_bytes,
            head_blake3: prev_hash,
        };
        Ok((
            Self {
                path: wal_path,
                next_seq: expected_seq,
                head_hash: prev_hash,
                active_manifest: ArcSwap::from_pointee(latest_sealed),
            },
            report,
        ))
    }

    /// Appends a new `EpisodeManifest` deterministically (`fsync` before swapping `ArcSwap` pointer).
    pub fn append_manifest(&mut self, manifest: EpisodeManifest) -> Result<[u8; 32], WalError> {
        let payload = manifest.encode_tlv();
        let seq = self.next_seq;
        let (crc, rec_hash) = compute_record_digests(seq, &self.head_hash, &payload);

        let mut rec_bytes = Vec::with_capacity(RECORD_HEADER_LEN.saturating_add(payload.len()));
        rec_bytes.extend_from_slice(&RECORD_MAGIC.to_le_bytes());
        rec_bytes.extend_from_slice(&seq.to_le_bytes());
        rec_bytes.extend_from_slice(&self.head_hash);
        rec_bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        rec_bytes.extend_from_slice(&crc.to_le_bytes());
        rec_bytes.extend_from_slice(&rec_hash);
        rec_bytes.extend_from_slice(&payload);

        let mut f = OpenOptions::new()
            .write(true)
            .open(&self.path)
            .map_err(WalError::Io)?;
        f.seek(SeekFrom::End(0)).map_err(WalError::Io)?;
        f.write_all(&rec_bytes).map_err(WalError::Io)?;
        f.sync_all().map_err(WalError::Io)?;

        let sealed = manifest.seal();
        self.active_manifest.store(Arc::new(sealed));
        self.head_hash = rec_hash;
        self.next_seq = self.next_seq.saturating_add(1);
        Ok(rec_hash)
    }

    #[inline]
    pub fn current_sealed(&self) -> Arc<SealedLedger<EpisodeManifest>> {
        self.active_manifest.load_full()
    }

    #[inline]
    pub const fn next_sequence(&self) -> u64 {
        self.next_seq
    }

    /// Writes a human-auditable `.ron` snapshot (`season/manifests/E_<id>.ron`).
    pub fn write_ron_snapshot(
        manifest: &EpisodeManifest,
        target: impl AsRef<Path>,
    ) -> Result<(), WalError> {
        let text = ron::ser::to_string_pretty(manifest, ron::ser::PrettyConfig::default())
            .map_err(WalError::RonEncode)?;
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(target.as_ref())
            .map_err(WalError::Io)?;
        f.write_all(text.as_bytes()).map_err(WalError::Io)?;
        f.sync_all().map_err(WalError::Io)?;
        Ok(())
    }
}
