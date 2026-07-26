#!/usr/bin/env python3
"""Generate the Mirage Dragon scene fixture and head departure cadence."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_pigma_duel import Record, load, rust_source, write_compact


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "mirage_dragon.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "mirage_dragon.rs"
)
DUEL_NAME = "Mirage Dragon"
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "E1B0"
MISSION_SELECTION = "9"
RAW_START_ELAPSED = 71_196

HEAD_PRE_DEPARTURE_RETAIL_FRAME = 564
HEAD_ENTRANCE_RETAIL_FRAME = 64
HEAD_DEPARTURE_RETAIL_FRAME = 568
HEAD_LAST_PRESENT_RETAIL_FRAME = 872
HEAD_REMOVAL_RETAIL_FRAME = 876
HEAD_DEPARTURE_VELOCITY = (124, -88, 8)
HEAD_DEPARTURE_PITCH_STEP = 30
HEAD_DEPARTURE_YAW = 196
HEAD_DEPARTURE_ROLL = 0
HEAD_DEPARTURE_SPEED = 40
RETAIL_FRAME_STEP = 4
PLAYER_CINEMATIC_END_RETAIL_FRAME = 400
PLAYER_NEUTRAL_FIRST_UPDATE_RETAIL_FRAME = 404
PLAYER_NEUTRAL_LAST_UPDATE_RETAIL_FRAME = 872
PLAYER_NEUTRAL_YAW = 66
PLAYER_NEUTRAL_TARGET_SPEED = 23
PLAYER_NEUTRAL_START_BANK_PHASE = 14
PLAYER_NEUTRAL_BANK_WAVE = (
    0,
    1,
    2,
    2,
    3,
    3,
    4,
    4,
    4,
    4,
    3,
    3,
    2,
    2,
    1,
    0,
    -1,
    -2,
    -2,
    -3,
    -3,
    -4,
    -4,
    -4,
    -4,
    -3,
    -3,
    -2,
    -2,
    -1,
)

SINGLE_LINE_SCENE_IMPORT = (
    "    mission_camera_keyframe, mission_player_keyframe, "
    "MissionCameraKeyframe, MissionPlayerKeyframe,\n"
)
MULTILINE_SCENE_IMPORT = (
    "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,\n"
    "    MissionPlayerKeyframe,\n"
)


def signed_byte(value: int) -> int:
    return value if value < 128 else value - 256


def player_neutral_flight_cadence(
    records: list[Record],
) -> list[tuple[int, int]]:
    poses = {record.retail_frame: record.player for record in records}
    previous = poses.get(PLAYER_CINEMATIC_END_RETAIL_FRAME)
    if previous is None:
        raise SystemExit("Mirage Dragon fixture lacks the player handoff pose")
    if (
        previous[3] != 0
        or previous[4] != PLAYER_NEUTRAL_YAW
        or signed_byte(previous[5])
        != PLAYER_NEUTRAL_BANK_WAVE[PLAYER_NEUTRAL_START_BANK_PHASE]
    ):
        raise SystemExit("Mirage Dragon player handoff does not match static flight state")

    phase = PLAYER_NEUTRAL_START_BANK_PHASE
    cumulative_control = 0
    cumulative_movement = 0
    cadence = []
    for retail_frame in range(
        PLAYER_NEUTRAL_FIRST_UPDATE_RETAIL_FRAME,
        PLAYER_NEUTRAL_LAST_UPDATE_RETAIL_FRAME + 1,
        RETAIL_FRAME_STEP,
    ):
        current = poses.get(retail_frame)
        if current is None:
            raise SystemExit(
                f"Mirage Dragon fixture lacks player pose at frame {retail_frame}"
            )
        if (
            current[1] != previous[1]
            or current[2] != previous[2]
            or current[3] != 0
            or current[4] != PLAYER_NEUTRAL_YAW
        ):
            raise SystemExit(
                f"player neutral flight axes changed at frame {retail_frame}"
            )

        # Static flight rotation at pitch 0 and yaw 66 produces this exact
        # horizontal velocity for the observed speed range.
        horizontal_velocity = -(current[6] - 2)
        horizontal_delta = current[0] - previous[0]
        if horizontal_velocity == 0 or horizontal_delta % horizontal_velocity != 0:
            raise SystemExit(
                f"player motion at frame {retail_frame} is not an exact flight step"
            )
        movement_updates = horizontal_delta // horizontal_velocity
        if movement_updates not in (0, 1, 2):
            raise SystemExit(
                f"player movement cadence at frame {retail_frame} is out of range"
            )

        control_candidates = []
        for control_updates in (0, 1, 2):
            speed = min(
                PLAYER_NEUTRAL_TARGET_SPEED,
                previous[6] + control_updates,
            )
            next_phase = (phase + control_updates) % len(PLAYER_NEUTRAL_BANK_WAVE)
            if (
                speed == current[6]
                and PLAYER_NEUTRAL_BANK_WAVE[next_phase]
                == signed_byte(current[5])
            ):
                control_candidates.append(control_updates)
        if not control_candidates:
            raise SystemExit(
                f"player control state at frame {retail_frame} "
                "does not match the static neutral-flight rules"
            )
        control_updates = min(
            control_candidates,
            key=lambda updates: (
                abs(
                    cumulative_control
                    + updates
                    - cumulative_movement
                    - movement_updates
                ),
                updates,
            ),
        )
        phase = (phase + control_updates) % len(PLAYER_NEUTRAL_BANK_WAVE)
        cumulative_control += control_updates
        cumulative_movement += movement_updates
        cadence.append((control_updates, movement_updates))
        previous = current

    if cumulative_control != cumulative_movement:
        raise SystemExit("player control and movement pipelines do not finish aligned")
    return cadence


def append_player_flight(
    source: str,
    records: list[Record],
    cadence: list[tuple[int, int]],
) -> str:
    cinematic_records = [
        record
        for record in records
        if record.retail_frame <= PLAYER_CINEMATIC_END_RETAIL_FRAME
    ]

    def keyframes(name: str, attribute: str) -> list[str]:
        values = [
            f"pub(super) const {name}: "
            f"[MissionPlayerKeyframe; {len(cinematic_records)}] = ["
        ]
        for record in cinematic_records:
            pose = getattr(record, attribute)
            values.append(
                f"    mission_player_keyframe({record.retail_frame}, "
                + ", ".join(f"{value:_}" for value in pose)
                + "),"
            )
        values.extend(["];", ""])
        return values

    cadence_values = []
    for control_updates, movement_updates in cadence:
        cadence_values.extend(
            [
                "    PlayerNeutralFlightCadence {",
                f"        control_updates: {control_updates},",
                f"        movement_updates: {movement_updates},",
                "    },",
            ]
        )
    bank_wave_head = ", ".join(
        str(value) for value in PLAYER_NEUTRAL_BANK_WAVE[:-2]
    )
    bank_wave_tail = ", ".join(
        str(value) for value in PLAYER_NEUTRAL_BANK_WAVE[-2:]
    )
    return source + "\n".join(
        [
            "",
            *keyframes("PLAYER_CINEMATIC_KEYFRAMES", "player"),
            *keyframes("WINGMATE_CINEMATIC_KEYFRAMES", "wingmate"),
            "pub(super) const PLAYER_NEUTRAL_START_RETAIL_FRAME: u16 = "
            f"{PLAYER_CINEMATIC_END_RETAIL_FRAME};",
            "pub(super) const PLAYER_NEUTRAL_YAW: u8 = "
            f"{PLAYER_NEUTRAL_YAW};",
            "pub(super) const PLAYER_NEUTRAL_TARGET_SPEED: u8 = "
            f"{PLAYER_NEUTRAL_TARGET_SPEED};",
            "pub(super) const PLAYER_NEUTRAL_START_BANK_PHASE: u8 = "
            f"{PLAYER_NEUTRAL_START_BANK_PHASE};",
            f"const PLAYER_RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
            "const PLAYER_NEUTRAL_FIRST_UPDATE_RETAIL_FRAME: u16 = "
            f"{PLAYER_NEUTRAL_FIRST_UPDATE_RETAIL_FRAME};",
            "const PLAYER_NEUTRAL_LAST_UPDATE_RETAIL_FRAME: u16 = "
            f"{PLAYER_NEUTRAL_LAST_UPDATE_RETAIL_FRAME};",
            "const PLAYER_NEUTRAL_BANK_PERIOD: u8 = "
            f"{len(PLAYER_NEUTRAL_BANK_WAVE)};",
            "const PLAYER_NEUTRAL_BANK_WAVE: "
            f"[i8; {len(PLAYER_NEUTRAL_BANK_WAVE)}] = [",
            f"    {bank_wave_head},",
            f"    {bank_wave_tail},",
            "];",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "pub(super) struct PlayerNeutralFlightCadence {",
            "    pub control_updates: u8,",
            "    pub movement_updates: u8,",
            "}",
            "",
            "const PLAYER_NEUTRAL_FLIGHT_CADENCE: "
            f"[PlayerNeutralFlightCadence; {len(cadence)}] = [",
            *cadence_values,
            "];",
            "",
            "pub(super) fn player_neutral_flight_cadence(",
            "    retail_frame: u16,",
            ") -> Option<PlayerNeutralFlightCadence> {",
            "    let offset = "
            "retail_frame.checked_sub(PLAYER_NEUTRAL_FIRST_UPDATE_RETAIL_FRAME)?;",
            "    if retail_frame > PLAYER_NEUTRAL_LAST_UPDATE_RETAIL_FRAME",
            "        || offset % PLAYER_RETAIL_FRAME_STEP != 0",
            "    {",
            "        return None;",
            "    }",
            "    PLAYER_NEUTRAL_FLIGHT_CADENCE",
            "        .get(usize::from(offset / PLAYER_RETAIL_FRAME_STEP))",
            "        .copied()",
            "}",
            "",
            "pub(super) fn advance_player_neutral_bank_phase(phase: u8, updates: u8) -> u8 {",
            "    phase.wrapping_add(updates) % PLAYER_NEUTRAL_BANK_PERIOD",
            "}",
            "",
            "pub(super) fn player_neutral_bank(phase: u8) -> i8 {",
            "    PLAYER_NEUTRAL_BANK_WAVE[usize::from(phase % PLAYER_NEUTRAL_BANK_PERIOD)]",
            "}",
            "",
        ]
    )


def head_departure_cadence(records: list[Record]) -> list[tuple[int, int]]:
    poses = {
        record.retail_frame: record.rival
        for record in records
        if record.rival is not None
    }
    previous = poses.get(HEAD_PRE_DEPARTURE_RETAIL_FRAME)
    if previous is None:
        raise SystemExit("Mirage Dragon fixture lacks its pre-departure head pose")

    cadence = []
    for retail_frame in range(
        HEAD_DEPARTURE_RETAIL_FRAME,
        HEAD_LAST_PRESENT_RETAIL_FRAME + 1,
        RETAIL_FRAME_STEP,
    ):
        current = poses.get(retail_frame)
        if current is None:
            raise SystemExit(
                f"Mirage Dragon fixture lacks head pose at frame {retail_frame}"
            )
        deltas = tuple(
            current_value - previous_value
            for previous_value, current_value in zip(
                previous[:3], current[:3], strict=True
            )
        )
        movement_updates = deltas[0] // HEAD_DEPARTURE_VELOCITY[0]
        if movement_updates not in (1, 2) or deltas != tuple(
            component * movement_updates for component in HEAD_DEPARTURE_VELOCITY
        ):
            raise SystemExit(
                f"head departure motion at frame {retail_frame} "
                "does not match its static velocity"
            )
        if retail_frame == HEAD_DEPARTURE_RETAIL_FRAME:
            pitch_updates = 1
        else:
            pitch_delta = (current[3] - previous[3]) % 256
            if pitch_delta % HEAD_DEPARTURE_PITCH_STEP != 0:
                raise SystemExit(
                    f"head departure pitch at frame {retail_frame} "
                    "does not match its static turn step"
                )
            pitch_updates = pitch_delta // HEAD_DEPARTURE_PITCH_STEP
            if pitch_updates not in (1, 2):
                raise SystemExit(
                    f"head departure pitch cadence at frame {retail_frame} "
                    "is outside the retained strategy cadence"
                )
        if (
            current[4] != HEAD_DEPARTURE_YAW
            or current[5] != HEAD_DEPARTURE_ROLL
            or current[6] != HEAD_DEPARTURE_SPEED
        ):
            raise SystemExit(
                f"head departure state at frame {retail_frame} "
                "does not match the retail hold path"
            )
        cadence.append((movement_updates, pitch_updates))
        previous = current
    return cadence


def append_head_cadence(source: str, cadence: list[tuple[int, int]]) -> str:
    values = []
    for movement_updates, pitch_updates in cadence:
        values.extend(
            [
                "    HeadDepartureCadence {",
                f"        movement_updates: {movement_updates},",
                f"        pitch_updates: {pitch_updates},",
                "    },",
            ]
        )
    return source + "\n".join(
        [
            "",
            "pub(super) const HEAD_ENTRANCE_RETAIL_FRAME: u16 = "
            f"{HEAD_ENTRANCE_RETAIL_FRAME};",
            "pub(super) const HEAD_DEPARTURE_RETAIL_FRAME: u16 = "
            f"{HEAD_DEPARTURE_RETAIL_FRAME};",
            "pub(super) const HEAD_LAST_PRESENT_RETAIL_FRAME: u16 = "
            f"{HEAD_LAST_PRESENT_RETAIL_FRAME};",
            "pub(super) const HEAD_REMOVAL_RETAIL_FRAME: u16 = "
            f"{HEAD_REMOVAL_RETAIL_FRAME};",
            "#[cfg(test)]",
            "pub(super) const HEAD_DEPARTURE_MOVEMENT_UPDATES: u16 = "
            f"{sum(movement for movement, _ in cadence)};",
            "#[cfg(test)]",
            "pub(super) const HEAD_DEPARTURE_PITCH_UPDATES: u16 = "
            f"{sum(pitch for _, pitch in cadence)};",
            f"const HEAD_RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
            "",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            "pub(super) struct HeadDepartureCadence {",
            "    pub movement_updates: u8,",
            "    pub pitch_updates: u8,",
            "}",
            "",
            "const HEAD_DEPARTURE_CADENCE: "
            f"[HeadDepartureCadence; {len(cadence)}] = [",
            *values,
            "];",
            "",
            "pub(super) fn head_departure_cadence(retail_frame: u16) "
            "-> Option<HeadDepartureCadence> {",
            "    let offset = retail_frame.checked_sub(HEAD_DEPARTURE_RETAIL_FRAME)?;",
            "    if retail_frame > HEAD_LAST_PRESENT_RETAIL_FRAME "
            "|| offset % HEAD_RETAIL_FRAME_STEP != 0 {",
            "        return None;",
            "    }",
            "    HEAD_DEPARTURE_CADENCE",
            "        .get(usize::from(offset / HEAD_RETAIL_FRAME_STEP))",
            "        .copied()",
            "}",
            "",
        ]
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--import-raw",
        type=Path,
        help="rebuild the compact fixture from the accepted raw Mesen trace",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source_trace = args.import_raw or DEFAULT_TRACE
    records, return_frame, map_ready_frame = load(
        (source_trace,),
        frozenset(),
        DUEL_NAME,
        RIVAL_SOURCE_ID,
        RIVAL_SHAPE_TOKEN,
        MISSION_SELECTION,
        RAW_START_ELAPSED if args.import_raw is not None else None,
    )
    if args.import_raw is not None:
        write_compact(
            (source_trace,),
            DEFAULT_TRACE,
            records,
            return_frame,
            map_ready_frame,
            DUEL_NAME,
        )

    head_cadence = head_departure_cadence(records)
    player_cadence = player_neutral_flight_cadence(records)
    scene_source = rust_source(
        DEFAULT_TRACE.name,
        records,
        return_frame,
        map_ready_frame,
        DUEL_NAME,
        Path(__file__).name,
        rival_test_only=True,
        timing_test_only=False,
        player_test_only=True,
        omit_wingmate=True,
    )
    scene_source = scene_source.replace(
        MULTILINE_SCENE_IMPORT,
        SINGLE_LINE_SCENE_IMPORT,
        1,
    )
    scene_source = append_player_flight(scene_source, records, player_cadence)
    generated = append_head_cadence(scene_source, head_cadence)
    if args.check:
        if not DEFAULT_OUTPUT.is_file() or DEFAULT_OUTPUT.read_text(
            encoding="utf-8"
        ) != generated:
            raise SystemExit(f"generated source is out of date: {DEFAULT_OUTPUT}")
        action = "verified"
    else:
        DEFAULT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        DEFAULT_OUTPUT.write_text(generated, encoding="utf-8")
        action = "generated"

    print(
        f"{action} {DEFAULT_OUTPUT}: {len(records)} scene frames, "
        f"{sum(movement for movement, _ in head_cadence)} head movement updates, "
        f"{sum(pitch for _, pitch in head_cadence)} head pitch updates, "
        f"{sum(control for control, _ in player_cadence)} player control updates, "
        f"{sum(movement for _, movement in player_cadence)} player movement updates"
    )


if __name__ == "__main__":
    main()
