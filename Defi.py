#!/usr/bin/env python3
"""
DeFi.py — 파이프라인 실행기 (Phase 1~8)

Phase 1   해킹.md 파싱 → data/seeds.json
Phase 2   Sourcify Parquet 다운로드 → data/sourcify/
Phase 3   DuckDB 중복 제거 → data/sourcify.duckdb
Phase 3.5 RPC 헬스 체크 (Chainlist + SCAN_RPCS) → data/rpc_health.json
Phase 4   유사도 검색 (exact + containment ≥ 0.2) → data/candidates.parquet
Phase 5   잔액 조회 → results/funds_{cid}.json
Phase 6   PoC 요청 → results/poc_request_{cid}.json
Phase 7   Foundry 초기화
Phase 8   verdict_{cid}.json 집계 → results/final_verdicts.parquet

forge test 실행과 판정 작성은 에이전트가 직접 수행한다.
전체 워크플로우는 DeFi.md 참조.

Usage:
    python DeFi.py             # 전체 실행 (idempotent)
    python DeFi.py --reset     # 전체 초기화
    python DeFi.py --recheck-rpc  # RPC 헬스 체크만 재실행
"""
import asyncio
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple
from urllib.parse import quote

import aiohttp
import duckdb
import pandas as pd
import requests

# ═══════════════════════════════════════════════════════════════════════
# 경로
# ═══════════════════════════════════════════════════════════════════════
ROOT = Path(__file__).parent
DATA = ROOT / "data"
RESULTS = ROOT / "results"
FOUNDRY = DATA / "foundry"
DATA.mkdir(exist_ok=True)
RESULTS.mkdir(exist_ok=True)

# ═══════════════════════════════════════════════════════════════════════
# 판정 (5종, 한국어)
# ═══════════════════════════════════════════════════════════════════════
VERDICT_PROFIT = "한화 즉시 이익 실현 가능"
VERDICT_SAFE = "취약 하지 않음"
VERDICT_NOT_RUN = "미실행"
VERDICT_NO_BALANCE = "잔액 없음"
VERDICT_SCOPE_EXCLUDED = "스코프 제외"
VALID_VERDICTS = {
    VERDICT_PROFIT, VERDICT_SAFE, VERDICT_NOT_RUN,
    VERDICT_NO_BALANCE, VERDICT_SCOPE_EXCLUDED,
}

# ═══════════════════════════════════════════════════════════════════════
# API 키
# ═══════════════════════════════════════════════════════════════════════
ANKR_KEY = "f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"
TATUM_KEY = "bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"
NODEREAL_KEY = "e7ec74ea77134e99b3aca76de8e602c5"
PREMIUM_KEYS = [ANKR_KEY, TATUM_KEY, NODEREAL_KEY]


def is_premium_rpc(url: str) -> bool:
    return any(k in url for k in PREMIUM_KEYS)


# ═══════════════════════════════════════════════════════════════════════
# 체인 매핑 (별칭 → chain_id)
# ═══════════════════════════════════════════════════════════════════════
CHAIN_MAP: Dict[str, int] = {
    "ethereum": 1, "eth": 1, "mainnet": 1,
    "bsc": 56, "bnb": 56, "bnb chain": 56,
    "polygon": 137, "matic": 137,
    "arbitrum": 42161, "arb": 42161,
    "base": 8453, "optimism": 10, "op": 10,
    "avalanche": 43114, "avax": 43114,
    "polygon zkevm": 1101, "linea": 59144, "blast": 81457,
    "scroll": 534352, "zksync era": 324, "zksync-era": 324,
    "berachain": 80094, "unichain": 130, "hyperevm": 999,
    "sonic": 146, "fraxtal": 252, "celo": 42220,
    "moonbeam": 1284, "moonriver": 1285, "metis": 1088,
    "manta": 169, "mantle": 5000, "kaia": 8217, "kava": 2222,
    "harmony": 1666600000, "fuse": 122, "oasis": 42262,
    "iotex": 4689, "opbnb": 204, "boba": 288,
    "xrplevm": 1440000, "cronos": 25,
    "flow evm": 747474, "flow-evm": 747474,
    "core": 1116, "tempo": 4217, "citrea": 4114, "hemi": 43111,
    "gnosis": 100, "oasys": 248, "robinhood": 3600,
    "ink": 57073, "poly-zkevm": 1101,
}

