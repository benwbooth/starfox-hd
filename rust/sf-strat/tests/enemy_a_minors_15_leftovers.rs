//! Tick 171: AUDIT #15 leftovers — para2 `nvecs` + full first add_vecs2pos;
//! zaco0_sweep unsigned worldy compare/clamp.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::common::strat_nvecs;
use sf_strat::enemy_a::{strat_para_init, zaco0_istrat};

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

/// para2: `s_gen_vecs` → nvecs leaves hop vy intact; first add_vecs2pos is xyz.
#[test]
fn para2_nvecs_preserves_vy_and_first_add_applies_it() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 2000);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldz = 0;
    strat_para_init(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = 0;
    run(&mut g, idx); // → para2, no body
    run(&mut g, idx); // latch aim (smflag1)

    // Second+ ticks chase + gen_vecs. Seed a hop vy that gen_vecs_2d would wipe.
    g.objs.aliens[idx as usize].vy = -15;
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].vel = 10;
    g.objs.aliens[idx as usize].roty = 40;
    g.vars.gameframe = 1; // avoid hop gate this frame
    let y0 = g.objs.aliens[idx as usize].worldy;

    run(&mut g, idx);

    // nvecs leaves hop vy; first+second add_vecs2pos both apply it, then gravity +3.
    assert_eq!(
        g.objs.aliens[idx as usize].worldy,
        y0.wrapping_add(-15).wrapping_add(-15),
        "first+second add_vecs2pos must apply hop vy (nvecs must not zero it)"
    );
    assert_eq!(g.objs.aliens[idx as usize].vy, -12); // -15 + gravity 3
    let ry = g.objs.aliens[idx as usize].roty;
    let (nx, nz) = strat_nvecs(ry, 10);
    assert_eq!(g.objs.aliens[idx as usize].vx, nx);
    assert_eq!(g.objs.aliens[idx as usize].vz, nz);
}

/// zaco0_sweep: unsigned worldy compare — positive enemy vs negative player still climbs.
#[test]
fn zaco0_sweep_unsigned_worldy_climbs_when_signed_would_not() {
    let mut g = Game::new();
    // Player above ground (negative Y); enemy below 0 (positive Y).
    // Signed: 10 < -40 is false → old port skipped. Unsigned: 10 < 65496 → climb.
    spawn_player(&mut g, 500, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 10;
    g.objs.aliens[idx as usize].worldz = 0;
    zaco0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = 10;
    g.objs.aliens[idx as usize].worldx = 0;

    run(&mut g, idx);
    // +20 then unsigned clamp: 30 as u16 < 65506 → no clamp to -30.
    assert_eq!(g.objs.aliens[idx as usize].worldy, 30);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 43);
}

/// zaco0_sweep: unsigned clamp only hits [-30,-1], not positive Y.
#[test]
fn zaco0_sweep_unsigned_clamp_to_minus_30_in_band() {
    let mut g = Game::new();
    spawn_player(&mut g, 500, -40, 0); // -40 as u16 > -50 → climb
    let idx = spawn(&mut g);
    zaco0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -50;
    g.objs.aliens[idx as usize].worldx = 0;
    run(&mut g, idx);
    // -50+20=-30; at boundary, unsigned >= -30 → clamp stays -30.
    assert_eq!(g.objs.aliens[idx as usize].worldy, -30);
}

/// In-band negative pair still climbs (regression vs signed path).
#[test]
fn zaco0_sweep_in_band_negative_still_climbs() {
    let mut g = Game::new();
    spawn_player(&mut g, 500, -40, 0);
    let idx = spawn(&mut g);
    zaco0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 0;
    run(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -80);
}
