//! ROM particle/mark explode family (EXPSTRAT.ASM).

use sf_game::alien::{AFEXP, ASF_COLLDISABLE, ASF_PARTOBJ, ASF_SHADOW};
use sf_game::Game;
use sf_strat::enemy_a::{
    bigparticleexplode_istrat, bigparticleexplode_strat, circ2particleexplode_istrat,
    circ2particleexplode_strat, circparticleexplode_istrat, circparticleexplode_strat,
    fastparticleexplode_istrat, fastparticleexplode_strat, lmarkexplode_istrat,
    mmarkexplode_istrat, particleexplode_istrat, particleexplode_strat, particlefire_icont,
    particlefire_istrat, particlefiredown_istrat, smarkexplode_istrat, ASF2_NOEXPSND,
    ASF2_RELEXPLODE,
};

#[test]
fn particleexplode_inits_and_expires_at_40() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldz = 1000;
    particleexplode_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[idx as usize].flags & AFEXP, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_PARTOBJ, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 6);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 60);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 30);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);

    for _ in 0..39 {
        particleexplode_strat(&mut g, idx);
        assert_eq!(g.objs.aldead, 0);
    }
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        1000i16.wrapping_add(35 * 39)
    );
    particleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn fastparticle_clears_payload_only() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    fastparticleexplode_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 7);
    fastparticleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 0);
    assert_eq!(g.objs.aldead, 0);
}

#[test]
fn bigparticle_expires_at_110_and_scrolls_when_relexplode() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.vars.pviewvelz = 10;
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_RELEXPLODE;
    bigparticleexplode_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 255);

    bigparticleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 1);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 110);
    assert_eq!(g.objs.aldead, 0);

    g.objs.aliens[idx as usize].count = 109;
    bigparticleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn circ2_expires_at_50() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    circ2particleexplode_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 250);
    g.objs.aliens[idx as usize].count = 49;
    circ2particleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn circ_is_noop() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    circparticleexplode_istrat(&mut g, idx);
    circparticleexplode_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 0);
}

#[test]
fn particlefire_and_down_set_payload() {
    let mut g = Game::new();
    let a = g.objs.alloc().expect("a");
    particlefire_istrat(&mut g, a);
    assert_eq!(g.objs.aliens[a as usize].sbyte3, 2);
    assert_eq!(g.objs.aliens[a as usize].sbyte1, 2);
    assert_eq!(g.objs.aliens[a as usize].sbyte2, 25);
    assert!(g.objs.aliens[a as usize].expstratptr.is_some());

    let b = g.objs.alloc().expect("b");
    particlefiredown_istrat(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].sbyte3, 3);
    assert_eq!(g.objs.aliens[b as usize].sbyte1, 4);
    assert_eq!(g.objs.aliens[b as usize].sbyte2, 9);

    // Icont alone preserves existing payload
    let c = g.objs.alloc().expect("c");
    g.objs.aliens[c as usize].sbyte3 = 9;
    particlefire_icont(&mut g, c);
    assert_eq!(g.objs.aliens[c as usize].sbyte3, 9);
    assert_ne!(g.objs.aliens[c as usize].flags & AFEXP, 0);
}

#[test]
fn markexplode_spawns_ground_mark_then_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldx = 40;
    g.objs.aliens[idx as usize].worldy = -80;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    smarkexplode_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    // mark spawned then victim marked dead (still active until free sweep)
    assert!(g.objs.aliens.iter().filter(|a| a.active).count() >= before);
    assert!(g
        .objs
        .aliens
        .iter()
        .any(|a| a.active && a.worldy == 0 && a.worldx == 40 && a.worldz == 500));

    let idx2 = g.objs.alloc().expect("2");
    g.objs.aliens[idx2 as usize].sflags2 |= ASF2_NOEXPSND;
    mmarkexplode_istrat(&mut g, idx2);
    assert_eq!(g.objs.aldead, 1);

    let idx3 = g.objs.alloc().expect("3");
    g.objs.aliens[idx3 as usize].sflags2 |= ASF2_NOEXPSND;
    lmarkexplode_istrat(&mut g, idx3);
    assert_eq!(g.objs.aldead, 1);
}
