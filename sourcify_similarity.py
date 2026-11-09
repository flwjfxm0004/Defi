#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Address -> Runtime Bytecode -> Sourcify Similarity Contract Search"""
import asyncio
import json
import os
import re
import sys
from typing import Any, Dict, List, Optional, Tuple

import aiohttp

ROOT = os.path.dirname(os.path.abspath(__file__))
# 해킹.md 파일 자동 찾기 (가장 큰 .md 파일 선택)
import glob
md_files = glob.glob(os.path.join(ROOT, "*.md"))
if not md_files:
    raise FileNotFoundError("No .md file found in " + ROOT)
# 가장 큰 파일을 해킹.md로 선택 (주소가 많은 파일)
HACK_FILE = max(md_files, key=lambda p: os.path.getsize(p))
RESULTS_FILE = os.path.join(ROOT, "sourcify_similarity_results.json")

ANKR_KEY = "f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"
TATUM_KEY = "bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"
NODEREAL_KEY = "e7ec74ea77134e99b3aca76de8e602c5"

CHAIN_ID_MAP: Dict[str, int] = {
    "ethereum": 1,
    "bsc": 56,
    "base": 8453,
    "arbitrum": 42161,
    "polygon": 137,
    "avalanche": 43114,
    "optimism": 10,
    "gnosis": 100,
    "scroll": 534352,
    "linea": 59141,
    "zksync-era": 324,
    "berachain": 80094,
    "ink": 571,
    "unichain": 130,
    "hyperliquid": 999,
    "fraxtal": 252,
    "sonic": 146,
    "celo": 42220,
    "kaia": 8217,
    "kava": 2222,
    "harmony": 1666600000,
    "fuse": 122,
    "oasis": 42262,
    "iotex": 4689,
    "robinhood": 998000001,
    "oasys": 5791,
    "opbnb": 204,
    "manta": 169,
    "mantle": 5000,
    "metis": 1088,
    "moonbeam": 1284,
    "moonriver": 1285,
    "boba": 288,
    "blast": 81457,
    "poly-zkevm": 1101,
    "xrplevm": 400,
    "cronos": 25,
    "flow-evm": 475,
    "core": 1116,
}

