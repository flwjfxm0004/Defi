import re
p='C:/Defi/research/strings_instagram.txt'
data=open(p,encoding='utf-8',errors='replace').read().splitlines()
urls=set()
for line in data:
    for m in re.finditer(r'https?://[^\s"<>]+', line):
        u=m.group(0)
        if len(u)>8 and len(u)<200:
            urls.add(u)
out=open('C:/Defi/research/urls_instagram.txt','w',encoding='utf-8')
for u in sorted(urls):
    out.write(u+'\n')
out.close()
print('count',len(urls))
