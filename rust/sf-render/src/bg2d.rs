//! 2D background layer pass.
//!
//! Port (C oracle): `src/renderer/bg2d.c`. Composes SNES BG layers CPU-side
//! from the uncompressed dev assets (.CGX tiles / .SCR tilemaps / .COL
//! palettes) into RGBA textures drawn as a screen-space quad before the 3D
//! pass, including the camera-coupled horizon scroll (GSTRATS.ASM
//! calcbgscroll_l) exactly as implemented in the C:
//! - vertical: -focal*tan(pitch), clamped to [-56, 232] unless
//!   nomaxbg2Yscroll
//! - horizontal: bg2Xscroll + yaw*8 + worldx/8 (`hofmode rotate` base)
//!
//! Deviations documented in bg2d.c apply here identically.

use std::path::{Path, PathBuf};

use crate::gpu::{Gpu, TextureId, Vertex2, WHITE_TEX};
use crate::renderer::{FrameInputs, GameState, BGF_BG};
use crate::shapes::background_fade_palette_bgr;
use crate::transform::Transform;
use sf_core::scene::{
    PaletteFadeTarget, BG2_HORIZONTAL_OFFSET_ROWS, BG2_VERTICAL_OFFSET_COLUMNS,
    PALETTE_FADE_COUNTER_START,
};

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[inline]
fn ortho(w: f32, h: f32) -> [f32; 16] {
    [
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        -1.0,
        0.0,
        -1.0,
        -1.0,
        0.0,
        1.0,
    ]
}

pub const BG2D_W: usize = 256;
pub const BG2D_H: usize = 224;

/// Source background pitch ramp. The authored 16-bit turn fraction is reduced
/// by 64, then half of that result is added before the direction is reversed.
/// Keeping the intermediate signed shifts avoids half-pixel sampling drift in
/// strict source-resolution output.
fn source_vertical_camera_offset(rotation: u16, unrestricted: bool) -> i16 {
    const MAXIMUM_DOWNWARD_OFFSET: i16 = 232;
    const MAXIMUM_UPWARD_OFFSET: i16 = -56;

    let reduced = (rotation as i16) >> 6;
    let offset = reduced.wrapping_add(reduced >> 1).wrapping_neg();
    if unrestricted {
        offset
    } else {
        offset.clamp(MAXIMUM_UPWARD_OFFSET, MAXIMUM_DOWNWARD_OFFSET)
    }
}

/// Source horizontal background coupling from the complete yaw fraction and
/// whole-unit camera position.
fn source_horizontal_camera_offset(camera_x: i32, rotation: u16) -> i16 {
    let world_x = (camera_x >> 16) as i16;
    ((rotation as i16) >> 5).wrapping_add(world_x >> 3)
}

fn interpolate_wrapped_offset(previous: i16, current: i16, alpha: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return f32::from(current);
    }
    let previous = f32::from(previous);
    let current = f32::from(current);
    let mut delta = (current - previous).rem_euclid(period);
    if delta > period * 0.5 {
        delta -= period;
    }
    previous + delta * alpha.clamp(0.0, 1.0)
}

fn interpolate_offset_table<const LENGTH: usize>(
    previous: Option<&[i16; LENGTH]>,
    current: &[i16; LENGTH],
    alpha: f32,
    period: f32,
) -> [f32; LENGTH] {
    std::array::from_fn(|index| {
        previous.map_or_else(
            || f32::from(current[index]),
            |previous| interpolate_wrapped_offset(previous[index], current[index], alpha, period),
        )
    })
}
const COLORS_PER_PALETTE: usize = 16;
const BACKGROUND_FADE_PALETTE: usize = 4;
/// Native polygon materials for the pre-rendered title presentation use the
/// authored CP-US row 6 ramp.
const TITLE_POLYGON_PALETTE: usize = 6;
/// The source reserves palette row 7 for its independently loaded game
/// palette while the title asset supplies rows 0 through 6.
const TITLE_GAME_PALETTE: usize = 7;
/// Effective BG3 horizontal scroll retained by the source Mode-1 setup.
const TITLE_BG3_HORIZONTAL_SCROLL: usize = 1_020;
const SOURCE_TILEMAP_EXTENT: usize = 1_024;
const TITLE_TILEMAP_EXTENT: usize = 256;
const TITLE_FRAMEBUFFER_LEFT: usize = 16;
const TITLE_FRAMEBUFFER_TOP: usize = 16;
const TITLE_FRAMEBUFFER_WIDTH: usize = 224;
const TITLE_FRAMEBUFFER_HEIGHT: usize = 192;
const STATIC_PALETTE_PIXEL: u8 = u8::MAX;

/// MHOFS.MC `bholetab` (70 signed bytes). The source deliberately places
/// repeated copies after `bholetabend` because the 112-line loop reads past the
/// nominal end; indexing modulo 70 is byte-identical to that padding.
const BHOLE_TAB: [i16; 70] = [
    -20, -20, -20, -20, -19, -19, -19, -18, -18, -17, -16, -15, -14, -13, -11, -9, -7, -5, -3, -1,
    2, 4, 6, 8, 11, 13, 14, 15, 16, 17, 18, 18, 19, 19, 19, 20, 20, 20, 20, 19, 19, 19, 18, 18, 17,
    16, 15, 14, 12, 11, 8, 6, 4, 2, -1, -3, -5, -7, -9, -11, -13, -14, -15, -16, -17, -18, -18,
    -19, -19, -19,
];

/// MHOFS `testk2` after `calls` invocations of `mbhole`. testk3 starts at
/// bholelimit=$a0 and testk4 at +1; the direction flips before the phase add
/// whenever the counter reaches zero. The complete state repeats every 640.
fn bhole_phase(calls: u32) -> i16 {
    let mut phase = 0i16;
    let mut remaining = 160u16;
    let mut direction = 1i16;
    for _ in 0..calls % 640 {
        remaining -= 1;
        if remaining == 0 {
            direction = -direction;
            remaining = 320;
        }
        phase = phase.wrapping_add(direction);
    }
    phase
}

/// Port of the central 112-iteration `mbhole` loop. Each generated HOFS word
/// is written to the scanline above and below the 111/112 center seam.
fn bhole_line_offsets(phase: i16) -> [i16; BG2D_H] {
    let mut out = [0i16; BG2D_H];
    // Oracle-derived fixed-point slope: 4*phase + floor(phase/4).
    let slope_8_8 = 4 * phase as i32 + (phase as i32).div_euclid(4);
    for step in 0..112usize {
        // Accumulate the signed 8.8 slope before emitting each line.
        let gradient = (((step + 1) as i32) * slope_8_8).div_euclid(256);
        // testk is zero in retail; mbhole adds 3 before indexing bholetab.
        let wave = BHOLE_TAB[(step + 3) % BHOLE_TAB.len()] as i32;
        let hofs = (512 + gradient + wave) as i16;
        out[111 - step] = hofs;
        out[112 + step] = hofs;
    }
    out
}

/// Background ids from levels.c map bytecode (setbg opcode operand).
pub const BG2D_ID_TITLE: u8 = 41;
/// Source controller-layout background.
pub const BG2D_ID_CONTINUE: u8 = 42;
/// Pseudo-id for the planet-select map screen.
pub const BG2D_ID_MAP: u8 = 63;
/// Pseudo-id for bg_special (SNES id 44 clashes with the port's BG_TRAINING).
pub const BG2D_ID_SPECIAL: u8 = 62;
/// Width and height of each CONT-2 controller-layout quadrant.
const CONTROLLER_PANEL_SIZE: usize = 256;

/// BGS.ASM bg_* block -> data files (mirror of `s_bg_defs`).
pub struct BgDef {
    pub id: u8,
    pub name: &'static str,
    pub cgx: &'static str,
    pub scr: &'static str,
    pub col: &'static str,
    pub vofs: i32,
    pub cgx3: Option<&'static str>,
    pub scr3: Option<&'static str>,
    pub vofs3: i32,
    pub sky: bool,
}

macro_rules! bgdef {
    ($id:expr, $name:expr, $cgx:expr, $scr:expr, $col:expr, $vofs:expr, $sky:expr) => {
        BgDef {
            id: $id,
            name: $name,
            cgx: $cgx,
            scr: $scr,
            col: $col,
            vofs: $vofs,
            cgx3: None,
            scr3: None,
            vofs3: 0,
            sky: $sky,
        }
    };
    ($id:expr, $name:expr, $cgx:expr, $scr:expr, $col:expr, $vofs:expr,
     $cgx3:expr, $scr3:expr, $vofs3:expr, $sky:expr) => {
        BgDef {
            id: $id,
            name: $name,
            cgx: $cgx,
            scr: $scr,
            col: $col,
            vofs: $vofs,
            cgx3: Some($cgx3),
            scr3: Some($scr3),
            vofs3: $vofs3,
            sky: $sky,
        }
    };
}

