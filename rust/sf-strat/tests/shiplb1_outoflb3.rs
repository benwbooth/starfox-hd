//! Tick 84: shiplb1 + shipoutoflb3 + boss1makechild.

use sf_game::alien::{ASF_COLLDISABLE, ATGND, NUMBER_AL};
use sf_game::Game;
use sf_strat::common::StratRam;
use sf_strat::enemy_a::wm;
use sf_strat::enemy_a::{
    boss1makechild, shiplb1_istrat, shiplb1ychase_srou, shipoutoflb3_istrat, shipoutoflb3_strat,
    strat_boss1_init, ASF4_CHILDOBJ, DEG45, DEG90, MEDPSPEED_I16,
};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn shiplb1_start_turn_and_ychase() {
    let mut g = Game::new();
    let view = spawn(&mut g);
    g.objs.aliens[view as usize].worldy = -200;
    g.vars
        .set_sv_i16(sf_strat::common::sv::VIEWTOOBJ, view as i16);

    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = 2;
    g.objs.aliens[idx as usize].sword1 = 50;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].stratstate = 0;

    shiplb1_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, 20);
    assert_eq!(g.objs.aliens[idx as usize].rotz, DEG45);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG90);
    // decbne: 2→1, still state 0
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    // ychase toward viewy+sword1 = -150
    assert!(g.objs.aliens[idx as usize].worldy < 0);

    // Next tick: 1→0 → state 1; same-frame fall-through decbne → 24
    shiplb1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 24); // 25 set then same-frame dec

    // Pure ychase helper
    g.objs.aliens[idx as usize].worldy = 0;
    shiplb1ychase_srou(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].worldy < 0);
}

#[test]
fn shipoutoflb3_cruise_then_wait() {
    let mut g = Game::new();
    let view = spawn(&mut g);
    g.objs.aliens[view as usize].worldx = 100;
    g.objs.aliens[view as usize].worldy = -50;
    g.vars
        .set_sv_i16(sf_strat::common::sv::VIEWTOOBJ, view as i16);
    g.vars.set_sv_i16(sf_strat::common::sv::VIEWPOSZ, 1000);

    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 0;
    shipoutoflb3_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 150);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATGND, 0);

    let z0 = g.objs.aliens[idx as usize].worldz;
    shipoutoflb3_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(MEDPSPEED_I16.wrapping_add(35))
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 149);

    // Force state 1 → snap Z to ptr+viewposz
    g.objs.aliens[idx as usize].stratstate = 1;
    g.objs.aliens[idx as usize].ptr = 200; // Z offset stand-in
    shipoutoflb3_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        200i16.wrapping_add(1000)
    );
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 0);

    // State 2 wait: track view + offsets
    g.objs.aliens[idx as usize].sword1 = 10;
    g.objs.aliens[idx as usize].sword2 = -20;
    let z2 = g.objs.aliens[idx as usize].worldz;
    shipoutoflb3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 10i16.wrapping_add(100));
    assert_eq!(
        g.objs.aliens[idx as usize].worldy,
        (-20i16).wrapping_add(-50)
    );
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z2.wrapping_add(MEDPSPEED_I16.wrapping_add(15))
    );
}

#[test]
fn boss1makechild_spawns_family() {
    let mut g = Game::new();
    g.vars.write_ext8(wm::CURRENTLEVEL, 1);
    let boss = spawn(&mut g);
    // Full init (includes makechild)
    strat_boss1_init(&mut g, boss);
    let children: Vec<_> = (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].sflags4 & ASF4_CHILDOBJ != 0)
        .collect();
    assert!(
        children.len() >= 9,
        "cover + 8 turrets, got {}",
        children.len()
    );
    assert_ne!(g.objs.aliens[boss as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[boss as usize].sbyte3, 1);

    // Direct makechild on a fresh mother
    let mut g2 = Game::new();
    let m = spawn(&mut g2);
    boss1makechild(&mut g2, m);
    let n = (0..NUMBER_AL)
        .filter(|&i| g2.objs.aliens[i].active && g2.objs.aliens[i].sflags4 & ASF4_CHILDOBJ != 0)
        .count();
    assert!(n >= 9);
}
