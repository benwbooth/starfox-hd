#!/usr/bin/env python3
"""Generate semantic Leon rival-flight dynamics from campaign-oracle logic."""

from __future__ import annotations

import argparse
import hashlib
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = Path(__file__).with_name("fixtures") / "leon_duel_rival_logic.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "leon_duel_rival.rs"
)

RAW_SAMPLE_START_ELAPSED = 63_320
PRESENTATION_START_RETAIL_FRAME = 52
FLIGHT_START_RETAIL_FRAME = 400
END_RETAIL_FRAME = 660
DEPARTURE_RETAIL_FRAME = 664
INITIAL_POSE = (10_139, 0, 8_138, 23, 78, 5, 1)
RETAIL_FRAME_STEP = 4
PARTIAL_APPROACH_ELAPSED = 63_915
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "C348"

SEMANTIC_EVENTS = frozenset(
    {
        ("move", "0174"),
        ("wait-for-angle", "027E"),
        ("move", "027E"),
    }
)

STEERING = {
    (40, 2, 40): "RivalApproachSteering::EntryClimb",
    (-40, -2, -40): "RivalApproachSteering::EntryDive",
}

POST_MOVEMENT_ALTITUDE_HOLDS = frozenset(
    range(616, DEPARTURE_RETAIL_FRAME, RETAIL_FRAME_STEP)
)


@dataclass(frozen=True)
class RawEvent:
    sequence: int
    elapsed: int
    event: str
    path: str
    pose: tuple[int, ...]
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


def semantic_actions(events: list[RawEvent]) -> dict[int, list[str]]:
    scheduled: dict[int, list[tuple[int, int, str]]] = defaultdict(list)
    phase_started = set()

    def add(frame: int, event: RawEvent, order: int, action: str) -> None:
        scheduled[frame].append((event.elapsed, event.sequence * 10 + order, action))

    for event in events:
        frame = retail_frame(event)
        if not FLIGHT_START_RETAIL_FRAME <= frame <= END_RETAIL_FRAME:
            continue

        if event.pose[1] == -4_000 and event.path != "0174":
            add(frame, event, 0, "LeonRivalAction::MaintainCombatAltitude")

        if event.event == "move" and event.path == "0174":
            steering_values = tuple(signed_byte(value) for value in event.extension[20:23])
            steering = STEERING.get(steering_values)
            if steering is None:
                raise SystemExit(f"unknown Leon approach steering: {steering_values}")
            if "approach" not in phase_started:
                add(frame, event, 1, "LeonRivalAction::BeginApproach")
                phase_started.add("approach")
            # The accepted first-flight pose already contains this operation's
            # angle and speed result. Subsequent operations advance from it.
            if event.elapsed == PARTIAL_APPROACH_ELAPSED:
                add(
                    frame,
                    event,
                    2,
                    f"LeonRivalAction::PrepareApproachAdvance({steering})",
                )
                add(
                    frame + RETAIL_FRAME_STEP,
                    event,
                    3,
                    "LeonRivalAction::FinishPreparedApproachAdvance",
                )
            elif frame != FLIGHT_START_RETAIL_FRAME:
                add(frame, event, 2, f"LeonRivalAction::AdvanceApproach({steering})")
        elif event.event == "wait-for-angle" and event.path == "027E":
            if "maneuver" not in phase_started:
                add(frame, event, 1, "LeonRivalAction::BeginCombatManeuver")
                phase_started.add("maneuver")
            add(frame, event, 2, "LeonRivalAction::ChaseRollToLevel")
        elif event.event == "move" and event.path == "027E":
            add(frame, event, 2, "LeonRivalAction::Advance")

    for frame in POST_MOVEMENT_ALTITUDE_HOLDS:
        scheduled[frame].append(
            (10**9, 10**9, "LeonRivalAction::MaintainCombatAltitude")
        )

    return {
        frame: [action for _, _, action in sorted(actions)]
        for frame, actions in sorted(scheduled.items())
    }


def render_compact(actions: dict[int, list[str]], raw_sha256: str) -> str:
    lines = [
        "# Semantic Leon rival actions recovered from the campaign oracle.",
        f"# Raw source SHA-256: {raw_sha256}",
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
        raise SystemExit(f"Leon rival logic fixture is empty: {path}")
    return dict(sorted(actions.items()))


def generate_rust(actions: dict[int, list[str]]) -> str:
    flattened = []
    ranges = []
    for frame, frame_actions in actions.items():
        ranges.append((frame, len(flattened), len(frame_actions)))
        flattened.extend(frame_actions)

    pose = ", ".join(f"{value:_}" for value in INITIAL_POSE)
    lines = [
        "//! Generated semantic rival dynamics for the retail Leon duel.",
        "//! Source addresses and opaque machine state remain in oracle tooling.",
        "",
        "use super::{",
        "    mission_encounter_pose, LeonRivalAction, MissionEncounterPose,",
        "    RivalApproachSteering,",
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
        ]
    )
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        raw = args.import_raw.read_bytes()
        actions = semantic_actions(raw_events(args.import_raw))
        compact = render_compact(actions, hashlib.sha256(raw).hexdigest())
        args.logic_fixture.parent.mkdir(parents=True, exist_ok=True)
        args.logic_fixture.write_text(compact, encoding="utf-8")
    actions = load_compact(args.logic_fixture)
    source = generate_rust(actions)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated Leon rival dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "Leon rival schedule verified: "
        f"{sum(map(len, actions.values()))} semantic actions across {len(actions)} boundaries"
    )


if __name__ == "__main__":
    main()