SCAN_RPCS: Dict[str, List[str]] = {
    "ethereum": [
        f"https://rpc.ankr.com/eth/{ANKR_KEY}",
        f"https://ethereum-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.nodereal.io/v1/{NODEREAL_KEY}",
        "https://ethereum-rpc.blockreq.com/v1/rpc/public",
        "https://gateway.tenderly.co/public/mainnet",
        "https://rpc.mevblocker.io",
    ],
    "bsc": [
        f"https://bsc-mainnet.nodereal.io/v1/{NODEREAL_KEY}",
        f"https://bsc-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.ankr.com/bsc/{ANKR_KEY}",
        "https://bsc-rpc.blockreq.com/v1/rpc/public",
        "https://bsc.drpc.org",
    ],
    "base": [
        f"https://base-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.ankr.com/base/{ANKR_KEY}",
        "https://base-rpc.blockreq.com/v1/rpc/public",
        "https://mainnet.base.org",
    ],
    "arbitrum": [
        f"https://arbitrum-one-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}",
        f"https://rpc.ankr.com/arbitrum/{ANKR_KEY}",
        "https://arbitrum-rpc.blockreq.com/v1/rpc/public",
        "https://arb1.arbitrum.io/rpc",
    ],
    "polygon": [
        f"https://polygon-rpc.blockreq.com/v1/rpc/public",
        f"https://rpc.ankr.com/polygon/{ANKR_KEY}",
        "https://polygon.drpc.org",
        "https://polygon-rpc.com",
    ],
    "avalanche": [
        f"https://rpc.ankr.com/avalanche/{ANKR_KEY}",
        "https://avax.drpc.org",
        "https://api.avax.network/ext/bc/C/rpc",
    ],
    "optimism": [
        f"https://rpc.ankr.com/optimism/{ANKR_KEY}",
        "https://optimism.drpc.org",
        "https://mainnet.optimism.io",
    ],
    "gnosis": [
        f"https://rpc.ankr.com/gnosis/{ANKR_KEY}",
        "https://gnosis.drpc.org",
        "https://rpc.gnosischain.com",
    ],
    "scroll": [
        f"https://rpc.ankr.com/scroll/{ANKR_KEY}",
        "https://scroll.drpc.org",
        "https://rpc.scroll.io",
    ],
    "linea": [
        f"https://rpc.ankr.com/linea/{ANKR_KEY}",
        "https://linea.drpc.org",
        "https://rpc.linea.build",
    ],
    "zksync-era": [
        f"https://rpc.ankr.com/zksync_era/{ANKR_KEY}",
        "https://zksync-era.drpc.org",
        "https://mainnet.era.zksync.io",
    ],
    "berachain": [
        f"https://rpc.ankr.com/berachain/{ANKR_KEY}",
        "https://rpc.berachain.com",
    ],
    "ink": [
        f"https://rpc.ankr.com/ink/{ANKR_KEY}",
        "https://rpc-gel.inkonchain.com",
    ],
    "unichain": [
        f"https://rpc.ankr.com/unichain/{ANKR_KEY}",
        "https://unichain.drpc.org",
        "https://mainnet.unichain.org",
    ],
    "hyperliquid": [
        f"https://rpc.ankr.com/hyperliquid/{ANKR_KEY}",
        "https://rpc.hyperliquid.xyz/evm",
    ],
    "fraxtal": [
        f"https://rpc.ankr.com/fraxtal/{ANKR_KEY}",
        "https://rpc.frax.com",
    ],
    "sonic": [
        f"https://rpc.ankr.com/sonic/{ANKR_KEY}",
        "https://rpc.soniclabs.com",
    ],
    "celo": [
        f"https://rpc.ankr.com/celo/{ANKR_KEY}",
        "https://forno.celo.org",
    ],
    "kaia": [
        f"https://rpc.ankr.com/kaia/{ANKR_KEY}",
        "https://public-en.node.kaia.io",
    ],
    "kava": [
        f"https://rpc.ankr.com/kava_evm/{ANKR_KEY}",
        "https://evm.kava.io",
    ],
    "harmony": [
        f"https://rpc.ankr.com/harmony/{ANKR_KEY}",
        "https://api.harmony.one",
    ],
    "fuse": [
        f"https://rpc.ankr.com/fuse/{ANKR_KEY}",
        "https://rpc.fuse.io",
    ],
    "oasis": [
        f"https://rpc.ankr.com/oasis_emerald/{ANKR_KEY}",
        "https://emerald.oasis.io",
    ],
    "iotex": [
        f"https://rpc.ankr.com/iotex/{ANKR_KEY}",
        "https://babel-api.mainnet.iotex.io",
    ],
    "robinhood": [
        f"https://rpc.ankr.com/robinhood/{ANKR_KEY}",
        "https://rpc.robinhood.com",
    ],
    "oasys": [
        f"https://rpc.ankr.com/oasys/{ANKR_KEY}",
        "https://oasys.1rpc.io",
    ],
    "opbnb": [
        f"https://rpc.ankr.com/opbnb/{ANKR_KEY}",
        "https://opbnb-mainnet-rpc.bnbchain.org",
    ],
    "manta": [
        f"https://rpc.ankr.com/manta_pacific/{ANKR_KEY}",
        "https://pacific-rpc.manta.network",
    ],
    "mantle": [
        f"https://rpc.ankr.com/mantle/{ANKR_KEY}",
        "https://rpc.mantle.xyz",
    ],
    "metis": [
        f"https://rpc.ankr.com/metis/{ANKR_KEY}",
        "https://andromeda.metis.io/?owner=1088",
    ],
    "moonbeam": [
        f"https://rpc.ankr.com/moonbeam/{ANKR_KEY}",
        "https://rpc.api.moonbeam.network",
    ],
    "moonriver": [
        f"https://rpc.ankr.com/moonriver/{ANKR_KEY}",
        "https://rpc.api.moonriver.io",
    ],
    "boba": [
        f"https://rpc.ankr.com/boba/{ANKR_KEY}",
        "https://mainnet.boba.network",
    ],
    "blast": [
        f"https://rpc.ankr.com/blast/{ANKR_KEY}",
        "https://rpc.blast.io",
    ],
    "poly-zkevm": [
        f"https://rpc.ankr.com/polygon_zkevm/{ANKR_KEY}",
        "https://polygon-zkevm.1rpc.io",
    ],
    "xrplevm": [
        f"https://rpc.ankr.com/xrplevm/{ANKR_KEY}",
        "https://xrplevm.1rpc.io",
    ],
    "cronos": [
        "https://evm.cronos.org",
        "https://cronos-evm-rpc.publicnode.com",
        "https://cronos.drpc.org",
    ],
    "flow-evm": [
        "https://mainnet.evm.nodes.onflow.org",
    ],
    "core": [
        "https://rpc.coredao.org",
        "https://core.drpc.org",
    ],
}

