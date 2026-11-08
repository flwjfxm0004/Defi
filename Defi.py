#!/usr/bin/env python3
"""Defi pipeline: multi-chain block scanner for hack-related contracts."""
import argparse
import asyncio
import json
import os
import re
import tempfile
from typing import Any, Dict, List, Optional
import aiohttp

ROOT = os.path.dirname(os.path.abspath(__file__))

RESULTS_FILE = os.path.join(ROOT, "scan_results.json")

# Bytecode patterns: required (must have) + optional (boosts similarity score)
BYTECODE_PATTERNS = {
    "NEM": ({"0xbbafc98c"}, set()),
    "Balancer": ({"0x6d06dfa0"}, {"0x46ab38f1", "0xb02f0b73", "0xf8b2cb4f", "0x4bb278f3", "0xe4e1e538", "0x8201aa3f", "0x7c5e9ea4"}),
    "TranchePool": ({"0x8e27c105"}, set()),
    "Slice": ({"0x9d2edfac"}, set()),
    "TENDRIL": ({"0x9c74daf0"}, {"0xedae876f", "0xb734e98b"}),
    "InsurancePool": ({"0x01681a62"}, {"0x09c5eabe", "0x13a1a562"}),
    "Hifi": ({"0x0f32f1c9"}, {"0x7adbf973", "0xc5ed81f7"}),
    "X100": ({"0xa4376352"}, {"0x3724fc1c", "0x4e71d92d"}),
    "zkLink": ({"0x68871c9c"}, {"0xbf22d9c3"}),
    "Yapeswap": ({"0xa2e74af6"}, set()),
}


