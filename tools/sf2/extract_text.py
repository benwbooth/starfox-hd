#!/usr/bin/env python3
"""SF2 text-table extractor (SF2_RECON.md phase 1, task 4).

Extracts the located, factual name/label tables (not creative prose):
  HUD/title labels   @0x3083   ("NINTENDO", "PRESENTS", "YOU LOST", ...)
  staff-roll credits @0x38BA   (control-byte-prefixed scroll records)
  boss display names @0x187D6  ("MOTH GLIDER", "HAL BIRD", "ANDORF", ...)
  character/rivals   @0x18941  ("ALGY", "PIGMA", "LEON", "WOLF", ...)

Strings are uppercase ASCII, NUL-terminated, some preceded by control bytes
(centering/color/row selectors -- the SF2 analogue of SF1's MSG/ tables).
Each table is read until it runs into non-text ROM (a run of control/high
bytes), so trailing code/garbage is not captured.

Emits rust/sf2-data/src/text.rs.
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom

PRINTABLE = set(range(0x20, 0x7F))


def read_table(d: bytes, start: int, limit: int):
    """Read NUL-terminated strings until the table ends.

    Returns list of (control_bytes, text).  Stops when a leading control-byte
    run is too long to be a formatting prefix (>3) or an empty-with-no-text
    record repeats -- i.e. we've walked off the table into code/graphics.
    """
    out = []
    i = start
    while i < len(d) and len(out) < limit:
        ctrl = []
        while i < len(d) and d[i] not in PRINTABLE and d[i] != 0:
            ctrl.append(d[i])
            i += 1
            if len(ctrl) > 3:
                break
        if len(ctrl) > 3:
            break  # leading control run too long: off the end of the table
        s = bytearray()
        while i < len(d) and d[i] in PRINTABLE:
            s.append(d[i])
            i += 1
        # A record with no control prefix and no text is a dead zone (padding /
        # off the end of the table), not a legitimate blank spacer.
        if not ctrl and not s:
            break
        # consume the NUL terminator(s)
        while i < len(d) and d[i] == 0:
            i += 1
        out.append((ctrl, s.decode("ascii")))
    return out


# (rust_const, rom_off, max_entries) -- max_entries caps at the visible list end.
TABLES = [
    ("HUD_LABELS", 0x003083, 20),
    ("CREDITS", 0x0038BA, 220),
    ("BOSS_NAMES", 0x0187D6, 24),
    ("RIVAL_NAMES", 0x018941, 16),
]


def esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def extract(d: bytes):
    tables = []
    for name, off, cap in TABLES:
        entries = read_table(d, off, cap)
        tables.append((name, off, entries))
    emit_rust(tables)
    return tables


def emit_rust(tables):
    L = []
    L.append(AUTOGEN_HEADER.format(tool="extract_text.py"))
    L.append("//! SF2 text/label tables (factual name & UI-label data).")
    L.append("//!")
    L.append("//! Uppercase-ASCII, NUL-terminated strings; the SF2 analogue of")
    L.append("//! SF1's MSG/ message tables. `control` holds the per-string")
    L.append("//! formatting prefix bytes (centering / color / row).")
    L.append("")
    L.append("#[derive(Debug, Clone, Copy)]")
    L.append("pub struct TextEntry {")
    L.append("    pub control: &'static [u8],")
    L.append("    pub text: &'static str,")
    L.append("}")
    L.append("")
    L.append("#[derive(Debug, Clone, Copy)]")
    L.append("pub struct TextTable {")
    L.append("    pub name: &'static str,")
    L.append("    pub rom_off: u32,")
    L.append("    pub entries: &'static [TextEntry],")
    L.append("}")
    L.append("")
    for name, off, entries in tables:
        L.append(f"pub static {name}: [TextEntry; {len(entries)}] = [")
        for ctrl, text in entries:
            cbytes = ", ".join(f"0x{c:02X}" for c in ctrl)
            L.append(f'    TextEntry {{ control: &[{cbytes}], text: "{esc(text)}" }},')
        L.append("];")
        L.append("")
    L.append(f"pub const TEXT_TABLE_COUNT: usize = {len(tables)};")
    L.append("pub static TEXT_TABLES: [TextTable; TEXT_TABLE_COUNT] = [")
    for name, off, entries in tables:
        L.append(
            f'    TextTable {{ name: "{name}", rom_off: 0x{off:06X}, '
            f"entries: &{name} }},")
    L.append("];")
    L.append("")
    with open(os.path.join(RUST_SRC, "text.rs"), "w") as f:
        f.write("\n".join(L))
    counts = ", ".join(f"{n}={len(e)}" for n, _o, e in tables)
    print(f"  text.rs: {counts}")


if __name__ == "__main__":
    extract(load_rom())
