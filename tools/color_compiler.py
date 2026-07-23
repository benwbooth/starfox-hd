#!/usr/bin/env python3
"""Compile Star Fox 1 shape colour/material tables from the assembled ROM.

The shape sources store pointers to per-shape colour tables, and those tables
may in turn point at variable-length animation records.  The old Rust renderer
discarded the ShapeHdr pointer and hand-copied only six partial tables, which
made every custom animation/texture resolve incorrectly.  This generator uses
the symbol-matched assembled ROM as the byte-exact oracle and emits standalone
Rust data; the normal Cargo build does not need the ROM.
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import sys
from typing import Dict, List, Set

ROOT = Path(__file__).resolve().parents[1]
ROM_PATH = ROOT / "rust/sf-oracle/data/sf.sfc"
SYMBOLS_PATH = ROOT / "rust/sf-oracle/data/symbols.txt"
OUTPUT_PATH = ROOT / "rust/sf-render/src/color_data.rs"


def load_shape_compiler():
    path = ROOT / "tools/shape_compiler.py"
    spec = importlib.util.spec_from_file_location("starfox_shape_compiler", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_symbols() -> Dict[str, int]:
    symbols: Dict[str, int] = {}
    for raw in SYMBOLS_PATH.read_text(errors="replace").splitlines():
        fields = raw.split()
        if len(fields) >= 2 and fields[1].startswith("$"):
            symbols[fields[0].lower()] = int(fields[1][1:], 16)
    return symbols


def lorom_offset(address: int) -> int:
    bank = (address >> 16) & 0x7F
    offset = address & 0xFFFF
    if offset < 0x8000:
        raise ValueError(f"not a LoROM ROM address: ${address:06x}")
    return bank * 0x8000 + offset - 0x8000


def read_u16(rom: bytes, address: int, index: int = 0) -> int:
    off = lorom_offset(address) + index * 2
    return rom[off] | (rom[off + 1] << 8)


def discover_table_lengths(sc) -> Dict[str, int]:
    """Return the number of material words actually indexed by shape faces."""
    asm_files = []
    inc_symbols: Dict[str, str] = {}
    for rel in sc.INC_SYMBOL_FILES:
        inc = sc.load_asm_file(os.path.join(sc.REPO_ROOT, rel))
        inc_symbols.update(inc.symbols)
    for rel in sc.SHAPE_ASM_FILES:
        af = sc.load_asm_file(os.path.join(sc.REPO_ROOT, rel))
        for name, expr in inc_symbols.items():
            af.symbols.setdefault(name, expr)
        sc.resolve_all_symbols(af)
        asm_files.append(af)

    lengths: Dict[str, int] = {}
    for af in asm_files:
        for hdr in sc.parse_shape_headers(af):
            table = hdr.color_table.strip().lower()
            if table == "0" or hdr.faces_label == "0":
                continue
            faces = sc.parse_faces(af, hdr.faces_label)
            if not faces:
                for other in asm_files:
                    if other is af:
                        continue
                    faces = sc.parse_faces(other, hdr.faces_label)
                    if faces:
                        break
            if faces:
                lengths[table] = max(
                    lengths.get(table, 0), max(f.color_index for f in faces) + 1
                )

    # Complete shared tables and live draw-list overrides.  ID_0/1 boundaries
    # come directly from adjacent symbols; ID_2..5 contain 48 words each.
    lengths.update(
        {
            "id_0_c": 109,
            "id_1_c": 106,
            "id_2_c": 48,
            "id_3_c": 48,
            "id_5_c": 48,
            "black_c": 64,
            "white_c": 56,
        }
    )
    # Great Commander swaps to this at runtime after both flames die.
    lengths["bonfire_c"] = max(lengths.get("bonfire_c", 0), 2)
    return lengths


def rust_ident(name: str) -> str:
    return "".join(c.upper() if c.isalnum() else "_" for c in name)


def main() -> int:
    if not ROM_PATH.exists() or not SYMBOLS_PATH.exists():
        print("color compiler needs rust/sf-oracle/data/sf.sfc and symbols.txt", file=sys.stderr)
        return 1

    sc = load_shape_compiler()
    rom = ROM_PATH.read_bytes()
    symbols = load_symbols()
    lengths = discover_table_lengths(sc)

    missing = sorted(name for name in lengths if name not in symbols)
    if missing:
        raise RuntimeError(f"missing colour-table symbols: {', '.join(missing)}")

    # Preserve the numeric contracts already used by strategy code.
    ordered = ["id_0_c", "id_1_c", "id_2_c", "id_3_c", "id_5_c", "id_5_c", "black_c"]
    ordered += sorted(set(lengths) - set(ordered))
    tables: List[List[int]] = []
    for name in ordered:
        address = symbols[name]
        tables.append([read_u16(rom, address, i) for i in range(lengths[name])])

    # Follow every colour-table COLANIM pointer.  Its low 14 bits are the
    # address within bank $03; the record is [u8 frame_count][u16 frames...].
    # Some original records deliberately declare a larger power-of-two mask
    # than the number of authored words before the next label (the game only
    # selects their valid prefix).  Preserve all bytes exactly, but do not
    # mistake those spill words for roots of additional animation records.
    animations: Dict[int, List[int]] = {}

    def add_animation(pointer: int) -> None:
        if pointer in animations:
            return
        address = 0x038000 | pointer
        off = lorom_offset(address)
        count = rom[off]
        if count == 0 or count > 64 or count & (count - 1):
            raise RuntimeError(
                f"invalid animation record ${address:06x}: frame count {count}"
            )
        frames = [rom[off + 1 + i * 2] | (rom[off + 2 + i * 2] << 8) for i in range(count)]
        animations[pointer] = frames
    for table in tables:
        for word in table:
            if word & 0xC000 == 0x8000:
                add_animation(word & 0x3FFF)

    # Exact texture descriptor table: [address16, bank8] until texturexytab.
    tex_addr = symbols["textureaddrtab"]
    tex_end = symbols["texturexytab"]
    tex_count = (tex_end - tex_addr) // 3
    texture_sprites = []
    tex_off = lorom_offset(tex_addr)
    for i in range(tex_count):
        address = rom[tex_off + i * 3] | (rom[tex_off + i * 3 + 1] << 8)
        bank = rom[tex_off + i * 3 + 2]
        if bank == 0x12:
            texture_sprites.append((0, address - 0x8000))
        elif bank == 0x13:
            texture_sprites.append((1, address - 0x8000))
        else:
            # msprites3 is used by 2D demo/title records, not polygon faces.
            texture_sprites.append((0xFF, 0))

    out: List[str] = []
    out += [
        "// Auto-generated by tools/color_compiler.py -- do not edit",
        "//! Byte-exact SF1 shape colour tables, animations, and texture descriptors.",
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct ColorTable {",
        "    pub name: &'static str,",
        "    pub entries: &'static [u16],",
        "}",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct TextureSprite {",
        "    pub bank: u8,",
        "    pub offset: u16,",
        "}",
        "",
    ]

    for i, (name, words) in enumerate(zip(ordered, tables)):
        ident = rust_ident(name)
        vals = ", ".join(f"0x{w:04X}" for w in words)
        out.append("#[rustfmt::skip]")
        out.append(f"pub static TABLE_{ident}_{i}: [u16; {len(words)}] = [{vals}];")
    out.append("")
    out.append(f"pub const COLOR_TABLE_COUNT: usize = {len(ordered)};")
    out.append("#[rustfmt::skip]")
    out.append("pub static COLOR_TABLES: [ColorTable; COLOR_TABLE_COUNT] = [")
    for i, name in enumerate(ordered):
        out.append(
            f'    ColorTable {{ name: "{name}", entries: &TABLE_{rust_ident(name)}_{i} }},'
        )
    out.append("];")
    out.append("")
    out.append("pub fn table_id_by_name(name: &str) -> Option<u16> {")
    out.append("    match name.to_ascii_lowercase().as_str() {")
    out.append('        "id_4_c" => Some(4),')
    # Emit one arm per unique name; duplicate id_5_c deliberately maps to 5.
    for name in sorted(set(ordered)):
        idx = 5 if name == "id_5_c" else ordered.index(name)
        out.append(f'        "{name}" => Some({idx}),')
    out += ["        _ => None,", "    }", "}", ""]

    for pointer, frames in sorted(animations.items()):
        vals = ", ".join(f"0x{w:04X}" for w in frames)
        out.append("#[rustfmt::skip]")
        out.append(f"static ANIM_{pointer:04X}: [u16; {len(frames)}] = [{vals}];")
    out.append("")
    out.append("pub fn animation_frames(pointer: u16) -> Option<&'static [u16]> {")
    out.append("    match pointer {")
    for pointer in sorted(animations):
        out.append(f"        0x{pointer:04X} => Some(&ANIM_{pointer:04X}),")
    out += ["        _ => None,", "    }", "}", ""]

    for name in ["ca_0", "ca_1", "ca_2", "ca_3", "ca_4", "ca_5", "bullet_a1"]:
        out.append(f"pub const ANIM_PTR_{rust_ident(name)}: u16 = 0x{symbols[name] & 0x3FFF:04X};")
    out.append("")
    vals = ",\n    ".join(
        f"TextureSprite {{ bank: {bank}, offset: 0x{offset:04X} }}"
        for bank, offset in texture_sprites
    )
    out.append("#[rustfmt::skip]")
    out.append(f"pub static TEXTURE_SPRITES: [TextureSprite; {len(texture_sprites)}] = [\n    {vals}\n];")
    out.append("")

    OUTPUT_PATH.write_text("\n".join(out))
    print(
        f"wrote {OUTPUT_PATH} ({len(ordered)} tables, {len(animations)} animations, "
        f"{len(texture_sprites)} texture descriptors)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
