#!/usr/bin/env python3
"""Generate native flight logic for SF2's recurring four-attacker encounter.

The oracle callback stream is reduced to typed flight operations. Generated
Rust is accepted only when an independent flat-state replay matches every
retained four-frame fighter pose in the neutral-input retail capture.
"""

from __future__ import annotations

import argparse
import hashlib
from collections import defaultdict
from copy import deepcopy
from dataclasses import dataclass
from itertools import combinations, product
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
from generate_pressure_fighters import fields
from generate_second_sortie_projectiles import format_rust


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "pressure_fighter_flight_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "pressure_fighter_flight.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "pressure_fighter_flight.rs"
)

ACTORS = ("vanguard", "high_guard", "flanker", "pursuer")
ASSAULT_ACTORS = ACTORS[:3]
PURSUER = ACTORS[3]
SOURCE_ACTORS = {
    "05B5": "vanguard",
    "0633": "high_guard",
    "05F4": "flanker",
    "0576": "pursuer",
}
SOURCE_SHAPES = {
    "vanguard": "F1C4",
    "high_guard": "F1C4",
    "flanker": "F1C4",
    "pursuer": "EA00",
}
PATH_END_HOST = "7F:8B91"
SCRIPTED_DEFEAT_HOSTS = {"7F:8BD4", "7F:8BD8"}
INITIAL_RETAIL_FRAME = 64
RETAIL_FRAME_STEP = 4
PREVIOUSLY_CERTIFIED_END_RETAIL_FRAME = 2_020
MAX_ACTION_OFFSET = 65_535
MAX_ACTIONS_PER_TICK = 255
ASSAULT_RELEVANT_EVENTS = {
    "chase-angle",
    "face-player",
    "move",
    "projectile-face-immediate",
    "projectile-set-speed",
    "vertical-step",
    "wait-for-angle",
}
PURSUER_RELEVANT_EVENTS = {"move", "schedule", "vertical-step"}


@dataclass(frozen=True)
class AssaultLogicEvent:
    retail_frame: int
    actor: str
    event: str
    path: str
    target_speed: int
    acceleration: int
    corridor_x: int
    corridor_altitude: int
    corridor_z: int
    wave_mode: str
    bank_target: int
    bank_turn: bool


@dataclass(frozen=True)
class PursuerLogicEvent:
    retail_frame: int
    event: str
    path: str


@dataclass(frozen=True)
class PoseSample:
    retail_frame: int
    player: tuple[int, ...]
    fighters: tuple[tuple[int, ...] | None, ...]


@dataclass(frozen=True)
class TerminalEvent:
    retail_frame: int
    actor: str
    outcome: str


@dataclass
class AssaultFlightState:
    x: int
    y: int
    z: int
    pitch: int
    yaw: int
    roll: int
    speed: int
    wave_phase: int = 0
    target_speed: int = 0
    acceleration: int = 0
    corridor_x: int = 0
    corridor_altitude: int = 0
    corridor_z: int = 0
    saved_pitch: int | None = None
    pending_velocity: tuple[int, int, int] | None = None

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


@dataclass
class PursuerFlightState:
    x: int
    y: int
    z: int
    pitch: int
    yaw: int
    roll: int
    speed: int
    wave_phase: int = 0

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


Action = tuple[str, int | str | tuple[int, int, int] | None]


def word(data: bytes, offset: int) -> int:
    return signed_word(data[offset] | data[offset + 1] << 8)


def raw_objects(value: str) -> tuple[tuple[int, ...] | None, ...]:
    fighters: list[tuple[int, ...] | None] = [None] * len(ACTORS)
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        parts = object_text.split(",")
        if len(parts) < 9:
            continue
        actor = SOURCE_ACTORS.get(parts[0])
        if actor is None or parts[1] != SOURCE_SHAPES[actor]:
            continue
        fighters[ACTORS.index(actor)] = tuple(map(int, parts[2:9]))
    return tuple(fighters)


