#!/usr/bin/env python3
"""Lower reviewed complete source path graphs into typed native catalogs.

No encoded operands or source-address lookup enter gameplay. The allow-list
deliberately contains complete graphs only; unsupported statements abort the
entire generation, rather than inserting placeholders or truncating a graph.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "disasm"))
from extract_path import DEFAULT_ROM, PathAddress, PathCommand, PathExtractor
from path_semantics import PATH_SEMANTICS

REPO = Path(__file__).resolve().parents[2]
OUTPUT = REPO / "rust/sf2-game/src/native/authored_paths.rs"
# Independently installed by source actor strategies, not a scanned candidate.
ROOTS = (
    ("ALTERNATE_EXHAUST", PathAddress(0xF536)),
    ("COLOR_CYCLE_SPRITE", PathAddress(0xF593)),
    ("RANDOMIZED_COLOR_PARTICLE", PathAddress(0xF294)),
    ("LOCAL_JITTER_SPRITE", PathAddress(0xF521)),
    ("AUXILIARY_GATED_SPRITE", PathAddress(0xF36F)),
)
SEMANTICS = {entry.opcode: entry for entry in PATH_SEMANTICS}


class UnsupportedPath(ValueError):
    pass


def word_field(variable: int) -> str:
    # CB47 maps these authored operands to the existing actor words. Numeric
    # encodings stop here: generated gameplay statements name actual fields.
    fields = {
        0x0C: "WordField::Position(Axis::X)",
        0x0E: "WordField::Position(Axis::Y)",
        0x10: "WordField::Position(Axis::Z)",
        0x32: "WordField::Velocity(Axis::X)",
        0x34: "WordField::Velocity(Axis::Y)",
        0x36: "WordField::Velocity(Axis::Z)",
        0x8E: "WordField::RelativePosition(Axis::X)",
        0x90: "WordField::RelativePosition(Axis::Y)",
        0x92: "WordField::RelativePosition(Axis::Z)",
        0xA1: "WordField::MotionPhase",
    }
    if variable not in fields:
        raise UnsupportedPath(f"unported word operand {variable:02X}")
    return fields[variable]


def byte_field(variable: int) -> str:
    if variable == 0x99:
        return "ByteField::TextureScrollX"
    # Each pair aliases one actual typed word; it must not create a separate
    # particle counter or independent byte shadow of the motion phase.
    for base in (0x0C, 0x0E, 0x10, 0x32, 0x34, 0x36, 0x8E, 0x90, 0x92, 0xA1):
        if variable in (base, base + 1):
            part = "Low" if variable == base else "High"
            return f"ByteField::WordPart {{ field: {word_field(base)}, part: BytePart::{part} }}"
    raise UnsupportedPath(f"unported byte operand {variable:02X}")


def graph(extractor: PathExtractor, root: PathAddress) -> list[PathCommand]:
    pending = [root]
    found = {}
    while pending:
        address = pending.pop()
        if address in found:
            continue
        command = extractor.decode_command(address)
        found[address] = command
        pending.extend(command.successors)
    return [found[address] for address in sorted(found)]


def lower_graph(extractor: PathExtractor, root: PathAddress, path_index: int, indices=None):
    commands = graph(extractor, root)
    if indices is None:
        indices = {command.address: index for index, command in enumerate(commands)}

    def cursor(address):
        return f"cursor({path_index}, {indices[address]})"

    statements = []
    for command in commands:
        spec = SEMANTICS.get(command.opcode)
        if spec is None or spec.handler_address != command.handler_address:
            raise UnsupportedPath(f"unreviewed handler at {command.address.label()}")
        name = spec.rust_name
        raw = bytes.fromhex(command.raw_hex)
        # The extractor counts only an optional zero escape in prefix_size;
        # the handler's opcode byte is still present after that prefix.
        operand_start = command.prefix_size + 1

        def parameters(count):
            if len(raw) != operand_start + count:
                raise UnsupportedPath(f"unexpected {name} record size at {command.address.label()}")
            return raw[operand_start:]

        def next_cursor():
            if len(command.successors) != 1:
                raise UnsupportedPath(f"unexpected {name} edges at {command.address.label()}")
            return cursor(command.successors[0])

        def branch_cursors(target):
            fallthrough = PathAddress((command.address.offset + len(raw)) & 0xFFFF)
            destination = PathAddress(target)
            if set(command.successors) != {fallthrough, destination}:
                raise UnsupportedPath(f"unexpected {name} branch edges at {command.address.label()}")
            return cursor(destination), cursor(fallthrough)

        if name == "AddByte":
            variable, amount = parameters(2)
            mutation = f"Mutation::Byte {{ field: {byte_field(variable)}, operation: ByteOperation::Add(ByteOperand::Literal({amount})) }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name == "IfSelectedAuxiliaryContinuation":
            low, high = parameters(2)
            taken, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::SelectedAuxiliaryBranch {{ taken: {taken}, next: {next_} }}"
        elif name == "Gosub":
            low, high = parameters(2)
            target, next_ = branch_cursors(low | (high << 8))
            statement = f"Statement::Control(ControlCommand::Call {{ target: {target}, next: {next_} }})"
        elif name in ("Goto", "GotoImmediate"):
            low, high = parameters(2)
            target = PathAddress(low | (high << 8))
            if set(command.successors) != {target}:
                raise UnsupportedPath(f"unexpected {name} destination at {command.address.label()}")
            operation = "Goto" if name == "Goto" else "Jump"
            statement = f"Statement::Control(ControlCommand::{operation} {{ target: {cursor(target)} }})"
        elif name == "Return":
            parameters(0)
            if command.successors:
                raise UnsupportedPath(f"RETURN has static outgoing edges at {command.address.label()}")
            statement = "Statement::Control(ControlCommand::Return)"
        elif name == "IfNot":
            parameters(0)
            statement = f"Statement::Branch(BranchCommand::InvertNext {{ next: {next_cursor()} }})"
        elif name in ("SetRandomByte", "SetRandomWord", "AddCenteredRandomByte", "AddCenteredRandomWord"):
            wide = name.endswith("Word")
            operands = parameters(3 if wide else 2)
            field = word_field(operands[0]) if wide else byte_field(operands[0])
            mask = int.from_bytes(operands[1:], "little")
            operation = ("Assign" if name.startswith("Set") else "AddCentered") + ("Word" if wide else "Byte")
            statement = f"Statement::Random {{ mutation: RandomMutation::{operation} {{ field: {field}, mask: {mask} }}, next: {next_cursor()} }}"
        elif name in ("IncrementByte", "DecrementWord"):
            variable, = parameters(1)
            if name == "IncrementByte":
                mutation = f"Mutation::Byte {{ field: {byte_field(variable)}, operation: ByteOperation::Increment }}"
            else:
                mutation = f"Mutation::Word {{ field: {word_field(variable)}, operation: WordOperation::Decrement }}"
            statement = f"Statement::Mutate {{ mutation: {mutation}, next: {next_cursor()} }}"
        elif name == "IfSameByte":
            variable, expected, low, high = parameters(4)
            taken, next_ = branch_cursors(low | (high << 8))
            condition = f"ActorCondition::EqualByte(ByteOperand::Actor({byte_field(variable)}), ByteOperand::Literal({expected}))"
            statement = f"Statement::Compare {{ condition: {condition}, taken: {taken}, next: {next_} }}"
        elif name == "SetObjectBytes0a0b":
            target, amount = parameters(2)
            statement = f"Statement::Motion {{ command: MotionCommand::AccelerateTo {{ target: {target}, amount: {amount} }}, next: {next_cursor()} }}"
        elif name == "DisableCollision":
            parameters(0)
            statement = f"Statement::DisableCollision {{ next: {next_cursor()} }}"
        elif name == "WaitOne":
            parameters(0)
            statement = f"Statement::Control(ControlCommand::WaitOne {{ next: {next_cursor()} }})"
        elif name == "Sprite":
            color, size = parameters(2)
            statement = f"Statement::Sprite {{ color: {color}, size: {size}, next: {next_cursor()} }}"
        elif name == "DoQueue":
            count, = parameters(1)
            statement = f"Statement::Control(ControlCommand::BeginLoop {{ iterations: {count}, next: {next_cursor()} }})"
        elif name in ("InitAnimation", "InitColorAnimation"):
            value, = parameters(1)
            channel = "Shape" if name == "InitAnimation" else "Color"
            statement = f"Statement::Animation {{ command: AnimationCommand::Initialize {{ channel: AnimationChannel::{channel}, value: {value} }}, next: {next_cursor()} }}"
        elif name in ("AddAnimation", "AddColorAnimation"):
            amount, period = parameters(2)
            channel = "Shape" if name == "AddAnimation" else "Color"
            statement = f"Statement::Animation {{ command: AnimationCommand::Advance {{ channel: AnimationChannel::{channel}, amount: {amount}, period: {period} }}, next: {next_cursor()} }}"
        elif name in ("Next", "ImmediateNext"):
            parameters(0)
            immediate = "true" if name == "ImmediateNext" else "false"
            statement = f"Statement::Control(ControlCommand::Next {{ immediate: {immediate}, next: {next_cursor()} }})"
        elif name == "End":
            parameters(0)
            if command.successors:
                raise UnsupportedPath(f"END has outgoing edges at {command.address.label()}")
            statement = "Statement::Control(ControlCommand::End)"
        else:
            raise UnsupportedPath(f"unsupported {name} at {command.address.label()}")
        statements.append(statement)
    return indices[root], statements


def generate(rom: bytes, roots=ROOTS) -> str:
    extractor = PathExtractor(rom)
    discovered = set(extractor.discover_roots())
    declarations = []
    # One shared address-to-semantic-index layout across all lowered roots.
    # Duplicating a callee in each root graph would give the same source path
    # multiple identities, breaking callback cancellation/cursor comparisons.
    addresses = sorted({command.address for _, root in roots for command in graph(extractor, root)})
    indices = {address: index for index, address in enumerate(addresses)}
    unique_statements = {}
    for name, root in roots:
        if root not in discovered:
            raise UnsupportedPath(f"{name} has no verified source installer")
        entry, statements = lower_graph(extractor, root, 0, indices)
        declarations.append(f"pub const {name}: PathCursor = cursor(0, {entry});")
        for command, statement in zip(graph(extractor, root), statements, strict=True):
            previous = unique_statements.setdefault(command.address, statement)
            if previous != statement:
                raise UnsupportedPath(f"inconsistent shared statement at {command.address.label()}")
    source = """// @generated by tools/sf2/generate_native_paths.py; do not edit.
