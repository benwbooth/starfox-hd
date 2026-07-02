#!/usr/bin/env python3
"""Locate the SF2 ISTRAT strategy-pointer table.

SF1's ISTRATS.ASM emits, per strategy, a 4-byte record:
    dw  strat_routine & 0xFFFF     ; 16-bit code address (>= $8000)
    db  strat_routine >> 16        ; bank byte
    db  shape_index                ; default shape id (< number of shapes)
i.e. a stride-4 array [addr_lo][addr_hi][bank][shape]. We scan the whole ROM for
the longest run of consecutive records that satisfy: addr in $8000..$FFFF (a
LoROM code pointer), bank in a plausible code-bank range, shape < 0xF8. The
longest such run is the ISTRAT table; its length ~= the strategy count.
"""
from __future__ import annotations
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from disasm_host import load

CODE_BANKS = set(range(0x00, 0x20))   # $00-$1F plausible for code/data pointers


def plausible(rom, o):
    addr = rom[o] | (rom[o+1] << 8)
    bank = rom[o+2]
    shape = rom[o+3]
    return addr >= 0x8000 and bank in CODE_BANKS and shape < 0xF8


def run_len(rom, o):
    n = 0
    while o + 4 <= len(rom) and plausible(rom, o):
        n += 1
        o += 4
    return n


def main():
    rom = load()
    best = []
    o = 0
    N = len(rom)
    while o + 4 <= N:
        if plausible(rom, o):
            n = run_len(rom, o)
            if n >= 16:
                best.append((o, n))
                o += n * 4
            else:
                o += 4
        else:
            o += 1
    best.sort(key=lambda t: -t[1])
    print("; longest stride-4 [addr16>=8000][bank][shape] runs (ISTRAT candidates):")
    for o, n in best[:20]:
        cpu = (o >> 15 << 16) | (0x8000 + (o & 0x7FFF))
        print(f"  file {o:06X} ({cpu>>16:02X}:{cpu&0xFFFF:04X})  len={n} records")
        # sample first 6 records
        for i in range(min(6, n)):
            p = o + i*4
            a = rom[p] | (rom[p+1] << 8); b = rom[p+2]; s = rom[p+3]
            print(f"      #{i}: -> {b:02X}:{a:04X}  shape={s}")
    return best


if __name__ == "__main__":
    main()
