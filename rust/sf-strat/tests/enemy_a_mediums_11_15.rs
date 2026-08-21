//! Tick 154: AUDIT_ENEMY_A Mediums #11–#15 — zaco2loop turn, wormgo drift,
//! itemtorange height, zaco3/cameleon s_beqdec test-then-dec.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::Game;
use sf_strat::enemy_a::{
    gasflags, item4_istrat, set_gasflags, strat_cameleon_init, strat_worm_init, strat_zaco3_init,
    zaco2_istrat, AF_LEFT_PL,
};

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

/// Medium #11: leftpl SET → rotz/roty −10/−4; CLEAR → +10/+4.
#[test]
fn zaco2loop_turn_follows_leftpl() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 100; // |dz|<500 → loop
    zaco2_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 0;
    g.objs.aliens[idx as usize].sbyte2 = 3;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.objs.aliens[idx as usize].worldy = -10; // skip ground bounce
    run(&mut g, idx); // enter loop (sbyte1 = DEG180/4)

    // leftpl CLEAR → +.
    g.objs.aliens[idx as usize].flags &= !AF_LEFT_PL;
    g.objs.aliens[idx as usize].rotz = 100;
    g.objs.aliens[idx as usize].roty = 50;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.objs.aliens[idx as usize].worldy = -10;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 110);
    assert_eq!(g.objs.aliens[idx as usize].roty, 54);

    // leftpl SET → −.
    g.objs.aliens[idx as usize].flags |= AF_LEFT_PL;
    g.objs.aliens[idx as usize].rotz = 100;
    g.objs.aliens[idx as usize].roty = 50;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].vx = 0;
    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].vz = 0;
    g.objs.aliens[idx as usize].worldy = -10;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 90);
    assert_eq!(g.objs.aliens[idx as usize].roty, 46);
}

/// Medium #12: leftpl SET → vx+=1; CLEAR → vx-=1.
#[test]
fn wormgo_drift_follows_leftpl() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    strat_worm_init(&mut g, idx);
    // Force wormsplit → wormgo: KILLTYPE2 + sbyte2 countdown.
    let gf = gasflags(&g) | 0x02; // GASF_KILLTYPE2
    set_gasflags(&mut g, gf);
    run(&mut g, idx); // wormsplit_init
    g.objs.aliens[idx as usize].sbyte2 = 0;
    run(&mut g, idx); // wormgo_init (vx=0, vz=-10)

    g.objs.aliens[idx as usize].flags |= AF_LEFT_PL;
    g.objs.aliens[idx as usize].vx = 5;
    run(&mut g, idx);
    // vx was 5, +=1 → 6, then apply_velocity adds vx to worldx.
    // Check via: after tick with leftpl, vx should be 6 before next...
    // wormgo applies vx+=1 then apply_velocity; so vx ends at 6.
    assert_eq!(g.objs.aliens[idx as usize].vx, 6);

    g.objs.aliens[idx as usize].flags &= !AF_LEFT_PL;
    g.objs.aliens[idx as usize].vx = 5;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vx, 4);
}

/// Medium #13: worldy+=3 only when worldy < minpmoveY+50.
#[test]
fn itemtorange_raises_only_when_higher_than_floor() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    g.vars.minpmove_y = -60; // floor+50 = -10
    let idx = spawn(&mut g);
    item4_istrat(&mut g, idx);

    g.objs.aliens[idx as usize].worldy = -40; // < -10 → +=3
    g.objs.aliens[idx as usize].worldz = 10_000; // skip pickup
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -37);

    g.objs.aliens[idx as usize].worldy = 0; // >= -10 → no add
    g.objs.aliens[idx as usize].worldz = 10_000;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
}

/// Medium #14: s_beqdec — with sbyte1=2 fires twice then circles on 3rd gate.
#[test]
fn zaco3_beqdec_fires_twice_then_circles() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let houdai = spawn(&mut g);
    g.objs.aliens[houdai as usize].shape = 54;
    g.objs.aliens[houdai as usize].worldz = 200;
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 200;
    strat_zaco3_init(&mut g, idx);
    let attack = g.objs.aliens[idx as usize].stratptr;

    g.objs.aliens[idx as usize].sbyte1 = 2;
    g.objs.aliens[idx as usize].vel = 0;
    g.objs.aliens[idx as usize].rotx = 0;
    g.objs.aliens[idx as usize].roty = 0;

    let count_active = |g: &Game| g.objs.aliens.iter().filter(|a| a.active).count();
    let before = count_active(&g);

    g.vars.gameframe = 0;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 1);
    assert_eq!(count_active(&g), before + 2); // bolt + muzzle flash
    assert_eq!(g.objs.aliens[idx as usize].stratptr, attack);

    g.vars.gameframe = 8;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
    assert_eq!(count_active(&g), before + 4); // two bolts + two flashes
    assert_eq!(g.objs.aliens[idx as usize].stratptr, attack);

    g.vars.gameframe = 16;
    run(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].stratptr, attack,
        "3rd gate must enter circle"
    );
    // Circle sets sbyte1=30 then runs same tick (dec → 29).
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 29);
}

/// Medium #15: cameleon_phase1 beqdec — sbyte1==0 transitions same tick.
#[test]
fn cameleon_beqdec_transitions_when_zero() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn(&mut g);
    strat_cameleon_init(&mut g, idx);
    let phase1 = g.objs.aliens[idx as usize].stratptr;

    // Reach the beqdec gate: rotx==DEG180, rotz==DEG90.
    g.objs.aliens[idx as usize].rotx = 128; // DEG180
    g.objs.aliens[idx as usize].rotz = 64; // DEG90
    g.objs.aliens[idx as usize].sbyte1 = 1;
    g.vars.gameframe = 1; // avoid fire gate this tick
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
    assert_eq!(g.objs.aliens[idx as usize].stratptr, phase1);

    g.objs.aliens[idx as usize].rotx = 128;
    g.objs.aliens[idx as usize].rotz = 64;
    run(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].stratptr, phase1,
        "sbyte1==0 must enter phase2 same tick"
    );
}