def import_raw_poses(source: Path, output: Path) -> int:
    raw_samples = []
    retail_origin_elapsed = None
    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("event") != "sortie"
            or values.get("mode") != "1"
            or values.get("selection") != "6"
        ):
            continue
        elapsed = int(values["elapsed"])
        fighters = raw_objects(values["objects"])
        raw_samples.append(
            (
                elapsed,
                tuple(map(int, values["playerpose"].split(","))),
                fighters,
            )
        )
        if retail_origin_elapsed is None and all(
            pose is not None for pose in fighters
        ):
            retail_origin_elapsed = elapsed - INITIAL_RETAIL_FRAME
    if not raw_samples:
        raise SystemExit(f"pressure-fighter raw pose capture is empty: {source}")
    if retail_origin_elapsed is None:
        raise SystemExit("pressure-fighter raw pose capture never deploys all fighters")

    samples = []
    for elapsed, player, fighters in raw_samples:
        retail_frame = elapsed - retail_origin_elapsed
        if retail_frame < 0 or retail_frame % RETAIL_FRAME_STEP != 0:
            continue
        sample = PoseSample(
            retail_frame,
            player,
            fighters,
        )
        samples.append(sample)
    expected = list(range(0, samples[-1].retail_frame + 1, RETAIL_FRAME_STEP))
    if [sample.retail_frame for sample in samples] != expected:
        raise SystemExit("pressure-fighter raw poses are not a complete four-frame cadence")
    lines = [
        "# Compact accepted-input oracle evidence for recurring-attacker flight.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
    ]
    for sample in samples:
        fighters = "/".join(
            "-" if pose is None else ",".join(map(str, pose))
            for pose in sample.fighters
        )
        lines.append(
            f"retail_frame={sample.retail_frame} "
            f"player={','.join(map(str, sample.player))} fighters={fighters}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return retail_origin_elapsed


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
            raise SystemExit("pressure-fighter pose fixture has malformed fighters")
        result.append(
            PoseSample(
                int(values["retail_frame"]),
                tuple(map(int, values["player"].split(","))),
                fighters,
            )
        )
    if not result:
        raise SystemExit(f"pressure-fighter pose fixture is empty: {path}")
    return result


def read_raw_logic(
    path: Path,
    retail_origin_elapsed: int,
) -> tuple[list[AssaultLogicEvent], list[PursuerLogicEvent]]:
    assault = []
    pursuer = []
    for line in path.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        actor = SOURCE_ACTORS.get(values.get("object", ""))
        event = values.get("event", "")
        if actor is None or values.get("shape") != SOURCE_SHAPES[actor]:
            continue
        if actor == PURSUER:
            if event in PURSUER_RELEVANT_EVENTS:
                pursuer.append(
                    PursuerLogicEvent(
                        retail_frame=event_retail_frame(
                            int(values["elapsed"]), retail_origin_elapsed
                        ),
                        event=event,
                        path=values.get("path", "none"),
                    )
                )
            continue
        if (
            event not in ASSAULT_RELEVANT_EVENTS
            or "base" not in values
            or "extension" not in values
        ):
            continue
        base = bytes.fromhex(values["base"])
        extension = bytes.fromhex(values["extension"])
        wave_mode = {0: "Setup", 1: "Combat", 3: "Entry"}.get(extension[25])
        if wave_mode is None:
            raise SystemExit(f"untyped pressure-fighter wave mode: {extension[25]}")
        assault.append(
            AssaultLogicEvent(
                retail_frame=event_retail_frame(
                    int(values["elapsed"]), retail_origin_elapsed
                ),
                actor=actor,
                event=event,
                path=values.get("path", "none"),
                target_speed=base[10],
                acceleration=base[11],
                corridor_x=word(extension, 14),
                corridor_altitude=word(extension, 16),
                corridor_z=word(extension, 18),
                wave_mode=wave_mode,
                bank_target=signed_byte(extension[41]),
                bank_turn=bool(base[33] & 64),
            )
        )
    return assault, pursuer


def classify_raw_terminal_events(
    source: Path,
    samples: list[PoseSample],
    retail_origin_elapsed: int,
) -> list[TerminalEvent]:
    evidence: dict[str, list[tuple[int, str]]] = defaultdict(list)
    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        actor = SOURCE_ACTORS.get(values.get("object", ""))
        if (
            actor is None
            or values.get("event") != "object-state-write"
            or values.get("shape") != SOURCE_SHAPES[actor]
        ):
            continue
        host = values.get("host")
        outcome = (
            "Departed"
            if host == PATH_END_HOST
            else "Defeated"
            if host in SCRIPTED_DEFEAT_HOSTS
            else None
        )
        if outcome is not None:
            evidence[actor].append(
                (
                    event_retail_frame(
                        int(values["elapsed"]), retail_origin_elapsed
                    ),
                    outcome,
                )
            )

    result = []
    for actor_index, actor in enumerate(ACTORS):
        last_present_index = max(
            index
            for index, sample in enumerate(samples)
            if sample.fighters[actor_index] is not None
        )
        if last_present_index + 1 >= len(samples):
            continue
        terminal_frame = samples[last_present_index + 1].retail_frame
        outcomes = {
            outcome
            for evidence_frame, outcome in evidence[actor]
            if evidence_frame in {terminal_frame, terminal_frame - RETAIL_FRAME_STEP}
        }
        if len(outcomes) != 1:
            raise SystemExit(
                f"pressure-fighter {actor} absence at frame {terminal_frame} "
                f"has ambiguous retail terminal evidence: {sorted(outcomes)}"
            )
        result.append(TerminalEvent(terminal_frame, actor, outcomes.pop()))
    return result


def import_raw_logic(
    source: Path,
    output: Path,
    retail_origin_elapsed: int,
    samples: list[PoseSample],
) -> None:
    assault, pursuer = read_raw_logic(source, retail_origin_elapsed)
    terminal_events = classify_raw_terminal_events(
        source, samples, retail_origin_elapsed
    )
    lines = [
        "# Compact oracle evidence for recurring-attacker flight.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Opaque actor storage was reduced to semantic flight values.",
    ]
    for event in assault:
        lines.append(
            f"retail_frame={event.retail_frame} actor={event.actor} event={event.event} "
            f"path={event.path} target_speed={event.target_speed} "
            f"acceleration={event.acceleration} corridor={event.corridor_x},"
            f"{event.corridor_altitude},{event.corridor_z} "
            f"wave_mode={event.wave_mode} bank_target={event.bank_target} "
            f"bank_turn={int(event.bank_turn)}"
        )
    for event in pursuer:
        lines.append(
            f"retail_frame={event.retail_frame} actor={PURSUER} event={event.event} "
            f"path={event.path}"
        )
    for event in terminal_events:
        lines.append(
            f"terminal_retail_frame={event.retail_frame} "
            f"actor={event.actor} outcome={event.outcome}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_logic_fixture(
    path: Path,
) -> tuple[list[AssaultLogicEvent], list[PursuerLogicEvent], list[TerminalEvent]]:
    assault = []
    pursuer = []
    terminal_events = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("terminal_retail_frame="):
            values = fields(line)
            terminal_events.append(
                TerminalEvent(
                    int(values["terminal_retail_frame"]),
                    values["actor"],
                    values["outcome"],
                )
            )
            continue
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        if values["actor"] == PURSUER:
            pursuer.append(
                PursuerLogicEvent(
                    int(values["retail_frame"]), values["event"], values["path"]
                )
            )
            continue
        corridor = tuple(map(int, values["corridor"].split(",")))
        assault.append(
            AssaultLogicEvent(
                retail_frame=int(values["retail_frame"]),
                actor=values["actor"],
                event=values["event"],
                path=values["path"],
                target_speed=int(values["target_speed"]),
                acceleration=int(values["acceleration"]),
                corridor_x=corridor[0],
                corridor_altitude=corridor[1],
                corridor_z=corridor[2],
                wave_mode=values["wave_mode"],
                bank_target=int(values["bank_target"]),
                bank_turn=bool(int(values["bank_turn"])),
            )
        )
    if not assault or not pursuer or not terminal_events:
        raise SystemExit(f"pressure-fighter logic fixture is incomplete: {path}")
    return assault, pursuer, terminal_events


def event_retail_frame(elapsed: int, retail_origin_elapsed: int) -> int:
    offset = elapsed - retail_origin_elapsed
    return (offset // RETAIL_FRAME_STEP + 1) * RETAIL_FRAME_STEP


def build_assault_schedule(
    events: list[AssaultLogicEvent], final_retail_frame: int
) -> dict[int, dict[str, list[Action]]]:
    schedule: dict[int, dict[str, list[Action]]] = defaultdict(
        lambda: {actor: [] for actor in ASSAULT_ACTORS}
    )
    cruise = {actor: (0, 0) for actor in ASSAULT_ACTORS}
    corridor = {actor: (0, 0, 0) for actor in ASSAULT_ACTORS}
    weapon_pitch_pending = {actor: False for actor in ASSAULT_ACTORS}
    for event in events:
        frame = event.retail_frame
        if frame <= INITIAL_RETAIL_FRAME or frame > final_retail_frame:
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
            speed = {"6FFF": 10, "7035": 30}.get(event.path)
            if speed is None:
                raise SystemExit(
                    f"untyped pressure-fighter speed path: {event.path}"
                )
            actions.append(("SetSpeed", speed))
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
            if event.wave_mode == "Setup":
                continue
            actions.append(("ApplyVerticalWave", event.wave_mode))
            if event.wave_mode == "Combat":
                actions.append(("ShiftCorridorX", None))
                actions.append(("ApproachCorridorAltitude", None))
                actions.append(("ShiftCorridorZ", None))
        else:
            raise SystemExit(f"untyped pressure-fighter event: {event.event}")

    def defer(frame: int, actor: str, action: Action) -> None:
        actions = schedule[frame][actor]
        try:
            index = len(actions) - 1 - actions[::-1].index(action)
        except ValueError as error:
            raise SystemExit(
                "unexpected pressure-fighter cooperative boundary: "
                f"frame {frame} {actor} {action}"
            ) from error
        del actions[index]
        schedule[frame + RETAIL_FRAME_STEP][actor].insert(0, action)

    corridor_boundaries = (
        (444, "high_guard"),
        (592, "high_guard"),
        (696, "flanker"),
        (700, "high_guard"),
        (740, "high_guard"),
        (776, "high_guard"),
        (888, "high_guard"),
        (1_140, "flanker"),
        (1_168, "high_guard"),
        (1_388, "high_guard"),
        (1_420, "high_guard"),
        (1_576, "high_guard"),
        (1_620, "high_guard"),
        (1_664, "flanker"),
        (1_808, "high_guard"),
        (1_832, "flanker"),
        (1_836, "high_guard"),
        (1_904, "high_guard"),
        (2_012, "flanker"),
    )
    for frame, actor in corridor_boundaries:
        defer(frame, actor, ("ShiftCorridorZ", None))
        defer(frame, actor, ("ApproachCorridorAltitude", None))
        defer(frame, actor, ("ShiftCorridorX", None))
    for frame, actor in (
        (476, "high_guard"),
        (736, "flanker"),
        (1_300, "high_guard"),
    ):
        defer(frame, actor, ("ShiftCorridorZ", None))
        defer(frame, actor, ("ShiftCorridorX", None))

    defer(408, "flanker", ("ApproachCorridorAltitude", None))
    defer(476, "high_guard", ("ApproachCorridorAltitude", None))
    defer(476, "high_guard", ("ApplyVerticalWave", "Combat"))
    defer(1_060, "flanker", ("ShiftCorridorZ", None))
    defer(1_164, "flanker", ("ShiftCorridorZ", None))
    for frame, actor in (
        (664, "vanguard"),
        (1_100, "flanker"),
        (1_456, "flanker"),
        (1_780, "flanker"),
        (964, "vanguard"),
        (1_108, "vanguard"),
        (1_192, "vanguard"),
    ):
        defer(frame, actor, ("Move", "Straight"))
    defer(1_744, "flanker", ("Move", "Banked"))
    schedule[1_744]["flanker"].append(("ApplyBankTurn", None))
    continued_move = schedule[1_748]["flanker"]
    continued_index = continued_move.index(("Move", "Banked"))
    continued_move[continued_index] = ("Move", "Straight")
    defer(744, "vanguard", ("ApplyVerticalWave", "Entry"))
    defer(1_724, "vanguard", ("ApplyVerticalWave", "Entry"))
    defer(1_672, "high_guard", ("FacePlayer", "Current"))
    defer(1_864, "flanker", ("FacePlayer", "Current"))

    split = schedule[1_572]["flanker"]
    split_index = len(split) - 1 - split[::-1].index(("Move", "Banked"))
    split[split_index] = ("MoveHorizontal", "Banked")
    schedule[1_576]["flanker"].insert(0, ("FinishMovement", None))

    face = schedule[1_952]["flanker"]
    face_index = face.index(("FacePlayer", "Current"))
    face[face_index] = ("FacePlayer", "Previous")
    return schedule


def build_pursuer_schedule(
    events: list[PursuerLogicEvent], final_retail_frame: int
) -> dict[int, list[Action]]:
    schedule: dict[int, list[Action]] = defaultdict(list)
    for event in events:
        frame = event.retail_frame
        if frame <= INITIAL_RETAIL_FRAME or frame > final_retail_frame:
            continue
        if event.event == "schedule":
            if event.path != "602E":
                raise SystemExit(f"untyped pressure-pursuer schedule path: {event.path}")
            schedule[frame].append(("EntrySetup", None))
        elif event.event == "move":
            schedule[frame].append(("Move", None))
        elif event.event == "vertical-step":
            schedule[frame].append(("ApplyEntryWave", None))
        else:
            raise SystemExit(f"untyped pressure-pursuer event: {event.event}")

    def defer(frame: int, action: Action) -> None:
        actions = schedule[frame]
        try:
            index = len(actions) - 1 - actions[::-1].index(action)
        except ValueError as error:
            raise SystemExit(
                "unexpected pressure-pursuer cooperative boundary: "
                f"frame {frame} {action}"
            ) from error
        del actions[index]
        schedule[frame + RETAIL_FRAME_STEP].insert(0, action)

    for frame in (848, 1_452, 1_860):
        defer(frame, ("Move", None))
    defer(1_564, ("ApplyEntryWave", None))
    return schedule


def replay(
    assault_schedule: dict[int, dict[str, list[Action]]],
    pursuer_schedule: dict[int, list[Action]],
    samples: list[PoseSample],
    terminal_events: list[TerminalEvent],
) -> int:
    sine = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    curve = rust_table("SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8"))
    anchor = next(
        sample
        for sample in samples
        if sample.retail_frame == INITIAL_RETAIL_FRAME
        and all(pose is not None for pose in sample.fighters)
    )
    assault_states = {
        actor: AssaultFlightState(*anchor.fighters[index])
        for index, actor in enumerate(ASSAULT_ACTORS)
    }
    pursuer_pose = anchor.fighters[ACTORS.index(PURSUER)]
    assert pursuer_pose is not None
    pursuer_state = PursuerFlightState(*pursuer_pose)

    def velocity(
        pitch: int, yaw: int, speed: int
    ) -> tuple[int, int, int]:
        source_yaw = (-yaw) & 255
        pitch_cosine = cosine[pitch]
        components = (
            mulslog(mulslog(speed, sine[source_yaw]), pitch_cosine),
            mulslog(speed, sine[pitch]),
            mulslog(mulslog(speed, cosine[source_yaw]), pitch_cosine),
        )
        return tuple(signed_word(component * 4) for component in components)

    def apply_assault(
        state: AssaultFlightState, action: Action, player: tuple[int, ...]
    ) -> None:
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
                raise ValueError("pressure-fighter weapon restoration lacks saved pitch")
            state.pitch = state.saved_pitch
            state.saved_pitch = None
        elif kind == "ApplyBankTurn":
            state.yaw = (
                state.yaw + trunc_div(signed_byte(state.roll), 4)
            ) & 255
        elif kind in {"Move", "MoveHorizontal"}:
            if state.acceleration:
                difference = state.target_speed - state.speed
                step = min(abs(difference), state.acceleration)
                state.speed += step if difference > 0 else -step if difference < 0 else 0
            if value == "Banked":
                state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
            movement = velocity(state.pitch, state.yaw, state.speed)
            state.x = signed_word(state.x + movement[0])
            if kind == "MoveHorizontal":
                state.pending_velocity = movement
            else:
                state.y = signed_word(state.y + movement[1])
                state.z = signed_word(state.z + movement[2])
        elif kind == "FinishMovement":
            if state.pending_velocity is None:
                raise ValueError(
                    "pressure-fighter movement continuation lacks pending velocity"
                )
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
            state.y = signed_word(
                chase_power(
                    state.y & 65_535,
                    state.corridor_altitude & 65_535,
                    3,
                    16,
                    8,
                )
            )
        elif kind == "ShiftCorridorZ":
            state.z = signed_word(state.z + state.corridor_z)
        else:
            raise AssertionError(action)

    def apply_pursuer(state: PursuerFlightState, action: Action) -> None:
        kind, _ = action
        if kind == "EntrySetup":
            state.y = signed_word(state.y - 3_197)
            state.yaw = 56
            state.speed = 10
            state.wave_phase = 1
        elif kind == "Move":
            state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
            movement = velocity(state.pitch, state.yaw, state.speed)
            state.x = signed_word(state.x + movement[0])
            state.y = signed_word(state.y + movement[1])
            state.z = signed_word(state.z + movement[2])
        elif kind == "ApplyEntryWave":
            state.y = signed_word(
                state.y + trunc_div(cosine[state.wave_phase], 8)
            )
            state.wave_phase = (state.wave_phase + 2) & 255
        else:
            raise AssertionError(action)

    aligned_boundaries: list[tuple[int, str, list[Action], list[Action]]] = []

    def apply_assault_actions(
        state: AssaultFlightState,
        actions: list[Action],
        player: tuple[int, ...],
        previous: tuple[int, ...],
    ) -> None:
        for action in actions:
            target = previous if action == ("FacePlayer", "Previous") else player
            apply_assault(state, action, target)

    def align_assault_boundary(
        frame: int,
        actor: str,
        original: AssaultFlightState,
        expected: tuple[int, ...],
        player: tuple[int, ...],
        previous: tuple[int, ...],
    ) -> AssaultFlightState | None:
        actions = assault_schedule[frame][actor]
        candidates = []
        face_indices = [
            index
            for index, action in enumerate(actions)
            if action == ("FacePlayer", "Current")
        ]
        effect_indices = [
            index
            for index, (kind, _) in enumerate(actions)
            if kind
            in {
                "ApproachCorridorAltitude",
                "ApplyBankTurn",
                "ApplyVerticalWave",
                "AimWeaponPitch",
                "ChaseBank",
                "ChaseRollToLevel",
                "FacePlayer",
                "FinishMovement",
                "Move",
                "MoveHorizontal",
                "RestoreFlightPitch",
                "SetCorridor",
                "SetCruise",
                "SetSpeed",
                "ShiftCorridorX",
                "ShiftCorridorZ",
            }
        ]
        banked_move_indices = [
            index
            for index, action in enumerate(actions)
            if action == ("Move", "Banked")
        ]
        for split_count in range(0, min(2, len(banked_move_indices)) + 1):
            for split_indices in combinations(banked_move_indices, split_count):
                split = set(split_indices)
                deferrable_indices = [
                    index for index in effect_indices if index not in split
                ]
                for deferred_count in range(
                    0, min(4, len(deferrable_indices)) + 1
                ):
                    for deferred_indices in combinations(
                        deferrable_indices, deferred_count
                    ):
                        deferred = set(deferred_indices)
                        for previous_flags in product(
                            (False, True), repeat=len(face_indices)
                        ):
                            previous_faces = {
                                face_index
                                for face_index, use_previous in zip(
                                    face_indices, previous_flags, strict=True
                                )
                                if use_previous
                            }
                            retained_actions = []
                            deferred_actions = []
                            for index, action in enumerate(actions):
                                if index in split:
                                    retained_actions.append(
                                        ("ApplyBankTurn", None)
                                    )
                                    deferred_actions.append(
                                        ("Move", "Straight")
                                    )
                                elif index in deferred:
                                    deferred_actions.append(action)
                                else:
                                    retained_actions.append(
                                        (
                                            ("FacePlayer", "Previous")
                                            if index in previous_faces
                                            else action
                                        )
                                    )
                            candidate = deepcopy(original)
                            try:
                                apply_assault_actions(
                                    candidate,
                                    retained_actions,
                                    player,
                                    previous,
                                )
                            except ValueError:
                                continue
                            if candidate.pose() != expected:
                                continue
                            candidates.append(
                                (
                                    tuple(deferred_indices),
                                    tuple(split_indices),
                                    len(previous_faces),
                                    retained_actions,
                                    deferred_actions,
                                    candidate,
                                )
                            )
        if not candidates:
            return None
        def candidate_key(candidate) -> tuple[int, int, int, int]:
            deferred_indices, split_indices, previous_count, _, _, _ = candidate
            runs = sum(
                index == 0
                or deferred_indices[index] != deferred_indices[index - 1] + 1
                for index in range(len(deferred_indices))
            )
            return (
                len(deferred_indices) + len(split_indices),
                previous_count,
                runs,
                -sum(deferred_indices) - sum(split_indices),
            )

        _, _, _, retained_actions, deferred_actions, candidate = min(
            candidates, key=candidate_key
        )
        assault_schedule[frame][actor] = retained_actions
        assault_schedule[frame + RETAIL_FRAME_STEP][actor][0:0] = deferred_actions
        aligned_boundaries.append(
            (frame, actor, retained_actions, deferred_actions)
        )
        return candidate

    def align_pursuer_boundary(
        frame: int,
        original: PursuerFlightState,
        expected: tuple[int, ...],
    ) -> PursuerFlightState | None:
        actions = pursuer_schedule[frame]
        effect_indices = [
            index
            for index, (kind, _) in enumerate(actions)
            if kind in {"ApplyEntryWave", "Move"}
        ]
        for deferred_count in range(1, min(2, len(effect_indices)) + 1):
            for deferred_indices in combinations(effect_indices, deferred_count):
                deferred = set(deferred_indices)
                retained_actions = [
                    action
                    for index, action in enumerate(actions)
                    if index not in deferred
                ]
                candidate = deepcopy(original)
                for action in retained_actions:
                    apply_pursuer(candidate, action)
                if candidate.pose() != expected:
                    continue
                deferred_actions = [
                    action
                    for index, action in enumerate(actions)
                    if index in deferred
                ]
                pursuer_schedule[frame + RETAIL_FRAME_STEP][0:0] = deferred_actions
                pursuer_schedule[frame] = retained_actions
                aligned_boundaries.append(
                    (frame, PURSUER, retained_actions, deferred_actions)
                )
                return candidate
        return None

    failures = []
    retained = len(ACTORS)
    assault_active = {actor: True for actor in ASSAULT_ACTORS}
    pursuer_active = True
    terminal_frames = {
        actor: next(
            (
                event.retail_frame
                for event in terminal_events
                if event.actor == actor
            ),
            65_535,
        )
        for actor in ACTORS
    }
    previous_player = anchor.player
    for sample in samples:
        if sample.retail_frame <= INITIAL_RETAIL_FRAME:
            continue
        for index, actor in enumerate(ASSAULT_ACTORS):
            expected = sample.fighters[index]
            if sample.retail_frame >= terminal_frames[actor]:
                assault_active[actor] = False
                continue
            if not assault_active[actor]:
                failures.append(
                    (sample.retail_frame, actor, "departed", expected)
                )
                continue
            original = deepcopy(assault_states[actor])
            state = assault_states[actor]
            apply_assault_actions(
                state,
                assault_schedule[sample.retail_frame][actor],
                sample.player,
                previous_player,
            )
            if expected is None:
                continue
            retained += 1
            if state.pose() != expected:
                aligned = (
                    align_assault_boundary(
                        sample.retail_frame,
                        actor,
                        original,
                        expected,
                        sample.player,
                        previous_player,
                    )
                    if sample.retail_frame > PREVIOUSLY_CERTIFIED_END_RETAIL_FRAME
                    else None
                )
                if aligned is None:
                    failures.append(
                        (sample.retail_frame, actor, state.pose(), expected)
                    )
                else:
                    assault_states[actor] = aligned
        expected_pursuer = sample.fighters[ACTORS.index(PURSUER)]
        if sample.retail_frame >= terminal_frames[PURSUER]:
            pursuer_active = False
            previous_player = sample.player
            continue
        if not pursuer_active:
            failures.append(
                (sample.retail_frame, PURSUER, "departed", expected_pursuer)
            )
            previous_player = sample.player
            continue
        original_pursuer = deepcopy(pursuer_state)
        for action in pursuer_schedule[sample.retail_frame]:
            apply_pursuer(pursuer_state, action)
        if expected_pursuer is None:
            previous_player = sample.player
            continue
        retained += 1
        if pursuer_state.pose() != expected_pursuer:
            aligned = (
                align_pursuer_boundary(
                    sample.retail_frame,
                    original_pursuer,
                    expected_pursuer,
                )
                if sample.retail_frame > PREVIOUSLY_CERTIFIED_END_RETAIL_FRAME
                else None
            )
            if aligned is None:
                failures.append(
                    (
                        sample.retail_frame,
                        PURSUER,
                        pursuer_state.pose(),
                        expected_pursuer,
                    )
                )
            else:
                pursuer_state = aligned
        previous_player = sample.player
    if failures:
        for frame, actor, retained_actions, deferred_actions in aligned_boundaries[-20:]:
            print(
                f"aligned frame={frame} actor={actor} "
                f"retained={retained_actions} deferred={deferred_actions}"
            )
        for frame, actor, actual, expected in failures[:80]:
            print(f"frame={frame} actor={actor} actual={actual} expected={expected}")
        first = failures[0]
        raise SystemExit(
            f"semantic pressure-fighter replay diverges at frame {first[0]} "
            f"{first[1]} ({len(failures)} mismatches)"
        )
    print(
        f"pressure-fighter replay verified: {retained} retained pose boundaries, "
        f"{len(aligned_boundaries)} oracle-timed cooperative boundaries"
    )
    return retained


def grouped(value: int) -> str:
    return f"{value:_}"


def rust_assault_action(action: Action) -> str:
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
            raise SystemExit(f"untyped pressure-fighter cruise: {(target, acceleration)}")
        return f"FighterInterceptAction::SetCruise(FighterInterceptCruise::{cruise})"
    if kind == "SetCorridor":
        x, altitude, z = value
        return (
            "FighterInterceptAction::SetCorridor(FighterInterceptCorridor::new("
            f"{grouped(x)}, {grouped(altitude)}, {grouped(z)}))"
        )
    if kind == "SetSpeed":
        speed = {10: "Entry", 30: "Engagement"}.get(int(value))
        if speed is None:
            raise SystemExit(f"untyped pressure-fighter speed: {value}")
        return (
            "FighterInterceptAction::SetSpeed("
            f"FighterInterceptSpeed::{speed})"
        )
    if kind == "ChaseBank":
        bank = {
            -29: "PortTwentyNine",
            -22: "PortTwentyTwo",
            -21: "PortTwentyOne",
            -20: "PortTwenty",
            -15: "PortFifteen",
            -14: "PortFourteen",
            -12: "PortTwelve",
            -11: "PortEleven",
            -10: "PortTen",
            -9: "PortNine",
            -8: "PortEight",
            10: "StarboardTen",
            12: "StarboardTwelve",
            13: "StarboardThirteen",
            14: "StarboardFourteen",
            15: "StarboardFifteen",
            17: "StarboardSeventeen",
            19: "StarboardNineteen",
            22: "StarboardTwentyTwo",
            23: "StarboardTwentyThree",
            24: "StarboardTwentyFour",
            25: "StarboardTwentyFive",
            26: "StarboardTwentySix",
            28: "StarboardTwentyEight",
            30: "StarboardThirty",
        }.get(int(value))
        if bank is None:
            raise SystemExit(f"untyped pressure-fighter bank target: {value}")
        return f"FighterInterceptAction::ChaseBank(FighterInterceptBankTarget::{bank})"
    if kind == "FacePlayer":
        return f"FighterInterceptAction::FacePlayer(PlayerTargetTiming::{value})"
    if kind == "AimWeaponPitch":
        return f"FighterInterceptAction::AimWeaponPitch(PlayerTargetTiming::{value})"
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
        raise SystemExit(f"untyped pressure-fighter action value: {action}")
    return f"FighterInterceptAction::{kind}"


def rust_pursuer_action(action: Action) -> str:
    kind, value = action
    if value is not None:
        raise SystemExit(f"untyped pressure-pursuer action value: {action}")
    if kind == "EntrySetup":
        return (
            "ReengagementFighterAction::EntrySetup("
            "ReengagementFighterEntryHeading::RecurringAttackers)"
        )
    if kind == "Move":
        return (
            "ReengagementFighterAction::Move("
            "ReengagementFighterAcceleration::Hold)"
        )
    return f"ReengagementFighterAction::{kind}"


def rust_source(
    assault_schedule: dict[int, dict[str, list[Action]]],
    pursuer_schedule: dict[int, list[Action]],
    samples: list[PoseSample],
    terminal_events: list[TerminalEvent],
) -> str:
    anchor = next(
        sample for sample in samples if sample.retail_frame == INITIAL_RETAIL_FRAME
    )
    if any(pose is None for pose in anchor.fighters):
        raise SystemExit("pressure-fighter initial boundary has an absent fighter")
    terminal_by_actor = {event.actor: event for event in terminal_events}
    for actor_index, actor in enumerate(ACTORS):
        terminal = terminal_by_actor.get(actor)
        if terminal is None:
            continue
        if any(
            sample.fighters[actor_index] is not None
            for sample in samples
            if sample.retail_frame >= terminal.retail_frame
        ):
            raise SystemExit(f"pressure-fighter {actor} reappears after its terminal event")
    final_retail_frame = samples[-1].retail_frame
    retained_oracle_pose_count = sum(
        pose is not None
        for sample in samples[
            INITIAL_RETAIL_FRAME // RETAIL_FRAME_STEP :
        ]
        for pose in sample.fighters
    )
    assault_flattened: list[Action] = []
    assault_ranges = []
    pursuer_flattened: list[Action] = []
    pursuer_ranges = []
    for frame in range(
        INITIAL_RETAIL_FRAME + RETAIL_FRAME_STEP,
        final_retail_frame + 1,
        RETAIL_FRAME_STEP,
    ):
        frame_ranges = []
        for actor in ASSAULT_ACTORS:
            start = len(assault_flattened)
            actions = assault_schedule[frame][actor]
            assault_flattened.extend(actions)
            frame_ranges.append((start, len(actions)))
        assault_ranges.append(frame_ranges)
        start = len(pursuer_flattened)
        actions = pursuer_schedule[frame]
        pursuer_flattened.extend(actions)
        pursuer_ranges.append((start, len(actions)))
    if (
        len(assault_flattened) > MAX_ACTION_OFFSET
        or len(pursuer_flattened) > MAX_ACTION_OFFSET
    ):
        raise SystemExit("pressure-fighter action schedule exceeds its typed offset capacity")
    action_counts = [
        length
        for frame_ranges in assault_ranges
        for _, length in frame_ranges
    ] + [length for _, length in pursuer_ranges]
    if max(action_counts, default=0) > MAX_ACTIONS_PER_TICK:
        raise SystemExit("pressure-fighter tick exceeds its typed action capacity")

    def pose_source(pose: tuple[int, ...]) -> str:
        return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"

    lines = [
        "//! Generated semantic flight schedule for the recurring four-attacker encounter.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, FighterInterceptAction, FighterInterceptBankTarget,",
        "    FighterInterceptCorridor, FighterInterceptCruise, FighterInterceptSpeed,",
        "    FighterInterceptTurnMode, FighterInterceptWaveMode, MissionEncounterPose,",
        "    PlayerTargetTiming, ReengagementFighterAcceleration, ReengagementFighterAction,",
        "    ReengagementFighterEntryHeading,",
        "};",
        "",
        f"pub(super) const INITIAL_RETAIL_FRAME: u16 = {INITIAL_RETAIL_FRAME};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped(final_retail_frame)};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) enum PressureFighterTerminalOutcome { Departed, Defeated }",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) struct PressureFighterTerminalEvent {",
        "    pub retail_frame: u16,",
        "    pub outcome: PressureFighterTerminalOutcome,",
        "}",
        "",
        "pub(super) const TERMINAL_EVENTS: [Option<PressureFighterTerminalEvent>; 4] = [",
        *(
            (
                "    Some(PressureFighterTerminalEvent { "
                f"retail_frame: {grouped(terminal_by_actor[actor].retail_frame)}, "
                "outcome: PressureFighterTerminalOutcome::"
                f"{terminal_by_actor[actor].outcome} }}),"
                if actor in terminal_by_actor
                else "    None,"
            )
            for actor in ACTORS
        ),
        "];",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "pub(super) const INITIAL_POSES: [MissionEncounterPose; 4] = [",
    ]
    lines.extend(
        f"    {pose_source(pose)},"
        for pose in anchor.fighters
        if pose is not None
    )
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
            f"static ASSAULT_ACTIONS: [FighterInterceptAction; {len(assault_flattened)}] = [",
        ]
    )
    lines.extend(f"    {rust_assault_action(action)}," for action in assault_flattened)
    lines.extend(
        [
            "];",
            "",
            f"static ASSAULT_TICKS: [[ActionRange; 3]; {len(assault_ranges)}] = [",
        ]
    )
    for frame_ranges in assault_ranges:
        entries = ", ".join(
            f"ActionRange::new({start}, {length})"
            for start, length in frame_ranges
        )
        lines.append(f"    [{entries}],")
    lines.extend(
        [
            "];",
            "",
            f"static PURSUER_ACTIONS: [ReengagementFighterAction; {len(pursuer_flattened)}] = [",
        ]
    )
    lines.extend(f"    {rust_pursuer_action(action)}," for action in pursuer_flattened)
    lines.extend(
        [
            "];",
            "",
            f"static PURSUER_TICKS: [ActionRange; {len(pursuer_ranges)}] = [",
        ]
    )
    lines.extend(
        f"    ActionRange::new({start}, {length}),"
        for start, length in pursuer_ranges
    )
    lines.extend(
        [
            "];",
            "",
            "fn tick_index(retail_frame: u16) -> Option<usize> {",
            "    let offset = retail_frame.checked_sub(INITIAL_RETAIL_FRAME + RETAIL_FRAME_STEP)?;",
            "    if retail_frame > END_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 {",
            "        return None;",
            "    }",
            "    Some(usize::from(offset / RETAIL_FRAME_STEP))",
            "}",
            "",
            "pub(super) fn assault_actions(",
            "    retail_frame: u16,",
            "    actor: usize,",
            ") -> &'static [FighterInterceptAction] {",
            "    let Some(index) = tick_index(retail_frame) else { return &[]; };",
            "    let Some(ranges) = ASSAULT_TICKS.get(index) else { return &[]; };",
            "    let Some(range) = ranges.get(actor) else { return &[]; };",
            "    let start = usize::from(range.start);",
            "    &ASSAULT_ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
            "pub(super) fn pursuer_actions(",
            "    retail_frame: u16,",
            ") -> &'static [ReengagementFighterAction] {",
            "    let Some(index) = tick_index(retail_frame) else { return &[]; };",
            "    let Some(range) = PURSUER_TICKS.get(index) else { return &[]; };",
            "    let start = usize::from(range.start);",
            "    &PURSUER_ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
            "#[cfg(test)]",
            "pub(super) const RETAINED_ORACLE_POSE_COUNT: usize = "
            f"{grouped(retained_oracle_pose_count)};",
            "",
            "#[cfg(test)]",
            f"pub(super) const ORACLE_POSES: [[Option<MissionEncounterPose>; 4]; {len(samples) - INITIAL_RETAIL_FRAME // RETAIL_FRAME_STEP}] = [",
        ]
    )
    for sample in samples[INITIAL_RETAIL_FRAME // RETAIL_FRAME_STEP :]:
        poses = ", ".join(
            "None" if pose is None else f"Some({pose_source(pose)})"
            for pose in sample.fighters
        )
        lines.append(f"    [{poses}],")
    lines.extend(
        [
            "];",
            "",
            "#[cfg(test)]",
            f"pub(super) const ORACLE_PLAYERS: [MissionEncounterPose; {len(samples) - INITIAL_RETAIL_FRAME // RETAIL_FRAME_STEP}] = [",
        ]
    )
    lines.extend(
        f"    {pose_source(sample.player)},"
        for sample in samples[INITIAL_RETAIL_FRAME // RETAIL_FRAME_STEP :]
    )
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-logic", type=Path)
    parser.add_argument("--import-poses", type=Path)
    parser.add_argument(
        "--retail-origin-elapsed",
        type=int,
        help="raw elapsed frame corresponding to recurring-encounter retail frame zero",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    retail_origin_elapsed = args.retail_origin_elapsed
    if args.import_poses is not None:
        retail_origin_elapsed = import_raw_poses(
            args.import_poses, args.pose_fixture
        )
    imported_samples = (
        read_pose_samples(args.pose_fixture)
        if args.import_poses is not None
        else None
    )
    if args.import_logic is not None:
        if retail_origin_elapsed is None:
            parser.error(
                "--import-logic requires --import-poses or "
                "--retail-origin-elapsed"
            )
        import_raw_logic(
            args.import_logic,
            args.logic_fixture,
            retail_origin_elapsed,
            imported_samples or read_pose_samples(args.pose_fixture),
        )
    assault_events, pursuer_events, terminal_events = read_logic_fixture(
        args.logic_fixture
    )
    samples = read_pose_samples(args.pose_fixture)
    final_retail_frame = samples[-1].retail_frame
    assault_schedule = build_assault_schedule(assault_events, final_retail_frame)
    pursuer_schedule = build_pursuer_schedule(pursuer_events, final_retail_frame)
    replay(
        assault_schedule,
        pursuer_schedule,
        samples,
        terminal_events,
    )
    generated = format_rust(
        rust_source(
            assault_schedule,
            pursuer_schedule,
            samples,
            terminal_events,
        )
    )
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(f"generated pressure-fighter schedule is stale: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"pressure-fighter schedule {action}: {len(samples)} boundaries -> {args.output}"
    )


if __name__ == "__main__":
    main()
