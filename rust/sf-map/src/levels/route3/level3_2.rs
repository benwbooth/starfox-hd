//! MAP_ID_3_2 — Asteroid Belt (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_2_wrapper_slice()` +
//! `register_level3_2_inline_callbacks()`.
//! ASM: LEVEL3_2.ASM / MAP3_2.ASM / CL_CHASE.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();
    let mm = crate::mothers::mother_maps();

    // LEVEL3_2.ASM wrapper around MAP3_2.ASM.
    b.mapjsr("level3_2.map3_2");
    b.mapjsr("cl_chase");
    b.emit8(op::END);

    // MAP3_2.ASM:5-33 – Asteroid Belt 3 opening (M formation through
    // the first asteroid/itachi block, stopping before mapmother).
    b.label("level3_2.map3_2");
    b.mapwait(3300);

    // M formation
    b.szaco2_mapobj(0, 2000, 0, 0, 100);
    b.szaco2_mapobj(-500, 2000, -300, 100, 0);
    b.szaco2_mapobj(500, 2000, 300, 100, 100);
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    b.szaco2_mapobj(-1000, 2000, -500, -100, 0);
    b.szaco2_mapobj(1000, 2000, 500, -100, 100);
    b.mapwait(2000);
    b.pathcspecial(2000, -200, 100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(4000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // friend
    b.pathcspecial(0, 0, -90, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathcspecial(1000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);
    b.cspecial(1000, 0, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapnobj(
        500,
        400,
        SPACE_VIEWCY - 100,
        4000,
        SH_ASTEROID1_PROXY,
        STRATEGY_SLOWMETEOR,
    );
    b.pathcspecial(500, 200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.mapnobj(
        1000,
        -400,
        SPACE_VIEWCY + 200,
        4000,
        SH_ASTEROID1_PROXY,
        STRATEGY_SLOWMETEOR,
    );
    b.cspecial(1000, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1000, -400, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapnobj(
        1000,
        200,
        SPACE_VIEWCY - 200,
        4000,
        SH_ASTEROID1_PROXY,
        STRATEGY_SLOWMETEOR,
    );
    b.cspecial(1000, 0, 300, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    b.pathcspecial(500, 250, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(500, -100, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.mapnobj(
        1000,
        -300,
        SPACE_VIEWCY + 200,
        4000,
        SH_ASTEROID1_PROXY,
        STRATEGY_SLOWMETEOR,
    );
    b.pathcspecial(500, 200, 100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:34-45 – mother ship pattern with break meteors and big meteors.
    b.mapmother(1300, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_1);
    b.cspecial(1300, -350, -100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1300, 0, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapobj(1300, -700, -300, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);
    b.cspecial(1300, 450, 50, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1300, 50, -150, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1300, -350, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.pathobj(
        1300,
        970,
        -100,
        7000,
        SH_BIG_METEOR_PROXY,
        PATH_ID_BIRD_METEOR,
        10,
        10,
    );
    b.cspecial(1300, 550, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1300, -250, -120, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1300, 450, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:47-52 – friend chase pair with itachi formations.
    b.pathcspecial(2000, 50, -70, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    b.pathcspecial(3000, -50, -140, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
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
    b.pathcspecial(1500, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    b.pathcspecial(500, -100, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:55-60 – asteroid/itachi block then mother_2.
    b.mapnobj(
        1000,
        -300,
        SPACE_VIEWCY + 200,
        4000,
        SH_ASTEROID1_PROXY,
        STRATEGY_SLOWMETEOR,
    );
    b.pathcspecial(1000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.cspecial(1000, -400, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapmother(2000, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_2);
    b.pathcspecial(1000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:61-70 – skillfly block with bonus item.
    b.skillfly_init();
    b.skillfly_set(0, -50, 4500, 120);
    b.cspecial(0, 180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(0, -180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(400, 0, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapmother(4000, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_1);
    b.mapremove(SH_MOTHER1);
    let skillfly_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, SPACE_VIEWCY, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_2.map3_2.skillfly_bonus_0_skip");

    // MAP3_2.ASM:71-73 – winglazerman and cameleons.
    b.cspecial(2000, -400, SPACE_VIEWCY, 3000, SH_W_L, IS_WINGLAZERMAN);
    b.special(0, -200, SPACE_VIEWCY + 100, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(1500, 200, SPACE_VIEWCY - 100, 800, SH_CAMELEON, IS_CAMELEON);

    // MAP3_2.ASM:74-79 – meteo & launcher mother with itachi formations.
    b.mapmother(4000, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.map_shou0);
    b.pathcspecial(2000, 0, -130, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(2000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(2000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:80-86 – gate with big meteors and break meteor.
    b.mapobj(500, 400, -200, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);
    b.mapobj(3000, -400, 200, 7000, SH_BIG_METEOR_PROXY, IS_BIG_METEOR);

    b.mapobj(0, 0, 0, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(1000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);
    b.cspecial(3500, -400, 200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    // MAP3_2.ASM:88-100 – windmill (round_1) with rotation/velocity sequence.
    b.special(0, -1500, SPACE_VIEWCY, 4000, SH_ROUND_0, IS_WINDMILL);
    b.setalvarb(AL_ROTY, 160);
    b.setalvarb(AL_VEL, 120);
    b.mapwait(1200);
    b.setalvarb(AL_ROTY, 140);
    b.setalvarb(AL_VEL, 100);
    b.mapwait(1200);
    b.setalvarb(AL_VEL, 0);
    b.setalvarb(AL_ROTY, 127);
    b.mapwait(1500);
    b.setalvarb(AL_VEL, 120);
    b.setalvarw(AL_SWORD1, -2);

    // MAP3_2.ASM:101-111 – mini_worm (head + 5 body segments).
    b.special(0, -200, SPACE_VIEWCY - 100, 2500, SH_D_HEAD_0, IS_WORMHEAD);
    b.setvarobj(WM_MAPVAR1);
    b.mapwait(150);
    for _ in 0..5 {
        b.cspecial(0, -200, SPACE_VIEWCY - 100, 2500, SH_D_BODY_0, IS_WORM);
        b.setalvarptrw(AL_SWORD1, WM_MAPVAR1);
        b.setvarobj(WM_MAPVAR1);
        b.mapwait(150);
    }

    // MAP3_2.ASM:113-114 – spacepilon and itachi_b formation.
    b.mapobj(2000, 0, 100, 2000, SH_SPACEPILON, STRATEGY_SPACEPILON);
    b.pathcspecial(2000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);

    // MAP3_2.ASM:115-125 – mini_worm #2 (head + 5 body segments).
    b.special(0, 200, SPACE_VIEWCY + 100, 2300, SH_D_HEAD_0, IS_WORMHEAD);
    b.setvarobj(WM_MAPVAR1);
    b.mapwait(150);
    for _ in 0..5 {
        b.cspecial(0, 200, SPACE_VIEWCY + 100, 2300, SH_D_BODY_0, IS_WORM);
        b.setalvarptrw(AL_SWORD1, WM_MAPVAR1);
        b.setvarobj(WM_MAPVAR1);
        b.mapwait(150);
    }

    // MAP3_2.ASM:126-127 – set bar shape solid, itachi_a formation.
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.pathcspecial(2000, 200, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);

    // MAP3_2.ASM:129-131 – friend chase3 pair.
    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    b.pathcspecial(1000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // MAP3_2.ASM:133-134 – colony pair.
    b.mapobj(
        0,
        -4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        5000,
        SH_COLONY3R,
        IS_NOCOLL,
    );
    b.mapobj(
        1600,
        4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        5000,
        SH_COLONY3L,
        IS_NOCOLL,
    );

    // MAP3_2.ASM:136-137 – colony pair.
    b.mapobj(
        0,
        -4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        5000,
        SH_COLONY3R,
        IS_NOCOLL,
    );
    b.mapobj(
        2000,
        4 * SPACEBAR_UNIT_LEN,
        SPACE_VIEWCY,
        5000,
        SH_COLONY3L,
        IS_NOCOLL,
    );

    // MAP3_2.ASM:139-141 – up1man + itachi_b formation.
    b.mapobj(0, 0, 0, 5000, SH_NULLSHAPE, IS_UP1MAN);
    b.setalvarb(AL_SBYTE1, 1);
    b.pathcspecial(2000, 200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);

    // MAP3_2.ASM:143-144 – itachi_a + spacepilon.
    b.pathcspecial(2000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.mapobj(3000, 0, 200, 2000, SH_SPACEPILON, STRATEGY_SPACEPILON);

    // MAP3_2.ASM:146-148 – meteo_0 triple.
    b.mapobj(2250, 0, 0, 4000, SH_METEO_0, IS_METEO0);
    b.mapobj(2250, 200, -100, 4000, SH_METEO_0, IS_METEO0);
    b.mapobj(2250, -200, -160, 4000, SH_METEO_0, IS_METEO0);

    // MAP3_2.ASM:150 – screw path.
    b.pathcspecial(400, 200, -100, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);

    // MAP3_2.ASM:152 – r_hou_0 special.
    b.special(0, -200, 0, 4000, SH_R_HOU_0, IS_SHOU0A);

    // MAP3_2.ASM:154-155 – item_5 with sbyte1.
    b.mapobj(0, 100, SPACE_VIEWCY - 100, 4500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);

    // MAP3_2.ASM:158-160 – friend chase5 trio.
    b.pathobj(0, 0, -600, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE5_1, 10, 10);
    b.pathcspecial(0, 1500, 100, 1300, SH_ZACO_B, PATH_ID_CHASE5_2, 10, 10);
    b.pathcspecial(3000, 0, -600, 0, SH_ZACO_B, PATH_ID_CHASE5_3, 10, 10);

    // MAP3_2.ASM:161-164 – break meteors + mother_1.
    b.cspecial(1000, 0, 300, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1000, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapmother(400, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_1);
    b.mapremove(SH_MOTHER1);
    b.mapwait(2000);

    // MAP3_2.ASM:167-168 – mother_5.
    b.mapmother(4000, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_5);
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:170-171 – hider (map_meteo0 mother).
    b.mapmother(
        5000,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER1,
        mm.map_meteo0,
    );
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:173-180 – mother_5 with itachi formations.
    b.mapmother(1500, 0, 0, 4000, SH_MOTHER1, STRATEGY_MOTHER1, mm.mother_5);
    b.pathcspecial(1000, 200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    b.pathcspecial(1000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(1000, -200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    b.pathcspecial(1000, -200, -200, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(1000, 0, -100, 4000, SH_ZACO_A, PATH_ID_ITACHI_A, 2, 4);
    b.pathcspecial(1000, 200, 0, 4000, SH_ZACO_A, PATH_ID_ITACHI_B, 2, 4);
    b.mapremove(SH_MOTHER1);

    // MAP3_2.ASM:182-183 – supply bird + amebmsg2.
    b.pathobj(6000, -380, -150, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    b.pathobj(0, 3000, 3000, 3000, SH_NULLSHAPE, PATH_ID_AMEBMSG2, 10, 10);

    // MAP3_2.ASM:186-194 – boss section (propeller boss).
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    // incmap webmonst: mapobj 0000,000,000,1200,boss_0_1,webmonster_istrat
    b.mapobj(0, 0, 0, 1200, SH_BOSS_0_1, IS_WEBMONSTER);

    // mapwaitboss
    b.mapwait(100);
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level3_2.map3_2.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_2.map3_2.bosswait.cont");
    b.mapgoto("level3_2.map3_2.bosswait.loop");
    b.label("level3_2.map3_2.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss32
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    // MAP3_2.ASM:196 – mapwait 1800
    b.mapwait(1800);

    // MAP3_2.ASM:198 – maprts
    b.maprts();

    // CL_CHASE.ASM – clear demo (chase type) appended as subroutine.
    append_cl_chase_submap(&mut b);

    b.resolve();

    // C zeroes the skip ptr when missing; the label is emitted above.
    assert!(
        b.lookup_label("level3_2.map3_2.skillfly_bonus_0_skip")
            .is_some(),
        "level3_2 skillfly bonus skip label missing"
    );

    let (data, labels) = b.finish();
    // C `register_level3_2_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (skillfly_bonus_guard_ptr, "level3_2_skillfly_bonus_guard"),
            (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
