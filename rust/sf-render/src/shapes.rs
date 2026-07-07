//! Shape color/material tables and pure color-resolution functions.
//!
//! Port (C oracle): the hand-maintained tables and resolution helpers in
//! `src/renderer/shapes.c`. All tables carry the same ASM citations as the
//! C source:
//! - [`NIGHT_PALETTE`]      -- NIGHT.COL, first 16 BGR555 entries
//! - [`SEA_PALETTE`]/[`GROUND_PALETTE`] -- SEA.COL/GROUND.COL row 0, the
//!   map-VM FADETOSEA/FADETOGROUND fade targets (MAIN.ASM:2924-2925)
//! - [`NIGHT1_DEPTH_PAIRS`].. [`NIGHT4_DEPTH_PAIRS`] -- ASM/COLTAB.ASM
//! - [`DEPTHZ_TABLES`]      -- ASM/COLTABS.ASM:1488-1491 `depthtables`
//! - [`COLTAB_ID0`]..[`COLTAB_ID5`] -- COLTABS.ASM ID_0_C..ID_5_C
//! - [`ANIM_CA_0`]..[`ANIM_CA_5`]   -- COLTABS.ASM CA_0..CA_5
//! - shade tables -- LIGHT.ASM via [`crate::light_data`]
//!
//! The resolution functions mirror `Shapes_ResolveMaterialColor`,
//! `Shapes_ComputeShadeIndex` and `Shapes_SelectDepthBank` exactly, but are
//! pure: the C globals (`s_current_depth_bank`, `s_depthz_table`) become
//! explicit parameters.

use crate::light_data::{SHADE_SUBTABLES, SHADE_TABLES, SHADE_TABLE_LEN};

// ---------------------------------------------------------------------------
// Well-known shape ids (src/renderer/shapes.h)
// ---------------------------------------------------------------------------

/// def_shape myship_4 (the Arwing).
pub const SHAPE_MYSHIP_4: u16 = 2;
pub const SHAPE_ARWING: u16 = SHAPE_MYSHIP_4;
/// def_shape boss_7_1 closed body.
pub const SHAPE_BOSS7_1: u16 = 56;
pub const SHAPE_BOSS7_0: u16 = 240;
pub const SHAPE_BOSS7_1O: u16 = 242;
pub const SHAPE_BOSS7_2: u16 = 243;
pub const SHAPE_BOSS7_3: u16 = 244;
pub const SHAPE_BOSS7_4: u16 = 245;
pub const SHAPE_ALIAS_MOTHER1: u16 = 278;
pub const SHAPE_ALIAS_OP_0: u16 = 508;
pub const SHAPE_ALIAS_OP_1: u16 = 509;
pub const SHAPE_ALIAS_OP_2: u16 = 510;
/// Player laser bolt (elaser2). Free runtime slot (< MAX_SHAPES=512); the
/// ROM's elaser2 has no `def_shape` id so we assign one for the builtin.
pub const SHAPE_ELASER2: u16 = 511;

const RAW_SHAPE_BOSS_7_1: u16 = 241;
const RAW_SHAPE_IMYSHIP_4: u16 = 554;
const SHAPE_INTERNAL_BOSS7_0_FRAME1: u16 = 480;
const SHAPE_INTERNAL_BOSS7_3_FRAME1: u16 = 488;
const SHAPE_INTERNAL_BOSS7_4_FRAME1: u16 = 497;
const SHAPE_INTERNAL_BOSS7_0_FRAME_COUNT: u16 = 8;
const SHAPE_INTERNAL_BOSS7_3_FRAME_COUNT: u16 = 9;
const SHAPE_INTERNAL_BOSS7_4_FRAME_COUNT: u16 = 9;

// ---------------------------------------------------------------------------
// Material word encoding (COLTABS.ASM material classes)
//
// Bit layout of the 16-bit material word, matching the MATERIAL_* macros in
// src/renderer/shapes.c:
//
//   COLANIM   1AAA AAAA AAAA AAAA  bit15 set, bits0-13 = anim table id
//                                  (bit14 set too => COLSMOOTH, unported)
//   COLTEXT   01SS SSSS SSSS SSSS  bit14 set, low bits = sprite id (unported)
//   COLNORM   0011 1111 HHHH LLLL  source 63: two palette nibbles hi/lo
//   COLDEPTH  0011 1110 IIII IIII  source 62: index into night depth bank
//   COLLITE   00SS SSSS CCCC CCCC  source < 12: light source row + normal
//                                  color byte (unused by the resolver)
// ---------------------------------------------------------------------------

pub const MATERIAL_SOURCE_COLNORM: u16 = 63;
pub const MATERIAL_SOURCE_COLDEPTH: u16 = 62;
/// COLLITE light sources are `source < 12` (rows 10/11 alias onto row 9).
pub const MATERIAL_COLLITE_SOURCE_LIMIT: u16 = 12;
pub const MATERIAL_COLANIM_FLAG: u16 = 0x8000;
pub const MATERIAL_COLTEXT_FLAG: u16 = 0x4000;

pub const fn material_colanim(anim_id: u16) -> u16 {
    0x8000 | anim_id
}

