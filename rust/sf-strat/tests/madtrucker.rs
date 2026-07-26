//! madtrucker boss-family behavioural tests — Route 2 L6 "trucker" road.
//!
//! ASM oracle: `madtrucker_istrat` (reference/ultrastarfox/SF/STRAT/DSTRATS.ASM:
//! 5233-5717), `madbiker_istrat` / `bike2_istrat` (:4947-5222) and
//! `barrier_istrat` (:5720-5751). Map placement: TRUCKER.ASM (boss_9_5 truck +
//! air_1 bikes), mirrored by route2/submaps.rs via STRAT_ADDR_MADTRUCKER /
//! STRAT_ADDR_MADBIKER.
//!
//! No sf-oracle differential fixture is used: the ROM boss's `.hit` gate keys
//! off per-sub-box hitflags from the boss_9_5 / boss_9_0 collision meshes.
//! These tests assert the mother mode machine, truck-body positioning, HP bar,
//! damage gate, death chain, exact route gate, escort-bike engine/hover/sparks,
//! and mine against hand-derived ASM expectations, cited inline.

use sf_game::alien::{ObjectVisualKind, AFONFIRE, ASF_INVISIBLE, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_map::consts::sh;
use sf_strat::bosses;
use sf_strat::snes_trig::{strat_roffs_pitch_yaw, SINTAB};

// Local mirrors of the private bosses.rs constants (cited to the port / ASM).
const SH_MT_BOSS_9_0: u16 = sh::BOSS_9_0;
const SH_MT_AIR_1: u16 = sh::AIR_1;
const SH_MT_BARRIER: u16 = sh::BARRIER;
const MADBIKER_SOUND: u8 = 9;
const MADBIKER_ENGINE_Z_OFFSET: i8 = -10;
const NEGATIVE_FIVE_AS_BYTE: u8 = -5i8 as u8;
const NEGATIVE_TEN_AS_BYTE: u8 = MADBIKER_ENGINE_Z_OFFSET as u8;
const MT_HF1: u8 = 0x01;
const MT_HF2: u8 = 0x02;
const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs
const ASF_SHADOW: u8 = 0x04; // alien.rs (s_set_alsflag shadow)
const ATZREMOVE: u8 = 0x08; // alien.rs type_ zremove
const ACF_COLLTYPE2: u8 = 0x10; // ROM ENEMY1 (not vars COLLTYPE_ENEMY1=0x01)
const COLLTYPE_ENEMY2: u8 = 0x20; // ROM ENEMY2 (acf_colltype3)
const MADTRUCKER_HP: u8 = 64; // DSTRATS.ASM:74
const MADBIKER_HP: u8 = 10; // DSTRATS.ASM:72

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

/// New game with a static player in slot 0 and the madtrucker mother
/// (nullshape) at `boss_z`. Returns (game, boss).
fn setup(player_z: i16, boss_z: i16) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(0x1F00, 0x1234); // RNG seed
    g.vars.internal_playpt = 0;
    bosses::register(&mut g.world);

    let p = spawn(&mut g, 0, 0, player_z, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    }
    let boss = spawn(&mut g, 0, 0, boss_z, 0); // SH_NULLSHAPE
    (g, boss)
}

/// Run the boss's current stratptr once (isolated from the rest of the graph).
fn tick(g: &mut Game, boss: u16) {
    let s = g.objs.aliens[boss as usize].stratptr.expect("stratptr");
    g.call_strat(s, boss);
}

/// Run the boss's expstrat once (the dead-object path).
fn tick_exp(g: &mut Game, boss: u16) {
    let e = g.objs.aliens[boss as usize]
        .expstratptr
        .expect("expstratptr");
    g.call_strat(e, boss);
}

fn child(g: &Game, boss: u16) -> Option<usize> {
    let raw = g.objs.aliens[boss as usize].ptr;
    if raw == 0 {
        return None;
    }
    let i = (raw - 1) as usize;
    (i < NUMBER_AL && g.objs.aliens[i].active).then_some(i)
}

fn count_shape(g: &Game, shape: u16) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
        .count()
}

fn find_shape(g: &Game, shape: u16) -> Option<usize> {
    (0..NUMBER_AL).find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
}

fn anim(g: &Game, idx: usize) -> u8 {
    g.objs.aliens[idx].animframe & 0x7f
}