pub static BG_DEFS: &[BgDef] = &[
    // Corneria family (ST-P sky + mountain horizon + ground gradient)
    bgdef!(
        4,
        "bg_1_1c",
        "data/bg/ST-P.CGX",
        "data/bg/ST-P.SCR",
        "data/bg/BG2-D.COL",
        232,
        true
    ),
    bgdef!(
        3,
        "bg_3_1c",
        "data/bg/ST-P.CGX",
        "data/bg/ST-P.SCR",
        "data/bg/BG2-G.COL",
        232,
        true
    ),
    bgdef!(
        44,
        "bg_training",
        "data/bg/ST-P.CGX",
        "data/bg/ST-P.SCR",
        "data/bg/BG2-D.COL",
        232,
        true
    ),
    // Asteroid-belt space (starfield + cratered moon)
    bgdef!(
        6,
        "bg_1_3i",
        "data/bg/3-4.CGX",
        "data/bg/3-4.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    bgdef!(
        7,
        "bg_1_3a",
        "data/bg/3-4.CGX",
        "data/bg/3-4.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    bgdef!(
        9,
        "bg_1_3c",
        "data/bg/3-4.CGX",
        "data/bg/3-4.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    bgdef!(
        35,
        "bg_3_4d",
        "data/bg/3-4.CGX",
        "data/bg/3-4.SCR",
        "data/bg/SPACE.COL",
        0,
        true
    ),
    // Asteroid clear demo (asteroid + planets starfield)
    bgdef!(
        12,
        "bg_1_3e",
        "data/bg/SPACE.CGX",
        "data/bg/1-3.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    // Tunnels (info voff; static base image)
    bgdef!(
        8,
        "bg_1_3b",
        "data/bg/T-SP.CGX",
        "data/bg/T-SP.SCR",
        "data/bg/T-M-3.COL",
        0,
        false
    ),
    bgdef!(
        34,
        "bg_3_4c",
        "data/bg/T-SP.CGX",
        "data/bg/T-SP.SCR",
        "data/bg/T-M-3.COL",
        0,
        false
    ),
    bgdef!(
        25,
        "bg_2_3c",
        "data/bg/T-SP.CGX",
        "data/bg/T-F-S.SCR",
        "data/bg/T-M-3.COL",
        0,
        false
    ),
    bgdef!(
        29,
        "bg_2_6c",
        "data/bg/T-ST.CGX",
        "data/bg/T-ST.SCR",
        "data/bg/T-M-3.COL",
        0,
        false
    ),
    // Venom final approach (glowing spheres; info voff)
    bgdef!(
        17,
        "bg_1_6c",
        "data/bg/B-HOLE.CGX",
        "data/bg/LAST.SCR",
        "data/bg/BG2-F.COL",
        0,
        false
    ),
    // Fortuna bridge (sky/water backdrop + BG3 water-surface overlay; voff)
    bgdef!(
        24,
        "bg_2_3b",
        "data/bg/B-M.CGX",
        "data/bg/2-3B.SCR",
        "data/bg/B-M.COL",
        0,
        "data/bg/2-3B.CGX",
        "data/bg/2-3H.SCR",
        24,
        false
    ),
    // Attract intro (Corneria seen from space; info von,hon)
    bgdef!(
        40,
        "bg_intro",
        "data/bg/DEMO.CGX",
        "data/bg/DEMO.SCR",
        "data/bg/BG2-B.COL",
        24,
        true
    ),
    // Continue / controller screen (US CONT-2). The 512-by-512 source
    // tilemap contains four 256-by-256 controller layouts, so retain the full
    // wrapping map and select its quadrant from typed briefing state.
    bgdef!(
        BG2D_ID_CONTINUE,
        "bg_cont",
        "data/bg/CONT-2.CGX",
        "data/bg/CONT-2.SCR",
        "data/bg/BG2-E.COL",
        0,
        true
    ),
    // Credits (nebula starfield; info von,hon)
    bgdef!(
        43,
        "bg_cred",
        "data/bg/2-4.CGX",
        "data/bg/2-4.SCR",
        "data/bg/BG2-F.COL",
        232,
        true
    ),
    // Planet-select map screen (pseudo-id)
    bgdef!(
        BG2D_ID_MAP,
        "planets_map",
        "data/bg/MAP.CGX",
        "data/bg/MAP.SCR",
        "data/bg/MAP_C.COL",
        0,
        false
    ),
    // --- Level-default backgrounds ---
    bgdef!(
        5,
        "bg_1_2",
        "data/bg/STARS.CGX",
        "data/bg/STARS.SCR",
        "data/bg/STARS.COL",
        232,
        true
    ),
    bgdef!(
        13,
        "bg_1_4",
        "data/bg/1-4.CGX",
        "data/bg/1-4.SCR",
        "data/bg/LIGHT.COL",
        232,
        true
    ),
    bgdef!(
        14,
        "bg_1_5",
        "data/bg/LSB.CGX",
        "data/bg/LSB.SCR",
        "data/bg/BG2-C.COL",
        164,
        true
    ),
    bgdef!(
        37,
        "bg_3_6",
        "data/bg/LSB.CGX",
        "data/bg/LSB.SCR",
        "data/bg/BG2-C.COL",
        164,
        true
    ),
    bgdef!(
        15,
        "bg_1_6a",
        "data/bg/F-1.CGX",
        "data/bg/F-1.SCR",
        "data/bg/BG2-A.COL",
        232,
        true
    ),
    bgdef!(
        38,
        "bg_3_7a",
        "data/bg/F-1.CGX",
        "data/bg/F-1.SCR",
        "data/bg/BG2-A.COL",
        232,
        true
    ),
    bgdef!(
        22,
        "bg_2_2",
        "data/bg/2-2.CGX",
        "data/bg/2-2.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    bgdef!(
        23,
        "bg_2_3a",
        "data/bg/2-3.CGX",
        "data/bg/2-3.SCR",
        "data/bg/BG2-A.COL",
        232,
        true
    ),
    bgdef!(
        26,
        "bg_2_4",
        "data/bg/2-4.CGX",
        "data/bg/2-4.SCR",
        "data/bg/BG2-F.COL",
        232,
        true
    ),
    bgdef!(
        27,
        "bg_2_6a",
        "data/bg/C-M.CGX",
        "data/bg/T-SS.SCR",
        "data/bg/T-M-2.COL",
        0,
        "data/bg/FS-BG3.CGX",
        "data/bg/FS-NI.SCR",
        0,
        false
    ),
    bgdef!(
        30,
        "bg_3_2",
        "data/bg/3-2.CGX",
        "data/bg/3-2.SCR",
        "data/bg/BG2-B.COL",
        24,
        true
    ),
    bgdef!(
        31,
        "bg_3_3a",
        "data/bg/3-3.CGX",
        "data/bg/3-3.SCR",
        "data/bg/BG2-C.COL",
        232,
        true
    ),
    bgdef!(
        33,
        "bg_3_4b",
        "data/bg/3-4.CGX",
        "data/bg/3-4.SCR",
        "data/bg/SPACE.COL",
        232,
        true
    ),
    bgdef!(
        36,
        "bg_3_5",
        "data/bg/HOLE-A.CGX",
        "data/bg/HOLE-A.SCR",
        "data/bg/HOLE.COL",
        272,
        true
    ),
    // bg_hole: info hon,voff; scanline HOFS warp is applied at render time.
    bgdef!(
        39,
        "bg_hole",
        "data/bg/B-HOLE.CGX",
        "data/bg/B-HOLE.SCR",
        "data/bg/BG2-D.COL",
        0,
        false
    ),
    bgdef!(
        BG2D_ID_SPECIAL,
        "bg_special",
        "data/bg/M.CGX",
        "data/bg/M.SCR",
        "data/bg/SPACE.COL",
        448,
        true
    ),
];

/// Default bg id per loaded map (mirror of `s_map_default_bg`; map ids are
/// the port's levels.h MAP_ID_* values).
pub static MAP_DEFAULT_BG: &[(u32, u8)] = &[
    (1, 4),                // MAP_ID_1_1
    (2, 5),                // MAP_ID_1_2
    (3, 6),                // MAP_ID_1_3
    (4, 13),               // MAP_ID_1_4
    (5, 14),               // MAP_ID_1_5
    (6, 15),               // MAP_ID_1_6
    (7, 4),                // MAP_ID_2_1
    (8, 22),               // MAP_ID_2_2
    (9, 23),               // MAP_ID_2_3
    (10, 26),              // MAP_ID_2_4
    (11, 14),              // MAP_ID_2_5
    (12, 27),              // MAP_ID_2_6
    (13, 3),               // MAP_ID_3_1
    (14, 30),              // MAP_ID_3_2
    (15, 31),              // MAP_ID_3_3
    (16, 33),              // MAP_ID_3_4
    (17, 36),              // MAP_ID_3_5
    (18, 37),              // MAP_ID_3_6
    (19, 38),              // MAP_ID_3_7
    (20, 39),              // MAP_ID_BLACKHOLE
    (21, BG2D_ID_SPECIAL), // MAP_ID_SPECIAL
    (22, 17),              // MAP_ID_FINAL
    (23, 40),              // MAP_ID_INTRO
    (24, BG2D_ID_TITLE),   // MAP_ID_TITLE
    (25, 42),              // MAP_ID_CONTINUE
    (28, 43),              // MAP_ID_CREDITS
    (29, 44),              // MAP_ID_TRAINING
];

/// Per-bg `shadowheight` (BGS.ASM set_bg blocks): the SNES-world Y of the
/// ground plane drop shadows flatten onto (MDRAWLIS.MC:1416-1432 rotates the
/// shadow at y = shadowheight). Every set_bg in BGS.ASM says `shadowheight 0`
/// except the Nucleus interiors bg_1_3d / bg_1_3da (BGS.ASM:280/292), which
/// use `nucleusheight = (100/2) << boss8_scale` = 50 << 3 = 400
/// (STRATEQU.INC:699, boss8_scale STRATEQU.INC:298). Keyed by the same
/// setbg-operand bg ids as [`BG_DEFS`] (bg_1_3c = 9, bg_1_3e = 12 anchor the
/// BGS.ASM bglists ordering: bg_1_3d = 10, bg_1_3da = 11).
pub static BG_SHADOW_HEIGHTS: &[(u16, f32)] = &[(10, 400.0), (11, 400.0)];

/// Shadow ground-plane height (SNES world Y, +down) for a bg id; 0 for
/// every bg without an explicit BGS.ASM `shadowheight`.
pub fn shadow_height_for_bg(bg_id: u16) -> f32 {
    BG_SHADOW_HEIGHTS
        .iter()
        .find(|(id, _)| *id == bg_id)
        .map_or(0.0, |&(_, h)| h)
}

// ---------------------------------------------------------------------------
// SNES decode helpers (pure; shared with tests)
// ---------------------------------------------------------------------------

pub fn decode_2bpp_tile(src: &[u8], dst8x8: &mut [u8; 64]) {
    for row in 0..8 {
        let p0 = src[row * 2];
        let p1 = src[row * 2 + 1];
        for bit in (0..8).rev() {
            let v = ((p0 >> bit) & 1) | (((p1 >> bit) & 1) << 1);
            dst8x8[(7 - bit) + row * 8] = v;
        }
    }
}

pub fn decode_4bpp_tile(src: &[u8], dst8x8: &mut [u8; 64]) {
    for row in 0..8 {
        let p0 = src[row * 2];
        let p1 = src[row * 2 + 1];
        let p2 = src[16 + row * 2];
        let p3 = src[16 + row * 2 + 1];
        for bit in (0..8).rev() {
            let v = ((p0 >> bit) & 1)
                | (((p1 >> bit) & 1) << 1)
                | (((p2 >> bit) & 1) << 2)
                | (((p3 >> bit) & 1) << 3);
            dst8x8[(7 - bit) + row * 8] = v;
        }
    }
}

/// BGR555 -> RGB888 using the source display's five-bit replication.
pub fn cgram_color(col: &[u8], index: usize) -> [u8; 3] {
    bgr555_color(cgram_word(col, index))
}

fn cgram_word(col: &[u8], index: usize) -> u16 {
    col[index * 2] as u16 | ((col[index * 2 + 1] as u16) << 8)
}

fn bgr555_color(word: u16) -> [u8; 3] {
    let expand = |component: u16| -> u8 {
        let five_bits = component & 31;
        ((five_bits << 3) | (five_bits >> 2)) as u8
    };
    [expand(word), expand(word >> 5), expand(word >> 10)]
}

fn cgram_palette(col: &[u8], palette: usize) -> [u16; COLORS_PER_PALETTE] {
    std::array::from_fn(|index| cgram_word(col, palette * COLORS_PER_PALETTE + index))
}

/// Return the authored palette used by native title polygon materials.
pub fn title_polygon_palette(col: &[u8]) -> [u16; COLORS_PER_PALETTE] {
    cgram_palette(col, TITLE_POLYGON_PALETTE)
}

fn title_cgram_color(col: &[u8], index: usize) -> [u8; 3] {
    if index / COLORS_PER_PALETTE == TITLE_GAME_PALETTE {
        bgr555_color(crate::shapes::NIGHT_PALETTE[index % COLORS_PER_PALETTE])
    } else {
        cgram_color(col, index)
    }
}

/// Tilemap entry for map pixel (mx, my). 8 KB .SCR = 64x64 tiles stored as
/// four 32x32 screens (TL, TR, BL, BR); 2 KB = one 32x32 screen.
pub fn scr_entry(scr: &[u8], mx: usize, my: usize) -> u16 {
    let quads_per_row = if scr.len() >= 8192 { 2 } else { 1 };
    let quad = (my / 256) * quads_per_row + (mx / 256) % quads_per_row;
    let off = quad * 2048 + (((my % 256) / 8) * 32 + ((mx % 256) / 8)) * 2;
    if off + 1 >= scr.len() {
        return 0;
    }
    scr[off] as u16 | ((scr[off + 1] as u16) << 8)
}

struct ComposedBg {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    palette_four_pixels: Vec<u8>,
    palette_four: [u16; COLORS_PER_PALETTE],
}

fn compose_bg_with_palette_trace(
    cgx: &[u8],
    scr: &[u8],
    col: &[u8],
    cgx3: Option<&[u8]>,
    scr3: Option<&[u8]>,
    vofs: i32,
    vofs3: i32,
    sky: bool,
) -> Option<ComposedBg> {
    if scr.len() < 2048 || col.len() < 512 {
        return None;
    }

    let n2 = cgx.len() / 32; // 4bpp: 32 bytes/tile
    let mut px2 = vec![0u8; n2 * 64];
    for t in 0..n2 {
        let mut tile = [0u8; 64];
        decode_4bpp_tile(&cgx[t * 32..], &mut tile);
        px2[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }

    let mut n3 = 0usize;
    let mut px3: Vec<u8> = Vec::new();
    let scr3_ok = matches!(scr3, Some(s) if s.len() >= 2048);
    if let (Some(c3), true) = (cgx3, scr3_ok) {
        n3 = c3.len() / 16; // 2bpp: 16 bytes/tile
        px3 = vec![0u8; n3 * 64];
        for t in 0..n3 {
            let mut tile = [0u8; 64];
            decode_2bpp_tile(&c3[t * 16..], &mut tile);
            px3[t * 64..t * 64 + 64].copy_from_slice(&tile);
        }
    }

    let map_h2: i32 = if scr.len() >= 8192 { 512 } else { 256 };
    let map_h3: i32 = match scr3 {
        Some(s) if s.len() >= 8192 => 512,
        _ => 256,
    };

    let backdrop = cgram_color(col, 0);

    let out_w = if sky {
        if scr.len() >= 8192 {
            512
        } else {
            256
        }
    } else {
        BG2D_W
    };
    let out_h = if sky { map_h2 as usize } else { BG2D_H };

    let mut rgba = vec![0u8; out_w * out_h * 4];
    let mut palette_four_pixels = vec![STATIC_PALETTE_PIXEL; out_w * out_h];

    for y in 0..out_h {
        // Flip vertically so GL row 0 = picture bottom (standard UVs)
        let row_off = (out_h - 1 - y) * out_w * 4;

        let mut my2 = (y as i32 + if sky { 0 } else { vofs }) % map_h2;
        if my2 < 0 {
            my2 += map_h2;
        }
        let mut my3 = (y as i32 + vofs3) % map_h3;
        if my3 < 0 {
            my3 += map_h3;
        }
        let (my2, my3) = (my2 as usize, my3 as usize);

        for x in 0..out_w {
            let mut rgb = backdrop;
            let mut palette_four_pixel = STATIC_PALETTE_PIXEL;

            // --- BG2 (4bpp) ---
            {
                let e = scr_entry(scr, x, my2);
                let tile = (e & 0x3FF) as usize;
                let pal = ((e >> 10) & 7) as usize;
                let mut r = my2 & 7;
                let mut c = x & 7;
                if e & 0x8000 != 0 {
                    r = 7 - r;
                }
                if e & 0x4000 != 0 {
                    c = 7 - c;
                }
                if tile < n2 {
                    let v = px2[tile * 64 + r * 8 + c];
                    if v != 0 {
                        rgb = cgram_color(col, pal * 16 + v as usize);
                        if pal == BACKGROUND_FADE_PALETTE {
                            palette_four_pixel = v;
                        }
                    }
                }
            }

            // --- optional BG3 overlay (2bpp; never present on sky defs) ---
            if !px3.is_empty() && !sky {
                let scr3 = scr3.unwrap();
                let e = scr_entry(scr3, x, my3);
                let tile = (e & 0x3FF) as usize;
                let pal = ((e >> 10) & 7) as usize;
                let mut r = my3 & 7;
                let mut c = x & 7;
                if e & 0x8000 != 0 {
                    r = 7 - r;
                }
                if e & 0x4000 != 0 {
                    c = 7 - c;
                }
                if tile < n3 {
                    let v = px3[tile * 64 + r * 8 + c];
                    if v != 0 {
                        rgb = cgram_color(col, pal * 4 + v as usize);
                        palette_four_pixel = STATIC_PALETTE_PIXEL;
                    }
                }
            }

            let pixel = (out_h - 1 - y) * out_w + x;
            palette_four_pixels[pixel] = palette_four_pixel;
            let px = &mut rgba[row_off + x * 4..row_off + x * 4 + 4];
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
            px[3] = 255;
        }
    }

    Some(ComposedBg {
        rgba,
        width: out_w,
        height: out_h,
        palette_four_pixels,
        palette_four: cgram_palette(col, BACKGROUND_FADE_PALETTE),
    })
}

/// CPU half of `build_bg_texture`: compose a gameplay background from raw
/// asset bytes into a bottom-up RGBA image. Returns `(rgba, out_w, out_h)`;
/// sky layers compose the full wrapping tilemap.
pub fn compose_bg(
    cgx: &[u8],
    scr: &[u8],
    col: &[u8],
    cgx3: Option<&[u8]>,
    scr3: Option<&[u8]>,
    vofs: i32,
    vofs3: i32,
    sky: bool,
) -> Option<(Vec<u8>, usize, usize)> {
    let composed = compose_bg_with_palette_trace(cgx, scr, col, cgx3, scr3, vofs, vofs3, sky)?;
    Some((composed.rgba, composed.width, composed.height))
}

const TILE_PRIORITY: u16 = 1 << 13;

/// Compose the title's Mode-1 priority planes around the low-priority BG1
/// SuperFX framebuffer. BG2 CP and BG3 TI-3 low tiles sit behind 3D; their
/// high tiles sit in front. With BGMODE bit 3 set, BG3-high is above BG2-high,
/// while BG2-low is above BG3-low.
pub fn compose_title_layers(
    ti_cgx: &[u8],
    ti_scr: &[u8],
    cp_cgx: &[u8],
    cp_scr: &[u8],
    col: &[u8],
) -> Option<(Vec<u8>, Vec<u8>)> {
    if ti_scr.len() < 2048 || cp_scr.len() < 2048 || col.len() < 512 {
        return None;
    }

    let n_ti = ti_cgx.len() / 16; // 2bpp: 16 bytes/tile
    let n_cp = cp_cgx.len() / 32; // 4bpp: 32 bytes/tile
    let mut ti_px = vec![0u8; n_ti * 64];
    let mut cp_px = vec![0u8; n_cp * 64];
    for t in 0..n_ti {
        let mut tile = [0u8; 64];
        decode_2bpp_tile(&ti_cgx[t * 16..], &mut tile);
        ti_px[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }
    for t in 0..n_cp {
        let mut tile = [0u8; 64];
        decode_4bpp_tile(&cp_cgx[t * 32..], &mut tile);
        cp_px[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }

    let mut background = vec![0u8; BG2D_W * BG2D_H * 4];
    let mut foreground = vec![0u8; BG2D_W * BG2D_H * 4];

    for y in 0..BG2D_H {
        // Flip vertically so GL row 0 = picture bottom (standard UVs)
        let row_off = (BG2D_H - 1 - y) * BG2D_W * 4;

        let by2 = (y + 1) % TITLE_TILEMAP_EXTENT; // BG2 scroll 257 -> effective +1
        let by3 = (y + 9) % TITLE_TILEMAP_EXTENT; // BG3 vertical scroll 9

        for x in 0..BG2D_W {
            let mut cp_pixel = None;
            let mut ti_pixel = None;

            // --- BG2 (CP backdrop, 4bpp) ---
            {
                let me = ((by2 / 8) * 32 + (x / 8)) * 2;
                let e = cp_scr[me] as u16 | ((cp_scr[me + 1] as u16) << 8);
                let tile = (e & 0x3FF) as usize;
                let pal = ((e >> 10) & 7) as usize;
                let mut r = by2 & 7;
                let mut c = x & 7;
                if e & 0x8000 != 0 {
                    r = 7 - r;
                }
                if e & 0x4000 != 0 {
                    c = 7 - c;
                }
                if tile < n_cp {
                    let v = cp_px[tile * 64 + r * 8 + c];
                    if v != 0 {
                        cp_pixel = Some((
                            title_cgram_color(col, pal * 16 + v as usize),
                            e & TILE_PRIORITY != 0,
                        ));
                    }
                }
            }

            // --- BG3 (TI-3 logo, 2bpp) over the top ---
            {
                let bx3 = (x + TITLE_BG3_HORIZONTAL_SCROLL % SOURCE_TILEMAP_EXTENT)
                    % TITLE_TILEMAP_EXTENT;
                let me = ((by3 / 8) * 32 + (bx3 / 8)) * 2;
                let e = ti_scr[me] as u16 | ((ti_scr[me + 1] as u16) << 8);
                let tile = (e & 0x3FF) as usize;
                let pal = ((e >> 10) & 7) as usize;
                let mut r = by3 & 7;
                let mut c = bx3 & 7;
                if e & 0x8000 != 0 {
                    r = 7 - r;
                }
                if e & 0x4000 != 0 {
                    c = 7 - c;
                }
                if tile < n_ti {
                    let v = ti_px[tile * 64 + r * 8 + c];
                    if v != 0 {
                        ti_pixel = Some((
                            cgram_color(col, pal * 4 + v as usize),
                            e & TILE_PRIORITY != 0,
                        ));
                    }
                }
            }

            // Low-priority order is BG2 over BG3 over the black backdrop.
            let mut background_rgb = [0u8; 3];
            if let Some((rgb, false)) = ti_pixel {
                background_rgb = rgb;
            }
            if let Some((rgb, false)) = cp_pixel {
                background_rgb = rgb;
            }
            let background_pixel = &mut background[row_off + x * 4..row_off + x * 4 + 4];
            background_pixel[..3].copy_from_slice(&background_rgb);
            background_pixel[3] = 255;

            // High-priority order is BG3 over BG2 over the live BG1 image.
            let mut foreground_rgb = None;
            if let Some((rgb, true)) = cp_pixel {
                foreground_rgb = Some(rgb);
            }
            if let Some((rgb, true)) = ti_pixel {
                foreground_rgb = Some(rgb);
            }
            if let Some(rgb) = foreground_rgb {
                let foreground_pixel = &mut foreground[row_off + x * 4..row_off + x * 4 + 4];
                foreground_pixel[..3].copy_from_slice(&rgb);
                foreground_pixel[3] = 255;
            }
        }
    }

    Some((background, foreground))
}

/// CPU half of `build_title_texture`: compose the complete static title
/// image with the same tile-priority order used by [`compose_title_layers`].
pub fn compose_title(
    ti_cgx: &[u8],
    ti_scr: &[u8],
    cp_cgx: &[u8],
    cp_scr: &[u8],
    col: &[u8],
) -> Option<Vec<u8>> {
    let (mut background, foreground) = compose_title_layers(ti_cgx, ti_scr, cp_cgx, cp_scr, col)?;
    for (background_pixel, foreground_pixel) in background
        .chunks_exact_mut(4)
        .zip(foreground.chunks_exact(4))
    {
        if foreground_pixel[3] != 0 {
            background_pixel.copy_from_slice(foreground_pixel);
        }
    }
    Some(background)
}

fn title_live_background(mut low_priority_tiles: Vec<u8>) -> Vec<u8> {
    let right = TITLE_FRAMEBUFFER_LEFT + TITLE_FRAMEBUFFER_WIDTH;
    let bottom = TITLE_FRAMEBUFFER_TOP + TITLE_FRAMEBUFFER_HEIGHT;
    for y in 0..BG2D_H {
        for x in 0..BG2D_W {
            if !(TITLE_FRAMEBUFFER_LEFT..right).contains(&x)
                || !(TITLE_FRAMEBUFFER_TOP..bottom).contains(&y)
            {
                let offset = (BG2D_H - 1 - y) * BG2D_W * 4 + x * 4;
                low_priority_tiles[offset..offset + 4].fill(0);
            }
        }
    }
    low_priority_tiles
}

// ---------------------------------------------------------------------------
// GL runtime state
// ---------------------------------------------------------------------------

fn load_file(base: &Path, rel: &str) -> Option<Vec<u8>> {
    match std::fs::read(base.join(rel)) {
        Ok(v) => Some(v),
        Err(_) => {
            eprintln!("Bg2d: cannot open {}", base.join(rel).display());
            None
        }
    }
}

fn upload_rgba(gpu: &mut Gpu, rgba: &[u8], w: usize, h: usize) -> Option<TextureId> {
    // Background tilemaps wrap independently from clamped portraits, glyphs,
    // and source bitmaps.
    Some(gpu.create_texture_rgba_repeat(w as u32, h as u32, rgba))
}

/// Apply the source `fadepalto_l` steps not yet reflected in `palette`.
/// `previous` and `current` are byte counters, so cursor 30 copies color 15,
/// cursor 28 copies color 14, and cursor 2 copies color 1. Color 0 is never
/// touched. Returns whether any entry changed.
fn apply_palette_four_steps(
    palette: &mut [u16; COLORS_PER_PALETTE],
    target: PaletteFadeTarget,
    previous: u16,
    current: u16,
) -> bool {
    let source = background_fade_palette_bgr(target);
    let mut cursor = previous.min(PALETTE_FADE_COUNTER_START) & !1;
    let current = current.min(cursor) & !1;
    let mut changed = false;
    while cursor > current {
        let index = usize::from(cursor / 2);
        changed |= palette[index] != source[index];
        palette[index] = source[index];
        cursor -= 2;
    }
    changed
}

fn recolor_palette_four(
    base: &[u8],
    pixels: &[u8],
    palette: &[u16; COLORS_PER_PALETTE],
) -> Vec<u8> {
    let mut rgba = base.to_vec();
    for (pixel, &palette_index) in pixels.iter().enumerate() {
        if palette_index == STATIC_PALETTE_PIXEL {
            continue;
        }
        let rgb = bgr555_color(palette[usize::from(palette_index)]);
        let offset = pixel * 4;
        rgba[offset..offset + 3].copy_from_slice(&rgb);
    }
    rgba
}

pub struct Bg2d {
    base_dir: PathBuf,
    title_tex: Option<TextureId>,
    title_foreground_tex: Option<TextureId>,
    title_polygon_palette: [u16; COLORS_PER_PALETTE],
    def_tex: Vec<Option<TextureId>>,
    def_tried: Vec<bool>,
    /// Original composite plus the final per-pixel color index for pixels
    /// sourced from CGRAM background palette row 4.
    def_base_rgba: Vec<Option<Vec<u8>>>,
    def_palette_four_pixels: Vec<Option<Vec<u8>>>,
    def_palette_four_base: Vec<Option<[u16; COLORS_PER_PALETTE]>>,
    /// Tilemap pixel size for sky (camera-coupled) textures; 0 for static
    /// pre-baked 256x224 composites.
    def_map_w: Vec<i32>,
    def_map_h: Vec<i32>,
    warned_bgs: u64,
    // g_currentbg staleness workaround state (statics in Bg2d_Render).
    prev_map: u32,
    bg_at_map_start: u16,
    /// `mbhole` BGS-init state: testk2/testk3/testk4 reset whenever a bhole
    /// background is entered, including re-entering the same id on a new map.
    bhole_key: Option<(u32, u8)>,
    bhole_start_frame: u16,
    /// Live palette-row state. A background load resets it from that
    /// background's COL asset; subsequent sea/ground commands replace one
    /// entry per game tick exactly like `fadepalto_l`.
    palette_four_key: Option<(u32, u8)>,
    palette_four_live: [u16; COLORS_PER_PALETTE],
    palette_four_last_target: Option<PaletteFadeTarget>,
    palette_four_last_num: u16,
    previous_vertical_offsets: Option<[i16; BG2_VERTICAL_OFFSET_COLUMNS]>,
    current_vertical_offsets: Option<[i16; BG2_VERTICAL_OFFSET_COLUMNS]>,
    previous_horizontal_offsets: Option<[i16; BG2_HORIZONTAL_OFFSET_ROWS]>,
    current_horizontal_offsets: Option<[i16; BG2_HORIZONTAL_OFFSET_ROWS]>,
}

impl Bg2d {
    pub fn new(gpu: &mut Gpu, base_dir: &Path) -> Self {
        let n = BG_DEFS.len();
        let mut bg = Bg2d {
            base_dir: base_dir.to_path_buf(),
            title_tex: None,
            title_foreground_tex: None,
            title_polygon_palette: crate::shapes::NIGHT_PALETTE,
            def_tex: vec![None; n],
            def_tried: vec![false; n],
            def_base_rgba: vec![None; n],
            def_palette_four_pixels: vec![None; n],
            def_palette_four_base: vec![None; n],
            def_map_w: vec![0; n],
            def_map_h: vec![0; n],
            warned_bgs: 0,
            prev_map: 0xFFFF_FFFF,
            bg_at_map_start: 0,
            bhole_key: None,
            bhole_start_frame: 0,
            palette_four_key: None,
            palette_four_live: [0; COLORS_PER_PALETTE],
            palette_four_last_target: None,
            palette_four_last_num: 0,
            previous_vertical_offsets: None,
            current_vertical_offsets: None,
            previous_horizontal_offsets: None,
            current_horizontal_offsets: None,
        };
        bg.build_title_texture(gpu);
        bg
    }

    pub fn has_title(&self) -> bool {
        self.title_tex.is_some()
    }

    /// Advance the fixed-tick offset-table history used only by the smooth HD
    /// presentation. Source-resolution captures continue to consume the
    /// current integer tables from [`FrameInputs`] directly.
    pub fn advance_offset_tables(
        &mut self,
        vertical: Option<[i16; BG2_VERTICAL_OFFSET_COLUMNS]>,
        horizontal: Option<[i16; BG2_HORIZONTAL_OFFSET_ROWS]>,
    ) {
        self.previous_vertical_offsets = self.current_vertical_offsets;
        self.current_vertical_offsets = vertical;
        self.previous_horizontal_offsets = self.current_horizontal_offsets;
        self.current_horizontal_offsets = horizontal;
    }

    pub fn snap_offset_tables(&mut self) {
        self.previous_vertical_offsets = self.current_vertical_offsets;
        self.previous_horizontal_offsets = self.current_horizontal_offsets;
    }

    pub fn title_polygon_palette(&self) -> &[u16; COLORS_PER_PALETTE] {
        &self.title_polygon_palette
    }

    fn build_title_texture(&mut self, gpu: &mut Gpu) {
        let ti_cgx = load_file(&self.base_dir, "data/title/TI-3-US.CGX");
        let ti_scr = load_file(&self.base_dir, "data/title/TI-3-US.SCR");
        let cp_cgx = load_file(&self.base_dir, "data/title/CP.CGX");
        let cp_scr = load_file(&self.base_dir, "data/title/CP.SCR");
        let col = load_file(&self.base_dir, "data/title/CP-US.COL");
        let (Some(ti_cgx), Some(ti_scr), Some(cp_cgx), Some(cp_scr), Some(col)) =
            (ti_cgx, ti_scr, cp_cgx, cp_scr, col)
        else {
            eprintln!("Bg2d: title assets missing/short, using fallback backdrop");
            return;
        };
        match compose_title_layers(&ti_cgx, &ti_scr, &cp_cgx, &cp_scr, &col) {
            Some((low_priority_tiles, foreground)) => {
                // The 224-by-192 BG1 framebuffer exposes low-priority title
                // tiles only inside its centered playfield. The surrounding
                // border remains black; high-priority title tiles can still
                // draw over it in the later foreground pass.
                let live_background = title_live_background(low_priority_tiles);
                self.title_tex = upload_rgba(gpu, &live_background, BG2D_W, BG2D_H);
                self.title_foreground_tex = upload_rgba(gpu, &foreground, BG2D_W, BG2D_H);
                self.title_polygon_palette = title_polygon_palette(&col);
            }
            None => eprintln!("Bg2d: title assets missing/short, using fallback backdrop"),
        }
    }

    fn build_bg_texture(&mut self, gpu: &mut Gpu, idx: usize) {
        let def = &BG_DEFS[idx];
        let cgx = load_file(&self.base_dir, def.cgx);
        let scr = load_file(&self.base_dir, def.scr);
        let col = load_file(&self.base_dir, def.col);
        let cgx3 = def.cgx3.and_then(|p| load_file(&self.base_dir, p));
        let scr3 = def.scr3.and_then(|p| load_file(&self.base_dir, p));

        let (Some(cgx), Some(scr), Some(col)) = (cgx, scr, col) else {
            eprintln!(
                "Bg2d: {} assets missing/short, using fallback backdrop",
                def.name
            );
            return;
        };

        match compose_bg_with_palette_trace(
            &cgx,
            &scr,
            &col,
            cgx3.as_deref(),
            scr3.as_deref(),
            def.vofs,
            def.vofs3,
            def.sky,
        ) {
            Some(composed) => {
                self.def_tex[idx] =
                    upload_rgba(gpu, &composed.rgba, composed.width, composed.height);
                self.def_base_rgba[idx] = Some(composed.rgba);
                self.def_palette_four_pixels[idx] = Some(composed.palette_four_pixels);
                self.def_palette_four_base[idx] = Some(composed.palette_four);
                if def.sky {
                    self.def_map_w[idx] = composed.width as i32;
                    self.def_map_h[idx] = composed.height as i32;
                }
            }
            None => {
                eprintln!(
                    "Bg2d: {} assets missing/short, using fallback backdrop",
                    def.name
                );
            }
        }
    }

    /// Lazily build the texture for a bg id; returns the def index or None.
    fn layer_index_for_id(&mut self, gpu: &mut Gpu, id: u8) -> Option<usize> {
        let idx = BG_DEFS.iter().position(|d| d.id == id)?;
        if !self.def_tried[idx] {
            self.def_tried[idx] = true;
            self.build_bg_texture(gpu, idx);
        }
        Some(idx)
    }

    fn sync_palette_four(
        &mut self,
        gpu: &mut Gpu,
        idx: usize,
        key: (u32, u8),
        target: Option<PaletteFadeTarget>,
        remaining: u16,
    ) {
        let (Some(texture), Some(base), Some(pixels), Some(initial_palette)) = (
            self.def_tex[idx],
            self.def_base_rgba[idx].as_deref(),
            self.def_palette_four_pixels[idx].as_deref(),
            self.def_palette_four_base[idx],
        ) else {
            return;
        };

        if self.palette_four_key != Some(key) {
            self.palette_four_key = Some(key);
            self.palette_four_live = initial_palette;
            self.palette_four_last_target = target;
            self.palette_four_last_num = remaining;
            gpu.update_texture(texture, base);
            return;
        }

        let previous =
            if target != self.palette_four_last_target || remaining > self.palette_four_last_num {
                PALETTE_FADE_COUNTER_START
            } else {
                self.palette_four_last_num
            };
        let changed = target.is_some_and(|target| {
            apply_palette_four_steps(&mut self.palette_four_live, target, previous, remaining)
        });
        self.palette_four_last_target = target;
        self.palette_four_last_num = remaining;

        if changed {
            let rgba = recolor_palette_four(base, pixels, &self.palette_four_live);
            gpu.update_texture(texture, &rgba);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_quad(
        &self,
        gpu: &mut Gpu,
        proj: &[f32; 16],
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
        use_texture: u32,
        tex: TextureId,
    ) {
        let verts = [
            Vertex2 {
                pos: [x, y],
                uv: [u0, v0],
            },
            Vertex2 {
                pos: [x + w, y],
                uv: [u1, v0],
            },
            Vertex2 {
                pos: [x + w, y + h],
                uv: [u1, v1],
            },
            Vertex2 {
                pos: [x, y + h],
                uv: [u0, v1],
            },
        ];
        gpu.push_overlay_fan(&verts, proj, &IDENTITY, color, use_texture, None, tex);
    }

    /// Starfield-ish dark gradient fallback (deterministic star placement).
    fn draw_fallback_backdrop(&self, gpu: &mut Gpu, proj: &[f32; 16], w: i32, h: i32) {
        // Vertical gradient: deep space black at top -> dark blue near bottom
        let bands = 8;
        for i in 0..bands {
            let t = i as f32 / (bands - 1) as f32;
            let color = [
                0.01 + 0.03 * (1.0 - t),
                0.01 + 0.03 * (1.0 - t),
                0.05 + 0.10 * (1.0 - t),
                1.0,
            ];
            let band_h = h as f32 / bands as f32;
            self.push_quad(
                gpu,
                proj,
                0.0,
                band_h * i as f32,
                w as f32,
                band_h + 1.0,
                0.0,
                0.0,
                1.0,
                1.0,
                color,
                0,
                WHITE_TEX,
            );
        }

        // Stars: deterministic pseudo-random spread
        let star = [0.85, 0.85, 0.95, 1.0];
        let mut seed: u32 = 0x123_4567;
        let sx = w as f32 / 256.0;
        let sy = h as f32 / 224.0;
        for i in 0..64 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let px = ((seed >> 8) & 0xFF) as i32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let py = ((seed >> 8) % 224) as i32;
            let size = if i & 7 == 0 { 2.0 } else { 1.0 };
            self.push_quad(
                gpu,
                proj,
                px as f32 * sx,
                py as f32 * sy,
                size * sx,
                size * sy,
                0.0,
                0.0,
                1.0,
                1.0,
                star,
                0,
                WHITE_TEX,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_layer_texture(
        &self,
        gpu: &mut Gpu,
        proj: &[f32; 16],
        tex: Option<TextureId>,
        w: i32,
        h: i32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        let Some(tex) = tex else {
            self.draw_fallback_backdrop(gpu, proj, w, h);
            return;
        };
        self.push_quad(
            gpu,
            proj,
            0.0,
            0.0,
            w as f32,
            h as f32,
            u0,
            v0,
            u1,
            v1,
            [1.0, 1.0, 1.0, 1.0],
            1,
            tex,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_ground_rolled_texture(
        &self,
        gpu: &mut Gpu,
        proj: &[f32; 16],
        tex: TextureId,
        w: i32,
        h: i32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        map_width: f32,
        map_height: f32,
        vertical_offsets: [f32; BG2_VERTICAL_OFFSET_COLUMNS],
        horizontal_offsets: [f32; BG2_HORIZONTAL_OFFSET_ROWS],
    ) {
        let uniform_vertical_offset = vertical_offsets
            .iter()
            .all(|offset| *offset == vertical_offsets[0]);
        let column_count = if uniform_vertical_offset {
            1
        } else {
            BG2_VERTICAL_OFFSET_COLUMNS
        };
        let mut vertices = Vec::with_capacity(BG2_HORIZONTAL_OFFSET_ROWS * column_count * 6);
        // Overlay coordinates are bottom-up; walk the authored top-down
        // display rows in reverse so each value remains attached to its
        // original raster line.
        for (row, horizontal_offset) in horizontal_offsets.into_iter().rev().enumerate() {
            let top_fraction = row as f32 / BG2_HORIZONTAL_OFFSET_ROWS as f32;
            let bottom_fraction = (row + 1) as f32 / BG2_HORIZONTAL_OFFSET_ROWS as f32;
            let y0 = h as f32 * top_fraction;
            let y1 = h as f32 * bottom_fraction;
            let row_v0 = v0 + (v1 - v0) * top_fraction;
            let row_v1 = v0 + (v1 - v0) * bottom_fraction;
            let horizontal_uv = horizontal_offset / map_width;

            for column in 0..column_count {
                let vertical_offset = if uniform_vertical_offset {
                    vertical_offsets[0]
                } else {
                    vertical_offsets[column]
                };
                let left_fraction = column as f32 / column_count as f32;
                let right_fraction = (column + 1) as f32 / column_count as f32;
                let x0 = w as f32 * left_fraction;
                let x1 = w as f32 * right_fraction;
                let column_u0 = u0 + (u1 - u0) * left_fraction + horizontal_uv;
                let column_u1 = u0 + (u1 - u0) * right_fraction + horizontal_uv;
                let vertical_uv = -vertical_offset / map_height;
                let cell_v0 = row_v0 + vertical_uv;
                let cell_v1 = row_v1 + vertical_uv;
                vertices.extend_from_slice(&[
                    Vertex2 {
                        pos: [x0, y0],
                        uv: [column_u0, cell_v0],
                    },
                    Vertex2 {
                        pos: [x1, y0],
                        uv: [column_u1, cell_v0],
                    },
                    Vertex2 {
                        pos: [x1, y1],
                        uv: [column_u1, cell_v1],
                    },
                    Vertex2 {
                        pos: [x0, y0],
                        uv: [column_u0, cell_v0],
                    },
                    Vertex2 {
                        pos: [x1, y1],
                        uv: [column_u1, cell_v1],
                    },
                    Vertex2 {
                        pos: [x0, y1],
                        uv: [column_u0, cell_v1],
                    },
                ]);
            }
        }
        gpu.push_overlay_tris(
            &vertices,
            proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            tex,
        );
    }

    /// Draw the title tilemap priorities that sit above the centered BG1
    /// SuperFX framebuffer. The complementary low-priority plane is clipped
    /// to that framebuffer's 224-by-192 source playfield.
    pub fn render_title_foreground(&self, gpu: &mut Gpu, screen_width: i32, screen_height: i32) {
        let Some(texture) = self.title_foreground_tex else {
            return;
        };
        let projection = ortho(screen_width as f32, screen_height as f32);
        self.push_quad(
            gpu,
            &projection,
            0.0,
            0.0,
            screen_width as f32,
            screen_height as f32,
            0.0,
            0.0,
            1.0,
            1.0,
            [1.0, 1.0, 1.0, 1.0],
            1,
            texture,
        );
    }

    /// Draw one triangle pair per SNES scanline so each row can carry the
    /// independently generated BG2HOFS word from `mbhole` (MHOFS.MC:307-401).
    /// All rows share one GPU draw; the repeat sampler performs SNES tilemap
    /// wrap for offsets outside the composed texture.
    #[allow(clippy::too_many_arguments)]
    fn draw_bhole_texture(
        &self,
        gpu: &mut Gpu,
        proj: &[f32; 16],
        tex: TextureId,
        screen_width: i32,
        screen_height: i32,
        map_width: f32,
        u_base: f32,
        v0: f32,
        v1: f32,
        phase: i16,
    ) {
        let offsets = bhole_line_offsets(phase);
        let mut verts = Vec::with_capacity(BG2D_H * 6);
        for (row, &offset) in offsets.iter().enumerate() {
            let fy0 = row as f32 / BG2D_H as f32;
            let fy1 = (row + 1) as f32 / BG2D_H as f32;
            let y0 = fy0 * screen_height as f32;
            let y1 = fy1 * screen_height as f32;
            let tv0 = v0 + (v1 - v0) * fy0;
            let tv1 = v0 + (v1 - v0) * fy1;
            let tu0 = u_base + offset as f32 / map_width;
            let tu1 = tu0 + BG2D_W as f32 / map_width;
            let x1 = screen_width as f32;
            verts.extend_from_slice(&[
                Vertex2 {
                    pos: [0.0, y0],
                    uv: [tu0, tv0],
                },
                Vertex2 {
                    pos: [x1, y0],
                    uv: [tu1, tv0],
                },
                Vertex2 {
                    pos: [x1, y1],
                    uv: [tu1, tv1],
                },
                Vertex2 {
                    pos: [0.0, y0],
                    uv: [tu0, tv0],
                },
                Vertex2 {
                    pos: [x1, y1],
                    uv: [tu1, tv1],
                },
                Vertex2 {
                    pos: [0.0, y1],
                    uv: [tu0, tv1],
                },
            ]);
        }
        gpu.push_overlay_tris(&verts, proj, &IDENTITY, [1.0, 1.0, 1.0, 1.0], 1, None, tex);
    }

    /// SNES BG2 scroll coupling (GSTRATS.ASM calcbgscroll_l): compute the UV
    /// window into a sky (von/hon) tilemap texture. Mirrors `sky_uv_window`.
    fn sky_uv_window(
        &self,
        idx: usize,
        with_camera: bool,
        transform: &Transform,
        inputs: &FrameInputs,
    ) -> (f32, f32, f32, f32) {
        let def = &BG_DEFS[idx];
        let mw = self.def_map_w[idx] as f32;
        let mh = self.def_map_h[idx] as f32;
        let mut vofs = def.vofs as f32; // bg2Yscroll base (BGS.ASM)
        let has_typed_horizontal_offsets = with_camera && inputs.bg2_horizontal_offsets.is_some();
        let mut hofs = if has_typed_horizontal_offsets {
            0.0
        } else {
            inputs.bg2_xscroll as f32
        };

        if inputs.game_state == GameState::Briefing && def.id == BG2D_ID_CONTINUE {
            hofs += (inputs.control_type.panel_column() * CONTROLLER_PANEL_SIZE) as f32;
            vofs += (inputs.control_type.panel_row() * CONTROLLER_PANEL_SIZE) as f32;
        }

        if with_camera {
            let cam = transform.render_camera();
            let (rx, ry) = transform.render_camera_angles_f();

            // Vertical: the ROM's exact SLOPE (calcbgscroll_l,
            // GSTRATS.ASM:3190): scroll = -(viewrotx16*3/128) — LINEAR,
            // -6 px per 8-bit pitch unit — clamped [-56, 232] unless
            // nomaxbg2Yscroll, added to the bg2Yscroll base (def.vofs from
            // BGS.ASM). (The old focal*tan(pitch) curve diverged from the
            // ROM ramp as pitch grew — the "shadow creeps above the horizon"
            // class — and stays removed.)
            let vdelta = if inputs.source_resolution {
                let (_, rotation) = transform.source_camera();
                f32::from(source_vertical_camera_offset(
                    inputs.source_background_pitch.unwrap_or(rotation[0]),
                    inputs.nomax_bg2_yscroll,
                ))
            } else {
                let mut offset = -(rx * 6.0);
                if !inputs.nomax_bg2_yscroll {
                    offset = offset.clamp(-56.0, 232.0);
                }
                offset
            };
            vofs += vdelta;

            // Strict capture uses the authored signed shifts. The HD path
            // retains render-frame interpolation for smooth presentation.
            if !has_typed_horizontal_offsets {
                hofs += if inputs.source_resolution {
                    let (_, rotation) = transform.source_camera();
                    f32::from(source_horizontal_camera_offset(cam.x, rotation[1]))
                } else {
                    ry * 8.0 + (cam.x >> 16) as f32 / 8.0
                };
            }
        }

        // Texture rows were flipped at compose time (GL row 0 = map bottom),
        // so map row m sits at v = (mh - m)/mh; the window wraps via
        // GL_REPEAT like the SNES tilemap.
        let u0 = hofs / mw;
        let u1 = (hofs + BG2D_W as f32) / mw;
        let v1 = (mh - vofs) / mh; // quad top
        let v0 = (mh - vofs - BG2D_H as f32) / mh; // quad bottom
        (u0, v0, u1, v1)
    }

    /// Mirror of `Bg2d_Render` (per-frame background pass).
    pub fn render(
        &mut self,
        gpu: &mut Gpu,
        transform: &Transform,
        inputs: &FrameInputs,
        alpha: f32,
        screen_width: i32,
        screen_height: i32,
    ) {
        self.render_pass(gpu, transform, inputs, alpha, screen_width, screen_height);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_pass(
        &mut self,
        gpu: &mut Gpu,
        transform: &Transform,
        inputs: &FrameInputs,
        alpha: f32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let bg_active = inputs.bgflags & BGF_BG != 0;
        let mut draw = false;
        let mut couple = false; // apply the per-frame camera scroll coupling
        let mut idx: Option<usize> = None; // BG_DEFS index (None: title/fallback)
        let mut tex: Option<TextureId> = None; // None -> fallback backdrop
        let mut display_id: Option<u8> = None;

        match inputs.game_state {
            GameState::Title => {
                // Title map's setbg opcode selects BG_TITLE; draw the logo
                // layer even if the map script hasn't reached setbg yet.
                draw = true;
                tex = self.title_tex;
            }
            GameState::PlanetSelect => {
                // PLANETS.ASM map screen backdrop.
                draw = true;
                idx = self.layer_index_for_id(gpu, BG2D_ID_MAP);
                display_id = Some(BG2D_ID_MAP);
            }
            GameState::Briefing | GameState::Continue => {
                // CONT.ASM / bg_cont controller screen backdrop.
                draw = true;
                idx = self.layer_index_for_id(gpu, BG2D_ID_CONTINUE);
                display_id = Some(BG2D_ID_CONTINUE);
            }
            GameState::Ending => {
                draw = true;
                let id = (inputs.currentbg & 63) as u8;
                idx = self.layer_index_for_id(gpu, id);
                display_id = Some(id);
            }
            _ if bg_active
                || matches!(
                    inputs.game_state,
                    GameState::AttractIntro | GameState::Playing | GameState::Tally
                ) =>
            {
                // BGF_BG is transient, so also key off the playing state;
                // g_currentbg holds the last setbg operand. Snapshot it at
                // map load and use the per-map default until the map issues
                // its own setbg.
                if inputs.newmap != self.prev_map {
                    self.prev_map = inputs.newmap;
                    self.bg_at_map_start = inputs.currentbg;
                }

                draw = true;
                let mut id = (inputs.currentbg & 63) as u8;
                if inputs.currentbg == self.bg_at_map_start {
                    // No setbg from this map yet -> level's opening bg.
                    if let Some(&(_, bg)) = MAP_DEFAULT_BG.iter().find(|(m, _)| *m == inputs.newmap)
                    {
                        id = bg;
                    }
                }

                if id == BG2D_ID_TITLE {
                    tex = self.title_tex;
                } else {
                    display_id = Some(id);
                    idx = self.layer_index_for_id(gpu, id);
                    couple = true; // flight/tally: slave sky layers to the camera
                    let missing = match idx {
                        None => true,
                        Some(i) => self.def_tex[i].is_none(),
                    };
                    if missing && self.warned_bgs & (1u64 << id) == 0 {
                        self.warned_bgs |= 1u64 << id;
                        println!("Bg2d: no layer data for bg id {id}, using fallback backdrop");
                    }
                }
            }
            _ => {}
        }

        if !draw {
            return;
        }

        if matches!(
            inputs.game_state,
            GameState::AttractIntro | GameState::Playing | GameState::Tally
        ) {
            if let (Some(i), Some(id)) = (idx, display_id) {
                self.sync_palette_four(
                    gpu,
                    i,
                    (inputs.newmap, id),
                    inputs.pal_target,
                    inputs.palfade_num,
                );
            }
        } else {
            self.palette_four_key = None;
        }

        if let Some(i) = idx {
            tex = self.def_tex[i];
        }

        // Sky (von/hon) layers are full wrapping tilemaps: window into them.
        let (mut u0, mut v0, mut u1, mut v1) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
        if let Some(i) = idx {
            if tex.is_some() && self.def_map_w[i] > 0 {
                (u0, v0, u1, v1) = self.sky_uv_window(i, couple, transform, inputs);
            }
        }

        let proj = ortho(screen_width as f32, screen_height as f32);
        let bhole = matches!(display_id, Some(17 | 39 | BG2D_ID_SPECIAL));
        if bhole {
            let id = display_id.unwrap();
            let key = (inputs.newmap, id);
            if self.bhole_key != Some(key) {
                self.bhole_key = Some(key);
                self.bhole_start_frame = inputs.gameframe;
            }
            if let (Some(tex), Some(i)) = (tex, idx) {
                let map_width = self.def_map_w[i].max(BG2D_W as i32) as f32;
                let mut base_hofs = inputs.bg2_xscroll as f32;
                if couple {
                    let cam = transform.render_camera();
                    let (_, yaw) = transform.render_camera_angles_f();
                    base_hofs += yaw * 8.0 + (cam.x >> 16) as f32 / 8.0;
                }
                let elapsed = inputs.gameframe.wrapping_sub(self.bhole_start_frame) as u32;
                let phase = bhole_phase(elapsed + 1);
                self.draw_bhole_texture(
                    gpu,
                    &proj,
                    tex,
                    screen_width,
                    screen_height,
                    map_width,
                    base_hofs / map_width,
                    v0,
                    v1,
                    phase,
                );
                return;
            }
        } else {
            self.bhole_key = None;
        }
        if couple {
            if let (
                Some(texture),
                Some(layer_index),
                Some(vertical_offsets),
                Some(horizontal_offsets),
            ) = (
                tex,
                idx,
                inputs.bg2_vertical_offsets,
                inputs.bg2_horizontal_offsets,
            ) {
                let map_width = self.def_map_w[layer_index] as f32;
                let map_height = self.def_map_h[layer_index] as f32;
                let previous_vertical = (!inputs.source_resolution
                    && self.current_vertical_offsets == Some(vertical_offsets))
                .then_some(self.previous_vertical_offsets)
                .flatten();
                let previous_horizontal = (!inputs.source_resolution
                    && self.current_horizontal_offsets == Some(horizontal_offsets))
                .then_some(self.previous_horizontal_offsets)
                .flatten();
                let vertical_offsets = interpolate_offset_table(
                    previous_vertical.as_ref(),
                    &vertical_offsets,
                    if inputs.source_resolution { 1.0 } else { alpha },
                    map_height,
                );
                let horizontal_offsets = interpolate_offset_table(
                    previous_horizontal.as_ref(),
                    &horizontal_offsets,
                    if inputs.source_resolution { 1.0 } else { alpha },
                    map_width,
                );
                self.draw_ground_rolled_texture(
                    gpu,
                    &proj,
                    texture,
                    screen_width,
                    screen_height,
                    u0,
                    v0,
                    u1,
                    v1,
                    map_width,
                    map_height,
                    vertical_offsets,
                    horizontal_offsets,
                );
                return;
            }
        }
        self.draw_layer_texture(gpu, &proj, tex, screen_width, screen_height, u0, v0, u1, v1);
    }
}

#[cfg(test)]
mod bhole_tests {
    use super::*;

    #[test]
    fn strict_camera_scroll_keeps_source_integer_quantization() {
        assert_eq!(source_vertical_camera_offset(320, false), -7);
        assert_eq!(source_vertical_camera_offset((-320i16) as u16, false), 8);
        assert_eq!(
            source_vertical_camera_offset((-20_000i16) as u16, false),
            232
        );
        assert_eq!(source_vertical_camera_offset(20_000, false), -56);
        assert_eq!(source_horizontal_camera_offset(-31 << 16, 272), 4);
    }

    #[test]
    fn hd_offset_tables_interpolate_across_the_texture_wrap() {
        const MAP_PERIOD: f32 = 1_024.0;
        let previous = [1_020i16, 8];
        let current = [4i16, 12];

        assert_eq!(
            interpolate_offset_table(Some(&previous), &current, 0.5, MAP_PERIOD),
            [1_024.0, 10.0],
        );
        assert_eq!(
            interpolate_offset_table(Some(&previous), &current, 1.0, MAP_PERIOD),
            [1_028.0, 12.0],
        );
        assert_eq!(
            interpolate_offset_table(None, &current, 0.0, MAP_PERIOD),
            [4.0, 12.0],
        );
    }

    #[test]
    fn background_palette_walk_matches_the_retail_color_order() {
        let initial: [u16; COLORS_PER_PALETTE] = std::array::from_fn(|index| 1_000 + index as u16);
        let mut palette = initial;

        assert!(!apply_palette_four_steps(
            &mut palette,
            PaletteFadeTarget::Sea,
            PALETTE_FADE_COUNTER_START,
            PALETTE_FADE_COUNTER_START,
        ));
        assert_eq!(palette, initial, "arming the fade copies no color yet");

        assert!(apply_palette_four_steps(
            &mut palette,
            PaletteFadeTarget::Sea,
            PALETTE_FADE_COUNTER_START,
            PALETTE_FADE_COUNTER_START - 2,
        ));
        assert_eq!(palette[15], crate::shapes::SEA_PALETTE[15]);
        assert_eq!(palette[14], initial[14]);

        assert!(apply_palette_four_steps(
            &mut palette,
            PaletteFadeTarget::Sea,
            PALETTE_FADE_COUNTER_START - 2,
            0,
        ));
        assert_eq!(palette[0], initial[0], "color zero is never replaced");
        assert_eq!(&palette[1..], &crate::shapes::SEA_PALETTE[1..]);

        assert!(apply_palette_four_steps(
            &mut palette,
            PaletteFadeTarget::Ground,
            PALETTE_FADE_COUNTER_START,
            PALETTE_FADE_COUNTER_START - 2,
        ));
        assert_eq!(palette[15], crate::shapes::GROUND_PALETTE[15]);
        assert_eq!(
            palette[14],
            crate::shapes::SEA_PALETTE[14],
            "retargeting preserves entries the new walk has not reached"
        );
    }

    #[test]
    fn fortuna_composite_retains_palette_four_pixel_ownership() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |relative: &str| {
            std::fs::read(root.join(relative))
                .unwrap_or_else(|error| panic!("read {relative}: {error}"))
        };
        let composed = compose_bg_with_palette_trace(
            &read("data/bg/3-3.CGX"),
            &read("data/bg/3-3.SCR"),
            &read("data/bg/BG2-C.COL"),
            None,
            None,
            232,
            0,
            true,
        )
        .expect("compose Fortuna background");

        let dynamic_pixels = composed
            .palette_four_pixels
            .iter()
            .filter(|&&index| index != STATIC_PALETTE_PIXEL)
            .count();
        assert!(
            dynamic_pixels > 1_000,
            "Fortuna must visibly use the scripted palette row"
        );

        let mut sea = composed.palette_four;
        assert!(apply_palette_four_steps(
            &mut sea,
            PaletteFadeTarget::Sea,
            PALETTE_FADE_COUNTER_START,
            0,
        ));
        let recolored = recolor_palette_four(&composed.rgba, &composed.palette_four_pixels, &sea);
        for (pixel, &palette_index) in composed.palette_four_pixels.iter().enumerate() {
            let offset = pixel * 4;
            if palette_index == STATIC_PALETTE_PIXEL {
                assert_eq!(
                    &recolored[offset..offset + 4],
                    &composed.rgba[offset..offset + 4]
                );
            } else {
                let expected = bgr555_color(crate::shapes::SEA_PALETTE[usize::from(palette_index)]);
                assert_eq!(&recolored[offset..offset + 3], &expected);
            }
        }
    }

    #[test]
    fn phase_matches_mhofs_flip_order_and_period() {
        assert_eq!(bhole_phase(0), 0);
        assert_eq!(bhole_phase(1), 1);
        assert_eq!(bhole_phase(159), 159);
        assert_eq!(bhole_phase(160), 158); // flip occurs before the add
        assert_eq!(bhole_phase(479), -161);
        assert_eq!(bhole_phase(640), 0);
    }

    #[test]
    fn scanline_offsets_are_center_symmetric() {
        let hofs = bhole_line_offsets(80);
        for y in 0..112 {
            assert_eq!(hofs[111 - y], hofs[112 + y]);
        }
        assert_ne!(hofs[0], hofs[111], "phase creates radial shear");
    }
}
