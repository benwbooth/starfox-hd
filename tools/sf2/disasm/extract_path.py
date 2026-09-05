#!/usr/bin/env python3
"""Extract the reachable Star Fox 2 object-path bytecode from retail ROM.

The path interpreter is copied by reset from ROM ``$0A:8000..$CDFF`` to
WRAM ``$7F:7E00..$CBFF``.  Its dispatcher at ``$7F:7E75`` uses two tables:

* non-zero bytes select one of 256 entries at ``$7F:7EE8``;
* a zero escape advances the stream once, then indexes from ``$7F:82E8``
  (reported here as logical opcodes ``$100..$1FF``).

Conventional four-byte entries contain ``handler - 1`` followed by bank
``$7F``.  The upper half of the extended address range deliberately aliases
the handler bytes that follow the conventional table; reachable opcode $180
uses such a slot.  The dispatcher only loads and pushes the low word before
RTS, so the extractor mirrors that real address calculation instead of
imposing a source-level table bound.  Handler flow is followed from the copied retail bytes;
record advances and branch targets are taken from the common continuation
routines at ``$7F:CA0C..$CB3B`` rather than inferred from path data.

Path roots are the operands of reachable map opcode ``$8C`` records plus the
operands of reviewed 65816 instructions which directly install a path into an
object's ``+$2B`` field.  The reachable graph additionally follows the literal
child-path operands of the three spawn opcodes; those paths start independent
object VMs and therefore are entry edges even though they are not control-flow
successors of the spawning object.  Path bytes themselves live in Super FX ROM
bank ``$44`` (file offset ``$40000``).  Every direct installer is signature
checked, so the result contains no scan-only path candidates.
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
from extract_map import DEFAULT_ROM, MapExtractor


PATH_DATA_FILE = 0x040000
COPY_SOURCE_FILE = 0x050000
COPY_SOURCE_CPU = 0x0A8000
COPY_DEST_CPU = 0x7F7E00
COPY_LEN = 0x004E00

PRIMARY_TABLE = 0x7F7EE8
PRIMARY_COUNT = 0x100
EXTENDED_TABLE = 0x7F82E8
EXTENDED_COUNT = 0x100

REDISPATCH = 0x7F7E75
MOVE_AND_YIELD = 0x7F9DDE

# These handlers create an independent object-path VM.  The child path is the
# word at operand +3 for both the short quick-spawn record and the two aliases
# of the full child-spawn record.
_SPAWN_PATH_OPCODES = frozenset((0x033, 0x05D, 0x0F5))

# Reviewed native 65816 sites which load a literal path and store it through
# ``STA $002B,Y``.  These entries are reached by object strategies and weapon
# services rather than map bytecode, so a map-only root set silently omitted
# live effects such as the player's alternate exhaust path at $F536.  Keep the
# code sites (not merely their current operands) and verify the complete
# instruction signature before accepting each literal as a root.
_DIRECT_PATH_INSTALLERS: tuple[int, ...] = (
    0x032089, 0x032172, 0x0321A4, 0x0321B0, 0x0321BC,
    0x0373EC, 0x0377D6,
    0x03CBC0, 0x03CDB3, 0x03CE21, 0x03CE98, 0x03CF94,
    0x03D03B, 0x03D09A, 0x03D10C, 0x03D145, 0x03D18B,
    0x03D3A6, 0x03D489, 0x03D67A,
    0x06CE1C, 0x06CE84,
    0x06DBB3, 0x06DBCA, 0x06DBE1, 0x06DC03, 0x06DC25,
    0x06DC3C, 0x06DC53, 0x06DC6A, 0x06DC93, 0x06DCAA,
    0x06DCDA, 0x06DD74, 0x06DE24,
)
_DIRECT_PATH_PREFIX = bytes.fromhex("c220a9")
_DIRECT_PATH_SUFFIX = bytes.fromhex("992b00")


@dataclass(frozen=True, order=True)
class PathAddress:
    """Offset within the full 64 KiB Super FX ROM bank ``$44``."""

    offset: int

    @property
    def cpu(self) -> int:
        return 0x440000 | self.offset

    def label(self) -> str:
        return f"44:{self.offset:04X}"


@dataclass(frozen=True)
class HandlerEntry:
    opcode: int
    table_address: int
    stored_address: int
    handler_address: int
    bank: int


@dataclass(frozen=True, order=True)
class FlowEffect:
    """A terminal path-pointer effect reached through a handler CFG."""

    kind: str
    value: int | None = None
    yields: bool = False
    resets_counter: bool = False


@dataclass(frozen=True)
class HandlerAnalysis:
    opcode: int
    handler_address: int
    effects: tuple[FlowEffect, ...]
    instruction_addresses: tuple[int, ...]
    unresolved_targets: tuple[int, ...]


@dataclass(frozen=True)
class PathCommand:
    address: PathAddress
    opcode: int
    prefix_size: int
    handler_address: int
    raw_hex: str
    effects: tuple[FlowEffect, ...]
    successors: tuple[PathAddress, ...]


@dataclass
class Extraction:
    roots: list[PathAddress]
    commands: list[PathCommand]
    handlers: dict[int, HandlerAnalysis]
    invalid_opcodes: list[tuple[PathAddress, int]]
    unresolved_handlers: list[int]


# Common pointer continuations.  Values are bytes relative to the pointer the
# handler sees.  Extended commands have already consumed their zero escape.
_ADVANCE_HELPERS: dict[int, tuple[int, bool]] = {
    0x7FCA0C: (17, False),
    0x7FCA1D: (16, False),
    0x7FCA2E: (14, False),
    0x7FCA3F: (13, False),
    0x7FCA50: (9, False),
    0x7FCA61: (8, False),
    0x7FCA72: (7, False),
    0x7FCA83: (6, False),
    0x7FCA94: (5, False),
    0x7FCAA5: (4, True),
    0x7FCAA9: (4, False),
    0x7FCABA: (3, True),
    0x7FCABE: (3, False),
    0x7FCACF: (2, True),
    0x7FCAD3: (2, False),
    0x7FCAE4: (1, True),
    0x7FCAE8: (1, False),
}

_JUMP_HELPERS: dict[int, int] = {
    0x7FCAF3: 1,
    0x7FCAFF: 2,
    0x7FCB0B: 3,
    0x7FCB17: 4,
    0x7FCB23: 5,
    0x7FCB2F: 6,
    0x7FCB3B: 7,
}

# Helpers returning a path operand in A.  Tracking these through STA $2B,X
# proves the few handlers that assign a pointer and then jump to movement.
_PATH_WORD_READERS: dict[int, int] = {
    0x7FC720: 1,
    0x7FC74C: 2,
    0x7FC778: 3,
    0x7FC7A4: 4,
    0x7FC7D0: 5,
    0x7FC7FC: 6,
    0x7FC828: 7,
}

# $7F:9B0C dispatches an object-state value through these 18 words.  The
# table ends exactly where its final target ($9B33) begins.
_OBJECT_STATE_JUMP_TABLE = 0x7F9B0F
_OBJECT_STATE_JUMP_COUNT = 18

# These three handlers mutate the per-object path call stack before reaching
# generic pointer helpers.  A context-free CFG sees only the final pointer
# write and therefore misclassifies RETURN as a three-byte advance at the
# return instruction itself.  Complete handler review establishes the actual
# bytecode-level effects:
#
# * $041 pushes the current path pointer and calls operand word +1;
# * $042 pops a caller pointer and resumes three bytes after that GOSUB;
# * $064 does the same using a word variable selected by operand byte +1.
#
# Model calls interprocedurally: both the callee and caller continuation are
# reachable, while RETURN itself remains a one-byte record.
_REVIEWED_CONTROL_EFFECTS: dict[int, tuple[FlowEffect, ...]] = {
    # $033 and $0F5 share one spawn handler, but the handler explicitly tests
    # the dispatcher-saved logical opcode at $1911.  $033 consumes the full
    # 17-byte child record; $0F5 consumes its 14-byte variant.  Treating both
    # terminal advances as possible for either opcode enters operand bytes as
    # fake path code and eventually escapes into embedded 65816 routines.
    0x033: (FlowEffect("advance", 17),),
    0x041: (FlowEffect("call", 1),),
    0x042: (FlowEffect("return"),),
    # Trigger builders call the shared heap-growth routine at $7F:97F3 and
    # then return through the normal path-pointer helpers.  The generic CFG
    # can merge an internal RTS state before recovering its caller, so retain
    # the reviewed bytecode-level effect and asynchronous path edge here.
    0x04A: (FlowEffect("advance", 4), FlowEffect("schedule", 1)),
    # FORCE_TRIGGER_PATH installs its literal path immediately through the
    # same trigger service.  The caller still falls through, while the target
    # becomes a separately executing object path and must therefore be part
    # of the closed retail graph.  Omitting this edge hid Meteor's base-opening
    # sequence beginning at $54F6.
    0x04C: (FlowEffect("advance", 3), FlowEffect("schedule", 1)),
    0x064: (FlowEffect("dynamic_call", 1),),
    # `$089` enters 65816 code embedded directly after the opcode.  The
    # inline block returns the next path offset in 16-bit A; decode those
    # blocks separately instead of treating the handler's RTL as path end.
    0x089: (FlowEffect("inline"),),
    0x0F8: (FlowEffect("advance", 3), FlowEffect("schedule", 1)),
    0x0F5: (FlowEffect("advance", 14),),
    0x0FD: (FlowEffect("advance", 3), FlowEffect("relative_schedule", 1)),
    0x165: (FlowEffect("advance", 5), FlowEffect("schedule", 1)),
}

# `$15E` ends the path after installing the following strategy word and state
# byte. Its dynamic-yield CFG has no pointer successor from which the generic
# decoder could infer those three semantic operands, so retain the reviewed
# five-byte record explicitly (zero escape, opcode, word, byte).
_REVIEWED_RECORD_SIZES: dict[int, int] = {
    0x15E: 5,
}


# Fully reviewed inline blocks reachable before following their returned path
# pointers.  Keys and successors are offsets within Super FX ROM bank `$44`.
# The byte signatures prevent a continuation from being asserted merely from
# a coincidental address.  More blocks are added as graph expansion reaches
# them; an unknown reachable `$089` is a hard extraction error.
_PATH_INLINE_BLOCKS: dict[int, tuple[bytes, tuple[int, ...]]] = {
    # Set current object flag `$25` bit `$02`; continue at `$4B5A`.
    0x4B4D: (bytes.fromhex("b52509029525c220a95a4b6b"), (0x4B5A,)),
    # Publish the current object through `$D767`; continue at `$9E7B`.
    0x9E71: (bytes.fromhex("8e67d7c220a97b9e6b"), (0x9E7B,)),
    # Call the retail `$07:F880` service; continue at `$F6F1`.
    0xF6E6: (bytes.fromhex("2280f807c220a9f1f66b"), (0xF6F1,)),
    # Native object-strategy thunks reached only by the reviewed direct path
    # installers above.  Each returns a literal continuation in 16-bit A.
    0xB8C5: (bytes.fromhex("a90222f86d7fc220a9d2b86b"), (0xB8D2,)),
    0xB8E4: (bytes.fromhex("2266fa06c220a9efb86b"), (0xB8EF,)),
    0xB91A: (bytes.fromhex("2201f507c220a925b96b"), (0xB925,)),
    0xCFF8: (bytes.fromhex("ad741d09048d741dc220a907d06b"), (0xD007,)),
    0xD024: (bytes.fromhex("2208f807c220a92fd06b"), (0xD02F,)),
    0xD098: (bytes.fromhex("ad741d09088d741dc220a9a7d06b"), (0xD0A7,)),
    0xD0DE: (bytes.fromhex("22e1f307c220a9e9d06b"), (0xD0E9,)),
    0xD253: (bytes.fromhex("ad741d09088d741dc220a962d26b"), (0xD262,)),
    0xDCBD: (bytes.fromhex("22b6f307c220a9c8dc6b"), (0xDCC8,)),
    0xE78A: (bytes.fromhex("2285a806c220a995e76b"), (0xE795,)),
    0xE839: (bytes.fromhex("225ff907c220a944e86b"), (0xE844,)),
    0xE845: (bytes.fromhex("ad741d09018d741dc220a954e86b"), (0xE854,)),
    0xE967: (bytes.fromhex("2253f906c220a972e96b"), (0xE972,)),
    0xF078: (bytes.fromhex("224ef507c220a983f06b"), (0xF083,)),
    # The first return is the not-equal arm; the equal arm branches over it
    # and returns the second continuation.
    0xF2E4: (
        bytes.fromhex("228df507bde21c89fef007c220a9f6f26b42c220a9fdf26b"),
        (0xF2F6, 0xF2FD),
    ),
    # One arm tail-jumps into the shared inline suffix at $09:F35C; the other
    # sets the per-object byte first.  Both return $F362.
    0xF348: (
        bytes.fromhex("acc312b925002920d0045c5cf309a9019de21cc220a962f36b"),
        (0xF362,),
    ),
    0xF391: (bytes.fromhex("22ecf507c220a99cf36b"), (0xF39C,)),
    0xF39E: (bytes.fromhex("2234f607c220a9a9f36b"), (0xF3A9,)),
    0xF3F0: (bytes.fromhex("22aefa06c220a9fbf36b"), (0xF3FB,)),
    0xF45B: (bytes.fromhex("2248fb06c220a966f46b"), (0xF466,)),
    0xF46E: (bytes.fromhex("22fbfa06c220a979f46b"), (0xF479,)),
    0xF500: (bytes.fromhex("227db607c220a90bf56b"), (0xF50B,)),
    # Publish the child auxiliary record selected by `$D771`; these repeated
    # thunks return the literal continuation immediately following the block.
    0x2059: (bytes.fromhex("ac71d7961cc220a965206b"), (0x2065,)),
    0x9122: (bytes.fromhex("ac71d7961cc220a92e916b"), (0x912E,)),
    0x919F: (bytes.fromhex("ac71d7961cc220a9ab916b"), (0x91AB,)),
    0x91DA: (bytes.fromhex("ac71d7961cc220a9e6916b"), (0x91E6,)),
    # Clear current object flag `$24` bit `$04`; continue at `$8D61`.
    0x8D54: (bytes.fromhex("b52429fb9524c220a9618d6b"), (0x8D61,)),
    # Set current object flag `$24` bit `$04`; continue at `$8D6F`.
    0x8D62: (bytes.fromhex("b52409049524c220a96f8d6b"), (0x8D6F,)),
    # Toggle the current object's color-animation phase; continue at `$9817`.
    0x9808: (bytes.fromhex("bdca1c49019dca1cc220a917986b"), (0x9817,)),
    # Dispatch through `$09:A8B2,X`, where X is the even byte index in
    # `$D77F`.  The ten-entry table immediately precedes path `$A8C6`.
    0xAB2A: (
        bytes.fromhex("c2209bae7fd7bfb2a809bb6b"),
        (0x8D81, 0xA8C6, 0xA8CA, 0xA8D5, 0xA8DC,
         0xA915, 0xA95C, 0xA964, 0xAA3A),
    ),
    # Retail launch helper embedded in the spawned object's path.  It invokes
    # the two bank-$7F services, copies/transforms the selected object's pose,
    # and returns the literal path continuation `$ACC1`.
    0xAC64: (
        bytes.fromhex(
            "acd61422aa2b7f22d22b7fa932991800dabbb5188585b5148d1215b5128d1115"
            "221f2d7ffaacd614e220a9148db116e220ceb116f020dabb7a22242c7fdabb7a"
            "c220b90e0069100030e5b90c008d67d7b910008d69d7c220a9c1ac6b"
        ),
        (0xACC1,),
    ),
    # Copy selected-object world X/Z/Y to the current object; continue at
    # `$B0E6`.
    0xB0CB: (
        bytes.fromhex("ac1fcfc220b90c00950cb910009510b90e00950ec220a9e6b06b"),
        (0xB0E6,),
    ),
    # Call the retail `$0D:AF3A` service; continue at `$B121`.
    0xB116: (bytes.fromhex("223aaf0dc220a921b16b"), (0xB121,)),
    # Set current object flag `$25` bit `$08`; continue at `$B136`.
    0xB129: (bytes.fromhex("b52509089525c220a936b16b"), (0xB136,)),
    # Install X into the auxiliary record selected by `$D771`; continue at
    # `$E69C`.
    0xE690: (bytes.fromhex("ac71d7961cc220a99ce66b"), (0xE69C,)),
    # Service call followed by a branch over an alternate embedded entry;
    # the taken path returns `$E94D` in A.
    0xE939: (
        bytes.fromhex("22d6f607800793a18922d0f607c220a94de96b"),
        (0xE94D,),
    ),
    # Three retail service thunks used by the late all-range paths.
    0xF659: (bytes.fromhex("2290f307c220a964f66b"), (0xF664,)),
    # Dispatch through the ten-word `$09:F5AC,X` table selected by `$D77F`.
    0xF668: (
        bytes.fromhex("c2209bae7fd7bfacf509bb6b"),
        (0x040B, 0x07FE, 0x7A2E, 0x8D81, 0x96A1,
         0xA26B, 0xC190, 0xF72C, 0xF7B1, 0xF7C9),
    ),
    0xF693: (bytes.fromhex("2265f307c220a99ef66b"), (0xF69E,)),
    # Set current object flag `$25` bit `$02`; continue at `$F320`.
    0xF313: (bytes.fromhex("b52509029525c220a920f36b"), (0xF320,)),
    0xF7C9: (bytes.fromhex("2280f707c220a9d4f76b"), (0xF7D4,)),
    0xF9A1: (bytes.fromhex("ac71d7961cc220a9adf96b"), (0xF9AD,)),
    # Attract craft drift: call $06:FA04 (ease bank toward level,
    # accumulate and damp the three drift components), then return FCC4.
    # The literal return proves path reachability, not a native port of the
    # called motion service. Keep those certification claims separate.
    0xFCB9: (bytes.fromhex("2204fa06c220a9c4fc6b"), (0xFCC4,)),
    # Same child-auxiliary publication thunk as 2059/9122/919F/91DA above.
    # Source $09:FDDD..FDE7 returns the first path byte after the RTL.
    0xFDDC: (bytes.fromhex("ac71d7961cc220a9e8fd6b"), (0xFDE8,)),
    # Indexed attract scene seven. These two source phase tests return one
    # of two literal path addresses; retain both arms (including the embedded
    # path Return byte between the machine-code arms).
    0xB796: (
        bytes.fromhex("ade01bc901f007c220a9a4b76b42c220a9abb76b"),
        (0xB7A4, 0xB7AB),
    ),
    0xB869: (
        bytes.fromhex("ade01bc9011007c220a977b86b42c220a97eb86b"),
        (0xB877, 0xB87E),
    ),
    # $07:F52B eases the camera's fine yaw toward the three-quarter turn;
    # $07:F3D1 advances the six-entry pilot selector. Both end in RTL.
    0xB7E7: (bytes.fromhex("222bf507c220a9f2b76b"), (0xB7F2,)),
    0xE91B: (bytes.fromhex("22d1f307c220a926e96b"), (0xE926,)),
}


def _runtime_low(address: int) -> int:
    return address & 0xFFFF


class PathExtractor:
    def __init__(self, rom: bytes):
        self.rom = rom
        self._handler_cache: dict[int, HandlerAnalysis] = {}
        self._handler_instruction_cache: dict[int, tuple[cpu.Insn, ...]] = {}

    def handler_instructions(self, opcode: int) -> tuple[cpu.Insn, ...]:
        """Return every width-resolved instruction reached by a handler CFG.

        Instructions reached with multiple M/X flag states are retained as
        separate records.  This review view includes shared helpers and both
        branch arms without decoding 16-bit immediates as code.
        """
        self.analyze_handler(opcode)
        return self._handler_instruction_cache[opcode]

    @staticmethod
    def runtime_file(address: int) -> int:
        """Translate copied WRAM code back to its byte in retail ROM."""
        if not (COPY_DEST_CPU <= address < COPY_DEST_CPU + COPY_LEN):
            raise ValueError(f"address is outside the reset copy: ${address:06X}")
        return COPY_SOURCE_FILE + address - COPY_DEST_CPU

    @staticmethod
    def source_cpu_to_runtime(address: int) -> int:
        """Translate a PC-relative target decoded in source bank $0A."""
        if not (COPY_SOURCE_CPU <= address < COPY_SOURCE_CPU + COPY_LEN):
            raise ValueError(f"source target is outside copied code: ${address:06X}")
        return COPY_DEST_CPU + address - COPY_SOURCE_CPU

    def path_byte(self, address: PathAddress, delta: int = 0) -> int:
        return self.rom[PATH_DATA_FILE + ((address.offset + delta) & 0xFFFF)]

    def path_word(self, address: PathAddress, delta: int) -> int:
        lo = self.path_byte(address, delta)
        hi = self.path_byte(address, delta + 1)
        return lo | hi << 8

    def discover_roots(self) -> list[PathAddress]:
        """Read path roots from map records and reviewed native installers."""
        map_result = MapExtractor(self.rom).extract()
        roots = {
            int.from_bytes(bytes.fromhex(command.raw_hex)[1:3], "little")
            for command in map_result.commands
            if command.opcode == 0x8C
        }
        for file_offset in _DIRECT_PATH_INSTALLERS:
            instruction = self.rom[file_offset:file_offset + 8]
            if (instruction[:3] != _DIRECT_PATH_PREFIX
                    or instruction[5:] != _DIRECT_PATH_SUFFIX):
                raise ValueError(
                    f"direct path installer signature mismatch at file ${file_offset:06X}"
                )
            roots.add(int.from_bytes(instruction[3:5], "little"))
        return [PathAddress(offset) for offset in sorted(roots)]

    def _table_address(self, opcode: int) -> int:
        if 0 <= opcode < PRIMARY_COUNT:
            return PRIMARY_TABLE + opcode * 4
        if 0x100 <= opcode < 0x100 + EXTENDED_COUNT:
            return EXTENDED_TABLE + (opcode - 0x100) * 4
        raise ValueError(f"invalid logical path opcode ${opcode:03X}")

    def handler_entry(self, opcode: int) -> HandlerEntry:
        table = self._table_address(opcode)
        off = self.runtime_file(table)
        stored = int.from_bytes(self.rom[off:off + 2], "little")
        bank = self.rom[off + 3]
        return HandlerEntry(opcode, table, stored, 0x7F0000 | ((stored + 1) & 0xFFFF), bank)

    def handler_entries(self) -> list[HandlerEntry]:
        return [
            self.handler_entry(opcode)
            for opcode in list(range(PRIMARY_COUNT))
            + list(range(0x100, 0x100 + EXTENDED_COUNT))
        ]

    def _decode_runtime(self, address: int, m_flag: int, x_flag: int) -> cpu.Insn:
        return cpu.decode_one(self.rom, self.runtime_file(address), m_flag, x_flag)

    def _control_target(self, ins: cpu.Insn) -> int | None:
        if ins.mode in (cpu.REL, cpu.RELL):
            if ins.target is None:
                return None
            return self.source_cpu_to_runtime(ins.target)
        if ins.mnem in ("JMP", "JSR") and ins.mode == cpu.ABS:
            return 0x7F0000 | ins.operand
        if ins.mnem in ("JML", "JSL") and ins.mode == cpu.ABL:
            return ins.operand
        return None

    @staticmethod
    def _terminal_effect(address: int, pointer_action: tuple[str, int] | None) -> FlowEffect | None:
        if address in _ADVANCE_HELPERS:
            size, resets = _ADVANCE_HELPERS[address]
            return FlowEffect("advance", size, resets_counter=resets)
        if address in _JUMP_HELPERS:
            return FlowEffect("jump", _JUMP_HELPERS[address])
        if address == REDISPATCH:
            if pointer_action is None:
                return FlowEffect("redispatch")
            if pointer_action[0] == "dynamic_jump":
                return FlowEffect("dynamic_jump")
            return FlowEffect(pointer_action[0], pointer_action[1])
        if address == MOVE_AND_YIELD:
            if pointer_action is None:
                return FlowEffect("hold", yields=True)
            if pointer_action[0] == "dynamic_jump":
                return FlowEffect("dynamic_jump", yields=True)
            return FlowEffect(pointer_action[0], pointer_action[1], yields=True)
        return None

    def analyze_handler(self, opcode: int) -> HandlerAnalysis:
        cached = self._handler_cache.get(opcode)
        if cached is not None:
            return cached

        entry = self.handler_entry(opcode)
        # Handler dispatch runs with 8-bit A and 16-bit X/Y.  The final tuple
        # member tracks a proven path-pointer assignment: (jump operand offset)
        # or a direct byte advance.
        work: list[
            tuple[
                int,
                int,
                int,
                tuple[str, int] | None,
                tuple[str, int] | None,
                tuple[int, ...],
                tuple[tuple[int, int], ...],
            ]
        ] = [
            (entry.handler_address, 1, 0, None, None, (), ())
        ]
        seen: set[
            tuple[
                int,
                int,
                int,
                tuple[str, int] | None,
                tuple[str, int] | None,
                tuple[int, ...],
                tuple[tuple[int, int], ...],
            ]
        ] = set()
        instruction_addresses: set[int] = set()
        decoded_instructions: dict[tuple[int, int, int], cpu.Insn] = {}
        effects: set[FlowEffect] = set()
        unresolved: set[int] = set()

        while work:
            pc, m_flag, x_flag, a_expr, pointer_action, return_stack, status_stack = work.pop()
            while True:
                terminal = self._terminal_effect(pc, pointer_action)
                if terminal is not None:
                    effects.add(terminal)
                    break
                if not (COPY_DEST_CPU <= pc < COPY_DEST_CPU + COPY_LEN):
                    unresolved.add(pc)
                    break

                state = (pc, m_flag, x_flag, a_expr, pointer_action, return_stack, status_stack)
                if state in seen:
                    break
                seen.add(state)
                if len(seen) > 20_000:
                    unresolved.add(pc)
                    break

                ins = self._decode_runtime(pc, m_flag, x_flag)
                instruction_addresses.add(pc)
                decoded_instructions[(pc, m_flag, x_flag)] = ins
                next_pc = pc + ins.length
                next_m, next_x = cpu.update_flags(ins, m_flag, x_flag)
                next_a = a_expr
                next_pointer = pointer_action
                next_status = status_stack

                if ins.mnem == "PHP":
                    next_status = status_stack + ((m_flag, x_flag),)
                elif ins.mnem == "PLP":
                    if status_stack:
                        next_m, next_x = status_stack[-1]
                        next_status = status_stack[:-1]
                    else:
                        # A PLP without a statically paired PHP consumes an
                        # unknown status byte.  Both width combinations are
                        # possible, so stop instead of decoding data as code.
                        unresolved.add(pc)
                        break

                target = self._control_target(ins)
                if ins.mnem == "JSR":
                    if target in _PATH_WORD_READERS:
                        next_a = ("path_word", _PATH_WORD_READERS[target])
                    elif target is not None and COPY_DEST_CPU <= target < COPY_DEST_CPU + COPY_LEN:
                        if len(return_stack) >= 32:
                            unresolved.add(target)
                            break
                        return_stack = return_stack + (next_pc,)
                        pc = target
                        m_flag, x_flag = next_m, next_x
                        a_expr, pointer_action = next_a, next_pointer
                        status_stack = next_status
                        continue
                    else:
                        next_a = None
                elif ins.mnem == "JSL":
                    if target is not None and COPY_DEST_CPU <= target < COPY_DEST_CPU + COPY_LEN:
                        if len(return_stack) >= 32:
                            unresolved.add(target)
                            break
                        return_stack = return_stack + (next_pc,)
                        pc = target
                        m_flag, x_flag = next_m, next_x
                        a_expr, pointer_action = None, next_pointer
                        status_stack = next_status
                        continue
                    next_a = None
                elif ins.mnem in ("LDA", "PLA", "TDC", "TXA", "TYA"):
                    next_a = None

                if (ins.mnem == "STA" and ins.mode in (cpu.DPX, cpu.ABX)
                        and ins.operand == 0x2B):
                    if next_a is not None and next_a[0] == "path_word":
                        next_pointer = ("jump", next_a[1])
                    else:
                        next_pointer = ("dynamic_jump", 0)
                elif (ins.mnem == "STY" and ins.mode in (cpu.DPX, cpu.ABX)
                      and ins.operand == 0x2B):
                    next_pointer = ("dynamic_jump", 0)
                elif ins.mnem == "INC" and ins.mode == cpu.DPX and ins.operand == 0x2B:
                    # This is executed with 16-bit A in the one reachable
                    # direct-advance handler, so it increments the path word.
                    next_pointer = ("advance", 1)

                if ins.mnem in cpu.BRANCHES:
                    if target is None:
                        unresolved.add(pc)
                        break
                    if ins.mnem in ("BRA", "BRL"):
                        pc = target
                        m_flag, x_flag = next_m, next_x
                        a_expr, pointer_action = next_a, next_pointer
                        status_stack = next_status
                        continue
                    work.append((
                        target, next_m, next_x, next_a, next_pointer,
                        return_stack, next_status,
                    ))
                    pc = next_pc
                    m_flag, x_flag = next_m, next_x
                    a_expr, pointer_action = next_a, next_pointer
                    status_stack = next_status
                    continue

                if ins.mnem in ("JMP", "JML"):
                    if ins.mode == cpu.IAX and ins.operand == (_OBJECT_STATE_JUMP_TABLE & 0xFFFF):
                        table_off = self.runtime_file(_OBJECT_STATE_JUMP_TABLE)
                        for index in range(_OBJECT_STATE_JUMP_COUNT):
                            branch = int.from_bytes(
                                self.rom[table_off + index * 2:table_off + index * 2 + 2],
                                "little",
                            )
                            work.append((
                                0x7F0000 | branch, next_m, next_x, next_a,
                                next_pointer, return_stack, next_status,
                            ))
                        break
                    if target is None:
                        unresolved.add(pc)
                        break
                    pc = target
                    m_flag, x_flag = next_m, next_x
                    a_expr, pointer_action = next_a, next_pointer
                    status_stack = next_status
                    continue

                if ins.mnem in cpu.RETURNS:
                    if return_stack:
                        pc = return_stack[-1]
                        return_stack = return_stack[:-1]
                        m_flag, x_flag = next_m, next_x
                        a_expr, pointer_action = next_a, next_pointer
                        status_stack = next_status
                        continue
                    effects.add(FlowEffect("return"))
                    break
                if ins.mnem in ("BRK", "COP", "STP", "WAI"):
                    effects.add(FlowEffect(ins.mnem.lower()))
                    break

                pc = next_pc
                m_flag, x_flag = next_m, next_x
                a_expr, pointer_action = next_a, next_pointer
                status_stack = next_status

        analysis = HandlerAnalysis(
            opcode=opcode,
            handler_address=entry.handler_address,
            effects=_REVIEWED_CONTROL_EFFECTS.get(opcode, tuple(sorted(effects))),
            instruction_addresses=tuple(sorted(instruction_addresses)),
            unresolved_targets=tuple(sorted(unresolved)),
        )
        self._handler_cache[opcode] = analysis
        self._handler_instruction_cache[opcode] = tuple(
            decoded_instructions[key] for key in sorted(decoded_instructions)
        )
        return analysis

    def decode_command(self, address: PathAddress) -> PathCommand:
        first = self.path_byte(address)
        if first:
            opcode = first
            prefix_size = 0
        else:
            opcode = 0x100 | self.path_byte(address, 1)
            prefix_size = 1

        analysis = self.analyze_handler(opcode)
        effects = _REVIEWED_CONTROL_EFFECTS.get(opcode, analysis.effects)
        successors: set[PathAddress] = set()
        raw_size = max(prefix_size + 1, _REVIEWED_RECORD_SIZES.get(opcode, 0))
        for effect in effects:
            if effect.kind == "advance" and effect.value is not None:
                successors.add(PathAddress((address.offset + prefix_size + effect.value) & 0xFFFF))
                raw_size = max(raw_size, prefix_size + effect.value)
            elif effect.kind == "jump" and effect.value is not None:
                target = self.path_word(address, prefix_size + effect.value)
                successors.add(PathAddress(target))
                raw_size = max(raw_size, prefix_size + effect.value + 2)
            elif effect.kind == "call" and effect.value is not None:
                target = self.path_word(address, prefix_size + effect.value)
                raw_size = max(raw_size, prefix_size + effect.value + 2)
                successors.add(PathAddress(target))
                successors.add(PathAddress((address.offset + raw_size) & 0xFFFF))
            elif effect.kind == "dynamic_call" and effect.value is not None:
                raw_size = max(raw_size, prefix_size + effect.value + 1)
                successors.add(PathAddress((address.offset + raw_size) & 0xFFFF))
            elif effect.kind == "schedule" and effect.value is not None:
                target = self.path_word(address, prefix_size + effect.value)
                successors.add(PathAddress(target))
                raw_size = max(raw_size, prefix_size + effect.value + 2)
            elif effect.kind == "relative_schedule" and effect.value is not None:
                delta = self.path_byte(address, prefix_size + effect.value)
                successors.add(PathAddress((address.offset + prefix_size + delta) & 0xFFFF))
                raw_size = max(raw_size, prefix_size + effect.value + 1)
            elif effect.kind == "inline":
                block = _PATH_INLINE_BLOCKS.get(address.offset)
                if block is None:
                    raise ValueError(
                        f"unreviewed path inline block at {address.label()}"
                    )
                signature, inline_successors = block
                actual = bytes(
                    self.path_byte(address, prefix_size + 1 + index)
                    for index in range(len(signature))
                )
                if actual != signature:
                    raise ValueError(
                        f"path inline signature mismatch at {address.label()}"
                    )
                successors.update(PathAddress(offset) for offset in inline_successors)

        raw = bytes(self.path_byte(address, i) for i in range(raw_size))
        return PathCommand(
            address=address,
            opcode=opcode,
            prefix_size=prefix_size,
            handler_address=analysis.handler_address,
            raw_hex=raw.hex(),
            effects=effects,
            successors=tuple(sorted(successors)),
        )

    def extract(self) -> Extraction:
        roots = self.discover_roots()
        work = list(reversed(roots))
        commands: dict[PathAddress, PathCommand] = {}
        invalid: list[tuple[PathAddress, int]] = []

        while work:
            address = work.pop()
            if address in commands:
                continue
            try:
                command = self.decode_command(address)
            except (IndexError, ValueError):
                invalid.append((address, self.path_byte(address)))
                continue
            commands[address] = command
            for successor in reversed(command.successors):
                if successor not in commands:
                    work.append(successor)
            if command.opcode in _SPAWN_PATH_OPCODES:
                child_path = PathAddress(
                    self.path_word(address, command.prefix_size + 3)
                )
                if child_path.offset != 0 and child_path not in commands:
                    work.append(child_path)

        handlers = {
            opcode: self.analyze_handler(opcode)
            for opcode in sorted({command.opcode for command in commands.values()})
        }
        unresolved_handlers = [
            opcode for opcode, handler in handlers.items()
            if handler.unresolved_targets
            or any(effect.kind in ("redispatch", "brk", "cop", "stp", "wai")
                   for effect in handler.effects)
        ]
        return Extraction(
            roots=roots,
            commands=[commands[address] for address in sorted(commands)],
            handlers=handlers,
            invalid_opcodes=invalid,
            unresolved_handlers=unresolved_handlers,
        )


def _jsonable(result: Extraction) -> dict:
    return {
        "roots": [asdict(root) for root in result.roots],
        "commands": [asdict(command) for command in result.commands],
        "handlers": {
            f"{opcode:03X}": asdict(handler)
            for opcode, handler in result.handlers.items()
        },
        "invalid_opcodes": [
            {"address": asdict(address), "byte": byte}
            for address, byte in result.invalid_opcodes
        ],
        "unresolved_handlers": result.unresolved_handlers,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", nargs="?", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--json", action="store_true", help="emit the full extraction as JSON")
    parser.add_argument("--handlers", action="store_true", help="print reachable handler effects")
    args = parser.parse_args()

    result = PathExtractor(args.rom.read_bytes()).extract()
    if args.json:
        print(json.dumps(_jsonable(result), indent=2))
        return 0

    counts = Counter(command.opcode for command in result.commands)
    print(f"roots={len(result.roots)} commands={len(result.commands)} opcodes={len(counts)}")
    print(f"invalid={len(result.invalid_opcodes)} unresolved_handlers={len(result.unresolved_handlers)}")
    if args.handlers:
        for opcode, handler in result.handlers.items():
            effects = ", ".join(
                effect.kind
                + (f"({effect.value})" if effect.value is not None else "")
                + ("+yield" if effect.yields else "")
                + ("+reset" if effect.resets_counter else "")
                for effect in handler.effects
            )
            unresolved = "" if not handler.unresolved_targets else (
                " unresolved=" + ",".join(f"${target:06X}" for target in handler.unresolved_targets)
            )
            print(
                f"{opcode:03X} count={counts[opcode]:3d} handler=${handler.handler_address:06X} "
                f"{effects}{unresolved}"
            )
    return 1 if result.invalid_opcodes or result.unresolved_handlers else 0


if __name__ == "__main__":
    raise SystemExit(main())
