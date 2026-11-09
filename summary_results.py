import json

with open('C:\\Defi\\sourcify_similarity_results.json', encoding='utf-8') as f:
    data = json.load(f)

print('=== Sourcify Similarity Search Results ===')
print(f'Total vulnerable contracts: {data["total_seeds"]}')
print(f'Contracts with bytecode: {len(data["results"])}')
print()

from collections import Counter
statuses = Counter()
for r in data['results']:
    s = r['sourcify_result'].get('status', 'unknown')
    statuses[s] += 1

print('Status distribution:')
for s, c in statuses.most_common():
    print(f'  {s}: {c}')

print()
print('--- ALREADY_VERIFIED (409) ---')
for r in data['results']:
    if r['sourcify_result'].get('status') == 409:
        print(f'  [{r["chain"]}] {r["address"]}')

print()
print('--- PENDING (202) - needs polling ---')
for r in data['results']:
    result = r['sourcify_result']
    if result.get('status') == 202:
        d = result.get('error', {})
        if isinstance(d, str):
            try:
                d = json.loads(d)
            except:
                pass
        vid = d.get('verificationId', 'unknown')
        print(f'  [{r["chain"]}] {r["address"]} -> vid={vid}')

print()
print('--- OTHER (400, 404, 429) ---')
for r in data['results']:
    s = r['sourcify_result'].get('status', 'unknown')
    if s not in (202, 409):
        print(f'  [{r["chain"]}] {r["address"]} -> {s}: {str(r["sourcify_result"].get("error", ""))[:100]}')
