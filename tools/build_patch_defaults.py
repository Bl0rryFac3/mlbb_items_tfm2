#!/usr/bin/env python3
"""Expose only implemented MLBB BuffV1 overrides to the inherited patcher.

Defaults match mlbb::make, including Rust's positive f64::round for Mythicals.
Do not add hardcoded passive formulas here: those are not configurable hooks.
"""
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIELDS = '''attack magic_power hp hp_regen defence magic_resistance vamp
crit_chance attack_speed_mult move_speed_mult skill_cooldown_mult ult_cooldown_mult
defence_penetration magic_resistance_penetration toughness base_attack_damaged_reduce
magic_power_mult'''.split()
BOOST = set('attack magic_power hp hp_regen defence magic_resistance attack_speed_mult'.split())


def defaults(specs):
    result = {}
    for spec in specs:
        for mythical in ([False, True] if spec['mythical'] else [False]):
            stats = {field: spec['stats'].get(field, 0) for field in FIELDS}
            if mythical:
                for field in BOOST:
                    stats[field] = math.floor(stats[field] * 1.35 + 0.5)
                stats['skill_cooldown_mult'] += 5
                stats['ult_cooldown_mult'] += 5
            key = ('mythical_' if mythical else '') + spec['key']
            result[key] = {field: value for field, value in stats.items() if value}
    return result


if __name__ == '__main__':
    entries = defaults(json.loads((ROOT / 'catalog/equipment.json').read_text()))
    (ROOT / 'config-default.json').write_text(json.dumps(entries, indent=2) + '\n')
    print(f'Wrote {len(entries)} MLBB stat defaults; passive formulas remain fixed.')
