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

from projectile_static import (
    read_collision_eligibility,
    validate_static_collision_gate,
    validate_static_hostile_projectile_path,
)
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
DEFAULT_COLLISION_FIXTURE = (
    Path(__file__).with_name("fixtures")
    / "second_sortie_projectile_collision.trace"
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
    sample_start_elapsed: int

    @property
    def retail_frame(self) -> int:
        offset = self.elapsed - (self.sample_start_elapsed - 1)
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


def read_pose_fixture(
    path: Path,
    expected_lifetime_count: int = 33,
    maximum_continuous_position_step: int | None = None,
) -> tuple[list[PoseRecord], list[ProjectileLifetime]]:
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
            existing = active.get(source)
            if (
                maximum_continuous_position_step is not None
                and existing
                and max(
                    abs(signed_word(pose[index] - existing[-1][1][index]))
                    for index in range(3)
                )
                > maximum_continuous_position_step
            ):
                lifetimes.append(ProjectileLifetime(source, tuple(active.pop(source))))
            active.setdefault(source, []).append((retail_frame, pose))
        for source in set(active).difference(projectiles):
            lifetimes.append(ProjectileLifetime(source, tuple(active.pop(source))))
    lifetimes.extend(
        ProjectileLifetime(source, tuple(samples)) for source, samples in active.items()
    )
    lifetimes.sort(key=lambda lifetime: lifetime.samples[0][0])
    if len(lifetimes) != expected_lifetime_count:
        raise SystemExit(
            f"expected {expected_lifetime_count} projectile lifetimes, "
            f"found {len(lifetimes)}"
        )
    return records, lifetimes


def read_raw_actions(
    path: Path, sample_start_elapsed: int = RAW_SAMPLE_START_ELAPSED
) -> list[LogicAction]:
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
                sample_start_elapsed=sample_start_elapsed,
            )
        )
    return result


def import_raw_logic(
    source: Path,
    output: Path,
    sample_start_elapsed: int = RAW_SAMPLE_START_ELAPSED,
    encounter_name: str = "first-reengagement",
    included_sources: frozenset[str] | None = None,
) -> None:
    actions = read_raw_actions(source, sample_start_elapsed)
    if included_sources is not None:
        actions = [action for action in actions if action.source in included_sources]
    lines = [
        f"# Compact oracle evidence for {encounter_name} hostile projectiles.",
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


def read_logic_fixture(
    path: Path, sample_start_elapsed: int = RAW_SAMPLE_START_ELAPSED
) -> list[LogicAction]:
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
                sample_start_elapsed=sample_start_elapsed,
            )
        )
    if not result:
        raise SystemExit(f"projectile logic fixture is empty: {path}")
    return result