pub const fn material_collite(light_source: u16, normal_color: u16) -> u16 {
    ((light_source & 0x3F) << 8) | (normal_color & 0xFF)
}

pub const fn material_coldepth(index: u16) -> u16 {
    (MATERIAL_SOURCE_COLDEPTH << 8) | (index & 0xFF)
}

pub const fn material_colnorm(color_lo: u16, color_hi: u16) -> u16 {
    (MATERIAL_SOURCE_COLNORM << 8) | ((color_hi & 0x0F) << 4) | (color_lo & 0x0F)
}

pub const fn material_colnorm1(color: u16) -> u16 {
    material_colnorm(color, color)
}

/// COLTEXT sprite-texture material (unported; resolves via the
/// unsupported-material path). Low byte disambiguates the source sprite.
pub const fn material_coltext(spr: u16) -> u16 {
    0x4000 | spr
}

// ---------------------------------------------------------------------------
// Animated material tables CA_0..CA_5 (COLTABS.ASM)
// ---------------------------------------------------------------------------

pub const SHAPE_ANIM_CA_0: u16 = 0;
pub const SHAPE_ANIM_CA_1: u16 = 1;
pub const SHAPE_ANIM_CA_2: u16 = 2;
pub const SHAPE_ANIM_CA_3: u16 = 3;
pub const SHAPE_ANIM_CA_4: u16 = 4;
pub const SHAPE_ANIM_CA_5: u16 = 5;
/// `bullet_a1` (COLTABS.ASM) — the player-laser color flash (white/cyan/blue).
pub const SHAPE_ANIM_BULLET: u16 = 6;
pub const SHAPE_ANIM_COUNT: usize = 7;

pub static ANIM_CA_0: [u16; 4] = [
    material_colnorm1(0xE),
    material_colnorm1(0x7),
    material_colnorm1(0x2),
    material_colnorm1(0xF),
];

pub static ANIM_CA_1: [u16; 4] = [
    material_colnorm1(0x4),
    material_colnorm1(0x3),
    material_colnorm1(0x2),
    material_colnorm1(0x1),
];

pub static ANIM_CA_2: [u16; 4] = [
    material_colnorm1(0x8),
    material_colnorm1(0x7),
    material_colnorm1(0x6),
    material_colnorm1(0x5),
];

pub static ANIM_CA_3: [u16; 4] = [
    material_colnorm1(0xE),
    material_colnorm1(0xD),
    material_colnorm1(0xC),
    material_colnorm1(0xB),
];

pub static ANIM_CA_4: [u16; 4] = [
    material_colnorm(0x4, 0xE),
    material_colnorm(0x6, 0x8),
    material_colnorm(0x2, 0x8),
    material_colnorm(0x1, 0x3),
];

pub static ANIM_CA_5: [u16; 4] = [
    material_colnorm(0x0, 0xE),
    material_colnorm(0x0, 0x7),
    material_colnorm(0x0, 0x2),
    material_colnorm(0x0, 0xF),
];
/// `bullet_a1` (COLTABS.ASM): COLNORM 14,14 / 8,8 / 14,14 / 7,7 — the player
/// laser bolt flashing NIGHT palette 14(white)/8(cyan)/7(blue).
pub static ANIM_BULLET: [u16; 4] = [
    material_colnorm(0xE, 0xE),
    material_colnorm(0x8, 0x8),
    material_colnorm(0xE, 0xE),
    material_colnorm(0x7, 0x7),
];

pub struct ShapeAnimTable {
    pub frames: &'static [u16],
    pub name: &'static str,
}

pub static ANIM_TABLES: [ShapeAnimTable; SHAPE_ANIM_COUNT] = [
    ShapeAnimTable { frames: &ANIM_CA_0, name: "CA_0" },
    ShapeAnimTable { frames: &ANIM_CA_1, name: "CA_1" },
    ShapeAnimTable { frames: &ANIM_CA_2, name: "CA_2" },
    ShapeAnimTable { frames: &ANIM_CA_3, name: "CA_3" },
    ShapeAnimTable { frames: &ANIM_CA_4, name: "CA_4" },
    ShapeAnimTable { frames: &ANIM_CA_5, name: "CA_5" },
    ShapeAnimTable { frames: &ANIM_BULLET, name: "bullet" },
];

// ---------------------------------------------------------------------------
// Light direction (MOBJ.MC `initlight`)
// ---------------------------------------------------------------------------

/// World-space unit light direction. MOBJ.MC `initlight` (:810-817) compiles
/// the constant 18917/32767 = 0.5773 into all three components; per object
/// the GSU rotates it into object space (m_rotlightx/y/z, MOBJ.MC:905-922)
/// before the per-face normal dot product.
pub const LIGHT_DIR: [f32; 3] = [0.577_350_26, 0.577_350_26, 0.577_350_26];

// ---------------------------------------------------------------------------
// COLDEPTH banks night1..night4 (ASM/COLTAB.ASM)
// ---------------------------------------------------------------------------

