#!/usr/bin/env python3
"""Generate the typed missile-interception sortie from a Mesen oracle trace."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "missile_interception.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "missile_interception.rs"
)
RETAIL_FRAME_STEP = 4
MISSILE_SOURCE_IDS = ("05F4", "05B5", "0576")
MISSILE_SHAPE_TOKEN = "D068"
MISSILE_CONSTANT_NAMES = (
    "LEAD_MISSILE_KEYFRAMES",
    "UPPER_MISSILE_KEYFRAMES",
    "LOWER_MISSILE_KEYFRAMES",
)


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]
    missiles: tuple[tuple[int, ...] | None, ...]


def parse_tuple(value: str, length: int, label: str) -> tuple[int, ...]:
    result = tuple(map(int, value.split(",")))
    if len(result) != length:
        raise SystemExit(f"{label} needs {length} values, found {len(result)}")
    return result


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def raw_missiles(value: str) -> tuple[tuple[int, ...] | None, ...]:
    objects: dict[str, tuple[str, tuple[int, ...]]] = {}
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        objects[parts[0]] = (parts[1], tuple(map(int, parts[2:9])))
    return tuple(
        objects[source][1]
        if source in objects and objects[source][0] == MISSILE_SHAPE_TOKEN
        else None
        for source in MISSILE_SOURCE_IDS
    )


def compact_missiles(value: str) -> tuple[tuple[int, ...] | None, ...]:
    parts = value.split("/")
    if len(parts) != len(MISSILE_SOURCE_IDS):
        raise SystemExit(
            f"missiles needs {len(MISSILE_SOURCE_IDS)} entries, found {len(parts)}"
        )
    return tuple(
        None if part == "-" else parse_tuple(part, 7, "missile pose")
        for part in parts
    )


def raw_records(trace: Path) -> tuple[list[Record], int, int]:
    parsed = []
    transitions = []
    for line in trace.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if "elapsed" not in values or "mode" not in values:
            continue
        elapsed = int(values["elapsed"])
        mode = int(values["mode"])
        transitions.append((elapsed, mode))
        if values.get("event") != "sortie" or mode != 1:
            continue
        parsed.append(
            (
                elapsed,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
                raw_missiles(values["objects"]),
            )
        )
    if not parsed:
        raise SystemExit("trace has no missile-interception samples")
    start_elapsed = parsed[0][0]
    records = [
        Record(elapsed - start_elapsed, camera, player, wingmate, missiles)
        for elapsed, camera, player, wingmate, missiles in parsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("interception samples are not a complete four-frame cadence")
    return_elapsed = next(
        (elapsed for elapsed, mode in transitions if elapsed > start_elapsed and mode == 7),
        None,
    )
    if return_elapsed is None:
        raise SystemExit("trace does not contain the strategic-map return")
    map_ready_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed >= return_elapsed + 1 and mode == 7
        ),
        return_elapsed,
    )
    return records, return_elapsed - start_elapsed, map_ready_elapsed - start_elapsed


def compact_records(trace: Path) -> tuple[list[Record], int, int]:
    records = []
    return_frame = None
    map_ready_frame = None
    for line in trace.read_text(encoding="utf-8").splitlines():
        if line.startswith("# return_retail_frame="):
            return_frame = int(line.split("=", 1)[1])
            continue
        if line.startswith("# map_ready_retail_frame="):
            map_ready_frame = int(line.split("=", 1)[1])
            continue
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        records.append(
            Record(
                int(values["retail_frame"]),
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["player"], 7, "player pose"),
                parse_tuple(values["wingmate"], 7, "wingmate pose"),
                compact_missiles(values["missiles"]),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit("compact interception fixture is incomplete")
    return records, return_frame, map_ready_frame


def load(trace: Path) -> tuple[list[Record], int, int]:
    first_content = next(
        (
            line
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line and not line.startswith("#")
        ),
        "",
    )
    if first_content.startswith("retail_frame="):
        return compact_records(trace)
    return raw_records(trace)


def write_compact(
    source: Path,
    output: Path,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> None:
    lines = [
        "# Compact Mesen oracle evidence for the missile-interception sortie.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# return_retail_frame={return_frame}",
        f"# map_ready_retail_frame={map_ready_frame}",
    ]
    for record in records:
        lines.append(
            f"retail_frame={record.retail_frame} "
            f"camera={','.join(map(str, record.camera))} "
            f"player={','.join(map(str, record.player))} "
            f"wingmate={','.join(map(str, record.wingmate))} "
            "missiles="
            + "/".join(
                "-" if missile is None else ",".join(map(str, missile))
                for missile in record.missiles
            )
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def rust_source(
    trace_name: str,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> str:
    lines = [
        "//! Generated typed neutral path for the retail missile interception.",
        "//!",
        f"//! Source: `{trace_name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_missile_interception.py [--check]`.",
        "",
        "use super::{",
        "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,",
        "    MissionPlayerKeyframe,",
        "};",
        "#[cfg(test)]",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_keyframe, MissionActorKeyframe,",
        "};",
        "",
        f"pub(super) const RETURN_RETAIL_FRAME: u16 = {return_frame};",
        f"pub(super) const MAP_READY_RETAIL_FRAME: u16 = {map_ready_frame};",
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
    lines.append("];")

    for missile_index, constant_name in enumerate(MISSILE_CONSTANT_NAMES):
        present_indices = [
            index
            for index, record in enumerate(records)
            if record.missiles[missile_index] is not None
        ]
        if not present_indices:
            raise SystemExit(f"oracle trace has no poses for {constant_name}")
        missile_records = records[present_indices[0] : present_indices[-1] + 1]
        if any(record.missiles[missile_index] is None for record in missile_records):
            raise SystemExit(f"oracle trace has an internal gap for {constant_name}")
        departure_frame = missile_records[-1].retail_frame + RETAIL_FRAME_STEP
        lines.extend(
            [
                "",
                "#[cfg(test)]",
                f"pub(super) const {constant_name}: [MissionActorKeyframe; "
                f"{len(missile_records) + 1}] = [",
            ]
        )
        for record in missile_records:
            pose = record.missiles[missile_index]
            assert pose is not None
            values = ", ".join(f"{value:_}" for value in pose)
            lines.append(
                f"    mission_actor_keyframe({record.retail_frame}, [{values}]),"
            )
        lines.append(f"    mission_actor_departure_keyframe({departure_frame}),")
        lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    records, return_frame, map_ready_frame = load(args.trace)
    if args.compact_output is not None:
        write_compact(args.trace, args.compact_output, records, return_frame, map_ready_frame)
    generated = rust_source(args.trace.name, records, return_frame, map_ready_frame)
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
        f"retail frames {records[0].retail_frame}..{records[-1].retail_frame}, "
        f"return {return_frame}, map ready {map_ready_frame}"
    )


if __name__ == "__main__":
    main()
