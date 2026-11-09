import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base

# Base 체인의 여러 주소로 테스트 (바이트코드 있는 곳 찾기)
test_addresses = [
    "0x50c5725949A6F0c72E6C4a641F24B45e9C52f896",
    "0x45119725Bf2b29131d983C07045B2E410D6A1D22",  # possible contract
    "0x2175708d2899e08f056966e4104c7F469b33B1b7",  # possible contract
]

async def check_bytecode(session, address):
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [address, "latest"]
    }
    async with session.post("https://base.publicnode.com", json=payload, timeout=aiohttp.ClientTimeout(total=15)) as r:
        d = await r.json()
        code = d.get("result", "")
        return address, code

async def main():
    async with aiohttp.ClientSession() as session:
        for addr in test_addresses:
            address, code = await check_bytecode(session, addr)
            print(f"Address: {address}")
            print(f"Bytecode length: {len(code)} chars ({len(code)//2} bytes), starts with: {code[:60]}...")
            
            if code and code != "0x" and len(code) > 100:
                # Try similarity search
                url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}/{address}"
                headers = {"Content-Type": "application/json"}
                print(f"  → Sourcify similarity 검색 시작...")
                async with session.post(url, headers=headers, timeout=aiohttp.ClientTimeout(total=30)) as r:
                    text = await r.text()
                    print(f"  Status: {r.status}")
                    print(f"  Response: {text[:500]}")
                    
                    if r.status == 200:
                        try:
                            d = json.loads(text)
                            vid = d.get("verificationId") or d.get("id")
                            if vid:
                                print(f"  Verification ID: {vid}")
                                print(f"  폴링 중...")
                                for i in range(30):
                                    await asyncio.sleep(1)
                                    async with session.get(f"https://sourcify.dev/server/v2/verify/{vid}", timeout=aiohttp.ClientTimeout(total=10)) as poll_r:
                                        poll_text = await poll_r.text()
                                        try:
                                            poll_d = json.loads(poll_text)
                                            status = poll_d.get("status", "unknown")
                                            if i % 5 == 0 or status in ("finished", "completed", "done", "error", "failed"):
                                                print(f"    [{i+1}s] status: {status}")
                                            if status in ("finished", "completed", "done", "error", "failed"):
                                                print(f"  최종: {json.dumps(poll_d, indent=2)[:2000]}")
                                                break
                                        except:
                                            pass
                        except:
                            pass
            print()

if __name__ == "__main__":
    asyncio.run(main())
