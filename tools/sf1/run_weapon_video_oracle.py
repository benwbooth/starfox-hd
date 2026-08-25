#!/usr/bin/env python3
"""Certify retail weapon video from repeated uninterrupted Mesen runs.

The oracle follows the source bitmap pipeline and captures the first complete,
repeatable screen three video frames after each completed-transfer marker. Two fresh runs
must produce byte-identical pixels and metadata; gameplay state is certified
independently by the semantic oracle.
"""

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
SCRIPT = ROOT / "tools/sf1/mesen_weapon_states_oracle.lua"
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
DEFAULT_FIRST_GAME_FRAME = 312
DEFAULT_LAST_GAME_FRAME = 337
DEFAULT_REPEATS = 2
DEFAULT_TIMEOUT_SECONDS = 180
SNES_FRAME_WIDTH = 256
SNES_FRAME_HEIGHT = 239
SOURCE_FRAME_TOP = 6
SOURCE_FRAME_HEIGHT = 224
VISIBLE_FRAME_DELAY = 3


def extract_source_frame(screen: Path) -> bytes:
    data = screen.read_bytes()
    header_end = -1
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
    if len(pixels) != SNES_FRAME_WIDTH * SNES_FRAME_HEIGHT * 3:
        raise RuntimeError(f"truncated Mesen screen buffer: {screen}")
    row_bytes = SNES_FRAME_WIDTH * 3
    crop_start = SOURCE_FRAME_TOP * row_bytes
    crop_end = crop_start + SOURCE_FRAME_HEIGHT * row_bytes
    ppm = bytearray(
        f"P6\n{SNES_FRAME_WIDTH} {SOURCE_FRAME_HEIGHT}\n255\n".encode()
    )
    ppm.extend(pixels[crop_start:crop_end])
    return bytes(ppm)


def capture_run(
    first: int,
    last: int,
    repeat: int,
    temporary_root: Path,
    timeout_seconds: int,
) -> Path:
    profile = temporary_root / f"repeat-{repeat}"
    environment = os.environ.copy()
    environment["SF1_WEAPON_CAPTURE_FIRST_GAME_FRAME"] = str(first)
    environment["SF1_WEAPON_CAPTURE_LAST_GAME_FRAME"] = str(last)
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
        timeout=timeout_seconds + 20,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Mesen weapon run {repeat} failed:\n{result.stdout}")
    data = profile / "Mesen2/LuaScriptData/mesen_weapon_states_oracle"
    manifest = data / "sf1_weapon_captures.txt"
    if not manifest.is_file():
        raise RuntimeError(f"Mesen weapon run {repeat} omitted its manifest")
    return data


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
    temporary_root = Path(tempfile.mkdtemp(prefix="sf1-weapon-video-oracle."))
    try:
        runs = [
            capture_run(args.first, args.last, repeat, temporary_root, args.timeout)
            for repeat in range(args.repeats)
        ]
        primary_manifest = runs[0] / "sf1_weapon_captures.txt"
        if any(
            (run / "sf1_weapon_captures.txt").read_bytes()
            != primary_manifest.read_bytes()
            for run in runs[1:]
        ):
            raise RuntimeError("weapon capture manifest differed across fresh runs")

        manifest_entries = {}
        for line in primary_manifest.read_text().splitlines():
            fields = dict(field.split("=", 1) for field in line.split())
            scene_game_frame = int(fields["scene_game_frame"])
            if scene_game_frame in manifest_entries:
                raise RuntimeError(f"duplicate weapon scene {scene_game_frame}")
            manifest_entries[scene_game_frame] = fields

        evidence = []
        for game_frame in range(args.first, args.last + 1):
            screen_name = f"sf1_weapon_scene_{game_frame:03}.ppm"
            screens = [run / screen_name for run in runs]
            if any(not path.is_file() for path in screens):
                raise RuntimeError(f"Mesen omitted game frame {game_frame} evidence")
            frames = [extract_source_frame(path) for path in screens]
            frame_hashes = [hashlib.sha256(frame).hexdigest() for frame in frames]
            if len(set(frame_hashes)) != 1:
                raise RuntimeError(
                    f"game frame {game_frame} framebuffer differed across fresh runs: "
                    + " != ".join(frame_hashes)
                )

            ppm_name = f"sf1_weapon_game_{game_frame:03}.ppm"
            (output / ppm_name).write_bytes(frames[0])
            fields = manifest_entries.get(game_frame)
            if fields is None:
                raise RuntimeError(f"weapon manifest omitted scene {game_frame}")
            retail_video_frame = int(fields["retail_video_frame"])
            transfer_complete_video_frame = int(
                fields["transfer_complete_video_frame"]
            )
            if retail_video_frame - transfer_complete_video_frame != VISIBLE_FRAME_DELAY:
                raise RuntimeError(
                    f"scene {game_frame} violated completed-transfer capture delay"
                )
            observed_game_frame = int(fields["observed_game_frame"])
            metadata_name = f"sf1_weapon_game_{game_frame:03}.txt"
            (output / metadata_name).write_text(
                f"game_frame={game_frame}\n"
                f"retail_observed_game_frame={observed_game_frame}\n"
                f"retail_video_frame={retail_video_frame}\n"
                f"transfer_complete_video_frame={transfer_complete_video_frame}\n"
            )
            evidence.append(
                {
                    "game_frame": game_frame,
                    "retail_observed_game_frame": observed_game_frame,
                    "retail_video_frame": retail_video_frame,
                    "transfer_complete_video_frame": transfer_complete_video_frame,
                    "sha256": frame_hashes[0],
                }
            )

        shutil.copyfile(primary_manifest, output / primary_manifest.name)
        primary_pipeline = runs[0] / "sf1_weapon_pipeline.txt"
        if any(
            (run / "sf1_weapon_pipeline.txt").read_bytes()
            != primary_pipeline.read_bytes()
            for run in runs[1:]
        ):
            raise RuntimeError("weapon bitmap pipeline differed across fresh runs")
        shutil.copyfile(primary_pipeline, output / primary_pipeline.name)
        manifest = {
            "scenario": "sf1_weapon_source_video",
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

    print(f"SF1_WEAPON_VIDEO_ORACLE={output}")
    print(
        f"certified_frames={args.last - args.first + 1} "
        f"fresh_run_repeats={args.repeats} first_divergence=none"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
