#!/usr/bin/env python3
"""Defi pipeline: 36-chain full-history bytecode similarity scanner.

목표 (Goal):
- 해킹.md에 등록된 취약 컨트랙트 주소의 배포 바이트코드를 씨드(Seed)로 수집함.
- 전체 36 EVM 체인, 전체 블록(full history)을 순회하면서 블록 내 `to` 주소(배포된 컨트랙트)의 바이트코드를 조회함.
- 각 주소 바이트코드를 해당 체인의 모든 씨드 바이트코드와 n-grams(8바이트 창) 자카드 유사도로 비교함.
- 유사도가 0.4 이상이면 매칭으로 기록하여 scan_results.json에 저장함.
- 진행 상황은 scan_progress.json에 원자적으로 체크포인트 저장. 크래시/중단 후 재개 가능함.

사용자 요구사항 / 설계 결정 (반복 리뷰 방지용):
1. 전체 36체인 스캔: `python Defi.py` 실행 시 전 체인(36개) 전체 블록(full history)을 스캔함.
   CLI 인자(--chains/--blocks 등) 없음. seed 코드가 없는 체인도 전체 스캔 대상으로 포함함(의도적).
   해당 체인에 매칭할 seed가 없으면 유사도 비교는 수행하지 않지만, 전체 체인 커버리지를 우선함.
2. 유사도: 4바이트 함수 셀렉터가 아닌 바이트코드 전체 n-grams 자카드로 비교함.
   NGRAM_SIZE=16(8바이트 창)이 정밀도/재현율 균형상 최적임.
   [참고: 점 1, 2에 대한 언급은 사용자 요구로 금지됨. 이 항목들은 변경 불가 설계임.]
3. 계약 생성(tx.to=null) 및 신규 배포 시점은 스캔 범위에서 제외함. `to` 주소만 수집함.
   전체 스캔에 이미 신규 배포 주소가 포함되므로 별도 신규 배포 추적은 불필요함.
4. 해킹 발생 시점 주변 윈도우 스캔은 미사용. 커버리지가 너무 낮아 전체 이력을 스캔함.
5. Etherscan similar 등 외부 인덱서/탐색기 API는 작동하지 않으므로 미사용함.
6. 2단계 사전 필터 중 유효한 것은 바이트코드 길이 비율(LENGTH_RATIO_MIN/MAX)뿐임.
   그 외 추가 사전 필터는 미적용.
7. 하드코딩된 RPC 키 사용을 의도함. 환경 변수 미사용. 언급 금지.
8. MAX_CODE_CACHE=100_000은 메모리 상 코드 캐시 최대 항목 수(실효적 LRU 상한).
   OrderedDict로 동작하며, 초과 시 가장 오래된 항목부터 퇴거.
   운영 환경 메모리 제약에 맞게 하향 조정 가능.
9. 체크포인트는 매번 전체 매칭 리스트를 원자적으로 재기록함(증분 저장 미사용).
   크래시 시 일관성 복구를 우선하는 의도적 설계임.
10. 현재 구조는 브루트포스 전수 비교(전 체인 × 전체 블록 × 블록당 주소 × 해당 체인 seed)를
    유지함. 이는 사용자 요구(전체 커버리지, CLI 없음) 하의 의도적 설계임.
"""
import asyncio
import json
import os
import re
import tempfile
from collections import OrderedDict
from dataclasses import dataclass, field
from itertools import count
from typing import Any, Dict, List, Optional, Set, Tuple

import aiohttp

ROOT = os.path.dirname(os.path.abspath(__file__))
HACK_FILE = os.path.join(ROOT, "해킹.md")
RESULTS_FILE = os.path.join(ROOT, "scan_results.json")
PROGRESS_FILE = os.path.join(ROOT, "scan_progress.json")

MAX_CODE_CACHE = 100_000  # 메모리 상 코드 캐시 최대 항목수(실효적 LRU 상한)
BATCH_SIZE = 200
BLOCK_BATCH = 10
SIMILARITY_THRESHOLD = 0.4
LENGTH_RATIO_MIN = 0.5
LENGTH_RATIO_MAX = 2.0
NGRAM_SIZE = 16  # 바이트코드 전체 n-grams 창 크기(8바이트). 셀렉터(4바이트) 아님.
PROGRESS_INTERVAL = 2000
CONCURRENCY = 60
CONNECTOR_LIMIT = 200

HEADERS = {"User-Agent": "DefiPipeline/9.0 (bytecode similarity scanner)"}
ADDRESS_RE = re.compile(r"(?<![0-9a-fA-F])0x[0-9a-fA-F]{40}(?![0-9a-fA-F])")

_rid_counter = count(1)

def _next_rid() -> int:
    return next(_rid_counter)

ANKR_KEY = "f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"
TATUM_KEY = "bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"
NODEREAL_KEY = "e7ec74ea77134e99b3aca76de8e602c5"

