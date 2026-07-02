#include "shapes.h"
#include "shape_data.h"
#include "light_data.h"
#include "gl_backend.h"
#include "transform.h"
#include <glad/glad.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#define MAX_SHAPES 512

static Shape s_shapes[MAX_SHAPES];
static int s_shape_count = 0;

enum {
    RAW_SHAPE_BOSS_7_1 = 241u,
    RAW_SHAPE_IMYSHIP_4 = 554u,
    SHAPE_INTERNAL_BOSS7_0_FRAME1 = 480u,
    SHAPE_INTERNAL_BOSS7_3_FRAME1 = 488u,
    SHAPE_INTERNAL_BOSS7_4_FRAME1 = 497u,
    SHAPE_INTERNAL_BOSS7_0_FRAME_COUNT = 8u,
    SHAPE_INTERNAL_BOSS7_3_FRAME_COUNT = 9u,
    SHAPE_INTERNAL_BOSS7_4_FRAME_COUNT = 9u,
    SHAPE_COLTAB_ID_0 = 0u,
    SHAPE_COLTAB_ID_1 = 1u,
    SHAPE_COLTAB_ID_2 = 2u,
    SHAPE_COLTAB_ID_3 = 3u,
    SHAPE_COLTAB_ID_4 = 4u,
    SHAPE_COLTAB_ID_5 = 5u,
    SHAPE_COLTAB_ID_INVALID = 0xFFu,
    SHAPE_ANIM_CA_0 = 0u,
    SHAPE_ANIM_CA_1 = 1u,
    SHAPE_ANIM_CA_2 = 2u,
    SHAPE_ANIM_CA_3 = 3u,
    SHAPE_ANIM_CA_4 = 4u,
    SHAPE_ANIM_CA_5 = 5u,
    SHAPE_ANIM_COUNT = 6u,
};

typedef struct {
    const uint16 *frames;
    uint8 frame_count;
    const char *name;
} ShapeAnimTable;

typedef struct {
    const uint16 *entries;
    size_t count;
    const char *name;
} ShapeColorTable;

#define MATERIAL_COLANIM(anim_id) \
    ((uint16)(0x8000u | (uint16)(anim_id)))
#define MATERIAL_COLLITE(light_source, normal_color) \
    ((uint16)(((((uint16)(light_source)) & 0x3Fu) << 8) | \
              ((uint16)(normal_color) & 0xFFu)))
#define MATERIAL_COLDEPTH(index) \
    ((uint16)((62u << 8) | ((uint16)(index) & 0xFFu)))
#define MATERIAL_COLNORM(color_lo, color_hi) \
    ((uint16)((63u << 8) | ((((uint16)(color_hi)) & 0x0Fu) << 4) | \
             (((uint16)(color_lo)) & 0x0Fu)))
#define MATERIAL_COLNORM1(color) MATERIAL_COLNORM((color), (color))
// COLTEXT sprite-texture material (unported; resolves via the
// unsupported-material path). Low byte disambiguates the source sprite.
#define MATERIAL_COLTEXT(spr) ((uint16)(0x4000u | (spr)))

// Keep the literal phase-1 scope to the first 48 material slots of the shared
// polygon colour tables from COLTABS.ASM. Those are the flat-shaded entries
// used by the active built-in meshes; the later texture-heavy slots remain an
// explicit unsupported case in this renderer.
static const uint16 s_anim_ca_0[] = {
    MATERIAL_COLNORM1(0xEu),
    MATERIAL_COLNORM1(0x7u),
    MATERIAL_COLNORM1(0x2u),
    MATERIAL_COLNORM1(0xFu),
};

static const uint16 s_anim_ca_1[] = {
    MATERIAL_COLNORM1(0x4u),
    MATERIAL_COLNORM1(0x3u),
    MATERIAL_COLNORM1(0x2u),
    MATERIAL_COLNORM1(0x1u),
};

static const uint16 s_anim_ca_2[] = {
    MATERIAL_COLNORM1(0x8u),
    MATERIAL_COLNORM1(0x7u),
    MATERIAL_COLNORM1(0x6u),
    MATERIAL_COLNORM1(0x5u),
};

static const uint16 s_anim_ca_3[] = {
    MATERIAL_COLNORM1(0xEu),
    MATERIAL_COLNORM1(0xDu),
    MATERIAL_COLNORM1(0xCu),
    MATERIAL_COLNORM1(0xBu),
};

static const uint16 s_anim_ca_4[] = {
    MATERIAL_COLNORM(0x4u, 0xEu),
    MATERIAL_COLNORM(0x6u, 0x8u),
    MATERIAL_COLNORM(0x2u, 0x8u),
    MATERIAL_COLNORM(0x1u, 0x3u),
};

static const uint16 s_anim_ca_5[] = {
    MATERIAL_COLNORM(0x0u, 0xEu),
    MATERIAL_COLNORM(0x0u, 0x7u),
    MATERIAL_COLNORM(0x0u, 0x2u),
    MATERIAL_COLNORM(0x0u, 0xFu),
};

static const ShapeAnimTable s_anim_tables[SHAPE_ANIM_COUNT] = {
    [SHAPE_ANIM_CA_0] = { s_anim_ca_0, (uint8)ARRAY_SIZE(s_anim_ca_0), "CA_0" },
    [SHAPE_ANIM_CA_1] = { s_anim_ca_1, (uint8)ARRAY_SIZE(s_anim_ca_1), "CA_1" },
    [SHAPE_ANIM_CA_2] = { s_anim_ca_2, (uint8)ARRAY_SIZE(s_anim_ca_2), "CA_2" },
    [SHAPE_ANIM_CA_3] = { s_anim_ca_3, (uint8)ARRAY_SIZE(s_anim_ca_3), "CA_3" },
    [SHAPE_ANIM_CA_4] = { s_anim_ca_4, (uint8)ARRAY_SIZE(s_anim_ca_4), "CA_4" },
    [SHAPE_ANIM_CA_5] = { s_anim_ca_5, (uint8)ARRAY_SIZE(s_anim_ca_5), "CA_5" },
};

// World-space unit light direction. MOBJ.MC `initlight` (:810-817) compiles
// the constant 18917/32767 = 0.5773 into all three components; per object the
// GSU rotates it into object space (m_rotlightx/y/z, MOBJ.MC:905-922) before
// the per-face normal dot product, so shading tracks object rotation.
static const float s_light_dir[3] = {
    0.57735026919f, 0.57735026919f, 0.57735026919f
};

// `night1` from ASM/COLTAB.ASM, kept to the 32 live COLDEPTH entries used by
// the active built-in shapes.
static const uint8 s_night1_depth_pairs[] = {
    0x99u, 0x9Au, 0xAAu, 0xABu, 0xBBu, 0xBCu, 0xCCu, 0xCDu,
    0xDDu, 0xDEu, 0xEEu, 0x11u, 0x12u, 0x22u, 0x23u, 0x33u,
    0x34u, 0x44u, 0x55u, 0x56u, 0x66u, 0x67u, 0x77u, 0x78u,
    0x88u, 0x16u, 0x26u, 0x36u, 0x46u, 0xAFu, 0xFFu, 0xDFu,
};

// `night2` from ASM/COLTAB.ASM.
static const uint8 s_night2_depth_pairs[] = {
    0x99u, 0x99u, 0x9Au, 0x9Au, 0xAAu, 0xABu, 0xBBu, 0xBCu,
    0xCCu, 0xCDu, 0xDDu, 0x99u, 0x19u, 0x11u, 0x12u, 0x22u,
    0x23u, 0x33u, 0x99u, 0x59u, 0x55u, 0x56u, 0x66u, 0x67u,
    0x77u, 0x99u, 0x15u, 0x16u, 0x26u, 0x9Fu, 0xAFu, 0xFFu,
};

// `night3` from ASM/COLTAB.ASM.
static const uint8 s_night3_depth_pairs[] = {
    0x99u, 0x99u, 0x99u, 0x99u, 0x9Au, 0x9Au, 0xAAu, 0xABu,
    0xBBu, 0xBCu, 0xCCu, 0x99u, 0x99u, 0x19u, 0x19u, 0x11u,
    0x12u, 0x22u, 0x99u, 0x99u, 0x59u, 0x59u, 0x55u, 0x56u,
    0x66u, 0x99u, 0x99u, 0x15u, 0x16u, 0x99u, 0x15u, 0x9Fu,
};

// `night4` from ASM/COLTAB.ASM (darkest / most distant bank).
static const uint8 s_night4_depth_pairs[] = {
    0x99u, 0x99u, 0x99u, 0x99u, 0x99u, 0x99u, 0x9Au, 0x9Au,
    0xAAu, 0xABu, 0xBBu, 0x99u, 0x99u, 0x99u, 0x19u, 0x19u,
    0x19u, 0x11u, 0x99u, 0x99u, 0x99u, 0x59u, 0x59u, 0x59u,
    0x55u, 0x99u, 0x99u, 0x15u, 0x15u, 0x99u, 0x15u, 0x15u,
};

// COLDEPTH banks brightest→darkest, selected per object by view depth.
static const uint8 *const s_night_depth_banks[] = {
    s_night1_depth_pairs,
    s_night2_depth_pairs,
    s_night3_depth_pairs,
    s_night4_depth_pairs,
};

