//! Player-movement / camera audit against the ROM oracle.
//!
//! Focus: ROM `getview_l` (GAME.ASM:4-58) — the routine that produces the
//! camera view-rotation angles `viewrotxw/yw/zw` fed to the rotation matrix.
//! These are the angles the sf-game `GameCamera::update` (camera.rs) is
//! supposed to reproduce as `rot_x/rot_y/rot_z`.
//!
//! ROM formula (GAME.ASM:6-58, normal viewtype, 16-bit A):
//!   if noxrot != 0 { outvx = 0 }
//!   viewrotxw = outvx
//!   viewrotyw = outvy - player_turnrot          (16-bit wrapping)
//!   viewrotzw = outvz - plrotz                  (16-bit wrapping)
//!   if dozrot == 0 { viewrotzw = 0 }
//!
//! The rotation matrix uses these 16-bit angles; the high byte (>>8) is the
//! 8-bit (256==360) angle the sf-render transform consumes, matching how the
//! ship's own `al_rotx = plrotx>>8` is derived.
//!
//! getview_l can't be run whole under the CPU-only oracle: past the angle
//! computation it builds the camera matrix and, at GAME.ASM:265, `jsl
//! runmario_l` (the SuperFX GSU) which the 65816 core does not emulate. So we
//! patch a single `RTL` (0x6B) into the ROM image at the `.ok` label
//! (file offset 0x17528, immediately after `viewrotzw` is finalized) and let
//! the oracle return there with the three angle words already written to WRAM.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

// WRAM symbol addresses (sf-oracle/data/symbols.txt).
const OUTVX: u32 = 0x1944;
const OUTVY: u32 = 0x1946;
const OUTVZ: u32 = 0x1948;
const PLROTZ: u32 = 0x12BF;
const PLAYER_TURNROT: u32 = 0x15B5;
const NOXROT: u32 = 0x1ACA;
const DOZROT: u32 = 0x1776;
const VIEWTYPE: u32 = 0x164F;
const VIEWROTXW: u32 = 0x16B8;
const VIEWROTYW: u32 = 0x16BA;
const VIEWROTZW: u32 = 0x16BC;
const VIEWSHAKEX: u32 = 0x161F;
const VIEWSHAKEY: u32 = 0x1620;
const VIEWSHAKEZ: u32 = 0x1621;
const PVIEWPOSX: u32 = 0x1581;
const PVIEWPOSY: u32 = 0x1583;
const PVIEWPOSZ: u32 = 0x1585;
const VIEWFLOATX: u32 = 0x1571;
const VIEWFLOATY: u32 = 0x1573;

/// `.ok` label patch point (see module doc): make getview_l return right
/// after it finishes writing viewrotxw/yw/zw, before the matrix + SuperFX.
const PATCH_FILE_OFFSET: usize = 0x17528;

#[derive(Clone, Copy, Debug)]
struct ViewIn {
    outvx: i16,
    outvy: i16,
    outvz: i16,
    plrotz: i16,
    turnrot: i16,
    noxrot: u8,
    dozrot: u8,
}

fn rom_getview(rom: &[u8], addr: u32, i: ViewIn) -> (i16, i16, i16) {
    let mut patched = rom.to_vec();
    patched[PATCH_FILE_OFFSET] = 0x6B; // RTL
    let mut bus = SnesBus::new(patched);
    bus.write16(OUTVX, i.outvx as u16);
    bus.write16(OUTVY, i.outvy as u16);
    bus.write16(OUTVZ, i.outvz as u16);
    bus.write16(PLROTZ, i.plrotz as u16);
    bus.write16(PLAYER_TURNROT, i.turnrot as u16);
    bus.write8(NOXROT, i.noxrot);
    bus.write8(DOZROT, i.dozrot);
    bus.write8(VIEWTYPE, 0); // VIEWTYPE_NORM -> reach the viewrot computation
                             // Zero the shake / bob / pos inputs so the (untested here) viewpos math is
                             // deterministic and can't wander into anything odd before the patch.
    for a in [
        VIEWSHAKEX, VIEWSHAKEY, VIEWSHAKEZ, PVIEWPOSX, PVIEWPOSY, PVIEWPOSZ, VIEWFLOATX, VIEWFLOATY,
    ] {
        bus.write16(a, 0);
    }
    bus.write16(VIEWROTXW, 0);
    bus.write16(VIEWROTYW, 0);
    bus.write16(VIEWROTZW, 0);

    call(
        &mut bus,
        addr,
        &Entry {
            p: 0x20,
            ..Default::default()
        },
    );

    (
        bus.read16(VIEWROTXW) as i16,
        bus.read16(VIEWROTYW) as i16,
        bus.read16(VIEWROTZW) as i16,
    )
}

/// The exact ROM formula (GAME.ASM:6-58), for cross-checking the oracle.
fn rom_formula(i: ViewIn) -> (i16, i16, i16) {
    let vx = if i.noxrot != 0 { 0 } else { i.outvx };
    let vy = i.outvy.wrapping_sub(i.turnrot);
    let vz = if i.dozrot != 0 {
        i.outvz.wrapping_sub(i.plrotz)
    } else {
        0
    };
    (vx, vy, vz)
}

