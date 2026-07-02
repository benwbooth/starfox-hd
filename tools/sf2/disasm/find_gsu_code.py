#!/usr/bin/env python3
"""Heuristically locate GSU (Super FX) microcode regions in the SF2 ROM.

GSU code has a distinctive texture: dense register prefixes (TO $10-1F, WITH
$20-2F, FROM $B0-BF), immediate loads (IBT $A0-AF+1, IWT $F0-FF+2), arithmetic
(ADD/SUB/MULT), CACHE($02) at routine heads, and LOOP($3C)/branch structure,
with STOP($00) terminators -- and *very few* long same-byte runs (unlike graphics
or coordinate data). We slide a window, decode it linearly as GSU, and score by
the fraction of "codey" opcodes minus a penalty for same-byte runs. High-scoring
windows are GSU-code candidates.
"""
from __future__ import annotations
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disasm_host import load
import gsu

PREFIX = set(range(0x10,0x30)) | set(range(0xB0,0xC0))
IMM1 = set(range(0xA0,0xB0)); IMM2 = set(range(0xF0,0x100))
ARITH = set(range(0x50,0xA0)) | set(range(0xC0,0xF0))
STRUCT = {0x02,0x3C,0x3D,0x3E,0x3F} | set(range(0x05,0x10))


def score_window(rom, o, size=256):
    alt = 0
    codey = 0; total = 0; illegal = 0
    end = o + size
    p = o
    while p < end:
        b = rom[p]
        ins, alt = gsu.decode_one(rom, p, alt)
        total += 1
        if ins.text.startswith("DB "):
            illegal += 1
        if b in PREFIX or b in IMM1 or b in IMM2 or b in ARITH or b in STRUCT:
            codey += 1
        p += ins.length
    # same-byte run penalty
    runs = 0; i = o
    while i < end-1:
        if rom[i] == rom[i+1]:
            runs += 1
        i += 1
    if total == 0: return 0.0
    return codey/total - 0.4*(runs/size)


def main():
    rom = load()
    print("; GSU-code density scan (window=256B, step=256B), top regions per bank\n")
    best = []
    for fo in range(0x040000, 0x0C0000, 0x100):
        s = score_window(rom, fo)
        best.append((s, fo))
    best.sort(reverse=True)
    # cluster: report distinct high-score offsets
    print("top 25 GSU-code candidate windows:")
    shown = 0
    for s, fo in best:
        if shown >= 25: break
        print(f"  file {fo:06X} (bank {fo>>15:02X}:{0x8000+(fo&0x7FFF):04X})  score={s:.2f}")
        shown += 1
    return best


if __name__ == "__main__":
    main()
