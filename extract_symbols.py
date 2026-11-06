import struct
p='C:/Defi/research/ipas/instagram_extracted/Payload/Instagram.app/Instagram'
data=open(p,'rb').read()
# from LC_SYMTAB
symoff=385004040; nsyms=39147; stroff=385901968; strsize=2078072
strtab=data[stroff:stroff+strsize]
syms=[]
for i in range(nsyms):
    off=symoff+i*16
    n_strx,n_type,n_sect,n_desc,n_value=struct.unpack('<IBBHI',data[off:off+12])[:4]+(struct.unpack('<Q',data[off+8:off+16])[0],)
    # actually unpack carefully
    n_strx=struct.unpack('<I',data[off:off+4])[0]
    n_type=data[off+4]
    n_sect=data[off+5]
    n_desc=struct.unpack('<H',data[off+6:off+8])[0]
    n_value=struct.unpack('<Q',data[off+8:off+16])[0]
    if n_strx==0 or n_strx>=strsize: 
        continue
    end=strtab.find(b'\x00',n_strx)
    name=strtab[n_strx:end].decode('utf-8','replace')
    syms.append((name,n_type,n_sect,n_value))
# filter for relevant names
import re
keys=['kaleidoscope','ks_classify','pdfcheck','FontFile','ImageIO','CGImage','ImageSource','Media','WebP','PNG','MP4','HEVC','AV1','Decoder','Parser']
hits=[s for s in syms if any(k.lower() in s[0].lower() for k in keys)]
out=open('C:/Defi/research/symbols.txt','w',encoding='utf-8')
for name,nt,ns,nv in sorted(hits, key=lambda x:x[0]):
    out.write('0x%x  %s  (type=%d sect=%d)\n'%(nv,name,nt,ns))
out.close()
print('total syms',len(syms),'hits',len(hits))
