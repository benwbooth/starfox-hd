#!/usr/bin/env python3
"""Generate semantic Pigma rival-flight dynamics from campaign-oracle logic."""

from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = Path(__file__).with_name("fixtures") / "pigma_duel_rival_logic.trace"
DEFAULT_POSE_FIXTURE = Path(__file__).with_name("fixtures") / "pigma_duel.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "pigma_duel_rival.rs"
)

RAW_SAMPLE_START_ELAPSED = 31_604
PRESENTATION_START_RETAIL_FRAME = 36
FLIGHT_START_RETAIL_FRAME = 384
END_RETAIL_FRAME = 1_224
DEPARTURE_RETAIL_FRAME = 1_228
INITIAL_POSE = (10_219, 0, 8_126, 21, 76, 0, 0)
RETAIL_FRAME_STEP = 4

SEMANTIC_EVENTS = frozenset(
    {
        ("move", "0174"),
        ("wait-for-angle", "027E"),
        ("move", "027E"),
        ("face-player", "029E"),
        ("projectile-face-smooth", "029A"),
        ("move", "028F"),
        ("move", "0169"),
        ("indexed-byte-step", "0392"),
        ("wait-for-angle", "03C9"),
        ("move", "03C9"),
        ("move", "0427"),
        ("move", "042A"),
        ("move", "0443"),
        ("chase-word", "0404"),
        ("indexed-byte-step", "0407"),
    }
)

STEERING = {
    (40, 2, 40): "RivalApproachSteering::EntryClimb",
    (-40, -2, -40): "RivalApproachSteering::EntryDive",
    (40, -2, -40): "RivalApproachSteering::SecondClimb",
    (-40, 2, 40): "RivalApproachSteering::SecondDive",
}

