//! Retail-derived semantic layout and timing for Fortuna's planetary base.
//!
//! The oracle trace contains the original object records; this shipping module
//! keeps only world-space poses, durability, phase thresholds, and timings.

use super::super::object::{Angle, Vector3};
use super::super::render::MaterialSetId;

pub(super) const SURFACE_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -1_150,
    z: -3_072,
};
pub(super) const SURFACE_START_SPEED: u8 = 30;
pub(super) const SURFACE_SWITCH_POSITIONS: [Vector3; 2] = [
    Vector3 {
        x: 1_800,
        y: -1_045,
        z: 580,
    },
    Vector3 {
        x: -120,
        y: -1_095,
        z: 3_000,
    },
];
pub(super) const SURFACE_SWITCH_YAW: Angle = Angle::from_units(192);
pub(super) const INSTALLATION_POSITION: Vector3 = Vector3 {
    x: -1_000,
    y: 0,
    z: 0,
};
pub(super) const INSTALLATION_YAW: Angle = Angle::from_units(64);
pub(super) const INSTALLATION_ENTRY_HALF_WIDTH: u16 = 640;
pub(super) const INSTALLATION_ENTRY_HALF_DEPTH: u16 = 640;

pub(super) const INTERIOR_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -29,
    z: 381,
};
pub(super) const KICK_GUNNER_INITIAL_POSITION: Vector3 = Vector3 {
    x: 1_280,
    y: 100,
    z: -1_280,
};
pub(super) const KICK_GUNNER_DURABILITY: u8 = 70;
pub(super) const KICK_GUNNER_INITIAL_WAIT_RETAIL_FRAMES: u16 = 236;
pub(super) const KICK_GUNNER_SUBMERGED_Y: i16 = -480;
pub(super) const KICK_GUNNER_ACTION_RETAIL_FRAMES: u8 = 6;
pub(super) const KICK_GUNNER_ANIMATION_FRAME_COUNT: u8 = 12;
pub(super) const KICK_GUNNER_FLOOR_DESCENT_STEP: i16 = -100;
pub(super) const KICK_GUNNER_FLOOR_DESCENT_ACTION_COUNT: u8 = 2;
pub(super) const KICK_GUNNER_LONG_DIVE_SPEED: u8 = 35;
pub(super) const KICK_GUNNER_ATTACK_SPEED: u8 = 25;
pub(super) const KICK_GUNNER_ATTACK_COUNT: u8 = 5;
pub(super) const KICK_GUNNER_REST_AFTER_DIVE_ACTIONS: u8 = 11;
pub(super) const KICK_GUNNER_ATTACK_PAUSE_ACTIONS: u8 = 4;
pub(super) const KICK_GUNNER_POST_SPAWN_WAIT_ACTIONS: u8 = 2;
pub(super) const KICK_GUNNER_BETWEEN_ROUTE_WAIT_RETAIL_FRAMES: u16 = 240;
pub(super) const KICK_GUNNER_CORNER_RANDOM_MASK: u8 = 3;
pub(super) const KICK_GUNNER_DIRECTION_RANDOM_MASK: u8 = 1;
pub(super) const KICK_GUNNER_ROUTES_PER_CORNER: usize = 2;
pub(super) const KICK_GUNNER_YAW_CHASE_SHIFT: u32 = 2;
pub(super) const KICK_GUNNER_RETREAT_CHASE_SHIFT: u32 = 3;
pub(super) const KICK_GUNNER_RETREAT_MINIMUM_STEP: i16 = 8;
pub(super) const KICK_GUNNER_MOUNT_OFFSET: u8 = 50;
pub(super) const KICK_GUNNER_MOUNT_DURABILITY: u8 = 10;
pub(super) const KICK_GUNNER_MOUNT_ATTACK_POWER: u8 = 10;
/// The mount performs its first settling action when it is created; these are
/// the remaining actions before its one retail shot.
pub(super) const KICK_GUNNER_MOUNT_SETTLE_ACTIONS: u8 = 9;
pub(super) const KICK_GUNNER_PROJECTILE_SPEED: u8 = 20;
pub(super) const KICK_GUNNER_PROJECTILE_POSITION_SCALE: i16 = 4;
pub(super) const KICK_GUNNER_PROJECTILE_DURABILITY: u8 = 120;
pub(super) const KICK_GUNNER_PROJECTILE_ATTACK_POWER: u8 = 4;
pub(super) const KICK_GUNNER_PROJECTILE_LIFETIME_RETAIL_FRAMES: u16 = 220;
pub(super) const KICK_GUNNER_SURFACE_Y: i16 = -100;
pub(super) const KICK_GUNNER_RESTING_Y: i16 = 100;
pub(super) const KICK_GUNNER_CORNER_POSITIONS: [Vector3; 4] = [
    KICK_GUNNER_INITIAL_POSITION,
    Vector3 {
        x: 1_280,
        y: KICK_GUNNER_RESTING_Y,
        z: 1_280,
    },
    Vector3 {
        x: -1_280,
        y: KICK_GUNNER_RESTING_Y,
        z: 1_280,
    },
    Vector3 {
        x: -1_280,
        y: KICK_GUNNER_RESTING_Y,
        z: -1_280,
    },
];
pub(super) const KICK_GUNNER_ROUTE_YAWS: [Angle; 8] = [
    Angle::ZERO,
    Angle::from_units(64),
    Angle::HALF_TURN,
    Angle::from_units(64),
    Angle::from_units(192),
    Angle::HALF_TURN,
    Angle::from_units(192),
    Angle::ZERO,
];
pub(super) const KICK_GUNNER_RETREAT_CORNERS: [usize; 8] = [1, 3, 0, 2, 1, 3, 0, 2];
pub(super) const KICK_GUNNER_LONG_DIVE_VERTICAL_STEPS: [i16; 20] = [
    -200, -81, -64, -49, -36, -25, -16, -9, -4, -1, 1, 4, 9, 16, 25, 36, 49, 64, 81, 200,
];
pub(super) const KICK_GUNNER_ATTACK_VERTICAL_STEPS: [i16; 14] =
    [-98, -72, -50, -32, -18, -8, -2, 2, 8, 18, 32, 50, 72, 98];
