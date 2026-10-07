#!/usr/bin/env python3
"""Decode the two surface-selected ambient-particle color-pair tables.

The generator at $01:C983..C995 takes a two-bit random sample and adds
either zero or four, so exactly eight bytes of each selected table are
reachable. Selection is ROM bank $19, offsets $FF5D/$FF6D. These are
palette-pair bytes, not signed height values or executable programs.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom

STARTS = (0xCFF5D, 0xCFF6D)
PAIRS_PER_PALETTE = 8


def decode(data: bytes) -> tuple[bytes, bytes]:
    if len(data) < STARTS[-1] + PAIRS_PER_PALETTE:
        raise ValueError("truncated surface ambient palette")
    return tuple(data[start:start + PAIRS_PER_PALETTE] for start in STARTS)


def render_rust(palettes) -> str:
    lines = [
        AUTOGEN_HEADER.format(tool="extract_surface_particles.py"),
        "//! Exact color-pair bytes for surface-selected ambient particles.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum SurfaceParticlePalette {",
        "    Negative,",
        "    Nonnegative,",
        "}",
        "",
        "impl SurfaceParticlePalette {",
        "    pub const fn colors(self) -> &'static [u8; 8] {",
        "        match self {",
    ]
    for name, row in zip(("Negative", "Nonnegative"), palettes):
        values = ", ".join(f"0x{byte:02X}" for byte in row)
        lines.append(f"            Self::{name} => &[{values}],")
    lines.extend(["        }", "    }", "}"])
    return "\n".join(lines) + "\n"


def extract(data: bytes):
    decoded = decode(data)
    Path(RUST_SRC, "surface_particles.rs").write_text(render_rust(decoded))
    print("  surface_particles.rs: two exact eight-pair ambient palettes")
    return decoded


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        assert Path(RUST_SRC, "surface_particles.rs").read_text() == render_rust(decode(load_rom()))
        print("surface_particles.rs matches source tables")
    else:
        extract(load_rom())
