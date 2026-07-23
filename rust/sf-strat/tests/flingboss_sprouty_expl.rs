//! Tick 214: flingboss `sprouty.expl` via shared fall-chain
//! (DSTRATS.ASM:3403-3414 / 3650-3657) + `chicken_arm_init` ENEMY1=0x10.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::{chicken_arm_init, flingboss_pullthearmsoff, strat_flingboss_init};

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = 0;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("o");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldy = -50;
    g.objs.aliens[idx as usize].worldz = 800;
    idx
}

/// pullthearmsoff: each arm root explodes; mother links cleared.
#[test]
fn pullthearmsoff_explodes_arm_roots() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let mother = spawn_obj(&mut g);
    strat_flingboss_init(&mut g, mother);
    // Drive a few ticks so arms exist.
    for _ in 0..5 {
        g.run_strategies();
    }
    let ptr = g.objs.aliens[mother as usize].ptr;
    let sw1 = g.objs.aliens[mother as usize].sword1;
    assert_ne!(ptr, 0, "arm1 linked");
    assert_ne!(sw1, 0, "arm2 linked");
    let a1 = (ptr - 1) as u16;
    let a2 = (sw1 as u16).wrapping_sub(1);
    assert!(g.objs.aliens[a1 as usize].active);
    assert!(g.objs.aliens[a2 as usize].active);

    flingboss_pullthearmsoff(&mut g, mother);

    assert_eq!(g.objs.aliens[mother as usize].ptr, 0);
    assert_eq!(g.objs.aliens[mother as usize].sword1, 0);
    // Roots armed for explode (hp 0); mother must stay alive (no nested aldead).
    assert!(g.objs.aliens[mother as usize].active);
    assert_eq!(g.objs.aliens[a1 as usize].hp, 0);
    assert_eq!(g.objs.aliens[a2 as usize].hp, 0);
}

/// Manual two-segment chain: root explodes, child keeps fall_istrat.
#[test]
fn pullthearmsoff_chain_child_falls() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let mother = spawn_obj(&mut g);
    let root = spawn_obj(&mut g);
    let child = spawn_obj(&mut g);
    g.objs.aliens[root as usize].hp = 10;
    g.objs.aliens[child as usize].hp = 10;
    g.objs.aliens[root as usize].ptr = child + 1;
    g.objs.aliens[mother as usize].ptr = root + 1;
    g.objs.aliens[mother as usize].sword1 = 0;

    flingboss_pullthearmsoff(&mut g, mother);

    assert_eq!(g.objs.aliens[root as usize].hp, 0);
    assert_eq!(g.objs.aliens[child as usize].hp, 10);
    assert!(g.objs.aliens[child as usize].stratptr.is_some());
    // Run deferred fall_istrat.
    if let Some(s) = g.objs.aliens[child as usize].stratptr {
        g.call_strat(s, child);
    }
    assert_eq!(g.objs.aliens[child as usize].vel, 30);
}

/// Shared arm_istrat init uses ROM ENEMY1 (= 0x10).
#[test]
fn chicken_arm_init_sets_enemy1_colltype2() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_obj(&mut g);
    chicken_arm_init(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].collflags & 0x10, 0);
    assert_eq!(g.objs.aliens[idx as usize].collflags & 0x01, 0);
}
