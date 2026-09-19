#!/usr/bin/env python3
"""Inventory SF2 source coverage without running original game code.

Success means the discovered script graphs and generated counts agree, not that
the shipping port implements every game behavior. No emulator, save state,
trace fixture, or generated gameplay recording is loaded by this tool.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).with_name("disasm")))
from extract_map import DEFAULT_ROM, InlineCall, MapExtractor
from extract_path import PathExtractor
from path_semantics import PATH_SEMANTICS
from generate_native_paths import OUTPUT as NATIVE_PATH_OUTPUT, ROOTS as NATIVE_PATH_ROOTS
from generate_native_paths import generate as generate_native_paths, graph as native_path_graph


def audit(rom_path: Path) -> dict:
    rom = rom_path.read_bytes()
    maps = MapExtractor(rom).extract()
    paths = PathExtractor(rom).extract()
    errors = []
    for name, failures in (
        ("map invalid commands", maps.invalid_opcodes),
        ("map unresolved inline exits", maps.unresolved_inline_exits),
        ("path invalid commands", paths.invalid_opcodes),
        ("path unresolved handlers", paths.unresolved_handlers),
    ):
        if failures:
            errors.append(f"{name}: {len(failures)}")

    generated = (ROOT / "rust/sf2-data/src/path.rs").read_text()
    native_path_extractor = PathExtractor(rom)
    native_path_commands = len({
        command.address
        for _, root in NATIVE_PATH_ROOTS
        for command in native_path_graph(native_path_extractor, root)
    })
    if NATIVE_PATH_OUTPUT.read_text() != generate_native_paths(rom):
        errors.append("generated native path catalog differs from complete-graph lowering")
    for name, actual in (
        ("PATH_ROOT_COUNT", len(paths.roots)),
        ("PATH_HANDLER_COUNT", len(paths.handlers)),
        ("PATH_COMMAND_COUNT", len(paths.commands)),
    ):
        match = re.search(rf"pub const {name}: usize = ([0-9_]+);", generated)
        if match is None or int(match[1].replace("_", "")) != actual:
            errors.append(f"generated {name} differs from extraction ({actual})")

    semantics = {spec.opcode: spec.rust_name for spec in PATH_SEMANTICS}
    missing = sorted(set(paths.handlers) - semantics.keys())
    if missing:
        errors.append(f"path semantic names missing: {missing}")
    for spec in PATH_SEMANTICS:
        handler = paths.handlers.get(spec.opcode)
        if handler is not None and handler.handler_address != spec.handler_address:
            errors.append(f"semantic handler address differs for opcode {spec.opcode:03X}")
    tree = subprocess.run(
        ["cargo", "tree", "--locked", "-p", "sf-app", "-e", "normal,build,features"],
        cwd=ROOT / "rust", check=True, text=True, capture_output=True,
    ).stdout
    excluded = ("sf2-map", "sf2-path", "sf-oracle", "w65c816", "oracle-bridge", "oracle-data")
    present = [name for name in excluded if re.search(rf"\b{re.escape(name)}\b", tree)]
    if present:
        errors.append(f"verification-only dependencies in sf-app: {present}")

    # Source-navigation hints only: this is not a Rust parser or reachability
    # proof. Stop at the existing top-level test module to avoid counting tests.
    game_path = ROOT / "rust/sf2-game/src/native/game.rs"
    source_parts = game_path.read_text().split("#[cfg(test)]\nmod tests", 1)
    if len(source_parts) != 2:
        errors.append("game source test-module boundary changed; review navigation hints")
    game = source_parts[0]
    markers = {
        "neutral_input_branch": "departed_certified_neutral_path",
        "keyframe_interpolation": "interpolated_player_keyframe(",
        "frame_indexed_actions": "::actions(",
        "recorded_spawn_boundary": "descriptor.start_retail_frame",
        "recorded_retirement_boundary": "descriptor.end_retail_frame",
    }
    sites = {
        name: [number for number, line in enumerate(game.splitlines(), 1) if marker in line]
        for name, marker in markers.items()
    }
    calls = Counter(
        action.target for action in maps.inline_actions.values() if isinstance(action, InlineCall)
    )
    opcodes = Counter(command.opcode for command in paths.commands)
    return {
        "schema_version": 1,
        "scope": "discovered map/path graph; not whole-ROM or gameplay parity proof",
        "source_rom_sha256": hashlib.sha256(rom).hexdigest(),
        "static_checks_passed": not errors,
        "errors": errors,
        "shipping_gameplay_complete": "not established; see docs/SF2_GAMEPLAY_COVERAGE.md",
        "map": {
            "roots": len(maps.roots), "commands": len(maps.commands),
            "opcodes": len({command.opcode for command in maps.commands}),
            "inline_routines": len(maps.inline_actions), "spawns": len(maps.spawns),
            "external_phase_gates": len(maps.phase_gates),
            "spawn_initializers": sorted({
                f"{spawn.strategy_bank:02X}:{spawn.strategy_addr:04X}" for spawn in maps.spawns
            }),
            "inline_call_targets": {f"{target:06X}": count for target, count in sorted(calls.items())},
        },
        "path": {
            "roots": len(paths.roots), "commands": len(paths.commands),
            "logical_opcodes": len(paths.handlers),
            "unique_handler_addresses": len({h.handler_address for h in paths.handlers.values()}),
            "handlers": [{
                "opcode": f"{opcode:03X}", "semantic": semantics.get(opcode),
                "source_address": f"{handler.handler_address:06X}",
                "command_count": opcodes[opcode],
                "pointer_effects": sorted({effect.kind for effect in handler.effects}),
                "shipping_equivalence": "not inferred from staging implementation",
            } for opcode, handler in sorted(paths.handlers.items())],
        },
        "shipping_dependency_boundary": {name: name in present for name in excluded},
        "native_path_catalog": {
            "complete_lowered_roots": len(NATIVE_PATH_ROOTS),
            "lowered_statements": native_path_commands,
            "named_roots": [name for name, _ in NATIVE_PATH_ROOTS],
            "caveat": "typed catalog lowering only; Game scheduler and spawn integration remain open",
        },
        "game_source_navigation_hints": {
            "path": str(game_path.relative_to(ROOT)), "lines_by_marker": sites,
            "caveat": "textual hints requiring review, not semantic coverage counts",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom", nargs="?", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    result = audit(args.rom)
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"ROM SHA-256: {result['source_rom_sha256']}")
        for name in ("map", "path"):
            print(f"{name}: " + " ".join(
                f"{key}={value}" for key, value in result[name].items() if isinstance(value, int)
            ))
        print("Excluded from shipping: " + ", ".join(
            name for name, present in result["shipping_dependency_boundary"].items() if not present
        ))
        native = result["native_path_catalog"]
        print(f"Native catalog: roots={native['complete_lowered_roots']} statements={native['lowered_statements']}")
        print(native["caveat"])
        print("Static checks: " + ("PASS" if result["static_checks_passed"] else "FAIL"))
        for error in result["errors"]:
            print(error)
        print("Gameplay completion: " + result["shipping_gameplay_complete"])
    return 0 if result["static_checks_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
