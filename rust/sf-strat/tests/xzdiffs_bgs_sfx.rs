//! Tick 110: XZDIFFS_ABS/OFF + DOBGREQ/TRANSSWAP/STARTSFX.

use sf_game::alien::Alien;
use sf_game::bgs::{do_bg_req, trans_swap, BgRequestResult};
use sf_game::clip::{start_sfx, BgScrollOffsets};
use sf_game::vars::{GameVars, BGF_BG, BGF_INFO, BGF_RESTART};
use sf_game::Game;
use sf_strat::common::{strat_dist_xz, xz_diffs_abs, xz_diffs_off};

#[test]
fn xz_diffs_abs_is_manhattan() {
    // ROM xzdiffs_abs_l: |dx|+|dz| (NOT scaled-Euclidean xzdiffs_l).
    assert_eq!(xz_diffs_abs(0, 0, 0, 0), 0);
    assert_eq!(xz_diffs_abs(0, 0, 400, 0), 400);
    assert_eq!(xz_diffs_abs(0, 0, 0, 300), 300);
    assert_eq!(xz_diffs_abs(10, 20, 40, 50), 60); // |30|+|30|
                                                  // Scaled Euclidean for same inputs is smaller than Manhattan.
    let mut a = Alien::default();
    let mut b = Alien::default();
    a.worldx = 0;
    a.worldz = 0;
    b.worldx = 400;
    b.worldz = 0;
    let scaled = strat_dist_xz(&a, &b);
    assert!(scaled > 0 && scaled < 400, "scaled={scaled}");
}

#[test]
fn xz_diffs_off_adds_offset_before_manhattan() {
    let mut g = Game::new();
    let a = g.objs.alloc().unwrap();
    let b = g.objs.alloc().unwrap();
    g.objs.aliens[a as usize].worldx = 0;
    g.objs.aliens[a as usize].worldz = 0;
    g.objs.aliens[b as usize].worldx = 100;
    g.objs.aliens[b as usize].worldz = 50;
    // Without offset: 150. With ox=10, oz=-10: |110|+|40|=150.
    assert_eq!(
        xz_diffs_off(
            &g.objs.aliens[a as usize],
            &g.objs.aliens[b as usize],
            10,
            -10
        ),
        150
    );
    assert_eq!(
        xz_diffs_off(&g.objs.aliens[a as usize], &g.objs.aliens[b as usize], 0, 0),
        150
    );
    assert_eq!(
        xz_diffs_off(
            &g.objs.aliens[a as usize],
            &g.objs.aliens[b as usize],
            50,
            50
        ),
        250
    );
}

#[test]
fn dobgreq_and_transswap() {
    let mut vars = GameVars::init();
    vars.bgflags = BGF_BG;
    do_bg_req(&mut vars);
    assert_eq!(vars.bgflags, 0);

    vars.bgflags = BGF_BG | BGF_INFO | BGF_RESTART;
    let r = trans_swap(&mut vars);
    assert_eq!(
        r,
        BgRequestResult {
            restart: true,
            bg_change: true,
            info: true,
        }
    );
    assert_eq!(vars.bgflags, 0);
}

#[test]
fn startsfx_clears_bg_scroll() {
    let mut s = BgScrollOffsets {
        bg1_vofs: 12,
        bg4_hofs: 34,
        ..Default::default()
    };
    start_sfx(&mut s);
    assert_eq!(s, BgScrollOffsets::default());
}
