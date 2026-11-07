#!/usr/bin/env python3
"""Defi pipeline: contract analysis, exploit replay, and multi-chain block scanner."""
import argparse
import asyncio
import json
import os
import aiohttp
import requests

ROOT = os.path.dirname(os.path.abspath(__file__))

CHAINS = {
    "avalanche": {"rpc": "https://api.avax.network/ext/bc/C/rpc", "usdc": "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E", "token": "usd-coin"},
    "ethereum": {"rpc": "https://ethereum-rpc.publicnode.com", "usdc": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "token": "usd-coin"},
    "bsc": {"rpc": "https://bsc-mainnet.nodereal.io/v1/e7ec74ea77134e99b3aca76de8e602c5", "usdc": "0x55d398326f99059fF775485246999027B3197955", "token": "tether"},
    "base": {"rpc": "https://mainnet.base.org", "usdc": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "token": "usd-coin"},
    "arbitrum": {"rpc": "https://arb1.arbitrum.io/rpc", "usdc": "0xaf88d065e77c8cC2239327C5EDb3A432268e5831", "token": "usd-coin"},
    "polygon": {"rpc": "https://polygon-rpc.com", "usdc": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359", "token": "usd-coin"},
    "optimism": {"rpc": "https://mainnet.optimism.io", "usdc": "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85", "token": "usd-coin"},
    "gnosis": {"rpc": "https://rpc.gnosischain.com", "usdc": "0xDDAfbb505ad214D7b80b1f830fcCc89B597556F4", "token": "usd-coin"},
    "celo": {"rpc": "https://forno.celo.org", "usdc": "0xcebA9300f2b948710d030a45609c9521b4113C30", "token": "usd-coin"},
    "iotex": {"rpc": "https://babel-api.mainnet.iotex.io", "usdc": "0x3B2bf2731427C534934653C4359d30307b115A80", "token": "usd-coin"},
    "moonbeam": {"rpc": "https://rpc.api.moonbeam.network", "usdc": "0x931715FEE2d06333043d11F658C8CE934aC61D0c", "token": "usd-coin"},
    "metis": {"rpc": "https://andromeda.metis.io/?owner=1088", "usdc": "0xEA32A96608495e54156Ae48931A7c20f0dcc1a21", "token": "usd-coin"},
    "moonriver": {"rpc": "https://rpc.api.moonriver.io", "usdc": "0xE3F5a90F9cb311505cd691a43596B292F8108379", "token": "usd-coin"},
    "fraxtal": {"rpc": "https://rpc.frax.com", "usdc": "0x51fA9444b37A150E1F312e3013234c1957d1D5E1", "token": "usd-coin"},
    "sonic": {"rpc": "https://rpc.soniclabs.com", "usdc": "0x29219dd400f2Bf60E5a23d13Be72B486D4038894", "token": "usd-coin"},
    "bera": {"rpc": "https://rpc.berachain.com", "usdc": "0x549943e04f40284185054145c6E4e9568C2682d5", "token": "usd-coin"},
    "hyperliquid": {"rpc": "https://rpc.hyperliquid.xyz/evm", "usdc": "0xb88339CB7199b77E23DB6E890353E22632Ba630f", "token": "usd-coin"},
    "ink": {"rpc": "https://rpc-gel.inkonchain.com", "usdc": "0x2D270e6886d130D724215A266106e683244f279b", "token": "usd-coin"},
    "scroll": {"rpc": "https://rpc.scroll.io", "usdc": "0x06eFdBFf2a14a7c8E15944D1F4A48F99776E49a5", "token": "usd-coin"},
    "robinhood": {"rpc": "https://rpc.robinhood.com", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "iotex": {"rpc": "https://iotex.api.pocket.network", "usdc": "0x3B2bf2731427C534934653C4359d30307b115A80", "token": "usd-coin"},
    "moonbeam": {"rpc": "https://moonbeam.api.pocket.network", "usdc": "0x931715FEE2d06333043d11F658C8CE934aC61D0c", "token": "usd-coin"},
    "metis": {"rpc": "https://metis.api.pocket.network", "usdc": "0xEA32A96608495e54156Ae48931A7c20f0dcc1a21", "token": "usd-coin"},
    "moonriver": {"rpc": "https://moonriver.api.pocket.network", "usdc": "0xE3F5a90F9cb311505cd691a43596B292F8108379", "token": "usd-coin"},
    "fraxtal": {"rpc": "https://fraxtal.api.pocket.network", "usdc": "0x51fA9444b37A150E1F312e3013234c1957d1D5E1", "token": "usd-coin"},
    "sonic": {"rpc": "https://sonic.api.pocket.network", "usdc": "0x29219dd400f2Bf60E5a23d13Be72B486D4038894", "token": "usd-coin"},
    "bera": {"rpc": "https://bera.api.pocket.network", "usdc": "0x549943e04f40284185054145c6E4e9568C2682d5", "token": "usd-coin"},
    "hyperliquid": {"rpc": "https://hyperliquid.api.pocket.network", "usdc": "0xb88339CB7199b77E23DB6E890353E22632Ba630f", "token": "usd-coin"},
    "ink": {"rpc": "https://ink.api.pocket.network", "usdc": "0x2D270e6886d130D724215A266106e683244f279b", "token": "usd-coin"},
    "poly-zkevm": {"rpc": "https://poly-zkevm.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "scroll": {"rpc": "https://scroll.api.pocket.network", "usdc": "0x06eFdBFf2a14a7c8E15944D1F4A48F99776E49a5", "token": "usd-coin"},
    "robinhood": {"rpc": "https://robinhood.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "celo": {"rpc": "https://celo.api.pocket.network", "usdc": "0xcebA9300f2b948710d030a45609c9521b4113C30", "token": "usd-coin"},
    "oasys": {"rpc": "https://oasys.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "opbnb": {"rpc": "https://opbnb.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "unichain": {"rpc": "https://unichain.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "xrplevm": {"rpc": "https://xrplevm.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "zksync-era": {"rpc": "https://zksync-era.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "boba": {"rpc": "https://boba.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "blast": {"rpc": "https://blast.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "linea": {"rpc": "https://linea.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "mantle": {"rpc": "https://mantle.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "manta": {"rpc": "https://manta.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "kaia": {"rpc": "https://kaia.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "kava": {"rpc": "https://kava.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "harmony": {"rpc": "https://harmony.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "fuse": {"rpc": "https://fuse.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
    "oasis": {"rpc": "https://oasis.api.pocket.network", "usdc": "0x0000000000000000000000000000000000000000", "token": "usd-coin"},
}




# Generic ERC20 selectors to filter out (not hack-related)
GENERIC_ERC20 = {"0xa9059cbb", "0x095ea7b3", "0x23b872dd", "0x70a08231", "0x18160ddd", "0x06fdde03", "0x95d89b41", "0x313ce567"}

# Bytecode patterns: sets of selectors that must ALL be present (AND logic)
# More specific than single selector matching
# Bytecode patterns: required (must have) + optional (boosts similarity score)
BYTECODE_PATTERNS = {
    "NEM": ({"0xbbafc98c"}, set()),
    "Balancer": ({"0x6d06dfa0"}, {"0x46ab38f1", "0xb02f0b73", "0xf8b2cb4f", "0x4bb278f3", "0xe4e1e538", "0x8201aa3f", "0x7c5e9ea4"}),
    "TranchePool": ({"0x8e27c105"}, set()),
    "Slice": ({"0x9d2edfac"}, set()),
    "TENDRIL": ({"0x9c74daf0"}, {"0xedae876f", "0xb734e98b"}),
    "InsurancePool": ({"0x01681a62"}, {"0x09c5eabe", "0x13a1a562"}),
    "Moonwell": ({"0x18bf5077"}, {"0x4bf5d7e9", "0x6fcfff45"}),
    "Hifi": ({"0x0f32f1c9"}, {"0x7adbf973", "0xc5ed81f7"}),
    "Premia": ({"0x531aa03e"}, {"0x6cee3448", "0xd41dcbea"}),
    "X100": ({"0xa4376352"}, {"0x3724fc1c", "0x4e71d92d"}),
    "zkLink": ({"0x68871c9c"}, {"0xbf22d9c3"}),
    "Yapeswap": ({"0xa2e74af6"}, set()),
}

# Core hack selectors (required for exploit, not just auxiliary functions)
CORE_HACK_SELECTORS = {
    "0xbbafc98c",  # NEM.flushLpSellRecycle
    "0x6d06dfa0",  # Balancer.joinswapPoolAmountOut
    "0x8e27c105",  # TranchePool.draw
    "0x9d2edfac",  # Slice.release
}

SCAN_RPCS = {
    "bsc": ["https://bsc.api.pocket.network", "https://bsc-mainnet.nodereal.io/v1/e7ec74ea77134e99b3aca76de8e602c5", "https://rpc.ankr.com/bsc/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://bsc-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "ethereum": ["https://eth.api.pocket.network", "https://ethereum-public.nodies.app", "https://rpc.mevblocker.io", "https://rpc.mevblocker.io/fast", "https://rpc.mevblocker.io/fullprivacy", "https://eth.meowrpc.com", "https://rpc.flashbots.net", "https://mainnet.gateway.tenderly.co", "https://mainnet.gateway.tenderly.co", "https://ethereum-rpc.publicnode.com", "https://rpc.ankr.com/eth/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://ethereum-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "base": ["https://base.api.pocket.network", "https://base.gateway.tenderly.co", "https://1rpc.io/base", "https://mainnet.base.org", "https://rpc.ankr.com/base/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://base-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "arbitrum": ["https://arb-one.api.pocket.network", "https://arbitrum.gateway.tenderly.co", "https://arb1.arbitrum.io/rpc", "https://rpc.ankr.com/arbitrum/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f", "https://arbitrum-one-mainnet.gateway.tatum.io/bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"],
    "avalanche": ["https://avax.api.pocket.network", "https://avalanche.gateway.tenderly.co", "https://api.avax.network/ext/bc/C/rpc", "https://rpc.ankr.com/avalanche/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
    "iotex": ["https://iotex.api.pocket.network"],
    "moonbeam": ["https://moonbeam.api.pocket.network"],
    "metis": ["https://metis.api.pocket.network"],
    "moonriver": ["https://moonriver.api.pocket.network"],
    "fraxtal": ["https://fraxtal.api.pocket.network"],
    "sonic": ["https://sonic.api.pocket.network"],
    "bera": ["https://bera.api.pocket.network"],
    "hyperliquid": ["https://hyperliquid.api.pocket.network"],
    "ink": ["https://ink.api.pocket.network"],
    "poly-zkevm": ["https://poly-zkevm.api.pocket.network"],
    "scroll": ["https://scroll.api.pocket.network"],
    "robinhood": ["https://robinhood.api.pocket.network"],
    "celo": ["https://celo.api.pocket.network"],
    "oasys": ["https://oasys.api.pocket.network"],
    "opbnb": ["https://opbnb.api.pocket.network"],
    "unichain": ["https://unichain.api.pocket.network"],
    "xrplevm": ["https://xrplevm.api.pocket.network"],
    "zksync-era": ["https://zksync-era.api.pocket.network"],
    "boba": ["https://boba.api.pocket.network"],
    "blast": ["https://blast.api.pocket.network"],
    "linea": ["https://linea.api.pocket.network"],
    "mantle": ["https://mantle.api.pocket.network"],
    "manta": ["https://manta.api.pocket.network"],
    "kaia": ["https://kaia.api.pocket.network"],
    "kava": ["https://kava.api.pocket.network"],
    "harmony": ["https://harmony.api.pocket.network"],
    "fuse": ["https://fuse.api.pocket.network"],
    "oasis": ["https://oasis.api.pocket.network"],
    "polygon": ["https://polygon.drpc.org", "https://poly.api.pocket.network", "https://polygon.gateway.tenderly.co", "https://polygon-rpc.com", "https://rpc.ankr.com/polygon/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
    "optimism": ["https://optimism.drpc.org", "https://op.api.pocket.network", "https://optimism.gateway.tenderly.co", "https://mainnet.optimism.io", "https://rpc.ankr.com/optimism/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
    "gnosis": ["https://gnosis.drpc.org", "https://gnosis.api.pocket.network", "https://rpc.gnosischain.com", "https://rpc.ankr.com/gnosis/f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"],
}

PROGRESS_FILE = os.path.join(ROOT, "scan_progress_{chain}.json")
RESULTS_FILE = os.path.join(ROOT, "scan_results.json")


def rpc_call_sync(chain, method, params, retries=3):
    urls = SCAN_RPCS.get(chain, [])
    for attempt in range(retries):
        for url in urls:
            try:
                r = requests.post(url, json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1}, timeout=15)
                res = r.json()
                if "result" in res:
                    return res["result"]
                if "error" in res:
                    continue
            except Exception:
                continue
    return None


def extract_selectors(code_hex):
    sels = set()
    i = 0
    # Solidity-style: PUSH4 sel EQ JUMPI (0x63 XXXXXXXX 0x14)
    while i < len(code_hex) - 12:
        if code_hex[i:i+2] == "63" and code_hex[i+10:i+12] == "14":
            sels.add("0x" + code_hex[i+2:i+10])
        i += 2
    # Vyper-style: PUSH4 sel (0x63 XXXXXXXX) followed by EQ within 6 bytes
    i = 0
    while i < len(code_hex) - 10:
        if code_hex[i:i+2] == "63":
            sel = "0x" + code_hex[i+2:i+10]
            window = code_hex[i+10:i+22]
            if "14" in window:
                sels.add(sel)
        i += 2
    return sels




def cmd_replay(args):
    tx = rpc_call_sync(args.chain, "eth_getTransactionByHash", [args.tx])
    print("tx from:", tx["from"])
    print("tx to:  ", tx["to"])
    print("block:  ", int(tx["blockNumber"], 16))
    fork_block = int(tx["blockNumber"], 16) - 1
    calldata = tx["input"]
    data_dir = os.path.join(ROOT, "test", "data")
    os.makedirs(data_dir, exist_ok=True)
    with open(os.path.join(data_dir, "replay.hex"), "w") as f:
        f.write(calldata)

    attacker = tx["from"]
    helper = tx["to"]
    if args.token.upper() == "USDC":
        if args.chain in CHAINS and CHAINS[args.chain]["usdc"] != "0x0000000000000000000000000000000000000000":
            token = CHAINS[args.chain]["usdc"]
        else:
            print(f"Warning: no USDC address for {args.chain}, provide --token or use a different token")
            token = args.token
    else:
        token = args.token

    test_src = """// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;
import "forge-std/Test.sol";
contract ReplayTest is Test {{
    address constant ATTACKER = {attacker};
    address constant HELPER   = {helper};
    address constant TOKEN    = {token};
    function test_replay() public {{
        uint256 fork = vm.createFork("{chain}", {fork_block});
        vm.selectFork(fork);
        uint256 before = IERC20(TOKEN).balanceOf(ATTACKER);
        console2.log("before:", before);
        bytes memory data = vm.parseBytes(vm.readFile("test/data/replay.hex"));
        vm.startPrank(ATTACKER);
        (bool ok, bytes memory ret) = HELPER.call(data);
        vm.stopPrank();
        console2.log("call ok:", ok);
        if (!ok) console2.logBytes(ret);
        uint256 afterBal = IERC20(TOKEN).balanceOf(ATTACKER);
        console2.log("after:", afterBal);
        console2.log("profit raw:", afterBal - before);
        assertTrue(ok);
    }}
}}
interface IERC20 {{ function balanceOf(address) external view returns (uint256); }}
""".format(attacker=attacker, helper=helper, token=token, chain=args.chain, fork_block=fork_block)

    test_path = os.path.join(ROOT, "test", "Replay.t.sol")
    with open(test_path, "w") as f:
        f.write(test_src)
    print("Generated test/Replay.t.sol")
    # Auto-run forge if available
    import subprocess, shutil
    forge = shutil.which("forge")
    if not forge:
        for foundry_bin in [os.path.join(os.path.expanduser("~"), ".foundry", "bin"), "/usr/local/bin", "/opt/foundry/bin"]:
            for name in ["forge", "forge.exe"]:
                candidate = os.path.join(foundry_bin, name)
                if os.path.exists(candidate):
                    forge = candidate
                    break
            if forge:
                break
    if forge:
        print("Running forge test...")
        env = os.environ.copy()
        env["PATH"] = env.get("PATH", "") + os.pathsep + os.path.dirname(forge)
        subprocess.run([forge, "test", "--match-test", "test_replay", "-vv"], cwd=ROOT, env=env)
    else:
        print("Run: forge test --match-test test_replay -vv")


async def scan_chain(chain, targets, num_blocks, resume_from=None):
    urls = SCAN_RPCS.get(chain, [])
    if not urls:
        print(f"[{chain}] No RPC endpoints configured")
        return []
    url = urls[0]
    connector = aiohttp.TCPConnector(limit=200, ttl_dns_cache=300)
    async with aiohttp.ClientSession(connector=connector) as session:
        sem = asyncio.Semaphore(100)
        # Get latest block
        latest = None
        for u in urls:
            try:
                async with session.post(u, json={"jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1}, timeout=aiohttp.ClientTimeout(total=10)) as resp:
                    data = await resp.json()
                    if data.get("result"):
                        latest = int(data["result"], 16)
                        url = u
                        break
            except Exception:
                continue
        if not latest:
            print(f"[{chain}] Failed to get latest block on all endpoints")
            return []
        if resume_from and 0 < resume_from < latest:
            start_block = resume_from + 1
            scan_range = min(num_blocks, latest - start_block)
            if scan_range <= 0:
                print(f"[{chain}] Already scanned to latest block")
                return []
            print(f"[{chain}] Latest block: {latest}, resuming from {start_block}, scanning {scan_range} blocks...")
        else:
            start_block = max(0, latest - num_blocks)
            scan_range = num_blocks
            print(f"[{chain}] Latest block: {latest}, scanning {num_blocks} blocks...")

        matches = []
        match_lock = asyncio.Lock()
        rpc_failures = [0]


        async def fetch_single(method, params):
            for u in urls:
                try:
                    async with session.post(u, json={"jsonrpc": "2.0", "method": method, "params": params, "id": 1}, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                        data = await resp.json()
                        if "result" in data:
                            return data["result"]
                except Exception:
                    continue
            rpc_failures[0] += 1
            return None

        async def process_block(blk_num):
            nonlocal matches
            async with sem:
                receipts = await fetch_single("eth_getBlockReceipts", [hex(blk_num)])
                if receipts and isinstance(receipts, list):
                    for r in receipts:
                        if not r or not r.get("contractAddress"):
                            continue
                        addr = r["contractAddress"]
                        code = await fetch_single("eth_getCode", [addr, "latest"])
                        if not code or code == "0x" or len(code) < 10:
                            continue
                        sels = extract_selectors(code[2:])
                        pattern_matches = [pname for pname, psels in BYTECODE_PATTERNS.items() if (psels[0] if isinstance(psels, tuple) else psels).issubset(sels)]
                        generic_only = bool(sels) and all(s in GENERIC_ERC20 for s in sels)
                        if pattern_matches and not generic_only:
                            m = {"chain": chain, "block": blk_num, "address": addr, "tx": r.get("transactionHash", ""), "selectors": sorted(sels), "patterns": pattern_matches}
                            async with match_lock:
                                matches.append(m)
                            print(f"  [{chain}] PATTERN MATCH: block {blk_num} addr {addr} -> {pattern_matches}")
                    return
                block = await fetch_single("eth_getBlockByNumber", [hex(blk_num), True])
                if not block or not block.get("transactions"):
                    return
                for tx in block["transactions"]:
                    if tx.get("to") is not None:
                        continue
                    receipt = await fetch_single("eth_getTransactionReceipt", [tx["hash"]])
                    if not receipt or not receipt.get("contractAddress"):
                        continue
                    addr = receipt["contractAddress"]
                    code = await fetch_single("eth_getCode", [addr, "latest"])
                    if not code or code == "0x" or len(code) < 10:
                        continue
                    sels = extract_selectors(code[2:])
                    pattern_matches = []
                    for pname, (req, opt) in BYTECODE_PATTERNS.items():
                        if req.issubset(sels):
                            opt_present = len(opt & sels)
                            score = (len(req) + opt_present) / (len(req) + len(opt)) if (len(req) + len(opt)) > 0 else 1.0
                            pattern_matches.append((pname, round(score, 2)))
                    generic_only = bool(sels) and all(s in GENERIC_ERC20 for s in sels)
                    if pattern_matches and not generic_only:
                        m = {"chain": chain, "block": blk_num, "address": addr, "tx": tx["hash"], "selectors": sorted(sels), "patterns": [p[0] for p in pattern_matches], "scores": {p[0]: p[1] for p in pattern_matches}}
                        async with match_lock:
                            matches.append(m)
                        for pname, score in pattern_matches:
                            print(f"  [{chain}] PATTERN MATCH: block {blk_num} addr {addr} -> {pname} (score={score})")

        # Batch processing: process 200 blocks at a time
        batch_size = 200
        for start in range(start_block, start_block + scan_range, batch_size):
            end = min(start + batch_size, start_block + scan_range)
            tasks = [process_block(blk) for blk in range(start, end)]
            await asyncio.gather(*tasks)
            if start % 1000 == 0:
                print(f"  [{chain}] scanned to block {start}, {len(matches)} matches, {rpc_failures[0]} rpc failures")
                with open(PROGRESS_FILE.format(chain=chain), "w") as f:
                    json.dump({"chain": chain, "last_block": start, "matches": matches, "rpc_failures": rpc_failures[0]}, f)

        print(f"[{chain}] Scan complete: {len(matches)} matches, {rpc_failures[0]} rpc failures")
        return matches

def cmd_scan(args):
    targets = list(set().union(*[p[0] | p[1] for p in BYTECODE_PATTERNS.values()]))
    print(f"Scanning for {len(BYTECODE_PATTERNS)} bytecode patterns ({len(targets)} selectors)...")
    chains = list(SCAN_RPCS.keys()) if args.chains == "all" else [c.strip() for c in args.chains.split(",")]

    async def run():
        tasks = []
        resume_map = {}
        for c in chains:
            prog_file = os.path.join(ROOT, f"scan_progress_{c}.json")
            rf = None
            if os.path.exists(prog_file):
                try:
                    with open(prog_file) as f:
                        progress = json.load(f)
                    rf = progress.get("last_block")
                    if rf:
                        print(f"[{c}] Resuming from block {rf}")
                except Exception:
                    pass
            resume_map[c] = rf
        for c in chains:
            tasks.append(scan_chain(c, targets, args.blocks, resume_map[c]))
        results = await asyncio.gather(*tasks)
        all_matches = []
        for r in results:
            all_matches.extend(r)
        with open(RESULTS_FILE, "w") as f:
            json.dump(all_matches, f, indent=2)
        print(f"\nTotal matches across all chains: {len(all_matches)}")
        if all_matches:
            print("KRW profit estimation requires forge reproduction of each matched contract.")
        pattern_count = sum(1 for m in all_matches if m.get("patterns"))
        if pattern_count:
            print(f"Pattern matches: {pattern_count}")
        pattern_count = sum(1 for m in all_matches if m.get("patterns"))
        if pattern_count:
            print(f"Pattern matches: {pattern_count}")
            for m in all_matches:
                if m.get("patterns"):
                    print(f"  [{m['chain']}] {m['address']} -> {', '.join(m['patterns'])} (profit requires forge reproduction)")
        # Deduplicate by address
        seen = set()
        unique_matches = []
        for m in all_matches:
            key = (m["chain"], m["address"])
            if key not in seen:
                seen.add(key)
                unique_matches.append(m)
        print(f"Unique contracts: {len(unique_matches)}")
        for m in unique_matches:
            print(f"  [{m['chain']}] {m['address']} (block {m['block']}) patterns: {m.get('patterns', [])}")

    asyncio.run(run())


def main():
    p = argparse.ArgumentParser(description="Defi pipeline: contract analysis, exploit replay, multi-chain block scanner")
    sub = p.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("replay", help="Replay a hacked tx on a Foundry fork")
    r.add_argument("--chain", required=True, choices=list(CHAINS))
    r.add_argument("--tx", required=True)
    r.add_argument("--token", default="USDC")
    r.set_defaults(func=cmd_replay)


    s = sub.add_parser("scan", help="Scan blocks across chains for hack-related contracts")
    s.add_argument("--chains", default="all", help="Comma-separated chains or 'all'")
    s.add_argument("--blocks", type=int, default=5000, help="Number of blocks to scan")
    s.set_defaults(func=cmd_scan)

    args = p.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
