#!/usr/bin/env python3
"""Generate registrations without reordering the catalog: saved indices must stay stable."""
from pathlib import Path
import json
r=Path(__file__).resolve().parents[1]
specs=json.loads((r/"catalog/equipment.json").read_text())
frozen = json.loads((r/"catalog/rc22_registry_order.json").read_text())
entries = {}
for spec in specs:
 entries[spec['key']] = (spec, False)
 if spec['mythical']:
  entries['mythical_' + spec['key']] = (spec, True)
assert set(frozen) <= entries.keys()
order = frozen + [key for key in entries if key not in frozen]
lines=['// Generated from catalog/equipment.json; see tools/build_registration.py.']
for k in order:
 s, myth = entries[k]
 lines.append(f'fn make_{k}(config: &crate::config::ItemConfig) -> MlbbItem {{ make("{s["key"]}", {str(myth).lower()}, Some(config)) }}')
lines.append('pub fn register(reg: &mut StableMod, configs: &std::collections::HashMap<String, crate::config::ItemConfig>) {');lines.append('let mut sustain = std::collections::HashMap::new();')
for k in order:
 s, myth = entries[k]
 lines += [f'{{ let key = "{k}"; let item = make("{s["key"]}", {str(myth).lower()}, configs.get(key));','if rules::sustain_kind(key).is_some() { sustain.insert(key.to_owned(), item.stats.vamp.max(0) as usize); }','crate::item_stats::note_registered(key, item.tier()); crate::smart_builds::note_mod_item(key, &item);','crate::route_choice::note_mod_item(key, &item); crate::match_builds::note_mod_item(key, &item);',('crate::strategy_ui::note_final_item(key);' if s['key'] == 'twilight_armor' else 'if is_final_key(key) { crate::strategy_ui::note_final_item(key); }'),f'crate::patches::note_mod_item(key, &item, make_{k}, |item| item.stats); reg.add_item(crate::perf::timed(key, item, make_{k})); }}']
lines.append('rules::register_sustain(sustain);');lines.append('}');(r/'src/mlbb_registration.rs').write_text('\n'.join(lines)+'\n')
