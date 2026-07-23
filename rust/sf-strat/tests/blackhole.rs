//! Black-hole route-warp arming tests — the strats that SET the LE_* warp
//! values shell.rs dispatches on.
//!
//! ASM oracle:
//!  * `blackhole_Istrat` / `.blackhole2_strat` (GA2STRAT.ASM:2170-2262) — the
//!    black-hole APPROACH on Asteroid Belt 1. On destruction it morphs into the
//!    hole and, after an sbyte1 draw-in from 70, ARMS the enter-warp:
//!    `routechange 2` + `levelfinished = le_enterbhole` (GA2STRAT.ASM:2202-2203).
//!  * `bholeexit1/2/3_istrat` + `bholecoll_istrat` + `blackholeexit_Istrat`
//!    (KSTRATS.ASM:679-758) — the three EXIT gates. Each preloads its LE code
//!    (le_bhole1/2/3 = 11/12/13) and, after the ship flies in + a 10-step
//!    draw-in, stores `levelfinished = al_sbyte2` (KSTRATS.ASM:730-731).
//!
//! These assert the game-side warp value the LE_* dispatch consumes
//! (`g.world.levelfinished`). The EXIT codes (11/12/13) are then fully consumed
//! by the already-wired shell warp_advance (routechangebhole1/2/3 -> routes[3]
//! = P19/P18/P20), proven by the sf-game shell test
//! `blackhole_exit_codes_repoint_routes3` (shell.rs). The ENTER code (15) needs
//! the routechange2 arming, which is the single remaining sf-game follow-up
//! (shell.rs warp_advance must fire planets.routechange2() on le::ENTERBHOLE —
//! today a no-op with a "branch armed upstream" TODO). See the BLACKHOLE_BEGIN
//! block note in bosses.rs.

use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;

// Local mirrors of the private bosses.rs / engine constants (cited to the port).
const LE_ENTERBHOLE: u8 = 15; // KALCS.INC:91-103
const LE_BHOLE1: u8 = 11;
const LE_BHOLE2: u8 = 12;
const LE_BHOLE3: u8 = 13;
const SH_BLACKHOLE: u16 = 193;
const BH_SFLAG1: u8 = 0x10; // sflags2 relocation of ROM sflag1
const GF_PLAYERDEAD: u8 = 64;

/// New game with an active player at slot 0 and one strat object armed with the
/// given registered ISTRAT index. Player + object share XZ so the touch/approach
/// gates pass immediately.
fn setup(istrat: usize) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bosses::register(&mut g.world);

    // Player (slot 0) at the origin.
    let p = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.shape = 2;
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
    }
    assert_eq!(p, 0);

    // Strat object at the origin (within the approach/touch range).
    let o = g.objs.alloc().expect("pool");
    strat_init_obj_vars(&mut g.objs.aliens[o as usize]);
    let init = g.world.istrats[istrat].expect("istrat registered");
    {
        let al = &mut g.objs.aliens[o as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.stratptr = Some(init);
    }
    (g, o)
}

// ------------------------------------------------------------
// 1. Registration: all four rows resolve to a strat.
// ------------------------------------------------------------
#[test]
fn registration_populates_istrat_rows() {
    let mut g = Game::new();
    bosses::register(&mut g.world);
    assert!(g.world.istrats[bosses::IS_BLACKHOLE].is_some());
    assert!(g.world.istrats[bosses::IS_BHOLEEXIT1].is_some());
    assert!(g.world.istrats[bosses::IS_BHOLEEXIT2].is_some());
    assert!(g.world.istrats[bosses::IS_BHOLEEXIT3].is_some());
    // Exact ISTRATS.ASM rows.
    assert_eq!(bosses::IS_BLACKHOLE, 195);
    assert_eq!(bosses::IS_BHOLEEXIT1, 243);
    assert_eq!(bosses::IS_BHOLEEXIT2, 244);
    assert_eq!(bosses::IS_BHOLEEXIT3, 245);
}

// ------------------------------------------------------------
// 2. Approach init sets up the shootable asteroid (GA2STRAT.ASM:2170-2178).
// ------------------------------------------------------------
#[test]
fn approach_init_arms_asteroid() {
    let (mut g, o) = setup(bosses::IS_BLACKHOLE);
    g.run_strategies();
    let al = &g.objs.aliens[o as usize];
    assert_eq!(al.hp, 20, "s_set_aldata hp");
    assert_eq!(al.sbyte1, 70, "al_sbyte1 = 70 draw-in counter");
    assert_eq!(al.sword1, 100, "al_sword1 = 100");
    assert!(al.expstratptr.is_some(), "exp morph strat wired");
    // Not yet the black hole; still the asteroid, no warp armed.
    assert_ne!(al.shape, SH_BLACKHOLE);
    assert_eq!(g.world.levelfinished, 0);
}

