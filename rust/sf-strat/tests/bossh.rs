//! bossH ("gggy") legged-spider boss behavioral tests.
//!
//! ASM oracle: reference/ultrastarfox/SF/STRAT/D3STRATS.ASM:34-931
//! (bossh_istrat / bosshleg_istrat / bosshtop_istrat + sub-labels). No C-oracle
//! port and no sf-oracle differential fixture (bossH has neither an ISTRATS.ASM
//! row nor a ported C strategy). These tests drive the registered strategy
//! black-box through `run_strategies` and assert the ported state machine, HP-
//! bar model, attack cycle, the bosshhitcount phase gate, the leg-dead →
//! vulnerable transition, and death → explode against hand-derived ASM
//! expectations, cited inline.

use sf_game::alien::{ACF_COLLTYPE2, ACF_WEAPON, NUMBER_AL};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::COLLTYPE_ENEMY1;
use sf_strat::bossh;

const WM_RNDVAL: u16 = 0x1F00;
const ASF3_CHILDOBJ: u8 = 0x10; // enemy_a::ASF3_CHILDOBJ
const ASF_NOHITAFFECT: u8 = 0x40; // alien::ASF_NOHITAFFECT

// D3STRATS.ASM equs.
const BOSSH_HP: u8 = 64; // :34 bosshHP
const HITCOUNT_INIT: u8 = 5 * 2 + 5 * 5; // :76 = 35
const BOSSMAXHP: u16 = HITCOUNT_INIT as u16 + BOSSH_HP as u16; // 99

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

/// New game with a static player in slot 0 and the boss (nullshape) placed.
fn setup(player_z: i16, boss_x: i16, boss_y: i16, boss_z: i16) -> (Game, u16) {
    let mut g = Game::new();
    g.vars.write_ext16(WM_RNDVAL, 0x1234);
    g.vars.internal_playpt = 0;
    bossh::register(&mut g.world);

    let p = spawn(&mut g, 0, 0, player_z, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.sflags4 |= 0x01; // ASF4_PLAYEROBJ
    }
    let boss = spawn(&mut g, boss_x, boss_y, boss_z, 0);
    (g, boss)
}

fn arm(g: &mut Game, boss: u16) {
    let id = g
        .world
        .find_direct_strategy(bossh::STRATEGY_BOSSH)
        .expect("bossH strategy registered");
    g.objs.aliens[boss as usize].stratptr = Some(id);
}

fn active_count(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

fn weapon_count(g: &Game) -> usize {
    (0..NUMBER_AL)
        .filter(|&i| g.objs.aliens[i].active && g.objs.aliens[i].collflags & ACF_WEAPON != 0)
        .count()
}

/// Indices of the mother's linked child legs (child_num 1..=5).
fn legs(g: &Game, mother: u16) -> Vec<u16> {
    (0..NUMBER_AL)
        .filter(|&i| {
            let a = &g.objs.aliens[i];
            a.active
                && a.sflags3 & ASF3_CHILDOBJ != 0
                && (1..=5).contains(&a.sbyte1)
                && a.ptr as usize == mother as usize + 1 // boss_obj_index_or_null = idx+1
        })
        .map(|i| i as u16)
        .collect()
}

/// Child numbers reachable through the mother's ROM sword1 sibling chain.
fn linked_child_numbers(g: &Game, mother: u16) -> Vec<u8> {
    let mut result = Vec::new();
    let mut ptr = g.objs.aliens[mother as usize].sword1 as u16;
    let mut guard = NUMBER_AL + 1;
    while ptr != 0 && guard != 0 {
        guard -= 1;
        let idx = (ptr - 1) as usize;
        assert!(idx < NUMBER_AL, "family link outside the object pool");
        result.push(g.objs.aliens[idx].sbyte1);
        ptr = g.objs.aliens[idx].sword1 as u16;
    }
    assert_ne!(guard, 0, "family list contains a cycle");
    result
}

// ---------------------------------------------------------------------------
// Registration + map wiring.
// ---------------------------------------------------------------------------

#[test]
fn registers_typed_map_strategy() {
    let (g, _b) = setup(0, 0, -600, 5000);
    assert!(
        g.world
            .find_direct_strategy(bossh::STRATEGY_BOSSH)
            .is_some(),
        "bossH typed strategy resolves"
    );
}

// ---------------------------------------------------------------------------
// Init — HP bar seed + family generation.
// ---------------------------------------------------------------------------

#[test]
fn init_seeds_bar_and_generates_family() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    let base = active_count(&g); // player + mother
    arm(&mut g, boss);
    g.run_strategies();

    // s_set_aldata #bosshHP (D3:70).
    assert_eq!(g.objs.aliens[boss as usize].hp, BOSSH_HP, "hp = bosshHP");
    // s_set_bossmaxHP bosshhitcount(35) + s_add_bossmaxHP #bosshHP(64) = 99.
    assert_eq!(
        g.vars.bossmaxhp, BOSSMAXHP,
        "bossmaxHP = hitcount + bosshHP"
    );
    // s_add_bossHP x,al_hp (64) + s_add_bossHP bosshhitcount (35) = 99 = full bar.
    assert_eq!(g.vars.bosshp, BOSSMAXHP, "m_bossHP full bar accumulated");
    // .generate: 5 legs + top = +6 objects.
    assert_eq!(active_count(&g), base + 6, "5 legs + top generated");
    assert_eq!(legs(&g, boss).len(), 5, "five child legs linked");
    // ROM colltype_enemy1 = 0x10 (ACF_COLLTYPE2), not vars 0x01.
    let cf = g.objs.aliens[boss as usize].collflags;
    assert_ne!(cf & ACF_COLLTYPE2, 0);
    assert_eq!(cf & COLLTYPE_ENEMY1, 0);
    for leg in legs(&g, boss) {
        let lcf = g.objs.aliens[leg as usize].collflags;
        assert_ne!(lcf & ACF_COLLTYPE2, 0, "leg {leg}");
        assert_eq!(lcf & COLLTYPE_ENEMY1, 0, "leg {leg}");
    }
    // bosshhitcount seeded in the mother's gate byte (al_sbyte1).
    assert_eq!(g.objs.aliens[boss as usize].sbyte1, HITCOUNT_INIT);
    // nohitaffect set while legs live (body invulnerable).
    assert_ne!(g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT, 0);
}

