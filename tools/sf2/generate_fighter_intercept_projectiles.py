#!/usr/bin/env python3
"""Generate native hostile-projectile dynamics for the fighter interception."""

from __future__ import annotations

import argparse
from pathlib import Path

from generate_second_sortie_projectiles import generate_dynamics, import_raw_logic


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_LOGIC_FIXTURE = (
    Path(__file__).with_name("fixtures") / "fighter_intercept_projectile_logic.trace"
)
DEFAULT_POSE_FIXTURE = (
    Path(__file__).with_name("fixtures") / "fighter_intercept.trace"
)
DEFAULT_OUTPUT = (
    REPO_ROOT
    / "rust"
    / "sf2-game"
    / "src"
    / "native"
    / "fighter_intercept_projectiles.rs"
)

RAW_SAMPLE_START_ELAPSED = 27_484
EXPECTED_PROJECTILE_LIFETIMES = 8


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--logic-fixture", type=Path, default=DEFAULT_LOGIC_FIXTURE)
    parser.add_argument("--pose-fixture", type=Path, default=DEFAULT_POSE_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.import_raw is not None:
        import_raw_logic(
            args.import_raw,
            args.logic_fixture,
            RAW_SAMPLE_START_ELAPSED,
            "the three-fighter interception",
        )
    source, retained_pose_count = generate_dynamics(
        args.logic_fixture,
        args.pose_fixture,
        EXPECTED_PROJECTILE_LIFETIMES,
        RAW_SAMPLE_START_ELAPSED,
        "the retail three-fighter interception",
    )
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != source:
            raise SystemExit(f"generated projectile dynamics are stale: {args.output}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(source, encoding="utf-8")
    print(
        "fighter-intercept projectile replay verified: "
        f"{retained_pose_count} retained pose boundaries"
    )


if __name__ == "__main__":
    main()