def read_natural_hits(
    collision_fixture: Path,
    records: list[PoseRecord],
    lifetimes: list[ProjectileLifetime],
) -> list[dict[str, str]]:
    hits = [
        fields(line)
        for line in collision_fixture.read_text(encoding="utf-8").splitlines()
        if line.startswith("natural_hit ")
    ]
    if not hits:
        raise SystemExit(
            "first-reengagement collision fixture must retain its natural hits"
        )
    expected_fields = {
        "elapsed",
        "collision_retail_frame",
        "player_retail_frame",
        "track",
        "source_object",
        "craft_class",
        "damage",
        "player_pose",
        "projectile_pose",
        "retained_projectile_pose",
        "source",
    }
    record_by_frame = {record.retail_frame: record for record in records}
    previous_elapsed = -1
    seen_tracks = set()
    for hit in hits:
        if set(hit) != expected_fields:
            raise SystemExit(
                "first-reengagement natural hit fields changed: "
                f"expected {sorted(expected_fields)}, found {sorted(hit)}"
            )
        elapsed = int(hit["elapsed"])
        collision_retail_frame = int(hit["collision_retail_frame"])
        player_retail_frame = int(hit["player_retail_frame"])
        track_index = int(hit["track"])
        retained_elapsed = RAW_SAMPLE_START_ELAPSED + player_retail_frame
        if not retained_elapsed <= elapsed < retained_elapsed + RETAIL_FRAME_STEP:
            raise SystemExit(
                "first-reengagement natural hit is outside its retained "
                "presentation interval"
            )
        if elapsed <= previous_elapsed:
            raise SystemExit(
                "first-reengagement natural hits are not chronological"
            )
        previous_elapsed = elapsed
        if track_index in seen_tracks:
            raise SystemExit(
                "first-reengagement projectile hit more than once"
            )
        seen_tracks.add(track_index)
        if hit["craft_class"] != "FoxFalco":
            raise SystemExit("first-reengagement natural hit craft class changed")
        if int(hit["damage"]) != 2:
            raise SystemExit("first-reengagement natural hit damage changed")
        if hit["source"] != "06:9707":
            raise SystemExit(
                "first-reengagement natural hit dispatch source changed"
            )
        if not 0 <= track_index < len(lifetimes):
            raise SystemExit(
                "first-reengagement natural hit track is out of range"
            )
        if hit["source_object"] != lifetimes[track_index].source:
            raise SystemExit(
                "first-reengagement natural hit source object does not match "
                "its projectile track"
            )
        projectile_pose = tuple(
            map(int, hit["retained_projectile_pose"].split(","))
        )
        if not any(
            frame == player_retail_frame and pose[:3] == projectile_pose
            for frame, pose in lifetimes[track_index].samples
        ):
            raise SystemExit(
                "first-reengagement natural hit projectile pose is absent "
                "from its track"
            )
        dispatch_pose = tuple(map(int, hit["projectile_pose"].split(",")))
        if len(dispatch_pose) != 3:
            raise SystemExit(
                "first-reengagement natural hit dispatch pose is malformed"
            )
        player_pose = tuple(map(int, hit["player_pose"].split(",")))
        record = record_by_frame.get(player_retail_frame)
        if record is None or record.player[:3] != player_pose:
            raise SystemExit(
                "first-reengagement natural hit player pose is absent "
                "from its retained boundary"
            )
        if collision_retail_frame not in {
            player_retail_frame,
            player_retail_frame + RETAIL_FRAME_STEP,
        }:
            raise SystemExit(
                "first-reengagement natural hit collision tick is not adjacent "
                "to its player pose"
            )
    return hits


def split_target_contractions(actions: list[LogicAction]) -> list[LogicAction]:
    """Expose retail cooperative coordinate writes as two semantic actions.

    Most encounters complete target contraction between retained presentation
    boundaries.  The Pigma duel contains a boundary after the horizontal
    coordinate is committed but before altitude and depth are committed, so
    that encounter opts into this expanded representation.
    """

    result = []
    for action in actions:
        if action.kind != "contract":
            result.append(action)
            continue
        result.extend(
            (
                LogicAction(
                    elapsed=action.elapsed,
                    source=action.source,
                    kind="begin-contract",
                    pose=action.pose,
                    target=action.target,
                    sample_start_elapsed=action.sample_start_elapsed,
                ),
                LogicAction(
                    elapsed=action.elapsed,
                    source=action.source,
                    kind="finish-contract",
                    pose=action.pose,
                    target=action.target,
                    sample_start_elapsed=action.sample_start_elapsed,
                ),
            )
        )
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
        def absolute(value: int) -> int:
            return (-value) & 0xFFFF if signed_word(value) < 0 else value & 0xFFFF

        def half(value: int) -> int:
            return (signed_word(value) >> 1) & 0xFFFF

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
        if action.kind == "begin-contract":
            contracted = self.contract(pose, action.target)
            return (contracted[0],) + pose[1:]
        if action.kind == "finish-contract":
            contracted = self.contract(action.pose, action.target)
            return pose[:1] + contracted[1:3] + pose[3:]
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
    target_position = action.target[:3]
    choices = (
        ("Current", retail_frame),
        ("Midpoint", None),
        ("Previous", retail_frame - RETAIL_FRAME_STEP),
        ("PreviousMidpoint", None),
        ("TwoTicksAgo", retail_frame - RETAIL_FRAME_STEP * 2),
    )
    for name, frame in choices:
        if name in ("Midpoint", "PreviousMidpoint"):
            later_frame = (
                retail_frame
                if name == "Midpoint"
                else retail_frame - RETAIL_FRAME_STEP
            )
            later = records.get(later_frame)
            earlier = records.get(later_frame - RETAIL_FRAME_STEP)
            if later is None or earlier is None:
                continue
            position = tuple(
                signed_word(start + trunc_div(signed_word(end - start), 2))
                for start, end in zip(earlier.player[:3], later.player[:3])
            )
            if position == target_position:
                return name
            continue
        record = records.get(frame)
        if record is not None and record.player[:3] == target_position:
            return name
    if action.kind in TARGET_ACTIONS or action.kind == "begin-contract":
        raise SystemExit(
            f"projectile target {target_position} at frame {retail_frame} "
            "is not a typed player-position timing"
        )
    return None


