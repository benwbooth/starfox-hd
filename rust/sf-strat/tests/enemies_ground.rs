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

use sf_game::alien::{ATLASER, ATZREMOVE, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground;

// ISTRATS.ASM def_Istrat indices (== sf-map placement).
const IS_BAZOOKAL: usize = 158;
const IS_BAZOOKAR: usize = 159;
const IS_TANK2: usize = 162;
const IS_TANK1A: usize = 183;
const IS_TANK3: usize = 186;
const IS_WALKING: usize = 78;
const IS_WIREMAN: usize = 88;
const IS_WINGLAZERMAN: usize = 91;
const IS_UPERM: usize = 160;
const IS_ROCKHARD: usize = 192;

// Mobile-family shape words (sf-map lc::SH_*).
const SH_WIRE_MAN: u16 = 48;
const SH_W_L: u16 = 50;
const SH_WALKER_0: u16 = 27;
const SH_UPER_M: u16 = 133;

// Mobile-family constants (cited).
const ASF_SHADOW: u8 = 0x04; // alien.rs
const COLLTYPE_ENEMY1_M: u8 = 0x10;
const COLLTYPE_ENEMYWEAP_M: u8 = 0x40;
const WIREMAN_HP: u8 = 4; // wiremanHP
const WINGLAZERMAN_HP: u8 = 8; // winglazermanHP
const WALKING_HP: u8 = 200; // DSTRATS.ASM:863
const UPERM_HP: u8 = 2; // upermHP
const HARDHP: u8 = 0xFF; // hardHP == -1

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

// ============================================================
// wireman (GASTRATS.ASM:2446-2511) — homing evasive flyer (no weapon).
// ============================================================

#[test]
fn wireman_init_sets_data_and_pose() {
    // wireman_Istrat: hp4/ap16, shadow, speed 40, hit/die strats wired. Placed
    // far (>1500 XZ) so it stays in the chase/move path (GASTRATS.ASM:2446-2466).
    let mut g = setup();
    let wm = place(&mut g, IS_WIREMAN, 6000, -500, 6000, SH_WIRE_MAN);
    tick(&mut g, wm);
    let a = g.objs.aliens[wm as usize];
    assert_eq!(a.hp, WIREMAN_HP, "s_set_aldata wiremanHP");
    assert_eq!(a.ap, 16, "wiremanAP");
    assert_eq!(a.vel, 40, "s_set_speed #40");
    assert_ne!(a.sflags & ASF_SHADOW, 0, "s_set_alsflag shadow");
    assert!(a.collstratptr.is_some() && a.expstratptr.is_some());
    assert!(a.stratptr.is_some());
}

#[test]
fn wireman_close_enters_evasive_roll() {
    // Inside 1500 XZ -> wireman2_init sets al_sbyte1 = deg180/4 = 32 (dodge
    // time), picks a random branch, and runs one dodge tick (countdown 32->31)
    // (GASTRATS.ASM:2458,2467-2492).
    let mut g = setup();
    let wm = place(&mut g, IS_WIREMAN, 0, -500, 1000, SH_WIRE_MAN);
    tick(&mut g, wm);
    assert_eq!(
        g.objs.aliens[wm as usize].sbyte1, 31,
        "entered wireman2 (sbyte1 32 -> 31 after one dodge tick)"
    );
    // Keep it close: the dodge counts down toward 0 then resumes chase, which
    // immediately re-enters wireman2 (still inside 1500) — sbyte1 stays bounded.
    for _ in 0..40 {
        g.objs.aliens[wm as usize].worldx = 0;
        g.objs.aliens[wm as usize].worldz = 1000;
        tick(&mut g, wm);
    }
    assert!(
        g.objs.aliens[wm as usize].sbyte1 <= 32,
        "dodge counter stays within its window"
    );
    assert!(g.objs.aliens[wm as usize].active, "still flying");
}

#[test]
fn wireman_grounded_pops_up() {
    // Far from the player (no dodge) but at/under the ground plane (worldy>=0):
    // wireman_cont routes to wiremanup_init -> worldy reset to 0, rotx=-deg22
    // (240), climb timer 30 (GASTRATS.ASM:2460-2498).
    let mut g = setup();
    let wm = place(&mut g, IS_WIREMAN, 6000, 200, 6000, SH_WIRE_MAN);
    tick(&mut g, wm);
    let a = g.objs.aliens[wm as usize];
    // wiremanup_init resets worldy to 0, then wireman_cont2 applies one climb
    // step the same tick, so worldy ends near 0 (down from the placed 200).
    assert!(a.worldy.abs() < 60, "worldy reset to ~0 (was 200): {}", a.worldy);
    assert_eq!(a.rotx, 240, "rotx = -deg22");
    assert!(a.sbyte1 <= 30, "climb timer armed (<=30)");
}

#[test]
fn wireman_death_explodes() {
    // hp=4: four hitflash hits route through wiremandie -> explode (aldead).
    let mut g = setup();
    let wm = place(&mut g, IS_WIREMAN, 6000, -500, 6000, SH_WIRE_MAN);
    tick(&mut g, wm); // init (hp=4)
    let coll = g.objs.aliens[wm as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    for _ in 0..3 {
        g.call_strat(coll, wm);
        assert_eq!(g.objs.aldead, 0, "alive until the 4th hit");
    }
    g.call_strat(coll, wm); // 1 -> 0 -> explode
    assert_eq!(g.objs.aldead, 1, "wiremandie -> explode set aldead");
}

// ============================================================
// winglazerman (GASTRATS.ASM:2811-2903) — wing-laser strafer.
// ============================================================

#[test]
fn winglazerman_init_sets_data_and_pose() {
    // hp8/ap16, speed 40, shadow, al_sbyte2=2 (spin-cycle budget). Far (>1000 Z)
    // so it stays in the chase/move path (GASTRATS.ASM:2811-2830).
    let mut g = setup();
    let wl = place(&mut g, IS_WINGLAZERMAN, 0, -400, 3000, SH_W_L);
    tick(&mut g, wl);
    let a = g.objs.aliens[wl as usize];
    assert_eq!(a.hp, WINGLAZERMAN_HP, "winglazermanHP");
    assert_eq!(a.ap, 16, "winglazermanAP");
    assert_eq!(a.vel, 40, "s_set_speed #40");
    assert_eq!(a.sbyte2, 2, "al_sbyte2 = 2");
    assert_ne!(a.sflags & ASF_SHADOW, 0, "shadow");
    assert!(a.collstratptr.is_some() && a.expstratptr.is_some());
}

#[test]
fn winglazerman_spins_then_fires_wing_lasers() {
    // Inside 1000 Z: spin (deg360/4) then fire two RELSLOWELASERs on the
    // notdelay-2 gate (GASTRATS.ASM:2844-2882). Pin Z in range each tick.
    let mut g = setup();
    let wl = place(&mut g, IS_WINGLAZERMAN, 0, -100, 500, SH_W_L);
    let mut fired = false;
    for _ in 0..400 {
        g.objs.aliens[wl as usize].worldx = 0;
        g.objs.aliens[wl as usize].worldz = 500;
        tick(&mut g, wl);
        if !g.objs.aliens[wl as usize].active {
            break;
        }
        if any_hplasma(&g) {
            fired = true;
            break;
        }
    }
    assert!(fired, "winglazerman reaches its fire routine and fires");
}

#[test]
fn winglazerman_death_explodes() {
    // hp=8: eight hitflash hits -> winglazermandie -> explode.
    let mut g = setup();
    let wl = place(&mut g, IS_WINGLAZERMAN, 0, -400, 3000, SH_W_L);
    tick(&mut g, wl);
    let coll = g.objs.aliens[wl as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    for _ in 0..7 {
        g.call_strat(coll, wl);
    }
    assert_eq!(g.objs.aldead, 0, "alive until the 8th hit");
    g.call_strat(coll, wl);
    assert_eq!(g.objs.aldead, 1, "winglazermandie -> explode");
}

// ============================================================
// walking (DSTRATS.ASM:860-964) — striding mech.
// ============================================================

#[test]
fn walking_init_sets_data_and_heading() {
    // hp200/ap16, heading 4, speed medpspeed+10=75, shadow, walk timer 200,
    // leg counters 4/4 (DSTRATS.ASM:860-872). First tick already coasts one step
    // and decrements the walk timer to 199.
    let mut g = setup();
    let wk = place(&mut g, IS_WALKING, 0, 0, 3000, SH_WALKER_0);
    tick(&mut g, wk);
    let a = g.objs.aliens[wk as usize];
    assert_eq!(a.hp, WALKING_HP, "s_set_aldata #200");
    assert_eq!(a.ap, 16, "walkingAP");
    assert_eq!(a.roty, 4, "heading 4");
    assert_eq!(a.vel, 75, "medpspeed+10");
    assert_eq!(a.sbyte1, 199, "walk timer 200 -> 199");
    assert_eq!(a.sbyte2, 4);
    assert_eq!(a.sbyte3, 4);
    assert_ne!(a.sflags & ASF_SHADOW, 0, "shadow");
    assert_eq!(a.type_, 0, "type 0 while walking (not yet zremove)");
}

#[test]
fn walking_turns_around_after_the_walk() {
    // After 200 walk frames -> walking2: type gains ATZREMOVE, it turns toward
    // deg180 (roty-=4) and decelerates (vel-=4 while >=10) (DSTRATS.ASM:881-895).
    let mut g = setup();
    let wk = place(&mut g, IS_WALKING, 0, 0, 3000, SH_WALKER_0);
    for _ in 0..230 {
        tick(&mut g, wk);
    }
    let a = g.objs.aliens[wk as usize];
    assert_ne!(a.type_ & ATZREMOVE, 0, "entered walking2 (zremove set)");
    assert!(a.vel < 75, "decelerating in walking2");
    assert_ne!(a.roty, 4, "turned away from the initial heading");
}

#[test]
fn walking_legs_topple_and_kill() {
    // Five HF1 (left-leg) hits topple the mech right -> wobble -> fallover ->
    // s_kill_obj (hp=0) (DSTRATS.ASM:897-964). Feed HF1 each tick; after the
    // topple the wobble/fall states ignore hitflags.
    let mut g = setup();
    let wk = place(&mut g, IS_WALKING, 0, 0, 3000, SH_WALKER_0);
    tick(&mut g, wk); // init
    let mut killed = false;
    for _ in 0..60 {
        g.objs.aliens[wk as usize].hitflags |= 0x01; // HF1
        tick(&mut g, wk);
        if g.objs.aliens[wk as usize].hp == 0 {
            killed = true;
            break;
        }
    }
    assert!(killed, "leg hits toppled and killed the walker (hp=0)");
}

#[test]
fn walking_death_explodes() {
    // Direct explode path (expstrat = explode_istrat).
    let mut g = setup();
    let wk = place(&mut g, IS_WALKING, 0, 0, 3000, SH_WALKER_0);
    tick(&mut g, wk);
    let exp = g.objs.aliens[wk as usize].expstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(exp, wk);
    assert_eq!(g.objs.aldead, 1, "walking explodes");
}

// ============================================================
// uperm (GA2STRAT.ASM:1112-1141) — pop-up dasher.
// ============================================================

#[test]
fn uperm_init_sets_rise_pose() {
    // hp2/ap8, enemy1+enemyweap, roty=deg180, rotx=-deg90(192), speed 70,
    // al_sword1=player_posy, snd2=2. Placed deep (worldy>=sword1+756) so it
    // just rises + spins rotz (+8) this tick (GA2STRAT.ASM:1112-1141).
    let mut g = setup();
    g.vars.player_posy = 100;
    let up = place(&mut g, IS_UPERM, 1000, 5000, 5000, SH_UPER_M);
    tick(&mut g, up);
    let a = g.objs.aliens[up as usize];
    assert_eq!(a.hp, UPERM_HP, "upermHP");
    assert_eq!(a.ap, 8, "upermAP");
    assert_eq!(a.roty, DEG180, "roty=deg180");
    assert_eq!(a.rotx, 192, "rotx=-deg90");
    assert_eq!(a.vel, 70, "still rising at speed 70");
    assert_eq!(a.sword1, 100, "captured player_posy");
    assert_eq!(a.snd2, 2);
    assert_ne!(a.collflags & COLLTYPE_ENEMY1_M, 0);
    assert_ne!(a.collflags & COLLTYPE_ENEMYWEAP_M, 0);
    assert_eq!(a.rotz, 8, "nysearch rotz += 8");
}

#[test]
fn uperm_dashes_when_leveled_and_in_range() {
    // worldy below the level-off datum AND player 400..1300 Z away -> dash:
    // speedto 80 (rate 2) + rotz += 18/tick + yaw-track (GA2STRAT.ASM:1128-1137).
    let mut g = setup();
    g.vars.player_posy = 100; // datum sword1+756 = 856
    let up = place(&mut g, IS_UPERM, 0, 0, 800, SH_UPER_M);
    for _ in 0..6 {
        g.objs.aliens[up as usize].worldy = 0; // pin below datum (< 856)
        g.objs.aliens[up as usize].worldz = 800; // pin |dz|=800 in [400,1300)
        tick(&mut g, up);
    }
    let a = g.objs.aliens[up as usize];
    assert_eq!(a.vel, 80, "dash accelerated to speed 80");
    // rotx achases -deg90 (192 == -64 i8) toward 0 the short way (through 255),
    // so it climbs 192->..., shrinking the signed pitch magnitude.
    assert!(
        (a.rotx as i8).unsigned_abs() < 64,
        "pitch leveled off toward 0 (rotx {})",
        a.rotx
    );
}

#[test]
fn uperm_death_explodes() {
    let mut g = setup();
    g.vars.player_posy = 100;
    let up = place(&mut g, IS_UPERM, 1000, 5000, 5000, SH_UPER_M);
    tick(&mut g, up);
    let coll = g.objs.aliens[up as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(coll, up); // 2 -> 1
    assert_eq!(g.objs.aldead, 0);
    g.call_strat(coll, up); // 1 -> 0 -> explode
    assert_eq!(g.objs.aldead, 1, "uperm explodes");
}

// ============================================================
// rockhard (GSTRATS.ASM:663-669) — static indestructible obstacle.
// ============================================================

#[test]
fn rockhard_init_is_static_indestructible() {
    // enemy1 collide, roty=deg180, hardHP(255)/rockhardAP(20), then s_set_strat
    // x,0 -> null tick (no further ticks, no hit/explode strats).
    let mut g = setup();
    let rk = place(&mut g, IS_ROCKHARD, 500, 0, 4000, 100);
    tick(&mut g, rk);
    let a = g.objs.aliens[rk as usize];
    assert_ne!(a.collflags & COLLTYPE_ENEMY1_M, 0, "enemy1 collide");
    assert_eq!(a.roty, DEG180, "roty=deg180");
    assert_eq!(a.hp, HARDHP, "hardHP (indestructible)");
    assert_eq!(a.ap, 20, "rockhardAP");
    assert!(a.stratptr.is_none(), "s_set_strat x,0 -> null tick");
    assert!(a.expstratptr.is_none(), "no explode strat (obstacle)");
}

// ============================================================
// Space / air-hazard family — meteo0 / big_meteor / break_meteor /
// break_meteorT / mine0 / torpedo. ASM oracle: GA2STRAT.ASM:2130-2168 (meteo0),
// D3STRATS.ASM:1069-1090 (big/break meteors) + DPATHDAT.ASM break1/break2,
// DSTRATS.ASM:1215-1246 (meteor fragment) / :1572-1582 (mine0), GASTRATS.ASM
// :2007-2044 (torpedo). No C oracle; every value hand-derived from the 65816
// source. Scope-outs asserted-around (never asserted): billboard sprite /
// flat-orient / splash / engine-up-sea cosmetics, and the unwired path VM for
// the break meteors (reduced to destructible + scroll + death trigger).
// ============================================================

const IS_TORPEDO_H: usize = 80;
const IS_METEO0: usize = 195;
const IS_BIG_METEOR: usize = 234;
const IS_BREAK_METEOR: usize = 235;
const IS_BREAK_METEORT: usize = 238;
const IS_MINE0: usize = 246;

const SH_ASTEROID2: u16 = 195; // break-meteor placement shape
const SH_METEO_0: u16 = 193; // meteo0 placement shape
const SH_ASTEROID1: u16 = 275; // meteo0 death fragment
const SH_F_FISH: u16 = 271; // torpedo surfaced shape
const SH_TADPOLE: u16 = 228; // break_meteorT death spawn

const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs:147
const ASF_COLLDISABLE: u8 = 0x10; // alien.rs:145
const COLLTYPE_ZENEMY: u8 = 0x01; // enemy_a acf_colltype6
const RNDVAL: u16 = 0x1F00; // ea_random seed slot (setup())

fn find_shape(g: &Game, shape: u16) -> Option<usize> {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| *i != 0 && a.active && a.shape == shape)
        .map(|(i, _)| i)
}

// ------------------------------------------------------------
// meteo0 (GA2STRAT.ASM:2130-2168)
// ------------------------------------------------------------

#[test]
fn meteo0_init_sets_pose_and_stays_inert_when_far() {
    // hp2/ap16, roty=deg180, anim 0, budget 20, nohitaffect; far (|dz|>=1000)
    // -> the fall-through tick returns at .nclose without growing.
    let mut g = setup();
    let m = place(&mut g, IS_METEO0, 0, 0, 5000, SH_METEO_0);
    tick(&mut g, m);
    let a = g.objs.aliens[m as usize];
    assert_eq!(a.hp, 2, "meteo0HP");
    assert_eq!(a.ap, 16, "meteo0AP");
    assert_eq!(a.roty, DEG180, "faces deg180");
    assert_eq!(a.animframe, 0, "inert: anim stays 0 when far");
    assert_eq!(a.sbyte1, 20, "fire budget primed to 20");
    assert_ne!(a.sflags & ASF_NOHITAFFECT, 0, "invulnerable while growing");
}

#[test]
fn meteo0_grows_to_max_then_sheds_invulnerability() {
    // Player within 1000 z: anim climbs +1/tick (wrap 9) to 8; nohitaffect is
    // held until anim==8 (.max), which is also the first tick sbyte1 decrements.
    let mut g = setup();
    let m = place(&mut g, IS_METEO0, 0, 0, 500, SH_METEO_0);
    // Init tick grows anim 0->1; ticks 2..8 climb it to 8 (still .grow path).
    for _ in 0..8 {
        g.objs.aliens[m as usize].worldz = 500; // keep |dz|<1000
        tick(&mut g, m);
    }
    let a = g.objs.aliens[m as usize];
    assert_eq!(a.animframe, 8, "grown to full (anim 8)");
    assert_ne!(a.sflags & ASF_NOHITAFFECT, 0, "still invulnerable at anim 8 entry");
    assert_eq!(a.sbyte1, 20, "budget untouched during growth");
    // One more tick: anim==8 -> .max clears nohitaffect and decs the budget.
    g.vars.gameframe = 1; // (gf+idx)&7 != 0 -> no fire this tick
    tick(&mut g, m);
    let a = g.objs.aliens[m as usize];
    assert_eq!(a.sflags & ASF_NOHITAFFECT, 0, "sheds nohitaffect at max");
    assert_eq!(a.sbyte1, 19, "budget decremented once maxed");
}

#[test]
fn meteo0_fires_homing_laser_on_notdelay_gate() {
    // Maxed meteo0, player close, gate (gf+idx)&7==0 -> fire RELSLOWELASERHOME
    // and consume one budget tick. idx==1 (player is slot 0), so gf=7 -> gate.
    let mut g = setup();
    let m = place(&mut g, IS_METEO0, 0, 0, 500, SH_METEO_0);
    tick(&mut g, m); // run init (stratptr -> meteo0_strat)
    g.objs.aliens[m as usize].animframe = 8;
    g.objs.aliens[m as usize].sbyte1 = 20;
    g.objs.aliens[m as usize].worldz = 500;
    g.vars.gameframe = 7; // (7 + 1) & 7 == 0
    assert!(!any_hplasma(&g), "no shot before the gate tick");
    tick(&mut g, m);
    assert!(any_hplasma(&g), "homing laser fired on the notdelay gate");
    assert_eq!(g.objs.aliens[m as usize].sbyte1, 19, "budget consumed");
}

#[test]
fn meteo0_death_spawns_meteor_fragment_and_explodes() {
    // .exp: make an asteroid1 fragment running meteor_Istrat, then explode.
    let mut g = setup();
    let m = place(&mut g, IS_METEO0, 300, 0, 4000, SH_METEO_0);
    tick(&mut g, m);
    let coll = g.objs.aliens[m as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(coll, m); // 2 -> 1 (flash)
    assert_eq!(g.objs.aldead, 0, "survives the first hit");
    g.call_strat(coll, m); // 1 -> 0 -> meteo0_exp
    assert_eq!(g.objs.aldead, 1, "meteo0 explodes on the fatal hit");
    let frag = find_shape(&g, SH_ASTEROID1).expect("asteroid1 fragment spawned");
    assert!(
        g.objs.aliens[frag].stratptr.is_some(),
        "fragment carries the meteor drift strat"
    );
    let z0 = g.objs.aliens[frag].worldz;
    tick(&mut g, frag as u16); // meteor_istrat init + first meteor_strat drift
    assert!(
        g.objs.aliens[frag].worldz < z0,
        "fragment drifts toward the viewer (worldz -= sword1)"
    );
}

// ------------------------------------------------------------
// big_meteor (D3STRATS.ASM:1069-1078)
// ------------------------------------------------------------

#[test]
fn big_meteor_is_indestructible_and_static() {
    let mut g = setup();
    let b = place(&mut g, IS_BIG_METEOR, 1300, -700, 7000, 100);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.hp, HARDHP, "hardHP (indestructible)");
    assert_eq!(a.ap, 12, "big_meteor ap 12");
    assert_ne!(a.sflags & ASF_NOHITAFFECT, 0, "nohitaffect");
    // .strat is a no-op: position unchanged across ticks (add_player_z is a
    // test no-op with pviewvelz=0, and big_meteor never scrolls anyway).
    let (x0, y0, z0) = (a.worldx, a.worldy, a.worldz);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!((a.worldx, a.worldy, a.worldz), (x0, y0, z0), "static no-op tick");
    // Indestructible: hitflash never kills a hardHP object.
    let coll = a.collstratptr.unwrap();
    g.objs.aldead = 0;
    for _ in 0..5 {
        g.call_strat(coll, b);
    }
    assert_eq!(g.objs.aldead, 0, "hardHP survives repeated hits");
    assert_eq!(g.objs.aliens[b as usize].hp, HARDHP, "hp pinned at hardHP");
}

// ------------------------------------------------------------
// break_meteor / break_meteorT (D3STRATS.ASM:1080-1090 + DPATHDAT break1/break2)
// ------------------------------------------------------------

#[test]
fn break_meteor_is_destructible_and_explodes_without_fragment() {
    let mut g = setup();
    let b = place(&mut g, IS_BREAK_METEOR, 2000, 200, 4000, SH_ASTEROID2);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.hp, 2, "meteorHP");
    assert_eq!(a.ap, 12, "meteorAP");
    let coll = a.collstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(coll, b); // 2 -> 1
    g.call_strat(coll, b); // 1 -> 0 -> explode
    assert_eq!(g.objs.aldead, 1, "break_meteor explodes");
    assert!(
        find_shape(&g, SH_TADPOLE).is_none(),
        "break_meteor (break2) spawns no tadpole"
    );
}

#[test]
fn break_meteort_death_spawns_tadpole_on_the_coin() {
    // break1.createtadpole: P_RANDOMGOTO skips on random<127, spawns otherwise.
    // RNDVAL=0 -> first draw 0x61D7 (low 0xD7=215 >= 127) -> spawn a tadpole.
    let mut g = setup();
    let b = place(&mut g, IS_BREAK_METEORT, 2000, 200, 4000, SH_ASTEROID2);
    tick(&mut g, b);
    let exp = g.objs.aliens[b as usize].expstratptr.unwrap();
    g.vars.write_ext16(RNDVAL, 0);
    g.objs.aldead = 0;
    g.call_strat(exp, b);
    assert_eq!(g.objs.aldead, 1, "break_meteorT explodes");
    assert!(
        find_shape(&g, SH_TADPOLE).is_some(),
        "spawns a tadpole on the >=127 coin"
    );
}

#[test]
fn break_meteort_death_skips_tadpole_on_the_low_coin() {
    // RNDVAL=0x1234 -> first draw 0xDA53 (low 0x53=83 < 127) -> skip the spawn.
    let mut g = setup();
    let b = place(&mut g, IS_BREAK_METEORT, 2000, 200, 4000, SH_ASTEROID2);
    tick(&mut g, b);
    let exp = g.objs.aliens[b as usize].expstratptr.unwrap();
    g.vars.write_ext16(RNDVAL, 0x1234);
    g.objs.aldead = 0;
    g.call_strat(exp, b);
    assert_eq!(g.objs.aldead, 1, "still explodes");
    assert!(
        find_shape(&g, SH_TADPOLE).is_none(),
        "no tadpole on the <127 coin"
    );
}

// ------------------------------------------------------------
// mine0 (DSTRATS.ASM:1572-1582)
// ------------------------------------------------------------

#[test]
fn mine0_init_static_destructible_then_explodes() {
    let mut g = setup();
    let m = place(&mut g, IS_MINE0, 0, -150, 4000, 0);
    let (x0, y0, z0) = {
        let a = g.objs.aliens[m as usize];
        (a.worldx, a.worldy, a.worldz)
    };
    tick(&mut g, m);
    let a = g.objs.aliens[m as usize];
    assert_eq!(a.hp, 2, "mine0HP");
    assert_eq!(a.ap, 10, "mine0AP");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 collide");
    assert_eq!((a.worldx, a.worldy, a.worldz), (x0, y0, z0), "static no-op tick");
    // Standard explosion (NOT mine2exp): two hits -> explode.
    let coll = a.collstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(coll, m); // 2 -> 1
    g.call_strat(coll, m); // 1 -> 0 -> explode
    assert_eq!(g.objs.aldead, 1, "mine0 explodes");
}

// ------------------------------------------------------------
// torpedo (GASTRATS.ASM:2007-2044)
// ------------------------------------------------------------

#[test]
fn torpedo_init_runs_submerged_and_tracks_yaw() {
    // Invisible (nullshape), colldisable, Zenemy, speed 30, hp4/ap4. Far from
    // the player (>800 z) it yaw-homes and moves but does NOT surface.
    let mut g = setup();
    let t = place(&mut g, IS_TORPEDO_H, 2000, 0, 5000, 0);
    tick(&mut g, t);
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.hp, 4, "torpedoHP");
    assert_eq!(a.ap, 4, "torpedoAP");
    assert_eq!(a.vel, 30, "speed 30");
    assert_eq!(a.shape, 0, "still submerged (nullshape)");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "non-collidable underwater");
    assert_ne!(a.collflags & COLLTYPE_ZENEMY, 0, "Zenemy collide");
    assert_ne!(a.roty, 0, "yaw turned toward the player (obj2obj_angle rate 3)");
}

