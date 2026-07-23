#!/usr/bin/env python3
"""Generate the accepted Leon duel path with correct projectile ownership."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_pigma_duel import load, rust_source, write_compact


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_TRACE = Path(__file__).with_name("fixtures") / "leon_duel.trace"
DEFAULT_OUTPUT = REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "leon_duel.rs"
HOSTILE_PROJECTILE_SHAPES = frozenset(("E3A8",))
RIVAL_SOURCE_ID = "0576"
RIVAL_SHAPE_TOKEN = "C348"
MISSION_SELECTION = "7"
DUEL_NAME = "Leon"
RAW_START_ELAPSED = 63_320


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

    generated = rust_source(
        DEFAULT_TRACE.name,
        records,
        return_frame,
        map_ready_frame,
        DUEL_NAME,
        Path(__file__).name,
        rival_test_only=True,
        timing_test_only=True,
    )
    if args.check:
        if not DEFAULT_OUTPUT.is_file() or DEFAULT_OUTPUT.read_text(encoding="utf-8") != generated:
            raise SystemExit(f"generated source is out of date: {DEFAULT_OUTPUT}")
        action = "verified"
    else:
        DEFAULT_OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        DEFAULT_OUTPUT.write_text(generated, encoding="utf-8")
        action = "generated"

    hostile_sample_count = sum(len(record.projectiles) for record in records)
    if hostile_sample_count:
        raise SystemExit("accepted fast-kill Leon trace unexpectedly contains hostile projectiles")
    print(
        f"{action} {DEFAULT_OUTPUT}: {len(records)} keyframes, "
        f"retail frames {records[0].retail_frame}..{records[-1].retail_frame}, "
        f"return {return_frame}, map ready {map_ready_frame}, "
        f"hostile projectile samples {hostile_sample_count}"
    )


if __name__ == "__main__":
    main()
