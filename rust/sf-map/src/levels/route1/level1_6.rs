//! MAP_ID_1_6 — Venom 1 Surface (Route 1 final level).
//!
//! C oracle: `src/map/levels.c` `build_level1_6_slice()`,
//! `append_finalmap_content()` (shared with level3_7 and final) and
//! `register_level1_6_inline_callbacks()`.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_6.ASM` — level wrapper: `initlevel 1_6a,0`, jsr map1_6a,
//!   then `incmap finalmap`.
//! - `FINALMAP.ASM` — Andross final tunnel & boss (`level1_6.final.*`).
//! - `MAP1_6A.ASM`  — Venom 1 surface content (stubbed in the C build:
//!   mapwait 2000 / maprts; to be ported in a future batch).

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

// Local constants from levels.c not yet in consts.rs.
// TODO(consolidation): move to consts.rs
mod lc {
    /// levels.c `#define BG_2_6C 29u`
    pub const BG_2_6C: i32 = 29;
    /// levels.c `#define BG_1_6C 17u`
    pub const BG_1_6C: i32 = 17;
    /// levels.c `#define BGM_BOSS_FINAL 0x13u`
    pub const BGM_BOSS_FINAL: i32 = 0x13;
    /// levels.c `#define BGM_FINAL_CONT 0x12u`
    pub const BGM_FINAL_CONT: i32 = 0x12;

    /// levels.c `#define SH_TUNNEL_0 122` (def_shape tunnel_0)
    pub const SH_TUNNEL_0: u16 = 122;
    /// levels.c `#define SH_WALL_2 89` (def_shape wall_2)
    pub const SH_WALL_2: u16 = 89;
    /// levels.c `#define SH_BOU_1_PROXY SH_NULLSHAPE`
    pub const SH_BOU_1_PROXY: u16 = 0;
    /// levels.c `#define SH_WALL_4_PROXY SH_NULLSHAPE`
    pub const SH_WALL_4_PROXY: u16 = 0;
    /// levels.c `#define SH_FACE_B_PROXY SH_NULLSHAPE`
    pub const SH_FACE_B_PROXY: u16 = 0;

    /// levels.c `#define STRAT_ADDR_TOPRIGHT1 0x050011u`
    pub const STRAT_ADDR_TOPRIGHT1: u32 = 0x050011;
    /// levels.c `#define STRAT_ADDR_TOPLEFT1 0x050012u`
    pub const STRAT_ADDR_TOPLEFT1: u32 = 0x050012;
    /// levels.c `#define STRAT_ADDR_BOTRIGHT1 0x050013u`
    pub const STRAT_ADDR_BOTRIGHT1: u32 = 0x050013;
    /// levels.c `#define STRAT_ADDR_BOTLEFT1 0x050014u`
    pub const STRAT_ADDR_BOTLEFT1: u32 = 0x050014;
    /// levels.c `#define STRAT_ADDR_MONOLITH 0x050015u`
    pub const STRAT_ADDR_MONOLITH: u32 = 0x050015;

    /// path_literals.h `PATH_ID_MES_ANDROSS1 = 342`
    pub const PATH_ID_MES_ANDROSS1: u16 = 342;
    /// path_literals.h `PATH_ID_MES_ANDROSS2 = 343`
    pub const PATH_ID_MES_ANDROSS2: u16 = 343;
}

/// C `build_level1_6_slice()` + `register_level1_6_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    let mut mapwaitboss_cantdie_ptr: u16 = 0;
    let mut mapwaitboss_cleanup_ptr: u16 = 0;

    // LEVEL1_6.ASM: initlevel 1_6a,0
    // mapjsr map1_6a — Route 1 Venom surface content (MAP1_6A.ASM)
    b.mapjsr("level1_6.map1_6a");

    // level1_end: incmap finalmap — Andross final tunnel & boss
    append_finalmap_content(
        &mut b,
        "level1_6.final",
        &mut mapwaitboss_cantdie_ptr,
        &mut mapwaitboss_cleanup_ptr,
    );

    // ---- MAP1_6A.ASM subroutine (stub — content not yet ported) ----
    b.label("level1_6.map1_6a");
    // MAP1_6A.ASM is 304 lines of Venom 1 surface content.
    // Stub: mapwait 2000 / maprts (to be ported in a future batch).
    b.mapwait(2000);
    b.maprts();

    b.resolve();

    let (data, labels) = b.finish();

    // C `register_level1_6_inline_callbacks()` — registration-call order,
    // guarded by ptr != 0 like C.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    for (ptr, name) in [
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
        native_regs: vec![],
        inline_regs,
    }
}

