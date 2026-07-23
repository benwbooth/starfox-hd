//! Behavioural tests for the clear-demo SHIP / TURN / BRIDGE / DIVE / UNDER
//! fly-through fleet ships (GCSTRATS.ASM:318-1033). One representative variant
//! per family: init field checks, fly-in advance, and the family-specific
//! boost/maneuver hand-off. Values hand-derived from the ASM (cited inline).

use sf_game::game::{Game, StrategyFn};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemy_a;

const DEG45: u8 = 32;
const DEG90_NEG: u8 = 192; // (64u8).wrapping_neg()
const CLSHIP_FLAG1: u8 = 0x10; // sflag1
const CLSHIP_FLAG2: u8 = 0x20; // sflag2

/// Spawn a player at slot 0 and a subject running `init`; return the subject
/// index. Player scrolls forward (pviewvelz) via the per-tick advance in
/// `tick`.
fn setup(init: StrategyFn, subject_x: i16) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.pviewvelz = 65;
    g.vars.psvar_word2 = 40; // playerZ boost term used by *_cont tails
                             // Player slot 0.
    let p = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
    g.objs.aliens[0].shape = 2;
    g.objs.aliens[0].hp = 40;
    g.objs.aliens[0].worldz = 0;
    // Subject.
    let e = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
    {
        let al = &mut g.objs.aliens[e as usize];
        al.shape = 30;
        al.worldx = subject_x;
        al.worldy = 0;
        al.worldz = 1000;
    }
    let sid = g.world.register_strategy(init);
    g.objs.aliens[e as usize].stratptr = Some(sid);
    // First tick runs the init (sets the real strat + fields).
    g.run_strategies();
    (g, e)
}

/// Advance one frame, scrolling the player forward first.
fn tick(g: &mut Game, t: i32) {
    let z = (65 * (t + 1)) as i16;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].vz = 65;
    g.run_strategies();
}

// ============================================================
// SHIP family (clshipSHIPa, GCSTRATS.ASM:318/328)
// ============================================================

#[test]
fn ship_inits_and_chases_worldx() {
    // clshipSHIPa_Istrat (:318): sflag1, sbyte1=10, rotz=-deg90, shadow/strat.
    let (mut g, e) = setup(enemy_a::strat_clship_shipa_init, -1000);
    {
        let al = &g.objs.aliens[e as usize];
        assert_ne!(al.sflags2 & CLSHIP_FLAG1, 0, "SHIP sets sflag1 (:320)");
        assert_eq!(al.sbyte1, 10, "SHIP sbyte1=10 (:321)");
        assert_eq!(al.rotz, DEG90_NEG, "SHIP rotz=-deg90 (:326)");
    }
    // clship1_strat (:330) `s_achase_alvar al_worldx,#-50,4` pulls worldx from
    // -1000 toward -50 (i.e. increasing) while the ship stays alive.
    let x0 = g.objs.aliens[e as usize].worldx;
    for t in 0..40 {
        tick(&mut g, t);
    }
    let x1 = g.objs.aliens[e as usize].worldx;
    assert!(x1 > x0, "worldx chases toward -50 ({x0} -> {x1})");
    assert!(x1 < -50, "not overshot the -50 target yet ({x1})");
    assert!(
        g.objs.aliens[e as usize].active,
        "SHIP stays alive during the demo chase"
    );
}

// ============================================================
// TURN family (clshipTURNa, GCSTRATS.ASM:435)
// ============================================================

#[test]
fn turn_flies_in_then_banks_to_speed32() {
    // clshipTURNa_Istrat (:435): vx=10, rotz=-deg90, sword1=180, sflag2.
    let (mut g, e) = setup(enemy_a::strat_clship_turna_init, -400);
    {
        let al = &g.objs.aliens[e as usize];
        assert_eq!(al.vx, 10, "TURN vx=10 (:440)");
        assert_eq!(al.rotz, DEG90_NEG, "TURN rotz=-deg90 (:441)");
        assert_eq!(al.sword1, 100 + 130 + 10 - 60, "TURN sword1=180 (:442)");
        assert_ne!(al.sflags2 & CLSHIP_FLAG2, 0, "TURNa sets sflag2 (:444)");
    }
    let x0 = g.objs.aliens[e as usize].worldx;
    // flyinleft_srou advances worldx by vx each frame; sword1 counts down in
    // clshipTURN_cont (:502). After sword1 expires the ship banks
    // (clshipTURN_strat :531 `s_speedto #32,1`), ramping vel from 0 to 32.
    let mut reached32 = false;
    for t in 0..260 {
        tick(&mut g, t);
        if g.objs.aliens[e as usize].vel == 32 {
            reached32 = true;
        }
    }
    assert_ne!(
        g.objs.aliens[e as usize].worldx, x0,
        "fly-in moved the ship in X"
    );
    assert!(reached32, "bank ramps speed to 32 after the sword1 timeout");
}