/// `night1` from ASM/COLTAB.ASM, kept to the 32 live COLDEPTH entries used by
/// the active built-in shapes.
#[rustfmt::skip]
pub static NIGHT1_DEPTH_PAIRS: [u8; 32] = [
    0x99, 0x9A, 0xAA, 0xAB, 0xBB, 0xBC, 0xCC, 0xCD,
    0xDD, 0xDE, 0xEE, 0x11, 0x12, 0x22, 0x23, 0x33,
    0x34, 0x44, 0x55, 0x56, 0x66, 0x67, 0x77, 0x78,
    0x88, 0x16, 0x26, 0x36, 0x46, 0xAF, 0xFF, 0xDF,
];

/// `night2` from ASM/COLTAB.ASM.
#[rustfmt::skip]
pub static NIGHT2_DEPTH_PAIRS: [u8; 32] = [
    0x99, 0x99, 0x9A, 0x9A, 0xAA, 0xAB, 0xBB, 0xBC,
    0xCC, 0xCD, 0xDD, 0x99, 0x19, 0x11, 0x12, 0x22,
    0x23, 0x33, 0x99, 0x59, 0x55, 0x56, 0x66, 0x67,
    0x77, 0x99, 0x15, 0x16, 0x26, 0x9F, 0xAF, 0xFF,
];

/// `night3` from ASM/COLTAB.ASM.
#[rustfmt::skip]
pub static NIGHT3_DEPTH_PAIRS: [u8; 32] = [
    0x99, 0x99, 0x99, 0x99, 0x9A, 0x9A, 0xAA, 0xAB,
    0xBB, 0xBC, 0xCC, 0x99, 0x99, 0x19, 0x19, 0x11,
    0x12, 0x22, 0x99, 0x99, 0x59, 0x59, 0x55, 0x56,
    0x66, 0x99, 0x99, 0x15, 0x16, 0x99, 0x15, 0x9F,
];

/// `night4` from ASM/COLTAB.ASM (darkest / most distant bank).
#[rustfmt::skip]
pub static NIGHT4_DEPTH_PAIRS: [u8; 32] = [
    0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9A, 0x9A,
    0xAA, 0xAB, 0xBB, 0x99, 0x99, 0x99, 0x19, 0x19,
    0x19, 0x11, 0x99, 0x99, 0x99, 0x59, 0x59, 0x59,
    0x55, 0x99, 0x99, 0x15, 0x15, 0x99, 0x15, 0x15,
];

/// COLDEPTH banks brightest -> darkest, selected per object by view depth.
pub static NIGHT_DEPTH_BANKS: [&[u8; 32]; 4] = [
    &NIGHT1_DEPTH_PAIRS,
    &NIGHT2_DEPTH_PAIRS,
    &NIGHT3_DEPTH_PAIRS,
    &NIGHT4_DEPTH_PAIRS,
];

// ---------------------------------------------------------------------------
// Depth-band thresholds (ASM/COLTABS.ASM `depthtables`)
// ---------------------------------------------------------------------------

pub const DEPTHZ_NORMAL: usize = 0;
pub const DEPTHZ_TUNNEL: usize = 1;
pub const DEPTHZ_MIST: usize = 2;
pub const DEPTHZ_STAGE1: usize = 3;
pub const DEPTHZ_COUNT: usize = 4;

/// Per-level depth-band threshold records, transcribed from the `def_depthz`
/// entries in ASM/COLTABS.ASM:1488-1491 (`depthtables`). Compared against the
/// object's view-space Z (`bigz`); the hardware keeps only the high byte, so
/// values are multiples of 256 by construction where the ASM used hex.
pub static DEPTHZ_TABLES: [[f32; 3]; DEPTHZ_COUNT] = [
    [2560.0, 3328.0, 3840.0],  // NORMAL: $a00/$d00/$f00
    [500.0, 750.0, 1000.0],    // TUNNEL
    [500.0, 750.0, 1000.0],    // MIST
    [2560.0, 3328.0, 16128.0], // STAGE1: $a00/$d00/$3f00
];

// ---------------------------------------------------------------------------
// NIGHT.COL palette
// ---------------------------------------------------------------------------

/// First 16 palette entries from NIGHT.COL, stored as SNES BGR555 values.
#[rustfmt::skip]
pub static NIGHT_PALETTE: [u16; 16] = [
    0x0000, 0x0451, 0x153A, 0x22BD,
    0x377F, 0x5445, 0x6D2B, 0x7E8F,
    0x7FB6, 0x0CA3, 0x2588, 0x424E,
    0x56F5, 0x6B9A, 0x7FFE, 0x0300,
];

// ---------------------------------------------------------------------------
// SEA.COL / GROUND.COL fade palettes (map-VM FADETOSEA / FADETOGROUND)
//
// ROM: WORLD.ASM:371-394 fadetoseado/fadetogrounddo arm palfade/palnum;
// MAIN.ASM:2762 fadepalto_l then copies one word per frame from
// seapal/groundpal (MAIN.ASM:2924-2925, DATA/COL/SEA.COL and GROUND.COL
// row 0) into shape-palette CGRAM row 4, colors 15 down to 1, over 15
// frames. The HD port renders this as a smooth crossfade of the whole
// shape palette by the bridged 0..1 fraction instead of the per-color
// stepped copy.
// ---------------------------------------------------------------------------

