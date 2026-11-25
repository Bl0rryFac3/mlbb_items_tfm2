#!/usr/bin/env python3
"""Verify MLBB Items hook targets against the supplied Windows v0.7.4 game."""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys

root = Path(__file__).resolve().parents[1]
game = Path(sys.argv[1]).read_bytes()
sha256 = hashlib.sha256(game).hexdigest()
assert sha256 == "c74955e11f4845f93dcb73f90c5cdd442615ac72f40ed642a1fdeb5f41def01e", "Unexpected v0.7.4 executable"


def u16(offset):
    return struct.unpack_from("<H", game, offset)[0]


def u32(offset):
    return struct.unpack_from("<I", game, offset)[0]


def u64(offset):
    return struct.unpack_from("<Q", game, offset)[0]


pe = u32(0x3C)
assert game[pe:pe + 4] == b"PE\0\0" and u16(pe + 4) == 0x8664
optional = pe + 24
assert u16(optional) == 0x20B
base = u64(optional + 24)
sections = {}
for index in range(u16(pe + 6)):
    entry = optional + u16(pe + 20) + index * 40
    name = game[entry:entry + 8].rstrip(b"\0")
    size, rva, raw_size, raw = struct.unpack_from("<IIII", game, entry + 8)
    sections[name] = (rva, raw, min(size, raw_size))


def at(rva, size):
    for start, raw, length in sections.values():
        if start <= rva and rva + size <= start + length:
            offset = raw + rva - start
            return game[offset:offset + size]
    raise ValueError(f"Unmapped range {rva:#x}, {size} bytes")


def relative_transfer(returns_to, target, opcode=b"\xe8"):
    assert at(returns_to - 5, 1) == opcode
    displacement = struct.unpack("<i", at(returns_to - 4, 4))[0]
    assert returns_to + displacement == target, hex(returns_to)


pdata, pdata_size = struct.unpack_from("<II", game, optional + 112 + 3 * 8)
functions = {
    start: end
    for start, end, _ in struct.iter_unpack("<III", at(pdata, pdata_size))
}
tactics = (root / "src/tactics/mod.rs").read_text()
combat = (root / "src/tactics/heal_shield.rs").read_text()
scorer = (root / "src/hooks/hook.rs").read_text()
assert len(game) == int(re.search(r"const GAME_EXE_SIZE_074: u64 = ([\d_]+)", tactics)[1])


def address(source, name):
    return int(re.search(rf"const {name}:.*?= \[?(0x[\da-fA-F]+)", source)[1], 16)


def signature(source, name):
    values = re.search(rf"const {name}:.*?= \[(.*?)\];", source, re.S)[1]
    return bytes(int(value, 16) for value in re.findall(r"0x[\da-fA-F]+", values))


checks = {}
for source, name, prologue in (
    (tactics, "RVA_BUY_ITEM", "BUY_PROLOGUE"),
    (tactics, "RVA_REALLOC", "REALLOC_PROLOGUE"),
    (tactics, "CL_LAUNCHER_RVA", "CL_LAUNCHER_PROLOGUE"),
    (tactics, "SEEDCTOR_RVA", "SEEDCTOR_PROLOGUE"),
    (tactics, "SPAWN_RVA", "SPAWN_PROLOGUE"),
    (tactics, "ITEMNET_FORWARD_RVA", "ITEMNET_FORWARD_PROLOGUE"),
    (tactics, "ROUTE_PICKER_RVA", "ROUTE_PICKER_SIG"),
    (tactics, "ROUTE_GRAPH_RVA", "ROUTE_GRAPH_SIG"),
    (combat, "HEAL_RVA", "HEAL_PROLOGUE"),
    (combat, "SHIELD_RVA", "SHIELD_PROLOGUE"),
    (combat, "SDK_SHIELD_RVA", "SDK_SHIELD_PROLOGUE"),
    (combat, "LANDING_RVA", "HEAL_PROLOGUE"),
    (combat, "MITIGATION_RVA", "MITIGATION_PROLOGUE"),
):
    rva, expected = address(source, name), signature(source, prologue)
    assert rva in functions and at(rva, len(expected)) == expected, name
    checks[name] = hex(rva)