# ═══════════════════════════════════════════════════════════════════════
# SCAN_RPCS — 시드 목록 (Phase 3.5에서 Chainlist로 확장)
# ═══════════════════════════════════════════════════════════════════════
SCAN_RPCS: Dict[int, List[str]] = {
    1: [
        "https://eth-mainnet.public.blastapi.io",
        "https://ethereum-rpc.publicnode.com",
        "https://eth.drpc.org",
        "https://rpc.mevblocker.io",
        "https://rpc.flashbots.net",
        f"https://rpc.ankr.com/eth/{ANKR_KEY}",
        f"https://ethereum-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.nodereal.io/v1/{NODEREAL_KEY}",
    ],
    56: [
        "https://bsc-rpc.publicnode.com",
        "https://bsc.drpc.org",
        "https://bsc-dataseed.binance.org",
        f"https://bsc-mainnet.nodereal.io/v1/{NODEREAL_KEY}",
        f"https://bsc-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
    ],
    8453: [
        "https://base.publicnode.com",
        "https://mainnet.base.org",
        "https://base.drpc.org",
    ],
    42161: [
        "https://arbitrum.publicnode.com",
        "https://arb1.arbitrum.io/rpc",
        "https://arbitrum.drpc.org",
    ],
    10: [
        "https://optimism.publicnode.com",
        "https://mainnet.optimism.io",
        "https://optimism.drpc.org",
    ],
    137: [
        "https://polygon-rpc.com",
        "https://polygon.drpc.org",
    ],
    43114: [
        "https://api.avax.network/ext/bc/C/rpc",
        "https://avax.drpc.org",
    ],
    59144: ["https://rpc.linea.build"],
    81457: ["https://rpc.blast.io"],
    324: ["https://mainnet.era.zksync.io"],
    80094: ["https://rpc.berachain.com"],
    130: ["https://mainnet.unichain.org"],
    5000: ["https://rpc.mantle.xyz"],
    42220: ["https://forno.celo.org"],
    100: ["https://rpc.gnosischain.com"],
    534352: ["https://rpc.scroll.io"],
    1088: ["https://andromeda.metis.io/?owner=1088"],
    1284: ["https://rpc.api.moonbeam.network"],
    1285: ["https://rpc.api.moonriver.io"],
    288: ["https://mainnet.boba.network"],
    2222: ["https://evm.kava.io"],
    1666600000: ["https://api.harmony.one"],
    122: ["https://rpc.fuse.io"],
    42262: ["https://emerald.oasis.io"],
    4689: ["https://babel-api.mainnet.iotex.io"],
    204: ["https://opbnb-mainnet-rpc.bnbchain.org"],
    169: ["https://pacific-rpc.manta.network"],
    25: ["https://evm.cronos.org"],
    747474: ["https://mainnet.evm.nodes.onflow.org"],
    1116: ["https://rpc.coredao.org"],
    8217: ["https://public-en.node.kaia.io"],
    248: ["https://rpc.mainnet.oasys.games"],
    3600: ["https://rpc.mainnet.chain.robinhood.com"],
    1440000: ["https://rpc.xrplevm.org"],
}

# Polygon zkEVM(1101)은 네트워크 종료로 제외

# ═══════════════════════════════════════════════════════════════════════
# 플래시론 프로바이더 (체인별 우선순위)
# ═══════════════════════════════════════════════════════════════════════
FLASH_LOAN_INFO: Dict[int, List[Dict[str, Any]]] = {
    1: [
        {"provider": "Balancer V2", "fee_bps": 0,
         "address": "0xBA12222222228d8Ba445958a75a0704d566BF2C8"},
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"},
        {"provider": "MakerDAO DssFlash", "fee_bps": 0,
         "address": "0x1EB4CF3A948E7D72A198fe073cCb8C7a948cD853"},
        {"provider": "Morpho Blue", "fee_bps": None,
         "address": "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb"},
    ],
    42161: [
        {"provider": "Balancer V2", "fee_bps": 0,
         "address": "0xBA12222222228d8Ba445958a75a0704d566BF2C8"},
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x794a61358D6845594F94dc1DB02A252b5b4814aD"},
    ],
    137: [
        {"provider": "Balancer V2", "fee_bps": 0,
         "address": "0xBA12222222228d8Ba445958a75a0704d566BF2C8"},
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x794a61358D6845594F94dc1DB02A252b5b4814aD"},
    ],
    10: [
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x794a61358D6845594F94dc1DB02A252b5b4814aD"},
    ],
    8453: [
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5"},
    ],
    56: [
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x6807dc923806fE8Fd134338EABCA509979a7e0cB"},
        {"provider": "PancakeSwap V3", "fee_bps": 5, "address": ""},
    ],
    43114: [
        {"provider": "Aave V3", "fee_bps": 5,
         "address": "0x794a61358D6845594F94dc1DB02A252b5b4814aD"},
    ],
}

# ═══════════════════════════════════════════════════════════════════════
# 체인별 기본 가스 가격 (gwei)
# ═══════════════════════════════════════════════════════════════════════
GAS_PRICE_GWEI: Dict[int, float] = {
    1: 20.0, 42161: 0.01, 8453: 0.005, 10: 0.001,
    137: 30.0, 56: 1.0, 43114: 25.0, 59144: 0.05,
    81457: 0.1, 324: 0.25, 80094: 1.0, 130: 0.001,
    5000: 0.02, 42220: 5.0, 100: 2.0, 534352: 0.05,
}