ADDRESS_RE = re.compile(rb"0x[a-fA-F0-9]{40}")

# 해킹.md를 바이너리 모드로 읽어 "취약 컨트랙트" UTF-8 바이트 패턴으로 찾기
# 취약 컨트랙트 UTF-8: ec b7 a8 ec 95 bd 20 ec bb a8 ed 8a b8 eb 9e 99 ed 8a b8
VULN_LABEL_BYTES = b"\xec\xb7\xa8\xec\x95\xbd \xec\xbb\xa8\xed\x8a\xb8\xeb\x9e\x99\xed\x8a\xb8"

def parse_hack_md(path: str) -> List[Tuple[str, str, str]]:
    """Extract (chain, address, description) from hack.md (binary read)"""
    results: List[Tuple[str, str, str]] = []
    with open(path, "rb") as f:
        content = f.read()
    
    print(f"  DEBUG: File size: {len(content)} bytes")
    print(f"  DEBUG: VULN_LABEL_BYTES hex: {VULN_LABEL_BYTES.hex()}")
    idx = content.find(VULN_LABEL_BYTES)
    print(f"  DEBUG: Label found at index: {idx}")
    if idx >= 0:
        print(f"  DEBUG: Context: {content[idx:idx+80]}")
    
    # 전체 내용에서 취약 컨트랙트 주소 찾기 (블록 분할 없이)
    for m in re.finditer(VULN_LABEL_BYTES + rb"[:\s]*[\`]*\s*([0-9a-fA-Fx]{42})", content):
        addr = m.group(1).decode("ascii")
        # 주소 주변에서 체인 이름 찾기 (괄호 안에 있는 체인명)
        start = max(0, m.start() - 200)
        context = content[start:m.start()]
        chain_match = re.search(rb"\(([^)]+)\)", context)
        chain = ""
        if chain_match:
            paren_content = chain_match.group(1)
            parts = [p.strip() for p in paren_content.split(b",")]
            if parts:
                chain = parts[-1].decode("utf-8", errors="ignore").lower()
        
        if ADDRESS_RE.fullmatch(addr.encode("ascii")):
            results.append((chain, addr.lower(), ""))
    
    return results

async def eth_get_code(session: aiohttp.ClientSession, chain: str, address: str) -> Optional[str]:
    """Get runtime bytecode from specified chain RPC"""
    rpcs = SCAN_RPCS.get(chain, [])
    if not rpcs:
        return None
    
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [address, "latest"]
    }
    
    for rpc in rpcs:
        try:
            async with session.post(rpc, json=payload, timeout=aiohttp.ClientTimeout(total=15)) as r:
                d = await r.json()
                code = d.get("result", "")
                if code and code != "0x":
                    return code
        except Exception:
            continue
    return None

