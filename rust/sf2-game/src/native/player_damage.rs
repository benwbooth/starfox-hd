//! Generated semantic timing and geometry for hostile contact against the player craft.
//!
//! Source: `player_damage.json`, reduced from clean projectile and Eladard-contact traces,
//! with compound laser geometry recovered statically from
//! `STRAT/STRATROU.ASM:generate_collist_l`, `ASM/COLDET.ASM:chkcoll`,
//! and `INC/SHMACS.INC:colbox`.
//! The hit reaction itself is statically recovered from retail routines
//! `06:AAAE` (impact), `06:9195`
//! and `7F:27B5` (bank recovery),
//! `07:9AAB` (camera recoil), and
//! `07:968B` (camera pitch composition).
//! Contact recovery uses `INC/STRATEQU.INC:framesperAP`.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_player_damage.py [--check]`.

use super::{CollisionBounds, ShapeId, Vector3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OrientedCollisionVolume {
    pub center_offset: Vector3,
    pub extents: CollisionBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CompoundCollisionProfile {
    pub scale: u32,
    pub volumes: &'static [OrientedCollisionVolume],
}

pub(super) const HOSTILE_PROJECTILE_ATTACK_POWER: u8 = 2;
pub(super) const PLAYER_HIT_BANK_IMPULSE: i8 = 30;
pub(super) const PLAYER_HIT_BANK_RECOVERY_DIVISOR: i16 = 8;
pub(super) const CAMERA_HIT_PITCH_RECOIL: i16 = 128;
pub(super) const CAMERA_HIT_PITCH_RECOIL_STEP: i16 = 16;
pub(super) const CAMERA_HIT_PITCH_RECOIL_SCALE: i16 = 2;
const HIT_DIRECTION_ALTERNATION_MASK: u64 = 1;
pub(super) const ELADARD_DEFENDER_CONTACT_DAMAGE: u8 = 4;
pub(super) const ELADARD_DEFENDER_CONTACT_RECOVERY_RETAIL_FRAMES: u8 = 56;
pub(super) const HOSTILE_PROJECTILE_COLLISION_PROFILE: CompoundCollisionProfile =
    CompoundCollisionProfile {
        scale: 2,
        volumes: &[
            OrientedCollisionVolume {
                center_offset: Vector3 { x: 0, y: 0, z: 0 },
                extents: CollisionBounds {
                    x: 40,
                    y: 40,
                    z: 40,
                },
            },
            OrientedCollisionVolume {
                center_offset: Vector3 { x: 0, y: 0, z: -20 },
                extents: CollisionBounds {
                    x: 40,
                    y: 40,
                    z: 40,
                },
            },
            OrientedCollisionVolume {
                center_offset: Vector3 { x: 0, y: 0, z: -40 },
                extents: CollisionBounds {
                    x: 40,
                    y: 40,
                    z: 40,
                },
            },
        ],
    };
const FOX_FALCO_FLIGHT_COLLISION_PROFILE: CompoundCollisionProfile = CompoundCollisionProfile {
    scale: 0,
    volumes: &[
        OrientedCollisionVolume {
            center_offset: Vector3 { x: 0, y: 0, z: 0 },
            extents: CollisionBounds {
                x: 10,
                y: 10,
                z: 10,
            },
        },
        OrientedCollisionVolume {
            center_offset: Vector3 {
                x: -25,
                y: 10,
                z: -15,
            },
            extents: CollisionBounds {
                x: 10,
                y: 10,
                z: 10,
            },
        },
        OrientedCollisionVolume {
            center_offset: Vector3 {
                x: 25,
                y: 10,
                z: -15,
            },
            extents: CollisionBounds {
                x: 10,
                y: 10,
                z: 10,
            },
        },
    ],
};
const MIYU_FAY_FLIGHT_COLLISION_PROFILE: CompoundCollisionProfile = CompoundCollisionProfile {
    scale: 0,
    volumes: &[
        OrientedCollisionVolume {
            center_offset: Vector3 { x: 0, y: 0, z: 0 },
            extents: CollisionBounds {
                x: 15,
                y: 10,
                z: 15,
            },
        },
        OrientedCollisionVolume {
            center_offset: Vector3 {
                x: -20,
                y: -5,
                z: -10,
            },
            extents: CollisionBounds {
                x: 10,
                y: 10,
                z: 10,
            },
        },
        OrientedCollisionVolume {
            center_offset: Vector3 {
                x: 20,
                y: -5,
                z: -10,
            },
            extents: CollisionBounds {
                x: 10,
                y: 10,
                z: 10,
            },
        },
    ],
};

pub(super) fn player_compound_collision_profile(
    shape: ShapeId,
) -> Option<CompoundCollisionProfile> {
    if shape == ShapeId::FOX_FALCO_FLIGHT_CRAFT {
        Some(FOX_FALCO_FLIGHT_COLLISION_PROFILE)
    } else if shape == ShapeId::MIYU_FAY_FLIGHT_CRAFT {
        Some(MIYU_FAY_FLIGHT_COLLISION_PROFILE)
    } else {
        None
    }
}
pub(super) const PLAYER_HIT_RECOVERY_RETAIL_FRAMES: u8 = 40;
pub(super) const PLAYER_DESTRUCTION_RETAIL_FRAMES: u16 = 280;
pub(super) const GAME_OVER_PROMPT_RETAIL_FRAMES: u16 = 400;
pub(super) const CONTINUE_RETURN_RETAIL_FRAMES: u16 = 172;

pub(super) fn player_hit_bank_impulse(source_frame: u64) -> i8 {
    if source_frame & HIT_DIRECTION_ALTERNATION_MASK == 0 {
        PLAYER_HIT_BANK_IMPULSE
    } else {
        -PLAYER_HIT_BANK_IMPULSE
    }
}

#[cfg(test)]
pub(super) const ORACLE_ZERO_SHIELD_SURVIVES: bool = true;
#[cfg(test)]
pub(super) const ORACLE_RESERVE_SHIELD: u8 = 40;
