#!/usr/bin/env python3
"""Import presentation-time Mirage Dragon body poses from the retail oracle.

This fixture is verification evidence only. Shipping Rust consumes the
separate semantic action schedule and never embeds source object identities or
sampled body poses.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


DEFAULT_OUTPUT = Path(__file__).with_name("fixtures") / "mirage_dragon_segments.trace"
RETAIL_FRAME_STEP = 4
MISSION_SELECTION = "9"
RETAIL_RETURN_FRAME = 876
RETAIL_MAP_READY_FRAME = 878
SOURCE_OBJECTS = (
    ("body_segment_1", "05B5", "E1E8"),
    ("body_segment_2", "05F4", "E1E8"),
    ("body_segment_3", "0633", "E1E8"),
    ("body_segment_4", "0672", "E1E8"),
    ("body_segment_5", "06B1", "E1E8"),
    ("body_segment_6", "06F0", "E1E8"),
    ("body_segment_7", "072F", "E1E8"),
    ("body_segment_8", "076E", "E1E8"),
    ("tail", "07AD", "E220"),
)


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


def object_poses(value: str) -> dict[tuple[str, str], tuple[int, ...]]:
    result = {}
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        result[(parts[0], parts[1])] = tuple(map(int, parts[2:9]))
    return result


def import_raw(source: Path) -> tuple[list[Record], int, int]:
    records = []
    transitions = []
    start_elapsed = None
    started = False
    for line in source.read_text(encoding="utf-8").splitlines():
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
        if (elapsed - start_elapsed) % RETAIL_FRAME_STEP != 0:
            continue
        objects = object_poses(values["objects"])
        records.append(
            Record(
                elapsed - start_elapsed,
                tuple(
                    objects.get((source_object, shape))
                    for _, source_object, shape in SOURCE_OBJECTS
                ),
            )
        )
    if not records or start_elapsed is None:
        raise SystemExit("trace has no Mirage Dragon sortie samples")
    return_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed > start_elapsed and mode == 7
        ),
        None,
    )
    if return_elapsed is None:
        return_elapsed = start_elapsed + RETAIL_RETURN_FRAME
    records = [
        record
        for record in records
        if record.retail_frame < return_elapsed - start_elapsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("Mirage Dragon samples are not a complete four-frame cadence")
    map_ready_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed >= return_elapsed + 1 and mode == 7
        ),
        start_elapsed + RETAIL_MAP_READY_FRAME,
    )
    return (
        records,
        return_elapsed - start_elapsed,
        map_ready_elapsed - start_elapsed,
    )


def compact_source(
    source: Path,
    records: list[Record],
    return_retail_frame: int,
    map_ready_retail_frame: int,
) -> str:
    lines = [
        "# Compact Mesen oracle evidence for the articulated Mirage Dragon body.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# return_retail_frame={return_retail_frame}",
        f"# map_ready_retail_frame={map_ready_retail_frame}",
    ]
    for record in records:
        values = [f"retail_frame={record.retail_frame}"]
        for (role, _, _), pose in zip(SOURCE_OBJECTS, record.poses, strict=True):
            values.append(
                f"{role}=" + ("-" if pose is None else ",".join(map(str, pose)))
            )
        lines.append(" ".join(values))
    return "\n".join(lines) + "\n"


def validate_compact(source: str) -> int:
    frames = [
        int(fields(line)["retail_frame"])
        for line in source.splitlines()
        if line.startswith("retail_frame=")
    ]
    if not frames or frames != list(range(0, frames[-1] + 1, RETAIL_FRAME_STEP)):
        raise SystemExit("compact Mirage Dragon pose fixture has an invalid cadence")
    for line in source.splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        for role, _, _ in SOURCE_OBJECTS:
            pose = values.get(role)
            if pose is None:
                raise SystemExit(f"compact pose fixture lacks {role}")
            if pose != "-" and len(pose.split(",")) != 7:
                raise SystemExit(f"{role} pose must contain seven values")
    return len(frames)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        records, return_frame, map_ready_frame = import_raw(args.import_raw)
        generated = compact_source(
            args.import_raw,
            records,
            return_frame,
            map_ready_frame,
        )
        if args.check:
            if not args.output.exists() or args.output.read_text(encoding="utf-8") != generated:
                raise SystemExit(f"{args.output} is stale")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(generated, encoding="utf-8")
    elif not args.output.exists():
        raise SystemExit(f"{args.output} does not exist")

    frame_count = validate_compact(args.output.read_text(encoding="utf-8"))
    print(f"Mirage Dragon presentation fixture verified: {frame_count} frames")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
