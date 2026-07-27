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
DEFAULT_NEUTRAL_TRACE = (
    Path(__file__).with_name("fixtures") / "pressure_fighter_neutral.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "pressure_fighters.rs"
)
RETAIL_FRAME_STEP = 4
MISSION_SELECTION = "6"
ENTRY_LAST_RETAIL_FRAME = 312
LIVE_FIRST_RETAIL_FRAME = 316
LIVE_LAST_RETAIL_FRAME = 2_016
PLAYER_HANDOFF_POSITION = (3_980, 3_177, 1_328)
PLAYER_HANDOFF_PITCH = 0
PLAYER_HANDOFF_YAW = 64
PLAYER_HANDOFF_BANK = 10
PLAYER_HANDOFF_SPEED = 0
CAMERA_HANDOFF_POSITION = (4_129, 3_197, 1_319)
PLAYER_NEUTRAL_TARGET_SPEED = 23
PLAYER_FAST_ACCELERATION = 2
PLAYER_FAST_SPEED_LIMIT = 8
PLAYER_AMBIENT_BANK_WAVE = (
    0,
    1,
    2,
    2,
    3,
    3,
    4,
    4,
    4,
    4,
    3,
    3,
    2,
    2,
    1,
    0,
    -1,
    -2,
    -2,
    -3,
    -3,
    -4,
    -4,
    -4,
    -4,
    -3,
    -3,
    -2,
    -2,
    -1,
)
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


@dataclass(frozen=True)
class NeutralRecord:
    retail_frame: int
    player_updates: int
    camera_updates: int
    camera: tuple[int, ...]
    player: tuple[int, ...]


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