// ------------------------------------------------------------
// 3. Approach: on destruction it morphs into the hole, then the draw-in
//    counting sbyte1 -> 0 ARMS the enter-warp (levelfinished = le_enterbhole).
// ------------------------------------------------------------
#[test]
fn approach_morph_then_countdown_sets_enterbhole() {
    let (mut g, o) = setup(bosses::IS_BLACKHOLE);
    g.run_strategies(); // init

    // Simulate the player shooting it: the engine runs the object's expstrat.
    // (Copying stratptr <- expstratptr drives blackhole_exp_strat without
    // naming the private fn.) It morphs to #blackhole + switches to
    // .blackhole2_strat, which runs the same tick (s_jmpto_strat).
    g.objs.aliens[o as usize].stratptr = g.objs.aliens[o as usize].expstratptr;
    g.run_strategies();
    assert_eq!(
        g.objs.aliens[o as usize].shape, SH_BLACKHOLE,
        ".exp_Istrat morphs shape to #blackhole"
    );
    assert_eq!(
        g.world.levelfinished, 0,
        "no warp yet — sbyte1 still counting down from 70"
    );

    // Drive to the trigger: sflag1 latched (ship within range), one step left.
    {
        let al = &mut g.objs.aliens[o as usize];
        al.sflags2 |= BH_SFLAG1; // .do path, skip the distance gate
        al.sbyte1 = 1; // next decbne -> 0 fires the warp
    }
    g.run_strategies();
    assert_eq!(
        g.world.levelfinished, LE_ENTERBHOLE,
        "sbyte1 -> 0 sets levelfinished = le_enterbhole (GA2STRAT.ASM:2203)"
    );
}

// ------------------------------------------------------------
// 4. Approach: a dead player suppresses the warp (s_jmp_ifplayerdead .ninto).
// ------------------------------------------------------------
#[test]
fn approach_dead_player_suppresses_warp() {
    let (mut g, o) = setup(bosses::IS_BLACKHOLE);
    g.run_strategies();
    g.objs.aliens[o as usize].stratptr = g.objs.aliens[o as usize].expstratptr;
    g.run_strategies(); // morph

    g.vars.gameflags |= GF_PLAYERDEAD;
    {
        let al = &mut g.objs.aliens[o as usize];
        al.sflags2 |= BH_SFLAG1;
        al.sbyte1 = 1;
    }
    g.run_strategies();
    assert_eq!(
        g.world.levelfinished, 0,
        "player-dead skips the whole warp block (.ninto)"
    );
}

// ------------------------------------------------------------
// 5. Exit gates: flying in hands to blackholeexit, whose step counter draining
//    to 0 stores the preloaded LE code (11/12/13). Each gate -> its own code.
// ------------------------------------------------------------
fn exit_reaches_code(istrat: usize, expect: u8) {
    let (mut g, o) = setup(istrat);

    // Tick 1: bholeexit*_init preloads al_sbyte2 + bholecoll_strat touches the
    // ship (dz=0, |dx|+|dy|=0) -> hands to blackholeexit (next tick).
    g.run_strategies();
    assert_eq!(
        g.objs.aliens[o as usize].sbyte2, expect,
        "gate preloads LE code"
    );

    // Tick 2: blackholeexit_init (sbyte1=8, sbyte3=10) + first .strat step.
    g.run_strategies();
    assert_eq!(g.world.levelfinished, 0, "still drawing in");

    // Force the final step: sub-counter about to wrap, one step left.
    {
        let al = &mut g.objs.aliens[o as usize];
        al.sbyte1 = 1; // decbne -> 0: advance a step
        al.sbyte3 = 1; // decbne -> 0: fire the warp
    }
    g.run_strategies();
    assert_eq!(
        g.world.levelfinished, expect,
        "blackholeexit stores levelfinished = al_sbyte2 (KSTRATS.ASM:730-731)"
    );
}

#[test]
fn exit_gate1_sets_bhole1() {
    exit_reaches_code(bosses::IS_BHOLEEXIT1, LE_BHOLE1); // -> Venom 1 Orbital (P19)
}
#[test]
fn exit_gate2_sets_bhole2() {
    exit_reaches_code(bosses::IS_BHOLEEXIT2, LE_BHOLE2); // -> Sector Y (P18)
}
#[test]
fn exit_gate3_sets_bhole3() {
    exit_reaches_code(bosses::IS_BHOLEEXIT3, LE_BHOLE3); // -> Sector Z (P20)
}

// ------------------------------------------------------------
// 6. Exit gate: an out-of-range ship does NOT trigger the gate (the touch
//    gates are |dz|<200 && |dx|+|dy|<100, both strict).
// ------------------------------------------------------------
#[test]
fn exit_gate_out_of_range_does_not_arm() {
    let (mut g, o) = setup(bosses::IS_BHOLEEXIT1);
    // Move the gate far from the ship in Z (>=200).
    g.objs.aliens[o as usize].worldz = 5000;
    g.run_strategies();
    // Still the collision strat (never handed to blackholeexit); no warp.
    for _ in 0..40 {
        g.run_strategies();
    }
    assert_eq!(g.world.levelfinished, 0, "no touch -> no warp");
}
