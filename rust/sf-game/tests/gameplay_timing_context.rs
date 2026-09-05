use sf_game::gameplay_timing::{
    timing_for_update, timing_for_update_with_restart_context, CORNERIA_CHECKPOINT_RESTART_FRAME,
};
use sf_map::catalog::map_id;

const RECORDED_MOTION_REFRESHES: u8 = 4;
const RECORDED_RESTART_REFRESHES: u8 = 103;
const NON_RESTART_FRAMES: [u16; 5] = [0, 186, 942, 944, 982];

#[test]
fn restart_boundary_is_long_only_for_live_restart() {
    let recorded = timing_for_update(map_id::M1_1, CORNERIA_CHECKPOINT_RESTART_FRAME);
    assert_eq!(recorded.motion_refreshes, RECORDED_MOTION_REFRESHES);
    assert_eq!(recorded.presentation_refreshes, RECORDED_RESTART_REFRESHES);

    let alive = timing_for_update_with_restart_context(
        map_id::M1_1,
        CORNERIA_CHECKPOINT_RESTART_FRAME,
        false,
    );
    assert_eq!(alive.motion_refreshes, RECORDED_MOTION_REFRESHES);
    assert_eq!(alive.presentation_refreshes, RECORDED_MOTION_REFRESHES);

    let restarting = timing_for_update_with_restart_context(
        map_id::M1_1,
        CORNERIA_CHECKPOINT_RESTART_FRAME,
        true,
    );
    assert_eq!(restarting, recorded);
}

#[test]
fn contextual_timing_does_not_change_ordinary_frames() {
    for frame in NON_RESTART_FRAMES {
        assert_eq!(
            timing_for_update_with_restart_context(map_id::M1_1, frame, false),
            timing_for_update(map_id::M1_1, frame)
        );
    }
}

#[test]
fn restart_override_is_corneria_specific() {
    assert_eq!(
        timing_for_update_with_restart_context(
            map_id::M1_2,
            CORNERIA_CHECKPOINT_RESTART_FRAME,
            false,
        ),
        timing_for_update(map_id::M1_2, CORNERIA_CHECKPOINT_RESTART_FRAME)
    );
}
