#!/usr/bin/env python3
"""Source-only SF2 path backlog: exact roots, coverage and first blockers.

No CPU execution, trace fixtures or gameplay recordings are used. A registered
complete graph is not proof that the shipping scheduler executes it. Indexed
and dynamic installers outside discover_roots() remain a separate inventory.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from generate_native_paths import (
    DEFAULT_ROM, ROOTS, SUBROUTINES, PathExtractor, UnsupportedPath, graph,
    lower_graph,
)


def inventory(rom: bytes) -> dict:
    extractor = PathExtractor(rom)
    registered = {root: name for name, root in ROOTS}
    discovered = set(extractor.discover_roots())
    entries = tuple(ROOTS) + tuple((name, root) for name, root, _, _ in SUBROUTINES)
    covered = {
        command.address
        for _, root in entries
        for command in graph(extractor, root)
    }
    rows = []
    for root in sorted(discovered, key=lambda address: address.offset):
        commands = graph(extractor, root)
        try:
            _, statements = lower_graph(extractor, root, 0)
        except UnsupportedPath as error:
            failure = str(error)
            lowered_count = None
        else:
            failure = None
            lowered_count = len(statements)
        rows.append({
            "root": root.label(),
            "registered_name": registered.get(root),
            "status": ("blocked" if failure else "registered" if root in registered
                       else "lowerable_unregistered"),
            "source_commands": len(commands),
            "commands_in_registered_graphs": sum(c.address in covered for c in commands),
            "source_sha256": hashlib.sha256(bytes.fromhex(
                "".join(c.raw_hex for c in commands)
            )).hexdigest(),
            "lowered_statements": lowered_count,
            "first_blocker": failure,
        })
    counts = Counter(row["status"] for row in rows)
    return {
        "schema_version": 1,
        "scope": "independently discovered actor roots; not whole-ROM or shipping integration proof",
        "source_rom_sha256": hashlib.sha256(rom).hexdigest(),
        "discovered_roots": len(rows),
        "registered_discovered_roots": counts["registered"],
        "lowerable_unregistered_roots": counts["lowerable_unregistered"],
        "blocked_roots": counts["blocked"],
        "additional_registered_roots": [
            {"root": root.label(), "name": name}
            for name, root in ROOTS if root not in discovered
        ],
        "callable_helpers": len(SUBROUTINES),
        "roots": rows,
        "discovered_catalog_complete": all(row["status"] == "registered" for row in rows),
        "shipping_integration": "not established by this report",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rom", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--check", action="store_true",
                        help="fail unless every independently discovered root is registered and lowers")
    args = parser.parse_args()
    result = inventory(args.rom.read_bytes())
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"Discovered roots: {result['discovered_roots']}; registered: {result['registered_discovered_roots']}; "
              f"blocked: {result['blocked_roots']}; lowerable but unregistered: {result['lowerable_unregistered_roots']}")
        for row in result["roots"]:
            if row["status"] != "registered":
                print(f"{row['root']} ({row['source_commands']} source commands): "
                      f"{row['first_blocker'] or 'complete graph needs reviewed registration'}")
        print("This is source-catalog coverage, not shipping integration or whole-game accuracy.")
    return int(args.check and not result["discovered_catalog_complete"])


if __name__ == "__main__":
    raise SystemExit(main())
