#!/usr/bin/env python3
"""Generate the typed fighter schedule for SF2's first re-engagement.

Raw emulator callbacks are reduced to gameplay operations in a compact fixture.
The generated Rust is accepted only after an independent flat-state replay
matches every retained four-frame pose for both fighters.
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
    chase_power,
    mulslog,
    rust_table,
    signed_byte,
    signed_word,
    trunc_div,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "second_sortie_fighter_logic.trace"
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
    / "second_sortie_fighters.rs"
)

UPPER_ACTOR = "upper"
LOWER_ACTOR = "lower"
SOURCE_ACTORS = {"05B5": UPPER_ACTOR, "0576": LOWER_ACTOR}
SOURCE_SHAPE = "EA00"
RAW_SAMPLE_START_ELAPSED = 14_912
ANCHOR_RETAIL_FRAME = 68
RETAIL_FRAME_STEP = 4
IGNORED_EVENTS = {"pitch-target-write", "position-y-write", "divide-angle"}
RELEVANT_EVENTS = {
    "move",
    "vertical-step",
    "wait-for-angle",
    "chase-angle",
    "wait",
    "random-value",
    "random-branch",
    "schedule",
    "fire",
}


@dataclass(frozen=True)
class LogicEvent:
    elapsed: int
    actor: str
    event: str
    path: str
    bank_target: int
    wave_sample: int
    wave_phase: int


@dataclass(frozen=True)
class PoseSample:
    retail_frame: int
    upper: tuple[int, ...] | None
    lower: tuple[int, ...] | None


@dataclass
class FlightState:
    x: int
    y: int
    z: int
    pitch: int
    yaw: int
    roll: int
    speed: int
    wave_phase: int = 1
    wave_sample: int = 0
    wave_quarters_applied: int = 0
    maneuver_bank: int = 0
    vertical_pitch_target: int = 0
    centering_ticks_remaining: int = 0
    pending_velocity: tuple[int, int, int] | None = None

    def pose(self) -> tuple[int, ...]:
        return self.x, self.y, self.z, self.pitch, self.yaw, self.roll, self.speed


Action = tuple[str, int | str | None]


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


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
            or event in IGNORED_EVENTS
        ):
            continue
        extension = bytes.fromhex(values.get("extension", ""))
        result.append(
            LogicEvent(
                elapsed=int(values["elapsed"]),
                actor=actor,
                event=event,
                path=values.get("path", "none"),
                bank_target=signed_byte(extension[22]) if len(extension) > 22 else 0,
                wave_sample=signed_byte(extension[33]) if len(extension) > 33 else 0,
                wave_phase=extension[34] if len(extension) > 34 else 0,
            )
        )
    return result


def import_raw_logic(source: Path, output: Path) -> None:
    lines = [
        "# Compact oracle evidence for the first re-engagement fighters.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Opaque actor storage was reduced to semantic flight values.",
    ]
    for event in read_raw_events(source):
        lines.append(
            f"elapsed={event.elapsed} actor={event.actor} event={event.event} "
            f"path={event.path} bank_target={event.bank_target} "
            f"wave_sample={event.wave_sample} wave_phase={event.wave_phase}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_logic_fixture(path: Path) -> list[LogicEvent]:
    result = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        values = fields(line)
        result.append(
            LogicEvent(
                elapsed=int(values["elapsed"]),
                actor=values["actor"],
                event=values["event"],
                path=values["path"],
                bank_target=int(values["bank_target"]),
                wave_sample=int(values["wave_sample"]),
                wave_phase=int(values["wave_phase"]),
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
        targets = values["targets"].split("/")
        if len(targets) != 4:
            raise SystemExit("second-sortie pose fixture has malformed targets")

        def pose(index: int) -> tuple[int, ...] | None:
            return None if targets[index] == "-" else tuple(map(int, targets[index].split(",")))

        result.append(PoseSample(int(values["retail_frame"]), pose(2), pose(3)))
    if not result:
        raise SystemExit("second-sortie pose fixture is empty")
    return result


def event_retail_frame(elapsed: int) -> int:
    offset = elapsed - (RAW_SAMPLE_START_ELAPSED - 1)
    return ((offset + RETAIL_FRAME_STEP - 1) // RETAIL_FRAME_STEP) * RETAIL_FRAME_STEP


def remove_last(actions: list[Action], expected: Action, label: str) -> None:
    for index in range(len(actions) - 1, -1, -1):
        if actions[index] == expected:
            del actions[index]
            return
    raise SystemExit(f"unexpected fighter cooperative boundary: {label}")


def build_schedule(
    events: list[LogicEvent], samples: list[PoseSample]
) -> dict[int, dict[str, list[Action]]]:
    schedule: dict[int, dict[str, list[Action]]] = defaultdict(
        lambda: {UPPER_ACTOR: [], LOWER_ACTOR: []}
    )
    observed_bank = {UPPER_ACTOR: 0, LOWER_ACTOR: 0}
    final_frames = {
        UPPER_ACTOR: max(sample.retail_frame for sample in samples if sample.upper is not None),
        LOWER_ACTOR: max(sample.retail_frame for sample in samples if sample.lower is not None),
    }

    for event in events:
        frame = event_retail_frame(event.elapsed)
        if frame > final_frames[event.actor]:
            continue
        actions = schedule[frame][event.actor]
        if event.event == "schedule":
            if event.path == "602E":
                actions.append(("EntrySetup", None))
            elif event.path == "6097":
                actions.append(("SetBankTarget", event.bank_target))
                observed_bank[event.actor] = event.bank_target
            elif event.path == "60AE":
                actions.append(("CenterAltitude", None))
        elif event.event == "random-branch":
            if event.path == "607D":
                direction = "Starboard" if event.wave_phase == 64 else "Port"
                actions.append(("BeginEntryTurn", direction))
                observed_bank[event.actor] = 24 if direction == "Starboard" else -24
            elif event.path == "60B9" and event.bank_target != observed_bank[event.actor]:
                actions.append(("BeginManeuver", event.bank_target))
                observed_bank[event.actor] = event.bank_target
        elif event.event == "wait-for-angle":
            target = {"6080": 24, "6086": -24, "6091": 0}.get(event.path)
            if target is None:
                raise SystemExit(f"untyped fighter bank chase path: {event.path}")
            actions.append(("ChaseRoll", target))
        elif event.event == "move":
            if event.path == "60A9":
                actions.append(("CenterAltitudeDuringManeuver", None))
            acceleration = "Accelerate" if event.path in {"6080", "6086"} else "Hold"
            actions.append(("Move", acceleration))
        elif event.event == "vertical-step":
            actions.append(
                ("ApplyEntryWave", None)
                if event.path == "8942"
                else ("ApplyWaveQuarter", None)
            )
        elif event.event == "chase-angle":
            if event.path == "60C6":
                actions.append(("ChasePitch", None))
            elif event.path == "60CB":
                actions.append(("ChaseBank", None))
            else:
                raise SystemExit(f"untyped fighter steering path: {event.path}")
        elif event.event == "fire":
            if event.wave_sample != -6:
                raise SystemExit(f"untyped fighter weapon pitch target: {event.wave_sample}")
            actions.append(("SetWeaponPitchTarget", event.wave_sample))
        elif event.event not in {"random-value", "wait"}:
            raise SystemExit(f"untyped fighter event: {event.event}")

    # Retail's cooperative task queue exposes four boundaries between parts of
    # otherwise ordinary typed flight operations.
    upper_340 = schedule[340][UPPER_ACTOR]
    move_index = upper_340.index(("Move", "Accelerate"))
    upper_340[move_index] = ("BeginMovement", "Accelerate")
    schedule[344][UPPER_ACTOR].insert(0, ("FinishMovement", None))

    remove_last(
        schedule[580][UPPER_ACTOR],
        ("ApplyWaveQuarter", None),
        "upper wave at frame 580",
    )
    schedule[584][UPPER_ACTOR].insert(0, ("ApplyWaveQuarter", None))

    upper_1128 = schedule[1_128][UPPER_ACTOR]
    try:
        del upper_1128[upper_1128.index(("CenterAltitudeDuringManeuver", None))]
    except ValueError as error:
        raise SystemExit(
            "unexpected fighter cooperative boundary: upper altitude center at frame 1128"
        ) from error
    schedule[1_124][UPPER_ACTOR].append(("CenterAltitudeDuringManeuver", None))

    remove_last(
        schedule[1_412][UPPER_ACTOR],
        ("ApplyWaveQuarter", None),
        "upper wave at frame 1412",
    )
    schedule[1_416][UPPER_ACTOR].insert(0, ("ApplyWaveQuarter", None))
    return schedule


def replay(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> None:
    sine = rust_table("SINTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    cosine = rust_table("COSTAB", TRIG_SOURCE.read_text(encoding="utf-8"))
    anchor = next(
        sample for sample in samples if sample.upper is not None and sample.lower is not None
    )
    assert anchor.upper is not None and anchor.lower is not None
    states = {
        UPPER_ACTOR: FlightState(*anchor.upper),
        LOWER_ACTOR: FlightState(*anchor.lower),
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

    def center_altitude(state: FlightState) -> None:
        difference = signed_word(-state.y)
        step = difference >> 3 if difference >= 0 else -((-difference) >> 3)
        if step == 0 and difference != 0:
            step = 1 if difference > 0 else -1
        state.y = signed_word(state.y + step)

    def apply_wave_quarter(state: FlightState) -> None:
        if state.wave_quarters_applied == 0:
            state.wave_sample = cosine[state.wave_phase]
        state.y = signed_word(state.y + state.wave_sample)
        state.wave_quarters_applied += 1
        if state.wave_quarters_applied == 4:
            state.wave_quarters_applied = 0
            state.wave_phase = (state.wave_phase + 4) & 255
            state.vertical_pitch_target = trunc_div(state.wave_sample, 2)

    def begin_movement(state: FlightState, accelerate: bool, horizontal_only: bool) -> None:
        if accelerate:
            state.speed = min(state.speed + 30, 63)
        state.yaw = (state.yaw + trunc_div(signed_byte(state.roll), 4)) & 255
        movement = velocity(state)
        state.x = signed_word(state.x + movement[0])
        if horizontal_only:
            state.pending_velocity = movement
        else:
            state.y = signed_word(state.y + movement[1])
            state.z = signed_word(state.z + movement[2])

    def apply(state: FlightState, action: Action) -> None:
        kind, value = action
        if kind == "EntrySetup":
            state.y = signed_word(state.y - 3_197)
            state.yaw = 38
            state.speed = 10
            state.wave_phase = 1
        elif kind == "SetBankTarget":
            state.maneuver_bank = int(value)
        elif kind == "BeginEntryTurn":
            state.wave_phase = 64 if value == "Starboard" else 192
            state.maneuver_bank = 24 if value == "Starboard" else -24
        elif kind == "BeginManeuver":
            state.maneuver_bank = int(value)
            state.centering_ticks_remaining = 32
        elif kind == "ChaseRoll":
            state.roll = chase_power(state.roll, int(value) & 255, 3, 8, 8)
        elif kind == "CenterAltitudeDuringManeuver":
            if state.centering_ticks_remaining > 0:
                center_altitude(state)
        elif kind == "CenterAltitude":
            center_altitude(state)
        elif kind in {"Move", "BeginMovement"}:
            begin_movement(
                state,
                value == "Accelerate",
                kind == "BeginMovement",
            )
        elif kind == "FinishMovement":
            if state.pending_velocity is None:
                raise SystemExit("fighter movement continuation lacks pending velocity")
            _, dy, dz = state.pending_velocity
            state.y = signed_word(state.y + dy)
            state.z = signed_word(state.z + dz)
            state.pending_velocity = None
        elif kind == "ApplyEntryWave":
            state.y = signed_word(state.y + trunc_div(cosine[state.wave_phase], 8))
            state.wave_phase = (state.wave_phase + 2) & 255
        elif kind == "ApplyWaveQuarter":
            apply_wave_quarter(state)
        elif kind == "ChasePitch":
            state.pitch = chase_power(
                state.pitch, state.vertical_pitch_target & 255, 3, 8, 8
            )
            if state.centering_ticks_remaining > 0:
                state.vertical_pitch_target = trunc_div(state.vertical_pitch_target, 2)
                state.centering_ticks_remaining -= 1
        elif kind == "ChaseBank":
            state.roll = chase_power(state.roll, state.maneuver_bank & 255, 3, 8, 8)
        elif kind == "SetWeaponPitchTarget":
            state.vertical_pitch_target = int(value)
        else:
            raise AssertionError(action)

    failures = []
    retained = 0
    for sample in samples:
        if sample.retail_frame <= anchor.retail_frame:
            continue
        for actor in (UPPER_ACTOR, LOWER_ACTOR):
            for action in schedule[sample.retail_frame][actor]:
                apply(states[actor], action)
            expected = sample.upper if actor == UPPER_ACTOR else sample.lower
            if expected is None:
                continue
            retained += 1
            if states[actor].pose() != expected:
                failures.append(
                    (sample.retail_frame, actor, states[actor].pose(), expected)
                )
    if failures:
        for frame, actor, actual, expected in failures[:12]:
            print(
                f"frame={frame} actor={actor} actual={actual} expected={expected}"
            )
        first = failures[0]
        raise SystemExit(
            f"semantic fighter replay diverges at frame {first[0]} "
            f"{first[1]} ({len(failures)} mismatches)"
        )
    print(f"second-sortie fighter replay verified: {retained} retained pose boundaries")


BANK_TARGET_NAMES = {
    0: "Level",
    4: "StarboardGentle",
    24: "StarboardEntry",
    -4: "PortGentle",
    -8: "PortInitial",
    -24: "PortEntry",
}


def rust_action(action: Action) -> str:
    kind, value = action
    if kind in {"SetBankTarget", "BeginManeuver", "ChaseRoll"}:
        try:
            target = BANK_TARGET_NAMES[int(value)]
        except KeyError as error:
            raise SystemExit(f"untyped fighter bank target: {value}") from error
        return f"ReengagementFighterAction::{kind}(ReengagementFighterBankTarget::{target})"
    if kind == "BeginEntryTurn":
        return (
            "ReengagementFighterAction::BeginEntryTurn("
            f"ReengagementFighterDirection::{value})"
        )
    if kind in {"Move", "BeginMovement"}:
        return (
            f"ReengagementFighterAction::{kind}("
            f"ReengagementFighterAcceleration::{value})"
        )
    if kind == "SetWeaponPitchTarget":
        if value != -6:
            raise SystemExit(f"untyped fighter pitch target: {value}")
        return (
            "ReengagementFighterAction::SetWeaponPitchTarget("
            "ReengagementFighterPitchTarget::WeaponAim)"
        )
    if value is not None:
        raise SystemExit(f"untyped fighter action value: {action}")
    return f"ReengagementFighterAction::{kind}"


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
        raise SystemExit(f"rustfmt failed for generated fighter schedule:\n{result.stderr}")
    return result.stdout


def rust_source(
    schedule: dict[int, dict[str, list[Action]]], samples: list[PoseSample]
) -> str:
    anchor = next(
        sample for sample in samples if sample.upper is not None and sample.lower is not None
    )
    assert anchor.upper is not None and anchor.lower is not None
    present = {
        UPPER_ACTOR: [sample for sample in samples if sample.upper is not None],
        LOWER_ACTOR: [sample for sample in samples if sample.lower is not None],
    }
    end_frame = present[UPPER_ACTOR][-1].retail_frame
    start_frame = anchor.retail_frame + RETAIL_FRAME_STEP
    flattened: list[Action] = []
    ranges = []
    for frame in range(start_frame, end_frame + 1, RETAIL_FRAME_STEP):
        actor_ranges = []
        for actor in (UPPER_ACTOR, LOWER_ACTOR):
            start = len(flattened)
            actions = schedule[frame][actor]
            flattened.extend(actions)
            actor_ranges.append((start, len(actions)))
        ranges.append(tuple(actor_ranges))

    def pose_source(pose: tuple[int, ...]) -> str:
        return "mission_encounter_pose([" + ", ".join(grouped(value) for value in pose) + "])"

    lines = [
        "//! Generated semantic fighter schedule for the first retail re-engagement.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, MissionEncounterPose, ReengagementFighterAcceleration,",
        "    ReengagementFighterAction, ReengagementFighterBankTarget,",
        "    ReengagementFighterDirection, ReengagementFighterPitchTarget,",
        "};",
        "",
        f"pub(super) const INITIAL_RETAIL_FRAME: u16 = {anchor.retail_frame};",
        f"pub(super) const START_RETAIL_FRAME: u16 = {start_frame};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {grouped(end_frame)};",
        f"pub(super) const UPPER_DEPARTURE_RETAIL_FRAME: u16 = {grouped(present[UPPER_ACTOR][-1].retail_frame + RETAIL_FRAME_STEP)};",
        f"pub(super) const LOWER_DEPARTURE_RETAIL_FRAME: u16 = {grouped(present[LOWER_ACTOR][-1].retail_frame + RETAIL_FRAME_STEP)};",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        "pub(super) const INITIAL_POSES: [MissionEncounterPose; 2] = [",
        f"    {pose_source(anchor.upper)},",
        f"    {pose_source(anchor.lower)},",
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
        "    upper: ActionRange,",
        "    lower: ActionRange,",
        "}",
        "",
        "impl ActionRangePair {",
        "    const fn new(upper_start: u16, upper_len: u8, lower_start: u16, lower_len: u8) -> Self {",
        "        Self {",
        "            upper: ActionRange::new(upper_start, upper_len),",
        "            lower: ActionRange::new(lower_start, lower_len),",
        "        }",
        "    }",
        "}",
        "",
        f"static ACTIONS: [ReengagementFighterAction; {len(flattened)}] = [",
    ]
    lines.extend(f"    {rust_action(action)}," for action in flattened)
    lines.extend(["];"])
    lines.extend(["", f"static TICKS: [ActionRangePair; {len(ranges)}] = ["])
    for (upper_start, upper_len), (lower_start, lower_len) in ranges:
        lines.append(
            "    ActionRangePair::new("
            f"{upper_start}, {upper_len}, {lower_start}, {lower_len}),"
        )
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16, upper: bool) -> &'static [ReengagementFighterAction] {",
            "    let Some(offset) = retail_frame.checked_sub(START_RETAIL_FRAME) else {",
            "        return &[];",
            "    };",
            "    if retail_frame > END_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 {",
            "        return &[];",
            "    }",
            "    let Some(pair) = TICKS.get(usize::from(offset / RETAIL_FRAME_STEP)) else {",
            "        return &[];",
            "    };",
            "    let range = if upper { pair.upper } else { pair.lower };",
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
    schedule = build_schedule(events, samples)
    replay(schedule, samples)
    generated = format_rust(rust_source(schedule, samples))
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(f"generated second-sortie fighter schedule is stale: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"second-sortie fighter schedule {action}: "
        f"{len(samples)} retained pose boundaries -> {args.output}"
    )


if __name__ == "__main__":
    main()
