#!/usr/bin/env python3
"""Generate typed post-frame-900 opening-sortie keyframes from an oracle trace.

Camera, player, and the four encounter poses are sampled from the same elapsed
presentation frame so the generated native scene stays coherent. Source object
tokens are used only to classify oracle rows and are never emitted into Rust.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import re
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ACTIVE_TRACE = Path(__file__).with_name("fixtures") / "first_sortie_neutral.trace"
DEFAULT_TIMER_TRACE = Path(__file__).with_name("fixtures") / "first_sortie_timer.trace"
DEFAULT_PLAYER_DYNAMICS = (
    Path(__file__).with_name("fixtures") / "first_sortie_player_dynamics.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "opening_continuation.rs"
)
ANCHOR_RETAIL_FRAME = 900
# The native SF2 game advances one simulation tick for every four retail
# presentation frames.  Sampling that exact cadence keeps every generated
# checkpoint oracle-observed; a ten-frame table required interpolation through
# states the retail game never actually held.
RETAIL_FRAME_STEP = 4
CAMERA_FOLLOW_LAST_RETAIL_FRAME = 7_840
CAMERA_RETURN_FIRST_RETAIL_FRAME = (
    CAMERA_FOLLOW_LAST_RETAIL_FRAME + RETAIL_FRAME_STEP
)
CAMERA_TYPED_LAST_RETAIL_FRAME = 8_052
CAMERA_FOLLOW_REAR_DISTANCE = 0
CAMERA_FOLLOW_VERTICAL_OFFSET = -20
CAMERA_RETURN_REAR_DISTANCE_TARGET = -240
CAMERA_RETURN_REAR_DISTANCE_STEP = 30
CAMERA_RETURN_VERTICAL_CHASE_DIVISOR = 8
CAMERA_RETURN_ORBIT_INITIAL_DEPTH = -80
CAMERA_RETURN_ORBIT_DEPTH_STEP = 5
CAMERA_RETURN_ORBIT_VERTICAL_OFFSET = -50
CAMERA_RETURN_ORBIT_YAW_STEP = 1
CAMERA_RETURN_LEAD_TARGET = 70
CAMERA_RETURN_LEAD_SETTLE_UPDATES = 5
CAMERA_RETURN_LEAD_DECAY_DIVISOR = 16
CAMERA_RETURN_CONTINUITY_DIVISOR = 16
CAMERA_RETURN_ORIENTATION_CHASE_DIVISOR = 2
CAMERA_RETURN_ORIENTATION_CHASE_MINIMUM = 2
CAMERA_RETURN_ANGULAR_VELOCITY_STEP = 1
CAMERA_ORIENTATION_COARSE_SHIFT = 8
CAMERA_ORIENTATION_SUBUNITS_PER_COARSE_UNIT = 256
CAMERA_FOLLOW_FINE_ORIENTATION = (0, 7_424, 0)
CAMERA_FOLLOW_POSITION_SCALE = 2
CAMERA_FOLLOW_POSITION_SCALE_SHIFT = 1
CAMERA_AMBIENT_HEIGHT_PHASE_AT_ANCHOR = 4
CAMERA_AMBIENT_HEIGHT_AT_ANCHOR = 1
CAMERA_AMBIENT_HEIGHT_WAVE = (
    1,
    0,
    1,
    0,
    0,
    1,
    0,
    0,
    0,
    0,
    -1,
    0,
    0,
    -1,
    0,
    -1,
    -1,
    0,
    -1,
    0,
    0,
    -1,
    0,
    0,
    0,
    0,
    1,
    0,
    0,
    1,
    0,
    1,
)
PLAYER_NEUTRAL_VELOCITY = (18, 0, 21)
ANCHOR_ENCOUNTER = (
    (-21_676, 9_847, -2_152, 0, 221, 0, 60),
    (-14_884, 7_640, -14_224, 0, 198, 11, 60),
    (-7_420, 6_421, 856, 221, 228, 248, 63),
    (-7_220, -6_395, -4_076, 35, 36, 252, 63),
)
ENCOUNTER_SHAPE_TOKENS = {"F5EC", "EA00"}
ENCOUNTER_SOURCE_IDS = ("0633", "05F4", "05B5", "0576")
ENCOUNTER_SHAPES_BY_SOURCE = {
    "0633": "F5EC",
    "05F4": "F5EC",
    "05B5": "EA00",
    "0576": "EA00",
}
ENCOUNTER_CONSTANT_NAMES = (
    "FIRST_CAPITAL_MISSION_KEYFRAMES",
    "SECOND_CAPITAL_MISSION_KEYFRAMES",
    "UPPER_FIGHTER_MISSION_KEYFRAMES",
    "LOWER_FIGHTER_MISSION_KEYFRAMES",
)
PROJECTILE_SHAPE_TOKEN = "E3A8"
PLAYER_AMBIENT_BANK_PHASE_AT_ANCHOR = 12
PLAYER_AMBIENT_BANK_WAVE = (
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
PLAYER_HIT_BANK_RECOVERY_DIVISOR = 8
PLAYER_HIT_CAMERA_RECOIL = 128
PLAYER_HIT_CAMERA_RECOIL_STEP = 16
PLAYER_HIT_CAMERA_RECOIL_SCALE = 2
TRIG_SOURCE = REPO_ROOT / "rust" / "sf-core" / "src" / "snes_trig.rs"
ANGLE_SOURCE = REPO_ROOT / "rust" / "sf-core" / "src" / "aim_angle.rs"


@dataclass(frozen=True)
class Record:
    elapsed: int
    mode: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    encounter: tuple[tuple[int, ...] | None, ...]
    projectiles: tuple[tuple[str, tuple[int, ...]], ...]


@dataclass(frozen=True)
class FlightCadence:
    retail_frame: int
    control_updates: int
    movement_updates: int
    damage_bank_impulse: int
    camera_updates: int
    camera_uses_previous_player_position: bool


def field(line: str, start: str, end: str) -> str:
    return line.split(start, 1)[1].split(end, 1)[0]


def parse_record(line: str) -> Record | None:
    if not line.startswith("elapsed="):
        return None
    objects = field(line, " active=[", "] object=").split(";")
    encounter_by_source = {}
    projectiles = []
    for raw in objects:
        parts = raw.split(",")
        if (
            len(parts) >= 9
            and parts[0] in ENCOUNTER_SOURCE_IDS
            and parts[1] in ENCOUNTER_SHAPE_TOKENS
        ):
            encounter_by_source[parts[0]] = tuple(map(int, parts[2:9]))
        if len(parts) >= 9 and parts[1] == PROJECTILE_SHAPE_TOKEN:
            projectiles.append((parts[0], tuple(map(int, parts[2:9]))))
    camera = tuple(map(int, field(line, " camera=", " pose=").split(",")))
    player = tuple(map(int, field(line, " pose=", " wingpose=").split(",")))
    return Record(
        elapsed=int(line.split(" ", 1)[0].split("=", 1)[1]),
        mode=int(field(line, " mode=", " phase=")),
        camera=camera,
        player=player,
        encounter=tuple(encounter_by_source.get(source) for source in ENCOUNTER_SOURCE_IDS),
        projectiles=tuple(projectiles),
    )


def continuation(trace: Path) -> tuple[list[tuple[int, Record]], int]:
    records = {}
    anchor_elapsed = None
    for line in trace.read_text(encoding="utf-8").splitlines():
        record = parse_record(line)
        if record is None:
            continue
        records[record.elapsed] = record
        if anchor_elapsed is None and record.encounter == ANCHOR_ENCOUNTER:
            anchor_elapsed = record.elapsed
    if anchor_elapsed is None:
        raise SystemExit("trace does not contain the certified frame-900 anchor")

    result = []
    retail_frame = ANCHOR_RETAIL_FRAME
    elapsed = anchor_elapsed
    while (record := records.get(elapsed)) and record.mode == 1:
        result.append((retail_frame, record))
        retail_frame += RETAIL_FRAME_STEP
        elapsed += RETAIL_FRAME_STEP
    if len(result) < 2:
        raise SystemExit("trace ends before the first continuation keyframe")
    return result, anchor_elapsed


def mission_timer_keyframes(
    trace: Path, anchor_elapsed: int, certified_end: int
) -> list[tuple[int, int]]:
    result = []
    previous_value = None
    for line in trace.read_text(encoding="utf-8").splitlines():
        if " timer=" not in line:
            continue
        elapsed = int(line.split(" ", 1)[0].split("=", 1)[1])
        retail_frame = ANCHOR_RETAIL_FRAME + elapsed - anchor_elapsed
        if retail_frame < 0 or retail_frame > certified_end:
            continue
        if retail_frame % RETAIL_FRAME_STEP != 0:
            continue
        whole, fractional_steps, _ = map(
            int, field(line, " timer=", " selected=").split(",")
        )
        value = whole * 10 + min(fractional_steps // 24, 9)
        if value != previous_value:
            result.append((retail_frame, value))
            previous_value = value
    if not result:
        raise SystemExit("timer trace has no native mission-timer checkpoints")
    return result


def player_cadence(trace: Path) -> list[FlightCadence]:
    result = []
    pending_controls = 0
    for line in trace.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = {
            token.split("=", 1)[0]: token.split("=", 1)[1]
            for token in line.split()
            if "=" in token
        }
        cadence = FlightCadence(
            retail_frame=int(values["retail_frame"]),
            control_updates=int(values["control_updates"]),
            movement_updates=int(values["movement_updates"]),
            damage_bank_impulse=int(values["damage_bank_impulse"]),
            camera_updates=int(values["camera_updates"]),
            camera_uses_previous_player_position=bool(
                int(values["camera_uses_previous_player_position"])
            ),
        )
        if cadence.control_updates not in (0, 1, 2):
            raise SystemExit("player dynamics contain an invalid control count")
        if cadence.movement_updates not in (0, 1, 2):
            raise SystemExit("player dynamics contain an invalid movement count")
        if cadence.camera_updates not in (0, 1, 2):
            raise SystemExit("player dynamics contain an invalid camera count")
        if (
            cadence.camera_uses_previous_player_position
            and cadence.camera_updates == 0
        ):
            raise SystemExit("camera lag requires a camera update")
        pending_controls += cadence.control_updates - cadence.movement_updates
        if pending_controls not in (0, 1):
            raise SystemExit("player dynamics violate control/movement ordering")
        result.append(cadence)
    if not result or pending_controls != 0:
        raise SystemExit("player dynamics are incomplete")
    return result


def signed_word(value: int) -> int:
    value &= 65_535
    return value - 65_536 if value >= 32_768 else value


def signed_byte(value: int) -> int:
    value &= 255
    return value - 256 if value >= 128 else value


def rust_table(name: str, source: Path) -> list[int]:
    body = re.search(
        rf"(?:pub )?(?:static|const) {name}: \[[iu]\d+; 256\] = \[(.*?)\];",
        source.read_text(encoding="utf-8"),
        re.S,
    )
    if body is None:
        raise SystemExit(f"could not read {name} from {source}")
    return ast.literal_eval("[" + body.group(1) + "]")


def trunc_div(value: int, divisor: int) -> int:
    return int(value / divisor)


def approach_power(
    current: int, target: int, divisions: int, minimum: int
) -> int:
    if current == target:
        return current
    difference = signed_word(target - current)
    if 0 < difference < minimum:
        difference = minimum
    elif -minimum < difference < 0:
        difference = -minimum
    for _ in range(divisions):
        difference = trunc_div(difference, 2)
    return signed_word(current + difference)


def approach_step(current: int, target: int, step: int) -> int:
    if current < target:
        return min(current + step, target)
    if current > target:
        return max(current - step, target)
    return current


def mulslog16(value: int, factor: int) -> int:
    value = signed_word(value)
    factor = signed_byte(factor)
    fraction = (abs(factor) << 1) & 255
    magnitude = (abs(value) * fraction) >> 8
    return -magnitude if (value < 0) != (factor < 0) else magnitude


def rotate_xz(
    angle: int, x: int, z: int, sine: list[int], cosine: list[int]
) -> tuple[int, int]:
    x = signed_word(x)
    z = signed_word(z)
    sine_value = sine[angle & 255]
    cosine_value = cosine[angle & 255]
    return (
        signed_word(
            mulslog16(x, cosine_value) - mulslog16(z, sine_value)
        ),
        signed_word(
            mulslog16(x, sine_value) + mulslog16(z, cosine_value)
        ),
    )


def rotate_yz(
    angle: int, y: int, z: int, sine: list[int], cosine: list[int]
) -> tuple[int, int]:
    return rotate_xz((-angle) & 255, y, z, sine, cosine)


def follow_camera_position(
    player: tuple[int, ...],
    vertical_offset: int,
    rear_distance: int,
    ambient_height: int,
    sine: list[int],
    cosine: list[int],
) -> tuple[int, int, int]:
    pitch = (-(CAMERA_FOLLOW_FINE_ORIENTATION[0] >> 8)) & 255
    yaw = (-(CAMERA_FOLLOW_FINE_ORIENTATION[1] >> 8)) & 255
    offset_y, depth = rotate_yz(
        pitch,
        vertical_offset * CAMERA_FOLLOW_POSITION_SCALE,
        rear_distance,
        sine,
        cosine,
    )
    offset_x, offset_z = rotate_xz(yaw, 0, depth, sine, cosine)
    return (
        signed_word(player[0] + (offset_x >> CAMERA_FOLLOW_POSITION_SCALE_SHIFT)),
        signed_word(
            player[1]
            + (offset_y >> CAMERA_FOLLOW_POSITION_SCALE_SHIFT)
            + ambient_height
        ),
        signed_word(player[2] + (offset_z >> CAMERA_FOLLOW_POSITION_SCALE_SHIFT)),
    )


def sf2_atan16(x: int, y: int, curve: list[int]) -> int:
    original_x = x & 65_535
    original_y = y & 65_535

    def absolute(value: int) -> int:
        return value if signed_word(value) >= 0 else (-value) & 65_535

    if original_y == 0:
        angle = 16_384
    else:
        numerator = absolute(original_x)
        denominator = absolute(original_y)
        if numerator == denominator:
            angle = 8_192
        else:
            swapped = signed_word(numerator - denominator) >= 0
            if swapped:
                numerator, denominator = denominator, numerator
            ratio = (
                32_767
                if denominator == 0
                else (numerator << 14) // denominator
            )
            sample = curve[((ratio >> 5) & 65_534) // 2]
            angle = (16_384 - sample) & 65_535 if swapped else sample
    if signed_word(original_x ^ original_y) < 0:
        angle = (-angle) & 65_535
    if signed_word(original_y) < 0:
        angle = (angle + 32_768) & 65_535
    return angle


def sf2_xz_angle_distance(delta_x: int, delta_z: int) -> int:
    def absolute(value: int) -> int:
        return (-value) & 65_535 if signed_word(value) < 0 else value & 65_535

    def signed_half(value: int) -> int:
        return (signed_word(value) >> 1) & 65_535

    x = signed_half(absolute(delta_x))
    z = signed_half(absolute(delta_z))
    total_range = ((z + x) & 65_535) << 1 & 65_535
    maximum = x if signed_word(z - x) < 0 else z
    total = (maximum + total_range) & 65_535
    value = (signed_half(total) + total) & 65_535
    return signed_word(signed_half(signed_half(value)))


def chase_orientation(current: int, target: int) -> int:
    return approach_power(
        current,
        target,
        divisions=1,
        minimum=CAMERA_RETURN_ORIENTATION_CHASE_MINIMUM,
    )


def recover_damage_bank(value: int) -> int:
    if value == 0:
        return 0
    adjusted_difference = max(abs(value), PLAYER_HIT_BANK_RECOVERY_DIVISOR)
    signed_difference = -adjusted_difference if value > 0 else adjusted_difference
    return value + int(signed_difference / PLAYER_HIT_BANK_RECOVERY_DIVISOR)


def verify_live_player(
    keyframes: list[tuple[int, Record]], cadence: list[FlightCadence]
) -> None:
    if [frame for frame, _ in keyframes] != [
        entry.retail_frame for entry in cadence
    ]:
        raise SystemExit("player dynamics do not span the opening keyframes")

    anchor = keyframes[0][1].player
    position = list(anchor[:3])
    pitch, yaw, roll, speed = anchor[3:]
    ambient_phase = PLAYER_AMBIENT_BANK_PHASE_AT_ANCHOR
    damage_bank_impulse = 0
    damage_bank_fresh = False
    pending_controls = 0
    for (retail_frame, record), entry in zip(keyframes, cadence):
        if entry.damage_bank_impulse != 0:
            damage_bank_impulse = entry.damage_bank_impulse
            damage_bank_fresh = True
        for _ in range(entry.control_updates):
            ambient_phase = (ambient_phase + 1) % len(PLAYER_AMBIENT_BANK_WAVE)
            if damage_bank_fresh:
                damage_bank_fresh = False
            else:
                damage_bank_impulse = recover_damage_bank(damage_bank_impulse)
            roll = (
                PLAYER_AMBIENT_BANK_WAVE[ambient_phase] + damage_bank_impulse
            ) & 255
        for _ in range(entry.movement_updates):
            position[0] = signed_word(position[0] + 18)
            position[2] = signed_word(position[2] + 21)
        pending_controls += entry.control_updates - entry.movement_updates
        expected = (*position, pitch, yaw, roll, speed)
        if expected != record.player:
            raise SystemExit(
                f"typed player rules diverge at retail frame {retail_frame}: "
                f"expected {record.player}, recovered {expected}"
            )
    if pending_controls != 0:
        raise SystemExit("player dynamics end with unmatched control motion")


def verify_live_camera(
    keyframes: list[tuple[int, Record]], cadence: list[FlightCadence]
) -> None:
    typed_pairs = list(zip(keyframes, cadence))
    if not typed_pairs or typed_pairs[-1][0][0] != CAMERA_TYPED_LAST_RETAIL_FRAME:
        raise SystemExit("camera dynamics do not span the typed follow window")

    sine = rust_table("SINTAB", TRIG_SOURCE)
    cosine = rust_table("COSTAB", TRIG_SOURCE)
    angle_curve = rust_table("SF2_ARCTANGENT_CURVE", ANGLE_SOURCE)
    initial_record = typed_pairs[0][0][1]
    ambient_phase = CAMERA_AMBIENT_HEIGHT_PHASE_AT_ANCHOR
    ambient_height = CAMERA_AMBIENT_HEIGHT_AT_ANCHOR
    camera = initial_record.camera
    vertical_offset = CAMERA_FOLLOW_VERTICAL_OFFSET
    rear_distance = CAMERA_FOLLOW_REAR_DISTANCE
    previous_output_position = camera[:3]
    previous_output_orientation = list(CAMERA_FOLLOW_FINE_ORIENTATION)
    continuity_translation = [0, 0, 0]
    angular_velocity = [0, 0, 0]
    translation_reference_yaw = 0
    return_started = False
    orbit_started = False
    orbit_depth = CAMERA_RETURN_ORBIT_INITIAL_DEPTH
    orbit_yaw = 0
    lead_depth = 0
    lead_settle_updates = 0
    damage_camera_recoil = 0
    expected_initial_position = (
        initial_record.player[0],
        initial_record.player[1]
        + CAMERA_FOLLOW_VERTICAL_OFFSET
        + CAMERA_AMBIENT_HEIGHT_AT_ANCHOR,
        initial_record.player[2],
    )
    if camera[:3] != expected_initial_position or camera[3:] != (0, 0, 0):
        raise SystemExit("typed camera handoff does not match the retail anchor")

    for (retail_frame, record), entry in typed_pairs[1:]:
        if entry.damage_bank_impulse != 0 and damage_camera_recoil == 0:
            damage_camera_recoil = PLAYER_HIT_CAMERA_RECOIL
        if entry.camera_updates == 2:
            camera_players = [
                tuple(
                    signed_word(position - velocity)
                    for position, velocity in zip(
                        record.player[:3],
                        PLAYER_NEUTRAL_VELOCITY,
                    )
                ),
                record.player[:3],
            ]
        elif entry.camera_updates == 1:
            camera_players = [record.player[:3]]
            if entry.camera_uses_previous_player_position:
                camera_players[0] = tuple(
                    signed_word(position - velocity)
                    for position, velocity in zip(
                        camera_players[0],
                        PLAYER_NEUTRAL_VELOCITY,
                    )
                )
        else:
            camera_players = []

        for update_index, camera_player in enumerate(camera_players):
            damage_camera_recoil = approach_step(
                -damage_camera_recoil,
                0,
                PLAYER_HIT_CAMERA_RECOIL_STEP,
            )
            if (
                retail_frame == CAMERA_RETURN_FIRST_RETAIL_FRAME
                and update_index + 1 == len(camera_players)
            ):
                return_started = True

            ambient_phase = (ambient_phase + 1) % len(CAMERA_AMBIENT_HEIGHT_WAVE)
            ambient_height += CAMERA_AMBIENT_HEIGHT_WAVE[ambient_phase]
            if not return_started:
                previous_output_position = follow_camera_position(
                    camera_player,
                    vertical_offset,
                    rear_distance,
                    ambient_height,
                    sine,
                    cosine,
                )
                previous_output_orientation = list(CAMERA_FOLLOW_FINE_ORIENTATION)
                camera = (
                    *previous_output_position,
                    (
                        damage_camera_recoil * PLAYER_HIT_CAMERA_RECOIL_SCALE
                    )
                    & 255,
                    0,
                    0,
                )
                continue

            vertical_offset = approach_power(
                vertical_offset,
                0,
                divisions=3,
                minimum=CAMERA_RETURN_VERTICAL_CHASE_DIVISOR,
            )
            anchor_position = follow_camera_position(
                camera_player,
                vertical_offset,
                rear_distance,
                ambient_height,
                sine,
                cosine,
            )
            if lead_settle_updates > 0:
                lead_settle_updates -= 1
                lead_depth = chase_orientation(
                    lead_depth,
                    CAMERA_RETURN_LEAD_TARGET,
                )
            else:
                lead_depth = approach_power(
                    lead_depth,
                    0,
                    divisions=4,
                    minimum=CAMERA_RETURN_LEAD_DECAY_DIVISOR,
                )
            lead_y, lead_depth_after_pitch = rotate_yz(
                record.player[3],
                0,
                lead_depth,
                sine,
                cosine,
            )
            lead_x, lead_z = rotate_xz(
                record.player[4],
                0,
                lead_depth_after_pitch,
                sine,
                cosine,
            )
            anchor_position = (
                signed_word(anchor_position[0] + lead_x),
                signed_word(anchor_position[1] + lead_y),
                signed_word(anchor_position[2] + lead_z),
            )
            base_position = anchor_position
            orientation = list(CAMERA_FOLLOW_FINE_ORIENTATION)
            install_continuity = False

            if rear_distance > CAMERA_RETURN_REAR_DISTANCE_TARGET:
                rear_distance = approach_step(
                    rear_distance,
                    CAMERA_RETURN_REAR_DISTANCE_TARGET,
                    CAMERA_RETURN_REAR_DISTANCE_STEP,
                )
                install_continuity = (
                    rear_distance == -CAMERA_RETURN_REAR_DISTANCE_STEP
                    or rear_distance == CAMERA_RETURN_REAR_DISTANCE_TARGET
                )
                if rear_distance == -CAMERA_RETURN_REAR_DISTANCE_STEP:
                    lead_depth = record.player[6]
                    lead_settle_updates = CAMERA_RETURN_LEAD_SETTLE_UPDATES
            else:
                if not orbit_started:
                    orbit_started = True
                    orbit_yaw = (
                        -(CAMERA_FOLLOW_FINE_ORIENTATION[1] >> 8)
                    ) & 255
                    install_continuity = True
                orbit_depth = signed_word(
                    orbit_depth - CAMERA_RETURN_ORBIT_DEPTH_STEP
                )
                orbit_yaw = (
                    orbit_yaw - CAMERA_RETURN_ORBIT_YAW_STEP
                ) & 255
                offset_x, offset_z = rotate_xz(
                    orbit_yaw,
                    0,
                    orbit_depth,
                    sine,
                    cosine,
                )
                base_position = (
                    signed_word(camera_player[0] + offset_x),
                    signed_word(
                        camera_player[1] + CAMERA_RETURN_ORBIT_VERTICAL_OFFSET
                    ),
                    signed_word(camera_player[2] + offset_z),
                )

                delta_x = signed_word(camera_player[0] - base_position[0])
                delta_y = signed_word(camera_player[1] - base_position[1])
                delta_z = signed_word(camera_player[2] - base_position[2])
                desired_pitch = signed_word(
                    -sf2_atan16(
                        delta_y,
                        sf2_xz_angle_distance(delta_x, delta_z),
                        angle_curve,
                    )
                ) >> 1
                desired_yaw = signed_word(
                    sf2_atan16(delta_x, delta_z, angle_curve)
                )
                orientation = [
                    chase_orientation(
                        previous_output_orientation[0],
                        desired_pitch,
                    ),
                    chase_orientation(
                        previous_output_orientation[1],
                        desired_yaw,
                    ),
                    0,
                ]

            if install_continuity:
                continuity_translation = [
                    signed_word(previous - current)
                    for previous, current in zip(
                        previous_output_position,
                        base_position,
                    )
                ]
                angular_velocity = [
                    signed_byte(
                        (previous >> 8) - (current >> 8)
                    )
                    for previous, current in zip(
                        previous_output_orientation,
                        orientation,
                    )
                ]
                translation_reference_yaw = (orientation[1] >> 8) & 255

            continuity_translation = [
                approach_power(
                    value,
                    0,
                    divisions=4,
                    minimum=CAMERA_RETURN_CONTINUITY_DIVISOR,
                )
                for value in continuity_translation
            ]
            angular_velocity = [
                approach_step(
                    value,
                    0,
                    CAMERA_RETURN_ANGULAR_VELOCITY_STEP,
                )
                for value in angular_velocity
            ]
            orientation = [
                signed_word(value + velocity * 256)
                for value, velocity in zip(orientation, angular_velocity)
            ]
            translation_yaw = (
                translation_reference_yaw - ((orientation[1] >> 8) & 255)
            ) & 255
            translation_x, translation_z = rotate_xz(
                translation_yaw,
                continuity_translation[0],
                continuity_translation[2],
                sine,
                cosine,
            )
            previous_output_position = (
                signed_word(base_position[0] + translation_x),
                signed_word(base_position[1] + continuity_translation[1]),
                signed_word(base_position[2] + translation_z),
            )
            previous_output_orientation = orientation
            camera = (
                *previous_output_position,
                *(value & 255 for value in previous_output_orientation),
            )

        if camera != record.camera:
            raise SystemExit(
                f"typed camera rules diverge at retail frame {retail_frame}: "
                f"expected {record.camera}, recovered {camera}"
            )


def source_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compact_active_fixture(
    source: Path, output: Path, keyframes: list[tuple[int, Record]]
) -> None:
    lines = [
        "# Compact Mesen oracle evidence for the neutral first sortie.",
        f"# Raw source SHA-256: {source_digest(source)}",
        "# Contains only fields consumed by generate_opening_continuation.py.",
    ]
    for _, record in keyframes:
        objects = []
        for source_id, pose in zip(ENCOUNTER_SOURCE_IDS, record.encounter):
            if pose is not None:
                objects.append(
                    f"{source_id},{ENCOUNTER_SHAPES_BY_SOURCE[source_id]},"
                    + ",".join(map(str, pose))
                )
        for source_id, pose in record.projectiles:
            objects.append(
                f"{source_id},{PROJECTILE_SHAPE_TOKEN}," + ",".join(map(str, pose))
            )
        lines.append(
            f"elapsed={record.elapsed} mode={record.mode} phase=- "
            f"camera={','.join(map(str, record.camera))} "
            f"pose={','.join(map(str, record.player))} wingpose=- "
            f"active=[{';'.join(objects)}] object=-"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def compact_timer_fixture(
    source: Path,
    output: Path,
    anchor_elapsed: int,
    timer_keyframes: list[tuple[int, int]],
) -> None:
    lines = [
        "# Compact Mesen oracle evidence for the first-sortie elapsed timer.",
        f"# Raw source SHA-256: {source_digest(source)}",
        "# Fractional steps use the retail 24-steps-per-tenth scheduler.",
    ]
    for retail_frame, elapsed_tenths in timer_keyframes:
        elapsed = anchor_elapsed + retail_frame - ANCHOR_RETAIL_FRAME
        whole, tenths = divmod(elapsed_tenths, 10)
        lines.append(
            f"elapsed={elapsed} timer={whole},{tenths * 24},0 selected=typed-player"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def rust_array(values: tuple[int, ...]) -> str:
    return "[" + ", ".join(f"{value:_}" for value in values) + "]"


def rust_source(
    trace_name: str,
    timer_trace_name: str,
    player_dynamics_name: str,
    keyframes: list[tuple[int, Record]],
    timer_keyframes: list[tuple[int, int]],
    cadence: list[FlightCadence],
) -> str:
    encounter_keyframes = []
    for keyframe in keyframes:
        if any(pose is None for pose in keyframe[1].encounter):
            break
        encounter_keyframes.append(keyframe)
    if not encounter_keyframes:
        raise SystemExit("trace has no coherent four-actor encounter window")

    lines = [
        "//! Generated typed continuation of the retail first-sortie opening.",
        "//!",
        f"//! Source: `{trace_name}`.",
        f"//! Mission timer source: `{timer_trace_name}`.",
        f"//! Player dynamics source: `{player_dynamics_name}`.",
        "//! Shipping player motion and the follow/return camera advance typed state",
        "//! from recovered semantic cadence; complete poses are test-only.",
        "//! Regenerate or verify with `uv run python "
        "tools/sf2/generate_opening_continuation.py [--check]`.",
        "",
        "#[cfg(test)]",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe, mission_actor_keyframe,",
        "    MissionActorKeyframe,",
        "};",
        "#[cfg(test)]",
        "use super::{mission_camera_keyframe, MissionCameraKeyframe};",
        "use super::{mission_encounter_keyframe, mission_timer_keyframe};",
        "#[cfg(test)]",
        "use super::{mission_player_keyframe, MissionPlayerKeyframe};",
        "use super::{Angle, Vector3};",
        "use super::{MissionEncounterKeyframe, MissionTimerKeyframe};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) struct OpeningFlightCadence {",
        "    pub control_updates: u8,",
        "    pub movement_updates: u8,",
        "    pub camera_updates: u8,",
        "    pub camera_uses_previous_player_position: bool,",
        "}",
        "",
        f"pub(super) const PLAYER_CERTIFIED_END_RETAIL_FRAME: u16 = {keyframes[-1][0]};",
        "#[cfg(test)]",
        "pub(super) const ENCOUNTER_CERTIFIED_END_RETAIL_FRAME: u16 = "
        f"{encounter_keyframes[-1][0]};",
        "",
        f"pub(super) const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        f"const PLAYER_LIVE_FIRST_RETAIL_FRAME: u16 = {keyframes[0][0]};",
        f"const PLAYER_LIVE_LAST_RETAIL_FRAME: u16 = {keyframes[-1][0]};",
        "#[cfg(test)]",
        "pub(super) const CAMERA_TYPED_LAST_RETAIL_FRAME: u16 = "
        f"{CAMERA_TYPED_LAST_RETAIL_FRAME};",
        "pub(super) const CAMERA_RETURN_FIRST_RETAIL_FRAME: u16 = "
        f"{CAMERA_RETURN_FIRST_RETAIL_FRAME};",
        "pub(super) const PLAYER_HANDOFF_POSITION: Vector3 = Vector3 {",
        f"    x: {keyframes[0][1].player[0]:_},",
        f"    y: {keyframes[0][1].player[1]:_},",
        f"    z: {keyframes[0][1].player[2]:_},",
        "};",
        "pub(super) const PLAYER_HANDOFF_PITCH: Angle = "
        f"Angle::from_units({keyframes[0][1].player[3]});",
        "pub(super) const PLAYER_HANDOFF_YAW: Angle = "
        f"Angle::from_units({keyframes[0][1].player[4]});",
        "pub(super) const PLAYER_HANDOFF_BANK: Angle = "
        f"Angle::from_units({keyframes[0][1].player[5]});",
        f"pub(super) const PLAYER_HANDOFF_SPEED: u8 = {keyframes[0][1].player[6]};",
        "pub(super) const PLAYER_HANDOFF_AMBIENT_BANK_PHASE: u8 = "
        f"{PLAYER_AMBIENT_BANK_PHASE_AT_ANCHOR};",
        "pub(super) const PLAYER_NEUTRAL_TARGET_SPEED: u8 = "
        f"{keyframes[0][1].player[6]};",
        "pub(super) const PLAYER_NEUTRAL_VELOCITY: Vector3 = Vector3 "
        f"{{ x: {PLAYER_NEUTRAL_VELOCITY[0]}, y: {PLAYER_NEUTRAL_VELOCITY[1]}, "
        f"z: {PLAYER_NEUTRAL_VELOCITY[2]} }};",
        "",
        "pub(super) const CAMERA_HANDOFF_POSITION: Vector3 = Vector3 {",
        f"    x: {keyframes[0][1].camera[0]:_},",
        f"    y: {keyframes[0][1].camera[1]:_},",
        f"    z: {keyframes[0][1].camera[2]:_},",
        "};",
        "pub(super) const CAMERA_FOLLOW_REAR_DISTANCE: i16 = "
        f"{CAMERA_FOLLOW_REAR_DISTANCE};",
        "pub(super) const CAMERA_FOLLOW_VERTICAL_OFFSET: i16 = "
        f"{CAMERA_FOLLOW_VERTICAL_OFFSET};",
        "pub(super) const CAMERA_AMBIENT_HEIGHT_PHASE_AT_HANDOFF: u8 = "
        f"{CAMERA_AMBIENT_HEIGHT_PHASE_AT_ANCHOR};",
        "pub(super) const CAMERA_AMBIENT_HEIGHT_AT_HANDOFF: i16 = "
        f"{CAMERA_AMBIENT_HEIGHT_AT_ANCHOR};",
        "pub(super) const CAMERA_RETURN_REAR_DISTANCE_TARGET: i16 = "
        f"{CAMERA_RETURN_REAR_DISTANCE_TARGET};",
        "pub(super) const CAMERA_RETURN_REAR_DISTANCE_STEP: i16 = "
        f"{CAMERA_RETURN_REAR_DISTANCE_STEP};",
        "pub(super) const CAMERA_RETURN_VERTICAL_CHASE_DIVISOR: i16 = "
        f"{CAMERA_RETURN_VERTICAL_CHASE_DIVISOR};",
        "pub(super) const CAMERA_RETURN_ORBIT_INITIAL_DEPTH: i16 = "
        f"{CAMERA_RETURN_ORBIT_INITIAL_DEPTH};",
        "pub(super) const CAMERA_RETURN_ORBIT_DEPTH_STEP: i16 = "
        f"{CAMERA_RETURN_ORBIT_DEPTH_STEP};",
        "pub(super) const CAMERA_RETURN_ORBIT_VERTICAL_OFFSET: i16 = "
        f"{CAMERA_RETURN_ORBIT_VERTICAL_OFFSET};",
        "pub(super) const CAMERA_RETURN_ORBIT_YAW_STEP: i8 = "
        f"{CAMERA_RETURN_ORBIT_YAW_STEP};",
        "pub(super) const CAMERA_RETURN_LEAD_TARGET: i16 = "
        f"{CAMERA_RETURN_LEAD_TARGET};",
        "pub(super) const CAMERA_RETURN_LEAD_SETTLE_UPDATES: u8 = "
        f"{CAMERA_RETURN_LEAD_SETTLE_UPDATES};",
        "pub(super) const CAMERA_RETURN_LEAD_DECAY_DIVISOR: i16 = "
        f"{CAMERA_RETURN_LEAD_DECAY_DIVISOR};",
        "pub(super) const CAMERA_RETURN_CONTINUITY_DIVISOR: i16 = "
        f"{CAMERA_RETURN_CONTINUITY_DIVISOR};",
        "pub(super) const CAMERA_RETURN_ORIENTATION_CHASE_DIVISOR: i16 = "
        f"{CAMERA_RETURN_ORIENTATION_CHASE_DIVISOR};",
        "pub(super) const CAMERA_RETURN_ORIENTATION_CHASE_MINIMUM: i16 = "
        f"{CAMERA_RETURN_ORIENTATION_CHASE_MINIMUM};",
        "pub(super) const CAMERA_RETURN_ANGULAR_VELOCITY_STEP: i8 = "
        f"{CAMERA_RETURN_ANGULAR_VELOCITY_STEP};",
        "pub(super) const CAMERA_ORIENTATION_COARSE_SHIFT: u32 = "
        f"{CAMERA_ORIENTATION_COARSE_SHIFT};",
        "pub(super) const CAMERA_ORIENTATION_SUBUNITS_PER_COARSE_UNIT: i16 = "
        f"{CAMERA_ORIENTATION_SUBUNITS_PER_COARSE_UNIT};",
        "pub(super) const CAMERA_FOLLOW_FINE_ORIENTATION: [u16; 3] = "
        f"{rust_array(CAMERA_FOLLOW_FINE_ORIENTATION)};",
        "",
        "#[cfg(test)]",
        f"pub(super) const CAMERA_KEYFRAMES: [MissionCameraKeyframe; {len(keyframes)}] = [",
    ]
    for frame, record in keyframes:
        lines.append(
            f"    mission_camera_keyframe({frame}, "
            + ", ".join(f"{value:_}" for value in record.camera)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
            "#[cfg(test)]",
            f"pub(super) const PLAYER_KEYFRAMES: [MissionPlayerKeyframe; {len(keyframes)}] = [",
        ]
    )
    for frame, record in keyframes:
        lines.append(
            f"    mission_player_keyframe({frame}, "
            + ", ".join(f"{value:_}" for value in record.player)
            + "),"
        )
    lines.extend(["];", ""])
    skipped_control_frames = [
        entry.retail_frame for entry in cadence if entry.control_updates == 0
    ]
    double_control_frames = [
        entry.retail_frame for entry in cadence if entry.control_updates == 2
    ]
    skipped_movement_frames = [
        entry.retail_frame for entry in cadence if entry.movement_updates == 0
    ]
    double_movement_frames = [
        entry.retail_frame for entry in cadence if entry.movement_updates == 2
    ]
    skipped_camera_frames = [
        entry.retail_frame
        for entry in cadence
        if entry.camera_updates == 0
    ]
    double_camera_frames = [
        entry.retail_frame
        for entry in cadence
        if entry.camera_updates == 2
    ]
    previous_player_camera_frames = [
        entry.retail_frame
        for entry in cadence
        if entry.camera_uses_previous_player_position
    ]
    natural_hit_frames = [
        entry.retail_frame for entry in cadence if entry.damage_bank_impulse != 0
    ]
    def frame_array(name: str, frames: list[int]) -> None:
        lines.append(f"const {name}: [u16; {len(frames)}] = [")
        for start in range(0, len(frames), 16):
            lines.append(
                "    "
                + ", ".join(str(frame) for frame in frames[start : start + 16])
                + ","
            )
        lines.extend(["];", ""])

    frame_array("PLAYER_SKIPPED_CONTROL_RETAIL_FRAMES", skipped_control_frames)
    frame_array("PLAYER_DOUBLE_CONTROL_RETAIL_FRAMES", double_control_frames)
    frame_array("PLAYER_SKIPPED_MOVEMENT_RETAIL_FRAMES", skipped_movement_frames)
    frame_array("PLAYER_DOUBLE_MOVEMENT_RETAIL_FRAMES", double_movement_frames)
    frame_array("CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES", skipped_camera_frames)
    frame_array("CAMERA_DOUBLE_UPDATE_RETAIL_FRAMES", double_camera_frames)
    frame_array(
        "CAMERA_PREVIOUS_PLAYER_POSITION_RETAIL_FRAMES",
        previous_player_camera_frames,
    )
    lines.extend(
        [
            "#[cfg(test)]",
            f"pub(super) const NATURAL_HIT_RETAIL_FRAMES: [u16; {len(natural_hit_frames)}] = "
            f"[{', '.join(map(str, natural_hit_frames))}];",
            "",
            "pub(super) fn player_flight_cadence(retail_frame: u16) "
            "-> Option<OpeningFlightCadence> {",
            "    let offset = retail_frame.checked_sub(PLAYER_LIVE_FIRST_RETAIL_FRAME)?;",
            "    if retail_frame > PLAYER_LIVE_LAST_RETAIL_FRAME "
            "|| offset % RETAIL_FRAME_STEP != 0 {",
            "        return None;",
            "    }",
            "    let control_updates = if "
            "PLAYER_SKIPPED_CONTROL_RETAIL_FRAMES.contains(&retail_frame) {",
            "        0",
            "    } else if PLAYER_DOUBLE_CONTROL_RETAIL_FRAMES.contains(&retail_frame) {",
            "        2",
            "    } else {",
            "        1",
            "    };",
            "    let movement_updates = if "
            "PLAYER_SKIPPED_MOVEMENT_RETAIL_FRAMES.contains(&retail_frame) {",
            "        0",
            "    } else if PLAYER_DOUBLE_MOVEMENT_RETAIL_FRAMES.contains(&retail_frame) {",
            "        2",
            "    } else {",
            "        1",
            "    };",
            "    let camera_updates = if CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES.contains(&retail_frame) {",
            "        0",
            "    } else if CAMERA_DOUBLE_UPDATE_RETAIL_FRAMES.contains(&retail_frame) {",
            "        2",
            "    } else {",
            "        1",
            "    };",
            "    Some(OpeningFlightCadence {",
            "        control_updates,",
            "        movement_updates,",
            "        camera_updates,",
            "        camera_uses_previous_player_position: "
            "CAMERA_PREVIOUS_PLAYER_POSITION_RETAIL_FRAMES",
            "            .contains(&retail_frame),",
            "    })",
            "}",
            "",
        ]
    )
    lines.extend(
        [
            "pub(super) const MISSION_TIMER_KEYFRAMES: "
            f"[MissionTimerKeyframe; {len(timer_keyframes)}] = [",
        ]
    )
    for frame, elapsed_tenths in timer_keyframes:
        lines.append(f"    mission_timer_keyframe({frame}, {elapsed_tenths}),")
    lines.extend(
        [
            "];",
            "",
            "pub(super) const ENCOUNTER_KEYFRAMES: "
            f"[MissionEncounterKeyframe; {len(encounter_keyframes)}] = [",
        ]
    )
    for frame, record in encounter_keyframes:
        lines.append("    mission_encounter_keyframe(")
        lines.append(f"        {frame},")
        for pose in record.encounter:
            assert pose is not None
            lines.append(f"        {rust_array(pose)},")
        lines.append("    ),")
    lines.extend(["];", ""])
    encounter_end = encounter_keyframes[-1][0]
    for actor_index, constant_name in enumerate(ENCOUNTER_CONSTANT_NAMES):
        later_frames = [keyframe for keyframe in keyframes if keyframe[0] > encounter_end]
        last_present_index = max(
            (
                index
                for index, (_, record) in enumerate(later_frames)
                if record.encounter[actor_index] is not None
            ),
            default=-1,
        )
        terminal_index = min(last_present_index + 1, len(later_frames) - 1)
        track = later_frames[: terminal_index + 1]
        if not track:
            raise SystemExit(f"{constant_name} has no departure frame")
        lines.append("#[cfg(test)]")
        if len(track) == 1 and track[0][1].encounter[actor_index] is None:
            lines.append(
                f"pub(super) const {constant_name}: [MissionActorKeyframe; 1] ="
            )
            lines.extend(
                [
                    f"    [mission_actor_departure_keyframe({track[0][0]})];",
                    "",
                ]
            )
            continue
        lines.append(
            f"pub(super) const {constant_name}: "
            f"[MissionActorKeyframe; {len(track)}] = ["
        )
        for index, (frame, record) in enumerate(track):
            pose = record.encounter[actor_index]
            if pose is not None:
                lines.append(f"    mission_actor_keyframe({frame}, {rust_array(pose)}),")
            elif index <= last_present_index:
                lines.append(f"    mission_actor_inactive_keyframe({frame}),")
            else:
                lines.append(f"    mission_actor_departure_keyframe({frame}),")
        lines.extend(["];", ""])

    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_ACTIVE_TRACE)
    parser.add_argument("timer_trace", type=Path, nargs="?", default=DEFAULT_TIMER_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--player-dynamics",
        type=Path,
        default=DEFAULT_PLAYER_DYNAMICS,
        help="compact control/movement cadence imported from the retail oracle",
    )
    parser.add_argument(
        "--compact-active-output",
        type=Path,
        help="write the generator-relevant subset of the full active-flight capture",
    )
    parser.add_argument(
        "--compact-timer-output",
        type=Path,
        help="write only timer value-change evidence from the full timer capture",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the checked-in generated Rust source is out of date",
    )
    args = parser.parse_args()
    keyframes, anchor_elapsed = continuation(args.trace)
    cadence = player_cadence(args.player_dynamics)
    verify_live_player(keyframes, cadence)
    verify_live_camera(keyframes, cadence)
    timer_keyframes = mission_timer_keyframes(
        args.timer_trace, anchor_elapsed, keyframes[-1][0]
    )
    if args.compact_active_output is not None:
        compact_active_fixture(args.trace, args.compact_active_output, keyframes)
    if args.compact_timer_output is not None:
        compact_timer_fixture(
            args.timer_trace,
            args.compact_timer_output,
            anchor_elapsed,
            timer_keyframes,
        )
    generated = rust_source(
        args.trace.name,
        args.timer_trace.name,
        args.player_dynamics.name,
        keyframes,
        timer_keyframes,
        cadence,
    )
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {args.output}")
        action = "verified"
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(generated, encoding="utf-8")
        action = "generated"
    print(
        f"{action} {args.output}: {len(keyframes)} coherent keyframes, "
        f"retail frames {keyframes[0][0]}..{keyframes[-1][0]}, "
        f"{len(timer_keyframes)} mission-timer changes"
    )


if __name__ == "__main__":
    main()
