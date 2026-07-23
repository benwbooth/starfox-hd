//! Tick 211: chicken `firebreathe_istrat` (DSTRATS.ASM:4629-4699) — trail
//! pieces, ground bounce re-aim, |worldx|/Z bounds → short fade.

use sf_game::alien::{ASF3_REALOBJ, ASF_NOHITAFFECT};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::HARD_HP;
use sf_strat::bosses::{
    chicken_firebreath2_istrat, chicken_firebreath_istrat, chicken_firebreath_strat,
};

const SH_CHICK_FIREBREATH: u16 = 363;
const FIREBREATH_AP: u8 = 8;

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("p");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
    g.vars.player_posz = z;
}

fn spawn_obj(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("e");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

fn count_firebreath(g: &Game) -> usize {
    g.objs
        .aliens
        .iter()
        .filter(|a| a.active && a.shape == SH_CHICK_FIREBREATH)
        .count()
}

/// firebreathe_istrat: vel 80, hardHP, AP 8, ENEMY1, nohitaffect, sbyte1=2.
#[test]
fn firebreath_istrat_sets_ball_data() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -100, 500);
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    chicken_firebreath_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.vel, 80);
    assert_eq!(al.hp, HARD_HP);
    assert_eq!(al.ap, FIREBREATH_AP);
    assert_eq!(al.sbyte1, 2);
    assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
    assert_ne!(al.collflags & 0x10, 0); // ENEMY1 / ACF_COLLTYPE2
                                        // Same-frame .strat spawned one deferred trail (set_strat, not yet faded).
    assert!(count_firebreath(&g) >= 2, "ball + trail");
}

/// firebreathe2 keeps caller vel (seadragon 120).
#[test]
fn firebreath2_preserves_vel() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -100, 500);
    g.objs.aliens[idx as usize].vel = 120;
    chicken_firebreath2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vel, 120);
}

/// Each tick spawns a short trail piece.
#[test]
fn firebreath_spawns_trail_each_tick() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -200, 800);
    chicken_firebreath_istrat(&mut g, idx);
    let after_init = count_firebreath(&g);
    chicken_firebreath_strat(&mut g, idx);
    assert!(count_firebreath(&g) > after_init, "another trail each tick");
}

/// worldy >= 0 → snap to ground, re-aim, resume via backagain.
#[test]
fn firebreath_ground_bounce_reaims() {
    let mut g = Game::new();
    spawn_player(&mut g, 1000);
    let idx = spawn_obj(&mut g, 200, -10, 500);
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    chicken_firebreath_istrat(&mut g, idx);
    // Force ground contact next tick.
    g.objs.aliens[idx as usize].worldy = 5;
    g.objs.aliens[idx as usize].vy = 0;
    chicken_firebreath_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, 80);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 2);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    let yaw = g.objs.aliens[idx as usize].roty;
    // Player at z=1000, ball at z=500 → aimed (Yanglexy+nega).
    assert_ne!(yaw, 0, "re-aimed yaw after bounce");
    // Resume via backagain → genvecs + strat.
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
    let al = &g.objs.aliens[idx as usize];
    assert!(al.vx != 0 || al.vz != 0, "vecs after backagain");
}

/// worldx >= 1000 → convert to short fade (nohitaffect trail).
#[test]
fn firebreath_x_limit_becomes_short() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g, 0, -100, 500);
    chicken_firebreath_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldx = 1000;
    g.objs.aliens[idx as usize].worldy = -50;
    chicken_firebreath_strat(&mut g, idx);
    // Still active as short; nohitaffect set.
    assert!(g.objs.aliens[idx as usize].active);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT, 0);
    // Drive short until remove (sbyte1=2 → ~4 ticks to colframe>=8).
    for _ in 0..8 {
        if !g.objs.aliens[idx as usize].active {
            break;
        }
        if let Some(s) = g.objs.aliens[idx as usize].stratptr {
            g.objs.aldead = 0;
            g.call_strat(s, idx);
            if g.objs.aldead != 0 {
                g.objs.free(idx);
                break;
            }
        }
    }
    assert!(!g.objs.aliens[idx as usize].active, "short fades out");
}
