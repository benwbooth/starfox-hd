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
const ASF4_PLAYEROBJ: u8 = 0x01;
const TEST_RANDOM_SEED: u16 = 4660;
const ANIMATION_FRAME_MASK: u8 = 0x7F;

// D3STRATS.ASM equs.
const BOSSH_HP: u8 = 64; // :34 bosshHP
const BOSSHLEG_PROTECTED_HP: u8 = 74;
const BOSSHLEG_RAISE_HP: u8 = 63;
const HITCOUNT_INIT: u8 = 5 * 2 + 5 * 5; // :76 = 35
const BOSSMAXHP: u16 = HITCOUNT_INIT as u16 + BOSSH_HP as u16; // 99
const LEG_MODE_WAGGLE: u8 = 6;
const MOTHER_WAIT_FOR_SECOND_RAISE: u8 = 9;
const MOTHER_ATTACK_LOOP: u8 = 13;
const TELEPORT_CHILD_NUMBER: u8 = 7;
const RED_COLOR_TABLE: u16 = 1;
const FIRST_DROP_HITCOUNT: u8 = 30;
const SECOND_DROP_HITCOUNT: u8 = 25;
const FIRST_LEG_CHILD_NUMBER: u8 = 1;
const LAST_LEG_CHILD_NUMBER: u8 = 5;
const TOP_CHILD_NUMBER: u8 = 6;
const LEG_COUNT: usize = 5;
const CHILD_POSITION_SCALE: u32 = 3;
const LEG_ONE_LOCAL_POSITION: (i8, i8, i8) = (0, 5, 15);
const HPLASMA_LIFETIME_AFTER_SPAWN_PASS: u8 = 254;
const FIRST_PHASE_TIMEOUT: usize = 400;
const FULL_CHOREOGRAPHY_TIMEOUT: usize = 1200;
const LEG_RAISE_TIMEOUT: usize = 32;
const ATTACK_TIMEOUT: usize = 400;
const DROP_TIMEOUT: usize = 300;

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
    g.vars.write_ext16(WM_RNDVAL, TEST_RANDOM_SEED);
    g.vars.internal_playpt = 0;
    bossh::register(&mut g.world);

    let p = spawn(&mut g, 0, 0, player_z, 2);
    {
        let al = &mut g.objs.aliens[p as usize];
        al.hp = 3;
        al.sflags4 |= ASF4_PLAYEROBJ;
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

/// Indices of the mother's linked child legs.
fn legs(g: &Game, mother: u16) -> Vec<u16> {
    (0..NUMBER_AL)
        .filter(|&i| {
            let a = &g.objs.aliens[i];
            a.active
                && a.sflags3 & ASF3_CHILDOBJ != 0
                && (FIRST_LEG_CHILD_NUMBER..=LAST_LEG_CHILD_NUMBER).contains(&a.sbyte1)
                && a.ptr as usize == mother as usize + 1 // boss_obj_index_or_null = idx+1
        })
        .map(|i| i as u16)
        .collect()
}

fn child(g: &Game, mother: u16, child_number: u8) -> Option<u16> {
    (0..NUMBER_AL)
        .find(|&index| {
            let object = &g.objs.aliens[index];
            object.active
                && object.sflags3 & ASF3_CHILDOBJ != 0
                && object.sbyte1 == child_number
                && object.ptr as usize == mother as usize + 1
        })
        .map(|index| index as u16)
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
    // Current body HP (64) plus bosshhitcount (35) yields a full bar of 99.
    assert_eq!(g.vars.bosshp, BOSSMAXHP, "m_bossHP full bar accumulated");
    // .generate: 5 legs + top = +6 objects.
    assert_eq!(active_count(&g), base + 6, "5 legs + top generated");
    assert_eq!(legs(&g, boss).len(), LEG_COUNT, "five child legs linked");
    // ROM colltype_enemy1 = 0x10 (ACF_COLLTYPE2), not vars 0x01.
    let cf = g.objs.aliens[boss as usize].collflags;
    assert_ne!(cf & ACF_COLLTYPE2, 0);
    assert_eq!(cf & COLLTYPE_ENEMY1, 0);
    for leg in legs(&g, boss) {
        let lcf = g.objs.aliens[leg as usize].collflags;
        assert_ne!(lcf & ACF_COLLTYPE2, 0, "leg {leg}");
        assert_eq!(lcf & COLLTYPE_ENEMY1, 0, "leg {leg}");
        let child_number = g.objs.aliens[leg as usize].sbyte1;
        assert_eq!(g.objs.aliens[leg as usize].hp, BOSSHLEG_PROTECTED_HP);
        assert_eq!(
            g.objs.aliens[leg as usize].animframe & ANIMATION_FRAME_MASK,
            child_number.wrapping_sub(2) & ANIMATION_FRAME_MASK,
            "leg {child_number} starts on its authored frame"
        );
    }

    // child_rotpos_l scales authored child bytes by childscale=3 before the
    // mother's full orientation is applied (DSTRATS.ASM:6939-6972).
    let leg_one = child(&g, boss, FIRST_LEG_CHILD_NUMBER).expect("first leg linked");
    let mother = g.objs.aliens[boss as usize];
    let (offset_x, offset_y, offset_z) = sf_strat::snes_trig::strat_roffs_full_scaled(
        mother.rotz,
        mother.rotx,
        mother.roty,
        LEG_ONE_LOCAL_POSITION.0,
        LEG_ONE_LOCAL_POSITION.1,
        LEG_ONE_LOCAL_POSITION.2,
        CHILD_POSITION_SCALE,
    );
    let leg = g.objs.aliens[leg_one as usize];
    assert_eq!(leg.worldx, mother.worldx.wrapping_add(offset_x));
    assert_eq!(leg.worldy, mother.worldy.wrapping_add(offset_y));
    assert_eq!(leg.worldz, mother.worldz.wrapping_add(offset_z));
    // bosshhitcount seeded in the mother's gate byte (al_sbyte1).
    assert_eq!(g.objs.aliens[boss as usize].sbyte1, HITCOUNT_INIT);
    // nohitaffect set while legs live (body invulnerable).
    assert_ne!(g.objs.aliens[boss as usize].sflags & ASF_NOHITAFFECT, 0);
}

#[test]
fn complete_leg_and_mother_phase_choreography_reaches_the_teleport_loop() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();

    for leg in legs(&g, boss).into_iter().take(3) {
        g.objs.aliens[leg as usize].hp = BOSSHLEG_RAISE_HP;
    }
    for _ in 0..FIRST_PHASE_TIMEOUT {
        g.run_strategies();
        if g.objs.aliens[boss as usize].stratstate == MOTHER_WAIT_FOR_SECOND_RAISE
            && g.objs.aliens[boss as usize].sbyte1 == FIRST_DROP_HITCOUNT
        {
            break;
        }
    }
    assert_eq!(
        g.objs.aliens[boss as usize].stratstate, MOTHER_WAIT_FOR_SECOND_RAISE,
        "the first raise/drop, middle pose, and scuttle sequence completed"
    );
    assert_eq!(g.objs.aliens[boss as usize].sbyte1, FIRST_DROP_HITCOUNT);

    for leg in legs(&g, boss).into_iter().take(3) {
        g.objs.aliens[leg as usize].hp = BOSSHLEG_RAISE_HP;
    }
    let mut saw_teleporter = false;
    let mut teleporter_followed_mother = false;
    let mut returned_to_attack_loop = false;
    for _ in 0..FULL_CHOREOGRAPHY_TIMEOUT {
        g.run_strategies();
        let linked = linked_child_numbers(&g, boss);
        saw_teleporter |= linked.contains(&TELEPORT_CHILD_NUMBER);
        if let Some(teleporter) = child(&g, boss, TELEPORT_CHILD_NUMBER) {
            let mother = g.objs.aliens[boss as usize];
            let teleporter = g.objs.aliens[teleporter as usize];
            assert_eq!(teleporter.worldx, mother.worldx);
            assert_eq!(teleporter.worldy, mother.worldy);
            assert_eq!(teleporter.worldz, mother.worldz);
            assert_ne!(teleporter.collflags & ACF_COLLTYPE2, 0);
            teleporter_followed_mother = true;
        }
        if saw_teleporter
            && !linked.contains(&TELEPORT_CHILD_NUMBER)
            && g.objs.aliens[boss as usize].stratstate == MOTHER_ATTACK_LOOP
        {
            returned_to_attack_loop = true;
            break;
        }
    }
    assert_eq!(
        g.objs.aliens[boss as usize].sbyte1, SECOND_DROP_HITCOUNT,
        "both authored ground impacts drained the phase gate"
    );
    assert!(
        legs(&g, boss)
            .iter()
            .all(|&leg| g.objs.aliens[leg as usize].coltab == RED_COLOR_TABLE),
        "the second impact made every leg vulnerable"
    );
    assert!(saw_teleporter, "the authored teleport child was created");
    assert!(
        teleporter_followed_mother,
        "the teleport child followed the mother's position"
    );
    assert!(
        returned_to_attack_loop,
        "teleport completion and the 30-tick delay returned to the attack loop"
    );
}

