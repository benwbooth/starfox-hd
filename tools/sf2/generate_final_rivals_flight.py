#!/usr/bin/env python3
"""Generate typed final-rival flight plans from operation-level oracle evidence."""

from __future__ import annotations

import argparse
import hashlib
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "final_rivals_flight.rs"
)
RETAIL_FRAME_STEP = 4
RIVAL_SOURCE_ID = "0576"

SEMANTIC_EVENTS = frozenset(
    {
        ("move", "0174"),
        ("wait-for-angle", "027E"),
        ("move", "027E"),
        ("face-player", "029E"),
        ("projectile-face-smooth", "029A"),
        ("move", "028F"),
        ("move", "0169"),
    }
)

STEERING = {
    (40, 2, 40): "RivalApproachSteering::EntryClimb",
    (-40, -2, -40): "RivalApproachSteering::EntryDive",
    (40, -2, -40): "RivalApproachSteering::SecondClimb",
    (-40, 2, 40): "RivalApproachSteering::SecondDive",
}


@dataclass(frozen=True)
class FlightSpec:
    label: str
    rust_name: str
    shape_token: str
    raw_sample_start_elapsed: int
    presentation_start_retail_frame: int
    flight_start_retail_frame: int
    end_retail_frame: int
    departure_retail_frame: int
    initial_pose: tuple[int, ...]
    hidden_retail_frames: tuple[int, ...]
    pose_fixture: Path
    logic_fixture: Path


FLIGHTS = (
    FlightSpec(
        "final pursuer",
        "FINAL_PURSUER",
        "C348",
        133_432,
        56,
        404,
        892,
        896,
        (10_148, 0, 8_192, 21, 76, 0, 0),
        (324,),
        FIXTURE_DIRECTORY / "final_pursuer_path.trace",
        FIXTURE_DIRECTORY / "final_pursuer_rival_logic.trace",
    ),
    FlightSpec(
        "Wolf blockade",
        "WOLF_BLOCKADE",
        "E62C",
        135_728,
        60,
        408,
        952,
        956,
        (10_137, 0, 8_181, 21, 76, 0, 0),
        (),
        FIXTURE_DIRECTORY / "wolf_blockade_path.trace",
        FIXTURE_DIRECTORY / "wolf_blockade_rival_logic.trace",
    ),
)


@dataclass(frozen=True)
class RawEvent:
    sequence: int
    elapsed: int
    event: str
    path: str
    pose: tuple[int, ...]
    selected_pose: tuple[int, ...]
    extension: bytes


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def signed_byte(value: int) -> int:
    return value - 256 if value >= 128 else value


def parse_tuple(value: str) -> tuple[int, ...]:
    return tuple(map(int, value.split(",")))


def load_player_poses(path: Path) -> dict[int, tuple[int, ...]]:
    poses = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        poses[int(values["retail_frame"])] = parse_tuple(values["player"])
    if not poses:
        raise SystemExit(f"pose fixture is empty: {path}")
    return poses


def load_rival_poses(path: Path) -> dict[int, tuple[int, ...]]:
    poses = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        if values["rival"] != "-":
            poses[int(values["retail_frame"])] = parse_tuple(values["rival"])
    if not poses:
        raise SystemExit(f"pose fixture has no rival poses: {path}")
    return poses


def raw_events(path: Path, spec: FlightSpec) -> list[RawEvent]:
    result = []
    for sequence, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        values = fields(line)
        event = values.get("event")
        path_offset = values.get("path")
        if values.get("object") != RIVAL_SOURCE_ID:
            continue
        if values.get("shape") != spec.shape_token:
            continue
        if (event, path_offset) not in SEMANTIC_EVENTS:
            continue
        result.append(
            RawEvent(
                sequence,
                int(values["elapsed"]),
                event,
                path_offset,
                parse_tuple(values["pose"]),
                parse_tuple(values["selected_pose"]),
                bytes.fromhex(values["extension"]),
            )
        )
    if not result:
        raise SystemExit(f"raw trace has no {spec.label} logic: {path}")
    return result


def retail_frame(event: RawEvent, spec: FlightSpec) -> int:
    return (
        (event.elapsed - spec.raw_sample_start_elapsed) // RETAIL_FRAME_STEP + 1
    ) * RETAIL_FRAME_STEP


def target_timing(
    event: RawEvent,
    frame: int,
    player_poses: dict[int, tuple[int, ...]],
) -> str:
    previous = player_poses[frame - RETAIL_FRAME_STEP]
    current = player_poses[frame]
    target = event.selected_pose
    if target[:3] == previous[:3]:
        return "PlayerTargetTiming::Previous"
    if target[:3] == current[:3]:
        return "PlayerTargetTiming::Current"
    midpoint = tuple(
        previous[index] + int((current[index] - previous[index]) / 2)
        for index in range(3)
    )
    if target[:3] == midpoint:
        return "PlayerTargetTiming::Midpoint"
    raise SystemExit(
        f"{event.elapsed}: {event.event} target is not a typed player timing"
    )