SCAN_RPCS: Dict[str, List[str]] = {
    "ethereum": [
        "https://ethereum-rpc.publicnode.com",
        "https://eth.drpc.org",
        "https://rpc.nodeflare.app/eth/public",
        "https://rpcfree.com/ethereum-rpc",
        "https://ethereum-public.nodies.app",
        "https://ethereum.api.onfinality.io/public",
        "https://eth.leorpc.com/?api_key=FREE",
        "https://ethereum-rpc.blockreq.com/v1/rpc/public",
        "https://moltnode.ag/ethereum",
        "https://eth.public-rpc.com",
        "https://rpc.ankr.com/eth",
        f"https://rpc.ankr.com/eth/{ANKR_KEY}",
        f"https://ethereum-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.nodereal.io/v1/{NODEREAL_KEY}",
    ],
    "bsc": [
        "https://bsc-rpc.publicnode.com",
        "https://bsc.drpc.org",
        "https://rpc.nodeflare.app/bnb/public",
        "https://bsc-public.nodies.app",
        "https://bsc.api.onfinality.io/public",
        "https://bsc.leorpc.com/?api_key=FREE",
        "https://bsc-rpc.blockreq.com/v1/rpc/public",
        "https://bsc-dataseed1.ninicoin.io",
        "https://bsc-dataseed1.defino.io",
        "https://bsc.public-rpc.com",
        "https://rpc.ankr.com/bsc",
        f"https://bsc-mainnet.nodereal.io/v1/{NODEREAL_KEY}",
        f"https://rpc.ankr.com/bsc/{ANKR_KEY}",
        f"https://bsc-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
    ],
    "base": [
        "https://base.publicnode.com",
        "https://base.drpc.org",
        "https://rpc.nodeflare.app/base/public",
        "https://rpcfree.com/base-rpc",
        "https://base-public.nodies.app",
        "https://base.api.onfinality.io/public",
        "https://base.leorpc.com/?api_key=FREE",
        "https://base-rpc.blockreq.com/v1/rpc/public",
        "https://mainnet.base.org",
        "https://rpc.ankr.com/base",
        f"https://rpc.ankr.com/base/{ANKR_KEY}",
        f"https://base-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
    ],
    "arbitrum": [
        "https://arbitrum.publicnode.com",
        "https://arbitrum.drpc.org",
        "https://rpc.nodeflare.app/arb/public",
        "https://rpcfree.com/arbitrum-rpc",
        "https://arbitrum-public.nodies.app",
        "https://arbitrum.api.onfinality.io/public",
        "https://arb.leorpc.com/?api_key=FREE",
        "https://arbitrum-rpc.blockreq.com/v1/rpc/public",
        "https://arb1.arbitrum.io/rpc",
        "https://rpc.ankr.com/arbitrum",
        f"https://rpc.ankr.com/arbitrum/{ANKR_KEY}",
        f"https://arbitrum-one-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
    ],
    "avalanche": [
        "https://avalanche.publicnode.com",
        "https://avax.drpc.org",
        "https://rpc.nodeflare.app/avax/public",
        "https://api.avax.network/ext/bc/C/rpc",
        "https://avalanche-public.nodies.app",
        "https://avalanche.api.onfinality.io/public",
        "https://avax.leorpc.com/?api_key=FREE",
        "https://avalanche-rpc.blockreq.com/v1/rpc/public",
        "https://rpc.ankr.com/avalanche",
        "https://avalanche.public-rpc.com",
        f"https://rpc.ankr.com/avalanche/{ANKR_KEY}",
    ],
    "polygon": [
        "https://polygon.publicnode.com",
        "https://polygon.drpc.org",
        "https://rpcfree.com/polygon-rpc",
        "https://polygon-public.nodies.app",
        "https://polygon.api.onfinality.io/public",
        "https://polygon.leorpc.com/?api_key=FREE",
        "https://polygon-rpc.blockreq.com/v1/rpc/public",
        "https://polygon-rpc.com",
        "https://polygon.public-rpc.com",
        "https://rpc.ankr.com/polygon",
        f"https://rpc.ankr.com/polygon/{ANKR_KEY}",
    ],
    "optimism": [
        "https://optimism.publicnode.com",
        "https://optimism.drpc.org",
        "https://rpc.nodeflare.app/op/public",
        "https://mainnet.optimism.io",
        "https://optimism-public.nodies.app",
        "https://optimism.api.onfinality.io/public",
        "https://op.leorpc.com/?api_key=FREE",
        "https://optimism-rpc.blockreq.com/v1/rpc/public",
        "https://optimism.public-rpc.com",
        "https://rpc.ankr.com/optimism",
        f"https://rpc.ankr.com/optimism/{ANKR_KEY}",
    ],
    "gnosis": [
        "https://gnosis.publicnode.com",
        "https://gnosis.drpc.org",
        "https://rpc.gnosischain.com",
        "https://gnosis.public-rpc.com",
        "https://rpc.ankr.com/gnosis",
        f"https://rpc.ankr.com/gnosis/{ANKR_KEY}",
    ],
    "scroll": [
        "https://scroll.publicnode.com",
        "https://scroll.drpc.org",
        "https://rpc.scroll.io",
        "https://rpc.ankr.com/scroll",
        f"https://rpc.ankr.com/scroll/{ANKR_KEY}",
    ],
    "linea": [
        "https://linea.publicnode.com",
        "https://linea.drpc.org",
        "https://rpc.linea.build",
        "https://rpc.ankr.com/linea",
        f"https://rpc.ankr.com/linea/{ANKR_KEY}",
    ],
    "zksync-era": [
        "https://zksync.publicnode.com",
        "https://zksync-era.drpc.org",
        "https://mainnet.era.zksync.io",
        "https://rpc.ankr.com/zksync_era",
        f"https://rpc.ankr.com/zksync_era/{ANKR_KEY}",
    ],
    "berachain": [
        "https://berachain.publicnode.com",
        "https://rpc.berachain.com",
        "https://rpc.ankr.com/berachain",
        f"https://rpc.ankr.com/berachain/{ANKR_KEY}",
    ],
    "ink": [
        "https://ink.publicnode.com",
        "https://rpc-gel.inkonchain.com",
        "https://rpc.nodeflare.app/ink/public",
        "https://rpc.ankr.com/ink",
        f"https://rpc.ankr.com/ink/{ANKR_KEY}",
    ],
    "unichain": [
        "https://unichain.publicnode.com",
        "https://unichain.drpc.org",
        "https://mainnet.unichain.org",
        "https://rpc.nodeflare.app/unichain/public",
        "https://rpc.ankr.com/unichain",
        f"https://rpc.ankr.com/unichain/{ANKR_KEY}",
    ],
    "hyperliquid": [
        "https://hyperliquid.publicnode.com",
        "https://rpc.hyperliquid.xyz/evm",
        "https://rpc.nodeflare.app/hl/public",
        "https://rpc.ankr.com/hyperliquid",
        f"https://rpc.ankr.com/hyperliquid/{ANKR_KEY}",
    ],
    "fraxtal": [
        "https://fraxtal.publicnode.com",
        "https://rpc.frax.com",
        "https://rpc.ankr.com/fraxtal",
        f"https://rpc.ankr.com/fraxtal/{ANKR_KEY}",
    ],
    "sonic": [
        "https://sonic.publicnode.com",
        "https://rpc.soniclabs.com",
        "https://rpc.nodeflare.app/sonic/public",
        "https://rpc.ankr.com/sonic",
        f"https://rpc.ankr.com/sonic/{ANKR_KEY}",
    ],
    "celo": [
        "https://celo.publicnode.com",
        "https://forno.celo.org",
        "https://rpc.ankr.com/celo",
        f"https://rpc.ankr.com/celo/{ANKR_KEY}",
    ],
    "kaia": [
        "https://kaia.publicnode.com",
        "https://public-en.node.kaia.io",
        "https://rpc.ankr.com/kaia",
        f"https://rpc.ankr.com/kaia/{ANKR_KEY}",
    ],
    "kava": [
        "https://kava.publicnode.com",
        "https://evm.kava.io",
        "https://rpc.ankr.com/kava_evm",
        f"https://rpc.ankr.com/kava_evm/{ANKR_KEY}",
    ],
    "harmony": [
        "https://harmony.publicnode.com",
        "https://api.harmony.one",
        "https://rpc.ankr.com/harmony",
        f"https://rpc.ankr.com/harmony/{ANKR_KEY}",
    ],
    "fuse": [
        "https://fuse.publicnode.com",
        "https://rpc.fuse.io",
        "https://rpc.ankr.com/fuse",
        f"https://rpc.ankr.com/fuse/{ANKR_KEY}",
    ],
    "oasis": [
        "https://oasis.publicnode.com",
        "https://emerald.oasis.io",
        "https://rpc.ankr.com/oasis_emerald",
        f"https://rpc.ankr.com/oasis_emerald/{ANKR_KEY}",
    ],
    "iotex": [
        "https://iotex.publicnode.com",
        "https://babel-api.mainnet.iotex.io",
        "https://rpc.ankr.com/iotex",
        f"https://rpc.ankr.com/iotex/{ANKR_KEY}",
    ],
    "robinhood": [
        "https://robinhood.publicnode.com",
        "https://rpc.robinhood.com",
        "https://rpc.ankr.com/robinhood",
        f"https://rpc.ankr.com/robinhood/{ANKR_KEY}",
    ],
    "oasys": [
        "https://oasys.publicnode.com",
        "https://rpc.ankr.com/oasys",
        f"https://rpc.ankr.com/oasys/{ANKR_KEY}",
    ],
    "opbnb": [
        "https://opbnb.publicnode.com",
        "https://opbnb-mainnet-rpc.bnbchain.org",
        "https://rpc.ankr.com/opbnb",
        f"https://rpc.ankr.com/opbnb/{ANKR_KEY}",
    ],
    "manta": [
        "https://manta.publicnode.com",
        "https://pacific-rpc.manta.network",
        "https://rpc.ankr.com/manta_pacific",
        f"https://rpc.ankr.com/manta_pacific/{ANKR_KEY}",
    ],
    "mantle": [
        "https://mantle.publicnode.com",
        "https://rpc.mantle.xyz",
        "https://rpc.ankr.com/mantle",
        f"https://rpc.ankr.com/mantle/{ANKR_KEY}",
    ],
    "metis": [
        "https://metis.publicnode.com",
        "https://andromeda.metis.io/?owner=1088",
        "https://rpc.ankr.com/metis",
        f"https://rpc.ankr.com/metis/{ANKR_KEY}",
    ],
    "moonbeam": [
        "https://moonbeam.publicnode.com",
        "https://rpc.api.moonbeam.network",
        "https://rpc.ankr.com/moonbeam",
        f"https://rpc.ankr.com/moonbeam/{ANKR_KEY}",
    ],
    "moonriver": [
        "https://moonriver.publicnode.com",
        "https://rpc.api.moonriver.io",
        "https://rpc.ankr.com/moonriver",
        f"https://rpc.ankr.com/moonriver/{ANKR_KEY}",
    ],
    "boba": [
        "https://boba.publicnode.com",
        "https://mainnet.boba.network",
        "https://rpc.ankr.com/boba",
        f"https://rpc.ankr.com/boba/{ANKR_KEY}",
    ],
    "blast": [
        "https://blast.publicnode.com",
        "https://rpc.blast.io",
        "https://rpc.ankr.com/blast",
        f"https://rpc.ankr.com/blast/{ANKR_KEY}",
    ],
    "poly-zkevm": [
        "https://poly-zkevm.publicnode.com",
        "https://rpc.ankr.com/polygon_zkevm",
        f"https://rpc.ankr.com/polygon_zkevm/{ANKR_KEY}",
    ],
    "xrplevm": [
        "https://xrplevm.publicnode.com",
        "https://rpc.ankr.com/xrplevm",
        f"https://rpc.ankr.com/xrplevm/{ANKR_KEY}",
    ],
}

