//! ROM walkright / walker1/2 / duct + public wall/shou0/bholecoll aliases.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::bosses::{bholecoll_istrat, bholecoll_strat};
use sf_strat::enemies_ground::{
    duct_istrat, lwalker1_istrat, move_strat, rwalker1_istrat, shou0_istrat, shou0_strat,
    walker1_istrat, walker1_strat, walker2_istrat, walker2_strat, walking2_strat, walking_hit,
    walkright_istrat, wall1_strat, walll_istrat, wallnothit, wallr_istrat, wallright_strat,
    wallrnd_istrat, SH_NULL_SHAPE, SH_SMALL_EXPLOSION, SH_WALKER_LEFT, SH_WALKER_RIGHT,
};
use sf_strat::enemy_a::{COLLTYPE_ENEMY1, DEG45, DEG90};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    idx
}

#[test]
fn walkright_drifts_and_counts_down() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    walkright_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 29); // 30 then same-tick dec
    assert_eq!(g.objs.aliens[idx as usize].worldx, 20);
    assert_eq!(g.objs.aliens[idx as usize].roty, 0u8.wrapping_sub(DEG90));
    assert!(g.objs.aliens[idx as usize].worldy <= 0);
}

#[test]
fn walker1_fires_when_close_then_moves() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 500; // xz < 3000
    walker1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 5);
    assert_eq!(g.objs.aliens[idx as usize].vel, 10);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    walker1_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "HMISSILE1 spawned");
    // Now on move_strat.
    move_strat(&mut g, idx);
}

#[test]
fn lwalker_rwalker_set_heading() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let l = spawn_obj(&mut g);
    lwalker1_istrat(&mut g, l);
    assert_eq!(
        g.objs.aliens[l as usize].roty,
        0u8.wrapping_sub(DEG90.wrapping_add(DEG45))
    );
    let r = spawn_obj(&mut g);
    rwalker1_istrat(&mut g, r);
    assert_eq!(g.objs.aliens[r as usize].roty, DEG90.wrapping_add(DEG45));
}

#[test]
fn walker2_chases_x_in_tunnel() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.player_posx = 40;
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = -200; // clamp then chase
    walker2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 10);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    let z0 = g.objs.aliens[idx as usize].worldz;
    walker2_strat(&mut g, idx);
    // Clamped to Mtunnel_minx+30 = -60, then chased toward 40.
    assert!(g.objs.aliens[idx as usize].worldx > -200);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_sub(30));
}

#[test]
fn duct_is_nocoll_and_wall_aliases_work() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let d = spawn_obj(&mut g);
    duct_istrat(&mut g, d);
    assert_ne!(g.objs.aliens[d as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[d as usize].stratptr.is_none());

    let w = spawn_obj(&mut g);
    wallrnd_istrat(&mut g, w);
    assert!(g.objs.aliens[w as usize].stratptr.is_some());
    let wl = spawn_obj(&mut g);
    walll_istrat(&mut g, wl);
    let wr = spawn_obj(&mut g);
    wallr_istrat(&mut g, wr);
    // Public tick aliases callable.
    let w2 = spawn_obj(&mut g);
    g.objs.aliens[w2 as usize].roty = 128;
    wall1_strat(&mut g, w2);
    wallnothit(&mut g, w2);
    wallright_strat(&mut g, w2);
    let s0 = spawn_obj(&mut g);
    shou0_istrat(&mut g, s0);
    let s1 = spawn_obj(&mut g);
    shou0_strat(&mut g, s1);
    let wk = spawn_obj(&mut g);
    walking2_strat(&mut g, wk);
    let wh = spawn_obj(&mut g);
    walking_hit(&mut g, wh);
}

