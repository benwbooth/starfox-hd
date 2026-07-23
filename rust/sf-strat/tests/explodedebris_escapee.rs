//! ROM escapee / explodedebris / exppiece family (EXPSTRAT.ASM).

use sf_game::alien::{ASF_COLLDISABLE, ASF_HITFLASH};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    escapee_istrat, escapeeexplode2_istrat, escapeeexplode_istrat, explodebigparts_istrat,
    explodedebris_istrat, exppiece_istrat, exppiece_strat, expspiece_istrat, expspiece_strat,
    fastexplodedebris_istrat, makeescapee_icont, strat_explode, ASF2_NOEXPSND, ASF2_RELEXPLODE,
    MEDPSPEED_I16,
};

#[test]
fn escapee_spins_rotz() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].rotz = 1;
    escapee_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 9);
}

#[test]
fn makeescapee_spawns_then_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    makeescapee_icont(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() > before);
    assert!(g.objs.aliens.iter().any(|a| a.active && a.shape == 341));
    assert_eq!(g.objs.aliens[idx as usize].worldz, 140);
}

#[test]
fn escapeeexplode_always_ends_in_explode() {
    let mut g = Game::new();
    // Force deterministic: seed rndval so both branches still explode.
    g.vars.set_sv_u16(sv::RNDVAL, 0);
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    escapeeexplode_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);

    g.objs.aldead = 0;
    let idx2 = g.objs.alloc().expect("2");
    g.objs.aliens[idx2 as usize].sflags2 |= ASF2_NOEXPSND;
    escapeeexplode2_istrat(&mut g, idx2);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn explodebigparts_spawns_big_particle_then_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    explodebigparts_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() > before);
}

#[test]
fn explodedebris_spawns_particle_and_two_pieces() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].debrisshape = 42;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    explodedebris_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.objs.aliens[idx as usize].debrisshape, 0);
    // particle + 2 pieces
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() >= before + 3);
    assert!(g.objs.aliens.iter().any(|a| a.active && a.shape == 42));
}

#[test]
fn fastexplodedebris_uses_fast_particle() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].debrisshape = 7;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    fastexplodedebris_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    // fast particle has sbyte3=7 after init then cleared on first tick — just check spawn
    assert!(g.objs.aliens.iter().any(|a| a.active && a.shape == 7));
}

#[test]
fn strat_explode_routes_to_debris_when_debrisshape_set() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].debrisshape = 99;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    strat_explode(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert!(g.objs.aliens.iter().any(|a| a.active && a.shape == 99));
}

#[test]
fn exppiece_lives_then_kills() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    exppiece_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_HITFLASH, 0);
    let life = g.objs.aliens[idx as usize].count;
    assert!(life >= 10 && life <= 17);

    for _ in 0..(life - 1) {
        exppiece_strat(&mut g, idx);
        assert_ne!(g.objs.aliens[idx as usize].hp, 0);
    }
    exppiece_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
}

#[test]
fn exppiece_relexplode_scrolls() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.vars.pviewvelz = 10;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_RELEXPLODE;
    g.objs.aliens[idx as usize].count = 20;
    // skip istrat random life — set strat manually
    exppiece_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 20;
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    exppiece_strat(&mut g, idx);
    // +pviewvelz - medpspeed/2
    let expect = 1000i16.wrapping_add(10).wrapping_sub(MEDPSPEED_I16 / 2);
    assert_eq!(g.objs.aliens[idx as usize].worldz, expect);
}

#[test]
fn expspiece_expires_via_remove() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    expspiece_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 15);
    for _ in 0..14 {
        expspiece_strat(&mut g, idx);
        assert_eq!(g.objs.aldead, 0);
    }
    expspiece_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}
