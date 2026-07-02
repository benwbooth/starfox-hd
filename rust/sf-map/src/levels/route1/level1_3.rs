//! MAP_ID_1_3 — Space Armada (Level 3, Route 1).
//!
//! C oracle: `src/map/levels.c` `build_level1_3_opening_slice()`,
//! `append_map1_3a1_submap()`, `append_map1_3a2_submap()`,
//! `append_map1_3b2_submap()`, `append_cl_ship_submap()` and
//! `register_level1_3_inline_callbacks()`.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_3.ASM` — level wrapper: warp-out intro, SHIP1/SHIP2 bounded
//!   sections, big-ship interior, washing-machine room, jsr chain.
//! - `CL_WARPO.ASM` — warp-out cutscene (`level1_3.cl_warpout`).
//! - `MAP1_3A.ASM`  — SPACE section (`level1_3.map1_3a`).
//! - `MAP1_3A1.ASM` — ship 1 interior (`level1_3.map1_3a1`).
//! - `MAP1_3A2.ASM` — ship 2 interior (`level1_3.map1_3a2`).
//! - `MAP1_3B2.ASM` — ship 2 tunnel stub (`level1_3.map1_3b2`).
//! - MAP1_3C / MAP1_3D — big-ship interior + wash-room boss marker.
//! - `CL_SHIP.ASM`  — shared clear demo for route-1 ship levels
//!   (`cl_ship3_4` / `cl_ship1_3` entries, shared `cl_ship.cont`).

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

/// Constants missing from `consts.rs`; values are verbatim from the
/// `src/map/levels.c` `#define` blocks (path ids from
/// `src/path/path_literals.h`).
/// TODO(consolidation): move to consts.rs.
mod lc {
    use crate::consts::sh;

    // ---- backgrounds (levels.c BG_* block) ----
    pub const BG_1_3I: i32 = 6;
    pub const BG_1_3B: i32 = 8;
    pub const BG_1_3C: i32 = 9;
    pub const BG_1_3E: i32 = 12;
    pub const BG_3_4D: i32 = 35;

    // ---- shape ids (levels.c SH_* block) ----
    pub const SH_W_L: u16 = 50;
    pub const SH_ZACO_7: u16 = 129;
    pub const SH_R_HOU_0: u16 = 162;
    pub const SH_S_HOU_0: u16 = 163;
    pub const SH_ZACO_B: u16 = 202;
    pub const SH_S_ZACO_0: u16 = 222;
    pub const SH_BZACO_8: u16 = 232;
    pub const SH_MOTHER1: u16 = 278;
    pub const SH_SPACEPILON: u16 = 614;

    // Nullshape proxies for shapes not yet in the compiled catalog.
    pub const SH_BOU_1_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_PIPE_9_0_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_PIPE_9_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_1_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_3_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_4_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_0_C_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_5S_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_5M_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SHIP_5_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_S_DOOR_1_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_S_DOOR_2_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_BSHIPEXITFACE_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_COLONY_0_PROXY: u16 = sh::NULLSHAPE;
    pub const SH_SSHIP_0_C_PROXY: u16 = sh::NULLSHAPE;

    // ---- strategy ids (levels.c IS_* block) ----
    pub const IS_CLSHIPSHIPA: u32 = 25;
    pub const IS_CLSHIPSHIPB: u32 = 26;
    pub const IS_CLSHIPSHIPC: u32 = 27;
    pub const IS_UP1MAN: u32 = 90;
    pub const IS_WINGLAZERMAN: u32 = 91;
    pub const IS_SZACO5: u32 = 156;
    pub const IS_SHOU0: u32 = 178;
    pub const IS_SHOU0A: u32 = 179;
    pub const IS_COLONYEXIT: u32 = 236;