def semantic_actions(
    events: list[RawEvent],
    spec: FlightSpec,
    player_poses: dict[int, tuple[int, ...]],
    rival_poses: dict[int, tuple[int, ...]],
) -> dict[int, list[str]]:
    scheduled: dict[int, list[tuple[int, int, str]]] = defaultdict(list)
    phase_started = set()

    def add(frame: int, event: RawEvent, order: int, action: str) -> None:
        scheduled[frame].append((event.elapsed, event.sequence * 10 + order, action))

    for event in events:
        frame = retail_frame(event, spec)
        if event.event in {"face-player", "projectile-face-smooth"}:
            following_move = next(
                candidate
                for candidate in events
                if candidate.sequence > event.sequence
                and candidate.event == "move"
                and candidate.path == "028F"
            )
            expected_pose = rival_poses.get(frame)
            if (
                expected_pose is not None
                and expected_pose[3:6] == event.pose[3:6]
                and expected_pose[3:6] != following_move.pose[3:6]
            ):
                frame = retail_frame(following_move, spec)
        if not spec.flight_start_retail_frame <= frame <= spec.end_retail_frame:
            continue

        if event.pose[1] == -4_000 and event.path != "0174":
            add(frame, event, 0, "FinalRivalAction::MaintainCombatAltitude")
        if event.pose[1] == 4_000 and event.path == "0174":
            add(frame, event, 0, "FinalRivalAction::ClampFlightAltitude")

        if event.event == "move" and event.path == "0174":
            steering_values = tuple(signed_byte(value) for value in event.extension[20:23])
            steering = STEERING.get(steering_values)
            if steering is None:
                raise SystemExit(f"unknown {spec.label} steering: {steering_values}")
            if "approach" not in phase_started:
                add(frame, event, 1, "FinalRivalAction::BeginApproach")
                phase_started.add("approach")
            add(frame, event, 2, f"FinalRivalAction::AdvanceSteered({steering})")
        elif event.event == "wait-for-angle" and event.path == "027E":
            if "maneuver" not in phase_started:
                add(frame, event, 1, "FinalRivalAction::BeginCombatManeuver")
                phase_started.add("maneuver")
            add(frame, event, 2, "FinalRivalAction::ChaseRollToLevel")
        elif event.event == "move" and event.path in {"027E", "028F"}:
            add(frame, event, 2, "FinalRivalAction::Advance")
        elif event.event == "face-player":
            if "attack" not in phase_started:
                add(frame, event, 1, "FinalRivalAction::BeginAttack")
                phase_started.add("attack")
            timing = target_timing(event, frame, player_poses)
            add(
                frame,
                event,
                2,
                f"FinalRivalAction::FacePlayerYawAndLevelPitch({timing})",
            )
        elif event.event == "projectile-face-smooth":
            timing = target_timing(event, frame, player_poses)
            add(frame, event, 2, f"FinalRivalAction::FacePlayerSmooth({timing})")
        elif event.event == "move" and event.path == "0169":
            add(frame, event, 1, "FinalRivalAction::BeginDeparture")
            add(frame, event, 2, "FinalRivalAction::LaunchDeparture")

    # The altitude keeper is a companion behavior that can run after the
    # authored movement operation. Retain that ordering only at boundaries
    # where the independent pose oracle proves the post-movement clamp.
    for frame, frame_actions in scheduled.items():
        if any("Advance" in action for _, _, action in frame_actions):
            rival_altitude = rival_poses.get(frame, (0, 0))[1]
            if rival_altitude == -4_000:
                frame_actions.append(
                    (10**9, 10**9, "FinalRivalAction::MaintainCombatAltitude")
                )
            elif rival_altitude == 4_000:
                frame_actions.append(
                    (10**9, 10**9, "FinalRivalAction::ClampFlightAltitude")
                )

    return {
        frame: [action for _, _, action in sorted(actions)]
        for frame, actions in sorted(scheduled.items())
    }


def render_compact(
    actions: dict[int, list[str]], spec: FlightSpec, raw_sha256: str
) -> str:
    lines = [
        f"# Semantic {spec.label} actions recovered from the campaign oracle.",
        f"# Raw source SHA-256: {raw_sha256}",
        f"# raw_sample_start_elapsed={spec.raw_sample_start_elapsed}",
        f"# presentation_start_retail_frame={spec.presentation_start_retail_frame}",
        f"# flight_start_retail_frame={spec.flight_start_retail_frame}",
        f"# end_retail_frame={spec.end_retail_frame}",
        f"# departure_retail_frame={spec.departure_retail_frame}",
        "# initial_pose=" + ",".join(map(str, spec.initial_pose)),
    ]
    for frame, frame_actions in actions.items():
        for action in frame_actions:
            lines.append(f"retail_frame={frame} action={action}")
    return "\n".join(lines) + "\n"


