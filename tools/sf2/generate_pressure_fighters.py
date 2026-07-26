#!/usr/bin/env python3
"""Generate the typed recurring-attacker encounter from Mesen oracle traces."""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path

from generate_second_sortie_projectiles import format_rust


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "pressure_fighters.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "pressure_fighters.rs"
)
RETAIL_FRAME_STEP = 4
MISSION_SELECTION = "6"
PROJECTILE_SHAPE_TOKENS = frozenset(("E3A8",))
# Oracle-only object identities and shape tokens. The generated port contains
# four semantic attacker tracks and never exposes these source details.
ATTACKER_IDENTITIES = (
    ("05B5", frozenset(("F1C4",))),
    ("0633", frozenset(("F1C4",))),
    ("05F4", frozenset(("F1C4",))),
    ("0576", frozenset(("EA00",))),
)


@dataclass(frozen=True)
class Record:
    retail_frame: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    wingmate: tuple[int, ...]
    attackers: tuple[tuple[int, ...] | None, ...]
    projectiles: tuple[tuple[str, tuple[int, ...]], ...]


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


def raw_objects(
    value: str,
) -> tuple[tuple[tuple[int, ...] | None, ...], tuple[tuple[str, tuple[int, ...]], ...]]:
    attackers: list[tuple[int, ...] | None] = [None] * len(ATTACKER_IDENTITIES)
    projectiles = []
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        if not object_text:
            continue
        parts = object_text.split(",")
        if len(parts) < 9:
            raise SystemExit(f"malformed oracle object: {object_text}")
        pose = tuple(map(int, parts[2:9]))
        for index, (source_id, shapes) in enumerate(ATTACKER_IDENTITIES):
            if parts[0] == source_id and parts[1] in shapes:
                attackers[index] = pose
                break
        if parts[1] in PROJECTILE_SHAPE_TOKENS:
            projectiles.append((parts[0], pose))
    return tuple(attackers), tuple(projectiles)


