//! ROM hitflash M/S/L/BOSSd + misscol / mchitflash (GSTRATS.ASM).

use sf_game::alien::{ASF_COLLIDE, ASF_HITFLASH, ASF_NOHITAFFECT, ATMISSILE};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    hitflash_bossd_istrat, hitflash_lexp_istrat, hitflash_mexp_istrat, hitflash_sexp_istrat,
    mchitflash_strat, misscol_istrat, strat_hit_flash, ASF4_NOPOLYEXP, EXPSHAPE_LARGE,
    EXPSHAPE_MEDIUM, EXPSHAPE_SMALL,
};

fn mark_normal_strategy(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
}

#[test]
fn hitflash_clears_collide_and_sets_flash() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].hp = 5;
    g.objs.aliens[idx as usize].sflags |= ASF_COLLIDE;
    strat_hit_flash(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLIDE, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
}

#[test]
fn hitflash_nohitaffect_skips_damage() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].hp = 5;
    g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT | ASF_COLLIDE;
    strat_hit_flash(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 5);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn hitflash_tail_calls_the_normal_strategy_on_the_collision_frame() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    let tick = g.world.register_strategy(mark_normal_strategy);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.sflags |= ASF_NOHITAFFECT | ASF_COLLIDE;
    al.stratptr = Some(tick);

    strat_hit_flash(&mut g, idx);

    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLIDE, 0);
}

#[test]
fn hitflash_mexp_spawns_med_at_collobj() {
    let mut g = Game::new();
    let victim = g.objs.alloc().expect("victim");
    let laser = g.objs.alloc().expect("laser");
    g.objs.aliens[victim as usize].hp = 10;
    g.objs.aliens[victim as usize].collobjptr = laser;
    g.objs.aliens[laser as usize].worldx = 50;
    g.objs.aliens[laser as usize].worldy = -20;
    g.objs.aliens[laser as usize].worldz = 900;

    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    hitflash_mexp_istrat(&mut g, victim);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert_eq!(after, before + 1);
    let exp = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| a.active && *i as u16 != victim && *i as u16 != laser)
        .expect("exp");
    assert_eq!(exp.1.shape, EXPSHAPE_MEDIUM);
    assert_eq!(exp.1.worldx, 50);
    assert_eq!(g.objs.aliens[victim as usize].hp, 9);
}

#[test]
fn hitflash_sexp_and_lexp_shapes() {
    let mut g = Game::new();
    let v = g.objs.alloc().expect("v");
    let p = g.objs.alloc().expect("p");
    g.objs.aliens[v as usize].hp = HARD_HP;
    g.objs.aliens[v as usize].collobjptr = p;

    hitflash_sexp_istrat(&mut g, v);
    assert!(g
        .objs
        .aliens
        .iter()
        .any(|a| a.active && a.shape == EXPSHAPE_SMALL));

    let v2 = g.objs.alloc().expect("v2");
    let p2 = g.objs.alloc().expect("p2");
    g.objs.aliens[v2 as usize].hp = HARD_HP;
    g.objs.aliens[v2 as usize].collobjptr = p2;
    hitflash_lexp_istrat(&mut g, v2);
    assert!(g
        .objs
        .aliens
        .iter()
        .any(|a| a.active && a.shape == EXPSHAPE_LARGE));
}

#[test]
fn hitflash_bossd_sets_nopolyexp_when_hittable() {
    let mut g = Game::new();
    let v = g.objs.alloc().expect("v");
    let p = g.objs.alloc().expect("p");
    g.objs.aliens[v as usize].hp = 8;
    g.objs.aliens[v as usize].collobjptr = p;
    hitflash_bossd_istrat(&mut g, v);
    assert!(g
        .objs
        .aliens
        .iter()
        .any(|a| a.active && a.shape == EXPSHAPE_MEDIUM && a.sflags4 & ASF4_NOPOLYEXP != 0));
}

#[test]
fn misscol_kills_non_missile_partner() {
    let mut g = Game::new();
    let m = g.objs.alloc().expect("missile");
    let other = g.objs.alloc().expect("other");
    g.objs.aliens[m as usize].hp = 5;
    g.objs.aliens[m as usize].shape = 10;
    g.objs.aliens[m as usize].collobjptr = other;
    g.objs.aliens[other as usize].shape = 20;
    g.objs.aliens[other as usize].type_ = 0; // not missile
    misscol_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].hp, 0);
    assert_ne!(
        g.objs.aliens[m as usize].sflags & sf_game::alien::ASF_COLLDISABLE,
        0
    );
}

#[test]
fn misscol_missile_partner_arms_mchitflash() {
    let mut g = Game::new();
    let m = g.objs.alloc().expect("m");
    let other = g.objs.alloc().expect("other");
    g.objs.aliens[m as usize].hp = 5;
    g.objs.aliens[m as usize].shape = 10;
    g.objs.aliens[m as usize].collobjptr = other;
    g.objs.aliens[other as usize].shape = 11;
    g.objs.aliens[other as usize].type_ |= ATMISSILE;
    misscol_istrat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].hp, 5);
    assert_ne!(g.objs.aliens[m as usize].sflags & ASF_HITFLASH, 0);
    assert!(g.objs.aliens[m as usize].collstratptr.is_some());

    mchitflash_strat(&mut g, m);
    assert_eq!(g.objs.aliens[m as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(g.objs.aliens[m as usize].hp, 4);
}
