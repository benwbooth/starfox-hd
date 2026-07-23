//! ROM `hover_Istrat` / `implode_*` / `stopexplode_Istrat` / `weapcollide_Istrat`.

use sf_game::alien::{ACF_WEAPON, AFEXP, ASF_COLLDISABLE};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    hover_istrat, implode_istrat, implode_strat, stopexplode_istrat, weapcollide_istrat,
    ASF2_NOEXPSND, ASF2_RELEXPLODE,
};

#[test]
fn hover_scrolls_and_spins() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.vars.pviewvelz = 7;
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[idx as usize].roty = 10;
    hover_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 107);
    assert_eq!(g.objs.aliens[idx as usize].roty, 14);
}

#[test]
fn implode_recovers_after_50_ticks() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].hp = 20;
    implode_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 50);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert_ne!(g.objs.aliens[idx as usize].flags & AFEXP, 0);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);

    for _ in 0..49 {
        implode_strat(&mut g, idx);
        assert_ne!(g.objs.aliens[idx as usize].flags & AFEXP, 0);
    }
    implode_strat(&mut g, idx); // count 1→0: recover
    assert_eq!(g.objs.aliens[idx as usize].flags & AFEXP, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn stopexplode_undoes_velocity_then_explodes() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = 50;
    g.objs.aliens[idx as usize].worldz = 200;
    g.objs.aliens[idx as usize].vx = 10;
    g.objs.aliens[idx as usize].vy = 5;
    g.objs.aliens[idx as usize].vz = 20;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
    stopexplode_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE, 0);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 90);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 45);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 180);
    assert_eq!(g.objs.aliens[idx as usize].vx, 0);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn weapcollide_kills_on_non_weapon_partner() {
    let mut g = Game::new();
    let w = g.objs.alloc().expect("weapon");
    let other = g.objs.alloc().expect("other");
    g.objs.aliens[w as usize].hp = 5;
    g.objs.aliens[w as usize].collobjptr = other;
    g.objs.aliens[other as usize].ap = 8;
    g.objs.aliens[other as usize].collflags = 0; // not ACF_WEAPON
    weapcollide_istrat(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].hp, 0);
    assert_ne!(g.objs.aliens[w as usize].sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn weapcollide_damages_from_weapon_partner() {
    let mut g = Game::new();
    let w = g.objs.alloc().expect("weapon");
    let other = g.objs.alloc().expect("other");
    g.objs.aliens[w as usize].hp = 5;
    g.objs.aliens[w as usize].collobjptr = other;
    g.objs.aliens[other as usize].ap = 3;
    g.objs.aliens[other as usize].collflags = ACF_WEAPON;
    weapcollide_istrat(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].hp, 4);
    assert_eq!(g.objs.aldead, 0);
}

#[test]
fn weapcollide_zero_ap_forces_one_damage() {
    let mut g = Game::new();
    let w = g.objs.alloc().expect("weapon");
    let other = g.objs.alloc().expect("other");
    g.objs.aliens[w as usize].hp = 5;
    g.objs.aliens[w as usize].collobjptr = other;
    g.objs.aliens[other as usize].ap = 0;
    weapcollide_istrat(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].hp, 4);
}