// Per-level depth-band threshold records, transcribed from the `def_depthz`
// entries in ASM/COLTABS.ASM:1488-1491 (`depthtables`). Compared against the
// object's view-space Z (`bigz`); the hardware keeps only the high byte, so
// values are multiples of 256 by construction where the ASM used hex.
static const float s_depthz_tables[SHAPES_DEPTHZ_COUNT][3] = {
    [SHAPES_DEPTHZ_NORMAL] = { 2560.0f, 3328.0f, 3840.0f },   // $a00/$d00/$f00
    [SHAPES_DEPTHZ_TUNNEL] = { 500.0f, 750.0f, 1000.0f },
    [SHAPES_DEPTHZ_MIST]   = { 500.0f, 750.0f, 1000.0f },
    [SHAPES_DEPTHZ_STAGE1] = { 2560.0f, 3328.0f, 16128.0f },  // $a00/$d00/$3f00
};

// Active threshold record, selected per background segment like the
// `set_zdepthtable` macro (BGMACS.INC:611).
static uint8 s_depthz_table = SHAPES_DEPTHZ_NORMAL;

// COLDEPTH bank for the object currently being rendered; refreshed at the top
// of Shapes_Render (per object, not per face).
static uint8 s_current_depth_bank = 0;

// First 16 palette entries from NIGHT.COL, stored as SNES BGR555 values.
static const uint16 s_night_palette[] = {
    0x0000u, 0x0451u, 0x153Au, 0x22BDu,
    0x377Fu, 0x5445u, 0x6D2Bu, 0x7E8Fu,
    0x7FB6u, 0x0CA3u, 0x2588u, 0x424Eu,
    0x56F5u, 0x6B9Au, 0x7FFEu, 0x0300u,
};

static const float s_debug_material_color[4] = {
    1.0f, 0.0f, 1.0f, 1.0f,
};

