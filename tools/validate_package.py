#!/usr/bin/env python3
from pathlib import Path
import json,re
from PIL import Image
R=Path(__file__).resolve().parents[1]
spec=json.loads((R/'catalog/equipment.json').read_text());mechanics=json.loads((R/'catalog/mechanics.json').read_text())
raw=json.loads((R/'catalog/arena-import.json').read_text());raw['mlbb_dagger']=raw['dagger']
keys={s['key'] for s in spec}|{'mythical_'+s['key'] for s in spec if s['mythical']}
meta=json.loads((R/'mod.mod_info').read_text())
expected = 169
pairs = 58
body = (R/'src/mlbb_registration.rs').read_text()
order = re.findall(r'let key = "([^"]+)";', body)
frozen = json.loads((R/'catalog/rc22_registry_order.json').read_text())
assert len(spec)==expected-pairs and len(order)==len(keys)==expected
assert order[:len(frozen)]==frozen and set(order)==keys
assert order[len(frozen):]==[
 'mythical_legacy_windtalker','mythical_bloodlust_axe',
 'calamity_reaper','mythical_calamity_reaper','scarlet_phantom','mythical_scarlet_phantom',
 'shadow_twinblades','mythical_shadow_twinblades','ice_force','mythical_ice_force',
 'bloodthirsty_king','mythical_bloodthirsty_king',
]
by_key = {item['key']:item for item in spec}
forges = json.loads((R/'catalog/forged_components.json').read_text())
assert {forge['key'] for forge in forges}=={item['key'] for item in spec if item['tier']==2}
for forge in forges:
 item = by_key[forge['key']]
 assert item['stats']==forge['stats'] and item['price']==forge['price']
 assert item['previous']==forge['parents'] and item['next']==forge['children']
 edges = [(by_key[key], item) for key in item['previous']]
 edges += [(item, by_key[key]) for key in item['next']]
 for parent, child in edges:
  assert parent['key'] in child['previous'] and child['key'] in parent['next']
  assert parent['tier'] < child['tier'] and parent['price'] < child['price']
  assert all(value <= child['stats'].get(stat, 0) for stat, value in parent['stats'].items()), (parent['key'], child['key'])
assert len([s for s in spec if s['mythical']])==pairs
assert len([s for s in spec if s['boots'] and s['tier']==3])==7
assert not set(json.loads((R/'catalog/exclusions.json').read_text()))&keys
assert 'dagger' not in keys and 'mlbb_dagger' in keys
assert 'radiant_armor' in keys and 'mythical_radiant_armor' in keys
for s in spec:
 assert s['price']>0 and s['price']%5==0
 assert len(s['previous'])==len(set(s['previous']))
 assert len(s['next'])==len(set(s['next']))
 assert all(k in keys for k in s['next']+s['previous'])
 if not s.get('restored_legacy') and not s.get('custom_adaptation') and (raw[s['key']]['has_passive'] or raw[s['key']]['has_active']):assert s['key'] in mechanics
texts=json.loads((R/'text/item.i18n').read_text());ui=json.loads((R/'text/ui.i18n').read_text())
for locale,entries in texts.items():
 assert keys<=entries.keys()
 for k in keys:assert entries[k]['name'] and 'TODO' not in entries[k]['option']
 assert entries['mythical_radiant_armor']['name']=='Mythical Radiant Armor'
for locale,entries in ui.items():assert entries['builds']['item_rules_all']=='All Mythicals'
D=R/'aseprite_resources/ingame';frames=json.loads((D/'item_icons_640X640#data.sprite_sheet').read_text())['images'];im=Image.open(D/'item_icons_640X640#sheet.png')
assert len(frames)==expected+30 and im.size==(1280,1280) and im.mode=='RGBA'
assert keys<=frames.keys();rects=set()
for k,r in frames.items():
 rect=tuple(round(r[c]*1280) for c in ['x','y','w','h']);assert rect not in rects;rects.add(rect)
 x,y,w,h=rect;assert w==h==64 and x+w<=1280 and y+h<=1280
 if k.startswith('mythical_'):assert im.getpixel((x+2,y+2))==(165,107,255,255)
meta=json.loads((R/'mod.mod_info').read_text());assert meta['mod_id']=='mlbb_items' and meta['dependencies']==[{'mod_id':'base','version':'=0.7.4'}]
for old,new in json.loads((R/'mod.override_info').read_text()).items():
 address=new['remapping'];assert address.startswith('asset/mlbb_items/')
 rel=address.removeprefix('asset/mlbb_items/')
 assert any((R/(rel+ext)).is_file() for ext in ['', '.ui','.sprite_sheet','.png','.i18n','.item_setting']),address
hook=(R/'src/hooks/hook.rs').read_text();assert 'create_trampoline(target)?' in hook and 'patch_target(target)?' in hook and 'game_core::' in hook
assert '    6\n}' in (R/'src/build_config.rs').read_text()
assert 'VERSION_0_7_2' in (R/'src/tactics/mod.rs').read_text() or '0.7.2' in (R/'src/tactics/mod.rs').read_text()
print(f'PASS: {len(spec)} base items (including 6 Forged and 7 restored classic jungle items), {pairs} upgrade pairs, {expected} registrations, {len(frames)} distinct atlas frames, {len(texts)} tooltip locales, recipe links, behavior coverage, overrides and hook source invariants.')

from build_patch_defaults import defaults
patch_defaults=json.loads((R/'config-default.json').read_text())
assert patch_defaults==defaults(spec), 'Regenerate MLBB patch defaults'
assert set(patch_defaults)==keys
assert patch_defaults['blade_of_despair']['attack']>0
assert patch_defaults['mythical_blade_of_despair']['attack']>patch_defaults['blade_of_despair']['attack']
print(f'PASS: {expected} MLBB config entries match catalog stats and Mythical scaling; no unsupported passive fields.')