    // ---- synthetic strategy addresses (levels.c STRAT_ADDR_*) ----
    pub const STRAT_ADDR_MOTHER1: u32 = 0x020000;
    pub const STRAT_ADDR_SPACEPILON: u32 = 0x030004;
    pub const STRAT_ADDR_SHIP0CDOWN: u32 = 0x030007;
    pub const STRAT_ADDR_SHIP1A: u32 = 0x05000B;
    pub const STRAT_ADDR_SHIP2: u32 = 0x05000C;
    pub const STRAT_ADDR_SDOOR1: u32 = 0x05000D;
    pub const STRAT_ADDR_SDOOR2: u32 = 0x05000E;
    pub const STRAT_ADDR_CRUISER2: u32 = 0x05000F;
    pub const STRAT_ADDR_CRUISER2FIRE: u32 = 0x050010;
    pub const STRAT_ADDR_CRUISER1: u32 = 0x050025;
    pub const STRAT_ADDR_CRUISER1F: u32 = 0x050026;
    pub const STRAT_ADDR_SHIP3A: u32 = 0x050027;
    pub const STRAT_ADDR_SHIP3: u32 = 0x050028;
    pub const STRAT_ADDR_EXITOPENSND2: u32 = 0x050029;

    // ---- path ids (src/path/path_literals.h PATH_ID_*) ----
    pub const PATH_PATRET_IRAB: u16 = 259;
    pub const PATH_PATRET_IFRO: u16 = 260;
    pub const PATH_PATRET_IFAL: u16 = 261;
    pub const PATH_CHASE3_1: u16 = 271;
    pub const PATH_CHASE3_2: u16 = 272;
    pub const PATH_PATRET: u16 = 308;
    pub const PATH_PATCOM: u16 = 339;
    pub const PATH_TOTUMSG: u16 = 340;
}

