import re
p='C:/Defi/해킹.md'
s=open(p,encoding='utf-8').read()
headers=list(re.finditer(r'\*\*\d+\.\s', s))
for idx,m in enumerate(headers):
    after=s[m.end():m.end()+12]
    if not (re.match(r'\d{2}/\d{2}', after) or re.match(r'\d{4}-\d{2}-\d{2}', after)): continue
    num=int(re.match(r'\*\*(\d+)\.', m.group(0)).group(1))
    if num==217:
        start=m.start()
        end=headers[idx+1].start() if idx+1<len(headers) else len(s)
        block=s[start:end]
        am=re.search(r'- \*\*피해 컨트랙트 주소\*\*:.*', block)
        print('addr match:', am.group(0) if am else 'NONE')
        print(block[:300])
