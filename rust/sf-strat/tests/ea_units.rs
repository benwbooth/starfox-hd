//! enemy_a lane unit tests: pure helpers, ground strategies, and a
//! no-panic smoke run over every ported Istrat (debug-build overflow
//! checks make this a real arithmetic-wrap audit).

use sf_game::alien::{ASF_COLLDISABLE, ATGND};
use sf_game::game::{Game, StrategyFn};
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemy_a::{self, achase_angle, strat_tab_scaled, wm};
use sf_strat::ground;

// ============================================================
// Pure helpers
// ============================================================

#[test]
fn achase_angle_steps_match_rom() {
    // ROM Achase (STRATMAC.INC:525 / SR8_ACHASE_ALVAR3, oracle-proven in
    // sf-oracle tests/audit_strats_b.rs): adiv2 rounds toward zero and the
    // reached-branch fires only when already at the target ON ENTRY.
    // ROM sr8_achase = current + adiv2^N(target-current) (oracle-probed
    // fuzz_pure_fns). At the antipodal |diff|==128 tie, (target-current) as i8
    // = -128, adiv2^3 = -16, so 0 + (-16) = 240. (The old port subtracted
    // (current-target) and gave 16 — the exact-180 turn-direction bug.)
    let mut cur = 0u8;
    assert!(!achase_angle(&mut cur, 128, 3));
    assert_eq!(cur, 240);
    // From 240 toward 128: target-current = 128-240 = -112 (i8), step -14 -> 226.
    assert!(!achase_angle(&mut cur, 128, 3));
    assert_eq!(cur, 226);
    // Toward-zero rounding: 0 -> 100 at rate 3 steps 100/8 = 12 (the old
    // floor shift gave 13; ROM gives 12).
    let mut cur = 0u8;
    assert!(!achase_angle(&mut cur, 100, 3));
    assert_eq!(cur, 12);
    // Small positive diff truncates to zero -> forced step of 1.
    let mut cur = 5u8;
    assert!(!achase_angle(&mut cur, 2, 3));
    assert_eq!(cur, 4);
    // Stepping onto the target still reports false; "reached" is an
    // entry-equality check, one tick later (ROM beq-before-step).
    let mut cur = 3u8;
    assert!(!achase_angle(&mut cur, 2, 3));
    assert_eq!(cur, 2);
    assert!(achase_angle(&mut cur, 2, 3)); // already there
    // Wrap-around chase picks the short way (8-bit signed diff).
    let mut cur = 250u8;
    achase_angle(&mut cur, 10, 3);
    assert!(cur > 250 || cur < 10, "cur={cur}");
}

#[test]
fn tab_scaled_matches_sin_table() {
    // sin(DEG90)*127 = 127 -> >>4 = 7 (C strat_tab_scaled(angle, sin, -4)).
    assert_eq!(strat_tab_scaled(64, true, -4), 7);
    // cos(DEG0)*127 = 127 -> <<1 = 254.
    assert_eq!(strat_tab_scaled(0, false, 1), 254);
    // sin(DEG180) ~ 0.
    assert_eq!(strat_tab_scaled(128, true, 0), 0);
    // Negative half: sin(192 = DEG270) = -1 -> -127>>2 = -32 (arith shift).
    assert_eq!(strat_tab_scaled(192, true, -2), -32);
}

#[test]
fn points_positive_z_boundaries() {
    let mut al = sf_game::alien::Alien::default();
    for (roty, want) in [
        (0u8, true),
        (32, true),   // +DEG45 inclusive
        (33, false),
        (224, true),  // -DEG45 inclusive
        (223, false),
        (128, false),
    ] {
        al.roty = roty;
        assert_eq!(enemy_a::strat_points_positive_z(&al), want, "roty={roty}");
    }
}

