#!/usr/bin/env python3
"""Regenerate the two typed final-campaign rival paths from compact fixtures."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from generate_pigma_duel import (
    DEFAULT_PROJECTILE_SHAPE_TOKENS,
    load,
    projectile_lifetimes,
    rust_source,
)


REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class RivalPath:
    fixture: Path
    output: Path
    name: str


PATHS = (
    RivalPath(
        Path(__file__).with_name("fixtures") / "final_pursuer_path.trace",
        REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "final_pursuer.rs",
        "final pursuer",
    ),
    RivalPath(
        Path(__file__).with_name("fixtures") / "wolf_blockade_path.trace",
        REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "wolf_blockade.rs",
        "Wolf blockade",
    ),
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for path in PATHS:
        records, return_frame, map_ready_frame = load(
            (path.fixture,),
            DEFAULT_PROJECTILE_SHAPE_TOKENS,
            path.name,
            "0576",
            "C348",
            "7",
        )
        generated = rust_source(
            path.fixture.name,
            records,
            return_frame,
            map_ready_frame,
            path.name,
            Path(__file__).name,
            rival_test_only=True,
            projectiles_test_only=True,
        )
        if args.check:
            if not path.output.is_file() or path.output.read_text(encoding="utf-8") != generated:
                raise SystemExit(f"generated source is out of date: {path.output}")
            action = "verified"
        else:
            path.output.write_text(generated, encoding="utf-8")
            action = "generated"
        print(
            f"{action} {path.output}: {len(records)} keyframes, "
            f"return {return_frame}, map ready {map_ready_frame}, "
            f"enemy laser tracks {len(projectile_lifetimes(records))}"
        )


if __name__ == "__main__":
    main()