text_rva, text_raw, text_size = sections[b".text"]
text = game[text_raw:text_raw + text_size]
needle = signature(scorer, "FALLBACK_SIGNATURE")
assert text.count(needle) == 1
build_scorer = text_rva + text.index(needle)
assert functions[build_scorer] - build_scorer == 2709
checks["build_scorer"] = hex(build_scorer)

for name in ("final_gate", "tick_finals_cap", "row_floor", "empty_row_floor", "result_row_floor"):
    body = re.search(rf"unsafe fn patch_{name}\([^\n]*\{{(.*?)\n\}}", tactics, re.S)[1]
    rva = int(re.search(r"exe_base_addr\(\) \+ (0x[\da-f]+)", body)[1], 16)
    expected = re.search(r"const EXPECT:.*?= \[(.*?)\];", body, re.S)[1]
    byte_checks = re.findall(r"\((\d+), (0x[\da-f]+)\)", expected)
    assert byte_checks, name
    for offset, value in byte_checks:
        assert at(rva + int(offset), 1)[0] == int(value, 16), (name, offset)
    checks[name] = hex(rva)

relative_transfer(address(tactics, "RVA_BUY_ITEM") + 0xD6, address(tactics, "ROUTE_PICKER_RVA"))
launcher = address(tactics, "CL_LAUNCHER_RVA")
seed_ctor = address(tactics, "SEEDCTOR_RVA")
relative_transfer(launcher + 0x1AF, seed_ctor)
relative_transfer(launcher + 0x284, seed_ctor)
client_returns = re.search(r"const CLIENT_MATCH_RETURNS:.*?= \[(.*?)\];", tactics)[1]
for value in client_returns.split(","):
    relative_transfer(int(value, 16), launcher)
assert at(seed_ctor + 0x726, 7) == bytes.fromhex("488986f8420100")
for name in ("REGEN_RETURN_RVAS", "VAMP_RETURN_RVA"):
    relative_transfer(address(combat, name), address(combat, "HEAL_RVA"))
relative_transfer(0x19E552D, address(combat, "LANDING_RVA"))
# Two copies of the mitigation body share the same entry bytes. The entity
# damage path must reach the hooked copy.
relative_transfer(0x19E47C5, address(combat, "MITIGATION_RVA"))
# Validate the scorer from the two training call sites and through its beam
# search to the scorer used by the extra slots.
relative_transfer(0x2251E87, build_scorer)
relative_transfer(0x2251EB5, build_scorer)
relative_transfer(0x2BD1835, 0x18CBA80)
relative_transfer(0x18CC50C, address(tactics, "ITEMNET_FORWARD_RVA"))
relative_transfer(int(checks["empty_row_floor"], 16) + 10, int(checks["row_floor"], 16) + 13, b"\xe9")

# The purchase-independent landing call puts AttackType in its fifth argument.
# ApplyDamage reads the source owner and the target's native nonlethal flag.
for name, rva, expected in [
    ("sustain_attack_argument", 0x19E551A, "89442420"),
    ("sustain_source_owner", 0x1BE1181, "4d8b58084983fbff"),
    ("sustain_nonlethal_flag", 0x1BE155B, "4180bc248004000000"),
    ("game_catalog_vector", 0x199CC6B, "498d88381d0000"),
    ("spawn_game_provider", 0x19A658F, "488b99401b0000"),
    ("spawn_athlete_copy_size", 0x19A5D68, "41b8b8080000"),
]:
    data = bytes.fromhex(expected)
    assert at(rva, len(data)) == data, name
    checks[name] = hex(rva)

table = 0x4385B90
for slot, rva in [(39, 0x35118D0), (72, address(combat, "SDK_SHIELD_RVA")), (81, 0x350FF90)]:
    assert struct.unpack("<Q", at(table + slot * 8, 8))[0] == base + rva
print(json.dumps({
    "sha256": sha256,
    "bytes": len(game),
    "PE": "AMD64",
    "game_version": "0.7.4",
    "checks": checks,
    "stable_vtable": hex(table),
    "result": "PASS (static checks)",
}, indent=2))