// DUPLICATE: consolidate — literal copy of the C `append_finalmap_content()`
// (levels.c), shared with build_level3_7_slice and build_final_slice. The
// `prefix` disambiguates labels across maps. `cantdie_ptr` and `cleanup_ptr`
// receive the inline callback offsets.
fn append_finalmap_content(
    b: &mut MapBuilder,
    prefix: &str,
    cantdie_ptr: &mut u16,
    cleanup_ptr: &mut u16,
) {
    // incmap dm_lb1 — stub for level base 1 demo intro
    b.mapwait(500);

    // final_tunnel entry point
    b.label(&format!("{prefix}.tunnel"));
    b.mapwait(2000);

    // set BG to 2_6c
    b.setbg(lc::BG_2_6C);

    // setrestart finalmap_restart
    b.mapcodejsl_builtin(cb::SETRESTART_L);

    // finalmap_cont entry point
    b.label(&format!("{prefix}.cont"));
    b.mapplayeroutview();
    b.mapwait(2000);

    // pathobj mes_andross1 message
    b.pathobj(0, 0, 0, 4000, sh::NULLSHAPE, lc::PATH_ID_MES_ANDROSS1, 10, 10);

    // .finalt: tunnel sections (4 iterations)
    let finalt = format!("{prefix}.finalt");
    b.label(&finalt);
    b.mapnobj(0, 0x0120, -120, 4000, lc::SH_TUNNEL_0, lc::STRAT_ADDR_TOPRIGHT1);
    b.mapnobj(0, -0x0120, -120, 4000, lc::SH_TUNNEL_0, lc::STRAT_ADDR_TOPLEFT1);
    b.mapnobj(0, 0x0120, 0, 4000, lc::SH_TUNNEL_0, lc::STRAT_ADDR_BOTRIGHT1);
    b.mapnobj(0x0600, -0x0120, 0, 4000, lc::SH_TUNNEL_0, lc::STRAT_ADDR_BOTLEFT1);
    b.maploop(&finalt, 4);

    // wall/gate obstacles (C `-060` is octal == -48)
    b.mapobj(0, -0x0090, -0o60, 4000, lc::SH_WALL_2, is::HARD180YR);
    b.mapobj(0x0500, 0x0090, -0o60, 4000, lc::SH_WALL_2, is::HARD180YR);
    b.mapnobj(0, 0, -60, 4000, sh::GATE_0, STRAT_ADDR_GATE3);
    b.setalvarb(al::SBYTE1, 1);
    b.mapwait(1000);

    // pillar pairs
    b.mapnobj(0, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0800, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0800, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);

    // item_5 + walls + pillars
    b.mapnobj(0, 0, -60, 4000, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, -0x0090, -0o60, 4000, lc::SH_WALL_2, is::HARD180YR);
    b.mapobj(0x1000, 0x0090, -0o60, 4000, lc::SH_WALL_2, is::HARD180YR);
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0800, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0800, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);

    // level 3 conditional pillar section (emitted unconditionally)
    b.mapnobj(0x0600, 0, -60, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0600, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0200, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0200, 0, -40, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0600, 0, -60, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0200, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0200, 0, -80, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0600, 0, -60, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x0600, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);

    // common pillar section
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -100, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapnobj(0x1500, 0, -20, 4000, sh::PILLAR3, is::PILLAR3);

    // level 3 half-door section
    let level3t = format!("{prefix}.level3t");
    b.label(&level3t);
    b.mapobj(0, 100, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapnobj(0x1500, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(0, -100, -0o60, 4000, lc::SH_BOU_1_PROXY, is::HARD180YR);
    b.mapnobj(0x1500, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.maploop(&level3t, 2);
    b.mapwait(500);

    // level 1 half-door section
    let level1t = format!("{prefix}.level1t");
    b.label(&level1t);
    b.mapobj(0, 110, -0o60, 4000, lc::SH_WALL_4_PROXY, is::HARD180YR);
    b.mapnobj(0x1500, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(0, -110, -0o60, 4000, lc::SH_WALL_4_PROXY, is::HARD180YR);
    b.mapnobj(0x1500, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.maploop(&level1t, 2);
    b.mapwait(500);

    // final corridor: item_7 + wall_4 + halfdL/R
    b.mapnobj(0, 0x0060, -60, 4000, sh::ITEM_7, is::ITEM7);
    b.setalvarb(al::SBYTE1, 1);
    b.mapobj(0, 110, -0o60, 4000, lc::SH_WALL_4_PROXY, is::HARD180YR);
    b.mapnobj(0x2000, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);
    b.mapobj(0, -110, -0o60, 4000, lc::SH_WALL_4_PROXY, is::HARD180YR);
    b.mapnobj(0x1000, 0, 0, 4000, sh::PILLAR3, is::PILLAR3);

    // tunnel exit
    b.mapwait(2000);
    b.pathobj(0, 0, 0, 3000, sh::NULLSHAPE, lc::PATH_ID_MES_ANDROSS2, 10, 10);
    b.mapwait(200);

    // BG transition
    b.mapwait(100);
    b.setbg(lc::BG_1_6C);
    b.initbg();
    b.mapwait(200);

    // boss final music
    b.setbgm(lc::BGM_BOSS_FINAL);
    b.mapwait(2000);

    // face_b monolith boss
    b.mapnobj(0x1000, 0, SPACE_VIEWCY, -200, lc::SH_FACE_B_PROXY, lc::STRAT_ADDR_MONOLITH);

    // mapwaitboss nosound
    b.mapwait(100);
    let bosswait_loop = format!("{prefix}.bosswait.loop");
    b.label(&bosswait_loop);
    {
        let contlabel = format!("{prefix}.bosswait.cont");
        b.mapif_builtin(cb::CHKBOSSDEAD, &contlabel);
        b.mapgoto(&bosswait_loop);
        b.label(&contlabel);
    }
    *cantdie_ptr = b.mapcode65816_inline();
    *cleanup_ptr = b.mapcode65816_inline();

    // markboss bossfinal
    b.mapcodejsl_builtin(cb::MARKBOSS_L);
    b.mapwait(5000);

    // finalmap_end: incmap dm_end — end demo (stub)
    b.mapwait(500);

    // .wait1: infinite wait
    let wait1 = format!("{prefix}.wait1");
    b.label(&wait1);
    b.mapwait(1000);
    b.mapgoto(&wait1);

    // finalmap_restart: setbgm $12, goto finalmap_cont
    b.label(&format!("{prefix}.restart"));
    b.mapwait(1000);
    b.setbgm(lc::BGM_FINAL_CONT);
    {
        let contlabel = format!("{prefix}.cont");
        b.mapgoto(&contlabel);
    }
}
