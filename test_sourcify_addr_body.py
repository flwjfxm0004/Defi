import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base
ADDRESS = "0xdac30a5e2612206e2756836ed6764ec5817e6fff"

async def get_runtime_bytecode(session, rpc_url, address):
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [address, "latest"]
    }
    async with session.post(rpc_url, json=payload, timeout=aiohttp.ClientTimeout(total=30)) as r:
        d = await r.json()
        return d.get("result", "")

async def main():
    async with aiohttp.ClientSession() as session:
        # 1. 바이트코드 가져오기
        print("[1단계] PRXVT 런타임 바이트코드 조회")
        bytecode = await get_runtime_bytecode(session, "https://base.publicnode.com", ADDRESS)
        print(f"  길이: {len(bytecode)//2} 바이트")
        
        # 2. Sourcify 유사 검색 - 주소 URL + 바이트코드 바디
        print(f"\n[2단계] Sourcify 유사 검색 - 주소 URL + 바이트코드 바디")
        
        url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}/{ADDRESS}"
        headers = {"Content-Type": "application/json"}
        
        body = {
            "bytecode": bytecode
        }
        
        async with session.post(url, headers=headers, json=body, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            print(f"  Status: {r.status}")
            print(f"  Response: {text[:1500]}")

if __name__ == "__main__":
    asyncio.run(main())