def raw_records(traces: tuple[Path, ...]) -> tuple[list[Record], int, int]:
    parsed_by_elapsed = {}
    transitions_by_elapsed = {}
    started = False
    for trace in traces:
        for line in trace.read_text(encoding="utf-8").splitlines():
            values = fields(line)
            if "elapsed" not in values or "mode" not in values:
                continue
            elapsed = int(values["elapsed"])
            mode = int(values["mode"])
            if started:
                transitions_by_elapsed[elapsed] = mode
            if (
                values.get("event") != "sortie"
                or mode != 1
                or values.get("selection") != MISSION_SELECTION
            ):
                continue
            started = True
            attackers, projectiles = raw_objects(values["objects"])
            parsed_by_elapsed[elapsed] = (
                elapsed,
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["playerpose"], 7, "player pose"),
                parse_tuple(values["wingpose"], 7, "wingmate pose"),
                attackers,
                projectiles,
            )
    parsed = [parsed_by_elapsed[elapsed] for elapsed in sorted(parsed_by_elapsed)]
    if not parsed:
        raise SystemExit("trace has no recurring-attacker samples")
    start_elapsed = parsed[0][0]
    transitions = sorted(transitions_by_elapsed.items())
    return_elapsed = next(
        (elapsed for elapsed, mode in transitions if elapsed > start_elapsed and mode == 7),
        None,
    )
    if return_elapsed is None:
        raise SystemExit("trace does not contain the strategic-map return")
    parsed = [record for record in parsed if record[0] < return_elapsed]
    records = [
        Record(elapsed - start_elapsed, camera, player, wingmate, attackers, projectiles)
        for elapsed, camera, player, wingmate, attackers, projectiles in parsed
    ]
    expected_frames = list(range(0, records[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("recurring-attacker samples are not a complete four-frame cadence")
    map_ready_elapsed = next(
        (
            elapsed
            for elapsed, mode in transitions
            if elapsed >= return_elapsed + 1 and mode == 7
        ),
        return_elapsed,
    )
    return records, return_elapsed - start_elapsed, map_ready_elapsed - start_elapsed


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
        attackers = tuple(
            None
            if values[f"attacker{index}"] == "-"
            else parse_tuple(values[f"attacker{index}"], 7, f"attacker {index}")
            for index in range(len(ATTACKER_IDENTITIES))
        )
        records.append(
            Record(
                int(values["retail_frame"]),
                parse_tuple(values["camera"], 6, "camera"),
                parse_tuple(values["player"], 7, "player pose"),
                parse_tuple(values["wingmate"], 7, "wingmate pose"),
                attackers,
                compact_projectiles(values.get("projectiles", "-")),
            )
        )
    if not records or return_frame is None or map_ready_frame is None:
        raise SystemExit("compact recurring-attacker fixture is incomplete")
    return records, return_frame, map_ready_frame


def load(traces: tuple[Path, ...]) -> tuple[list[Record], int, int]:
    first_content = next(
        (
            line
            for line in traces[0].read_text(encoding="utf-8").splitlines()
            if line and not line.startswith("#")
        ),
        "",
    )
    return (
        compact_records(traces[0])
        if first_content.startswith("retail_frame=")
        else raw_records(traces)
    )


def write_compact(
    sources: tuple[Path, ...],
    output: Path,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> None:
    digest = hashlib.sha256(b"".join(source.read_bytes() for source in sources)).hexdigest()
    lines = [
        "# Compact Mesen oracle evidence for the recurring four-attacker encounter.",
        f"# Raw source SHA-256: {digest}",
        f"# return_retail_frame={return_frame}",
        f"# map_ready_retail_frame={map_ready_frame}",
    ]
    for record in records:
        fields_out = [
            f"retail_frame={record.retail_frame}",
            f"camera={','.join(map(str, record.camera))}",
            f"player={','.join(map(str, record.player))}",
            f"wingmate={','.join(map(str, record.wingmate))}",
        ]
        fields_out.extend(
            f"attacker{index}="
            + ("-" if pose is None else ",".join(map(str, pose)))
            for index, pose in enumerate(record.attackers)
        )
        fields_out.append(
            "projectiles="
            + (
                ";".join(
                    source_id + "," + ",".join(map(str, pose))
                    for source_id, pose in record.projectiles
                )
                or "-"
            )
        )
        lines.append(" ".join(fields_out))
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def rust_source(
    trace_names: str,
    records: list[Record],
    return_frame: int,
    map_ready_frame: int,
) -> str:
    last_attacker_frame = max(
        record.retail_frame
        for record in records
        if any(attacker is not None for attacker in record.attackers)
    )
    accepted_defeat_frame = last_attacker_frame + RETAIL_FRAME_STEP
    accepted_defeat_record = next(
        (record for record in records if record.retail_frame == accepted_defeat_frame),
        None,
    )
    if accepted_defeat_record is None or any(
        attacker is not None for attacker in accepted_defeat_record.attackers
    ):
        raise SystemExit(
            "accepted recurring-attacker trace lacks an all-defeated boundary"
        )
    if return_frame < accepted_defeat_frame or map_ready_frame < accepted_defeat_frame:
        raise SystemExit("recurring-attacker return precedes the accepted defeat")
    defeat_to_return = return_frame - accepted_defeat_frame
    defeat_to_map_ready = map_ready_frame - accepted_defeat_frame

    lines = [
        "//! Generated typed path for the retail recurring four-attacker encounter.",
        "//!",
        f"//! Source: `{trace_names}`.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_pressure_fighters.py [--check]`.",
        "",
        "use super::{",
        "    mission_camera_keyframe, mission_player_keyframe,",
        "    MissionCameraKeyframe, MissionPlayerKeyframe,",
        "};",
        "",
        "pub(super) const CERTIFIED_PRESENTATION_END_RETAIL_FRAME: u16 = "
        f"{return_frame};",
        f"pub(super) const DEFEAT_TO_RETURN_RETAIL_FRAMES: u16 = {defeat_to_return};",
        "pub(super) const DEFEAT_TO_MAP_READY_RETAIL_FRAMES: u16 = "
        f"{defeat_to_map_ready};",
        "#[cfg(test)]",
        "pub(super) const ACCEPTED_ALL_DEFEATED_RETAIL_FRAME: u16 = "
        f"{accepted_defeat_frame};",
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
    lines.extend(["];", ""])
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--continuation-trace", action="append", type=Path, default=[])
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    traces = (args.trace, *args.continuation_trace)
    records, return_frame, map_ready_frame = load(traces)
    if args.compact_output is not None:
        write_compact(traces, args.compact_output, records, return_frame, map_ready_frame)
    generated = rust_source(
        ", ".join(trace.name for trace in traces), records, return_frame, map_ready_frame
    )
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
