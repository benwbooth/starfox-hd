#!/usr/bin/env python3
"""Generate the recurring fighter's certified shared-RNG cadence.

The fighter behavior itself lives in typed Rust state. This fixture records
only the shared random generator state visible to each logic slice, because
other retail tasks interleave their own draws with the fighter task.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = ROOT / "tools/sf2/fixtures/pressure_fighter_survival_random.trace"
DEFAULT_OUTPUT = ROOT / "rust/sf2-game/src/native/pressure_fighter_survival.rs"

HANDOFF_RETAIL_FRAME = 7_168
FIRST_LIVE_RETAIL_FRAME = HANDOFF_RETAIL_FRAME + 4
CERTIFIED_END_RETAIL_FRAME = 7_780
RAW_ORIGIN_ELAPSED = 81_000
FIGHTER_OBJECT = "05F4"
FIGHTER_SHAPE = "F1C4"
RETAIL_FRAME_STEP = 4


def fields(line: str) -> dict[str, str]:
    return dict(field.split("=", 1) for field in line.split() if "=" in field)


def import_raw_logic(source: Path, output: Path) -> None:
    frames: dict[
        int,
        list[
            tuple[
                tuple[int, int, int, int],
                tuple[int, int, int],
                tuple[int, int, int, int, int, int, int],
                tuple[int, int, int, int, int, int, int],
                bool,
                tuple[int, int, int, int] | None,
            ]
        ],
    ] = {}
    pending_decision_state: tuple[int, int, int, int] | None = None
    pending_slice: tuple[
        int,
        tuple[int, int, int, int],
        tuple[int, int, int],
        tuple[int, int, int, int, int, int, int],
        tuple[int, int, int],
        bool,
    ] | None = None
    completed_slice: tuple[
        int,
        tuple[int, int, int, int],
        tuple[int, int, int],
        tuple[int, int, int, int, int, int, int],
        tuple[int, int, int, int, int, int, int],
        tuple[int, int, int],
        bool,
    ] | None = None
    random_calls_since_vertical: list[
        tuple[tuple[int, int, int, int] | None, int]
    ] = []
    last_completed_random_state: tuple[int, int, int, int] | None = None
    pending_fire_projectile = False

    def maneuver_targets(values: dict[str, str]) -> tuple[int, int, int]:
        extension = bytes.fromhex(values["extension"])

        def signed_word(offset: int) -> int:
            value = extension[offset] | (extension[offset + 1] << 8)
            return value - 65_536 if value >= 32_768 else value

        return signed_word(14), signed_word(16), signed_word(18)

    def finalize_completed_slice(next_targets: tuple[int, int, int]) -> None:
        nonlocal completed_slice, random_calls_since_vertical
        if completed_slice is None:
            return
        (
            retail_frame,
            state,
            player_position,
            control_pose,
            movement_pose,
            previous_targets,
            fire_projectile,
        ) = completed_slice
        retain_slice = FIRST_LIVE_RETAIL_FRAME <= retail_frame <= CERTIFIED_END_RETAIL_FRAME
        refresh_random_state = None
        if retain_slice and next_targets != previous_targets:
            candidates = []
            for index in range(len(random_calls_since_vertical) - 5):
                outputs = [
                    output
                    for _, output in random_calls_since_vertical[index : index + 6]
                ]
                altitude = (
                    (((outputs[0] << 8) | outputs[1]) & 8_191) - 4_096
                )
                generated_targets = (
                    outputs[3] - 128,
                    altitude,
                    outputs[5] - 128,
                )
                state_before_draws = random_calls_since_vertical[index][0]
                if generated_targets == next_targets and state_before_draws is not None:
                    candidates.append(state_before_draws)
            if len(candidates) != 1:
                raise SystemExit(
                    f"retail frame {retail_frame} target refresh has "
                    f"{len(candidates)} matching six-draw windows"
                )
            refresh_random_state = candidates[0]
        if retain_slice:
            frames.setdefault(retail_frame, []).append(
                (
                    state,
                    player_position,
                    control_pose,
                    movement_pose,
                    fire_projectile,
                    refresh_random_state,
                )
            )
        completed_slice = None
        random_calls_since_vertical = []

    for line in source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        event = values.get("event")
        if event == "random-state-write" and values.get("address") == "00E0":
            raw_state = tuple(bytes.fromhex(values["rng"]))
            completed_state = (int(values["value"]), *raw_state[1:])
            if completed_slice is not None:
                random_calls_since_vertical.append(
                    (last_completed_random_state, int(values["value"]))
                )
            last_completed_random_state = completed_state
            continue
        if (
            values.get("object") != FIGHTER_OBJECT
            or values.get("shape") != FIGHTER_SHAPE
        ):
            continue
        if event == "random-value":
            pending_decision_state = tuple(bytes.fromhex(values["rng"]))
            continue
        if event == "fire":
            pending_fire_projectile = True
            continue
        if event == "move":
            finalize_completed_slice(maneuver_targets(values))
            if pending_slice is not None:
                raise SystemExit("survival RNG capture has a move without its vertical step")
            elapsed = int(values["elapsed"])
            retail_frame = HANDOFF_RETAIL_FRAME + (
                (elapsed - RAW_ORIGIN_ELAPSED) // RETAIL_FRAME_STEP + 1
            ) * RETAIL_FRAME_STEP
            state = pending_decision_state or tuple(bytes.fromhex(values["rng"]))
            pending_decision_state = None
            selected_pose = tuple(map(int, values["selected_pose"].split(",")))
            control_pose = tuple(map(int, values["pose"].split(",")))
            pending_slice = (
                retail_frame,
                state,
                selected_pose[:3],
                control_pose,
                maneuver_targets(values),
                pending_fire_projectile,
            )
            pending_fire_projectile = False
            continue
        if event != "vertical-step" or pending_slice is None:
            continue
        (
            retail_frame,
            state,
            player_position,
            control_pose,
            targets,
            fire_projectile,
        ) = pending_slice
        movement_pose = tuple(map(int, values["pose"].split(",")))
        pending_slice = None
        completed_slice = (
            retail_frame,
            state,
            player_position,
            control_pose,
            movement_pose,
            targets,
            fire_projectile,
        )
        random_calls_since_vertical = []

    if pending_slice is not None and pending_slice[0] <= CERTIFIED_END_RETAIL_FRAME:
        raise SystemExit("survival RNG capture ends during a fighter movement slice")
    if completed_slice is not None and completed_slice[0] <= CERTIFIED_END_RETAIL_FRAME:
        raise SystemExit("survival RNG capture lacks a following fighter movement")

    expected_frames = list(
        range(
            FIRST_LIVE_RETAIL_FRAME,
            CERTIFIED_END_RETAIL_FRAME + 1,
            RETAIL_FRAME_STEP,
        )
    )
    if sorted(frames) != expected_frames:
        raise SystemExit("survival RNG capture is missing certified retail frames")
    for retail_frame in expected_frames:
        if not 1 <= len(frames[retail_frame]) <= 2:
            raise SystemExit(
                f"retail frame {retail_frame} has {len(frames[retail_frame])} "
                "logic slices; expected one or two"
            )

    lines = [
        "# Shared-RNG oracle evidence for the surviving recurring fighter.",
        f"# Raw source SHA-256: {hashlib.sha256(source.read_bytes()).hexdigest()}",
        "# Values are the four-byte generator state visible before each fighter decision.",
    ]
    for retail_frame in expected_frames:
        states = "/".join(
            ",".join(map(str, state))
            + "@"
            + ",".join(map(str, player_position))
            + ">"
            + ",".join(map(str, control_pose))
            + ">"
            + ",".join(map(str, movement_pose))
            + ">"
            + str(int(fire_projectile))
            + ">"
            + (
                "-"
                if refresh_random_state is None
                else ",".join(map(str, refresh_random_state))
            )
            for (
                state,
                player_position,
                control_pose,
                movement_pose,
                fire_projectile,
                refresh_random_state,
            ) in frames[retail_frame]
        )
        lines.append(f"retail_frame={retail_frame} slices={states}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def read_fixture(
    path: Path,
) -> list[
    list[
        tuple[
            tuple[int, int, int, int],
            tuple[int, int, int],
            tuple[int, int, int, int, int, int, int],
            tuple[int, int, int, int, int, int, int],
            bool,
            tuple[int, int, int, int] | None,
        ]
    ]
]:
    frames = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        retail_frame = int(values["retail_frame"])
        expected_frame = FIRST_LIVE_RETAIL_FRAME + len(frames) * RETAIL_FRAME_STEP
        if retail_frame != expected_frame:
            raise SystemExit(
                f"survival RNG frame {retail_frame} does not follow {expected_frame}"
            )
        slices = []
        for value in values["slices"].split("/"):
            state, value = value.split("@", 1)
            (
                player_position,
                control_pose,
                movement_pose,
                fire_projectile,
                refresh_state,
            ) = value.split(
                ">", 4
            )
            slices.append(
                (
                    tuple(map(int, state.split(","))),
                    tuple(map(int, player_position.split(","))),
                    tuple(map(int, control_pose.split(","))),
                    tuple(map(int, movement_pose.split(","))),
                    bool(int(fire_projectile)),
                    (
                        None
                        if refresh_state == "-"
                        else tuple(map(int, refresh_state.split(",")))
                    ),
                )
            )
        if not 1 <= len(slices) <= 2 or any(
            len(state) != 4
            or any(not 0 <= byte <= 255 for byte in state)
            or len(player_position) != 3
            or any(not -32_768 <= coordinate <= 32_767 for coordinate in player_position)
            or len(control_pose) != 7
            or len(movement_pose) != 7
            or any(not -32_768 <= value <= 32_767 for value in control_pose)
            or any(not -32_768 <= value <= 32_767 for value in movement_pose)
            or (
                refresh_state is not None
                and (
                    len(refresh_state) != 4
                    or any(not 0 <= byte <= 255 for byte in refresh_state)
                )
            )
            for (
                state,
                player_position,
                control_pose,
                movement_pose,
                fire_projectile,
                refresh_state,
            ) in slices
        ):
            raise SystemExit(f"malformed survival RNG states at frame {retail_frame}")
        frames.append(slices)
    expected_count = (
        (CERTIFIED_END_RETAIL_FRAME - FIRST_LIVE_RETAIL_FRAME)
        // RETAIL_FRAME_STEP
        + 1
    )
    if len(frames) != expected_count:
        raise SystemExit(
            f"survival RNG fixture has {len(frames)} frames; expected {expected_count}"
        )
    return frames


def render(
    frames: list[
        list[
            tuple[
                tuple[int, int, int, int],
                tuple[int, int, int],
                tuple[int, int, int, int, int, int, int],
                tuple[int, int, int, int, int, int, int],
                bool,
                tuple[int, int, int, int] | None,
            ]
        ]
    ],
) -> str:
    lines = [
        "// @generated by tools/sf2/generate_pressure_fighter_survival.py",
        "// Tests replay this oracle-certified shared-RNG interleaving.",
        "",
        (
            "pub const RETAINED_ORACLE_SLICE_COUNT: usize = "
            f"{sum(map(len, frames))};"
        ),
        "pub const HANDOFF_LOGIC_CREDIT: u8 = 2;",
        "pub const HANDOFF_VERTICAL_WAVE_PHASE: u8 = 48;",
        "pub const HANDOFF_STRAIGHT_TICKS_ELAPSED: u8 = 20;",
        "pub const HANDOFF_MANEUVER_DRIFT_X: i16 = 30;",
        "pub const HANDOFF_MANEUVER_ALTITUDE_TARGET: i16 = -517;",
        "pub const HANDOFF_MANEUVER_DRIFT_Z: i16 = -52;",
        "pub const HANDOFF_POSE: FighterOraclePose = FighterOraclePose {",
        "    position: [29_458, -41, -3_959],",
        "    pitch: 0,",
        "    yaw: 79,",
        "    roll: 0,",
        "    speed: 60,",
        "};",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct FighterOraclePose {",
        "    pub position: [i16; 3],",
        "    pub pitch: u8,",
        "    pub yaw: u8,",
        "    pub roll: u8,",
        "    pub speed: u8,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct FighterOracleSlice {",
        "    pub random_state: [u8; 4],",
        "    pub player_position: [i16; 3],",
        "    pub control_pose: FighterOraclePose,",
        "    pub movement_pose: FighterOraclePose,",
        "    pub fire_projectile: bool,",
        "    pub refresh_random_state: Option<[u8; 4]>,",
        "}",
        "",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub struct FighterOracleFrame {",
        "    slices: [FighterOracleSlice; 2],",
        "    count: u8,",
        "}",
        "",
        "impl FighterOracleFrame {",
        "    pub fn slices(&self) -> &[FighterOracleSlice] {",
        "        &self.slices[..usize::from(self.count)]",
        "    }",
        "}",
        "",
        (
            "pub static ORACLE_FRAMES: "
            f"[FighterOracleFrame; {len(frames)}] = ["
        ),
    ]
    for slices in frames:
        empty_pose = (0, 0, 0, 0, 0, 0, 0)
        padded = slices + [
            ((0, 0, 0, 0), (0, 0, 0), empty_pose, empty_pose, False, None)
        ] * (2 - len(slices))
        lines.extend(
            [
                "    FighterOracleFrame {",
                "        slices: [",
                "            FighterOracleSlice {",
                f"                random_state: [{', '.join(map(str, padded[0][0]))}],",
                f"                player_position: [{', '.join(map(str, padded[0][1]))}],",
                *rust_pose("                control_pose", padded[0][2]),
                *rust_pose("                movement_pose", padded[0][3]),
                f"                fire_projectile: {str(padded[0][4]).lower()},",
                (
                    "                refresh_random_state: "
                    f"{rust_optional_state(padded[0][5])},"
                ),
                "            },",
                "            FighterOracleSlice {",
                f"                random_state: [{', '.join(map(str, padded[1][0]))}],",
                f"                player_position: [{', '.join(map(str, padded[1][1]))}],",
                *rust_pose("                control_pose", padded[1][2]),
                *rust_pose("                movement_pose", padded[1][3]),
                f"                fire_projectile: {str(padded[1][4]).lower()},",
                (
                    "                refresh_random_state: "
                    f"{rust_optional_state(padded[1][5])},"
                ),
                "            },",
                "        ],",
                f"        count: {len(slices)},",
                "    },",
            ]
        )
    lines.extend(
        [
            "];",
            "",
        ]
    )
    return "\n".join(lines)


def rust_pose(name: str, pose: tuple[int, ...]) -> list[str]:
    x, y, z, pitch, yaw, roll, speed = pose
    return [
        f"{name}: FighterOraclePose {{",
        f"                    position: [{x}, {y}, {z}],",
        f"                    pitch: {pitch},",
        f"                    yaw: {yaw},",
        f"                    roll: {roll},",
        f"                    speed: {speed},",
        "                },",
    ]


def rust_optional_state(state: tuple[int, ...] | None) -> str:
    if state is None:
        return "None"
    return f"Some([{', '.join(map(str, state))}])"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-logic", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_logic is not None:
        import_raw_logic(args.import_logic, args.fixture)
    rendered = render(read_fixture(args.fixture))
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != rendered:
            raise SystemExit(f"generated survival RNG module is stale: {args.output}")
        return
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")


if __name__ == "__main__":
    main()
