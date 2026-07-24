//! Generated semantic timing for hostile contact against the player craft.
//!
//! Source: `player_damage.json`, reduced from clean projectile and Eladard-contact traces.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_player_damage.py [--check]`.

use super::{CollisionBounds, Vector3};

pub(super) const HOSTILE_PROJECTILE_ATTACK_POWER: u8 = 2;
pub(super) const ELADARD_DEFENDER_CONTACT_DAMAGE: u8 = 4;
pub(super) const ELADARD_DEFENDER_CONTACT_RECOVERY_RETAIL_FRAMES: u8 = 56;
pub(super) const HOSTILE_PROJECTILE_PLAYER_OFFSET_CENTER: Vector3 = Vector3 {
    x: -9,
    y: 34,
    z: -11,
};
pub(super) const HOSTILE_PROJECTILE_PLAYER_COLLISION_EXTENTS: CollisionBounds = CollisionBounds {
    x: 68,
    y: 84,
    z: 65,
};
pub(super) const PLAYER_HIT_RECOVERY_RETAIL_FRAMES: u8 = 40;
pub(super) const PLAYER_DESTRUCTION_RETAIL_FRAMES: u16 = 280;
pub(super) const GAME_OVER_PROMPT_RETAIL_FRAMES: u16 = 400;
pub(super) const CONTINUE_RETURN_RETAIL_FRAMES: u16 = 172;

#[cfg(test)]
pub(super) const ORACLE_ZERO_SHIELD_SURVIVES: bool = true;
#[cfg(test)]
pub(super) const ORACLE_RESERVE_SHIELD: u8 = 40;
