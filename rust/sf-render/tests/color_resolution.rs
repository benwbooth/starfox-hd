//! Color-resolution tests against the assembled SF1 ROM data and semantics.
//!
//! CPU-preview RGBA values are computed independently here from the NIGHT.COL
//! BGR555 palette. Live GPU rendering retains both palette nibbles and applies
//! exact source-raster dithering; these average checks remain only for the
//! diagnostic preview helper.

use sf_render::color_data::{self, COLOR_TABLES};
use sf_render::shapes::{
    compute_shade_index, decode_palette_pair, material_colanim, material_coldepth,
    material_collite, material_colnorm, material_coltext, resolve_face_color,
    resolve_face_material, resolve_material_color, resolve_material_palette_pair_for_scene,
    resolve_sf2_material_palette_pair, select_depth_bank, PalettePair, DEBUG_MATERIAL_COLOR,
    DEPTHZ_MIST, DEPTHZ_NORMAL, DEPTHZ_STAGE1, DEPTHZ_TUNNEL, LIGHT_DIR, NIGHT_PALETTE,
    SHAPE_ANIM_CA_2, SHAPE_BOSS7_1, SHAPE_ELASER2,
};

/// Independent BGR555 + nibble-pair-average reference (mirrors the SNES
/// 5-bit channel layout: bits 0-4 red, 5-9 green, 10-14 blue).
fn expected_pair(pair: u8) -> [f32; 4] {
    let expand = |component: u16| {
        let five_bits = component & 31;
        f32::from(((five_bits << 3) | (five_bits >> 2)) as u8) / 255.0
    };
    let decode = |c: u16| -> [f32; 3] { [expand(c), expand(c >> 5), expand(c >> 10)] };
    let lo = decode(NIGHT_PALETTE[(pair & 0x0F) as usize]);
    let hi = decode(NIGHT_PALETTE[(pair >> 4) as usize]);
    [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
        1.0,
    ]
}

const ARWING_SHAPE: u16 = 3; // non-boss7 shape, color_table 0 -> id_0_c
const TRAINING_RING_SHAPE: u16 = 482;
const TRAINING_RING_FACE_COLOR: u8 = 14;
const TRAINING_RING_SOURCE_COLOR_TABLE: u16 = 0x8481;

#[test]
fn source_light_uses_renderer_coordinate_basis() {
    const SOURCE_LIGHT_COMPONENT: f32 = 18_917.0 / 32_768.0;
    assert_eq!(LIGHT_DIR[0], SOURCE_LIGHT_COMPONENT);
    assert_eq!(LIGHT_DIR[1], -SOURCE_LIGHT_COMPONENT);
    assert_eq!(LIGHT_DIR[2], SOURCE_LIGHT_COMPONENT);
}

/// Resolve an id_0_c FX slot the way the renderer does for Arwing faces.
fn resolve_fx(fx: u8, col_frame: u8, shade_index: i32, depth_bank: u8) -> [f32; 4] {
    resolve_face_color(ARWING_SHAPE, fx, col_frame, 0, shade_index, depth_bank)
}

#[test]
fn coltab_id0_layout() {
    let id0 = COLOR_TABLES[0].entries;
    assert_eq!(id0.len(), 109, "ROM ID_0_C is exactly FX0-FX108");
    assert_eq!(id0[0], material_collite(0, 0));
    assert_eq!(id0[10], material_coldepth(0));
    assert_eq!(id0[20], material_coldepth(10));
    assert_eq!(id0[28], material_coldepth(18));
    assert_eq!(id0[44], material_colanim(SHAPE_ANIM_CA_2));
    assert_eq!(id0[48], material_coltext(0x1B)); // asteroid1_spr
    assert_eq!(id0[52], 0x878C); // gateflash_a1, not a texture
    assert_eq!(id0[71], material_colnorm(0x9, 0x9));
    assert_eq!(id0[95], material_colnorm(0x8, 0x8));
    assert_eq!(id0[108], material_coltext(0x04)); // tunnelwall_spr
    assert_eq!(COLOR_TABLES[1].entries.len(), 106);
    assert_eq!(COLOR_TABLES[5].entries.len(), 48);
}

