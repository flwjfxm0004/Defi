import re
p='C:/Defi/해킹.md'
s=open(p,encoding='utf-8').read()
headers=list(re.finditer(r'\*\*\d+\.\s', s))
for idx in range(len(headers)-1,-1,-1):
    m=headers[idx]
    after=s[m.end():m.end()+12]
    if not (re.match(r'\d{2}/\d{2}', after) or re.match(r'\d{4}-\d{2}-\d{2}', after)): continue
    num=int(re.match(r'\*\*(\d+)\.', m.group(0)).group(1))
    if num!=217: continue
    start=m.start()
    end=headers[idx+1].start() if idx+1<len(headers) else len(s)
    block=s[start:end]
    am=re.search(r'- \*\*피해 컨트랙트/주소\*\*:.*', block)
    if am:
        abs_start=start+am.start()
        abs_end=start+am.end()
        new_addr='- **피해 컨트랙트 주소**: Tempo FeeAMM (FeeManager precompile `0xfeec000000000000000000000000000000000000`, Tempo chain)'
        s=s[:abs_start]+new_addr+s[abs_end:]
        print('updated address for 217')
open(p,'w',encoding='utf-8').write(s)
