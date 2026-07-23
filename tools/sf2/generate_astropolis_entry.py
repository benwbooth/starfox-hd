#!/usr/bin/env python3
"""Generate the typed neutral-input Astropolis entry path from an oracle trace."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "astropolis_entry_path.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "astropolis_entry.rs"
RETAIL_FRAME_STEP = 4


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def parse_tuple(value: str, length: int, label: str) -> tuple[int, ...]:
    result = tuple(map(int, value.split(",")))
    if len(result) != length:
        raise SystemExit(f"{label} needs {length} values, found {len(result)}")
    return result


def raw_records(trace: Path) -> list[Record]:
    mission_start = None
    records = []
    for line in trace.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if values.get("selection") != "11" or values.get("mode") != "1":
            continue
        elapsed = int(values["elapsed"])
        mission_start = elapsed if mission_start is None else mission_start
        if values.get("event") != "sortie":
            continue
        records.append(
            Record(
                elapsed - mission_start,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
            )
        )
    if not records:
        raise SystemExit("trace has no Astropolis sortie samples")
    expected = list(
        range(records[0].retail_frame, records[-1].retail_frame + 1, RETAIL_FRAME_STEP)
    )
    if [record.retail_frame for record in records] != expected:
        raise SystemExit("Astropolis samples are not a complete four-frame cadence")
    return records


def compact_records(trace: Path) -> list[Record]:
    records = []
    for line in trace.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        records.append(
            Record(
                int(values["retail_frame"]),
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["player"], 7, "player pose"),
                parse_tuple(values["wingmate"], 7, "wingmate pose"),
            )
        )
    if not records:
        raise SystemExit("compact Astropolis fixture is empty")
    return records


def load(trace: Path) -> list[Record]:
    first_content = next(
        (
            line
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line and not line.startswith("#")
        ),
        "",
    )
    return compact_records(trace) if first_content.startswith("retail_frame=") else raw_records(trace)


def write_compact(source: Path, output: Path, records: list[Record]) -> None:
    lines = [
        "# Complete four-retail-frame cadence for the clean Astropolis entry path.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# first_retail_frame={records[0].retail_frame}",
        f"# last_retail_frame={records[-1].retail_frame}",
    ]
    for record in records:
        lines.append(
            f"retail_frame={record.retail_frame} "
            f"camera={','.join(map(str, record.camera))} "
            f"player={','.join(map(str, record.player))} "
            f"wingmate={','.join(map(str, record.wingmate))}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def rust_source(trace: Path, records: list[Record]) -> str:
    lines = [
        "//! Generated typed path for the clean retail Astropolis entry.",
        "//!",
        f"//! Source: `{trace.name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_astropolis_entry.py [--check]`.",
        "",
        "use super::{",
        "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,",
        "    MissionPlayerKeyframe,",
        "};",
        "",
        f"pub(super) const LAST_RETAIL_FRAME: u16 = {records[-1].retail_frame};",
        "",
        f"pub(super) const CAMERA_KEYFRAMES: [MissionCameraKeyframe; {len(records)}] = [",
    ]
    for record in records:
        values = ", ".join(f"{value:_}" for value in record.camera)
        lines.append(f"    mission_camera_keyframe({record.retail_frame}, {values}),")
    lines.extend(
        [
            "];",
            "",
            f"pub(super) const PLAYER_KEYFRAMES: [MissionPlayerKeyframe; {len(records)}] = [",
        ]
    )
    for record in records:
        values = ", ".join(f"{value:_}" for value in record.player)
        lines.append(f"    mission_player_keyframe({record.retail_frame}, {values}),")
    lines.extend(
        [
            "];",
            "",
            f"pub(super) const WINGMATE_KEYFRAMES: [MissionPlayerKeyframe; {len(records)}] = [",
        ]
    )
    for record in records:
        values = ", ".join(f"{value:_}" for value in record.wingmate)
        lines.append(f"    mission_player_keyframe({record.retail_frame}, {values}),")
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    records = load(args.trace)
    if args.compact_output is not None:
        write_compact(args.trace, args.compact_output, records)
    generated = rust_source(args.trace, records)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"{action} {args.output}: {len(records)} keyframes, "
        f"retail frames {records[0].retail_frame}..{records[-1].retail_frame}"
    )


if __name__ == "__main__":
    main()
