#!/usr/bin/env python3
"""Compare the source-bound laser replay with two fresh original-game captures.

The actual original controller latch is checked at every strategy visit. Full
original bitmap identity and two settled original scanouts select each scene;
native pixels never select the reference and no fixed scanout delay is used.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from run_launch_video_oracle import ROM, ROM_SHA256, ROOT, RUNNER, verify_repeated_captures


SCRIPT = ROOT / "tools/sf1/mesen_weapon_scanout_oracle.lua"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=int, default=180, help="seconds per Mesen run")
    parser.add_argument("--debug", action="store_true", help="run the paired replay in debug mode")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    if not ROM.is_file() or hashlib.sha256(ROM.read_bytes()).hexdigest() != ROM_SHA256:
        parser.error("the original Star Fox USA Rev 2 ROM is required")

    directory = Path(tempfile.mkdtemp(prefix="sf1-weapon-scanout-oracle."))
    print(f"SF1_WEAPON_EVIDENCE={directory}", flush=True)
    captures = []
    for repeat in range(2):
        profile = directory / f"repeat-{repeat}"
        subprocess.run(
            [sys.executable, str(RUNNER), "--quiet", "--render-every-frame",
             "--timeout", str(args.timeout), "--profile", str(profile), str(SCRIPT), str(ROM)],
            cwd=ROOT, check=True, timeout=args.timeout + 30,
        )
        captures.append(profile / "Mesen2/LuaScriptData/mesen_weapon_scanout_oracle")
    verify_repeated_captures(*captures, prefix="weapon")
    print("fresh_original_runs=2 capture_bytes_identical=true", flush=True)
    environment = os.environ.copy()
    environment["SF1_WEAPON_MESEN_DIR"] = str(captures[0])
    command = ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "-p", "sf-oracle"]
    if not args.debug:
        command.append("--release")
    command.extend(["--example", "sf1_weapon_trace"])
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    print("weapon_video_scenes=26 independent_comparison=passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