#[test]
fn torpedo_surfaces_inside_800z_and_levels_pitch() {
    // Inside 800 z: torpedoa_init -> f_fish shape, pitch -deg45 (224),
    // collidable; torpedoa_strat then achases the pitch back toward 0.
    let mut g = setup();
    let t = place(&mut g, IS_TORPEDO_H, 0, 0, 500, 0);
    tick(&mut g, t); // init falls through -> torpedo_strat -> surfaces this tick
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.shape, SH_F_FISH, "surfaced to the f_fish shape");
    assert_eq!(a.sflags & ASF_COLLDISABLE, 0, "now collidable");
    let pitch0 = a.rotx;
    // rotx was set to -deg45 (224) then achased once toward 0 the short way
    // (through 255), so it should already be > 224 (climbing toward 256==0).
    assert!(pitch0 >= 224, "pitch pitched up near -deg45 ({pitch0})");
    g.objs.aliens[t as usize].worldz = 400; // stay surfaced
    tick(&mut g, t);
    let pitch1 = g.objs.aliens[t as usize].rotx;
    assert!(
        (pitch1 as i8).unsigned_abs() < (pitch0 as i8).unsigned_abs(),
        "pitch levels toward 0 ({pitch0} -> {pitch1})"
    );
}

#[test]
fn torpedo_death_explodes() {
    let mut g = setup();
    let t = place(&mut g, IS_TORPEDO_H, 0, 0, 5000, 0);
    tick(&mut g, t);
    let coll = g.objs.aliens[t as usize].collstratptr.unwrap();
    g.objs.aldead = 0;
    for _ in 0..4 {
        g.call_strat(coll, t); // hp 4 -> 0
    }
    assert_eq!(g.objs.aldead, 1, "torpedo explodes when hp hits 0");
}