#[test]
fn ea_random_is_prng_next() {
    // C PRNG_NEXT(rnd) = (rnd*91 + 0x61D7) & 0xFFFF (src/types.h:57).
    let mut g = Game::new();
    g.vars.write_ext16(wm::RNDVAL, 0x1234);
    let r1 = enemy_a::ea_random(&mut g);
    assert_eq!(r1, 0x1234u16.wrapping_mul(91).wrapping_add(0x61D7));
    let r2 = enemy_a::ea_random(&mut g);
    assert_eq!(r2, r1.wrapping_mul(91).wrapping_add(0x61D7));
    assert_eq!(g.vars.read_ext16(wm::RNDVAL), r2);
}

// ============================================================
// Ground strategies (C strat_ground.c oracle is fully covered here)
// ============================================================

fn game_with_obj() -> (Game, u16) {
    let mut g = Game::new();
    let idx = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    (g, idx)
}

#[test]
fn ground_stayrel_scrolls_and_disables_collision() {
    let (mut g, idx) = game_with_obj();
    g.vars.pviewvelz = 65;
    g.objs.aliens[idx as usize].worldz = 100;
    ground::strat_stayrel_init(&mut g, idx);
    let sid = g.objs.aliens[idx as usize].stratptr.expect("tick strat");
    g.call_strat(sid, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.worldz, 165);
    assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
    g.call_strat(sid, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 230);
}

#[test]
fn ground_gnd_is_inert_ground_type() {
    let (mut g, idx) = game_with_obj();
    ground::strat_gnd_init(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert!(al.stratptr.is_none());
    assert!(al.collstratptr.is_none());
    assert!(al.expstratptr.is_none());
    assert_ne!(al.type_ & ATGND, 0);
    assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
}

#[test]
fn ground_stayrelhard180yr_sets_hardvars_and_scrolls() {
    let (mut g, idx) = game_with_obj();
    g.vars.pviewvelz = -30;
    ground::strat_stayrelhard180yr_init(&mut g, idx);
    {
        let al = &g.objs.aliens[idx as usize];
        assert_eq!(al.hp, 0xFF); // HARD_HP
        assert_eq!(al.ap, 8); // HARD_AP
        assert_eq!(al.roty, 128); // DEG180
    }
    let sid = g.objs.aliens[idx as usize].stratptr.unwrap();
    g.call_strat(sid, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, -30);
}

#[test]
fn ground_staydist_offsets_from_view_z() {
    let (mut g, idx) = game_with_obj();
    g.vars.write_ext16(wm::PVIEWPOSZ, 1200u16);
    g.objs.aliens[idx as usize].sword1 = -200;
    ground::strat_staydist_init(&mut g, idx);
    {
        let al = &g.objs.aliens[idx as usize];
        assert_eq!(al.worldz, 1000);
        assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
    }
    // ROM staydist_Istrat (GSTRATS.ASM:706-711) re-runs every tick: the
    // object TRACKS pviewposz instead of freezing at the init-time value.
    let sid = g.objs.aliens[idx as usize]
        .stratptr
        .expect("staydist keeps a tick strat");
    g.vars.write_ext16(wm::PVIEWPOSZ, 1300u16);
    g.call_strat(sid, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 1100);
}

// ============================================================
// No-panic smoke run over every ported Istrat.
// ============================================================

fn smoke(f: StrategyFn) {
    let mut g = Game::new();
    g.vars.write_ext16(wm::RNDVAL, 0x77AB);
    g.vars.pviewvelz = 65;
    g.vars.minpmove_y = -60;
    g.vars.playerflymode = 8;
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);
    // Player slot 0.
    let p = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
    g.objs.aliens[0].shape = 2;
    g.objs.aliens[0].hp = 40;
    g.objs.aliens[0].sflags4 |= 0x01;
    // Subject.
    let e = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
    {
        let al = &mut g.objs.aliens[e as usize];
        al.shape = 30;
        al.worldx = 50;
        al.worldy = 20;
        al.worldz = 1500;
    }
    let sid = g.world.register_strategy(f);
    g.objs.aliens[e as usize].stratptr = Some(sid);
    for t in 0..80i32 {
        let al = &mut g.objs.aliens[0];
        al.worldz = (65 * (t + 1)) as i16;
        al.worldx = (((t * 11) % 200) - 100) as i16;
        al.worldy = (-50 + ((t * 5) % 90)) as i16;
        al.vz = 65;
        g.run_strategies();
    }
}