#[test]
fn custom_shape_tables_and_animation_records_are_retained() {
    let asteroid = color_data::table_id_by_name("asteroid_c").unwrap();
    assert_eq!(COLOR_TABLES[asteroid as usize].entries, &[0x401B]);

    // The runtime-only laser shape now uses bullet_c instead of a synthetic
    // extra ID_0_C slot. bullet_a1 has all eight ROM frames.
    assert_eq!(resolve_face_material(SHAPE_ELASER2, 0, 0, 0), Some(0x3FEE));
    assert_eq!(resolve_face_material(SHAPE_ELASER2, 0, 7, 0), Some(0x3F66));
    assert_eq!(
        color_data::animation_frames(color_data::ANIM_PTR_BULLET_A1)
            .unwrap()
            .len(),
        8
    );
}

#[test]
fn source_color_table_words_normalize_to_typed_table_ids() {
    assert_eq!(
        color_data::table_id_by_source_word(TRAINING_RING_SOURCE_COLOR_TABLE),
        Some(color_data::COLOR_TABLE_ID_5_C)
    );
    assert_eq!(
        resolve_face_material(
            TRAINING_RING_SHAPE,
            TRAINING_RING_FACE_COLOR,
            0,
            TRAINING_RING_SOURCE_COLOR_TABLE,
        ),
        resolve_face_material(
            TRAINING_RING_SHAPE,
            TRAINING_RING_FACE_COLOR,
            0,
            color_data::COLOR_TABLE_ID_5_C,
        )
    );
}

#[test]
fn generated_player_laser_faces_use_the_rom_bullet_table() {
    let laser = sf_render::shape_data::SHAPE_DATA
        .iter()
        .find(|entry| entry.shape_id == SHAPE_ELASER2)
        .expect("generated elaser2 shape");
    assert_eq!(laser.animation_frames.len(), 9);
    for face in laser.faces {
        assert_eq!(face.color_index, 0, "bullet_c contains exactly one entry");
        assert!(face.visibility_vertices.is_some());
    }
    for frame in 0..8 {
        assert_ne!(
            resolve_face_color(SHAPE_ELASER2, 0, frame, 0, 0, 0),
            DEBUG_MATERIAL_COLOR,
            "bullet_a1 frame {frame} must resolve"
        );
    }
}

#[test]
fn arwing_face_colors_id0() {
    // Table-driven: (fx, depth_bank, expected night pair).
    // FX10 = COLDEPTH(0), FX20 = COLDEPTH(10), FX28 = COLDEPTH(18); the
    // night1 (bank 0) entries at those indices are 0x99 / 0xEE / 0x55.
    let cases: &[(u8, u8, u8)] = &[
        (10, 0, 0x99), // FX10 -> palette 9 pair 0x99
        (20, 0, 0xEE), // FX20 -> 0xEE
        (28, 0, 0x55), // FX28 -> 0x55
        // Deeper banks re-map the same COLDEPTH index (COLTAB.ASM night2/4).
        (20, 1, 0xDD), // night2[10]
        (20, 3, 0xBB), // night4[10]
        (10, 3, 0x99), // night4[0]
    ];
    for &(fx, bank, pair) in cases {
        assert_eq!(
            resolve_fx(fx, 0, 0, bank),
            expected_pair(pair),
            "FX{fx} bank {bank}"
        );
    }
}

#[test]
fn arwing_fx44_ca2_cycle() {
    // FX44 = COLANIM(CA_2): COLNORM1 ramp 8 -> 7 -> 6 -> 5, and the Super FX
    // masks col_frame with (frame_count - 1), so frame 4 wraps to 0x88.
    let cycle: &[(u8, u8)] = &[
        (0, 0x88),
        (1, 0x77),
        (2, 0x66),
        (3, 0x55),
        (4, 0x88), // frame & 3 wraps
        (7, 0x55),
    ];
    for &(col_frame, pair) in cycle {
        assert_eq!(
            resolve_fx(44, col_frame, 0, 0),
            expected_pair(pair),
            "FX44 col_frame {col_frame}"
        );
    }
}

