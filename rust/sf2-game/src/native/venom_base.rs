//! Retail-derived semantic layout and timing for Venom's planetary base.
//!
//! Oracle traces expose source object records at the verification boundary.
//! The shipping mission keeps only decoded world poses, durability, spatial
//! thresholds, and presentation timing in ordinary Rust values.

use super::super::object::{Angle, Vector3};

pub(super) const SURFACE_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -300,
    z: -3_500,
};
pub(super) const SURFACE_START_SPEED: u8 = 30;
pub(super) const SURFACE_SWITCH_POSITIONS: [Vector3; 2] = [
    Vector3 {
        x: -3_000,
        y: -160,
        z: -2_800,
    },
    Vector3 {
        x: 0,
        y: 0,
        z: 3_200,
    },
];
pub(super) const SURFACE_SWITCH_YAW: Angle = Angle::ZERO;
pub(super) const INSTALLATION_POSITION: Vector3 = Vector3 {
    x: 1_500,
    y: 0,
    z: 2_000,
};
pub(super) const INSTALLATION_YAW: Angle = Angle::from_units(32);
pub(super) const INSTALLATION_ENTRY_HALF_WIDTH: u16 = 700;
pub(super) const INSTALLATION_ENTRY_HALF_DEPTH: u16 = 700;

pub(super) const INTERIOR_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -120,
    z: 33,
};
pub(super) const ACCESS_SWITCH_POSITION: Vector3 = Vector3 {
    x: 768,
    y: -64,
    z: 1_280,
};
pub(super) const ACCESS_DOOR_POSITION: Vector3 = Vector3 {
    x: 0,
    y: 0,
    z: 2_188,
};
pub(super) const ACCESS_DOOR_TRANSIT_Z: i16 = 2_188;

pub(super) const ARMORED_ROOM_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -120,
    z: 2_700,
};
pub(super) const REACTOR_DOOR_POSITION: Vector3 = Vector3 {
    x: 0,
    y: 0,
    z: 5_260,
};
pub(super) const REACTOR_DOOR_OPENING_Z: i16 = 5_000;
pub(super) const REACTOR_ROOM_TRANSIT_Z: i16 = 6_400;
pub(super) const KNIGHT_POSITION: Vector3 = Vector3 {
    x: 0,
    y: 0,
    z: 6_004,
};
pub(super) const KNIGHT_DURABILITY: u8 = 100;
pub(super) const KNIGHT_SIDE_VULNERABILITY: u16 = 1_200;
pub(super) const KNIGHT_REAR_VULNERABILITY: i16 = 400;

pub(super) const REACTOR_ROOM_START_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -120,
    z: 6_757,
};
pub(super) const REACTOR_PARENT_POSITION: Vector3 = Vector3 {
    x: 0,
    y: 0,
    z: 7_168,
};
pub(super) const REACTOR_CORE_POSITION: Vector3 = Vector3 {
    x: 0,
    y: -152,
    z: 7_168,
};
pub(super) const REACTOR_TRIGGER_HALF_WIDTH: u16 = 640;
pub(super) const REACTOR_TRIGGER_MINIMUM_Z: i16 = 6_600;
pub(super) const REACTOR_MAXIMUM_DURABILITY: u8 = 125;
pub(super) const REACTOR_DESTROYED_DURABILITY: u8 = 1;

// Venom uses the same retail installation-reactor sequence recovered at
// Meteor: partial wake-up, armed weak points, then a vulnerable central core.
pub(super) const REACTOR_TRIGGER_PARTIAL_RETAIL_FRAME: u16 = 167;
pub(super) const REACTOR_TRIGGER_ARMED_RETAIL_FRAME: u16 = 475;
pub(super) const REACTOR_ACTIVE_RETAIL_FRAME: u16 = 483;
pub(super) const REACTOR_DEFEAT_TO_OBJECTIVE_RETAIL_FRAMES: u16 = 5;
pub(super) const REACTOR_DEFEAT_TO_PARENT_FINAL_RETAIL_FRAMES: u16 = 20;
pub(super) const RETURN_TO_MAP_RETAIL_FRAMES: u16 = 1_444;
pub(super) const RETURN_FLIGHT_RETAIL_FRAMES: u16 =
    RETURN_TO_MAP_RETAIL_FRAMES - REACTOR_DEFEAT_TO_PARENT_FINAL_RETAIL_FRAMES;

pub(super) const ENEMY_ATTACK_POWER: u8 = 10;
