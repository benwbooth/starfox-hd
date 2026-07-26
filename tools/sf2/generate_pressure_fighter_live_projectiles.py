#!/usr/bin/env python3
"""Reduce recurring-fighter fire and projectile traces to typed test evidence.

The raw Mesen logs contain source object addresses and path/interpreter state.
This generator validates those details while importing, then emits only
gameplay concepts for the Rust port: launch range, allocation outcome, phase
lengths, scheduled correction phase, crossing outcome, and terminal outcome.
"""

from __future__ import annotations

import argparse
import hashlib
import math
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = (
    ROOT / "tools/sf2/fixtures/pressure_fighter_live_projectiles.trace"
)
DEFAULT_OUTPUT = (
    ROOT
    / "rust/sf2-game/src/native/pressure_fighter_live_projectile_oracle.rs"
)

FIGHTER_OBJECT = "05F4"
FIGHTER_SHAPE = "F1C4"
PROJECTILE_SHAPE = "E3A8"
INITIAL_SPEED = 30
CRUISE_SPEED = 63
MAXIMUM_LAUNCH_DISTANCE = 12_000
HOMING_RADIUS = 1_024
HOMING_CONTRACTIONS_PER_STEP = 3
MAXIMUM_AIM_STEPS = 40
SMOOTH_AIM_TRIGGER_PERIOD = 2
CRUISE_STEPS = 15
EXPECTED_FIRE_ATTEMPTS = 48
EXPECTED_LIFETIMES = 36
RAW_ORIGIN_ELAPSED = 81_000
HANDOFF_STRATEGY_FRAME = 142
PROJECTILE_EVENTS = frozenset(
    (
        "projectile-set-speed",
        "projectile-distance-test",
        "projectile-orbit-pitch",
        "projectile-face-immediate",
        "projectile-face-smooth",
        "move",
        "wait",
    )
)


@dataclass(frozen=True)
class RawProjectileEvent:
    elapsed: int
    strategy_frame: int
    kind: str
    path: str
    pose: tuple[int, ...]
    target: tuple[int, ...]


@dataclass(frozen=True)
class FireAttempt:
    elapsed: int
    launch: tuple[int, ...]
    target: tuple[int, ...]
    distance: int
    launched: bool
    start_delay: int | None


@dataclass(frozen=True)
class ProjectileLifetime:
    start_elapsed: int
    end_elapsed: int
    launch: tuple[int, ...]
    homing_steps: int
    aim_steps: int
    first_strategy_frame: int
    aim_correction_mask: int
    crossed_target: bool
    cruise_steps: int
    natural_expiry: bool


@dataclass(frozen=True)
class AimStep:
    lifetime_start: int
    step_index: int
    previous_position: tuple[int, ...]
    current_position: tuple[int, ...]
    target: tuple[int, ...]
    pitch_before_correction: int
    yaw_before_correction: int
    expected_pitch: int
    expected_yaw: int
    strategy_frame: int
    corrected: bool
    terminal: bool


@dataclass(frozen=True)
class Evidence:
    actor_sha256: str
    projectile_sha256: str
    movement_interval_sum: int
    movement_interval_count: int
    reusable_allocation_count: int
    maximum_concurrent_projectiles: int
    handoff_strategy_frame: int
    fire_attempts: tuple[FireAttempt, ...]
    lifetimes: tuple[ProjectileLifetime, ...]
    aim_steps: tuple[AimStep, ...]


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


def wrapped_delta(first: int, second: int) -> int:
    return ((first - second + 32_768) & 65_535) - 32_768


def horizontal_distance(first: tuple[int, ...], second: tuple[int, ...]) -> int:
    delta_x = wrapped_delta(first[0], second[0])
    delta_z = wrapped_delta(first[2], second[2])
    return math.isqrt(delta_x * delta_x + delta_z * delta_z)


