//! MAP_ID_1_5 — Venom 1 Orbital (Level 5, Route 1).
//!
//! C oracle: `src/map/levels.c` `build_level1_5_slice()` (lines 9771-9999)
//! + `register_level1_5_inline_callbacks()` (lines 10000-10027).
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_5.ASM` — wrapper: mapjsr map1_5, cl_dive, mapend__not 1_6.
//! - `MAP1_5.ASM`   — Venom 1 orbital space content + boss_b_1 boss block.
//! - `CL_DIVE.ASM`  — dive clear demo (`cl_dive` label).

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

/// Level-local constants from `src/map/levels.c` #defines.
// TODO(consolidation): move to consts.rs
mod lc {
    use crate::consts::sh;

    // Shapes (levels.c SH_* block)
    pub const SH_ROUND_0: u16 = 17;
    pub const SH_BIG_M: u16 = 19;
    pub const SH_W_L: u16 = 50;
    pub const SH_BAZOOKA: u16 = 132;
    pub const SH_UPER_M: u16 = 133;
    pub const SH_ZACO_B: u16 = 202;
    pub const SH_S_ZACO_0: u16 = 222;
    pub const SH_BZACO_8: u16 = 232;
    pub const SH_MOTHER1: u16 = 278;
    pub const SH_MY_BIRD: u16 = 557;
    // Proxies (levels.c: unported shapes proxied to placeholders)
    pub const SH_ZACO_1_PROXY: u16 = SH_S_ZACO_0;
    pub const SH_WARP_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_MINE_0_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_BOSS_B_1_PROXY: u16 = sh::NULLSHAPE;

    // Strategies (levels.c IS_* block)
    pub const IS_CLSHIPDIVEA: u32 = 40;
    pub const IS_CLSHIPDIVEB: u32 = 41;
    pub const IS_CLSHIPDIVEC: u32 = 42;
    pub const IS_WINDMILL: u32 = 66;
    pub const IS_MISSPOD: u32 = 68;
    pub const IS_WINGLAZERMAN: u32 = 91;
    pub const IS_BAZOOKAL: u32 = 158;
    pub const IS_BAZOOKAR: u32 = 159;
    pub const IS_UPERM: u32 = 160;
    pub const IS_MINE0: u32 = 246;

    // Synthetic strategy addresses (levels.c STRAT_ADDR_*)
    pub const STRAT_ADDR_MOTHER2: u32 = crate::consts::STRAT_ADDR_MOTHER2;
    pub const STRAT_ADDR_WARP: u32 = 0x06000E;
    pub const STRAT_ADDR_BOSSB: u32 = 0x06000F;

    // Path ids (src/path/path_literals.h)
    pub const PATH_ID_MY_BIRD: u16 = 245;
    pub const PATH_ID_CHASE1_1: u16 = 247;
    pub const PATH_ID_CHASE1_2: u16 = 248;
    pub const PATH_ID_PATRET_IRAB: u16 = 259;
    pub const PATH_ID_PATRET_IFAL: u16 = 261;
    pub const PATH_ID_CHECK: u16 = 266;
    pub const PATH_ID_CHASE2_1: u16 = 269;
    pub const PATH_ID_CHASE2_2: u16 = 270;
    pub const PATH_ID_CHASE3_1: u16 = 271;
    pub const PATH_ID_CHASE3_2: u16 = 272;
    pub const PATH_ID_PATRET: u16 = 308;
    pub const PATH_ID_E_SHAWERL: u16 = 354;
    pub const PATH_ID_E_SHAWERR: u16 = 355;
}

/// C `build_level1_5_slice()` + `register_level1_5_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();
    let mm = crate::mothers::mother_maps();

    // LEVEL1_5.ASM wrapper: initlevel 1_5,mstarwipe_circle
    // mapwait 100 / setvar dospacesc,1 / setvar.W bg2Yscroll,172
    // mapjsr map1_5 / mapjsr cl_dive / mapend__not level1_6
    b.mapjsr("level1_5.map1_5");
    b.mapjsr("cl_dive");
    // mapend__not level1_6: sets levelfinished=7 (le_startgame), matching
    // ROM LEVEL1_5.ASM:13 `mapend__not` (MAPMACS.INC:1989-1990) and level2_5.
    // Was mapend(6) (le_endofgame) — see AUDIT Finding 3.
    b.mapend(7);

    // === MAP1_5.ASM subroutine — Venom 1 Orbital space content ===
    b.label("level1_5.map1_5");

    // Line 3: mapwait 3000
    b.mapwait(3000);

    // Lines 15-18: pathcspecial x4
    b.pathcspecial(0, 400, 100, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);
    b.pathcspecial(0, -400, 100, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);
    b.pathcspecial(0, 200, 500, 900, lc::SH_ZACO_1_PROXY, lc::PATH_ID_E_SHAWERR, 10, 10);
    b.pathcspecial(4500, -200, 500, 1200, lc::SH_ZACO_1_PROXY, lc::PATH_ID_E_SHAWERR, 10, 10);

    // Lines 20-24: friendship_4 chase2 + zaco_b chase2 + bzaco_8 patret trio
    b.pathobj(0, -900, 0, 0, sh::FRIENDSHIP_4, lc::PATH_ID_CHASE2_1, 10, 10);
    b.pathcspecial(5000, -900, 0, 0, lc::SH_ZACO_B, lc::PATH_ID_CHASE2_2, 10, 10);
    b.pathcspecial(0, 0, 200, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);
    b.pathcspecial(0, 400, -400, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);
    b.pathcspecial(5000, -400, -400, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);

    // Lines 26-30: uper_missile — mapmother + cspecial loop x2
    b.mapmother(1200, 0, 2000, 3000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER2, mm.map_uperm);
    b.label("level1_5.uperloop");
    b.cspecial(1200, 180, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1200, -180, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.maploop("level1_5.uperloop", 2);

    // Lines 32-37: check + e_shawer pair
    b.pathobj(0, 0, -700, 1000, sh::NULLSHAPE, lc::PATH_ID_CHECK, 10, 10);
    b.pathcspecial(0, 200, 1000, 700, lc::SH_ZACO_1_PROXY, lc::PATH_ID_E_SHAWERR, 10, 10);
    b.pathspecial(1000, -200, 1000, 800, lc::SH_ZACO_1_PROXY, lc::PATH_ID_E_SHAWERL, 10, 10);

    // Lines 39-53: maprem mother1, big_m missile, windmill
    b.mapremove(lc::SH_MOTHER1);
    b.cspecial(2800, -100, SPACE_VIEWCY - 100, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);
    // windmill: special 0,-200,space_viewCY+1500,4000,round_0,windmill_istrat
    b.special(0, -200, SPACE_VIEWCY + 1500, 4000, lc::SH_ROUND_0, lc::IS_WINDMILL);
    b.setalvarb(al::VEL, 120);
    b.setalvarb(al::ROTY, 134);
    b.setalvarb(al::ROTX, 230);
    b.mapwait(3000);
    b.setalvarb(al::VEL, 0);
    b.setalvarb(al::ROTX, 0);
    b.setalvarb(al::ROTY, 127);
    b.mapwait(2000);
    b.setalvarb(al::VEL, 120);
    b.setalvarw(al::SWORD1, -2);

    // Lines 54-55: check + warp
    b.pathobj(0, 0, -700, 1000, sh::NULLSHAPE, lc::PATH_ID_CHECK, 10, 10);
    b.cspecial(0, 100, 0, 1500, lc::SH_WARP_PROXY, lc::STRAT_ADDR_WARP);

    // Lines 56-80: second windmill with rotation sequence
    b.mapwait(2000);
    b.special(0, 500, SPACE_VIEWCY - 300, 1000, lc::SH_ROUND_0, lc::IS_WINDMILL);
    b.setalvarb(al::VEL, 127);
    b.setalvarb(al::ROTY, 64);
    b.setalvarb(al::ROTX, 5);
    b.mapwait(300);
    b.setalvarb(al::ROTY, 40);
    b.mapwait(200);
    b.setalvarb(al::ROTY, 20);
    b.mapwait(200);
    b.setalvarb(al::ROTY, 0);
    b.mapwait(1400);
    b.setalvarb(al::ROTY, 250);
    b.mapwait(100);
    b.setalvarb(al::ROTY, 230);
    b.mapwait(100);
    b.setalvarb(al::ROTY, 200);
    b.mapwait(100);
    b.setalvarb(al::ROTY, 170);
    b.mapwait(100);
    b.setalvarb(al::ROTY, 140);
    b.mapwait(100);
    b.setalvarb(al::VEL, 0);
    b.setalvarb(al::ROTX, 0);
    b.mapwait(3000);
    b.setalvarb(al::VEL, 100);
    b.setalvarw(al::SWORD1, 4);

    // Lines 84-91: big_missile section
    b.pathobj(0, 0, -700, 1000, sh::NULLSHAPE, lc::PATH_ID_CHECK, 10, 10);
    b.cspecial(2000, 0, SPACE_VIEWCY, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);
    b.pathcspecial(0, 0, -300, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET_IFAL, 10, 10);
    b.cspecial(2000, 100, SPACE_VIEWCY - 100, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);
    b.cspecial(1000, -100, SPACE_VIEWCY + 100, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);
    b.cspecial(1000, -100, SPACE_VIEWCY - 100, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);
    b.cspecial(1000, 100, SPACE_VIEWCY + 100, 4000, lc::SH_BIG_M, lc::IS_MISSPOD);

    // Lines 92-94: warp
    b.pathcspecial(0, 0, -300, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET_IRAB, 10, 10);
    b.cspecial(3000, 0, -200, 1500, lc::SH_WARP_PROXY, lc::STRAT_ADDR_WARP);

    // Lines 96-112: skillfly ring section
    b.skillfly_init();
    b.skillfly_set(0, 0, 4000, 100);
    b.mapobj(0, -150, 0, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, 150, 0, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, -100, 100, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, -100, -100, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, 100, 100, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, 100, -100, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(0, 0, 150, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    b.mapobj(4000, 0, -150, 4000, lc::SH_MINE_0_PROXY, lc::IS_MINE0);
    // skillfly_bonus item_5
    let skillfly_bonus0_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, 0, 1500, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.label("level1_5.skillfly_bonus_0_skip");
    b.skillfly_set_default(0, 0, 1500);
    b.mapwait(1500);
    // skillfly_bonus item_5 (second)
    let skillfly_bonus1_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 0, 0, 1500, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.label("level1_5.skillfly_bonus_1_skip");
    b.mapwait(1000);

    // Lines 115-117: bazooka + winglazerman
    b.cspecial(5000, 0, 1000, 3500, lc::SH_BAZOOKA, lc::IS_BAZOOKAR);
    b.cspecial(1000, -400, SPACE_VIEWCY, 3000, lc::SH_W_L, lc::IS_WINGLAZERMAN);

    // Lines 119-130: mapmother + chase3 + uper_m group + maprem
    b.mapmother(2000, 0, 2000, 3000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER2, mm.map_uperm);
    b.pathobj(0, 0, 500, 0, sh::FRIENDSHIP_4, lc::PATH_ID_CHASE3_1, 10, 10);
    b.pathcspecial(6000, 0, 500, 0, lc::SH_ZACO_B, lc::PATH_ID_CHASE3_2, 10, 10);
    b.cspecial(1200, 0, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1200, 100, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1200, -100, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.mapremove(lc::SH_MOTHER1);

    // Lines 132-135: gate_0 + e_gate path
    b.mapobj(500, 100, SPACE_VIEWCY + 100, 3000, sh::GATE_0, is::GATE);
    b.pathobj(2500, 3000, 3000, 3000, sh::NULLSHAPE, path::E_GATE, 10, 10);
    b.mapwait(1000);

    // Lines 137-147: warp + uper_m series
    b.cspecial(3000, 0, 0, 1500, lc::SH_WARP_PROXY, lc::STRAT_ADDR_WARP);
    b.cspecial(0, 0, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.special(2000, 0, -200, 1500, lc::SH_WARP_PROXY, lc::STRAT_ADDR_WARP);
    b.cspecial(0, 0, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1000, 0, 0, 1500, lc::SH_WARP_PROXY, lc::STRAT_ADDR_WARP);
    b.cspecial(1300, -100, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, 100, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, 200, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, -300, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, 300, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);

    // Lines 152-159: chase1 + uper_m series + mapmother + supply_bird
    b.pathobj(0, 1200, 200, 600, sh::FRIENDSHIP_4, lc::PATH_ID_CHASE1_1, 10, 10);
    b.pathcspecial(1000, 1200, 200, 600, lc::SH_ZACO_B, lc::PATH_ID_CHASE1_2, 10, 10);
    b.cspecial(1300, 100, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, -200, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.cspecial(1300, 200, 2000, 3000, lc::SH_UPER_M, lc::IS_UPERM);
    b.mapmother(2500, 0, 2000, 3000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER2, mm.map_uperm);
    // supply_bird
    b.pathobj(5000, -350, -400, 0, lc::SH_MY_BIRD, lc::PATH_ID_MY_BIRD, 10, 10);

    // Lines 161-168: maprem + bazooka + bzaco_8 patret pair
    b.mapremove(lc::SH_MOTHER1);
    b.cspecial(1000, 0, 1000, 3500, lc::SH_BAZOOKA, lc::IS_BAZOOKAL);
    b.pathcspecial(1500, 400, -100, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);
    b.pathcspecial(3000, -400, -100, -100, lc::SH_BZACO_8, lc::PATH_ID_PATRET, 10, 10);

    // Lines 169-179: boss section
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, 1000, 3500, lc::SH_BOSS_B_1_PROXY, lc::STRAT_ADDR_BOSSB);

    // mapwaitboss / markboss boss15
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level1_5.bosswait.loop");
    b.mapif_builtin(cb::CHKBOSSDEAD, "level1_5.bosswait.cont");
    b.mapgoto("level1_5.bosswait.loop");
    b.label("level1_5.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();

    // markboss boss15
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(cb::MARKBOSS_L);
    b.mapwait(3000);
    b.maprts();

    // CL_DIVE.ASM — clear demo (dive type) appended as subroutine.
    append_cl_dive_submap(&mut b);

    b.resolve();

    // C: s_level1_5_skillfly_bonus{0,1}_skip_ptr lookups (must exist).
    assert!(
        b.lookup_label("level1_5.skillfly_bonus_0_skip").is_some(),
        "level1_5 skillfly bonus 0 skip label missing"
    );
    assert!(
        b.lookup_label("level1_5.skillfly_bonus_1_skip").is_some(),
        "level1_5 skillfly bonus 1 skip label missing"
    );

    let (data, labels) = b.finish();

    // C `register_level1_5_inline_callbacks()` — registration-call order,
    // guarded by ptr != 0 like C. NOTE: the mapwaitboss handlers reuse the
    // level1_1_* C functions.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    for (ptr, name) in [
        (skillfly_bonus0_guard_ptr, "level1_5_skillfly_bonus0_guard"),
        (skillfly_bonus1_guard_ptr, "level1_5_skillfly_bonus1_guard"),
        (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
        (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
        (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
    ] {
        if ptr != 0 {
            inline_regs.push((ptr, name));
        }
    }

    Route1Level {
        level: BuiltLevel {
            data,
            labels,
            native_callbacks: vec![],
            inline_callbacks: vec![],
        },
        native_regs: vec![], // level1_5 registers no natives
        inline_regs,
    }
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::consts::{op, wm};

    /// AUDIT Finding 3: the level1_5 wrapper's `mapend__not` must store
    /// levelfinished=7 (le_startgame), matching ROM LEVEL1_5.ASM:13 and
    /// level2_5 — NOT 6 (le_endofgame). The map VM encodes `mapend(N)` as
    /// `setvarb(LEVELFINISHED, N)` = [SETVARB, N, addr_lo, addr_hi, 0].
    #[test]
    fn mapend_sets_levelfinished_le_startgame() {
        let lvl = build();
        let data = &lvl.level.data;
        let lo = (wm::LEVELFINISHED & 0xFF) as u8;
        let hi = (wm::LEVELFINISHED >> 8) as u8;
        let mut found = None;
        for i in 0..data.len().saturating_sub(4) {
            if data[i] == op::SETVARB
                && data[i + 2] == lo
                && data[i + 3] == hi
                && data[i + 4] == 0
            {
                found = Some(data[i + 1]);
                break;
            }
        }
        assert_eq!(found, Some(7), "level1_5 mapend must store le_startgame (7)");
    }
}

/// C `append_cl_dive_submap()` (levels.c lines 4324-4381) — CL_DIVE.ASM
/// clear demo for dive levels.
// DUPLICATE: consolidate (shared with other routes).
fn append_cl_dive_submap(b: &mut MapBuilder) {
    b.label("cl_dive");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_FANFARE);
    b.mapcodejsl_builtin(cb::SET_PLAYER_DIVE_L);
    b.mapwait(2800);

    b.setvarb(wm::STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(cb::FROG_ALIVE, "cl_dive.frog_alive");
    b.mapgoto("cl_dive.nf");
    b.label("cl_dive.frog_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_FROG);
    b.mapobj(CL_GND_FRIENDWAIT, 200, SPACE_VIEWCY, 50, sh::MYSHIP_4, lc::IS_CLSHIPDIVEB);
    b.label("cl_dive.nf");

    b.mapif_builtin(cb::BUNNY_ALIVE, "cl_dive.bunny_alive");
    b.mapgoto("cl_dive.nb");
    b.label("cl_dive.bunny_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_BUNNY);
    b.mapobj(CL_GND_FRIENDWAIT, -200, SPACE_VIEWCY, 50, sh::MYSHIP_4, lc::IS_CLSHIPDIVEA);
    b.label("cl_dive.nb");

    b.mapif_builtin(cb::COCK_ALIVE, "cl_dive.cock_alive");
    b.mapgoto("cl_dive.nc");
    b.label("cl_dive.cock_alive");
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_COCK);
    b.mapobj(CL_GND_FRIENDWAIT, 0, SPACE_VIEWCY - 40, -50, sh::MYSHIP_4, lc::IS_CLSHIPDIVEC);
    b.label("cl_dive.nc");

    b.mapwait(5000);
    b.setvarb(wm::CLB2, 0);
    b.setvarb(wm::STAGECLEAR, 0);
    b.mapcodejsl_builtin(cb::CL_GROUND_PRINTLEVELFIN);
    b.label("cl_dive.eswait");
    b.mapwait(1);
    b.maploop("cl_dive.eswait", 100);

    // Inline 65816: clear engine sound flag (pshipflags3 &= ~psf3_enginesnd)
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(cb::CL_DIVE_CLEAR_ENGINESND);
    b.qfadedown();
    b.waitfade();
    b.setvarb(wm::CLB2, 1);
    b.maprts();
}
