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
pub(super) const INTERIOR_DOORWAY_POSITION: Vector3 = Vector3 {
    x: 2_560,
    y: 0,
    z: 0,
};
pub(super) const INTERIOR_DOORWAY_HALF_WIDTH: u16 = 512;
pub(super) const INTERIOR_DOORWAY_HALF_DEPTH: u16 = 512;

/// Six-retail-frame samples from the guardian's first verified dive and leap.
/// The sequence preserves the characteristic underwater parabola and forward
/// advance without retaining a source path cursor.
pub(super) const KICK_GUNNER_MOTION: [Vector3; 42] = [
    Vector3 {
        x: 1_280,
        y: -100,
        z: -1_280,
    },
    Vector3 {
        x: 1_280,
        y: -300,
        z: -1_247,
    },
    Vector3 {
        x: 1_280,
        y: -381,
        z: -1_214,
    },
    Vector3 {
        x: 1_280,
        y: -445,
        z: -1_181,
    },
    Vector3 {
        x: 1_280,
        y: -494,
        z: -1_148,
    },
    Vector3 {
        x: 1_280,
        y: -530,
        z: -1_115,
    },
    Vector3 {
        x: 1_280,
        y: -555,
        z: -1_082,
    },
    Vector3 {
        x: 1_280,
        y: -571,
        z: -1_049,
    },
    Vector3 {
        x: 1_280,
        y: -580,
        z: -1_016,
    },
    Vector3 {
        x: 1_280,
        y: -584,
        z: -983,
    },
    Vector3 {
        x: 1_280,
        y: -585,
        z: -950,
    },
    Vector3 {
        x: 1_280,
        y: -584,
        z: -917,
    },
    Vector3 {
        x: 1_280,
        y: -580,
        z: -884,
    },
    Vector3 {
        x: 1_280,
        y: -571,
        z: -851,
    },
    Vector3 {
        x: 1_280,
        y: -555,
        z: -818,
    },
    Vector3 {
        x: 1_280,
        y: -530,
        z: -785,
    },
    Vector3 {
        x: 1_280,
        y: -494,
        z: -752,
    },
    Vector3 {
        x: 1_280,
        y: -445,
        z: -719,
    },
    Vector3 {
        x: 1_280,
        y: -381,
        z: -686,
    },
    Vector3 {
        x: 1_280,
        y: -300,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -100,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -92,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -80,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -76,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -80,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -100,
        z: -653,
    },
    Vector3 {
        x: 1_280,
        y: -198,
        z: -630,
    },
    Vector3 {
        x: 1_280,
        y: -270,
        z: -607,
    },
    Vector3 {
        x: 1_280,
        y: -320,
        z: -584,
    },
    Vector3 {
        x: 1_280,
        y: -352,
        z: -561,
    },
    Vector3 {
        x: 1_280,
        y: -370,
        z: -538,
    },
    Vector3 {
        x: 1_280,
        y: -378,
        z: -515,
    },
    Vector3 {
        x: 1_280,
        y: -380,
        z: -492,
    },
    Vector3 {
        x: 1_280,
        y: -378,
        z: -469,
    },
    Vector3 {
        x: 1_280,
        y: -370,
        z: -446,
    },
    Vector3 {
        x: 1_280,
        y: -352,
        z: -423,
    },
    Vector3 {
        x: 1_280,
        y: -320,
        z: -400,
    },
    Vector3 {
        x: 1_280,
        y: -270,
        z: -377,
    },
    Vector3 {
        x: 1_280,
        y: -198,
        z: -354,
    },
    Vector3 {
        x: 1_280,
        y: -100,
        z: -354,
    },
    Vector3 {
        x: 1_280,
        y: -80,
        z: -354,
    },
    Vector3 {
        x: 1_280,
        y: -76,
        z: -354,
    },
];
pub(super) const KICK_GUNNER_MOTION_SAMPLE_RETAIL_FRAMES: u16 = 6;

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
