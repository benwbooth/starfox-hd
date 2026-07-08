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

// ============================================================
// Environmental hazards — trackcorner / windmill / volcano / firepillar +
// their volplasma / volrock / volrockdown children. ASM oracle: GASTRATS.ASM
// (trackcorner:1626-1630, windmill:3528-3570), GA2STRAT.ASM (volcano:1929-2033,
// firepillar/volrockdown:2039-2127). Every expected value hand-derived from the
// 65816 source and cited inline. flypillars is aliased to pillar3 (IS 79) in
// this port's index scheme and is not exercised here (see enemies_ground.rs
// hazard section doc). Scoped-out cosmetics (particlefire, make_smoke,
// rots_flat, SLOWELASER "smoke" jets, windexp/round0p) are never asserted.
// ============================================================

const IS_TRACKCORNER: usize = 50;
const IS_WINDMILL: usize = 66;
const IS_VOLCANO: usize = 191;
const IS_FIREPILLAR: usize = 193;

const ASF2_SFLAG2: u8 = 0x20; // STRATEQU.INC:914
const DEG180_H: u8 = 128; // deg180
const HARD_AP_M: u8 = 8; // STRATEQU.INC:66 hardAP
const PLASMA_AP_M: u8 = 10; // STRATEQU.INC:86 plasmaAP
const WINDMILL_HP_M: u8 = 6; // STRATEQU.INC:102
const WINDMILL_AP_M: u8 = 4; // STRATEQU.INC:103

/// Active aliens that are neither the player (slot 0) nor `hazard` — i.e. the
/// spawned projectile children.
fn children(g: &Game, hazard: u16) -> Vec<u16> {
    (0..NUMBER_AL as u16)
        .filter(|&i| i != 0 && i != hazard && g.objs.aliens[i as usize].active)
        .collect()
}

// ---- trackcorner (GASTRATS.ASM:1626-1630) -----------------

#[test]
fn trackcorner_is_inert_static_marker() {
    // alptrs all 0, aldata hardHP/ap0, no colltype -> render-only scenery.
    let mut g = setup();
    let t = place(&mut g, IS_TRACKCORNER, 0, 0, 1000, 0);
    tick(&mut g, t);
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.hp, HARDHP, "indestructible hardHP");
    assert_eq!(a.ap, 0, "ap 0");
    assert!(a.stratptr.is_none(), "no tick strat (alptrs 0)");
    assert!(a.collstratptr.is_none(), "no collide strat");
    assert!(a.expstratptr.is_none(), "no explode strat");
    assert_eq!(a.collflags & COLLTYPE_ENEMY1, 0, "no colltype set");
}

// ---- windmill (GASTRATS.ASM:3528-3570) --------------------

#[test]
fn windmill_init_then_spins_in_range() {
    // z1000 -> |dz| in [500,2000); gameframe0 -> notdelay-1 gate open. Init data
    // (hp6/ap4/vel50/snd$f), roty += sword1, rotz += 4.
    let mut g = setup();
    g.vars.gameframe = 0;
    let w = place(&mut g, IS_WINDMILL, 0, 0, 1000, 0);
    g.objs.aliens[w as usize].sword1 = 5; // al_word1 Y-rot add (map datum)
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.hp, WINDMILL_HP_M, "windmillHP");
    assert_eq!(a.ap, WINDMILL_AP_M, "windmillAP");
    assert_eq!(a.vel, 50, "speed 50");
    assert_eq!(a.snd2, 0x0f, "set_sound2 $f");
    assert_eq!(a.roty, 5, "roty += sword1 (in-range gate)");
    assert_eq!(a.rotz, 4, "blades spin rotz += 4");
}

#[test]
fn windmill_no_body_spin_out_of_range() {
    // z3000 -> |dz| = 3000 >= 2000 -> OUTZdistrng skips the roty add; blades
    // (rotz += 4) still spin every tick.
    let mut g = setup();
    g.vars.gameframe = 0;
    let w = place(&mut g, IS_WINDMILL, 0, 0, 3000, 0);
    g.objs.aliens[w as usize].sword1 = 5;
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.roty, 0, "no body turn when out of [500,2000) z");
    assert_eq!(a.rotz, 4, "blades still spin");
}

// ---- volcano (GA2STRAT.ASM:1929-1966) ---------------------

#[test]
fn volcano_init_flags() {
    // z100 (|dz|<600 out of range) + gameframe1 (both notdelay gates shut) ->
    // no spawn; just the init datum. hardHP/hardAP, enemy1, roty=deg180, no
    // collide/explode ptr, sflag1 clear.
    let mut g = setup();
    g.vars.gameframe = 1; // (1+1)&7=2, (1+1)&15=2 -> gates closed
    let v = place(&mut g, IS_VOLCANO, 0, 0, 100, 0);
    tick(&mut g, v);
    let a = g.objs.aliens[v as usize];
    assert_eq!(a.hp, HARDHP, "hardHP indestructible");
    assert_eq!(a.ap, HARD_AP_M, "hardAP");
    assert_eq!(a.roty, DEG180_H, "faces deg180");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 colltype");
    assert!(a.collstratptr.is_none(), "alptrs coll = 0");
    assert!(a.expstratptr.is_none(), "alptrs exp = 0");
    assert_eq!(a.sflags2 & ASF2_SFLAG1, 0, "sflag1 cleared");
    assert!(children(&g, v).is_empty(), "no children when gates shut");
}

#[test]
fn volcano_in_range_spawns_plasma_and_rock_and_rumbles() {
    // z2000 -> |dz| in [600,4000); gameframe15 -> (15+1)&15==0 (notdelay-4) and
    // (15+1)&7==0 (notdelay-3) both fire -> a volplasma AND a volrock spawn at
    // pose worldy-120, and sflag1 latches (rumble).
    let mut g = setup();
    g.vars.gameframe = 15;
    let v = place(&mut g, IS_VOLCANO, 0, 0, 2000, 0);
    tick(&mut g, v);
    assert_ne!(
        g.objs.aliens[v as usize].sflags2 & ASF2_SFLAG1,
        0,
        "in-range rumble latches sflag1"
    );
    let kids = children(&g, v);
    assert_eq!(kids.len(), 2, "volplasma + volrock spawned");
    for c in kids {
        assert_eq!(
            g.objs.aliens[c as usize].worldy, -120,
            "child at volcano worldy - 30<<2"
        );
    }
}

#[test]
fn volcano_out_of_range_throws_only_volrock() {
    // z5000 -> |dz| = 5000 >= 4000 (out of range): the sound + plasma block is
    // skipped, but the .nfire branch lands on the volrock notdelay-3 gate, which
    // still fires. gameframe15 -> (15+1)&7==0.
    let mut g = setup();
    g.vars.gameframe = 15;
    let v = place(&mut g, IS_VOLCANO, 0, 0, 5000, 0);
    tick(&mut g, v);
    assert_eq!(
        g.objs.aliens[v as usize].sflags2 & ASF2_SFLAG1,
        0,
        "no rumble when out of range"
    );
    assert_eq!(children(&g, v).len(), 1, "only the ballistic volrock");
}

