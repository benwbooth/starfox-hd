//! MAP_ID_3_6 — Venom 3 Orbital (Level 3, Route 3).
//!
//! C oracle: `src/map/levels.c` `build_level3_6_slice()` +
//! `register_level3_6_inline_callbacks()`.
//! ASM: LEVEL3_6.ASM / MAP3_6.ASM / CL_DIVE.ASM.

use super::common::*;
use super::finish_level;
use super::Route3Level;
use crate::builder::MapBuilder;

pub(crate) fn build() -> Route3Level {
    let mut b = MapBuilder::new();
    let mm = crate::mothers::mother_maps();

    // MAP3_6.ASM wrapper: space level, mapjsr map3_6, mapjsr cl_dive, mapend.
    b.mapjsr("level3_6.map3_6");
    b.mapjsr("cl_dive");
    // mapend (level3_6 transitions to level3_7 = Venom surface).
    b.mapend(1);

    // === MAP3_6.ASM subroutine — Venom 2 Space content ===
    b.label("level3_6.map3_6");

    // Line 3: mapwait 2000
    b.mapwait(2000);

    // Lines 6-10: big_missile group
    b.cspecial(2000, 0, SPACE_VIEWCY, 4000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(2000, 100, SPACE_VIEWCY - 100, 4000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(3000, -100, SPACE_VIEWCY + 100, 4000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(2000, -100, SPACE_VIEWCY - 100, 4000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(2000, 100, SPACE_VIEWCY + 100, 4000, SH_BIG_M, IS_MISSPOD);

    // Lines 13-20: zacos — mapmother mine2 + shark call_fol paths + bzaco_8 + maprem
    b.mapmother(6000, 0, 1035 + SPACE_VIEWCY, 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_mine2);
    b.pathcspecial(2000, 1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    b.pathcspecial(2000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    b.pathcspecial(2000, 1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    b.pathcspecial(2000, -1000, -700, 2000, SH_SHARK, PATH_ID_CALL_FOL, 10, 10);
    b.pathcspecial(4000, -700, 100, -100, SH_BZACO_8, PATH_ID_PATRET_IFAL, 10, 10);
    b.mapremove(SH_MOTHER1);

    // Line 22: mapwait 2000
    b.mapwait(2000);

    // Lines 25-31: M formation (szaco2_mapobj)
    b.szaco2_mapobj(0, 2000, 0, 0, 100);
    b.szaco2_mapobj(-500, 1000, -300, 100, 0);
    b.szaco2_mapobj(500, 1000, 300, 100, 100);
    b.szaco2_mapobj(-1000, 1000, -400, -100, 0);
    b.szaco2_mapobj(1000, 1000, 400, -100, 100);

    // Lines 33-43: mapmother + friend chase3 + spacepilon + uper_m group + maprem
    b.mapwait(2000);
    b.mapmother(9000, 0, 1035 + SPACE_VIEWCY, 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_mine2);
    b.pathobj(0, 0, 400, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE3_1, 200, 10);
    b.pathobj(2000, 0, 400, 0, SH_ZACO_B, PATH_ID_CHASE3_2, 10, 10);
    b.mapobj(2000, 200, -200, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    b.cspecial(1200, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1200, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1200, -100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.mapremove(SH_MOTHER1);

    // Lines 47-55: mothers + windmill
    b.mapmother(5000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_uperm);
    b.mapremove(SH_MOTHER1);
    // windmill
    b.special(0, 0, 0, 4000, SH_ROUND_0, IS_WINDMILL);
    b.setalvarb(AL_ROTY, DEG180);
    b.setalvarw(AL_SWORD1, 1);

    // Lines 54-56: mapmother mine2 + maprem + asteroid cspecial
    b.mapmother(6000, 0, 1035 + SPACE_VIEWCY, 1800, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_mine2);
    b.mapremove(SH_MOTHER1);
    b.cspecial(3000, 0, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, IS_MISSPOD);

    // Lines 58-74: skillfly section
    b.skillfly_init();
    b.skillfly_set(0, SPACE_VIEWCY, 6000, 100);
    b.mapobj(0, 0, SPACE_VIEWCY, 6000, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    b.special(0, -180, -250, 6000, SH_R_HOU_0, IS_SHOU0A);
    b.pathcspecial(0, 180, -250, 6000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    b.cspecial(0, 300, 0, 6000, SH_R_HOU_0, IS_SHOU0A);
    b.special(0, 180, 250, 6000, SH_R_HOU_0, IS_SHOU0A);
    b.pathcspecial(0, -180, 250, 6000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    b.cspecial(6000, -300, 0, 6000, SH_R_HOU_0, IS_SHOU0A);
    // skillfly_bonus item_6
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline(); 
    b.mapobj(0, 0, SPACE_VIEWCY, 1500, SH_ITEM_6, IS_ITEM6);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_6.skillfly_bonus_0_skip");
    b.skillfly_set_default(0, SPACE_VIEWCY, 1500);
    b.mapwait(1500);
    // skillfly_bonus item_7
    let skillfly_bonus1_guard_ptr = b.mapcode65816_inline(); 
    b.mapobj(0, 0, SPACE_VIEWCY, 1500, SH_ITEM_7, IS_ITEM7);
    b.setalvarb(AL_SBYTE1, 1);
    b.label("level3_6.skillfly_bonus_1_skip");

    // Lines 75-82: friend chase2 + gate + e_gate
    b.mapwait(1000);
    b.pathobj(0, -900, 0, 0, SH_FRIENDSHIP_4, PATH_ID_CHASE2_1, 10, 10);
    b.pathobj(2000, -900, 0, 0, SH_ZACO_B, PATH_ID_CHASE2_2, 10, 10);
    b.mapobj(0, -280, SPACE_VIEWCY, 3000, SH_GATE_0, IS_GATE);
    b.pathobj(2000, 3000, 0, 1000, SH_NULLSHAPE, PATH_ID_E_GATE, 10, 10);

    // Lines 85-89: warp section + mapmother
    b.mapmother(2000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_uperm);
    b.cspecial(2000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    b.special(2000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    b.cspecial(4000, 0, 0, 1500, SH_WARP_PROXY, STRAT_ADDR_WARP);
    b.mapremove(SH_MOTHER1);

    // Lines 91-102: bazooka + uper_m + skillfly + supply_bird
    b.cspecial(1000, -100, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    b.cspecial(1200, 0, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(1200, 100, 2000, 3000, SH_UPER_M, IS_UPERM);
    b.cspecial(3000, 100, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAR);
    b.skillfly_init();
    b.skillfly_set(0, SPACE_VIEWCY, 4000, 100);
    b.pathcspecial(0, 0, 0, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 2, 4);
    b.mapmother(4000, 0, 1035 + SPACE_VIEWCY, 1500, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_mine2);
    // skillfly_bonus — reuse bonus0 guard (already consumed, so just mapobj)
    b.mapobj(4000, 0, SPACE_VIEWCY, 1500, SH_ITEM_5, IS_ITEM5);
    b.setalvarb(AL_SBYTE1, 1);
    // supply_bird
    b.pathobj(4000, -400, -300, 0, SH_MY_BIRD, PATH_ID_MY_BIRD, 10, 10);
    b.mapremove(SH_MOTHER1);

    // Lines 104-115: mapmother + big_m missiles + bazooka + shieldr + spacepilon
    b.mapmother(3000, 0, 2000, 3000, SH_MOTHER1, STRAT_ADDR_MOTHER2, mm.map_uperm);
    b.cspecial(3000, 100, SPACE_VIEWCY - 100, 3000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(2000, -100, SPACE_VIEWCY + 100, 3000, SH_BIG_M, IS_MISSPOD);
    b.cspecial(4000, -200, 1000, 3500, SH_BAZOOKA, IS_BAZOOKAL);
    b.pathspecial(200, 100, 100, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathcspecial(200, 0, 0, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.pathcspecial(5000, -100, 100, 3000, SH_SHIELDR, PATH_ID_E_SHIELDR, 10, 10);
    b.mapobj(10000, 200, -200, 2000, SH_SPACEPILON, STRAT_ADDR_SPACEPILON);
    b.mapremove(SH_MOTHER1);
    b.mapwait(4500);

    // Lines 117-160: boss section
    // .boss: busy-wait for noctrl flag to clear
    b.label("level3_6.boss");
    b.mapwait(1);
    let noctrl_wait_ptr = b.mapcode65816_inline(); 

    // .tcont: wait for player HP > 0 and fly mode check
    b.label("level3_6.owait");
    b.mapwait(5);
    let hpcheck_wait_ptr = b.mapcode65816_inline(); 

    // .cont2: boss spawn
    b.label("level3_6.cont2");
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, 2000, 2500, SH_BOSS_F_3_PROXY, STRAT_ADDR_BOSSF);

    // mapwaitboss / markboss boss36
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline(); 
    b.label("level3_6.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "level3_6.bosswait.cont");
    b.mapgoto("level3_6.bosswait.loop");
    b.label("level3_6.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline(); 
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline(); 

    // markboss boss36
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);
    b.maprts();

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    append_cl_dive_submap(&mut b);

    b.resolve();

    // C zeroes these when missing; all five labels are emitted above.
    for (label, what) in [
        ("level3_6.skillfly_bonus_0_skip", "skillfly bonus 0 skip"),
        ("level3_6.skillfly_bonus_1_skip", "skillfly bonus 1 skip"),
        ("level3_6.boss", "noctrl-wait boss target"),
        ("level3_6.owait", "hpcheck-wait owait target"),
        ("level3_6.cont2", "flymode-check cont2 target"),
    ] {
        assert!(b.lookup_label(label).is_some(), "level3_6 {what} label missing");
    }

    let (data, labels) = b.finish();
    // C `register_level3_6_inline_callbacks()` registration-call order.
    finish_level(
        data,
        labels,
        vec![
            (skillfly_bonus0_guard_ptr, "level3_6_skillfly_bonus0_guard"),
            (skillfly_bonus1_guard_ptr, "level3_6_skillfly_bonus1_guard"),
            (noctrl_wait_ptr, "map3_6_noctrl_wait"),
            (hpcheck_wait_ptr, "map3_6_hpcheck_wait"),
            (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
        ],
    )
}
