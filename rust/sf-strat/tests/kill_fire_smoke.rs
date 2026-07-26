//! ROM `kill_Istrat` / `makefire_srou_l` / `fire_*` / `smokeP_*` / `puff_*`
//! (GSTRATS.ASM).

use sf_game::alien::{ObjectVisualKind, AFONFIRE, ASF3_REALOBJ, ASF_COLLDISABLE, ATZREMOVE};
use sf_game::Game;
use sf_strat::common::{
    fire_istrat, fire_strat, kill_istrat, makefire_srou, makesmoke_srou, puff_istrat, puff_strat,
    smoke_p_istrat, smoke_p_strat, sv, StratRam,
};

#[test]
fn kill_istrat_zeros_hp_and_disables_collision() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].hp = 40;
    g.objs.aliens[idx as usize].sflags = 0;

    kill_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn makefire_attaches_child_and_sets_onfire() {
    let mut g = Game::new();
    let parent = g.objs.alloc().expect("parent");
    g.objs.aliens[parent as usize].worldx = 100;
    g.objs.aliens[parent as usize].worldy = -50;
    g.objs.aliens[parent as usize].worldz = 2000;
    g.vars.set_sv_u8(sv::SMVAR_BYTE1, 32);

    let fire = makefire_srou(&mut g, parent).expect("fire");
    assert_eq!(g.objs.aliens[parent as usize].flags & AFONFIRE, AFONFIRE);
    assert_eq!(g.objs.aliens[parent as usize].fireobjptr, fire + 1);
    assert_eq!(g.objs.aliens[fire as usize].sbyte1, 32);
    assert_eq!(g.objs.aliens[fire as usize].worldx, 100);
    assert_eq!(g.objs.aliens[fire as usize].worldz, 2000);
    assert_eq!(g.objs.aliens[fire as usize].sflags3 & ASF3_REALOBJ, 0);
    assert_ne!(g.objs.aliens[fire as usize].sflags & ASF_COLLDISABLE, 0);
    assert_ne!(g.objs.aliens[fire as usize].type_ & ATZREMOVE, 0);

    fire_istrat(&mut g, fire);
    assert_eq!(g.objs.aliens[fire as usize].sbyte2, 9);
}

#[test]
fn fire_strat_emits_smoke_at_period_and_midpoints() {
    let mut g = Game::new();
    let fire = g.objs.alloc().expect("fire");
    g.objs.aliens[fire as usize].sbyte1 = 32;
    g.objs.aliens[fire as usize].sbyte2 = 9;
    g.objs.aliens[fire as usize].worldx = 10;
    g.objs.aliens[fire as usize].worldy = 20;
    g.objs.aliens[fire as usize].worldz = 30;

    // 9→8: mid-period smoke
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    fire_strat(&mut g, fire);
    assert_eq!(g.objs.aliens[fire as usize].sbyte2, 8);
    let after_8 = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after_8, before + 1);

    // Count down to 1 without more smoke at non-8/16
    for expect in (1..=7).rev() {
        let n0 = g.objs.aliens.iter().filter(|a| a.active).count();
        fire_strat(&mut g, fire);
        assert_eq!(g.objs.aliens[fire as usize].sbyte2, expect);
        let n1 = g.objs.aliens.iter().filter(|a| a.active).count();
        assert_eq!(n1, n0, "no smoke at sbyte2={expect}");
    }

    // 1→0: reload from sbyte1 and smoke
    let n0 = g.objs.aliens.iter().filter(|a| a.active).count();
    fire_strat(&mut g, fire);
    assert_eq!(g.objs.aliens[fire as usize].sbyte2, 32);
    assert_eq!(g.objs.aliens.iter().filter(|a| a.active).count(), n0 + 1);
}

#[test]
fn smoke_p_drifts_and_expires_on_lift() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("smoke");
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    smoke_p_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 20);
    assert_eq!(g.objs.aliens[idx as usize].sword1, 6);
    assert_eq!(
        g.objs.aliens[idx as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.objs.aliens[idx as usize].depthoffset, 0);
    assert_eq!(g.objs.aliens[idx as usize].tx, 0);

    smoke_p_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, -1);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -6);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 19);
    assert_eq!(g.objs.aldead, 0);

    // Exhaust lift: 20 ticks per sword1 step, 6 steps → sword1 hits 0.
    for _ in 0..(20 * 6) {
        if g.objs.aldead != 0 {
            break;
        }
        smoke_p_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn makesmoke_spawns_at_parent_pos() {
    let mut g = Game::new();
    let parent = g.objs.alloc().expect("parent");
    g.objs.aliens[parent as usize].worldx = 7;
    g.objs.aliens[parent as usize].worldy = 8;
    g.objs.aliens[parent as usize].worldz = 9;
    let smoke = makesmoke_srou(&mut g, parent).expect("smoke");
    assert_eq!(g.objs.aliens[smoke as usize].worldx, 7);
    assert_eq!(g.objs.aliens[smoke as usize].worldy, 8);
    assert_eq!(g.objs.aliens[smoke as usize].worldz, 9);
}

#[test]
fn puff_animates_then_removes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("puff");
    g.vars.pviewvelz = 5;
    g.objs.aliens[idx as usize].worldz = 100;
    puff_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(
        g.objs.aliens[idx as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.objs.aliens[idx as usize].depthoffset, 0);
    assert_eq!(g.objs.aliens[idx as usize].tx, 0);

    // Frames 0→1…→7: still alive; at frame 8 after add, remove.
    for _ in 0..8 {
        assert_eq!(g.objs.aldead, 0);
        puff_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aldead, 1);
}