//! Complete statically lowered source paths. This is an explicit subset,
//! not a fallback catalog for paths that have not been ported.
use super::path_appearance::{AnimationChannel, AnimationCommand};
use super::path_commands::{BranchCommand, ControlCommand, MotionCommand};
use super::path_fields::{Axis, ByteField, ByteOperand, ByteOperation, BytePart, Mutation, WordField, WordOperation};
use super::path_program::{ActorCondition, PathCatalog, Statement};
use super::path_random::RandomMutation;
use super::{PathCursor, PathId};

const fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor { path: PathId::from_catalog_index(path), command_index }
}
"""
    source += "\n".join(declarations)
    source += f"\npub const LOWERED_ROOT_COUNT: usize = {len(roots)};"
    source += f"\npub const LOWERED_COMMAND_COUNT: usize = {len(unique_statements)};"
    source += "\npub fn catalog() -> PathCatalog {\nPathCatalog::new(vec!["
    source += "vec![" + ",\n".join(unique_statements[address] for address in addresses) + "]"
    source += "]).expect(\"generated catalog indices fit native cursors\")\n}\n"
    return subprocess.run(
        ["rustfmt", "--edition", "2021", "--emit", "stdout"],
        input=source, text=True, capture_output=True, check=True,
    ).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source = generate(args.rom.read_bytes())
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != source:
            print(f"out of date: {OUTPUT}", file=sys.stderr)
            return 1
    else:
        OUTPUT.write_text(source)
    print(f"Native path catalog: {len(ROOTS)} complete source root(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
