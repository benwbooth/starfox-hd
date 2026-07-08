//! webmonster behavioral tests — Route 3 L2 "web monster" spider boss.
//!
//! ASM oracle: `webmonster_istrat` (mother) + `propturret_istrat` (the 6
//! damageable turrets) + `drill_istrat` (the "web_fan" child) + `web_istrat`
//! (launched grab) + `launchatplayer_istrat` (fan detach)
//! (reference/ultrastarfox/SF/STRAT/DSTRATS.ASM:6504-6934).
//!
//! No sf-oracle differential fixture is used: the ROM boss's `.hit` reflect
//! (RebElasercol) and the web's player-drag/shake-free reader are player- and
//! collision-lane behaviors deliberately scoped in the WEBMONSTER_BEGIN note in
//! bosses.rs. These tests assert the ported mother state machine (intro descent
//! -> spin/position -> "all turrets dead" death gate), the turret-driven HP
//! bar, the drill fan's turret-fire sequencing, and the death spew ->
//! bossexplode routing against hand-derived ASM expectations, cited inline.

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;
const WM_BOSSFLAGS: u16 = 0x1F02;

// Local mirrors of the private bosses.rs constants (cited to the port).
const SH_BOSS_0_1: u16 = 85; // mother body (sf-map route3::common)
const SH_WM_BOSS_0_2: u16 = 293; // turret proxy
const SH_WM_BOSS_0_0: u16 = 294; // fan proxy (rest shape)
const ATMISSILE: u8 = 2; // alien.rs al_type
const BF_DYING: u8 = 16; // bossflags (bosses.rs)
const HARD_HP: u8 = 0xFF;
const WM_MAXHP: u16 = 120; // 6 * propturretHP(20)

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

/// New game with a static player in slot 0 at `player_z` and the webmonster
/// spawned at `boss_z` (pre-init). Returns (game, boss).
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
    let boss = spawn(&mut g, 0, 0, boss_z, SH_BOSS_0_1);
    (g, boss)
}

fn count_shape(g: &Game, shape: u16) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
        .count()
}

fn count_missiles(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].type_ & ATMISSILE != 0)
        .count()
}

