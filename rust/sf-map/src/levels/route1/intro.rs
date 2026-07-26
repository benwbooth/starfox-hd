//! MAP_ID_INTRO — the attract-mode intro demo.
//!
//! C oracle: `src/map/levels.c` `build_intro_slice()` and
//! `register_intro_inline_callbacks()`.
//!
//! ASM source transcribed (via the C port): `INTRO.ASM` — Nintendo
//! Presents fade-in, player intro ships, deboss_1 intro boss, zaco waves,
//! infinite `.lp` wait.

use super::Route1Level;
use crate::builder::MapBuilder;
use crate::consts::intro_strategy_address as intro_addr;
use crate::consts::*;
use crate::levels::BuiltLevel;

// Local constants from levels.c not yet in consts.rs.
// TODO(consolidation): move to consts.rs
mod lc {
    pub const SH_OLD_TYPE: u16 = 323;
    pub const SH_DEBOSS_1: u16 = 312;
    pub const WING_CRAFT_ENTRY_DISTANCE: i32 = 4096;

    /// levels.c `#define STRAT_ADDR_BOSS7INTRO 0x05001Du`
    pub const STRAT_ADDR_BOSS7INTRO: u32 = crate::consts::is::BOSS7INTRO;
}

/// C `build_intro_slice()` + `register_intro_inline_callbacks()`.
pub fn build() -> Route1Level {
    let mut b = MapBuilder::new();

    // Lines 2-3: setfadedown quick / mapwaitfade
    b.qfadedown();
    b.waitfade();

    // Line 4-5: setbg intro / initbg
    b.setbg(BG_INTRO);
    b.initbg();

    // Lines 7-14: start_65816 — reset lastplayz, pviewposz, and player Z.
    let reset_view_ptr = b.mapcode65816_inline();

    // Lines 16-17: mapcode_jsl initblack_l / setvar.b stayblack,10.
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.setvarb(wm::STAYBLACK, 10);

    // Line 19: setfadeup quick
    b.qfadeup();

    // Line 21: literal source is `mapwait 246 800`; MAPMACS consumes its
    // first argument and encodes WAIT2(246 >> 4), an effective distance 240.
    b.mapwait(246);

    // Lines 23-33: start_65816 block — disable wobble, set noctrl+nofire
    let intro_init_ptr = b.mapcode65816_inline();

    // Lines 36-37: exact Nintendo Presents scaled-text paths.
    b.textpath(0, -3000, -100, 4000, msg::NINTENDO, path::DINTRO1, 14, None);
    b.textpath(
        0,
        3000,
        100,
        4000,
        msg::PRESENTS,
        path::DINTRO1,
        14,
        Some(-32),
    );

    // Lines 41-47: player intro ships
    b.mapnobj(
        lc::WING_CRAFT_ENTRY_DISTANCE,
        50,
        -400,
        -700,
        lc::SH_OLD_TYPE,
        intro_addr::PLAYER_DOWN_LEFT,
    );
    b.mapnobj(
        lc::WING_CRAFT_ENTRY_DISTANCE,
        50,
        -400,
        -700,
        lc::SH_OLD_TYPE,
        intro_addr::PLAYER_DOWN_RIGHT,
    );
    b.mapnobj(
        MEDPSPEED * 5,
        50,
        -400,
        -700,
        lc::SH_OLD_TYPE,
        intro_addr::PLAYER_DOWN,
    );
    b.setvarobj(wm::MAPVAR1);
    b.mapnobj(0, 0, -400, -700, sh::NULLSHAPE, intro_addr::PLAYER_FIRE);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);

    // Line 50: mapwait 2000
    b.mapwait(2000);

    // Line 52: deboss_1 boss7intro
    b.mapnobj(0, 0, -800, -400, lc::SH_DEBOSS_1, lc::STRAT_ADDR_BOSS7INTRO);

    // Line 54: mapwait 8000
    b.mapwait(8000);

    // Lines 55-63: zaco waves
    b.mapnobj(600, -400, -800, 2000, sh::ZACO_A, intro_addr::ZACO);
    b.mapnobj(600, 400, -800, 2000, sh::ZACO_A, intro_addr::ZACO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);

    // Line 65: zaco2intro
    b.mapnobj(400, 0, -800, 2000, sh::ZACO_5, intro_addr::ZACO_LEADER);

    // Lines 67-70: more zacos
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, intro_addr::ZACO);

    // .lp: infinite wait
    b.label("intro.lp");
    b.mapwait(5000);
    b.mapgoto("intro.lp");

    b.resolve();

    let (data, labels) = b.finish();

    // C `register_intro_inline_callbacks()` — guarded by ptr != 0 like C.
    let mut inline_regs: Vec<(u16, &'static str)> = Vec::new();
    if intro_init_ptr != 0 {
        inline_regs.push((intro_init_ptr, "intro_init_inline"));
    }
    if reset_view_ptr != 0 {
        inline_regs.insert(0, (reset_view_ptr, "intro_reset_view_inline"));
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