/// Shape-palette ids for the FADETOSEA/FADETOGROUND bridge
/// (`FrameInputs::pal_from` / `pal_target`). Values are the numeric
/// contract with sf-game's `PALFADE_*` consts.
pub const PAL_TARGET_NIGHT: u8 = 0;
pub const PAL_TARGET_SEA: u8 = 1;
pub const PAL_TARGET_GROUND: u8 = 2;

/// First 16 palette entries from SEA.COL (ROM `seapal`, MAIN.ASM:2924) —
/// the underwater blue ramp FADETOSEA walks the shape palette toward.
#[rustfmt::skip]
pub static SEA_PALETTE: [u16; 16] = [
    0x0000, 0x34C0, 0x38E0, 0x3D00,
    0x4120, 0x4540, 0x4960, 0x4D80,
    0x51A0, 0x55C0, 0x59E0, 0x5E00,
    0x6220, 0x6640, 0x6A60, 0x6E80,
];

/// First 16 palette entries from GROUND.COL (ROM `groundpal`,
/// MAIN.ASM:2925) — the surface ramp FADETOGROUND restores.
#[rustfmt::skip]
pub static GROUND_PALETTE: [u16; 16] = [
    0x0000, 0x0088, 0x00C9, 0x04EA,
    0x090B, 0x0D2C, 0x114D, 0x156E,
    0x198F, 0x1DB0, 0x21D1, 0x25F2,
    0x2A13, 0x2E34, 0x3255, 0x3676,
];

/// Look up the BGR555 shape palette for a PAL_TARGET_* id (unknown ids
/// fall back to night, the default shape palette).
pub fn shape_palette_bgr(id: u8) -> &'static [u16; 16] {
    match id {
        PAL_TARGET_SEA => &SEA_PALETTE,
        PAL_TARGET_GROUND => &GROUND_PALETTE,
        _ => &NIGHT_PALETTE,
    }
}

/// A decoded (linear RGB) 16-entry shape palette, the per-frame input of
/// the `*_in` color resolvers.
pub type ShapePaletteRgb = [[f32; 3]; 16];

/// Decode a BGR555 shape palette to linear RGB.
pub fn decode_shape_palette(palette: &[u16; 16]) -> ShapePaletteRgb {
    let mut out = [[0.0f32; 3]; 16];
    for (dst, &word) in out.iter_mut().zip(palette.iter()) {
        *dst = decode_bgr555(word);
    }
    out
}

/// Build the effective shape palette for the current FADETOSEA/GROUND
/// state: decode `from` and `target` (PAL_TARGET_*) and mix by `fade`
/// (0..1, clamped). fade=0 is the `from` palette, fade=1 the `target`.
/// HD rendition of the ROM's 15-frame fadepalto_l walk (MAIN.ASM:2762).
pub fn mixed_shape_palette(from: u8, target: u8, fade: f32) -> ShapePaletteRgb {
    let a = decode_shape_palette(shape_palette_bgr(from));
    if from == target {
        return a;
    }
    let b = decode_shape_palette(shape_palette_bgr(target));
    let t = fade.clamp(0.0, 1.0);
    let mut out = [[0.0f32; 3]; 16];
    for i in 0..16 {
        for c in 0..3 {
            // Two-sided lerp so fade 0/1 reproduce the endpoints exactly.
            out[i][c] = a[i][c] * (1.0 - t) + b[i][c] * t;
        }
    }
    out
}

/// The default (no fade) decoded shape palette: NIGHT.COL.
pub fn night_shape_palette() -> ShapePaletteRgb {
    decode_shape_palette(&NIGHT_PALETTE)
}

/// Magenta placeholder emitted for unported/unknown materials.
pub const DEBUG_MATERIAL_COLOR: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

// ---------------------------------------------------------------------------
// Shared polygon color tables ID_0_C..ID_5_C (COLTABS.ASM)
// ---------------------------------------------------------------------------

pub const SHAPE_COLTAB_ID_0: u8 = 0;
pub const SHAPE_COLTAB_ID_1: u8 = 1;
pub const SHAPE_COLTAB_ID_2: u8 = 2;
pub const SHAPE_COLTAB_ID_3: u8 = 3;
pub const SHAPE_COLTAB_ID_4: u8 = 4;
pub const SHAPE_COLTAB_ID_5: u8 = 5;
pub const SHAPE_COLTAB_ID_INVALID: u8 = 0xFF;