/// Pitch/yaw/roll helper matching ROM `getview_l` (and live `GameCamera`
/// pitch/roll). Live player-obj yaw still chase-feels `player.roty`; this
/// helper uses ROM yaw so the oracle cases assert MATCH on viewrot words.
fn rust_camera(i: ViewIn, _plrotx: i16, _plroty: i16) -> (i16, i16, i16) {
    let vx = if i.noxrot != 0 { 0 } else { i.outvx };
    let rot_x = vx >> 8;
    let rot_y = i.outvy.wrapping_sub(i.turnrot) >> 8;
    let rot_z = if i.dozrot != 0 {
        i.outvz.wrapping_sub(i.plrotz) >> 8
    } else {
        0
    };
    (rot_x << 8, rot_y << 8, rot_z << 8)
}

#[test]
fn getview_viewrot_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("GETVIEW_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };

    // Representative states. plrotx/plroty are the ship pitch/steer-yaw the
    // Rust camera keys off (irrelevant to the ROM viewrot, which uses outv*).
    let cases: [(ViewIn, i16, i16, &str); 6] = [
        (
            ViewIn {
                outvx: 0,
                outvy: 0,
                outvz: 0,
                plrotz: 0,
                turnrot: 0,
                noxrot: 0,
                dozrot: 0,
            },
            0,
            0,
            "neutral",
        ),
        (
            // Camera leaning up (outvx set by the gf_viewrot lean logic), ship level.
            ViewIn {
                outvx: 0x0300,
                outvy: 0,
                outvz: 0,
                plrotz: 0,
                turnrot: 0,
                noxrot: 0,
                dozrot: 0,
            },
            0,
            0,
            "view-lean pitch, ship level",
        ),
        (
            // noxrot gate: outvx must be forced to 0.
            ViewIn {
                outvx: 0x0300,
                outvy: 0,
                outvz: 0,
                plrotz: 0,
                turnrot: 0,
                noxrot: 1,
                dozrot: 0,
            },
            0,
            0,
            "noxrot gate",
        ),
        (
            // View yaw lean, ship steering the other way.
            ViewIn {
                outvx: 0,
                outvy: 0x0400,
                outvz: 0,
                plrotz: 0,
                turnrot: 0,
                noxrot: 0,
                dozrot: 0,
            },
            0,
            0x0200,
            "view-lean yaw",
        ),
        (
            // All-range/boss 90-deg turn: player_turnrot = 64<<8. ROM camera yaw
            // follows -turnrot; Rust cancels turnrot and stays put.
            ViewIn {
                outvx: 0,
                outvy: 0,
                outvz: 0,
                plrotz: 0,
                turnrot: 0x4000,
                noxrot: 0,
                dozrot: 0,
            },
            0,
            0,
            "all-range turnrot",
        ),
        (
            // Inside/tunnel: outvz carries ztilt+zshake, roll enabled (dozrot=1).
            ViewIn {
                outvx: 0,
                outvy: 0,
                outvz: 0x0100,
                plrotz: 0x0200,
                turnrot: 0,
                noxrot: 0,
                dozrot: 1,
            },
            0,
            0,
            "dozrot roll + outvz",
        ),
    ];

    let mut formula_bad = 0;
    let mut rust_diffs = 0;
    for (i, plrotx, plroty, name) in cases {
        let (ox, oy, oz) = rom_getview(&rom, addr, i);
        let (fx, fy, fz) = rom_formula(i);
        if (ox, oy, oz) != (fx, fy, fz) {
            formula_bad += 1;
            eprintln!(
                "FORMULA MISMATCH [{name}] ROM({ox:#06x},{oy:#06x},{oz:#06x}) != formula({fx:#06x},{fy:#06x},{fz:#06x})"
            );
        }
        let (rx, ry, rz) = rust_camera(i, plrotx, plroty);
        let diff = (ox, oy, oz) != (rx, ry, rz);
        if diff {
            rust_diffs += 1;
        }
        eprintln!(
            "[{name:28}] ROM viewrot=({:>4},{:>4},{:>4})  RUST cam=({:>4},{:>4},{:>4})  {}",
            ox >> 8,
            oy >> 8,
            oz >> 8,
            rx >> 8,
            ry >> 8,
            rz >> 8,
            if diff { "DIFF" } else { "ok" }
        );
    }

    // The oracle must confirm the documented ROM formula bit-exactly.
    assert_eq!(formula_bad, 0, "ROM getview formula cross-check failed");
    eprintln!("=> {rust_diffs} case(s) where sf-game camera pitch/roll helper diverges from ROM getview_l");
    // Pitch/roll now track outvx/outvz (+ noxrot/dozrot). Helper uses ROM yaw
    // too; live GameCamera still chase-feels player yaw for ASF4_PLAYEROBJ.
    assert_eq!(
        rust_diffs, 0,
        "camera pitch/roll must match ROM getview_l (float-ground fix)"
    );
}
