#!/usr/bin/env python3
"""Generate the typed first-sortie capital-flight schedule.

The import path consumes an oracle-only Mesen logic trace and reduces it to a
small event fixture. Generated Rust contains gameplay operations only: angle
chases, player-facing, banked movement, vertical waves, and temporary weapon
aim. Source addresses and opaque object bytes never cross that boundary.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import re
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = Path(__file__).with_name("fixtures") / "first_sortie_capital_logic.trace"
DEFAULT_POSE_FIXTURE = Path(__file__).with_name("fixtures") / "first_sortie_neutral.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "capital_continuation.rs"
TRIG_SOURCE = REPO_ROOT / "rust" / "sf-core" / "src" / "snes_trig.rs"
ANGLE_SOURCE = REPO_ROOT / "rust" / "sf-core" / "src" / "aim_angle.rs"

FIRST_ACTOR = "first"
SECOND_ACTOR = "second"
SOURCE_ACTORS = {"0633": FIRST_ACTOR, "05F4": SECOND_ACTOR}
SOURCE_SHAPE = "F5EC"
ANCHOR_RETAIL_FRAME = 900
START_RETAIL_FRAME = 904
END_RETAIL_FRAME = 7_872
RETAIL_FRAME_STEP = 4
RAW_ANCHOR_ELAPSED = 7_311
RAW_END_ELAPSED = 14_288
NATIVE_SCHEDULE_HANDOFF_RETAIL_FRAME = 2_448
NATIVE_SCHEDULE_HANDOFF_ELAPSED = 8_856

RELEVANT_EVENTS = {
    "wait-for-angle",
    "chase-angle",
    "face-player",
    "move",
    "vertical-step",
    "random-branch",
    "fire",
}
MANEUVER_BRANCH_PATHS = {"717E", "71BA"}
PITCH_PATHS = {
    "7101": ("roll", "Level"),
    "714B": ("roll", "Level"),
    "7185": ("pitch", "Dive"),
    "719D": ("pitch", "Level"),
    "71C1": ("pitch", "Climb"),
    "71D9": ("pitch", "Level"),
}
BANK_TARGETS = {
    12: "StarboardAssertive",
    13: "StarboardStrong",
    14: "StarboardSteep",
    15: "StarboardVerySteep",
    18: "StarboardVeryHard",
    22: "StarboardSharp",
    26: "StarboardExtreme",
    -8: "PortLight",
    -9: "PortShallow",
    -11: "PortMedium",
    -12: "PortFirm",
    -14: "PortSteep",
    -15: "PortVerySteep",
    -17: "PortVeryHard",
    -28: "PortHard",
}


@dataclass(frozen=True)
class LogicEvent:
    elapsed: int
    actor: str
    event: str
    path: str
    pose: tuple[int, ...]
    banked: bool
    bank_target: int
    selected: tuple[int, ...]


@dataclass(frozen=True)
class PoseSample:
    retail_frame: int
    player: tuple[int, ...]
    first: tuple[int, ...] | None
    second: tuple[int, ...] | None


@dataclass
class FlightState:
    x: int
    y: int
    z: int
    pitch: int
    yaw: int
    roll: int
    speed: int
    wave_phase: int
    pending_velocity: tuple[int, int, int] = (0, 0, 0)
    saved_angles: tuple[int, int, int] | None = None

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


Action = tuple[str, str | None]
TraceAction = tuple[str, str | int | tuple[int, ...] | None]


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def signed_byte(value: int) -> int:
    return value - 256 if value >= 128 else value


def signed_word(value: int) -> int:
    value &= 0xFFFF
    return value - 65_536 if value >= 32_768 else value


def import_raw_logic(source: Path, output: Path) -> None:
    retained: list[LogicEvent] = []
    for line in source.read_text(encoding="utf-8").splitlines():
        parsed = fields(line)
        actor = SOURCE_ACTORS.get(parsed.get("object", ""))
        event = parsed.get("event", "")
        if actor is None or event not in RELEVANT_EVENTS or "elapsed" not in parsed:
            continue
        elapsed = int(parsed["elapsed"])
        if not RAW_ANCHOR_ELAPSED <= elapsed < RAW_END_ELAPSED:
            continue
        path = parsed.get("path", "")
        if event == "random-branch" and path not in MANEUVER_BRANCH_PATHS:
            continue
        base = bytes.fromhex(parsed["base"])
        extension = bytes.fromhex(parsed["extension"])
        retained.append(
            LogicEvent(
                elapsed=elapsed,
                actor=actor,
                event=event,
                path=path,
                pose=tuple(map(int, parsed["pose"].split(","))),
                banked=bool(base[33] & 64),
                bank_target=signed_byte(extension[41]),
                selected=tuple(map(int, parsed["selectedpose"].split(","))),
            )
        )

    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    lines = [
        "# Compact oracle evidence for the opening capital-flight tasks.",
        f"# Raw source SHA-256: {digest}",
        "# Opaque object bytes were reduced to the semantic fields below.",
    ]
    for event in retained:
        lines.append(
            f"elapsed={event.elapsed} actor={event.actor} event={event.event} "
            f"path={event.path} pose={','.join(map(str, event.pose))} "
            f"banked={int(event.banked)} bank_target={event.bank_target} "
            f"selected={','.join(map(str, event.selected))}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_logic_fixture(path: Path) -> list[LogicEvent]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        parsed = fields(line)
        result.append(
            LogicEvent(
                elapsed=int(parsed["elapsed"]),
                actor=parsed["actor"],
                event=parsed["event"],
                path=parsed["path"],
                pose=tuple(map(int, parsed["pose"].split(","))),
                banked=bool(int(parsed["banked"])),
                bank_target=int(parsed["bank_target"]),
                selected=tuple(map(int, parsed["selected"].split(","))),
            )
        )
    if not result:
        raise SystemExit(f"capital logic fixture is empty: {path}")
    return result


def read_pose_fixture(path: Path) -> list[PoseSample]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        parsed = fields(line)
        elapsed = int(parsed["elapsed"])
        retail_frame = ANCHOR_RETAIL_FRAME + elapsed - 7_312
        if not ANCHOR_RETAIL_FRAME <= retail_frame <= END_RETAIL_FRAME:
            continue
        actors = {}
        for raw in line.split(" active=[", 1)[1].split("] object=", 1)[0].split(";"):
            parts = raw.split(",")
            if len(parts) >= 9 and parts[1] == SOURCE_SHAPE:
                actor = SOURCE_ACTORS.get(parts[0])
                if actor is not None:
                    actors[actor] = tuple(map(int, parts[2:9]))
        result.append(
            PoseSample(
                retail_frame=retail_frame,
                player=tuple(map(int, parsed["pose"].split(","))),
                first=actors.get(FIRST_ACTOR),
                second=actors.get(SECOND_ACTOR),
            )
        )
    expected_count = (END_RETAIL_FRAME - ANCHOR_RETAIL_FRAME) // RETAIL_FRAME_STEP + 1
    if len(result) != expected_count:
        raise SystemExit(f"expected {expected_count} capital pose samples, found {len(result)}")
    return result


def event_retail_frame(elapsed: int) -> int:
    offset = elapsed - RAW_ANCHOR_ELAPSED
    return ANCHOR_RETAIL_FRAME + ((offset + RETAIL_FRAME_STEP - 1) // RETAIL_FRAME_STEP) * RETAIL_FRAME_STEP


def remove_action(actions: list[Action], expected: Action) -> None:
    count = actions.count(expected)
    if count != 1:
        raise SystemExit(f"timing correction expected one {expected}, found {count}: {actions}")
    actions.remove(expected)


def target_timing(
    target: tuple[int, ...],
    retail_frame: int,
    samples: dict[int, PoseSample],
) -> str:
    current = samples.get(retail_frame)
    previous = samples.get(retail_frame - RETAIL_FRAME_STEP)
    if current is None or previous is None:
        raise SystemExit(f"capital target has no pose boundary at frame {retail_frame}")
    candidates = [
        ("Current", current.player[:3]),
        (
            "Midpoint",
            tuple(
                start + trunc_div(end - start, 2)
                for start, end in zip(previous.player[:3], current.player[:3])
            ),
        ),
        ("Previous", previous.player[:3]),
    ]
    two_ticks_ago = samples.get(retail_frame - 2 * RETAIL_FRAME_STEP)
    if two_ticks_ago is not None:
        candidates.extend(
            (
                ("TwoTicksAgo", two_ticks_ago.player[:3]),
                (
                    "PreviousMidpoint",
                    tuple(
                        start + trunc_div(end - start, 2)
                        for start, end in zip(
                            two_ticks_ago.player[:3], previous.player[:3]
                        )
                    ),
                ),
            )
        )
    for timing, position in candidates:
        if target[:3] == position:
            return timing
    raise SystemExit(
        f"untyped capital player-target timing at frame {retail_frame}: {target[:3]}"
    )


def build_schedule(events: list[LogicEvent]) -> dict[int, dict[str, list[Action]]]:
    schedule: dict[int, dict[str, list[Action]]] = defaultdict(
        lambda: {FIRST_ACTOR: [], SECOND_ACTOR: []}
    )
    per_actor = {
        actor: [event for event in events if event.actor == actor]
        for actor in (FIRST_ACTOR, SECOND_ACTOR)
    }
    maneuver_direction = {}
    for actor, actor_events in per_actor.items():
        for index, event in enumerate(actor_events):
            if event.event != "random-branch":
                continue
            following = next(
                candidate
                for candidate in actor_events[index + 1 :]
                if candidate.event == "wait-for-angle" and candidate.path in {"7185", "71C1"}
            )
            maneuver_direction[(actor, event.elapsed)] = (
                "Port" if signed_byte(following.pose[5]) < 0 else "Starboard"
            )

    for event in events:
        frame = event_retail_frame(event.elapsed)
        actions = schedule[frame][event.actor]
        if event.event == "random-branch":
            actions.append(("BeginPitchManeuver", maneuver_direction[(event.actor, event.elapsed)]))
        elif event.event == "wait-for-angle":
            field, target = PITCH_PATHS[event.path]
            actions.append(("ChaseRollToLevel", None) if field == "roll" else ("ChasePitch", target))
        elif event.event == "chase-angle":
            try:
                target = BANK_TARGETS[event.bank_target]
            except KeyError as error:
                raise SystemExit(f"untyped capital bank target {event.bank_target}") from error
            actions.append(("ChaseBank", target))
        elif event.event == "face-player":
            actions.append(("FacePlayer", None))
        elif event.event == "move":
            if event.path == "714A":
                actions.extend((("CenterAltitude", None), ("ChaseRollToLevel", None)))
            actions.append(("Move", "Banked" if event.banked else "Straight"))
        elif event.event == "vertical-step":
            actions.append(("ApplyVerticalWave", None))

    # Four retail samples land inside a cooperative operation. Preserve those
    # real boundaries with typed pending work instead of storing a pose.
    second_1080 = schedule[1_080][SECOND_ACTOR]
    move = next(action for action in second_1080 if action[0] == "Move")
    remove_action(second_1080, move)
    second_1080.append(("MoveHorizontal", move[1]))
    schedule[1_084][SECOND_ACTOR].insert(0, ("FinishMovement", None))

    remove_action(schedule[1_320][FIRST_ACTOR], ("ApplyVerticalWave", None))
    schedule[1_324][FIRST_ACTOR].insert(0, ("ApplyVerticalWave", None))
    remove_action(schedule[1_564][SECOND_ACTOR], ("ApplyVerticalWave", None))
    schedule[1_568][SECOND_ACTOR].insert(0, ("ApplyVerticalWave", None))
    remove_action(schedule[1_920][SECOND_ACTOR], ("FacePlayer", None))
    schedule[1_924][SECOND_ACTOR].insert(0, ("FacePlayer", None))

    # Weapon aiming is normally restored inside one slice. These two samples
    # observe the authored aim before restoration.
    schedule[2_312][FIRST_ACTOR].append(("AimWeapon", None))
    schedule[2_316][FIRST_ACTOR].insert(0, ("RestoreFlightAngles", None))
    # The second fire slice reaches pitch aiming before the retail sample, but
    # its yaw has already returned to the flight heading at that boundary.
    schedule[2_444][FIRST_ACTOR].append(("AimWeaponPitch", None))
    schedule[2_448][FIRST_ACTOR].insert(0, ("RestoreFlightAngles", None))
    return schedule


def rust_table(name: str, source: str) -> list[int]:
    body = re.search(rf"(?:pub )?static {name}: \[[iu]\d+; 256\] = \[(.*?)\];", source, re.S)
    if body is None:
        raise SystemExit(f"could not read {name} from Rust source")
    return ast.literal_eval("[" + body.group(1) + "]")


def trunc_div(value: int, divisor: int) -> int:
    return int(value / divisor)


def mulslog(a: int, b: int) -> int:
    a = signed_byte(a & 0xFF)
    b = signed_byte(b & 0xFF)
    magnitude = ((((abs(a) & 0xFF) << 1) & 0xFF) * (abs(b) & 0xFF) >> 8) & 0xFF
    return signed_byte((-magnitude if (a < 0) ^ (b < 0) else magnitude) & 0xFF)


def chase_power(current: int, target: int, divisions: int, bits: int, minimum: int) -> int:
    mask = (1 << bits) - 1
    if current == target:
        return current
    difference = (target - current) & mask
    if difference & (1 << (bits - 1)):
        difference -= 1 << bits
    if 0 <= difference < minimum:
        difference = minimum
    elif -minimum < difference < 0:
        difference = -minimum
    for _ in range(divisions):
        difference = trunc_div(difference, 2)
    return (current + difference) & mask


def sf2_atan16(x: int, y: int, curve: list[int]) -> int:
    original_x = x & 0xFFFF
    original_y = y & 0xFFFF
    absolute = lambda value: value if signed_word(value) >= 0 else (-value) & 0xFFFF
    if original_y == 0:
        angle = 16_384
    else:
        numerator = absolute(original_x)
        denominator = absolute(original_y)
        if numerator == denominator:
            angle = 8_192
        else:
            swapped = signed_word(numerator - denominator) >= 0
            if swapped:
                numerator, denominator = denominator, numerator
            ratio = 0x7FFF if denominator == 0 else (numerator << 14) // denominator
            sample = curve[((ratio >> 5) & 0xFFFE) // 2]
            angle = (16_384 - sample) & 0xFFFF if swapped else sample
    if signed_word(original_x ^ original_y) < 0:
        angle = (-angle) & 0xFFFF
    if signed_word(original_y) < 0:
        angle = (angle + 32_768) & 0xFFFF
    return angle


def xz_distance(dx: int, dz: int) -> int:
    absolute = lambda value: (-value) & 0xFFFF if signed_word(value) < 0 else value & 0xFFFF
    half = lambda value: (signed_word(value) >> 1) & 0xFFFF
    x = half(absolute(dx))
    z = half(absolute(dz))
    total_range = ((z + x) & 0xFFFF) << 1 & 0xFFFF
    maximum = x if signed_word(z - x) < 0 else z
    total = (maximum + total_range) & 0xFFFF
    value = (half(total) + total) & 0xFFFF
    return signed_word(half(half(value)))


def flight_velocity(
    state: FlightState, sine: list[int], cosine: list[int]
) -> tuple[int, int, int]:
    source_yaw = (-state.yaw) & 0xFF
    pitch_cosine = cosine[state.pitch]
    components = (
        mulslog(mulslog(state.speed, sine[source_yaw]), pitch_cosine),
        mulslog(state.speed, sine[state.pitch]),
        mulslog(mulslog(state.speed, cosine[source_yaw]), pitch_cosine),
    )
    return tuple(signed_word(component * 4) for component in components)


def apply_trace_action(
    state: FlightState,
    action: TraceAction,
    sine: list[int],
    cosine: list[int],
    curve: list[int],
) -> None:
    kind, value = action
    if kind == "BeginPitchManeuver":
        state.roll = 4 if value == "Starboard" else 252
    elif kind == "ChasePitch":
        state.pitch = chase_power(
            state.pitch,
            {"Level": 0, "Dive": 206, "Climb": 50}[str(value)],
            3,
            8,
            8,
        )
    elif kind == "ChaseRollToLevel":
        state.roll = chase_power(state.roll, 0, 3, 8, 8)
    elif kind == "ChaseBank":
        state.roll = chase_power(state.roll, int(value), 3, 8, 8)
    elif kind == "FacePlayer":
        assert isinstance(value, tuple)
        dx = signed_word(value[0] - state.x)
        dz = signed_word(value[2] - state.z)
        target = (-(sf2_atan16(dx, dz, curve) >> 8)) & 0xFF
        state.yaw = chase_power(state.yaw, target, 2, 8, 4)
    elif kind == "CenterAltitude":
        state.y = signed_word(chase_power(state.y & 0xFFFF, 0, 3, 16, 8))
    elif kind == "ApplyBankTurn":
        state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 0xFF
    elif kind in {"Move", "MoveHorizontal"}:
        if value == "Banked":
            state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 0xFF
        movement = flight_velocity(state, sine, cosine)
        state.x = signed_word(state.x + movement[0])
        if kind == "MoveHorizontal":
            state.pending_velocity = movement
        else:
            state.y = signed_word(state.y + movement[1])
            state.z = signed_word(state.z + movement[2])
            state.pending_velocity = (0, 0, 0)
    elif kind == "FinishMovement":
        state.y = signed_word(state.y + state.pending_velocity[1])
        state.z = signed_word(state.z + state.pending_velocity[2])
        state.pending_velocity = (0, 0, 0)
    elif kind == "ApplyVerticalWave":
        state.y = signed_word(state.y + cosine[state.wave_phase])
        state.wave_phase = (state.wave_phase + 1) & 0xFF
    elif kind in {"AimWeapon", "AimWeaponPitch"}:
        assert isinstance(value, tuple)
        state.saved_angles = state.pitch, state.yaw, state.roll
        dx = signed_word(value[0] - state.x)
        dy = signed_word(value[1] - state.y)
        dz = signed_word(value[2] - state.z)
        state.pitch = sf2_atan16(dy, xz_distance(dx, dz), curve) >> 8
        if kind == "AimWeapon":
            state.yaw = (-(sf2_atan16(dx, dz, curve) >> 8)) & 0xFF
    elif kind == "RestoreFlightAngles":
        if state.saved_angles is None:
            raise SystemExit("capital flight-angle restoration has no saved state")
        state.pitch, state.yaw, state.roll = state.saved_angles
        state.saved_angles = None
    else:
        raise AssertionError(action)


def trace_event_actions(
    event: LogicEvent,
    maneuver_directions: dict[tuple[str, int], str],
) -> list[TraceAction]:
    if event.event == "random-branch":
        return [
            (
                "BeginPitchManeuver",
                maneuver_directions[(event.actor, event.elapsed)],
            )
        ]
    if event.event == "wait-for-angle":
        field, target = PITCH_PATHS[event.path]
        return [
            ("ChaseRollToLevel", None)
            if field == "roll"
            else ("ChasePitch", target)
        ]
    if event.event == "chase-angle":
        return [("ChaseBank", event.bank_target)]
    if event.event == "face-player":
        return [("FacePlayer", event.selected)]
    if event.event == "move":
        actions: list[TraceAction] = []
        if event.path in {"714A", "719C", "71D8"}:
            actions.extend(
                (("CenterAltitude", None), ("ChaseRollToLevel", None))
            )
        if event.banked:
            actions.append(("ApplyBankTurn", None))
        actions.extend(
            (
                ("MoveHorizontal", "Straight"),
                ("FinishMovement", None),
            )
        )
        return actions
    if event.event == "vertical-step":
        return [("ApplyVerticalWave", None)]
    if event.event == "fire":
        return [("AimWeapon", event.selected)]
    return []


def shipping_action(
    action: TraceAction,
    retail_frame: int,
    samples: dict[int, PoseSample],
) -> Action:
    kind, value = action
    if kind == "ChaseBank":
        try:
            return kind, BANK_TARGETS[int(value)]
        except KeyError as error:
            raise SystemExit(f"untyped capital bank target {value}") from error
    if kind in {"FacePlayer", "AimWeapon", "AimWeaponPitch"}:
        assert isinstance(value, tuple)
        return f"{kind}At", target_timing(value, retail_frame, samples)
    assert value is None or isinstance(value, str)
    return kind, value


def compact_movement_actions(actions: list[Action]) -> list[Action]:
    result: list[Action] = []
    index = 0
    banked = [
        ("ApplyBankTurn", None),
        ("MoveHorizontal", "Straight"),
        ("FinishMovement", None),
    ]
    straight = [
        ("MoveHorizontal", "Straight"),
        ("FinishMovement", None),
    ]
    while index < len(actions):
        if actions[index : index + len(banked)] == banked:
            result.append(("Move", "Banked"))
            index += len(banked)
        elif actions[index : index + len(straight)] == straight:
            result.append(("Move", "Straight"))
            index += len(straight)
        else:
            result.append(actions[index])
            index += 1
    return result


def extend_schedule(
    schedule: dict[int, dict[str, list[Action]]],
    events: list[LogicEvent],
    samples: list[PoseSample],
) -> None:
    sine = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    curve = rust_table(
        "SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8")
    )
    if samples[0].first is None or samples[0].second is None:
        raise SystemExit("capital pose fixture lacks both actors at the handoff")
    states = {
        FIRST_ACTOR: FlightState(*samples[0].first, wave_phase=104),
        SECOND_ACTOR: FlightState(*samples[0].second, wave_phase=101),
    }
    sample_by_frame = {sample.retail_frame: sample for sample in samples}

    for sample in samples[1:]:
        if sample.retail_frame > NATIVE_SCHEDULE_HANDOFF_RETAIL_FRAME:
            break
        for actor, state in states.items():
            for action in schedule[sample.retail_frame][actor]:
                kind, value = action
                trace_value: str | int | tuple[int, ...] | None = value
                if kind == "ChaseBank":
                    trace_value = next(
                        units
                        for units, name in BANK_TARGETS.items()
                        if name == value
                    )
                elif kind in {"FacePlayer", "AimWeapon", "AimWeaponPitch"}:
                    trace_value = sample.player
                apply_trace_action(
                    state,
                    (kind, trace_value),
                    sine,
                    cosine,
                    curve,
                )
            expected = sample.first if actor == FIRST_ACTOR else sample.second
            if expected is None or state.pose() != expected:
                raise SystemExit(
                    f"capital native handoff diverges at frame "
                    f"{sample.retail_frame} {actor}"
                )

    per_actor = {
        actor: [
            event
            for event in events
            if event.actor == actor
            and event.elapsed > NATIVE_SCHEDULE_HANDOFF_ELAPSED
        ]
        for actor in (FIRST_ACTOR, SECOND_ACTOR)
    }
    maneuver_directions = {}
    for actor, actor_events in per_actor.items():
        for index, event in enumerate(actor_events):
            if event.event != "random-branch":
                continue
            following = next(
                candidate
                for candidate in actor_events[index + 1 :]
                if candidate.event == "wait-for-angle"
                and candidate.path in {"7185", "71C1"}
            )
            maneuver_directions[(actor, event.elapsed)] = (
                "Port" if signed_byte(following.pose[5]) < 0 else "Starboard"
            )

    indices = {FIRST_ACTOR: 0, SECOND_ACTOR: 0}
    pending: dict[str, list[TraceAction]] = {
        FIRST_ACTOR: [],
        SECOND_ACTOR: [],
    }
    for sample in samples:
        if sample.retail_frame <= NATIVE_SCHEDULE_HANDOFF_RETAIL_FRAME:
            continue
        for actor, state in states.items():
            expected = sample.first if actor == FIRST_ACTOR else sample.second
            if expected is None:
                continue
            assigned: list[Action] = []
            for _ in range(64):
                actor_events = per_actor[actor]
                next_event = (
                    actor_events[indices[actor]]
                    if indices[actor] < len(actor_events)
                    else None
                )
                if state.pose() == expected and (
                    assigned
                    or pending[actor]
                    or next_event is None
                    or next_event.pose == expected
                ):
                    break
                if pending[actor]:
                    action = pending[actor].pop(0)
                    apply_trace_action(state, action, sine, cosine, curve)
                    assigned.append(
                        shipping_action(action, sample.retail_frame, sample_by_frame)
                    )
                    continue
                if next_event is None:
                    break
                if state.saved_angles is not None:
                    flight_pose = (
                        state.x,
                        state.y,
                        state.z,
                        *state.saved_angles,
                        state.speed,
                    )
                    if next_event.pose == flight_pose:
                        action = ("RestoreFlightAngles", None)
                        apply_trace_action(state, action, sine, cosine, curve)
                        assigned.append(action)
                        continue
                indices[actor] += 1
                pending[actor] = trace_event_actions(
                    next_event, maneuver_directions
                )
            if state.pose() != expected:
                raise SystemExit(
                    f"semantic capital extension diverges at frame "
                    f"{sample.retail_frame} {actor}: "
                    f"actual={state.pose()} expected={expected}"
                )
            schedule[sample.retail_frame][actor].extend(
                compact_movement_actions(assigned)
            )


def replay(schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]) -> None:
    trig_source = TRIG_SOURCE.read_text(encoding="utf-8")
    sine = rust_table("SINTAB", trig_source)
    cosine = rust_table("COSTAB", trig_source)
    curve = rust_table("SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8"))
    if samples[0].first is None or samples[0].second is None:
        raise SystemExit("capital pose fixture lacks both actors at the handoff")
    states = {
        FIRST_ACTOR: FlightState(*samples[0].first, wave_phase=104),
        SECOND_ACTOR: FlightState(*samples[0].second, wave_phase=101),
    }

    def velocity(state: FlightState) -> tuple[int, int, int]:
        source_yaw = (-state.yaw) & 0xFF
        pitch_cosine = cosine[state.pitch]
        components = (
            mulslog(mulslog(state.speed, sine[source_yaw]), pitch_cosine),
            mulslog(state.speed, sine[state.pitch]),
            mulslog(mulslog(state.speed, cosine[source_yaw]), pitch_cosine),
        )
        return tuple(signed_word(component * 4) for component in components)

    def apply(
        state: FlightState,
        action: Action,
        player: tuple[int, ...],
        previous_player: tuple[int, ...],
        two_ticks_ago_player: tuple[int, ...],
    ) -> None:
        kind, value = action
        target_position = {
            "Current": player,
            "Midpoint": tuple(
                start + trunc_div(end - start, 2)
                for start, end in zip(previous_player, player)
            ),
            "Previous": previous_player,
            "PreviousMidpoint": tuple(
                start + trunc_div(end - start, 2)
                for start, end in zip(two_ticks_ago_player, previous_player)
            ),
            "TwoTicksAgo": two_ticks_ago_player,
        }.get(str(value), player)
        if kind == "BeginPitchManeuver":
            state.roll = 4 if value == "Starboard" else 252
        elif kind == "ChasePitch":
            target = {"Level": 0, "Dive": 206, "Climb": 50}[value]
            state.pitch = chase_power(state.pitch, target, 3, 8, 8)
        elif kind == "ChaseRollToLevel":
            state.roll = chase_power(state.roll, 0, 3, 8, 8)
        elif kind == "ChaseBank":
            target = next(units for units, name in BANK_TARGETS.items() if name == value) & 0xFF
            state.roll = chase_power(state.roll, target, 3, 8, 8)
        elif kind in {"FacePlayer", "FacePlayerAt"}:
            dx = signed_word(target_position[0] - state.x)
            dz = signed_word(target_position[2] - state.z)
            target = (-(sf2_atan16(dx, dz, curve) >> 8)) & 0xFF
            state.yaw = chase_power(state.yaw, target, 2, 8, 4)
        elif kind == "CenterAltitude":
            state.y = signed_word(chase_power(state.y & 0xFFFF, 0, 3, 16, 8))
        elif kind == "ApplyBankTurn":
            state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 0xFF
        elif kind in {"Move", "MoveHorizontal"}:
            if value == "Banked":
                state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 0xFF
            state.pending_velocity = velocity(state)
            state.x = signed_word(state.x + state.pending_velocity[0])
            if kind == "Move":
                state.y = signed_word(state.y + state.pending_velocity[1])
                state.z = signed_word(state.z + state.pending_velocity[2])
                state.pending_velocity = (0, 0, 0)
        elif kind == "FinishMovement":
            state.y = signed_word(state.y + state.pending_velocity[1])
            state.z = signed_word(state.z + state.pending_velocity[2])
            state.pending_velocity = (0, 0, 0)
        elif kind == "ApplyVerticalWave":
            state.y = signed_word(state.y + cosine[state.wave_phase])
            state.wave_phase = (state.wave_phase + 1) & 0xFF
        elif kind in {"AimWeapon", "AimWeaponAt"}:
            state.saved_angles = state.pitch, state.yaw, state.roll
            dx = signed_word(target_position[0] - state.x)
            dy = signed_word(target_position[1] - state.y)
            dz = signed_word(target_position[2] - state.z)
            state.pitch = sf2_atan16(dy, xz_distance(dx, dz), curve) >> 8
            state.yaw = (-(sf2_atan16(dx, dz, curve) >> 8)) & 0xFF
        elif kind == "AimWeaponPitch":
            state.saved_angles = state.pitch, state.yaw, state.roll
            dx = signed_word(player[0] - state.x)
            dy = signed_word(player[1] - state.y)
            dz = signed_word(player[2] - state.z)
            state.pitch = sf2_atan16(dy, xz_distance(dx, dz), curve) >> 8
        elif kind == "RestoreFlightAngles":
            if state.saved_angles is None:
                raise SystemExit("capital flight-angle restoration has no saved state")
            state.pitch, state.yaw, state.roll = state.saved_angles
            state.saved_angles = None
        else:
            raise AssertionError(action)

    failures = []
    previous_player = samples[0].player
    two_ticks_ago_player = samples[0].player
    for sample in samples[1:]:
        for actor in (FIRST_ACTOR, SECOND_ACTOR):
            for action in schedule[sample.retail_frame][actor]:
                apply(
                    states[actor],
                    action,
                    sample.player,
                    previous_player,
                    two_ticks_ago_player,
                )
            expected = sample.first if actor == FIRST_ACTOR else sample.second
            if expected is None:
                continue
            if states[actor].pose() != expected:
                failures.append((sample.retail_frame, actor, states[actor].pose(), expected))
        two_ticks_ago_player = previous_player
        previous_player = sample.player
    if failures:
        first = failures[0]
        raise SystemExit(
            f"semantic capital replay diverges at retail frame {first[0]} {first[1]}: "
            f"actual={first[2]} expected={first[3]} ({len(failures)} mismatches)"
        )


def rust_action(action: Action) -> str:
    kind, value = action
    if value is None:
        return f"CapitalFlightAction::{kind}"
    if kind in {"FacePlayerAt", "AimWeaponAt", "AimWeaponPitchAt"}:
        action_name = kind.removesuffix("At")
        return (
            f"CapitalFlightAction::{action_name}At("
            f"PlayerTargetTiming::{value})"
        )
    type_name = {
        "BeginPitchManeuver": "CapitalManeuverDirection",
        "ChasePitch": "CapitalPitchTarget",
        "ChaseBank": "CapitalBankTarget",
        "Move": "CapitalTurnMode",
        "MoveHorizontal": "CapitalTurnMode",
    }[kind]
    return f"CapitalFlightAction::{kind}({type_name}::{value})"


def grouped_number(value: int) -> str:
    return f"{value:_}"


def rust_source(
    schedule: dict[int, dict[str, list[Action]]],
    samples: list[PoseSample],
) -> str:
    first_present = [sample for sample in samples if sample.first is not None]
    second_present = [sample for sample in samples if sample.second is not None]
    second_inactive = [
        sample.retail_frame
        for sample in samples
        if sample.second is None
        and second_present[0].retail_frame
        < sample.retail_frame
        < second_present[-1].retail_frame
    ]
    if not first_present or not second_present or not second_inactive:
        raise SystemExit("capital fixture lacks a complete actor lifetime")
    if second_inactive != list(
        range(
            second_inactive[0],
            second_inactive[-1] + RETAIL_FRAME_STEP,
            RETAIL_FRAME_STEP,
        )
    ):
        raise SystemExit("second capital has a non-contiguous inactive window")
    first_departure = first_present[-1].retail_frame + RETAIL_FRAME_STEP
    second_departure = second_present[-1].retail_frame + RETAIL_FRAME_STEP
    second_inactive_start = second_inactive[0]
    second_resume = second_inactive[-1] + RETAIL_FRAME_STEP

    flattened: list[Action] = []
    ranges = []
    for frame in range(START_RETAIL_FRAME, END_RETAIL_FRAME + 1, RETAIL_FRAME_STEP):
        actor_ranges = []
        for actor in (FIRST_ACTOR, SECOND_ACTOR):
            start = len(flattened)
            actions = schedule[frame][actor]
            flattened.extend(actions)
            actor_ranges.append((start, len(actions)))
        ranges.append(tuple(actor_ranges))

    lines = [
        "//! Generated semantic continuation of the retail first-sortie capital craft.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    CapitalBankTarget, CapitalFlightAction, CapitalManeuverDirection, CapitalPitchTarget,",
        "    CapitalTurnMode, PlayerTargetTiming,",
        "};",
        "",
        f"pub(super) const START_RETAIL_FRAME: u16 = {grouped_number(START_RETAIL_FRAME)};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped_number(END_RETAIL_FRAME)};",
        "pub(super) const FIRST_DEPARTURE_RETAIL_FRAME: u16 = "
        f"{grouped_number(first_departure)};",
        "pub(super) const SECOND_INACTIVE_RETAIL_FRAME: u16 = "
        f"{grouped_number(second_inactive_start)};",
        "pub(super) const SECOND_RESUME_RETAIL_FRAME: u16 = "
        f"{grouped_number(second_resume)};",
        "pub(super) const SECOND_DEPARTURE_RETAIL_FRAME: u16 = "
        f"{grouped_number(second_departure)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct ActionRange {",
        "    start: u16,",
        "    len: u8,",
        "}",
        "",
        "impl ActionRange {",
        "    const fn new(start: u16, len: u8) -> Self {",
        "        Self { start, len }",
        "    }",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct ActionRangePair {",
        "    first: ActionRange,",
        "    second: ActionRange,",
        "}",
        "",
        "impl ActionRangePair {",
        "    const fn new(first_start: u16, first_len: u8, second_start: u16, second_len: u8) -> Self {",
        "        Self {",
        "            first: ActionRange::new(first_start, first_len),",
        "            second: ActionRange::new(second_start, second_len),",
        "        }",
        "    }",
        "}",
        "",
        f"static ACTIONS: [CapitalFlightAction; {len(flattened)}] = [",
    ]
    lines.extend(f"    {rust_action(action)}," for action in flattened)
    lines.extend(["];", "", f"static TICKS: [ActionRangePair; {len(ranges)}] = ["])
    for (first_start, first_len), (second_start, second_len) in ranges:
        lines.append(
            "    ActionRangePair::new("
            f"{first_start}, {first_len}, {second_start}, {second_len}),"
        )
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16, first: bool) -> &'static [CapitalFlightAction] {",
            "    let Some(offset) = retail_frame.checked_sub(START_RETAIL_FRAME) else {",
            "        return &[];",
            "    };",
            "    if retail_frame > END_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 {",
            "        return &[];",
            "    }",
            "    let Some(pair) = TICKS.get(usize::from(offset / RETAIL_FRAME_STEP)) else {",
            "        return &[];",
            "    };",
            "    let range = if first { pair.first } else { pair.second };",
            "    let start = usize::from(range.start);",
            "    &ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path, help="reduce a raw Mesen logic trace first")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        import_raw_logic(args.import_raw, args.logic_fixture)
    events = read_logic_fixture(args.logic_fixture)
    samples = read_pose_fixture(args.pose_fixture)
    schedule = build_schedule(
        [
            event
            for event in events
            if event.elapsed <= NATIVE_SCHEDULE_HANDOFF_ELAPSED
        ]
    )
    extend_schedule(schedule, events, samples)
    replay(schedule, samples)
    generated = rust_source(schedule, samples)
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(f"generated capital continuation is stale: {args.output}")
        print(f"capital continuation verified: {len(samples)} exact pose samples")
        return
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(generated, encoding="utf-8")
    print(f"capital continuation: {len(samples)} exact pose samples -> {args.output}")


if __name__ == "__main__":
    main()