// ------------------------------------------------------------
// 1. init: HP bar + maxHP, a linked hard truck body, mother is an ENEMY1
//    shadow controller with strat/hit/explode ptrs.
//    (DSTRATS.ASM:5233-5246: madtruckerHP/AP, colltype ENEMY1, shadow,
//     bossmaxHP=madtruckerHP, .generate -> boss_9_0 body via al_ptr.)
// ------------------------------------------------------------
#[test]
fn init_links_truck_body_and_sets_hp_bar() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);

    assert_eq!(
        g.vars.bossmaxhp, MADTRUCKER_HP as u16,
        "bossmaxHP = madtruckerHP"
    );
    let b = &g.objs.aliens[boss as usize];
    assert_eq!(b.hp, MADTRUCKER_HP, "mother hp = madtruckerHP");
    assert_ne!(b.collflags & ACF_COLLTYPE2, 0, "mother is ENEMY1 (0x10)");
    assert_ne!(b.sflags & ASF_SHADOW, 0, "mother casts a shadow");
    assert!(b.stratptr.is_some() && b.collstratptr.is_some() && b.expstratptr.is_some());

    let c = child(&g, boss).expect("truck body linked via al_ptr");
    assert_eq!(
        g.objs.aliens[c].shape, SH_MT_BOSS_9_0,
        "body shape = boss_9_0"
    );
    assert_eq!(g.objs.aliens[c].hp, 0xFF, "truck body is hard/invincible");
    assert_ne!(g.objs.aliens[c].collflags & ACF_COLLTYPE2, 0);
}

// ------------------------------------------------------------
// 2. mode 0 .bargeforward creeps right (+2/tick) until worldx>=30, then falls
//    through into .maketwobikes (mode 1, spawns two bikes) and .openup (mode 2).
//    (DSTRATS.ASM:5315-5328 .rightlane; :5250-5252 mode table.)
// ------------------------------------------------------------
#[test]
fn bargeforward_creeps_right_then_spawns_bikes() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    // .strat already ran once inside init (mode 0, worldx 0 -> 2).
    assert_eq!(g.objs.aliens[boss as usize].stratstate, 0);
    assert_eq!(g.objs.aliens[boss as usize].worldx, 2, ".rightlane +2");

    // Jump worldx to the lane edge; one tick secures the lane and advances
    // through mode 1 (two bikes) into mode 2 (armour opens to anim 1).
    g.objs.aliens[boss as usize].worldx = 30;
    tick(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].stratstate, 2,
        "reached .openup"
    );
    assert_eq!(
        count_shape(&g, SH_MT_AIR_1),
        2,
        ".maketwobikes spawned two bikes"
    );
    let c = child(&g, boss).unwrap();
    assert_eq!(anim(&g, c), 1, ".openback raised the armour anim to 1");
}

// ------------------------------------------------------------
// 3. truck_accel chases the player's worldz. With the player well ahead
//    (larger z), the truck accelerates forward (worldz climbs by an increasing
//    sword1 velocity, capped +10). (DSTRATS.ASM:174-197 truck_accel.)
// ------------------------------------------------------------
#[test]
fn truck_accel_closes_on_player() {
    let (mut g, boss) = setup(5000, 0);
    bosses::madtrucker_init(&mut g, boss);
    // Stay in mode 0 (.bargeforward -> .move2). worldx<30 keeps it there.
    g.objs.aliens[boss as usize].worldx = 0;
    let z0 = g.objs.aliens[boss as usize].worldz;
    tick(&mut g, boss);
    let z1 = g.objs.aliens[boss as usize].worldz;
    tick(&mut g, boss);
    let z2 = g.objs.aliens[boss as usize].worldz;
    assert!(z1 > z0 && z2 > z1, "truck accelerates toward the player");
    assert!(
        (z2 - z1) > (z1 - z0),
        "sword1 ramps up (accel), later step is larger"
    );
    assert_eq!(g.objs.aliens[boss as usize].stratstate, 0, "still barging");
}