// ---------------------------------------------------------------------------
// Attack cycle — modes advance + the top fires.
// ---------------------------------------------------------------------------

#[test]
fn attack_cycle_advances_and_top_fires() {
    // Boss centred + close: walkon completes fast, the mode machine advances,
    // and the spinning top fires HPLASMA through its forward window.
    let (mut g, boss) = setup(0, 0, -600, 1000);
    arm(&mut g, boss);
    g.run_strategies();
    let start_mode = g.objs.aliens[boss as usize].stratstate;

    let mut advanced = false;
    let mut fired = false;
    for _ in 0..400 {
        g.run_strategies();
        if g.objs.aliens[boss as usize].stratstate != start_mode {
            advanced = true;
        }
        if weapon_count(&g) > 0 {
            fired = true;
        }
        if advanced && fired {
            break;
        }
    }
    assert!(advanced, "the mother mode machine advanced (nxtmode)");
    assert!(fired, "the top fired its HPLASMA fire cycle");
    assert!(g.vars.bosshp > 0, "the bar keeps accumulating");
}

// ---------------------------------------------------------------------------
// bosshhitcount phase gate — scripted drop + leg death both drain it.
// ---------------------------------------------------------------------------

#[test]
fn droptoground_gate_drains_hitcount() {
    // Boss starts high (worldy -600) and centred; walkon -> spin -> droptoground
    // (mode 2) lands it at y=-80 and subtracts 5 from the gate (D3:356).
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();
    let hc0 = g.objs.aliens[boss as usize].sbyte1;
    let mut dropped = false;
    for _ in 0..200 {
        g.run_strategies();
        if g.objs.aliens[boss as usize].sbyte1 < hc0 {
            dropped = true;
            break;
        }
    }
    assert!(
        dropped,
        "a scripted droptoground drained the hitcount gate by 5"
    );
}

#[test]
fn leg_death_drains_gate_and_bar() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();
    let hc0 = g.objs.aliens[boss as usize].sbyte1;
    let leg = legs(&g, boss)[0];

    // Shoot a leg dead -> its expstrat (bosshleg_explode) fires.
    g.objs.aliens[leg as usize].hp = 0;
    g.run_strategies(); // leg explode: -5 gate + detach
                        // s_sub_var bosshhitcount,#5 (D3:853).
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte1,
        hc0 - 5,
        "leg death drained the gate by 5"
    );
    // The leg is gone from the mother's children.
    assert_eq!(legs(&g, boss).len(), 4, "one leg removed from the family");
    let linked = linked_child_numbers(&g, boss);
    assert_eq!(linked.len(), 5, "four legs plus the top remain linked");
    assert!(linked.contains(&6), "leg removal preserves the top link");
    // Re-sum: the bar reflects the reduced gate.
    g.run_strategies();
    assert!(
        g.vars.bosshp < BOSSMAXHP,
        "bar dropped below full after leg loss"
    );
}

// ---------------------------------------------------------------------------
// Body invulnerable until legs dead, then killable -> explode.
// ---------------------------------------------------------------------------

#[test]
fn body_invulnerable_until_all_legs_dead_then_explodes() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();

    // While legs live the body pins al_hp = bosshHP and stays nohitaffect
    // (s_jmp_childrendead else-branch, D3:566-568).
    g.objs.aliens[boss as usize].hp = 10; // simulate a chip
    g.run_strategies();
    assert_eq!(
        g.objs.aliens[boss as usize].hp, BOSSH_HP,
        "body HP re-pinned (invulnerable)"
    );
    assert_ne!(
        g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT,
        0,
        "still nohitaffect"
    );

    // Kill all five legs.
    for leg in legs(&g, boss) {
        g.objs.aliens[leg as usize].hp = 0;
    }
    g.run_strategies(); // legs explode + detach
    g.run_strategies(); // mother now sees childrendead -> becomes vulnerable
    assert!(legs(&g, boss).is_empty(), "all legs destroyed");
    // s_clr_alsflag nohitaffect (D3:571) — body killable now.
    assert_eq!(
        g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT,
        0,
        "body dropped nohitaffect once legs dead"
    );
    // With the gate drained (5 legs x -5 = -25 -> 10) the body HP is no longer
    // pinned; it now takes damage.
    let before = active_count(&g);
    g.objs.aliens[boss as usize].hp = 0; // -> expstrat bossh_explode
    g.run_strategies();
    // .explode -> strat_boss_explode_init spawns the explosion burst.
    assert!(
        active_count(&g) > before,
        "death spawned the boss explosion burst"
    );
}

// ---------------------------------------------------------------------------
// Bar drains on damage once the body is vulnerable.
// ---------------------------------------------------------------------------

#[test]
fn bar_tracks_body_hp_when_vulnerable() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();
    // Kill the legs so the body stops pinning its HP.
    for leg in legs(&g, boss) {
        g.objs.aliens[leg as usize].hp = 0;
    }
    g.run_strategies();
    g.run_strategies();
    let full = g.vars.bosshp;
    g.objs.aliens[boss as usize].hp = 20;
    g.run_strategies();
    assert!(
        g.vars.bosshp < full,
        "m_bossHP bar dropped after body damage"
    );
}