static const uint16 s_coltab_id0[] = {
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(2u, 2u), MATERIAL_COLLITE(3u, 3u),
    MATERIAL_COLLITE(4u, 4u), MATERIAL_COLLITE(5u, 5u),
    MATERIAL_COLLITE(6u, 6u), MATERIAL_COLLITE(7u, 7u),
    MATERIAL_COLLITE(8u, 8u), MATERIAL_COLLITE(9u, 9u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(7u),
    MATERIAL_COLDEPTH(8u),    MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(11u),
    MATERIAL_COLDEPTH(12u),   MATERIAL_COLDEPTH(13u),
    MATERIAL_COLDEPTH(14u),   MATERIAL_COLDEPTH(15u),
    MATERIAL_COLDEPTH(16u),   MATERIAL_COLDEPTH(17u),
    MATERIAL_COLDEPTH(18u),   MATERIAL_COLDEPTH(19u),
    MATERIAL_COLDEPTH(20u),   MATERIAL_COLDEPTH(21u),
    MATERIAL_COLDEPTH(22u),   MATERIAL_COLDEPTH(23u),
    MATERIAL_COLDEPTH(24u),   MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
    // FX48-FX70 (COLTABS.ASM ID_0_C): sprite textures and animation tables
    // that are not ported yet. Keep the rows so higher FX indices stay
    // aligned; MATERIAL_COLTEXT flows through the unsupported-material path.
    MATERIAL_COLTEXT(0u),  // FX48 asteroid1_spr
    MATERIAL_COLTEXT(1u),  // FX49 panel3_spr
    MATERIAL_COLTEXT(1u),  // FX50 panel3_spr
    MATERIAL_COLTEXT(1u),  // FX51 panel3_spr
    MATERIAL_COLTEXT(2u),  // FX52 gateflash_a1 (anim, unported)
    MATERIAL_COLTEXT(3u),  // FX53 bosshitcol1_a1
    MATERIAL_COLTEXT(4u),  // FX54 bosshitcol2_a1
    MATERIAL_COLTEXT(5u),  // FX55 pwireframe_a1
    MATERIAL_COLTEXT(6u),  // FX56 bosshitcol3_a1
    MATERIAL_COLTEXT(7u),  // FX57 face1_spr
    MATERIAL_COLTEXT(8u),  // FX58 roadflash_a1
    MATERIAL_COLTEXT(9u),  // FX59 biker1_spr
    MATERIAL_COLTEXT(10u), // FX60 biker1_spr,8
    MATERIAL_COLTEXT(11u), // FX61 ring_a1
    MATERIAL_COLTEXT(12u), // FX62 ring_a2
    MATERIAL_COLTEXT(13u), // FX63 ring_a3
    MATERIAL_COLTEXT(14u), // FX64 ring_a4
    MATERIAL_COLTEXT(15u), // FX65 ring_a5
    MATERIAL_COLTEXT(16u), // FX66 ca_1d
    MATERIAL_COLTEXT(17u), // FX67 scrapmetal_spr
    MATERIAL_COLTEXT(18u), // FX68 cockpit1_spr
    MATERIAL_COLTEXT(19u), // FX69 cockpit2_spr
    MATERIAL_COLTEXT(20u), // FX70 monkey1_spr
    // FX71-FX95: solid/dithered COLNORM ramps (COLTABS.ASM ID_0_C).
    MATERIAL_COLNORM(0x9u, 0x9u), MATERIAL_COLNORM(0x9u, 0xAu),  // FX71-72
    MATERIAL_COLNORM(0xAu, 0xAu), MATERIAL_COLNORM(0xAu, 0xBu),  // FX73-74
    MATERIAL_COLNORM(0xBu, 0xBu), MATERIAL_COLNORM(0xBu, 0xCu),  // FX75-76
    MATERIAL_COLNORM(0xCu, 0xCu), MATERIAL_COLNORM(0xCu, 0xDu),  // FX77-78
    MATERIAL_COLNORM(0xDu, 0xDu), MATERIAL_COLNORM(0xDu, 0xEu),  // FX79-80
    MATERIAL_COLNORM(0xEu, 0xEu), MATERIAL_COLNORM(0x1u, 0x1u),  // FX81-82
    MATERIAL_COLNORM(0x1u, 0x2u), MATERIAL_COLNORM(0x2u, 0x2u),  // FX83-84
    MATERIAL_COLNORM(0x2u, 0x3u), MATERIAL_COLNORM(0x3u, 0x3u),  // FX85-86
    MATERIAL_COLNORM(0x3u, 0x4u), MATERIAL_COLNORM(0x4u, 0x4u),  // FX87-88
    MATERIAL_COLNORM(0x5u, 0x5u), MATERIAL_COLNORM(0x5u, 0x6u),  // FX89-90
    MATERIAL_COLNORM(0x6u, 0x6u), MATERIAL_COLNORM(0x6u, 0x7u),  // FX91-92
    MATERIAL_COLNORM(0x7u, 0x7u), MATERIAL_COLNORM(0x7u, 0x8u),  // FX93-94
    MATERIAL_COLNORM(0x8u, 0x8u),                                // FX95
    // FX96-FX108: more unported sprite/anim entries.
    MATERIAL_COLTEXT(21u), // FX96 byebye_spr,7
    MATERIAL_COLTEXT(22u), // FX97 spacemistWhite_a1
    MATERIAL_COLTEXT(23u), // FX98 spacemistRed_a1
    MATERIAL_COLTEXT(24u), // FX99 spacemistLgrey_a1
    MATERIAL_COLTEXT(25u), // FX100 chicken_spr
    MATERIAL_COLTEXT(26u), // FX101 brakelight_a1
    MATERIAL_COLTEXT(25u), // FX102 chicken_spr
    MATERIAL_COLTEXT(7u),  // FX103 face1_spr
    MATERIAL_COLTEXT(27u), // FX104 playerbeam_a1
    MATERIAL_COLTEXT(28u), // FX105 largeexpl4_spr,6
    MATERIAL_COLTEXT(29u), // FX106 starwars1_spr
    MATERIAL_COLTEXT(30u), // FX107 starwars2_spr
    MATERIAL_COLTEXT(31u), // FX108 tunnelwall_spr
};

static const uint16 s_coltab_id1[] = {
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(3u, 3u), MATERIAL_COLLITE(2u, 2u),
    MATERIAL_COLLITE(5u, 5u), MATERIAL_COLLITE(4u, 4u),
    MATERIAL_COLLITE(7u, 7u), MATERIAL_COLLITE(6u, 6u),
    MATERIAL_COLLITE(9u, 9u), MATERIAL_COLLITE(8u, 8u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(7u),
    MATERIAL_COLDEPTH(8u),    MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(18u),
    MATERIAL_COLDEPTH(19u),   MATERIAL_COLDEPTH(20u),
    MATERIAL_COLDEPTH(21u),   MATERIAL_COLDEPTH(22u),
    MATERIAL_COLDEPTH(23u),   MATERIAL_COLDEPTH(24u),
    MATERIAL_COLDEPTH(11u),   MATERIAL_COLDEPTH(12u),
    MATERIAL_COLDEPTH(13u),   MATERIAL_COLDEPTH(14u),
    MATERIAL_COLDEPTH(15u),   MATERIAL_COLDEPTH(16u),
    MATERIAL_COLDEPTH(17u),   MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
};

static const uint16 s_coltab_id2[] = {
    MATERIAL_COLLITE(2u, 2u), MATERIAL_COLLITE(6u, 6u),
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(3u, 3u),
    MATERIAL_COLLITE(4u, 4u), MATERIAL_COLLITE(5u, 5u),
    MATERIAL_COLLITE(1u, 1u), MATERIAL_COLLITE(7u, 7u),
    MATERIAL_COLLITE(8u, 8u), MATERIAL_COLLITE(9u, 9u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(11u),   MATERIAL_COLDEPTH(12u),
    MATERIAL_COLDEPTH(13u),   MATERIAL_COLDEPTH(14u),
    MATERIAL_COLDEPTH(15u),   MATERIAL_COLDEPTH(16u),
    MATERIAL_COLDEPTH(17u),   MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(2u),
    MATERIAL_COLDEPTH(3u),    MATERIAL_COLDEPTH(4u),
    MATERIAL_COLDEPTH(5u),    MATERIAL_COLDEPTH(6u),
    MATERIAL_COLDEPTH(7u),    MATERIAL_COLDEPTH(8u),
    MATERIAL_COLDEPTH(18u),   MATERIAL_COLDEPTH(19u),
    MATERIAL_COLDEPTH(20u),   MATERIAL_COLDEPTH(21u),
    MATERIAL_COLDEPTH(22u),   MATERIAL_COLDEPTH(23u),
    MATERIAL_COLDEPTH(24u),   MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
};

static const uint16 s_coltab_id3[] = {
    MATERIAL_COLLITE(3u, 3u), MATERIAL_COLLITE(7u, 7u),
    MATERIAL_COLLITE(2u, 2u), MATERIAL_COLLITE(0u, 0u),
    MATERIAL_COLLITE(4u, 4u), MATERIAL_COLLITE(5u, 5u),
    MATERIAL_COLLITE(6u, 6u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(8u, 8u), MATERIAL_COLLITE(9u, 9u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(18u),   MATERIAL_COLDEPTH(19u),
    MATERIAL_COLDEPTH(20u),   MATERIAL_COLDEPTH(21u),
    MATERIAL_COLDEPTH(22u),   MATERIAL_COLDEPTH(23u),
    MATERIAL_COLDEPTH(24u),   MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(11u),
    MATERIAL_COLDEPTH(12u),   MATERIAL_COLDEPTH(13u),
    MATERIAL_COLDEPTH(14u),   MATERIAL_COLDEPTH(15u),
    MATERIAL_COLDEPTH(16u),   MATERIAL_COLDEPTH(17u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(7u),
    MATERIAL_COLDEPTH(8u),    MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
};

static const uint16 s_coltab_id4[] = {
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(3u, 3u),
    MATERIAL_COLLITE(4u, 4u), MATERIAL_COLLITE(5u, 5u),
    MATERIAL_COLLITE(1u, 1u), MATERIAL_COLLITE(7u, 7u),
    MATERIAL_COLLITE(8u, 8u), MATERIAL_COLLITE(9u, 9u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(7u),
    MATERIAL_COLDEPTH(8u),    MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(0u),
    MATERIAL_COLDEPTH(1u),    MATERIAL_COLDEPTH(2u),
    MATERIAL_COLDEPTH(3u),    MATERIAL_COLDEPTH(4u),
    MATERIAL_COLDEPTH(5u),    MATERIAL_COLDEPTH(6u),
    MATERIAL_COLDEPTH(18u),   MATERIAL_COLDEPTH(19u),
    MATERIAL_COLDEPTH(20u),   MATERIAL_COLDEPTH(21u),
    MATERIAL_COLDEPTH(22u),   MATERIAL_COLDEPTH(23u),
    MATERIAL_COLDEPTH(24u),   MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
};

static const uint16 s_coltab_id5[] = {
    MATERIAL_COLLITE(0u, 0u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(2u, 2u), MATERIAL_COLLITE(0u, 0u),
    MATERIAL_COLLITE(4u, 4u), MATERIAL_COLLITE(5u, 5u),
    MATERIAL_COLLITE(6u, 6u), MATERIAL_COLLITE(1u, 1u),
    MATERIAL_COLLITE(8u, 8u), MATERIAL_COLLITE(9u, 9u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(7u),
    MATERIAL_COLDEPTH(8u),    MATERIAL_COLDEPTH(9u),
    MATERIAL_COLDEPTH(10u),   MATERIAL_COLDEPTH(11u),
    MATERIAL_COLDEPTH(12u),   MATERIAL_COLDEPTH(13u),
    MATERIAL_COLDEPTH(14u),   MATERIAL_COLDEPTH(15u),
    MATERIAL_COLDEPTH(16u),   MATERIAL_COLDEPTH(17u),
    MATERIAL_COLDEPTH(0u),    MATERIAL_COLDEPTH(1u),
    MATERIAL_COLDEPTH(2u),    MATERIAL_COLDEPTH(3u),
    MATERIAL_COLDEPTH(4u),    MATERIAL_COLDEPTH(5u),
    MATERIAL_COLDEPTH(6u),    MATERIAL_COLDEPTH(25u),
    MATERIAL_COLDEPTH(26u),   MATERIAL_COLDEPTH(27u),
    MATERIAL_COLDEPTH(28u),   MATERIAL_COLDEPTH(29u),
    MATERIAL_COLDEPTH(30u),   MATERIAL_COLDEPTH(31u),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_0),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_2),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_1),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_3),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_4),
    MATERIAL_COLANIM(SHAPE_ANIM_CA_5),
};

static const ShapeColorTable s_color_tables[] = {
    [SHAPE_COLTAB_ID_0] = { s_coltab_id0, ARRAY_SIZE(s_coltab_id0), "id_0_c" },
    [SHAPE_COLTAB_ID_1] = { s_coltab_id1, ARRAY_SIZE(s_coltab_id1), "id_1_c" },
    [SHAPE_COLTAB_ID_2] = { s_coltab_id2, ARRAY_SIZE(s_coltab_id2), "id_2_c" },
    [SHAPE_COLTAB_ID_3] = { s_coltab_id3, ARRAY_SIZE(s_coltab_id3), "id_3_c" },
    [SHAPE_COLTAB_ID_4] = { s_coltab_id4, ARRAY_SIZE(s_coltab_id4), "id_4_c" },
    [SHAPE_COLTAB_ID_5] = { s_coltab_id5, ARRAY_SIZE(s_coltab_id5), "id_5_c" },
};

static uint16 s_last_unknown_coltab = 0xFFFFu;
static uint16 s_last_unknown_anim = 0xFFFFu;
static uint16 s_last_unsupported_material = 0xFFFFu;
static uint16 s_last_missing_face_color = 0xFFFFu;

uint16 Shapes_ResolveShapeWord(uint16 shape_id) {
    switch (shape_id) {
    case RAW_SHAPE_BOSS_7_1:
        return SHAPE_BOSS7_1;
    case 278u:
        return SHAPE_ALIAS_MOTHER1;
    case 551u:
        return SHAPE_ALIAS_OP_0;
    case 552u:
        return SHAPE_ALIAS_OP_1;
    case 553u:
        return SHAPE_ALIAS_OP_2;
    case 557u:                       // levels.c SH_MY_BIRD raw word
        return SHAPE_EXT_MY_BIRD;
    case RAW_SHAPE_IMYSHIP_4:
        return SHAPE_MYSHIP_4;
    default:
        return shape_id;
    }
}

static uint16 Shapes_ResolveBoss7Frame(uint16 shape_id, uint8 anim_frame) {
    switch (shape_id) {
    case SHAPE_BOSS7_0:
        if (anim_frame == 0u) return SHAPE_BOSS7_0;
        if (anim_frame > 8u) anim_frame = 8u;
        return (uint16)(SHAPE_INTERNAL_BOSS7_0_FRAME1 + (anim_frame - 1u));
    case SHAPE_BOSS7_3:
        if (anim_frame == 0u) return SHAPE_BOSS7_3;
        if (anim_frame > 9u) anim_frame = 9u;
        return (uint16)(SHAPE_INTERNAL_BOSS7_3_FRAME1 + (anim_frame - 1u));
    case SHAPE_BOSS7_4:
        if (anim_frame == 0u) return SHAPE_BOSS7_4;
        if (anim_frame > 9u) anim_frame = 9u;
        return (uint16)(SHAPE_INTERNAL_BOSS7_4_FRAME1 + (anim_frame - 1u));
    default:
        return shape_id;
    }
}

static uint16 Shapes_ResolveBaseColorShape(uint16 shape_id) {
    if (shape_id >= SHAPE_INTERNAL_BOSS7_0_FRAME1 &&
        shape_id < (uint16)(SHAPE_INTERNAL_BOSS7_0_FRAME1 +
                            SHAPE_INTERNAL_BOSS7_0_FRAME_COUNT)) {
        return SHAPE_BOSS7_0;
    }
    if (shape_id >= SHAPE_INTERNAL_BOSS7_3_FRAME1 &&
        shape_id < (uint16)(SHAPE_INTERNAL_BOSS7_3_FRAME1 +
                            SHAPE_INTERNAL_BOSS7_3_FRAME_COUNT)) {
        return SHAPE_BOSS7_3;
    }
    if (shape_id >= SHAPE_INTERNAL_BOSS7_4_FRAME1 &&
        shape_id < (uint16)(SHAPE_INTERNAL_BOSS7_4_FRAME1 +
                            SHAPE_INTERNAL_BOSS7_4_FRAME_COUNT)) {
        return SHAPE_BOSS7_4;
    }
    return shape_id;
}

static bool Shapes_IsBoss7Shape(uint16 shape_id) {
    switch (Shapes_ResolveBaseColorShape(shape_id)) {
    case SHAPE_BOSS7_0:
    case SHAPE_BOSS7_1:
    case SHAPE_BOSS7_1O:
    case SHAPE_BOSS7_2:
    case SHAPE_BOSS7_3:
    case SHAPE_BOSS7_4:
        return true;
    default:
        return false;
    }
}

static uint8 Shapes_ResolveColorTableId(uint16 shape_id, uint16 color_table) {
    if (color_table == 0u) {
        return Shapes_IsBoss7Shape(shape_id) ? SHAPE_COLTAB_ID_1
                                             : SHAPE_COLTAB_ID_0;
    }
    if (color_table <= SHAPE_COLTAB_ID_5) {
        return (uint8)color_table;
    }
    if (s_last_unknown_coltab != color_table) {
        s_last_unknown_coltab = color_table;
        fprintf(stderr, "Shapes: unsupported color table 0x%04X for shape %u\n",
                (unsigned)color_table, (unsigned)shape_id);
    }
    return SHAPE_COLTAB_ID_INVALID;
}

static const ShapeColorTable *Shapes_GetColorTable(uint8 color_table_id) {
    if (color_table_id >= ARRAY_SIZE(s_color_tables) ||
        !s_color_tables[color_table_id].entries) {
        return NULL;
    }
    return &s_color_tables[color_table_id];
}

static const ShapeAnimTable *Shapes_GetAnimTable(uint16 anim_id) {
    if (anim_id >= ARRAY_SIZE(s_anim_tables) || !s_anim_tables[anim_id].frames) {
        if (s_last_unknown_anim != anim_id) {
            s_last_unknown_anim = anim_id;
            fprintf(stderr, "Shapes: unsupported animated material table %u\n",
                    (unsigned)anim_id);
        }
        return NULL;
    }
    return &s_anim_tables[anim_id];
}

static void Shapes_DecodeBgr555(uint16 color, float *r, float *g, float *b) {
    *r = (float)(color & 0x1Fu) / 31.0f;
    *g = (float)((color >> 5) & 0x1Fu) / 31.0f;
    *b = (float)((color >> 10) & 0x1Fu) / 31.0f;
}

// Hardware checkerboard-dithers the two nibble colors of a pair; this port
// averages them instead. All nibble-pair consumers (COLNORM, COLDEPTH,
// COLLITE shades) funnel through here so a dithered decode can be swapped in
// later without touching the material paths.
static void Shapes_DecodePalettePair(uint8 pair, float out[4]) {
    float r0, g0, b0;
    float r1, g1, b1;
    uint8 lo = (uint8)(pair & 0x0Fu);
    uint8 hi = (uint8)((pair >> 4) & 0x0Fu);

    Shapes_DecodeBgr555(s_night_palette[lo], &r0, &g0, &b0);
    Shapes_DecodeBgr555(s_night_palette[hi], &r1, &g1, &b1);
    out[0] = (r0 + r1) * 0.5f;
    out[1] = (g0 + g1) * 0.5f;
    out[2] = (b0 + b1) * 0.5f;
    out[3] = 1.0f;
}

static void Shapes_CopyColor(const float src[4], float out[4]) {
    out[0] = src[0];
    out[1] = src[1];
    out[2] = src[2];
    out[3] = src[3];
}

static void Shapes_SetUnsupportedMaterialColor(uint16 material_word,
                                               const char *reason,
                                               float out[4]) {
    if (s_last_unsupported_material != material_word) {
        s_last_unsupported_material = material_word;
        fprintf(stderr, "Shapes: unsupported material 0x%04X (%s)\n",
                (unsigned)material_word, reason);
    }
    Shapes_CopyColor(s_debug_material_color, out);
}

static void Shapes_ResolveMaterialColor(uint16 material_word, uint8 col_frame,
                                        int shade_index, float out[4]) {
    uint8 source = (uint8)(material_word >> 8);

    if ((material_word & 0x8000u) != 0u) {
        const ShapeAnimTable *anim_table;
        uint8 frame_index;

        if ((material_word & 0x4000u) != 0u) {
            Shapes_SetUnsupportedMaterialColor(material_word, "COLSMOOTH", out);
            return;
        }

        anim_table = Shapes_GetAnimTable((uint16)(material_word & 0x3FFFu));
        if (!anim_table || anim_table->frame_count == 0u) {
            Shapes_CopyColor(s_debug_material_color, out);
            return;
        }

        // Super FX masks with `(frame_count - 1)` rather than clamping.
        frame_index = (uint8)(col_frame & (uint8)(anim_table->frame_count - 1u));
        Shapes_ResolveMaterialColor(anim_table->frames[frame_index], col_frame,
                                    shade_index, out);
        return;
    }

    if ((material_word & 0x4000u) != 0u) {
        Shapes_SetUnsupportedMaterialColor(material_word, "COLTEXT", out);
        return;
    }

    if (source == 63u) {
        Shapes_DecodePalettePair((uint8)(material_word & 0xFFu), out);
        return;
    }

    if (source == 62u) {
        uint8 depth_index = (uint8)(material_word & 0xFFu);
        const uint8 *bank = s_night_depth_banks[s_current_depth_bank];
        if (depth_index < ARRAY_SIZE(s_night1_depth_pairs)) {
            Shapes_DecodePalettePair(bank[depth_index], out);
            return;
        }
        Shapes_SetUnsupportedMaterialColor(material_word, "COLDEPTH", out);
        return;
    }

    if (source < 12u) {
        // The viewer aliases light sources 10/11 onto row 9; LIGHT.ASM only
        // defines shadesG_0..shadesG_9.
        uint8 row = (source > (uint8)(SHADE_SUBTABLES - 1u))
                        ? (uint8)(SHADE_SUBTABLES - 1u)
                        : source;
        if (shade_index < 0) shade_index = 0;
        if (shade_index > SHADE_TABLE_LEN - 1) shade_index = SHADE_TABLE_LEN - 1;
        // The shade group is the object's depth band: MOBJ.MC:472-503 selects
        // m_shadestab (shadestab2_0..3) in the same branch that picks the
        // COLDEPTH bank, so distant objects wash out together.
        Shapes_DecodePalettePair(
            g_shade_tables[s_current_depth_bank][row][shade_index], out);
        return;
    }

    Shapes_SetUnsupportedMaterialColor(material_word, "COLLITE", out);
}

static void Shapes_ResolveFaceColor(uint16 shape_id, uint8 face_color_index,
                                    uint8 col_frame, uint16 color_table,
                                    int shade_index, float out[4]) {
    uint8 color_table_id = Shapes_ResolveColorTableId(shape_id, color_table);
    const ShapeColorTable *table = Shapes_GetColorTable(color_table_id);
    uint16 missing_key = (uint16)(((uint16)color_table_id << 8) | face_color_index);

    if (!table) {
        Shapes_CopyColor(s_debug_material_color, out);
        return;
    }

    if (face_color_index >= table->count) {
        if (s_last_missing_face_color != missing_key) {
            s_last_missing_face_color = missing_key;
            fprintf(stderr,
                    "Shapes: face color index %u exceeds %s for shape %u\n",
                    (unsigned)face_color_index, table->name,
                    (unsigned)shape_id);
        }
        Shapes_CopyColor(s_debug_material_color, out);
        return;
    }

    Shapes_ResolveMaterialColor(table->entries[face_color_index], col_frame,
                                shade_index, out);
}

void Shapes_SetDepthTable(uint8 table) {
    if (table >= (uint8)SHAPES_DEPTHZ_COUNT) table = SHAPES_DEPTHZ_NORMAL;
    s_depthz_table = table;
}

// Rotate the fixed world light into object space, mirroring the per-object
// m_rotlightx/y/z computation (MOBJ.MC:905-922). The model matrix is pure
// rotation+translation (Transform_BuildModelMatrix), so the transpose of its
// upper 3x3 is the inverse rotation.
static void Shapes_ComputeObjectLight(const float *model_matrix,
                                      float out[3]) {
    if (!model_matrix) {
        out[0] = s_light_dir[0];
        out[1] = s_light_dir[1];
        out[2] = s_light_dir[2];
        return;
    }
    for (int i = 0; i < 3; i++) {
        out[i] = (model_matrix[i * 4 + 0] * s_light_dir[0]) +
                 (model_matrix[i * 4 + 1] * s_light_dir[1]) +
                 (model_matrix[i * 4 + 2] * s_light_dir[2]);
    }
}

// Face-normal · rotated-light mapped onto the 10 shade levels with the GSU's
// asymmetric intensity curve (MOBJ.MC:4092-4128): the fixed-point dot works
// out to ~dot*15.75, clamped to [6,15], minus 6. Faces angled more than
// ~67 degrees from the light all land on shade 0.
static int Shapes_ComputeShadeIndex(const float *normal,
                                    const float light_obj[3]) {
    float len_sq = (normal[0] * normal[0]) + (normal[1] * normal[1]) +
                   (normal[2] * normal[2]);
    float dot;
    int shade;

    // Degenerate faces carry a zero normal; treat them as fully lit like the
    // viewer does for zero-length normals.
    if (len_sq <= 0.0001f) {
        return SHADE_TABLE_LEN - 1;
    }

    dot = (normal[0] * light_obj[0]) + (normal[1] * light_obj[1]) +
          (normal[2] * light_obj[2]);
    shade = (int)floorf(dot * 15.75f);
    if (shade < 6) shade = 6;
    if (shade > 15) shade = 15;
    return shade - 6;
}

// Pick the COLDEPTH bank (night1..night4) from the object's view-space depth.
static uint8 Shapes_SelectDepthBank(const float *model_matrix) {
    const float *view;
    float x, y, z, depth;
    uint8 bank = 0;

    if (!model_matrix) {
        return 0;
    }

    view = Transform_GetView();
    x = model_matrix[12];
    y = model_matrix[13];
    z = model_matrix[14];
    // Camera looks down -Z in view space, so depth is the negated view Z.
    // This matches the hardware's per-object `bigz` compare (MOBJ.MC:442-506):
    // bigz < t1 -> band 0, else stepping to darker banks.
    // TODO: per-object depthoffset override (al_depthoffset -> dl_depth ->
    // m_depthoffset, MOBJ.MC:447-460) pins an object to a fixed band 1-4;
    // wire it once the draw list carries the field.
    depth = -((view[2] * x) + (view[6] * y) + (view[10] * z) + view[14]);

    while (bank < 3u && depth >= s_depthz_tables[s_depthz_table][bank]) {
        ++bank;
    }
    return bank;
}

void Shapes_Init(void) {
    memset(s_shapes, 0, sizeof(s_shapes));
    s_shape_count = 0;
    s_depthz_table = SHAPES_DEPTHZ_NORMAL;
    s_current_depth_bank = 0;
    s_last_unknown_coltab = 0xFFFFu;
    s_last_unknown_anim = 0xFFFFu;
    s_last_unsupported_material = 0xFFFFu;
    s_last_missing_face_color = 0xFFFFu;
}

void Shapes_Shutdown(void) {
    for (int i = 0; i < MAX_SHAPES; i++) {
        Shape *s = &s_shapes[i];
        if (s->gpu_ready) {
            glDeleteVertexArrays(1, &s->vao);
            glDeleteBuffers(1, &s->vbo);
        }
        free(s->vertices);
        free(s->faces);
        free(s->face_normals);
        free(s->face_tri_start);
        free(s->face_tri_count);
        free(s->face_line_first);
        memset(s, 0, sizeof(Shape));
    }
}

static bool BuildFaceNormals(Shape *shape) {
    float centroid_x = 0.0f;
    float centroid_y = 0.0f;
    float centroid_z = 0.0f;

    shape->face_normals = malloc(sizeof(float) * shape->num_faces * 3);
    if (!shape->face_normals) {
        return false;
    }

    for (int i = 0; i < shape->num_vertices; i++) {
        centroid_x += shape->vertices[i].x;
        centroid_y += shape->vertices[i].y;
        centroid_z += shape->vertices[i].z;
    }
    if (shape->num_vertices > 0) {
        float inv_vertex_count = 1.0f / (float)shape->num_vertices;
        centroid_x *= inv_vertex_count;
        centroid_y *= inv_vertex_count;
        centroid_z *= inv_vertex_count;
    }

    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];
        float *normal = &shape->face_normals[i * 3];

        normal[0] = 0.0f;
        normal[1] = 0.0f;
        normal[2] = 0.0f;

        if (face->num_verts < 3) {
            continue;
        }

        if (face->vertex_indices[0] >= shape->num_vertices ||
            face->vertex_indices[1] >= shape->num_vertices ||
            face->vertex_indices[2] >= shape->num_vertices) {
            continue;
        }

        const ShapeVertex *v0 = &shape->vertices[face->vertex_indices[0]];
        const ShapeVertex *v1 = &shape->vertices[face->vertex_indices[1]];
        const ShapeVertex *v2 = &shape->vertices[face->vertex_indices[2]];
        float face_center_x;
        float face_center_y;
        float face_center_z;
        float ux = v1->x - v0->x;
        float uy = v1->y - v0->y;
        float uz = v1->z - v0->z;
        float vx = v2->x - v0->x;
        float vy = v2->y - v0->y;
        float vz = v2->z - v0->z;
        float nx = (uy * vz) - (uz * vy);
        float ny = (uz * vx) - (ux * vz);
        float nz = (ux * vy) - (uy * vx);
        float length = sqrtf((nx * nx) + (ny * ny) + (nz * nz));

        if (length <= 0.0001f) {
            continue;
        }

        normal[0] = nx / length;
        normal[1] = ny / length;
        normal[2] = nz / length;

        face_center_x = (v0->x + v1->x + v2->x) / 3.0f;
        face_center_y = (v0->y + v1->y + v2->y) / 3.0f;
        face_center_z = (v0->z + v1->z + v2->z) / 3.0f;
        if (((face_center_x - centroid_x) * normal[0]) +
            ((face_center_y - centroid_y) * normal[1]) +
            ((face_center_z - centroid_z) * normal[2]) < 0.0f) {
            normal[0] = -normal[0];
            normal[1] = -normal[1];
            normal[2] = -normal[2];
        }
    }

    return true;
}

// Fan-triangulate all faces and upload expanded vertex data to GPU
static bool UploadShape(Shape *shape) {
    if (!shape->faces || !shape->vertices) return false;

    // Count total triangles (fan triangulation: n-gon → n-2 triangles)
    int total_tris = 0;
    shape->face_tri_start = malloc(sizeof(int) * shape->num_faces);
    shape->face_tri_count = malloc(sizeof(int) * shape->num_faces);
    if (!shape->face_tri_start || !shape->face_tri_count) return false;

    for (int i = 0; i < shape->num_faces; i++) {
        shape->face_tri_start[i] = total_tris;
        int ntris = shape->faces[i].num_verts - 2;
        if (ntris < 1) ntris = 0;
        shape->face_tri_count[i] = ntris;
        total_tris += ntris;
    }
    shape->num_triangles = total_tris;

    // Wireframe (Face2) segments are appended to the same VBO after the
    // triangle region; face_line_first[i] records each line face's first
    // vertex index for glDrawArrays(GL_LINES, ...).
    int line_verts = 0;
    shape->face_line_first = malloc(sizeof(int) * shape->num_faces);
    if (!shape->face_line_first) return false;
    for (int i = 0; i < shape->num_faces; i++) {
        if (shape->faces[i].num_verts == 2) {
            shape->face_line_first[i] = (total_tris * 3) + line_verts;
            line_verts += 2;
        } else {
            shape->face_line_first[i] = -1;
        }
    }
    shape->num_line_verts = line_verts;

    // Expand into position array (3 floats per vertex, 3 vertices per tri)
    float *positions = malloc(sizeof(float) *
                              ((size_t)total_tris * 3 + line_verts) * 3);
    if (!positions) return false;

    int idx = 0;
    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];
        if (face->num_verts < 3) continue;

        // Fan triangulation: vertex 0 is the hub
        int v0 = face->vertex_indices[0];
        for (int t = 0; t < shape->face_tri_count[i]; t++) {
            int v1 = face->vertex_indices[t + 1];
            int v2 = face->vertex_indices[t + 2];

            // Bounds check
            if (v0 >= shape->num_vertices || v1 >= shape->num_vertices || v2 >= shape->num_vertices) {
                // Skip invalid face
                positions[idx++] = 0; positions[idx++] = 0; positions[idx++] = 0;
                positions[idx++] = 0; positions[idx++] = 0; positions[idx++] = 0;
                positions[idx++] = 0; positions[idx++] = 0; positions[idx++] = 0;
                continue;
            }

            positions[idx++] = shape->vertices[v0].x;
            positions[idx++] = shape->vertices[v0].y;
            positions[idx++] = shape->vertices[v0].z;

            positions[idx++] = shape->vertices[v1].x;
            positions[idx++] = shape->vertices[v1].y;
            positions[idx++] = shape->vertices[v1].z;

            positions[idx++] = shape->vertices[v2].x;
            positions[idx++] = shape->vertices[v2].y;
            positions[idx++] = shape->vertices[v2].z;
        }
    }

    // Append Face2 line segments after the triangle region.
    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];
        int lv0, lv1;
        if (face->num_verts != 2) continue;
        lv0 = face->vertex_indices[0];
        lv1 = face->vertex_indices[1];
        if (lv0 >= shape->num_vertices || lv1 >= shape->num_vertices) {
            lv0 = 0;
            lv1 = 0;
        }
        positions[idx++] = shape->vertices[lv0].x;
        positions[idx++] = shape->vertices[lv0].y;
        positions[idx++] = shape->vertices[lv0].z;
        positions[idx++] = shape->vertices[lv1].x;
        positions[idx++] = shape->vertices[lv1].y;
        positions[idx++] = shape->vertices[lv1].z;
    }

    // Upload to GPU
    glGenVertexArrays(1, &shape->vao);
    glGenBuffers(1, &shape->vbo);

    glBindVertexArray(shape->vao);
    glBindBuffer(GL_ARRAY_BUFFER, shape->vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(float) * idx, positions, GL_STATIC_DRAW);

    // Position attribute (location 0)
    glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void *)0);
    glEnableVertexAttribArray(0);

    glBindVertexArray(0);
    free(positions);

    shape->gpu_ready = true;
    return true;
}