def parse_fires(
    path: Path,
) -> tuple[list[tuple[int, tuple[int, ...], tuple[int, ...]]], int]:
    fires = []
    first_live_strategy_frame = None
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            first_live_strategy_frame is None
            and values.get("elapsed") == str(RAW_ORIGIN_ELAPSED)
            and values.get("event") == "move"
            and values.get("object") == FIGHTER_OBJECT
            and values.get("shape") == FIGHTER_SHAPE
        ):
            first_live_strategy_frame = int(values["strategy_frame"])
        if (
            values.get("event") != "fire"
            or values.get("object") != FIGHTER_OBJECT
            or values.get("shape") != FIGHTER_SHAPE
        ):
            continue
        fires.append(
            (
                int(values["elapsed"]),
                parse_tuple(values["pose"], 7, "fighter launch pose"),
                parse_tuple(values["selected_pose"], 7, "fighter target pose"),
            )
        )
    if len(fires) != EXPECTED_FIRE_ATTEMPTS:
        raise SystemExit(
            f"actor trace has {len(fires)} fire attempts; "
            f"expected {EXPECTED_FIRE_ATTEMPTS}"
        )
    if first_live_strategy_frame is None:
        raise SystemExit("actor trace lacks the first retained live fighter slice")
    handoff_strategy_frame = (first_live_strategy_frame - 1) & 255
    if handoff_strategy_frame != HANDOFF_STRATEGY_FRAME:
        raise SystemExit(
            f"actor trace hands off at strategy frame {handoff_strategy_frame}; "
            f"expected {HANDOFF_STRATEGY_FRAME}"
        )
    return fires, handoff_strategy_frame


def parse_projectile_events(
    path: Path,
) -> dict[str, list[RawProjectileEvent]]:
    events: dict[str, list[RawProjectileEvent]] = defaultdict(list)
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("shape") != PROJECTILE_SHAPE
            or values.get("event") not in PROJECTILE_EVENTS
        ):
            continue
        events[values["object"]].append(
            RawProjectileEvent(
                elapsed=int(values["elapsed"]),
                strategy_frame=int(values["strategy_frame"]),
                kind=values["event"],
                path=values["path"],
                pose=parse_tuple(values["pose"], 7, "projectile pose"),
                target=parse_tuple(values["selected_pose"], 7, "projectile target pose"),
            )
        )
    if not events:
        raise SystemExit("projectile trace contains no hostile laser events")
    return events


def validate_distance_step(
    lifetime_start: int,
    events: list[RawProjectileEvent],
    index: int,
) -> bool:
    distance_event = events[index]
    following = events[index + 1 :]
    next_move = next((event for event in following if event.kind == "move"), None)
    if next_move is None:
        raise SystemExit(
            f"projectile {lifetime_start} distance test has no following movement"
        )
    before_move = following[: following.index(next_move)]
    contraction_count = sum(
        event.kind == "projectile-orbit-pitch" for event in before_move
    )
    immediate_faces = sum(
        event.kind == "projectile-face-immediate" for event in before_move
    )
    transition_count = sum(
        event.kind == "projectile-set-speed" and event.path == "EEAF"
        for event in before_move
    )
    distance = horizontal_distance(distance_event.pose, distance_event.target)
    if distance >= HOMING_RADIUS:
        expected = (HOMING_CONTRACTIONS_PER_STEP, 1, 0, "EE9D")
        observed = (
            contraction_count,
            immediate_faces,
            transition_count,
            next_move.path,
        )
        if observed != expected:
            raise SystemExit(
                f"projectile {lifetime_start} far homing step is {observed}; "
                f"expected {expected}"
            )
        return False
    expected = (0, 1, 1, "EEC6")
    observed = (
        contraction_count,
        immediate_faces,
        transition_count,
        next_move.path,
    )
    if observed != expected:
        raise SystemExit(
            f"projectile {lifetime_start} final homing step is {observed}; "
            f"expected {expected}"
        )
    return True


def target_crossed_after_movement(
    lifetime_start: int,
    events: list[RawProjectileEvent],
    movement_index: int,
) -> bool:
    movement = events[movement_index]
    following_pose = next(
        (
            event.pose
            for event in events[movement_index + 1 :]
            if event.pose[:3] != movement.pose[:3]
        ),
        None,
    )
    if following_pose is None:
        raise SystemExit(
            f"projectile {lifetime_start} final aim movement lacks a resulting pose"
        )
    displacement = tuple(
        wrapped_delta(after, before)
        for after, before in zip(following_pose[:3], movement.pose[:3])
    )
    remaining = tuple(
        wrapped_delta(target, after)
        for target, after in zip(movement.target[:3], following_pose[:3])
    )
    return sum(
        direction * distance
        for direction, distance in zip(displacement, remaining)
    ) <= 0


