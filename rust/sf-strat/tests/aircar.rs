//! ROM aircar1–5 colony air cars (GA2STRAT.ASM:2265-2490).

use sf_game::alien::ASF_SHADOW;
use sf_game::Game;
use sf_strat::enemy_a::{
    aircar1_istrat, aircar1_strat, aircar2_istrat, aircar2_strat, aircar3_istrat, aircar3_strat,
    aircar4_istrat, aircar4_strat, aircar5_istrat, aircar5_strat, ASF2_SFLAG1, COLLTYPE_ENEMY1,
    DEG90, MEDPSPEED_I16,
};

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
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = -200;
    idx
}

#[test]
fn aircar1_skids_then_stops() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    aircar1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 10);
    assert_eq!(g.objs.aliens[idx as usize].ap, 10);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 25);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 0u8.wrapping_sub(DEG90));
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    let s0 = g.objs.aliens[idx as usize].sbyte1;
    aircar1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, s0 - 1);
    // Drain to stop.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    aircar1_strat(&mut g, idx);
    // Stop path: colanim bit set (init_colanim #1 → 0x81).
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 1);
}

#[test]
fn aircar2_drops_barrier_and_peels() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    aircar2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 50);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);
    // Force barrier drop.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    aircar2_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].vz, -30);
    // State 1 peels in the same tick after next_state (vx -= 1 from 0).
    assert_eq!(g.objs.aliens[idx as usize].vx, -1);
    aircar2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, -2);
}

#[test]
fn aircar3_weaves_then_climbs() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    aircar3_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 40);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    // Force state0 → state1.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    aircar3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    // Force weave complete → state2 (state2 body also runs same tick).
    g.objs.aliens[idx as usize].sword1 = 256 + 63;
    aircar3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].count, 39); // lifecnt 40 then same-tick dec
    assert_eq!(g.objs.aliens[idx as usize].vz, 12);
}

#[test]
fn aircar4_speeds_when_player_near() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    aircar4_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 60);
    assert_eq!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    // Far: no speedup latch.
    g.objs.aliens[idx as usize].worldz = 2000;
    aircar4_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    // Near: latch sflag1 and speed toward medpspeed+15.
    g.objs.aliens[idx as usize].worldz = 200;
    g.objs.aliens[idx as usize].vel = 0;
    aircar4_strat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    assert!(g.objs.aliens[idx as usize].vel > 0);
    assert!(g.objs.aliens[idx as usize].vel <= MEDPSPEED_I16 as u8 + 15);
}

#[test]
fn aircar5_hits_wall_and_tumbles() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    aircar5_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, DEG90);
    assert_eq!(g.objs.aliens[idx as usize].vel, 40);
    // Approach wall.
    g.objs.aliens[idx as usize].worldx = 100; // >= colony_maxX-20 (100)
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    aircar5_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].vx, -5);
    // State1 runs same tick; gameframe 0 opens notdelay→beqdec → 15.
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 15);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before);
    aircar5_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
}