// ---- volplasma / volrock children (GA2STRAT.ASM:1969-2033) -

#[test]
fn volplasma_child_init_and_coasts() {
    // Spawn a volplasma via an in-range volcano, then tick the child through its
    // init. hp2/plasmaAP, shadow, roty=deg180 (the ROM al_sbyte1 aim; player far
    // >500 z so no homing this tick), and it moves (worldz changes from 2000).
    let mut g = setup();
    g.vars.gameframe = 15;
    let v = place(&mut g, IS_VOLCANO, 0, 0, 2000, 0);
    tick(&mut g, v);
    // The volplasma is the notdelay-4 child (spawned first); it starts at z2000,
    // y-120. Player at z0 -> |dz| 2000 > 500, so no homing: the ROM aim
    // (roty=deg180, rotx=-deg90) fires it straight up (vz=0), so the double
    // add_vecs2pos moves it on Y only.
    let c = children(&g, v)[0];
    let y0 = g.objs.aliens[c as usize].worldy;
    let z0 = g.objs.aliens[c as usize].worldz;
    tick(&mut g, c);
    let a = g.objs.aliens[c as usize];
    assert_eq!(a.hp, 2, "volplasma hp2");
    assert_eq!(a.ap, PLASMA_AP_M, "plasmaAP");
    assert_ne!(a.sflags & ASF_SHADOW, 0, "shadow flag");
    assert_eq!(a.roty, DEG180_H, "no homing beyond 500 z (ROM sbyte1 aim)");
    assert_eq!(a.worldz, z0, "vertical shot -> vz 0");
    assert_ne!(a.worldy, y0, "coasts on Y (double add_vecs2pos)");
}

#[test]
fn volrock_child_launches_upward_ballistic() {
    // The volrock child gets a random upward launch vy = -(rnd&15)-30 (in
    // [-45,-30]) and hp2/plasmaAP/shadow. Tick it through its init.
    let mut g = setup();
    g.vars.gameframe = 15;
    let v = place(&mut g, IS_VOLCANO, 0, 0, 5000, 0); // out of range -> only volrock
    tick(&mut g, v);
    let c = children(&g, v)[0];
    tick(&mut g, c);
    let a = g.objs.aliens[c as usize];
    assert_eq!(a.hp, 2, "volrock hp2");
    assert_eq!(a.ap, PLASMA_AP_M, "plasmaAP");
    assert_ne!(a.sflags & ASF_SHADOW, 0, "shadow flag");
    // vy after the leading falldown gravity (+2): launch in [-45,-30] then +2.
    assert!(
        a.vy >= -45 && a.vy <= -28,
        "upward ballistic launch, got vy={}",
        a.vy
    );
}

// ---- firepillar (GA2STRAT.ASM:2039-2090) ------------------

#[test]
fn firepillar_init_faces_upside_down() {
    // roty=deg180, rotz=deg180 (upside-down), hardHP/hardAP, enemy1, no
    // collide/explode ptr.
    let mut g = setup();
    g.vars.gameframe = 1; // keep the init-tick rock gate shut ((1+1)&3=2)
    let f = place(&mut g, IS_FIREPILLAR, 0, 0, 400, 0);
    tick(&mut g, f);
    let a = g.objs.aliens[f as usize];
    assert_eq!(a.roty, DEG180_H, "faces deg180");
    assert_eq!(a.rotz, DEG180_H, "rotz deg180 (upside down)");
    assert_eq!(a.hp, HARDHP, "hardHP");
    assert_eq!(a.ap, HARD_AP_M, "hardAP");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 colltype");
    assert!(a.collstratptr.is_none(), "alptrs coll = 0");
}

#[test]
fn firepillar_inert_when_sflag2_set() {
    // sflag2 pillars branch straight to END: no particle latch, no rock, even
    // with the gate open and the player in range.
    let mut g = setup();
    g.vars.gameframe = 1;
    let f = place(&mut g, IS_FIREPILLAR, 0, 0, 400, 0);
    tick(&mut g, f); // init (rock gate shut)
    let base = children(&g, f).len();
    // Force inert + open the rock gate.
    g.objs.aliens[f as usize].sflags2 |= ASF2_SFLAG2;
    g.objs.aliens[f as usize].sflags2 &= !ASF2_SFLAG1;
    g.vars.gameframe = 3; // (3+1)&3==0
    tick(&mut g, f);
    assert_eq!(children(&g, f).len(), base, "inert pillar spawns nothing");
    assert_eq!(
        g.objs.aliens[f as usize].sflags2 & ASF2_SFLAG1,
        0,
        "inert pillar never latches sflag1"
    );
}

#[test]
fn firepillar_active_latches_and_drops_rock() {
    // Active pillar within 800 z on the notdelay-2 gate: latches sflag1 (within
    // 1000 z rumble) AND drops a volrockdown.
    let mut g = setup();
    g.vars.gameframe = 1;
    let f = place(&mut g, IS_FIREPILLAR, 0, -200, 400, 0);
    tick(&mut g, f);
    // Force active (clear the inert + particle latches).
    g.objs.aliens[f as usize].sflags2 &= !(ASF2_SFLAG2 | ASF2_SFLAG1);
    let base = children(&g, f).len();
    g.vars.gameframe = 3; // (3+1)&3==0 -> notdelay-2 gate open
    tick(&mut g, f);
    assert_ne!(
        g.objs.aliens[f as usize].sflags2 & ASF2_SFLAG1,
        0,
        "within 1000 z latches sflag1"
    );
    assert_eq!(
        children(&g, f).len(),
        base + 1,
        "within 800 z drops a volrockdown"
    );
}

#[test]
fn volrockdown_rises_scatters_then_falls_and_removes() {
    // The volrockdown child rises (vy=80) from firepillar worldy (-200) toward
    // y>=0; at apex it scatters + advances to state 1; then gravity brings it
    // down and it self-removes (ATZREMOVE) once the bounce decays.
    let mut g = setup();
    g.vars.gameframe = 1;
    let f = place(&mut g, IS_FIREPILLAR, 0, -200, 400, 0);
    tick(&mut g, f);
    g.objs.aliens[f as usize].sflags2 &= !(ASF2_SFLAG2 | ASF2_SFLAG1);
    g.vars.gameframe = 3;
    tick(&mut g, f);
    let c = *children(&g, f).last().expect("volrockdown child");
    // First child tick: init (vy=80) + leading add_vecs2pos -> worldy -200+80.
    tick(&mut g, c);
    assert_eq!(g.objs.aliens[c as usize].vy, 80, "launched downward vy=80");
    assert_eq!(g.objs.aliens[c as usize].stratstate, 0, "still rising");
    // Rise until it reaches y>=0 and flips to the fall state.
    let mut reached = false;
    for _ in 0..8 {
        tick(&mut g, c);
        if g.objs.aliens[c as usize].stratstate == 1 {
            reached = true;
            break;
        }
    }
    assert!(reached, "apex reached -> next_state to the fall state");
    // Fall + bounce decay until self-removal.
    let mut removed = false;
    for _ in 0..200 {
        tick(&mut g, c);
        if g.objs.aliens[c as usize].type_ & ATZREMOVE != 0 {
            removed = true;
            break;
        }
    }
    assert!(removed, "self-removes (remove_istrat) once the bounce decays");
}