/// C `build_level1_3_opening_slice()` + `register_level1_3_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    // LEVEL1_3.ASM — Space Armada level wrapper.
    // Opening: initlevel 1_3i,whitefadeout,0
    b.qfadedown();
    b.waitfade();
    b.setbg(lc::BG_1_3I);
    b.initbg();
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.mapcodejsl_builtin(cb::INITFADEWHITE2NORM_L);

    // Line 4: mapjsr cl_warpout
    b.mapjsr("level1_3.cl_warpout");

    // Line 10: mapjsr map1_3a (SPACE section)
    b.mapjsr("level1_3.map1_3a");

    // LEVEL1_3.ASM lines 16-23: SHIP1 bounded section
    // .start1: mapjsr map1_3a1 (ship1 interior)
    b.mapjsr("level1_3.map1_3a1");

    // LEVEL1_3.ASM lines 24-26: setbg 1_3b, initbg, mapjsr map1_3b1 (tunnel)
    // map1_3b1 is incmap 1-3-t1 + mapjsr mtunnelexit; stub for now.
    b.setbg(lc::BG_1_3B);
    b.initbg();
    b.mapwait(500); // placeholder for incmap 1-3-t1 tunnel data
    b.mapwait(100); // placeholder for mtunnelexit

    // LEVEL1_3.ASM lines 34-47: SHIP2 bounded section
    // .start2: mapjsr map1_3a2 (ship2 interior)
    b.mapjsr("level1_3.map1_3a2");

    // setbg 1_3b, initbg, mapjsr map1_3b2 (tunnel)
    b.setbg(lc::BG_1_3B);
    b.initbg();
    b.mapjsr("level1_3.map1_3b2");

    // LEVEL1_3.ASM lines 49-67: .bigship section
    b.setbg(lc::BG_1_3C);
    b.mapwait(100); // maptexitwait -100 placeholder
    b.initbg();
    b.mapjsr("level1_3.map1_3c");

    // .washroom: 8x bou_1 HARD180yr obstacles (C literal `-060` is octal -48).
    b.mapobj(0x0000, 0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x0000, 0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x0000, 0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x0000, 0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapobj(0x1000, -0x0070, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    // incmap washent — WASHENT.ASM: 1-3 Boss Entry Cutscene (colony pipe
    // entrance). mapplayercantdie / mapplayermode / mappipe opcodes are not
    // yet implemented in the map executor; the C oracle skips them too.

    // Lines 10-12: three pipe background objects (mapobjnomem -> mapobj)
    b.mapobj(0, 0, -60, 4200, lc::SH_PIPE_9_0_PROXY, is::NOCOLL);
    b.mapobj(400, 0, -60, 4200, lc::SH_PIPE_9_0_PROXY, is::NOCOLL);
    b.mapobj(0, 0, -60, 4200, lc::SH_PIPE_9_PROXY, lc::IS_COLONYEXIT);
    // Line 13: mapwait 4000
    b.mapwait(4000);

    // mapjsr map1_3d (washing machine room) — stub
    b.mapjsr("level1_3.map1_3d");

    // .fin: mapjsr cl_ship1_3, mapend
    b.mapjsr("cl_ship1_3");
    b.mapend(1);

    // CL_WARPO.ASM:1-7.
    b.label("level1_3.cl_warpout");
    b.mapplayeroutview();
    b.mapcodejsl_builtin(cb::SET_PLAYER_WARP_L);
    b.mapwait(10000);
    b.maprts();

    // MAP1_3A.ASM:2-33.
    b.label("level1_3.map1_3a");
    b.cspecial(1000, 100, SPACE_VIEWCY - 100, 3000, lc::SH_ZACO_7, lc::IS_SZACO5);

    b.map_farships2(-2000, -500, 9000, -30, 8, 2);
    b.map_farships2(-1000, 0, 9000, -10, 20, 4);
    b.map_farships1(1500, -300, 9000, 20, 18, 2);
    b.map_farships0(2000, 0, 9000, 10, 10, 3);
    b.map_farships2(-800, 500, 8000, -20, 16, 2);
    b.map_farships2(-1500, 1200, 8000, -30, 20, 2);
    b.map_farships0(1000, -800, 7700, 10, -8, 2);
    b.map_farships1(0, -1200, 7700, 16, -40, 2);
    b.map_farships2(500, -1000, 7700, 20, -16, 1);

    b.mapobj(2500, -500, -300, 3000, lc::SH_W_L, lc::IS_WINGLAZERMAN);

    b.map_farships2(-2500, -300, 8000, -30, 15, 2);
    b.mapwait(1000);
    b.map_farships2(0, -1200, 8000, 16, -40, 1);
    b.mapwait(1000);
    b.map_farships1(500, -1000, 6000, 30, -20, 2);
    b.mapwait(3000);

    b.map_farships0(500, -500, 6000, 50, -30, 1);

    b.cspecial(1000, 0, SPACE_VIEWCY - 200, 3000, lc::SH_ZACO_7, lc::IS_SZACO5);
    b.cspecial(0, 400, SPACE_VIEWCY + 200, 3000, lc::SH_ZACO_7, lc::IS_SZACO5);
    b.cspecial(3000, -400, SPACE_VIEWCY + 200, 3000, lc::SH_ZACO_7, lc::IS_SZACO5);

    b.mapobj(0, 100, -100, 5000, sh::NULLSHAPE, lc::IS_UP1MAN);
    b.maprts();

    // MAP1_3A1.ASM — ship1 interior subroutine
    append_map1_3a1_submap(&mut b);

    // MAP1_3A2.ASM — ship2 interior subroutine
    append_map1_3a2_submap(&mut b);

    // MAP1_3B2.ASM — ship2 tunnel subroutine
    append_map1_3b2_submap(&mut b);

    // MAP1_3C subroutine — Space Armada part C (big ship interior)
    b.label("level1_3.map1_3c");

    // Lines 4-6: near_side cruiser
    b.mapnobj(0, -1000, SPACE_VIEWCY, 350, lc::SH_SHIP_4_PROXY, lc::STRAT_ADDR_CRUISER1);
    b.setalvarb(al::VEL, 200);
    b.setalvarb(al::ROTZ, 230);

    // Lines 8-11: normal far cruiser
    b.mapnobj(0, -3400, SPACE_VIEWCY + 100, 3000, lc::SH_SHIP_4_PROXY, lc::STRAT_ADDR_CRUISER1F);
    b.setalvarb(al::SBYTE1, 25);
    b.setalvarb(al::VEL, 55);
    b.setalvarb(al::ROTZ, 20);
    b.mapwait(2000);

    // Lines 14-16: far_big_ship
    b.mapnobj(0, 600, SPACE_VIEWCY, 8000, lc::SH_SHIP_0_C_PROXY, lc::STRAT_ADDR_SHIP3A);
    b.setalvarb(al::VEL, 125);
    b.setalvarb(al::ROTX, 10);

    // Line 18: cspecial r_hou_0
    b.cspecial(0, -100, -200, 5000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Lines 20-23: from_top cruiser
    b.mapnobj(0, SPACE_MINX - 2000, SPACE_VIEWCY - 3000, 3000, lc::SH_SHIP_4_PROXY, lc::STRAT_ADDR_CRUISER1);
    b.setalvarb(al::VEL, 100);
    b.setalvarb(al::ROTX, 25);
    b.setalvarb(al::ROTZ, 230);
    b.mapwait(3500);

    // Lines 26-28: reverse cruiser
    b.mapnobj(0, -2500, SPACE_VIEWCY - 100, 4000, lc::SH_SHIP_4_PROXY, lc::STRAT_ADDR_CRUISER1F);
    b.setalvarb(al::VEL, 55);
    b.setalvarb(al::ROTZ, 150);
    b.mapwait(1000);

    // Lines 30-31: cspecial r_hou_0 pair
    b.cspecial(2000, 0x0200, 0x0300, 5000, lc::SH_R_HOU_0, lc::IS_SHOU0A);
    b.cspecial(9000, -0x0200, -0x0200, 5000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Lines 33-34: gate + pathobj e_gate
    b.mapobj(2000, 0, 100, 5000, sh::GATE_0, is::GATE);
    b.pathobj(4000, 3000, 3000, 1000, sh::NULLSHAPE, path::E_GATE, 10, 10);

    // Lines 36-38: pathspecial/pathcspecial escorts
    b.pathspecial(800, 600, 400, -100, lc::SH_S_ZACO_0, lc::PATH_PATRET, 10, 10);
    b.pathcspecial(800, 500, -100, -100, lc::SH_BZACO_8, lc::PATH_PATRET, 10, 10);
    b.pathcspecial(4000, -400, 200, -100, lc::SH_BZACO_8, lc::PATH_PATRET, 10, 10);

    // Line 39: cspecial s_hou_0
    b.cspecial(1000, 0, 0x0200, 4000, lc::SH_S_HOU_0, lc::IS_SHOU0);

    // Lines 46-48: big ship approach (spsdist=13000, sphigh=6000)
    b.mapnobj(0, 0, 6000, 13000, lc::SH_SHIP_0_C_PROXY, lc::STRAT_ADDR_SHIP3);
    b.setvarobj(wm::MAPVAR1);

    // Lines 50-53: bshipexitface door 1 (below)
    b.mapnobj(0, 0, 6000 - 140, 13000 - 240, lc::SH_BSHIPEXITFACE_PROXY, lc::STRAT_ADDR_EXITOPENSND2);
    b.setalvarw(al::SWORD1, 400);
    b.setalvarptrw(al::SWORD2, wm::MAPVAR1);
    b.setalvarb(al::SBYTE1, -10);

    // Lines 56-59: bshipexitface door 2 (above)
    b.mapnobj(0, 0, 6000 + 140, 13000 - 240, lc::SH_BSHIPEXITFACE_PROXY, lc::STRAT_ADDR_EXITOPENSND2);
    b.setalvarw(al::SWORD1, 400);
    b.setalvarptrw(al::SWORD2, wm::MAPVAR1);
    b.setalvarb(al::SBYTE1, 10);

    // Lines 62-71: wait, fade music, boss music
    b.mapwait(4000);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 7);
    b.setbgm(BGM_BOSS1);

    // Lines 75-77: .loop — chkstratdone1 busy-wait
    b.label("level1_3.map1_3c.loop");
    b.mapwait(16);
    let chkstratdone1_loop_ptr = b.mapcode65816_inline();
    b.mapgoto("level1_3.map1_3c.loop");
    b.label("level1_3.map1_3c.cont");

    // Lines 79-80: setbg 1_3b, initbg
    b.setbg(lc::BG_1_3B);
    b.initbg();

    // Line 83: incmap 1-3-t3 — tunnel transition data (stub for now)
    b.mapwait(500);

    b.maprts();

    // MAP1_3D subroutine — Space Armada part D (washing machine boss)
    b.label("level1_3.map1_3d");
    // INCMAP washmape — wash entrance map data (stub)
    b.mapwait(500);
    // markboss boss13
    b.mapcodejsl_builtin(cb::MARKBOSS_L);
    b.maprts();

    // CL_SHIP1_3 — shared clear-demo for ship levels.
    // Append the cl_ship submap so the label resolves.
    append_cl_ship_submap(&mut b);

    b.resolve();

    // C: s_map1_3c_chkstratdone1_end_ptr lookup on "level1_3.map1_3c.cont"
    // (consumed by the callback at runtime; must resolve).
    assert!(
        b.lookup_label("level1_3.map1_3c.cont").is_some(),
        "level1_3 map1_3c cont label missing"
    );

    let (data, labels) = b.finish();

    // C `register_level1_3_inline_callbacks()` — registration-call order.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    if chkstratdone1_loop_ptr != 0 {
        inline_regs.push((chkstratdone1_loop_ptr, "map1_3c_chkstratdone1_check"));
    }

    Route1Level {
        level: BuiltLevel {
            data,
            labels,
            native_callbacks: vec![],
            inline_callbacks: vec![],
        },
        // level1_3 registers no native callbacks in C.
        native_regs: vec![],
        inline_regs,
    }
}

/// C `append_map1_3a1_submap()` — MAP1_3A1.ASM: Space Armada Part A1
/// (Ship 1 interior; SHIP1 bounded section of LEVEL1_3.ASM lines 16-23).
fn append_map1_3a1_submap(b: &mut MapBuilder) {
    b.label("level1_3.map1_3a1");

    // Line 4: mapwait 2500
    b.mapwait(2500);

    // Line 6: ship_1 with setalvar vel,roty,rotx,rotz
    b.mapobj(0, SPACE_MINX + 2000, SPACE_VIEWCY + 600, 9000, lc::SH_SHIP_1_PROXY, lc::STRAT_ADDR_SHIP1A);
    b.setalvarb(al::VEL, 60);
    b.setalvarb(al::ROTY, 115);
    b.setalvarb(al::ROTX, 250);
    b.setalvarb(al::ROTZ, 20);

    // Lines 11-12: pathcspecial escorts
    b.pathcspecial(0x0300, SPACE_MINX + 1000, SPACE_VIEWCY + 400, 8000, lc::SH_ZACO_7, lc::PATH_PATCOM, 10, 10);
    b.pathcspecial(0, SPACE_MINX + 500, SPACE_VIEWCY + 500, 7500, lc::SH_ZACO_7, lc::PATH_PATCOM, 10, 10);

    // Line 13: mapwait 6000
    b.mapwait(6000);

    // Lines 15-17: pathspecial + pathcspecials
    b.pathspecial(0x0600, 0, -600, -100, lc::SH_S_ZACO_0, lc::PATH_PATRET_IFAL, 10, 10);
    b.pathcspecial(0x0600, -500, 100, -100, lc::SH_BZACO_8, lc::PATH_PATRET_IRAB, 10, 10);
    b.pathcspecial(2500, 500, 100, -100, lc::SH_BZACO_8, lc::PATH_PATRET_IFRO, 10, 10);

    // Lines 20-25: second ship_1 with escorts
    b.mapobj(0, SPACE_MAXX - 300, SPACE_VIEWCY + 200, 10000, lc::SH_SHIP_1_PROXY, lc::STRAT_ADDR_SHIP1A);
    b.setalvarb(al::VEL, 50);
    b.setalvarb(al::ROTY, 134);
    b.setalvarb(al::ROTZ, 250);
    b.pathcspecial(0x0300, SPACE_MAXX, SPACE_VIEWCY + 800, 8000, lc::SH_ZACO_7, lc::PATH_PATCOM, 10, 10);
    b.pathcspecial(0, SPACE_MAXX + 200, SPACE_VIEWCY + 700, 7500, lc::SH_ZACO_7, lc::PATH_PATCOM, 10, 10);

    // Line 26: map_farships2
    b.map_farships2(-500, -300, 8000, -16, -25, 2);

    // Line 27: mapwait 8000
    b.mapwait(8000);

    // Line 28: map_farships1
    b.map_farships1(0, -500, 8000, 20, -40, 1);

    // Line 29: mapcspecial (zaco_7 fly out of ship2)
    b.cspecial(0, -350, SPACE_VIEWCY - 300, 4000, lc::SH_ZACO_7, lc::IS_SZACO5);

    // Line 30: mapwait 1000
    b.mapwait(1000);

    // Line 31: map_farships0
    b.map_farships0(500, -1000, 6000, 30, -20, 2);

    // Lines 32-33: pathspecial + pathcspecial
    b.pathspecial(0x0500, -700, -400, -100, lc::SH_S_ZACO_0, lc::PATH_PATRET, 10, 10);
    b.pathcspecial(0x0500, -800, 200, -100, lc::SH_BZACO_8, lc::PATH_PATRET, 10, 10);

    // Line 34: mapwait 1000
    b.mapwait(1000);

    // Line 36: cspecial (zaco_7 fly out of ship2)
    b.cspecial(0, -300, SPACE_VIEWCY - 200, 3400, lc::SH_ZACO_7, lc::IS_SZACO5);

    // Line 37: mapwait 2000
    b.mapwait(2000);

    // Lines 41-47: totumsg + ship_3 + doors (C `#define SPSDIST 6000`)
    const SPSDIST: i32 = 6000;
    b.pathobj(0, 3000, 3000, 3000, sh::NULLSHAPE, lc::PATH_TOTUMSG, 10, 10);
    b.mapobj(0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST, lc::SH_SHIP_3_PROXY, lc::STRAT_ADDR_SHIP2);
    b.setvarobj(wm::MAPVAR1);
    b.mapobj(0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST, lc::SH_S_DOOR_1_PROXY, lc::STRAT_ADDR_SDOOR1);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
    b.mapobj(0, 0x0300, SPACE_VIEWCY - 1500, SPSDIST, lc::SH_S_DOOR_2_PROXY, lc::STRAT_ADDR_SDOOR2);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);

    // Lines 50-54: .loop1 — chkstratdone1/2 check loop
    b.label("level1_3.map1_3a1.loop1");
    b.mapif_builtin(cb::CHKSTRATDONE1, "level1_3.map1_3a1.cont1");
    b.mapif_builtin(cb::CHKSTRATDONE2, "level1_3.map1_3a1");
    b.mapwait(1);
    b.mapgoto("level1_3.map1_3a1.loop1");

    // Line 58: .cont1 — DO TUNNEL
    b.label("level1_3.map1_3a1.cont1");

    // maprts
    b.maprts();
}

