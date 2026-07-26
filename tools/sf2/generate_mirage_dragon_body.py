#!/usr/bin/env python3
"""Reduce Mirage Dragon handler work to a semantic articulated-body schedule.

The raw oracle stream contains source object identities and callback
addresses. The compact fixture and generated Rust retain only presentation
frames, typed operations, and one-based articulated-part ordinals.
"""

from __future__ import annotations

import argparse
import hashlib
from collections import defaultdict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = Path(__file__).with_name("fixtures") / "mirage_dragon_body_logic.trace"
DEFAULT_POSE_FIXTURE = Path(__file__).with_name("fixtures") / "mirage_dragon_segments.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "mirage_dragon_body_actions.rs"
)
PRESENTATION_FRAME_STEP = 4
FOLLOW_OPERATION_START_RETAIL_FRAME = 75
FOLLOW_PRESENTATION_START_RETAIL_FRAME = 76
FOLLOW_PRESENTATION_END_RETAIL_FRAME = 564
DEPARTURE_START_RETAIL_FRAME = 568
DEPARTURE_END_RETAIL_FRAME = 632
DEPARTURE_VELOCITIES = (
    (-52, -144, -24),
    (-12, -156, -12),
    (-4, -156, 0),
    (0, -152, 32),
    (4, -140, 68),
    (8, -96, 120),
    (12, 28, 148),
    (8, 120, 96),
    (4, 144, 60),
)
DEPARTURE_PITCH_STEP = 32
DEPARTURE_YAW_STEP = 16
DEPARTURE_ROLL_STEP = 8
PITCH_WRITE_SOURCE = "7F:BD35"
YAW_WRITE_SOURCE = "7F:BD45"
POSITION_WRITE_SOURCE = "7F:BDC4"
SOURCE_OBJECTS = (
    "05B5",
    "05F4",
    "0633",
    "0672",
    "06B1",
    "06F0",
    "072F",
    "076E",
    "07AD",
)
SOURCE_OBJECT_ORDINALS = {
    source_object: ordinal
    for ordinal, source_object in enumerate(SOURCE_OBJECTS, start=1)
}
HEAD_OBJECT = "0576"
HEAD_NATIVE_START_RETAIL_FRAME = 64
HEAD_FACE_YAW_WRITE_SOURCE = "7F:87C4"
HEAD_VERTICAL_WRITE_SOURCE = "07:F7B0"
HEAD_ORBIT_PATH = "F61A"
HEAD_ORBIT_MOVE_PATH = "F616"
HEAD_ORIENTED_MOVE_PATHS = {"F5F5", "F602", HEAD_ORBIT_MOVE_PATH}
HEAD_POSITION_Y_LOW = int(HEAD_OBJECT, 16) + 14
HEAD_POSITION_Y_HIGH = HEAD_POSITION_Y_LOW + 1

