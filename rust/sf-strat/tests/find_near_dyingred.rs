//! Tick 116: FIND_NEAR*/RADIUS*/MOBJECT/SWORD1 + DYINGRED + MODECHANGE +
//! PRINTAW + FIND_WINDOW_PRI + DRAWLINESBITBYBIT / HDMA markers.

use sf_game::alien::{ASF3_REALOBJ, ASF_SHADOW};
use sf_game::debug_draw::{DebugPrint, HdmaRegion, PlanetScreenDma};
use sf_game::windows::{Windows, WINDOW_MODE_DYINGRED, WINDOW_MODE_HITFLASH};
use sf_game::Game;
use sf_strat::common::{
    find_any_near_object, find_any_radius_object, find_mobject, find_near_object,
    find_radius_object, find_sword1, modechange_add, modechange_set,
};

fn spawn(g: &mut Game, shape: u16, x: i16, z: i16) -> u16 {
    let i = g.objs.alloc().unwrap();
    g.objs.aliens[i as usize].shape = shape;
    g.objs.aliens[i as usize].worldx = x;
    g.objs.aliens[i as usize].worldz = z;
    g.objs.aliens[i as usize].sflags3 |= ASF3_REALOBJ;
    i
}

#[test]
fn find_near_picks_closest_in_band() {
    let mut g = Game::new();
    let me = spawn(&mut g, 1, 0, 0);
    let far = spawn(&mut g, 10, 500, 0); // r=500
    let near = spawn(&mut g, 10, 100, 0); // r=100
    let _ = far;
    let mut fobj = g.objs.active_head;
    let found = find_near_object(&g, 10, me, 0, 800, &mut fobj).expect("near");
    assert_eq!(found, near);
}

#[test]
fn find_radius_returns_first_in_band() {
    let mut g = Game::new();
    let me = spawn(&mut g, 1, 0, 0);
    let a = spawn(&mut g, 20, 50, 0);
    let b = spawn(&mut g, 20, 60, 0);
    let _ = b;
    let mut fobj = g.objs.active_head;
    // List is push-front: b -> a -> me. First in band walking from head.
    let found = find_radius_object(&g, 20, me, 0, 100, &mut fobj).expect("rad");
    assert!(found == a || found == b);
}

#[test]
fn find_any_near_and_mobject_and_sword1() {
    let mut g = Game::new();
    let me = spawn(&mut g, 1, 0, 0);
    let other = spawn(&mut g, 2, 30, 0);
    g.objs.aliens[other as usize].sflags |= ASF_SHADOW;
    g.objs.aliens[other as usize].sword1 = me as i16;

    let mut fobj = g.objs.active_head;
    assert_eq!(find_any_near_object(&g, me, 0, 100, &mut fobj), Some(other));

    let mut fobj = g.objs.active_head;
    assert_eq!(find_mobject(&g, 0, me, ASF_SHADOW, &mut fobj), Some(other));
    assert_eq!(find_sword1(&g, me as i16), Some(other));

    let mut fobj = g.objs.active_head;
    assert!(find_any_radius_object(&g, me, 0, 100, &mut fobj).is_some());
}

#[test]
fn modechange_set_add() {
    let mut g = Game::new();
    let i = spawn(&mut g, 1, 0, 0);
    modechange_set(&mut g, i, 5);
    assert_eq!(g.objs.aliens[i as usize].stratstate, 5);
    modechange_add(&mut g, i, 3);
    assert_eq!(g.objs.aliens[i as usize].stratstate, 8);
}

#[test]
fn dyingred_and_window_pri() {
    let mut w = Windows::new();
    assert!(w.find_window_pri().is_none());
    w.dying_red();
    assert_eq!(w.slots[0].mode, WINDOW_MODE_DYINGRED);
    assert_eq!(w.slots[0].wm_val, 10);
    assert_eq!(w.find_window_pri(), Some(0));
    w.flash_turq();
    assert_eq!(w.slots[1].mode, WINDOW_MODE_HITFLASH);
    assert_eq!(w.find_window_pri(), Some(0));
    w.dying_red_off();
    assert_eq!(w.windowmode & 1, 0); // slot 0 cleared
    assert_eq!(w.find_window_pri(), Some(1)); // hitflash remains
}

#[test]
fn printaw_and_drawlines_hdma() {
    let mut p = DebugPrint::new();
    p.print_aw(2, 0xABCD);
    assert_eq!(p.ab_col, 5);
    assert_eq!(p.glyphs, vec![0xA, 0xB, 0xC, 0xD]);

    let mut d = PlanetScreenDma::default();
    d.draw_lines_bit_by_bit();
    assert_eq!(d.draw_lines, 1);
    let h = HdmaRegion::default();
    assert!(h.start_marker && h.end_marker);
}
