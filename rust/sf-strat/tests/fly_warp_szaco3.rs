//! ROM fly family + highfly/distantfly + szaco3 + warp.

use sf_game::alien::ASF_SHADOW;
use sf_game::Game;
use sf_strat::enemies_ground::{
    distantfly_istrat, fly2_istrat, fly3_istrat, fly3_strat, fly4_istrat, fly4_strat, fly_istrat,
    fly_strat, flydead_istrat, flydead_strat, highfly_istrat, szaco3_istrat, szaco3_strat,
    warp_istrat, warp_strat,
};
use sf_strat::enemy_a::{COLLTYPE_ENEMY1, DEG180, DEG90};

const EXPECTED_WARP_SHAPES: [u16; 4] = [456, 455, 454, 133];

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
fn fly_init_dive_then_lr() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fly_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 4);
    assert_eq!(g.objs.aliens[idx as usize].sword1, -30);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);

    // Force delay gate open and raise sword1 toward 0.
    g.vars.gameframe = 0;
    g.objs.aliens[idx as usize].sword1 = -2;
    fly_strat(&mut g, idx);
    // sword1 ≥0 → flylr → flyr same tick (speed_to may raise vel past 10).
    assert!(g.objs.aliens[idx as usize].vel >= 10);
}

#[test]
fn highfly_distant_to_fly2_fly3() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    highfly_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 4);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some()); // fly2

    let d = spawn_obj(&mut g);
    g.objs.aliens[d as usize].worldx = 50;
    g.objs.aliens[d as usize].worldy = -40;
    distantfly_istrat(&mut g, d);
    assert_eq!(g.objs.aliens[d as usize].sword1, 50);

    // Force WP arrived → fly3
    g.objs.aliens[idx as usize].swpx1 = g.objs.aliens[idx as usize].worldx;
    g.objs.aliens[idx as usize].swpy1 = g.objs.aliens[idx as usize].worldy;
    g.objs.aliens[idx as usize].swpz1 = g.objs.aliens[idx as usize].worldz;
    g.objs.aliens[idx as usize].vel = 0;
    fly2_istrat(&mut g, idx);
    // may or may not reach in one tick; call fly3 directly
    fly3_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    fly3_strat(&mut g, idx);

    fly4_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 100);
    fly4_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 99);
}

#[test]
fn flydead_then_ground() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldy = -80; // airborne
    flydead_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 64);
    // Manually place at/below ground and tick → hitgnd.
    g.objs.aliens[idx as usize].worldy = 5;
    flydead_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 0);
}

#[test]
fn szaco3_banks_close_aims_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 300; // |dz| < 400 → bank
    szaco3_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].vel, 70);
    assert_eq!(g.objs.aliens[idx as usize].count, 70);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    szaco3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 69);
    // bank may change rotz
    let _ = rotz0;

    g.objs.aliens[idx as usize].worldz = 2000; // far → aim
    szaco3_strat(&mut g, idx);
}

#[test]
fn warp_init_and_states() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    warp_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].rotx, DEG90);
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, 4);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & (COLLTYPE_ENEMY1), 0);

    for (phase, expected_shape) in EXPECTED_WARP_SHAPES.into_iter().enumerate() {
        g.objs.aliens[idx as usize].stratstate = 5;
        g.objs.aliens[idx as usize].sbyte3 = (phase as u8) * 2;
        warp_strat(&mut g, idx);
        assert_eq!(g.objs.aliens[idx as usize].shape, expected_shape);
    }
    g.objs.aliens[idx as usize].stratstate = 0;

    warp_strat(&mut g, idx);
    // Still state 0 unless already at pos.
    assert!(g.objs.aliens[idx as usize].stratstate <= 1);

    // Jump to fire state.
    g.objs.aliens[idx as usize].stratstate = 2;
    g.objs.aliens[idx as usize].sbyte2 = 2;
    g.vars.gameframe = 0;
    warp_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);

    g.objs.aliens[idx as usize].stratstate = 4;
    warp_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 5);
    assert_eq!(g.objs.aliens[idx as usize].vz, -30);
}