/// COLTABS.ASM `ID_0_C`, full 109 entries FX0-FX108.
pub static COLTAB_ID0: [u16; 110] = [
    // FX0-FX9: COLLITE light-source rows.
    material_collite(0, 0), material_collite(1, 1),
    material_collite(2, 2), material_collite(3, 3),
    material_collite(4, 4), material_collite(5, 5),
    material_collite(6, 6), material_collite(7, 7),
    material_collite(8, 8), material_collite(9, 9),
    // FX10-FX41: COLDEPTH bank indices.
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(7),
    material_coldepth(8),  material_coldepth(9),
    material_coldepth(10), material_coldepth(11),
    material_coldepth(12), material_coldepth(13),
    material_coldepth(14), material_coldepth(15),
    material_coldepth(16), material_coldepth(17),
    material_coldepth(18), material_coldepth(19),
    material_coldepth(20), material_coldepth(21),
    material_coldepth(22), material_coldepth(23),
    material_coldepth(24), material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    // FX42-FX47: animated materials CA_0..CA_5.
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
    // FX48-FX70 (COLTABS.ASM ID_0_C): sprite textures and animation tables
    // that are not ported yet. Keep the rows so higher FX indices stay
    // aligned; material_coltext flows through the unsupported-material path.
    material_coltext(0),  // FX48 asteroid1_spr
    material_coltext(1),  // FX49 panel3_spr
    material_coltext(1),  // FX50 panel3_spr
    material_coltext(1),  // FX51 panel3_spr
    material_coltext(2),  // FX52 gateflash_a1 (anim, unported)
    material_coltext(3),  // FX53 bosshitcol1_a1
    material_coltext(4),  // FX54 bosshitcol2_a1
    material_coltext(5),  // FX55 pwireframe_a1
    material_coltext(6),  // FX56 bosshitcol3_a1
    material_coltext(7),  // FX57 face1_spr
    material_coltext(8),  // FX58 roadflash_a1
    material_coltext(9),  // FX59 biker1_spr
    material_coltext(10), // FX60 biker1_spr,8
    material_coltext(11), // FX61 ring_a1
    material_coltext(12), // FX62 ring_a2
    material_coltext(13), // FX63 ring_a3
    material_coltext(14), // FX64 ring_a4
    material_coltext(15), // FX65 ring_a5
    material_coltext(16), // FX66 ca_1d
    material_coltext(17), // FX67 scrapmetal_spr
    material_coltext(18), // FX68 cockpit1_spr
    material_coltext(19), // FX69 cockpit2_spr
    material_coltext(20), // FX70 monkey1_spr
    // FX71-FX95: solid/dithered COLNORM ramps (COLTABS.ASM ID_0_C).
    material_colnorm(0x9, 0x9), material_colnorm(0x9, 0xA), // FX71-72
    material_colnorm(0xA, 0xA), material_colnorm(0xA, 0xB), // FX73-74
    material_colnorm(0xB, 0xB), material_colnorm(0xB, 0xC), // FX75-76
    material_colnorm(0xC, 0xC), material_colnorm(0xC, 0xD), // FX77-78
    material_colnorm(0xD, 0xD), material_colnorm(0xD, 0xE), // FX79-80
    material_colnorm(0xE, 0xE), material_colnorm(0x1, 0x1), // FX81-82
    material_colnorm(0x1, 0x2), material_colnorm(0x2, 0x2), // FX83-84
    material_colnorm(0x2, 0x3), material_colnorm(0x3, 0x3), // FX85-86
    material_colnorm(0x3, 0x4), material_colnorm(0x4, 0x4), // FX87-88
    material_colnorm(0x5, 0x5), material_colnorm(0x5, 0x6), // FX89-90
    material_colnorm(0x6, 0x6), material_colnorm(0x6, 0x7), // FX91-92
    material_colnorm(0x7, 0x7), material_colnorm(0x7, 0x8), // FX93-94
    material_colnorm(0x8, 0x8),                             // FX95
    // FX96-FX108: more unported sprite/anim entries.
    material_coltext(21), // FX96 byebye_spr,7
    material_coltext(22), // FX97 spacemistWhite_a1
    material_coltext(23), // FX98 spacemistRed_a1
    material_coltext(24), // FX99 spacemistLgrey_a1
    material_coltext(25), // FX100 chicken_spr
    material_coltext(26), // FX101 brakelight_a1
    material_coltext(25), // FX102 chicken_spr
    material_coltext(7),  // FX103 face1_spr
    material_coltext(27), // FX104 playerbeam_a1
    material_coltext(28), // FX105 largeexpl4_spr,6
    material_coltext(29), // FX106 starwars1_spr
    material_coltext(30), // FX107 starwars2_spr
    material_coltext(31), // FX108 tunnelwall_spr
    material_colanim(SHAPE_ANIM_BULLET), // FX109 bullet_c (player laser flash)
];

/// COLTABS.ASM `ID_1_C`.
pub static COLTAB_ID1: [u16; 48] = [
    material_collite(0, 0), material_collite(1, 1),
    material_collite(3, 3), material_collite(2, 2),
    material_collite(5, 5), material_collite(4, 4),
    material_collite(7, 7), material_collite(6, 6),
    material_collite(9, 9), material_collite(8, 8),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(7),
    material_coldepth(8),  material_coldepth(9),
    material_coldepth(10), material_coldepth(18),
    material_coldepth(19), material_coldepth(20),
    material_coldepth(21), material_coldepth(22),
    material_coldepth(23), material_coldepth(24),
    material_coldepth(11), material_coldepth(12),
    material_coldepth(13), material_coldepth(14),
    material_coldepth(15), material_coldepth(16),
    material_coldepth(17), material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
];

