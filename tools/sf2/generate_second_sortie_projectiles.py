#!/usr/bin/env python3
"""Generate native projectile dynamics for SF2's first re-engagement.

The raw emulator callbacks are reduced to five gameplay operations.  A
flat-state replay partitions those operations at the retail presentation
boundaries and must reproduce every retained projectile pose before Rust is
emitted.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import subprocess
from dataclasses import dataclass
from pathlib import Path

from generate_capital_continuation import (
    TRIG_SOURCE,
    chase_power,
    mulslog,
    rust_table,
    sf2_atan16,
    signed_word,
    trunc_div,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "second_sortie_projectile_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "second_sortie_reengagement.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "second_sortie_projectiles.rs"
)
ANGLE_SOURCE = REPO_ROOT / "rust" / "sf-core" / "src" / "aim_angle.rs"

RAW_SAMPLE_START_ELAPSED = 14_912
RETAIL_FRAME_STEP = 4
CONTRACTION_DISTANCE = 127
CRUISE_SPEED = 63
FLIGHT_POSITION_SCALE = 4
MAXIMUM_ACTIONS_PER_TICK = 24

ACTION_NAMES = {
    "projectile-orbit-pitch": "contract",
    "projectile-face-immediate": "face-immediate",
    "projectile-face-smooth": "face-smooth",
    "projectile-set-speed": "set-cruise-speed",
}
MOVEMENT_PHASES = {
    "EE9D": "advance-homing",
    "EEC6": "advance-aim",
    "EED2": "advance-cruise",
}
TARGET_ACTIONS = frozenset(("contract", "face-immediate", "face-smooth"))


@dataclass(frozen=True)
class PoseRecord:
    retail_frame: int
    player: tuple[int, ...]
    projectiles: dict[str, tuple[int, ...]]


@dataclass(frozen=True)
class ProjectileLifetime:
    source: str
    samples: tuple[tuple[int, tuple[int, ...]], ...]


@dataclass(frozen=True)
class LogicAction:
    elapsed: int
    source: str
    kind: str
    pose: tuple[int, ...]
    target: tuple[int, ...]

    @property
    def retail_frame(self) -> int:
        offset = self.elapsed - (RAW_SAMPLE_START_ELAPSED - 1)
        return ((offset + RETAIL_FRAME_STEP - 1) // RETAIL_FRAME_STEP) * RETAIL_FRAME_STEP


@dataclass(frozen=True)
class ScheduledAction:
    action: LogicAction
    retail_frame: int
    target_timing: str | None


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def parse_pose(value: str) -> tuple[int, ...]:
    pose = tuple(map(int, value.split(",")))
    if len(pose) != 7:
        raise SystemExit(f"malformed projectile pose: {value}")
    return pose


def read_pose_fixture(path: Path) -> tuple[list[PoseRecord], list[ProjectileLifetime]]:
    records = []
    active: dict[str, list[tuple[int, tuple[int, ...]]]] = {}
    lifetimes = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        retail_frame = int(values["retail_frame"])
        projectiles = {}
        if values["projectiles"] != "-":
            for item in values["projectiles"].split(";"):
                source, *pose_values = item.split(",")
                projectiles[source] = tuple(map(int, pose_values))
        records.append(
            PoseRecord(
                retail_frame=retail_frame,
                player=parse_pose(values["player"]),
                projectiles=projectiles,
            )
        )
        for source, pose in projectiles.items():
            active.setdefault(source, []).append((retail_frame, pose))
        for source in set(active).difference(projectiles):
            lifetimes.append(ProjectileLifetime(source, tuple(active.pop(source))))
    lifetimes.extend(
        ProjectileLifetime(source, tuple(samples)) for source, samples in active.items()
    )
    lifetimes.sort(key=lambda lifetime: lifetime.samples[0][0])
    if len(lifetimes) != 33:
        raise SystemExit(f"expected 33 projectile lifetimes, found {len(lifetimes)}")
    return records, lifetimes


def read_raw_actions(path: Path) -> list[LogicAction]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        event = values.get("event", "")
        kind = ACTION_NAMES.get(event)
        if event == "move":
            kind = MOVEMENT_PHASES.get(values.get("path", ""))
        if kind is None or values.get("shape") != "E3A8":
            continue
        result.append(
            LogicAction(
                elapsed=int(values["elapsed"]),
                source=values["object"],
                kind=kind,
                pose=parse_pose(values["pose"]),
                target=parse_pose(values["selected_pose"]),
            )
        )
    return result


def import_raw_logic(source: Path, output: Path) -> None:
    actions = read_raw_actions(source)
    lines = [
        "# Compact oracle evidence for first-reengagement hostile projectiles.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Source path addresses and opaque actor storage were reduced to gameplay operations.",
    ]
    for action in actions:
        lines.append(
            f"elapsed={action.elapsed} source={action.source} action={action.kind} "
            f"pose={','.join(map(str, action.pose))} "
            f"target={','.join(map(str, action.target))}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_logic_fixture(path: Path) -> list[LogicAction]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        values = fields(line)
        result.append(
            LogicAction(
                elapsed=int(values["elapsed"]),
                source=values["source"],
                kind=values["action"],
                pose=parse_pose(values["pose"]),
                target=parse_pose(values["target"]),
            )
        )
    if not result:
        raise SystemExit(f"projectile logic fixture is empty: {path}")
    return result


class Replay:
    def __init__(self) -> None:
        trig_source = TRIG_SOURCE.read_text(encoding="utf-8")
        self.sine = rust_table("SINTAB", trig_source)
        self.cosine = rust_table("COSTAB", trig_source)
        self.angle_curve = rust_table(
            "SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8")
        )

    @staticmethod
    def xz_distance(dx: int, dz: int) -> int:
        absolute = lambda value: (
            (-value) & 0xFFFF if signed_word(value) < 0 else value & 0xFFFF
        )
        half = lambda value: (signed_word(value) >> 1) & 0xFFFF
        x = half(absolute(dx))
        z = half(absolute(dz))
        summed = ((z + x) & 0xFFFF) << 1 & 0xFFFF
        maximum = x if signed_word(z - x) < 0 else z
        total = (maximum + summed) & 0xFFFF
        value = (half(total) + total) & 0xFFFF
        return signed_word(half(half(value)))

    def contract(self, pose: tuple[int, ...], target: tuple[int, ...]) -> tuple[int, ...]:
        delta = [signed_word(pose[index] - target[index]) for index in range(3)]
        radius = math.isqrt(sum(value * value for value in delta) & 0xFFFF_FFFF)
        if radius == 0:
            return pose
        precision_shift = max(0, radius.bit_length() - 1)
        reciprocal = (32_767 << precision_shift) // radius
        direction = [(value * reciprocal) >> precision_shift for value in delta]
        contracted_radius = (radius - CONTRACTION_DISTANCE) & 0xFFFF
        position = tuple(
            signed_word(target[index] + ((direction[index] * contracted_radius) >> 15))
            for index in range(3)
        )
        return position + pose[3:]

    def face(
        self, pose: tuple[int, ...], target: tuple[int, ...], smooth: bool
    ) -> tuple[int, ...]:
        dx = signed_word(target[0] - pose[0])
        dy = signed_word(target[1] - pose[1])
        dz = signed_word(target[2] - pose[2])
        pitch = (sf2_atan16(dy, self.xz_distance(dx, dz), self.angle_curve) >> 8) & 255
        yaw = (-(sf2_atan16(dx, dz, self.angle_curve) >> 8)) & 255
        if smooth:
            pitch = chase_power(pose[3], pitch, 2, 8, 4)
            yaw = chase_power(pose[4], yaw, 2, 8, 4)
        return pose[:3] + (pitch, yaw, pose[5], pose[6])

    def advance(self, pose: tuple[int, ...]) -> tuple[int, ...]:
        source_yaw = (-pose[4]) & 255
        pitch_cosine = self.cosine[pose[3]]
        velocity = (
            mulslog(mulslog(pose[6], self.sine[source_yaw]), pitch_cosine),
            mulslog(pose[6], self.sine[pose[3]]),
            mulslog(mulslog(pose[6], self.cosine[source_yaw]), pitch_cosine),
        )
        position = tuple(
            signed_word(pose[index] + velocity[index] * FLIGHT_POSITION_SCALE)
            for index in range(3)
        )
        return position + pose[3:]

    def apply(self, pose: tuple[int, ...], action: LogicAction) -> tuple[int, ...]:
        if action.kind == "contract":
            return self.contract(pose, action.target)
        if action.kind == "face-immediate":
            return self.face(pose, action.target, False)
        if action.kind == "face-smooth":
            return self.face(pose, action.target, True)
        if action.kind == "set-cruise-speed":
            return pose[:6] + (CRUISE_SPEED,)
        if action.kind.startswith("advance-"):
            return self.advance(pose)
        raise AssertionError(action.kind)


def schedule_lifetime(
    replay: Replay,
    lifetime: ProjectileLifetime,
    actions: list[LogicAction],
) -> list[tuple[int, list[LogicAction]]]:
    start_frame = lifetime.samples[0][0]
    end_frame = lifetime.samples[-1][0]
    stream = [
        action
        for action in actions
        if action.source == lifetime.source
        and start_frame - RETAIL_FRAME_STEP
        <= action.retail_frame
        <= end_frame + RETAIL_FRAME_STEP
    ]
    initial_pose = lifetime.samples[0][1]
    initial_indices = [index for index, action in enumerate(stream) if action.pose == initial_pose]
    if not initial_indices:
        raise SystemExit(
            f"projectile at frame {start_frame} has no matching semantic-action boundary"
        )

    desired_start = sum(action.retail_frame <= start_frame for action in stream)
    states: dict[int, tuple[int, int, list[int]]] = {
        index: ((index - desired_start) ** 2, index, []) for index in initial_indices
    }
    for sample_index, (retail_frame, expected) in enumerate(lifetime.samples[1:], 1):
        desired_cut = sum(action.retail_frame <= retail_frame for action in stream)
        next_states: dict[int, tuple[int, int, list[int]]] = {}
        previous_pose = lifetime.samples[sample_index - 1][1]
        for start, (cost, initial_index, cuts) in states.items():
            pose = previous_pose
            limit = min(len(stream), start + MAXIMUM_ACTIONS_PER_TICK)
            for end in range(start, limit + 1):
                if end > start:
                    pose = replay.apply(pose, stream[end - 1])
                if pose != expected:
                    continue
                candidate = (
                    cost + (end - desired_cut) ** 2,
                    initial_index,
                    cuts + [end],
                )
                if end not in next_states or candidate[0] < next_states[end][0]:
                    next_states[end] = candidate
        if not next_states:
            raise SystemExit(
                f"semantic projectile replay diverges at frame {retail_frame} "
                f"for lifetime beginning at {start_frame}"
            )
        states = next_states

    end, (_, start, cuts) = min(
        states.items(),
        key=lambda item: item[1][0] + (len(stream) - item[0]) ** 2,
    )

    result = []
    previous_cut = start
    for (retail_frame, _), cut in zip(lifetime.samples[1:], cuts):
        result.append((retail_frame, stream[previous_cut:cut]))
        previous_cut = cut
    print(
        f"projectile {start_frame:4d}..{end_frame:4d}: "
        f"{end - start} semantic actions, {len(lifetime.samples)} poses"
    )
    return result


def target_timing(
    action: LogicAction,
    retail_frame: int,
    records: dict[int, PoseRecord],
) -> str | None:
    if action.kind not in TARGET_ACTIONS:
        return None
    target_position = action.target[:3]
    choices = (
        ("Current", retail_frame),
        ("Midpoint", None),
        ("Previous", retail_frame - RETAIL_FRAME_STEP),
        ("TwoTicksAgo", retail_frame - RETAIL_FRAME_STEP * 2),
    )
    for name, frame in choices:
        if name == "Midpoint":
            current = records.get(retail_frame)
            previous = records.get(retail_frame - RETAIL_FRAME_STEP)
            if current is None or previous is None:
                continue
            position = tuple(
                signed_word(start + trunc_div(signed_word(end - start), 2))
                for start, end in zip(previous.player[:3], current.player[:3])
            )
            if position == target_position:
                return name
            continue
        record = records.get(frame)
        if record is not None and record.player[:3] == target_position:
            return name
    raise SystemExit(
        f"projectile target {target_position} at frame {retail_frame} "
        "is not a typed player-position timing"
    )


def grouped(value: int) -> str:
    return f"{value:_}"


def pose_source(pose: tuple[int, ...]) -> str:
    return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"


def rust_action(action: ScheduledAction) -> str:
    timing = (
        ""
        if action.target_timing is None
        else f"(ReengagementProjectileTarget::{action.target_timing})"
    )
    variants = {
        "contract": "ContractTowardTarget",
        "face-immediate": "FaceTargetImmediate",
        "face-smooth": "FaceTargetSmooth",
        "set-cruise-speed": "SetCruiseSpeed",
        "advance-homing": "AdvanceHoming",
        "advance-aim": "AdvanceAimCorrection",
        "advance-cruise": "AdvanceCruise",
    }
    return f"ReengagementProjectileAction::{variants[action.action.kind]}{timing}"


def format_rust(source: str) -> str:
    result = subprocess.run(
        ["rustfmt", "--edition", "2021", "--emit", "stdout"],
        input=source,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"rustfmt failed for generated projectile schedule:\n{result.stderr}")
    return result.stdout


def rust_source(
    lifetimes: list[ProjectileLifetime],
    schedules: list[list[tuple[int, list[ScheduledAction]]]],
) -> str:
    flattened_actions = []
    tick_ranges = []
    descriptors = []
    for lifetime, schedule in zip(lifetimes, schedules):
        tick_offset = len(tick_ranges)
        for _, actions in schedule:
            action_offset = len(flattened_actions)
            flattened_actions.extend(actions)
            tick_ranges.append((action_offset, len(actions)))
        descriptors.append(
            (
                lifetime.samples[0][0],
                lifetime.samples[-1][0],
                tick_offset,
                len(schedule),
                lifetime.samples[0][1],
            )
        )

    lines = [
        "//! Generated semantic projectile dynamics for the first retail re-engagement.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, MissionEncounterPose, ReengagementProjectileAction,",
        "    ReengagementProjectileTarget,",
        "};",
        "",
        f"pub(super) const PROJECTILE_COUNT: usize = {len(lifetimes)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) struct ReengagementProjectileDescriptor {",
        "    pub start_retail_frame: u16,",
        "    pub end_retail_frame: u16,",
        "    pub initial_pose: MissionEncounterPose,",
        "    tick_offset: u16,",
        "    tick_count: u8,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct ActionRange {",
        "    start: u16,",
        "    len: u8,",
        "}",
        "",
        f"static DESCRIPTORS: [ReengagementProjectileDescriptor; {len(descriptors)}] = [",
    ]
    for start, end, tick_offset, tick_count, pose in descriptors:
        lines.extend(
            [
                "    ReengagementProjectileDescriptor {",
                f"        start_retail_frame: {grouped(start)},",
                f"        end_retail_frame: {grouped(end)},",
                f"        initial_pose: {pose_source(pose)},",
                f"        tick_offset: {tick_offset},",
                f"        tick_count: {tick_count},",
                "    },",
            ]
        )
    lines.extend(
        [
            "];",
            "",
            f"static TICKS: [ActionRange; {len(tick_ranges)}] = [",
        ]
    )
    for start, length in tick_ranges:
        lines.append(f"    ActionRange {{ start: {start}, len: {length} }},")
    lines.extend(
        [
            "];",
            "",
            f"static ACTIONS: [ReengagementProjectileAction; {len(flattened_actions)}] = [",
        ]
    )
    lines.extend(f"    {rust_action(action)}," for action in flattened_actions)
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn descriptor(track_index: usize) -> Option<&'static ReengagementProjectileDescriptor> {",
            "    DESCRIPTORS.get(track_index)",
            "}",
            "",
            "pub(super) fn actions(track_index: usize, retail_frame: u16) -> &'static [ReengagementProjectileAction] {",
            "    let Some(descriptor) = descriptor(track_index) else {",
            "        return &[];",
            "    };",
            "    let Some(offset) = retail_frame.checked_sub(descriptor.start_retail_frame + RETAIL_FRAME_STEP) else {",
            "        return &[];",
            "    };",
            "    if offset % RETAIL_FRAME_STEP != 0 {",
            "        return &[];",
            "    }",
            "    let tick = offset / RETAIL_FRAME_STEP;",
            "    if tick >= u16::from(descriptor.tick_count) {",
            "        return &[];",
            "    }",
            "    let range = TICKS[usize::from(descriptor.tick_offset + tick)];",
            "    let start = usize::from(range.start);",
            "    &ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
        ]
    )
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        import_raw_logic(args.import_raw, args.logic_fixture)
    records, lifetimes = read_pose_fixture(args.pose_fixture)
    actions = read_logic_fixture(args.logic_fixture)
    replay = Replay()
    scheduled = [schedule_lifetime(replay, lifetime, actions) for lifetime in lifetimes]
    record_by_frame = {record.retail_frame: record for record in records}
    typed_schedules = [
        [
            (
                retail_frame,
                [
                    ScheduledAction(
                        action,
                        retail_frame,
                        target_timing(action, retail_frame, record_by_frame),
                    )
                    for action in frame_actions
                ],
            )
            for retail_frame, frame_actions in schedule
        ]
        for schedule in scheduled
    ]
    source = rust_source(lifetimes, typed_schedules)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated projectile dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        f"second-sortie projectile replay verified: "
        f"{sum(len(lifetime.samples) for lifetime in lifetimes)} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
