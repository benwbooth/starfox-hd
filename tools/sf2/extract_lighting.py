#!/usr/bin/env python3
"""Extract Star Fox 2 polygon depth-colour and light-shade lookup tables.

The retail renderer stores five four-bank depth-colour families followed by
four groups of ten light-shade rows and fourteen depth-threshold records.
The light pointer catalog contains twelve
rows per group, with its final two entries aliasing row nine; validating that
catalog proves the table shape rather than merely copying a plausible byte
window.

Emits rust/sf2-data/src/lighting.rs.  Source offsets remain confined to this
oracle/data-extraction tool.
"""

from __future__ import annotations

import argparse
from pathlib import Path

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
DEPTH_THRESHOLDS_START = 0x8F1C
DEPTH_THRESHOLD_COUNT = 14
DEPTH_THRESHOLD_STRIDE = 4


def chunks(values: bytes, size: int) -> list[list[int]]:
    assert len(values) % size == 0
    return [list(values[start : start + size]) for start in range(0, len(values), size)]


def decode(d: bytes):
    required_length = DEPTH_THRESHOLDS_START + DEPTH_THRESHOLD_COUNT * DEPTH_THRESHOLD_STRIDE
    if len(d) < required_length:
        raise ValueError("source ends before the complete lighting catalog")
    family_size = DEPTH_BANK_COUNT * DEPTH_PAIR_COUNT
    depth_end = DEPTH_FAMILY_START + DEPTH_FAMILY_COUNT * family_size
    assert depth_end == SHADE_TABLE_START

    depth_families = [
        chunks(d[start : start + family_size], DEPTH_PAIR_COUNT)
        for start in range(DEPTH_FAMILY_START, depth_end, family_size)
    ]

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

    assert SHADE_TABLE_START + shade_size == DEPTH_THRESHOLDS_START
    thresholds = []
    for row in range(DEPTH_THRESHOLD_COUNT):
        start = DEPTH_THRESHOLDS_START + row * DEPTH_THRESHOLD_STRIDE
        record = d[start : start + DEPTH_THRESHOLD_STRIDE]
        assert len(record) == DEPTH_THRESHOLD_STRIDE and record[3] == 0
        thresholds.append([value if value < 128 else value - 256 for value in record[:3]])
    return depth_families, shades, thresholds


def rust_row(values: list[int]) -> str:
    return ", ".join(f"0x{value:02X}" for value in values)


def render_rust(depth_families, shades, thresholds) -> str:
    lines = [
        AUTOGEN_HEADER.format(tool="extract_lighting.py"),
        "//! Exact SF2 polygon depth-colour and light-shade palette pairs.",
        "//! Each byte packs the alternating low/high polygon palette entries.",
        "",
        f"pub const DEPTH_BANK_COUNT: usize = {DEPTH_BANK_COUNT};",
        f"pub const DEPTH_PAIR_COUNT: usize = {DEPTH_PAIR_COUNT};",
        f"pub const DEPTH_FAMILY_COUNT: usize = {DEPTH_FAMILY_COUNT};",
        f"pub const DEPTH_THRESHOLD_COUNT: usize = {DEPTH_THRESHOLD_COUNT};",
        f"pub const SHADE_GROUP_COUNT: usize = {SHADE_GROUP_COUNT};",
        f"pub const SHADE_ROW_COUNT: usize = {SHADE_ROW_COUNT};",
        f"pub const SHADE_LEVEL_COUNT: usize = {SHADE_LEVEL_COUNT};",
        "",
        "#[rustfmt::skip]",
        "pub static DEPTH_PAIRS: [[[u8; DEPTH_PAIR_COUNT]; DEPTH_BANK_COUNT]; DEPTH_FAMILY_COUNT] = [",
    ]
    for family in depth_families:
        lines.append("    [")
        for row in family:
            lines.append(f"        [{rust_row(row)}],")
        lines.append("    ],")
    lines.extend(
        [
            "];",
            "",
            "pub static STANDARD_DEPTH_PAIRS: [[u8; DEPTH_PAIR_COUNT]; DEPTH_BANK_COUNT] = DEPTH_PAIRS[0];",
            "",
            "#[rustfmt::skip]",
            "pub static DEPTH_THRESHOLDS: [[i8; 3]; DEPTH_THRESHOLD_COUNT] = [",
        ]
    )
    for row in thresholds:
        lines.append(f"    [{', '.join(map(str, row))}],")
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

    return "\n".join(lines) + "\n"


def extract(d: bytes):
    decoded = decode(d)
    Path(RUST_SRC, "lighting.rs").write_text(render_rust(*decoded))
    print("  lighting.rs: 5 depth families, 14 threshold rows, 4 x 10 x 10 light-shade pairs")
    return decoded


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        assert Path(RUST_SRC, "lighting.rs").read_text() == render_rust(*decode(load_rom()))
        print("lighting.rs matches source tables")
    else:
        extract(load_rom())