# The altitude keeper is a scheduled companion behavior. These are the
# presentation boundaries at which it runs after Pigma's own final movement.
POST_MOVEMENT_ALTITUDE_HOLDS = frozenset(
    {
        604,
        *range(616, 692, RETAIL_FRAME_STEP),
        696,
        700,
        *range(708, 756, RETAIL_FRAME_STEP),
    }
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
        raise SystemExit(f"Pigma pose fixture is empty: {path}")
    return poses


def raw_events(path: Path) -> list[RawEvent]:
    result = []
    for sequence, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        values = fields(line)
        event = values.get("event")
        path_offset = values.get("path")
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
        raise SystemExit(f"raw trace has no Pigma rival logic: {path}")
    return result


def retail_frame(event: RawEvent) -> int:
    frame = (
        (event.elapsed - RAW_SAMPLE_START_ELAPSED) // RETAIL_FRAME_STEP + 1
    ) * RETAIL_FRAME_STEP
    # These two operations begin after the presentation sampler has already
    # retained its boundary, despite sharing that boundary's source interval.
    if event.event == "wait-for-angle" and event.path == "027E" and event.elapsed == 32_283:
        frame += RETAIL_FRAME_STEP
    if event.event == "face-player" and event.elapsed == 32_303:
        frame += RETAIL_FRAME_STEP
    return frame


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
    raise SystemExit(
        f"player target at elapsed {event.elapsed} is neither previous nor current"
    )


def altitude_timing(
    event: RawEvent,
    frame: int,
    player_poses: dict[int, tuple[int, ...]],
) -> str:
    target = int.from_bytes(event.extension[35:37], "little", signed=True)
    previous = player_poses[frame - RETAIL_FRAME_STEP][1]
    earlier = player_poses[frame - 2 * RETAIL_FRAME_STEP][1]
    if target == previous:
        return "PigmaPlayerAltitudeTiming::Previous"
    midpoint = earlier + int((previous - earlier) / 2)
    if target == midpoint:
        return "PigmaPlayerAltitudeTiming::EarlierMidpoint"
    if target == midpoint + 2:
        return "PigmaPlayerAltitudeTiming::EarlierMidpointWithEntryRounding"
    raise SystemExit(
        f"player altitude target {target} at elapsed {event.elapsed} has no typed timing"
    )


def semantic_actions(
    events: list[RawEvent], player_poses: dict[int, tuple[int, ...]]
) -> dict[int, list[str]]:
    scheduled: dict[int, list[tuple[int, int, str]]] = defaultdict(list)
    phase_started = set()

    def add(frame: int, event: RawEvent, order: int, action: str) -> None:
        scheduled[frame].append((event.elapsed, event.sequence * 10 + order, action))

    for event in events:
        frame = retail_frame(event)
        if not FLIGHT_START_RETAIL_FRAME <= frame <= END_RETAIL_FRAME:
            continue

        if event.pose[1] == -4_000 and event.path not in {"0174", "0392", "0404", "0407"}:
            add(frame, event, 0, "PigmaRivalAction::MaintainCombatAltitude")

        if event.event == "move" and event.path == "0174":
            steering_values = tuple(signed_byte(value) for value in event.extension[20:23])
            steering = STEERING.get(steering_values)
            if steering is None:
                raise SystemExit(f"unknown Pigma approach steering: {steering_values}")
            if "approach" not in phase_started:
                add(frame, event, 1, "PigmaRivalAction::BeginApproach")
                phase_started.add("approach")
            add(
                frame,
                event,
                2,
                f"PigmaRivalAction::AdvanceApproach({steering})",
            )
        elif event.event == "wait-for-angle" and event.path == "027E":
            if "maneuver" not in phase_started:
                add(frame, event, 1, "PigmaRivalAction::BeginCombatManeuver")
                phase_started.add("maneuver")
            add(frame, event, 2, "PigmaRivalAction::ChaseRollToLevel")
        elif event.event == "move" and event.path in {"027E", "028F", "03C9"}:
            add(frame, event, 2, "PigmaRivalAction::Advance")
        elif event.event == "face-player":
            if "attack" not in phase_started:
                add(frame, event, 1, "PigmaRivalAction::BeginAttack")
                phase_started.add("attack")
            timing = target_timing(event, frame, player_poses)
            add(
                frame,
                event,
                2,
                f"PigmaRivalAction::FacePlayerYawAndLevelPitch({timing})",
            )
        elif event.event == "projectile-face-smooth":
            timing = target_timing(event, frame, player_poses)
            add(
                frame,
                event,
                2,
                f"PigmaRivalAction::FacePlayerSmooth({timing})",
            )
        elif event.event == "move" and event.path == "0169":
            add(frame, event, 2, "PigmaRivalAction::LaunchSecondApproach")
        elif event.event == "indexed-byte-step" and event.path == "0392":
            add(frame, event, 2, "PigmaRivalAction::ApplySecondApproachWave")
        elif event.event == "wait-for-angle" and event.path == "03C9":
            if "deceleration" not in phase_started:
                add(frame, event, 1, "PigmaRivalAction::BeginDeceleration")
                phase_started.add("deceleration")
            add(frame, event, 2, "PigmaRivalAction::ChaseRollToLevel")
        elif event.event == "move" and event.path == "0427":
            if "escape" not in phase_started:
                add(frame, event, 1, "PigmaRivalAction::BeginEscape")
                phase_started.add("escape")
            add(frame, event, 2, "PigmaRivalAction::TurnAwayAndAdvance")
        elif event.event == "move" and event.path in {"042A", "0443"}:
            if event.path == "042A" and event.elapsed == 32_649:
                add(frame, event, 1, "PigmaRivalAction::TurnAway")
            add(frame, event, 2, "PigmaRivalAction::Advance")
        elif event.event == "chase-word":
            timing = altitude_timing(event, frame, player_poses)
            add(
                frame,
                event,
                3,
                f"PigmaRivalAction::ChasePlayerAltitude({timing})",
            )
        elif event.event == "indexed-byte-step" and event.path == "0407":
            add(frame, event, 4, "PigmaRivalAction::ApplyEscapeWobble")

    # The attack path returns Pigma to the authored second-pass origin between
    # its last attack movement and the next scheduled flight step.
    scheduled[884].append((32_485, 10**9 - 2, "PigmaRivalAction::BeginSecondApproach"))
    for frame in POST_MOVEMENT_ALTITUDE_HOLDS:
        scheduled[frame].append((10**9, 10**9, "PigmaRivalAction::MaintainCombatAltitude"))

    return {
        frame: [action for _, _, action in sorted(actions)]
        for frame, actions in sorted(scheduled.items())
    }


def render_compact(actions: dict[int, list[str]]) -> str:
    lines = [
        "# Semantic Pigma rival actions recovered from the campaign oracle.",
        f"# raw_sample_start_elapsed={RAW_SAMPLE_START_ELAPSED}",
        f"# presentation_start_retail_frame={PRESENTATION_START_RETAIL_FRAME}",
        f"# flight_start_retail_frame={FLIGHT_START_RETAIL_FRAME}",
        f"# end_retail_frame={END_RETAIL_FRAME}",
        f"# departure_retail_frame={DEPARTURE_RETAIL_FRAME}",
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
        raise SystemExit(f"Pigma rival logic fixture is empty: {path}")
    return dict(sorted(actions.items()))


def generate_rust(actions: dict[int, list[str]]) -> str:
    flattened = []
    ranges = []
    for frame, frame_actions in actions.items():
        ranges.append((frame, len(flattened), len(frame_actions)))
        flattened.extend(frame_actions)

    pose = ", ".join(f"{value:_}" for value in INITIAL_POSE)
    lines = [
        "//! Generated semantic rival dynamics for the retail Pigma duel.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, MissionEncounterPose, PigmaPlayerAltitudeTiming,",
        "    PigmaRivalAction, PlayerTargetTiming, RivalApproachSteering,",
        "};",
        "",
        "pub(super) const PRESENTATION_START_RETAIL_FRAME: u16 = "
        f"{PRESENTATION_START_RETAIL_FRAME:_};",
        f"pub(super) const FLIGHT_START_RETAIL_FRAME: u16 = {FLIGHT_START_RETAIL_FRAME:_};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {END_RETAIL_FRAME:_};",
        f"pub(super) const DEPARTURE_RETAIL_FRAME: u16 = {DEPARTURE_RETAIL_FRAME:_};",
        "pub(super) const INITIAL_POSE: MissionEncounterPose =",
        f"    mission_encounter_pose([{pose}]);",
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
    lines.extend(["", f"static ACTIONS: [PigmaRivalAction; {len(flattened)}] = ["])
    lines.extend(f"    {action}," for action in flattened)
    lines.extend(
        [
            "];",
            "",
            "pub(super) fn actions(retail_frame: u16) -> &'static [PigmaRivalAction] {",
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
        actions = semantic_actions(raw_events(args.import_raw), load_player_poses(args.pose_fixture))
        compact = render_compact(actions)
        args.logic_fixture.parent.mkdir(parents=True, exist_ok=True)
        args.logic_fixture.write_text(compact, encoding="utf-8")
    actions = load_compact(args.logic_fixture)
    source = generate_rust(actions)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated Pigma rival dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "Pigma rival schedule verified: "
        f"{sum(map(len, actions.values()))} semantic actions across {len(actions)} boundaries"
    )


if __name__ == "__main__":
    main()