// ============================================================
// Base / colony structure set-pieces
//   base0       KSTRATS.ASM:353-370
//   massivebase D2STRATS.ASM:650-681
//   colony0/1/2 GA2STRAT.ASM:1671-1779
//   colonyexit  GA2STRAT.ASM:3039-3053
// sf-map placement indices (ISTRATS.ASM def rows drift +1 past ~row 162).
// ============================================================

const IS_BASE0: usize = 138;
const IS_MASSIVEBASE: usize = 142;
const IS_COLONY0: usize = 170;
const IS_COLONY1: usize = 171;
const IS_COLONY2: usize = 172;
const IS_COLONYEXIT: usize = 236;

const ATGND_M: u8 = 1; // alien.rs:165
const DEG90: u8 = 64; // VARS.INC
const KICHI_0: u16 = 120; // shape_data #120 / sf-map SH_KICHI_0
const XPWIRESPACEBAR: u16 = 138; // sf-map consts.rs / shape_data #138
const GF_STRATDONE1: u8 = 8; // vars.rs:24
const PSF_NOCTRL: u8 = 32; // vars.rs:31
const PSF_NOFIRE: u8 = 64; // vars.rs:32
const PSF2_PLAYERHP0: u8 = 128; // vars.rs:35
const PSTF_INSEQ: u8 = 8; // vars.rs:45
const SV_VIEWCY: u16 = 0x0524; // common.rs sv::VIEWCY
const SV_VIEWPOSY: u16 = 0x0552; // common.rs sv::VIEWPOSY
const HARD_AP: u8 = 8; // STRATEQU.INC:66 hardAP
const BASE0_AP: u8 = 2; // KSTRATS.ASM:357

