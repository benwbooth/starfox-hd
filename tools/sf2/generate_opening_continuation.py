#!/usr/bin/env python3
"""Generate typed post-frame-900 opening-sortie keyframes from an oracle trace.

Camera, player, and the four encounter poses are sampled from the same elapsed
presentation frame so the generated native scene stays coherent. Source object
tokens are used only to classify oracle rows and are never emitted into Rust.
"""

from __future__ import annotations

import argparse
import hashlib
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


@dataclass(frozen=True)
class Record:
    elapsed: int
    mode: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    encounter: tuple[tuple[int, ...] | None, ...]
    projectiles: tuple[tuple[str, tuple[int, ...]], ...]


@dataclass(frozen=True)
class PlayerCadence:
    retail_frame: int
    control_updates: int
    movement_updates: int
    damage_bank_impulse: int


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


def player_cadence(trace: Path) -> list[PlayerCadence]:
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
        cadence = PlayerCadence(
            retail_frame=int(values["retail_frame"]),
            control_updates=int(values["control_updates"]),
            movement_updates=int(values["movement_updates"]),
            damage_bank_impulse=int(values["damage_bank_impulse"]),
        )
        if cadence.control_updates not in (0, 1, 2):
            raise SystemExit("player dynamics contain an invalid control count")
        if cadence.movement_updates not in (0, 1, 2):
            raise SystemExit("player dynamics contain an invalid movement count")
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


def recover_damage_bank(value: int) -> int:
    if value == 0:
        return 0
    adjusted_difference = max(abs(value), PLAYER_HIT_BANK_RECOVERY_DIVISOR)
    signed_difference = -adjusted_difference if value > 0 else adjusted_difference
    return value + int(signed_difference / PLAYER_HIT_BANK_RECOVERY_DIVISOR)


def verify_live_player(
    keyframes: list[tuple[int, Record]], cadence: list[PlayerCadence]
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
    cadence: list[PlayerCadence],
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
        "//! Shipping player motion advances typed state from the recovered",
        "//! control/movement cadence; the complete player poses are test-only.",
        "//! Regenerate or verify with `uv run python "
        "tools/sf2/generate_opening_continuation.py [--check]`.",
        "",
        "#[cfg(test)]",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe, mission_actor_keyframe,",
        "    MissionActorKeyframe,",
        "};",
        "use super::{",
        "    mission_camera_keyframe, mission_encounter_keyframe, mission_timer_keyframe,",
        "    MissionCameraKeyframe, MissionEncounterKeyframe, MissionTimerKeyframe,",
        "};",
        "#[cfg(test)]",
        "use super::{mission_player_keyframe, MissionPlayerKeyframe};",
        "use super::{Angle, Vector3};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub(super) struct PlayerFlightCadence {",
        "    pub control_updates: u8,",
        "    pub movement_updates: u8,",
        "}",
        "",
        f"pub(super) const PLAYER_CERTIFIED_END_RETAIL_FRAME: u16 = {keyframes[-1][0]};",
        "#[cfg(test)]",
        "pub(super) const ENCOUNTER_CERTIFIED_END_RETAIL_FRAME: u16 = "
        f"{encounter_keyframes[-1][0]};",
        "",
        f"const RETAIL_FRAME_STEP: u16 = {RETAIL_FRAME_STEP};",
        f"const PLAYER_LIVE_FIRST_RETAIL_FRAME: u16 = {keyframes[0][0]};",
        f"const PLAYER_LIVE_LAST_RETAIL_FRAME: u16 = {keyframes[-1][0]};",
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
        "",
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
    lines.extend(
        [
            "#[cfg(test)]",
            f"pub(super) const NATURAL_HIT_RETAIL_FRAMES: [u16; {len(natural_hit_frames)}] = "
            f"[{', '.join(map(str, natural_hit_frames))}];",
            "",
            "pub(super) fn player_flight_cadence(retail_frame: u16) "
            "-> Option<PlayerFlightCadence> {",
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
            "    Some(PlayerFlightCadence {",
            "        control_updates,",
            "        movement_updates,",
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
