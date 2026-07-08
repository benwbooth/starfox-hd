//! chicken (Route 3 L3 boss) + the shared `arm_istrat` grabber-tentacle code
//! behavioral tests.
//!
//! ASM oracle: `chicken_istrat` / `chick` (DSTRATS.ASM:3696-4523) and the shared
//! `arm_istrat` / `ars` (DSTRATS.ASM:2444-2944).
//!
//! No sf-oracle differential fixture is used: the ROM boss depends on the
//! unported firebreath/egg/shell/wing flight strats and a projected-screen
//! turn selector (`s_leftview_strat`) — all deliberately simplified (see the
//! CHICKEN_BEGIN scope note in bosses.rs). These tests assert the ported mother
//! mode machine, neck GROWTH, the inter-segment SPRING easing, the grabber
//! hit->mother-sflag5 routing, the HP-bar accumulator and the death->explode
//! chain against hand-derived ASM expectations, cited inline.

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;
const WM_ARMMODE: u16 = 0x17F0; // ARMMODE ($17f0)

// Local mirrors of the private bosses.rs constants (cited to the port).
const CH_BOSS_D_1: u16 = 78; // body (map SH_BOSS_D_1)
const SH_CHICK_BOSS_D_0: u16 = 282; // head
const SH_CHICK_BOSS_D_2: u16 = 283; // tail
const SH_CHICK_NECK: u16 = 284;
const SH_CHICK_GRABBER: u16 = 287;
const SH_CHICK_BOSS_D_8: u16 = 291; // wing1
const SH_CHICK_BOSS_D_9: u16 = 292; // wing2
const SH_FLINGBOSS_BODY: u16 = 12; // .passiton mother target

const CH_SFLAG5_SFLAGS3: u8 = 0x01; // mother/parent damage latch (sflags3)
const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs
const ATLASER: u8 = 4; // alien.rs al_type
const CHICKEN_MAXHP: u16 = 64; // chickenbodyHP (DSTRATS.ASM:68)

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

/// New game with a static player in slot 0 and the chicken body (SH_BOSS_D_1)
/// spawned at `boss_z`. Returns (game, boss).
fn setup(player_z: i16, boss_z: i16) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bosses::register(&mut g.world);

    let p = spawn(&mut g, 0, 0, player_z, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    }
    let boss = spawn(&mut g, 0, -100, boss_z, CH_BOSS_D_1);
    (g, boss)
}

