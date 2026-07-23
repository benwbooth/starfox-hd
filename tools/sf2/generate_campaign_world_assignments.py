#!/usr/bin/env python3
"""Generate semantic SF2 campaign-world assignments from the retail ROM."""

from __future__ import annotations

import argparse
from pathlib import Path

from rom import load_rom
from verify_difficulty_profiles import (
    WORLD_NAMES_BY_SELECTION,
    assignment_rows,
    profile,
    verify_assignment_structure,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "campaign_world_assignments.rs"
)


def rust_name(name: str) -> str:
    return "".join(word.title() for word in name.split("_"))


def rust_row(row: tuple[int, ...]) -> str:
    worlds = [
        f"CampaignWorld::{rust_name(WORLD_NAMES_BY_SELECTION[selection])}"
        for selection in row
    ]
    if len(worlds) <= 2:
        return f"    [{', '.join(worlds)}],"
    return "\n".join(["    [", *(f"        {world}," for world in worlds), "    ],"])


def rust_source(rom: bytes) -> str:
    normal_profile = profile(rom, 0)
    hard_profile = profile(rom, 1)
    expert_profile = profile(rom, 2)
    normal_rows = assignment_rows(
        rom,
        0,
        normal_profile.planetary_defense_units,
    )
    hard_rows = assignment_rows(
        rom,
        1,
        hard_profile.planetary_defense_units,
    )
    expert_rows = assignment_rows(
        rom,
        2,
        expert_profile.planetary_defense_units,
    )
    verify_assignment_structure("normal", normal_profile, normal_rows)
    verify_assignment_structure("hard", hard_profile, hard_rows)
    verify_assignment_structure("expert", expert_profile, expert_rows)

    hard_occupied = tuple(
        row[: hard_profile.occupied_planets] for row in hard_rows
    )
    expert_occupied = tuple(
        row[: expert_profile.occupied_planets] for row in expert_rows
    )
    if hard_occupied != expert_occupied:
        raise SystemExit("Hard and Expert occupied-world choices do not align")

    lines = [
        "//! Generated semantic campaign-world assignments from the retail ROM.",
        "//!",
        "//! Source-machine addresses and selection ordinals remain in the generator.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_campaign_world_assignments.py [--check]`.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]",
        "pub enum CampaignWorld {",
    ]
    lines.extend(f"    {rust_name(name)}," for name in WORLD_NAMES_BY_SELECTION)
    lines.extend(
        [
            "}",
            "",
            f"pub const CAMPAIGN_WORLD_COUNT: usize = {len(WORLD_NAMES_BY_SELECTION)};",
            f"pub const MAX_OCCUPIED_WORLD_COUNT: usize = {hard_profile.occupied_planets};",
            f"pub(super) const NORMAL_OCCUPIED_WORLD_COUNT: usize = {normal_profile.occupied_planets};",
            f"pub(super) const NORMAL_CAMPAIGN_ASSIGNMENT_COUNT: usize = {len(normal_rows)};",
            f"pub(super) const THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT: usize = {len(hard_occupied)};",
            "",
            "impl CampaignWorld {",
            "    pub const ALL: [Self; CAMPAIGN_WORLD_COUNT] = [",
        ]
    )
    lines.extend(
        f"        Self::{rust_name(name)}," for name in WORLD_NAMES_BY_SELECTION
    )
    lines.extend(
        [
            "    ];",
            "}",
            "",
            "pub(super) const NORMAL_CAMPAIGN_WORLD_ASSIGNMENTS: [[CampaignWorld; NORMAL_OCCUPIED_WORLD_COUNT];",
            "    NORMAL_CAMPAIGN_ASSIGNMENT_COUNT] = [",
        ]
    )
    lines.extend(rust_row(row) for row in normal_rows)
    lines.extend(
        [
            "];",
            "",
            "pub(super) const THREE_WORLD_CAMPAIGN_ASSIGNMENTS: [[CampaignWorld; MAX_OCCUPIED_WORLD_COUNT];",
            "    THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT] = [",
        ]
    )
    lines.extend(rust_row(row) for row in hard_occupied)
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    generated = rust_source(load_rom())
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"{action} {args.output}: "
        f"{len(WORLD_NAMES_BY_SELECTION)} worlds, 6 normal assignments, "
        "20 Hard/Expert assignments"
    )


if __name__ == "__main__":
    main()