Action = tuple[str, tuple[int, ...]]


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def import_raw(source: Path, start_elapsed: int) -> dict[int, list[Action]]:
    schedule: dict[int, list[Action]] = defaultdict(list)
    pending_head_orbit_position: tuple[int, int, int] | None = None
    pending_head_vertical: tuple[int, int] | None = None

    def append_action(retail_frame: int, operation: str, *arguments: int) -> None:
        presentation_frame = (
            retail_frame // PRESENTATION_FRAME_STEP + 1
        ) * PRESENTATION_FRAME_STEP
        if presentation_frame > FOLLOW_PRESENTATION_END_RETAIL_FRAME:
            return
        schedule[presentation_frame].append((operation, arguments))

    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        source_object = values.get("object")
        if source_object == HEAD_OBJECT and "elapsed" in values:
            pose_values = values.get("pose", "-").split(",")
            line_position = (
                tuple(int(value) for value in pose_values[:3])
                if len(pose_values) >= 3 and pose_values[0] != "-"
                else None
            )
            retail_frame = int(values["elapsed"]) - start_elapsed
            if (
                HEAD_NATIVE_START_RETAIL_FRAME
                <= retail_frame
                < DEPARTURE_START_RETAIL_FRAME
            ):
                event = values.get("event")
                if event == "chase-angle":
                    extension = bytes.fromhex(values["extension"])
                    if values.get("path") == "F942":
                        append_action(retail_frame, "head-pitch", extension[20])
                    elif values.get("path") in {"F94B", "F95F"}:
                        append_action(retail_frame, "head-yaw", extension[21])
                    else:
                        raise SystemExit(
                            f"untyped Mirage Dragon head chase path: {values.get('path')}"
                        )
                elif (
                    event == "object-state-write"
                    and values.get("source") == "main-work"
                    and values.get("offset") == "14"
                    and values.get("host") == HEAD_FACE_YAW_WRITE_SOURCE
                ):
                    append_action(retail_frame, "head-face-yaw", int(values["value"]))
                elif (
                    event == "object-state-write"
                    and values.get("source") == "main-work"
                    and values.get("offset") == "12"
                ):
                    append_action(retail_frame, "head-set-pitch", int(values["value"]))
                elif (
                    event == "object-state-write"
                    and values.get("source") == "main-work"
                    and values.get("offset") == "14"
                ):
                    append_action(retail_frame, "head-set-yaw", int(values["value"]))
                elif (
                    event == "main-position-write"
                    and values.get("source") == HEAD_VERTICAL_WRITE_SOURCE
                ):
                    address = int(values["address"], 16)
                    if address == HEAD_POSITION_Y_LOW:
                        if line_position is None:
                            raise SystemExit("Mirage Dragon vertical step lacks prior position")
                        pending_head_vertical = (line_position[1], int(values["value"]))
                    elif address == HEAD_POSITION_Y_HIGH:
                        if pending_head_vertical is None:
                            raise SystemExit("Mirage Dragon vertical step is incomplete")
                        prior_y, low_byte = pending_head_vertical
                        encoded_y = low_byte | (int(values["value"]) << 8)
                        final_y = (
                            encoded_y
                            if encoded_y < 32768
                            else encoded_y - 65536
                        )
                        append_action(
                            retail_frame,
                            "head-vertical",
                            final_y - prior_y,
                        )
                        pending_head_vertical = None
                elif event == "projectile-orbit-pitch":
                    if values.get("path") != HEAD_ORBIT_PATH:
                        raise SystemExit(
                            f"untyped Mirage Dragon head orbit path: {values.get('path')}"
                        )
                    pending_head_orbit_position = tuple(
                        int(value) for value in values["pose"].split(",")[:3]
                    )
                elif event == "move":
                    if values.get("path") == HEAD_ORBIT_MOVE_PATH:
                        if pending_head_orbit_position is None:
                            raise SystemExit("Mirage Dragon orbit move lacks its orbit entry")
                        position = tuple(
                            int(value) for value in values["pose"].split(",")[:3]
                        )
                        append_action(
                            retail_frame,
                            "head-orbit",
                            *(end - start for start, end in zip(
                                pending_head_orbit_position, position
                            )),
                        )
                        pending_head_orbit_position = None
                    if values.get("path") in HEAD_ORIENTED_MOVE_PATHS:
                        append_action(retail_frame, "head-move")
                    else:
                        append_action(
                            retail_frame,
                            "head-linear-move",
                            *(int(value) for value in values["velocity"].split(",")),
                        )
        if source_object not in SOURCE_OBJECT_ORDINALS:
            continue
        object_address = int(source_object, 16)
        operation = None
        if (
            values.get("event") == "object-state-write"
            and values.get("source") == "main-work"
            and values.get("offset") == "12"
            and values.get("host") == PITCH_WRITE_SOURCE
        ):
            operation = "pitch"
        elif (
            values.get("event") == "object-state-write"
            and values.get("source") == "main-work"
            and values.get("offset") == "14"
            and values.get("host") == YAW_WRITE_SOURCE
        ):
            operation = "yaw"
        elif (
            values.get("event") == "main-position-write"
            and values.get("source") == POSITION_WRITE_SOURCE
            and int(values["address"], 16) == object_address + 15
        ):
            operation = "position"
        if operation is None:
            continue
        retail_frame = int(values["elapsed"]) - start_elapsed
        if not (
            FOLLOW_OPERATION_START_RETAIL_FRAME
            <= retail_frame
            < DEPARTURE_START_RETAIL_FRAME
        ):
            continue
        append_action(retail_frame, operation, SOURCE_OBJECT_ORDINALS[source_object])

    expected_frames = range(
        FOLLOW_PRESENTATION_START_RETAIL_FRAME,
        FOLLOW_PRESENTATION_END_RETAIL_FRAME + 1,
        PRESENTATION_FRAME_STEP,
    )
    missing = [frame for frame in expected_frames if frame not in schedule]
    if missing:
        raise SystemExit(f"raw follow schedule is missing presentation frames: {missing}")
    return dict(sorted(schedule.items()))