fn count_shape(g: &Game, shape: u16) -> usize {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| *i != 0 && a.active && a.shape == shape)
        .count()
}

// ---- base0 (KSTRATS.ASM:353-370) --------------------------

#[test]
fn base0_init_static_and_waits_far() {
    // z=6000 -> |dz| 6000 >= 2500: init sets data/facing, falls into base0_strat
    // which stays waiting (KSTRATS.ASM:362 s_jmp_Zdistless #2500 not taken).
    let mut g = setup();
    let istrat = g.world.istrats[IS_BASE0];
    let b = place(&mut g, IS_BASE0, 0, 0, 6000, 8);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.hp, HARDHP, "hardHP");
    assert_eq!(a.ap, BASE0_AP, "AP 2");
    assert_eq!(a.roty, DEG270, "faces deg270");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 collide");
    assert!(a.collstratptr.is_some(), "collide wired to the tick");
    assert!(a.expstratptr.is_some(), "explode wired to the tick");
    assert_eq!(a.animframe, 0, "closed while far");
    assert_ne!(a.stratptr, istrat, "handed the istrat off to base0_strat");
}

#[test]
fn base0_close_opens_and_caps_at_8() {
    // z=2000 < 2500: base0_strat -> base0b_strat, anim grows 0->8 and holds
    // (KSTRATS.ASM:364-370 cmp #8/beq gate over s_add_anim #1,#15).
    let mut g = setup();
    let b = place(&mut g, IS_BASE0, 0, 0, 2000, 8);
    tick(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].animframe, 1, "first open frame");
    for _ in 0..20 {
        tick(&mut g, b);
    }
    assert_eq!(g.objs.aliens[b as usize].animframe, 8, "opens fully then holds");
}

