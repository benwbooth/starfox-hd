//! ROM walkright / walker1/2 / duct + public wall/shou0/bholecoll aliases.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::bosses::{bholecoll_istrat, bholecoll_strat};
use sf_strat::enemies_ground::{
    duct_istrat, lwalker1_istrat, move_strat, rwalker1_istrat, shou0_istrat, shou0_strat,
    walker1_istrat, walker1_strat, walker2_istrat, walker2_strat, walking2_strat, walking_hit,
    walkright_istrat, wall1_strat, walll_istrat, wallnothit, wallr_istrat, wallright_strat,
    wallrnd_istrat,
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
