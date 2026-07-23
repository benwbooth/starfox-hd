#!/usr/bin/env python3
"""Extract reachable Star Fox 2 map-VM records from the retail ROM.

This is a clean-room extractor.  It follows the bank-03 map dispatcher at
03:8FD3 and the script roots installed through ``$192E/$1657`` by the retail
host code.  It also follows the retail phase-gate macro proven by live
``$1657`` traces: ``delay $1388; jump self`` parks the dispatcher on the jump,
and the surrounding stage state machine later resumes at the byte immediately
after that jump.  SF2 map data is unusual: opcode $78 transfers control to inline
65816 code embedded in the script bank.  That code selects its continuation by
returning with a new constant X value, so a raw byte scan cannot distinguish
scripts, code, and data.  The small abstract interpreter below follows both
conditional sides and records the constant-X RTL exits.

The record sizes are not guessed from data.  They are the byte advances made
by the corresponding handlers in the 83-entry table at 03:8FE7.  Branch and
call transitions likewise mirror those handlers.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cpu65816 as cpu


REPO = Path(__file__).resolve().parents[3]
DEFAULT_ROM = REPO / "Star Fox 2 (USA, Europe).sfc"
MAP_HANDLER_TABLE_FILE = 0x18FE7
MAP_HANDLER_COUNT = 83

# Total record sizes, including the opcode byte.  Zero means that the handler
# does not advance to a statically adjacent record (hold, return, reset, or a
# control transfer handled explicitly by MapExtractor._successors).
RECORD_SIZES: dict[int, int] = {
    0: 0, 2: 0, 4: 5, 6: 0, 8: 1, 10: 16, 12: 5, 14: 1,
    16: 3, 18: 3, 20: 2, 22: 1, 24: 1, 26: 1, 28: 2, 30: 1,
    32: 1, 34: 1, 36: 1, 38: 0, 40: 4, 42: 0, 44: 0, 46: 4,
    48: 2, 50: 2, 52: 2, 54: 4, 56: 5, 58: 6, 60: 4, 62: 5,
    64: 6, 66: 1, 68: 1, 70: 6, 72: 6, 74: 4, 76: 1, 78: 1,
    80: 1, 82: 1, 84: 1, 86: 1, 88: 1, 90: 1, 92: 5, 94: 6,
    96: 7, 98: 4, 100: 1, 102: 1, 104: 6, 106: 6, 108: 0,
    110: 0, 112: 7, 114: 7, 116: 0, 118: 7, 120: 0, 122: 4,
    124: 7, 126: 7, 128: 7, 130: 2, 132: 1, 134: 14, 136: 2,
    138: 2, 140: 3, 142: 13, 144: 12, 146: 2, 148: 9, 150: 5,
    152: 5, 154: 3, 156: 1, 158: 4, 160: 12, 162: 3, 164: 3,
}

SPAWN_OPS = frozenset((10, 112, 114, 118, 134))
STATIC_STOPS = frozenset((0, 2, 6, 38, 42, 44, 108, 110, 116))


@dataclass(frozen=True, order=True)
class MapAddress:
    bank: int
    offset: int

    @property
    def cpu(self) -> int:
        return (self.bank << 16) | (0x8000 + self.offset)

    def label(self) -> str:
        return f"{self.bank:02X}:{0x8000 + self.offset:04X}"


@dataclass(frozen=True)
class ScriptRoot:
    address: MapAddress
    installed_at_file: int | None


@dataclass(frozen=True)
class Command:
    address: MapAddress
    opcode: int
    size: int
    raw_hex: str


@dataclass(frozen=True)
class Spawn:
    address: MapAddress
    opcode: int
    delay: int
    x: int
    y: int
    z: int
    shape: int
    strategy_bank: int
    strategy_addr: int
    linked_object: int | None

    @property
    def strategy(self) -> int:
        return (self.strategy_bank << 16) | self.strategy_addr


@dataclass(frozen=True)
class ExternalPhaseGate:
    """One host-released ``delay $1388; jump self`` map phase boundary."""

    hold: MapAddress
    parked: MapAddress
    continuation: MapAddress


@dataclass(frozen=True)
class InlineCall:
    target: int
    accumulator: int | None
    continuation: int


@dataclass(frozen=True)
class InlineWordBits:
    address: int
    mask: int
    set_bits: bool
    continuation: int


@dataclass(frozen=True)
class InlineBranchWordBits:
    address: int
    mask: int
    if_clear: int
    if_set: int


@dataclass(frozen=True)
class InlineSetPilotLinkedFlag:
    continuation: int


@dataclass(frozen=True)
class InlineSelectGsuProgram:
    continuation: int


InlineAction = (
    InlineCall
    | InlineWordBits
    | InlineBranchWordBits
    | InlineSetPilotLinkedFlag
    | InlineSelectGsuProgram
)


@dataclass
class Extraction:
    roots: list[ScriptRoot]
    commands: list[Command]
    spawns: list[Spawn]
    inline_exits: dict[MapAddress, tuple[int, ...]]
    inline_actions: dict[MapAddress, InlineAction]
    unresolved_inline_exits: list[MapAddress]
    invalid_opcodes: list[tuple[MapAddress, int]]
    phase_gates: list[ExternalPhaseGate]


def _s8(value: int) -> int:
    return value if value < 0x80 else value - 0x100


def _s16(value: int) -> int:
    return value if value < 0x8000 else value - 0x10000


class MapExtractor:
    def __init__(self, rom: bytes):
        self.rom = rom

    def file_offset(self, address: MapAddress) -> int:
        return address.bank * 0x8000 + address.offset

    def byte(self, address: MapAddress, delta: int = 0) -> int:
        return self.rom[self.file_offset(address) + delta]

    def word(self, address: MapAddress, delta: int = 0) -> int:
        off = self.file_offset(address) + delta
        return int.from_bytes(self.rom[off:off + 2], "little")

    def discover_roots(self) -> list[ScriptRoot]:
        """Find literal ``LDA #bank; LDX #off; STA/STX`` root installs."""
        roots: dict[MapAddress, int | None] = {}
        marker = bytes.fromhex("8D 2E 19 8E 57 16")
        for off in range(len(self.rom) - 11):
            if self.rom[off] != 0xA9 or self.rom[off + 2] != 0xA2:
                continue
            if self.rom[off + 5:off + 11] != marker:
                continue
            bank = self.rom[off + 1]
            stream = int.from_bytes(self.rom[off + 3:off + 5], "little")
            if bank < 0x20 and stream < 0x8000:
                roots[MapAddress(bank, stream)] = off

        # Bank 05 begins with the initial map pointer/bank tuple loaded by
        # 03:84AF..84C0.  This root is data-driven rather than an immediate
        # $192E/$1657 store and therefore needs its own mechanically checked
        # discovery rule.
        initial_bank = self.rom[0x28002]
        initial_offset = int.from_bytes(self.rom[0x28000:0x28002], "little")
        if initial_bank < 0x20 and initial_offset < 0x8000:
            roots.setdefault(MapAddress(initial_bank, initial_offset), None)

        return [ScriptRoot(address, roots[address]) for address in sorted(roots)]

    def handler_table(self) -> list[int]:
        return [
            int.from_bytes(
                self.rom[MAP_HANDLER_TABLE_FILE + i * 2:
                         MAP_HANDLER_TABLE_FILE + i * 2 + 2],
                "little",
            )
            for i in range(MAP_HANDLER_COUNT)
        ]

    def _decode_inline_exits(self, address: MapAddress) -> tuple[set[int], bool]:
        """Return constant X values observed at RTL exits of inline opcode $78.

        The transfer handler enters the script with 8-bit A and 16-bit index
        registers.  Values are intentionally conservative: any load from
        memory or stack makes that register unknown.  Both sides of every
        conditional branch are followed.
        """
        start = self.file_offset(address)
        bank_start = address.bank * 0x8000
        bank_end = bank_start + 0x8000
        # off, M, X flag, A const, X const, Y const
        work: list[tuple[int, int, int, int | None, int | None, int | None]] = [
            (start, 1, 0, None, None, None)
        ]
        seen: set[tuple[int, int, int, int | None, int | None, int | None]] = set()
        exits: set[int] = set()
        unresolved = False

        while work:
            off, m_flag, x_flag, a_value, x_value, y_value = work.pop()
            while bank_start <= off < bank_end:
                state = (off, m_flag, x_flag, a_value, x_value, y_value)
                if state in seen:
                    break
                seen.add(state)
                if len(seen) > 10_000:
                    return exits, True

                ins = cpu.decode_one(self.rom, off, m_flag, x_flag)
                next_m, next_x_flag = cpu.update_flags(ins, m_flag, x_flag)
                next_a, next_x, next_y = a_value, x_value, y_value

                if ins.mnem == "LDA" and ins.mode == cpu.IMM_M:
                    next_a = ins.operand
                elif ins.mnem in ("LDA", "PLA"):
                    next_a = None

                if ins.mnem == "LDX" and ins.mode == cpu.IMM_X:
                    next_x = ins.operand
                elif ins.mnem in ("LDX", "PLX"):
                    next_x = None
                elif ins.mnem == "TAX":
                    next_x = next_a
                elif ins.mnem == "INX" and next_x is not None:
                    next_x = (next_x + 1) & 0xFFFF
                elif ins.mnem == "DEX" and next_x is not None:
                    next_x = (next_x - 1) & 0xFFFF
                elif ins.mnem == "TYX":
                    next_x = next_y

                if ins.mnem == "LDY" and ins.mode == cpu.IMM_X:
                    next_y = ins.operand
                elif ins.mnem in ("LDY", "PLY"):
                    next_y = None
                elif ins.mnem == "TAY":
                    next_y = next_a
                elif ins.mnem == "TXY":
                    next_y = next_x

                if ins.mnem == "RTL":
                    if next_x is not None and next_x < 0x8000:
                        exits.add(next_x)
                    else:
                        unresolved = True
                    break

                if ins.mnem in cpu.BRANCHES:
                    target = cpu.cpu_to_file(ins.target)
                    if target is not None:
                        work.append((target, next_m, next_x_flag,
                                     next_a, next_x, next_y))
                    if ins.mnem in ("BRA", "BRL"):
                        break

                if ins.mnem in ("JMP", "JML", "RTS", "RTI", "BRK", "STP"):
                    if ins.mnem == "JMP" and ins.mode == cpu.ABS:
                        target = cpu.cpu_to_file(ins.target)
                        if target is not None:
                            work.append((target, next_m, next_x_flag,
                                         next_a, next_x, next_y))
                    break

                off += ins.length
                m_flag, x_flag = next_m, next_x_flag
                a_value, x_value, y_value = next_a, next_x, next_y

        return exits, unresolved

    def _linear_inline_block(
        self, address: MapAddress, rtl_count: int
    ) -> list[cpu.Insn]:
        """Decode the contiguous inline block through all of its RTL exits.

        The recovered blocks are deliberately laid out contiguously. Eight
        pilot-link blocks contain one local JSR/RTS helper in the middle; the
        sequential decode includes that helper and then resumes at the main
        epilogue. Any future layout that does not match a proven action below
        is rejected instead of being silently treated as a host callback.
        """
        off = self.file_offset(address)
        m_flag, x_flag = 1, 0
        instructions: list[cpu.Insn] = []
        seen_rtls = 0
        while seen_rtls < rtl_count:
            ins = cpu.decode_one(self.rom, off, m_flag, x_flag)
            instructions.append(ins)
            m_flag, x_flag = cpu.update_flags(ins, m_flag, x_flag)
            off += ins.length
            if ins.mnem == "RTL":
                seen_rtls += 1
            if len(instructions) > 100:
                raise ValueError(f"inline block too large at {address.label()}")
        return instructions

    def _decode_inline_action(
        self, address: MapAddress, exits: tuple[int, ...]
    ) -> InlineAction:
        instructions = self._linear_inline_block(address, len(exits))
        if instructions[0].mnem != "SEI":
            raise ValueError(f"inline block lacks opcode-$78 SEI at {address.label()}")

        jsls = [ins for ins in instructions if ins.mnem == "JSL"]
        jsrs = [ins for ins in instructions if ins.mnem == "JSR"]
        bit_changes = [ins for ins in instructions if ins.mnem in ("TSB", "TRB")]
        continuations = [
            ins.operand
            for index, ins in enumerate(instructions)
            if ins.mnem == "LDX"
            and ins.mode == cpu.IMM_X
            and index + 1 < len(instructions)
            and instructions[index + 1].mnem == "RTL"
        ]

        if jsls:
            if len(jsls) != 1 or len(exits) != 1 or continuations != [exits[0]]:
                raise ValueError(f"unrecognized inline call at {address.label()}")
            accumulator_loads = [
                ins.operand
                for ins in instructions
                if ins.mnem == "LDA" and ins.mode == cpu.IMM_M
            ]
            if len(accumulator_loads) > 1:
                raise ValueError(f"ambiguous inline accumulator at {address.label()}")
            # Every currently reachable inline call is exactly SEI, optional
            # 8-bit LDA immediate, JSL, LDX continuation, RTL.
            expected_length = 5 if accumulator_loads else 4
            if len(instructions) != expected_length:
                raise ValueError(f"unrecognized inline call body at {address.label()}")
            return InlineCall(
                jsls[0].target,
                accumulator_loads[0] if accumulator_loads else None,
                exits[0],
            )

        if bit_changes:
            if len(bit_changes) != 1 or len(exits) != 1 or continuations != [exits[0]]:
                raise ValueError(f"unrecognized inline bit change at {address.label()}")
            immediates = [
                ins.operand
                for ins in instructions
                if ins.mnem == "LDA" and ins.mode == cpu.IMM_M
            ]
            change = bit_changes[0]
            if len(immediates) != 1 or change.mode != cpu.ABS:
                raise ValueError(f"ambiguous inline bit change at {address.label()}")
            return InlineWordBits(
                0x7E0000 | change.operand,
                immediates[0],
                change.mnem == "TSB",
                exits[0],
            )

        if jsrs:
            # Exact repeated block: set bit $40 in the linked records selected
            # through player pointers $12C3/$12C5, skipping player two when
            # `$1916 == $00C0`.
            operands = {(ins.mnem, ins.mode, ins.operand) for ins in instructions}
            required = {
                ("LDX", cpu.ABS, 0x12C3),
                ("LDX", cpu.ABS, 0x12C5),
                ("LDY", cpu.DPX, 0x2B),
                ("LDA", cpu.ABY, 0x6BEC),
                ("STA", cpu.ABY, 0x6BEC),
                ("LDA", cpu.ABS, 0x1916),
                ("CMP", cpu.IMM_M, 0x00C0),
                ("ORA", cpu.IMM_M, 0x40),
            }
            if (
                len(exits) != 1
                or continuations != [exits[0]]
                or len(jsrs) != 2
                or len(instructions) != 28
                or not required.issubset(operands)
            ):
                raise ValueError(f"unrecognized pilot-link inline block at {address.label()}")
            return InlineSetPilotLinkedFlag(exits[0])

        gsu_writes = [
            ins for ins in instructions
            if ins.mnem == "STA" and ins.mode == cpu.ABL and ins.operand == 0x700050
        ]
        if gsu_writes:
            operands = {(ins.mnem, ins.mode, ins.operand) for ins in instructions}
            required = {
                ("LDA", cpu.DP, 0x5E),
                ("STA", cpu.DP, 0x5E),
                ("STA", cpu.ABL, 0x00303A),
                ("LDA", cpu.ABS, 0x1B9C),
                ("BIT", cpu.IMM_M, 0x0020),
                ("LDA", cpu.IMM_M, 0x8F44),
                ("LDA", cpu.IMM_M, 0x8F48),
            }
            if (
                len(exits) != 1
                or continuations != [exits[0]]
                or len(gsu_writes) != 2
                or len(instructions) != 27
                or not required.issubset(operands)
            ):
                raise ValueError(f"unrecognized GSU-select inline block at {address.label()}")
            return InlineSelectGsuProgram(exits[0])

        # The remaining blocks choose one of two continuations from a word
        # mask. Absolute operands use the dispatcher's retail DBR=$7E; long
        # operands retain their literal bus bank (including the WRAM mirror in
        # bank $00).
        if len(exits) == 2 and len(continuations) == 2:
            loads = [
                ins for ins in instructions
                if ins.mnem == "LDA" and ins.mode in (cpu.ABS, cpu.ABL)
            ]
            masks = [
                ins.operand
                for ins in instructions
                if ins.mnem in ("AND", "BIT") and ins.mode == cpu.IMM_M
            ]
            if len(loads) == 1 and len(masks) == 1:
                load = loads[0]
                bus_address = (
                    (0x7E0000 | load.operand) if load.mode == cpu.ABS else load.operand
                )
                # Conditional layout is: BEQ clear; LDX set; RTL; clear: LDX;
                # RTL. This ordering is checked against the abstract exits.
                if set(continuations) != set(exits):
                    raise ValueError(f"inline branch exits disagree at {address.label()}")
                return InlineBranchWordBits(
                    bus_address, masks[0], continuations[1], continuations[0]
                )

        raise ValueError(f"unrecognized reachable inline block at {address.label()}")

    def _spawn(self, address: MapAddress, opcode: int) -> Spawn:
        if opcode in (10, 134):
            delay = self.word(address, 1)
            x = _s16(self.word(address, 3))
            y = _s16(self.word(address, 5))
            z = _s16(self.word(address, 7))
            shape = self.word(address, 9)
            strategy_addr = self.word(address, 11)
            strategy_bank = self.byte(address, 13)
            linked = self.word(address, 14) if opcode == 10 else None
        else:
            delay = self.byte(address, 1) * 4
            x = _s8(self.byte(address, 2)) * 4
            y = _s8(self.byte(address, 3)) * 4
            z = _s8(self.byte(address, 4)) * 16
            shape = self.word(address, 5)
            # The compact handler deliberately overlaps these reads with the
            # next record: +6/+7 form the strategy address and +8 its bank,
            # while the stream advances by seven bytes.  Preserve the ROM's
            # exact behavior; do not normalize it to a guessed 9-byte record.
            strategy_addr = self.word(address, 6)
            strategy_bank = self.byte(address, 8)
            linked = None
        return Spawn(address, opcode, delay, x, y, z, shape,
                     strategy_bank, strategy_addr, linked)

    def _successors(
        self,
        address: MapAddress,
        opcode: int,
        inline_exits: dict[MapAddress, tuple[int, ...]],
        unresolved_inline: list[MapAddress],
    ) -> list[MapAddress]:
        bank, stream = address.bank, address.offset
        if opcode in STATIC_STOPS:
            return []
        if opcode == 4:
            return [MapAddress(bank, self.word(address, 1)),
                    MapAddress(bank, stream + 5)]
        if opcode == 40:
            return [MapAddress(self.byte(address, 3), self.word(address, 1)),
                    MapAddress(bank, stream + 4)]
        if opcode == 46:
            target = MapAddress(self.byte(address, 3), self.word(address, 1))
            successors = [target]
            # Retail stage phases are delimited by this exact macro:
            #   $12 $88 $13                 delay $1388
            #   $2E <delay offset> <bank>   jump back to the delay
            # The dispatcher yields with $1657 parked on the jump.  Live Mesen
            # traces prove the host later writes/advances $1657 to the byte
            # after the jump (e.g. $05:E055 -> $05:E059 -> $05:E5B7 during
            # the all-range attract battle).  Include that host transition in
            # the closed graph, but only when the continuation begins with a
            # handler-table opcode; the final $FF data sentinel is not code.
            if self._is_external_phase_gate(address):
                continuation = MapAddress(bank, stream + RECORD_SIZES[opcode])
                if self.byte(continuation) in RECORD_SIZES:
                    successors.append(continuation)
            return successors
        if opcode == 120:
            exits, unresolved = self._decode_inline_exits(address)
            inline_exits[address] = tuple(sorted(exits))
            if unresolved:
                unresolved_inline.append(address)
            return [MapAddress(bank, target) for target in sorted(exits)]
        if opcode in (124, 126, 128):
            return [MapAddress(bank, self.word(address, 5)),
                    MapAddress(bank, stream + 7)]
        if opcode in (150, 152):
            return [MapAddress(bank, self.word(address, 3)),
                    MapAddress(bank, stream + 5)]
        if opcode == 158:
            return [MapAddress(bank, self.word(address, 2)),
                    MapAddress(bank, stream + 4)]
        if opcode in (162, 164):
            return [MapAddress(bank, self.word(address, 1)),
                    MapAddress(bank, stream + 3)]
        return [MapAddress(bank, stream + RECORD_SIZES[opcode])]

    def _is_external_phase_gate(self, address: MapAddress) -> bool:
        if address.offset < 3 or self.byte(address) != 46:
            return False
        hold = MapAddress(address.bank, address.offset - 3)
        return (
            self.byte(hold) == 0x12
            and self.word(hold, 1) == 0x1388
            and self.byte(address, 3) == address.bank
            and self.word(address, 1) == hold.offset
        )

    def extract(self) -> Extraction:
        roots = self.discover_roots()
        work = [root.address for root in roots]
        visited: set[MapAddress] = set()
        commands: list[Command] = []
        spawns: list[Spawn] = []
        inline_exits: dict[MapAddress, tuple[int, ...]] = {}
        unresolved_inline: list[MapAddress] = []
        invalid: list[tuple[MapAddress, int]] = []

        while work:
            address = work.pop()
            if address in visited or address.bank >= 0x20 or address.offset >= 0x8000:
                continue
            visited.add(address)
            opcode = self.byte(address)
            if opcode not in RECORD_SIZES:
                invalid.append((address, opcode))
                continue
            size = RECORD_SIZES[opcode]
            raw_size = max(1, size)
            off = self.file_offset(address)
            raw = self.rom[off:off + raw_size]
            commands.append(Command(address, opcode, size, raw.hex()))
            if opcode in SPAWN_OPS:
                spawns.append(self._spawn(address, opcode))
            work.extend(self._successors(
                address, opcode, inline_exits, unresolved_inline,
            ))

        commands.sort(key=lambda command: command.address)
        spawns.sort(key=lambda spawn: spawn.address)
        unresolved_inline.sort()
        invalid.sort(key=lambda item: item[0])
        inline_actions = {
            address: self._decode_inline_action(address, exits)
            for address, exits in sorted(inline_exits.items())
        }
        phase_gates = [
            ExternalPhaseGate(
                hold=MapAddress(command.address.bank, command.address.offset - 3),
                parked=command.address,
                continuation=MapAddress(
                    command.address.bank,
                    command.address.offset + RECORD_SIZES[command.opcode],
                ),
            )
            for command in commands
            if command.opcode == 46 and self._is_external_phase_gate(command.address)
            and self.byte(MapAddress(
                command.address.bank,
                command.address.offset + RECORD_SIZES[command.opcode],
            )) in RECORD_SIZES
        ]
        return Extraction(roots, commands, spawns, inline_exits, inline_actions,
                          unresolved_inline, invalid, phase_gates)


