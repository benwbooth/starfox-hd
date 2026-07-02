//! MAP_ID_CREDITS — staff roll (CREDITS.ASM).
//!
//! C oracle: `src/map/levels.c` `build_credits_slice()` +
//! `register_credits_inline_callbacks()`.

use super::rc::*;
use super::Route2Level;
use crate::builder::MapBuilder;

/// C `build_credits_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.qfadedown();
    b.waitfade();

    // Lines 6-7: meters_off trans (runtime only), setbg cred, initbg
    b.setbg(BG_CRED);
    b.initbg();

    // Lines 10-17: start_65816 block — clear player Z, clear viewpos
    // Approximated as inline callback that disables wobble + controls.
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);

    // Lines 19-20: mapcode_jsl initblack_l, setvar.b stayblack,10
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.setvarb(WM_GSVAR_BYTE1, 10);

    // Lines 26-27: setfadeup quick, mapwait 200
    b.qfadeup();
    b.mapwait(200);

    // Lines 28-38: start_65816 block — disable wobble, set noctrl+nofire
    let credits_init_ptr = b.mapcode65816_inline();

    // Line 40: mapjsr actualcreds
    b.mapjsr("credits.actualcreds");

    // Lines 42-47: THE END letter pathobjs
    b.pathobj(0, 972, -969, 1000, SH_FONT_T2_PROXY, PATH_ID_THEENDT, 6, 4);
    b.pathobj(0, -1120, 1377, 1000, SH_FONT_H2_PROXY, PATH_ID_THEENDH, 6, 4);
    b.pathobj(0, -1019, -1530, 1000, SH_FONT_E2_PROXY, PATH_ID_THEENDE, 6, 4);
    b.pathobj(0, 1070, -1326, 1000, SH_FONT_E3_PROXY, PATH_ID_THEENDE2, 6, 4);
    b.pathobj(0, 1550 + 29, 1323 + 54, 1000, SH_FONT_N2_PROXY, PATH_ID_THEENDN, 6, 4);
    b.pathobj(0, -1050 + 129, 1428, 1000, SH_FONT_D2_PROXY, PATH_ID_THEENDD, 6, 4);

    // Line 48: mapwait 6000
    b.mapwait(6000);

    // Line 49: setvar.b levelfinished,le_endofcreds
    // le_endofcreds is a level-finished sentinel value.
    // Use 2 as a proxy (normal level finished is 1).
    b.setvarb(WM_LEVELFINISHED, 2);

    // Lines 50-53: .lp infinite wait (IFEQ EXITCREDITS path — credits loop)
    b.label("credits.lp");
    b.mapwait(5000);
    b.mapgoto("credits.lp");

    // ---- actualcreds subroutine ----
    b.label("credits.actualcreds");

    // Line 85: mapjsr cutcreds
    b.mapjsr("credits.cutcreds");

    // Lines 86-131: textpath credit blocks — stubbed as timed waits.
    // credwait = 5000 (NTSC default)
    b.mapwait(5000);  // cutcreds wait
    // superfxstaff + names
    b.mapwait(5000);
    // software + names
    b.mapwait(5000);
    // english + names
    b.mapwait(5000);
    // japanese + names
    b.mapwait(5000);

    // Line 131: mapwait 9000-31*medpspeed
    b.mapwait(9000 - 31 * MEDPSPEED);
    b.maprts();

    // ---- cutcreds subroutine ----
    b.label("credits.cutcreds");

    // Line 136: mapwait 2000
    b.mapwait(2000);

    // Lines 137-140: Star Fox / presented by / Nintendo pathobjs
    b.pathobj(1200, 0, -1500, 3500, SH_NULLSHAPE, PATH_ID_DSTARFOX, 10, 10);
    // textpath stubs — not yet implemented
    b.mapwait(0);  // placeholder for presented/by textpaths
    b.pathobj(1200, 0, 1500, 3500, SH_NULLSHAPE, PATH_ID_DNINTENDO, 10, 10);

    // Lines 142-191: executive through developed by — textpath credit blocks
    b.mapwait(3000);  // executive + yamauchi
    b.mapwait(5000);  // producer + miyamoto
    b.mapwait(5000);  // director + eguchi
    b.mapwait(5000);  // assistant director + yamada
    b.mapwait(5000);  // programmed by + dylan/giles/krister
    b.mapwait(5000);  // 3d system + pete/carl
    b.mapwait(5000);  // graphic designer + imamura
    b.mapwait(5000);  // shape designer + watanabe
    b.mapwait(5000);  // effects + kondo
    b.mapwait(5000);  // composer + hirasawa
    b.mapwait(5000);  // developed by + argonaut

    b.maprts();

    b.resolve();
    let (data, labels) = b.finish();

    // C `register_credits_inline_callbacks()`.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![(credits_init_ptr, "credits_init_inline")],
    )
}
