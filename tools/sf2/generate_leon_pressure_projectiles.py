#!/usr/bin/env python3
"""Generate semantic hostile-projectile dynamics for Leon's pressure encounter."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_first_sortie_projectiles import validate_static_projectile_path
from generate_second_sortie_projectiles import (
    format_rust,
    generate_dynamics,
    import_raw_logic,
    read_pose_fixture,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_LOGIC_FIXTURE = FIXTURE_DIRECTORY / "leon_pressure_projectile_logic.trace"
DEFAULT_POSE_FIXTURE = FIXTURE_DIRECTORY / "leon_pressure.trace"
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "leon_pressure_projectiles.rs"
)

RAW_SAMPLE_START_ELAPSED = 73_648
EXPECTED_PROJECTILE_LIFETIMES = 5


def append_test_oracle(source: str, pose_fixture: Path) -> str:
    """Embed retained poses only for exhaustive native-runtime verification."""
    records, lifetimes = read_pose_fixture(
        pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
    )
    lines = [
        source,
        "#[cfg(test)]",
        "use super::{",
        "    mission_player_keyframe, mission_projectile_keyframe,",
        "    MissionPlayerKeyframe, MissionProjectileKeyframe,",
        "};",
        "",
        "#[cfg(test)]",
        f"pub(super) static ORACLE_PLAYER_KEYFRAMES: "
        f"[MissionPlayerKeyframe; {len(records)}] = [",
    ]
    for record in records:
        x, y, z, pitch, yaw, roll, speed = record.player
        lines.append(
            f"    mission_player_keyframe({record.retail_frame:_}, "
            f"{x:_}, {y:_}, {z:_}, {pitch}, {yaw}, {roll}, {speed}),"
        )
    lines.extend(["];", ""])
    for index, lifetime in enumerate(lifetimes):
        lines.append(
            f"#[cfg(test)] static ORACLE_TRACK_{index}: "
            f"[MissionProjectileKeyframe; {len(lifetime.samples)}] = ["
        )
        for retail_frame, pose in lifetime.samples:
            pose_values = ", ".join(f"{value:_}" for value in pose)
            lines.append(
                f"    mission_projectile_keyframe({retail_frame:_}, [{pose_values}]),"
            )
        lines.extend(["];", ""])
    lines.extend(
        [
            "#[cfg(test)]",
            "pub(super) static ORACLE_TRACKS: "
            f"[&[MissionProjectileKeyframe]; {len(lifetimes)}] = [",
        ]
    )
    lines.extend(f"    &ORACLE_TRACK_{index}," for index in range(len(lifetimes)))
    lines.extend(["];", ""])
    return format_rust("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    validate_static_projectile_path()
    if args.import_raw_logic is not None:
        import_raw_logic(
            args.import_raw_logic,
            args.logic_fixture,
            RAW_SAMPLE_START_ELAPSED,
            "the Leon pressure encounter",
        )

    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        RAW_SAMPLE_START_ELAPSED,
        "the retail Leon pressure encounter",
        allow_split_contractions=True,
    )
    source = append_test_oracle(source, args.pose_fixture)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(
                f"generated Leon-pressure projectile dynamics are stale: {args.output}"
            )
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "Leon-pressure hostile-projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
