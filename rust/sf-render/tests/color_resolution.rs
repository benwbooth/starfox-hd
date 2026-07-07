//! Color-resolution tests against `src/renderer/shapes.c` semantics.
//!
//! Expected RGBA values are computed independently here from the NIGHT.COL
//! BGR555 palette (decode + nibble-pair average), so a table or decode
//! regression in the crate cannot cancel itself out.

use sf_render::shapes::{
    compute_shade_index, decode_palette_pair, material_colanim,
    material_coldepth, material_collite, material_colnorm, material_coltext,
    resolve_face_color, resolve_material_color, select_depth_bank,
    COLTAB_ID0, COLTAB_ID1, COLTAB_ID5, DEBUG_MATERIAL_COLOR, DEPTHZ_MIST,
    DEPTHZ_NORMAL, DEPTHZ_STAGE1, DEPTHZ_TUNNEL, NIGHT_PALETTE,
    SHAPE_ANIM_CA_2, SHAPE_BOSS7_1,
};

/// Independent BGR555 + nibble-pair-average reference (mirrors the SNES
/// 5-bit channel layout: bits 0-4 red, 5-9 green, 10-14 blue).
fn expected_pair(pair: u8) -> [f32; 4] {
    let decode = |c: u16| -> [f32; 3] {
        [
            (c & 0x1F) as f32 / 31.0,
            ((c >> 5) & 0x1F) as f32 / 31.0,
            ((c >> 10) & 0x1F) as f32 / 31.0,
        ]
    };
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

/// Resolve an id_0_c FX slot the way the renderer does for Arwing faces.
fn resolve_fx(fx: u8, col_frame: u8, shade_index: i32, depth_bank: u8) -> [f32; 4] {
    resolve_face_color(ARWING_SHAPE, fx, col_frame, 0, shade_index, depth_bank)
}

#[test]
fn coltab_id0_layout() {
    // FX0-FX108 from the ROM + FX109 = bullet_c colanim added for the player
    // laser shimmer (obj colframe fix). See shapes.rs COLTAB_ID0.
    assert_eq!(COLTAB_ID0.len(), 110, "id_0_c = FX0-FX108 + FX109 bullet_c");
    assert_eq!(COLTAB_ID0[0], material_collite(0, 0));
    assert_eq!(COLTAB_ID0[10], material_coldepth(0)); // FX10
    assert_eq!(COLTAB_ID0[20], material_coldepth(10)); // FX20
    assert_eq!(COLTAB_ID0[28], material_coldepth(18)); // FX28
    assert_eq!(COLTAB_ID0[44], material_colanim(SHAPE_ANIM_CA_2)); // FX44
    assert_eq!(COLTAB_ID0[48], material_coltext(0)); // FX48
    assert_eq!(COLTAB_ID0[71], material_colnorm(0x9, 0x9)); // FX71
    assert_eq!(COLTAB_ID0[95], material_colnorm(0x8, 0x8)); // FX95
    assert_eq!(COLTAB_ID0[108], material_coltext(31)); // FX108
    assert_eq!(COLTAB_ID1.len(), 48);
    assert_eq!(COLTAB_ID5.len(), 48);
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
    assert_eq!(resolve_material_color(material_collite(10, 0), 0, 9, 0), expected);
    assert_eq!(resolve_material_color(material_collite(11, 0), 0, 9, 0), expected);
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
    assert_eq!(resolve_face_color(ARWING_SHAPE, 2, 0, 0, 0, 0), expected_pair(0x19));
}

#[test]
fn unsupported_materials_are_debug_magenta() {
    // COLTEXT (FX48) and COLSMOOTH stay unported.
    assert_eq!(resolve_fx(48, 0, 0, 0), DEBUG_MATERIAL_COLOR);
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
fn shade_curve_edge_cases() {
    // GSU intensity curve: clamp(floor(dot * 15.75), 6, 15) - 6.
    let n = [1.0, 0.0, 0.0];
    // dot = 1 -> floor(15.75) = 15 -> 9.
    assert_eq!(compute_shade_index(n, [1.0, 0.0, 0.0]), 9);
    // dot = 0.38 -> floor(5.985) = 5 -> clamped to 6 -> 0.
    assert_eq!(compute_shade_index(n, [0.38, 0.0, 0.0]), 0);
    // dot < 0 -> 0.
    assert_eq!(compute_shade_index(n, [-0.7, 0.0, 0.0]), 0);
    // Just past the knee: dot = 0.45 -> floor(7.0875) = 7 -> 1.
    assert_eq!(compute_shade_index(n, [0.45, 0.0, 0.0]), 1);
    // Degenerate zero normal -> fully lit (viewer behavior).
    assert_eq!(compute_shade_index([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), 9);
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
    // Tunnel/mist share 500/750/1000.
    assert_eq!(select_depth_bank(750.0, DEPTHZ_TUNNEL), 2);
    assert_eq!(select_depth_bank(750.0, DEPTHZ_MIST), 2);
    // Stage1 keeps band 2 out to $3f00.
    assert_eq!(select_depth_bank(5000.0, DEPTHZ_STAGE1), 2);
    assert_eq!(select_depth_bank(16128.0, DEPTHZ_STAGE1), 3);
    // Out-of-range table falls back to NORMAL like Shapes_SetDepthTable.
    assert_eq!(select_depth_bank(2560.0, 99), 1);
}

// ---------------------------------------------------------------------------
// FADETOSEA / FADETOGROUND shape-palette fade (WORLD.ASM:371-394 +
// MAIN.ASM:2762 fadepalto_l; SEA.COL / GROUND.COL row 0)
// ---------------------------------------------------------------------------

#[test]
fn palette_fade_endpoints_and_reversal() {
    use sf_render::shapes::{
        decode_shape_palette, mixed_shape_palette, GROUND_PALETTE,
        PAL_TARGET_GROUND, PAL_TARGET_NIGHT, PAL_TARGET_SEA, SEA_PALETTE,
    };
    let night = decode_shape_palette(&NIGHT_PALETTE);
    let sea = decode_shape_palette(&SEA_PALETTE);
    let ground = decode_shape_palette(&GROUND_PALETTE);

    // fade 0 = source palette, fade 1 = target palette (exact endpoints).
    assert_eq!(mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 0.0), night);
    assert_eq!(mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 1.0), sea);
    // fadetoground reverses a completed sea fade toward groundpal.
    assert_eq!(mixed_shape_palette(PAL_TARGET_SEA, PAL_TARGET_GROUND, 0.0), sea);
    assert_eq!(mixed_shape_palette(PAL_TARGET_SEA, PAL_TARGET_GROUND, 1.0), ground);
    // Mid-fade is strictly between the endpoints on a channel where the
    // palettes differ (entry 9, red channel: night 0x0CA3 vs sea 0x55C0).
    let mid = mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 0.5);
    let (lo, hi) = if night[9][0] < sea[9][0] { (night[9][0], sea[9][0]) } else { (sea[9][0], night[9][0]) };
    assert!(mid[9][0] > lo && mid[9][0] < hi);
    // Out-of-range fades clamp.
    assert_eq!(mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 2.0), sea);
    // No-fade (from == target) is the identity regardless of fraction.
    assert_eq!(mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_NIGHT, 1.0), night);
}

