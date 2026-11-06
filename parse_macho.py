import struct
p='C:/Defi/research/ipas/instagram_extracted/Payload/Instagram.app/Instagram'
data=open(p,'rb').read()
magic=struct.unpack('<I',data[:4])[0]
off=0
# header 32 bytes for 64-bit
ncmds=struct.unpack('<I',data[16:20])[0]
o=32
segs=[]
for i in range(ncmds):
    cmd,cmdsize=struct.unpack('<II',data[o:o+8])
    if cmd==0x19: # LC_SEGMENT_64
        segname=data[o+8:o+24].split(b'\x00')[0].decode()
        vmaddr,vmsize,fileoff,filesize,nsects,flags=struct.unpack('<QQQQII',data[o+24:o+64])
        secs=[]
        so=o+72
        for j in range(nsects):
            secname=data[so:so+16].split(b'\x00')[0].decode()
            segn=data[so+16:so+32].split(b'\x00')[0].decode()
            addr,size=struct.unpack('<QQ',data[so+32:so+48])
            offset,align,reloff,nreloc,flags2,r1,r2,r3=struct.unpack('<IIIIIIII',data[so+48:so+80])
            secs.append((secname,segn,addr,size,offset))
            so+=80
        segs.append((segname,vmaddr,vmsize,fileoff,filesize,nsects,secs))
    o+=cmdsize
# print sections
for segname,vmaddr,vmsize,fileoff,filesize,nsects,secs in segs:
    if secs:
        print('SEG',segname,'nsects',nsects)
        for secname,segn,addr,size,offset in secs:
            print(f'  {secname:28} off={offset} size={size}')