/// COLTABS.ASM `ID_2_C`.
pub static COLTAB_ID2: [u16; 48] = [
    material_collite(2, 2), material_collite(6, 6),
    material_collite(0, 0), material_collite(3, 3),
    material_collite(4, 4), material_collite(5, 5),
    material_collite(1, 1), material_collite(7, 7),
    material_collite(8, 8), material_collite(9, 9),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(11), material_coldepth(12),
    material_coldepth(13), material_coldepth(14),
    material_coldepth(15), material_coldepth(16),
    material_coldepth(17), material_coldepth(9),
    material_coldepth(10), material_coldepth(2),
    material_coldepth(3),  material_coldepth(4),
    material_coldepth(5),  material_coldepth(6),
    material_coldepth(7),  material_coldepth(8),
    material_coldepth(18), material_coldepth(19),
    material_coldepth(20), material_coldepth(21),
    material_coldepth(22), material_coldepth(23),
    material_coldepth(24), material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
];

/// COLTABS.ASM `ID_3_C`.
pub static COLTAB_ID3: [u16; 48] = [
    material_collite(3, 3), material_collite(7, 7),
    material_collite(2, 2), material_collite(0, 0),
    material_collite(4, 4), material_collite(5, 5),
    material_collite(6, 6), material_collite(1, 1),
    material_collite(8, 8), material_collite(9, 9),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(18), material_coldepth(19),
    material_coldepth(20), material_coldepth(21),
    material_coldepth(22), material_coldepth(23),
    material_coldepth(24), material_coldepth(9),
    material_coldepth(10), material_coldepth(11),
    material_coldepth(12), material_coldepth(13),
    material_coldepth(14), material_coldepth(15),
    material_coldepth(16), material_coldepth(17),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(7),
    material_coldepth(8),  material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
];

/// COLTABS.ASM `ID_4_C`.
pub static COLTAB_ID4: [u16; 48] = [
    material_collite(0, 0), material_collite(1, 1),
    material_collite(0, 0), material_collite(3, 3),
    material_collite(4, 4), material_collite(5, 5),
    material_collite(1, 1), material_collite(7, 7),
    material_collite(8, 8), material_collite(9, 9),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(7),
    material_coldepth(8),  material_coldepth(9),
    material_coldepth(10), material_coldepth(0),
    material_coldepth(1),  material_coldepth(2),
    material_coldepth(3),  material_coldepth(4),
    material_coldepth(5),  material_coldepth(6),
    material_coldepth(18), material_coldepth(19),
    material_coldepth(20), material_coldepth(21),
    material_coldepth(22), material_coldepth(23),
    material_coldepth(24), material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
];

/// COLTABS.ASM `ID_5_C`.
pub static COLTAB_ID5: [u16; 48] = [
    material_collite(0, 0), material_collite(1, 1),
    material_collite(2, 2), material_collite(0, 0),
    material_collite(4, 4), material_collite(5, 5),
    material_collite(6, 6), material_collite(1, 1),
    material_collite(8, 8), material_collite(9, 9),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(7),
    material_coldepth(8),  material_coldepth(9),
    material_coldepth(10), material_coldepth(11),
    material_coldepth(12), material_coldepth(13),
    material_coldepth(14), material_coldepth(15),
    material_coldepth(16), material_coldepth(17),
    material_coldepth(0),  material_coldepth(1),
    material_coldepth(2),  material_coldepth(3),
    material_coldepth(4),  material_coldepth(5),
    material_coldepth(6),  material_coldepth(25),
    material_coldepth(26), material_coldepth(27),
    material_coldepth(28), material_coldepth(29),
    material_coldepth(30), material_coldepth(31),
    material_colanim(SHAPE_ANIM_CA_0),
    material_colanim(SHAPE_ANIM_CA_2),
    material_colanim(SHAPE_ANIM_CA_1),
    material_colanim(SHAPE_ANIM_CA_3),
    material_colanim(SHAPE_ANIM_CA_4),
    material_colanim(SHAPE_ANIM_CA_5),
];

pub struct ShapeColorTable {
    pub entries: &'static [u16],
    pub name: &'static str,
}

/// COLTABS.ASM `BLACK_C`: 64x COLNORM(9,9) — the whole shape drawn in the
/// darkest night palette pair. Used by the bossg shadow-clone flicker
/// (D2STRATS.ASM:481-486: coltab BLACK_C on odd gameframes). Registered at
/// table id 6 (`COLTAB_BLACK_C` in sf-strat bosses.rs).
pub static COLTAB_ID6_BLACK: [u16; 64] = [material_colnorm(9, 9); 64];

pub static COLOR_TABLES: [ShapeColorTable; 7] = [
    ShapeColorTable { entries: &COLTAB_ID0, name: "id_0_c" },
    ShapeColorTable { entries: &COLTAB_ID1, name: "id_1_c" },
    ShapeColorTable { entries: &COLTAB_ID2, name: "id_2_c" },
    ShapeColorTable { entries: &COLTAB_ID3, name: "id_3_c" },
    ShapeColorTable { entries: &COLTAB_ID4, name: "id_4_c" },
    ShapeColorTable { entries: &COLTAB_ID5, name: "id_5_c" },
    ShapeColorTable { entries: &COLTAB_ID6_BLACK, name: "black_c" },
];

// ---------------------------------------------------------------------------
// Shape word / color table id resolution (Shapes_ResolveShapeWord & friends)
// ---------------------------------------------------------------------------

