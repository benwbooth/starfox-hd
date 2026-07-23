//! ROM evader dodge + truck1/2 air trucks + truck_cont/col aliases.

use sf_game::alien::ASF_SHADOW;
use sf_game::Game;
use sf_strat::enemies_ground::{truck_cont, truckcol_istrat};
use sf_strat::enemy_a::{
    evader_cont, evader_init, evader_istrat, evader_strat, evadera_strat, truck1_istrat,
    truck1_strat, truck2_istrat, truck2_strat, COLLTYPE_ENEMY1, COLLTYPE_ZENEMY, DEG180,
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
    idx
}

#[test]
fn evader_istrat_picks_wp_and_enters_a() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    evader_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 8);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ZENEMY, 0);
    // newpos filled swp offsets from tables.
    let x = g.objs.aliens[idx as usize].swpx1;
    assert!([-400, 400, -100, 100].contains(&x));
    let y = g.objs.aliens[idx as usize].swpy1;
    assert!([-300, -200, -100, -50].contains(&y));
    let z = g.objs.aliens[idx as usize].sword1;
    assert!([1000, 800, 1600, 1200].contains(&z));
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn evadera_moves_toward_wp() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    evader_istrat(&mut g, idx);
    // Force a known WP near current pos so speed brakes.
    g.objs.aliens[idx as usize].swpx1 = 0;
    g.objs.aliens[idx as usize].swpy1 = -100;
    g.objs.aliens[idx as usize].sword1 = 2000; // player_z(0)+2000
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].vel = 50;
    let v0 = g.objs.aliens[idx as usize].vel;
    evadera_strat(&mut g, idx);
    // In range → speedto 0: vel should drop.
    assert!(g.objs.aliens[idx as usize].vel < v0 || g.objs.aliens[idx as usize].vel == 0);
}

#[test]
fn evader_init_aims_and_cont_scrolls() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].hp = 8;
    g.objs.aliens[idx as usize].worldz = 500;
    evader_init(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    let z0 = g.objs.aliens[idx as usize].worldz;
    g.vars.pviewvelz = 10;
    evader_cont(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_add(10));
}

#[test]
fn evader_strat_chases_aim() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].worldx = 100;
    g.objs.aliens[idx as usize].worldy = -40;
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    evader_init(&mut g, idx);
    let ry0 = g.objs.aliens[idx as usize].roty;
    g.vars.gameframe = 1; // not fire gate
    evader_strat(&mut g, idx);
    // Aim should have moved toward the player.
    assert_ne!(g.objs.aliens[idx as usize].roty, ry0);
}

#[test]
fn truck1_drifts_toward_camera() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    truck1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 16);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].count, 60);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    g.vars.pviewvelz = 0;
    let z0 = g.objs.aliens[idx as usize].worldz;
    truck1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_sub(35));
}

#[test]
fn truck2_weaves_and_advances_phase() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    truck2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 60);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 50);
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    truck2_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, s1.wrapping_add(6));
    assert_eq!(g.objs.aliens[idx as usize].vz, -35);
}

#[test]
fn truck_cont_and_col_are_public() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].sbyte1 = DEG180;
    g.objs.aliens[idx as usize].vel = 30;
    truck_cont(&mut g, idx);
    // Non-rail collide → hitflash path (no panic).
    g.objs.aliens[idx as usize].collobjptr = 0;
    truckcol_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].active);
}
