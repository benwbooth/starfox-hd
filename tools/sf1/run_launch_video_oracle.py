#!/usr/bin/env python3
"""Compare launch pixels and aperture buffers with two fresh original-game runs.

The original bitmap selects its settled Mesen scanout before native pixels are
compared. Captures remain in disposable profiles for inspection; none are used
as shipping gameplay data or checked-in native-generated expectations.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
ROM_SHA256 = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
SCRIPT = ROOT / "tools/sf1/mesen_launch_display_oracle.lua"
RUNNER = ROOT / "tools/sf2/run_mesen_oracle.py"


def capture_identity(directory: Path) -> dict[str, str]:
    files = [directory / "launch_display.txt"]
    files.extend(sorted(directory.glob("launch_*.ppm")))
    files.extend(sorted(directory.glob("launch_*.vram")))
    files.extend(sorted(directory.glob("launch_edges_*.bin")))
    if len(files) <= 1:
        raise RuntimeError("Mesen omitted its launch captures")
    return {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in files}


def verify_repeated_captures(first: Path, second: Path) -> None:
    a, b = capture_identity(first), capture_identity(second)
    if a != b:
        changed = sorted(name for name in a.keys() | b.keys() if a.get(name) != b.get(name))
        raise RuntimeError(f"fresh Mesen launch runs differ: {', '.join(changed)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=int, default=180, help="seconds per Mesen run")
    parser.add_argument("--debug", action="store_true", help="run native gates in debug mode")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    if not ROM.is_file() or hashlib.sha256(ROM.read_bytes()).hexdigest() != ROM_SHA256:
        parser.error("the original Star Fox USA Rev 2 ROM is required")

    directory = Path(tempfile.mkdtemp(prefix="sf1-launch-video-oracle."))
    print(f"SF1_LAUNCH_EVIDENCE={directory}", flush=True)
    captures = []
    for repeat in range(2):
        profile = directory / f"repeat-{repeat}"
        subprocess.run(
            [sys.executable, str(RUNNER), "--quiet", "--render-every-frame",
             "--timeout", str(args.timeout), "--profile", str(profile), str(SCRIPT), str(ROM)],
            cwd=ROOT, check=True, timeout=args.timeout + 30,
        )
        captures.append(profile / "Mesen2/LuaScriptData/mesen_launch_display_oracle")
    verify_repeated_captures(*captures)
    print("fresh_original_runs=2 capture_bytes_identical=true", flush=True)
    environment = os.environ.copy()
    environment["SF1_LAUNCH_MESEN_DIR"] = str(captures[0])
    command = ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "sf-oracle"]
    if not args.debug:
        command.append("--release")
    command.extend(["--test", "sf1_launch_video", "--test", "sf1_launch_wipe", "--", "--nocapture"])
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    print("launch_video_scenes=16 aperture_records=16 independent_comparison=passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
