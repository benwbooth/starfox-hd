//! flingboss (+deadflingboss) behavioral tests — Route 2 L4 "armsmap" boss.
//!
//! ASM oracle: `flingboss_istrat` / `deadflingboss_istrat`
//! (reference/ultrastarfox/SF/STRAT/DSTRATS.ASM:2951-3545 / :3650-3687).
//!
//! No sf-oracle differential fixture is used: the ROM's flingboss spawns the
//! shared, unported `arm_istrat` grabber-tentacle chain, so a byte-exact whole-
//! object dump would diverge on the (deliberately) scoped-out recursive arm
//! growth (see the SCOPE NOTE in bosses.rs). These tests instead assert the
//! ported mother state machine + fire systems + two-form transition + death
//! routing against hand-derived ASM expectations, cited inline.

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;
const WM_BOSSFLAGS: u16 = 0x1F02;

// Local mirrors of the private bosses.rs constants (cited to the port).
const SH_FLINGARM_PROXY: u16 = 327;
const FB_SFLAG5_SFLAGS3: u8 = 0x01; // sflags3 mother damage latch
const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs
const ATMISSILE: u8 = 2; // alien.rs al_type
const BF_DYING: u8 = 16; // bossflags (bosses.rs)
const HARD_HP: u8 = 0xFF;
const FLINGBOSS_MAXHP: u16 = 104; // flingboss1HP(24) + flingboss2HP(80)

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

/// New game with a static player in slot 0 at `player_z` and the flingboss
/// spawned at `boss_z` (pre-init; its init adds +4000). Returns (game, boss).
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
    // Body shape SH_FLINGBOSS = 12 (sf-map rc.rs).
    let boss = spawn(&mut g, 0, -80, boss_z, 12);
    (g, boss)
}

fn count_arms(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_FLINGARM_PROXY)
        .count()
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

/// Set the boss's stratptr to the registered IS_FLINGBOSS init so
/// `run_strategies` drives the whole object graph.
fn arm_boss(g: &mut Game, boss: u16) {
    let id = g.world.istrats[bosses::IS_FLINGBOSS].expect("IS_FLINGBOSS registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

// ------------------------------------------------------------
// 1. init sets the boss HP bar + spawns the two arms.
//    (DSTRATS.ASM:2954 s_set_bossmaxHP #flingboss1HP+flingboss2HP;
//     :2961 sbyte4=flingboss1HP; :2962 roty=deg180; :2959 nohitaffect;
//     :3177 .generate -> two arm children.)
// ------------------------------------------------------------
#[test]
fn init_sets_bossmaxhp_and_spawns_arms() {
    let (mut g, boss) = setup(4000, 0);
    bosses::strat_flingboss_init(&mut g, boss);

    assert_eq!(g.vars.bossmaxhp, FLINGBOSS_MAXHP, "bossmaxHP = 24+80");
    let b = &g.objs.aliens[boss as usize];
    assert_eq!(b.sbyte4, 24, "phase-1 hit reserve = flingboss1HP");
    assert_eq!(b.roty, 128, "roty = deg180");
    assert_eq!(b.hp, HARD_HP, "phase-1 body is hard/invulnerable");
    assert_ne!(b.sflags & ASF_NOHITAFFECT, 0, "nohitaffect set in phase 1");
    assert_ne!(b.ptr, 0, "left arm linked via al_ptr");
    assert_ne!(b.sword1, 0, "right arm linked via al_sword1");
    assert_eq!(count_arms(&g), 2, "two arm children spawned");
}

#[test]
fn shared_arms_grow_full_grabber_chains() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    for _ in 0..180 {
        g.run_strategies();
        if count_shape(&g, 384) == 2 {
            break;
        }
    }

    assert_eq!(count_shape(&g, 384), 2, "each arm terminates in a grabber");
    assert!(count_shape(&g, 327) >= 8, "both recursive arm chains grew");
}

// ------------------------------------------------------------
// 2. the fling/fire cycle advances through its states + fires.
//    Approach tumbles in on rotx (DSTRATS.ASM:2977 +deg22) until rotx==0 and
//    within 2000, then .mainstrat (sbyte1=80) -> .spin (roty spins ±8) and
//    .triggermissile2 fires a BOSSHMISSILE1 every 64 frames.
// ------------------------------------------------------------
#[test]
fn fling_fire_cycle_advances_states() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    let mut saw_main = false; // sbyte1 timer running (main/spin/wave/...)
    let mut saw_missile = false; // BOSSHMISSILE1 fired
    let mut saw_spin = false; // roty left deg180 (spin state)

    for _ in 0..220 {
        g.run_strategies();
        let b = &g.objs.aliens[boss as usize];
        if b.sbyte1 > 0 {
            saw_main = true;
        }
        if b.roty != 128 {
            saw_spin = true;
        }
        if count_missiles(&g) > 0 {
            saw_missile = true;
        }
    }

    assert!(
        saw_main,
        "boss handed off from approach into the main/fling arc"
    );
    assert!(saw_missile, "boss fired a BOSSHMISSILE1 (triggermissile2)");
    assert!(saw_spin, "boss reached the spin state (roty left deg180)");
}

