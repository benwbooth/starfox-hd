#!/usr/bin/env python3
"""Generate Leon's live semantic rival flight from extended oracle evidence."""

from __future__ import annotations

import argparse
import hashlib
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_LOGIC_FIXTURE = FIXTURE_DIRECTORY / "leon_duel_rival_logic.trace"
DEFAULT_POSE_FIXTURE = FIXTURE_DIRECTORY / "leon_duel_extended.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "leon_duel_rival.rs"
)

RAW_SAMPLE_START_ELAPSED = 63_320
PRESENTATION_START_RETAIL_FRAME = 52
FLIGHT_START_RETAIL_FRAME = 400
END_RETAIL_FRAME = 1_880
INITIAL_POSE = (10_143, 0, 8_142, 21, 76, 0, 0)
RETAIL_FRAME_STEP = 4
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "C348"

SEMANTIC_EVENTS = frozenset(
    {
        ("move", "0174"),
        ("wait-for-angle", "027E"),
        ("move", "027E"),
        ("face-player", "029E"),
        ("face-player", "0194"),
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


def load_poses(
    path: Path,
) -> tuple[dict[int, tuple[int, ...]], dict[int, tuple[int, ...]]]:
    players = {}
    rivals = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        frame = int(values["retail_frame"])
        players[frame] = parse_tuple(values["player"])
        if values["rival"] != "-":
            rivals[frame] = parse_tuple(values["rival"])
    if not players or not rivals:
        raise SystemExit(f"extended Leon pose fixture is incomplete: {path}")
    return players, rivals


def raw_events(path: Path) -> list[RawEvent]:
    result = []
    for sequence, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        values = fields(line)
        event = values.get("event")
        path_offset = values.get("path")
        if values.get("object") != RIVAL_SOURCE_ID:
            continue
        if values.get("shape") != RIVAL_SHAPE_TOKEN:
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
        raise SystemExit(f"raw trace has no Leon rival logic: {path}")
    return result


def retail_frame(event: RawEvent) -> int:
    return (
        (event.elapsed - RAW_SAMPLE_START_ELAPSED) // RETAIL_FRAME_STEP + 1
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
    player_poses: dict[int, tuple[int, ...]],
    rival_poses: dict[int, tuple[int, ...]],
) -> dict[int, list[str]]:
    scheduled: dict[int, list[tuple[int, int, str]]] = defaultdict(list)
    previous_movement_path: str | None = None
    attack_frames: set[int] = set()
    approach_frames: set[int] = set()

    def add(frame: int, event: RawEvent, order: int, action: str) -> None:
        scheduled[frame].append((event.elapsed, event.sequence * 10 + order, action))

    for event in events:
        frame = retail_frame(event)
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
                frame = retail_frame(following_move)
        if not FLIGHT_START_RETAIL_FRAME <= frame <= END_RETAIL_FRAME:
            continue

        if event.pose[1] == -4_000:
            add(frame, event, 0, "LeonRivalAction::MaintainCombatAltitude")
        if event.pose[1] == 4_000:
            add(frame, event, 0, "LeonRivalAction::ClampFlightAltitude")

        if event.event == "move" and event.path == "0174":
            steering_values = tuple(signed_byte(value) for value in event.extension[20:23])
            steering = STEERING.get(steering_values)
            if steering is None:
                raise SystemExit(f"unknown Leon approach steering: {steering_values}")
            if previous_movement_path != "0174":
                add(frame, event, 1, "LeonRivalAction::BeginApproach")
            approach_frames.add(frame)
            add(
                frame,
                event,
                2,
                f"LeonRivalAction::TrackApproachPitchAndBank({event.pose[3]},{event.pose[5]})",
            )
            add(frame, event, 3, f"LeonRivalAction::TrackApproachYaw({event.pose[4]})")
            add(frame, event, 4, "LeonRivalAction::PrepareAdvance")
            finish_frame = (
                frame + RETAIL_FRAME_STEP
                if rival_poses.get(frame, ())[:3] == event.pose[:3]
                else frame
            )
            add(finish_frame, event, 5, "LeonRivalAction::FinishAdvance")
            previous_movement_path = event.path
        elif event.event == "wait-for-angle" and event.path == "027E":
            if previous_movement_path != "027E":
                add(frame, event, 1, "LeonRivalAction::BeginCombatManeuver")
            add(frame, event, 2, "LeonRivalAction::ChaseRollToLevel")
        elif event.event == "move" and event.path in {"027E", "028F"}:
            if event.path == "028F":
                attack_frames.add(frame)
            add(frame, event, 2, "LeonRivalAction::PrepareAdvance")
            finish_frame = (
                frame + RETAIL_FRAME_STEP
                if rival_poses.get(frame, ())[:3] == event.pose[:3]
                else frame
            )
            add(finish_frame, event, 3, "LeonRivalAction::FinishAdvance")
            previous_movement_path = event.path
        elif event.event == "move" and event.path == "0169":
            add(frame, event, 1, "LeonRivalAction::LaunchApproach")
            add(frame, event, 2, "LeonRivalAction::PrepareAdvance")
            finish_frame = (
                frame + RETAIL_FRAME_STEP
                if rival_poses.get(frame, ())[:3] == event.pose[:3]
                else frame
            )
            add(finish_frame, event, 3, "LeonRivalAction::FinishAdvance")
            previous_movement_path = event.path
        elif event.event == "face-player":
            timing = target_timing(event, frame, player_poses)
            if event.path == "0194":
                add(frame, event, 2, f"LeonRivalAction::FacePlayerYawSmooth({timing})")
            else:
                add(frame, event, 1, "LeonRivalAction::BeginAttack")
                add(
                    frame,
                    event,
                    2,
                    f"LeonRivalAction::FacePlayerYawAndLevelPitch({timing})",
                )
        elif event.event == "projectile-face-smooth":
            timing = target_timing(event, frame, player_poses)
            add(frame, event, 2, f"LeonRivalAction::FacePlayerSmooth({timing})")

    # The angle trackers are companion behaviors whose step can precede
    # the traced movement handler at a retained presentation boundary.
    for frame in approach_frames:
        pose = rival_poses[frame]
        scheduled[frame].append(
            (
                10**9,
                10**9 - 1,
                f"LeonRivalAction::TrackApproachPitchAndBank({pose[3]},{pose[5]})",
            )
        )
        scheduled[frame].append(
            (10**9, 10**9, f"LeonRivalAction::TrackApproachYaw({pose[4]})")
        )
    for frame in attack_frames:
        scheduled[frame].append(
            (0, -1, f"LeonRivalAction::TrackAttackBank({rival_poses[frame][5]})")
        )

    # The altitude limiter is an independently scheduled companion behavior.
    # Preserve its after-movement ordering only where the pose oracle proves it.
    for frame, frame_actions in scheduled.items():
        if any("FinishAdvance" in action for _, _, action in frame_actions):
            rival_altitude = rival_poses.get(frame, (0, 0))[1]
            if rival_altitude == -4_000:
                frame_actions.append(
                    (10**9, 10**9, "LeonRivalAction::MaintainCombatAltitude")
                )
            elif rival_altitude == 4_000:
                frame_actions.append(
                    (10**9, 10**9, "LeonRivalAction::ClampFlightAltitude")
                )

    return {
        frame: [action for _, _, action in sorted(actions)]
        for frame, actions in sorted(scheduled.items())
    }


def render_compact(actions: dict[int, list[str]], raw_sha256: str) -> str:
    lines = [
        "# Semantic Leon rival actions recovered from the extended campaign oracle.",
        f"# Raw source SHA-256: {raw_sha256}",
        f"# raw_sample_start_elapsed={RAW_SAMPLE_START_ELAPSED}",
        f"# presentation_start_retail_frame={PRESENTATION_START_RETAIL_FRAME}",
        f"# flight_start_retail_frame={FLIGHT_START_RETAIL_FRAME}",
        f"# end_retail_frame={END_RETAIL_FRAME}",
        "# initial_pose=" + ",".join(map(str, INITIAL_POSE)),
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
        raise SystemExit(f"Leon rival logic fixture is empty: {path}")
    return dict(sorted(actions.items()))


def pose_source(pose: tuple[int, ...]) -> str:
    return "mission_encounter_pose([" + ", ".join(f"{value:_}" for value in pose) + "])"


def generate_rust(
    actions: dict[int, list[str]],
    player_poses: dict[int, tuple[int, ...]],
    rival_poses: dict[int, tuple[int, ...]],
) -> str:
    flattened = []
    ranges = []
    for frame, frame_actions in actions.items():
        ranges.append((frame, len(flattened), len(frame_actions)))
        flattened.extend(frame_actions)

    oracle_frames = list(
        range(FLIGHT_START_RETAIL_FRAME, END_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    )
    player_frames = list(range(0, END_RETAIL_FRAME + 1, RETAIL_FRAME_STEP))
    lines = [
        "//! Generated semantic rival dynamics for Leon's extended retail duel.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, LeonRivalAction, MissionEncounterPose, PlayerTargetTiming,",
        "};",
        "",
        f"pub(super) const PRESENTATION_START_RETAIL_FRAME: u16 = {PRESENTATION_START_RETAIL_FRAME:_};",
        f"pub(super) const FLIGHT_START_RETAIL_FRAME: u16 = {FLIGHT_START_RETAIL_FRAME:_};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {END_RETAIL_FRAME:_};",
        f"pub(super) const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "pub(super) const INITIAL_POSE: MissionEncounterPose =",
        f"    {pose_source(INITIAL_POSE)};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "struct FrameActions {",
        "    retail_frame: u16,",
        "    start: u16,",
        "    len: u8,",
        "}",
        "",
        f"static FRAMES: [FrameActions; {len(ranges)}] = [",
    ]
    for frame, start, length in ranges:
        lines.append(
            "    FrameActions { "
            f"retail_frame: {frame:_}, start: {start:_}, len: {length} "
            "},"
        )
    lines.extend(["];"])
    lines.extend(["", f"static ACTIONS: [LeonRivalAction; {len(flattened)}] = ["])
    lines.extend(f"    {action}," for action in flattened)
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16) -> &'static [LeonRivalAction] {",
            "    if retail_frame > END_RETAIL_FRAME {",
            "        return &[];",
            "    }",
            "    let Ok(index) = FRAMES.binary_search_by_key(&retail_frame, |frame| frame.retail_frame) else {",
            "        return &[];",
            "    };",
            "    let range = FRAMES[index];",
            "    let start = usize::from(range.start);",
            "    &ACTIONS[start..start + usize::from(range.len)]",
            "}",
            "",
            f"pub(super) static PLAYER_POSES: [MissionEncounterPose; {len(player_frames)}] = [",
        ]
    )
    lines.extend(f"    {pose_source(player_poses[frame])}," for frame in player_frames)
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn player_pose(retail_frame: u16) -> Option<MissionEncounterPose> {",
            "    if retail_frame % RETAIL_FRAME_STEP != 0 {",
            "        return None;",
            "    }",
            "    PLAYER_POSES.get(usize::from(retail_frame / RETAIL_FRAME_STEP)).copied()",
            "}",
            "",
            "#[cfg(test)]",
            f"pub(super) static ORACLE_RIVAL_POSES: [MissionEncounterPose; {len(oracle_frames)}] = [",
        ]
    )
    lines.extend(f"    {pose_source(rival_poses[frame])}," for frame in oracle_frames)
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    player_poses, rival_poses = load_poses(args.pose_fixture)
    if args.import_raw is not None:
        raw = args.import_raw.read_bytes()
        actions = semantic_actions(raw_events(args.import_raw), player_poses, rival_poses)
        compact = render_compact(actions, hashlib.sha256(raw).hexdigest())
        args.logic_fixture.parent.mkdir(parents=True, exist_ok=True)
        args.logic_fixture.write_text(compact, encoding="utf-8")
    actions = load_compact(args.logic_fixture)
    source = generate_rust(actions, player_poses, rival_poses)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated Leon rival dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        f"Leon rival semantic schedule verified through retail frame {END_RETAIL_FRAME}: "
        f"{len(rival_poses)} retained rival poses"
    )


if __name__ == "__main__":
    main()
