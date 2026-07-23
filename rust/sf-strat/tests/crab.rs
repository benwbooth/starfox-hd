//! ROM crab B/L/T/R screen-edge walker (GASTRATS.ASM:1821).

use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    crab_cont, crab_init, crabb_init, crabb_istrat, crabb_strat, crabl_init, crabl_istrat,
    crabr_init, crabr_istrat, crabt_init, crabt_istrat, DEG180, DEG90,
};

const ASF2_SFLAG1: u8 = 0x10;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_crab(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("crab");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    idx
}

fn set_space_bounds(g: &mut Game) {
    // space_min/max from STRATEQU.INC (+ playerB_Ystop on maxY).
    g.vars.set_sv_i16(sv::MINPMOVEX, -240);
    g.vars.set_sv_i16(sv::MAXPMOVEX, 240);
    g.vars.minpmove_y = -190;
    g.vars.set_sv_i16(sv::MAXPMOVEY, 80 - 20); // space_maxY + playerB_Ystop
}

#[test]
fn crab_init_sets_hp_ap_facing() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_crab(&mut g);
    crab_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 10);
}

#[test]
fn crabb_istrat_wires_and_walks_left() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    crabb_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    let x0 = g.objs.aliens[idx as usize].worldx;
    crabb_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].worldx < x0);
}

#[test]
fn crabb_turns_to_l_at_minx() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldx = -250; // < minPmoveX
    crabb_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG90); // now L
}

#[test]
fn crabt_turns_to_r_at_maxx() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldx = 250;
    crabt_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte1,
        DEG180.wrapping_add(DEG90)
    );
}

#[test]
fn crabl_turns_to_t_at_miny() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldy = -200;
    crabl_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, DEG180);
}

#[test]
fn crabr_turns_to_b_at_maxy() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldy = 100;
    crabr_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
}

#[test]
fn crab_cont_removes_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldz = 5000;
    crab_cont(&mut g, idx, -8, 0);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn crab_cont_fires_missile_at_center() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let idx = spawn_crab(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 1000; // in fire range
    g.objs.aliens[idx as usize].sbyte2 = 6; // next tick → 5 → fire
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // already latched
    let before = g.objs.active_indices().len();
    crab_cont(&mut g, idx, -8, 0);
    assert!(g.objs.active_indices().len() > before);
}

#[test]
fn all_istrats_set_distinct_sbyte1() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_space_bounds(&mut g);
    let b = spawn_crab(&mut g);
    crabb_istrat(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].sbyte1, 0);
    let l = spawn_crab(&mut g);
    crabl_istrat(&mut g, l);
    assert_eq!(g.objs.aliens[l as usize].sbyte1, DEG90);
    let t = spawn_crab(&mut g);
    crabt_istrat(&mut g, t);
    assert_eq!(g.objs.aliens[t as usize].sbyte1, DEG180);
    let r = spawn_crab(&mut g);
    crabr_istrat(&mut g, r);
    assert_eq!(g.objs.aliens[r as usize].sbyte1, DEG180.wrapping_add(DEG90));
}
