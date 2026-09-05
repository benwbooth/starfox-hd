#!/usr/bin/env python3
"""Decode SF2 boot-intro paths from the signature-checked scene installer.

The boot map installs the generic ``$7F:7E00`` strategy, which obtains its
path from the object's ``+$2B`` field.  That field contains a 16-bit offset
into the copied Super FX bank-$44 path data; it is not a Rust animation track.
This tool follows the authored root through :class:`PathExtractor`, then
checks the active path cursors observed by Mesen against that graph. It never
starts disassembling at a sampled mid-instruction cursor. Graph recovery is
not a native scene implementation or a proof of complete game behavior.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from extract_map import DEFAULT_ROM
from extract_path import PathAddress, PathExtractor
from path_semantics import PATH_SEMANTIC_BY_OPCODE


def authored_intro_root(rom: bytes) -> int:
    """Recover the title root from the source's indexed scene installer.

    The live boot write is $06:A96E, not an immediate path installation.
    The selector clamps to 30 records, multiplies by eight, and reads the
    path word from $0D:D4C7. The boot intro selects record six. Both code
    slices are checked before interpreting that table; observed cursors are
    evidence for reachability, never replacement animation data.
    """
    signatures = (
        (0x032938, bytes.fromhex("ad731d29ff00c91e003003a900000a0a0aaa")),
        (0x032968, bytes.fromhex("bfc7d40dfa7a992b00")),
    )
    for offset, expected in signatures:
        if rom[offset:offset + len(expected)] != expected:
            raise ValueError(f"scene installer signature mismatch at file {offset:#x}")
    scene_table_file = 0x06D4C7
    scene_record_size = 8
    intro_scene_index = 6
    offset = scene_table_file + intro_scene_index * scene_record_size
    payload = rom[offset:offset + 2]
    if len(payload) != 2:
        raise ValueError("incomplete scene path table")
    return int.from_bytes(payload, "little")


def trace_paths(trace: Path) -> list[int]:
    """Read paths only from generic path strategies in active-list records."""
    roots: set[int] = set()
    records = 0
    for line in trace.read_text().splitlines():
        marker = " objects="
        if marker not in line:
            continue
        objects = line.split(marker, 1)[1].split(" draws=", 1)[0]
        for record in filter(None, objects.split(";")):
            fields = record.split(",")
            if len(fields) != 9:
                raise ValueError(f"malformed object record: {record!r}")
            try:
                strategy = int(fields[1], 16)
            except ValueError as error:
                raise ValueError(f"malformed strategy in object record: {record!r}") from error
            if strategy not in (0x7F7E00, 0x7F7E53):
                continue
            records += 1
            try:
                offset = int(fields[-1])
            except ValueError as error:
                raise ValueError(f"malformed path in object record: {record!r}") from error
            if not 0 <= offset < 0x10000:
                raise ValueError(f"path outside bank in object record: {record!r}")
            if offset:
                roots.add(offset)
    if records == 0:
        raise ValueError("trace contains no generic active-list path records")
    return sorted(roots)


def decode(rom: Path, roots: list[int]):
    extractor = PathExtractor(rom.read_bytes())
    commands = {}
    work = [PathAddress(root) for root in roots]
    visited = set()
    failures = []
    while work:
        address = work.pop()
        if address in visited:
            continue
        visited.add(address)
        try:
            command = extractor.decode_command(address)
        except (IndexError, ValueError) as error:
            failures.append((address, str(error)))
            continue
        commands[address] = command
        if (extractor.analyze_handler(command.opcode).unresolved_targets
            or any(effect.kind in ("redispatch", "brk", "cop", "stp", "wai")
                   for effect in command.effects)):
            failures.append((address, "unresolved path handler effect"))
        work.extend(command.successors)
        if command.opcode in (0x033, 0x05D, 0x0F5):
            child = PathAddress(extractor.path_word(address, command.prefix_size + 3))
            if child.offset and child not in commands:
                work.append(child)
    return commands, failures


def cursor_is_reached(commands, offset: int) -> bool:
    """The dispatcher consumes an extended opcode's zero escape in place.

    An end-of-frame sample can therefore point at its second opcode byte.
    That byte is not an independent primary-opcode root. Accept only this
    proven prefix adjustment, never arbitrary offsets inside operands.
    """
    if PathAddress(offset) in commands:
        return True
    command = commands.get(PathAddress(offset - 1))
    return command is not None and command.prefix_size == 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--summary", action="store_true", help="omit the per-command listing")
    parser.add_argument("--require-reviewed-semantics", action="store_true")
    args = parser.parse_args()

    observed = trace_paths(args.trace)
    if not observed:
        raise SystemExit("trace contains no nonzero path cursors")
    root = authored_intro_root(args.rom.read_bytes())
    commands, failures = decode(args.rom, [root])
    missing = [offset for offset in observed if not cursor_is_reached(commands, offset)]
    unnamed = sorted({command.opcode for command in commands.values()
                      if command.opcode not in PATH_SEMANTIC_BY_OPCODE})
    print(f"authored_root=44:{root:04X} observed_cursors={len(observed)} "
          f"commands={len(commands)} failures={len(failures)} missing_observed={len(missing)} "
          f"unreviewed_semantics={len(unnamed)}")
    print("observed=" + ",".join(f"44:{offset:04X}" for offset in observed))
    for address in ([] if args.summary else sorted(commands)):
        command = commands[address]
        successors = ",".join(f"44:{item.offset:04X}" for item in command.successors)
        effects = ",".join(effect.kind for effect in command.effects)
        print(f"44:{address.offset:04X} op={command.opcode:03X} raw={command.raw_hex} "
              f"effects={effects} successors={successors}")
    for address, error in failures:
        print(f"UNRESOLVED 44:{address.offset:04X} {error}")
    for offset in missing:
        print(f"UNREACHED_OBSERVED 44:{offset:04X}")
    for opcode in unnamed:
        print(f"UNREVIEWED_SEMANTIC opcode={opcode:03X}")
    if unnamed and args.require_reviewed_semantics:
        return 1
    if failures or missing:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
