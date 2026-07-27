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
        "static_sources",
        "hostile_projectile_attack_power",
        "player_hit_bank_impulse",
        "player_hit_bank_recovery_divisor",
        "camera_hit_pitch_recoil",
        "camera_hit_pitch_recoil_step",
        "camera_hit_pitch_recoil_scale",
        "eladard_defender_contact_damage",
        "eladard_defender_first_damage_retail_frame",
        "eladard_defender_second_damage_retail_frame",
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

    static_sources = values["static_sources"]
    required_static_sources = {
        "impact_response",
        "bank_recovery",
        "proportional_approach",
        "camera_recoil",
        "camera_pitch_composition",
    }
    if not isinstance(static_sources, dict):
        raise ValueError("static_sources must be an object")
    missing_static_sources = required_static_sources - static_sources.keys()
    if missing_static_sources:
        raise ValueError(
            "fixture is missing static sources: "
            + ", ".join(sorted(missing_static_sources))
        )
    for name in required_static_sources:
        source = static_sources[name]
        if not isinstance(source, str) or not source:
            raise ValueError(f"static source {name!r} must be a non-empty string")

    attack_power = int(values["hostile_projectile_attack_power"])
    hit_bank_impulse = int(values["player_hit_bank_impulse"])
    hit_bank_recovery_divisor = int(values["player_hit_bank_recovery_divisor"])
    camera_hit_pitch_recoil = int(values["camera_hit_pitch_recoil"])
    camera_hit_pitch_recoil_step = int(values["camera_hit_pitch_recoil_step"])
    camera_hit_pitch_recoil_scale = int(values["camera_hit_pitch_recoil_scale"])
    defender_contact_damage = int(values["eladard_defender_contact_damage"])
    defender_first_damage = int(values["eladard_defender_first_damage_retail_frame"])
    defender_second_damage = int(values["eladard_defender_second_damage_retail_frame"])
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
    defender_contact_recovery_frames = defender_second_damage - defender_first_damage
    # The trace records the first frame after each completed transition. The
    # native timeline advances in four-frame quanta, so the executable
    # duration ends one retail frame before the observed mode write.
    destruction_frames = game_over - lethal_damage - 1
    prompt_frames = continue_prompt - game_over
    return_frames = strategic_map - continue_prompt - 1
    for name, value in {
        "attack power": attack_power,
        "player hit bank impulse": hit_bank_impulse,
        "player hit bank recovery divisor": hit_bank_recovery_divisor,
        "camera hit pitch recoil": camera_hit_pitch_recoil,
        "camera hit pitch recoil step": camera_hit_pitch_recoil_step,
        "camera hit pitch recoil scale": camera_hit_pitch_recoil_scale,
        "Eladard defender contact damage": defender_contact_damage,
        "Eladard defender contact recovery duration": defender_contact_recovery_frames,
        "recovery duration": recovery_frames,
        "destruction duration": destruction_frames,
        "prompt duration": prompt_frames,
        "return duration": return_frames,
    }.items():
        if value <= 0:
            raise ValueError(f"{name} must be positive")

    return f'''//! Generated semantic timing for hostile contact against the player craft.
//!
//! Source: `player_damage.json`, reduced from clean projectile and Eladard-contact traces.
//! The hit reaction itself is statically recovered from retail routines
//! `{static_sources["impact_response"]}` (impact), `{static_sources["bank_recovery"]}`
//! and `{static_sources["proportional_approach"]}` (bank recovery),
//! `{static_sources["camera_recoil"]}` (camera recoil), and
//! `{static_sources["camera_pitch_composition"]}` (camera pitch composition).
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_player_damage.py [--check]`.

use super::{{CollisionBounds, Vector3}};

pub(super) const HOSTILE_PROJECTILE_ATTACK_POWER: u8 = {attack_power};
pub(super) const PLAYER_HIT_BANK_IMPULSE: i8 = {hit_bank_impulse};
pub(super) const PLAYER_HIT_BANK_RECOVERY_DIVISOR: i16 = {hit_bank_recovery_divisor};
pub(super) const CAMERA_HIT_PITCH_RECOIL: i16 = {camera_hit_pitch_recoil};
pub(super) const CAMERA_HIT_PITCH_RECOIL_STEP: i16 = {camera_hit_pitch_recoil_step};
pub(super) const CAMERA_HIT_PITCH_RECOIL_SCALE: i16 = {camera_hit_pitch_recoil_scale};
const HIT_DIRECTION_ALTERNATION_MASK: u64 = 1;
pub(super) const ELADARD_DEFENDER_CONTACT_DAMAGE: u8 = {defender_contact_damage};
pub(super) const ELADARD_DEFENDER_CONTACT_RECOVERY_RETAIL_FRAMES: u8 = {defender_contact_recovery_frames};
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

pub(super) fn player_hit_bank_impulse(source_frame: u64) -> i8 {{
    if source_frame & HIT_DIRECTION_ALTERNATION_MASK == 0 {{
        PLAYER_HIT_BANK_IMPULSE
    }} else {{
        -PLAYER_HIT_BANK_IMPULSE
    }}
}}

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
