#!/usr/bin/env python3
"""Generate native hostile-projectile dynamics for both final rival sorties."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from generate_second_sortie_projectiles import (
    generate_dynamics,
    import_raw_logic,
    read_pose_fixture,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE_DIRECTORY = Path(__file__).with_name("fixtures")
EXPECTED_PROJECTILE_LIFETIMES = 3


@dataclass(frozen=True)
class ProjectileEncounter:
    label: str
    raw_sample_start_elapsed: int
    logic_fixture: Path
    pose_fixture: Path
    output: Path


ENCOUNTERS = (
    ProjectileEncounter(
        "the final pursuer sortie",
        133_432,
        FIXTURE_DIRECTORY / "final_pursuer_projectile_logic.trace",
        FIXTURE_DIRECTORY / "final_pursuer_path.trace",
        REPO_ROOT
        / "rust"
        / "sf2-game"
        / "src"
        / "native"
        / "final_pursuer_projectiles.rs",
    ),
    ProjectileEncounter(
        "the Wolf blockade sortie",
        135_728,
        FIXTURE_DIRECTORY / "wolf_blockade_projectile_logic.trace",
        FIXTURE_DIRECTORY / "wolf_blockade_path.trace",
        REPO_ROOT
        / "rust"
        / "sf2-game"
        / "src"
        / "native"
        / "wolf_blockade_projectiles.rs",
    ),
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--import-raw", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    for encounter in ENCOUNTERS:
        if args.import_raw is not None:
            _, lifetimes = read_pose_fixture(
                encounter.pose_fixture,
                EXPECTED_PROJECTILE_LIFETIMES,
            )
            import_raw_logic(
                args.import_raw,
                encounter.logic_fixture,
                encounter.raw_sample_start_elapsed,
                encounter.label,
                frozenset(lifetime.source for lifetime in lifetimes),
            )
        source, retained_pose_count = generate_dynamics(
            encounter.logic_fixture,
            encounter.pose_fixture,
            EXPECTED_PROJECTILE_LIFETIMES,
            encounter.raw_sample_start_elapsed,
            encounter.label,
        )
        if args.check:
            if not encounter.output.exists() or encounter.output.read_text(
                encoding="utf-8"
            ) != source:
                raise SystemExit(
                    f"generated final-rival projectile dynamics are stale: "
                    f"{encounter.output}"
                )
        else:
            encounter.output.parent.mkdir(parents=True, exist_ok=True)
            encounter.output.write_text(source, encoding="utf-8")
        print(
            f"{encounter.label} projectile replay verified: "
            f"{retained_pose_count} retained pose boundaries"
        )


if __name__ == "__main__":
    main()
