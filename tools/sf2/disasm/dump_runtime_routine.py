#!/usr/bin/env python3
"""Linearly disassemble an SF2 routine copied into WRAM bank $7F.

The early runtime block is copied from LoROM file `$010000 + address`; the
path VM block `$7F:7E00..CBFF` is copied from file `$050000..54DFF`.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import cpu65816 as cpu
from extract_map import DEFAULT_ROM


def source_offset(address: int) -> int:
    if 0x7F7E00 <= address < 0x7FCC00:
        return 0x050000 + address - 0x7F7E00
    if address >> 16 == 0x7F and (address & 0xFFFF) < 0x7E00:
        return 0x010000 + (address & 0xFFFF)
    bank = address >> 16
    offset = address & 0xFFFF
    if bank < 0x7E and offset >= 0x8000:
        return (bank & 0x7F) * 0x8000 + (offset & 0x7FFF)
    raise ValueError(f"unsupported runtime/LoROM address ${address:06X}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("address", type=lambda value: int(value, 16))
    parser.add_argument("--count", type=int, default=80)
    parser.add_argument("--m", type=int, choices=(0, 1), default=1)
    parser.add_argument("--x", type=int, choices=(0, 1), default=0)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    args = parser.parse_args()

    rom = args.rom.read_bytes()
    address = args.address
    m_flag = args.m
    x_flag = args.x
    for _ in range(args.count):
        offset = source_offset(address)
        instruction = cpu.decode_one(rom, offset, m_flag, x_flag)
        instruction.cpu = address
        if instruction.mode == cpu.REL:
            relative = instruction.operand
            if relative & 0x80:
                relative -= 0x100
            instruction.target = (
                address & 0xFF0000
                | ((address + instruction.length + relative) & 0xFFFF)
            )
        elif instruction.mode == cpu.RELL:
            relative = instruction.operand
            if relative & 0x8000:
                relative -= 0x10000
            instruction.target = (
                address & 0xFF0000
                | ((address + instruction.length + relative) & 0xFFFF)
            )
        elif instruction.mnem in ("JMP", "JSR") and instruction.mode == cpu.ABS:
            instruction.target = address & 0xFF0000 | instruction.operand
        print(cpu.fmt_insn(instruction))
        m_flag, x_flag = cpu.update_flags(instruction, m_flag, x_flag)
        address += instruction.length
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
