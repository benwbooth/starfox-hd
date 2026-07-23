//! Tick 216: ROM ENEMY1 colltype = `ACF_COLLTYPE2` (0x10) for chicken /
//! flingboss mothers + weapons; bossB scream/ouch `sflag5` = sflags3 bit0
//! (was wrongly ASF2_SFLAG1 / image bit).

use sf_game::alien::{ACF_COLLTYPE2, ASF3_REALOBJ, ASF_NOHITAFFECT};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::COLLTYPE_ENEMY1;
use sf_strat::bossb::{
    bossbrob_init, bossbrobcol_istrat, bossbrobsepcol_istrat, bossbrobstart_init,
    bossbscream_istrat,
};
use sf_strat::bosses::{strat_chicken_init, strat_flingboss_init};

const ASF2_SFLAG1: u8 = 0x10;
const ASF3_SFLAG5: u8 = 0x01;
const HF1: u8 = 1;

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = 0;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("o");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldy = -50;
    g.objs.aliens[idx as usize].worldz = 800;
    idx
}

#[test]
fn chicken_mother_enemy1_is_colltype2() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    strat_chicken_init(&mut g, idx);
    let cf = g.objs.aliens[idx as usize].collflags;
    assert_ne!(cf & ACF_COLLTYPE2, 0);
    assert_eq!(cf & COLLTYPE_ENEMY1, 0);
}

#[test]
fn flingboss_mother_enemy1_is_colltype2() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    strat_flingboss_init(&mut g, idx);
    let cf = g.objs.aliens[idx as usize].collflags;
    assert_ne!(cf & ACF_COLLTYPE2, 0);
    assert_eq!(cf & COLLTYPE_ENEMY1, 0);
}

#[test]
fn scream_sets_sflag5_not_image_sflag1() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    bossbscream_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_ne!(al.sflags3 & ASF3_SFLAG5, 0);
    assert_eq!(al.sflags2 & ASF2_SFLAG1, 0);
    assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
}

#[test]
fn ouch_latches_sflag5_and_blocks_rehit() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    bossbrob_init(&mut g, idx);
    // Image sflag1 stays set from init; ouch must use sflags3.
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[idx as usize].hitflags = HF1;
    bossbrobcol_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_ne!(al.sflags3 & ASF3_SFLAG5, 0);
    assert_eq!(al.sbyte3, 16);
    assert_ne!(al.sflags2 & ASF2_SFLAG1, 0); // image bit untouched

    // Second hit while ouching → .nohitend (hitflags cleared, no re-latch).
    g.objs.aliens[idx as usize].hitflags = HF1;
    g.objs.aliens[idx as usize].sbyte3 = 8;
    bossbrobcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 8);
}

#[test]
fn start_clears_ouch_sflag5_keeps_image() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    bossbrob_init(&mut g, idx);
    g.objs.aliens[idx as usize].sflags3 |= ASF3_SFLAG5;
    bossbrobstart_init(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.sflags3 & ASF3_SFLAG5, 0);
    assert_ne!(al.sflags2 & ASF2_SFLAG1, 0);
}

#[test]
fn sepcol_gates_on_sflag5_sets_sflag1_on_pounce() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    bossbrob_init(&mut g, idx);
    g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
    g.objs.aliens[idx as usize].sflags3 |= ASF3_SFLAG5;
    g.objs.aliens[idx as usize].sbyte2 = 1;
    g.objs.aliens[idx as usize].hitflags = HF1;
    bossbrobsepcol_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1); // gated, no dec

    g.objs.aliens[idx as usize].sflags3 &= !ASF3_SFLAG5;
    g.objs.aliens[idx as usize].sbyte2 = 1;
    bossbrobsepcol_istrat(&mut g, idx);
    // sbyte2 1→0 → set sflag1 + nextstate (may change strat).
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
}
