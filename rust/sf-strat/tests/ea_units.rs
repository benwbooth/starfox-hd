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
fn achase_angle_steps_match_c() {
    // diff = (int8)(0-128) = -128, step = -128>>3 = -16 -> cur = 16.
    let mut cur = 0u8;
    assert!(!achase_angle(&mut cur, 128, 3));
    assert_eq!(cur, 16);
    // diff = (int8)(16-128) = -112, step = -14 -> cur = 30.
    assert!(!achase_angle(&mut cur, 128, 3));
    assert_eq!(cur, 30);
    // Small positive diff floors to zero -> forced step of 1.
    let mut cur = 5u8;
    assert!(!achase_angle(&mut cur, 2, 3));
    assert_eq!(cur, 4);
    // Stepping onto the target returns true immediately.
    let mut cur = 3u8;
    assert!(achase_angle(&mut cur, 2, 3));
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
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.worldz, 1000);
    assert_ne!(al.sflags & ASF_COLLDISABLE, 0);
    assert!(al.stratptr.is_none());
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
