#!/usr/bin/env python3
"""Generate the Mirage Dragon scene fixture and head departure cadence."""

from __future__ import annotations

import argparse
import ast
import re
from dataclasses import dataclass
from pathlib import Path

from generate_capital_continuation import (
    ANGLE_SOURCE,
    TRIG_SOURCE,
    chase_power,
    rust_table,
    sf2_atan16,
    signed_word,
    xz_distance,
)
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
CAMERA_INTRO_FIRST_RETAIL_FRAME = 80
CAMERA_INTRO_LAST_RETAIL_FRAME = 336
CAMERA_FOLLOW_FIRST_RETAIL_FRAME = 340
CAMERA_FOCUS_INITIAL_POSITION = (0, 1_628, 0)
CAMERA_FOCUS_VELOCITY = (37, 0, 26)
CAMERA_INITIAL_LATERAL_OFFSET = 0
CAMERA_ACTIVE_LATERAL_OFFSET = 800
CAMERA_INITIAL_DEPTH_OFFSET = 2_000
CAMERA_INITIAL_DEPTH_MOTION = -70
CAMERA_DEPTH_MOTION_FIRST_STEP = 2
CAMERA_LATERAL_OFFSET_FIRST_STEP = 2
CAMERA_DEPTH_ACCELERATION = -10
CAMERA_DEPTH_TARGET = 30
CAMERA_DEPTH_ACCELERATION_FIRST_STEP = 13
CAMERA_DEPTH_CHASE_FIRST_STEP = 47
CAMERA_DEPTH_CHASE_DIVISOR = 8
CAMERA_DEPTH_CHASE_MINIMUM = 8
CAMERA_ANCHOR_PITCH = 20
CAMERA_ANCHOR_YAW = 64
CAMERA_ANCHOR_ROLL = 0
CAMERA_ROTATION_CHASE_FIRST_STEP = 47
CAMERA_ROTATION_CHASES_PER_STEP = 2
CAMERA_ROTATION_CHASE_DIVISIONS = 3
CAMERA_ROTATION_CHASE_MINIMUM = 8
CAMERA_ROTATION_TARGET = (0, 192 << 8, 0)
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


def fixed_rust_table(name: str, length: int, source: str) -> list[int]:
    body = re.search(
        rf"(?:pub )?(?:static|const) {name}: \[i16; {length}\] = \[(.*?)\];",
        source,
        re.S,
    )
    if body is None:
        raise SystemExit(f"could not read {name} from Rust source")
    return ast.literal_eval("[" + body.group(1) + "]")


def signed_q15(value: int) -> int:
    value &= 0xFFFF
    return value if value < 0x8000 else value - 0x10000


def q15_product(left: int, right: int) -> int:
    return signed_q15((left * right) >> 15)


def sine_q15(angle: int, quarter: list[int]) -> int:
    angle &= 0xFF
    if angle <= 0x40:
        return quarter[angle]
    if angle <= 0x7F:
        return quarter[0x80 - angle]
    if angle <= 0xC0:
        return -quarter[angle - 0x80]
    return -quarter[0x100 - angle]


def camera_anchor_offset(
    quarter: list[int],
    lateral: int,
    depth: int,
) -> tuple[int, int, int]:
    # `$7F:2229` builds the controller's Z-X-Y matrix before multiplying the
    # anchor's whole relative vector. At yaw 64 and roll zero, its three live
    # coefficients reduce to the values below. Preserve the matrix-build
    # truncation before multiplying by depth; sequentially rotating the vector
    # would round one unit differently from the retail GSU path.
    cosine_pitch = sine_q15(CAMERA_ANCHOR_PITCH + 0x40, quarter)
    sine_pitch = sine_q15(CAMERA_ANCHOR_PITCH, quarter)
    quarter_turn = sine_q15(CAMERA_ANCHOR_YAW, quarter)
    forward_x_coefficient = q15_product(cosine_pitch, quarter_turn)
    return (
        q15_product(depth, -forward_x_coefficient),
        q15_product(depth, sine_pitch),
        q15_product(lateral, quarter_turn),
    )