def compact_source(
    raw_source: Path,
    start_elapsed: int,
    schedule: dict[int, list[Action]],
) -> str:
    lines = [
        "# Compact semantic Mirage Dragon follow schedule.",
        f"# Raw source SHA-256: {hashlib.sha256(raw_source.read_bytes()).hexdigest()}",
        f"# sample_start_elapsed={start_elapsed}",
    ]
    lines.extend(
        f"retail_frame={retail_frame} actions="
        + ",".join(
            ":".join((operation, *(str(argument) for argument in arguments)))
            for operation, arguments in actions
        )
        for retail_frame, actions in schedule.items()
    )
    return "\n".join(lines) + "\n"


def load_fixture(path: Path) -> dict[int, list[Action]]:
    schedule = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        retail_frame = int(values["retail_frame"])
        schedule[retail_frame] = [
            (parts[0], tuple(int(argument) for argument in parts[1:]))
            for value in values["actions"].split(",")
            for parts in [value.split(":")]
        ]
    expected_frames = list(
        range(
            FOLLOW_PRESENTATION_START_RETAIL_FRAME,
            FOLLOW_PRESENTATION_END_RETAIL_FRAME + 1,
            PRESENTATION_FRAME_STEP,
        )
    )
    if list(schedule) != expected_frames:
        raise SystemExit("compact follow schedule does not cover every presentation frame")
    return schedule


