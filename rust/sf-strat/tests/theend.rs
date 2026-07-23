//! Tick 98: THEEND zoom/fin/flip/flyaway/check (KSTRATS.ASM).

use sf_game::alien::{ASF_COLLDISABLE, ATZREMOVE};
use sf_game::Game;
use sf_strat::theend::{
    theend_check_istrat, theend_fin2_istrat, theend_fin_istrat, theend_fin_strat,
    theend_flip_istrat, theend_flip_strat, theend_flyaway_istrat, theend_flyaway_strat,
    theend_zoom2_istrat, theend_zoom2_strat, theend_zoom_istrat, theend_zoom_strat,
};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn zoom_counts_down_to_fin() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].sword1 = 11;
    g.objs.aliens[idx as usize].sword2 = 22;
    g.objs.aliens[idx as usize].rotz = 0; // fin settles immediately via theendok

    theend_zoom_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 32); // 33 then same-frame dec
    assert_eq!(g.objs.aliens[idx as usize].roty, 4);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1000 - 19);

    // Force handoff
    g.objs.aliens[idx as usize].sbyte1 = 1;
    theend_zoom_strat(&mut g, idx);
    // fin_istrat ran with rotz==0 → theendok → shape=sword1, numendok++
    assert_eq!(g.objs.aliens[idx as usize].shape, 11);
    assert_eq!(g.vars.numendok, 1);
}

#[test]
fn zoom2_handoff_fin2() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sword1 = 5;
    g.objs.aliens[idx as usize].rotz = 128;
    theend_zoom2_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 1;
    theend_zoom2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].shape, 5);
    assert_eq!(g.vars.numendok, 1);
}

#[test]
fn fin_unsettled_uses_sword2_shape() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].rotz = 40;
    g.objs.aliens[idx as usize].sword2 = 77;
    g.objs.aliens[idx as usize].type_ |= ATZREMOVE;
    theend_fin_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    assert_eq!(g.objs.aliens[idx as usize].shape, 77);
}

#[test]
fn check_six_starts_wait_and_bgm() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.numendok = 6;
    theend_check_istrat(&mut g, idx);
    assert_eq!(g.vars.numendok, 0); // reset this frame
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
    assert_eq!(g.vars.numendok, 0xFF);
}

#[test]
fn flyaway_spins_and_retreats() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    theend_flyaway_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 3);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 500 - 20);
    theend_flyaway_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 6);
}

#[test]
fn flip_tumble_then_restore() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    let fin = g.world.register_strategy(theend_fin_strat);
    g.objs.aliens[idx as usize].stratptr = Some(fin);
    g.objs.aliens[idx as usize].sbyte3 = 0;

    theend_flip_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 2);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 31); // 32 then same-frame dec
    assert_eq!(g.objs.aliens[idx as usize].vy, -45 + 3);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);

    g.objs.aliens[idx as usize].sbyte1 = 1;
    theend_flip_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratptr, Some(fin));
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn ok_negative_numendok_flies_away() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.numendok = 0xFF;
    g.objs.aliens[idx as usize].rotz = 0;
    theend_fin2_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATZREMOVE, 0);
}