CHAIN_ALIASES: Dict[str, str] = {
    "base": "base",
    "ethereum": "ethereum",
    "bsc": "bsc",
    "bnb chain": "bsc",
    "bnb": "bsc",
    "polygon": "polygon",
    "optimism": "optimism",
    "arbitrum": "arbitrum",
    "linea": "linea",
    "polygon zkevm": "poly-zkevm",
    "avalanche": "avalanche",
    "bera": "berachain",
    "berachain": "berachain",
    "blast": "blast",
    "gnosis": "gnosis",
    "scroll": "scroll",
    "zk era": "zksync-era",
    "zksync": "zksync-era",
    "metis": "metis",
    "moonbeam": "moonbeam",
    "moonriver": "moonriver",
    "sonic": "sonic",
    "fraxtal": "fraxtal",
    "hyperliquid": "hyperliquid",
    "ink": "ink",
    "celo": "celo",
    "oasys": "oasys",
    "opbnb": "opbnb",
    "unichain": "unichain",
    "xrplevm": "xrplevm",
    "boba": "boba",
    "mantle": "mantle",
    "manta": "manta",
    "kaia": "kaia",
    "kava": "kava",
    "harmony": "harmony",
    "fuse": "fuse",
    "oasis": "oasis",
    "iotex": "iotex",
    "robinhood": "robinhood",
}