#[test]
fn collite_uses_shade_tables_and_depth_group() {
    // FX0 = COLLITE row 0. Shade group = depth bank (LIGHT.ASM shades0_0 /
    // shades3_0), shade index selects within the row.
    assert_eq!(resolve_fx(0, 0, 9, 0), expected_pair(0xEE)); // shades0_0[9]
    assert_eq!(resolve_fx(0, 0, 0, 0), expected_pair(0xAB)); // shades0_0[0]
    assert_eq!(resolve_fx(0, 0, 9, 3), expected_pair(0xBB)); // shades3_0[9]
                                                             // Out-of-range shade indices clamp like the C code.
    assert_eq!(resolve_fx(0, 0, 42, 0), expected_pair(0xEE));
    assert_eq!(resolve_fx(0, 0, -3, 0), expected_pair(0xAB));
}

#[test]
fn collite_row_alias_10_11() {
    // shades0_9 (LIGHT.ASM) = ... index 9 = 0xef.
    let expected = expected_pair(0xEF);
    assert_eq!(
        resolve_material_color(material_collite(10, 0), 0, 9, 0),
        expected
    );
    assert_eq!(
        resolve_material_color(material_collite(11, 0), 0, 9, 0),
        expected
    );
}

#[test]
fn sf2_uses_its_exact_depth_and_light_pairs() {
    let sf1_depth = |index, bank| {
        resolve_material_palette_pair_for_scene(
            material_coldepth(index),
            0,
            0,
            bank,
            sf_core::scene::DepthColors::Night,
        )
        .unwrap()
        .packed()
    };
    let sf2_depth = |index, bank| {
        resolve_sf2_material_palette_pair(material_coldepth(index), 0, 0, bank)
            .unwrap()
            .packed()
    };

    assert_eq!(sf1_depth(25, 0), 0x16);
    assert_eq!(sf2_depth(25, 0), 0x0E);
    assert_eq!(sf1_depth(25, 1), 0x99);
    assert_eq!(sf2_depth(25, 1), 0x0D);

    let sf1_light = resolve_material_palette_pair_for_scene(
        material_collite(6, 0),
        0,
        0,
        0,
        sf_core::scene::DepthColors::Night,
    )
    .unwrap();
    let sf2_light = resolve_sf2_material_palette_pair(material_collite(6, 0), 0, 0, 0).unwrap();
    assert_eq!(sf1_light.packed(), 0x19);
    assert_eq!(sf2_light.packed(), 0x11);

    let alias_9 = resolve_sf2_material_palette_pair(material_collite(9, 0), 0, 9, 0);
    assert_eq!(
        resolve_sf2_material_palette_pair(material_collite(10, 0), 0, 9, 0),
        alias_9
    );
    assert_eq!(
        resolve_sf2_material_palette_pair(material_collite(11, 0), 0, 9, 0),
        alias_9
    );
}

#[test]
fn boss7_defaults_to_id1() {
    // color_table 0 on a boss7 shape resolves through ID_1_C, whose row 2 is
    // COLLITE(3,3) instead of ID_0_C's COLLITE(2,2): shades0_3[9] = 0xEE vs
    // shades0_2[9] = 0xEE -- use shade 0 where they differ:
    // shades0_3[0] = 0x59, shades0_2[0] = 0x19.
    assert_eq!(
        resolve_face_color(SHAPE_BOSS7_1, 2, 0, 0, 0, 0),
        expected_pair(0x59)
    );
    assert_eq!(
        resolve_face_color(ARWING_SHAPE, 2, 0, 0, 0, 0),
        expected_pair(0x19)
    );
}

