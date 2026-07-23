//! ROM suckbits/cube + lseqdoor + volrock/plasma/down + tree3 leaves.

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemies_ground::{
    lseqdoor1_istrat, lseqdoor2_istrat, tree3_istrat, volplasma_istrat, volrock_istrat,
    volrock_strat, volrockdown_istrat, volrockdown_strat,
};
use sf_strat::enemy_a::{
    suckbits_cont, suckbits_istrat, suckcube_istrat, suckcube_strat, suckobj_srou,
    suckobjfast_srou, ASF2_SFLAG1, COLLTYPE_ENEMY1, DEG45, DEG90,
};

const WM_GAMEFLAGS2: u16 = 0x155C;
const GF2_STRATFLAG1: u8 = 1;
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
    g.vars.set_sv_i16(sv::VIEWTOOBJ, 0);
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 400;
    idx
}

#[test]
fn suck_bits_cube_and_helpers() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let bits = spawn_obj(&mut g);
    suckbits_istrat(&mut g, bits);
    assert_eq!(g.objs.aliens[bits as usize].count, 6);
    assert_ne!(g.objs.aliens[bits as usize].sflags & ASF_COLLDISABLE, 0);
    let x0 = g.objs.aliens[bits as usize].worldx;
    let z0 = g.objs.aliens[bits as usize].worldz;
    suckbits_cont(&mut g, bits);
    assert!(g.objs.aliens[bits as usize].worldx.abs() < x0.abs() || x0 == 0);
    assert_eq!(
        g.objs.aliens[bits as usize].worldz,
        z0.wrapping_add(60) // +playerZ(0)
    );
    assert_eq!(g.objs.aliens[bits as usize].count, 5);
    // Drain life → remove on transition 1→0
    g.objs.aliens[bits as usize].count = 1;
    g.objs.aldead = 0;
    suckbits_cont(&mut g, bits);
    assert_eq!(g.objs.aliens[bits as usize].count, 0);
    assert_eq!(g.objs.aldead, 1);

    let cube = spawn_obj(&mut g);
    suckcube_istrat(&mut g, cube);
    assert_eq!(g.objs.aliens[cube as usize].count, 20);
    let rx = g.objs.aliens[cube as usize].rotx;
    suckcube_strat(&mut g, cube);
    assert_eq!(g.objs.aliens[cube as usize].rotx, rx.wrapping_add(12));

    let other = spawn_obj(&mut g);
    g.objs.aliens[other as usize].worldx = 800;
    g.objs.aliens[other as usize].worldy = 0;
    g.objs.aliens[other as usize].worldz = 1000;
    suckobj_srou(&mut g, other, 500);
    assert!(g.objs.aliens[other as usize].worldx.abs() < 800);
    assert!(g.objs.aliens[other as usize].worldy > 0); // toward 280

    let fast = spawn_obj(&mut g);
    g.objs.aliens[fast as usize].worldz = 100;
    g.objs.aliens[fast as usize].vx = 9;
    suckobjfast_srou(&mut g, fast);
    assert_eq!(g.objs.aliens[fast as usize].vx, 0);
    assert_eq!(g.objs.aliens[fast as usize].worldz, 150);
}

#[test]
fn lseqdoor_and_tree3() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let d1 = spawn_obj(&mut g);
    // Far: closed.
    g.objs.aliens[d1 as usize].worldz = 2000;
    g.objs.aliens[d1 as usize].animframe = 5;
    lseqdoor1_istrat(&mut g, d1);
    assert_eq!(g.objs.aliens[d1 as usize].animframe, 0);
    assert_ne!(g.objs.aliens[d1 as usize].sflags & ASF_COLLDISABLE, 0);
    // Close (|dz|<585): open anim.
    g.objs.aliens[d1 as usize].worldz = 100;
    g.objs.aliens[d1 as usize].animframe = 0;
    lseqdoor1_istrat(&mut g, d1);
    assert_eq!(g.objs.aliens[d1 as usize].animframe, 1);

    let d2 = spawn_obj(&mut g);
    g.objs.aliens[d2 as usize].worldy = -50;
    g.objs.aliens[0].worldy = -60; // viewtoobj=0 → |dy|=10 < 200
    g.vars.write_ext8(WM_GAMEFLAGS2, 0);
    lseqdoor2_istrat(&mut g, d2);
    assert_ne!(g.objs.aliens[d2 as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_ne!(g.vars.read_ext8(WM_GAMEFLAGS2) & GF2_STRATFLAG1, 0);
    assert_eq!(g.objs.aliens[d2 as usize].animframe & 0x7F, 1);

    let t3 = spawn_obj(&mut g);
    g.objs.aliens[t3 as usize].worldx = 100; // right of player → -deg45
    tree3_istrat(&mut g, t3);
    assert_eq!(g.objs.aliens[t3 as usize].sbyte1, 255);
    assert_eq!(g.objs.aliens[t3 as usize].roty, 0u8.wrapping_sub(DEG45));
}

#[test]
fn volrock_plasma_down_public() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let rock = spawn_obj(&mut g);
    volrock_istrat(&mut g, rock);
    assert_eq!(g.objs.aliens[rock as usize].hp, 2);
    assert_ne!(g.objs.aliens[rock as usize].collflags & COLLTYPE_ENEMY1, 0);
    assert!(g.objs.aliens[rock as usize].vy < 0); // upward launch
    let vy0 = g.objs.aliens[rock as usize].vy;
    volrock_strat(&mut g, rock);
    // Gravity +2 applied by falldown before move.
    assert_eq!(g.objs.aliens[rock as usize].vy, vy0.wrapping_add(2));

    let plasma = spawn_obj(&mut g);
    volplasma_istrat(&mut g, plasma);
    assert_eq!(g.objs.aliens[plasma as usize].hp, 2);
    assert_eq!(g.objs.aliens[plasma as usize].vel, 50);
    assert_eq!(g.objs.aliens[plasma as usize].rotx, (-(DEG90 as i8)) as u8);

    let down = spawn_obj(&mut g);
    g.objs.aliens[down as usize].worldy = -200;
    volrockdown_istrat(&mut g, down);
    assert_eq!(g.objs.aliens[down as usize].hp, 2);
    // Force ground hit → state 1
    g.objs.aliens[down as usize].stratstate = 0;
    g.objs.aliens[down as usize].worldy = 10;
    g.objs.aliens[down as usize].vy = 80;
    volrockdown_strat(&mut g, down);
    assert_eq!(g.objs.aliens[down as usize].stratstate, 1);
}
