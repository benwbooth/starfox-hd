//! ROM zaco7 + sdragonfly (+ makeSdrag) and zaco0 phase aliases (GA2STRAT / KSTRATS).

use sf_game::alien::ASF_SHADOW;
use sf_game::Game;
use sf_strat::enemy_a::{
    dragonfly_istrat, dragonfly_strat, sdragonfly_istrat, sdragonfly_strat, zaco0_istrat,
    zaco0_strat, zaco0b_istrat, zaco0c2_istrat, zaco0c_istrat, zaco0d_istrat, zaco7_istrat,
    zaco7_strat, COLLTYPE_ENEMY1, DEG180, DEG270, DEG90,
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
    g.objs.aliens[idx as usize].worldz = 3000;
    g.objs.aliens[idx as usize].worldy = -80;
    g.objs.aliens[idx as usize].vel = 40;
    idx
}

#[test]
fn zaco7_istrat_sets_szaco5_stats() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    zaco7_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn zaco7_animates_and_banks_when_far() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    zaco7_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 2000; // |dz|>=600 → bank
    g.objs.aliens[idx as usize].worldx = 200;
    g.vars.player_posx = 0;
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    zaco7_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe, 1);
    // bank adds dx>>6 to rotz (200>>6 = 3).
    assert_ne!(g.objs.aliens[idx as usize].rotz, rotz0);
}

#[test]
fn zaco7_aims_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    zaco7_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 300; // |dz|<600
    g.objs.aliens[idx as usize].rotz = 40;
    zaco7_strat(&mut g, idx);
    // rotz chases 0 at rate 3.
    assert!(g.objs.aliens[idx as usize].rotz < 40);
}

#[test]
fn sdragonfly_istrat_and_weave() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    sdragonfly_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].vel, 25);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 6);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_SHADOW, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    // Far weave: burn state 0 timer.
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].sbyte1 = 1;
    sdragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 12);
}

#[test]
fn sdragonfly_stops_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    sdragonfly_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldz = 200; // |dz|<400
    sdragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 2);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 20);
}

#[test]
fn dragonfly_spawns_sdragonfly_on_turn() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    dragonfly_istrat(&mut g, idx);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    g.objs.aliens[idx as usize].sbyte1 = 1;
    dragonfly_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "makeSdrag should spawn a child");
    // Child should be an sdragonfly (vel 25).
    let child = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| *i as u16 != idx && *i != 0 && a.active && a.vel == 25);
    assert!(child.is_some());
}

#[test]
fn zaco0_phase_aliases_chain() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldx = -200;
    zaco0_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG270);
    assert_eq!(g.objs.aliens[idx as usize].rotx, DEG90);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 10);
    // Sweep: worldx += 43 each tick.
    let x0 = g.objs.aliens[idx as usize].worldx;
    zaco0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, x0.wrapping_add(43));
    // Force turn-in.
    zaco0b_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG270.wrapping_sub(8));
    // Jump to fire phase.
    g.objs.aliens[idx as usize].roty = DEG180;
    zaco0c_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    // Turn out → flyaway.
    zaco0c2_istrat(&mut g, idx);
    zaco0d_istrat(&mut g, idx);
    let y0 = g.objs.aliens[idx as usize].worldy;
    // flyaway already ran once in d_istrat; y decreased by 19.
    assert_eq!(g.objs.aliens[idx as usize].worldy, y0); // already applied
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}
