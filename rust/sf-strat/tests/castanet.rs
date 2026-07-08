//! castanet ("Metal Smasher") behavioral tests — Route 2 L5 boss.
//!
//! ASM oracle: `castanet_istrat` / `castbit_istrat` / `fireringlaser_l` +
//! `ringlaser_istrat`, and the shared `trucklaunch`/`fallingtruck`
//! ground-vehicle base (reference/ultrastarfox/SF/STRAT/DSTRATS.ASM:5754-6336).
//!
//! No sf-oracle differential fixture is used: the ROM boss depends on the
//! unported `path_istrat` `minicastanet` swarm and reads cross-object global
//! scratch RAM (locusmode/powerbuild) that shapes the ringlaser trajectory —
//! both deliberately scoped out (see the CASTANET_BEGIN scope note in
//! bosses.rs). These tests assert the ported mother mode machine, cymbal-bit
//! positioning, HP-bar accumulation and death->trucklaunch->explode routing
//! against hand-derived ASM expectations, cited inline.

use sf_game::alien::NUMBER_AL;
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bosses;

const WM_RNDVAL: u16 = 0x1F00;

// Local mirrors of the private bosses.rs constants (cited to the port).
const SH_CAST_BOSS_E_0_PROXY: u16 = 276; // bit1 (DSTRATS.ASM:6137)
const SH_CAST_BOSS_E_1_PROXY: u16 = 277; // bit2 rotz==128 (DSTRATS.ASM:6237)
const SH_CAST_BOSS_E_1A_PROXY: u16 = 278; // bit2 otherwise (DSTRATS.ASM:6239)
const SH_CAST_RINGLASER_PROXY: u16 = 281; // ringlaser (DSTRATS.ASM:6363)
const CAST_SFLAG1: u8 = 0x10; // sflag1 on sflags2
const ASF_COLLDISABLE: u8 = 0x10; // alien.rs
const ASF_NOHITAFFECT: u8 = 0x40; // alien.rs
const CASTANET_MAXHP: u16 = 240; // castanetHP(120) * 2 (DSTRATS.ASM:5762)
const FIRE_MODE: u8 = 18; // .fire mode-table index (DSTRATS.ASM:5803)

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

/// New game with a static player in slot 0 and the castanet mother (nullshape)
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
    // The map spawns the mother as SH_NULLSHAPE (0).
    let boss = spawn(&mut g, 0, 0, boss_z, 0);
    (g, boss)
}

/// Set the boss's stratptr to the registered IS_CASTANET init so
/// `run_strategies` drives the whole object graph.
fn arm_boss(g: &mut Game, boss: u16) {
    let id = g.world.istrats[bosses::IS_CASTANET].expect("IS_CASTANET registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

fn is_bit(shape: u16) -> bool {
    matches!(
        shape,
        SH_CAST_BOSS_E_0_PROXY | SH_CAST_BOSS_E_1_PROXY | SH_CAST_BOSS_E_1A_PROXY
    )
}

fn count_bits(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && is_bit(g.objs.aliens[i].shape))
        .count()
}

fn count_ringlasers(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_CAST_RINGLASER_PROXY)
        .count()
}

/// Decode a mother pointer slot (index+1 encoding) into the live bit idx.
fn bit_idx(g: &Game, raw: u16) -> Option<usize> {
    if raw == 0 {
        return None;
    }
    let i = (raw - 1) as usize;
    if i < NUMBER_AL && g.objs.aliens[i].active {
        Some(i)
    } else {
        None
    }
}

// ------------------------------------------------------------
// 1. init sets the HP bar + spawns the two cymbal bits, mother is an
//    invisible (colldisable) nohitaffect controller.
//    (DSTRATS.ASM:5757-5768: colldisable, hardHP, bossmaxHP=castanetHP*2,
//     .generate -> two castbit children linked via al_ptr / al_sword1.)
// ------------------------------------------------------------
#[test]
fn init_sets_bossmaxhp_and_spawns_two_bits() {
    let (mut g, boss) = setup(0, 0);
    bosses::strat_castanet_init(&mut g, boss);

    assert_eq!(g.vars.bossmaxhp, CASTANET_MAXHP, "bossmaxHP = castanetHP*2");
    let b = &g.objs.aliens[boss as usize];
    assert_ne!(b.sflags & ASF_COLLDISABLE, 0, "mother is colldisable");
    assert_ne!(b.sflags & ASF_NOHITAFFECT, 0, "mother is nohitaffect");
    assert_ne!(b.ptr, 0, "bit1 linked via al_ptr");
    assert_ne!(b.sword1, 0, "bit2 linked via al_sword1");
    assert_eq!(count_bits(&g), 2, "two cymbal bits spawned");
}

