#!/usr/bin/env python3
"""Update or verify typed Corneria timing from a full Mesen retail capture."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT / "rust" / "sf-game" / "src" / "gameplay_timing.rs"
FIRST_TIMING_FRAME = 0
LAST_TIMING_FRAME = 982
FIRST_CAPTURED_SCENE = FIRST_TIMING_FRAME + 1
LAST_CAPTURED_SCENE = LAST_TIMING_FRAME + 2
CAPTURED_SCENE_COUNT = LAST_CAPTURED_SCENE - FIRST_CAPTURED_SCENE + 1
ARRAY_NAMES = (
    "CORNERIA_NEUTRAL_MOTION_REFRESHES",
    "CORNERIA_NEUTRAL_PRESENTATION_REFRESHES",
)


def parse_rows(path: Path, mode: str) -> list[dict[str, str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        raise RuntimeError(f"empty Mesen artifact: {path}")
    header = lines[0].split()
    raw_rows = [
        dict(zip(header, line.split(), strict=True))
        for line in lines[1:]
        if line.startswith(f"{mode} ")
    ]
    expected_scenes = list(range(FIRST_CAPTURED_SCENE, LAST_CAPTURED_SCENE + 1))
    captures: list[list[dict[str, str]]] = []
    for start, row in enumerate(raw_rows):
        if int(row["scene_game_frame"]) != FIRST_CAPTURED_SCENE:
            continue
        candidate = raw_rows[start : start + CAPTURED_SCENE_COUNT]
        if [int(item["scene_game_frame"]) for item in candidate] == expected_scenes:
            captures.append(candidate)
    if not captures:
        raise RuntimeError(
            f"Mesen artifact has no contiguous scenes "
            f"{FIRST_CAPTURED_SCENE}-{LAST_CAPTURED_SCENE}"
        )
    canonical = captures[0]
    for duplicate in captures[1:]:
        if duplicate != canonical:
            raise RuntimeError("duplicate complete Mesen captures disagree")
    return canonical


def derive_timing(rows: list[dict[str, str]]) -> tuple[list[int], list[int]]:
    motion = [
        int(rows[frame]["strategies_begin_motion"])
        for frame in range(FIRST_TIMING_FRAME, LAST_TIMING_FRAME + 1)
    ]
    strategy_frames = [int(row["strategies_begin_video_frame"]) for row in rows]
    presentation = [
        strategy_frames[frame + 1] - strategy_frames[frame]
        for frame in range(FIRST_TIMING_FRAME, LAST_TIMING_FRAME + 1)
    ]
    for name, values in zip(ARRAY_NAMES, (motion, presentation), strict=True):
        if len(values) != LAST_TIMING_FRAME + 1:
            raise RuntimeError(f"{name} has the wrong length: {len(values)}")
        if any(value <= 0 or value > 255 for value in values):
            raise RuntimeError(f"{name} contains an invalid refresh count")
    return motion, presentation


def parse_array(source: str, name: str) -> list[int]:
    match = re.search(rf"const {name}:.*?= \[(.*?)\];", source, re.DOTALL)
    if match is None:
        raise RuntimeError(f"missing Rust timing array: {name}")
    return [int(value) for value in re.findall(r"\b\d+\b", match.group(1))]


def format_array(values: list[int]) -> str:
    width = 24
    return "\n" + "\n".join(
        "    " + ", ".join(str(value) for value in values[start : start + width]) + ","
        for start in range(0, len(values), width)
    ) + "\n"


def replace_array(source: str, name: str, values: list[int]) -> str:
    pattern = re.compile(rf"(const {name}:.*?= \[)(.*?)(\];)", re.DOTALL)
    updated, count = pattern.subn(
        lambda match: match.group(1) + format_array(values) + match.group(3),
        source,
        count=1,
    )
    if count != 1:
        raise RuntimeError(f"could not replace Rust timing array: {name}")
    return updated


def mismatch_summary(actual: list[int], expected: list[int]) -> tuple[int, str]:
    mismatches = [
        (frame, current, retail)
        for frame, (current, retail) in enumerate(zip(actual, expected, strict=True))
        if current != retail
    ]
    sample = ", ".join(
        f"{frame}:{current}->{retail}" for frame, current, retail in mismatches[:8]
    )
    return len(mismatches), sample


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--mode", choices=("neutral",), default="neutral")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--write", action="store_true")
    args = parser.parse_args()

    rows = parse_rows(args.artifact, args.mode)
    expected_arrays = derive_timing(rows)
    source = args.source.read_text(encoding="utf-8")
    actual_arrays = [parse_array(source, name) for name in ARRAY_NAMES]
    summaries = [
        mismatch_summary(actual, expected)
        for actual, expected in zip(actual_arrays, expected_arrays, strict=True)
    ]
    if args.check:
        failures = [
            f"{name}: {count} mismatches ({sample})"
            for name, (count, sample) in zip(ARRAY_NAMES, summaries, strict=True)
            if count
        ]
        if failures:
            raise RuntimeError("Mesen timing differs from Rust\n" + "\n".join(failures))
        print("Mesen Corneria timing matches the typed Rust arrays: frames 0-982")
        return 0

    updated = source
    for name, values in zip(ARRAY_NAMES, expected_arrays, strict=True):
        updated = replace_array(updated, name, values)
    args.source.write_text(updated, encoding="utf-8")
    print(
        "Updated typed Corneria timing from Mesen: "
        + ", ".join(
            f"{name}={count} changes"
            for name, (count, _) in zip(ARRAY_NAMES, summaries, strict=True)
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
