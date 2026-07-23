//! MAP_ID_SPECIAL — "Out of this Dimension" (LEVEL_S.ASM / SPECIAL.ASM).
//!
//! C oracle: `src/map/levels.c` `build_level_special_slice()` +
//! `register_level_special_inline_callbacks()`.
//!
//! Runtime-only C side effect NOT mirrored here: the register function
//! resets `g_numendok` for the level.

use super::rc::*;
use super::Route2Level;
use crate::builder::MapBuilder;
use crate::consts::wm;

/// C `build_level_special_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapcodejsl_builtin(MAP_CB_INITBLACK_L);
    b.mapwait(100);
    b.setvarb(wm::DOSPACESC, 2);
    b.setvarw(wm::BG2YSCROLL, -64);
    b.mapjsr("special.specialmap");
    b.mapend(1);

    // SPECIAL.ASM — specialmap subroutine
    b.label("special.specialmap");

    // Lines 3: mapwait 5000
    b.mapwait(5000);

    // Lines 5-9: paper plane wave 1 + pole_0
    b.pathobj(5000, 0, 0, 3000, SH_PAPER_1_PROXY, PATH_ID_PAPER_1B, 10, 10);
    b.pathobj(
        5000,
        0x0300,
        -0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        6000,
        -0x200,
        0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        6000,
        0x0200,
        -0x100,
        4000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        10000,
        0x0100,
        -0x400,
        1500,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.mapobj(8000, 0, 0, 4000, SH_POLE_0_PROXY, STRATEGY_POLE0);

    // Lines 12-24: paper plane wave 2 + poles
    b.pathobj(
        5000,
        -0x200,
        0x0200,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        5000,
        0x0100,
        -0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        5000,
        -0x200,
        0x400,
        1500,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        6000,
        -0x400,
        0x150,
        3000,
        SH_PAPER_3_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.mapobj(6000, 0, -0x200, 4000, SH_POLE_0_PROXY, STRATEGY_POLE0);
    b.mapobj(6000, -0x100, 0x100, 4000, SH_POLE_0_PROXY, STRATEGY_POLE0);
    b.pathobj(
        6000,
        0x0400,
        0,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        6000,
        0,
        -0x400,
        1500,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        8000,
        0x0200,
        0x200,
        2000,
        SH_PAPER_3_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        1000,
        0,
        0x0100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        1000,
        -0x300,
        0x200,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        5000,
        -0x100,
        -0x400,
        1000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        4000,
        0,
        -0x400,
        1500,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );

    // Lines 26-27: paper pair
    b.pathobj(
        2000,
        -0x300,
        0,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        2000,
        -0x300,
        0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );

    // Lines 29-34: mixed paper_3 / paper_1 wave
    b.pathobj(
        5000,
        -0x200,
        0x200,
        4000,
        SH_PAPER_3_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        3000,
        0,
        0x200,
        4000,
        SH_PAPER_3_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        10000,
        0x200,
        0x200,
        4000,
        SH_PAPER_3_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        5000,
        0x0300,
        0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        6000,
        -0x200,
        0x100,
        3000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );
    b.pathobj(
        10000,
        0x0200,
        0x100,
        4000,
        SH_PAPER_1_PROXY,
        PATH_ID_PAPER_1B,
        10,
        10,
    );

    // Line 37-38: fadeoutbgm + setbgm 5 (boss music)
    b.setbgm(BGM_FADEOUT);
    b.setbgm(BGM_BOSS1);

    // Line 40: slot machine boss pathobj
    b.pathobj(
        0,
        3000,
        0,
        4200,
        SH_SLOT_0_PROXY,
        PATH_ID_SLOTMACHINE,
        10,
        10,
    );

    // Line 41: exact `mapwaitboss 7` expansion.
    b.mapwait(100);
    let mapwaitboss_trigse_ptr = b.mapcode65816_inline();
    b.label("special.bosswait.loop");
    b.mapif_builtin(MAP_CB_CHKBOSSDEAD, "special.bosswait.cont");
    b.mapgoto("special.bosswait.loop");
    b.label("special.bosswait.cont");
    let mapwaitboss_cantdie_ptr = b.mapcode65816_inline();
    let mapwaitboss_cleanup_ptr = b.mapcode65816_inline();
    b.setbgm(7);

    // Lines 45-57: endofspecialmap — inline 65816 block
    // Clears nofire and notdie flags, hides HUD on boss death.
    let special_boss_cleanup_ptr = b.mapcode65816_inline();

    // Line 59: mapwait 2000
    b.mapwait(2000);

    // Line 61: `rotate_hof` is enum value zero in KALCS.INC.
    b.setvarw(wm::HPOSJMP, 0);

    // Line 64: shared exact CREDITS.ASM `cutcreds` subroutine.
    b.mapjsr("special.cutcreds");

    // Line 66: mapwait 6000
    b.mapwait(6000);

    // Lines 68-73: "THE END" letter objects
    b.label("special.theend_loop");
    b.mapobj(0, 972, -969, 1000, SH_FONT_T2_PROXY, STRAT_ADDR_THEEND_T);
    b.mapobj(0, -1120, 1377, 1000, SH_FONT_H2_PROXY, STRAT_ADDR_THEEND_H);
    b.mapobj(0, -1019, -1530, 1000, SH_FONT_E2_PROXY, STRAT_ADDR_THEEND_E);
    b.mapobj(0, 1070, -1326, 1000, SH_FONT_E3_PROXY, STRAT_ADDR_THEEND_E2);
    b.mapobj(
        0,
        1550 + 29,
        1323 + 54,
        1000,
        SH_FONT_N2_PROXY,
        STRAT_ADDR_THEEND_N,
    );
    b.mapobj(
        0,
        -1050 + 129,
        1428,
        1000,
        SH_FONT_D2_PROXY,
        STRAT_ADDR_THEEND_D,
    );

    // Lines 74-76: theenddead check loop
    b.label("special.theenddead_check");
    let special_theenddead_ptr = b.mapcode65816_inline();
    // If theenddead false, goto .ll (theenddead_check)
    // theenddead_check callback handles the branching.

    // Lines 77-84: .cont — clear + restart THE END sequence
    b.label("special.theenddead_cont");
    b.mapwait(2500);
    b.mapcodejsl_builtin(MAP_CB_CLEARMAP_L);
    // SPECIAL.ASM uses a one-instruction inline `stz numplasers`; SETVARB
    // has the same externally visible effect in the Rust VM.
    b.setvarb(wm::NUMPLASERS, 0);
    b.setvarb(WM_NUMENDOK, 0);
    b.mapwait(1000);
    b.mapgoto("special.theend_loop");

    // maprts (end of specialmap subroutine — unreachable due to loop)
    b.maprts();

    // Shared exact CREDITS.ASM subroutine.
    b.label("special.cutcreds");
    super::credits::append_cutcreds(&mut b);
    b.maprts();

    b.resolve();

    // C: theenddead label-ptr lookups (fall back to 0; must exist here).
    assert!(b.lookup_label("special.theenddead_cont").is_some());
    assert!(b.lookup_label("special.theenddead_check").is_some());

    let (data, labels) = b.finish();

    // C `register_level_special_inline_callbacks()` — registration order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (mapwaitboss_trigse_ptr, "special_mapwaitboss_trigse"),
            (mapwaitboss_cantdie_ptr, "special_mapwaitboss_cantdie"),
            (mapwaitboss_cleanup_ptr, "special_mapwaitboss_cleanup"),
            (special_boss_cleanup_ptr, "special_boss_cleanup"),
            (special_theenddead_ptr, "special_theenddead_check"),
        ],
    )
}