pub(super) const KICK_GUNNER_SURFACE_BOB_STEPS: [i16; 5] = [8, 12, 4, -4, -20];
pub(super) const KICK_GUNNER_LONG_DIVE_ANIMATION_FRAMES: [u8; 20] = [
    7, 8, 9, 9, 9, 9, 9, 9, 10, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1,
];
pub(super) const KICK_GUNNER_ATTACK_ANIMATION_FRAMES: [u8; 14] =
    [6, 7, 8, 9, 10, 8, 7, 6, 5, 4, 3, 2, 1, 0];
pub(super) const INTERIOR_DOORWAY_POSITION: Vector3 = Vector3 {
    x: 2_560,
    y: 0,
    z: 0,
};
pub(super) const INTERIOR_DOORWAY_HALF_WIDTH: u16 = 512;
pub(super) const INTERIOR_DOORWAY_HALF_DEPTH: u16 = 512;

pub(super) const CORE_ROOM_START_POSITION: Vector3 = Vector3 {
    x: 2_065,
    y: -31,
    z: 25,
};
pub(super) const CORE_POSITION: Vector3 = Vector3 {
    x: 4_352,
    y: -160,
    z: 0,
};
pub(super) const CORE_DEFENDER_POSITIONS: [Vector3; 2] = [
    Vector3 {
        x: 5_152,
        y: -160,
        z: -1_056,
    },
    Vector3 {
        x: 5_152,
        y: -160,
        z: 1_056,
    },
];
pub(super) const CORE_DEFENDER_YAWS: [Angle; 2] = [Angle::ZERO, Angle::from_units(64)];
pub(super) const CORE_DEFENDER_DURABILITY: u8 = 100;
pub(super) const CORE_DEFENDER_HEAD_HEIGHT: i16 = 159;
pub(super) const CORE_DURABILITY: u8 = 125;
pub(super) const CORE_INNER_PHASE_DURABILITY: u8 = 105;
pub(super) const CORE_DESTROYED_DURABILITY: u8 = 75;
pub(super) const CORE_INNER_MATERIAL: MaterialSetId = MaterialSetId::from_catalog_token(0x82FE);
pub(super) const CORE_SHIELD_OPENING_RETAIL_FRAMES: u16 = 332;
pub(super) const CORE_RETIRE_RETAIL_FRAMES: u16 = 764;
pub(super) const RETURN_TO_MAP_RETAIL_FRAMES: u16 = 1_336;
pub(super) const RETURN_FLIGHT_RETAIL_FRAMES: u16 =
    RETURN_TO_MAP_RETAIL_FRAMES - CORE_RETIRE_RETAIL_FRAMES;
pub(super) const CORE_EMITTER_WAIT_RETAIL_FRAMES: u8 = 20;
pub(super) const CORE_EMITTER_OFFSETS: [Vector3; 4] = [
    Vector3 {
        x: -280,
        y: -120,
        z: 0,
    },
    Vector3 {
        x: 279,
        y: -120,
        z: 0,
    },
    Vector3 {
        x: 0,
        y: -120,
        z: -280,
    },
    Vector3 {
        x: 0,
        y: -120,
        z: 279,
    },
];
pub(super) const CORE_PROJECTILE_SPEED: u8 = 20;
pub(super) const CORE_PROJECTILE_POSITION_SCALE: i16 = 4;
pub(super) const CORE_PROJECTILE_LIFETIME_RETAIL_FRAMES: u16 = 120;
pub(super) const ENEMY_ATTACK_POWER: u8 = 4;

/// Both the Hard and Expert retail phase loaders install this same pair. The
/// higher difficulty changes encounter pressure elsewhere, not this layout.
pub(super) const fn core_defender_count() -> usize {
    CORE_DEFENDER_POSITIONS.len()
}
