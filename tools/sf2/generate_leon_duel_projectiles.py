#!/usr/bin/env python3
"""Generate native hostile-projectile dynamics for Leon's extended duel."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from generate_pigma_duel import fields, raw_objects
from generate_second_sortie_projectiles import (
    format_rust,
    generate_dynamics,
    import_raw_logic,
    read_pose_fixture,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "leon_duel_projectile_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "leon_duel_extended.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "leon_duel_projectiles.rs"
)

RAW_SAMPLE_START_ELAPSED = 63_320
RETAIL_FRAME_STEP = 4
FINAL_CERTIFIED_RETAIL_FRAME = 1_880
EXPECTED_PROJECTILE_LIFETIMES = 6
MISSION_SELECTION = "7"
HOSTILE_PROJECTILE_SHAPES = frozenset(("E3A8",))


def append_test_oracle(source: str, pose_fixture: Path) -> str:
    """Embed retained poses only for exhaustive native-runtime verification."""
    _, lifetimes = read_pose_fixture(pose_fixture, EXPECTED_PROJECTILE_LIFETIMES)
    lines = [
        source,
        "#[cfg(test)]",
        "use super::{mission_projectile_keyframe, MissionProjectileKeyframe};",
        "",
    ]
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


def import_raw_poses(source: Path, output: Path) -> None:
    """Reduce an uninterrupted duel capture to typed gameplay poses."""
    records: dict[
        int,
        tuple[
            tuple[int, ...],
            tuple[int, ...] | None,
            tuple[tuple[str, tuple[int, ...]], ...],
        ],
    ] = {}
    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("event") != "sortie"
            or values.get("mode") != "1"
            or values.get("selection") != MISSION_SELECTION
            or "elapsed" not in values
        ):
            continue
        elapsed = int(values["elapsed"])
        retail_frame = elapsed - RAW_SAMPLE_START_ELAPSED
        if not 0 <= retail_frame <= FINAL_CERTIFIED_RETAIL_FRAME:
            continue
        rival, projectiles = raw_objects(
            values["objects"], HOSTILE_PROJECTILE_SHAPES, "0576", "C348"
        )
        records[retail_frame] = (
            tuple(map(int, values["playerpose"].split(","))),
            rival,
            projectiles,
        )

    expected_frames = range(0, FINAL_CERTIFIED_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    if list(records) != list(expected_frames):
        missing = sorted(set(expected_frames).difference(records))
        raise SystemExit(
            "extended Leon capture is not a complete four-frame cadence; "
            f"missing={missing[:8]}"
        )

    lines = [
        "# Compact oracle evidence for Leon's extended-duel hostile projectiles.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# certified_retail_frames=0..{FINAL_CERTIFIED_RETAIL_FRAME}",
    ]
    for retail_frame in expected_frames:
        player, rival, projectiles = records[retail_frame]
        projectile_text = ";".join(
            source_id + "," + ",".join(map(str, pose))
            for source_id, pose in projectiles
        )
        lines.append(
            f"retail_frame={retail_frame} "
            f"player={','.join(map(str, player))} "
            f"rival={'-' if rival is None else ','.join(map(str, rival))} "
            f"projectiles={projectile_text or '-'}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw-pose", type=Path)
    parser.add_argument("--import-raw-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw_pose is not None:
        import_raw_poses(args.import_raw_pose, args.pose_fixture)
    if args.import_raw_logic is not None:
        import_raw_logic(
            args.import_raw_logic,
            args.logic_fixture,
            RAW_SAMPLE_START_ELAPSED,
            "Leon's extended duel",
        )

    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        RAW_SAMPLE_START_ELAPSED,
        "the extended retail Leon duel",
        allow_split_contractions=True,
    )
    source = append_test_oracle(source, args.pose_fixture)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated Leon projectile dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "Leon-duel hostile-projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
