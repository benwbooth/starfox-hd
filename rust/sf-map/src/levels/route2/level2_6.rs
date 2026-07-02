//! MAP_ID_2_6 — Venom 2 Highway (Level 2, Route 2).
//!
//! C oracle: `src/map/levels.c` `build_level2_6_wrapper_slice()` +
//! `register_level2_6_inline_callbacks()` (TRUCKER.ASM Mad Trucker boss).
//!
//! Runtime-only C side effect NOT mirrored here: the register function
//! resets `g_maptrigger` before hooking the trucker callbacks.

use super::rc::*;
use super::submaps;
use super::Route2Level;
use crate::builder::MapBuilder;
use crate::consts::op;

/// C `build_level2_6_wrapper_slice()`.
pub fn build() -> Route2Level {
    let mut b = MapBuilder::new();

    b.mapwait(4000);

    // ============================================================
    // incmap CL_COLON.ASM — Colony pipe clear demo (inlined)
    // ============================================================
    // CL_COLON.ASM lines 1-2: gpipescale=16
    // Lines 3-4: mapplayercantdie / mapplayermode toCslow
    //   — These opcodes (mapplayercantdie, mapplayermode) are not yet
    //     implemented in the map executor. Skipped for now.
    // TODO: b.mapplayercantdie();
    // TODO: b.mapplayermode(PLAYER_MODE_TOCSLOW);

    // Lines 6-8: three pipe background objects (mapobjnomem)
    //   — mapobjnomem is not yet implemented. Emit as regular mapobj.
    b.mapobj(0, 0, -60, 4200, SH_PIPE_8_0_PROXY, IS_NOCOLL);
    b.mapobj(400, 0, -60, 4200, SH_PIPE_8_0_PROXY, IS_NOCOLL);
    b.mapobj(0, 0, -60, 4200, SH_PIPE_8_PROXY, IS_COLONYEXIT);
    // Line 9: mapwait 4000
    b.mapwait(4000);

    // Lines 11-14: pdist, setbg, initbg — background setup
    //   — setbg/initbg opcodes exist but bg id '2_6b' is not mapped yet.
    //     Skip background changes for now.

    // Lines 16-64: mappipe sequence (colony pipe path)
    //   — The mappipe opcode is not yet implemented in the map executor.
    //     This is the pipe-following clear demo sequence with 25+ pipe
    //     segments and setalvar rotz calls for camera rotation.
    //     TODO: Implement mappipe opcode in world.c and port these calls.
    // mappipe 0,0,0,0,0
    // mappipe -11,40,-1,0,2
    // mappipe -40,70,-2,1,2
    // ... (full sequence in CL_COLON.ASM)
    // setalvar rotz,0 / rotz,-12 / rotz,-25 / rotz,-42 / rotz,-56
    // End of CL_COLON.ASM inline
    // ============================================================

    // mapwait 2000
    b.mapwait(2000);

    // MAP2_6A.ASM line 151: incmap trucker
    // The trucker boss sequence is inlined at this point in the map stream.
    // We emit it as a subroutine for modularity, called via mapjsr.
    b.mapjsr("level2_6.trucker");

    // MAP2_6A.ASM lines after trucker: setbgm $f1, mapwait, setbgm $12, markboss
    b.setbgm(BGM_FADEOUT);
    b.mapwait(MEDPSPEED * 15 * 2);
    // setbgm $12 — victory fanfare variant (not standard BGM_FANFARE)
    b.setbgm(0x12);
    // markboss boss26 — mark as completed
    b.mapcodejsl_builtin(MAP_CB_MARKBOSS_L);

    b.emit8(op::END);

    // TRUCKER.ASM — Mad Trucker subroutine
    let trucker = submaps::append_trucker_submap(&mut b);

    b.resolve();

    // C: trucker label lookups (carryon/rightblockbit/continue/loops).
    assert!(b.lookup_label("level2_6.trucker.carryon").is_some());
    assert!(b.lookup_label("level2_6.trucker.rightblockbit").is_some());
    assert!(b.lookup_label("level2_6.trucker.continue").is_some());
    assert!(b.lookup_label("level2_6.trucker.loop").is_some());
    assert!(b.lookup_label("level2_6.trucker.loop2").is_some());

    let (data, labels) = b.finish();

    // C `register_level2_6_inline_callbacks()` — registration-call order.
    Route2Level::new(
        data,
        labels,
        vec![],
        vec![
            (trucker.biker_check, "trucker_biker_check"),
            (trucker.trigger, "trucker_trigger_check"),
        ],
    )
}