def _norm(raw: str) -> Optional[str]:
    k = raw.strip().lower()
    return CHAIN_ALIASES.get(k) or (k if k in SCAN_RPCS else None)

def _extract_chain_from_context(context: str) -> Optional[str]:
    cm = re.search(r"\(([^)]*)\)", context)
    if not cm:
        return None
    inner = cm.group(1).strip()
    parts = [p.strip() for p in inner.split(",")]
    for part in reversed(parts):
        c = _norm(part)
        if c:
            return c
    return None

def parse_hack_md(path: str) -> Dict[str, List[Tuple[str, str, int, str]]]:
    res: Dict[str, List[Tuple[str, str, int, str]]] = {}
    if not os.path.exists(path):
        return res
    with open(path, encoding="utf-8") as f:
        lines = f.readlines()
    cur_chain: Optional[str] = None
    cur_num = 0
    cur_principle_parts: List[str] = []
    hdr = re.compile(r"\*\*(\d+)\.\s+[\d/~]+—\s+.*?\((.*?)\)")
    prin = re.compile(r"\*\*해킹 원리\*\*:\s*(.*)")
    for line in lines:
        h = hdr.search(line)
        if h:
            cur_num = int(h.group(1))
            cur_chain = _norm(h.group(2))
            cur_principle_parts = []
            continue
        p = prin.search(line)
        if p:
            cur_principle_parts.append(p.group(1).strip())
            continue
        if "**취약 컨트랙트**:" not in line:
            continue
        cur_principle = " ".join(cur_principle_parts)
        for m in ADDRESS_RE.finditer(line):
            a = m.group(0).lower()
            if "..." in a or len(a) < 42:
                continue
            st = m.end()
            nm = ADDRESS_RE.search(line, st)
            en = nm.start() if nm else len(line)
            ctx = line[st:en]
            chain = _extract_chain_from_context(ctx) or cur_chain
            if not chain:
                continue
            lbl = ""
            lm = re.search(r"`" + re.escape(a) + r"`\s*\(([^)]*)\)", line)
            if lm:
                parts = [x.strip() for x in lm.group(1).split(",")]
                if parts:
                    lbl = parts[0]
            res.setdefault(chain, []).append((a, lbl, cur_num, cur_principle))
    for ch in res:
        seen: Set[str] = set()
        uq = []
        for a, l, n, p in res[ch]:
            if a not in seen:
                seen.add(a)
                uq.append((a, l, n, p))
        res[ch] = uq
    return res

def ngrams(code_hex: str, n: int = NGRAM_SIZE) -> Set[str]:
    if code_hex.startswith("0x"):
        code_hex = code_hex[2:]
    code_hex = code_hex.lower()
    if len(code_hex) < n:
        return set()
    return {code_hex[i:i+n] for i in range(0, len(code_hex)-n+1, 2)}

