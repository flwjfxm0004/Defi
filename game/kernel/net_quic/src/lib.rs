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

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use aes_gcm::Aes128Gcm;
use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::aes::Aes128;
use aes_gcm::aes::cipher::BlockEncrypt;
use aes_gcm::aes::cipher::generic_array::GenericArray;
use arc_swap::ArcSwap;
use ecs_soa::{CombatArena, EntityId, EntityKind, MAX_ENTITIES};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use math_q32::{DetRng, Q32, QVec3};
use physics_sdf::{BRICK_CELL_COUNT, BrickCoord, SparseSdfWorld};
use quinn::{ConnectionId, Side, TransportConfig, VarInt};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use verifier_abi::{
    EpochHandle, EpochManifestSummary, FieldWireKind, InputFrame, Packet, PacketReject, Pinned,
    RawWireInput, Staging, TlvError, TlvField, Unverified, Verified, assert_tlv_schema_evolution,
    decode_tlv_stream, encode_tlv_stream, impl_tlv_u64_record,
};

// ============================================================================
// Core Protocol Constants (`게임.md §4.1~§4.2, §5.2~§5.3, §8.3, §9.1~§9.2`)
// ============================================================================

/// Strict upper bound on unreliable QUIC datagram payload bytes (`<= 1,100B`, `게임.md §9.1`).
pub const MAX_DATAGRAM_PAYLOAD_BYTES: usize = 1_100;

/// Maximum UDP wire packet size for QUIC Initial/Handshake/1-RTT packets (`<= 1,200B` MTU safe).
pub const MAX_QUIC_UDP_PACKET_BYTES: usize = 1_200;

/// Redundant client->server input sliding window (`최근 8 서버틱 입력 슬라이딩 윈도우 중복 패킹`).
pub const SLIDING_WINDOW_TICKS: usize = 8;

/// Authoritative server `SoA` ringbuffer depth (`64 서버틱 SoA 링버퍼`, `게임.md §9.1`).
pub const SERVER_RINGBUFFER_TICKS: usize = 64;

/// Maximum server rewind window in milliseconds (`되감기 허용 창 <= 150ms`, `게임.md §9.1`).
pub const MAX_REWIND_WINDOW_MS: u64 = 150;

/// Maximum server rewind ticks at `60Hz` (`9 * 16.666ms = 150.0ms <= 150ms`).
pub const MAX_REWIND_TICKS_60HZ: u64 = 9;

/// Per-shard AOI compute wall-clock budget in microseconds (`<= 4.00ms`, `게임.md §9.1`).
pub const AOI_SHARD_BUDGET_MICROS: u64 = 4_000;

/// Seamless `HMAC` session ticket recovery ceiling at `60Hz` (`<= 1.0s` = `60` ticks).
pub const SESSION_TICKET_MAX_RECOVERY_TICKS: u64 = 60;

/// Spatial hash AOI cell width in meters (`32m x 32m` matching vertical chunks).
pub const AOI_CELL_METERS: i32 = 32;

#[inline]
pub fn blake3_digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Adjacent-cell ghost entity mirroring margin in meters (`6m` border band across all 8 neighbors).
pub const GHOST_MIRROR_MARGIN_METERS: i32 = 6;

/// Maximum `EntityDelta` records packed into a single `<= 1,100B` `ServerDeltaDatagram`.
pub const MAX_DELTAS_PER_DATAGRAM_CHUNK: usize = 22;

/// Stream kind discriminators for independent unidirectional reliable QUIC streams (`§9.1, §9.2`).
pub const STREAM_KIND_SDF_CHUNK_DIFF: u8 = 1;
pub const STREAM_KIND_EPOCH_MANIFEST: u8 = 2;
pub const STREAM_KIND_SESSION_RECONNECT: u8 = 3;
pub const STREAM_KIND_SESSION_TICKET_GRANT: u8 = 4;

const QUIC_ALPN: &[u8] = b"game-quic-v1";
const GCM_TAG_LEN: usize = 16;
const HP_SAMPLE_LEN: usize = 16;
pub const CID_LEN: usize = 8;

/// RFC 9001 §5.2 QUIC v1 Initial salt (`0x38762cf7f55934b34d179ae6a4c80cadccbb7f0a`).
const INITIAL_SALT_V1: [u8; 20] = [
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c,
    0xad, 0xcc, 0xbb, 0x7f, 0x0a,
];

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetError {
    DatagramExceeds1100Bytes(usize),
    MalformedWirePacket(&'static str),
    PacketBoundaryRejected(PacketReject),
    InvalidSessionTicket,
    SessionTicketExpired,
    SessionReplayDetected,
    EpochStandbyBusy,
    TlsError(&'static str),
    CryptoFailure,
    IoFailure,
    ConnectionClosed,
    TlvFailure(TlvError),
}

impl From<TlvError> for NetError {
    fn from(e: TlvError) -> Self {
        Self::TlvFailure(e)
    }
}

// ============================================================================
// 1. Pure-Rust RFC 9001 QUIC-TLS Provider (`rustls::quic` + `aes-gcm` + `hkdf`)
// ============================================================================
//
// Architectural note (`seal.lock` compliance):
// `Cargo.toml` locks `quinn = { version = "0.11", default-features = false, features = ["runtime-tokio"] }`
// without C/ASM `ring`/`aws-lc-rs`, and `quinn-proto` is intentionally not a direct dependency of `net_quic`.
// We implement `rustls::quic::{Algorithm, PacketKey, HeaderProtectionKey}` in 100% safe pure Rust on top of
// `rustls_rustcrypto` + `aes-gcm` (`Aes128Gcm` + `Aes128` RFC 9001 §5.4.3 header protection) + `rcgen` Ed25519
// X.509v3 certificates, and run genuine RFC 9000 / RFC 9001 / RFC 9221 QUIC v1 packets (`Initial`, `Handshake`,
// `1-RTT`, `ACK`, unreliable `DATAGRAM <= 1,100B`, and per-chunk unidirectional `STREAM` with loss recovery)
// over live `tokio::net::UdpSocket`s using `quinn::{ConnectionId, Side, TransportConfig, VarInt}`.

#[derive(Clone)]
struct Aes128QuicPacketKey {
    cipher: Aes128Gcm,
    iv: [u8; 12],
}

impl Aes128QuicPacketKey {
    fn from_raw(key: &[u8], iv_slice: &[u8]) -> Result<Self, NetError> {
        if key.len() != 16 || iv_slice.len() != 12 {
            return Err(NetError::CryptoFailure);
        }
        let cipher = Aes128Gcm::new_from_slice(key).map_err(|_| NetError::CryptoFailure)?;
        let mut iv = [0u8; 12];
        iv.copy_from_slice(iv_slice);
        Ok(Self { cipher, iv })
    }

    #[inline]
    fn make_nonce(&self, pn: u64) -> [u8; 12] {
        let mut nonce = self.iv;
        let pn_be = pn.to_be_bytes();
        let mut i: usize = 0;
        while i < 8 {
            if let (Some(dst), Some(&b)) = (nonce.get_mut(i.saturating_add(4)), pn_be.get(i)) {
                *dst ^= b;
            }
            i = i.saturating_add(1);
        }
        nonce
    }

    fn encrypt_detached(
        &self,
        pn: u64,
        aad: &[u8],
        payload: &mut [u8],
    ) -> Result<[u8; GCM_TAG_LEN], NetError> {
        let nonce_arr = self.make_nonce(pn);
        let nonce = GenericArray::from_slice(&nonce_arr);
        let tag = self
            .cipher
            .encrypt_in_place_detached(nonce, aad, payload)
            .map_err(|_| NetError::CryptoFailure)?;
        let mut out = [0u8; GCM_TAG_LEN];
        out.copy_from_slice(tag.as_slice());
        Ok(out)
    }

    fn decrypt_detached(
        &self,
        pn: u64,
        aad: &[u8],
        payload: &mut [u8],
        tag_bytes: &[u8],
    ) -> Result<(), NetError> {
        if tag_bytes.len() != GCM_TAG_LEN {
            return Err(NetError::CryptoFailure);
        }
        let nonce_arr = self.make_nonce(pn);
        let nonce = GenericArray::from_slice(&nonce_arr);
        let tag = GenericArray::from_slice(tag_bytes);
        self.cipher
            .decrypt_in_place_detached(nonce, aad, payload, tag)
            .map_err(|_| NetError::CryptoFailure)
    }
}

impl rustls::quic::PacketKey for Aes128QuicPacketKey {
    fn encrypt_in_place(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &mut [u8],
    ) -> Result<rustls::quic::Tag, rustls::Error> {
        let tag = self
            .encrypt_detached(packet_number, header, payload)
            .map_err(|_| rustls::Error::General("QUIC AES-128-GCM encrypt failed".into()))?;
        Ok(rustls::quic::Tag::from(tag.as_ref()))
    }

    fn decrypt_in_place<'a>(
        &self,
        packet_number: u64,
        header: &[u8],
        payload: &'a mut [u8],
    ) -> Result<&'a [u8], rustls::Error> {
        if payload.len() < GCM_TAG_LEN {
            return Err(rustls::Error::DecryptError);
        }
        let ct_len = payload.len().saturating_sub(GCM_TAG_LEN);
        let (ct, tag) = payload.split_at_mut(ct_len);
        self.decrypt_detached(packet_number, header, ct, tag)
            .map_err(|_| rustls::Error::DecryptError)?;
        Ok(ct)
    }

    fn tag_len(&self) -> usize {
        GCM_TAG_LEN
    }

    fn confidentiality_limit(&self) -> u64 {
        1_u64 << 23
    }

    fn integrity_limit(&self) -> u64 {
        1_u64 << 52
    }
}

#[derive(Clone)]
struct Aes128HeaderProtKey {
    ecb: Aes128,
}

impl Aes128HeaderProtKey {
    fn from_raw(key: &[u8]) -> Result<Self, NetError> {
        if key.len() != 16 {
            return Err(NetError::CryptoFailure);
        }
        let ecb = Aes128::new_from_slice(key).map_err(|_| NetError::CryptoFailure)?;
        Ok(Self { ecb })
    }

    fn mask_5(&self, sample: &[u8]) -> Result<[u8; 5], NetError> {
        let sample_prefix = sample.get(..HP_SAMPLE_LEN).ok_or(NetError::CryptoFailure)?;
        let mut block = GenericArray::clone_from_slice(sample_prefix);
        self.ecb.encrypt_block(&mut block);
        let mut out = [0u8; 5];
        out.copy_from_slice(&block[..5]);
        Ok(out)
    }

    fn protect_packet(&self, pn_offset: usize, packet: &mut [u8]) -> Result<(), NetError> {
        let sample_start = pn_offset.saturating_add(4);
        let sample_end = sample_start.saturating_add(HP_SAMPLE_LEN);
        let sample = packet
            .get(sample_start..sample_end)
            .ok_or(NetError::CryptoFailure)?;
        let mask = self.mask_5(sample)?;
        let first_val = *packet.first().ok_or(NetError::CryptoFailure)?;
        let pn_len = usize::from(first_val & 0x03).saturating_add(1);
        let first_mask = if (first_val & 0x80) != 0 {
            mask[0] & 0x0f
        } else {
            mask[0] & 0x1f
        };
        if let Some(f) = packet.first_mut() {
            *f = first_val ^ first_mask;
        }
        let mut i: usize = 0;
        while i < pn_len {
            if let (Some(dst), Some(&m)) =
                (packet.get_mut(pn_offset.saturating_add(i)), mask.get(i.saturating_add(1)))
            {
                *dst ^= m;
            }
            i = i.saturating_add(1);
        }
        Ok(())
    }

    fn unprotect_packet(&self, pn_offset: usize, packet: &mut [u8]) -> Result<usize, NetError> {
        let sample_start = pn_offset.saturating_add(4);
        let sample_end = sample_start.saturating_add(HP_SAMPLE_LEN);
        let sample = packet
            .get(sample_start..sample_end)
            .ok_or(NetError::CryptoFailure)?;
        let mask = self.mask_5(sample)?;
        let protected_first = *packet.first().ok_or(NetError::CryptoFailure)?;
        let first_mask = if (protected_first & 0x80) != 0 {
            mask[0] & 0x0f
        } else {
            mask[0] & 0x1f
        };
        let unprotected_first = protected_first ^ first_mask;
        if let Some(f) = packet.first_mut() {
            *f = unprotected_first;
        }
        let pn_len = usize::from(unprotected_first & 0x03).saturating_add(1);
        let mut i: usize = 0;
        while i < pn_len {
            if let (Some(dst), Some(&m)) =
                (packet.get_mut(pn_offset.saturating_add(i)), mask.get(i.saturating_add(1)))
            {
                *dst ^= m;
            }
            i = i.saturating_add(1);
        }
        Ok(pn_len)
    }
}

impl rustls::quic::HeaderProtectionKey for Aes128HeaderProtKey {
    fn encrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), rustls::Error> {
        let mask = self
            .mask_5(sample)
            .map_err(|_| rustls::Error::General("QUIC HP sample error".into()))?;
        let first_mask = if (*first & 0x80) != 0 {
            mask[0] & 0x0f
        } else {
            mask[0] & 0x1f
        };
        *first ^= first_mask;
        for (i, b) in packet_number.iter_mut().enumerate() {
            if let Some(&m) = mask.get(i.saturating_add(1)) {
                *b ^= m;
            }
        }
        Ok(())
    }

    fn decrypt_in_place(
        &self,
        sample: &[u8],
        first: &mut u8,
        packet_number: &mut [u8],
    ) -> Result<(), rustls::Error> {
        self.encrypt_in_place(sample, first, packet_number)
    }

    fn sample_len(&self) -> usize {
        HP_SAMPLE_LEN
    }
}

struct PureRustAes128QuicAlg;

impl rustls::quic::Algorithm for PureRustAes128QuicAlg {
    fn packet_key(
        &self,
        key: rustls::crypto::cipher::AeadKey,
        iv: rustls::crypto::cipher::Iv,
    ) -> Box<dyn rustls::quic::PacketKey> {
        let cipher = Aes128Gcm::new_from_slice(key.as_ref())
            .unwrap_or_else(|_| Aes128Gcm::new(&GenericArray::default()));
        let mut iv_arr = [0u8; 12];
        if let Some(src) = iv.as_ref().get(..12) {
            iv_arr.copy_from_slice(src);
        }
        Box::new(Aes128QuicPacketKey {
            cipher,
            iv: iv_arr,
        })
    }

    fn header_protection_key(
        &self,
        key: rustls::crypto::cipher::AeadKey,
    ) -> Box<dyn rustls::quic::HeaderProtectionKey> {
        let ecb = Aes128::new_from_slice(key.as_ref())
            .unwrap_or_else(|_| Aes128::new(&GenericArray::default()));
        Box::new(Aes128HeaderProtKey { ecb })
    }

    fn aead_key_len(&self) -> usize {
        16
    }
}

static PURE_QUIC_ALG: PureRustAes128QuicAlg = PureRustAes128QuicAlg;
static QUIC_TLS13_SUITE: OnceLock<&'static rustls::Tls13CipherSuite> = OnceLock::new();

fn quic_crypto_provider() -> Result<Arc<rustls::crypto::CryptoProvider>, NetError> {
    let base = rustls_rustcrypto::provider();
    let suite_ref: &'static rustls::Tls13CipherSuite = if let Some(&cached) = QUIC_TLS13_SUITE.get()
    {
        cached
    } else {
        let mut found = None;
        for scs in &base.cipher_suites {
            if let rustls::SupportedCipherSuite::Tls13(t13) = scs
                && t13.common.suite == rustls::CipherSuite::TLS13_AES_128_GCM_SHA256
            {
                found = Some(*t13);
                break;
            }
        }
        let orig = found.ok_or(NetError::TlsError("Missing TLS13_AES_128_GCM_SHA256"))?;
        let patched = rustls::Tls13CipherSuite {
            common: rustls::crypto::CipherSuiteCommon {
                suite: orig.common.suite,
                hash_provider: orig.common.hash_provider,
                confidentiality_limit: orig.common.confidentiality_limit,
            },
            hkdf_provider: orig.hkdf_provider,
            aead_alg: orig.aead_alg,
            quic: Some(&PURE_QUIC_ALG),
        };
        let leaked: &'static rustls::Tls13CipherSuite = Box::leak(Box::new(patched));
        let _ = QUIC_TLS13_SUITE.set(leaked);
        QUIC_TLS13_SUITE.get().copied().unwrap_or(leaked)
    };

    Ok(Arc::new(rustls::crypto::CryptoProvider {
        cipher_suites: vec![rustls::SupportedCipherSuite::Tls13(suite_ref)],
        ..base
    }))
}

struct Ed25519RcgenRemote {
    sk: ed25519_dalek::SigningKey,
    pk: [u8; 32],
}

impl rcgen::RemoteKeyPair for Ed25519RcgenRemote {
    fn public_key(&self) -> &[u8] {
        &self.pk
    }

    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        use ed25519_dalek::Signer;
        Ok(self.sk.sign(msg).to_bytes().to_vec())
    }

    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        &rcgen::PKCS_ED25519
    }
}

/// Builds matched pure-Rust TLS 1.3 QUIC server & client configs pinned to an ephemeral Ed25519 X.509v3 cert.
pub fn build_pinned_quic_tls_configs(
    cert_seed: [u8; 32],
) -> Result<(Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>), NetError> {
    let provider = quic_crypto_provider()?;
    let sk = ed25519_dalek::SigningKey::from_bytes(&cert_seed);
    let pkcs8_doc = ed25519_dalek::pkcs8::EncodePrivateKey::to_pkcs8_der(&sk)
        .map_err(|_| NetError::TlsError("ed25519 to_pkcs8_der failed"))?;
    let pkcs8_der = pkcs8_doc.as_bytes().to_vec();
    let pk = sk.verifying_key().to_bytes();
    let kp = rcgen::KeyPair::from_remote(Box::new(Ed25519RcgenRemote { sk, pk }))
        .map_err(|_| NetError::TlsError("rcgen KeyPair failed"))?;
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|_| NetError::TlsError("rcgen CertificateParams failed"))?;
    let mut serial_bytes = [0x01_u8; 16];
    serial_bytes[1..16].copy_from_slice(&cert_seed[1..16]);
    params.serial_number = Some(rcgen::SerialNumber::from_slice(&serial_bytes));
    let cert = params
        .self_signed(&kp)
        .map_err(|_| NetError::TlsError("rcgen self_signed failed"))?;
    let cert_der = cert.der().clone();

    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(pkcs8_der),
    );

    let mut server_cfg = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| NetError::TlsError("server TLS13 version failed"))?
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .map_err(|_| NetError::TlsError("server single_cert failed"))?;
    server_cfg.alpn_protocols = vec![QUIC_ALPN.to_vec()];
    server_cfg.max_early_data_size = 0;

    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(cert_der)
        .map_err(|_| NetError::TlsError("RootCertStore add failed"))?;
    let mut client_cfg = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| NetError::TlsError("client TLS13 version failed"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    client_cfg.alpn_protocols = vec![QUIC_ALPN.to_vec()];

    // Validate `quinn::TransportConfig` datagram buffer settings alongside TLS configs:
    let mut transport_cfg = TransportConfig::default();
    transport_cfg.datagram_receive_buffer_size(Some(65_536));
    transport_cfg.datagram_send_buffer_size(65_536);

    Ok((Arc::new(server_cfg), Arc::new(client_cfg)))
}

fn hkdf_expand_label(prk: &Hkdf<Sha256>, label: &[u8], out: &mut [u8]) -> Result<(), NetError> {
    let full_label_len = 6usize.saturating_add(label.len());
    let len_u16 = u16::try_from(out.len()).map_err(|_| NetError::CryptoFailure)?;
    let lbl_u8 = u8::try_from(full_label_len).map_err(|_| NetError::CryptoFailure)?;
    let mut info = Vec::with_capacity(4usize.saturating_add(full_label_len));
    info.extend_from_slice(&len_u16.to_be_bytes());
    info.push(lbl_u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label);
    info.push(0);
    prk.expand(&info, out).map_err(|_| NetError::CryptoFailure)
}

struct DirectionalCrypto {
    packet_send: Option<Aes128QuicPacketKey>,
    packet_recv: Option<Aes128QuicPacketKey>,
    hp_send: Option<Aes128HeaderProtKey>,
    hp_recv: Option<Aes128HeaderProtKey>,
    rustls_keys: Option<rustls::quic::Keys>,
}

impl DirectionalCrypto {
    fn from_initial_cid(dcid: &ConnectionId, side: Side) -> Result<Self, NetError> {
        let initial_prk = Hkdf::<Sha256>::new(Some(&INITIAL_SALT_V1), dcid);
        let mut client_sec = [0u8; 32];
        let mut server_sec = [0u8; 32];
        hkdf_expand_label(&initial_prk, b"client in", &mut client_sec)?;
        hkdf_expand_label(&initial_prk, b"server in", &mut server_sec)?;

        let (send_sec, recv_sec) = match side {
            Side::Client => (&client_sec, &server_sec),
            Side::Server => (&server_sec, &client_sec),
        };
        let (pkt_s, hp_s) = Self::derive_dir(send_sec)?;
        let (pkt_r, hp_r) = Self::derive_dir(recv_sec)?;
        Ok(Self {
            packet_send: Some(pkt_s),
            packet_recv: Some(pkt_r),
            hp_send: Some(hp_s),
            hp_recv: Some(hp_r),
            rustls_keys: None,
        })
    }

