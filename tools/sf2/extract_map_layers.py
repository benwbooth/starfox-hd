#!/usr/bin/env python3
"""Extract the strategic map's terrain grid and palette rows.

The map entry ($04:DCBC) decompresses both with the Super FX decompressor
($01:D9FF, ported in decompress.py): $04:EB80 takes the stream ending at
$19:B33C and keeps every other byte of its first 0x800 as the terrain grid
($7F:D400, one byte per 8x8 cell); $04:EC76 takes the stream ending at
$18:AF24 (output based at GSU RAM $6E18) and copies the palette shadow's
rows from it: $8258 (0x200 bytes) to EFE5, then $8458, $8958 and $8658
(0x100 each) to the cycle sources at F1E5, F2E5 and F3E5.

Emits rust/sf2-data/src/map_layers.rs.
"""

from __future__ import annotations

import os

from decompress import decompress
from rom import AUTOGEN_HEADER, RUST_SRC, load_rom

TERRAIN_STREAM = (0x19, 0xB33C)
TERRAIN_BYTES = 0x400
PALETTE_STREAM = (0x18, 0xAF24)
PALETTE_BASE = 0x6E18
PALETTE_ROWS = ((0x8258, 0x200), (0x8458, 0x100), (0x8958, 0x100), (0x8658, 0x100))


def terrain(d: bytes) -> bytes:
    data = decompress(d, *TERRAIN_STREAM)
    return bytes(data[0 : 2 * TERRAIN_BYTES : 2])


def palette(d: bytes) -> bytes:
    data = decompress(d, *PALETTE_STREAM)
    out = b""
    for start, length in PALETTE_ROWS:
        out += data[start - PALETTE_BASE : start - PALETTE_BASE + length]
    return out


def table(name: str, doc: str, data: bytes) -> list[str]:
    lines = [f"/// {doc}\n", f"pub static {name}: [u8; 0x{len(data):04X}] = [\n"]
    for i in range(0, len(data), 16):
        lines.append("    " + " ".join(f"0x{b:02X}," for b in data[i : i + 16]) + "\n")
    lines.append("];\n")
    return lines


def extract(d: bytes) -> None:
    grid = terrain(d)
    rows = palette(d)
    lines = [AUTOGEN_HEADER.format(tool="extract_map_layers.py")]
    lines.append("//! The strategic map's terrain grid and palette rows, as the map entry\n")
    lines.append("//! (`$04:DCBC`) decompresses them.\n\n")
    lines += table("MAP_TERRAIN", "`$7F:D400`: one byte per 8x8 cell (from `$19:B33C`).", grid)
    lines.append("\n")
    lines += table("MAP_PALETTE", "EFE5..F4E5: the palette shadow and its cycle sources (from `$18:AF24`).", rows)
    path = os.path.join(RUST_SRC, "map_layers.rs")
    with open(path, "w") as f:
        f.write("".join(lines))
    print(f"  map layers: {len(grid)} + {len(rows)} bytes -> {path}")


if __name__ == "__main__":
    extract(load_rom())