def departure_schedule(path: Path) -> dict[int, list[Action]]:
    poses: dict[int, tuple[tuple[int, ...] | None, ...]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        retail_frame = int(values["retail_frame"])
        if not (
            DEPARTURE_START_RETAIL_FRAME - PRESENTATION_FRAME_STEP
            <= retail_frame
            <= DEPARTURE_END_RETAIL_FRAME
        ):
            continue
        part_values = []
        for role in (
            "body_segment_1",
            "body_segment_2",
            "body_segment_3",
            "body_segment_4",
            "body_segment_5",
            "body_segment_6",
            "body_segment_7",
            "body_segment_8",
            "tail",
        ):
            value = values[role]
            part_values.append(
                None if value == "-" else tuple(map(int, value.split(",")))
            )
        poses[retail_frame] = tuple(part_values)

    expected_frames = list(
        range(
            DEPARTURE_START_RETAIL_FRAME - PRESENTATION_FRAME_STEP,
            DEPARTURE_END_RETAIL_FRAME + 1,
            PRESENTATION_FRAME_STEP,
        )
    )
    if list(poses) != expected_frames:
        raise SystemExit("departure pose evidence does not cover every presentation frame")

    schedule: dict[int, list[Action]] = {}
    for retail_frame in range(
        DEPARTURE_START_RETAIL_FRAME,
        DEPARTURE_END_RETAIL_FRAME + 1,
        PRESENTATION_FRAME_STEP,
    ):
        actions = []
        prior = poses[retail_frame - PRESENTATION_FRAME_STEP]
        current = poses[retail_frame]
        for ordinal, (prior_pose, current_pose, velocity) in enumerate(
            zip(prior, current, DEPARTURE_VELOCITIES, strict=True),
            start=1,
        ):
            if retail_frame == DEPARTURE_START_RETAIL_FRAME:
                if prior_pose is None or current_pose is None:
                    raise SystemExit("every Mirage Dragon part must begin departure")
                actions.append(("departure-begin", (ordinal,)))
                continue
            if prior_pose is None:
                if current_pose is not None:
                    raise SystemExit("a departed Mirage Dragon part reappeared")
                continue
            if current_pose is None:
                actions.append(("departure-remove", (ordinal,)))
                continue

            deltas = tuple(
                current_value - prior_value
                for prior_value, current_value in zip(
                    prior_pose[:3], current_pose[:3], strict=True
                )
            )
            nonzero_axes = [
                (delta, component)
                for delta, component in zip(deltas, velocity, strict=True)
                if component != 0
            ]
            motion_steps = nonzero_axes[0][0] // nonzero_axes[0][1]
            if motion_steps < 0 or any(
                delta != component * motion_steps
                for delta, component in zip(deltas, velocity, strict=True)
            ):
                raise SystemExit(
                    f"departure motion at frame {retail_frame} part {ordinal} "
                    "does not match its static velocity"
                )
            actions.extend(
                ("departure-position", (ordinal,)) for _ in range(motion_steps)
            )
            for operation, before, after, step in (
                ("departure-pitch", prior_pose[3], current_pose[3], DEPARTURE_PITCH_STEP),
                ("departure-yaw", prior_pose[4], current_pose[4], DEPARTURE_YAW_STEP),
                ("departure-roll", prior_pose[5], current_pose[5], DEPARTURE_ROLL_STEP),
            ):
                delta = (after - before) % 256
                if delta % step != 0:
                    raise SystemExit(
                        f"{operation} at frame {retail_frame} part {ordinal} "
                        "does not match its static step"
                    )
                actions.extend(
                    (operation, (ordinal,)) for _ in range(delta // step)
                )
            if current_pose[6] != 216:
                raise SystemExit("departing Mirage Dragon part has the wrong speed")
        schedule[retail_frame] = actions
    return schedule


def rust_source(fixture: Path, schedule: dict[int, list[Action]]) -> str:
    flattened = [
        (operation, arguments)
        for retail_frame in schedule
        for operation, arguments in schedule[retail_frame]
    ]
    ranges = []
    start = 0
    for ordinals in schedule.values():
        ranges.append((start, len(ordinals)))
        start += len(ordinals)
    if len(flattened) > 65_535:
        raise SystemExit("follow action offset exceeds u16")
    if max((length for _, length in ranges), default=0) > 255:
        raise SystemExit("follow action count exceeds u8")

    lines = [
        "//! Generated semantic follow cadence for Mirage Dragon's articulated body.",
        "//! Source identities and callback addresses remain in oracle tooling.",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) enum Action {",
        "    HeadChasePitch(u8),",
        "    HeadChaseYaw(u8),",
        "    HeadFaceYaw(u8),",
        "    HeadSetPitch(u8),",
        "    HeadSetYaw(u8),",
        "    HeadVerticalOffset(i16),",
        "    HeadOrbitOffset(i16, i16, i16),",
        "    HeadMove,",
        "    HeadLinearMove(i16, i16, i16),",
        "    FacePitch(u8),",
        "    FaceYaw(u8),",
        "    FollowPosition(u8),",
        "    BeginDeparture(u8),",
        "    AdvanceDeparturePosition(u8),",
        "    AdvanceDeparturePitch(u8),",
        "    AdvanceDepartureYaw(u8),",
        "    AdvanceDepartureRoll(u8),",
        "    RemoveDepartedPart(u8),",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct ActionRange {",
        "    start: u16,",
        "    len: u8,",
        "}",
        "",
        f"const FIRST_RETAIL_FRAME: u16 = {FOLLOW_PRESENTATION_START_RETAIL_FRAME};",
        f"const LAST_RETAIL_FRAME: u16 = {DEPARTURE_END_RETAIL_FRAME};",
        f"const RETAIL_FRAME_STEP: u16 = {PRESENTATION_FRAME_STEP};",
        "",
        f"static RANGES: [ActionRange; {len(ranges)}] = [",
    ]
    lines.extend(
        f"    ActionRange {{ start: {start}, len: {length} }},"
        for start, length in ranges
    )
    lines.extend(
        [
            "];",
            "",
            f"static ACTIONS: [Action; {len(flattened)}] = [",
        ]
    )
    variants = {
        "head-pitch": "HeadChasePitch",
        "head-yaw": "HeadChaseYaw",
        "head-face-yaw": "HeadFaceYaw",
        "head-set-pitch": "HeadSetPitch",
        "head-set-yaw": "HeadSetYaw",
        "head-vertical": "HeadVerticalOffset",
        "head-orbit": "HeadOrbitOffset",
        "head-move": "HeadMove",
        "head-linear-move": "HeadLinearMove",
        "pitch": "FacePitch",
        "yaw": "FaceYaw",
        "position": "FollowPosition",
        "departure-begin": "BeginDeparture",
        "departure-position": "AdvanceDeparturePosition",
        "departure-pitch": "AdvanceDeparturePitch",
        "departure-yaw": "AdvanceDepartureYaw",
        "departure-roll": "AdvanceDepartureRoll",
        "departure-remove": "RemoveDepartedPart",
    }
    for operation, arguments in flattened:
        arguments_source = ", ".join(map(str, arguments))
        constructor = f"({arguments_source})" if arguments else ""
        lines.append(f"    Action::{variants[operation]}{constructor},")
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16) -> &'static [Action] {",
            "    if !(FIRST_RETAIL_FRAME..=LAST_RETAIL_FRAME).contains(&retail_frame)",
            "        || (retail_frame - FIRST_RETAIL_FRAME) % RETAIL_FRAME_STEP != 0",
            "    {",
            "        return &[];",
            "    }",
            "    let index = usize::from((retail_frame - FIRST_RETAIL_FRAME) / RETAIL_FRAME_STEP);",
            "    let range = RANGES[index];",
            "    let start = usize::from(range.start);",
            "    &ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
            "#[cfg(test)]",
            "mod tests {",
            "    use super::*;",
            "",
            "    #[test]",
            "    fn generated_schedule_has_valid_parts() {",
            "        for retail_frame in",
            "            (FIRST_RETAIL_FRAME..=LAST_RETAIL_FRAME).step_by(usize::from(RETAIL_FRAME_STEP))",
            "        {",
            "            let actions = actions(retail_frame);",
            "            assert!(!actions.is_empty());",
            "            assert!(actions.iter().all(|action| {",
            "                let ordinal = match action {",
            "                    Action::FacePitch(ordinal)",
            "                    | Action::FaceYaw(ordinal)",
            "                    | Action::FollowPosition(ordinal)",
            "                    | Action::BeginDeparture(ordinal)",
            "                    | Action::AdvanceDeparturePosition(ordinal)",
            "                    | Action::AdvanceDeparturePitch(ordinal)",
            "                    | Action::AdvanceDepartureYaw(ordinal)",
            "                    | Action::AdvanceDepartureRoll(ordinal)",
            "                    | Action::RemoveDepartedPart(ordinal) => Some(ordinal),",
            "                    Action::HeadChasePitch(_)",
            "                    | Action::HeadChaseYaw(_)",
            "                    | Action::HeadFaceYaw(_)",
            "                    | Action::HeadSetPitch(_)",
            "                    | Action::HeadSetYaw(_)",
            "                    | Action::HeadVerticalOffset(_)",
            "                    | Action::HeadOrbitOffset(_, _, _)",
            "                    | Action::HeadMove",
            "                    | Action::HeadLinearMove(_, _, _) => None,",
            "                };",
            "                ordinal.is_none_or(|ordinal| (1..=9).contains(ordinal))",
            "            }));",
            "        }",
            "    }",
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def write_or_check(path: Path, source: str, check: bool) -> None:
    if check:
        if not path.exists() or path.read_text(encoding="utf-8") != source:
            raise SystemExit(f"{path} is stale")
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(source, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--start-elapsed", type=int)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        if args.start_elapsed is None:
            raise SystemExit("--import-raw requires --start-elapsed")
        schedule = import_raw(args.import_raw, args.start_elapsed)
        write_or_check(
            args.fixture,
            compact_source(args.import_raw, args.start_elapsed, schedule),
            args.check,
        )
    schedule = load_fixture(args.fixture)
    schedule.update(departure_schedule(args.pose_fixture))
    write_or_check(args.output, rust_source(args.fixture, schedule), args.check)
    print(
        "Mirage Dragon follow schedule verified: "
        f"{len(schedule)} presentation frames, "
        f"{sum(map(len, schedule.values()))} semantic part actions"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
