//! Shared ROM aim-angle / xzdiffs helpers (`sf_core::aim_angle`).

use sf_core::aim_angle::{
    sf2_atan16, sf2_pitch_to_target, sf2_xz_angle_distance, sf2_yaw_to_target, xanglexabs,
    xanglexy, xanglexy_negated_fine, xzdiffs, xzdiffs_abs_manhattan, yanglexy, yanglexy_nega,
};

#[test]
fn xzdiffs_scaled_vs_manhattan() {
    // Axis-aligned: scaled ≈ 0.875 * max after the ROM formula; Manhattan = sum.
    assert_eq!(xzdiffs_abs_manhattan(300, 400), 700);
    assert_eq!(xzdiffs(300, 400), 506); // ROM scaled-Euclid sample
    assert_ne!(xzdiffs(300, 400), xzdiffs_abs_manhattan(300, 400));
}

#[test]
fn fixed_view_pitch_negates_before_discarding_fraction() {
    const VERTICAL_DELTA: i16 = 730;
    const HORIZONTAL_DELTA_X: i16 = 397;
    const HORIZONTAL_DELTA_Z: i16 = 467;
    const RETAIL_SIGNED_PITCH: i8 = -36;

    let pitch = xanglexy_negated_fine(VERTICAL_DELTA, HORIZONTAL_DELTA_X, HORIZONTAL_DELTA_Z);
    assert_eq!(pitch as i8, RETAIL_SIGNED_PITCH);
    assert_ne!(
        pitch,
        xanglexy(VERTICAL_DELTA, HORIZONTAL_DELTA_X, HORIZONTAL_DELTA_Z).wrapping_neg(),
        "negating after truncation loses the retail fractional carry"
    );
}

#[test]
fn yanglexy_nega_is_wrapping_neg() {
    assert_eq!(yanglexy(0, 1000), 0);
    assert_eq!(yanglexy_nega(0, 1000), 0);
    assert_eq!(yanglexy(1000, 0), 64);
    assert_eq!(yanglexy_nega(1000, 0), 192);
}

#[test]
fn xanglexy_vs_xanglexabs_diverge_off_axis() {
    let dy = 150i16;
    let dx = 300i16;
    let dz = 400i16;
    assert_ne!(xanglexy(dy, dx, dz), xanglexabs(dy, dx, dz));
}

#[test]
fn sf2_table_angle_matches_opening_capital_flight() {
    const FIRST_FACE_DELTA_X: i16 = -5_857;
    const FIRST_FACE_DELTA_Z: i16 = 6_899;
    const LATER_FACE_DELTA_X: i16 = -5_373;
    const LATER_FACE_DELTA_Z: i16 = 6_845;
    const FIRST_FACE_YAW: u8 = 29;
    const LATER_FACE_YAW: u8 = 27;

    assert_eq!(
        sf2_yaw_to_target(FIRST_FACE_DELTA_X, FIRST_FACE_DELTA_Z),
        FIRST_FACE_YAW
    );
    assert_eq!(
        sf2_yaw_to_target(LATER_FACE_DELTA_X, LATER_FACE_DELTA_Z),
        LATER_FACE_YAW
    );
    assert_eq!(sf2_atan16(1, 0), 16_384);
    assert_eq!(sf2_atan16(1, 1), 8_192);
}

#[test]
fn sf2_table_pitch_matches_temporary_capital_weapon_aim() {
    const DELTA_X: i16 = 16_033;
    const DELTA_Y: i16 = -10_646;
    const DELTA_Z: i16 = -266;
    const EXPECTED_DISTANCE: i16 = -7_267;
    const EXPECTED_PITCH: u8 = 167;

    let distance = sf2_xz_angle_distance(DELTA_X, DELTA_Z);
    assert_eq!(distance, EXPECTED_DISTANCE);
    assert_eq!(sf2_pitch_to_target(DELTA_Y, distance), EXPECTED_PITCH);
}
