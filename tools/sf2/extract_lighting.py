#!/usr/bin/env python3
"""Extract Star Fox 2 polygon depth-colour and light-shade lookup tables.

The retail renderer stores five four-bank depth-colour families followed by
four groups of ten light-shade rows.  Verified live SF2 mission states select
the first (standard) depth family.  The light pointer catalog contains twelve
rows per group, with its final two entries aliasing row nine; validating that
catalog proves the table shape rather than merely copying a plausible byte
window.

Emits rust/sf2-data/src/lighting.rs.  Source offsets remain confined to this
oracle/data-extraction tool.
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, u16

LIGHT_POINTERS_START = 0x8AAC
DEPTH_FAMILY_START = 0x8B0C
SHADE_TABLE_START = 0x8D8C

DEPTH_FAMILY_COUNT = 5
DEPTH_BANK_COUNT = 4
DEPTH_PAIR_COUNT = 32
SHADE_GROUP_COUNT = 4
SHADE_ROW_COUNT = 10
SHADE_LEVEL_COUNT = 10
POINTER_ROWS_PER_GROUP = 12


def chunks(values: bytes, size: int) -> list[list[int]]:
    assert len(values) % size == 0
    return [list(values[start : start + size]) for start in range(0, len(values), size)]


def extract(d: bytes):
    family_size = DEPTH_BANK_COUNT * DEPTH_PAIR_COUNT
    depth_end = DEPTH_FAMILY_START + DEPTH_FAMILY_COUNT * family_size
    assert depth_end == SHADE_TABLE_START

    standard_depth = chunks(
        d[DEPTH_FAMILY_START : DEPTH_FAMILY_START + family_size],
        DEPTH_PAIR_COUNT,
    )

    shade_size = SHADE_GROUP_COUNT * SHADE_ROW_COUNT * SHADE_LEVEL_COUNT
    flat_shades = chunks(
        d[SHADE_TABLE_START : SHADE_TABLE_START + shade_size],
        SHADE_LEVEL_COUNT,
    )
    shades = [
        flat_shades[group * SHADE_ROW_COUNT : (group + 1) * SHADE_ROW_COUNT]
        for group in range(SHADE_GROUP_COUNT)
    ]

    for group in range(SHADE_GROUP_COUNT):
        for pointer_row in range(POINTER_ROWS_PER_GROUP):
            actual = u16(
                d,
                LIGHT_POINTERS_START
                + (group * POINTER_ROWS_PER_GROUP + pointer_row) * 2,
            )
            source_row = min(pointer_row, SHADE_ROW_COUNT - 1)
            expected = (
                SHADE_TABLE_START
                + (group * SHADE_ROW_COUNT + source_row) * SHADE_LEVEL_COUNT
            )
            assert actual == expected, (
                f"light pointer group {group} row {pointer_row}: "
                f"0x{actual:04X} != 0x{expected:04X}"
            )

    emit_rust(standard_depth, shades)
    return standard_depth, shades


def rust_row(values: list[int]) -> str:
    return ", ".join(f"0x{value:02X}" for value in values)


def emit_rust(standard_depth: list[list[int]], shades: list[list[list[int]]]):
    lines = [
        AUTOGEN_HEADER.format(tool="extract_lighting.py"),
        "//! Exact SF2 polygon depth-colour and light-shade palette pairs.",
        "//! Each byte packs the alternating low/high polygon palette entries.",
        "",
        f"pub const DEPTH_BANK_COUNT: usize = {DEPTH_BANK_COUNT};",
        f"pub const DEPTH_PAIR_COUNT: usize = {DEPTH_PAIR_COUNT};",
        f"pub const SHADE_GROUP_COUNT: usize = {SHADE_GROUP_COUNT};",
        f"pub const SHADE_ROW_COUNT: usize = {SHADE_ROW_COUNT};",
        f"pub const SHADE_LEVEL_COUNT: usize = {SHADE_LEVEL_COUNT};",
        "",
        "#[rustfmt::skip]",
        "pub static STANDARD_DEPTH_PAIRS: [[u8; DEPTH_PAIR_COUNT]; DEPTH_BANK_COUNT] = [",
    ]
    for row in standard_depth:
        lines.append(f"    [{rust_row(row)}],")
    lines.extend(
        [
            "];",
            "",
            "#[rustfmt::skip]",
            "pub static SHADE_PAIRS: [[[u8; SHADE_LEVEL_COUNT]; SHADE_ROW_COUNT]; SHADE_GROUP_COUNT] = [",
        ]
    )
    for group in shades:
        lines.append("    [")
        for row in group:
            lines.append(f"        [{rust_row(row)}],")
        lines.append("    ],")
    lines.extend(["];"])

    with open(os.path.join(RUST_SRC, "lighting.rs"), "w") as output:
        output.write("\n".join(lines) + "\n")
    print("  lighting.rs: 4 depth banks, 4 x 10 x 10 light-shade pairs")


if __name__ == "__main__":
    extract(load_rom())
