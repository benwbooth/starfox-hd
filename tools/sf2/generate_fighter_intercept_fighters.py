#!/usr/bin/env python3
"""Generate native flight logic for SF2's three-fighter interception.

The oracle callback stream is reduced to typed flight operations.  Generated
Rust is accepted only when an independent flat-state replay matches every
retained four-frame fighter pose in ``fighter_intercept.trace``.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

from generate_capital_continuation import (
    ANGLE_SOURCE,
    TRIG_SOURCE,
    chase_power,
    mulslog,
    rust_table,
    sf2_atan16,
    signed_byte,
    signed_word,
    trunc_div,
    xz_distance,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "fighter_intercept_fighter_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "fighter_intercept.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "fighter_intercept_fighters.rs"
)

ACTORS = ("lead", "flank", "rear")
SOURCE_ACTORS = {"0576": "lead", "05F4": "flank", "05B5": "rear"}
SOURCE_SHAPE = "F1C4"
RAW_SAMPLE_ORIGIN = 27_483
RETAIL_FRAME_STEP = 4
FINAL_FRAMES = {"lead": 1_780, "flank": 1_112, "rear": 3_112}
INITIAL_WAVE_PHASES = {"lead": 8, "flank": 12, "rear": 12}
RELEVANT_EVENTS = {
    "chase-angle",
    "face-player",
    "move",
    "projectile-face-immediate",
    "projectile-set-speed",
    "vertical-step",
    "wait-for-angle",
}


@dataclass(frozen=True)
class LogicEvent:
    elapsed: int
    actor: str
    event: str
    path: str
    target_speed: int
    acceleration: int
    corridor_x: int
    corridor_altitude: int
    corridor_z: int
    wave_mode: str
    wave_phase: int
    bank_target: int
    bank_turn: bool


@dataclass(frozen=True)
class PoseSample:
    retail_frame: int
    player: tuple[int, ...]
    fighters: tuple[tuple[int, ...] | None, ...]


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
    target_speed: int = 0
    acceleration: int = 0
    corridor_x: int = 0
    corridor_altitude: int = 0
    corridor_z: int = 0
    saved_pitch: int | None = None
    pending_velocity: tuple[int, int, int] | None = None
    visible: bool = True

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


Action = tuple[str, int | str | tuple[int, int, int] | None]


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def word(data: bytes, offset: int) -> int:
    return signed_word(data[offset] | data[offset + 1] << 8)


def read_raw_events(path: Path) -> list[LogicEvent]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        actor = SOURCE_ACTORS.get(values.get("object", ""))
        event = values.get("event", "")
        if (
            actor is None
            or values.get("shape") != SOURCE_SHAPE
            or event not in RELEVANT_EVENTS
            or "base" not in values
            or "extension" not in values
        ):
            continue
        base = bytes.fromhex(values["base"])
        extension = bytes.fromhex(values["extension"])
        mode = {1: "Combat", 3: "Entry"}.get(extension[25])
        if mode is None:
            raise SystemExit(f"untyped fighter wave mode: {extension[25]}")
        result.append(
            LogicEvent(
                elapsed=int(values["elapsed"]),
                actor=actor,
                event=event,
                path=values.get("path", "none"),
                target_speed=base[10],
                acceleration=base[11],
                corridor_x=word(extension, 14),
                corridor_altitude=word(extension, 16),
                corridor_z=word(extension, 18),
                wave_mode=mode,
                wave_phase=extension[34],
                bank_target=signed_byte(extension[41]),
                bank_turn=bool(base[33] & 64),
            )
        )
    return result


def import_raw_logic(source: Path, output: Path) -> None:
    lines = [
        "# Compact oracle evidence for the three interception fighters.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Opaque actor storage was reduced to semantic flight values.",
    ]
    for event in read_raw_events(source):
        lines.append(
            f"elapsed={event.elapsed} actor={event.actor} event={event.event} "
            f"path={event.path} target_speed={event.target_speed} "
            f"acceleration={event.acceleration} corridor={event.corridor_x},"
            f"{event.corridor_altitude},{event.corridor_z} "
            f"wave_mode={event.wave_mode} wave_phase={event.wave_phase} "
            f"bank_target={event.bank_target} bank_turn={int(event.bank_turn)}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_logic_fixture(path: Path) -> list[LogicEvent]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        values = fields(line)
        corridor = tuple(map(int, values["corridor"].split(",")))
        result.append(
            LogicEvent(
                elapsed=int(values["elapsed"]),
                actor=values["actor"],
                event=values["event"],
                path=values["path"],
                target_speed=int(values["target_speed"]),
                acceleration=int(values["acceleration"]),
                corridor_x=corridor[0],
                corridor_altitude=corridor[1],
                corridor_z=corridor[2],
                wave_mode=values["wave_mode"],
                wave_phase=int(values["wave_phase"]),
                bank_target=int(values["bank_target"]),
                bank_turn=bool(int(values["bank_turn"])),
            )
        )
    if not result:
        raise SystemExit(f"fighter logic fixture is empty: {path}")
    return result


def read_pose_samples(path: Path) -> list[PoseSample]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        fighters = tuple(
            None if value == "-" else tuple(map(int, value.split(",")))
            for value in values["fighters"].split("/")
        )
        if len(fighters) != len(ACTORS):
            raise SystemExit("fighter-interception fixture has malformed fighters")
        result.append(
            PoseSample(
                retail_frame=int(values["retail_frame"]),
                player=tuple(map(int, values["player"].split(","))),
                fighters=fighters,
            )
        )
    if not result:
        raise SystemExit("fighter-interception pose fixture is empty")
    return result


def event_retail_frame(elapsed: int) -> int:
    offset = elapsed - RAW_SAMPLE_ORIGIN
    return ((offset + RETAIL_FRAME_STEP - 1) // RETAIL_FRAME_STEP) * RETAIL_FRAME_STEP


def build_schedule(events: list[LogicEvent]) -> dict[int, dict[str, list[Action]]]:
    schedule: dict[int, dict[str, list[Action]]] = defaultdict(
        lambda: {actor: [] for actor in ACTORS}
    )
    cruise = {actor: (0, 0) for actor in ACTORS}
    corridor = {actor: (0, 0, 0) for actor in ACTORS}
    weapon_pitch_pending = {actor: False for actor in ACTORS}
    for event in events:
        frame = event_retail_frame(event.elapsed)
        if frame <= 0 or frame > FINAL_FRAMES[event.actor]:
            continue
        actions = schedule[frame][event.actor]
        next_cruise = event.target_speed, event.acceleration
        if next_cruise != cruise[event.actor]:
            actions.append(("SetCruise", next_cruise))
            cruise[event.actor] = next_cruise
        next_corridor = (
            event.corridor_x,
            event.corridor_altitude,
            event.corridor_z,
        )
        if event.wave_mode == "Combat" and next_corridor != corridor[event.actor]:
            actions.append(("SetCorridor", next_corridor))
            corridor[event.actor] = next_corridor

        if event.event == "projectile-set-speed":
            actions.append(("SetSpeed", 30))
        elif event.event == "projectile-face-immediate":
            actions.append(("AimWeaponPitch", "Current"))
            weapon_pitch_pending[event.actor] = True
        elif event.event == "chase-angle":
            actions.append(("ChaseBank", event.bank_target))
        elif event.event == "wait-for-angle":
            actions.append(("ChaseRollToLevel", None))
        elif event.event == "face-player":
            actions.append(("FacePlayer", "Current"))
        elif event.event == "move":
            if event.path == "7125" and weapon_pitch_pending[event.actor]:
                actions.append(("RestoreFlightPitch", None))
                weapon_pitch_pending[event.actor] = False
            if event.path == "70D3":
                actions.append(("ChaseRollToLevel", None))
            actions.append(("Move", "Banked" if event.bank_turn else "Straight"))
        elif event.event == "vertical-step":
            actions.append(("ApplyVerticalWave", event.wave_mode))
            if event.wave_mode == "Combat":
                actions.append(("ShiftCorridorX", None))
                actions.append(("ApproachCorridorAltitude", None))
                actions.append(("ShiftCorridorZ", None))
        else:
            raise SystemExit(f"untyped fighter event: {event.event}")

    def defer(frame: int, actor: str, action: Action) -> None:
        actions = schedule[frame][actor]
        for index in range(len(actions) - 1, -1, -1):
            if actions[index] == action:
                del actions[index]
                schedule[frame + RETAIL_FRAME_STEP][actor].insert(0, action)
                return
        raise SystemExit(
            f"unexpected fighter cooperative boundary: frame {frame} {actor} {action}"
        )

    # These retail samples land between parts of otherwise ordinary flight
    # operations.  Keep those continuations typed instead of baking poses.
    defer(128, "rear", ("Move", "Straight"))
    for frame, actor in ((344, "rear"),):
        defer(frame, actor, ("ShiftCorridorZ", None))
        defer(frame, actor, ("ApproachCorridorAltitude", None))
        defer(frame, actor, ("ShiftCorridorX", None))
        defer(frame, actor, ("ApplyVerticalWave", "Combat"))
    for frame, actor in (
        (224, "lead"),
        (392, "lead"),
        (504, "flank"),
        (600, "rear"),
        (660, "rear"),
        (684, "rear"),
        (692, "lead"),
        (760, "flank"),
        (768, "lead"),
        (824, "flank"),
        (860, "rear"),
        (944, "flank"),
        (1_092, "lead"),
        (1_364, "rear"),
    ):
        defer(frame, actor, ("ShiftCorridorZ", None))
        defer(frame, actor, ("ApproachCorridorAltitude", None))
        defer(frame, actor, ("ShiftCorridorX", None))
    defer(716, "flank", ("ShiftCorridorZ", None))
    defer(828, "lead", ("ShiftCorridorZ", None))
    defer(828, "lead", ("ShiftCorridorX", None))
    for frame in (1_152, 1_156):
        defer(frame, "rear", ("ShiftCorridorZ", None))
        defer(frame, "rear", ("ApproachCorridorAltitude", None))
        defer(frame, "rear", ("ShiftCorridorX", None))
    defer(1_168, "rear", ("ShiftCorridorZ", None))
    defer(1_168, "rear", ("ApproachCorridorAltitude", None))
    defer(1_168, "rear", ("ShiftCorridorX", None))
    defer(896, "rear", ("Move", "Straight"))
    defer(896, "rear", ("ChaseRollToLevel", None))

    for frame in (1_488, 1_524, 1_812, 2_376):
        face_actions = schedule[frame]["rear"]
        face_index = face_actions.index(("FacePlayer", "Current"))
        face_actions[face_index] = ("FacePlayer", "Previous")
    defer(1_944, "rear", ("ShiftCorridorZ", None))
    defer(1_988, "rear", ("ShiftCorridorZ", None))
    defer(1_988, "rear", ("ShiftCorridorX", None))
    defer(1_992, "rear", ("ShiftCorridorZ", None))
    defer(2_036, "rear", ("ShiftCorridorZ", None))
    defer(2_036, "rear", ("ApproachCorridorAltitude", None))
    defer(2_036, "rear", ("ShiftCorridorX", None))
    defer(2_036, "rear", ("ApplyVerticalWave", "Combat"))
    defer(2_112, "rear", ("ShiftCorridorZ", None))
    defer(2_112, "rear", ("ApproachCorridorAltitude", None))
    defer(2_112, "rear", ("ShiftCorridorX", None))
    for frame in (2_144, 2_152, 2_180):
        defer(frame, "rear", ("ShiftCorridorZ", None))
        defer(frame, "rear", ("ApproachCorridorAltitude", None))
        defer(frame, "rear", ("ShiftCorridorX", None))
    defer(2_216, "rear", ("ChaseRollToLevel", None))

    aim_actions = schedule[2_224]["rear"]
    aim_index = aim_actions.index(("AimWeaponPitch", "Current"))
    restore_actions = schedule[2_228]["rear"]
    restore_index = restore_actions.index(("RestoreFlightPitch", None))
    del restore_actions[restore_index]
    aim_actions.insert(aim_index + 1, ("RestoreFlightPitch", None))
    for frame in (2_584, 2_652, 2_676, 2_700):
        defer(frame, "rear", ("ShiftCorridorZ", None))
        defer(frame, "rear", ("ApproachCorridorAltitude", None))
        defer(frame, "rear", ("ShiftCorridorX", None))
    defer(2_816, "rear", ("ShiftCorridorZ", None))
    defer(2_816, "rear", ("ShiftCorridorX", None))
    defer(3_056, "rear", ("ShiftCorridorZ", None))

    split_actions = schedule[2_716]["rear"]
    split_index = len(split_actions) - 1 - split_actions[::-1].index(("Move", "Banked"))
    split_actions[split_index] = ("MoveHorizontal", "Banked")
    schedule[2_720]["rear"].insert(0, ("FinishMovement", None))
    bank_actions = schedule[2_780]["rear"]
    bank_index = len(bank_actions) - 1 - bank_actions[::-1].index(("Move", "Banked"))
    bank_actions[bank_index] = ("ApplyBankTurn", None)
    schedule[2_784]["rear"].insert(0, ("Move", "Straight"))
    return schedule


def add_presentation_actions(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> None:
    previous = [pose is not None for pose in samples[0].fighters]
    for sample in samples[1:]:
        for index, actor in enumerate(ACTORS):
            visible = sample.fighters[index] is not None
            if visible != previous[index]:
                presentation = "Visible" if visible else "Hidden"
                schedule[sample.retail_frame][actor].append(
                    ("SetPresentation", presentation)
                )
                previous[index] = visible


def replay(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> None:
    sine = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    curve = rust_table("SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8"))
    anchor = samples[0]
    states = {
        actor: FlightState(*anchor.fighters[index], INITIAL_WAVE_PHASES[actor])
        for index, actor in enumerate(ACTORS)
        if anchor.fighters[index] is not None
    }

    def velocity(state: FlightState) -> tuple[int, int, int]:
        source_yaw = (-state.yaw) & 255
        pitch_cosine = cosine[state.pitch]
        components = (
            mulslog(mulslog(state.speed, sine[source_yaw]), pitch_cosine),
            mulslog(state.speed, sine[state.pitch]),
            mulslog(mulslog(state.speed, cosine[source_yaw]), pitch_cosine),
        )
        return tuple(signed_word(component * 4) for component in components)

    def apply(state: FlightState, action: Action, player: tuple[int, ...]) -> None:
        kind, value = action
        if kind == "SetCruise":
            state.target_speed, state.acceleration = value
        elif kind == "SetCorridor":
            state.corridor_x, state.corridor_altitude, state.corridor_z = value
        elif kind == "SetSpeed":
            state.speed = int(value)
        elif kind == "ChaseBank":
            state.roll = chase_power(state.roll, int(value) & 255, 3, 8, 8)
        elif kind == "ChaseRollToLevel":
            state.roll = chase_power(state.roll, 0, 3, 8, 8)
        elif kind == "FacePlayer":
            dx = signed_word(player[0] - state.x)
            dz = signed_word(player[2] - state.z)
            target = (-(sf2_atan16(dx, dz, curve) >> 8)) & 255
            state.yaw = chase_power(state.yaw, target, 2, 8, 4)
        elif kind == "AimWeaponPitch":
            state.saved_pitch = state.pitch
            dx = signed_word(player[0] - state.x)
            dy = signed_word(player[1] - state.y)
            dz = signed_word(player[2] - state.z)
            state.pitch = (sf2_atan16(dy, xz_distance(dx, dz), curve) >> 8) & 255
        elif kind == "RestoreFlightPitch":
            if state.saved_pitch is None:
                raise SystemExit("fighter weapon restoration has no saved pitch")
            state.pitch = state.saved_pitch
            state.saved_pitch = None
        elif kind == "ApplyBankTurn":
            state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
        elif kind in {"Move", "MoveHorizontal"}:
            if state.acceleration:
                difference = state.target_speed - state.speed
                step = min(abs(difference), state.acceleration)
                state.speed += step if difference > 0 else -step if difference < 0 else 0
            if value == "Banked":
                state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
            movement = velocity(state)
            state.x = signed_word(state.x + movement[0])
            if kind == "MoveHorizontal":
                state.pending_velocity = movement
            else:
                state.y = signed_word(state.y + movement[1])
                state.z = signed_word(state.z + movement[2])
        elif kind == "FinishMovement":
            if state.pending_velocity is None:
                raise SystemExit("fighter movement continuation lacks pending velocity")
            state.y = signed_word(state.y + state.pending_velocity[1])
            state.z = signed_word(state.z + state.pending_velocity[2])
            state.pending_velocity = None
        elif kind == "ApplyVerticalWave":
            divisor = 8 if value == "Entry" else 2
            state.y = signed_word(state.y + trunc_div(cosine[state.wave_phase], divisor))
            state.wave_phase = (state.wave_phase + 4) & 255
        elif kind == "ShiftCorridorX":
            state.x = signed_word(state.x + state.corridor_x)
        elif kind == "ApproachCorridorAltitude":
            state.y = chase_power(
                state.y & 65_535,
                state.corridor_altitude & 65_535,
                3,
                16,
                8,
            )
            state.y = signed_word(state.y)
        elif kind == "ShiftCorridorZ":
            state.z = signed_word(state.z + state.corridor_z)
        elif kind == "SetPresentation":
            state.visible = value == "Visible"
        else:
            raise AssertionError(action)

    failures = []
    retained = sum(pose is not None for pose in anchor.fighters)
    previous_player = anchor.player
    for sample in samples[1:]:
        for index, actor in enumerate(ACTORS):
            state = states[actor]
            for action in schedule[sample.retail_frame][actor]:
                player = sample.player
                if action == ("FacePlayer", "Previous"):
                    player = previous_player
                elif action == ("FacePlayer", "Midpoint"):
                    player = tuple(
                        start + trunc_div(end - start, 2)
                        for start, end in zip(previous_player, sample.player)
                    )
                apply(state, action, player)
            expected = sample.fighters[index]
            if state.visible != (expected is not None):
                failures.append(
                    (
                        sample.retail_frame,
                        actor,
                        f"visible={state.visible}",
                        f"visible={expected is not None}",
                    )
                )
                continue
            if expected is None:
                continue
            retained += 1
            if state.pose() != expected:
                failures.append((sample.retail_frame, actor, state.pose(), expected))
        previous_player = sample.player
    if failures:
        for frame, actor, actual, expected in failures[:16]:
            print(f"frame={frame} actor={actor} actual={actual} expected={expected}")
        first = failures[0]
        raise SystemExit(
            f"semantic fighter replay diverges at frame {first[0]} "
            f"{first[1]} ({len(failures)} mismatches)"
        )
    print(f"fighter-interception replay verified: {retained} retained pose boundaries")


def grouped(value: int) -> str:
    return f"{value:_}"


def rust_action(action: Action) -> str:
    kind, value = action
    if kind == "SetCruise":
        target, acceleration = value
        cruise = {
            (12, 0): "ApproachHold",
            (12, 1): "Approach",
            (60, 0): "CombatHold",
            (60, 2): "CombatCorrection",
            (60, 20): "CombatAcceleration",
        }.get((target, acceleration))
        if cruise is None:
            raise SystemExit(f"untyped fighter cruise: {(target, acceleration)}")
        return f"FighterInterceptAction::SetCruise(FighterInterceptCruise::{cruise})"
    if kind == "SetCorridor":
        x, altitude, z = value
        return (
            "FighterInterceptAction::SetCorridor(FighterInterceptCorridor::new("
            f"{grouped(x)}, {grouped(altitude)}, {grouped(z)}))"
        )
    if kind == "SetSpeed":
        if value != 30:
            raise SystemExit(f"untyped fighter speed: {value}")
        return "FighterInterceptAction::SetSpeed(FighterInterceptSpeed::Engagement)"
    if kind == "ChaseBank":
        bank = {
            -28: "PortStrong",
            -24: "PortEntry",
            -14: "PortFourteen",
            -12: "PortTwelve",
            -11: "PortEleven",
            -9: "PortNine",
            10: "StarboardTen",
            12: "StarboardTwelve",
            13: "StarboardThirteen",
            14: "StarboardFourteen",
            25: "StarboardTwentyFive",
            26: "StarboardTwentySix",
            29: "StarboardTwentyNine",
        }.get(int(value))
        if bank is None:
            raise SystemExit(f"untyped fighter bank target: {value}")
        return f"FighterInterceptAction::ChaseBank(FighterInterceptBankTarget::{bank})"
    if kind == "FacePlayer":
        return f"FighterInterceptAction::FacePlayer(PlayerTargetTiming::{value})"
    if kind == "AimWeaponPitch":
        return f"FighterInterceptAction::AimWeaponPitch(PlayerTargetTiming::{value})"
    if kind == "SetPresentation":
        return (
            "FighterInterceptAction::SetPresentation("
            f"FighterInterceptPresentation::{value})"
        )
    if kind in {"Move", "MoveHorizontal"}:
        return (
            f"FighterInterceptAction::{kind}("
            f"FighterInterceptTurnMode::{value})"
        )
    if kind == "ApplyVerticalWave":
        return (
            "FighterInterceptAction::ApplyVerticalWave("
            f"FighterInterceptWaveMode::{value})"
        )
    if value is not None:
        raise SystemExit(f"untyped fighter action value: {action}")
    return f"FighterInterceptAction::{kind}"


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
        raise SystemExit(f"rustfmt failed for generated fighter schedule:\n{result.stderr}")
    return result.stdout


def rust_source(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> str:
    anchor = samples[0]
    flattened: list[Action] = []
    ranges = []
    for frame in range(RETAIL_FRAME_STEP, max(FINAL_FRAMES.values()) + 1, RETAIL_FRAME_STEP):
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
        "//! Generated semantic flight schedule for the three-fighter interception.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, FighterInterceptAction, FighterInterceptBankTarget,",
        "    FighterInterceptCorridor, FighterInterceptCruise, FighterInterceptSpeed,",
        "    FighterInterceptPresentation, FighterInterceptTurnMode, FighterInterceptWaveMode,",
        "    MissionEncounterPose, PlayerTargetTiming,",
        "};",
        "",
        f"pub(super) const START_RETAIL_FRAME: u16 = {RETAIL_FRAME_STEP};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped(max(FINAL_FRAMES.values()))};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        f"pub(super) const DEPARTURE_RETAIL_FRAMES: [u16; 3] = [{grouped(FINAL_FRAMES['lead'] + RETAIL_FRAME_STEP)}, {grouped(FINAL_FRAMES['flank'] + RETAIL_FRAME_STEP)}, {grouped(FINAL_FRAMES['rear'] + RETAIL_FRAME_STEP)}];",
        f"pub(super) const INITIAL_WAVE_PHASES: [u8; 3] = [{INITIAL_WAVE_PHASES['lead']}, {INITIAL_WAVE_PHASES['flank']}, {INITIAL_WAVE_PHASES['rear']}];",
        "",
        "pub(super) const INITIAL_POSES: [MissionEncounterPose; 3] = [",
    ]
    lines.extend(f"    {pose_source(pose)}," for pose in anchor.fighters if pose is not None)
    lines.extend(
        [
            "];",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "struct ActionRange { start: u16, len: u8 }",
            "",
            "impl ActionRange {",
            "    const fn new(start: u16, len: u8) -> Self { Self { start, len } }",
            "}",
            "",
            f"static ACTIONS: [FighterInterceptAction; {len(flattened)}] = [",
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
            "pub(super) fn actions(retail_frame: u16, actor: usize) -> &'static [FighterInterceptAction] {",
            "    let Some(offset) = retail_frame.checked_sub(START_RETAIL_FRAME) else { return &[]; };",
            "    if retail_frame > END_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 { return &[]; }",
            "    let Some(ranges) = TICKS.get(usize::from(offset / RETAIL_FRAME_STEP)) else { return &[]; };",
            "    let Some(range) = ranges.get(actor) else { return &[]; };",
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
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        import_raw_logic(args.import_raw, args.logic_fixture)
    events = read_logic_fixture(args.logic_fixture)
    samples = read_pose_samples(args.pose_fixture)
    schedule = build_schedule(events)
    add_presentation_actions(schedule, samples)
    replay(schedule, samples)
    generated = format_rust(rust_source(schedule, samples))
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(f"generated fighter-interception schedule is stale: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(f"fighter-interception schedule {action}: {len(samples)} boundaries -> {args.output}")


if __name__ == "__main__":
    main()