// ============================================================
// Firing-enemy family — misspod / misstank / szaco0 / szaco5 / houdai5f.
// ASM oracle: GASTRATS.ASM (misspod:3275-3395, misstank:1319-1436),
// GA2STRAT.ASM (szaco0:329-357, szaco5:478-527), KSTRATS.ASM
// (houdai5f:588-608), STRATMAC.INC (s_goto_WPpostab:2510-2583). No C oracle.
// Every expected value is hand-derived from the 65816 source and cited inline.
// ============================================================

// misstank's ROM-correct istrat index is 51 (ISTRATS.ASM:471) — sf-map rc.rs's
// IS_MISSTANK=50 is a mislabel that collides with trackcorner (see the port's
// module doc). Registering at 51 keeps trackcorner (50) intact.
const IS_MISSTANK: usize = 51;
const IS_MISSPOD: usize = 68; // level1_5
const IS_SZACO0: usize = 130; // level1_2
const IS_SZACO5: usize = 156; // level1_3
const IS_HOUDAI5F: usize = 187; // route3 level3_7

const MISSPOD_HP: u8 = 2; // STRATEQU.INC:112
const MISSPOD_AP: u8 = 16; // STRATEQU.INC:113
const MISSTANK_HP: u8 = 4; // STRATEQU.INC:150
const MISSTANK_AP: u8 = 8; // STRATEQU.INC:151
const SZACO0_HP: u8 = 4; // STRATEQU.INC:158
const SZACO5_HP: u8 = 2; // STRATEQU.INC:162
const SZACO5_AP: u8 = 8; // STRATEQU.INC:163
const HOUDAI5_HP: u8 = 4; // KSTRATS.ASM:47
const HOUDAI5_AP: u8 = 6; // KSTRATS.ASM:48
const ATMISSILE: u8 = 2; // alien.rs
// (ASF_COLLDISABLE = 0x10 is already declared earlier in this test module.)

fn count_type(g: &Game, enemy: u16, tflag: u8) -> usize {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .filter(|(i, a)| *i != 0 && *i as u16 != enemy && a.active && a.type_ & tflag != 0)
        .count()
}

fn any_laser(g: &Game) -> bool {
    g.objs
        .aliens
        .iter()
        .enumerate()
        .any(|(i, a)| i != 0 && a.active && a.type_ & ATLASER != 0)
}

// ---------------- misspod (IS 68) ----------------

#[test]
fn misspod_init_sets_flags_and_drifts_when_far() {
    // GASTRATS.ASM:3277-3283: hp/ap, vz=-10, roty=deg180, sbyte1=rnd&3.
    let mut g = setup();
    let pod = place(&mut g, IS_MISSPOD, 0, 0, 3000, 40);
    tick(&mut g, pod); // misspod_Istrat (no fall-through)
    let a = g.objs.aliens[pod as usize];
    assert_eq!(a.hp, MISSPOD_HP, "misspodHP");
    assert_eq!(a.ap, MISSPOD_AP, "misspodAP");
    assert_eq!(a.roty, DEG180, "s_set_alvar roty,#deg180");
    assert_eq!(a.vz, -10, "s_set_vecs #0,#0,#-10");
    assert!(a.sbyte1 <= 3, "sbyte1 = rnd&3 pattern select");
    assert!(a.expstratptr.is_some(), "explode wired");
    // Far (z3000, dist_xz ~1000+): misspod_strat drifts + rolls rotz.
    let rotz0 = g.objs.aliens[pod as usize].rotz;
    tick(&mut g, pod);
    assert_eq!(
        g.objs.aliens[pod as usize].rotz,
        rotz0.wrapping_add(5),
        "endmisspod: s_add_alvar rotz,#5"
    );
    assert!(g.objs.aliens[pod as usize].active, "still alive when far");
}

#[test]
fn misspod_close_fires_five_missiles_and_self_destructs() {
    // GASTRATS.ASM:3288 s_jmp_distless #1000 -> misspoda: 5x s_fire_weapon
    // missile2 then s_kill_obj x (hp0 + colldisable).
    let mut g = setup();
    let pod = place(&mut g, IS_MISSPOD, 0, 0, 800, 40); // dist_xz(800) ~675 < 1000
    tick(&mut g, pod); // init
    tick(&mut g, pod); // misspod_strat -> misspoda burst
    assert_eq!(count_type(&g, pod, ATMISSILE), 5, "5x missile2 burst");
    let a = g.objs.aliens[pod as usize];
    assert_eq!(a.hp, 0, "s_kill_obj: hp=0");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "s_kill_obj: colldisable");
}

// ---------------- misstank (IS 50) ----------------

#[test]
fn misstank_init_builds_carrier_and_counts_down() {
    // GASTRATS.ASM:1327-1342 init falls through into the strat (no s_end_strat),
    // so sbyte1 (20) is decremented once this same tick.
    let mut g = setup();
    let tank = place(&mut g, IS_MISSTANK, 0, 0, 3000, 60);
    tick(&mut g, tank);
    let a = g.objs.aliens[tank as usize];
    assert_eq!(a.hp, MISSTANK_HP, "misstankHP");
    assert_eq!(a.ap, MISSTANK_AP, "misstankAP");
    assert_eq!(a.vel, 30, "s_set_speed #30");
    assert_eq!(a.sbyte1, 19, "sbyte1 20 set, strat decremented once");
    assert_ne!(a.ptr, 0, "al_ptr holds the small_m carrier");
    let child = (a.ptr - 1) as usize;
    assert_eq!(g.objs.aliens[child].hp, 4, "carrier hp4");
    assert_eq!(a.sflags2 & ASF2_SFLAG1, 0, "not launched while far");
}

#[test]
fn misstank_launches_missile_when_player_close() {
    // GASTRATS.ASM:1362-1370: within 1000 z -> child becomes woodsgo missile,
    // speed 60, sflag1 latched.
    let mut g = setup();
    let tank = place(&mut g, IS_MISSTANK, 0, 0, 500, 60);
    tick(&mut g, tank); // init falls through -> launch this tick
    let a = g.objs.aliens[tank as usize];
    assert_ne!(a.sflags2 & ASF2_SFLAG1, 0, "sflag1 latched after launch");
    let child = (a.ptr - 1) as usize;
    assert_eq!(g.objs.aliens[child].vel, 60, "launched missile speed 60");
    assert!(g.objs.aliens[child].stratptr.is_some(), "child now woodsgo_strat");
}

