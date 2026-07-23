#!/usr/bin/env python3
"""Generate typed Mirage Dragon body tracks from a Mesen oracle trace.

The source object identifiers and shape tokens below are oracle classification
details.  Generated Rust exposes only semantic body-segment and tail tracks.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "mirage_dragon_segments.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "mirage_dragon_segments.rs"
)
RETAIL_FRAME_STEP = 4
MISSION_SELECTION = "9"

# Oracle-only identities, ordered from the head toward the tail.
BODY_SOURCE_OBJECTS = (
    ("body_segment_1", "05B5", "E1E8"),
    ("body_segment_2", "05F4", "E1E8"),
    ("body_segment_3", "0633", "E1E8"),
    ("body_segment_4", "0672", "E1E8"),
    ("body_segment_5", "06B1", "E1E8"),
    ("body_segment_6", "06F0", "E1E8"),
    ("body_segment_7", "072F", "E1E8"),
    ("body_segment_8", "076E", "E1E8"),
)
TAIL_SOURCE_OBJECT = ("tail", "07AD", "E220")
SEMANTIC_ROLES = tuple(role for role, _, _ in BODY_SOURCE_OBJECTS) + ("tail",)


@dataclass(frozen=True)
class Record:
    retail_frame: int
    poses: tuple[tuple[int, ...] | None, ...]


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def parse_pose(value: str, label: str) -> tuple[int, ...] | None:
    if value == "-":
        return None
    pose = tuple(map(int, value.split(",")))
    if len(pose) != 7:
        raise SystemExit(f"{label} needs 7 values, found {len(pose)}")
    return pose


def raw_object_poses(value: str) -> dict[tuple[str, str], tuple[int, ...]]:
    result = {}
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        result[(parts[0], parts[1])] = tuple(map(int, parts[2:9]))
    return result


def raw_records(trace: Path) -> tuple[list[Record], int, int]:
    source_objects = BODY_SOURCE_OBJECTS + (TAIL_SOURCE_OBJECT,)
    parsed = []
    transitions = []
    started = False
    start_elapsed = None
    for line in trace.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if "elapsed" not in values or "mode" not in values:
            continue
        elapsed = int(values["elapsed"])
        mode = int(values["mode"])
        if started:
            transitions.append((elapsed, mode))
        if (
            values.get("event") != "sortie"
            or mode != 1
            or values.get("selection") != MISSION_SELECTION
        ):
            continue
        if start_elapsed is None:
            start_elapsed = elapsed
            started = True
        objects = raw_object_poses(values["objects"])
        parsed.append(
            Record(
                elapsed - start_elapsed,
                tuple(objects.get((source_id, shape)) for _, source_id, shape in source_objects),
            )
        )
    if not parsed or start_elapsed is None:
        raise SystemExit("trace has no Mirage Dragon sortie samples")
    return_elapsed = next(
        (elapsed for elapsed, mode in transitions if elapsed > start_elapsed and mode == 7),
        None,
    )
    if return_elapsed is None:
        raise SystemExit("trace does not contain the strategic-map return")
    records = [record for record in parsed if record.retail_frame < return_elapsed - start_elapsed]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("Mirage Dragon samples are not a complete four-frame cadence")
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
                tuple(parse_pose(values[role], role) for role in SEMANTIC_ROLES),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit("compact Mirage Dragon segment fixture is incomplete")
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
    return compact_records(trace) if first_content.startswith("retail_frame=") else raw_records(trace)


def write_compact(
    source: Path,
    output: Path,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> None:
    lines = [
        "# Compact Mesen oracle evidence for the articulated Mirage Dragon body.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# return_retail_frame={return_frame}",
        f"# map_ready_retail_frame={map_ready_frame}",
    ]
    for record in records:
        values = [f"retail_frame={record.retail_frame}"]
        for role, pose in zip(SEMANTIC_ROLES, record.poses, strict=True):
            values.append(f"{role}=" + ("-" if pose is None else ",".join(map(str, pose))))
        lines.append(" ".join(values))
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def track_source(role: str, records: list[Record], role_index: int) -> tuple[str, str]:
    present_indices = [index for index, record in enumerate(records) if record.poses[role_index] is not None]
    if not present_indices:
        raise SystemExit(f"oracle trace has no {role} poses")
    first = present_indices[0]
    last = present_indices[-1]
    samples = records[first : last + 1]
    const_name = "RETAIL_" + role.upper() + "_KEYFRAMES"
    lines = [f"const {const_name}: [MissionActorKeyframe; {len(samples) + 1}] = ["]
    for record in samples:
        pose = record.poses[role_index]
        if pose is None:
            lines.append(f"    mission_actor_inactive_keyframe({record.retail_frame}),")
        else:
            values = ", ".join(f"{value:_}" for value in pose)
            lines.append(f"    mission_actor_keyframe({record.retail_frame}, [{values}]),")
    departure_frame = samples[-1].retail_frame + RETAIL_FRAME_STEP
    lines.extend([f"    mission_actor_departure_keyframe({departure_frame}),", "];", ""])
    return const_name, "\n".join(lines)


def rust_source(trace_name: str, records: list[Record]) -> str:
    has_inactive = False
    for role_index in range(len(SEMANTIC_ROLES)):
        present_indices = [
            index
            for index, record in enumerate(records)
            if record.poses[role_index] is not None
        ]
        has_inactive |= any(
            record.poses[role_index] is None
            for record in records[present_indices[0] : present_indices[-1] + 1]
        )
    actor_imports = [
        "use super::{mission_actor_departure_keyframe, mission_actor_keyframe, MissionActorKeyframe};"
    ]
    if has_inactive:
        actor_imports = [
            "use super::{",
            "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe,",
            "    mission_actor_keyframe, MissionActorKeyframe,",
            "};",
        ]
    lines = [
        "//! Generated typed paths for the retail Mirage Dragon's articulated body.",
        "//!",
        f"//! Source: `{trace_name}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_mirage_dragon_segments.py [--check]`.",
        "",
        *actor_imports,
        "",
    ]
    const_names = []
    for role_index, role in enumerate(SEMANTIC_ROLES):
        const_name, source = track_source(role, records, role_index)
        const_names.append(const_name)
        lines.append(source)
    body_names = const_names[:-1]
    lines.append(
        "pub(super) const BODY_SEGMENT_KEYFRAME_TRACKS: "
        f"[&[MissionActorKeyframe]; {len(body_names)}] = ["
    )
    lines.extend(f"    &{name}," for name in body_names)
    lines.extend(["];"])
    lines.extend(
        [
            "",
            f"pub(super) const TAIL_KEYFRAMES: &[MissionActorKeyframe] = &{const_names[-1]};",
            "",
        ]
    )
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
    generated = rust_source(args.trace.name, records)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"{action} {args.output}: {len(records)} frames, "
        f"{len(BODY_SOURCE_OBJECTS)} body segments and one tail"
    )


if __name__ == "__main__":
    main()