#[test]
fn smoke_all_istrats() {
    let fns: &[StrategyFn] = &[
        // ground.rs
        ground::strat_stayrel_init,
        ground::strat_gnd_init,
        ground::strat_stayrelhard180yr_init,
        ground::strat_staydist_init,
        // enemy_a.rs statics + misc
        enemy_a::strat_hard_init,
        enemy_a::strat_hard180yr_init,
        enemy_a::strat_hard90yr_init,
        enemy_a::strat_hard180yr_nzr_init,
        enemy_a::strat_hardrot_init,
        enemy_a::strat_nocoll_init,
        enemy_a::strat_rader0_init,
        enemy_a::strat_rader1_init,
        enemy_a::strat_pillar3_init,
        enemy_a::strat_skillfly_init,
        enemy_a::strat_gate3_init,
        enemy_a::strat_gate_init,
        enemy_a::strat_gate2_init,
        enemy_a::strat_boss1_init,
        enemy_a::strat_tow0_explode,
        enemy_a::strat_wormhead_init,
        enemy_a::strat_worm_init,
        enemy_a::strat_worm2_init,
        enemy_a::strat_item5_init,
        enemy_a::strat_item7_init,
        enemy_a::strat_up1man_init,
        enemy_a::strat_bomwing_init,
        enemy_a::strat_tadpole_init,
        enemy_a::strat_spacebarwalker_init,
        enemy_a::strat_spacebarshoot_init,
        enemy_a::strat_zacos_init,
        enemy_a::strat_tower0_init,
        enemy_a::strat_houdai_init,
        enemy_a::strat_houdai_ns_init,
        enemy_a::strat_zaco3_init,
        enemy_a::strat_zaco4_init,
        enemy_a::strat_zaco0_init,
        enemy_a::strat_para_init,
        enemy_a::strat_carrier_init,
        enemy_a::strat_base1_init,
        enemy_a::strat_cameleon_init,
        enemy_a::strat_hit_flash,
        enemy_a::strat_explode,
        enemy_a::strat_szaco2_init,
        enemy_a::strat_zaco1l_init,
        enemy_a::strat_zaco1r_init,
        enemy_a::strat_friendexitbase_init,
        enemy_a::strat_clship_warpa_init,
        enemy_a::strat_clship_warpb_init,
        enemy_a::strat_clship_warpc_init,
        enemy_a::strat_clship_gnda_init,
        enemy_a::strat_clship_gndb_init,
        enemy_a::strat_clship_gndc_init,
        enemy_a::strat_clship_eartha_init,
        enemy_a::strat_clship_earthb_init,
        enemy_a::strat_clship_earthc_init,
        enemy_a::strat_clship_chasea_init,
        enemy_a::strat_clship_chaseb_init,
        enemy_a::strat_clship_chasec_init,
        enemy_a::strat_clship_shipa_init,
        enemy_a::strat_clship_shipb_init,
        enemy_a::strat_clship_shipc_init,
        enemy_a::strat_clship_turna_init,
        enemy_a::strat_clship_turnb_init,
        enemy_a::strat_clship_turnc_init,
        enemy_a::strat_clship_bridgea_init,
        enemy_a::strat_clship_bridgeb_init,
        enemy_a::strat_clship_bridgec_init,
        enemy_a::strat_clship_divea_init,
        enemy_a::strat_clship_diveb_init,
        enemy_a::strat_clship_divec_init,
        enemy_a::strat_clship_undera_init,
        enemy_a::strat_clship_underb_init,
        enemy_a::strat_clship_underc_init,
        enemy_a::strat_boss_delay_explode_init,
        enemy_a::strat_qboss_explode_init,
        enemy_a::strat_boss_explode_init,
    ];
    for &f in fns {
        smoke(f);
    }
}

