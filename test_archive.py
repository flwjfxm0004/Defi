import asyncio
import aiohttp
import json

ANKR_KEY = "f098d8113a6ebea616cd06908b2421bdf72916efd6342ba6981db600d1e3c26f"
TATUM_KEY = "bm_live_ca8f52c5b744adaab43de4a4fd7e7fbe027afbbb039265c28b481b3abcf1fc0c"
NODEREAL_KEY = "e7ec74ea77134e99b3aca76de8e602c5"

WETH = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
WBNB = "0xbb4CdB9CBd36B01bD1cBaEBF2J0831873405af91"

# 테스트할 엔드포인트 (과거 블록 1000에서 WETH 코드 조회)
tests = [
    ("ANKR eth", f"https://rpc.ankr.com/eth/{ANKR_KEY}", "ethereum", 1000, WETH),
    ("ANKR bsc", f"https://rpc.ankr.com/bsc/{ANKR_KEY}", "bsc", 1000, WBNB),
    ("ANKR polygon", f"https://rpc.ankr.com/polygon/{ANKR_KEY}", "polygon", 1000, WETH),
    ("TATUM ethereum", f"https://ethereum-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}", "ethereum", 1000, WETH),
    ("TATUM bsc", f"https://bsc-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}", "bsc", 1000, WBNB),
    ("TATUM base", f"https://base-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}", "base", 1000, WETH),
    ("TATUM arbitrum", f"https://arbitrum-one-mainnet.gateway.tatum.io/v4/api-key/{TATUM_KEY}", "arbitrum", 1000, WETH),
    ("NODEREAL ethereum", f"https://rpc.nodereal.io/v1/{NODEREAL_KEY}", "ethereum", 1000, WETH),
    ("NODEREAL bsc", f"https://bsc-mainnet.nodereal.io/v1/{NODEREAL_KEY}", "bsc", 1000, WBNB),
    ("NODEREAL avax", f"https://avax-mainnet.nodereal.io/v1/{NODEREAL_KEY}", "avalanche", 1000, WETH),
]

async def test_one(session, label, chain, block, addr, url):
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [addr, hex(block)]
    }
    try:
        async with session.post(url, json=payload, timeout=aiohttp.ClientTimeout(total=20)) as r:
            text = await r.text()
            try:
                d = json.loads(text)
                result = d.get("result", "")
                error = d.get("error")
                if error:
                    msg = error.get('message', 'unknown')
                    code = error.get('code', '?')
                    return f"  [{label}] chain={chain} block={block} -> ERROR {code}: {msg}"
                if result == "0x":
                    return f"  [{label}] chain={chain} block={block} -> EMPTY (contract not deployed yet)"
                if len(result) > 100:
                    return f"  [{label}] chain={chain} block={block} -> OK ({len(result)//2} bytes)"
                return f"  [{label}] chain={chain} block={block} -> SHORT: {result[:50]}..."
            except:
                return f"  [{label}] chain={chain} block={block} -> INVALID: {text[:100]}"
    except Exception as e:
        return f"  [{label}] chain={chain} block={block} -> EXC: {type(e).__name__}: {str(e)[:80]}"

async def main():
    print("=== 아카이브 RPC 실제 테스트 (과거 블록 1000에서 eth_getCode) ===\n")
    async with aiohttp.ClientSession() as session:
        tasks = [test_one(session, label, chain, block, addr, url) 
                 for label, url, chain, block, addr in tests]
        results = await asyncio.gather(*tasks)
        for r in results:
            print(r)

if __name__ == "__main__":
    asyncio.run(main())
