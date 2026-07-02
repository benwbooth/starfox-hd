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
use crate::consts::*;
use crate::levels::BuiltLevel;

// Local constants from levels.c not yet in consts.rs.
// TODO(consolidation): move to consts.rs
mod lc {
    use crate::consts::sh;

    /// levels.c `#define SH_OLD_TYPE_PROXY SH_MYSHIP_4`
    pub const SH_OLD_TYPE_PROXY: u16 = sh::MYSHIP_4;
    /// levels.c `#define SH_DEBOSS_1_PROXY SH_NULLSHAPE`
    pub const SH_DEBOSS_1_PROXY: u16 = sh::NULLSHAPE;

    /// levels.c `#define STRAT_ADDR_PLAYERDOWNINTRO 0x050019u`
    pub const STRAT_ADDR_PLAYERDOWNINTRO: u32 = 0x050019;
    /// levels.c `#define STRAT_ADDR_PLAYERDOWN2INTRO 0x05001Au`
    pub const STRAT_ADDR_PLAYERDOWN2INTRO: u32 = 0x05001A;
    /// levels.c `#define STRAT_ADDR_PLAYERDOWN3INTRO 0x05001Bu`
    pub const STRAT_ADDR_PLAYERDOWN3INTRO: u32 = 0x05001B;
    /// levels.c `#define STRAT_ADDR_PLAYERFIREINTRO 0x05001Cu`
    pub const STRAT_ADDR_PLAYERFIREINTRO: u32 = 0x05001C;
    /// levels.c `#define STRAT_ADDR_BOSS7INTRO 0x05001Du`
    pub const STRAT_ADDR_BOSS7INTRO: u32 = 0x05001D;
    /// levels.c `#define STRAT_ADDR_ZACOINTRO 0x05001Eu`
    pub const STRAT_ADDR_ZACOINTRO: u32 = 0x05001E;
    /// levels.c `#define STRAT_ADDR_ZACO2INTRO 0x05001Fu`
    pub const STRAT_ADDR_ZACO2INTRO: u32 = 0x05001F;
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

    // Lines 7-14: start_65816 block — clear position, disable wobble
    b.mapcodejsl_builtin(cb::INITBLACK_L);

    // Line 16-17: mapcode_jsl initblack_l / setvar.b stayblack,10
    b.setvarb(wm::GSVAR_BYTE1, 10); // stayblack proxy

    // Line 19: setfadeup quick
    b.qfadeup();

    // Line 21: mapwait 800 (originally "mapwait 246 800")
    b.mapwait(800);

    // Lines 23-33: start_65816 block — disable wobble, set noctrl+nofire
    let intro_init_ptr = b.mapcode65816_inline();

    // Lines 36-37: textpath nintendo/presents — text rendering (stub)
    // TODO: implement textpath for NINTENDO PRESENTS text when text renderer is ported
    b.mapwait(2000);

    // Lines 41-47: player intro ships
    b.mapnobj(0x1000, 50, -400, -700, lc::SH_OLD_TYPE_PROXY, lc::STRAT_ADDR_PLAYERDOWN2INTRO);
    b.mapnobj(0x1000, 50, -400, -700, lc::SH_OLD_TYPE_PROXY, lc::STRAT_ADDR_PLAYERDOWN3INTRO);
    b.mapnobj(MEDPSPEED * 5, 50, -400, -700, lc::SH_OLD_TYPE_PROXY, lc::STRAT_ADDR_PLAYERDOWNINTRO);
    b.setvarobj(wm::MAPVAR1);
    b.mapnobj(0, 0, -400, -700, sh::NULLSHAPE, lc::STRAT_ADDR_PLAYERFIREINTRO);
    b.setalvarptrw(al::SWORD1, wm::MAPVAR1);

    // Line 50: mapwait 2000
    b.mapwait(2000);

    // Line 52: deboss_1 boss7intro
    b.mapnobj(0, 0, -800, -400, lc::SH_DEBOSS_1_PROXY, lc::STRAT_ADDR_BOSS7INTRO);

    // Line 54: mapwait 8000
    b.mapwait(8000);

    // Lines 55-63: zaco waves
    b.mapnobj(600, -400, -800, 2000, sh::ZACO_A, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(600, 400, -800, 2000, sh::ZACO_A, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);

    // Line 65: zaco2intro
    b.mapnobj(400, 0, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACO2INTRO);

    // Lines 67-70: more zacos
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, -400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);
    b.mapnobj(400, 400, -800, 2000, sh::ZACO_5, lc::STRAT_ADDR_ZACOINTRO);

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
