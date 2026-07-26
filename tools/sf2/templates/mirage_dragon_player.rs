/// Semantic phases of the off-screen player staging and the retail control
/// handoff. Hidden source-machine scratch poses are deliberately not retained:
/// the craft is placed directly at the next meaningful staging point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlayerCinematicPhase {
    MapDeparture,
    FormationHold,
    CameraApproach,
    ControlHandoff,
}

pub(super) const PLAYER_MAP_DEPARTURE_POSITION: Vector3 = Vector3 {
    x: 400,
    y: -150,
    z: 0,
};
pub(super) const PLAYER_FORMATION_POSITION: Vector3 = Vector3 {
    x: 86,
    y: 0,
    z: 137,
};
pub(super) const PLAYER_CONTROL_HANDOFF_POSITION: Vector3 = Vector3 {
    x: 10_652,
    y: -2_881,
    z: 2_498,
};

pub(super) const PLAYER_FORMATION_START_RETAIL_FRAME: u16 = 44;
pub(super) const PLAYER_CAMERA_APPROACH_START_RETAIL_FRAME: u16 = 260;
pub(super) const PLAYER_CONTROL_HANDOFF_START_RETAIL_FRAME: u16 = 340;
pub(super) const PLAYER_FORMATION_YAW: Angle = Angle::from_units(64);
pub(super) const PLAYER_CONTROL_HANDOFF_BANK: Angle = Angle::from_units(10);
pub(super) const PLAYER_CONTROL_HANDOFF_PITCH_TARGET: i16 = 10_240;
pub(super) const PLAYER_CONTROL_HANDOFF_YAW_IMPULSE: i16 = 544;
pub(super) const PLAYER_CONTROL_HANDOFF_BANK_RECOVERY: i8 = 10;
pub(super) const PLAYER_CONTROL_HANDOFF_BANK_TRIM: i8 = 4;
pub(super) const PLAYER_CONTROL_HANDOFF_FAST_ACCELERATION: u8 = 2;
pub(super) const PLAYER_CONTROL_HANDOFF_FAST_SPEED_LIMIT: u8 = 8;
const PLAYER_CONTROL_HANDOFF_CADENCE: [u8; 16] = [1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1];

pub(super) fn player_cinematic_phase(retail_frame: u16) -> PlayerCinematicPhase {
    if retail_frame < PLAYER_FORMATION_START_RETAIL_FRAME {
        PlayerCinematicPhase::MapDeparture
    } else if retail_frame < PLAYER_CAMERA_APPROACH_START_RETAIL_FRAME {
        PlayerCinematicPhase::FormationHold
    } else if retail_frame < PLAYER_CONTROL_HANDOFF_START_RETAIL_FRAME {
        PlayerCinematicPhase::CameraApproach
    } else {
        PlayerCinematicPhase::ControlHandoff
    }
}

/// The retail controller reduces each signed bank recovery term to three
/// quarters by summing its arithmetic half and quarter.
pub(super) fn decay_player_bank_recovery(value: i8) -> i8 {
    let half = value >> 1;
    half.wrapping_add(half >> 1)
}

pub(super) fn decay_player_bank_trim(value: i8) -> i8 {
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => value - 1,
        std::cmp::Ordering::Less => value + 1,
        std::cmp::Ordering::Equal => 0,
    }
}

pub(super) fn player_control_handoff_updates(retail_frame: u16) -> Option<u8> {
    let offset = retail_frame.checked_sub(PLAYER_CONTROL_HANDOFF_START_RETAIL_FRAME)?;
    if retail_frame > PLAYER_NEUTRAL_START_RETAIL_FRAME || offset % PLAYER_RETAIL_FRAME_STEP != 0 {
        return None;
    }
    PLAYER_CONTROL_HANDOFF_CADENCE
        .get(usize::from(offset / PLAYER_RETAIL_FRAME_STEP))
        .copied()
}
