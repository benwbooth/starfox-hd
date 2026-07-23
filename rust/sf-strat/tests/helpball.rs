//! ROM `helpball` / `helpballhome` / Hcoll / Hrem (GSTRATS.ASM).

use sf_game::alien::{ASF3_LOCKON, ASF3_REALOBJ, ASF_COLLDISABLE, ASF_NOHITAFFECT};
use sf_game::draw::AF_INVIEW_PL;
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemy_a::{
    helpball_hcoll_istrat, helpball_hrem_istrat, helpball_istrat, helpball_strat,
    helpballhome_istrat,
};
use sf_strat::snes_trig::strat_roffs_roll;

fn spawn_player(g: &mut Game) -> u16 {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[p as usize].worldx = 0;
    g.objs.aliens[p as usize].worldy = 0;
    g.objs.aliens[p as usize].worldz = 0;
    g.objs.aliens[p as usize].sflags3 |= ASF3_REALOBJ;
    p
}

fn spawn_enemy(g: &mut Game, x: i16, z: i16, hp: u8) -> u16 {
    let e = g.objs.alloc().expect("enemy");
    let al = &mut g.objs.aliens[e as usize];
    al.worldx = x;
    al.worldz = z;
    al.hp = hp;
    al.flags |= AF_INVIEW_PL;
    al.sflags3 |= ASF3_REALOBJ;
    e
}

#[test]
fn helpball_istrat_sets_orbit_radius_and_colldisable() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let idx = g.objs.alloc().expect("hb");
    helpball_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 30);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

/// Orbit pos = player + `rotate_8yx(rotz, #0, radius)` with z=#60 (flags 0,0,1).
#[test]
fn helpball_orbit_uses_rotate8yx_roll() {
    let mut g = Game::new();
    let p = spawn_player(&mut g);
    g.objs.aliens[p as usize].worldx = 100;
    g.objs.aliens[p as usize].worldy = -50;
    g.objs.aliens[p as usize].worldz = 200;
    let hb = g.objs.alloc().expect("hb");
    helpball_istrat(&mut g, hb);
    g.objs.aliens[hb as usize].rotz = 64; // 90°
    g.objs.aliens[hb as usize].sbyte3 = 30;
    // No valid target nearby → still orbits, no home spawn required.
    helpball_strat(&mut g, hb);
    // strat adds rotz+=12 after Roffs, so orbit used entry rotz=64.
    let (dx, dy, dz) = strat_roffs_roll(64, 0, 30, 60);
    assert_eq!(g.objs.aliens[hb as usize].worldx, 100i16.wrapping_add(dx));
    assert_eq!(g.objs.aliens[hb as usize].worldy, (-50i16).wrapping_add(dy));
    assert_eq!(g.objs.aliens[hb as usize].worldz, 200i16.wrapping_add(dz));
    assert_eq!(g.objs.aliens[hb as usize].rotz, 64u8.wrapping_add(12));
}

#[test]
fn helpball_orbits_and_spawns_home_on_valid_target() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let enemy = spawn_enemy(&mut g, 500, 0, 10);
    let hb = g.objs.alloc().expect("hb");
    helpball_istrat(&mut g, hb);

    helpball_strat(&mut g, hb);
    assert_eq!(
        g.objs.aliens[enemy as usize].sflags3 & ASF3_LOCKON,
        ASF3_LOCKON
    );
    assert_eq!(g.objs.aliens[hb as usize].sbyte1, 1);
    assert_eq!(g.objs.aliens[hb as usize].sbyte2, 1);
    let home = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| {
            *i as u16 != hb && *i as u16 != enemy && *i as u16 != 0 && a.active && a.ap == 20
        })
        .map(|(i, _)| i as u16)
        .expect("home shot");
    assert_eq!(g.objs.aliens[home as usize].ptr, enemy.wrapping_add(1));
    assert_eq!(g.objs.aliens[home as usize].sword1, hb as i16);
    assert_eq!(g.objs.aliens[home as usize].vel, 40);
    assert_eq!(g.objs.aliens[home as usize].count, 70);
    assert_eq!(g.objs.aliens[home as usize].shape, 406);
}

#[test]
fn helpball_skips_locked_friend_hard_and_nohit() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let hard = spawn_enemy(&mut g, 400, 0, HARD_HP);

    let friend = spawn_enemy(&mut g, 450, 0, 5);
    g.objs.aliens[friend as usize].collflags |= 0x80; // ACF_COLLTYPE5 friend

    let nohit = spawn_enemy(&mut g, 480, 0, 5);
    g.objs.aliens[nohit as usize].sflags |= ASF_NOHITAFFECT;

    let hb = g.objs.alloc().expect("hb");
    helpball_istrat(&mut g, hb);
    helpball_strat(&mut g, hb);
    assert_eq!(g.objs.aliens[hb as usize].sbyte1, 0);
    assert_eq!(g.objs.aliens[hard as usize].sflags3 & ASF3_LOCKON, 0);
}

#[test]
fn helpballhome_rem_clears_lockon_and_decs_mother() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let enemy = spawn_enemy(&mut g, 100, 0, 5);
    g.objs.aliens[enemy as usize].sflags3 |= ASF3_LOCKON;
    let hb = g.objs.alloc().expect("hb");
    g.objs.aliens[hb as usize].sbyte1 = 2;
    let home = g.objs.alloc().expect("home");
    g.objs.aliens[home as usize].ptr = enemy.wrapping_add(1);
    g.objs.aliens[home as usize].sword1 = hb as i16;
    helpball_hrem_istrat(&mut g, home);
    assert_eq!(g.objs.aliens[enemy as usize].sflags3 & ASF3_LOCKON, 0);
    assert_eq!(g.objs.aliens[hb as usize].sbyte1, 1);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn helpball_hcoll_only_hurts_when_partner_is_target() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let target = spawn_enemy(&mut g, 200, 0, 5);
    let other = spawn_enemy(&mut g, 300, 0, 5);
    let home = g.objs.alloc().expect("h");
    helpballhome_istrat(&mut g, home);
    g.objs.aliens[home as usize].ptr = target.wrapping_add(1);
    g.objs.aliens[home as usize].collobjptr = other;
    g.objs.aliens[home as usize].sflags2 |= 0x10; // skip missbound
    helpball_hcoll_istrat(&mut g, home);
    assert_eq!(g.objs.aliens[home as usize].hp, 1);

    g.objs.aliens[home as usize].collobjptr = target;
    helpball_hcoll_istrat(&mut g, home);
    assert!(g.objs.aliens[home as usize].hp == 0 || g.objs.aldead == 1);
}

#[test]
fn helpball_expires_after_ten_shots_and_radius_grow() {
    let mut g = Game::new();
    let _p = spawn_player(&mut g);
    let hb = g.objs.alloc().expect("hb");
    helpball_istrat(&mut g, hb);
    g.objs.aliens[hb as usize].sbyte2 = 10;
    g.objs.aliens[hb as usize].sbyte3 = 117;
    helpball_strat(&mut g, hb);
    assert_eq!(g.objs.aliens[hb as usize].sbyte3, 120);
    helpball_strat(&mut g, hb);
    assert_eq!(g.objs.aldead, 1);
}
