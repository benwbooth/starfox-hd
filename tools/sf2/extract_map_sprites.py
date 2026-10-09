#!/usr/bin/env python3
"""Extract Star Fox 2's strategic-map sprite catalog.

The map's sprite pass (bank 04) composes its OAM from bank-18 tables: the
animated sprite catalog at 18:AF24 (per-animation frame lists and sprite
pieces), the HUD and icon tile words at 18:BDA5..18:BF15, the icon
animation scripts at 18:C037 and the metasprite pointer table at 18:C0FD.
They are one contiguous catalog that runs to the end of the bank; the
native sprite pass reads it by its bank-18 address.

Emits rust/sf2-data/src/map_sprites.rs.
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom

BANK = 0x18
START = 0xAF24
END = 0x10000


def file_offset(address: int) -> int:
    return BANK * 0x8000 + (address & 0x7FFF)


def extract(d: bytes) -> None:
    data = d[file_offset(START) : file_offset(START) + (END - START)]
    assert len(data) == END - START
    lines = [AUTOGEN_HEADER.format(tool="extract_map_sprites.py")]
    lines.append("//! The strategic map's sprite catalog (bank 18 from `$18:AF24`).\n\n")
    lines.append("/// The catalog's first bank-18 address.\n")
    lines.append(f"pub const MAP_SPRITE_BASE: u16 = 0x{START:04X};\n\n")
    lines.append(f"/// `$18:{START:04X}..$18:FFFF`.\n")
    lines.append(f"pub static MAP_SPRITES: [u8; 0x{len(data):04X}] = [\n")
    for i in range(0, len(data), 16):
        lines.append("    " + " ".join(f"0x{b:02X}," for b in data[i : i + 16]) + "\n")
    lines.append("];\n")
    path = os.path.join(RUST_SRC, "map_sprites.rs")
    with open(path, "w") as f:
        f.write("".join(lines))
    print(f"  map sprites: {len(data)} bytes -> {path}")


if __name__ == "__main__":
    extract(load_rom())
