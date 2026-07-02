//! MAP_ID_1_2 — Asteroid Belt (Level 1, Route 1).
//!
//! C oracle: `src/map/levels.c` `build_level1_2_wrapper_slice()` and
//! `register_level1_2_inline_callbacks()`.
//!
//! ASM sources transcribed (via the C port):
//! - `LEVEL1_2.ASM` — level wrapper (generic init approximation, jsr chain).
//! - `MAP1_2.ASM`   — the asteroid-belt run (`level1_2.map1_2` label) and the
//!   `map12boss` subroutine.
//! - `CL_WARP.ASM`  — warp clear-demo slice (`cl_warp` label; inlined here
//!   exactly like the C build does).

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::*;
use crate::levels::BuiltLevel;

// Constants used by this level that are not yet in consts.rs.
// Values are verbatim from the `#define` blocks in src/map/levels.c.
// TODO(consolidation): move to consts.rs (sh/is/path modules).
mod lc {
    // Shape ids (levels.c SH_* block).
    pub const SH_D_HEAD_0: u16 = 14;
    pub const SH_D_BODY_0: u16 = 15;
    pub const SH_CAMELEON: u16 = 16;
    pub const SH_BOSS_1_2: u16 = 20;
    pub const SH_ZACO_4: u16 = 106;
    pub const SH_B_HOU_0: u16 = 164;
    pub const SH_ASTEROID2: u16 = 195;
    pub const SH_ZACO_B: u16 = 202;
    pub const SH_TADPOLE: u16 = 228;
    pub const SH_ASTEROID1_PROXY: u16 = 275; // SHAPE_EXT_ASTEROID1 (USHAPES.ASM)
    pub const SH_MOTHER1: u16 = 278;

    // Strategy ids (levels.c IS_* block).
    pub const IS_CLSHIPWARPA: u32 = 22;
    pub const IS_CLSHIPWARPB: u32 = 23;
    pub const IS_CLSHIPWARPC: u32 = 24;
    pub const IS_WORMHEAD: u32 = 52;
    pub const IS_WORM: u32 = 61;
    pub const IS_CAMELEON: u32 = 63;
    pub const IS_BOSS1: u32 = 69;
    pub const IS_SZACO0: u32 = 130;
    pub const IS_BLACKHOLE: u32 = 196;
    pub const IS_TADPOLE: u32 = 228;
    pub const IS_BREAK_METEOR: u32 = 235;
    pub const IS_BREAK_METEORT: u32 = 238;

    // Synthetic strategy addresses (levels.c STRAT_ADDR_*).
    pub const STRAT_ADDR_MOTHER1: u32 = 0x020000;
    pub const STRAT_ADDR_SLOWMETEOR: u32 = 0x030003;

    // Path ids (src/path/path_literals.h PATH_ID_*).
    pub const PATH_ID_ASTEMSG: u16 = 241;
    pub const PATH_ID_CHASE1_1: u16 = 247;
    pub const PATH_ID_CHASE1_2: u16 = 248;
    pub const PATH_ID_PYONTA: u16 = 250;
    pub const PATH_ID_CHASE4_1: u16 = 251;
    pub const PATH_ID_CHASE4_2: u16 = 252;
    pub const PATH_ID_CHASE4_3: u16 = 253;
    pub const PATH_ID_INSEKIKUN: u16 = 256;
    pub const PATH_ID_SCREW: u16 = 257;
    pub const PATH_ID_DAMYSCR: u16 = 258;
}

use lc::*;

