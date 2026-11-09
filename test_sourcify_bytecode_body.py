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
        print(f"  앞 100자: {bytecode[:100]}")
        
        # 2. Sourcify에 바이트코드 직접 전송 (MCP 도구 방식 시뮬레이션)
        # POST /v2/verify/similarity/{chainId} 에 바이트코드를 바디로 전송
        print(f"\n[2단계] Sourcify 유사 검색 - 바이트코드 바디 전송")
        
        url = f"https://sourcify.dev/server/v2/verify/similarity/{CHAIN_ID}"
        headers = {"Content-Type": "application/json"}
        body = {"bytecode": bytecode}
        
        async with session.post(url, headers=headers, json=body, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            print(f"  Status: {r.status}")
            print(f"  Response: {text[:1000]}")
            
            if r.status == 200:
                try:
                    d = json.loads(text)
                    vid = d.get("verificationId") or d.get("id")
                    if vid:
                        print(f"  Verification ID: {vid}")
                        print(f"\n[3단계] 폴링 중 (최대 60초)...")
                        for i in range(60):
                            await asyncio.sleep(1)
                            async with session.get(f"https://sourcify.dev/server/v2/verify/{vid}", timeout=aiohttp.ClientTimeout(total=10)) as poll_r:
                                poll_text = await poll_r.text()
                                try:
                                    p_d = json.loads(poll_text)
                                    p_status = p_d.get("status", "unknown")
                                    if i % 5 == 0 or p_status in ("finished", "completed", "done", "error", "failed"):
                                        print(f"  [{i+1}s] status: {p_status}")
                                    if p_status in ("finished", "completed", "done", "error", "failed"):
                                        print(f"  최종 결과: {json.dumps(p_d, indent=2)[:3000]}")
                                        break
                                except:
                                    print(f"  [{i+1}s] raw: {poll_text[:200]}")
                except:
                    pass

if __name__ == "__main__":
    asyncio.run(main())
