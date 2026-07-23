//! MAP_ID_CREDITS — staff roll (CREDITS.ASM).
//!
//! C oracle: `src/map/levels.c` `build_credits_slice()` +
//! `register_credits_inline_callbacks()`.

use super::rc::*;
use super::Route2Level;
use crate::builder::MapBuilder;
use crate::consts::{cb, msg, wm};

const CREDWAIT: i32 = 5000;

#[inline]
fn text(
    b: &mut MapBuilder,
    wait: i32,
    x: i32,
    y: i32,
    z: i32,
    message: u16,
    path: u16,
    size: Option<i32>,
) {
    b.textpath(wait, x, y, z, message, path, 14, size);
}

/// Shared `cutcreds` body from CREDITS.ASM:134-191. SPECIAL.ASM calls this
/// same assembled subroutine; keeping one emitter prevents the two endings
/// from drifting.
pub(crate) fn append_cutcreds(b: &mut MapBuilder) {
    b.mapwait(2000);
    b.pathobj(1200, 0, -1500, 3500, SH_NULLSHAPE, PATH_ID_DSTARFOX, 10, 10);
    text(
        b,
        0,
        1535,
        0,
        1800,
        msg::PRESENTED,
        PATH_ID_DPRESENTED,
        Some(-48),
    );
    text(
        b,
        800,
        -1265,
        0,
        1800,
        msg::BY,
        PATH_ID_DPRESENTED,
        Some(-48),
    );
    b.pathobj(1200, 0, 1500, 3500, SH_NULLSHAPE, PATH_ID_DNINTENDO, 10, 10);

    b.mapwait(3000);
    text(
        b,
        1000,
        2000,
        -200,
        3000,
        msg::EXECUTIVE,
        PATH_ID_DSIDESLIP,
        None,
    );
    text(
        b,
        1000,
        2000,
        200,
        3000,
        msg::YAMAUCHI,
        PATH_ID_DSIDESLIP,
        Some(32),
    );

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::PRODUCER, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::MIYAMOTO, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::DIRECTOR, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::EGUCHI, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::ASSISTANTDIRECTOR, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::YAMADA, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -600, 3000, msg::PROGRAMMED, 350, Some(32));
    text(b, 1000, 2000, -300, 3000, msg::BY, 350, None);
    text(b, 1000, 2000, 0, 4400, msg::DYLAN, 350, Some(100));
    text(b, 1000, 2000, 400, 4400, msg::GILES, 350, Some(100));
    text(b, 1000, 2000, 800, 4400, msg::KRISTER, 350, Some(100));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -300, 3000, msg::SYSTEM3D, 350, None);
    text(b, 1000, 2000, 0, 3000, msg::PETE, 350, Some(32));
    text(b, 1000, 2000, 300, 3000, msg::CARL, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::GRAPHICDESIGNER, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::IMAMURA, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::SHAPEDESIGNER, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::WATANABE, 350, Some(16));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::EFFECTS, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::KONDO, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -200, 3000, msg::COMPOSER, 350, None);
    text(b, 1000, 2000, 200, 3000, msg::HIRASAWA, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(b, 1000, 2000, -300, 3000, msg::DEVELOPED, 350, Some(24));
    text(b, 1000, 2000, 0, 3000, msg::BY, 350, Some(-32));
    text(b, 1000, 2000, 300, 3000, msg::ARGONAUT, 350, Some(24));
}

