#!/usr/bin/env python3
"""Generate native flight logic for SF2's campaign-missile interception.

The oracle callback stream is reduced to steering, spin, movement, visibility,
and departure operations. Generated Rust is accepted only when an independent
flat-state replay matches every retained four-frame missile pose in
``missile_interception.trace``.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

from generate_capital_continuation import (
    TRIG_SOURCE,
    mulslog,
    rust_table,
    signed_word,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "missile_interception_target_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "missile_interception.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "missile_interception_targets.rs"
)

ACTORS = ("lead", "upper", "lower")
SOURCE_ACTORS = {"05F4": "lead", "05B5": "upper", "0576": "lower"}
SOURCE_SHAPE = "D068"
RAW_SAMPLE_ORIGIN = 24_068
RETAIL_FRAME_STEP = 4
START_RETAIL_FRAME = 64
LOGIC_ANCHOR_RETAIL_FRAME = 132
FINAL_FRAMES = {"lead": 2_564, "upper": 2_416, "lower": 2_468}
DEPARTURE_FRAMES = {
    actor: frame + RETAIL_FRAME_STEP for actor, frame in FINAL_FRAMES.items()
}


@dataclass(frozen=True)
class PoseSample:
    retail_frame: int
    missiles: tuple[tuple[int, ...] | None, ...]


@dataclass(frozen=True)
class MoveEvent:
    elapsed: int
    actor: str
    pose: tuple[int, ...]


@dataclass
class FlightState:
    x: int
    y: int
    z: int
    pitch: int
    yaw: int
    roll: int
    speed: int
    visible: bool = False

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


Action = str


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def read_pose_samples(path: Path) -> list[PoseSample]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        missiles = tuple(
            None if value == "-" else tuple(map(int, value.split(",")))
            for value in values["missiles"].split("/")
        )
        if len(missiles) != len(ACTORS):
            raise SystemExit("missile-interception fixture has malformed targets")
        result.append(PoseSample(int(values["retail_frame"]), missiles))
    if not result:
        raise SystemExit(f"missile-interception pose fixture is empty: {path}")
    return result


def read_raw_move_events(path: Path) -> list[MoveEvent]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        actor = SOURCE_ACTORS.get(values.get("object", ""))
        if (
            actor is None
            or values.get("shape") != SOURCE_SHAPE
            or values.get("event") != "move"
        ):
            continue
        pose = tuple(map(int, values["pose"].split(",")))
        if len(pose) != 7:
            raise SystemExit(f"malformed missile pose in raw logic: {line}")
        result.append(MoveEvent(int(values["elapsed"]), actor, pose))
    if not result:
        raise SystemExit(f"raw missile logic contains no movement events: {path}")
    return result


def read_logic_fixture(path: Path) -> dict[int, dict[str, list[Action]]]:
    schedule = defaultdict(lambda: {actor: [] for actor in ACTORS})
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        frame = int(values["retail_frame"])
        actor = values["actor"]
        if actor not in ACTORS:
            raise SystemExit(f"unknown missile actor in logic fixture: {actor}")
        actions = [] if values["actions"] == "-" else values["actions"].split(",")
        for action in actions:
            if action not in {
                "Present",
                "BeginLowerFlight",
                "SteerClimb",
                "SteerDive",
                "SteerClockwise",
                "SteerCounterClockwise",
                "Spin",
                "Move",
                "Depart",
            }:
                raise SystemExit(f"unknown missile action in logic fixture: {action}")
        schedule[frame][actor].extend(actions)
    if not schedule:
        raise SystemExit(f"missile logic fixture is empty: {path}")
    return schedule


SINE = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
COSINE = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))


def flight_velocity(state: FlightState) -> tuple[int, int, int]:
    source_yaw = (-state.yaw) & 255
    pitch_cosine = COSINE[state.pitch]
    return (
        mulslog(mulslog(state.speed, SINE[source_yaw]), pitch_cosine),
        mulslog(state.speed, SINE[state.pitch]),
        mulslog(mulslog(state.speed, COSINE[source_yaw]), pitch_cosine),
    )


def apply_action(
    state: FlightState, action: Action, lower_flight_pose: tuple[int, ...]
) -> None:
    if action == "Present":
        state.visible = True
    elif action == "BeginLowerFlight":
        (
            state.x,
            state.y,
            state.z,
            state.pitch,
            state.yaw,
            state.roll,
            state.speed,
        ) = lower_flight_pose
    elif action == "SteerClimb":
        state.pitch = (state.pitch + 1) & 255
    elif action == "SteerDive":
        state.pitch = (state.pitch - 1) & 255
    elif action == "SteerClockwise":
        state.yaw = (state.yaw + 1) & 255
    elif action == "SteerCounterClockwise":
        state.yaw = (state.yaw - 1) & 255
    elif action == "Spin":
        state.roll = (state.roll + 2) & 255
    elif action == "Move":
        velocity = flight_velocity(state)
        state.x = signed_word(state.x + velocity[0])
        state.y = signed_word(state.y + velocity[1])
        state.z = signed_word(state.z + velocity[2])
    elif action == "Depart":
        state.visible = False
    else:
        raise AssertionError(action)


def angle_delta(start: int, end: int) -> int:
    return ((end - start + 128) & 255) - 128


def append_rotation_actions(
    actions: list[Action],
    state: FlightState,
    target: tuple[int, ...],
    lower_flight_pose: tuple[int, ...],
) -> None:
    if state.pose()[:3] != target[:3] or state.speed != target[6]:
        raise SystemExit(
            f"rotation boundary moved unexpectedly: {state.pose()} -> {target}"
        )
    pitch_delta = angle_delta(state.pitch, target[3])
    yaw_delta = angle_delta(state.yaw, target[4])
    roll_delta = (target[5] - state.roll) & 255
    if pitch_delta and yaw_delta:
        raise SystemExit(
            f"missile changed both steering axes at one boundary: {state.pose()} -> {target}"
        )
    if abs(pitch_delta) > 1 or abs(yaw_delta) > 1:
        raise SystemExit(f"untyped missile steering step: {state.pose()} -> {target}")
    if roll_delta % 2 or roll_delta > 4:
        raise SystemExit(f"untyped missile spin step: {state.pose()} -> {target}")
    if pitch_delta > 0:
        actions.append("SteerClimb")
    elif pitch_delta < 0:
        actions.append("SteerDive")
    elif yaw_delta > 0:
        actions.append("SteerClockwise")
    elif yaw_delta < 0:
        actions.append("SteerCounterClockwise")
    actions.extend("Spin" for _ in range(roll_delta // 2))
    added = abs(pitch_delta) + abs(yaw_delta) + roll_delta // 2
    if added:
        for action in actions[-added:]:
            apply_action(state, action, lower_flight_pose)
    if state.pose() != target:
        raise SystemExit(f"rotation replay failed: {state.pose()} != {target}")


def sample_by_frame(samples: list[PoseSample]) -> dict[int, PoseSample]:
    return {sample.retail_frame: sample for sample in samples}


def build_schedule(
    events: list[MoveEvent], samples: list[PoseSample]
) -> dict[int, dict[str, list[Action]]]:
    poses = sample_by_frame(samples)
    anchor = poses[START_RETAIL_FRAME]
    lower_flight_pose = poses[START_RETAIL_FRAME + RETAIL_FRAME_STEP].missiles[2]
    if lower_flight_pose is None:
        raise SystemExit("lower missile has no initialized flight pose")
    schedule = defaultdict(lambda: {actor: [] for actor in ACTORS})
    states = {}
    for index, actor in enumerate(ACTORS):
        pose = anchor.missiles[index]
        if pose is None:
            raise SystemExit(f"missing missile at initial frame: {actor}")
        states[actor] = FlightState(*pose)
        schedule[START_RETAIL_FRAME][actor].append("Present")

    # The saved operation stream begins at frame 132. Before that boundary the
    # targets only execute the same straight movement and two-unit spin used by
    # the captured stream. Recover its cooperative cadence mechanically from
    # the accepted four-frame poses; no position or orientation samples enter
    # the generated Rust action table.
    previous_frame = START_RETAIL_FRAME
    for frame in range(
        START_RETAIL_FRAME + RETAIL_FRAME_STEP,
        LOGIC_ANCHOR_RETAIL_FRAME + RETAIL_FRAME_STEP,
        RETAIL_FRAME_STEP,
    ):
        sample = poses[frame]
        for index, actor in enumerate(ACTORS):
            expected = sample.missiles[index]
            if expected is None:
                raise SystemExit(f"missile disappeared before logic capture: {actor}")
            state = states[actor]
            actions = schedule[frame][actor]
            if actor == "lower" and frame == START_RETAIL_FRAME + RETAIL_FRAME_STEP:
                actions.append("BeginLowerFlight")
                apply_action(state, actions[-1], lower_flight_pose)
            else:
                move_count = None
                for candidate in range(3):
                    trial = FlightState(*state.pose(), visible=state.visible)
                    for _ in range(candidate):
                        apply_action(trial, "Move", lower_flight_pose)
                    if trial.pose()[:3] == expected[:3]:
                        move_count = candidate
                        break
                if move_count is None:
                    raise SystemExit(
                        f"opening straight-flight cadence is not semantic at frame {frame} {actor}"
                    )
                for _ in range(move_count):
                    actions.append("Move")
                    apply_action(state, "Move", lower_flight_pose)
            append_rotation_actions(actions, state, expected, lower_flight_pose)
        previous_frame = frame
    if previous_frame != LOGIC_ANCHOR_RETAIL_FRAME:
        raise AssertionError(previous_frame)

    events_by_actor = {
        actor: sorted((event for event in events if event.actor == actor), key=lambda e: e.elapsed)
        for actor in ACTORS
    }
    used_events = {actor: 0 for actor in ACTORS}
    pending_move = {actor: False for actor in ACTORS}
    for frame in range(
        LOGIC_ANCHOR_RETAIL_FRAME + RETAIL_FRAME_STEP,
        max(FINAL_FRAMES.values()) + RETAIL_FRAME_STEP,
        RETAIL_FRAME_STEP,
    ):
        previous_frame = frame - RETAIL_FRAME_STEP
        for index, actor in enumerate(ACTORS):
            if frame > FINAL_FRAMES[actor]:
                continue
            expected = poses[frame].missiles[index]
            if expected is None:
                raise SystemExit(f"missile departed before certified final frame: {actor}")
            state = states[actor]
            actions = schedule[frame][actor]
            if pending_move[actor]:
                actions.append("Move")
                apply_action(state, "Move", lower_flight_pose)
                pending_move[actor] = False
            group = [
                event
                for event in events_by_actor[actor]
                if RAW_SAMPLE_ORIGIN + previous_frame
                <= event.elapsed
                < RAW_SAMPLE_ORIGIN + frame
            ]
            used_events[actor] += len(group)
            for event_index, event in enumerate(group):
                append_rotation_actions(actions, state, event.pose, lower_flight_pose)
                is_last = event_index + 1 == len(group)
                if is_last and state.pose()[:3] == expected[:3]:
                    trial = FlightState(*state.pose(), visible=state.visible)
                    apply_action(trial, "Move", lower_flight_pose)
                    if trial.pose()[:3] != expected[:3]:
                        pending_move[actor] = True
                        continue
                actions.append("Move")
                apply_action(state, "Move", lower_flight_pose)
            append_rotation_actions(actions, state, expected, lower_flight_pose)

    for actor in ACTORS:
        if pending_move[actor]:
            raise SystemExit(f"missile retained a cooperative movement at departure: {actor}")
        expected_events = sum(
            RAW_SAMPLE_ORIGIN + LOGIC_ANCHOR_RETAIL_FRAME
            <= event.elapsed
            < RAW_SAMPLE_ORIGIN + FINAL_FRAMES[actor]
            for event in events_by_actor[actor]
        )
        if used_events[actor] != expected_events:
            raise SystemExit(
                f"missile event coverage mismatch for {actor}: "
                f"used {used_events[actor]}, expected {expected_events}"
            )
        schedule[DEPARTURE_FRAMES[actor]][actor].append("Depart")
    return schedule


def replay(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> int:
    poses = sample_by_frame(samples)
    anchor = poses[START_RETAIL_FRAME]
    lower_flight_pose = poses[START_RETAIL_FRAME + RETAIL_FRAME_STEP].missiles[2]
    assert lower_flight_pose is not None
    states = {
        actor: FlightState(*anchor.missiles[index])
        for index, actor in enumerate(ACTORS)
        if anchor.missiles[index] is not None
    }
    retained = 0
    failures = []
    for frame in range(
        START_RETAIL_FRAME,
        max(DEPARTURE_FRAMES.values()) + RETAIL_FRAME_STEP,
        RETAIL_FRAME_STEP,
    ):
        sample = poses[frame]
        for index, actor in enumerate(ACTORS):
            state = states[actor]
            for action in schedule[frame][actor]:
                apply_action(state, action, lower_flight_pose)
            expected = sample.missiles[index]
            if state.visible != (expected is not None):
                failures.append(
                    (frame, actor, f"visible={state.visible}", f"visible={expected is not None}")
                )
                continue
            if expected is None:
                continue
            retained += 1
            if state.pose() != expected:
                failures.append((frame, actor, state.pose(), expected))
    if failures:
        for frame, actor, actual, expected in failures[:16]:
            print(f"frame={frame} actor={actor} actual={actual} expected={expected}")
        first = failures[0]
        raise SystemExit(
            f"semantic missile replay diverges at frame {first[0]} "
            f"{first[1]} ({len(failures)} mismatches)"
        )
    return retained


def write_logic_fixture(
    raw_source: Path,
    pose_source: Path,
    output: Path,
    schedule: dict[int, dict[str, list[Action]]],
) -> None:
    lines = [
        "# Compact semantic oracle evidence for the campaign missiles.",
        f"# Raw logic SHA-256: {hashlib.sha256(raw_source.read_bytes()).hexdigest()}",
        f"# Pose fixture SHA-256: {hashlib.sha256(pose_source.read_bytes()).hexdigest()}",
        "# Opening straight-flight cadence precedes the saved operation stream and is pose-derived.",
    ]
    for frame in sorted(schedule):
        for actor in ACTORS:
            actions = schedule[frame][actor]
            if actions:
                lines.append(
                    f"retail_frame={frame} actor={actor} actions={','.join(actions)}"
                )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def grouped(value: int) -> str:
    return f"{value:_}"


def rust_action(action: Action) -> str:
    return {
        "Present": "InterceptionMissileAction::Present",
        "BeginLowerFlight": "InterceptionMissileAction::BeginLowerFlight",
        "SteerClimb": "InterceptionMissileAction::Steer(InterceptionMissileSteering::Climb)",
        "SteerDive": "InterceptionMissileAction::Steer(InterceptionMissileSteering::Dive)",
        "SteerClockwise": "InterceptionMissileAction::Steer(InterceptionMissileSteering::Clockwise)",
        "SteerCounterClockwise": "InterceptionMissileAction::Steer(InterceptionMissileSteering::CounterClockwise)",
        "Spin": "InterceptionMissileAction::Spin",
        "Move": "InterceptionMissileAction::Move",
        "Depart": "InterceptionMissileAction::Depart",
    }[action]


def format_rust(source: str) -> str:
    result = subprocess.run(
        [
            "rustfmt",
            "--edition",
            "2021",
            "--config",
            "skip_children=true",
            "--emit",
            "stdout",
        ],
        input=source,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"rustfmt failed for generated missile schedule:\n{result.stderr}")
    return result.stdout


def rust_source(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> str:
    poses = sample_by_frame(samples)
    anchor = poses[START_RETAIL_FRAME]
    lower_flight_pose = poses[START_RETAIL_FRAME + RETAIL_FRAME_STEP].missiles[2]
    assert lower_flight_pose is not None
    end_frame = max(DEPARTURE_FRAMES.values())
    flattened = []
    ranges = []
    for frame in range(START_RETAIL_FRAME, end_frame + RETAIL_FRAME_STEP, RETAIL_FRAME_STEP):
        frame_ranges = []
        for actor in ACTORS:
            start = len(flattened)
            actions = schedule[frame][actor]
            flattened.extend(actions)
            frame_ranges.append((start, len(actions)))
        ranges.append(frame_ranges)

    def pose_source(pose: tuple[int, ...]) -> str:
        return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"

    lines = [
        "//! Generated semantic flight schedule for the campaign-missile interception.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, InterceptionMissileAction, InterceptionMissileSteering,",
        "    MissionEncounterPose,",
        "};",
        "",
        f"pub(super) const START_RETAIL_FRAME: u16 = {START_RETAIL_FRAME};",
        "#[cfg(test)]",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped(end_frame)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "#[cfg(test)]",
        f"pub(super) const DEPARTURE_RETAIL_FRAMES: [u16; 3] = [{grouped(DEPARTURE_FRAMES['lead'])}, {grouped(DEPARTURE_FRAMES['upper'])}, {grouped(DEPARTURE_FRAMES['lower'])}];",
        "",
        "pub(super) const INITIAL_POSES: [MissionEncounterPose; 3] = [",
    ]
    lines.extend(
        f"    {pose_source(pose)}," for pose in anchor.missiles if pose is not None
    )
    lines.extend(
        [
            "];",
            "",
            "pub(super) const LOWER_FLIGHT_POSE: MissionEncounterPose =",
            f"    {pose_source(lower_flight_pose)};",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "struct ActionRange { start: u16, len: u8 }",
            "",
            "impl ActionRange {",
            "    const fn new(start: u16, len: u8) -> Self { Self { start, len } }",
            "}",
            "",
            f"static ACTIONS: [InterceptionMissileAction; {len(flattened)}] = [",
        ]
    )
    lines.extend(f"    {rust_action(action)}," for action in flattened)
    lines.extend(["];"])
    lines.extend(["", f"static TICKS: [[ActionRange; 3]; {len(ranges)}] = ["])
    for frame_ranges in ranges:
        entries = ", ".join(
            f"ActionRange::new({start}, {length})" for start, length in frame_ranges
        )
        lines.append(f"    [{entries}],")
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16, actor: usize) -> &'static [InterceptionMissileAction] {",
            "    let Some(offset) = retail_frame.checked_sub(START_RETAIL_FRAME) else { return &[]; };",
            "    if offset % RETAIL_FRAME_STEP != 0 { return &[]; }",
            "    let Some(ranges) = TICKS.get(usize::from(offset / RETAIL_FRAME_STEP)) else { return &[]; };",
            "    let Some(range) = ranges.get(actor) else { return &[]; };",
            "    &ACTIONS[usize::from(range.start)..usize::from(range.start) + usize::from(range.len)]",
            "}",
            "",
        ]
    )
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("logic", type=Path, nargs="?", default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--compact-output", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    samples = read_pose_samples(args.pose_fixture)
    if args.import_raw is not None:
        events = read_raw_move_events(args.import_raw)
        schedule = build_schedule(events, samples)
        if args.compact_output is None:
            raise SystemExit("--import-raw requires --compact-output")
        retained = replay(schedule, samples)
        write_logic_fixture(
            args.import_raw, args.pose_fixture, args.compact_output, schedule
        )
        print(
            f"imported {args.compact_output}: {len(events)} move events, "
            f"{retained} retained pose boundaries"
        )
        return

    schedule = read_logic_fixture(args.logic)
    retained = replay(schedule, samples)
    generated = rust_source(schedule, samples)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"{action} {args.output}: {retained} retained pose boundaries, "
        f"retail frames {START_RETAIL_FRAME}..{max(DEPARTURE_FRAMES.values())}"
    )


if __name__ == "__main__":
    main()
