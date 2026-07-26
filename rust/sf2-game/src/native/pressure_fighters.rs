//! Generated typed entry and live cadence for the retail recurring-attacker encounter.
//!
//! Combat source: `pressure_fighters.trace`.
//! Neutral-flight source: `pressure_fighter_neutral.trace`.
//! Live player and camera poses remain oracle evidence only; shipping
//! Rust advances typed state using the statically recovered rules below.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_pressure_fighters.py [--check]`.

use super::{
    mission_camera_keyframe, mission_player_keyframe, Angle, MissionCameraKeyframe,
    MissionPlayerKeyframe, Vector3,
};

#[cfg(test)]
pub(super) const ACCEPTED_RETURN_RETAIL_FRAME: u16 = 2020;
pub(super) const DEFEAT_TO_RETURN_RETAIL_FRAMES: u16 = 212;
pub(super) const DEFEAT_TO_MAP_READY_RETAIL_FRAMES: u16 = 214;
#[cfg(test)]
pub(super) const ACCEPTED_ALL_DEFEATED_RETAIL_FRAME: u16 = 1808;

pub(super) const ENTRY_LAST_RETAIL_FRAME: u16 = 312;
pub(super) const LIVE_FIRST_RETAIL_FRAME: u16 = 316;
pub(super) const LIVE_LAST_RETAIL_FRAME: u16 = 2016;
#[cfg(test)]
pub(super) const NEUTRAL_FLIGHT_LAST_RETAIL_FRAME: u16 = 1764;
const RETAIL_FRAME_STEP: u16 = 4;

pub(super) const PLAYER_HANDOFF_POSITION: Vector3 = Vector3 {
    x: 3_980,
    y: 3_177,
    z: 1_328,
};
pub(super) const PLAYER_HANDOFF_PITCH: Angle = Angle::from_units(0);
pub(super) const PLAYER_HANDOFF_YAW: Angle = Angle::from_units(64);
pub(super) const PLAYER_HANDOFF_BANK: Angle = Angle::from_units(10);
pub(super) const PLAYER_HANDOFF_SPEED: u8 = 0;
pub(super) const PLAYER_HANDOFF_BANK_RECOVERY: i8 = 10;
pub(super) const PLAYER_NEUTRAL_TARGET_SPEED: u8 = 23;
pub(super) const PLAYER_FAST_ACCELERATION: u8 = 2;
pub(super) const PLAYER_FAST_SPEED_LIMIT: u8 = 8;

pub(super) const CAMERA_HANDOFF_POSITION: Vector3 = Vector3 {
    x: 4_129,
    y: 3_197,
    z: 1_319,
};
pub(super) const CAMERA_FOLLOW_INITIAL_REAR_DISTANCE: i16 = -240;
pub(super) const CAMERA_FOLLOW_REAR_DISTANCE_TARGET: i16 = 0;
pub(super) const CAMERA_FOLLOW_REAR_DISTANCE_STEP: i16 = 30;
pub(super) const CAMERA_FOLLOW_VERTICAL_OFFSET: i16 = -20;
pub(super) const CAMERA_FOLLOW_VIEW_PITCH_SUBUNITS: u16 = 0;
pub(super) const CAMERA_FOLLOW_VIEW_YAW_SUBUNITS: u16 = (-16_384i16) as u16;
pub(super) const CAMERA_FOLLOW_VERTICAL_POSITION_SCALE: i16 = 2;
pub(super) const CAMERA_FOLLOW_POSITION_SCALE_SHIFT: u32 = 1;
pub(super) const CAMERA_CONTINUITY_TRANSLATION_DIVISOR: i16 = 16;
pub(super) const CAMERA_ORIENTATION_COARSE_SHIFT: u32 = 8;