#[test]
fn misstank_death_kills_unlaunched_carrier() {
    // GASTRATS.ASM:1319-1325: misstankexp kills the carried missile if unlaunched.
    let mut g = setup();
    let tank = place(&mut g, IS_MISSTANK, 0, 0, 3000, 60); // far -> not launched
    tick(&mut g, tank);
    let child = (g.objs.aliens[tank as usize].ptr - 1) as usize;
    let exp = g.objs.aliens[tank as usize].expstratptr.expect("exp wired");
    g.call_strat(exp, tank);
    assert_eq!(g.objs.aliens[child].hp, 0, "s_kill_obj carrier on death");
}

// ---------------- szaco0 (IS 130) ----------------

#[test]
fn szaco0_init_picks_path_and_arms_timer() {
    // GA2STRAT.ASM:329-337 falls through into szaco0_strat; sword1 (340) is
    // decremented once this tick.
    let mut g = setup();
    let z = place(&mut g, IS_SZACO0, 0, 0, 3000, 42);
    tick(&mut g, z);
    let a = g.objs.aliens[z as usize];
    assert_eq!(a.hp, SZACO0_HP, "Szaco0HP");
    assert!(a.sbyte1 <= 3, "rnd&3 flight-path select");
    assert_eq!(a.sword1, 339, "sword1 340 set, decremented once");
    assert!(a.expstratptr.is_some(), "explode wired");
}

#[test]
fn szaco0_navigates_toward_waypoint() {
    // s_goto_WP s_speedto #40: velocity ramps up from 0 as it flies the path.
    let mut g = setup();
    let z = place(&mut g, IS_SZACO0, 0, 0, 3000, 42);
    for _ in 0..4 {
        tick(&mut g, z);
    }
    assert!(g.objs.aliens[z as usize].vel > 0, "speed_to ramps toward 40");
}

#[test]
fn szaco0_fires_at_the_fire_waypoint() {
    // Every table's waypoint 1 carries wp_fire; in state 1 on the notdelay(2)
    // gate it fires RELSLOWELASER at the player (STRATMAC.INC:2545-2554).
    let mut g = setup();
    let z = place(&mut g, IS_SZACO0, 0, 0, 900, 42);
    tick(&mut g, z); // init -> stratptr = szaco0_strat
    g.objs.aliens[z as usize].stratstate = 1; // waypoint 1 (wp_fire)
    g.vars.gameframe = 0; // notdelay(2) passes
    tick(&mut g, z);
    assert!(any_laser(&g), "wp_fire spawns a RELSLOWELASER");
}

// ---------------- szaco5 (IS 156) ----------------

#[test]
fn szaco5_init_sets_flags_state0() {
    // GA2STRAT.ASM:478-488: hp/ap, speed 30, deg180 facing, anim active.
    let mut g = setup();
    let z = place(&mut g, IS_SZACO5, 0, 0, 4000, 129); // far: no fire, stays state 0
    tick(&mut g, z);
    let a = g.objs.aliens[z as usize];
    assert_eq!(a.hp, SZACO5_HP, "Szaco5HP");
    assert_eq!(a.ap, SZACO5_AP, "Szaco5AP");
    assert_eq!(a.vel, 30, "s_set_speed #30");
    assert_eq!(a.animframe & 0x80, 0x80, "s_init_anim active flag");
    assert_eq!(a.stratstate, 0, "far (>=1500 z) stays in the aim state");
    assert!(a.expstratptr.is_some(), "explode wired");
}

#[test]
fn szaco5_fires_and_advances_when_in_range() {
    // GA2STRAT.ASM:491-499: within 1500 z it aims, fires RELSLOWELASER, and
    // s_next_state advances out of state 0.
    let mut g = setup();
    let z = place(&mut g, IS_SZACO5, 0, 0, 1000, 129);
    tick(&mut g, z); // init falls through into state 0
    assert!(any_laser(&g), "state 0 fires RELSLOWELASER within 1500 z");
    assert!(g.objs.aliens[z as usize].stratstate >= 1, "s_next_state advanced");
}

// ---------------- houdai5f (IS 187) ----------------

#[test]
fn houdai5f_init_sets_flags() {
    // KSTRATS.ASM:588-593: hp/ap, anim active. Use a frame that fails the /32
    // gate so no shot is spawned during the init tick.
    let mut g = setup();
    g.vars.gameframe = 1; // (gf & 31) != 0 -> no fire
    let h = place(&mut g, IS_HOUDAI5F, 0, 0, 3000, 44);
    tick(&mut g, h);
    let a = g.objs.aliens[h as usize];
    assert_eq!(a.hp, HOUDAI5_HP, "houdai5HP");
    assert_eq!(a.ap, HOUDAI5_AP, "houdai5AP");
    assert_eq!(a.animframe & 0x80, 0x80, "s_init_anim active flag");
    assert!(a.expstratptr.is_some(), "explode wired");
    assert!(!any_laser(&g), "gate closed -> no shot");
}

#[test]
fn houdai5f_fires_homing_hplasma_when_far() {
    // KSTRATS.ASM:596-606: (gf&31)==0 and |dz|>=400 -> homing Hplasma, speed/
    // life 100.
    let mut g = setup();
    g.vars.gameframe = 0; // gate open
    let h = place(&mut g, IS_HOUDAI5F, 0, 0, 3000, 44); // |dz|=3000 >= 400
    tick(&mut g, h);
    assert!(any_laser(&g), "fires a homing Hplasma when far");
    let shot = g
        .objs
        .aliens
        .iter()
        .enumerate()
        .find(|(i, a)| *i != 0 && a.active && a.type_ & ATLASER != 0)
        .map(|(_, a)| *a)
        .expect("shot");
    assert_eq!(shot.vel, 100, "s_set_speed y,#100");
    assert_eq!(shot.count, 100, "s_set_lifecnt y,#100");
}

#[test]
fn houdai5f_holds_fire_when_player_close() {
    // KSTRATS.ASM:598 s_jmp_Zdistless #400 -> no fire when the player is near.
    let mut g = setup();
    g.vars.gameframe = 0;
    let h = place(&mut g, IS_HOUDAI5F, 0, 0, 200, 44); // |dz|=200 < 400
    tick(&mut g, h);
    assert!(!any_laser(&g), "no shot when the player is within 400 z");
}

// ============================================================
// Door / wall / tree / woods scenery family.
// ASM oracle: GASTRATS.ASM (woods:1381-1435), D2STRATS.ASM (kdoor:686-721),
// DSTRATS.ASM (walls:968-1053, trees:1976-2063). ISTRAT rows: see the port's
// section doc (woods=54, kdoor=140[ROM], kdoor2=141, wall{leftright,l,r}=
// 75/76/77[ROM], tree1=204, tree2=205). Every expected value hand-derived from
// the 65816 source. Scoped-out (never asserted): door sounds, trigse/movewall
// sound, launch-flash/smoke meshes, wall_l/wall_r cosmetic mesh swap, and the
// sprouty segment-chain / leaf-flower bloom for trees.
// ============================================================