def chase_camera_depth_motion(current: int) -> int:
    difference = CAMERA_DEPTH_TARGET - current
    if 0 < difference < CAMERA_DEPTH_CHASE_MINIMUM:
        difference = CAMERA_DEPTH_CHASE_MINIMUM
    elif -CAMERA_DEPTH_CHASE_MINIMUM < difference < 0:
        difference = -CAMERA_DEPTH_CHASE_MINIMUM
    return current + int(difference / CAMERA_DEPTH_CHASE_DIVISOR)


@dataclass
class CameraAnchorState:
    strategy_step: int = 0
    focus_position: tuple[int, int, int] = CAMERA_FOCUS_INITIAL_POSITION
    lateral_offset: int = CAMERA_INITIAL_LATERAL_OFFSET
    depth_offset: int = CAMERA_INITIAL_DEPTH_OFFSET
    depth_motion: int = CAMERA_INITIAL_DEPTH_MOTION
    anchor_position: tuple[int, int, int] = (0, 0, 0)
    rotation_fine: tuple[int, int, int] = (0, 0, 0)

    def advance(self, quarter: list[int], curve: list[int]) -> None:
        self.strategy_step += 1
        self.focus_position = tuple(
            position + velocity
            for position, velocity in zip(
                self.focus_position,
                CAMERA_FOCUS_VELOCITY,
                strict=True,
            )
        )
        if self.strategy_step >= CAMERA_DEPTH_MOTION_FIRST_STEP:
            self.depth_offset += self.depth_motion
        if self.strategy_step == CAMERA_LATERAL_OFFSET_FIRST_STEP:
            self.lateral_offset = CAMERA_ACTIVE_LATERAL_OFFSET
        offset = camera_anchor_offset(
            quarter,
            self.lateral_offset,
            self.depth_offset,
        )
        self.anchor_position = tuple(
            focus + component
            for focus, component in zip(
                self.focus_position,
                offset,
                strict=True,
            )
        )
        if (
            CAMERA_DEPTH_ACCELERATION_FIRST_STEP
            <= self.strategy_step
            <= CAMERA_DEPTH_CHASE_FIRST_STEP
        ):
            self.depth_motion += CAMERA_DEPTH_ACCELERATION
        if self.strategy_step >= CAMERA_DEPTH_CHASE_FIRST_STEP:
            self.depth_motion = chase_camera_depth_motion(self.depth_motion)

        if self.strategy_step < CAMERA_ROTATION_CHASE_FIRST_STEP:
            self.rotation_fine = self.look_at_rotation(curve)
        else:
            # `$44:CC7E` runs opcode `$143` twice. Its helper at `$7F:25A3`
            # is a signed 16-bit rate-three proportional chase.
            for _ in range(CAMERA_ROTATION_CHASES_PER_STEP):
                self.rotation_fine = tuple(
                    chase_power(
                        current,
                        target,
                        CAMERA_ROTATION_CHASE_DIVISIONS,
                        16,
                        CAMERA_ROTATION_CHASE_MINIMUM,
                    )
                    for current, target in zip(
                        self.rotation_fine,
                        CAMERA_ROTATION_TARGET,
                        strict=True,
                    )
                )

    def look_at_rotation(self, curve: list[int]) -> tuple[int, int, int]:
        delta_x, delta_y, delta_z = (
            focus - anchor
            for focus, anchor in zip(
                self.focus_position,
                self.anchor_position,
                strict=True,
            )
        )
        return (
            (-sf2_atan16(
                delta_y,
                xz_distance(delta_x, delta_z),
                curve,
            )) & 0xFFFF,
            sf2_atan16(delta_x, delta_z, curve) & 0xFFFF,
            CAMERA_ANCHOR_ROLL,
        )

    def camera(self) -> tuple[int, ...]:
        return (
            *self.anchor_position,
            *(rotation & 0xFF for rotation in self.rotation_fine),
        )