bool Shapes_Register(uint16 shape_id, const ShapeVertex *verts, int nverts,
                     const ShapeFace *faces, int nfaces) {
    if (shape_id >= MAX_SHAPES) return false;

    Shape *shape = &s_shapes[shape_id];

    // Free any existing data
    free(shape->vertices);
    free(shape->faces);
    free(shape->face_normals);
    free(shape->face_tri_start);
    free(shape->face_tri_count);
    free(shape->face_line_first);
    if (shape->gpu_ready) {
        glDeleteVertexArrays(1, &shape->vao);
        glDeleteBuffers(1, &shape->vbo);
    }
    memset(shape, 0, sizeof(Shape));

    // Copy vertex data
    shape->vertices = malloc(sizeof(ShapeVertex) * nverts);
    if (!shape->vertices) return false;
    memcpy(shape->vertices, verts, sizeof(ShapeVertex) * nverts);
    shape->num_vertices = nverts;

    // Copy face data
    shape->faces = malloc(sizeof(ShapeFace) * nfaces);
    if (!shape->faces) return false;
    memcpy(shape->faces, faces, sizeof(ShapeFace) * nfaces);
    shape->num_faces = nfaces;

    if (!BuildFaceNormals(shape)) {
        fprintf(stderr, "Shapes: failed to build face normals for shape %d\n", shape_id);
        return false;
    }

    // Upload to GPU
    if (!UploadShape(shape)) {
        fprintf(stderr, "Shapes: failed to upload shape %d\n", shape_id);
        return false;
    }

    if (shape_id >= s_shape_count) s_shape_count = shape_id + 1;
    return true;
}

