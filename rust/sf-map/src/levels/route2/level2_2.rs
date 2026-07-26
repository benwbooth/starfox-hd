//! MAP_ID_2_2 — Sector X (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_2_wrapper_slice()` +
//! `register_level2_2_inline_callbacks()`.
//! ASM sources: LEVEL2_2.ASM wrapper, MAP2_2.ASM body, CL_EARTH.ASM.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::{BarShapeMode, MapBuilder};

/// C `build_level2_2_wrapper_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapjsr("map2_2");
    b.mapjsr("cl_earth");
    b.mapend(1);

    // MAP2_2.ASM literal port through the moving shooter wall and boss handoff.
    b.label("map2_2");
    b.map_setbarshape(BarShapeMode::Solid, false);

    b.mapwait(600);

    b.cspecial(1500, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    b.cspecial(1500, 1000, SPACE_VIEWCY - 500, 800, SH_ZACO_4, IS_SZACO0);
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    b.cspecial(5000, 1000, SPACE_VIEWCY, 800, SH_ZACO_4, IS_SZACO0);
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathcspecial(9000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    b.pathcspecial(0, 2500, -2000, 1800, SH_ZACO_8, PATH_ID_EGU6, 10, 10);
    b.pathcspecial(12000, -2500, -2000, 2100, SH_ZACO_8, PATH_ID_EGU6, 10, 10);

    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    b.pathcspecial(
        0,
        2 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY - SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST,
        SH_WALKER_2,
        PATH_ID_PYONTA,
        10,
        10,
    );
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype7(0, -5, 0, 0);
    b.map_sbtype7(0, 5, 0, 0);
    b.label("level2_2.solidbar1");
    b.map_sbtype1(2, 0, 1, 0);
    b.map_sbtype1(0, 0, -1, 0);
    b.maploop("level2_2.solidbar1", 3);

    b.map_sbtype7(0, -6, 0, 0);
    b.map_sbtype7(0, 6, 0, 0);
    b.map_sbtype1(2, 0, 1, 0);
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype1(2, 0, 1, 0);
    b.special(
        0,
        0,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST,
        SH_S_WARK_0,
        IS_SPACEBARWALKER,
    );
    b.map_sbtype1(4 * 2, 0, 1, 0);

    b.pathobj(0, 900, -60, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE4_1, 200, 10);
    b.pathcspecial(0, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_2, 200, 10);
    b.pathcspecial(1000, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_3, 200, 10);

    b.pathcspecial(
        0,
        2 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST,
        SH_WALKER_2,
        PATH_ID_PYONTA,
        10,
        10,
    );
    b.map_sbtype1(4 * 2, 0, 1, 0);
    b.map_sbtype0(1 * 2, -3, 0, 0);
    b.map_sbtype0(2 * 2, 2, 1, 0);
    b.map_sbtype0(1 * 2, -4, 0, 0);
    b.map_sbtype0(2 * 2, 4, 1, 0);
    b.map_sbtype6(4 * 2, 0, 0, 0);

    b.mapobj(0, 100, -100, 3000, SH_NULLSHAPE, IS_UP1MAN);
    b.setalvarw(AL_SWORD2, SH_ITEM_0 as i32);
    b.mapwait(2000);

    b.map_sbtype7(4 * 2, 1, 1, 0);
    b.map_sbtype5(1 * 2, -1, -1, 0);
    b.map_sbtype5(6 * 2, 1, 1, 0);

    b.map_sbtype0(0, -1, 0, 0);
    b.map_sbtype0(0, 6, 0, 0);
    b.map_sbtype0(3, 1, 0, 0);
    b.map_sbtype0(0, -1, 0, 0);
    b.map_sbtype0(0, -6, 0, 0);
    b.map_sbtype0(3, 1, 0, 0);
    b.map_sbtype0(0, -1, 0, 0);
    b.map_sbtype0(0, 4, 0, 0);
    b.map_sbtype0(3, 1, 0, 0);
    b.map_sbtype0(0, -1, 0, 0);
    b.map_sbtype0(0, -4, 0, 0);
    b.label("level2_2.solidbar2");
    b.map_sbtype0(3, 1, 0, 0);
    b.map_sbtype0(0, -1, 0, 0);
    b.maploop("level2_2.solidbar2", 2);

    b.map_sbtype0(0, 1, 0, 0);
    b.map_sbtype0(0, -2, 0, 0);
    b.map_sbtype0(0, 2, 0, 0);
    b.map_sbtype0(0, -3, 0, 0);
    b.map_sbtype0(4, 3, 0, 0);

    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
    b.map_sbtype15(0, 0, 0, 0, 0, 4);
    b.map_sbtype10(5, -5, 0, 0);
    b.map_sbtype15(5, 0, 0, 0, 0, 4);
    b.map_sbtype15(5, 1, 0, 0, 0, 4);
    b.map_sbtype15(5, 2, 0, 0, 0, 4);
    b.map_sbtype15(5, 1, 0, 0, 0, 4);
    b.map_sbtype15(5, 0, 0, 0, 0, 4);
    b.mapobj(
        0,
        50,
        SPACE_VIEWCY + 50,
        SPACEBAR_BASE_DIST,
        SH_ITEM_5,
        IS_ITEM5,
    );
    b.setalvarb(AL_SBYTE1, 1);
    b.map_sbtype15(5, 0, 0, 0, 0, 4);
    b.map_sbtype15(5, -1, 0, 0, 0, 4);
    b.map_sbtype15(0, -2, 0, 0, 0, 4);
    b.map_sbtype10(5, 5, 0, 0);
    b.map_sbtype15(5, -1, 0, 0, 0, 4);
    b.map_sbtype15(5, 0, 0, 0, 0, 4);

    b.mapobj(
        0,
        -4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        SPACEBAR_BASE_DIST,
        SH_COLONY3R,
        IS_NOCOLL,
    );
    b.mapobj(
        2000,
        4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        SPACEBAR_BASE_DIST,
        SH_COLONY3L,
        IS_NOCOLL,
    );

    b.cspecial(0, -500, -300, 4000, SH_W_L, IS_WINGLAZERMAN);
    b.special(0, 300, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(1000, -300, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(0, 0, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(2000, 0, SPACE_VIEWCY - 250, 800, SH_CAMELEON, IS_CAMELEON);

    b.pathcspecial(0, 2500, -2000, 1800, SH_ZACO_8, PATH_ID_EGU6_IFAL, 10, 10);
    b.pathcspecial(
        6000,
        -2500,
        -2000,
        2400,
        SH_ZACO_8,
        PATH_ID_EGU6_IRAB,
        10,
        10,
    );

    b.cspecial(0, 250, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(
        1000,
        -250,
        SPACE_VIEWCY - 250,
        800,
        SH_CAMELEON,
        IS_CAMELEON,
    );
    b.cspecial(0, -250, SPACE_VIEWCY + 250, 800, SH_CAMELEON, IS_CAMELEON);
    b.special(4000, 250, SPACE_VIEWCY - 250, 800, SH_CAMELEON, IS_CAMELEON);

    b.map_sbtype8(1 * 2, -2, 0, 0);
    b.map_sbtype8(1 * 2, 1, 0, 0);
    b.map_sbtype_a(1 * 2, -2, 0, 0);
    b.map_sbtype_d(6 * 2, 2, 0, 0);

    b.mapobj(0, 0, SPACE_VIEWCY, SPACEBAR_BASE_DIST, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);

    b.special(
        0,
        -2 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST,
        SH_S_WARK_0,
        IS_SPACEBARWALKER,
    );
    b.cspecial(
        0,
        2 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST,
        SH_BWARKER_3,
        IS_SPACEBARWALKER,
    );
    b.map_sbtype_e(2 * 2, 4, 1, 0);
    b.map_sbtype_c(6 * 2, 3, 0, 0);
    b.map_sbtype3(3 * 2, 0, 0, 0);
    b.map_sbtype6(3 * 2, 0, 0, 0);

    b.mapobj(
        0,
        SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        SPACEBAR_BASE_DIST + (2 * SPACEBAR_UNIT_LEN),
        SH_ITEM_6,
        IS_ITEM6,
    );

    b.special(0, 350, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(3000, -350, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.setalvarb(AL_SBYTE1, 1);

    b.map_sbtype11(4 * 2, 0, 0, 0);
    b.map_sbtype_c(0, -1, 0, 0);
    b.map_sbtype_b(1 * 2, -1, 0, 0);
    b.map_sbtype_b(1 * 2, 1, -1, 0);
    b.map_sbtype8(1 * 2, -1, 0, 0);

    b.cspecial(
        0,
        -2 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST + (4 * SPACEBAR_UNIT_LEN),
        SH_BWARKER_3,
        IS_SPACEBARWALKER,
    );
    b.special(
        0,
        0,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST + (6 * SPACEBAR_UNIT_LEN),
        SH_S_WARK_0,
        IS_SPACEBARWALKER,
    );
    b.cspecial(
        0,
        3 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY + SPACEBAR_UNIT_LEN,
        SPACEBAR_BASE_DIST + (7 * SPACEBAR_UNIT_LEN),
        SH_BWARKER_3,
        IS_SPACEBARWALKER,
    );
    b.map_sbtype_f(15 * 2, 0, 1, 0);

    b.mapobj(0, 0, -60, 2800, SH_GATE_0, IS_GATE);

    b.mapwait(1000);
    b.mapwait(3000);

    b.map_sbtype10(8 * 2, 0, 0, 0);

    b.mapobj(
        0,
        -4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        SPACEBAR_BASE_DIST,
        SH_COLONY3R,
        IS_NOCOLL,
    );
    b.mapobj(
        2000,
        4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        SPACEBAR_BASE_DIST,
        SH_COLONY3L,
        IS_NOCOLL,
    );

    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    b.pathcspecial(3000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    b.pathcspecial(200, 0, -450, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathcspecial(200, 0, -200, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathcspecial(200, 0, 50, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathspecial(15000, 0, 300, 4000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);

    b.map_sbtype7(3 * 2, -3, 1, 0);
    b.map_sbtype7(3 * 2, 3, -1, 0);
    b.map_sbtype14(5 * 2, 0, 0, 0);
    b.map_sbtype10(2 * 2, 4, 0, 0);
    b.map_sbtype10(4 * 2, -4, 0, 0);
    b.map_sbtype6(4 * 2, 0, 0, 0);
    b.map_sbtype7(4 * 2, -3, 0, 0);
    b.map_sbtype7(4 * 2, 3, 0, 0);
    b.map_sbtype7(4 * 2, 0, 0, 0);
    b.map_sbtype10(4 * 2, 0, 0, 0);
    b.map_sbtype5(1 * 2, -2, 0, 0);
    b.map_sbtype1(3 * 2, 0, 0, 0);

    {
        let speed: i32 = 30;

        b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);
        b.map_sbtype17(6, 0, 10, 0, -speed, 0);
        b.map_sbtype16(6, -10, -1, 0, speed, 0);
        b.map_sbtype17(6, -1, -10, 0, speed, 0);
        b.map_sbtype16(6, 10, 1, 0, -speed, 0);

        b.map_sbtype17(0, 5, -10, 0, speed, 0);
        b.map_sbtype17(0, -5, -10, 0, speed, 0);
        b.map_sbtype17(3, 0, 10, 0, -speed, 0);
        b.map_sbtype16(0, -20, -2, 0, speed, 0);
        b.map_sbtype16(0, -10, -1, 0, speed, 0);
        b.map_sbtype16(3, 0, 0, 0, speed, 0);
        b.map_sbtype17(0, 4, 10, 0, -speed, 0);
        b.map_sbtype17(0, -6, 10, 0, -speed, 0);
        b.map_sbtype17(3, -1, -10, 0, speed, 0);
        b.map_sbtype16(0, 20, 0, 0, -speed, 0);
        b.map_sbtype16(0, 10, 1, 0, -speed, 0);
        b.map_sbtype16(3, 0, 2, 0, -speed, 0);

        b.map_sbtype17(0, 6, -10, 0, speed, 0);
        b.map_sbtype17(0, -4, -10, 0, speed, 0);
        b.map_sbtype17(3, 1, 10, 0, -speed, 0);
        b.map_sbtype16(0, -20, -1, 0, speed, 0);
        b.map_sbtype16(0, -10, 0, 0, speed, 0);
        b.map_sbtype16(3, 0, 0, 1, speed, 0);
        b.map_sbtype17(0, 3, 10, 0, -speed, 0);
        b.map_sbtype17(0, -7, 10, 0, -speed, 0);
        b.map_sbtype17(3, -2, -10, 0, speed, 0);
        b.map_sbtype16(0, 20, 0, 0, -speed, 0);
        b.map_sbtype16(0, 10, -1, 0, -speed, 0);
        b.map_sbtype16(3, 0, -2, 0, -speed, 0);

        b.map_sbtype17(0, 7, 10, 0, speed, 0);
        b.map_sbtype17(0, -3, 10, 0, speed, 0);
        b.map_sbtype17(3, 2, 10, 0, -speed, 0);
        b.map_sbtype16(0, -20, -1, 0, speed, 0);
        b.map_sbtype16(0, -10, 0, 0, speed, 0);
        b.map_sbtype16(3, 0, 1, 0, speed, 0);
        b.map_sbtype17(0, 5, 10, 0, -speed, 0);
        b.map_sbtype17(0, -5, 10, 0, -speed, 0);
        b.map_sbtype17(3, 0, -10, 0, speed, 0);
        b.map_sbtype16(0, 20, 2, 0, -speed, 0);
        b.map_sbtype16(0, 10, 1, 0, -speed, 0);
        b.map_sbtype16(3, 0, 0, 0, -speed, 0);

        b.map_sbtype17(1, 0, 10, 0, -speed, 0);
        b.map_sbtype16(1, -10, -1, 0, speed, 0);
        b.map_sbtype17(1, -1, -10, 0, speed, 0);
        b.map_sbtype16(1, 10, 1, 0, -speed, 0);
        b.map_sbtype17(1, 1, 10, 0, -speed, 0);
        b.map_sbtype16(1, -10, 0, 0, speed, 0);
        b.map_sbtype17(1, -2, -10, 0, speed, 0);
        b.map_sbtype16(1, 10, -1, 0, -speed, 0);
        b.map_sbtype17(1, 2, 10, 0, -speed, 0);
        b.map_sbtype16(1, -10, 0, 0, speed, 0);
        b.map_sbtype17(1, 0, -10, 0, speed, 0);
        b.map_sbtype16(3000, 10, 1, 0, -speed, 0);
    }

    b.pathobj(
        0,
        1200,
        200,
        600,
        SH_FRIENDSHIP_4,
        PATH_ID_CHASE1_1,
        200,
        10,
    );
    b.pathcspecial(1000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    b.mapobj(4000, 0, 0, 2000, SH_SPACEPILON, STRATEGY_SPACEPILON);

    b.pathspecial(200, 0, -200, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathspecial(200, 0, 200, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathcspecial(200, 200, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathobj(0, -250, -350, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    b.pathcspecial(12000, -200, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);

    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, SPACE_VIEWCY + 1000, 1500, SH_BOSS_1_2, IS_BOSS1);

    b.mapwait(100);
    let level2_2_mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level2_2.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level2_2.bosswait.cont");
    b.mapgoto("level2_2.bosswait.loop");
    b.label("level2_2.bosswait.cont");
    let level2_2_mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let level2_2_mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.mapwait(2000);
    b.maprts();

    submaps::append_cl_earth_submap(&mut b);

    b.resolve();
    let (data, labels) = b.finish();

    // C `register_level2_2_inline_callbacks()` — registration-call order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (
                level2_2_mapwaitboss_trigse_ptr,
                "level1_1_mapwaitboss_trigse",
            ),
            (
                level2_2_mapwaitboss_cantdie_ptr,
                "level1_1_mapwaitboss_cantdie",
            ),
            (
                level2_2_mapwaitboss_cleanup_ptr,
                "level1_1_mapwaitboss_cleanup",
            ),
        ],
    )
}
