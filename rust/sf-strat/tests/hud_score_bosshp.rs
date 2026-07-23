//! Tick 149: AUDIT_HUD Critical #1 + High #4 verify —
//! explode specials_dead → calcstageperc / checkbonus; bosshp zero + frame feed.

use sf_game::alien::ASF4_SPECIAL;
use sf_game::planets::Planets;
use sf_game::score::{self, BONERTAB};
use sf_game::Game;
use sf_strat::enemy_a::{strat_explode, wm};

/// Critical #1: explode special → specials_dead → stage % → bonertab credit.
#[test]
fn explode_special_feeds_stage_perc_and_bonus() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("special");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].sflags4 |= ASF4_SPECIAL;
    g.objs.aliens[idx as usize].sflags2 |= 0x08; // ASF2_NOEXPSND
    g.world.total_specials = 1;
    g.vars.write_ext8(wm::SPECIALS_DEAD, 0);

    strat_explode(&mut g, idx);
    assert_eq!(g.vars.read_ext8(wm::SPECIALS_DEAD), 1);

    let perc = score::calc_stage_perc(1, 1, 3); // all teammates
    assert_eq!(perc, 100);

    let mut planets = Planets::new();
    assert!(
        planets.record_stage_score(perc),
        "crossing bonertab {} awards credit",
        BONERTAB[BONERTAB.len() - 2] // 100
    );
    assert_eq!(planets.total_score(), 100);
    assert_eq!(planets.credits, 1);
}

/// High #4: init_strats zeroes bosshp each frame (mdrawbossHP re-sum contract).
#[test]
fn init_strats_zeroes_bosshp_each_frame() {
    let mut g = Game::new();
    // Need a player so init_strats doesn't early-return after the zero.
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.vars.internal_playpt = 0;
    g.vars.bosshp = 99;
    g.vars.bossmaxhp = 200;
    g.run_strategies(); // calls init_strats → bosshp=0
    assert_eq!(g.vars.bosshp, 0, "stale bosshp cleared before strat re-sum");
    assert_eq!(g.vars.bossmaxhp, 200, "bossmaxhp unchanged");
}
