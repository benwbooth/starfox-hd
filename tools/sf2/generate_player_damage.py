#!/usr/bin/env python3
"""Generate typed player-damage timing from the retained retail trace summary."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_FIXTURE = Path(__file__).with_name("fixtures") / "player_damage.json"
DEFAULT_OUTPUT = (
    REPO_ROOT / "rust" / "sf2-game" / "src" / "native" / "player_damage.rs"
)


def generated_source(fixture: Path) -> str:
    values = json.loads(fixture.read_text(encoding="utf-8"))
    required = {
        "hostile_projectile_attack_power",
        "hostile_projectile_player_offset_min",
        "hostile_projectile_player_offset_max",
        "shield_before_first_hit",
        "shield_after_first_hit",
        "first_damage_retail_frame",
        "recovery_complete_retail_frame",
        "lethal_damage_retail_frame",
        "game_over_retail_frame",
        "continue_prompt_retail_frame",
        "strategic_map_retail_frame",
        "reserve_shield_before_handoff",
        "active_shield_after_handoff",
        "reserve_shield_after_handoff",
    }
    missing = required.difference(values)
    if missing:
        raise ValueError(f"fixture is missing fields: {', '.join(sorted(missing))}")

    attack_power = int(values["hostile_projectile_attack_power"])
    collision_minimum = tuple(
        int(value) for value in values["hostile_projectile_player_offset_min"]
    )
    collision_maximum = tuple(
        int(value) for value in values["hostile_projectile_player_offset_max"]
    )
    shield_before = int(values["shield_before_first_hit"])
    shield_after = int(values["shield_after_first_hit"])
    first_damage = int(values["first_damage_retail_frame"])
    recovery_complete = int(values["recovery_complete_retail_frame"])
    lethal_damage = int(values["lethal_damage_retail_frame"])
    game_over = int(values["game_over_retail_frame"])
    continue_prompt = int(values["continue_prompt_retail_frame"])
    strategic_map = int(values["strategic_map_retail_frame"])
    reserve_before = int(values["reserve_shield_before_handoff"])
    active_after = int(values["active_shield_after_handoff"])
    reserve_after = int(values["reserve_shield_after_handoff"])

    if shield_before != 1 or shield_after != 0:
        raise ValueError("fixture must retain the zero-shield-survives boundary")
    if reserve_before != active_after or reserve_after != 0:
        raise ValueError("fixture must retain the reserve-pilot handoff")
    if len(collision_minimum) != 3 or len(collision_maximum) != 3:
        raise ValueError("collision limits must contain x, y, and z")

    collision_centers = []
    collision_extents = []
    for minimum, maximum in zip(collision_minimum, collision_maximum, strict=True):
        if minimum > maximum or (minimum + maximum) % 2 != 0:
            raise ValueError("collision limits must form an integral centered box")
        center = (minimum + maximum) // 2
        collision_centers.append(center)
        collision_extents.append(maximum - center + 1)

    recovery_frames = recovery_complete - first_damage
    # The trace records the first frame after each completed transition. The
    # native timeline advances in four-frame quanta, so the executable
    # duration ends one retail frame before the observed mode write.
    destruction_frames = game_over - lethal_damage - 1
    prompt_frames = continue_prompt - game_over
    return_frames = strategic_map - continue_prompt - 1
    for name, value in {
        "attack power": attack_power,
        "recovery duration": recovery_frames,
        "destruction duration": destruction_frames,
        "prompt duration": prompt_frames,
        "return duration": return_frames,
    }.items():
        if value <= 0:
            raise ValueError(f"{name} must be positive")

    return f'''//! Generated semantic timing for hostile fire against the player craft.
//!
//! Source: `player_damage.json`, reduced from a clean retail two-hit trace.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_player_damage.py [--check]`.

use super::{{CollisionBounds, Vector3}};

pub(super) const HOSTILE_PROJECTILE_ATTACK_POWER: u8 = {attack_power};
pub(super) const HOSTILE_PROJECTILE_PLAYER_OFFSET_CENTER: Vector3 = Vector3 {{
    x: {collision_centers[0]},
    y: {collision_centers[1]},
    z: {collision_centers[2]},
}};
pub(super) const HOSTILE_PROJECTILE_PLAYER_COLLISION_EXTENTS: CollisionBounds = CollisionBounds {{
    x: {collision_extents[0]},
    y: {collision_extents[1]},
    z: {collision_extents[2]},
}};
pub(super) const PLAYER_HIT_RECOVERY_RETAIL_FRAMES: u8 = {recovery_frames};
pub(super) const PLAYER_DESTRUCTION_RETAIL_FRAMES: u16 = {destruction_frames};
pub(super) const GAME_OVER_PROMPT_RETAIL_FRAMES: u16 = {prompt_frames};
pub(super) const CONTINUE_RETURN_RETAIL_FRAMES: u16 = {return_frames};

#[cfg(test)]
pub(super) const ORACLE_ZERO_SHIELD_SURVIVES: bool = true;
#[cfg(test)]
pub(super) const ORACLE_RESERVE_SHIELD: u8 = {reserve_before};
'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source = generated_source(args.fixture)
    if args.check:
        if not args.output.is_file() or args.output.read_text(encoding="utf-8") != source:
            parser.error(f"generated output is stale: {args.output}")
        return 0

    args.output.write_text(source, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