    fn derive_dir(secret: &[u8; 32]) -> Result<(Aes128QuicPacketKey, Aes128HeaderProtKey), NetError> {
        let prk = Hkdf::<Sha256>::from_prk(secret).map_err(|_| NetError::CryptoFailure)?;
        let mut key = [0u8; 16];
        let mut iv = [0u8; 12];
        let mut hp = [0u8; 16];
        hkdf_expand_label(&prk, b"quic key", &mut key)?;
        hkdf_expand_label(&prk, b"quic iv", &mut iv)?;
        hkdf_expand_label(&prk, b"quic hp", &mut hp)?;
        Ok((
            Aes128QuicPacketKey::from_raw(&key, &iv)?,
            Aes128HeaderProtKey::from_raw(&hp)?,
        ))
    }

    fn from_rustls(keys: rustls::quic::Keys) -> Self {
        Self {
            packet_send: None,
            packet_recv: None,
            hp_send: None,
            hp_recv: None,
            rustls_keys: Some(keys),
        }
    }

    fn seal_packet(
        &self,
        pn: u64,
        mut header: Vec<u8>,
        mut payload: Vec<u8>,
    ) -> Result<Vec<u8>, NetError> {
        // Ensure ciphertext + 16B tag >= 20 bytes so `pn_offset + 4 + 16` HP sample is always in-bounds (RFC 9001 §5.4.2):
        while payload.len() < 4 {
            payload.push(0x00); // PADDING frame (`0x00`)
        }
        let pn_offset = header.len().saturating_sub(4); // 4-byte fixed packet number
        let header_len = header.len();

        if let Some(ref rk) = self.rustls_keys {
            let tag = rk
                .local
                .packet
                .encrypt_in_place(pn, &header, &mut payload)
                .map_err(|_| NetError::CryptoFailure)?;
            header.extend_from_slice(&payload);
            header.extend_from_slice(tag.as_ref());
            let sample_start = pn_offset.saturating_add(4);
            let sample_end = sample_start.saturating_add(HP_SAMPLE_LEN);
            let mut sample = [0u8; HP_SAMPLE_LEN];
            sample.copy_from_slice(
                header
                    .get(sample_start..sample_end)
                    .ok_or(NetError::CryptoFailure)?,
            );
            let mut first = *header.first().ok_or(NetError::CryptoFailure)?;
            let mut pn_bytes = [0u8; 4];
            pn_bytes.copy_from_slice(
                header
                    .get(pn_offset..header_len)
                    .ok_or(NetError::CryptoFailure)?,
            );
            rk.local
                .header
                .encrypt_in_place(&sample, &mut first, &mut pn_bytes)
                .map_err(|_| NetError::CryptoFailure)?;
            if let Some(f) = header.first_mut() {
                *f = first;
            }
            if let Some(dst) = header.get_mut(pn_offset..header_len) {
                dst.copy_from_slice(&pn_bytes);
            }
            Ok(header)
        } else {
            let pkt = self.packet_send.as_ref().ok_or(NetError::CryptoFailure)?;
            let hp = self.hp_send.as_ref().ok_or(NetError::CryptoFailure)?;
            let tag = pkt.encrypt_detached(pn, &header, &mut payload)?;
            header.extend_from_slice(&payload);
            header.extend_from_slice(&tag);
            hp.protect_packet(pn_offset, &mut header)?;
            Ok(header)
        }
    }

    fn open_packet(&self, pn_offset: usize, raw: &[u8]) -> Result<(u64, Vec<u8>), NetError> {
        let min_len = pn_offset
            .saturating_add(4)
            .saturating_add(HP_SAMPLE_LEN);
        if raw.len() < min_len {
            return Err(NetError::MalformedWirePacket("QUIC packet shorter than HP sample"));
        }
        let mut buf = raw.to_vec();
        if let Some(ref rk) = self.rustls_keys {
            let sample_start = pn_offset.saturating_add(4);
            let sample_end = sample_start.saturating_add(HP_SAMPLE_LEN);
            let mut sample = [0u8; HP_SAMPLE_LEN];
            sample.copy_from_slice(
                buf.get(sample_start..sample_end)
                    .ok_or(NetError::CryptoFailure)?,
            );
            let mut first = *buf.first().ok_or(NetError::CryptoFailure)?;
            let mut pn_bytes = [0u8; 4];
            pn_bytes.copy_from_slice(
                buf.get(pn_offset..pn_offset.saturating_add(4))
                    .ok_or(NetError::CryptoFailure)?,
            );
            rk.remote
                .header
                .decrypt_in_place(&sample, &mut first, &mut pn_bytes)
                .map_err(|_| NetError::CryptoFailure)?;
            if let Some(f) = buf.first_mut() {
                *f = first;
            }
            let pn_len = usize::from(first & 0x03).saturating_add(1);
            if pn_len != 4 {
                return Err(NetError::MalformedWirePacket("Expected 4-byte PN"));
            }
            if let Some(dst) = buf.get_mut(pn_offset..pn_offset.saturating_add(4)) {
                dst.copy_from_slice(&pn_bytes);
            }
            let pn = u64::from(u32::from_be_bytes(pn_bytes));
            let header_end = pn_offset.saturating_add(4);
            let (hdr, ct_and_tag) = buf.split_at_mut(header_end);
            let plain = rk
                .remote
                .packet
                .decrypt_in_place(pn, hdr, ct_and_tag)
                .map_err(|_| NetError::CryptoFailure)?;
            Ok((pn, plain.to_vec()))
        } else {
            let pkt = self.packet_recv.as_ref().ok_or(NetError::CryptoFailure)?;
            let hp = self.hp_recv.as_ref().ok_or(NetError::CryptoFailure)?;
            let pn_len = hp.unprotect_packet(pn_offset, &mut buf)?;
            if pn_len != 4 {
                return Err(NetError::MalformedWirePacket("Expected 4-byte PN"));
            }
            let mut pn_bytes = [0u8; 4];
            pn_bytes.copy_from_slice(
                buf.get(pn_offset..pn_offset.saturating_add(4))
                    .ok_or(NetError::CryptoFailure)?,
            );
            let pn = u64::from(u32::from_be_bytes(pn_bytes));
            let header_end = pn_offset.saturating_add(4);
            if buf.len() < header_end.saturating_add(GCM_TAG_LEN) {
                return Err(NetError::CryptoFailure);
            }
            let tag_start = buf.len().saturating_sub(GCM_TAG_LEN);
            let mut tag = [0u8; GCM_TAG_LEN];
            tag.copy_from_slice(buf.get(tag_start..).ok_or(NetError::CryptoFailure)?);
            buf.truncate(tag_start);
            let (hdr, ct) = buf.split_at_mut(header_end);
            pkt.decrypt_detached(pn, hdr, ct, &tag)?;
            Ok((pn, ct.to_vec()))
        }
    }
}

#[inline]
fn encode_varint(v: u64, out: &mut Vec<u8>) {
    let vi = VarInt::from_u64(v).unwrap_or(VarInt::MAX);
    let raw = vi.into_inner();
    if raw < 64 {
        out.push(raw as u8);
    } else if raw < 16_384 {
        let b = (raw as u16) | 0x4000;
        out.extend_from_slice(&b.to_be_bytes());
    } else if raw < 1_073_741_824 {
        let b = (raw as u32) | 0x8000_0000;
        out.extend_from_slice(&b.to_be_bytes());
    } else {
        let b = raw | 0xc000_0000_0000_0000;
        out.extend_from_slice(&b.to_be_bytes());
    }
}

#[inline]
fn decode_varint(buf: &[u8], cursor: &mut usize) -> Result<u64, NetError> {
    let b0 = *buf
        .get(*cursor)
        .ok_or(NetError::MalformedWirePacket("Truncated VarInt"))?;
    let prefix = b0 >> 6;
    let len: usize = 1usize << prefix;
    let end = cursor
        .checked_add(len)
        .ok_or(NetError::MalformedWirePacket("VarInt overflow"))?;
    let slice = buf
        .get(*cursor..end)
        .ok_or(NetError::MalformedWirePacket("Truncated VarInt body"))?;
    *cursor = end;
    let val = match len {
        1 => u64::from(b0 & 0x3f),
        2 => u64::from(u16::from_be_bytes([slice[0] & 0x3f, slice[1]])),
        4 => u64::from(u32::from_be_bytes([
            slice[0] & 0x3f,
            slice[1],
            slice[2],
            slice[3],
        ])),
        _ => u64::from_be_bytes([
            slice[0] & 0x3f,
            slice[1],
            slice[2],
            slice[3],
            slice[4],
            slice[5],
            slice[6],
            slice[7],
        ]),
    };
    Ok(val)
}

