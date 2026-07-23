//! Tick 162: AUDIT_ENEMY_A Minors #15–#16 — zaco1 phase fall-through +
//! spiral SINTAB/COSTAB toward-zero; szaco2 relexplode; hard_Istrat no
//! enemy1 + hardenemy1 ISTRATS row 104.

use sf_game::alien::{ASF3_REALOBJ, ATZREMOVE};
use sf_game::game::Game;
use sf_strat::enemy_a::{
    hardenemy1_istrat, strat_hard_init, strat_szaco2_init, strat_zaco1l_init, ASF2_RELEXPLODE,
    COLLTYPE_ENEMY1, DEG0,
};
use sf_strat::table;

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

/// Minor #16: hard_Istrat does not set COLLTYPE_ENEMY1.
#[test]
fn hard_init_has_no_enemy1_colltype() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    strat_hard_init(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "hard_Istrat must not set enemy1"
    );
    assert!(g.objs.aliens[idx as usize].stratptr.is_none());
}

/// Minor #16: hardenemy1_Istrat sets COLLTYPE_ENEMY1; ISTRATS index 104 wired.
#[test]
fn hardenemy1_sets_enemy1_and_is_registered() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    hardenemy1_istrat(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1,
        0,
        "hardenemy1 must set enemy1"
    );
    assert!(g.objs.aliens[idx as usize].stratptr.is_none());

    table::register_all(&mut g);
    assert!(
        g.world.istrats[104].is_some(),
        "IS_HARDENEMY1=104 must be registered"
    );
}

/// Minor #15: zaco1a_init falls into phase1 same frame (ATZREMOVE + rotx−1).
#[test]
fn zaco1_phase0_falls_into_phase1_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    // |dz| = 1000 → phase0 transitions to phase1 on the spawn-frame body.
    // Seed yaw away from deg0 so phase1 does not also fall into phase2.
    // worldy already 0 → phase0 pitch-chase is a no-op (target pitch 0 from dy=0
    // still moves rotx via achase); accept rotx < start as phase1's −1 ran.
    g.objs.aliens[idx as usize].worldz = 1000;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].rotx = 10;
    g.objs.aliens[idx as usize].roty = 128; // DEG180
    strat_zaco1l_init(&mut g, idx);
    assert!(
        g.objs.aliens[idx as usize].type_ & ATZREMOVE != 0,
        "phase0→1 must set remove-behind"
    );
    assert!(
        g.objs.aliens[idx as usize].rotx < 10,
        "phase1 body must run on the transition frame (rotx−1 at least)"
    );
    // Still in phase1 (not cascaded to phase2 circ): sword2/ptr stay 0.
    assert_eq!(g.objs.aliens[idx as usize].sword2, 0);
    assert_eq!(g.objs.aliens[idx as usize].ptr, 0);
}

/// Minor #15: zaco1b_init falls into phase2 same frame (spiral writes).
#[test]
fn zaco1_phase1_falls_into_phase2_same_frame() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    strat_zaco1l_init(&mut g, idx);
    // Now in phase1. Snap yaw to deg0 and move into .circ band, then tick.
    g.objs.aliens[idx as usize].roty = DEG0;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotz = 0;
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].sword2 = 0;
    g.objs.aliens[idx as usize].ptr = 0;
    run(&mut g, idx);
    let sword2 = g.objs.aliens[idx as usize].sword2;
    let ptr = g.objs.aliens[idx as usize].ptr;
    assert!(
        sword2 != 0 || ptr != 0,
        "phase2 .circ must run on phase1→2 fall-through frame"
    );
}

/// Minor #15: spiral uses SINTAB/COSTAB toward-zero (not arithmetic >>).
#[test]
fn zaco1_phase2_spiral_uses_sintab_toward_zero() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].worldz = 1000;
    strat_zaco1l_init(&mut g, idx);
    // rotz=0 → sbyte1 = 0−64 = 192; SINTAB[192]=−127 → /8 toward zero = −15
    // (arithmetic >>3 would be −16). COSTAB[192]=0 → ptr=0.
    g.objs.aliens[idx as usize].roty = DEG0;
    g.objs.aliens[idx as usize].worldz = 500;
    g.objs.aliens[idx as usize].rotz = 0;
    g.objs.aliens[idx as usize].sbyte2 = 0;
    run(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].sword2, -15,
        "sintab,-3 toward zero: -127/8 = -15"
    );
    assert_eq!(g.objs.aliens[idx as usize].ptr, 0);
}

/// Minor #15: szaco2_Istrat sets relexplode (HD ASF2_RELEXPLODE).
#[test]
fn szaco2_init_sets_relexplode() {
    let mut g = Game::new();
    spawn_player(&mut g, 0, -40, 0);
    let idx = spawn(&mut g);
    strat_szaco2_init(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE,
        0,
        "szaco2 must set relexplode"
    );
}
