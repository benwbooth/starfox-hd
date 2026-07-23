#!/usr/bin/env python3
"""Generate the typed three-fighter interception from a Mesen oracle trace."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "fighter_intercept.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "fighter_intercept.rs"
)
RETAIL_FRAME_STEP = 4
FIGHTER_SOURCE_IDS = ("0576", "05F4", "05B5")
FIGHTER_SHAPE_TOKEN = "F1C4"
FIGHTER_CONSTANT_NAMES = (
    "LEAD_FIGHTER_KEYFRAMES",
    "FLANK_FIGHTER_KEYFRAMES",
    "REAR_FIGHTER_KEYFRAMES",
)
PROJECTILE_SHAPE_TOKEN = "E3A8"


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]
    fighters: tuple[tuple[int, ...] | None, ...]
    projectiles: tuple[tuple[str, tuple[int, ...]], ...]


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


def raw_objects(
    value: str,
) -> tuple[
    tuple[tuple[int, ...] | None, ...], tuple[tuple[str, tuple[int, ...]], ...]
]:
    objects: dict[str, tuple[str, tuple[int, ...]]] = {}
    projectiles = []
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        pose = tuple(map(int, parts[2:9]))
        objects[parts[0]] = (parts[1], pose)
        if parts[1] == PROJECTILE_SHAPE_TOKEN:
            projectiles.append((parts[0], pose))
    fighters = tuple(
        objects[source][1]
        if source in objects and objects[source][0] == FIGHTER_SHAPE_TOKEN
        else None
        for source in FIGHTER_SOURCE_IDS
    )
    return fighters, tuple(projectiles)


def compact_fighters(value: str) -> tuple[tuple[int, ...] | None, ...]:
    parts = value.split("/")
    if len(parts) != len(FIGHTER_SOURCE_IDS):
        raise SystemExit(
            f"fighters needs {len(FIGHTER_SOURCE_IDS)} entries, found {len(parts)}"
        )
    return tuple(
        None if part == "-" else parse_tuple(part, 7, "fighter pose")
        for part in parts
    )


def compact_projectiles(value: str) -> tuple[tuple[str, tuple[int, ...]], ...]:
    if value == "-":
        return ()
    result = []
    for projectile in value.split(";"):
        parts = projectile.split(",")
        if len(parts) != 8:
            raise SystemExit(f"malformed compact projectile: {projectile}")
        result.append((parts[0], tuple(map(int, parts[1:]))))
    return tuple(result)


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
        fighters, projectiles = raw_objects(values["objects"])
        parsed.append(
            (
                elapsed,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
                fighters,
                projectiles,
            )
        )
    if not parsed:
        raise SystemExit("trace has no three-fighter interception samples")
    start_elapsed = parsed[0][0]
    records = [
        Record(elapsed - start_elapsed, camera, player, wingmate, fighters, projectiles)
        for elapsed, camera, player, wingmate, fighters, projectiles in parsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("fighter-interception samples are not a complete four-frame cadence")
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
                compact_fighters(values["fighters"]),
                compact_projectiles(values.get("projectiles", "-")),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit("compact fighter-interception fixture is incomplete")
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
        "# Compact Mesen oracle evidence for the three-fighter interception.",
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
            "fighters="
            + "/".join(
                "-" if fighter is None else ",".join(map(str, fighter))
                for fighter in record.fighters
            )
            + " projectiles="
            + (
                ";".join(
                    source_id + "," + ",".join(map(str, pose))
                    for source_id, pose in record.projectiles
                )
                or "-"
            )
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def projectile_lifetimes(
    records: list[Record],
) -> list[list[tuple[int, tuple[int, ...]]]]:
    samples_by_source: dict[str, list[tuple[int, tuple[int, ...]]]] = {}
    for record in records:
        for source, pose in record.projectiles:
            samples_by_source.setdefault(source, []).append((record.retail_frame, pose))
    lifetimes = []
    for samples in samples_by_source.values():
        lifetime = []
        for sample in samples:
            if lifetime and sample[0] - lifetime[-1][0] > RETAIL_FRAME_STEP:
                lifetimes.append(lifetime)
                lifetime = []
            lifetime.append(sample)
        if lifetime:
            lifetimes.append(lifetime)
    lifetimes.sort(key=lambda lifetime: lifetime[0][0])
    return lifetimes


def rust_source(
    trace_name: str,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> str:
    lines = [
        "//! Generated typed path for the retail three-fighter interception.",
        "//!",
        f"//! Source: `{trace_name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_fighter_intercept.py [--check]`.",
        "",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe,",
        "    mission_actor_keyframe, mission_camera_keyframe, mission_player_keyframe,",
        "    MissionActorKeyframe, MissionCameraKeyframe, MissionPlayerKeyframe,",
        "};",
        "#[cfg(test)]",
        "use super::{mission_projectile_keyframe, MissionProjectileKeyframe};",
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

    for fighter_index, constant_name in enumerate(FIGHTER_CONSTANT_NAMES):
        present_indices = [
            index
            for index, record in enumerate(records)
            if record.fighters[fighter_index] is not None
        ]
        if not present_indices:
            raise SystemExit(f"oracle trace has no poses for {constant_name}")
        fighter_records = records[present_indices[0] : present_indices[-1] + 1]
        departure_frame = fighter_records[-1].retail_frame + RETAIL_FRAME_STEP
        lines.extend(
            [
                "",
                f"pub(super) const {constant_name}: [MissionActorKeyframe; "
                f"{len(fighter_records) + 1}] = [",
            ]
        )
        for record in fighter_records:
            pose = record.fighters[fighter_index]
            if pose is None:
                lines.append(
                    f"    mission_actor_inactive_keyframe({record.retail_frame}),"
                )
            else:
                values = ", ".join(f"{value:_}" for value in pose)
                lines.append(
                    f"    mission_actor_keyframe({record.retail_frame}, [{values}]),"
                )
        lines.append(f"    mission_actor_departure_keyframe({departure_frame}),")
        lines.append("];")

    lifetimes = projectile_lifetimes(records)
    for index, lifetime in enumerate(lifetimes):
        lines.extend(
            [
                "",
                "#[cfg(test)]",
                f"const ENEMY_LASER_TRACK_{index}: [MissionProjectileKeyframe; "
                f"{len(lifetime)}] = [",
            ]
        )
        for frame, pose in lifetime:
            values = ", ".join(f"{value:_}" for value in pose)
            lines.append(f"    mission_projectile_keyframe({frame}, [{values}]),")
        lines.append("];")
    lines.extend(
        [
            "",
            "#[cfg(test)]",
            "pub(super) const ENEMY_LASER_KEYFRAME_TRACKS: "
            f"[&[MissionProjectileKeyframe]; {len(lifetimes)}] = [",
        ]
    )
    for index in range(len(lifetimes)):
        lines.append(f"    &ENEMY_LASER_TRACK_{index},")
    lines.extend(["];", ""])
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
        f"return {return_frame}, map ready {map_ready_frame}, "
        f"enemy laser tracks {len(projectile_lifetimes(records))}"
    )


if __name__ == "__main__":
    main()
