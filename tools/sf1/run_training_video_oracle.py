#!/usr/bin/env python3
"""Certify complete retail Training video from two fresh Mesen runs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools/sf2/run_mesen_oracle.py"
SCRIPT = ROOT / "tools/sf1/mesen_training_video_oracle.lua"
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
DEFAULT_FIRST_GAME_FRAME = 1
DEFAULT_LAST_GAME_FRAME = 1758
DEFAULT_REPEATS = 2
DEFAULT_TIMEOUT_SECONDS = 900
SNES_FRAME_WIDTH = 256
SNES_FRAME_HEIGHT = 239
SOURCE_FRAME_TOP = 6
SOURCE_FRAME_HEIGHT = 224
VISIBLE_FRAME_DELAY = 3


def extract_source_frame(screen: Path) -> bytes:
    data = screen.read_bytes()
    search_from = 0
    for _ in range(3):
        header_end = data.find(b"\n", search_from)
        if header_end < 0:
            raise RuntimeError(f"incomplete PPM header: {screen}")
        search_from = header_end + 1
    fields = data[:search_from].split()
    if fields != [b"P6", b"256", b"239", b"255"]:
        raise RuntimeError(f"unexpected Mesen screen dimensions: {screen}")
    pixels = data[search_from:]
    expected = SNES_FRAME_WIDTH * SNES_FRAME_HEIGHT * 3
    if len(pixels) != expected:
        raise RuntimeError(f"truncated Mesen screen buffer: {screen}")
    row_bytes = SNES_FRAME_WIDTH * 3
    crop_start = SOURCE_FRAME_TOP * row_bytes
    crop_end = crop_start + SOURCE_FRAME_HEIGHT * row_bytes
    return (
        f"P6\n{SNES_FRAME_WIDTH} {SOURCE_FRAME_HEIGHT}\n255\n".encode()
        + pixels[crop_start:crop_end]
    )


def capture_run(
    first: int,
    last: int,
    repeat: int,
    temporary_root: Path,
    timeout_seconds: int,
) -> Path:
    profile = temporary_root / f"repeat-{repeat}"
    environment = os.environ.copy()
    environment["SF1_TRAINING_CAPTURE_FIRST_GAME_FRAME"] = str(first)
    environment["SF1_TRAINING_CAPTURE_LAST_GAME_FRAME"] = str(last)
    result = subprocess.run(
        [
            sys.executable,
            str(RUNNER),
            "--quiet",
            "--timeout",
            str(timeout_seconds),
            "--profile",
            str(profile),
            str(SCRIPT),
            str(ROM),
        ],
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout_seconds + 30,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Mesen Training run {repeat} failed:\n{result.stdout}")
    data = profile / "Mesen2/LuaScriptData/mesen_training_video_oracle"
    if not (data / "sf1_training_captures.txt").is_file():
        raise RuntimeError(f"Mesen Training run {repeat} omitted its manifest")
    return data


def parse_capture_manifest(path: Path) -> dict[int, dict[str, int]]:
    entries: dict[int, dict[str, int]] = {}
    for line in path.read_text().splitlines():
        fields = {
            name: int(value)
            for name, value in (field.split("=", 1) for field in line.split())
        }
        scene = fields["scene_game_frame"]
        if scene in entries:
            raise RuntimeError(f"duplicate Training scene {scene}")
        entries[scene] = fields
    return entries


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--first", type=int, default=DEFAULT_FIRST_GAME_FRAME)
    parser.add_argument("--last", type=int, default=DEFAULT_LAST_GAME_FRAME)
    parser.add_argument("--repeats", type=int, choices=(2,), default=DEFAULT_REPEATS)
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    args = parser.parse_args()
    if args.first < 0 or args.first > args.last:
        parser.error("capture range must be ordered and nonnegative")
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    if not ROM.is_file():
        parser.error(f"retail ROM not found: {ROM}")

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    temporary_root = Path(tempfile.mkdtemp(prefix="sf1-training-video-oracle."))
    try:
        runs = [
            capture_run(args.first, args.last, repeat, temporary_root, args.timeout)
            for repeat in range(args.repeats)
        ]
        primary_capture_manifest = runs[0] / "sf1_training_captures.txt"
        if any(
            (run / primary_capture_manifest.name).read_bytes()
            != primary_capture_manifest.read_bytes()
            for run in runs[1:]
        ):
            raise RuntimeError("Training capture manifest differed across fresh runs")
        primary_pipeline = runs[0] / "sf1_training_pipeline.txt"
        if any(
            (run / primary_pipeline.name).read_bytes() != primary_pipeline.read_bytes()
            for run in runs[1:]
        ):
            raise RuntimeError("Training bitmap pipeline differed across fresh runs")

        capture_entries = parse_capture_manifest(primary_capture_manifest)
        evidence = []
        for game_frame in range(args.first, args.last + 1):
            screen_name = f"sf1_training_scene_{game_frame:04}.ppm"
            screens = [run / screen_name for run in runs]
            if any(not path.is_file() for path in screens):
                raise RuntimeError(f"Mesen omitted Training scene {game_frame}")
            source_frames = [extract_source_frame(path) for path in screens]
            hashes = [hashlib.sha256(frame).hexdigest() for frame in source_frames]
            if len(set(hashes)) != 1:
                raise RuntimeError(
                    f"Training scene {game_frame} differed across fresh runs: "
                    + " != ".join(hashes)
                )
            fields = capture_entries.get(game_frame)
            if fields is None:
                raise RuntimeError(f"Training manifest omitted scene {game_frame}")
            retail_video_frame = fields["retail_video_frame"]
            transfer_video_frame = fields["transfer_complete_video_frame"]
            if retail_video_frame - transfer_video_frame != VISIBLE_FRAME_DELAY:
                raise RuntimeError(
                    f"Training scene {game_frame} violated completed-transfer delay"
                )
            (output / f"sf1_training_game_{game_frame:04}.ppm").write_bytes(
                source_frames[0]
            )
            cgram_name = f"sf1_training_cgram_{game_frame:04}.bin"
            cgram_paths = [run / cgram_name for run in runs]
            if any(not path.is_file() for path in cgram_paths):
                raise RuntimeError(f"Mesen omitted Training CGRAM scene {game_frame}")
            cgram_payloads = [path.read_bytes() for path in cgram_paths]
            if any(len(payload) != 512 for payload in cgram_payloads):
                raise RuntimeError(f"Mesen emitted invalid Training CGRAM scene {game_frame}")
            cgram_hashes = [hashlib.sha256(payload).hexdigest() for payload in cgram_payloads]
            if len(set(cgram_hashes)) != 1:
                raise RuntimeError(
                    f"Training CGRAM scene {game_frame} differed across fresh runs: "
                    + " != ".join(cgram_hashes)
                )
            (output / cgram_name).write_bytes(cgram_payloads[0])
            oam_name = f"sf1_training_oam_{game_frame:04}.bin"
            oam_paths = [run / oam_name for run in runs]
            if any(not path.is_file() for path in oam_paths):
                raise RuntimeError(f"Mesen omitted Training OAM scene {game_frame}")
            oam_payloads = [path.read_bytes() for path in oam_paths]
            if any(len(payload) != 544 for payload in oam_payloads):
                raise RuntimeError(f"Mesen emitted invalid Training OAM scene {game_frame}")
            oam_hashes = [hashlib.sha256(payload).hexdigest() for payload in oam_payloads]
            if len(set(oam_hashes)) != 1:
                raise RuntimeError(
                    f"Training OAM scene {game_frame} differed across fresh runs: "
                    + " != ".join(oam_hashes)
                )
            (output / oam_name).write_bytes(oam_payloads[0])
            evidence.append(
                {
                    "game_frame": game_frame,
                    "retail_observed_game_frame": fields["observed_game_frame"],
                    "retail_video_frame": retail_video_frame,
                    "transfer_complete_video_frame": transfer_video_frame,
                    "display_control": fields["display_control"],
                    "display_table_control": fields["display_table_control"],
                    "stage_countdown": fields["stage_countdown"],
                    "background_vertical_scroll": fields[
                        "background_vertical_scroll"
                    ],
                    "first_background_vertical_offset": fields[
                        "first_background_vertical_offset"
                    ],
                    "vertical_offset_enabled": fields["vertical_offset_enabled"],
                    "sha256": hashes[0],
                    "cgram_sha256": cgram_hashes[0],
                    "oam_sha256": oam_hashes[0],
                }
            )

        shutil.copyfile(primary_capture_manifest, output / primary_capture_manifest.name)
        shutil.copyfile(primary_pipeline, output / primary_pipeline.name)
        manifest = {
            "scenario": "sf1_complete_training_source_video",
            "first_game_frame": args.first,
            "last_game_frame": args.last,
            "fresh_run_repeats": args.repeats,
            "capture_method": "completed_bitmap_transfer_plus_three_video_frames",
            "source_frame_top": SOURCE_FRAME_TOP,
            "visible_frame_delay": VISIBLE_FRAME_DELAY,
            "frames": evidence,
        }
        (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    finally:
        shutil.rmtree(temporary_root)

    print(f"SF1_TRAINING_VIDEO_ORACLE={output}")
    print(
        f"certified_frames={args.last - args.first + 1} "
        f"fresh_run_repeats={args.repeats} first_divergence=none"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
