//! Tick 118: PERC*A verify + SET_*/INIT*/DO_* + XFLYTOPOS + display FX.

use sf_core::scene::PaletteFadeTarget;
use sf_game::bgs::{set_bg, set_bg_info_req, set_restart_fade};
use sf_game::debug_draw::{BootInit, DisplayFx};
use sf_game::vars::{GameVars, BGF_BG, BGF_INFO};
use sf_game::Game;
use sf_strat::common::{
    set_0_collptrs, set_norm_collptrs, strat_perc56, strat_perc62, strat_perc75, strat_perc87,
    strat_perc93, x_fly_to_pos,
};

#[test]
fn perc_a_family_matches_ported_helpers() {
    // PERC*A_L are the far wrappers around these; already oracle-fuzzed.
    assert_eq!(strat_perc56(100), 56); // 50+6
    assert_eq!(strat_perc62(100), 62); // 50+12
    assert_eq!(strat_perc75(100), 75); // 50+25
    assert_eq!(strat_perc87(100), 87); // 50+25+12
    assert_eq!(strat_perc93(100), 93);
}

#[test]
fn set_bg_and_info_and_restart_fade() {
    let mut v = GameVars::init();
    set_bg(&mut v, 3);
    assert_eq!(v.currentbg, 3);
    assert_ne!(v.bgflags & BGF_BG, 0);
    set_bg_info_req(&mut v);
    assert_ne!(v.bgflags & BGF_INFO, 0);
    set_restart_fade(&mut v, 30);
    assert_eq!(v.palfade_num, 30);
    assert_eq!(v.palfade_target, Some(PaletteFadeTarget::Sea));
    set_restart_fade(&mut v, 62);
    assert_eq!(
        v.palfade_num, 30,
        "saved ground offset is not a frame count"
    );
    assert_eq!(v.palfade_target, Some(PaletteFadeTarget::Ground));
    set_restart_fade(&mut v, 0); // no-op
    assert_eq!(v.palfade_num, 30);
    assert_eq!(v.palfade_target, Some(PaletteFadeTarget::Ground));
}

#[test]
fn collptrs_zero_and_norm() {
    let mut g = Game::new();
    let idx = g.objs.alloc().unwrap();
    set_norm_collptrs(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].collstratptr.is_some());
    assert!(g.objs.aliens[idx as usize].expstratptr.is_some());
    set_0_collptrs(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].collstratptr.is_none());
    assert!(g.objs.aliens[idx as usize].expstratptr.is_none());
}

#[test]
fn xflytopos_banks_toward_target() {
    let mut g = Game::new();
    let idx = g.objs.alloc().unwrap();
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    x_fly_to_pos(&mut g, idx, 200);
    assert!((g.objs.aliens[idx as usize].sbyte3 as i8) > 0);
    assert!(g.objs.aliens[idx as usize].worldx > 0);
    // Chase left
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    x_fly_to_pos(&mut g, idx, 0);
    assert!((g.objs.aliens[idx as usize].sbyte3 as i8) < 0);
}

#[test]
fn boot_init_wmat_mario_mem_and_display_fx() {
    let mut b = BootInit::default();
    b.init_wmat();
    b.init_mario_3d();
    b.minit_dust();
    b.init_mem();
    assert!(b.init_3d >= 2);
    assert!(b.init_sprites >= 1);
    assert!(b.init_game >= 1);

    let mut d = DisplayFx::default();
    d.set_inidisp1();
    assert_eq!(d.inidisp & 0x80, 0x80);
    d.set_noclash();
    assert!(d.noclash);
    d.set_pal();
    d.set_game_pal();
    d.setup_planet_pal();
    let mut ca = 5i16;
    d.do_circle_explosion(&mut ca);
    assert_eq!(ca, 6);
    d.do_window_wipe();
    d.do_hpositions();
    d.undraw_planet_lines();
    d.pepper_fade();
    d.wipe_init();
    d.reset_sprites();
    assert_eq!(d.pal_set, 1);
    assert_eq!(d.reset_sprites, 1);
    assert_eq!(d.circle_explosion, 1);
}