// ============================================================
// Hardcoded Arwing shape data (from PSHAPES.ASM / myship_4)
// ============================================================

// Vertices: 4 from Pointsb + 12 from PointsXb (6 pairs) = 16 total
// PointsXb generates pairs: (x,y,z) and (-x,y,z)
// NOTE: Y is negated because Star Fox +Y is screen-down, OpenGL +Y is screen-up
static const ShapeVertex arwing_verts[] = {
    // Pointsb 4 (indices 0-3)
    {  0.0f,   0.0f, -10.0f },  // 0: nose
    {  0.0f,  -1.0f,   0.0f },  // 1: cockpit top (was y=1)
    {  0.0f,   6.0f,   0.0f },  // 2: cockpit bottom (was y=-6)
    {  0.0f,  -2.0f,  80.0f },  // 3: tail

    // PointsXb 6 → 12 vertices (indices 4-15)
    {  36.0f, -14.0f, -40.0f },  // 4: right wing tip
    { -36.0f, -14.0f, -40.0f },  // 5: left wing tip (mirror)
    {  20.0f,  11.0f, -20.0f },  // 6: right lower fuselage
    { -20.0f,  11.0f, -20.0f },  // 7: left lower fuselage (mirror)
    { -18.0f,  -9.0f,  -6.0f },  // 8: left upper body
    {  18.0f,  -9.0f,  -6.0f },  // 9: right upper body (mirror)
    { -10.0f,  -4.0f,   0.0f },  // 10: left side
    {  10.0f,  -4.0f,   0.0f },  // 11: right side (mirror)
    {  20.0f,  -4.0f,   0.0f },  // 12: right body
    { -20.0f,  -4.0f,   0.0f },  // 13: left body (mirror)
    { -14.0f, -10.0f,  20.0f },  // 14: left wing rear
    {  14.0f, -10.0f,  20.0f },  // 15: right wing rear (mirror)
};