/// Canonicalize the live raw/16-bit shape words used by current literal
/// slices into bounded flat runtime ids. Unknown words are returned
/// unchanged. Mirrors `Shapes_ResolveShapeWord`.
pub fn resolve_shape_word(shape_id: u16) -> u16 {
    match shape_id {
        RAW_SHAPE_BOSS_7_1 => SHAPE_BOSS7_1,
        278 => SHAPE_ALIAS_MOTHER1,
        551 => SHAPE_ALIAS_OP_0,
        552 => SHAPE_ALIAS_OP_1,
        553 => SHAPE_ALIAS_OP_2,
        // levels.c SH_MY_BIRD raw word.
        557 => crate::shape_data::SHAPE_EXT_MY_BIRD,
        RAW_SHAPE_IMYSHIP_4 => SHAPE_MYSHIP_4,
        _ => shape_id,
    }
}

/// Map boss7 internal animation-frame slots back onto the base shape id used
/// for color lookups. Mirrors `Shapes_ResolveBaseColorShape`.
pub fn resolve_base_color_shape(shape_id: u16) -> u16 {
    if (SHAPE_INTERNAL_BOSS7_0_FRAME1
        ..SHAPE_INTERNAL_BOSS7_0_FRAME1 + SHAPE_INTERNAL_BOSS7_0_FRAME_COUNT)
        .contains(&shape_id)
    {
        return SHAPE_BOSS7_0;
    }
    if (SHAPE_INTERNAL_BOSS7_3_FRAME1
        ..SHAPE_INTERNAL_BOSS7_3_FRAME1 + SHAPE_INTERNAL_BOSS7_3_FRAME_COUNT)
        .contains(&shape_id)
    {
        return SHAPE_BOSS7_3;
    }
    if (SHAPE_INTERNAL_BOSS7_4_FRAME1
        ..SHAPE_INTERNAL_BOSS7_4_FRAME1 + SHAPE_INTERNAL_BOSS7_4_FRAME_COUNT)
        .contains(&shape_id)
    {
        return SHAPE_BOSS7_4;
    }
    shape_id
}

/// Mirrors `Shapes_IsBoss7Shape`.
pub fn is_boss7_shape(shape_id: u16) -> bool {
    matches!(
        resolve_base_color_shape(shape_id),
        SHAPE_BOSS7_1
            | SHAPE_BOSS7_0
            | SHAPE_BOSS7_1O
            | SHAPE_BOSS7_2
            | SHAPE_BOSS7_3
            | SHAPE_BOSS7_4
    )
}

/// Mirrors `Shapes_ResolveColorTableId`: color_table 0 defaults to ID_0_C
/// (ID_1_C for boss7 shapes); out-of-range tables are invalid.
pub fn resolve_color_table_id(shape_id: u16, color_table: u16) -> u8 {
    if color_table == 0 {
        return if is_boss7_shape(shape_id) {
            SHAPE_COLTAB_ID_1
        } else {
            SHAPE_COLTAB_ID_0
        };
    }
    if color_table <= SHAPE_COLTAB_ID_5 as u16 {
        return color_table as u8;
    }
    SHAPE_COLTAB_ID_INVALID
}

// ---------------------------------------------------------------------------
// Color resolution (pure mirrors of the shapes.c statics)
// ---------------------------------------------------------------------------

/// Decode a SNES BGR555 word to linear RGB in [0,1].
pub fn decode_bgr555(color: u16) -> [f32; 3] {
    [
        (color & 0x1F) as f32 / 31.0,
        ((color >> 5) & 0x1F) as f32 / 31.0,
        ((color >> 10) & 0x1F) as f32 / 31.0,
    ]
}

/// Hardware checkerboard-dithers the two nibble colors of a pair; this port
/// averages them instead. All nibble-pair consumers (COLNORM, COLDEPTH,
/// COLLITE shades) funnel through here so a dithered decode can be swapped
/// in later without touching the material paths. `palette` is the decoded
/// (and possibly FADETOSEA/GROUND-mixed) shape palette for this frame.
pub fn decode_palette_pair_in(pair: u8, palette: &ShapePaletteRgb) -> [f32; 4] {
    let lo = palette[(pair & 0x0F) as usize];
    let hi = palette[((pair >> 4) & 0x0F) as usize];
    [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
        1.0,
    ]
}

/// [`decode_palette_pair_in`] against the unfaded night palette.
pub fn decode_palette_pair(pair: u8) -> [f32; 4] {
    decode_palette_pair_in(pair, &night_shape_palette())
}