/// zaco3/zaco4 with a live target in range (they bail to a fallback path
/// when the target shape is absent, which the plain smoke run covers).
#[test]
fn smoke_zaco34_with_targets() {
    for (f, shape) in [
        (enemy_a::strat_zaco3_init as StrategyFn, 54u16), // SH_HOUDAI_0
        (enemy_a::strat_zaco4_init as StrategyFn, 27u16), // SH_PILLAR3
    ] {
        let mut g = Game::new();
        g.vars.write_ext16(wm::RNDVAL, 0x77AB);
        g.vars.pviewvelz = 65;
        let p = g.objs.alloc().unwrap();
        strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
        g.objs.aliens[0].shape = 2;
        g.objs.aliens[0].hp = 40;
        let tgt = g.objs.alloc().unwrap();
        strat_init_obj_vars(&mut g.objs.aliens[tgt as usize]);
        {
            let al = &mut g.objs.aliens[tgt as usize];
            al.shape = shape;
            al.hp = 8;
            al.worldx = 100;
            al.worldz = 2000;
        }
        let e = g.objs.alloc().unwrap();
        strat_init_obj_vars(&mut g.objs.aliens[e as usize]);
        {
            let al = &mut g.objs.aliens[e as usize];
            al.worldx = -300;
            al.worldy = 200;
            al.worldz = 1000;
        }
        let sid = g.world.register_strategy(f);
        g.objs.aliens[e as usize].stratptr = Some(sid);
        for t in 0..120i32 {
            g.objs.aliens[0].worldz = (65 * (t + 1)) as i16;
            g.objs.aliens[0].vz = 65;
            g.run_strategies();
        }
    }
}

// ============================================================
// Boss HP bar accumulator (m_bossHP / bossmaxhp), MDRAWLIS.MC:985-1057.
// ============================================================

/// boss1 seeds bossmaxhp once at init, then every living part re-adds its
/// current HP into m_bossHP each frame (zeroed in init_strats); damaging a
/// part drops the bar proportionally. Uses run_strategies (no collision) so
/// the parts stay alive across ticks.
#[test]
fn boss_hp_bar_accumulates_and_drains() {
    let mut g = Game::new();
    g.vars.write_ext16(wm::RNDVAL, 0x77AB);
    g.vars.pviewvelz = 65;
    g.vars.minpmove_y = -60;
    g.vars.playerflymode = 8;
    // Level 2 -> boss1 keeps full HP (level 1 halves it, GBSTRATS.ASM:99/102).
    g.vars.write_ext8(wm::CURRENTLEVEL, 2);

    // Player slot 0 (add_player_z / aim look it up).
    let p = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[p as usize]);
    g.objs.aliens[0].shape = 2;
    g.objs.aliens[0].hp = 40;
    g.objs.aliens[0].sflags4 |= 0x01;

    // boss1 mother: stratptr = the Istrat, which on its first tick spawns the
    // 8 turrets + cover and seeds bossmaxhp, then hands off to boss1up_strat.
    let boss = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[boss as usize]);
    g.objs.aliens[boss as usize].worldz = 1500;
    let init = g.world.register_strategy(enemy_a::strat_boss1_init as StrategyFn);
    g.objs.aliens[boss as usize].stratptr = Some(init);

    // First tick runs the init: bossmaxhp = mother(70) + 8 turrets * 8 = 134,
    // set ONCE (s_set_bossmaxHP + 8x s_add_bossmaxHP).
    g.run_strategies();
    assert_eq!(g.vars.bossmaxhp, 70 + 8 * 8, "bossmaxhp seeded at boss init");

    // Let the parts settle; every frame m_bossHP is zeroed then re-summed.
    for _ in 0..4 {
        g.run_strategies();
    }
    let full = g.vars.bosshp;
    assert_eq!(
        full, g.vars.bossmaxhp,
        "full bar: mother + all 8 turrets re-add their HP each frame"
    );
    assert!(full > 0);

    // Damage the mother by 30 HP; the accumulator (and the bar) must drop by
    // exactly that on the next frame.
    let hp = g.objs.aliens[boss as usize].hp;
    assert_eq!(hp, 70, "mother at full HP before damage");
    g.objs.aliens[boss as usize].hp = hp - 30;
    g.run_strategies();
    let drained = g.vars.bosshp;
    assert_eq!(drained, full - 30, "m_bossHP drops by the damage dealt");
    assert!(
        drained < g.vars.bossmaxhp,
        "bar reads less than full after damage ({drained} < {})",
        g.vars.bossmaxhp
    );
}