// ------------------------------------------------------------
// 2a. the mode machine advances: the boss rolls on (mode 1) and progresses
//     into the repeating attack cycle (stratstate moves past rollon).
//     (DSTRATS.ASM:5769-5814 mode table; .rollon advances at dz<2000.)
// ------------------------------------------------------------
#[test]
fn mode_machine_rolls_on_and_advances() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);

    let mut max_mode = 0u8;
    for _ in 0..400 {
        g.run_strategies();
        max_mode = max_mode.max(g.objs.aliens[boss as usize].stratstate);
    }
    // Past rollon (1) means it reached the moveaway/generateminis/moveapart
    // attack cycle — i.e. the mode machine is running, not stuck rolling in.
    assert!(
        max_mode > 2,
        "advanced past rollon into the attack cycle (reached mode {max_mode})"
    );
    assert_eq!(count_bits(&g), 2, "both bits still linked/positioned");
}

// ------------------------------------------------------------
// 2b. the fire mode makes both bits fire ringlasers.
//     (DSTRATS.ASM:5867-5889 .fire sets both bits' sflag1;
//      castbit .strat DSTRATS.ASM:6241-6243 fires fireringlaser on gf&1==0.)
// ------------------------------------------------------------
#[test]
fn fire_mode_spawns_ringlasers() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);
    g.run_strategies(); // init + generate bits
    assert_eq!(count_bits(&g), 2);

    // Force the mother into the .fire mode (skip the long organic approach).
    g.objs.aliens[boss as usize].stratstate = FIRE_MODE;
    g.objs.aliens[boss as usize].sbyte1 = 0;

    let mut saw_sflag1 = false;
    let mut fired = false;
    for _ in 0..8 {
        g.run_strategies();
        let m = g.objs.aliens[boss as usize];
        if let Some(b1) = bit_idx(&g, m.ptr) {
            if g.objs.aliens[b1].sflags2 & CAST_SFLAG1 != 0 {
                saw_sflag1 = true;
            }
        }
        if count_ringlasers(&g) > 0 {
            fired = true;
        }
    }
    assert!(saw_sflag1, "fire mode set a bit's sflag1 (firing flag)");
    assert!(fired, "a bit fired a ringlaser during the fire window");
}

// ------------------------------------------------------------
// 3. the boss bar = bit1.hp + bit2.hp accumulated per tick; damaging a bit
//    drops it. (DSTRATS.ASM:6119-6127 s_add_bossHP y,al_hp for each bit.)
// ------------------------------------------------------------
#[test]
fn bosshp_drains_on_bit_damage() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);

    // Tick until both bits have run their init (hp = castanetHP = 120) and the
    // mother is summing them into the bar.
    for _ in 0..4 {
        g.run_strategies();
    }
    let full = g.vars.bosshp;
    assert_eq!(full, CASTANET_MAXHP, "full bar = 120 + 120");

    // Damage bit1 by 20 (120 -> 100), tick once more, the bar drops by 20.
    let b1 = bit_idx(&g, g.objs.aliens[boss as usize].ptr).expect("bit1 alive");
    g.objs.aliens[b1].hp = 100;
    g.run_strategies();
    assert_eq!(g.vars.bosshp, CASTANET_MAXHP - 20, "bar tracked the -20 hit");
    assert!(g.vars.bosshp < full, "m_bossHP bar dropped");
}

// ------------------------------------------------------------
// 4. killing bit2 (al_sword1) routes death through the ground-vehicle base:
//    the mother is removed, bit1 becomes a trucklaunch, and the trucklaunch
//    fuse eventually kills itself into bossexplode (bossmaxHP -> 0).
//    (DSTRATS.ASM:6085-6096 death branch; :6308-6309 s_set_bossmaxHP #0 +
//     s_kill_obj.)
// ------------------------------------------------------------
#[test]
fn death_routes_to_trucklaunch_explode() {
    let (mut g, boss) = setup(0, 0);
    arm_boss(&mut g, boss);
    for _ in 0..4 {
        g.run_strategies();
    }
    assert_eq!(count_bits(&g), 2);

    // Kill bit2 (the al_sword1-linked half): the death test keys on it.
    let b2 = bit_idx(&g, g.objs.aliens[boss as usize].sword1 as u16).expect("bit2 alive");
    g.objs.aliens[b2].hp = 0;
    g.run_strategies();

    assert!(
        !g.objs.aliens[boss as usize].active,
        "mother removed (s_remove_obj) on bit2 death"
    );

    // Run out the trucklaunch fuse (70 + 40 + spawn + 1 ticks) — it ends by
    // zeroing bossmaxHP and killing itself into bossexplode.
    let mut reached_kill = false;
    for _ in 0..160 {
        g.run_strategies();
        if g.vars.bossmaxhp == 0 {
            reached_kill = true;
            break;
        }
    }
    assert!(
        reached_kill,
        "trucklaunch reached its s_kill_obj (bossmaxHP zeroed -> bossexplode)"
    );
}