/// C `append_map1_3a2_submap()` — MAP1_3A2.ASM: Space Armada Part A2
/// (Ship 2 interior).
fn append_map1_3a2_submap(b: &mut MapBuilder) {
    b.label("level1_3.map1_3a2");

    // Line 3: mapwait 2000
    b.mapwait(2000);

    // Line 4: cspecial r_hou_0
    b.cspecial(0, -250, 300, 4000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Lines 6-7: friend + pathobj (chase3)
    b.pathobj(0, 0, 0x0400, 0, sh::FRIENDSHIP_4, lc::PATH_CHASE3_1, 200, 10);
    b.pathobj(3000, 0, 0x0400, 0, lc::SH_ZACO_B, lc::PATH_CHASE3_2, 10, 10);

    // Lines 9-10: ship_5S cruiser2 with setalvar vel,roty
    b.mapnobj(0, -0x0800, SPACE_VIEWCY - 200, 4000, lc::SH_SHIP_5S_PROXY, lc::STRAT_ADDR_CRUISER2);
    b.setalvarb(al::VEL, 18);

    // Lines 12-14: ship_5S cruiser2 with roty, vel
    b.mapnobj(0, -0x1000, SPACE_VIEWCY + 400, 3000, lc::SH_SHIP_5S_PROXY, lc::STRAT_ADDR_CRUISER2);
    b.setalvarb(al::ROTY, 20);
    b.setalvarb(al::VEL, 20);

    // Lines 16-18: ship_5m cruiser2 with vel, rotx
    b.mapnobj(0, -500, SPACE_VIEWCY + 400, 2000, lc::SH_SHIP_5M_PROXY, lc::STRAT_ADDR_CRUISER2);
    b.setalvarb(al::VEL, 20);
    b.setalvarb(al::ROTX, 240);

    // Line 19: mapwait 3000
    b.mapwait(3000);

    // Line 20: cspecial r_hou_0
    b.cspecial(0, -200, -300, 4000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Line 21: pathspecial s_zaco_0, patret
    b.pathspecial(0, 1500, -600, -100, lc::SH_S_ZACO_0, lc::PATH_PATRET, 10, 10);

    // Lines 22-25: ship_5S cruiser2 with vel, ship_5m with vel,rotx
    b.mapnobj(0, -2500, SPACE_VIEWCY - 100, 3000, lc::SH_SHIP_5S_PROXY, lc::STRAT_ADDR_CRUISER2);
    b.setalvarb(al::VEL, 20);
    b.mapnobj(0, -700, SPACE_VIEWCY - 100, 4000, lc::SH_SHIP_5M_PROXY, lc::STRAT_ADDR_CRUISER2);
    b.setalvarb(al::VEL, 25);
    b.setalvarb(al::ROTX, 15);

    // Lines 28-29: spacepilon + r_hou_0
    b.mapnobj(3000, 0, -100, 2000, lc::SH_SPACEPILON, lc::STRAT_ADDR_SPACEPILON);
    b.mapobj(0, -300, 0x0300, 4000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Lines 31-33: ship_5 cruiser2fire with vel, rotx
    b.mapnobj(0, -1800, SPACE_VIEWCY, 5500, lc::SH_SHIP_5_PROXY, lc::STRAT_ADDR_CRUISER2FIRE);
    b.setalvarb(al::VEL, 40);
    b.setalvarb(al::ROTX, 254);

    // Line 35: gate
    b.mapnobj(2000, -150, 0, 5000, sh::GATE_0, STRAT_ADDR_GATE3);

    // Line 38: mapwait 4000
    b.mapwait(4000);

    // Line 39: cspecial zaco_7 fly out of ship2
    b.cspecial(0, 0, SPACE_VIEWCY + 100, 4000, lc::SH_ZACO_7, lc::IS_SZACO5);
    b.setalvarb(al::ROTZ, 240);

    // Line 41: mapwait 2000
    b.mapwait(2000);

    // Line 43: pathspecial s_zaco_0, patret
    b.pathspecial(0, 2500, -600, -400, lc::SH_S_ZACO_0, lc::PATH_PATRET, 10, 10);

    // Line 46: r_hou_0
    b.mapobj(1000, -250, 0x0100, 6000, lc::SH_R_HOU_0, lc::IS_SHOU0A);

    // Lines 48-55: totumsg + ship_3 + doors (C `#define SPSDIST2 6000`)
    const SPSDIST2: i32 = 6000;
    b.pathobj(0, 3000, 3000, 3000, sh::NULLSHAPE, lc::PATH_TOTUMSG, 10, 10);
    b.mapobj(0, -300, SPACE_VIEWCY - 1500, SPSDIST2, lc::SH_SHIP_3_PROXY, lc::STRAT_ADDR_SHIP2);
    b.setvarobj(wm::MAPVAR1);
    b.mapobj(0, -300, SPACE_VIEWCY - 1500, SPSDIST2, lc::SH_S_DOOR_1_PROXY, lc::STRAT_ADDR_SDOOR1);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
    b.mapobj(0, -300, SPACE_VIEWCY - 1500, SPSDIST2, lc::SH_S_DOOR_2_PROXY, lc::STRAT_ADDR_SDOOR2);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);

    // Lines 58-62: .loop2 — chkstratdone1/2 check loop
    b.label("level1_3.map1_3a2.loop2");
    b.mapif_builtin(cb::CHKSTRATDONE1, "level1_3.map1_3a2.cont2");
    b.mapif_builtin(cb::CHKSTRATDONE2, "level1_3.map1_3a2");
    b.mapwait(1);
    b.mapgoto("level1_3.map1_3a2.loop2");

    // Line 69: .cont2 — DO TUNNEL
    b.label("level1_3.map1_3a2.cont2");
    b.maprts();
}

/// C `append_map1_3b2_submap()` — MAP1_3B2.ASM: Space Armada Part B2
/// (Ship 2 tunnel stub).
fn append_map1_3b2_submap(b: &mut MapBuilder) {
    b.label("level1_3.map1_3b2");

    // incmap 1-3-t2 — tunnel data (stub/placeholder)
    b.mapwait(500);

    // mapjsr mtunnelexit — medium tunnel exit (stub)
    b.mapwait(100);

    b.maprts();
}

/// C `append_cl_ship_submap()` — CL_SHIP.ASM: clear demo for route-1 ship
/// levels (3_4 and 1_3). Two entry points: cl_ship3_4 (colony bg) and
/// cl_ship1_3 (Sship bg); both fall into the shared cl_ship.cont.
// DUPLICATE: consolidate — this submap is shared with other routes in the
// C oracle; each lane transcribes its own copy until the route lanes merge.
fn append_cl_ship_submap(b: &mut MapBuilder) {
    // cl_ship3_4 entry point
    b.label("cl_ship3_4");
    b.setbg(lc::BG_3_4D);
    b.initbg();
    b.mapcodejsl_builtin(cb::SET_PLAYER_CLEAR_SHIP2_L);
    b.setbgm(BGM_FANFARE);
    b.mapobj(0, 0, SPACE_VIEWCY, 0, lc::SH_COLONY_0_PROXY, lc::STRAT_ADDR_SHIP0CDOWN);
    b.setalvarb(al::ROTY, DEG180);
    b.mapgoto("cl_ship.cont");

    // cl_ship1_3 entry point
    b.label("cl_ship1_3");
    b.setbg(lc::BG_1_3E);
    b.initbg();
    b.mapcodejsl_builtin(cb::SET_PLAYER_CLEAR_SHIP2_L);
    b.setbgm(BGM_FANFARE);
    b.mapobj(0, 0, SPACE_VIEWCY, 0, lc::SH_SSHIP_0_C_PROXY, lc::STRAT_ADDR_SHIP0CDOWN);

    // cl_ship_cont shared continuation
    b.label("cl_ship.cont");
    b.mapwait(9000 - CL_GND_FRIENDWAIT);
    b.mapmother(0, 0, 0, 3000, lc::SH_MOTHER1, lc::STRAT_ADDR_MOTHER1, 0);

    b.setvarb(wm::STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_GND_FRIENDWAIT);

    b.mapif_builtin(cb::FROG_ALIVE, "cl_ship.frog_alive");
    b.mapgoto("cl_ship.nf");
    b.label("cl_ship.frog_alive");
    b.mapobj(CL_GND_FRIENDWAIT, -1000, -50, 50, sh::MYSHIP_4, lc::IS_CLSHIPSHIPA);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_FROG);
    b.label("cl_ship.nf");

    b.mapif_builtin(cb::BUNNY_ALIVE, "cl_ship.bunny_alive");
    b.mapgoto("cl_ship.nb");
    b.label("cl_ship.bunny_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 1000, -50, 50, sh::MYSHIP_4, lc::IS_CLSHIPSHIPB);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_BUNNY);
    b.label("cl_ship.nb");

    b.mapif_builtin(cb::COCK_ALIVE, "cl_ship.cock_alive");
    b.mapgoto("cl_ship.nc");
    b.label("cl_ship.cock_alive");
    b.mapobj(CL_GND_FRIENDWAIT, 0, 200, -500, sh::MYSHIP_4, lc::IS_CLSHIPSHIPC);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_COCK);
    b.label("cl_ship.nc");

    b.mapwait(3000);
    b.setvarb(wm::CLB2, 0);
    b.setvarb(wm::STAGECLEAR, 0);
    b.mapcodejsl_builtin(cb::CL_GROUND_PRINTLEVELFIN);
    b.label("cl_ship.sdloop");
    b.mapif_builtin(cb::CHKSTAGEDONE, "cl_ship.sdcont");
    b.mapgoto("cl_ship.sdloop");
    b.label("cl_ship.sdcont");
    b.mapcodejsl_builtin(cb::CL_GROUND_WIPEOUT);
    b.setbgm(BGM_FADEOUT);
    b.mapwait(45 * MEDPSPEED * 2);
    b.maprts();
}
