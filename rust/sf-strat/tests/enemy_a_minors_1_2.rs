//! Tick 158/173: AUDIT_ENEMY_A Minors #1–#2 — relslowlaser muzzle via
//! `rotate_8*` Roffs; relelaserhome lock latch |dz|<800.

use sf_game::alien::{ACF_COLLTYPE1, ACF_COLLTYPE4, ASF3_REALOBJ};
use sf_game::Game;
use sf_strat::enemy_a::{
    relelaserhome_strat, strat_fire_relslowlaser, strat_fire_relslowlaserhome,
    RELSLOWELASERHOME_LOCK_FLAG,
};
use sf_strat::snes_trig::strat_roffs_full_scaled;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = 0;
    al.worldy = -40;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

fn find_shot(g: &Game, firer: u16) -> u16 {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| a.active && *i as u16 != 0 && *i as u16 != firer)
        .map(|(i, _)| i as u16)
        .expect("projectile")
}

/// ROM muzzle: byte `80>>2=20`, rotate_8*, ASL×2 (not float identity +80).
fn muzzle_dz(rotx: u8, roty: u8, rotz: u8) -> i16 {
    strat_roffs_full_scaled(rotz, rotx, roty, 0, 0, 20, 2).2
}

/// Minor #1: fire_relslowElaser muzzle via scaled Roffs.
#[test]
fn relslowlaser_muzzle_z80_identity_rots() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let firer = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[firer as usize];
        al.worldx = 100;
        al.worldy = -50;
        al.worldz = 500;
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
    }
    strat_fire_relslowlaser(&mut g, firer, 0, 0);
    let shot = find_shot(&g, firer);
    let s = &g.objs.aliens[shot as usize];
    let expect_z = 500i16.wrapping_add(muzzle_dz(0, 0, 0));
    assert_eq!(s.worldx, 100);
    assert_eq!(s.worldy, -50);
    assert_eq!(s.worldz, expect_z, "identity Roffs Z");
    assert_ne!(s.collflags & ACF_COLLTYPE1, 0, "laser");
    assert_ne!(s.collflags & ACF_COLLTYPE4, 0, "enemyweap");
}

/// Minor #1: yaw 180 flips the forward muzzle (local +Z → world −Z).
#[test]
fn relslowlaser_muzzle_flips_with_yaw180() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let firer = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[firer as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 1000;
        al.rotx = 0;
        al.roty = 128; // DEG180
        al.rotz = 0;
    }
    strat_fire_relslowlaser(&mut g, firer, 0, 128);
    let shot = find_shot(&g, firer);
    let s = &g.objs.aliens[shot as usize];
    let expect_z = 1000i16.wrapping_add(muzzle_dz(0, 128, 0));
    assert_eq!(s.worldx, 0);
    assert_eq!(s.worldy, 0);
    assert_eq!(s.worldz, expect_z, "yaw180 flips forward muzzle");
    assert!(expect_z < 1000, "must move toward −Z");
}

/// Minor #1: home helper also places the scaled muzzle.
#[test]
fn relslowlaserhome_muzzle_z80() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let firer = spawn(&mut g);
    g.objs.aliens[firer as usize].worldz = 200;
    strat_fire_relslowlaserhome(&mut g, firer, 0, 0);
    let shot = find_shot(&g, firer);
    let expect_z = 200i16.wrapping_add(muzzle_dz(0, 0, 0));
    assert_eq!(g.objs.aliens[shot as usize].worldz, expect_z);
    assert_ne!(g.objs.aliens[shot as usize].collflags & ACF_COLLTYPE1, 0);
}

/// Minor #2: lock latches only when |dz| < 800 (not at 800).
#[test]
fn relelaserhome_lock_strict_less_than_800() {
    // |dz| == 800 → no lock
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        let shot = spawn(&mut g);
        g.objs.aliens[shot as usize].worldz = 800;
        g.objs.aliens[shot as usize].count = 40;
        g.objs.aliens[shot as usize].sflags2 = 0;
        relelaserhome_strat(&mut g, shot);
        assert_eq!(
            g.objs.aliens[shot as usize].sflags2 & RELSLOWELASERHOME_LOCK_FLAG,
            0,
            "|dz|==800 must NOT latch"
        );
    }
    // |dz| == 799 → lock
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        let shot = spawn(&mut g);
        g.objs.aliens[shot as usize].worldz = 799;
        g.objs.aliens[shot as usize].count = 40;
        g.objs.aliens[shot as usize].sflags2 = 0;
        relelaserhome_strat(&mut g, shot);
        assert_ne!(
            g.objs.aliens[shot as usize].sflags2 & RELSLOWELASERHOME_LOCK_FLAG,
            0,
            "|dz|==799 must latch"
        );
    }
}