#[test]
fn non_flat_materials_do_not_forge_a_flat_color() {
    // COLTEXT is routed by ShapeStore to the 3D texture pipeline; the pure
    // flat-color helper deliberately has no texture/sample inputs.
    assert_eq!(resolve_fx(48, 0, 0, 0), DEBUG_MATERIAL_COLOR);
    // COLSMOOTH is disabled in the shipped MOBJ build (`msmooth_shading=0`)
    // and SMOOTH_c is under `IFEQ 1`, so no retail shape can select it.
    assert_eq!(
        resolve_material_color(0xC000, 0, 0, 0),
        DEBUG_MATERIAL_COLOR,
        "COLSMOOTH"
    );
    // COLLITE sources 12..61 are unsupported.
    assert_eq!(
        resolve_material_color(material_collite(20, 0), 0, 0, 0),
        DEBUG_MATERIAL_COLOR
    );
    // COLDEPTH index past the 32 live entries.
    assert_eq!(
        resolve_material_color(material_coldepth(32), 0, 0, 0),
        DEBUG_MATERIAL_COLOR
    );
    // Face color index past the table end.
    assert_eq!(
        resolve_face_color(ARWING_SHAPE, 200, 0, 0, 0, 0),
        DEBUG_MATERIAL_COLOR
    );
    // Unknown color table word.
    assert_eq!(
        resolve_face_color(ARWING_SHAPE, 0, 0, 0x1234, 0, 0),
        DEBUG_MATERIAL_COLOR
    );
}

#[test]
fn colnorm_decodes_nibble_pair() {
    assert_eq!(
        resolve_material_color(material_colnorm(0x4, 0xE), 0, 0, 0),
        expected_pair(0xE4)
    );
    assert_eq!(decode_palette_pair(0xE4), expected_pair(0xE4));
}

#[test]
fn palette_pair_preserves_retail_checkerboard_selection() {
    let pair = PalettePair::from_packed(0xE4);
    assert_eq!(pair, PalettePair { low: 4, high: 14 });
    assert_eq!(pair.packed(), 0xE4);
    assert_eq!(pair.color_at(0, 0), 4);
    assert_eq!(pair.color_at(1, 0), 14);
    assert_eq!(pair.color_at(0, 1), 14);
    assert_eq!(pair.color_at(1, 1), 4);
    assert_eq!(pair.color_at(-1, 0), 14);

    assert_eq!(
        resolve_material_palette_pair_for_scene(
            material_colnorm(4, 14),
            0,
            0,
            0,
            sf_core::scene::DepthColors::Night,
        ),
        Some(pair)
    );
}

#[test]
fn shade_curve_edge_cases() {
    // GSU intensity curve: signed-byte dot, arithmetic shift 10, clamp 6..15.
    let n = [127, 0, 0];
    // Light 1.0 quantizes to 127; 127*127 >> 10 = 15 -> shade 9.
    assert_eq!(compute_shade_index(n, [1.0, 0.0, 0.0]), 9);
    // Light 0.38 quantizes to 48; 127*48 >> 10 = 5 -> clamped to 6.
    assert_eq!(compute_shade_index(n, [0.38, 0.0, 0.0]), 0);
    // Negative dot products clamp to shade 0.
    assert_eq!(compute_shade_index(n, [-0.7, 0.0, 0.0]), 0);
    // Light 0.45 quantizes to 57; 127*57 >> 10 = 7 -> shade 1.
    assert_eq!(compute_shade_index(n, [0.45, 0.0, 0.0]), 1);
    // A zero authored normal follows the same source arithmetic.
    assert_eq!(compute_shade_index([0, 0, 0], [1.0, 0.0, 0.0]), 0);
}

