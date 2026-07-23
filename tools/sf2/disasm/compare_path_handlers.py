#!/usr/bin/env python3
"""Rank retail SF1 handler bodies against reachable retail SF2 handlers.

This is a candidate generator, not semantic proof.  It decodes only each
handler's local body (calls and terminal jumps remain opaque), normalizes
relocated memory/control operands, and compares the ordered instruction
shapes.  Any proposed identity must still be reviewed with
``dump_path_handler.py`` before entering ``path_semantics.py``.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Callable

import cpu65816 as cpu
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor
from path_semantics import PATH_SEMANTIC_BY_OPCODE


REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_SF1_ROM = REPO_ROOT / "reference" / "ultrastarfox" / "SF.SFC"
SF1_TABLE_FILE = 0x020079
# The assembled table contains opcodes $00..$A6.  P_SCORE ($A7) exists in
# source constants but is not emitted by this retail SF1 table.
SF1_HANDLER_COUNT = 167


@dataclass(frozen=True)
class BodyInstruction:
    address: int
    instruction: cpu.Insn


def sf1_handler_address(rom: bytes, opcode: int) -> int:
    entry = SF1_TABLE_FILE + opcode * 4
    stored = rom[entry] | rom[entry + 1] << 8
    bank = rom[entry + 3]
    return bank << 16 | ((stored + 1) & 0xFFFF)


def sf1_file(address: int) -> int:
    bank = address >> 16 & 0x7F
    low = address & 0xFFFF
    if low < 0x8000:
        raise ValueError(f"SF1 code address outside LoROM: ${address:06X}")
    return bank * 0x8000 + low - 0x8000


def decode_body(
    entry: int,
    decode: Callable[[int, int, int], cpu.Insn],
    control_target: Callable[[cpu.Insn], int | None],
) -> tuple[BodyInstruction, ...]:
    work = [(entry, 1, 0, ())]
    seen: set[tuple[int, int, int, tuple[tuple[int, int], ...]]] = set()
    decoded: dict[tuple[int, int, int], BodyInstruction] = {}

    while work:
        pc, m_flag, x_flag, status_stack = work.pop()
        while True:
            state = (pc, m_flag, x_flag, status_stack)
            if state in seen:
                break
            seen.add(state)
            if len(seen) > 1024:
                raise RuntimeError(f"local handler body escaped at ${entry:06X}")

            instruction = decode(pc, m_flag, x_flag)
            decoded[(pc, m_flag, x_flag)] = BodyInstruction(pc, instruction)
            next_pc = pc + instruction.length
            next_m, next_x = cpu.update_flags(instruction, m_flag, x_flag)
            next_status = status_stack
            if instruction.mnem == "PHP":
                next_status = status_stack + ((m_flag, x_flag),)
            elif instruction.mnem == "PLP":
                if not status_stack:
                    break
                next_m, next_x = status_stack[-1]
                next_status = status_stack[:-1]

            target = control_target(instruction)
            if instruction.mnem in cpu.BRANCHES:
                if instruction.mnem in ("BRA", "BRL"):
                    if target is not None:
                        pc = target
                        m_flag, x_flag, status_stack = next_m, next_x, next_status
                        continue
                    break
                if target is not None:
                    work.append((target, next_m, next_x, next_status))
                pc = next_pc
                m_flag, x_flag, status_stack = next_m, next_x, next_status
                continue

            # Calls are deliberately opaque.  Terminal jumps delimit the
            # local handler body even when they enter shared continuation code.
            if instruction.mnem in ("JMP", "JML") or instruction.mnem in cpu.RETURNS:
                break
            if instruction.mnem in ("BRK", "COP", "STP", "WAI"):
                break
            pc = next_pc
            m_flag, x_flag, status_stack = next_m, next_x, next_status

    return tuple(decoded[key] for key in sorted(decoded))


def normalized(body: tuple[BodyInstruction, ...]) -> tuple[str, ...]:
    tokens = []
    for item in body:
        instruction = item.instruction
        token = f"{instruction.mnem}:{instruction.mode}:{instruction.m}:{instruction.x}"
        if instruction.mode in (cpu.IMM8, cpu.IMM_M, cpu.IMM_X):
            token += f":{instruction.operand:X}"
        tokens.append(token)
    return tuple(tokens)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sf1-rom", type=Path, default=DEFAULT_SF1_ROM)
    parser.add_argument("--sf2-rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--top", type=int, default=3)
    parser.add_argument("opcodes", nargs="*", type=lambda text: int(text, 16))
    args = parser.parse_args()

    sf1_rom = args.sf1_rom.read_bytes()
    sf2 = PathExtractor(args.sf2_rom.read_bytes())
    extraction = sf2.extract()

    sf1_bodies = {}
    for opcode in range(SF1_HANDLER_COUNT):
        entry = sf1_handler_address(sf1_rom, opcode)
        body = decode_body(
            entry,
            lambda address, m, x: cpu.decode_one(sf1_rom, sf1_file(address), m, x),
            lambda instruction: instruction.target,
        )
        sf1_bodies[opcode] = normalized(body)

    requested = set(args.opcodes)
    for opcode, handler in extraction.handlers.items():
        if requested and opcode not in requested:
            continue
        if not requested and opcode in PATH_SEMANTIC_BY_OPCODE:
            continue
        body = decode_body(handler.handler_address, sf2._decode_runtime, sf2._control_target)
        signature = normalized(body)
        ranked = sorted(
            (
                SequenceMatcher(None, signature, candidate).ratio(),
                sf1_opcode,
                len(candidate),
            )
            for sf1_opcode, candidate in sf1_bodies.items()
        )
        best = reversed(ranked[-args.top :])
        matches = " ".join(
            f"SF1:${sf1_opcode:02X} score={score:.3f} n={length}"
            for score, sf1_opcode, length in best
        )
        print(
            f"SF2:${opcode:03X} handler=${handler.handler_address:06X} "
            f"n={len(signature)} {matches}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