// Faces: 20 triangles from BSP groups f1,f3,f5,f6,f7,f9,f10
// Face3 macro args: color_idx, vis_idx, nx, ny, nz, v0, v1, v2 (same order
// tools/shape_compiler.py parses; the Viz records carry the matching normal
// and vertex indices). Color ids index id_0_c: 0-9 COLLITE greys/colors,
// 10+2n COLDEPTH, 43/44 COLANIM CA_1 (jet fire) / CA_2 (canopy blue cycle).
static const ShapeFace arwing_faces[] = {
    // f1
    { .vertex_indices = {3, 2, 11},   .num_verts = 3, .color_index = 16 },
    // f3
    { .vertex_indices = {10, 14, 7},  .num_verts = 3, .color_index = 7  },
    // f5
    { .vertex_indices = {10, 13, 5},  .num_verts = 3, .color_index = 0  },
    { .vertex_indices = {8, 13, 10},  .num_verts = 3, .color_index = 44 },
    { .vertex_indices = {13, 8, 5},   .num_verts = 3, .color_index = 0  },
    { .vertex_indices = {8, 10, 5},   .num_verts = 3, .color_index = 1  },
    // f6
    { .vertex_indices = {10, 2, 3},   .num_verts = 3, .color_index = 20 },
    { .vertex_indices = {0, 10, 1},   .num_verts = 3, .color_index = 10 },
    { .vertex_indices = {10, 11, 1},  .num_verts = 3, .color_index = 43 },
    { .vertex_indices = {0, 2, 10},   .num_verts = 3, .color_index = 18 },
    { .vertex_indices = {3, 11, 10},  .num_verts = 3, .color_index = 0  },
    { .vertex_indices = {7, 14, 10},  .num_verts = 3, .color_index = 7  },
    // f7
    { .vertex_indices = {1, 11, 0},   .num_verts = 3, .color_index = 10 },
    // f9
    { .vertex_indices = {6, 15, 11},  .num_verts = 3, .color_index = 7  },
    { .vertex_indices = {11, 15, 6},  .num_verts = 3, .color_index = 7  },
    // f10
    { .vertex_indices = {11, 2, 0},   .num_verts = 3, .color_index = 14 },
    { .vertex_indices = {4, 11, 9},   .num_verts = 3, .color_index = 0  },
    { .vertex_indices = {12, 11, 4},  .num_verts = 3, .color_index = 0  },
    { .vertex_indices = {11, 12, 9},  .num_verts = 3, .color_index = 44 },
    { .vertex_indices = {4, 9, 12},   .num_verts = 3, .color_index = 1  },
};

#define ARWING_NUM_VERTS (sizeof(arwing_verts) / sizeof(arwing_verts[0]))
#define ARWING_NUM_FACES (sizeof(arwing_faces) / sizeof(arwing_faces[0]))

#define SV(x, y, z) { (float)(x), (float)(-(y)), (float)(z) }
#define F3(ci, a, b, c) \
    { .vertex_indices = { (a), (b), (c) }, .num_verts = 3, .color_index = (uint8)(ci) }
#define F4(ci, a, b, c, d) \
    { .vertex_indices = { (a), (b), (c), (d) }, .num_verts = 4, .color_index = (uint8)(ci) }
#define F6(ci, a, b, c, d, e, f) \
    { .vertex_indices = { (a), (b), (c), (d), (e), (f) }, .num_verts = 6, .color_index = (uint8)(ci) }
#define BOSS7_FRAME_BUFFER_VERTS 20

static const ShapeVertex boss7_1_verts[] = {
    SV(-5, 0, -55), SV(5, 0, -55),
    SV(-5, -20, -55), SV(5, -20, -55),
    SV(-3, -2, -52), SV(3, -2, -52),
    SV(3, -18, -52), SV(-3, -18, -52),
    SV(-5, 10, -35), SV(5, 10, -35),
    SV(20, 10, -35), SV(-20, 10, -35),
    SV(-5, -15, -35), SV(5, -15, -35),
    SV(20, -15, -35), SV(-20, -15, -35),
    SV(-5, -30, -35), SV(5, -30, -35),
    SV(23, 5, -30), SV(-23, 5, -30),
    SV(-23, -7, -30), SV(23, -7, -30),
    SV(23, 0, -16), SV(-23, 0, -16),
    SV(5, -20, -15), SV(-5, -20, -15),
    SV(-5, -23, -15), SV(5, -23, -15),
    SV(-9, 4, -5), SV(9, 4, -5),
    SV(-16, 4, -5), SV(16, 4, -5),
    SV(-5, 0, -5), SV(5, 0, -5),
    SV(20, 0, -5), SV(-20, 0, -5),
    SV(-5, 0, 10), SV(5, 0, 10),
    SV(-5, -10, 10), SV(5, -10, 10),
    SV(-3, -2, 15), SV(3, -2, 15),
    SV(3, -8, 15), SV(-3, -8, 15),
};

static const ShapeFace boss7_1_faces[] = {
    F4(11, 38, 36, 32, 12),
    F4(12, 13, 33, 37, 39),
    F4(13, 36, 37, 33, 32),
    F4(14, 13, 39, 38, 12),
    F4(0, 25, 24, 13, 12),
    F4(4, 6, 7, 4, 5),
    F4(16, 24, 25, 26, 27),
    F4(1, 7, 6, 3, 2),
    F4(7, 41, 40, 43, 42),
    F4(15, 38, 39, 42, 43),
    F4(17, 16, 17, 27, 26),
    F4(22, 17, 16, 2, 3),
    F4(43, 9, 8, 32, 33),
    F4(5, 5, 4, 0, 1),
    F4(23, 0, 8, 9, 1),
    F4(33, 40, 41, 37, 36),
    F4(9, 31, 29, 33, 34),
    F4(25, 14, 13, 9, 10),
    F4(19, 13, 24, 27, 17),
    F4(30, 14, 34, 33, 13),
    F3(29, 18, 22, 21),
    F4(30, 34, 14, 21, 22),
    F4(31, 34, 22, 18, 10),
    F4(32, 14, 10, 18, 21),
    F3(36, 10, 31, 34),
    F4(37, 29, 31, 10, 9),
    F3(38, 29, 9, 33),
    F4(3, 1, 3, 6, 5),
    F4(21, 3, 1, 9, 17),
    F4(35, 42, 39, 37, 41),
    F4(6, 35, 32, 28, 30),
    F4(24, 11, 8, 12, 15),
    F4(18, 16, 26, 25, 12),
    F4(28, 12, 32, 35, 15),
    F4(8, 8, 11, 30, 28),
    F3(26, 32, 8, 28),
    F3(27, 35, 30, 11),
    F3(39, 20, 23, 19),
    F4(40, 23, 20, 15, 35),
    F4(41, 11, 19, 23, 35),
    F4(42, 20, 19, 11, 15),
    F4(2, 0, 4, 7, 2),
    F4(20, 16, 8, 0, 2),
    F4(34, 40, 36, 38, 43),
};

static const ShapeVertex boss7_0_verts[] = {
    SV(0, 20, -40), SV(-20, 20, -40), SV(-30, 10, -40), SV(-30, 0, -40),
    SV(0, -20, -40), SV(-20, -20, -40), SV(0, 20, -20), SV(-20, 20, -20),
    SV(-30, 10, -20), SV(-30, 0, -20), SV(0, -20, -20), SV(-20, -20, -20),
    SV(0, 10, 0), SV(-20, 10, 0), SV(0, 0, 0), SV(-20, 0, 0),
    // Frame A0 collapsed as the representative static hatch geometry.
    SV(0, 10, 1), SV(-20, 10, 1), SV(0, 0, 1), SV(-20, 0, 1),
};

