//! MAP_ID_3_4 — Sector Z / Space Armada approach (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_4_slice()` +
//! `register_level3_4_inline_callbacks()`.
//! ASM: LEVEL3_4.ASM / MAP3_4B.ASM / MAP3_4C.ASM / WASHMAP / CL_SHIP.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();

    // LEVEL3_4.ASM: mapjsr map3_4b (no map3_4a in the original wrapper).
    b.mapjsr("level3_4.map3_4b");
    // After map3_4b returns: setbg 1_3d, INCMAP washmap, markboss, cl_ship3_4
    b.setbg(BG_3_4D);
    b.initbg();
    // INCMAP washmap — Giant Washing Machine Boss (Sector Z Boss)
    // WASHMAP.ASM: setbgm 6
    b.setbgm(6);
    b.mapwait(300);

    // boss_8_0: main boss shell
    // mapobj 0,0,(-50<<boss8_scale)+nucleusheight,210<<boss8_scale,boss_8_0,boss8_Istrat
    b.mapobj(0, 0, (-50 << BOSS8_SCALE) + NUCLEUSHEIGHT, 210 << BOSS8_SCALE, SH_BOSS_8_0_PROXY, STRAT_ADDR_BOSS8);

    // 4 nucleus launchers at various angles
    b.mapobj(0, 0, (-50 << BOSS8_SCALE) + NUCLEUSHEIGHT, BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    b.setalvarb(AL_SBYTE2, DEG90 + DEG22);

    b.mapobj(0, 0, (-50 << BOSS8_SCALE) + NUCLEUSHEIGHT, BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    b.setalvarb(AL_SBYTE2, DEG135 + DEG22);

    b.mapobj(0, 0, (-50 << BOSS8_SCALE) + NUCLEUSHEIGHT, BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    b.setalvarb(AL_SBYTE2, DEG270 - DEG22);

    b.mapobj(0, 0, (-50 << BOSS8_SCALE) + NUCLEUSHEIGHT, BOSS8_CIRC, SH_HOU_4_PROXY, STRAT_ADDR_NUCLEUSLAUNCHER);
    b.setalvarb(AL_SBYTE2, 0 - DEG22);

    // REPT rotnum: 8 nucleus pillars at rotsize*prot angles
    {
    for prot in 0..ROTNUM_WASH {
    b.mapobj(0, 0, 0 + NUCLEUSHEIGHT, BOSS8_CIRC, SH_BOSS_8_4_PROXY, STRAT_ADDR_NUCLEUSPILLAR);
    b.setalvarb(AL_SBYTE2, ROTSIZE_WASH * prot);
    }
    }

    // maptexitwait -300 (stub: mapwait 300)
    b.mapwait(300);
    // initbg
    b.initbg();

    // .loop: mapif chkstagedone,.cont / mapgoto .loop
    b.label("washmap.loop");
    b.mapif_builtin(MAP_CB_CHKSTAGEDONE, "washmap.cont");
    b.mapgoto("washmap.loop");

    // .cont: clear boss HP + wait
    b.label("washmap.cont");
    b.setvarw(WM_BOSSMAXHP, 0);
    b.mapwait(1000);

    // setbgm $f1 (fadeout)
    b.setbgm(BGM_FADEOUT);

    // mapplayermode EscapeNucleus — approximated as player outview
    b.mapplayeroutview();

    // mapwait 4360 (first arg used)
    b.mapwait(4360);

    // mapcode_jsl clearrealobjmap_l
    b.mapcodejsl_builtin(MAP_CB_CLEARREALOBJMAP_L);
    // mapwait medpspeed
    b.mapwait(MEDPSPEED);

    // markboss boss34
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    // mapjsr cl_ship3_4
    b.mapjsr("cl_ship3_4");
    b.mapend(1);

    // ---- MAP3_4B subroutine (504 lines) ----
    b.label("level3_4.map3_4b");

    // Line 3: mapwait 2000
    b.mapwait(2000);

    // Lines 4-6: szaco2_mapobj trio
    b.szaco2_mapobj(0, 1800, 0, -100, 100);
    b.szaco2_mapobj(400, 1800, 400, 100, 0);
    b.szaco2_mapobj(-400, 1800, -400, 100, 0);

    // Line 7: mapwait 3000
    b.mapwait(3000);

    // Lines 9-10: swinger sharks
    b.pathcspecial(400, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 4, 4);
    b.pathcspecial(1000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 4, 4);

    // Lines 11-14: space houses and zaco
    b.cspecial(1500, 200, 0, 4000, SH_R_HOU_0, IS_SHOU0A);
    b.special(1500, 0, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    b.cspecial(1000, -400, 200, 4000, SH_R_HOU_0, IS_SHOU0A);
    b.pathcspecial(1000, 0, -200, 4000, SH_ZACO_8, PATH_ID_ITACHI_B, 2, 4);

    // Lines 17-18: friend pair (chase2)
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathobj(1000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);

    // Lines 19-20: spacebar setup, wire mode
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype8(8, 2, -1, 0);

    // Line 21: cspecial house
    b.cspecial(1200, -100, -200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 22-27: spacebar patterns
    b.map_sbtype_b(6, -2, 0, 0);
    b.map_sbtype8(4, -4, 2, 0);
    b.map_sbtype_a(4, 2, 3, 0);
    b.map_sbtype_c(2, 0, -3, 0);
    b.pathcspecial(1000, -300, -100, 4000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);
    b.map_sbtype13(2, -5, -1, 0);

    // Lines 28-34: skillfly init + set + spacebar
    b.skillfly_init();
    b.skillfly_set(0, -10, 3000, 100);
    b.setalvarb(AL_SBYTE1, 1);
    b.map_sbtype_b(0, 1, 0, 0);
    b.map_sbtype_b(0, -1, 0, 0);
    b.map_sbtype_c(0, 0, 1, 0);
    b.map_sbtype_c(0, 0, -1, 0);

    // Line 36: shark
    b.pathcspecial(3000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);

    // Line 37: skillfly_bonus (item_5)
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline(); 
    b.mapobj(0, 0, 0, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_4.skillfly_bonus_0_skip");

    // Lines 42-48: big_missile section
    b.mapnobj(500, -200, 0, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    b.mapobj(2000, 0, SPACE_VIEWCY, 4000, SH_BIG_M, IS_MISSPOD);
    b.map_sbtype7(1, 5, 1, 0);
    b.mapnobj(500, 200, 100, 4000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    b.mapobj(2000, 200, SPACE_VIEWCY - 100, 4000, SH_BIG_M, IS_MISSPOD);
    b.mapobj(1000, -100, SPACE_VIEWCY + 200, 4000, SH_BIG_M, IS_MISSPOD);
    b.map_sbtype6(1, 4, -1, 0);

    // Lines 50-51: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(200, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 52-53: spacebar
    b.map_sbtype13(10, -3, 0, 0);
    b.map_sbtype6(4, -4, -1, 0);

    // Lines 55-70: windmill section
    b.special(0, 1800, SPACE_VIEWCY - 100, 4000, SH_ROUND_0, IS_WINDMILL);
    b.setalvarb(AL_ROTY, 64); // deg90
    b.setalvarb(AL_VEL, 120);
    b.mapwait(400);
    b.setalvarb(AL_ROTY, 80);
    b.setalvarb(AL_VEL, 120);
    b.mapwait(400);
    b.setalvarb(AL_ROTY, 100);
    b.setalvarb(AL_VEL, 120);
    b.mapwait(400);
    b.setalvarb(AL_ROTY, 120);
    b.setalvarb(AL_VEL, 100);
    b.mapwait(400);
    b.setalvarb(AL_VEL, 0);
    b.setalvarb(AL_ROTY, DEG180);
    b.mapwait(1500);
    b.setalvarb(AL_VEL, 100);
    b.setalvarw(AL_SWORD1, -2);

    // Lines 73-76
    b.map_sbtype8(-4, 2, 0, 0);
    b.mapwait(500);
    b.map_sbtype_a(4, 1, 0, 0);
    b.mapwait(1000);

    // Line 77: solid bars
    b.map_setbarshape(BarShapeMode::Solid, false);

    // Lines 79-80: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(200, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 82: wire mode
    b.map_setbarshape(BarShapeMode::Wire, false);

    // Line 84: pathobj check
    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);

    // Lines 86-89: rotation_bar (4x SBtype18)
    b.map_sbtype18(4, 0, 0, 0, 0, 0);
    b.map_sbtype18(4, 0, 0, 0, 0, -4);
    b.map_sbtype18(4, 0, 0, 0, 0, 0);
    b.map_sbtype18(4, 0, 0, 0, 0, 4);

    // Lines 91-92: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(200, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 94-103: solid bars then wire bars
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype0(0, 5, 0, 0);
    b.map_sbtype0(0, -5, 0, 0);
    b.map_sbtype0(0, 4, 0, 0);
    b.map_sbtype0(0, -4, 0, 0);
    b.map_sbtype0(0, 3, 0, 0);
    b.map_sbtype0(0, -3, 0, 0);
    b.map_sbtype0(0, 2, 0, 0);
    b.map_sbtype0(1, -2, 0, 0);
    b.map_setbarshape(BarShapeMode::Wire, false);

    // Lines 105-108: rotation_bar (4x SBtype18)
    b.map_sbtype18(4, 0, 0, 0, 0, 0);
    b.map_sbtype18(4, 0, 0, 0, 0, -4);
    b.map_sbtype18(4, 0, 0, 0, 0, 0);
    b.map_sbtype18(4, 0, 0, 0, 0, 4);

    // Lines 109-117: solid bars
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype0(0, 5, 0, 0);
    b.map_sbtype0(0, -5, 0, 0);
    b.map_sbtype0(0, 4, 0, 0);
    b.map_sbtype0(0, -4, 0, 0);
    b.map_sbtype0(0, 3, 0, 0);
    b.map_sbtype0(0, -3, 0, 0);
    b.map_sbtype0(0, 2, 0, 0);
    b.map_sbtype0(1, -2, 0, 0);

    // Lines 118-124: wire rotation bars with init offsets
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype18(4, 0, 0, 0, 2, 0);
    b.map_sbtype_obj(0, 2, 0, 0, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.map_sbtype18(4, 0, 0, 0, 2, -4);
    b.map_sbtype18(4, 0, 0, 0, 2, 0);
    b.map_sbtype18(4, 0, 0, 0, 2, 4);

    // Lines 125-128: solid + colony pair
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(0, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 130-137: solid bars
    b.map_sbtype0(0, 5, 0, 0);
    b.map_sbtype0(0, -5, 0, 0);
    b.map_sbtype0(0, 4, 0, 0);
    b.map_sbtype0(0, -4, 0, 0);
    b.map_sbtype0(0, 3, 0, 0);
    b.map_sbtype0(0, -3, 0, 0);
    b.map_sbtype0(0, 2, 0, 0);
    b.map_sbtype0(1, -2, 0, 0);

    // Lines 138-145: wire bars + special house
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype18(4, 0, 0, 0, 2, 0);
    b.map_sbtype18(4, 0, 0, 0, 2, -4);
    b.map_sbtype18(4, 0, 0, 0, 2, 0);
    b.special(0, 100, -300, 4000, SH_R_HOU_0, IS_SHOU0A);
    b.map_sbtype18(4, 0, 0, 0, 2, 4);
    b.map_sbtype18(4, 0, 0, 0, 2, 0);
    b.map_sbtype10(4, 0, 0, 0);

    // Lines 146-155: solid bars + SBtype12 + gate
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype0(0, 5, 0, 0);
    b.map_sbtype0(0, -5, 0, 0);
    b.map_sbtype0(0, 4, 0, 0);
    b.map_sbtype0(0, -4, 0, 0);
    b.map_sbtype0(0, 3, 0, 0);
    b.map_sbtype0(0, -3, 0, 0);
    b.map_sbtype0(0, 2, 0, 0);
    b.map_sbtype0(8, -2, 0, 0);
    b.map_sbtype12(8, -2, 0, 0);

    // Line 157: gate (SBtypeOBJ with gate3_istrat = raw strat address)
    b.map_sbtype_obj_nobj(8, 1, -1, 1, SH_GATE_0, STRAT_ADDR_GATE3);
    b.pathobj(1000, 3000, 3000, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 160-161: SBtype11 + spacebarwalker
    b.map_sbtype11(1, 0, -1, 1);
    b.special(0, 2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_S_WARK_0, IS_SPACEBARWALKER);
    b.mapwait(2000);

    // Lines 164-165: friend pair (chase3)
    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    b.pathobj(0, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);

    // Lines 167-168: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(0, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 170: itachi_a
    b.pathcspecial(0, 400, -150, 5000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);

    // Lines 171-174: .sbbar1 loop (6 iterations)
    b.label("level3_4.sbbar1");
    b.map_sbtype8(0, -1, 0, 0);
    b.map_sbtype8(4, 1, 0, 0);
    b.maploop("level3_4.sbbar1", 6);

    // Lines 176-177: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(0, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 179: itachi_a
    b.pathcspecial(0, -300, -100, 4000, SH_ZACO_8, PATH_ID_ITACHI_A, 2, 4);

    // Lines 180-183: .sbbar4 loop (2 iterations)
    b.label("level3_4.sbbar4");
    b.map_sbtype8(0, -1, 1, 0);
    b.map_sbtype8(4, 1, 1, 0);
    b.maploop("level3_4.sbbar4", 2);

    // Line 184: Bwarker spacebarwalker
    b.cspecial(0, 1 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 186-189: .sbbar5 loop (2 iterations)
    b.label("level3_4.sbbar5");
    b.map_sbtype8(0, -1, 1, 0);
    b.map_sbtype8(4, 1, 1, 0);
    b.maploop("level3_4.sbbar5", 2);

    // Line 190: cspecial house
    b.cspecial(0, 300, 200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 191-195: wire bars .sbbar6 loop (2 iterations)
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.label("level3_4.sbbar6");
    b.map_sbtype8(0, -1, 0, 0);
    b.map_sbtype8(4, 1, 0, 0);
    b.maploop("level3_4.sbbar6", 2);

    // Lines 197-198: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(0, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Line 200: solid bars
    b.map_setbarshape(BarShapeMode::Solid, false);

    // Lines 202-206: Bwarker + .sbbar7 loop (4 iterations)
    b.cspecial(0, 0, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);
    b.label("level3_4.sbbar7");
    b.map_sbtype8(0, 0, 1, 0);
    b.map_sbtype8(4, 0, -1, 0);
    b.maploop("level3_4.sbbar7", 4);

    // Line 207: Bwarker
    b.cspecial(0, 0, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 209-212: .sbbar9 (no loop — just 2 bars + spacebarwalker)
    b.map_sbtype8(0, 0, 1, 0);
    b.map_sbtype8(4, 0, -1, 0);
    b.special(0, 0, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_S_WARK_0, IS_SPACEBARWALKER);

    // Lines 215-216: colony pair
    b.mapobj(0, -4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3R, IS_NOCOLL);
    b.mapobj(0, 4 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + 0, 5000, SH_COLONY3L, IS_NOCOLL);

    // Lines 218-221: .sbbarb loop (4 iterations)
    b.label("level3_4.sbbarb");
    b.map_sbtype8(0, 0, 1, 0);
    b.map_sbtype8(4, 0, -1, 0);
    b.maploop("level3_4.sbbarb", 4);

    // Lines 223-224: two more bars
    b.map_sbtype8(0, 0, 1, 0);
    b.map_sbtype8(8, 0, -1, 0);

    // Lines 226-229: skillfly init + set + house + setalvar
    b.skillfly_init();
    b.skillfly_set(-280, 0, 3000, 150);
    b.mapobj(0, -280, 0, 3000, SH_R_HOU_0, IS_SHOU0A);
    b.setalvarb(AL_SBYTE1, 1);

    // Lines 231-237: SBtype12, spacebarwait, houses, skillfly_set
    b.map_sbtype12(8, -5, 0, 0);
    b.map_spacebarwait(5);
    b.special(0, -150, -100, 4500, SH_S_HOU_0, IS_SHOU0);
    b.skillfly_set(200, 0, 3500, 150);
    b.mapobj(0, 200, 0, 3500, SH_R_HOU_0, IS_SHOU0A);
    b.setalvarb(AL_SBYTE1, 1);
    b.map_sbtype12(8, 0, 0, 5);

    // Line 239: mapwait 3000
    b.mapwait(3000);

    // Line 240: skillfly_bonus (gate3)
    let skillfly_bonus1_guard_ptr = b.mapcode65816_inline(); 
    b.mapnobj(0, -50, 0, 2000, SH_GATE_0, STRAT_ADDR_GATE3);
    b.label("level3_4.skillfly_bonus_1_skip");

    // Lines 241-245: spacebar patterns
    b.map_sbtype14(4, 0, 0, 0);
    b.map_sbtype_a(2, -2, 1, 5);
    b.map_sbtype8(2, 4, 0, 5);
    b.map_sbtype_c(2, 2, -1, 5);
    b.map_sbtype_b(2, -1, 0, 5);

    // Line 248: cspecial house
    b.cspecial(1000, 200, 200, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 250-253: wire&solid section
    b.map_sbtype1(1, 0, 1, 0);
    b.special(0, -2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_S_WARK_0, IS_SPACEBARWALKER);
    b.cspecial(0, 2 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + (1 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (0 * SPACEBAR_UNIT_LEN), SH_BWARKER_3, IS_SPACEBARWALKER);

    // Lines 254-258: more spacebar
    b.map_sbtype3(0, 4, -1, 1);
    b.map_sbtype10(0, -4, 1, 1);
    b.map_sbtype5(0, 2, 1, 1);
    b.map_sbtype5(0, -2, 1, 1);

    // Lines 259-260: SBtype1 + wire
    b.map_sbtype1(2, -2, -1, 4);
    b.map_setbarshape(BarShapeMode::Wire, false);

    // Lines 262-263: SBtype19
    b.map_sbtype19(0, 1, 0, 0, 7, 0);
    b.map_sbtype19(8, -3, 0, 0, 7, 0);

    // Lines 265-268: house + solid bars
    b.cspecial(200, 0, 0, 4000, SH_R_HOU_0, IS_SHOU0A);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype_b(0, -2, 0, 0);
    b.map_sbtype_b(0, 2, 0, 0);

    // Lines 269-274: wire/solid alternation
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype1(2, 0, 1, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype5(0, 2, 1, 1);
    b.map_sbtype5(8, -2, -1, 1);

    // Line 276: item_6
    b.mapobj(1000, 1 * SPACEBAR_UNIT_LEN, SPACE_VIEWCY + (0 * SPACEBAR_UNIT_LEN), SPACEBAR_BASE_DIST + (2 * SPACEBAR_UNIT_LEN), SH_ITEM_6, IS_ITEM6);

    // Lines 278-285: repeat pattern
    b.map_sbtype_b(0, -2, 0, 0);
    b.map_sbtype_b(0, 2, 0, 0);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype1(2, 0, 1, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype5(0, 2, 1, 1);
    b.map_sbtype5(8, -2, -1, 1);

    // Lines 287-296: more bars
    b.map_sbtype_b(0, -2, 1, 0);
    b.map_sbtype_b(0, 2, 1, 0);
    b.map_sbtype18(0, 4, 0, 0, 0, 4);
    b.map_sbtype18(0, 4, 0, 0, 0, -4);
    b.map_sbtype18(0, -4, 0, 0, 0, 4);
    b.map_sbtype18(0, -4, 0, 0, 0, -4);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 0, 0, 0);
    b.map_sbtype1(2, 0, 2, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype5(0, 2, 2, 1);
    b.map_sbtype5(8, -2, 0, 1);

    // Lines 300-308: special house + bars
    b.special(0, 200, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    b.map_sbtype_b(0, -1, 0, 0);
    b.map_sbtype_b(0, 3, 0, 0);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 1, -1, 0);
    b.map_sbtype1(2, 1, 1, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype5(0, 3, 1, 1);
    b.map_sbtype5(8, -1, -1, 1);

    // Lines 310-319: more bars
    b.map_sbtype_c(0, 0, -2, 0);
    b.map_sbtype_c(0, 0, 2, 0);
    b.map_sbtype18(0, 4, 0, 0, 0, 4);
    b.map_sbtype18(0, 4, 0, 0, 0, -4);
    b.map_sbtype18(0, -4, 0, 0, 0, 4);
    b.map_sbtype18(0, -4, 0, 0, 0, -4);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype0(0, 1, 0, 0);
    b.map_sbtype0(4, -1, 0, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype_b(0, -2, 0, 0);
    b.map_sbtype_b(0, 2, 0, 0);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype1(6, 0, 1, 0);
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype_c(0, 2, -2, 0);
    b.map_sbtype_c(0, 2, 2, 0);
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype0(0, 3, 0, 0);
    b.map_sbtype0(0, 1, 0, 0);

    // Line 331: pathobj check
    b.pathobj(0, 0, -700, 1000, SH_NULLSHAPE, PATH_ID_CHECK, 10, 10);

    // Lines 333-334: gate (gate_Istrat = IS_GATE)
    b.map_sbtype_obj(0, 2, 0, 0, SH_GATE_0, IS_GATE);
    b.pathobj(1500, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 335-341: solid bars + SBtype18 quad
    b.map_setbarshape(BarShapeMode::Solid, false);
    b.map_sbtype_b(0, -2, 0, 0);
    b.map_sbtype_b(0, 2, 0, 0);
    b.map_sbtype18(0, 4, 0, 0, 0, 4);
    b.map_sbtype18(0, 4, 0, 0, 0, -4);
    b.map_sbtype18(0, -4, 0, 0, 0, 4);
    b.map_sbtype18(0, -4, 0, 0, 0, -4);

    // Lines 342-344: wire bars
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_sbtype1(0, 0, -1, 0);
    b.map_sbtype1(8, 0, 1, 0);

    // Line 346: sbtypeA
    b.map_sbtype_a(2, -2, 1, 5);

    // Lines 351-374: horiz and vert moving (speed=30)
    {
    let speed: i32 = 30;
    b.map_sbtype17(6, 0, 12, 0, -speed, -4);
    b.map_sbtype16(6, -12, -1, 0, speed, 3);
    b.map_sbtype17(6, 0, 10, 0, -speed, -4);
    b.map_sbtype16(6, -10, -1, 0, speed, 3);
    b.map_sbtype17(6, -1, -10, 0, speed, -3);
    b.map_sbtype16(6, 10, 1, 0, -speed, 4);

    b.map_sbtype17(2, 0, 12, 0, -speed, -4);
    b.map_sbtype16(2, -12, -1, 0, speed, 3);
    b.map_sbtype17(2, 0, 10, 0, -speed, -6);
    b.map_sbtype16(2, -10, -1, 0, speed, 5);
    b.map_sbtype17(2, -1, -10, 0, speed, -4);
    b.map_sbtype16(2, 10, 1, 0, -speed, 3);
    b.map_sbtype17(2, 1, 10, 0, -speed, -2);
    b.map_sbtype16(2, -10, 0, 0, speed, 7);
    b.map_sbtype17(2, -2, -10, 0, speed, -6);
    b.map_sbtype17(2, 1, 12, 0, -speed, -2);
    b.map_sbtype16(2, -12, 0, 0, speed, 7);
    b.map_sbtype16(2, 10, -1, 0, -speed, 4);
    b.map_sbtype17(2, 2, 10, 0, -speed, -5);
    b.map_sbtype16(2, -10, 0, 0, speed, 3);
    b.map_sbtype17(2, 0, -10, 0, speed, -3);
    b.map_sbtype16(4, 10, 1, 0, -speed, 2);
    }

    // Lines 375-380: more bars + poles
    b.map_sbtype_b(0, 1, 0, 0);
    b.map_sbtype_b(4, -4, 0, 0);
    b.map_sbtype8(8, 5, 0, 0);
    b.mapnobj(800, 0, 0, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    b.mapnobj(800, 400, 100, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    b.mapnobj(800, -400, -100, 2500, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 382-383: friend pair (chase1)
    b.pathobj(0, 1200, 200, 600, SH_FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    b.pathobj(1000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);

    // Lines 384-385: poles
    b.mapnobj(1000, 200, 100, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);
    b.mapnobj(1000, -200, -200, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 386-392: more horiz/vert moving bars
    {
    let speed: i32 = 30;
    b.map_sbtype16(4, -12, -1, 0, speed, 5);
    b.map_sbtype17(2, -1, -12, 0, speed, -4);
    b.map_sbtype16(2, 10, 1, 0, -speed, 3);
    b.map_sbtype17(4, 0, -12, 0, speed, -3);
    b.map_sbtype16(4, 12, -1, 0, -speed, 4);
    b.map_sbtype17(4, 0, 12, 0, -speed, -6);
    b.map_sbtype16(4, -10, 0, 0, speed, 7);
    }

    // Lines 393-394: houses
    b.special(1500, -200, -100, 4000, SH_S_HOU_0, IS_SHOU0);
    b.cspecial(1500, 200, 120, 4000, SH_R_HOU_0, IS_SHOU0A);

    // Lines 396-400: cameleons + pole
    b.special(0, 0, -100, 800, SH_CAMELEON, IS_CAMELEON);
    b.cspecial(0, -100, 100, 800, SH_CAMELEON, IS_CAMELEON);
    b.special(1000, 100, 100, 800, SH_CAMELEON, IS_CAMELEON);
    b.mapnobj(0, 0, 0, 3000, SH_POLE_0_PROXY, STRAT_ADDR_POLE0);

    // Lines 404-406: big iron flame (XPspacebar)
    b.map_xpspacebar(1000, 0, 0, 3000, 0, 6);
    b.map_xpspacebar(1000, -200, 0, 3000, 2, -6);
    b.map_xpspacebar(1000, 200, 0, 3000, 4, 6);

    // Lines 409-415: spacebarC + spacebarZ
    b.map_spacebarc(0, 0, 3000, 1, 3);
    b.map_spacebarc(0, 0, 3000, 3, 3);
    b.map_zspacebar(-1, -1, 4);
    b.map_zspacebar(1, -1, 4);
    b.map_zspacebar(-1, 1, 4);
    b.map_zspacebar(1, 1, 4);

    // Line 418: mapwait 1000
    b.mapwait(1000);

    // Lines 421-422: spacebarC + spacebarX
    b.map_spacebarc(-1, 0, 3000, 4, 3);
    b.map_spacebarx(-1, -1, 0, 2);
    b.mapwait(1000);

    // Lines 426-427: spacebarC + spacebarX
    b.map_spacebarc(1, 0, 3000, 2, -4);
    b.map_spacebarx(2, -1, 0, 4);
    b.mapwait(1000);

    // Lines 431-432: spacebarC + spacebarX
    b.map_spacebarc(0, -2, 3000, 4, 5);
    b.map_spacebarx(0, -3, 0, 2);
    b.mapwait(1000);

    // Lines 436-437: spacebarC + spacebarX
    b.map_spacebarc(0, 1, 3000, 2, -2);
    b.map_spacebarx(0, 1, 0, 4);
    b.mapwait(1000);

    // Lines 442-453: large bit — Zspacebar, spacebarwait, Xspacebar, Yspacebar
    b.map_zspacebar(-2, -2, 0);
    b.map_zspacebar(2, -2, 0);
    b.map_zspacebar(-2, 2, 0);
    b.map_zspacebar(2, 2, 0);
    b.map_spacebarwait(2);
    b.map_xspacebar(0, -2, 0);
    b.map_xspacebar(0, 2, 0);
    b.map_yspacebar(-2, 0, 0);
    b.map_yspacebar(2, 0, 0);
    b.map_spacebarwait(2);

    // Lines 456-463: Zspacebar + SBtype18 quad
    b.map_zspacebar(-2, -2, 0);
    b.map_zspacebar(2, -2, 0);
    b.map_zspacebar(-2, 2, 0);
    b.map_zspacebar(2, 2, 0);
    b.map_sbtype18(0, 4, 0, 0, 0, 4);
    b.map_sbtype18(0, 4, 0, 0, 0, -4);
    b.map_sbtype18(0, -4, 0, 0, 0, 4);
    b.map_sbtype18(0, -4, 0, 0, 0, -4);

    // Lines 466-468: setbgm boss music transition
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 7);
    b.setbgm(BGM_BOSS1);

    // Lines 470-475: colony boss objects
    b.mapobj(0, 0, SPACE_VIEWCY, 4000, SH_COLONY_0, IS_COLONY0);
    b.setvarobj(WM_MAPVAR1);
    b.mapobj(0, 100 << 2, SPACE_VIEWCY, 4000, SH_COLONY_1, IS_COLONY1);
    b.mapobj(0, 0, SPACE_VIEWCY, 4000, SH_COLONY_2, IS_COLONY2);
    b.setalvarptrw(AL_PTR, WM_MAPVAR1);

    // Lines 477-479: mapwait + item_5 + setalvar
    b.mapwait(1000);
    b.mapobj(0, 0, SPACE_VIEWCY, 5000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);

    // Lines 481-484: SBtype18 quad
    b.map_sbtype18(0, 4, 0, 0, 0, 4);
    b.map_sbtype18(0, 4, 0, 0, 0, -4);
    b.map_sbtype18(0, -4, 0, 0, 0, 4);
    b.map_sbtype18(0, -4, 0, 0, 0, -4);

    // Lines 485-489: .wait loop — busy-wait for boss defeat (chkstratdone1)
    b.label("level3_4.wait");
    b.mapwait(16);
    let chkstratdone1_loop_ptr = b.mapcode65816_inline(); 
    b.mapgoto("level3_4.wait");
    b.label("level3_4.end");

    // Line 491: setbg 1_3b
    b.setbg(BG_1_3B);
    b.initbg();

    // Line 494: incmap 3-4-t — 3-4 Sector Z Base Tunnel Map (3-4-T.ASM)
    // Line 3: mapwait 1000
    b.mapwait(1000);

    // Lines 5-8: spacebar pattern — wire mode Z bars + wait
    b.map_setbarshape(BarShapeMode::Wire, false);
    b.map_zspacebar(-1, 0, 0);
    b.map_zspacebar(1, 0, 0);
    b.mapwait(800);

    // Lines 10-26: SY spacebar pattern
    b.map_syspacebar(1, 0, 0);
    b.map_syspacebar(-1, 0, 0);
    b.mapwait(500);
    b.map_syspacebar(1, 0, 0);
    b.map_syspacebar(-1, 0, 0);
    b.mapwait(500);
    b.map_syspacebar(0, 0, 0);
    b.mapwait(500);
    b.map_syspacebar(1, 0, 0);
    b.map_syspacebar(-1, 0, 0);
    b.mapwait(500);
    b.map_syspacebar(1, 0, 0);
    b.map_syspacebar(-1, 0, 0);
    b.mapwait(500);
    b.map_syspacebar(1, 0, 0);
    b.map_syspacebar(-1, 0, 0);
    b.mapwait(1000);

    // Line 27: SX spacebar
    b.map_sxspacebar(0, 0, 1);

    // Line 28: special warker
    b.special(1000, 0x0060, 0, 3500, SH_S_WARK_0, STRAT_ADDR_WARKER3);

    // Lines 29-33: .tunnel0 loop — 4 tunnel_0 objects x3 + trailing set
    b.label("level3_4.t.tunnel0");
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    b.maploop("level3_4.t.tunnel0", 3);
    // Lines 34-37: trailing tunnel_0 set
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(400, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 39: mapUPDNdoor 1500,4000
    b.mapobj(1500, 0, -60, 4000, SH_UP_DOOR_PROXY, STRAT_ADDR_UPDOOR);

    // Line 40: WALL_0 obstacle
    b.mapobj(1000, 0, -100, 5000, SH_WALL_0_PROXY, IS_HARD180YR);

    // Lines 41-48: two sets of 4 tunnel_0 objects
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Lines 49-50: WALL_2 pair
    b.mapobj(0, 0x0060, -60, 4000, SH_WALL_2, IS_HARD180YR);
    b.mapobj(1000, -0x0060, -60, 4000, SH_WALL_2, IS_HARD180YR);

    // Lines 51-54: tunnel_0 set
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 56: mapLRdoor 0,4000
    b.mapobj(0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.mapobj(0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.setalvarb(AL_ROTZ, DEG180);
    // mapwait 0 (from LRdoor macro first arg)

    // Lines 57-59: warker enemies
    b.special(300, 0, 0, 4300, SH_S_WARK_0, STRAT_ADDR_WARKER3);
    b.mapobj(300, -70, 0, 4050, SH_WARKER_3_PROXY, STRAT_ADDR_WARKER3);
    b.special(1000, 0x0070, 0, 4550, SH_S_WARK_0, STRAT_ADDR_WARKER3);

    // Lines 60-64: .tunnel1 loop — 4 tunnel_0 objects x5
    b.label("level3_4.t.tunnel1");
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(1000, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);
    b.maploop("level3_4.t.tunnel1", 5);

    // Lines 65-68: trailing tunnel_0 set (last botleft has 0 wait)
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(0, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 69: bou_0 wall obstacle
    b.mapobj(1000, 0, -60, 4100, SH_BOU_0_PROXY, STRAT_ADDR_TWALL0);

    // Lines 70-73: tunnel_0 set
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(200, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Lines 75-77: three mapLRdoor calls
    // mapLRdoor 400,4000
    b.mapobj(0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.mapobj(0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(400);
    // mapLRdoor 400,4000
    b.mapobj(0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.mapobj(0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(400);
    // mapLRdoor 500,4000
    b.mapobj(0, -45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.mapobj(0, 45, -60, 4000, SH_OPEN_L_PROXY, STRAT_ADDR_OPENLR);
    b.setalvarb(AL_ROTZ, DEG180);
    b.mapwait(500);

    // Lines 78-81: final tunnel_0 set
    b.mapobj(0, 90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPRIGHT1);
    b.mapobj(0, -90, -120, 4000, SH_TUNNEL_0, STRAT_ADDR_TOPLEFT1);
    b.mapobj(0, 90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTRIGHT1);
    b.mapobj(500, -90, 0, 4000, SH_TUNNEL_0, STRAT_ADDR_BOTLEFT1);

    // Line 496: mapjsr Mtunnelexit
    // TODO: port Mtunnelexit (medium tunnel exit sequence) as a subroutine.
    // For now, inline a simplified version.
    b.mapwait(100);

    // Line 497: setbgm $f1 (fade out music)
    b.setbgm(BGM_FADEOUT);

    // Line 500: maprts (end of map3_4b)
    b.maprts();

    // ---- MAP3_4C subroutine (boss wait section) ----
    b.label("level3_4.map3_4c");
    b.setbg(BG_3_4C);
    b.initbg();
    b.label("level3_4.map3_4c.wait");
    b.mapwait(2000);
    b.mapgoto("level3_4.map3_4c.wait");
    b.maprts();

    // CL_SHIP3_4 — clear-demo for colony ship levels
    append_cl_ship_submap(&mut b);

    // ---- Resolve ----

    b.resolve();

    // C zeroes these when missing; the labels are emitted above.
    assert!(
        b.lookup_label("level3_4.skillfly_bonus_0_skip").is_some(),
        "level3_4 skillfly bonus 0 skip label missing"
    );
    assert!(
        b.lookup_label("level3_4.skillfly_bonus_1_skip").is_some(),
        "level3_4 skillfly bonus 1 skip label missing"
    );
    assert!(
        b.lookup_label("level3_4.end").is_some(),
        "level3_4 end label missing"
    );

    let (data, labels) = b.finish();
    // C `register_level3_4_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (skillfly_bonus0_guard_ptr, "level3_4_skillfly_bonus0_guard"),
            (skillfly_bonus1_guard_ptr, "level3_4_skillfly_bonus1_guard"),
            (chkstratdone1_loop_ptr, "level3_4_chkstratdone1_check"),
        ],
    )
}