def grouped(value: int) -> str:
    return f"{value:_}"


def pose_source(pose: tuple[int, ...]) -> str:
    return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"


def rust_action(action: ScheduledAction) -> str:
    timing = (
        ""
        if action.action.kind not in TARGET_ACTIONS
        and action.action.kind != "begin-contract"
        else f"(HostileProjectileTarget::{action.target_timing})"
    )
    variants = {
        "contract": "ContractTowardTarget",
        "begin-contract": "BeginTargetContraction",
        "finish-contract": "FinishTargetContraction",
        "face-immediate": "FaceTargetImmediate",
        "face-smooth": "FaceTargetSmooth",
        "set-cruise-speed": "SetCruiseSpeed",
        "advance-homing": "AdvanceHoming",
        "advance-aim": "AdvanceAimCorrection",
        "advance-cruise": "AdvanceCruise",
    }
    return f"HostileProjectileAction::{variants[action.action.kind]}{timing}"


def format_rust(source: str) -> str:
    result = subprocess.run(
        [
            "rustfmt",
            "--edition",
            "2021",
            "--config",
            "skip_children=true,reorder_modules=false",
            "--emit",
            "stdout",
        ],
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
    encounter_description: str = "the first retail re-engagement",
    firing_actors: list[str] | None = None,
    collision_eligibility: list[bool] | None = None,
    emit_action_player_targets: bool = False,
) -> str:
    if firing_actors is not None and len(firing_actors) != len(lifetimes):
        raise SystemExit(
            f"expected {len(lifetimes)} firing actors, found {len(firing_actors)}"
        )
    if collision_eligibility is not None and len(collision_eligibility) != len(
        lifetimes
    ):
        raise SystemExit(
            f"expected {len(lifetimes)} collision eligibility entries, "
            f"found {len(collision_eligibility)}"
        )
    flattened_actions = []
    tick_ranges = []
    descriptors = []
    for track_index, (lifetime, schedule) in enumerate(zip(lifetimes, schedules)):
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
                None if firing_actors is None else firing_actors[track_index],
                (
                    None
                    if collision_eligibility is None
                    else collision_eligibility[track_index]
                ),
            )
        )

    actor_import = (
        "    MissionEncounterActor, MissionEncounterPose,"
        if firing_actors is not None
        else "    MissionEncounterPose,"
    )
    lines = [
        f"//! Generated semantic projectile dynamics for {encounter_description}.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, HostileProjectileAction, HostileProjectileTarget,",
        actor_import,
        "};",
        "",
        f"pub(super) const PROJECTILE_COUNT: usize = {len(lifetimes)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) struct HostileProjectileDescriptor {",
        "    pub start_retail_frame: u16,",
        "    pub end_retail_frame: u16,",
        "    pub initial_pose: MissionEncounterPose,",
    ]
    if firing_actors is not None:
        lines.append("    pub firing_actor: MissionEncounterActor,")
    if collision_eligibility is not None:
        lines.append("    pub collision_enabled: bool,")
    lines.extend(
        [
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
        f"static DESCRIPTORS: [HostileProjectileDescriptor; {len(descriptors)}] = [",
        ]
    )
    for (
        start,
        end,
        tick_offset,
        tick_count,
        pose,
        firing_actor,
        collision_enabled,
    ) in descriptors:
        lines.extend(
            [
                "    HostileProjectileDescriptor {",
                f"        start_retail_frame: {grouped(start)},",
                f"        end_retail_frame: {grouped(end)},",
                f"        initial_pose: {pose_source(pose)},",
            ]
        )
        if firing_actor is not None:
            lines.append(f"        firing_actor: MissionEncounterActor::{firing_actor},")
        if collision_enabled is not None:
            lines.append(f"        collision_enabled: {str(collision_enabled).lower()},")
        lines.extend(
            [
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
            f"static ACTIONS: [HostileProjectileAction; {len(flattened_actions)}] = [",
        ]
    )
    lines.extend(f"    {rust_action(action)}," for action in flattened_actions)
    if emit_action_player_targets:
        lines.extend(
            [
                "];",
                "",
                "static ACTION_PLAYER_TARGETS: "
                f"[Option<HostileProjectileTarget>; {len(flattened_actions)}] = [",
            ]
        )
        for action in flattened_actions:
            timing = (
                "None"
                if action.target_timing is None
                else f"Some(HostileProjectileTarget::{action.target_timing})"
            )
            lines.append(f"    {timing},")
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn descriptor(track_index: usize) -> Option<&'static HostileProjectileDescriptor> {",
            "    DESCRIPTORS.get(track_index)",
            "}",
            "",
            "pub(super) fn actions(track_index: usize, retail_frame: u16) -> &'static [HostileProjectileAction] {",
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
    if emit_action_player_targets:
        lines.extend(
            [
                "pub(super) fn action_player_targets(",
                "    track_index: usize,",
                "    retail_frame: u16,",
                ") -> &'static [Option<HostileProjectileTarget>] {",
                "    let Some(descriptor) = descriptor(track_index) else {",
                "        return &[];",
                "    };",
                "    let Some(offset) = retail_frame",
                "        .checked_sub(descriptor.start_retail_frame + RETAIL_FRAME_STEP)",
                "    else {",
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
                "    &ACTION_PLAYER_TARGETS[start..start + usize::from(range.len)]",
                "}",
                "",
            ]
        )
    return format_rust("\n".join(lines))