def camera_anchor_cadence(records: list[Record]) -> list[int]:
    quarter = fixed_rust_table(
        "SINTAB16_QUARTER",
        65,
        TRIG_SOURCE.read_text(encoding="utf-8"),
    )
    curve = rust_table(
        "SF2_ARCTANGENT_CURVE",
        ANGLE_SOURCE.read_text(encoding="utf-8"),
    )
    state = CameraAnchorState()
    cadence = []
    intro_records = [
        record
        for record in records
        if CAMERA_INTRO_FIRST_RETAIL_FRAME
        <= record.retail_frame
        <= CAMERA_INTRO_LAST_RETAIL_FRAME
    ]
    expected_count = (
        CAMERA_INTRO_LAST_RETAIL_FRAME - CAMERA_INTRO_FIRST_RETAIL_FRAME
    ) // RETAIL_FRAME_STEP + 1
    if len(intro_records) != expected_count:
        raise SystemExit("Mirage Dragon fixture lacks the complete camera intro")

    for record in intro_records:
        matched_updates = None
        for updates in range(4):
            if state.strategy_step > 0 and state.camera() == record.camera:
                matched_updates = updates
                break
            state.advance(quarter, curve)
        if matched_updates is None:
            raise SystemExit(
                f"camera anchor path does not match frame {record.retail_frame}: "
                f"got {state.camera()}, expected {record.camera}"
            )
        cadence.append(matched_updates)
    return cadence


