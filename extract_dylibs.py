import struct
p='C:/Defi/research/ipas/instagram_extracted/Payload/Instagram.app/Instagram'
data=open(p,'rb').read()
ncmds=struct.unpack('<I',data[16:20])[0]
o=32
dylibs=[]
for i in range(ncmds):
    cmd,cmdsize=struct.unpack('<II',data[o:o+8])
    if cmd==0xc: # LC_LOAD_DYLIB
        # name offset at o+8 (uint32), then path
        off_name=struct.unpack('<I',data[o+8:o+12])[0]
        name=data[o+off_name:o+cmdsize].split(b'\x00')[0].decode('utf-8','replace')
        dylibs.append(name)
    o+=cmdsize
out=open('C:/Defi/research/dylibs.txt','w',encoding='utf-8')
for d in dylibs:
    out.write(d+'\n')
out.close()
print('count',len(dylibs))
for d in dylibs[:80]:
    print(d)