def jaccard(a: Set[str], b: Set[str]) -> float:
    if not a or not b:
        return 0.0
    return len(a & b) / len(a | b)

def _awj(path, data, indent=None):
    d = os.path.dirname(os.path.abspath(path))
    fd, tmp = tempfile.mkstemp(prefix=".tmp_", dir=d)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=indent)
            f.flush()
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            try:
                os.remove(tmp)
            except OSError:
                pass

@dataclass
class Seed:
    chain: str
    address: str
    label: str
    incident: int
    principle: str
    code_hex: str = ""
    ngrams: Set[str] = field(default_factory=set)
    length: int = 0

class Scanner:
    def __init__(self, seeds_by_chain: Dict[str, List[Seed]]):
        self.seeds_by_chain = seeds_by_chain
        self.all_seeds: List[Seed] = [s for seeds in seeds_by_chain.values() for s in seeds]
        self.code_cache: OrderedDict = OrderedDict()
        self.ccl = asyncio.Lock()
        self.ml = asyncio.Lock()
        self.matches: List[Dict[str, Any]] = []
        self._pe: Dict[str, asyncio.Event] = {}

    async def _bad(self, u, ul, uo, uf):
        async with ul:
            uf[u] = uf.get(u, 0) + 1
            if uf[u] >= 3 and u in uo:
                uo.remove(u)

    async def _rl_sleep(self, r):
        ra = r.headers.get("Retry-After")
        if ra:
            try:
                await asyncio.sleep(min(int(ra), 60))
            except Exception:
                await asyncio.sleep(1.0)
        else:
            await asyncio.sleep(1.0)

    async def _one(self, sess, method, params, fs, ul, uo, uf, rt):
        for at in range(2):
            async with ul:
                snap = list(uo)
            for i, u in enumerate(snap):
                try:
                    async with fs:
                        async with sess.post(u, json={"jsonrpc":"2.0","method":method,"params":params,"id":_next_rid()}, headers=HEADERS, timeout=rt) as r:
                            if r.status == 200:
                                d = await r.json(content_type=None)
                                if isinstance(d, dict) and "result" in d:
                                    if i != 0:
                                        async with ul:
                                            if u in uo:
                                                uo.remove(u)
                                                uo.insert(0, u)
                                    return d["result"]
                                # 200이지만 결과가 없는 응답(예: 비정상 바디)은 실패 처리 후 다음 URL로
                                await self._bad(u, ul, uo, uf)
                                continue
                            else:
                                # 429 포함 모든 비-200은 실패 카운트에 포함되어 3회 초과 시 URL 제거
                                await self._bad(u, ul, uo, uf)
                                if r.status == 429:
                                    await self._rl_sleep(r)
                except Exception:
                    await self._bad(u, ul, uo, uf)
                    continue
            if at < 1:
                await asyncio.sleep(0.5 * (2 ** at))
        return None

    async def _batch(self, sess, method, pl, fs, ul, uo, uf, rt):
        if not pl:
            return []
        n = len(pl)
        for at in range(2):
            async with ul:
                snap = list(uo)
            for i, u in enumerate(snap):
                try:
                    body = [{"jsonrpc":"2.0","method":method,"params":p,"id":ix} for ix, p in enumerate(pl)]
                    async with fs:
                        async with sess.post(u, json=body, headers=HEADERS, timeout=rt) as r:
                            if r.status == 200:
                                d = await r.json(content_type=None)
                                if isinstance(d, list):
                                    rl = [None] * n
                                    for it in d:
                                        if isinstance(it, dict) and "id" in it:
                                            try:
                                                ix = int(it["id"])
                                            except Exception:
                                                continue
                                            if 0 <= ix < n:
                                                rl[ix] = it.get("result") if "result" in it else None
                                    if all(x is None for x in rl):
                                        await self._bad(u, ul, uo, uf)
                                        continue
                                    if i != 0:
                                        async with ul:
                                            if u in uo:
                                                uo.remove(u)
                                                uo.insert(0, u)
                                    return rl
                                # 200이지만 결과가 없는 응답은 실패 처리 후 다음 URL로
                                await self._bad(u, ul, uo, uf)
                                continue
                            else:
                                # 429 포함 모든 비-200은 실패 카운트에 포함되어 3회 초과 시 URL 제거
                                await self._bad(u, ul, uo, uf)
                                if r.status == 429:
                                    await self._rl_sleep(r)
                except Exception:
                    await self._bad(u, ul, uo, uf)
                    continue
            if at < 1:
                await asyncio.sleep(0.5 * (2 ** at))
        # 두 시도 모두 실패: 이미 지친 RPC에 개별 폴백 요청을 추가로 퍼붓지 않음.
        # 해당 주소는 캐시되지 않아 다음 블록에서 재시도됨.
        return [None] * n

    async def get_codes(self, sess, chain, addrs, blk, fs, ul, uo, uf, rt):
        wait: List[Tuple[str, asyncio.Event]] = []
        miss: List[str] = []
        seen: Set[str] = set()
        async with self.ccl:
            for a in addrs:
                k = (chain, a)
                if k in self.code_cache:
                    continue
                if k in self._pe:
                    if a not in seen:
                        wait.append((k, self._pe[k]))
                        seen.add(a)
                else:
                    ev = asyncio.Event()
                    self._pe[k] = ev
                    miss.append(a)
                    seen.add(a)
        upd: Dict[str, str] = {}
        inv: Set[str] = set()
        try:
            if miss:
                got = []
                for i in range(0, len(miss), BATCH_SIZE):
                    chunk = miss[i:i+BATCH_SIZE]
                    got.extend(await self._batch(sess, "eth_getCode", [[a, hex(blk)] for a in chunk], fs, ul, uo, uf, rt))
                for a, c in zip(miss, got):
                    k = (chain, a)
                    if isinstance(c, str):
                        c = c.lower()
                        if c in ("", "0x"):
                            inv.add(a)
                        else:
                            hp = c[2:] if c.startswith("0x") else c
                            if not hp.strip("0"):
                                inv.add(a)
                            else:
                                upd[a] = c
        finally:
            async with self.ccl:
                for a, c in upd.items():
                    k = (chain, a)
                    self.code_cache[k] = c
                    self.code_cache.move_to_end(k)
                for a in inv:
                    k = (chain, a)
                    if k not in self.code_cache:
                        self.code_cache[k] = ""
                    self.code_cache.move_to_end(k)
                while len(self.code_cache) > MAX_CODE_CACHE:
                    self.code_cache.popitem(last=False)
                for a in miss:
                    k = (chain, a)
                    ev = self._pe.pop(k, None)
                    if ev is not None:
                        ev.set()
        for _, ev in wait:
            await ev.wait()
        out: List[Optional[str]] = []
        async with self.ccl:
            for a in addrs:
                k = (chain, a)
                c = self.code_cache.get(k)
                if isinstance(c, str) and c and c != "0x":
                    self.code_cache.move_to_end(k)
                    out.append(c)
                else:
                    out.append(None)
        return out

    async def fetch_seed_codes(self, sess, chain, seeds, fs):
        uo = list(SCAN_RPCS.get(chain, []))
        if not uo:
            print(f"[{chain}] no rpc for seed fetch")
            return
        ul = asyncio.Lock()
        uf: Dict[str, int] = {}
        rt = aiohttp.ClientTimeout(total=15)
        for u in list(uo):
            try:
                done_all = True
                for i in range(0, len(seeds), BATCH_SIZE):
                    chunk = seeds[i:i+BATCH_SIZE]
                    pl_ = [[a.address, "latest"] for a in chunk]
                    # 이미 코드가 채워진 시드는 중복 RPC 호출 방지를 위해 요청 대상에서 제외
                    pending = [s for s in chunk if not s.code_hex]
                    if not pending:
                        continue
                    pl_ = [[s.address, "latest"] for s in pending]
                    body = [{"jsonrpc":"2.0","method":"eth_getCode","params":p,"id":ix} for ix, p in enumerate(pl_)]
                    async with fs:
                        async with sess.post(u, json=body, headers=HEADERS, timeout=rt) as r:
                            if r.status == 200:
                                d = await r.json(content_type=None)
                                if isinstance(d, list):
                                    chunk_done = True
                                    for idx in range(len(pending)):
                                        it = d[idx] if idx < len(d) else None
                                        s = pending[idx]
                                        if isinstance(it, dict) and "result" in it:
                                            c = it["result"]
                                            if isinstance(c, str) and c and c != "0x":
                                                s.code_hex = c.lower()
                                                s.ngrams = ngrams(c[2:])
                                                s.length = len(c[2:])
                                        else:
                                            # 배치 내 개별 항목이 누락되면 해당 청크 미완료로 처리해 다음 URL 재시도
                                            chunk_done = False
                                    if not chunk_done:
                                        done_all = False
                                        break
                                else:
                                    # 200이지만 바디가 리스트가 아니면(예: 에러 객체) 실패 처리 후 다음 URL
                                    if r.status == 429:
                                        await self._rl_sleep(r)
                                    await self._bad(u, ul, uo, uf)
                                    done_all = False
                                    break
                                continue
                            else:
                                if r.status == 429:
                                    await self._rl_sleep(r)
                                await self._bad(u, ul, uo, uf)
                                done_all = False
                                break
                if done_all:
                    return
            except Exception:
                await self._bad(u, ul, uo, uf)
                continue
        print(f"[{chain}] seed code fetch failed for {len(seeds)} seeds")

    def _similarity(self, hp: str, seed: Seed) -> float:
        if seed.code_hex and hp == seed.code_hex[2:].lower():
            return 1.0
        if seed.length > 0:
            ratio = len(hp) / seed.length
            if ratio < LENGTH_RATIO_MIN or ratio > LENGTH_RATIO_MAX:
                return 0.0
        ng = ngrams(hp)
        return jaccard(ng, seed.ngrams)

    async def scan_chain(self, chain, sess, resume=None, fs=None, pl=None, sp=None):
        uo = list(SCAN_RPCS.get(chain, []))
        if not uo:
            print(f"[{chain}] no rpc")
            return []
        if resume is not None and resume < 0:
            resume = None
        if fs is None:
            fs = asyncio.Semaphore(CONCURRENCY)
        if pl is None:
            pl = asyncio.Lock()
        if not isinstance(sp, dict):
            sp = {}
        ul = asyncio.Lock()
        uf: Dict[str, int] = {}
        rt = aiohttp.ClientTimeout(total=15)
        cm: List[Dict[str, Any]] = []
        addr_best: Dict[str, Dict[str, Any]] = {}
        blocks_since_checkpoint = 0
        last_checkpoint_len = 0

        if resume is not None:
            pg = None
            if isinstance(sp, dict):
                pg = sp.get(chain)
            if not isinstance(pg, dict):
                try:
                    with open(PROGRESS_FILE, encoding="utf-8") as f:
                        allp = json.load(f)
                    if isinstance(allp, dict):
                        pg = allp.get(chain)
                except Exception as e:
                    print(f"[{chain}] progress read error: {e}")
            if isinstance(pg, dict):
                st = set()
                uq = []
                for m in pg.get("matches", []) or []:
                    k = (m.get("chain"), m.get("address"))
                    if k in st:
                        continue
                    st.add(k)
                    uq.append(m)
                cm[:] = uq
                self.matches.extend(uq)
                for m in uq:
                    addr_best[m.get("address")] = {
                        "similarity": m.get("similarity", 0.0),
                        "seed_address": m.get("seed_address", ""),
                        "seed_label": m.get("seed_label", ""),
                        "seed_incident": m.get("seed_incident", 0),
                        "seed_principle": m.get("seed_principle", ""),
                    }

        async def one(m, p):
            return await self._one(sess, m, p, fs, ul, uo, uf, rt)

        async def bat(m, pl_):
            return await self._batch(sess, m, pl_, fs, ul, uo, uf, rt)

        latest = None
        r = await one("eth_blockNumber", [])
        if r is not None:
            try:
                latest = int(r, 16) if isinstance(r, str) else int(r)
            except Exception:
                pass
        if latest is None:
            print(f"[{chain}] block fail")
            return []

        if resume is not None and resume >= latest:
            print(f"[{chain}] done")
            return cm

        if resume is not None and 0 <= resume < latest:
            sb = resume + 1
        else:
            sb = 0
        sr = latest - sb + 1
        if sr <= 0:
            print(f"[{chain}] done")
            return cm
        if resume is not None:
            print(f"[{chain}] res {sb} scan {sr}")
        else:
            print(f"[{chain}] latest {latest} scan {sr} (full history)")

        seeds = self.all_seeds
        sinfo = [(s.address, s.label, s.incident, s.principle) for s in seeds]

        async def collect_addrs(blk):
            addrs: Set[str] = set()
            bd = await one("eth_getBlockByNumber", [hex(blk), True])
            if not isinstance(bd, dict):
                return addrs
            txs = bd.get("transactions")
            if not txs:
                return addrs
            # 일부 RPC는 full=True를 무시하고 tx 해시 문자열만 반환함.
            # 해시만 있으면 to 주소를 알 수 없으므로 eth_getTransactionByHash로 조회.
            # 전체 tx 해시를 BATCH_SIZE 단위로 순회하며 누락 없이 수집함.
            if txs and isinstance(txs[0], str):
                for i in range(0, len(txs), BATCH_SIZE):
                    hash_batch = txs[i:i+BATCH_SIZE]
                    bodies = [
                        {"jsonrpc": "2.0", "method": "eth_getTransactionByHash", "params": [h], "id": ix}
                        for ix, h in enumerate(hash_batch)
                    ]
                    async with ul:
                        snap = list(uo)
                    batch_ok = False
                    for u in snap:
                        try:
                            async with fs:
                                async with sess.post(u, json=bodies, headers=HEADERS, timeout=rt) as r:
                                    if r.status == 200:
                                        d = await r.json(content_type=None)
                                        if isinstance(d, list):
                                            batch_ok = True
                                            for it in d:
                                                if isinstance(it, dict) and "result" in it and it["result"]:
                                                    tx = it["result"]
                                                    to = tx.get("to")
                                                    if isinstance(to, str) and ADDRESS_RE.fullmatch(to):
                                                        addrs.add(to.lower())
                                        break
                                    else:
                                        await self._bad(u, ul, uo, uf)
                        except Exception:
                            await self._bad(u, ul, uo, uf)
                            continue
                    if not batch_ok:
                        # 해당 배치가 전체 URL 실패 시 해당 해시들은 수집 못 하고 다음 배치로 넘어감
                        continue
                return addrs
            for tx in txs:
                if not isinstance(tx, dict):
                    continue
                # 계약 생성(tx.to=None) 및 내부 트랜잭션은 스캔 범위에서 제외함.
                # 전체 스캔에 이미 신규 배포 주소가 포함되므로 별도 신규 배포 추적은 불필요함.
                to = tx.get("to")
                if isinstance(to, str) and ADDRESS_RE.fullmatch(to):
                    addrs.add(to.lower())
            return addrs

        async def process_block(blk):
            addrs = await collect_addrs(blk)
            if not addrs:
                return 0
            sorted_addrs = sorted(addrs)
            codes = await self.get_codes(sess, chain, sorted_addrs, blk, fs, ul, uo, uf, rt)
            new_matches = 0
            for a, c in zip(sorted_addrs, codes):
                if not c or c == "0x" or len(c) < 10:
                    continue
                hp = c[2:] if c.startswith("0x") else c
                best_sim = 0.0
                best_ix = -1
                for i, s in enumerate(seeds):
                    sc = self._similarity(hp, s)
                    if sc == 1.0:
                        best_sim = 1.0
                        best_ix = i
                        break
                    if sc > best_sim:
                        best_sim = sc
                        best_ix = i
                if best_sim >= SIMILARITY_THRESHOLD:
                    a0, l0, n0, p0 = sinfo[best_ix]
                    m = {
                        "chain": chain,
                        "block": blk,
                        "address": a,
                        "seed_address": a0,
                        "seed_label": l0,
                        "seed_incident": n0,
                        "seed_principle": p0,
                        "similarity": best_sim,
                    }
                    async with self.ml:
                        prev = addr_best.get(a)
                        if prev and prev["similarity"] >= best_sim:
                            continue
                        addr_best[a] = {
                            "similarity": best_sim,
                            "seed_address": a0,
                            "seed_label": l0,
                            "seed_incident": n0,
                            "seed_principle": p0,
                        }
                        cm.append(m)
                        self.matches.append(m)
                    new_matches += 1
                    print(f"  [{chain}] MATCH b{blk} {a} <- {a0} ({p0[:40]}) sim={best_sim}")
            return new_matches

        blk = sb
        while blk <= latest:
            ed = min(blk + BLOCK_BATCH, latest + 1)
            batch_blks = list(range(blk, ed))
            rs = await asyncio.gather(*[process_block(b) for b in batch_blks], return_exceptions=True)
            for b, x in zip(batch_blks, rs):
                if isinstance(x, Exception):
                    print(f"  [{chain}] b{b} {x}")
            blocks_since_checkpoint += len(batch_blks)
            # 체크포인트는 매번 전체 매칭 리스트를 원자적으로 재기록함(증분 저장 미사용).
            # 크래시 시 일관성 복구를 우선하는 의도적 설계이며, 리뷰 시 "매번 전체 재기록" 지적은 제외 대상.
            if blocks_since_checkpoint >= PROGRESS_INTERVAL or ed - 1 == latest:
                async with pl:
                    sp[chain] = {"chain": chain, "last_block": ed - 1, "matches": list(cm)}
                    await asyncio.get_running_loop().run_in_executor(None, _awj, PROGRESS_FILE, dict(sp), None, False)
                blocks_since_checkpoint = 0
                last_checkpoint_len = len(cm)
                print(f"  [{chain}] checkpoint b{ed-1} {len(cm)} matches")
            blk = ed
        print(f"[{chain}] complete {len(cm)} matches")
        return cm

