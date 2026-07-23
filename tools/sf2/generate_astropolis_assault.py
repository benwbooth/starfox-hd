#!/usr/bin/env python3
"""Generate typed Astropolis constants from the semantic oracle fixture."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "astropolis_assault.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "astropolis_assault.rs"
)


@dataclass(frozen=True)
class Objective:
    count: int
    durability: int


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def load(trace: Path) -> tuple[dict[str, Objective], dict[str, int], dict[str, int]]:
    objectives: dict[str, Objective] = {}
    transitions: dict[str, int] = {}
    ending_samples: dict[str, int] = {}
    for line in trace.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        values = fields(line)
        kind = line.split(maxsplit=1)[0]
        name = values["name"]
        if kind == "objective":
            objectives[name] = Objective(
                count=int(values["count"]), durability=int(values["durability"])
            )
        elif kind == "transition":
            transitions[name] = int(values["elapsed"])
        elif kind == "ending_sample":
            ending_samples[name] = int(values["elapsed"])
        else:
            raise SystemExit(f"unknown fixture record: {line}")
    expected_objectives = {
        "security_turret",
        "core_spike",
        "exposed_cube",
        "mask_eye",
        "final_core",
    }
    expected_transitions = {
        "core_exposed",
        "core_reform_trigger",
        "core_destroyed",
        "ending_handoff",
    }
    if objectives.keys() != expected_objectives:
        raise SystemExit("Astropolis objective fixture is incomplete")
    if transitions.keys() != expected_transitions:
        raise SystemExit("Astropolis transition fixture is incomplete")
    if ending_samples.keys() != {"credits", "end_screen"}:
        raise SystemExit("Astropolis ending fixture is incomplete")
    return objectives, transitions, ending_samples


def rust_source(
    trace: Path,
    objectives: dict[str, Objective],
    transitions: dict[str, int],
    _ending_samples: dict[str, int],
) -> str:
    core_exposure = transitions["core_reform_trigger"] - transitions["core_exposed"]
    destruction = transitions["ending_handoff"] - transitions["core_destroyed"]
    values = {
        "SECURITY_TURRET_DURABILITY": objectives["security_turret"].durability,
        "CORE_SPIKE_COUNT": objectives["core_spike"].count,
        "CORE_SPIKE_DURABILITY": objectives["core_spike"].durability,
        "EXPOSED_CUBE_DURABILITY": objectives["exposed_cube"].durability,
        "MASK_EYE_DURABILITY": objectives["mask_eye"].durability,
        "FINAL_CORE_DURABILITY": objectives["final_core"].durability,
        "CORE_EXPOSURE_RETAIL_FRAMES": core_exposure,
        "CORE_DESTRUCTION_RETAIL_FRAMES": destruction,
    }
    lines = [
        "//! Generated typed constants for the retail Astropolis assault.",
        "//!",
        f"//! Source: `{trace.name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_astropolis_assault.py [--check]`.",
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

    objectives, transitions, ending_samples = load(args.trace)
    generated = rust_source(args.trace, objectives, transitions, ending_samples)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(f"{action} {args.output}: {len(objectives)} objectives, 2 ending samples")


if __name__ == "__main__":
    main()
