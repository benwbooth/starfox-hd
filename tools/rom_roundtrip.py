#!/usr/bin/env python3
"""Rebuild a retail SNES ROM losslessly while promoting reviewed regions to ASM.

The input ROM remains the authority.  Bytes outside promoted regions are
included directly; promoted regions are assembled from tracked WLA-DX source.
The command succeeds only when the linked image is byte-identical to the
hash-bound input ROM.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MANIFEST_SCHEMA = 1
DEFAULT_BANK_SIZE = 32_768
ASSEMBLER_PROGRAM = "wla-65816"
LINKER_PROGRAM = "wlalink"
GENERATED_SOURCE_NAME = "roundtrip.asm"
GENERATED_OBJECT_NAME = "roundtrip.o"
GENERATED_LINK_NAME = "roundtrip.link"
GENERATED_ROM_NAME = "roundtrip.sfc"


@dataclass(frozen=True)
class Region:
    name: str
    offset: int
    size: int
    source: Path

    @property
    def end(self) -> int:
        return self.offset + self.size


@dataclass(frozen=True)
class Manifest:
    identifier: str
    title: str
    sha256: str
    size: int
    bank_size: int
    regions: tuple[Region, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Assemble reviewed regions and prove a byte-exact retail-ROM rebuild."
    )
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--rom", type=Path, required=True)
    parser.add_argument(
        "--output",
        type=Path,
        help="optional destination for the verified rebuilt ROM",
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        help="retain generated assembler, object, symbols, and ROM here",
    )
    return parser.parse_args()


def require_integer(value: Any, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"{field} must be an integer")
    return value


def load_manifest(path: Path) -> Manifest:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if require_integer(raw.get("schema"), "schema") != MANIFEST_SCHEMA:
        raise ValueError(f"unsupported manifest schema: {raw.get('schema')!r}")

    base = path.resolve().parent
    regions = []
    for index, item in enumerate(raw.get("regions", [])):
        source = (base / item["source"]).resolve()
        if not source.is_file():
            raise ValueError(f"region {index} source does not exist: {source}")
        regions.append(
            Region(
                name=str(item["name"]),
                offset=require_integer(item["offset"], f"regions[{index}].offset"),
                size=require_integer(item["size"], f"regions[{index}].size"),
                source=source,
            )
        )

    manifest = Manifest(
        identifier=str(raw["id"]),
        title=str(raw["title"]),
        sha256=str(raw["sha256"]).lower(),
        size=require_integer(raw["size"], "size"),
        bank_size=require_integer(raw.get("bank_size", DEFAULT_BANK_SIZE), "bank_size"),
        regions=tuple(sorted(regions, key=lambda region: region.offset)),
    )
    validate_manifest(manifest)
    return manifest


def validate_manifest(manifest: Manifest) -> None:
    if manifest.size <= 0 or manifest.size % manifest.bank_size != 0:
        raise ValueError("ROM size must be a positive whole number of banks")
    if manifest.bank_size != DEFAULT_BANK_SIZE:
        raise ValueError("only headerless 32 KiB LoROM banks are currently supported")
    if len(manifest.sha256) != 64 or any(
        character not in "0123456789abcdef" for character in manifest.sha256
    ):
        raise ValueError("sha256 must contain exactly 64 lowercase hexadecimal digits")

    previous_end = 0
    for region in manifest.regions:
        if region.size <= 0:
            raise ValueError(f"region {region.name!r} must have a positive size")
        if region.offset < previous_end:
            raise ValueError(f"region {region.name!r} overlaps the preceding region")
        if region.end > manifest.size:
            raise ValueError(f"region {region.name!r} extends beyond the ROM")
        if region.offset // manifest.bank_size != (region.end - 1) // manifest.bank_size:
            raise ValueError(f"region {region.name!r} crosses a LoROM bank boundary")
        previous_end = region.end


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def quoted_path(path: Path) -> str:
    text = str(path.resolve())
    if '"' in text or "\n" in text or "\r" in text:
        raise ValueError(f"path cannot be represented safely in WLA source: {path}")
    return f'"{text}"'


def emit_incbin(lines: list[str], rom: Path, offset: int, size: int) -> None:
    if size > 0:
        lines.append(f".INCBIN {quoted_path(rom)} SKIP {offset} READ {size}")


def generate_source(manifest: Manifest, rom: Path) -> str:
    bank_count = manifest.size // manifest.bank_size
    lines = [
        "; Generated by tools/rom_roundtrip.py. Do not edit.",
        ".MEMORYMAP",
        "  DEFAULTSLOT 0",
        f"  SLOTSIZE {manifest.bank_size}",
        "  SLOT 0 $8000",
        ".ENDME",
        "",
        ".ROMBANKMAP",
        f"  BANKSTOTAL {bank_count}",
        f"  BANKSIZE {manifest.bank_size}",
        f"  BANKS {bank_count}",
        ".ENDRO",
        "",
    ]

    regions_by_bank: dict[int, list[Region]] = {}
    for region in manifest.regions:
        regions_by_bank.setdefault(region.offset // manifest.bank_size, []).append(region)

    for bank in range(bank_count):
        bank_start = bank * manifest.bank_size
        bank_end = bank_start + manifest.bank_size
        cursor = bank_start
        lines.extend((f".BANK {bank} SLOT 0", ".ORG 0"))
        for region in regions_by_bank.get(bank, []):
            emit_incbin(lines, rom, cursor, region.offset - cursor)
            lines.append(f"; promoted region: {region.name}")
            lines.append(f".INCLUDE {quoted_path(region.source)}")
            cursor = region.end
        emit_incbin(lines, rom, cursor, bank_end - cursor)
        lines.append("")
    return "\n".join(lines)


def run_command(command: list[str], work_dir: Path) -> None:
    completed = subprocess.run(
        command,
        cwd=work_dir,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if completed.returncode != 0:
        rendered = " ".join(command)
        raise RuntimeError(f"command failed ({completed.returncode}): {rendered}\n{completed.stdout}")


def first_difference(expected: bytes, actual: bytes) -> dict[str, int | None]:
    shared = min(len(expected), len(actual))
    for offset in range(shared):
        if expected[offset] != actual[offset]:
            return {
                "offset": offset,
                "expected": expected[offset],
                "actual": actual[offset],
            }
    if len(expected) != len(actual):
        return {"offset": shared, "expected": None, "actual": None}
    return {"offset": None, "expected": None, "actual": None}


def rebuild(manifest: Manifest, rom: Path, work_dir: Path) -> tuple[Path, dict[str, Any]]:
    original = rom.read_bytes()
    if len(original) != manifest.size:
        raise ValueError(
            f"{manifest.title}: expected {manifest.size} bytes, found {len(original)}"
        )
    original_digest = digest(original)
    if original_digest != manifest.sha256:
        raise ValueError(
            f"{manifest.title}: SHA-256 mismatch; expected {manifest.sha256}, "
            f"found {original_digest}"
        )

    work_dir.mkdir(parents=True, exist_ok=True)
    source = work_dir / GENERATED_SOURCE_NAME
    source.write_text(generate_source(manifest, rom), encoding="utf-8")
    (work_dir / GENERATED_LINK_NAME).write_text(
        f"[objects]\n{GENERATED_OBJECT_NAME}\n", encoding="utf-8"
    )

    run_command(
        [ASSEMBLER_PROGRAM, "-i", "-o", GENERATED_OBJECT_NAME, GENERATED_SOURCE_NAME],
        work_dir,
    )
    run_command(
        [LINKER_PROGRAM, "-S", "-A", GENERATED_LINK_NAME, GENERATED_ROM_NAME],
        work_dir,
    )

    rebuilt_path = work_dir / GENERATED_ROM_NAME
    rebuilt = rebuilt_path.read_bytes()
    rebuilt_digest = digest(rebuilt)
    report: dict[str, Any] = {
        "id": manifest.identifier,
        "title": manifest.title,
        "status": "exact" if rebuilt == original else "different",
        "input_sha256": original_digest,
        "rebuilt_sha256": rebuilt_digest,
        "size": len(rebuilt),
        "bank_count": manifest.size // manifest.bank_size,
        "promoted_regions": len(manifest.regions),
        "promoted_bytes": sum(region.size for region in manifest.regions),
    }
    if rebuilt != original:
        report["first_difference"] = first_difference(original, rebuilt)
        raise RuntimeError(json.dumps(report, indent=2, sort_keys=True))
    return rebuilt_path, report


def main() -> int:
    args = parse_args()
    try:
        manifest = load_manifest(args.manifest)
        rom = args.rom.resolve()
        if args.work_dir is not None:
            rebuilt_path, report = rebuild(manifest, rom, args.work_dir.resolve())
            retained_path = rebuilt_path
        else:
            with tempfile.TemporaryDirectory(prefix=f"{manifest.identifier}-roundtrip-") as temp:
                rebuilt_path, report = rebuild(manifest, rom, Path(temp))
                if args.output is not None:
                    args.output.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(rebuilt_path, args.output)
                retained_path = args.output

        if args.work_dir is not None and args.output is not None:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(rebuilt_path, args.output)
            retained_path = args.output
        if retained_path is not None:
            report["output"] = str(retained_path)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    except (OSError, ValueError, RuntimeError) as error:
        print(f"rom-roundtrip: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
