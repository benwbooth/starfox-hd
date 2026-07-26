pub(super) const CAMERA_TARGET_TRACKING_FIRST_RETAIL_FRAME: u16 = 456;
pub(super) const CAMERA_FOLLOW_INITIAL_VERTICAL_OFFSET: i16 = -20;
pub(super) const CAMERA_FOLLOW_VERTICAL_CHASE_DIVISOR: i16 = 8;
pub(super) const CAMERA_FOLLOW_NEAR_DISTANCE: i16 = 0;
pub(super) const CAMERA_FOLLOW_FAR_DISTANCE: i16 = -240;
pub(super) const CAMERA_FOLLOW_DISTANCE_STEP: i16 = 30;
pub(super) const CAMERA_FOLLOW_NEAR_HOLD_UPDATES: u8 = 10;
pub(super) const CAMERA_FOLLOW_PITCH_SUBUNITS: u16 = 0;
pub(super) const CAMERA_FOLLOW_YAW_SUBUNITS: u16 = (-16_928i16) as u16;
pub(super) const CAMERA_FOLLOW_FIRST_ROLL_SUBUNITS: u16 = (-256i16) as u16;
pub(super) const CAMERA_FOLLOW_ROLL_SUBUNITS: u16 = 0;
pub(super) const CAMERA_FOLLOW_INITIAL_VIEW_YAW_SUBUNITS: u16 = (-16_384i16) as u16;
pub(super) const CAMERA_CONTINUITY_TRANSLATION_DIVISOR: i16 = 16;
pub(super) const CAMERA_CONTINUITY_ORIENTATION_STEP: i8 = 1;
pub(super) const CAMERA_TRACKING_ORBIT_HEIGHT: i16 = 20;
pub(super) const CAMERA_TRACKING_ORBIT_REAR_DISTANCE: i16 = -120;
pub(super) const CAMERA_TRACKING_BEARING_HOLD_UPDATES: u8 = 15;
pub(super) const CAMERA_TRACKING_BEARING_STEP: i8 = 1;
pub(super) const CAMERA_TRACKING_ORIENTATION_DIVISOR: i16 = 2;
pub(super) const MAXIMUM_CAMERA_UPDATES_PER_RETAIL_FRAME: usize = 2;
pub(super) const CAMERA_ORIENTATION_COARSE_SHIFT: u32 = 8;
pub(super) const CAMERA_ORIENTATION_SUBUNITS_PER_COARSE_UNIT: i16 = 256;
pub(super) const CAMERA_TRACKING_PITCH_SCALE_SHIFT: u32 = 1;
pub(super) const CAMERA_FOLLOW_VERTICAL_POSITION_SCALE: i16 = 2;
pub(super) const CAMERA_FOLLOW_POSITION_SCALE_SHIFT: u32 = 1;

const CAMERA_AMBIENT_HEIGHT_PERIOD: u8 = 32;
const CAMERA_AMBIENT_HEIGHT_WAVE: [i8; 32] = [
    1, 0, 1, 0, 0, 1, 0, 0, 0, 0, -1, 0, 0, -1, 0, -1, -1, 0, -1, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 1,
    0, 1,
];

pub(super) fn advance_camera_ambient_height(phase: u8, height: i16) -> (u8, i16) {
    let phase = phase.wrapping_add(1) % CAMERA_AMBIENT_HEIGHT_PERIOD;
    (
        phase,
        height.wrapping_add(i16::from(CAMERA_AMBIENT_HEIGHT_WAVE[usize::from(phase)])),
    )
}

const CAMERA_TRACKING_HEAD_MOVEMENT_LEADS: [(u16, u8); 4] =
    [(704, 1), (720, 0), (728, 1), (732, 0)];

pub(super) fn camera_tracking_head_movement_lead(retail_frame: u16, movement_update: u8) -> u8 {
    u8::from(CAMERA_TRACKING_HEAD_MOVEMENT_LEADS.contains(&(retail_frame, movement_update)))
}
