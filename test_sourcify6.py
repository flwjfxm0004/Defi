import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base
# Base USDC - 검증된 컨트랙트
ADDRESS = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"

async def main():
    url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}/{ADDRESS}"
    headers = {"Content-Type": "application/json"}
    
    async with aiohttp.ClientSession() as session:
        print(f"POST {url}")
        async with session.post(url, headers=headers, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            print(f"Status: {r.status}")
            print(f"Response: {text[:1000]}")

if __name__ == "__main__":
    asyncio.run(main())