// ---- massivebase (D2STRATS.ASM:650-681) -------------------

#[test]
fn massivebase_init_indestructible_static() {
    // Far (z >= 0x3500): colldisable + hardHP/hardAP, no collide/explode ptr,
    // faces deg180, far LOD shape (D2STRATS.ASM:650-675).
    let mut g = setup();
    let b = place(&mut g, IS_MASSIVEBASE, 0, 0, 20000, KICHI_0);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.hp, HARDHP, "hardHP");
    assert_eq!(a.ap, HARD_AP, "hardAP");
    assert_eq!(a.roty, DEG180, "faces deg180");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "colldisable");
    assert!(a.collstratptr.is_none(), "no collide handler (s_set_alptrs .strat,0,0)");
    assert!(a.expstratptr.is_none(), "no explode handler");
    assert_eq!(a.shape, KICHI_0, "far LOD (kichi_1 mesh uncompiled -> kichi_0)");
}

#[test]
fn massivebase_funnels_player_when_near() {
    // Inside 3000 z: playerctrl off + drag the player toward x=0 / y=viewcy
    // (D2STRATS.ASM:663-668). viewcy=-60; player parked off-centre.
    let mut g = setup();
    g.vars.write_ext16(SV_VIEWCY, (-60i16) as u16);
    g.objs.aliens[0].worldx = 500;
    g.objs.aliens[0].worldy = 500;
    let b = place(&mut g, IS_MASSIVEBASE, 0, 0, 1000, KICHI_0);
    tick(&mut g, b);
    assert_ne!(g.vars.pshipflags & PSF_NOCTRL, 0, "control disabled");
    assert_ne!(g.vars.pshipflags & PSF_NOFIRE, 0, "fire disabled");
    assert!(g.objs.aliens[0].worldx < 500, "player dragged toward x=0");
    assert!(g.objs.aliens[0].worldy < 500, "player dragged toward y=viewcy(-60)");
    assert_eq!(g.objs.aliens[b as usize].shape, KICHI_0, "near LOD kichi_0");
}

