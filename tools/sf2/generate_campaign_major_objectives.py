#!/usr/bin/env python3
"""Generate typed campaign-objective constants from semantic oracle evidence."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "campaign_major_objectives.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "campaign_major_objectives.rs"
)


@dataclass(frozen=True)
class Target:
    mission_selection: int
    required_visits: int


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def load(
    trace: Path,
) -> tuple[dict[str, Target], dict[tuple[str, str], int], dict[tuple[str, str], int]]:
    targets: dict[str, Target] = {}
    objectives: dict[tuple[str, str], int] = {}
    samples: dict[tuple[str, str], int] = {}
    for line in trace.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        values = fields(line)
        kind = line.split(maxsplit=1)[0]
        if kind == "target":
            targets[values["name"]] = Target(
                mission_selection=int(values["mission_selection"]),
                required_visits=int(values["required_visits"]),
            )
        elif kind == "objective":
            objectives[(values["target"], values["name"])] = int(values["count"])
        elif kind == "sample":
            samples[(values["target"], values["name"])] = int(values["elapsed"])
        else:
            raise SystemExit(f"unknown fixture record: {line}")

    if targets.keys() != {"titania", "eladard", "battle_carrier"}:
        raise SystemExit("campaign target fixture is incomplete")
    if objectives != {
        ("titania", "surface_switch"): 2,
        ("titania", "reactor"): 1,
    }:
        raise SystemExit("Titania objective fixture is incomplete")
    required_samples = {
        ("titania", "mission_start"),
        ("titania", "base_entry"),
        ("titania", "interior"),
        ("titania", "reactor"),
        ("titania", "return_flight"),
        ("titania", "map_ready"),
        ("battle_carrier", "mission_start"),
        ("battle_carrier", "map_ready"),
    }
    if samples.keys() != required_samples:
        raise SystemExit("campaign objective timing fixture is incomplete")
    if targets["battle_carrier"].required_visits != 2:
        raise SystemExit("the campaign requires two distinct carrier visits")
    return targets, objectives, samples


def rust_source(
    trace: Path,
    targets: dict[str, Target],
    objectives: dict[tuple[str, str], int],
    samples: dict[tuple[str, str], int],
) -> str:
    titania_start = samples[("titania", "mission_start")]
    values = {
        "TITANIA_MISSION_SELECTION": targets["titania"].mission_selection,
        "ELADARD_MISSION_SELECTION": targets["eladard"].mission_selection,
        "BATTLE_CARRIER_MISSION_SELECTION": targets["battle_carrier"].mission_selection,
        "BATTLE_CARRIER_REQUIRED_VISITS": targets["battle_carrier"].required_visits,
        "TITANIA_SURFACE_SWITCH_COUNT": objectives[("titania", "surface_switch")],
        "TITANIA_REACTOR_COUNT": objectives[("titania", "reactor")],
        "TITANIA_BASE_ENTRY_RETAIL_FRAME": samples[("titania", "base_entry")]
        - titania_start,
        "TITANIA_INTERIOR_RETAIL_FRAME": samples[("titania", "interior")]
        - titania_start,
        "TITANIA_REACTOR_RETAIL_FRAME": samples[("titania", "reactor")]
        - titania_start,
        "TITANIA_RETURN_RETAIL_FRAME": samples[("titania", "return_flight")]
        - titania_start,
        "TITANIA_MAP_READY_RETAIL_FRAME": samples[("titania", "map_ready")]
        - titania_start,
    }
    lines = [
        "//! Generated typed constants for the retail campaign's major objectives.",
        "//!",
        f"//! Source: `{trace.name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_campaign_major_objectives.py [--check]`.",
        "",
    ]
    for name, value in values.items():
        rust_type = "usize" if name.endswith("_COUNT") else "u16"
        lines.append(f"pub(super) const {name}: {rust_type} = {value:_};")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    targets, objectives, samples = load(args.trace)
    generated = rust_source(args.trace, targets, objectives, samples)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(f"{action} {args.output}: 3 targets, {len(objectives)} Titania objectives")


if __name__ == "__main__":
    main()