// ------------------------------------------------------------
// 3. damage drains m_bossHP (via the sflag5 latch -> sbyte4 -2 per hit).
//    (DSTRATS.ASM:3010-3015 .fin sflag5 -> two s_beqdec sbyte4;
//     :3027 s_add_bossHP x,al_sbyte4,#flingboss2HP.)
// ------------------------------------------------------------
#[test]
fn damage_drains_bosshp() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    // Advance into the main/fling arc (fin_body must be running).
    for _ in 0..40 {
        g.run_strategies();
        if g.objs.aliens[boss as usize].sbyte1 > 0 {
            break;
        }
    }
    let bar_full = g.vars.bosshp;
    assert_eq!(bar_full, FLINGBOSS_MAXHP, "full bar = sbyte4(24) + 80");
    let sb4_before = g.objs.aliens[boss as usize].sbyte4;

    // One arm hit: latch sflag5, tick once.
    g.objs.aliens[boss as usize].sflags3 |= FB_SFLAG5_SFLAGS3;
    g.run_strategies();

    let sb4_after = g.objs.aliens[boss as usize].sbyte4;
    assert_eq!(sb4_after, sb4_before - 2, "one hit drains sbyte4 by 2");
    assert!(g.vars.bosshp < bar_full, "m_bossHP bar dropped");
    assert_eq!(g.vars.bosshp, (sb4_after as u16) + 80, "bar = sbyte4 + 80");
}

// ------------------------------------------------------------
// 4. draining sbyte4 to 0 transforms into phase 2 (.crazy2 -> .almostgone2):
//    body becomes directly damageable (nohitaffect cleared), hp=flingboss2HP.
//    (DSTRATS.ASM:3479-3490.)
// ------------------------------------------------------------
#[test]
fn phase2_transition_makes_body_damageable() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    for _ in 0..40 {
        g.run_strategies();
        if g.objs.aliens[boss as usize].sbyte1 > 0 {
            break;
        }
    }

    // 13 arm hits drain sbyte4 24 -> 0 -> crazy2 (two -2 decrements per hit,
    // then the next hit's first s_beqdec sees 0 and branches).
    let mut reached_phase2 = false;
    for _ in 0..30 {
        g.objs.aliens[boss as usize].sflags3 |= FB_SFLAG5_SFLAGS3;
        g.run_strategies();
        if g.objs.aliens[boss as usize].hp == 80 {
            reached_phase2 = true;
            break;
        }
    }

    assert!(reached_phase2, "phase 2 reached (hp = flingboss2HP = 80)");
    let b = &g.objs.aliens[boss as usize];
    assert_eq!(b.hp, 80, "phase-2 hp = flingboss2HP");
    assert_eq!(b.sflags & ASF_NOHITAFFECT, 0, "phase-2 body damageable");
}

// ------------------------------------------------------------
// 5. death routes to the boss-explode chain (deadflingboss -> bossexplode).
//    Phase-2 hp==0 fires the mother's expstrat (deadflingboss_istrat), which
//    sinks then s_kill_obj -> bossexplode_istrat (boss_dying sets BF_DYING).
//    (DSTRATS.ASM:3481 expstrat=deadflingboss; :3662 expstrat=bossexplode;
//     :3686 s_kill_obj.)
// ------------------------------------------------------------
#[test]
fn death_routes_to_explode_chain() {
    let (mut g, boss) = setup(4000, 0);
    arm_boss(&mut g, boss);

    for _ in 0..40 {
        g.run_strategies();
        if g.objs.aliens[boss as usize].sbyte1 > 0 {
            break;
        }
    }
    for _ in 0..30 {
        g.objs.aliens[boss as usize].sflags3 |= FB_SFLAG5_SFLAGS3;
        g.run_strategies();
        if g.objs.aliens[boss as usize].hp == 80 {
            break;
        }
    }
    assert_eq!(g.objs.aliens[boss as usize].hp, 80, "in phase 2");

    // Kill: fire the mother's expstrat directly (what the engine does at hp==0).
    let exp = g.objs.aliens[boss as usize]
        .expstratptr
        .expect("phase-2 expstrat");
    g.call_strat(exp, boss); // -> deadflingboss_init (sink form)

    // Keep the player flush so the deadflingboss sink advances, then detonates.
    let mut detonated = false;
    for _ in 0..80 {
        g.objs.aliens[0].worldz = g.objs.aliens[boss as usize].worldz;
        g.run_strategies();
        if g.vars.read_ext8(WM_BOSSFLAGS) & BF_DYING != 0 {
            detonated = true;
            break;
        }
    }
    assert!(
        detonated,
        "deadflingboss reached bossexplode (BF_DYING latched)"
    );
}
