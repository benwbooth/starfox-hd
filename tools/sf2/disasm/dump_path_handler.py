#!/usr/bin/env python3
"""Print complete CFG-derived disassemblies of retail SF2 path handlers."""

from __future__ import annotations

import argparse
from pathlib import Path

import cpu65816 as cpu
from compare_path_handlers import decode_body
from extract_map import DEFAULT_ROM
from extract_path import PathExtractor
from path_semantics import PATH_SEMANTIC_BY_OPCODE


def parse_opcode(text: str) -> int:
    return int(text, 16)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "opcodes", nargs="+", type=parse_opcode, help="logical hexadecimal opcodes"
    )
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument(
        "--local",
        action="store_true",
        help="print only the handler-local CFG; keep calls and terminal jumps opaque",
    )
    args = parser.parse_args()

    extractor = PathExtractor(args.rom.read_bytes())
    for opcode in args.opcodes:
        handler = extractor.analyze_handler(opcode)
        semantic = PATH_SEMANTIC_BY_OPCODE.get(opcode)
        name = semantic.rust_name if semantic is not None else "unreviewed"
        effects = ", ".join(
            effect.kind
            + (f"({effect.value})" if effect.value is not None else "")
            + ("+yield" if effect.yields else "")
            for effect in handler.effects
        )
        print(
            f"\n=== ${opcode:03X} {name} handler=${handler.handler_address:06X} "
            f"effects=[{effects}] ==="
        )
        if args.local:
            instructions = (
                item.instruction
                for item in decode_body(
                    handler.handler_address,
                    extractor._decode_runtime,
                    extractor._control_target,
                )
            )
        else:
            instructions = extractor.handler_instructions(opcode)
        for instruction in instructions:
            print(cpu.fmt_insn(instruction))
        if handler.unresolved_targets:
            targets = ", ".join(f"${target:06X}" for target in handler.unresolved_targets)
            print(f"unresolved: {targets}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