SCAN_RPCS = {
    "bsc": ["https://bsc.api.pocket.network", "https://bsc-mainnet.nodereal.io/v1/e7ec74ea77134e99b3aca76de8e602c5", "https://rpc.ankr.com/bsc/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://bsc-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "ethereum": ["https://eth.api.pocket.network", "https://ethereum-public.nodies.app", "https://rpc.mevblocker.io", "https://rpc.mevblocker.io/fast", "https://rpc.mevblocker.io/fullprivacy", "https://eth.meowrpc.com", "https://rpc.flashbots.net", "https://mainnet.gateway.tenderly.co", "https://ethereum-rpc.publicnode.com", "https://rpc.ankr.com/eth/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://ethereum-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "base": ["https://base.api.pocket.network", "https://base.gateway.tenderly.co", "https://1rpc.io/base", "https://mainnet.base.org", "https://rpc.ankr.com/base/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://base-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "arbitrum": ["https://arb-one.api.pocket.network", "https://arbitrum.gateway.tenderly.co", "https://arb1.arbitrum.io/rpc", "https://rpc.ankr.com/arbitrum/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://arbitrum-one-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "avalanche": ["https://avax.api.pocket.network", "https://avalanche.gateway.tenderly.co", "https://api.avax.network/ext/bc/C/rpc", "https://rpc.ankr.com/avalanche/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://rpc.ankr.com/avax"],
    "iotex": ["https://iotex.api.pocket.network", "https://babel-api.mainnet.iotex.io", "https://rpc.ankr.com/iotex"],
    "moonbeam": ["https://moonbeam.api.pocket.network", "https://rpc.api.moonbeam.network", "https://rpc.ankr.com/moonbeam"],
    "metis": ["https://metis.api.pocket.network", "https://andromeda.metis.io/?owner=1088", "https://rpc.ankr.com/metis"],
    "moonriver": ["https://moonriver.api.pocket.network", "https://rpc.api.moonriver.io", "https://rpc.ankr.com/moonriver"],
    "fraxtal": ["https://fraxtal.api.pocket.network", "https://rpc.frax.com", "https://rpc.ankr.com/fraxtal"],
    "sonic": ["https://sonic.api.pocket.network", "https://rpc.soniclabs.com", "https://rpc.ankr.com/sonic"],
    "bera": ["https://bera.api.pocket.network", "https://rpc.berachain.com", "https://rpc.ankr.com/berachain"],
    "hyperliquid": ["https://hyperliquid.api.pocket.network", "https://rpc.hyperliquid.xyz/evm", "https://rpc.ankr.com/hyperliquid"],
    "ink": ["https://ink.api.pocket.network", "https://rpc-gel.inkonchain.com", "https://rpc.ankr.com/ink"],
    "poly-zkevm": ["https://poly-zkevm.api.pocket.network", "https://rpc.ankr.com/polygon_zkevm"],
    "scroll": ["https://scroll.api.pocket.network", "https://rpc.scroll.io", "https://rpc.ankr.com/scroll"],
    "robinhood": ["https://robinhood.api.pocket.network"],
    "celo": ["https://celo.api.pocket.network", "https://forno.celo.org", "https://rpc.ankr.com/celo"],
    "oasys": ["https://oasys.api.pocket.network", "https://rpc.ankr.com/oasys"],
    "opbnb": ["https://opbnb.api.pocket.network", "https://opbnb-mainnet-rpc.bnbchain.org", "https://rpc.ankr.com/opbnb"],
    "unichain": ["https://unichain.api.pocket.network", "https://mainnet.unichain.org", "https://rpc.ankr.com/unichain"],
    "xrplevm": ["https://xrplevm.api.pocket.network", "https://rpc.ankr.com/xrplevm"],
    "zksync-era": ["https://zksync-era.api.pocket.network", "https://mainnet.era.zksync.io", "https://rpc.ankr.com/zksync_era"],
    "boba": ["https://boba.api.pocket.network", "https://mainnet.boba.network", "https://rpc.ankr.com/boba"],
    "blast": ["https://blast.api.pocket.network", "https://rpc.blast.io", "https://rpc.ankr.com/blast"],
    "linea": ["https://linea.api.pocket.network", "https://rpc.linea.build", "https://rpc.ankr.com/linea"],
    "mantle": ["https://mantle.api.pocket.network", "https://rpc.mantle.xyz", "https://rpc.ankr.com/mantle"],
    "manta": ["https://manta.api.pocket.network", "https://pacific-rpc.manta.network", "https://rpc.ankr.com/manta_pacific"],
    "kaia": ["https://kaia.api.pocket.network", "https://public-en.node.kaia.io", "https://rpc.ankr.com/kaia"],
    "kava": ["https://kava.api.pocket.network", "https://evm.kava.io", "https://rpc.ankr.com/kava_evm"],
    "harmony": ["https://harmony.api.pocket.network", "https://api.harmony.one", "https://rpc.ankr.com/harmony"],
    "fuse": ["https://fuse.api.pocket.network", "https://rpc.fuse.io", "https://rpc.ankr.com/fuse"],
    "oasis": ["https://oasis.api.pocket.network", "https://emerald.oasis.io", "https://rpc.ankr.com/oasis_emerald"],
    "polygon": ["https://polygon.drpc.org", "https://poly.api.pocket.network", "https://polygon.gateway.tenderly.co", "https://polygon-rpc.com", "https://rpc.ankr.com/polygon/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
    "optimism": ["https://optimism.drpc.org", "https://op.api.pocket.network", "https://optimism.gateway.tenderly.co", "https://mainnet.optimism.io", "https://rpc.ankr.com/optimism/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
    "gnosis": ["https://gnosis.drpc.org", "https://gnosis.api.pocket.network", "https://rpc.gnosischain.com", "https://rpc.ankr.com/gnosis/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
}

def _atomic_write_json(path, data, indent=None):
    d = os.path.dirname(os.path.abspath(path))
    fd, tmp = tempfile.mkstemp(prefix=".tmp_", dir=d)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=indent)
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, path)
        try:
            dfd = os.open(d, os.O_RDONLY)
            try:
                os.fsync(dfd)
            finally:
                os.close(dfd)
        except OSError:
            pass
    finally:
        if os.path.exists(tmp):
            try:
                os.remove(tmp)
            except OSError:
                pass


_SELECTOR_RE = re.compile(r"63([0-9a-f]{8})14")


def extract_selectors(code_hex):
    code_hex = code_hex.lower()
    sels = set()
    # Strip Solidity metadata tail (ipfs/cbor) to avoid scanning data region
    positions = []
    for mk in ("a26469706673", "64736f6c63"):
        p = code_hex.rfind(mk)
        if p != -1:
            positions.append(p)
    if positions:
        cut = min(positions)
        # Guard against false positives: strip when marker is in trailing region,
        # or close enough to the end to be metadata even on small contracts.
        if cut > 0 and cut >= len(code_hex) - 2048:
            code_hex = code_hex[:cut]
    n = len(code_hex)
    if n % 2:
        n -= 1
    for m in _SELECTOR_RE.finditer(code_hex[:n]):
        if m.start() % 2 != 0:
            continue
        sels.add("0x" + m.group(1))
    return sels


def match_bytecode_patterns(sels):
    out = []
    for pname, (req, opt) in BYTECODE_PATTERNS.items():
        if not req.issubset(sels):
            continue
        if opt:
            matched_opt = opt & sels
            score = len(matched_opt) / len(opt)
        else:
            score = 1.0
        out.append((pname, round(score, 2)))
    return out




async def scan_chain(chain: str, num_blocks: int, resume_from: Optional[int] = None, sem: Optional[asyncio.Semaphore] = None, fetch_sem: Optional[asyncio.Semaphore] = None, session: Optional[aiohttp.ClientSession] = None) -> List[Dict[str, Any]]:
    urls = SCAN_RPCS.get(chain, [])
    if not urls:
        print(f"[{chain}] No RPC endpoints configured")
        return []
    if resume_from is not None and resume_from < 0:
        print(f"[{chain}] Invalid resume block {resume_from}; scanning fresh")
        resume_from = None
    _own_session = False
    if session is None:
        connector = aiohttp.TCPConnector(limit=200, ttl_dns_cache=300)
        session = aiohttp.ClientSession(connector=connector)
        _own_session = True
    try:
        if sem is None:
            sem = asyncio.Semaphore(100)
        match_lock = asyncio.Lock()
        matches = []
        code_cache = {}
        pending_events = {}
        rpc_failures = 0
        rpc_lock = asyncio.Lock()
        url_lock = asyncio.Lock()
        if fetch_sem is None:
            fetch_sem = asyncio.Semaphore(50)
        request_timeout = aiohttp.ClientTimeout(total=15)
        code_cache_lock = asyncio.Lock()
        async def fetch_codes(addrs, blk_num):
            wait_events = []
            missing = []
            wait_seen = set()
            async with code_cache_lock:
                for a in addrs:
                    if a in code_cache:
                        continue
                    if a in pending_events:
                        if a not in wait_seen:
                            wait_events.append((a, pending_events[a]))
                            wait_seen.add(a)
                    else:
                        ev = asyncio.Event()
                        pending_events[a] = ev
                        missing.append(a)
                        wait_seen.add(a)
            updates = {}
            invalid = set()
            failures = set()
            try:
                if missing:
                    fetched = []
                    for i in range(0, len(missing), 50):
                        chunk = missing[i:i + 100]
                        fetched.extend(await asyncio.gather(*[fetch_single("eth_getCode", [a, hex(blk_num)]) for a in chunk], return_exceptions=True))
                else:
                    fetched = []
                for a, code in zip(missing, fetched):
                    if isinstance(code, str):
                        is_zero = False
                        if code.startswith("0x"):
                            hexpart = code[2:]
                            is_zero = (len(hexpart) == 0) or (not hexpart.strip("0"))
                        if code in ("", "0x") or is_zero:
                            invalid.add(a)
                        else:
                            updates[a] = code
                    else:
                        failures.add(a)
            finally:
                async with code_cache_lock:
                    for a, code in updates.items():
                        if a not in code_cache:
                            code_cache[a] = code
                    for a in invalid:
                        if a not in code_cache:
                            code_cache[a] = ""
                    # Do not permanently cache failures: allow retry on transient RPC errors
                    for a in missing:
                        ev = pending_events.pop(a, None)
                        if ev is not None:
                            ev.set()
            for _, ev in wait_events:
                await ev.wait()
            out = []
            async with code_cache_lock:
                for addr in addrs:
                    cached = code_cache.get(addr)
                    if isinstance(cached, str) and cached and cached != "":
                        out.append(cached)
                    else:
                        out.append(None)
            return out
        url_order = list(urls)

        async def fetch_single(method, params):
            nonlocal rpc_failures
            for attempt in range(2):
                async with url_lock:
                    snapshot = list(url_order)
                for i, u in enumerate(snapshot):
                    status_429 = False
                    try:
                        async with fetch_sem:
                            async with session.post(u, json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1}, timeout=request_timeout) as resp:
                                if resp.status == 429:
                                    status_429 = True
                                elif resp.status == 200:
                                    data = await resp.json(content_type=None)
                                    if isinstance(data, dict) and "result" in data:
                                        if i != 0:
                                            async with url_lock:
                                                if u in url_order:
                                                    url_order.remove(u)
                                                    url_order.insert(0, u)
                                        return data["result"]
                    except Exception:
                        continue
                    if status_429:
                        await asyncio.sleep(2.0)
                        continue
                if attempt < 1:
                    await asyncio.sleep(0.5 * (2 ** attempt))
            async with rpc_lock:
                rpc_failures += 1
            return None

        # Get latest block
        latest = None
        res = await fetch_single("eth_blockNumber", [])
        if res is not None:
            try:
                latest = int(res, 16) if isinstance(res, str) else int(res)
            except (TypeError, ValueError):
                pass
        if latest is None:
            print(f"[{chain}] Failed to get latest block on all endpoints")
            return []

        receipts_supported = False
        probe = await fetch_single("eth_getBlockReceipts", [hex(latest)])
        if isinstance(probe, list):
            receipts_supported = True

        if resume_from is not None:
            prog_file = os.path.join(ROOT, f"scan_progress_{chain}.json")
            try:
                with open(prog_file, encoding="utf-8") as f:
                    prog = json.load(f)
                seen2 = set()
                uniq = []
                for m in prog.get("matches", []) or []:
                    key = (m.get("chain"), m.get("address"))
                    if key in seen2:
                        continue
                    seen2.add(key)
                    uniq.append(m)
                matches[:] = uniq
                if "rpc_failures" in prog:
                    rpc_failures = prog["rpc_failures"]
            except Exception as e:
                print(f"[{chain}] progress read error: {e}")

        if resume_from is not None and resume_from >= latest:
            print(f"[{chain}] Already scanned to latest block")
            return matches

        if resume_from is not None and 0 <= resume_from < latest:
            start_block = resume_from + 1
            scan_range = min(num_blocks, latest - start_block + 1)
            if scan_range <= 0:
                print(f"[{chain}] Already scanned to latest block")
                return matches
            print(f"[{chain}] Latest block: {latest}, resuming from {start_block}, scanning {scan_range} blocks...")
        else:
            start_block = max(0, latest - num_blocks + 1)
            scan_range = min(num_blocks, latest - start_block + 1)
            if scan_range <= 0:
                print(f"[{chain}] Nothing to scan")
                return matches
            print(f"[{chain}] Latest block: {latest}, scanning {scan_range} blocks...")

        async def resolve_proxy_impl(addr, code, blk_num):
            ch = code[2:].lower() if isinstance(code, str) and code.startswith("0x") else (code.lower() if isinstance(code, str) else "")
            impl_addr = None
            if ch.startswith("363d3d373d3d3d363d73") and len(ch) >= 90 and ch[60:90] == "5af43d82803e903d91602b57fd5bf3":
                impl_addr = "0x" + ch[20:60].lower()
            elif len(ch) // 2 <= 45 and any(ch[i:i+2] == "f4" for i in range(0, len(ch), 2)):
                slot = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
                val = await fetch_single("eth_getStorageAt", [addr, slot, hex(blk_num)])
                if isinstance(val, str) and len(val) >= 66:
                    impl_addr = "0x" + val[-40:].lower()
                    if impl_addr == "0x" + "0" * 40:
                        impl_addr = None
            if not impl_addr or impl_addr == "0x" + "0" * 40:
                return None
            # Coalesce concurrent implementation fetches
            _cached_hit = None
            wait = False
            async with code_cache_lock:
                cached = code_cache.get(impl_addr.lower())
                if isinstance(cached, str) and cached and cached != "0x" and cached[2:].strip("0") != "":
                    _cached_hit = cached
                elif impl_addr.lower() in pending_events:
                    ev = pending_events[impl_addr.lower()]
                    wait = True
                else:
                    ev = asyncio.Event()
                    pending_events[impl_addr.lower()] = ev
                    wait = False
            if _cached_hit is not None:
                return _cached_hit
            if wait:
                await ev.wait()
                async with code_cache_lock:
                    cached = code_cache.get(impl_addr.lower())
                if isinstance(cached, str) and cached and cached != "0x" and cached[2:].strip("0") != "":
                    return cached
                return None
            try:
                impl_code = await fetch_single("eth_getCode", [impl_addr, hex(blk_num)])
                if isinstance(impl_code, str) and impl_code and impl_code != "0x" and len(impl_code) >= 10 and impl_code[2:].strip("0") != "":
                    async with code_cache_lock:
                        code_cache[impl_addr.lower()] = impl_code
                    return impl_code
                return None
            finally:
                async with code_cache_lock:
                    e = pending_events.pop(impl_addr.lower(), None)
                    if e is not None:
                        e.set()

        async def process_block(blk_num):
            async with sem:
                receipts = await fetch_single("eth_getBlockReceipts", [hex(blk_num)]) if receipts_supported else None
                if isinstance(receipts, list):
                    jobs = [r for r in receipts if isinstance(r, dict) and r.get("contractAddress")]
                    for r in jobs:
                        r["contractAddress"] = str(r["contractAddress"]).lower()
                    addrs = [r["contractAddress"] for r in jobs]
                    codes = await fetch_codes(addrs, blk_num)
                    for r, code in zip(jobs, codes):
                        if not isinstance(code, str) or not code or code == "0x" or len(code) < 10 or code[2:].strip("0") == "":
                            continue
                        addr = r["contractAddress"]
                        sels = extract_selectors(code[2:])
                        pattern_matches = match_bytecode_patterns(sels)
                        if not pattern_matches:
                            impl_code = await resolve_proxy_impl(addr, code, blk_num)
                            if impl_code:
                                sels = extract_selectors(impl_code[2:])
                                pattern_matches = match_bytecode_patterns(sels)
                        if pattern_matches:
                            m = {"chain": chain, "block": blk_num, "address": addr, "tx": r.get("transactionHash", ""), "selectors": sorted(sels), "patterns": [p[0] for p in pattern_matches], "scores": {p[0]: p[1] for p in pattern_matches}}
                            async with match_lock:
                                matches.append(m)
                            for pname, score in pattern_matches:
                                print(f"  [{chain}] PATTERN MATCH: block {blk_num} addr {addr} -> {pname} (score={score})")
                    return
                block = await fetch_single("eth_getBlockByNumber", [hex(blk_num), True])
                if not block or not block.get("transactions"):
                    return
                creation_txs = [tx for tx in block["transactions"] if isinstance(tx, dict) and (tx.get("to") is None or tx.get("to") in ("", "0x") or (isinstance(tx.get("to"), str) and tx.get("to").lower() == "0x" + "0" * 40))]
                if creation_txs:
                    fetched_receipts = await asyncio.gather(*[fetch_single("eth_getTransactionReceipt", [tx.get("hash")]) for tx in creation_txs], return_exceptions=True)
                else:
                    fetched_receipts = []
                code_jobs = []
                tx_receipt_pairs = []
                for tx, receipt in zip(creation_txs, fetched_receipts):
                    if isinstance(receipt, Exception) or not isinstance(receipt, dict) or not receipt.get("contractAddress"):
                        continue
                    receipt["contractAddress"] = str(receipt["contractAddress"]).lower()
                    tx_receipt_pairs.append((tx, receipt))
                    code_jobs.append(receipt["contractAddress"])
                codes = await fetch_codes(code_jobs, blk_num)
                for (tx, receipt), code in zip(tx_receipt_pairs, codes):
                    if not isinstance(code, str) or not code or code == "0x" or len(code) < 10 or code[2:].strip("0") == "":
                        continue
                    addr = receipt["contractAddress"]
                    sels = extract_selectors(code[2:])
                    pattern_matches = match_bytecode_patterns(sels)
                    if not pattern_matches:
                        impl_code = await resolve_proxy_impl(addr, code, blk_num)
                        if impl_code:
                            sels = extract_selectors(impl_code[2:])
                            pattern_matches = match_bytecode_patterns(sels)
                    if pattern_matches:
                        m = {"chain": chain, "block": blk_num, "address": addr, "tx": tx.get("hash", ""), "selectors": sorted(sels), "patterns": [p[0] for p in pattern_matches], "scores": {p[0]: p[1] for p in pattern_matches}}
                        async with match_lock:
                            matches.append(m)
                        for pname, score in pattern_matches:
                            print(f"  [{chain}] PATTERN MATCH: block {blk_num} addr {addr} -> {pname} (score={score})")

        # Batch processing: process blocks in batches
        batch_size = 50
        last_pm = -1
        last_pf = -1
        for start in range(start_block, start_block + scan_range, batch_size):
            end = min(start + batch_size, start_block + scan_range)
            tasks = [process_block(blk) for blk in range(start, end)]
            results = await asyncio.gather(*tasks, return_exceptions=True)
            for blk, res in zip(range(start, end), results):
                if isinstance(res, Exception):
                    print(f"  [{chain}] block {blk} error: {res}")
            async with match_lock:
                _mc = len(matches)
                _snapshot = list(matches)
            async with rpc_lock:
                _rf = rpc_failures
            print(f"  [{chain}] scanned to block {end - 1}, {_mc} matches, {_rf} rpc failures")
            prog_path = os.path.join(ROOT, f"scan_progress_{chain}.json")
            await asyncio.get_running_loop().run_in_executor(
                None, _atomic_write_json, prog_path,
                 {"chain": chain, "last_block": end - 1, "matches": _snapshot, "rpc_failures": rpc_failures}, None)
            last_pm = _mc
            last_pf = rpc_failures

        print(f"[{chain}] Scan complete: {len(matches)} matches, {rpc_failures} rpc failures")
        return matches
    finally:
        if _own_session:
            await session.close()

def cmd_scan(args):
    if args.blocks <= 0:
        print("blocks must be > 0")
        return
    print(f"Scanning for {len(BYTECODE_PATTERNS)} bytecode patterns...")
    raw = list(SCAN_RPCS.keys()) if args.chains.strip().lower() == "all" else [c.strip().lower() for c in args.chains.split(",")]
    chains = []
    for c in raw:
        if c not in SCAN_RPCS:
            print(f"[{c}] Unknown chain, skipping")
            continue
        safe = "".join(ch for ch in c if ch.isascii() and (ch.isalnum() or ch in "._-"))
        if safe != c:
            print(f"[{c}] Invalid chain name, skipping")
            continue
        chains.append(c)

    async def run():
        tasks = []
        resume_map = {}
        global_sem = asyncio.Semaphore(200)
        global_fetch_sem = asyncio.Semaphore(100)
        for c in chains:
            prog_file = os.path.join(ROOT, f"scan_progress_{c}.json")
            rf = None
            if os.path.exists(prog_file):
                try:
                    with open(prog_file, encoding="utf-8") as f:
                        progress = json.load(f)
                    rf = progress.get("last_block")
                    if rf is not None:
                        print(f"[{c}] Resuming from block {rf}")
                except Exception as e:
                    print(f"[{c}] progress read error: {e}")
            resume_map[c] = rf
        connector = aiohttp.TCPConnector(limit=200, ttl_dns_cache=300)
        async with aiohttp.ClientSession(connector=connector) as session:
            for c in chains:
                tasks.append(scan_chain(c, args.blocks, resume_map.get(c), global_sem, global_fetch_sem, session))
            results = await asyncio.gather(*tasks, return_exceptions=True)
        all_matches = []
        for r in results:
            if isinstance(r, Exception):
                print(f"Chain scan error: {r}")
                continue
            if isinstance(r, list):
                all_matches.extend(r)
        # Merge with any previously saved results to avoid data loss on resume
        try:
            if os.path.exists(RESULTS_FILE):
                with open(RESULTS_FILE, encoding="utf-8") as f:
                    prev = json.load(f)
                if isinstance(prev, list):
                    seen_keys = {(m.get("chain"), m.get("address")) for m in all_matches}
                    for m in prev:
                        if (m.get("chain"), m.get("address")) not in seen_keys:
                            all_matches.append(m)
                            seen_keys.add((m.get("chain"), m.get("address")))
        except Exception as e:
            print(f"results merge error: {e}")
        seen = set()
        unique_matches = []
        for m in all_matches:
            key = (m.get("chain"), m.get("address"))
            if key not in seen:
                seen.add(key)
                unique_matches.append(m)
        await asyncio.get_running_loop().run_in_executor(None, _atomic_write_json, RESULTS_FILE, unique_matches, 2)
        print(f"\nTotal matches across all chains: {len(unique_matches)}")
        if all_matches:
            print("KRW profit estimation requires forge reproduction of each matched contract.")
            print("Approx rates: USDC/USDT/DAI=1363, ETH=3053802, BNB=946619 KRW")
        pattern_count = sum(1 for m in unique_matches if m.get("patterns"))
        if pattern_count:
            print(f"Pattern matches: {pattern_count}")
            for m in unique_matches:
                if m.get("patterns"):
                    print(f"  [{m.get('chain')}] {m.get('address')} -> {', '.join(m.get('patterns', []))} (profit requires forge reproduction)")
        print(f"Unique contracts: {len(unique_matches)}")
        for m in unique_matches:
            print(f"  [{m.get('chain')}] {m.get('address')} (block {m.get('block')}) patterns: {m.get('patterns', [])}")

    asyncio.run(run())


def main():
    p = argparse.ArgumentParser(description="Defi pipeline: multi-chain block scanner for hack-related contracts")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("scan", help="Scan blocks across chains for hack-related contracts")
    s.add_argument("--chains", default="all", help="Comma-separated chains or 'all'")
    s.add_argument("--blocks", type=int, default=5000, help="Number of blocks to scan")
    s.set_defaults(func=cmd_scan)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
