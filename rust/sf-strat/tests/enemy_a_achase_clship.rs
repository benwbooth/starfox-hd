//! Tick 152: AUDIT_ENEMY_A High #6–#9 — proportional Achase (zaco3/4 circle,
//! parajump, clship) + clship_chase → general clshipboost on sword1==0.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::common::chase_proportional;
use sf_strat::enemy_a::{
    strat_clship_chasea_init, strat_para_init, strat_zaco3_init, strat_zaco4_init,
};

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn run_strat(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

fn spawn_houdai(g: &mut Game, z: i16) -> u16 {
    let h = spawn(g);
    g.objs.aliens[h as usize].shape = 54; // SH_HOUDAI_0
    g.objs.aliens[h as usize].worldz = z;
    h
}

/// High #6: zaco3_circle worldy uses Achase shift-1 (half delta), not ±1/frame.
#[test]
fn zaco3_circle_worldy_is_proportional_achase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let _h = spawn_houdai(&mut g, 200);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco3_init(&mut g, idx);

    // Force circle entry: sbyte1==0, dist_xz < 1300, gameframe&7==0.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].sbyte3 = 3; // target_y = -60
    g.objs.aliens[idx as usize].worldy = -400;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = 0;
    g.vars.gameframe = 0;

    let expect = chase_proportional(-400, -60, 1); // -230
    assert_eq!(expect, -230);
    // Linear chase would only move ±1 → -399.
    assert_ne!(expect, -399);

    run_strat(&mut g, idx);
    // After circle body, worldy was Achase'd before move3d; with rotx≈0 the
    // Y component of velocity is tiny vs the 170-unit Achase step.
    let y = g.objs.aliens[idx as usize].worldy;
    assert!(
        (y - expect).abs() < 40,
        "expected ~{expect} (proportional), got {y} (linear would be ~-399)"
    );
    assert!((y - (-399)).abs() > 100, "must not be linear ±1 chase");
}

/// High #6 sibling: zaco4_circle always Achases toward -200.
#[test]
fn zaco4_circle_worldy_is_proportional_achase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    // zaco4_init requires nearby SH_PILLAR3 (shape 27).
    let pillar = spawn(&mut g);
    g.objs.aliens[pillar as usize].shape = 27;
    g.objs.aliens[pillar as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco4_init(&mut g, idx);

    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].worldy = -600;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = 0;
    g.vars.gameframe = 0;

    let expect = chase_proportional(-600, -200, 1); // -400
    run_strat(&mut g, idx);
    let y = g.objs.aliens[idx as usize].worldy;
    assert!((y - expect).abs() < 40, "expected ~{expect}, got {y}");
}

/// High #7: parajump Achases worldy shift-2 / worldx shift-3.
#[test]
fn parajump_uses_proportional_achase() {
    let mut g = Game::new();
    spawn_player(&mut g, 100, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -400;
    g.objs.aliens[idx as usize].worldz = 50; // |dz|=50 → dist_xz < 400
    strat_para_init(&mut g, idx);

    // Advance into para2 (para_strat → para2 when worldy >= 0 path, or force).
    // Force para2 then close enough for parajump: set strat via running until
    // dist triggers, or call after placing in para2 by simulating transition.
    // Easiest: run para until it hops to para2, then set positions.
    // para_init sets para_strat; when worldy>=0 it switches to para2.
    g.objs.aliens[idx as usize].worldy = 0;
    run_strat(&mut g, idx); // may transition to para2

    // Ensure close XZ and far Y for a clear Achase step.
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -400;
    g.objs.aliens[idx as usize].worldz = 50;
    g.vars.player_posy = -40;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].worldx = 100;
    g.objs.aliens[0].worldz = 50;

    let y_expect = chase_proportional(-400, -40, 2); // -310
    let x_expect = chase_proportional(0, 100, 3); // 12 (100/8) or similar
    assert_eq!(y_expect, -310);
    assert_ne!(y_expect, -399); // not linear ±1

    run_strat(&mut g, idx);
    let y = g.objs.aliens[idx as usize].worldy;
    let x = g.objs.aliens[idx as usize].worldx;
    assert_eq!(y, y_expect, "parajump worldy Achase shift 2");
    assert_eq!(x, x_expect, "parajump worldx Achase shift 3");
}

/// High #8: clship chase cont uses proportional Achase on worldy/z.
#[test]
fn clship_chase_worldy_is_proportional() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = -1000;
    g.objs.aliens[idx as usize].worldy = -1000;
    g.objs.aliens[idx as usize].worldz = 500;
    strat_clship_chasea_init(&mut g, idx);
    // Keep sword1 > 0 so we stay in chase (not boost).
    g.objs.aliens[idx as usize].sword1 = 10;
    g.objs.aliens[idx as usize].worldy = -1000;

    // chasea: worldy → player_y+20 = -20, shift 5 → step = (-20-(-1000))>>5 = 30
    let expect = chase_proportional(-1000, -20, 5);
    assert_eq!(expect, -970);
    // Linear ±5 would be -995.
    assert_ne!(expect, -995);

    run_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldy, expect,
        "clship chase worldy must be Achase shift-5"
    );
}

/// High #9: sword1==0 → general clshipboost (vel 120, snd2 $32), not chaseboost.
#[test]
fn clship_chase_expires_into_general_boost() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_clship_chasea_init(&mut g, idx);
    let chase = g.objs.aliens[idx as usize].stratptr;
    g.objs.aliens[idx as usize].sword1 = 0;
    g.objs.aliens[idx as usize].vel = 40;

    run_strat(&mut g, idx);

    assert_ne!(g.objs.aliens[idx as usize].stratptr, chase);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120, "general boost speed");
    assert_eq!(
        g.objs.aliens[idx as usize].snd2, 0x32,
        "trigse $32 via snd2 latch"
    );
}
