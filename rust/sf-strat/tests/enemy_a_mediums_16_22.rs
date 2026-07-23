//! Tick 155: AUDIT_ENEMY_A Mediums #16–#22 — zaco4 same-tick fallthrough,
//! flyaway leftpl, zaco3die signed pitch, zaco3go stale vecs, para→para2
//! initface + stored aim + gravity +3.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::enemy_a::{
    strat_para_init, strat_zaco3_init, strat_zaco4_init, AF_LEFT_PL, ASF2_SMFLAG1, DEG45,
};

fn spawn_player(g: &mut Game, x: i16, y: i16, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    let al = &mut g.objs.aliens[0];
    al.active = true;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    al.sflags3 |= ASF3_REALOBJ;
    g.vars.player_posx = x;
    g.vars.player_posy = y;
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

/// Medium #16: zaco4_attack sbyte1==0 falls into circle same tick (sbyte1→29).
#[test]
fn zaco4_attack_enters_circle_same_tick() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let pillar = spawn(&mut g);
    g.objs.aliens[pillar as usize].shape = 27;
    g.objs.aliens[pillar as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco4_init(&mut g, idx);
    let attack = g.objs.aliens[idx as usize].stratptr;

    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].worldy = -600;
    g.vars.gameframe = 0;

    run(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, attack);
    // Circle sets 30 then decs same tick → 29.
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29);
    // Achase toward -200 ran same tick (not deferred).
    assert!(
        g.objs.aliens[idx as usize].worldy > -600,
        "circle body must run same tick"
    );
}

/// Medium #16 sibling: circle sbyte1→0 falls into flyaway same tick.
#[test]
fn zaco4_circle_enters_flyaway_same_tick() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let pillar = spawn(&mut g);
    g.objs.aliens[pillar as usize].shape = 27;
    g.objs.aliens[pillar as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco4_init(&mut g, idx);

    // Enter circle first.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].vel = 0;
    g.vars.gameframe = 0;
    run(&mut g, idx);
    let circle = g.objs.aliens[idx as usize].stratptr;

    // Force next tick to hit sbyte1==1 → dec to 0 → flyaway same frame.
    g.objs.aliens[idx as usize].sbyte1 = 1;
    g.objs.aliens[idx as usize].sbyte2 = 140;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].roty = 0;
    run(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, circle);
    // Flyaway achases toward ±30 yaw — roty must have moved from 0.
    assert_ne!(
        g.objs.aliens[idx as usize].roty, 0,
        "flyaway body same tick"
    );
}

/// Medium #17: flyaway uses AF_LEFT_PL, not live worldx compare.
#[test]
fn zaco4_flyaway_uses_leftpl_not_worldx() {
    let mut g = Game::new();
    // Player to the RIGHT of enemy (worldx larger) — live compare would say "right".
    spawn_player(&mut g, 500, -40, 200);
    let pillar = spawn(&mut g);
    g.objs.aliens[pillar as usize].shape = 27;
    g.objs.aliens[pillar as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco4_init(&mut g, idx);

    // Enter flyaway with leftpl SET → target_yaw = -30 (not +30).
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.vars.gameframe = 0;
    run(&mut g, idx); // → circle
    g.objs.aliens[idx as usize].sbyte1 = 1;
    g.objs.aliens[idx as usize].flags |= AF_LEFT_PL;
    g.objs.aliens[idx as usize].roty = 0;
    g.objs.aliens[idx as usize].vel = 0;
    run(&mut g, idx); // → flyaway, achase toward -30

    let roty = g.objs.aliens[idx as usize].roty as i8;
    assert!(
        roty < 0,
        "leftpl SET must chase toward -30 (got {roty}); live worldx would chase +30"
    );
}

/// Medium #18: zaco3die signed rotx cap — negative pitch still climbs toward deg45.
#[test]
fn zaco3die_signed_rotx_cap_climbs_from_negative() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let houdai = spawn(&mut g);
    g.objs.aliens[houdai as usize].shape = 54;
    g.objs.aliens[houdai as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco3_init(&mut g, idx);
    let exp = g.objs.aliens[idx as usize].expstratptr.expect("exp");
    g.call_strat(exp, idx); // zaco3die_init
    let die = g.objs.aliens[idx as usize].stratptr.expect("die");

    // Stay in dive (worldy < -100). Start at rotx = -30 (226 unsigned).
    g.objs.aliens[idx as usize].worldy = -200;
    g.objs.aliens[idx as usize].rotx = (-30i8) as u8;
    g.objs.aliens[idx as usize].vel = 0;
    g.call_strat(die, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].rotx,
        (-26i8) as u8,
        "signed compare must allow climb from -30 toward deg45"
    );

    // Past deg45 signed: stop adding.
    g.objs.aliens[idx as usize].worldy = -200;
    g.objs.aliens[idx as usize].rotx = DEG45.wrapping_add(1);
    g.objs.aliens[idx as usize].vel = 0;
    let before = g.objs.aliens[idx as usize].rotx;
    g.call_strat(die, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].rotx, before,
        "must not add when (i8)rotx > deg45"
    );
}