// ------------------------------------------------------------
// 4a. damage gate: an OPEN truck (child anim!=0) whose mother HF2 weak-spot was
//     struck takes 1 hp. (DSTRATS.ASM:5580-5603 .hit -> .actualhit.)
// ------------------------------------------------------------
#[test]
fn open_weakspot_hit_damages() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    let c = child(&g, boss).unwrap();
    g.objs.aliens[c].animframe = 0x80 | 5; // armour open
    g.objs.aliens[boss as usize].hitflags |= MT_HF2; // weak spot struck
    let coll = g.objs.aliens[boss as usize].collstratptr.unwrap();

    g.call_strat(coll, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].hp,
        MADTRUCKER_HP - 1,
        "took 1 hp"
    );
    assert_eq!(
        g.objs.aliens[boss as usize].hitflags & MT_HF2,
        0,
        "HF2 consumed"
    );
    assert_eq!(g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT, 0);
}

// ------------------------------------------------------------
// 4b. damage gate: a CLOSED truck (child anim==0) is invulnerable; the mother
//     latches nohitaffect and takes no damage. (DSTRATS.ASM:5584-5587.)
// ------------------------------------------------------------
#[test]
fn closed_armour_is_invulnerable() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    // child anim is 0 from .generate.
    g.objs.aliens[boss as usize].hitflags |= MT_HF2;
    let coll = g.objs.aliens[boss as usize].collstratptr.unwrap();

    g.call_strat(coll, boss);
    assert_eq!(g.objs.aliens[boss as usize].hp, MADTRUCKER_HP, "no damage");
    assert_ne!(g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT, 0);
}

// ------------------------------------------------------------
// 4c. damage gate: the body-armour HF1 sub-box absorbs the hit even when open,
//     consuming HF1 and dealing no damage. (DSTRATS.ASM:5588-5593.)
// ------------------------------------------------------------
#[test]
fn body_armour_absorbs_hit() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    let c = child(&g, boss).unwrap();
    g.objs.aliens[c].animframe = 0x80 | 5; // open
    g.objs.aliens[c].hitflags |= MT_HF1; // armour plate struck
    g.objs.aliens[boss as usize].hitflags |= MT_HF2;
    let coll = g.objs.aliens[boss as usize].collstratptr.unwrap();

    g.call_strat(coll, boss);
    assert_eq!(g.objs.aliens[boss as usize].hp, MADTRUCKER_HP, "no damage");
    assert_eq!(
        g.objs.aliens[c].hitflags & MT_HF1,
        0,
        "HF1 absorbed & cleared"
    );
    assert_ne!(
        g.objs.aliens[boss as usize].hitflags & MT_HF2,
        0,
        "HF2 untouched"
    );
}

// ------------------------------------------------------------
// 5. death chain: a fatal hit routes to .explode (child flagged zremove, 35-tick
//    swerve armed), the swerve counts down to the .skid state (maptrigger bit2 +
//    sword1=20), then the wreck is removed once the player drives past it.
//    (DSTRATS.ASM:5616-5667.)
// ------------------------------------------------------------
#[test]
fn fatal_hit_runs_swerve_skid_death() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    let c = child(&g, boss).unwrap();
    g.objs.aliens[c].animframe = 0x80 | 5; // open
    g.objs.aliens[boss as usize].hp = 1;
    g.objs.aliens[boss as usize].hitflags |= MT_HF2;
    let coll = g.objs.aliens[boss as usize].collstratptr.unwrap();

    g.call_strat(coll, boss); // hp 1 -> 0 -> .explode -> one swerve tick
    assert_eq!(g.objs.aliens[boss as usize].hp, 0, "boss killed");
    assert_ne!(
        g.objs.aliens[c].type_ & ATZREMOVE,
        0,
        "truck body flagged zremove"
    );
    // .explode sets sbyte1=35, then .swerveviolently decs to 34.
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte1, 34,
        "swerve timer armed"
    );
    assert_ne!(
        count_shape(&g, sh::LINE_SPARK),
        0,
        "swerve emits the authored rear scrape spark"
    );

    // Drive the swerve down to 0 -> transition to .skid (its expstrat swaps,
    // maptrigger bit2 latches). The truck crept +1 in z during the init tick so
    // the player is behind and .flippul coasts sword1 rather than removing yet.
    let swerve_exp = g.objs.aliens[boss as usize].expstratptr.unwrap();
    for _ in 0..34 {
        tick_exp(&mut g, boss);
    }
    assert_ne!(
        g.vars.read_ext8(0x0311) & 2,
        0,
        "maptrigger bit2 set at skid"
    );
    assert_ne!(
        g.objs.aliens[boss as usize].expstratptr.unwrap(),
        swerve_exp,
        "expstrat advanced from .swerveviolently to .skid"
    );

    // Player drives past the wreck (player_z >= truck_z) -> s_remove_obj.
    g.objs.aliens[0].worldz = g.objs.aliens[boss as usize].worldz + 500;
    tick_exp(&mut g, boss);
    assert_eq!(g.objs.aldead, 1, "wreck removed once passed");
}

