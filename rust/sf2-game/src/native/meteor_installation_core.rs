//! Generated typed mechanics for Meteor's installation core.
//!
//! Source: `meteor_installation_core.trace`.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_meteor_installation_core.py [--check]`.

use super::{ShapeId, Vector3};

pub(super) const PARENT_SHAPE: ShapeId = ShapeId::from_catalog_index(427);
pub(super) const CORE_SHAPE: ShapeId = ShapeId::from_catalog_index(428);
pub(super) const PARENT_POSITION: Vector3 = Vector3 { x: 1_536, y: 0, z: 7_936 };
pub(super) const CORE_POSITION: Vector3 = Vector3 { x: 1_536, y: -152, z: 7_936 };
pub(super) const MAXIMUM_DURABILITY: u8 = 125;
pub(super) const TRIGGER_PARTIAL_RETAIL_FRAME: u16 = 167;
pub(super) const TRIGGER_ARMED_RETAIL_FRAME: u16 = 475;
pub(super) const ACTIVE_RETAIL_FRAME: u16 = 483;
pub(super) const DEFEAT_TO_OBJECTIVE_RETAIL_FRAMES: u16 = 5;
pub(super) const DEFEAT_TO_PARENT_FINAL_RETAIL_FRAMES: u16 = 20;
