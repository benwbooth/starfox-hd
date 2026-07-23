#!/usr/bin/env python3
"""Generate semantic SF2 campaign-world assignments from the retail ROM."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
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
DEFAULT_ENTRY_TRACE = Path(__file__).with_name("fixtures") / "campaign_world_entries.trace"
AUDIO_PROGRAM_TABLE = 0x1E495
AUDIO_PROGRAM_TABLE_END = 0x1E724
WORLD_AUDIO_PROGRAM_RECORDS = (0x062, 0x076, 0x08A, 0x09E, 0x0B2, 0x0C3)


@dataclass(frozen=True)
class EntryEvidence:
    mission_selection: int
    setup_map: int
    active_map: int
    player: tuple[int, int, int]
    audio_program: int
    entry_trace_sha256: str
    audio_trace_sha256: str


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def audio_program_records(rom: bytes) -> set[int]:
    records = set()
    record = 0
    while AUDIO_PROGRAM_TABLE + record < AUDIO_PROGRAM_TABLE_END:
        records.add(record)
        upload_count = rom[AUDIO_PROGRAM_TABLE + record]
        record += 2 + upload_count * 3
    if AUDIO_PROGRAM_TABLE + record != AUDIO_PROGRAM_TABLE_END:
        raise SystemExit("retail audio program table does not end on a record boundary")
    return records


def load_entry_evidence(trace: Path, rom: bytes) -> dict[str, EntryEvidence]:
    entries: dict[str, EntryEvidence] = {}
    for line in trace.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        if not line.startswith("entry "):
            raise SystemExit(f"unknown campaign-world entry record: {line}")
        values = fields(line)
        entries[values["name"]] = EntryEvidence(
            mission_selection=int(values["mission_selection"]),
            setup_map=int(values["setup_map"], 16),
            active_map=int(values["active_map"], 16),
            player=tuple(int(value) for value in values["player"].split(",")),
            audio_program=int(values["audio_program"], 16),
            entry_trace_sha256=values["entry_trace_sha256"],
            audio_trace_sha256=values["audio_trace_sha256"],
        )

    expected_names = {"venom", "macbeth", "meteor", "fortuna"}
    if entries.keys() != expected_names:
        raise SystemExit("campaign-world entry fixture is incomplete")
    if len({entry.setup_map for entry in entries.values()}) != len(entries):
        raise SystemExit("campaign-world setup maps must be distinct")
    if len({entry.active_map for entry in entries.values()}) != len(entries):
        raise SystemExit("campaign-world active maps must be distinct")
    if len({entry.audio_program for entry in entries.values()}) != len(entries):
        raise SystemExit("campaign-world audio programs must be distinct")
    retail_audio_programs = audio_program_records(rom)
    for name, entry in entries.items():
        if WORLD_NAMES_BY_SELECTION[entry.mission_selection] != name:
            raise SystemExit(f"{name} entry disagrees with its retail map label")
        if len(entry.player) != 3:
            raise SystemExit(f"{name} entry has an incomplete player position")
        if entry.audio_program != WORLD_AUDIO_PROGRAM_RECORDS[entry.mission_selection]:
            raise SystemExit(f"{name} entry disagrees with the retail world-audio sequence")
        if entry.audio_program not in retail_audio_programs:
            raise SystemExit(f"{name} audio program is not a retail record")
        for digest in (entry.entry_trace_sha256, entry.audio_trace_sha256):
            invalid_character = any(
                character not in "0123456789abcdef" for character in digest
            )
            if len(digest) != 64 or invalid_character:
                raise SystemExit(f"{name} entry has an invalid trace digest")
    return entries


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


def rust_source(rom: bytes, entry_trace: Path) -> str:
    entry_evidence = load_entry_evidence(entry_trace, rom)
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
        f"//! Missing-world entry evidence: `{entry_trace.name}` ({len(entry_evidence)} missions).",
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
    parser.add_argument("--entry-trace", type=Path, default=DEFAULT_ENTRY_TRACE)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    generated = rust_source(load_rom(), args.entry_trace)
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
        "20 Hard/Expert assignments, 4 distinct missing-world entries"
    )


if __name__ == "__main__":
    main()