// ------------------------------------------------------------
// 6. .dropmines (mode 9) spawns a barrier mine; the mine falls under gravity,
//    lands, then plays its deploy/settle animation.
//    (DSTRATS.ASM:5290-5300 .dropmines; :5720-5740 barrier_istrat.)
// ------------------------------------------------------------
#[test]
fn dropmines_spawns_falling_barrier() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    // Lift the mother so the mine spawns above the ground (worldy<0).
    g.objs.aliens[boss as usize].worldy = -200;
    g.objs.aliens[boss as usize].stratstate = 9; // .dropmines
    tick(&mut g, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].stratstate, 10,
        ".dropmines -> nxtmode"
    );

    let mine = find_shape(&g, SH_MT_BARRIER).expect("barrier spawned");
    // First tick runs barrier_init (hp/roty set) then one fall step.
    let s0 = g.objs.aliens[mine].stratptr.unwrap();
    g.call_strat(s0, mine as u16);
    assert_eq!(g.objs.aliens[mine].hp, 6, "barrierHP");
    assert_eq!(g.objs.aliens[mine].roty, 128, "roty = deg180");
    let y0 = g.objs.aliens[mine].worldy;
    let ms = g.objs.aliens[mine].stratptr.unwrap();

    // Drive the mine until it lands (worldy clamps to ground 0 and the deploy
    // anim starts advancing).
    for _ in 0..40 {
        let s = g.objs.aliens[mine].stratptr.unwrap();
        g.call_strat(s, mine as u16);
    }
    assert!(g.objs.aliens[mine].worldy > y0, "mine fell downward");
    assert_ne!(
        g.objs.aliens[mine].stratptr.unwrap(),
        ms,
        "mine advanced past the falling strat after landing"
    );
    assert!(anim(&g, mine) > 0, "deploy/settle animation running");
}

// ------------------------------------------------------------
// 7. escort bikes: a truck-spawned bike2 drops for 11 ticks, then becomes a
//    madbiker (full escort AI: madbikerHP, ENEMY2 collision).
//    (DSTRATS.ASM:4947-4964.)
// ------------------------------------------------------------
#[test]
fn bike2_becomes_madbiker_after_eleven_ticks() {
    let (mut g, boss) = setup(0, 0);
    bosses::madtrucker_init(&mut g, boss);
    g.objs.aliens[boss as usize].stratstate = 1; // .maketwobikes
    tick(&mut g, boss);
    let bike = find_shape(&g, SH_MT_AIR_1).expect("bike spawned") as u16;
    assert!(
        g.objs.aliens[bike as usize].hp != MADBIKER_HP,
        "still bike2 (not madbiker yet)"
    );

    // bike2 already ran its strat once inside the spawn tick? No — it is spawned
    // with stratptr=bike2_strat and ticks on subsequent frames. Drive 11 ticks.
    for _ in 0..11 {
        let s = g.objs.aliens[bike as usize].stratptr.unwrap();
        g.call_strat(s, bike);
    }
    assert_eq!(
        g.objs.aliens[bike as usize].hp, MADBIKER_HP,
        "handed off to madbiker"
    );
    assert_ne!(
        g.objs.aliens[bike as usize].collflags & COLLTYPE_ENEMY2,
        0,
        "ENEMY2 colltype"
    );
}

