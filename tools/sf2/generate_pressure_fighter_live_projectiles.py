#!/usr/bin/env python3
"""Reduce recurring-fighter fire and projectile traces to typed test evidence.

The raw Mesen logs contain source object addresses and path/interpreter state.
This generator validates those details while importing, then emits only
gameplay concepts for the Rust port: launch range, allocation outcome, phase
lengths, and terminal outcome.
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
MINIMUM_AIM_STEPS = 2
MAXIMUM_AIM_STEPS = 5
CRUISE_STEPS = 15
EXPECTED_FIRE_ATTEMPTS = 48
EXPECTED_LIFETIMES = 36


@dataclass(frozen=True)
class RawProjectileEvent:
    elapsed: int
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
    cruise_steps: int
    natural_expiry: bool


@dataclass(frozen=True)
class Evidence:
    actor_sha256: str
    projectile_sha256: str
    movement_interval_sum: int
    movement_interval_count: int
    reusable_allocation_count: int
    maximum_concurrent_projectiles: int
    fire_attempts: tuple[FireAttempt, ...]
    lifetimes: tuple[ProjectileLifetime, ...]


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


def parse_fires(path: Path) -> list[tuple[int, tuple[int, ...], tuple[int, ...]]]:
    fires = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
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
    return fires


def parse_projectile_events(
    path: Path,
) -> dict[str, list[RawProjectileEvent]]:
    events: dict[str, list[RawProjectileEvent]] = defaultdict(list)
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if values.get("shape") != PROJECTILE_SHAPE:
            continue
        events[values["object"]].append(
            RawProjectileEvent(
                elapsed=int(values["elapsed"]),
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


def projectile_lifetimes(
    events_by_source: dict[str, list[RawProjectileEvent]],
) -> tuple[list[ProjectileLifetime], list[tuple[int, int]], int]:
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
        if not MINIMUM_AIM_STEPS <= aim_steps <= MAXIMUM_AIM_STEPS:
            raise SystemExit(
                f"projectile {first.elapsed} has {aim_steps} aim steps"
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
    return lifetimes, intervals, maximum_concurrent


def import_raw(actor_path: Path, projectile_path: Path) -> Evidence:
    fires = parse_fires(actor_path)
    raw_events = parse_projectile_events(projectile_path)
    lifetimes, intervals, maximum_concurrent = projectile_lifetimes(raw_events)

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
        fire_attempts=tuple(attempts),
        lifetimes=tuple(lifetimes),
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
            f"cruise={lifetime.cruise_steps} "
            f"terminal={'expired' if lifetime.natural_expiry else 'terminated'}"
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
    attempts = []
    lifetimes = []
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
                    cruise_steps=int(values["cruise"]),
                    natural_expiry=values["terminal"] == "expired",
                )
            )
    if (
        len(actor_sha256) != 64
        or len(projectile_sha256) != 64
        or movement_interval_sum is None
        or movement_interval_count is None
        or reusable_allocation_count is None
        or maximum_concurrent_projectiles is None
        or len(attempts) != EXPECTED_FIRE_ATTEMPTS
        or len(lifetimes) != EXPECTED_LIFETIMES
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
    return Evidence(
        actor_sha256=actor_sha256,
        projectile_sha256=projectile_sha256,
        movement_interval_sum=movement_interval_sum,
        movement_interval_count=movement_interval_count,
        reusable_allocation_count=reusable_allocation_count,
        maximum_concurrent_projectiles=maximum_concurrent_projectiles,
        fire_attempts=tuple(attempts),
        lifetimes=tuple(lifetimes),
    )


def render(evidence: Evidence) -> str:
    natural_expiry_count = sum(
        lifetime.natural_expiry for lifetime in evidence.lifetimes
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
                f"        cruise_steps: {lifetime.cruise_steps},",
                (
                    "        natural_expiry: "
                    f"{str(lifetime.natural_expiry).lower()},"
                ),
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
