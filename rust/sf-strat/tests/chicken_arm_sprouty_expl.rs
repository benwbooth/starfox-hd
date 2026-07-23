//! Tick 213: chicken arm `sprouty.expl` → `.fall_istrat` chain
//! (DSTRATS.ASM:2348-2381) — was instant unlink/remove; now children fall
//! then explode on land. Also verifies `chick_istrat` ENEMY1 = 0x10.

use sf_game::alien::{ASF3_REALOBJ, ASF_SHADOW};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses::{
    chick_istrat, chicken_arm_fall_istrat, chicken_arm_fall_strat, chicken_arm_sprouty_expl,
};

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

fn spawn_seg(g: &mut Game, y: i16) -> u16 {
    let idx = g.objs.alloc().expect("seg");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = 0;
    al.worldy = y;
    al.worldz = 500;
    al.hp = 10;
    idx
}

/// Head explodes; child keeps fall_istrat for next tick.
#[test]
fn sprouty_expl_head_explodes_child_falls() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let head = spawn_seg(&mut g, -80);
    let child = spawn_seg(&mut g, -40);
    g.objs.aliens[head as usize].ptr = child + 1; // index+1 encoding
    chicken_arm_sprouty_expl(&mut g, head);
    // Head armed for explode (hp 0); detonation deferred (no nested aldead).
    assert_eq!(g.objs.aliens[head as usize].hp, 0);
    assert!(g.objs.aliens[head as usize].active);
    assert!(g.objs.aliens[head as usize].expstratptr.is_some());
    // Child still active, wired to fall_istrat (not yet run).
    assert!(g.objs.aliens[child as usize].active);
    assert!(g.objs.aliens[child as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[child as usize].hp, 10);
    // Run fall_istrat → vel 30, remove_alptrs, first fall tick.
    if let Some(s) = g.objs.aliens[child as usize].stratptr {
        g.call_strat(s, child);
    }
    assert_eq!(g.objs.aliens[child as usize].vel, 30);
    assert!(g.objs.aliens[child as usize].active);
    assert_ne!(g.objs.aliens[child as usize].hp, 0);
    // Parent ptrs at child cleared by remove_alptrs in fall_istrat.
    assert_eq!(g.objs.aliens[head as usize].ptr, 0);
}

/// Fall spins and detonates when worldy reaches ground.
#[test]
fn arm_fall_explodes_on_landing() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_seg(&mut g, -5);
    chicken_arm_fall_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 30);
    // Force near-ground and drive until explode.
    g.objs.aliens[idx as usize].worldy = -1;
    g.objs.aliens[idx as usize].vy = 20;
    let mut exploded = false;
    for _ in 0..20 {
        if g.objs.aliens[idx as usize].hp == 0 {
            exploded = true;
            break;
        }
        chicken_arm_fall_strat(&mut g, idx);
    }
    assert!(exploded, "fall lands → explode");
}

/// Fall spins rotz/rotx each tick while airborne.
#[test]
fn arm_fall_spins_while_airborne() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_seg(&mut g, -200);
    g.objs.aliens[idx as usize].rotz = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    chicken_arm_fall_istrat(&mut g, idx);
    // Istrat falls through one `.fall` tick: +5/+2.
    assert_eq!(g.objs.aliens[idx as usize].rotz, 5);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 2);
    chicken_arm_fall_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 10);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 4);
}

/// chick_istrat uses ROM ENEMY1 (= ACF_COLLTYPE2 / 0x10), not vars 0x01.
#[test]
fn chick_istrat_sets_enemy1_colltype2() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = spawn_seg(&mut g, -40);
    chick_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & 0x10, 0);
    assert_eq!(g.objs.aliens[idx as usize].collflags & 0x01, 0);
}
