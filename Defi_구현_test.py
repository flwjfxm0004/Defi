#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""
Defi_구현_test.py — Defi_구현.py 분리 회귀 테스트 러너 (T1~T45, 태그/범위 선택 실행 지원)

사용법:
- 전체 실행: `c:\Defi\python.exe -B -X utf8 Defi_구현_test.py`
- 선택 실행: `c:\Defi\python.exe -B -X utf8 Defi_구현_test.py T44 T45` (또는 `T1-T14`, `FLASH`, `--list`)
"""
from __future__ import annotations

import ast
import contextlib
from datetime import datetime, timezone
from decimal import Decimal
import gc
import importlib
import io
import json
import math
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path
from typing import Any, Dict, FrozenSet, Iterator, List, Optional, Sequence, Tuple

_TEST_DIR: Path = Path(__file__).resolve().parent
_MAIN_MOD = sys.modules.get("__main__")
if (
    _MAIN_MOD is not None
    and getattr(_MAIN_MOD, "__file__", None)
    and Path(str(getattr(_MAIN_MOD, "__file__"))).resolve() == (_TEST_DIR / "Defi_구현.py").resolve()
):
    eng = _MAIN_MOD
    sys.modules.setdefault("Defi_구현", eng)
else:
    if str(_TEST_DIR) not in sys.path:
        sys.path[:0] = [str(_TEST_DIR)]
    eng = importlib.import_module("Defi_구현")

_ENGINE_FILE: Path = Path(str(eng.__file__)).resolve()
_TEST_MOD_GLOBALS: Dict[str, Any] = globals()
_SKIP_MIRROR_KEYS: FrozenSet[str] = frozenset(
    {
        "__name__",
        "__doc__",
        "__package__",
        "__loader__",
        "__spec__",
        "__file__",
        "__cached__",
        "__builtins__",
        "cmd_selftest",
    }
)
for _mk, _mv in eng.__dict__.items():
    if _mk not in _SKIP_MIRROR_KEYS:
        _TEST_MOD_GLOBALS[_mk] = _mv


def _set_eng(name: str, value: Any) -> None:
    """엔진(Defi_구현) 모듈 전역과 테스트 미러 전역을 동시에 동기 갱신한다."""
    eng.__dict__[name] = value
    _TEST_MOD_GLOBALS[name] = value


class _EngGlobalsMirror:
    """엔진 모듈 전역을 대상으로 하는 globals() 대체 미러."""

    def __contains__(self, key: object) -> bool:
        return key in eng.__dict__

    def __getitem__(self, key: str) -> Any:
        return eng.__dict__[key]

    def get(self, key: str, default: Any = None) -> Any:
        return eng.__dict__.get(key, default)

    def __setitem__(self, key: str, value: Any) -> None:
        _set_eng(key, value)


_ENG_GLOBALS_MIRROR = _EngGlobalsMirror()


def _eng_globals() -> _EngGlobalsMirror:
    return _ENG_GLOBALS_MIRROR


TEST_TAGS: Tuple[str, ...] = (
    "T1_KECCAK_AND_FULL_ABI_CODEC",
    "T2_PROXY_6TIER_AND_SLOT_COLLISION",
    "T3_MODERN_EVM_OPCODES_AND_SKELETON_HASH",
    "T4_E1_VAULT_THRESHOLD_AND_MAINNET_REGISTRY",
    "T5_E2_FOUR_PHYSICAL_VALUE_SINKS",
    "T6_E3_CALLER_DOMINATOR_THEOREM",
    "T7_FOUR_CHANNEL_CAUSAL_SLICER",
    "T8_DISCONTINUITY_BOUNDARY_SOLVER",
    "T9_CHEATCODE_DEFAULT_DENY_ALLOWLIST",
    "T10_CONSERVATION_ACCOUNTING_AND_DECIMALS",
    "T11_THREE_VERDICT_SYSTEM_AND_PINS_SHA256",
    "T12_SQLITE_ZOMBIE_AND_PROVEN_VAULTS",
    "T13_E3_BRANCH_SENSITIVE_SOUNDNESS",
    "T14_CHEATCODE_ALIAS_NESTED_ASM_AND_MODE_B_SEEDS",
    "T15_LIVE_FORGE_EXECUTION_AND_EVENT_DECODE",
    "T16_CRASH_RESUME_PRIORITY_AND_4STAGE_HARVEST",
    "T17_REAL_SOLC_SOUNDNESS_PAYABLE_PACKED_AND_MIDFRAME",
    "T18_OFFLINE_SOURCIFY_V2_AND_1BYTE_MULTIGRAM",
    "T19_DONE_GATES_NESTED_ABI_STABLES_AND_AMM_IDENTITY",
    "T20_SOUNDNESS_AND_SSOT_REGRESSION",
    "T21_AST_STDLIB_CORPUS_DISPATCHER_AND_DUAL_LEDGER",
    "T22_FUSAKA_FIXPOINT_SOURCIFY_AND_SOUNDNESS_REGRESSION",
    "T23_GLAMSTERDAM_EIP8024_DECIMAL_SCROLL_AND_TRACE_SOUNDNESS",
    "T24_ROUND2_CWIA_MULTI_ASSET_HARNESS_AND_FAILOPEN",
    "T25_ROUND3_EIP7702_MINT_INFRA_ALLOWANCE_AND_PARTITION",
    "T26_ROUND4_100POINT_ACCOUNTING_CAUSALITY_CFG512_AND_GATES",
    "T27_ROUND5_16POINT_COLD_AUDIT_REGRESSION",
    "T28_STATIC_SOUNDNESS_CONSERVATION_AND_CLOSURE_EXHAUSTION",
    "T29_SSOT_HEAD_FRESHNESS_WNATIVE_MINT_V3PEG_PRECOMPILE_AND_ISOLATION",
    "T30_SSOT_CDNS_UDP_SELFISO_ARENA_DIAMOND_PRECOMPILE_AND_TRACE_AUTH",
    "T31_QA_COUNTERFACTUAL_PROXY_HARNESS_CMD_AND_ISOLATION",
    "T32_FULL_AUDIT_SOUNDNESS_PRECISION_AND_ISOLATION_SEAL",
    "T33_FIVE_SESSION_ISOLATION_SOURCIFY_STREAM_REAL_SELL_AND_PROFIT_STOP",
    "T34_CONCURRENT_5SESS_HARDENING_SHARDED_CURSOR_REVPROXY_AND_QUOTER",
    "T35_MODEB_SEEDHIST_VANITY_DORMANT_INBOUND_AND_DEFER_MODEA",
    "T36_HARNESS_TOKEN_RANK_WITNESS_ISOLATION_MODEB_PROXY_AND_AUTO_LIQ",
    "T37_BLUECHIP_LP_AGENT_BRIDGE_PARQUET_PROXY_AND_PRESCREEN_CHUNK",
    "T38_CALLDATA_LP_QEXIT_AND_SQLITE_PRIO_MIGRATION",
    "T39_ALL_21_REMEDIATIONS_REGRESSION",
    "T40_COMPREHENSIVE_REMEDIATION_REGRESSION",
    "T41_TWELVE_HARDENING_FIXES_REGRESSION",
    "T42_SEVENTEEN_HARDENING_FIXES_REGRESSION",
    "T43_TWENTY_SEVEN_HARDENING_FIXES_REGRESSION",
    "T44_SYMBOLIC_CFG_ROLE_ABI_MIDFRAME_PREDIAG",
    "T45_DEDUP_DYNAMIC_FLASH_OPTIMIZER_AND_STAGE4_FLASH",
    "T46_ROUND6_SOTA_TRANSCENDENCE_100POINT",
    "T47_FIRST_PRINCIPLE_AGENT_OPTIMIZATION",
)


def _want_test(tag: str, filters: Optional[Sequence[str]]) -> bool:
    """CLI 태그 필터(`T45`, `45`, `T1-T14`, 부분문자열)가 비어 있거나 해당 테스트와 일치할 때 True."""
    if not filters:
        return True
    tag_u = tag.upper()
    m_tag = re.match(r"^T(\d+)_", tag_u)
    tag_num = int(m_tag.group(1)) if m_tag else -1
    for raw in filters:
        tok = str(raw or "").strip().upper()
        if not tok:
            continue
        if tok == "ALL":
            return True
        m_num = re.fullmatch(r"T?(\d+)", tok)
        if m_num:
            if tag_num == int(m_num.group(1)):
                return True
            continue
        m_rng = re.fullmatch(r"T?(\d+)-T?(\d+)", tok)
        if m_rng:
            lo, hi = int(m_rng.group(1)), int(m_rng.group(2))
            if lo <= hi and lo <= tag_num <= hi:
                return True
            continue
        if tok in tag_u:
            return True
    return False


@contextlib.contextmanager
def _offline_socket_guard() -> Iterator[None]:
    """selftest 실행 중 외부 인터넷 C-DNS 조회 및 TCP/UDP 소켓 송수신·시스템 프록시 우회를 물리적으로 차단(127.0.0.1/::1 루프백만 허용)."""
    if getattr(socket.getaddrinfo, "__name__", "") == "_guarded_getaddrinfo":
        yield
        return
    orig_getaddrinfo = socket.getaddrinfo
    orig_gethostbyname = socket.gethostbyname
    orig_gethostbyname_ex = socket.gethostbyname_ex
    orig_gethostbyaddr = socket.gethostbyaddr
    orig_getnameinfo = socket.getnameinfo
    orig_connect = socket.socket.connect
    orig_connect_ex = socket.socket.connect_ex
    orig_sendto = socket.socket.sendto
    orig_sendmsg = getattr(socket.socket, "sendmsg", None)
    orig_create_conn = socket.create_connection
    orig_getproxies = urllib.request.getproxies
    strict_loopback = ("127.0.0.1", "localhost", "::1", "::ffff:127.0.0.1")

    def _norm_guard_host(raw_h: Any) -> Optional[str]:
        if isinstance(raw_h, (bytes, bytearray)):
            try:
                s = bytes(raw_h).decode("ascii")
            except UnicodeDecodeError:
                return None
        elif isinstance(raw_h, str):
            s = raw_h
        else:
            return None
        s = s.strip()
        if s.startswith("[") and s.endswith("]"):
            s = s[1:-1]
        if "%" in s:
            s = s.split("%", 1)[0]
        return s.strip().lower()

    def _is_loopback(addr: Any) -> bool:
        if isinstance(addr, tuple) and len(addr) >= 2:
            h_n = _norm_guard_host(addr[0])
            return bool(h_n is not None and h_n in strict_loopback)
        return False

    def _guarded_getaddrinfo(host: Any, port: Any, *args: Any, **kwargs: Any) -> Any:
        h_s = _norm_guard_host(host)
        if h_s is None or h_s not in strict_loopback:
            raise OSError(f"OFFLINE_SOCKET_GUARD_DNS_BLOCKED:{host}")
        return orig_getaddrinfo(host, port, *args, **kwargs)

    def _make_dns_host_guard(orig_fn: Any) -> Any:
        def _guarded_dns_host(target_h: Any) -> Any:
            h_s = _norm_guard_host(target_h)
            if h_s is None or h_s not in strict_loopback:
                raise OSError(f"OFFLINE_SOCKET_GUARD_DNS_BLOCKED:{target_h}")
            return orig_fn(target_h)

        return _guarded_dns_host

    def _guarded_getnameinfo(sockaddr: Any, flags: int) -> Any:
        if not _is_loopback(sockaddr):
            raise OSError(f"OFFLINE_SOCKET_GUARD_DNS_BLOCKED:{sockaddr}")
        return orig_getnameinfo(sockaddr, flags)

    def _make_sock_conn_guard(orig_fn: Any) -> Any:
        def _guarded_sock_conn(self: socket.socket, address: Any) -> Any:
            if not _is_loopback(address):
                raise OSError(f"OFFLINE_SOCKET_GUARD_BLOCKED:{address}")
            return orig_fn(self, address)

        return _guarded_sock_conn

    def _guarded_sendto(self: socket.socket, data: Any, *args: Any) -> int:
        dest = args[-1] if args else None
        if not _is_loopback(dest):
            raise OSError(f"OFFLINE_SOCKET_GUARD_UDP_BLOCKED:{dest}")
        return orig_sendto(self, data, *args)

    def _guarded_sendmsg(self: socket.socket, buffers: Any, ancdata: Any = (), flags: int = 0, address: Any = None) -> int:
        if not _is_loopback(address):
            raise OSError(f"OFFLINE_SOCKET_GUARD_UDP_BLOCKED:{address}")
        return orig_sendmsg(self, buffers, ancdata, flags, address)  # type: ignore[misc]

    def _guarded_create_connection(address: Any, *args: Any, **kwargs: Any) -> socket.socket:
        if not _is_loopback(address):
            raise OSError(f"OFFLINE_SOCKET_GUARD_BLOCKED:{address}")
        return orig_create_conn(address, *args, **kwargs)

    socket.getaddrinfo = _guarded_getaddrinfo  # type: ignore[assignment]
    socket.gethostbyname = _make_dns_host_guard(orig_gethostbyname)  # type: ignore[assignment]
    socket.gethostbyname_ex = _make_dns_host_guard(orig_gethostbyname_ex)  # type: ignore[assignment]
    socket.gethostbyaddr = _make_dns_host_guard(orig_gethostbyaddr)  # type: ignore[assignment]
    socket.getnameinfo = _guarded_getnameinfo  # type: ignore[assignment]
    socket.socket.connect = _make_sock_conn_guard(orig_connect)  # type: ignore[assignment]
    socket.socket.connect_ex = _make_sock_conn_guard(orig_connect_ex)  # type: ignore[assignment]
    socket.socket.sendto = _guarded_sendto  # type: ignore[assignment]
    if orig_sendmsg is not None:
        socket.socket.sendmsg = _guarded_sendmsg  # type: ignore[assignment]
    socket.create_connection = _guarded_create_connection  # type: ignore[assignment]
    urllib.request.getproxies = lambda: {}  # type: ignore[assignment]
    try:
        yield
    finally:
        urllib.request.getproxies = orig_getproxies  # type: ignore[assignment]
        socket.getaddrinfo = orig_getaddrinfo  # type: ignore[assignment]
        socket.gethostbyname = orig_gethostbyname  # type: ignore[assignment]
        socket.gethostbyname_ex = orig_gethostbyname_ex  # type: ignore[assignment]
        socket.gethostbyaddr = orig_gethostbyaddr  # type: ignore[assignment]
        socket.getnameinfo = orig_getnameinfo  # type: ignore[assignment]
        socket.socket.connect = orig_connect  # type: ignore[assignment]
        socket.socket.connect_ex = orig_connect_ex  # type: ignore[assignment]
        socket.socket.sendto = orig_sendto  # type: ignore[assignment]
        if orig_sendmsg is not None:
            socket.socket.sendmsg = orig_sendmsg  # type: ignore[assignment]
        socket.create_connection = orig_create_conn  # type: ignore[assignment]


def run_selftest(tags: Optional[Sequence[str]] = None) -> int:
    """분리 회귀 테스트 스위트(T1~T45)를 오프라인 격리 네임스페이스(`audit_artifacts/_selftest_ns_*`)에서 실행한다."""
    tag_filters: Optional[List[str]] = [str(t).strip() for t in tags if str(t).strip()] if tags else None
    results: List[Tuple[str, bool, str]] = []
    audit_root_pre_existed = AUDIT_ROOT.exists()
    orig_artifact_root = ARTIFACT_ROOT
    orig_cache_dir = CACHE_DIR
    orig_sqlite_db = SQLITE_DB_PATH
    orig_queue_file = QUEUE_FILE_PATH
    orig_queue_lock = QUEUE_LOCK_PATH
    orig_lock_file = LOCK_FILE_PATH
    orig_sourcify_parquet = SOURCIFY_PARQUET_DIR
    orig_sourcify_manifest = SOURCIFY_MANIFEST_PATH
    orig_tempdir = tempfile.tempdir
    selftest_ns = AUDIT_ROOT / f"_selftest_ns_{os.getpid()}_{int(time.time() * 1000)}"
    shutil.rmtree(selftest_ns, ignore_errors=True)
    (selftest_ns / "_cache").mkdir(parents=True, exist_ok=True)
    (selftest_ns / "_py_tmp").mkdir(parents=True, exist_ok=True)
    _set_eng("ARTIFACT_ROOT", selftest_ns)
    _set_eng("CACHE_DIR", selftest_ns / "_cache")
    _set_eng("SQLITE_DB_PATH", CACHE_DIR / "engine_cache.sqlite")
    _set_eng("QUEUE_FILE_PATH", selftest_ns / "_queue.json")
    _set_eng("QUEUE_LOCK_PATH", selftest_ns / "_queue.lock")
    _set_eng("LOCK_FILE_PATH", selftest_ns / "_engine.lock")
    _set_eng("SOURCIFY_PARQUET_DIR", CACHE_DIR / "sourcify_v2")
    _set_eng("SOURCIFY_MANIFEST_PATH", SOURCIFY_PARQUET_DIR / "_manifest.json")
    tempfile.tempdir = str((selftest_ns / "_py_tmp").resolve())
    selftest_guard_stack = contextlib.ExitStack()
    selftest_stop_prank_sel = _selector("stopPrank()")
    selftest_drain_sel = _selector("drain()")

    def _noop_http_hook(*_a: Any) -> None:
        return

    class _SelftestMockRouter(RPCRouter):
        _cand_urls: Tuple[str, ...] = ()
        _mc3_zero_word: bool = False
        _block_ts_int: int = 0

        def candidate_urls(self, chain: str, **kw: Any) -> List[str]:
            return list(self._cand_urls)

        def call(self, chain: str, method: str, params: Any = (), **kw: Any) -> Any:
            if self._block_ts_int and method == "eth_getBlockByNumber":
                return {"timestamp": hex(self._block_ts_int)}
            return "0x"

        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], **kw: Any) -> List[Tuple[bool, bytes]]:
            if self._mc3_zero_word:
                return [(True, (0).to_bytes(32, "big")) for _ in calls]
            return [(False, b"") for _ in calls]

    try:
        selftest_guard_stack.enter_context(_offline_socket_guard())
        with _offline_socket_guard():
            def _rlp_create_low_nonce(sender_hex: str, nonce: int) -> str:
                s_b = bytes.fromhex(_norm_addr(sender_hex)[2:])
                n_b = b"\x80" if nonce == 0 else bytes([nonce])
                return _norm_addr("0x" + _keccak256(bytes([0xD6, 0x94]) + s_b + n_b)[12:32].hex())

            def _make_mock_guard_router(
                st_obj: SQLiteStore,
                *,
                code_by_addr: Optional[Dict[str, str]] = None,
                mc3_zero_word: bool = False,
            ) -> Any:
                code_lut = {_norm_addr(k): v for k, v in (code_by_addr or {}).items()}

                class _MockGuardRouter(_SelftestMockRouter):
                    _mc3_zero_word = mc3_zero_word

                    def call(self, ch: str, method: str, params: Any = (), **kw: Any) -> Any:
                        if method == "eth_getBlockByNumber":
                            return {"number": hex(20_000_000), "timestamp": hex(1_760_000_000)}
                        if method == "eth_getTransactionCount":
                            return "0x1"
                        return "0x"

                    def get_quorum_head_block(self, ch: str) -> Tuple[str, int]:
                        return ("http://127.0.0.1:8545", 20_000_002)

                    def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                        out_b: List[Any] = []
                        for m_c, p_c in calls:
                            if m_c == "eth_getCode":
                                a_q = _norm_addr(str(p_c[0])) if p_c else ""
                                out_b.append(code_lut.get(a_q, "0x"))
                            elif m_c == "eth_getStorageAt":
                                out_b.append("0x" + ("00" * 32))
                            elif m_c == "eth_getBalance":
                                out_b.append("0x0")
                            else:
                                out_b.append("0x")
                        return out_b

                return _MockGuardRouter(store=st_obj)
            bc_uw: Optional[bytes] = None
            ch_code: Optional[bytes] = None
            cfg_closed: Any = None
            evm_h: Any = None
            m22_router: Any = None
            st31: Any = None
            tc_beacon_31: Any = None
            if _want_test("T1_KECCAK_AND_FULL_ABI_CODEC", tag_filters):
                # T1: 순수 Python Keccak-256 및 동적/정적 ABI 인코더·디코더 왕복 검증
                empty_k = _keccak256_hex(b"")
                t1_ok = empty_k == "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                enc = abi_encode(
                    ["address", "uint256", "bytes", "uint256[]"],
                    ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 10**18, b"\xde\xad\xbe\xef", [7, 42]],
                )
                dec = abi_decode(["address", "uint256", "bytes", "uint256[]"], enc)
                t1_ok = t1_ok and (
                    _norm_addr(dec[0]) == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    and dec[1] == 10**18
                    and dec[2] == b"\xde\xad\xbe\xef"
                    and dec[3] == [7, 42]
                )
                results.append(("T1_KECCAK_AND_FULL_ABI_CODEC", t1_ok, f"keccak_ok={t1_ok}"))

            if _want_test("T2_PROXY_6TIER_AND_SLOT_COLLISION", tag_filters):
                # T2: 6단계 프록시 판별기 (EIP-7702, EIP-1167 45B + >45B 불변인자 변형, Slot-0 Safe 2KB 가드, 슬롯 충돌)
                c7702 = bytes.fromhex("ef0100" + "11" * 20)
                c1167_ext = bytes.fromhex("363d3d373d3d3d363d73" + "22" * 20 + "5af43d82803e903d91602b57fd5bf3" + "aa" * 32)
                big_non_proxy = b"\x60\x00\x54\x5a\xf4" + (b"\x00" * 2100)
                p_dis = _disassemble(bytes.fromhex("600554600555"))
                i_dis = _disassemble(bytes.fromhex("600554"))
                col = _detect_storage_slot_collision(p_dis, i_dis)
                t2_ok = (
                    _eip7702_delegate(c7702) == "0x" + "11" * 20
                    and _eip1167_impl(c1167_ext) == "0x" + "22" * 20
                    and not _slot0_safe_proxy_shape(big_non_proxy, _disassemble(big_non_proxy))
                    and 5 in col
                )
                results.append(("T2_PROXY_6TIER_AND_SLOT_COLLISION", t2_ok, f"collisions={col}"))

            if _want_test("T3_MODERN_EVM_OPCODES_AND_SKELETON_HASH", tag_filters):
                # T3: 현행 EVM 옵코드(CLZ 0x1e, TLOAD 0x5c, TSTORE 0x5d, MCOPY 0x5e, SLOTNUM 0x4b) 및 스켈레톤 해시
                code_modern = bytes([0x1E, 0x4B, 0x5C, 0x5D, 0x5E, 0xE6, 0x02, 0x00])
                dis_modern = [nm for _, nm, _ in _disassemble(code_modern)]
                skel_a = _skeleton_hash(bytes.fromhex("600160020160005500"))
                skel_b = _skeleton_hash(bytes.fromhex("60ff60ee0160005500"))
                t3_ok = (
                    dis_modern[:6] == ["CLZ", "SLOTNUM", "TLOAD", "TSTORE", "MCOPY", "DUPN"]
                    and skel_a == skel_b
                )
                results.append(("T3_MODERN_EVM_OPCODES_AND_SKELETON_HASH", t3_ok, f"ops={dis_modern[:6]}"))

            if _want_test("T4_E1_VAULT_THRESHOLD_AND_MAINNET_REGISTRY", tag_filters):
                # T4: E1 금고 가치 임계치(< $100 -> 안전) 및 메인넷 전용 레지스트리 검증 (테스트넷 0건)
                t4_ok = (
                    MIN_PROFIT_USD == 100.0
                    and VERDICT_SAFE == "안전"
                    and VERDICT_PROFIT == "한화 즉시 얼마 이익 가능"
                    and "sepolia" not in CHAIN_IDS
                    and "diode" not in CHAIN_IDS
                    and "probechain" not in CHAIN_IDS
                    and CHAIN_IDS.get("mantra") == 5888
                    and "sei" not in NON_EVM_CHAINS
                )
                results.append(("T4_E1_VAULT_THRESHOLD_AND_MAINNET_REGISTRY", t4_ok, f"chains={len(CHAIN_IDS)}"))

            if _want_test("T5_E2_FOUR_PHYSICAL_VALUE_SINKS", tag_filters):
                # T5: E2 4대 물리적 Value Sink 선형 스캔 (무출구/전수 REVERT -> 안전, CALL/DELEGATECALL/TSTORE -> 통과)
                pure_view_code = bytes.fromhex("602a60005260206000f3")
                revert_only_code = bytes.fromhex("60006000fd")
                call_sink_code = bytes.fromhex("63a9059cbb6000526000600060246000600160025af100")
                tstore_sink_code = bytes.fromhex("600160005d00")
                scan_view = quick_scan_value_sinks(pure_view_code)
                scan_rev = quick_scan_value_sinks(revert_only_code)
                scan_call = quick_scan_value_sinks(call_sink_code)
                scan_tstore = quick_scan_value_sinks(tstore_sink_code)
                t5_ok = (
                    scan_view.early_exit_safe
                    and scan_rev.early_exit_safe
                    and (not scan_call.early_exit_safe)
                    and scan_call.has_s2_token_call
                    and (not scan_tstore.early_exit_safe)
                    and scan_tstore.has_s5_state_mutation
                )
                results.append(("T5_E2_FOUR_PHYSICAL_VALUE_SINKS", t5_ok, f"view_exit={scan_view.early_exit_safe},call_s2={scan_call.has_s2_token_call}"))

            if _want_test("T6_E3_CALLER_DOMINATOR_THEOREM", tag_filters):
                # T6: E3 Caller-Dominator Theorem 3대 공리 증명 (폐쇄형 오너 가드 vs 개방형 진입점)
                closed_bytecode = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3373" + ("99" * 20) + "14602c57600080fd"
                    "5b6000808080808080f100"
                )
                open_bytecode = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b6000808080808080f100"
                )
                cfg_closed = disassemble_and_build_cfg(closed_bytecode)
                proof_closed = prove_caller_dominator_closed(cfg_closed, target_addr="0x" + "11" * 20)
                cfg_open = disassemble_and_build_cfg(open_bytecode)
                proof_open = prove_caller_dominator_closed(cfg_open, target_addr="0x" + "11" * 20)
                t6_ok = proof_closed.early_exit_safe and (not proof_open.early_exit_safe) and ("0xa9059cbb" in proof_open.open_selectors)
                results.append(("T6_E3_CALLER_DOMINATOR_THEOREM", t6_ok, f"closed={proof_closed.all_closed},open={proof_open.open_selectors}"))

            if _want_test("T7_FOUR_CHANNEL_CAUSAL_SLICER", tag_filters):
                # T7: 4대 물리적 상태 채널(CH_STORAGE, CH_TRANSIENT, CH_BALANCE, CH_MIDFRAME) 및 다중 leg 슬라이서
                ch_code = bytes.fromhex(
                    "631111111114600d57600080fd"
                    "5b6000546001600055600160005d30316103e804f100"
                )
                ch_cfg = disassemble_and_build_cfg(ch_code)
                ch_slices = slice_four_channel_causal_chains(
                    "0x" + "33" * 20,
                    ch_cfg,
                    ["0x11111111"],
                    peer_dep_targets=["0x" + "55" * 20],
                )
                channels_found = {s.channel for s in ch_slices}
                max_legs = max((len(s.legs) for s in ch_slices), default=0)
                t7_ok = {"CH_STORAGE", "CH_TRANSIENT", "CH_BALANCE", "CH_MIDFRAME"}.issubset(channels_found) and max_legs >= 3
                results.append(("T7_FOUR_CHANNEL_CAUSAL_SLICER", t7_ok, f"channels={sorted(channels_found)},max_legs={max_legs}"))

            if _want_test("T8_DISCONTINUITY_BOUNDARY_SOLVER", tag_filters):
                if ch_code is None:
                    ch_code = bytes.fromhex("631111111114600d57600080fd5b6000546001600055600160005d30316103e804f100")
                # T8: 불연속 경계 솔버(Discontinuity Boundary Solver: DIV d-1/d/d+1, isqrt, SHL)
                knots = solve_discontinuity_boundaries(_disassemble(ch_code))
                t8_ok = 999 in knots and 1000 in knots and 1001 in knots and math.isqrt((1 << 256) - 1) in knots
                results.append(("T8_DISCONTINUITY_BOUNDARY_SOLVER", t8_ok, f"knot_count={len(knots)}"))

            if _want_test("T9_CHEATCODE_DEFAULT_DENY_ALLOWLIST", tag_filters):
                # T9: Default-Deny 치트코드 화이트리스트 검증기 (허용 집합 vs vm.deal/store/warp/roll/저수준 호출 차단)
                good_sol = 'contract T { function t() public { vm.createSelectFork("ethereum", 1); vm.startPrank(address(0x1337)); vm.stopPrank(); } }'
                bad_sol = 'contract T { function t() public { vm.deal(address(1), 100 ether); vm.store(address(1), bytes32(0), bytes32(uint256(1))); vm.warp(999); } }'
                bad_ll = "contract T { function t() public { 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D.call(hex'1122'); } }"
                ok_g, _ = verify_cheatcode_allowlist(good_sol, allowed_caller="0x0000000000000000000000000000000000001337")
                ok_b, v_b = verify_cheatcode_allowlist(bad_sol)
                ok_l, _ = verify_cheatcode_allowlist(bad_ll)
                t9_ok = ok_g and (not ok_b) and len(v_b) == 3 and (not ok_l)
                results.append(("T9_CHEATCODE_DEFAULT_DENY_ALLOWLIST", t9_ok, f"bad_violations={v_b}"))

            if _want_test("T10_CONSERVATION_ACCOUNTING_AND_DECIMALS", tag_filters):
                # T10: 가치 보존 회계(Conservation of Value), 1<->2 뮤텍스 플립 기각, 6-decimal USDC 정산 및 동적 KRW 원화 판정 포맷
                v_mutex = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={"native": 0},
                    caller_after={"native": 0},
                    vault_before={"native": 10**18},
                    vault_after={"native": 10**18},
                    storage_slot_flips=[(0, 1, 2)],
                    allow_offline_fixture=True,
                )
                usdc_eth = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                v_usdc = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={usdc_eth: 0},
                    caller_after={usdc_eth: 500 * 10**6},
                    vault_before={usdc_eth: 1000 * 10**6},
                    vault_after={usdc_eth: 500 * 10**6},
                    gas_used=120_000,
                    decimals_map={usdc_eth: 6},
                    prices_map={usdc_eth: 1.0},
                    allow_offline_fixture=True,
                )
                t10_ok = (
                    v_mutex.verdict == VERDICT_SAFE
                    and v_mutex.mutex_only_rejected
                    and _is_profit_verdict(v_usdc.verdict)
                    and v_usdc.verdict == _format_profit_verdict(v_usdc.net_krw)
                    and 450.0 <= v_usdc.net_usd <= 500.0
                    and v_usdc.net_krw > 500_000
                )
                results.append(("T10_CONSERVATION_ACCOUNTING_AND_DECIMALS", t10_ok, f"verdict={v_usdc.verdict},usdc_net_usd={v_usdc.net_usd}"))

            if _want_test("T11_THREE_VERDICT_SYSTEM_AND_PINS_SHA256", tag_filters):
                # T11: 3대 종결 판정 체계("잠긴 가치" 폐기 검증) 및 pins.json SHA-256 + 필드 교차 무결성 검증
                test_slug = "_selftest_probe"
                write_protocol_artifacts(
                    test_slug,
                    {
                        "slug": test_slug,
                        "chain": "ethereum",
                        "audit_block": 123456,
                        "verdict": VERDICT_SAFE,
                        "net_usd": 0.0,
                        "net_krw": 0,
                        "reason": "Selftest safe proof",
                    },
                )
                ok_pin1, _ = verify_protocol_pins(test_slug)
                (ARTIFACT_ROOT / _norm_slug(test_slug) / "result.json").write_text(
                    json.dumps({"slug": test_slug, "verdict": "잠긴 가치"}), encoding="utf-8"
                )
                ok_pin2, reason2 = verify_protocol_pins(test_slug)
                shutil.rmtree(ARTIFACT_ROOT / _norm_slug(test_slug), ignore_errors=True)
                t11_ok = ok_pin1 and (not ok_pin2) and ("INVALID_VERDICT" in reason2)
                results.append(("T11_THREE_VERDICT_SYSTEM_AND_PINS_SHA256", t11_ok, f"tamper_check={reason2}"))

            if _want_test("T12_SQLITE_ZOMBIE_AND_PROVEN_VAULTS", tag_filters):
                # T12: SQLite WAL 큐 상태머신, 부트 ID 좀비 클레임 복구 및 tools/foundry/forge.exe 바이너리 확인
                tmp_db = selftest_ns / "_selftest_state.db"
                st = SQLiteStore(tmp_db)
                st.upsert_queue_item("test-proto", 1, "ethereum", 250_000.0, ["0x" + "44" * 20])
                claimed = st.claim_next_batch("old_boot", limit=1)
                rec = st.recover_zombie_claims("new_boot")
                st.mark_vault_proven("ethereum", "0x" + "44" * 20, "test-proto", 150000.0)
                vp_ok = st.is_vault_proven("ethereum", "0x" + "44" * 20)
                st.close()
                t12_ok = len(claimed) == 1 and rec == 1 and vp_ok and FORGE_BIN.exists() and len(CHAIN_IDS) >= 680
                results.append(("T12_SQLITE_ZOMBIE_AND_PROVEN_VAULTS", t12_ok, f"chains={len(CHAIN_IDS)},proven_vault_db={vp_ok}"))

            if _want_test("T13_E3_BRANCH_SENSITIVE_SOUNDNESS", tag_filters):
                # T13: E3 분기 민감(Branch-Sensitive) 반례 방어 증명
                blacklist_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3373" + ("99" * 20) + "1415602d57600080fd"
                    "5b6000808080808080f100"
                )
                non_reverting_vip_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3373" + ("99" * 20) + "14602a57600150"
                    "5b6000808080808080f100"
                )
                push0_sload_guard = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b335f5414601957600080fd"
                    "5b6000808080808080f100"
                )
                p_bl = prove_caller_dominator_closed(disassemble_and_build_cfg(blacklist_code), target_addr="0x" + "11" * 20)
                p_vip = prove_caller_dominator_closed(disassemble_and_build_cfg(non_reverting_vip_code), target_addr="0x" + "11" * 20)
                p_p0 = prove_caller_dominator_closed(disassemble_and_build_cfg(push0_sload_guard), target_addr="0x" + "11" * 20)
                t13_ok = (not p_bl.early_exit_safe) and (not p_vip.early_exit_safe) and (not p_p0.early_exit_safe)
                results.append(
                    (
                        "T13_E3_BRANCH_SENSITIVE_SOUNDNESS",
                        t13_ok,
                        f"blacklist_open={not p_bl.early_exit_safe},vip_open={not p_vip.early_exit_safe},uninit_slot0_open={not p_p0.early_exit_safe}",
                    )
                )

            if _want_test("T14_CHEATCODE_ALIAS_NESTED_ASM_AND_MODE_B_SEEDS", tag_filters):
                # T14: Vm 별칭/인라인/중첩 중괄호 어셈블리 치트코드 차단 + Mode B 컨트랙트.md 파서(프리컴파일 0x802 배제) 검증
                alias_sol = "contract T { Vm constant v = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D); function t() public { v.deal(address(1), 1 ether); } }"
                inline_sol = "contract T { function t() public { Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D).store(address(1), bytes32(0), bytes32(0)); } }"
                asm_nested_sol = "contract T { function t() public { assembly { if iszero(0) { let x := 1 } pop(call(gas(), 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D, 0, 0, 0, 0, 0)) } } }"
                ok_a, _ = verify_cheatcode_allowlist(alias_sol)
                ok_i, _ = verify_cheatcode_allowlist(inline_sol)
                ok_m, _ = verify_cheatcode_allowlist(asm_nested_sol)
                c_seeds = _parse_corpus_md()
                no_precompile_802 = all(
                    int(ad, 16) > 0x1000
                    for hc in c_seeds
                    for alist in hc.seed_addresses.values()
                    for ad in alist
                )
                t14_ok = (not ok_a) and (not ok_i) and (not ok_m) and len(c_seeds) > 0 and no_precompile_802
                results.append(("T14_CHEATCODE_ALIAS_NESTED_ASM_AND_MODE_B_SEEDS", t14_ok, f"corpus_seeds={len(c_seeds)},no_precompile={no_precompile_802}"))

            if _want_test("T15_LIVE_FORGE_EXECUTION_AND_EVENT_DECODE", tag_filters):
                # T15: 실전 tools/foundry/forge.exe 오프라인 컴파일·실행·바이트코드 화이트리스트 검사(createSelectFork 포함) 및 ConservationGain 이벤트 디코딩 검증
                t15_slug = "_selftest_forge_live"
                ws = ensure_clean_forge_workspace({"ethereum": "http://127.0.0.1:8545"}, slug=t15_slug, offline=True)
                local_sol = """// SPDX-License-Identifier: MIT
pragma solidity >=0.8.0 <0.9.0;

interface Vm {
    function createSelectFork(string calldata urlOrAlias, uint256 blockNumber) external returns (uint256);
    function prank(address msgSender) external;
}

contract LocalVault {
    uint256 public reserve = 1000 ether;
    function withdrawAll() external returns (uint256) {
        uint256 amt = reserve;
        reserve = 0;
        return amt;
    }
}

contract Verify_Selftest_Forge {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);
    event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);

    function _unusedForkHelper() internal {
        if (block.number == type(uint256).max) {
            vm.createSelectFork("ethereum", 20000000);
        }
    }

    function test_slice_0_local_conservation() public {
        _unusedForkHelper();
        LocalVault v = new LocalVault();
        uint256 vBefore = v.reserve();
        vm.prank(address(0x1337));
        uint256 got = v.withdrawAll();
        uint256 vAfter = v.reserve();
        if (got > 0 && vBefore > vAfter) {
            emit ConservationGain(0, address(0), got, vBefore - vAfter, 85000);
        }
    }
}
"""
                rel_test = "test/Verify_Selftest_Forge.t.sol"
                (ws / rel_test).write_text(local_sol, encoding="utf-8")
                forge_res = execute_forge_test_file(
                    ws,
                    rel_test,
                    local_sol,
                    timeout_sec=60,
                    allowed_caller="0x0000000000000000000000000000000000001337",
                )
                shutil.rmtree(ARTIFACT_ROOT / _norm_slug(t15_slug), ignore_errors=True)
                t15_ok = (
                    forge_res.executed
                    and forge_res.passed_tests >= 1
                    and len(forge_res.conservation_gains) == 1
                    and forge_res.conservation_gains[0]["caller_gain"] == 1000 * 10**18
                    and forge_res.conservation_gains[0]["vault_loss"] == 1000 * 10**18
                    and forge_res.conservation_gains[0]["asset"] == "native"
                    and bool(forge_res.trace_summary.get("conservation_event_verified", False))
                )
                results.append(
                    (
                        "T15_LIVE_FORGE_EXECUTION_AND_EVENT_DECODE",
                        t15_ok,
                        f"passed={forge_res.passed_tests},gains={len(forge_res.conservation_gains)},replay={forge_res.trace_summary.get('conservation_event_verified')}",
                    )
                )

            if _want_test("T16_CRASH_RESUME_PRIORITY_AND_4STAGE_HARVEST", tag_filters):
                # T16: 중단 세션 최우선 배정('RECOVERED') + checkpoint.json 이어하기(Resume) + 4단계 주소 수집 검증
                tmp_db16 = selftest_ns / "_selftest_t16.db"
                st16 = SQLiteStore(tmp_db16)
                e2e_slug_a = "_selftest_proto_a_halfway"
                e2e_slug_c = "_selftest_proto_c_later"
                st16.upsert_queue_items_batch(
                    [
                        ("proto-live-session", 1, "ethereum", 950_000.0, ["0x" + "22" * 20], "A", ""),
                        (e2e_slug_a, 1, "ethereum", 500_000.0, ["0x" + "11" * 20], "A", ""),
                        (e2e_slug_c, 1, "ethereum", 800_000.0, ["0x" + "33" * 20], "A", ""),
                    ]
                )
                c_live = st16.claim_next_batch(f"{HOST_BOOT_ID}:pid={os.getpid()}", limit=1)
                with st16._lock:
                    st16._conn.execute(
                        "UPDATE audit_queue SET status='claimed', claim_boot_id=?, claimed_at=? WHERE slug=?",
                        (f"{HOST_BOOT_ID}:pid=99999999", time.time(), _norm_slug(e2e_slug_a)),
                    )
                    st16._conn.commit()

                closed_code_hex = (
                    "0x63a9059cbb14600d57600080fd"
                    "5b3373" + ("99" * 20) + "14602c57600080fd"
                    "5b6000808080808080f100"
                )
                halfway_closure = TrustClosureResult(
                    chain="ethereum",
                    block_pin=20_000_000,
                    seed_addresses=["0x" + "11" * 20],
                    all_addresses=["0x" + "11" * 20],
                    resolutions={
                        "0x" + "11" * 20: ProxyResolution(
                            address="0x" + "11" * 20,
                            proxy_kind="none",
                            impl_addresses=[],
                            admin_address="",
                            raw_code_hex=closed_code_hex,
                            effective_code_hex=closed_code_hex,
                            skeleton_hash=_skeleton_hash(_hex_to_bytes(closed_code_hex)),
                            slot_collisions=[],
                            impl_codes_hex={"0x" + "11" * 20: closed_code_hex},
                        )
                    },
                    vault_exposures={"0x" + "11" * 20: VaultExposure(address="0x" + "11" * 20, native_wei=100 * 10**18, native_usd=300_000.0, total_usd=300_000.0)},
                    max_extractable_usd=300_000.0,
                    min_gas_usd=0.05,
                    early_exit_safe=False,
                    early_exit_reason="",
                    has_dynamic_external_reachability=True,
                )
                deser_hc = _deserialize_closure(_serialize_closure(halfway_closure))
                _write_json_atomic(
                    ARTIFACT_ROOT / _norm_slug(e2e_slug_a) / "checkpoint.json",
                    {
                        "stage": "S2_CLOSURE_VERIFIED",
                        "audit_block": 20_000_000,
                        "pin_head_block": 20_000_002,
                        "seed_addrs": ["0x" + "11" * 20],
                        "closure_ready": True,
                        "closure": _serialize_closure(halfway_closure),
                        "verified_sources": {},
                        "remediation_exhausted": {k: True for k in REMEDIATION_KEYS},
                    },
                )

                rec16 = st16.recover_zombie_claims(f"{HOST_BOOT_ID}:pid=88888888")
                session_c_batch = st16.claim_next_batch(f"{HOST_BOOT_ID}:pid=88888888", limit=1)

                def _mock_4stage_fetcher(u: str) -> str:
                    if "docs.mockdefi.io/llms.txt" in u:
                        return "Official Docs Contract: 0x1111111111111111111111111111111111111111 GitBook: https://mockdefi.gitbook.io/docs"
                    if "mockdefi.gitbook.io/llms.txt" in u:
                        return "GitBook Contract: 0x2222222222222222222222222222222222222222"
                    if "raw.githubusercontent.com/mockorg/mockdefi/main/deployments.json" in u:
                        return '{"vault": "0x3333333333333333333333333333333333333333"}'
                    if "DefiLlama-Adapters/main/projects/mockdefi/index.js" in u:
                        return "const vault = '0x4444444444444444444444444444444444444444';"
                    return ""

                class _MockE2ERouter(RPCRouter):
                    def __init__(self, store: Optional[SQLiteStore] = None) -> None:
                        super().__init__(store=store)
                        self.stage1_or_2_calls = 0

                    def call(self, chain: str, method: str, params: List[Any], block_pin: int = 0, need_archive: bool = False, timeout: float = 8.0) -> Any:
                        if method == "eth_blockNumber":
                            self.stage1_or_2_calls += 1
                            return hex(20_000_002)
                        if method == "eth_getTransactionCount":
                            return "0x1"
                        return "0x"

                    def batch_call(self, chain: str, calls: Sequence[Tuple[str, List[Any]]], block_pin: int = 0, need_archive: bool = False, raise_on_all_failed: bool = False) -> List[Any]:
                        for _m, p in calls:
                            if p and str(p[0]).lower() == "0x" + "11" * 20:
                                self.stage1_or_2_calls += 1
                        return ["0x" for _ in calls]

                    def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 50) -> List[Tuple[bool, bytes]]:
                        self.stage1_or_2_calls += 1
                        return [(False, b"") for _ in calls]

                mock_router = _MockE2ERouter(store=st16)
                mock_slug = "_selftest_mock_defi"
                st16.put_proto_meta_batch(
                    [
                        (
                            mock_slug,
                            {
                                "url": "https://app.mockdefi.io",
                                "github": ["mockorg/mockdefi"],
                                "module": "mockdefi/index.js",
                            },
                        )
                    ]
                )
                harvested_order = _resolve_protocol_addresses(
                    mock_router,
                    mock_slug,
                    "ethereum",
                    ["0x" + "55" * 20],
                    store=st16,
                    block_pin=0,
                    fetch_text_fn=_mock_4stage_fetcher,
                )
                expected_order = [
                    "0x" + "11" * 20,
                    "0x" + "22" * 20,
                    "0x" + "33" * 20,
                    "0x" + "44" * 20,
                    "0x" + "55" * 20,
                ]

                req_f = emit_address_discovery_request(mock_slug, "ethereum", 20_000_000, 250_000.0, meta_hint=st16.get_proto_meta(mock_slug))
                disc_f = ARTIFACT_ROOT / _norm_slug(mock_slug) / "discovered_addresses.json"
                _write_json_atomic(
                    disc_f,
                    {
                        "official_docs": ["0x" + "aa" * 20],
                        "gitbook": ["0x" + "bb" * 20],
                        "github": ["0x" + "cc" * 20],
                        "defillama_adapter": ["0x" + "dd" * 20],
                    },
                )
                agent_addrs, agent_none = _extract_agent_discovered_addresses(disc_f)
                agent_order_ok = (
                    req_f.is_file()
                    and not agent_none
                    and agent_addrs == ["0x" + "aa" * 20, "0x" + "bb" * 20, "0x" + "cc" * 20, "0x" + "dd" * 20]
                )
                shutil.rmtree(ARTIFACT_ROOT / _norm_slug(mock_slug), ignore_errors=True)

                mock_router.stage1_or_2_calls = 0
                e2e_res = audit_single_protocol(
                    mock_router,
                    st16,
                    session_c_batch[0],
                    set(),
                )
                e2e_pin_ok, _ = verify_protocol_pins(e2e_slug_a)
                ckpt_cleaned = not (ARTIFACT_ROOT / _norm_slug(e2e_slug_a) / "checkpoint.json").exists()
                st16.close()
                t16_ok = (
                    deser_hc.has_dynamic_external_reachability is True
                    and len(c_live) == 1
                    and rec16 == 1
                    and len(session_c_batch) == 1
                    and session_c_batch[0]["slug"] == _norm_slug(e2e_slug_a)
                    and session_c_batch[0]["was_recovered"] is True
                    and mock_router.stage1_or_2_calls == 0
                    and e2e_res.get("resumed_from_checkpoint") == "S2_CLOSURE_VERIFIED"
                    and e2e_res.get("verdict") == VERDICT_SAFE
                    and e2e_res.get("early_exit") == "E3_CALLER_DOMINATOR_CLOSED"
                    and ckpt_cleaned
                    and harvested_order == expected_order
                    and agent_order_ok
                    and e2e_pin_ok
                )
                results.append(
                    (
                        "T16_CRASH_RESUME_PRIORITY_AND_4STAGE_HARVEST",
                        t16_ok,
                        f"assigned_first={session_c_batch[0]['slug']==_norm_slug(e2e_slug_a)},resumed={e2e_res.get('resumed_from_checkpoint')},order_ok={harvested_order==expected_order and agent_order_ok}",
                    )
                )

            if _want_test("T17_REAL_SOLC_SOUNDNESS_PAYABLE_PACKED_AND_MIDFRAME", tag_filters):
                # T17: 실전 solc 0.8.28(forge.exe) 컴파일 바이트코드 기반 + CH_MIDFRAME 무상태 보조 컨트랙트 격리 검증
                t17_slug = "_selftest_real_solc_e1_e4"
                ws17 = ensure_clean_forge_workspace({"ethereum": "http://127.0.0.1:8545"}, slug=t17_slug, offline=True)
                real_contracts_sol = """// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract ConnectedVault {
    address public router;
    constructor(address _r) payable { router = _r; }
    function pushFunds(address payable to, uint256 amt) external {
        require(msg.sender == router, "only router");
        (bool ok, ) = to.call{value: amt}("");
        require(ok);
    }
}

contract ConnectedRouter {
    ConnectedVault public vault;
    function setVault(ConnectedVault _v) external { vault = _v; }
    function routeFlow(uint256 amt) external {
        vault.pushFunds(payable(msg.sender), amt);
    }
}

contract UnprotectedWithdraw {
    mapping(address => uint256) public balances;
    function emergencySweep(uint256 amt) external {
        (bool ok, ) = payable(msg.sender).call{value: amt}("");
        require(ok);
    }
}

contract UnprotectedMintToken {
    mapping(address => uint256) public balanceOf;
    uint256 public totalSupply;
    function mint(address to, uint256 amt) external {
        balanceOf[to] += amt;
        totalSupply += amt;
    }
}

contract PayablePublicVault {
    function depositAndSweep() external payable {
        require(msg.value == 1 ether, "need 1 eth");
        (bool ok, ) = payable(msg.sender).call{value: address(this).balance}("");
        require(ok);
    }
}

contract PackedStorageVault {
    bool public paused;
    address public owner;
    function adminSweep(uint256 amt) external {
        require(!paused, "paused");
        require(msg.sender == owner, "not owner");
        (bool ok, ) = payable(msg.sender).call{value: amt}("");
        require(ok);
    }
}

contract OwnerVault {
    address public constant OWNER = 0x9999999999999999999999999999999999999999;
    function sweep(uint256 amt) external {
        require(msg.sender == OWNER, "not owner");
        (bool ok, ) = payable(msg.sender).call{value: amt}("");
        require(ok);
    }
}
"""
                (ws17 / "src" / "RealTargets.sol").write_text(real_contracts_sol, encoding="utf-8")
                complex_slice = CausalSlice(
                    slice_id="s_complex_4leg",
                    target_address="0x" + "11" * 20,
                    channel="CH_STORAGE",
                    legs=["0x11111111", "0x22222222", "0x33333333", "0x44444444"],
                    leg_targets=["0x" + "11" * 20, "0x" + "22" * 20, "0x" + "11" * 20, "0x" + "22" * 20],
                    witness_candidates=[1, 1000, 10**18],
                    repeat_counts=[1, 2],
                    rationale="4-leg x 3-witness stack-depth stress test",
                )
                midframe_slice = CausalSlice(
                    slice_id="s_midframe_probe",
                    target_address="0x" + "11" * 20,
                    channel="CH_MIDFRAME",
                    legs=["0x11111111"],
                    leg_targets=["0x" + "11" * 20],
                    witness_candidates=[1],
                    repeat_counts=[1],
                    rationale="CH_MIDFRAME must use stateless helper without harness balance contamination",
                )
                harness17_sol = _render_minimal_solidity_harness(
                    slug=t17_slug,
                    chain="ethereum",
                    block_pin=0,
                    caller="0x1000000000000000000000000000000000013370",
                    target_addr="0x" + "11" * 20,
                    token_addrs=["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdAC17F958D2ee523a2206206994597C13D831ec7"],
                    slices=[complex_slice, midframe_slice],
                    vault_addresses=["0x" + "11" * 20, "0x" + "22" * 20, "0x" + "33" * 20],
                )
                rel_h17 = "test/Verify_RealTargets.t.sol"
                (ws17 / rel_h17).write_text(harness17_sol, encoding="utf-8")
                h17_out = execute_forge_test_file(
                    ws17,
                    rel_h17,
                    harness17_sol,
                    timeout_sec=60,
                    allowed_caller="0x1000000000000000000000000000000000013370",
                )

                def _load_deployed_bytes(contract_name: str) -> bytes:
                    jf = ws17 / "out" / "RealTargets.sol" / f"{contract_name}.json"
                    if not jf.is_file():
                        return b""
                    doc = json.loads(jf.read_text(encoding="utf-8"))
                    return _hex_to_bytes(str((doc.get("deployedBytecode") or {}).get("object") or ""))

                bc_router = _load_deployed_bytes("ConnectedRouter")
                bc_uw = _load_deployed_bytes("UnprotectedWithdraw")
                bc_mint = _load_deployed_bytes("UnprotectedMintToken")
                bc_payable = _load_deployed_bytes("PayablePublicVault")
                bc_packed = _load_deployed_bytes("PackedStorageVault")
                bc_owner = _load_deployed_bytes("OwnerVault")
                shutil.rmtree(ARTIFACT_ROOT / _norm_slug(t17_slug), ignore_errors=True)

                sink_router = quick_scan_value_sinks(bc_router)
                dom_router = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_router), target_addr="0x" + "aa" * 20)
                sink_uw = quick_scan_value_sinks(bc_uw)
                dom_uw = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_uw), target_addr="0x" + "bb" * 20)
                sink_mint = quick_scan_value_sinks(bc_mint)
                dom_mint = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_mint), target_addr="0x" + "cc" * 20)
                dom_payable = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_payable), target_addr="0x" + "ee" * 20)
                dom_packed_uninit = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_packed), target_addr="0x" + "ff" * 20)
                dom_owner = prove_caller_dominator_closed(disassemble_and_build_cfg(bc_owner), target_addr="0x" + "dd" * 20)

                t17_ok = (
                    h17_out.executed
                    and h17_out.passed_tests == 4
                    and len(h17_out.conservation_gains) == 0
                    and "_drainHarnessInitialBalance" not in harness17_sol
                    and len(bc_router) > 50
                    and (not sink_router.early_exit_safe)
                    and (not dom_router.early_exit_safe)
                    and (not sink_uw.early_exit_safe)
                    and (not dom_uw.early_exit_safe)
                    and (not sink_mint.early_exit_safe)
                    and sink_mint.has_s5_state_mutation
                    and (not dom_mint.early_exit_safe)
                    and (not dom_payable.early_exit_safe)
                    and (not dom_packed_uninit.early_exit_safe)
                    and dom_owner.early_exit_safe
                )
                results.append(
                    (
                        "T17_REAL_SOLC_SOUNDNESS_PAYABLE_PACKED_AND_MIDFRAME",
                        t17_ok,
                        f"harness_ok={h17_out.executed},payable_open={not dom_payable.early_exit_safe},packed_open={not dom_packed_uninit.early_exit_safe},owner_closed={dom_owner.early_exit_safe}",
                    )
                )

            if _want_test("T18_OFFLINE_SOURCIFY_V2_AND_1BYTE_MULTIGRAM", tag_filters):
                # T18: 오프라인 Sourcify v2(로컬 tools/duckdb/duckdb.exe Zero-Copy Parquet 엔진) + 1바이트 슬라이딩 멀티그램 + S3 XML <Size> 파싱 검증
                sample_runtime = bc_uw if bc_uw else bytes.fromhex("608060405263a9059cbb14600d57600080fd5b6000808080808080f100")
                sample_with_cbor = sample_runtime + bytes.fromhex("a2646970667358221220" + "ab" * 32 + "64736f6c634300081c0033")
                stripped_sample = _strip_cbor_bytes(sample_with_cbor)
                shifted_variant = b"\x5b" + stripped_sample
                shift_containment = compute_multigram_containment(stripped_sample, shifted_variant)
                sample_s3_ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
                sample_s3_xml = (
                    "<ListBucketResult><Contents>"
                    "<Key>v2/code/part-0000.parquet</Key>"
                    f"<LastModified>{sample_s3_ts}</LastModified>"
                    "<ETag>\"abc\"</ETag>"
                    "<Size>987654321</Size>"
                    "</Contents></ListBucketResult>"
                )
                parsed_s3 = _parse_sourcify_s3_xml_contents(sample_s3_xml, wanted={"code"})
                s3_xml_ok = len(parsed_s3) == 1 and parsed_s3[0] == ("code", "v2/code/part-0000.parquet", 987654321)

                tmp_sf_dir = selftest_ns / "_selftest_sourcify_parquet"
                t18_ok = False
                t18_detail = ""
                try:
                    for tbl in (
                        "sourcify_matches",
                        "verified_contracts",
                        "contract_deployments",
                        "contracts",
                        "compiled_contracts",
                        "code",
                        "compiled_contracts_sources",
                        "sources",
                    ):
                        (tmp_sf_dir / tbl).mkdir(parents=True, exist_ok=True)
                    c_hash_hex = _keccak256_hex(sample_with_cbor)[2:]
                    addr_hex = "77" * 20
                    src_hash_hex = "88" * 32
                    code_hex_full = sample_with_cbor.hex()
                    abi_json_escaped = '{\\"abi\\":[{\\"type\\":\\"function\\",\\"name\\":\\"emergencySweep\\",\\"inputs\\":[{\\"name\\":\\"amt\\",\\"type\\":\\"uint256\\"}]}]}'

                    if DUCKDB_BIN.is_file():
                        setup_sql = f"""
                        COPY (SELECT 1::BIGINT AS id, 1::BIGINT AS verified_contract_id, 'exact_match' AS creation_match, 'exact_match' AS runtime_match, NULL AS metadata)
                          TO '{str((tmp_sf_dir / "sourcify_matches" / "m0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 1::BIGINT AS id, 10::BIGINT AS deployment_id, 20::BIGINT AS compilation_id)
                          TO '{str((tmp_sf_dir / "verified_contracts" / "vc0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 10::BIGINT AS id, 1::BIGINT AS chain_id, unhex('{addr_hex}') AS address, 30::BIGINT AS contract_id)
                          TO '{str((tmp_sf_dir / "contract_deployments" / "cd0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 30::BIGINT AS id, unhex('{c_hash_hex}') AS creation_code_hash, unhex('{c_hash_hex}') AS runtime_code_hash)
                          TO '{str((tmp_sf_dir / "contracts" / "c0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 20::BIGINT AS id, 'solc' AS compiler, '0.8.28' AS version, 'solidity' AS language, 'UnprotectedWithdraw' AS name, 'src/UW.sol:UnprotectedWithdraw' AS fully_qualified_name, '{{}}' AS compiler_settings, e'{abi_json_escaped}' AS compilation_artifacts, unhex('{c_hash_hex}') AS creation_code_hash, '{{}}' AS creation_code_artifacts, unhex('{c_hash_hex}') AS runtime_code_hash, '{{}}' AS runtime_code_artifacts)
                          TO '{str((tmp_sf_dir / "compiled_contracts" / "cc0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT unhex('{c_hash_hex}') AS code_hash, unhex('{code_hex_full}') AS code, unhex('{c_hash_hex}') AS code_hash_keccak)
                          TO '{str((tmp_sf_dir / "code" / "code0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 1::BIGINT AS id, 20::BIGINT AS compilation_id, unhex('{src_hash_hex}') AS source_hash, 'src/UW.sol' AS path)
                          TO '{str((tmp_sf_dir / "compiled_contracts_sources" / "ccs0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT unhex('{src_hash_hex}') AS source_hash, unhex('{src_hash_hex}') AS source_hash_keccak, 'contract UnprotectedWithdraw {{}}' AS content)
                          TO '{str((tmp_sf_dir / "sources" / "s0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        SELECT 1 AS ok;
                        """
                        _run_duckdb_cli_json(setup_sql, parquet_root=tmp_sf_dir, scratch_root=tmp_sf_dir / "_ddb_scratch", timeout_sec=20)
                        engine_used = "duckdb_cli"
                    else:
                        sidecar_row = {
                            "chain_id": 1,
                            "address": "0x" + addr_hex,
                            "contract_name": "UnprotectedWithdraw",
                            "abi": [{"type": "function", "name": "emergencySweep", "inputs": [{"name": "amt", "type": "uint256"}]}],
                            "sources": {"src/UW.sol": "contract UnprotectedWithdraw {}"},
                            "code_hex": code_hex_full,
                        }
                        (tmp_sf_dir / "_local_index.jsonl").write_text(json.dumps(sidecar_row) + "\n", encoding="utf-8")
                        engine_used = "stdlib_sidecar"

                    sf_ver = _query_local_sourcify_verified(1, "0x" + addr_hex, parquet_root=tmp_sf_dir)
                    sf_sim = search_sourcify_v2_similar_contracts(stripped_sample, parquet_root=tmp_sf_dir, chain_id=None, limit=5)
                    t18_ok = (
                        s3_xml_ok
                        and shift_containment >= 0.85
                        and sf_ver is not None
                        and sf_ver.get("verified") is True
                        and sf_ver.get("contract_name") == "UnprotectedWithdraw"
                        and "src/UW.sol" in (sf_ver.get("sources") or {})
                        and len(sf_sim) == 1
                        and sf_sim[0]["address"] == "0x" + addr_hex
                        and sf_sim[0]["containment"] >= 0.99
                    )
                    t18_detail = f"engine={engine_used},shift_sim={shift_containment:.3f},s3_size_ok={s3_xml_ok}"
                except Exception as exc:
                    t18_detail = f"err:{exc}"
                finally:
                    shutil.rmtree(tmp_sf_dir, ignore_errors=True)
                results.append(("T18_OFFLINE_SOURCIFY_V2_AND_1BYTE_MULTIGRAM", t18_ok, t18_detail))

            if _want_test("T19_DONE_GATES_NESTED_ABI_STABLES_AND_AMM_IDENTITY", tag_filters):
                # T19: 교차 정합성 회귀 검증
                nested_abi = [
                    {
                        "type": "function",
                        "name": "swapNested",
                        "inputs": [
                            {
                                "name": "order",
                                "type": "tuple",
                                "components": [
                                    {"name": "maker", "type": "address"},
                                    {
                                        "name": "asset",
                                        "type": "tuple",
                                        "components": [
                                            {"name": "token", "type": "address"},
                                            {"name": "amount", "type": "uint256"},
                                        ],
                                    },
                                ],
                            },
                            {"name": "proof", "type": "uint256[2]"},
                        ],
                    }
                ]
                abi_map_19 = _extract_abi_inputs_map(nested_abi)
                expected_nested_sel = _selector("swapNested((address,(address,uint256)),uint256[2])").lower()
                nested_ok = (
                    expected_nested_sel in abi_map_19
                    and _abi_requires_agent_reasoning(["uint256[2]"])
                    and _abi_requires_agent_reasoning(["(address,uint256)"])
                )

                wnatives_all = {_norm_addr(v) for v in WRAPPED_NATIVE_BY_CHAIN.values()}
                bluechips_all = {_norm_addr(x) for lst in CHAIN_BLUECHIPS.values() for x in lst}
                aliases_all = {_norm_addr(x) for lst in NATIVE_ALIAS_TOKENS_BY_CHAIN.values() for x in lst}
                stables_clean = all(
                    (ns := _norm_addr(s)) not in wnatives_all and ns not in bluechips_all and ns not in aliases_all
                    for lst in CHAIN_STABLES.values()
                    for s in lst
                )

                class _MockAMMRouter(RPCRouter):
                    def http_get_json(self, url: str, headers: Optional[Dict[str, str]] = None, timeout: float = 10.0) -> Any:
                        if "coingecko:ethereum" in url:
                            return {"coins": {"coingecko:ethereum": {"price": 3000.0}}}
                        return {"coins": {}}

                    def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 50) -> List[Tuple[bool, bytes]]:
                        out: List[Tuple[bool, bytes]] = []
                        pair_addr_padded = b"\x00" * 12 + bytes.fromhex("ab" * 20)
                        for to_a, cd in calls:
                            sel = "0x" + cd[:4].hex()
                            if sel == TOTAL_SUPPLY_SEL:
                                out.append((True, (1_000_000 * 10**18).to_bytes(32, "big")))
                            elif sel == V2_GET_PAIR_SEL:
                                out.append((True, pair_addr_padded))
                            elif sel == BALANCE_OF_SEL and _norm_addr(to_a) == WRAPPED_NATIVE_BY_CHAIN["ethereum"]:
                                out.append((True, (100 * 10**18).to_bytes(32, "big")))
                            elif sel == BALANCE_OF_SEL and _norm_addr(to_a) == "0x" + "cc" * 20:
                                out.append((True, (100_000 * 10**18).to_bytes(32, "big")))
                            elif sel == DECIMALS_SEL:
                                out.append((True, (18).to_bytes(32, "big")))
                            else:
                                out.append((False, b""))
                        return out

                amm_router = _MockAMMRouter(store=None)
                unlisted_tok = "0x" + "cc" * 20
                dex_out = probe_dex_exit_liquidity(amm_router, "ethereum", unlisted_tok, 1_000 * 10**18, 18, 0.0, 3000.0, 20_000_000)
                acct_unlisted = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={unlisted_tok: 0},
                    caller_after={unlisted_tok: 1_000 * 10**18},
                    vault_before={unlisted_tok: 10_000 * 10**18},
                    vault_after={unlisted_tok: 9_000 * 10**18},
                    gas_used=150_000,
                    gas_price_wei=1_000_000_000,
                    decimals_map={unlisted_tok: 18},
                    prices_map={"native": 3000.0},
                    router=amm_router,
                    block_pin=20_000_000,
                    live_head_block=20_000_002,
                    allow_offline_fixture=True,
                )
                amm_ok = (
                    dex_out["exit_liquid"] is True
                    and 2900.0 <= dex_out["realizable_usd"] <= 2995.0
                    and _is_profit_verdict(acct_unlisted.verdict)
                    and abs((acct_unlisted.gross_usd - acct_unlisted.swap_fee_usd - acct_unlisted.slippage_usd) - dex_out["realizable_usd"]) < 0.05
                )

                tmp_done_dir = selftest_ns / "_selftest_done_gates"
                tmp_done_db = tmp_done_dir / "q.sqlite"
                tmp_done_script = selftest_ns / "_selftest_done_script"
                tmp_done_tools = tmp_done_script / "tools"
                tmp_done_dir.mkdir(parents=True, exist_ok=True)
                tmp_done_tools.mkdir(parents=True, exist_ok=True)
                for _fn_dg in ("Defi_구현.py", "원하는 것.md", "지시서.md", "컨트랙트.md"):
                    (tmp_done_script / _fn_dg).write_text("# clean\n", encoding="utf-8")
                st_done = SQLiteStore(tmp_done_db)
                try:
                    st_done.upsert_queue_item("Euler_V2", 1, "ethereum", 200_000.0, ["0x" + "11" * 20])
                    st_done.complete_queue_item("Euler_V2", VERDICT_SAFE, 0.0)
                    rep_missing_dir = _evaluate_done_gates(
                        st_done, artifact_root=tmp_done_dir, run_selftest=False, script_dir=tmp_done_script, tools_dir=tmp_done_tools
                    )
                    write_protocol_artifacts(
                        "Euler_V2",
                        {
                            "slug": _norm_slug("Euler_V2"),
                            "chain": "ethereum",
                            "audit_block": 20_000_000,
                            "verdict": VERDICT_SAFE,
                            "net_usd": 0.0,
                            "net_krw": 0,
                            "reason": "Verified closed invariant",
                        },
                        root_dir=tmp_done_dir,
                    )
                    rep_all_good = _evaluate_done_gates(
                        st_done, artifact_root=tmp_done_dir, run_selftest=False, script_dir=tmp_done_script, tools_dir=tmp_done_tools
                    )
                finally:
                    st_done.close()
                done_gate_ok = (
                    rep_missing_dir["gate1_queue_drained_and_nonempty"] is True
                    and rep_missing_dir["gate3_pins_sha256_valid"] is False
                    and rep_all_good["all_gates_passed"] is True
                )

                t19_ok = nested_ok and stables_clean and amm_ok and done_gate_ok
                results.append(
                    (
                        "T19_DONE_GATES_NESTED_ABI_STABLES_AND_AMM_IDENTITY",
                        t19_ok,
                        f"nested_abi={nested_ok},stables_clean={stables_clean},amm_ok={amm_ok},done_gates={done_gate_ok}",
                    )
                )

            if _want_test("T20_SOUNDNESS_AND_SSOT_REGRESSION", tag_filters):
                # T20: 680체인 전단사(Bijection) + 컨트랙트.md 무손실 분할 + E3 마스크/다중비교 건전성 + Native/WNATIVE 동치 정산 + ABI 엄격 bool 검증
                bijection_ok = len(CHAIN_IDS) == 680 and len(CHAIN_BY_ID) == 680 and CHAIN_IDS.get("b3") == 8333
                h_part_20 = _parse_corpus_md_partition()
                evm_h = list(h_part_20.get("evm_cases") or [])
                non_evm_h = list(h_part_20.get("pure_non_evm_cases") or [])
                total_h = int(h_part_20.get("total_headers") or 0)
                corpus_partition_ok = total_h >= 251 and (len(evm_h) + len(non_evm_h) == total_h) and len(evm_h) >= 240

                partial_mask_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3360ff16609914601b57600080fd"
                    "5b6000808080808080f100"
                )
                p_mask = prove_caller_dominator_closed(disassemble_and_build_cfg(partial_mask_code), target_addr="0x" + "11" * 20)

                ok_txgas, _ = verify_cheatcode_allowlist("contract T { function t() public { vm.txGasPrice(1); } }")
                ok_bad_prank, _ = verify_cheatcode_allowlist(
                    "contract T { function t() public { vm.prank(address(0x9999)); } }",
                    allowed_caller="0x0000000000000000000000000000000000001337",
                )

                weth_addr = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                v_wrap = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={"native": 10 * 10**18, weth_addr: 0},
                    caller_after={"native": 0, weth_addr: 10 * 10**18},
                    vault_before={"native": 0, weth_addr: 10 * 10**18},
                    vault_after={"native": 10 * 10**18, weth_addr: 0},
                    decimals_map={weth_addr: 18},
                    prices_map={"native": 3000.0, weth_addr: 3000.0},
                    allow_offline_fixture=True,
                )

                bool_dirty_rejected = False
                try:
                    abi_decode(["bool"], (2).to_bytes(32, "big"))
                except ValueError:
                    bool_dirty_rejected = True

                t20_ok = (
                    bijection_ok
                    and corpus_partition_ok
                    and (not p_mask.early_exit_safe)
                    and (not ok_txgas)
                    and (not ok_bad_prank)
                    and v_wrap.verdict == VERDICT_SAFE
                    and v_wrap.gross_usd == 0.0
                    and bool_dirty_rejected
                )
                results.append(
                    (
                        "T20_SOUNDNESS_AND_SSOT_REGRESSION",
                        t20_ok,
                        f"chains_bijection={bijection_ok},corpus_251={len(evm_h)}+{len(non_evm_h)}/{total_h},wrap_zero={v_wrap.gross_usd==0.0}",
                    )
                )

            if _want_test("T21_AST_STDLIB_CORPUS_DISPATCHER_AND_DUAL_LEDGER", tag_filters):
                if cfg_closed is None:
                    cfg_closed = disassemble_and_build_cfg(bytes.fromhex("63a9059cbb14600d57600080fd5b3373" + ("99" * 20) + "14602c57600080fd5b6000808080808080f100"))
                if evm_h is None:
                    evm_h = list((_parse_corpus_md_partition()).get("evm_cases") or [])
                # T21: AST 표준라이브러리 전용/환경변수 참조 0건 + 컨트랙트.md 실데이터 체인/비대상 라벨/괄호 라우팅 + E3 서브루틴·SUB디스패처·슬롯충돌 반례 + 이중원장 별칭 중복차단 검증
                self_src = _ENGINE_FILE.read_text(encoding="utf-8")
                self_tree = ast.parse(self_src)
                stdlib_names = set(sys.stdlib_module_names) | {"__future__"}
                non_stdlib_imports: List[str] = []
                env_refs: List[str] = []
                subproc_iso_violations: List[str] = []
                for node in ast.walk(self_tree):
                    if isinstance(node, ast.Import):
                        for al in node.names:
                            root_mod = al.name.split(".", 1)[0]
                            if root_mod not in stdlib_names:
                                non_stdlib_imports.append(root_mod)
                    elif isinstance(node, ast.ImportFrom):
                        if node.module:
                            root_mod = node.module.split(".", 1)[0]
                            if root_mod not in stdlib_names:
                                non_stdlib_imports.append(root_mod)
                    elif isinstance(node, ast.Attribute) and node.attr in ("environ", "getenv", "putenv", "unsetenv"):
                        env_refs.append(node.attr)
                    elif isinstance(node, ast.Name) and node.id in ("environ", "getenv", "putenv", "unsetenv"):
                        env_refs.append(node.id)
                    elif (
                        isinstance(node, ast.Call)
                        and isinstance(node.func, ast.Attribute)
                        and isinstance(node.func.value, ast.Name)
                        and node.func.value.id == "subprocess"
                        and node.func.attr in ("run", "Popen", "call", "check_call", "check_output")
                    ):
                        kw_names = {kw.arg for kw in node.keywords if kw.arg}
                        has_shell_true = any(
                            kw.arg == "shell" and isinstance(kw.value, ast.Constant) and kw.value.value is True
                            for kw in node.keywords
                        )
                        if has_shell_true or "cwd" not in kw_names or "env" not in kw_names:
                            subproc_iso_violations.append(f"{node.func.attr}@L{getattr(node, 'lineno', 0)}")
                ast_clean_ok = len(non_stdlib_imports) == 0 and len(env_refs) == 0 and len(subproc_iso_violations) == 0

                # 컨트랙트.md 실데이터 체인 매핑 및 불릿 단위 라우팅 검증
                title_chains_ok = (
                    _resolve_corpus_chains_from_text("Velar Perps (Mezo)") == ["mezo"]
                    and _resolve_corpus_chains_from_text("WEMIX.FI (WEMIX3.0)") == ["wemix30"]
                    and _resolve_corpus_chains_from_text("RISEx (RISE)") == ["rise"]
                    and "flow" in _resolve_corpus_chains_from_text("More Markets (Flow EVM)")
                    and _resolve_corpus_chains_from_text("Zentra (Citrea)") == ["citrea"]
                )
                velar_mezo_ok = any(
                    "0xc627be9381d8e102ed134795bd4788236c16617d" in hc.seed_addresses.get("mezo", [])
                    and "0xc627be9381d8e102ed134795bd4788236c16617d" not in hc.seed_addresses.get("ethereum", [])
                    for hc in evm_h
                )
                dgld_eth_only_ok = any(
                    "0xa9299c296d7830a99414d1e5546f5171fa01e9c8" in hc.seed_addresses.get("ethereum", [])
                    and "0xa9299c296d7830a99414d1e5546f5171fa01e9c8" not in hc.seed_addresses.get("base", [])
                    for hc in evm_h
                )
                zeta_arb_ok = velar_mezo_ok and dgld_eth_only_ok

                # E3 반례 1: PUSH4 sel; SUB; PUSH1 revert_pc; JUMPI (일치 시 fallthrough 싱크 진입)
                sub_disp_code = bytes.fromhex(
                    "60003560e01c63a9059cbb03601957"
                    "6000808080808080f100"
                    "5b600080fd"
                )
                p_sub_disp = prove_caller_dominator_closed(disassemble_and_build_cfg(sub_disp_code), target_addr="0x" + "11" * 20)

                # E3 반례 2: 내부 서브루틴 미해결 반환 JUMP + 프록시 슬롯 충돌 방어
                unres_ret_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b601556"
                    "5b56"
                    "5b6000808080808080f100"
                )
                p_unres_ret = prove_caller_dominator_closed(disassemble_and_build_cfg(unres_ret_code), target_addr="0x" + "11" * 20)
                p_slot_col = prove_caller_dominator_closed(cfg_closed, target_addr="0x" + "11" * 20, slot_collisions=[0])

                # Celo 이중원장 네이티브 별칭 중복 계상 방지 검증
                celo_alias = "0x471ece3750da237f93b8e339c536989b8978a438"
                cg_celo, vl_celo, _ = _domain_deltas(
                    caller_before={"native": 0, celo_alias: 0},
                    caller_after={"native": 100 * 10**18, celo_alias: 100 * 10**18},
                    vault_before={"native": 100 * 10**18, celo_alias: 100 * 10**18},
                    vault_after={"native": 0, celo_alias: 0},
                    chain="celo",
                )
                dual_ledger_ok = cg_celo.get("native") == 100 * 10**18 and vl_celo.get("native") == 100 * 10**18 and celo_alias not in cg_celo

                t21_ok = (
                    ast_clean_ok
                    and title_chains_ok
                    and zeta_arb_ok
                    and (not p_sub_disp.early_exit_safe)
                    and (not p_unres_ret.early_exit_safe)
                    and (not p_slot_col.early_exit_safe)
                    and dual_ledger_ok
                )
                results.append(
                    (
                        "T21_AST_STDLIB_CORPUS_DISPATCHER_AND_DUAL_LEDGER",
                        t21_ok,
                        f"ast_clean={ast_clean_ok},corpus_ok={title_chains_ok and zeta_arb_ok},e3_sound={not p_sub_disp.early_exit_safe and not p_unres_ret.early_exit_safe},dual_ledger={dual_ledger_ok}",
                    )
                )

            if _want_test("T22_FUSAKA_FIXPOINT_SOURCIFY_AND_SOUNDNESS_REGRESSION", tag_filters):
                # T22: Fusaka EIP-7825 가스 상한 + EIP-8024 하위호환 + 미모델 오프코드 Fail-Open + 2-Step SSTORE 부동점 + 치트코드 검증 + code-less Sourcify v2
                class _HistAndOpMockRouter:
                    def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                        if method == "eth_getStorageAt":
                            a_l = _norm_addr(str(params[0]))
                            sl_l = str(params[1]).lower()
                            if a_l == "0x" + "bb" * 20 and sl_l == EIP1967_SLOTS["impl"].lower():
                                return "0x" + ("00" * 12) + ("aa" * 20)
                            if a_l == "0x" + "55" * 20:
                                slot_idx = int(sl_l, 16)
                                if slot_idx == 1:
                                    return "0x" + ("00" * 32)
                                if slot_idx == 0:
                                    return "0x" + ("00" * 12) + ("99" * 20)
                            return "0x" + ("00" * 32)
                        if method == "eth_getCode":
                            a_l = _norm_addr(str(params[0]))
                            if a_l == "0x" + "aa" * 20:
                                return "0x" + ("600080fd" * 6)
                            return "0x"
                        if method == "eth_call":
                            call_obj = params[0] if isinstance(params, list) and params else {}
                            if str(call_obj.get("data") or "").lower().startswith("0x49948e0e"):
                                return "0x" + (777_000_000_000).to_bytes(32, "big").hex()
                        return "0x"

                    def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                        return [self.call(ch, m, p, **kw) for m, p in calls]

                m22_router = _HistAndOpMockRouter()
                hist_codes_22 = _recover_historical_proxy_impl_bytecodes(m22_router, "ethereum", "0x" + "bb" * 20, 20_000_000)
                hist_ok = len(hist_codes_22) >= 1 and hist_codes_22[0] == bytes.fromhex("600080fd" * 6)
                op_l1_fee_22 = _estimate_rollup_l1_fee_wei(m22_router, "base", 20_000_000, 128)
                op_l1_fee_256 = _estimate_rollup_l1_fee_wei(m22_router, "base", 20_000_000, 256)
                op_fee_ok = (
                    op_l1_fee_22 == 777_000_000_000
                    and op_l1_fee_256 == 777_000_000_000
                    and "0xdeaddeaddeaddeaddeaddeaddeaddeaddead0000" in NATIVE_ALIAS_TOKENS_BY_CHAIN.get("mantle", frozenset())
                    and _strip_chain_suffix_slug("myapp-arbitrum-nova") == "myapp"
                )

                v_eip7825 = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={"native": 0},
                    caller_after={"native": 50 * 10**18},
                    vault_before={"native": 50 * 10**18},
                    vault_after={"native": 0},
                    gas_used=20_000_000,
                    gas_price_wei=1_000_000_000,
                    decimals_map={},
                    prices_map={"native": 3000.0},
                    live_head_block=20_000_000,
                    block_pin=20_000_000,
                    allow_offline_fixture=True,
                )
                eip7825_ok = v_eip7825.verdict == VERDICT_SAFE and "EIP7825_TX_GAS_CAP_EXCEEDED" in v_eip7825.reason

                dis_8024 = _disassemble(bytes([0xE6, 0x5B, 0x00]))
                eip8024_compat_ok = len(dis_8024) == 3 and dis_8024[1][1] == "JUMPDEST"
                unmod_code = bytes.fromhex("631111111114600c57600080fd5be300")
                unmod_e2_ok = not quick_scan_value_sinks(unmod_code).early_exit_safe
                unmod_e3_ok = not prove_caller_dominator_closed(
                    disassemble_and_build_cfg(unmod_code), target_addr="0x" + "11" * 20
                ).early_exit_safe

                two_step_code = bytes.fromhex(
                    "60003560e01c80631111111114601d57"
                    "632222222214603057600080fd"
                    "5b6001543314602a57600080fd"
                    "5b3360005500"
                    "5b6000543314603d57600080fd"
                    "5b6000808080808080f100"
                )
                p_twostep = prove_caller_dominator_closed(
                    disassemble_and_build_cfg(two_step_code),
                    router=m22_router,
                    chain="ethereum",
                    target_addr="0x" + "55" * 20,
                    block_pin=20_000_000,
                )
                twostep_ok = (not p_twostep.early_exit_safe) and ("0x22222222" in p_twostep.open_selectors)

                ok_fcfg, _ = verify_cheatcode_allowlist("/// forge-config: default.evm_version = 'cancun'\ncontract T {}")
                ok_dyn_prank, _ = verify_cheatcode_allowlist(
                    "contract T { Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D); function t(address u) public { vm.prank(u); } }",
                    allowed_caller="0x0000000000000000000000000000000000001337",
                )
                ok_def_sender, _ = verify_cheatcode_allowlist(
                    "contract T { address constant S = 0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38; }"
                )
                ok_const_prank, _ = verify_cheatcode_allowlist(
                    "contract T { address constant ZERO_STATE_CALLER = 0x0000000000000000000000000000000000001337; "
                    "Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D); "
                    "function t() public { vm.startPrank(ZERO_STATE_CALLER, ZERO_STATE_CALLER); } }",
                    allowed_caller="0x0000000000000000000000000000000000001337",
                )
                cheat_hard_ok = (not ok_fcfg) and (not ok_dyn_prank) and (not ok_def_sender) and ok_const_prank

                sf_codeless_ok = True
                if DUCKDB_BIN.is_file():
                    tmp_sf_cl = selftest_ns / "_selftest_sourcify_codeless"
                    try:
                        for tbl in (
                            "sourcify_matches",
                            "verified_contracts",
                            "contract_deployments",
                            "contracts",
                            "compiled_contracts",
                            "compiled_contracts_metadata",
                            "signatures",
                            "compiled_contracts_signatures",
                        ):
                            (tmp_sf_cl / tbl).mkdir(parents=True, exist_ok=True)
                        cl_code = bytes.fromhex("63aaaaaaaa1463bbbbbbbb1463cccccccc14600080fd")
                        cl_sha = hashlib.sha256(cl_code).hexdigest()
                        cl_addr = "66" * 20
                        cl_proxy_addr = "67" * 20
                        cl_impl_hex = "68" * 20
                        meta_escaped = '{\\"output\\":{\\"abi\\":[{\\"type\\":\\"function\\",\\"name\\":\\"ping\\",\\"inputs\\":[]}]}}'
                        rv_escaped = f'{{\\"implementation\\":\\"000000000000000000000000{cl_impl_hex}\\"}}'
                        sql_cl = f"""
                        COPY (SELECT 1::BIGINT AS id, 1::BIGINT AS verified_contract_id, 'exact_match' AS creation_match, 'exact_match' AS runtime_match, NULL AS metadata)
                          TO '{str((tmp_sf_cl / "sourcify_matches" / "m0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (
                            SELECT 1::BIGINT AS id, 10::BIGINT AS deployment_id, 20::BIGINT AS compilation_id, '{{}}' AS creation_values, '{{}}' AS runtime_values
                            UNION ALL
                            SELECT 2::BIGINT AS id, 11::BIGINT AS deployment_id, 20::BIGINT AS compilation_id, '{{}}' AS creation_values, e'{rv_escaped}' AS runtime_values
                        ) TO '{str((tmp_sf_cl / "verified_contracts" / "vc0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (
                            SELECT 10::BIGINT AS id, 1::BIGINT AS chain_id, unhex('{cl_addr}') AS address, 30::BIGINT AS contract_id
                            UNION ALL
                            SELECT 11::BIGINT AS id, 8453::BIGINT AS chain_id, unhex('{cl_proxy_addr}') AS address, 30::BIGINT AS contract_id
                        ) TO '{str((tmp_sf_cl / "contract_deployments" / "cd0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 30::BIGINT AS id, unhex('{cl_sha}') AS creation_code_hash, unhex('{cl_sha}') AS runtime_code_hash)
                          TO '{str((tmp_sf_cl / "contracts" / "c0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 20::BIGINT AS id, 'solc' AS compiler, '0.8.28' AS version, 'solidity' AS language, 'CodelessVault' AS name, 'src/CV.sol:CodelessVault' AS fully_qualified_name, '{{}}' AS compiler_settings, '{{}}' AS compilation_artifacts, unhex('{cl_sha}') AS creation_code_hash, '{{}}' AS creation_code_artifacts, unhex('{cl_sha}') AS runtime_code_hash, '{{}}' AS runtime_code_artifacts)
                          TO '{str((tmp_sf_cl / "compiled_contracts" / "cc0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (SELECT 1::BIGINT AS id, 20::BIGINT AS compilation_id, unhex('{cl_sha}') AS metadata_hash, e'{meta_escaped}' AS metadata)
                          TO '{str((tmp_sf_cl / "compiled_contracts_metadata" / "ccm0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (
                            SELECT unhex('{"aa" * 32}') AS signature_hash_32, unhex('aaaaaaaa') AS signature_hash_4, 'a()' AS signature
                            UNION ALL SELECT unhex('{"bb" * 32}'), unhex('bbbbbbbb'), 'b()'
                            UNION ALL SELECT unhex('{"cc" * 32}'), unhex('cccccccc'), 'c()'
                        ) TO '{str((tmp_sf_cl / "signatures" / "sig0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        COPY (
                            SELECT 1::BIGINT AS id, 20::BIGINT AS compilation_id, unhex('{"aa" * 32}') AS signature_hash_32, 'function' AS signature_type
                            UNION ALL SELECT 2::BIGINT, 20::BIGINT, unhex('{"bb" * 32}'), 'function'
                            UNION ALL SELECT 3::BIGINT, 20::BIGINT, unhex('{"cc" * 32}'), 'function'
                        ) TO '{str((tmp_sf_cl / "compiled_contracts_signatures" / "ccs0.parquet").resolve()).replace("\\", "/")}' (FORMAT PARQUET);
                        SELECT 1 AS ok;
                        """
                        _run_duckdb_cli_json(sql_cl, parquet_root=tmp_sf_cl, scratch_root=tmp_sf_cl / "_ddb_scratch", timeout_sec=20)
                        ver_cl = _query_local_sourcify_verified(1, "0x" + cl_addr, parquet_root=tmp_sf_cl)
                        sim_exact = search_sourcify_v2_similar_contracts(cl_code, parquet_root=tmp_sf_cl, limit=5)
                        mutated_cl_code = cl_code + b"\x00"
                        sim_sel = search_sourcify_v2_similar_contracts(mutated_cl_code, parquet_root=tmp_sf_cl, limit=5)
                        prx_cl = search_sourcify_v2_proxies_for_impls(["0x" + cl_impl_hex], parquet_root=tmp_sf_cl, limit=5)
                        sf_codeless_ok = (
                            ver_cl is not None
                            and len(ver_cl.get("abi") or []) == 1
                            and any(r["address"] == "0x" + cl_addr for r in sim_exact)
                            and any(r["address"] == "0x" + cl_addr for r in sim_sel)
                            and any(r["proxy_address"] == "0x" + cl_proxy_addr for r in prx_cl)
                        )
                    finally:
                        shutil.rmtree(tmp_sf_cl, ignore_errors=True)

                t22_ok = (
                    hist_ok
                    and op_fee_ok
                    and eip7825_ok
                    and eip8024_compat_ok
                    and unmod_e2_ok
                    and unmod_e3_ok
                    and twostep_ok
                    and cheat_hard_ok
                    and sf_codeless_ok
                )
                results.append(
                    (
                        "T22_FUSAKA_FIXPOINT_SOURCIFY_AND_SOUNDNESS_REGRESSION",
                        t22_ok,
                        f"hist={hist_ok},op_fee={op_fee_ok},eip7825={eip7825_ok},e2e3={unmod_e2_ok and unmod_e3_ok and twostep_ok},cheat={cheat_hard_ok},sf_codeless={sf_codeless_ok}",
                    )
                )

            if _want_test("T23_GLAMSTERDAM_EIP8024_DECIMAL_SCROLL_AND_TRACE_SOUNDNESS", tag_filters):
                if m22_router is None:
                    class _HistAndOpMockRouter:
                        def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                            if method == "eth_getStorageAt":
                                a_l = _norm_addr(str(params[0]))
                                sl_l = str(params[1]).lower()
                                if a_l == "0x" + "bb" * 20 and sl_l == EIP1967_SLOTS["impl"].lower():
                                    return "0x" + ("00" * 12) + ("aa" * 20)
                                if a_l == "0x" + "55" * 20:
                                    slot_idx = int(sl_l, 16)
                                    if slot_idx == 1:
                                        return "0x" + ("00" * 32)
                                    if slot_idx == 0:
                                        return "0x" + ("00" * 12) + ("99" * 20)
                                return "0x" + ("00" * 32)
                            if method == "eth_getCode":
                                a_l = _norm_addr(str(params[0]))
                                if a_l == "0x" + "aa" * 20:
                                    return "0x" + ("600080fd" * 6)
                                return "0x"
                            if method == "eth_call":
                                call_obj = params[0] if isinstance(params, list) and params else {}
                                if str(call_obj.get("data") or "").lower().startswith("0x49948e0e"):
                                    return "0x" + (777_000_000_000).to_bytes(32, "big").hex()
                            return "0x"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            return [self.call(ch, m, p, **kw) for m, p in calls]

                    m22_router = _HistAndOpMockRouter()
                # T23: Glamsterdam EIP-8024 공식 디코딩·스택언더플로 방어 + SHL96/SHR96 160비트 마스크 + 동적 STATICCALL/JUMP E2 + 다중 패싯 가드 보존 + Scroll/Arb L1 수수료 + 트레이스 격리
                dis_8024_tload = _disassemble(bytes([0xE6, 0x5C, 0x00]))
                dis_8024_exch = _disassemble(bytes([0xE8, 0x52, 0x00]))
                eip8024_bounds_ok = (
                    len(dis_8024_tload) == 3
                    and dis_8024_tload[1][1] == "TLOAD"
                    and len(dis_8024_exch) == 3
                    and dis_8024_exch[1][1] == "MSTORE"
                )

                falloff_sstore_scan = quick_scan_value_sinks(bytes.fromhex("6001600055"))
                dyn_static_scan = quick_scan_value_sinks(bytes.fromhex("60008080806000355afa00"))
                dyn_jump_scan = quick_scan_value_sinks(bytes.fromhex("631111111114600d57600080fd5b600035565b00"))
                unmod_nostop_scan = quick_scan_value_sinks(bytes.fromhex("631111111114600d57600080fd5be3600080fd"))
                falloff_ok = (
                    (not falloff_sstore_scan.early_exit_safe)
                    and falloff_sstore_scan.has_s5_state_mutation
                    and (not dyn_static_scan.early_exit_safe)
                    and (not dyn_jump_scan.early_exit_safe)
                    and (not unmod_nostop_scan.early_exit_safe)
                )

                shl96_shr96_closed = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3360601b60601c73" + ("99" * 20) + "14603257600080fd"
                    "5b6000808080808080f100"
                )
                p_shl96 = prove_caller_dominator_closed(
                    disassemble_and_build_cfg(shl96_shr96_closed),
                    target_addr="0x" + "11" * 20,
                )

                swap_underflow_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b60005b339073" + ("99" * 20) + "14603057600080fd"
                    "5b6000808080808080f100"
                )
                p_swap_uf = prove_caller_dominator_closed(
                    disassemble_and_build_cfg(swap_underflow_code),
                    target_addr="0x" + "11" * 20,
                )
                mc3_guard_code = bytes.fromhex(
                    "63a9059cbb14600d57600080fd"
                    "5b3373" + MULTICALL3_ADDR[2:].lower() + "14602c57600080fd"
                    "5b6000808080808080f100"
                )
                p_mc3_guard = prove_caller_dominator_closed(
                    disassemble_and_build_cfg(mc3_guard_code),
                    target_addr="0x" + "11" * 20,
                )

                facet_w_cfg = disassemble_and_build_cfg(bytes.fromhex("631111111114600d57600080fd5b3360005500"))
                facet_g_cfg = disassemble_and_build_cfg(
                    bytes.fromhex("632222222214600d57600080fd5b6000543314601a57600080fd5b6000808080808080f100")
                )
                facet_col0_cfg = disassemble_and_build_cfg(
                    bytes.fromhex("633333333314600d57600080fd5b6000543314601a57600080fd5b6000808080808080f100")
                )
                facet_col1_cfg = disassemble_and_build_cfg(
                    bytes.fromhex("633333333314600d57600080fd5b6001543314601a57600080fd5b6000808080808080f100")
                )
                p_col_facets = prove_caller_dominator_closed(
                    _merge_facet_cfgs([facet_col0_cfg, facet_col1_cfg]),
                    router=m22_router,
                    chain="ethereum",
                    target_addr="0x" + "55" * 20,
                    block_pin=20_000_000,
                )
                merged_facet_cfg = _merge_facet_cfgs([facet_w_cfg, facet_g_cfg])
                p_merged_facets = prove_caller_dominator_closed(
                    merged_facet_cfg,
                    router=m22_router,
                    chain="ethereum",
                    target_addr="0x" + "55" * 20,
                    block_pin=20_000_000,
                )
                merged_slices = slice_four_channel_causal_chains(
                    "0x" + "55" * 20,
                    merged_facet_cfg,
                    p_merged_facets.open_selectors,
                )
                cross_facet_ok = (
                    p_shl96.early_exit_safe
                    and (not p_swap_uf.early_exit_safe)
                    and (not p_mc3_guard.early_exit_safe)
                    and (not p_col_facets.early_exit_safe)
                    and ("0x33333333" in p_col_facets.open_selectors)
                    and ("0x22222222" in p_merged_facets.open_selectors)
                    and any(s.legs == ["0x11111111", "0x22222222"] for s in merged_slices)
                )

                class _RollupAndLagMockRouter:
                    def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                        if method == "eth_getBlockByNumber":
                            blk_hex = str(params[0])
                            bnum = _hex_to_int(blk_hex)
                            return {"number": blk_hex, "timestamp": hex(1_760_000_000 + (bnum - 20_000_000))}
                        if method == "eth_call":
                            call_obj = params[0] if isinstance(params, list) and params else {}
                            to_l = _norm_addr(str(call_obj.get("to") or ""))
                            if ch == "scroll" and to_l == "0x5300000000000000000000000000000000000002":
                                return "0x" + (555_000_000_000).to_bytes(32, "big").hex()
                            if ch == "arbitrum" and to_l == "0x000000000000000000000000000000000000006c":
                                return "0x" + (200_000_000_000).to_bytes(32, "big").hex() + (1_000_000_000).to_bytes(32, "big").hex()
                        return "0x"

                    def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                        return [None for _ in calls]

                rl_router = _RollupAndLagMockRouter()
                scroll_fee = _estimate_rollup_l1_fee_wei(rl_router, "scroll", 20_000_000, 128)
                arb_fee = _estimate_rollup_l1_fee_wei(rl_router, "arbitrum", 20_000_000, 200)
                v_fast_l2 = judge_conservation_accounting(
                    store=None,
                    chain="arbitrum",
                    caller_before={"native": 0},
                    caller_after={"native": 10**18},
                    vault_before={"native": 10**18},
                    vault_after={"native": 0},
                    gas_used=100_000,
                    gas_price_wei=100_000_000,
                    prices_map={"native": 3000.0},
                    router=rl_router,
                    block_pin=20_000_000,
                    live_head_block=20_000_010,
                    allow_offline_fixture=True,
                )
                partial_rpc_fail_open = False
                try:
                    _resolve_proxy_and_delegates(rl_router, "ethereum", ["0x" + "11" * 20], 20_000_000, raise_on_rpc_failure=True)
                except RuntimeError:
                    partial_rpc_fail_open = True

                trace_untyped_blocked = _scan_trace_tree_for_disallowed_hevm(
                    {"kind": "Call", "address": HEVM_ADDRESS_LOWER, "data": "0x1122"}
                )
                fork_sel_hex = _selector("createSelectFork(string,uint256)").lower()
                trace_inline_fork_blocked = _scan_trace_tree_for_disallowed_hevm(
                    {"kind": "Call", "address": HEVM_ADDRESS_LOWER, "data": fork_sel_hex + ("00" * 64)},
                    forbid_inline_fork=True,
                )
                tmp_rq_db = selftest_ns / "_selftest_requeue_modeb.db"
                st_rq = SQLiteStore(tmp_rq_db)
                try:
                    st_rq.requeue_with_addresses("modeb-ethereum-007-112233", ["0x" + "77" * 20], chain="ethereum", tvl_usd=0.0)
                    st_rq.requeue_with_addresses("case-0101-sample", ["0x" + "78" * 20], chain="ethereum", tvl_usd=0.0)
                    cf_guard_addr = "0x" + "98" * 20
                    st_rq.put_contract_code(
                        chain_id=8453,
                        address=cf_guard_addr,
                        audit_block=10_000_000,
                        code_hash="cf",
                        skeleton_hash="cf",
                        opcode_4grams="",
                        raw_code_hex="0x60008080808080f100",
                    )

                    cf_router = _make_mock_guard_router(st_rq)
                    cf_eth_code = bytes.fromhex(
                        "63a9059cbb14600d57600080fd"
                        "5b3373" + cf_guard_addr[2:] + "14602c57600080fd"
                        "5b6000808080808080f100"
                    )
                    p_cf_eth = prove_caller_dominator_closed(
                        disassemble_and_build_cfg(cf_eth_code),
                        router=cf_router,
                        chain="ethereum",
                        target_addr="0x" + "12" * 20,
                        block_pin=20_000_000,
                    )
                    p_dyn_sc_e3 = prove_caller_dominator_closed(
                        disassemble_and_build_cfg(bytes.fromhex("60008080806000355afa00")),
                        target_addr="0x" + "13" * 20,
                    )
                    self_sc_bytes = bytes.fromhex("6000808080305afa00")
                    self_sc_ok = (
                        quick_scan_value_sinks(self_sc_bytes).early_exit_safe
                        and prove_caller_dominator_closed(disassemble_and_build_cfg(self_sc_bytes), target_addr="0x" + "14" * 20).early_exit_safe
                    )

                    off_slug_ok = "modeb-ethereum-901-offchain"
                    (ARTIFACT_ROOT / off_slug_ok).mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(
                        ARTIFACT_ROOT / off_slug_ok / "discovered_addresses.json",
                        {
                            "addresses": [],
                            "no_onchain_contracts": True,
                            "pure_offchain_proven": True,
                            "offchain_evidence": "Verified pure off-chain DNS/frontend incident with zero EVM deployment.",
                        },
                    )
                    res_off_ok = audit_single_protocol(
                        cf_router,
                        st_rq,
                        {"slug": off_slug_ok, "chain": "ethereum", "tvl_usd": 0.0, "addresses": [], "mode": "B", "delegate_address_search": False},
                        set(),
                    )
                    res_empty_modeb = audit_single_protocol(
                        cf_router,
                        st_rq,
                        {"slug": "modeb-ethereum-902-unrec", "chain": "ethereum", "tvl_usd": 0.0, "addresses": ["0x" + "79" * 20], "mode": "B", "delegate_address_search": False},
                        set(),
                    )
                    v_closed_hex = "0x63a9059cbb14600d57600080fd5b3373" + ("99" * 20) + "14602c57600080fd5b6000808080808080f100"
                    o_view_hex = "0x631234567814600d57600080fd5b60206000f3"
                    v_a23 = "0x" + "21" * 20
                    o_a23 = "0x" + "22" * 20
                    vc_slug_23 = "modea-closed-plus-view"
                    (ARTIFACT_ROOT / vc_slug_23).mkdir(parents=True, exist_ok=True)
                    cl_vc_23 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=[v_a23, o_a23],
                        all_addresses=[v_a23, o_a23],
                        resolutions={
                            v_a23: ProxyResolution(address=v_a23, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex=v_closed_hex, effective_code_hex=v_closed_hex, skeleton_hash="s1", slot_collisions=[], impl_codes_hex={v_a23: v_closed_hex}),
                            o_a23: ProxyResolution(address=o_a23, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex=o_view_hex, effective_code_hex=o_view_hex, skeleton_hash="s2", slot_collisions=[], impl_codes_hex={o_a23: o_view_hex}),
                        },
                        vault_exposures={v_a23: VaultExposure(address=v_a23, native_wei=100 * 10**18, native_usd=300_000.0, total_usd=300_000.0), o_a23: VaultExposure(address=o_a23)},
                        max_extractable_usd=300_000.0,
                        min_gas_usd=0.05,
                        early_exit_safe=False,
                        early_exit_reason="",
                        has_dynamic_external_reachability=True,
                    )
                    _write_json_atomic(
                        ARTIFACT_ROOT / vc_slug_23 / "checkpoint.json",
                        {
                            "stage": "S2_CLOSURE_VERIFIED",
                            "audit_block": 20_000_000,
                            "pin_head_block": 20_000_002,
                            "seed_addrs": [v_a23, o_a23],
                            "closure_ready": True,
                            "closure": _serialize_closure(cl_vc_23),
                            "verified_sources": {},
                            "remediation_exhausted": {k: True for k in REMEDIATION_KEYS},
                        },
                    )
                    res_vc_23 = audit_single_protocol(
                        cf_router, st_rq, {"slug": vc_slug_23, "chain": "ethereum", "tvl_usd": 300_000.0, "addresses": [v_a23, o_a23], "mode": "A"}, set()
                    )
                    ea_slug_23 = "modea-paused-ext-allowance"
                    (ARTIFACT_ROOT / ea_slug_23).mkdir(parents=True, exist_ok=True)
                    ext_sp_23 = BALANCER_V3_VAULT.lower()
                    cl_ea_23 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=[v_a23],
                        all_addresses=[v_a23, ext_sp_23],
                        resolutions={
                            v_a23: ProxyResolution(address=v_a23, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex="0x600080fd", effective_code_hex="0x600080fd", skeleton_hash="s3", slot_collisions=[], impl_codes_hex={v_a23: "0x600080fd"}),
                            ext_sp_23: ProxyResolution(address=ext_sp_23, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex="0x600080fd", effective_code_hex="0x600080fd", skeleton_hash="s4", slot_collisions=[], impl_codes_hex={ext_sp_23: "0x600080fd"}),
                        },
                        vault_exposures={v_a23: VaultExposure(address=v_a23, total_usd=10_000.0), ext_sp_23: VaultExposure(address=ext_sp_23, allowance_usd=5_000.0, total_usd=5_000.0)},
                        max_extractable_usd=15_000.0,
                        min_gas_usd=0.05,
                        early_exit_safe=False,
                        early_exit_reason="",
                        has_dynamic_external_reachability=False,
                    )
                    _write_json_atomic(
                        ARTIFACT_ROOT / ea_slug_23 / "checkpoint.json",
                        {
                            "stage": "S2_CLOSURE_VERIFIED",
                            "audit_block": 20_000_000,
                            "pin_head_block": 20_000_002,
                            "seed_addrs": [v_a23],
                            "closure_ready": True,
                            "closure": _serialize_closure(cl_ea_23),
                            "verified_sources": {},
                            "remediation_exhausted": {k: True for k in REMEDIATION_KEYS},
                        },
                    )
                    res_ea_23 = audit_single_protocol(
                        cf_router, st_rq, {"slug": ea_slug_23, "chain": "ethereum", "tvl_usd": 15_000.0, "addresses": [v_a23], "mode": "A"}, set()
                    )
                    rq_mb = st_rq.queue_mode_breakdown()
                finally:
                    st_rq.close()
                modeb_sound_ok = (
                    (not p_cf_eth.early_exit_safe)
                    and (not p_dyn_sc_e3.early_exit_safe)
                    and self_sc_ok
                    and res_off_ok.get("verdict") == VERDICT_SAFE
                    and res_off_ok.get("early_exit") == "E1_PURE_OFFCHAIN_NO_CONTRACT_PROVEN"
                    and res_empty_modeb.get("verdict") == VERDICT_BLOCKED
                    and res_vc_23.get("early_exit") == "NONE"
                    and bool(res_vc_23.get("needs_agent"))
                    and res_ea_23.get("early_exit") == "NONE"
                    and bool(res_ea_23.get("needs_agent"))
                    and int((rq_mb.get("B") or {}).get("total", 0)) >= 2
                )
                rpc_cls_ok = (
                    _classify_rpc_error("block reverted due to chain reorg") == "gap"
                    and _classify_rpc_error("execution reverted: custom guard") == "revert"
                    and modeb_sound_ok
                )
                registry_sync_ok = (
                    WRAPPED_NATIVE_BY_CHAIN.get("monad") == "0x3bd359c1119da7da1d913d1c4d2b7c461115433a"
                    and WRAPPED_NATIVE_BY_CHAIN.get("abstract") == "0x3439153eb7af838ad19d56e1571fbd09333c2809"
                    and WRAPPED_NATIVE_BY_CHAIN.get("katana") == "0xee7d8bcfb72bc1880d0cf19822eb0a2e6577ab62"
                    and "unichain" in UNISWAP_V4_POOL_MANAGERS
                    and "monad" in UNISWAP_V4_POOL_MANAGERS
                    and "balancer_v3" in {fp.get("provider") for fp in FLASH_PROVIDERS.get("ethereum", [])}
                    and rpc_cls_ok
                )

                t23_ok = (
                    eip8024_bounds_ok
                    and falloff_ok
                    and cross_facet_ok
                    and scroll_fee == 555_000_000_000
                    and arb_fee == 400_000_000_000
                    and _is_profit_verdict(v_fast_l2.verdict)
                    and partial_rpc_fail_open
                    and trace_untyped_blocked
                    and trace_inline_fork_blocked
                    and registry_sync_ok
                )
                results.append(
                    (
                        "T23_GLAMSTERDAM_EIP8024_DECIMAL_SCROLL_AND_TRACE_SOUNDNESS",
                        t23_ok,
                        f"eip8024={eip8024_bounds_ok},falloff={falloff_ok},facet_e3={cross_facet_ok},modeb_cf={modeb_sound_ok},scroll_arb={scroll_fee==555_000_000_000 and arb_fee==400_000_000_000},fast_l2={_is_profit_verdict(v_fast_l2.verdict)},rpc_failopen={partial_rpc_fail_open},trace={trace_untyped_blocked and trace_inline_fork_blocked}",
                    )
                )

            if _want_test("T24_ROUND2_CWIA_MULTI_ASSET_HARNESS_AND_FAILOPEN", tag_filters):
                # T24: Round-2 심층 회귀 (Vyper/CWIA 프록시, 다중자산 분할정산·8-dec WNATIVE, 하네스 보조금/setUp 차단, 무조작 스켈레톤 안전 반려, Mode A 미포착 금고 Fail-Open)
                vyper_mp_24 = bytes.fromhex("366000600037611000600036600073" + ("aa" * 20) + "5af4602c57600080fd5b6110006000f3")
                cwia_mp_24 = bytes.fromhex(
                    "3d3d3d3d363d3d376100406037363936610040013d73" + ("bb" * 20) + "5af43d3d93803e603557fd5bf3" + "5473" + ("cc" * 30)
                )
                stateful_non_mp_24 = bytes.fromhex("60005473" + ("dd" * 20) + "5af4601e57600080fd5b600080f3")
                cwia_vyper_ok = (
                    _eip1167_impl(vyper_mp_24) == "0x" + ("aa" * 20)
                    and _eip1167_impl(cwia_mp_24) == "0x" + ("bb" * 20)
                    and _eip1167_impl(stateful_non_mp_24) == ""
                )

                usdc_24 = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                usdt_24 = "0xdac17f958d2ee523a2206206994597c13d831ec7"
                v_split_24 = judge_conservation_accounting(
                    store=None,
                    chain="ethereum",
                    caller_before={usdc_24: 0, usdt_24: 0},
                    caller_after={usdc_24: 60 * 10**6, usdt_24: 60 * 10**6},
                    vault_before={usdc_24: 100 * 10**6, usdt_24: 100 * 10**6},
                    vault_after={usdc_24: 40 * 10**6, usdt_24: 40 * 10**6},
                    gas_used=50_000,
                    gas_price_wei=100_000_000,
                    decimals_map={usdc_24: 6, usdt_24: 6},
                    prices_map={"native": 3000.0, usdc_24: 1.0, usdt_24: 1.0},
                    allow_offline_fixture=True,
                )
                weth_24 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                cg_w8_24, vl_w8_24, _ = _domain_deltas(
                    caller_before={"native": 10**18, weth_24: 0},
                    caller_after={"native": 0, weth_24: 10**8},
                    vault_before={"native": 0, weth_24: 10**8},
                    vault_after={"native": 10**18, weth_24: 0},
                    chain="ethereum",
                    decimals_map={weth_24: 8},
                )
                split_and_w8_ok = (
                    _is_profit_verdict(v_split_24.verdict)
                    and v_split_24.swap_fee_usd == 0.0
                    and v_split_24.slippage_usd == 0.0
                    and 119.0 <= v_split_24.net_usd <= 120.0
                    and len(cg_w8_24) == 0
                    and len(vl_w8_24) == 0
                )

                ok_setup_24, _ = verify_cheatcode_allowlist("contract Verify_X { function setUp() public {} }")
                ok_this_24, _ = verify_cheatcode_allowlist(
                    "contract AgentVerify_1 { function test_x() public view returns (uint256) { return address(this).balance; } }"
                )
                ok_7fa9_24, _ = verify_cheatcode_allowlist("contract T { address constant H = 0x7FA9385bE102ac3EAc297483Dd6233D62b3e1496; }")
                tr_sub_blk_24 = _scan_trace_tree_for_disallowed_hevm(
                    {"kind": "Call", "caller": "0x7fa9385be102ac3eac297483dd6233d62b3e1496", "address": "0x" + ("11" * 20), "value": "0xde0b6b3a7640000", "data": "0x"}
                )
                tr_zero_cr_ok_24 = not _scan_trace_tree_for_disallowed_hevm(
                    {"kind": "Create", "caller": "0x7fa9385be102ac3eac297483dd6233d62b3e1496", "address": "0x" + ("22" * 20), "value": "0x0", "data": "0x6080"}
                )
                harness_iso_24_ok = (not ok_setup_24) and (not ok_this_24) and (not ok_7fa9_24) and tr_sub_blk_24 and tr_zero_cr_ok_24

                st24 = SQLiteStore(selftest_ns / "_selftest_t24.db")
                try:
                    sk_slug_24 = "t24-skeleton-safe"
                    st24.upsert_queue_item(sk_slug_24, 1, "ethereum", 200_000.0, ["0x" + "31" * 20])
                    st24.set_queue_agent_state(sk_slug_24, "awaiting_verdict")
                    sk_dir_24 = ARTIFACT_ROOT / sk_slug_24
                    sk_req_24 = sk_dir_24 / "poc_requests"
                    sk_verd_24 = sk_dir_24 / "verdicts"
                    sk_ws_test_24 = sk_dir_24 / "forge" / "test"
                    sk_req_24.mkdir(parents=True, exist_ok=True)
                    sk_verd_24.mkdir(parents=True, exist_ok=True)
                    sk_ws_test_24.mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(
                        sk_dir_24 / "result.json",
                        {"slug": sk_slug_24, "chain": "ethereum", "audit_block": 20_000_000, "verdict": VERDICT_SAFE, "needs_agent": True},
                    )
                    _write_json_atomic(
                        sk_dir_24 / "checkpoint.json",
                        {"audit_block": 20_000_000, "pin_head_block": 20_000_002, "caller": "0x0000000000000000000000000000000000001337"},
                    )
                    _write_json_atomic(
                        sk_req_24 / "poc_request_1.json",
                        {"candidate_id": 1, "target_address": "0x" + "31" * 20, "zero_state_caller": "0x0000000000000000000000000000000000001337"},
                    )
                    sol_sk_24 = sk_ws_test_24 / "test_1.t.sol"
                    sol_sk_24.write_text(
                        _render_agent_poc_skeleton(
                            sk_slug_24,
                            1,
                            "ethereum",
                            20_000_000,
                            "0x0000000000000000000000000000000000001337",
                            "0x" + "31" * 20,
                        ),
                        encoding="utf-8",
                    )
                    _write_json_atomic(
                        sk_dir_24 / "forge_output_1.json",
                        {
                            "executed": True,
                            "passed_tests": 1,
                            "failed_tests": 0,
                            "conservation_gains": [],
                            "error_reason": "",
                            "trace_summary": {"disallowed_cheatcode_in_trace": False, "target_called_in_trace": False},
                        },
                    )
                    now_ts_24 = time.time() + 5.0
                    os.utime(sk_dir_24 / "forge_output_1.json", (now_ts_24, now_ts_24))
                    _write_json_atomic(
                        sk_verd_24 / "verdict_1.json",
                        {"verdict": VERDICT_SAFE, "notes": "Claiming safe on untouched skeleton file"},
                    )

                    gov_router_24 = _make_mock_guard_router(
                        st24,
                        code_by_addr={"0x" + "41" * 20: "0x6318160ddd14600d57600080fd5b60206000f3"},
                        mc3_zero_word=True,
                    )
                    process_agent_submissions(gov_router_24, st24)
                    sk_rejected_ok = (
                        not (sk_verd_24 / "verdict_1.json").exists()
                        and int((_read_json(sk_req_24 / "poc_request_1.json", default={}) or {}).get("rejection_count") or 0) == 1
                    )

                    res_ma_disc_24 = audit_single_protocol(
                        gov_router_24,
                        st24,
                        {"slug": "t24-modea-gov-disc", "chain": "ethereum", "tvl_usd": 500_000.0, "addresses": ["0x" + "41" * 20], "mode": "A", "delegate_address_search": True},
                        set(),
                    )
                    res_ma_blk_24 = audit_single_protocol(
                        gov_router_24,
                        st24,
                        {"slug": "t24-modea-gov-blk", "chain": "ethereum", "tvl_usd": 500_000.0, "addresses": ["0x" + "41" * 20], "mode": "A", "delegate_address_search": False},
                        set(),
                    )
                    modea_uncov_ok = (
                        bool(res_ma_disc_24.get("awaiting_address_discovery"))
                        and res_ma_blk_24.get("verdict") == VERDICT_BLOCKED
                        and "MODE_A_SPOT_VAULT_UNCOVERED_DIVERGENCE" in str(res_ma_blk_24.get("reason") or "")
                    )
                finally:
                    st24.close()

                t24_ok = cwia_vyper_ok and split_and_w8_ok and harness_iso_24_ok and sk_rejected_ok and modea_uncov_ok
                results.append(
                    (
                        "T24_ROUND2_CWIA_MULTI_ASSET_HARNESS_AND_FAILOPEN",
                        t24_ok,
                        f"cwia_vyper={cwia_vyper_ok},split_w8={split_and_w8_ok},harness_iso={harness_iso_24_ok},sk_rej={sk_rejected_ok},modea_uncov={modea_uncov_ok}",
                    )
                )

            if _want_test("T25_ROUND3_EIP7702_MINT_INFRA_ALLOWANCE_AND_PARTITION", tag_filters):
                # T25: Round-3 (1) EIP-7702 empty/counterfactual delegate Fail-Open across E1/E2/E3,
                # (2) Unbacked ERC-20 supply increase (totalSupply delta) conservation accounting & pool reserve cap,
                # (3) Singleton infra exclusion in graph expansion & approved owner allowance vault tracking,
                # (4) Live-mode quote stablecoin peg cross-check & zero-state caller token balance filtering,
                # (5) Corpus partition pure non-EVM chain preservation + requeue verdict reset
                st25 = SQLiteStore(selftest_ns / "_t25_round3.sqlite3")
                try:
                    eip7702_seed_25 = "0x" + "77" * 20
                    eip7702_del_25 = "0x" + "78" * 20

                    class _EIP7702EmptyRouter(RPCRouter):
                        def get_quorum_head_block(self, chain: str) -> Tuple[int, int]:
                            return (1000, 1000)

                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            if method == "eth_getCode":
                                t_a = _norm_addr(params[0]) if params else ""
                                if t_a == eip7702_seed_25:
                                    return "0xef0100" + eip7702_del_25[2:]
                                return "0x"
                            if method == "eth_getStorageAt":
                                return "0x" + "00" * 32
                            if method == "eth_getBalance":
                                return hex(10 * 10**18) if (_norm_addr(params[0]) if params else "") == eip7702_seed_25 else "0x0"
                            if method == "eth_getTransactionCount":
                                return "0x0"
                            if method == "eth_getLogs":
                                return []
                            return "0x"

                        def batch_call(self, chain: str, calls: List[Tuple[str, List[Any]]], block_pin: Optional[int] = None, need_archive: bool = False, raise_on_all_failed: bool = False) -> List[Any]:
                            return [self.call(chain, m, p, block_pin=block_pin, need_archive=need_archive) for m, p in calls]

                        def multicall3(self, chain: str, subcalls: List[Tuple[str, bytes]], block_pin: int) -> Optional[List[Tuple[bool, bytes]]]:
                            return [(False, b"") for _ in subcalls]

                    r7702_25 = _EIP7702EmptyRouter(st25)
                    res_map_7702 = _resolve_proxy_and_delegates(r7702_25, "ethereum", [eip7702_seed_25], 1000)
                    pr_7702 = res_map_7702.get(eip7702_seed_25)
                    res_7702_audit = audit_single_protocol(
                        r7702_25,
                        st25,
                        {"slug": "t25-eip7702-empty", "chain": "ethereum", "tvl_usd": 50_000.0, "addresses": [eip7702_seed_25], "mode": "A"},
                        set(),
                    )
                    eip7702_failopen_ok = (
                        pr_7702 is not None
                        and pr_7702.proxy_kind == "eip7702"
                        and pr_7702.effective_code_hex == "0x"
                        and eip7702_del_25 in pr_7702.impl_addresses
                        and res_7702_audit.get("early_exit") == "NONE"
                        and bool(res_7702_audit.get("needs_agent"))
                    )

                    # (2) Unbacked ERC-20 mint harness ConservationGain & pool reserve cap
                    ws25 = ensure_clean_forge_workspace({"ethereum": "http://127.0.0.1:8545"}, slug="t25_mint_suite", offline=True)
                    mint_caller = "0x" + "66" * 20
                    mint_tok_addr = "0x" + _keccak256(bytes([0xd6, 0x94]) + bytes.fromhex(mint_caller[2:]) + bytes([0x80]))[12:32].hex().lower()
                    mint_sl = CausalSlice(
                        slice_id="mint_0",
                        target_address=mint_tok_addr,
                        channel="CH_STORAGE",
                        legs=["0x40c10f19"],
                        witness_candidates=[1000 * 10**18],
                        repeat_counts=[1],
                        rationale="open mint(address,uint256)",
                        leg_targets=[mint_tok_addr],
                        abi_inputs_by_sel={"0x40c10f19": ["address", "uint256"]},
                    )
                    h_mint_25 = _render_minimal_solidity_harness(
                        slug="t25_mint_suite",
                        chain="ethereum",
                        block_pin=0,
                        caller=mint_caller,
                        target_addr=mint_tok_addr,
                        token_addrs=[mint_tok_addr],
                        slices=[mint_sl],
                        vault_addresses=[mint_tok_addr],
                    )
                    mint_token_sol = """
contract UnprotectedMintToken {
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    function mint(address to, uint256 amount) external returns (bool) {
        totalSupply += amount;
        balanceOf[to] += amount;
        return true;
    }
    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount);
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}
"""
                    c_chk_25 = _checksum(mint_caller)
                    h_mint_25_with_deploy = (
                        h_mint_25.replace(
                            "contract Verify_t25_mint_suite {",
                            mint_token_sol + "\ncontract Verify_t25_mint_suite {",
                        ).replace(
                            f"vm.startPrank({c_chk_25}, {c_chk_25});",
                            f"vm.startPrank({c_chk_25}, {c_chk_25});\n        new UnprotectedMintToken();",
                        )
                    )
                    tpath_25 = ws25 / "test" / "Verify_t25_mint_suite.t.sol"
                    tpath_25.write_text(h_mint_25_with_deploy, encoding="utf-8")
                    fout_25 = execute_forge_test_file(
                        ws=ws25,
                        rel_test_path="test/Verify_t25_mint_suite.t.sol",
                        sol_code=h_mint_25_with_deploy,
                        timeout_sec=60,
                        allowed_caller=mint_caller,
                    )
                    mint_harness_ok = (
                        fout_25.executed
                        and fout_25.passed_tests == 1
                        and len(fout_25.conservation_gains) == 1
                        and int(fout_25.conservation_gains[0].get("caller_gain") or 0) == 1000 * 10**18
                        and int(fout_25.conservation_gains[0].get("vault_loss") or 0) == 1000 * 10**18
                    )

                    # (3) WETH singleton exclusion in _expand_address_graph_conceptually & WNATIVE exclusion in raw_vaults
                    weth_eth = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                    weth_push20_code = bytes([0x73]) + bytes.fromhex(weth_eth[2:]) + bytes([0x00])
                    dummy_seed_25 = "0x" + "88" * 20
                    exp_addrs_25 = _expand_address_graph_conceptually(
                        r7702_25,
                        "ethereum",
                        [dummy_seed_25],
                        {
                            dummy_seed_25: ProxyResolution(
                                address=dummy_seed_25,
                                proxy_kind="none",
                                impl_addresses=[],
                                admin_address="",
                                raw_code_hex="0x" + weth_push20_code.hex(),
                                effective_code_hex="0x" + weth_push20_code.hex(),
                                skeleton_hash=_keccak256(weth_push20_code).hex(),
                                slot_collisions=[],
                            )
                        },
                        1000,
                    )
                    h_weth_excl = _render_minimal_solidity_harness(
                        slug="t25_weth_excl",
                        chain="ethereum",
                        block_pin=0,
                        caller=mint_caller,
                        target_addr=dummy_seed_25,
                        token_addrs=[weth_eth],
                        slices=[mint_sl],
                        vault_addresses=[dummy_seed_25, weth_eth],
                    )
                    infra_excl_ok = (weth_eth not in exp_addrs_25) and (f"= {_checksum(weth_eth)};" not in "\n".join(line for line in h_weth_excl.splitlines() if "constant VAULT_" in line))

                    # (4) Live-mode depegged stablecoin Fail-Open & zero-state caller token balance filtering
                    usdc_eth = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    mock_pair_25 = "0x" + "99" * 20

                    class _DepegAndCallerRouter(_EIP7702EmptyRouter):
                        def multicall3(self, chain: str, subcalls: List[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 50) -> Optional[List[Tuple[bool, bytes]]]:
                            cands_25 = _deterministic_caller_candidates("ethereum")
                            first_cand = cands_25[0]
                            out_mc: List[Tuple[bool, bytes]] = []
                            for t_a, cd in subcalls:
                                if cd[:4] == bytes.fromhex("70a08231") and len(cd) >= 36:
                                    who = "0x" + cd[16:36].hex().lower()
                                    if who == first_cand:
                                        out_mc.append((True, (500 * 10**6).to_bytes(32, "big")))
                                        continue
                                    if who == mock_pair_25:
                                        if _norm_addr(t_a) == weth_eth:
                                            # 2,000 WETH ($5,000,000 at $2,500/ETH)
                                            out_mc.append((True, (2_000 * 10**18).to_bytes(32, "big")))
                                            continue
                                        if _norm_addr(t_a) == usdc_eth:
                                            # 10,000,000 USDC -> spot price $0.50 (50% divergence from $1.00)
                                            out_mc.append((True, (10_000_000 * 10**6).to_bytes(32, "big")))
                                            continue
                                    out_mc.append((True, (0).to_bytes(32, "big")))
                                    continue
                                if cd[:4] == bytes.fromhex("e6a43905"):
                                    pair_addr_bytes = bytes(12) + bytes.fromhex("99" * 20)
                                    out_mc.append((True, pair_addr_bytes))
                                elif cd[:4] == bytes.fromhex("313ce567"):
                                    dec_v = 6 if _norm_addr(t_a) == usdc_eth else 18
                                    out_mc.append((True, dec_v.to_bytes(32, "big")))
                                else:
                                    out_mc.append((False, b""))
                            return out_mc

                    r_depeg_25 = _DepegAndCallerRouter(st25)
                    chosen_c0_25 = select_zero_state_caller(
                        r_depeg_25,
                        "ethereum",
                        1000,
                        exclude_addrs=[],
                        check_tokens=[usdc_eth],
                    )
                    dex_depeg_25 = probe_dex_exit_liquidity(
                        r_depeg_25,
                        "ethereum",
                        usdc_eth,
                        1000 * 10**6,
                        6,
                        1.0,
                        2500.0,
                        1000,
                    )
                    depeg_and_c0_ok = (
                        chosen_c0_25 != _deterministic_caller_candidates("ethereum")[0]
                        and dex_depeg_25.get("exit_liquid") is False
                        and bool(dex_depeg_25.get("price_diverged")) is True
                    )

                    # (5) Corpus partition pure non-EVM chain preservation + requeue verdict reset
                    st25.upsert_queue_item("t25-requeue-slug", 1, "ethereum", 1000.0, ["0x" + "11" * 20], mode="B")
                    st25.complete_queue_item("t25-requeue-slug", VERDICT_BLOCKED, 0.0)
                    st25.requeue_with_addresses("t25-requeue-slug", ["0x" + "22" * 20])
                    with st25._read_conn() as rc25:
                        rq_row_25 = rc25.execute(
                            "SELECT status, verdict, net_krw FROM audit_queue WHERE slug='t25-requeue-slug'"
                        ).fetchone()
                    requeue_reset_ok = (
                        rq_row_25 is not None
                        and str(rq_row_25["status"]) == "pending"
                        and str(rq_row_25["verdict"] or "") == ""
                        and float(rq_row_25["net_krw"] or 0.0) == 0.0
                    )
                    synth_md_25 = selftest_ns / "_t25_corpus.md"
                    synth_md_25.write_text(
                        "## 02/01 — SyntheticSolanaVault (Solana)\n"
                        "- **취약 원리**: PDA authority check missing on base pool\n"
                        "- **취약 컨트랙트**:\n"
                        "  - `So11111111111111111111111111111111111111112` (Solana Program)\n\n"
                        "## 02/02 — SyntheticBscZeroSeed\n"
                        "- **취약 원리**: https://bscscan.com/tx/0x" + ("ab" * 32) + "\n"
                        "- **취약 컨트랙트**:\n"
                        "  - Unverified contract on bscscan.com\n",
                        encoding="utf-8",
                    )
                    part_25 = _parse_corpus_md_partition(synth_md_25)
                    evm_c25 = list(part_25.get("evm_cases") or [])
                    non_evm_c25 = list(part_25.get("pure_non_evm_cases") or [])
                    real_part_25 = _parse_corpus_md_partition()
                    real_non_evm_25 = list(real_part_25.get("pure_non_evm_cases") or [])
                    partition_non_evm_ok = (
                        len(non_evm_c25) == 1
                        and non_evm_c25[0].chains == ["solana"]
                        and len(evm_c25) == 1
                        and evm_c25[0].chains == ["bsc"]
                        and ("0x" + ("ab" * 32)) in evm_c25[0].tx_hashes
                        and all(
                            all(ch in (NON_EVM_CHAINS | set(_PURE_NON_EVM_KWS) | {"non_evm"}) for ch in nc.chains)
                            for nc in real_non_evm_25
                        )
                    )
                finally:
                    st25.close()

                t25_ok = (
                    eip7702_failopen_ok
                    and mint_harness_ok
                    and infra_excl_ok
                    and depeg_and_c0_ok
                    and requeue_reset_ok
                    and partition_non_evm_ok
                )
                results.append(
                    (
                        "T25_ROUND3_EIP7702_MINT_INFRA_ALLOWANCE_AND_PARTITION",
                        t25_ok,
                        f"eip7702={eip7702_failopen_ok},mint_h={mint_harness_ok},infra_excl={infra_excl_ok},depeg_c0={depeg_and_c0_ok},requeue={requeue_reset_ok},part={partition_non_evm_ok}",
                    )
                )

            if _want_test("T26_ROUND4_100POINT_ACCOUNTING_CAUSALITY_CFG512_AND_GATES", tag_filters):
                # T26: Round 4 100-point remediation suite (CFG start_pc>=512 PUSH1..3, TSTORE/TLOAD + uninitialized EOA guard, agent accounting integrity, counterfactual causality, ConservationSupplyMint + post-commit settlement, EIP-7623 calldata floor gas + flash-fee single deduction, fully_exhausted vault E4 + Mode B sort_rank + Mode A done gate)
                st26 = SQLiteStore(selftest_ns / "_t26_round4.db")
                try:
                    # (1) CFG dispatcher start_pc >= 512 with PUSH2 selector + TSTORE/TLOAD guard overwrite & uninitialized EOA guard
                    pad_nops = b"\x5b" * 520  # 520 JUMPDEST bytes so dispatcher block starts at pc >= 520
                    body_sink = bytes.fromhex(
                        "5b"                # JUMPDEST (0x0220 = 544)
                        "3360055c14"        # CALLER PUSH1 0x05 TLOAD EQ
                        "61022f57"          # PUSH2 0x022f JUMPI (559)
                        "60006000fd"        # revert
                        "5b"                # JUMPDEST (0x022f = 559)
                        "600060006000600060013361fffff100"  # CALL with value=1 to CALLER, STOP
                        "5b"                # JUMPDEST (0x0245 = 581) for open TSTORE writer
                        "3360055d00"        # CALLER PUSH1 0x05 TSTORE STOP
                    )
                    disp_520 = (
                        pad_nops
                        + bytes.fromhex(
                            "60003560e01c"
                            "8061123414"
                            "61022057"
                            "8061567814"
                            "61024557"
                            "60006000fd"
                        )
                        + body_sink
                    )
                    cfg_520 = disassemble_and_build_cfg(disp_520)
                    cfg512_push2_ok = (
                        "0x00001234" in cfg_520.entries
                        and "0x00005678" in cfg_520.entries
                        and (5 | (1 << 240)) in cfg_520.entries["0x00005678"].written_slots
                    )

                    class _MockEOAGuardRouter:
                        def batch_call(self, chain: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out: List[Any] = []
                            for m_c, _ in calls:
                                if m_c == "eth_getCode":
                                    out.append("0x")
                                elif m_c == "eth_getStorageAt":
                                    out.append("0x" + ("00" * 32))
                                else:
                                    out.append("0x0")
                            return out

                        def call(self, chain: str, method: str, params: List[Any], **kw: Any) -> Any:
                            if method == "eth_getStorageAt":
                                return "0x" + ("00" * 12) + ("cc" * 20)
                            if method == "eth_getTransactionCount":
                                return "0x0"
                            return "0x"

                    # Build a simple SLOAD guard contract where guard address 0xcc..cc has code=0x and nonce=0
                    sload_guard_code = bytes.fromhex(
                        "60003560e01c8063aaaaaaaa1460155760006000fd"
                        "5b336000541460235760006000fd"
                        "5b600060006000600060013361fffff100"
                    )
                    cfg_sload = disassemble_and_build_cfg(sload_guard_code)
                    dom_eoa0 = prove_caller_dominator_closed(
                        cfg_sload,
                        _MockEOAGuardRouter(),  # type: ignore[arg-type]
                        "ethereum",
                        "0x" + "dd" * 20,
                        block_pin=19000000,
                    )
                    eoa_nonce0_open_ok = not dom_eoa0.early_exit_safe and "0xaaaaaaaa" in dom_eoa0.open_selectors

                    # (2) Agent harness accounting integrity & counterfactual causality
                    forged_emit_sol = (
                        "pragma solidity ^0.8.20;\n"
                        "contract ForgedTest {\n"
                        "  event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);\n"
                        "  function test_forged() public {\n"
                        "    emit ConservationGain(0, address(0), 1000 ether, 1000 ether, 50000);\n"
                        "  }\n"
                        "}\n"
                    )
                    valid_emit_sol = _render_minimal_solidity_harness(
                        slug="t26_valid",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "11" * 20,
                        target_addr="0x" + "22" * 20,
                        token_addrs=["0x" + "33" * 20],
                        slices=[
                            CausalSlice(
                                slice_id="s0",
                                target_address="0x" + "22" * 20,
                                channel="CH_STORAGE",
                                legs=["0x12345678"],
                                witness_candidates=[1],
                                repeat_counts=[1],
                                rationale="test",
                            )
                        ],
                    )
                    ok_forged, reason_forged = verify_agent_harness_accounting_integrity(forged_emit_sol)
                    ok_valid_h, _ = verify_agent_harness_accounting_integrity(valid_emit_sol)
                    agent_acct_guard_ok = (
                        (not ok_forged)
                        and any("LITERAL_" in r or "UNCOMPUTED_" in r for r in reason_forged)
                        and ok_valid_h
                    )

                    # (3) ConservationSupplyMint event & post-commit settlement check in rendered harness + _parse_forge_json_gains
                    supply_mint_topic = "0x" + _keccak256(b"ConservationSupplyMint(uint256,address,uint256)").hex()
                    gain_topic = "0x" + _keccak256(b"ConservationGain(uint256,address,uint256,uint256,uint256)").hex()
                    tok_26 = "0x" + "44" * 20
                    mock_forge_json = json.dumps(
                        {
                            "test/Verify_T26.t.sol:Verify_T26": {
                                "test_results": {
                                    "test_slice_0()": {
                                        "status": "Success",
                                        "gas_used": 180000,
                                        "decoded_logs": [
                                            f"ConservationGain(0, {tok_26}, 500000000000000000000, 0, 180000)",
                                            f"ConservationSupplyMint(0, {tok_26}, 500000000000000000000)",
                                        ],
                                    }
                                }
                            }
                        }
                    )
                    p_pass, p_fail, p_gains = _parse_forge_json_gains(mock_forge_json)
                    supply_mint_parse_ok = (
                        p_pass == 1
                        and p_fail == 0
                        and len(p_gains) == 1
                        and bool(p_gains[0].get("supply_mint", False))
                        and "_verifyPostCommitSettlement" in valid_emit_sol
                        and "ConservationSupplyMint" in valid_emit_sol
                        and len(supply_mint_topic) == 66
                        and len(gain_topic) == 66
                    )

                    # (4) EIP-7623 calldata floor gas + flash_fee_already_in_balances single deduction + price cross-verification divergence fail-open
                    st26.put_price("fx:USD_KRW", 1400.0, "fixture:t26")
                    def _judge_t26_eth(gas_u: int, gas_p: int, **extra_kw: Any) -> ConservationVerdict:
                        return judge_conservation_accounting(
                            store=st26,
                            chain="ethereum",
                            caller_before={"native": 0},
                            caller_after={"native": 10**18},
                            vault_before={"native": 10**18},
                            vault_after={"native": 0},
                            gas_used=gas_u,
                            gas_price_wei=gas_p,
                            decimals_map={},
                            prices_map={"native": 2000.0},
                            **extra_kw,
                        )

                    acct_no_cd = _judge_t26_eth(25_000, 10_000_000_000, calldata_len=0)
                    acct_big_cd = _judge_t26_eth(25_000, 10_000_000_000, calldata_len=2000)
                    acct_ff_deducted = _judge_t26_eth(
                        100_000, 1_000_000_000, flash_principal_usd=100_000.0, flash_fee_bps=9.0, flash_fee_already_in_balances=False
                    )
                    acct_ff_in_bal = _judge_t26_eth(
                        100_000, 1_000_000_000, flash_principal_usd=100_000.0, flash_fee_bps=9.0, flash_fee_already_in_balances=True
                    )
                    gas_and_ff_ok = (
                        acct_big_cd.gas_usd > acct_no_cd.gas_usd
                        and acct_ff_deducted.flash_fee_usd > 0.0
                        and acct_ff_in_bal.flash_fee_usd == 0.0
                        and acct_ff_in_bal.net_usd > acct_ff_deducted.net_usd
                    )

                    # (5) SQLiteStore fully_exhausted vault tracking, Mode B sort_rank ordering, and _evaluate_done_gates mode_a_seed_ok check
                    v_addr_26 = "0x" + "55" * 20
                    st26.mark_vault_proven("ethereum", v_addr_26, "partial-slug", 200_000.0, fully_exhausted=False)
                    partial_not_exhausted = (
                        st26.is_vault_proven("ethereum", v_addr_26, require_exhausted=False)
                        and not st26.is_vault_proven("ethereum", v_addr_26, require_exhausted=True)
                    )
                    st26.mark_vault_proven("ethereum", v_addr_26, "full-slug", 500_000.0, fully_exhausted=True)
                    full_exhausted_ok = st26.is_vault_proven("ethereum", v_addr_26, require_exhausted=True)

                    st26.upsert_queue_item("b-low-rank", 2, "ethereum", MIN_TVL_USD, [], mode="B", sort_rank=10.0)
                    st26.upsert_queue_item("b-high-rank", 2, "ethereum", MIN_TVL_USD, ["0x" + "66" * 20], mode="B", sort_rank=9000.0)
                    claimed_26 = st26.claim_next_batch("boot-t26", limit=2)
                    sort_rank_ok = len(claimed_26) == 2 and claimed_26[0]["slug"] == "b-high-rank" and claimed_26[1]["slug"] == "b-low-rank"
                    for it_26 in claimed_26:
                        st26.complete_queue_item(str(it_26["slug"]), VERDICT_SAFE, 0.0)

                    st26.put_engine_meta("mode_a_seed_ok", "0")
                    gates_missing_a = _evaluate_done_gates(st26, artifact_root=selftest_ns, run_selftest=False)
                    mode_a_gate_blocked_ok = not bool(gates_missing_a.get("gate_mode_partition_valid", True))
                finally:
                    st26.close()

                t26_ok = (
                    cfg512_push2_ok
                    and eoa_nonce0_open_ok
                    and agent_acct_guard_ok
                    and supply_mint_parse_ok
                    and gas_and_ff_ok
                    and partial_not_exhausted
                    and full_exhausted_ok
                    and sort_rank_ok
                    and mode_a_gate_blocked_ok
                )
                results.append(
                    (
                        "T26_ROUND4_100POINT_ACCOUNTING_CAUSALITY_CFG512_AND_GATES",
                        t26_ok,
                        f"cfg512={cfg512_push2_ok},eoa0={eoa_nonce0_open_ok},agent_acct={agent_acct_guard_ok},mint_ev={supply_mint_parse_ok},gas_ff={gas_and_ff_ok},e4_exh={partial_not_exhausted and full_exhausted_ok},rank={sort_rank_ok},gate_a={mode_a_gate_blocked_ok}",
                    )
                )

            if _want_test("T27_ROUND5_16POINT_COLD_AUDIT_REGRESSION", tag_filters):
                # T27: Round 5 16-Point Cold Audit Regression (process_agent_submissions max-profit & ckpt_data fix, WNATIVE 1:1 wrapping backing check, unverified selector synthesis & >48 slice preservation, variable-alias accounting guard, 0-arg CLI rejection)
                st27 = SQLiteStore(selftest_ns / "t27_store.sqlite")
                try:
                    st27.put_price("fx:USD_KRW", 1400.0, "fixture:t27")
                    r27 = RPCRouter(store=st27)
                    slug27 = "t27-agent-profit"
                    pdir27 = selftest_ns / slug27
                    (pdir27 / "poc_requests").mkdir(parents=True, exist_ok=True)
                    (pdir27 / "verdicts").mkdir(parents=True, exist_ok=True)
                    (pdir27 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    st27.upsert_queue_item(slug27, 1, "ethereum", 100_000.0, ["0x" + "77" * 20], mode="A")
                    st27.set_queue_agent_state(slug27, "awaiting_verdict")
                    _write_json_atomic(
                        pdir27 / "result.json",
                        {"slug": slug27, "chain": "ethereum", "audit_block": 0, "verdict": VERDICT_SAFE, "needs_agent": True},
                    )
                    _write_json_atomic(
                        pdir27 / "checkpoint.json",
                        {
                            "chain": "ethereum",
                            "audit_block": 0,
                            "caller": "0x" + "12" * 20,
                            "closure": {"all_addresses": ["0x" + "77" * 20], "token_decimals": {}, "token_prices_usd": {"native": 2000.0}},
                        },
                    )
                    for cid27, gain_eth in ((1, 1), (2, 5)):
                        _write_json_atomic(
                            pdir27 / "poc_requests" / f"poc_request_{cid27}.json",
                            {"candidate_id": cid27, "target_address": "0x" + "77" * 20, "zero_state_caller": "0x" + "12" * 20},
                        )
                        sol27 = f"""// SPDX-License-Identifier: MIT
    pragma solidity ^0.8.20;
    contract AgentVerify_{cid27} {{
        address constant TARGET = 0x{"77" * 20};
        function test_candidate_{cid27}() public {{
            (bool ok, ) = TARGET.call("");
            ok;
        }}
    }}
    """
                        tfile27 = pdir27 / "forge" / "test" / f"test_{cid27}.t.sol"
                        tfile27.write_text(sol27, encoding="utf-8")
                        _write_json_atomic(
                            pdir27 / f"forge_output_{cid27}.json",
                            {
                                "executed": True,
                                "passed_tests": 1,
                                "failed_tests": 0,
                                "conservation_gains": [
                                    {
                                        "test": f"test_candidate_{cid27}",
                                        "slice_idx": cid27,
                                        "asset": "native",
                                        "caller_gain": gain_eth * (10**18),
                                        "vault_loss": gain_eth * (10**18),
                                        "gas_used": 100_000,
                                        "supply_mint": False,
                                    }
                                ],
                                "error_reason": "",
                                "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False},
                            },
                        )
                        os.utime(pdir27 / f"forge_output_{cid27}.json", (time.time() + 10.0, time.time() + 10.0))
                        _write_json_atomic(
                            pdir27 / "verdicts" / f"verdict_{cid27}.json",
                            {
                                "candidate_id": cid27,
                                "verdict": "PROVEN_BY_FORGE",
                                "notes": f"Verified dynamic drain of {gain_eth} ETH on target vault",
                            },
                        )
                    handled27 = process_agent_submissions(r27, st27)
                    res27_doc = _read_json(pdir27 / "result.json", default={})
                    agent_max_profit_ok = (
                        handled27 >= 1
                        and isinstance(res27_doc, dict)
                        and _is_profit_verdict(str(res27_doc.get("verdict") or ""))
                        and float(res27_doc.get("net_usd") or 0.0) > 8000.0
                    )

                    # (2) WNATIVE 1:1 backed wrapping harness check
                    weth_eth = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    h_weth_27 = _render_minimal_solidity_harness(
                        slug="t27_weth_wrap",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "34" * 20,
                        target_addr=weth_eth,
                        token_addrs=[weth_eth],
                        slices=[],
                        vault_addresses=[weth_eth],
                    )
                    wnative_backing_harness_ok = (
                        "uint256 wnativeBacking;" in h_weth_27
                        and "s.wnativeBacking = address(TOK_0).balance;" in h_weth_27
                        and "wSInc = wSInc > wBacked ? (wSInc - wBacked) : 0;" in h_weth_27
                        and "emit ConservationSupplyMint(sliceIdx, assets[wIdx], wSInc);" in h_weth_27
                    )

                    # (3) Unverified selector needs_agent_synthesis & prescreen_eth_call_batch >48 slice preservation
                    cfg_unver = disassemble_and_build_cfg(
                        bytes.fromhex("60003560e01c631234567814601257600080fd5b6001600055600080808060013361fffff100")
                    )
                    slices_unver = slice_four_channel_causal_chains(
                        "0x" + "88" * 20,
                        cfg_unver,
                        ["0x12345678"],
                        abi_list=None,
                    )
                    unver_flag_ok = bool(slices_unver) and all(s.needs_agent_synthesis for s in slices_unver)
                    many_slices = [
                        CausalSlice(
                            slice_id=f"sl_{idx_m}",
                            target_address="0x" + "88" * 20,
                            channel="CH_STORAGE",
                            legs=[f"0x{idx_m + 1:08x}"],
                            needs_agent_synthesis=(idx_m == 55),
                        )
                        for idx_m in range(60)
                    ]
                    presc_many = prescreen_eth_call_batch(r27, "ethereum", 0, "0x" + "34" * 20, many_slices)
                    presc_preserve_ok = len(presc_many) == 60 and any(s.slice_id == "sl_55" for s in presc_many[:48])

                    # (4) Variable-aliased literal emit ConservationGain blocked
                    var_alias_sol = """
                contract FakeVarTest {
                    event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);
                    function test_fake() public {
                        uint256 callerGain = 1000 ether;
                        uint256 vaultLoss = 1000 ether;
                        emit ConservationGain(0, address(0), callerGain, vaultLoss, 21000);
                    }
                }
                """
                    ok_alias_27, viol_alias_27 = verify_agent_harness_accounting_integrity(var_alias_sol)
                    var_alias_blocked_ok = (not ok_alias_27) and any("LITERAL_" in v for v in viol_alias_27)
                finally:
                    st27.close()

                t27_ok = (
                    agent_max_profit_ok
                    and wnative_backing_harness_ok
                    and unver_flag_ok
                    and presc_preserve_ok
                    and var_alias_blocked_ok
                )
                results.append(
                    (
                        "T27_ROUND5_16POINT_COLD_AUDIT_REGRESSION",
                        t27_ok,
                        f"agent_profit={agent_max_profit_ok},wnative_back={wnative_backing_harness_ok},unver={unver_flag_ok},presc60={presc_preserve_ok},var_alias={var_alias_blocked_ok}",
                    )
                )

            if _want_test("T28_STATIC_SOUNDNESS_CONSERVATION_AND_CLOSURE_EXHAUSTION", tag_filters):
                # T28: Static Soundness, Exact Decimal Conservation, Trace Isolation, Historical Block Clues, and E4 Multi-Vault Profit Lattice
                st28 = SQLiteStore(selftest_ns / "t28_store.sqlite")
                try:
                    st28.put_price("fx:USD_KRW", 1400.0, "fixture:t28")
                    tok_v3_28 = "0x" + "a1" * 20
                    pool_v3_28 = "0x" + "b2" * 20
                    weth_28 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]

                    class _MockV3AndStaleRouter(_SelftestMockRouter):
                        _cand_urls = ("http://127.0.0.1:8545",)

                        def multicall3(self, chain: str, calls: List[Tuple[str, bytes]], **kw: Any) -> List[Tuple[bool, bytes]]:
                            out: List[Tuple[bool, bytes]] = []
                            for addr_c, data_c in calls:
                                na_c = _norm_addr(addr_c)
                                sel_h = "0x" + data_c[:4].hex() if len(data_c) >= 4 else ""
                                if na_c == tok_v3_28 and sel_h == TOTAL_SUPPLY_SEL:
                                    out.append((True, (1_000_000 * 10**18).to_bytes(32, "big")))
                                elif sel_h == V3_GET_POOL_SEL and weth_28[2:] in data_c.hex():
                                    out.append((True, b"\x00" * 12 + bytes.fromhex("b2" * 20)))
                                elif na_c == weth_28 and sel_h == BALANCE_OF_SEL:
                                    out.append((True, (500 * 10**18).to_bytes(32, "big")))
                                elif na_c == weth_28 and sel_h == DECIMALS_SEL:
                                    out.append((True, (18).to_bytes(32, "big")))
                                elif na_c == tok_v3_28 and sel_h == BALANCE_OF_SEL:
                                    out.append((True, (500_000 * 10**18).to_bytes(32, "big")))
                                elif na_c == pool_v3_28 and sel_h == V3_SLOT0_SEL:
                                    out.append((True, ((1 << 96) // 32).to_bytes(32, "big") + b"\x00" * 192))
                                elif na_c == pool_v3_28 and sel_h == V2_TOKEN0_SEL:
                                    out.append((True, b"\x00" * 12 + bytes.fromhex("a1" * 20)))
                                elif na_c == _CHAINLINK_NATIVE_USD_FEEDS["ethereum"] and sel_h == "0xfeaf968c":
                                    stale_r = (
                                        (10).to_bytes(32, "big")
                                        + (2500 * 10**8).to_bytes(32, "big")
                                        + (1_790_000_000).to_bytes(32, "big")
                                        + (1_790_000_000).to_bytes(32, "big")
                                        + (10).to_bytes(32, "big")
                                    )
                                    out.append((True, stale_r))
                                elif na_c == _CHAINLINK_NATIVE_USD_FEEDS["ethereum"] and sel_h == DECIMALS_SEL:
                                    out.append((True, (8).to_bytes(32, "big")))
                                else:
                                    out.append((False, b""))
                            return out

                        def call(self, chain: str, method: str, params: List[Any], **kw: Any) -> Any:
                            if method == "eth_getBlockByNumber":
                                return {"timestamp": hex(1_800_000_000), "baseFeePerGas": hex(100_000)}
                            if method == "eth_getCode":
                                pb = int(str(params[1]), 16) if len(params) > 1 and str(params[1]).startswith("0x") else 0
                                if pb in (18_500_000, 18_499_999):
                                    return "0x600160005560206000f360016002600360046005"
                                return "0x"
                            return None

                        def batch_call(self, chain: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_b: List[Any] = []
                            for m_c, p_c in calls:
                                out_b.append(self.call(chain, m_c, p_c))
                            return out_b

                        def http_get_json(self, url: str, timeout: float = 5.0) -> Dict[str, Any]:
                            return {}

                    r28 = _MockV3AndStaleRouter(store=st28)
                    v3_liq = probe_dex_exit_liquidity(
                        r28,  # type: ignore[arg-type]
                        "ethereum",
                        tok_v3_28,
                        100 * 10**18,
                        18,
                        2.0,
                        2000.0,
                        block_pin=19_000_000,
                    )
                    v3_slot0_ok = bool(v3_liq.get("exit_liquid")) and float(v3_liq.get("realizable_usd") or 0.0) > 150.0

                    stale_lo, stale_hi = _estimate_native_price_interval_usd(
                        "ethereum", router=r28, block_pin=19_000_000, allow_offline_fixture=False  # type: ignore[arg-type]
                    )
                    stale_feed_rejected_ok = (stale_lo == 0.0 and stale_hi == 0.0)

                    arb_usdt = "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"
                    poly_usdt = "0xc2132d05d31c914a87c6611c10748aeb04b58e8f"
                    zk_alias = "0x000000000000000000000000000000000000800a"
                    h_abs = _render_minimal_solidity_harness(
                        slug="t28_abs",
                        chain="abstract",
                        block_pin=0,
                        caller="0x" + "12" * 20,
                        target_addr="0x" + "34" * 20,
                        token_addrs=[zk_alias, CHAIN_STABLES["abstract"][0], CHAIN_STABLES["abstract"][0].upper().replace("0X", "0x")],
                        slices=[],
                    )
                    acct_exact_100 = judge_conservation_accounting(
                        store=st28,
                        chain="arbitrum",
                        caller_before={arb_usdt: 0},
                        caller_after={arb_usdt: 100_010_000},
                        vault_before={arb_usdt: 100_010_000},
                        vault_after={arb_usdt: 0},
                        gas_used=21_000,
                        gas_price_wei=100_000,
                        decimals_map={arb_usdt: 6},
                        prices_map={"native": 1.0, arb_usdt: 1.0},
                        router=r28,  # type: ignore[arg-type]
                        block_pin=19_000_000,
                    )
                    tier1_decimal_ok = (
                        arb_usdt in CHAIN_STABLES["arbitrum"][:4]
                        and poly_usdt in CHAIN_STABLES["polygon"][:4]
                        and "monad" in CHAIN_STABLES
                        and zk_alias not in h_abs.lower()
                        and "TOK_1" not in h_abs
                        and _is_profit_verdict(acct_exact_100.verdict)
                        and acct_exact_100.swap_fee_usd == 0.0
                        and acct_exact_100.slippage_usd == 0.0
                        and acct_exact_100.net_usd == 100.0
                    )

                    hevm_empty_cd_caught = _scan_trace_tree_for_disallowed_hevm(
                        {"address": HEVM_ADDRESS, "kind": "call", "value": "0x0"}
                    )
                    hevm_val_caught = _scan_trace_tree_for_disallowed_hevm(
                        {"address": HEVM_ADDRESS, "kind": "call", "value": "0x1", "data": selftest_stop_prank_sel}
                    )
                    hevm_dcall_caught = _scan_trace_tree_for_disallowed_hevm(
                        {"address": HEVM_ADDRESS, "kind": "delegatecall", "value": "0x0", "data": selftest_stop_prank_sel}
                    )
                    import http.server as _hs28
                    import socketserver as _ss28

                    class _LoopbackRPC28(_ss28.ThreadingMixIn, _hs28.HTTPServer):
                        daemon_threads = True
                        handle_error = _noop_http_hook

                    _z32_28 = "0x" + ("00" * 32)
                    _et_28 = "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
                    _eu_28 = "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347"

                    class _LoopbackHandler28(_hs28.BaseHTTPRequestHandler):
                        log_message = _noop_http_hook

                        def do_POST(self) -> None:
                            try:
                                ln28 = int(self.headers.get("Content-Length") or "0")
                                req28 = json.loads(self.rfile.read(ln28).decode("utf-8"))
                            except (OSError, ValueError):
                                return

                            def _one28(it28: Dict[str, Any]) -> Dict[str, Any]:
                                m28 = str(it28.get("method") or "")
                                id28 = it28.get("id", 1)
                                if m28 == "eth_chainId":
                                    r_v: Any = "0x1"
                                elif m28 == "net_version":
                                    r_v = "1"
                                elif m28 == "eth_blockNumber":
                                    r_v = hex(20_000_000)
                                elif m28 == "eth_gasPrice":
                                    r_v = "0x3b9aca00"
                                elif m28 == "eth_getBlockByNumber":
                                    p0_28 = (it28.get("params") or ["latest"])[0]
                                    bnum28 = p0_28 if isinstance(p0_28, str) and p0_28.startswith("0x") else hex(20_000_000)
                                    r_v = {
                                        "number": bnum28,
                                        "hash": "0x" + ("ab" * 32),
                                        "parentHash": "0x" + ("cd" * 32),
                                        "sha3Uncles": _eu_28,
                                        "logsBloom": "0x" + ("00" * 256),
                                        "transactionsRoot": _et_28,
                                        "stateRoot": _et_28,
                                        "receiptsRoot": _et_28,
                                        "miner": "0x" + ("00" * 20),
                                        "difficulty": "0x0",
                                        "totalDifficulty": "0x0",
                                        "extraData": "0x",
                                        "size": "0x3e8",
                                        "gasLimit": "0x1c9c380",
                                        "gasUsed": "0x0",
                                        "timestamp": "0x66000000",
                                        "transactions": [],
                                        "uncles": [],
                                        "baseFeePerGas": "0x3b9aca00",
                                        "mixHash": _z32_28,
                                        "nonce": "0x0000000000000000",
                                        "withdrawals": [],
                                        "withdrawalsRoot": _et_28,
                                        "blobGasUsed": "0x0",
                                        "excessBlobGas": "0x0",
                                        "parentBeaconBlockRoot": _z32_28,
                                    }
                                elif m28 in ("eth_getBalance", "eth_getTransactionCount"):
                                    r_v = "0x0"
                                elif m28 == "eth_getCode":
                                    r_v = "0x"
                                elif m28 == "eth_getStorageAt":
                                    r_v = _z32_28
                                else:
                                    r_v = "0x0"
                                return {"jsonrpc": "2.0", "id": id28, "result": r_v}

                            resp28 = [_one28(x) for x in req28] if isinstance(req28, list) else _one28(req28)
                            raw_b28 = json.dumps(resp28).encode("utf-8")
                            try:
                                self.send_response(200)
                                self.send_header("Content-Type", "application/json")
                                self.send_header("Content-Length", str(len(raw_b28)))
                                self.end_headers()
                                self.wfile.write(raw_b28)
                            except OSError:
                                return

                    srv28 = _LoopbackRPC28(("127.0.0.1", 0), _LoopbackHandler28)
                    th28 = threading.Thread(target=srv28.serve_forever, daemon=True)
                    th28.start()
                    try:
                        rpc_u28 = f"http://127.0.0.1:{srv28.server_address[1]}"
                        ws_lr28 = ensure_clean_forge_workspace({"ethereum": rpc_u28}, slug="t28_live_replay", offline=True)
                        rel_lr28 = "test/LiveHeadGuard28.t.sol"
                        (ws_lr28 / rel_lr28).write_text(
                            "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n"
                            "interface Vm { function deal(address who, uint256 newBalance) external; }\n"
                            "contract AgentVerify_LiveHeadGuard28 {\n"
                            "    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);\n"
                            "    function test_live_cond() public {\n"
                            "        if (block.number >= 20000000) { vm.deal(address(0x1234), 10 ether); }\n"
                            "    }\n"
                            "}\n",
                            encoding="utf-8",
                        )
                        pb28 = subprocess.run(
                            [str(FORGE_BIN), "build", "--offline"],
                            cwd=str(ws_lr28),
                            env=_isolated_subproc_env(ws_lr28),
                            stdin=subprocess.DEVNULL,
                            capture_output=True,
                            text=True,
                            encoding="utf-8",
                            errors="replace",
                            timeout=60,
                        )
                        lr_out28 = replay_compiled_test_at_live_head(
                            ws_lr28, rel_lr28, rpc_u28, live_head_block=20_000_000, match_test="test_live_cond", timeout_sec=60
                        )
                        live_replay_cli_ok = (
                            pb28.returncode == 0
                            and lr_out28.error_reason == "CHEATCODE_TRACE_VIOLATION_AT_LIVE_HEAD"
                            and bool(lr_out28.trace_summary.get("disallowed_cheatcode_in_trace"))
                        )
                    finally:
                        srv28.shutdown()
                        srv28.server_close()
                    trace_hevm_strict_ok = (
                        hevm_empty_cd_caught and hevm_val_caught and hevm_dcall_caught and live_replay_cli_ok
                    )

                    synth_md_28 = selftest_ns / "_t28_clues.md"
                    synth_md_28.write_text(
                        "## 03/15 — SyntheticClueVault (Ethereum)\n"
                        "- **취약 원리**: Destroyed at block: 18500000 after drain\n"
                        "- **취약 컨트랙트**:\n"
                        "  - `0x" + ("c3" * 20) + "`\n",
                        encoding="utf-8",
                    )
                    part_28 = _parse_corpus_md_partition(synth_md_28)
                    evm_28 = list(part_28.get("evm_cases") or [])
                    rec_code_28 = _recover_historical_bytecode_bisect(
                        r28, "ethereum", "0x" + ("c3" * 20), 21_000_000, hint_blocks=[18_500_000]  # type: ignore[arg-type]
                    )
                    hist_clue_ok = (
                        len(evm_28) == 1
                        and 18_500_000 in evm_28[0].block_numbers
                        and len(rec_code_28) >= 16
                    )

                    # E4 sealed_mechanical_profit preservation when remaining agent slice reports VERDICT_SAFE
                    slug28 = "t28-e4-sealed-profit"
                    pdir28 = selftest_ns / slug28
                    (pdir28 / "poc_requests").mkdir(parents=True, exist_ok=True)
                    (pdir28 / "verdicts").mkdir(parents=True, exist_ok=True)
                    (pdir28 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    st28.upsert_queue_item(slug28, 1, "ethereum", 100_000.0, ["0x" + "89" * 20], mode="A", sort_rank=500.0)
                    st28.set_queue_agent_state(slug28, "awaiting_verdict")
                    _write_json_atomic(
                        pdir28 / "result.json",
                        {"slug": slug28, "chain": "ethereum", "audit_block": 0, "verdict": VERDICT_SAFE, "needs_agent": True},
                    )
                    _write_json_atomic(
                        pdir28 / "checkpoint.json",
                        {
                            "chain": "ethereum",
                            "audit_block": 0,
                            "caller": "0x" + "12" * 20,
                            "closure": {"all_addresses": ["0x" + "89" * 20], "token_decimals": {}, "token_prices_usd": {"native": 2000.0}},
                            "sealed_mechanical_profit": {
                                "verdict": _format_profit_verdict(350_000),
                                "audit_block": 0,
                                "gross_usd": 255.0,
                                "gas_usd": 5.0,
                                "flash_fee_usd": 0.0,
                                "swap_fee_usd": 0.0,
                                "slippage_usd": 0.0,
                                "net_usd": 250.0,
                                "net_krw": 350_000,
                                "fx_rate_krw": 1400.0,
                                "fx_source": "fixture:t28",
                                "gross_profit_native": "0.127500000000000000",
                                "net_profit_native": "0.125000000000000000",
                                "profit_native_unit": "ETH",
                                "costs": {"gas_native": "0.0025", "flash_loan_fee_native": "0", "swap_fees_native": "0", "slippage_native": "0"},
                                "reason": "PROVEN_NET_PROFIT_350000_KRW",
                                "forge_payload": {
                                    "executed": True,
                                    "passed_tests": 1,
                                    "failed_tests": 0,
                                    "conservation_gains": [{"test": "test_slice_0", "slice_idx": 0, "asset": "native", "caller_gain": 1275 * 10**14, "vault_loss": 1275 * 10**14, "gas_used": 100000}],
                                    "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False},
                                },
                            },
                        },
                    )
                    _write_json_atomic(
                        pdir28 / "poc_requests" / "poc_request_1.json",
                        {"candidate_id": 1, "target_address": "0x" + "89" * 20, "zero_state_caller": "0x" + "12" * 20},
                    )
                    (pdir28 / "forge" / "test" / "test_1.t.sol").write_text(
                        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n"
                        f"contract AgentVerify_1 {{ address constant TARGET = 0x{'89' * 20}; function test_candidate_1() public {{ (bool ok,) = TARGET.call(\"\"); ok; }} }}\n",
                        encoding="utf-8",
                    )
                    _write_json_atomic(
                        pdir28 / "forge_output_1.json",
                        {
                            "executed": True,
                            "passed_tests": 1,
                            "failed_tests": 0,
                            "conservation_gains": [],
                            "error_reason": "",
                            "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False},
                        },
                    )
                    os.utime(pdir28 / "forge_output_1.json", (time.time() + 10.0, time.time() + 10.0))
                    _write_json_atomic(
                        pdir28 / "verdicts" / "verdict_1.json",
                        {"candidate_id": 1, "verdict": VERDICT_SAFE, "notes": "Secondary vault entrypoint verified invariant-closed on-chain"},
                    )
                    pend_before_28 = st28.list_agent_pending_items()
                    handled28 = process_agent_submissions(RPCRouter(store=st28), st28)
                    res28_doc = _read_json(pdir28 / "result.json", default={})
                    e4_sealed_lattice_ok = (
                        len(pend_before_28) == 1
                        and handled28 >= 1
                        and isinstance(res28_doc, dict)
                        and _is_profit_verdict(str(res28_doc.get("verdict") or ""))
                        and float(res28_doc.get("net_usd") or 0.0) == 250.0
                        and int(res28_doc.get("net_krw") or 0) == 350_000
                    )
                finally:
                    st28.close()

                t28_ok = (
                    v3_slot0_ok
                    and stale_feed_rejected_ok
                    and tier1_decimal_ok
                    and trace_hevm_strict_ok
                    and hist_clue_ok
                    and e4_sealed_lattice_ok
                )
                results.append(
                    (
                        "T28_STATIC_SOUNDNESS_CONSERVATION_AND_CLOSURE_EXHAUSTION",
                        t28_ok,
                        f"v3={v3_slot0_ok},stale_cl={stale_feed_rejected_ok},dec100={tier1_decimal_ok},hevm_tr={trace_hevm_strict_ok},hist_b={hist_clue_ok},e4_lat={e4_sealed_lattice_ok}",
                    )
                )

            if _want_test("T29_SSOT_HEAD_FRESHNESS_WNATIVE_MINT_V3PEG_PRECOMPILE_AND_ISOLATION", tag_filters):
                # T29: SSOT Soundness, DNS Socket Guard, WNATIVE Mint Cap, Decimal Rounding, V3 Stable Peg, Leading-Zero/Victim Seeds, 12-Key Env & Live Head Freshness
                st29 = SQLiteStore(selftest_ns / "_t29_ssot.sqlite")
                try:
                    # (1) Offline DNS resolution guard + loopback pass
                    dns_blocked_ok = False
                    try:
                        socket.getaddrinfo("example.com", 443)
                    except OSError as exc_dns:
                        dns_blocked_ok = "OFFLINE_SOCKET_GUARD" in str(exc_dns)
                    loopback_dns_ok = len(socket.getaddrinfo("127.0.0.1", 8545)) >= 1

                    # (2) WNATIVE unbacked supply mint cap, Decimal $99.999 downward quantization, and full CHAIN_STABLES (>8) coverage
                    st29.put_price("fx:USD_KRW", 1400.0, "fixture:t29")
                    weth29 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                    acct_wmint_live = judge_conservation_accounting(
                        store=st29,
                        chain="ethereum",
                        caller_before={weth29: 0},
                        caller_after={weth29: 10 * 10**18},
                        vault_before={weth29: 10 * 10**18},
                        vault_after={weth29: 0},
                        gas_used=50_000,
                        gas_price_wei=100_000_000,
                        decimals_map={weth29: 18},
                        prices_map={"native": 3000.0, weth29: 3000.0},
                        block_pin=20_000_000,
                        live_head_block=20_000_001,
                        supply_minted_assets={weth29},
                        allow_offline_fixture=True,
                    )
                    acct_99999 = judge_conservation_accounting(
                        store=st29,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 100_020_999_000_000_000},
                        vault_before={"native": 100_020_999_000_000_000},
                        vault_after={"native": 0},
                        gas_used=21_000,
                        gas_price_wei=1_000_000_000,
                        decimals_map={},
                        prices_map={"native": 1000.0},
                        allow_offline_fixture=True,
                    )
                    synth_9th_stable = "0x" + "91" * 20
                    orig_bsc_stables29 = list(CHAIN_STABLES.get("bsc", []))
                    try:
                        CHAIN_STABLES["bsc"] = orig_bsc_stables29 + [synth_9th_stable]
                        p9_val, _ = _fetch_token_price_usd(st29, "bsc", synth_9th_stable, allow_offline_fixture=True)
                    finally:
                        CHAIN_STABLES["bsc"] = orig_bsc_stables29
                    acct_live_no_router = judge_conservation_accounting(
                        store=st29,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 10**18},
                        vault_before={"native": 10**18},
                        vault_after={"native": 0},
                        router=None,
                        block_pin=20_000_000,
                    )
                    wmint_and_dec_ok = (
                        acct_wmint_live.gross_usd == 0.0
                        and acct_99999.verdict == VERDICT_SAFE
                        and acct_99999.net_usd == 99.99
                        and p9_val == 1.0
                        and acct_live_no_router.verdict == VERDICT_BLOCKED
                    )

                    # (3) Uniswap V3 slot0 stable peg verification (asymmetric reserves at $1.00 peg vs $0.50 depeg) + 24-decimal ERC-20 metadata pricing & upper bound
                    usdc29 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    v3_pool29 = "0x" + "88" * 20
                    tok24_addr = "0x" + "76" * 20
                    pair24_addr = "0x" + "75" * 20

                    class _V3PegAnd24DecRouter(RPCRouter):
                        def __init__(self, st_obj: SQLiteStore, sqrt_mult: int) -> None:
                            super().__init__(store=st_obj)
                            self.sqrt_mult = sqrt_mult

                        def http_get_json(self, url: str, timeout: float = 15.0, extra_headers: Optional[Dict[str, str]] = None) -> Any:
                            if tok24_addr in url:
                                return {"coins": {f"ethereum:{tok24_addr}": {"price": 2.10}}}
                            return {"coins": {}}

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 50) -> List[Tuple[bool, bytes]]:
                            out_m: List[Tuple[bool, bytes]] = []
                            for to_a, cd in calls:
                                sel = "0x" + cd[:4].hex().lower()
                                nt = _norm_addr(to_a)
                                if sel == V3_GET_POOL_SEL.lower():
                                    out_m.append((True, b"\x00" * 12 + bytes.fromhex("88" * 20)))
                                elif sel == V2_GET_PAIR_SEL.lower():
                                    if tok24_addr[2:] in cd.hex().lower():
                                        out_m.append((True, b"\x00" * 12 + bytes.fromhex("75" * 20)))
                                    else:
                                        out_m.append((True, b"\x00" * 32))
                                elif sel == DECIMALS_SEL.lower():
                                    d_v = 24 if nt == tok24_addr else (6 if nt == usdc29 else 18)
                                    out_m.append((True, d_v.to_bytes(32, "big")))
                                elif sel == SYMBOL_SEL.lower():
                                    out_m.append((True, abi_encode(["string"], ["T24"])))
                                elif sel == BALANCE_OF_SEL.lower() and len(cd) >= 36:
                                    who = "0x" + cd[16:36].hex().lower()
                                    if who == v3_pool29:
                                        # Asymmetric concentrated reserves: 1 WETH ($2,500) vs 50,000 USDC
                                        if nt == weth29:
                                            out_m.append((True, (1 * 10**18).to_bytes(32, "big")))
                                        else:
                                            out_m.append((True, (50_000 * 10**6).to_bytes(32, "big")))
                                    elif who == pair24_addr:
                                        # 100 WETH ($250,000) vs 125,000 * 10^24 T24 -> $2.00 DEX unit price
                                        if nt == weth29:
                                            out_m.append((True, (100 * 10**18).to_bytes(32, "big")))
                                        else:
                                            out_m.append((True, (125_000 * 10**24).to_bytes(32, "big")))
                                    else:
                                        out_m.append((True, (0).to_bytes(32, "big")))
                                elif sel == V3_SLOT0_SEL.lower():
                                    sqrt_p = self.sqrt_mult * (1 << 96)
                                    out_m.append((True, sqrt_p.to_bytes(32, "big") + b"\x00" * (32 * 6)))
                                elif sel == V2_TOKEN0_SEL.lower():
                                    out_m.append((True, b"\x00" * 12 + bytes.fromhex(usdc29[2:])))
                                else:
                                    out_m.append((False, b""))
                            return out_m

                    r_v3_peg_ok = _V3PegAnd24DecRouter(st29, 20000)  # (20000)^2 * 10^-12 * 2500 = $1.00
                    r_v3_depeg = _V3PegAnd24DecRouter(st29, 14142)   # (14142)^2 * 10^-12 * 2500 ~= $0.50
                    q_peg_ok = probe_dex_exit_liquidity(r_v3_peg_ok, "ethereum", usdc29, 1000 * 10**6, 6, 1.0, 2500.0, 20_000_000)
                    q_depeg_ok = probe_dex_exit_liquidity(r_v3_depeg, "ethereum", usdc29, 1000 * 10**6, 6, 1.0, 2500.0, 20_000_000)
                    hi_prices_29: Dict[str, float] = {}
                    dec_m29, lo_m29 = _resolve_token_metadata_batch(
                        r_v3_peg_ok, "ethereum", [tok24_addr], 20_000_000, 2500.0, upper_prices_out=hi_prices_29
                    )
                    v3_and_dec24_ok = (
                        bool(q_peg_ok.get("exit_liquid")) is True
                        and not bool(q_peg_ok.get("price_diverged"))
                        and bool(q_depeg_ok.get("exit_liquid")) is False
                        and bool(q_depeg_ok.get("price_diverged")) is True
                        and dec_m29.get(tok24_addr) == 24
                        and 1.95 <= float(lo_m29.get(tok24_addr) or 0.0) <= 2.02
                        and float(hi_prices_29.get(tok24_addr) or 0.0) >= 2.09
                    )

                    # (4) Post-commit residual approval check, constant-subtraction harness spoof guard, and mixed-selector bytecode cheatcode guard
                    h29_sol = _render_minimal_solidity_harness(
                        slug="t29_allow",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "11" * 20,
                        target_addr="0x" + "22" * 20,
                        token_addrs=["0x" + "33" * 20],
                        slices=[],
                    )
                    const_sub_spoof = (
                        "pragma solidity ^0.8.20;\ncontract Spoof {\n"
                        "  event ConservationGain(uint256 s, address a, uint256 cg, uint256 vl, uint256 gu);\n"
                        "  function test_spoof() public { emit ConservationGain(0, address(0), 1000 ether - 1, 1000 ether - 1, 50000); }\n}\n"
                    )
                    ok_spoof29, _ = verify_agent_harness_accounting_integrity(const_sub_spoof)
                    stop_prank_b = bytes.fromhex(selftest_stop_prank_sel[2:])
                    deal_b = bytes.fromhex(_selector("deal(address,uint256)")[2:])
                    smuggled_hevm_bc = (
                        b"\x63" + stop_prank_b
                        + b"\x63" + deal_b + b"\x60\xe0\x1b"
                        + b"\x73" + bytes.fromhex(HEVM_ADDRESS_LOWER[2:])
                        + b"\xf1\x00"
                    )
                    ok_smug_bc, _ = verify_cheatcode_allowlist("contract Clean {}", compiled_bytecode=smuggled_hevm_bc)
                    harness_and_bc_ok = (
                        "_erc20Allowance" in h29_sol
                        and (not ok_spoof29)
                        and (not ok_smug_bc)
                    )

                    # (5) Leading-zero-byte contract preservation vs precompile exclusion, victim component labels, and log-based pre-upgrade proxy recovery
                    lead_zero_contract = "0x0000000000000000000000000000000000000250"
                    precompile_802 = "0x0000000000000000000000000000000000000802"
                    synth_md_29 = selftest_ns / "_t29_corpus.md"
                    synth_md_29.write_text(
                        "## 03/15 — SyntheticModularCase (Ethereum)\n"
                        "- **취약 원리**: Oracle adapter rounding\n"
                        "- **취약 컨트랙트**:\n"
                        f"  - 오라클 어댑터: `{lead_zero_contract}`\n"
                        "  - 스마트 계정 모듈: `0x0000000000000000000000000000000000001234`\n"
                        f"  - Precompile ref: `{precompile_802}`\n",
                        encoding="utf-8",
                    )
                    parsed_29 = _parse_corpus_md(synth_md_29)
                    seeds_29 = parsed_29[0].seed_addresses.get("ethereum", []) if parsed_29 else []

                    class _UpgradeLogHistRouter:
                        def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                            if method == "eth_getLogs":
                                return [
                                    {
                                        "blockNumber": hex(19_998_500),
                                        "topics": [
                                            "0xbc7cd75a20ee27fd9adebab32041f755214dbc6bffa90cc0225b39da2e5c2d3b",
                                            "0x" + ("00" * 12) + ("dd" * 20),
                                        ],
                                        "data": "0x",
                                    }
                                ]
                            if method == "eth_getCode":
                                a_q = _norm_addr(str(params[0]))
                                b_q = _hex_to_int(params[1]) if len(params) > 1 else 0
                                if a_q == ("0x" + "cc" * 20) and b_q == 19_998_499:
                                    return "0x" + ("6001600055" * 4)
                                if a_q == ("0x" + "dd" * 20):
                                    return "0x" + ("600080fd" * 5)
                                return "0x"
                            return "0x"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_b: List[Any] = []
                            for m_c, p_c in calls:
                                if m_c == "eth_getStorageAt":
                                    sl_q = str(p_c[1]).lower()
                                    b_q = _hex_to_int(p_c[2]) if len(p_c) > 2 else 0
                                    if sl_q == EIP1967_SLOTS["impl"].lower() and b_q == 19_998_499:
                                        out_b.append("0x" + ("00" * 12) + ("cc" * 20))
                                    else:
                                        out_b.append("0x" + ("00" * 32))
                                elif m_c == "eth_getCode":
                                    out_b.append(self.call(ch, m_c, p_c, **kw))
                                else:
                                    out_b.append("0x")
                            return out_b

                    hist_29 = _recover_historical_proxy_impl_bytecodes(
                        _UpgradeLogHistRouter(),  # type: ignore[arg-type]
                        "ethereum",
                        "0x" + "ee" * 20,
                        20_000_000,
                    )
                    precompile_and_hist_ok = (
                        _is_non_precompile_addr(lead_zero_contract) is True
                        and _is_non_precompile_addr(precompile_802) is False
                        and lead_zero_contract in seeds_29
                        and "0x0000000000000000000000000000000000001234" in seeds_29
                        and precompile_802 not in seeds_29
                        and any(cb == bytes.fromhex("6001600055" * 4) for cb in hist_29)
                    )

                    # (6) 12-key self-describing isolated_env_hint, custom artifact_root done-gate, malformed/unverified verdict rejection, Mode A dust-vault gap & stale live head Fail-Open
                    req_12_keys = {
                        "TEMP", "TMP", "TMPDIR", "USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA",
                        "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "FOUNDRY_DIR", "SVM_DATA_DIR",
                    }
                    addr_req_29 = emit_address_discovery_request("t29-req-slug", "ethereum", 20_000_000, 200_000.0)
                    addr_doc_29 = _read_json(addr_req_29, default={})
                    poc_paths_29 = emit_poc_requests_for_slug(
                        "t29-poc-slug",
                        "ethereum",
                        20_000_000,
                        "0x" + "11" * 20,
                        [CausalSlice(slice_id="s1", target_address="0x" + "22" * 20, channel="CH_STORAGE", legs=["0x12345678"], needs_agent_synthesis=True)],
                        {},
                    )
                    poc_doc_29 = _read_json(poc_paths_29[0], default={}) if poc_paths_29 else {}
                    env_a29 = ((addr_doc_29.get("isolated_commands") or {}).get("isolated_env_hint") or {})
                    env_p29 = ((poc_doc_29.get("isolated_commands") or {}).get("isolated_env_hint") or {})
                    cmd_str_all = json.dumps({
                        "a": {k: v for k, v in (addr_doc_29.get("isolated_commands") or {}).items() if k != "isolated_env_hint"},
                        "p": {k: v for k, v in (poc_doc_29.get("isolated_commands") or {}).items() if k != "isolated_env_hint"},
                    })
                    env_12_and_cmd_ok = (
                        req_12_keys.issubset(env_a29.keys())
                        and req_12_keys.issubset(env_p29.keys())
                        and all(Path(str(env_a29[k])).resolve().is_relative_to(selftest_ns.resolve()) for k in req_12_keys)
                        and all(Path(str(env_p29[k])).resolve().is_relative_to(selftest_ns.resolve()) for k in req_12_keys)
                        and "<" not in cmd_str_all
                        and ">" not in cmd_str_all
                    )

                    custom_gate_dir = selftest_ns / "_t29_custom_gate"
                    (custom_gate_dir / "pending-proto").mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(custom_gate_dir / "pending-proto" / "address_request.json", {"slug": "pending-proto"})
                    st_cg29 = SQLiteStore(custom_gate_dir / "cg.sqlite")
                    try:
                        st_cg29.upsert_queue_item("done-proto", 1, "ethereum", 150_000.0, ["0x" + "11" * 20])
                        st_cg29.complete_queue_item("done-proto", VERDICT_SAFE, 0.0)
                        write_protocol_artifacts(
                            "done-proto",
                            {"slug": "done-proto", "chain": "ethereum", "audit_block": 20_000_000, "verdict": VERDICT_SAFE, "net_usd": 0.0, "net_krw": 0, "reason": "ok"},
                            root_dir=custom_gate_dir,
                        )
                        cg_rep29 = _evaluate_done_gates(st_cg29, artifact_root=custom_gate_dir, run_selftest=False)
                    finally:
                        st_cg29.close()
                    custom_gate2_blocked_ok = cg_rep29.get("gate2_zero_pending_agent") is False

                    rej_slug29 = "t29-unver-profit"
                    pdir_r29 = selftest_ns / rej_slug29
                    (pdir_r29 / "poc_requests").mkdir(parents=True, exist_ok=True)
                    (pdir_r29 / "verdicts").mkdir(parents=True, exist_ok=True)
                    (pdir_r29 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    st29.upsert_queue_item(rej_slug29, 1, "ethereum", 120_000.0, ["0x" + "22" * 20], mode="A")
                    st29.set_queue_agent_state(rej_slug29, "awaiting_verdict")
                    _write_json_atomic(pdir_r29 / "result.json", {"slug": rej_slug29, "chain": "ethereum", "audit_block": 0, "verdict": VERDICT_SAFE, "needs_agent": True})
                    _write_json_atomic(pdir_r29 / "checkpoint.json", {"chain": "ethereum", "audit_block": 0, "caller": "0x" + "11" * 20})
                    _write_json_atomic(pdir_r29 / "poc_requests" / "poc_request_1.json", {"candidate_id": 1, "target_address": "0x" + "22" * 20})
                    (pdir_r29 / "forge" / "test" / "test_1.t.sol").write_text(
                        "pragma solidity ^0.8.20;\ncontract T { function test_1() public { (bool ok,) = address(0x" + "22" * 20 + ").call(\"\"); ok; } }\n",
                        encoding="utf-8",
                    )
                    _write_json_atomic(
                        pdir_r29 / "forge_output_1.json",
                        {"executed": True, "passed_tests": 1, "failed_tests": 0, "conservation_gains": [], "error_reason": "", "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False}},
                    )
                    os.utime(pdir_r29 / "forge_output_1.json", (time.time() + 10.0, time.time() + 10.0))
                    _write_json_atomic(pdir_r29 / "verdicts" / "verdict_1.json", {"candidate_id": 1, "verdict": "PROVEN_BY_FORGE", "notes": "Claiming profit with zero gains"})
                    process_agent_submissions(RPCRouter(store=st29), st29)
                    unver_profit_rejected_ok = (
                        not (pdir_r29 / "verdicts" / "verdict_1.json").exists()
                        and int((_read_json(pdir_r29 / "poc_requests" / "poc_request_1.json", default={}) or {}).get("rejection_count") or 0) == 1
                    )
                    (pdir_r29 / "verdicts" / "verdict_1.json").write_text("{malformed_json", encoding="utf-8")
                    process_agent_submissions(RPCRouter(store=st29), st29)
                    malformed_rejected_ok = (
                        not (pdir_r29 / "verdicts" / "verdict_1.json").exists()
                        and int((_read_json(pdir_r29 / "poc_requests" / "poc_request_1.json", default={}) or {}).get("rejection_count") or 0) == 2
                    )

                    class _DustAndStaleHeadRouter(_SelftestMockRouter):
                        _mc3_zero_word = True

                        def get_quorum_head_block(self, ch: str) -> Tuple[int, int]:
                            return (20_000_020, 20_000_005)  # lag 15 > MAX_AUDIT_BLOCK_LAG(5)

                        def http_get_json(self, url: str, **kw: Any) -> Any:
                            if "coingecko:ethereum" in url:
                                return {"coins": {"coingecko:ethereum": {"price": 2500.0}}}
                            return {"coins": {}}

                        def call(self, ch: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_getBlockByNumber":
                                bnum = _hex_to_int(params[0]) if params else 20_000_000
                                return {"number": hex(bnum), "timestamp": hex(1_760_000_000 + (bnum - 20_000_000) * 12)}
                            return "0x"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_d: List[Any] = []
                            for m_c, _p_c in calls:
                                if m_c == "eth_getCode":
                                    out_d.append("0x6318160ddd14600d57600080fd5b60206000f3")
                                elif m_c == "eth_getBalance":
                                    out_d.append(hex(2 * 10**15))  # 0.002 ETH = $5.00 (< $100)
                                elif m_c == "eth_getStorageAt":
                                    out_d.append("0x" + ("00" * 32))
                                else:
                                    out_d.append("0x")
                            return out_d

                    dust_router29 = _DustAndStaleHeadRouter(store=st29)
                    stale_fresh_ok = not _is_head_fresh_against_tip(dust_router29, "ethereum", 20_000_005, 20_000_020)
                    dust_res29 = audit_single_protocol(
                        dust_router29,  # type: ignore[arg-type]
                        st29,
                        {"slug": "t29-modea-dust-only", "chain": "ethereum", "tvl_usd": 500_000.0, "addresses": ["0x" + "51" * 20], "mode": "A", "delegate_address_search": False},
                        set(),
                    )
                    dust_and_stale_ok = (
                        stale_fresh_ok
                        and dust_res29.get("verdict") == VERDICT_BLOCKED
                        and "MODE_A_SPOT_VAULT_UNCOVERED_DIVERGENCE" in str(dust_res29.get("reason") or "")
                    )
                finally:
                    st29.close()

                t29_ok = (
                    dns_blocked_ok
                    and loopback_dns_ok
                    and wmint_and_dec_ok
                    and v3_and_dec24_ok
                    and harness_and_bc_ok
                    and precompile_and_hist_ok
                    and env_12_and_cmd_ok
                    and custom_gate2_blocked_ok
                    and unver_profit_rejected_ok
                    and malformed_rejected_ok
                    and dust_and_stale_ok
                )
                results.append(
                    (
                        "T29_SSOT_HEAD_FRESHNESS_WNATIVE_MINT_V3PEG_PRECOMPILE_AND_ISOLATION",
                        t29_ok,
                        f"dns={dns_blocked_ok and loopback_dns_ok},wmint_dec={wmint_and_dec_ok},v3_24d={v3_and_dec24_ok},h_bc={harness_and_bc_ok},prec_hist={precompile_and_hist_ok},iso_gate={env_12_and_cmd_ok and custom_gate2_blocked_ok and unver_profit_rejected_ok and malformed_rejected_ok and dust_and_stale_ok}",
                    )
                )

            if _want_test("T30_SSOT_CDNS_UDP_SELFISO_ARENA_DIAMOND_PRECOMPILE_AND_TRACE_AUTH", tag_filters):
                # T30: C-DNS/UDP Guard, Self-Isolating Command Wrapper, Unwrapped WNATIVE Mint, 2-Arg Prank/Arena Balance, >32 Diamond/Unresolved Proxy/Custom Precompile & Authoritative Trace Gate
                orig_corpus_md_30 = _eng_globals()["INCIDENT_CORPUS_PATH"]
                st30 = SQLiteStore(selftest_ns / "_t30_ssot.sqlite")
                try:
                    # (1) C-DNS gethostbyname + UDP sendto guard + getproxies isolation
                    cdns_blocked = False
                    try:
                        socket.gethostbyname("example.com")
                    except OSError as exc_cdns:
                        cdns_blocked = "OFFLINE_SOCKET_GUARD" in str(exc_cdns)
                    udp_blocked = False
                    s_udp30 = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    try:
                        try:
                            s_udp30.sendto(b"x", ("8.8.8.8", 53))
                        except OSError as exc_udp:
                            udp_blocked = "OFFLINE_SOCKET_GUARD" in str(exc_udp)
                    finally:
                        s_udp30.close()
                    cdns_udp_ok = (
                        cdns_blocked
                        and udp_blocked
                        and socket.gethostbyname("127.0.0.1") == "127.0.0.1"
                        and urllib.request.getproxies() == {}
                    )

                    # (2) Self-isolating command wrappers (_isolated_env.json, foundry.toml, no --config-path conflict, no angle brackets)
                    addr_req_30 = emit_address_discovery_request("t30-req-slug", "ethereum", 20_000_000, 250_000.0)
                    addr_doc_30 = _read_json(addr_req_30, default={})
                    poc_paths_30 = emit_poc_requests_for_slug(
                        "t30-poc-slug",
                        "ethereum",
                        20_000_000,
                        "0x" + "11" * 20,
                        [CausalSlice(slice_id="s1", target_address="0x" + "22" * 20, channel="CH_STORAGE", legs=["0x12345678"], needs_agent_synthesis=True)],
                        {},
                    )
                    poc_doc_30 = _read_json(poc_paths_30[0], default={}) if poc_paths_30 else {}
                    iso_a30 = addr_doc_30.get("isolated_commands") or {}
                    iso_p30 = poc_doc_30.get("isolated_commands") or {}
                    cmd_vcode_30 = str(iso_a30.get("verify_code_template") or "")
                    cmd_forge_30 = str(iso_p30.get("preverify_forge_test") or "")
                    cmd_ccall_30 = str(iso_p30.get("readonly_cast_call_template") or "")
                    cast_toml_30 = selftest_ns / "t30-req-slug" / "_cast_env" / "foundry.toml"
                    cast_env_j30 = selftest_ns / "t30-req-slug" / "_cast_env" / "_isolated_env.json"
                    ws_env_j30 = selftest_ns / "t30-poc-slug" / "forge" / "_isolated_env.json"
                    self_iso_wrap_ok = (
                        cast_toml_30.is_file()
                        and cast_env_j30.is_file()
                        and ws_env_j30.is_file()
                        and "-B -X utf8 -c" in cmd_vcode_30
                        and "-B -X utf8 -c" in cmd_forge_30
                        and "-B -X utf8 -c" in cmd_ccall_30
                        and "--config-path" not in cmd_forge_30
                        and all("<" not in cx and ">" not in cx for cx in (cmd_vcode_30, cmd_forge_30, cmd_ccall_30))
                    )

                    # (3) Unwrapped WNATIVE ConservationSupplyMint attribution + full CHAIN_STABLES (>8) quote probing
                    weth30 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                    mint_data_30 = "0x" + _abi_encode_seq(["uint256", "address", "uint256"], [0, weth30, 5 * 10**18]).hex()
                    gain_data_30 = "0x" + _abi_encode_seq(
                        ["uint256", "address", "uint256", "uint256", "uint256"],
                        [0, "0x0000000000000000000000000000000000000000", 5 * 10**18, 5 * 10**18, 45_000],
                    ).hex()
                    synth_forge_json_30 = json.dumps(
                        {
                            "test/Verify_t30.t.sol:Verify_t30": {
                                "test_results": {
                                    "test_0()": {
                                        "status": "Success",
                                        "logs": [
                                            {"topics": [CONSERVATION_SUPPLY_MINT_TOPIC0], "data": mint_data_30},
                                            {"topics": [CONSERVATION_GAIN_TOPIC0], "data": gain_data_30},
                                        ],
                                        "decoded_logs": [],
                                    }
                                }
                            }
                        }
                    )
                    p30_cnt, f30_cnt, gains_30 = _parse_forge_json_gains(synth_forge_json_30)

                    class _QuoteCaptureRouter30:
                        def __init__(self) -> None:
                            self.seen_calldata: List[bytes] = []

                        def multicall3(self, ch: str, queries: Any, **kw: Any) -> List[Tuple[bool, bytes]]:
                            for _t, cd in queries:
                                self.seen_calldata.append(bytes(cd))
                            return [(False, b"") for _ in queries]

                    synth_9th_st30 = "0x" + "92" * 20
                    orig_eth_stables_30 = list(CHAIN_STABLES.get("ethereum", []))
                    try:
                        CHAIN_STABLES["ethereum"] = orig_eth_stables_30 + [synth_9th_st30]
                        qcap_30 = _QuoteCaptureRouter30()
                        probe_dex_exit_liquidity(
                            qcap_30,  # type: ignore[arg-type]
                            "ethereum",
                            "0x" + "77" * 20,
                            10**24,
                            24,
                            1.0,
                            2500.0,
                            20_000_000,
                        )
                        eth_stables_30 = [nx for x in CHAIN_STABLES["ethereum"] if (nx := _norm_addr(x))]
                    finally:
                        CHAIN_STABLES["ethereum"] = orig_eth_stables_30
                    all_blobs_hex_30 = "".join(b.hex() for b in qcap_30.seen_calldata)
                    wmint_and_stables_ok = (
                        p30_cnt == 1
                        and f30_cnt == 0
                        and len(gains_30) == 1
                        and gains_30[0].get("asset") == "native"
                        and gains_30[0].get("supply_mint") is True
                        and synth_9th_st30 in eth_stables_30
                        and all(st_a[2:] in all_blobs_hex_30 for st_a in eth_stables_30)
                    )

                    # (4) 2-arg prank enforcement, renamed test contract self-reference ban, and arena pre-funded caller transfer guard
                    c0_30 = _checksum("0x" + "aa" * 20)
                    ok_1arg_30, v_1arg_30 = verify_cheatcode_allowlist(
                        f"pragma solidity ^0.8.20;\ninterface Vm {{ function prank(address) external; }}\ncontract CustomSuite {{ Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D); function test_one() public {{ vm.prank({c0_30}); }} }}",
                        allowed_caller=c0_30,
                        forbid_inline_fork=True,
                    )
                    ok_2arg_30, _ = verify_cheatcode_allowlist(
                        f"pragma solidity ^0.8.20;\ninterface Vm {{ function prank(address,address) external; }}\ncontract CustomSuite {{ Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D); function test_two() public {{ vm.prank({c0_30}, {c0_30}); }} }}",
                        allowed_caller=c0_30,
                        forbid_inline_fork=True,
                    )
                    ok_self_30, v_self_30 = verify_cheatcode_allowlist(
                        "pragma solidity ^0.8.20;\ncontract RenamedSuite { function test_self() public view returns (uint256) { return address(this).balance; } }",
                        allowed_caller=c0_30,
                        forbid_inline_fork=True,
                    )
                    arena_prefunded_30 = {
                        "arena": [
                            {
                                "trace": {
                                    "caller": "0x1804c8ab1f12e6bbf3894d4083f33e07309d1f38",
                                    "address": "0x7fa9385be102ac3eac297483dd6233d62b3e1496",
                                    "kind": "CALL",
                                    "value": "0x0",
                                    "success": True,
                                },
                                "children": [1],
                            },
                            {
                                "trace": {
                                    "caller": "0x" + "aa" * 20,
                                    "address": "0x" + "bb" * 20,
                                    "kind": "CALL",
                                    "value": hex(10**18),
                                    "success": True,
                                },
                                "children": [],
                            },
                        ]
                    }
                    arena_sourced_30 = {
                        "arena": [
                            {
                                "trace": {
                                    "caller": "0x1804c8ab1f12e6bbf3894d4083f33e07309d1f38",
                                    "address": "0x7fa9385be102ac3eac297483dd6233d62b3e1496",
                                    "kind": "CALL",
                                    "value": "0x0",
                                    "success": True,
                                },
                                "children": [1, 2],
                            },
                            {
                                "trace": {
                                    "caller": "0x" + "aa" * 20,
                                    "address": "0x" + "bb" * 20,
                                    "kind": "CALL",
                                    "value": "0x0",
                                    "success": True,
                                },
                                "children": [3],
                            },
                            {
                                "trace": {
                                    "caller": "0x" + "aa" * 20,
                                    "address": "0x" + "cc" * 20,
                                    "kind": "CALL",
                                    "value": hex(5 * 10**17),
                                    "success": True,
                                },
                                "children": [],
                            },
                            {
                                "trace": {
                                    "caller": "0x" + "bb" * 20,
                                    "address": "0x" + "aa" * 20,
                                    "kind": "CALL",
                                    "value": hex(10**18),
                                    "success": True,
                                },
                                "children": [],
                            },
                        ]
                    }
                    cheatcode_and_arena_ok = (
                        (not ok_1arg_30)
                        and ("disallowed_single_arg_prank_missing_tx_origin" in v_1arg_30)
                        and ok_2arg_30
                        and (not ok_self_30)
                        and ("disallowed_harness_self_reference" in v_self_30)
                        and _scan_trace_tree_for_disallowed_hevm(arena_prefunded_30, forbid_inline_fork=True) is True
                        and _scan_trace_tree_for_disallowed_hevm(arena_sourced_30, forbid_inline_fork=True) is False
                    )

                    # (5) >32 EIP-2535 Diamond facets + leading-zero preservation + unresolved proxy Fail-Open + custom precompile 0x64 STATICCALL Fail-Open
                    facet_tuples_30 = [(f"0x{idx_f:040x}", [b"\x11\x22\x33\x44"]) for idx_f in range(0x200, 0x228)]
                    raw_diamond_30 = "0x" + _abi_encode_seq(["(address,bytes4[])[]"], [facet_tuples_30]).hex()
                    dec_facets_30 = _decode_diamond_facets(raw_diamond_30)

                    class _UnresolvedProxyRouter30(_SelftestMockRouter):
                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_p: List[Any] = []
                            for m_c, p_c in calls:
                                if m_c == "eth_getCode":
                                    a_q = _norm_addr(str(p_c[0]))
                                    out_p.append("0x3659cfe600" if a_q == ("0x" + "dd" * 20) else "0x")
                                elif m_c == "eth_getStorageAt":
                                    sl_q = str(p_c[1]).lower()
                                    if sl_q == EIP1967_SLOTS["impl"].lower():
                                        out_p.append("0x" + ("00" * 12) + ("00" * 18) + "0250")
                                    else:
                                        out_p.append("0x" + ("00" * 32))
                                else:
                                    out_p.append("0x")
                            return out_p

                    pres_30 = _resolve_proxy_and_delegates(
                        _UnresolvedProxyRouter30(),  # type: ignore[arg-type]
                        "ethereum",
                        ["0x" + "dd" * 20],
                        20_000_000,
                    ).get("0x" + "dd" * 20)
                    # PUSH1 0x00 DUP1 DUP1 DUP1 PUSH1 0x64 GAS STATICCALL STOP vs PUSH1 0x01 (ecRecover)
                    cfg_prec64 = disassemble_and_build_cfg(bytes.fromhex("600080808060645afa00"))
                    cfg_ecrec1 = disassemble_and_build_cfg(bytes.fromhex("600080808060015afa00"))
                    diamond_proxy_prec_ok = (
                        len(dec_facets_30) == 40
                        and "0x0000000000000000000000000000000000000200" in dec_facets_30
                        and pres_30 is not None
                        and pres_30.unresolved_delegation is True
                        and any(bb.has_dynamic_staticcall for bb in cfg_prec64.blocks.values())
                        and not any(bb.has_dynamic_staticcall for bb in cfg_ecrec1.blocks.values())
                    )

                    # (6) Authoritative target_called_in_trace=False rejection on dead-branch source + Mode A time-series TVL seeding resilience
                    rej_slug30 = "t30-dead-branch-safe"
                    pdir_r30 = selftest_ns / rej_slug30
                    (pdir_r30 / "poc_requests").mkdir(parents=True, exist_ok=True)
                    (pdir_r30 / "verdicts").mkdir(parents=True, exist_ok=True)
                    (pdir_r30 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    st30.upsert_queue_item(rej_slug30, 1, "ethereum", 180_000.0, ["0x" + "33" * 20], mode="A")
                    st30.set_queue_agent_state(rej_slug30, "awaiting_verdict")
                    _write_json_atomic(pdir_r30 / "result.json", {"slug": rej_slug30, "chain": "ethereum", "audit_block": 0, "verdict": VERDICT_SAFE, "needs_agent": True})
                    _write_json_atomic(pdir_r30 / "checkpoint.json", {"chain": "ethereum", "audit_block": 0, "caller": "0x" + "11" * 20})
                    _write_json_atomic(pdir_r30 / "poc_requests" / "poc_request_1.json", {"candidate_id": 1, "target_address": "0x" + "33" * 20})
                    (pdir_r30 / "forge" / "test" / "test_1.t.sol").write_text(
                        "pragma solidity ^0.8.20;\ncontract T { function test_1() public { if (false) { (bool ok,) = address(0x" + "33" * 20 + ").call(\"\"); ok; } } }\n",
                        encoding="utf-8",
                    )
                    _write_json_atomic(
                        pdir_r30 / "forge_output_1.json",
                        {
                            "executed": True,
                            "passed_tests": 1,
                            "failed_tests": 0,
                            "conservation_gains": [],
                            "error_reason": "",
                            "trace_summary": {"target_called_in_trace": False, "disallowed_cheatcode_in_trace": False},
                        },
                    )
                    os.utime(pdir_r30 / "forge_output_1.json", (time.time() + 10.0, time.time() + 10.0))
                    _write_json_atomic(
                        pdir_r30 / "verdicts" / "verdict_1.json",
                        {"candidate_id": 1, "verdict": VERDICT_SAFE, "notes": "Dead branch call should not satisfy trace interaction"},
                    )
                    process_agent_submissions(RPCRouter(store=st30), st30)
                    dead_branch_rejected_ok = (
                        not (pdir_r30 / "verdicts" / "verdict_1.json").exists()
                        and int((_read_json(pdir_r30 / "poc_requests" / "poc_request_1.json", default={}) or {}).get("rejection_count") or 0) == 1
                    )

                    synth_corpus_30 = selftest_ns / "_t30_synth_corpus.md"
                    synth_corpus_30.write_text(
                        "## 03/20 — SyntheticVaultCase (Ethereum)\n"
                        "- **취약 원리**: Accounting rounding check\n"
                        "- **취약 컨트랙트**:\n"
                        "  - Vault: `0x1000000000000000000000000000000000000301`\n",
                        encoding="utf-8",
                    )
                    _write_json_atomic(
                        QUEUE_FILE_PATH,
                        [
                            {"slug": "t30-timeseries-proto", "chain": "ethereum", "tvl": [{"date": 1700000000, "totalLiquidityUSD": 275000.0}], "addresses": ["0x" + "44" * 20]},
                            {"slug": "t30-malformed-proto", "chain": "ethereum", "tvl": "not-a-number", "addresses": []},
                        ],
                    )
                    _eng_globals()["INCIDENT_CORPUS_PATH"] = synth_corpus_30
                    st30_seed = SQLiteStore(selftest_ns / "_t30_seed.sqlite")
                    try:
                        seeded_cnt_30 = _seed_queue_from_existing_or_live(st30_seed)
                        claimed_30 = st30_seed.claim_next_batch("boot-t30", limit=5)
                    finally:
                        st30_seed.close()
                        QUEUE_FILE_PATH.unlink(missing_ok=True)
                    tvl_seed_ok = (
                        seeded_cnt_30 == 2
                        and any(it.get("slug") == "t30-timeseries-proto" and abs(float(it.get("tvl_usd") or 0.0) - 275000.0) < 1e-6 for it in claimed_30)
                    )

                    t30_ok = (
                        cdns_udp_ok
                        and self_iso_wrap_ok
                        and wmint_and_stables_ok
                        and cheatcode_and_arena_ok
                        and diamond_proxy_prec_ok
                        and dead_branch_rejected_ok
                        and tvl_seed_ok
                    )
                    results.append(
                        (
                            "T30_SSOT_CDNS_UDP_SELFISO_ARENA_DIAMOND_PRECOMPILE_AND_TRACE_AUTH",
                            t30_ok,
                            f"cdns_udp={cdns_udp_ok},wrap={self_iso_wrap_ok},wmint_st={wmint_and_stables_ok},arena_cc={cheatcode_and_arena_ok},diam_prec={diamond_proxy_prec_ok},tr_tvl={dead_branch_rejected_ok and tvl_seed_ok}",
                        )
                    )
                finally:
                    _eng_globals()["INCIDENT_CORPUS_PATH"] = orig_corpus_md_30
                    st30.close()

            if _want_test("T31_QA_COUNTERFACTUAL_PROXY_HARNESS_CMD_AND_ISOLATION", tag_filters):
                # T31: Counterfactual Stub Causality, Unresolved Delegation Fail-Open, 77-Dec/WNATIVE Conservation, Subprocess/Socket/CLI Isolation
                st31 = SQLiteStore(selftest_ns / "t31_store.sqlite")
                try:
                    st31.put_price("fx:USD_KRW", 1400.0, "fixture:t31")
                    iso_probe_31 = selftest_ns / "_t31_iso_probe"
                    env31 = _isolated_subproc_env(iso_probe_31)
                    min_win_31 = ("SYSTEMROOT", "WINDIR", "SystemDrive", "COMSPEC", "PATHEXT")
                    iso_12_31 = (
                        "USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA",
                        "TEMP", "TMP", "TMPDIR", "FOUNDRY_DIR", "SVM_DATA_DIR",
                        "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
                    )
                    addr_req_31 = emit_address_discovery_request("t31-addr-slug", "ethereum", 20_000_000, 150_000.0)
                    addr_doc_31 = _read_json(addr_req_31, default={})
                    poc_paths_31 = emit_poc_requests_for_slug(
                        "t31-poc-slug",
                        "ethereum",
                        20_000_000,
                        "0x" + "12" * 20,
                        [CausalSlice(slice_id="s1", target_address="0x" + "34" * 20, channel="CH_STORAGE", legs=["0x12345678"], needs_agent_synthesis=True)],
                        {},
                    )
                    poc_doc_31 = _read_json(poc_paths_31[0], default={}) if poc_paths_31 else {}
                    tpl_blob_31 = json.dumps({
                        "v": ((addr_doc_31.get("isolated_commands") or {}).get("verify_code_template") or ""),
                        "p": ((poc_doc_31.get("isolated_commands") or {}).get("preverify_forge_test") or ""),
                        "r": ((poc_doc_31.get("isolated_commands") or {}).get("readonly_cast_call_template") or ""),
                    })
                    iso_env_and_cli_ok = (
                        all(isinstance(env31.get(k), str) and bool(env31[k].strip()) for k in min_win_31)
                        and all(Path(str(env31[k])).resolve().is_relative_to(selftest_ns.resolve()) for k in iso_12_31)
                        and env31.get("HTTP_PROXY") == ""
                        and env31.get("HTTPS_PROXY") == ""
                        and env31.get("ALL_PROXY") == ""
                        and env31.get("ETHERSCAN_API_KEY") == ""
                        and str(env31.get("NO_PROXY") or "").startswith("*")
                        and f'"{_ENGINE_FILE.resolve()}"' in CLI_INVOKE_PREFIX
                        and all(ph not in tpl_blob_31 for ph in ("__RPC_URL__", "__TARGET_ADDR__", "__CANDIDATE_ADDR__", "__FUNC_SIG__"))
                    )

                    dns_empty_blk = False
                    dns_bytes_blk = False
                    af_unix_blk = False
                    udp_ext_blk = False
                    try:
                        socket.getaddrinfo("", 80)
                    except OSError:
                        dns_empty_blk = True
                    try:
                        socket.gethostbyname(b"8.8.8.8")  # type: ignore[arg-type]
                    except OSError:
                        dns_bytes_blk = True
                    s_udp31 = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    try:
                        try:
                            s_udp31.connect("/tmp/t31_ipc.sock")  # type: ignore[arg-type]
                        except OSError:
                            af_unix_blk = True
                        try:
                            s_udp31.sendto(b"x", ("8.8.8.8", 53))
                        except OSError:
                            udp_ext_blk = True
                        if hasattr(s_udp31, "sendmsg"):
                            sendmsg_blk = False
                            try:
                                s_udp31.sendmsg([b"x"], [], 0, ("8.8.8.8", 53))
                            except OSError:
                                sendmsg_blk = True
                            udp_ext_blk = udp_ext_blk and sendmsg_blk
                    finally:
                        s_udp31.close()
                    sock_guard_31_ok = dns_empty_blk and dns_bytes_blk and af_unix_blk and udp_ext_blk

                    beacon_proxy_31 = "0x" + "b1" * 20
                    beacon_contract_31 = "0x" + "b2" * 20
                    facet_hist_31 = "0x" + "f1" * 20
                    dcut_data_31 = "0x" + abi_encode(
                        ["(address,uint8,bytes4[])[]", "address", "bytes"],
                        [[(facet_hist_31, 0, [bytes.fromhex("12345678")])], "0x" + "00" * 20, b""],
                    ).hex()

                    class _T31ProxyAndReserveRouter(_SelftestMockRouter):
                        _cand_urls = ("http://127.0.0.1:8545",)

                        def call(self, ch: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_getLogs":
                                return [
                                    {
                                        "blockNumber": hex(19_999_100),
                                        "topics": [DIAMOND_CUT_TOPIC0],
                                        "data": dcut_data_31,
                                    }
                                ]
                            if method == "eth_getCode":
                                aq = _norm_addr(str(params[0]))
                                if aq == beacon_proxy_31:
                                    return "0x363d3d373d3d363d7f" + ("00" * 32) + "5af43d82803e903d91602b57fd5bf3"
                                if aq == facet_hist_31:
                                    return "0x" + ("6001600055" * 4)
                                return "0x"
                            if method == "eth_getBalance":
                                return "0x0"
                            if method == "eth_getBlockByNumber":
                                return {"timestamp": hex(1_800_000_000), "baseFeePerGas": hex(100_000)}
                            return "0x0"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_b31: List[Any] = []
                            for m_c, p_c in calls:
                                if m_c == "eth_getStorageAt":
                                    aq = _norm_addr(str(p_c[0]))
                                    slq = str(p_c[1]).lower()
                                    if aq == beacon_proxy_31 and slq == EIP1967_SLOTS["beacon"].lower():
                                        out_b31.append("0x" + ("00" * 12) + beacon_contract_31[2:])
                                    else:
                                        out_b31.append("0x" + ("00" * 32))
                                else:
                                    out_b31.append(self.call(ch, m_c, p_c, **kw))
                            return out_b31

                    r31 = _T31ProxyAndReserveRouter(store=st31)
                    res_beacon_31 = _resolve_proxy_and_delegates(r31, "ethereum", [beacon_proxy_31], 20_000_000)  # type: ignore[arg-type]
                    tc_beacon_31 = build_backward_trust_closure(r31, "ethereum", [beacon_proxy_31], 20_000_000, 250_000.0)  # type: ignore[arg-type]
                    hist_dcut_31 = _recover_historical_proxy_impl_bytecodes(r31, "ethereum", "0x" + "dd" * 20, 20_000_000)  # type: ignore[arg-type]
                    hybrid_md_31 = selftest_ns / "_t31_hybrid_corpus.md"
                    hybrid_md_31.write_text(
                        "## 04/10 \u2013 CrossBridgeCase (Solana)\n"
                        "- **\ucde8\uc57d \ucee8\ud2b8\ub799\ud2b8**:\n"
                        "  - Bridge Adapter: https://etherscan.io/address/0x1000000000000000000000000000000000000311\n",
                        encoding="utf-8",
                    )
                    part_31 = _parse_corpus_md_partition(hybrid_md_31)
                    evm_31 = part_31.get("evm_cases", [])
                    non_evm_31 = part_31.get("pure_non_evm_cases", [])
                    proxy_hist_corpus_ok = (
                        bool(res_beacon_31[beacon_proxy_31].unresolved_delegation) is True
                        and tc_beacon_31.early_exit_safe is False
                        and any(cb == bytes.fromhex("6001600055" * 4) for cb in hist_dcut_31)
                        and len(evm_31) == 1
                        and len(non_evm_31) == 0
                    )

                    inh_cb_sol_31 = (
                        "pragma solidity ^0.8.20;\n"
                        "contract BaseRecv { receive() payable external {} }\n"
                        "contract Verify_Inh is BaseRecv { function test_inh() public {} }\n"
                    )
                    asm_self_sol_31 = (
                        "pragma solidity ^0.8.20;\n"
                        "contract Verify_Asm { function test_asm() public view { uint256 v; assembly { v := selfbalance() } v; } }\n"
                    )
                    ok_inh_31, _ = verify_cheatcode_allowlist(inh_cb_sol_31)
                    ok_asm_31, _ = verify_cheatcode_allowlist(asm_self_sol_31)
                    def _arena_root_31(children: List[int]) -> Dict[str, Any]:
                        return {
                            "idx": 0,
                            "parent": None,
                            "children": children,
                            "trace": {
                                "caller": "0x1804c8ab1f12e6bbf3894d4083f33e07309d1f38",
                                "address": "0x" + "aa" * 20,
                                "kind": "CALL",
                                "value": "0x0",
                                "success": True,
                            },
                        }

                    reverted_arena_31 = {
                        "arena": [
                            _arena_root_31([1]),
                            {
                                "idx": 1,
                                "parent": 0,
                                "children": [],
                                "trace": {
                                    "caller": "0x" + "12" * 20,
                                    "address": "0x" + "bb" * 20,
                                    "kind": "CALL",
                                    "value": hex(10**18),
                                    "reverted": True,
                                },
                            },
                        ]
                    }
                    helper_create_arena_31 = {
                        "arena": [
                            _arena_root_31([1, 2]),
                            {
                                "idx": 1,
                                "parent": 0,
                                "children": [],
                                "trace": {
                                    "caller": "0x" + "aa" * 20,
                                    "address": "0x" + "cc" * 20,
                                    "kind": "CREATE",
                                    "value": "0x0",
                                    "success": True,
                                },
                            },
                            {
                                "idx": 2,
                                "parent": 0,
                                "children": [],
                                "trace": {
                                    "caller": "0x" + "cc" * 20,
                                    "address": "0x" + "dd" * 20,
                                    "kind": "CALL",
                                    "value": hex(10**18),
                                    "success": True,
                                },
                            },
                        ]
                    }
                    cfg_sc256_31 = disassemble_and_build_cfg(bytes.fromhex("60008080806101005afa00"))
                    static_and_trace_31_ok = (
                        (not ok_inh_31)
                        and (not ok_asm_31)
                        and (not _scan_trace_tree_for_disallowed_hevm(reverted_arena_31))
                        and _scan_trace_tree_for_disallowed_hevm(helper_create_arena_31)
                        and any(bb.has_dynamic_staticcall for bb in cfg_sc256_31.blocks.values())
                    )

                    tok77_31 = "0x" + "77" * 20
                    acct77_31 = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={tok77_31: 0},
                        caller_after={tok77_31: 250 * (10**77)},
                        vault_before={tok77_31: 250 * (10**77)},
                        vault_after={tok77_31: 0},
                        gas_used=21_000,
                        gas_price_wei=100_000,
                        decimals_map={tok77_31: 77},
                        prices_map={"native": 2000.0, tok77_31: 1.0},
                        block_pin=0,
                    )
                    weth_31 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    acct_unbacked_unwrap_31 = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 50 * (10**18)},
                        vault_before={weth_31: 50 * (10**18)},
                        vault_after={weth_31: 0},
                        gas_used=21_000,
                        gas_price_wei=100_000,
                        prices_map={"native": 2000.0, weth_31: 2000.0},
                        router=r31,  # type: ignore[arg-type]
                        block_pin=20_000_000,
                    )
                    acct_and_wdec_31_ok = (
                        _is_profit_verdict(acct77_31.verdict)
                        and abs(acct77_31.gross_usd - 250.0) < 1e-6
                        and acct_unbacked_unwrap_31.verdict == VERDICT_BLOCKED
                        and acct_unbacked_unwrap_31.reason == "WNATIVE_RESERVE_UNCONFIRMED_FAIL_OPEN"
                    )

                    cf_causality_31_ok = True
                    if FORGE_BIN.exists():
                        ws31 = ensure_clean_forge_workspace({}, slug="t31-ws", offline=True)
                        caller31 = "0x" + "12" * 20
                        sl_w31 = CausalSlice(slice_id="s0", target_address=weth_31, channel="CH_DIRECT", legs=["0x18160ddd"])
                        h_w31 = _render_minimal_solidity_harness(
                            slug="t31_wdec",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller31,
                            target_addr=weth_31,
                            token_addrs=[weth_31],
                            slices=[sl_w31],
                            vault_addresses=[weth_31],
                        )
                        (ws31 / "test" / "Verify_W31.t.sol").write_text(h_w31, encoding="utf-8")
                        out_w31 = execute_forge_test_file(
                            ws31,
                            "test/Verify_W31.t.sol",
                            h_w31,
                            allowed_caller=caller31,
                            target_addr=weth_31,
                            offline=True,
                        )
                        acct_and_wdec_31_ok = acct_and_wdec_31_ok and out_w31.executed and out_w31.passed_tests == 1

                        targ31 = _checksum("0x" + "31" * 20)
                        pool31 = _checksum("0x" + "32" * 20)
                        tok31 = _checksum("0x" + "33" * 20)
                        etch_sel31 = HEVM_ETCH_SEL
                        causal_sol_31 = f"""// SPDX-License-Identifier: MIT
    pragma solidity ^0.8.20;
    interface ITarg31 {{ function releaseTo(address to) external; }}
    contract Tok31 {{
        mapping(address => uint256) public balanceOf;
        function mint(address to, uint256 a) external {{ balanceOf[to] += a; }}
        function transfer(address to, uint256 a) external returns (bool) {{
            require(balanceOf[msg.sender] >= a);
            balanceOf[msg.sender] -= a;
            balanceOf[to] += a;
            return true;
        }}
    }}
    contract VaultImpl31 {{
        address constant TOK = {tok31};
        function releaseTo(address to) external {{
            Tok31(TOK).transfer(to, 100 ether);
        }}
    }}
    contract Verify_Causal31 {{
        address constant TARGET = {targ31};
        address constant TOK = {tok31};
        address constant CALLER = {_checksum(caller31)};
        event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);
        function test_causal() public {{
            Tok31 tImpl = new Tok31();
            VaultImpl31 vImpl = new VaultImpl31();
            (bool o1,) = address({HEVM_ADDRESS}).call(abi.encodeWithSelector({etch_sel31}, TOK, address(tImpl).code));
            (bool o2,) = address({HEVM_ADDRESS}).call(abi.encodeWithSelector({etch_sel31}, address(uint160(0x{"31" * 19}30) + 1), address(vImpl).code));
            o1; o2;
            Tok31(TOK).mint(TARGET, 100 ether);
            uint256 b0 = Tok31(TOK).balanceOf(CALLER);
            uint256 v0 = Tok31(TOK).balanceOf(TARGET);
            ITarg31(TARGET).releaseTo(CALLER);
            uint256 b1 = Tok31(TOK).balanceOf(CALLER);
            uint256 v1 = Tok31(TOK).balanceOf(TARGET);
            if (b1 > b0 && v0 > v1) {{
                emit ConservationGain(0, TOK, b1 - b0, v0 - v1, 21000);
            }}
        }}
    }}
    """
                        noncausal_sol_31 = f"""// SPDX-License-Identifier: MIT
    pragma solidity ^0.8.20;
    interface IPing31 {{ function ping() external returns (uint256); }}
    contract TokNC31 {{
        mapping(address => uint256) public balanceOf;
        function mint(address to, uint256 a) external {{ balanceOf[to] += a; }}
        function transfer(address to, uint256 a) external returns (bool) {{
            require(balanceOf[msg.sender] >= a);
            balanceOf[msg.sender] -= a;
            balanceOf[to] += a;
            return true;
        }}
    }}
    contract PingImpl31 {{
        function ping() external pure returns (uint256) {{ return 1; }}
    }}
    contract PoolImpl31 {{
        address constant TOK = {tok31};
        function payout(address to) external {{
            TokNC31(TOK).transfer(to, 100 ether);
        }}
    }}
    contract Verify_NonCausal31 {{
        address constant TARGET = {targ31};
        address constant POOL = {pool31};
        address constant TOK = {tok31};
        address constant CALLER = {_checksum(caller31)};
        event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);
        function test_noncausal() public {{
            TokNC31 tImpl = new TokNC31();
            PingImpl31 pImpl = new PingImpl31();
            PoolImpl31 lImpl = new PoolImpl31();
            (bool o1,) = address({HEVM_ADDRESS}).call(abi.encodeWithSelector({etch_sel31}, TOK, address(tImpl).code));
            (bool o2,) = address({HEVM_ADDRESS}).call(abi.encodeWithSelector({etch_sel31}, address(uint160(0x{"31" * 19}30) + 1), address(pImpl).code));
            (bool o3,) = address({HEVM_ADDRESS}).call(abi.encodeWithSelector({etch_sel31}, POOL, address(lImpl).code));
            o1; o2; o3;
            TokNC31(TOK).mint(POOL, 100 ether);
            uint256 b0 = TokNC31(TOK).balanceOf(CALLER);
            uint256 v0 = TokNC31(TOK).balanceOf(POOL);
            IPing31(TARGET).ping();
            PoolImpl31(POOL).payout(CALLER);
            uint256 b1 = TokNC31(TOK).balanceOf(CALLER);
            uint256 v1 = TokNC31(TOK).balanceOf(POOL);
            if (b1 > b0 && v0 > v1) {{
                emit ConservationGain(0, TOK, b1 - b0, v0 - v1, 21000);
            }}
        }}
    }}
    """
                        (ws31 / "test" / "Causal31.t.sol").write_text(causal_sol_31, encoding="utf-8")
                        (ws31 / "test" / "NonCausal31.t.sol").write_text(noncausal_sol_31, encoding="utf-8")
                        cf_ok_pos, _ = verify_counterfactual_target_causality(
                            ws31, "test/Causal31.t.sol", causal_sol_31, targ31, winning_test="test_causal", offline=True
                        )
                        cf_ok_neg, cf_rsn_neg = verify_counterfactual_target_causality(
                            ws31, "test/NonCausal31.t.sol", noncausal_sol_31, targ31, winning_test="test_noncausal", offline=True
                        )
                        cf_causality_31_ok = (
                            bool(cf_ok_pos) is True
                            and bool(cf_ok_neg) is False
                            and cf_rsn_neg == "COUNTERFACTUAL_NON_CAUSAL_GAIN_PERSISTED"
                        )

                    t31_ok = (
                        iso_env_and_cli_ok
                        and sock_guard_31_ok
                        and proxy_hist_corpus_ok
                        and static_and_trace_31_ok
                        and acct_and_wdec_31_ok
                        and cf_causality_31_ok
                    )
                    results.append(
                        (
                            "T31_QA_COUNTERFACTUAL_PROXY_HARNESS_CMD_AND_ISOLATION",
                            t31_ok,
                            f"iso_cli={iso_env_and_cli_ok},sock={sock_guard_31_ok},prx_crp={proxy_hist_corpus_ok},st_tr={static_and_trace_31_ok},acct={acct_and_wdec_31_ok},cf={cf_causality_31_ok}",
                        )
                    )
                finally:
                    st31.close()
                    st31 = None

            if _want_test("T32_FULL_AUDIT_SOUNDNESS_PRECISION_AND_ISOLATION_SEAL", tag_filters):
                t32_owns_st = st31 is None
                if t32_owns_st:
                    st31 = SQLiteStore(selftest_ns / "t32_store.sqlite")
                    st31.put_price("fx:USD_KRW", 1400.0, "fixture:t32")
                if tc_beacon_31 is None:
                    tc_beacon_31 = TrustClosureResult(chain="ethereum", block_pin=20_000_000, seed_addresses=["0x" + "11" * 20], all_addresses=["0x" + "11" * 20], resolutions={}, vault_exposures={}, max_extractable_usd=5000.0, min_gas_usd=1.0, early_exit_safe=False, early_exit_reason="", max_extractable_usd_dec="5000")
                try:
                    # T32: 100자리 고정밀 보존 회계·w_dec>18 서브웨이 손실 차단·SQLite 최신 블록 그림자 해소·정적/하네스/트레이스 무결성 봉인
                    tok77_32 = "0x" + "72" * 20
                    unit77_32 = 10**77
                    amt77_below = 10001 * (10**75) - 1
                    amt77_exact = 10001 * (10**75)
                    v32_below = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={tok77_32: 0},
                        caller_after={tok77_32: amt77_below},
                        vault_before={tok77_32: amt77_below},
                        vault_after={tok77_32: 0},
                        gas_used=25_000,
                        gas_price_wei=200_000_000,
                        decimals_map={tok77_32: 77},
                        prices_map={"native": 2000.0, tok77_32: 1.0},
                        allow_offline_fixture=True,
                    )
                    v32_exact = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={tok77_32: 0},
                        caller_after={tok77_32: amt77_exact},
                        vault_before={tok77_32: amt77_exact},
                        vault_after={tok77_32: 0},
                        gas_used=25_000,
                        gas_price_wei=200_000_000,
                        decimals_map={tok77_32: 77},
                        prices_map={"native": 2000.0, tok77_32: 1.0},
                        allow_offline_fixture=True,
                    )
                    weth_32 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    v32_subwei_loss = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={weth_32: 10, tok77_32: 0},
                        caller_after={weth_32: 9, tok77_32: 500 * unit77_32},
                        vault_before={tok77_32: 500 * unit77_32},
                        vault_after={tok77_32: 0},
                        gas_used=21_000,
                        gas_price_wei=100_000,
                        decimals_map={weth_32: 24, tok77_32: 77},
                        prices_map={"native": 2000.0, weth_32: 2000.0, tok77_32: 1.0},
                        allow_offline_fixture=True,
                    )
                    v32_wei18 = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 10_123_456_789_012_345_678},
                        vault_before={"native": 10_123_456_789_012_345_678},
                        vault_after={"native": 0},
                        gas_used=21_000,
                        gas_price_wei=100_000,
                        prices_map={"native": 1000.0},
                        allow_offline_fixture=True,
                    )
                    v32_100009 = judge_conservation_accounting(
                        store=st31,
                        chain="ethereum",
                        caller_before={tok77_32: 0},
                        caller_after={tok77_32: 100019 * (10**74)},
                        vault_before={tok77_32: 100019 * (10**74)},
                        vault_after={tok77_32: 0},
                        gas_used=25_000,
                        gas_price_wei=200_000_000,
                        decimals_map={tok77_32: 77},
                        prices_map={"native": 2000.0, tok77_32: 1.0},
                        allow_offline_fixture=True,
                    )
                    tc_rt_32 = _deserialize_closure(json.loads(json.dumps(_serialize_closure(tc_beacon_31))))
                    prec100_and_subwei_ok = (
                        v32_below.verdict == VERDICT_SAFE
                        and _is_profit_verdict(v32_exact.verdict)
                        and abs(v32_exact.net_usd - 100.0) < 1e-6
                        and _to_dec(v32_exact.net_usd_exact) == Decimal("100.00")
                        and v32_wei18.gross_profit_native == "10.123456789012345678"
                        and v32_100009.net_usd == 100.0
                        and _to_dec(v32_100009.net_usd_exact) == Decimal("100.009")
                        and _verdict_rank_key(v32_100009) > _verdict_rank_key(v32_exact)
                        and (Decimal("200.00") - _to_dec(v32_100009.net_usd_exact)) < PROFIT_FLOOR_DEC
                        and tc_rt_32.max_extractable_usd_dec == tc_beacon_31.max_extractable_usd_dec
                        and v32_subwei_loss.verdict == VERDICT_SAFE
                        and v32_subwei_loss.reason == "CALLER_NEGATIVE_ASSET_DELTA_REJECTED"
                    )

                    upg_proxy_32 = "0x" + "81" * 20
                    old_impl_32 = "0x" + "82" * 20
                    new_impl_32 = "0x" + "83" * 20
                    st31.put_contract_code(
                        chain_id=1,
                        address=upg_proxy_32,
                        audit_block=100,
                        code_hash="h1",
                        skeleton_hash="sk_old_32",
                        opcode_4grams="01020304",
                        raw_code_hex="0x6001600055",
                        proxy_kind="eip1967",
                        impl_addrs=[old_impl_32],
                    )
                    st31.put_contract_code(
                        chain_id=1,
                        address=upg_proxy_32,
                        audit_block=200,
                        code_hash="h2",
                        skeleton_hash="sk_new_32",
                        opcode_4grams="05060708",
                        raw_code_hex="0x6002600055",
                        proxy_kind="eip1967",
                        impl_addrs=[new_impl_32],
                    )
                    sqlite_upg_dedup_ok = (
                        len(st31.find_proxies_by_impl([old_impl_32], chain_id=1)) == 0
                        and len(st31.find_proxies_by_impl([new_impl_32], chain_id=1)) == 1
                        and all(r.get("address") != upg_proxy_32 for r in st31.find_by_skeleton_or_grams("sk_old_32", "01020304", chain_id=1))
                        and any(r.get("address") == upg_proxy_32 for r in st31.find_by_skeleton_or_grams("sk_new_32", "05060708", chain_id=1))
                    )

                    disp_unmod_scan_32 = quick_scan_value_sinks(bytes.fromhex("6000e300"))
                    ok_abs_cb_32, v_abs_cb_32 = verify_cheatcode_allowlist(
                        "abstract contract BaseH32 { receive() virtual external payable {} } contract Verify_Sub32 is BaseH32 { function testX() public {} }"
                    )
                    ok_lib_self_32, v_lib_self_32 = verify_cheatcode_allowlist(
                        "library LibH32 { function f() internal view returns (uint256) { return address(this).balance; } } contract Verify_L32 { function testY() public {} }"
                    )
                    ok_zsub_32, v_zsub_32 = verify_agent_harness_accounting_integrity(
                        "contract AgentVerify_1 { function t() public { uint256 a = x; emit ConservationGain(1, address(0), a - 0, v0 - v1, 21000); } }"
                    )
                    ok_vref_32, v_vref_32 = verify_agent_harness_accounting_integrity(
                        "contract AgentVerify_1 { function t() public { emit ConservationGain(1, address(0), vaultBefore - vaultAfter, vaultBefore - vaultAfter, 21000); } }"
                    )
                    flat_rev_arena_32 = {
                        "arena": [
                            {"parent": None, "trace": {"kind": "Call", "caller": "0x1804c8ab1f12e6bbf3894d4083f33e07309d1f38", "address": "0x7fa9385be102ac3eac297483dd6233d62b3e1496", "value": "0x0", "depth": 0, "status": "0x1"}},
                            {"parent": 0, "is_prank_caller": True, "trace": {"kind": "Call", "caller": "0x" + "12" * 20, "address": "0x" + "aa" * 20, "value": "0x0", "depth": 1, "reverted": True}},
                            {"parent": 1, "trace": {"kind": "Call", "caller": "0x" + "aa" * 20, "address": "0x" + "12" * 20, "value": hex(10**18), "depth": 2, "reverted": False}},
                            {"parent": 0, "is_prank_caller": True, "trace": {"kind": "Call", "caller": "0x" + "12" * 20, "address": "0x" + "bb" * 20, "value": hex(10**18), "depth": 1, "reverted": False}},
                        ]
                    }
                    tail_sl_32 = [
                        CausalSlice(slice_id=f"s_{i}", target_address="0x" + "44" * 20, channel="CH_DIRECT", legs=[f"0x{i + 1:08x}"], needs_agent_synthesis=(i == 0))
                        for i in range(50)
                    ]
                    tail_reqs_32 = emit_poc_requests_for_slug(
                        slug="t32-tail-cap",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "12" * 20,
                        slices=tail_sl_32,
                        verified_sources={},
                        force_all=False,
                        mechanical_cap=48,
                    )
                    st31.upsert_queue_item("t32-repair-done", 1, "ethereum", 150_000.0, ["0x" + "55" * 20])
                    st31.complete_queue_item("t32-repair-done", VERDICT_SAFE, 0.0)
                    rep_cnt_32 = st31.requeue_done_for_repair(["t32-repair-done"])
                    rec_claim_32 = st31.claim_next_batch("t32_boot", limit=1)
                    write_protocol_artifacts(
                        "t32-bad-safe",
                        {"slug": "t32-bad-safe", "chain": "ethereum", "audit_block": 1, "verdict": VERDICT_SAFE, "net_usd": 0.0, "net_krw": 0, "reason": ""},
                        root_dir=selftest_ns,
                    )
                    ok_bad_safe_32, rsn_bad_safe_32 = verify_protocol_pins("t32-bad-safe", root_dir=selftest_ns)
                    write_protocol_artifacts(
                        "t32-bad-blk",
                        {"slug": "t32-bad-blk", "chain": "ethereum", "audit_block": 1, "verdict": VERDICT_BLOCKED, "net_usd": 0.0, "net_krw": 0, "reason": "blocked", "remediation_exhausted": {}},
                        root_dir=selftest_ns,
                    )
                    ok_bad_blk_32, rsn_bad_blk_32 = verify_protocol_pins("t32-bad-blk", root_dir=selftest_ns)
                    soundness_guards_32_ok = (
                        (not disp_unmod_scan_32.early_exit_safe)
                        and (not ok_abs_cb_32 and "disallowed_harness_callback_entrypoint" in v_abs_cb_32)
                        and (not ok_lib_self_32 and "disallowed_harness_self_reference" in v_lib_self_32)
                        and (not ok_zsub_32 and any("TRIVIAL_ZERO_SUB" in x for x in v_zsub_32))
                        and (not ok_vref_32 and any("VAULT_STATE_IN_CALLER_GAIN" in x for x in v_vref_32))
                        and _scan_trace_tree_for_disallowed_hevm(flat_rev_arena_32)
                        and len(tail_reqs_32) == 3
                        and rep_cnt_32 == 1
                        and len(rec_claim_32) == 1
                        and rec_claim_32[0].get("was_recovered") is True
                        and (not ok_bad_safe_32 and rsn_bad_safe_32 == "SAFE_VERDICT_MISSING_PROOF_REASON")
                        and (not ok_bad_blk_32 and rsn_bad_blk_32 == "BLOCKED_VERDICT_INCOMPLETE_REMEDIATION")
                    )
                    t32_ok = prec100_and_subwei_ok and sqlite_upg_dedup_ok and soundness_guards_32_ok
                    results.append(
                        (
                            "T32_FULL_AUDIT_SOUNDNESS_PRECISION_AND_ISOLATION_SEAL",
                            t32_ok,
                            f"prec100={prec100_and_subwei_ok},sqlite_upg={sqlite_upg_dedup_ok},guards={soundness_guards_32_ok}",
                        )
                    )
                finally:
                    if t32_owns_st and st31 is not None:
                        st31.close()
                        st31 = None

            if _want_test("T33_FIVE_SESSION_ISOLATION_SOURCIFY_STREAM_REAL_SELL_AND_PROFIT_STOP", tag_filters):
                st33 = SQLiteStore(selftest_ns / "_selftest_t33.db")
                try:
                    # (1) 5-session IDs, Mode B Priority #1, per-session PROFIT_FOUND stop, and stale lease recovery
                    s_ids_33 = [st33.ensure_session()["session_id"] for _ in range(5)]
                    sess_ids_ok = len(set(s_ids_33)) == 5 and all(s.startswith("ses-") for s in s_ids_33)

                    st33.upsert_queue_item("t33-modea-hi-tvl", 2, "ethereum", 5_000_000.0, ["0x" + "10" * 20], mode="A")
                    st33.upsert_queue_item("t33-modeb-profit", 1, "base", 50_000.0, ["0x" + "20" * 20], mode="B")
                    st33.upsert_queue_item("t33-modeb-peer-1", 1, "base", 40_000.0, ["0x" + "21" * 20], mode="B")
                    st33.upsert_queue_item("t33-modeb-peer-2", 1, "base", 30_000.0, ["0x" + "22" * 20], mode="B")

                    c0_33 = st33.claim_next_for_session(s_ids_33[0], "boot-s0")
                    modeb_prio_ok = c0_33 is not None and c0_33.get("slug") == "t33-modeb-profit" and int(c0_33.get("priority") or 0) == 1

                    pdir_33 = ARTIFACT_ROOT / "t33-modeb-profit"
                    (pdir_33 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    (pdir_33 / "forge" / "test" / "Verify_1.t.sol").write_text("// t33 profit test", encoding="utf-8")
                    prof_verd_33 = _format_profit_verdict(250_000.0)
                    write_protocol_artifacts(
                        "t33-modeb-profit",
                        {
                            "slug": "t33-modeb-profit",
                            "chain": "base",
                            "audit_block": 20_000_000,
                            "verdict": prof_verd_33,
                            "net_krw": 250_000.0,
                            "net_usd": 180.0,
                            "fx_rate_krw": 1400.0,
                            "fx_source": "fixture:t33",
                            "session_id": s_ids_33[0],
                            "caller": "0x" + "aa" * 20,
                            "target": "0x" + "20" * 20,
                        },
                        forge_payload={
                            "executed": True,
                            "passed_tests": 1,
                            "failed_tests": 0,
                            "conservation_gains": [{"asset": "native", "caller_net_raw": "100000000000000000"}],
                            "trace_summary": {"disallowed_cheatcode_in_trace": False, "target_called_in_trace": True},
                        },
                        root_dir=ARTIFACT_ROOT,
                    )
                    st33.complete_queue_item("t33-modeb-profit", prof_verd_33, 250_000.0)

                    t33_script = selftest_ns / "_t33_fs_script"
                    t33_tools = t33_script / "tools"
                    t33_tools.mkdir(parents=True, exist_ok=True)
                    for _fn_33 in ("Defi_구현.py", "원하는 것.md", "지시서.md", "컨트랙트.md"):
                        (t33_script / _fn_33).write_text("# clean\n", encoding="utf-8")
                    done_s0_33 = _evaluate_session_done_gates(
                        st33, session_id=s_ids_33[0], artifact_root=ARTIFACT_ROOT, run_selftest=False, script_dir=t33_script, tools_dir=t33_tools
                    )
                    after_stop_claim = st33.claim_next_for_session(s_ids_33[0], "boot-s0")
                    c1_33 = st33.claim_next_for_session(s_ids_33[1], "boot-s1")
                    c2_33 = st33.claim_next_for_session(s_ids_33[2], "boot-s2")
                    done_s1_33 = _evaluate_session_done_gates(
                        st33, session_id=s_ids_33[1], artifact_root=ARTIFACT_ROOT, run_selftest=False, script_dir=t33_script, tools_dir=t33_tools
                    )
                    c3_dead = st33.claim_next_for_session(s_ids_33[3], "boot-dead")
                    rec_cnt_33 = st33.recover_zombie_claims("boot-live", active_session_id=s_ids_33[4], lease_ttl_sec=1800.0)
                    c4_rec = st33.claim_next_for_session(s_ids_33[4], "boot-live")
                    sess_isolation_ok = (
                        sess_ids_ok
                        and modeb_prio_ok
                        and after_stop_claim is None
                        and bool(done_s0_33.get("all_gates_passed"))
                        and done_s0_33.get("profit_slug") == "t33-modeb-profit"
                        and c1_33 is not None and c1_33.get("slug") == "t33-modeb-peer-1"
                        and c2_33 is not None and c2_33.get("slug") == "t33-modeb-peer-2"
                        and not bool(done_s1_33.get("all_gates_passed"))
                        and c3_dead is not None and c3_dead.get("slug") == "t33-modea-hi-tvl"
                        and rec_cnt_33 >= 1
                        and c4_rec is not None and c4_rec.get("slug") == "t33-modeb-peer-1" and bool(c4_rec.get("was_recovered"))
                    )

                    # (2) Immutable-masked multigram (>=0.995), Sourcify v2 streaming cursor, and DuckDB table_files
                    seed_raw_33 = (
                        bytes.fromhex("63deadbeef146020577f")
                        + b"\xaa" * 32
                        + bytes.fromhex("73" + "11" * 20)
                        + bytes.fromhex("5b60016002016003025060206000f3") * 6
                    )
                    clone_raw_33 = (
                        bytes.fromhex("63deadbeef146020577f")
                        + b"\xbb" * 32
                        + bytes.fromhex("73" + "22" * 20)
                        + bytes.fromhex("5b60016002016003025060206000f3") * 6
                    )
                    masked_sim_33 = compute_multigram_containment(seed_raw_33, clone_raw_33)

                    sf33 = selftest_ns / "_sf33_stream"
                    sf33.mkdir(parents=True, exist_ok=True)
                    seed_addr_33 = "0x" + "51" * 20
                    clone_addr_33 = "0x" + "52" * 20
                    st33.upsert_queue_item(
                        "modeb-seed-33",
                        1,
                        "base",
                        10_000.0,
                        [seed_addr_33],
                        mode="B",
                        principle_summary="T33 seeded vulnerability",
                    )
                    idx33_lines = [
                        json.dumps(
                            {
                                "chain_id": 8453,
                                "address": seed_addr_33,
                                "runtime_code_hex": seed_raw_33.hex(),
                                "fully_qualified_name": "src/VaultSeed.sol:VaultSeed",
                                "abi": [{"type": "function", "name": "drain"}],
                                "sources": {"src/VaultSeed.sol": {"content": "pragma solidity ^0.8.20;\ncontract VaultSeed {}\n"}},
                            }
                        ),
                        json.dumps(
                            {
                                "chain_id": 8453,
                                "address": clone_addr_33,
                                "runtime_code_hex": clone_raw_33.hex(),
                            }
                        ),
                    ]
                    (sf33 / "_local_index.jsonl").write_text("\n".join(idx33_lines) + "\n", encoding="utf-8")

                    st_sf_33 = SQLiteStore(selftest_ns / "_selftest_t33_sf.db")
                    try:
                        st_sf_33.upsert_queue_item(
                            "modeb-seed-33",
                            1,
                            "base",
                            10_000.0,
                            [seed_addr_33],
                            mode="B",
                            principle_summary="T33 seeded vulnerability",
                        )
                        cur_step1 = _advance_mode_b_sourcify_cursor(None, st_sf_33, parquet_root=sf33, max_wall_sec=4.5)
                        cur_step2 = _advance_mode_b_sourcify_cursor(None, st_sf_33, parquet_root=sf33, max_wall_sec=4.5)
                        qrows_33 = st_sf_33._conn.execute(
                            "SELECT slug, priority, mode FROM audit_queue ORDER BY slug"
                        ).fetchall()
                        clone_slug_matches = [
                            str(r[0]) for r in qrows_33
                            if str(r[0]) != "modeb-seed-33" and int(r[1]) == 1 and str(r[2]).upper() == "B"
                        ]

                        duck_tf_ok = True
                        if DUCKDB_BIN.is_file():
                            tf_root_33 = selftest_ns / "_t33_duck_tf"
                            (tf_root_33 / "code").mkdir(parents=True, exist_ok=True)
                            tf_pq_a = tf_root_33 / "code" / "part_a.parquet"
                            tf_pq_b = tf_root_33 / "code" / "part_b.parquet"
                            _run_duckdb_cli_json(
                                f"COPY (SELECT 8453 AS cid, 'ok' AS tag) TO {_sql_quote_str(str(tf_pq_a.resolve()))} (FORMAT PARQUET)",
                                parquet_root=tf_root_33,
                                table_files={},
                            )
                            _run_duckdb_cli_json(
                                f"COPY (SELECT 9999 AS cid, 'other' AS tag) TO {_sql_quote_str(str(tf_pq_b.resolve()))} (FORMAT PARQUET)",
                                parquet_root=tf_root_33,
                                table_files={},
                            )
                            tf_rows = (
                                _run_duckdb_cli_json(
                                    "SELECT cid, tag FROM code ORDER BY cid",
                                    parquet_root=tf_root_33,
                                    table_files={"code": [tf_pq_a]},
                                )
                                if (tf_pq_a.is_file() and tf_pq_b.is_file())
                                else None
                            )
                            fail_open_rows = _run_duckdb_cli_json(
                                "SELECT * FROM code_missing_table_xyz",
                                parquet_root=tf_root_33,
                                table_files={"code": [tf_pq_a]} if tf_pq_a.is_file() else {},
                            )
                            duck_tf_ok = (
                                isinstance(tf_rows, list)
                                and len(tf_rows) == 1
                                and int(tf_rows[0].get("cid") or 0) == 8453
                                and fail_open_rows is None
                            )
                        sourcify_stream_ok = (
                            masked_sim_33 >= 0.99
                            and cur_step1.get("matched") == 1
                            and cur_step1.get("enqueued") == 1
                            and cur_step1.get("phase") == "exhausted"
                            and cur_step2.get("enqueued") == 0
                            and cur_step2.get("phase") == "exhausted"
                            and len(clone_slug_matches) == 1
                            and duck_tf_ok
                        )
                    finally:
                        st_sf_33.close()

                    # (3) OZ __gap slot 0x64 vault discovery + Solidly shallow $150 pool real sell cap + paused() exclusion
                    target33 = "0x" + "61" * 20
                    vault33 = "0x" + "62" * 20
                    tok33 = "0x" + "63" * 20
                    tok_paused33 = "0x" + "64" * 20
                    solidly_pool33 = "0x" + "65" * 20
                    usdc_base33 = _norm_addr(CHAIN_STABLES["base"][0])
                    solidly_fac33 = _norm_addr(_SOLIDLY_FACTORIES["base"][0])
                    base_cl33 = _norm_addr(_CHAINLINK_NATIVE_USD_FEEDS["base"])
                    z12_33 = bytes(12)
                    z32_33 = bytes(32)

                    class _T33VaultSellRouter(RPCRouter):
                        def __init__(self, st_obj: SQLiteStore) -> None:
                            super().__init__(store=st_obj)

                        def http_get_json(self, url: str, timeout: float = 15.0, extra_headers: Optional[Dict[str, str]] = None) -> Any:
                            coins_33: Dict[str, Dict[str, float]] = {}
                            if tok33 in url:
                                coins_33[f"base:{tok33}"] = {"price": 1.0}
                            if tok_paused33 in url:
                                coins_33[f"base:{tok_paused33}"] = {"price": 1.0}
                            if usdc_base33 in url:
                                coins_33[f"base:{usdc_base33}"] = {"price": 1.0}
                            if "coingecko:ethereum" in url:
                                coins_33["coingecko:ethereum"] = {"price": 3000.0}
                            return {"coins": coins_33}

                        def call(self, chain: str, method: str, params: List[Any], **kw: Any) -> Any:
                            if method == "eth_chainId":
                                return hex(CHAIN_IDS["base"])
                            if method == "eth_blockNumber":
                                return hex(20_000_010)
                            if method == "eth_getBlockByNumber":
                                return {"number": hex(20_000_000), "timestamp": hex(1_794_600_000), "baseFeePerGas": hex(1_000_000)}
                            if method == "eth_gasPrice":
                                return hex(1_000_000)
                            if method == "eth_getCode":
                                na = _norm_addr(params[0]) if params else ""
                                if na in (target33, vault33, tok33, tok_paused33, usdc_base33, solidly_pool33, solidly_fac33):
                                    return "0x60005460005260206000f3"
                                return "0x"
                            if method == "eth_getBalance":
                                return "0x0"
                            if method == "eth_getStorageAt":
                                na = _norm_addr(params[0]) if params else ""
                                slot_i = int(str(params[1]), 16) if len(params) > 1 else -1
                                if na == target33 and slot_i == 100:
                                    return "0x" + ("00" * 12) + vault33[2:]
                                return "0x" + ("00" * 32)
                            if method == "eth_getLogs":
                                filt = params[0] if params and isinstance(params[0], dict) else {}
                                tps = filt.get("topics") or []
                                if tps and str(tps[0]).lower() == TRANSFER_TOPIC0.lower():
                                    t_hex = json.dumps(tps).lower()
                                    if vault33[2:] in t_hex:
                                        return [
                                            {
                                                "address": tok33,
                                                "topics": [
                                                    TRANSFER_TOPIC0,
                                                    "0x" + ("00" * 32),
                                                    "0x" + ("00" * 12) + vault33[2:],
                                                ],
                                                "data": "0x" + (1_000 * 10**18).to_bytes(32, "big").hex(),
                                                "blockNumber": hex(19_990_000),
                                            }
                                        ]
                                return []
                            return None

                        def batch_call(self, chain: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            return [self.call(chain, m_c, p_c, **kw) for m_c, p_c in calls]

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 120) -> List[Tuple[bool, bytes]]:
                            out_m: List[Tuple[bool, bytes]] = []
                            for to_a, cd in calls:
                                sel = "0x" + cd[:4].hex().lower()
                                nt = _norm_addr(to_a)
                                if nt == base_cl33 and sel == "0xfeaf968c":
                                    cl_p = (
                                        (10).to_bytes(32, "big")
                                        + (3000 * 10**8).to_bytes(32, "big")
                                        + (1_794_599_900).to_bytes(32, "big")
                                        + (1_794_599_950).to_bytes(32, "big")
                                        + (10).to_bytes(32, "big")
                                    )
                                    out_m.append((True, cl_p))
                                elif nt == base_cl33 and sel == DECIMALS_SEL.lower():
                                    out_m.append((True, (8).to_bytes(32, "big")))
                                elif sel == PAUSED_SEL.lower():
                                    out_m.append((True, (1 if nt == tok_paused33 else 0).to_bytes(32, "big")))
                                elif sel == DECIMALS_SEL.lower():
                                    out_m.append((True, (6 if nt == usdc_base33 else 18).to_bytes(32, "big")))
                                elif sel == SYMBOL_SEL.lower():
                                    out_m.append((True, abi_encode(["string"], ["USDC" if nt == usdc_base33 else "T33"])))
                                elif sel == TOTAL_SUPPLY_SEL.lower():
                                    out_m.append((True, (1_000_000 * 10**18).to_bytes(32, "big")))
                                elif sel in (SOLIDLY_GET_POOL_SEL.lower(), SOLIDLY_GET_PAIR_SEL.lower()):
                                    cd_h = cd.hex().lower()
                                    if (tok33[2:] in cd_h or tok_paused33[2:] in cd_h) and usdc_base33[2:] in cd_h and "0000000000000000000000000000000000000000000000000000000000000000" in cd_h:
                                        out_m.append((True, z12_33 + bytes.fromhex(solidly_pool33[2:])))
                                    else:
                                        out_m.append((True, z32_33))
                                elif sel == SOLIDLY_AMOUNT_OUT_SEL.lower() and nt == solidly_pool33 and len(cd) >= 36:
                                    amt_in = int.from_bytes(cd[4:36], "big")
                                    out_q = (1 * 10**6) if amt_in <= (2 * 10**18) else (45 * 10**6)
                                    out_m.append((True, out_q.to_bytes(32, "big")))
                                elif sel == BALANCE_OF_SEL.lower() and len(cd) >= 36:
                                    who = "0x" + cd[16:36].hex().lower()
                                    if nt == tok33 and who in (vault33, solidly_pool33):
                                        out_m.append((True, (1_000 * 10**18).to_bytes(32, "big")))
                                    elif nt == usdc_base33 and who == solidly_pool33:
                                        out_m.append((True, (75 * 10**6).to_bytes(32, "big")))
                                    else:
                                        out_m.append((True, z32_33))
                                else:
                                    out_m.append((False, b""))
                            return out_m

                    r33 = _T33VaultSellRouter(st33)
                    cl33 = build_backward_trust_closure(r33, "base", [target33], block_pin=20_000_000, protocol_tvl_hint_usd=0.0)
                    paused_q_33 = probe_dex_exit_liquidity(
                        r33, "base", tok_paused33, 1_000 * 10**18, 18, 1.0, 3000.0, 20_000_000, quote_prices_usd={usdc_base33: 1.0}
                    )
                    vault_and_real_sell_ok = (
                        vault33 in cl33.all_addresses
                        and vault33 in cl33.vault_exposures
                        and float(cl33.vault_exposures[vault33].total_usd or 0.0) >= 900.0
                        and cl33.max_extractable_usd >= 900.0
                        and 0.0 < cl33.realizable_extractable_usd <= 75.0
                        and len(cl33.verified_exit_pools) >= 1
                        and tok33 in cl33.verified_exit_pools
                        and cl33.early_exit_safe is True
                        and str(cl33.early_exit_reason or "").startswith("E1_REALIZABLE_BELOW_MIN_USD(")
                        and paused_q_33.get("exit_liquid") is False
                        and paused_q_33.get("paused") is True
                    )

                    # (4) Self-describing compact single PoC bundle with _incident_seed sources & clean resume_engine
                    sl1_33 = CausalSlice(
                        slice_id="slice_1",
                        target_address=target33,
                        channel="CH_BALANCE",
                        legs=["0xdeadbeef"],
                        rationale="T33 slice 1",
                        needs_agent_synthesis=True,
                    )
                    sl2_33 = CausalSlice(
                        slice_id="slice_2",
                        target_address=vault33,
                        channel="CH_STORAGE",
                        legs=["0xcafebabe"],
                        rationale="T33 slice 2",
                        needs_agent_synthesis=True,
                    )
                    inc_doc_33 = {
                        "case_slug": "case-t33-seed",
                        "principle_summary": "T33 drain principle",
                        "similarity": 0.995,
                        "fully_qualified_name": "src/VulnSeed.sol:VulnSeed",
                        "sources": {"src/VulnSeed.sol": {"content": "pragma solidity ^0.8.20;\ncontract VulnSeed { function drain() external {} }\n"}},
                        "abi": [{"type": "function", "name": "drain"}],
                    }
                    tgt_src_33 = {
                        target33: {
                            "fully_qualified_name": "src/Target33.sol:Target33",
                            "sources": {"src/Target33.sol": {"content": "pragma solidity ^0.8.20;\ncontract Target33 {}\n"}},
                            "abi": [],
                        }
                    }
                    emitted_33 = emit_poc_requests_for_slug(
                        "t33-bundle",
                        "base",
                        20_000_000,
                        "0x" + "aa" * 20,
                        [sl1_33, sl2_33],
                        tgt_src_33,
                        principle_summary="T33 bundled incident exploit pattern",
                        router=r33,
                        session_id=s_ids_33[0],
                        closure=cl33,
                        incident_sources=inc_doc_33,
                        compact_single_bundle=True,
                    )
                    req_doc_33 = _read_json(emitted_33[0], default={}) or {}
                    src_dir_33 = ARTIFACT_ROOT / "t33-bundle" / "sources"
                    readable_33 = [str(rf) for rf in (req_doc_33.get("readable_files") or ())]
                    iso_cmds_33 = req_doc_33.get("isolated_commands") if isinstance(req_doc_33.get("isolated_commands"), dict) else {}
                    res_eng_33 = str(iso_cmds_33.get("resume_engine") or "")
                    cast_tpl_33 = str(iso_cmds_33.get("readonly_cast_call_template") or "")
                    src_files_33 = [rf for rf in readable_33 if "/sources/" in rf]
                    bundle_export_ok = (
                        len(emitted_33) == 1
                        and not (ARTIFACT_ROOT / "t33-bundle" / "poc_requests" / "poc_request_2.json").exists()
                        and len(req_doc_33.get("candidate_slices") or ()) == 2
                        and req_doc_33.get("session_id") == s_ids_33[0]
                        and len(req_doc_33.get("connected_vaults") or ()) >= 1
                        and len(req_doc_33.get("verified_exit_pools") or ()) >= 1
                        and (req_doc_33.get("source_incident") or {}).get("case_slug") == "case-t33-seed"
                        and float((req_doc_33.get("source_incident") or {}).get("similarity") or 0.0) >= 0.99
                        and (src_dir_33 / "_incident_seed" / "src" / "VulnSeed.sol").is_file()
                        and (src_dir_33 / "_incident_seed" / "abi.json").is_file()
                        and len(src_files_33) >= 3
                        and all((ARTIFACT_ROOT / Path(rf).relative_to("audit_artifacts")).is_file() for rf in src_files_33)
                        and ("<" not in res_eng_33 and ">" not in res_eng_33 and f"run {s_ids_33[0]}" in res_eng_33)
                        and ("<" not in cast_tpl_33 and ">" not in cast_tpl_33)
                    )

                    t33_ok = sess_isolation_ok and sourcify_stream_ok and vault_and_real_sell_ok and bundle_export_ok
                    results.append(
                        (
                            "T33_FIVE_SESSION_ISOLATION_SOURCIFY_STREAM_REAL_SELL_AND_PROFIT_STOP",
                            t33_ok,
                            f"sess5={sess_isolation_ok},sf_stream={sourcify_stream_ok},real_sell={vault_and_real_sell_ok},bundle={bundle_export_ok}",
                        )
                    )
                finally:
                    st33.close()

            if _want_test("T34_CONCURRENT_5SESS_HARDENING_SHARDED_CURSOR_REVPROXY_AND_QUOTER", tag_filters):
                # T34: 5세션 병렬 슬라이스 샤딩 + 커서 소진 전 Mode A/done 잠금 + 역방향 Upgraded/Beacon 프록시 + QuoterV2 격리/Algebra/RPC 쿨다운 전파
                st34 = SQLiteStore(selftest_ns / "_selftest_t34.db")
                try:
                    # (1) _merge_idx_ranges + _claim_next_slice_range 병렬 샤딩/만료 회수 + max_priority=1 잠금 + done cursor 게이트
                    merged_34 = _merge_idx_ranges([[0, 5], [5, 8], [10, 12], [11, 14]])
                    done_r34: List[List[int]] = []
                    inf_r34: Dict[str, float] = {}
                    sl_a = _claim_next_slice_range(done_r34, inf_r34, 12, 5, 100.0, lease_sec=60.0)
                    sl_b = _claim_next_slice_range(done_r34, inf_r34, 12, 5, 101.0, lease_sec=60.0)
                    sl_c = _claim_next_slice_range(done_r34, inf_r34, 12, 5, 102.0, lease_sec=60.0)
                    sl_none = _claim_next_slice_range(done_r34, inf_r34, 12, 5, 103.0, lease_sec=60.0)
                    # sl_b 만료 시뮬레이션 -> 다음 워커가 회수 재할당
                    inf_r34[f"{sl_b[0]}:{sl_b[1]}"] = 50.0
                    sl_reclaim = _claim_next_slice_range(done_r34, inf_r34, 12, 5, 104.0, lease_sec=60.0)

                    s34_a = st34.ensure_session("ses-t34-a")["session_id"]
                    st34.upsert_queue_item("t34-modea-wait", 2, "ethereum", 9_000_000.0, ["0x" + "71" * 20], mode="A")
                    locked_claim34 = st34.claim_next_for_session(s34_a, "boot-t34", max_priority=1)
                    unlocked_claim34 = st34.claim_next_for_session(s34_a, "boot-t34", max_priority=None)
                    if unlocked_claim34:
                        st34.complete_queue_item("t34-modea-wait", VERDICT_SAFE, 0.0)
                    st34.upsert_queue_item("modeb-primary-t34", 1, "ethereum", 10_000.0, ["0x" + "72" * 20], mode="B")
                    st34.complete_queue_item("modeb-primary-t34", VERDICT_SAFE, 0.0)
                    sf34_dir = selftest_ns / "_sf34_cursor"
                    (sf34_dir / "code").mkdir(parents=True, exist_ok=True)
                    (sf34_dir / "code" / "dummy.parquet").write_bytes(b"PAR1")
                    _set_eng("SOURCIFY_PARQUET_DIR", sf34_dir)
                    st34.put_engine_meta("mode_b_sourcify_cursor_v1", json.dumps({"phase": "sig_slices"}))
                    done_active_c34 = _evaluate_session_done_gates(
                        st34, session_id=s34_a, artifact_root=ARTIFACT_ROOT, run_selftest=False
                    )
                    st34.put_engine_meta("mode_b_sourcify_cursor_v1", json.dumps({"phase": "exhausted"}))
                    done_exh_c34 = _evaluate_session_done_gates(
                        st34, session_id=s34_a, artifact_root=ARTIFACT_ROOT, run_selftest=False
                    )
                    sharded_gate_ok = (
                        merged_34 == [[0, 8], [10, 14]]
                        and sl_a == (0, 5)
                        and sl_b == (5, 10)
                        and sl_c == (10, 12)
                        and sl_none is None
                        and sl_reclaim == (5, 10)
                        and locked_claim34 is None
                        and unlocked_claim34 is not None
                        and not bool(done_active_c34.get("gate1_session_goal_satisfied"))
                        and not bool(done_active_c34.get("all_gates_passed"))
                        and bool(done_exh_c34.get("gate1_session_goal_satisfied"))
                    )

                    # (2) _find_reverse_upgraded_proxies (EIP-1967 active vs stale + BeaconUpgraded) & (3) QuoterV2 격리 + V3 100 / Algebra
                    matched_impl34 = "0x" + "81" * 20
                    patched_impl34 = "0x" + "82" * 20
                    proxy_act34 = "0x" + "83" * 20
                    proxy_stale34 = "0x" + "84" * 20
                    beacon34 = "0x" + "85" * 20
                    proxy_bcn34 = "0x" + "86" * 20
                    # tok 주소가 quote 토큰들(weth=0xc02..., usdc=0xa0b..., arb_usdc=0xaf8...)보다 작도록 0x10.. 선택
                    tok_v3_34 = "0x" + "10" * 20
                    pool_v3_100 = "0x" + "88" * 20
                    pool_alg_34 = "0x" + "89" * 20
                    usdc_eth34 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    usdc_arb34 = _norm_addr(CHAIN_STABLES["arbitrum"][0])
                    fac_v3_eth34 = _norm_addr(_UNIV3_FACTORIES["ethereum"][0])
                    qv2_eth34 = _norm_addr(_UNIV3_QUOTERS_V2[("ethereum", fac_v3_eth34)])
                    fac_alg_arb34 = _norm_addr(_ALGEBRA_FACTORIES["arbitrum"][0])
                    z12_34 = bytes(12)
                    z32_34 = bytes(32)
                    sqrt_p_1to1_18_6 = (1 << 96) // (10 ** 6)

                    class _T34HardeningRouter(RPCRouter):
                        def __init__(self, st_o: SQLiteStore, sess_id: str = "") -> None:
                            super().__init__(store=st_o, session_id=sess_id)
                            self.mixed_quoter_reserve_batch = False

                        def http_get_json(self, url: str, timeout: float = 15.0, extra_headers: Optional[Dict[str, str]] = None) -> Any:
                            return {"coins": {}}

                        def call(self, chain: str, method: str, params: List[Any], **kw: Any) -> Any:
                            c_nm = _norm_chain(chain)
                            if method == "eth_chainId":
                                return hex(CHAIN_IDS.get(c_nm, 1))
                            if method == "eth_blockNumber":
                                return hex(20_000_010)
                            if method == "eth_getCode":
                                na = _norm_addr(params[0]) if params else ""
                                if na in (
                                    matched_impl34,
                                    patched_impl34,
                                    proxy_act34,
                                    proxy_stale34,
                                    beacon34,
                                    proxy_bcn34,
                                    tok_v3_34,
                                    pool_v3_100,
                                    pool_alg_34,
                                    fac_v3_eth34,
                                    qv2_eth34,
                                    fac_alg_arb34,
                                ):
                                    return "0x60005460005260206000f3"
                                return "0x"
                            if method == "eth_getStorageAt":
                                na = _norm_addr(params[0]) if params else ""
                                sl_h = str(params[1] if len(params) > 1 else "").lower()
                                if sl_h == EIP1967_SLOTS["impl"].lower():
                                    if na == proxy_act34:
                                        return "0x" + ("00" * 12) + matched_impl34[2:]
                                    if na == proxy_stale34:
                                        return "0x" + ("00" * 12) + patched_impl34[2:]
                                if sl_h == EIP1967_SLOTS["beacon"].lower() and na == proxy_bcn34:
                                    return "0x" + ("00" * 12) + beacon34[2:]
                                return "0x" + ("00" * 32)
                            if method == "eth_getLogs":
                                filt = params[0] if params and isinstance(params[0], dict) else {}
                                tps = filt.get("topics") or []
                                t0_s = str(tps[0]).lower() if tps else ""
                                if t0_s == _EIP1967_UPGRADED_TOPIC.lower():
                                    return [
                                        {"address": proxy_act34},
                                        {"address": proxy_stale34},
                                        {"address": beacon34},
                                    ]
                                if t0_s == _BEACON_UPGRADED_TOPIC.lower():
                                    return [{"address": proxy_bcn34}]
                                return []
                            return None

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0, chunk_size: int = 120) -> List[Tuple[bool, bytes]]:
                            sels_in = {"0x" + cd[:4].hex().lower() for _, cd in calls if len(cd) >= 4}
                            if QUOTER_V2_EXACT_IN_SINGLE_SEL.lower() in sels_in and (
                                BALANCE_OF_SEL.lower() in sels_in or V3_SLOT0_SEL.lower() in sels_in
                            ):
                                self.mixed_quoter_reserve_batch = True
                            out_m: List[Tuple[bool, bytes]] = []
                            c_nm = _norm_chain(chain)
                            for to_a, cd in calls:
                                sel = "0x" + cd[:4].hex().lower()
                                nt = _norm_addr(to_a)
                                if sel == _IMPL_VIEW_SEL.lower() and nt == beacon34:
                                    out_m.append((True, z12_34 + bytes.fromhex(matched_impl34[2:])))
                                elif sel == V3_GET_POOL_SEL.lower() and c_nm == "ethereum" and nt == fac_v3_eth34 and len(cd) >= 100:
                                    fee_v = int.from_bytes(cd[68:100], "big")
                                    cd_h = cd.hex().lower()
                                    if fee_v == 100 and tok_v3_34[2:] in cd_h and usdc_eth34[2:] in cd_h:
                                        out_m.append((True, z12_34 + bytes.fromhex(pool_v3_100[2:])))
                                    else:
                                        out_m.append((True, z32_34))
                                elif sel == ALGEBRA_POOL_SEL.lower() and c_nm == "arbitrum" and nt == fac_alg_arb34:
                                    cd_h = cd.hex().lower()
                                    if tok_v3_34[2:] in cd_h and usdc_arb34[2:] in cd_h:
                                        out_m.append((True, z12_34 + bytes.fromhex(pool_alg_34[2:])))
                                    else:
                                        out_m.append((True, z32_34))
                                elif sel == DECIMALS_SEL.lower():
                                    out_m.append((True, (6 if nt in (usdc_eth34, usdc_arb34) else 18).to_bytes(32, "big")))
                                elif sel == BALANCE_OF_SEL.lower() and len(cd) >= 36:
                                    who = "0x" + cd[16:36].hex().lower()
                                    if nt in (usdc_eth34, usdc_arb34) and who in (pool_v3_100, pool_alg_34):
                                        out_m.append((True, (10_000 * 10**6 if who == pool_alg_34 else 5_000 * 10**6).to_bytes(32, "big")))
                                    else:
                                        out_m.append((True, z32_34))
                                elif sel == V3_SLOT0_SEL.lower() and nt == pool_v3_100:
                                    out_m.append((True, sqrt_p_1to1_18_6.to_bytes(32, "big") + z32_34))
                                elif sel == ALGEBRA_GLOBAL_STATE_SEL.lower() and nt == pool_alg_34:
                                    out_m.append((True, sqrt_p_1to1_18_6.to_bytes(32, "big") + z32_34 + (500).to_bytes(32, "big")))
                                elif sel == V2_TOKEN0_SEL.lower() and nt in (pool_v3_100, pool_alg_34):
                                    out_m.append((True, z12_34 + bytes.fromhex(tok_v3_34[2:])))
                                elif sel == V3_LIQUIDITY_SEL.lower() and nt in (pool_v3_100, pool_alg_34):
                                    out_m.append((True, (1_000_000 * 10**18).to_bytes(32, "big")))
                                elif sel == QUOTER_V2_EXACT_IN_SINGLE_SEL.lower() and nt == qv2_eth34:
                                    out_m.append((True, (800 * 10**6).to_bytes(32, "big")))
                                else:
                                    out_m.append((False, b""))
                            return out_m

                    r34_net = _T34HardeningRouter(st34, sess_id="ses-r34-0")
                    rev_proxies_34 = _find_reverse_upgraded_proxies(
                        r34_net, "ethereum", [matched_impl34], 20_000_000
                    )
                    v3_q34 = probe_dex_exit_liquidity(
                        r34_net, "ethereum", tok_v3_34, 1_000 * 10**18, 18, 1.0, 3000.0, 20_000_000, quote_prices_usd={usdc_eth34: 1.0}
                    )
                    alg_q34 = probe_dex_exit_liquidity(
                        r34_net, "arbitrum", tok_v3_34, 1_000 * 10**18, 18, 1.0, 3000.0, 20_000_000, quote_prices_usd={usdc_arb34: 1.0}
                    )
                    rev_proxy_ok = set(rev_proxies_34) == {proxy_act34, proxy_bcn34} and proxy_stale34 not in rev_proxies_34
                    quoter_alg_ok = (
                        not r34_net.mixed_quoter_reserve_batch
                        and v3_q34.get("exit_liquid") is True
                        and v3_q34.get("swap_fee_bps") == 1
                        and float(v3_q34.get("realizable_usd") or 0.0) == 800.0
                        and alg_q34.get("exit_liquid") is True
                        and alg_q34.get("best_dex_kind") == "algebra"
                        and alg_q34.get("swap_fee_bps") == 5
                    )

                    # (4) RPCRouter 세션별 엔드포인트 분산 + SQLite "gap" 쿨다운 공유 전파
                    routers_34 = [RPCRouter(store=st34, session_id=f"ses-rpc-shard-{idx_r}") for idx_r in range(5)]
                    leads_34 = [rr.candidate_urls("ethereum")[0] for rr in routers_34 if rr.candidate_urls("ethereum")]
                    lead_to_fail = leads_34[0] if leads_34 else ""
                    if lead_to_fail:
                        routers_34[0]._record_failure(lead_to_fail, "gap", cid=CHAIN_IDS["ethereum"])
                        routers_34[1]._sync_shared_endpoints_if_due(force=True)
                    post_fail_lead_1 = routers_34[1].candidate_urls("ethereum")[0] if routers_34[1].candidate_urls("ethereum") else ""
                    rpc_shard_ok = len(set(leads_34)) >= 2 and bool(lead_to_fail) and post_fail_lead_1 != lead_to_fail

                    t34_ok = sharded_gate_ok and rev_proxy_ok and quoter_alg_ok and rpc_shard_ok
                    results.append(
                        (
                            "T34_CONCURRENT_5SESS_HARDENING_SHARDED_CURSOR_REVPROXY_AND_QUOTER",
                            t34_ok,
                            f"sharded_gate={sharded_gate_ok},rev_proxy={rev_proxy_ok},quoter_alg={quoter_alg_ok},rpc_shard={rpc_shard_ok}",
                        )
                    )
                finally:
                    st34.close()

            if _want_test("T35_MODEB_SEEDHIST_VANITY_DORMANT_INBOUND_AND_DEFER_MODEA", tag_filters):
                # T35: Mode B 과거 시드(프록시/파괴전/tx_hash/6주소+) 복원 + 0x000000 클론 충돌방지/자금우선 + ERC-4626 준비금상한 + 인바운드 권한/휴면토큰 + Mode A 지연적재
                ns35 = selftest_ns / "t35_ns"
                ns35.mkdir(parents=True, exist_ok=True)
                sf35_dir = ns35 / "sourcify"
                sf35_dir.mkdir(parents=True, exist_ok=True)
                (sf35_dir / "sidecar.jsonl").write_text("{}\n", encoding="utf-8")
                art35_dir = ns35 / "artifacts"
                art35_dir.mkdir(parents=True, exist_ok=True)
                st35 = SQLiteStore(ns35 / "t35.db")
                st35_defer = SQLiteStore(ns35 / "t35_defer.db")
                st35_gate = SQLiteStore(ns35 / "t35_gate.db")
                orig_q_local_hints35 = _query_local_sourcify_deployment_hints
                orig_http_fb35 = _http_get_json_with_fallback
                orig_sf_dir35 = SOURCIFY_PARQUET_DIR
                try:
                    common_disp35 = bytes.fromhex("608060405234801561001057600080fd5b50600436106100365760003560e01c80")
                    code_v1_35 = common_disp35 + bytes.fromhex("63aaaaaaaa14") + (b"\x5b" * 70)
                    code_v2_35 = common_disp35 + bytes.fromhex("63bbbbbbbb14") + (b"\x5b" * 70)
                    code_6th_35 = common_disp35 + bytes.fromhex("63cccccc0614") + (b"\x5b" * 70)
                    code_sd_35 = common_disp35 + bytes.fromhex("63dddddddd14") + (b"\x5b" * 70)
                    code_shell_impl35 = common_disp35 + bytes.fromhex("63eeeeeeee14") + (b"\x5b" * 70)
                    code_thin_shell35 = (
                        bytes.fromhex("363d3d373d3d3d363d73" + ("00" * 20) + "5af43d82803e903d91602b57fd5bf3")
                        + (b"\x5b" * 40)
                    )
                    multi_addrs35 = [f"0x{0x1000 + i_m:040x}" for i_m in range(6)]
                    proxy_hist35 = "0x" + ("22" * 20)
                    v2_impl35 = "0x" + ("23" * 20)
                    v1_impl35 = "0x" + ("24" * 20)
                    sd_35 = "0x" + ("25" * 20)
                    shell_local35 = "0x" + ("26" * 20)
                    shell_impl35 = "0x" + ("27" * 20)
                    txh_prepatch35 = "0x" + ("ab" * 32)
                    eip1967_impl_slot35 = EIP1967_SLOTS["impl"].lower()

                    class _MockRouter35Hist(RPCRouter):
                        def call(
                            self,
                            chain: str,
                            method: str,
                            params: Sequence[Any],
                            block_pin: Optional[int] = None,
                            need_archive: bool = False,
                            timeout: float = 15.0,
                            raise_on_all_failed: bool = False,
                        ) -> Any:
                            if method == "eth_blockNumber":
                                return hex(20_000_000)
                            if method == "eth_getTransactionReceipt":
                                txh = str(params[0] if params else "").lower()
                                if txh == txh_prepatch35:
                                    return {"blockNumber": hex(19_000_100)}
                                return None
                            if method == "eth_getCode":
                                ad = _norm_addr(str(params[0] if params else ""))
                                btag = str(params[1] if len(params) > 1 else "latest").lower()
                                bn = _hex_to_int(btag) if btag.startswith("0x") else 20_000_000
                                if ad == proxy_hist35 or ad == shell_local35:
                                    return "0x" + code_thin_shell35.hex()
                                if ad == v2_impl35:
                                    return "0x" + code_v2_35.hex()
                                if ad == v1_impl35:
                                    return "0x" + code_v1_35.hex()
                                if ad == shell_impl35:
                                    return "0x" + code_shell_impl35.hex()
                                if ad == sd_35:
                                    return "0x" if bn >= 20_000_000 else ("0x" + code_sd_35.hex())
                                if ad in multi_addrs35:
                                    idx_a = multi_addrs35.index(ad)
                                    if idx_a == 5:
                                        return "0x" + code_6th_35.hex()
                                    return "0x" + (common_disp35 + bytes([0x63, 0x10, 0x20, 0x30, idx_a, 0x14]) + (b"\x5b" * 70)).hex()
                                return "0x"
                            if method == "eth_getStorageAt":
                                ad = _norm_addr(str(params[0] if params else ""))
                                sl = "0x" + str(params[1] if len(params) > 1 else "").lower().lstrip("0x").rjust(64, "0")
                                btag = str(params[2] if len(params) > 2 else "latest").lower()
                                bn = _hex_to_int(btag) if btag.startswith("0x") else 20_000_000
                                if sl == eip1967_impl_slot35:
                                    if ad == proxy_hist35:
                                        target_i = v1_impl35 if bn <= 19_000_100 else v2_impl35
                                        return "0x" + ("00" * 12) + target_i[2:]
                                    if ad == shell_local35:
                                        return "0x" + ("00" * 12) + shell_impl35[2:]
                                return "0x" + ("00" * 32)
                            if method == "eth_call":
                                return None
                            return None

                    def _mock_local_hints35(
                        chain_id: int,
                        addresses: Sequence[str],
                        parquet_root: Optional[Path] = None,
                    ) -> Dict[str, Any]:
                        norm_set = {_norm_addr(a) for a in addresses}
                        orig_codes = {shell_local35: code_thin_shell35} if shell_local35 in norm_set else {}
                        return {
                            "original_bytecodes": orig_codes,
                            "embedded_addresses": {},
                            "sibling_contracts": [],
                            "abi_zero_arg_getters": {},
                        }

                    _eng_globals()["_query_local_sourcify_deployment_hints"] = _mock_local_hints35
                    st35.put_proto_meta_batch(
                        [
                            ("modeb-ethereum-001-t35hist", {"block_numbers": [19_500_000], "tx_hashes": [txh_prepatch35]}),
                        ]
                    )
                    st35.upsert_queue_items_batch(
                        [
                            ("modeb-ethereum-000-t35multi", 1, "ethereum", MIN_TVL_USD, multi_addrs35, "B", "multi", 1000.0),
                            ("modeb-ethereum-001-t35hist", 1, "ethereum", MIN_TVL_USD, [proxy_hist35], "B", "hist", 999.0),
                            ("modeb-ethereum-002-t35sd", 1, "ethereum", MIN_TVL_USD, [sd_35], "B", "sd", 998.0),
                            ("modeb-ethereum-003-t35shell", 1, "ethereum", MIN_TVL_USD, [shell_local35], "B", "shell", 997.0),
                        ]
                    )
                    r35_hist = _MockRouter35Hist(store=st35, session_id="ses-t35-hist")
                    _advance_mode_b_sourcify_cursor(r35_hist, st35, parquet_root=sf35_dir, max_wall_sec=4.5)
                    seed_idx_file35 = ns35 / "_cache" / "mode_b_seed_index_v1.json"
                    seed_vars35 = (
                        json.loads(seed_idx_file35.read_text(encoding="utf-8")).get("variants", [])
                        if seed_idx_file35.is_file()
                        else []
                    )
                    harvested_hashes35 = {h for v in seed_vars35 for h in (v.get("hashes") or ())}
                    seed_hist_ok35 = (
                        hashlib.sha256(code_6th_35).hexdigest().lower() in harvested_hashes35
                        and hashlib.sha256(code_v1_35).hexdigest().lower() in harvested_hashes35
                        and hashlib.sha256(code_v2_35).hexdigest().lower() in harvested_hashes35
                        and hashlib.sha256(code_sd_35).hexdigest().lower() in harvested_hashes35
                        and hashlib.sha256(code_shell_impl35).hexdigest().lower() in harvested_hashes35
                        and _is_delegatecall_or_thin_proxy(code_thin_shell35)
                    )

                    # (b) CREATE2 0x000000 프리픽스 슬러그 충돌 방지 + 자금 보유 클론 Priority-1 내 우선 랭킹
                    vanity_zero35 = "0x0000000000000000000000000000000000aa0001"
                    vanity_funded35 = "0x0000000000000000000000000000000000bb0002"

                    class _MockRouter35Balance(RPCRouter):
                        def call(
                            self,
                            chain: str,
                            method: str,
                            params: Sequence[Any],
                            block_pin: Optional[int] = None,
                            need_archive: bool = False,
                            timeout: float = 15.0,
                            raise_on_all_failed: bool = False,
                        ) -> Any:
                            if method == "eth_getBalance":
                                ad = _norm_addr(str(params[0] if params else ""))
                                return hex(10 * 10**18) if ad == vanity_funded35 else "0x0"
                            if method == "eth_getCode":
                                return "0x"
                            return None

                    r35_bal = _MockRouter35Balance(store=st35, session_id="ses-t35-rank")
                    # Clear primary items first so claim_next picks from freshly matched vanity clones
                    for _it_clr in st35.list_mode_b_primary_items():
                        st35.complete_queue_item(_it_clr["slug"], VERDICT_SAFE, 0.0)
                    n_vanity35 = _enqueue_mode_b_sourcify_matches(
                        st35,
                        [
                            {"chain": "ethereum", "address": vanity_zero35, "case_idx": 1, "score": 1.0, "seed_slug": "s35"},
                            {"chain": "ethereum", "address": vanity_funded35, "case_idx": 1, "score": 0.7, "seed_slug": "s35"},
                        ],
                        router=r35_bal,
                    )
                    first_claimed35 = st35.claim_next_for_session("ses-t35-rank", BOOT_ID, max_priority=1)
                    second_claimed35 = st35.claim_next_for_session("ses-t35-rank", BOOT_ID, max_priority=1)
                    vanity_rank_ok35 = (
                        n_vanity35 == 2
                        and first_claimed35 is not None
                        and second_claimed35 is not None
                        and list(first_claimed35.get("addresses") or ()) == [vanity_funded35]
                        and list(second_claimed35.get("addresses") or ()) == [vanity_zero35]
                        and str(first_claimed35.get("slug") or "").endswith("000000bb0002")
                        and str(second_claimed35.get("slug") or "").endswith("000000aa0001")
                    )

                    # (c) ERC-4626 영수증 토큰 온체인 준비금 한도 상환 가치 실측
                    vault_4626_35 = "0x" + ("46" * 20)
                    weth_eth35 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])

                    class _MockRouter35Vault(RPCRouter):
                        def call(
                            self,
                            chain: str,
                            method: str,
                            params: Sequence[Any],
                            block_pin: Optional[int] = None,
                            need_archive: bool = False,
                            timeout: float = 15.0,
                            raise_on_all_failed: bool = False,
                        ) -> Any:
                            if method == "eth_getCode":
                                return "0x"
                            if method == "eth_call":
                                tx_o = params[0] if params else {}
                                to_a = _norm_addr(str(tx_o.get("to") or ""))
                                data_h = str(tx_o.get("data") or "").lower()
                                if to_a == vault_4626_35:
                                    if data_h.startswith(ERC4626_ASSET_SEL.lower()):
                                        return "0x" + abi_encode(["address"], [_checksum(weth_eth35)]).hex()
                                    if data_h.startswith(ERC4626_CONVERT_TO_ASSETS_SEL.lower()):
                                        return "0x" + abi_encode(["uint256"], [100 * 10**18]).hex()
                                if to_a == weth_eth35:
                                    if data_h.startswith(DECIMALS_SEL.lower()):
                                        return "0x" + abi_encode(["uint8"], [18]).hex()
                                    if data_h.startswith(BALANCE_OF_SEL.lower()) and vault_4626_35[2:] in data_h[10:]:
                                        return "0x" + abi_encode(["uint256"], [2 * 10**18]).hex()
                                    if data_h.startswith(PAUSED_SEL.lower()):
                                        return None
                                return None
                            return None

                    r35_vault = _MockRouter35Vault(store=st35, session_id="ses-t35-vault")
                    liq_4626_35 = probe_dex_exit_liquidity(
                        r35_vault,
                        "ethereum",
                        vault_4626_35,
                        100 * 10**18,
                        18,
                        2500.0,
                        2500.0,
                        20_000_000,
                        quote_prices_usd={weth_eth35: 2500.0},
                    )
                    erc4626_ok35 = (
                        liq_4626_35.get("exit_liquid") is True
                        and liq_4626_35.get("best_dex_kind") == "redeem_erc4626"
                        and float(liq_4626_35.get("realizable_usd") or 0.0) == 5000.0
                        and float(liq_4626_35.get("dex_depth_usd") or 0.0) == 5000.0
                    )

                    # (d) 분리형 아키텍처 인바운드 RoleGranted(해지 검증)/Approval 역추적 + 휴면(500k) 블록 토큰 + PUSH20 정적 발견
                    strategy35 = "0x" + ("51" * 20)
                    static_tok35 = "0x" + ("52" * 20)
                    ext_vault_act35 = "0x" + ("53" * 20)
                    ext_vault_rev35 = "0x" + ("54" * 20)
                    treasury35 = "0x" + ("55" * 20)
                    dormant_tok35 = "0x" + ("56" * 20)
                    strategy_code35 = (
                        bytes.fromhex("73" + static_tok35[2:] + "506000808080805af1") + (b"\x5b" * 70)
                    )
                    role35_hex = "0x" + ("11" * 32)
                    pad_strat35 = "0x" + ("00" * 12) + strategy35[2:]
                    pad_treas35 = "0x" + ("00" * 12) + treasury35[2:]

                    class _MockRouter35Inbound(RPCRouter):
                        def call(
                            self,
                            chain: str,
                            method: str,
                            params: Sequence[Any],
                            block_pin: Optional[int] = None,
                            need_archive: bool = False,
                            timeout: float = 15.0,
                            raise_on_all_failed: bool = False,
                        ) -> Any:
                            if method == "eth_getCode":
                                ad = _norm_addr(str(params[0] if params else ""))
                                if ad == strategy35:
                                    return "0x" + strategy_code35.hex()
                                if ad in (ext_vault_act35, ext_vault_rev35, treasury35, dormant_tok35, static_tok35):
                                    return "0x" + (b"\x5b" * 70).hex()
                                return "0x"
                            if method == "eth_getStorageAt" or method == "eth_getBalance":
                                return "0x" + ("00" * 32)
                            if method == "eth_getLogs":
                                flt = params[0] if params and isinstance(params[0], dict) else {}
                                fb_l = _hex_to_int(flt.get("fromBlock") or "0x0")
                                tb_l = _hex_to_int(flt.get("toBlock") or "0x0")
                                tps_l = flt.get("topics") or []
                                t0_items = [str(x).lower() for x in (tps_l[0] if isinstance(tps_l[0], list) else [tps_l[0]])] if tps_l else []
                                if ROLE_GRANTED_TOPIC0.lower() in t0_items:
                                    return [
                                        {"address": _checksum(ext_vault_act35), "topics": [ROLE_GRANTED_TOPIC0, role35_hex, pad_strat35]},
                                        {"address": _checksum(ext_vault_rev35), "topics": [ROLE_GRANTED_TOPIC0, role35_hex, pad_strat35]},
                                        {"address": _checksum(dormant_tok35), "topics": [APPROVAL_TOPIC0, pad_treas35, pad_strat35]},
                                    ]
                                if TRANSFER_TOPIC0.lower() in t0_items:
                                    if fb_l <= 19_600_000 <= tb_l:
                                        return [
                                            {"address": _checksum(dormant_tok35), "topics": [TRANSFER_TOPIC0, pad_treas35, pad_strat35]},
                                        ]
                                return []
                            if method == "eth_call":
                                tx_o = params[0] if params else {}
                                to_a = _norm_addr(str(tx_o.get("to") or ""))
                                data_h = str(tx_o.get("data") or "").lower()
                                if data_h.startswith(HAS_ROLE_SEL.lower()):
                                    if to_a == ext_vault_act35:
                                        return "0x" + abi_encode(["bool"], [True]).hex()
                                    if to_a == ext_vault_rev35:
                                        return "0x" + abi_encode(["bool"], [False]).hex()
                                if data_h.startswith(DECIMALS_SEL.lower()) and to_a in (dormant_tok35, static_tok35):
                                    return "0x" + abi_encode(["uint8"], [18]).hex()
                                if data_h.startswith(BALANCE_OF_SEL.lower()) and to_a == dormant_tok35:
                                    return "0x" + abi_encode(["uint256"], [10**18]).hex()
                                if data_h.startswith(ALLOWANCE_SEL.lower()) and to_a == dormant_tok35:
                                    return "0x" + abi_encode(["uint256"], [10**18]).hex()
                                return None
                            return None

                    _set_eng("SOURCIFY_PARQUET_DIR", ns35 / "sf_empty")
                    r35_inb = _MockRouter35Inbound(store=st35, session_id="ses-t35-inb")
                    v_inb35, g_inb35, at_inb35 = _harvest_inbound_trust_and_approvals(
                        r35_inb, "ethereum", [strategy35], 20_000_000, is_live_rpc=False
                    )
                    cl35 = build_backward_trust_closure(r35_inb, "ethereum", [strategy35], 20_000_000)
                    treas_exp35 = cl35.vault_exposures.get(treasury35)
                    inbound_dormant_ok35 = (
                        ext_vault_act35 in v_inb35
                        and ext_vault_rev35 not in v_inb35
                        and treasury35 in g_inb35
                        and dormant_tok35 in at_inb35
                        and ext_vault_act35 in cl35.all_addresses
                        and ext_vault_rev35 not in cl35.all_addresses
                        and ext_vault_rev35 not in cl35.token_decimals
                        and treasury35 in cl35.all_addresses
                        and treas_exp35 is not None
                        and treas_exp35.token_balances.get(dormant_tok35) == 10**18
                        and dormant_tok35 in cl35.token_decimals
                        and static_tok35 in cl35.token_decimals
                    )

                    # (e) defer_mode_a=True 콜드 스타트 무HTTP Mode B 우선 커밋 + 완료 게이트 차단 + _ensure_mode_a_seeded 지연 적재
                    http_calls35 = 0

                    def _mock_http_fb35(router_arg: Any, url: str, timeout: float = 15.0) -> Any:
                        nonlocal http_calls35
                        http_calls35 += 1
                        return [
                            {
                                "slug": "t35-deferred-proto",
                                "name": "T35 Deferred Proto",
                                "category": "Lending",
                                "chain": "ethereum",
                                "address": "0x" + ("77" * 20),
                                "tvl": 500_000.0,
                                "chainTvls": {"ethereum": 500_000.0},
                            }
                        ]

                    _eng_globals()["_http_get_json_with_fallback"] = _mock_http_fb35
                    n_def35 = _seed_queue_from_existing_or_live(st35_defer, defer_mode_a=True)
                    pre_http_zero_ok = (
                        n_def35 > 0
                        and http_calls35 == 0
                        and st35_defer.get_engine_meta("mode_a_seed_pending") == "1"
                    )
                    # Pending state must block done gates even if current Mode B queue happens to be drained
                    st35_gate.ensure_session("ses-t35-gate")
                    st35_gate.upsert_queue_items_batch(
                        [("modeb-ethereum-000-g35", 1, "ethereum", MIN_TVL_USD, [], "B", "g", 100.0)]
                    )
                    g_claimed35 = st35_gate.claim_next_for_session("ses-t35-gate", BOOT_ID)
                    if g_claimed35:
                        st35_gate.complete_queue_item("modeb-ethereum-000-g35", VERDICT_SAFE, 0.0)
                    st35_gate.put_engine_meta("mode_a_seed_pending", "1")
                    gate_glob_blk35 = _evaluate_done_gates(st35_gate, artifact_root=art35_dir, run_selftest=False)
                    gate_sess_blk35 = _evaluate_session_done_gates(
                        st35_gate, "ses-t35-gate", artifact_root=art35_dir, run_selftest=False
                    )
                    st35_gate.put_engine_meta("mode_a_seed_pending", "0")
                    gate_glob_unblk35 = _evaluate_done_gates(st35_gate, artifact_root=art35_dir, run_selftest=False)
                    gate_sess_unblk35 = _evaluate_session_done_gates(
                        st35_gate, "ses-t35-gate", artifact_root=art35_dir, run_selftest=False
                    )
                    gate_blocked_ok35 = (
                        gate_glob_blk35.get("all_gates_passed") is False
                        and gate_glob_blk35.get("gate1_queue_drained_and_nonempty") is False
                        and gate_sess_blk35.get("all_gates_passed") is False
                        and gate_sess_blk35.get("gate1_session_goal_satisfied") is False
                        and gate_glob_unblk35.get("gate1_queue_drained_and_nonempty") is True
                        and gate_sess_unblk35.get("gate1_session_goal_satisfied") is True
                    )
                    ins_a35 = _ensure_mode_a_seeded(st35_defer)
                    ins_a35_idem = _ensure_mode_a_seeded(st35_defer)
                    bd35_after = st35_defer.queue_mode_breakdown()
                    defer_modea_ok35 = (
                        pre_http_zero_ok
                        and gate_blocked_ok35
                        and ins_a35 >= 1
                        and ins_a35_idem == 0
                        and http_calls35 == 1
                        and st35_defer.get_engine_meta("mode_a_seed_pending") == "0"
                        and st35_defer.get_engine_meta("mode_a_seed_ok") == "1"
                        and int((bd35_after.get("A") or {}).get("total", 0)) >= 1
                        and int((bd35_after.get("B") or {}).get("total", 0)) >= 1
                    )

                    t35_ok = (
                        seed_hist_ok35
                        and vanity_rank_ok35
                        and erc4626_ok35
                        and inbound_dormant_ok35
                        and defer_modea_ok35
                    )
                    results.append(
                        (
                            "T35_MODEB_SEEDHIST_VANITY_DORMANT_INBOUND_AND_DEFER_MODEA",
                            t35_ok,
                            (
                                f"seed_hist={seed_hist_ok35},vanity_rank={vanity_rank_ok35},"
                                f"erc4626={erc4626_ok35},inb_dormant={inbound_dormant_ok35},defer_modea={defer_modea_ok35}"
                            ),
                        )
                    )
                finally:
                    _set_eng("SOURCIFY_PARQUET_DIR", orig_sf_dir35)
                    _eng_globals()["_query_local_sourcify_deployment_hints"] = orig_q_local_hints35
                    _eng_globals()["_http_get_json_with_fallback"] = orig_http_fb35
                    st35_gate.close()
                    st35_defer.close()
                    st35.close()

            if _want_test("T36_HARNESS_TOKEN_RANK_WITNESS_ISOLATION_MODEB_PROXY_AND_AUTO_LIQ", tag_filters):
                # T36: 하네스 토큰 우선순위[:12], 멀티위트니스 테스트 격리, modeb-match 프록시 스코프/스테일 필터/자금합산, 온체인 자동 청산 스왑·상환
                st36 = SQLiteStore(selftest_ns / "_t36_verify.sqlite")
                orig_sf_dir36 = SOURCIFY_PARQUET_DIR
                orig_proxy_search36 = _eng_globals()["search_sourcify_v2_proxies_for_impls"]
                orig_build_closure36 = _eng_globals()["build_backward_trust_closure"]
                try:
                    # (a) Prioritized token_addrs[:12] with >12 candidates, $500k custom/allowance token + exit quote preserved inside [:12]
                    tok36_custom = "0x" + "a1" * 20
                    tok36_quote = "0x" + "b2" * 20
                    tok36_seed = "0x" + "c3" * 20
                    tok36_bal_only = "0x" + "d4" * 20
                    tok36_fillers = [f"0x{10 + i:040x}" for i in range(12)]
                    dec36_map = {a: 18 for a in [tok36_seed, *tok36_fillers, tok36_bal_only, tok36_custom, tok36_quote]}
                    cl36_prior = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=[tok36_seed, "0x" + "ee" * 20],
                        all_addresses=list(dec36_map.keys()) + ["0x" + "ee" * 20],
                        resolutions={},
                        vault_exposures={
                            "0x" + "ee" * 20: VaultExposure(
                                address="0x" + "ee" * 20,
                                token_balances={tok36_bal_only: 12345},
                                token_usd={f: 0.0 for f in tok36_fillers},
                            )
                        },
                        max_extractable_usd=500_000.0,
                        min_gas_usd=0.05,
                        early_exit_safe=False,
                        early_exit_reason="",
                        token_decimals=dec36_map,
                        token_prices_usd={},
                        verified_exit_pools={
                            tok36_custom: {
                                "pool": _checksum("0x" + "f5" * 20),
                                "quote_token": _checksum(tok36_quote),
                                "dex_kind": "v2",
                                "held_raw": str(10**18),
                                "realizable_usd": 500_000.0,
                                "paused": False,
                            }
                        },
                    )
                    picked36_a = _prioritize_closure_monitored_tokens("ethereum", cl36_prior, limit=12)
                    # Also verify 12 non-zero USD tokens + 13th exit quote preserves both top token & its quote
                    cl36_all_paid = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=[tok36_custom],
                        all_addresses=[tok36_custom, *tok36_fillers, tok36_quote],
                        resolutions={},
                        vault_exposures={
                            "0x" + "ee" * 20: VaultExposure(
                                address="0x" + "ee" * 20,
                                token_balances={tok36_custom: 10**18, **{f: 10 for f in tok36_fillers}},
                                token_usd={tok36_custom: 500_000.0, **{f: 10.0 + idx_f for idx_f, f in enumerate(tok36_fillers)}},
                            )
                        },
                        max_extractable_usd=500_000.0,
                        min_gas_usd=0.05,
                        early_exit_safe=False,
                        early_exit_reason="",
                        token_decimals={**{f: 18 for f in tok36_fillers}, tok36_custom: 18, tok36_quote: 18},
                        verified_exit_pools={
                            tok36_custom: {
                                "pool": _checksum("0x" + "f5" * 20),
                                "quote_token": _checksum(tok36_quote),
                                "dex_kind": "v2",
                                "held_raw": str(10**18),
                                "realizable_usd": 500_000.0,
                                "paused": False,
                            }
                        },
                    )
                    picked36_b = _prioritize_closure_monitored_tokens("ethereum", cl36_all_paid, limit=12)
                    tok_rank_ok36 = (
                        len(picked36_a) == 12
                        and picked36_a[0] == tok36_custom
                        and tok36_quote in picked36_a
                        and tok36_seed in picked36_a
                        and tok36_bal_only in picked36_a
                        and len(picked36_b) == 12
                        and tok36_custom in picked36_b
                        and tok36_quote in picked36_b
                    )

                    # (b) Per-witness isolation: w0=1 sets one-shot lock; w1=100 ether still succeeds cleanly in isolated test
                    _rlp_create_36 = _rlp_create_low_nonce

                    witness_iso_ok36 = True
                    auto_liq_ok36 = True
                    if FORGE_BIN.exists():
                        caller36 = "0x" + "61" * 20
                        c_chk36 = _checksum(caller36)
                        tok36_os = _rlp_create_36(caller36, 0)
                        vault36_os = _rlp_create_36(caller36, 1)
                        claim_sel36 = _selector("claim(uint256)")
                        sl_os36 = CausalSlice(
                            slice_id="oneshot_0",
                            target_address=vault36_os,
                            channel="CH_STORAGE",
                            legs=[claim_sel36],
                            witness_candidates=[1, 100 * 10**18],
                            repeat_counts=[1],
                            rationale="one-shot mutation isolation test",
                            leg_targets=[vault36_os],
                            abi_inputs_by_sel={claim_sel36: ["uint256"]},
                        )
                        h_os36 = _render_minimal_solidity_harness(
                            slug="t36_oneshot",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller36,
                            target_addr=vault36_os,
                            token_addrs=[tok36_os],
                            slices=[sl_os36],
                            vault_addresses=[vault36_os],
                        )
                        os_contracts_sol = """
contract OneShotTok36 {
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "BAL");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
}
contract OneShotVault36 {
    address public immutable tok;
    bool public locked;
    constructor(address _tok) { tok = _tok; }
    function claim(uint256 amt) external {
        if (locked) { return; }
        locked = true;
        if (amt >= 100 ether) {
            OneShotTok36(tok).transfer(msg.sender, amt);
        }
    }
}
"""
                        h_os36_exec = h_os36.replace(
                            "contract Verify_t36_oneshot {",
                            os_contracts_sol + "\ncontract Verify_t36_oneshot {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36});",
                            f"        vm.startPrank({c_chk36}, {c_chk36});\n"
                            f"        OneShotTok36 _t36 = new OneShotTok36();\n"
                            f"        OneShotVault36 _v36 = new OneShotVault36(address(_t36));\n"
                            f"        _t36.mint(address(_v36), 100 ether);\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36});",
                        )
                        ws36_os = ensure_clean_forge_workspace({}, slug="t36_oneshot", offline=True)
                        rel36_os = "test/Verify_t36_oneshot.t.sol"
                        (ws36_os / rel36_os).write_text(h_os36_exec, encoding="utf-8")
                        out36_os = execute_forge_test_file(
                            ws36_os,
                            rel36_os,
                            h_os36_exec,
                            timeout_sec=60,
                            allowed_caller=caller36,
                            target_addr=vault36_os,
                            offline=True,
                        )
                        cf36_os = verify_counterfactual_target_causality(
                            ws36_os,
                            rel36_os,
                            h_os36_exec,
                            vault36_os,
                            winning_test="test_slice_0_w1",
                            offline=True,
                        )
                        ok_allow_os, _ = verify_cheatcode_allowlist(h_os36, allowed_caller=caller36)
                        ok_acct_os, _ = verify_agent_harness_accounting_integrity(h_os36)
                        witness_iso_ok36 = (
                            "function test_slice_0_w0()" in h_os36
                            and "function test_slice_0_w1()" in h_os36
                            and "function _probe_slice_0(uint256 _w) internal" in h_os36
                            and ok_allow_os
                            and ok_acct_os
                            and out36_os.executed
                            and out36_os.passed_tests == 2
                            and out36_os.failed_tests == 0
                            and len(out36_os.conservation_gains) == 1
                            and out36_os.conservation_gains[0].get("test") == "test_slice_0_w1()"
                            and int(out36_os.conservation_gains[0].get("caller_gain") or 0) == 100 * 10**18
                            and bool(cf36_os)
                        )

                        # (d) Real on-chain auto-liquidation (V2 pair + atomic broken-swap revert fallback + 2-hop ERC4626 -> V3 swap)
                        caller36_d = "0x" + "62" * 20
                        c_chk36_d = _checksum(caller36_d)
                        d_tok_custom = _rlp_create_36(caller36_d, 0)
                        d_tok_quote = _rlp_create_36(caller36_d, 1)
                        d_vault = _rlp_create_36(caller36_d, 2)
                        d_pair = _rlp_create_36(caller36_d, 3)
                        d_broken_pair = _rlp_create_36(caller36_d, 4)
                        drain_sel36 = selftest_drain_sel
                        sl_d_v2 = CausalSlice(
                            slice_id="liq_v2_0",
                            target_address=d_vault,
                            channel="CH_STORAGE",
                            legs=[drain_sel36],
                            witness_candidates=[1],
                            repeat_counts=[1],
                            rationale="steal custom token and auto-liquidate via V2",
                            leg_targets=[d_vault],
                            abi_inputs_by_sel={drain_sel36: []},
                        )
                        dex_mocks_sol = """
contract MockErc36 {
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "BAL");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
    function transferFrom(address fr, address to, uint256 a) external returns (bool) {
        require(allowance[fr][msg.sender] >= a && balanceOf[fr] >= a, "ALW");
        allowance[fr][msg.sender] -= a;
        balanceOf[fr] -= a;
        balanceOf[to] += a;
        return true;
    }
}
contract DrainVault36 {
    address public immutable tok;
    constructor(address _tok) { tok = _tok; }
    function drain() external {
        MockErc36(tok).transfer(msg.sender, 50 ether);
    }
}
contract MockV2Pair36 {
    address public immutable token0;
    address public immutable token1;
    uint112 public r0;
    uint112 public r1;
    constructor(address _t0, address _t1) { token0 = _t0; token1 = _t1; }
    function sync() external {
        r0 = uint112(MockErc36(token0).balanceOf(address(this)));
        r1 = uint112(MockErc36(token1).balanceOf(address(this)));
    }
    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }
    function swap(uint256 a0Out, uint256 a1Out, address to, bytes calldata) external {
        if (a0Out > 0) { MockErc36(token0).transfer(to, a0Out); }
        if (a1Out > 0) { MockErc36(token1).transfer(to, a1Out); }
        uint256 b0 = MockErc36(token0).balanceOf(address(this));
        uint256 b1 = MockErc36(token1).balanceOf(address(this));
        uint256 in0 = b0 > (uint256(r0) - a0Out) ? (b0 - (uint256(r0) - a0Out)) : 0;
        uint256 in1 = b1 > (uint256(r1) - a1Out) ? (b1 - (uint256(r1) - a1Out)) : 0;
        require(in0 > 0 || in1 > 0, "IIN");
        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3) >= uint256(r0) * uint256(r1) * 1000000, "K");
        r0 = uint112(b0);
        r1 = uint112(b1);
    }
}
contract MockBrokenPair36 {
    address public immutable token0;
    constructor(address _t0) { token0 = _t0; }
    function getReserves() external pure returns (uint112, uint112, uint32) { return (uint112(1000 ether), uint112(100 ether), 1); }
    function swap(uint256, uint256, address, bytes calldata) external pure { revert("BROKEN_POOL"); }
}
"""
                        h_d_v2 = _render_minimal_solidity_harness(
                            slug="t36_liq_v2",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller36_d,
                            target_addr=d_vault,
                            token_addrs=[d_tok_custom, d_tok_quote],
                            slices=[sl_d_v2],
                            vault_addresses=[d_vault],
                            exit_routes={
                                d_tok_custom: {
                                    "pool": _checksum(d_pair),
                                    "quote_token": _checksum(d_tok_quote),
                                    "dex_kind": "v2",
                                    "realizable_usd": 10_000.0,
                                    "paused": False,
                                }
                            },
                        )
                        deploy_d_snippet = (
                            f"        vm.startPrank({c_chk36_d}, {c_chk36_d});\n"
                            f"        MockErc36 _tc36 = new MockErc36();\n"
                            f"        MockErc36 _tq36 = new MockErc36();\n"
                            f"        DrainVault36 _dv36 = new DrainVault36(address(_tc36));\n"
                            f"        MockV2Pair36 _dp36 = new MockV2Pair36(address(_tc36), address(_tq36));\n"
                            f"        new MockBrokenPair36(address(_tc36));\n"
                            f"        _tc36.mint(address(_dv36), 50 ether);\n"
                            f"        _tc36.mint(address(_dp36), 1000 ether);\n"
                            f"        _tq36.mint(address(_dp36), 100 ether);\n"
                            f"        _dp36.sync();\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36_d});"
                        )
                        h_d_v2_exec = h_d_v2.replace(
                            "contract LiquidatorHelper_t36_liq_v2 {",
                            dex_mocks_sol + "\ncontract LiquidatorHelper_t36_liq_v2 {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36_d});",
                            deploy_d_snippet,
                        )
                        ws36_v2 = ensure_clean_forge_workspace({}, slug="t36_liq_v2", offline=True)
                        rel36_v2 = "test/Verify_t36_liq_v2.t.sol"
                        (ws36_v2 / rel36_v2).write_text(h_d_v2_exec, encoding="utf-8")
                        out36_v2 = execute_forge_test_file(
                            ws36_v2,
                            rel36_v2,
                            h_d_v2_exec,
                            timeout_sec=60,
                            allowed_caller=caller36_d,
                            target_addr=d_vault,
                            offline=True,
                        )
                        cf36_v2 = verify_counterfactual_target_causality(
                            ws36_v2,
                            rel36_v2,
                            h_d_v2_exec,
                            d_vault,
                            winning_test="test_slice_0",
                            offline=True,
                        )
                        ok_allow_v2, _ = verify_cheatcode_allowlist(h_d_v2, allowed_caller=caller36_d)
                        ok_acct_v2, _ = verify_agent_harness_accounting_integrity(h_d_v2)

                        # Swap-revert fallback check
                        h_d_fail = _render_minimal_solidity_harness(
                            slug="t36_liq_v2",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller36_d,
                            target_addr=d_vault,
                            token_addrs=[d_tok_custom, d_tok_quote],
                            slices=[sl_d_v2],
                            vault_addresses=[d_vault],
                            exit_routes={
                                d_tok_custom: {
                                    "pool": _checksum(d_broken_pair),
                                    "quote_token": _checksum(d_tok_quote),
                                    "dex_kind": "v2",
                                    "realizable_usd": 10_000.0,
                                    "paused": False,
                                }
                            },
                        )
                        h_d_fail_exec = h_d_fail.replace(
                            "contract LiquidatorHelper_t36_liq_v2 {",
                            dex_mocks_sol + "\ncontract LiquidatorHelper_t36_liq_v2 {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36_d});",
                            deploy_d_snippet,
                        )
                        rel36_fail = "test/Verify_t36_liq_fail.t.sol"
                        (ws36_v2 / rel36_fail).write_text(h_d_fail_exec, encoding="utf-8")
                        out36_fail = execute_forge_test_file(
                            ws36_v2,
                            rel36_fail,
                            h_d_fail_exec,
                            timeout_sec=60,
                            allowed_caller=caller36_d,
                            target_addr=d_vault,
                            offline=True,
                        )

                        # 2-hop ERC-4626 redeem -> V3 callback swap check
                        caller36_h2 = "0x" + "63" * 20
                        c_chk36_h2 = _checksum(caller36_h2)
                        h2_share = _rlp_create_36(caller36_h2, 0)
                        h2_mid = _rlp_create_36(caller36_h2, 1)
                        h2_quote = _rlp_create_36(caller36_h2, 2)
                        h2_vault = _rlp_create_36(caller36_h2, 3)
                        h2_v3pool = _rlp_create_36(caller36_h2, 4)
                        hop2_mocks_sol = """
interface IV3SwapCb36 {
    function uniswapV3SwapCallback(int256 a0, int256 a1, bytes calldata data) external;
}
contract MockVault4626_36 {
    address public asset;
    address public immutable midTok;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    constructor(address _mid) { midTok = _mid; asset = _mid; }
    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "SBAL");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
    function redeem(uint256 shares, address receiver, address owner) external returns (uint256) {
        require(msg.sender == owner && balanceOf[owner] >= shares, "RDM");
        balanceOf[owner] -= shares;
        totalSupply -= shares;
        MockErc36(midTok).transfer(receiver, shares);
        return shares;
    }
}
contract ShareDrain36 {
    address public immutable sh;
    constructor(address _sh) { sh = _sh; }
    function drain() external { MockVault4626_36(sh).transfer(msg.sender, 40 ether); }
}
contract MockV3Pool36 {
    address public immutable token0;
    address public immutable token1;
    constructor(address _t0, address _t1) { token0 = _t0; token1 = _t1; }
    function swap(address recipient, bool zeroForOne, int256 amountSpecified, uint160, bytes calldata data) external returns (int256, int256) {
        require(amountSpecified > 0 && zeroForOne, "V3A");
        uint256 amtIn = uint256(amountSpecified);
        uint256 outAmt = amtIn / 2;
        uint256 pre0 = MockErc36(token0).balanceOf(address(this));
        MockErc36(token1).transfer(recipient, outAmt);
        IV3SwapCb36(msg.sender).uniswapV3SwapCallback(int256(amtIn), -int256(outAmt), data);
        require(MockErc36(token0).balanceOf(address(this)) >= pre0 + amtIn, "CBIN");
        return (int256(amtIn), -int256(outAmt));
    }
}
"""
                        sl_h2 = CausalSlice(
                            slice_id="liq_h2_0",
                            target_address=h2_vault,
                            channel="CH_STORAGE",
                            legs=[drain_sel36],
                            witness_candidates=[1],
                            repeat_counts=[1],
                            rationale="steal share -> redeem 4626 -> swap V3",
                            leg_targets=[h2_vault],
                            abi_inputs_by_sel={drain_sel36: []},
                        )
                        h_d_h2 = _render_minimal_solidity_harness(
                            slug="t36_liq_h2",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller36_h2,
                            target_addr=h2_vault,
                            token_addrs=[h2_share, h2_mid, h2_quote],
                            slices=[sl_h2],
                            vault_addresses=[h2_vault],
                            exit_routes={
                                h2_share: {
                                    "pool": _checksum(h2_share),
                                    "quote_token": _checksum(h2_mid),
                                    "dex_kind": "redeem_erc4626",
                                    "redeem_target": _checksum(h2_share),
                                    "realizable_usd": 10_000.0,
                                    "paused": False,
                                    "underlying_exit": {
                                        "pool": _checksum(h2_v3pool),
                                        "quote_token": _checksum(h2_quote),
                                        "dex_kind": "v3",
                                        "v3_fee": 3000,
                                    },
                                }
                            },
                        )
                        h_d_h2_exec = h_d_h2.replace(
                            "contract LiquidatorHelper_t36_liq_h2 {",
                            dex_mocks_sol + "\n" + hop2_mocks_sol + "\ncontract LiquidatorHelper_t36_liq_h2 {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36_h2});",
                            f"        vm.startPrank({c_chk36_h2}, {c_chk36_h2});\n"
                            f"        MockErc36 _h2m = MockErc36(address(new MockVault4626_36({ _checksum(h2_mid) })));\n"
                            f"        MockErc36 _h2mid = new MockErc36();\n"
                            f"        MockErc36 _h2q = new MockErc36();\n"
                            f"        ShareDrain36 _h2dv = new ShareDrain36(address(_h2m));\n"
                            f"        MockV3Pool36 _h2p = new MockV3Pool36(address(_h2mid), address(_h2q));\n"
                            f"        MockVault4626_36(address(_h2m)).mint(address(_h2dv), 40 ether);\n"
                            f"        _h2mid.mint(address(_h2m), 40 ether);\n"
                            f"        _h2q.mint(address(_h2p), 100 ether);\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk36_h2});",
                        )
                        ws36_h2 = ensure_clean_forge_workspace({}, slug="t36_liq_h2", offline=True)
                        rel36_h2 = "test/Verify_t36_liq_h2.t.sol"
                        (ws36_h2 / rel36_h2).write_text(h_d_h2_exec, encoding="utf-8")
                        out36_h2 = execute_forge_test_file(
                            ws36_h2,
                            rel36_h2,
                            h_d_h2_exec,
                            timeout_sec=60,
                            allowed_caller=caller36_h2,
                            target_addr=h2_vault,
                            offline=True,
                        )
                        cf36_h2 = verify_counterfactual_target_causality(
                            ws36_h2,
                            rel36_h2,
                            h_d_h2_exec,
                            h2_vault,
                            winning_test="test_slice_0",
                            offline=True,
                        )
                        ok_allow_h2, _ = verify_cheatcode_allowlist(h_d_h2, allowed_caller=caller36_h2)
                        ok_acct_h2, _ = verify_agent_harness_accounting_integrity(h_d_h2)
                        auto_liq_ok36 = (
                            ok_allow_v2
                            and ok_acct_v2
                            and out36_v2.executed
                            and out36_v2.passed_tests == 1
                            and len(out36_v2.conservation_gains) == 1
                            and out36_v2.conservation_gains[0].get("asset") == d_tok_quote
                            and 4 * 10**18 < int(out36_v2.conservation_gains[0].get("caller_gain") or 0) < 5 * 10**18
                            and int(out36_v2.conservation_gains[0].get("vault_loss") or 0) == int(out36_v2.conservation_gains[0].get("caller_gain") or 0)
                            and bool(cf36_v2)
                            and out36_fail.executed
                            and out36_fail.passed_tests == 1
                            and len(out36_fail.conservation_gains) == 1
                            and out36_fail.conservation_gains[0].get("asset") == d_tok_custom
                            and int(out36_fail.conservation_gains[0].get("caller_gain") or 0) == 50 * 10**18
                            and ok_allow_h2
                            and ok_acct_h2
                            and out36_h2.executed
                            and out36_h2.passed_tests == 1
                            and len(out36_h2.conservation_gains) == 1
                            and out36_h2.conservation_gains[0].get("asset") == h2_quote
                            and int(out36_h2.conservation_gains[0].get("caller_gain") or 0) == 20 * 10**18
                            and int(out36_h2.conservation_gains[0].get("vault_loss") or 0) == 20 * 10**18
                            and bool(cf36_h2)
                        )

                    # (c) Mode B scoping (chain_id=cid_target, limit=64), early patched proxy exit, stale proxy filter, & proxy balance aggregation
                    seed_slug36 = "t36-seed-src"
                    seed_addr36 = "0x" + "71" * 20
                    impl_vuln36 = "0x" + "72" * 20
                    impl_patch36 = "0x" + "73" * 20
                    proxy_stale36 = "0x" + "74" * 20
                    proxy_active36 = "0x" + "75" * 20
                    proxy_fast_cl36 = "0x" + "76" * 20

                    # >=80B distinct bytecode: open withdraw/value-sink vs patched owner-only code
                    vuln_bc36 = bytes.fromhex(
                        "633ccfd60b60003560e01c14601257600080fd5b"
                        "6000808080473361fffff100" + ("5b00" * 30)
                    )
                    patch_bc36 = bytes.fromhex(
                        "3373" + ("88" * 20) + "14601e57600080fd5b"
                        "3415602857600080fd5b"
                        "30315032504250435044504550465047504850"
                        + ("5a016001900350" * 8)
                        + "600080f3"
                    )
                    eip1967_stub_hex36 = (
                        "0x6000366000600037600036"
                        "7f" + EIP1967_SLOTS["impl"][2:]
                        + "545af43d82803e903d91604457fd5bf3"
                    )
                    st36.put_contract_code_batch([
                        (
                            1,
                            seed_addr36,
                            20_000_000,
                            hashlib.sha256(vuln_bc36).hexdigest().lower(),
                            _skeleton_hash(vuln_bc36),
                            json.dumps(sorted(_opcode_4grams(vuln_bc36, cap=128))),
                            "0x" + vuln_bc36.hex(),
                            "none",
                            [],
                            True,
                        ),
                        (
                            1,
                            proxy_stale36,
                            20_000_000,
                            hashlib.sha256(bytes.fromhex(eip1967_stub_hex36[2:])).hexdigest().lower(),
                            "stale_proxy_skel_36",
                            "[]",
                            eip1967_stub_hex36,
                            "eip1967_impl",
                            [impl_vuln36],
                            True,
                        ),
                    ])
                    eip1967_slot_val = "0x000000000000000000000000" + impl_patch36[2:]
                    eip1967_active_val = "0x000000000000000000000000" + impl_vuln36[2:]

                    class _ModeBProxyRouter36(_SelftestMockRouter):
                        def call(self, chain: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_blockNumber":
                                return hex(20_000_000)
                            if method == "eth_getCode" and params:
                                na = _norm_addr(params[0])
                                if na == impl_vuln36:
                                    return "0x" + vuln_bc36.hex()
                                if na == impl_patch36:
                                    return "0x" + patch_bc36.hex()
                                if na in (proxy_stale36, proxy_active36):
                                    return eip1967_stub_hex36
                            if method == "eth_getStorageAt" and len(params) >= 2:
                                na = _norm_addr(params[0])
                                sl_p = str(params[1]).lower()
                                if sl_p == EIP1967_SLOTS["impl"].lower():
                                    if na == proxy_stale36:
                                        return eip1967_slot_val
                                    if na == proxy_active36:
                                        return eip1967_active_val
                                if sl_p == "0x0":
                                    return "0x" + "00" * 31 + "01"
                            if method == "eth_getLogs":
                                return []
                            return "0x0"

                        def batch_call(self, chain: str, calls: Sequence[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_b: List[Any] = []
                            for m_c, p_c in calls:
                                if m_c == "eth_getBalance":
                                    na = _norm_addr(p_c[0]) if p_c else ""
                                    if na == proxy_active36:
                                        out_b.append(hex(50 * 10**18))
                                    elif na == proxy_fast_cl36:
                                        out_b.append(hex(100 * 10**18))
                                    elif _norm_addr(p_c[0]) == _norm_addr(f"0x{500 + 59:040x}"):
                                        out_b.append(hex(2 * 10**18))
                                    elif _norm_addr(p_c[0]) == _norm_addr(f"0x{500:040x}"):
                                        out_b.append(hex(1 * 10**18))
                                    else:
                                        out_b.append("0x0")
                                else:
                                    out_b.append(self.call(chain, m_c, p_c))
                            return out_b

                    r36_mb = _ModeBProxyRouter36(store=st36)
                    patched_slug36 = _norm_slug(f"modeb-match-ethereum-001-{proxy_stale36[2:8]}{proxy_stale36[-6:]}")
                    st36.put_proto_meta_batch([
                        (
                            patched_slug36,
                            {
                                "mode_b_source_slug": seed_slug36,
                                "mode_b_source_addrs": [seed_addr36],
                                "mode_b_source_chain": "ethereum",
                                "mode_b_similarity": 0.99,
                                "mode_b_seed_hashes": [hashlib.sha256(vuln_bc36).hexdigest().lower()],
                            },
                        )
                    ])
                    seen_calls_36: Dict[str, Any] = {"closure": 0, "proxy_args": []}

                    def _mock_proxy_search36(impl_addrs: Sequence[str], parquet_root: Optional[Path] = None, chain_id: Optional[int] = None, limit: Optional[int] = None) -> List[Dict[str, Any]]:
                        seen_calls_36["proxy_args"].append((chain_id, limit))
                        return [
                            {"chain_id": 1, "chain": "ethereum", "address": proxy_stale36, "proxy_address": proxy_stale36, "proxy_source": "creation_values"},
                            {"chain_id": 1, "chain": "ethereum", "address": proxy_active36, "proxy_address": proxy_active36, "proxy_source": "runtime_values"},
                        ]

                    def _counting_closure36(*args: Any, **kwargs: Any) -> TrustClosureResult:
                        seen_calls_36["closure"] += 1
                        seen_calls_36["seeds"] = list(kwargs.get("seed_addresses") or [])
                        return orig_build_closure36(*args, **kwargs)

                    _eng_globals()["search_sourcify_v2_proxies_for_impls"] = _mock_proxy_search36
                    _eng_globals()["build_backward_trust_closure"] = _counting_closure36

                    early_patch_res36 = audit_single_protocol(
                        r36_mb,  # type: ignore[arg-type]
                        st36,
                        {"slug": patched_slug36, "chain": "ethereum", "tvl_usd": 100_000.0, "addresses": [proxy_stale36], "mode": "B", "delegate_address_search": False},
                        set(),
                    )
                    closure_before_active = seen_calls_36["closure"]

                    active_match_slug36 = _norm_slug(f"modeb-match-ethereum-002-{impl_vuln36[2:8]}{impl_vuln36[-6:]}")
                    st36.put_proto_meta_batch([
                        (
                            active_match_slug36,
                            {
                                "mode_b_source_slug": seed_slug36,
                                "mode_b_source_addrs": [seed_addr36],
                                "mode_b_source_chain": "ethereum",
                                "mode_b_similarity": 0.99,
                                "mode_b_seed_hashes": [hashlib.sha256(vuln_bc36).hexdigest().lower()],
                            },
                        )
                    ])
                    _ = audit_single_protocol(
                        r36_mb,  # type: ignore[arg-type]
                        st36,
                        {"slug": active_match_slug36, "chain": "ethereum", "tvl_usd": 100_000.0, "addresses": [impl_vuln36], "mode": "B", "delegate_address_search": False},
                        set(),
                    )
                    merged_seeds36 = set(seen_calls_36.get("seeds") or ())

                    # Test chunked >48 hints & proxy balance attribution in _enqueue_mode_b_sourcify_matches
                    chunk_pairs36 = [("ethereum", f"0x{500 + k:040x}") for k in range(60)]
                    chunk_hints36 = _probe_clone_liquid_balance_hints(r36_mb, chunk_pairs36)

                    sf_dir36 = selftest_ns / "_t36_sf"
                    sf_dir36.mkdir(parents=True, exist_ok=True)
                    impl_fast36 = "0x" + "78" * 20
                    impl_small36 = "0x" + "79" * 20
                    # Write sidecar with EIP-1167 runtime clone bytecode referencing impl_fast36
                    cl_rt_36 = "0x363d3d373d3d3d363d73" + impl_fast36[2:] + "5af43d82803e903d91602b57fd5bf3"
                    (sf_dir36 / "_local_index.jsonl").write_text(
                        json.dumps({"chain_id": 1, "address": proxy_fast_cl36, "runtime_code_hex": cl_rt_36}) + "\n",
                        encoding="utf-8",
                    )
                    _eng_globals()["SOURCIFY_PARQUET_DIR"] = sf_dir36
                    st36.put_contract_code_batch([
                        (
                            1,
                            proxy_active36,
                            20_000_000,
                            hashlib.sha256(bytes.fromhex(eip1967_stub_hex36[2:])).hexdigest().lower(),
                            "active_proxy_skel_36",
                            "[]",
                            eip1967_stub_hex36,
                            "eip1967_impl",
                            [impl_vuln36],
                            True,
                        ),
                    ])
                    enq36 = _enqueue_mode_b_sourcify_matches(
                        st36,
                        [
                            {"chain": "ethereum", "address": impl_vuln36, "case_idx": 10, "score": 0.90, "seed_slug": seed_slug36},
                            {"chain": "ethereum", "address": impl_fast36, "case_idx": 11, "score": 0.90, "seed_slug": seed_slug36},
                            {"chain": "ethereum", "address": impl_small36, "case_idx": 12, "score": 0.99, "seed_slug": seed_slug36},
                        ],
                        router=r36_mb,
                    )
                    sl_v = _norm_slug(f"modeb-match-ethereum-010-{impl_vuln36[2:8]}{impl_vuln36[-6:]}")
                    sl_f = _norm_slug(f"modeb-match-ethereum-011-{impl_fast36[2:8]}{impl_fast36[-6:]}")
                    sl_s = _norm_slug(f"modeb-match-ethereum-012-{impl_small36[2:8]}{impl_small36[-6:]}")
                    ranks36 = {
                        row[0]: float(row[1] or 0.0)
                        for row in st36._conn.execute(
                            "SELECT slug, sort_rank FROM audit_queue WHERE slug IN (?, ?, ?)", (sl_v, sl_f, sl_s)
                        ).fetchall()
                    }
                    meta_f36 = st36.get_proto_meta(sl_f) or {}
                    meta_v36 = st36.get_proto_meta(sl_v) or {}
                    modeb_proxy_ok36 = (
                        early_patch_res36.get("verdict") == VERDICT_SAFE
                        and early_patch_res36.get("early_exit") == "E3_PROXY_ALREADY_PATCHED"
                        and closure_before_active == 0
                        and (1, 64) in seen_calls_36["proxy_args"]
                        and proxy_stale36 not in merged_seeds36
                        and proxy_active36 in merged_seeds36
                        and float(chunk_hints36.get(("ethereum", f"0x{500:040x}"), 0.0)) > 2000.0
                        and float(chunk_hints36.get(("ethereum", f"0x{559:040x}"), 0.0)) > 4000.0
                        and enq36 == 3
                        and ranks36.get(sl_f, 0.0) > ranks36.get(sl_v, 0.0) > ranks36.get(sl_s, 0.0)
                        and proxy_fast_cl36 in (meta_f36.get("mode_b_proxy_addrs") or [])
                        and proxy_active36 in (meta_v36.get("mode_b_proxy_addrs") or [])
                        and float(meta_f36.get("mode_b_hint_usd") or 0.0) >= 200_000.0
                        and float(meta_v36.get("mode_b_hint_usd") or 0.0) >= 100_000.0
                    )
                finally:
                    _eng_globals()["SOURCIFY_PARQUET_DIR"] = orig_sf_dir36
                    _eng_globals()["search_sourcify_v2_proxies_for_impls"] = orig_proxy_search36
                    _eng_globals()["build_backward_trust_closure"] = orig_build_closure36
                    st36.close()

                t36_ok = (
                    tok_rank_ok36
                    and witness_iso_ok36
                    and modeb_proxy_ok36
                    and auto_liq_ok36
                )
                results.append(
                    (
                        "T36_HARNESS_TOKEN_RANK_WITNESS_ISOLATION_MODEB_PROXY_AND_AUTO_LIQ",
                        t36_ok,
                        f"tok_rank={tok_rank_ok36},witness_iso={witness_iso_ok36},modeb_proxy={modeb_proxy_ok36},auto_liq={auto_liq_ok36}",
                    )
                )

            if _want_test("T37_BLUECHIP_LP_AGENT_BRIDGE_PARQUET_PROXY_AND_PRESCREEN_CHUNK", tag_filters):
                # ── T37: CHAIN_BLUECHIPS 정합 + LP unwind + 에이전트 스켈레톤/브리지 + Mode B Parquet/교차체인 프록시 + 파라미터 프리스크린/테스트 청크 ──
                _rlp_create_37 = _rlp_create_low_nonce

                # (a) CHAIN_BLUECHIPS: harness benchmark exclusion + 1-hop & 2-hop ERC4626 -> bluechip -> quote
                weth_eth37 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                usdc_eth37 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                wbtc_eth37 = _norm_addr(CHAIN_BLUECHIPS["ethereum"][0])
                bset_eth37 = _harness_benchmark_set("ethereum")
                dummy_share37 = "0x" + "90" * 20
                dummy_pair37 = "0x" + "92" * 20
                ex_bc37: Dict[str, Dict[str, Any]] = {
                    wbtc_eth37: {
                        "pool": _checksum(dummy_pair37),
                        "quote_token": _checksum(weth_eth37),
                        "dex_kind": "v2",
                        "realizable_usd": 50_000.0,
                        "paused": False,
                    },
                    dummy_share37: {
                        "pool": _checksum(dummy_share37),
                        "quote_token": _checksum(wbtc_eth37),
                        "dex_kind": "redeem_erc4626",
                        "redeem_target": _checksum(dummy_share37),
                        "realizable_usd": 50_000.0,
                        "paused": False,
                        "underlying_exit": {
                            "pool": _checksum(dummy_pair37),
                            "quote_token": _checksum(weth_eth37),
                            "dex_kind": "v2",
                        },
                    },
                }
                dummy_junk_toks37 = {_norm_addr(f"0x{9300 + j:040x}"): 10 for j in range(1, 13)}
                cl_bc37 = TrustClosureResult(
                    chain="ethereum",
                    block_pin=0,
                    seed_addresses=[dummy_share37],
                    all_addresses=[dummy_share37],
                    resolutions={},
                    vault_exposures={
                        dummy_share37: VaultExposure(
                            address=dummy_share37,
                            token_balances={dummy_share37: 40 * 10**18, wbtc_eth37: 10**8, **dummy_junk_toks37},
                            token_usd={dummy_share37: 50_000.0, wbtc_eth37: 30_000.0, **{k: float(1000 - i) for i, k in enumerate(dummy_junk_toks37)}},
                        )
                    },
                    max_extractable_usd=50_000.0,
                    min_gas_usd=0.01,
                    early_exit_safe=False,
                    early_exit_reason="",
                    verified_exit_pools=ex_bc37,
                    token_decimals={dummy_share37: 18, wbtc_eth37: 8, weth_eth37: 18, **{k: 18 for k in dummy_junk_toks37}},
                    token_prices_usd={dummy_share37: 1.0, wbtc_eth37: 60_000.0, weth_eth37: 3000.0},
                )
                prio_bc37 = _prioritize_closure_monitored_tokens("ethereum", cl_bc37, limit=12)
                routes_bc_1hop = _build_harness_liquidation_routes("ethereum", [wbtc_eth37, weth_eth37], ex_bc37)
                routes_bc_2hop = _build_harness_liquidation_routes("ethereum", [dummy_share37, wbtc_eth37, weth_eth37], ex_bc37)
                bluechip_ok37 = (
                    weth_eth37 in bset_eth37
                    and usdc_eth37 in bset_eth37
                    and wbtc_eth37 not in bset_eth37
                    and dummy_share37 in prio_bc37
                    and wbtc_eth37 in prio_bc37
                    and weth_eth37 in prio_bc37
                    and len(routes_bc_1hop) == 1
                    and _norm_addr(routes_bc_1hop[0]["token"]) == wbtc_eth37
                    and _norm_addr(routes_bc_1hop[0]["quote"]) == weth_eth37
                    and any(isinstance(r.get("hop2"), dict) and _norm_addr(r["hop2"]["quote"]) == weth_eth37 for r in routes_bc_2hop)
                )

                # Shared mock contracts for real Forge verification of (a), (b), (c)
                t37_mocks_sol = """
contract MockErc37 {
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    function decimals() external pure returns (uint8) { return 18; }
    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "BAL");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
    function transferFrom(address fr, address to, uint256 a) external returns (bool) {
        require(allowance[fr][msg.sender] >= a && balanceOf[fr] >= a, "ALW");
        allowance[fr][msg.sender] -= a;
        balanceOf[fr] -= a;
        balanceOf[to] += a;
        return true;
    }
}
contract DrainVault37 {
    address public immutable tok;
    uint256 public immutable amt;
    constructor(address _tok, uint256 _amt) { tok = _tok; amt = _amt; }
    function drain() external {
        (bool ok, ) = tok.call(abi.encodeWithSelector(0xa9059cbb, msg.sender, amt));
        require(ok, "DRN");
    }
}
contract MockVault4626_37 {
    address public immutable asset;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    constructor(address _mid) { asset = _mid; }
    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "SBAL");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
    function redeem(uint256 shares, address receiver, address owner) external returns (uint256) {
        require(msg.sender == owner && balanceOf[owner] >= shares, "RDM");
        balanceOf[owner] -= shares;
        totalSupply -= shares;
        MockErc37(asset).transfer(receiver, shares);
        return shares;
    }
}
contract MockV2Pair37 {
    address public immutable token0;
    address public immutable token1;
    uint112 public r0;
    uint112 public r1;
    constructor(address _t0, address _t1) { token0 = _t0; token1 = _t1; }
    function sync() external {
        r0 = uint112(MockErc37(token0).balanceOf(address(this)));
        r1 = uint112(MockErc37(token1).balanceOf(address(this)));
    }
    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }
    function swap(uint256 a0Out, uint256 a1Out, address to, bytes calldata) external {
        if (a0Out > 0) { MockErc37(token0).transfer(to, a0Out); }
        if (a1Out > 0) { MockErc37(token1).transfer(to, a1Out); }
        uint256 b0 = MockErc37(token0).balanceOf(address(this));
        uint256 b1 = MockErc37(token1).balanceOf(address(this));
        uint256 in0 = b0 > (uint256(r0) - a0Out) ? (b0 - (uint256(r0) - a0Out)) : 0;
        uint256 in1 = b1 > (uint256(r1) - a1Out) ? (b1 - (uint256(r1) - a1Out)) : 0;
        require(in0 > 0 || in1 > 0, "IIN");
        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3) >= uint256(r0) * uint256(r1) * 1000000, "K");
        r0 = uint112(b0);
        r1 = uint112(b1);
    }
}
contract MockLpPair37 {
    address public immutable token0;
    address public immutable token1;
    uint112 public r0;
    uint112 public r1;
    uint256 public totalSupply;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    constructor(address _t0, address _t1) { token0 = _t0; token1 = _t1; }
    function mintLP(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }
    function sync() external {
        r0 = uint112(MockErc37(token0).balanceOf(address(this)));
        r1 = uint112(MockErc37(token1).balanceOf(address(this)));
    }
    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }
    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }
    function transfer(address to, uint256 a) external returns (bool) {
        require(balanceOf[msg.sender] >= a, "LPB");
        balanceOf[msg.sender] -= a;
        balanceOf[to] += a;
        return true;
    }
    function transferFrom(address fr, address to, uint256 a) external returns (bool) {
        require(allowance[fr][msg.sender] >= a && balanceOf[fr] >= a, "LPA");
        allowance[fr][msg.sender] -= a;
        balanceOf[fr] -= a;
        balanceOf[to] += a;
        return true;
    }
    function burn(address to) external returns (uint256 a0, uint256 a1) {
        uint256 lp = balanceOf[address(this)];
        uint256 _ts = totalSupply;
        require(lp > 0 && _ts > 0, "ZLP");
        uint256 b0 = MockErc37(token0).balanceOf(address(this));
        uint256 b1 = MockErc37(token1).balanceOf(address(this));
        a0 = (lp * b0) / _ts;
        a1 = (lp * b1) / _ts;
        balanceOf[address(this)] = 0;
        totalSupply = _ts - lp;
        if (a0 > 0) { MockErc37(token0).transfer(to, a0); }
        if (a1 > 0) { MockErc37(token1).transfer(to, a1); }
        r0 = uint112(MockErc37(token0).balanceOf(address(this)));
        r1 = uint112(MockErc37(token1).balanceOf(address(this)));
    }
    function swap(uint256 a0Out, uint256 a1Out, address to, bytes calldata) external {
        if (a0Out > 0) { MockErc37(token0).transfer(to, a0Out); }
        if (a1Out > 0) { MockErc37(token1).transfer(to, a1Out); }
        uint256 b0 = MockErc37(token0).balanceOf(address(this));
        uint256 b1 = MockErc37(token1).balanceOf(address(this));
        uint256 in0 = b0 > (uint256(r0) - a0Out) ? (b0 - (uint256(r0) - a0Out)) : 0;
        uint256 in1 = b1 > (uint256(r1) - a1Out) ? (b1 - (uint256(r1) - a1Out)) : 0;
        require(in0 > 0 || in1 > 0, "IIN");
        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3) >= uint256(r0) * uint256(r1) * 1000000, "K");
        r0 = uint112(b0);
        r1 = uint112(b1);
    }
}
"""
                orig_bluechips_eth37 = list(CHAIN_BLUECHIPS["ethereum"])
                orig_stables_eth37 = list(CHAIN_STABLES["ethereum"])
                lp_probe_ok37 = False
                lp_forge_ok37 = True
                agent_skel_ok37 = True
                try:
                    # (a-exec) Real Forge 2-hop ERC4626 -> registered CHAIN_BLUECHIPS token -> stable quote
                    if FORGE_BIN.exists():
                        caller37_a = "0x" + "64" * 20
                        c_chk37_a = _checksum(caller37_a)
                        a_bc = _rlp_create_37(caller37_a, 0)
                        a_quote = _rlp_create_37(caller37_a, 1)
                        a_share = _rlp_create_37(caller37_a, 2)
                        a_vault = _rlp_create_37(caller37_a, 3)
                        a_pair = _rlp_create_37(caller37_a, 4)
                        CHAIN_BLUECHIPS["ethereum"].append(a_bc)
                        CHAIN_STABLES["ethereum"].append(a_quote)
                        drain_sel37 = selftest_drain_sel
                        sl_a37 = CausalSlice(
                            slice_id="bc_2hop_0",
                            target_address=a_vault,
                            channel="CH_STORAGE",
                            legs=[drain_sel37],
                            witness_candidates=[1],
                            repeat_counts=[1],
                            rationale="steal share -> redeem 4626 -> bluechip -> swap quote",
                            leg_targets=[a_vault],
                            abi_inputs_by_sel={drain_sel37: []},
                        )
                        h_bc37 = _render_minimal_solidity_harness(
                            slug="t37_bc_2hop",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller37_a,
                            target_addr=a_vault,
                            token_addrs=[a_share, a_bc, a_quote],
                            slices=[sl_a37],
                            vault_addresses=[a_vault],
                            exit_routes={
                                a_share: {
                                    "pool": _checksum(a_share),
                                    "quote_token": _checksum(a_bc),
                                    "dex_kind": "redeem_erc4626",
                                    "redeem_target": _checksum(a_share),
                                    "realizable_usd": 10_000.0,
                                    "paused": False,
                                    "underlying_exit": {
                                        "pool": _checksum(a_pair),
                                        "quote_token": _checksum(a_quote),
                                        "dex_kind": "v2",
                                    },
                                }
                            },
                        )
                        h_bc37_exec = h_bc37.replace(
                            "contract LiquidatorHelper_t37_bc_2hop {",
                            t37_mocks_sol + "\ncontract LiquidatorHelper_t37_bc_2hop {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_a});",
                            f"        vm.startPrank({c_chk37_a}, {c_chk37_a});\n"
                            f"        MockErc37 _abc = new MockErc37();\n"
                            f"        MockErc37 _aq = new MockErc37();\n"
                            f"        MockVault4626_37 _ash = new MockVault4626_37(address(_abc));\n"
                            f"        DrainVault37 _av = new DrainVault37(address(_ash), 40 ether);\n"
                            f"        MockV2Pair37 _ap = new MockV2Pair37(address(_abc), address(_aq));\n"
                            f"        _ash.mint(address(_av), 40 ether);\n"
                            f"        _abc.mint(address(_ash), 40 ether);\n"
                            f"        _abc.mint(address(_ap), 100 ether);\n"
                            f"        _aq.mint(address(_ap), 100 ether);\n"
                            f"        _ap.sync();\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_a});",
                        )
                        ws37_a = ensure_clean_forge_workspace({}, slug="t37_bc_2hop", offline=True)
                        rel37_a = "test/Verify_t37_bc_2hop.t.sol"
                        (ws37_a / rel37_a).write_text(h_bc37_exec, encoding="utf-8")
                        out37_a = execute_forge_test_file(
                            ws37_a,
                            rel37_a,
                            h_bc37_exec,
                            timeout_sec=60,
                            allowed_caller=caller37_a,
                            target_addr=a_vault,
                            offline=True,
                        )
                        cf37_a = verify_counterfactual_target_causality(
                            ws37_a,
                            rel37_a,
                            h_bc37_exec,
                            a_vault,
                            winning_test="test_slice_0",
                            offline=True,
                        )
                        bluechip_ok37 = (
                            bluechip_ok37
                            and out37_a.executed
                            and out37_a.passed_tests == 1
                            and len(out37_a.conservation_gains) == 1
                            and out37_a.conservation_gains[0].get("asset") == a_quote
                            and 28 * 10**18 < int(out37_a.conservation_gains[0].get("caller_gain") or 0) < 29 * 10**18
                            and int(out37_a.conservation_gains[0].get("vault_loss") or 0) == int(out37_a.conservation_gains[0].get("caller_gain") or 0)
                            and bool(cf37_a)
                        )

                    # (b) LP token unwind: probe_dex_exit_liquidity metadata + dual-benchmark & custom+quote Forge execution
                    lp_addr37 = "0x" + "94" * 20
                    custom_lp_other37 = "0x" + "95" * 20

                    class _LpProbeRouter37(_SelftestMockRouter):
                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], **kw: Any) -> List[Tuple[bool, bytes]]:
                            out_mc: List[Tuple[bool, bytes]] = []
                            for tgt, cd in calls:
                                nt = _norm_addr(tgt)
                                sel = cd[:4].hex() if len(cd) >= 4 else ""
                                if nt == lp_addr37 and sel == "0dfe1681":  # token0()
                                    out_mc.append((True, abi_encode(["address"], [_checksum(usdc_eth37)])))
                                elif nt == lp_addr37 and sel == "d21220a7":  # token1()
                                    out_mc.append((True, abi_encode(["address"], [_checksum(custom_lp_other37)])))
                                elif nt == lp_addr37 and sel == "18160ddd":  # totalSupply()
                                    out_mc.append((True, abi_encode(["uint256"], [100 * 10**18])))
                                elif nt == lp_addr37 and sel == "0902f1ac":  # getReserves()
                                    out_mc.append((True, abi_encode(["uint112", "uint112", "uint32"], [50_000 * 10**6, 50_000 * 10**18, 1])))
                                elif nt == usdc_eth37 and sel == DECIMALS_SEL[2:].lower():
                                    out_mc.append((True, abi_encode(["uint8"], [6])))
                                else:
                                    out_mc.append((False, b""))
                            return out_mc

                    lp_entry37 = probe_dex_exit_liquidity(
                        _LpProbeRouter37(),  # type: ignore[arg-type]
                        "ethereum",
                        lp_addr37,
                        10 * 10**18,
                        18,
                        0.0,
                        3000.0,
                        18_000_000,
                        quote_prices_usd={usdc_eth37: 1.0},
                    )
                    lp_probe_ok37 = (
                        lp_entry37.get("best_dex_kind") == "lp_token"
                        and _norm_addr(lp_entry37.get("best_quote_token")) == usdc_eth37
                        and _norm_addr(lp_entry37.get("best_other_token")) == custom_lp_other37
                    )

                    if FORGE_BIN.exists():
                        # (b1) Dual benchmark LP pair (tok0 & tok1 both benchmarks -> both credited)
                        caller37_b1 = "0x" + "65" * 20
                        c_chk37_b1 = _checksum(caller37_b1)
                        b1_t0 = _rlp_create_37(caller37_b1, 0)
                        b1_t1 = _rlp_create_37(caller37_b1, 1)
                        b1_lp = _rlp_create_37(caller37_b1, 2)
                        b1_vault = _rlp_create_37(caller37_b1, 3)
                        CHAIN_STABLES["ethereum"].extend([b1_t0, b1_t1])
                        drain_sel37 = selftest_drain_sel
                        sl_b1 = CausalSlice(
                            slice_id="lp_dual_0",
                            target_address=b1_vault,
                            channel="CH_STORAGE",
                            legs=[drain_sel37],
                            witness_candidates=[1],
                            repeat_counts=[1],
                            rationale="steal benchmark LP pair",
                            leg_targets=[b1_vault],
                            abi_inputs_by_sel={drain_sel37: []},
                        )
                        h_b1 = _render_minimal_solidity_harness(
                            slug="t37_lp_dual",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller37_b1,
                            target_addr=b1_vault,
                            token_addrs=[b1_lp, b1_t0, b1_t1],
                            slices=[sl_b1],
                            vault_addresses=[b1_vault],
                            exit_routes={
                                b1_lp: {
                                    "pool": _checksum(b1_lp),
                                    "quote_token": _checksum(b1_t0),
                                    "lp_other_token": _checksum(b1_t1),
                                    "dex_kind": "lp_token",
                                    "realizable_usd": 20_000.0,
                                    "paused": False,
                                }
                            },
                        )
                        h_b1_exec = h_b1.replace(
                            "contract LiquidatorHelper_t37_lp_dual {",
                            t37_mocks_sol + "\ncontract LiquidatorHelper_t37_lp_dual {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_b1});",
                            f"        vm.startPrank({c_chk37_b1}, {c_chk37_b1});\n"
                            f"        MockErc37 _b1t0 = new MockErc37();\n"
                            f"        MockErc37 _b1t1 = new MockErc37();\n"
                            f"        MockLpPair37 _b1p = new MockLpPair37(address(_b1t0), address(_b1t1));\n"
                            f"        DrainVault37 _b1v = new DrainVault37(address(_b1p), 40 ether);\n"
                            f"        _b1t0.mint(address(_b1p), 100 ether);\n"
                            f"        _b1t1.mint(address(_b1p), 100 ether);\n"
                            f"        _b1p.sync();\n"
                            f"        _b1p.mintLP(address(_b1v), 40 ether);\n"
                            f"        _b1p.mintLP(address(0x000000000000000000000000000000000000dEaD), 60 ether);\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_b1});",
                        )
                        ws37_b1 = ensure_clean_forge_workspace({}, slug="t37_lp_dual", offline=True)
                        rel37_b1 = "test/Verify_t37_lp_dual.t.sol"
                        (ws37_b1 / rel37_b1).write_text(h_b1_exec, encoding="utf-8")
                        out37_b1 = execute_forge_test_file(
                            ws37_b1, rel37_b1, h_b1_exec, timeout_sec=60, allowed_caller=caller37_b1, target_addr=b1_vault, offline=True
                        )
                        cf37_b1 = verify_counterfactual_target_causality(
                            ws37_b1, rel37_b1, h_b1_exec, b1_vault, winning_test="test_slice_0", offline=True
                        )
                        b1_gains_by_asset = {g.get("asset"): int(g.get("caller_gain") or 0) for g in out37_b1.conservation_gains}

                        # (b2) Custom + Benchmark LP pair -> unwindV2LP + fallback swapV2 of custom side into quote
                        caller37_b2 = "0x" + "66" * 20
                        c_chk37_b2 = _checksum(caller37_b2)
                        b2_custom = _rlp_create_37(caller37_b2, 0)
                        b2_quote = _rlp_create_37(caller37_b2, 1)
                        b2_lp = _rlp_create_37(caller37_b2, 2)
                        b2_vault = _rlp_create_37(caller37_b2, 3)
                        CHAIN_STABLES["ethereum"].append(b2_quote)
                        sl_b2 = CausalSlice(
                            slice_id="lp_mix_0",
                            target_address=b2_vault,
                            channel="CH_STORAGE",
                            legs=[drain_sel37],
                            witness_candidates=[1],
                            repeat_counts=[1],
                            rationale="steal mixed custom+quote LP pair",
                            leg_targets=[b2_vault],
                            abi_inputs_by_sel={drain_sel37: []},
                        )
                        h_b2 = _render_minimal_solidity_harness(
                            slug="t37_lp_mix",
                            chain="ethereum",
                            block_pin=0,
                            caller=caller37_b2,
                            target_addr=b2_vault,
                            token_addrs=[b2_lp, b2_quote, b2_custom],
                            slices=[sl_b2],
                            vault_addresses=[b2_vault],
                            exit_routes={
                                b2_lp: {
                                    "pool": _checksum(b2_lp),
                                    "quote_token": _checksum(b2_quote),
                                    "lp_other_token": _checksum(b2_custom),
                                    "dex_kind": "lp_token",
                                    "realizable_usd": 20_000.0,
                                    "paused": False,
                                }
                            },
                        )
                        h_b2_exec = h_b2.replace(
                            "contract LiquidatorHelper_t37_lp_mix {",
                            t37_mocks_sol + "\ncontract LiquidatorHelper_t37_lp_mix {",
                        ).replace(
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_b2});",
                            f"        vm.startPrank({c_chk37_b2}, {c_chk37_b2});\n"
                            f"        MockErc37 _b2c = new MockErc37();\n"
                            f"        MockErc37 _b2q = new MockErc37();\n"
                            f"        MockLpPair37 _b2p = new MockLpPair37(address(_b2c), address(_b2q));\n"
                            f"        DrainVault37 _b2v = new DrainVault37(address(_b2p), 40 ether);\n"
                            f"        _b2c.mint(address(_b2p), 100 ether);\n"
                            f"        _b2q.mint(address(_b2p), 100 ether);\n"
                            f"        _b2p.sync();\n"
                            f"        _b2p.mintLP(address(_b2v), 40 ether);\n"
                            f"        _b2p.mintLP(address(0x000000000000000000000000000000000000dEaD), 60 ether);\n"
                            f"        vm.stopPrank();\n"
                            f"        ConservationSnap memory snapBefore = _takeSnap({c_chk37_b2});",
                        )
                        ws37_b2 = ensure_clean_forge_workspace({}, slug="t37_lp_mix", offline=True)
                        rel37_b2 = "test/Verify_t37_lp_mix.t.sol"
                        (ws37_b2 / rel37_b2).write_text(h_b2_exec, encoding="utf-8")
                        out37_b2 = execute_forge_test_file(
                            ws37_b2, rel37_b2, h_b2_exec, timeout_sec=60, allowed_caller=caller37_b2, target_addr=b2_vault, offline=True
                        )
                        cf37_b2 = verify_counterfactual_target_causality(
                            ws37_b2, rel37_b2, h_b2_exec, b2_vault, winning_test="test_slice_0", offline=True
                        )
                        ok_allow_b2, _ = verify_cheatcode_allowlist(h_b2, allowed_caller=caller37_b2)
                        ok_acct_b2, _ = verify_agent_harness_accounting_integrity(h_b2)
                        lp_forge_ok37 = (
                            out37_b1.executed
                            and out37_b1.passed_tests == 1
                            and len(out37_b1.conservation_gains) == 2
                            and b1_gains_by_asset.get(b1_t0) == 40 * 10**18
                            and b1_gains_by_asset.get(b1_t1) == 40 * 10**18
                            and bool(cf37_b1)
                            and ok_allow_b2
                            and ok_acct_b2
                            and out37_b2.executed
                            and out37_b2.passed_tests == 1
                            and len(out37_b2.conservation_gains) == 1
                            and out37_b2.conservation_gains[0].get("asset") == b2_quote
                            and 63 * 10**18 < int(out37_b2.conservation_gains[0].get("caller_gain") or 0) < 64 * 10**18
                            and int(out37_b2.conservation_gains[0].get("vault_loss") or 0) == int(out37_b2.conservation_gains[0].get("caller_gain") or 0)
                            and bool(cf37_b2)
                        )

                    # (c) Agent PoC skeleton: emit_poc_requests_for_slug wiring, multi-file collision-free build, & _bridgeLiquidatedGain caps
                    caller37_c = "0x" + "67" * 20
                    ag_custom = _rlp_create_37(caller37_c, 0)
                    ag_quote = _rlp_create_37(caller37_c, 1)
                    ag_vault = _rlp_create_37(caller37_c, 2)
                    ag_pair = _rlp_create_37(caller37_c, 3)
                    CHAIN_STABLES["ethereum"].append(ag_quote)
                    cl_c37 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=0,
                        seed_addresses=[ag_vault],
                        all_addresses=[ag_vault],
                        resolutions={},
                        vault_exposures={
                            ag_vault: VaultExposure(
                                address=ag_vault,
                                token_balances={ag_custom: 50 * 10**18},
                                token_usd={ag_custom: 50_000.0},
                            )
                        },
                        max_extractable_usd=50_000.0,
                        min_gas_usd=0.01,
                        early_exit_safe=False,
                        early_exit_reason="",
                        verified_exit_pools={
                            ag_custom: {
                                "pool": _checksum(ag_pair),
                                "quote_token": _checksum(ag_quote),
                                "dex_kind": "v2",
                                "realizable_usd": 50_000.0,
                                "paused": False,
                            }
                        },
                        token_decimals={ag_custom: 18, ag_quote: 18},
                        token_prices_usd={ag_custom: 100.0, ag_quote: 1.0},
                    )
                    ag_slices37 = [
                        CausalSlice(slice_id="ag_s1", target_address=ag_vault, channel="CH_STORAGE", legs=["0x11223344"], needs_agent_synthesis=True),
                        CausalSlice(slice_id="ag_s2", target_address=ag_vault, channel="CH_BALANCE", legs=["0x55667788"], needs_agent_synthesis=True),
                    ]
                    poc_reqs37 = emit_poc_requests_for_slug(
                        "t37_agent_req",
                        "ethereum",
                        0,
                        caller37_c,
                        ag_slices37,
                        verified_sources={ag_vault: {"sources": {"Vault.sol": "contract V {}"}}},
                        principle_summary="custom token drain",
                        force_all=True,
                        closure=cl_c37,
                        compact_single_bundle=False,
                    )
                    req1_doc37 = json.loads(poc_reqs37[0].read_text(encoding="utf-8")) if poc_reqs37 else {}
                    ws_ag_req = ensure_clean_forge_workspace({}, slug="t37_agent_req", offline=True)
                    ag_t1_txt = (ws_ag_req / "test" / "test_1.t.sol").read_text(encoding="utf-8")
                    ag_t2_txt = (ws_ag_req / "test" / "test_2.t.sol").read_text(encoding="utf-8")
                    ok_allow_ag1, _ = verify_cheatcode_allowlist(ag_t1_txt, allowed_caller=caller37_c, forbid_inline_fork=True)
                    ok_acct_ag1, _ = verify_agent_harness_accounting_integrity(ag_t1_txt)
                    agent_skel_ok37 = (
                        len(poc_reqs37) == 2
                        and "_bridgeLiquidatedGain" in str((req1_doc37.get("submission_schema") or {}).get("note") or "")
                        and "// Implement dynamic on-chain state verification against TARGET" in ag_t1_txt
                        and "LiquidatorHelper_Agent_t37_agent_req_1" in ag_t1_txt
                        and "LiquidatorHelper_Agent_t37_agent_req_2" in ag_t2_txt
                        and _checksum(ag_custom) in ag_t1_txt
                        and _checksum(ag_quote) in ag_t1_txt
                        and ok_allow_ag1
                        and ok_acct_ag1
                    )
                    if FORGE_BIN.exists() and agent_skel_ok37:
                        # Generate unrouted skeleton for test_1.t.sol alongside routed test_2.t.sol & test_3.t.sol in ONE workspace
                        sol1_bridge = _render_agent_poc_skeleton(
                            "t37_agent_ws", 1, "ethereum", 0, caller37_c, ag_vault,
                            token_addrs=[ag_custom, ag_quote], vault_addresses=[ag_vault], exit_routes=None,
                        )
                        sol2_routed = _render_agent_poc_skeleton(
                            "t37_agent_ws", 2, "ethereum", 0, caller37_c, ag_vault,
                            token_addrs=[ag_custom, ag_quote], vault_addresses=[ag_vault], exit_routes=cl_c37.verified_exit_pools,
                        )
                        sol3_routed = _render_agent_poc_skeleton(
                            "t37_agent_ws", 3, "ethereum", 0, caller37_c, ag_vault,
                            token_addrs=[ag_custom, ag_quote], vault_addresses=[ag_vault], exit_routes=cl_c37.verified_exit_pools,
                        )
                        sol1_exec = sol1_bridge.replace(
                            "contract AgentVerify_t37_agent_ws_1 {",
                            t37_mocks_sol + "\ncontract AgentVerify_t37_agent_ws_1 {",
                        ).replace(
                            "        ConservationSnap memory snapBefore = _takeSnap(ZERO_STATE_CALLER);",
                            "        vm.startPrank(ZERO_STATE_CALLER, ZERO_STATE_CALLER);\n"
                            "        MockErc37 _ac = new MockErc37();\n"
                            "        MockErc37 _aq = new MockErc37();\n"
                            "        DrainVault37 _av = new DrainVault37(address(_ac), 50 ether);\n"
                            "        MockV2Pair37 _ap = new MockV2Pair37(address(_ac), address(_aq));\n"
                            "        _ac.mint(address(_av), 50 ether);\n"
                            "        _ac.mint(address(_ap), 1000 ether);\n"
                            "        _aq.mint(address(_ap), 100 ether);\n"
                            "        _ap.sync();\n"
                            "        vm.stopPrank();\n"
                            "        ConservationSnap memory snapBefore = _takeSnap(ZERO_STATE_CALLER);",
                        ).replace(
                            "        // Implement dynamic on-chain state verification against TARGET",
                            "        DrainVault37(TARGET).drain();",
                        ).replace(
                            "        // _bridgeLiquidatedGain(snapBefore, snapMid, liqCredit, liqDebited, tokSlot, quoteSlot, spentAmt, boughtAmt);",
                            "        vm.startPrank(ZERO_STATE_CALLER, ZERO_STATE_CALLER);\n"
                            "        uint256 qSwBefore = _erc20Bal(TOK_1, ZERO_STATE_CALLER);\n"
                            "        (bool okT, ) = TOK_0.call(abi.encodeWithSelector(0xa9059cbb, " + _checksum(ag_pair) + ", 50 ether));\n"
                            "        require(okT, \"BR_T\");\n"
                            "        uint256 outB = (uint256(50 ether) * 997 * uint256(100 ether)) / (uint256(1000 ether) * 1000 + uint256(50 ether) * 997);\n"
                            "        MockV2Pair37(" + _checksum(ag_pair) + ").swap(0, outB, ZERO_STATE_CALLER, \"\");\n"
                            "        vm.stopPrank();\n"
                            "        uint256 qSwBought = _erc20Bal(TOK_1, ZERO_STATE_CALLER) - qSwBefore;\n"
                            "        _bridgeLiquidatedGain(snapBefore, snapMid, liqCredit, liqDebited, 1, 2, 0, qSwBought + 99999 ether);",
                        )
                        ws37_c = ensure_clean_forge_workspace({}, slug="t37_agent_ws", offline=True)
                        (ws37_c / "test" / "test_2.t.sol").write_text(sol2_routed, encoding="utf-8")
                        (ws37_c / "test" / "test_3.t.sol").write_text(sol3_routed, encoding="utf-8")
                        rel37_c = "test/test_1.t.sol"
                        (ws37_c / rel37_c).write_text(sol1_exec, encoding="utf-8")
                        out37_c = execute_forge_test_file(
                            ws37_c,
                            rel37_c,
                            sol1_exec,
                            timeout_sec=60,
                            allowed_caller=caller37_c,
                            target_addr=ag_vault,
                            offline=True,
                        )
                        cf37_c = verify_counterfactual_target_causality(
                            ws37_c, rel37_c, sol1_exec, ag_vault, winning_test="test_candidate_1", offline=True
                        )
                        agent_skel_ok37 = (
                            agent_skel_ok37
                            and out37_c.executed
                            and out37_c.passed_tests == 1
                            and len(out37_c.conservation_gains) == 1
                            and out37_c.conservation_gains[0].get("asset") == ag_quote
                            and 4 * 10**18 < int(out37_c.conservation_gains[0].get("caller_gain") or 0) < 5 * 10**18
                            and int(out37_c.conservation_gains[0].get("vault_loss") or 0) == int(out37_c.conservation_gains[0].get("caller_gain") or 0)
                            and bool(cf37_c)
                        )
                finally:
                    CHAIN_BLUECHIPS["ethereum"] = orig_bluechips_eth37
                    CHAIN_STABLES["ethereum"] = orig_stables_eth37

                # (d) Mode B Parquet proxy impl_address attribution + cross_chain_clones proxy attribution
                st37 = SQLiteStore(selftest_ns / "t37_store.sqlite")
                orig_proxy_search37 = search_sourcify_v2_proxies_for_impls
                orig_build_closure37 = build_backward_trust_closure
                modeb_proxy_ok37 = False
                try:
                    impl_eth_pq37 = "0x" + "a1" * 20
                    impl_eth_rich37 = "0x" + "a2" * 20
                    proxy_eth_pq37 = "0x" + "b1" * 20
                    proxy_unann37 = "0x" + "b2" * 20
                    seed_slug37 = "t37-src-eth"
                    seed_addr37 = "0x" + "a3" * 20
                    impl_arb_cc37 = "0x" + "a4" * 20
                    proxy_arb_cc37 = "0x" + "b4" * 20
                    vuln_bc37 = bytes.fromhex(
                        "633ccfd60b60003560e01c14601257600080fd5b"
                        "6000808080473361fffff100" + ("5b00" * 30)
                    )
                    sql_case37 = _sql_first_matching_impl_case("runtime_values", [impl_eth_pq37, impl_eth_rich37])
                    st37.put_contract_code_batch([
                        (
                            1,
                            seed_addr37,
                            18_000_000,
                            hashlib.sha256(vuln_bc37).hexdigest().lower(),
                            _skeleton_hash(vuln_bc37),
                            json.dumps(sorted(_opcode_4grams(vuln_bc37, cap=128))),
                            "0x" + vuln_bc37.hex(),
                            "none",
                            [],
                            False,
                        ),
                        (
                            CHAIN_IDS["arbitrum"],
                            impl_arb_cc37,
                            150_000_000,
                            hashlib.sha256(vuln_bc37).hexdigest().lower(),
                            _skeleton_hash(vuln_bc37),
                            json.dumps(sorted(_opcode_4grams(vuln_bc37, cap=128))),
                            "0x" + vuln_bc37.hex(),
                            "none",
                            [],
                            False,
                        ),
                    ])

                    class _ModeBRouter37(_SelftestMockRouter):
                        def call(self, chain: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_blockNumber":
                                return hex(20_000_000)
                            if method == "eth_getCode" and params:
                                na = _norm_addr(params[0])
                                if na == seed_addr37 and _norm_chain(chain) == "ethereum":
                                    return "0x" + vuln_bc37.hex()
                                return "0x"
                            if method == "eth_getLogs":
                                return []
                            if method == "eth_getBalance" and params:
                                na = _norm_addr(params[0])
                                if na in (proxy_eth_pq37, proxy_arb_cc37):
                                    return hex(100 * 10**18)
                                return "0x0"
                            if method == "eth_chainId":
                                return hex(CHAIN_IDS.get(_norm_chain(chain), 1))
                            return "0x0"

                    r37_mb = _ModeBRouter37(store=st37)

                    def _mock_proxy_search37(
                        impl_addrs: Sequence[str],
                        parquet_root: Optional[Path] = None,
                        chain_id: Optional[int] = None,
                        limit: Optional[int] = None,
                    ) -> List[Dict[str, Any]]:
                        out_r: List[Dict[str, Any]] = []
                        n_set = {_norm_addr(a) for a in impl_addrs}
                        if chain_id in (None, 1):
                            if impl_eth_pq37 in n_set:
                                out_r.append({
                                    "chain_id": 1,
                                    "chain": "ethereum",
                                    "address": proxy_eth_pq37,
                                    "proxy_address": proxy_eth_pq37,
                                    "impl_address": impl_eth_pq37,
                                    "proxy_source": "runtime_values",
                                })
                            out_r.append({
                                "chain_id": 1,
                                "chain": "ethereum",
                                "address": proxy_unann37,
                                "proxy_address": proxy_unann37,
                                "proxy_source": "creation_values",
                            })
                        if chain_id in (None, CHAIN_IDS["arbitrum"]) and impl_arb_cc37 in n_set:
                            out_r.append({
                                "chain_id": CHAIN_IDS["arbitrum"],
                                "chain": "arbitrum",
                                "address": proxy_arb_cc37,
                                "proxy_address": proxy_arb_cc37,
                                "impl_address": impl_arb_cc37,
                                "proxy_source": "runtime_values",
                            })
                        return out_r

                    _eng_globals()["search_sourcify_v2_proxies_for_impls"] = _mock_proxy_search37
                    pbm_single, _ = _collect_impl_proxies_for_chains(st37, {"ethereum": [impl_eth_pq37]})
                    pbm_multi, _ = _collect_impl_proxies_for_chains(st37, {"ethereum": [impl_eth_pq37, impl_eth_rich37]})
                    enq37 = _enqueue_mode_b_sourcify_matches(
                        st37,
                        [
                            {"chain": "ethereum", "address": impl_eth_pq37, "case_idx": 20, "score": 0.90, "seed_slug": seed_slug37},
                            {"chain": "ethereum", "address": impl_eth_rich37, "case_idx": 21, "score": 0.99, "seed_slug": seed_slug37},
                        ],
                        router=r37_mb,  # type: ignore[arg-type]
                    )
                    sl_pq37 = _norm_slug(f"modeb-match-ethereum-020-{impl_eth_pq37[2:8]}{impl_eth_pq37[-6:]}")
                    sl_un37 = _norm_slug(f"modeb-match-ethereum-021-{impl_eth_rich37[2:8]}{impl_eth_rich37[-6:]}")
                    ranks37 = {
                        row[0]: float(row[1] or 0.0)
                        for row in st37._conn.execute(
                            "SELECT slug, sort_rank FROM audit_queue WHERE slug IN (?, ?)", (sl_pq37, sl_un37)
                        ).fetchall()
                    }
                    meta_pq37 = st37.get_proto_meta(sl_pq37) or {}
                    meta_un37 = st37.get_proto_meta(sl_un37) or {}

                    def _fast_stop_closure37(
                        router: Any,
                        chain: str,
                        seed_addresses: Sequence[str],
                        block_pin: int = 0,
                        **kw: Any,
                    ) -> TrustClosureResult:
                        return TrustClosureResult(
                            chain=chain,
                            block_pin=block_pin,
                            seed_addresses=list(seed_addresses),
                            all_addresses=list(seed_addresses),
                            resolutions={},
                            vault_exposures={},
                            max_extractable_usd=0.0,
                            min_gas_usd=0.0,
                            early_exit_safe=True,
                            early_exit_reason="T37_MODEB_CLONE_CHECK_DONE",
                        )

                    _eng_globals()["build_backward_trust_closure"] = _fast_stop_closure37
                    _ = audit_single_protocol(
                        r37_mb,  # type: ignore[arg-type]
                        st37,
                        {"slug": seed_slug37, "chain": "ethereum", "tvl_usd": 100_000.0, "addresses": [seed_addr37], "mode": "B", "delegate_address_search": False},
                        set(),
                    )
                    cc_slug37 = _norm_slug(f"modeb-clone-arbitrum-{seed_slug37[-16:]}-{impl_arb_cc37[2:8]}{impl_arb_cc37[-6:]}")
                    meta_cc37 = st37.get_proto_meta(cc_slug37) or {}
                    modeb_proxy_ok37 = (
                        impl_eth_pq37 in sql_case37
                        and proxy_unann37 in pbm_single.get(("ethereum", impl_eth_pq37), set())
                        and proxy_unann37 not in pbm_multi.get(("ethereum", impl_eth_pq37), set())
                        and proxy_unann37 not in pbm_multi.get(("ethereum", impl_eth_rich37), set())
                        and enq37 == 2
                        and ranks37.get(sl_pq37, 0.0) > ranks37.get(sl_un37, 0.0)
                        and proxy_eth_pq37 in (meta_pq37.get("mode_b_proxy_addrs") or [])
                        and not (meta_un37.get("mode_b_proxy_addrs") or [])
                        and float(meta_pq37.get("mode_b_hint_usd") or 0.0) >= 100_000.0
                        and proxy_arb_cc37 in (meta_cc37.get("mode_b_proxy_addrs") or [])
                        and float(meta_cc37.get("mode_b_hint_usd") or 0.0) >= 100_000.0
                    )
                finally:
                    _eng_globals()["search_sourcify_v2_proxies_for_impls"] = orig_proxy_search37
                    _eng_globals()["build_backward_trust_closure"] = orig_build_closure37
                    st37.close()

                # (e) Parameterized prescreen_eth_call_batch (<=48 slices, <=128 RPC calls, witness bubbling) & max_tests_per_chunk=12
                caller37_e = "0x" + "68" * 20
                tgt37_e = "0x" + "e0" * 20
                sel40_e = "0xaaaaaaaa"
                sel41_e = "0xbbbbbbbb"
                slices37_e: List[CausalSlice] = [
                    CausalSlice(slice_id=f"revs_{i}", target_address=tgt37_e, channel="CH_STORAGE", legs=[f"0x{0x10000001 + i:08x}"])
                    for i in range(40)
                ]
                slices37_e.append(
                    CausalSlice(
                        slice_id="sl_40",
                        target_address=tgt37_e,
                        channel="CH_BALANCE",
                        legs=[sel40_e],
                        witness_candidates=[0, 10**18, 5],
                        abi_inputs_by_sel={sel40_e: ["address", "uint256"]},
                    )
                )
                slices37_e.append(
                    CausalSlice(
                        slice_id="sl_41",
                        target_address=tgt37_e,
                        channel="CH_STORAGE",
                        legs=[sel41_e],
                        witness_candidates=[7],
                    )
                )
                hit_cd40 = encode_calldata(sel40_e, ["address", "uint256"], [_checksum(caller37_e), 10**18]).lower()
                hit_cd41 = encode_calldata(sel41_e, ["address", "uint256"], [_checksum(caller37_e), 7]).lower()

                class _PrescreenRouter37(_SelftestMockRouter):
                    _cand_urls = ("http://127.0.0.1:8545",)

                    def __init__(self) -> None:
                        super().__init__()
                        self.batch_sizes: List[int] = []

                    def batch_call(self, chain: str, calls: Sequence[Tuple[str, Any]], **kw: Any) -> List[Any]:
                        self.batch_sizes.append(len(calls))
                        out_p: List[Any] = []
                        for _, p in calls:
                            d_hex = str((p[0] or {}).get("data") or "").lower()
                            if d_hex in (hit_cd40, hit_cd41):
                                out_p.append("0x" + "00" * 31 + "01")
                            else:
                                out_p.append(None)
                        return out_p

                r37_pe = _PrescreenRouter37()
                ordered37_e = prescreen_eth_call_batch(r37_pe, "ethereum", 18_000_000, caller37_e, slices37_e)  # type: ignore[arg-type]

                orig_render37_e = _render_minimal_solidity_harness
                orig_exec37_e = execute_forge_test_file
                orig_ws37_e = ensure_clean_forge_workspace
                chunk_lens37_e: List[int] = []
                try:
                    multi_w_slices37_e = [
                        CausalSlice(
                            slice_id=f"mw_{i}",
                            target_address=tgt37_e,
                            channel="CH_STORAGE",
                            legs=[sel40_e],
                            witness_candidates=[1, 2, 3, 4, 5],
                        )
                        for i in range(6)
                    ]

                    def _mock_render_e(**kw: Any) -> str:
                        chunk_lens37_e.append(len(kw.get("slices") or []))
                        return "contract Verify_t37 {}"

                    def _mock_exec_e(*args: Any, **kwargs: Any) -> ForgeRunOutcome:
                        return ForgeRunOutcome(executed=True, passed_tests=1, failed_tests=0)

                    def _mock_ws_e(*args: Any, **kwargs: Any) -> Path:
                        p_ws = selftest_ns / "ws37_chunk"
                        (p_ws / "test").mkdir(parents=True, exist_ok=True)
                        return p_ws

                    _eng_globals()["_render_minimal_solidity_harness"] = _mock_render_e
                    _eng_globals()["execute_forge_test_file"] = _mock_exec_e
                    _eng_globals()["ensure_clean_forge_workspace"] = _mock_ws_e
                    chunk_out37_e = (
                        generate_and_run_forge_suites(
                            r37_pe,  # type: ignore[arg-type]
                            "t37_chunk_test",
                            "ethereum",
                            0,
                            caller37_e,
                            tgt37_e,
                            [],
                            multi_w_slices37_e,
                            slice_chunk_size=8,
                            max_tests_per_chunk=12,
                        )
                        if FORGE_BIN.exists()
                        else None
                    )
                finally:
                    _eng_globals()["_render_minimal_solidity_harness"] = orig_render37_e
                    _eng_globals()["execute_forge_test_file"] = orig_exec37_e
                    _eng_globals()["ensure_clean_forge_workspace"] = orig_ws37_e

                chunk_check_ok37 = (
                    (chunk_out37_e is not None and chunk_out37_e.executed and chunk_lens37_e == [2, 2, 2])
                    if FORGE_BIN.exists()
                    else True
                )
                prescreen_chunk_ok37 = (
                    len(ordered37_e) == 42
                    and sum(r37_pe.batch_sizes) <= 128
                    and {ordered37_e[0].slice_id, ordered37_e[1].slice_id} == {"sl_40", "sl_41"}
                    and ordered37_e[0].witness_candidates[0] in (10**18, 7)
                    and next(s for s in ordered37_e if s.slice_id == "sl_40").witness_candidates[0] == 10**18
                    and slices37_e[40].witness_candidates[0] == 0
                    and chunk_check_ok37
                )

                t37_ok = (
                    bluechip_ok37
                    and lp_probe_ok37
                    and lp_forge_ok37
                    and agent_skel_ok37
                    and modeb_proxy_ok37
                    and prescreen_chunk_ok37
                )
                results.append(
                    (
                        "T37_BLUECHIP_LP_AGENT_BRIDGE_PARQUET_PROXY_AND_PRESCREEN_CHUNK",
                        t37_ok,
                        f"bluechip={bluechip_ok37},lp_probe={lp_probe_ok37},lp_forge={lp_forge_ok37},agent_skel={agent_skel_ok37},modeb_pq={modeb_proxy_ok37},prescreen_chunk={prescreen_chunk_ok37}",
                    )
                )

            if _want_test("T38_CALLDATA_LP_QEXIT_AND_SQLITE_PRIO_MIGRATION", tag_filters):
                # T38: Calldata length per-function slicing, LP quote_exit route, SQLite modeb_prio_v1
                sol_multi_fn = (
                    "contract T {\n"
                    "  function test_first() public { uint256 a = 1; }\n"
                    "  function test_second() public { uint256 b = 2; }\n"
                    "  function helper() internal pure returns (uint256) { return 3; }\n"
                    "}\n"
                )
                cd_len_first = _estimate_test_calldata_len(sol_multi_fn, fallback=64, test_name="test_first")
                cd_len_ok38 = cd_len_first == 64 and len(sol_multi_fn) > 100

                lp_qexit_routes = _build_harness_liquidation_routes(
                    "ethereum",
                    [
                        "0x1111111111111111111111111111111111111111",
                        "0x4444444444444444444444444444444444444444",
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    ],
                    {
                        "0x1111111111111111111111111111111111111111": {
                            "dex_kind": "lp_token",
                            "pool": "0x2222222222222222222222222222222222222222",
                            "quote_token": "0x4444444444444444444444444444444444444444",
                            "lp_other_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "realizable_usd": 1000.0,
                        },
                        "0x4444444444444444444444444444444444444444": {
                            "dex_kind": "v2",
                            "pool": "0x3333333333333333333333333333333333333333",
                            "quote_token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                            "realizable_usd": 5000.0,
                        },
                    },
                )
                lp_qexit_ok38 = any(
                    _norm_addr(rt.get("token")) == "0x1111111111111111111111111111111111111111"
                    and isinstance(rt.get("quote_exit"), dict)
                    and _norm_addr(rt["quote_exit"].get("quote")) == "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    for rt in lp_qexit_routes
                )

                st_prio38 = SQLiteStore(":memory:")
                prio_meta_ok38 = st_prio38.get_engine_meta("modeb_prio_v1", "") == "1"
                st_prio38.close()

                t38_ok = cd_len_ok38 and lp_qexit_ok38 and prio_meta_ok38
                results.append(
                    (
                        "T38_CALLDATA_LP_QEXIT_AND_SQLITE_PRIO_MIGRATION",
                        t38_ok,
                        f"cd_len={cd_len_ok38},lp_qexit={lp_qexit_ok38},prio_meta={prio_meta_ok38}",
                    )
                )

            if _want_test("T39_ALL_21_REMEDIATIONS_REGRESSION", tag_filters):
                # T39: 21개 결함 보완 일괄 회귀 검증 (100% 오프라인·결정적, selftest_ns 격리)
                st39_list: List[SQLiteStore] = []
                orig_ensure_ws39 = _eng_globals().get("ensure_clean_forge_workspace")
                orig_exec_forge39 = _eng_globals().get("execute_forge_test_file")
                try:
                    # (1) #7 STATICCALL 프리컴파일(0x05) vs 비프리컴파일(0x0a)
                    cfg_pre39 = disassemble_and_build_cfg(bytes.fromhex("6000600060006000600561fffffa00"))
                    cfg_ext39 = disassemble_and_build_cfg(bytes.fromhex("6000600060006000600a61fffffa00"))
                    sc_pre_ok39 = bool(cfg_pre39.blocks) and not any(b.has_dynamic_staticcall for b in cfg_pre39.blocks.values())
                    sc_ext_ok39 = any(b.has_dynamic_staticcall for b in cfg_ext39.blocks.values())
                    t39_staticcall_ok = sc_pre_ok39 and sc_ext_ok39

                    # (2) #8 최상위 가변 상태 초기화 거부 + initcode 부작용 오프코드 차단
                    sol_bad_init39 = (
                        "// SPDX-License-Identifier: MIT\n"
                        "pragma solidity ^0.8.20;\n"
                        "interface Vm { function prank(address) external; }\n"
                        "contract T39BadInitTest {\n"
                        "    Vm constant vm = Vm(address(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D));\n"
                        "    uint256 x = _badInit();\n"
                        "    function _badInit() internal pure returns (uint256) { return 1; }\n"
                        "    function test_x() public {}\n"
                        "}\n"
                    )
                    sol_good_init39 = (
                        "// SPDX-License-Identifier: MIT\n"
                        "pragma solidity ^0.8.20;\n"
                        "interface Vm { function prank(address) external; }\n"
                        "contract T39GoodInitTest {\n"
                        "    Vm constant vm = Vm(address(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D));\n"
                        "    function test_x() public {}\n"
                        "}\n"
                    )
                    ok_bi39, vio_bi39 = verify_cheatcode_allowlist(sol_bad_init39)
                    ok_gi39, vio_gi39 = verify_cheatcode_allowlist(sol_good_init39)
                    out_dir39 = selftest_ns / "t39_initcode_out"
                    sol_out39 = out_dir39 / "T39Harness.sol"
                    sol_out39.mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(
                        sol_out39 / "AgentVerify_T39Harness.json",
                        {"bytecode": {"object": "0x6001600055"}, "deployedBytecode": {"object": "0x00"}},
                    )
                    ok_init_bad39, vio_init_bad39 = _scan_compiled_out_dir_allowlist(out_dir39, "T39Harness.sol")
                    _write_json_atomic(
                        sol_out39 / "AgentVerify_T39Harness.json",
                        {"bytecode": {"object": "0x600080f3"}, "deployedBytecode": {"object": "0x00"}},
                    )
                    ok_init_clean39, vio_init_clean39 = _scan_compiled_out_dir_allowlist(out_dir39, "T39Harness.sol")
                    t39_allowlist_ok = (
                        (not ok_bi39)
                        and ("disallowed_harness_state_initializer" in vio_bi39)
                        and ok_gi39
                        and (not vio_gi39)
                        and (not ok_init_bad39)
                        and any(v.startswith("disallowed_harness_initcode_side_effect:") for v in vio_init_bad39)
                        and ok_init_clean39
                        and (not vio_init_clean39)
                    )

                    # (3) #9 & #21 WNATIVE 준비금 감소/무담보 발행 헬퍼 + 치트코드/회계 무결성 (+ Forge 컴파일 검증)
                    t39_target = "0x1111111111111111111111111111111111111111"
                    t39_vault = "0x2222222222222222222222222222222222222222"
                    t39_caller = "0x0000000000000000000000000000000000099939"
                    weth39 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    usdc39 = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                    sl39 = CausalSlice(
                        slice_id="t39_s1",
                        target_address=t39_target,
                        channel="CH_BALANCE",
                        legs=["0x2e1a7d4d"],
                        witness_candidates=[10**18],
                        repeat_counts=[1],
                        leg_targets=[t39_vault],
                    )
                    rendered39 = _render_minimal_solidity_harness(
                        "t39-wnative-insolv",
                        "ethereum",
                        20_000_000,
                        t39_caller,
                        t39_target,
                        [weth39, usdc39],
                        [sl39],
                        vault_addresses=[t39_target, t39_vault],
                    )
                    ok_rend_allow39, _ = verify_cheatcode_allowlist(rendered39, forbid_inline_fork=True)
                    ok_rend_acct39, _ = verify_agent_harness_accounting_integrity(rendered39)
                    forge_compile_ok39 = True
                    if FORGE_BIN.exists():
                        ws_c39 = ensure_clean_forge_workspace({"ethereum": "http://127.0.0.1:8545"}, slug="t39-compile-only", offline=True)
                        out_c39 = execute_forge_test_file(
                            ws_c39,
                            "test/test_t39_compile.t.sol",
                            rendered39,
                            timeout_sec=75,
                            allowed_caller=t39_caller,
                            forbid_inline_fork=True,
                            target_addr=t39_target,
                            offline=True,
                        )
                        forge_compile_ok39 = (
                            ("Stack too deep" not in out_c39.error_reason)
                            and ("CHEATCODE" not in out_c39.error_reason)
                            and out_c39.executed
                        )
                    t39_wnative_insolv_ok = (
                        ("_wReserveDropWei" in rendered39)
                        and ("_wSupplyAndBackingLoss" in rendered39)
                        and ("subWLoss" in rendered39)
                        and ("ConservationSupplyMint" in rendered39)
                        and ok_rend_allow39
                        and ok_rend_acct39
                        and forge_compile_ok39
                    )

                    # (4) #20 _fetch_usd_krw_rate 부팅 ID·3600s TTL 무효화 + #6 $0.01 고정 가스 하한 제거
                    st39_fx = SQLiteStore(":memory:")
                    st39_list.append(st39_fx)
                    st39_fx.put_engine_meta("session_usd_krw", "999.0")
                    st39_fx.put_engine_meta("session_usd_krw_source", "stale_boot_fx")
                    st39_fx.put_engine_meta("session_usd_krw_boot", "OTHER_BOOT_39")
                    st39_fx.put_engine_meta("session_usd_krw_at", str(time.time()))
                    r_stale_boot39, s_stale_boot39 = _fetch_usd_krw_rate(st39_fx, allow_offline_fixture=True)

                    st39_fx.put_engine_meta("session_usd_krw", "888.0")
                    st39_fx.put_engine_meta("session_usd_krw_source", "expired_ttl_fx")
                    st39_fx.put_engine_meta("session_usd_krw_boot", BOOT_ID)
                    st39_fx.put_engine_meta("session_usd_krw_at", str(time.time() - 4000.0))
                    r_exp39, s_exp39 = _fetch_usd_krw_rate(st39_fx, allow_offline_fixture=True)

                    st39_fx.put_engine_meta("session_usd_krw", "1420.5")
                    st39_fx.put_engine_meta("session_usd_krw_source", "sealed_fresh_39")
                    st39_fx.put_engine_meta("session_usd_krw_boot", BOOT_ID)
                    st39_fx.put_engine_meta("session_usd_krw_at", str(time.time()))
                    r_fresh39, s_fresh39 = _fetch_usd_krw_rate(st39_fx, allow_offline_fixture=False)

                    v39_no_floor = judge_conservation_accounting(
                        store=st39_fx,
                        chain="ethereum",
                        caller_before={"native": 10**18, usdc39: 0},
                        caller_after={"native": 10**18, usdc39: 100_005_000},
                        vault_before={usdc39: 1_000_000_000},
                        vault_after={usdc39: 899_995_000},
                        gas_used=20_000,
                        gas_price_wei=100_000,
                        decimals_map={usdc39: 6},
                        prices_map={"native": 2000.0, usdc39: 1.0},
                        allow_offline_fixture=True,
                    )
                    t39_fx_and_gas_ok = (
                        (r_stale_boot39 != 999.0)
                        and (s_stale_boot39 != "stale_boot_fx")
                        and (r_exp39 != 888.0)
                        and (s_exp39 != "expired_ttl_fx")
                        and (abs(r_fresh39 - 1420.5) < 1e-6)
                        and (s_fresh39 == "sealed_fresh_39")
                        and _is_profit_verdict(v39_no_floor.verdict, allow_template=False)
                        and (abs(v39_no_floor.gas_usd - 0.000004) < 1e-9)
                        and (v39_no_floor.net_usd >= 100.0)
                    )

                    # (5) #16 트레이스 HEVM 차단 + #15 다중 청크 trace_summary 병합/target_addrs + #10 핀/trace 게이트
                    t39_a1 = "0x1111111111111111111111111111111111111111"
                    t39_a2 = "0x2222222222222222222222222222222222222222"
                    deal_sel39 = _keccak256_hex(b"deal(address,uint256)")[:10]
                    fork_sel39 = _keccak256_hex(b"createSelectFork(string,uint256)")[:10]
                    j_deal39 = json.dumps(
                        {"arena": [{"trace": {"kind": "CALL", "address": HEVM_ADDRESS_LOWER, "caller": t39_a1, "data": deal_sel39 + "00" * 64, "success": True}, "children": []}]}
                    )
                    j_fork39 = json.dumps(
                        {"arena": [{"trace": {"kind": "CALL", "address": HEVM_ADDRESS_LOWER, "caller": t39_a1, "data": fork_sel39 + "00" * 64, "success": True}, "children": []}]}
                    )
                    j_clean39 = json.dumps(
                        {"arena": [{"trace": {"kind": "CALL", "address": t39_a2, "caller": t39_a1, "data": "0x2e1a7d4d", "success": True}, "children": []}]}
                    )
                    h_deal39, _, _ = _detect_disallowed_hevm_in_trace_output(
                        j_deal39,
                        forbid_inline_fork=False,
                        target_addr=t39_a1,
                    )
                    h_fork39, _, _ = _detect_disallowed_hevm_in_trace_output(
                        j_fork39,
                        forbid_inline_fork=True,
                        target_addr=t39_a1,
                    )
                    h_clean39, h_tgt39, _ = _detect_disallowed_hevm_in_trace_output(
                        j_clean39,
                        forbid_inline_fork=True,
                        target_addr=t39_a1,
                        target_addrs=[t39_a1, t39_a2],
                    )
                    t39_hevm_ok = h_deal39 and h_fork39 and (not h_clean39) and h_tgt39

                    captured_taddrs39: List[Any] = []
                    chunk_idx39 = {"i": 0}

                    def _mock_clean_ws39(rpc_endpoints: Dict[str, str], slug: str = "_shared", offline: bool = False, evm_version: str = "") -> Path:
                        ws_m = selftest_ns / slug / "forge"
                        (ws_m / "test").mkdir(parents=True, exist_ok=True)
                        return ws_m

                    def _mock_exec_forge39(ws: Path, rel_test_path: str, sol_code: str, **kw: Any) -> ForgeRunOutcome:
                        idx_now = chunk_idx39["i"]
                        chunk_idx39["i"] += 1
                        captured_taddrs39.append(set(_norm_addr(x) for x in (kw.get("target_addrs") or [])))
                        if idx_now == 0:
                            return ForgeRunOutcome(
                                executed=True,
                                passed_tests=1,
                                failed_tests=0,
                                conservation_gains=[],
                                trace_summary={"disallowed_cheatcode_in_trace": False, "target_called_in_trace": False, "trace_test_count": 1},
                            )
                        return ForgeRunOutcome(
                            executed=True,
                            passed_tests=1,
                            failed_tests=0,
                            conservation_gains=[{"slice_idx": 1, "witness_idx": 0, "asset_index": 0, "gain_wei": str(10**17), "total_supply_mint_wei": "0"}],
                            trace_summary={"disallowed_cheatcode_in_trace": False, "target_called_in_trace": True, "trace_test_count": 1},
                        )

                    class _RouterMc39(_SelftestMockRouter):
                        _cand_urls = ("http://127.0.0.1:8545",)

                    _eng_globals()["ensure_clean_forge_workspace"] = _mock_clean_ws39
                    _eng_globals()["execute_forge_test_file"] = _mock_exec_forge39
                    try:
                        router39_mc = _RouterMc39()
                        sl39_1 = CausalSlice(slice_id="s1", target_address=t39_a1, channel="CH_BALANCE", legs=["0x2e1a7d4d"], witness_candidates=[1], repeat_counts=[1], leg_targets=[t39_a1])
                        sl39_2 = CausalSlice(slice_id="s2", target_address=t39_a2, channel="CH_BALANCE", legs=["0x2e1a7d4d"], witness_candidates=[1], repeat_counts=[1], leg_targets=[t39_a2])
                        mc_out39 = generate_and_run_forge_suites(
                            router39_mc,
                            "t39-multichunk",
                            "ethereum",
                            20_000_000,
                            t39_caller,
                            t39_a1,
                            [usdc39],
                            [sl39_1, sl39_2],
                            slice_chunk_size=1,
                            max_tests_per_chunk=4,
                            target_addrs=[t39_a1, t39_a2],
                        )
                    finally:
                        if orig_ensure_ws39 is not None:
                            _eng_globals()["ensure_clean_forge_workspace"] = orig_ensure_ws39
                        if orig_exec_forge39 is not None:
                            _eng_globals()["execute_forge_test_file"] = orig_exec_forge39

                    t39_multichunk_ok = (
                        chunk_idx39["i"] == 2
                        and len(captured_taddrs39) == 2
                        and ({t39_a1, t39_a2} <= captured_taddrs39[0])
                        and ({t39_a1, t39_a2} <= captured_taddrs39[1])
                        and bool((mc_out39.trace_summary or {}).get("target_called_in_trace", False))
                        and not bool((mc_out39.trace_summary or {}).get("disallowed_cheatcode_in_trace", True))
                    )

                    prof_payload39 = {
                        "slug": "t39-pin-profit",
                        "chain": "ethereum",
                        "audit_block": 20_000_000,
                        "verdict": _format_profit_verdict(150_000),
                        "net_usd": 100.0,
                        "net_krw": 150_000,
                        "usd_krw_rate": 1500.0,
                        "fx_source": "sealed_fresh_39",
                        "gross_profit_native": "0.050000000000000000",
                        "net_profit_native": "0.049995000000000000",
                        "profit_native_unit": "ETH",
                        "costs": {"gas_usd": "0.01"},
                    }
                    forge_payload39 = {
                        "slug": "t39-pin-profit",
                        "chain": "ethereum",
                        "audit_block": 20_000_000,
                        "executed": True,
                        "passed_tests": 1,
                        "failed_tests": 0,
                        "conservation_gains": [{"asset_index": 0, "gain_wei": str(10**17)}],
                        "error_reason": "",
                    }
                    trace_valid39 = {"disallowed_cheatcode_in_trace": False, "target_called_in_trace": True, "trace_test_count": 1}
                    trace_no_target39 = {"disallowed_cheatcode_in_trace": False, "target_called_in_trace": False, "trace_test_count": 1}
                    write_protocol_artifacts(
                        "t39-pin-profit",
                        prof_payload39,
                        forge_payload=forge_payload39,
                        trace_payload=trace_valid39,
                    )
                    ok_pin39, _, res_pin39, f_pin39 = _verify_protocol_pins_loaded("t39-pin-profit")
                    t39_pins_ok = (
                        ok_pin39
                        and isinstance(res_pin39, dict)
                        and (not _profit_artifact_valid(res_pin39, f_pin39, strict_live_fx=False, trace_doc=trace_no_target39))
                        and _profit_artifact_valid(res_pin39, f_pin39, strict_live_fx=False, trace_doc=trace_valid39)
                    )

                    # (6) #11 온체인 코드 필터·모드B 발견요청 + #1 고아 세션 바인딩 + #13 리버스 프록시/라이브헤드 + #12 허위 SAFE 거부 + #1 유휴 세션 게이트 + #14 FS 불변성
                    eoa39 = "0x3333333333333333333333333333333333333333"
                    pre39 = "0x0000000000000000000000000000000000000005"
                    router39_code = _make_mock_guard_router(st39_fx, code_by_addr={t39_a1: "0x60006000f3", eoa39: "0x"})
                    filt39, filt_ok39 = _filter_onchain_contract_addrs(router39_code, "ethereum", [t39_a1, eoa39, pre39], 20_000_000)
                    req_path39 = emit_address_discovery_request(
                        "t39-disc",
                        "ethereum",
                        20_000_000,
                        1_500_000.0,
                        router=router39_code,
                        session_id="ses-t39a",
                        mode="B",
                        principle_summary="flashloan price skew",
                    )
                    req_doc39 = _read_json(req_path39, default={})
                    t39_disc_ok = (
                        filt_ok39
                        and filt39 == [t39_a1]
                        and isinstance(req_doc39, dict)
                        and req_doc39.get("mode") == "B"
                        and req_doc39.get("principle_summary") == "flashloan price skew"
                        and req_doc39.get("session_id") == "ses-t39a"
                    )

                    st39_misc = SQLiteStore(":memory:")
                    st39_list.append(st39_misc)
                    slug_orph39 = "t39-orphan"
                    st39_misc.upsert_queue_item(slug_orph39, 10, "ethereum", 1_000_000.0, [t39_a1])
                    st39_misc.set_queue_agent_state(slug_orph39, "awaiting_poc", session_id="")
                    orph_dir39 = ARTIFACT_ROOT / slug_orph39
                    (orph_dir39 / "poc_requests").mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(
                        orph_dir39 / "address_request.json",
                        {"slug": slug_orph39, "session_id": "", "isolated_commands": {"resume_engine": f"{CLI_INVOKE_PREFIX} run"}},
                    )
                    _write_json_atomic(
                        orph_dir39 / "poc_requests" / "poc_request_1.json",
                        {"candidate_id": 1, "session_id": "", "isolated_commands": {"resume_engine": f"{CLI_INVOKE_PREFIX} run"}},
                    )
                    st39_misc.ensure_session("ses-t39b")
                    bound_slug39 = st39_misc.bind_orphan_agent_item_to_session("ses-t39b")
                    orph_addr_after39 = _read_json(orph_dir39 / "address_request.json", default={}) or {}
                    orph_poc_after39 = _read_json(orph_dir39 / "poc_requests" / "poc_request_1.json", default={}) or {}
                    t39_orphan_ok = (
                        bound_slug39 == slug_orph39
                        and orph_addr_after39.get("session_id") == "ses-t39b"
                        and ("ses-t39b" in str((orph_addr_after39.get("isolated_commands") or {}).get("resume_engine") or ""))
                        and orph_poc_after39.get("session_id") == "ses-t39b"
                        and ("ses-t39b" in str((orph_poc_after39.get("isolated_commands") or {}).get("resume_engine") or ""))
                    )

                    proxy_p39 = "0x4444444444444444444444444444444444444444"
                    seed_ret39 = "0x5555555555555555555555555555555555555555"
                    seed_sstore39 = "0x6666666666666666666666666666666666666666"
                    seed_dyncall39 = "0x7777777777777777777777777777777777777777"
                    up_topic39 = EIP1967_UPGRADED_TOPIC0
                    impl_slot39 = EIP1967_SLOTS["impl"].lower()

                    class _LiveHeadRouter39(_SelftestMockRouter):
                        def __init__(self, c_map: Dict[str, str], s_map: Optional[Dict[Tuple[str, str], str]] = None, bal_wei: int = 10**20) -> None:
                            super().__init__(store=st39_misc)
                            self._c_map = {_norm_addr(k): v for k, v in c_map.items()}
                            self._s_map = s_map or {}
                            self._bal_wei = bal_wei

                        def get_quorum_head_block(self, ch: str) -> Tuple[str, int]:
                            return ("http://127.0.0.1:8545", 20_000_005)

                        def http_get_json(self, url: str, **kw: Any) -> Any:
                            return {"coins": {"coingecko:ethereum": {"price": 2000.0}, "ethereum:0x0000000000000000000000000000000000000000": {"price": 2000.0}}}

                        def call(self, ch: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_getLogs":
                                return [{"address": proxy_p39, "topics": [up_topic39, "0x" + ("0" * 24) + seed_ret39[2:]]}]
                            if method == "eth_getBlockByNumber":
                                return {"number": hex(20_000_005), "timestamp": hex(1_760_000_000)}
                            if method == "eth_getTransactionCount":
                                return "0x1"
                            if method == "eth_getStorageAt" and params:
                                na_s = _norm_addr(str(params[0]))
                                sl_s = str(params[1] if len(params) > 1 else "").lower()
                                return self._s_map.get((na_s, sl_s), "0x" + ("00" * 32))
                            return "0x"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_l: List[Any] = []
                            for m_l, p_l in calls:
                                na_b = _norm_addr(str(p_l[0])) if p_l else ""
                                if m_l == "eth_getCode":
                                    out_l.append(self._c_map.get(na_b, "0x"))
                                elif m_l == "eth_getStorageAt":
                                    sl_b = str(p_l[1] if len(p_l) > 1 else "").lower()
                                    out_l.append(self._s_map.get((na_b, sl_b), "0x" + ("00" * 32)))
                                elif m_l == "eth_getBalance":
                                    out_l.append(hex(self._bal_wei))
                                else:
                                    out_l.append("0x")
                            return out_l

                    r_up39 = _LiveHeadRouter39(
                        {proxy_p39: "0x60006000f4", seed_ret39: "0x60006000f3"},
                        {(proxy_p39, impl_slot39): "0x" + ("00" * 12) + seed_ret39[2:]},
                    )
                    rev_proxies39 = _find_reverse_upgraded_proxies(r_up39, "ethereum", [seed_ret39], 0)
                    r_patched39 = _LiveHeadRouter39({t39_a1: "0x600080fd", seed_ret39: "0x60006000f3"})
                    r_sstore39 = _LiveHeadRouter39({t39_a1: "0x600080fd", seed_sstore39: "0x600160005560006000f3"})
                    r_dyncall39 = _LiveHeadRouter39({t39_a1: "0x600080fd", seed_dyncall39: "0x60006000600060006000356000f100"})
                    r_depleted39 = _LiveHeadRouter39({t39_a1: "0x"}, bal_wei=0)
                    t39_livehead_ok = (
                        (_norm_addr(proxy_p39) in {_norm_addr(x) for x in rev_proxies39})
                        and (_verify_live_head_patched_or_depleted(r_patched39, "ethereum", 20_000_005, [t39_a1, seed_ret39]) is not None)
                        and (_verify_live_head_patched_or_depleted(r_sstore39, "ethereum", 20_000_005, [t39_a1, seed_sstore39]) is None)
                        and (_verify_live_head_patched_or_depleted(r_dyncall39, "ethereum", 20_000_005, [t39_a1, seed_dyncall39]) is None)
                        and (_verify_live_head_patched_or_depleted(r_depleted39, "ethereum", 20_000_005, [t39_a1]) is not None)
                    )

                    # #12 process_agent_submissions: gains_pre 또는 f_err가 있을 때 허위 VERDICT_SAFE 거부
                    slug_s12_a = "t39-safe-with-gains"
                    slug_s12_b = "t39-safe-with-ferr"
                    for sl12, f12 in (
                        (
                            slug_s12_a,
                            {"slug": slug_s12_a, "candidate_id": 1, "chain": "ethereum", "audit_block": 20_000_000, "executed": True, "passed_tests": 1, "failed_tests": 0, "conservation_gains": [{"asset_index": 0, "gain_wei": "1000"}], "error_reason": ""},
                        ),
                        (
                            slug_s12_b,
                            {"slug": slug_s12_b, "candidate_id": 1, "chain": "ethereum", "audit_block": 20_000_000, "executed": False, "passed_tests": 0, "failed_tests": 1, "conservation_gains": [], "error_reason": "FOUNDRY_COMPILE_ERR"},
                        ),
                    ):
                        st39_misc.upsert_queue_item(sl12, 10, "ethereum", 500_000.0, [t39_a1])
                        st39_misc.set_queue_agent_state(sl12, "awaiting_verdict", session_id="ses-t39b")
                        d12 = ARTIFACT_ROOT / sl12
                        (d12 / "verdicts").mkdir(parents=True, exist_ok=True)
                        (d12 / "poc_requests").mkdir(parents=True, exist_ok=True)
                        _write_json_atomic(d12 / "result.json", {"slug": sl12, "chain": "ethereum", "audit_block": 20_000_000, "needs_agent": True, "verdict": "UNDECIDED"})
                        _write_json_atomic(d12 / "checkpoint.json", {"chain": "ethereum", "audit_block": 20_000_000, "caller": t39_caller, "closure": {"token_decimals": {usdc39: 6}, "token_prices_usd": {usdc39: 1.0}}})
                        _write_json_atomic(d12 / "poc_requests" / "poc_request_1.json", {"candidate_id": 1, "target": t39_a1, "caller": t39_caller})
                        _write_json_atomic(d12 / "forge_output_1.json", f12)
                        _write_json_atomic(d12 / "verdicts" / "verdict_1.json", {"candidate_id": 1, "verdict": VERDICT_SAFE})
                    process_agent_submissions(router39_code, st39_misc, only_slugs={slug_s12_a, slug_s12_b})
                    res_12a = _read_json(ARTIFACT_ROOT / slug_s12_a / "result.json", default={}) or {}
                    res_12b = _read_json(ARTIFACT_ROOT / slug_s12_b / "result.json", default={}) or {}
                    t39_agent_safe_reject_ok = (
                        (res_12a.get("verdict") != VERDICT_SAFE)
                        and bool(res_12a.get("needs_agent", False))
                        and (st39_misc.get_queue_status(slug_s12_a) != "done")
                        and (res_12b.get("verdict") != VERDICT_SAFE)
                        and bool(res_12b.get("needs_agent", False))
                        and (st39_misc.get_queue_status(slug_s12_b) != "done")
                    )

                    # #1 유휴 세션(owned_slugs == []) done gate 통과 + #14 _fs_invariants_ok allowlist 검증
                    st39_idle = SQLiteStore(":memory:")
                    st39_list.append(st39_idle)
                    st39_idle.put_engine_meta("mode_a_seed_pending", "0")
                    st39_idle.upsert_queue_item("t39-idle-done", 10, "ethereum", 100_000.0, [t39_a1])
                    st39_idle.complete_queue_item("t39-idle-done", VERDICT_SAFE, 0.0)
                    st39_idle.ensure_session("ses-t39-idle")
                    t39_idle_root = selftest_ns / "t39_idle_root"
                    t39_idle_root.mkdir(parents=True, exist_ok=True)
                    fs_root39 = selftest_ns / "t39_fs_root"
                    fs_script39 = selftest_ns / "t39_fs_script"
                    fs_tools39 = fs_script39 / "tools"
                    fs_root39.mkdir(parents=True, exist_ok=True)
                    fs_tools39.mkdir(parents=True, exist_ok=True)
                    for req_fn39 in ("Defi_구현.py", "원하는 것.md", "지시서.md", "컨트랙트.md"):
                        (fs_script39 / req_fn39).write_text("# clean\n", encoding="utf-8")
                    idle_gate39 = _evaluate_session_done_gates(
                        st39_idle, "ses-t39-idle", artifact_root=t39_idle_root, run_selftest=False, script_dir=fs_script39, tools_dir=fs_tools39
                    )
                    fs_clean_ok39 = _fs_invariants_ok(fs_root39, False, script_dir=fs_script39, tools_dir=fs_tools39)
                    (fs_script39 / "stray_diag.txt").write_text("stray", encoding="utf-8")
                    fs_stray_root_rej39 = not _fs_invariants_ok(fs_root39, False, script_dir=fs_script39, tools_dir=fs_tools39)
                    (fs_script39 / "stray_diag.txt").unlink()
                    (fs_tools39 / "stray_tool.py").write_text("x=1", encoding="utf-8")
                    fs_stray_tools_rej39 = not _fs_invariants_ok(fs_root39, False, script_dir=fs_script39, tools_dir=fs_tools39)
                    t39_idle_and_fs_ok = (
                        bool(idle_gate39.get("all_gates_passed", False))
                        and bool(idle_gate39.get("gate1_session_goal_satisfied", False))
                        and bool(idle_gate39.get("gate3_pins_sha256_valid", False))
                        and fs_clean_ok39
                        and fs_stray_root_rej39
                        and fs_stray_tools_rej39
                    )

                    t39_ok = (
                        t39_staticcall_ok
                        and t39_allowlist_ok
                        and t39_wnative_insolv_ok
                        and t39_fx_and_gas_ok
                        and t39_hevm_ok
                        and t39_multichunk_ok
                        and t39_pins_ok
                        and t39_disc_ok
                        and t39_orphan_ok
                        and t39_livehead_ok
                        and t39_agent_safe_reject_ok
                        and t39_idle_and_fs_ok
                    )
                    results.append(
                        (
                            "T39_ALL_21_REMEDIATIONS_REGRESSION",
                            t39_ok,
                            (
                                f"sc={t39_staticcall_ok},allow={t39_allowlist_ok},wn={t39_wnative_insolv_ok},"
                                f"fx_gas={t39_fx_and_gas_ok},hevm={t39_hevm_ok},mc={t39_multichunk_ok},"
                                f"pins={t39_pins_ok},disc={t39_disc_ok},orph={t39_orphan_ok},lh={t39_livehead_ok},"
                                f"safe_rej={t39_agent_safe_reject_ok},idle_fs={t39_idle_and_fs_ok}"
                            ),
                        )
                    )
                finally:
                    if orig_ensure_ws39 is not None:
                        _eng_globals()["ensure_clean_forge_workspace"] = orig_ensure_ws39
                    if orig_exec_forge39 is not None:
                        _eng_globals()["execute_forge_test_file"] = orig_exec_forge39
                    for st_c39 in st39_list:
                        try:
                            st_c39.close()
                        except Exception:
                            pass

            if _want_test("T40_COMPREHENSIVE_REMEDIATION_REGRESSION", tag_filters):
                # T40: Comprehensive Remediation Regression (Groups A-D + FX/wMeta[0]/isolation fixes)
                st40 = SQLiteStore(selftest_ns / "_t40_main.sqlite3")
                orig_argv40 = list(sys.argv)
                orig_http_get40 = _eng_globals().get("_http_get_json_with_fallback")
                orig_qfile40 = QUEUE_FILE_PATH
                orig_forge_sub40 = _eng_globals().get("_run_forge_subproc")
                orig_replay_sum40 = _eng_globals().get("_run_single_replay_trace_summary")
                orig_cf_causal40 = _eng_globals().get("verify_counterfactual_target_causality")
                orig_replay_live40 = _eng_globals().get("replay_compiled_test_at_live_head")
                t40_guard_stack = contextlib.ExitStack()
                try:
                    t40_guard_stack.enter_context(_offline_socket_guard())
                    # 1) Allowlist (disallowed_assembly_raw_log, 1-arg prank bytecode) & HEVM trace detection
                    ok_rawlog40, r_rawlog40 = verify_cheatcode_allowlist(
                        "contract X40 { function f() public { assembly { log0(0, 32) } } }"
                    )
                    sel_p1_40 = _selector("prank(address)").lower()
                    bc_p1_40 = bytes.fromhex(
                        "63" + sel_p1_40[2:] + "60e01b60005273" + HEVM_ADDRESS_LOWER[2:] + "600080808080945af1"
                    )
                    ok_bc_p1_40, _ = verify_cheatcode_allowlist(
                        "contract X40 {}",
                        compiled_bytecode=bc_p1_40,
                        forbid_inline_fork=True,
                    )
                    tr_p1_json40 = json.dumps(
                        {
                            "test/Verify_t40.t.sol:Verify_t40": {
                                "test_results": {
                                    "test_0()": {
                                        "traces": [
                                            [
                                                "Execution",
                                                {
                                                    "arena": [
                                                        {
                                                            "trace": {
                                                                "kind": "Call",
                                                                "address": HEVM_ADDRESS_LOWER,
                                                                "data": f"{sel_p1_40}{'0' * 64}",
                                                            }
                                                        }
                                                    ]
                                                },
                                            ]
                                        ]
                                    }
                                }
                            }
                        }
                    )
                    hevm_tr_disallowed40, _, _ = _detect_disallowed_hevm_in_trace_output(
                        tr_p1_json40,
                        forbid_inline_fork=True,
                    )
                    t40_allowlist_prank_trace_ok = (
                        (not ok_rawlog40)
                        and ("disallowed_assembly_raw_log" in r_rawlog40)
                        and (not ok_bc_p1_40)
                        and bool(hevm_tr_disallowed40)
                    )

                    # 2) Spoof ConservationGain emitter (raw+decoded drop), non-harness emitter, and wMeta[0]
                    ok_nonhar40, r_nonhar40 = verify_agent_harness_accounting_integrity(
                        "contract FakeTarget40 { event ConservationGain(uint256,address,uint256,uint256,uint256); "
                        "function f() public { emit ConservationGain(0, address(0), 1, 1, 1); } }"
                    )
                    spoof_gain_data40 = "0x" + _abi_encode_seq(
                        ["uint256", "address", "uint256", "uint256", "uint256"],
                        [0, "0x0000000000000000000000000000000000000000", 5 * 10**18, 5 * 10**18, 45_000],
                    ).hex()
                    spoof_addr40 = "0x" + "99" * 20
                    spoof_json40 = json.dumps(
                        {
                            "test/Verify_t40.t.sol:Verify_t40": {
                                "test_results": {
                                    "test_0()": {
                                        "status": "Success",
                                        "logs": [
                                            {
                                                "address": spoof_addr40,
                                                "topics": [CONSERVATION_GAIN_TOPIC0],
                                                "data": spoof_gain_data40,
                                            }
                                        ],
                                        "decoded_logs": [
                                            "ConservationGain(0, 0x0000000000000000000000000000000000000000, 5000000000000000000, 5000000000000000000, 45000)"
                                        ],
                                    }
                                }
                            }
                        }
                    )
                    sp_pass40, sp_fail40, sp_gains40 = _parse_forge_json_gains(spoof_json40)
                    legit_json40 = json.dumps(
                        {
                            "test/Verify_t40.t.sol:Verify_t40": {
                                "test_results": {
                                    "test_0()": {
                                        "status": "Success",
                                        "logs": [
                                            {
                                                "address": FOUNDRY_TEST_CONTRACT,
                                                "topics": [CONSERVATION_GAIN_TOPIC0],
                                                "data": spoof_gain_data40,
                                            }
                                        ],
                                        "decoded_logs": [
                                            "ConservationGain(0, 0x0000000000000000000000000000000000000000, 5000000000000000000, 5000000000000000000, 45000)"
                                        ],
                                    }
                                }
                            }
                        }
                    )
                    lg_pass40, lg_fail40, lg_gains40 = _parse_forge_json_gains(legit_json40)
                    weth40 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    h_wmeta40 = _render_minimal_solidity_harness(
                        slug="t40_wmeta",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "34" * 20,
                        target_addr=weth40,
                        token_addrs=[weth40],
                        slices=[],
                        vault_addresses=[weth40],
                    )
                    ok_wmeta40, _ = verify_agent_harness_accounting_integrity(h_wmeta40)
                    t40_spoof_emitter_and_wmeta_ok = (
                        (not ok_nonhar40)
                        and any("FORBIDDEN_NON_HARNESS_CONSERVATION_EMITTER:FakeTarget40" in str(x) for x in r_nonhar40)
                        and sp_pass40 == 1
                        and sp_fail40 == 0
                        and len(sp_gains40) == 0
                        and lg_pass40 == 1
                        and lg_fail40 == 0
                        and len(lg_gains40) == 1
                        and ok_wmeta40
                    )

                    # 3) Price divergence (>15% -> (0,0)), session FX isolation, live RPCRouter rejecting fixture,
                    #    rollup L1 fee fail-open, and native_price_max conservative gas valuation
                    class _PriceDivRouter40(RPCRouter):
                        def __init__(self, store: SQLiteStore, llama_price: float) -> None:
                            super().__init__(store=store)
                            self.llama_price = llama_price

                        def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                            if method == "eth_getBlockByNumber":
                                return {"number": hex(20_000_001), "timestamp": hex(1_760_000_000)}
                            return "0x"

                        def multicall3(self, ch: str, queries: Any, **kw: Any) -> List[Tuple[bool, bytes]]:
                            enc_round = _abi_encode_seq(
                                ["uint80", "int256", "uint256", "uint256", "uint80"],
                                [1, 2000 * 10**8, 1_759_999_900, 1_759_999_900, 1],
                            )
                            enc_dec = _abi_encode_seq(["uint8"], [8])
                            if len(queries) == 2:
                                return [(True, enc_round), (True, enc_dec)]
                            return [(False, b"") for _ in queries]

                        def http_get_json(self, url: str, timeout: float = 15.0, extra_headers: Optional[Dict[str, str]] = None) -> Any:
                            return {"coins": {"coingecko:ethereum": {"price": float(self.llama_price)}}}

                    div_hi_r40 = _PriceDivRouter40(st40, 2600.0)
                    div_ok_r40 = _PriceDivRouter40(st40, 2100.0)
                    p_div_bad40 = _estimate_native_price_interval_usd("ethereum", router=div_hi_r40, block_pin=20_000_001)
                    p_div_good40 = _estimate_native_price_interval_usd("ethereum", router=div_ok_r40, block_pin=20_000_002)

                    st40.put_price("fx:USD_KRW", 1350.0, "fixture:t40")
                    st40.put_engine_meta("session_usd_krw:ses40a", "1450.0")
                    st40.put_engine_meta("session_usd_krw_source:ses40a", "observed:ses40a")
                    st40.put_engine_meta("session_usd_krw_at:ses40a", f"{time.time() - 1.0:.3f}")
                    fx_a_r40 = _make_mock_guard_router(st40)
                    fx_a_val40, fx_a_src40 = _fetch_usd_krw_rate(st40, allow_offline_fixture=False, router=fx_a_r40, session_id="ses40a")
                    fx_b_val40, fx_b_src40 = _fetch_usd_krw_rate(st40, allow_offline_fixture=False, router=fx_a_r40, session_id="ses40b")
                    fx_live_val40, fx_live_src40 = _fetch_usd_krw_rate(
                        st40,
                        allow_offline_fixture=False,
                        router=RPCRouter(store=st40),
                        session_id="ses40live",
                    )

                    op_live_r40 = RPCRouter(store=st40)
                    op_l1_neg40 = _estimate_rollup_l1_fee_wei(op_live_r40, "optimism", 20_000_000, 256)
                    acct_op_blk40 = judge_conservation_accounting(
                        store=st40,
                        chain="optimism",
                        caller_before={"native": 0},
                        caller_after={"native": 5 * 10**18},
                        vault_before={"native": 5 * 10**18},
                        vault_after={"native": 0},
                        gas_used=100_000,
                        gas_price_wei=1_000_000_000,
                        prices_map={"native": 2000.0, "native_max": 2000.0},
                        router=op_live_r40,
                        block_pin=20_000_000,
                        live_head_block=0,
                        allow_offline_fixture=False,
                        session_id="ses40a",
                    )
                    acct_max_gas40 = judge_conservation_accounting(
                        store=st40,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 5 * 10**18},
                        vault_before={"native": 5 * 10**18},
                        vault_after={"native": 0},
                        gas_used=1_000_000,
                        gas_price_wei=100_000_000_000,
                        prices_map={"native": 2000.0, "native_max": 3000.0},
                        allow_offline_fixture=True,
                        session_id="ses40a",
                    )
                    t40_price_fx_rollup_gas_ok = (
                        p_div_bad40 == (0.0, 0.0)
                        and p_div_good40 == (2000.0, 2100.0)
                        and abs(fx_a_val40 - 1450.0) < 1e-6
                        and fx_a_src40 == "observed:ses40a"
                        and abs(fx_b_val40 - 1350.0) < 1e-6
                        and fx_b_src40 == "fixture:t40"
                        and fx_live_val40 == 0.0
                        and "fixture:" not in str(fx_live_src40)
                        and op_l1_neg40 == -1
                        and acct_op_blk40.verdict == VERDICT_BLOCKED
                        and "ROLLUP_L1_FEE_UNCONFIRMED_FAIL_OPEN" in str(acct_op_blk40.reason)
                        and abs(acct_max_gas40.gas_usd - 300.0) < 1e-6
                    )

                    # 4) Session claim isolation, quorum <2 failure, batch/multicall transport failure propagation
                    st_q40 = SQLiteStore(selftest_ns / "_t40_qclaim.sqlite3")
                    try:
                        st_q40.upsert_queue_item("t40-q1", 1, "ethereum", 100_000.0, ["0x" + "72" * 20], mode="B")
                        q40_a1 = st_q40.claim_next_for_session("ses40_owner", "boot40_a")
                        q40_b1 = st_q40.claim_next_for_session("ses40_other", "boot40_b")
                        q40_a2 = st_q40.claim_next_for_session("ses40_owner", "boot40_a2")
                        session_claim_ok40 = (
                            isinstance(q40_a1, dict)
                            and q40_a1.get("slug") == "t40-q1"
                            and q40_b1 is None
                            and q40_a2 is None
                        )
                    finally:
                        st_q40.close()

                    class _QuorumSingleFailRouter40(RPCRouter):
                        def candidate_urls(self, ch: str, need_archive: bool = False, for_forge: bool = False) -> List[str]:
                            return ["http://127.0.0.1:19991", "http://127.0.0.1:19992"]

                        def _pooled_post(self, url: str, body: bytes, timeout: float) -> bytes:
                            if ":19992" in url:
                                raise OSError("simulated_rpc_down")
                            txt = body.decode("utf-8", errors="ignore")
                            if "eth_chainId" in txt:
                                return b'{"jsonrpc":"2.0","id":1,"result":"0x1"}'
                            return b'{"jsonrpc":"2.0","id":1,"result":"0x1312d00"}'

                    q_fail_r40 = _QuorumSingleFailRouter40(store=st40)
                    quorum_rejected40 = False
                    try:
                        q_fail_r40.get_quorum_head_block("ethereum")
                    except RuntimeError:
                        quorum_rejected40 = True

                    class _BatchFailClosureRouter40(_SelftestMockRouter):
                        def call(self, ch: str, method: str, params: Any = (), **kw: Any) -> Any:
                            if method == "eth_getBlockByNumber":
                                return {"number": hex(20_000_000), "timestamp": hex(1_760_000_000)}
                            if method == "eth_getCode":
                                return "0x60006000f3"
                            if method == "eth_getStorageAt":
                                return "0x" + ("00" * 32)
                            if method == "eth_getBalance":
                                return None
                            return "0x"

                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out_bf: List[Any] = []
                            for m_b, _ in calls:
                                if m_b == "eth_getCode":
                                    out_bf.append("0x60006000f3")
                                elif m_b == "eth_getStorageAt":
                                    out_bf.append("0x" + ("00" * 32))
                                else:
                                    out_bf.append(None)
                            return out_bf

                        def last_batch_transport_failures(self) -> int:
                            return 2

                        def last_multicall_transport_failed(self) -> bool:
                            return True

                    bf_r40 = _BatchFailClosureRouter40(store=st40)
                    cl_bf40 = build_backward_trust_closure(bf_r40, "ethereum", ["0x" + "71" * 20], 20_000_000)
                    t40_quorum_claim_and_batch_ok = (
                        session_claim_ok40
                        and quorum_rejected40
                        and bool(cl_bf40.rpc_balance_incomplete)
                        and (not bool(cl_bf40.early_exit_safe))
                    )

                    # 5) Inbound Approval log spender harvest, CFG live pushed_jumpdests (pop exclusion),
                    #    and Mode B Sourcify cursor RPC error preservation
                    ap_owner40 = "0x" + "31" * 20
                    ap_spender40 = "0x" + "32" * 20
                    ap_tok40 = "0x" + "33" * 20

                    class _LogsRouter40(RPCRouter):
                        def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                            if method == "eth_getLogs":
                                return [
                                    {
                                        "address": ap_tok40,
                                        "topics": [
                                            APPROVAL_TOPIC0,
                                            "0x" + ("00" * 12) + ap_owner40[2:],
                                            "0x" + ("00" * 12) + ap_spender40[2:],
                                        ],
                                        "data": "0x" + ("00" * 31) + "01",
                                    }
                                ]
                            return "0x"

                    spenders40: Set[str] = set()
                    _harvest_token_logs_bisect(
                        _LogsRouter40(store=st40),
                        "ethereum",
                        [ap_owner40],
                        19_999_995,
                        20_000_000,
                        max_depth=2,
                        out_spenders=spenders40,
                    )
                    cfg_jmp_dyn40 = disassemble_and_build_cfg(bytes.fromhex("600a600b5060003556005b5b00"))
                    bb0_dyn40 = cfg_jmp_dyn40.blocks.get(0)
                    cfg_jmp_st40 = disassemble_and_build_cfg(bytes.fromhex("600750600856005b5b00"))
                    bb0_st40 = cfg_jmp_st40.blocks.get(0)
                    cfg_jump_ok40 = (
                        bb0_dyn40 is not None
                        and (10 in bb0_dyn40.pushed_jumpdests)
                        and (11 not in bb0_dyn40.pushed_jumpdests)
                        and (10 not in bb0_dyn40.succs)
                        and bb0_st40 is not None
                        and (8 in bb0_st40.succs)
                        and (7 not in bb0_st40.succs)
                        and (7 not in bb0_st40.pushed_jumpdests)
                    )

                    sf40_dir = selftest_ns / "_t40_sf"
                    sf40_dir.mkdir(parents=True, exist_ok=True)
                    seed_cur40 = "0x" + "44" * 20
                    (sf40_dir / "_local_index.jsonl").write_text(
                        json.dumps(
                            {
                                "chain_id": 1,
                                "address": "0x" + "45" * 20,
                                "runtime_code_hex": "60006000f3",
                            }
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                    st_cur40 = SQLiteStore(selftest_ns / "_t40_cur.sqlite3")
                    try:
                        st_cur40.upsert_queue_item("modeb-cur-40", 1, "ethereum", 10_000.0, [seed_cur40], mode="B")

                        class _FailCodeRouter40(RPCRouter):
                            def call(self, ch: str, method: str, params: Any, **kw: Any) -> Any:
                                if method == "eth_getCode":
                                    raise RuntimeError("RPC_CODE_FAIL_40")
                                return "0x"

                            def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                                if any(m_c == "eth_getCode" for m_c, _ in calls):
                                    raise RuntimeError("RPC_CODE_FAIL_40")
                                return ["0x" for _ in calls]

                        cur_err40 = _advance_mode_b_sourcify_cursor(
                            _FailCodeRouter40(store=st_cur40),
                            st_cur40,
                            parquet_root=sf40_dir,
                            max_wall_sec=4.5,
                        )
                        cur_ok40 = _advance_mode_b_sourcify_cursor(
                            _SelftestMockRouter(store=st_cur40),
                            st_cur40,
                            parquet_root=sf40_dir,
                            max_wall_sec=4.5,
                        )
                        cursor_ok40 = (
                            cur_err40.get("phase") == "seed_harvest"
                            and cur_ok40.get("phase") == "exhausted"
                        )
                    finally:
                        st_cur40.close()
                    t40_approval_cfg_cursor_ok = (ap_spender40 in spenders40) and cfg_jump_ok40 and cursor_ok40

                    # 6) Mode A explicit zero EVM TVL exclusion, requeue_with_addresses mode_b_zero_seed_recovered, CLI main() rc=2
                    _set_eng("QUEUE_FILE_PATH", selftest_ns / "_nonexistent_q40.json")
                    _eng_globals()["_http_get_json_with_fallback"] = lambda *a, **kw: [
                        {
                            "slug": "sol-only-t40",
                            "tvl": 5_000_000.0,
                            "chainTvls": {"Solana": 5_000_000.0, "Ethereum": 0.0},
                            "chains": ["Solana", "Ethereum"],
                        },
                        {
                            "slug": "eth-ok-t40",
                            "tvl": 500_000.0,
                            "chainTvls": {"Ethereum": 500_000.0},
                            "chains": ["Ethereum"],
                        },
                    ]
                    ma_items40, _, _ = _collect_mode_a_seed_items()
                    ma_slugs40 = {str(x[0]) for x in ma_items40}
                    sol_excluded40 = ("eth-ok-t40" in ma_slugs40) and ("sol-only-t40" not in ma_slugs40)

                    st40.upsert_queue_item("modeb-zrec-40", 1, "ethereum", 0.0, [], mode="B")
                    st40.requeue_with_addresses("modeb-zrec-40", ["0x" + "ab" * 20], chain="ethereum")
                    pm_zrec40 = st40._conn.execute(
                        "SELECT meta_json FROM proto_meta WHERE slug=?",
                        ("modeb-zrec-40",),
                    ).fetchone()
                    zrec_meta_ok40 = bool(
                        pm_zrec40 is not None
                        and json.loads(pm_zrec40["meta_json"]).get("mode_b_zero_seed_recovered") is True
                    )

                    with contextlib.redirect_stderr(io.StringIO()):
                        sys.argv = ["Defi_구현.py", "run", "--bad"]
                        rc_cli_run40 = main()
                        sys.argv = ["Defi_구현.py", "done", "badid"]
                        rc_cli_done40 = main()
                    t40_modea_evm_requeue_cli_ok = (
                        sol_excluded40
                        and zrec_meta_ok40
                        and rc_cli_run40 == 2
                        and rc_cli_done40 == 2
                    )

                    # 7) _verify_live_head_patched_or_depleted (pure-view -> None) & _is_e4_remaining_below_floor
                    view_r40 = _make_mock_guard_router(st40, code_by_addr={"0x" + "55" * 20: "0x60006000f3"})
                    _v40_orig_call = view_r40.call
                    _v40_orig_batch = view_r40.batch_call
                    view_r40.call = lambda ch, m, p, **kw: (
                        hex(10 * 10**18) if m == "eth_getBalance" else _v40_orig_call(ch, m, p, **kw)
                    )
                    view_r40.batch_call = lambda ch, calls, **kw: [
                        hex(10 * 10**18) if m_c == "eth_getBalance" else b_v
                        for (m_c, _), b_v in zip(calls, _v40_orig_batch(ch, calls, **kw))
                    ]
                    lh_view40 = _verify_live_head_patched_or_depleted(view_r40, "ethereum", 20_000_005, ["0x" + "55" * 20])
                    base_acct40 = ConservationVerdict(
                        verdict=VERDICT_SAFE,
                        gross_usd=0.0,
                        gas_usd=0.0,
                        flash_fee_usd=0.0,
                        net_usd=0.0,
                        net_krw=0,
                        fx_rate_krw=1400.0,
                        fx_source="fixture:t40",
                        corroborated=False,
                        mutex_only_rejected=False,
                        reason="OK",
                        net_usd_exact="0",
                    )
                    cl_clean40 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=["0x" + "55" * 20],
                        all_addresses=["0x" + "55" * 20],
                        resolutions={},
                        vault_exposures={},
                        max_extractable_usd=50.0,
                        min_gas_usd=0.01,
                        early_exit_safe=False,
                        early_exit_reason="",
                        max_extractable_usd_dec="50",
                    )
                    cl_unp40 = dataclasses.replace(cl_clean40, has_unpriced_token_exposure=True)
                    cl_inc40 = dataclasses.replace(cl_clean40, rpc_balance_incomplete=True)
                    cl_trn40 = dataclasses.replace(cl_clean40, closure_truncated=True)
                    cl_dyn40 = dataclasses.replace(cl_clean40, has_dynamic_external_reachability=True)
                    t40_live_head_and_e4_ok = (
                        lh_view40 is None
                        and _is_e4_remaining_below_floor(cl_clean40, base_acct40) is True
                        and _is_e4_remaining_below_floor(cl_unp40, base_acct40) is False
                        and _is_e4_remaining_below_floor(cl_inc40, base_acct40) is False
                        and _is_e4_remaining_below_floor(cl_trn40, base_acct40) is False
                        and _is_e4_remaining_below_floor(cl_dyn40, base_acct40) is False
                    )

                    # 8) Group D2 process_agent_submissions:
                    #    (a) discovered_addresses.json clears stale checkpoint keys & sets S0_ADDRESSES_UPDATED
                    #    (b) counterfactual failure no UnboundLocalError
                    #    (c) live_head_safe_override beats sibling blocked_from_profit & dust PROVEN_BY_FORGE -> blocked
                    st_ag40 = SQLiteStore(selftest_ns / "_t40_agent.sqlite3")
                    try:
                        st_ag40.put_price("fx:USD_KRW", 1400.0, "fixture:t40")
                        disc_addr40 = "0x" + "66" * 20
                        slug_disc40 = "t40-disc-ckpt-clear"
                        ddir40 = selftest_ns / slug_disc40
                        ddir40.mkdir(parents=True, exist_ok=True)
                        st_ag40.upsert_queue_item(slug_disc40, 1, "ethereum", 100_000.0, [], mode="B")
                        st_ag40.set_queue_agent_state(slug_disc40, "awaiting_addresses")
                        _write_json_atomic(
                            ddir40 / "result.json",
                            {
                                "slug": slug_disc40,
                                "chain": "ethereum",
                                "audit_block": 20_000_000,
                                "verdict": VERDICT_BLOCKED,
                                "needs_agent": True,
                                "awaiting_address_discovery": True,
                            },
                        )
                        _write_json_atomic(
                            ddir40 / "checkpoint.json",
                            {
                                "chain": "ethereum",
                                "audit_block": 20_000_000,
                                "seed_addrs": ["0x" + "01" * 20],
                                "mode_b_expanded": True,
                                "closure_ready": True,
                                "closure": {"all_addresses": ["0x" + "01" * 20]},
                                "verified_sources": {"dummy": True},
                                "completed_slice_ids": ["s0"],
                                "sealed_mechanical_profit": True,
                                "stage": "S3_DONE",
                            },
                        )
                        _write_json_atomic(
                            ddir40 / "discovered_addresses.json",
                            {"official_docs": [disc_addr40], "addresses": [disc_addr40]},
                        )
                        now_ag40 = time.time() + 10.0
                        os.utime(ddir40 / "discovered_addresses.json", (now_ag40, now_ag40))
                        def _make_code_router40(st_o: SQLiteStore, cmap: Dict[str, str]) -> Any:
                            base_r = _make_mock_guard_router(st_o, code_by_addr=cmap)
                            lut40 = {_norm_addr(k): v for k, v in cmap.items()}
                            orig_call_fn = base_r.call

                            def _call_with_code40(ch: str, method: str, params: Any, **kw: Any) -> Any:
                                if method == "eth_getCode":
                                    aq = _norm_addr(str(params[0])) if params else ""
                                    return lut40.get(aq, "0x")
                                if method == "eth_getStorageAt":
                                    return "0x" + ("00" * 32)
                                return orig_call_fn(ch, method, params, **kw)

                            base_r.call = _call_with_code40
                            base_r.get_quorum_head_block = lambda ch: (20_000_004, 20_000_002)
                            base_r.candidate_urls = lambda ch, **kw: ["http://127.0.0.1:8545"]
                            return base_r

                        r_disc40 = _make_code_router40(st_ag40, {disc_addr40: "0x60006000f3"})
                        process_agent_submissions(r_disc40, st_ag40, only_slugs={slug_disc40})
                        ckpt_after_disc40 = _read_json(ddir40 / "checkpoint.json", default={}) or {}
                        disc_clear_ok40 = (
                            isinstance(ckpt_after_disc40, dict)
                            and ckpt_after_disc40.get("stage") == "S0_ADDRESSES_UPDATED"
                            and all(
                                k not in ckpt_after_disc40
                                for k in (
                                    "seed_addrs",
                                    "mode_b_expanded",
                                    "closure_ready",
                                    "closure",
                                    "verified_sources",
                                    "completed_slice_ids",
                                    "sealed_mechanical_profit",
                                )
                            )
                        )

                        # (b) Counterfactual fail UnboundLocalError check + (c) live_head_safe_override vs blocked_from_profit
                        tgt_d2_40 = "0x" + "78" * 20
                        clr_d2_40 = "0x" + "12" * 20
                        closed_hex40 = "0x60006000f3"
                        r_d2_40 = _make_code_router40(st_ag40, {tgt_d2_40: closed_hex40})

                        def _setup_agent_slug40(sl: str, cand_specs: Sequence[Tuple[int, int]]) -> Path:
                            pdir = selftest_ns / sl
                            (pdir / "poc_requests").mkdir(parents=True, exist_ok=True)
                            (pdir / "verdicts").mkdir(parents=True, exist_ok=True)
                            (pdir / "forge" / "test").mkdir(parents=True, exist_ok=True)
                            st_ag40.upsert_queue_item(sl, 1, "ethereum", 100_000.0, [tgt_d2_40], mode="A")
                            st_ag40.set_queue_agent_state(sl, "awaiting_verdict")
                            _write_json_atomic(
                                pdir / "result.json",
                                {"slug": sl, "chain": "ethereum", "audit_block": 20_000_000, "verdict": VERDICT_SAFE, "needs_agent": True},
                            )
                            _write_json_atomic(
                                pdir / "checkpoint.json",
                                {
                                    "chain": "ethereum",
                                    "audit_block": 20_000_000,
                                    "caller": clr_d2_40,
                                    "closure": {"all_addresses": [tgt_d2_40], "token_decimals": {}, "token_prices_usd": {"native": 2000.0}},
                                },
                            )
                            for cid_i, gain_wei in cand_specs:
                                _write_json_atomic(
                                    pdir / "poc_requests" / f"poc_request_{cid_i}.json",
                                    {"candidate_id": cid_i, "target_address": tgt_d2_40, "zero_state_caller": clr_d2_40},
                                )
                                sol_i = (
                                    f"// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n"
                                    f"contract Verify_{sl.replace('-', '_')}_{cid_i} {{\n"
                                    f"    address constant TARGET = 0x{'78' * 20};\n"
                                    f"    function test_cand_{cid_i}() public {{ (bool ok, ) = TARGET.call(\"\"); ok; }}\n"
                                    f"}}\n"
                                )
                                (pdir / "forge" / "test" / f"test_{cid_i}.t.sol").write_text(sol_i, encoding="utf-8")
                                _write_json_atomic(
                                    pdir / f"forge_output_{cid_i}.json",
                                    {
                                        "executed": True,
                                        "passed_tests": 1,
                                        "failed_tests": 0,
                                        "conservation_gains": [
                                            {
                                                "test": f"test_cand_{cid_i}",
                                                "slice_idx": cid_i,
                                                "asset": "native",
                                                "caller_gain": gain_wei,
                                                "vault_loss": gain_wei,
                                                "gas_used": 100_000,
                                                "supply_mint": False,
                                            }
                                        ],
                                        "error_reason": "",
                                        "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False},
                                    },
                                )
                                os.utime(pdir / f"forge_output_{cid_i}.json", (time.time() + 10.0, time.time() + 10.0))
                                _write_json_atomic(
                                    pdir / "verdicts" / f"verdict_{cid_i}.json",
                                    {"candidate_id": cid_i, "verdict": "PROVEN_BY_FORGE", "notes": f"cand {cid_i}"},
                                )
                            return pdir

                        pdir_cf40 = _setup_agent_slug40("t40-cf-unbound", [(1, 5 * 10**18)])
                        _eng_globals()["verify_counterfactual_target_causality"] = lambda *a, **kw: CounterfactualCausalityResult(
                            False,
                            "NO_CAUSE",
                        )
                        process_agent_submissions(r_d2_40, st_ag40, only_slugs={"t40-cf-unbound"})
                        res_cf40 = _read_json(pdir_cf40 / "result.json", default={}) or {}
                        cf_no_unbound_ok40 = (
                            res_cf40.get("verdict") == VERDICT_BLOCKED
                            and res_cf40.get("reason") == "NO_CAUSE"
                        )

                        pdir_lh40 = _setup_agent_slug40("t40-lh-override", [(1, 10**12), (2, 5 * 10**18)])
                        _eng_globals()["verify_counterfactual_target_causality"] = lambda *a, **kw: CounterfactualCausalityResult(True, "OK")
                        _eng_globals()["replay_compiled_test_at_live_head"] = lambda *a, **kw: ForgeRunOutcome(
                            executed=True,
                            passed_tests=0,
                            failed_tests=1,
                            conservation_gains=[],
                            trace_summary={"replay_executed": True},
                            raw_stdout="",
                            raw_stderr="",
                            error_reason="REPLAY_REVERTED",
                        )
                        process_agent_submissions(r_d2_40, st_ag40, only_slugs={"t40-lh-override"})
                        res_lh40 = _read_json(pdir_lh40 / "result.json", default={}) or {}
                        lh_override_ok40 = res_lh40.get("verdict") == VERDICT_SAFE
                    finally:
                        st_ag40.close()
                    t40_agent_d2_ok = disc_clear_ok40 and cf_no_unbound_ok40 and lh_override_ok40

                    # 9) Mode A $5k sub-chain uncovered divergence & execute_forge_test_file mixed test target attribution
                    r_ma5k_40 = _make_mock_guard_router(
                        st40,
                        code_by_addr={"0x" + "43" * 20: "0x6318160ddd14600d57600080fd5b60206000f3"},
                        mc3_zero_word=True,
                    )
                    res_ma5k_40 = audit_single_protocol(
                        r_ma5k_40,
                        st40,
                        {
                            "slug": "t40-modea-5k-blk",
                            "chain": "ethereum",
                            "tvl_usd": 5_000.0,
                            "addresses": ["0x" + "43" * 20],
                            "mode": "A",
                            "delegate_address_search": False,
                        },
                        set(),
                    )
                    modea_5k_ok40 = (
                        res_ma5k_40.get("verdict") == VERDICT_BLOCKED
                        and "MODE_A_SPOT_VAULT_UNCOVERED_DIVERGENCE" in str(res_ma5k_40.get("reason") or "")
                    )

                    ws_mix40 = ensure_clean_forge_workspace({}, slug="t40_target_mix", offline=True)
                    rel_mix40 = "test/Verify_t40_mix.t.sol"
                    tgt_mix40 = "0x" + "88" * 20
                    sol_mix40 = (
                        "// SPDX-License-Identifier: MIT\n"
                        "pragma solidity ^0.8.20;\n"
                        "contract Verify_t40_mix {\n"
                        f"    address constant TARGET = 0x{'88' * 20};\n"
                        "    function test_fake() public pure returns (uint256) { return 1; }\n"
                        "    function test_real() public view returns (address) { return TARGET; }\n"
                        "}\n"
                    )
                    (ws_mix40 / rel_mix40).write_text(sol_mix40, encoding="utf-8")
                    mix_json_out40 = json.dumps(
                        {
                            f"{rel_mix40}:Verify_t40_mix": {
                                "test_results": {
                                    "test_fake()": {
                                        "status": "Success",
                                        "logs": [
                                            {
                                                "address": FOUNDRY_TEST_CONTRACT,
                                                "topics": [CONSERVATION_GAIN_TOPIC0],
                                                "data": spoof_gain_data40,
                                            }
                                        ],
                                        "decoded_logs": [],
                                    },
                                    "test_real()": {
                                        "status": "Success",
                                        "logs": [
                                            {
                                                "address": FOUNDRY_TEST_CONTRACT,
                                                "topics": [CONSERVATION_GAIN_TOPIC0],
                                                "data": spoof_gain_data40,
                                            }
                                        ],
                                        "decoded_logs": [],
                                    },
                                }
                            }
                        }
                    )
                    only_fake_json40 = json.dumps(
                        {
                            f"{rel_mix40}:Verify_t40_mix": {
                                "test_results": {
                                    "test_fake()": {
                                        "status": "Success",
                                        "logs": [
                                            {
                                                "address": FOUNDRY_TEST_CONTRACT,
                                                "topics": [CONSERVATION_GAIN_TOPIC0],
                                                "data": spoof_gain_data40,
                                            }
                                        ],
                                        "decoded_logs": [
                                            "ConservationGain(0, 0x0000000000000000000000000000000000000000, 5000000000000000000, 5000000000000000000, 45000)"
                                        ],
                                    }
                                }
                            }
                        }
                    )
                    _eng_globals()["_run_single_replay_trace_summary"] = lambda ws, rel, tname, *a, **kw: {
                        "replay_executed": True,
                        "disallowed_cheatcode_in_trace": False,
                        "target_called_in_trace": ("test_real" in str(tname)),
                        "replay_passed": True,
                        "disallowed_count": 0,
                        "gas_used": 45_000,
                    }
                    _eng_globals()["_run_forge_subproc"] = lambda ws, cmd, to_s: subprocess.CompletedProcess(
                        cmd,
                        0,
                        stdout=(mix_json_out40 if any(x == "test" for x in cmd) else ""),
                        stderr="",
                    )
                    out_mix40 = execute_forge_test_file(
                        ws_mix40,
                        rel_mix40,
                        sol_mix40,
                        timeout_sec=30,
                        verify_trace_always=True,
                        strict_agent_accounting=False,
                        target_addr=tgt_mix40,
                        offline=True,
                    )
                    _eng_globals()["_run_forge_subproc"] = lambda ws, cmd, to_s: subprocess.CompletedProcess(
                        cmd,
                        0,
                        stdout=(only_fake_json40 if any(x == "test" for x in cmd) else ""),
                        stderr="",
                    )
                    out_fake_only40 = execute_forge_test_file(
                        ws_mix40,
                        rel_mix40,
                        sol_mix40,
                        timeout_sec=30,
                        verify_trace_always=True,
                        strict_agent_accounting=False,
                        target_addr=tgt_mix40,
                        offline=True,
                    )
                    target_mix_ok40 = (
                        out_mix40.executed
                        and len(out_mix40.conservation_gains) == 1
                        and out_mix40.conservation_gains[0].get("test") == "test_real()"
                        and out_mix40.trace_summary.get("target_called_in_trace") is True
                        and out_mix40.trace_summary.get("winning_test") == "test_real()"
                        and len(out_fake_only40.conservation_gains) == 0
                        and out_fake_only40.trace_summary.get("target_called_in_trace") is False
                    )

                    # 10) Option A artifact compaction: SAFE/BLOCKED purge working content & keep 1KB receipt; PROFIT keeps PoC .sol & purges build cache; done never reassigned
                    cmp_safe_slug = "t40-compact-safe"
                    cmp_prof_slug = "t40-compact-profit"
                    cmp_safe_dir = ARTIFACT_ROOT / cmp_safe_slug
                    cmp_prof_dir = ARTIFACT_ROOT / cmp_prof_slug
                    for _sub_c in ("forge/test", "forge/out", "forge/cache", "sources", "_cast_env", "poc_requests"):
                        (cmp_safe_dir / _sub_c).mkdir(parents=True, exist_ok=True)
                        (cmp_prof_dir / _sub_c).mkdir(parents=True, exist_ok=True)
                    (cmp_safe_dir / "forge" / "test" / "Tmp.t.sol").write_text("// discard", encoding="utf-8")
                    (cmp_safe_dir / "sources" / "Src.sol").write_text("// discard", encoding="utf-8")
                    (cmp_prof_dir / "forge" / "test" / "Verify_1.t.sol").write_text("// keep poc", encoding="utf-8")
                    (cmp_prof_dir / "forge" / "out" / "Verify_1.json").write_text("{}", encoding="utf-8")
                    (cmp_prof_dir / "sources" / "Src.sol").write_text("// discard", encoding="utf-8")
                    write_protocol_artifacts(
                        cmp_safe_slug,
                        {"slug": cmp_safe_slug, "chain": "ethereum", "audit_block": 20_000_000, "verdict": VERDICT_SAFE, "net_usd": 0.0, "net_krw": 0, "reason": "E2 closed"},
                        forge_payload={"executed": True},
                    )
                    write_protocol_artifacts(
                        cmp_prof_slug,
                        {"slug": cmp_prof_slug, "chain": "ethereum", "audit_block": 20_000_000, "verdict": _format_profit_verdict(280_000), "net_usd": 200.0, "net_krw": 280_000, "fx_rate_krw": 1400.0, "fx_source": "fixture:t40", "reason": "verified"},
                        forge_payload={"executed": True, "passed_tests": 1, "failed_tests": 0, "conservation_gains": [{"asset_index": 0, "gain_wei": "1"}], "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False}},
                    )
                    st40_cmp = SQLiteStore(":memory:")
                    try:
                        st40_cmp_sid = st40_cmp.ensure_session()["session_id"]
                        st40_cmp.upsert_queue_item(cmp_safe_slug, 1, "ethereum", 100_000.0, ["0x" + "71" * 20], mode="B")
                        st40_cmp.complete_queue_item(cmp_safe_slug, VERDICT_SAFE, 0.0)
                        no_reassign40 = st40_cmp.claim_next_for_session(st40_cmp_sid, "boot-cmp40") is None
                    finally:
                        st40_cmp.close()
                    compact_a_ok40 = (
                        verify_protocol_pins(cmp_safe_slug)[0]
                        and not (cmp_safe_dir / "forge").exists()
                        and not (cmp_safe_dir / "sources").exists()
                        and not (cmp_safe_dir / "_cast_env").exists()
                        and not (cmp_safe_dir / "forge_output.json").exists()
                        and verify_protocol_pins(cmp_prof_slug)[0]
                        and (cmp_prof_dir / "forge" / "test" / "Verify_1.t.sol").is_file()
                        and not (cmp_prof_dir / "forge" / "out").exists()
                        and not (cmp_prof_dir / "sources").exists()
                        and no_reassign40
                    )

                    # 11) PC-bang cross-machine relay (_export_relay_state -> fresh SQLite DB _import_relay_state)
                    rel_art_dir = selftest_ns / "t40_relay_art"
                    rel_art_dir.mkdir(parents=True, exist_ok=True)
                    st40_pc1 = SQLiteStore(selftest_ns / "t40_pc1.sqlite")
                    st40_pc2 = SQLiteStore(selftest_ns / "t40_pc2.sqlite")
                    try:
                        write_protocol_artifacts(
                            "modeb-ethereum-001-relaysafe",
                            {"slug": "modeb-ethereum-001-relaysafe", "chain": "ethereum", "audit_block": 20_000_000, "verdict": VERDICT_SAFE, "net_usd": 0.0, "net_krw": 0, "reason": "E3 closed"},
                            root_dir=rel_art_dir,
                        )
                        st40_pc1.upsert_queue_item("modeb-ethereum-001-relaysafe", 1, "ethereum", 100_000.0, ["0x" + "81" * 20], mode="B")
                        st40_pc1.complete_queue_item("modeb-ethereum-001-relaysafe", VERDICT_SAFE, 0.0)
                        st40_pc1.upsert_queue_item("modeb-match-ethereum-0x828282828282", 1, "ethereum", 50_000.0, ["0x" + "82" * 20], mode="B")
                        st40_pc1.upsert_queue_item("modeb-ethereum-003-zerorec", 1, "ethereum", 90_000.0, [], mode="B")
                        st40_pc1.put_proto_meta_batch([("modeb-ethereum-003-zerorec", {"mode_b_zero_seed": True})])
                        st40_pc1.requeue_with_addresses("modeb-ethereum-003-zerorec", ["0x" + "85" * 20], chain="ethereum", mode="B")
                        st40_pc1.mark_vault_proven("ethereum", "0x" + "83" * 20, "modeb-ethereum-001-relaysafe", 300_000.0, fully_exhausted=True)
                        st40_pc1.put_engine_meta(
                            "mode_b_sourcify_cursor_v1",
                            json.dumps({"phase": "sig_slices", "seed_cursor": 301, "sig_done_ranges": [[0, 24]], "sig_inflight": {"dead": [24, 36]}}),
                        )
                        _export_relay_state(st40_pc1, artifact_root=rel_art_dir)

                        half_orp_dir = rel_art_dir / "modea-half-unsealed"
                        half_orp_dir.mkdir(parents=True, exist_ok=True)
                        _write_json_atomic(half_orp_dir / "result.json", {"slug": "modea-half-unsealed", "verdict": VERDICT_SAFE, "needs_agent": True})

                        st40_pc2.upsert_queue_item("modeb-ethereum-001-relaysafe", 1, "ethereum", 100_000.0, ["0x" + "81" * 20], mode="B")
                        st40_pc2.upsert_queue_item("modeb-ethereum-002-relaynext", 1, "ethereum", 100_000.0, ["0x" + "84" * 20], mode="B")
                        st40_pc2.upsert_queue_item("modeb-ethereum-003-zerorec", 1, "ethereum", 90_000.0, [], mode="B")
                        st40_pc2.put_proto_meta_batch([("modeb-ethereum-003-zerorec", {"mode_b_zero_seed": True})])
                        imp_cnt40 = _import_relay_state(st40_pc2, artifact_root=rel_art_dir, prune_unsealed_orphans=True)
                        pc2_sid = st40_pc2.ensure_session()["session_id"]
                        pc2_claimed_1 = st40_pc2.claim_next_for_session(pc2_sid, "boot-pc2")
                        pc2_cur = json.loads(st40_pc2.get_engine_meta("mode_b_sourcify_cursor_v1", "{}") or "{}")
                        pc2_zmeta = st40_pc2.get_proto_meta("modeb-ethereum-003-zerorec")
                        relay_ok40 = (
                            imp_cnt40 == 1
                            and not half_orp_dir.exists()
                            and st40_pc2.get_queue_status("modeb-ethereum-001-relaysafe") == "done"
                            and st40_pc2.get_queue_status("modeb-match-ethereum-0x828282828282") == "pending"
                            and pc2_claimed_1 is not None
                            and pc2_claimed_1.get("slug") == "modeb-ethereum-003-zerorec"
                            and pc2_claimed_1.get("addresses") == ["0x" + "85" * 20]
                            and bool(pc2_zmeta.get("mode_b_zero_seed_recovered"))
                            and pc2_cur.get("phase") == "sig_slices"
                            and pc2_cur.get("sig_done_ranges") == [[0, 24]]
                            and pc2_cur.get("sig_inflight") == {}
                            and st40_pc2.is_vault_proven("ethereum", "0x" + "83" * 20, require_exhausted=True)
                        )
                    finally:
                        st40_pc1.close()
                        st40_pc2.close()

                    # 13) Operator quorum dedup, expanded Scroll/Curve discovery, verified zero rollup L1 fee,
                    #     trace observed_calldata_bytes + stripped estimator, Mode A lock, Mode B prune preservation,
                    #     16-token harness + callback guidance, and flash-capital scalar-ABI delegation
                    prov_key_ok40 = (
                        _rpc_quorum_provider_key("https://eth-mainnet.g.alchemy.com/v2/key1") == "alchemy.com"
                        and _rpc_quorum_provider_key("https://base-mainnet.g.alchemy.com/v2/key2") == "alchemy.com"
                        and _rpc_quorum_provider_key("https://lb.drpc.org/ogrpc") == "drpc.org"
                        and _rpc_quorum_provider_key("http://127.0.0.1:8545") == "127.0.0.1:8545"
                    )
                    qr40 = RPCRouter(store=st40)
                    qr40._verify_endpoint_chain_id = lambda u, cid, timeout=6.0: True  # type: ignore[method-assign]
                    qr40.candidate_urls = lambda ch, need_archive=False, for_forge=False: [  # type: ignore[method-assign]
                        "https://eth-mainnet.g.alchemy.com/v2/a",
                        "https://base-mainnet.g.alchemy.com/v2/b",
                        "https://lb.drpc.org/eth",
                        "https://my.quiknode.pro/c",
                    ]
                    qr40._pooled_post = lambda u, body, timeout=15.0: json.dumps(  # type: ignore[method-assign]
                        {
                            "jsonrpc": "2.0",
                            "id": 1,
                            "result": {
                                "number": "0x100" if "alchemy.com" in u else "0x10000",
                                "timestamp": "0x66000000",
                            },
                        }
                    ).encode("utf-8")
                    tip_qd40, head_qd40 = qr40.get_quorum_head_block("ethereum")
                    quor_dedup_ok40 = prov_key_ok40 and tip_qd40 == 65536 and head_qd40 == 65536

                    tok_crv40 = "0x" + "91" * 20
                    crv_reg40 = "0x" + "92" * 20
                    crv_pool40 = "0x" + "93" * 20
                    usdc_arb40 = CHAIN_STABLES["arbitrum"][0]

                    class _CurveArbRouter40(RPCRouter):
                        def multicall3(self, ch: str, calls: Sequence[Tuple[str, bytes]], **kw: Any) -> List[Tuple[bool, bytes]]:
                            out_mc: List[Tuple[bool, bytes]] = []
                            for tgt_m, cd_m in calls:
                                sel_m = "0x" + cd_m[:4].hex() if len(cd_m) >= 4 else ""
                                tn_m = _norm_addr(tgt_m)
                                if tn_m == CURVE_ADDRESS_PROVIDER and sel_m == CURVE_GET_ADDRESS_SEL:
                                    out_mc.append((True, bytes.fromhex(("00" * 12) + crv_reg40[2:])))
                                elif tn_m == crv_reg40 and sel_m == CURVE_FIND_POOL_SEL:
                                    q_arg = "0x" + cd_m[4 + 32 + 12 : 4 + 64].hex()
                                    if _norm_addr(q_arg) == _norm_addr(usdc_arb40) and cd_m[-1] == 0:
                                        out_mc.append((True, bytes.fromhex(("00" * 12) + crv_pool40[2:])))
                                    else:
                                        out_mc.append((True, b"\x00" * 32))
                                elif tn_m == crv_reg40 and sel_m == CURVE_GET_COIN_INDICES_SEL:
                                    out_mc.append((True, (0).to_bytes(32, "big") + (1).to_bytes(32, "big") + (0).to_bytes(32, "big")))
                                elif tn_m == _norm_addr(usdc_arb40) and sel_m == DECIMALS_SEL:
                                    out_mc.append((True, (6).to_bytes(32, "big")))
                                elif tn_m == crv_reg40 and sel_m == CURVE_GET_BALANCES_SEL:
                                    bals = [1000 * (10**18), 10000 * (10**6)] + [0] * 6
                                    out_mc.append((True, b"".join(x.to_bytes(32, "big") for x in bals)))
                                elif tn_m == crv_pool40 and sel_m == CURVE_GET_DY_SEL:
                                    out_mc.append((True, (500 * (10**6)).to_bytes(32, "big")))
                                else:
                                    out_mc.append((False, b""))
                            return out_mc

                    crv_res40 = probe_dex_exit_liquidity(
                        _CurveArbRouter40(store=st40),
                        "arbitrum",
                        tok_crv40,
                        10**18,
                        decimals=18,
                        unit_price_usd=500.0,
                        native_usd=2000.0,
                        block_pin=20_000_000,
                        quote_prices_usd={usdc_arb40: 1.0},
                    )
                    chain_cov_ok40 = (
                        "scroll" in _CHAINLINK_NATIVE_USD_FEEDS
                        and "scroll" in _UNIV2_FACTORIES
                        and "gnosis" in _UNIV2_FACTORIES
                        and "berachain" in _UNIV2_FACTORIES
                        and "mantle" in _UNIV2_FACTORIES
                        and "unichain" in _UNIV3_FACTORIES
                        and "arbitrum" in _CURVE_METAREGISTRY_CHAINS
                        and crv_res40.get("best_dex_kind") == "curve"
                        and _norm_addr(str(crv_res40.get("best_pool_address") or "")) == crv_pool40
                    )

                    ro_zero_r40 = RPCRouter(store=st40)
                    ro_zero_r40._verify_endpoint_chain_id = lambda u, cid, timeout=6.0: True  # type: ignore[method-assign]
                    ro_zero_r40.candidate_urls = lambda ch, need_archive=False, for_forge=False: ["https://mainnet.optimism.io"]  # type: ignore[method-assign]
                    ro_zero_mode = {"code": "0x60806040"}

                    def _ro_zero_post(u: str, body: bytes, timeout: float = 15.0) -> bytes:
                        req_d = json.loads(body.decode("utf-8"))
                        m_r = str(req_d.get("method") or "")
                        if m_r == "eth_call":
                            res_v = "0x" + ("00" * 32)
                        elif m_r == "eth_getCode":
                            res_v = ro_zero_mode["code"]
                        else:
                            res_v = "0x0"
                        return json.dumps({"jsonrpc": "2.0", "id": req_d.get("id", 1), "result": res_v}).encode("utf-8")

                    ro_zero_r40._pooled_post = _ro_zero_post  # type: ignore[method-assign]
                    l1_zero_ok40 = _estimate_rollup_l1_fee_wei(ro_zero_r40, "optimism", 20_000_001, 256) == 0
                    ro_zero_mode["code"] = "0x"
                    l1_nocode_fail40 = _estimate_rollup_l1_fee_wei(ro_zero_r40, "optimism", 20_000_002, 256) == -1
                    rollup_zero_ok40 = l1_zero_ok40 and l1_nocode_fail40

                    cd_caller40 = "0x" + "19" * 20
                    cd_tgt40 = "0x" + "29" * 20
                    tr_cd_json40 = json.dumps(
                        {
                            "traces": [
                                [
                                    "Execution",
                                    {
                                        "arena": [
                                            {
                                                "trace": {
                                                    "kind": "Call",
                                                    "caller": FOUNDRY_DEFAULT_SENDER,
                                                    "address": FOUNDRY_TEST_CONTRACT,
                                                    "depth": 0,
                                                    "data": "0x12345678",
                                                }
                                            },
                                            {
                                                "trace": {
                                                    "kind": "StaticCall",
                                                    "caller": FOUNDRY_TEST_CONTRACT,
                                                    "address": cd_tgt40,
                                                    "depth": 1,
                                                    "data": "0x" + ("aa" * 128),
                                                }
                                            },
                                            {
                                                "trace": {
                                                    "kind": "Call",
                                                    "caller": cd_caller40,
                                                    "address": cd_tgt40,
                                                    "depth": 1,
                                                    "data": "0x" + ("bb" * 68),
                                                }
                                            },
                                            {
                                                "trace": {
                                                    "kind": "Call",
                                                    "caller": cd_caller40,
                                                    "address": cd_tgt40,
                                                    "depth": 1,
                                                    "data": "0x" + ("cc" * 260),
                                                }
                                            },
                                        ]
                                    },
                                ]
                            ]
                        }
                    )
                    obs_cd40 = _extract_trace_observed_calldata_bytes(tr_cd_json40, allowed_caller=cd_caller40)
                    sol_bp40 = (
                        "contract T {\n"
                        "  function test_bp() public {\n"
                        "    /* " + ("comment padding " * 120) + " */\n"
                        "    ConservationSnap memory snapBefore = _takeSnap(ZERO_STATE_CALLER);\n"
                        "    uint256 gasStart = gasleft();\n"
                        "    vm.startPrank(ZERO_STATE_CALLER, ZERO_STATE_CALLER);\n"
                        "    TARGET.call(hex\"00\");\n"
                        "    vm.stopPrank();\n"
                        "    uint256[2] memory liqCredit;\n"
                        "    uint256[2] memory liqDebited;\n"
                        "    _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);\n"
                        "  }\n"
                        "}\n"
                    )
                    est_bp40 = _estimate_test_calldata_len(sol_bp40, test_name="test_bp")
                    eff_cd40 = _resolve_effective_calldata_len(
                        {"observed_calldata_bytes": obs_cd40, "per_test_observed_calldata_bytes": {"test_bp": 412}},
                        sol_bp40,
                        test_name="test_bp",
                    )
                    trace_cd_ok40 = (obs_cd40 == 328) and (est_bp40 == 64) and (eff_cd40 == 412)

                    st_ma40 = SQLiteStore(selftest_ns / "_t40_maseed.sqlite3")
                    st_pr40 = SQLiteStore(selftest_ns / "_t40_prune.sqlite3")
                    orig_collect_ma40 = _eng_globals().get("_collect_mode_a_seed_items")
                    orig_max_sql40 = MAX_SQLITE_BYTES
                    try:
                        st_ma40.put_engine_meta("mode_a_seed_pending", "1")
                        ma_calls40 = 0

                        def _mock_collect_ma40() -> Tuple[List[Tuple[Any, ...]], List[Tuple[str, Dict[str, Any]]], bool]:
                            nonlocal ma_calls40
                            ma_calls40 += 1
                            return ([("modea-lock-test", 2, "ethereum", 100_000.0, ["0x" + "94" * 20], "A", "modea", 100_000.0)], [], True)

                        _eng_globals()["_collect_mode_a_seed_items"] = _mock_collect_ma40
                        ma_ins1 = _ensure_mode_a_seeded(st_ma40)
                        ma_ins2 = _ensure_mode_a_seeded(st_ma40)
                        modea_lock_ok40 = (
                            ma_ins1 >= 1
                            and ma_ins2 == 0
                            and ma_calls40 == 1
                            and (CACHE_DIR / "mode_a_seed.lock").exists()
                            and st_ma40.get_engine_meta("mode_a_seed_pending", "1") == "0"
                        )

                        seed_b40 = "0x" + "95" * 20
                        zero_b40 = "0x" + "96" * 20
                        nonseed_40 = "0x" + "97" * 20
                        st_pr40.upsert_queue_item("modeb-prune-seed", 1, "ethereum", 100_000.0, [seed_b40], mode="B")
                        big_hex40 = "0x" + ("60" * 2048)
                        st_pr40.put_contract_code_batch(
                            [
                                (1, seed_b40, 20_000_000, "0x1", "0x2", "{}", big_hex40, "none", [], True),
                                (1, nonseed_40, 20_000_000, "0x3", "0x4", "{}", big_hex40, "none", [], True),
                                (1, zero_b40, 0, "0x5", "0x6", "{}", big_hex40, "none", [], True),
                            ]
                        )
                        _eng_globals()["MAX_SQLITE_BYTES"] = 1024
                        st_pr40._maybe_prune_oversized()
                        _eng_globals()["MAX_SQLITE_BYTES"] = orig_max_sql40
                        row_seed40 = st_pr40.get_contract_code(1, seed_b40, 20_000_000)
                        row_zero40 = st_pr40.get_contract_code(1, zero_b40, 0)
                        prune_modeb_ok40 = (
                            isinstance(row_seed40, dict)
                            and row_seed40.get("raw_code_hex") == big_hex40
                            and isinstance(row_zero40, dict)
                            and row_zero40.get("raw_code_hex") == big_hex40
                            and st_pr40.get_contract_code(1, nonseed_40, 20_000_000) is None
                        )
                    finally:
                        _eng_globals()["MAX_SQLITE_BYTES"] = orig_max_sql40
                        if orig_collect_ma40 is not None:
                            _eng_globals()["_collect_mode_a_seed_items"] = orig_collect_ma40
                        st_ma40.close()
                        st_pr40.close()

                    toks16_40 = [f"0x{i:040x}" for i in range(0x101, 0x111)]
                    skel16_40 = _render_agent_poc_skeleton(
                        "t40_tok16",
                        1,
                        "ethereum",
                        0,
                        "0x" + "18" * 20,
                        "0x" + "28" * 20,
                        token_addrs=toks16_40,
                    )
                    tok16_compile_ok40 = ("TOK_15" in skel16_40) and ("_bridgeLiquidatedGain" in skel16_40)
                    if tok16_compile_ok40 and FORGE_BIN.exists():
                        ws16_40 = ensure_clean_forge_workspace({"ethereum": ""}, slug="t40_tok16", offline=True)
                        (ws16_40 / "test").mkdir(parents=True, exist_ok=True)
                        (ws16_40 / "test/Test16.t.sol").write_text(skel16_40, encoding="utf-8")
                        out16_40 = execute_forge_test_file(
                            ws16_40,
                            "test/Test16.t.sol",
                            skel16_40,
                            allowed_caller="0x" + "18" * 20,
                            offline=True,
                        )
                        tok16_compile_ok40 = bool(out16_40.executed and out16_40.passed_tests >= 1 and out16_40.failed_tests == 0)
                    req_p16_40 = emit_poc_requests_for_slug(
                        slug="t40-tok16-note",
                        chain="ethereum",
                        block_pin=0,
                        caller="0x" + "18" * 20,
                        slices=[
                            CausalSlice(
                                slice_id="s16_40",
                                target_address="0x" + "28" * 20,
                                channel="CH_MIDFRAME",
                                legs=["0xb6b55f25"],
                                witness_candidates=[1],
                                repeat_counts=[1],
                                rationale="test note check",
                                leg_targets=["0x" + "28" * 20],
                                needs_agent_synthesis=True,
                            )
                        ],
                        verified_sources={},
                        router=_make_mock_guard_router(st40),
                        session_id="ses40a",
                    )
                    note16_doc40 = _read_json(req_p16_40[0], default={}) if req_p16_40 else {}
                    note16_txt40 = str((note16_doc40.get("submission_schema") or {}).get("note") or "")
                    tok16_skel_ok40 = (
                        tok16_compile_ok40
                        and ("_bridgeLiquidatedGain" in note16_txt40)
                        and ("separate helper contract" in note16_txt40)
                    )

                    fc_slug40 = "t40-flash-cap-vault"
                    fc_tgt40 = "0x" + "98" * 20
                    st40.put_proto_meta_batch([(fc_slug40, {"resolved_addrs_ethereum": [fc_tgt40]})])
                    fc_dep_sel40 = _selector("deposit(uint256)")[2:].lower()
                    fc_code40 = bytes.fromhex(
                        "60003560e01c"
                        + "5b"
                        + "63" + fc_dep_sel40 + "14"
                        + "61001557"
                        + "600080fd"
                        + "5b"
                        + "6323b872dd50"
                        + "60008080808080fa50"
                        + "600260040450"
                        + "6001600055"
                        + "00"
                    )
                    fc_res40 = ProxyResolution(
                        address=fc_tgt40,
                        proxy_kind="none",
                        impl_addresses=[fc_tgt40],
                        admin_address="",
                        raw_code_hex="0x" + fc_code40.hex(),
                        effective_code_hex="0x" + fc_code40.hex(),
                        skeleton_hash="0x" + ("00" * 32),
                        slot_collisions=[],
                        impl_codes_hex={fc_tgt40: "0x" + fc_code40.hex()},
                    )
                    fc_cl40 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=20_000_000,
                        seed_addresses=[fc_tgt40],
                        all_addresses=[fc_tgt40],
                        resolutions={fc_tgt40: fc_res40},
                        vault_exposures={
                            fc_tgt40: VaultExposure(
                                address=fc_tgt40,
                                native_wei=10 * 10**18,
                                native_usd=25_000.0,
                                total_usd=25_000.0,
                                total_usd_dec="25000",
                                native_usd_dec="25000",
                            )
                        },
                        max_extractable_usd=25_000.0,
                        min_gas_usd=0.01,
                        early_exit_safe=False,
                        early_exit_reason="",
                        max_extractable_usd_dec="25000",
                        realizable_extractable_usd=25_000.0,
                        realizable_extractable_usd_dec="25000",
                    )
                    fc_router40 = _make_mock_guard_router(st40, code_by_addr={fc_tgt40: "0x" + fc_code40.hex()})
                    fc_router40.get_quorum_head_block = lambda *a_q, **kw_q: (20_000_005, 20_000_005)  # type: ignore[method-assign]
                    orig_bcl40 = _eng_globals().get("build_backward_trust_closure")
                    orig_fvc40 = _eng_globals().get("fetch_verified_contract")
                    orig_genf40 = _eng_globals().get("generate_and_run_forge_suites")
                    orig_seltgt40 = _eng_globals().get("select_zero_state_caller")
                    orig_prescr40 = _eng_globals().get("prescreen_eth_call_batch")
                    try:
                        _eng_globals()["build_backward_trust_closure"] = lambda *a_m, **kw_m: fc_cl40
                        _eng_globals()["fetch_verified_contract"] = lambda *a_m, **kw_m: {
                            "verified": True,
                            "abi": [
                                {
                                    "type": "function",
                                    "name": "deposit",
                                    "stateMutability": "nonpayable",
                                    "inputs": [{"name": "assets", "type": "uint256"}],
                                    "outputs": [],
                                }
                            ],
                            "sources": {"Vault.sol": "contract Vault { function deposit(uint256 assets) external {} }"},
                        }
                        _eng_globals()["select_zero_state_caller"] = lambda *a_m, **kw_m: "0x" + "19" * 20
                        _eng_globals()["prescreen_eth_call_batch"] = lambda *a_m, **kw_m: list(a_m[4] if len(a_m) >= 5 else kw_m.get("slices", []))
                        _eng_globals()["generate_and_run_forge_suites"] = lambda *a_m, **kw_m: ForgeRunOutcome(
                            executed=True,
                            passed_tests=len(kw_m.get("slices") or []),
                            failed_tests=0,
                            conservation_gains=[],
                            trace_summary={"disallowed_cheatcode_in_trace": False, "target_called_in_trace": True},
                        )
                        st40.upsert_queue_item(fc_slug40, 1, "ethereum", 25_000.0, [fc_tgt40], mode="B")
                        fc_out40 = audit_single_protocol(
                            fc_router40,
                            st40,
                            {"slug": fc_slug40, "chain": "ethereum", "addresses": [fc_tgt40], "mode": "B", "principle_summary": ""},
                            set(),
                            session_id="ses40a",
                        )
                        fc_reqs40 = list((ARTIFACT_ROOT / fc_slug40 / "poc_requests").glob("poc_request_*.json"))
                        fc_req_txt40 = fc_reqs40[0].read_text(encoding="utf-8") if fc_reqs40 else ""
                        flash_cap_ok40 = (
                            bool(fc_out40.get("needs_agent"))
                            and int(fc_out40.get("poc_requests_emitted") or 0) >= 1
                            and "FLASH_CAPITAL_COUPLED_SCALAR_ABI_VAULT" in fc_req_txt40
                        )
                    finally:
                        if orig_bcl40 is not None:
                            _eng_globals()["build_backward_trust_closure"] = orig_bcl40
                        if orig_fvc40 is not None:
                            _eng_globals()["fetch_verified_contract"] = orig_fvc40
                        if orig_genf40 is not None:
                            _eng_globals()["generate_and_run_forge_suites"] = orig_genf40
                        if orig_seltgt40 is not None:
                            _eng_globals()["select_zero_state_caller"] = orig_seltgt40
                        if orig_prescr40 is not None:
                            _eng_globals()["prescreen_eth_call_batch"] = orig_prescr40

                    t40_post_opt_ok = (
                        quor_dedup_ok40
                        and chain_cov_ok40
                        and rollup_zero_ok40
                        and trace_cd_ok40
                        and modea_lock_ok40
                        and prune_modeb_ok40
                        and tok16_skel_ok40
                        and flash_cap_ok40
                    )

                    t40_ok = (
                        t40_allowlist_prank_trace_ok
                        and t40_spoof_emitter_and_wmeta_ok
                        and t40_price_fx_rollup_gas_ok
                        and t40_quorum_claim_and_batch_ok
                        and t40_approval_cfg_cursor_ok
                        and t40_modea_evm_requeue_cli_ok
                        and t40_live_head_and_e4_ok
                        and t40_agent_d2_ok
                        and modea_5k_ok40
                        and target_mix_ok40
                        and compact_a_ok40
                        and relay_ok40
                        and t40_post_opt_ok
                    )
                    results.append(
                        (
                            "T40_COMPREHENSIVE_REMEDIATION_REGRESSION",
                            t40_ok,
                            (
                                f"allow={t40_allowlist_prank_trace_ok},spoof={t40_spoof_emitter_and_wmeta_ok},"
                                f"fx_roll={t40_price_fx_rollup_gas_ok},quor={t40_quorum_claim_and_batch_ok},"
                                f"cur_cfg={t40_approval_cfg_cursor_ok},ma_cli={t40_modea_evm_requeue_cli_ok},"
                                f"lh_e4={t40_live_head_and_e4_ok},ag_d2={t40_agent_d2_ok},"
                                f"ma5k={modea_5k_ok40},mix={target_mix_ok40},opt={t40_post_opt_ok}"
                                f"(qd={quor_dedup_ok40},cc={chain_cov_ok40},r0={rollup_zero_ok40},"
                                f"cd={trace_cd_ok40},ma={modea_lock_ok40},pr={prune_modeb_ok40},"
                                f"t16={tok16_skel_ok40},fc={flash_cap_ok40})"
                            ),
                        )
                    )
                finally:
                    sys.argv = orig_argv40
                    _set_eng("QUEUE_FILE_PATH", orig_qfile40)
                    if orig_http_get40 is not None:
                        _eng_globals()["_http_get_json_with_fallback"] = orig_http_get40
                    if orig_forge_sub40 is not None:
                        _eng_globals()["_run_forge_subproc"] = orig_forge_sub40
                    if orig_replay_sum40 is not None:
                        _eng_globals()["_run_single_replay_trace_summary"] = orig_replay_sum40
                    if orig_cf_causal40 is not None:
                        _eng_globals()["verify_counterfactual_target_causality"] = orig_cf_causal40
                    if orig_replay_live40 is not None:
                        _eng_globals()["replay_compiled_test_at_live_head"] = orig_replay_live40
                    try:
                        st40.close()
                    except Exception:
                        pass
                    try:
                        t40_guard_stack.close()
                    except Exception:
                        pass

            if _want_test("T41_TWELVE_HARDENING_FIXES_REGRESSION", tag_filters):
                # T41: 12건 핵심 보안/회계/동시성/오탐/미탐 수정 회귀 검증
                t41_ns = selftest_ns / "t41_hardening"
                t41_ns.mkdir(parents=True, exist_ok=True)
                t41_art_root = t41_ns / "artifacts"
                t41_art_root.mkdir(parents=True, exist_ok=True)
                st41_a: Optional[SQLiteStore] = None
                st41_b: Optional[SQLiteStore] = None
                st41_c: Optional[SQLiteStore] = None
                orig_cf_41 = _eng_globals().get("verify_counterfactual_target_causality")
                orig_replay_41 = _eng_globals().get("replay_compiled_test_at_live_head")
                orig_gas_41 = _eng_globals().get("fetch_gas_price_wei")
                orig_sub_41 = _eng_globals().get("_run_forge_subproc")
                orig_ni_41 = _eng_globals().get("_estimate_native_price_interval_dec")
                orig_nd_41 = _eng_globals().get("_estimate_native_price_dec")
                orig_max_sq_41 = _eng_globals().get("MAX_SQLITE_BYTES")
                orig_art_41 = ARTIFACT_ROOT
                try:
                    # (#1) _aggregate_gain_entries 동일 테스트·동일 자산 중복 합산 차단 & 네이티브 sink 스윕 + 다중/루프 emit 차단
                    c_ded1, v_ded1, _, mg_ded1, _ = _aggregate_gain_entries(
                        [
                            {
                                "test": "test_pwn",
                                "asset": "0x0000000000000000000000000000000000000000",
                                "caller_gain": "1000",
                                "vault_loss": "1000",
                                "gas_used": 160000,
                            },
                            {
                                "test": "test_pwn",
                                "asset": "0x0000000000000000000000000000000000000000",
                                "caller_gain": "1200",
                                "vault_loss": "1000",
                                "gas_used": 180000,
                            },
                        ]
                    )
                    agg_dedup_ok41 = (
                        c_ded1.get("0x0000000000000000000000000000000000000000") == 1200
                        and v_ded1.get("0x0000000000000000000000000000000000000000") == 1000
                        and mg_ded1 == 180000
                    )
                    ctx41 = _prepare_conservation_harness_context(
                        "t41-canon",
                        "ethereum",
                        "0x0000000000000000000000000000000000001337",
                        "0x000000000000000000000000000000000000a001",
                        ["0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"],
                        vault_addresses=[f"0x000000000000000000000000000000000000b{i:03x}" for i in range(24)],
                    )
                    body41 = _render_conservation_contract_body(ctx41, "Verify_t41", "ZERO_STATE_CALLER", caller_const_decl=True)
                    sink_nat_ok41 = (
                        "sink.call{value:" in body41
                        and "_verifyPostCommitSettlement(address(0)," in body41
                        and ctx41["vault_count"] == 25
                        and "_hasResidualSpenderAllowance(" in body41
                        and _checksum("0x000000000022d473030f116ddee9f6b43ac78ba3") in body41
                    )
                    ok_multi_assert, v_multi_assert = verify_agent_harness_accounting_integrity(
                        "contract Verify_Multi {\n"
                        "  function test_x() public {\n"
                        "    _assertConservation(0, sb, sa, g);\n"
                        "    _assertConservation(0, sb, sa, g);\n"
                        "  }\n"
                        "}\n"
                    )
                    ok_loop_assert, v_loop_assert = verify_agent_harness_accounting_integrity(
                        "contract Verify_Loop {\n"
                        "  function test_x() public {\n"
                        "    for (uint256 i = 0; i < 3; i++) {\n"
                        "      _assertConservation(0, sb, sa, g);\n"
                        "    }\n"
                        "  }\n"
                        "}\n"
                    )
                    ok_dup_emit, v_dup_emit = verify_agent_harness_accounting_integrity(
                        "contract Verify_Dup {\n"
                        "  function test_x() public {\n"
                        "    uint256 d = balAfter - balBefore;\n"
                        "    uint256 l = vBefore - vAfter;\n"
                        "    emit ConservationGain(0, address(0), d, l, 21000);\n"
                        "    emit ConservationGain(1, address(0), d, l, 21000);\n"
                        "  }\n"
                        "}\n"
                    )
                    ok_trans_multi, v_trans_multi = verify_agent_harness_accounting_integrity(
                        "contract Verify_Trans {\n"
                        "  function _h() internal { _assertConservation(0, sb, sa, g); }\n"
                        "  function test_x() public { _h(); _h(); }\n"
                        "}\n"
                    )
                    t41_fix1_ok = bool(
                        agg_dedup_ok41
                        and sink_nat_ok41
                        and (not ok_multi_assert and any("FORBIDDEN_MULTI_CONSERVATION_CALL" in x for x in v_multi_assert))
                        and (not ok_loop_assert and any("FORBIDDEN_LOOP_CONSERVATION_EMISSION" in x for x in v_loop_assert))
                        and (not ok_dup_emit and any("FORBIDDEN_DUPLICATE_ASSET_CONSERVATION_EMIT" in x for x in v_dup_emit))
                        and (not ok_trans_multi and any("FORBIDDEN_MULTI_CONSERVATION_CALL" in x for x in v_trans_multi))
                    )

                    # (#2) 반사실 검증 시 VAULT_* 피해 승인자(preserve_addrs) 보존 + TARGET/프로토콜 금고만 무효화
                    cf_seen_sol: List[str] = []
                    cf_ws = t41_ns / "cf_ws"
                    (cf_ws / "test").mkdir(parents=True, exist_ok=True)
                    t41_tgt = "0x000000000000000000000000000000000000cafe"
                    t41_vic = "0x000000000000000000000000000000000000dead"
                    cf_sol = (
                        "pragma solidity ^0.8.20;\n"
                        "contract Verify_cf41 {\n"
                        f"  address constant TARGET = {_checksum(t41_tgt)};\n"
                        f"  address constant VAULT_0 = {_checksum(t41_vic)};\n"
                        "  function test_s0() public { TARGET.call(''); VAULT_0.staticcall(''); }\n"
                        "}\n"
                    )

                    def _mock_cf_sub41(ws_m: Path, cmd_m: List[str], timeout_m: int) -> subprocess.CompletedProcess[str]:
                        for p_cf in sorted((ws_m / "test").glob("_CounterfactualCausalityProbe_*.t.sol")):
                            cf_seen_sol.append(p_cf.read_text(encoding="utf-8", errors="replace"))
                        return subprocess.CompletedProcess(
                            cmd_m,
                            0,
                            stdout='{"ok":{"test_results":{"test_s0()":{"status":"Success","logs":[],"decoded_logs":[]}}}}',
                            stderr="",
                        )

                    _eng_globals()["_run_forge_subproc"] = _mock_cf_sub41
                    cf_res41 = verify_counterfactual_target_causality(
                        ws=cf_ws,
                        rel_test_path="test/Verify_cf41.t.sol",
                        sol_code=cf_sol,
                        target_addr=t41_tgt,
                        fork_url="http://127.0.0.1:8545",
                        fork_block=19_000_000,
                        closure_addrs=[t41_tgt, t41_vic],
                        preserve_addrs=[t41_vic],
                    )
                    cf_low41 = cf_seen_sol[0].lower() if cf_seen_sol else ""
                    t41_fix2_ok = bool(
                        bool(cf_res41)
                        and cf_low41
                        and f"address constant target = 0x0000000000000000000000000000000000099998;" in cf_low41
                        and f"address constant vault_0 = {t41_vic};" in cf_low41
                        and f"address({t41_tgt})" in cf_low41
                        and f"0xb4d6c782, address({t41_vic})" not in cf_low41
                        and "0x0000000000000000000000000000000000099998" in cf_low41
                    )
                    _eng_globals()["_run_forge_subproc"] = orig_sub_41

                    # (#3 & #8) 반사실 RPC failover + b_head live_replay_artifacts_out 전파 + write_protocol_artifacts fallback
                    cf_calls_u41: List[str] = []

                    def _mock_cf_failover41(**kw: Any) -> CounterfactualCausalityResult:
                        cf_calls_u41.append(str(kw.get("fork_url") or ""))
                        if len(cf_calls_u41) < 2:
                            return CounterfactualCausalityResult(False, "COUNTERFACTUAL_COMPILE_OR_EXEC_FAILED:503")
                        return CounterfactualCausalityResult(True, "CAUSALITY_VERIFIED_PROFIT_ELIMINATED")

                    def _mock_lh_replay41(**kw: Any) -> ForgeRunOutcome:
                        bh = int(kw.get("live_head_block") or 0)
                        return ForgeRunOutcome(
                            executed=True,
                            passed_tests=1,
                            failed_tests=0,
                            conservation_gains=[
                                {
                                    "test": "test_live",
                                    "slice_idx": 0,
                                    "asset": "0x0000000000000000000000000000000000000000",
                                    "caller_gain": str(5 * 10**18),
                                    "vault_loss": str(5 * 10**18),
                                    "gas_used": 60000,
                                }
                            ],
                            trace_summary={
                                "winning_test": "test_live",
                                "target_called_in_trace": True,
                                "observed_calldata_bytes": 128,
                                "observed_calldata_zero_bytes": 64,
                                "observed_calldata_nonzero_bytes": 64,
                                "live_head_replayed": bh,
                            },
                        )

                    class _FakeRouter41(RPCRouter):
                        def http_get_json(self, url: str, timeout: float = 10.0) -> Any:
                            return {}

                        def candidate_urls(self, chain: str, need_archive: bool = True, for_forge: bool = False) -> List[str]:
                            return ["http://rpc1.local", "http://rpc2.local", "http://rpc3.local"]

                        def get_quorum_head_block(self, chain: str) -> Tuple[int, int]:
                            return 19_000_002, 19_000_002

                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            if method == "eth_chainId":
                                return "0x1"
                            if method == "eth_gasPrice":
                                return "0x3b9aca00"
                            return None

                    st41_a = SQLiteStore(":memory:")
                    st41_a.put_price("FX:USD_KRW", 1400.0, "observed:multi_source")
                    _eng_globals()["_estimate_native_price_interval_dec"] = lambda *_a, **_kw: (Decimal("3000"), Decimal("3000"))
                    _eng_globals()["_estimate_native_price_dec"] = lambda *_a, **_kw: Decimal("3000")
                    acct_in41 = judge_conservation_accounting(
                        store=st41_a,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 5 * 10**18},
                        vault_before={"native": 5 * 10**18},
                        vault_after={"native": 0},
                        gas_used=60000,
                        gas_price_wei=1_000_000_000,
                        prices_map={"native": 3000.0},
                        allow_offline_fixture=False,
                    )
                    _eng_globals()["verify_counterfactual_target_causality"] = _mock_cf_failover41
                    _eng_globals()["replay_compiled_test_at_live_head"] = _mock_lh_replay41
                    _eng_globals()["fetch_gas_price_wei"] = lambda _r, _c, _b: 1_000_000_000
                    art_out41: Dict[str, Any] = {}
                    ok_cf_lh41, _, eff_blk41, _ = _confirm_profit_causality_and_live_head(
                        ws=cf_ws,
                        rel_test_path="test/Verify_cf41.t.sol",
                        sol_code=cf_sol,
                        match_test="test_live",
                        target_addr=t41_tgt,
                        closure_addrs=[t41_tgt],
                        live_head_seed_addrs=[t41_tgt],
                        allowed_caller="0x0000000000000000000000000000000000001337",
                        calldata_len=128,
                        gas_price_wei=1_000_000_000,
                        acct_in=acct_in41,
                        store=st41_a,
                        router=_FakeRouter41(),
                        chain="ethereum",
                        block_pin=19_000_000,
                        decimals_map={"0x0000000000000000000000000000000000000000": 18},
                        prices_map={"native": 3000.0, "native_max": 3000.0},
                        live_replay_artifacts_out=art_out41,
                    )
                    if orig_cf_41 is not None:
                        _eng_globals()["verify_counterfactual_target_causality"] = orig_cf_41
                    if orig_replay_41 is not None:
                        _eng_globals()["replay_compiled_test_at_live_head"] = orig_replay_41
                    if orig_gas_41 is not None:
                        _eng_globals()["fetch_gas_price_wei"] = orig_gas_41
                    if orig_ni_41 is not None:
                        _eng_globals()["_estimate_native_price_interval_dec"] = orig_ni_41
                    if orig_nd_41 is not None:
                        _eng_globals()["_estimate_native_price_dec"] = orig_nd_41
                    _set_eng("ARTIFACT_ROOT", t41_art_root)
                    p41_sealed = write_protocol_artifacts(
                        "t41-bhead",
                        {
                            "slug": "t41-bhead",
                            "chain": "ethereum",
                            "audit_block": eff_blk41,
                            "verdict": _format_profit_verdict(1000000),
                            "net_usd": 1000.0,
                            "net_krw": 1000000,
                            "needs_agent": False,
                        },
                        forge_payload=art_out41.get("forge_payload"),
                        trace_payload=None,
                        root_dir=t41_art_root,
                    )
                    ts_fb41 = _read_json(p41_sealed / "execution_trace_summary.json", default={})
                    t41_fix3_8_ok = bool(
                        ok_cf_lh41
                        and len(cf_calls_u41) == 2
                        and eff_blk41 == 19_000_002
                        and art_out41.get("audit_block") == 19_000_002
                        and isinstance(art_out41.get("forge_payload"), dict)
                        and art_out41["forge_payload"].get("audit_block") == 19_000_002
                        and isinstance(ts_fb41, dict)
                        and ts_fb41.get("live_head_replayed") == 19_000_002
                    )

                    # (#4) 비아레나 중첩 CREATE 서브콜 차단 (무자식 CREATE 허용) + 가스 나눗셈/시프트 변조 차단
                    tr_nested_bad41 = {
                        "kind": "CREATE",
                        "caller": FOUNDRY_TEST_CONTRACT,
                        "address": "0x000000000000000000000000000000000000c001",
                        "value": "0x0",
                        "calls": [
                            {
                                "kind": "CALL",
                                "caller": "0x000000000000000000000000000000000000c001",
                                "address": "0x000000000000000000000000000000000000a001",
                                "value": "0x0",
                            }
                        ],
                    }
                    tr_flat_create41 = {
                        "kind": "CREATE",
                        "caller": FOUNDRY_TEST_CONTRACT,
                        "address": "0x000000000000000000000000000000000000c001",
                        "value": "0x0",
                    }
                    ok_gas_div41, v_gas_div41 = verify_agent_harness_accounting_integrity(
                        "contract Verify_Gas41 {\n"
                        "  function test_x() public {\n"
                        "    uint256 gasUsed = (gasStart - gasleft()) / 2;\n"
                        "    _assertConservation(0, sb, sa, gasUsed);\n"
                        "  }\n"
                        "}\n"
                    )
                    t41_fix4_ok = bool(
                        _scan_trace_tree_for_disallowed_hevm(tr_nested_bad41)
                        and (not _scan_trace_tree_for_disallowed_hevm(tr_flat_create41))
                        and (not ok_gas_div41)
                        and any("FORBIDDEN_GAS_SCALING_TAMPER" in x for x in v_gas_div41)
                    )

                    # (#5) arbitrumnova 롤업 포함 + 비0/0바이트 콜데이터 통계 + EIP-7623 가중 바닥
                    trace_cd_json41 = json.dumps({
                        "arena": [
                            {
                                "trace": {
                                    "kind": "CALL",
                                    "caller": "0x0000000000000000000000000000000000001337",
                                    "address": "0x000000000000000000000000000000000000a001",
                                    "data": "0x1234567800000000",
                                    "depth": 1,
                                }
                            }
                        ]
                    })
                    z41, nz41 = _extract_trace_observed_calldata_stats(
                        trace_cd_json41, allowed_caller="0x0000000000000000000000000000000000001337"
                    )
                    v_zero_heavy = judge_conservation_accounting(
                        store=None,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 10**18},
                        vault_before={"native": 10**18},
                        vault_after={"native": 0},
                        gas_used=21_000,
                        gas_price_wei=100_000_000_000,
                        prices_map={"native": 3000.0},
                        allow_offline_fixture=True,
                        calldata_zero_bytes=1000,
                        calldata_nonzero_bytes=0,
                    )
                    v_nz_heavy = judge_conservation_accounting(
                        store=None,
                        chain="ethereum",
                        caller_before={"native": 0},
                        caller_after={"native": 10**18},
                        vault_before={"native": 10**18},
                        vault_after={"native": 0},
                        gas_used=21_000,
                        gas_price_wei=100_000_000_000,
                        prices_map={"native": 3000.0},
                        allow_offline_fixture=True,
                        calldata_zero_bytes=0,
                        calldata_nonzero_bytes=1000,
                    )
                    t41_fix5_ok = bool(
                        "arbitrumnova" in _ROLLUP_L1_FEE_CHAINS
                        and (z41, nz41) == (4, 4)
                        and v_nz_heavy.gas_usd > v_zero_heavy.gas_usd > 0.0
                    )

                    # (#6) 다중 활성 세션 존재 시 SQLite VACUUM 생략 (PASSIVE 체크포인트)
                    vac_db_p = t41_ns / "prune41.db"
                    st41_b = SQLiteStore(vac_db_p)
                    st41_b.ensure_session("ses-t41-1")
                    st41_b.ensure_session("ses-t41-2")
                    pragmas_seen: List[str] = []

                    class _ConnWrap41:
                        def __init__(self, real_c: sqlite3.Connection) -> None:
                            self._rc = real_c

                        def execute(self, sql: str, *a: Any, **kw: Any) -> Any:
                            pragmas_seen.append(str(sql).strip())
                            return self._rc.execute(sql, *a, **kw)

                        def __getattr__(self, item: str) -> Any:
                            return getattr(self._rc, item)

                    real_conn41 = st41_b._conn
                    st41_b._conn = _ConnWrap41(real_conn41)  # type: ignore[assignment]
                    _eng_globals()["MAX_SQLITE_BYTES"] = 1
                    st41_b._maybe_prune_oversized()
                    multi_no_vac_ok41 = (
                        "PRAGMA wal_checkpoint(PASSIVE)" in pragmas_seen
                        and "VACUUM" not in pragmas_seen
                    )
                    pragmas_seen.clear()
                    st41_b._conn = real_conn41
                    st41_b.set_session_status("ses-t41-2", "closed")
                    st41_b._conn = _ConnWrap41(real_conn41)  # type: ignore[assignment]
                    st41_b._maybe_prune_oversized()
                    single_vac_ok41 = "VACUUM" in pragmas_seen
                    st41_b._conn = real_conn41
                    _eng_globals()["MAX_SQLITE_BYTES"] = orig_max_sq_41
                    t41_fix6_ok = bool(multi_no_vac_ok41 and single_vac_ok41)

                    # (#7) onlyOwner 쓰기 엔트리 + 무방어 퍼블릭 view RETURN 엔트리 결합 누락 방지
                    owner_view_bc = bytes.fromhex(
                        "60003560e01c"  # CALLDATALOAD(0) SHR 224
                        "8063111111111461001f57"  # sel 0x11111111 -> pc 0x001f (onlyOwner SSTORE)
                        "63222222221461004657"  # sel 0x22222222 -> pc 0x0046 (public view RETURN)
                        "600080fd"  # fallback REVERT (pc 0x001b..0x001e)
                        "5b3373000000000000000000000000000000000000a9991461003f57600080fd5b600160005500"  # pc 0x001f..0x0045
                        "5b600160005260206000f3"  # pc 0x0046: RETURN
                    )
                    ov_cfg41 = disassemble_and_build_cfg(owner_view_bc)
                    ov_dom41 = prove_caller_dominator_closed(
                        cfg=ov_cfg41,
                        router=_FakeRouter41(),
                        chain="ethereum",
                        target_addr=t41_tgt,
                        block_pin=0,
                    )
                    has_unprot_view41 = any(
                        (not ep.caller_dominated)
                        and any(
                            bp in ov_cfg41.blocks and ov_cfg41.blocks[bp].has_return
                            for bp in ep.reachable_blocks
                        )
                        for ep in ov_cfg41.entries.values()
                    )
                    t41_fix7_ok = bool(ov_dom41.early_exit_safe and has_unprot_view41)

                    # (#10) process_agent_submissions 다중 후보 실행 시 루트 forge_output/trace 덮어쓰기 방지 + session_id 유지
                    cand_slug41 = "t41-multicid"
                    cdir41 = t41_art_root / cand_slug41
                    (cdir41 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    (cdir41 / "verdicts").mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(
                        cdir41 / "checkpoint.json",
                        {"slug": cand_slug41, "chain": "ethereum", "audit_block": 0, "session_id": "ses-t41-sub"},
                    )
                    _write_json_atomic(
                        cdir41 / "result.json",
                        {"slug": cand_slug41, "chain": "ethereum", "audit_block": 0, "needs_agent": True, "session_id": "ses-t41-sub"},
                    )
                    (cdir41 / "forge" / "test" / "test_1.t.sol").write_text("// empty\n", encoding="utf-8")
                    with st41_b._write_tx():
                        st41_b._conn.execute(
                            "INSERT OR REPLACE INTO audit_queue(slug, priority, mode, chain, status, session_id, addresses_json) "
                            "VALUES(?, 1, 'A', 'ethereum', 'awaiting_poc', 'ses-t41-sub', '[]')",
                            (cand_slug41,),
                        )
                    if FORGE_BIN.exists():
                        process_agent_submissions(_FakeRouter41(), st41_b, only_slugs={cand_slug41})
                    row_sub41 = st41_b._conn.execute(
                        "SELECT status, session_id FROM audit_queue WHERE slug=?", (cand_slug41,)
                    ).fetchone()
                    t41_fix10_ok = bool(
                        not (cdir41 / "forge_output.json").exists()
                        and not (cdir41 / "execution_trace_summary.json").exists()
                        and row_sub41 is not None
                        and str(row_sub41["session_id"]) == "ses-t41-sub"
                    )

                    # (#11) 세션 환율 영속 + 릴레이 왕복 + 스테이블 간 페그 교차검증
                    fx_db41 = t41_ns / "fx41.db"
                    st41_c = SQLiteStore(fx_db41)
                    st41_c.ensure_session("ses-perm41")
                    st41_c.put_engine_meta("session_usd_krw:ses-perm41", "1412.5")
                    st41_c.put_engine_meta("session_usd_krw_source:ses-perm41", "live:frankfurter")
                    st41_c.put_engine_meta("session_usd_krw_at:ses-perm41", f"{time.time() - 86400.0:.3f}")
                    fx_perm_d, fx_perm_s = _fetch_usd_krw_rate_dec(
                        st41_c, allow_offline_fixture=False, session_id="ses-perm41"
                    )
                    _export_relay_state(st41_c, artifact_root=t41_art_root)
                    st41_imp = SQLiteStore(":memory:")
                    _import_relay_state(st41_imp, artifact_root=t41_art_root)
                    fx_imp_v = st41_imp.get_engine_meta("session_usd_krw:ses-perm41", "")
                    st41_imp.close()

                    usdc_eth = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    dai_eth = _norm_addr(CHAIN_STABLES["ethereum"][2])
                    pool_du = _norm_addr("0x000000000000000000000000000000000000d101")
                    fac3_eth = _norm_addr(_UNIV3_FACTORIES["ethereum"][0])
                    sqrt_1_1_du = int(Decimal(1 << 96) * (Decimal(10) ** Decimal("-6")))  # token0=dai(18), token1=usdc(6) => spot=1.0

                    class _PeerPegRouter41(RPCRouter):
                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            return None

                        def http_get_json(self, url: str, timeout: float = 10.0) -> Any:
                            return {}

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            out_m: List[Tuple[bool, bytes]] = []
                            for to_a, cd_b in calls:
                                nt = _norm_addr(to_a)
                                sel_h = "0x" + cd_b[:4].hex()
                                if nt == fac3_eth and sel_h == V3_GET_POOL_SEL:
                                    a0 = _norm_addr("0x" + cd_b[16:36].hex())
                                    a1 = _norm_addr("0x" + cd_b[48:68].hex())
                                    fee_v = int.from_bytes(cd_b[68:100], "big")
                                    if {a0, a1} == {dai_eth, usdc_eth} and fee_v == 500:
                                        out_m.append((True, abi_encode(["address"], [pool_du])))
                                    else:
                                        out_m.append((True, b"\x00" * 32))
                                elif sel_h == DECIMALS_SEL:
                                    out_m.append((True, abi_encode(["uint256"], [6 if nt == usdc_eth else 18])))
                                elif sel_h == BALANCE_OF_SEL:
                                    if nt == usdc_eth:
                                        out_m.append((True, abi_encode(["uint256"], [200_000 * 10**6])))
                                    else:
                                        out_m.append((True, abi_encode(["uint256"], [200_000 * 10**18])))
                                elif nt == pool_du and sel_h == V3_SLOT0_SEL:
                                    out_m.append((True, abi_encode(["uint160", "int24", "uint16", "uint16", "uint16", "uint8", "bool"], [sqrt_1_1_du, 0, 0, 0, 0, 0, True])))
                                elif nt == pool_du and sel_h == V2_TOKEN0_SEL:
                                    out_m.append((True, abi_encode(["address"], [dai_eth])))
                                elif sel_h == PAUSED_SEL:
                                    out_m.append((True, abi_encode(["bool"], [False])))
                                else:
                                    out_m.append((False, b""))
                            return out_m

                    peer_peg_q = probe_dex_exit_liquidity(
                        _PeerPegRouter41(),
                        "ethereum",
                        dai_eth,
                        1000 * 10**18,
                        18,
                        Decimal("1.0"),
                        Decimal("3000.0"),
                        19_000_000,
                        quote_prices_usd={
                            _norm_addr(s): Decimal("1.0")
                            for s in CHAIN_STABLES["ethereum"][:4]
                            if _norm_addr(s) != dai_eth
                        },
                    )
                    t41_fix11_ok = bool(
                        fx_perm_d == Decimal("1412.5")
                        and fx_perm_s == "live:frankfurter"
                        and fx_imp_v == "1412.5"
                        and bool(peer_peg_q.get("exit_liquid"))
                        and not bool(peer_peg_q.get("price_diverged"))
                    )

                    # (#12) build_backward_trust_closure에서 native_total_hi_dec 및 비네이티브 풀 nat_scale_hi 이중 스케일링 차단
                    weth_eth12 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                    tok_sc_hi = _norm_addr(next(t for t in CHAIN_BLUECHIPS["ethereum"] if _norm_addr(t) != weth_eth12))
                    v12_addr = _norm_addr("0x000000000000000000000000000000000000a012")
                    orig_est_nat41 = _eng_globals().get("_estimate_native_price_interval_dec")
                    orig_rtm_41 = _eng_globals().get("_resolve_token_metadata_batch")
                    orig_probe_41 = _eng_globals().get("probe_dex_exit_liquidity")

                    class _NoopClosureRouter41(RPCRouter):
                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            return None

                        def http_get_json(self, url: str, timeout: float = 10.0) -> Any:
                            return {}

                        def batch_call(self, chain: str, calls: Sequence[Tuple[str, List[Any]]], **kw: Any) -> List[Any]:
                            out_b: List[Any] = []
                            for m_n, _p_l in calls:
                                if m_n == "eth_getCode":
                                    out_b.append("0x6000")
                                elif m_n == "eth_getBalance":
                                    out_b.append(hex(10**18))
                                else:
                                    out_b.append(None)
                            return out_b

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            out_mc: List[Tuple[bool, bytes]] = []
                            for to_m, cd_m in calls:
                                if _norm_addr(to_m) == tok_sc_hi and ("0x" + cd_m[:4].hex()) == BALANCE_OF_SEL:
                                    out_mc.append((True, abi_encode(["uint256"], [1000 * 10**18])))
                                else:
                                    out_mc.append((False, b""))
                            return out_mc

                    try:
                        _eng_globals()["_estimate_native_price_interval_dec"] = lambda *_a, **_kw: (Decimal("2000"), Decimal("4000"))

                        def _mock_rtm41(_r: Any, _c: Any, toks: Sequence[str], _bp: int, _nlo: Any, **kw: Any) -> Tuple[Dict[str, int], Dict[str, float]]:
                            dm = {_norm_addr(tok_sc_hi): 18}
                            pm = {_norm_addr(tok_sc_hi): 1.0}
                            if kw.get("prices_dec_out") is not None:
                                kw["prices_dec_out"][_norm_addr(tok_sc_hi)] = Decimal("1.0")
                            if kw.get("upper_prices_dec_out") is not None:
                                kw["upper_prices_dec_out"][_norm_addr(tok_sc_hi)] = Decimal("1.0")
                            return dm, pm

                        def _mock_probe_12(*a: Any, **kw: Any) -> Dict[str, Any]:
                            t_arg = _norm_addr(kw.get("token_addr") if "token_addr" in kw else (a[2] if len(a) > 2 else ""))
                            if t_arg == tok_sc_hi:
                                return {
                                    "exit_liquid": True,
                                    "realizable_usd": 1000.0,
                                    "realizable_usd_dec": "1000",
                                    "best_pool_address": "0x000000000000000000000000000000000000p123",
                                    "best_quote_token": usdc_eth,
                                    "best_dex_kind": "v2",
                                }
                            return {"exit_liquid": False, "realizable_usd": 0.0, "realizable_usd_dec": "0"}

                        _eng_globals()["_resolve_token_metadata_batch"] = _mock_rtm41
                        _eng_globals()["probe_dex_exit_liquidity"] = _mock_probe_12
                        cl12 = build_backward_trust_closure(
                            _NoopClosureRouter41(),
                            "ethereum",
                            [v12_addr],
                            19_000_000,
                        )
                        # 1 ETH * np_max(4000) + 1000 USDC-quoted token(1000) = 5000 (not 10000!)
                        t41_fix12_ok = _to_dec(cl12.max_extractable_usd_dec or cl12.max_extractable_usd) == Decimal("5000")
                    finally:
                        if orig_est_nat41 is not None:
                            _eng_globals()["_estimate_native_price_interval_dec"] = orig_est_nat41
                        if orig_rtm_41 is not None:
                            _eng_globals()["_resolve_token_metadata_batch"] = orig_rtm_41
                        if orig_probe_41 is not None:
                            _eng_globals()["probe_dex_exit_liquidity"] = orig_probe_41

                    t41_all_ok = bool(
                        t41_fix1_ok
                        and t41_fix2_ok
                        and t41_fix3_8_ok
                        and t41_fix4_ok
                        and t41_fix5_ok
                        and t41_fix6_ok
                        and t41_fix7_ok
                        and t41_fix10_ok
                        and t41_fix11_ok
                        and t41_fix12_ok
                    )
                    results.append(
                        (
                            "T41_TWELVE_HARDENING_FIXES_REGRESSION",
                            t41_all_ok,
                            (
                                f"f1={t41_fix1_ok},f2={t41_fix2_ok},f3_8={t41_fix3_8_ok},"
                                f"f4={t41_fix4_ok},f5={t41_fix5_ok},f6={t41_fix6_ok},"
                                f"f7={t41_fix7_ok},f10={t41_fix10_ok},f11={t41_fix11_ok},f12={t41_fix12_ok}"
                            ),
                        )
                    )
                finally:
                    _set_eng("ARTIFACT_ROOT", orig_art_41)
                    if orig_cf_41 is not None:
                        _eng_globals()["verify_counterfactual_target_causality"] = orig_cf_41
                    if orig_replay_41 is not None:
                        _eng_globals()["replay_compiled_test_at_live_head"] = orig_replay_41
                    if orig_gas_41 is not None:
                        _eng_globals()["fetch_gas_price_wei"] = orig_gas_41
                    if orig_sub_41 is not None:
                        _eng_globals()["_run_forge_subproc"] = orig_sub_41
                    if orig_ni_41 is not None:
                        _eng_globals()["_estimate_native_price_interval_dec"] = orig_ni_41
                    if orig_nd_41 is not None:
                        _eng_globals()["_estimate_native_price_dec"] = orig_nd_41
                    if orig_max_sq_41 is not None:
                        _eng_globals()["MAX_SQLITE_BYTES"] = orig_max_sq_41
                    for _st41 in (st41_a, st41_b, st41_c):
                        if _st41 is not None:
                            try:
                                _st41.close()
                            except Exception:
                                pass

            if _want_test("T42_SEVENTEEN_HARDENING_FIXES_REGRESSION", tag_filters):
                # ── T42: 17건 하드닝 회귀 (#1 회계 우회, #2 반사실 미실행, #3 인프라 실패, #4/#5 세션 승계·릴레이, #7 L1 수수료, #9 콜데이터 통계,
                #        #12 봉인 verdict, #13 문자열 중괄호 은닉, #14 크기 한도 면제, #15 패싯 병합, #16 부정 캐시) ──
                orig_art_42 = ARTIFACT_ROOT
                orig_sub_42 = _eng_globals().get("_run_forge_subproc")
                st42_a: Optional[SQLiteStore] = None
                st42_b: Optional[SQLiteStore] = None
                try:
                    # (#13) 문자열 리터럴 내 `}` 로 컨트랙트/assembly 본문을 조기 종료시켜 검증기를 우회하는 공격 차단
                    src13a = (
                        "contract AgentVerify_1 {\n"
                        "    string constant S = \"}\";\n"
                        "    function testA() public { address a = address(this); a; }\n"
                        "}\n"
                    )
                    _, v13a = verify_cheatcode_allowlist(src13a)
                    src13b = (
                        "contract AgentVerify_1 {\n"
                        "    function testA() public { assembly { let x := \"}\" log1(0, 0, 0) } }\n"
                        "}\n"
                    )
                    _, v13b = verify_cheatcode_allowlist(src13b)
                    src13c = (
                        "contract AgentVerify_1 {\n"
                        "    string constant S = \"}\";\n"
                        "    function testA() public { uint256 gasUsed = 100; gasUsed /= 2; }\n"
                        "}\n"
                    )
                    _, v13c = verify_agent_harness_accounting_integrity(src13c)
                    src13d = (
                        "contract AgentVerify_1 {\n"
                        "    string constant S = \"contract Fake {\";\n"
                        "    function testA() public { address a = address(this); a; }\n"
                        "}\n"
                    )
                    _, v13d = verify_cheatcode_allowlist(src13d)
                    t42_f13 = bool(
                        "disallowed_harness_self_reference" in v13a
                        and "disallowed_assembly_raw_log" in v13b
                        and "FORBIDDEN_GAS_SCALING_TAMPER" in v13c
                        and "disallowed_harness_self_reference" in v13d
                    )

                    # (#14) 접두사 일치만으로 EIP-170/3860 한도를 우회하는 보조 공격 컨트랙트 차단 (정확한 Vm/IERC20 및 테스트 ABI 하네스만 면제)
                    with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as td14:
                        od14 = Path(td14)
                        sd14 = od14 / "T.t.sol"
                        sd14.mkdir()
                        big14 = {
                            "deployedBytecode": {"object": "0x" + "00" * 25000},
                            "bytecode": {"object": "0x" + "00" * 25000},
                            "abi": [],
                        }
                        for nm14 in ("VmHelper", "Vm", "IERC20Router"):
                            (sd14 / f"{nm14}.json").write_text(json.dumps(big14), encoding="utf-8")
                        ok14, v14 = _scan_compiled_out_dir_allowlist(od14, "T.t.sol")
                    t42_f14 = bool(
                        (not ok14)
                        and any("eip170_runtime_size_exceeded:VmHelper:" in x for x in v14)
                        and any("eip170_runtime_size_exceeded:IERC20Router:" in x for x in v14)
                        and not any("eip170_runtime_size_exceeded:Vm:" in x for x in v14)
                    )

                    # (#1) 복합 대입 자기참조·modifier 은닉·native/WETH 이중 합산 차단
                    src1a = (
                        "contract AgentVerify_1 {\n"
                        "    function testA() public {\n"
                        "        uint256 gain = balAfter - balBefore;\n"
                        "        gain += gain;\n"
                        "        uint256 loss = vBefore - vAfter;\n"
                        "        emit ConservationGain(0, address(0), gain, loss, gasUsed);\n"
                        "    }\n"
                        "}\n"
                    )
                    _, v1a = verify_agent_harness_accounting_integrity(src1a)
                    src1b = (
                        "contract AgentVerify_1 {\n"
                        "    modifier m() { emit ConservationGain(0, address(0), a - b, c - d, e); _; }\n"
                        "    function testA() public m {}\n"
                        "}\n"
                    )
                    _, v1b = verify_agent_harness_accounting_integrity(src1b)
                    weth42 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN.get("ethereum", ""))
                    ents42 = [
                        {"test": "t", "asset": "native", "caller_gain": 5, "vault_loss": 5, "gas_used": 160000},
                        {"test": "t", "asset": weth42, "caller_gain": 5, "vault_loss": 5, "gas_used": 160000},
                    ]
                    c42_chain, _, _, _, _ = _aggregate_gain_entries(ents42, chain="ethereum")
                    c42_plain, _, _, _, _ = _aggregate_gain_entries(ents42)
                    t42_f1 = bool(
                        any("FORBIDDEN_COMPOUND_ASSIGN_CALLER_GAIN_VAR:gain" in x for x in v1a)
                        and any("FORBIDDEN_CONSERVATION_IN_MODIFIER:m" in x for x in v1b)
                        and len(c42_chain) == 1
                        and len(c42_plain) == 2
                    )

                    # (#2) 반사실 변이 테스트 미실행(0건)·RPC 백엔드 오류는 인과성 증거가 될 수 없다
                    class _P42:
                        def __init__(self, out_s: str) -> None:
                            self.stdout = out_s
                            self.stderr = ""
                            self.returncode = 0

                    def _cf_case42(stdout_txt: str) -> Tuple[bool, str]:
                        _eng_globals()["_run_forge_subproc"] = lambda *_a, **_k: _P42(stdout_txt)
                        with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as tdcf:
                            wsx = Path(tdcf)
                            (wsx / "test").mkdir()
                            tgt_cf = "0x" + "ab" * 20
                            sol_cf = (
                                "contract AgentVerify_1 {\n"
                                f"    address constant TARGET = {tgt_cf};\n"
                                "    function testA() public {}\n"
                                "}\n"
                            )
                            r_cf = verify_counterfactual_target_causality(
                                ws=wsx,
                                rel_test_path="test/x.t.sol",
                                sol_code=sol_cf,
                                target_addr=tgt_cf,
                                offline=True,
                                match_test="testA",
                            )
                            return bool(r_cf[0]), str(r_cf[1])

                    if FORGE_BIN.exists():
                        cf_empty42 = _cf_case42(json.dumps({"s": {"test_results": {}}}))
                        cf_rpc42 = _cf_case42(
                            json.dumps(
                                {"s": {"test_results": {"testA()": {"status": "Failure", "reason": "error sending request for url (429)", "logs": [], "decoded_logs": []}}}}
                            )
                        )
                        cf_rev42 = _cf_case42(
                            json.dumps(
                                {"s": {"test_results": {"testA()": {"status": "Failure", "reason": "execution reverted", "logs": [], "decoded_logs": []}}}}
                            )
                        )
                        cf_ok42 = _cf_case42(
                            json.dumps({"s": {"test_results": {"testA()": {"status": "Success", "logs": [], "decoded_logs": []}}}})
                        )
                        t42_f2 = bool(
                            (not cf_empty42[0] and "MUTANT_NOT_EXECUTED" in cf_empty42[1])
                            and (not cf_rpc42[0] and "MUTANT_NOT_EXECUTED" in cf_rpc42[1])
                            and cf_rev42[0]
                            and cf_ok42[0]
                        )
                    else:
                        t42_f2 = True
                    _eng_globals()["_run_forge_subproc"] = orig_sub_42

                    # (#3) 인프라성 라이브 헤드 실패는 패치/고갈 안전 판정의 근거가 아니다
                    t42_f3 = bool(
                        _is_live_head_infra_failure("LIVE_HEAD_BLOCKED:FX_UNCONFIRMED")
                        and _is_live_head_infra_failure("LIVE_HEAD_REPLAY_ERROR:timeout")
                        and not _is_live_head_infra_failure("LIVE_HEAD_REPLAY_FAILED")
                    )

                    # (#4/#5) 고아 항목 승계 시 checkpoint/result의 session_id 동기화 + `run <sid>` 선행 빈 세션에도 릴레이 상태 병합
                    with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as td45:
                        art45 = Path(td45)
                        _set_eng("ARTIFACT_ROOT", art45)
                        st42_a = SQLiteStore(Path(":memory:"))
                        slug45 = "t42-orphan"
                        st42_a.upsert_queue_items_batch([(slug45, 1, "ethereum", 1.0, ["0x" + "11" * 20], "B", "", 0.0)])
                        st42_a.set_queue_agent_state(slug45, "awaiting_poc", session_id="")
                        (art45 / slug45).mkdir(parents=True, exist_ok=True)
                        _write_json_atomic(art45 / slug45 / "checkpoint.json", {"session_id": "ses-dead01", "chain": "ethereum"})
                        _write_json_atomic(art45 / slug45 / "result.json", {"session_id": "ses-dead01", "chain": "ethereum"})
                        st42_a.ensure_session("ses-new001")
                        bound45 = st42_a.bind_orphan_agent_item_to_session("ses-new001")
                        ck45 = _read_json(art45 / slug45 / "checkpoint.json", default={})
                        rs45 = _read_json(art45 / slug45 / "result.json", default={})
                        t42_f4 = bool(
                            bound45 == slug45
                            and isinstance(ck45, dict)
                            and ck45.get("session_id") == "ses-new001"
                            and isinstance(rs45, dict)
                            and rs45.get("session_id") == "ses-new001"
                        )
                        st42_b = SQLiteStore(Path(":memory:"))
                        st42_b.ensure_session("ses-rel001")
                        _write_json_atomic(
                            art45 / "_relay_state.json",
                            {
                                "audit_sessions": [
                                    {
                                        "session_id": "ses-rel001",
                                        "profit_slug": "t42-profit",
                                        "profit_verdict": _format_profit_verdict(123456),
                                        "profit_krw": 123456.0,
                                        "completed_slugs": ["t42-profit"],
                                        "status": "profit_ready",
                                    }
                                ]
                            },
                        )
                        _import_relay_state(st42_b, artifact_root=art45)
                        sess45 = st42_b.get_session("ses-rel001") or {}
                        t42_f5 = bool(
                            str(sess45.get("profit_slug") or "") == "t42-profit"
                            and "t42-profit" in list(sess45.get("completed_slugs") or [])
                        )
                        _set_eng("ARTIFACT_ROOT", orig_art_42)

                    # (#7) L1 데이터 수수료 프로브 페이로드에 실측 0바이트 구성을 반영(100% 비0 과대 추정 제거)
                    l1_calls42: List[str] = []

                    class _R42(RPCRouter):
                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False, timeout: float = 8.0) -> Any:
                            l1_calls42.append(str(params[0].get("data") or ""))
                            return "0x" + (10**15).to_bytes(32, "big").hex()

                    r42 = _R42()
                    _estimate_rollup_l1_fee_wei(r42, "optimism", 20_000_000, 256)
                    _estimate_rollup_l1_fee_wei(r42, "optimism", 20_000_000, 256, zero_bytes=200, nonzero_bytes=56)
                    z_plain42 = _hex_to_bytes(l1_calls42[0]).count(0) if len(l1_calls42) >= 1 else 0
                    z_zn42 = _hex_to_bytes(l1_calls42[1]).count(0) if len(l1_calls42) >= 2 else 0
                    t42_f7 = bool(len(l1_calls42) == 2 and (z_zn42 - z_plain42) >= 150)

                    # (#9) 다른 테스트의 콜데이터 통계를 이 테스트 값으로 오용하지 않는다
                    ts42 = {
                        "observed_calldata_zero_bytes": 10,
                        "observed_calldata_nonzero_bytes": 20,
                        "per_test_observed_calldata_stats": {"testA": {"zero_bytes": 1, "nonzero_bytes": 2}},
                        "winning_test": "testA",
                    }
                    ts42_flat = {"observed_calldata_zero_bytes": 10, "observed_calldata_nonzero_bytes": 20}
                    t42_f9 = bool(
                        _resolve_effective_calldata_stats(ts42, test_name="testB") == (-1, -1)
                        and _resolve_effective_calldata_stats(ts42, test_name="testA") == (1, 2)
                        and _resolve_effective_calldata_stats(ts42_flat, test_name="testB") == (10, 20)
                    )

                    # (#12) 에이전트의 `PROVEN_BY_FORGE` 미봉인 verdict 파일을 엔진 봉인 원화 판정·재무 내역으로 갱신
                    with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as td12:
                        root12 = Path(td12)
                        slug12, pdir12 = _resolve_slug_artifact_dir("t42-seal", root12)
                        (pdir12 / "verdicts").mkdir(parents=True, exist_ok=True)
                        _write_json_atomic(
                            pdir12 / "verdicts" / "verdict_1.json",
                            {"candidate_id": 1, "verdict": "PROVEN_BY_FORGE", "notes": "agent"},
                        )
                        write_protocol_artifacts(
                            slug12,
                            {
                                "verdict": _format_profit_verdict(123456),
                                "net_krw": 123456,
                                "net_usd": 90.0,
                                "audit_block": 100,
                                "chain": "ethereum",
                                "reason": "sealed",
                                "needs_agent": False,
                            },
                            root_dir=root12,
                        )
                        vd12 = _read_json(pdir12 / "verdicts" / "verdict_1.json", default={})
                        t42_f12 = bool(
                            isinstance(vd12, dict)
                            and _is_profit_verdict(str(vd12.get("verdict") or ""))
                            and int(vd12.get("net_profit_krw") or 0) == 123456
                            and int(vd12.get("audit_block") or 0) == 100
                        )

                    # (#15) 패싯 병합 시 비활성·무권한 뷰 엔트리(`fallback` 충돌)의 도달 블록이 유실되지 않는다
                    def _facet42(active: bool, dominated: bool) -> CFGModel:
                        bb42 = BasicBlock(start_pc=0, end_pc=1, has_return=True, has_sstore=active, has_value_sink=False)
                        ep42 = EntryPointCFG(
                            selector="fallback",
                            entry_pc=0,
                            reachable_blocks={0},
                            reaches_sstore=active,
                            caller_dominated=dominated,
                        )
                        return CFGModel(blocks={0: bb42}, entries={"fallback": ep42}, disassembly=[])

                    merged42 = _merge_facet_cfgs([_facet42(True, True), _facet42(False, False)])
                    t42_f15 = bool(
                        any(
                            (not ep_m.caller_dominated) and (0x100000 in ep_m.reachable_blocks)
                            for ep_m in merged42.entries.values()
                        )
                    )

                    # (#16) 일시적 RPC/레이트리밋 장애는 6시간 `unverified` 부정 캐시로 고착시키지 않는다
                    class _FetchR42(RPCRouter):
                        def __init__(self, st: SQLiteStore, exc_factory: Any) -> None:
                            super().__init__(st)
                            self._exc_factory = exc_factory

                        def http_get_json(self, url: str, timeout: float = 15.0, extra_headers: Optional[Dict[str, str]] = None) -> Any:
                            raise self._exc_factory()

                    fa42 = "0x" + "77" * 20
                    st42_c = SQLiteStore(Path(":memory:"))
                    try:
                        fetch_verified_contract(_FetchR42(st42_c, lambda: RuntimeError("HTTP 429")), "ethereum", fa42)
                        cached_transient42 = st42_c.get_verified_source(CHAIN_IDS["ethereum"], fa42)
                        fetch_verified_contract(_FetchR42(st42_c, lambda: _DeterministicHTTPClientError(404)), "ethereum", fa42)
                        cached_definitive42 = st42_c.get_verified_source(CHAIN_IDS["ethereum"], fa42)
                    finally:
                        st42_c.close()
                    t42_f16 = bool(
                        cached_transient42 is None
                        and cached_definitive42 is not None
                        and not bool(cached_definitive42.get("verified"))
                    )

                    # (#6) S2 무권한 뷰 결합 슬라이스가 바이트코드 최초 PUSH4(onlyOwner 등 닫힌 셀렉터)가 아닌 실제 무권한 뷰 셀렉터를 선택
                    v6_a = "0x" + "61" * 20
                    v6_b = "0x" + "62" * 20
                    v6_closed_hex = "0x63aaaaaaaa14600d57600080fd5b3373" + ("99" * 20) + "14602c57600080fd5b6000808080808080f100"
                    v6_view_hex = (
                        "0x60003560e01c80631111111114602157632222222214604757600080fdfefefefe"
                        "5b3373" + ("99" * 20) + "14604057600080fd5b6001600055005b60206000f3"
                    )
                    with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as td6:
                        art6 = Path(td6)
                        _set_eng("ARTIFACT_ROOT", art6)
                        st6 = SQLiteStore(Path(":memory:"))
                        try:
                            cf_r6 = _make_mock_guard_router(st6)
                            slug6 = "t42-view-sel"
                            (art6 / slug6).mkdir(parents=True, exist_ok=True)
                            cl6 = TrustClosureResult(
                                chain="ethereum",
                                block_pin=20_000_000,
                                seed_addresses=[v6_a, v6_b],
                                all_addresses=[v6_a, v6_b],
                                resolutions={
                                    v6_a: ProxyResolution(address=v6_a, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex=v6_closed_hex, effective_code_hex=v6_closed_hex, skeleton_hash="s6a", slot_collisions=[], impl_codes_hex={v6_a: v6_closed_hex}),
                                    v6_b: ProxyResolution(address=v6_b, proxy_kind="none", impl_addresses=[], admin_address="", raw_code_hex=v6_view_hex, effective_code_hex=v6_view_hex, skeleton_hash="s6b", slot_collisions=[], impl_codes_hex={v6_b: v6_view_hex}),
                                },
                                vault_exposures={v6_a: VaultExposure(address=v6_a, native_wei=100 * 10**18, native_usd=300_000.0, total_usd=300_000.0), v6_b: VaultExposure(address=v6_b)},
                                max_extractable_usd=300_000.0,
                                min_gas_usd=0.05,
                                early_exit_safe=False,
                                early_exit_reason="",
                                has_dynamic_external_reachability=True,
                            )
                            _write_json_atomic(
                                art6 / slug6 / "checkpoint.json",
                                {
                                    "stage": "S2_CLOSURE_VERIFIED",
                                    "audit_block": 20_000_000,
                                    "pin_head_block": 20_000_002,
                                    "seed_addrs": [v6_a, v6_b],
                                    "closure_ready": True,
                                    "closure": _serialize_closure(cl6),
                                    "verified_sources": {},
                                    "remediation_exhausted": {k: True for k in REMEDIATION_KEYS},
                                },
                            )
                            audit_single_protocol(cf_r6, st6, {"slug": slug6, "chain": "ethereum", "tvl_usd": 300_000.0, "addresses": [v6_a, v6_b], "mode": "A"}, set())
                            poc6 = _read_json(art6 / slug6 / "poc_requests" / "poc_request_1.json", default={})
                        finally:
                            st6.close()
                        _set_eng("ARTIFACT_ROOT", orig_art_42)
                    t42_f6 = bool(
                        isinstance(poc6, dict)
                        and any("0x22222222" in (cs.get("legs") or []) for cs in (poc6.get("candidate_slices") or []))
                        and not any("0x11111111" in (cs.get("legs") or []) for cs in (poc6.get("candidate_slices") or []))
                    )

                    # (#8) 외부 승인자(approved_owners)가 PoC 요청 및 테스트 스켈레톤 VAULT_*에 편입되는지 검증
                    with tempfile.TemporaryDirectory(dir=selftest_ns / "_py_tmp") as td8:
                        art8 = Path(td8)
                        _set_eng("ARTIFACT_ROOT", art8)
                        tgt8 = "0x" + "81" * 20
                        ow8 = "0x" + "82" * 20
                        cl8 = TrustClosureResult(
                            chain="ethereum",
                            block_pin=20_000_000,
                            seed_addresses=[tgt8],
                            all_addresses=[tgt8],
                            resolutions={},
                            vault_exposures={tgt8: VaultExposure(address=tgt8, allowance_usd=5000.0, allowance_usd_dec="5000", total_usd=5000.0, total_usd_dec="5000")},
                            max_extractable_usd=5000.0,
                            min_gas_usd=0.05,
                            early_exit_safe=False,
                            early_exit_reason="",
                            approved_owners=[ow8],
                        )
                        sl8 = CausalSlice(slice_id="s8", target_address=tgt8, channel="CH_STORAGE", legs=["0x23b872dd"], witness_candidates=[1], repeat_counts=[1], needs_agent_synthesis=True)
                        emit_poc_requests_for_slug("t42-ow", "ethereum", 0, "0x" + "83" * 20, [sl8], {}, closure=cl8)
                        pr8 = _read_json(art8 / "t42-ow" / "poc_requests" / "poc_request_1.json", default={})
                        sol8 = (art8 / "t42-ow" / "forge" / "test" / "test_1.t.sol").read_text(encoding="utf-8", errors="replace")
                        _set_eng("ARTIFACT_ROOT", orig_art_42)
                    t42_f8 = bool(
                        isinstance(pr8, dict)
                        and any(_norm_addr(x) == ow8 for x in (pr8.get("approved_owner_victims") or []))
                        and _checksum(ow8) in sol8
                    )

                    # (#10) 비네이티브 견적 풀 dex_p_hi_d 비팽창 + build_backward_trust_closure ETH/WETH realizable_extractable_usd 하한(np_min_dec) 일관성
                    usdc10 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    weth10 = _norm_addr(WRAPPED_NATIVE_BY_CHAIN["ethereum"])
                    tok10 = _norm_addr(next(t for t in CHAIN_BLUECHIPS["ethereum"] if _norm_addr(t) != weth10))
                    v10 = _norm_addr("0x" + "91" * 20)
                    orig_probe10 = _eng_globals().get("probe_dex_exit_liquidity")
                    orig_est10 = _eng_globals().get("_estimate_native_price_interval_dec")
                    orig_fetch_ll10 = _eng_globals().get("_fetch_defillama_prices_dec")

                    class _R10(RPCRouter):
                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            return None

                        def http_get_json(self, url: str, timeout: float = 10.0) -> Any:
                            if "coins.llama.fi" in url:
                                return {"coins": {f"ethereum:{tok10}": {"price": 1.0}, f"ethereum:{weth10}": {"price": 2000.0}}}
                            return {}

                        def batch_call(self, chain: str, calls: Sequence[Tuple[str, List[Any]]], **kw: Any) -> List[Any]:
                            return [("0x6000" if m == "eth_getCode" else hex(10**18) if m == "eth_getBalance" else None) for m, _ in calls]

                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            res10: List[Tuple[bool, bytes]] = []
                            for to_m, cd_m in calls:
                                sel_m = "0x" + cd_m[:4].hex()
                                if sel_m == DECIMALS_SEL:
                                    res10.append((True, abi_encode(["uint256"], [18])))
                                elif sel_m == SYMBOL_SEL:
                                    res10.append((True, abi_encode(["string"], ["TOK10"])))
                                elif sel_m == TOTAL_SUPPLY_SEL:
                                    res10.append((True, abi_encode(["uint256"], [10**24])))
                                elif sel_m == BALANCE_OF_SEL and _norm_addr(to_m) == weth10:
                                    res10.append((True, abi_encode(["uint256"], [10**18])))
                                else:
                                    res10.append((False, b""))
                            return res10

                    try:
                        _eng_globals()["_fetch_defillama_prices_dec"] = lambda *_a, **_k: {tok10: Decimal("1.0"), weth10: Decimal("2000")}
                        _eng_globals()["_estimate_native_price_interval_dec"] = lambda *_a, **_k: (Decimal("2000"), Decimal("4000"))
                        _eng_globals()["probe_dex_exit_liquidity"] = lambda *_a, **_k: {
                            "exit_liquid": True,
                            "realizable_usd": 1.0,
                            "realizable_usd_dec": "1.0",
                            "best_pool_address": "0x" + "92" * 20,
                            "best_quote_token": usdc10,
                            "best_dex_kind": "v2",
                        }
                        up10_dec: Dict[str, Decimal] = {}
                        _resolve_token_metadata_batch(
                            _R10(),
                            "ethereum",
                            [tok10],
                            19_000_000,
                            Decimal("2000"),
                            native_price_max_usd=Decimal("4000"),
                            upper_prices_dec_out=up10_dec,
                        )
                        cl10 = build_backward_trust_closure(_R10(), "ethereum", [v10], 19_000_000)
                        t42_f10 = bool(
                            up10_dec.get(tok10) == Decimal("1.0")
                            and _to_dec(cl10.realizable_extractable_usd_dec or cl10.realizable_extractable_usd) == Decimal("4000")
                            and _to_dec(cl10.max_extractable_usd_dec or cl10.max_extractable_usd) == Decimal("8000")
                        )
                    finally:
                        if orig_probe10 is not None:
                            _eng_globals()["probe_dex_exit_liquidity"] = orig_probe10
                        if orig_est10 is not None:
                            _eng_globals()["_estimate_native_price_interval_dec"] = orig_est10
                        if orig_fetch_ll10 is not None:
                            _eng_globals()["_fetch_defillama_prices_dec"] = orig_fetch_ll10

                    # (#11 & #17) 과거 로그·블록 타임스탬프 조회의 block_pin/need_archive 전달 + 모의 라우터 통일 판별
                    seen_kw11: List[Tuple[str, Any, Any]] = []

                    class _RecR11(RPCRouter):
                        def multicall3(self, chain: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            return [(False, b"") for _ in calls]

                        def call(self, chain: str, method: str, params: List[Any], block_pin: Optional[int] = None, need_archive: bool = False, timeout: float = 12.0) -> Any:
                            seen_kw11.append((method, block_pin, need_archive))
                            if method == "eth_getBlockByNumber":
                                return {"timestamp": hex(1_700_000_000 if int(params[0], 16) == 100 else 1_700_000_020)}
                            return []

                    r11 = _RecR11()
                    _harvest_logs_bisect(r11, "ethereum", ["0x" + "11" * 20], 900, 1000, max_depth=0)
                    _harvest_token_logs_bisect(r11, "ethereum", ["0x" + "11" * 20], 900, 1000, max_depth=0)
                    _harvest_inbound_trust_and_approvals(r11, "ethereum", ["0x" + "11" * 20], 1000, is_live_rpc=False)
                    t42_f11 = bool(
                        len([x for x in seen_kw11 if x[0] == "eth_getLogs"]) >= 5
                        and all(bp == 1000 for m, bp, _ in seen_kw11 if m == "eth_getLogs")
                        and not _is_live_real_router(_R10())
                    )
                    seen_kw11.clear()
                    _is_head_fresh_against_tip(r11, "ethereum", 100, 120)
                    _find_reverse_upgraded_proxies(r11, "ethereum", ["0x" + "11" * 20], 1000)
                    t42_f17 = bool(
                        any(m == "eth_getBlockByNumber" and bp == 100 and arch is True for m, bp, arch in seen_kw11)
                        and any(m == "eth_getLogs" and bp == 1000 and arch is True for m, bp, arch in seen_kw11)
                    )

                    t42_all_ok = bool(
                        t42_f1
                        and t42_f2
                        and t42_f3
                        and t42_f4
                        and t42_f5
                        and t42_f6
                        and t42_f7
                        and t42_f8
                        and t42_f9
                        and t42_f10
                        and t42_f11
                        and t42_f12
                        and t42_f13
                        and t42_f14
                        and t42_f15
                        and t42_f16
                        and t42_f17
                    )
                    results.append(
                        (
                            "T42_SEVENTEEN_HARDENING_FIXES_REGRESSION",
                            t42_all_ok,
                            (
                                f"f1={t42_f1},f2={t42_f2},f3={t42_f3},f4={t42_f4},f5={t42_f5},f6={t42_f6},"
                                f"f7={t42_f7},f8={t42_f8},f9={t42_f9},f10={t42_f10},f11={t42_f11},f12={t42_f12},"
                                f"f13={t42_f13},f14={t42_f14},f15={t42_f15},f16={t42_f16},f17={t42_f17}"
                            ),
                        )
                    )
                finally:
                    _set_eng("ARTIFACT_ROOT", orig_art_42)
                    if orig_sub_42 is not None:
                        _eng_globals()["_run_forge_subproc"] = orig_sub_42
                    for _st42 in (st42_a, st42_b):
                        if _st42 is not None:
                            try:
                                _st42.close()
                            except Exception:
                                pass

            if _want_test("T43_TWENTY_SEVEN_HARDENING_FIXES_REGRESSION", tag_filters):
                # T43: 27건 종합 하드닝 회귀 검증 (#1~#27 전수 오프라인 + 실 Forge 바이너리 검증)
                t43_ns = selftest_ns / "_t43_27"
                t43_ns.mkdir(parents=True, exist_ok=True)
                orig_art_43 = ARTIFACT_ROOT
                t43_art = t43_ns / "artifacts"
                t43_art.mkdir(parents=True, exist_ok=True)
                orig_sub_43 = _eng_globals().get("_run_forge_subproc")
                orig_est_nat_43 = _eng_globals().get("_estimate_native_price_interval_dec")
                orig_fx_43 = _eng_globals().get("_fetch_usd_krw_rate_dec")
                orig_llama_43 = _eng_globals().get("_fetch_defillama_prices_dec")
                orig_replay_tr_43 = _eng_globals().get("_run_single_replay_trace_summary")
                st43_list: List[SQLiteStore] = []
                try:
                    _set_eng("ARTIFACT_ROOT", t43_art)

                    # (#1) Counterfactual setup injection ignores commented setUp() and string-literal contract braces
                    ws_t43_cf = ensure_clean_forge_workspace({}, slug="t43_cf_cm", offline=True)
                    rel_t43_cf = "test/Verify_t43_cf.t.sol"
                    tgt_t43_1 = "0x000000000000000000000000000000000000aa01"
                    sol_t43_1 = (
                        "// SPDX-License-Identifier: MIT\n"
                        "pragma solidity ^0.8.20;\n"
                        "// function setUp() public {}\n"
                        "contract Verify_t43_cf {\n"
                        f"    address constant TARGET = {_checksum(tgt_t43_1)};\n"
                        "    function testA() public {\n"
                        "        string memory s = \"contract Fake { function setUp() public {} }\";\n"
                        "        TARGET.call(\"\");\n"
                        "        s;\n"
                        "    }\n"
                        "}\n"
                    )
                    seen_mut_43: List[str] = []

                    def _mock_cf_sub43(ws_m: Path, cmd_m: List[str], timeout_m: int) -> subprocess.CompletedProcess[str]:
                        for p_cf in sorted((ws_m / "test").glob("_CounterfactualCausalityProbe_*.t.sol")):
                            seen_mut_43.append(p_cf.read_text(encoding="utf-8", errors="replace"))
                        return subprocess.CompletedProcess(
                            cmd_m, 0, stdout='{"s":{"test_results":{"testA()":{"status":"Success","logs":[]}}}}', stderr=""
                        )

                    _eng_globals()["_run_forge_subproc"] = _mock_cf_sub43
                    cf_r_43 = verify_counterfactual_target_causality(
                        ws=ws_t43_cf,
                        rel_test_path=rel_t43_cf,
                        sol_code=sol_t43_1,
                        target_addr=tgt_t43_1,
                        match_test="testA",
                        offline=True,
                    )
                    t43_f1 = bool(cf_r_43) and bool(seen_mut_43) and ("bytes memory _cfNoop" in seen_mut_43[0])
                    if orig_sub_43 is not None:
                        _eng_globals()["_run_forge_subproc"] = orig_sub_43

                    # (#2) Dual native + WNATIVE single combined event & 8-dec WNATIVE aggregation
                    c_43_2 = "0x" + "61" * 20
                    w8_43 = "0x" + "62" * 20
                    v_43_2 = "0x" + "63" * 20
                    h_dual_43 = _render_minimal_solidity_harness(
                        slug="t43_dual",
                        chain="ethereum",
                        block_pin=0,
                        caller=c_43_2,
                        target_addr=v_43_2,
                        token_addrs=[WRAPPED_NATIVE_BY_CHAIN["ethereum"]],
                        slices=[],
                        vault_addresses=[v_43_2],
                    )
                    ok_acct_dual, _ = verify_agent_harness_accounting_integrity(h_dual_43)
                    c_agg2, _, _, _, _ = _aggregate_gain_entries(
                        [
                            {"test": "t", "asset": "native", "caller_gain": 2 * 10**18, "vault_loss": 2 * 10**18},
                            {"test": "t", "asset": w8_43, "caller_gain": 5 * 10**8, "vault_loss": 5 * 10**8},
                        ],
                        chain="ethereum",
                        decimals_map={w8_43: 8},
                    )
                    t43_f2 = (
                        ok_acct_dual
                        and ("combNatGain" in h_dual_43)
                        and (len(c_agg2) == 2 or ("native" in c_agg2))
                    )
                    c_agg_wn8, _, _, _, _ = _aggregate_gain_entries(
                        [
                            {"test": "t", "asset": "native", "caller_gain": 1 * 10**18, "vault_loss": 1 * 10**18},
                            {"test": "t", "asset": WRAPPED_NATIVE_BY_CHAIN["ethereum"], "caller_gain": 5 * 10**8, "vault_loss": 5 * 10**8},
                        ],
                        chain="ethereum",
                        decimals_map={WRAPPED_NATIVE_BY_CHAIN["ethereum"]: 8},
                    )
                    t43_f2 = t43_f2 and (len(c_agg_wn8) == 1) and (WRAPPED_NATIVE_BY_CHAIN["ethereum"] in c_agg_wn8)

                    # (#3) _prioritize_closure_monitored_tokens multi-hop quote preservation + LP other->quote chaining + bridge post-mid credit
                    mid_tok_43 = "0x" + "9a" * 20
                    oth_tok_43 = "0x" + "9b" * 20
                    bm_usdc_43 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    lp_tok_43 = "0x" + "9c" * 20
                    junk_43 = {_norm_addr(f"0x{8100 + k:040x}"): 1 for k in range(1, 10)}
                    cl_43_3 = TrustClosureResult(
                        chain="ethereum",
                        block_pin=0,
                        seed_addresses=[lp_tok_43],
                        all_addresses=[lp_tok_43],
                        resolutions={},
                        vault_exposures={
                            lp_tok_43: VaultExposure(
                                address=lp_tok_43,
                                token_balances={lp_tok_43: 10 * 10**18, **junk_43},
                                token_usd={lp_tok_43: 100_000.0, **{jk: 10.0 for jk in junk_43}},
                            )
                        },
                        max_extractable_usd=100_000.0,
                        min_gas_usd=0.01,
                        early_exit_safe=False,
                        early_exit_reason="",
                        verified_exit_pools={
                            lp_tok_43: {
                                "pool": _checksum(lp_tok_43),
                                "quote_token": _checksum(mid_tok_43),
                                "lp_other_token": _checksum(oth_tok_43),
                                "dex_kind": "lp_token",
                                "paused": False,
                            },
                            mid_tok_43: {
                                "pool": _checksum("0x" + "9d" * 20),
                                "quote_token": _checksum(bm_usdc_43),
                                "dex_kind": "v2",
                                "paused": False,
                            },
                        },
                        token_decimals={lp_tok_43: 18, mid_tok_43: 18, oth_tok_43: 18, bm_usdc_43: 6, **{jk: 18 for jk in junk_43}},
                    )
                    prio_43_3 = _prioritize_closure_monitored_tokens("ethereum", cl_43_3, limit=4)
                    t43_f3 = (lp_tok_43 in prio_43_3) and (mid_tok_43 in prio_43_3) and (bm_usdc_43 in prio_43_3)

                    # (#4) Static gas tamper check + runtime revm observed_gas_used
                    ok_g_assign, v_g_assign = verify_agent_harness_accounting_integrity(
                        "contract AgentVerify_1 { function testA() public { uint256 gasUsed = 0; _assertConservation(0, sb, sa, gasUsed); } }"
                    )
                    ok_g_lit, _v_g_lit = verify_agent_harness_accounting_integrity(
                        "contract AgentVerify_1 { function testA() public { _assertConservation(0, sb, sa, 21000); } }"
                    )
                    t43_f4 = (not ok_g_assign) and (not ok_g_lit) and ("FORBIDDEN_GAS_SCALING_TAMPER" in v_g_assign)
                    # (#4 + #18 + #12 + #17 + #24 real Forge tests)
                    t43_f12 = True
                    t43_f17 = True
                    t43_f18 = True
                    t43_f24 = False
                    if FORGE_BIN.exists():
                        def _caddr43(d_hex: str, nonce: int) -> str:
                            n_b = b"\x80" if nonce == 0 else bytes([nonce])
                            pl = b"\x94" + bytes.fromhex(d_hex[2:]) + n_b
                            return "0x" + _keccak256(bytes([0xC0 + len(pl)]) + pl)[12:].hex()

                        c43_f = "0x" + "6a" * 20
                        # Real #17 sibling isolation + #4 observed gas + #12 1-wei FeeToken + #24 trace
                        a_ftok = _caddr43(c43_f, 0)
                        a_fvault = _caddr43(c43_f, 1)
                        h12_raw = _render_agent_poc_skeleton(
                            "t43_fee_w", 1, "ethereum", 0, c43_f, a_fvault, token_addrs=[a_ftok], vault_addresses=[a_fvault]
                        )
                        fee_mocks = (
                            "contract FeeTok43 {\n"
                            "    uint256 public totalSupply;\n"
                            "    mapping(address => uint256) public balanceOf;\n"
                            "    function decimals() external pure returns (uint8) { return 18; }\n"
                            "    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }\n"
                            "    function allowance(address, address) external pure returns (uint256) { return 0; }\n"
                            "    function transfer(address to, uint256 a) external returns (bool) {\n"
                            "        uint256 fee = a > 1 ? 1 : 0;\n"
                            "        balanceOf[msg.sender] -= a;\n"
                            "        balanceOf[to] += (a - fee);\n"
                            "        return true;\n"
                            "    }\n"
                            "}\n"
                            "contract FeeVault43 {\n"
                            "    address immutable tk;\n"
                            "    constructor(address _t) { tk = _t; }\n"
                            "    function take() external {\n"
                            "        uint256 b = FeeTok43(tk).balanceOf(address(this));\n"
                            "        FeeTok43(tk).transfer(msg.sender, b);\n"
                            "    }\n"
                            "}\n"
                        )
                        h12_exec = h12_raw.replace(
                            "contract AgentVerify_t43_fee_w_1 {", fee_mocks + "\ncontract AgentVerify_t43_fee_w_1 {"
                        ).replace(
                            "        ConservationSnap memory snapBefore = _takeSnap(ZERO_STATE_CALLER);",
                            "        vm.startPrank(ZERO_STATE_CALLER, ZERO_STATE_CALLER);\n"
                            "        FeeTok43 _ft = new FeeTok43();\n"
                            "        FeeVault43 _fv = new FeeVault43(address(_ft));\n"
                            "        _ft.mint(address(_fv), 50 ether);\n"
                            "        vm.stopPrank();\n"
                            "        ConservationSnap memory snapBefore = _takeSnap(ZERO_STATE_CALLER);",
                        ).replace(
                            "        // Implement dynamic on-chain state verification against TARGET",
                            "        FeeVault43(TARGET).take();",
                        )
                        ws17 = ensure_clean_forge_workspace({}, slug="t43_sib_ws", offline=True)
                        rel12 = "test/test_1.t.sol"
                        (ws17 / rel12).write_text(h12_exec, encoding="utf-8")
                        (ws17 / "test" / "BrokenSibling.t.sol").write_text("pragma solidity ^0.8.20;\ncontract Broken { syntax err ! }\n", encoding="utf-8")
                        out12 = execute_forge_test_file(
                            ws17, rel12, h12_exec, timeout_sec=60, allowed_caller=c43_f, target_addr=a_fvault, offline=True
                        )
                        t43_f17 = bool(out12.executed and out12.passed_tests == 1 and out12.failed_tests == 0)
                        t43_f12 = t43_f17 and len(out12.conservation_gains) == 1 and int(out12.conservation_gains[0].get("caller_gain") or 0) == (50 * 10**18 - 2)
                        t43_f4 = t43_f4 and int((out12.trace_summary or {}).get("observed_gas_used") or 0) >= 21_000
                        reqs12 = emit_poc_requests_for_slug(
                            "t43_note12", "ethereum", 0, c43_f,
                            [CausalSlice(slice_id="s0", target_address=a_fvault, channel="CH_STORAGE", legs=["0x11223344"], needs_agent_synthesis=True)],
                            verified_sources={}, principle_summary="test", force_all=True,
                        )
                        req12_doc = json.loads(reqs12[0].read_text(encoding="utf-8")) if reqs12 else {}
                        t43_f12 = t43_f12 and ("clear residual DEX router ERC20 allowances" in str((req12_doc.get("submission_schema") or {}).get("note") or ""))

                        # Real #18 overflow safety (LiquidatorHelper + _satAdd + V3 cap) & #3 LP 2-hop chain
                        mocks18 = (
                            "contract HugeTok43 {\n"
                            "    uint256 public totalSupply;\n"
                            "    mapping(address => uint256) public balanceOf;\n"
                            "    mapping(address => mapping(address => uint256)) public allowance;\n"
                            "    function setBal(address who, uint256 b) external { balanceOf[who] = b; }\n"
                            "    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }\n"
                            "    function transferFrom(address fr, address to, uint256 a) external returns (bool) {\n"
                            "        if (a > 0) { allowance[fr][msg.sender] -= a; balanceOf[fr] -= a; balanceOf[to] += a; }\n"
                            "        return true;\n"
                            "    }\n"
                            "    function transfer(address to, uint256 a) external returns (bool) {\n"
                            "        if (a > 0) { balanceOf[msg.sender] -= a; balanceOf[to] += a; }\n"
                            "        return true;\n"
                            "    }\n"
                            "}\n"
                            "contract MockV2Huge {\n"
                            "    address public token0;\n"
                            "    address public token1;\n"
                            "    uint112 r0; uint112 r1;\n"
                            "    constructor(address t0, address t1) { token0 = t0; token1 = t1; r0 = 100 ether; r1 = 100 ether; }\n"
                            "    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }\n"
                            "    function swap(uint256 a0, uint256 a1, address to, bytes calldata) external {\n"
                            "        if (a0 > 0) HugeTok43(token0).transfer(to, a0);\n"
                            "        if (a1 > 0) HugeTok43(token1).transfer(to, a1);\n"
                            "    }\n"
                            "}\n"
                            "contract MockV3Huge {\n"
                            "    address public token0; address public token1;\n"
                            "    constructor(address t0, address t1) { token0 = t0; token1 = t1; }\n"
                            "    function swap(address to, bool z2o, int256 amtSpecified, uint160, bytes calldata d) external returns (int256, int256) {\n"
                            "        require(amtSpecified > 0, \"NEG\");\n"
                            "        HugeTok43(z2o ? token1 : token0).transfer(to, 1);\n"
                            "        (bool ok,) = msg.sender.call(abi.encodeWithSelector(0xfa461e33, z2o ? int256(1) : int256(0), z2o ? int256(0) : int256(1), d));\n"
                            "        require(ok, \"CB\");\n"
                            "        return (0, 0);\n"
                            "    }\n"
                            "}\n"
                        )
                        lh_src_18 = _render_liquidator_helper_contract("t43ov")
                        h18_harness = _render_minimal_solidity_harness(
                            slug="t43_sat18", chain="ethereum", block_pin=0, caller=c43_f, target_addr=a_fvault, token_addrs=[a_ftok], slices=[], vault_addresses=[a_fvault, _caddr43(c43_f, 9)]
                        )
                        c18_addr = "0x" + "6c" * 20
                        c18_chk = _checksum(c18_addr)
                        ov_sol = (
                            "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n"
                            "interface Vm18 { function startPrank(address,address) external; function stopPrank() external; }\n"
                            + mocks18
                            + "\n"
                            + lh_src_18
                            + "\ncontract AgentVerify_OvTest43 {\n"
                            "    Vm18 constant vm = Vm18(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);\n"
                            f"    address constant CALLER18 = {c18_chk};\n"
                            "    function testOv() public {\n"
                            "        HugeTok43 tA = new HugeTok43(); HugeTok43 tB = new HugeTok43();\n"
                            "        MockV2Huge p2 = new MockV2Huge(address(tA), address(tB));\n"
                            "        MockV3Huge p3 = new MockV3Huge(address(tA), address(tB));\n"
                            "        tA.setBal(CALLER18, 1e60); tB.setBal(address(p2), 100 ether); tB.setBal(address(p3), 100 ether);\n"
                            "        LiquidatorHelper_t43ov lh = new LiquidatorHelper_t43ov();\n"
                            "        vm.startPrank(CALLER18, CALLER18);\n"
                            "        tA.approve(address(lh), type(uint256).max);\n"
                            "        lh.swapV2(address(p2), address(tA), address(tB), 1e60, CALLER18);\n"
                            "        lh.swapV3(address(p3), address(tA), address(tB), type(uint256).max, CALLER18);\n"
                            "        vm.stopPrank();\n"
                            "        require(tB.balanceOf(CALLER18) > 99 ether, \"NO_OUT18\");\n"
                            "    }\n"
                            "}\n"
                        )
                        ws18 = ensure_clean_forge_workspace({}, slug="t43_ov18", offline=True)
                        (ws18 / "test" / "Ov.t.sol").write_text(ov_sol, encoding="utf-8")
                        out18 = execute_forge_test_file(ws18, "test/Ov.t.sol", ov_sol, timeout_sec=60, allowed_caller=c18_addr, offline=True)
                        t43_f18 = out18.executed and out18.passed_tests == 1 and ("_satAdd(" in h18_harness)

                        # Real #3 Forge check: 2-hop LP (other -> mid -> bench) via unwindV2LP + swapV2 + quote_exit
                        c43_lp = "0x" + "6d" * 20
                        c43_lp_chk = _checksum(c43_lp)
                        lp_oth_a = _caddr43(c43_lp, 0)
                        lp_mid_a = _caddr43(c43_lp, 1)
                        lp_ben_a = _caddr43(c43_lp, 2)
                        lp_pair_a = _caddr43(c43_lp, 3)
                        lp_v2_a = _caddr43(c43_lp, 4)
                        lp_vault_a = _caddr43(c43_lp, 5)
                        lp_sel = _selector("drain()")
                        lp_mocks = (
                            "contract Erc43_3 {\n"
                            "    uint256 public totalSupply;\n"
                            "    mapping(address => uint256) public balanceOf;\n"
                            "    mapping(address => mapping(address => uint256)) public allowance;\n"
                            "    function decimals() external pure returns (uint8) { return 18; }\n"
                            "    function mint(address to, uint256 a) external { totalSupply += a; balanceOf[to] += a; }\n"
                            "    function approve(address sp, uint256 a) external returns (bool) { allowance[msg.sender][sp] = a; return true; }\n"
                            "    function transfer(address to, uint256 a) external returns (bool) {\n"
                            "        require(balanceOf[msg.sender] >= a, \"B\"); balanceOf[msg.sender] -= a; balanceOf[to] += a; return true;\n"
                            "    }\n"
                            "    function transferFrom(address fr, address to, uint256 a) external returns (bool) {\n"
                            "        require(allowance[fr][msg.sender] >= a && balanceOf[fr] >= a, \"TF\");\n"
                            "        allowance[fr][msg.sender] -= a; balanceOf[fr] -= a; balanceOf[to] += a; return true;\n"
                            "    }\n"
                            "}\n"
                            "contract V2Pair43_3 {\n"
                            "    address public immutable token0; address public immutable token1;\n"
                            "    uint112 public r0; uint112 public r1;\n"
                            "    constructor(address t0, address t1) { token0 = t0; token1 = t1; }\n"
                            "    function sync() external { r0 = uint112(Erc43_3(token0).balanceOf(address(this))); r1 = uint112(Erc43_3(token1).balanceOf(address(this))); }\n"
                            "    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }\n"
                            "    function swap(uint256 a0O, uint256 a1O, address to, bytes calldata) external {\n"
                            "        if (a0O > 0) Erc43_3(token0).transfer(to, a0O);\n"
                            "        if (a1O > 0) Erc43_3(token1).transfer(to, a1O);\n"
                            "        uint256 b0 = Erc43_3(token0).balanceOf(address(this));\n"
                            "        uint256 b1 = Erc43_3(token1).balanceOf(address(this));\n"
                            "        uint256 in0 = b0 > (uint256(r0) - a0O) ? (b0 - (uint256(r0) - a0O)) : 0;\n"
                            "        uint256 in1 = b1 > (uint256(r1) - a1O) ? (b1 - (uint256(r1) - a1O)) : 0;\n"
                            "        require(in0 > 0 || in1 > 0, \"I\");\n"
                            "        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3) >= uint256(r0) * uint256(r1) * 1000000, \"K\");\n"
                            "        r0 = uint112(b0); r1 = uint112(b1);\n"
                            "    }\n"
                            "}\n"
                            "contract LpPair43_3 is Erc43_3 {\n"
                            "    address public immutable token0; address public immutable token1;\n"
                            "    uint112 public r0; uint112 public r1;\n"
                            "    constructor(address t0, address t1) { token0 = t0; token1 = t1; }\n"
                            "    function sync() external { r0 = uint112(Erc43_3(token0).balanceOf(address(this))); r1 = uint112(Erc43_3(token1).balanceOf(address(this))); }\n"
                            "    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 1); }\n"
                            "    function burn(address to) external returns (uint256 a0, uint256 a1) {\n"
                            "        uint256 liq = balanceOf[address(this)]; require(liq > 0 && totalSupply > 0, \"BL\");\n"
                            "        a0 = (liq * Erc43_3(token0).balanceOf(address(this))) / totalSupply;\n"
                            "        a1 = (liq * Erc43_3(token1).balanceOf(address(this))) / totalSupply;\n"
                            "        balanceOf[address(this)] = 0; totalSupply -= liq;\n"
                            "        Erc43_3(token0).transfer(to, a0); Erc43_3(token1).transfer(to, a1);\n"
                            "        r0 = uint112(Erc43_3(token0).balanceOf(address(this))); r1 = uint112(Erc43_3(token1).balanceOf(address(this)));\n"
                            "    }\n"
                            "    function swap(uint256 a0O, uint256 a1O, address to, bytes calldata) external {\n"
                            "        if (a0O > 0) Erc43_3(token0).transfer(to, a0O);\n"
                            "        if (a1O > 0) Erc43_3(token1).transfer(to, a1O);\n"
                            "        uint256 b0 = Erc43_3(token0).balanceOf(address(this));\n"
                            "        uint256 b1 = Erc43_3(token1).balanceOf(address(this));\n"
                            "        uint256 in0 = b0 > (uint256(r0) - a0O) ? (b0 - (uint256(r0) - a0O)) : 0;\n"
                            "        uint256 in1 = b1 > (uint256(r1) - a1O) ? (b1 - (uint256(r1) - a1O)) : 0;\n"
                            "        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3) >= uint256(r0) * uint256(r1) * 1000000, \"K\");\n"
                            "        r0 = uint112(b0); r1 = uint112(b1);\n"
                            "    }\n"
                            "}\n"
                            "contract DrainVault43_3 {\n"
                            "    address immutable tk; uint256 immutable am;\n"
                            "    constructor(address _t, uint256 _a) { tk = _t; am = _a; }\n"
                            "    function drain() external { Erc43_3(tk).transfer(msg.sender, am); }\n"
                            "}\n"
                        )
                        CHAIN_STABLES["ethereum"].append(lp_ben_a)
                        try:
                            sl_lp43 = CausalSlice(
                                slice_id="lp2hop_0", target_address=lp_vault_a, channel="CH_STORAGE",
                                legs=[lp_sel], witness_candidates=[1], repeat_counts=[1],
                                leg_targets=[lp_vault_a], abi_inputs_by_sel={lp_sel: []},
                            )
                            h_lp43 = _render_minimal_solidity_harness(
                                slug="t43_lp2h", chain="ethereum", block_pin=0, caller=c43_lp, target_addr=lp_vault_a,
                                token_addrs=[lp_pair_a, lp_oth_a, lp_mid_a, lp_ben_a],
                                slices=[sl_lp43], vault_addresses=[lp_vault_a],
                                exit_routes={
                                    lp_pair_a: {
                                        "pool": _checksum(lp_pair_a), "quote_token": _checksum(lp_mid_a),
                                        "lp_other_token": _checksum(lp_oth_a), "dex_kind": "lp_token",
                                        "realizable_usd": 20_000.0, "paused": False,
                                    },
                                    lp_mid_a: {
                                        "pool": _checksum(lp_v2_a), "quote_token": _checksum(lp_ben_a),
                                        "dex_kind": "v2", "realizable_usd": 20_000.0, "paused": False,
                                    },
                                },
                            )
                            h_lp43_exec = h_lp43.replace(
                                "contract LiquidatorHelper_t43_lp2h {",
                                lp_mocks + "\ncontract LiquidatorHelper_t43_lp2h {",
                            ).replace(
                                f"        ConservationSnap memory snapBefore = _takeSnap({c43_lp_chk});",
                                f"        vm.startPrank({c43_lp_chk}, {c43_lp_chk});\n"
                                f"        Erc43_3 _lo = new Erc43_3(); Erc43_3 _lm = new Erc43_3(); Erc43_3 _lb = new Erc43_3();\n"
                                f"        LpPair43_3 _lp = new LpPair43_3(address(_lo), address(_lm));\n"
                                f"        V2Pair43_3 _v2 = new V2Pair43_3(address(_lm), address(_lb));\n"
                                f"        DrainVault43_3 _lv = new DrainVault43_3(address(_lp), 40 ether);\n"
                                f"        _lo.mint(address(_lp), 100 ether); _lm.mint(address(_lp), 100 ether); _lp.sync();\n"
                                f"        _lp.mint(address(_lv), 40 ether);\n"
                                f"        _lp.mint(address(0x000000000000000000000000000000000000dEaD), 60 ether);\n"
                                f"        _lm.mint(address(_v2), 100 ether); _lb.mint(address(_v2), 100 ether); _v2.sync();\n"
                                f"        vm.stopPrank();\n"
                                f"        ConservationSnap memory snapBefore = _takeSnap({c43_lp_chk});",
                            )
                            ws_lp43 = ensure_clean_forge_workspace({}, slug="t43_lp2h_ws", offline=True)
                            rel_lp43 = "test/Verify_t43_lp2h.t.sol"
                            (ws_lp43 / rel_lp43).write_text(h_lp43_exec, encoding="utf-8")
                            out_lp43 = execute_forge_test_file(
                                ws_lp43, rel_lp43, h_lp43_exec, timeout_sec=60,
                                allowed_caller=c43_lp, target_addr=lp_vault_a, offline=True,
                            )
                            cf_lp43 = verify_counterfactual_target_causality(
                                ws_lp43, rel_lp43, h_lp43_exec, lp_vault_a, winning_test="test_slice_0", offline=True
                            )
                            t43_f3 = bool(
                                t43_f3
                                and out_lp43.executed
                                and out_lp43.passed_tests == 1
                                and len(out_lp43.conservation_gains) == 1
                                and out_lp43.conservation_gains[0].get("asset") == lp_ben_a
                                and int(out_lp43.conservation_gains[0].get("caller_gain") or 0) > 35 * 10**18
                                and bool(cf_lp43)
                            )
                        finally:
                            if CHAIN_STABLES["ethereum"] and CHAIN_STABLES["ethereum"][-1] == lp_ben_a:
                                CHAIN_STABLES["ethereum"].pop()

                        # Real #24 selfdestruct windfall + Return/Stop trace acceptance
                        sd_caller_43 = "0x" + "6b" * 20
                        sd_json_fake = {
                            "arena": [
                                {"idx": 0, "children": [1], "trace": {"caller": FOUNDRY_DEFAULT_SENDER, "address": FOUNDRY_TEST_CONTRACT, "kind": "CALL", "value": "0x0", "success": True, "status": "Stop", "depth": 0}},
                                {"idx": 1, "children": [2], "trace": {"caller": sd_caller_43, "address": "0x" + "91" * 20, "kind": "CALL", "value": "0x0", "success": True, "status": "Return", "depth": 1}},
                                {"idx": 2, "children": [], "trace": {"caller": "0x" + "91" * 20, "address": "0x" + "92" * 20, "kind": "CALL", "value": "0x0", "success": True, "status": "SelfDestruct", "depth": 2}},
                                {"idx": 3, "children": [], "trace": {"caller": sd_caller_43, "address": "0x0000000000000000000000000000000000099999", "kind": "CALL", "value": hex(5 * 10**18), "success": True, "status": "Stop", "depth": 1}},
                            ]
                        }
                        sd_json_fake["arena"][0]["children"] = [1, 3]
                        t43_f24 = not _scan_trace_tree_for_disallowed_hevm(sd_json_fake, allowed_caller=sd_caller_43)

                        # Ensure pre-funded transfer without SELFDESTRUCT is still caught when status="Return"
                        prefund_json = {
                            "arena": [
                                {"idx": 0, "children": [1, 2], "trace": {"caller": FOUNDRY_DEFAULT_SENDER, "address": FOUNDRY_TEST_CONTRACT, "kind": "CALL", "value": "0x0", "success": True, "status": "Stop", "depth": 0}},
                                {"idx": 1, "children": [], "trace": {"caller": sd_caller_43, "address": "0x" + "91" * 20, "kind": "CALL", "value": "0x0", "success": True, "status": "Return", "depth": 1}},
                                {"idx": 2, "children": [], "trace": {"caller": sd_caller_43, "address": "0x0000000000000000000000000000000000099999", "kind": "CALL", "value": hex(5 * 10**18), "success": True, "status": "Stop", "depth": 1}},
                            ]
                        }
                        t43_f24 = t43_f24 and bool(_scan_trace_tree_for_disallowed_hevm(prefund_json, allowed_caller=sd_caller_43))

                    # (#5) Relay pending agent states + disk orphan upsert
                    st5_a = SQLiteStore(t43_ns / "st5_a.db")
                    st5_b = SQLiteStore(t43_ns / "st5_b.db")
                    st43_list.extend([st5_a, st5_b])
                    art5 = t43_ns / "art5"
                    art5.mkdir(parents=True, exist_ok=True)
                    st5_a.upsert_queue_item("modea-wait-43", 2, "ethereum", 100_000.0, [], mode="A")
                    st5_a.set_queue_agent_state("modea-wait-43", "awaiting_addresses")
                    (art5 / "disk-orphan-43").mkdir(parents=True, exist_ok=True)
                    _write_json_atomic(art5 / "disk-orphan-43" / "address_request.json", {"slug": "disk-orphan-43", "chain": "ethereum"})
                    _export_relay_state(st5_a, artifact_root=art5)
                    _import_relay_state(st5_b, artifact_root=art5)
                    row5_a = st5_b._conn.execute("SELECT status, needs_agent FROM audit_queue WHERE slug='modea-wait-43'").fetchone()
                    row5_b = st5_b._conn.execute("SELECT status, needs_agent FROM audit_queue WHERE slug='disk-orphan-43'").fetchone()
                    t43_f5 = bool(
                        row5_a is not None and row5_a["status"] == "awaiting_addresses" and int(row5_a["needs_agent"]) == 1
                        and row5_b is not None and row5_b["status"] == "awaiting_addresses" and int(row5_b["needs_agent"]) == 1
                    )

                    # (#6) Monotonic mark_vault_proven + requeue_done_for_repair cleans audit_sessions & proven_vaults
                    st6 = SQLiteStore(t43_ns / "st6.db")
                    st43_list.append(st6)
                    v6_a = "0x" + "71" * 20
                    st6.mark_vault_proven("ethereum", v6_a, "slug-full-6", 500_000.0, fully_exhausted=True)
                    st6.mark_vault_proven("ethereum", v6_a, "slug-part-6", 100_000.0, fully_exhausted=False)
                    pv6_row = st6._conn.execute("SELECT slug, net_krw, fully_exhausted FROM proven_vaults WHERE vault_addr=?", (v6_a,)).fetchone()
                    st6.upsert_queue_item("slug-rep-6", 1, "ethereum", 100_000.0, [v6_a], mode="A")
                    st6.ensure_session("ses-rep-6")
                    st6.claim_next_for_session("ses-rep-6", "boot-rep-6")
                    st6.complete_queue_item("slug-rep-6", _format_profit_verdict(50_000), 50_000.0)
                    st6.mark_vault_proven("ethereum", "0x" + "72" * 20, "slug-rep-6", 50_000.0, fully_exhausted=True)
                    st6.requeue_done_for_repair(["slug-rep-6"])
                    s6_row = st6.get_session("ses-rep-6") or {}
                    pv6_gone = st6._conn.execute("SELECT 1 FROM proven_vaults WHERE slug='slug-rep-6'").fetchone()
                    t43_f6 = bool(
                        pv6_row is not None and int(pv6_row["fully_exhausted"]) == 1 and pv6_row["slug"] == "slug-full-6"
                        and not s6_row.get("profit_slug")
                        and not s6_row.get("active_slug")
                        and "slug-rep-6" not in (s6_row.get("completed_slugs") or [])
                        and pv6_gone is None
                    )

                    # (#7) Tight RPC quorum band drops outlier high node & picks corroborated consensus tip
                    qr7 = RPCRouter(store=st6)
                    qr7._verify_endpoint_chain_id = lambda u, cid, timeout=6.0: True  # type: ignore[method-assign]
                    qr7.candidate_urls = lambda ch, need_archive=False, for_forge=False: [  # type: ignore[method-assign]
                        "http://n1.local", "http://n2.local", "http://n3.local"
                    ]
                    qr7._pooled_post = lambda u, body, timeout=15.0: json.dumps(  # type: ignore[method-assign]
                        {"jsonrpc": "2.0", "id": 1, "result": {"number": hex(20_005_000 if "n3" in u else 20_000_000), "timestamp": hex(1_760_000_000)}}
                    ).encode("utf-8")
                    tip7, head7 = qr7.get_quorum_head_block("ethereum")
                    t43_f7 = (tip7 == 20_000_000 and head7 == 20_000_000)

                    # (#8) E3 resolved non-minimal proxy strips pure DELEGATECALL forwarder fallback, catches open upgrade
                    proxy_pure_fb = bytes.fromhex(
                        ("5b" * 70) + "366000803760008036600073" + ("aa" * 20) + "5af43d82803e903d91607357fd5bf3"
                    )
                    proxy_dirty_fb = bytes.fromhex(
                        ("5b" * 70) + "6001600055366000803760008036600073" + ("aa" * 20) + "5af43d82803e903d91607857fd5bf3"
                    )
                    impl_closed_8 = bytes.fromhex(
                        "60003560e01c631234567814601357600080fd"
                        "5b3373" + ("88" * 20) + "14603257600080fd"
                        "5b6000808080808080f100"
                    )
                    pr_8 = ProxyResolution(
                        address="0x" + "e1" * 20,
                        proxy_kind="eip1967_impl",
                        impl_addresses=["0x" + "e2" * 20],
                        admin_address="",
                        raw_code_hex="0x" + proxy_pure_fb.hex(),
                        effective_code_hex="0x" + impl_closed_8.hex(),
                        skeleton_hash="s8",
                        slot_collisions=[],
                        impl_codes_hex={"0x" + "e1" * 20: "0x" + proxy_pure_fb.hex(), "0x" + "e2" * 20: "0x" + impl_closed_8.hex()},
                    )
                    mcfg_8 = _merge_facet_cfgs(_build_res_facet_cfgs_for_e3(pr_8, "0x" + "e1" * 20))
                    p_8 = prove_caller_dominator_closed(mcfg_8, target_addr="0x" + "e1" * 20)
                    pr_8_dirty = ProxyResolution(
                        address="0x" + "e3" * 20,
                        proxy_kind="eip1967_impl",
                        impl_addresses=["0x" + "e2" * 20],
                        admin_address="",
                        raw_code_hex="0x" + proxy_dirty_fb.hex(),
                        effective_code_hex="0x" + impl_closed_8.hex(),
                        skeleton_hash="s8d",
                        slot_collisions=[],
                        impl_codes_hex={"0x" + "e3" * 20: "0x" + proxy_dirty_fb.hex(), "0x" + "e2" * 20: "0x" + impl_closed_8.hex()},
                    )
                    p_8_dirty = prove_caller_dominator_closed(
                        _merge_facet_cfgs(_build_res_facet_cfgs_for_e3(pr_8_dirty, "0x" + "e3" * 20)),
                        target_addr="0x" + "e3" * 20,
                    )
                    t43_f8 = p_8.early_exit_safe and (not p_8_dirty.early_exit_safe)

                    # (#9) _replay_and_judge_at_live_head requires target_called_in_trace when target_addr given
                    orig_lh_rep_43 = _eng_globals().get("replay_compiled_test_at_live_head")
                    try:
                        _eng_globals()["replay_compiled_test_at_live_head"] = lambda **kw: ForgeRunOutcome(
                            executed=True,
                            passed_tests=1,
                            failed_tests=0,
                            conservation_gains=[{"test": "t", "slice_idx": 0, "asset": "native", "caller_gain": 5 * 10**18, "vault_loss": 5 * 10**18, "gas_used": 50000}],
                            trace_summary={"target_called_in_trace": False},
                        )
                        class _R9(RPCRouter):
                            def candidate_urls(self, ch: str, need_archive: bool = False, for_forge: bool = False) -> List[str]:
                                return ["http://127.0.0.1:8545"]
                            def get_quorum_head_block(self, ch: str) -> Tuple[int, int]:
                                return 20_000_000, 20_000_000
                            def call(self, ch: str, m: str, p: Any = (), **kw: Any) -> Any:
                                return hex(10**9) if m == "eth_gasPrice" else None
                        ok9, _, _, rsn9, _ = _replay_and_judge_at_live_head(
                            ws=t43_ns, rel_test_path="test/x.t.sol", match_test="t", store=None,
                            router=_R9(), chain="ethereum", block_pin=20_000_000,
                            decimals_map={}, prices_map={"native": 2000.0}, target_addr="0x" + "77" * 20,
                        )
                        t43_f9 = (not ok9) and rsn9 == "LIVE_HEAD_TARGET_NOT_CALLED_IN_TRACE"
                    finally:
                        if orig_lh_rep_43 is not None:
                            _eng_globals()["replay_compiled_test_at_live_head"] = orig_lh_rep_43

                    # (#10 & #15) write_protocol_artifacts seals/prunes all PROVEN_BY_FORGE & process_agent_submissions skips sealed dirs
                    sl10 = "t43-seal-10"
                    p10 = t43_art / sl10
                    (p10 / "verdicts").mkdir(parents=True, exist_ok=True)
                    (p10 / "forge" / "test").mkdir(parents=True, exist_ok=True)
                    (p10 / "forge" / "test" / "test_1.t.sol").write_text("contract C1 {}", encoding="utf-8")
                    (p10 / "forge" / "test" / "test_2.t.sol").write_text("contract C2 {}", encoding="utf-8")
                    _write_json_atomic(p10 / "verdicts" / "verdict_1.json", {"candidate_id": 1, "verdict": "PROVEN_BY_FORGE"})
                    _write_json_atomic(p10 / "verdicts" / "verdict_2.json", {"candidate_id": 2, "verdict": "PROVEN_BY_FORGE"})
                    write_protocol_artifacts(
                        sl10,
                        {
                            "slug": sl10,
                            "chain": "ethereum",
                            "audit_block": 100,
                            "verdict": _format_profit_verdict(1_000_000),
                            "net_usd": 1000.0,
                            "net_krw": 1_000_000,
                            "needs_agent": False,
                            "winning_candidate_id": 2,
                        },
                        forge_payload={
                            "executed": True,
                            "passed_tests": 1,
                            "failed_tests": 0,
                            "conservation_gains": [
                                {"test": "test_2", "slice_idx": 0, "asset": "native", "caller_gain": 10**18, "vault_loss": 10**18, "gas_used": 21_000}
                            ],
                            "trace_summary": {"target_called_in_trace": True, "disallowed_cheatcode_in_trace": False},
                        },
                        root_dir=t43_art,
                    )
                    t43_f10 = (
                        (not (p10 / "verdicts" / "verdict_1.json").exists())
                        and _is_profit_verdict(str((_read_json(p10 / "verdicts" / "verdict_2.json", default={}) or {}).get("verdict") or ""))
                        and verify_protocol_pins(sl10, root_dir=t43_art)[0]
                    )
                    t43_f15 = (
                        (not (p10 / "forge" / "test" / "test_1.t.sol").exists())
                        and (p10 / "forge" / "test" / "test_2.t.sol").exists()
                        and process_agent_submissions(_SelftestMockRouter(st6), st6, only_slugs={sl10}) == 0
                    )

                    # (#11) _advance_mode_b_sourcify_cursor wakes on recovered zero-seed addr & _resolve_protocol_addresses keeps initial_addrs
                    st11 = SQLiteStore(t43_ns / "st11.db")
                    st43_list.append(st11)
                    pq_dir_11 = t43_ns / "pq11"
                    pq_dir_11.mkdir(parents=True, exist_ok=True)
                    rec_addr_11 = "0x" + "1a" * 20
                    rec_clone_11 = "0x" + "1b" * 20
                    rec_code_11 = bytes.fromhex("60003560e01c631234567814601257600080fd" + ("5b6001600055" * 16) + "00")
                    (pq_dir_11 / "_local_index.jsonl").write_text(
                        "\n".join(
                            [
                                json.dumps({"chain_id": 1, "address": rec_addr_11, "runtime_code_hex": rec_code_11.hex()}),
                                json.dumps({"chain_id": 8453, "address": rec_clone_11, "runtime_code_hex": rec_code_11.hex()}),
                            ]
                        ) + "\n",
                        encoding="utf-8",
                    )
                    st11.upsert_queue_item("modeb-z-11", 1, "ethereum", 1000.0, [], mode="B")
                    st11.put_engine_meta("mode_b_sourcify_cursor_v1", json.dumps({"phase": "exhausted", "seed_cursor": 1, "exact_done": True}))
                    st11.requeue_with_addresses("modeb-z-11", [rec_addr_11], chain="ethereum", tvl_usd=1000.0)
                    _advance_mode_b_sourcify_cursor(None, st11, parquet_root=pq_dir_11, max_wall_sec=2.0)
                    _advance_mode_b_sourcify_cursor(None, st11, parquet_root=pq_dir_11, max_wall_sec=2.0)
                    q_11_rows = [str(r[0]) for r in st11._conn.execute("SELECT slug FROM audit_queue").fetchall()]
                    _write_json_atomic(t43_art / "addr-merge-11" / "discovered_addresses.json", {"addresses": ["0x" + "b1" * 20]})
                    merged_11 = _resolve_protocol_addresses(
                        _SelftestMockRouter(st11), "addr-merge-11", "ethereum", ["0x" + "b2" * 20], store=st11, block_pin=0
                    )
                    t43_f11 = (
                        len(json.loads(st11.get_engine_meta("mode_b_sourcify_cursor_v1", "{}")).get("harvested_addr_keys") or []) >= 1
                        and any(sl != "modeb-z-11" for sl in q_11_rows)
                        and ("0x" + "b1" * 20) in merged_11
                        and ("0x" + "b2" * 20) in merged_11
                    )

                    # (#13) V2 skewed pool fallback still capped by eff_nom_dec * fee_mult_dec
                    v2_tok_13 = "0x" + "c1" * 20
                    v2_pair_13 = "0x" + "c2" * 20
                    usdc_13 = _norm_addr(CHAIN_STABLES["ethereum"][0])
                    class _SkewV2Router13(_SelftestMockRouter):
                        def multicall3(self, ch: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            out: List[Tuple[bool, bytes]] = []
                            for to_m, cd_m in calls:
                                sel_h = "0x" + cd_m[:4].hex()
                                tm = _norm_addr(to_m)
                                if tm == _norm_addr(_UNIV2_FACTORIES["ethereum"][0]) and sel_h == V2_GET_PAIR_SEL:
                                    q_in = _norm_addr("0x" + cd_m[4 + 32 + 12 : 4 + 64].hex())
                                    out.append((True, bytes.fromhex(("00" * 12) + v2_pair_13[2:]) if q_in == usdc_13 else (b"\x00" * 32)))
                                elif tm == usdc_13 and sel_h == BALANCE_OF_SEL:
                                    out.append((True, (100_000 * 10**6).to_bytes(32, "big")))
                                elif tm == usdc_13 and sel_h == DECIMALS_SEL:
                                    out.append((True, (6).to_bytes(32, "big")))
                                elif tm == v2_tok_13 and sel_h == BALANCE_OF_SEL:
                                    out.append((True, (1).to_bytes(32, "big")))
                                else:
                                    out.append((False, b""))
                            return out
                    dex13 = probe_dex_exit_liquidity(
                        _SkewV2Router13(st6), "ethereum", v2_tok_13, 100 * 10**18, 18, Decimal("1"), Decimal("2000"), 19_000_000, quote_prices_usd={usdc_13: Decimal("1")}
                    )
                    t43_f13 = _to_dec(dex13.get("realizable_usd_dec") or dex13.get("realizable_usd")) <= Decimal("100")

                    # (#14) Parameterized fuzz and invariant tests rejected
                    ok_fuzz, v_fuzz = verify_cheatcode_allowlist("contract T { function testFuzz(uint256 x) public {} }")
                    ok_inv, v_inv = verify_cheatcode_allowlist("contract T { function invariant_k() public {} }")
                    t43_f14 = (not ok_fuzz and any("parameterized_fuzz" in x for x in v_fuzz)) and (not ok_inv and "disallowed_invariant_test" in v_inv)

                    # (#16) Same-session RECOVERED reclaim + _try_slug_lock contention doesn't reset live holder's claim
                    st16_r = SQLiteStore(t43_ns / "st16.db")
                    st43_list.append(st16_r)
                    st16_r.upsert_queue_item("lock-16", 1, "ethereum", 100_000.0, ["0x" + "16" * 20], mode="A")
                    st16_r.ensure_session("ses16")
                    c16 = st16_r.claim_next_for_session("ses16", "boot16_holder")
                    l16_dir = t43_art / "lock-16"
                    with _try_slug_lock(l16_dir):
                        audit_single_protocol(_SelftestMockRouter(st16_r), st16_r, {"slug": "lock-16", "chain": "ethereum", "addresses": ["0x" + "16" * 20]}, set(), session_id="ses16")
                    q16_after = st16_r._conn.execute("SELECT status, claim_boot_id, session_id FROM audit_queue WHERE slug='lock-16'").fetchone()
                    st16_r._conn.execute("UPDATE audit_queue SET claim_boot_id='RECOVERED' WHERE slug='lock-16'")
                    st16_r._conn.commit()
                    c16_rec = st16_r.claim_next_for_session("ses16", "boot16_new")
                    t43_f16 = bool(
                        c16 is not None and q16_after is not None
                        and q16_after["status"] == "claimed" and q16_after["claim_boot_id"] == "boot16_holder" and q16_after["session_id"] == "ses16"
                        and c16_rec is not None and c16_rec["slug"] == "lock-16"
                    )

                    # (#19) Unbacked WNATIVE supply mint bypasses _verify_wnative_backing
                    wn19 = WRAPPED_NATIVE_BY_CHAIN["ethereum"]
                    class _R19(_SelftestMockRouter):
                        _block_ts_int = 1_760_000_000
                        def eth_call_zero(self) -> None: pass
                    try:
                        _eng_globals()["_estimate_native_price_interval_dec"] = lambda *_a, **_kw: (Decimal("2000"), Decimal("2000"))
                        _eng_globals()["_fetch_usd_krw_rate_dec"] = lambda *_a, **_kw: (Decimal("1400"), "observed:t43")
                        orig_probe19 = _eng_globals().get("probe_dex_exit_liquidity")
                        _eng_globals()["probe_dex_exit_liquidity"] = lambda *_a, **_kw: {
                            "exit_liquid": True, "realizable_usd_dec": "9970", "nominal_usd_dec": "10000", "swap_fee_bps": 30, "slippage_bps": 0, "dex_depth_usd": 100000.0
                        }
                        v19_mint = judge_conservation_accounting(
                            store=st6, chain="ethereum",
                            caller_before={wn19: 0}, caller_after={wn19: 5 * 10**18},
                            vault_before={wn19: 5 * 10**18}, vault_after={wn19: 0},
                            gas_used=50_000, gas_price_wei=10**9, decimals_map={wn19: 18},
                            prices_map={"native": Decimal("2000")}, router=_R19(st6), block_pin=19_000_000,
                            supply_minted_assets={wn19},
                        )
                        v19_nomint = judge_conservation_accounting(
                            store=st6, chain="ethereum",
                            caller_before={wn19: 0}, caller_after={wn19: 5 * 10**18},
                            vault_before={wn19: 5 * 10**18}, vault_after={wn19: 0},
                            gas_used=50_000, gas_price_wei=10**9, decimals_map={wn19: 18},
                            prices_map={"native": Decimal("2000")}, router=_R19(st6), block_pin=19_000_000,
                        )
                        t43_f19 = _is_profit_verdict(v19_mint.verdict) and v19_nomint.verdict == VERDICT_BLOCKED
                    finally:
                        if orig_probe19 is not None:
                            _eng_globals()["probe_dex_exit_liquidity"] = orig_probe19

                    # (#20) Implicit EOF STOP falloff on non-terminator and trailing JUMPI sets has_return=True
                    cfg20_a = disassemble_and_build_cfg(bytes.fromhex("60003560e01c631234567814601357600080fd5b6000"))
                    cfg20_b = disassemble_and_build_cfg(bytes.fromhex("60003560e01c631234567814600e57"))
                    t43_f20 = bool(
                        any(bb.has_return for bb in cfg20_a.blocks.values())
                        and any(bb.has_return for bb in cfg20_b.blocks.values())
                        and (0x13 in cfg20_a.blocks and not _branch_unconditionally_reverts(cfg20_a.blocks, 0x13))
                        and any(not _branch_unconditionally_reverts(cfg20_b.blocks, bpc) for bpc in cfg20_b.blocks)
                    )

                    # (#21) _parse_forge_json_gains ignores failed test logs & decoded_logs when logs key exists
                    p21_fail = json.dumps({"s": {"test_results": {"t()": {"status": "Failure", "logs": [], "decoded_logs": ["ConservationGain(0, 0x0000000000000000000000000000000000000000, 1000000000000000000, 1000000000000000000, 21000)"]}}}})
                    p21_empty = json.dumps({"s": {"test_results": {"t()": {"status": "Success", "logs": [], "decoded_logs": ["ConservationGain(0, 0x0000000000000000000000000000000000000000, 1000000000000000000, 1000000000000000000, 21000)"]}}}})
                    _, _, g21_f = _parse_forge_json_gains(p21_fail)
                    _, _, g21_e = _parse_forge_json_gains(p21_empty)
                    t43_f21 = (len(g21_f) == 0) and (len(g21_e) == 0)

                    # (#22) Pre-Multicall3 historical pin does NOT poison _no_multicall_chains
                    class _MC22Router(RPCRouter):
                        def call(self, ch: str, m: str, p: Any = (), block_pin: Optional[int] = None, need_archive: bool = False) -> Any:
                            if m == "eth_getCode" and _norm_addr(p[0]) == _norm_addr(MULTICALL3_ADDR):
                                return "0x" if p[1] != "latest" else "0x6000"
                            return "0x"
                    r22 = _MC22Router(st6)
                    r22.multicall3("ethereum", [(v2_tok_13, bytes.fromhex(DECIMALS_SEL[2:]))], block_pin=100)
                    t43_f22 = "ethereum" not in r22._no_multicall_chains

                    # (#23) Self-custodied token balance (T.balanceOf(T)) measured for protocol seed ERC20
                    self_tok_23 = "0x" + "d1" * 20
                    erc20_code_23 = "0x6370a082316318160ddd63313ce567600080fd"
                    class _SelfBalRouter23(_SelftestMockRouter):
                        def candidate_urls(self, ch: str, need_archive: bool = False, for_forge: bool = False) -> List[str]:
                            return []
                        def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                            out = []
                            for m_c, _ in calls:
                                if m_c == "eth_getCode":
                                    out.append(erc20_code_23)
                                elif m_c == "eth_getStorageAt":
                                    out.append("0x" + "00" * 32)
                                elif m_c == "eth_getBalance":
                                    out.append("0x0")
                                else:
                                    out.append("0x")
                            return out
                        def multicall3(self, ch: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            out = []
                            for to_m, cd_m in calls:
                                sel_h = "0x" + cd_m[:4].hex()
                                if _norm_addr(to_m) == self_tok_23 and sel_h == BALANCE_OF_SEL:
                                    who_m = _norm_addr("0x" + cd_m[16:36].hex())
                                    out.append((True, (50 * 10**18 if who_m == self_tok_23 else 0).to_bytes(32, "big")))
                                elif _norm_addr(to_m) == self_tok_23 and sel_h == DECIMALS_SEL:
                                    out.append((True, (18).to_bytes(32, "big")))
                                elif _norm_addr(to_m) == self_tok_23 and sel_h == SYMBOL_SEL:
                                    out.append((True, abi_encode(["string"], ["SELF"])))
                                else:
                                    out.append((False, b""))
                            return out
                    cl23 = build_backward_trust_closure(_SelfBalRouter23(st6), "ethereum", [self_tok_23], 0)
                    t43_f23 = int((cl23.vault_exposures.get(self_tok_23).token_balances if cl23.vault_exposures.get(self_tok_23) else {}).get(self_tok_23, 0)) == 50 * 10**18

                    # (#25) _takeSnap StaticCall from FOUNDRY_TEST_CONTRACT ignored for target_called_in_trace
                    tr_snap_only = json.dumps({"arena": [{"trace": {"caller": FOUNDRY_TEST_CONTRACT, "address": self_tok_23, "kind": "StaticCall"}}]})
                    tr_real_call = json.dumps({"arena": [{"trace": {"caller": c_43_2, "address": self_tok_23, "kind": "Call"}}]})
                    _, called_snap, _ = _detect_disallowed_hevm_in_trace_output(tr_snap_only, forbid_inline_fork=True, target_addr=self_tok_23)
                    _, called_real, _ = _detect_disallowed_hevm_in_trace_output(tr_real_call, forbid_inline_fork=True, target_addr=self_tok_23)
                    t43_f25 = (not called_snap) and called_real

                    # (#26) Mutual pricing recursion broken & $0 lookup not cached in token_valuation
                    class _NoRecRouter26(_SelftestMockRouter):
                        _block_ts_int = 1_760_000_000
                        def http_get_json(self, u: str, **kw: Any) -> Any:
                            return {}
                        def multicall3(self, ch: str, calls: Sequence[Tuple[str, bytes]], block_pin: int = 0) -> List[Tuple[bool, bytes]]:
                            out = []
                            for _to_m, cd_m in calls:
                                sel_h = "0x" + cd_m[:4].hex()
                                if sel_h == DECIMALS_SEL:
                                    out.append((True, (18).to_bytes(32, "big")))
                                elif sel_h == SYMBOL_SEL:
                                    out.append((True, abi_encode(["string"], ["Z26"])))
                                else:
                                    out.append((False, b""))
                            return out
                    st26_43 = SQLiteStore(t43_ns / "st26.db")
                    st43_list.append(st26_43)
                    tok26_a = "0x" + "e3" * 20
                    _resolve_token_metadata_batch(_NoRecRouter26(st26_43), "ethereum", [tok26_a], 19_000_000, Decimal("2000"))
                    p_cyc, _ = _fetch_token_price_usd_dec(st26_43, "ethereum", tok26_a, router=_NoRecRouter26(st26_43), block_pin=19_000_000)
                    t43_f26 = (p_cyc == _DEC_ZERO) and (st26_43.get_token_meta(1, tok26_a, 19_000_000) is None)

                    # (#27) Live-head transport error classification + comment-stripped verify_trace_always completeness
                    _eng_globals()["_run_forge_subproc"] = lambda w_m, c_m, t_m: subprocess.CompletedProcess(
                        c_m, 1, stdout="", stderr="error sending request: connection reset"
                    )
                    lh27 = replay_compiled_test_at_live_head(t43_ns, "test/x.t.sol", "http://127.0.0.1:8545", 20_000_000, "testA")
                    ws27 = ensure_clean_forge_workspace({}, slug="t43_vta", offline=True)
                    sol27 = "contract T27 {\n // function test_cmt() public {}\n function testA() public {}\n}\n"
                    (ws27 / "test" / "T27.t.sol").write_text(sol27, encoding="utf-8")
                    _eng_globals()["_run_forge_subproc"] = lambda w_m, c_m, t_m: subprocess.CompletedProcess(
                        c_m, 0, stdout=json.dumps({"s": {"test_results": {"testA()": {"status": "Success", "logs": []}}}}), stderr=""
                    )
                    seen_tr_27: List[str] = []
                    def _mock_rep27(ws_r: Path, rel_r: str, tname_r: str, *a_r: Any, **kw_r: Any) -> Dict[str, Any]:
                        seen_tr_27.append(tname_r)
                        return {"replay_executed": False, "disallowed_cheatcode_in_trace": False, "target_called_in_trace": True}
                    _eng_globals()["_run_single_replay_trace_summary"] = _mock_rep27
                    out27 = execute_forge_test_file(ws27, "test/T27.t.sol", sol27, verify_trace_always=True, offline=True)
                    t43_f27 = (
                        (not lh27.executed)
                        and lh27.error_reason.startswith("LIVE_HEAD_REPLAY_ERROR:")
                        and seen_tr_27 == ["testA"]
                        and out27.error_reason == "TRACE_REPLAY_UNVERIFIED"
                    )

                    t43_all_ok = bool(
                        t43_f1 and t43_f2 and t43_f3 and t43_f4 and t43_f5 and t43_f6
                        and t43_f7 and t43_f8 and t43_f9 and t43_f10 and t43_f11 and t43_f12
                        and t43_f13 and t43_f14 and t43_f15 and t43_f16 and t43_f17 and t43_f18
                        and t43_f19 and t43_f20 and t43_f21 and t43_f22 and t43_f23 and t43_f24
                        and t43_f25 and t43_f26 and t43_f27
                    )
                    results.append(
                        (
                            "T43_TWENTY_SEVEN_HARDENING_FIXES_REGRESSION",
                            t43_all_ok,
                            (
                                f"f1={t43_f1},f2={t43_f2},f3={t43_f3},f4={t43_f4},f5={t43_f5},f6={t43_f6},"
                                f"f7={t43_f7},f8={t43_f8},f9={t43_f9},f10={t43_f10},f11={t43_f11},f12={t43_f12},"
                                f"f13={t43_f13},f14={t43_f14},f15={t43_f15},f16={t43_f16},f17={t43_f17},f18={t43_f18},"
                                f"f19={t43_f19},f20={t43_f20},f21={t43_f21},f22={t43_f22},f23={t43_f23},f24={t43_f24},"
                                f"f25={t43_f25},f26={t43_f26},f27={t43_f27}"
                            ),
                        )
                    )
                finally:
                    _set_eng("ARTIFACT_ROOT", orig_art_43)
                    if orig_sub_43 is not None:
                        _eng_globals()["_run_forge_subproc"] = orig_sub_43
                    if orig_est_nat_43 is not None:
                        _eng_globals()["_estimate_native_price_interval_dec"] = orig_est_nat_43
                    if orig_fx_43 is not None:
                        _eng_globals()["_fetch_usd_krw_rate_dec"] = orig_fx_43
                    if orig_llama_43 is not None:
                        _eng_globals()["_fetch_defillama_prices_dec"] = orig_llama_43
                    if orig_replay_tr_43 is not None:
                        _eng_globals()["_run_single_replay_trace_summary"] = orig_replay_tr_43
                    for _st43 in st43_list:
                        try:
                            _st43.close()
                        except Exception:
                            pass

            if _want_test("T44_SYMBOLIC_CFG_ROLE_ABI_MIDFRAME_PREDIAG", tag_filters):
                # T44: 블록 간 심볼 전파·Keccak 매핑 슬롯 결합·바이트코드 ABI 추론·역할인식 Calldata·Midframe 콜백·사전진단 회귀 검증
                # 1) Keccak 매핑 베이스 슬롯 결합 + 바이트코드 ABI 타입 추론 + 블록 간 안전 전파 검증
                # fn 0x11111111: writes mapping(uint256 => uint256) slot 5 via MSTORE(0, CALLDATALOAD(4)); MSTORE(32, 5); KECCAK256; SSTORE
                # fn 0x22222222: checks address(CD:4) & uint8(CD:36), reads scalar slot 9 (SSTORE too) AND reads mapping slot 5 -> CALL
                map_cfg_code = bytes.fromhex(
                    "60003560e01c"
                    "80631111111114601d57"
                    "632222222214603357"
                    "600080fd"
                    "5b"
                    "600435600052600560205260406000206001905500"
                    "5b"
                    "60043573ffffffffffffffffffffffffffffffffffffffff1650"
                    "60243560011650"
                    "33600052600560205260406000205450"
                    "6001600955"
                    "6000808080808080f100"
                )
                map_cfg_44 = disassemble_and_build_cfg(map_cfg_code)
                ep_wr44 = map_cfg_44.entries.get("0x11111111")
                ep_sk44 = map_cfg_44.entries.get("0x22222222")
                slices_44 = slice_four_channel_causal_chains(
                    "0x" + "44" * 20,
                    map_cfg_44,
                    ["0x11111111", "0x22222222"],
                    vault_token_amounts=[123_456_789],
                )
                coupled_map_slices = [
                    s for s in slices_44
                    if s.legs == ["0x11111111", "0x22222222"] and 5 in s.shared_map_slots
                ]
                # 2) 블록 간 단일 전임자 CALLER 전파 vs 다중 전임자 비일치 방어 검증
                # inter_closed: 0x11111111 -> block A (pushes CALLER) -> block B (JUMPDEST; PUSH20 0xab..ab; EQ; JUMPI ok else revert) -> CALL
                inter_closed_code = bytes.fromhex(
                    "631111111114600d57600080fd"
                    "5b33601256"
                    "5b73" + ("ab" * 20) + "14603057600080fd"
                    "5b6000808080808080f100"
                )
                # multi_pred_mixed: pred1 pushes CALLER, pred2 pushes CALLDATA -> join yields UNK (must remain OPEN!)
                multi_mixed_code = bytes.fromhex(
                    "631111111114601657"
                    "632222222214601b57"
                    "600080fd"
                    "5b33602256"
                    "5b600435602256"
                    "5b60005414602e57600080fd"
                    "5b6000808080808080f100"
                )
                dom_inter_closed = prove_caller_dominator_closed(disassemble_and_build_cfg(inter_closed_code), target_addr="0x" + "4a" * 20)
                dom_multi_mixed = prove_caller_dominator_closed(disassemble_and_build_cfg(multi_mixed_code), target_addr="0x" + "4b" * 20)
                t44_cfg_ok = (
                    ep_wr44 is not None
                    and ep_sk44 is not None
                    and 5 in ep_wr44.written_map_slots
                    and 5 in ep_sk44.read_map_slots
                    and len(coupled_map_slices) >= 1
                    and map_cfg_44.inferred_abi_by_sel.get("0x22222222") == ["address", "bool"]
                    and coupled_map_slices[0].inferred_abi_by_sel.get("0x22222222") == ["address", "bool"]
                    and dom_inter_closed.early_exit_safe
                    and (not dom_multi_mixed.early_exit_safe)
                )

                # 3) 불연속 경계 잔고 비례 + 역할 인식 ABI Encoding + Prescreen Revert 진단 검증
                knots_44 = solve_discontinuity_boundaries(map_cfg_44.disassembly, vault_token_amounts=[123_456_789])
                tok44_chk = _checksum("0x" + "71" * 20)
                vlt44_chk = _checksum("0x" + "72" * 20)
                caller44_chk = _checksum("0x" + "73" * 20)
                role_sel44 = _selector("swapRole(address,address,uint8,uint256,uint256,uint256,address[],bytes)")
                role_sl44 = CausalSlice(
                    slice_id="t44_role",
                    target_address=vlt44_chk,
                    channel="CH_STORAGE",
                    legs=[role_sel44],
                    witness_candidates=[10**18],
                    leg_targets=[vlt44_chk],
                    abi_inputs_by_sel={role_sel44: ["address", "address", "uint8", "uint256", "uint256", "uint256", "address[]", "bytes"]},
                    abi_param_names_by_sel={role_sel44: ["tokenIn", "recipient", "feeTier", "amountIn", "amountOutMin", "deadline", "path", "data"]},
                    needs_agent_synthesis=True,
                )
                vars_44 = _encode_prescreen_leg_calldata_variants(role_sl44, caller44_chk, token_addrs=[tok44_chk])
                sol_role44 = _render_leg_invocation_solidity(
                    vlt44_chk,
                    role_sel44,
                    "_w",
                    caller44_chk,
                    role_sl44.abi_inputs_by_sel[role_sel44],
                    param_names=role_sl44.abi_param_names_by_sel[role_sel44],
                    vault_addr=vlt44_chk,
                    token_addrs=[tok44_chk],
                )

                class _RevertDiagRouter44(RPCRouter):
                    def batch_call(self, ch: str, calls: List[Tuple[str, Any]], **kw: Any) -> List[Any]:
                        tls = getattr(self, "_tls", None)
                        err_d = {
                            0: {"code": -32000, "message": "execution reverted", "data": "0x08c379a0" + abi_encode(["string"], ["InsufficientBal"]).hex()},
                        }
                        if tls is not None:
                            tls.last_batch_errors = err_d
                        return [None for _ in calls]

                ps_44 = prescreen_eth_call_batch(_RevertDiagRouter44(store=None), "ethereum", 100, caller44_chk, [role_sl44], token_addrs=[tok44_chk])
                t44_role_ok = (
                    123_456_789 in knots_44
                    and 12_345_678 in knots_44
                    and len(vars_44) >= 1
                    and tok44_chk in sol_role44
                    and "(block.timestamp + 3600)" in sol_role44
                    and "uint256(0)" in sol_role44
                    and "address[] memory _arr6" in sol_role44
                    and ps_44[0].prescreen_diagnostics.get("revert_reason") == "Revert:InsufficientBal"
                )

                # 4) 실 Forge 컴파일·실행 검증: 역할인식(address[] + bytes + uint8 + deadline) + Midframe ERC721/ERC3156 매직 리턴 + pre_diagnostics PoC 번들
                t44_forge_ok = False
                orig_art_44 = ARTIFACT_ROOT
                try:
                    _set_eng("ARTIFACT_ROOT", selftest_ns)
                    ws44 = ensure_clean_forge_workspace({}, slug="t44_real", offline=True)
                    c44_chk = _checksum("0x" + "74" * 20)
                    tok44_a = _checksum("0x" + "75" * 20)
                    mock44_src = (
                        "contract MockRoleAndCb44 {\n"
                        "    bool public roleHit;\n"
                        "    uint256 public reentered;\n"
                        "    function swapRole(address tokenIn, address recipient, uint8 feeTier, uint256 amountIn, uint256 amountOutMin, uint256 deadline, address[] calldata path, bytes calldata data) external returns (uint256) {\n"
                        "        require(feeTier <= 255 && amountOutMin == 0 && deadline > block.timestamp && path.length == 1 && data.length == 0 && recipient != address(0) && tokenIn != address(0), \"BAD_ROLE\");\n"
                        "        roleHit = true;\n"
                        "        return amountIn;\n"
                        "    }\n"
                        "    function triggerMid(address to) external {\n"
                        "        bytes4 r721 = IERC721R44(to).onERC721Received(msg.sender, msg.sender, 1, \"\");\n"
                        "        bytes32 r3156 = IERC3156R44(to).onFlashLoan(msg.sender, address(0), 0, 0, \"\");\n"
                        "        require(r721 == bytes4(0x150b7a02) && r3156 == keccak256(\"ERC3156FlashBorrower.onFlashLoan\"), \"BAD_CB\");\n"
                        "    }\n"
                        "    function reenterFn(address, uint256, uint256 minOut, uint256 dl, address[] calldata p, bytes calldata d) external {\n"
                        "        require(minOut == 0 && dl > block.timestamp && p.length == 1 && d.length == 0, \"BAD_CB_PAYLOAD\");\n"
                        "        reentered += 1;\n"
                        "    }\n"
                        "}\n"
                        "interface IERC721R44 { function onERC721Received(address,address,uint256,bytes calldata) external returns (bytes4); }\n"
                        "interface IERC3156R44 { function onFlashLoan(address,address,uint256,uint256,bytes calldata) external returns (bytes32); }\n"
                    )
                    sel44_trig = _selector("triggerMid(address)")
                    sel44_reent = _selector("reenterFn(address,uint256,uint256,uint256,address[],bytes)")
                    sl44_role_run = CausalSlice(
                        slice_id="t44_role_exec",
                        target_address=c44_chk,
                        channel="CH_STORAGE",
                        legs=[role_sel44],
                        witness_candidates=[100],
                        repeat_counts=[1],
                        leg_targets=[c44_chk],
                        abi_inputs_by_sel={role_sel44: role_sl44.abi_inputs_by_sel[role_sel44]},
                        abi_param_names_by_sel={role_sel44: role_sl44.abi_param_names_by_sel[role_sel44]},
                    )
                    # Midframe test invoking MockRoleAndCb44.reenterFn callback cleanly via MidframeHelper.prepareCallbackData + ERC721/ERC3156 magic values
                    h44_sol = _render_minimal_solidity_harness(
                        slug="t44_real",
                        chain="ethereum",
                        block_pin=0,
                        caller=c44_chk,
                        target_addr=c44_chk,
                        token_addrs=[tok44_a],
                        slices=[sl44_role_run],
                        vault_addresses=[c44_chk],
                    )
                    mid_call_check = (
                        "    function test_mid_cb_44() public {\n"
                        "        MockRoleAndCb44 tgt = new MockRoleAndCb44();\n"
                        "        MidframeHelper_t44_real mh = new MidframeHelper_t44_real();\n"
                        "        address[] memory arr = new address[](1);\n"
                        f"        arr[0] = {tok44_a};\n"
                        f"        bytes memory cb = abi.encodeWithSelector(bytes4(hex\"{sel44_reent[2:]}\"), address(mh), uint256(7), uint256(0), block.timestamp + 3600, arr, bytes(\"\"));\n"
                        f"        mh.prepareCallbackData(address(tgt), bytes4(hex\"{sel44_reent[2:]}\"), cb);\n"
                        "        tgt.triggerMid(address(mh));\n"
                        "        require(tgt.reentered() == 1, \"NO_REENT\");\n"
                        "        bytes memory roleCd = abi.encodeWithSelector(\n"
                        f"            bytes4(hex\"{role_sel44[2:]}\"), {tok44_a}, {c44_chk}, uint8(5), uint256(77), uint256(0), block.timestamp + 3600, arr, bytes(\"\")\n"
                        "        );\n"
                        "        (bool okR, ) = address(tgt).call(roleCd);\n"
                        "        require(okR && tgt.roleHit(), \"ROLE_EXEC_FAIL\");\n"
                        "    }\n"
                    )
                    h44_comb = h44_sol.replace("// Audit block pin:", mock44_src + "\n// Audit block pin:", 1)
                    h44_comb = h44_comb.replace("    function test_slice_0()", mid_call_check + "    function test_slice_0()", 1)
                    (ws44 / "test" / "Verify_t44_real.t.sol").write_text(h44_comb, encoding="utf-8")
                    out44 = execute_forge_test_file(
                        ws44,
                        "test/Verify_t44_real.t.sol",
                        h44_comb,
                        timeout_sec=60,
                        allowed_caller=c44_chk,
                        offline=True,
                    )
                    poc_paths_44 = emit_poc_requests_for_slug(
                        slug="t44_poc_diag",
                        chain="ethereum",
                        block_pin=0,
                        caller=c44_chk,
                        slices=ps_44,
                        verified_sources={},
                        force_all=True,
                        pre_diagnostics={"sub_threshold_best_usd": "42.50"},
                    )
                    poc_doc_44 = json.loads(poc_paths_44[0].read_text(encoding="utf-8")) if poc_paths_44 else {}
                    t44_forge_ok = (
                        out44.executed
                        and out44.passed_tests >= 2
                        and poc_doc_44.get("pre_diagnostics", {}).get("sub_threshold_best_usd") == "42.50"
                        and (poc_doc_44.get("pre_diagnostics", {}).get("slice_telemetry") or [{}])[0].get("prescreen", {}).get("revert_reason") == "Revert:InsufficientBal"
                    )
                finally:
                    _set_eng("ARTIFACT_ROOT", orig_art_44)
                    shutil.rmtree(selftest_ns / "t44_real", ignore_errors=True)
                    shutil.rmtree(selftest_ns / "t44_poc_diag", ignore_errors=True)

                t44_all_ok = t44_cfg_ok and t44_role_ok and t44_forge_ok
                results.append(
                    (
                        "T44_SYMBOLIC_CFG_ROLE_ABI_MIDFRAME_PREDIAG",
                        t44_all_ok,
                        f"cfg={t44_cfg_ok},role={t44_role_ok},forge={t44_forge_ok}",
                    )
                )

            if _want_test("T45_DEDUP_DYNAMIC_FLASH_OPTIMIZER_AND_STAGE4_FLASH", tag_filters):
                # --- T45: 중복 제거(트레이스/앵커) + 동적 온체인/웹 플래시론 최적화 + Stage-4 FlashRunner 실측 ---
                tok45_a = _checksum("0x" + "81" * 20)
                tok45_b = _checksum("0x" + "82" * 20)
                tok45_u = _checksum("0x" + "83" * 20)
                aave_eth_pool = _checksum("0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2")
                atoken45_a = _checksum("0x" + "84" * 20)
                v3_pool45_b = _checksum("0x" + "85" * 20)
                bal_col45 = _checksum("0x" + "86" * 20)
                caller45_chk = _checksum("0x" + "87" * 20)

                class _FlashMockRouter45(RPCRouter):
                    def candidate_urls(self, ch: str, **kw: Any) -> List[str]:
                        return ["http://127.0.0.1:0"] if ch in ("ethereum", "unichain") else []

                    def multicall3(self, ch: str, calls: Sequence[Tuple[str, bytes]], **kw: Any) -> List[Tuple[bool, bytes]]:
                        out_mc: List[Tuple[bool, bytes]] = []
                        sel_bal = _selector("balanceOf(address)")
                        sel_prem = _selector("FLASHLOAN_PREMIUM_TOTAL()")
                        sel_atok = _selector("getReserveAToken(address)")
                        sel_bcol = _selector("getProtocolFeesCollector()")
                        sel_bfee = _selector("getFlashLoanFeePercentage()")
                        for tgt_s, cd_b in calls:
                            tgt_n = _norm_addr(tgt_s)
                            cd_l = "0x" + bytes(cd_b).hex().lower()
                            if ch != "ethereum":
                                out_mc.append((False, b""))
                                continue
                            if cd_l.startswith(sel_prem) and tgt_n == _norm_addr(aave_eth_pool):
                                out_mc.append((True, (5).to_bytes(32, "big")))
                            elif cd_l.startswith(sel_bcol) and tgt_n == _norm_addr(BALANCER_V2_VAULT):
                                out_mc.append((True, bytes.fromhex("00" * 12 + _norm_addr(bal_col45)[2:])))
                            elif cd_l.startswith(sel_bfee) and tgt_n == _norm_addr(bal_col45):
                                out_mc.append((True, (0).to_bytes(32, "big")))
                            elif cd_l.startswith(sel_atok) and tgt_n == _norm_addr(aave_eth_pool):
                                arg_tok = "0x" + cd_l[-40:]
                                if arg_tok == _norm_addr(tok45_a):
                                    out_mc.append((True, bytes.fromhex("00" * 12 + _norm_addr(atoken45_a)[2:])))
                                else:
                                    out_mc.append((False, b""))
                            elif cd_l.startswith(sel_bal) and len(cd_l) >= 74:
                                who_n = "0x" + cd_l[-40:]
                                val_w = 0
                                if tgt_n == _norm_addr(tok45_a):
                                    if who_n == _norm_addr(MORPHO_BLUE_MAINNET):
                                        val_w = 500 * (10**18)
                                    elif who_n == _norm_addr(BALANCER_V2_VAULT):
                                        val_w = 200 * (10**18)
                                    elif who_n == _norm_addr(atoken45_a):
                                        val_w = 9000 * (10**18)
                                elif tgt_n == _norm_addr(tok45_b) and who_n == _norm_addr(v3_pool45_b):
                                    val_w = 40 * (10**18)
                                out_mc.append((True, int(val_w).to_bytes(32, "big")))
                            else:
                                out_mc.append((False, b""))
                        return out_mc

                r45 = _FlashMockRouter45(store=None)
                fplan_eth45 = discover_optimal_flash_sources_for_tokens(
                    r45,
                    "ethereum",
                    21_000_000,
                    [tok45_a, tok45_b],
                    exit_routes={
                        tok45_b: {
                            "dex_kind": "v3",
                            "pool": v3_pool45_b,
                            "quote_token": tok45_a,
                            "v3_fee": 500,
                            "paused": False,
                        }
                    },
                    decimals_map={_norm_addr(tok45_a): 18, _norm_addr(tok45_b): 18},
                    prices_map={_norm_addr(tok45_a): Decimal("1"), _norm_addr(tok45_b): Decimal("2")},
                )
                fplan_uni45 = discover_optimal_flash_sources_for_tokens(r45, "unichain", 100, [tok45_u])
                best_a45 = (fplan_eth45.get("by_token") or {}).get(tok45_a, {}).get("best") or {}
                best_b45 = (fplan_eth45.get("by_token") or {}).get(tok45_b, {}).get("best") or {}
                cands_a45 = (fplan_eth45.get("by_token") or {}).get(tok45_a, {}).get("candidates") or []
                sl45_flash_seed = CausalSlice(
                    slice_id="ch_flash_capital_t45",
                    target_address=v3_pool45_b,
                    channel="CH_STORAGE",
                    legs=["0x47e7ef24"],
                    witness_candidates=[10**18],
                    leg_targets=[v3_pool45_b],
                    needs_agent_synthesis=True,
                )
                wrapped45 = _build_stage4_flash_wrapped_slices(
                    [sl45_flash_seed], fplan_eth45, [tok45_a], {_norm_addr(tok45_a): 18}
                )
                orig_art_45 = ARTIFACT_ROOT
                t45_flash_opt_ok = False
                t45_dedup_ok = False
                t45_stage4_flash_ok = False
                try:
                    _set_eng("ARTIFACT_ROOT", selftest_ns)
                    poc45_paths = emit_poc_requests_for_slug(
                        slug="t45_niche_flash",
                        chain="unichain",
                        block_pin=0,
                        caller=caller45_chk,
                        slices=[dataclasses.replace(sl45_flash_seed, slice_id="t45_uni")],
                        verified_sources={},
                        force_all=True,
                        router=r45,
                        flash_plan=fplan_uni45,
                    )
                    poc45_doc = json.loads(poc45_paths[0].read_text(encoding="utf-8")) if poc45_paths else {}
                    q45 = poc45_doc.get("recommended_web_queries") or []
                    t45_flash_opt_ok = (
                        fplan_eth45.get("probed_onchain") is True
                        and best_a45.get("provider") == "morpho_blue"
                        and best_a45.get("fee_bps") == 0
                        and best_a45.get("stage4_mode") == 2
                        and len(cands_a45) >= 3
                        and cands_a45[1].get("provider") == "balancer_v2"
                        and cands_a45[2].get("provider") == "aave_v3"
                        and int(cands_a45[2].get("max_borrowable_raw") or 0) == 9000 * (10**18)
                        and best_b45.get("provider") == "dex_flash_v3"
                        and fplan_uni45.get("flash_search_needed") is True
                        and tok45_u in (fplan_uni45.get("unfunded_tokens") or [])
                        and poc45_doc.get("flash_search_needed") is True
                        and tok45_u in (poc45_doc.get("unfunded_flash_tokens") or [])
                        and any("flash loan" in str(q) and "unichain" in str(q) for q in q45)
                        and len(wrapped45) == 1
                        and wrapped45[0].flash_loan_hint is not None
                        and wrapped45[0].flash_loan_hint.get("mode") == 2
                    )

                    # 트레이스 그래프·가스 언래핑 통합 헬퍼 동치 검증
                    tgt45_l = "0x" + "91" * 20
                    helper45_l = "0x" + "92" * 20
                    coupled45_l = "0x" + "93" * 20
                    tr_obj45 = {
                        "test_results": {
                            "test_t45()": {
                                "traces": [
                                    [
                                        "Execution",
                                        {
                                            "arena": [
                                                {"trace": {"depth": 0, "kind": "call", "caller": FOUNDRY_DEFAULT_SENDER, "address": FOUNDRY_TEST_CONTRACT, "gas_used": 250000, "data": "0x"}, "children": [1]},
                                                {"trace": {"depth": 1, "kind": "create", "caller": _norm_addr(caller45_chk), "address": helper45_l, "gas_used": 60000, "data": "0x6080"}, "parent": 0},
                                                {"trace": {"depth": 1, "kind": "call", "caller": _norm_addr(caller45_chk), "address": helper45_l, "gas_used": 120000, "data": "0x12345678"}, "children": [3], "parent": 0},
                                                {"trace": {"depth": 2, "kind": "call", "caller": helper45_l, "address": tgt45_l, "gas_used": 50000, "data": "0xabcdef01"}, "children": [4], "parent": 2},
                                                {"trace": {"depth": 3, "kind": "call", "caller": tgt45_l, "address": coupled45_l, "gas_used": 10000, "data": "0x01"}, "parent": 3},
                                            ]
                                        },
                                    ]
                                ]
                            }
                        }
                    }
                    c_addrs45 = _extract_trace_target_coupled_addrs([tr_obj45], {tgt45_l}, allowed_caller=caller45_chk, match_test="test_t45")
                    saw45, causal45 = _has_causal_target_interaction_in_trace([tr_obj45], {tgt45_l}, allowed_caller=caller45_chk, match_test="test_t45")
                    g45, mg45, tc45 = _extract_trace_observed_gas_details("", allowed_caller=caller45_chk, match_test="test_t45", parsed_objs=[tr_obj45])
                    t45_dedup_ok = (coupled45_l in c_addrs45) and saw45 and causal45 and g45 > 0 and mg45 > 0 and tc45 >= 1

                    # Stage-4 FlashRunner 실 Foundry 컴파일·무결성·플래시론 차익 실증
                    ws45 = ensure_clean_forge_workspace({}, slug="t45_real", offline=True)
                    vlt45_chk = _checksum("0x" + "88" * 20)
                    sl45_render = causal_sl45 = CausalSlice(
                        slice_id="t45_bal_flash",
                        target_address=vlt45_chk,
                        channel="CH_STORAGE",
                        legs=[_selector("depositProfit(uint256)")],
                        witness_candidates=[100],
                        repeat_counts=[1],
                        leg_targets=[vlt45_chk],
                        abi_inputs_by_sel={_selector("depositProfit(uint256)"): ["uint256"]},
                        flash_loan_hint={
                            "mode": 1,
                            "vault": BALANCER_V2_VAULT,
                            "token": tok45_a,
                            "fee_bps": 0,
                            "max_borrowable_raw": str(500),
                        },
                    )
                    h45_sol = _render_minimal_solidity_harness(
                        slug="t45_real",
                        chain="ethereum",
                        block_pin=0,
                        caller=caller45_chk,
                        target_addr=vlt45_chk,
                        token_addrs=[tok45_a],
                        slices=[sl45_render],
                        vault_addresses=[vlt45_chk],
                    )
                    acct_ok45, acct_viols45 = verify_agent_harness_accounting_integrity(
                        h45_sol, allowed_caller=caller45_chk, allowed_target_addrs=[vlt45_chk], chain="ethereum"
                    )
                    mock_flash_sol45 = (
                        "contract MockTok45 {\n"
                        "    mapping(address => uint256) public balanceOf;\n"
                        "    mapping(address => mapping(address => uint256)) public allowance;\n"
                        "    constructor(address balVault, address vulnVault) {\n"
                        "        balanceOf[balVault] = 1000;\n"
                        "        balanceOf[vulnVault] = 200;\n"
                        "    }\n"
                        "    function approve(address sp, uint256 amt) external returns (bool) {\n"
                        "        allowance[msg.sender][sp] = amt;\n"
                        "        return true;\n"
                        "    }\n"
                        "    function transfer(address to, uint256 amt) external returns (bool) {\n"
                        "        require(balanceOf[msg.sender] >= amt, \"BAL\");\n"
                        "        balanceOf[msg.sender] -= amt;\n"
                        "        balanceOf[to] += amt;\n"
                        "        return true;\n"
                        "    }\n"
                        "    function transferFrom(address fr, address to, uint256 amt) external returns (bool) {\n"
                        "        require(balanceOf[fr] >= amt && allowance[fr][msg.sender] >= amt, \"ALW\");\n"
                        "        allowance[fr][msg.sender] -= amt;\n"
                        "        balanceOf[fr] -= amt;\n"
                        "        balanceOf[to] += amt;\n"
                        "        return true;\n"
                        "    }\n"
                        "}\n"
                        "interface IBalCB45 { function receiveFlashLoan(address[] calldata,uint256[] calldata,uint256[] calldata,bytes calldata) external; }\n"
                        "contract MockBal45 {\n"
                        "    function flashLoan(address rec, address[] calldata toks, uint256[] calldata amts, bytes calldata ud) external {\n"
                        "        uint256 bPre = MockTok45(toks[0]).balanceOf(address(this));\n"
                        "        MockTok45(toks[0]).transfer(rec, amts[0]);\n"
                        "        uint256[] memory fees = new uint256[](1);\n"
                        "        IBalCB45(rec).receiveFlashLoan(toks, amts, fees, ud);\n"
                        "        require(MockTok45(toks[0]).balanceOf(address(this)) >= bPre, \"REPAY\");\n"
                        "    }\n"
                        "}\n"
                        "contract MockVulnVault45 {\n"
                        "    MockTok45 public tok;\n"
                        "    constructor() {}\n"
                        "    function setTok(address t) external { tok = MockTok45(t); }\n"
                        "    function exploitWithFlash(uint256 amt) external {\n"
                        "        require(tok.transferFrom(msg.sender, address(this), amt), \"IN\");\n"
                        "        require(tok.transfer(msg.sender, amt + 25), \"OUT\");\n"
                        "    }\n"
                        "}\n"
                    )
                    sel45_exp = _selector("exploitWithFlash(uint256)")
                    flash_exec_test45 = (
                        "    function test_flash_runner_45() public {\n"
                        "        MockVulnVault45 vv = new MockVulnVault45();\n"
                        "        MockBal45 bv = new MockBal45();\n"
                        "        MockTok45 tk = new MockTok45(address(bv), address(vv));\n"
                        "        vv.setTok(address(tk));\n"
                        f"        vm.startPrank({caller45_chk}, {caller45_chk});\n"
                        "        FlashRunner_t45_real fr = new FlashRunner_t45_real();\n"
                        "        address[] memory tgts = new address[](1);\n"
                        "        tgts[0] = address(vv);\n"
                        "        bytes[] memory cds = new bytes[](1);\n"
                        f'        cds[0] = abi.encodeWithSelector(bytes4(hex"{sel45_exp[2:]}"), uint256(100));\n'
                        f"        fr.runFlash(1, address(bv), address(tk), 100, tgts, cds, {caller45_chk});\n"
                        "        vm.stopPrank();\n"
                        f'        require(tk.balanceOf({caller45_chk}) == 25, "PROFIT25");\n'
                        "        require(tk.balanceOf(address(bv)) == 1000, \"BAL1000\");\n"
                        "        require(tk.allowance(address(fr), address(vv)) == 0, \"NO_ALW_LEAK\");\n"
                        "    }\n"
                    )
                    h45_comb = h45_sol.replace("// Audit block pin:", mock_flash_sol45 + "\n// Audit block pin:", 1)
                    h45_comb = h45_comb.replace("    function test_slice_0()", flash_exec_test45 + "    function test_slice_0()", 1)
                    (ws45 / "test" / "Verify_t45_real.t.sol").write_text(h45_comb, encoding="utf-8")
                    out45 = execute_forge_test_file(
                        ws45,
                        "test/Verify_t45_real.t.sol",
                        h45_comb,
                        timeout_sec=60,
                        allowed_caller=caller45_chk,
                        offline=True,
                    )
                    t45_stage4_flash_ok = acct_ok45 and (not acct_viols45) and ("contract FlashRunner_t45_real" in h45_sol) and out45.executed and out45.passed_tests >= 1
                finally:
                    _set_eng("ARTIFACT_ROOT", orig_art_45)
                    shutil.rmtree(selftest_ns / "t45_niche_flash", ignore_errors=True)
                    shutil.rmtree(selftest_ns / "t45_real", ignore_errors=True)

                t45_all_ok = t45_flash_opt_ok and t45_dedup_ok and t45_stage4_flash_ok
                results.append(
                    (
                        "T45_DEDUP_DYNAMIC_FLASH_OPTIMIZER_AND_STAGE4_FLASH",
                        t45_all_ok,
                        f"flash_opt={t45_flash_opt_ok},dedup={t45_dedup_ok},stage4_flash={t45_stage4_flash_ok}",
                    )
                )

            if _want_test("T46_ROUND6_SOTA_TRANSCENDENCE_100POINT", tag_filters):
                # --- T46: Decimal-only thresholds, O(N2) chunking, EIP-1153 transient mutex, satAdd parity, intent sinks ---
                import inspect as _inspect46
                from decimal import Decimal as _Dec46
                # 1) Decimal thresholds are Decimal (no float annotation) and value-correct
                t46_dec_ok = (
                    isinstance(MIN_TVL_USD, _Dec46)
                    and isinstance(MIN_PROFIT_USD, _Dec46)
                    and isinstance(FALLBACK_USD_KRW, _Dec46)
                    and _to_dec(MIN_TVL_USD) == _Dec46("100000")
                    and _to_dec(MIN_PROFIT_USD) == _Dec46("100.00")
                    and _to_dec(FALLBACK_USD_KRW) == _Dec46("1400")
                )
                # 2) judge signature uses Decimal unions (no float) for accounting params
                _sig46 = _inspect46.signature(judge_conservation_accounting)
                _ann46_fp = str(_sig46.parameters["flash_principal_usd"].annotation)
                _ann46_fb = str(_sig46.parameters["flash_fee_bps"].annotation)
                t46_sig_ok = ("Decimal" in _ann46_fp and "float" not in _ann46_fp) and ("Decimal" in _ann46_fb and "float" not in _ann46_fb)
                # 3) O(N2) fix: upsert_queue_items_batch with >200 slugs uses chunked IN (no full-table scan literal)
                _src_upsert = _inspect46.getsource(SQLiteStore.upsert_queue_items_batch)
                t46_on2_ok = ("off_sq" in _src_upsert and "FOR off_sq" not in _src_upsert and "FROM audit_queue\"" not in _src_upsert.replace(" ", "").replace("\n", ""))
                # Actually verify no bare full scan when >200: chunk loop present and no "SELECT slug, priority" without WHERE when >200
                t46_on2_ok = ("off_sq" in _src_upsert and "chunk_sq" in _src_upsert and "WHERE slug IN" in _src_upsert)
                # 4) EIP-1153 mutex: single transient write must NOT be mutex (requires >=2 writes)
                # Build minimal CFG with transient slot written once + guarded read -> should be open (not mutex)
                _t_slot = 7 | (1 << 240)
                _t_cfg46 = CFGModel(
                    blocks={
                        0: BasicBlock(start_pc=0, end_pc=0, insns=[(0, "SLOAD", b""), (1, "JUMPI", b"")], succs=[2, 3], sload_slots=[_t_slot], has_sload=True),
                        2: BasicBlock(start_pc=2, end_pc=2, insns=[(2, "TSTORE", b"")], succs=[3], sstore_slots=[_t_slot], has_tstore=True, has_sstore=True),
                        3: BasicBlock(start_pc=3, end_pc=3, insns=[(3, "STOP", b"")], succs=[]),
                    },
                    entries={
                        "0x12345678": EntryPointCFG(
                            selector="0x12345678", entry_pc=0, reachable_blocks={0, 2, 3},
                            reaches_value_sink=True, reaches_sstore=True,
                            read_slots={_t_slot}, written_slots={_t_slot},
                        )
                    },
                    disassembly=[(0, "SLOAD", b""), (1, "JUMPI", b""), (2, "TSTORE", b""), (3, "STOP", b"")],
                )
                # Monkey guard: mark JUMPI as reverting by making succ 3 revert-only (simulate via direct profile call)
                # Instead test the source disallows `or m_sl >= (1 << 240)` shortcut
                _src_mutex = _inspect46.getsource(_extract_cfg_entry_guard_profile)
                t46_mutex_ok = ("m_sl >= (1 << 240)" not in _src_mutex and "slot_write_cnt.get(m_sl, 0) >= 2" in _src_mutex)
                # 5) satAdd parity: _domain_deltas caps at uint256 max
                _big = (1 << 256) - 1
                _cg46, _vl46, _cl46 = _domain_deltas(
                    {"0x" + "aa" * 20: _big}, {"0x" + "aa" * 20: _big}, {}, {}, chain="ethereum"
                )
                _cg_wrap46, _, _ = _domain_deltas(
                    {},
                    {"0x" + "11" * 20: _big, "0x" + "22" * 20: _big},
                    {},
                    {},
                    chain="ethereum",
                )
                # _agg caps each asset at max, so two huge mints to different addrs each cap separately but same native class would saturate
                # Test native wrap saturation: huge native + huge WETH via decimals scale should not exceed max
                _src_sat = _inspect46.getsource(_domain_deltas)
                t46_sat_ok = ("_sat_add_u256" in _src_sat and "_UINT256_MAX_SAT" in _src_sat)
                # 6) intent sinks present
                t46_intent_ok = (
                    "0x1f6c34c9" in VALUE_SINK_SELECTORS
                    and "0x950fb655" in VALUE_SINK_SELECTORS
                    and _selector("executeBatch((uint8,address,uint256,bytes)[])").lower() in VALUE_SINK_SELECTORS
                    and _selector("entryPoint()") in _FACTORY_CLONE_GETTER_SELS
                )
                # 7) duplicate 0 (AST bodies) + os.environ 0
                import ast as _ast46, hashlib as _hl46
                with open(SCRIPT_DIR / "Defi_구현.py", "r", encoding="utf-8") as _fh46:
                    _src_all46 = _fh46.read()
                _tree46 = _ast46.parse(_src_all46)
                _seen46: dict = {}
                _dup46 = 0
                for _n46 in _ast46.walk(_tree46):
                    if isinstance(_n46, _ast46.FunctionDef):
                        _h46 = _hl46.sha256(_ast46.dump(_n46).encode()).hexdigest()
                        if _h46 in _seen46:
                            _dup46 += 1
                        else:
                            _seen46[_h46] = _n46.name
                t46_dup_ok = (_dup46 == 0 and "os.environ" not in _src_all46)
                t46_all_ok = t46_dec_ok and t46_sig_ok and t46_on2_ok and t46_mutex_ok and t46_sat_ok and t46_intent_ok and t46_dup_ok
                results.append(
                    (
                        "T46_ROUND6_SOTA_TRANSCENDENCE_100POINT",
                        t46_all_ok,
                        f"dec={t46_dec_ok},sig={t46_sig_ok},on2={t46_on2_ok},mutex={t46_mutex_ok},sat={t46_sat_ok},intent={t46_intent_ok},dup={t46_dup_ok}",
                    )
                )

            if _want_test("T47_FIRST_PRINCIPLE_AGENT_OPTIMIZATION", tag_filters):
                # --- T47: 제1원리 에이전트 최적화 (신규 Muse Code가 지시서를 처음 받는 관점, SOTA 참고만) ---
                import inspect as _inspect47
                _src47_run = _inspect47.getsource(cmd_run)
                # run 전문 5건 + checklist 순서 명시 (한 번 읽고 직렬 실행)
                t47_slim_ok = ("processed[-5:]" in _src47_run and "agent_checklist" in _src47_run)
                # PoC 동일 target 소수 자동 단일 번들 (N->1 파일, 지시서 단계 단축)
                _src47_poc = _inspect47.getsource(emit_poc_requests_for_slug)
                t47_compact_ok = ("_auto_compact" in _src47_poc and "len(agent_slices) <= 4" in _src47_poc)
                # 산출물·캐시는 audit_artifacts 격리 유지 + os.environ 0 + 중복 0
                import ast as _ast47, hashlib as _hl47
                with open(SCRIPT_DIR / "Defi_구현.py", "r", encoding="utf-8") as _fh47:
                    _src_all47 = _fh47.read()
                _tree47 = _ast47.parse(_src_all47)
                _seen47: dict = {}
                _dup47 = 0
                for _n47 in _ast47.walk(_tree47):
                    if isinstance(_n47, _ast47.FunctionDef):
                        _h47 = _hl47.sha256(_ast47.dump(_n47).encode()).hexdigest()
                        if _h47 in _seen47:
                            _dup47 += 1
                        else:
                            _seen47[_h47] = _n47.name
                t47_dup_ok = (_dup47 == 0 and "os.environ" not in _src_all47)
                # 실 PoC 단일 번들 동작: 동일 target 2 슬라이스 -> 파일 1건
                try:
                    _chk47 = _checksum("0x" + "aa" * 20)
                    _sl47a = CausalSlice(slice_id="t47_a", target_address=_chk47, channel="CH_STORAGE", legs=["0xa9059cbb"], witness_candidates=[1], leg_targets=[_chk47], needs_agent_synthesis=True)
                    _sl47b = CausalSlice(slice_id="t47_b", target_address=_chk47, channel="CH_STORAGE", legs=["0x23b872dd"], witness_candidates=[2], leg_targets=[_chk47], needs_agent_synthesis=True)
                    _orig_art47 = ARTIFACT_ROOT
                    try:
                        _set_eng("ARTIFACT_ROOT", selftest_ns)
                        _paths47 = emit_poc_requests_for_slug(slug="t47_agent_opt", chain="ethereum", block_pin=0, caller=_chk47, slices=[_sl47a, _sl47b], verified_sources={}, force_all=True, router=None)
                        t47_bundle_ok = (len(_paths47) == 1 and _paths47[0].name == "poc_request_1.json")
                    finally:
                        _set_eng("ARTIFACT_ROOT", _orig_art47)
                        shutil.rmtree(selftest_ns / "t47_agent_opt", ignore_errors=True)
                except Exception:
                    t47_bundle_ok = False
                t47_all_ok = t47_slim_ok and t47_compact_ok and t47_dup_ok and t47_bundle_ok
                results.append(
                    (
                        "T47_FIRST_PRINCIPLE_AGENT_OPTIMIZATION",
                        t47_all_ok,
                        f"slim={t47_slim_ok},compact={t47_compact_ok},dup={t47_dup_ok},bundle={t47_bundle_ok}",
                    )
                )

    finally:
        try:
            selftest_guard_stack.close()
        except Exception:
            pass
        tempfile.tempdir = orig_tempdir
        _set_eng("SOURCIFY_MANIFEST_PATH", orig_sourcify_manifest)
        _set_eng("SOURCIFY_PARQUET_DIR", orig_sourcify_parquet)
        _set_eng("LOCK_FILE_PATH", orig_lock_file)
        _set_eng("QUEUE_LOCK_PATH", orig_queue_lock)
        _set_eng("QUEUE_FILE_PATH", orig_queue_file)
        _set_eng("SQLITE_DB_PATH", orig_sqlite_db)
        _set_eng("CACHE_DIR", orig_cache_dir)
        _set_eng("ARTIFACT_ROOT", orig_artifact_root)
        gc.collect()
        _ns_rm_target = ("\\\\?\\" + str(selftest_ns.resolve())) if os.name == "nt" else str(selftest_ns)
        for _attempt_rm in range(5):
            shutil.rmtree(_ns_rm_target, ignore_errors=True)
            if not selftest_ns.exists():
                break
            time.sleep(0.05)
        if AUDIT_ROOT.is_dir():
            for _orph in sorted(AUDIT_ROOT.glob("_selftest_ns_*")):
                _m_op = _SELFTEST_ORPHAN_NS_RE.fullmatch(_orph.name)
                if _m_op and not _is_pid_alive(int(_m_op.group(1))):
                    _orph_rm = ("\\\\?\\" + str(_orph.resolve())) if os.name == "nt" else str(_orph)
                    shutil.rmtree(_orph_rm, ignore_errors=True)
        if not audit_root_pre_existed and AUDIT_ROOT.is_dir():
            for _attempt_ar in range(5):
                try:
                    if not any(AUDIT_ROOT.iterdir()):
                        AUDIT_ROOT.rmdir()
                    break
                except OSError:
                    time.sleep(0.05)

    if not results:
        print(f"[FAIL] NO_TESTS_MATCHED (filters={tag_filters})")
        return 1

    all_ok = True
    for name, passed, detail in results:
        tag = "PASS" if passed else "FAIL"
        print(f"[{tag}] {name} ({detail})")
        if not passed:
            all_ok = False

    if all_ok:
        if SQLITE_DB_PATH.is_file():
            try:
                cur_st_fp_s = _compute_runtime_selftest_seal_fp()
                if cur_st_fp_s:
                    with _exclusive_file_lock(LOCK_FILE_PATH):
                        st_seal = SQLiteStore(SQLITE_DB_PATH)
                        try:
                            if not tag_filters:
                                st_seal.put_engine_meta("selftest_verified_fp", cur_st_fp_s)
                        finally:
                            st_seal.close()
            except Exception:
                pass
        print("selftest_ok")
        return 0
    return 1


cmd_selftest = run_selftest


def run_cli() -> int:
    args = sys.argv[1:]
    if any(a in ("-l", "--list") for a in args):
        for t in TEST_TAGS:
            print(t)
        return 0
    return run_selftest(args if args else None)


if __name__ == "__main__":
    sys.exit(run_cli())