def append_camera_path(
    source: str,
    records: list[Record],
    cadence: list[int],
) -> str:
    source = source.replace(
        "pub(super) const CAMERA_KEYFRAMES:",
        "#[cfg(test)]\npub(super) const CAMERA_KEYFRAMES:",
        1,
    )
    follow_records = [
        record
        for record in records
        if record.retail_frame >= CAMERA_FOLLOW_FIRST_RETAIL_FRAME
    ]
    cadence_lines = [
        "    " + ", ".join(str(updates) for updates in cadence[index : index + 32]) + ","
        for index in range(0, len(cadence), 32)
    ]
    lines = [
        "",
        "pub(super) const CAMERA_INTRO_FIRST_RETAIL_FRAME: u16 = "
        f"{CAMERA_INTRO_FIRST_RETAIL_FRAME};",
        "pub(super) const CAMERA_INTRO_LAST_RETAIL_FRAME: u16 = "
        f"{CAMERA_INTRO_LAST_RETAIL_FRAME};",
        "pub(super) const CAMERA_FOLLOW_FIRST_RETAIL_FRAME: u16 = "
        f"{CAMERA_FOLLOW_FIRST_RETAIL_FRAME};",
        "pub(super) const CAMERA_FOCUS_INITIAL_POSITION: [i16; 3] = "
        f"{list(CAMERA_FOCUS_INITIAL_POSITION)};",
        "pub(super) const CAMERA_FOCUS_VELOCITY: [i16; 3] = "
        f"{list(CAMERA_FOCUS_VELOCITY)};",
        "pub(super) const CAMERA_INITIAL_LATERAL_OFFSET: i16 = "
        f"{CAMERA_INITIAL_LATERAL_OFFSET};",
        "pub(super) const CAMERA_ACTIVE_LATERAL_OFFSET: i16 = "
        f"{CAMERA_ACTIVE_LATERAL_OFFSET};",
        "pub(super) const CAMERA_INITIAL_DEPTH_OFFSET: i16 = "
        f"{CAMERA_INITIAL_DEPTH_OFFSET:_};",
        "pub(super) const CAMERA_INITIAL_DEPTH_MOTION: i16 = "
        f"{CAMERA_INITIAL_DEPTH_MOTION};",
        "pub(super) const CAMERA_DEPTH_MOTION_FIRST_STEP: u8 = "
        f"{CAMERA_DEPTH_MOTION_FIRST_STEP};",
        "pub(super) const CAMERA_LATERAL_OFFSET_FIRST_STEP: u8 = "
        f"{CAMERA_LATERAL_OFFSET_FIRST_STEP};",
        "pub(super) const CAMERA_DEPTH_ACCELERATION: i16 = "
        f"{CAMERA_DEPTH_ACCELERATION};",
        "pub(super) const CAMERA_DEPTH_TARGET: i16 = "
        f"{CAMERA_DEPTH_TARGET};",
        "pub(super) const CAMERA_DEPTH_ACCELERATION_FIRST_STEP: u8 = "
        f"{CAMERA_DEPTH_ACCELERATION_FIRST_STEP};",
        "pub(super) const CAMERA_DEPTH_CHASE_FIRST_STEP: u8 = "
        f"{CAMERA_DEPTH_CHASE_FIRST_STEP};",
        "pub(super) const CAMERA_DEPTH_CHASE_DIVISOR: i16 = "
        f"{CAMERA_DEPTH_CHASE_DIVISOR};",
        "pub(super) const CAMERA_DEPTH_CHASE_MINIMUM: i16 = "
        f"{CAMERA_DEPTH_CHASE_MINIMUM};",
        "pub(super) const CAMERA_ANCHOR_PITCH: u8 = "
        f"{CAMERA_ANCHOR_PITCH};",
        "pub(super) const CAMERA_ANCHOR_YAW: u8 = "
        f"{CAMERA_ANCHOR_YAW};",
        "pub(super) const CAMERA_ANCHOR_ROLL: u8 = "
        f"{CAMERA_ANCHOR_ROLL};",
        "pub(super) const CAMERA_ROTATION_CHASE_FIRST_STEP: u8 = "
        f"{CAMERA_ROTATION_CHASE_FIRST_STEP};",
        "pub(super) const CAMERA_ROTATION_CHASES_PER_STEP: u8 = "
        f"{CAMERA_ROTATION_CHASES_PER_STEP};",
        "pub(super) const CAMERA_ROTATION_CHASE_DIVISIONS: u8 = "
        f"{CAMERA_ROTATION_CHASE_DIVISIONS};",
        "pub(super) const CAMERA_ROTATION_CHASE_MINIMUM: i16 = "
        f"{CAMERA_ROTATION_CHASE_MINIMUM};",
        "pub(super) const CAMERA_ROTATION_TARGET_SUBUNITS: [u16; 3] = "
        f"{list(CAMERA_ROTATION_TARGET)};",
        f"const CAMERA_RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        "",
        f"const CAMERA_ANCHOR_CADENCE: [u8; {len(cadence)}] = [",
        *cadence_lines,
        "];",
        "#[cfg(test)]",
        "pub(super) const CAMERA_ANCHOR_TOTAL_STRATEGY_UPDATES: u8 = "
        f"{sum(cadence)};",
        "",
        "pub(super) fn camera_anchor_strategy_updates(retail_frame: u16) -> Option<u8> {",
        "    let offset = retail_frame.checked_sub(CAMERA_INTRO_FIRST_RETAIL_FRAME)?;",
        "    if retail_frame > CAMERA_INTRO_LAST_RETAIL_FRAME "
        "|| offset % CAMERA_RETAIL_FRAME_STEP != 0 {",
        "        return None;",
        "    }",
        "    CAMERA_ANCHOR_CADENCE",
        "        .get(usize::from(offset / CAMERA_RETAIL_FRAME_STEP))",
        "        .copied()",
        "}",
        "",
        "pub(super) const CAMERA_FOLLOW_KEYFRAMES: "
        f"[MissionCameraKeyframe; {len(follow_records)}] = [",
    ]
    for record in follow_records:
        values = ", ".join(f"{value:_}" for value in record.camera)
        lines.append(
            f"    mission_camera_keyframe({record.retail_frame}, {values}),"
        )
    lines.extend(["];", ""])
    return source + "\n".join(lines)


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

    camera_cadence = camera_anchor_cadence(records)
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
    scene_source = append_camera_path(scene_source, records, camera_cadence)
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
        f"{sum(camera_cadence)} camera anchor updates, "
        f"{sum(movement for movement, _ in head_cadence)} head movement updates, "
        f"{sum(pitch for _, pitch in head_cadence)} head pitch updates, "
        f"{sum(control for control, _ in player_cadence)} player control updates, "
        f"{sum(movement for _, movement in player_cadence)} player movement updates"
    )


if __name__ == "__main__":
    main()