def _json(extraction: Extraction) -> dict[str, object]:
    return {
        "roots": [
            {
                "address": root.address.label(),
                "installed_at_file": root.installed_at_file,
            }
            for root in extraction.roots
        ],
        "commands": [
            {
                "address": command.address.label(),
                "opcode": command.opcode,
                "size": command.size,
                "raw_hex": command.raw_hex,
            }
            for command in extraction.commands
        ],
        "spawns": [
            {
                **asdict(spawn),
                "address": spawn.address.label(),
                "strategy": f"{spawn.strategy_bank:02X}:{spawn.strategy_addr:04X}",
            }
            for spawn in extraction.spawns
        ],
        "inline_exits": {
            address.label(): [f"{address.bank:02X}:{0x8000 + target:04X}"
                              for target in targets]
            for address, targets in sorted(extraction.inline_exits.items())
        },
        "unresolved_inline_exits": [
            address.label() for address in extraction.unresolved_inline_exits
        ],
        "invalid_opcodes": [
            {"address": address.label(), "opcode": opcode}
            for address, opcode in extraction.invalid_opcodes
        ],
        "phase_gates": [
            {
                "hold": gate.hold.label(),
                "parked": gate.parked.label(),
                "continuation": gate.continuation.label(),
            }
            for gate in extraction.phase_gates
        ],
    }


