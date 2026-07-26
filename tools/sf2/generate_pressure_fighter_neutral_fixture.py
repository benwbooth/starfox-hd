#!/usr/bin/env python3
"""Import the recurring-attacker neutral player path from the retail oracle.

The compact fixture is verification evidence. Shipping Rust consumes the
static entry poses plus a semantic player/camera update cadence; it never
replays the oracle's live player or camera samples.
"""

from __future__ import annotations

import argparse
import hashlib
from dataclasses import dataclass
from pathlib import Path


DEFAULT_OUTPUT = (
    Path(__file__).with_name("fixtures") / "pressure_fighter_neutral.trace"
)
MISSION_SELECTION = "6"
RETAIL_FRAME_STEP = 4
ENTRY_LAST_RETAIL_FRAME = 312
LIVE_FIRST_RETAIL_FRAME = 316
LIVE_LAST_RETAIL_FRAME = 2_016
LIVE_EVENT_FIRST_RETAIL_FRAME = 314


@dataclass(frozen=True)
class NeutralRecord:
    retail_frame: int
    player_updates: int
    camera_updates: int
    camera: tuple[int, ...]
    player: tuple[int, ...]


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


def import_raw(
    sortie_source: Path,
    dynamics_source: Path,
) -> tuple[list[NeutralRecord], int]:
    samples: dict[int, tuple[tuple[int, ...], tuple[int, ...]]] = {}
    start_elapsed = None
    for line in sortie_source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if (
            values.get("event") != "sortie"
            or values.get("mode") != "1"
            or values.get("selection") != MISSION_SELECTION
        ):
            continue
        elapsed = int(values["elapsed"])
        if start_elapsed is None:
            start_elapsed = elapsed
        retail_frame = elapsed - start_elapsed
        if (
            retail_frame < 0
            or retail_frame > LIVE_LAST_RETAIL_FRAME
            or retail_frame % RETAIL_FRAME_STEP != 0
        ):
            continue
        samples[retail_frame] = (
            parse_tuple(values["camera"], 6, "camera"),
            parse_tuple(values["playerpose"], 7, "player pose"),
        )
    if start_elapsed is None:
        raise SystemExit("sortie trace has no recurring-attacker mission")

    expected_frames = list(
        range(0, LIVE_LAST_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    )
    if sorted(samples) != expected_frames:
        raise SystemExit("neutral sortie samples are not a complete four-frame cadence")

    player_events = []
    camera_events = []
    first_combat_response_retail_frame = None
    for line in dynamics_source.read_text(encoding="utf-8").splitlines():
        values = fields(line)
        if "elapsed" not in values or "stage" not in values:
            continue
        retail_frame = int(values["elapsed"]) - start_elapsed
        if not LIVE_EVENT_FIRST_RETAIL_FRAME <= retail_frame < LIVE_LAST_RETAIL_FRAME:
            continue
        if values["stage"] == "after-motion":
            player_events.append(retail_frame)
            bank_components = parse_tuple(values["bank"], 6, "player bank components")
            if bank_components[-1] != 0 and first_combat_response_retail_frame is None:
                first_combat_response_retail_frame = retail_frame
        elif values["stage"] == "camera_motion_applied":
            camera_events.append(retail_frame)
    if first_combat_response_retail_frame is None:
        raise SystemExit("neutral trace does not expose its first natural combat response")
    neutral_last_retail_frame = (
        (first_combat_response_retail_frame - 1) // RETAIL_FRAME_STEP
    ) * RETAIL_FRAME_STEP

    records = []
    for retail_frame in expected_frames:
        lower_bound = retail_frame - RETAIL_FRAME_STEP
        if retail_frame < LIVE_FIRST_RETAIL_FRAME:
            player_updates = 0
            camera_updates = 0
        else:
            player_updates = sum(
                lower_bound <= event < retail_frame for event in player_events
            )
            camera_updates = sum(
                lower_bound <= event < retail_frame for event in camera_events
            )
        if player_updates not in (0, 1, 2) or camera_updates not in (0, 1, 2):
            raise SystemExit(
                f"source cadence is out of range at retail frame {retail_frame}"
            )
        camera, player = samples[retail_frame]
        records.append(
            NeutralRecord(
                retail_frame=retail_frame,
                player_updates=player_updates,
                camera_updates=camera_updates,
                camera=camera,
                player=player,
            )
        )
    return records, neutral_last_retail_frame


def compact_source(
    sortie_source: Path,
    dynamics_source: Path,
    records: list[NeutralRecord],
    neutral_last_retail_frame: int,
) -> str:
    lines = [
        "# Compact Mesen oracle evidence for recurring-attacker neutral flight.",
        "# Shipping Rust retains the entry poses and semantic update cadence,",
        "# not the live player or camera samples below.",
        "# Static routines: player control 06:ECB0..06:EE06,",
        "# player motion 06:EE0A..06:EED4, camera control 07:84EC.",
        f"# Sortie source SHA-256: {hashlib.sha256(sortie_source.read_bytes()).hexdigest()}",
        f"# Dynamics source SHA-256: {hashlib.sha256(dynamics_source.read_bytes()).hexdigest()}",
        f"# entry_last_retail_frame={ENTRY_LAST_RETAIL_FRAME}",
        f"# live_first_retail_frame={LIVE_FIRST_RETAIL_FRAME}",
        f"# live_last_retail_frame={LIVE_LAST_RETAIL_FRAME}",
        f"# neutral_last_retail_frame={neutral_last_retail_frame}",
    ]
    for record in records:
        lines.append(
            " ".join(
                (
                    f"retail_frame={record.retail_frame}",
                    f"player_updates={record.player_updates}",
                    f"camera_updates={record.camera_updates}",
                    f"camera={','.join(map(str, record.camera))}",
                    f"player={','.join(map(str, record.player))}",
                )
            )
        )
    return "\n".join(lines) + "\n"


def load_compact(source: str) -> tuple[list[NeutralRecord], int]:
    records = []
    neutral_last_retail_frame = None
    for line in source.splitlines():
        if line.startswith("# neutral_last_retail_frame="):
            neutral_last_retail_frame = int(line.split("=", 1)[1])
            continue
        if not line.startswith("retail_frame="):
            continue
        values = fields(line)
        records.append(
            NeutralRecord(
                retail_frame=int(values["retail_frame"]),
                player_updates=int(values["player_updates"]),
                camera_updates=int(values["camera_updates"]),
                camera=parse_tuple(values["camera"], 6, "camera"),
                player=parse_tuple(values["player"], 7, "player pose"),
            )
        )
    expected_frames = list(
        range(0, LIVE_LAST_RETAIL_FRAME + 1, RETAIL_FRAME_STEP)
    )
    if [record.retail_frame for record in records] != expected_frames:
        raise SystemExit("compact neutral fixture has an invalid cadence")
    if any(
        record.player_updates != 0 or record.camera_updates != 0
        for record in records
        if record.retail_frame <= ENTRY_LAST_RETAIL_FRAME
    ):
        raise SystemExit("entry presentation must not contain live update credits")
    if neutral_last_retail_frame is None:
        raise SystemExit("compact neutral fixture is missing its pre-impact boundary")
    return records, neutral_last_retail_frame


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--import-sortie", type=Path)
    parser.add_argument("--import-dynamics", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if (args.import_sortie is None) != (args.import_dynamics is None):
        raise SystemExit("--import-sortie and --import-dynamics must be used together")
    if args.import_sortie is not None:
        records, neutral_last_retail_frame = import_raw(
            args.import_sortie,
            args.import_dynamics,
        )
        generated = compact_source(
            args.import_sortie,
            args.import_dynamics,
            records,
            neutral_last_retail_frame,
        )
        if args.check:
            if not args.output.exists() or args.output.read_text(encoding="utf-8") != generated:
                raise SystemExit(f"{args.output} is stale")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(generated, encoding="utf-8")
    elif not args.output.exists():
        raise SystemExit(f"{args.output} does not exist")

    records, neutral_last_retail_frame = load_compact(
        args.output.read_text(encoding="utf-8")
    )
    print(
        "recurring-attacker neutral fixture verified: "
        f"{len(records)} samples through retail frame {records[-1].retail_frame}; "
        f"pre-impact boundary {neutral_last_retail_frame}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
