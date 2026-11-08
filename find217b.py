p='C:/Defi/해킹.md'
s=open(p,encoding='utf-8').read()
idx=s.find('**217.')
out=s[idx:idx+300]
open('C:/Defi/inc217b.txt','w',encoding='utf-8').write(out)