// ------------------------------------------------------------
// 8. madbiker init + escort motion + spinning-crash death.
//    (DSTRATS.ASM:4961-4978 init; :4979-5122 modes; :5136-5164 death.)
// ------------------------------------------------------------
#[test]
fn madbiker_inits_moves_and_dies() {
    const FLOAT_PHASE: u8 = 63;
    const PLAYER_BOUNDARY: i16 = 500;
    const CRUISE_HEIGHT: i16 = -200;

    let (mut g, boss) = setup(3000, 0);
    // Re-purpose the "boss" slot as a lone madbiker for this test.
    g.objs.aliens[0].worldy = CRUISE_HEIGHT;
    g.objs.aliens[boss as usize].worldy = CRUISE_HEIGHT;
    g.vars.shared.float_variables = [FLOAT_PHASE; 2];
    g.vars.strategy.player_max_x = PLAYER_BOUNDARY;
    bosses::madbiker_init(&mut g, boss);

    let b = &g.objs.aliens[boss as usize];
    assert_eq!(b.hp, MADBIKER_HP, "madbikerHP");
    assert_ne!(b.collflags & COLLTYPE_ENEMY2, 0, "ENEMY2");
    assert_ne!(b.sflags & ASF_SHADOW, 0);
    assert_eq!(b.snd2, MADBIKER_SOUND);
    assert_ne!(b.flags & AFONFIRE, 0);

    let phase = FLOAT_PHASE.wrapping_add(boss as u8);
    let hover_offset = (SINTAB[phase as usize] as i16) / 8;
    assert_eq!(
        b.worldx, 0,
        "source preserves horizontal position around float64_srou"
    );
    assert_eq!(
        b.worldy,
        b.sword2.wrapping_add(hover_offset),
        "float64_srou y hover follows the chased base height"
    );

    let engine = b.fireobjptr.checked_sub(1).expect("engine link") as usize;
    let flame = &g.objs.aliens[engine];
    assert!(flame.active);
    assert_eq!(flame.shape, sh::BOOST_SHAPE);
    assert_eq!(flame.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(flame.tx, NEGATIVE_TEN_AS_BYTE);
    assert_eq!(flame.relposx, 0);
    assert_eq!(flame.relposy, NEGATIVE_FIVE_AS_BYTE);
    assert_eq!(flame.relposz, NEGATIVE_TEN_AS_BYTE);
    assert_eq!(flame.sflags & ASF_INVISIBLE, 0);
    let (engine_x, engine_y, engine_z) =
        strat_roffs_pitch_yaw(b.rotx, b.roty, 0, 0, MADBIKER_ENGINE_Z_OFFSET);
    assert_eq!(flame.worldx, b.worldx.wrapping_add(engine_x));
    assert_eq!(flame.worldy, b.worldy.wrapping_add(engine_y));
    assert_eq!(flame.worldz, b.worldz.wrapping_add(engine_z));

    // .movealongside truck_accel chases the player-z; worldz should advance.
    let z0 = g.objs.aliens[boss as usize].worldz;
    tick(&mut g, boss);
    tick(&mut g, boss);
    assert!(
        g.objs.aliens[boss as usize].worldz > z0,
        "madbiker closes on player"
    );

    // Death: hp 0 routes to .explode which revives to 1hp under the crash strat.
    g.objs.aliens[boss as usize].hp = 0;
    let exp = g.objs.aliens[boss as usize].expstratptr.unwrap();
    g.call_strat(exp, boss);
    assert_eq!(
        g.objs.aliens[boss as usize].hp, 1,
        "revived under konostrat"
    );
    assert_eq!(
        g.objs.aliens[boss as usize].collflags & COLLTYPE_ENEMY2,
        0,
        "ENEMY2 cleared on crash"
    );
}

#[test]
fn madbiker_wall_contact_emits_the_authored_offset_spark() {
    const PLAYER_BOUNDARY: i16 = 100;
    const CLAMPED_BIKE_X: i16 = PLAYER_BOUNDARY - 16;
    const SPARK_X: i16 = CLAMPED_BIKE_X + 15;

    let (mut g, bike) = setup(3000, 0);
    g.vars.strategy.player_max_x = PLAYER_BOUNDARY;
    g.objs.aliens[bike as usize].worldx = PLAYER_BOUNDARY;
    bosses::madbiker_init(&mut g, bike);

    assert_eq!(g.objs.aliens[bike as usize].worldx, CLAMPED_BIKE_X);
    assert_eq!(g.objs.aliens[bike as usize].rotz, 8);
    let spark = find_shape(&g, sh::LINE_SPARK).expect("wall scrape spark");
    assert_eq!(g.objs.aliens[spark].worldx, SPARK_X);
}