def _summary(extraction: Extraction) -> str:
    opcode_counts = Counter(command.opcode for command in extraction.commands)
    strategies = {(spawn.strategy_bank, spawn.strategy_addr)
                  for spawn in extraction.spawns}
    shapes = {spawn.shape for spawn in extraction.spawns}
    lines = [
        f"roots: {len(extraction.roots)}",
        f"reachable commands: {len(extraction.commands)}",
        f"reachable opcodes: {len(opcode_counts)}",
        f"inline routines: {len(extraction.inline_exits)}",
        f"unresolved inline exits: {len(extraction.unresolved_inline_exits)}",
        f"invalid opcodes: {len(extraction.invalid_opcodes)}",
        f"spawn records: {len(extraction.spawns)}",
        f"unique shapes: {len(shapes)}",
        f"unique strategy targets: {len(strategies)}",
        f"external phase gates: {len(extraction.phase_gates)}",
        "opcode histogram: " + " ".join(
            f"{opcode:02X}={count}" for opcode, count in sorted(opcode_counts.items())
        ),
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", nargs="?", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--json", action="store_true", help="emit the full extraction")
    args = parser.parse_args()
    extraction = MapExtractor(args.rom.read_bytes()).extract()
    if args.json:
        print(json.dumps(_json(extraction), indent=2, sort_keys=True))
    else:
        print(_summary(extraction))
    return 1 if extraction.invalid_opcodes or extraction.unresolved_inline_exits else 0


if __name__ == "__main__":
    raise SystemExit(main())