/// C `build_level1_2_wrapper_slice()` + `register_level1_2_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    // LEVEL1_2.ASM wrapper. Keep the generic level init approximation already
    // used by the C port, then hand off into the map body and clear-demo warp.
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.mapwait(1);
    b.mapwait(1);
    b.mapcodejsl_builtin(cb::SETRESTART_L);
    b.mapjsr("level1_2.map1_2");
    b.mapjsr("cl_warp");
    b.mapend(4);

    // MAP1_2.ASM:7-109 through the cameleon/item/friend block. Mother-map refs
    // are still placeholders until the MOTHERS.ASM submaps are ported.
    b.label("level1_2.map1_2");
    b.mapwait(1000);

    b.cspecial(1800, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    b.pathobj(5000, 3000, 3000, 3000, sh::NULLSHAPE, PATH_ID_ASTEMSG, 10, 10);
    b.cspecial(2000, 1000, SPACE_VIEWCY, 800, SH_ZACO_4, IS_SZACO0);
    b.cspecial(5000, 1000, SPACE_VIEWCY + 1000, 800, SH_ZACO_4, IS_SZACO0);

    b.szaco2_mapobj(0, 2000, 0, 0, 100);
    b.mapwait(500);
    b.szaco2_mapobj(-500, 2000, -300, 100, 0);
    b.mapwait(500);
    b.szaco2_mapobj(-1000, 2000, -400, -100, 0);
    b.mapwait(2000);
    b.szaco2_mapobj(0, 2000, 0, 0, 100);
    b.mapwait(500);
    b.szaco2_mapobj(500, 2000, 300, 100, 100);
    b.mapwait(500);
    b.szaco2_mapobj(1000, 2000, 400, -100, 100);
    b.mapwait(1500);

    b.special(0, -250, SPACE_VIEWCY, 2500, SH_D_HEAD_0, IS_WORMHEAD);
    b.setvarobj(wm::MAPVAR1);
    b.mapwait(150);

    for _ in 0..5u8 {
        b.cspecial(0, -250, SPACE_VIEWCY, 2500, SH_D_BODY_0, IS_WORM);
        b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
        b.setvarobj(wm::MAPVAR1);
        b.mapwait(150);
    }

    b.mapwait(4500);
    b.mapmother(3500, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);

    b.mapobj(2000, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(1000, 0, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.pathobj(0, 1200, 200, 600, sh::FRIENDSHIP_4, PATH_ID_CHASE1_1, 200, 10);
    b.pathcspecial(2000, 1200, 200, 600, SH_ZACO_B, PATH_ID_CHASE1_2, 10, 10);
    b.mapnobj(400, -400, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapobj(200, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(2000, 0, SPACE_VIEWCY - 1000, 800, SH_ZACO_4, IS_SZACO0);
    b.mapnobj(1400, -400, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.cspecial(1200, -200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapnobj(1400, 300, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapobj(2000, -100, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    b.special(0, -128, SPACE_VIEWCY + 128, 2000, SH_D_HEAD_0, IS_WORMHEAD);
    b.setvarobj(wm::MAPVAR1);
    b.mapwait(150);
    for _ in 0..5u8 {
        b.cspecial(0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
        b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
        b.setvarobj(wm::MAPVAR1);
        b.mapwait(150);
    }

    b.mapnobj(1400, -300, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapobj(2000, 100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(0, 200, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.special(2000, -200, SPACE_VIEWCY, 800, SH_CAMELEON, IS_CAMELEON);
    b.mapnobj(400, 300, SPACE_VIEWCY - 300, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.cspecial(0, 0, SPACE_VIEWCY + 200, 800, SH_CAMELEON, IS_CAMELEON);
    b.special(4000, 0, SPACE_VIEWCY - 200, 800, SH_CAMELEON, IS_CAMELEON);

    b.mapmother(3000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.cspecial(4000, -200, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    b.mapremove(SH_MOTHER1);

    b.mapobj(1000, 100, SPACE_VIEWCY + 100, 3000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapmother(4000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);
    b.mapobj(0, 0, SPACE_VIEWCY - 100, 6800, sh::ITEM_5, is::ITEM5);
    b.setalvarb(al::SBYTE1, 1);
    b.pathspecial(1000, 250, SPACE_VIEWCY, 7000, sh::WALKER_2, PATH_ID_PYONTA, 10, 10);
    b.mapnobj(800, -300, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapobj(800, 300, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    b.pathobj(0, 900, -60, 0, sh::FRIENDSHIP_4, PATH_ID_CHASE4_1, 200, 10);
    b.pathcspecial(0, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_2, 200, 10);
    b.pathcspecial(2000, 900, -60, 0, SH_ZACO_B, PATH_ID_CHASE4_3, 200, 10);
    b.mapnobj(200, -400, SPACE_VIEWCY, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapobj(1800, 100, 200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);

    b.skillfly_init();
    b.skillfly_set(0, -50, 4000, 100);
    b.cspecial(0, 180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(0, -180, 100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.cspecial(400, 0, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.mapnobj(300, 200, SPACE_VIEWCY + 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.mapmother(2000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);
    b.mapmother(1300, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);

    let skillfly_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, 100, SPACE_VIEWCY - 100, 1500, sh::ITEM_7, is::ITEM7);
    b.setalvarb(al::SBYTE1, 1);
    b.label("level1_2.map1_2.skillfly_bonus_0_skip");

    b.special(0, -128, SPACE_VIEWCY + 128, 2000, SH_D_HEAD_0, IS_WORMHEAD);
    b.setvarobj(wm::MAPVAR1);
    b.mapwait(150);
    for _ in 0..15u8 {
        b.cspecial(0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
        b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
        b.setvarobj(wm::MAPVAR1);
        b.mapwait(150);
    }
    b.mapobj(0, -128, SPACE_VIEWCY + 128, 2000, SH_D_BODY_0, IS_WORM);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);
    b.setvarobj(wm::MAPVAR1);
    b.mapwait(2500);

    b.cspecial(1000, 200, SPACE_VIEWCY - 500, 3000, SH_TADPOLE, IS_TADPOLE);
    b.skillfly_init();
    b.skillfly_set_default(0, SPACE_VIEWCY - 100, 4000);
    b.pathcspecial(1000, 0, -100, 4000, sh::NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    b.special(1000, 1000, SPACE_VIEWCY + 100, 3000, SH_TADPOLE, IS_TADPOLE);
    b.pathcspecial(400, -200, 200, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    b.mapmother(200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);
    b.pathcspecial(200, 100, -100, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    b.mapmother(200, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);

    b.pathcspecial(2000, 200, -200, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    b.mapobj(2000, -200, -200, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.pathcspecial(1800, 300, -100, 4000, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    b.pathcspecial(400, -300, 0, 4000, SH_B_HOU_0, PATH_ID_SCREW, 10, 10);
    b.mapobj(800, 300, -100, 4000, SH_ASTEROID2, IS_BREAK_METEOR);
    b.skillfly_set_default(0, SPACE_VIEWCY - 100, 4000);
    b.pathcspecial(1000, 0, -100, 4000, sh::NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    b.mapobj(1000, -100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    b.mapmother(1000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.mapremove(SH_MOTHER1);
    b.skillfly_set_default(-200, SPACE_VIEWCY - 100, 4000);
    b.pathcspecial(1000, -200, -100, 4000, sh::NULLSHAPE, PATH_ID_INSEKIKUN, 10, 10);
    b.pathcspecial(1000, 0, -200, 3500, SH_B_HOU_0, PATH_ID_DAMYSCR, 10, 10);
    b.mapobj(1000, -400, -100, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    b.mapobj(1000, 200, 100, 4000, SH_ASTEROID2, IS_BREAK_METEORT);

    let blackhole_bonus_guard_ptr = b.mapcode65816_inline();
    b.mapobj(0, -300, SPACE_VIEWCY + 100, 3000, SH_ASTEROID2, IS_BLACKHOLE);
    b.setalvarb(al::SBYTE1, 1);
    b.label("level1_2.map1_2.blackhole_bonus_skip");
    b.cspecial(1500, -100, 0, 4000, SH_ASTEROID2, IS_BREAK_METEORT);
    b.mapnobj(1200, -100, SPACE_VIEWCY - 200, 4000, SH_ASTEROID1_PROXY, STRAT_ADDR_SLOWMETEOR);
    b.maprts();

    // MAP1_2.ASM:195 map12boss subroutine.
    b.label("level1_2.map12boss");
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);
    b.mapobj(0, 0, SPACE_VIEWCY + 1000, 1500, SH_BOSS_1_2, IS_BOSS1);

    b.mapwait(100);
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("level1_2.map12boss.waitboss.loop");
    b.mapif_builtin(cb::CHKBOSSDEAD, "level1_2.map12boss.waitboss.cont");
    b.mapgoto("level1_2.map12boss.waitboss.loop");
    b.label("level1_2.map12boss.waitboss.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(BGM_FADEOUT);
    b.mapcodejsl_builtin(cb::MARKBOSS_L);
    b.mapwait(1800);
    b.maprts();

    // CL_WARP.ASM clear-demo slice.
    // DUPLICATE: consolidate — this submap is shared with other warp routes
    // in the C build; each C level rebuilds it inline, so we transcribe it
    // inline here too.
    b.label("cl_warp");
    b.mapplayeroutview();
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_FANFARE);
    b.mapcodejsl_builtin(cb::SET_PLAYER_WARP_L);
    b.mapwait(2800);

    b.setvarb(wm::STAGECLEAR, 1);
    b.sendmsg(1);
    b.mapwait(CL_WARP_FRIENDWAIT);

    b.mapif_builtin(cb::FROG_ALIVE, "cl_warp.frog_alive");
    b.mapgoto("cl_warp.nf");
    b.label("cl_warp.frog_alive");
    b.mapobj(CL_WARP_FRIENDWAIT, 300, -60, 50, sh::MYSHIP_4, IS_CLSHIPWARPB);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_FROG);
    b.label("cl_warp.nf");

    b.mapif_builtin(cb::BUNNY_ALIVE, "cl_warp.bunny_alive");
    b.mapgoto("cl_warp.nb");
    b.label("cl_warp.bunny_alive");
    b.mapobj(CL_WARP_FRIENDWAIT, -300, -60, 50, sh::MYSHIP_4, IS_CLSHIPWARPA);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_BUNNY);
    b.label("cl_warp.nb");

    b.mapif_builtin(cb::COCK_ALIVE, "cl_warp.cock_alive");
    b.mapgoto("cl_warp.nc");
    b.label("cl_warp.cock_alive");
    b.mapobj(CL_WARP_FRIENDWAIT, 0, -100, -3000, sh::MYSHIP_4, IS_CLSHIPWARPC);
    b.mapcodejsl_builtin(cb::CLFRIENDMSG_COCK);
    b.label("cl_warp.nc");

    b.mapwait(500);

    // `mother_1` from MOTHERS.ASM and `mother1_istrat` are still unported.
    // Keep the wrapper structure literal, but use a bounded placeholder map ref.
    b.mapmother(10000, 0, 0, 4000, SH_MOTHER1, STRAT_ADDR_MOTHER1, 0);
    b.setvarb(wm::CLB2, 0);
    b.setvarb(wm::STAGECLEAR, 0);
    b.mapcodejsl_builtin(cb::CL_WARP_PRINTLEVELFIN);
    b.label("cl_warp.eswait");
    b.mapwait(1);
    b.maploop("cl_warp.eswait", 100);
    b.setvarb(wm::CLB2, 2);
    b.setvarb(wm::ONECREDSPR, 0);
    b.mapwait(2000);
    b.mapremove(SH_MOTHER1);
    b.mapwait(9000);
    b.setvarb(wm::CLB2, 1);
    b.maprts();

    b.resolve();

    // C: mb_lookup_label guards for the bonus skip labels (must exist).
    assert!(
        b.lookup_label("level1_2.map1_2.skillfly_bonus_0_skip").is_some(),
        "level1_2 skillfly bonus skip label missing"
    );
    assert!(
        b.lookup_label("level1_2.map1_2.blackhole_bonus_skip").is_some(),
        "level1_2 blackhole bonus skip label missing"
    );

    let (data, labels) = b.finish();

    // C `register_level1_2_inline_callbacks()` — registration-call order:
    // the native CL_WARP_PRINTLEVELFIN registration is UNCONDITIONAL, then
    // the five inline registrations are each guarded by ptr != 0.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    for (ptr, name) in [
        (skillfly_bonus_guard_ptr, "level1_2_skillfly_bonus_guard"),
        (blackhole_bonus_guard_ptr, "level1_2_blackhole_bonus_guard"),
        (mapwaitboss_trigse_ptr, "level1_1_mapwaitboss_trigse"),
        (mapwaitboss_cantdie_ptr, "level1_1_mapwaitboss_cantdie"),
        (mapwaitboss_cleanup_ptr, "level1_1_mapwaitboss_cleanup"),
    ] {
        if ptr != 0 {
            inline_regs.push((ptr, name));
        }
    }

    Route1Level {
        // Typed callback lists intentionally empty; see the consolidation
        // TODO in route1/mod.rs.
        level: BuiltLevel {
            data,
            labels,
            native_callbacks: vec![],
            inline_callbacks: vec![],
        },
        native_regs: vec![(cb::CL_WARP_PRINTLEVELFIN, "level1_1_cl_ground_printlevelfin")],
        inline_regs,
    }
}
