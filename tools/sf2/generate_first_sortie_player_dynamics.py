#!/usr/bin/env python3
"""Import the first-sortie player cadence from the retail oracle.

The compact fixture retains presentation-boundary control and movement counts
plus natural damage impulses. Shipping Rust consumes those semantic events and
the statically recovered controller rules; the detailed source-machine trace
is verification input only.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


DEFAULT_ACTIVE_TRACE = Path(__file__).with_name("fixtures") / "first_sortie_neutral.trace"
DEFAULT_OUTPUT = (
    Path(__file__).with_name("fixtures") / "first_sortie_player_dynamics.trace"
)
ANCHOR_RETAIL_FRAME = 900
RETAIL_FRAME_STEP = 4
CAMERA_FOLLOW_LAST_RETAIL_FRAME = 7_840
CAMERA_RETURN_LAST_RETAIL_FRAME = 8_052
PLAYER_VELOCITY = (18, 0, 21)
PLAYER_PITCH = 0
PLAYER_YAW = 227
PLAYER_SPEED = 30
PLAYER_HIT_BANK_IMPULSE = 30


@dataclass(frozen=True)
class PlayerSample:
    elapsed: int
    camera: tuple[int, ...]
    pose: tuple[int, ...]


@dataclass(frozen=True)
class ControlEvent:
    elapsed: int
    presented_bank: int
    damage_bank_impulse: int
    velocity: tuple[int, ...]
    speed: int


@dataclass(frozen=True)
class CameraEvent:
    elapsed: int
    camera: tuple[int, ...]
    player_pose: tuple[int, ...]


@dataclass(frozen=True)
class Cadence:
    retail_frame: int
    control_updates: int
    movement_updates: int
    damage_bank_impulse: int
    camera_updates: int
    camera_uses_previous_player_position: bool


def fields(line: str) -> dict[str, str]:
    return {
        token.split("=", 1)[0]: token.split("=", 1)[1]
        for token in line.split()
        if "=" in token
    }


def parse_tuple(value: str, length: int, label: str) -> tuple[int, ...]:
    result = tuple(map(int, value.split(",")))
    if len(result) != length:
        raise SystemExit(f"{label} needs {length} values, found {len(result)}")
    return result


def signed_word(value: int) -> int:
    value &= 65_535
    return value - 65_536 if value >= 32_768 else value


def player_samples(active_trace: Path) -> list[PlayerSample]:
    result = []
    for line in active_trace.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed="):
            continue
        values = fields(line)
        result.append(
            PlayerSample(
                elapsed=int(values["elapsed"]),
                camera=parse_tuple(values["camera"], 6, "camera"),
                pose=parse_tuple(values["pose"], 7, "player pose"),
            )
        )
    if len(result) < 2:
        raise SystemExit("active trace has no complete first-sortie player path")
    if any(
        later.elapsed - earlier.elapsed != RETAIL_FRAME_STEP
        for earlier, later in zip(result, result[1:])
    ):
        raise SystemExit("active trace is not sampled every four retail frames")
    return result


def import_raw(active_trace: Path, dynamics_trace: Path) -> list[Cadence]:
    samples = player_samples(active_trace)
    controls = []
    movements = []
    camera_anchors = []
    camera_motion_outputs = []
    for line in dynamics_trace.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        stage = values.get("stage")
        if stage == "after-control":
            bank = parse_tuple(values["bank"], 6, "player bank components")
            controls.append(
                ControlEvent(
                    elapsed=int(values["elapsed"]),
                    presented_bank=(bank[2] + bank[5]) & 255,
                    damage_bank_impulse=bank[5],
                    velocity=parse_tuple(values["motion"], 3, "player velocity"),
                    speed=int(values["speed"]),
                )
            )
        elif stage == "after-motion":
            movements.append(int(values["elapsed"]))
        elif stage == "camera_anchor_applied_b":
            camera_anchors.append(
                CameraEvent(
                    elapsed=int(values["elapsed"]),
                    camera=parse_tuple(values["camera"], 7, "camera output")[:6],
                    player_pose=parse_tuple(values["pose"], 7, "camera player pose"),
                )
            )
        elif stage == "camera_motion_applied":
            camera_motion_outputs.append(
                CameraEvent(
                    elapsed=int(values["elapsed"]),
                    camera=parse_tuple(values["camera"], 7, "camera output")[:6],
                    player_pose=parse_tuple(values["pose"], 7, "camera player pose"),
                )
            )

    first_elapsed = samples[0].elapsed
    last_elapsed = samples[-1].elapsed
    controls = [
        event for event in controls if first_elapsed <= event.elapsed < last_elapsed
    ]
    movements = [
        elapsed for elapsed in movements if first_elapsed <= elapsed < last_elapsed
    ]
    camera_anchors = [
        event
        for event in camera_anchors
        if first_elapsed <= event.elapsed < last_elapsed
    ]
    camera_motion_outputs = [
        event
        for event in camera_motion_outputs
        if first_elapsed <= event.elapsed < last_elapsed
    ]
    if not controls or not movements:
        raise SystemExit("dynamics trace has no first-sortie control and movement events")
    if not camera_anchors:
        raise SystemExit("dynamics trace has no first-sortie camera anchor events")
    if not camera_motion_outputs:
        raise SystemExit("dynamics trace has no first-sortie camera motion events")
    if any(
        event.velocity != PLAYER_VELOCITY or event.speed != PLAYER_SPEED
        for event in controls
    ):
        raise SystemExit("first-sortie player motion is not the recovered neutral velocity")

    result = []
    previous_elapsed = first_elapsed
    previous_pose = samples[0].pose
    previous_damage_impulse = 0
    pending_controls = 0
    for index, sample in enumerate(samples):
        if index == 0:
            window_controls: list[ControlEvent] = []
            window_movements: list[int] = []
            window_camera_events: list[CameraEvent] = []
        else:
            window_controls = [
                event
                for event in controls
                if previous_elapsed <= event.elapsed < sample.elapsed
            ]
            window_movements = [
                elapsed
                for elapsed in movements
                if previous_elapsed <= elapsed < sample.elapsed
            ]
            retail_frame = ANCHOR_RETAIL_FRAME + index * RETAIL_FRAME_STEP
            camera_events = (
                camera_anchors
                if retail_frame <= CAMERA_FOLLOW_LAST_RETAIL_FRAME
                else camera_motion_outputs
            )
            window_camera_events = [
                event
                for event in camera_events
                if previous_elapsed <= event.elapsed < sample.elapsed
            ]

        control_updates = len(window_controls)
        movement_updates = len(window_movements)
        camera_updates = len(window_camera_events)
        if control_updates > 2 or movement_updates > 2 or camera_updates > 2:
            raise SystemExit(
                f"source cadence is out of range at retail frame "
                f"{ANCHOR_RETAIL_FRAME + index * RETAIL_FRAME_STEP}"
            )
        pending_controls += control_updates - movement_updates
        if pending_controls not in (0, 1):
            raise SystemExit(
                f"control/movement ordering diverges at retail frame "
                f"{ANCHOR_RETAIL_FRAME + index * RETAIL_FRAME_STEP}"
            )

        expected_position = (
            signed_word(previous_pose[0] + PLAYER_VELOCITY[0] * movement_updates),
            signed_word(previous_pose[1] + PLAYER_VELOCITY[1] * movement_updates),
            signed_word(previous_pose[2] + PLAYER_VELOCITY[2] * movement_updates),
        )
        if sample.pose[:3] != expected_position:
            raise SystemExit(
                f"movement events do not reproduce the player position at elapsed "
                f"{sample.elapsed}: expected {expected_position}, found {sample.pose[:3]}"
            )
        if sample.pose[3] != PLAYER_PITCH or sample.pose[4] != PLAYER_YAW:
            raise SystemExit("first-sortie neutral orientation changed")
        if sample.pose[6] != PLAYER_SPEED:
            raise SystemExit("first-sortie neutral speed changed")
        expected_bank = (
            window_controls[-1].presented_bank
            if window_controls
            else previous_pose[5]
        )
        if sample.pose[5] != expected_bank:
            raise SystemExit(
                f"control events do not reproduce player bank at elapsed "
                f"{sample.elapsed}: expected {expected_bank}, found {sample.pose[5]}"
            )

        retail_frame = ANCHOR_RETAIL_FRAME + index * RETAIL_FRAME_STEP
        camera_uses_previous_player_position = False
        if window_camera_events:
            latest_camera = window_camera_events[-1]
            if latest_camera.camera != sample.camera:
                source = (
                    "camera anchor"
                    if retail_frame <= CAMERA_FOLLOW_LAST_RETAIL_FRAME
                    else "camera motion"
                )
                raise SystemExit(
                    f"{source} does not reproduce retail frame {retail_frame}: "
                    f"expected {sample.camera}, found {latest_camera.camera}"
                )
            event_lags = []
            for event in window_camera_events:
                delta = tuple(
                    signed_word(current - anchored)
                    for current, anchored in zip(
                        sample.pose[:3],
                        event.player_pose[:3],
                    )
                )
                if delta == (0, 0, 0):
                    event_lags.append(0)
                elif delta == PLAYER_VELOCITY:
                    event_lags.append(1)
                else:
                    raise SystemExit(
                        f"camera update uses an unsupported player position at "
                        f"retail frame {retail_frame}: {delta}"
                    )
            expected_lags = (1, 0) if camera_updates == 2 else (event_lags[-1],)
            if tuple(event_lags) != expected_lags:
                raise SystemExit(
                    f"camera update ordering diverges at retail frame "
                    f"{retail_frame}: {event_lags}"
                )
            camera_uses_previous_player_position = event_lags[-1] == 1
        elif index > 0 and sample.camera != samples[index - 1].camera:
            raise SystemExit(
                f"camera changed without a completed update at retail frame "
                f"{retail_frame}"
            )

        damage_bank_impulse = 0
        for event in window_controls:
            if (
                event.damage_bank_impulse == PLAYER_HIT_BANK_IMPULSE
                and previous_damage_impulse != PLAYER_HIT_BANK_IMPULSE
            ):
                damage_bank_impulse = PLAYER_HIT_BANK_IMPULSE
            previous_damage_impulse = event.damage_bank_impulse

        result.append(
            Cadence(
                retail_frame=retail_frame,
                control_updates=control_updates,
                movement_updates=movement_updates,
                damage_bank_impulse=damage_bank_impulse,
                camera_updates=camera_updates,
                camera_uses_previous_player_position=(
                    camera_uses_previous_player_position
                ),
            )
        )
        previous_elapsed = sample.elapsed
        previous_pose = sample.pose

    if pending_controls != 0:
        raise SystemExit("first-sortie trace ends with unmatched control motion")
    return result


def compact_source(
    active_trace: Path, dynamics_trace: Path, records: list[Cadence]
) -> str:
    lines = [
        "# Compact Mesen oracle evidence for first-sortie player dynamics.",
        "# Shipping Rust advances typed flat state from the semantic cadence;",
        "# detailed source-machine fields remain confined to the oracle.",
        "# Static routines: player control 06:ECB0..06:EE06,",
        "# player motion 06:EE0A..06:EED4, bank recovery 06:9195,",
        "# camera control 07:84EC, publication 07:8097..07:80B1,",
        "# return orbit 07:9F44..07:A149, motion 07:97FB..07:98CE.",
        f"# camera_follow_last_retail_frame={CAMERA_FOLLOW_LAST_RETAIL_FRAME}",
        f"# camera_return_last_retail_frame={CAMERA_RETURN_LAST_RETAIL_FRAME}",
        f"# Active source SHA-256: {hashlib.sha256(active_trace.read_bytes()).hexdigest()}",
        f"# Dynamics source SHA-256: {hashlib.sha256(dynamics_trace.read_bytes()).hexdigest()}",
    ]
    for record in records:
        lines.append(
            " ".join(
                (
                    f"retail_frame={record.retail_frame}",
                    f"control_updates={record.control_updates}",
                    f"movement_updates={record.movement_updates}",
                    f"damage_bank_impulse={record.damage_bank_impulse}",
                    f"camera_updates={record.camera_updates}",
                    "camera_uses_previous_player_position="
                    f"{int(record.camera_uses_previous_player_position)}",
                )
            )
        )
    return "\n".join(lines) + "\n"


def load_compact(source: str) -> list[Cadence]:
    records = []
    pending_controls = 0
    for line in source.splitlines():
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        record = Cadence(
            retail_frame=int(values["retail_frame"]),
            control_updates=int(values["control_updates"]),
            movement_updates=int(values["movement_updates"]),
            damage_bank_impulse=int(values["damage_bank_impulse"]),
            camera_updates=int(values["camera_updates"]),
            camera_uses_previous_player_position=bool(
                int(values["camera_uses_previous_player_position"])
            ),
        )
        if record.control_updates not in (0, 1, 2):
            raise SystemExit("compact fixture contains an invalid control count")
        if record.movement_updates not in (0, 1, 2):
            raise SystemExit("compact fixture contains an invalid movement count")
        if record.damage_bank_impulse not in (0, PLAYER_HIT_BANK_IMPULSE):
            raise SystemExit("compact fixture contains an invalid damage impulse")
        if record.camera_updates not in (0, 1, 2):
            raise SystemExit("compact fixture contains an invalid camera count")
        if (
            record.camera_uses_previous_player_position
            and record.camera_updates == 0
        ):
            raise SystemExit("compact fixture has a camera lag without an update")
        pending_controls += record.control_updates - record.movement_updates
        if pending_controls not in (0, 1):
            raise SystemExit("compact fixture violates control/movement ordering")
        records.append(record)

    if not records:
        raise SystemExit("compact fixture has no cadence records")
    expected_frames = list(
        range(
            ANCHOR_RETAIL_FRAME,
            records[-1].retail_frame + RETAIL_FRAME_STEP,
            RETAIL_FRAME_STEP,
        )
    )
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("compact fixture has an invalid retail-frame cadence")
    if records[0] != Cadence(ANCHOR_RETAIL_FRAME, 0, 0, 0, 0, False):
        raise SystemExit("compact fixture is missing its inert frame-900 anchor")
    if records[-1].retail_frame != CAMERA_RETURN_LAST_RETAIL_FRAME:
        raise SystemExit("compact fixture does not cover the complete return camera")
    if pending_controls != 0:
        raise SystemExit("compact fixture ends with unmatched control motion")
    return records


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--active-trace", type=Path, default=DEFAULT_ACTIVE_TRACE)
    parser.add_argument("--import-dynamics", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_dynamics is not None:
        records = import_raw(args.active_trace, args.import_dynamics)
        generated = compact_source(args.active_trace, args.import_dynamics, records)
        if args.check:
            if not args.output.exists() or args.output.read_text(encoding="utf-8") != generated:
                raise SystemExit(f"{args.output} is stale")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(generated, encoding="utf-8")
    elif not args.output.exists():
        raise SystemExit(f"{args.output} does not exist")

    records = load_compact(args.output.read_text(encoding="utf-8"))
    hits = [
        record.retail_frame
        for record in records
        if record.damage_bank_impulse != 0
    ]
    print(
        "first-sortie player dynamics verified: "
        f"{len(records)} boundaries through retail frame {records[-1].retail_frame}; "
        f"{sum(record.control_updates for record in records)} control updates; "
        f"{sum(record.movement_updates for record in records)} movement updates; "
        f"{sum(record.camera_updates for record in records)} typed camera updates; "
        f"{sum(record.camera_uses_previous_player_position for record in records)} "
        "previous-player camera anchors; "
        f"natural hits {hits}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
