#!/usr/bin/env python3
"""Generate the typed capital-craft schedule for SF2's first re-engagement.

The raw import consumes Mesen operation callbacks, then discards source
addresses and opaque object storage.  The retained fixture contains only
gameplay operations and the semantic values needed to replay them.  Generation
is accepted only after an independent flat-state replay matches every
four-frame oracle pose for both capital craft.
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
    Path(__file__).with_name("fixtures") / "second_sortie_capital_logic.trace"
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
    / "second_sortie_capital.rs"
)

FIRST_ACTOR = "first"
SECOND_ACTOR = "second"
SOURCE_ACTORS = {"0633": FIRST_ACTOR, "05F4": SECOND_ACTOR}
SOURCE_SHAPE = "F5EC"
RAW_SAMPLE_START_ELAPSED = 14_912
ANCHOR_RETAIL_FRAME = 68
RETAIL_FRAME_STEP = 4

RELEVANT_EVENTS = {
    "random-branch",
    "wait-for-angle",
    "chase-angle",
    "face-player",
    "move",
    "vertical-step",
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


@dataclass(frozen=True)
class LogicEvent:
    elapsed: int
    actor: str
    event: str
    path: str
    pose: tuple[int, ...]
    banked: bool
    bank_target: int
    wave_phase: int
    wave_sample: int
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


Action = tuple[str, int | str | None]


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def import_raw_logic(source: Path, output: Path) -> None:
    retained: list[LogicEvent] = []
    for line in source.read_text(encoding="utf-8").splitlines():
        parsed = fields(line)
        actor = SOURCE_ACTORS.get(parsed.get("object", ""))
        event = parsed.get("event", "")
        if (
            actor is None
            or parsed.get("shape") != SOURCE_SHAPE
            or event not in RELEVANT_EVENTS
            or "elapsed" not in parsed
        ):
            continue
        path = parsed.get("path", "")
        if event == "random-branch" and path not in MANEUVER_BRANCH_PATHS:
            continue
        base = bytes.fromhex(parsed.get("base", ""))
        extension = bytes.fromhex(parsed.get("extension", ""))
        retained.append(
            LogicEvent(
                elapsed=int(parsed["elapsed"]),
                actor=actor,
                event=event,
                path=path,
                pose=tuple(map(int, parsed["pose"].split(","))),
                banked=len(base) > 33 and bool(base[33] & 64),
                bank_target=(
                    signed_byte(extension[41]) if len(extension) > 41 else 0
                ),
                wave_phase=extension[34] if len(extension) > 34 else 0,
                wave_sample=(
                    int.from_bytes(extension[33:34], "little", signed=True)
                    if len(extension) > 33
                    else 0
                ),
                selected=tuple(map(int, parsed["selected_pose"].split(","))),
            )
        )

    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    lines = [
        "# Compact oracle evidence for the first re-engagement capital craft.",
        f"# Raw source SHA-256: {digest}",
        "# Opaque object bytes were reduced to semantic flight fields.",
    ]
    for event in retained:
        lines.append(
            f"elapsed={event.elapsed} actor={event.actor} event={event.event} "
            f"path={event.path} pose={','.join(map(str, event.pose))} "
            f"banked={int(event.banked)} bank_target={event.bank_target} "
            f"wave_phase={event.wave_phase} wave_sample={event.wave_sample} "
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
                wave_phase=int(parsed["wave_phase"]),
                wave_sample=int(parsed["wave_sample"]),
                selected=tuple(map(int, parsed["selected"].split(","))),
            )
        )
    if not result:
        raise SystemExit(f"capital logic fixture is empty: {path}")
    return result


def parse_target(value: str) -> tuple[int, ...] | None:
    return None if value == "-" else tuple(map(int, value.split(",")))


def read_pose_fixture(path: Path) -> list[PoseSample]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        parsed = fields(line)
        retail_frame = int(parsed["retail_frame"])
        if retail_frame < ANCHOR_RETAIL_FRAME:
            continue
        targets = parsed["targets"].split("/")
        result.append(
            PoseSample(
                retail_frame=retail_frame,
                player=tuple(map(int, parsed["player"].split(","))),
                first=parse_target(targets[0]),
                second=parse_target(targets[1]),
            )
        )
    if not result or result[0].retail_frame != ANCHOR_RETAIL_FRAME:
        raise SystemExit("second-sortie capital pose fixture lacks its anchor")
    return result


def event_retail_frame(elapsed: int) -> int:
    # Logic callbacks occur after the pose recorder for the identically
    # numbered emulator frame, so work belongs to the next retained boundary.
    offset = elapsed - (RAW_SAMPLE_START_ELAPSED - 1)
    return ((offset + RETAIL_FRAME_STEP - 1) // RETAIL_FRAME_STEP) * RETAIL_FRAME_STEP


def wave_action(event: LogicEvent, cosine: list[int]) -> Action:
    value = cosine[event.wave_phase]
    divisions = 0
    while value != event.wave_sample and divisions < 8:
        value = trunc_div(value, 2)
        divisions += 1
    if value != event.wave_sample:
        raise SystemExit(
            f"untyped capital wave sample {event.wave_sample} at phase {event.wave_phase}"
        )
    # The phase advance is filled by build_schedule after looking at the next
    # wave callback for this actor. At the cosine zero crossings, divided and
    # undivided samples are intentionally indistinguishable.
    return "ApplyVerticalWave", divisions


def target_timing(event: LogicEvent, samples: dict[int, PoseSample]) -> str:
    frame = event_retail_frame(event.elapsed)
    current = samples.get(frame)
    previous = samples.get(frame - RETAIL_FRAME_STEP)
    if current is None or previous is None:
        return "Current"
    target = event.selected[:3]
    candidates = (
        ("Current", current.player[:3]),
        (
            "Midpoint",
            tuple(
                start + trunc_div(end - start, 2)
                for start, end in zip(previous.player[:3], current.player[:3])
            ),
        ),
        ("Previous", previous.player[:3]),
    )
    for timing, position in candidates:
        if position == target:
            return timing
    raise SystemExit(
        f"untyped player-target timing at elapsed {event.elapsed}: {target}"
    )


def build_schedule(
    events: list[LogicEvent], cosine: list[int], pose_samples: list[PoseSample]
) -> dict[int, dict[str, list[Action]]]:
    schedule: dict[int, dict[str, list[Action]]] = defaultdict(
        lambda: {FIRST_ACTOR: [], SECOND_ACTOR: []}
    )
    per_actor = {
        actor: [event for event in events if event.actor == actor]
        for actor in (FIRST_ACTOR, SECOND_ACTOR)
    }
    maneuver_direction = {}
    wave_advances = {}
    movement_speeds = {}
    observed_speed = {FIRST_ACTOR: 0, SECOND_ACTOR: 0}
    samples = {sample.retail_frame: sample for sample in pose_samples}
    for actor, actor_events in per_actor.items():
        for index, event in enumerate(actor_events):
            if event.event == "random-branch":
                following = next(
                    candidate
                    for candidate in actor_events[index + 1 :]
                    if candidate.event == "wait-for-angle"
                    and candidate.path in {"7185", "71C1"}
                )
                maneuver_direction[(actor, event.elapsed)] = (
                    "Port" if signed_byte(following.pose[5]) < 0 else "Starboard"
                )
            if event.event == "move":
                following_wave = next(
                    (
                        candidate
                        for candidate in actor_events[index + 1 :]
                        if candidate.event in {"move", "vertical-step"}
                    ),
                    None,
                )
                movement_speeds[(actor, event.elapsed)] = (
                    following_wave.pose[6]
                    if following_wave is not None
                    and following_wave.event == "vertical-step"
                    else event.pose[6]
                )
            if event.event == "vertical-step":
                following = next(
                    (
                        candidate
                        for candidate in actor_events[index + 1 :]
                        if candidate.event == "vertical-step"
                    ),
                    None,
                )
                advance = (
                    (following.wave_phase - event.wave_phase) & 255
                    if following is not None
                    else 1
                )
                wave_advances[(actor, event.elapsed)] = advance

    for event in events:
        frame = event_retail_frame(event.elapsed)
        actions = schedule[frame][event.actor]
        if event.event == "random-branch":
            actions.append(
                ("BeginPitchManeuver", maneuver_direction[(event.actor, event.elapsed)])
            )
        elif event.event == "wait-for-angle":
            field, target = PITCH_PATHS[event.path]
            actions.append(
                ("ChaseRollToLevel", None)
                if field == "roll"
                else ("ChasePitch", target)
            )
        elif event.event == "chase-angle":
            actions.append(("ChaseBank", event.bank_target))
        elif event.event == "face-player":
            actions.append(("FacePlayer", target_timing(event, samples)))
        elif event.event == "move":
            speed = movement_speeds[(event.actor, event.elapsed)]
            if speed != observed_speed[event.actor]:
                actions.append(("SetSpeed", speed))
                observed_speed[event.actor] = speed
            if event.path == "70D3":
                actions.append(("ChaseRollToLevel", None))
            if event.path in {"714A", "719C", "71D8"}:
                actions.extend(
                    (("CenterAltitude", None), ("ChaseRollToLevel", None))
                )
            actions.append(("Move", "Banked" if event.banked else "Straight"))
        elif event.event == "vertical-step":
            _, divisions = wave_action(event, cosine)
            advance = wave_advances[(event.actor, event.elapsed)]
            mode = (
                "Entry"
                if advance == 4 and int(divisions) in {0, 3}
                else "Combat"
                if advance == 1 and int(divisions) == 0
                else None
            )
            if mode is None:
                raise SystemExit(
                    f"untyped capital wave mode divisions={divisions} advance={advance}"
                )
            actions.append(("ApplyVerticalWave", mode))

    # One retained pose lands between the second craft's horizontal movement
    # and its vertical-wave task. Keep the work as adjacent typed operations,
    # but place each operation on the boundary where retail exposes it.
    early_second = schedule[260][SECOND_ACTOR]
    early_second.remove(("Move", "Straight"))
    schedule[264][SECOND_ACTOR].insert(0, ("Move", "Straight"))

    first_712 = schedule[712][FIRST_ACTOR]
    if first_712[-2:] != [("ChaseRollToLevel", None), ("Move", "Straight")]:
        raise SystemExit("unexpected first-capital cooperative boundary at frame 712")
    del first_712[-2:]
    schedule[716][FIRST_ACTOR][0:0] = [
        ("ChaseRollToLevel", None),
        ("Move", "Straight"),
    ]

    first_920 = schedule[920][FIRST_ACTOR]
    if first_920[-1] != ("ApplyVerticalWave", "Combat"):
        raise SystemExit("unexpected first-capital wave boundary at frame 920")
    first_920.pop()
    schedule[924][FIRST_ACTOR].insert(0, ("ApplyVerticalWave", "Combat"))

    second_1424 = schedule[1_424][SECOND_ACTOR]
    second_1424.remove(("Move", "Banked"))
    schedule[1_428][SECOND_ACTOR].insert(0, ("Move", "Banked"))

    second_1932 = schedule[1_932][SECOND_ACTOR]
    if second_1932[-2:] != [("ChasePitch", "Level"), ("Move", "Straight")]:
        raise SystemExit("unexpected second-capital cooperative boundary at frame 1932")
    del second_1932[-2:]
    second_1932.append(("ChasePitch", "Level"))
    schedule[1_936][SECOND_ACTOR].insert(0, ("Move", "Straight"))

    second_2104 = schedule[2_104][SECOND_ACTOR]
    second_2104.remove(("ApplyVerticalWave", "Combat"))
    schedule[2_108][SECOND_ACTOR].insert(0, ("ApplyVerticalWave", "Combat"))

    first_2156 = schedule[2_156][FIRST_ACTOR]
    if first_2156[-1] != ("ApplyVerticalWave", "Combat"):
        raise SystemExit("unexpected first-capital wave boundary at frame 2156")
    first_2156.pop()
    schedule[2_160][FIRST_ACTOR].insert(0, ("ApplyVerticalWave", "Combat"))

    # Retail exposes weapon aim after saving the flight heading, then restores
    # that heading before the following movement slice.
    fire_3316 = next(
        event
        for event in events
        if event.actor == FIRST_ACTOR
        and event.event == "fire"
        and event_retail_frame(event.elapsed) == 3_316
    )
    schedule[3_316][FIRST_ACTOR].append(
        ("AimWeapon", target_timing(fire_3316, samples))
    )
    schedule[3_320][FIRST_ACTOR].insert(0, ("RestoreFlightAngles", None))

    first_3668 = schedule[3_668][FIRST_ACTOR]
    if first_3668[-1] != ("Move", "Banked"):
        raise SystemExit("unexpected first-capital turn boundary at frame 3668")
    first_3668[-1] = ("ApplyBankTurn", None)
    schedule[3_672][FIRST_ACTOR].insert(0, ("Move", "Straight"))

    first_3980 = schedule[3_980][FIRST_ACTOR]
    if first_3980[-1] != ("ApplyVerticalWave", "Combat"):
        raise SystemExit("unexpected first-capital wave boundary at frame 3980")
    first_3980.pop()
    schedule[3_984][FIRST_ACTOR].insert(0, ("ApplyVerticalWave", "Combat"))

    fire_4976 = next(
        event
        for event in events
        if event.actor == FIRST_ACTOR
        and event.event == "fire"
        and event_retail_frame(event.elapsed) == 4_976
    )
    schedule[4_976][FIRST_ACTOR].append(
        ("AimWeapon", target_timing(fire_4976, samples))
    )
    schedule[4_980][FIRST_ACTOR].insert(0, ("RestoreFlightAngles", None))

    first_5884 = schedule[5_884][FIRST_ACTOR]
    if first_5884[-1] != ("ApplyVerticalWave", "Combat"):
        raise SystemExit("unexpected first-capital wave boundary at frame 5884")
    first_5884.pop()
    schedule[5_888][FIRST_ACTOR].insert(0, ("ApplyVerticalWave", "Combat"))
    return schedule


def replay(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> None:
    sine = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    curve = rust_table(
        "SF2_ARCTANGENT_CURVE", ANGLE_SOURCE.read_text(encoding="utf-8")
    )
    anchor = samples[0]
    assert anchor.first is not None and anchor.second is not None
    states = {
        FIRST_ACTOR: FlightState(*anchor.first, wave_phase=0),
        SECOND_ACTOR: FlightState(*anchor.second, wave_phase=0),
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

    def apply(
        state: FlightState,
        action: Action,
        player: tuple[int, ...],
        previous_player: tuple[int, ...],
    ) -> None:
        kind, value = action
        target_position = {
            "Current": player,
            "Midpoint": tuple(
                start + trunc_div(end - start, 2)
                for start, end in zip(previous_player, player)
            ),
            "Previous": previous_player,
        }.get(str(value), player)
        if kind == "BeginPitchManeuver":
            state.roll = 4 if value == "Starboard" else 252
        elif kind == "ChasePitch":
            state.pitch = chase_power(
                state.pitch, {"Level": 0, "Dive": 206, "Climb": 50}[str(value)], 3, 8, 8
            )
        elif kind == "ChaseRollToLevel":
            state.roll = chase_power(state.roll, 0, 3, 8, 8)
        elif kind == "ChaseBank":
            state.roll = chase_power(state.roll, int(value) & 255, 3, 8, 8)
        elif kind == "FacePlayer":
            dx = signed_word(target_position[0] - state.x)
            dz = signed_word(target_position[2] - state.z)
            target = (-(sf2_atan16(dx, dz, curve) >> 8)) & 255
            state.yaw = chase_power(state.yaw, target, 2, 8, 4)
        elif kind == "CenterAltitude":
            state.y = signed_word(chase_power(state.y & 65_535, 0, 3, 16, 8))
        elif kind == "SetSpeed":
            state.speed = int(value)
        elif kind == "ApplyBankTurn":
            state.yaw = (
                state.yaw + trunc_div(signed_byte(state.roll), 4)
            ) & 255
        elif kind == "Move":
            if value == "Banked":
                state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
            movement = velocity(state)
            state.x = signed_word(state.x + movement[0])
            state.y = signed_word(state.y + movement[1])
            state.z = signed_word(state.z + movement[2])
        elif kind == "ApplyVerticalWave":
            divisions, advance = {"Entry": (3, 4), "Combat": (0, 1)}[str(value)]
            displacement = cosine[state.wave_phase]
            for _ in range(divisions):
                displacement = trunc_div(displacement, 2)
            state.y = signed_word(state.y + displacement)
            state.wave_phase = (state.wave_phase + advance) & 255
        elif kind in {"AimWeapon", "AimWeaponPitch"}:
            state.saved_angles = state.pitch, state.yaw, state.roll
            dx = signed_word(target_position[0] - state.x)
            dy = signed_word(target_position[1] - state.y)
            dz = signed_word(target_position[2] - state.z)
            state.pitch = sf2_atan16(dy, xz_distance(dx, dz), curve) >> 8
            if kind == "AimWeapon":
                state.yaw = (-(sf2_atan16(dx, dz, curve) >> 8)) & 255
        elif kind == "RestoreFlightAngles":
            if state.saved_angles is None:
                raise SystemExit("capital flight-angle restoration has no saved state")
            state.pitch, state.yaw, state.roll = state.saved_angles
            state.saved_angles = None
        else:
            raise AssertionError(action)

    failures = []
    previous_player = anchor.player
    for sample in samples[1:]:
        for actor in (FIRST_ACTOR, SECOND_ACTOR):
            expected = sample.first if actor == FIRST_ACTOR else sample.second
            for action in schedule[sample.retail_frame][actor]:
                apply(states[actor], action, sample.player, previous_player)
            if expected is None:
                continue
            if states[actor].pose() != expected:
                failures.append(
                    (sample.retail_frame, actor, states[actor].pose(), expected)
                )
        previous_player = sample.player
    if failures:
        first = failures[0]
        for failure in failures[:12]:
            print(
                f"frame={failure[0]} actor={failure[1]} "
                f"actual={failure[2]} expected={failure[3]}"
            )
        raise SystemExit(
            f"semantic capital replay diverges at retail frame {first[0]} "
            f"{first[1]} ({len(failures)} mismatches)"
        )


BANK_TARGET_NAMES = {
    8: "StarboardLight",
    10: "StarboardModerate",
    11: "StarboardFirm",
    13: "StarboardStrong",
    14: "StarboardSteep",
    17: "StarboardHard",
    -8: "PortLight",
    -9: "PortShallow",
    -13: "PortStrong",
    -14: "PortSteep",
    -15: "PortVerySteep",
    -16: "PortSharp",
    -26: "PortExtreme",
    -27: "PortNearMaximum",
}
SPEED_NAMES = {10: "Entry", 30: "Approach", 50: "Accelerating", 60: "Combat"}


def rust_action(action: Action) -> str:
    kind, value = action
    if kind == "BeginPitchManeuver":
        return (
            "CapitalFlightAction::BeginPitchManeuver("
            f"CapitalManeuverDirection::{value})"
        )
    if kind == "ChasePitch":
        return f"CapitalFlightAction::ChasePitch(CapitalPitchTarget::{value})"
    if kind == "ChaseBank":
        try:
            target = BANK_TARGET_NAMES[int(value)]
        except KeyError as error:
            raise SystemExit(f"untyped capital bank target {value}") from error
        return f"CapitalFlightAction::ChaseBank(CapitalBankTarget::{target})"
    if kind == "SetSpeed":
        try:
            speed = SPEED_NAMES[int(value)]
        except KeyError as error:
            raise SystemExit(f"untyped capital flight speed {value}") from error
        return f"CapitalFlightAction::SetSpeed(CapitalFlightSpeed::{speed})"
    if kind == "Move":
        return f"CapitalFlightAction::Move(CapitalTurnMode::{value})"
    if kind == "ApplyVerticalWave":
        return (
            "CapitalFlightAction::ApplyVerticalWaveMode("
            f"CapitalWaveMode::{value})"
        )
    if kind == "FacePlayer":
        return (
            "CapitalFlightAction::FacePlayerAt("
            f"PlayerTargetTiming::{value})"
        )
    if kind == "AimWeapon":
        return (
            "CapitalFlightAction::AimWeaponAt("
            f"PlayerTargetTiming::{value})"
        )
    if value is not None:
        raise SystemExit(f"untyped capital action value: {action}")
    return f"CapitalFlightAction::{kind}"


def grouped(value: int) -> str:
    return f"{value:_}"


def format_rust(source: str) -> str:
    result = subprocess.run(
        ["rustfmt", "--edition", "2021", "--emit", "stdout"],
        input=source,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"rustfmt failed for generated capital schedule:\n{result.stderr}")
    return result.stdout


def rust_source(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> str:
    first_present = [sample for sample in samples if sample.first is not None]
    second_present = [sample for sample in samples if sample.second is not None]
    anchor = samples[0]
    assert anchor.first is not None and anchor.second is not None
    start_frame = ANCHOR_RETAIL_FRAME + RETAIL_FRAME_STEP
    end_frame = first_present[-1].retail_frame
    first_inactive = [
        sample.retail_frame
        for sample in samples
        if sample.first is None
        and first_present[0].retail_frame < sample.retail_frame < first_present[-1].retail_frame
    ]

    flattened: list[Action] = []
    ranges = []
    for frame in range(start_frame, end_frame + 1, RETAIL_FRAME_STEP):
        actor_ranges = []
        for actor in (FIRST_ACTOR, SECOND_ACTOR):
            start = len(flattened)
            actions = schedule[frame][actor]
            flattened.extend(actions)
            actor_ranges.append((start, len(actions)))
        ranges.append(tuple(actor_ranges))

    def pose_source(pose: tuple[int, ...]) -> str:
        return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"

    lines = [
        "//! Generated semantic capital-craft schedule for the first retail re-engagement.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, CapitalBankTarget, CapitalFlightAction, CapitalFlightSpeed,",
        "    CapitalManeuverDirection, CapitalPitchTarget, CapitalTurnMode, CapitalWaveMode,",
        "    MissionEncounterPose, PlayerTargetTiming,",
        "};",
        "",
        f"pub(super) const INITIAL_RETAIL_FRAME: u16 = {ANCHOR_RETAIL_FRAME};",
        f"pub(super) const START_RETAIL_FRAME: u16 = {start_frame};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped(end_frame)};",
        f"pub(super) const FIRST_DEPARTURE_RETAIL_FRAME: u16 = {grouped(first_present[-1].retail_frame + RETAIL_FRAME_STEP)};",
        f"pub(super) const SECOND_DEPARTURE_RETAIL_FRAME: u16 = {grouped(second_present[-1].retail_frame + RETAIL_FRAME_STEP)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "pub(super) const INITIAL_POSES: [MissionEncounterPose; 2] = [",
        f"    {pose_source(anchor.first)},",
        f"    {pose_source(anchor.second)},",
        "];",
        "",
        f"pub(super) const FIRST_INACTIVE_RETAIL_FRAMES: [u16; {len(first_inactive)}] = [",
    ]
    lines.extend(f"    {grouped(frame)}," for frame in first_inactive)
    lines.extend(
        [
            "];",
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
    )
    lines.extend(f"    {rust_action(action)}," for action in flattened)
    lines.extend(["];"])
    lines.extend(["", f"static TICKS: [ActionRangePair; {len(ranges)}] = ["])
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
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        import_raw_logic(args.import_raw, args.logic_fixture)
    events = read_logic_fixture(args.logic_fixture)
    samples = read_pose_fixture(args.pose_fixture)
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    schedule = build_schedule(events, cosine, samples)
    replay(schedule, samples)
    generated = format_rust(rust_source(schedule, samples))
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(f"generated second-sortie capital schedule is stale: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"second-sortie capital schedule {action}: "
        f"{len(samples)} retained pose boundaries -> {args.output}"
    )


if __name__ == "__main__":
    main()
