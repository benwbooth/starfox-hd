#!/usr/bin/env python3
"""Generate Leon's pressure-encounter rival flight from oracle operations."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

import generate_leon_duel_rival as shared
from generate_second_sortie_projectiles import format_rust
from rival_static import validate_static_leon_pressure_path


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_LOGIC_FIXTURE = FIXTURE_DIRECTORY / "leon_pressure_rival_logic.trace"
DEFAULT_POSE_FIXTURE = FIXTURE_DIRECTORY / "leon_pressure.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "leon_pressure_rival.rs"
)

RAW_SAMPLE_START_ELAPSED = 73_648
PRESENTATION_START_RETAIL_FRAME = 56
STAGING_RETAIL_FRAME = 400
FLIGHT_START_RETAIL_FRAME = 404
END_RETAIL_FRAME = 1_512
DEPARTURE_RETAIL_FRAME = END_RETAIL_FRAME + shared.RETAIL_FRAME_STEP
STAGING_POSE = (10_096, -4_000, 3_137, 21, 0, 0, 0)
INITIAL_POSE = (10_096, 0, 8_137, 21, 76, 0, 0)
SPECIAL_TARGET_TIMINGS = {
    (-3, 0, -3): "PlayerTargetTiming::PressureEntryMidpoint",
    (1, -8, 0): "PlayerTargetTiming::PressureClimbMidpoint",
    (-2, 0, 0): "PlayerTargetTiming::PressureCruiseMidpoint",
}


def target_timing(
    event: shared.RawEvent,
    frame: int,
    player_poses: dict[int, tuple[int, ...]],
) -> str:
    previous = player_poses[frame - shared.RETAIL_FRAME_STEP]
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
    adjustment = tuple(target[index] - midpoint[index] for index in range(3))
    timing = SPECIAL_TARGET_TIMINGS.get(adjustment)
    if timing is not None:
        return timing
    raise SystemExit(
        f"{event.elapsed}: {event.event} target is not a typed player timing"
    )


def configure_shared_generator() -> None:
    shared.RAW_SAMPLE_START_ELAPSED = RAW_SAMPLE_START_ELAPSED
    shared.PRESENTATION_START_RETAIL_FRAME = PRESENTATION_START_RETAIL_FRAME
    shared.FLIGHT_START_RETAIL_FRAME = FLIGHT_START_RETAIL_FRAME
    shared.END_RETAIL_FRAME = END_RETAIL_FRAME
    shared.INITIAL_POSE = INITIAL_POSE
    shared.target_timing = target_timing


def render_compact(actions: dict[int, list[str]], raw_sha256: str) -> str:
    source = shared.render_compact(actions, raw_sha256)
    source = source.replace(
        "# Semantic Leon rival actions recovered from the extended campaign oracle.",
        "# Semantic Leon rival actions recovered from the pressure-encounter oracle.",
    )
    marker = "# initial_pose=" + ",".join(map(str, INITIAL_POSE))
    return source.replace(
        marker,
        "# staging_retail_frame="
        f"{STAGING_RETAIL_FRAME}\n"
        "# staging_pose="
        + ",".join(map(str, STAGING_POSE))
        + "\n"
        + marker,
    )


def generate_rust(
    actions: dict[int, list[str]],
    player_poses: dict[int, tuple[int, ...]],
    rival_poses: dict[int, tuple[int, ...]],
) -> str:
    source = shared.generate_rust(actions, player_poses, rival_poses)
    source = source.replace(
        "Leon's extended retail duel",
        "Leon's pressure-encounter retail duel",
    )
    marker = (
        f"pub(super) const FLIGHT_START_RETAIL_FRAME: u16 = "
        f"{FLIGHT_START_RETAIL_FRAME:_};\n"
    )
    staging = (
        f"pub(super) const STAGING_RETAIL_FRAME: u16 = {STAGING_RETAIL_FRAME:_};\n"
        f"pub(super) const DEPARTURE_RETAIL_FRAME: u16 = "
        f"{DEPARTURE_RETAIL_FRAME:_};\n"
        "pub(super) const STAGING_POSE: MissionEncounterPose =\n"
        f"    {shared.pose_source(STAGING_POSE)};\n"
    )
    source = source.replace(marker, marker + staging)
    source = source.replace(
        f"pub(super) const RETAIL_FRAME_STEP: u16 = {shared.RETAIL_FRAME_STEP};",
        f"#[cfg(test)]\n"
        f"pub(super) const RETAIL_FRAME_STEP: u16 = {shared.RETAIL_FRAME_STEP};",
    )
    source = source.replace(
        "pub(super) static PLAYER_POSES:",
        "#[cfg(test)]\npub(super) static PLAYER_POSES:",
    )
    source = source.replace(
        "pub(super) fn player_pose(",
        "#[cfg(test)]\npub(super) fn player_pose(",
    )
    source = source.split(
        "\n#[cfg(test)]\npub(super) static ORACLE_RIVAL_POSES",
        maxsplit=1,
    )[0]
    return format_rust(source)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    validate_static_leon_pressure_path()
    configure_shared_generator()
    player_poses, rival_poses = shared.load_poses(args.pose_fixture)
    if args.import_raw is not None:
        raw = args.import_raw.read_bytes()
        actions = shared.semantic_actions(
            shared.raw_events(args.import_raw),
            player_poses,
            rival_poses,
        )
        compact = render_compact(actions, hashlib.sha256(raw).hexdigest())
        args.logic_fixture.parent.mkdir(parents=True, exist_ok=True)
        args.logic_fixture.write_text(compact, encoding="utf-8")
    actions = shared.load_compact(args.logic_fixture)
    source = generate_rust(actions, player_poses, rival_poses)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(
                f"generated Leon pressure rival dynamics are stale: {args.output}"
            )
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "Leon pressure rival semantic schedule verified through retail frame "
        f"{END_RETAIL_FRAME}: {len(rival_poses)} retained rival poses"
    )


if __name__ == "__main__":
    main()
