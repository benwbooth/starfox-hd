#!/usr/bin/env python3
"""Compare typed native Corneria checkpoints with independent Mesen state."""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
from pathlib import Path

if __package__:
    from .semantic_evidence import validate_object_inventory
else:
    from semantic_evidence import validate_object_inventory

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools" / "sf2" / "run_mesen_oracle.py"
SCRIPT = Path(__file__).with_name("mesen_corneria_timing_oracle.lua")
ROM = ROOT / "Star Fox (USA) (Rev 2).sfc"
RETAIL_ROM_SHA256 = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
NATIVE_PATH_CATALOG = ROOT / "data" / "path_catalog.bin"
RETAIL_PATH_CATALOG_ROM_OFFSET = 0x22413
PATH_CURSOR_ROLE = "path_cursor"
PATH_CURSOR_WINDOW_RADIUS = 32
PATH_CURSOR_MAX_RELOCATION = 64
PATH_CURSOR_MINIMUM_SCORE_MARGIN = 8
CHECKPOINT_SCENES = tuple(range(1, 984))
FIRST_SCENE = CHECKPOINT_SCENES[0]
LAST_SCENE = CHECKPOINT_SCENES[-1]
BACKGROUND_FIRST_SOURCE_OFFSET = 3
BACKGROUND_RECORD_BYTES = 6
MESEN_ONLY_OBJECT_FIELDS = {"shape_source", "pointer"}
NATIVE_ONLY_OBJECT_FIELDS = {"shape"}


def path_catalogs() -> tuple[bytes, bytes]:
    verify_rom()
    native = NATIVE_PATH_CATALOG.read_bytes()
    retail_rom = ROM.read_bytes()
    retail = retail_rom[
        RETAIL_PATH_CATALOG_ROM_OFFSET : RETAIL_PATH_CATALOG_ROM_OFFSET + len(native)
    ]
    if len(retail) != len(native):
        raise RuntimeError("retail path catalog is truncated")
    return retail, native


def relocate_path_cursor(source_cursor: int, retail: bytes, native: bytes) -> int:
    """Accept identity only for identical programs; otherwise require proof.

    Byte similarity can suggest a location to investigate, but does not prove
    matching instruction boundaries, operands, or control flow. It must never
    normalize a verification result into agreement.
    """
    if not 0 <= source_cursor < len(retail):
        raise RuntimeError(f"retail path cursor is outside the catalog: {source_cursor:#06x}")
    if retail == native:
        return source_cursor
    raise RuntimeError(
        f"retail path cursor {source_cursor:#06x} has no verified instruction mapping; "
        "the retail and native path catalogs differ"
    )


def suggest_path_cursor(source_cursor: int, retail: bytes, native: bytes) -> int:
    """Diagnostic suggestion only. Never called by the parity comparator."""
    if not 0 <= source_cursor < len(retail):
        raise RuntimeError(f"retail path cursor is outside the catalog: {source_cursor:#06x}")

    first = max(0, source_cursor - PATH_CURSOR_WINDOW_RADIUS)
    last = min(len(retail), source_cursor + PATH_CURSOR_WINDOW_RADIUS + 1)
    span = last - first
    candidates: list[tuple[int, int]] = []
    for delta in range(-PATH_CURSOR_MAX_RELOCATION, PATH_CURSOR_MAX_RELOCATION + 1):
        native_first = first + delta
        native_last = last + delta
        if native_first < 0 or native_last > len(native):
            continue
        score = sum(
            source == candidate
            for source, candidate in zip(
                retail[first:last], native[native_first:native_last]
            )
        )
        candidates.append((score, delta))

    candidates.sort(reverse=True)
    best_score, best_delta = candidates[0]
    second_score = candidates[1][0]
    if best_score * 5 < span * 2:
        raise RuntimeError(
            f"retail path cursor {source_cursor:#06x} has weak relocation evidence "
            f"({best_score}/{span} matching bytes)"
        )
    if best_score - second_score < PATH_CURSOR_MINIMUM_SCORE_MARGIN:
        raise RuntimeError(
            f"retail path cursor {source_cursor:#06x} has ambiguous relocation evidence "
            f"(best={best_score}, second={second_score})"
        )
    return source_cursor + best_delta


