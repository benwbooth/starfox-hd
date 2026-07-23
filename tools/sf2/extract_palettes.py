#!/usr/bin/env python3
"""Extract SF2's exact five-entry polygon-palette catalog.

The retail host palette loader indexes five contiguous 16-color BGR555 rows
beginning at file offset ``0x8A0C``. Live CGRAM captures identify catalog row
0 as the standard space/interior palette, row 3 as Eladard's exterior
palette, and row 4 as Astropolis's exterior-entry palette.

The generated Rust module contains ordinary typed assets. Source addresses
and ROM access remain confined to this extraction tool.
"""

from __future__ import annotations

import os
import subprocess

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, u16


PALETTE_CATALOG_OFFSET = 0x8A0C
PALETTE_COUNT = 5
COLORS_PER_PALETTE = 16
BYTES_PER_COLOR = 2


def extract(data: bytes) -> tuple[tuple[int, ...], ...]:
    byte_count = PALETTE_COUNT * COLORS_PER_PALETTE * BYTES_PER_COLOR
    end = PALETTE_CATALOG_OFFSET + byte_count
    if len(data) < end:
        raise RuntimeError("truncated SF2 polygon-palette catalog")

    palettes = tuple(
        tuple(
            u16(
                data,
                PALETTE_CATALOG_OFFSET
                + palette * COLORS_PER_PALETTE * BYTES_PER_COLOR
                + color * BYTES_PER_COLOR,
            )
            for color in range(COLORS_PER_PALETTE)
        )
        for palette in range(PALETTE_COUNT)
    )
    if any(palette[0] != 0 for palette in palettes):
        raise RuntimeError("SF2 polygon palette color zero must be transparent")

    emit_rust(palettes)
    print(f"  palettes.rs: {len(palettes)} exact BGR555 polygon palettes")
    return palettes


def emit_rust(palettes: tuple[tuple[int, ...], ...]) -> None:
    lines = [
        AUTOGEN_HEADER.format(tool="extract_palettes.py"),
        "//! Exact SF2 polygon-palette catalog as ordinary BGR555 assets.",
        "//!",
        "//! Shipping code selects these rows by semantic scene identity; the",
        "//! source catalog position is represented only by this typed data id.",
        "",
        "pub const COLORS_PER_POLYGON_PALETTE: usize = 16;",
        f"pub const POLYGON_PALETTE_COUNT: usize = {len(palettes)};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "#[repr(u8)]",
        "pub enum PolygonPaletteId {",
        "    Standard = 0,",
        "    CatalogOne = 1,",
        "    CatalogTwo = 2,",
        "    EladardSurface = 3,",
        "    AstropolisExterior = 4,",
        "}",
        "",
        "impl PolygonPaletteId {",
        "    pub const fn colors(self) -> &'static [u16; COLORS_PER_POLYGON_PALETTE] {",
        "        &POLYGON_PALETTES[self as usize]",
        "    }",
        "}",
        "",
        "#[rustfmt::skip]",
        "pub static POLYGON_PALETTES:",
        "    [[u16; COLORS_PER_POLYGON_PALETTE]; POLYGON_PALETTE_COUNT] = [",
    ]
    for palette in palettes:
        values = ", ".join(f"0x{value:04X}" for value in palette)
        lines.append(f"    [{values}],")
    lines.append("];")

    output = os.path.join(RUST_SRC, "palettes.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)


if __name__ == "__main__":
    extract(load_rom())
