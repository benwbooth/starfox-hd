#!/usr/bin/env python3
"""Run a Star Fox 2 Mesen Lua oracle in a disposable, headless profile.

Mesen checks for ``settings.json`` before it checks ``--testRunner``.  A truly
empty XDG profile therefore opens the first-run GUI and appears to hang.  This
runner installs the minimal valid settings document with an SNES controller
assigned to port 1, enables only the per-script data directory, and leaves the
profile in ``/tmp`` so binary oracle artifacts can be inspected after the
emulator exits.  The explicit controller assignment is required: Mesen accepts
Lua ``setInput`` calls for an empty port without error, but returns an empty
input table and injects no controller bits.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ROM = ROOT / "Star Fox 2 (USA, Europe).sfc"
SETTINGS_TEMPLATE = Path(__file__).with_name("mesen-settings.json")


def resolve_mesen(explicit: Path | None) -> Path:
    candidates: list[Path] = []
    if explicit is not None:
        candidates.append(explicit)
    if value := os.environ.get("MESEN_BIN"):
        candidates.append(Path(value))
    for name in ("Mesen", "mesen"):
        if value := shutil.which(name):
            candidates.append(Path(value))

    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate.resolve()

    if shutil.which("nix") is None:
        raise RuntimeError("Mesen was not found and nix is unavailable")
    result = subprocess.run(
        [
            "nix",
            "build",
            "--no-link",
            "--print-out-paths",
            "nixpkgs#mesen",
        ],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    for store_path in reversed(result.stdout.splitlines()):
        for name in ("Mesen", "mesen"):
            candidate = Path(store_path) / "bin" / name
            if candidate.is_file() and os.access(candidate, os.X_OK):
                return candidate
    raise RuntimeError("nixpkgs#mesen built, but its executable was not found")


def prepare_profile(requested: Path | None) -> Path:
    profile = requested or Path(tempfile.mkdtemp(prefix="starfox-mesen-profile."))
    home = profile / "Mesen2"
    home.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(SETTINGS_TEMPLATE, home / "settings.json")
    return profile.resolve()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("script", type=Path, help="Mesen Lua oracle script")
    parser.add_argument("rom", nargs="?", type=Path, default=DEFAULT_ROM)
    parser.add_argument("--mesen-bin", type=Path)
    parser.add_argument("--profile", type=Path, help="use this empty profile directory")
    parser.add_argument("--timeout", type=int, default=30, help="Mesen timeout in seconds")
    parser.add_argument(
        "--expect-exit",
        type=lambda value: int(value, 0),
        default=0,
        help="required emulator exit code (accepts decimal or 0x-prefixed values)",
    )
    args = parser.parse_args()

    script = args.script.resolve()
    rom = args.rom.resolve()
    if not script.is_file():
        parser.error(f"Lua script not found: {script}")
    if not rom.is_file():
        parser.error(f"ROM not found: {rom}")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    try:
        mesen = resolve_mesen(args.mesen_bin)
        profile = prepare_profile(args.profile)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"oracle setup failed: {error}", file=sys.stderr)
        return 2

    print(f"MESEN_PROFILE={profile}", flush=True)
    print(
        f"MESEN_SCRIPT_DATA={profile / 'Mesen2' / 'LuaScriptData' / script.stem}",
        flush=True,
    )
    command = [
        str(mesen),
        "--testRunner",
        "--enableStdout",
        "--doNotSaveSettings",
        "--debug.scriptWindow.allowIoOsAccess=true",
        f"--timeout={args.timeout}",
        str(script),
        str(rom),
    ]
    environment = os.environ.copy()
    environment["XDG_CONFIG_HOME"] = str(profile)
    try:
        result = subprocess.run(
            command,
            env=environment,
            timeout=args.timeout + 10,
            check=False,
        )
    except subprocess.TimeoutExpired:
        print("oracle exceeded the Mesen timeout plus shutdown grace", file=sys.stderr)
        return 124

    if result.returncode != args.expect_exit:
        print(
            f"oracle exit mismatch: expected {args.expect_exit}, got {result.returncode}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
