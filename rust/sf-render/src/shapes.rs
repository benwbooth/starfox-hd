//! Shape color/material tables and pure color-resolution functions.
//!
//! ROM-backed material resolution for the generated shape catalog.  Exact
//! ShapeHdr color tables, COLANIM records, and COLTEXT descriptors are emitted
//! by `tools/color_compiler.py` into [`crate::color_data`].  The remaining
//! shared palette/light tables carry their original ASM citations:
//! - [`NIGHT_PALETTE`]      -- NIGHT.COL, first 16 BGR555 entries
//! - [`SEA_PALETTE`]/[`GROUND_PALETTE`] -- SEA.COL/GROUND.COL row 0, the
//!   map-VM FADETOSEA/FADETOGROUND fade targets (MAIN.ASM:2924-2925)
//! - [`NIGHT1_DEPTH_PAIRS`].. [`NIGHT4_DEPTH_PAIRS`] -- ASM/COLTAB.ASM
//! - [`DEPTHZ_TABLES`]      -- ASM/COLTABS.ASM:1488-1491 `depthtables`
//! - shade tables -- LIGHT.ASM via [`crate::light_data`]
//!
//! The pure resolution functions take the current depth bank and palette as
//! explicit parameters.  COLTEXT faces are returned as materials for the GPU
//! texture path in `shapes_gl`; the flat-color-only helpers deliberately
//! return the debug color for those non-flat materials.

use crate::light_data::{SHADE_SUBTABLES, SHADE_TABLES, SHADE_TABLE_LEN};
use sf_core::scene::{DepthColors, DepthThresholds, GamePalette};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Well-known shape ids (src/renderer/shapes.h)
// ---------------------------------------------------------------------------

/// `nullshape`, used by invisible strategy and camera-controller objects.
pub const SHAPE_NULL: u16 = 0;
/// Authored `ShapeHdr` half-extents for `nullshape`.
///
/// `USHAPES.ASM:162` supplies `(34, 34, 36)` with coordinate shift 2;
/// `SHMACS.INC` stores each bound after applying that shift. The shape has no
/// vertices, so these source bounds cannot be reconstructed from its mesh.
pub const SHAPE_NULL_HALF_EXTENTS: (i16, i16, i16) = (136, 136, 144);
/// def_shape myship_4 (the Arwing).
pub const SHAPE_MYSHIP_4: u16 = 2;
pub const SHAPE_ARWING: u16 = SHAPE_MYSHIP_4;
/// def_shape boss_7_1 closed body.
pub const SHAPE_BOSS7_1: u16 = 55;
/// Direct-only boss components occupy collision-free native slots. The old
/// translation reused 240-245, silently replacing six catalog meshes.
pub const SHAPE_BOSS7_0: u16 = 421;
pub const SHAPE_BOSS7_1O: u16 = 422;
pub const SHAPE_BOSS7_2: u16 = 423;
pub const SHAPE_BOSS7_3: u16 = 424;
pub const SHAPE_BOSS7_4: u16 = 425;
pub const SHAPE_ALIAS_MOTHER1: u16 = 278;
pub const SHAPE_ALIAS_OP_0: u16 = 508;
pub const SHAPE_ALIAS_OP_1: u16 = 509;
pub const SHAPE_ALIAS_OP_2: u16 = 510;
/// Player laser bolt (elaser2). Free runtime slot (< MAX_SHAPES=512); the
/// ROM's elaser2 has no `def_shape` id so we assign one for the builtin.
pub const SHAPE_ELASER2: u16 = 511;

/// Build the SF1 source-authored half-extents table without constructing the
/// GPU shape store. The game shell consumes this typed table for collision and
/// retail view-plane culling; oracle tests use the same generated metadata.
pub fn sf1_shape_half_extents() -> HashMap<u16, (i16, i16, i16)> {
    sf_core::sf1_shape_metrics::SF1_SHAPE_METRICS
        .iter()
        .map(|&(shape_id, metrics)| {
            (
                shape_id,
                (
                    metrics.half_extents[0],
                    metrics.half_extents[1],
                    metrics.half_extents[2],
                ),
            )
        })
        .collect()
}

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
//   COLANIM   1AAA AAAA AAAA AAAA  bit15 set, bits0-13 = bank-local pointer
//                                  (bit14 set too => dormant COLSMOOTH)
//   COLTEXT   01XY XHSS SSSS SSSS  bit14, xy layout, high-nibble, sprite id
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