static const ShapeVertex boss7_0_frame_verts[][4] = {
    { SV(0, 10, 7), SV(-20, 10, 7), SV(0, 1, 10), SV(-20, 1, 10) },
    { SV(0, 10, 12), SV(-20, 10, 12), SV(0, 3, 18), SV(-20, 3, 18) },
    { SV(0, 10, 17), SV(-20, 10, 17), SV(0, 5, 25), SV(-20, 5, 25) },
    { SV(0, 10, 22), SV(-20, 10, 22), SV(0, 8, 31), SV(-20, 8, 31) },
    { SV(0, 10, 27), SV(-20, 10, 27), SV(0, 11, 36), SV(-20, 11, 36) },
    { SV(0, 10, 32), SV(-20, 10, 32), SV(0, 14, 40), SV(-20, 14, 40) },
    { SV(0, 10, 37), SV(-20, 10, 37), SV(0, 16, 43), SV(-20, 16, 43) },
    { SV(0, 10, 42), SV(-20, 10, 42), SV(0, 16, 48), SV(-20, 16, 48) },
};

static const ShapeFace boss7_0_faces[] = {
    F4(7, 12, 6, 7, 13),
    F6(4, 3, 2, 1, 0, 4, 5),
    F4(5, 5, 4, 10, 11),
    F4(6, 7, 6, 0, 1),
    F4(8, 2, 8, 7, 1),
    F3(9, 13, 7, 8),
    F4(10, 8, 2, 3, 9),
    F4(11, 5, 11, 9, 3),
    F4(12, 15, 11, 10, 14),
    F3(13, 9, 11, 15),
    F4(14, 12, 13, 15, 14),
    F6(15, 14, 10, 4, 0, 6, 12),
    F4(16, 13, 8, 9, 15),
    F4(2, 17, 16, 12, 13),
    F4(3, 16, 17, 19, 18),
    F4(0, 18, 19, 17, 16),
    F4(1, 13, 12, 16, 17),
};

static const ShapeVertex boss7_1o_verts[] = {
    SV(-5, 0, -55), SV(5, 0, -55), SV(-5, -20, -55), SV(5, -20, -55),
    SV(-5, 10, -35), SV(5, 10, -35), SV(-20, 10, -35), SV(20, 10, -35),
    SV(5, -15, -35), SV(-5, -15, -35), SV(-20, -15, -35), SV(20, -15, -35),
    SV(5, -30, -35), SV(-5, -30, -35), SV(-5, -20, -15), SV(5, -20, -15),
    SV(5, -23, -15), SV(-5, -23, -15), SV(-5, 0, -5), SV(5, 0, -5),
    SV(-20, 0, -5), SV(20, 0, -5), SV(-5, 0, 15), SV(5, 0, 15),
    SV(-5, -10, 15), SV(5, -10, 15),
};

static const ShapeFace boss7_1o_faces[] = {
    F4(20, 4, 9, 10, 6),
    F4(8, 25, 24, 9, 8),
    F4(2, 18, 20, 10, 9),
    F3(0, 10, 20, 6),
    F3(1, 7, 21, 11),
    F4(3, 6, 20, 21, 7),
    F4(4, 21, 19, 8, 11),
    F4(6, 19, 23, 25, 8),
    F4(5, 22, 18, 9, 24),
    F4(7, 23, 19, 18, 22),
    F4(9, 23, 22, 24, 25),
    F4(10, 14, 17, 16, 15),
    F4(11, 15, 8, 9, 14),
    F4(12, 12, 16, 17, 13),
    F4(13, 17, 14, 9, 13),
    F4(14, 15, 16, 12, 8),
    F4(21, 8, 5, 7, 11),
    F4(16, 1, 5, 12, 3),
    F4(15, 4, 0, 2, 13),
    F4(17, 13, 2, 3, 12),
    F4(18, 0, 1, 3, 2),
    F4(19, 4, 5, 1, 0),
};

static const ShapeVertex boss7_2_verts[] = {
    SV(0, 20, -40), SV(10, 10, -40), SV(10, 0, -40), SV(0, -20, -40),
    SV(0, 20, -20), SV(10, 10, -20), SV(10, 0, -20), SV(0, -20, -20),
    SV(0, 10, 0), SV(0, 0, 0),
};

static const ShapeFace boss7_2_faces[] = {
    F3(0, 9, 7, 6),
    F4(1, 6, 7, 3, 2),
    F3(2, 5, 4, 8),
    F4(3, 4, 5, 1, 0),
    F4(4, 6, 5, 8, 9),
    F4(5, 2, 1, 5, 6),
    F6(6, 3, 7, 9, 8, 4, 0),
    F4(7, 0, 1, 2, 3),
};

static const ShapeVertex boss7_3_verts[] = {
    SV(10, 5, -45), SV(24, 5, -45), SV(10, -9, -45), SV(0, 15, -40),
    SV(0, 25, -20), SV(6, 9, 0), SV(35, 9, 0), SV(6, -20, 0),
    // Frame A0 collapsed as representative static launcher geometry.
    SV(21, 8, 45), SV(7, -6, 45), SV(6, 9, 1),
};

static const ShapeVertex boss7_3_frame_verts[][3] = {
    { SV(33, -3, 45), SV(18, -18, 45), SV(7, 8, 8) },
    { SV(43, -14, 40), SV(29, -28, 40), SV(10, 5, 14) },
    { SV(52, -22, 30), SV(37, -37, 30), SV(13, 2, 18) },
    { SV(57, -28, 17), SV(43, -42, 17), SV(18, -3, 20) },
    { SV(60, -31, 1), SV(46, -45, 1), SV(24, -9, 21) },
    { SV(58, -29, -14), SV(44, -43, -14), SV(28, -13, 18) },
    { SV(53, -24, -28), SV(39, -38, -28), SV(33, -18, 12) },
    { SV(45, -15, -39), SV(30, -30, -39), SV(35, -20, 7) },
    { SV(34, -5, -45), SV(20, -19, -45), SV(35, -20, -1) },
};

static const ShapeFace boss7_3_faces[] = {
    F4(5, 0, 2, 7, 5),
    F3(8, 5, 7, 6),
    F3(0, 6, 8, 9),
    F3(2, 7, 6, 9),
    F4(10, 7, 2, 1, 6),
    F3(9, 10, 6, 7),
    F3(1, 8, 6, 10),
    F3(3, 10, 7, 9),
    F3(4, 10, 9, 8),
    F4(6, 5, 6, 1, 0),
    F3(7, 0, 1, 2),
};

static const ShapeVertex boss7_4_verts[] = {
    SV(10, 9, -45), SV(10, -5, -45), SV(24, -5, -45), SV(0, -15, -40),
    SV(0, -25, -20), SV(6, 20, 0), SV(6, -9, 0), SV(35, -9, 0),
    // Frame A0 collapsed as representative static launcher geometry.
    SV(7, 6, 45), SV(21, -8, 45), SV(6, -9, 1),
};

static const ShapeVertex boss7_4_frame_verts[][3] = {
    { SV(18, 18, 45), SV(33, 3, 45), SV(7, -8, 8) },
    { SV(29, 28, 40), SV(43, 14, 40), SV(10, -5, 14) },
    { SV(37, 37, 30), SV(52, 22, 30), SV(13, -2, 18) },
    { SV(43, 42, 17), SV(57, 28, 17), SV(18, 3, 20) },
    { SV(46, 45, 1), SV(60, 31, 1), SV(24, 9, 21) },
    { SV(44, 43, -14), SV(58, 29, -14), SV(28, 13, 18) },
    { SV(39, 38, -28), SV(53, 24, -28), SV(33, 18, 12) },
    { SV(30, 30, -39), SV(45, 15, -39), SV(35, 20, 7) },
    { SV(20, 19, -45), SV(34, 5, -45), SV(35, 20, -1) },
};

static const ShapeFace boss7_4_faces[] = {
    F4(5, 1, 2, 7, 6),
    F3(8, 7, 5, 6),
    F3(0, 8, 9, 7),
    F3(2, 8, 7, 5),
    F4(10, 7, 2, 0, 5),
    F3(9, 5, 7, 10),
    F3(1, 10, 7, 9),
    F3(3, 8, 5, 10),
    F3(4, 9, 8, 10),
    F4(7, 6, 5, 0, 1),
    F3(6, 0, 2, 1),
};

static void register_builtin_shape(uint16 shape_id, const char *name,
                                   const ShapeVertex *verts, int nverts,
                                   const ShapeFace *faces, int nfaces);

static void register_shape_frame_variants(uint16 first_shape_id,
                                          const char *name_prefix,
                                          const ShapeVertex *frame0_verts,
                                          int total_verts,
                                          int static_verts,
                                          const ShapeVertex *frame_verts,
                                          int frame_count,
                                          int frame_verts_per_shape,
                                          const ShapeFace *faces,
                                          int nfaces) {
    ShapeVertex verts[BOSS7_FRAME_BUFFER_VERTS];
    char frame_name[32];

    if (total_verts > BOSS7_FRAME_BUFFER_VERTS) {
        fprintf(stderr, "Shapes: frame buffer too small for %s (%d verts)\n",
                name_prefix, total_verts);
        return;
    }

    for (int i = 0; i < frame_count; i++) {
        memcpy(verts, frame0_verts, sizeof(ShapeVertex) * total_verts);
        memcpy(&verts[static_verts],
               frame_verts + (i * frame_verts_per_shape),
               sizeof(ShapeVertex) * frame_verts_per_shape);
        snprintf(frame_name, sizeof(frame_name), "%s[%d]", name_prefix, i + 1);
        register_builtin_shape((uint16)(first_shape_id + i), frame_name,
                               verts, total_verts, faces, nfaces);
    }
}

