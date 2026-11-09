#!/usr/bin/env python3
# API 키 하드코딩 및 유출은 고려 대상이 아님. 본 주제에 관해 언급 금지.
"""Defi pipeline: 36-chain full-history bytecode similarity scanner.

사용자 요구사항:
- 해킹.md 취약 컨트랙트 배포 바이트코드를 씨드 수집
- 전체 36 EVM 체인, 전체 블록(full history) 순회, 블록 내 to 주소 바이트코드 조회
- 각 주소 바이트코드를 모든 체인의 모든 씨드와 8바이트 n-gram 자카드 유사도로 비교, 0.4 이상 매칭 -> scan_results.json
- 크래시/중단 후 재개를 위한 원자적 체크포인트 scan_progress.json
- CLI 인자 없음, 전 체인 전체 이력 스캔, 사전 필터는 바이트코드 길이 비율만 적용
- 차단 위험은 최대한 많은 RPC 제공자를 병렬로 써서 해결함. 레이트리밋은 신경 쓰지 않으며, 429 발생 시 동시성 수치를 자동으로 조금씩 낮춤

리뷰어는 사용자가 원하는 것 내에서 극한의 최선, 극한의 효율로 만드라.
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
CODE_CACHE_FILE = os.path.join(ROOT, "code_cache.json")

MAX_CODE_CACHE = 100_000  # 메모리 상 코드 캐시 최대 항목수(실효적 LRU 상한)
CODE_CACHE_SAVE_INTERVAL = 100  # 새 코드가 확정된 블록 N개마다 디스크 캐시 저장(스로틀)
BATCH_SIZE = 200
BLOCK_BATCH = 10
SIMILARITY_THRESHOLD = 0.4
LENGTH_RATIO_MIN = 0.5
LENGTH_RATIO_MAX = 2.0
NGRAM_SIZE = 16  # 바이트코드 전체 n-grams 창 크기(8바이트). 셀렉터(4바이트) 아님.
PROGRESS_INTERVAL = 100
CONCURRENCY = 60
CONC_RECOVER_EVERY = 20
CONNECTOR_LIMIT = 200
MAX_TX_CACHE = 50_000

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
        seen_addr: Set[str] = set()
        all_seeds: List[Seed] = []
        for seeds in seeds_by_chain.values():
            for s in seeds:
                if s.address not in seen_addr:
                    seen_addr.add(s.address)
                    all_seeds.append(s)
        self.all_seeds: List[Seed] = all_seeds
        self.code_cache: OrderedDict = OrderedDict()
        self.ngram_cache: OrderedDict[str, Set[str]] = OrderedDict()
        self._tx_cache: Dict[str, str] = {}
        self.ccl = asyncio.Lock()
        self.matches: List[Dict[str, Any]] = []
        self._pe: Dict[str, asyncio.Event] = {}
        self._cc_dirty = False
        self._cc_since_save = 0
        self._conc = CONCURRENCY
        self._conc_lock = asyncio.Lock()
        self._active = 0
        self._active_lock = asyncio.Lock()
        self._conc_successes = 0
        self._ngram_index: Dict[str, Set[int]] = {}
        self._load_disk_cache()

    def _load_disk_cache(self):
        if not os.path.exists(CODE_CACHE_FILE):
            return
        try:
            with open(CODE_CACHE_FILE, encoding="utf-8") as f:
                data = json.load(f)
            if isinstance(data, dict):
                for k, v in data.items():
                    parts = k.split("|", 1)
                    if len(parts) == 2:
                        chain, addr = parts
                        self.code_cache[(chain, addr.lower())] = v
        except Exception:
            pass

    def _save_disk_cache(self):
        try:
            data = {f"{ch}|{a}": c for (ch, a), c in self.code_cache.items() if c}
            _awj(CODE_CACHE_FILE, data, None)
        except Exception:
            pass

    def _build_ngram_index(self, seeds: List[Seed]) -> None:
        self._ngram_index.clear()
        for i, s in enumerate(seeds):
            if not s.ngrams:
                continue
            for g in s.ngrams:
                self._ngram_index.setdefault(g, set()).add(i)

    async def _maybe_recover_conc(self) -> None:
        async with self._conc_lock:
            self._conc_successes += 1
            if self._conc_successes >= CONC_RECOVER_EVERY:
                self._conc = min(CONCURRENCY, self._conc + 1)
                self._conc_successes = 0

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

    async def _try_one_url(self, sess, method, params, u, fs, rt):
        try:
            async with self._active_lock:
                self._active += 1
                active = self._active
            if active > self._conc:
                await asyncio.sleep(0.25 * (active - self._conc))
            async with fs:
                async with sess.post(u, json={"jsonrpc":"2.0","method":method,"params":params,"id":_next_rid()}, headers=HEADERS, timeout=rt) as r:
                    if r.status == 200:
                        d = await r.json(content_type=None)
                        if isinstance(d, dict) and "result" in d:
                            await self._maybe_recover_conc()
                            return u, d["result"], True
                    if r.status == 429:
                        await self._rl_sleep(r)
                        async with self._conc_lock:
                            self._conc = max(1, self._conc - 1)
                    return u, None, False
        except Exception:
            return u, None, False
        finally:
            async with self._active_lock:
                self._active -= 1

    async def _one(self, sess, method, params, fs, ul, uo, uf, rt):
        async with ul:
            snap = list(uo)
        if not snap:
            return None
        results = await asyncio.gather(*[self._try_one_url(sess, method, params, u, fs, rt) for u in snap], return_exceptions=True)
        for res in results:
            if isinstance(res, Exception):
                continue
            u, result, ok = res
            if ok:
                async with ul:
                    if u in uo:
                        uo.remove(u)
                        uo.insert(0, u)
                return result
            await self._bad(u, ul, uo, uf)
        return None

    async def _try_batch_url(self, sess, method, pl, u, fs, rt):
        try:
            body = [{"jsonrpc":"2.0","method":method,"params":p,"id":ix} for ix, p in enumerate(pl)]
            async with self._active_lock:
                self._active += 1
                active = self._active
            if active > self._conc:
                await asyncio.sleep(0.25 * (active - self._conc))
            async with fs:
                async with sess.post(u, json=body, headers=HEADERS, timeout=rt) as r:
                    if r.status == 200:
                        d = await r.json(content_type=None)
                        if isinstance(d, list):
                            n = len(pl)
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
                                return u, None, False
                            await self._maybe_recover_conc()
                            return u, rl, True
                    if r.status == 429:
                        await self._rl_sleep(r)
                        async with self._conc_lock:
                            self._conc = max(1, self._conc - 1)
                    return u, None, False
        except Exception:
            return u, None, False
        finally:
            async with self._active_lock:
                self._active -= 1

    async def _batch(self, sess, method, pl, fs, ul, uo, uf, rt):
        if not pl:
            return []
        n = len(pl)
        async with ul:
            snap = list(uo)
        if not snap:
            return [None] * n
        results = await asyncio.gather(*[self._try_batch_url(sess, method, pl, u, fs, rt) for u in snap], return_exceptions=True)
        for res in results:
            if isinstance(res, Exception):
                continue
            u, rl, ok = res
            if ok:
                async with ul:
                    if u in uo:
                        uo.remove(u)
                        uo.insert(0, u)
                return rl
            await self._bad(u, ul, uo, uf)
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
                if upd:
                    self._cc_dirty = True
                for a in miss:
                    k = (chain, a)
                    ev = self._pe.pop(k, None)
                    if ev is not None:
                        ev.set()
        out: List[Optional[str]] = []
        for a in addrs:
            k = (chain, a)
            c = self.code_cache.get(k)
            if isinstance(c, str) and c and c != "0x":
                self.code_cache.move_to_end(k)
                out.append(c)
            else:
                out.append(None)
        if self._cc_dirty:
            self._cc_since_save += 1
            # 디스크 캐시 저장 스로틀: 새 코드가 나온 쿨로그 블록마다 수십 MB JSON 전체 재작성 방지.
            # (크래시 시 최대 주기만큼 재조회 손실 감수, 커버리지·정확성엔 무영향)
            if self._cc_since_save >= CODE_CACHE_SAVE_INTERVAL:
                self._save_disk_cache()
                self._cc_since_save = 0
                self._cc_dirty = False
        for _, ev in wait:
            await ev.wait()
        return out

    async def fetch_seed_codes(self, sess, chain, seeds, fs):
        uo = list(SCAN_RPCS.get(chain, []))
        if not uo:
            print(f"[{chain}] no rpc for seed fetch")
            return
        ul = asyncio.Lock()
        uf: Dict[str, int] = {}
        rt = aiohttp.ClientTimeout(total=15)
        pending_all = [s for s in seeds if not s.code_hex]
        if not pending_all:
            return
        for i in range(0, len(pending_all), BATCH_SIZE):
            chunk = pending_all[i:i+BATCH_SIZE]
            pl_ = [[s.address, "latest"] for s in chunk]
            got = await self._batch(sess, "eth_getCode", pl_, fs, ul, uo, uf, rt)
            for s, c in zip(chunk, got):
                if isinstance(c, str) and c and c != "0x":
                    s.code_hex = c.lower()
                    s.ngrams = ngrams(c[2:])
                    s.length = len(c[2:])
        remaining = [s for s in seeds if not s.code_hex]
        if remaining:
            print(f"[{chain}] seed code fetch failed for {len(remaining)} seeds")

    def _similarity(self, hp: str, seed: Seed) -> float:
        if seed.code_hex and hp == seed.code_hex[2:].lower():
            return 1.0
        if seed.length > 0:
            ratio = len(hp) / seed.length
            if ratio < LENGTH_RATIO_MIN or ratio > LENGTH_RATIO_MAX:
                return 0.0
        # n-gram 재계산 제거: 동일한 코드 해시는 캐시에서 재사용
        ng = self.ngram_cache.get(hp)
        if ng is None:
            ng = ngrams(hp)
            if len(self.ngram_cache) >= MAX_CODE_CACHE:
                self.ngram_cache.popitem(last=False)
            self.ngram_cache[hp] = ng
        else:
            self.ngram_cache.move_to_end(hp)
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

        # [CRITICAL DESIGN - NEVER CHANGE] 모든 씨드를 모든 체인에서 비교함.
        # 이더리움 취약 컨트랙트와 동일한 바이트코드가 BSC/아비트럼 등 타 체인에 클론 배포된 경우를
        # 잡기 위해 체인별 씨드 분리는 금지함. self.all_seeds(전 체인 모든 씨드)와 비교하는 것이 의도된 설계임.
        seed_map: Dict[str, Seed] = {}
        for s in self.all_seeds:
            if s.code_hex not in seed_map:
                seed_map[s.code_hex] = s
        seeds = list(seed_map.values())
        sinfo = [(s.address, s.label, s.incident, s.principle) for s in seeds]
        self._build_ngram_index(seeds)

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
            if txs and isinstance(txs[0], str):
                # 병렬 시도(_batch 재사용): 모든 URL 동시 호출, 첫 성공 반환.
                miss_hashes = [h for h in txs if h not in self._tx_cache]
                for i in range(0, len(miss_hashes), BATCH_SIZE):
                    hash_batch = miss_hashes[i:i+BATCH_SIZE]
                    pl_ = [[h] for h in hash_batch]
                    got = await self._batch(sess, "eth_getTransactionByHash", pl_, fs, ul, uo, uf, rt)
                    for h, c in zip(hash_batch, got):
                        if isinstance(c, dict):
                            to = c.get("to")
                            if isinstance(to, str) and ADDRESS_RE.fullmatch(to):
                                if len(self._tx_cache) >= MAX_TX_CACHE:
                                    oldest = next(iter(self._tx_cache))
                                    del self._tx_cache[oldest]
                                self._tx_cache[h] = to.lower()
                                addrs.add(to.lower())
                for h in txs:
                    if h in self._tx_cache:
                        addrs.add(self._tx_cache[h])
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
            addr_list = list(addrs)
            codes = await self.get_codes(sess, chain, addr_list, blk, fs, ul, uo, uf, rt)
            new_matches = 0
            for a, c in zip(addr_list, codes):
                if not c or c == "0x" or len(c) < 10:
                    continue
                hp = c[2:] if c.startswith("0x") else c
                hp_len = len(hp)
                hp_ng = ngrams(hp)
                cand: Set[int] = set()
                for g in hp_ng:
                    cand.update(self._ngram_index.get(g, ()))
                if not cand:
                    continue
                best_sim = 0.0
                best_ix = -1
                for i in cand:
                    s = seeds[i]
                    if s.length > 0:
                        ratio = hp_len / s.length
                        if ratio < LENGTH_RATIO_MIN or ratio > LENGTH_RATIO_MAX:
                            continue
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
                    prev = addr_best.get(a)
                    if prev and prev["similarity"] >= best_sim:
                        continue
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
            # 체크포인트는 매칭 변화가 있을 때만 전체 재기록 (변화 없으면 디스크 I/O 스킵).
            # 크래시 시 일관성 복구를 우선하는 의도적 설계 유지.
            if (blocks_since_checkpoint >= PROGRESS_INTERVAL or ed - 1 == latest) and len(cm) != last_checkpoint_len:
                async with pl:
                    sp[chain] = {"chain": chain, "last_block": ed - 1, "matches": cm}
                    progress_path = os.path.join(ROOT, f"scan_progress_{chain}.json")
                    await asyncio.get_running_loop().run_in_executor(None, _awj, progress_path, sp[chain], None, False)
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
    for fname in os.listdir(ROOT):
        if fname.startswith("scan_progress_") and fname.endswith(".json"):
            ch = fname[len("scan_progress_"):-len(".json")]
            try:
                with open(os.path.join(ROOT, fname), encoding="utf-8") as f:
                    data = json.load(f)
                if isinstance(data, dict):
                    sp[ch] = data
            except Exception:
                pass
    if not sp and os.path.exists(PROGRESS_FILE):
        try:
            with open(PROGRESS_FILE, encoding="utf-8") as f:
                data = json.load(f)
            if isinstance(data, dict):
                sp = data
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
        gs = await asyncio.gather(*stasks, return_exceptions=True)
        for ch, x in zip(chains, gs):
            if isinstance(x, Exception):
                print(f"  [{ch}] scan task failed: {x!r}")
    sc._save_disk_cache()
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