#[test]
fn palette_fade_changes_resolved_face_colors() {
    use sf_render::shapes::{
        mixed_shape_palette, resolve_face_color_in, SEA_PALETTE,
        PAL_TARGET_NIGHT, PAL_TARGET_SEA,
    };
    // FX71 = COLNORM(9,9): resolves to palette entry 9 directly.
    let base = resolve_face_color(ARWING_SHAPE, 71, 0, 0, 0, 0);
    assert_eq!(base, expected_pair(0x99));

    let sea_pal = mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 1.0);
    let faded = resolve_face_color_in(ARWING_SHAPE, 71, 0, 0, 0, 0, &sea_pal);
    assert_ne!(faded, base, "full sea fade must recolor COLNORM faces");
    // Independent expectation: decoded SEA.COL entry 9.
    let c = SEA_PALETTE[9];
    let expected = [
        (c & 0x1F) as f32 / 31.0,
        ((c >> 5) & 0x1F) as f32 / 31.0,
        ((c >> 10) & 0x1F) as f32 / 31.0,
        1.0,
    ];
    assert_eq!(faded, expected);

    // fade 0 reproduces the unfaded resolver output bit-exactly.
    let idle = mixed_shape_palette(PAL_TARGET_NIGHT, PAL_TARGET_SEA, 0.0);
    assert_eq!(resolve_face_color_in(ARWING_SHAPE, 71, 0, 0, 0, 0, &idle), base);
    // COLDEPTH and COLLITE materials funnel through the same palette:
    // they must shift under the fade too (FX10 = coldepth(0), FX0 = collite).
    assert_ne!(
        resolve_face_color_in(ARWING_SHAPE, 10, 0, 0, 0, 0, &sea_pal),
        resolve_face_color(ARWING_SHAPE, 10, 0, 0, 0, 0)
    );
    assert_ne!(
        resolve_face_color_in(ARWING_SHAPE, 0, 0, 0, 9, 0, &sea_pal),
        resolve_face_color(ARWING_SHAPE, 0, 0, 0, 9, 0)
    );
}