pub(super) const ENTRY_CAMERA_KEYFRAMES: [MissionCameraKeyframe; 79] = [
    mission_camera_keyframe(0, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(4, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(8, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(12, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(16, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(20, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(24, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(28, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(32, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(36, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(40, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(44, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(48, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(52, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(56, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(60, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(64, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(68, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(72, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(76, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(80, -1_514, -4_447, 823, 208, 80, 0),
    mission_camera_keyframe(84, -1_507, -4_400, 831, 16, 16, 0),
    mission_camera_keyframe(88, -1_500, -4_352, 839, 64, 192, 0),
    mission_camera_keyframe(92, -1_493, -4_305, 847, 128, 128, 0),
    mission_camera_keyframe(96, -1_486, -4_258, 855, 208, 80, 0),
    mission_camera_keyframe(100, -1_479, -4_211, 863, 32, 32, 0),
    mission_camera_keyframe(104, -1_472, -4_164, 871, 96, 16, 0),
    mission_camera_keyframe(108, -1_465, -4_117, 879, 192, 16, 0),
    mission_camera_keyframe(112, -1_459, -4_070, 887, 48, 32, 0),
    mission_camera_keyframe(116, -1_452, -4_023, 895, 144, 64, 0),
    mission_camera_keyframe(120, -1_445, -3_976, 903, 48, 96, 0),
    mission_camera_keyframe(124, -1_445, -3_976, 903, 48, 96, 0),
    mission_camera_keyframe(128, -1_431, -3_923, 911, 176, 224, 0),
    mission_camera_keyframe(132, -1_409, -3_862, 919, 192, 192, 0),
    mission_camera_keyframe(136, -1_380, -3_795, 927, 48, 240, 0),
    mission_camera_keyframe(140, -1_343, -3_721, 935, 240, 144, 0),
    mission_camera_keyframe(144, -1_299, -3_641, 943, 16, 192, 0),
    mission_camera_keyframe(148, -1_248, -3_553, 951, 16, 128, 0),
    mission_camera_keyframe(152, -1_189, -3_459, 959, 48, 0, 0),
    mission_camera_keyframe(156, -1_050, -3_251, 975, 112, 0, 0),
    mission_camera_keyframe(160, -969, -3_137, 983, 16, 80, 0),
    mission_camera_keyframe(164, -880, -3_016, 991, 112, 240, 0),
    mission_camera_keyframe(168, -785, -2_889, 999, 224, 64, 0),
    mission_camera_keyframe(172, -682, -2_754, 1_007, 208, 64, 0),
    mission_camera_keyframe(176, -571, -2_613, 1_015, 128, 144, 0),
    mission_camera_keyframe(180, -453, -2_466, 1_023, 80, 64, 0),
    mission_camera_keyframe(184, -328, -2_311, 1_031, 240, 32, 0),
    mission_camera_keyframe(188, -55, -1_982, 1_047, 0, 48, 0),
    mission_camera_keyframe(192, 93, -1_807, 1_055, 192, 128, 0),
    mission_camera_keyframe(196, 248, -1_626, 1_063, 80, 112, 0),
    mission_camera_keyframe(200, 411, -1_438, 1_071, 208, 48, 0),
    mission_camera_keyframe(204, 580, -1_243, 1_079, 80, 176, 0),
    mission_camera_keyframe(208, 758, -1_042, 1_087, 160, 208, 0),
    mission_camera_keyframe(212, 942, -834, 1_095, 240, 0, 0),
    mission_camera_keyframe(216, 1_334, -397, 1_111, 112, 192, 0),
    mission_camera_keyframe(220, 1_541, -169, 1_119, 160, 128, 0),
    mission_camera_keyframe(224, 1_755, 66, 1_127, 224, 64, 0),
    mission_camera_keyframe(228, 1_977, 308, 1_135, 16, 224, 0),
    mission_camera_keyframe(232, 2_206, 556, 1_143, 48, 80, 0),
    mission_camera_keyframe(236, 2_686, 1_073, 1_159, 128, 64, 0),
    mission_camera_keyframe(240, 2_938, 1_342, 1_167, 160, 176, 0),
    mission_camera_keyframe(244, 3_197, 1_617, 1_175, 87, 42, 0),
    mission_camera_keyframe(248, 3_421, 1_862, 1_183, 60, 75, 0),
    mission_camera_keyframe(252, 3_610, 2_073, 1_191, 203, 40, 0),
    mission_camera_keyframe(256, 3_766, 2_256, 1_199, 160, 210, 0),
    mission_camera_keyframe(260, 3_895, 2_414, 1_207, 111, 84, 0),
    mission_camera_keyframe(264, 4_000, 2_549, 1_215, 254, 183, 0),
    mission_camera_keyframe(268, 4_083, 2_665, 1_223, 32, 4, 0),
    mission_camera_keyframe(272, 4_148, 2_765, 1_231, 177, 62, 0),
    mission_camera_keyframe(276, 4_196, 2_849, 1_239, 152, 107, 0),
    mission_camera_keyframe(280, 4_230, 2_921, 1_247, 193, 141, 0),
    mission_camera_keyframe(284, 4_252, 2_982, 1_255, 28, 167, 0),
    mission_camera_keyframe(288, 4_263, 3_032, 1_263, 158, 187, 0),
    mission_camera_keyframe(292, 4_264, 3_074, 1_271, 62, 202, 0),
    mission_camera_keyframe(296, 4_256, 3_108, 1_279, 245, 214, 0),
    mission_camera_keyframe(300, 4_242, 3_135, 1_287, 189, 223, 0),
    mission_camera_keyframe(304, 4_221, 3_158, 1_295, 146, 230, 0),
    mission_camera_keyframe(308, 4_195, 3_175, 1_303, 112, 235, 0),
    mission_camera_keyframe(312, 4_164, 3_188, 1_311, 86, 239, 0),
];

pub(super) const ENTRY_PLAYER_KEYFRAMES: [MissionPlayerKeyframe; 79] = [
    mission_player_keyframe(0, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(4, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(8, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(12, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(16, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(20, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(24, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(28, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(32, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(36, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(40, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(44, 7_700, -2_881, 2_498, 0, 66, 0, 23),
    mission_player_keyframe(48, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(52, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(56, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(60, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(64, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(68, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(72, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(76, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(80, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(84, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(88, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(92, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(96, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(100, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(104, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(108, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(112, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(116, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(120, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(124, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(128, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(132, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(136, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(140, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(144, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(148, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(152, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(156, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(160, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(164, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(168, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(172, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(176, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(180, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(184, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(188, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(192, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(196, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(200, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(204, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(208, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(212, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(216, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(220, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(224, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(228, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(232, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(236, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(240, 22, 0, 137, 0, 64, 0, 0),
    mission_player_keyframe(244, 0, 0, 0, 0, 0, 0, 0),
    mission_player_keyframe(248, 3_403, 1_842, 1_134, 0, 22, 40, 0),
    mission_player_keyframe(252, 3_571, 2_053, 1_168, 0, 28, 40, 0),
    mission_player_keyframe(256, 3_704, 2_236, 1_197, 0, 34, 40, 0),
    mission_player_keyframe(260, 3_808, 2_394, 1_220, 0, 40, 40, 0),
    mission_player_keyframe(264, 3_888, 2_529, 1_239, 0, 46, 40, 0),
    mission_player_keyframe(268, 3_946, 2_645, 1_253, 0, 52, 40, 0),
    mission_player_keyframe(272, 3_987, 2_745, 1_263, 0, 58, 40, 0),
    mission_player_keyframe(276, 4_018, 2_829, 1_267, 0, 64, 74, 0),
    mission_player_keyframe(280, 4_041, 2_901, 1_272, 0, 64, 104, 0),
    mission_player_keyframe(284, 4_057, 2_962, 1_277, 0, 64, 131, 0),
    mission_player_keyframe(288, 4_066, 3_012, 1_283, 0, 64, 155, 0),
    mission_player_keyframe(292, 4_067, 3_054, 1_289, 0, 64, 176, 0),
    mission_player_keyframe(296, 4_060, 3_088, 1_295, 0, 64, 195, 0),
    mission_player_keyframe(300, 4_050, 3_115, 1_301, 0, 64, 212, 0),
    mission_player_keyframe(304, 4_036, 3_138, 1_308, 0, 64, 227, 0),
    mission_player_keyframe(308, 4_019, 3_155, 1_315, 0, 64, 241, 0),
    mission_player_keyframe(312, 4_001, 3_168, 1_321, 0, 64, 254, 0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LiveFlightCadence {
    pub player_updates: u8,
    pub player_control_only: bool,
    pub camera_updates: u8,
    pub camera_anchor_only: bool,
}

const PLAYER_SKIPPED_UPDATE_RETAIL_FRAMES: [u16; 54] = [
    328, 352, 388, 424, 456, 488, 516, 548, 576, 604, 628, 676, 716, 752, 788, 824, 868, 900, 932,
    972, 1008, 1040, 1076, 1112, 1156, 1184, 1204, 1224, 1248, 1268, 1292, 1312, 1328, 1348, 1376,
    1404, 1436, 1476, 1508, 1544, 1592, 1636, 1680, 1704, 1732, 1756, 1772, 1792, 1820, 1848, 1880,
    1920, 1960, 2000,
];
const PLAYER_CONTROL_ONLY_RETAIL_FRAMES: [u16; 2] = [932, 1224];
const CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES: [u16; 54] = [
    324, 352, 384, 420, 456, 488, 516, 548, 576, 600, 628, 672, 712, 748, 788, 824, 864, 896, 932,
    968, 1004, 1040, 1072, 1112, 1152, 1180, 1204, 1224, 1244, 1268, 1292, 1312, 1328, 1348, 1372,
    1400, 1436, 1476, 1508, 1544, 1592, 1632, 1680, 1704, 1732, 1756, 1772, 1792, 1816, 1844, 1876,
    1920, 1960, 2000,
];
const CAMERA_ANCHOR_ONLY_RETAIL_FRAMES: [u16; 4] = [420, 748, 1152, 1632];

pub(super) fn live_flight_cadence(retail_frame: u16) -> Option<LiveFlightCadence> {
    let offset = retail_frame.checked_sub(LIVE_FIRST_RETAIL_FRAME)?;
    if retail_frame > LIVE_LAST_RETAIL_FRAME || offset % RETAIL_FRAME_STEP != 0 {
        return None;
    }
    Some(LiveFlightCadence {
        player_updates: u8::from(!PLAYER_SKIPPED_UPDATE_RETAIL_FRAMES.contains(&retail_frame)),
        player_control_only: PLAYER_CONTROL_ONLY_RETAIL_FRAMES.contains(&retail_frame),
        camera_updates: u8::from(!CAMERA_SKIPPED_UPDATE_RETAIL_FRAMES.contains(&retail_frame)),
        camera_anchor_only: CAMERA_ANCHOR_ONLY_RETAIL_FRAMES.contains(&retail_frame),
    })
}

const PLAYER_AMBIENT_BANK_PERIOD: u8 = 30;
const PLAYER_AMBIENT_BANK_WAVE: [i8; 30] = [
    0, 1, 2, 2, 3, 3, 4, 4, 4, 4, 3, 3, 2, 2, 1, 0, -1, -2, -2, -3, -3, -4, -4, -4, -4, -3, -3, -2,
    -2, -1,
];

pub(super) fn advance_player_ambient_bank_phase(phase: u8, updates: u8) -> u8 {
    phase.wrapping_add(updates) % PLAYER_AMBIENT_BANK_PERIOD
}

pub(super) fn player_ambient_bank(phase: u8) -> i8 {
    PLAYER_AMBIENT_BANK_WAVE[usize::from(phase % PLAYER_AMBIENT_BANK_PERIOD)]
}

/// The source controller reduces the scripted bank spring to three
/// quarters by summing its arithmetic half and quarter.
pub(super) fn decay_player_bank_recovery(value: i8) -> i8 {
    let half = value >> 1;
    half.wrapping_add(half >> 1)
}

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
