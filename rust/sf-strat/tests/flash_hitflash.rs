//! Tick 105: FLASHTURQ / FLASHTURQ2 / FLASHRED + structural achase/remove flips.

use sf_game::windows::{
    Windows, HITFLASH_RED, HITFLASH_TURQ, HITFLASH_TURQ2, WINDOW_MODE_HITFLASH,
};
use sf_game::Game;
use sf_strat::common::{flashred_l, flashturq2_l, flashturq_l, hitflash_off_l};

#[test]
fn windows_hitflash_colors() {
    let mut w = Windows::new();
    w.flash_turq();
    assert_eq!(w.slots[0].mode, WINDOW_MODE_HITFLASH);
    assert_eq!(w.slots[0].stayblack, HITFLASH_TURQ);
    assert_eq!(w.slots[0].wm_val, 31);
    w.flash_turq2();
    assert_eq!(w.slots[0].stayblack, HITFLASH_TURQ2);
    assert_eq!(w.slots[0].wm_val, 7);
    w.flash_red();
    assert_eq!(w.slots[0].stayblack, HITFLASH_RED);
    w.hitflash_off();
    assert_eq!(w.windowmode, 0);
}

#[test]
fn strat_flash_hooks_are_callable() {
    // NullHooks no-ops — verifies the strat wrappers compile and dispatch.
    let mut g = Game::new();
    flashturq_l(&mut g);
    flashturq2_l(&mut g);
    flashred_l(&mut g);
    hitflash_off_l(&mut g);
}

#[test]
fn achase_angle_matches_sr8_rate3_sample() {
    // Structural coverage for SR8_ACHASE_ALVAR* (fuzz_pure_fns already exact).
    use sf_strat::enemy_a::achase_angle;
    let mut cur = 0u8;
    assert!(!achase_angle(&mut cur, 64, 3));
    assert_ne!(cur, 0);
    let same = cur;
    assert!(achase_angle(&mut cur, same, 3));
}

#[test]
fn chase_proportional_matches_sr16_sample() {
    use sf_strat::common::chase_proportional;
    let next = chase_proportional(0, 1000, 3);
    assert!(next > 0 && next < 1000);
    assert_eq!(chase_proportional(1000, 1000, 3), 1000);
}