async def sourcify_similarity(session: aiohttp.ClientSession, chain_id: int, address: str) -> Dict[str, Any]:
    """Sourcify HTTP API v2 similarity search (address based)"""
    url = f"https://sourcify.dev/server/v2/verify/similarity/{chain_id}/{address}"
    headers = {"Content-Type": "application/json"}
    
    try:
        async with session.post(url, headers=headers, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            try:
                d = json.loads(text)
            except:
                return {"status": r.status, "error": f"invalid_json: {text[:200]}"}
            
            if r.status == 200:
                vid = d.get("verificationId") or d.get("id")
                if vid:
                    for i in range(60):
                        await asyncio.sleep(1)
                        try:
                            async with session.get(f"https://sourcify.dev/server/v2/verify/{vid}", timeout=aiohttp.ClientTimeout(total=10)) as poll_r:
                                poll_text = await poll_r.text()
                                try:
                                    p_d = json.loads(poll_text)
                                    p_status = p_d.get("status", "unknown")
                                    if p_status in ("finished", "completed", "done", "error", "failed"):
                                        return {"status": p_status, "data": p_d}
                                except:
                                    pass
                        except:
                            pass
                    return {"status": "timeout", "data": d}
                return {"status": "no_id", "data": d}
            else:
                return {"status": r.status, "error": d.get("message", text[:200])}
    except Exception as e:
        return {"status": "exception", "error": str(e)[:200]}

async def main():
    print("=== Address -> Runtime Bytecode -> Sourcify Similarity Search ===\n")
    print(f"HACK_FILE: {HACK_FILE}")
    
    # Step 1: Extract vulnerable contract addresses from hack.md
    print("[Step 1] Extracting vulnerable contract addresses from hack.md")
    seeds = parse_hack_md(HACK_FILE)
    print(f"  Found vulnerable contracts: {len(seeds)}")
    for chain, addr, desc in seeds:
        print(f"    [{chain}] {addr} - {desc[:80]}")
    
    # Step 2: Get runtime bytecode for each address
    print(f"\n[Step 2] Fetching runtime bytecode (eth_getCode)")
    seed_codes: List[Tuple[str, str, str, str, Optional[str]]] = []
    
    async with aiohttp.ClientSession() as session:
        tasks = []
        for chain, addr, desc in seeds:
            tasks.append(eth_get_code(session, chain, addr))
        
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        for i, (chain, addr, desc) in enumerate(seeds):
            code = results[i] if not isinstance(results[i], Exception) else None
            chain_id = CHAIN_ID_MAP.get(chain, 0)
            seed_codes.append((chain, addr, desc, str(chain_id), code))
            status = "OK" if code and code != "0x" else "FAIL/EMPTY"
            print(f"  [{chain}] {addr} -> {status} ({len(code)//2 if code else 0} bytes)")
    
    # Step 3: Sourcify similarity search
    print(f"\n[Step 3] Sourcify similar contract search")
    
    all_results: List[Dict[str, Any]] = []
    
    async with aiohttp.ClientSession() as session:
        for chain, addr, desc, chain_id, bytecode in seed_codes:
            if not bytecode or bytecode == "0x":
                print(f"  [{chain}] {addr}: no bytecode - skip")
                continue
            
            print(f"  [{chain}] {addr} (chainId={chain_id}) -> Sourcify similarity search...")
            result = await sourcify_similarity(session, int(chain_id), addr)
            
            entry = {
                "chain": chain,
                "chain_id": chain_id,
                "address": addr,
                "description": desc,
                "bytecode_length": len(bytecode) // 2 if bytecode else 0,
                "sourcify_result": result,
            }
            all_results.append(entry)
            
            status = result.get("status", "unknown")
            if status in ("finished", "completed", "done"):
                data = result.get("data", {})
                print(f"    -> Done: {json.dumps(data, ensure_ascii=False)[:500]}")
            elif status == 409:
                print(f"    -> Already verified contract (409)")
            elif status == 404:
                print(f"    -> No bytecode at address (404)")
            else:
                print(f"    -> Status: {status}, Error: {result.get('error', '')[:100]}")
    
    # Step 4: Save results
    print(f"\n[Step 4] Saving results")
    output = {
        "total_seeds": len(seeds),
        "results": all_results,
    }
    
    with open(RESULTS_FILE, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
    
    print(f"  Result file: {RESULTS_FILE}")
    print(f"  Search completed: {len(all_results)} contracts")

if __name__ == "__main__":
    asyncio.run(main())
