#!/usr/bin/env python3
"""Compact and verify Fortuna Kick Gunner behavior evidence.

The compact fixture retains only semantic world-space poses from the retail
oracle. Static verification independently checks the complete path graph
around the guardian, its linked mount, and the fired projectile.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_OUTPUT = Path(__file__).with_name("fixtures") / "fortuna_kick_gunner.trace"
RUST_SOURCE = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "fortuna_base.rs"
DISASM_DIR = Path(__file__).with_name("disasm")
sys.path.insert(0, str(DISASM_DIR))

from extract_map import DEFAULT_ROM  # noqa: E402
from extract_path import PathAddress, PathExtractor  # noqa: E402
from path_semantics import PATH_SEMANTICS  # noqa: E402


GUARDIAN_PATTERN = re.compile(r"(?:objects=\[|;)(0AA1,EA1C,[^;\]]+)")
MOUNT_PATTERN = re.compile(r"(?:objects=\[|;)(07EC,BECC,[^;\]]+)")
ELAPSED_PATTERN = re.compile(r"\belapsed=(\d+)\b")

GUARDIAN_SAMPLES = (
    ("waiting", 100_521),
    ("floor_descent", 100_758),
    ("floor_descent", 100_764),
    ("long_dive", 100_770),
    ("long_dive", 100_776),
    ("long_dive", 100_782),
    ("long_dive", 100_788),
    ("long_dive", 100_792),
    ("long_dive", 100_800),
    ("long_dive", 100_808),
    ("long_dive", 100_812),
    ("long_dive", 100_820),
    ("long_dive", 100_824),
    ("long_dive", 100_832),
    ("long_dive", 100_836),
    ("long_dive", 100_844),
    ("long_dive", 100_848),
    ("long_dive", 100_856),
    ("long_dive", 100_860),
    ("long_dive", 100_868),
    ("long_dive", 100_872),
    ("long_dive", 100_880),
    ("long_dive", 100_884),
    ("surface_bob", 100_892),
    ("surface_bob", 100_900),
    ("surface_bob", 100_904),
    ("surface_bob", 100_912),
    ("surface_bob", 100_916),
    ("attack_preparation_bob", 100_988),
    ("attack_preparation_bob", 100_992),
    ("attack_preparation_bob", 101_000),
    ("attack_preparation_bob", 101_004),
    ("attack_preparation_bob", 101_012),
    ("attack_leap", 101_020),
    ("attack_leap", 101_024),
    ("attack_leap", 101_032),
    ("attack_leap", 101_040),
    ("attack_leap", 101_044),
    ("attack_leap", 101_052),
    ("attack_leap", 101_060),
    ("attack_leap", 101_064),
    ("attack_leap", 101_072),
    ("attack_leap", 101_080),
    ("attack_leap", 101_084),
    ("attack_leap", 101_092),
    ("attack_leap", 101_100),
    ("attack_leap", 101_108),
    ("attack_recovery_bob", 101_112),
    ("attack_recovery_bob", 101_120),
    ("attack_recovery_bob", 101_128),
    ("attack_recovery_bob", 101_132),
    ("attack_recovery_bob", 101_140),
)
MOUNT_SAMPLE_ELAPSED = 101_168

EXPECTED_LONG_DIVE_VERTICAL_STEPS = (
    -200,
    -81,
    -64,
    -49,
    -36,
    -25,
    -16,
    -9,
    -4,
    -1,
    1,
    4,
    9,
    16,
    25,
    36,
    49,
    64,
    81,
    200,
)
EXPECTED_SURFACE_BOB_STEPS = (8, 12, 4, -4, -20)
EXPECTED_ATTACK_VERTICAL_STEPS = (-98, -72, -50, -32, -18, -8, -2, 2, 8, 18, 32, 50, 72, 98)
EXPECTED_LONG_DIVE_FORWARD_STEP = 33
EXPECTED_ATTACK_FORWARD_STEP = 23
EXPECTED_MOUNT_OFFSET = (-42, 0, 27)
EXPECTED_LONG_DIVE_ANIMATION_FRAMES = (
    7,
    8,
    9,
    9,
    9,
    9,
    9,
    9,
    10,
    11,
    10,
    9,
    8,
    7,
    6,
    5,
    4,
    3,
    2,
    1,
)
EXPECTED_ATTACK_ANIMATION_FRAMES = (6, 7, 8, 9, 10, 8, 7, 6, 5, 4, 3, 2, 1, 0)
EXPECTED_RETREAT_CORNERS = (1, 3, 0, 2, 1, 3, 0, 2)
EXPECTED_ROUTE_YAWS = (0, 64, 128, 64, 192, 128, 192, 0)
EXPECTED_CORNER_POSITIONS = (
    (1_280, 100, -1_280),
    (1_280, 100, 1_280),
    (-1_280, 100, 1_280),
    (-1_280, 100, -1_280),
)

# File offsets into the headerless retail ROM. Hexadecimal is appropriate here
# because these are source addresses used only by this oracle verifier.
LONG_DIVE_ANIMATION_TABLE_OFFSET = 0x37D10
ATTACK_ANIMATION_TABLE_OFFSET = 0x37D27
RETREAT_CORNER_TABLE_OFFSET = 0x37D35
ROUTE_YAW_TABLE_OFFSET = 0x37D3D
CORNER_Z_TABLE_OFFSET = 0x37E35
CORNER_X_TABLE_OFFSET = 0x37E37


def parse_object(encoded: str) -> dict[str, object]:
    fields = encoded.split(",")
    if len(fields) < 16:
        raise ValueError(f"truncated retail object record: {encoded}")
    return {
        "position": tuple(int(value) for value in fields[2:5]),
        "yaw": int(fields[6]),
        "speed": int(fields[8]),
        "velocity": tuple(int(value) for value in fields[12:15]),
        "durability": int(fields[15]),
    }


def parse_raw_trace(path: Path) -> tuple[dict[int, dict[str, object]], dict[str, object]]:
    guardian_records: dict[int, dict[str, object]] = {}
    mount_record = None
    requested = {elapsed for _, elapsed in GUARDIAN_SAMPLES} | {MOUNT_SAMPLE_ELAPSED}
    for line in path.read_text(encoding="utf-8").splitlines():
        elapsed_match = ELAPSED_PATTERN.search(line)
        if elapsed_match is None:
            continue
        elapsed = int(elapsed_match.group(1))
        if elapsed in requested:
            guardian_match = GUARDIAN_PATTERN.search(line)
            if guardian_match is not None:
                guardian_records[elapsed] = parse_object(guardian_match.group(1))
        if elapsed == MOUNT_SAMPLE_ELAPSED:
            mount_match = MOUNT_PATTERN.search(line)
            if mount_match is not None:
                mount_record = parse_object(mount_match.group(1))
    missing = sorted(requested - guardian_records.keys())
    if missing:
        raise ValueError(f"raw trace is missing guardian samples: {missing}")
    if mount_record is None:
        raise ValueError("raw trace is missing the linked mount spawn")
    return guardian_records, mount_record


def render_fixture(raw_path: Path) -> str:
    guardian_records, mount_record = parse_raw_trace(raw_path)
    baseline = GUARDIAN_SAMPLES[0][1]
    lines = [
        "# Generated by tools/sf2/generate_fortuna_kick_gunner.py.",
        f"# Raw trace SHA-256: {hashlib.sha256(raw_path.read_bytes()).hexdigest()}",
        "# Source object identities and path addresses were removed after reduction.",
    ]
    for phase, elapsed in GUARDIAN_SAMPLES:
        record = guardian_records[elapsed]
        lines.append(
            "guardian "
            f"retail_frame={elapsed - baseline} "
            f"phase={phase} "
            f"position={','.join(str(value) for value in record['position'])} "
            f"yaw={record['yaw']} "
            f"speed={record['speed']} "
            f"velocity={','.join(str(value) for value in record['velocity'])} "
            f"durability={record['durability']}"
        )
    guardian = guardian_records[MOUNT_SAMPLE_ELAPSED]
    mount_position = mount_record["position"]
    offset = tuple(
        mount_position[index] - guardian["position"][index] for index in range(3)
    )
    lines.append(
        "mount "
        f"retail_frame={MOUNT_SAMPLE_ELAPSED - baseline} "
        f"position={','.join(str(value) for value in mount_position)} "
        f"parent_offset={','.join(str(value) for value in offset)} "
        f"yaw={mount_record['yaw']} "
        f"durability={mount_record['durability']}"
    )
    lines.append("summary attack_count=5 shots_per_attack=1")
    return "\n".join(lines) + "\n"


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def position(record: dict[str, str]) -> tuple[int, int, int]:
    return tuple(int(value) for value in record["position"].split(","))


def phase_records(contents: str, phase: str) -> list[dict[str, str]]:
    return [
        fields(line)
        for line in contents.splitlines()
        if line.startswith("guardian ") and f"phase={phase}" in line
    ]


def deltas(records: list[dict[str, str]], initial_y: int) -> tuple[int, ...]:
    values = []
    previous = initial_y
    for record in records:
        current = position(record)[1]
        values.append(current - previous)
        previous = current
    return tuple(values)


def validate_fixture(contents: str) -> None:
    long_dive = phase_records(contents, "long_dive")
    surface_bob = phase_records(contents, "surface_bob")
    attack_leap = phase_records(contents, "attack_leap")
    if deltas(long_dive, -100) != EXPECTED_LONG_DIVE_VERTICAL_STEPS:
        raise ValueError("oracle long-dive parabola changed")
    if deltas(surface_bob, -100) != EXPECTED_SURFACE_BOB_STEPS:
        raise ValueError("oracle surface bob changed")
    if deltas(attack_leap, -100) != EXPECTED_ATTACK_VERTICAL_STEPS:
        raise ValueError("oracle attack-leap parabola changed")
    if any(
        record["durability"] != "70"
        for record in long_dive + surface_bob + attack_leap
    ):
        raise ValueError("guardian durability changed during the passive oracle replay")
    long_z = [position(record)[2] for record in long_dive]
    long_forward_steps = tuple(
        right - left for left, right in zip([-1_280] + long_z, long_z)
    )
    if long_forward_steps != (EXPECTED_LONG_DIVE_FORWARD_STEP,) * 19 + (0,):
        raise ValueError("oracle long-dive forward velocity changed")
    attack_z = [position(record)[2] for record in attack_leap]
    attack_forward_steps = tuple(
        right - left for left, right in zip([-653] + attack_z, attack_z)
    )
    if attack_forward_steps != (EXPECTED_ATTACK_FORWARD_STEP,) * 13 + (0,):
        raise ValueError("oracle attack forward velocity changed")
    mount = next(
        (fields(line) for line in contents.splitlines() if line.startswith("mount ")),
        None,
    )
    if mount is None:
        raise ValueError("fixture has no linked mount")
    if tuple(int(value) for value in mount["parent_offset"].split(",")) != EXPECTED_MOUNT_OFFSET:
        raise ValueError("linked mount offset changed")
    if mount["durability"] != "10":
        raise ValueError("linked mount durability changed")
    if "summary attack_count=5 shots_per_attack=1" not in contents:
        raise ValueError("fixture has an unexpected attack summary")


def command_map() -> tuple[dict[int, object], dict[int, str]]:
    result = PathExtractor(Path(DEFAULT_ROM).read_bytes()).extract()
    commands = {command.address.offset: command for command in result.commands}
    names = {spec.opcode: spec.rust_name for spec in PATH_SEMANTICS}
    return commands, names


def validate_static_paths() -> None:
    commands, names = command_map()
    expected = {
        0x3506: ("DoQueue", "6105"),
        0x3519: ("DoQueue", "610e"),
        0x3510: ("SetVelocity", "0619"),
        0x353E: ("Wait", "0303"),
        0x3540: ("SpawnChild", "f5ccbe368c0a0a0000000032000b"),
        0x354E: ("Wait", "0302"),
        0x3550: ("Next", "44"),
        0x355A: ("DoQueue", "6114"),
        0x3585: ("SetRandomByte", "58a103"),
        0x3594: ("SetRandomByte", "58a201"),
        0x35D2: ("SetVelocity", "0623"),
        0x35D6: ("DoQueue", "6114"),
        0x35FA: ("DoQueue", "6105"),
        0x8C36: ("Sprite", "4d0008"),
        0x8C3B: ("SetWeapon", "391a"),
        0x8C3F: ("DoQueue", "610a"),
        0x8C41: ("AddByte", "079902"),
        0x8C44: ("Next", "44"),
        0x8C45: ("FaceSelectedImmediate", "000f"),
        0x8C48: ("FireWeapon", "35"),
        0xECB4: ("SetByte", "0b782d"),
        0xECB7: ("SetWord", "0c6cc104"),
        0xECBE: ("Sprite", "4d0005"),
        0xECD4: ("SetVelocity", "0614"),
        0xECD7: ("Wait", "0303"),
        0xECE1: ("Wait", "0332"),
    }
    for address, (expected_name, expected_raw) in expected.items():
        command = commands.get(address)
        if command is None:
            raise ValueError(f"retail path command {address:04X} is no longer reachable")
        actual_name = names.get(command.opcode)
        if (actual_name, command.raw_hex) != (expected_name, expected_raw):
            raise ValueError(
                f"retail path command {address:04X} changed: "
                f"{actual_name} {command.raw_hex}"
            )
    mount_next = commands[0x8C44]
    if mount_next.successors != (PathAddress(0x8C45),) or tuple(
        effect.kind for effect in mount_next.effects
    ) != ("advance", "dynamic_jump"):
        raise ValueError("mount settling loop no longer falls through to one fire command")


def signed_words(data: bytes) -> tuple[int, ...]:
    return tuple(
        int.from_bytes(data[index : index + 2], "little", signed=True)
        for index in range(0, len(data), 2)
    )


def validate_static_tables() -> None:
    rom = Path(DEFAULT_ROM).read_bytes()
    animation_bias = 128
    table_checks = (
        (
            LONG_DIVE_ANIMATION_TABLE_OFFSET,
            tuple(value + animation_bias for value in EXPECTED_LONG_DIVE_ANIMATION_FRAMES),
            "long-dive animation",
        ),
        (
            ATTACK_ANIMATION_TABLE_OFFSET,
            tuple(value + animation_bias for value in EXPECTED_ATTACK_ANIMATION_FRAMES),
            "attack animation",
        ),
        (RETREAT_CORNER_TABLE_OFFSET, EXPECTED_RETREAT_CORNERS, "retreat corner"),
        (ROUTE_YAW_TABLE_OFFSET, EXPECTED_ROUTE_YAWS, "route yaw"),
    )
    for offset, expected, description in table_checks:
        actual = tuple(rom[offset : offset + len(expected)])
        if actual != expected:
            raise ValueError(f"retail {description} table changed")

    coordinate_byte_count = len(EXPECTED_CORNER_POSITIONS) * 2
    corner_x = signed_words(
        rom[CORNER_X_TABLE_OFFSET : CORNER_X_TABLE_OFFSET + coordinate_byte_count]
    )
    corner_z = signed_words(
        rom[CORNER_Z_TABLE_OFFSET : CORNER_Z_TABLE_OFFSET + coordinate_byte_count]
    )
    if tuple(zip(corner_x, (100,) * len(corner_x), corner_z)) != EXPECTED_CORNER_POSITIONS:
        raise ValueError("retail corner-position tables changed")


def rust_array(source: str, name: str) -> tuple[int, ...]:
    match = re.search(
        rf"const {name}: \[[^;]+;\s*\d+\]\s*=\s*\[(.*?)\];",
        source,
        re.DOTALL,
    )
    if match is None:
        raise ValueError(f"Rust source is missing {name}")
    return tuple(int(token.replace("_", "")) for token in re.findall(r"-?\d[\d_]*", match.group(1)))


def rust_angle_array(source: str, name: str) -> tuple[int, ...]:
    match = re.search(
        rf"const {name}: \[[^;]+;\s*\d+\]\s*=\s*\[(.*?)\];",
        source,
        re.DOTALL,
    )
    if match is None:
        raise ValueError(f"Rust source is missing {name}")
    values = []
    for token in match.group(1).split(","):
        token = token.strip()
        if not token:
            continue
        if token == "Angle::ZERO":
            values.append(0)
        elif token == "Angle::HALF_TURN":
            values.append(128)
        else:
            units = re.fullmatch(r"Angle::from_units\((\d+)\)", token)
            if units is None:
                raise ValueError(f"unrecognized Rust angle in {name}: {token}")
            values.append(int(units.group(1)))
    return tuple(values)


def rust_scalar(source: str, name: str) -> int:
    match = re.search(rf"const {name}: [^=]+=\s*(\d[\d_]*)\s*;", source)
    if match is None:
        raise ValueError(f"Rust source is missing {name}")
    return int(match.group(1).replace("_", ""))


def rust_vector(source: str, name: str) -> tuple[int, int, int]:
    match = re.search(
        rf"const {name}: Vector3\s*=\s*Vector3\s*\{{(.*?)\}};",
        source,
        re.DOTALL,
    )
    if match is None:
        raise ValueError(f"Rust source is missing {name}")
    values = []
    for field in ("x", "y", "z"):
        value = re.search(rf"\b{field}:\s*(-?\d[\d_]*)", match.group(1))
        if value is None:
            raise ValueError(f"Rust source has a non-literal {name}.{field}")
        values.append(int(value.group(1).replace("_", "")))
    return tuple(values)


def rust_corner_positions(source: str) -> tuple[tuple[int, int, int], ...]:
    match = re.search(
        r"const KICK_GUNNER_CORNER_POSITIONS: \[[^;]+;\s*\d+\]\s*=\s*\[(.*?)\];",
        source,
        re.DOTALL,
    )
    if match is None or "KICK_GUNNER_INITIAL_POSITION" not in match.group(1):
        raise ValueError("Rust source is missing the Kick Gunner corner positions")
    positions = [rust_vector(source, "KICK_GUNNER_INITIAL_POSITION")]
    resting_y = rust_scalar(source, "KICK_GUNNER_RESTING_Y")
    for body in re.findall(r"Vector3\s*\{(.*?)\}", match.group(1), re.DOTALL):
        coordinates = []
        for field in ("x", "z"):
            value = re.search(rf"\b{field}:\s*(-?\d[\d_]*)", body)
            if value is None:
                raise ValueError(f"Rust corner has a non-literal {field} coordinate")
            coordinates.append(int(value.group(1).replace("_", "")))
        positions.append((coordinates[0], resting_y, coordinates[1]))
    return tuple(positions)


def validate_rust_translation() -> None:
    source = RUST_SOURCE.read_text(encoding="utf-8")
    arrays = {
        "KICK_GUNNER_LONG_DIVE_VERTICAL_STEPS": EXPECTED_LONG_DIVE_VERTICAL_STEPS,
        "KICK_GUNNER_SURFACE_BOB_STEPS": EXPECTED_SURFACE_BOB_STEPS,
        "KICK_GUNNER_ATTACK_VERTICAL_STEPS": EXPECTED_ATTACK_VERTICAL_STEPS,
        "KICK_GUNNER_LONG_DIVE_ANIMATION_FRAMES": EXPECTED_LONG_DIVE_ANIMATION_FRAMES,
        "KICK_GUNNER_ATTACK_ANIMATION_FRAMES": EXPECTED_ATTACK_ANIMATION_FRAMES,
        "KICK_GUNNER_RETREAT_CORNERS": EXPECTED_RETREAT_CORNERS,
    }
    for name, expected in arrays.items():
        if rust_array(source, name) != expected:
            raise ValueError(f"shipping Rust {name} differs from oracle evidence")
    scalars = {
        "KICK_GUNNER_ACTION_RETAIL_FRAMES": 6,
        "KICK_GUNNER_FLOOR_DESCENT_ACTION_COUNT": 2,
        "KICK_GUNNER_LONG_DIVE_SPEED": 35,
        "KICK_GUNNER_ATTACK_SPEED": 25,
        "KICK_GUNNER_ATTACK_COUNT": 5,
        "KICK_GUNNER_CORNER_RANDOM_MASK": 3,
        "KICK_GUNNER_DIRECTION_RANDOM_MASK": 1,
        "KICK_GUNNER_ROUTES_PER_CORNER": 2,
        "KICK_GUNNER_MOUNT_OFFSET": 50,
        "KICK_GUNNER_MOUNT_DURABILITY": 10,
        "KICK_GUNNER_MOUNT_SETTLE_ACTIONS": 9,
        "KICK_GUNNER_PROJECTILE_SPEED": 20,
        "KICK_GUNNER_PROJECTILE_POSITION_SCALE": 4,
        "KICK_GUNNER_PROJECTILE_DURABILITY": 120,
        "KICK_GUNNER_PROJECTILE_ATTACK_POWER": 4,
        "KICK_GUNNER_PROJECTILE_LIFETIME_RETAIL_FRAMES": 220,
    }
    for name, expected in scalars.items():
        if rust_scalar(source, name) != expected:
            raise ValueError(f"shipping Rust {name} differs from static retail behavior")
    if rust_angle_array(source, "KICK_GUNNER_ROUTE_YAWS") != EXPECTED_ROUTE_YAWS:
        raise ValueError("shipping Rust route yaws differ from the retail table")
    if rust_corner_positions(source) != EXPECTED_CORNER_POSITIONS:
        raise ValueError("shipping Rust corner positions differ from the retail tables")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        generated = render_fixture(args.import_raw)
        validate_fixture(generated)
        if args.check:
            if args.output.read_text(encoding="utf-8") != generated:
                raise SystemExit(f"compact fixture is out of date: {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(generated, encoding="utf-8")
    else:
        validate_fixture(args.output.read_text(encoding="utf-8"))
    validate_static_paths()
    validate_static_tables()
    validate_rust_translation()


if __name__ == "__main__":
    main()
