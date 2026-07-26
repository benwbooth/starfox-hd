#!/usr/bin/env python3
"""Generate the typed post-opening first-sortie fighter schedule.

The normal path consumes a compact oracle fixture and emits only semantic Rust
operations. The optional import path accepts the mechanically recovered probe
table, verifies its random-state boundaries against the raw Mesen pose trace,
and reduces both sources to the checked-in fixture.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = (
    Path(__file__).with_name("fixtures") / "first_sortie_fighter_mission.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "fighter_mission_continuation.rs"
)
START_RETAIL_FRAME = 2_452
LAST_VISIBLE_RETAIL_FRAME = 3_652
END_RETAIL_FRAME = 3_656
LOWER_DEPARTURE_RETAIL_FRAME = 2_452
UPPER_DEPARTURE_RETAIL_FRAME = 3_656
RETAIL_FRAME_STEP = 4
ORACLE_ANCHOR_RETAIL_FRAME = 900
ORACLE_ANCHOR_ELAPSED = 7_312
ORACLE_POST_SAMPLE_OFFSET = 1

DISPATCH_NAMES = {
    "Wait",
    "AdvanceCurrentMovement",
    "TurnOnly",
    "MovementOnly",
    "MovementContinuation",
    "MovementAndRoll",
    "SteeringOnly",
    "PitchContinuation",
    "PrepareWave",
    "SplitWave",
    "QuarterWave",
    "ThreeQuarterWave",
    "ApplyWave",
    "AltitudeCenteringOnly",
    "CompleteAfterEarlyAltitude",
    "AltitudeAndTurnOnly",
    "MovementContinuationAfterEarlyAltitude",
    "PrepareMovement",
    "FinishPreparedAndBeginMovement",
    "SteeringAfterEarlyAltitude",
    "Complete",
    "Depart",
}


@dataclass(frozen=True)
class ScheduleRow:
    retail_frame: int
    dispatches: tuple[str, ...]
    ambient_before: int
    ambient_between_first: int
    ambient_between_second: int
    ambient_between_fighters: int
    ambient_after: int
    random_state: tuple[int, int, int, int]


def rust_int(value: int) -> str:
    return f"{value:_}"


def parse_bytes(encoded: str, context: str) -> tuple[int, int, int, int]:
    values = tuple(map(int, encoded.split(",")))
    if len(values) != 4 or any(not 0 <= value <= 255 for value in values):
        raise SystemExit(f"{context}: expected four decimal bytes, got {encoded!r}")
    return values


def validate_rows(rows: list[ScheduleRow], *, recovered: bool = False) -> None:
    end_retail_frame = (
        LAST_VISIBLE_RETAIL_FRAME if recovered else END_RETAIL_FRAME
    )
    expected_frames = list(
        range(
            START_RETAIL_FRAME,
            end_retail_frame + RETAIL_FRAME_STEP,
            RETAIL_FRAME_STEP,
        )
    )
    frames = [row.retail_frame for row in rows]
    if frames != expected_frames:
        raise SystemExit(
            "fighter schedule frames are not the complete contiguous "
            f"{START_RETAIL_FRAME}..={end_retail_frame} sequence"
        )
    expected_row_count = (
        (end_retail_frame - START_RETAIL_FRAME) // RETAIL_FRAME_STEP + 1
    )
    if len(rows) != expected_row_count:
        raise SystemExit(
            f"fighter schedule has {len(rows)} rows, expected {expected_row_count}"
        )
    if recovered and any("Depart" in row.dispatches for row in rows):
        raise SystemExit("the recovered movement table must not contain lifecycle rows")
    if not recovered and rows[-1].dispatches != ("Depart",):
        raise SystemExit("the fighter schedule must end with one semantic Depart row")
    for row in rows:
        if not 1 <= len(row.dispatches) <= 3:
            raise SystemExit(
                f"frame {row.retail_frame}: expected one to three semantic dispatches"
            )
        unknown = set(row.dispatches) - DISPATCH_NAMES
        if unknown:
            raise SystemExit(
                f"frame {row.retail_frame}: unknown dispatches {sorted(unknown)}"
            )
        cadence = (
            row.ambient_before,
            row.ambient_between_first,
            row.ambient_between_second,
            row.ambient_between_fighters,
            row.ambient_after,
        )
        if any(not 0 <= draws <= 255 for draws in cadence):
            raise SystemExit(
                f"frame {row.retail_frame}: random cadence is outside byte range"
            )
        if row.ambient_between_first or row.ambient_between_second:
            raise SystemExit(
                f"frame {row.retail_frame}: inter-slice random draws require "
                "an explicit shipping cadence model"
            )


def read_recovered(path: Path) -> list[ScheduleRow]:
    with path.open(encoding="utf-8", newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        rows = [
            ScheduleRow(
                retail_frame=int(raw["retail_frame"]),
                dispatches=tuple(raw["dispatches"].split(",")),
                ambient_before=int(raw["ambient_before"]),
                ambient_between_first=int(raw["ambient_between_first"]),
                ambient_between_second=int(raw["ambient_between_second"]),
                ambient_between_fighters=int(raw["ambient_between_fighters"]),
                ambient_after=int(raw["ambient_after"]),
                random_state=parse_bytes(
                    raw["random_state"],
                    f"frame {raw['retail_frame']} recovered random state",
                ),
            )
            for raw in reader
        ]
    validate_rows(rows, recovered=True)
    return rows


def raw_random_states(path: Path) -> dict[int, tuple[int, int, int, int]]:
    states = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("elapsed=") or " rng=" not in line:
            continue
        elapsed = int(line.split(" ", 1)[0].split("=", 1)[1])
        encoded = line.rsplit(" rng=", 1)[1].split()[0]
        if len(encoded) != 8:
            continue
        states[elapsed] = tuple(
            int(encoded[index : index + 2], 16) for index in range(0, 8, 2)
        )
    return states


def next_random_state(
    state: tuple[int, int, int, int],
) -> tuple[int, int, int, int]:
    values = list(state)
    original_first = values[0]
    no_borrow = False

    def subtract(left: int, right: int) -> tuple[int, bool]:
        borrow = int(not no_borrow)
        subtrahend = right + borrow
        return (left - subtrahend) % 256, left >= subtrahend

    value, no_borrow = subtract(original_first, values[1])
    values[1] = value
    value, no_borrow = subtract(value, values[2])
    values[2] = value
    value, no_borrow = subtract(value, values[3])
    values[3] = value
    value, _ = subtract(value, original_first)
    values[0] = value
    return tuple(values)


def draws_between(
    start: tuple[int, int, int, int],
    target: tuple[int, int, int, int],
) -> int:
    state = start
    for draws in range(256):
        if state == target:
            return draws
        state = next_random_state(state)
    raise SystemExit(
        f"departure random state {target} is not reachable within one byte of draws"
    )


def import_recovered(recovered: Path, raw_trace: Path, fixture: Path) -> None:
    rows = read_recovered(recovered)
    states = raw_random_states(raw_trace)
    for row in rows:
        elapsed = (
            ORACLE_ANCHOR_ELAPSED
            + row.retail_frame
            - ORACLE_ANCHOR_RETAIL_FRAME
            + ORACLE_POST_SAMPLE_OFFSET
        )
        expected = states.get(elapsed)
        if expected != row.random_state:
            raise SystemExit(
                f"frame {row.retail_frame}: recovered state {row.random_state} "
                f"does not match raw elapsed {elapsed}: {expected}"
            )

    departure_elapsed = (
        ORACLE_ANCHOR_ELAPSED
        + UPPER_DEPARTURE_RETAIL_FRAME
        - ORACLE_ANCHOR_RETAIL_FRAME
        + ORACLE_POST_SAMPLE_OFFSET
    )
    departure_state = states.get(departure_elapsed)
    if departure_state is None:
        raise SystemExit(
            f"raw trace has no departure state at elapsed {departure_elapsed}"
        )
    departure_draws = draws_between(rows[-1].random_state, departure_state)
    rows.append(
        ScheduleRow(
            retail_frame=UPPER_DEPARTURE_RETAIL_FRAME,
            dispatches=("Depart",),
            ambient_before=0,
            ambient_between_first=0,
            ambient_between_second=0,
            ambient_between_fighters=0,
            ambient_after=departure_draws,
            random_state=departure_state,
        )
    )
    validate_rows(rows)

    digest = hashlib.sha256(raw_trace.read_bytes()).hexdigest()
    lines = [
        "# Compact oracle evidence for the post-opening first-sortie fighter task.",
        f"# Raw source SHA-256: {digest}",
        "# Each row was recovered by exact typed-pose and post-tick random-state replay.",
        "# cadence=before,between-upper-slice-1,between-upper-slice-2,between-fighters,after",
    ]
    for row in rows:
        lines.append(
            f"frame={row.retail_frame} "
            f"actions={','.join(row.dispatches)} "
            "cadence="
            f"{row.ambient_before},{row.ambient_between_first},"
            f"{row.ambient_between_second},{row.ambient_between_fighters},"
            f"{row.ambient_after} "
            f"random={','.join(map(str, row.random_state))}"
        )
    fixture.parent.mkdir(parents=True, exist_ok=True)
    fixture.write_text("\n".join(lines) + "\n", encoding="utf-8")


def fields(line: str) -> dict[str, str]:
    return dict(part.split("=", 1) for part in line.split() if "=" in part)


def read_fixture(path: Path) -> list[ScheduleRow]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("frame="):
            continue
        parsed = fields(line)
        cadence = tuple(map(int, parsed["cadence"].split(",")))
        if len(cadence) != 5:
            raise SystemExit(
                f"frame {parsed['frame']}: cadence must contain five draw counts"
            )
        rows.append(
            ScheduleRow(
                retail_frame=int(parsed["frame"]),
                dispatches=tuple(parsed["actions"].split(",")),
                ambient_before=cadence[0],
                ambient_between_first=cadence[1],
                ambient_between_second=cadence[2],
                ambient_between_fighters=cadence[3],
                ambient_after=cadence[4],
                random_state=parse_bytes(
                    parsed["random"],
                    f"frame {parsed['frame']} fixture random state",
                ),
            )
        )
    validate_rows(rows)
    return rows


def dispatch_expression(row: ScheduleRow) -> list[str]:
    first = "Wait" if row.dispatches == ("Depart",) else row.dispatches[0]
    lines = [
        "    FighterLogicDispatchPair::new(",
        f"        FighterLogicDispatch::{first},",
        "        FighterLogicDispatch::Wait,",
        "    )",
    ]
    if len(row.dispatches) == 2:
        lines[-1] += ".with_upper_next_slice("
        lines.append(f"        FighterLogicDispatch::{row.dispatches[1]},")
        lines.append("    ),")
    elif len(row.dispatches) == 3:
        lines[-1] += ".with_upper_next_slices("
        lines.append(f"        FighterLogicDispatch::{row.dispatches[1]},")
        lines.append(f"        FighterLogicDispatch::{row.dispatches[2]},")
        lines.append("    ),")
    else:
        lines[-1] += ","
    return lines


def rust_source(rows: list[ScheduleRow]) -> str:
    lines = [
        "//! Generated semantic continuation of the retail first-sortie fighter task.",
        "//! Source addresses and machine state remain in oracle tooling.",
        "",
        "use super::{FighterLogicDispatch, FighterLogicDispatchPair, FighterRandomCadence};",
        "",
        f"pub(super) const START_RETAIL_FRAME: u16 = {rust_int(START_RETAIL_FRAME)};",
        "#[cfg(test)]",
        "pub(super) const LAST_VISIBLE_RETAIL_FRAME: u16 = "
        f"{rust_int(LAST_VISIBLE_RETAIL_FRAME)};",
        f"pub(super) const END_RETAIL_FRAME: u16 = {rust_int(END_RETAIL_FRAME)};",
        "pub(super) const LOWER_DEPARTURE_RETAIL_FRAME: u16 = "
        f"{rust_int(LOWER_DEPARTURE_RETAIL_FRAME)};",
        "pub(super) const UPPER_DEPARTURE_RETAIL_FRAME: u16 = "
        f"{rust_int(UPPER_DEPARTURE_RETAIL_FRAME)};",
        "",
        "pub(super) const DISPATCH: [FighterLogicDispatchPair; "
        f"{len(rows)}] = [",
    ]
    for row in rows:
        lines.extend(dispatch_expression(row))
    lines.extend(
        [
            "];",
            "",
            "pub(super) const RANDOM_CADENCE: [FighterRandomCadence; "
            f"{len(rows)}] = [",
        ]
    )
    for row in rows:
        lines.append(
            "    FighterRandomCadence::around_fighter_checks("
            f"{row.ambient_before}, {row.ambient_between_fighters}, "
            f"{row.ambient_after}),"
        )
    lines.extend(
        [
            "];",
            "",
            "#[cfg(test)]",
            "pub(super) const EXPECTED_RANDOM_STATES: [[u8; 4]; "
            f"{len(rows)}] = [",
        ]
    )
    for row in rows:
        lines.append(
            "    ["
            + ", ".join(map(str, row.random_state))
            + "],"
        )
    lines.extend(["];", ""])
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--import-recovered",
        type=Path,
        help="reduce a mechanically recovered schedule table first",
    )
    parser.add_argument(
        "--raw-trace",
        type=Path,
        help="raw Mesen pose trace used to verify imported random boundaries",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_recovered is not None:
        if args.raw_trace is None:
            raise SystemExit("--import-recovered requires --raw-trace")
        import_recovered(args.import_recovered, args.raw_trace, args.fixture)

    rows = read_fixture(args.fixture)
    generated = rust_source(rows)
    if args.check:
        current = args.output.read_text(encoding="utf-8") if args.output.exists() else ""
        if current != generated:
            raise SystemExit(
                f"generated fighter mission continuation is stale: {args.output}"
            )
        print(
            f"fighter mission continuation verified: {len(rows)} exact oracle boundaries"
        )
        return

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(generated, encoding="utf-8")
    print(
        f"fighter mission continuation: {len(rows)} exact oracle boundaries "
        f"-> {args.output}"
    )


if __name__ == "__main__":
    main()
