#!/usr/bin/env python3
"""Locate indexed-dispatch sites in the SF2 host code.

SF1's map-VM (newobjex/map_exec), path-VM, and game-state machines all dispatch
by loading an opcode/index, doubling it, transferring to X, and jumping through
a pointer table:  JMP ($xxxx,X)  [opcode 0x7C]  or  JSR ($xxxx,X) [0xFC].
Scanning for those two opcodes and disassembling a short window before each
site surfaces every jump-table dispatcher in the host banks -- the structural
signature the map/path VMs share. We then read the pointed-to table and count
plausible in-bank entries to size each VM.
"""
from __future__ import annotations
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cpu65816 as c
from disasm_host import load, linear

HOST_BANKS = range(0x00, 0x08)   # banks 0..7 -> file 0 .. 0x40000


def find_indexed_jumps(rom):
    """Return list of (file_off, opcode_byte) for 7C/FC sites in host banks.

    Filter to the real dispatch idiom: the indexed jump is immediately preceded
    by TAX (0xAA) -- the index-load that every jump-table dispatcher uses. This
    removes data/misalignment false positives that a blind 7C/FC scan produces.
    """
    hits = []
    end = 0x08 * 0x8000
    for off in range(1, end):
        b = rom[off]
        if b in (0x7C, 0xFC) and rom[off-1] == 0xAA:
            hits.append((off, b))
    return hits


def table_stats(rom, table_cpu):
    """Read a 16-bit pointer table at CPU addr; count consecutive entries whose
    high byte points into a plausible code bank ($80-$FF meaning $8000+ offset).
    Return (count, first_entries)."""
    tf = c.cpu_to_file(table_cpu)
    if tf is None:
        return 0, []
    entries = []
    count = 0
    for i in range(0, 512):
        o = tf + i * 2
        if o + 1 >= len(rom):
            break
        val = rom[o] | (rom[o+1] << 8)
        # in-bank code pointer: >= 0x8000
        if val >= 0x8000:
            count += 1
            if i < 24:
                entries.append(val)
        else:
            break
    return count, entries


def main():
    rom = load()
    hits = find_indexed_jumps(rom)
    print(f"; {len(hits)} indexed-jump sites (7C/FC) in host banks 0-7\n")
    for off, opb in hits:
        cpu = c.file_to_cpu(off)
        ins = c.decode_one(rom, off, 1, 1)
        table_cpu = (cpu & 0xFF0000) | ins.operand
        cnt, ents = table_stats(rom, table_cpu)
        kind = "JMP" if opb == 0x7C else "JSR"
        print(f"=== {kind} (${ins.operand:04X},X) at {cpu>>16:02X}:{cpu&0xFFFF:04X} "
              f"(file {off:06X}); table->{table_cpu&0xFFFF:04X} in-bank entries~{cnt}")
        # context: disassemble the 12 instructions preceding, linearly aligned by
        # scanning back a small window and re-decoding forward to the site.
        start = max(0, off - 24)
        print(linear(rom, start, 16))
        if ents:
            print("   first table entries:", " ".join(f"{e:04X}" for e in ents[:16]))
        print()


if __name__ == "__main__":
    main()
