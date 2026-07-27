#!/usr/bin/env python3
"""Generate the accepted Leon pressure path with native combat ownership."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_pigma_duel import Record, load, rust_source, write_compact
from generate_second_sortie_projectiles import format_rust


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "leon_pressure.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "leon_pressure.rs"
HOSTILE_PROJECTILE_SHAPES = frozenset(("E3A8",))
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "C348"
MISSION_SELECTION = "7"
DUEL_NAME = "Leon pressure encounter"
RAW_START_ELAPSED = 73_648
TRANSIENT_OBJECT_LIST_SNAPSHOT_RETAIL_FRAME = 324


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--import-raw",
        type=Path,
        help="rebuild the compact fixture from the accepted raw Mesen trace",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source_trace = args.import_raw or DEFAULT_TRACE
    records, return_frame, map_ready_frame = load(
        (source_trace,),
        HOSTILE_PROJECTILE_SHAPES,
        DUEL_NAME,
        RIVAL_SOURCE_ID,
        RIVAL_SHAPE_TOKEN,
        MISSION_SELECTION,
        RAW_START_ELAPSED if args.import_raw is not None else None,
    )
    if args.import_raw is not None:
        write_compact(
            (source_trace,),
            DEFAULT_TRACE,
            records,
            return_frame,
            map_ready_frame,
            DUEL_NAME,
        )

    transient_index = next(
        index
        for index, record in enumerate(records)
        if record.retail_frame == TRANSIENT_OBJECT_LIST_SNAPSHOT_RETAIL_FRAME
    )
    transient = records[transient_index]
    previous_rival = records[transient_index - 1].rival
    next_rival = records[transient_index + 1].rival
    if transient.rival is not None or previous_rival is None or previous_rival != next_rival:
        raise SystemExit(
            "Leon pressure transient object-list snapshot no longer has its proven shape"
        )
    presentation_records = [
        Record(
            record.retail_frame,
            record.camera,
            record.player,
            record.wingmate,
            (
                previous_rival
                if record.retail_frame == TRANSIENT_OBJECT_LIST_SNAPSHOT_RETAIL_FRAME
                else record.rival
            ),
            (),
        )
        for record in records
    ]
    generated = rust_source(
        DEFAULT_TRACE.name,
        presentation_records,
        return_frame,
        map_ready_frame,
        DUEL_NAME,
        Path(__file__).name,
        rival_test_only=True,
    )
    generated = generated.replace(
        f"//! Source: `{DEFAULT_TRACE.name}`.",
        f"//! Source: `{DEFAULT_TRACE.name}`.\n"
        "//! The transient frame-324 linked-list snapshot is repaired from its\n"
        "//! identical neighboring poses; the operation trace proves the rival remains live.",
    )
    generated = format_rust(generated)
    if args.check:
        if not DEFAULT_OUTPUT.is_file() or DEFAULT_OUTPUT.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {DEFAULT_OUTPUT}")
        action = "verified"
    else:
        DEFAULT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        DEFAULT_OUTPUT.write_text(generated, encoding="utf-8")
        action = "generated"

    print(
        f"{action} {DEFAULT_OUTPUT}: {len(records)} keyframes, "
        f"retail frames {records[0].retail_frame}..{records[-1].retail_frame}, "
        f"return {return_frame}, map ready {map_ready_frame}, "
        "rival poses retained for tests only"
    )


if __name__ == "__main__":
    main()
