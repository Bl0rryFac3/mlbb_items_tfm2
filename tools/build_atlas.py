#!/usr/bin/env python3
"""Pack original item art with deterministic tier frames; no recoloring of source art."""
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import json,sys
R=Path(__file__).resolve().parents[1]
S=R/'catalog/icons';S.mkdir(exist_ok=True)
if len(sys.argv)>1:
 src=Path(sys.argv[1]); im=Image.open(src/'item_icons_640X640#sheet.png').convert('RGBA');data=json.loads((src/'item_icons_640X640#data.sprite_sheet').read_text())['images']
 for s in json.loads((R/'catalog/equipment.json').read_text()):
  r=data[{'boots':'basic_boots','mlbb_dagger':'dagger'}.get(s['key'],s['key'])];x,y,w,h=[round(r[a]*im.width) for a in ('x','y','w','h')];im.crop((x,y,x+w,y+h)).save(S/(s.get('icon_source',s['key'])+'.png'))
specs=json.loads((R/'catalog/equipment.json').read_text());canvas=Image.new('RGBA',(1280,1280));frames={}
font=ImageFont.load_default()
def put(k,tile):
 i=len(frames);x=i%20*64;y=i//20*64;canvas.alpha_composite(tile,(x,y));frames[k]={'x':x/1280,'y':y/1280,'w':.05,'h':.05}
# Proper equipment illustrations replace the placeholder letters for required stock frames.
# These remain explicitly marked TFM2 equipment in text, with original engine mechanics.
stock=[['mlbb_dagger','legion_sword','vampire_mallet','blade_of_despair','blade_of_despair'],['knife','regular_spear','rogue_meteor','windtalker','windtalker'],['leather_jerkin','steel_legplates','dreadnaught_armor','blade_armor','blade_armor'],['magic_resist_cloak','silence_robe','silence_robe','valkyries_shield','valkyries_shield'],['mystery_codex','magic_wand','tome_of_evil','lightning_truncheon','holy_crystal'],['vitality_crystal','heros_ring','ares_belt','cursed_helmet','guardian_helmet']]
for tier in range(1,6):
 for c in range(6):
  tile=Image.new('RGBA',(64,64),(29,32,42,255));art=Image.open(S/(stock[c][tier-1]+'.png')).convert('RGBA').resize((54,54),Image.Resampling.LANCZOS);tile.alpha_composite(art,(5,5));d=ImageDraw.Draw(tile);d.rectangle((1,1,62,62),outline='#687080',width=2);d.rectangle((1,50,27,62),fill='#262b38');d.text((3,51),'TFM2',font=font,fill='#d1d6df');put(f't{tier}_{c}',tile)
for s in specs:
 for upgraded in ([False,True] if s['mythical'] else [False]):
  tile=Image.new('RGBA',(64,64),(22,20,31,255));art=Image.open(S/(s.get('icon_source',s['key'])+'.png')).convert('RGBA').resize((54,54),Image.Resampling.NEAREST if s.get('custom_adaptation') else Image.Resampling.LANCZOS);tile.alpha_composite(art,(5,5));d=ImageDraw.Draw(tile)
  if upgraded:
   d.rectangle((0,0,63,63),outline='#452075',width=5);d.rectangle((2,2,61,61),outline='#a56bff',width=3);d.line((4,4,59,4),fill='#ead9ff',width=1);d.polygon([(54,46),(63,54),(54,63),(46,54)],fill='#ad76ff',outline='#f0dcff')
  else:
   d.rectangle((1,1,62,62),outline='#e0b575' if s.get('custom_adaptation') else '#9297ad' if s['tier']==3 else '#53596a',width=2)
   if s.get('custom_adaptation'):d.polygon([(49,47),(61,47),(61,61),(49,61)],fill='#745228',outline='#ffe0a1');d.line((51,57,58,50),fill='#ffe0a1',width=2);d.line((51,50,58,57),fill='#ffe0a1',width=2)
  if s.get('legacy') or s.get('restored_legacy'):d.rectangle((2,48,61,61),fill='#392d27');d.text((9,50),'LEGACY',font=font,fill='#d8b595')
  put(('mythical_' if upgraded else '')+s['key'],tile)
D=R/'aseprite_resources/ingame';canvas.save(D/'item_icons_640X640#sheet.png');(D/'item_icons_640X640#data.sprite_sheet').write_text(json.dumps({'images':frames},indent=1)+'\n')
preview=Image.new('RGB',(660,390),'#16131f');draw=ImageDraw.Draw(preview)
for i,k in enumerate(['holy_crystal','winter_crown','radiant_armor','demon_hunter_sword','immortality','blade_of_despair']):
 s=next(s for s in specs if s['key']==k);x=(i%3)*220;y=(i//3)*190
 for j,key in enumerate([k,'mythical_'+k]):
  r=frames[key];a,b=round(r['x']*1280),round(r['y']*1280);preview.paste(canvas.crop((a,b,a+64,b+64)).resize((82,82),Image.Resampling.NEAREST),(x+16+j*100,y+20))
 draw.text((x+15,y+112),s['name'],font=font,fill='white');draw.text((x+15,y+137),'Finished      Mythical',font=font,fill='#d2b2ff')
preview.save(R/'docs/mythical-preview.png')
print(len(frames),'atlas frames packed')