// ============================================================
// install() registration surface (table-lane contract)
// ============================================================

#[test]
fn install_registers_distinct_idempotent_ids() {
    let mut g = Game::new();
    let ea = enemy_a::install(&mut g);
    let gr = ground::install(&mut g);

    // All handles distinct (each entry point is its own registry slot).
    let ids = [
        ea.hard.0, ea.hard180yr.0, ea.hard90yr.0, ea.hard180yr_nzr.0,
        ea.hardrot.0, ea.nocoll.0, ea.rader0.0, ea.rader1.0, ea.pillar3.0,
        ea.skillfly.0, ea.gate3.0, ea.gate.0, ea.gate2.0, ea.boss1.0,
        ea.tow0_explode.0, ea.wormhead.0, ea.worm.0, ea.worm2.0, ea.item5.0,
        ea.item7.0, ea.bomwing.0, ea.tadpole.0, ea.spacebarwalker.0,
        ea.spacebarshoot.0, ea.up1man.0, ea.zacos.0, ea.tower0.0,
        ea.houdai_ns.0, ea.houdai.0, ea.zaco3.0, ea.zaco4.0, ea.zaco0.0,
        ea.para.0, ea.carrier.0, ea.base1.0, ea.cameleon.0, ea.szaco2.0,
        ea.zaco1l.0, ea.zaco1r.0, ea.friendexitbase.0, ea.clship_warpa.0,
        ea.clship_warpb.0, ea.clship_warpc.0, ea.clship_gnda.0,
        ea.clship_gndb.0, ea.clship_gndc.0, ea.clship_eartha.0,
        ea.clship_earthb.0, ea.clship_earthc.0, ea.clship_chasea.0,
        ea.clship_chaseb.0, ea.clship_chasec.0, ea.boss_delay_explode.0,
        ea.qboss_explode.0, ea.boss_explode.0, ea.hit_flash.0, ea.explode.0,
        gr.stayrel.0, gr.gnd.0, gr.stayrelhard180yr.0, gr.staydist.0,
    ];
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate StratId in install()");

    // Idempotent: a second install returns the same handles (sid memoizes
    // on function identity, like C function addresses).
    let ea2 = enemy_a::install(&mut g);
    let gr2 = ground::install(&mut g);
    assert_eq!(ea2.hard.0, ea.hard.0);
    assert_eq!(ea2.boss1.0, ea.boss1.0);
    assert_eq!(ea2.explode.0, ea.explode.0);
    assert_eq!(gr2.staydist.0, gr.staydist.0);

    // Handles are callable through the registry (spot check: gnd init
    // makes an inert ATGND object via its registered id).
    let idx = g.objs.alloc().unwrap();
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.call_strat(gr.gnd, idx);
    assert_ne!(g.objs.aliens[idx as usize].type_ & ATGND, 0);
}