def generate_dynamics(
    logic_fixture: Path,
    pose_fixture: Path,
    expected_lifetime_count: int,
    sample_start_elapsed: int,
    encounter_description: str,
    allow_split_contractions: bool = False,
    firing_actors: list[str] | None = None,
    maximum_continuous_position_step: int | None = None,
    collision_eligibility: list[bool] | None = None,
    emit_action_player_targets: bool = False,
) -> tuple[str, int]:
    records, lifetimes = read_pose_fixture(
        pose_fixture,
        expected_lifetime_count,
        maximum_continuous_position_step,
    )
    actions = read_logic_fixture(logic_fixture, sample_start_elapsed)
    if allow_split_contractions:
        actions = split_target_contractions(actions)
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
    if emit_action_player_targets:
        supported_collision_timings = {"Previous", "Midpoint", "Current"}
        unsupported = {
            action.target_timing
            for schedule in typed_schedules
            for _, frame_actions in schedule
            for action in frame_actions
            if action.target_timing is not None
            and action.target_timing not in supported_collision_timings
        }
        if unsupported:
            raise SystemExit(
                "projectile action collision timing needs deeper player history: "
                f"{sorted(unsupported)}"
            )
    return (
        rust_source(
            lifetimes,
            typed_schedules,
            encounter_description,
            firing_actors,
            collision_eligibility,
            emit_action_player_targets,
        ),
        sum(len(lifetime.samples) for lifetime in lifetimes),
    )


def append_test_oracle(
    source: str,
    pose_fixture: Path,
    collision_fixture: Path,
) -> str:
    records, lifetimes = read_pose_fixture(pose_fixture)
    natural_hits = read_natural_hits(collision_fixture, records, lifetimes)
    lines = [
        source,
        "#[cfg(test)]",
        f"pub(super) const NATURAL_HITS: "
        f"[(u16, usize, u8); {len(natural_hits)}] = [",
    ]
    for hit in natural_hits:
        lines.append(
            "    "
            f"({int(hit['collision_retail_frame']):_}, {int(hit['track'])}, "
            f"{int(hit['damage'])}),"
        )
    lines.extend(["];", ""])
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument(
        "--collision-fixture", type=Path, default=DEFAULT_COLLISION_FIXTURE
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    validate_static_hostile_projectile_path()
    validate_static_collision_gate()
    if args.import_raw is not None:
        import_raw_logic(args.import_raw, args.logic_fixture)
    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        33,
        RAW_SAMPLE_START_ELAPSED,
        "the first retail re-engagement",
        collision_eligibility=read_collision_eligibility(
            args.collision_fixture,
            33,
            "first-reengagement",
        ),
        emit_action_player_targets=True,
    )
    source = append_test_oracle(
        source,
        args.pose_fixture,
        args.collision_fixture,
    )
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated projectile dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        f"second-sortie projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
