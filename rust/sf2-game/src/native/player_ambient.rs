//! Ambient player waveforms ($06:F2F7..F365). These advance retained
//! player terms; orientation composition and offset consumers run later.

use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

const MODE_FAMILY_MASK: u8 = 0xF0;
const BANK_HELD_FAMILY: u8 = 0x30;
const BANK_WAVE: [i8; 30] = [
    0, 1, 2, 2, 3, 3, 4, 4, 4, 4, 3, 3, 2, 2, 1, 0, -1, -2, -2, -3, -3, -4, -4, -4, -4, -3, -3, -2,
    -2, -1,
];
const OFFSET_WAVE: [i8; 32] = [
    1, 0, 1, 0, 0, 1, 0, 0, 0, 0, -1, 0, 0, -1, 0, -1, -1, 0, -1, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 1,
    0, 1,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerAmbient {
    /// 6AD6: advance before sampling; out-of-range values reset to zero.
    pub bank_phase: u8,
    /// 6ADB: independent phase, also consumed by Walker animation.
    pub offset_phase: u8,
    /// 6AE2: signed retained term; Walker has a separate chase writer.
    /// This is not actor height or a camera position publication.
    pub retained_offset: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientError {
    World(WorldInputError),
    MissingViewMode,
    MissingAmbient(ObjectId),
    MissingPose(ObjectId),
}

impl From<WorldInputError> for AmbientError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn advance_phase(phase: u8, period: u8) -> u8 {
    let next = phase.wrapping_add(1);
    if next < period {
        next
    } else {
        0
    }
}

/// Called after barrel-roll input and before boost/brake selection. Scripted
/// view mode skips the entire operation before accessing player storage.
/// The special mode family freezes only the bank waveform, not the offset.
pub fn advance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), AmbientError> {
    if world
        .scripted_view_active()
        .ok_or(AmbientError::MissingViewMode)?
    {
        return Ok(());
    }
    let record = world.player_mut(objects, owner)?;
    let mode = record
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode;
    let ambient = record
        .ambient
        .as_mut()
        .ok_or(AmbientError::MissingAmbient(owner))?;
    if mode & MODE_FAMILY_MASK != BANK_HELD_FAMILY {
        ambient.bank_phase = advance_phase(ambient.bank_phase, BANK_WAVE.len() as u8);
        record
            .pose
            .as_mut()
            .ok_or(AmbientError::MissingPose(owner))?
            .ambient_bank = BANK_WAVE[usize::from(ambient.bank_phase)];
    }
    ambient.offset_phase = advance_phase(ambient.offset_phase, OFFSET_WAVE.len() as u8);
    ambient.retained_offset = ambient
        .retained_offset
        .wrapping_add(i16::from(OFFSET_WAVE[usize::from(ambient.offset_phase)]));
    Ok(())
}

#[cfg(test)]
#[path = "player_ambient_tests.rs"]
mod tests;
