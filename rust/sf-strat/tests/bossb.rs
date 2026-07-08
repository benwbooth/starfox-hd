//! Andross final-boss (bossB face + bossBrob robot) behavioral tests.
//!
//! ASM oracle: reference/ultrastarfox/SF/STRAT/GB3STRAT.ASM:1252-3140
//! (bossB_Istrat / bossBrob_Istrat + sub-labels). No sf-oracle differential
//! fixture (Andross has no C-oracle port and the ROM fight depends on the
//! scoped-out entity-image trail + multi-form transform). These tests drive
//! the registered istrats black-box through `run_strategies` and assert the
//! ported state machine, HP-bar model, attack cycle, split, and death routing
//! against hand-derived ASM expectations, cited inline.

use sf_game::alien::{ACF_WEAPON, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::bossb;

const WM_RNDVAL: u16 = 0x1F00;
const GF_BOSSDEAD: u8 = sf_game::vars::GF_BOSSDEAD;

const BOSSB_AIR_HP: u8 = 40; // STRATEQU.INC:284
const BOSSBROB_HP: u8 = 32; // STRATEQU.INC:286

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

/// New game with a static player in slot 0 and the boss (nullshape) at boss_z.
fn setup(player_z: i16, boss_z: i16) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bossb::register(&mut g.world);

    let p = spawn(&mut g, 0, 0, player_z, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    }
    let boss = spawn(&mut g, 0, 0, boss_z, 0);
    (g, boss)
}

fn arm(g: &mut Game, boss: u16, is_index: usize) {
    let id = g.world.istrats[is_index].expect("istrat registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

/// Count active enemy projectiles (spawn_projectile sets ACF_WEAPON).
fn weapon_count(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].collflags & ACF_WEAPON != 0)
        .count()
}

fn active_count(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

// ---------------------------------------------------------------------------
// Registration + ISTRAT wiring.
// ---------------------------------------------------------------------------

#[test]
fn registers_at_istrat_indices_and_map_address() {
    let (g, _b) = setup(0, 5000);
    // ISTRATS.ASM:539/542 (MACRO-counted): bossB=115, bossBrob=118.
    assert_eq!(bossb::IS_BOSSB, 115);
    assert_eq!(bossb::IS_BOSSBROB, 118);
    assert!(g.world.istrats[bossb::IS_BOSSB].is_some(), "bossB row");
    assert!(g.world.istrats[bossb::IS_BOSSBROB].is_some(), "bossBrob row");
    // MAP1_5.ASM places the face via synthetic STRAT_ADDR_BOSSB (0x06000F).
    assert!(
        g.world.find_strategy_address(bossb::STRAT_ADDR_BOSSB).is_some(),
        "MAP1_5 boss address resolves"
    );
}

// ---------------------------------------------------------------------------
// bossB (face) — HP bar + fire + death=escape.
// ---------------------------------------------------------------------------

#[test]
fn face_init_sets_bar_and_accumulates() {
    // Far away (|dz|=5000 > 2500): init fires, then the entry strat just flies.
    let (mut g, boss) = setup(0, 5000);
    arm(&mut g, boss, bossb::IS_BOSSB);
    g.run_strategies();
    // s_set_aldata #bossBairHP + s_set_bossmaxHP #bossBairHP (GB3STRAT.ASM:1254).
    assert_eq!(g.objs.aliens[boss as usize].hp, BOSSB_AIR_HP, "hp = bossBairHP");
    assert_eq!(g.vars.bossmaxhp, BOSSB_AIR_HP as u16, "bossmaxHP = bossBairHP");
    // s_add_bossHP x,al_hp ran in bossB_cont2 -> the accumulator holds hp.
    assert_eq!(g.vars.bosshp, BOSSB_AIR_HP as u16, "m_bossHP accumulated");
}

#[test]
fn face_reaches_dodge_and_fires() {
    // Close in (|dz|=1000 < 2500): the entry strat decelerates then
    // bossBdodge_init; the dodge attack fires at the player.
    let (mut g, boss) = setup(0, 1000);
    arm(&mut g, boss, bossb::IS_BOSSB);
    let mut fired = false;
    for _ in 0..120 {
        g.run_strategies();
        if weapon_count(&g) > 0 {
            fired = true;
            break;
        }
    }
    assert!(fired, "the face fires during the dodge attack");
    assert!(g.vars.bosshp > 0, "the bar keeps draining/accumulating");
}

#[test]
fn face_drains_on_damage() {
    let (mut g, boss) = setup(0, 5000);
    arm(&mut g, boss, bossb::IS_BOSSB);
    g.run_strategies();
    let full = g.vars.bosshp;
    // Chip the face's HP; the accumulator (re-summed each frame) tracks it.
    g.objs.aliens[boss as usize].hp = 20;
    g.run_strategies();
    assert!(g.vars.bosshp < full, "m_bossHP bar dropped after damage");
    assert_eq!(g.vars.bosshp, 20, "bar == current hp");
}

#[test]
fn face_death_sets_bossdead_via_escape() {
    // The face's expstrat is bossBescape: on HP0 it flees and sets GF_BOSSDEAD
    // (GB3STRAT.ASM:1734 s_or_var gameflags,#gf_bossdead).
    let (mut g, boss) = setup(0, 5000);
    arm(&mut g, boss, bossb::IS_BOSSB);
    g.run_strategies();
    assert_eq!(g.vars.gameflags & GF_BOSSDEAD, 0, "alive before death");
    g.objs.aliens[boss as usize].hp = 0; // do_strat routes hp0 -> expstrat
    g.run_strategies();
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0, "escape set GF_BOSSDEAD");
}

