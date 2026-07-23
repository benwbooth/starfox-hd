#!/usr/bin/env python3
"""Emit the mechanically reachable SF2 map program as Rust data.

The reachability and inline-65816 analysis live in ``disasm/extract_map.py``.
This front-end only serializes that verified result into ``sf2-data`` so the
runtime can consume exact roots, operands, spawns, and inline continuations
without reading the retail ROM at run time.
"""

from __future__ import annotations

import os

from disasm.extract_map import (
    InlineBranchWordBits,
    InlineCall,
    InlineSelectGsuProgram,
    InlineSetPilotLinkedFlag,
    InlineWordBits,
    MapExtractor,
)
from rom import AUTOGEN_HEADER, RUST_SRC


def _address(a) -> str:
    return f"MapAddress {{ bank: 0x{a.bank:02X}, address: 0x{0x8000 + a.offset:04X} }}"


def emit_rust(extraction) -> None:
    if extraction.invalid_opcodes or extraction.unresolved_inline_exits:
        raise RuntimeError(
            "refusing to generate ambiguous map data: "
            f"invalid={extraction.invalid_opcodes}, "
            f"unresolved_inline={extraction.unresolved_inline_exits}"
        )

    lines = [AUTOGEN_HEADER.format(tool="extract_map.py")]
    lines += [
        "//! Reachable SF2 map bytecode recovered from the retail dispatcher.",
        "//!",
        "//! Roots are actual `$192E/$1657` installs, commands are reached by",
        "//! following the handlers' real transitions plus the host-released",
        "//! `$1388` phase gates, and `$78` inline-code",
        "//! continuations come from a conservative 65816 abstract interpreter.",
        "//! No scan-only candidates or guessed records are included.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]",
        "pub struct MapAddress {",
        "    pub bank: u8,",
        "    /// CPU address within the LoROM bank (`$8000..=$FFFF`).",
        "    pub address: u16,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ScriptRoot {",
        "    pub address: MapAddress,",
        "    /// ROM file offset of the installing host instruction, if literal.",
        "    pub installed_at_file: Option<u32>,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct MapCommand {",
        "    pub address: MapAddress,",
        "    pub opcode: u8,",
        "    /// Handler-proven total byte length; zero denotes a control stop.",
        "    pub size: u8,",
        "    pub raw_len: u8,",
        "    pub raw: [u8; 16],",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct SpawnRecord {",
        "    pub address: MapAddress,",
        "    pub opcode: u8,",
        "    pub delay: u16,",
        "    pub x: i16,",
        "    pub y: i16,",
        "    pub z: i16,",
        "    /// Bank-$00 ShapeHdr address installed as the object +$04 token.",
        "    pub shape: u16,",
        "    /// Exact 24-bit init-strategy target installed in object +$19..+$1B.",
        "    pub strategy: u32,",
        "    pub linked_object: Option<u16>,",
        "}",
        "",
        "/// A retail `delay $1388; jump self` boundary released by the host",
        "/// stage state machine rather than by the map dispatcher itself.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct ExternalPhaseGate {",
        "    pub hold: MapAddress,",
        "    pub parked: MapAddress,",
        "    pub continuation: MapAddress,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct InlineExit {",
        "    pub address: MapAddress,",
        "    /// CPU addresses selected in X when the inline routine RTLs.",
        "    pub continuations: &'static [u16],",
        "}",
        "",
        "/// Fully decoded effect of one reachable opcode-$78 inline block.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum InlineAction {",
        "    Call { target: u32, accumulator: Option<u8>, continuation: u16 },",
        "    WordBits { address: u32, mask: u16, set_bits: bool, continuation: u16 },",
        "    BranchWordBits { address: u32, mask: u16, if_clear: u16, if_set: u16 },",
        "    /// Set bit $40 on each live pilot's linked `$6BEC` record.",
        "    SetPilotLinkedFlag { continuation: u16 },",
        "    /// Select GSU entry $8F44/$8F48 from `$1B9C & $20`.",
        "    SelectGsuProgram { continuation: u16 },",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct InlineProgram {",
        "    pub address: MapAddress,",
        "    pub action: InlineAction,",
        "}",
        "",
        "/// Reset copies `$02:8000..=$FDFF` byte-for-byte here.",
        "pub const WRAM_ENGINE_COPY_ROM_FILE: u32 = 0x010000;",
        "pub const WRAM_ENGINE_COPY_DEST: u32 = 0x7F0000;",
        "pub const WRAM_ENGINE_COPY_LEN: u32 = 0x007E00;",
        "/// Reset copies `$0A:8000..=$CDFF` byte-for-byte here.",
        "pub const WRAM_LOGIC_COPY_ROM_FILE: u32 = 0x050000;",
        "pub const WRAM_LOGIC_COPY_DEST: u32 = 0x7F7E00;",
        "pub const WRAM_LOGIC_COPY_LEN: u32 = 0x004E00;",
        "",
        "/// Translate a RAM-resident strategy/code target back to its retail",
        "/// ROM file offset using the two reset-copy ranges.",
        "pub const fn wram_code_rom_file(address: u32) -> Option<u32> {",
        "    if address >= WRAM_ENGINE_COPY_DEST",
        "        && address < WRAM_ENGINE_COPY_DEST + WRAM_ENGINE_COPY_LEN",
        "    {",
        "        Some(WRAM_ENGINE_COPY_ROM_FILE + address - WRAM_ENGINE_COPY_DEST)",
        "    } else if address >= WRAM_LOGIC_COPY_DEST",
        "        && address < WRAM_LOGIC_COPY_DEST + WRAM_LOGIC_COPY_LEN",
        "    {",
        "        Some(WRAM_LOGIC_COPY_ROM_FILE + address - WRAM_LOGIC_COPY_DEST)",
        "    } else {",
        "        None",
        "    }",
        "}",
        "",
    ]

    lines.append(f"pub const SCRIPT_ROOT_COUNT: usize = {len(extraction.roots)};")
    lines.append("pub static SCRIPT_ROOTS: [ScriptRoot; SCRIPT_ROOT_COUNT] = [")
    for root in extraction.roots:
        installed = ("None" if root.installed_at_file is None else
                     f"Some(0x{root.installed_at_file:06X})")
        lines.append(
            f"    ScriptRoot {{ address: {_address(root.address)}, "
            f"installed_at_file: {installed} }},"
        )
    lines += ["];", ""]

    lines.append(f"pub const EXTERNAL_PHASE_GATE_COUNT: usize = {len(extraction.phase_gates)};")
    lines.append(
        "pub static EXTERNAL_PHASE_GATES: "
        "[ExternalPhaseGate; EXTERNAL_PHASE_GATE_COUNT] = ["
    )
    for gate in extraction.phase_gates:
        lines.append(
            f"    ExternalPhaseGate {{ hold: {_address(gate.hold)}, "
            f"parked: {_address(gate.parked)}, "
            f"continuation: {_address(gate.continuation)} }},"
        )
    lines += ["];", ""]

    programs = sorted(extraction.inline_actions.items())
    lines.append(f"pub const INLINE_PROGRAM_COUNT: usize = {len(programs)};")
    lines.append("pub static INLINE_PROGRAMS: [InlineProgram; INLINE_PROGRAM_COUNT] = [")
    for address, action in programs:
        if isinstance(action, InlineCall):
            accumulator = (
                "None" if action.accumulator is None else f"Some(0x{action.accumulator:02X})"
            )
            encoded = (
                "InlineAction::Call { "
                f"target: 0x{action.target:06X}, accumulator: {accumulator}, "
                f"continuation: 0x{0x8000 + action.continuation:04X} }}"
            )
        elif isinstance(action, InlineWordBits):
            encoded = (
                "InlineAction::WordBits { "
                f"address: 0x{action.address:06X}, mask: 0x{action.mask:04X}, "
                f"set_bits: {str(action.set_bits).lower()}, "
                f"continuation: 0x{0x8000 + action.continuation:04X} }}"
            )
        elif isinstance(action, InlineBranchWordBits):
            encoded = (
                "InlineAction::BranchWordBits { "
                f"address: 0x{action.address:06X}, mask: 0x{action.mask:04X}, "
                f"if_clear: 0x{0x8000 + action.if_clear:04X}, "
                f"if_set: 0x{0x8000 + action.if_set:04X} }}"
            )
        elif isinstance(action, InlineSetPilotLinkedFlag):
            encoded = (
                "InlineAction::SetPilotLinkedFlag { "
                f"continuation: 0x{0x8000 + action.continuation:04X} }}"
            )
        elif isinstance(action, InlineSelectGsuProgram):
            encoded = (
                "InlineAction::SelectGsuProgram { "
                f"continuation: 0x{0x8000 + action.continuation:04X} }}"
            )
        else:
            raise TypeError(f"unsupported inline action {action!r}")
        lines.append(
            f"    InlineProgram {{ address: {_address(address)}, action: {encoded} }},"
        )
    lines += ["];", ""]

    lines.append(f"pub const MAP_COMMAND_COUNT: usize = {len(extraction.commands)};")
    lines.append("pub static MAP_COMMANDS: [MapCommand; MAP_COMMAND_COUNT] = [")
    for command in extraction.commands:
        raw = bytes.fromhex(command.raw_hex)
        padded = raw + bytes(16 - len(raw))
        values = ", ".join(f"0x{b:02X}" for b in padded)
        lines.append(
            f"    MapCommand {{ address: {_address(command.address)}, "
            f"opcode: 0x{command.opcode:02X}, size: {command.size}, "
            f"raw_len: {len(raw)}, raw: [{values}] }},"
        )
    lines += ["];", ""]

    lines.append(f"pub const SPAWN_RECORD_COUNT: usize = {len(extraction.spawns)};")
    lines.append("pub static SPAWN_RECORDS: [SpawnRecord; SPAWN_RECORD_COUNT] = [")
    for spawn in extraction.spawns:
        linked = "None" if spawn.linked_object is None else f"Some(0x{spawn.linked_object:04X})"
        lines.append(
            f"    SpawnRecord {{ address: {_address(spawn.address)}, "
            f"opcode: 0x{spawn.opcode:02X}, delay: {spawn.delay}, "
            f"x: {spawn.x}, y: {spawn.y}, z: {spawn.z}, "
            f"shape: 0x{spawn.shape:04X}, strategy: 0x{spawn.strategy:06X}, "
            f"linked_object: {linked} }},"
        )
    lines += ["];", ""]

    inline = sorted(extraction.inline_exits.items())
    for i, (_, exits) in enumerate(inline):
        values = ", ".join(f"0x{0x8000 + value:04X}" for value in exits)
        lines.append(f"static INLINE_CONTINUATIONS_{i}: [u16; {len(exits)}] = [{values}];")
    lines.append("")
    lines.append(f"pub const INLINE_EXIT_COUNT: usize = {len(inline)};")
    lines.append("pub static INLINE_EXITS: [InlineExit; INLINE_EXIT_COUNT] = [")
    for i, (address, _) in enumerate(inline):
        lines.append(
            f"    InlineExit {{ address: {_address(address)}, "
            f"continuations: &INLINE_CONTINUATIONS_{i} }},"
        )
    lines += ["];", ""]

    out = os.path.join(RUST_SRC, "map.rs")
    with open(out, "w") as f:
        f.write("\n".join(lines))
    print(
        f"  map.rs: {len(extraction.roots)} roots, "
        f"{len(extraction.commands)} commands, {len(extraction.spawns)} spawns, "
        f"{len(inline)} typed inline routines"
    )


def extract(rom: bytes):
    extraction = MapExtractor(rom).extract()
    emit_rust(extraction)
    return extraction


if __name__ == "__main__":
    from rom import load_rom

    extract(load_rom())
