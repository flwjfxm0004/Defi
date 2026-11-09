import aiohttp
import asyncio
import json

# PRXVT (stPRXVT) on Base - 1번 사건
CHAIN_ID = 8453
ADDRESS = "0xdac30a5e2612206e2756836ed6764ec5817e6fff"

async def get_runtime_bytecode(session, rpc_url, address):
    """런타임 바이트코드 조회"""
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getCode",
        "params": [address, "latest"]
    }
    async with session.post(rpc_url, json=payload, timeout=aiohttp.ClientTimeout(total=30)) as r:
        d = await r.json()
        return d.get("result", "")

async def sourcify_similarity(session, chain_id, address_or_bytecode, is_bytecode=False):
    """Sourcify 유사 검색 - 주소 또는 바이트코드 입력"""
    
    if is_bytecode:
        # 바이트코드 직접 입력 (MCP 도구 방식)
        url = f"https://sourcify.dev/server/v2/verify/similarity/{chain_id}/{address_or_bytecode}"
        headers = {"Content-Type": "application/json"}
        body = {"bytecode": address_or_bytecode}
        async with session.post(url, headers=headers, json=body, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            return r.status, text
    else:
        # 주소 기반 입력 (HTTP API 방식)
        url = f"https://sourcify.dev/server/v2/verify/similarity/{chain_id}/{address_or_bytecode}"
        headers = {"Content-Type": "application/json"}
        async with session.post(url, headers=headers, timeout=aiohttp.ClientTimeout(total=30)) as r:
            text = await r.text()
            return r.status, text

async def poll_sourcify(session, verification_id):
    """Sourcify 검증 결과 폴링"""
    url = f"https://sourcify.dev/server/v2/verify/{verification_id}"
    async with session.get(url, timeout=aiohttp.ClientTimeout(total=15)) as r:
        text = await r.text()
        return r.status, text

async def main():
    print("=== Sourcify 유사 컨트랙트 검색 (MCP 도구 방식 구현) ===\n")
    
    async with aiohttp.ClientSession() as session:
        # 1단계: 런타임 바이트코드 조회
        print("[1단계] PRXVT 런타임 바이트코드 조회 (Base RPC)")
        bytecode = await get_runtime_bytecode(session, "https://base.publicnode.com", ADDRESS)
        print(f"  주소: {ADDRESS}")
        print(f"  바이트코드 길이: {len(bytecode)} 문자 ({len(bytecode)//2} 바이트)")
        print(f"  시작: {bytecode[:80]}...")
        
        if not bytecode or bytecode == "0x":
            print("  오류: 바이트코드를 가져올 수 없습니다.")
            return
        
        # 2단계: Sourcify 유사 검색 (주소 기반)
        print(f"\n[2단계] Sourcify 유사 검색 - 주소 기반 (chainId={CHAIN_ID})")
        status, text = await sourcify_similarity(session, CHAIN_ID, ADDRESS, is_bytecode=False)
        print(f"  Status: {status}")
        print(f"  Response: {text[:500]}")
        
        if status == 200:
            try:
                d = json.loads(text)
                vid = d.get("verificationId") or d.get("id")
                if vid:
                    print(f"  Verification ID: {vid}")
                    print(f"\n[3단계] 폴링 중 (최대 60초)...")
                    for i in range(60):
                        await asyncio.sleep(1)
                        p_status, p_text = await poll_sourcify(session, vid)
                        try:
                            p_d = json.loads(p_text)
                            p_status_val = p_d.get("status", "unknown")
                            if i % 5 == 0 or p_status_val in ("finished", "completed", "done", "error", "failed"):
                                print(f"  [{i+1}s] status: {p_status_val}")
                            if p_status_val in ("finished", "completed", "done", "error", "failed"):
                                print(f"  최종 결과: {json.dumps(p_d, indent=2)[:3000]}")
                                break
                        except:
                            print(f"  [{i+1}s] raw: {p_text[:200]}")
            except:
                pass
        
        # 3단계: Sourcify 유사 검색 (바이트코드 직접 입력 - MCP 도구 방식 시뮬레이션)
        print(f"\n[3단계] Sourcify 유사 검색 - 바이트코드 직접 입력 (MCP 도구 방식)")
        status, text = await sourcify_similarity(session, CHAIN_ID, bytecode, is_bytecode=True)
        print(f"  Status: {status}")
        print(f"  Response: {text[:500]}")

if __name__ == "__main__":
    asyncio.run(main())