fn arm_boss(g: &mut Game, boss: u16) {
    let id = g.world.istrats[bosses::IS_CHICKEN].expect("IS_CHICKEN registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

fn count_shape(g: &Game, shape: u16) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
        .count()
}

fn obj_encode(idx: u16) -> u16 {
    idx + 1
}

// ------------------------------------------------------------
// 1. init sets the HP bar (chickenbodyHP=64), armmode=128, and spawns the three
//    neck-chain roots (al_ptr / al_sword1 / al_sword2) + two wings.
//    (DSTRATS.ASM:3698-3717 init; :4268-4324 .generate.)
// ------------------------------------------------------------
#[test]
fn init_sets_bossmaxhp_armmode_and_spawns_necks_wings() {
    let (mut g, boss) = setup(0, 0);
    bosses::strat_chicken_init(&mut g, boss);

    assert_eq!(g.vars.bossmaxhp, CHICKEN_MAXHP, "bossmaxHP = chickenbodyHP");
    assert_eq!(g.vars.read_ext8(WM_ARMMODE), 128, "armmode initialised to 128");

    let b = &g.objs.aliens[boss as usize];
    assert_ne!(b.ptr, 0, "left neck linked via al_ptr");
    assert_ne!(b.sword1, 0, "right neck linked via al_sword1");
    assert_ne!(b.sword2, 0, "tail linked via al_sword2");
    assert_ne!(b.swpx1, 0, "wing1 linked via al_sWPx1");
    assert_ne!(b.swpy1, 0, "wing2 linked via al_sWPy1");
    assert_eq!(b.hp as u16, CHICKEN_MAXHP, "body hp = chickenbodyHP");

    assert_eq!(count_shape(&g, SH_CHICK_BOSS_D_8), 1, "wing1 spawned");
    assert_eq!(count_shape(&g, SH_CHICK_BOSS_D_9), 1, "wing2 spawned");
    assert_eq!(count_shape(&g, SH_CHICK_NECK), 2, "two neck roots (ptr, sword1)");
}

// ------------------------------------------------------------
// 2. the neck chains GROW: the sword1-counted roots generate their heads/tail
//    (.nbl -> .generate), and the mother becomes vulnerable (.check_fin clears
//    nohitaffect) once a head/tail has grown. (DSTRATS.ASM:2617-2637 growth;
//    :4031-4079 check_fin.)
// ------------------------------------------------------------
#[test]
fn neck_chains_grow_heads_and_tail() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);

    for _ in 0..30 {
        g.run_strategies();
    }
    let heads = count_shape(&g, SH_CHICK_BOSS_D_0);
    let tails = count_shape(&g, SH_CHICK_BOSS_D_2);
    assert!(
        heads + tails > 0,
        "at least one neck grew a head/tail terminus (heads={heads} tails={tails})"
    );
    // armmode gained the head/tail grow bits (+2 per terminus, from 128).
    assert_ne!(
        g.vars.read_ext8(WM_ARMMODE) & 6,
        0,
        "armmode tracked the generated head/tail(s)"
    );
}

// ------------------------------------------------------------
// 2b. .check_fin exposes the body (clears nohitaffect) once a head/tail is
//     linked DIRECTLY to a mother slot — the "shoot the necks back, then the
//     body" mechanic. (DSTRATS.ASM:4031-4047.)
// ------------------------------------------------------------
#[test]
fn check_fin_exposes_body_when_terminus_direct() {
    let (mut g, boss) = setup(0, 0);
    bosses::strat_chicken_init(&mut g, boss); // stratptr now = chicken_strat
    assert_ne!(
        g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT,
        0,
        "body starts invulnerable (long necks)"
    );

    // Link a tail (boss_d_2) directly onto the al_sword2 slot (fully retracted).
    let tail = spawn(&mut g, 0, 0, 0, SH_CHICK_BOSS_D_2);
    g.objs.aliens[boss as usize].sword2 = obj_encode(tail) as i16;

    g.run_strategies(); // runs the mode machine -> .check_fin
    assert_eq!(
        g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT,
        0,
        "body became vulnerable with a tail directly exposed"
    );
}

// ------------------------------------------------------------
// 3. the inter-segment SPRING easing runs: chicken_arm_position eases a child's
//    rotation toward its parent and accumulates a momentum term on al_sbyte1.
//    (DSTRATS.ASM:2860-2934 .position.)
// ------------------------------------------------------------
#[test]
fn arm_position_spring_eases_child_toward_parent() {
    let (mut g, _boss) = setup(0, 0);
    let parent = spawn(&mut g, 0, 0, 0, SH_CHICK_NECK);
    let child = spawn(&mut g, 0, 0, 0, SH_CHICK_NECK);
    g.objs.aliens[parent as usize].rotx = 100;
    g.objs.aliens[child as usize].rotx = 0;
    g.objs.aliens[child as usize].sbyte1 = 0;

    bosses::chicken_arm_position(&mut g, parent, child);

    assert_ne!(
        g.objs.aliens[child as usize].rotx, 0,
        "child rotx eased toward the parent (100)"
    );
    assert_ne!(
        g.objs.aliens[child as usize].sbyte1, 0,
        "spring momentum accumulated on al_sbyte1"
    );
}

