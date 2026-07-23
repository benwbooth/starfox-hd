#!/usr/bin/env python3
"""SF2 color/material table extractor (SF2_RECON.md phase 1, task 3).

Extracts the bank-01 material-word region (0x806C..0x86F1) documented in the
recon.  The 16-bit material words use the same encoding sf-render's color
resolver consumes (rust/sf-render/src/shapes.rs Shapes_ResolveMaterialColor):

  COLANIM   bit15 set              (bit14 too => COLSMOOTH)
  COLTEXT   bit14 set
  COLNORM   high byte 0x3F (63)    two palette nibbles
  COLDEPTH  high byte 0x3E (62)    night depth-bank index
  COLLITE   high byte < 12         light source + normal color byte

The 0x8000..0x806C prefix is a separate 4-byte-record pointer table (27
entries), NOT material words -- it is skipped.  The recon's "196-word master
table at 0x806C" is the first sub-table; the full 0x806C..0x86F1 span holds
several concatenated color tables, extracted here as one flat word array.

Emits rust/sf2-data/src/colors.rs.
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom, u16

TABLE_START = 0x806C
TABLE_END = 0x86F1
MASTER_LEN = 196  # recon: master sub-table word count at 0x806C


def classify(w: int) -> str:
    if w & 0x8000:
        return "COLSMOOTH" if (w & 0x4000) else "COLANIM"
    if w & 0x4000:
        return "COLTEXT"
    src = w >> 8
    if src == 63:
        return "COLNORM"
    if src == 62:
        return "COLDEPTH"
    if src < 12:
        return "COLLITE"
    return "OTHER"


def extract(d: bytes):
    raw = d[TABLE_START:TABLE_END]
    n = (TABLE_END - TABLE_START) // 2
    words = [u16(d, TABLE_START + i * 2) for i in range(n)]
    counts = {}
    for w in words:
        counts[classify(w)] = counts.get(classify(w), 0) + 1
    emit_rust(raw, words, counts)
    return words, counts


def emit_rust(raw, words, counts):
    L = []
    L.append(AUTOGEN_HEADER.format(tool="extract_colors.py"))
    L.append("//! SF2 color/material word tables (bank 01, 0x806C..0x86F1).")
    L.append("//!")
    L.append("//! Words use the SF1 material encoding decoded by")
    L.append("//! `sf_render::shapes::resolve_material_color`; the class helpers")
    L.append("//! below mirror the `MATERIAL_SOURCE_*` discriminators there.")
    ct = ", ".join(f"{k}={v}" for k, v in sorted(counts.items()))
    L.append(f"//! Class histogram: {ct}.")
    L.append("")
    L.append("/// Material-word class (mirror of sf-render's source discriminators).")
    L.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    L.append("pub enum MaterialClass {")
    L.append("    ColLite,")
    L.append("    ColDepth,")
    L.append("    ColNorm,")
    L.append("    ColAnim,")
    L.append("    ColText,")
    L.append("    ColSmooth,")
    L.append("    Other,")
    L.append("}")
    L.append("")
    L.append("/// Classify a material word (mirror of `extract_colors.classify`).")
    L.append("pub const fn classify(w: u16) -> MaterialClass {")
    L.append("    if w & 0x8000 != 0 {")
    L.append("        return if w & 0x4000 != 0 { MaterialClass::ColSmooth } else { MaterialClass::ColAnim };")
    L.append("    }")
    L.append("    if w & 0x4000 != 0 {")
    L.append("        return MaterialClass::ColText;")
    L.append("    }")
    L.append("    match w >> 8 {")
    L.append("        63 => MaterialClass::ColNorm,")
    L.append("        62 => MaterialClass::ColDepth,")
    L.append("        s if s < 12 => MaterialClass::ColLite,")
    L.append("        _ => MaterialClass::Other,")
    L.append("    }")
    L.append("}")
    L.append("")
    L.append(f"pub const COLOR_TABLE_ROM_OFF: u32 = 0x{TABLE_START:04X};")
    L.append(f"pub const COLOR_TABLE_ROM_END: u32 = 0x{TABLE_END:04X};")
    L.append(f"pub const MASTER_TABLE_LEN: usize = {MASTER_LEN};")
    L.append(f"pub const MATERIAL_WORD_COUNT: usize = {len(words)};")
    L.append("")
    L.append("/// Byte-exact bank-$01 color data. A number of retail tables begin")
    L.append("/// at odd addresses, so this is the authoritative lookup source.")
    L.append("#[rustfmt::skip]")
    L.append(f"pub static COLOR_DATA: [u8; {len(raw)}] = [")
    for i in range(0, len(raw), 16):
        row = ", ".join(f"0x{b:02X}" for b in raw[i:i + 16])
        L.append(f"    {row},")
    L.append("];")
    L.append("")
    L.append("/// Flat material-word array, 0x806C..0x86F1 (several concatenated")
    L.append("/// color tables; the first MASTER_TABLE_LEN words are the master).")
    L.append("#[rustfmt::skip]")
    L.append(f"pub static MATERIAL_WORDS: [u16; MATERIAL_WORD_COUNT] = [")
    for i in range(0, len(words), 12):
        row = ", ".join(f"0x{w:04X}" for w in words[i:i + 12])
        L.append(f"    {row},")
    L.append("];")
    L.append("")
    L.append("/// Read an unaligned little-endian material word by bank-$01 address.")
    L.append("pub fn word_at(address: u16) -> Option<u16> {")
    L.append("    let offset = usize::from(address.checked_sub(COLOR_TABLE_ROM_OFF as u16)?);")
    L.append("    let bytes = COLOR_DATA.get(offset..offset + 2)?;")
    L.append("    Some(u16::from_le_bytes([bytes[0], bytes[1]]))")
    L.append("}")
    L.append("")
    L.append("/// Resolve a face index through its exact ShapeHdr color-table pointer.")
    L.append("pub fn material_at(table_address: u16, face_color_index: u8) -> Option<u16> {")
    L.append("    word_at(table_address.checked_add(u16::from(face_color_index) * 2)?)")
    L.append("}")
    L.append("")
    L.append("/// Follow SF2's bank-$01 COLANIM records for the object's color frame.")
    L.append("pub fn resolve_animated_material(mut material: u16, col_frame: u8) -> Option<u16> {")
    L.append("    for _ in 0..4 {")
    L.append("        if material & 0xC000 != 0x8000 {")
    L.append("            return Some(material);")
    L.append("        }")
    L.append("        let record = 0x8000 | (material & 0x3FFF);")
    L.append("        let offset = usize::from(record.checked_sub(COLOR_TABLE_ROM_OFF as u16)?);")
    L.append("        let &count = COLOR_DATA.get(offset)?;")
    L.append("        if count == 0 || count > 64 || !count.is_power_of_two() {")
    L.append("            return None;")
    L.append("        }")
    L.append("        let frame = usize::from(col_frame & (count - 1));")
    L.append("        let bytes = COLOR_DATA.get(offset + 1 + frame * 2..offset + 3 + frame * 2)?;")
    L.append("        material = u16::from_le_bytes([bytes[0], bytes[1]]);")
    L.append("    }")
    L.append("    None")
    L.append("}")
    L.append("")
    with open(os.path.join(RUST_SRC, "colors.rs"), "w") as f:
        f.write("\n".join(L))
    print(f"  colors.rs: {len(words)} material words, {ct}")


if __name__ == "__main__":
    extract(load_rom())