/// Medium #19: |dz|<400 keeps stale vecs (no gen_vecs_3d).
#[test]
fn zaco3go_keeps_stale_vecs_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let houdai = spawn(&mut g);
    g.objs.aliens[houdai as usize].shape = 54;
    g.objs.aliens[houdai as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco3_init(&mut g, idx);
    let exp = g.objs.aliens[idx as usize].expstratptr.expect("exp");
    g.call_strat(exp, idx);
    let die = g.objs.aliens[idx as usize].stratptr.expect("die");

    // Land into go.
    g.objs.aliens[idx as usize].worldy = -50;
    g.call_strat(die, idx);

    // Close: |dz|=100 < 400 — plant stale vecs, confirm they survive.
    g.objs.aliens[idx as usize].worldz = 100;
    g.objs.aliens[0].worldz = 0;
    g.objs.aliens[idx as usize].vx = 77;
    g.objs.aliens[idx as usize].vy = 88;
    g.objs.aliens[idx as usize].vz = 99;
    g.objs.aliens[idx as usize].vel = 60;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, 77);
    assert_eq!(g.objs.aliens[idx as usize].vy, 88);
    assert_eq!(g.objs.aliens[idx as usize].vz, 99);
}

/// Medium #20/#21: para→para2 clears smflag1 and does not run para2 same tick;
/// first para2 tick latches aim into sbyte3/4.
#[test]
fn para_to_para2_initface_latches_aim() {
    let mut g = Game::new();
    spawn_player(&mut g, 200, -40, 500);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -5;
    g.objs.aliens[idx as usize].worldz = 500;
    strat_para_init(&mut g, idx);
    let para = g.objs.aliens[idx as usize].stratptr;

    // Drop to ground → para2 transition (no para2 body this frame).
    g.objs.aliens[idx as usize].worldy = 0;
    run(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].stratptr, para);
    assert_eq!(
        g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1,
        0,
        "smflag1 cleared for initface"
    );
    // Aim not latched yet (para2 didn't run).
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 0);

    // First para2 tick latches aim.
    let before_roty = g.objs.aliens[idx as usize].roty;
    run(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1, 0);
    let latched_yaw = g.objs.aliens[idx as usize].sbyte3;
    let latched_pitch = g.objs.aliens[idx as usize].sbyte4;
    assert!(latched_yaw != 0 || latched_pitch != 0 || before_roty != 0);

    // Move player far away — subsequent ticks must chase stored aim, not live.
    g.objs.aliens[0].worldx = -5000;
    g.objs.aliens[0].worldz = -5000;
    g.vars.player_posx = -5000;
    g.vars.player_posz = -5000;
    // Keep dist_xz >= 400 so we don't jump.
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldz = 0;
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sbyte3, latched_yaw,
        "must not re-latch live player aim"
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, latched_pitch);
}

/// Medium #22: para2 gravity adds +3 to vy each frame.
#[test]
fn para2_gravity_adds_three() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 2000);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldy = -5;
    g.objs.aliens[idx as usize].worldz = 0;
    strat_para_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = 0;
    run(&mut g, idx); // → para2, no body
    run(&mut g, idx); // latch aim

    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].worldy = -100; // stay airborne
    g.vars.gameframe = 1; // avoid hop gate
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].vy, 3,
        "s_falldown_Yvec gravity #3"
    );
}
