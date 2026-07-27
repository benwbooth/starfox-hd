#!/usr/bin/env python3
"""Generate semantic hostile-projectile dynamics for SF2's opening sortie.

The shipping Rust consumes only typed gameplay operations. Raw object
identities and path addresses remain in the oracle fixtures and this static
verification tool. Every emitted action schedule must replay every retained
retail pose exactly before the generated module is accepted.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

from generate_second_sortie_projectiles import (
    format_rust,
    generate_dynamics,
    import_raw_logic,
    read_pose_fixture,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
DEFAULT_LOGIC_FIXTURE = FIXTURE_DIRECTORY / "first_sortie_projectile_logic.trace"
DEFAULT_POSE_FIXTURE = FIXTURE_DIRECTORY / "first_sortie_projectiles.trace"
DEFAULT_COLLISION_FIXTURE = (
    FIXTURE_DIRECTORY / "first_sortie_projectile_collision.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "opening_projectiles.rs"
)

DISASM_DIRECTORY = Path(__file__).with_name("disasm")
sys.path.insert(0, str(DISASM_DIRECTORY))

from extract_map import DEFAULT_ROM  # noqa: E402
from extract_path import PathAddress, PathExtractor  # noqa: E402
from dump_runtime_routine import source_offset  # noqa: E402
from path_semantics import PATH_SEMANTICS  # noqa: E402


# Projectile dispatch runs one cooperative slice ahead of the player pose
# capture used as its target. This offset is independently visible at the
# frame-900 handoff: projectile pose elapsed 7308 and player pose elapsed 7312
# are both retail frame 900.
PROJECTILE_SAMPLE_START_ELAPSED = 6_408
PLAYER_SAMPLE_START_ELAPSED = 6_412
RETAIL_FRAME_STEP = 4
FIRST_COMPLETE_RETAIL_FRAME = 332
LAST_COMPLETE_RETAIL_FRAME = 8_052
HOSTILE_PROJECTILE_SHAPE = "E3A8"
EXPECTED_PROJECTILE_LIFETIMES = 43
MAXIMUM_CONTINUOUS_POSITION_STEP = 4_096

# Chronological firing ownership follows the reviewed weapon schedules for
# the two capital craft and two fighters. The identities are semantic scene
# actors; raw object slots are intentionally not preserved in shipping state.
FIRING_ACTORS = [
    "UpperFighter",
    "UpperFighter",
    "LowerFighter",
    "SecondCapital",
    "UpperFighter",
    "SecondCapital",
    "SecondCapital",
    "FirstCapital",
    "UpperFighter",
    "UpperFighter",
    "SecondCapital",
    "UpperFighter",
    "SecondCapital",
    "LowerFighter",
    "FirstCapital",
    "FirstCapital",
    *(["SecondCapital"] * 27),
]


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
    """Reduce a complete first-sortie capture to player/projectile poses."""
    records: dict[
        int,
        tuple[int, tuple[int, ...], tuple[tuple[str, tuple[int, ...]], ...]],
    ] = {}
    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("mode") != "1"
            or "elapsed" not in values
            or "pose" not in values
            or "active" not in values
        ):
            continue
        elapsed = int(values["elapsed"])
        records[elapsed] = (
            int(values["mode"]),
            tuple(map(int, values["pose"].split(","))),
            raw_projectiles(values["active"]),
        )

    expected_frames = list(
        range(
            FIRST_COMPLETE_RETAIL_FRAME,
            LAST_COMPLETE_RETAIL_FRAME + 1,
            RETAIL_FRAME_STEP,
        )
    )
    missing = []
    compact = []
    for retail_frame in expected_frames:
        projectile_elapsed = PROJECTILE_SAMPLE_START_ELAPSED + retail_frame
        player_elapsed = PLAYER_SAMPLE_START_ELAPSED + retail_frame
        projectile_record = records.get(projectile_elapsed)
        player_record = records.get(player_elapsed)
        if projectile_record is None or player_record is None:
            missing.append(retail_frame)
            continue
        compact.append(
            (
                retail_frame,
                player_record[1],
                projectile_record[2],
            )
        )
    if missing:
        raise SystemExit(
            "first-sortie capture is not a complete four-frame cadence; "
            f"missing={missing[:8]}"
        )

    lines = [
        "# Compact laser-hold oracle evidence for first-sortie hostile projectiles.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Player and projectile cooperative slices are aligned to the same retail tick.",
        f"# certified_retail_frames={FIRST_COMPLETE_RETAIL_FRAME}..{LAST_COMPLETE_RETAIL_FRAME}",
    ]
    for retail_frame, player, projectiles in compact:
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


def validate_static_projectile_path() -> None:
    """Require the reviewed retail bytecode behind the semantic action model."""
    extractor = PathExtractor(Path(DEFAULT_ROM).read_bytes())
    semantic_names = {spec.opcode: spec.rust_name for spec in PATH_SEMANTICS}
    expected = {
        0xEE9B: ("SetVelocity", "063f"),
        0xEE9D: ("IfSelectedDistanceLess", "14e803adee"),
        0xEEA2: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA4: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA6: ("RotateAroundSelectedPitch", "ae7f"),
        0xEEA8: ("FaceSelectedImmediate", "000f"),
        0xEEAD: ("FaceSelectedImmediate", "000f"),
        0xEEAF: ("SetVelocity", "063f"),
        0xEEC6: ("Next", "44"),
        0xEED2: ("Wait", "030f"),
        0xEEDA: ("FaceSelectedSmooth", "09"),
    }
    for address, (semantic_name, raw_hex) in expected.items():
        command = extractor.decode_command(PathAddress(address))
        actual = (semantic_names.get(command.opcode), command.raw_hex)
        if actual != (semantic_name, raw_hex):
            raise SystemExit(
                f"opening projectile path changed at {address:04X}: "
                f"expected {(semantic_name, raw_hex)}, found {actual}"
            )


def validate_static_collision_gate() -> None:
    """Require the retail gate and source-level meaning behind eligibility."""
    rom = Path(DEFAULT_ROM).read_bytes()
    gate_address = 0x7F32CE
    expected_gate = bytes.fromhex("B5 21 29 01 00 D0 60")
    gate_offset = source_offset(gate_address)
    actual_gate = rom[gate_offset : gate_offset + len(expected_gate)]
    if actual_gate != expected_gate:
        raise SystemExit(
            "opening projectile collision-list gate changed at 7F:32CE: "
            f"expected {expected_gate.hex()}, found {actual_gate.hex()}"
        )

    path_source = (
        REPO_ROOT / "reference" / "ultrastarfox" / "SF" / "PATH" / "PATHS.ASM"
    ).read_text(encoding="latin-1")
    strategy_flags = (
        REPO_ROOT / "reference" / "ultrastarfox" / "SF" / "INC" / "STRATEQU.INC"
    ).read_text(encoding="latin-1")
    required_path_source = (
        ".collisionson SHORTA",
        "s_clr_alsflag\tx,colldisable",
        ".collisionsoff SHORTA",
        "s_set_alsflag\tx,colldisable",
    )
    if any(text not in path_source for text in required_path_source):
        raise SystemExit("reference collision enable/disable path semantics changed")
    if "make_sflag\tcolldisable" not in strategy_flags:
        raise SystemExit("reference collision-disable strategy flag changed")


def read_collision_eligibility(path: Path) -> list[bool]:
    """Read one semantic collision decision per chronological projectile."""
    eligibility = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("track="):
            continue
        values = fields(line)
        expected_track = len(eligibility)
        if int(values["track"]) != expected_track:
            raise SystemExit(
                "opening projectile collision fixture is not sequential: "
                f"expected track {expected_track}, found {values['track']}"
            )
        enabled = values.get("collision_enabled")
        if enabled not in ("true", "false"):
            raise SystemExit(
                f"opening projectile track {expected_track} has invalid "
                f"collision_enabled={enabled}"
            )
        eligibility.append(enabled == "true")
    if len(eligibility) != EXPECTED_PROJECTILE_LIFETIMES:
        raise SystemExit(
            f"expected {EXPECTED_PROJECTILE_LIFETIMES} collision entries, "
            f"found {len(eligibility)}"
        )
    return eligibility


def append_test_oracle(source: str, pose_fixture: Path) -> str:
    """Embed retained poses only for exhaustive native-runtime verification."""
    records, lifetimes = read_pose_fixture(
        pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        MAXIMUM_CONTINUOUS_POSITION_STEP,
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
    parser.add_argument(
        "--collision-fixture", type=Path, default=DEFAULT_COLLISION_FIXTURE
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw-pose", type=Path)
    parser.add_argument("--import-raw-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    validate_static_projectile_path()
    validate_static_collision_gate()
    if args.import_raw_pose is not None:
        import_raw_poses(args.import_raw_pose, args.pose_fixture)
    if args.import_raw_logic is not None:
        if not args.pose_fixture.exists():
            raise SystemExit("import projectile poses before importing logic")
        _, lifetimes = read_pose_fixture(
            args.pose_fixture,
            EXPECTED_PROJECTILE_LIFETIMES,
            MAXIMUM_CONTINUOUS_POSITION_STEP,
        )
        import_raw_logic(
            args.import_raw_logic,
            args.logic_fixture,
            PROJECTILE_SAMPLE_START_ELAPSED,
            "the opening sortie",
            frozenset(lifetime.source for lifetime in lifetimes),
        )

    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        PROJECTILE_SAMPLE_START_ELAPSED,
        "the retail opening sortie",
        allow_split_contractions=True,
        firing_actors=FIRING_ACTORS,
        maximum_continuous_position_step=MAXIMUM_CONTINUOUS_POSITION_STEP,
        collision_eligibility=read_collision_eligibility(args.collision_fixture),
    )
    source = append_test_oracle(source, args.pose_fixture)
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(
                f"generated first-sortie projectile dynamics are stale: {args.output}"
            )
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "first-sortie projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