#[test]
fn walking_mech_uses_authored_topple_bodies_and_asymmetric_leg_effects() {
    const WALKER_STANDING: u16 = 26;
    const LEFT_LEG_HIT: u8 = 1;
    const RIGHT_LEG_HIT: u8 = 2;

    let mut right_game = Game::new();
    let right = spawn_obj(&mut right_game);
    right_game.objs.aliens[right as usize].shape = WALKER_STANDING;
    right_game.objs.aliens[right as usize].sbyte2 = 0;
    right_game.objs.aliens[right as usize].hitflags = LEFT_LEG_HIT;
    walking_hit(&mut right_game, right);
    assert_eq!(
        right_game.objs.aliens[right as usize].shape,
        SH_WALKER_RIGHT
    );
    assert!(right_game
        .objs
        .aliens
        .iter()
        .any(|alien| { alien.active && alien.shape == SH_SMALL_EXPLOSION && alien.hp == 0 }));

    let mut left_game = Game::new();
    let left = spawn_obj(&mut left_game);
    left_game.objs.aliens[left as usize].shape = WALKER_STANDING;
    left_game.objs.aliens[left as usize].sbyte3 = 0;
    left_game.objs.aliens[left as usize].hitflags = RIGHT_LEG_HIT;
    let before = left_game.objs.active_indices().len();
    walking_hit(&mut left_game, left);
    assert_eq!(left_game.objs.aliens[left as usize].shape, SH_WALKER_LEFT);
    assert_eq!(left_game.objs.active_indices().len(), before + 1);
    assert!(left_game
        .objs
        .aliens
        .iter()
        .any(|alien| { alien.active && alien.shape == SH_NULL_SHAPE && alien.hp == 0 }));
    assert!(!left_game
        .objs
        .aliens
        .iter()
        .any(|alien| { alien.active && alien.shape == SH_SMALL_EXPLOSION }));
}

#[test]
fn walking_mech_final_fall_uses_authored_explosion_mesh() {
    const WALKER_STANDING: u16 = 26;
    const LEFT_LEG_HIT: u8 = 1;
    const ACTIVE_HP: u8 = 1;
    const FALL_TICKS_AFTER_TRIGGER: usize = 13;

    let mut g = Game::new();
    let walker = spawn_obj(&mut g);
    g.objs.aliens[walker as usize].shape = WALKER_STANDING;
    g.objs.aliens[walker as usize].hp = ACTIVE_HP;
    g.objs.aliens[walker as usize].sbyte2 = 0;
    g.objs.aliens[walker as usize].hitflags = LEFT_LEG_HIT;
    walking_hit(&mut g, walker);
    let first_effect_pos = g
        .objs
        .aliens
        .iter()
        .find(|alien| alien.active && alien.shape == SH_SMALL_EXPLOSION)
        .map(|alien| (alien.worldx, alien.worldy, alien.worldz))
        .expect("right-fall flash");
    for tick_index in 0..FALL_TICKS_AFTER_TRIGGER {
        let tick = g.objs.aliens[walker as usize]
            .stratptr
            .expect("walker fall strategy");
        g.call_strat(tick, walker);
        if tick_index + 1 < FALL_TICKS_AFTER_TRIGGER {
            assert_ne!(g.objs.aliens[walker as usize].hp, 0);
        }
    }
    assert_eq!(g.objs.aliens[walker as usize].hp, 0);
    let effect_positions: Vec<_> = g
        .objs
        .aliens
        .iter()
        .filter(|alien| alien.active && alien.shape == SH_SMALL_EXPLOSION)
        .map(|alien| (alien.worldx, alien.worldy, alien.worldz))
        .collect();
    assert!(
        effect_positions.iter().any(|&position| position != first_effect_pos),
        "the cleaned-up right-fall flash is replaced by a new final body explosion: first={first_effect_pos:?}, active={effect_positions:?}"
    );
}

#[test]
fn bholecoll_public_spin() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    bholecoll_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    let rz0 = g.objs.aliens[idx as usize].rotz;
    // Far player — just spin.
    g.objs.aliens[idx as usize].worldz = 5000;
    bholecoll_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rz0.wrapping_add(12));
}
