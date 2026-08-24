//! MAP_ID_TITLE / MAP_ID_CONTINUE / MAP_ID_WAIT — title screen maps.
//!
//! C oracle: `src/map/levels.c` `build_title_slice()` +
//! `register_title_inline_callbacks()`.
//!
//! ASM source transcribed (via the C port): `TITLE.ASM` — three entry
//! points in one blob: `titlemap` (offset 0), `contmap` (label
//! `title.contmap`) and `waitmap` (label `title.waitmap`).

use super::{BuiltLevel, InlineCallback};
use crate::builder::MapBuilder;
use crate::consts::*;

/// Authored title blackout hold written after `initblack_l`.
pub const TITLE_BLACK_HOLD_STEPS: i32 = 10;
/// Authored continue-screen blackout hold written after `initblack_l`.
pub const CONTINUE_BLACK_HOLD_STEPS: i32 = 3;
/// `mapobj ... 70` selects the source `mapqobj` record. Its depth operand is
/// stored in 16-unit steps, so the retail object is born at 64 rather than at
/// the unquantized authored value.
const TITLE_DEMO_DEPTH: i32 = 64;

/// C `build_title_slice()` + `register_title_inline_callbacks()`.
///
/// The continue and wait maps are alternate entry points into this same
/// blob; use [`BuiltLevel::label_offset`] with `"title.contmap"` /
/// `"title.waitmap"` (C `s_contmap_ptr` / `s_waitmap_ptr`).
pub fn build() -> BuiltLevel {
    let mut b = MapBuilder::new();

    // ---- titlemap ----
    // Lines 2-3: setfadedown quick / mapwaitfade
    b.qfadedown();
    b.waitfade();
    // Lines 5-6: setbg title / initbg
    b.setbg(BG_TITLE);
    b.initbg();
    // bg_title_1 in BGS.ASM: bgm title — load title music
    b.setbgm(2); // SND_TITLE

    // Lines 8-16: timeovercont — start_65816 block: clear pos
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.setvarb(wm::STAYBLACK, TITLE_BLACK_HOLD_STEPS);

    // Line 22: mapwait 800
    b.mapwait(800);

    // Line 23: mapobj my_demo tit_istrat
    b.mapnobj(
        0,
        20,
        20,
        TITLE_DEMO_DEPTH,
        sh::MY_DEMO_PROXY,
        STRAT_ADDR_TIT,
    );

    // Line 25: setfadeup quick
    b.qfadeup();

    // Lines 29-36: start_65816 block — disable wobble, set noctrl+nofire
    let title_init_ptr = b.mapcode65816_inline();

    // .ll: infinite wait
    b.label("title.ll");
    b.mapwait(4000);
    b.mapgoto("title.ll");

    // ---- contmap ----
    b.label("title.contmap");
    b.qfadedown();
    b.waitfade();
    b.setbg(BG_CONT);
    b.initbg();

    // Lines 48-57: start_65816 block — clear pos, disable wobble
    b.mapcodejsl_builtin(cb::INITBLACK_L);

    // Line 60: mapwait medpspeed*4
    b.mapwait(MEDPSPEED * 4);

    // Lines 62-63: initblack + stayblack
    b.mapcodejsl_builtin(cb::INITBLACK_L);
    b.setvarb(wm::STAYBLACK, CONTINUE_BLACK_HOLD_STEPS);

    // Line 64: setfadeup quick
    b.qfadeup();

    // Lines 65-66: .ll infinite wait
    b.label("title.cont.ll");
    b.mapwait(4000);
    b.mapgoto("title.cont.ll");

    // ---- waitmap ----
    b.label("title.waitmap");
    b.label("title.wait.ll");
    b.mapwait(4000);
    b.mapgoto("title.wait.ll");

    b.resolve();

    let (data, labels) = b.finish();

    // C `register_title_inline_callbacks()`: title_init is always emitted;
    // contmap_init (`s_contmap_init_script_ptr`) is never assigned by the C
    // build, so `contmap_init_inline` is never registered. Kept here as a
    // note: InlineCallback::ContmapInit exists for when the contmap block
    // grows its own start_65816 hook.
    let mut inline_callbacks: Vec<(u16, InlineCallback)> = Vec::new();
    if title_init_ptr != 0 {
        inline_callbacks.push((title_init_ptr, InlineCallback::TitleInit));
    }

    BuiltLevel {
        data,
        labels,
        native_callbacks: Vec::new(),
        inline_callbacks,
    }
}
