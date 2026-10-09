#!/usr/bin/env python3
"""Extract Star Fox 2's radio and dialog messages and the font that lays
them out.

Messages are numbered from 1; `$00:AEB3` holds each record's bank-00
address. A record is a speaker byte, a style byte and the text, ended by a
zero; the text is laid out by the GSU text engine with the font at
`$0D:E1FB` (a header of line height, first and last glyph and spacing, the
glyph index table, then two bytes per glyph whose second carries the
width in its top five bits).

Emits rust/sf2-data/src/messages.rs.
"""

from __future__ import annotations

import os

from rom import AUTOGEN_HEADER, RUST_SRC, load_rom

MESSAGE_TABLE = 0xAEB3
MESSAGE_COUNT = 216
FONT_START = 0xE1FB
FONT_END = 0xE472


def file_offset(bank: int, address: int) -> int:
    return bank * 0x8000 + (address & 0x7FFF)


def emit_bytes(name: str, data: bytes) -> list[str]:
    lines = [f"pub static {name}: [u8; 0x{len(data):04X}] = [\n"]
    for i in range(0, len(data), 16):
        lines.append("    " + " ".join(f"0x{b:02X}," for b in data[i : i + 16]) + "\n")
    lines.append("];\n")
    return lines


def extract(d: bytes) -> None:
    pointers = []
    for i in range(MESSAGE_COUNT):
        o = file_offset(0x00, MESSAGE_TABLE + 2 * i)
        pointers.append(d[o] | d[o + 1] << 8)
    start = min(pointers)
    assert start >= 0x8000 and max(pointers) < MESSAGE_TABLE
    text = d[file_offset(0x00, start) : file_offset(0x00, MESSAGE_TABLE)]
    font = d[file_offset(0x0D, FONT_START) : file_offset(0x0D, FONT_END)]
    out = [AUTOGEN_HEADER.format(tool="extract_messages.py")]
    out.append("//! Radio and dialog messages (`$00:AEB3` records) and their font (`$0D:E1FB`).\n\n")
    out.append(f"/// The first message record's bank-00 address.\npub const MESSAGE_BASE: u16 = 0x{start:04X};\n")
    out.append(f"/// Bank-00 addresses of messages 1..={MESSAGE_COUNT}.\n")
    out.append(f"pub static MESSAGE_POINTERS: [u16; {MESSAGE_COUNT}] = [\n")
    for i in range(0, MESSAGE_COUNT, 12):
        out.append("    " + " ".join(f"0x{p:04X}," for p in pointers[i : i + 12]) + "\n")
    out.append("];\n")
    out.append(f"/// `$00:{start:04X}..$00:{MESSAGE_TABLE:04X}`.\n")
    out.extend(emit_bytes("MESSAGES", text))
    out.append(f"/// The font's bank-0D address.\npub const FONT_BASE: u16 = 0x{FONT_START:04X};\n")
    out.append(f"/// `$0D:{FONT_START:04X}..$0D:{FONT_END:04X}`.\n")
    out.extend(emit_bytes("FONT", font))
    path = os.path.join(RUST_SRC, "messages.rs")
    with open(path, "w") as f:
        f.write("".join(out))
    print(f"  messages: {MESSAGE_COUNT} records, {len(text)} text bytes, {len(font)} font bytes -> {path}")


if __name__ == "__main__":
    extract(load_rom())
