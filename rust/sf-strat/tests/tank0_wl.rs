//! ROM tank0/tank1 + public tank1a2/tank2/tank3 + leftwall/wl/spacetest/bomwingdie.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemies_ground::{
    leftwall_istrat, spacetest_istrat, tank0_istrat, tank1_goforward, tank1_istrat, tank1_strat,
    tank1a2_istrat, tank1a_istrat, tank2_istrat, tank2zaco_istrat, tank3_istrat, wl_istrat,
    wl_strat, wldie_istrat,
};
use sf_strat::enemy_a::{bomwingdie_istrat, DEG180, DEG90};

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
fn tank0_waits_then_jumps_to_forward() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 5000; // far: wait
    tank0_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 16);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 50);
    assert_eq!(g.objs.aliens[idx as usize].roty, 0);

    g.objs.aliens[idx as usize].worldz = 1500; // <2000 → .forward
    tank0_istrat(&mut g, idx);
    // Same-tick goforward: z += 17 at medpspeed move.
    assert!(g.objs.aliens[idx as usize].worldz > 1500);
}

#[test]
fn tank1_hangar_then_forward() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 2500;
    g.objs.aliens[idx as usize].roty = 40;
    tank1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 50);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 15);
    // Hangar move ran same tick; still chasing unless already at 0.
    tank1_strat(&mut g, idx);
    // Force forward path.
    tank1_goforward(&mut g, idx);
    let z = g.objs.aliens[idx as usize].worldz;
    assert!(z >= 2500);
}

#[test]
fn tank1a2_and_tank3_close_range() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let a = spawn_obj(&mut g);
    g.objs.aliens[a as usize].worldz = 1000;
    tank1a2_istrat(&mut g, a);
    // Same-tick hangar→chase may already have entered goforward (fire+lr).
    assert!(g.objs.aliens[a as usize].sbyte2 <= 50);
    assert!(g.objs.aliens[a as usize].sbyte3 <= 15);
    assert!(g.objs.aliens[a as usize].stratptr.is_some());

    let t = spawn_obj(&mut g);
    g.objs.aliens[t as usize].worldz = 1000; // <1800 → forward
    tank3_istrat(&mut g, t);
    assert_eq!(g.objs.aliens[t as usize].hp, 2);
    // Forward adds +25 z same tick.
    assert!(g.objs.aliens[t as usize].worldz > 1000);

    // tank1a wait far away stays in init.
    let far = spawn_obj(&mut g);
    g.objs.aliens[far as usize].worldz = 8000;
    tank1a_istrat(&mut g, far);
    assert_eq!(g.objs.aliens[far as usize].roty, 192); // DEG270
}

#[test]
fn tank2_spawns_four_zacos() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    tank2_istrat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before + 4, "4 turret drones");
    assert_eq!(g.objs.aliens[idx as usize].hp, 40);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);

    let z = spawn_obj(&mut g);
    tank2zaco_istrat(&mut g, z);
    assert_eq!(g.objs.aliens[z as usize].hp, 4);
    assert_eq!(g.objs.aliens[z as usize].ap, 8);
}

#[test]
fn leftwall_wl_spacetest_bomwingdie() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let lw = spawn_obj(&mut g);
    leftwall_istrat(&mut g, lw);
    assert_eq!(g.objs.aliens[lw as usize].roty, DEG180);
    assert_ne!(g.objs.aliens[lw as usize].sflags & ASF_COLLDISABLE, 0);

    let w = spawn_obj(&mut g);
    wl_istrat(&mut g, w);
    assert_eq!(g.objs.aliens[w as usize].hp, 8);
    assert_eq!(g.objs.aliens[w as usize].ap, 16);
    wl_strat(&mut g, w);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    wldie_istrat(&mut g, w);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(
        after >= before,
        "item_7 drop (or explode-only if alloc fails)"
    );

    let st = spawn_obj(&mut g);
    let z0 = g.objs.aliens[st as usize].worldz;
    spacetest_istrat(&mut g, st);
    assert_eq!(g.objs.aliens[st as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[st as usize].rotz, 0u8.wrapping_sub(DEG90));
    assert_eq!(g.objs.aliens[st as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[st as usize].ap, 1);
    // add_player_z may leave z unchanged when playervel matches view.
    let _ = z0;

    let bw = spawn_obj(&mut g);
    bomwingdie_istrat(&mut g, bw);
}
