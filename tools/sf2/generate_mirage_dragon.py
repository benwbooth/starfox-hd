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

SINGLE_LINE_SCENE_IMPORT = (
    "    mission_camera_keyframe, mission_player_keyframe, "
    "MissionCameraKeyframe, MissionPlayerKeyframe,\n"
)
MULTILINE_SCENE_IMPORT = (
    "    mission_camera_keyframe, mission_player_keyframe, MissionCameraKeyframe,\n"
    "    MissionPlayerKeyframe,\n"
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

    cadence = head_departure_cadence(records)
    scene_source = rust_source(
        DEFAULT_TRACE.name,
        records,
        return_frame,
        map_ready_frame,
        DUEL_NAME,
        Path(__file__).name,
        rival_test_only=True,
        timing_test_only=False,
    )
    scene_source = scene_source.replace(
        MULTILINE_SCENE_IMPORT,
        SINGLE_LINE_SCENE_IMPORT,
        1,
    )
    generated = append_head_cadence(scene_source, cadence)
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
        f"{sum(movement for movement, _ in cadence)} movement updates, "
        f"{sum(pitch for _, pitch in cadence)} pitch updates"
    )


if __name__ == "__main__":
    main()
