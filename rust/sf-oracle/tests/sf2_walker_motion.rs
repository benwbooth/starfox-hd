//! ROM-backed proof for the typed native Walker motion profiles.

use std::path::PathBuf;

use sf2_game::Pilot;

const MAXIMUM_ASCENT_IMPULSE_TABLE: usize = 0x3313C;
const INITIAL_ASCENT_IMPULSE_TABLE: usize = 0x33148;
const HELD_ASCENT_STEP_TABLE: usize = 0x33154;
const LAUNCH_COUNTDOWN_TABLE: usize = 0x33160;
const POSE_EXTENSION_STEP_TABLE: usize = 0x3316C;
const PROFILE_ENTRY_BYTES: usize = 2;
const LEFT_TURN_SPRING_TARGET_OPERAND: usize = 0x334EB;
const RIGHT_TURN_SPRING_TARGET_OPERAND: usize = 0x3352A;
const TURN_VELOCITY_TARGET_TABLE: usize = 0x352D7;
const VERTICAL_POSITION_INTEGRATOR: usize = 0x6B2CE;
const FALL_ACCELERATION_ACCUMULATOR: usize = 0x6B319;
const FALL_VELOCITY_SCALER: usize = 0x6B328;
const ASCENT_IMPULSE_SCALER: usize = 0x6B33F;
const SIGNED_ASCENT_HALVER: usize = 0x32FE3;

fn retail_rom() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc")).ok()
}

fn signed_word(rom: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([rom[offset], rom[offset + 1]])
}

fn word(rom: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([rom[offset], rom[offset + 1]])
}

#[test]
fn native_walker_profiles_match_all_six_retail_table_rows() {
    let Some(rom) = retail_rom() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    for (index, pilot) in Pilot::ALL.into_iter().enumerate() {
        let entry_offset = index * PROFILE_ENTRY_BYTES;
        let profile = pilot.walker_motion_profile();
        assert_eq!(
            signed_word(&rom, MAXIMUM_ASCENT_IMPULSE_TABLE + entry_offset),
            profile.maximum_ascent_impulse,
            "maximum ascent impulse for {pilot:?}"
        );
        assert_eq!(
            signed_word(&rom, INITIAL_ASCENT_IMPULSE_TABLE + entry_offset),
            profile.initial_ascent_impulse,
            "initial ascent impulse for {pilot:?}"
        );
        assert_eq!(
            signed_word(&rom, HELD_ASCENT_STEP_TABLE + entry_offset),
            profile.held_ascent_step,
            "held ascent step for {pilot:?}"
        );
        assert_eq!(
            word(&rom, LAUNCH_COUNTDOWN_TABLE + entry_offset),
            u16::from(profile.launch_ticks),
            "launch countdown for {pilot:?}"
        );
        assert_eq!(
            word(&rom, POSE_EXTENSION_STEP_TABLE + entry_offset),
            profile.pose_extension_step,
            "pose extension step for {pilot:?}"
        );
    }
}

#[test]
fn retail_walker_turn_targets_match_the_native_spring_regression() {
    let Some(rom) = retail_rom() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    assert_eq!(signed_word(&rom, LEFT_TURN_SPRING_TARGET_OPERAND), 8_704);
    assert_eq!(signed_word(&rom, RIGHT_TURN_SPRING_TARGET_OPERAND), -8_704);
    assert_eq!(
        &rom[TURN_VELOCITY_TARGET_TABLE..TURN_VELOCITY_TARGET_TABLE + Pilot::ALL.len()],
        &[5; Pilot::ALL.len()]
    );
}

#[test]
fn retail_walker_vertical_pipeline_matches_the_native_regression() {
    let Some(rom) = retail_rom() else {
        eprintln!("skip: no retail SF2 ROM");
        return;
    };

    assert_eq!(
        &rom[VERTICAL_POSITION_INTEGRATOR..VERTICAL_POSITION_INTEGRATOR + 23],
        &[
            0xB5, 0x34, 0x18, 0x69, 0x03, 0x00, 0xC9, 0x06, 0x00, 0xB0, 0x03, 0xA9, 0x03, 0x00,
            0x38, 0xE9, 0x03, 0x00, 0x18, 0x75, 0x0E, 0x95, 0x0E,
        ]
    );
    assert_eq!(
        &rom[FALL_ACCELERATION_ACCUMULATOR..FALL_ACCELERATION_ACCUMULATOR + 8],
        &[0xB5, 0x34, 0x18, 0x6D, 0xAB, 0x1D, 0x95, 0x34]
    );
    assert_eq!(
        &rom[FALL_VELOCITY_SCALER..FALL_VELOCITY_SCALER + 7],
        &[0xB5, 0x34, 0x0A, 0x0A, 0x0A, 0x95, 0x34]
    );
    assert_eq!(
        &rom[ASCENT_IMPULSE_SCALER..ASCENT_IMPULSE_SCALER + 9],
        &[0xBD, 0xC3, 0x1C, 0x0A, 0x0A, 0x0A, 0x9D, 0xC3, 0x1C]
    );
    assert_eq!(
        &rom[SIGNED_ASCENT_HALVER..SIGNED_ASCENT_HALVER + 13],
        &[0xA5, 0x08, 0xC9, 0x00, 0x80, 0x6A, 0x10, 0x03, 0x69, 0x00, 0x00, 0x85, 0x08,]
    );
}