/// Resolve a material word to RGBA. Pure mirror of
/// `Shapes_ResolveMaterialColor`; `depth_bank` (0..=3) replaces the C
/// `s_current_depth_bank` global and selects both the COLDEPTH night bank
/// and the COLLITE shade group. Unsupported materials (COLTEXT, COLSMOOTH,
/// COLLITE sources 12..62) resolve to [`DEBUG_MATERIAL_COLOR`].
/// `palette` is the decoded (possibly FADETOSEA/GROUND-mixed) shape
/// palette every nibble pair resolves against.
pub fn resolve_material_color_in(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    let source = (material_word >> 8) as u8;
    let bank = (depth_bank as usize).min(3);

    if material_word & MATERIAL_COLANIM_FLAG != 0 {
        if material_word & MATERIAL_COLTEXT_FLAG != 0 {
            // COLSMOOTH: unported.
            return DEBUG_MATERIAL_COLOR;
        }
        let anim_id = (material_word & 0x3FFF) as usize;
        let Some(anim_table) = ANIM_TABLES.get(anim_id) else {
            return DEBUG_MATERIAL_COLOR;
        };
        if anim_table.frames.is_empty() {
            return DEBUG_MATERIAL_COLOR;
        }
        // Super FX masks with `(frame_count - 1)` rather than clamping.
        let frame_index = (col_frame as usize) & (anim_table.frames.len() - 1);
        return resolve_material_color_in(
            anim_table.frames[frame_index],
            col_frame,
            shade_index,
            depth_bank,
            palette,
        );
    }

    if material_word & MATERIAL_COLTEXT_FLAG != 0 {
        // COLTEXT: unported.
        return DEBUG_MATERIAL_COLOR;
    }

    if source as u16 == MATERIAL_SOURCE_COLNORM {
        return decode_palette_pair_in((material_word & 0xFF) as u8, palette);
    }

    if source as u16 == MATERIAL_SOURCE_COLDEPTH {
        let depth_index = (material_word & 0xFF) as usize;
        if depth_index < NIGHT1_DEPTH_PAIRS.len() {
            return decode_palette_pair_in(NIGHT_DEPTH_BANKS[bank][depth_index], palette);
        }
        return DEBUG_MATERIAL_COLOR;
    }

    if (source as u16) < MATERIAL_COLLITE_SOURCE_LIMIT {
        // The viewer aliases light sources 10/11 onto row 9; LIGHT.ASM only
        // defines shadesG_0..shadesG_9.
        let row = (source as usize).min(SHADE_SUBTABLES - 1);
        let shade = shade_index.clamp(0, SHADE_TABLE_LEN as i32 - 1) as usize;
        // The shade group is the object's depth band: MOBJ.MC:472-503 selects
        // m_shadestab (shadestab2_0..3) in the same branch that picks the
        // COLDEPTH bank, so distant objects wash out together.
        return decode_palette_pair_in(SHADE_TABLES[bank][row][shade], palette);
    }

    DEBUG_MATERIAL_COLOR
}

/// [`resolve_material_color_in`] against the unfaded night palette.
pub fn resolve_material_color(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
) -> [f32; 4] {
    resolve_material_color_in(
        material_word,
        col_frame,
        shade_index,
        depth_bank,
        &night_shape_palette(),
    )
}

/// Resolve a face's color-table entry to RGBA. Pure mirror of
/// `Shapes_ResolveFaceColor`; `palette` as in
/// [`resolve_material_color_in`].
pub fn resolve_face_color_in(
    shape_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    let color_table_id = resolve_color_table_id(shape_id, color_table);
    let Some(table) = COLOR_TABLES.get(color_table_id as usize) else {
        return DEBUG_MATERIAL_COLOR;
    };
    let Some(&material_word) = table.entries.get(face_color_index as usize) else {
        return DEBUG_MATERIAL_COLOR;
    };
    resolve_material_color_in(material_word, col_frame, shade_index, depth_bank, palette)
}

/// [`resolve_face_color_in`] against the unfaded night palette.
pub fn resolve_face_color(
    shape_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
) -> [f32; 4] {
    resolve_face_color_in(
        shape_id,
        face_color_index,
        col_frame,
        color_table,
        shade_index,
        depth_bank,
        &night_shape_palette(),
    )
}

/// Face-normal . rotated-light mapped onto the 10 shade levels with the GSU's
/// asymmetric intensity curve (MOBJ.MC:4092-4128): the fixed-point dot works
/// out to ~dot*15.75, clamped to [6,15], minus 6. Faces angled more than
/// ~67 degrees from the light all land on shade 0. Pure mirror of
/// `Shapes_ComputeShadeIndex`.
pub fn compute_shade_index(normal: [f32; 3], light_obj: [f32; 3]) -> i32 {
    let len_sq =
        normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];

    // Degenerate faces carry a zero normal; treat them as fully lit like the
    // viewer does for zero-length normals.
    if len_sq <= 0.0001 {
        return SHADE_TABLE_LEN as i32 - 1;
    }

    let dot = normal[0] * light_obj[0]
        + normal[1] * light_obj[1]
        + normal[2] * light_obj[2];
    let shade = (dot * 15.75).floor() as i32;
    shade.clamp(6, 15) - 6
}

/// Pick the COLDEPTH bank (night1..night4) from the object's view-space
/// depth. Pure core of `Shapes_SelectDepthBank`: the caller computes `depth`
/// as the negated view-space Z (the hardware's per-object `bigz` compare,
/// MOBJ.MC:442-506) and `depthz_table` selects the `def_depthz` record
/// ([`DEPTHZ_NORMAL`]..[`DEPTHZ_STAGE1`]).
pub fn select_depth_bank(depth: f32, depthz_table: usize) -> u8 {
    let table = if depthz_table < DEPTHZ_COUNT {
        depthz_table
    } else {
        DEPTHZ_NORMAL
    };
    let mut bank: u8 = 0;
    while bank < 3 && depth >= DEPTHZ_TABLES[table][bank as usize] {
        bank += 1;
    }
    bank
}