fn encode_rfc9000_transport_params() -> Vec<u8> {
    // RFC 9000 §18.2 + RFC 9221 §3 (`max_datagram_frame_size = 0x20` -> `1100`):
    let mut out = Vec::with_capacity(32);
    // `max_udp_payload_size` (0x03) = 1200
    encode_varint(0x03, &mut out);
    encode_varint(2, &mut out);
    encode_varint(MAX_QUIC_UDP_PACKET_BYTES as u64, &mut out);
    // `initial_max_data` (0x04) = 16 MiB
    encode_varint(0x04, &mut out);
    encode_varint(4, &mut out);
    encode_varint(16_777_216, &mut out);
    // `initial_max_streams_uni` (0x09) = 4096
    encode_varint(0x09, &mut out);
    encode_varint(2, &mut out);
    encode_varint(4_096, &mut out);
    // `max_datagram_frame_size` (0x20) = 1100 (`<= 1,100B`, `게임.md §9.1`)
    encode_varint(0x20, &mut out);
    encode_varint(2, &mut out);
    encode_varint(MAX_DATAGRAM_PAYLOAD_BYTES as u64, &mut out);
    out
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum PacketSpace {
    Initial = 0,
    Handshake = 1,
    Data1Rtt = 2,
}

/// PTO threshold in `15ms` ticks (`7 * 15ms = 105ms > 80ms` link RTT, preventing spurious resends).
const PTO_THRESHOLD_TICKS_15MS: u8 = 7;

#[derive(Clone)]
struct RetransmitEntry {
    space: PacketSpace,
    pn: u64,
    plain_frame: Vec<u8>,
    ticks_elapsed: u8,
}

#[derive(Default)]
struct UniStreamAssembler {
    total_len: Option<usize>,
    chunks: BTreeMap<usize, Vec<u8>>,
    completed: bool,
}

impl UniStreamAssembler {
    fn insert_segment(&mut self, offset: usize, data: &[u8], fin: bool) -> Option<Vec<u8>> {
        if self.completed {
            return None;
        }
        if fin {
            self.total_len = Some(offset.saturating_add(data.len()));
        }
        self.chunks.insert(offset, data.to_vec());
        let target_total = self.total_len?;
        let mut expected_cursor: usize = 0;
        for (&off, seg) in &self.chunks {
            if off > expected_cursor {
                return None;
            }
            let seg_end = off.saturating_add(seg.len());
            if seg_end > expected_cursor {
                expected_cursor = seg_end;
            }
        }
        if expected_cursor < target_total {
            return None;
        }
        let mut full = Vec::with_capacity(target_total);
        let mut cur: usize = 0;
        for (&off, seg) in &self.chunks {
            let seg_end = off.saturating_add(seg.len());
            if seg_end > cur {
                let skip = cur.saturating_sub(off);
                if let Some(tail) = seg.get(skip..) {
                    full.extend_from_slice(tail);
                }
                cur = seg_end;
            }
        }
        full.truncate(target_total);
        self.completed = true;
        self.chunks.clear();
        Some(full)
    }
}

enum TlsEngine {
    Client(rustls::quic::ClientConnection),
    Server(rustls::quic::ServerConnection),
}

impl TlsEngine {
    fn read_hs(&mut self, plain: &[u8]) -> Result<(), NetError> {
        match self {
            Self::Client(c) => c
                .read_hs(plain)
                .map_err(|_| NetError::TlsError("client read_hs failed")),
            Self::Server(s) => s
                .read_hs(plain)
                .map_err(|_| NetError::TlsError("server read_hs failed")),
        }
    }

    fn write_hs(&mut self, out: &mut Vec<u8>) -> Option<rustls::quic::KeyChange> {
        match self {
            Self::Client(c) => c.write_hs(out),
            Self::Server(s) => s.write_hs(out),
        }
    }

    fn is_handshaking(&self) -> bool {
        match self {
            Self::Client(c) => c.is_handshaking(),
            Self::Server(s) => s.is_handshaking(),
        }
    }
}

struct QuicConnState {
    side: Side,
    local_cid: [u8; CID_LEN],
    remote_cid: [u8; CID_LEN],
    peer_addr: SocketAddr,
    tls: TlsEngine,
    write_space: PacketSpace,
    initial_crypto: DirectionalCrypto,
    handshake_crypto: Option<DirectionalCrypto>,
    data_crypto: Option<DirectionalCrypto>,
    crypto_send_off: [u64; 2],
    crypto_recv_off: [u64; 2],
    crypto_recv_buf: [BTreeMap<u64, Vec<u8>>; 2],
    next_pn: [u64; 3],
    recv_pns: [BTreeSet<u64>; 3],
    next_uni_stream_seq: u64,
    in_flight_reliable: Vec<RetransmitEntry>,
    uni_assemblers: BTreeMap<u64, UniStreamAssembler>,
    retired_uni_streams: BTreeSet<u64>,
    handshake_done_sent: bool,
    inbound_datagrams: mpsc::UnboundedSender<Vec<u8>>,
    inbound_uni_streams: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    established: Arc<AtomicBool>,
}

impl QuicConnState {
    fn new_client(
        local_cid: [u8; CID_LEN],
        remote_cid: [u8; CID_LEN],
        peer_addr: SocketAddr,
        client_cfg: Arc<rustls::ClientConfig>,
        inbound_datagrams: mpsc::UnboundedSender<Vec<u8>>,
        inbound_uni_streams: mpsc::UnboundedSender<(u64, Vec<u8>)>,
        established: Arc<AtomicBool>,
    ) -> Result<Self, NetError> {
        let server_name = rustls::pki_types::ServerName::try_from("localhost")
            .map_err(|_| NetError::TlsError("Invalid ServerName"))?;
        let tls_conn = rustls::quic::ClientConnection::new(
            client_cfg,
            rustls::quic::Version::V1,
            server_name,
            encode_rfc9000_transport_params(),
        )
        .map_err(|_| NetError::TlsError("ClientConnection::new failed"))?;
        let init_dcid = ConnectionId::new(&remote_cid);
        let initial_crypto = DirectionalCrypto::from_initial_cid(&init_dcid, Side::Client)?;

        Ok(Self {
            side: Side::Client,
            local_cid,
            remote_cid,
            peer_addr,
            tls: TlsEngine::Client(tls_conn),
            write_space: PacketSpace::Initial,
            initial_crypto,
            handshake_crypto: None,
            data_crypto: None,
            crypto_send_off: [0, 0],
            crypto_recv_off: [0, 0],
            crypto_recv_buf: [BTreeMap::new(), BTreeMap::new()],
            next_pn: [1, 1, 1],
            recv_pns: [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()],
            next_uni_stream_seq: 0,
            in_flight_reliable: Vec::new(),
            uni_assemblers: BTreeMap::new(),
            retired_uni_streams: BTreeSet::new(),
            handshake_done_sent: false,
            inbound_datagrams,
            inbound_uni_streams,
            established,
        })
    }

    fn new_server(
        init_dcid: [u8; CID_LEN],
        client_scid: [u8; CID_LEN],
        peer_addr: SocketAddr,
        server_cfg: Arc<rustls::ServerConfig>,
        inbound_datagrams: mpsc::UnboundedSender<Vec<u8>>,
        inbound_uni_streams: mpsc::UnboundedSender<(u64, Vec<u8>)>,
        established: Arc<AtomicBool>,
    ) -> Result<Self, NetError> {
        let tls_conn = rustls::quic::ServerConnection::new(
            server_cfg,
            rustls::quic::Version::V1,
            encode_rfc9000_transport_params(),
        )
        .map_err(|_| NetError::TlsError("ServerConnection::new failed"))?;
        let dcid_obj = ConnectionId::new(&init_dcid);
        let initial_crypto = DirectionalCrypto::from_initial_cid(&dcid_obj, Side::Server)?;

        Ok(Self {
            side: Side::Server,
            local_cid: init_dcid,
            remote_cid: client_scid,
            peer_addr,
            tls: TlsEngine::Server(tls_conn),
            write_space: PacketSpace::Initial,
            initial_crypto,
            handshake_crypto: None,
            data_crypto: None,
            crypto_send_off: [0, 0],
            crypto_recv_off: [0, 0],
            crypto_recv_buf: [BTreeMap::new(), BTreeMap::new()],
            next_pn: [1, 1, 1],
            recv_pns: [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()],
            next_uni_stream_seq: 0,
            in_flight_reliable: Vec::new(),
            uni_assemblers: BTreeMap::new(),
            retired_uni_streams: BTreeSet::new(),
            handshake_done_sent: false,
            inbound_datagrams,
            inbound_uni_streams,
            established,
        })
    }

    fn alloc_pn(&mut self, space: PacketSpace) -> u64 {
        let idx = space as usize;
        let pn = self.next_pn.get(idx).copied().unwrap_or(1);
        if let Some(slot) = self.next_pn.get_mut(idx) {
            *slot = pn.saturating_add(1);
        }
        pn
    }

    fn build_header(&self, space: PacketSpace, pn: u64, payload_plus_tag_len: usize) -> Vec<u8> {
        let pn_u32 = (pn & 0xffff_ffff) as u32;
        match space {
            PacketSpace::Initial => {
                // RFC 9000 §17.2.2 Long Header Initial (`0xC3` = Long | Fixed | Initial(00) | 4B PN)
                let mut h = Vec::with_capacity(32);
                h.push(0xc3);
                h.extend_from_slice(&1_u32.to_be_bytes()); // QUIC Version 1 (`0x00000001`)
                h.push(CID_LEN as u8);
                h.extend_from_slice(&self.remote_cid);
                h.push(CID_LEN as u8);
                h.extend_from_slice(&self.local_cid);
                encode_varint(0, &mut h); // Token Length = 0
                encode_varint(payload_plus_tag_len.saturating_add(4) as u64, &mut h);
                h.extend_from_slice(&pn_u32.to_be_bytes());
                h
            }
            PacketSpace::Handshake => {
                // RFC 9000 §17.2.4 Long Header Handshake (`0xE3` = Long | Fixed | Handshake(10) | 4B PN)
                let mut h = Vec::with_capacity(32);
                h.push(0xe3);
                h.extend_from_slice(&1_u32.to_be_bytes());
                h.push(CID_LEN as u8);
                h.extend_from_slice(&self.remote_cid);
                h.push(CID_LEN as u8);
                h.extend_from_slice(&self.local_cid);
                encode_varint(payload_plus_tag_len.saturating_add(4) as u64, &mut h);
                h.extend_from_slice(&pn_u32.to_be_bytes());
                h
            }
            PacketSpace::Data1Rtt => {
                // RFC 9000 §17.3.1 Short Header 1-RTT (`0x43` = Fixed | 4B PN)
                let mut h = Vec::with_capacity(16);
                h.push(0x43);
                h.extend_from_slice(&self.remote_cid);
                h.extend_from_slice(&pn_u32.to_be_bytes());
                h
            }
        }
    }

    fn seal_packet_with_pn(
        &self,
        space: PacketSpace,
        pn: u64,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, NetError> {
        let padded_len = payload.len().max(4);
        let hdr = self.build_header(space, pn, padded_len.saturating_add(GCM_TAG_LEN));
        match space {
            PacketSpace::Initial => self.initial_crypto.seal_packet(pn, hdr, payload),
            PacketSpace::Handshake => self
                .handshake_crypto
                .as_ref()
                .ok_or(NetError::CryptoFailure)?
                .seal_packet(pn, hdr, payload),
            PacketSpace::Data1Rtt => self
                .data_crypto
                .as_ref()
                .ok_or(NetError::CryptoFailure)?
                .seal_packet(pn, hdr, payload),
        }
    }

    fn seal_in_space(
        &mut self,
        space: PacketSpace,
        payload: Vec<u8>,
        reliable: bool,
    ) -> Result<Vec<u8>, NetError> {
        let pn = self.alloc_pn(space);
        let retained_plain = if reliable { Some(payload.clone()) } else { None };
        let wire = self.seal_packet_with_pn(space, pn, payload)?;
        if let Some(plain_frame) = retained_plain {
            self.in_flight_reliable.push(RetransmitEntry {
                space,
                pn,
                plain_frame,
                ticks_elapsed: 0,
            });
        }
        Ok(wire)
    }

    fn pump_handshake_outbound(&mut self) -> Result<Vec<Vec<u8>>, NetError> {
        let mut packets = Vec::new();
        loop {
            let cur_space = self.write_space;
            let mut hs_bytes = Vec::new();
            let key_change = self.tls.write_hs(&mut hs_bytes);

            if !hs_bytes.is_empty() && cur_space != PacketSpace::Data1Rtt {
                let space_idx = cur_space as usize;
                // Split large handshake flights (e.g. X.509 cert) into `<= 850B` CRYPTO chunks:
                let mut slice_cursor: usize = 0;
                while slice_cursor < hs_bytes.len() {
                    let chunk_end = slice_cursor.saturating_add(850).min(hs_bytes.len());
                    let seg = hs_bytes
                        .get(slice_cursor..chunk_end)
                        .ok_or(NetError::CryptoFailure)?;
                    let off = self.crypto_send_off.get(space_idx).copied().unwrap_or(0);
                    if let Some(slot) = self.crypto_send_off.get_mut(space_idx) {
                        *slot = off.saturating_add(seg.len() as u64);
                    }
                    let mut frame = Vec::with_capacity(seg.len().saturating_add(16));
                    frame.push(0x06); // RFC 9000 §19.6 CRYPTO frame
                    encode_varint(off, &mut frame);
                    encode_varint(seg.len() as u64, &mut frame);
                    frame.extend_from_slice(seg);
                    packets.push(self.seal_in_space(cur_space, frame, true)?);
                    slice_cursor = chunk_end;
                }
            }

            match key_change {
                Some(rustls::quic::KeyChange::Handshake { keys }) => {
                    self.handshake_crypto = Some(DirectionalCrypto::from_rustls(keys));
                    self.write_space = PacketSpace::Handshake;
                }
                Some(rustls::quic::KeyChange::OneRtt { keys, .. }) => {
                    self.data_crypto = Some(DirectionalCrypto::from_rustls(keys));
                    self.write_space = PacketSpace::Data1Rtt;
                }
                None => break,
            }
        }
        // RFC 9000 §19.20: Server confirms TLS 1.3 ClientFinished and sends reliable `HANDSHAKE_DONE (0x1e)`;
        // Client waits for `HANDSHAKE_DONE` before marking `established = true`!
        if self.side == Side::Server && self.data_crypto.is_some() && !self.tls.is_handshaking() {
            self.established.store(true, Ordering::Release);
            if !self.handshake_done_sent {
                self.handshake_done_sent = true;
                packets.push(self.seal_in_space(PacketSpace::Data1Rtt, vec![0x1e], true)?);
            }
        }
        Ok(packets)
    }

    fn build_ack_packet(&mut self, space: PacketSpace, fallback_pn: u64) -> Result<Vec<u8>, NetError> {
        let sidx = space as usize;
        let mut ranges: Vec<(u64, u64)> = Vec::new();
        if let Some(pns) = self.recv_pns.get(sidx) {
            for &pn in pns.iter().rev() {
                if let Some(last) = ranges.last_mut()
                    && pn.saturating_add(1) == last.0
                {
                    last.0 = pn;
                    continue;
                }
                ranges.push((pn, pn));
                if ranges.len() >= 9 {
                    break;
                }
            }
        }
        if ranges.is_empty() {
            ranges.push((fallback_pn, fallback_pn));
        }

        let (first_lo, largest_acked) = ranges[0];
        let first_ack_range = largest_acked.saturating_sub(first_lo);
        let extra_cnt = ranges.len().saturating_sub(1);

        let mut frame = Vec::with_capacity(32);
        frame.push(0x02); // RFC 9000 §19.3 ACK frame
        encode_varint(largest_acked, &mut frame);
        encode_varint(0, &mut frame); // ack_delay
        encode_varint(extra_cnt as u64, &mut frame);
        encode_varint(first_ack_range, &mut frame);

        let mut prev_smallest = first_lo;
        for &(lo, hi) in ranges.iter().skip(1) {
            // RFC 9000 §19.3.1: `gap = prev_smallest - hi - 2`
            let gap = prev_smallest.saturating_sub(hi).saturating_sub(2);
            let rng_len = hi.saturating_sub(lo);
            encode_varint(gap, &mut frame);
            encode_varint(rng_len, &mut frame);
            prev_smallest = lo;
        }
        self.seal_in_space(space, frame, false)
    }

    fn encode_datagram_packet(&mut self, app_bytes: &[u8]) -> Result<Vec<u8>, NetError> {
        if app_bytes.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(app_bytes.len()));
        }
        if !self.established.load(Ordering::Acquire) {
            return Err(NetError::ConnectionClosed);
        }
        // RFC 9221 §4 DATAGRAM frame with length (`0x31`):
        let mut frame = Vec::with_capacity(app_bytes.len().saturating_add(8));
        frame.push(0x31);
        encode_varint(app_bytes.len() as u64, &mut frame);
        frame.extend_from_slice(app_bytes);
        self.seal_in_space(PacketSpace::Data1Rtt, frame, false)
    }

    fn encode_uni_stream_packets(&mut self, data: &[u8]) -> Result<(u64, Vec<Vec<u8>>), NetError> {
        if !self.established.load(Ordering::Acquire) {
            return Err(NetError::ConnectionClosed);
        }
        let seq = self.next_uni_stream_seq;
        self.next_uni_stream_seq = seq.saturating_add(1);
        // RFC 9000 §2.1 Stream IDs: `4*seq + 2` (Client-Initiated Uni), `4*seq + 3` (Server-Initiated Uni)
        let role_bits: u64 = match self.side {
            Side::Client => 2,
            Side::Server => 3,
        };
        let stream_id = seq.saturating_mul(4).saturating_add(role_bits);

        let mut packets = Vec::new();
        let total = data.len();
        if total == 0 {
            let mut frame = Vec::with_capacity(16);
            frame.push(0x0f); // STREAM (OFF | LEN | FIN)
            encode_varint(stream_id, &mut frame);
            encode_varint(0, &mut frame);
            encode_varint(0, &mut frame);
            packets.push(self.seal_in_space(PacketSpace::Data1Rtt, frame, true)?);
            return Ok((stream_id, packets));
        }

        let mut offset: usize = 0;
        while offset < total {
            let end = offset.saturating_add(900).min(total);
            let fin = end == total;
            let seg = data.get(offset..end).ok_or(NetError::CryptoFailure)?;
            let mut frame = Vec::with_capacity(seg.len().saturating_add(24));
            // RFC 9000 §19.8 STREAM frame: `0x0e` (OFF|LEN) or `0x0f` (OFF|LEN|FIN)
            frame.push(if fin { 0x0f } else { 0x0e });
            encode_varint(stream_id, &mut frame);
            encode_varint(offset as u64, &mut frame);
            encode_varint(seg.len() as u64, &mut frame);
            frame.extend_from_slice(seg);
            packets.push(self.seal_in_space(PacketSpace::Data1Rtt, frame, true)?);
            offset = end;
        }
        Ok((stream_id, packets))
    }

    fn handle_inbound_packet(&mut self, raw: &[u8]) -> Result<Vec<Vec<u8>>, NetError> {
        let first = *raw
            .first()
            .ok_or(NetError::MalformedWirePacket("Empty UDP datagram"))?;
        let (space, pn_offset) = if (first & 0x80) != 0 {
            let pkt_type = (first & 0x30) >> 4;
            let mut cur: usize = 23;
            if pkt_type == 0x00 {
                let tok_len = decode_varint(raw, &mut cur)? as usize;
                cur = cur.saturating_add(tok_len);
                let _rem_len = decode_varint(raw, &mut cur)?;
                (PacketSpace::Initial, cur)
            } else if pkt_type == 0x02 {
                let _rem_len = decode_varint(raw, &mut cur)?;
                (PacketSpace::Handshake, cur)
            } else {
                return Err(NetError::MalformedWirePacket("Unsupported long header type"));
            }
        } else {
            (PacketSpace::Data1Rtt, 1usize.saturating_add(CID_LEN))
        };

        let (pn, plain) = match space {
            PacketSpace::Initial => self.initial_crypto.open_packet(pn_offset, raw)?,
            PacketSpace::Handshake => self
                .handshake_crypto
                .as_ref()
                .ok_or(NetError::CryptoFailure)?
                .open_packet(pn_offset, raw)?,
            PacketSpace::Data1Rtt => self
                .data_crypto
                .as_ref()
                .ok_or(NetError::CryptoFailure)?
                .open_packet(pn_offset, raw)?,
        };

        if let Some(pset) = self.recv_pns.get_mut(space as usize) {
            pset.insert(pn);
            if let Some(&max_pn) = pset.iter().next_back()
                && max_pn > 128
            {
                let cutoff = max_pn.saturating_sub(128);
                *pset = pset.split_off(&cutoff);
            }
        }

        let mut outbound = Vec::new();
        let mut need_ack = false;
        let mut cur: usize = 0;
        while cur < plain.len() {
            let ftype = *plain
                .get(cur)
                .ok_or(NetError::MalformedWirePacket("Frame cursor OOB"))?;
            cur = cur.saturating_add(1);
            match ftype {
                0x00 | 0x01 => {
                    if ftype == 0x01 {
                        need_ack = true;
                    }
                }
                0x02 => {
                    // RFC 9000 §19.3 ACK frame with full `first_ack_range` + `(gap, ack_range)` retirement:
                    let largest_acked = decode_varint(&plain, &mut cur)?;
                    let _delay = decode_varint(&plain, &mut cur)?;
                    let range_cnt = decode_varint(&plain, &mut cur)?;
                    let first_rng = decode_varint(&plain, &mut cur)?;
                    let first_lo = largest_acked.saturating_sub(first_rng);
                    let mut acked_ranges: Vec<(u64, u64)> = vec![(first_lo, largest_acked)];
                    let mut prev_smallest = first_lo;
                    let mut r: u64 = 0;
                    while r < range_cnt {
                        let gap = decode_varint(&plain, &mut cur)?;
                        let ack_rng = decode_varint(&plain, &mut cur)?;
                        let hi = prev_smallest.saturating_sub(gap).saturating_sub(2);
                        let lo = hi.saturating_sub(ack_rng);
                        acked_ranges.push((lo, hi));
                        prev_smallest = lo;
                        r = r.saturating_add(1);
                    }
                    self.in_flight_reliable.retain(|e| {
                        !(e.space == space
                            && acked_ranges
                                .iter()
                                .any(|&(lo, hi)| e.pn >= lo && e.pn <= hi))
                    });
                }
                0x06 => {
                    // CRYPTO frame (with duplicate / overlapping retransmission filtering)
                    need_ack = true;
                    let off = decode_varint(&plain, &mut cur)?;
                    let len = decode_varint(&plain, &mut cur)? as usize;
                    let end = cur
                        .checked_add(len)
                        .ok_or(NetError::MalformedWirePacket("CRYPTO len overflow"))?;
                    let data = plain
                        .get(cur..end)
                        .ok_or(NetError::MalformedWirePacket("Truncated CRYPTO frame"))?;
                    cur = end;

                    let sidx = (space as usize).min(1);
                    let expected_now = self.crypto_recv_off.get(sidx).copied().unwrap_or(0);
                    let seg_end = off.saturating_add(data.len() as u64);
                    if seg_end > expected_now {
                        let (ins_off, ins_slice) = if off < expected_now {
                            let trim = (expected_now.saturating_sub(off)) as usize;
                            (expected_now, data.get(trim..).unwrap_or(&[]))
                        } else {
                            (off, data)
                        };
                        if !ins_slice.is_empty()
                            && let Some(buf_map) = self.crypto_recv_buf.get_mut(sidx)
                        {
                            buf_map.insert(ins_off, ins_slice.to_vec());
                        }
                    }
                    loop {
                        let expected = self.crypto_recv_off.get(sidx).copied().unwrap_or(0);
                        let next_seg = self
                            .crypto_recv_buf
                            .get_mut(sidx)
                            .and_then(|m| m.remove(&expected));
                        if let Some(seg) = next_seg {
                            if let Some(slot) = self.crypto_recv_off.get_mut(sidx) {
                                *slot = expected.saturating_add(seg.len() as u64);
                            }
                            self.tls.read_hs(&seg)?;
                        } else {
                            break;
                        }
                    }
                    let mut hs_out = self.pump_handshake_outbound()?;
                    outbound.append(&mut hs_out);
                }
                0x0e | 0x0f => {
                    // Unidirectional STREAM frame (`OFF | LEN | [FIN]`) with retired-stream pruning
                    need_ack = true;
                    let fin = (ftype & 0x01) != 0;
                    let stream_id = decode_varint(&plain, &mut cur)?;
                    let off = decode_varint(&plain, &mut cur)? as usize;
                    let len = decode_varint(&plain, &mut cur)? as usize;
                    let end = cur
                        .checked_add(len)
                        .ok_or(NetError::MalformedWirePacket("STREAM len overflow"))?;
                    let data = plain
                        .get(cur..end)
                        .ok_or(NetError::MalformedWirePacket("Truncated STREAM frame"))?;
                    cur = end;

                    if !self.retired_uni_streams.contains(&stream_id) {
                        let completed_opt = {
                            let asm = self.uni_assemblers.entry(stream_id).or_default();
                            asm.insert_segment(off, data, fin)
                        };
                        if let Some(completed_msg) = completed_opt {
                            let _ = self.uni_assemblers.remove(&stream_id);
                            self.retired_uni_streams.insert(stream_id);
                            if self.retired_uni_streams.len() > 4_096
                                && let Some(&oldest) = self.retired_uni_streams.iter().next()
                            {
                                let _ = self.retired_uni_streams.remove(&oldest);
                            }
                            let _ = self.inbound_uni_streams.send((stream_id, completed_msg));
                        }
                    }
                }
                0x1e => {
                    // RFC 9000 §19.20 HANDSHAKE_DONE frame
                    need_ack = true;
                    if self.side == Side::Client
                        && self.data_crypto.is_some()
                        && !self.tls.is_handshaking()
                    {
                        self.established.store(true, Ordering::Release);
                    }
                }
                0x31 => {
                    // RFC 9221 Unreliable DATAGRAM frame
                    let len = decode_varint(&plain, &mut cur)? as usize;
                    if len > MAX_DATAGRAM_PAYLOAD_BYTES {
                        return Err(NetError::DatagramExceeds1100Bytes(len));
                    }
                    let end = cur
                        .checked_add(len)
                        .ok_or(NetError::MalformedWirePacket("DATAGRAM len overflow"))?;
                    let data = plain
                        .get(cur..end)
                        .ok_or(NetError::MalformedWirePacket("Truncated DATAGRAM frame"))?;
                    cur = end;
                    let _ = self.inbound_datagrams.send(data.to_vec());
                }
                _ => {
                    break;
                }
            }
        }

        if need_ack {
            outbound.push(self.build_ack_packet(space, pn)?);
        }
        Ok(outbound)
    }

    /// RFC 9000 §12.3 / §13.3 PTO retransmission: allocates a fresh packet number and re-seals `plain_frame`.
    fn collect_pto_retransmits(&mut self) -> Vec<Vec<u8>> {
        let mut to_retransmit: Vec<usize> = Vec::new();
        for (idx, entry) in self.in_flight_reliable.iter_mut().enumerate() {
            entry.ticks_elapsed = entry.ticks_elapsed.saturating_add(1);
            if entry.ticks_elapsed >= PTO_THRESHOLD_TICKS_15MS {
                entry.ticks_elapsed = 0;
                to_retransmit.push(idx);
            }
        }
        let mut resends = Vec::with_capacity(to_retransmit.len());
        for idx in to_retransmit {
            let Some((space, plain)) = self
                .in_flight_reliable
                .get(idx)
                .map(|e| (e.space, e.plain_frame.clone()))
            else {
                continue;
            };
            let fresh_pn = self.alloc_pn(space);
            if let Ok(wire) = self.seal_packet_with_pn(space, fresh_pn, plain) {
                if let Some(entry) = self.in_flight_reliable.get_mut(idx) {
                    entry.pn = fresh_pn;
                }
                resends.push(wire);
            }
        }
        resends
    }
}

/// Configurable live UDP link impairment (`5%` packet loss + `80ms` RTT = `40ms` one-way, `게임.md §11.3`).
#[derive(Clone, Copy, Debug)]
pub struct UdpLinkImpairment {
    pub enabled: bool,
    pub loss_permille: u32,
    pub one_way_delay_ms: u64,
}

impl UdpLinkImpairment {
    pub const fn lossless_lan() -> Self {
        Self {
            enabled: false,
            loss_permille: 0,
            one_way_delay_ms: 0,
        }
    }

    /// `게임.md §11.3`: `5%` packet loss (`50` permille) + `80ms` RTT (`40ms` one-way delay).
    pub const fn phase2_raid_impairment() -> Self {
        Self {
            enabled: true,
            loss_permille: 50,
            one_way_delay_ms: 40,
        }
    }
}

#[derive(Default)]
pub struct UdpTransportCounters {
    pub datagrams_tx: AtomicU64,
    pub datagrams_dropped_impairment: AtomicU64,
    pub datagrams_delivered_udp: AtomicU64,
}

async fn send_udp_with_impairment(
    sock: &Arc<UdpSocket>,
    dest: SocketAddr,
    wire: Vec<u8>,
    impairment: UdpLinkImpairment,
    rng_mutex: &Mutex<DetRng>,
    counters: &Arc<UdpTransportCounters>,
) -> Result<(), NetError> {
    counters.datagrams_tx.fetch_add(1, Ordering::Relaxed);
    if impairment.enabled {
        let drop_it = {
            let mut g = rng_mutex.lock().map_err(|_| NetError::IoFailure)?;
            (g.next_u64() % 1_000) < u64::from(impairment.loss_permille)
        };
        if drop_it {
            counters
                .datagrams_dropped_impairment
                .fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        if impairment.one_way_delay_ms > 0 {
            let sock_cl = Arc::clone(sock);
            let cnt_cl = Arc::clone(counters);
            let delay = Duration::from_millis(impairment.one_way_delay_ms);
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                if sock_cl.send_to(&wire, dest).await.is_ok() {
                    cnt_cl
                        .datagrams_delivered_udp
                        .fetch_add(1, Ordering::Relaxed);
                }
            });
            return Ok(());
        }
    }
    sock.send_to(&wire, dest)
        .await
        .map_err(|_| NetError::IoFailure)?;
    counters
        .datagrams_delivered_udp
        .fetch_add(1, Ordering::Relaxed);
    Ok(())
}

/// Live client QUIC v1 connection over OS `tokio::net::UdpSocket` (with `impairment_cfg` active from packet `0`).
pub struct QuicClientEndpoint {
    sock: Arc<UdpSocket>,
    state: Arc<AsyncMutex<QuicConnState>>,
    datagram_rx: AsyncMutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    uni_rx: AsyncMutex<mpsc::UnboundedReceiver<(u64, Vec<u8>)>>,
    impairment_cfg: UdpLinkImpairment,
    rng: Arc<Mutex<DetRng>>,
    pub counters: Arc<UdpTransportCounters>,
    shutdown: Arc<AtomicBool>,
}