const IS_WOODS: usize = 54;
const IS_KDOOR: usize = 140;
const IS_KDOOR2: usize = 141;
const IS_WALLLEFTRIGHT: usize = 75;
const IS_WALLL: usize = 76;
const IS_WALLR: usize = 77;
const IS_TREE1: usize = 204;
const IS_TREE2: usize = 205;

const SH_MISS_1_2: u16 = 9; // route3 common.rs SH_MISS_1_2 (woods)
const SH_K_DOOR: u16 = 118; // rc.rs SH_K_DOOR
const SH_STALK: u16 = 209; // route3 common.rs SH_STALK (trees)
const SH_KICHI_0: u16 = 120; // massivebase mesh (kdoor2 removes it)

const WOODS_HP: u8 = 2; // STRATEQU.INC:148
const WOODS_AP: u8 = 8; // STRATEQU.INC:149
const WALL1_AP: u8 = 16; // STRATEQU.INC:210
const TREE1_AP: u8 = 8; // DSTRATS.ASM:99
// (HARD_AP / ASF_NOHITAFFECT / ASF_COLLDISABLE / ASF_SHADOW / PSF_NOCTRL /
// PSF_NOFIRE are already defined earlier in this test module.)
const DEG22: u8 = 16; // deg360/16
const DEG45: u8 = 32; // enemy_a DEG45

// -------------------- woods (GASTRATS.ASM:1381-1435) --------------------

#[test]
fn woods_init_is_zenemy_obstacle_while_far() {
    // Player z0, woods z3000 -> |dz|=3000 >= 2100 -> stays woods_strat
    // (GASTRATS.ASM:1389 s_jmp_Zdistless #2100 falls through to s_end_strat).
    let mut g = setup();
    let w = place(&mut g, IS_WOODS, 0, 0, 3000, SH_MISS_1_2);
    tick(&mut g, w); // init -> falls into woods_strat (stays, |dz| >= 2100)
    let launcher = g.objs.aliens[w as usize].stratptr;
    tick(&mut g, w); // still far -> stratptr stable, not converted
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.hp, WOODS_HP, "s_set_aldata #woodsHP");
    assert_eq!(a.ap, WOODS_AP, "s_set_aldata #woodsAP");
    assert_ne!(a.collflags & COLLTYPE_ZENEMY, 0, "s_set_colltype Zenemy");
    assert_eq!(a.stratptr, launcher, "still the woods launcher while far");
    assert_eq!(a.snd2, 0, "not converted (woodsgo_init would set snd2=2)");
    assert!(a.collstratptr.is_some(), "hitflash wired");
    assert!(a.expstratptr.is_some(), "woodsexp wired");
}

#[test]
fn woods_converts_to_homing_missile_when_close() {
    // |dz|=1000 < 2100 -> woodsgo_init: swap tick, arm the 10-frame home timer,
    // sound2=2 (GASTRATS.ASM:1392-1398).
    let mut g = setup();
    let istrat = g.world.istrats[IS_WOODS];
    let w = place(&mut g, IS_WOODS, 0, 0, 1000, SH_MISS_1_2);
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_ne!(a.stratptr, istrat, "converted out of the launcher");
    assert!(a.stratptr.is_some());
    assert_eq!(a.sbyte1, 10, "s_set_alvar al_sbyte1,#10");
    assert_eq!(a.snd2, 2, "set_sound2 #$2");
}

#[test]
fn woods_death_explodes() {
    // woodsexp_Istrat -> remove (no) child + explode -> aldead
    // (GASTRATS.ASM:1431-1435).
    let mut g = setup();
    let w = place(&mut g, IS_WOODS, 0, 0, 3000, SH_MISS_1_2);
    tick(&mut g, w); // run init (far, stays launcher)
    let exp = g.objs.aliens[w as usize].expstratptr.unwrap();
    g.objs.aldead = 0;
    g.call_strat(exp, w);
    assert_eq!(g.objs.aldead, 1, "woodsexp routes to explode");
}

// -------------------- kdoor / kdoor2 (D2STRATS.ASM:686-721) --------------------

#[test]
fn kdoor_init_is_closed_indestructible_door() {
    // hardHP/hardAP, colldisable, anim 0; far (|dz|>=600) -> stays closed
    // (D2STRATS.ASM:690-706).
    let mut g = setup();
    let k = place(&mut g, IS_KDOOR, 0, 0, 3000, SH_K_DOOR);
    tick(&mut g, k);
    let a = g.objs.aliens[k as usize];
    assert_eq!(a.hp, HARDHP, "s_set_aldata #hardHP");
    assert_eq!(a.ap, HARD_AP, "s_set_aldata #hardAP");
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "s_set_alsflag colldisable");
    assert_eq!(a.animframe & 0x7F, 0, "closed (anim 0) while far");
}

#[test]
fn kdoor_opens_when_player_close_then_clamps() {
    // |dz|=100 < 600 -> s_add_anim x,#1,#8,.remove each tick: 0->1->..->7, then
    // clamps at 7 (D2STRATS.ASM:707-710).
    let mut g = setup();
    let k = place(&mut g, IS_KDOOR, 0, 0, 100, SH_K_DOOR);
    tick(&mut g, k); // init + first open step
    assert_eq!(g.objs.aliens[k as usize].animframe & 0x7F, 1, "anim 0->1");
    for _ in 0..20 {
        tick(&mut g, k);
    }
    assert_eq!(g.objs.aliens[k as usize].animframe & 0x7F, 7, "clamps at max-1 (7)");
}

#[test]
fn kdoor_closes_when_player_recedes() {
    // Open a few steps, then move the player far -> s_cmp_anim #0 / s_add_anim
    // x,#-1,#8 decrements toward 0 (D2STRATS.ASM:702-706).
    let mut g = setup();
    let k = place(&mut g, IS_KDOOR, 0, 0, 100, SH_K_DOOR);
    for _ in 0..4 {
        tick(&mut g, k); // open toward 4
    }
    let open = g.objs.aliens[k as usize].animframe & 0x7F;
    assert!(open >= 3, "opened up first (got {open})");
    g.objs.aliens[k as usize].worldz = 3000; // recede -> |dz| >= 600
    tick(&mut g, k);
    assert_eq!(g.objs.aliens[k as usize].animframe & 0x7F, open - 1, "closes one step");
}

#[test]
fn kdoor2_restores_control_and_removes_kichi_when_open() {
    // kdoor2 sets sflag1; once fully open .remove runs s_playerctrl on + removes
    // the kichi_0 object (D2STRATS.ASM:686-689/711-721).
    let mut g = setup();
    let door = place(&mut g, IS_KDOOR2, 0, 0, 100, SH_K_DOOR);
    let kichi = spawn(&mut g, 0, 0, 200, SH_KICHI_0);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE; // control was taken away
    for _ in 0..20 {
        tick(&mut g, door); // open fully -> .remove
    }
    assert_eq!(
        g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE),
        0,
        "s_playerctrl on cleared the noctrl/nofire bits"
    );
    assert!(!g.objs.aliens[kichi as usize].active, "kichi_0 (massivebase) removed");
}

