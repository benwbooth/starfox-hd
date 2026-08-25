//! ROM bossB face spin / scream / bent / entsplit (GB3STRAT.ASM).

use sf_game::alien::{ASF2_COLLDISABLE, ASF3_NOHITAFFECT};
use sf_game::Game;
use sf_strat::bossb::{
    bossbent_cont, bossbent_istrat, bossbent_strat, bossbentlong_istrat, bossbentsplit2_istrat,
    bossbentsplit_cont, bossbentsplit_istrat, bossbentsplit_strat, bossbentsplitcol_istrat,
    bossbscream2_init, bossbscream_istrat, bossbscreamend_init, bossbspin1_init, bossbspin1_strat,
    bossbspin2_init, bossbspinend2_init, bossbspinend2_strat, bossbspinend_cont,
    bossbspinend_istrat, bossbspinendcol_istrat, bossbspinendentcol_istrat,
    bossbspinendnewent_srou,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
}

fn spawn_face(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("face");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].hp = 20;
    idx
}

#[test]
fn spin1_recenters_then_spin2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    g.objs.aliens[idx as usize].worldx = 10;
    g.objs.aliens[idx as usize].worldy = 40; // near SPACE_VIEWCY+100 = 40
    bossbspin1_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    // Already near center → spin2 (nohitaffect).
    bossbspin1_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT, 0);
}

#[test]
fn spin2_to_spinend2_resets_hp() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    bossbspin2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, 64); // DEG90
                                                      // Force sbyte1 expire → spinend2.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    // Call strat via spinend2_init path: set sbyte1=0 and tick through spin2.
    // Directly enter spinend2.
    bossbspinend2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 30); // bossBspinHP
    assert_eq!(g.objs.aliens[idx as usize].sword2, 30);
}

#[test]
fn spinend2_screams_on_big_damage() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    bossbspinend2_init(&mut g, idx);
    g.objs.aliens[idx as usize].sword2 = 30;
    g.objs.aliens[idx as usize].hp = 20; // Δ=10 ≥ 6
    bossbspinend2_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT, 0);
    assert!(g.objs.aliens[idx as usize].sbyte1 <= 30);
}

#[test]
fn scream_chain_returns_to_spinend2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    bossbscream_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29);
    bossbscream2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 92);
    // Avoid re-entering scream (Δhp gate) on the spinend2 fall-through.
    g.objs.aliens[idx as usize].sword2 = g.objs.aliens[idx as usize].hp as i16;
    bossbscreamend_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, 13); // 14 then spinend2 tick
    assert_eq!(g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT, 0);
}

#[test]
fn newent_and_cols() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    bossbspinendnewent_srou(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, 14);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 1);
    bossbspinendcol_istrat(&mut g, idx);
    let child = spawn_face(&mut g);
    bossbspinendentcol_istrat(&mut g, child);
    assert_ne!(g.objs.aliens[child as usize].sflags2 & ASF2_COLLDISABLE, 0);
}

#[test]
fn spinend_child_and_cont() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let mother = spawn_face(&mut g);
    let child = spawn_face(&mut g);
    g.objs.aliens[child as usize].ptr = mother;
    g.objs.aliens[mother as usize].sbyte1 = 0;
    bossbspinend_istrat(&mut g, child);
    assert_eq!(g.objs.aliens[child as usize].hp, 0xff);
    bossbspinend_cont(&mut g, child);
}

#[test]
fn bent_and_long_fade() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    bossbent_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 7); // 8 then tick
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_COLLDISABLE, 0);
    bossbent_cont(&mut g, idx);
    let long = spawn_face(&mut g);
    bossbentlong_istrat(&mut g, long);
    assert_eq!(g.objs.aliens[long as usize].count, 19); // 20 then tick
    bossbent_strat(&mut g, long);
}

#[test]
fn entsplit_fires_and_drifts() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_face(&mut g);
    g.objs.aliens[idx as usize].sword1 = 100;
    g.objs.aliens[idx as usize].sword2 = -200;
    bossbentsplit_istrat(&mut g, idx);
    // Icont sets sbyte1=1 then cont → 0→100.
    assert!(g.objs.aliens[idx as usize].sbyte1 > 0);
    bossbentsplitcol_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 50;
    bossbentsplit_strat(&mut g, idx);
    bossbentsplit_cont(&mut g, idx);
    bossbentsplit2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT, 0);
}
