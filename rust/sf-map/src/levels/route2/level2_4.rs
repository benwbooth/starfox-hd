//! MAP_ID_2_4 — Sector Y (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_4_slice()` +
//! `register_level2_4_inline_callbacks()`.
//! ASM sources: LEVEL2_4.ASM wrapper, MAP2_4.ASM body, CL_TURN.ASM.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::MapBuilder;

/// C `build_level2_4_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapjsr("level2_4.map2_4");
    b.mapjsr("cl_turn");
    b.mapend(1);

    // MAP2_4.ASM — Sector Y subroutine.
    b.label("level2_4.map2_4");

    b.mapwait(600);

    b.pathobj(0, 180, -300, -200, SH_WHALE, PATH_ID_E_WHALE, 10, 10);

    b.map_sfish(2800, 0, -100, 1000, 10);

    b.pathobj(1000, 0, -150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(1000, 150, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(1000, -150, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);

    b.pathobj(300, 1500, 900, 3200, SH_IKA, PATH_ID_IKA_2, 10, 10);
    b.pathobj(5000, 1800, 1100, 2800, SH_IKA, PATH_ID_E_IKA, 10, 10);

    b.pathcspecial(200, 100, -300, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);
    b.pathcspecial(200, 300, -600, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);
    b.pathcspecial(6000, 500, -900, 0, SH_ZACO_7, PATH_ID_EGU1, 4, 10);

    b.pathobj(1000, -1500, 900, 2800, SH_IKA, PATH_ID_E_IKA, 10, 10);
    b.pathobj(1000, 1500, 900, 3200, SH_IKA, PATH_ID_IKA_2, 10, 10);
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathcspecial(4000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    b.pathcspecial(500, 300, -1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);
    b.pathcspecial(12000, -300, -1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);

    // .amoebas1 loop: 3 iterations of mapmother + maprem
    b.label("level2_4.amoebas1");
    b.mapmother(
        200,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER2,
        crate::mothers::mother_maps().map_amoebas,
    );
    b.mapremove(SH_MOTHER1);

    b.mapwait(1000);
    b.maploop("level2_4.amoebas1", 3);
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_AMEBMSG, 10, 10);

    b.mapmother(
        200,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER2,
        crate::mothers::mother_maps().map_amoebas,
    );
    b.mapremove(SH_MOTHER1);

    b.mapobj(0, 100, -100, 4500, SH_NULLSHAPE, IS_UP1MAN);
    b.setalvarw(AL_SWORD2, SH_ITEM_0 as i32);
    b.mapwait(1000);

    b.mapmother(
        200,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER2,
        crate::mothers::mother_maps().map_amoebas,
    );
    b.mapremove(SH_MOTHER1);

    b.mapwait(1000);

    b.mapmother(
        8000,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER2,
        crate::mothers::mother_maps().map_amoebas,
    );
    b.pathcspecial(300, 300, -300, 0, SH_ZACO_7, PATH_ID_EGU1_IFRO, 4, 10);
    b.pathcspecial(300, 500, -600, 0, SH_ZACO_7, PATH_ID_EGU1_IRAB, 4, 10);
    b.pathcspecial(4000, 700, -900, 0, SH_ZACO_7, PATH_ID_EGU1_IFAL, 4, 10);
    b.mapremove(SH_MOTHER1);

    b.mapwait(5000);

    b.cspecial(4000, -700, -300, 3000, SH_W_L, IS_WINGLAZERMAN);
    b.pathobj(0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_BRAYMSG, 10, 10);
    b.pathobj(6700, 0, -250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);
    b.pathobj(0, 0, -600, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE5_1, 10, 10);
    b.pathcspecial(0, 1500, 100, 1300, SH_ZACO_B, PATH_ID_CHASE5_2, 10, 10);
    b.pathcspecial(5000, 0, -600, 0, SH_ZACO_B, PATH_ID_CHASE5_3, 10, 10);

    b.pathobj(5000, 0, 250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);
    b.pathspecial(500, 0, -1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);
    b.pathcspecial(500, -300, 1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);
    b.pathcspecial(8000, 300, 1400, 2000, SH_ZACO_B, PATH_ID_EGU3, 10, 10);

    b.pathobj(0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 10, 10);
    b.pathcspecial(3000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);

    b.mapobj(0, 0, 0, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    b.mapwait(1000);

    // bzaco_8 patret trio + sfish school
    b.pathcspecial(200, 0, 200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    b.pathcspecial(200, 800, -200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    b.pathcspecial(200, -800, -200, -100, SH_BZACO_8, PATH_ID_PATRET, 10, 10);
    b.map_sfish(800, 0, -100, 1000, 8);

    b.pathobj(5000, -1500, 1100, 2800, SH_IKA, PATH_ID_IKA_2, 10, 10);

    b.pathobj(0, -100, -250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);

    b.pathobj(500, -150, -120, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(2000, -200, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(500, 50, -150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(500, 50, 150, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);

    // (line 91 commented out in original ASM)
    b.pathobj(3000, -200, 250, 0, SH_RAY_1, PATH_ID_E_RAY_1, 10, 10);

    b.pathobj(500, -150, -120, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathobj(500, 2100, 1000, 3000, SH_IKA, PATH_ID_IKA_2, 10, 10);
    b.pathobj(500, -200, 0, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.map_sfish(0, 0, -100, 1000, 4);
    b.pathobj(5000, 80, -200, 3000, SH_RAY_0, PATH_ID_E_RAY_0, 10, 10);
    b.pathcspecial(0, -600, -600, 0, SH_ZACO_7, PATH_ID_EGU1, 10, 10);
    b.pathspecial(0, 300, 1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);
    b.pathspecial(5000, -300, -1400, 2000, SH_S_ZACO_0, PATH_ID_EGU3, 10, 10);

    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 10, 10);
    b.pathcspecial(13000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    b.pathobj(0, 0, 0, 3000, SH_NULLSHAPE, PATH_ID_REM_WHALE, 10, 10);

    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_HANDMSG, 10, 10);

    // fadeoutbgm + setbgm 5
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);

    // mapjsr armsmap (inline: flingboss + maprts)
    b.mapjsr("level2_4.armsmap");

    // mapwaitboss
    b.mapwait(100);
    let level2_4_mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level2_4.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level2_4.bosswait.cont");
    b.mapgoto("level2_4.bosswait.loop");
    b.label("level2_4.bosswait.cont");
    let level2_4_mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let level2_4_mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss24
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    b.mapwait(2000);

    b.maprts();

    // armsmap subroutine: flingboss at (0, -80, 2000)
    b.label("level2_4.armsmap");
    b.mapobj(0, 0, -80, 2000, SH_FLINGBOSS, IS_FLINGBOSS);
    b.maprts();

    // CL_TURN.ASM — clear demo (turn type) appended as subroutine.
    submaps::append_cl_turn_submap(&mut b);

    b.resolve();
    let (data, labels) = b.finish();

    // C `register_level2_4_inline_callbacks()` — registration-call order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (
                level2_4_mapwaitboss_trigse_ptr,
                "level1_1_mapwaitboss_trigse",
            ),
            (
                level2_4_mapwaitboss_cantdie_ptr,
                "level1_1_mapwaitboss_cantdie",
            ),
            (
                level2_4_mapwaitboss_cleanup_ptr,
                "level1_1_mapwaitboss_cleanup",
            ),
        ],
    )
}