def projectile_lifetimes(
    events_by_source: dict[str, list[RawProjectileEvent]],
) -> tuple[
    list[ProjectileLifetime],
    list[AimStep],
    list[tuple[int, int]],
    int,
]:
    raw_lifetimes: list[tuple[str, list[RawProjectileEvent]]] = []
    for source, events in events_by_source.items():
        starts = [
            index
            for index, event in enumerate(events)
            if event.kind == "projectile-set-speed" and event.path == "EE9B"
        ]
        for lifetime_index, start_index in enumerate(starts):
            end_index = (
                starts[lifetime_index + 1]
                if lifetime_index + 1 < len(starts)
                else len(events)
            )
            raw_lifetimes.append((source, events[start_index:end_index]))
    raw_lifetimes.sort(key=lambda item: item[1][0].elapsed)
    if len(raw_lifetimes) != EXPECTED_LIFETIMES:
        raise SystemExit(
            f"projectile trace has {len(raw_lifetimes)} lifetimes; "
            f"expected {EXPECTED_LIFETIMES}"
        )

    lifetimes = []
    aim_samples = []
    intervals = []
    visible_ranges = []
    for _, events in raw_lifetimes:
        first = events[0]
        if first.pose[6] != INITIAL_SPEED:
            raise SystemExit(
                f"projectile {first.elapsed} launches at {first.pose[6]}, "
                f"expected {INITIAL_SPEED}"
            )
        speed_events = [
            (event.path, event.pose[6])
            for event in events
            if event.kind == "projectile-set-speed"
        ]
        if speed_events != [("EE9B", INITIAL_SPEED), ("EEAF", CRUISE_SPEED)]:
            raise SystemExit(
                f"projectile {first.elapsed} speed transitions are {speed_events}"
            )

        distance_indices = [
            index
            for index, event in enumerate(events)
            if event.kind == "projectile-distance-test"
        ]
        near_steps = sum(
            validate_distance_step(first.elapsed, events, index)
            for index in distance_indices
        )
        if near_steps != 1:
            raise SystemExit(
                f"projectile {first.elapsed} has {near_steps} final homing steps"
            )

        movements = [event for event in events if event.kind == "move"]
        homing_steps = sum(event.path == "EE9D" for event in movements)
        aim_steps = sum(event.path == "EEC6" for event in movements)
        cruise_steps = sum(event.path == "EED2" for event in movements)
        if len(distance_indices) != homing_steps + 1:
            raise SystemExit(
                f"projectile {first.elapsed} homing movements do not follow "
                "all distance tests"
            )
        if not 1 <= aim_steps <= MAXIMUM_AIM_STEPS:
            raise SystemExit(
                f"projectile {first.elapsed} has {aim_steps} aim steps"
            )

        aim_event_indices = [
            index
            for index, event in enumerate(events)
            if event.kind == "move" and event.path == "EEC6"
        ]
        correction_mask = 0
        smooth_by_aim_step = {}
        for smooth_index, smooth in (
            (index, event)
            for index, event in enumerate(events)
            if event.kind == "projectile-face-smooth"
        ):
            preceding_aim_steps = [
                (ordinal, event_index)
                for ordinal, event_index in enumerate(aim_event_indices)
                if event_index < smooth_index
            ]
            if not preceding_aim_steps:
                raise SystemExit(
                    f"projectile {first.elapsed} corrects before its first aim move"
                )
            ordinal, aim_index = preceding_aim_steps[-1]
            aim = events[aim_index]
            if (
                smooth.elapsed - aim.elapsed not in (0, 1)
                or smooth.strategy_frame != aim.strategy_frame
                or smooth.strategy_frame % SMOOTH_AIM_TRIGGER_PERIOD != 0
            ):
                raise SystemExit(
                    f"projectile {first.elapsed} has an off-phase aim correction"
                )
            bit = 1 << ordinal
            if correction_mask & bit:
                raise SystemExit(
                    f"projectile {first.elapsed} corrects aim step {ordinal} twice"
                )
            correction_mask |= bit
            smooth_by_aim_step[ordinal] = smooth_index
        if correction_mask >> aim_steps:
            raise SystemExit(
                f"projectile {first.elapsed} corrects outside its aim phase"
            )
        first_strategy_frame = events[aim_event_indices[0]].strategy_frame
        if first_strategy_frame != (
            first.strategy_frame + homing_steps
        ) & 255:
            raise SystemExit(
                f"projectile {first.elapsed} aim phase does not follow homing "
                "on consecutive strategy frames"
            )
        crossed_target = target_crossed_after_movement(
            first.elapsed,
            events,
            aim_event_indices[-1],
        )
        for ordinal, aim_index in enumerate(aim_event_indices):
            aim = events[aim_index]
            after_index = next(
                (
                    index
                    for index in range(aim_index + 1, len(events))
                    if events[index].pose[:3] != aim.pose[:3]
                ),
                None,
            )
            if after_index is None:
                raise SystemExit(
                    f"projectile {first.elapsed} aim step {ordinal} lacks movement"
                )
            after = events[after_index]
            smooth_index = smooth_by_aim_step.get(ordinal)
            if smooth_index is None:
                expected_pitch, expected_yaw = after.pose[3:5]
            else:
                smooth = events[smooth_index]
                if smooth.target[:3] != aim.target[:3]:
                    raise SystemExit(
                        f"projectile {first.elapsed} correction changes target"
                    )
                expected_event = next(
                    iter(events[smooth_index + 1 :]),
                    None,
                )
                if expected_event is None:
                    raise SystemExit(
                        f"projectile {first.elapsed} correction lacks a result"
                    )
                expected_pitch, expected_yaw = expected_event.pose[3:5]
            aim_samples.append(
                AimStep(
                    lifetime_start=first.elapsed,
                    step_index=ordinal,
                    previous_position=aim.pose[:3],
                    current_position=after.pose[:3],
                    target=aim.target[:3],
                    pitch_before_correction=after.pose[3],
                    yaw_before_correction=after.pose[4],
                    expected_pitch=expected_pitch,
                    expected_yaw=expected_yaw,
                    strategy_frame=aim.strategy_frame,
                    corrected=smooth_index is not None,
                    terminal=ordinal + 1 == len(aim_event_indices),
                )
            )

        natural_expiry = events[-1].kind == "wait" and events[-1].path == "EED2"
        if natural_expiry:
            if cruise_steps != CRUISE_STEPS:
                raise SystemExit(
                    f"projectile {first.elapsed} naturally expires after "
                    f"{cruise_steps} cruise steps"
                )
        elif (
            events[-1].kind != "move"
            or events[-1].path != "EED2"
            or cruise_steps != 1
        ):
            raise SystemExit(
                f"projectile {first.elapsed} has an unknown terminal sequence"
            )

        for previous, current in zip(movements, movements[1:]):
            interval = current.elapsed - previous.elapsed
            if interval <= 0:
                raise SystemExit(
                    f"projectile {first.elapsed} has a nonpositive movement interval"
                )
            intervals.append((previous.elapsed, interval))
        visible_ranges.append((first.elapsed, events[-1].elapsed))
        lifetimes.append(
            ProjectileLifetime(
                start_elapsed=first.elapsed,
                end_elapsed=events[-1].elapsed,
                launch=first.pose,
                homing_steps=homing_steps,
                aim_steps=aim_steps,
                first_strategy_frame=first_strategy_frame,
                aim_correction_mask=correction_mask,
                crossed_target=crossed_target,
                cruise_steps=cruise_steps,
                natural_expiry=natural_expiry,
            )
        )

    maximum_concurrent = max(
        sum(start <= elapsed <= end for start, end in visible_ranges)
        for elapsed in {
            boundary for visible_range in visible_ranges for boundary in visible_range
        }
    )
    return lifetimes, aim_samples, intervals, maximum_concurrent