# ═══════════════════════════════════════════════════════════════════════
# 토큰 및 네이티브 가격 (USD 근사치)
# ═══════════════════════════════════════════════════════════════════════
TOKENS: Dict[int, List[Tuple[str, str, int, float]]] = {
    1: [
        ("USDC", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 6, 1.0),
        ("USDT", "0xdAC17F958D2ee523a2206206994597C13D831ec7", 6, 1.0),
        ("DAI", "0x6B175474E89094C44Da98b954EedeAC495271d0F", 18, 1.0),
        ("WETH", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 18, 3000.0),
    ],
    56: [
        ("USDT", "0x55d398326f99059fF775485246999027B3197955", 18, 1.0),
        ("USDC", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d", 18, 1.0),
        ("WBNB", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c", 18, 600.0),
    ],
    8453: [
        ("USDC", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 6, 1.0),
        ("WETH", "0x4200000000000000000000000000000000000006", 18, 3000.0),
    ],
    42161: [
        ("USDC", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831", 6, 1.0),
        ("USDT", "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9", 6, 1.0),
        ("WETH", "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1", 18, 3000.0),
    ],
}

NATIVE_PRICE: Dict[int, float] = {
    1: 3000.0, 56: 600.0, 137: 0.5, 42161: 3000.0,
    8453: 3000.0, 10: 3000.0, 43114: 30.0, 59144: 3000.0,
    81457: 3000.0, 324: 3000.0, 80094: 5.0, 130: 3000.0,
    5000: 0.6, 42220: 0.5, 100: 1.0, 534352: 1.0,
}

# ═══════════════════════════════════════════════════════════════════════
# 임계값
# ═══════════════════════════════════════════════════════════════════════
FUND_THRESHOLD_USD = 100
CONTAINMENT_MIN = 0.2
MIN_SIZE_RATIO = 0.3
NGRAM_SIZES = [4, 8, 16, 32]
MAX_CANDIDATES_PER_SEED = 500
RPC_HEALTH_TTL = 86400  # 24h


# ═══════════════════════════════════════════════════════════════════════
# 유틸
# ═══════════════════════════════════════════════════════════════════════
def ngrams(hex_str: str, n: int) -> Set[str]:
    if hex_str.startswith("0x"):
        hex_str = hex_str[2:]
    hex_str = hex_str.lower()
    if len(hex_str) < n:
        return set()
    return {hex_str[i:i + n] for i in range(0, len(hex_str) - n + 1, 2)}


def strip_cbor(bytecode_hex: str) -> str:
    if not bytecode_hex or not bytecode_hex.startswith("0x"):
        return bytecode_hex or ""
    try:
        b = bytes.fromhex(bytecode_hex[2:])
    except ValueError:
        return bytecode_hex
    if len(b) < 4:
        return bytecode_hex
    meta_len = int.from_bytes(b[-2:], "big")
    if meta_len + 2 > len(b) or meta_len == 0:
        return bytecode_hex
    cb = b[-2 - meta_len:-2]
    if not cb or cb[0] not in (0xa1, 0xa2, 0xa3):
        return bytecode_hex
    return "0x" + b[:-2 - meta_len].hex()


def load_rpc_health() -> Dict[int, List[Dict[str, Any]]]:
    """data/rpc_health.json 로드. 없으면 SCAN_RPCS로 폴백."""
    health_file = DATA / "rpc_health.json"
    if health_file.exists():
        try:
            data = json.loads(health_file.read_text())
            return {int(k): v for k, v in data.get("chains", {}).items()}
        except Exception:
            pass
    # 폴백: SCAN_RPCS
    return {cid: [{"url": u, "p50_ms": None, "head_lag": None, "score": 0.5}]
            for cid, urls in SCAN_RPCS.items() for u in urls}


def best_rpc(chain_id: int) -> Optional[str]:
    """체인에서 가장 좋은 RPC 1개"""
    health = load_rpc_health()
    endpoints = health.get(chain_id, [])
    if endpoints:
        sorted_eps = sorted(endpoints, key=lambda e: e.get("score", 0), reverse=True)
        return sorted_eps[0]["url"]
    # 환경변수 override
    env = os.environ.get(f"RPC_{chain_id}")
    if env:
        return env
    # SCAN_RPCS 폴백
    if SCAN_RPCS.get(chain_id):
        return SCAN_RPCS[chain_id][0]
    return None


def rpc_call(chain_id: int, method: str, params: List[Any], timeout: int = 15) -> Any:
    url = best_rpc(chain_id)
    if not url:
        return None
    try:
        r = requests.post(
            url,
            json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1},
            timeout=timeout,
        )
        if r.status_code == 200:
            data = r.json()
            if "result" in data:
                return data["result"]
    except Exception:
        pass
    return None


def erc20_balance(chain_id: int, token: str, holder: str) -> int:
    data = "0x70a08231" + holder.lower().replace("0x", "").rjust(64, "0")
    result = rpc_call(chain_id, "eth_call", [{"to": token, "data": data}, "latest"])
    if result and result != "0x":
        try:
            return int(result, 16)
        except ValueError:
            return 0
    return 0


def _awj(path, data, indent=None):
    d = os.path.dirname(os.path.abspath(path))
    fd, tmp = tempfile.mkstemp(prefix=".tmp_", dir=d)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=indent, ensure_ascii=False)
            f.flush()
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            try:
                os.remove(tmp)
            except OSError:
                pass


# ═══════════════════════════════════════════════════════════════════════
# Phase 1: 해킹.md 파싱
# ═══════════════════════════════════════════════════════════════════════
def phase_init():
    md_path = ROOT / "해킹.md"
    if not md_path.exists():
        print(f"  [FATAL] {md_path} 없음")
        sys.exit(1)

    text = md_path.read_text(encoding="utf-8")
    blocks = re.split(r"\n\*\*(?=\d{2}/\d{2})", text)
    seeds, incident_no = [], 0

    for block in blocks:
        h = re.match(r"(\d{2}/\d{2})\s*[—–-]\s*([^(\n]+)\(([^)]+)\)", block)
        if not h:
            continue
        incident_no += 1
        date, project, chain_raw = h.group(1), h.group(2).strip(), h.group(3)

        chains = [CHAIN_MAP.get(c.strip().lower())
                  for c in re.split(r"[/,]", chain_raw)]
        chains = [c for c in chains if c]
        if not chains:
            continue
        chain_id = chains[0]

        pm = re.search(r"해킹 원리\*\*:\s*(.+?)(?:\n-|\Z)", block, re.S)
        principle = pm.group(1).strip().replace("\n", " ") if pm else ""

        cm = re.search(r"취약 컨트랙트\*\*:\s*(.+?)(?:\n-|\Z)", block, re.S)
        if not cm:
            continue

        for line in cm.group(1).split("\n"):
            for m in re.finditer(r"(0x[a-fA-F0-9]{40})\s*(?:\(([^)]*)\))?", line):
                addr = m.group(1).lower()
                label_raw = (m.group(2) or "").lower()
                if any(p in label_raw for p in
                       ["attacker", "helper", "exploiter", "poisoner",
                        "fake spawner", "attack contract"]):
                    continue
                if addr == "0x" + "0" * 38 + "0802":
                    continue

                label = m.group(2) or ""
                display = f"{date} — {project} ({chain_raw})"
                if label:
                    display += f" [{label}]"

                seeds.append({
                    "incident_no": incident_no,
                    "date": date,
                    "project": project,
                    "chain_label": chain_raw,
                    "chain": chain_id,
                    "address": addr,
                    "label": label,
                    "display": display,
                    "principle": principle,
                })

    seen, uniq = set(), []
    for s in seeds:
        k = (s["incident_no"], s["address"])
        if k not in seen:
            seen.add(k)
            uniq.append(s)

    (DATA / "seeds.json").write_text(
        json.dumps(uniq, ensure_ascii=False, indent=2))
    print(f"  [OK] 씨드 {len(uniq)}개")


# ═══════════════════════════════════════════════════════════════════════
# Phase 2: Sourcify 다운로드
# ═══════════════════════════════════════════════════════════════════════
def phase_download():
    dl = DATA / "sourcify"
    dl.mkdir(exist_ok=True)
    try:
        r = requests.get("https://export.sourcify.dev/?prefix=v2/", timeout=120)
    except Exception as e:
        print(f"  [FATAL] 목록 조회 실패: {e}")
        sys.exit(1)

    keys = re.findall(r"<Key>([^<]+)</Key>", r.text)
    print(f"  [INFO] {len(keys)}개 파일 (~136GB)")

    for i, key in enumerate(keys, 1):
        local = dl / Path(key).name
        if local.exists() and local.stat().st_size > 0:
            continue
        url = f"https://export.sourcify.dev/{quote(key)}"
        subprocess.run(["curl", "-L", "-C", "-", "-sS", "-o", str(local), url],
                       check=False)
        if i % 100 == 0:
            print(f"  {i}/{len(keys)} 다운로드 완료")
    print(f"  [OK] {dl}")


# ═══════════════════════════════════════════════════════════════════════
# Phase 3: Dedup
# ═══════════════════════════════════════════════════════════════════════
def phase_dedup():
    db_path = DATA / "sourcify.duckdb"
    conn = duckdb.connect(str(db_path))

    files = sorted((DATA / "sourcify").glob("*.parquet"))
    if not files:
        print("  [FATAL] parquet 파일 없음")
        sys.exit(1)

    tables: Dict[str, List[str]] = {}
    for f in files:
        name = re.match(r"([a-z_]+?)(?:_\d+)?\.parquet", f.name)
        if name:
            tables.setdefault(name.group(1), []).append(str(f))

    print(f"  [INFO] 테이블: {list(tables.keys())}")

    cc = tables.get("compiled_contracts", [])
    cd = tables.get("contract_deployments", [])
    vc = tables.get("verified_contracts", [])
    if not (cc and cd and vc):
        print("  [FATAL] 필수 테이블 누락")
        sys.exit(1)

    print("  [INFO] unique_bytecodes 생성 중...")
    try:
        conn.execute(f"""
        CREATE OR REPLACE TABLE unique_bytecodes AS
        SELECT
          c.runtime_bytecode_hash AS bytecode_hash,
          c.runtime_bytecode AS bytecode,
          list(d.address) AS addresses,
          list(d.chain_id) AS chains,
          min(d.created_at) AS created_at
        FROM read_parquet({cc!r}) c
        JOIN read_parquet({vc!r}) v ON v.compilation_id = c.id
        JOIN read_parquet({cd!r}) d ON d.id = v.deployment_id
        WHERE c.runtime_bytecode IS NOT NULL AND c.runtime_bytecode != '0x'
        GROUP BY c.runtime_bytecode_hash, c.runtime_bytecode
        """)
        n = conn.execute("SELECT count(*) FROM unique_bytecodes").fetchone()[0]
        print(f"  [OK] unique_bytecodes: {n:,} rows")
    except Exception as e:
        print(f"  [FATAL] dedup 실패: {e}")
        sys.exit(1)

    try:
        conn.execute(f"""
        CREATE OR REPLACE TABLE compiled_source AS
        SELECT id, runtime_bytecode_hash, sources
        FROM read_parquet({cc!r})
        WHERE runtime_bytecode IS NOT NULL
        """)
        print("  [OK] compiled_source")
    except Exception as e:
        print(f"  [WARN] compiled_source: {e}")


# ═══════════════════════════════════════════════════════════════════════
# Phase 3.5: RPC 헬스 체크
# ═══════════════════════════════════════════════════════════════════════
async def _rpc_post_json(session, url, method, params, timeout=5):
    try:
        async with session.post(
            url,
            json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1},
            timeout=aiohttp.ClientTimeout(total=timeout),
        ) as r:
            if r.status == 200:
                data = await r.json(content_type=None)
                if isinstance(data, dict) and "result" in data:
                    return data["result"]
    except Exception:
        pass
    return None


