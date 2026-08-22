//! MAP_ID_3_7 — Venom 3 Surface (Level 3, Route 3 final).
//!
//! C oracle: `src/map/levels.c` `build_level3_7_slice()` +
//! `register_level3_7_inline_callbacks()`.
//! ASM: LEVEL3_7.ASM / MAP3_7A/B/C.ASM / FINALMAP.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // (MAP_ID_3_7)
    // ============================================================

    // CLEN = SXspacebarlen/2 = 250/2 = 125 = SPACEBAR_UNIT_LEN
    // C: `#define MAP37_CLEN SPACEBAR_UNIT_LEN` (common::MAP37_CLEN).

    // LEVEL3_7.ASM: initlevel 3_7a,0
    // mapjsr map3_7a
    b.mapjsr("level3_7.map3_7a");
    // mapgoto level1_end — jump to finalmap content
    b.mapgoto("level3_7.final.tunnel");

    // Dead code in original ASM (after mapgoto):
    // setbg 3_7b / initbg / mapjsr map3_7b / setbg 3_7c / initbg / mapjsr map3_7c
    // mapwait 10000 / mapend
    // We still emit the subroutines so labels resolve.

    // ---- incmap finalmap (level1_end target) ----
    let (mapwaitboss_cantdie_ptr, mapwaitboss_cleanup_ptr) =
        append_finalmap_content(&mut b, "level3_7.final", 3);

    // ---- MAP3_7A.ASM subroutine — Venom 3 Surface Part A (383 lines) ----
    b.label("level3_7.map3_7a");

    // Lines 2-4: restart3_7 — mapwait 2000, mapgoto cont3_7
    b.label("level3_7.restart3_7");
    b.mapwait(2000);
    b.mapgoto("level3_7.cont3_7");

    // Line 8: map3_7a label
    // Line 10: incmap planet — inline planet scenery objects
    // PLANET.ASM: mapnozremove + scenery objects
    b.preserve_behind_view_objects();
    b.mapobj(0, 544, -1000, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(0, 544, -500, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(0, 544, -10, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(0, -500, 0, 1024, SH_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 256 - 64);
    b.mapobj(0, 500, 0, 1024, SH_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 64);
    b.mapobj(0, 800, 0, -300, SH_BU_0, IS_HARD);
    b.mapobj(0, -800, 0, -300, SH_BU_0, IS_HARD);
    b.mapobj(0, -300, 0, -800, SH_BU_2, IS_HARD);
    b.mapobj(0, 300, 0, -800, SH_BU_2, IS_HARD);
    b.mapobj(0, -544, -1000, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(0, -544, -500, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(512, -544, -10, -200, SH_R_BU_4, IS_HARD);
    b.mapobj(0, -200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(512, 200, -300, 600, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(512, -200, -300, 800, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1024, -180, -250, 1000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 150, -200, 1000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(768, -150, -200, 1000, SH_R_BU_7, IS_HARD180YR);

    // Lines 11-18: r_bu_7 pairs
    b.mapobj(0, 512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1024, -512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1024, -512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1024, -512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 512, -293, 2000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1024, -512, -293, 2000, SH_R_BU_7, IS_HARD180YR);

    // Line 21: setrestart restart3_7
    b.mapcodejsl_builtin(MAP_CB_SETRESTART_L);

    // Line 22: cont3_7
    b.label("level3_7.cont3_7");

    // Lines 24-25: bu_0 pair
    b.mapobj(0, 768, 0, 3000, SH_BU_0, IS_HARD180YR);
    b.mapobj(512, -768, 0, 3000, SH_BU_0, IS_HARD180YR);

    // Line 26: map_setbarshape solid
    b.map_setbarshape(BarShapeMode::Solid, false);

    // Lines 28-47: .ougi spacebar section
    // map_spacebarIC 0,0,15,0,-2 / map_spacebarX -2,0,-15,0
    b.map_spacebaric(0, 0, 15, 0, -2);
    b.map_spacebarx(-2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, -(MAP37_CLEN * 150 / 100));
    b.map_spacebarwait(2);

    b.map_spacebaric(0, 0, 15, 0, 2);
    b.map_spacebarx(2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, MAP37_CLEN * 150 / 100);
    b.map_spacebarwait(2);

    b.map_spacebaric(0, 0, 15, 0, -2);
    b.map_spacebarx(-2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, -(MAP37_CLEN * 150 / 100));
    b.map_spacebarwait(2);

    b.map_spacebaric(0, 0, 15, 0, 2);
    b.map_spacebarx(2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, MAP37_CLEN * 150 / 100);
    b.map_spacebarwait(3);

    // Lines 48-49: bu_0 pair
    b.mapobj(0, 768, 0, 2970, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, -768, 0, 2970, SH_BU_0, IS_HARD180YR);

    // Lines 50-62: .bars loop (6 iterations)
    b.label("level3_7.bars");
    b.map_spacebaric(0, 0, 15, 0, -2);
    b.map_spacebarx(-2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, -(MAP37_CLEN * 150 / 100));
    b.map_spacebarwait(2);

    b.map_spacebaric(0, 0, 15, 0, 2);
    b.map_spacebarx(2, 0, -15, 0);
    b.setalvarw(AL_WORLDX, MAP37_CLEN * 150 / 100);
    b.map_spacebarwait(2);
    b.maploop("level3_7.bars", 6);

    // Lines 61-63: final bar + mapwait 1000
    b.map_spacebaric(0, 0, 15, 0, 0);
    b.map_spacebarx(0, -2, -15, 2);
    b.mapwait(1000);

    // Lines 67-72: dossun pathobjs + item_6
    b.pathobj(0, -450, -150, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 450, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.mapobj(0, 0, -50, 4000, SH_ITEM_6, IS_ITEM6);
    b.pathobj(0, -200, -350, 3000, SH_R_BU_1, PATH_ID_E_DOSUN, 10, 8);
    b.pathobj(0, 200, -300, 3000, SH_R_BU_1, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(1024, 0, -250, 3000, SH_R_BU_1, PATH_ID_E_DOSUN, 10, 8);

    // Lines 73-86: .boards loop (3 iterations)
    b.label("level3_7.boards");
    b.pathobj(0, -450, -350, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(0, -200, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(1024, 200, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 450, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(1024, 100, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(0, -300, -150, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.pathobj(0, 300, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(1024, 0, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 450, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(1024, -200, -200, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, -450, -350, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(1024, -100, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);
    b.maploop("level3_7.boards", 3);
    b.pathobj(16384, 0, -200, 3000, SH_R_BU_2, PATH_ID_ITADOSUN, 10, 8);

    // Lines 89-98: flypillar + pillar3 objects
    b.mapobj(0, 2048, 0, 4000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(0, -1000, 0, 5000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(0, 1200, 0, 6000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(0, -900, 0, 6000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(0, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(0, 512, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(1024, -512, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(0, 1280, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(512, -1280, 0, 3000, SH_RPILLAR3_PROXY, IS_FLYPILLARS);
    b.mapobj(6144, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);

    // Line 100: mapmother flypillars
    b.mapmother(
        32768,
        0,
        0,
        4000,
        SH_MOTHER1,
        STRATEGY_MOTHER1,
        crate::mothers::mother_maps().map_flypillars,
    );

    // Lines 101-107: pillar3 objects
    b.mapobj(2048, 0, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, -512, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, 256, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, 768, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, -832, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, 80, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2048, -256, 0, 3000, SH_RPILLAR3_PROXY, IS_PILLAR3);

    // Line 108: maprem mother1
    b.mapremove(SH_MOTHER1);

    // Lines 110-111: friend chase6
    b.pathobj(0, -750, -480, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathobj(1536, -720, -480, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Line 112: pathspecial s_tank_0 e_tank
    b.pathspecial(1536, 0, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 4, 2);

    // Line 113: mapobj bu_0
    b.mapobj(1000, -600, 0, 4000, SH_BU_0, IS_HARD180YR);

    // Lines 115-116: pathcspecial tank_1 e_tank
    b.pathcspecial(2000, 768, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);
    b.pathcspecial(2000, -250, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);

    // Line 118: pathspecial patrol
    b.pathspecial(1280, -1100, -600, 2000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);

    // Lines 119-140: R_BU_6 arch with roty settings
    b.mapobj(256, -500, -50, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 127);
    b.mapobj(256, -400, -150, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 110);
    b.mapobj(256, -300, -250, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 97);
    b.mapobj(256, -200, -300, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 86);
    b.mapobj(256, -100, -350, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 75);
    b.mapobj(256, 0, -400, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 64);
    b.mapobj(256, 100, -350, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 180);
    b.mapobj(256, 200, -300, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 168);
    b.mapobj(256, 300, -250, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 155);
    b.mapobj(256, 400, -150, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 140);
    b.mapobj(256, 500, -50, 1800, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, 127);

    // Line 141: pathcspecial tank_1 e_tank
    b.pathcspecial(2500, 0, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);

    // Lines 142-143: bu_0 pair
    b.mapobj(2000, -400, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(2000, 400, 0, 5000, SH_BU_0, IS_HARD180YR);

    // Lines 146-176: movingwalls section
    b.mapobj(0, 0, 0, 4000, SH_WALL_1_PROXY, IS_WALLL);

    // map_setbarshape wire for the SBtype16 sections
    b.map_setbarshape(BarShapeMode::Wire, false);
    {
        let speed: i32 = 30;
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
    }
    b.mapwait(3000);

    b.mapobj(1500, -200, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    b.mapobj(2000, 400, 0, 4000, SH_WALL_1_PROXY, IS_WALLL);
    b.mapobj(400, 0, 0, 4000, SH_RPILLAR3_PROXY, IS_PILLAR3);
    b.mapobj(2000, 100, 0, 4000, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);

    {
        let speed: i32 = 30;
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
    }
    b.mapobj(0, 0, -50, 4000, SH_ITEM_7, IS_ITEM7);

    b.mapobj(1500, -350, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    b.mapobj(500, 350, 0, 4200, SH_WALL_1_PROXY, IS_WALLL);
    {
        let speed: i32 = 30;
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
    }
    b.mapwait(1000);

    b.mapobj(1500, 0, 0, 4000, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    b.mapobj(1500, 400, 0, 4200, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    b.mapobj(800, -400, 0, 4200, SH_WALL_1_PROXY, IS_WALLLEFTRIGHT);
    {
        let speed: i32 = 30;
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(5, -10, -3, 0, speed, 0);
        b.map_sbtype16(0, 10, -4, 0, -speed, 0);
        b.map_sbtype16(4, -10, -3, 0, speed, 0);
    }
    b.mapobj(1500, -450, 0, 4000, SH_WALL_1_PROXY, IS_WALLR);
    b.mapobj(1500, 450, 0, 4200, SH_WALL_1_PROXY, IS_WALLL);

    // Lines 180-181: gate + e_gate
    b.mapobj(2000, 0, -200, 4000, SH_GATE_0, IS_GATE);
    b.pathobj(1000, 0, -200, 4000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 184-186: friend + pathspecial tank + chase7
    b.pathspecial(0, 768, 0, 3000, SH_S_TANK_0, PATH_ID_E_TANK, 4, 2);
    b.pathobj(0, 0, -370, -150, SH_FRIENDSHIP_4, PATH_ID_CHASE7_1, 10, 10);
    b.pathobj(1000, 0, -370, -150, SH_ZACO_A, PATH_ID_CHASE7_2, 10, 10);

    // Line 187: special s_wark_0 spacebarwalker
    b.special(0, 0, -270, 3000, SH_S_WARK_0, IS_SPACEBARWALKER);

    // Lines 188-189: r_bu_7 pair
    b.mapobj(0, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(256, 100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 190-191: skillfly_init + skillfly_set
    b.skillfly_init();
    b.skillfly_set(0, -150, 3000, 150);

    // Lines 192-194: more r_bu objects
    b.mapobj(256, 0, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(0, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1000, 100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 196-201
    b.mapobj(0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(256, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.skillfly_set(-200, -150, 3000, 150);
    b.mapobj(256, -200, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1500, -100, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 203-211
    b.mapobj(0, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, -200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(256, 200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, -100, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    b.skillfly_set(256, -150, 3000, 150);
    b.mapobj(256, 100, -260, 3000, SH_R_BU_2, IS_HARD180YR);
    b.mapobj(0, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, -200, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(1000, 200, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 213-224: bwarker + r_bu_7/r_bu_6 sequence + roty settings
    b.mapobj(0, 350, -300, 3000, SH_BWARKER_3, IS_SPACEBARWALKER);
    b.mapobj(150, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(150, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(0, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, 200, -280, 3000, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, DEG90);
    b.mapobj(150, 50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 50, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(1000, 50, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Line 224: skillfly_bonus item_5
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 200, -100, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_7.skillfly_bonus_0_skip");

    // Lines 227-238: more r_bu_7/r_bu_6 with roty
    b.mapobj(150, -50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, -50, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(0, -50, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(0, -200, -280, 3000, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, DEG90);
    b.mapobj(0, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.pathcspecial(0, 300, 0, 3000, SH_TANK_1, PATH_ID_E_TANK, 4, 2);
    b.mapobj(0, -200, -280, 3000, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, DEG90);
    b.mapobj(150, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, -350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(1000, -350, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 240-253
    b.mapobj(150, 450, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 450, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(0, 450, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.skillfly_init();
    b.skillfly_set(768, -150, 3000, 150);
    b.mapobj(0, 300, -280, 3000, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, DEG90);
    b.mapobj(150, 150, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(150, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(150, 350, -280, 3000, SH_R_BU_6, IS_HARD180YR);
    b.mapobj(1500, 350, -125, 3000, SH_R_BU_7, IS_HARD180YR);

    // Lines 255-264
    b.mapobj(0, 500, 0, 3000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1500, -200, 0, 3000, SH_BU_2, IS_HARD180YR);
    b.mapobj(0, -300, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.skillfly_set(-150, -150, 3000, 150);
    b.mapobj(0, -150, -280, 3000, SH_R_BU_6, IS_HARD);
    b.setalvarb(AL_ROTY, DEG90);
    b.mapobj(1500, 0, -125, 3000, SH_R_BU_7, IS_HARD180YR);
    b.mapobj(500, -300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(1200, -600, 0, 4000, SH_BU_0, IS_HARD180YR);

    // Line 264: skillfly_bonus item_5
    let skillfly_bonus1_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, -180, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_7.skillfly_bonus_1_skip");

    // Lines 267-275: patrol + cspecial houdai/walker
    b.pathspecial(500, 1100, -600, 3000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    b.cspecial(1000, -600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(1000, 600, 0, 1000, SH_WALKER_0, IS_WALKING);
    b.cspecial(1000, 600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(1000, -600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    b.cspecial(1000, -300, 0, 1000, SH_WALKER_0, IS_WALKING);

    // Lines 274-275: friend chase6
    b.pathobj(0, -750, -480, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE6_1, 10, 10);
    b.pathobj(800, -720, -480, 0, SH_ZACO_A, PATH_ID_CHASE6_2, 10, 10);

    // Lines 277-306: .block loop (2 iterations) — mapblocksnd + r_bu_1 blocks
    b.label("level3_7.block");
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, -250, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, -150, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, -250, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, -150, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 250, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 150, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 250, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(2048, 150, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, 100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(2048, -100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.maploop("level3_7.block", 2);

    // Lines 308-327: post-block V pattern
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -500, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 500, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -400, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 400, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -300, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 300, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -200, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 200, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -100, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(2048, 100, -50, 1000, SH_R_BU_1, IS_HARD180YR);

    // Lines 328-346: .block2 loop (2 iterations)
    b.label("level3_7.block2");
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -300, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 300, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -200, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 200, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(0, -100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapobj(256, 100, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -350, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -250, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -150, 1000, SH_R_BU_1, IS_HARD180YR);
    b.mapcodejsl_builtin(MAP_CB_BLOCKSND_L);
    b.mapobj(256, 0, -50, 1000, SH_R_BU_1, IS_HARD180YR);
    b.maploop("level3_7.block2", 2);

    // Lines 348-354: post-block enemies + buildings
    b.cspecial(1000, 600, 0, 4000, SH_HOU_5, IS_HOUDAI5F);
    b.mapobj(1000, -1200, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1500, 1200, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, 300, 0, 4000, SH_BU_8, IS_HARD180YR);
    b.mapobj(1000, 1000, 0, 4000, SH_BU_2, IS_HARD180YR);
    b.mapobj(1000, -1400, 0, 4000, SH_BU_2, IS_HARD180YR);

    // Lines 355-364: e_kururi formation (10 pathobjs)
    b.pathobj(0, -465, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, -465, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, -220, -220, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, -220, -520, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 0, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 0, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 220, -220, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 220, -520, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(0, 465, -420, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);
    b.pathobj(3500, 465, -120, 3000, SH_R_BU_2, PATH_ID_E_KURURI, 10, 8);

    // Lines 365-367: patrol specials
    b.pathspecial(0, 1100, -600, 2000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    b.pathcspecial(0, 1300, -800, 2500, SH_ZACO_A, PATH_ID_PATROL, 10, 10);
    b.pathcspecial(500, 1500, -1000, 3000, SH_ZACO_A, PATH_ID_PATROL, 10, 10);

    // Lines 369-374: bu_0 + walker pairs
    b.mapobj(0, 400, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.pathcspecial(0, 350, 0, 5250, SH_WALKER_0, PATH_ID_E_WALK_1, 4, 4);
    b.mapobj(2000, -400, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.mapobj(0, 400, 0, 5000, SH_BU_0, IS_HARD180YR);
    b.pathcspecial(0, -350, 0, 5250, SH_WALKER_0, PATH_ID_E_WALK_1, 4, 4);
    b.mapobj(2000, -400, 0, 5000, SH_BU_0, IS_HARD180YR);

    // Lines 377-381: pre-boss transition
    b.mapwait(2000);
    b.setbgm(BGM_FADEOUT);
    b.setbgm(6);

    // incmap transfor — TRANSFOR.ASM: boss spawn + mapwaitboss
    // boss_f_4 at -100,-500,0
    b.mapobj(0, -100, -500, 0, SH_BOSS_F_4_PROXY, STRAT_ADDR_AIRSHIP);

    // mapwaitboss + markboss boss37
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level3_7.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_7.bosswait.cont");
    b.mapgoto("level3_7.bosswait.loop");
    b.label("level3_7.bosswait.cont");
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.maprts();

    // ---- MAP3_7B.ASM subroutine (stub — empty in original) ----
    b.label("level3_7.map3_7b");
    b.maprts();

    // ---- MAP3_7C.ASM subroutine (stub — empty in original) ----
    b.label("level3_7.map3_7c");
    b.maprts();

    b.resolve();

    // C zeroes these when missing; both labels are emitted above.
    assert!(
        b.lookup_label("level3_7.skillfly_bonus_0_skip").is_some(),
        "level3_7 skillfly bonus 0 skip label missing"
    );
    assert!(
        b.lookup_label("level3_7.skillfly_bonus_1_skip").is_some(),
        "level3_7 skillfly bonus 1 skip label missing"
    );

    let (data, labels) = b.finish();
    // C `register_level3_7_inline_callbacks()` registration-call order
    // (3-7 reuses the 3-6 skillfly guards and the 1-1 mapwaitboss trio).
    finish_level(
        data,
        labels,
        vec![
            (skillfly_bonus0_guard_ptr, "level3_6_skillfly_bonus0_guard"),
            (skillfly_bonus1_guard_ptr, "level3_6_skillfly_bonus1_guard"),
            (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
