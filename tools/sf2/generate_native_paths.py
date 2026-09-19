#!/usr/bin/env python3
"""Lower reviewed complete source path graphs into typed native catalogs.

No encoded operands or source-address lookup enter gameplay. The allow-list
deliberately starts with one complete graph; unsupported statements abort the
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
ROOTS = (("ALTERNATE_EXHAUST", PathAddress(0xF536)),)
SEMANTICS = {entry.opcode: entry for entry in PATH_SEMANTICS}


class UnsupportedPath(ValueError):
    pass


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


def lower_graph(extractor: PathExtractor, root: PathAddress, path_index: int):
    commands = graph(extractor, root)
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

        if name == "Sprite":
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


def generate(rom: bytes) -> str:
    extractor = PathExtractor(rom)
    discovered = set(extractor.discover_roots())
    declarations = []
    paths = []
    command_count = 0
    for path_index, (name, root) in enumerate(ROOTS):
        if root not in discovered:
            raise UnsupportedPath(f"{name} has no verified source installer")
        entry, statements = lower_graph(extractor, root, path_index)
        declarations.append(f"pub const {name}: PathCursor = cursor({path_index}, {entry});")
        paths.append("vec![" + ",\n".join(statements) + "]")
        command_count += len(statements)
    source = """// @generated by tools/sf2/generate_native_paths.py; do not edit.
//! Complete statically lowered source paths. This is an explicit subset,
//! not a fallback catalog for paths that have not been ported.
use super::path_appearance::{AnimationChannel, AnimationCommand};
use super::path_commands::ControlCommand;
use super::path_program::{PathCatalog, Statement};
use super::{PathCursor, PathId};

const fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor { path: PathId::from_catalog_index(path), command_index }
}
"""
    source += "\n".join(declarations)
    source += f"\npub const LOWERED_ROOT_COUNT: usize = {len(ROOTS)};"
    source += f"\npub const LOWERED_COMMAND_COUNT: usize = {command_count};"
    source += "\npub fn catalog() -> PathCatalog {\nPathCatalog::new(vec!["
    source += ",\n".join(paths)
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