#[test]
fn kdoor_plain_leaves_control_and_objects_alone() {
    // kdoor (no sflag1): .remove hits .noflagclr -> no playerctrl / no removal
    // (D2STRATS.ASM:712).
    let mut g = setup();
    let door = place(&mut g, IS_KDOOR, 0, 0, 100, SH_K_DOOR);
    let kichi = spawn(&mut g, 0, 0, 200, SH_KICHI_0);
    g.vars.pshipflags |= PSF_NOCTRL;
    for _ in 0..20 {
        tick(&mut g, door);
    }
    assert_ne!(g.vars.pshipflags & PSF_NOCTRL, 0, "plain kdoor never restores control");
    assert!(g.objs.aliens[kichi as usize].active, "plain kdoor removes nothing");
}

// -------------------- walls (DSTRATS.ASM:968-1053) --------------------

#[test]
fn walll_init_faces_deg180_leans_left() {
    // hardHP, wall1AP, roty deg180, nohitaffect, colanim 4, anim 1 (leans left)
    // (DSTRATS.ASM:987-998). Placed far so no swing this tick.
    let mut g = setup();
    let w = place(&mut g, IS_WALLL, 0, 0, 3000, SH_MISS_1_2);
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.hp, HARDHP, "s_set_aldata #hardHP");
    assert_eq!(a.ap, WALL1_AP, "s_set_aldata #wall1AP");
    assert_eq!(a.roty, DEG180, "s_set_alvar al_roty,#deg180");
    assert_ne!(a.sflags & ASF_NOHITAFFECT, 0, "s_set_alsflag nohitaffect");
    // init sets colanim 4, then the fall-through wall1_strat add_colanim +1 -> 5.
    assert_eq!(a.colframe & 0x7F, 5, "s_init_colanim #4 then +1 same tick");
    assert_eq!(a.animframe & 0x7F, 1, "s_init_anim #1 (leans left)");
}

#[test]
fn walll_swings_left_when_player_close() {
    // Within wall1DIST(600) xz -> walllr_i: anim 1 != 0 -> wallleft_strat swings
    // roty toward -64(=192) by +16 (DSTRATS.ASM:1027-1040).
    let mut g = setup();
    let w = place(&mut g, IS_WALLL, 0, 0, 100, SH_MISS_1_2); // |dz|=100 -> xz<600
    tick(&mut g, w); // init falls into wall1_strat -> wall_nothit -> swing latched
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.roty, DEG180.wrapping_add(16), "swung +16 toward -64");
}

#[test]
fn wallr_swings_right_when_player_close() {
    // wallr anim 0 -> wallright_strat swings roty toward +64 by -16
    // (DSTRATS.ASM:1043-1053).
    let mut g = setup();
    let w = place(&mut g, IS_WALLR, 0, 0, 100, SH_MISS_1_2);
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.animframe & 0x7F, 0, "wallr leans right (anim 0)");
    assert_eq!(a.roty, DEG180.wrapping_sub(16), "swung -16 toward +64");
}

#[test]
fn wallleftright_oscillates_lean_on_notdelay() {
    // wall2_strat toggles animframe bit0 every 16 frames (notdelay 4). Placed far
    // so wall_nothit is a no-op; drive on a /16 frame (DSTRATS.ASM:976-1019).
    let mut g = setup();
    let w = place(&mut g, IS_WALLLEFTRIGHT, 0, 0, 3000, SH_MISS_1_2);
    // First tick runs init (falls into wall2_strat once). Land the flip on a
    // notdelay(4) frame.
    g.vars.gameframe = 16; // 16 & 15 == 0
    tick(&mut g, w);
    let a = g.objs.aliens[w as usize];
    assert_eq!(a.hp, HARDHP, "wallleftright hp = -1 (indestructible)");
    assert_eq!(a.animframe & 1, 1, "lean bit flipped on the /16 gate");
}

// -------------------- trees (DSTRATS.ASM:1976-2063) --------------------

#[test]
fn tree1_init_is_indestructible_sprout_scenery() {
    // hp=tree1HP=hardHP(-1), ap=tree1AP, ENEMY1 + nohitaffect, root lowered by
    // sprout_maxy/2, sbyte1 in [1,4], anim speed 2 (DSTRATS.ASM:2016-2043).
    let mut g = setup();
    let t = place(&mut g, IS_TREE1, 0, 100, 500, SH_STALK);
    tick(&mut g, t); // init + first grow step
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.hp, HARDHP, "tree1HP = hardHP (indestructible)");
    assert_eq!(a.ap, TREE1_AP, "s_set_aldata #tree1ap");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "s_set_colltype ENEMY1");
    assert_ne!(a.sflags & ASF_NOHITAFFECT, 0, "s_set_alsflag nohitaffect");
    assert_eq!(a.worldy, 60, "root lowered by sprout_maxy/2 (100 - 40)");
    assert!((1..=4).contains(&a.sbyte1), "height (rnd&3)+1 in [1,4]");
    assert_eq!(a.sword1 as u16 & 0xff, 2, "anim speed 2 (sword1 lo)");
    assert_eq!(a.sflags & ASF_SHADOW, 0, "tree1 casts no shadow (unlike tree2)");
}

#[test]
fn tree1_trunk_grows_to_full_then_holds() {
    // sprouty.strat .notsnake grow: +2 per tick, clamp/hold at 8
    // (DSTRATS.ASM:2148-2149, scoped base-trunk grow).
    let mut g = setup();
    let t = place(&mut g, IS_TREE1, 0, 0, 500, SH_STALK);
    for _ in 0..10 {
        tick(&mut g, t);
    }
    assert_eq!(g.objs.aliens[t as usize].animframe & 0x7F, 8, "trunk grown to full and held");
}

#[test]
fn tree2_tilts_toward_player_and_casts_shadow() {
    // Player at x0. Tree2 right of player (x>0): self.worldx - px >= 0 ->
    // .notthatway -> roty += -deg45; sets shadow (DSTRATS.ASM:1993-2004). Trees
    // never set an absolute roty (unlike walls), so the tilt is relative to the
    // map-placed base (0 here).
    let mut g = setup();
    let t = place(&mut g, IS_TREE2, 500, 0, 500, SH_STALK);
    tick(&mut g, t);
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.roty, 0u8.wrapping_sub(DEG45), "tilt -deg45 (right of player)");
    assert_ne!(a.sflags & ASF_SHADOW, 0, "tree2 casts a shadow");
    assert_eq!(a.sbyte2, DEG22, "sbyte2 = +deg22 overhang (not negated)");
}

