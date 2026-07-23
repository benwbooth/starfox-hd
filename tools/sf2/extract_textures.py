#!/usr/bin/env python3
"""Extract SF2's exact polygon-texture descriptors, layouts, and banks.

The retail GSU polygon-colour routine at file offsets ``0x9E4B..0x9E7D``
indexes a three-byte descriptor table at GSU bank-01 address ``$8703`` and a
layout-pointer table at ``$897C``.  The descriptor table ends exactly where
the layout pointers begin.  Its live entries address packed-nibble texture
data in GSU ROM banks ``$12``, ``$13``, and ``$14``.

Emits ``rust/sf2-data/src/textures.rs`` so the shipping renderer consumes
ordinary generated Rust data and never needs a ROM or emulated address space.
"""

from __future__ import annotations

import os
import subprocess

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, u16


DESCRIPTOR_START = 0x8703
LAYOUT_POINTER_START = 0x897C
LAYOUT_COUNT = 12
LAYOUT_RECORD_SIZE = 10
TEXTURE_ROM_BANKS = (0x12, 0x13, 0x14)
ROM_BANK_SIZE = 0x8000


def bank_file_offset(bank: int) -> int:
    return bank * ROM_BANK_SIZE


def extract(data: bytes):
    descriptor_bytes = LAYOUT_POINTER_START - DESCRIPTOR_START
    if descriptor_bytes % 3:
        raise RuntimeError("SF2 texture descriptor table is not three-byte aligned")

    descriptors: list[tuple[int, int]] = []
    for index in range(descriptor_bytes // 3):
        offset = DESCRIPTOR_START + index * 3
        address = u16(data, offset)
        source_bank = data[offset + 2]
        if source_bank == 0:
            descriptors.append((255, 0))
            continue
        if source_bank not in TEXTURE_ROM_BANKS or address < 0x8000:
            raise RuntimeError(
                f"invalid texture descriptor {index}: ${source_bank:02X}:${address:04X}"
            )
        descriptors.append((TEXTURE_ROM_BANKS.index(source_bank), address - 0x8000))

    layouts: list[tuple[int, tuple[tuple[int, int], ...]]] = []
    first_layout = LAYOUT_POINTER_START + LAYOUT_COUNT * 2
    for index in range(LAYOUT_COUNT):
        address = u16(data, LAYOUT_POINTER_START + index * 2)
        expected = first_layout + index * LAYOUT_RECORD_SIZE
        if address != expected:
            raise RuntimeError(
                f"texture layout {index} points to ${address:04X}, expected ${expected:04X}"
            )
        mask = u16(data, address)
        coords = tuple(
            (data[address + 2 + vertex * 2], data[address + 3 + vertex * 2])
            for vertex in range(4)
        )
        layouts.append((mask, coords))

    banks = [
        data[bank_file_offset(bank) : bank_file_offset(bank) + ROM_BANK_SIZE]
        for bank in TEXTURE_ROM_BANKS
    ]
    if any(len(bank) != ROM_BANK_SIZE for bank in banks):
        raise RuntimeError("truncated SF2 texture ROM bank")

    emit_rust(descriptors, layouts, banks)
    print(
        f"  textures.rs: {len(descriptors)} descriptors, "
        f"{len(layouts)} layouts, {len(banks)} packed-nibble banks"
    )
    return descriptors, layouts, banks


def emit_rust(descriptors, layouts, banks):
    lines = [
        AUTOGEN_HEADER.format(tool="extract_textures.py"),
        "//! Exact SF2 polygon-texture descriptors, coordinate layouts, and data.",
        "//!",
        "//! The shipping renderer treats the generated bank index and offset as a",
        "//! flat asset lookup. The source GSU bank/address split is retained only",
        "//! in these extraction constants and never becomes runtime game state.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct TextureSprite {",
        "    pub bank: u8,",
        "    pub offset: u16,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct TextureLayout {",
        "    pub mask: u16,",
        "    pub coords: [[u8; 2]; 4],",
        "}",
        "",
        f"pub const TEXTURE_DESCRIPTOR_COUNT: usize = {len(descriptors)};",
        f"pub const TEXTURE_LAYOUT_COUNT: usize = {len(layouts)};",
        f"pub const TEXTURE_BANK_SIZE: usize = {ROM_BANK_SIZE};",
        "pub const UNUSED_TEXTURE_BANK: u8 = 255;",
        "",
        "#[rustfmt::skip]",
        "pub static TEXTURE_SPRITES: [TextureSprite; TEXTURE_DESCRIPTOR_COUNT] = [",
    ]
    for bank, offset in descriptors:
        lines.append(f"    TextureSprite {{ bank: {bank}, offset: 0x{offset:04X} }},")
    lines.extend(["];"])
    lines.extend(["", "#[rustfmt::skip]"])
    lines.append("pub static TEXTURE_LAYOUTS: [TextureLayout; TEXTURE_LAYOUT_COUNT] = [")
    for mask, coords in layouts:
        coords_rust = ", ".join(f"[{x}, {y}]" for x, y in coords)
        lines.append(
            f"    TextureLayout {{ mask: 0x{mask:04X}, coords: [{coords_rust}] }},"
        )
    lines.extend(["];"])

    for index, bank in enumerate(banks):
        lines.extend(["", "#[rustfmt::skip]"])
        lines.append(
            f"pub static TEXTURE_BANK_{index}: [u8; TEXTURE_BANK_SIZE] = ["
        )
        for offset in range(0, len(bank), 16):
            row = ", ".join(f"0x{value:02X}" for value in bank[offset : offset + 16])
            lines.append(f"    {row},")
        lines.append("];")

    output = os.path.join(RUST_SRC, "textures.rs")
    with open(output, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
    subprocess.run(["rustfmt", "--edition", "2021", output], check=True)


if __name__ == "__main__":
    extract(load_rom())
