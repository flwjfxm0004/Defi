import json
import asyncio
import aiohttp

with open('C:\\Defi\\sourcify_similarity_results.json', encoding='utf-8') as f:
    data = json.load(f)

# PENDING 상태인 것들의 verificationId 추출 (error 필드에서)
pending = []
for r in data['results']:
    result = r['sourcify_result']
    if result.get('status') == 202:
        d = result.get('error', {})
        if isinstance(d, str):
            try:
                d = json.loads(d)
            except:
                d = {}
        vid = d.get('verificationId')
        if vid:
            pending.append((r['chain'], r['address'], vid))

print(f'PENDING contracts: {len(pending)}')
for chain, addr, vid in pending:
    print(f'  [{chain}] {addr} -> vid={vid}')

# 폴링하여 결과 확인
async def poll_one(session, chain, addr, vid):
    url = f"https://sourcify.dev/server/v2/verify/{vid}"
    for i in range(30):
        await asyncio.sleep(2)
        try:
            async with session.get(url, timeout=aiohttp.ClientTimeout(total=10)) as r:
                text = await r.text()
                try:
                    d = json.loads(text)
                    status = d.get('status', 'unknown')
                    if i % 5 == 0 or status in ('finished', 'completed', 'done', 'error', 'failed'):
                        print(f'  [{chain}] {addr} [{i*2}s] status: {status}')
                    if status in ('finished', 'completed', 'done', 'error', 'failed'):
                        return chain, addr, d
                except:
                    pass
        except:
            pass
    return chain, addr, {"status": "timeout"}

async def main():
    results = []
    async with aiohttp.ClientSession() as session:
        for chain, addr, vid in pending:
            print(f'\nPolling [{chain}] {addr} (vid={vid})...')
            chain, addr, d = await poll_one(session, chain, addr, vid)
            results.append((chain, addr, d))
    
    print('\n=== FINAL RESULTS ===')
    for chain, addr, d in results:
        status = d.get('status', 'unknown')
        print(f'[{chain}] {addr} -> {status}')
        if status in ('finished', 'completed', 'done'):
            print(f'  Data: {json.dumps(d, ensure_ascii=False)[:1000]}')

asyncio.run(main())