/// COLTEXT sprite-texture material with the default texture-XY layout.
pub const fn material_coltext(spr: u16) -> u16 {
    0x4000 | spr
}

// ---------------------------------------------------------------------------
// Animated material tables CA_0..CA_5 (COLTABS.ASM)
// ---------------------------------------------------------------------------

pub const SHAPE_ANIM_CA_0: u16 = crate::color_data::ANIM_PTR_CA_0;
pub const SHAPE_ANIM_CA_1: u16 = crate::color_data::ANIM_PTR_CA_1;
pub const SHAPE_ANIM_CA_2: u16 = crate::color_data::ANIM_PTR_CA_2;
pub const SHAPE_ANIM_CA_3: u16 = crate::color_data::ANIM_PTR_CA_3;
pub const SHAPE_ANIM_CA_4: u16 = crate::color_data::ANIM_PTR_CA_4;
pub const SHAPE_ANIM_CA_5: u16 = crate::color_data::ANIM_PTR_CA_5;
/// `bullet_a1` (COLTABS.ASM) — the player-laser color flash (white/cyan/blue).
pub const SHAPE_ANIM_BULLET: u16 = crate::color_data::ANIM_PTR_BULLET_A1;

// ---------------------------------------------------------------------------
// Light direction (MOBJ.MC `initlight`)
// ---------------------------------------------------------------------------

/// World-space unit light direction. MOBJ.MC `initlight` (:810-817) compiles
/// the Q15 constant 18917/32768 into all three source-space components.
/// Generated vertices, normals, and matrices reflect source Y into the
/// renderer's Y-up coordinates, so the light vector must use that same basis.
/// Per object the GSU rotates it into object space (MOBJ.MC:905-922) before
/// the per-face normal dot product.
pub const LIGHT_DIR: [f32; 3] = [
    18_917.0 / 32_768.0,
    -18_917.0 / 32_768.0,
    18_917.0 / 32_768.0,
];

// ---------------------------------------------------------------------------
// COLDEPTH banks night1..night4 (ASM/COLTAB.ASM)
// ---------------------------------------------------------------------------

pub use crate::scene_color_data::{
    NIGHT_1_DEPTH_COLORS as NIGHT1_DEPTH_PAIRS, NIGHT_2_DEPTH_COLORS as NIGHT2_DEPTH_PAIRS,
    NIGHT_3_DEPTH_COLORS as NIGHT3_DEPTH_PAIRS, NIGHT_4_DEPTH_COLORS as NIGHT4_DEPTH_PAIRS,
    NIGHT_DEPTH_COLORS as NIGHT_DEPTH_BANKS,
};

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

pub use crate::scene_color_data::NIGHT_GAME_PALETTE as NIGHT_PALETTE;

/// Semantic SF2 polygon-palette families used by verified live missions.
///
/// This is presentation state for a flat-memory native port. Source catalog
/// positions remain in generated asset data and are not carried at runtime.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sf2PolygonPalette {
    #[default]
    Standard,
    EladardSurface,
    AstropolisExterior,
}

impl Sf2PolygonPalette {
    pub const fn bgr555(self) -> &'static [u16; 16] {
        use sf2_data::palettes::PolygonPaletteId;

        match self {
            Self::Standard => PolygonPaletteId::Standard.colors(),
            Self::EladardSurface => PolygonPaletteId::EladardSurface.colors(),
            Self::AstropolisExterior => PolygonPaletteId::AstropolisExterior.colors(),
        }
    }
}

// ---------------------------------------------------------------------------
// SEA.COL / GROUND.COL background fade palettes
//
// ROM: WORLD.ASM:371-394 fadetoseado/fadetogrounddo arm palfade/palnum;
// MAIN.ASM:2762 fadepalto_l then copies one word per frame from
// seapal/groundpal (MAIN.ASM:2924-2925, DATA/COL/SEA.COL and GROUND.COL
// row 0) into background CGRAM palette row 4, colors 15 down to 1, over
// 15 frames. Polygon colors live in the independent game palette row 7.
// ---------------------------------------------------------------------------

