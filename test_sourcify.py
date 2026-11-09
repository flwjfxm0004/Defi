import aiohttp
import asyncio
import json

CHAIN_ID = 8453  # Base
ADDRESS = "0xdac30a5e2612206e2756836ed6764ec5817e6fff"

async def main():
    url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}/{ADDRESS}"
    headers = {"Content-Type": "application/json"}
    
    async with aiohttp.ClientSession() as session:
        print(f"POST {url}")
        async with session.post(url, headers=headers, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            print(f"Status: {r.status}")
            print(f"Response: {text[:1000]}")
            
            try:
                d = json.loads(text)
                vid = d.get("verificationId") or d.get("id")
                if vid:
                    print(f"\nVerification ID: {vid}")
                    print(f"\n폴링 중... (최대 60초)")
                    
                    for i in range(60):
                        await asyncio.sleep(1)
                        async with session.get(f"https://sourcify.dev/server/v2/verify/{vid}", timeout=aiohttp.ClientTimeout(total=10)) as poll_r:
                            poll_text = await poll_r.text()
                            try:
                                poll_d = json.loads(poll_text)
                                status = poll_d.get("status", "unknown")
                                print(f"  [{i+1}s] status: {status}")
                                if status in ("finished", "completed", "done", "error", "failed"):
                                    print(f"  최종 결과: {json.dumps(poll_d, indent=2)[:3000]}")
                                    break
                            except:
                                print(f"  [{i+1}s] raw: {poll_text[:200]}")
            except:
                pass

if __name__ == "__main__":
    asyncio.run(main())