#[test]
fn depth_bank_selection() {
    // COLTABS.ASM depthtables: normal = $a00/$d00/$f00.
    assert_eq!(select_depth_bank(0.0, DEPTHZ_NORMAL), 0);
    assert_eq!(select_depth_bank(2559.9, DEPTHZ_NORMAL), 0);
    assert_eq!(select_depth_bank(2560.0, DEPTHZ_NORMAL), 1);
    assert_eq!(select_depth_bank(3328.0, DEPTHZ_NORMAL), 2);
    assert_eq!(select_depth_bank(3840.0, DEPTHZ_NORMAL), 3);
    assert_eq!(select_depth_bank(1e9, DEPTHZ_NORMAL), 3);
    // Decimal tunnel/mist values are stored as negative high bytes, yielding
    // effective 512/768/1024 boundaries.
    assert_eq!(select_depth_bank(750.0, DEPTHZ_TUNNEL), 1);
    assert_eq!(select_depth_bank(750.0, DEPTHZ_MIST), 1);
    assert_eq!(select_depth_bank(768.0, DEPTHZ_TUNNEL), 2);
    assert_eq!(select_depth_bank(768.0, DEPTHZ_MIST), 2);
    // Stage1 keeps band 2 out to $3f00.
    assert_eq!(select_depth_bank(5000.0, DEPTHZ_STAGE1), 2);
    assert_eq!(select_depth_bank(16128.0, DEPTHZ_STAGE1), 3);
    // Out-of-range table falls back to NORMAL like Shapes_SetDepthTable.
    assert_eq!(select_depth_bank(2560.0, 99), 1);
}

#[test]
fn scene_profiles_select_generated_palette_and_depth_families() {
    use sf_core::scene::{DepthColors, GamePalette};
    use sf_render::shapes::{
        decode_shape_palette, game_palette_bgr, material_coldepth, resolve_material_color_for_scene,
    };

    let night = decode_shape_palette(game_palette_bgr(GamePalette::Night));
    let red = decode_shape_palette(game_palette_bgr(GamePalette::Red));
    let blue = decode_shape_palette(game_palette_bgr(GamePalette::Blue));
    assert_ne!(night, red);
    assert_ne!(night, blue);

    let material = material_coldepth(1);
    let night_depth = resolve_material_color_for_scene(material, 0, 0, 2, DepthColors::Night, &red);
    let red_depth = resolve_material_color_for_scene(material, 0, 0, 2, DepthColors::Red, &red);
    assert_ne!(night_depth, red_depth);
}

#[test]
fn sf2_scene_palettes_match_the_live_polygon_cgram_rows() {
    use sf2_data::palettes::PolygonPaletteId;
    use sf_render::shapes::{sf2_polygon_shape_palette, Sf2PolygonPalette};

    assert_eq!(
        Sf2PolygonPalette::Standard.bgr555(),
        PolygonPaletteId::Standard.colors()
    );
    assert_eq!(
        Sf2PolygonPalette::EladardSurface.bgr555(),
        PolygonPaletteId::EladardSurface.colors()
    );
    assert_eq!(
        Sf2PolygonPalette::AstropolisExterior.bgr555(),
        PolygonPaletteId::AstropolisExterior.colors()
    );
    assert_eq!(
        PolygonPaletteId::EladardSurface.colors(),
        &[
            0x0000, 0x0C6F, 0x1559, 0x22BD, 0x377F, 0x4C63, 0x6D29, 0x7E8F, 0x7F76, 0x10C4, 0x2148,
            0x35ED, 0x52D4, 0x6B9A, 0x7BFE, 0x1B46,
        ]
    );
    assert_eq!(
        PolygonPaletteId::AstropolisExterior.colors(),
        &[
            0x0000, 0x28B1, 0x395A, 0x4A9E, 0x573F, 0x60A5, 0x7D4A, 0x7EB0, 0x7F97, 0x24C4, 0x3548,
            0x4E0E, 0x66D4, 0x7B9A, 0x7FFE, 0x4F06,
        ]
    );

    let standard = sf2_polygon_shape_palette(Sf2PolygonPalette::Standard);
    let eladard = sf2_polygon_shape_palette(Sf2PolygonPalette::EladardSurface);
    let astropolis = sf2_polygon_shape_palette(Sf2PolygonPalette::AstropolisExterior);
    assert_eq!(standard[6], sf_render::shapes::decode_bgr555(0x6D29));
    assert_ne!(standard[9], eladard[9]);
    assert_ne!(standard[1], astropolis[1]);
}
