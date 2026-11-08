p='C:/Defi/해킹.md'
s=open(p,encoding='utf-8').read()
# search for 'FeeAMM' in the file
idx=s.find('FeeAMM')
print('FeeAMM found at:', idx)
if idx!=-1:
    # show 100 chars around it
    snippet=s[max(0,idx-120):idx+80]
    open('C:/Defi/snippet217.txt','w',encoding='utf-8').write(snippet)
    print('written')