// ---- colony0 (GA2STRAT.ASM:1671-1730) ---------------------

#[test]
fn colony0_init_flags_and_clears_stratdone() {
    // gameframe=1 so the notdelay-4 debris gate is closed; verify base init.
    let mut g = setup();
    g.vars.gameframe = 1;
    g.vars.gameflags |= GF_STRATDONE1; // must be cleared by init.
    let b = place(&mut g, IS_COLONY0, 0, 0, 5000, 0);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.hp, 10, "hp 10");
    assert_eq!(a.ap, 10, "ap 10");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 collide");
    assert_ne!(a.type_ & ATGND_M, 0, "gnd type");
    assert_eq!(a.snd2, 8, "sound2 = 8");
    assert_eq!(g.vars.gameflags & GF_STRATDONE1, 0, "GF_STRATDONE1 cleared at init");
    assert_eq!(count_shape(&g, XPWIRESPACEBAR), 0, "no debris while notdelay closed");
}

#[test]
fn colony0_far_spawns_debris_on_gate() {
    // z=2000 (>=1500, >=800) + gameframe=0 (notdelay-4 open) -> one wireframe
    // spacebar shed, oriented roty=deg90 (GA2STRAT.ASM:1714-1723).
    let mut g = setup();
    g.vars.gameframe = 0;
    let b = place(&mut g, IS_COLONY0, 0, 0, 2000, 0);
    tick(&mut g, b);
    assert_eq!(count_shape(&g, XPWIRESPACEBAR), 1, "ambient debris spawned");
    let dbr = g
        .objs
        .aliens
        .iter()
        .find(|a| a.active && a.shape == XPWIRESPACEBAR)
        .unwrap();
    assert_eq!(dbr.roty, DEG90, "debris roty = deg90");
    assert_ne!(dbr.sflags & ASF_COLLDISABLE, 0, "debris colldisable proxy");
}