def import_raw(actor_path: Path, projectile_path: Path) -> Evidence:
    fires, handoff_strategy_frame = parse_fires(actor_path)
    raw_events = parse_projectile_events(projectile_path)
    lifetimes, aim_steps, intervals, maximum_concurrent = projectile_lifetimes(raw_events)

    unmatched_lifetimes = set(range(len(lifetimes)))
    attempts = []
    for elapsed, launch, target in fires:
        matching = [
            index
            for index in unmatched_lifetimes
            if lifetimes[index].start_elapsed in (elapsed, elapsed + 1)
            and lifetimes[index].launch[:6] == launch[:6]
        ]
        if len(matching) > 1:
            raise SystemExit(f"fire attempt {elapsed} matches multiple projectiles")
        matched = matching[0] if matching else None
        if matched is not None:
            unmatched_lifetimes.remove(matched)
        distance = horizontal_distance(launch, target)
        launched = matched is not None
        if launched != (distance < MAXIMUM_LAUNCH_DISTANCE):
            raise SystemExit(
                f"fire attempt {elapsed} launch={launched} at distance {distance}; "
                f"expected threshold {MAXIMUM_LAUNCH_DISTANCE}"
            )
        attempts.append(
            FireAttempt(
                elapsed=elapsed,
                launch=launch,
                target=target,
                distance=distance,
                launched=launched,
                start_delay=(
                    None
                    if matched is None
                    else lifetimes[matched].start_elapsed - elapsed
                ),
            )
        )
    if unmatched_lifetimes:
        raise SystemExit(
            f"{len(unmatched_lifetimes)} projectile lifetimes lack fighter fire events"
        )

    return Evidence(
        actor_sha256=hashlib.sha256(actor_path.read_bytes()).hexdigest(),
        projectile_sha256=hashlib.sha256(projectile_path.read_bytes()).hexdigest(),
        movement_interval_sum=sum(interval for _, interval in intervals),
        movement_interval_count=len(intervals),
        reusable_allocation_count=len(raw_events),
        maximum_concurrent_projectiles=maximum_concurrent,
        handoff_strategy_frame=handoff_strategy_frame,
        fire_attempts=tuple(attempts),
        lifetimes=tuple(lifetimes),
        aim_steps=tuple(aim_steps),
    )