// ------------------------------------------------------------
// 4. a grabber hit routes damage to the mother's sflag5 (.grabberhit ->
//    .passiton -> find #flingboss -> set sflag5). The shared arm code targets
//    the flingboss body shape; exercised here with a flingboss-shaped stand-in
//    (see the CHICKEN_BEGIN section note). (DSTRATS.ASM:2752-2791.)
// ------------------------------------------------------------
#[test]
fn grabber_hit_routes_to_mother_sflag5() {
    let (mut g, _boss) = setup(0, 0);
    let mother = spawn(&mut g, 0, 0, 0, SH_FLINGBOSS_BODY);
    let grabber = spawn(&mut g, 0, 0, 0, SH_CHICK_GRABBER);
    bosses::chicken_arm_init(&mut g, grabber); // sets collstrat=.grabberhit

    // Face the player so the grabberhit facing window passes
    // ((roty+173)&0xff < 90 and (rotx+45)&0xff < 90).
    g.objs.aliens[grabber as usize].roty = 90;
    g.objs.aliens[grabber as usize].rotx = 0;
    g.objs.aliens[grabber as usize].sflags2 &= !0x10; // sflag1 clear -> mother route

    // A laser collided with the grabber.
    let laser = spawn(&mut g, 0, 0, 0, 0);
    g.objs.aliens[laser as usize].type_ |= ATLASER;
    g.objs.aliens[grabber as usize].collobjptr = obj_encode(laser);

    assert_eq!(
        g.objs.aliens[mother as usize].sflags3 & CH_SFLAG5_SFLAGS3,
        0,
        "mother sflag5 clear before the hit"
    );
    bosses::chicken_arm_grabberhit(&mut g, grabber);
    assert_ne!(
        g.objs.aliens[mother as usize].sflags3 & CH_SFLAG5_SFLAGS3,
        0,
        "grabber hit latched the mother's sflag5"
    );

    // A non-laser collision does NOT route (s_jmpNOT_colltype y,laser).
    g.objs.aliens[mother as usize].sflags3 &= !CH_SFLAG5_SFLAGS3;
    g.objs.aliens[laser as usize].type_ &= !ATLASER;
    bosses::chicken_arm_grabberhit(&mut g, grabber);
    assert_eq!(
        g.objs.aliens[mother as usize].sflags3 & CH_SFLAG5_SFLAGS3,
        0,
        "a non-laser hit does not route to the mother"
    );
}

// ------------------------------------------------------------
// 5. the boss bar = body.hp accumulated per tick (s_add_bossHP x,al_hp,
//    DSTRATS.ASM:4027); damaging the body drops the bar.
// ------------------------------------------------------------
#[test]
fn bosshp_tracks_body_hp() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);
    for _ in 0..4 {
        g.run_strategies();
    }
    assert_eq!(g.vars.bosshp, CHICKEN_MAXHP, "full bar = chickenbodyHP");

    g.objs.aliens[boss as usize].hp = 40;
    g.run_strategies();
    assert_eq!(g.vars.bosshp, 40, "bar tracked the damaged body hp");
}

// ------------------------------------------------------------
// 6. death (.chickenexplode) kills every neck chain + wing and hands off to
//    bossexplode. (DSTRATS.ASM:4505-4519.)
// ------------------------------------------------------------
#[test]
fn death_kills_chains_and_explodes() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);
    for _ in 0..20 {
        g.run_strategies();
    }
    let before = (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count();
    assert!(before > 3, "chicken graph populated before death");

    bosses::chicken_chickenexplode(&mut g, boss);

    // All neck/wing children are freed (only the body + player + explosion
    // objects remain — far fewer than before).
    assert_eq!(count_shape(&g, SH_CHICK_NECK), 0, "necks killed");
    assert_eq!(count_shape(&g, SH_CHICK_BOSS_D_8), 0, "wing1 killed");
    assert_eq!(count_shape(&g, SH_CHICK_BOSS_D_9), 0, "wing2 killed");
    assert!(
        g.objs.aliens[boss as usize].expstratptr.is_some(),
        "body handed off to bossexplode"
    );
}