/// First 16 palette entries from SEA.COL (ROM `seapal`, MAIN.ASM:2924) —
/// the underwater blue ramp copied into background palette row 4.
#[rustfmt::skip]
pub static SEA_PALETTE: [u16; 16] = [
    0x0000, 0x34C0, 0x38E0, 0x3D00,
    0x4120, 0x4540, 0x4960, 0x4D80,
    0x51A0, 0x55C0, 0x59E0, 0x5E00,
    0x6220, 0x6640, 0x6A60, 0x6E80,
];

/// First 16 palette entries from GROUND.COL (ROM `groundpal`,
/// MAIN.ASM:2925) — the surface ramp copied into background palette row 4.
#[rustfmt::skip]
pub static GROUND_PALETTE: [u16; 16] = [
    0x0000, 0x0088, 0x00C9, 0x04EA,
    0x090B, 0x0D2C, 0x114D, 0x156E,
    0x198F, 0x1DB0, 0x21D1, 0x25F2,
    0x2A13, 0x2E34, 0x3255, 0x3676,
];

/// Source row selected by the typed background-palette fade state.
pub fn background_fade_palette_bgr(
    target: sf_core::scene::PaletteFadeTarget,
) -> &'static [u16; 16] {
    match target {
        sf_core::scene::PaletteFadeTarget::Sea => &SEA_PALETTE,
        sf_core::scene::PaletteFadeTarget::Ground => &GROUND_PALETTE,
    }
}

pub fn game_palette_bgr(palette: GamePalette) -> &'static [u16; 16] {
    match palette {
        GamePalette::Night => &crate::scene_color_data::NIGHT_GAME_PALETTE,
        GamePalette::Red => &crate::scene_color_data::RED_GAME_PALETTE,
        GamePalette::Blue => &crate::scene_color_data::BLUE_GAME_PALETTE,
    }
}

pub fn depth_color_banks(colors: DepthColors) -> &'static [&'static [u8; 32]; 4] {
    match colors {
        DepthColors::Night => &crate::scene_color_data::NIGHT_DEPTH_COLORS,
        DepthColors::Mist => &crate::scene_color_data::MIST_DEPTH_COLORS,
        DepthColors::Desert => &crate::scene_color_data::DESERT_DEPTH_COLORS,
        DepthColors::Marine => &crate::scene_color_data::MARINE_DEPTH_COLORS,
        DepthColors::Red => &crate::scene_color_data::RED_DEPTH_COLORS,
    }
}