def write_fixture(path: Path, evidence: Evidence) -> None:
    lines = [
        "# Oracle evidence for recurring-fighter live hostile projectiles.",
        f"# actor_raw_sha256={evidence.actor_sha256}",
        f"# projectile_raw_sha256={evidence.projectile_sha256}",
        (
            f"# movement_interval_sum={evidence.movement_interval_sum} "
            f"movement_interval_count={evidence.movement_interval_count}"
        ),
        (
            f"# reusable_allocation_count={evidence.reusable_allocation_count} "
            f"maximum_concurrent_projectiles="
            f"{evidence.maximum_concurrent_projectiles}"
        ),
        f"# handoff_strategy_frame={evidence.handoff_strategy_frame}",
        "# terminal=expired means the complete fifteen-step free-flight path;",
        "# terminal=terminated means the object vanished after its first cruise move.",
    ]
    for attempt in evidence.fire_attempts:
        lines.append(
            f"fire elapsed={attempt.elapsed} "
            f"launch={','.join(map(str, attempt.launch))} "
            f"target={','.join(map(str, attempt.target))} "
            f"distance={attempt.distance} "
            f"launched={int(attempt.launched)} "
            f"start_delay={attempt.start_delay if attempt.start_delay is not None else '-'}"
        )
    for lifetime in evidence.lifetimes:
        lines.append(
            f"projectile start={lifetime.start_elapsed} end={lifetime.end_elapsed} "
            f"launch={','.join(map(str, lifetime.launch))} "
            f"homing={lifetime.homing_steps} aim={lifetime.aim_steps} "
            f"first_strategy_frame={lifetime.first_strategy_frame} "
            f"aim_correction_mask={lifetime.aim_correction_mask} "
            f"crossed_target={int(lifetime.crossed_target)} "
            f"cruise={lifetime.cruise_steps} "
            f"terminal={'expired' if lifetime.natural_expiry else 'terminated'}"
        )
    for aim in evidence.aim_steps:
        lines.append(
            f"aim_step lifetime_start={aim.lifetime_start} "
            f"index={aim.step_index} "
            f"previous={','.join(map(str, aim.previous_position))} "
            f"current={','.join(map(str, aim.current_position))} "
            f"target={','.join(map(str, aim.target))} "
            f"angles_before={aim.pitch_before_correction},{aim.yaw_before_correction} "
            f"angles_after={aim.expected_pitch},{aim.expected_yaw} "
            f"strategy_frame={aim.strategy_frame} "
            f"corrected={int(aim.corrected)} terminal={int(aim.terminal)}"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_fixture(path: Path) -> Evidence:
    actor_sha256 = ""
    projectile_sha256 = ""
    movement_interval_sum = None
    movement_interval_count = None
    reusable_allocation_count = None
    maximum_concurrent_projectiles = None
    handoff_strategy_frame = None
    attempts = []
    lifetimes = []
    aim_steps = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if line.startswith("# actor_raw_sha256="):
            actor_sha256 = line.split("=", 1)[1]
        elif line.startswith("# projectile_raw_sha256="):
            projectile_sha256 = line.split("=", 1)[1]
        elif line.startswith("# movement_interval_sum="):
            movement_interval_sum = int(values["movement_interval_sum"])
            movement_interval_count = int(values["movement_interval_count"])
        elif line.startswith("# reusable_allocation_count="):
            reusable_allocation_count = int(values["reusable_allocation_count"])
            maximum_concurrent_projectiles = int(
                values["maximum_concurrent_projectiles"]
            )
        elif line.startswith("# handoff_strategy_frame="):
            handoff_strategy_frame = int(values["handoff_strategy_frame"])
        elif line.startswith("fire "):
            attempts.append(
                FireAttempt(
                    elapsed=int(values["elapsed"]),
                    launch=parse_tuple(values["launch"], 7, "fixture launch pose"),
                    target=parse_tuple(values["target"], 7, "fixture target pose"),
                    distance=int(values["distance"]),
                    launched=bool(int(values["launched"])),
                    start_delay=(
                        None
                        if values["start_delay"] == "-"
                        else int(values["start_delay"])
                    ),
                )
            )
        elif line.startswith("projectile "):
            lifetimes.append(
                ProjectileLifetime(
                    start_elapsed=int(values["start"]),
                    end_elapsed=int(values["end"]),
                    launch=parse_tuple(
                        values["launch"], 7, "fixture projectile launch pose"
                    ),
                    homing_steps=int(values["homing"]),
                    aim_steps=int(values["aim"]),
                    first_strategy_frame=int(values["first_strategy_frame"]),
                    aim_correction_mask=int(values["aim_correction_mask"]),
                    crossed_target=bool(int(values["crossed_target"])),
                    cruise_steps=int(values["cruise"]),
                    natural_expiry=values["terminal"] == "expired",
                )
            )
        elif line.startswith("aim_step "):
            angles_before = parse_tuple(
                values["angles_before"], 2, "fixture pre-correction angles"
            )
            angles_after = parse_tuple(
                values["angles_after"], 2, "fixture post-correction angles"
            )
            aim_steps.append(
                AimStep(
                    lifetime_start=int(values["lifetime_start"]),
                    step_index=int(values["index"]),
                    previous_position=parse_tuple(
                        values["previous"], 3, "fixture previous aim position"
                    ),
                    current_position=parse_tuple(
                        values["current"], 3, "fixture current aim position"
                    ),
                    target=parse_tuple(values["target"], 3, "fixture aim target"),
                    pitch_before_correction=angles_before[0],
                    yaw_before_correction=angles_before[1],
                    expected_pitch=angles_after[0],
                    expected_yaw=angles_after[1],
                    strategy_frame=int(values["strategy_frame"]),
                    corrected=bool(int(values["corrected"])),
                    terminal=bool(int(values["terminal"])),
                )
            )
    if (
        len(actor_sha256) != 64
        or len(projectile_sha256) != 64
        or movement_interval_sum is None
        or movement_interval_count is None
        or reusable_allocation_count is None
        or maximum_concurrent_projectiles is None
        or handoff_strategy_frame is None
        or len(attempts) != EXPECTED_FIRE_ATTEMPTS
        or len(lifetimes) != EXPECTED_LIFETIMES
        or not aim_steps
    ):
        raise SystemExit(f"malformed live-projectile fixture: {path}")
    if any(
        attempt.distance
        != horizontal_distance(attempt.launch, attempt.target)
        or attempt.launched
        != (attempt.distance < MAXIMUM_LAUNCH_DISTANCE)
        for attempt in attempts
    ):
        raise SystemExit("live-projectile fixture has inconsistent launch ranges")
    if sum(attempt.launched for attempt in attempts) != len(lifetimes):
        raise SystemExit("live-projectile fixture launch and lifetime counts differ")
    if handoff_strategy_frame != HANDOFF_STRATEGY_FRAME:
        raise SystemExit("live-projectile fixture has the wrong strategy handoff")
    if any(
        lifetime.aim_steps < 1
        or lifetime.aim_steps > MAXIMUM_AIM_STEPS
        or lifetime.aim_correction_mask >> lifetime.aim_steps
        for lifetime in lifetimes
    ):
        raise SystemExit("live-projectile fixture has inconsistent aim phases")
    if len(aim_steps) != sum(lifetime.aim_steps for lifetime in lifetimes):
        raise SystemExit("live-projectile fixture has incomplete aim-step evidence")
    by_lifetime = defaultdict(list)
    for aim in aim_steps:
        by_lifetime[aim.lifetime_start].append(aim)
    for lifetime in lifetimes:
        steps = by_lifetime[lifetime.start_elapsed]
        if (
            [step.step_index for step in steps] != list(range(lifetime.aim_steps))
            or sum(step.terminal for step in steps) != 1
            or not steps[-1].terminal
            or sum(
                (1 << step.step_index) if step.corrected else 0
                for step in steps
            )
            != lifetime.aim_correction_mask
        ):
            raise SystemExit(
                f"live-projectile fixture has malformed aim steps for "
                f"{lifetime.start_elapsed}"
            )
    return Evidence(
        actor_sha256=actor_sha256,
        projectile_sha256=projectile_sha256,
        movement_interval_sum=movement_interval_sum,
        movement_interval_count=movement_interval_count,
        reusable_allocation_count=reusable_allocation_count,
        maximum_concurrent_projectiles=maximum_concurrent_projectiles,
        handoff_strategy_frame=handoff_strategy_frame,
        fire_attempts=tuple(attempts),
        lifetimes=tuple(lifetimes),
        aim_steps=tuple(aim_steps),
    )


def render(evidence: Evidence) -> str:
    natural_expiry_count = sum(
        lifetime.natural_expiry for lifetime in evidence.lifetimes
    )
    crossing_exit_count = sum(
        lifetime.crossed_target for lifetime in evidence.lifetimes
    )
    lines = [
        "// @generated by tools/sf2/generate_pressure_fighter_live_projectiles.py",
        "// Runtime tests consume only typed gameplay evidence, not source-machine state.",
        "",
        f"pub const FIRE_ATTEMPT_COUNT: usize = {len(evidence.fire_attempts)};",
        (
            "pub const ALLOCATED_PROJECTILE_COUNT: usize = "
            f"{len(evidence.lifetimes)};"
        ),
        (
            "pub const NATURAL_EXPIRY_COUNT: usize = "
            f"{natural_expiry_count};"
        ),
        (
            "pub const EARLY_TERMINATION_COUNT: usize = "
            f"{len(evidence.lifetimes) - natural_expiry_count};"
        ),
        (
            "pub const REUSABLE_ALLOCATION_COUNT: usize = "
            f"{evidence.reusable_allocation_count};"
        ),
        (
            "pub const MAXIMUM_CONCURRENT_PROJECTILES: usize = "
            f"{evidence.maximum_concurrent_projectiles};"
        ),
        (
            "pub const HANDOFF_STRATEGY_FRAME: u8 = "
            f"{evidence.handoff_strategy_frame};"
        ),
        f"pub const AIM_STEP_COUNT: usize = {len(evidence.aim_steps)};",
        f"pub const TARGET_CROSSING_EXIT_COUNT: usize = {crossing_exit_count};",
        (
            "pub const MOVEMENT_INTERVAL_SUM: usize = "
            f"{evidence.movement_interval_sum};"
        ),
        (
            "pub const MOVEMENT_INTERVAL_COUNT: usize = "
            f"{evidence.movement_interval_count};"
        ),
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct OracleFireAttempt {",
        "    pub distance: u16,",
        "    pub launched: bool,",
        "    pub start_delay: Option<u8>,",
        "}",
        "",
        (
            f"pub const FIRE_ATTEMPTS: [OracleFireAttempt; "
            f"{len(evidence.fire_attempts)}] = ["
        ),
    ]
    for attempt in evidence.fire_attempts:
        delay = (
            "None"
            if attempt.start_delay is None
            else f"Some({attempt.start_delay})"
        )
        lines.extend(
            [
                "    OracleFireAttempt {",
                f"        distance: {attempt.distance:_},",
                f"        launched: {str(attempt.launched).lower()},",
                f"        start_delay: {delay},",
                "    },",
            ]
        )
    lines.extend(
        [
            "];",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "pub struct OracleProjectileLifetime {",
            "    pub homing_steps: u8,",
            "    pub aim_steps: u8,",
            "    pub first_strategy_frame: u8,",
            "    pub aim_correction_mask: u64,",
            "    pub crossed_target: bool,",
            "    pub cruise_steps: u8,",
            "    pub natural_expiry: bool,",
            "}",
            "",
            (
                f"pub const PROJECTILE_LIFETIMES: [OracleProjectileLifetime; "
                f"{len(evidence.lifetimes)}] = ["
            ),
        ]
    )
    for lifetime in evidence.lifetimes:
        lines.extend(
            [
                "    OracleProjectileLifetime {",
                f"        homing_steps: {lifetime.homing_steps},",
                f"        aim_steps: {lifetime.aim_steps},",
                (
                    "        first_strategy_frame: "
                    f"{lifetime.first_strategy_frame},"
                ),
                (
                    "        aim_correction_mask: "
                    f"{lifetime.aim_correction_mask},"
                ),
                (
                    "        crossed_target: "
                    f"{str(lifetime.crossed_target).lower()},"
                ),
                f"        cruise_steps: {lifetime.cruise_steps},",
                (
                    "        natural_expiry: "
                    f"{str(lifetime.natural_expiry).lower()},"
                ),
                "    },",
            ]
        )
    lines.extend(
        [
            "];",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "pub struct OracleAimStep {",
            "    pub previous_position: [i16; 3],",
            "    pub current_position: [i16; 3],",
            "    pub target: [i16; 3],",
            "    pub pitch_before_correction: u8,",
            "    pub yaw_before_correction: u8,",
            "    pub expected_pitch: u8,",
            "    pub expected_yaw: u8,",
            "    pub strategy_frame: u8,",
            "    pub corrected: bool,",
            "    pub terminal: bool,",
            "}",
            "",
            (
                f"pub const AIM_STEPS: [OracleAimStep; "
                f"{len(evidence.aim_steps)}] = ["
            ),
        ]
    )
    for aim in evidence.aim_steps:
        lines.extend(
            [
                "    OracleAimStep {",
                (
                    "        previous_position: ["
                    f"{', '.join(map(str, aim.previous_position))}],"
                ),
                (
                    "        current_position: ["
                    f"{', '.join(map(str, aim.current_position))}],"
                ),
                f"        target: [{', '.join(map(str, aim.target))}],",
                (
                    "        pitch_before_correction: "
                    f"{aim.pitch_before_correction},"
                ),
                (
                    "        yaw_before_correction: "
                    f"{aim.yaw_before_correction},"
                ),
                f"        expected_pitch: {aim.expected_pitch},",
                f"        expected_yaw: {aim.expected_yaw},",
                f"        strategy_frame: {aim.strategy_frame},",
                f"        corrected: {str(aim.corrected).lower()},",
                f"        terminal: {str(aim.terminal).lower()},",
                "    },",
            ]
        )
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-actor-logic", type=Path)
    parser.add_argument("--import-projectile-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if (args.import_actor_logic is None) != (args.import_projectile_logic is None):
        raise SystemExit("both raw logic traces are required for import")
    if args.check and args.import_actor_logic is not None:
        raise SystemExit("--check cannot import raw traces")
    if args.import_actor_logic is not None:
        write_fixture(
            args.fixture,
            import_raw(args.import_actor_logic, args.import_projectile_logic),
        )

    evidence = read_fixture(args.fixture)
    rendered = render(evidence)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != rendered:
            raise SystemExit(f"generated live-projectile module is stale: {args.output}")
        return
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")


if __name__ == "__main__":
    main()
