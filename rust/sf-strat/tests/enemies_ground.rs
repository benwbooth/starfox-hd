//! Ground-artillery family behavioural tests (tank1a / tank2 / tank3 +
//! bazookaL/R). ASM oracle: `reference/ultrastarfox/SF/STRAT/KSTRATS.ASM`
//! (tank1a:418-458, tank3:566-611, tank1lr/tank1fire:496-534) and
//! `GA2STRAT.ASM` (tank2:1266-1408, bazooka:1001-1082). No C oracle exists for
//! the tanks; every expected value is hand-derived from the 65816 source and
//! cited inline. Strategies are private, so every tick is driven through the
//! registered `world.istrats[]` row + `Game::call_strat`, exactly as the game
//! engine's `do_strat` would.
//!
//! Fidelity scope-outs asserted-around (never asserted): fog (`s_initfog`/
//! `s_dofog`), engine-flame sprites (`makeengine_srou`), debris/smoke meshes,
//! and positional sound — all cosmetic reads of global scratch RAM.

use sf_game::alien::{ATLASER, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground;

// ISTRATS.ASM def_Istrat indices (== sf-map placement).
const IS_BAZOOKAL: usize = 158;
const IS_BAZOOKAR: usize = 159;
const IS_TANK2: usize = 162;
const IS_TANK1A: usize = 183;
const IS_TANK3: usize = 186;

// Constants mirrored from the port / ASM (cited).
const DEG180: u8 = 128;
const DEG270: u8 = 192; // VARS.INC:18
const COLLTYPE_ENEMY1: u8 = 0x10; // enemy_a acf_colltype2
const COLLTYPE_ENEMYWEAP: u8 = 0x40; // acf_colltype4
const ASF2_SFLAG1: u8 = 0x10; // STRATEQU.INC:914
const SH_ZACO_7: u16 = 129; // route2 rc.rs
const TANK1_HP: u8 = 2; // KSTRATS.ASM:44
const TANK1_AP: u8 = 16; // KSTRATS.ASM:45
const TANK1_FIRERATE: u8 = 50; // KSTRATS.ASM:46
const TANK2_HP: u8 = 40; // STRATEQU.INC:243
const TANK2_AP: u8 = 32; // STRATEQU.INC:244
const BAZOOKA_HP: u8 = 8; // STRATEQU.INC:237
const BAZOOKA_AP: u8 = 16; // STRATEQU.INC:238

fn spawn(g: &mut Game, x: i16, y: i16, z: i16, shape: u16) -> u16 {
    let idx = g.objs.alloc().expect("alien pool");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.shape = shape;
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

/// New game with a static player in slot 0 and the ground family registered.
fn setup() -> Game {
    let mut g = Game::new();
    g.vars.write_ext16(0x1F00, 0x1234); // ea_random seed (RNDVAL)
    g.vars.internal_playpt = 0;
    enemies_ground::register(&mut g.world);
    // Player at the origin, pviewvelz=0 so add_player_z is a no-op in tests.
    let _p = spawn(&mut g, 0, 0, 0, 2);
    g.vars.pviewvelz = 0;
    g
}

/// Spawn an enemy carrying `istrat`'s strategy, as the map would.
fn place(g: &mut Game, istrat: usize, x: i16, y: i16, z: i16, shape: u16) -> u16 {
    let idx = spawn(g, x, y, z, shape);
    g.objs.aliens[idx as usize].stratptr = g.world.istrats[istrat];
    idx
}

fn tick(g: &mut Game, idx: u16) {
    let s = g.objs.aliens[idx as usize].stratptr.expect("stratptr");
    g.call_strat(s, idx);
}

fn any_hplasma(g: &Game) -> bool {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .any(|(i, a)| i != 0 && a.active && a.type_ & ATLASER != 0)
}

// ============================================================
// tank1a (KSTRATS.ASM:418-458)
// ============================================================

#[test]
fn tank1a_far_waits_and_sets_base_vars() {
    // Player z0, tank z6000 -> |dz| = 6000 >= 5000 -> stays in the init/wait
    // strat (KSTRATS.ASM:426 s_jmp_Zdistless #5000 falls through to s_end_strat).
    let mut g = setup();
    let istrat = g.world.istrats[IS_TANK1A];
    let tank = place(&mut g, IS_TANK1A, 0, 0, 6000, 168);
    tick(&mut g, tank);
    let a = g.objs.aliens[tank as usize];
    assert_eq!(a.hp, TANK1_HP, "s_set_aldata tank1HP");
    assert_eq!(a.ap, TANK1_AP, "s_set_aldata tank1AP");
    assert_eq!(a.roty, DEG270, "s_set_alvar al_roty,#deg270");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "s_set_colltype enemy1");
    assert_eq!(a.stratptr, istrat, "still in the wait strat while far");
}

#[test]
fn tank1a_closes_and_arms() {
    // |dz| = 3000 < 5000 -> tank1a2 handoff: strat swap + fire timers +
    // hit/explode strats (KSTRATS.ASM:429-434).
    let mut g = setup();
    let istrat = g.world.istrats[IS_TANK1A];
    let tank = place(&mut g, IS_TANK1A, 0, 0, 3000, 168);
    tick(&mut g, tank);
    let a = g.objs.aliens[tank as usize];
    assert_ne!(a.stratptr, istrat, "handed off out of the wait strat");
    assert!(a.stratptr.is_some());
    assert_eq!(a.sbyte2, TANK1_FIRERATE, "al_sbyte2 = tank1firerate");
    assert_eq!(a.sbyte3, 15, "al_sbyte3 = 15 (left/right time)");
    assert!(a.collstratptr.is_some(), "hitflash wired");
    assert!(a.expstratptr.is_some(), "explode wired");
}

#[test]
fn tank1a_forward_loop_fires_hplasma() {
    // Drive the full close->chase->forward path; the tank1fire cadence
    // (KSTRATS.ASM:515-534) must spawn a homing Hplasma while the player sits
    // in range. Pin the tank at z=1500/x=0 each tick so it stays in .goforward
    // (|dz| in [500,3000), |dx| < 300) and the fire gate can be exercised.
    let mut g = setup();
    let tank = place(&mut g, IS_TANK1A, 0, 0, 1500, 168);
    let mut fired = false;
    for _ in 0..400 {
        g.objs.aliens[tank as usize].worldx = 0;
        g.objs.aliens[tank as usize].worldz = 1500;
        tick(&mut g, tank);
        if any_hplasma(&g) {
            fired = true;
            break;
        }
    }
    assert!(fired, "tank1a reaches .goforward and fires an Hplasma");
}

#[test]
fn tank1a_death_explodes() {
    // hp=2: two hitflash hits (2->1->0) route into explode -> aldead
    // (Strat_Explode, EXPSTRAT.ASM escapeeexplode). Arm first, then damage.
    let mut g = setup();
    let tank = place(&mut g, IS_TANK1A, 0, 0, 3000, 168);
    tick(&mut g, tank); // arm (sets hp=2 + collstrat/expstrat)
    let coll = g.objs.aliens[tank as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(coll, tank); // 2 -> 1 (flash)
    assert_eq!(g.objs.aliens[tank as usize].hp, 1);
    assert_eq!(g.objs.aldead, 0, "still alive after first hit");
    g.call_strat(coll, tank); // 1 -> 0 -> explode
    assert_eq!(g.objs.aldead, 1, "explode set aldead");
}

// ============================================================
// tank3 (KSTRATS.ASM:566-611)
// ============================================================

#[test]
fn tank3_far_waits_with_short_fire_timer() {
    // |dz| = 3000 >= 1800 -> wait; strats wired immediately, roty=0, sbyte2=11
    // (KSTRATS.ASM:569-576).
    let mut g = setup();
    let tank = place(&mut g, IS_TANK3, 0, 0, 3000, 168);
    tick(&mut g, tank);
    let a = g.objs.aliens[tank as usize];
    assert_eq!(a.hp, TANK1_HP);
    assert_eq!(a.roty, 0, "s_set_alvar al_roty,#0");
    assert_eq!(a.sbyte2, 11, "s_set_alvar al_sbyte2,#11");
    assert_eq!(a.sbyte3, 15);
    assert!(a.collstratptr.is_some() && a.expstratptr.is_some());
}

#[test]
fn tank3_closes_enters_forward() {
    // |dz| = 1000 < 1800 -> .forward handoff (KSTRATS.ASM:575).
    let mut g = setup();
    let istrat = g.world.istrats[IS_TANK3];
    let tank = place(&mut g, IS_TANK3, 0, 0, 1000, 168);
    tick(&mut g, tank);
    assert_ne!(
        g.objs.aliens[tank as usize].stratptr, istrat,
        "entered the .goforward loop"
    );
}

// ============================================================
// tank2 (GA2STRAT.ASM:1266-1408)
// ============================================================

#[test]
fn tank2_init_spawns_four_turret_drones() {
    // tank2_Istrat: hp40/ap32, faces deg180, and spawns 4 zaco_7 drones with
    // child numbers 1..4, mother link (al_ptr), relpos, and rise thresholds
    // in al_sword2 (-140,-140,-200,-200) (GA2STRAT.ASM:1266-1289).
    let mut g = setup();
    let tank = place(&mut g, IS_TANK2, 0, 0, 5000, 230);
    tick(&mut g, tank);
    let a = g.objs.aliens[tank as usize];
    assert_eq!(a.hp, TANK2_HP, "tank2HP");
    assert_eq!(a.ap, TANK2_AP, "tank2AP");
    assert_eq!(a.roty, DEG180, "s_set_alvar al_roty,#deg180");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0);
    assert_ne!(a.collflags & COLLTYPE_ENEMYWEAP, 0);

    let drones: Vec<usize> = (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_ZACO_7)
        .collect();
    assert_eq!(drones.len(), 4, "four zaco_7 turret drones");
    for &d in &drones {
        let dz = g.objs.aliens[d];
        assert_eq!(dz.hp, 4, "tank2zaco hp");
        assert_eq!(dz.ap, 8, "tank2zaco ap");
        assert_eq!(dz.ptr, tank + 1, "al_ptr = mother (index+1)");
        assert!((1..=4).contains(&dz.sbyte1), "child number in 1..4");
        let expect_sword2 = if dz.sbyte1 <= 2 { -140 } else { -200 };
        assert_eq!(dz.sword2, expect_sword2, "child rise threshold");
    }
    // Distinct child numbers.
    let mut nums: Vec<u8> = drones.iter().map(|&d| g.objs.aliens[d].sbyte1).collect();
    nums.sort_unstable();
    assert_eq!(nums, vec![1, 2, 3, 4]);
}

#[test]
fn tank2_countdown_wakes_drones_and_flies_them() {
    // Advance the body into its release states: as al_sbyte2 counts down past
    // 90/70/50/30 the four drones are woken to state 1 (rise) and then state 2
    // (detach + chase). The isolated harness must tick each object, as the
    // engine's do_strat does for the whole active list. Assert a drone is woken
    // (stratstate >= 1) and one eventually rises fully to state 2, proving
    // set_childstate + the child state machine (GA2STRAT.ASM:1331-1345 / 1357-1408).
    let mut g = setup();
    // Player near the surface; body inside 1000 z so state 0 advances.
    g.objs.aliens[0].worldy = 100;
    let tank = place(&mut g, IS_TANK2, 0, 200, 900, 230);
    tick(&mut g, tank); // spawn drones + enter the body state machine

    let drones: Vec<u16> = (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_ZACO_7)
        .map(|i| i as u16)
        .collect();
    assert_eq!(drones.len(), 4);

    let mut woke = false;
    let mut flew = false;
    for _ in 0..400 {
        g.objs.aliens[tank as usize].worldz = 900; // keep |dz| < 1000
        tick(&mut g, tank);
        for &d in &drones {
            if g.objs.aliens[d as usize].active {
                tick(&mut g, d);
            }
        }
        if drones.iter().any(|&d| g.objs.aliens[d as usize].stratstate >= 1) {
            woke = true;
        }
        if drones.iter().any(|&d| g.objs.aliens[d as usize].stratstate >= 2) {
            flew = true;
            break;
        }
    }
    assert!(woke, "tank2 countdown woke a drone (set_childstate -> state 1)");
    assert!(flew, "a woken drone rose fully and detached (state 2)");
}

// ============================================================
// bazooka L/R (GA2STRAT.ASM:1001-1082)
// ============================================================

#[test]
fn bazookal_init_sets_left_latch_and_pose() {
    // bazookaL sets sflag1 (turn-left latch); both share bazooka_Icont
    // (hp8/ap16, rotx=-deg90, roty=deg180, speed 80). Spawn high (worldy>=440)
    // so state 0 just rises and the init pose survives the fall-through tick.
    let mut g = setup();
    let baz = place(&mut g, IS_BAZOOKAL, 0, 1000, 3000, 132);
    tick(&mut g, baz);
    let a = g.objs.aliens[baz as usize];
    assert_ne!(a.sflags2 & ASF2_SFLAG1, 0, "bazookaL sets sflag1");
    assert_eq!(a.hp, BAZOOKA_HP);
    assert_eq!(a.ap, BAZOOKA_AP);
    assert_eq!(a.rotx, (-(64i8)) as u8, "rotx = -deg90 (192)");
    assert_eq!(a.roty, DEG180);
    assert_eq!(a.vel, 80, "still rising at speed 80 while worldy>=440");
    assert!(a.collstratptr.is_some() && a.expstratptr.is_some());
}

#[test]
fn bazookar_init_has_no_left_latch() {
    let mut g = setup();
    let baz = place(&mut g, IS_BAZOOKAR, 0, 1000, 3000, 132);
    tick(&mut g, baz);
    assert_eq!(
        g.objs.aliens[baz as usize].sflags2 & ASF2_SFLAG1,
        0,
        "bazookaR does NOT set sflag1"
    );
    assert_eq!(g.objs.aliens[baz as usize].hp, BAZOOKA_HP);
}

#[test]
fn bazooka_rises_levels_and_reaches_fire_state() {
    // Full state 0->1->2 progression: the bazooka rises to worldy<440, levels
    // its pitch, chases worldy to the player, aims, and lobs a RELSLOWELASER
    // burst. Assert a projectile is eventually spawned and the state advanced.
    let mut g = setup();
    // Player near the surface so the state-1 worldy chase + aim converge.
    g.objs.aliens[0].worldy = 100;
    let baz = place(&mut g, IS_BAZOOKAL, 0, 1000, 1200, 132);
    let mut fired = false;
    for _ in 0..600 {
        tick(&mut g, baz);
        if !g.objs.aliens[baz as usize].active {
            break;
        }
        if any_hplasma(&g) {
            fired = true;
            break;
        }
    }
    assert!(fired, "bazooka reaches its fire state and lobs a laser");
}

#[test]
fn bazooka_death_drops_debris_and_explodes() {
    // bazexp_Istrat: spawn a falling debris object (30-frame lifecnt,
    // colldisable) then explode the bazooka (GA2STRAT.ASM:1055-1082).
    let mut g = setup();
    let baz = place(&mut g, IS_BAZOOKAL, 0, 1000, 3000, 132);
    tick(&mut g, baz); // init
    let before = (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count();
    let exp = g.objs.aliens[baz as usize].expstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(exp, baz);
    let after = (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count();
    assert!(after > before, "a falling debris object was spawned");
    assert_eq!(g.objs.aldead, 1, "bazooka explodes (aldead)");
}
