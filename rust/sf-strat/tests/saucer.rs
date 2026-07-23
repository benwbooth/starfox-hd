//! ROM saucer1 state machine + saucer bounce/circle.

use sf_game::alien::ASF_SHADOW;
use sf_game::Game;
use sf_strat::enemies_ground::{
    saucer1_istrat, saucer1_istrat2, saucer1_istrat3, saucer1_istrat4, saucer1_strat,
    saucer1_strat2, saucer1_strat3, saucer1_strat4, saucer_istrat, saucer_strat, saucer_strat2,
};
use sf_strat::enemy_a::ASF2_SMFLAG1;

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
fn saucer1_init_and_phases() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = -80;
    saucer1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 12);
    assert_eq!(g.objs.aliens[idx as usize].ap, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 2);
    assert_eq!(g.objs.aliens[idx as usize].sword1, 100);
    assert_eq!(g.objs.aliens[idx as usize].ptr as i16, -80);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);

    saucer1_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());

    // Face phase — init clears smflag1; same-tick strat2 may re-latch.
    let tick_before = g.objs.aliens[idx as usize].stratptr;
    saucer1_istrat2(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_ne!(g.objs.aliens[idx as usize].stratptr, tick_before);

    // Force already-aligned so face returns true → istrat3.
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SMFLAG1;
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].roty;
    g.objs.aliens[idx as usize].sbyte4 = g.objs.aliens[idx as usize].rotx;
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    saucer1_strat2(&mut g, idx);
    // Handoff into strat3 spins rotz += 10.
    assert_eq!(g.objs.aliens[idx as usize].rotz, rotz0.wrapping_add(10));

    // Peel
    saucer1_istrat4(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 0);
    // Same-tick strat4 may already have decremented lifecnt.
    assert!(g.objs.aliens[idx as usize].count <= 100);
    saucer1_strat4(&mut g, idx);
}

#[test]
fn saucer1_fire_exhausts_then_peels() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    saucer1_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].animframe = 6;
    g.objs.aliens[idx as usize].sbyte2 = 1; // last shot → peel
    g.vars.gameframe = 0;
    saucer1_strat3(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
    // istrat4 → strat4 same tick may dec lifecnt.
    assert!(g.objs.aliens[idx as usize].count <= 100);
    assert!(g.objs.aliens[idx as usize].count >= 99);
}

#[test]
fn saucer_bounces_then_circles() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -10; // closer to ground
    saucer_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 10);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].vz, 30);
    assert_eq!(g.objs.aliens[idx as usize].vy, -20);

    let mut entered = false;
    for _ in 0..200 {
        saucer_strat(&mut g, idx);
        if g.objs.aliens[idx as usize].vel == 40 && g.objs.aliens[idx as usize].sbyte2 == 16 {
            entered = true;
            break;
        }
    }
    assert!(entered, "should enter strat2 after bounce rest");

    let h0 = g.objs.aliens[idx as usize].sbyte1;
    saucer_strat2(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sbyte1, h0); // turned ±8
}
