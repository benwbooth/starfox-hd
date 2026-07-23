#!/usr/bin/env python3
"""Emit the mechanically reachable SF2 object-path graph as Rust data.

The ROM/WRAM mapping, dispatcher analysis, and handler CFG traversal live in
``disasm/extract_path.py``. This front-end serializes only fully resolved
results so later semantic decompilation can consume an immutable retail-byte
catalog without reading the user's ROM at run time.
"""

from __future__ import annotations

import os

from disasm.extract_path import PathExtractor
from disasm.path_semantics import PATH_SEMANTICS, PATH_SEMANTIC_BY_OPCODE
from rom import AUTOGEN_HEADER, RUST_SRC


def _address(address) -> str:
    return f"PathAddress {{ offset: 0x{address.offset:04X} }}"


def _effect(effect) -> str:
    kind = {
        "advance": "FlowKind::Advance",
        "jump": "FlowKind::Jump",
        "call": "FlowKind::Call",
        "hold": "FlowKind::Hold",
        "dynamic_jump": "FlowKind::DynamicJump",
        "dynamic_call": "FlowKind::DynamicCall",
        "schedule": "FlowKind::Schedule",
        "relative_schedule": "FlowKind::RelativeSchedule",
        "inline": "FlowKind::Inline",
        "return": "FlowKind::Return",
        "trap": "FlowKind::Trap",
    }.get(effect.kind)
    if kind is None:
        raise ValueError(f"unsupported path effect {effect!r}")
    value = "None" if effect.value is None else f"Some({effect.value})"
    return (
        f"FlowEffect {{ kind: {kind}, value: {value}, "
        f"yields: {str(effect.yields).lower()}, "
        f"resets_counter: {str(effect.resets_counter).lower()} }}"
    )


