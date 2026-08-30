#!/usr/bin/env python3
"""Compare typed native Corneria checkpoints with independent Mesen state."""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools" / "sf2" / "run_mesen_oracle.py"
SCRIPT = Path(__file__).with_name("mesen_corneria_timing_oracle.lua")
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
RETAIL_ROM_SHA256 = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
CHECKPOINT_SCENES = (1, 187, 307, 607, 807, 907, *range(940, 984))
FIRST_SCENE = CHECKPOINT_SCENES[0]
LAST_SCENE = CHECKPOINT_SCENES[-1]
BACKGROUND_FIRST_SOURCE_OFFSET = 3
BACKGROUND_RECORD_BYTES = 6
MESEN_ONLY_OBJECT_FIELDS = {"shape_source", "pointer"}
NATIVE_ONLY_OBJECT_FIELDS = {"shape"}


def parse_fields(line: str) -> dict[str, str]:
    return dict(token.split("=", 1) for token in line.split() if "=" in token)


def parse_semantic(text: str) -> tuple[dict[int, dict[str, str]], dict[tuple[int, int], dict[str, str]]]:
    scenes: dict[int, dict[str, str]] = {}
    objects: dict[tuple[int, int], dict[str, str]] = {}
    for line in text.splitlines():
        if line.startswith("kind=semantic "):
            fields = parse_fields(line)
            scene = int(fields.pop("scene"))
            fields.pop("kind")
            if scene in scenes:
                raise RuntimeError(f"duplicate semantic scene {scene}")
            scenes[scene] = fields
        elif line.startswith("kind=semantic_object "):
            fields = parse_fields(line)
            scene = int(fields.pop("scene"))
            slot = int(fields.pop("slot"))
            fields.pop("kind")
            key = (scene, slot)
            if key in objects:
                raise RuntimeError(f"duplicate semantic object scene={scene} slot={slot}")
            objects[key] = fields
    return scenes, objects


def normalize_mesen(
    scenes: dict[int, dict[str, str]],
    objects: dict[tuple[int, int], dict[str, str]],
) -> None:
    for scene, fields in scenes.items():
        source = int(fields.pop("background_source"))
        relative = source - BACKGROUND_FIRST_SOURCE_OFFSET
        if relative < 0 or relative % BACKGROUND_RECORD_BYTES != 0:
            raise RuntimeError(
                f"scene {scene} has invalid retail background source offset {source}"
            )
        fields["background"] = str(relative // BACKGROUND_RECORD_BYTES)
    for fields in objects.values():
        for name in MESEN_ONLY_OBJECT_FIELDS:
            fields.pop(name)


def normalize_native(objects: dict[tuple[int, int], dict[str, str]]) -> None:
    for fields in objects.values():
        for name in NATIVE_ONLY_OBJECT_FIELDS:
            fields.pop(name)


def compare_records(
    label: str,
    reference: dict[object, dict[str, str]],
    candidate: dict[object, dict[str, str]],
) -> list[str]:
    failures: list[str] = []
    if reference.keys() != candidate.keys():
        missing = sorted(reference.keys() - candidate.keys())
        extra = sorted(candidate.keys() - reference.keys())
        failures.append(f"{label} keys differ: missing={missing[:8]} extra={extra[:8]}")
    for key in sorted(reference.keys() & candidate.keys()):
        expected = reference[key]
        actual = candidate[key]
        if expected.keys() != actual.keys():
            missing = sorted(expected.keys() - actual.keys())
            extra = sorted(actual.keys() - expected.keys())
            failures.append(
                f"{label} {key} fields differ: missing={missing} extra={extra}"
            )
            continue
        for field in expected:
            if expected[field] != actual[field]:
                failures.append(
                    f"{label} {key} {field}: Mesen={expected[field]} "
                    f"native={actual[field]}"
                )
    return failures


def compare(mesen_text: str, native_text: str) -> None:
    mesen_scenes, mesen_objects = parse_semantic(mesen_text)
    native_scenes, native_objects = parse_semantic(native_text)
    selected = set(CHECKPOINT_SCENES)
    mesen_scenes = {key: value for key, value in mesen_scenes.items() if key in selected}
    mesen_objects = {
        key: value for key, value in mesen_objects.items() if key[0] in selected
    }
    native_scenes = {key: value for key, value in native_scenes.items() if key in selected}
    native_objects = {
        key: value for key, value in native_objects.items() if key[0] in selected
    }
    normalize_mesen(mesen_scenes, mesen_objects)
    normalize_native(native_objects)
    failures = compare_records("scene", mesen_scenes, native_scenes)
    failures.extend(compare_records("object", mesen_objects, native_objects))
    if failures:
        sample = "\n".join(failures[:40])
        raise RuntimeError(
            f"Mesen/native semantic mismatch ({len(failures)} fields)\n{sample}"
        )


def verify_rom() -> None:
    if not ROM.is_file():
        raise RuntimeError(f"retail ROM not found: {ROM}")
    digest = hashlib.sha256(ROM.read_bytes()).hexdigest()
    if digest != RETAIL_ROM_SHA256:
        raise RuntimeError(f"retail ROM SHA-256 changed: {digest}")


def run_mesen(mesen_bin: Path, timeout: int, profile: Path) -> Path:
    env = os.environ.copy()
    env.update(
        {
            "SF1_MESEN_CORNERIA_INPUT": "neutral",
            "SF1_MESEN_CORNERIA_FIRST_SCENE": str(FIRST_SCENE),
            "SF1_MESEN_CORNERIA_LAST_SCENE": str(LAST_SCENE),
            "SF1_MESEN_CORNERIA_TIMEOUT_VIDEO_FRAMES": "30000",
            "SF1_MESEN_CORNERIA_CHECKPOINT_INTERVAL": "25",
            "SF1_MESEN_CORNERIA_TIMELINE": "0",
            "SF1_MESEN_CORNERIA_GSU_JOBS": "0",
            "SF1_MESEN_CORNERIA_SEMANTIC": "1",
        }
    )
    subprocess.run(
        [
            "python3",
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
        env=env,
        check=True,
    )
    return (
        profile
        / "Mesen2"
        / "LuaScriptData"
        / SCRIPT.stem
        / "sf1_corneria_timing_neutral.txt"
    )


def run_native() -> str:
    result = subprocess.run(
        [
            "nix",
            "develop",
            "--command",
            "cargo",
            "run",
            "--manifest-path",
            "rust/Cargo.toml",
            "-q",
            "-p",
            "sf-oracle",
            "--example",
            "sf1_native_semantic_probe",
        ],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return result.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mesen-bin", type=Path)
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--native-output", type=Path)
    args = parser.parse_args()
    if (args.artifact is None) != (args.native_output is None):
        parser.error("--artifact and --native-output must be supplied together")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    if args.artifact is not None:
        compare(
            args.artifact.read_text(encoding="utf-8"),
            args.native_output.read_text(encoding="utf-8"),
        )
    else:
        if args.mesen_bin is None:
            parser.error("--mesen-bin is required unless artifacts are supplied")
        verify_rom()
        with tempfile.TemporaryDirectory(prefix="sf1-corneria-semantic.") as temp:
            artifact = run_mesen(args.mesen_bin, args.timeout, Path(temp))
            compare(artifact.read_text(encoding="utf-8"), run_native())
    print(
        "Mesen/native Corneria semantic checkpoints verified: "
        + ", ".join(map(str, CHECKPOINT_SCENES))
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