// ---------------------------------------------------------------------------
// bossBrob (robot) — HP bar + split + attack cycle + death=explode.
// ---------------------------------------------------------------------------

#[test]
fn robot_init_sets_bar() {
    let (mut g, boss) = setup(0, 5000);
    arm(&mut g, boss, bossb::IS_BOSSBROB);
    g.run_strategies();
    // s_set_aldata #bossBrobHP + s_set_bossmaxHP #bossBrobHP (GB3STRAT.ASM:1880).
    assert_eq!(g.objs.aliens[boss as usize].hp, BOSSBROB_HP, "hp = bossBrobHP");
    assert_eq!(g.vars.bossmaxhp, BOSSBROB_HP as u16, "bossmaxHP = bossBrobHP");
    assert_eq!(g.vars.bosshp, BOSSBROB_HP as u16, "m_bossHP accumulated");
}

#[test]
fn robot_approaches_and_splits() {
    // Close (|dz|=1000, worldx=0): approach -> rob2 (brake) -> split -> spawns
    // the shootable face/hand parts (bossBentsplit, child slots).
    let (mut g, boss) = setup(0, 1000);
    arm(&mut g, boss, bossb::IS_BOSSBROB);
    let base = active_count(&g); // player + boss
    let mut split = false;
    for _ in 0..200 {
        g.run_strategies();
        if active_count(&g) > base + 1 {
            split = true; // at least the two non-body parts spawned
            break;
        }
    }
    assert!(split, "the robot split into extra parts");
}

#[test]
fn robot_attack_cycle_advances_and_fires() {
    // Drive the floating form to death -> the expstrat transforms to the
    // walking form and enters the bossBrobnextstate attack rotation, which
    // fires each state.
    let (mut g, boss) = setup(0, 1000);
    arm(&mut g, boss, bossb::IS_BOSSBROB);
    g.run_strategies(); // init (floating form)
    // Simulate the floating form's death -> Phase-1 transform (attack cycle).
    g.objs.aliens[boss as usize].hp = 0;
    g.run_strategies();
    assert!(
        g.objs.aliens[boss as usize].sflags3 & 0x01 != 0,
        "walking-form latch set (transformed)"
    );
    let start_state = g.objs.aliens[boss as usize].stratstate;
    let mut fired = false;
    let mut advanced = false;
    for _ in 0..300 {
        g.run_strategies();
        if weapon_count(&g) > 0 {
            fired = true;
        }
        if g.objs.aliens[boss as usize].stratstate != start_state {
            advanced = true;
        }
        if fired && advanced {
            break;
        }
    }
    assert!(advanced, "the attack cycle advanced states (bossBrobnextstate)");
    assert!(fired, "the attack cycle fired weapons");
    assert!(g.vars.bosshp > 0, "the walking-form bar accumulates");
}

#[test]
fn robot_walking_death_explodes() {
    // Phase 2: after transforming, a second HP0 routes through the expstrat's
    // walking branch -> GF_BOSSDEAD + boss explosion (killable Andross).
    let (mut g, boss) = setup(0, 1000);
    arm(&mut g, boss, bossb::IS_BOSSBROB);
    g.run_strategies();
    g.objs.aliens[boss as usize].hp = 0; // -> transform (phase 1)
    g.run_strategies();
    assert_eq!(g.vars.gameflags & GF_BOSSDEAD, 0, "not dead after transform");
    assert!(g.objs.aliens[boss as usize].hp > 0, "walking form has fresh HP");
    // Kill the walking form.
    g.objs.aliens[boss as usize].hp = 0;
    g.run_strategies();
    assert_ne!(g.vars.gameflags & GF_BOSSDEAD, 0, "walking death set GF_BOSSDEAD");
}

#[test]
fn robot_drains_on_damage() {
    let (mut g, boss) = setup(0, 5000);
    arm(&mut g, boss, bossb::IS_BOSSBROB);
    g.run_strategies();
    let full = g.vars.bosshp;
    g.objs.aliens[boss as usize].hp = 10;
    g.run_strategies();
    assert!(g.vars.bosshp < full, "m_bossHP bar dropped after damage");
}
