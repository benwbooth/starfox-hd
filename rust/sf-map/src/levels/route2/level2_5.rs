//! MAP_ID_2_5 — Venom 2 Orbital (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_5_slice()` +
//! `register_level2_5_inline_callbacks()`.
//! ASM sources: LEVEL2_5.ASM wrapper, MAP2_5.ASM body, CL_DIVE.ASM.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::MapBuilder;

/// C `build_level2_5_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapjsr("level2_5.map2_5");
    b.mapjsr("cl_dive");
    // mapend__not level2_6: sets levelfinished=7, game loop handles transition.
    b.mapend(7);

    // MAP2_5.ASM — Venom 2 Orbital subroutine.
    b.label("level2_5.map2_5");

    // mapwait 600
    b.mapwait(600);

    // Lines 4-6: pathspecial / pathcspecial trio (egu6)
    b.pathspecial(0, 2700, 2000, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(0, 2500, 2000, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(3000, 2900, 2000, 2100, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    // Lines 8-10: pathspecial / pathcspecial trio (egu6 variants)
    b.pathspecial(0, -2700, 2000, 1500, SH_S_ZACO_0, PATH_ID_EGU6_IFAL, 10, 10);
    b.pathcspecial(0, -2500, 2000, 1800, SH_ZACO_8, PATH_ID_EGU6_IRAB, 10, 10);
    b.pathcspecial(9000, -2900, 2000, 2100, SH_ZACO_8, PATH_ID_EGU6_IFRO, 10, 10);

    // Lines 12-14: pathcspecial / pathspecial / pathcspecial trio (egu5)
    b.pathcspecial(400, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathspecial(400, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    b.pathcspecial(7000, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);

    // Lines 16-17: friendship_4 chase + zaco_b chase
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathcspecial(8000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    // Lines 19-25: check + minicas2 group
    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    b.pathobj(700, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(700, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(700, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(700, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(2500, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Lines 26-32: mapmother + cspecial uper_m group + maprem
    b.mapmother(1000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, crate::mothers::mother_maps().map_uperm);
    b.cspecial(1000, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.mapremove(SH_MOTHER1);

    // Lines 34-35: pathspecial egu6 pair
    b.pathspecial(400, -2700, 2200, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    b.pathspecial(400, 2700, 2200, 1500, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);

    // Lines 37-38: pathcspecial egu6 pair
    b.pathcspecial(400, -2500, 2200, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(400, 2500, 2200, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    // Lines 40-41: pathcspecial egu6 variants pair
    b.pathcspecial(400, -2900, 2200, 2100, SH_ZACO_8, PATH_ID_EGU6_IRAB, 10, 10);
    b.pathcspecial(6000, 2900, 2200, 2100, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);

    // Lines 43-47: check + minicas2 group
    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    b.pathobj(800, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // === Skillfly section 1 (lines 48-68) ===
    b.skillfly_init();
    b.skillfly_set(0, -50, 4000, 100);

    // Lines 51-56: damyscr pathcspecials with setalvar
    b.pathcspecial(0, 180, 100, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathcspecial(0, -180, 100, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathcspecial(0, 0, -200, 4000, SH_BOSS_E_4, PATH_ID_DAMYSCR, 10, 10);
    b.setalvarb(AL_SBYTE1, 1);

    // Lines 57-61: minicas2 group
    b.pathobj(800, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(800, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Line 62: skillfly_bonus item_7
    let level2_5_skillfly_bonus0_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, -50, 2000, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level2_5.skillfly_bonus_0_skip");

    // Lines 63-67: setalvar + more minicas2 + item_5 mapobj
    b.setalvarb(AL_SBYTE1, 1);
    b.pathobj(800, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.mapobj(0, -100, -100, 3500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathobj(800, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Line 69: gate_0 mapobj
    b.mapobj(0, 0, 0, 4000, SH_GATE_0, IS_GATE);

    // Lines 71-72: e_gate pathobj + mapwait 1600
    b.pathobj(300, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    b.mapwait(1600);

    // Lines 74-81: check + minicas2 group + chase1 + more minicas2
    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    b.pathobj(700, -200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(700, 200, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 10, 10);
    b.pathcspecial(0, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    b.pathobj(700, 0, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(700, 100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);
    b.pathobj(4000, -100, 2000, 2600, SH_BOSS_E_4, PATH_ID_MINICAS2, 10, 10);

    // Lines 83-84: zaco_4 egu3 pair
    b.pathcspecial(0, 800, 1300, 2000, SH_ZACO_4, PATH_ID_EGU3, 10, 10);
    b.pathcspecial(1000, -800, 1300, 2300, SH_ZACO_4, PATH_ID_EGU3, 10, 10);

    // Line 86: cspecial wait=500,x=0,y=Space_viewCY-500,z=800
    b.cspecial(500, 0, SPACE_VIEWCY - 500, 800, SH_ZACO_4, IS_SZACO0);

    // Lines 88-89: zaco_4 egu3 pair
    b.pathcspecial(0, 200, 1900, 2000, SH_ZACO_4, PATH_ID_EGU3, 10, 10);
    b.pathcspecial(4000, -200, 1900, 2300, SH_ZACO_4, PATH_ID_EGU3, 10, 10);

    // Lines 91-93: bzaco_8 + s_zaco_0 egu5 trio
    b.pathcspecial(300, -300, 2200, 2000, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathspecial(300, 0, 2200, 1700, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    b.pathcspecial(8000, 300, 2200, 2300, SH_BZACO_8, PATH_ID_EGU5, 10, 10);

    // Lines 95-99: mapmother + cspecial uper_m group (second mother)
    b.mapmother(1000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, crate::mothers::mother_maps().map_uperm);
    b.cspecial(1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);

    // Lines 101-102: friendship_4 chase3 + zaco_b chase3
    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 10, 10);
    b.pathcspecial(10000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // Lines 104-110: bazookaL + uper_m group + maprem + bazookaR
    b.cspecial(1000, -150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    b.cspecial(1000, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, 200, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1000, -200, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.mapremove(SH_MOTHER1);
    b.cspecial(5500, 150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);

    // Lines 112-115: egu6 pathspecial + pathcspecial pairs
    b.pathspecial(0, -2000, 2000, 2000, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    b.pathspecial(0, 2000, 2000, 2300, SH_S_ZACO_0, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(0, -2000, -2000, 2600, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(9000, 2000, -2000, 2900, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);

    // Lines 117-119: wire_man + bazooka pair
    b.cspecial(6000, 0, 1500, 2000, SH_WIRE_MAN, IS_WIREMAN);
    b.cspecial(4000, -150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    b.cspecial(4000, 150, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);

    // === Skillfly section 2 (lines 121-143) ===
    b.skillfly_init();
    b.skillfly_set(0, -100, 2500, 120);

    b.pathcspecial(1000, 0, -100, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(1000, -500, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(300, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathspecial(300, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);

    // Line 128: skillfly_bonus item_5
    let level2_5_skillfly_bonus1_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, -100, 2000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level2_5.skillfly_bonus_1_skip");

    b.pathcspecial(300, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathcspecial(1000, -200, 50, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(800, 500, 0, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(800, -300, -150, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(800, 100, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(300, -300, 2200, 2800, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathspecial(300, 0, 2200, 2500, SH_S_ZACO_0, PATH_ID_EGU5, 10, 10);
    b.pathcspecial(300, 300, 2200, 3100, SH_BZACO_8, PATH_ID_EGU5, 10, 10);
    b.pathcspecial(600, -200, 50, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathobj(0, -250, -350, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    b.pathcspecial(600, 0, -150, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(600, 500, -130, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(400, -500, -200, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);
    b.pathcspecial(3400, 200, -100, 2500, SH_BOSS_E_4, PATH_ID_MINICAS0, 10, 10);

    // Line 145: kastmsg
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_KASTMSG, 10, 10);

    // Lines 151-152: fadeoutbgm + setbgm 5
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);

    // Line 153: incmap castanet — spawn Metal Smasher boss
    b.mapobj(0, 0, 0, 2000, SH_NULLSHAPE, IS_CASTANET);

    // Lines 154-155: mapwaitboss / markboss boss25
    b.mapwait(100);
    let level2_5_mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level2_5.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level2_5.bosswait.cont");
    b.mapgoto("level2_5.bosswait.loop");
    b.label("level2_5.bosswait.cont");
    let level2_5_mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let level2_5_mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss25
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    // Line 157: mapwait 2400
    b.mapwait(2400);

    // Line 159: maprts
    b.maprts();

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    submaps::append_cl_dive_submap(&mut b);

    b.resolve();

    // C: skillfly skip-label lookups (C falls back to 0; they must exist).
    assert!(b.lookup_label("level2_5.skillfly_bonus_0_skip").is_some());
    assert!(b.lookup_label("level2_5.skillfly_bonus_1_skip").is_some());

    let (data, labels) = b.finish();

    // C `register_level2_5_inline_callbacks()` — registration-call order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (level2_5_skillfly_bonus0_guard_ptr, "level2_5_skillfly_bonus0_guard"),
            (level2_5_skillfly_bonus1_guard_ptr, "level2_5_skillfly_bonus1_guard"),
            (level2_5_mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (level2_5_mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (level2_5_mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
