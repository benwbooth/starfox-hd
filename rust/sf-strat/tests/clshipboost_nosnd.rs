//! Tick 104: clshipboost / clshipboostnosnd + structural flips (UPDATEENGINE/DOMAKESPL/SR_MAKE_*).

use sf_game::Game;
use sf_strat::enemy_a::{clshipboost_istrat, clshipboost_strat, clshipboostnosnd_istrat};
use sf_strat::player::player_warp1_init;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn clshipboost_istrat_plays_se_and_sets_speed() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte2 = 0; // no auto-remove
    clshipboost_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 0x32);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn clshipboostnosnd_istrat_silent() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].snd2 = 0;
    clshipboostnosnd_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 0);
}

#[test]
fn clshipboost_strat_removes_when_sbyte2_hits_1() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    clshipboostnosnd_istrat(&mut g, idx);
    // After DEC, value==1 → remove (ROM: start 2 → one tick removes).
    g.objs.aliens[idx as usize].sbyte2 = 2;
    g.objs.aldead = 0;
    clshipboost_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);
}

#[test]
fn player_warp1_init_wires_nosnd_boost_on_dup() {
    let mut g = Game::new();
    let player = spawn(&mut g);
    g.objs.aliens[player as usize].shape = 1;
    g.objs.aliens[player as usize].worldz = 100;
    let dup = player_warp1_init(&mut g, player).expect("dup");
    assert_ne!(dup, player);
    // istrat falls into strat same frame → sbyte2 19→18.
    assert_eq!(g.objs.aliens[dup as usize].sbyte2, 18);
    assert_eq!(g.objs.aliens[dup as usize].vel, 120);
    assert_eq!(g.objs.aliens[dup as usize].snd2, 0); // nosnd
    assert!(g.objs.aliens[dup as usize].stratptr.is_some());
}