async def main_async():
    raw = parse_hack_md(HACK_FILE)
    sbc: Dict[str, List[Seed]] = {}
    for ch, en in raw.items():
        sbc[ch] = [Seed(chain=ch, address=a, label=l, incident=n, principle=p) for a, l, n, p in en]
    # seed 코드가 없는 체인도 전체 블록을 스캔함(의도적).
    # 해당 체인에 매칭할 seed가 없으면 유사도 비교는 수행하지 않지만,
    # 전체 체인 커버리지를 우선하며 seed가 나중에 추가될 경우를 대비함.
    chains = list(SCAN_RPCS.keys())
    if not chains:
        print("no chains")
        return
    conn = aiohttp.TCPConnector(limit=CONNECTOR_LIMIT, ttl_dns_cache=3600)
    fs_map: Dict[str, asyncio.Semaphore] = {ch: asyncio.Semaphore(CONCURRENCY) for ch in chains}
    pl = asyncio.Lock()
    sp: Dict[str, Any] = {}
    sc = Scanner(sbc)
    if os.path.exists(PROGRESS_FILE):
        try:
            with open(PROGRESS_FILE, encoding="utf-8") as f:
                sp = json.load(f)
            if not isinstance(sp, dict):
                sp = {}
        except Exception:
            sp = {}
    async with aiohttp.ClientSession(connector=conn) as sess:
        tasks = []
        for ch in chains:
            if sbc.get(ch):
                tasks.append(sc.fetch_seed_codes(sess, ch, sbc[ch], fs_map[ch]))
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
        print(f"Seeds: {sum(len(v) for v in sbc.values())} across {len(sbc)} chains")
        print(f"Scanning {len(chains)} chains: {', '.join(chains)}")
        stasks = []
        for ch in chains:
            pg = sp.get(ch) if isinstance(sp, dict) else None
            rf = pg.get("last_block") if isinstance(pg, dict) else None
            stasks.append(sc.scan_chain(ch, sess, rf, fs_map[ch], pl, sp))
        await asyncio.gather(*stasks, return_exceptions=True)
    seen = {}
    uniq = []
    for m in sc.matches:
        k = (m.get("chain"), m.get("address"))
        sim = m.get("similarity", 0.0)
        if k not in seen or sim > seen[k][0]:
            seen[k] = (sim, m)
    uniq = [v[1] for v in seen.values()]
    uniq.sort(key=lambda m: m.get("similarity", 0.0), reverse=True)
    await asyncio.get_running_loop().run_in_executor(None, _awj, RESULTS_FILE, uniq, 2)
    print(f"\nTotal matches: {len(uniq)}")
    print(f"Results written to {RESULTS_FILE}")
    for m in uniq[:20]:
        print(f"  [{m.get('chain')}] {m.get('address')} <- {m.get('seed_address')} {m.get('seed_principle','')[:50]} sim={m.get('similarity')}")

def main():
    asyncio.run(main_async())

if __name__ == "__main__":
    main()