#[test]
fn tree2_tilts_other_way_when_left_of_player() {
    // Tree2 left of player (x<0): self.worldx - px < 0 -> .otherway -> neg sbyte2,
    // roty += deg45 (DSTRATS.ASM:1998-2001).
    let mut g = setup();
    let t = place(&mut g, IS_TREE2, -500, 0, 500, SH_STALK);
    tick(&mut g, t);
    let a = g.objs.aliens[t as usize];
    assert_eq!(a.roty, DEG45, "tilt +deg45 (left of player, base 0)");
    assert_eq!(a.sbyte2, DEG22.wrapping_neg(), "sbyte2 negated (-deg22)");
}

// ============================================================
// Final niche enemies — shou0/shou0a, iris, truck, item6.
// ASM oracle: GA2STRAT.ASM:1850-1897 (shou0), DSTRATS.ASM:1375-1399 (iris),
// GASTRATS.ASM:1575-1623 (truck), GASTRATS.ASM:2598-2621 (item6). No C oracle;
// every expected value hand-derived from the 65816 source and cited inline.
// ============================================================

// sf-map IS_FOO placement == sf-strat register() row (verified in the port doc).
const IS_SHOU0: usize = 178;
const IS_SHOU0A: usize = 179;
const IS_IRIS: usize = 48;
const IS_TRUCK: usize = 49;
const IS_ITEM6: usize = 176;

const SH_RAIL_4: u16 = 6; // sf-map SH_RAIL_4 (route3 common.rs:298)
const SHOU0_HP: u8 = 2; // STRATEQU.INC:249
const SHOU0_AP: u8 = 12; // STRATEQU.INC:250
const TRUCK_HP: u8 = 4; // STRATEQU.INC:142
const TRUCK_AP: u8 = 8; // STRATEQU.INC:143
// HARD_AP / DEG90 / ASF_COLLDISABLE / ASF2_SFLAG2 / PSF2_PLAYERHP0 are already
// defined earlier in this test module (reused here).
const ASF_COLLIDE_M: u8 = 0x20; // alien.rs ASF_COLLIDE
const PSF2_WIRESHIP: u8 = 2; // GILESALC.INC:85

fn any_missile(g: &Game) -> bool {
    use sf_game::alien::ATMISSILE;
    g.objs
        .aliens
        .iter()
        .enumerate()
        .any(|(i, a)| i != 0 && a.active && a.type_ & ATMISSILE != 0)
}

fn call_collide(g: &mut Game, idx: u16) {
    let c = g.objs.aliens[idx as usize]
        .collstratptr
        .expect("collstratptr");
    g.call_strat(c, idx);
}

// ---- shou0 / shou0a (GA2STRAT.ASM:1850-1897) --------------------------------

#[test]
fn shou0_init_rolls_sbyte1_and_wires() {
    // s_set_aldata shou0HP/AP, enemy1; `.again` picks sbyte1 in {0,1,2}
    // (reroll on 3). Placed far (z6000 -> |dz|>=2500) so the fall-through tick
    // does not spin/fire (GA2STRAT.ASM:1853-1859).
    let mut g = setup();
    let e = place(&mut g, IS_SHOU0, 0, 0, 6000, 100);
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_eq!(a.hp, SHOU0_HP, "shou0HP");
    assert_eq!(a.ap, SHOU0_AP, "shou0AP");
    assert_ne!(a.collflags & COLLTYPE_ENEMY1, 0, "enemy1 colltype");
    assert!(a.sbyte1 <= 2, "sbyte1 rolled into {{0,1,2}} (reroll on 3)");
    // out of [500,2500): no spin.
    assert_eq!((a.roty, a.rotx, a.rotz), (0, 0, 0), "no spin out of range");
}

#[test]
fn shou0_in_range_spins_selected_axes_and_fires() {
    // In [500,2500) z the sbyte1-selected pair advances +6; on the /16 gate
    // ((gameframe+idx)&15==0) a player-aimed PLASMA fires (GA2STRAT.ASM:1860-1896).
    let mut g = setup();
    let e = place(&mut g, IS_SHOU0, 0, 0, 1000, 100); // slot 1
    g.vars.gameframe = 15; // (15+1)&15 == 0 -> fire gate open
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    match a.sbyte1 {
        0 => assert_eq!((a.roty, a.rotx, a.rotz), (6, 6, 0), "sbyte1=0: roty+rotx"),
        1 => assert_eq!((a.roty, a.rotx, a.rotz), (6, 0, 6), "sbyte1=1: roty+rotz"),
        _ => assert_eq!((a.roty, a.rotx, a.rotz), (0, 6, 6), "sbyte1=2: rotx+rotz"),
    }
    assert!(any_hplasma(&g), "PLASMA fired on the /16 gate");
}

#[test]
fn shou0a_sets_sflag1_and_uses_slower_gate() {
    // shou0a = shou0 + sflag1 -> fires on /32, not /16. At gameframe=15,idx=1 the
    // /16 gate is open (16&15==0) but /32 is not (16&31!=0): shou0a must NOT fire
    // yet, though it still spins (GA2STRAT.ASM:1850-1852, 1892-1896).
    let mut g = setup();
    let e = place(&mut g, IS_SHOU0A, 0, 0, 1000, 100);
    assert_ne!(
        g.objs.aliens[e as usize].stratptr, None,
        "shou0a wired via init"
    );
    g.vars.gameframe = 15;
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_ne!(a.sflags2 & ASF2_SFLAG1, 0, "shou0a sets sflag1");
    // still spun this frame (in range) but held fire on the /32 cadence.
    assert!(
        (a.roty, a.rotx, a.rotz) != (0, 0, 0),
        "spun while in range"
    );
    assert!(!any_hplasma(&g), "shou0a holds fire on the /32 gate");
}

// ---- iris (DSTRATS.ASM:1375-1399) -------------------------------------------

#[test]
fn iris_init_sealed_faces_deg180() {
    // hp 127, roty deg180, anim 0, hardAP; sealed while hp>=125 so the
    // fall-through tick does not open (DSTRATS.ASM:1382-1399).
    let mut g = setup();
    let e = place(&mut g, IS_IRIS, 0, 0, 4000, 4);
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_eq!(a.hp, 127, "s_set_aldata #127");
    assert_eq!(a.ap, HARD_AP, "hardAP");
    assert_eq!(a.roty, DEG180, "faces deg180");
    assert_eq!(a.animframe & 0x7F, 0, "sealed: anim stays 0");
    assert!(a.expstratptr.is_some(), "explode wired (death path)");
}

#[test]
fn iris_opens_when_damaged_below_threshold() {
    // Drop hp below 127-irisHP=125 -> the aperture animates open 0->8
    // (DSTRATS.ASM:1395-1398).
    let mut g = setup();
    let e = place(&mut g, IS_IRIS, 0, 0, 4000, 4);
    tick(&mut g, e); // init (sealed at 127)
    g.objs.aliens[e as usize].hp = 124; // one below the 125 seal threshold
    tick(&mut g, e);
    assert_eq!(g.objs.aliens[e as usize].animframe & 0x7F, 1, "opened +1");
    // holds at the 4-arg jmp clamp (max-1 == 7) after enough ticks.
    for _ in 0..12 {
        tick(&mut g, e);
    }
    assert_eq!(g.objs.aliens[e as usize].animframe & 0x7F, 7, "holds fully open (max-1)");
}

