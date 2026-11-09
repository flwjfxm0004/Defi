import json

with open('C:\\Defi\\sourcify_similarity_results.json', encoding='utf-8') as f:
    data = json.load(f)

print('Total seeds:', data['total_seeds'])
print('Results count:', len(data['results']))
print()

for r in data['results']:
    addr = r['address']
    chain = r['chain']
    result = r['sourcify_result']
    status = result.get('status', 'unknown')
    
    if status == 202:
        vid = result.get('data', {}).get('verificationId', 'unknown')
        print(f'[{chain}] {addr} -> PENDING vid={vid}')
    elif status == 409:
        print(f'[{chain}] {addr} -> ALREADY_VERIFIED')
    elif status == 404:
        print(f'[{chain}] {addr} -> NOT_FOUND')
    else:
        err = str(result.get('error', ''))[:80]
        print(f'[{chain}] {addr} -> {status}: {err}')