async def _check_endpoint(session, url, expected_chain_id, semaphore):
    async with semaphore:
        result = {
            "url": url,
            "chain_id_match": False,
            "block_number": None,
            "latency_samples": [],
        }

        # eth_chainId
        cid_hex = await _rpc_post_json(session, url, "eth_chainId", [])
        if cid_hex:
            try:
                if int(cid_hex, 16) == expected_chain_id:
                    result["chain_id_match"] = True
            except Exception:
                pass

        # eth_blockNumber × 2
        for _ in range(2):
            t0 = time.time()
            bn = await _rpc_post_json(session, url, "eth_blockNumber", [])
            if bn:
                try:
                    result["latency_samples"].append((time.time() - t0) * 1000)
                    result["block_number"] = int(bn, 16)
                except Exception:
                    pass

        if result["latency_samples"]:
            result["p50_ms"] = sorted(result["latency_samples"])[
                len(result["latency_samples"]) // 2]
        else:
            result["p50_ms"] = None

        return result


async def _check_chain(session, chain_id, urls, semaphore):
    tasks = [_check_endpoint(session, u, chain_id, semaphore) for u in urls]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    valid = [r for r in results
             if isinstance(r, dict) and r.get("chain_id_match")]

    if not valid:
        return chain_id, []

    best_head = max(r["block_number"] for r in valid if r.get("block_number"))
    for r in valid:
        r["head_lag"] = best_head - r["block_number"] if r.get("block_number") else None

    # 필터: p50 ≤ 2000ms AND head_lag ≤ 100
    filtered = [
        r for r in valid
        if r.get("p50_ms") is not None and r["p50_ms"] <= 2000
        and r.get("head_lag") is not None and r["head_lag"] <= 100
    ]

    # 점수: p50 낮을수록, head_lag 낮을수록 높음
    for r in filtered:
        r["score"] = 1.0 / (1.0 + r["p50_ms"] / 100.0) * (1.0 - min(r["head_lag"], 100) / 200.0)

    filtered.sort(key=lambda r: r["score"], reverse=True)
    return chain_id, filtered


async def _phase_rpc_health_async():
    # 1. Chainlist 로드
    try:
        r = requests.get("https://chainid.network/chains.json", timeout=60)
        chainlist = r.json()
        print(f"  [INFO] Chainlist: {len(chainlist)}개 체인")
    except Exception as e:
        print(f"  [WARN] Chainlist 조회 실패: {e}")
        chainlist = []

    # 2. 후보 수집 (Chainlist + SCAN_RPCS)
    candidates: Dict[int, Set[str]] = {}

    for chain in chainlist:
        cid = chain.get("chainId")
        if not cid:
            continue
        for rpc in chain.get("rpc", []):
            if not isinstance(rpc, str):
                continue
            if not rpc.startswith("https://"):
                continue
            if "${" in rpc or "}" in rpc:
                continue
            candidates.setdefault(cid, set()).add(rpc)

    for cid, urls in SCAN_RPCS.items():
        for u in urls:
            if not is_premium_rpc(u):
                candidates.setdefault(cid, set()).add(u)

    # 3. 헬스 체크
    total_eps = sum(len(v) for v in candidates.values())
    print(f"  [INFO] {len(candidates)}개 체인 / {total_eps}개 엔드포인트 검사 시작")

    semaphore = asyncio.Semaphore(50)
    health: Dict[int, List[Dict[str, Any]]] = {}

    connector = aiohttp.TCPConnector(limit=200, ttl_dns_cache=3600)
    async with aiohttp.ClientSession(connector=connector) as session:
        tasks = [
            _check_chain(session, cid, list(urls), semaphore)
            for cid, urls in candidates.items()
        ]
        results = await asyncio.gather(*tasks, return_exceptions=True)

    for res in results:
        if isinstance(res, Exception):
            continue
        cid, endpoints = res
        if endpoints:
            health[cid] = endpoints

    # 4. 프리미엄 RPC 추가 (헬스 체크 생략, 최상위 점수로)
    for cid, urls in SCAN_RPCS.items():
        premium = [u for u in urls if is_premium_rpc(u)]
        if premium:
            health.setdefault(cid, [])
            for u in premium:
                health[cid].append({
                    "url": u, "p50_ms": None, "head_lag": 0,
                    "score": 1.0, "premium": True,
                })

    # 5. 저장
    payload = {
        "generated_at": time.time(),
        "chains": {str(k): v for k, v in health.items()},
    }
    _awj(str(DATA / "rpc_health.json"), payload, indent=2)

    live_chains = len(health)
    live_eps = sum(len(v) for v in health.values())
    print(f"  [OK] 생존 체인 {live_chains}개 / 생존 엔드포인트 {live_eps}개")


def phase_rpc_health(force: bool = False):
    health_file = DATA / "rpc_health.json"
    if health_file.exists() and not force:
        try:
            data = json.loads(health_file.read_text())
            age = time.time() - data.get("generated_at", 0)
            if age < RPC_HEALTH_TTL:
                chains = data.get("chains", {})
                print(f"  SKIP (캐시, {age/3600:.1f}h 경과, {len(chains)}개 체인)")
                return
        except Exception:
            pass
    asyncio.run(_phase_rpc_health_async())


# ═══════════════════════════════════════════════════════════════════════
# Phase 4: 유사도 검색
# ═══════════════════════════════════════════════════════════════════════
def phase_search():
    db = duckdb.connect(str(DATA / "sourcify.duckdb"))
    seeds = json.loads((DATA / "seeds.json").read_text())
    print(f"  [INFO] 씨드 {len(seeds)}개")

    for s in seeds:
        try:
            row = db.execute(
                "SELECT bytecode_hash, bytecode FROM unique_bytecodes "
                "WHERE list_contains(addresses, ?) LIMIT 1",
                [s["address"]],
            ).fetchone()
            s["bytecode_hash"] = row[0] if row else None
            s["bytecode"] = row[1] if row else None
        except Exception:
            s["bytecode_hash"] = None
            s["bytecode"] = None

    seeds_ok = [s for s in seeds if s.get("bytecode")]
    print(f"  [INFO] 바이트코드 보유 씨드 {len(seeds_ok)}개")

    results = []
    seed_hashes = [s["bytecode_hash"] for s in seeds_ok if s.get("bytecode_hash")]

    # ── Exact hash fast path ──
    if seed_hashes:
        print(f"  [INFO] exact hash lookup ({len(seed_hashes)} hashes)...")
        try:
            exact_df = db.execute(
                "SELECT bytecode_hash, addresses, chains FROM unique_bytecodes "
                "WHERE bytecode_hash IN (SELECT unnest(?))",
                [seed_hashes],
            ).fetchdf()
            hash_to_seeds = {}
            for s in seeds_ok:
                h = s.get("bytecode_hash")
                if h:
                    hash_to_seeds.setdefault(h, []).append(s)
            for _, row in exact_df.iterrows():
                for seed in hash_to_seeds.get(row["bytecode_hash"], []):
                    for addr, chain in zip(row["addresses"], row["chains"]):
                        results.append({
                            "seed_incident_no": seed["incident_no"],
                            "seed_date": seed["date"],
                            "seed_project": seed["project"],
                            "seed_chain_label": seed["chain_label"],
                            "seed_label": seed["label"],
                            "seed_display": seed["display"],
                            "seed_address": seed["address"],
                            "seed_principle": seed["principle"],
                            "candidate_chain": int(chain),
                            "candidate_address": addr,
                            "candidate_bytecode_hash": row["bytecode_hash"],
                            "similarity": 1.0,
                            "match_type": "exact",
                        })
            print(f"  [OK] exact 매치: {len(results)}")
        except Exception as e:
            print(f"  [WARN] exact fast path 실패: {e}")

    # ── Containment 매칭 ──
    print(f"  [INFO] containment 매칭 중...")
    for idx, seed in enumerate(seeds_ok, 1):
        seed_ng = set()
        seed_norm = strip_cbor(seed["bytecode"])
        for n in NGRAM_SIZES:
            seed_ng |= ngrams(seed_norm, n)
        if not seed_ng:
            continue

        L = len(seed["bytecode"])
        try:
            cands = db.execute(
                "SELECT bytecode_hash, bytecode, addresses, chains "
                "FROM unique_bytecodes "
                "WHERE length(bytecode) BETWEEN ? AND ? LIMIT ?",
                [int(L * MIN_SIZE_RATIO), int(L * 2.0),
                 MAX_CANDIDATES_PER_SEED * 10],
            ).fetchdf()
        except Exception:
            continue

        seed_hits = []
        for _, row in cands.iterrows():
            if row["bytecode_hash"] == seed.get("bytecode_hash"):
                continue
            cand_ng = set()
            cand_norm = strip_cbor(row["bytecode"])
            for n in NGRAM_SIZES:
                cand_ng |= ngrams(cand_norm, n)
            if not cand_ng:
                continue
            containment = len(seed_ng & cand_ng) / len(seed_ng)
            if containment >= CONTAINMENT_MIN:
                seed_hits.append((row, containment))

        seed_hits.sort(key=lambda x: -x[1])
        for row, c in seed_hits[:MAX_CANDIDATES_PER_SEED]:
            for addr, chain in zip(row["addresses"], row["chains"]):
                results.append({
                    "seed_incident_no": seed["incident_no"],
                    "seed_date": seed["date"],
                    "seed_project": seed["project"],
                    "seed_chain_label": seed["chain_label"],
                    "seed_label": seed["label"],
                    "seed_display": seed["display"],
                    "seed_address": seed["address"],
                    "seed_principle": seed["principle"],
                    "candidate_chain": int(chain),
                    "candidate_address": addr,
                    "candidate_bytecode_hash": row["bytecode_hash"],
                    "similarity": float(c),
                    "match_type": "containment",
                })

        if idx % 10 == 0:
            print(f"  씨드 {idx}/{len(seeds_ok)} | 후보: {len(results)}")

    df = pd.DataFrame(results).drop_duplicates(
        subset=["seed_incident_no", "candidate_address", "candidate_chain",
                "match_type"]
    )
    df = df.sort_values("similarity", ascending=False).reset_index(drop=True)
    df["candidate_id"] = df.index
    df.to_parquet(DATA / "candidates.parquet")
    print(f"  [OK] 후보: {len(df)}")


# ═══════════════════════════════════════════════════════════════════════
# Phase 5: 잔액 조회
# ═══════════════════════════════════════════════════════════════════════
def phase_check_funds(candidate_id: int):
    df = pd.read_parquet(DATA / "candidates.parquet")
    row = df[df["candidate_id"] == candidate_id]
    if row.empty:
        return None
    r = row.iloc[0]
    chain = int(r["candidate_chain"])
    addr = r["candidate_address"]

    bal_hex = rpc_call(chain, "eth_getBalance", [addr, "latest"])
    native_wei = int(bal_hex, 16) if bal_hex else 0
    native_price = NATIVE_PRICE.get(chain, 0)
    native_usd = native_wei / 1e18 * native_price

    erc20_holdings, erc20_usd = [], 0.0
    for symbol, token, decimals, price in TOKENS.get(chain, []):
        b = erc20_balance(chain, token, addr)
        if b > 0:
            usd = b / (10 ** decimals) * price
            erc20_holdings.append({
                "symbol": symbol, "token": token,
                "balance": str(b), "usd": usd,
            })
            erc20_usd += usd

    total_usd = native_usd + erc20_usd
    decision = "IN_SCOPE" if total_usd >= FUND_THRESHOLD_USD else "PASS"

    result = {
        "candidate_id": candidate_id,
        "candidate_chain": chain,
        "candidate_address": addr,
        "native_balance_wei": str(native_wei),
        "native_usd_approx": native_usd,
        "erc20_holdings": erc20_holdings,
        "erc20_usd_approx": erc20_usd,
        "reachable_usd_approx": total_usd,
        "decision": decision,
    }
    _awj(str(RESULTS / f"funds_{candidate_id}.json"), result, indent=2)
    return result


# ═══════════════════════════════════════════════════════════════════════
# Phase 6: PoC 요청 생성
# ═══════════════════════════════════════════════════════════════════════
def phase_poc_request(candidate_id: int):
    df = pd.read_parquet(DATA / "candidates.parquet")
    row = df[df["candidate_id"] == candidate_id]
    if row.empty:
        return None
    r = row.iloc[0]

    funds_file = RESULTS / f"funds_{candidate_id}.json"
    if not funds_file.exists():
        return None
    funds = json.loads(funds_file.read_text())
    if funds["decision"] != "IN_SCOPE":
        return None

    source = "(소스 미확인 — 에이전트가 Etherscan 등에서 조회)"
    try:
        db = duckdb.connect(str(DATA / "sourcify.duckdb"), read_only=True)
        row_src = db.execute(
            "SELECT sources FROM compiled_source WHERE runtime_bytecode_hash = ? LIMIT 1",
            [r["candidate_bytecode_hash"]],
        ).fetchone()
        if row_src and row_src[0]:
            try:
                src_obj = json.loads(row_src[0]) if isinstance(row_src[0], str) else row_src[0]
                files = src_obj.get("files", src_obj) if isinstance(src_obj, dict) else {}
                if isinstance(files, dict) and files:
                    parts = []
                    for path, content in list(files.items())[:5]:
                        c = content.get("content", content) if isinstance(content, dict) else content
                        parts.append(f"// {path}\n{c}")
                    source = "\n\n".join(parts)
            except Exception:
                pass
    except Exception:
        pass

    chain = int(r["candidate_chain"])
    flash_loans = FLASH_LOAN_INFO.get(chain, [])
    gas_price = GAS_PRICE_GWEI.get(chain, 1.0)
    rpc_url = best_rpc(chain) or ""

    # 같은 사건의 다른 seed들 (다중 컨트랙트 의존성 참고용)
    related_seeds = []
    same_incident = df[df["seed_incident_no"] == int(r["seed_incident_no"])]
    for _, sr in same_incident.iterrows():
        if sr["seed_address"] != r["seed_address"]:
            related_seeds.append({
                "address": sr["seed_address"],
                "label": sr["seed_label"],
                "display": sr["seed_display"],
            })

    request = {
        "candidate_id": candidate_id,
        "candidate_chain": chain,
        "candidate_address": r["candidate_address"],
        "seed_incident_no": int(r["seed_incident_no"]),
        "seed_date": r["seed_date"],
        "seed_project": r["seed_project"],
        "seed_chain_label": r["seed_chain_label"],
        "seed_label": r["seed_label"],
        "seed_display": r["seed_display"],
        "seed_address": r["seed_address"],
        "seed_principle": r["seed_principle"],
        "related_seeds": related_seeds,
        "similarity": float(r["similarity"]),
        "match_type": r["match_type"],
        "source": source,
        "funds": funds,
        "rpc_url": rpc_url,
        "flash_loan_options": flash_loans,
        "gas_price_gwei": gas_price,
        "test_file_target": f"data/foundry/test/test_{candidate_id}.sol",
        "verdict_file_target": f"results/verdict_{candidate_id}.json",
    }
    _awj(str(RESULTS / f"poc_request_{candidate_id}.json"), request, indent=2)
    return request


# ═══════════════════════════════════════════════════════════════════════
# Phase 7: Foundry 초기화
# ═══════════════════════════════════════════════════════════════════════
def phase_foundry_init():
    if not (FOUNDRY / "foundry.toml").exists():
        FOUNDRY.mkdir(parents=True, exist_ok=True)
        subprocess.run(["forge", "init", "--force", "--no-git"],
                       cwd=FOUNDRY, check=False, capture_output=True)
    if not (FOUNDRY / "lib" / "forge-std").exists():
        subprocess.run(["forge", "install", "foundry-rs/forge-std"],
                       cwd=FOUNDRY, check=False, capture_output=True)
    (FOUNDRY / "test").mkdir(exist_ok=True)

    toml = FOUNDRY / "foundry.toml"
    if toml.exists():
        content = toml.read_text()
        if "optimizer" not in content:
            content += "\noptimizer = true\noptimizer_runs = 200\n"
            toml.write_text(content)
    print(f"  [OK] {FOUNDRY}")


# ═══════════════════════════════════════════════════════════════════════
# Phase 8: 집계
# ═══════════════════════════════════════════════════════════════════════
def phase_report():
    df = pd.read_parquet(DATA / "candidates.parquet")
    verdicts = []
    for _, r in df.iterrows():
        cid = int(r["candidate_id"])
        funds_f = RESULTS / f"funds_{cid}.json"
        verdict_f = RESULTS / f"verdict_{cid}.json"

        net_profit_krw = None
        profit_native_unit = None
        notes = None

        if not funds_f.exists():
            verdict, evidence = VERDICT_NOT_RUN, "funds_not_computed"
        else:
            funds = json.loads(funds_f.read_text())
            if funds["decision"] != "IN_SCOPE":
                verdict, evidence = VERDICT_NO_BALANCE, "scope_pass"
            elif not verdict_f.exists():
                verdict, evidence = VERDICT_NOT_RUN, "verdict_not_written"
            else:
                try:
                    v = json.loads(verdict_f.read_text())
                except Exception:
                    v = {}
                verdict = v.get("verdict", VERDICT_NOT_RUN)
                if verdict not in VALID_VERDICTS:
                    evidence = f"invalid_verdict:{verdict}"
                    verdict = VERDICT_NOT_RUN
                else:
                    evidence = "manual"
                net_profit_krw = v.get("net_profit_krw") or v.get("profit_krw")
                profit_native_unit = v.get("profit_native_unit")
                notes = v.get("notes")

        verdicts.append({
            "candidate_id": cid,
            "seed_incident_no": int(r["seed_incident_no"]),
            "seed_date": r["seed_date"],
            "seed_project": r["seed_project"],
            "seed_chain_label": r["seed_chain_label"],
            "seed_label": r["seed_label"],
            "seed_display": r["seed_display"],
            "seed_address": r["seed_address"],
            "seed_principle": r["seed_principle"],
            "candidate_chain": int(r["candidate_chain"]),
            "candidate_address": r["candidate_address"],
            "candidate_bytecode_hash": r["candidate_bytecode_hash"],
            "similarity": float(r["similarity"]),
            "match_type": r["match_type"],
            "verdict": verdict,
            "net_profit_krw": net_profit_krw,
            "profit_native_unit": profit_native_unit,
            "notes": notes,
            "evidence_source": evidence,
        })

    out_df = pd.DataFrame(verdicts)
    out_df.to_parquet(RESULTS / "final_verdicts.parquet")
    print(out_df["verdict"].value_counts().to_string())


# ═══════════════════════════════════════════════════════════════════════
# 메인
# ═══════════════════════════════════════════════════════════════════════
def main():
    print("=" * 70)
    print("DeFi.py — EVM 취약 컨트랙트 유사본 탐색 파이프라인")
    print("=" * 70)

    print("\n[Phase 1/8] 해킹.md 파싱")
    if not (DATA / "seeds.json").exists():
        phase_init()
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 2/8] Sourcify 다운로드")
    if not (DATA / "sourcify").exists() or not any(
            (DATA / "sourcify").glob("*.parquet")):
        phase_download()
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 3/8] Dedup")
    if not (DATA / "sourcify.duckdb").exists():
        phase_dedup()
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 3.5/8] RPC 헬스 체크 (100+ 체인)")
    phase_rpc_health()

    print("\n[Phase 4/8] 유사 컨트랙트 검색")
    if not (DATA / "candidates.parquet").exists():
        phase_search()
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 5/8] 잔액 조회")
    df = pd.read_parquet(DATA / "candidates.parquet")
    pending = [int(c) for c in df["candidate_id"]
               if not (RESULTS / f"funds_{c}.json").exists()]
    if pending:
        print(f"  {len(pending)}개 후보 조회")
        for i, cid in enumerate(pending, 1):
            phase_check_funds(cid)
            if i % 50 == 0:
                print(f"    {i}/{len(pending)}")
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 6/8] PoC 요청 생성")
    in_scope = []
    for cid in df["candidate_id"]:
        f = RESULTS / f"funds_{cid}.json"
        if f.exists():
            funds = json.loads(f.read_text())
            if funds["decision"] == "IN_SCOPE":
                in_scope.append(int(cid))

    print(f"  IN_SCOPE: {len(in_scope)}개")
    pending_req = [cid for cid in in_scope
                   if not (RESULTS / f"poc_request_{cid}.json").exists()]
    for cid in pending_req:
        phase_poc_request(cid)
    if pending_req:
        print(f"  [OK] {len(pending_req)}개 PoC 요청 생성")
    else:
        print("  SKIP (이미 완료)")

    print("\n[Phase 7/8] Foundry 초기화")
    phase_foundry_init()

    print("\n[Phase 8/8] 집계")
    phase_report()

    pending_pocs = [cid for cid in in_scope
                    if not (FOUNDRY / "test" / f"test_{cid}.sol").exists()]
    pending_verdicts = [cid for cid in in_scope
                        if (FOUNDRY / "test" / f"test_{cid}.sol").exists()
                        and not (RESULTS / f"verdict_{cid}.json").exists()]

    print("\n" + "=" * 70)
    if pending_pocs or pending_verdicts:
        print(f"PENDING_POCS: {len(pending_pocs)} candidates need PoC")
        print(f"PENDING_VERDICTS: {len(pending_verdicts)} candidates need verdict")
        print("=" * 70)
        print("\n에이전트 수행 작업 (DeFi.md 8-2 참조):")
        print("  1. results/poc_request_*.json 읽기")
        print("  2. Solidity PoC 작성 → data/foundry/test/test_{cid}.sol")
        print("  3. forge test 실행")
        print("  4. 결과 확인, 실패 시 PoC 수정 반복")
        print("  5. results/verdict_{cid}.json 작성")
        print("  6. python DeFi.py 재실행")
        if pending_pocs[:5]:
            for cid in pending_pocs[:5]:
                req = json.loads(
                    (RESULTS / f"poc_request_{cid}.json").read_text())
                print(f"  - [{req.get('seed_display', '?')}] 후보 {cid}")
            if len(pending_pocs) > 5:
                print(f"  ... 외 {len(pending_pocs) - 5}개")
    else:
        print("COMPLETE — 모든 후보 처리 완료")
        print("=" * 70)
        final = pd.read_parquet(RESULTS / "final_verdicts.parquet")
        print("\n최종 판정 분포:")
        print(final["verdict"].value_counts().to_string())


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--reset", action="store_true", help="전체 초기화")
    parser.add_argument("--recheck-rpc", action="store_true",
                        help="RPC 헬스 체크만 재실행")
    args = parser.parse_args()

    if args.reset:
        for p in [DATA, RESULTS]:
            if p.exists():
                shutil.rmtree(p)
        print("초기화 완료")
        sys.exit(0)

    if args.recheck_rpc:
        phase_rpc_health(force=True)
        sys.exit(0)

    main()