pub const fn depth_threshold_index(thresholds: DepthThresholds) -> usize {
    match thresholds {
        DepthThresholds::Normal => DEPTHZ_NORMAL,
        DepthThresholds::Tunnel => DEPTHZ_TUNNEL,
        DepthThresholds::Mist => DEPTHZ_MIST,
        DepthThresholds::StageOne => DEPTHZ_STAGE1,
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

/// The default (no fade) decoded shape palette: NIGHT.COL.
pub fn night_shape_palette() -> ShapePaletteRgb {
    decode_shape_palette(&NIGHT_PALETTE)
}

/// Decode the exact polygon palette selected by semantic SF2 scene state.
pub fn sf2_polygon_shape_palette(palette: Sf2PolygonPalette) -> ShapePaletteRgb {
    decode_shape_palette(palette.bgr555())
}

/// Magenta diagnostic emitted only for invalid/unknown materials and by the
/// pure flat-color helper when handed a texture or dormant smooth material.
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

/// The byte-exact tables live in `color_data`; these stable ids are kept
/// because strategy state stores them directly.

// ---------------------------------------------------------------------------
// Shape word / color table id resolution (Shapes_ResolveShapeWord & friends)
// ---------------------------------------------------------------------------

/// Canonicalize the live raw/16-bit shape words used by current literal
/// slices into bounded flat runtime ids. Unknown words are returned
/// unchanged. Mirrors `Shapes_ResolveShapeWord`.
pub fn resolve_shape_word(shape_id: u16) -> u16 {
    sf_core::shape::resolve_shape_word(shape_id)
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

/// Resolve the ShapeHdr's `sh_col_ptr` retained by the generated mesh data.
/// Hand-authored replacement meshes keep their original header contract.
pub fn default_color_table_id(shape_id: u16) -> u16 {
    let base = resolve_base_color_shape(resolve_shape_word(shape_id));
    if base == SHAPE_MYSHIP_4 {
        return SHAPE_COLTAB_ID_0 as u16;
    }
    if base == SHAPE_ELASER2 {
        return crate::color_data::table_id_by_name("bullet_c").unwrap();
    }
    if is_boss7_shape(base) {
        return SHAPE_COLTAB_ID_1 as u16;
    }
    crate::shape_data::SHAPE_DATA
        .iter()
        .find(|entry| entry.shape_id == base)
        .and_then(|entry| crate::color_data::table_id_by_name(entry.default_color_table))
        .unwrap_or(SHAPE_COLTAB_ID_0 as u16)
}

/// A zero live `al_coltab` means the shape header table.  Non-zero values are
/// the stable runtime table ids used by the Rust strategy/path bridge.
pub fn resolve_color_table_id(shape_id: u16, color_table: u16) -> u16 {
    if color_table == 0 {
        return default_color_table_id(shape_id);
    }
    if (color_table as usize) < crate::color_data::COLOR_TABLES.len() {
        return color_table;
    }
    if let Some(table_id) = crate::color_data::table_id_by_source_word(color_table) {
        return table_id;
    }
    SHAPE_COLTAB_ID_INVALID as u16
}

// ---------------------------------------------------------------------------
// Color resolution (pure mirrors of the shapes.c statics)
// ---------------------------------------------------------------------------

/// Decode a SNES BGR555 word to its exact replicated eight-bit RGB value,
/// normalized for the GPU. The PPU expands five bits as `abcdeabc`, rather
/// than scaling by 255/31 and rounding through floating point.
pub fn decode_bgr555(color: u16) -> [f32; 3] {
    let expand = |component: u16| {
        let five_bits = component & 31;
        f32::from(((five_bits << 3) | (five_bits >> 2)) as u8) / 255.0
    };
    [expand(color), expand(color >> 5), expand(color >> 10)]
}

/// The two palette entries carried by every retail flat-polygon material.
/// Dithered drawing chooses [`Self::low`] when the source-pixel X/Y parity is
/// equal and [`Self::high`] when it differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PalettePair {
    pub low: u8,
    pub high: u8,
}

impl PalettePair {
    pub const fn from_packed(pair: u8) -> Self {
        Self {
            low: pair & 15,
            high: pair >> 4,
        }
    }

    pub const fn packed(self) -> u8 {
        (self.high << 4) | self.low
    }

    /// Retail dither selection for one source-raster pixel.
    pub const fn color_at(self, x: i32, y: i32) -> u8 {
        if (x ^ y) & 1 == 0 {
            self.low
        } else {
            self.high
        }
    }
}

/// Average a pair for CPU-side previews and color-family comparisons. The
/// live shape renderer preserves both palette entries and applies the exact
/// checkerboard in the fragment shader.
pub fn decode_palette_pair_in(pair: u8, palette: &ShapePaletteRgb) -> [f32; 4] {
    let pair = PalettePair::from_packed(pair);
    let lo = palette[pair.low as usize];
    let hi = palette[pair.high as usize];
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

/// Follow a ROM COLANIM pointer for this object's color frame.  The hardware
/// masks with `frame_count - 1`; generated records are byte-exact ROM data.
pub fn resolve_animated_material(mut material_word: u16, col_frame: u8) -> Option<u16> {
    // Valid material animation tables do not recurse, but retain a short loop
    // to mirror `.getwordagain` and safely reject corrupt/unreachable spills.
    for _ in 0..4 {
        if material_word & MATERIAL_COLANIM_FLAG == 0 || material_word & MATERIAL_COLTEXT_FLAG != 0
        {
            return Some(material_word);
        }
        let pointer = material_word & 0x3FFF;
        let frames = crate::color_data::animation_frames(pointer)?;
        if frames.is_empty() {
            return None;
        }
        material_word = frames[(col_frame as usize) & (frames.len() - 1)];
    }
    None
}

/// Resolve a flat material to the exact pair of palette entries consumed by
/// retail dithered drawing. Texture materials and invalid table references do
/// not have a flat pair and return `None`.
pub fn resolve_material_palette_pair_for_scene(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
    depth_colors: DepthColors,
) -> Option<PalettePair> {
    let source = (material_word >> 8) as u8;
    let bank = (depth_bank as usize).min(3);

    if material_word & MATERIAL_COLANIM_FLAG != 0 {
        if material_word & MATERIAL_COLTEXT_FLAG != 0 {
            return None;
        }
        let material = resolve_animated_material(material_word, col_frame)?;
        return resolve_material_palette_pair_for_scene(
            material,
            col_frame,
            shade_index,
            depth_bank,
            depth_colors,
        );
    }

    if material_word & MATERIAL_COLTEXT_FLAG != 0 {
        return None;
    }

    if source as u16 == MATERIAL_SOURCE_COLNORM {
        return Some(PalettePair::from_packed(material_word as u8));
    }

    if source as u16 == MATERIAL_SOURCE_COLDEPTH {
        let depth_index = material_word as u8 as usize;
        return depth_color_banks(depth_colors)[bank]
            .get(depth_index)
            .copied()
            .map(PalettePair::from_packed);
    }

    if (source as u16) < MATERIAL_COLLITE_SOURCE_LIMIT {
        let row = (source as usize).min(SHADE_SUBTABLES - 1);
        let shade = shade_index.clamp(0, SHADE_TABLE_LEN as i32 - 1) as usize;
        return Some(PalettePair::from_packed(SHADE_TABLES[bank][row][shade]));
    }

    None
}

/// Resolve one SF2 material through the exact retail SF2 depth and lighting
/// pair tables. Verified live missions all select the standard depth family;
/// the polygon palette itself remains an independent semantic scene input.
pub fn resolve_sf2_material_palette_pair(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
) -> Option<PalettePair> {
    let material_word = if material_word & MATERIAL_COLANIM_FLAG != 0 {
        if material_word & MATERIAL_COLTEXT_FLAG != 0 {
            return None;
        }
        sf2_data::colors::resolve_animated_material(material_word, col_frame)?
    } else {
        material_word
    };
    let source = (material_word >> 8) as u8;
    let bank = (depth_bank as usize).min(sf2_data::lighting::DEPTH_BANK_COUNT - 1);

    if material_word & MATERIAL_COLTEXT_FLAG != 0 {
        return None;
    }

    if source as u16 == MATERIAL_SOURCE_COLNORM {
        return Some(PalettePair::from_packed(material_word as u8));
    }

    if source as u16 == MATERIAL_SOURCE_COLDEPTH {
        let depth_index = material_word as u8 as usize;
        return sf2_data::lighting::STANDARD_DEPTH_PAIRS[bank]
            .get(depth_index)
            .copied()
            .map(PalettePair::from_packed);
    }

    if (source as u16) < MATERIAL_COLLITE_SOURCE_LIMIT {
        let row = (source as usize).min(sf2_data::lighting::SHADE_ROW_COUNT - 1);
        let shade = shade_index.clamp(0, sf2_data::lighting::SHADE_LEVEL_COUNT as i32 - 1) as usize;
        return Some(PalettePair::from_packed(
            sf2_data::lighting::SHADE_PAIRS[bank][row][shade],
        ));
    }

    None
}

/// Diagnostic averaged color for an SF2 material. The live GPU path retains
/// the returned pair and applies retail checkerboard selection per pixel.
pub fn resolve_sf2_material_color_in(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    resolve_sf2_material_palette_pair(material_word, col_frame, shade_index, depth_bank)
        .map_or(DEBUG_MATERIAL_COLOR, |pair| {
            decode_palette_pair_in(pair.packed(), palette)
        })
}

/// Resolve a material word to RGBA. Pure mirror of
/// `Shapes_ResolveMaterialColor`; `depth_bank` (0..=3) replaces the C
/// `s_current_depth_bank` global and selects both the COLDEPTH night bank
/// and the COLLITE shade group. Non-flat materials (COLTEXT, dormant
/// COLSMOOTH, COLLITE sources 12..62) resolve to [`DEBUG_MATERIAL_COLOR`];
/// the GPU renderer routes COLTEXT before calling this helper.
/// `palette` is the decoded BGS-selected polygon palette every nibble pair
/// resolves against.
pub fn resolve_material_color_for_scene(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
    depth_colors: DepthColors,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    resolve_material_palette_pair_for_scene(
        material_word,
        col_frame,
        shade_index,
        depth_bank,
        depth_colors,
    )
    .map_or(DEBUG_MATERIAL_COLOR, |pair| {
        decode_palette_pair_in(pair.packed(), palette)
    })
}

pub fn resolve_material_color_in(
    material_word: u16,
    col_frame: u8,
    shade_index: i32,
    depth_bank: u8,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    resolve_material_color_for_scene(
        material_word,
        col_frame,
        shade_index,
        depth_bank,
        DepthColors::Night,
        palette,
    )
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
    let Some(table) = crate::color_data::COLOR_TABLES.get(color_table_id as usize) else {
        return DEBUG_MATERIAL_COLOR;
    };
    let Some(&material_word) = table.entries.get(face_color_index as usize) else {
        return DEBUG_MATERIAL_COLOR;
    };
    resolve_material_color_in(material_word, col_frame, shade_index, depth_bank, palette)
}

/// Resolve a face to its final (post-COLANIM) material word.  The GPU shape
/// renderer uses this to route COLTEXT faces to the texture pipeline before
/// asking the flat-color resolver for an RGBA value.
pub fn resolve_face_material(
    shape_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
) -> Option<u16> {
    let id = resolve_color_table_id(shape_id, color_table);
    let table = crate::color_data::COLOR_TABLES.get(id as usize)?;
    let material = *table.entries.get(face_color_index as usize)?;
    resolve_animated_material(material, col_frame)
}

/// Resolve a face when the caller already retained the ShapeHdr table id.
pub fn resolve_face_material_from_table(
    default_table_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
) -> Option<u16> {
    let id = if color_table == 0 {
        default_table_id
    } else if (color_table as usize) < crate::color_data::COLOR_TABLES.len() {
        color_table
    } else {
        crate::color_data::table_id_by_source_word(color_table)?
    };
    let table = crate::color_data::COLOR_TABLES.get(id as usize)?;
    resolve_animated_material(*table.entries.get(face_color_index as usize)?, col_frame)
}

pub fn resolve_face_color_from_table_in(
    default_table_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    let Some(material) = resolve_face_material_from_table(
        default_table_id,
        face_color_index,
        col_frame,
        color_table,
    ) else {
        return DEBUG_MATERIAL_COLOR;
    };
    resolve_material_color_in(material, col_frame, shade_index, depth_bank, palette)
}

pub fn resolve_face_color_from_table_for_scene(
    default_table_id: u16,
    face_color_index: u8,
    col_frame: u8,
    color_table: u16,
    shade_index: i32,
    depth_bank: u8,
    depth_colors: DepthColors,
    palette: &ShapePaletteRgb,
) -> [f32; 4] {
    let Some(material) = resolve_face_material_from_table(
        default_table_id,
        face_color_index,
        col_frame,
        color_table,
    ) else {
        return DEBUG_MATERIAL_COLOR;
    };
    resolve_material_color_for_scene(
        material,
        col_frame,
        shade_index,
        depth_bank,
        depth_colors,
        palette,
    )
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

/// Authored signed-byte face normal dotted with the source-quantized rotated
/// light and mapped onto the ten shade levels (MOBJ.MC:4092-4128). Retail
/// keeps the normal's authored magnitude, converts each Q15 light component
/// to a signed byte, sums three signed 8x8 products, shifts by ten, clamps to
/// 6..15, then subtracts 6.
pub fn compute_shade_index(normal: [i16; 3], light_obj: [f32; 3]) -> i32 {
    const LIGHT_COMPONENT_SCALE: f32 = 128.0;
    const LIGHT_COMPONENT_MIN: f32 = -128.0;
    const LIGHT_COMPONENT_MAX: f32 = 127.0;

    let light = light_obj.map(|component| {
        (component * LIGHT_COMPONENT_SCALE)
            .floor()
            .clamp(LIGHT_COMPONENT_MIN, LIGHT_COMPONENT_MAX) as i8
    });
    compute_quantized_shade_index(normal, light)
}

pub fn compute_quantized_shade_index(normal: [i16; 3], light_obj: [i8; 3]) -> i32 {
    const SHADE_SHIFT: u32 = 10;
    const SHADE_MIN: i32 = 6;
    const SHADE_MAX: i32 = 15;

    let dot = i32::from(normal[0]) * i32::from(light_obj[0])
        + i32::from(normal[1]) * i32::from(light_obj[1])
        + i32::from(normal[2]) * i32::from(light_obj[2]);
    (dot >> SHADE_SHIFT).clamp(SHADE_MIN, SHADE_MAX) - SHADE_MIN
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