#[test]
fn colony0_latches_stratdone_when_player_passes() {
    // Player alive, within 800 z, and PAST the colony (self.z < player.z) ->
    // objinfront false -> latch sflag1 + GF_STRATDONE1 (GA2STRAT.ASM:1708-1711).
    let mut g = setup();
    g.objs.aliens[0].worldz = 300; // player ahead of the colony.
    let b = place(&mut g, IS_COLONY0, 0, 0, 100, 0);
    tick(&mut g, b);
    assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0, "stratdone latched");
    assert_ne!(g.objs.aliens[b as usize].sflags2 & ASF2_SFLAG1, 0, "sflag1 latched");
    assert_ne!(g.objs.aliens[b as usize].sflags & ASF_COLLDISABLE, 0, "collide off inside");
    assert_ne!(g.vars.pshipflags & PSF_NOCTRL, 0, "control disabled");
    assert_ne!(g.vars.pstratflags & PSTF_INSEQ, 0, "in-seq flag set");
}

#[test]
fn colony0_player_dead_skips_cutscene() {
    // Within 800 but player HP0: disable collide then bail to cont without the
    // funnel/latch (GA2STRAT.ASM:1686-1688).
    let mut g = setup();
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    g.objs.aliens[0].worldz = 300;
    let b = place(&mut g, IS_COLONY0, 0, 0, 100, 0);
    tick(&mut g, b);
    assert_ne!(g.objs.aliens[b as usize].sflags & ASF_COLLDISABLE, 0, "collide off");
    assert_eq!(g.vars.gameflags & GF_STRATDONE1, 0, "no latch while player dead");
    assert_eq!(g.objs.aliens[b as usize].sflags2 & ASF2_SFLAG1, 0, "sflag1 not set");
}

