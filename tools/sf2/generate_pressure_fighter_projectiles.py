#!/usr/bin/env python3
"""Generate native hostile-projectile dynamics for recurring attackers.

The retained neutral-input oracle capture keeps the retail attackers alive
without altering their flight logic.  Raw object identities are reduced to
typed projectile lifetimes, and the shared flat-state replay must reproduce
every retained pose before Rust is emitted.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from projectile_static import (
    read_collision_eligibility,
    validate_static_collision_gate,
    validate_static_hostile_projectile_path,
)
from generate_second_sortie_projectiles import (
    format_rust,
    generate_dynamics,
    import_raw_logic,
    read_pose_fixture,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_LOGIC_FIXTURE = (
    FIXTURE_DIRECTORY / "pressure_fighter_projectile_logic.trace"
)
DEFAULT_POSE_FIXTURE = FIXTURE_DIRECTORY / "pressure_fighter_projectiles.trace"
DEFAULT_COLLISION_FIXTURE = (
    FIXTURE_DIRECTORY / "pressure_fighter_projectile_collision.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "pressure_fighter_projectiles.rs"
)

RAW_SAMPLE_START_ELAPSED = 73_832
FINAL_CERTIFIED_RETAIL_FRAME = 1_968
RETAIL_FRAME_STEP = 4
MISSION_SELECTION = "6"
HOSTILE_PROJECTILE_SHAPE = "E3A8"
EXPECTED_PROJECTILE_LIFETIMES = 5


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def raw_projectiles(value: str) -> tuple[tuple[str, tuple[int, ...]], ...]:
    result = []
    for object_text in value.removeprefix("[").removesuffix("]").split(";"):
        parts = object_text.split(",")
        if len(parts) >= 9 and parts[1] == HOSTILE_PROJECTILE_SHAPE:
            result.append((parts[0], tuple(map(int, parts[2:9]))))
    return tuple(result)


def import_raw_poses(source: Path, output: Path) -> None:
    records: dict[int, tuple[tuple[int, ...], tuple[tuple[str, tuple[int, ...]], ...]]] = {}
    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("event") != "sortie"
            or values.get("mode") != "1"
            or values.get("selection") != MISSION_SELECTION
            or "elapsed" not in values
        ):
            continue
        retail_frame = int(values["elapsed"]) - RAW_SAMPLE_START_ELAPSED
        if not 0 <= retail_frame <= FINAL_CERTIFIED_RETAIL_FRAME:
            continue
        records[retail_frame] = (
            tuple(map(int, values["playerpose"].split(","))),
            raw_projectiles(values["objects"]),
        )

    expected_frames = list(
        range(0, FINAL_CERTIFIED_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    )
    if list(records) != expected_frames:
        missing = sorted(set(expected_frames).difference(records))
        raise SystemExit(
            "recurring-attacker capture is not a complete four-frame cadence; "
            f"missing={missing[:8]}"
        )

    lines = [
        "# Compact neutral-input oracle evidence for recurring-attacker projectiles.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        f"# certified_retail_frames=0..{FINAL_CERTIFIED_RETAIL_FRAME}",
    ]
    for retail_frame in expected_frames:
        player, projectiles = records[retail_frame]
        projectile_text = ";".join(
            source_id + "," + ",".join(map(str, pose))
            for source_id, pose in projectiles
        )
        lines.append(
            f"retail_frame={retail_frame} "
            f"player={','.join(map(str, player))} "
            f"projectiles={projectile_text or '-'}"
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_natural_hit(
    collision_fixture: Path,
    records: list,
    lifetimes: list,
) -> dict[str, str]:
    hits = [
        fields(line)
        for line in collision_fixture.read_text(encoding="utf-8").splitlines()
        if line.startswith("natural_hit ")
    ]
    if len(hits) != 1:
        raise SystemExit(
            "recurring-attacker collision fixture must retain one natural hit"
        )
    hit = hits[0]
    expected_fields = {
        "retail_frame",
        "track",
        "craft_class",
        "damage",
        "player_pose",
        "projectile_pose",
        "source",
    }
    if set(hit) != expected_fields:
        raise SystemExit(
            "recurring-attacker natural hit fields changed: "
            f"expected {sorted(expected_fields)}, found {sorted(hit)}"
        )
    retail_frame = int(hit["retail_frame"])
    track_index = int(hit["track"])
    if hit["craft_class"] != "PeppySlippy":
        raise SystemExit("recurring-attacker natural hit craft class changed")
    if int(hit["damage"]) != 2:
        raise SystemExit("recurring-attacker natural hit damage changed")
    if hit["source"] != "06:9707":
        raise SystemExit("recurring-attacker natural hit dispatch source changed")
    if not 0 <= track_index < len(lifetimes):
        raise SystemExit("recurring-attacker natural hit track is out of range")
    projectile_pose = tuple(map(int, hit["projectile_pose"].split(",")))
    if not any(
        frame == retail_frame and pose[:3] == projectile_pose
        for frame, pose in lifetimes[track_index].samples
    ):
        raise SystemExit(
            "recurring-attacker natural hit projectile pose is absent from its track"
        )
    player_pose = tuple(map(int, hit["player_pose"].split(",")))
    previous_player = next(
        (
            record.player[:3]
            for record in records
            if record.retail_frame == retail_frame - RETAIL_FRAME_STEP
        ),
        None,
    )
    if previous_player != player_pose:
        raise SystemExit(
            "recurring-attacker natural hit player pose does not match "
            "the preceding cooperative-update boundary"
        )
    return hit


def append_test_oracle(
    source: str,
    pose_fixture: Path,
    collision_fixture: Path,
) -> str:
    records, lifetimes = read_pose_fixture(
        pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
    )
    natural_hit = read_natural_hit(collision_fixture, records, lifetimes)
    lines = [
        source,
        "#[cfg(test)]",
        "pub(super) const NATURAL_HIT_RETAIL_FRAME: u16 = "
        f"{int(natural_hit['retail_frame']):_};",
        "#[cfg(test)]",
        "pub(super) const NATURAL_HIT_TRACK_INDEX: usize = "
        f"{int(natural_hit['track'])};",
        "",
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
    lines.extend(
        [
            "];",
            "",
        ]
    )
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
    parser.add_argument(
        "--collision-fixture", type=Path, default=DEFAULT_COLLISION_FIXTURE
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw-pose", type=Path)
    parser.add_argument("--import-raw-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    validate_static_hostile_projectile_path()
    validate_static_collision_gate()
    if args.import_raw_pose is not None:
        import_raw_poses(args.import_raw_pose, args.pose_fixture)
    if args.import_raw_logic is not None:
        if not args.pose_fixture.exists():
            raise SystemExit("import projectile poses before importing logic")
        _, lifetimes = read_pose_fixture(
            args.pose_fixture,
            EXPECTED_PROJECTILE_LIFETIMES,
        )
        import_raw_logic(
            args.import_raw_logic,
            args.logic_fixture,
            RAW_SAMPLE_START_ELAPSED,
            "the recurring-attacker sortie",
            frozenset(lifetime.source for lifetime in lifetimes),
        )

    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        RAW_SAMPLE_START_ELAPSED,
        "the recurring-attacker sortie",
        collision_eligibility=read_collision_eligibility(
            args.collision_fixture,
            EXPECTED_PROJECTILE_LIFETIMES,
            "recurring-attacker",
        ),
    )
    source = append_test_oracle(source, args.pose_fixture, args.collision_fixture)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(
                f"generated recurring-attacker projectile dynamics are stale: {args.output}"
            )
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "recurring-attacker projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