#[test]
fn damaged_leg_raises_waggles_and_restores_its_protected_hp() {
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();
    let leg = legs(&g, boss)[0];
    g.objs.aliens[leg as usize].hp = BOSSHLEG_RAISE_HP;
    for _ in 0..LEG_RAISE_TIMEOUT {
        g.run_strategies();
        if g.objs.aliens[leg as usize].stratstate == LEG_MODE_WAGGLE {
            break;
        }
    }
    assert_eq!(g.objs.aliens[leg as usize].stratstate, LEG_MODE_WAGGLE);
    assert_eq!(g.objs.aliens[leg as usize].hp, BOSSHLEG_PROTECTED_HP);
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
    for _ in 0..ATTACK_TIMEOUT {
        g.run_strategies();
        if g.objs.aliens[boss as usize].stratstate != start_mode {
            advanced = true;
        }
        if weapon_count(&g) > 0 {
            fired = true;
            let shot = (0..NUMBER_AL)
                .find(|&index| {
                    g.objs.aliens[index].active && g.objs.aliens[index].collflags & ACF_WEAPON != 0
                })
                .expect("weapon object");
            let shot = g.objs.aliens[shot];
            assert_eq!(shot.ptr, 1, "the top targets the player object");
            assert_ne!(shot.collflags & ACF_COLLTYPE2, 0);
            assert_eq!(
                shot.count, HPLASMA_LIFETIME_AFTER_SPAWN_PASS,
                "the source-linked projectile runs once on its spawn pass"
            );
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
    // Three legs crossing below the protected 64-HP boundary raise into the
    // waggle state. The mother observes those three authored states, advances
    // from spin into the first drop, lands at y=-80, and drains five points.
    let (mut g, boss) = setup(0, 0, -600, 5000);
    arm(&mut g, boss);
    g.run_strategies();
    let hc0 = g.objs.aliens[boss as usize].sbyte1;
    for leg in legs(&g, boss).into_iter().take(3) {
        g.objs.aliens[leg as usize].hp = BOSSHLEG_RAISE_HP;
    }
    let mut dropped = false;
    for _ in 0..DROP_TIMEOUT {
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
    assert!(
        linked.contains(&TOP_CHILD_NUMBER),
        "leg removal preserves the top link"
    );
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
