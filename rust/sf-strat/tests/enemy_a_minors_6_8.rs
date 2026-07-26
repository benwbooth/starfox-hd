//! Tick 160: AUDIT_ENEMY_A Minors #6–#8 — zacos muzzle Z120; clship flyin
//! sflag1 gated by notdelay; zaco2loop HMISSILE1 unconditional on level!=1.

use sf_game::alien::{ASF3_REALOBJ, ASF_INVISIBLE, ATMISSILE};
use sf_game::Game;
use sf_strat::enemy_a::{
    frame_tick_mod, strat_clship_turna_init, strat_clship_turnb_init, wm, zaco2_istrat,
    zacos2_init, SH_MISSILE,
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

fn run(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("strat");
    g.call_strat(s, idx);
}

const CLSHIP_FLAG1: u8 = 0x10;

/// Minor #6: zacos2_init muzzle = elaserfireZoff + weapon_pos → world +120.
#[test]
fn zacos_muzzle_is_weapon_pos_plus_elaserfirezoff() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 50;
        al.worldy = -20;
        al.worldz = 1000;
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
    }
    zacos2_init(&mut g, idx);
    let shot = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| a.active && *i as u16 != 0 && *i as u16 != idx)
        .expect("laser");
    assert_eq!(shot.1.worldx, 50);
    assert_eq!(shot.1.worldy, -20);
    // ROM: (80+40)>>2 = 30 byte, rotate_8*, ASL×2 — not float +120.
    let expect_z = 1000i16.wrapping_add(strat_roffs_full_scaled(0, 0, 0, 0, 0, 30, 2).2);
    assert_eq!(
        shot.1.worldz, expect_z,
        "identity Roffs Z for elaser+weapon_pos"
    );
}

/// Minor #7: flyinleft sets sflag1 only on notdelay-1 ticks (with vx==-5).
#[test]
fn clship_flyinleft_sflag1_gated_by_notdelay() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    strat_clship_turna_init(&mut g, idx);
    // Enter flyinleft approach: clear flag, vx at max, inside X band.
    g.objs.aliens[idx as usize].sflags2 &= !CLSHIP_FLAG1;
    g.objs.aliens[idx as usize].vx = -5;
    g.objs.aliens[idx as usize].worldx = 0;

    // Odd frame: notdelay 1 false → flag must NOT set.
    g.vars.gameframe = 1;
    assert!(!frame_tick_mod(&g, 1));
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sflags2 & CLSHIP_FLAG1,
        0,
        "odd frame must not set sflag1"
    );
    assert_eq!(g.objs.aliens[idx as usize].vx, -5);

    // Even frame: notdelay 1 true → flag sets.
    g.vars.gameframe = 2;
    assert!(frame_tick_mod(&g, 1));
    run(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & CLSHIP_FLAG1,
        0,
        "even frame with vx==-5 must set sflag1"
    );
}

/// Minor #7: flyinright mirrors — sflag1 only on notdelay when vx==5.
#[test]
fn clship_flyinright_sflag1_gated_by_notdelay() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 500;
    strat_clship_turnb_init(&mut g, idx);
    g.objs.aliens[idx as usize].sflags2 &= !CLSHIP_FLAG1;
    g.objs.aliens[idx as usize].vx = 5;
    g.objs.aliens[idx as usize].worldx = 0;

    g.vars.gameframe = 1;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags2 & CLSHIP_FLAG1, 0);

    g.vars.gameframe = 2;
    run(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & CLSHIP_FLAG1, 0);
}

/// Minor #8: zaco2loop_init fires HMISSILE1 on level!=1 with no aliens[0] gate.
#[test]
fn zaco2loop_fires_hmissile_on_non_easy_unconditionally() {
    // level 2 → missile
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        g.vars.write_ext8(wm::CURRENTLEVEL, 2);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldz = 100; // |dz|<500 after sbyte1 drains
        zaco2_istrat(&mut g, idx);
        // Drain approach countdown.
        g.objs.aliens[idx as usize].sbyte1 = 0;
        let before = g.objs.aliens.iter().filter(|a| a.active).count();
        run(&mut g, idx); // → zaco2loop_init → HMISSILE1
        let after = g.objs.aliens.iter().filter(|a| a.active).count();
        assert!(
            after > before,
            "level!=1 must fire HMISSILE1, before={before} after={after}"
        );
        let missile = g
            .objs
            .aliens
            .iter()
            .find(|alien| alien.active && alien.type_ & ATMISSILE != 0)
            .expect("zaco2 HMISSILE1");
        assert_eq!(missile.shape, SH_MISSILE);
        assert_eq!(missile.sflags & ASF_INVISIBLE, 0);
    }
    // level 1 → no missile
    {
        let mut g = Game::new();
        spawn_player(&mut g, 0);
        g.vars.write_ext8(wm::CURRENTLEVEL, 1);
        let idx = spawn(&mut g);
        g.objs.aliens[idx as usize].worldz = 100;
        zaco2_istrat(&mut g, idx);
        g.objs.aliens[idx as usize].sbyte1 = 0;
        let before = g.objs.aliens.iter().filter(|a| a.active).count();
        run(&mut g, idx);
        let after = g.objs.aliens.iter().filter(|a| a.active).count();
        assert_eq!(after, before, "level 1 must not fire HMISSILE1");
    }
}