/// C `build_credits_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.qfadedown();
    b.waitfade();

    // Lines 6-8: `meters_off trans`, setbg cred, initbg. `m_meters`
    // resides in Super FX RAM bank $70; `trans` also reloads the game
    // character map before the new background is initialized.
    b.setvarb24(wm::M_METERS, 0);
    b.mapcodejsl_builtin(cb::SETCHARMAPFROMMAP_L);
    b.setbg(BG_CRED);
    b.initbg();

    // Lines 10-17: start_65816 — reset lastplayz, pviewposz, and player Z.
    let reset_view_ptr = b.mapcode65816_inline();

    // Lines 19-20: mapcode_jsl initblack_l, setvar.b stayblack,10
    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.setvarb(wm::STAYBLACK, 10);

    // Lines 22-24: freeze BG2 at the origin for the staff roll.
    b.setvarb(wm::BG2VOFSOVERRIDE, 1);
    b.setvarw(wm::BG2HOFSREQ, 0);
    b.setvarw(wm::BG2VOFSREQ, 0);

    // Lines 26-27: setfadeup quick, mapwait 200
    b.qfadeup();
    b.mapwait(200);

    // Lines 28-38: start_65816 block — disable wobble, set noctrl+nofire
    let credits_init_ptr = b.mapcode65816_inline();

    // Line 40: mapjsr actualcreds
    b.mapjsr("credits.actualcreds");

    // Lines 42-47: THE END letter pathobjs
    b.pathobj(0, 972, -969, 1000, SH_FONT_T2_PROXY, PATH_ID_THEENDT, 6, 4);
    b.pathobj(
        0,
        -1120,
        1377,
        1000,
        SH_FONT_H2_PROXY,
        PATH_ID_THEENDH,
        6,
        4,
    );
    b.pathobj(
        0,
        -1019,
        -1530,
        1000,
        SH_FONT_E2_PROXY,
        PATH_ID_THEENDE,
        6,
        4,
    );
    b.pathobj(
        0,
        1070,
        -1326,
        1000,
        SH_FONT_E3_PROXY,
        PATH_ID_THEENDE2,
        6,
        4,
    );
    b.pathobj(
        0,
        1550 + 29,
        1323 + 54,
        1000,
        SH_FONT_N2_PROXY,
        PATH_ID_THEENDN,
        6,
        4,
    );
    b.pathobj(
        0,
        -1050 + 129,
        1428,
        1000,
        SH_FONT_D2_PROXY,
        PATH_ID_THEENDD,
        6,
        4,
    );

    // Line 48: mapwait 6000
    b.mapwait(6000);

    // Line 49: KALCS.INC defines `le_endofcreds` as 8.
    b.setvarb(WM_LEVELFINISHED, 8);

    // Lines 50-53: .lp infinite wait (IFEQ EXITCREDITS path — credits loop)
    b.label("credits.lp");
    b.mapwait(5000);
    b.mapgoto("credits.lp");

    // ---- actualcreds subroutine ----
    b.label("credits.actualcreds");

    // Line 85: mapjsr cutcreds
    b.mapjsr("credits.cutcreds");

    // Lines 86-131: NTSC credit blocks (GERMAN/RUMBLE build sections are
    // absent from the USA retail ROM).
    b.mapwait(CREDWAIT);
    text(
        &mut b,
        1000,
        2000,
        -1000,
        3500,
        msg::SUPERFXSTAFF,
        350,
        None,
    );
    text(&mut b, 1000, 2000, -600, 3500, msg::JEZ, 350, Some(32));
    text(&mut b, 1000, 2000, -300, 3500, msg::BEN, 350, Some(32));
    text(&mut b, 1000, 2000, 0, 3500, msg::RICK, 350, Some(32));
    text(&mut b, 1000, 2000, 300, 3500, msg::NISHIUMI, 350, Some(32));
    text(&mut b, 1000, 2000, 600, 3500, msg::KAKUI, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(&mut b, 1000, 2000, -450, 3500, msg::SOFTWARE, 350, None);
    text(&mut b, 1000, 2000, -150, 3500, msg::NISHIDA, 350, Some(32));
    text(&mut b, 1000, 2000, 150, 3500, msg::KAWAGUCHI, 350, Some(32));
    text(&mut b, 1000, 2000, 450, 3500, msg::YAMASHIRO, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(&mut b, 1000, 2000, -600, 3500, msg::ENGLISH, 350, None);
    text(&mut b, 1000, 2000, -300, 3500, msg::DAN, 350, Some(32));
    text(&mut b, 1000, 2000, 0, 3500, msg::TONY, 350, Some(32));
    text(&mut b, 1000, 2000, 300, 3500, msg::JONDEAN, 350, Some(32));
    text(&mut b, 1000, 2000, 600, 3500, msg::IAN, 350, Some(32));

    b.mapwait(CREDWAIT);
    text(&mut b, 1000, 2000, -1000, 3500, msg::JAPANESE, 350, None);
    text(&mut b, 1000, 2000, -600, 3500, msg::KATO, 350, Some(32));
    text(&mut b, 1000, 2000, -300, 3500, msg::SHIMIZU, 350, Some(32));
    text(&mut b, 1000, 2000, 0, 3500, msg::KIMURA, 350, Some(32));
    text(&mut b, 1000, 2000, 300, 3500, msg::YAJIMA, 350, Some(32));
    text(&mut b, 1000, 2000, 600, 3500, msg::YAMAMOTO, 350, Some(32));

    // Line 131: mapwait 9000-31*medpspeed
    b.mapwait(9000 - 31 * MEDPSPEED);
    b.maprts();

    // ---- cutcreds subroutine ----
    b.label("credits.cutcreds");

    append_cutcreds(&mut b);

    b.maprts();

    b.resolve();
    let (data, labels) = b.finish();

    // C `register_credits_inline_callbacks()`.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (reset_view_ptr, "credits_reset_view_inline"),
            (credits_init_ptr, "credits_init_inline"),
        ],
    )
}