// ---- truck (GASTRATS.ASM:1575-1623) -----------------------------------------

#[test]
fn truck_init_wires_and_generates_drive_vecs() {
    // truckHP/AP, sbyte1<-roty(0), speed 30, Zenemy, gen vecs from heading 0
    // (sin0=0 -> vx 0, cos0 max -> vz>0). s_end_strat: no movement this frame
    // (GASTRATS.ASM:1575-1583).
    let mut g = setup();
    let e = place(&mut g, IS_TRUCK, 0, 0, 4000, 5);
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_eq!(a.hp, TRUCK_HP, "truckHP");
    assert_eq!(a.ap, TRUCK_AP, "truckAP");
    assert_eq!(a.sbyte1, 0, "sbyte1 = roty(0)");
    assert_eq!(a.vel, 30, "speed 30");
    assert_ne!(a.collflags & COLLTYPE_ZENEMY, 0, "Zenemy colltype");
    assert_eq!(a.vx, 0, "heading 0: vx=0 (sin0)");
    assert!(a.vz > 0, "heading 0: vz>0 (cos0)");
    assert_eq!(a.worldz, 4000, "s_end_strat: no drive on the init frame");
}

#[test]
fn truck_fires_one_homing_missile_in_range() {
    // In 1000<=|dz|<3000, on the global /16 gate (gameframe&15==0), sflag2 clear,
    // fire ONE homing HMISSILE1 and latch sflag2 (GASTRATS.ASM:1591-1602).
    let mut g = setup();
    let e = place(&mut g, IS_TRUCK, 0, 0, 2000, 5);
    tick(&mut g, e); // init only (s_end_strat)
    g.vars.gameframe = 0; // notdelay(4): 0&15==0
    tick(&mut g, e); // truck_strat -> truck_norm -> fire
    assert!(any_missile(&g), "HMISSILE1 fired in range");
    assert_ne!(g.objs.aliens[e as usize].sflags2 & ASF2_SFLAG2, 0, "sflag2 latched");
    // second in-range gate: no second missile (one-shot).
    let missiles = |g: &Game| {
        use sf_game::alien::ATMISSILE;
        g.objs
            .aliens
            .iter()
            .enumerate()
            .filter(|(i, a)| *i != 0 && a.active && a.type_ & ATMISSILE != 0)
            .count()
    };
    let n1 = missiles(&g);
    tick(&mut g, e);
    assert_eq!(missiles(&g), n1, "sflag2 blocks a second missile");
}

#[test]
fn truck_turns_on_rail_hit_and_hitflashes_otherwise() {
    // truckcol: hitting a rail_4 snaps onto it, sets sflag1, and turns the heading
    // by +deg90 (rail sbyte1 != 1). A non-rail partner takes a normal hit and does
    // NOT turn (GASTRATS.ASM:1606-1623).
    let mut g = setup();
    let e = place(&mut g, IS_TRUCK, 0, 0, 2000, 5);
    tick(&mut g, e); // init
    g.objs.aliens[e as usize].sbyte1 = 0;
    g.objs.aliens[e as usize].sflags |= ASF_COLLIDE_M; // engine sets this on contact
    // Rail partner (shape 6, sbyte1=0 -> the .not_right +deg90 branch).
    let rail = spawn(&mut g, 500, 0, 2000, SH_RAIL_4);
    g.objs.aliens[rail as usize].sbyte1 = 0;
    g.objs.aliens[e as usize].collobjptr = rail;
    call_collide(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_eq!(a.sbyte1, DEG90, "turned +deg90 on rail (rail sbyte1 != 1)");
    assert_ne!(a.sflags2 & ASF2_SFLAG1, 0, "sflag1 debounce set");
    assert_eq!(a.sflags & ASF_COLLIDE_M, 0, "collide flag cleared");
    // truck_cont falls into truck_norm, which drives (add_vecs2pos) the same
    // frame, so worldx is the snapped rail x (500) plus one step of the new
    // heading's velocity — not exactly 500. The turn + snap having happened is
    // what matters; assert it left the origin.
    assert_ne!(a.worldx, 0, "moved off the origin after snapping to the rail");

    // Non-rail collision: normal hit, no turn.
    let mut g2 = setup();
    let e2 = place(&mut g2, IS_TRUCK, 0, 0, 2000, 5);
    tick(&mut g2, e2);
    g2.objs.aliens[e2 as usize].sbyte1 = 33;
    let foe = spawn(&mut g2, 100, 0, 2000, 999); // not rail_4
    g2.objs.aliens[e2 as usize].collobjptr = foe;
    call_collide(&mut g2, e2);
    let b = g2.objs.aliens[e2 as usize];
    assert_eq!(b.sbyte1, 33, "non-rail: heading unchanged");
    assert_eq!(b.sflags2 & ASF2_SFLAG1, 0, "non-rail: no turn debounce");
}

// ---- item6 (GASTRATS.ASM:2598-2621) -----------------------------------------

#[test]
fn item6_init_colldisable_and_drifts() {
    // colldisable; sbyte1==0 -> worldz+=20, roty+=4. Placed far so no pickup
    // (GASTRATS.ASM:2598-2610).
    let mut g = setup();
    let e = place(&mut g, IS_ITEM6, 0, 0, 5000, 160);
    tick(&mut g, e);
    let a = g.objs.aliens[e as usize];
    assert_ne!(a.sflags & ASF_COLLDISABLE, 0, "colldisable");
    assert_eq!(a.worldz, 5020, "drift +20 z (sbyte1==0)");
    assert_eq!(a.roty, 4, "spin roty +4");
    assert_eq!(a.hp, 0, "far away: not picked up (still alive-ish)");
}

#[test]
fn item6_pickup_grants_wireship_and_removes() {
    // Player close (|dz|<120, xy<60 after the +20 drift): grant the wireframe
    // ship bit, chime, self-remove (GASTRATS.ASM:2611-2620).
    let mut g = setup();
    // z90 -> +20 drift -> 110 < 120 pickup window; xy 0 < 60.
    let e = place(&mut g, IS_ITEM6, 0, 0, 90, 160);
    tick(&mut g, e);
    assert_ne!(g.vars.pshipflags2 & PSF2_WIRESHIP, 0, "psf2_wireship granted");
    assert_eq!(g.objs.aldead, 1, "item removes itself on pickup");
}

#[test]
fn item6_removes_on_player_dead() {
    // s_remove_ifplayerdead: pshipflags2 & psf2_playerHP0 -> remove (GASTRATS.ASM:2603).
    let mut g = setup();
    let e = place(&mut g, IS_ITEM6, 0, 0, 5000, 160);
    tick(&mut g, e); // init
    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    tick(&mut g, e);
    assert_eq!(g.objs.aldead, 1, "removed when player HP0");
}