impl QuicClientEndpoint {
    pub async fn connect(
        server_addr: SocketAddr,
        client_cfg: Arc<rustls::ClientConfig>,
        seed: u64,
        impairment_cfg: UdpLinkImpairment,
    ) -> Result<Self, NetError> {
        let sock = Arc::new(
            UdpSocket::bind("127.0.0.1:0")
                .await
                .map_err(|_| NetError::IoFailure)?,
        );
        let mut rng_init = DetRng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
        let local_cid = rng_init.next_u64().to_le_bytes();
        let remote_cid = rng_init.next_u64().to_le_bytes();

        let (dg_tx, dg_rx) = mpsc::unbounded_channel();
        let (uni_tx, uni_rx) = mpsc::unbounded_channel();
        let established = Arc::new(AtomicBool::new(false));

        let mut conn = QuicConnState::new_client(
            local_cid,
            remote_cid,
            server_addr,
            client_cfg,
            dg_tx,
            uni_tx,
            Arc::clone(&established),
        )?;
        let initial_pkts = conn.pump_handshake_outbound()?;
        let state = Arc::new(AsyncMutex::new(conn));
        let rng = Arc::new(Mutex::new(rng_init));
        let counters = Arc::new(UdpTransportCounters::default());
        let shutdown = Arc::new(AtomicBool::new(false));

        // Background UDP receive + PTO retransmit task (full `impairment_cfg` active across Initial, Handshake, PTO, & 1-RTT):
        {
            let sock_bg = Arc::clone(&sock);
            let state_bg = Arc::clone(&state);
            let rng_bg = Arc::clone(&rng);
            let cnt_bg = Arc::clone(&counters);
            let shut_bg = Arc::clone(&shutdown);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let mut pto_tick = tokio::time::interval(Duration::from_millis(15));
                while !shut_bg.load(Ordering::Relaxed) {
                    tokio::select! {
                        recv_res = sock_bg.recv_from(&mut buf) => {
                            if let Ok((n, _src)) = recv_res
                                && let Some(raw) = buf.get(..n)
                            {
                                let out_pkts = {
                                    let mut st = state_bg.lock().await;
                                    st.handle_inbound_packet(raw).unwrap_or_default()
                                };
                                for p in out_pkts {
                                    let _ = send_udp_with_impairment(&sock_bg, server_addr, p, impairment_cfg, &rng_bg, &cnt_bg).await;
                                }
                            }
                        }
                        _ = pto_tick.tick() => {
                            let resends = {
                                let mut st = state_bg.lock().await;
                                st.collect_pto_retransmits()
                            };
                            for p in resends {
                                let _ = send_udp_with_impairment(&sock_bg, server_addr, p, impairment_cfg, &rng_bg, &cnt_bg).await;
                            }
                        }
                    }
                }
            });
        }

        // Transmit Initial flight through the live impaired UDP link (`5%` loss / `80ms` RTT):
        for pkt in initial_pkts {
            send_udp_with_impairment(&sock, server_addr, pkt, impairment_cfg, &rng, &counters)
                .await?;
        }

        // Await TLS 1.3 handshake + server `HANDSHAKE_DONE (0x1e)` confirmation:
        let mut wait_iters: u32 = 0;
        while !established.load(Ordering::Acquire) {
            if wait_iters > 1_200 {
                shutdown.store(true, Ordering::Release);
                return Err(NetError::TlsError("QUIC handshake timed out"));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
            wait_iters = wait_iters.saturating_add(1);
        }

        Ok(Self {
            sock,
            state,
            datagram_rx: AsyncMutex::new(dg_rx),
            uni_rx: AsyncMutex::new(uni_rx),
            impairment_cfg,
            rng,
            counters,
            shutdown,
        })
    }

    pub async fn send_datagram(&self, payload: &[u8]) -> Result<(), NetError> {
        if payload.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(payload.len()));
        }
        let (peer, wire) = {
            let mut st = self.state.lock().await;
            let w = st.encode_datagram_packet(payload)?;
            (st.peer_addr, w)
        };
        send_udp_with_impairment(
            &self.sock,
            peer,
            wire,
            self.impairment_cfg,
            &self.rng,
            &self.counters,
        )
        .await
    }

    pub async fn send_uni_stream(&self, stream_payload: &[u8]) -> Result<u64, NetError> {
        let (peer, sid, pkts) = {
            let mut st = self.state.lock().await;
            let (sid, pkts) = st.encode_uni_stream_packets(stream_payload)?;
            (st.peer_addr, sid, pkts)
        };
        for w in pkts {
            send_udp_with_impairment(
                &self.sock,
                peer,
                w,
                self.impairment_cfg,
                &self.rng,
                &self.counters,
            )
            .await?;
        }
        Ok(sid)
    }

    pub async fn try_recv_datagrams(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut rx = self.datagram_rx.lock().await;
        while let Ok(msg) = rx.try_recv() {
            out.push(msg);
        }
        out
    }

    pub async fn try_recv_uni_streams(&self) -> Vec<(u64, Vec<u8>)> {
        let mut out = Vec::new();
        let mut rx = self.uni_rx.lock().await;
        while let Ok(msg) = rx.try_recv() {
            out.push(msg);
        }
        out
    }

    pub fn close(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

impl Drop for QuicClientEndpoint {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

struct ServerPeerSlot {
    conn: QuicConnState,
    dg_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    uni_rx: mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
}

/// Multiplexed authoritative QUIC v1 UDP server endpoint handling 50+ concurrent connections.
pub struct QuicServerEndpoint {
    sock: Arc<UdpSocket>,
    local_addr: SocketAddr,
    peers: Arc<AsyncMutex<BTreeMap<[u8; CID_LEN], ServerPeerSlot>>>,
    impairment_cfg: UdpLinkImpairment,
    rng: Arc<Mutex<DetRng>>,
    pub counters: Arc<UdpTransportCounters>,
    shutdown: Arc<AtomicBool>,
}

impl QuicServerEndpoint {
    pub async fn bind(
        bind_addr: SocketAddr,
        server_cfg: Arc<rustls::ServerConfig>,
        seed: u64,
        impairment_cfg: UdpLinkImpairment,
    ) -> Result<Self, NetError> {
        let sock = Arc::new(
            UdpSocket::bind(bind_addr)
                .await
                .map_err(|_| NetError::IoFailure)?,
        );
        let local_addr = sock.local_addr().map_err(|_| NetError::IoFailure)?;
        let peers: Arc<AsyncMutex<BTreeMap<[u8; CID_LEN], ServerPeerSlot>>> =
            Arc::new(AsyncMutex::new(BTreeMap::new()));
        let rng = Arc::new(Mutex::new(DetRng::new(seed ^ 0xA5A5_5A5A_F0F0_0F0F)));
        let counters = Arc::new(UdpTransportCounters::default());
        let shutdown = Arc::new(AtomicBool::new(false));

        {
            let sock_bg = Arc::clone(&sock);
            let peers_bg = Arc::clone(&peers);
            let rng_bg = Arc::clone(&rng);
            let cnt_bg = Arc::clone(&counters);
            let shut_bg = Arc::clone(&shutdown);
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let mut pto_tick = tokio::time::interval(Duration::from_millis(15));
                while !shut_bg.load(Ordering::Relaxed) {
                    tokio::select! {
                        recv_res = sock_bg.recv_from(&mut buf) => {
                            if let Ok((n, src_addr)) = recv_res
                                && let Some(raw) = buf.get(..n)
                            {
                                let first = raw.first().copied().unwrap_or(0);
                                let parsed_cids: Option<([u8; CID_LEN], Option<[u8; CID_LEN]>)> = if (first & 0x80) != 0 {
                                    if raw.len() >= 23 && raw.get(5) == Some(&(CID_LEN as u8)) && raw.get(14) == Some(&(CID_LEN as u8)) {
                                        let mut dcid = [0u8; CID_LEN];
                                        let mut scid = [0u8; CID_LEN];
                                        if let (Some(ds), Some(ss)) = (raw.get(6..14), raw.get(15..23)) {
                                            dcid.copy_from_slice(ds);
                                            scid.copy_from_slice(ss);
                                            Some((dcid, Some(scid)))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else if raw.len() >= 9 {
                                    let mut dcid = [0u8; CID_LEN];
                                    if let Some(ds) = raw.get(1..9) {
                                        dcid.copy_from_slice(ds);
                                        Some((dcid, None))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if let Some((dcid, maybe_scid)) = parsed_cids {
                                    let mut out_to_send: Vec<(SocketAddr, Vec<u8>)> = Vec::new();
                                    {
                                        let mut map = peers_bg.lock().await;
                                        if !map.contains_key(&dcid)
                                            && let Some(scid) = maybe_scid
                                            && (first & 0x30) == 0x00
                                        {
                                            let (dg_tx, dg_rx) = mpsc::unbounded_channel();
                                            let (uni_tx, uni_rx) = mpsc::unbounded_channel();
                                            let est = Arc::new(AtomicBool::new(false));
                                            if let Ok(conn) = QuicConnState::new_server(
                                                dcid,
                                                scid,
                                                src_addr,
                                                Arc::clone(&server_cfg),
                                                dg_tx,
                                                uni_tx,
                                                est,
                                            ) {
                                                map.insert(dcid, ServerPeerSlot { conn, dg_rx, uni_rx });
                                            }
                                        }
                                        if let Some(slot) = map.get_mut(&dcid) {
                                            slot.conn.peer_addr = src_addr;
                                            if let Ok(pkts) = slot.conn.handle_inbound_packet(raw) {
                                                for p in pkts {
                                                    out_to_send.push((src_addr, p));
                                                }
                                            }
                                        }
                                    }
                                    for (addr, p) in out_to_send {
                                        let _ = send_udp_with_impairment(&sock_bg, addr, p, impairment_cfg, &rng_bg, &cnt_bg).await;
                                    }
                                }
                            }
                        }
                        _ = pto_tick.tick() => {
                            let mut resends: Vec<(SocketAddr, Vec<u8>)> = Vec::new();
                            {
                                let mut map = peers_bg.lock().await;
                                for slot in map.values_mut() {
                                    for p in slot.conn.collect_pto_retransmits() {
                                        resends.push((slot.conn.peer_addr, p));
                                    }
                                }
                            }
                            for (addr, p) in resends {
                                let _ = send_udp_with_impairment(&sock_bg, addr, p, impairment_cfg, &rng_bg, &cnt_bg).await;
                            }
                        }
                    }
                }
            });
        }

        Ok(Self {
            sock,
            local_addr,
            peers,
            impairment_cfg,
            rng,
            counters,
            shutdown,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn drain_all_inbound(&self) -> (Vec<([u8; CID_LEN], Vec<u8>)>, Vec<([u8; CID_LEN], u64, Vec<u8>)>) {
        let mut dgs = Vec::new();
        let mut streams = Vec::new();
        let mut map = self.peers.lock().await;
        for (&cid, slot) in map.iter_mut() {
            while let Ok(dg) = slot.dg_rx.try_recv() {
                dgs.push((cid, dg));
            }
            while let Ok((sid, body)) = slot.uni_rx.try_recv() {
                streams.push((cid, sid, body));
            }
        }
        (dgs, streams)
    }

    pub async fn send_datagram_to(&self, cid: &[u8; CID_LEN], payload: &[u8]) -> Result<(), NetError> {
        if payload.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(payload.len()));
        }
        let maybe_send = {
            let mut map = self.peers.lock().await;
            if let Some(slot) = map.get_mut(cid) {
                let w = slot.conn.encode_datagram_packet(payload)?;
                Some((slot.conn.peer_addr, w))
            } else {
                None
            }
        };
        if let Some((addr, wire)) = maybe_send {
            send_udp_with_impairment(
                &self.sock,
                addr,
                wire,
                self.impairment_cfg,
                &self.rng,
                &self.counters,
            )
            .await?;
        }
        Ok(())
    }

    pub async fn send_uni_stream_to(
        &self,
        cid: &[u8; CID_LEN],
        stream_payload: &[u8],
    ) -> Result<u64, NetError> {
        let maybe_send = {
            let mut map = self.peers.lock().await;
            if let Some(slot) = map.get_mut(cid) {
                let (sid, pkts) = slot.conn.encode_uni_stream_packets(stream_payload)?;
                Some((slot.conn.peer_addr, sid, pkts))
            } else {
                None
            }
        };
        if let Some((addr, sid, pkts)) = maybe_send {
            for w in pkts {
                send_udp_with_impairment(
                    &self.sock,
                    addr,
                    w,
                    self.impairment_cfg,
                    &self.rng,
                    &self.counters,
                )
                .await?;
            }
            return Ok(sid);
        }
        Err(NetError::ConnectionClosed)
    }

    pub fn close(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

impl Drop for QuicServerEndpoint {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

// ============================================================================
// 2. `HMAC-SHA256` + `HKDF-SHA256` Seamless Session Ticket (`<= 1.0s`, `§9.1`)
// ============================================================================

pub const HMAC_SESSION_TICKET_WIRE_LEN: usize = 68;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HmacSessionTicket {
    pub account_id: u64,
    pub entity_slot: u32,
    pub entity_gen: u32,
    pub issued_tick: u64,
    pub expiry_tick: u64,
    pub reconnect_seq: u32,
    pub mac_tag: [u8; 32],
}

impl HmacSessionTicket {
    pub fn encode(&self) -> [u8; HMAC_SESSION_TICKET_WIRE_LEN] {
        let mut out = [0u8; HMAC_SESSION_TICKET_WIRE_LEN];
        out[0..8].copy_from_slice(&self.account_id.to_le_bytes());
        out[8..12].copy_from_slice(&self.entity_slot.to_le_bytes());
        out[12..16].copy_from_slice(&self.entity_gen.to_le_bytes());
        out[16..24].copy_from_slice(&self.issued_tick.to_le_bytes());
        out[24..32].copy_from_slice(&self.expiry_tick.to_le_bytes());
        out[32..36].copy_from_slice(&self.reconnect_seq.to_le_bytes());
        out[36..68].copy_from_slice(&self.mac_tag);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NetError> {
        let s = bytes
            .get(..HMAC_SESSION_TICKET_WIRE_LEN)
            .ok_or(NetError::MalformedWirePacket("Truncated HmacSessionTicket"))?;
        let mut acc = [0u8; 8];
        acc.copy_from_slice(s.get(0..8).ok_or(NetError::MalformedWirePacket("acc"))?);
        let mut es = [0u8; 4];
        es.copy_from_slice(s.get(8..12).ok_or(NetError::MalformedWirePacket("es"))?);
        let mut eg = [0u8; 4];
        eg.copy_from_slice(s.get(12..16).ok_or(NetError::MalformedWirePacket("eg"))?);
        let mut iss = [0u8; 8];
        iss.copy_from_slice(s.get(16..24).ok_or(NetError::MalformedWirePacket("iss"))?);
        let mut exp = [0u8; 8];
        exp.copy_from_slice(s.get(24..32).ok_or(NetError::MalformedWirePacket("exp"))?);
        let mut seq = [0u8; 4];
        seq.copy_from_slice(s.get(32..36).ok_or(NetError::MalformedWirePacket("seq"))?);
        let mut mac_tag = [0u8; 32];
        mac_tag.copy_from_slice(s.get(36..68).ok_or(NetError::MalformedWirePacket("mac"))?);

        Ok(Self {
            account_id: u64::from_le_bytes(acc),
            entity_slot: u32::from_le_bytes(es),
            entity_gen: u32::from_le_bytes(eg),
            issued_tick: u64::from_le_bytes(iss),
            expiry_tick: u64::from_le_bytes(exp),
            reconnect_seq: u32::from_le_bytes(seq),
            mac_tag,
        })
    }
}

pub struct SessionTicketManager {
    hmac_key: [u8; 32],
    active_reconnect_seq: BTreeMap<u64, u32>,
}

impl SessionTicketManager {
    pub fn new(master_secret: &[u8; 32]) -> Result<Self, NetError> {
        let hk = Hkdf::<Sha256>::new(Some(b"game-session-ticket-salt-v1"), master_secret);
        let mut hmac_key = [0u8; 32];
        hk.expand(b"session-ticket-hmac-sha256", &mut hmac_key)
            .map_err(|_| NetError::CryptoFailure)?;
        Ok(Self {
            hmac_key,
            active_reconnect_seq: BTreeMap::new(),
        })
    }

    pub fn issue_ticket(
        &self,
        account_id: u64,
        entity: EntityId,
        current_tick: u64,
        reconnect_seq: u32,
    ) -> Result<HmacSessionTicket, NetError> {
        let expiry_tick = current_tick.saturating_add(SESSION_TICKET_MAX_RECOVERY_TICKS);
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.hmac_key)
            .map_err(|_| NetError::CryptoFailure)?;
        mac.update(&account_id.to_le_bytes());
        mac.update(&entity.0.to_le_bytes());
        mac.update(&entity.1.to_le_bytes());
        mac.update(&current_tick.to_le_bytes());
        mac.update(&expiry_tick.to_le_bytes());
        mac.update(&reconnect_seq.to_le_bytes());
        let tag_bytes = mac.finalize().into_bytes();
        let mut mac_tag = [0u8; 32];
        mac_tag.copy_from_slice(&tag_bytes[..32]);

        Ok(HmacSessionTicket {
            account_id,
            entity_slot: entity.0,
            entity_gen: entity.1,
            issued_tick: current_tick,
            expiry_tick,
            reconnect_seq,
            mac_tag,
        })
    }

    #[inline]
    pub fn active_seq_for(&self, account_id: u64) -> u32 {
        self.active_reconnect_seq
            .get(&account_id)
            .copied()
            .unwrap_or(1)
    }

    /// Validates `HMAC-SHA256` signature, `<= 1.0s` (`60` ticks) window, AND active `reconnect_seq` revocation (`§9.1`).
    pub fn verify_live_packet_ticket(
        &self,
        ticket: &HmacSessionTicket,
        current_tick: u64,
    ) -> Result<EntityId, NetError> {
        if current_tick > ticket.expiry_tick
            || ticket.expiry_tick.saturating_sub(ticket.issued_tick)
                > SESSION_TICKET_MAX_RECOVERY_TICKS
        {
            return Err(NetError::SessionTicketExpired);
        }
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.hmac_key)
            .map_err(|_| NetError::CryptoFailure)?;
        mac.update(&ticket.account_id.to_le_bytes());
        mac.update(&ticket.entity_slot.to_le_bytes());
        mac.update(&ticket.entity_gen.to_le_bytes());
        mac.update(&ticket.issued_tick.to_le_bytes());
        mac.update(&ticket.expiry_tick.to_le_bytes());
        mac.update(&ticket.reconnect_seq.to_le_bytes());
        mac.verify_slice(&ticket.mac_tag)
            .map_err(|_| NetError::InvalidSessionTicket)?;

        let expected_seq = self.active_seq_for(ticket.account_id);
        if ticket.reconnect_seq < expected_seq {
            return Err(NetError::SessionReplayDetected);
        }
        if ticket.reconnect_seq > expected_seq {
            return Err(NetError::InvalidSessionTicket);
        }
        Ok(EntityId(ticket.entity_slot, ticket.entity_gen))
    }

    /// Consumes a reconnection attempt, revoking all prior `reconnect_seq` tickets and issuing `seq + 1`.
    pub fn accept_reconnect(
        &mut self,
        ticket: &HmacSessionTicket,
        current_tick: u64,
    ) -> Result<(EntityId, HmacSessionTicket), NetError> {
        let entity = self.verify_live_packet_ticket(ticket, current_tick)?;
        let next_seq = ticket
            .reconnect_seq
            .checked_add(1)
            .ok_or(NetError::InvalidSessionTicket)?;
        self.active_reconnect_seq
            .insert(ticket.account_id, next_seq);
        let next_ticket = self.issue_ticket(ticket.account_id, entity, current_tick, next_seq)?;
        Ok((entity, next_ticket))
    }
}

// ============================================================================
// 3. Battle Plane Datagrams (`<= 1,100B`), 8-Tick Window & Priority Delta Chunks
// ============================================================================

/// Updates a client's `(ack_server_tick, ack_bitfield)` 64-tick selective ACK bitmask once all chunks of `server_tick` arrive.
pub fn record_received_delta_chunk(
    ack_server_tick: &mut u64,
    ack_bitfield: &mut u64,
    partial_chunks: &mut BTreeMap<u64, (u8, u8)>,
    server_tick: u64,
    chunk_index: u8,
    chunk_total: u8,
) {
    if server_tick == 0 || chunk_total == 0 || chunk_total > 8 || chunk_index >= chunk_total {
        return;
    }
    let entry = partial_chunks
        .entry(server_tick)
        .or_insert((0, chunk_total));
    entry.0 |= 1_u8 << chunk_index;
    entry.1 = chunk_total;
    let full_mask = if chunk_total == 8 {
        u8::MAX
    } else {
        (1_u8 << chunk_total).saturating_sub(1)
    };
    if (entry.0 & full_mask) != full_mask {
        return;
    }
    partial_chunks.remove(&server_tick);
    if let Some(cutoff) = ack_server_tick.checked_sub(64) {
        let stale: Vec<u64> = partial_chunks.range(..=cutoff).map(|(&k, _)| k).collect();
        for k in stale {
            partial_chunks.remove(&k);
        }
    }

    if *ack_server_tick == 0 {
        *ack_server_tick = server_tick;
        *ack_bitfield = 1;
    } else if server_tick > *ack_server_tick {
        let shift = server_tick.saturating_sub(*ack_server_tick);
        if shift >= 64 {
            *ack_bitfield = 1;
        } else {
            *ack_bitfield = (*ack_bitfield << shift) | 1;
        }
        *ack_server_tick = server_tick;
    } else {
        let diff = ack_server_tick.saturating_sub(server_tick);
        if diff < 64 {
            *ack_bitfield |= 1_u64 << diff;
        }
    }
}

/// Client -> Server unreliable QUIC datagram (`<= 1,100B`) packing the recent `8` server ticks of inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientInputDatagram {
    pub account_id: u64,
    pub ack_server_tick: u64,
    pub ack_bitfield: u64,
    pub window_len: u8,
    pub inputs: [RawWireInput; SLIDING_WINDOW_TICKS],
    pub ticket: HmacSessionTicket,
}

/// Verified payload returned after `Packet<Unverified>::verify_bounds()` succeeds across the `8`-tick window.
pub type VerifiedClientInputPayload = (u64, EntityId, u64, u64, Vec<InputFrame>, HmacSessionTicket);

impl ClientInputDatagram {
    pub fn new(
        account_id: u64,
        ack_server_tick: u64,
        ack_bitfield: u64,
        window_len: u8,
        inputs: [RawWireInput; SLIDING_WINDOW_TICKS],
        ticket: HmacSessionTicket,
    ) -> Self {
        Self {
            account_id,
            ack_server_tick,
            ack_bitfield,
            window_len: window_len.min(SLIDING_WINDOW_TICKS as u8),
            inputs,
            ticket,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, NetError> {
        let count = usize::from(self.window_len).min(SLIDING_WINDOW_TICKS);
        let mut buf = Vec::with_capacity(428);
        buf.extend_from_slice(b"CID1"); // 4B magic
        buf.extend_from_slice(&self.account_id.to_le_bytes());
        buf.extend_from_slice(&self.ack_server_tick.to_le_bytes());
        buf.extend_from_slice(&self.ack_bitfield.to_le_bytes());
        buf.push(count as u8);

        let mut i: usize = 0;
        while i < count {
            if let Some(raw) = self.inputs.get(i) {
                buf.extend_from_slice(&raw.tick.to_le_bytes());
                buf.extend_from_slice(&raw.move_dir_milli[0].to_le_bytes());
                buf.extend_from_slice(&raw.move_dir_milli[1].to_le_bytes());
                buf.extend_from_slice(&raw.move_dir_milli[2].to_le_bytes());
                buf.extend_from_slice(&raw.aim_yaw_turn_milli.to_le_bytes());
                buf.extend_from_slice(&raw.aim_pitch_turn_milli.to_le_bytes());
                buf.extend_from_slice(&raw.action_mask.to_le_bytes());
                buf.push(raw.active_weapon_slot);
            }
            i = i.saturating_add(1);
        }
        buf.extend_from_slice(&self.ticket.encode());

        if buf.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(buf.len()));
        }
        Ok(buf)
    }

    /// Decodes wire bytes and enforces `Packet<Unverified>::verify_bounds() -> Packet<Verified> -> InputFrame`
    /// across the entire `8`-tick sliding window BEFORE returning any simulation-consumable input!
    pub fn decode_and_verify(
        bytes: &[u8],
        ticket_mgr: &SessionTicketManager,
        current_server_tick: u64,
    ) -> Result<VerifiedClientInputPayload, NetError> {
        if bytes.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(bytes.len()));
        }
        if bytes.get(0..4) != Some(b"CID1".as_slice()) {
            return Err(NetError::MalformedWirePacket("Invalid CID1 magic"));
        }
        let mut acc_b = [0u8; 8];
        acc_b.copy_from_slice(
            bytes
                .get(4..12)
                .ok_or(NetError::MalformedWirePacket("Truncated account_id"))?,
        );
        let account_id = u64::from_le_bytes(acc_b);

        let mut ack_b = [0u8; 8];
        ack_b.copy_from_slice(
            bytes
                .get(12..20)
                .ok_or(NetError::MalformedWirePacket("Truncated ack_server_tick"))?,
        );
        let ack_server_tick = u64::from_le_bytes(ack_b);

        let mut bit_b = [0u8; 8];
        bit_b.copy_from_slice(
            bytes
                .get(20..28)
                .ok_or(NetError::MalformedWirePacket("Truncated ack_bitfield"))?,
        );
        let ack_bitfield = u64::from_le_bytes(bit_b);

        let count = usize::from(
            *bytes
                .get(28)
                .ok_or(NetError::MalformedWirePacket("Truncated window_len"))?,
        );
        if count == 0 || count > SLIDING_WINDOW_TICKS {
            return Err(NetError::MalformedWirePacket("Invalid window_len"));
        }

        const RAW_WIRE_INPUT_WIRE_LEN: usize = 33;
        let expected_len = 29usize
            .saturating_add(count.saturating_mul(RAW_WIRE_INPUT_WIRE_LEN))
            .saturating_add(HMAC_SESSION_TICKET_WIRE_LEN);
        if bytes.len() != expected_len {
            return Err(NetError::MalformedWirePacket("Exact CID1 length mismatch"));
        }

        let ticket_off = 29usize.saturating_add(count.saturating_mul(RAW_WIRE_INPUT_WIRE_LEN));
        let ticket = HmacSessionTicket::decode(
            bytes
                .get(ticket_off..)
                .ok_or(NetError::MalformedWirePacket("Missing ticket"))?,
        )?;
        if ticket.account_id != account_id {
            return Err(NetError::InvalidSessionTicket);
        }
        let bound_entity = ticket_mgr.verify_live_packet_ticket(&ticket, current_server_tick)?;

        let mut verified_frames = Vec::with_capacity(count);
        let mut offset: usize = 29;
        let mut i: usize = 0;
        while i < count {
            let rec = bytes
                .get(offset..offset.saturating_add(RAW_WIRE_INPUT_WIRE_LEN))
                .ok_or(NetError::MalformedWirePacket("Truncated RawWireInput"))?;
            let mut tb = [0u8; 8];
            tb.copy_from_slice(rec.get(0..8).ok_or(NetError::MalformedWirePacket("t"))?);
            let mut m0 = [0u8; 4];
            m0.copy_from_slice(rec.get(8..12).ok_or(NetError::MalformedWirePacket("m0"))?);
            let mut m1 = [0u8; 4];
            m1.copy_from_slice(rec.get(12..16).ok_or(NetError::MalformedWirePacket("m1"))?);
            let mut m2 = [0u8; 4];
            m2.copy_from_slice(rec.get(16..20).ok_or(NetError::MalformedWirePacket("m2"))?);
            let mut yb = [0u8; 4];
            yb.copy_from_slice(rec.get(20..24).ok_or(NetError::MalformedWirePacket("yaw"))?);
            let mut pb = [0u8; 4];
            pb.copy_from_slice(rec.get(24..28).ok_or(NetError::MalformedWirePacket("pitch"))?);
            let mut ab = [0u8; 4];
            ab.copy_from_slice(rec.get(28..32).ok_or(NetError::MalformedWirePacket("act"))?);
            let slot = *rec.get(32).ok_or(NetError::MalformedWirePacket("slot"))?;

            let raw = RawWireInput {
                tick: u64::from_le_bytes(tb),
                move_dir_milli: [
                    i32::from_le_bytes(m0),
                    i32::from_le_bytes(m1),
                    i32::from_le_bytes(m2),
                ],
                aim_yaw_turn_milli: i32::from_le_bytes(yb),
                aim_pitch_turn_milli: i32::from_le_bytes(pb),
                action_mask: u32::from_le_bytes(ab),
                active_weapon_slot: slot,
            };
            // Typestate Contract #1 (`verifier_abi`): Unverified -> Verified -> InputFrame
            let verified: Packet<Verified> = Packet::<Unverified>::new_unverified(raw)
                .verify_bounds()
                .map_err(NetError::PacketBoundaryRejected)?;
            verified_frames.push(verified.into_input_frame());

            offset = offset.saturating_add(RAW_WIRE_INPUT_WIRE_LEN);
            i = i.saturating_add(1);
        }

        Ok((
            account_id,
            bound_entity,
            ack_server_tick,
            ack_bitfield,
            verified_frames,
            ticket,
        ))
    }
}

/// Exact `Q32.32` entity delta record (`44` bytes on wire, zero floating-point loss!).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityDelta {
    pub entity: EntityId,
    pub kind: u8,
    pub mobility: u8,
    pub active_weapon: u8,
    pub is_ghost: bool,
    pub pos_x_raw: i64,
    pub pos_y_raw: i64,
    pub pos_z_raw: i64,
    pub hp_milli: i32,
    pub shield_milli: i32,
}

impl EntityDelta {
    pub const WIRE_LEN: usize = 44;

    pub fn from_arena(arena: &CombatArena, slot: usize, is_ghost: bool) -> Option<Self> {
        if slot >= MAX_ENTITIES || *arena.alive.get(slot)? == 0 {
            return None;
        }
        let generation = *arena.generations.get(slot)?;
        let hp_raw = arena.hp.get(slot)?.raw();
        let sh_raw = arena.shield.get(slot)?.raw();
        let hp_milli = i32::try_from((hp_raw.saturating_mul(1_000)) >> 32).unwrap_or(i32::MAX);
        let shield_milli = i32::try_from((sh_raw.saturating_mul(1_000)) >> 32).unwrap_or(i32::MAX);

        Some(Self {
            entity: EntityId(slot as u32, generation),
            kind: *arena.kinds.get(slot)?,
            mobility: *arena.mobility.get(slot)?,
            active_weapon: *arena.active_weapon.get(slot)?,
            is_ghost,
            pos_x_raw: arena.pos_x.get(slot)?.raw(),
            pos_y_raw: arena.pos_y.get(slot)?.raw(),
            pos_z_raw: arena.pos_z.get(slot)?.raw(),
            hp_milli,
            shield_milli,
        })
    }

    pub fn differs_from(&self, baseline: &Self) -> bool {
        self.entity != baseline.entity
            || self.kind != baseline.kind
            || self.mobility != baseline.mobility
            || self.active_weapon != baseline.active_weapon
            || self.pos_x_raw != baseline.pos_x_raw
            || self.pos_y_raw != baseline.pos_y_raw
            || self.pos_z_raw != baseline.pos_z_raw
            || self.hp_milli != baseline.hp_milli
            || self.shield_milli != baseline.shield_milli
    }

    pub fn encode(&self) -> [u8; Self::WIRE_LEN] {
        let mut b = [0u8; Self::WIRE_LEN];
        b[0..4].copy_from_slice(&self.entity.0.to_le_bytes());
        b[4..8].copy_from_slice(&self.entity.1.to_le_bytes());
        b[8] = self.kind;
        b[9] = self.mobility;
        b[10] = self.active_weapon;
        b[11] = u8::from(self.is_ghost);
        b[12..20].copy_from_slice(&self.pos_x_raw.to_le_bytes());
        b[20..28].copy_from_slice(&self.pos_y_raw.to_le_bytes());
        b[28..36].copy_from_slice(&self.pos_z_raw.to_le_bytes());
        b[36..40].copy_from_slice(&self.hp_milli.to_le_bytes());
        b[40..44].copy_from_slice(&self.shield_milli.to_le_bytes());
        b
    }

    pub fn decode(s: &[u8]) -> Result<Self, NetError> {
        let s = s
            .get(..Self::WIRE_LEN)
            .ok_or(NetError::MalformedWirePacket("Truncated EntityDelta"))?;
        let mut e0 = [0u8; 4];
        e0.copy_from_slice(s.get(0..4).ok_or(NetError::MalformedWirePacket("e0"))?);
        let mut e1 = [0u8; 4];
        e1.copy_from_slice(s.get(4..8).ok_or(NetError::MalformedWirePacket("e1"))?);
        let mut px = [0u8; 8];
        px.copy_from_slice(s.get(12..20).ok_or(NetError::MalformedWirePacket("px"))?);
        let mut py = [0u8; 8];
        py.copy_from_slice(s.get(20..28).ok_or(NetError::MalformedWirePacket("py"))?);
        let mut pz = [0u8; 8];
        pz.copy_from_slice(s.get(28..36).ok_or(NetError::MalformedWirePacket("pz"))?);
        let mut hp = [0u8; 4];
        hp.copy_from_slice(s.get(36..40).ok_or(NetError::MalformedWirePacket("hp"))?);
        let mut sh = [0u8; 4];
        sh.copy_from_slice(s.get(40..44).ok_or(NetError::MalformedWirePacket("sh"))?);

        Ok(Self {
            entity: EntityId(u32::from_le_bytes(e0), u32::from_le_bytes(e1)),
            kind: *s.get(8).ok_or(NetError::MalformedWirePacket("kind"))?,
            mobility: *s.get(9).ok_or(NetError::MalformedWirePacket("mob"))?,
            active_weapon: *s.get(10).ok_or(NetError::MalformedWirePacket("wpn"))?,
            is_ghost: *s.get(11).ok_or(NetError::MalformedWirePacket("gh"))? != 0,
            pos_x_raw: i64::from_le_bytes(px),
            pos_y_raw: i64::from_le_bytes(py),
            pos_z_raw: i64::from_le_bytes(pz),
            hp_milli: i32::from_le_bytes(hp),
            shield_milli: i32::from_le_bytes(sh),
        })
    }
}

/// Server -> Client priority delta chunk datagram (`<= 1,100B`, `게임.md §9.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerDeltaDatagram {
    pub server_tick: u64,
    pub ack_client_tick: u64,
    pub baseline_ack_tick: u64,
    pub chunk_index: u8,
    pub chunk_total: u8,
    pub pinned_epoch_merkle: [u8; 32],
    pub refreshed_ticket: HmacSessionTicket,
    pub deltas: Vec<EntityDelta>,
}

impl ServerDeltaDatagram {
    pub fn encode(&self) -> Result<Vec<u8>, NetError> {
        if self.deltas.len() > MAX_DELTAS_PER_DATAGRAM_CHUNK {
            return Err(NetError::DatagramExceeds1100Bytes(self.deltas.len()));
        }
        // Header = 4 + 8 + 8 + 8 + 1 + 1 + 32 + 68 + 1 = 131B; + 22 * 44B (968B) = 1,099B <= 1,100B!
        let mut buf = Vec::with_capacity(131usize.saturating_add(self.deltas.len().saturating_mul(44)));
        buf.extend_from_slice(b"SWD1");
        buf.extend_from_slice(&self.server_tick.to_le_bytes());
        buf.extend_from_slice(&self.ack_client_tick.to_le_bytes());
        buf.extend_from_slice(&self.baseline_ack_tick.to_le_bytes());
        buf.push(self.chunk_index);
        buf.push(self.chunk_total);
        buf.extend_from_slice(&self.pinned_epoch_merkle);
        buf.extend_from_slice(&self.refreshed_ticket.encode());
        buf.push(self.deltas.len() as u8);
        for d in &self.deltas {
            buf.extend_from_slice(&d.encode());
        }
        if buf.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(buf.len()));
        }
        Ok(buf)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, NetError> {
        if bytes.len() > MAX_DATAGRAM_PAYLOAD_BYTES {
            return Err(NetError::DatagramExceeds1100Bytes(bytes.len()));
        }
        if bytes.get(0..4) != Some(b"SWD1".as_slice()) || bytes.len() < 131 {
            return Err(NetError::MalformedWirePacket("Invalid SWD1 header"));
        }
        let mut st = [0u8; 8];
        st.copy_from_slice(bytes.get(4..12).ok_or(NetError::MalformedWirePacket("st"))?);
        let mut act = [0u8; 8];
        act.copy_from_slice(bytes.get(12..20).ok_or(NetError::MalformedWirePacket("act"))?);
        let mut bat = [0u8; 8];
        bat.copy_from_slice(bytes.get(20..28).ok_or(NetError::MalformedWirePacket("bat"))?);
        let chunk_index = *bytes.get(28).ok_or(NetError::MalformedWirePacket("ci"))?;
        let chunk_total = *bytes.get(29).ok_or(NetError::MalformedWirePacket("ct"))?;
        let mut root = [0u8; 32];
        root.copy_from_slice(bytes.get(30..62).ok_or(NetError::MalformedWirePacket("root"))?);
        let refreshed_ticket = HmacSessionTicket::decode(
            bytes
                .get(62..130)
                .ok_or(NetError::MalformedWirePacket("ticket"))?,
        )?;
        let count = usize::from(*bytes.get(130).ok_or(NetError::MalformedWirePacket("cnt"))?);
        let expected = 131usize.saturating_add(count.saturating_mul(EntityDelta::WIRE_LEN));
        if bytes.len() != expected {
            return Err(NetError::MalformedWirePacket("SWD1 exact length mismatch"));
        }
        let mut deltas = Vec::with_capacity(count);
        let mut off: usize = 131;
        let mut i: usize = 0;
        while i < count {
            deltas.push(EntityDelta::decode(
                bytes
                    .get(off..off.saturating_add(EntityDelta::WIRE_LEN))
                    .ok_or(NetError::MalformedWirePacket("delta"))?,
            )?);
            off = off.saturating_add(EntityDelta::WIRE_LEN);
            i = i.saturating_add(1);
        }

        Ok(Self {
            server_tick: u64::from_le_bytes(st),
            ack_client_tick: u64::from_le_bytes(act),
            baseline_ack_tick: u64::from_le_bytes(bat),
            chunk_index,
            chunk_total,
            pinned_epoch_merkle: root,
            refreshed_ticket,
            deltas,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PriorityDeltaParams {
    pub server_tick: u64,
    pub ack_client_tick: u64,
    pub client_ack_server_tick: u64,
    pub self_entity: EntityId,
    pub pinned_epoch_merkle: [u8; 32],
    pub refreshed_ticket: HmacSessionTicket,
    pub max_chunks_per_tick: usize,
}

/// Per-client entity delta synchronization tracker preventing stale suppression of truncated or lost deltas (`§9.1`).
#[derive(Clone, Debug, Default)]
pub struct ClientEntitySyncState {
    pub confirmed_entity_tick: BTreeMap<u32, u64>,
    pub starvation_age: BTreeMap<u32, u16>,
    pub sent_history: VecDeque<(u64, Vec<u32>)>,
}

impl ClientEntitySyncState {
    pub fn process_client_ack(&mut self, ack_server_tick: u64, ack_bitfield: u64) {
        if ack_server_tick == 0 && ack_bitfield == 0 {
            return;
        }
        for (sent_tick, slots) in &self.sent_history {
            if *sent_tick <= ack_server_tick {
                let diff = ack_server_tick.saturating_sub(*sent_tick);
                if diff < 64 && ((ack_bitfield >> diff) & 1) == 1 {
                    for &slot_u32 in slots {
                        let prev = self
                            .confirmed_entity_tick
                            .get(&slot_u32)
                            .copied()
                            .unwrap_or(0);
                        if *sent_tick > prev {
                            self.confirmed_entity_tick.insert(slot_u32, *sent_tick);
                        }
                    }
                }
            }
        }
        while self.sent_history.len() > SERVER_RINGBUFFER_TICKS {
            let _ = self.sent_history.pop_front();
        }
    }

    pub fn record_sent_chunks(&mut self, server_tick: u64, chunks: &[ServerDeltaDatagram]) {
        let mut slots = Vec::new();
        for ch in chunks {
            for d in &ch.deltas {
                slots.push(d.entity.0);
            }
        }
        self.sent_history.push_back((server_tick, slots));
        while self.sent_history.len() > SERVER_RINGBUFFER_TICKS {
            let _ = self.sent_history.pop_front();
        }
    }
}

/// Builds per-entity ACK-confirmed priority-sorted `<= 1,100B` entity delta chunks (`게임.md §9.1`).
pub fn build_ack_priority_delta_chunks(
    params: &PriorityDeltaParams,
    visible_entities: &[(u32, bool)],
    current_arena: &CombatArena,
    ring: &ServerSoARingBuffer64,
    sync_state: &mut ClientEntitySyncState,
) -> Result<Vec<ServerDeltaDatagram>, NetError> {
    let self_pos = current_arena
        .position(params.self_entity)
        .unwrap_or(QVec3::ZERO);

    // `(priority_score_desc, slot_asc, EntityDelta)`
    let mut scored: Vec<(u64, u32, EntityDelta)> = Vec::with_capacity(visible_entities.len());

    for &(slot_u32, is_ghost) in visible_entities {
        let slot = slot_u32 as usize;
        let Some(cur_delta) = EntityDelta::from_arena(current_arena, slot, is_ghost) else {
            continue;
        };

        let confirmed_tick = sync_state
            .confirmed_entity_tick
            .get(&slot_u32)
            .copied()
            .unwrap_or(0);
        let entity_baseline = if confirmed_tick > 0 {
            ring.snapshot_at(confirmed_tick)
        } else {
            None
        };
        let changed = match entity_baseline.and_then(|b| EntityDelta::from_arena(b, slot, is_ghost))
        {
            Some(base_delta) => cur_delta.differs_from(&base_delta),
            None => true,
        };
        let age = sync_state.starvation_age.get(&slot_u32).copied().unwrap_or(0);
        let is_self = cur_delta.entity == params.self_entity;
        let is_boss = cur_delta.kind == (EntityKind::Boss as u8);

        if !changed && age == 0 && !is_self && !is_boss {
            continue; // Zero-byte suppression ONLY when this exact entity's state was ACK-confirmed!
        }

        let epos = QVec3::new(
            Q32::from_raw(cur_delta.pos_x_raw),
            Q32::from_raw(cur_delta.pos_y_raw),
            Q32::from_raw(cur_delta.pos_z_raw),
        );
        let d = epos.sat_sub(self_pos);
        let dist_m = (d.x.sat_abs().to_int_floor() as u64)
            .saturating_add(d.y.sat_abs().to_int_floor() as u64)
            .saturating_add(d.z.sat_abs().to_int_floor() as u64);
        let proximity_bonus = 5_000_u64.saturating_sub(dist_m.saturating_mul(40));

        let mut score = proximity_bonus.saturating_add(u64::from(age).saturating_mul(250));
        if is_self {
            score = score.saturating_add(1_000_000);
        } else if is_boss {
            score = score.saturating_add(800_000);
        } else if !is_ghost {
            score = score.saturating_add(10_000);
        }
        scored.push((score, slot_u32, cur_delta));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let cap_chunks = params.max_chunks_per_tick.clamp(1, 8);
    let max_records = cap_chunks.saturating_mul(MAX_DELTAS_PER_DATAGRAM_CHUNK);
    for (idx, &(_, slot_u32, _)) in scored.iter().enumerate() {
        if idx < max_records {
            sync_state.starvation_age.insert(slot_u32, 0);
        } else {
            let prev = sync_state
                .starvation_age
                .get(&slot_u32)
                .copied()
                .unwrap_or(0);
            sync_state
                .starvation_age
                .insert(slot_u32, prev.saturating_add(1));
        }
    }
    scored.truncate(max_records);

    if scored.is_empty() {
        let empty_out = vec![ServerDeltaDatagram {
            server_tick: params.server_tick,
            ack_client_tick: params.ack_client_tick,
            baseline_ack_tick: params.client_ack_server_tick,
            chunk_index: 0,
            chunk_total: 1,
            pinned_epoch_merkle: params.pinned_epoch_merkle,
            refreshed_ticket: params.refreshed_ticket,
            deltas: Vec::new(),
        }];
        sync_state.record_sent_chunks(params.server_tick, &empty_out);
        return Ok(empty_out);
    }

    let total_chunks = scored
        .len()
        .saturating_add(MAX_DELTAS_PER_DATAGRAM_CHUNK.saturating_sub(1))
        .checked_div(MAX_DELTAS_PER_DATAGRAM_CHUNK)
        .unwrap_or(1)
        .max(1);

    let mut out = Vec::with_capacity(total_chunks);
    let mut chunk_idx: usize = 0;
    while chunk_idx < total_chunks {
        let start = chunk_idx.saturating_mul(MAX_DELTAS_PER_DATAGRAM_CHUNK);
        let end = start
            .saturating_add(MAX_DELTAS_PER_DATAGRAM_CHUNK)
            .min(scored.len());
        let mut chunk_deltas = Vec::with_capacity(end.saturating_sub(start));
        if let Some(slice) = scored.get(start..end) {
            for &(_, _, d) in slice {
                chunk_deltas.push(d);
            }
        }
        out.push(ServerDeltaDatagram {
            server_tick: params.server_tick,
            ack_client_tick: params.ack_client_tick,
            baseline_ack_tick: params.client_ack_server_tick,
            chunk_index: chunk_idx as u8,
            chunk_total: total_chunks as u8,
            pinned_epoch_merkle: params.pinned_epoch_merkle,
            refreshed_ticket: params.refreshed_ticket,
            deltas: chunk_deltas,
        });
        chunk_idx = chunk_idx.saturating_add(1);
    }
    sync_state.record_sent_chunks(params.server_tick, &out);
    Ok(out)
}

// ============================================================================
// 4. Per-Chunk Reliable Unidirectional QUIC Stream for Permanent SDF Diffs
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfBrickDiffRecord {
    pub coord: BrickCoord,
    pub voxel_changes: Vec<(u16, i16, u8)>,
    pub brick_blake3: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdfChunkDiffStreamPacket {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub server_tick: u64,
    pub bricks: Vec<SdfBrickDiffRecord>,
}

impl SdfChunkDiffStreamPacket {
    pub fn extract_from_dirty_bricks(
        world: &SparseSdfWorld,
        dirty_coords: &[BrickCoord],
        server_tick: u64,
    ) -> BTreeMap<(i32, i32), Self> {
        let mut by_chunk: BTreeMap<(i32, i32), Vec<SdfBrickDiffRecord>> = BTreeMap::new();
        for &coord in dirty_coords {
            let Some(brick) = world.brick(coord) else {
                continue;
            };
            let cx = coord.bx.div_euclid(4);
            let cz = coord.bz.div_euclid(4);

            let mut hasher = blake3::Hasher::new();
            let mut changes = Vec::new();
            let mut idx: usize = 0;
            while idx < BRICK_CELL_COUNT {
                let sdf = brick.sdf_cm.get(idx).copied().unwrap_or(100);
                let fld = brick.field_state.get(idx).copied().unwrap_or(0);
                hasher.update(&sdf.to_le_bytes());
                hasher.update(&[fld]);
                changes.push((idx as u16, sdf, fld));
                idx = idx.saturating_add(1);
            }
            let digest = *hasher.finalize().as_bytes();
            by_chunk
                .entry((cx, cz))
                .or_default()
                .push(SdfBrickDiffRecord {
                    coord,
                    voxel_changes: changes,
                    brick_blake3: digest,
                });
        }

        let mut packets = BTreeMap::new();
        for ((cx, cz), bricks) in by_chunk {
            packets.insert(
                (cx, cz),
                Self {
                    chunk_x: cx,
                    chunk_z: cz,
                    server_tick,
                    bricks,
                },
            );
        }
        packets
    }

    pub fn encode_stream_frame(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.push(STREAM_KIND_SDF_CHUNK_DIFF);
        out.extend_from_slice(&self.chunk_x.to_le_bytes());
        out.extend_from_slice(&self.chunk_z.to_le_bytes());
        out.extend_from_slice(&self.server_tick.to_le_bytes());
        let bcount = u16::try_from(self.bricks.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&bcount.to_le_bytes());
        for b in self.bricks.iter().take(usize::from(bcount)) {
            out.extend_from_slice(&b.coord.bx.to_le_bytes());
            out.extend_from_slice(&b.coord.by.to_le_bytes());
            out.extend_from_slice(&b.coord.bz.to_le_bytes());
            out.extend_from_slice(&b.brick_blake3);
            let vcount = u16::try_from(b.voxel_changes.len()).unwrap_or(u16::MAX);
            out.extend_from_slice(&vcount.to_le_bytes());
            for &(vidx, sdf, fld) in b.voxel_changes.iter().take(usize::from(vcount)) {
                out.extend_from_slice(&vidx.to_le_bytes());
                out.extend_from_slice(&sdf.to_le_bytes());
                out.push(fld);
            }
        }
        out
    }

    pub fn decode_stream_frame(bytes: &[u8]) -> Result<Self, NetError> {
        if bytes.first().copied() != Some(STREAM_KIND_SDF_CHUNK_DIFF) || bytes.len() < 19 {
            return Err(NetError::MalformedWirePacket("Invalid SDF stream header"));
        }
        let mut cx = [0u8; 4];
        cx.copy_from_slice(bytes.get(1..5).ok_or(NetError::MalformedWirePacket("cx"))?);
        let mut cz = [0u8; 4];
        cz.copy_from_slice(bytes.get(5..9).ok_or(NetError::MalformedWirePacket("cz"))?);
        let mut st = [0u8; 8];
        st.copy_from_slice(bytes.get(9..17).ok_or(NetError::MalformedWirePacket("st"))?);
        let mut bc = [0u8; 2];
        bc.copy_from_slice(bytes.get(17..19).ok_or(NetError::MalformedWirePacket("bc"))?);
        let brick_count = usize::from(u16::from_le_bytes(bc));

        let mut cur: usize = 19;
        let mut bricks = Vec::with_capacity(brick_count);
        let mut bi: usize = 0;
        while bi < brick_count {
            let hdr = bytes
                .get(cur..cur.saturating_add(46))
                .ok_or(NetError::MalformedWirePacket("Truncated brick diff header"))?;
            let mut bx = [0u8; 4];
            bx.copy_from_slice(hdr.get(0..4).ok_or(NetError::MalformedWirePacket("bx"))?);
            let mut by = [0u8; 4];
            by.copy_from_slice(hdr.get(4..8).ok_or(NetError::MalformedWirePacket("by"))?);
            let mut bz = [0u8; 4];
            bz.copy_from_slice(hdr.get(8..12).ok_or(NetError::MalformedWirePacket("bz"))?);
            let mut dig = [0u8; 32];
            dig.copy_from_slice(
                hdr.get(12..44)
                    .ok_or(NetError::MalformedWirePacket("dig"))?,
            );
            let mut vc = [0u8; 2];
            vc.copy_from_slice(hdr.get(44..46).ok_or(NetError::MalformedWirePacket("vc"))?);
            let vcount = usize::from(u16::from_le_bytes(vc));
            cur = cur.saturating_add(46);

            let mut changes = Vec::with_capacity(vcount);
            let mut vi: usize = 0;
            while vi < vcount {
                let vs = bytes
                    .get(cur..cur.saturating_add(5))
                    .ok_or(NetError::MalformedWirePacket("Truncated voxel diff"))?;
                let mut idx_b = [0u8; 2];
                idx_b.copy_from_slice(vs.get(0..2).ok_or(NetError::MalformedWirePacket("vi"))?);
                let mut sdf_b = [0u8; 2];
                sdf_b.copy_from_slice(vs.get(2..4).ok_or(NetError::MalformedWirePacket("sdf"))?);
                let fld = *vs.get(4).ok_or(NetError::MalformedWirePacket("fld"))?;
                changes.push((u16::from_le_bytes(idx_b), i16::from_le_bytes(sdf_b), fld));
                cur = cur.saturating_add(5);
                vi = vi.saturating_add(1);
            }
            bricks.push(SdfBrickDiffRecord {
                coord: BrickCoord {
                    bx: i32::from_le_bytes(bx),
                    by: i32::from_le_bytes(by),
                    bz: i32::from_le_bytes(bz),
                },
                voxel_changes: changes,
                brick_blake3: dig,
            });
            bi = bi.saturating_add(1);
        }

        Ok(Self {
            chunk_x: i32::from_le_bytes(cx),
            chunk_z: i32::from_le_bytes(cz),
            server_tick: u64::from_le_bytes(st),
            bricks,
        })
    }

    pub fn apply_to_world(&self, world: &mut SparseSdfWorld) -> Result<usize, NetError> {
        let mut applied = 0usize;
        for rec in &self.bricks {
            let mut updated = world.brick(rec.coord).cloned().unwrap_or_default();
            for &(vidx, sdf, fld) in &rec.voxel_changes {
                let idx = usize::from(vidx);
                if let (Some(s_ref), Some(f_ref)) =
                    (updated.sdf_cm.get_mut(idx), updated.field_state.get_mut(idx))
                {
                    *s_ref = sdf;
                    *f_ref = fld;
                }
            }
            let mut hasher = blake3::Hasher::new();
            let mut idx: usize = 0;
            while idx < BRICK_CELL_COUNT {
                hasher.update(&updated.sdf_cm.get(idx).copied().unwrap_or(100).to_le_bytes());
                hasher.update(&[updated.field_state.get(idx).copied().unwrap_or(0)]);
                idx = idx.saturating_add(1);
            }
            if *hasher.finalize().as_bytes() != rec.brick_blake3 {
                return Err(NetError::MalformedWirePacket("SDF brick BLAKE3 digest mismatch"));
            }
            if world.apply_streamed_brick(rec.coord, updated, self.server_tick) {
                applied = applied.saturating_add(1);
            }
        }
        Ok(applied)
    }
}

// ============================================================================
// 5. Spatial Hash AOI, 8-Neighbor Ghost Mirroring & Elastic Channel Split
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct ShardCoord {
    pub channel_id: u16,
    pub sx: i32,
    pub sz: i32,
}

impl ShardCoord {
    pub fn from_qvec3(channel_id: u16, pos: QVec3) -> Self {
        let xm = pos.x.to_int_floor();
        let zm = pos.z.to_int_floor();
        Self {
            channel_id,
            sx: xm.div_euclid(AOI_CELL_METERS),
            sz: zm.div_euclid(AOI_CELL_METERS),
        }
    }

    /// Computes all qualifying adjacent cells (including diagonal corners!) within `GHOST_MIRROR_MARGIN_METERS`.
    pub fn ghost_neighbor_cells(self, pos: QVec3) -> Vec<ShardCoord> {
        let xm = pos.x.to_int_floor();
        let zm = pos.z.to_int_floor();
        let lx = xm.rem_euclid(AOI_CELL_METERS);
        let lz = zm.rem_euclid(AOI_CELL_METERS);

        let dx_neg = lx < GHOST_MIRROR_MARGIN_METERS;
        let dx_pos = lx >= AOI_CELL_METERS.saturating_sub(GHOST_MIRROR_MARGIN_METERS);
        let dz_neg = lz < GHOST_MIRROR_MARGIN_METERS;
        let dz_pos = lz >= AOI_CELL_METERS.saturating_sub(GHOST_MIRROR_MARGIN_METERS);

        let mut neighbors = Vec::with_capacity(3);
        for &(dx, dz) in &[
            (-1_i32, 0_i32),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
        ] {
            let x_match = (dx == 0) || (dx == -1 && dx_neg) || (dx == 1 && dx_pos);
            let z_match = (dz == 0) || (dz == -1 && dz_neg) || (dz == 1 && dz_pos);
            if x_match && z_match {
                neighbors.push(ShardCoord {
                    channel_id: self.channel_id,
                    sx: self.sx.saturating_add(dx),
                    sz: self.sz.saturating_add(dz),
                });
            }
        }
        neighbors
    }
}

#[derive(Clone, Debug, Default)]
pub struct AoiSpatialGrid {
    home_members: BTreeMap<ShardCoord, BTreeSet<u32>>,
    ghost_mirrors: BTreeMap<ShardCoord, BTreeSet<u32>>,
}

impl AoiSpatialGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rebuild_from_arena(&mut self, channel_id: u16, arena: &CombatArena) {
        self.home_members.clear();
        self.ghost_mirrors.clear();
        let mut slot: usize = 0;
        while slot < MAX_ENTITIES {
            if arena.alive.get(slot).copied().unwrap_or(0) != 0 {
                let pos = QVec3::new(
                    arena.pos_x.get(slot).copied().unwrap_or(Q32::ZERO),
                    arena.pos_y.get(slot).copied().unwrap_or(Q32::ZERO),
                    arena.pos_z.get(slot).copied().unwrap_or(Q32::ZERO),
                );
                let home = ShardCoord::from_qvec3(channel_id, pos);
                self.home_members
                    .entry(home)
                    .or_default()
                    .insert(slot as u32);
                for ghost_cell in home.ghost_neighbor_cells(pos) {
                    self.ghost_mirrors
                        .entry(ghost_cell)
                        .or_default()
                        .insert(slot as u32);
                }
            }
            slot = slot.saturating_add(1);
        }
    }

    /// Multi-channel rebuild supporting live elastic channel splits (`§4.1, §8.3`):
    /// Places each player/projectile into `entity_channels[slot]` and mirrors Raid `Boss` entities across `active_channels`.
    pub fn rebuild_from_arena_channels(
        &mut self,
        arena: &CombatArena,
        entity_channels: &[u16],
        active_channels: &BTreeSet<u16>,
    ) {
        self.home_members.clear();
        self.ghost_mirrors.clear();
        let mut slot: usize = 0;
        while slot < MAX_ENTITIES {
            if arena.alive.get(slot).copied().unwrap_or(0) != 0 {
                let pos = QVec3::new(
                    arena.pos_x.get(slot).copied().unwrap_or(Q32::ZERO),
                    arena.pos_y.get(slot).copied().unwrap_or(Q32::ZERO),
                    arena.pos_z.get(slot).copied().unwrap_or(Q32::ZERO),
                );
                let kind = arena.kinds.get(slot).copied().unwrap_or(0);
                if kind == (EntityKind::Boss as u8) && !active_channels.is_empty() {
                    for &ch in active_channels {
                        let home = ShardCoord::from_qvec3(ch, pos);
                        self.home_members
                            .entry(home)
                            .or_default()
                            .insert(slot as u32);
                        for ghost_cell in home.ghost_neighbor_cells(pos) {
                            self.ghost_mirrors
                                .entry(ghost_cell)
                                .or_default()
                                .insert(slot as u32);
                        }
                    }
                } else {
                    let ch = entity_channels.get(slot).copied().unwrap_or(0);
                    let home = ShardCoord::from_qvec3(ch, pos);
                    self.home_members
                        .entry(home)
                        .or_default()
                        .insert(slot as u32);
                    for ghost_cell in home.ghost_neighbor_cells(pos) {
                        self.ghost_mirrors
                            .entry(ghost_cell)
                            .or_default()
                            .insert(slot as u32);
                    }
                }
            }
            slot = slot.saturating_add(1);
        }
    }

    /// Returns `(entity_slot, is_ghost)` visible to an observer in `shard`.
    pub fn visible_in_shard(&self, shard: ShardCoord) -> Vec<(u32, bool)> {
        let mut out = Vec::new();
        if let Some(homes) = self.home_members.get(&shard) {
            for &s in homes {
                out.push((s, false));
            }
        }
        if let Some(ghosts) = self.ghost_mirrors.get(&shard) {
            for &g in ghosts {
                out.push((g, true));
            }
        }
        out
    }

    pub fn total_ghost_mirrors(&self) -> usize {
        let mut sum: usize = 0;
        for set in self.ghost_mirrors.values() {
            sum = sum.saturating_add(set.len());
        }
        sum
    }

    /// Pre-overflow elastic channel split trigger before `SERVER_SHARD_TICK_FUEL_CAP (200,000)` (`§8.3`).
    #[inline]
    pub const fn should_induce_channel_split(shard_fuel_ops: u32, fuel_cap: u32) -> bool {
        // Induce elastic split at 80% of shard fuel cap (`160,000 < 200,000`) so cap is never breached.
        shard_fuel_ops >= (fuel_cap / 5).saturating_mul(4)
    }
}

// ============================================================================
// 6. `64`-Tick Server `SoA` Ringbuffer (`되감기 허용 창 <= 150ms`, `§9.1`)
// ============================================================================

pub struct ServerSoARingBuffer64 {
    arenas: Vec<Box<CombatArena>>,
    zones_per_tick: Vec<Vec<ZoneInstanceKind>>,
    channels_per_tick: Vec<Vec<u16>>,
    state_ticks: [u64; SERVER_RINGBUFFER_TICKS],
    inputs_per_tick: Vec<BTreeMap<EntityId, InputFrame>>,
    input_ticks: [u64; SERVER_RINGBUFFER_TICKS],
}

impl ServerSoARingBuffer64 {
    pub fn new(world_seed: u64) -> Self {
        let mut arenas = Vec::with_capacity(SERVER_RINGBUFFER_TICKS);
        let mut zones_per_tick = Vec::with_capacity(SERVER_RINGBUFFER_TICKS);
        let mut channels_per_tick = Vec::with_capacity(SERVER_RINGBUFFER_TICKS);
        let mut inputs_per_tick = Vec::with_capacity(SERVER_RINGBUFFER_TICKS);
        let mut i: usize = 0;
        while i < SERVER_RINGBUFFER_TICKS {
            arenas.push(CombatArena::new_boxed(world_seed));
            zones_per_tick.push(Vec::new());
            channels_per_tick.push(Vec::new());
            inputs_per_tick.push(BTreeMap::new());
            i = i.saturating_add(1);
        }
        Self {
            arenas,
            zones_per_tick,
            channels_per_tick,
            state_ticks: [u64::MAX; SERVER_RINGBUFFER_TICKS],
            inputs_per_tick,
            input_ticks: [u64::MAX; SERVER_RINGBUFFER_TICKS],
        }
    }

    #[inline]
    const fn slot_of(tick: u64) -> usize {
        (tick & ((SERVER_RINGBUFFER_TICKS as u64).saturating_sub(1))) as usize
    }

    /// Records the authoritative world snapshot at the boundary of `tick` WITHOUT overwriting `inputs_per_tick[slot(tick)]`!
    pub fn record_state_at_tick(
        &mut self,
        tick: u64,
        arena: &CombatArena,
        zones: &[ZoneInstanceKind],
        channels: &[u16],
    ) {
        let idx = Self::slot_of(tick);
        if let (Some(dst_arena), Some(dst_zones), Some(dst_ch), Some(t_slot)) = (
            self.arenas.get_mut(idx),
            self.zones_per_tick.get_mut(idx),
            self.channels_per_tick.get_mut(idx),
            self.state_ticks.get_mut(idx),
        ) {
            (**dst_arena).clone_from(arena);
            dst_zones.clear();
            dst_zones.extend_from_slice(zones);
            dst_ch.clear();
            dst_ch.extend_from_slice(channels);
            *t_slot = tick;
        }
    }

    /// Records the authoritative input map executed during `tick` in its own independent ring slot!
    pub fn record_inputs_for_tick(&mut self, tick: u64, tick_inputs: &BTreeMap<EntityId, InputFrame>) {
        let idx = Self::slot_of(tick);
        if let (Some(inp_slot), Some(it_slot)) = (
            self.inputs_per_tick.get_mut(idx),
            self.input_ticks.get_mut(idx),
        ) {
            inp_slot.clone_from(tick_inputs);
            *it_slot = tick;
        }
    }

    /// Looks up any snapshot still resident in the `64`-tick ringbuffer (used for per-entity ACK delta baselines).
    pub fn snapshot_at(&self, tick: u64) -> Option<&CombatArena> {
        let idx = Self::slot_of(tick);
        if self.state_ticks.get(idx).copied() == Some(tick) {
            self.arenas.get(idx).map(|b| &**b)
        } else {
            None
        }
    }

    /// Enforces the strict `<= 150ms` (`9` ticks at `60Hz`) rewind window (`게임.md §9.1`),
    /// restoring `CombatArena`, portal `zones`, and dynamic `entity_channels` atomically.
    pub fn restore_for_rewind(
        &self,
        current_tick: u64,
        target_tick: u64,
        out_arena: &mut CombatArena,
        out_zones: &mut Vec<ZoneInstanceKind>,
        out_channels: &mut Vec<u16>,
    ) -> bool {
        if target_tick > current_tick {
            return false;
        }
        let age_ticks = current_tick.saturating_sub(target_tick);
        if age_ticks > MAX_REWIND_TICKS_60HZ {
            return false;
        }
        let idx = Self::slot_of(target_tick);
        if self.state_ticks.get(idx).copied() != Some(target_tick) {
            return false;
        }
        let (Some(saved_arena), Some(saved_zones), Some(saved_ch)) = (
            self.arenas.get(idx),
            self.zones_per_tick.get(idx),
            self.channels_per_tick.get(idx),
        ) else {
            return false;
        };
        out_arena.clone_from(saved_arena);
        out_zones.clone_from(saved_zones);
        out_channels.clone_from(saved_ch);
        true
    }

    pub fn recorded_input(&self, tick: u64, entity: EntityId) -> Option<InputFrame> {
        let idx = Self::slot_of(tick);
        if self.input_ticks.get(idx).copied() == Some(tick) {
            self.inputs_per_tick
                .get(idx)
                .and_then(|m| m.get(&entity).copied())
        } else {
            None
        }
    }

    pub fn set_recorded_input(&mut self, tick: u64, entity: EntityId, frame: InputFrame) -> bool {
        let idx = Self::slot_of(tick);
        if let (Some(map), Some(it_slot)) = (
            self.inputs_per_tick.get_mut(idx),
            self.input_ticks.get_mut(idx),
        ) {
            if *it_slot != tick {
                map.clear();
                *it_slot = tick;
            }
            map.insert(entity, frame);
            return true;
        }
        false
    }
}

// ============================================================================
// 7. Additive `u16` TLV Epoch Manifest, `(E_a, E_b)` `ArcSwap` & Zero-Load Portal
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EpochHeaderRecordV1 {
    pub epoch_id: u64,
    pub season_id: u64,
    pub base_move_speed_raw: u64,
    pub expected_farm_ticks: u64,
    pub synth_stats_packed: u64,
    pub unknown_fields: Vec<TlvField>,
}

impl_tlv_u64_record!(
    EpochHeaderRecordV1,
    prev_schema: &[],
    schema_const: EPOCH_HEADER_SCHEMA_V1,
    fields: [
        (1, epoch_id, 0),
        (2, season_id, 1),
        (3, base_move_speed_raw, 0),
        (4, expected_farm_ticks, 600),
        (5, synth_stats_packed, 0),
    ],
    unknown_field: unknown_fields
);

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EpochHeaderRecordV2 {
    pub epoch_id: u64,
    pub season_id: u64,
    pub base_move_speed_raw: u64,
    pub expected_farm_ticks: u64,
    pub synth_stats_packed: u64,
    pub hot_balance_micro_revision: u64,
    pub boss_enrage_tuning_permille: u64,
    pub unknown_fields: Vec<TlvField>,
}

// Compile-time enforcement that `EPOCH_HEADER_SCHEMA_V2` is strictly additive over `V1` (`§9.2`)!
impl_tlv_u64_record!(
    EpochHeaderRecordV2,
    prev_schema: EPOCH_HEADER_SCHEMA_V1,
    schema_const: EPOCH_HEADER_SCHEMA_V2,
    fields: [
        (1, epoch_id, 0),
        (2, season_id, 1),
        (3, base_move_speed_raw, 0),
        (4, expected_farm_ticks, 600),
        (5, synth_stats_packed, 0),
        (6, hot_balance_micro_revision, 0),
        (7, boss_enrage_tuning_permille, 1000),
    ],
    unknown_field: unknown_fields
);

pub const EPOCH_BUNDLE_OUTER_SCHEMA_V1: &[(u16, FieldWireKind)] = &[
    (10, FieldWireKind::Bytes), // nested header TLV
    (11, FieldWireKind::Bytes), // 8 x i64 faucet_caps
    (12, FieldWireKind::Bytes), // qvm bytecode digest
    (13, FieldWireKind::Bytes), // spirv IR digest
    (14, FieldWireKind::Bytes), // chunk tree digest
];
pub const EPOCH_BUNDLE_OUTER_SCHEMA_V2: &[(u16, FieldWireKind)] = &[
    (10, FieldWireKind::Bytes),
    (11, FieldWireKind::Bytes),
    (12, FieldWireKind::Bytes),
    (13, FieldWireKind::Bytes),
    (14, FieldWireKind::Bytes),
    (15, FieldWireKind::Bytes), // additive streaming patch metadata (`E_0.1+`)
];
assert_tlv_schema_evolution!(EPOCH_BUNDLE_OUTER_SCHEMA_V1, EPOCH_BUNDLE_OUTER_SCHEMA_V2);

/// Self-describing canonical epoch bundle with `H(E_id) = BLAKE3(E_id)` (`게임.md §9.2`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEpochBundle {
    pub pinned_handle: EpochHandle<Pinned>,
    pub full_merkle_root: [u8; 32],
    pub qvm_bytecode_blake3: [u8; 32],
    pub spirv_ir_blake3: [u8; 32],
    pub chunk_tree_blake3: [u8; 32],
    pub micro_revision: u64,
    pub boss_tuning_permille: u64,
    pub preserved_unknown_outer: Vec<TlvField>,
    pub preserved_unknown_inner: Vec<TlvField>,
}

impl CanonicalEpochBundle {
    pub fn seal_new(
        mut summary: EpochManifestSummary,
        qvm_bytecode_blake3: [u8; 32],
        spirv_ir_blake3: [u8; 32],
        chunk_tree_blake3: [u8; 32],
        micro_revision: u64,
        boss_tuning_permille: u64,
    ) -> Self {
        let pinned_handle = EpochHandle::<Staging>::new_staging(summary).seal_and_pin();
        summary = *pinned_handle.manifest();
        let full_merkle_root = Self::compute_canonical_merkle(
            &summary,
            &qvm_bytecode_blake3,
            &spirv_ir_blake3,
            &chunk_tree_blake3,
            micro_revision,
            boss_tuning_permille,
        );
        Self {
            pinned_handle,
            full_merkle_root,
            qvm_bytecode_blake3,
            spirv_ir_blake3,
            chunk_tree_blake3,
            micro_revision,
            boss_tuning_permille,
            preserved_unknown_outer: Vec::new(),
            preserved_unknown_inner: Vec::new(),
        }
    }

    pub fn compute_canonical_merkle(
        summary: &EpochManifestSummary,
        qvm_hash: &[u8; 32],
        spirv_hash: &[u8; 32],
        chunk_hash: &[u8; 32],
        micro_revision: u64,
        boss_tuning_permille: u64,
    ) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&summary.compute_merkle_root());
        h.update(qvm_hash);
        h.update(spirv_hash);
        h.update(chunk_hash);
        h.update(&micro_revision.to_le_bytes());
        h.update(&boss_tuning_permille.to_le_bytes());
        *h.finalize().as_bytes()
    }

    pub fn encode_stream_tlv_v2(&self, patch_note: &[u8]) -> Result<Vec<u8>, NetError> {
        let m = self.pinned_handle.manifest();
        let packed_synth = (u64::from(m.synth_clear_permille))
            | (u64::from(m.synth_exit_mean_permille) << 16)
            | (u64::from(m.synth_exit_std_permille) << 32);
        let hdr_v2 = EpochHeaderRecordV2 {
            epoch_id: m.epoch_id,
            season_id: u64::from(m.season_id),
            base_move_speed_raw: m.base_move_speed_q32.raw() as u64,
            expected_farm_ticks: m.expected_farm_ticks,
            synth_stats_packed: packed_synth,
            hot_balance_micro_revision: self.micro_revision,
            boss_enrage_tuning_permille: self.boss_tuning_permille,
            unknown_fields: self.preserved_unknown_inner.clone(),
        };
        let hdr_bytes = hdr_v2.encode_tlv_bytes()?;
        let mut caps_bytes = Vec::with_capacity(64);
        for c in &m.faucet_caps {
            caps_bytes.extend_from_slice(&c.to_le_bytes());
        }
        let mut outer = vec![
            TlvField {
                tag: 10,
                value: hdr_bytes,
            },
            TlvField {
                tag: 11,
                value: caps_bytes,
            },
            TlvField {
                tag: 12,
                value: self.qvm_bytecode_blake3.to_vec(),
            },
            TlvField {
                tag: 13,
                value: self.spirv_ir_blake3.to_vec(),
            },
            TlvField {
                tag: 14,
                value: self.chunk_tree_blake3.to_vec(),
            },
            TlvField {
                tag: 15,
                value: patch_note.to_vec(),
            },
        ];
        for unk in &self.preserved_unknown_outer {
            outer.push(unk.clone());
        }
        let tlv = encode_tlv_stream(&outer)?;
        let mut frame = Vec::with_capacity(tlv.len().saturating_add(1));
        frame.push(STREAM_KIND_EPOCH_MANIFEST);
        frame.extend_from_slice(&tlv);
        Ok(frame)
    }

    /// Decodes a streamed epoch manifest using either `V1` (preserving future `V2` tags in `unknown_fields`!)
    /// or `V2` (recognizing tag `15` and defaulting missing `V2` tags when reading an older `E_0` manifest!).
    pub fn decode_stream_tlv(frame: &[u8], use_legacy_v1_decoder: bool) -> Result<Self, NetError> {
        if frame.first().copied() != Some(STREAM_KIND_EPOCH_MANIFEST) {
            return Err(NetError::MalformedWirePacket("Expected STREAM_KIND_EPOCH_MANIFEST"));
        }
        let body = frame
            .get(1..)
            .ok_or(NetError::MalformedWirePacket("Empty epoch TLV"))?;
        let fields = decode_tlv_stream(body)?;
        let mut hdr_raw: Option<Vec<u8>> = None;
        let mut caps = [0_i64; 8];
        let mut qvm_h = [0u8; 32];
        let mut spv_h = [0u8; 32];
        let mut chk_h = [0u8; 32];
        let mut unknown_outer = Vec::new();

        for f in fields {
            match f.tag {
                10 => hdr_raw = Some(f.value),
                11 if f.value.len() == 64 => {
                    let mut i: usize = 0;
                    while i < 8 {
                        let off = i.saturating_mul(8);
                        if let (Some(src), Some(dst)) = (f.value.get(off..off.saturating_add(8)), caps.get_mut(i)) {
                            let mut b = [0u8; 8];
                            b.copy_from_slice(src);
                            *dst = i64::from_le_bytes(b);
                        }
                        i = i.saturating_add(1);
                    }
                }
                12 if f.value.len() == 32 => qvm_h.copy_from_slice(&f.value),
                13 if f.value.len() == 32 => spv_h.copy_from_slice(&f.value),
                14 if f.value.len() == 32 => chk_h.copy_from_slice(&f.value),
                15 if !use_legacy_v1_decoder => {} // Known V2 additive streaming patch metadata tag (`EPOCH_BUNDLE_OUTER_SCHEMA_V2`)
                _ => unknown_outer.push(f),
            }
        }
        let hdr_bytes = hdr_raw.ok_or(NetError::MalformedWirePacket("Missing tag 10"))?;

        let (epoch_id, season_id, speed_raw, farm_ticks, synth_packed, micro_rev, boss_tune, unk_in) =
            if use_legacy_v1_decoder {
                let v1 = EpochHeaderRecordV1::decode_tlv_bytes(&hdr_bytes)?;
                (
                    v1.epoch_id,
                    v1.season_id as u32,
                    v1.base_move_speed_raw as i64,
                    v1.expected_farm_ticks,
                    v1.synth_stats_packed,
                    0_u64,
                    1_000_u64,
                    v1.unknown_fields,
                )
            } else {
                let v2 = EpochHeaderRecordV2::decode_tlv_bytes(&hdr_bytes)?;
                (
                    v2.epoch_id,
                    v2.season_id as u32,
                    v2.base_move_speed_raw as i64,
                    v2.expected_farm_ticks,
                    v2.synth_stats_packed,
                    v2.hot_balance_micro_revision,
                    v2.boss_enrage_tuning_permille,
                    v2.unknown_fields,
                )
            };

        let summary = EpochManifestSummary {
            epoch_id,
            season_id,
            merkle_root: [0u8; 32],
            faucet_caps: caps,
            base_move_speed_q32: Q32::from_raw(speed_raw),
            expected_farm_ticks: farm_ticks,
            synth_clear_permille: (synth_packed & 0xffff) as u16,
            synth_exit_mean_permille: ((synth_packed >> 16) & 0xffff) as u16,
            synth_exit_std_permille: ((synth_packed >> 32) & 0xffff) as u16,
        };
        let mut bundle = Self::seal_new(summary, qvm_h, spv_h, chk_h, micro_rev, boss_tune);
        bundle.preserved_unknown_outer = unknown_outer;
        bundle.preserved_unknown_inner = unk_in;
        Ok(bundle)
    }
}

pub struct MountedEpochSlot {
    pub bundle: CanonicalEpochBundle,
    active_encounter_pins: AtomicUsize,
}

impl MountedEpochSlot {
    pub fn new(bundle: CanonicalEpochBundle) -> Self {
        Self {
            bundle,
            active_encounter_pins: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn active_pins(&self) -> usize {
        self.active_encounter_pins.load(Ordering::Acquire)
    }
}

pub struct DualEpochSlots {
    pub primary: Arc<MountedEpochSlot>,
    pub draining_standby: Option<Arc<MountedEpochSlot>>,
}

/// Encounter-pinned RAII lease guaranteeing `0` mid-combat epoch drift (`게임.md §5.3, §9.2`).
pub struct EncounterEpochLease {
    slot: Arc<MountedEpochSlot>,
    pinned_copy: EpochHandle<Pinned>,
}

impl EncounterEpochLease {
    #[inline]
    pub fn pinned(&self) -> EpochHandle<Pinned> {
        self.pinned_copy
    }

    #[inline]
    pub fn bundle(&self) -> &CanonicalEpochBundle {
        &self.slot.bundle
    }

    /// Executes atomic draining transition ONLY at an encounter/gate/portal boundary (`§5.3`).
    pub fn drain_at_boundary(&mut self, table: &DualSlotEpochTable) {
        let next_slot = Arc::clone(&table.slots.load().primary);
        if !Arc::ptr_eq(&self.slot, &next_slot) {
            next_slot
                .active_encounter_pins
                .fetch_add(1, Ordering::AcqRel);
            let updated_handle = self
                .pinned_copy
                .drain_at_encounter_boundary(next_slot.bundle.pinned_handle);
            self.slot
                .active_encounter_pins
                .fetch_sub(1, Ordering::AcqRel);
            self.slot = next_slot;
            self.pinned_copy = updated_handle;
        }
    }
}

impl Drop for EncounterEpochLease {
    fn drop(&mut self) {
        self.slot
            .active_encounter_pins
            .fetch_sub(1, Ordering::AcqRel);
    }
}

/// Reference-counted `(E_a, E_b)` `ArcSwap` dual-slot epoch table (`게임.md §9.2`).
pub struct DualSlotEpochTable {
    slots: ArcSwap<DualEpochSlots>,
    reclaimed_epochs: Mutex<Vec<u64>>,
}

impl DualSlotEpochTable {
    pub fn new(initial_e0: CanonicalEpochBundle) -> Self {
        let primary = Arc::new(MountedEpochSlot::new(initial_e0));
        Self {
            slots: ArcSwap::from_pointee(DualEpochSlots {
                primary,
                draining_standby: None,
            }),
            reclaimed_epochs: Mutex::new(Vec::new()),
        }
    }

    /// Pins the currently active epoch for a newly entered combat encounter (`§5.3`).
    pub fn pin_for_new_encounter(&self) -> EncounterEpochLease {
        let guard = self.slots.load();
        let slot = Arc::clone(&guard.primary);
        slot.active_encounter_pins.fetch_add(1, Ordering::AcqRel);
        let pinned_copy = slot.bundle.pinned_handle;
        EncounterEpochLease { slot, pinned_copy }
    }

    pub fn active_bundle(&self) -> CanonicalEpochBundle {
        self.slots.load().primary.bundle.clone()
    }

    pub fn standby_pin_count(&self) -> Option<usize> {
        self.slots
            .load()
            .draining_standby
            .as_ref()
            .map(|s| s.active_pins())
    }

    /// Atomically mounts a background-staged epoch into `(E_new, E_draining)`.
    /// Strictly refuses overwriting an occupied `draining_standby` (`active_pins() > 0`) with `Err(NetError::EpochStandbyBusy)`!
    pub fn mount_staged_epoch(&self, staged_bundle: CanonicalEpochBundle) -> Result<(), NetError> {
        let prev = self.slots.load_full();
        if let Some(ref sb) = prev.draining_standby {
            if sb.active_pins() > 0 {
                return Err(NetError::EpochStandbyBusy);
            }
            let retired_id = sb.bundle.pinned_handle.manifest().epoch_id;
            if let Ok(mut g) = self.reclaimed_epochs.lock() {
                g.push(retired_id);
            }
        }
        let next_primary = Arc::new(MountedEpochSlot::new(staged_bundle));
        self.slots.store(Arc::new(DualEpochSlots {
            primary: next_primary,
            draining_standby: Some(Arc::clone(&prev.primary)),
        }));
        Ok(())
    }

    /// Reclaims the standby slot ONLY once its active encounter reference count reaches `0` (`§9.2`).
    pub fn try_reclaim_zero_ref_standby(&self) -> Option<u64> {
        let cur = self.slots.load_full();
        let standby = cur.draining_standby.as_ref()?;
        if standby.active_pins() != 0 {
            return None; // Refuses premature reclamation while any encounter is still pinned!
        }
        let retired_id = standby.bundle.pinned_handle.manifest().epoch_id;
        self.slots.store(Arc::new(DualEpochSlots {
            primary: Arc::clone(&cur.primary),
            draining_standby: None,
        }));
        if let Ok(mut g) = self.reclaimed_epochs.lock() {
            g.push(retired_id);
        }
        Some(retired_id)
    }

    pub fn reclaimed_history(&self) -> Vec<u64> {
        self.reclaimed_epochs
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

/// Zero-loading Open-World <-> Raid-Instance seamless portal (`게임.md §5.2, §11.3`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneInstanceKind {
    OpenWorldField,
    RaidBossInstance,
}

#[derive(Clone, Copy, Debug)]
pub struct SeamlessPortalGate {
    pub portal_id: u32,
    pub from_zone: ZoneInstanceKind,
    pub to_zone: ZoneInstanceKind,
    pub entry_center: QVec3,
    pub exit_anchor: QVec3,
    pub prewarm_radius: Q32,
    pub trigger_radius: Q32,
    pub dest_chunk: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalStepOutcome {
    Outside,
    PrewarmingDestChunk((i32, i32)),
    SeamlessTransferred {
        new_zone: ZoneInstanceKind,
        preserved_speed_raw: i64,
    },
}

impl SeamlessPortalGate {
    /// Evaluates proximity pre-warming and 0-tick momentum-preserving seamless transfer (`0` loading screens).
    pub fn step_entity_portal(
        &self,
        current_zone: &mut ZoneInstanceKind,
        arena: &mut CombatArena,
        entity: EntityId,
    ) -> PortalStepOutcome {
        if *current_zone != self.from_zone || !arena.is_valid(entity) {
            return PortalStepOutcome::Outside;
        }
        let idx = entity.0 as usize;
        let pos = QVec3::new(arena.pos_x[idx], arena.pos_y[idx], arena.pos_z[idx]);
        let vel = QVec3::new(arena.vel_x[idx], arena.vel_y[idx], arena.vel_z[idx]);
        let dist = pos.sat_sub(self.entry_center).length();

        if dist <= self.trigger_radius {
            let speed_before = vel.length().raw();
            if self.exit_anchor != self.entry_center {
                let rel_offset = pos.sat_sub(self.entry_center);
                let new_pos = self.exit_anchor.sat_add(rel_offset);
                arena.pos_x[idx] = new_pos.x;
                arena.pos_y[idx] = new_pos.y;
                arena.pos_z[idx] = new_pos.z;
            }
            // Momentum vector (`vel_x, vel_y, vel_z`), `mobility`, and `active_weapon` are preserved 1:1!
            *current_zone = self.to_zone;
            return PortalStepOutcome::SeamlessTransferred {
                new_zone: self.to_zone,
                preserved_speed_raw: speed_before,
            };
        }
        if dist <= self.prewarm_radius {
            return PortalStepOutcome::PrewarmingDestChunk(self.dest_chunk);
        }
        PortalStepOutcome::Outside
    }
}

#[derive(Clone, Debug)]
pub enum ClientInboundEvent {
    Datagram(Vec<u8>),
    UniStream(Vec<u8>),
}

/// Non-blocking `K_client` (`client_bin`) bridge packing the `8`-tick redundant input window,
/// polling live QUIC `ServerDeltaDatagram` / `SdfChunkDiffStreamPacket` / `CanonicalEpochBundle` frames,
/// tracking seamless portal transitions, and mounting `§9.2` `(E_a, E_b)` dual-slot epoch updates
/// with `0ms` main-thread stall and ZERO server `HMAC` secret in client memory (`게임.md §8.2, §9.1, §11.3`).
pub struct ClientNetBridge {
    pub account_id: u64,
    pub current_zone: ZoneInstanceKind,
    pub portal_gate: SeamlessPortalGate,
    pub dual_epochs: DualSlotEpochTable,
    pub encounter_lease: EncounterEpochLease,
    recent_wire_window: VecDeque<RawWireInput>,
    ack_server_tick: u64,
    ack_bitfield: u64,
    partial_delta_chunks: BTreeMap<u64, (u8, u8)>,
    active_ticket: HmacSessionTicket,
    outbound_dg_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    inbound_rx: Option<mpsc::UnboundedReceiver<ClientInboundEvent>>,
    pub prewarmed_chunks: u32,
    pub seamless_portal_transfers: u32,
    pub packed_datagrams_ready: u64,
    pub received_server_deltas: u64,
    pub received_sdf_bricks: u64,
    pub received_epoch_bundles: u32,
}

impl ClientNetBridge {
    pub fn new_local_linked(
        account_id: u64,
        player: EntityId,
        initial_epoch: EpochHandle<Pinned>,
    ) -> Result<Self, NetError> {
        let summary = *initial_epoch.manifest();
        let bundle = CanonicalEpochBundle::seal_new(
            summary,
            [0x11; 32],
            [0x22; 32],
            [0x33; 32],
            0,
            1_000,
        );
        let dual_epochs = DualSlotEpochTable::new(bundle);
        let encounter_lease = dual_epochs.pin_for_new_encounter();
        let active_ticket = HmacSessionTicket {
            account_id,
            entity_slot: player.0,
            entity_gen: player.1,
            issued_tick: 0,
            expiry_tick: SESSION_TICKET_MAX_RECOVERY_TICKS,
            reconnect_seq: 1,
            mac_tag: [0u8; 32], // Opaque token populated by server grant (`0` HMAC key on client!)
        };

        let portal_gate = SeamlessPortalGate {
            portal_id: 1,
            from_zone: ZoneInstanceKind::OpenWorldField,
            to_zone: ZoneInstanceKind::RaidBossInstance,
            entry_center: QVec3::from_ints(32, 14, 12),
            exit_anchor: QVec3::from_ints(32, 14, 12),
            prewarm_radius: Q32::from_int(22),
            trigger_radius: Q32::from_int(12),
            dest_chunk: (1, 0),
        };

        Ok(Self {
            account_id,
            current_zone: ZoneInstanceKind::OpenWorldField,
            portal_gate,
            dual_epochs,
            encounter_lease,
            recent_wire_window: VecDeque::with_capacity(SLIDING_WINDOW_TICKS),
            ack_server_tick: 0,
            ack_bitfield: 0,
            partial_delta_chunks: BTreeMap::new(),
            active_ticket,
            outbound_dg_tx: None,
            inbound_rx: None,
            prewarmed_chunks: 0,
            seamless_portal_transfers: 0,
            packed_datagrams_ready: 0,
            received_server_deltas: 0,
            received_sdf_bricks: 0,
            received_epoch_bundles: 0,
        })
    }

    /// Attaches a live `QuicClientEndpoint` via lock-free `mpsc` channels so the `120Hz` render/physics thread
    /// never blocks on async UDP I/O (`0ms` main-thread stall).
    pub fn attach_live_quic_endpoint(
        &mut self,
        endpoint: Arc<QuicClientEndpoint>,
        initial_server_ticket: HmacSessionTicket,
        rt_handle: &tokio::runtime::Handle,
    ) {
        self.active_ticket = initial_server_ticket;
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (in_tx, in_rx) = mpsc::unbounded_channel::<ClientInboundEvent>();
        self.outbound_dg_tx = Some(out_tx);
        self.inbound_rx = Some(in_rx);

        let ep_tx = Arc::clone(&endpoint);
        rt_handle.spawn(async move {
            while let Some(wire) = out_rx.recv().await {
                if ep_tx.is_closed() {
                    break;
                }
                let _ = ep_tx.send_datagram(&wire).await;
            }
        });

        let ep_rx = endpoint;
        rt_handle.spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(2));
            loop {
                let _ = ticker.tick().await;
                if ep_rx.is_closed() || in_tx.is_closed() {
                    break;
                }
                for dg in ep_rx.try_recv_datagrams().await {
                    if in_tx.send(ClientInboundEvent::Datagram(dg)).is_err() {
                        return;
                    }
                }
                for (_sid, sbytes) in ep_rx.try_recv_uni_streams().await {
                    if in_tx.send(ClientInboundEvent::UniStream(sbytes)).is_err() {
                        return;
                    }
                }
            }
        });
    }

    /// Non-blocking inbound poll (`< 1 µs` when idle/offline): applies `ServerDeltaDatagram`, reliable
    /// per-chunk `SdfChunkDiffStreamPacket`, streamed `CanonicalEpochBundle`, and `HmacSessionTicket` grants.
    pub fn poll_inbound_network(
        &mut self,
        arena: &mut CombatArena,
        world: &mut SparseSdfWorld,
    ) -> Result<usize, NetError> {
        let Some(rx) = self.inbound_rx.as_mut() else {
            return Ok(0);
        };
        let mut processed: usize = 0;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                ClientInboundEvent::Datagram(bytes) => {
                    let parsed = ServerDeltaDatagram::decode(&bytes)?;
                    record_received_delta_chunk(
                        &mut self.ack_server_tick,
                        &mut self.ack_bitfield,
                        &mut self.partial_delta_chunks,
                        parsed.server_tick,
                        parsed.chunk_index,
                        parsed.chunk_total,
                    );
                    self.active_ticket = parsed.refreshed_ticket;
                    for d in &parsed.deltas {
                        if !d.is_ghost && arena.is_valid(d.entity) {
                            let idx = d.entity.0 as usize;
                            arena.pos_x[idx] = Q32::from_raw(d.pos_x_raw);
                            arena.pos_y[idx] = Q32::from_raw(d.pos_y_raw);
                            arena.pos_z[idx] = Q32::from_raw(d.pos_z_raw);
                            arena.mobility[idx] = d.mobility;
                            arena.active_weapon[idx] = d.active_weapon;
                            arena.hp[idx] = Q32::from_ratio_i32(d.hp_milli, 1_000);
                            arena.shield[idx] = Q32::from_ratio_i32(d.shield_milli, 1_000);
                        }
                    }
                    self.received_server_deltas = self
                        .received_server_deltas
                        .saturating_add(parsed.deltas.len() as u64);
                }
                ClientInboundEvent::UniStream(stream_bytes) => {
                    match stream_bytes.first().copied() {
                        Some(STREAM_KIND_SDF_CHUNK_DIFF) => {
                            let pkt = SdfChunkDiffStreamPacket::decode_stream_frame(&stream_bytes)?;
                            let count = pkt.apply_to_world(world)?;
                            self.received_sdf_bricks =
                                self.received_sdf_bricks.saturating_add(count as u64);
                        }
                        Some(STREAM_KIND_EPOCH_MANIFEST) => {
                            let staged =
                                CanonicalEpochBundle::decode_stream_tlv(&stream_bytes, false)?;
                            let _ = self.dual_epochs.try_reclaim_zero_ref_standby();
                            self.dual_epochs.mount_staged_epoch(staged)?;
                            self.received_epoch_bundles =
                                self.received_epoch_bundles.saturating_add(1);
                        }
                        Some(STREAM_KIND_SESSION_TICKET_GRANT) => {
                            let body = stream_bytes.get(1..).ok_or(
                                NetError::MalformedWirePacket("Missing granted session ticket"),
                            )?;
                            self.active_ticket = HmacSessionTicket::decode(body)?;
                        }
                        _ => {}
                    }
                }
            }
            processed = processed.saturating_add(1);
        }
        Ok(processed)
    }

    /// Invoked once per `60Hz` server tick inside `K_client`: packs the `8`-tick redundant `ClientInputDatagram`
    /// (`<= 1,100B`), enqueues it non-blockingly when a live QUIC endpoint is attached, and steps zero-loading portal / boundary drain.
    pub fn on_client_server_tick(
        &mut self,
        _server_tick: u64,
        raw_input: RawWireInput,
        arena: &mut CombatArena,
        player: EntityId,
    ) -> Result<Vec<u8>, NetError> {
        if self.recent_wire_window.len() == SLIDING_WINDOW_TICKS {
            let _ = self.recent_wire_window.pop_back();
        }
        self.recent_wire_window.push_front(raw_input);

        let mut arr = [RawWireInput::default(); SLIDING_WINDOW_TICKS];
        let win_len = self.recent_wire_window.len();
        for (i, item) in self.recent_wire_window.iter().enumerate() {
            if let Some(dst) = arr.get_mut(i) {
                *dst = *item;
            }
        }
        let dgram = ClientInputDatagram::new(
            self.account_id,
            self.ack_server_tick,
            self.ack_bitfield,
            win_len as u8,
            arr,
            self.active_ticket,
        );
        let encoded = dgram.encode()?;
        if let Some(tx) = &self.outbound_dg_tx {
            let _ = tx.send(encoded.clone());
        }
        self.packed_datagrams_ready = self.packed_datagrams_ready.saturating_add(1);

        match self
            .portal_gate
            .step_entity_portal(&mut self.current_zone, arena, player)
        {
            PortalStepOutcome::PrewarmingDestChunk(_) => {
                self.prewarmed_chunks = self.prewarmed_chunks.saturating_add(1);
            }
            PortalStepOutcome::SeamlessTransferred { .. } => {
                self.seamless_portal_transfers = self.seamless_portal_transfers.saturating_add(1);
                self.encounter_lease.drain_at_boundary(&self.dual_epochs);
            }
            PortalStepOutcome::Outside => {}
        }

        Ok(encoded)
    }
}

// ============================================================================
// Unit Tests (`net_quic`)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use verifier_abi::actions;

    #[test]
    fn test_live_quic_udp_handshake_datagram_and_reliable_stream_under_loss() {
        let rt_res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build();
        assert!(rt_res.is_ok());
        let Ok(rt) = rt_res else {
            return;
        };

        rt.block_on(async {
            let cfg_res = build_pinned_quic_tls_configs([0x37; 32]);
            assert_eq!(cfg_res.as_ref().err(), None);
            let Ok((srv_cfg, cli_cfg)) = cfg_res else {
                return;
            };
            let bind_addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], 0));
            let imp = UdpLinkImpairment::phase2_raid_impairment();

            let server = QuicServerEndpoint::bind(bind_addr, srv_cfg, 1001, imp).await;
            assert_eq!(server.as_ref().err(), None);
            let Ok(server) = server else {
                return;
            };

            let client =
                QuicClientEndpoint::connect(server.local_addr(), cli_cfg, 2002, imp).await;
            assert_eq!(client.as_ref().err(), None);
            let Ok(client) = client else {
                return;
            };

            // Send 30 datagrams (`<= 1,100B`) + 1 multi-KB reliable unidirectional stream under 5% loss + 80ms RTT:
            for i in 0_u32..30 {
                let res = client.send_datagram(&i.to_le_bytes()).await;
                assert!(res.is_ok());
            }
            let large_sdf_blob = vec![0xAB_u8; 3_200];
            let sid_res = client.send_uni_stream(&large_sdf_blob).await;
            assert!(sid_res.is_ok());

            // Reject > 1,100B datagram strictly at send boundary:
            let oversize = vec![0u8; 1_101];
            assert_eq!(
                client.send_datagram(&oversize).await,
                Err(NetError::DatagramExceeds1100Bytes(1_101))
            );

            let mut got_stream = false;
            let mut got_dgrams = 0usize;
            for _ in 0..80 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let (dgs, streams) = server.drain_all_inbound().await;
                got_dgrams = got_dgrams.saturating_add(dgs.len());
                for (_cid, _sid, body) in streams {
                    if body == large_sdf_blob {
                        got_stream = true;
                    }
                }
                if got_stream && got_dgrams >= 15 {
                    break;
                }
            }
            assert!(got_stream);
            assert!(got_dgrams >= 15);

            client.close();
            server.close();
        });
    }

    #[test]
    fn test_tlv_unknown_tag_preservation_and_dual_slot_pinning() {
        let s0 = EpochManifestSummary {
            epoch_id: 0,
            season_id: 1,
            merkle_root: [0; 32],
            faucet_caps: [1_000_000; 8],
            base_move_speed_q32: Q32::from_int(10),
            expected_farm_ticks: 600,
            synth_clear_permille: 500,
            synth_exit_mean_permille: 20,
            synth_exit_std_permille: 10,
        };
        let s01 = EpochManifestSummary {
            epoch_id: 1,
            base_move_speed_q32: Q32::from_ratio_i32(10_200, 1_000),
            ..s0
        };
        let b0 = CanonicalEpochBundle::seal_new(s0, [1; 32], [2; 32], [3; 32], 0, 1_000);
        let b01 = CanonicalEpochBundle::seal_new(s01, [4; 32], [2; 32], [3; 32], 7, 1_050);

        // Encode `E_0.1` with V2 tags (`6`, `7`, `15`), decode via legacy V1 (`E_0`), and verify V1 preserves unknown tags while V2 recognizes tag 15!
        let wire_v2 = b01.encode_stream_tlv_v2(b"E_0.1_micro_patch");
        assert!(wire_v2.is_ok());
        if let Ok(bytes) = wire_v2 {
            let decoded_by_v1 = CanonicalEpochBundle::decode_stream_tlv(&bytes, true);
            assert!(decoded_by_v1.is_ok());
            if let Ok(v1_view) = decoded_by_v1 {
                assert!(!v1_view.preserved_unknown_inner.is_empty());
                assert!(!v1_view.preserved_unknown_outer.is_empty());
            }
            let decoded_by_v2 = CanonicalEpochBundle::decode_stream_tlv(&bytes, false);
            assert!(decoded_by_v2.is_ok());
            if let Ok(v2_view) = decoded_by_v2 {
                assert!(v2_view.preserved_unknown_outer.is_empty());
                assert_eq!(v2_view.full_merkle_root, b01.full_merkle_root);
            }
        }

        // Verify encounter pinning, busy-standby protection (`EpochStandbyBusy`), & zero-ref async standby reclamation:
        let table = DualSlotEpochTable::new(b0.clone());
        let mut raid_lease = table.pin_for_new_encounter();
        assert_eq!(raid_lease.pinned().manifest().epoch_id, 0);

        // Hot-swap to `E_0.1` mid-combat:
        assert_eq!(table.mount_staged_epoch(b01), Ok(()));
        // Active raid MUST stay pinned to `E_0`, standby `E_0` MUST NOT be reclaimed, and a 3rd mount while `E_0` is busy MUST fail!
        assert_eq!(raid_lease.pinned().manifest().epoch_id, 0);
        assert_eq!(table.try_reclaim_zero_ref_standby(), None);
        assert_eq!(
            table.mount_staged_epoch(b0.clone()),
            Err(NetError::EpochStandbyBusy)
        );

        // Encounter boundary drain -> switches to `E_0.1`, drops `E_0` pin to 0, standby `E_0` reclaims!
        raid_lease.drain_at_boundary(&table);
        assert_eq!(raid_lease.pinned().manifest().epoch_id, 1);
        assert_eq!(table.try_reclaim_zero_ref_standby(), Some(0));
    }

    #[test]
    fn test_aoi_diagonal_ghost_and_packet_typestate_reject() {
        let mut arena = CombatArena::new_boxed(42);
        let p = arena.spawn(
            EntityKind::Player,
            QVec3::from_ints(31, 10, 31), // Near `(0,0)` corner -> mirrors to `(1,0)`, `(0,1)`, AND `(1,1)`!
            Q32::from_int(1_000),
            Q32::from_int(200),
        );
        assert!(p.is_some());
        let mut grid = AoiSpatialGrid::new();
        grid.rebuild_from_arena(0, &arena);
        let diag_vis = grid.visible_in_shard(ShardCoord {
            channel_id: 0,
            sx: 1,
            sz: 1,
        });
        assert_eq!(diag_vis.len(), 1);
        assert!(diag_vis[0].1); // `is_ghost == true`

        let mgr_res = SessionTicketManager::new(&[0x99; 32]);
        assert!(mgr_res.is_ok());
        let Ok(mut mgr) = mgr_res else {
            return;
        };
        let t_res = mgr.issue_ticket(7, EntityId(0, 0), 5, 1);
        assert!(t_res.is_ok());
        let Ok(ticket) = t_res else {
            return;
        };
        let bad_raw = RawWireInput {
            tick: 5,
            move_dir_milli: [2_500, 0, 0], // Out of bounds (`> 1000`)!
            aim_yaw_turn_milli: 0,
            aim_pitch_turn_milli: 0,
            action_mask: u32::from(actions::PRIMARY_ATTACK),
            active_weapon_slot: 0,
        };
        let dgram = ClientInputDatagram::new(7, 0, 0, 1, [bad_raw; SLIDING_WINDOW_TICKS], ticket);
        let bytes = dgram.encode().unwrap_or_default();
        assert_eq!(
            ClientInputDatagram::decode_and_verify(&bytes, &mgr, 6),
            Err(NetError::PacketBoundaryRejected(
                PacketReject::MoveComponentOutOfBounds
            ))
        );

        // Verify `SessionTicketManager::accept_reconnect` revokes old `seq=1` ticket on live packets AND reconnects:
        let rec = mgr.accept_reconnect(&ticket, 6);
        assert!(rec.is_ok());
        assert_eq!(
            mgr.verify_live_packet_ticket(&ticket, 6),
            Err(NetError::SessionReplayDetected)
        );
        assert_eq!(
            mgr.accept_reconnect(&ticket, 6),
            Err(NetError::SessionReplayDetected)
        );
    }

    #[test]
    fn test_truncated_entity_delta_recovery_and_live_client_net_bridge() {
        // 1. Prove truncated stationary entity at tick 10 is NOT suppressed at tick 11 (`CE-4`):
        let mut arena = CombatArena::new_boxed(123);
        let mut vis = Vec::new();
        let mut self_ent = EntityId(0, 1);
        for i in 0..25_i32 {
            let pid = arena.spawn(
                EntityKind::Player,
                QVec3::from_ints(10_i32.saturating_add(i), 10, 10),
                Q32::from_int(1_000),
                Q32::from_int(100),
            );
            if let Some(e) = pid {
                if i == 0 {
                    self_ent = e;
                }
                vis.push((e.0, false));
            }
        }
        let mut ring = ServerSoARingBuffer64::new(123);
        // Move entity 24 at tick 10 (`pos.z = 15`), then keep it stationary at tick 11 (`pos.z = 15`):
        ring.record_state_at_tick(9, &arena, &[], &[]);
        arena.pos_z[24] = Q32::from_int(15);
        ring.record_state_at_tick(10, &arena, &[], &[]);
        ring.record_state_at_tick(11, &arena, &[], &[]);

        let mut sync = ClientEntitySyncState::default();
        // Pretend slots 0..25 were previously confirmed at tick 9 (`pos.z = 10`):
        for s in 0..25_u32 {
            sync.confirmed_entity_tick.insert(s, 9);
        }
        let p10 = PriorityDeltaParams {
            server_tick: 10,
            ack_client_tick: 10,
            client_ack_server_tick: 9,
            self_entity: self_ent,
            pinned_epoch_merkle: [0; 32],
            refreshed_ticket: HmacSessionTicket::default(),
            max_chunks_per_tick: 1, // Truncates at 22 entities; slot 24 is farthest so truncated!
        };
        // Also move slots 1..23 slightly so 23 entities compete at tick 10:
        let mut arena10 = CombatArena::new_boxed(123);
        arena10.clone_from(&arena);
        for s in 1..24_usize {
            arena10.pos_y[s] = Q32::from_int(11);
        }
        ring.record_state_at_tick(10, &arena10, &[], &[]);
        ring.record_state_at_tick(11, &arena10, &[], &[]);

        let chunks10 =
            build_ack_priority_delta_chunks(&p10, &vis, &arena10, &ring, &mut sync).unwrap_or_default();
        assert_eq!(chunks10.len(), 1);
        assert!(!chunks10[0].deltas.iter().any(|d| d.entity.0 == 24));
        assert_eq!(sync.starvation_age.get(&24).copied(), Some(1));

        // Client ACKs tick 10 (`ack_bitfield = 1`), slots 0..22 become confirmed at tick 10, while slot 24 stays at tick 9!
        sync.process_client_ack(10, 1);
        let p11 = PriorityDeltaParams {
            server_tick: 11,
            client_ack_server_tick: 10,
            ..p10
        };
        let chunks11 =
            build_ack_priority_delta_chunks(&p11, &vis, &arena10, &ring, &mut sync).unwrap_or_default();
        assert!(chunks11[0].deltas.iter().any(|d| d.entity.0 == 24));

        // 2. Prove `ClientNetBridge` live QUIC attachment & non-blocking `poll_inbound_network` (`CE-7`):
        let rt_res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build();
        let Ok(rt) = rt_res else {
            return;
        };
        rt.block_on(async {
            let Ok((srv_cfg, cli_cfg)) = build_pinned_quic_tls_configs([0x51; 32]) else {
                return;
            };
            let imp = UdpLinkImpairment::lossless_lan();
            let Ok(server) =
                QuicServerEndpoint::bind(SocketAddr::from(([127, 0, 0, 1], 0)), srv_cfg, 1, imp).await
            else {
                return;
            };
            let Ok(client) =
                QuicClientEndpoint::connect(server.local_addr(), cli_cfg, 2, imp).await
            else {
                return;
            };
            let client = Arc::new(client);
            let Ok(mgr) = SessionTicketManager::new(&[0xAB; 32]) else {
                return;
            };
            let Ok(init_tkt) = mgr.issue_ticket(1, self_ent, 0, 1) else {
                return;
            };

            let s0 = EpochManifestSummary {
                epoch_id: 0,
                season_id: 1,
                merkle_root: [0; 32],
                faucet_caps: [1_000; 8],
                base_move_speed_q32: Q32::from_int(10),
                expected_farm_ticks: 600,
                synth_clear_permille: 500,
                synth_exit_mean_permille: 20,
                synth_exit_std_permille: 10,
            };
            let pinned0 = EpochHandle::<Staging>::new_staging(s0).seal_and_pin();
            let Ok(mut bridge) = ClientNetBridge::new_local_linked(1, self_ent, pinned0) else {
                return;
            };
            bridge.attach_live_quic_endpoint(
                Arc::clone(&client),
                init_tkt,
                &tokio::runtime::Handle::current(),
            );

            let mut cli_arena = CombatArena::new_boxed(1);
            let _ = cli_arena.spawn(
                EntityKind::Player,
                QVec3::from_ints(5, 5, 5),
                Q32::from_int(100),
                Q32::from_int(50),
            );
            let mut cli_world = SparseSdfWorld::new();
            let raw = RawWireInput {
                tick: 1,
                move_dir_milli: [500, 0, 0],
                aim_yaw_turn_milli: 0,
                aim_pitch_turn_milli: 0,
                action_mask: 0,
                active_weapon_slot: 0,
            };
            assert!(bridge.on_client_server_tick(1, raw, &mut cli_arena, self_ent).is_ok());

            let mut cid_opt = None;
            for _ in 0..40 {
                tokio::time::sleep(Duration::from_millis(5)).await;
                let (dgs, _) = server.drain_all_inbound().await;
                if let Some((cid, payload)) = dgs.first() {
                    assert!(ClientInputDatagram::decode_and_verify(payload, &mgr, 1).is_ok());
                    cid_opt = Some(*cid);
                    break;
                }
            }
            assert!(cid_opt.is_some());
            if let Some(cid) = cid_opt {
                let Some(ed) = EntityDelta::from_arena(&arena10, 0, false) else {
                    return;
                };
                let sd = ServerDeltaDatagram {
                    server_tick: 2,
                    ack_client_tick: 1,
                    baseline_ack_tick: 0,
                    chunk_index: 0,
                    chunk_total: 1,
                    pinned_epoch_merkle: [0; 32],
                    refreshed_ticket: init_tkt,
                    deltas: vec![ed],
                };
                if let Ok(w) = sd.encode() {
                    let _ = server.send_datagram_to(&cid, &w).await;
                }
            }
            for _ in 0..40 {
                tokio::time::sleep(Duration::from_millis(5)).await;
                let _ = bridge.poll_inbound_network(&mut cli_arena, &mut cli_world);
                if bridge.received_server_deltas > 0 {
                    break;
                }
            }
            assert_eq!(bridge.received_server_deltas, 1);
            client.close();
            server.close();
        });
    }
}
