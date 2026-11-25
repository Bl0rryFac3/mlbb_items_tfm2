#!/usr/bin/env python3
"""Read-only verification of the supplied 0.7.4 shield/HP layout anchors."""
import hashlib,json,struct,sys
from pathlib import Path
p=Path(sys.argv[1]);data=p.read_bytes()
expected='c74955e11f4845f93dcb73f90c5cdd442615ac72f40ed642a1fdeb5f41def01e'
assert hashlib.sha256(data).hexdigest()==expected, 'Unsupported executable: do not reuse private offsets'
pe=struct.unpack_from('<I',data,0x3c)[0];opt=pe+24
sections=[]
for i in range(struct.unpack_from('<H',data,pe+6)[0]):
 s=opt+struct.unpack_from('<H',data,pe+20)[0]+40*i
 sections.append(tuple(struct.unpack_from('<I',data,s+offset)[0] for offset in [12,20,16]))
def at(rva,n):
 for va,raw,size in sections:
  if va<=rva and rva+n<=va+size:return data[raw+rva-va:raw+rva-va+n]
 raise ValueError(hex(rva))
checks={
 'sdk_shield_vector_len':(0x3510196,'488bb068020000'),
 'sdk_shield_stride':(0x35101c2,'486bd638'),
 'sdk_shield_duration':(0x35101ca,'4c89441130'),
 'shield_counter':(0x1bdc88b,'488b48304885c9741c48ffc948894830'),
 'shield_amount':(0x1be1486,'4a8b443f28'),
 'final_hp':(0x1be1572,'4d89842468060000'),
}
# The exact instruction bytes are recorded rather than inferred from field names.
for name,(rva,hex_bytes) in checks.items():
 expected_bytes=bytes.fromhex(hex_bytes)
 actual=at(rva,len(expected_bytes))
 assert actual==expected_bytes, (name,hex(rva),actual.hex(),hex_bytes)
print(json.dumps({'status':'PASS','game_sha256':expected,'entity_size':'0x6b8','shield_stride':'0x38','shield_vector':['0x258','0x260','0x268'],'shield_amount':'0x28','shield_remaining_ticks':'0x30','current_hp':'0x668','checks':{name:hex(rva) for name,(rva,_) in checks.items()}},indent=2))
