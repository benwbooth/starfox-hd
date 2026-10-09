#!/usr/bin/env python3
"""The Super FX decompressor (`$01:D9FF`), as a pure function.

The CPU starts the job with GSU RAM words 0068 (the stream's end address),
006A (its ROM bank), 002C (the output's base) and 00A2 (a word added to the
output afterwards, when nonzero). The stream is read backwards from its end:
a header (the output length, two skipped bytes, then the first 32-bit bit
buffer), then commands. Bits are taken from the buffer's low end; a buffer
that empties refills from the next four bytes, with a sentinel bit on top.
The output is written backwards from base + length down to base: literal
runs of 8-bit values and copies from `distance` bytes above.
"""
from __future__ import annotations


def rom_byte(rom: bytes, bank: int, address: int) -> int:
    """A Super FX ROM read (`GETB`): banks 00-3F are 32 KiB LoROM halves."""
    if address < 0x8000:
        raise ValueError(f"GETB below $8000 in bank {bank:02X}: {address:04X}")
    return rom[(bank & 0x3F) * 0x8000 + (address - 0x8000)]


class _Bits:
    def __init__(self, rom: bytes, bank: int, end: int):
        self.rom = rom
        self.bank = bank
        self.at = end

    def byte(self) -> int:
        self.at = (self.at - 1) & 0xFFFF
        return rom_byte(self.rom, self.bank, self.at)

    def word(self) -> int:
        low = self.byte()
        return low | (self.byte() << 8)

    def start(self) -> int:
        length = self.word()
        self.byte()
        self.byte()
        self.low = self.word()
        self.high = self.word()
        return length

    def bit(self) -> int:
        carry = self.high & 1
        self.high >>= 1
        out = self.low & 1
        self.low = (self.low >> 1) | (carry << 15)
        if self.high | self.low:
            return out
        # The sentinel left; refill with a new sentinel above the data.
        self.low = self.word()
        self.high = self.word()
        carry = self.high & 1
        self.high = (self.high >> 1) | 0x8000
        out = self.low & 1
        self.low = (self.low >> 1) | (carry << 15)
        return out

    def bits(self, count: int) -> int:
        value = 0
        for _ in range(count):
            value = ((value << 1) | self.bit()) & 0xFFFF
        return value


def decompress(rom: bytes, bank: int, end: int, add: int = 0) -> bytes:
    """Decompress the stream ending at `bank:end`; returns the output bytes
    (the job writes them at its base, in GSU RAM)."""
    bits = _Bits(rom, bank, end)
    length = bits.start()
    out = bytearray(length)
    at = length
    while True:
        run = bits.bits(3)
        if run:
            if run == 7:
                if bits.bit():
                    run = bits.bits(10)
                    if run == 0:
                        run = bits.bits(18)
                else:
                    run = bits.bits(4) + 7
            for _ in range(run):
                at -= 1
                out[at] = bits.bits(8)
        if at == 0:
            break
        # $01:DAE4: a copy.
        kind = bits.bits(2)
        if kind == 0:
            count, width = 2, 8
        else:
            if kind == 2:
                count = 4
            elif kind == 1:
                count = 3
            else:
                extra = bits.bits(2)
                if extra == 3:
                    count = bits.bits(8)
                elif extra == 2:
                    count = bits.bits(2) + 7
                else:
                    count = extra + 5
            if kind == 1:
                width = 8 if bits.bit() else 14
            elif bits.bit():
                width = 8 if bits.bit() else 12
            else:
                width = 16
        distance = bits.bits(width)
        for _ in range(count):
            at -= 1
            out[at] = out[at + distance]
    if add:
        # $01:DAD1 adds the word across the output and past its end; no
        # ported caller asks for it.
        raise ValueError("the decompressor's output adjustment is not ported")
    return bytes(out)
