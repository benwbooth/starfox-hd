#!/usr/bin/env python3
"""Reject emulator-shaped dependencies and APIs in the shipping Rust port."""

from __future__ import annotations

import re
import sys
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
RUST = REPOSITORY / "rust"

SHIPPING_SOURCE_ROOTS = (
    RUST / "sf-app" / "src",
    RUST / "sf-audio" / "src",
    RUST / "sf-core" / "src",
    RUST / "sf-game" / "src",
    RUST / "sf-map" / "src",
    RUST / "sf-path" / "src",
    RUST / "sf-render" / "src",
    RUST / "sf-strat" / "src",
    RUST / "sf2-data" / "src",
    RUST / "sf2-game" / "src",
)

ORACLE_ONLY_SOURCE = {
    (RUST / "sf-audio" / "src" / "backend.rs").resolve(),
    (RUST / "sf-audio" / "src" / "boot.rs").resolve(),
    (RUST / "sf-audio" / "src" / "native.rs").resolve(),
    (RUST / "sf-audio" / "src" / "player.rs").resolve(),
    (RUST / "sf2-game" / "src" / "cpu_bridge.rs").resolve(),
    (RUST / "sf2-game" / "src" / "map_host.rs").resolve(),
    (RUST / "sf2-game" / "src" / "memory.rs").resolve(),
    (RUST / "sf2-game" / "src" / "object.rs").resolve(),
    (RUST / "sf2-game" / "src" / "oracle_compat.rs").resolve(),
    (RUST / "sf2-game" / "src" / "path_host.rs").resolve(),
    (RUST / "sf2-game" / "src" / "strategy.rs").resolve(),
    (RUST / "sf2-data" / "src" / "draw.rs").resolve(),
    (RUST / "sf2-data" / "src" / "map.rs").resolve(),
    (RUST / "sf2-data" / "src" / "map_vm.rs").resolve(),
    (RUST / "sf2-data" / "src" / "path.rs").resolve(),
}

# Retained map/path programs still carry encoded variable operands. Their
# decoder is deliberately confined to these import-boundary files; ordinary
# gameplay code must use `GameVars` fields directly.
IMPORTED_OPERAND_BOUNDARY = {
    (RUST / "sf-game" / "src" / "vars.rs").resolve(),
    (RUST / "sf-game" / "src" / "game.rs").resolve(),
    (RUST / "sf-strat" / "src" / "path_adapter.rs").resolve(),
}

IMPORTED_OPERAND_PATTERN = re.compile(
    r"\b(?:read_ext8|read_ext16|write_ext8|write_ext16)\b"
)

FORBIDDEN_SOURCE_PATTERNS = {
    "oracle crate import": re.compile(r"\bsf_oracle\b"),
    "retail machine runtime": re.compile(r"\bRetailMachine\b"),
    "CPU execution dependency": re.compile(r"\bw65c816\b"),
    "segmented mutable-state API": re.compile(r"\b(?:read|write)_wram(?:_byte|_word)?\b"),
    "segmented map cursor": re.compile(r"\bmap_?bank\b", re.IGNORECASE),
    "coprocessor register file": re.compile(r"\bgsu_(?:regs|pbr|sfr)\b"),
    "processor register vocabulary": re.compile(
        r"\b(?:cpu_regs?|program_counter|stack_pointer|status_register|"
        r"register_[axy]|[axy]_register|cpu_[axy])\b"
    ),
    "numbered processor register identifier": re.compile(
        r"\br(?:1[0-5]|[0-9])\s*[:=]", re.IGNORECASE
    ),
    "processor instruction choreography": re.compile(
        r"\b(?:lda|ldx|ldy|sta|stx|sty|tax|tay|txa|tya|pha|pla|phx|plx|phy|ply|"
        r"tsx|txs|txy|tyx)\b",
        re.IGNORECASE,
    ),
    "addressed memory model": re.compile(r"\bself\.memory\b"),
    "addressed RAM model": re.compile(r"\bself\.ram\b|\.ram\s*\["),
    "generic memory field": re.compile(r"\b(?:pub\s+)?(?:memory|ram)\s*:"),
    "generic mutable memory type": re.compile(r"\b(?:struct|enum|type)\s+Memory\b"),
    "address-based state API": re.compile(
        r"\b(?:read|write)_(?:byte|word|long_byte|long_word)\b"
    ),
}


def source_violations() -> list[str]:
    failures: list[str] = []
    for root in SHIPPING_SOURCE_ROOTS:
        for path in sorted(root.rglob("*.rs")):
            if path.resolve() in ORACLE_ONLY_SOURCE:
                continue
            text = path.read_text(encoding="utf-8")
            if path.resolve() not in IMPORTED_OPERAND_BOUNDARY:
                for match in IMPORTED_OPERAND_PATTERN.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    relative = path.relative_to(REPOSITORY)
                    failures.append(
                        f"{relative}:{line}: imported operand decoder escaped "
                        f"map/path boundary: {match.group(0)}"
                    )
            for description, pattern in FORBIDDEN_SOURCE_PATTERNS.items():
                for match in pattern.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    relative = path.relative_to(REPOSITORY)
                    failures.append(f"{relative}:{line}: {description}: {match.group(0)}")
    return failures


def manifest_violations() -> list[str]:
    failures: list[str] = []
    app_manifest = (RUST / "sf-app" / "Cargo.toml").read_text(encoding="utf-8")
    if "sf-oracle" in app_manifest:
        failures.append("rust/sf-app/Cargo.toml: shipping app depends on sf-oracle")
    if not re.search(
        r'sf2-game\s*=\s*\{[^}]*default-features\s*=\s*false', app_manifest
    ):
        failures.append(
            "rust/sf-app/Cargo.toml: sf2-game must disable default features"
        )

    audio_manifest = (RUST / "sf-audio" / "Cargo.toml").read_text(encoding="utf-8")
    audio_dependency = re.search(r"^sf-spc\s*=\s*(.+)$", audio_manifest, re.MULTILINE)
    if audio_dependency and "optional = true" not in audio_dependency.group(1):
        failures.append(
            "rust/sf-audio/Cargo.toml: sf-spc must be optional and oracle-only"
        )
    if not re.search(
        r'oracle-audio\s*=\s*\[[^\]]*"dep:sf-spc"', audio_manifest
    ):
        failures.append(
            "rust/sf-audio/Cargo.toml: missing explicit oracle-audio feature"
        )
    if "oracle-audio" in app_manifest:
        failures.append("rust/sf-app/Cargo.toml: shipping app enables oracle-audio")

    game_manifest = (RUST / "sf2-game" / "Cargo.toml").read_text(encoding="utf-8")
    dependency = re.search(r"^w65c816\s*=\s*(.+)$", game_manifest, re.MULTILINE)
    if dependency and "optional = true" not in dependency.group(1):
        failures.append(
            "rust/sf2-game/Cargo.toml: w65c816 must be optional and oracle-only"
        )
    if not re.search(
        r'oracle-bridge\s*=\s*\[[^\]]*"dep:w65c816"', game_manifest
    ):
        failures.append(
            "rust/sf2-game/Cargo.toml: missing explicit oracle-bridge feature"
        )
    return failures


def main() -> int:
    failures = manifest_violations() + source_violations()
    if failures:
        print("Native architecture check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1
    print("Native architecture check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
