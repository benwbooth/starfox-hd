//! Launch-transition craft presentation and camera alignment.
use super::render::MaterialSetId;
use super::{Angle, Object, ShapeId};

const PILOT_SELECTOR_MASK: u8 = 0x0F;
const CAMERA_TURN_SHIFT: u32 = 3;
const LOW_SHIELD_LIMIT: u8 = 13;
const LOW_SHIELD_DEPTH: u16 = 3;
const DEPTH_FLASH_CLOCK_MASK: u8 = 0x05;

/// Decoded entries of the six-pilot appearance table ($06:8135).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PilotCraftAppearance {
    pub shape: ShapeId,
    pub material: MaterialSetId,
}

/// The low nibble is clamped, not reduced modulo the number of pilots.
pub fn select_appearance(actor: &mut Object, pilot: u8, appearances: &[PilotCraftAppearance; 6]) {
    let index = usize::from(pilot & PILOT_SELECTOR_MASK).min(appearances.len() - 1);
    actor.base.shape = appearances[index].shape;
    actor.extension.material_set = Some(appearances[index].material);
}

/// Source $07:F3B6 uses the camera's coarse heading, negated, and rounds
/// negative eighth-turn deltas down. Small positive deltas do not move.
pub fn align_camera_heading(actor: &mut Object, camera_yaw: Angle) {
    let delta = camera_yaw
        .units()
        .wrapping_neg()
        .wrapping_sub(actor.base.yaw.units()) as i8;
    actor.base.yaw = actor.base.yaw.wrapping_add(delta >> CAMERA_TURN_SHIFT);
}

/// Source $07:F6D6 reads the published shield, not the selected actor's
/// health. The callback saves/restores its phase byte around this service;
/// the complete depth word is still replaced on every invocation.
pub fn update_low_shield_visual(actor: &mut Object, shield: u8, clock: u8) {
    let low = shield < LOW_SHIELD_LIMIT;
    actor.extension.path_state.motion_phase =
        (actor.extension.path_state.motion_phase & 0xFF00) | u16::from(low);
    actor.extension.depth_offset = if low && clock & DEPTH_FLASH_CLOCK_MASK == 0 {
        LOW_SHIELD_DEPTH
    } else {
        0
    };
}