// ---- colony1 (GA2STRAT.ASM:1734-1754) ---------------------

#[test]
fn colony1_pins_worldy_to_camera_mirror() {
    // worldy = 2*viewcy - viewposy + 50. viewcy=-60, viewposy=100 -> -170.
    let mut g = setup();
    g.vars.write_ext16(SV_VIEWCY, (-60i16) as u16);
    g.vars.write_ext16(SV_VIEWPOSY, 100u16);
    let b = place(&mut g, IS_COLONY1, 0, 999, 4000, KICHI_0);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.worldy, -170, "worldy = 2*(-60) - 100 + 50");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "colldisable");
    assert_ne!(a.type_ & ATGND_M, 0, "gnd type");
}

// ---- colony2 (GA2STRAT.ASM:1758-1779) ---------------------

#[test]
fn colony2_opens_when_player_in_front() {
    // No al_ptr link -> holds placement; +280 z each tick; player ahead
    // (player.z >= self.z) -> door anim 0->1 (GA2STRAT.ASM:1773-1777).
    let mut g = setup();
    g.objs.aliens[0].worldz = 5000; // player well in front.
    let b = place(&mut g, IS_COLONY2, 0, 0, 100, KICHI_0);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.animframe, 1, "door opened one frame");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "colldisable");
    assert_ne!(a.type_ & ATGND_M, 0, "gnd type");
}

#[test]
fn colony2_stays_shut_when_player_behind() {
    // Player behind (self.z after +280 > player.z) and |dz| >= 40 -> .nopen,
    // anim stays 0 (GA2STRAT.ASM:1773-1774).
    let mut g = setup();
    g.objs.aliens[0].worldz = 0;
    let b = place(&mut g, IS_COLONY2, 0, 0, 100, KICHI_0);
    tick(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].animframe, 0, "door stays shut");
}

// ---- colonyexit (GA2STRAT.ASM:3039-3053) ------------------

#[test]
fn colonyexit_opens_as_player_approaches_from_front() {
    // Player in front and beyond 75 z -> neither close condition -> anim 0->9.
    let mut g = setup();
    g.objs.aliens[0].worldz = 1000; // player ahead, far.
    let b = place(&mut g, IS_COLONYEXIT, 0, 0, 100, 0);
    tick(&mut g, b);
    let a = g.objs.aliens[b as usize];
    assert_eq!(a.animframe, 1, "exit door begins opening");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "colldisable");
    assert_ne!(a.type_ & ATGND_M, 0, "gnd type");
    for _ in 0..20 {
        g.objs.aliens[0].worldz = 1000;
        tick(&mut g, b);
    }
    assert_eq!(g.objs.aliens[b as usize].animframe, 9, "opens fully then holds");
}

#[test]
fn colonyexit_snaps_shut_when_player_behind() {
    // self in front of player (self.z >= player.z) -> .close resets anim to 0.
    let mut g = setup();
    g.objs.aliens[0].worldz = 50; // player behind the door.
    let b = place(&mut g, IS_COLONYEXIT, 0, 0, 100, 0);
    g.objs.aliens[b as usize].animframe = 5; // pretend partly open.
    tick(&mut g, b);
    assert_eq!(g.objs.aliens[b as usize].animframe, 0, "door snaps shut");
}