def parse_fields(line: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for token in line.split():
        if "=" not in token:
            raise RuntimeError(f"malformed semantic field: {token!r}")
        name, value = token.split("=", 1)
        if not name or name in fields:
            raise RuntimeError(f"empty or duplicate semantic field: {name!r}")
        fields[name] = value
    return fields


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
    catalogs: tuple[bytes, bytes] | None = None
    for key, fields in objects.items():
        for name in MESEN_ONLY_OBJECT_FIELDS:
            fields.pop(name)
        if fields.get("state_word_2_role") == PATH_CURSOR_ROLE:
            if catalogs is None:
                catalogs = path_catalogs()
            source_cursor = int(fields["state_word_2"])
            try:
                fields["state_word_2"] = str(
                    relocate_path_cursor(source_cursor, *catalogs)
                )
            except RuntimeError as error:
                raise RuntimeError(
                    f"object {key} cannot canonicalize path cursor: {error}"
                ) from error


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


def compare(
    mesen_text: str,
    native_text: str,
    checkpoint_scenes: tuple[int, ...] = CHECKPOINT_SCENES,
) -> None:
    if not checkpoint_scenes or len(set(checkpoint_scenes)) != len(checkpoint_scenes):
        raise RuntimeError("semantic scene selection must be nonempty and unique")
    mesen_scenes, mesen_objects = parse_semantic(mesen_text)
    native_scenes, native_objects = parse_semantic(native_text)
    selected = set(checkpoint_scenes)
    missing_mesen = sorted(selected - mesen_scenes.keys())
    missing_native = sorted(selected - native_scenes.keys())
    if missing_mesen or missing_native:
        raise RuntimeError(
            "semantic checkpoint evidence is incomplete: "
            f"Mesen missing={missing_mesen} native missing={missing_native}"
        )
    mesen_scenes = {key: value for key, value in mesen_scenes.items() if key in selected}
    mesen_objects = {
        key: value for key, value in mesen_objects.items() if key[0] in selected
    }
    native_scenes = {key: value for key, value in native_scenes.items() if key in selected}
    native_objects = {
        key: value for key, value in native_objects.items() if key[0] in selected
    }
    validate_object_inventory("Mesen", mesen_scenes, mesen_objects)
    validate_object_inventory("native", native_scenes, native_objects)
    normalize_mesen(mesen_scenes, mesen_objects)
    normalize_native(native_objects)
    failures: list[str] = []
    for scene in sorted(selected):
        failures.extend(compare_records(
            "scene", {scene: mesen_scenes[scene]}, {scene: native_scenes[scene]}
        ))
        failures.extend(compare_records(
            "object",
            {key: value for key, value in mesen_objects.items() if key[0] == scene},
            {key: value for key, value in native_objects.items() if key[0] == scene},
        ))
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
    env = os.environ.copy()
    # A caller's diagnostic environment must not narrow the verification run
    # or silently select a different input tape.
    for name in (
        "SF1_NATIVE_SEMANTIC_SCENES", "SF1_NATIVE_SEMANTIC_ROUTE",
        "SF1_NATIVE_SEMANTIC_NO_OBJECTS",
    ):
        env.pop(name, None)
    env["SF1_NATIVE_SEMANTIC_RANGE"] = f"{FIRST_SCENE}-{LAST_SCENE}"
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
        env=env,
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
    parser.add_argument("--output-dir", type=Path, help="retain both captures for diagnosis")
    parser.add_argument(
        "--scenes",
        help="comma-separated decimal checkpoint scenes (artifact diagnostics only)",
    )
    args = parser.parse_args()
    if (args.artifact is None) != (args.native_output is None):
        parser.error("--artifact and --native-output must be supplied together")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    checkpoint_scenes = CHECKPOINT_SCENES
    if args.scenes is not None:
        if args.artifact is None:
            parser.error("--scenes is only valid with supplied artifacts")
        try:
            checkpoint_scenes = tuple(int(value) for value in args.scenes.split(","))
        except ValueError:
            parser.error("--scenes must contain comma-separated decimal values")
        if not checkpoint_scenes or any(scene < 0 for scene in checkpoint_scenes):
            parser.error("--scenes must contain at least one non-negative scene")
        if len(set(checkpoint_scenes)) != len(checkpoint_scenes):
            parser.error("--scenes must not contain duplicates")

    if args.artifact is not None:
        compare(
            args.artifact.read_text(encoding="utf-8"),
            args.native_output.read_text(encoding="utf-8"),
            checkpoint_scenes,
        )
    else:
        if args.mesen_bin is None:
            parser.error("--mesen-bin is required unless artifacts are supplied")
        verify_rom()
        output = args.output_dir or Path(tempfile.mkdtemp(prefix="sf1-corneria-semantic."))
        output.mkdir(parents=True, exist_ok=True)
        print(f"Semantic evidence directory: {output.resolve()}", flush=True)
        artifact = run_mesen(args.mesen_bin, args.timeout, output / "mesen")
        native_text = run_native()
        (output / "native.txt").write_text(native_text, encoding="utf-8")
        compare(artifact.read_text(encoding="utf-8"), native_text)
    print(
        f"Compared {len(checkpoint_scenes)} Corneria semantic scenes. "
        "This check alone does not certify shape identity, pixels, audio, or full-game coverage."
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f"Semantic verification failed: {error}", file=sys.stderr)
        raise SystemExit(1)
