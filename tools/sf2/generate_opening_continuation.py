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
PROJECTILE_TRACKS = {
    "04B9": (
        "UPPER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES",
        "SECOND_CAPITAL_OPENING_SHOT_THREE_KEYFRAMES",
        "UPPER_FIGHTER_OPENING_SHOT_FOUR_KEYFRAMES",
    ),
    "043B": ("LOWER_FIGHTER_OPENING_SHOT_KEYFRAMES",),
    "082B": (
        "SECOND_CAPITAL_OPENING_SHOT_ONE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_ONE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_THREE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_FIVE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_EIGHT_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_SIXTEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_EIGHTEEN_KEYFRAMES",
    ),
    "047A": (
        "UPPER_FIGHTER_OPENING_SHOT_THREE_KEYFRAMES",
        "FIRST_CAPITAL_OPENING_SHOT_KEYFRAMES",
        "UPPER_FIGHTER_OPENING_SHOT_FIVE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_NINE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWELVE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_FIFTEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_TWO_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_THREE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_SIX_KEYFRAMES",
    ),
    "086A": (
        "SECOND_CAPITAL_OPENING_SHOT_TWO_KEYFRAMES",
        "SECOND_CAPITAL_OPENING_SHOT_FOUR_KEYFRAMES",
        "UPPER_FIGHTER_OPENING_SHOT_SIX_KEYFRAMES",
        "SECOND_CAPITAL_OPENING_SHOT_FIVE_KEYFRAMES",
    ),
    "0576": ("SECOND_CAPITAL_MISSION_SHOT_SIX_KEYFRAMES",),
    "05B5": (
        "SECOND_CAPITAL_MISSION_SHOT_ELEVEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_THIRTEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_FOURTEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_NINETEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_ONE_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_FOUR_KEYFRAMES",
    ),
    "0672": (
        "SECOND_CAPITAL_MISSION_SHOT_TEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_SEVENTEEN_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_TWENTY_FIVE_KEYFRAMES",
    ),
    "06B1": ("SECOND_CAPITAL_MISSION_SHOT_TWO_KEYFRAMES",),
    "07EC": ("FIRST_CAPITAL_OPENING_SHOT_THREE_KEYFRAMES",),
    "0966": (
        "LOWER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES",
        "FIRST_CAPITAL_OPENING_SHOT_TWO_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_FOUR_KEYFRAMES",
        "SECOND_CAPITAL_MISSION_SHOT_SEVEN_KEYFRAMES",
    ),
}


@dataclass(frozen=True)
class Record:
    elapsed: int
    mode: int
    camera: tuple[int, ...]
    player: tuple[int, ...]
    encounter: tuple[tuple[int, ...] | None, ...]
    projectiles: tuple[tuple[str, tuple[int, ...]], ...]


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
    keyframes: list[tuple[int, Record]],
    timer_keyframes: list[tuple[int, int]],
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
        "//! Regenerate or verify with `uv run python "
        "tools/sf2/generate_opening_continuation.py [--check]`.",
        "",
        "#[cfg(test)]",
        "use super::{",
        "    mission_actor_departure_keyframe, mission_actor_inactive_keyframe,",
        "    mission_actor_keyframe, MissionActorKeyframe,",
        "};",
        "use super::{",
        "    mission_camera_keyframe,",
        "    mission_encounter_keyframe, mission_player_keyframe, "
        "mission_projectile_keyframe,",
        "    mission_timer_keyframe, MissionCameraKeyframe, MissionEncounterKeyframe,",
        "    MissionPlayerKeyframe, MissionProjectileKeyframe, MissionTimerKeyframe,",
        "};",
        "",
        f"pub(super) const PLAYER_CERTIFIED_END_RETAIL_FRAME: u16 = {keyframes[-1][0]};",
        "#[cfg(test)]",
        "pub(super) const ENCOUNTER_CERTIFIED_END_RETAIL_FRAME: u16 = "
        f"{encounter_keyframes[-1][0]};",
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
            f"pub(super) const PLAYER_KEYFRAMES: [MissionPlayerKeyframe; {len(keyframes)}] = [",
        ]
    )
    for frame, record in keyframes:
        lines.append(
            f"    mission_player_keyframe({frame}, "
            + ", ".join(f"{value:_}" for value in record.player)
            + "),"
        )
    lines.extend(
        [
            "];",
            "",
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
        lines.append(f"    mission_encounter_keyframe(")
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

    for source_id, constant_names in PROJECTILE_TRACKS.items():
        track = []
        for frame, record in keyframes:
            pose = dict(record.projectiles).get(source_id)
            if pose is not None:
                track.append((frame, pose))
        if not track:
            continue
        lifetimes: list[list[tuple[int, tuple[int, ...]]]] = []
        for frame, pose in track:
            if not lifetimes or frame - lifetimes[-1][-1][0] > RETAIL_FRAME_STEP:
                lifetimes.append([])
            lifetimes[-1].append((frame, pose))
        if len(lifetimes) > len(constant_names):
            raise SystemExit(
                f"projectile source {source_id} has {len(lifetimes)} lifetimes, "
                f"but only {len(constant_names)} semantic names"
            )
        for constant_name, lifetime in zip(constant_names, lifetimes):
            declaration = (
                f"pub(super) const {constant_name}: "
                f"[MissionProjectileKeyframe; {len(lifetime)}] = ["
            )
            # rustfmt's type-aware layout keeps declarations through this
            # width on one line, then splits the array length onto the next.
            if len(declaration) <= 102:
                lines.append(declaration)
            else:
                lines.extend(
                    [
                        f"pub(super) const {constant_name}: "
                        "[MissionProjectileKeyframe;",
                        f"    {len(lifetime)}] = [",
                    ]
                )
            for frame, pose in lifetime:
                lines.append(
                    f"    mission_projectile_keyframe({frame}, {rust_array(pose)}),"
                )
            lines.extend(["];", ""])
    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path, nargs="?", default=DEFAULT_ACTIVE_TRACE)
    parser.add_argument("timer_trace", type=Path, nargs="?", default=DEFAULT_TIMER_TRACE)
    parser.add_argument("output", type=Path, nargs="?", default=DEFAULT_OUTPUT)
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
        keyframes,
        timer_keyframes,
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