def load_compact(path: Path) -> dict[int, list[str]]:
    actions: dict[int, list[str]] = defaultdict(list)
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        actions[int(values["retail_frame"])].append(values["action"])
    if not actions:
        raise SystemExit(f"final-rival logic fixture is empty: {path}")
    return dict(sorted(actions.items()))


def rust_identifier(spec: FlightSpec, suffix: str) -> str:
    return f"{spec.rust_name}_{suffix}"


def generate_rust(plans: list[tuple[FlightSpec, dict[int, list[str]]]]) -> str:
    lines = [
        "//! Generated semantic flight plans for the two retail final rivals.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, FinalRivalAction, MissionEncounterPose, PlayerTargetTiming,",
        "    RivalApproachSteering,",
        "};",
        "",
        "#[derive(Debug, Clone, Copy)]",
        "pub(super) struct FinalRivalFlightPlan {",
        "    pub presentation_start_retail_frame: u16,",
        "    pub flight_start_retail_frame: u16,",
        "    pub end_retail_frame: u16,",
        "    pub departure_retail_frame: u16,",
        "    pub initial_pose: MissionEncounterPose,",
        "    hidden_retail_frames: &'static [u16],",
        "    frames: &'static [FrameActions],",
        "    actions: &'static [FinalRivalAction],",
        "}",
        "",
        "impl FinalRivalFlightPlan {",
        "    pub fn actions(self, retail_frame: u16) -> &'static [FinalRivalAction] {",
        "        if retail_frame > self.end_retail_frame {",
        "            return &[];",
        "        }",
        "        let Ok(index) = self.frames.binary_search_by_key(&retail_frame, |frame| frame.retail_frame) else {",
        "            return &[];",
        "        };",
        "        let range = self.frames[index];",
        "        let start = usize::from(range.start);",
        "        &self.actions[start..start + usize::from(range.len)]",
        "    }",
        "",
        "    pub fn is_hidden(self, retail_frame: u16) -> bool {",
        "        self.hidden_retail_frames.contains(&retail_frame)",
        "    }",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct FrameActions {",
        "    retail_frame: u16,",
        "    start: u16,",
        "    len: u8,",
        "}",
    ]

    for spec, actions in plans:
        flattened = []
        ranges = []
        for frame, frame_actions in actions.items():
            ranges.append((frame, len(flattened), len(frame_actions)))
            flattened.extend(frame_actions)
        frames_name = rust_identifier(spec, "FRAMES")
        actions_name = rust_identifier(spec, "ACTIONS")
        lines.extend(["", f"static {frames_name}: [FrameActions; {len(ranges)}] = ["])
        for frame, start, length in ranges:
            lines.append(
                "    FrameActions { "
                f"retail_frame: {frame:_}, start: {start:_}, len: {length} "
                "},"
            )
        lines.extend(["];"])
        lines.extend(["", f"static {actions_name}: [FinalRivalAction; {len(flattened)}] = ["])
        lines.extend(f"    {action}," for action in flattened)
        lines.append("];")
        pose = ", ".join(f"{value:_}" for value in spec.initial_pose)
        hidden_frames = ", ".join(f"{frame:_}" for frame in spec.hidden_retail_frames)
        lines.extend(
            [
                "",
                f"pub(super) const {spec.rust_name}: FinalRivalFlightPlan = FinalRivalFlightPlan {{",
                "    presentation_start_retail_frame: "
                f"{spec.presentation_start_retail_frame:_},",
                f"    flight_start_retail_frame: {spec.flight_start_retail_frame:_},",
                f"    end_retail_frame: {spec.end_retail_frame:_},",
                f"    departure_retail_frame: {spec.departure_retail_frame:_},",
                f"    initial_pose: mission_encounter_pose([{pose}]),",
                f"    hidden_retail_frames: &[{hidden_frames}],",
                f"    frames: &{frames_name},",
                f"    actions: &{actions_name},",
                "};",
            ]
        )
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    raw_sha256 = None
    if args.import_raw is not None:
        raw_sha256 = hashlib.sha256(args.import_raw.read_bytes()).hexdigest()

    plans = []
    for spec in FLIGHTS:
        if args.import_raw is not None:
            actions = semantic_actions(
                raw_events(args.import_raw, spec),
                spec,
                load_player_poses(spec.pose_fixture),
                load_rival_poses(spec.pose_fixture),
            )
            spec.logic_fixture.write_text(
                render_compact(actions, spec, raw_sha256), encoding="utf-8"
            )
        actions = load_compact(spec.logic_fixture)
        plans.append((spec, actions))

    source = generate_rust(plans)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated final-rival flight plans are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")

    for spec, actions in plans:
        print(
            f"{spec.label} flight plan verified: "
            f"{sum(map(len, actions.values()))} semantic actions across "
            f"{len(actions)} boundaries"
        )


if __name__ == "__main__":
    main()
