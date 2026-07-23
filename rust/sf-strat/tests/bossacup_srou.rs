//! Tick 90: bossAcup open/up/uplow/getchild + L/M/R istrat leaves.

use sf_game::alien::{ASF_NOHITAFFECT, ASF_SHADOW};
use sf_game::Game;
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    bossacupopen_srou, bossacupperl_istrat, bossacupperm_istrat, bossacupperr_istrat,
    bossacupup_srou, bossacupuplow_srou, getbossacupchild_srou,
};

const DEG90: u8 = 64;
const DEG180: u8 = 128;
const BOSSA_SCALE: i16 = 2;
const BOSSA_CUP_HP: u8 = 24;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn attach_mother_turret_cup(g: &mut Game) -> (u16, u16, u16) {
    let mother = spawn(g);
    let turret = spawn(g);
    let cup = spawn(g);
    assert!(boss_attach_child_to_mother(g, mother, turret, 1));
    assert!(boss_attach_child_to_mother(g, mother, cup, 4));
    g.objs.aliens[turret as usize].worldx = 100;
    g.objs.aliens[turret as usize].worldy = -200;
    g.objs.aliens[turret as usize].worldz = 500;
    g.objs.aliens[mother as usize].roty = 40;
    (mother, turret, cup)
}

#[test]
fn bossacupper_lmr_init_and_getchild() {
    let mut g = Game::new();
    let (mother, turret, cup) = attach_mother_turret_cup(&mut g);

    bossacupperl_istrat(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].sbyte2, 1); // turret L
    assert_eq!(g.objs.aliens[cup as usize].hp, BOSSA_CUP_HP);
    assert_eq!(g.objs.aliens[cup as usize].rotx, 0u8.wrapping_sub(DEG90));
    assert_eq!(g.objs.aliens[cup as usize].roty, DEG180);
    assert_ne!(g.objs.aliens[cup as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(g.objs.aliens[cup as usize].sflags & ASF_NOHITAFFECT, 0);
    // Home: rotz=mother.roty; pos = turret + (0,-15<<2,-2<<2)
    assert_eq!(g.objs.aliens[cup as usize].rotz, 40);
    assert_eq!(g.objs.aliens[cup as usize].worldx, 100);
    assert_eq!(
        g.objs.aliens[cup as usize].worldy,
        (-200i16).wrapping_sub(15 << BOSSA_SCALE)
    );
    assert_eq!(
        g.objs.aliens[cup as usize].worldz,
        500i16.wrapping_sub(2 << BOSSA_SCALE)
    );
    assert_eq!(getbossacupchild_srou(&mut g, cup), Some(turret));

    let cup_m = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, cup_m, 5));
    bossacupperm_istrat(&mut g, cup_m);
    assert_eq!(g.objs.aliens[cup_m as usize].sbyte2, 2);

    let cup_r = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, cup_r, 6));
    bossacupperr_istrat(&mut g, cup_r);
    assert_eq!(g.objs.aliens[cup_r as usize].sbyte2, 3);
}

#[test]
fn bossacupopen_and_up_chase() {
    let mut g = Game::new();
    let (_mother, _turret, cup) = attach_mother_turret_cup(&mut g);
    bossacupperl_istrat(&mut g, cup);
    g.objs.aliens[cup as usize].animframe = 0;
    g.vars.gameframe = 0; // even → open steps

    bossacupopen_srou(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].animframe, 1);
    g.vars.gameframe = 1; // odd → no step
    bossacupopen_srou(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].animframe, 1);

    // Cap at 6
    g.objs.aliens[cup as usize].animframe = 6;
    g.vars.gameframe = 0;
    bossacupopen_srou(&mut g, cup);
    assert_eq!(g.objs.aliens[cup as usize].animframe, 6);

    // Place cup away from turret home; UP chase toward turret Y−440
    g.objs.aliens[cup as usize].worldx = 0;
    g.objs.aliens[cup as usize].worldy = 0;
    g.objs.aliens[cup as usize].worldz = 0;
    bossacupup_srou(&mut g, cup);
    let y1 = g.objs.aliens[cup as usize].worldy;
    assert_ne!(g.objs.aliens[cup as usize].worldx, 0);
    assert_ne!(y1, 0);
    bossacupup_srou(&mut g, cup);
    // Second chase moves further toward target (more negative from 0)
    assert!(g.objs.aliens[cup as usize].worldy <= y1);

    // UPlow uses −70≪scale (shallower than UP's −110)
    g.objs.aliens[cup as usize].worldy = 0;
    bossacupuplow_srou(&mut g, cup);
    let y_low = g.objs.aliens[cup as usize].worldy;
    assert_ne!(y_low, 0);
    // From same start, low lift chases a less-negative target → |y_low| < |y1| after one step
    assert!(y_low.abs() < y1.abs() || y_low > y1);
}
