#!/usr/bin/env python3
"""Probe the SF2 3D banks (0x12-0x17) for the SF1 point-block / face grammar.

SF1 shape point-block (from tools/shape_compiler.py + reference SHAPES):
    0x04 <n> <n * (sx,sy,sz signed bytes)> 0x0C     (byte-coordinate block)
    0x08 <n> <n * (sx,sy,sz signed words)> 0x0C     (word-coordinate block)
SF1 face record list follows: [N][col][vis][nx][ny][nz][idx..], FE/FF terminators.

This scans each 3D bank for *well-formed* point-blocks: a 0x04/0x08 lead byte, a
plausible vertex count, followed exactly by count*3 (or count*6) coordinate bytes
and a 0x0C terminator. A high count of self-consistent blocks confirms the grammar
transfers; the distribution of counts/strides characterises SF2's real encoding.
"""
from __future__ import annotations
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disasm_host import load

BANKS = {0x12:(0x090000,0x098000),0x13:(0x098000,0x0A0000),0x14:(0x0A0000,0x0A8000),
         0x15:(0x0A8000,0x0B0000),0x16:(0x0B0000,0x0B8000),0x17:(0x0B8000,0x0C0000)}


def scan_blocks(rom, lo, hi, lead, coordsize):
    """Count well-formed [lead][n][n*3*coordsize bytes][0C] blocks."""
    hits = []
    o = lo
    while o < hi - 2:
        if rom[o] == lead:
            n = rom[o+1]
            if 1 <= n <= 64:
                end = o + 2 + n*3*coordsize
                if end < hi and rom[end] == 0x0C:
                    hits.append((o, n))
        o += 1
    return hits


def main():
    rom = load()
    print("; SF2 3D-bank point-block probe (SF1 grammar: 04/08 <n> ... 0C)\n")
    grand = {}
    for bank,(lo,hi) in BANKS.items():
        b04 = scan_blocks(rom, lo, hi, 0x04, 1)   # byte coords
        b08 = scan_blocks(rom, lo, hi, 0x08, 2)   # word coords
        print(f"bank {bank:02X}: byte-blocks(04..0C)={len(b04):4d}  word-blocks(08..0C)={len(b08):4d}")
        grand[bank] = (b04, b08)
    print()
    # characterise: for bank 12 show first blocks and vertex-count histogram
    b04, b08 = grand[0x12]
    from collections import Counter
    hist = Counter(n for _,n in b04)
    print("bank 12 byte-block vertex-count histogram (n:count):",
          " ".join(f"{n}:{c}" for n,c in sorted(hist.items())[:20]))
    print("\nbank 12 first 6 byte-blocks (offset, n, coords):")
    for o,n in b04[:6]:
        coords = []
        for i in range(min(n,6)):
            p = o+2+i*3
            sx = rom[p]-256 if rom[p]>=128 else rom[p]
            sy = rom[p+1]-256 if rom[p+1]>=128 else rom[p+1]
            sz = rom[p+2]-256 if rom[p+2]>=128 else rom[p+2]
            coords.append(f"({sx},{sy},{sz})")
        print(f"  {o:06X} n={n}: " + " ".join(coords) + (" ..." if n>6 else ""))
    return grand


if __name__ == "__main__":
    main()