def load_neutral(trace: Path) -> tuple[list[NeutralRecord], int]:
    records = []
    neutral_last_retail_frame = None
    for line in trace.read_text(encoding="utf-8").splitlines():
        if line.startswith("# neutral_last_retail_frame="):
            neutral_last_retail_frame = int(line.split("=", 1)[1])
            continue
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        records.append(
            NeutralRecord(
                retail_frame=int(values["retail_frame"]),
                player_updates=int(values["player_updates"]),
                camera_updates=int(values["camera_updates"]),
                camera=parse_tuple(values["camera"], 6, "neutral camera"),
                player=parse_tuple(values["player"], 7, "neutral player"),
            )
        )
    expected_frames = list(
        range(0, LIVE_LAST_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    )
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("neutral recurring-attacker fixture has an invalid cadence")
    if any(
        record.player_updates != 0 or record.camera_updates != 0
        for record in records
        if record.retail_frame <= ENTRY_LAST_RETAIL_FRAME
    ):
        raise SystemExit("neutral entry fixture contains live update credits")
    if any(
        record.player_updates not in (0, 1)
        or record.camera_updates not in (0, 1)
        for record in records
        if record.retail_frame >= LIVE_FIRST_RETAIL_FRAME
    ):
        raise SystemExit("neutral live cadence contains unsupported update counts")
    if neutral_last_retail_frame is None:
        raise SystemExit("neutral recurring-attacker fixture lacks a pre-impact boundary")
    return records, neutral_last_retail_frame


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
    neutral_trace_name: str,
    neutral_records: list[NeutralRecord],
    neutral_last_retail_frame: int,
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
    entry_records = [
        record
        for record in neutral_records
        if record.retail_frame <= ENTRY_LAST_RETAIL_FRAME
    ]
    live_records = [
        record
        for record in neutral_records
        if record.retail_frame >= LIVE_FIRST_RETAIL_FRAME
    ]
    player_skip_frames = [
        record.retail_frame for record in live_records if record.player_updates == 0
    ]
    player_control_only_frames = [
        record.retail_frame
        for previous, record in zip(neutral_records, neutral_records[1:])
        if record.retail_frame >= LIVE_FIRST_RETAIL_FRAME
        and record.player_updates == 0
        and record.player != previous.player
    ]
    camera_skip_frames = [
        record.retail_frame for record in live_records if record.camera_updates == 0
    ]
    camera_anchor_only_frames = [
        record.retail_frame
        for previous, record in zip(neutral_records, neutral_records[1:])
        if record.retail_frame >= LIVE_FIRST_RETAIL_FRAME
        and record.camera_updates == 0
        and record.camera != previous.camera
    ]
    bank_wave = ", ".join(map(str, PLAYER_AMBIENT_BANK_WAVE))
    camera_ambient_wave = (
        "1, 0, 1, 0, 0, 1, 0, 0, 0, 0, -1, 0, 0, -1, 0, -1, "
        "-1, 0, -1, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1"
    )

    lines = [
        "//! Generated typed entry and live cadence for the retail recurring-attacker encounter.",
        "//!",
        f"//! Combat source: `{trace_names}`.",
        f"//! Neutral-flight source: `{neutral_trace_name}`.",
        "//! Live player and camera poses remain oracle evidence only; shipping",
        "//! Rust advances typed state using the statically recovered rules below.",
        "//! Regenerate or verify with `uv run python",
        "//! tools/sf2/generate_pressure_fighters.py [--check]`.",
        "",
        "use super::{",
        "    mission_camera_keyframe, mission_player_keyframe, Angle,",
        "    MissionCameraKeyframe, MissionPlayerKeyframe, Vector3,",
        "};",
        "",
        "#[cfg(test)]",
        "pub(super) const ACCEPTED_RETURN_RETAIL_FRAME: u16 = "
        f"{return_frame};",
        f"pub(super) const DEFEAT_TO_RETURN_RETAIL_FRAMES: u16 = {defeat_to_return};",
        "pub(super) const DEFEAT_TO_MAP_READY_RETAIL_FRAMES: u16 = "
        f"{defeat_to_map_ready};",
        "#[cfg(test)]",
        "pub(super) const ACCEPTED_ALL_DEFEATED_RETAIL_FRAME: u16 = "
        f"{accepted_defeat_frame};",
        "",
        f"pub(super) const ENTRY_LAST_RETAIL_FRAME: u16 = {ENTRY_LAST_RETAIL_FRAME};",
        f"pub(super) const LIVE_FIRST_RETAIL_FRAME: u16 = {LIVE_FIRST_RETAIL_FRAME};",
        f"pub(super) const LIVE_LAST_RETAIL_FRAME: u16 = {LIVE_LAST_RETAIL_FRAME};",
        "#[cfg(test)]",
        "pub(super) const FIRST_NATURAL_HIT_RETAIL_FRAME: u16 = "
        f"{neutral_last_retail_frame + RETAIL_FRAME_STEP};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "pub(super) const PLAYER_HANDOFF_POSITION: Vector3 = Vector3 {",
        f"    x: {PLAYER_HANDOFF_POSITION[0]:_},",
        f"    y: {PLAYER_HANDOFF_POSITION[1]:_},",
        f"    z: {PLAYER_HANDOFF_POSITION[2]:_},",
        "};",
        f"pub(super) const PLAYER_HANDOFF_PITCH: Angle = Angle::from_units({PLAYER_HANDOFF_PITCH});",
        f"pub(super) const PLAYER_HANDOFF_YAW: Angle = Angle::from_units({PLAYER_HANDOFF_YAW});",
        f"pub(super) const PLAYER_HANDOFF_BANK: Angle = Angle::from_units({PLAYER_HANDOFF_BANK});",
        f"pub(super) const PLAYER_HANDOFF_SPEED: u8 = {PLAYER_HANDOFF_SPEED};",
        f"pub(super) const PLAYER_HANDOFF_BANK_RECOVERY: i8 = {PLAYER_HANDOFF_BANK};",
        f"pub(super) const PLAYER_NEUTRAL_TARGET_SPEED: u8 = {PLAYER_NEUTRAL_TARGET_SPEED};",
        f"pub(super) const PLAYER_FAST_ACCELERATION: u8 = {PLAYER_FAST_ACCELERATION};",
        f"pub(super) const PLAYER_FAST_SPEED_LIMIT: u8 = {PLAYER_FAST_SPEED_LIMIT};",
        "",
        "pub(super) const CAMERA_HANDOFF_POSITION: Vector3 = Vector3 {",
        f"    x: {CAMERA_HANDOFF_POSITION[0]:_},",
        f"    y: {CAMERA_HANDOFF_POSITION[1]:_},",
        f"    z: {CAMERA_HANDOFF_POSITION[2]:_},",
        "};",
        "pub(super) const CAMERA_FOLLOW_INITIAL_REAR_DISTANCE: i16 = -240;",
        "pub(super) const CAMERA_FOLLOW_REAR_DISTANCE_TARGET: i16 = 0;",
        "pub(super) const CAMERA_FOLLOW_REAR_DISTANCE_STEP: i16 = 30;",
        "pub(super) const CAMERA_FOLLOW_VERTICAL_OFFSET: i16 = -20;",
        "pub(super) const CAMERA_FOLLOW_VIEW_PITCH_SUBUNITS: u16 = 0;",
        "pub(super) const CAMERA_FOLLOW_VIEW_YAW_SUBUNITS: u16 = (-16_384i16) as u16;",
        "pub(super) const CAMERA_FOLLOW_VERTICAL_POSITION_SCALE: i16 = 2;",
        "pub(super) const CAMERA_FOLLOW_POSITION_SCALE_SHIFT: u32 = 1;",
        "pub(super) const CAMERA_CONTINUITY_TRANSLATION_DIVISOR: i16 = 16;",
        "pub(super) const CAMERA_ORIENTATION_COARSE_SHIFT: u32 = 8;",
        "",
        f"pub(super) const ENTRY_CAMERA_KEYFRAMES: [MissionCameraKeyframe; {len(entry_records)}] = [",
    ]
    for record in entry_records:
        lines.append(
            f"    mission_camera_keyframe({record.retail_frame}, "
            + ", ".join(f"{value:_}" for value in record.camera)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
            f"pub(super) const ENTRY_PLAYER_KEYFRAMES: [MissionPlayerKeyframe; {len(entry_records)}] = [",
        ]
    )
    for record in entry_records:
        lines.append(
            f"    mission_player_keyframe({record.retail_frame}, "
            + ", ".join(f"{value:_}" for value in record.player)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "pub(super) struct LiveFlightCadence {",
            "    pub player_updates: u8,",
            "    pub player_control_only: bool,",
            "    pub camera_updates: u8,",
            "    pub camera_anchor_only: bool,",
            "}",
            "",
            f"const PLAYER_SKIPPED_UPDATE_RETAIL_FRAMES: [u16; {len(player_skip_frames)}] = [",
            *[f"    {frame}," for frame in player_skip_frames],
            "];",
            f"const PLAYER_CONTROL_ONLY_RETAIL_FRAMES: [u16; {len(player_control_only_frames)}] = [",
            *[f"    {frame}," for frame in player_control_only_frames],
            "];",
            f"const CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES: [u16; {len(camera_skip_frames)}] = [",
            *[f"    {frame}," for frame in camera_skip_frames],
            "];",
            f"const CAMERA_ANCHOR_ONLY_RETAIL_FRAMES: [u16; {len(camera_anchor_only_frames)}] = [",
            *[f"    {frame}," for frame in camera_anchor_only_frames],
            "];",
            "",
            "pub(super) fn live_flight_cadence(retail_frame: u16) -> Option<LiveFlightCadence> {",
            "    let offset = retail_frame.checked_sub(LIVE_FIRST_RETAIL_FRAME)?;",
            "    if retail_frame > LIVE_LAST_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 {",
            "        return None;",
            "    }",
            "    Some(LiveFlightCadence {",
            "        player_updates: u8::from(",
            "            !PLAYER_SKIPPED_UPDATE_RETAIL_FRAMES.contains(&retail_frame),",
            "        ),",
            "        player_control_only: PLAYER_CONTROL_ONLY_RETAIL_FRAMES.contains(&retail_frame),",
            "        camera_updates: u8::from(",
            "            !CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES.contains(&retail_frame),",
            "        ),",
            "        camera_anchor_only: CAMERA_ANCHOR_ONLY_RETAIL_FRAMES.contains(&retail_frame),",
            "    })",
            "}",
            "",
            f"const PLAYER_AMBIENT_BANK_PERIOD: u8 = {len(PLAYER_AMBIENT_BANK_WAVE)};",
            f"const PLAYER_AMBIENT_BANK_WAVE: [i8; {len(PLAYER_AMBIENT_BANK_WAVE)}] = [{bank_wave}];",
            "",
            "pub(super) fn advance_player_ambient_bank_phase(phase: u8, updates: u8) -> u8 {",
            "    phase.wrapping_add(updates) % PLAYER_AMBIENT_BANK_PERIOD",
            "}",
            "",
            "pub(super) fn player_ambient_bank(phase: u8) -> i8 {",
            "    PLAYER_AMBIENT_BANK_WAVE[usize::from(phase % PLAYER_AMBIENT_BANK_PERIOD)]",
            "}",
            "",
            "/// The source controller reduces the scripted bank spring to three",
            "/// quarters by summing its arithmetic half and quarter.",
            "pub(super) fn decay_player_bank_recovery(value: i8) -> i8 {",
            "    let half = value >> 1;",
            "    half.wrapping_add(half >> 1)",
            "}",
            "",
            "const CAMERA_AMBIENT_HEIGHT_PERIOD: u8 = 32;",
            f"const CAMERA_AMBIENT_HEIGHT_WAVE: [i8; 32] = [{camera_ambient_wave}];",
            "",
            "pub(super) fn advance_camera_ambient_height(phase: u8, height: i16) -> (u8, i16) {",
            "    let phase = phase.wrapping_add(1) % CAMERA_AMBIENT_HEIGHT_PERIOD;",
            "    (",
            "        phase,",
            "        height.wrapping_add(i16::from(",
            "            CAMERA_AMBIENT_HEIGHT_WAVE[usize::from(phase)],",
            "        )),",
            "    )",
            "}",
            "",
        ]
    )
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--continuation-trace", action="append", type=Path, default=[])
    parser.add_argument("--neutral-trace", type=Path, default=DEFAULT_NEUTRAL_TRACE)
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    traces = (args.trace, *args.continuation_trace)
    records, return_frame, map_ready_frame = load(traces)
    neutral_records, neutral_last_retail_frame = load_neutral(args.neutral_trace)
    if args.compact_output is not None:
        write_compact(traces, args.compact_output, records, return_frame, map_ready_frame)
    generated = rust_source(
        ", ".join(trace.name for trace in traces),
        records,
        args.neutral_trace.name,
        neutral_records,
        neutral_last_retail_frame,
        return_frame,
        map_ready_frame,
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
        f"{action} {args.output}: {len(neutral_records)} neutral samples, "
        f"retail frames {neutral_records[0].retail_frame}..{neutral_records[-1].retail_frame}, "
        f"return {return_frame}, map ready {map_ready_frame}"
    )


if __name__ == "__main__":
    main()