// ============================================================
// BRIDGE family (clshipBRIDGEa, GCSTRATS.ASM:564)
// ============================================================

#[test]
fn bridge_chases_then_boosts_via_bridgeboost() {
    // clshipBRIDGEa_Istrat (:564): rotz=-deg90, roty=deg45, sword1=70.
    let (mut g, e) = setup(enemy_a::strat_clship_bridgea_init, 600);
    {
        let al = &g.objs.aliens[e as usize];
        assert_eq!(al.roty, DEG45, "BRIDGE roty=deg45 (:570)");
        assert_eq!(al.sword1, 130 - 60, "BRIDGE sword1=70 (:571)");
    }
    // sword1(70) counts down in clshipBRIDGE_cont, then clshipBridgeboost_strat
    // runs at speed 20 for 50 frames (:645/:663) before the general boost sets
    // speed 120 (clshipboost_strat, :242). Observe both speeds, in order.
    let mut saw20 = false;
    let mut saw120_after20 = false;
    for t in 0..260 {
        tick(&mut g, t);
        let vel = g.objs.aliens[e as usize].vel;
        if vel == 20 {
            saw20 = true;
        }
        if vel == 120 && saw20 {
            saw120_after20 = true;
        }
    }
    assert!(saw20, "bridge-boost drifts at speed 20 (:645)");
    assert!(
        saw120_after20,
        "hands off to the general boost at speed 120 (:242)"
    );
}

// ============================================================
// DIVE family (clshipDIVEa, GCSTRATS.ASM:672)
// ============================================================

#[test]
fn dive_runs_all_regimes_then_boosts() {
    // clshipDIVEa_Istrat (:672): sword1=120, rotz=-deg90, noremove.
    let (mut g, e) = setup(enemy_a::strat_clship_divea_init, -400);
    assert_eq!(
        g.objs.aliens[e as usize].sword1,
        180 - 60,
        "DIVE sword1=120 (:676)"
    );
    // sword1 120->0 across the high/dive/velocity regimes (clshipDIVE_cont2
    // :793 decrements each frame); at 0 clshipDIVEboost_Istrat (:799) levels out
    // (rotx=deg5) and the general boost sets speed 120.
    let mut reached_boost = false;
    for t in 0..160 {
        tick(&mut g, t);
        if g.objs.aliens[e as usize].vel == 120 {
            reached_boost = true;
        }
    }
    assert!(reached_boost, "DIVE reaches the general boost (speed 120)");
    assert_eq!(
        g.objs.aliens[e as usize].rotx, 4,
        "DIVE boost sets rotx=deg5=4 (:802)"
    );
}

// ============================================================
// UNDER family (clshipUNDERa, GCSTRATS.ASM:922)
// ============================================================

#[test]
fn under_chases_then_boosts_to_speed40() {
    // clshipUNDERa_Istrat (:922): rotz=-deg90, roty=deg45, sword1=145, sbyte1=1.
    let (mut g, e) = setup(enemy_a::strat_clship_undera_init, 500);
    {
        let al = &g.objs.aliens[e as usize];
        assert_eq!(al.roty, DEG45, "UNDER roty=deg45 (:928)");
        assert_eq!(al.sword1, 140 + 5, "UNDER sword1=145 (:929)");
        assert_eq!(al.sbyte1, 1, "UNDER sbyte1=1 (:930)");
        assert_ne!(al.sflags2 & CLSHIP_FLAG2, 0, "UNDERa sets sflag2 (:931)");
    }
    // sword1(145) counts down in clshipUNDER_cont, then clshipUNDERboost_strat
    // (:1014 `s_speedto #40,1`) ramps vel from 0 to 40 and flies off.
    let mut reached40 = false;
    for t in 0..260 {
        tick(&mut g, t);
        if g.objs.aliens[e as usize].vel == 40 {
            reached40 = true;
        }
    }
    assert!(reached40, "under-boost ramps speed to 40 (:1014)");
}
