#!/usr/bin/env python3
"""Verify SF1 Corneria timing against independent Mesen execution."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools" / "sf2" / "run_mesen_oracle.py"
SCRIPT = Path(__file__).with_name("mesen_corneria_timing_oracle.lua")
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
RETAIL_ROM_SHA256 = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
FIRST_SCENE = 315
LAST_SCENE = 322

EXPECTED = {
    "neutral": {
        "measured_motion": [4, 4, 4, 4, 4, 4, 5, 5],
        "reset_to_sample": [
            1_406_592,
            1_399_378,
            1_345_322,
            1_381_600,
            1_464_464,
            1_438_056,
            1_778_918,
            1_756_302,
        ],
        "reset_to_sample_cpu_cycles": [
            147_233,
            146_411,
            139_866,
            144_101,
            154_248,
            151_129,
            193_043,
            190_373,
        ],
        "safe_wait": [1_218, 3_394, 874, 1_610, 4_538, 882, 386, 2_362],
    },
    "route": {
        "measured_motion": [4, 4, 4, 4, 4, 4, 5, 5],
        "reset_to_sample": [
            1_406_484,
            1_399_322,
            1_345_148,
            1_381_672,
            1_464_658,
            1_437_642,
            1_816_578,
            1_732_436,
        ],
        "reset_to_sample_cpu_cycles": [
            147_224,
            146_395,
            139_847,
            144_125,
            154_259,
            151_084,
            197_605,
            187_466,
        ],
        "safe_wait": [2_562, 3_394, 634, 3_234, 546, 2_098, 674, 4_666],
    },
}


def retail_rom_sha256() -> str:
    digest = hashlib.sha256()
    with ROM.open("rb") as rom:
        for block in iter(lambda: rom.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_rows(path: Path, mode: str) -> list[dict[str, str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        raise RuntimeError(f"Mesen produced an empty timing artifact: {path}")
    header = lines[0].split()
    rows = [
        dict(zip(header, line.split(), strict=True))
        for line in lines[1:]
        if line.startswith(f"{mode} ")
    ]
    expected_count = LAST_SCENE - FIRST_SCENE + 1
    if len(rows) != expected_count:
        raise RuntimeError(
            f"{mode} Mesen capture produced {len(rows)} rows; expected {expected_count}"
        )
    return rows


def verify_rows(mode: str, rows: list[dict[str, str]]) -> None:
    scenes = [int(row["scene_game_frame"]) for row in rows]
    expected_scenes = list(range(FIRST_SCENE, LAST_SCENE + 1))
    if scenes != expected_scenes:
        raise RuntimeError(f"{mode} scene sequence changed: {scenes}")
    for field, expected in EXPECTED[mode].items():
        actual = [int(row[field]) for row in rows]
        if actual != expected:
            raise RuntimeError(
                f"{mode} {field} diverged\nexpected={expected}\nactual={actual}"
            )
    interrupt_counts = [int(row["video_interrupt_count"]) for row in rows]
    measured_motion = [int(row["measured_motion"]) for row in rows]
    if interrupt_counts != measured_motion:
        raise RuntimeError(
            f"{mode} sampled motion no longer copies the retail interrupt count"
        )


def run_mode(mode: str, mesen_bin: Path, timeout: int, profile: Path) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "SF1_MESEN_CORNERIA_INPUT": mode,
            "SF1_MESEN_CORNERIA_FIRST_SCENE": str(FIRST_SCENE),
            "SF1_MESEN_CORNERIA_LAST_SCENE": str(LAST_SCENE),
            "SF1_MESEN_CORNERIA_TIMELINE": "0",
            "SF1_MESEN_CORNERIA_GSU_JOBS": "0",
        }
    )
    subprocess.run(
        [
            sys.executable,
            str(RUNNER),
            "--quiet",
            "--timeout",
            str(timeout),
            "--profile",
            str(profile),
            "--mesen-bin",
            str(mesen_bin),
            str(SCRIPT),
            str(ROM),
        ],
        cwd=ROOT,
        env=environment,
        check=True,
    )
    artifact = (
        profile
        / "Mesen2"
        / "LuaScriptData"
        / SCRIPT.stem
        / f"sf1_corneria_timing_{mode}.txt"
    )
    verify_rows(mode, parse_rows(artifact, mode))


def resolve_mesen(explicit: Path | None) -> Path:
    sys.path.insert(0, str(RUNNER.parent))
    import run_mesen_oracle  # pylint: disable=import-error,import-outside-toplevel

    return run_mesen_oracle.resolve_mesen(explicit)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mesen-bin", type=Path)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    if not ROM.is_file():
        parser.error(f"retail ROM not found: {ROM}")
    actual_sha256 = retail_rom_sha256()
    if actual_sha256 != RETAIL_ROM_SHA256:
        parser.error(
            f"retail ROM SHA-256 changed: expected {RETAIL_ROM_SHA256}, got {actual_sha256}"
        )

    mesen_bin = resolve_mesen(args.mesen_bin)
    with tempfile.TemporaryDirectory(prefix="sf1-corneria-mesen-neutral.") as neutral_dir:
        with tempfile.TemporaryDirectory(prefix="sf1-corneria-mesen-route.") as route_dir:
            profiles = {
                "neutral": Path(neutral_dir),
                "route": Path(route_dir),
            }
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
                futures = {
                    mode: executor.submit(
                        run_mode, mode, mesen_bin, args.timeout, profiles[mode]
                    )
                    for mode in EXPECTED
                }
                for mode, future in futures.items():
                    try:
                        future.result()
                    except Exception as error:
                        raise RuntimeError(f"{mode} Mesen timing verification failed") from error

    print(
        "Mesen Corneria timing verified: "
        f"scenes {FIRST_SCENE}-{LAST_SCENE}, neutral and route"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
