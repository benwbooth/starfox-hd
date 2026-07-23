#!/usr/bin/env python3
"""Generate typed re-engagement keyframes from the post-sortie oracle trace."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "second_sortie_reengagement.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "second_sortie.rs"
RETAIL_FRAME_STEP = 4
TARGET_SOURCE_IDS = ("0633", "05F4", "05B5", "0576")
TARGET_SHAPES_BY_SOURCE = {
    "0633": "F5EC",
    "05F4": "F5EC",
    "05B5": "EA00",
    "0576": "EA00",
}
TARGET_CONSTANT_NAMES = (
    "FIRST_CAPITAL_KEYFRAMES",
    "SECOND_CAPITAL_KEYFRAMES",
    "UPPER_FIGHTER_KEYFRAMES",
    "LOWER_FIGHTER_KEYFRAMES",
)
PROJECTILE_SHAPE_TOKEN = "E3A8"
PROJECTILE_CONSTANT_NAMES = tuple(
    f"ENEMY_LASER_{number}_KEYFRAMES"
    for number in (
        "ONE",
        "TWO",
        "THREE",
        "FOUR",
        "FIVE",
        "SIX",
        "SEVEN",
        "EIGHT",
        "NINE",
        "TEN",
        "ELEVEN",
        "TWELVE",
        "THIRTEEN",
        "FOURTEEN",
        "FIFTEEN",
        "SIXTEEN",
        "SEVENTEEN",
        "EIGHTEEN",
        "NINETEEN",
        "TWENTY",
        "TWENTY_ONE",
        "TWENTY_TWO",
        "TWENTY_THREE",
        "TWENTY_FOUR",
        "TWENTY_FIVE",
        "TWENTY_SIX",
        "TWENTY_SEVEN",
        "TWENTY_EIGHT",
        "TWENTY_NINE",
        "THIRTY",
        "THIRTY_ONE",
        "THIRTY_TWO",
        "THIRTY_THREE",
    )
)


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]
    targets: tuple[tuple[int, ...] | None, ...]
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
    targets = tuple(
        objects[source][1]
        if source in objects and objects[source][0] == TARGET_SHAPES_BY_SOURCE[source]
        else None
        for source in TARGET_SOURCE_IDS
    )
    return targets, tuple(projectiles)


def compact_target_poses(value: str) -> tuple[tuple[int, ...] | None, ...]:
    parts = value.split("/")
    if len(parts) != len(TARGET_SOURCE_IDS):
        raise SystemExit(
            f"targets needs {len(TARGET_SOURCE_IDS)} entries, found {len(parts)}"
        )
    return tuple(
        None if part == "-" else parse_tuple(part, 7, "target pose") for part in parts
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
        targets, projectiles = raw_objects(values["objects"])
        parsed.append(
            (
                elapsed,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
                targets,
                projectiles,
            )
        )
    if not parsed:
        raise SystemExit("trace has no second-sortie samples")
    start_elapsed = parsed[0][0]
    records = [
        Record(elapsed - start_elapsed, camera, player, wingmate, targets, projectiles)
        for elapsed, camera, player, wingmate, targets, projectiles in parsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("second-sortie samples are not a complete four-frame cadence")
    return_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed > start_elapsed and mode == 7
        ),
        None,
    )
    if return_elapsed is None:
        raise SystemExit("trace does not contain the second strategic-map return")
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
                compact_target_poses(values.get("targets", "-/-/-/-")),
                compact_projectiles(values.get("projectiles", "-")),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit("compact second-sortie fixture is incomplete")
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
        "# Compact Mesen oracle evidence for the first re-engagement sortie.",
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
            "targets="
            + "/".join(
                "-" if target is None else ",".join(map(str, target))
                for target in record.targets
            )
            + " projectiles="
            + (
                ";".join(
                    source + "," + ",".join(map(str, pose))
                    for source, pose in record.projectiles
                )
                or "-"
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
        "//! Generated typed neutral path for the first retail re-engagement sortie.",
        "//!",
        f"//! Source: `{trace_name}`.",
        "//! Regenerate or verify with `uv run python "
        "tools/sf2/generate_second_sortie.py [--check]`.",
        "",
        "use super::{",
        "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,",
        "    MissionPlayerKeyframe,",
        "};",
        "#[cfg(test)]",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe,",
        "    mission_actor_keyframe, mission_projectile_keyframe, MissionActorKeyframe,",
        "    MissionProjectileKeyframe,",
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
    for target_index, constant_name in enumerate(TARGET_CONSTANT_NAMES):
        present_indices = [
            index
            for index, record in enumerate(records)
            if record.targets[target_index] is not None
        ]
        if not present_indices:
            raise SystemExit(f"oracle trace has no poses for {constant_name}")
        target_records = records[present_indices[0] : present_indices[-1] + 1]
        departure_frame = target_records[-1].retail_frame + RETAIL_FRAME_STEP
        lines.extend(
            [
                "",
                "#[cfg(test)]",
                f"pub(super) const {constant_name}: [MissionActorKeyframe; "
                f"{len(target_records) + 1}] = [",
            ]
        )
        for record in target_records:
            pose = record.targets[target_index]
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

    projectile_samples: dict[str, list[tuple[int, tuple[int, ...]]]] = {}
    for record in records:
        for source, pose in record.projectiles:
            projectile_samples.setdefault(source, []).append((record.retail_frame, pose))
    lifetimes: list[list[tuple[int, tuple[int, ...]]]] = []
    for samples in projectile_samples.values():
        lifetime: list[tuple[int, tuple[int, ...]]] = []
        for sample in samples:
            if lifetime and sample[0] - lifetime[-1][0] > RETAIL_FRAME_STEP:
                lifetimes.append(lifetime)
                lifetime = []
            lifetime.append(sample)
        if lifetime:
            lifetimes.append(lifetime)
    lifetimes.sort(key=lambda lifetime: lifetime[0][0])
    if len(lifetimes) != len(PROJECTILE_CONSTANT_NAMES):
        raise SystemExit(
            f"expected {len(PROJECTILE_CONSTANT_NAMES)} enemy-laser lifetimes, "
            f"found {len(lifetimes)}"
        )
    for constant_name, lifetime in zip(PROJECTILE_CONSTANT_NAMES, lifetimes):
        lines.extend(
            [
                "",
                "#[cfg(test)]",
                f"const {constant_name}: [MissionProjectileKeyframe; {len(lifetime)}] = [",
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
    for constant_name in PROJECTILE_CONSTANT_NAMES:
        lines.append(f"    &{constant_name},")
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
        write_compact(
            args.trace,
            args.compact_output,
            records,
            return_frame,
            map_ready_frame,
        )
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
