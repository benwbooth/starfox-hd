#!/usr/bin/env python3
"""Generate the typed Pigma duel from a Mesen campaign-oracle trace."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "pigma_duel.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "pigma_duel.rs"
RETAIL_FRAME_STEP = 4
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "C348"
RIVAL_MISSION_SELECTION = "7"
DEFAULT_PROJECTILE_SHAPE_TOKENS = frozenset(("E3A8",))


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]
    rival: tuple[int, ...] | None
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
    projectile_shape_tokens: frozenset[str],
    rival_source_id: str,
    rival_shape_token: str,
) -> tuple[tuple[int, ...] | None, tuple[tuple[str, tuple[int, ...]], ...]]:
    rival = None
    projectiles = []
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        pose = tuple(map(int, parts[2:9]))
        if parts[0] == rival_source_id and parts[1] == rival_shape_token:
            rival = pose
        if parts[1] in projectile_shape_tokens:
            projectiles.append((parts[0], pose))
    return rival, tuple(projectiles)


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


def raw_records(
    traces: tuple[Path, ...],
    projectile_shape_tokens: frozenset[str],
    duel_name: str,
    rival_source_id: str,
    rival_shape_token: str,
    mission_selection: str,
    start_elapsed: int | None,
) -> tuple[list[Record], int, int]:
    parsed_by_elapsed = {}
    transitions_by_elapsed = {}
    started = False
    for trace in traces:
        for line in trace.read_text(encoding="utf-8").splitlines():
            values = fields(line)
            if "elapsed" not in values or "mode" not in values:
                continue
            elapsed = int(values["elapsed"])
            if start_elapsed is not None and elapsed < start_elapsed:
                continue
            mode = int(values["mode"])
            if started:
                transitions_by_elapsed[elapsed] = mode
            if (
                values.get("event") != "sortie"
                or mode != 1
                or values.get("selection") != mission_selection
            ):
                continue
            started = True
            rival, projectiles = raw_objects(
                values["objects"],
                projectile_shape_tokens,
                rival_source_id,
                rival_shape_token,
            )
            parsed_by_elapsed[elapsed] = (
                elapsed,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
                rival,
                projectiles,
            )
    parsed = [parsed_by_elapsed[elapsed] for elapsed in sorted(parsed_by_elapsed)]
    transitions = sorted(transitions_by_elapsed.items())
    if not parsed:
        raise SystemExit(f"trace has no {duel_name}-duel samples")
    start_elapsed = parsed[0][0]
    return_elapsed = next(
        (elapsed for elapsed, mode in transitions if elapsed > start_elapsed and mode == 7),
        None,
    )
    if return_elapsed is None:
        raise SystemExit("trace does not contain the post-duel strategic-map return")
    parsed = [record for record in parsed if record[0] < return_elapsed]
    parsed = [
        record
        for record in parsed
        if (record[0] - start_elapsed) % RETAIL_FRAME_STEP == 0
    ]
    records = [
        Record(elapsed - start_elapsed, camera, player, wingmate, rival, projectiles)
        for elapsed, camera, player, wingmate, rival, projectiles in parsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit(
            f"{duel_name}-duel samples are not a complete four-frame cadence"
        )
    map_ready_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed >= return_elapsed + 1 and mode == 7
        ),
        return_elapsed,
    )
    return records, return_elapsed - start_elapsed, map_ready_elapsed - start_elapsed


def compact_records(trace: Path, duel_name: str) -> tuple[list[Record], int, int]:
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
                None
                if values["rival"] == "-"
                else parse_tuple(values["rival"], 7, "rival pose"),
                compact_projectiles(values.get("projectiles", "-")),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit(f"compact {duel_name}-duel fixture is incomplete")
    return records, return_frame, map_ready_frame


def load(
    traces: tuple[Path, ...],
    projectile_shape_tokens: frozenset[str],
    duel_name: str,
    rival_source_id: str,
    rival_shape_token: str,
    mission_selection: str,
    start_elapsed: int | None = None,
) -> tuple[list[Record], int, int]:
    trace = traces[0]
    first_content = next(
        (
            line
            for line in trace.read_text(encoding="utf-8").splitlines()
            if line and not line.startswith("#")
        ),
        "",
    )
    return (
        compact_records(trace, duel_name)
        if first_content.startswith("retail_frame=")
        else raw_records(
            traces,
            projectile_shape_tokens,
            duel_name,
            rival_source_id,
            rival_shape_token,
            mission_selection,
            start_elapsed,
        )
    )


def write_compact(
    sources: tuple[Path, ...],
    output: Path,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
    duel_name: str,
) -> None:
    lines = [
        f"# Compact Mesen oracle evidence for the {duel_name} duel.",
        "# Raw source SHA-256: "
        + hashlib.sha256(b"".join(source.read_bytes() for source in sources)).hexdigest(),
        f"# return_retail_frame={return_frame}",
        f"# map_ready_retail_frame={map_ready_frame}",
    ]
    for record in records:
        lines.append(
            f"retail_frame={record.retail_frame} "
            f"camera={','.join(map(str, record.camera))} "
            f"player={','.join(map(str, record.player))} "
            f"wingmate={','.join(map(str, record.wingmate))} "
            "rival="
            + ("-" if record.rival is None else ",".join(map(str, record.rival)))
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
    duel_name: str,
    generator_name: str,
    projectiles_test_only: bool = False,
    rival_test_only: bool = False,
    timing_test_only: bool = False,
) -> str:
    present_indices = [index for index, record in enumerate(records) if record.rival is not None]
    if not present_indices:
        raise SystemExit(f"oracle trace has no {duel_name} poses")
    rival_records = records[present_indices[0] : present_indices[-1] + 1]
    actor_helpers = [
        "    mission_actor_departure_keyframe, mission_actor_keyframe, mission_camera_keyframe,",
        "    mission_player_keyframe, mission_projectile_keyframe, MissionActorKeyframe,",
        "    MissionCameraKeyframe, MissionPlayerKeyframe, MissionProjectileKeyframe,",
    ]
    if any(record.rival is None for record in rival_records):
        actor_helpers = [
            "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe, mission_actor_keyframe,",
            "    mission_camera_keyframe, mission_player_keyframe, mission_projectile_keyframe,",
            "    MissionActorKeyframe, MissionCameraKeyframe, MissionPlayerKeyframe, MissionProjectileKeyframe,",
        ]
    if not any(record.projectiles for record in records):
        actor_helpers = [
            "    mission_actor_departure_keyframe, mission_actor_keyframe, mission_camera_keyframe,",
            "    mission_player_keyframe, MissionActorKeyframe, MissionCameraKeyframe, MissionPlayerKeyframe,",
        ]
        if any(record.rival is None for record in rival_records):
            actor_helpers = [
                "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe, mission_actor_keyframe,",
                "    mission_camera_keyframe, mission_player_keyframe, MissionActorKeyframe,",
                "    MissionCameraKeyframe, MissionPlayerKeyframe,",
            ]
    elif projectiles_test_only:
        actor_helpers = [
            "    mission_actor_departure_keyframe, mission_actor_keyframe, mission_camera_keyframe,",
            "    mission_player_keyframe, MissionActorKeyframe, MissionCameraKeyframe, MissionPlayerKeyframe,",
        ]
        if any(record.rival is None for record in rival_records):
            actor_helpers = [
                "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe, mission_actor_keyframe,",
                "    mission_camera_keyframe, mission_player_keyframe, MissionActorKeyframe,",
                "    MissionCameraKeyframe, MissionPlayerKeyframe,",
            ]
    projectile_import = (
        [
            "#[cfg(test)]",
            "use super::{mission_projectile_keyframe, MissionProjectileKeyframe};",
            "",
        ]
        if projectiles_test_only
        else []
    )
    rival_import = []
    if rival_test_only:
        if any(record.projectiles for record in records) and not projectiles_test_only:
            actor_helpers = [
                "    mission_camera_keyframe, mission_player_keyframe, mission_projectile_keyframe,",
                "    MissionCameraKeyframe, MissionPlayerKeyframe, MissionProjectileKeyframe,",
            ]
        else:
            actor_helpers = [
                "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,",
                "    MissionPlayerKeyframe,",
            ]
        rival_import = [
            "#[cfg(test)]",
            "use super::{mission_actor_departure_keyframe, mission_actor_keyframe, MissionActorKeyframe};",
            "",
        ]
        if any(record.rival is None for record in rival_records):
            rival_import = [
                "#[cfg(test)]",
                "use super::{",
                "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe,",
                "    mission_actor_keyframe, MissionActorKeyframe,",
                "};",
                "",
            ]
    lines = [
        f"//! Generated typed path for the retail {duel_name} duel.",
        "//!",
        f"//! Source: `{trace_name}`.",
        "//! Regenerate or verify with `uv run python",
        f"//! tools/sf2/{generator_name} [--check]`.",
        "",
        "use super::{",
        *actor_helpers,
        "};",
        "",
        *rival_import,
        *projectile_import,
        *(["#[cfg(test)]"] if timing_test_only else []),
        f"pub(super) const RETURN_RETAIL_FRAME: u16 = {return_frame};",
        *(["#[cfg(test)]"] if timing_test_only else []),
        f"pub(super) const MAP_READY_RETAIL_FRAME: u16 = {map_ready_frame};",
        "",
        f"pub(super) const CAMERA_KEYFRAMES: [MissionCameraKeyframe; {len(records)}] = [",
    ]
    for record in records:
        lines.append(
            f"    mission_camera_keyframe({record.retail_frame}, "
            + ", ".join(f"{value:_}" for value in record.camera)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
            f"pub(super) const PLAYER_KEYFRAMES: [MissionPlayerKeyframe; {len(records)}] = [",
        ]
    )
    for record in records:
        lines.append(
            f"    mission_player_keyframe({record.retail_frame}, "
            + ", ".join(f"{value:_}" for value in record.player)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
            f"pub(super) const WINGMATE_KEYFRAMES: [MissionPlayerKeyframe; {len(records)}] = [",
        ]
    )
    for record in records:
        lines.append(
            f"    mission_player_keyframe({record.retail_frame}, "
            + ", ".join(f"{value:_}" for value in record.wingmate)
            + "),"
        )
    lines.append("];")

    departure_frame = rival_records[-1].retail_frame + RETAIL_FRAME_STEP
    lines.extend(
        [
            "",
            *(["#[cfg(test)]"] if rival_test_only else []),
            f"pub(super) const RIVAL_KEYFRAMES: [MissionActorKeyframe; {len(rival_records) + 1}] = [",
        ]
    )
    for record in rival_records:
        if record.rival is None:
            lines.append(f"    mission_actor_inactive_keyframe({record.retail_frame}),")
        else:
            values = ", ".join(f"{value:_}" for value in record.rival)
            lines.append(f"    mission_actor_keyframe({record.retail_frame}, [{values}]),")
    lines.extend([f"    mission_actor_departure_keyframe({departure_frame}),", "];"])

    lifetimes = projectile_lifetimes(records)
    for index, lifetime in enumerate(lifetimes):
        lines.extend(
            [
                "",
                *(["#[cfg(test)]"] if projectiles_test_only else []),
                f"const ENEMY_LASER_TRACK_{index}: [MissionProjectileKeyframe; {len(lifetime)}] = [",
            ]
        )
        for frame, pose in lifetime:
            values = ", ".join(f"{value:_}" for value in pose)
            lines.append(f"    mission_projectile_keyframe({frame}, [{values}]),")
        lines.append("];")
    if lifetimes:
        lines.extend(
            [
                "",
                *(["#[cfg(test)]"] if projectiles_test_only else []),
                f"pub(super) const ENEMY_LASER_KEYFRAME_TRACKS: [&[MissionProjectileKeyframe]; {len(lifetimes)}] = [",
            ]
        )
        for index in range(len(lifetimes)):
            lines.append(f"    &ENEMY_LASER_TRACK_{index},")
        lines.extend(["];", ""])
    else:
        lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--continuation-trace",
        action="append",
        type=Path,
        default=[],
        help="continued raw oracle trace; overlapping samples are deduplicated by elapsed frame",
    )
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--duel-name", default="Pigma")
    parser.add_argument(
        "--generator-name",
        default="generate_pigma_duel.py",
        help="script name embedded in the generated module documentation",
    )
    parser.add_argument(
        "--projectile-shape-token",
        action="append",
        help="oracle-only source shape token belonging to an enemy projectile",
    )
    parser.add_argument("--rival-source-id", default=RIVAL_SOURCE_ID)
    parser.add_argument("--rival-shape-token", default=RIVAL_SHAPE_TOKEN)
    parser.add_argument("--mission-selection", default=RIVAL_MISSION_SELECTION)
    parser.add_argument(
        "--start-elapsed",
        type=int,
        help="ignore raw oracle records before this elapsed frame",
    )
    parser.add_argument(
        "--rival-test-only",
        action="store_true",
        help="retain rival poses only as an oracle-backed test fixture",
    )
    parser.add_argument(
        "--omit-projectiles",
        action="store_true",
        help="omit projectile pose tracks after semantic dynamics have replaced them",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    projectile_shape_tokens = frozenset(
        token.removeprefix("0x").upper()
        for token in (
            args.projectile_shape_token
            if args.projectile_shape_token is not None
            else DEFAULT_PROJECTILE_SHAPE_TOKENS
        )
    )
    traces = (args.trace, *args.continuation_trace)
    records, return_frame, map_ready_frame = load(
        traces,
        projectile_shape_tokens,
        args.duel_name,
        args.rival_source_id.upper(),
        args.rival_shape_token.removeprefix("0x").upper(),
        args.mission_selection,
        args.start_elapsed,
    )
    if args.compact_output is not None:
        write_compact(
            traces,
            args.compact_output,
            records,
            return_frame,
            map_ready_frame,
            args.duel_name,
        )
    generated = rust_source(
        ", ".join(trace.name for trace in traces),
        (
            [
                Record(
                    record.retail_frame,
                    record.camera,
                    record.player,
                    record.wingmate,
                    record.rival,
                    (),
                )
                for record in records
            ]
            if args.omit_projectiles
            else records
        ),
        return_frame,
        map_ready_frame,
        args.duel_name,
        args.generator_name,
        projectiles_test_only=args.output.resolve() == DEFAULT_OUTPUT.resolve(),
        rival_test_only=(
            args.rival_test_only or args.output.resolve() == DEFAULT_OUTPUT.resolve()
        ),
    )
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    projectile_summary = (
        "enemy laser tracks omitted"
        if args.omit_projectiles
        else f"enemy laser tracks {len(projectile_lifetimes(records))}"
    )
    print(
        f"{action} {args.output}: {len(records)} keyframes, "
        f"retail frames {records[0].retail_frame}..{records[-1].retail_frame}, "
        f"return {return_frame}, map ready {map_ready_frame}, "
        f"{projectile_summary}"
    )


if __name__ == "__main__":
    main()
