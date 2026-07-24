//! Generated typed mechanics for Meteor's Queen Dragoon encounter.
//!
//! Source: `meteor_queen_dragoon.trace`.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_meteor_queen_dragoon.py [--check]`.

use super::{Angle, ShapeId, Vector3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueenComponentRole {
    LeadingLeft,
    LeadingRight,
    TrailingLeft,
    TrailingRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct QueenComponentPlacement {
    pub role: QueenComponentRole,
    pub shape: ShapeId,
    pub offset: Vector3,
    pub yaw_offset: i8,
}

pub(super) const MAXIMUM_DURABILITY: u8 = 80;
pub(super) const DEFEAT_TO_EXPLOSION_RETAIL_FRAMES: u16 = 20;
pub(super) const BODY_SHAPE: ShapeId = ShapeId::from_catalog_index(566);
pub(super) const BODY_EXPLOSION_SHAPE: ShapeId = ShapeId::from_catalog_index(11);
pub(super) const COMPONENT_BURST_SHAPE: ShapeId = ShapeId::from_catalog_index(9);
pub(super) const COMPONENT_DEBRIS_SHAPE: ShapeId = ShapeId::from_catalog_index(12);
pub(super) const DROPPED_SWITCH_SHAPE: ShapeId = ShapeId::from_catalog_index(464);
pub(super) const PRESSED_SWITCH_SHAPE: ShapeId = ShapeId::from_catalog_index(465);
pub(super) const INITIAL_BODY_POSITION: Vector3 = Vector3 { x: 2_040, y: -350, z: 1_942 };
pub(super) const INITIAL_BODY_YAW: Angle = Angle::from_units(157);
pub(super) const BODY_SPEED: u8 = 10;
pub(super) const BODY_VELOCITY: Vector3 = Vector3 { x: 5, y: 0, z: -6 };
pub(super) const MOVEMENT_CADENCE_RETAIL_FRAMES: [u8; 3] = [7, 7, 8];

pub(super) const COMPONENTS: [QueenComponentPlacement; 4] = [
    QueenComponentPlacement {
        role: QueenComponentRole::LeadingLeft,
        shape: ShapeId::from_catalog_index(567),
        offset: Vector3 { x: -55, y: 0, z: -153 },
        yaw_offset: 15,
    },
    QueenComponentPlacement {
        role: QueenComponentRole::LeadingRight,
        shape: ShapeId::from_catalog_index(568),
        offset: Vector3 { x: 158, y: 0, z: 30 },
        yaw_offset: -15,
    },
    QueenComponentPlacement {
        role: QueenComponentRole::TrailingLeft,
        shape: ShapeId::from_catalog_index(567),
        offset: Vector3 { x: -118, y: 0, z: -49 },
        yaw_offset: -10,
    },
    QueenComponentPlacement {
        role: QueenComponentRole::TrailingRight,
        shape: ShapeId::from_catalog_index(568),
        offset: Vector3 { x: 63, y: 0, z: 108 },
        yaw_offset: 10,
    },
];
