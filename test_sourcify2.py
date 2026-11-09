import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base
ADDRESS = "0xdac30a5e2612206e2756836ed6764ec5817e6fff"

async def main():
    async with aiohttp.ClientSession() as session:
        # Try to get verification info for this address
        url = f"https://sourcify.dev/server/v2/verify/{CHAIN_ID}/{ADDRESS}"
        print(f"GET {url}")
        async with session.get(url, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            print(f"Status: {r.status}")
            print(f"Response: {text[:3000]}")

if __name__ == "__main__":
    asyncio.run(main())