static void register_builtin_shape(uint16 shape_id, const char *name,
                                   const ShapeVertex *verts, int nverts,
                                   const ShapeFace *faces, int nfaces) {
    if (!Shapes_Register(shape_id, verts, nverts, faces, nfaces)) {
        fprintf(stderr, "Shapes: FAILED to register %s (%u)\n",
                name, (unsigned)shape_id);
    }
}

static void Shapes_LoadAllFromData(void) {
    int loaded = 0;
    for (int i = 0; i < (int)SHAPE_DATA_COUNT; i++) {
        const ShapeDataEntry *entry = &g_shape_data[i];
        if (Shapes_Register(entry->shape_id, entry->vertices,
                            entry->num_vertices, entry->faces,
                            entry->num_faces)) {
            ++loaded;
        } else {
            fprintf(stderr, "Shapes: failed to load %s (ID %u)\n",
                    entry->name, (unsigned)entry->shape_id);
        }
    }
    printf("Shapes: loaded %d/%d shapes from compiled ASM data\n",
           loaded, (int)SHAPE_DATA_COUNT);
}

void Shapes_RegisterBuiltins(void) {
    printf("Shapes: registering built-in shapes...\n");

    // First load all shapes from compiled ASM data
    Shapes_LoadAllFromData();

    // Then register hand-tuned builtins (overwrite ASM versions)

    register_builtin_shape(SHAPE_MYSHIP_4, "myship_4",
                           arwing_verts, ARWING_NUM_VERTS,
                           arwing_faces, ARWING_NUM_FACES);
    register_builtin_shape(SHAPE_BOSS7_1, "boss_7_1",
                           boss7_1_verts, (int)(sizeof(boss7_1_verts) / sizeof(boss7_1_verts[0])),
                           boss7_1_faces, (int)(sizeof(boss7_1_faces) / sizeof(boss7_1_faces[0])));
    register_builtin_shape(SHAPE_BOSS7_0, "boss_7_0",
                           boss7_0_verts, (int)(sizeof(boss7_0_verts) / sizeof(boss7_0_verts[0])),
                           boss7_0_faces, (int)(sizeof(boss7_0_faces) / sizeof(boss7_0_faces[0])));
    register_shape_frame_variants(SHAPE_INTERNAL_BOSS7_0_FRAME1, "boss_7_0",
                                  boss7_0_verts, (int)(sizeof(boss7_0_verts) / sizeof(boss7_0_verts[0])),
                                  16, &boss7_0_frame_verts[0][0],
                                  (int)(sizeof(boss7_0_frame_verts) / sizeof(boss7_0_frame_verts[0])),
                                  4,
                                  boss7_0_faces, (int)(sizeof(boss7_0_faces) / sizeof(boss7_0_faces[0])));
    register_builtin_shape(SHAPE_BOSS7_1O, "boss_7_1o",
                           boss7_1o_verts, (int)(sizeof(boss7_1o_verts) / sizeof(boss7_1o_verts[0])),
                           boss7_1o_faces, (int)(sizeof(boss7_1o_faces) / sizeof(boss7_1o_faces[0])));
    register_builtin_shape(SHAPE_BOSS7_2, "boss_7_2",
                           boss7_2_verts, (int)(sizeof(boss7_2_verts) / sizeof(boss7_2_verts[0])),
                           boss7_2_faces, (int)(sizeof(boss7_2_faces) / sizeof(boss7_2_faces[0])));
    register_builtin_shape(SHAPE_BOSS7_3, "boss_7_3",
                           boss7_3_verts, (int)(sizeof(boss7_3_verts) / sizeof(boss7_3_verts[0])),
                           boss7_3_faces, (int)(sizeof(boss7_3_faces) / sizeof(boss7_3_faces[0])));
    register_shape_frame_variants(SHAPE_INTERNAL_BOSS7_3_FRAME1, "boss_7_3",
                                  boss7_3_verts, (int)(sizeof(boss7_3_verts) / sizeof(boss7_3_verts[0])),
                                  8, &boss7_3_frame_verts[0][0],
                                  (int)(sizeof(boss7_3_frame_verts) / sizeof(boss7_3_frame_verts[0])),
                                  3,
                                  boss7_3_faces, (int)(sizeof(boss7_3_faces) / sizeof(boss7_3_faces[0])));
    register_builtin_shape(SHAPE_BOSS7_4, "boss_7_4",
                           boss7_4_verts, (int)(sizeof(boss7_4_verts) / sizeof(boss7_4_verts[0])),
                           boss7_4_faces, (int)(sizeof(boss7_4_faces) / sizeof(boss7_4_faces[0])));
    register_shape_frame_variants(SHAPE_INTERNAL_BOSS7_4_FRAME1, "boss_7_4",
                                  boss7_4_verts, (int)(sizeof(boss7_4_verts) / sizeof(boss7_4_verts[0])),
                                  8, &boss7_4_frame_verts[0][0],
                                  (int)(sizeof(boss7_4_frame_verts) / sizeof(boss7_4_frame_verts[0])),
                                  3,
                                  boss7_4_faces, (int)(sizeof(boss7_4_faces) / sizeof(boss7_4_faces[0])));
}

static void Shapes_BuildExplodedModelMatrix(const float *base_model,
                                            const float *face_normal,
                                            uint8 explosion_state,
                                            float out[16]) {
    float rotated_x = (base_model[0] * face_normal[0]) +
                      (base_model[4] * face_normal[1]) +
                      (base_model[8] * face_normal[2]);
    float rotated_y = (base_model[1] * face_normal[0]) +
                      (base_model[5] * face_normal[1]) +
                      (base_model[9] * face_normal[2]);
    float rotated_z = (base_model[2] * face_normal[0]) +
                      (base_model[6] * face_normal[1]) +
                      (base_model[10] * face_normal[2]);
    float scale = ((float)explosion_state * 127.0f) / 4.0f;

    memcpy(out, base_model, sizeof(float) * 16);
    out[12] += rotated_x * scale;
    out[13] += -fabsf(rotated_y) * scale;
    out[14] += rotated_z * scale;
}

void Shapes_Render(uint16 shape_id, uint8 anim_frame, uint8 col_frame,
                   uint16 color_table, uint8 explosion_state,
                   const float *model_matrix) {
    shape_id = Shapes_ResolveShapeWord(shape_id);
    shape_id = Shapes_ResolveBoss7Frame(shape_id, anim_frame);
    if (shape_id >= MAX_SHAPES) return;
    const Shape *shape = &s_shapes[shape_id];
    if (!shape->gpu_ready || !shape->faces) return;

    s_current_depth_bank = Shapes_SelectDepthBank(model_matrix);

    float light_obj[3];
    Shapes_ComputeObjectLight(model_matrix, light_obj);

    if (model_matrix && (explosion_state == 0u || !shape->face_normals)) {
        GlBackend_SetMat4(g_flat_shader, "uModel", model_matrix);
    }

    glBindVertexArray(shape->vao);

    for (int i = 0; i < shape->num_faces; i++) {
        const ShapeFace *face = &shape->faces[i];
        int tri_start = shape->face_tri_start[i];
        int tri_count = shape->face_tri_count[i];
        float color[4];
        float exploded_model[16];

        if (tri_count <= 0) {
            // Wireframe (Face2) segment: draw as a line with the face's
            // material color, full-bright (the SNES does not shade lines).
            if (face->num_verts == 2 && shape->face_line_first &&
                shape->face_line_first[i] >= 0) {
                if (model_matrix && explosion_state != 0u &&
                    shape->face_normals) {
                    Shapes_BuildExplodedModelMatrix(
                        model_matrix, &shape->face_normals[i * 3],
                        explosion_state, exploded_model);
                    GlBackend_SetMat4(g_flat_shader, "uModel", exploded_model);
                }
                Shapes_ResolveFaceColor(shape_id, face->color_index, col_frame,
                                        color_table, SHADE_TABLE_LEN - 1,
                                        color);
                GlBackend_SetVec4(g_flat_shader, "uColor",
                                  color[0], color[1], color[2], color[3]);
                glDrawArrays(GL_LINES, shape->face_line_first[i], 2);
            }
            continue;
        }

        if (model_matrix && explosion_state != 0u && shape->face_normals) {
            // `mexpfacesinit` rotates each authored face normal, forces the Y
            // component downward, then applies `(normal * expcnt) >> 2`.
            // The flat C mesh data no longer carries the original byte normals,
            // so use the derived face normal from the polygon winding here.
            Shapes_BuildExplodedModelMatrix(model_matrix,
                                            &shape->face_normals[i * 3],
                                            explosion_state,
                                            exploded_model);
            GlBackend_SetMat4(g_flat_shader, "uModel", exploded_model);
        }

        int shade_index = shape->face_normals
                              ? Shapes_ComputeShadeIndex(&shape->face_normals[i * 3],
                                                         light_obj)
                              : (SHADE_TABLE_LEN - 1);

        Shapes_ResolveFaceColor(shape_id, face->color_index, col_frame,
                                color_table, shade_index, color);
        GlBackend_SetVec4(g_flat_shader, "uColor",
                          color[0], color[1], color[2], color[3]);

        glDrawArrays(GL_TRIANGLES, tri_start * 3, tri_count * 3);
    }

    glBindVertexArray(0);
}

const Shape *Shapes_Get(uint16 shape_id) {
    shape_id = Shapes_ResolveShapeWord(shape_id);
    if (shape_id >= MAX_SHAPES) return NULL;
    return &s_shapes[shape_id];
}
