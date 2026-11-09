import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base
ADDRESS = "0xdac30a5e2612206e2756836ed6764ec5817e6fff"

async def main():
    async with aiohttp.ClientSession() as session:
        # First, get the contract's runtime bytecode from latest block
        print("1단계: 런타임 바이트코드 조회")
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getCode",
            "params": [ADDRESS, "latest"]
        }
        
        # Use a public RPC to get bytecode
        async with session.post("https://base.publicnode.com", json=payload, timeout=aiohttp.ClientTimeout(total=30)) as r:
            d = await r.json()
            bytecode = d.get("result", "")
            print(f"런타임 바이트코드 길이: {len(bytecode)} 문자 ({len(bytecode)//2} 바이트)")
            print(f"시작: {bytecode[:100]}...")
        
        if bytecode and bytecode != "0x":
            print(f"\n2단계: Sourcify 유사 검색 (바이트코드 직접 입력)")
            # Try posting bytecode directly
            url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}/{ADDRESS}"
            headers = {"Content-Type": "application/json"}
            body = {"bytecode": bytecode}
            
            async with session.post(url, headers=headers, json=body, timeout=aiohttp.ClientTimeout(total=30)) as r:
                text = await r.text()
                print(f"Status: {r.status}")
                print(f"Response: {text[:2000]}")

if __name__ == "__main__":
    asyncio.run(main())