def emit_rust(extraction) -> None:
    if extraction.invalid_opcodes or extraction.unresolved_handlers:
        raise RuntimeError(
            "refusing to generate ambiguous path data: "
            f"invalid={extraction.invalid_opcodes}, "
            f"unresolved_handlers={extraction.unresolved_handlers}"
        )

    for spec in PATH_SEMANTICS:
        handler = extraction.handlers.get(spec.opcode)
        if handler is None or handler.handler_address != spec.handler_address:
            actual = None if handler is None else handler.handler_address
            raise RuntimeError(
                f"semantic {spec.rust_name} no longer matches its retail handler: "
                f"expected ${spec.handler_address:06X}, got {actual!r}"
            )

    lines = [AUTOGEN_HEADER.format(tool="extract_path.py")]
    lines += [
        "//! Reachable SF2 object-path bytecode recovered from the retail dispatcher.",
        "//!",
        "//! Roots are exact operands of reachable map opcode `$8C` plus signature-checked",
        "//! native 65816 object-path installers. Handler effects",
        "//! come from CFG traversal of the reset-time `$0A` ROM to `$7F` WRAM copy,",
        "//! including the zero-prefixed extended dispatch and its high-slot alias.",
        "//! No scan-only candidates or guessed record boundaries are included.",
        "//! Semantic names are an explicit proof-gated catalog. Every reachable",
        "//! handler is named and validated against its extracted retail address; a",
        "//! semantic identity alone does not claim that every downstream service is native.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]",
        "pub struct PathAddress {",
        "    /// Offset in the full 64 KiB Super FX ROM bank `$44`.",
        "    pub offset: u16,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum FlowKind {",
        "    Advance,",
        "    Jump,",
        "    Call,",
        "    Hold,",
        "    DynamicJump,",
        "    DynamicCall,",
        "    Schedule,",
        "    RelativeSchedule,",
        "    Inline,",
        "    Return,",
        "    Trap,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct FlowEffect {",
        "    pub kind: FlowKind,",
        "    /// Byte advance or path-word operand offset, when applicable.",
        "    pub value: Option<u8>,",
        "    pub yields: bool,",
        "    pub resets_counter: bool,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum PathSemantic {",
    ]
    for spec in PATH_SEMANTICS:
        lines.append(f"    {spec.rust_name},")
    lines += [
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct PathHandler {",
        "    pub opcode: u16,",
        "    pub address: u32,",
        "    /// Reviewed semantic identity, or `None` when proof is incomplete.",
        "    pub semantic: Option<PathSemantic>,",
        "    pub effects: &'static [FlowEffect],",
        "    pub instruction_count: u16,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct PathCommand {",
        "    pub address: PathAddress,",
        "    /// Logical opcode: `$000..$0FF`, or `$100..$1FF` after a zero escape.",
        "    pub opcode: u16,",
        "    pub prefix_size: u8,",
        "    pub handler_address: u32,",
        "    pub raw_len: u8,",
        "    pub raw: [u8; 18],",
        "    pub effects: &'static [FlowEffect],",
        "    pub successors: &'static [PathAddress],",
        "}",
        "",
        "pub const PATH_DATA_ROM_FILE: u32 = 0x040000;",
        "pub const PATH_CPU_BANK: u8 = 0x44;",
        "pub const PATH_DISPATCH_PRIMARY: u32 = 0x7F7EE8;",
        "pub const PATH_DISPATCH_EXTENDED: u32 = 0x7F82E8;",
        "",
    ]

    lines.append(f"pub const PATH_ROOT_COUNT: usize = {len(extraction.roots)};")
    roots = ", ".join(_address(root) for root in extraction.roots)
    lines.append(f"pub static PATH_ROOTS: [PathAddress; PATH_ROOT_COUNT] = [{roots}];")
    lines.append("")

    handlers = sorted(extraction.handlers.items())
    for index, (_, handler) in enumerate(handlers):
        values = ", ".join(_effect(effect) for effect in handler.effects)
        lines.append(
            f"static PATH_EFFECTS_{index}: [FlowEffect; {len(handler.effects)}] = [{values}];"
        )
    lines.append("")
    lines.append(f"pub const PATH_HANDLER_COUNT: usize = {len(handlers)};")
    lines.append("pub static PATH_HANDLERS: [PathHandler; PATH_HANDLER_COUNT] = [")
    handler_index = {}
    for index, (opcode, handler) in enumerate(handlers):
        handler_index[opcode] = index
        semantic = PATH_SEMANTIC_BY_OPCODE.get(opcode)
        semantic_value = (
            "None" if semantic is None
            else f"Some(PathSemantic::{semantic.rust_name})"
        )
        lines.append(
            "    PathHandler { "
            f"opcode: 0x{opcode:03X}, address: 0x{handler.handler_address:06X}, "
            f"semantic: {semantic_value}, "
            f"effects: &PATH_EFFECTS_{index}, "
            f"instruction_count: {len(handler.instruction_addresses)} }},"
        )
    lines += ["];", ""]

    for index, command in enumerate(extraction.commands):
        successors = ", ".join(_address(address) for address in command.successors)
        lines.append(
            f"static PATH_SUCCESSORS_{index}: [PathAddress; {len(command.successors)}] = "
            f"[{successors}];"
        )
    lines.append("")
    lines.append(f"pub const PATH_COMMAND_COUNT: usize = {len(extraction.commands)};")
    lines.append("pub static PATH_COMMANDS: [PathCommand; PATH_COMMAND_COUNT] = [")
    for index, command in enumerate(extraction.commands):
        raw = bytes.fromhex(command.raw_hex)
        if len(raw) > 18:
            raise ValueError(f"path command at {command.address.label()} exceeds 18 bytes")
        padded = raw + bytes(18 - len(raw))
        values = ", ".join(f"0x{byte:02X}" for byte in padded)
        effects_index = handler_index[command.opcode]
        lines.append(
            "    PathCommand { "
            f"address: {_address(command.address)}, opcode: 0x{command.opcode:03X}, "
            f"prefix_size: {command.prefix_size}, "
            f"handler_address: 0x{command.handler_address:06X}, "
            f"raw_len: {len(raw)}, raw: [{values}], "
            f"effects: &PATH_EFFECTS_{effects_index}, "
            f"successors: &PATH_SUCCESSORS_{index} }},"
        )
    lines += ["];", ""]

    out = os.path.join(RUST_SRC, "path.rs")
    with open(out, "w") as file:
        file.write("\n".join(lines))
    print(
        f"  path.rs: {len(extraction.roots)} roots, "
        f"{len(extraction.commands)} commands, {len(handlers)} handlers"
    )


def extract(rom: bytes):
    extraction = PathExtractor(rom).extract()
    emit_rust(extraction)
    return extraction


if __name__ == "__main__":
    from rom import load_rom

    extract(load_rom())