/// Point the boss's stratptr at the registered IS_WEBMONSTER init so
/// `run_strategies` drives the whole object graph.
fn arm_boss(g: &mut Game, boss: u16) {
    let id = g.world.istrats[bosses::IS_WEBMONSTER].expect("IS_WEBMONSTER registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

// ------------------------------------------------------------
// 1. init spawns the 6-turret ring + fan, arms the invulnerable body, and the
//    turrets accumulate the boss HP bar to 120.
//    (DSTRATS.ASM:6506-6514 init; :6572-6583 .generate 6 turrets + fan;
//     :6890 propturret s_add_bossmaxHP; :6933 s_add_bossHP.)
// ------------------------------------------------------------
#[test]
fn init_spawns_ring_and_hp_bar() {
    let (mut g, boss) = setup(4000, 0);
    bosses::strat_webmonster_init(&mut g, boss);

    // Init falls into `.strat` the same tick (ROM s_start_strat re-entry), so
    // one descent step has run: rotx 96->95, worldy 1000->990.
    {
        let b = &g.objs.aliens[boss as usize];
        assert_eq!(b.rotx, 95, "rotx = deg90+deg45-1 after the fall-through tick");
        assert_eq!(b.worldy, 990, "worldy = 1000-10 after the fall-through tick");
        assert_eq!(b.depthoffset, 1, "depthoffset = 1");
        assert_eq!(b.hp, HARD_HP, "body is hard/invulnerable");
        assert_ne!(b.sword1, 0, "children linked via al_sword1 chain");
    }
    assert_eq!(count_shape(&g, SH_WM_BOSS_0_2), 6, "6 propturrets spawned");
    assert_eq!(count_shape(&g, SH_WM_BOSS_0_0), 1, "1 fan (web_fan) spawned");

    // The turrets each add propturretHP to the bar's max on their first tick.
    for _ in 0..4 {
        g.run_strategies();
    }
    assert_eq!(g.vars.bossmaxhp, WM_MAXHP, "bossmaxHP = 6 * propturretHP(20)");
    assert_eq!(
        g.vars.bosshp, WM_MAXHP,
        "all 6 turrets alive -> m_bossHP re-summed to 120"
    );
}

// ------------------------------------------------------------
// 2. the intro descent completes: rotx 96->0 (-1/tick) and worldy 1000->0
//    (-10/tick), then the mother reaches .mainstrat and slow-spins (rotz-=1).
//    (DSTRATS.ASM:6515-6537.)
// ------------------------------------------------------------
#[test]
fn intro_descent_completes_then_spins() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    // rotx hits 0 after 96 ticks, worldy after 100 — run past both.
    for _ in 0..110 {
        g.run_strategies();
    }
    let b = &g.objs.aliens[boss as usize];
    assert_eq!(b.rotx, 0, "descent drove rotx to 0");
    assert_eq!(b.worldy, 0, "descent drove worldy to 0");
    // .move spins rotz by -1 every tick it runs -> no longer 0.
    assert_ne!(b.rotz, 0, "mother slow-spins in .move (rotz-=1)");
    // Boss still alive (turrets present) -> not in .bossdead.
    assert_eq!(g.vars.read_ext8(WM_BOSSFLAGS) & BF_DYING, 0, "boss not dying yet");
}

// ------------------------------------------------------------
// 3. the drill fan sequences the turrets so they eventually fire: the drill
//    sweep (sbyte2) sets turret sflag1 (spinning) then the fade window
//    (sbyte3=50) releases banks so a turret fires an HMISSILE1.
//    (DSTRATS.ASM:6646-6681 sweep; :6605-6645 release; :6892-6934 turret fire.)
// ------------------------------------------------------------
#[test]
fn drill_sequences_turret_fire() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);

    let mut saw_missile = false;
    for _ in 0..500 {
        g.run_strategies();
        if count_missiles(&g) > 0 {
            saw_missile = true;
            break;
        }
    }
    assert!(
        saw_missile,
        "drill's fade window released a turret bank -> HMISSILE1 fired"
    );
}

// ------------------------------------------------------------
// 4. killing all 6 turrets routes death: the mother reaches .bossdead, spews
//    medium explosions while spinning up, and at sbyte2==30 hands off to
//    bossexplode (boss_dying -> BF_DYING).
//    (DSTRATS.ASM:6531 childrendead gate; :6539-6566 .bossdead; :6551
//     s_beq bossexplode_istrat.)
// ------------------------------------------------------------
#[test]
fn death_routes_to_bossexplode() {
    let (mut g, boss) = setup(0, 0);
    bosses::strat_webmonster_init(&mut g, boss);
    // Let the turrets link + tick once.
    for _ in 0..3 {
        g.run_strategies();
    }
    // Force the intro to completion so the next mother tick is .mainstrat.
    {
        let b = &mut g.objs.aliens[boss as usize];
        b.rotx = 0;
        b.worldy = 0;
    }
    // Kill every turret (hp==0 -> engine's dead path runs its expstrat + frees).
    for i in 0..NUMBER_AL {
        if g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_WM_BOSS_0_2 {
            g.objs.aliens[i].hp = 0;
        }
    }
    // One frame to explode/free the turrets.
    g.run_strategies();
    assert_eq!(count_shape(&g, SH_WM_BOSS_0_2), 0, "all turrets dead");

    // .bossdead increments sbyte2 each tick; at 30 it hands off to bossexplode
    // (boss_dying sets BF_DYING). Run well past 30 mother ticks.
    let mut died = false;
    for _ in 0..40 {
        g.run_strategies();
        if g.vars.read_ext8(WM_BOSSFLAGS) & BF_DYING != 0 {
            died = true;
            break;
        }
    }
    assert!(died, "all-turrets-dead -> .bossdead -> bossexplode (BF_DYING)");
}
