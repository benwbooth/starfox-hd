//! State-aware 2D UI pass (title prompt, planet-select route map, fades).
//!
//! Port (C oracle): `src/renderer/ui.c`. `Planets_GetRoutePathIds` becomes
//! the caller-supplied `FrameInputs::route_path_ids` slice (this crate does
//! not depend on sf-game). The `SF_UI_DEBUG` overlay is not ported.

use std::path::{Path, PathBuf};

use crate::bg2d::Bg2d;
use crate::font::Font;
use crate::gpu::{Gpu, TextureId, Vertex2, WHITE_TEX};
use crate::renderer::{
    EndingReplayBackdrop, EndingReplayInputs, FrameInputs, GameState, Sf2AudioOutput,
    Sf2Difficulty, Sf2FlightControlStyle, Sf2FrameInputs, Sf2GameOverChoice, Sf2GameOverPhase,
    Sf2MissionBackdrop, Sf2Mode, Sf2Pilot, Sf2PilotSelectionCursor, Sf2PilotSelectionPhase,
    Sf2ResultsChoice, Sf2ResultsPhase,
    Sf2StrategicActor, Sf2StrategicActorAppearance, Sf2StrategicActorKind, Sf2StrategicPhase,
    Sf2TitleMenuItem, Sf2TitlePage, WINDOW_MODE_BLACK, WINDOW_MODE_MAPFADE,
    WINDOW_MODE_WHITE2NORM, WINDOW_MODE_WHITEFADE,
};
use crate::sprites::decode_4bpp_tile;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

const SF2_REFERENCE_WIDTH: i32 = 256;
const SF2_REFERENCE_HEIGHT: i32 = 224;
const SF2_OPAQUE_BLACK_PIXEL: [u8; 4] = [0, 0, 0, u8::MAX];
const SF2_GAME_OVER_CONTINUE_END_RETAIL_FRAME: u16 = 172;
const SF2_GAME_OVER_RESULTS_END_RETAIL_FRAME: u16 = 76;
const SF2_FADE_THIRTEEN_FRAMES_BEFORE_END: u16 = 24;
const SF2_FADE_ELEVEN_FRAMES_BEFORE_END: u16 = 20;
const SF2_FADE_NINE_FRAMES_BEFORE_END: u16 = 16;
const SF2_FADE_FIVE_FRAMES_BEFORE_END: u16 = 12;
const SF2_FADE_THREE_FRAMES_BEFORE_END: u16 = 8;
const SF2_FADE_ONE_FRAMES_BEFORE_END: u16 = 4;
const SF2_RESULTS_EXIT_RETAIL_FRAMES: u16 = 128;
const SF2_RESULTS_RETRY_FADE_THIRTEEN_RETAIL_FRAME: u16 = 120;
const SF2_RESULTS_RETRY_FADE_SEVEN_RETAIL_FRAME: u16 = 124;
const SF2_RESULTS_TITLE_FADE_NINE_RETAIL_FRAME: u16 = 124;
const SF2_MAX_SCORE: u32 = 99_999;
const SF2_MISSION_SCORE_X: i32 = 62;
const SF2_MISSION_SCORE_TOP: i32 = 17;
const SF2_MISSION_TIMER_X: i32 = 163;
const SF2_MISSION_TIMER_TOP: i32 = 17;
const SF2_MISSION_TIMER_SEPARATOR_X: i32 = 179;
const SF2_MISSION_TIMER_SEPARATOR_TOP: i32 = 18;
const SF2_MISSION_TIMER_FRACTION_X: i32 = 187;
const SF2_MISSION_TIMER_TENTHS_PER_UNIT: u16 = 10;
const SF2_MISSION_TIMER_MAX_WHOLE: u16 = 99;
const SF2_MISSION_ITEM_COUNT_X: i32 = 37;
const SF2_MISSION_ITEM_COUNT_TOP: i32 = 190;
const SF2_MISSION_SHIELD_PIP_X: i32 = 206;
const SF2_MISSION_SHIELD_PIP_TOP: i32 = 190;
const SF2_MISSION_SHIELD_PIP_COLUMNS: usize = 4;
const SF2_MISSION_SHIELD_PIP_CAPACITY: usize = 8;
const SF2_MISSION_MAX_SHIELD: u8 = 32;
const SF2_MISSION_SHIELD_PER_PIP: usize = 4;
const SF2_MISSION_MAX_ITEM_COUNT: u8 = 3;
const SF2_RADAR_PLAYER_LEFT: i32 = 216;
const SF2_RADAR_PLAYER_TOP: i32 = 31;
const SF2_RADAR_AXIS_RADIUS: i32 = 17;
const SF2_RADAR_WORLD_RANGE: i32 = 16_384;
const SF2_MAP_PRIMARY_PILOT_LEFT: i32 = 74;
const SF2_MAP_WINGMATE_LEFT: i32 = 93;
const SF2_MAP_PILOT_TOP: i32 = 196;
const SF2_MAP_SHIELD_LEFT: i32 = 118;
const SF2_MAP_POST_ELADARD_SHIELD_LEFT: i32 = 114;
const SF2_MAP_PRIMARY_SHIELD_TOP: i32 = 197;
const SF2_MAP_WINGMATE_SHIELD_TOP: i32 = 205;
const SF2_MAP_SHIELD_PIPS_PER_PILOT: usize = 4;
const SF2_MAP_SHIELD_PER_PIP: usize = 8;
const SF2_MAP_ITEM_ICON_LEFT: i32 = 157;
const SF2_MAP_ITEM_ICON_TOP: i32 = 196;
const SF2_MAP_GAUGE_LEFT: i32 = 240;
const SF2_MAP_GAUGE_TOP: i32 = 182;
const SF2_MAP_FIRST_CRAFT_ACCENT_X_OFFSET: i32 = 8;
const SF2_MAP_FIRST_CRAFT_ACCENT_Y_OFFSET: i32 = -1;
const SF2_MAP_SECOND_CRAFT_ACCENT_X_OFFSET: i32 = 0;
const SF2_MAP_SECOND_CRAFT_ACCENT_Y_OFFSET: i32 = 5;
const SF2_MAP_SECOND_CRAFT_CURSOR_X_OFFSET: i32 = -4;
const SF2_MAP_SECOND_CRAFT_CURSOR_Y_OFFSET: i32 = 6;
const SF2_MAP_POST_INTERCEPTION_CRAFT_CURSOR_X_OFFSET: i32 = 4;
const SF2_MAP_POST_INTERCEPTION_CRAFT_CURSOR_Y_OFFSET: i32 = 6;
const SF2_MAP_POST_FIGHTER_INTERCEPT_CRAFT_CURSOR_X_OFFSET: i32 = 8;
const SF2_MAP_POST_FIGHTER_INTERCEPT_CRAFT_CURSOR_Y_OFFSET: i32 = 2;
const SF2_MAP_POST_PIGMA_CRAFT_CURSOR_X_OFFSET: i32 = 1;
const SF2_MAP_POST_PIGMA_CRAFT_CURSOR_Y_OFFSET: i32 = -2;
const SF2_MAP_POST_ELADARD_CRAFT_CURSOR_X_OFFSET: i32 = 4;
const SF2_MAP_POST_ELADARD_CRAFT_CURSOR_Y_OFFSET: i32 = 6;
const SF2_MAP_POST_CARRIER_CRAFT_CURSOR_X_OFFSET: i32 = 4;
const SF2_MAP_POST_CARRIER_CRAFT_CURSOR_Y_OFFSET: i32 = -4;
const SF2_MAP_POST_LEON_CRAFT_MARKER_X_OFFSET: i32 = 0;
const SF2_MAP_POST_LEON_CRAFT_MARKER_Y_OFFSET: i32 = -6;
const SF2_MAP_POST_LEON_CRAFT_MARKER_WIDTH: i32 = 16;
const SF2_MAP_POST_LEON_CRAFT_MARKER_HEIGHT: i32 = 24;
const SF2_MAP_POST_MIRAGE_CRAFT_MARKER_X_OFFSET: i32 = 0;
const SF2_MAP_POST_MIRAGE_CRAFT_MARKER_Y_OFFSET: i32 = 0;
const SF2_MAP_POST_MIRAGE_CRAFT_MARKER_WIDTH: i32 = 16;
const SF2_MAP_POST_MIRAGE_CRAFT_MARKER_HEIGHT: i32 = 24;
const SF2_MAP_DAMAGE_LEFT: i32 = 16;
const SF2_MAP_DAMAGE_TOP: i32 = 182;
const SF2_MAP_MAX_DAMAGE_PERCENT: u8 = 99;
const SF2_MAP_DAMAGE_WARNING_PERCENT: u8 = 50;
const SF2_MAP_TIME_LEFT: i32 = 200;
const SF2_MAP_TIME_TOP: i32 = 190;
const SF2_MAP_SCORE_LEFT: i32 = 192;
const SF2_MAP_SCORE_TOP: i32 = 206;
const SF2_MAP_SHIELD_ROW_WIDTH: i32 = 40;
const SF2_MAP_ITEM_LEFT: i32 = 176;
const SF2_MAP_ITEM_TOP: i32 = 198;
const SF2_CAMPAIGN_FRAMES_PER_DISPLAY_SECOND: u64 = 15;
const SF2_MAP_MAX_DISPLAY_SECONDS: u64 = 999;
const SF2_MAP_SCORE_DIGIT_DIVISORS: [u32; 5] = [10_000, 1_000, 100, 10, 1];
const SF2_MAP_BACKGROUND_COLOR: [f32; 4] = [24.0 / 255.0, 24.0 / 255.0, 90.0 / 255.0, 1.0];
const SF2_MAP_DAMAGE_BACKGROUND_COLOR: [f32; 4] = [24.0 / 255.0, 24.0 / 255.0, 24.0 / 255.0, 1.0];
const SF2_BLASTER_PALETTE_PHASE_TICKS: u32 = 2;

const TALLY_PORTRAIT_WIDTH: usize = 32;
const TALLY_PORTRAIT_HEIGHT: usize = 40;
const TALLY_PORTRAIT_SLOTS: usize = 5;
const TALLY_PORTRAIT_ATLAS_WIDTH: usize = TALLY_PORTRAIT_WIDTH * TALLY_PORTRAIT_SLOTS;
const TALLY_PORTRAIT_ATLAS_HEIGHT: usize = TALLY_PORTRAIT_HEIGHT;
const TALLY_PORTRAIT_FRAME_BYTES: usize = 640;
const TALLY_PORTRAIT_TILE_COLUMNS: usize = 4;
const TALLY_PORTRAIT_TILE_ROWS: usize = 5;
const TALLY_PORTRAIT_SOURCE_FRAMES: [usize; TALLY_PORTRAIT_SLOTS] = [7, 9, 11, 17, 4];
const TALLY_MAX_TEAMMATE_SHIELD: u8 = 40;
const TALLY_GRAPH_INNER_WIDTH: i32 = 100;
const TALLY_TEAMMATE_BAR_INNER_WIDTH: i32 = 40;
const TALLY_WHITE: [f32; 4] = [232.0 / 255.0, 248.0 / 255.0, 248.0 / 255.0, 1.0];
const TALLY_CYAN: [f32; 4] = [104.0 / 255.0, 216.0 / 255.0, 248.0 / 255.0, 1.0];
const TALLY_PINK: [f32; 4] = [240.0 / 255.0, 88.0 / 255.0, 104.0 / 255.0, 1.0];
const TALLY_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const ENDING_TEXT_LEFT: i32 = 16;
const ENDING_TITLE_TOP: i32 = 24;
const ENDING_SUBTITLE_TOP: i32 = 32;
const ENDING_LOCATION_TOP: i32 = 40;
const ENDING_LOCATION_SECOND_TOP: i32 = 56;
const ENDING_DETAILS_TOP: i32 = 168;

/// Retail BLUE.COL row 7, used by the framebuffer portrait data. BGR555 is
/// retained here because hexadecimal notation expresses the packed colors.
const TALLY_PORTRAIT_PALETTE_BGR555: [u16; 16] = [
    0x0000, 0x2035, 0x357E, 0x36BF, 0x4B5F, 0x6D40, 0x7E2C, 0x7F6D, 0x7FF5, 0x24C3, 0x3989, 0x4E0E,
    0x62D3, 0x7778, 0x7FFD, 0x0220,
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

// Planet-select graphics atlas
const PS_AW: usize = 256;
const PS_AH: usize = 96;

// ---------------------------------------------------------------------------
// PLANETS.ASM literal tables
// ---------------------------------------------------------------------------

/// planetpos: top-left of each planet's 32x32 square, SNES pixels (y down).
static PLANET_POS: [[u8; 2]; 16] = [
    [16, 176],
    [64, 176],
    [56, 136],
    [16, 128],
    [112, 144],
    [40, 64],
    [128, 96],
    [184, 144],
    [168, 96],
    [112, 32],
    [80, 80],
    [208, 96],
    [192, 40],
    [192, 40],
    [136, 176],
    [192, 40],
];

/// startplanetpos: course-line/ship anchor per planet.
static START_POS: [[u8; 2]; 16] = [
    [16, 176],
    [64, 176],
    [64, 136],
    [16, 128],
    [112, 144],
    [48, 64],
    [128, 104],
    [184, 144],
    [176, 96],
    [112, 32],
    [80, 80],
    [208, 96],
    [192, 40],
    [192, 40],
    [136, 176],
    [192, 40],
];

/// planetsprs: which SuperFX msprites texture cell each planet uses.
#[derive(Clone, Copy)]
struct PlanetSpr {
    sheet: u8,
    cell: u8,
    sphere: u8,
}

const fn psr(sheet: u8, cell: u8, sphere: u8) -> PlanetSpr {
    PlanetSpr {
        sheet,
        cell,
        sphere,
    }
}

static PLANET_SPRS: [PlanetSpr; 16] = [
    psr(1, 24, 1), //  0 Corneria      (playerplanet)
    psr(1, 18, 0), //  1 Asteroid 1    (space2)
    psr(1, 18, 0), //  2 Asteroid 3    (space2)
    psr(1, 17, 0), //  3 Sector X      (space1)
    psr(1, 28, 1), //  4 Fortuna       (planetb)
    psr(1, 26, 1), //  5 Titania       (planeta)
    psr(1, 21, 0), //  6 Space Armada  (bigships)
    psr(1, 19, 0), //  7 Sector Z      (cluster)
    psr(1, 16, 0), //  8 Meteor        (bigmeteo)
    psr(0, 20, 0), //  9 Sector Y      (space4)
    psr(1, 20, 0), // 10 Black Hole    (blackhole)
    psr(1, 22, 1), // 11 Macbeth       (planetc)
    psr(0, 7, 0),  // 12 unused        (space3)
    psr(0, 20, 0), // 13 unused        (space4)
    psr(0, 14, 0), // 14 OOTD          (starwars3)
    psr(1, 30, 1), // 15 Venom         (enemyplanet)
];

// stagepaths line geometry: 8px steps with MAP-OBJ.CGX segment tiles.
const SEG_HIDDEN: u8 = 0;
const SEG_DIAG_UP: u8 = 1; // tile 0        '/'  (PATHUPRIGHT)
const SEG_DIAG_DN: u8 = 2; // tile 0 vflip  '\'  (PATHDOWNRIGHT)
const SEG_HORIZ: u8 = 3; //   tile 1             (PATHRIGHT / PATHHIGHRIGHT)
const SEG_VERT: u8 = 4; //    tile 2 hflip       (PATHUP / PATHDOWN)

#[derive(Clone, Copy)]
struct PathStep {
    dx: i8,
    dy: i8,
    seg: u8,
}

const P_UP: PathStep = PathStep {
    dx: 0,
    dy: -8,
    seg: SEG_VERT,
};
const P_UR: PathStep = PathStep {
    dx: 8,
    dy: -8,
    seg: SEG_DIAG_UP,
};
const P_R: PathStep = PathStep {
    dx: 8,
    dy: 0,
    seg: SEG_HORIZ,
};
const P_UPH: PathStep = PathStep {
    dx: 0,
    dy: -8,
    seg: SEG_HIDDEN,
};
const P_URH: PathStep = PathStep {
    dx: 8,
    dy: -8,
    seg: SEG_HIDDEN,
};
const P_DRH: PathStep = PathStep {
    dx: 8,
    dy: 8,
    seg: SEG_HIDDEN,
};
const P_RH: PathStep = PathStep {
    dx: 8,
    dy: 0,
    seg: SEG_HIDDEN,
};

static STEPS1: [PathStep; 1] = [P_UP];
static STEPS2: [PathStep; 3] = [P_UP, P_UP, P_UR];
static STEPS3: [PathStep; 5] = [P_UR, P_UR, P_UR, P_R, P_R];
static STEPS4: [PathStep; 4] = [P_R, P_R, P_R, P_R];
static STEPS6: [PathStep; 1] = [P_UR];
static STEPS7: [PathStep; 5] = [P_UR, P_UR, P_UR, P_R, P_R];
static STEPS8: [PathStep; 1] = [P_R];
static STEPS9: [PathStep; 1] = [P_UP];
static STEPS11: [PathStep; 2] = [P_R, P_R];
static STEPS12: [PathStep; 2] = [P_UR, P_UR];
static STEPS13: [PathStep; 5] = [P_R, P_R, P_R, P_R, P_R];
static STEPS14: [PathStep; 2] = [P_UR, P_UP];
static STEPS15: [PathStep; 1] = [P_UP];
static STEPS17: [PathStep; 3] = [P_URH, P_URH, P_URH];
static STEPS18: [PathStep; 2] = [P_URH, P_URH];
static STEPS19: [PathStep; 9] = [P_RH, P_RH, P_URH, P_URH, P_RH, P_RH, P_RH, P_RH, P_RH];
static STEPS20: [PathStep; 12] = [
    P_DRH, P_DRH, P_DRH, P_DRH, P_RH, P_RH, P_RH, P_RH, P_RH, P_RH, P_RH, P_RH,
];
static STEPS21: [PathStep; 3] = [P_UPH, P_UPH, P_URH];
static STEPS22: [PathStep; 6] = [P_RH, P_RH, P_RH, P_RH, P_RH, P_RH];

struct PathGeo {
    sx: u8, // PATHSTART x,y (8px units)
    sy: u8,
    steps: &'static [PathStep],
}

const fn geo(sx: u8, sy: u8, steps: &'static [PathStep]) -> PathGeo {
    PathGeo { sx, sy, steps }
}
const GEO_NONE: PathGeo = PathGeo {
    sx: 0,
    sy: 0,
    steps: &[],
};

/// Indexed by planets.c PATH_ID_* (1..22, then END3/END2/END1/OTHEREND).
const UI_PATH_ID_COUNT: usize = 27;
static PATH_GEO: [PathGeo; UI_PATH_ID_COUNT] = [
    GEO_NONE,              //  0 invalid
    geo(4, 20, &STEPS1),   //  1 Corneria 2
    geo(4, 14, &STEPS2),   //  2 Sector X
    geo(9, 8, &STEPS3),    //  3 Titania
    geo(18, 5, &STEPS4),   //  4 Sector Y
    GEO_NONE,              //  5 Venom 2 orbital
    geo(6, 21, &STEPS6),   //  6 Corneria 1
    geo(11, 17, &STEPS7),  //  7 Asteroid Belt 1
    geo(20, 14, &STEPS8),  //  8 Space Armada
    geo(24, 11, &STEPS9),  //  9 Meteor
    GEO_NONE,              // 10 Venom 1 orbital
    geo(6, 23, &STEPS11),  // 11 Corneria 3
    geo(12, 21, &STEPS12), // 12 Asteroid Belt 3
    geo(18, 19, &STEPS13), // 13 Fortuna
    geo(27, 19, &STEPS14), // 14 Sector Z
    geo(28, 11, &STEPS15), // 15 Macbeth
    GEO_NONE,              // 16 Venom 3 orbital
    geo(6, 15, &STEPS17),  // 17 Sector X (black hole entry)
    geo(12, 9, &STEPS18),  // 18 Black Hole -> Sector Y
    geo(13, 11, &STEPS19), // 19 Black Hole -> Venom 1
    geo(11, 14, &STEPS20), // 20 Black Hole -> Sector Z
    geo(9, 16, &STEPS21),  // 21 Asteroid 1 (black hole entry)
    geo(12, 25, &STEPS22), // 22 Asteroid 3 (OOTD entry)
    GEO_NONE,              // 23 end3
    GEO_NONE,              // 24 end2
    GEO_NONE,              // 25 end1
    GEO_NONE,              // 26 otherend
];

// ---------------------------------------------------------------------------
// Planet-select atlas composition helpers (pure)
// ---------------------------------------------------------------------------

/// One 16-color BGR555 palette row from a .COL file.
fn ps_pal_row(col: &[u8], row: usize) -> [[u8; 4]; 16] {
    let mut pal = [[0u8; 4]; 16];
    for (c, entry) in pal.iter_mut().enumerate() {
        let w = col[row * 32 + c * 2] as u16 | ((col[row * 32 + c * 2 + 1] as u16) << 8);
        entry[0] = (((w) & 0x1F) as u32 * 255 / 31) as u8;
        entry[1] = (((w >> 5) & 0x1F) as u32 * 255 / 31) as u8;
        entry[2] = (((w >> 10) & 0x1F) as u32 * 255 / 31) as u8;
        entry[3] = 255;
    }
    pal
}

fn tally_portrait_color(index: usize) -> [u8; 4] {
    let packed = TALLY_PORTRAIT_PALETTE_BGR555[index];
    [
        ((packed & 31) * 255 / 31) as u8,
        (((packed >> 5) & 31) * 255 / 31) as u8,
        (((packed >> 10) & 31) * 255 / 31) as u8,
        // FACE.CGX is copied directly into the visible framebuffer. Palette
        // index zero is opaque black here, not an OAM transparency key.
        u8::MAX,
    ]
}

/// Decode the five exact FACE.CGX frames used by the tally: the three living
/// pilots followed by the two alternating dead/static frames. The source file
/// stores each 32x40 frame as four columns of five ordinary 4bpp tiles.
pub fn compose_tally_portrait_atlas(face_data: &[u8]) -> Option<Vec<u8>> {
    let required_frames = TALLY_PORTRAIT_SOURCE_FRAMES
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        + 1;
    if face_data.len() < required_frames * TALLY_PORTRAIT_FRAME_BYTES {
        return None;
    }

    let mut atlas = vec![0u8; TALLY_PORTRAIT_ATLAS_WIDTH * TALLY_PORTRAIT_ATLAS_HEIGHT * 4];
    let mut tile = [0u8; 64];
    for (slot, source_frame) in TALLY_PORTRAIT_SOURCE_FRAMES.iter().copied().enumerate() {
        let frame_start = source_frame * TALLY_PORTRAIT_FRAME_BYTES;
        for tile_x in 0..TALLY_PORTRAIT_TILE_COLUMNS {
            for tile_y in 0..TALLY_PORTRAIT_TILE_ROWS {
                let tile_index = tile_x * TALLY_PORTRAIT_TILE_ROWS + tile_y;
                let tile_start = frame_start + tile_index * 32;
                decode_4bpp_tile(&face_data[tile_start..tile_start + 32], &mut tile);
                for pixel_y in 0..8 {
                    for pixel_x in 0..8 {
                        let color = tally_portrait_color(tile[pixel_y * 8 + pixel_x] as usize);
                        let atlas_x = slot * TALLY_PORTRAIT_WIDTH + tile_x * 8 + pixel_x;
                        let atlas_y = tile_y * 8 + pixel_y;
                        let output = (atlas_y * TALLY_PORTRAIT_ATLAS_WIDTH + atlas_x) * 4;
                        atlas[output..output + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
    Some(atlas)
}

/// Pixel of a CGX viewed as a 128px-wide sheet (16 tiles per row).
fn ps_sheet_px(tiles64: &[u8], ntiles: usize, x: i32, y: i32) -> u8 {
    let t = (y >> 3) * 16 + (x >> 3);
    if t < 0 || t as usize >= ntiles {
        return 0;
    }
    tiles64[t as usize * 64 + ((y & 7) * 8 + (x & 7)) as usize]
}

fn ps_put(atlas: &mut [u8], x: usize, y: usize, rgba: [u8; 4]) {
    let px = &mut atlas[(y * PS_AW + x) * 4..][..4];
    px.copy_from_slice(&rgba);
}

/// Flat 32x32 msprites cell -> atlas (color 0 transparent).
fn ps_blit_flat(
    atlas: &mut [u8],
    ax: usize,
    ay: usize,
    sheet: &[u8],
    ntiles: usize,
    cell: u8,
    pal: &[[u8; 4]; 16],
) {
    let bx = (cell as i32 & 3) * 32;
    let by = (cell as i32 >> 2) * 32;
    for y in 0..32 {
        for x in 0..32 {
            let v = ps_sheet_px(sheet, ntiles, bx + x, by + y);
            if v == 0 {
                continue;
            }
            ps_put(atlas, ax + x as usize, ay + y as usize, pal[v as usize]);
        }
    }
}

/// dpsphere planet: bake the 64x32 wrap texture onto a shaded 32x32 disc.
fn ps_blit_sphere(
    atlas: &mut [u8],
    ax: usize,
    ay: usize,
    sheet: &[u8],
    ntiles: usize,
    cell: u8,
    pal: &[[u8; 4]; 16],
) {
    let bx = (cell as i32 & 3) * 32; // doub: cells c, c+1
    let by = (cell as i32 >> 2) * 32;
    let r = 15.0f32;
    let pi = std::f32::consts::PI;
    for y in 0..32i32 {
        for x in 0..32i32 {
            let dx = (x as f32 - 15.5) / r;
            let dy = (y as f32 - 15.5) / r;
            let d2 = dx * dx + dy * dy;
            if d2 > 1.0 {
                continue;
            }
            let nz = (1.0 - d2).sqrt();
            let lon = dx.asin();
            let lat = dy.asin();
            let u = ((32.0 + lon * (64.0 / (2.0 * pi))) as i32).clamp(0, 63);
            let v = ((16.0 + lat * (32.0 / pi)) as i32).clamp(0, 31);
            let t = ps_sheet_px(sheet, ntiles, bx + u, by + v) as usize;
            let shade = 0.30 + 0.70 * nz; // limb darkening
            let rgba = [
                (pal[t][0] as f32 * shade) as u8,
                (pal[t][1] as f32 * shade) as u8,
                (pal[t][2] as f32 * shade) as u8,
                255,
            ];
            ps_put(atlas, ax + x as usize, ay + y as usize, rgba);
        }
    }
}

/// 8x8 4bpp tile -> atlas; opaque_bg0: color 0 drawn as pal[0] (BG chars).
fn ps_blit_tile(
    atlas: &mut [u8],
    ax: usize,
    ay: usize,
    tile64: &[u8],
    pal: &[[u8; 4]; 16],
    opaque_bg0: bool,
) {
    for y in 0..8 {
        for x in 0..8 {
            let v = tile64[y * 8 + x] as usize;
            if v == 0 && !opaque_bg0 {
                continue;
            }
            ps_put(atlas, ax + x, ay + y, pal[v]);
        }
    }
}

/// Split a running-total score into the five glyphs of the map-screen score
/// line: three decimal places of the total (capped at 999, matching the ROM's
/// per-place count-down in `drawroutename`) followed by two fixed zeros — the
/// score is displayed x100 (PLANETS.ASM:1583-1595). Each entry is a digit
/// 0..=9 selecting atlas column `144 + digit*8` (glyphs '0'..'9').
pub(crate) fn score_line_digits(score: u16) -> [u8; 5] {
    let s = score.min(999);
    [
        (s / 100 % 10) as u8,
        (s / 10 % 10) as u8,
        (s % 10) as u8,
        0,
        0,
    ]
}

/// CPU half of `ps_ensure_atlas`: compose the planet-select atlas from raw
/// asset bytes. Public for tests.
pub fn compose_planet_select_atlas(
    tex0: &[u8],
    tex1: &[u8],
    mo: &[u8],
    mocol: &[u8],
    mcgx: &[u8],
    mcol: &[u8],
) -> Option<Vec<u8>> {
    if tex0.len() < 0x4000
        || tex1.len() < 0x4000
        || mo.len() < 30 * 32
        || mocol.len() < 32
        || mcgx.len() < 160 * 32
        || mcol.len() < 7 * 32
    {
        return None;
    }

    // Decode tile data. msprites CGX: only the first 0x4000 bytes are
    // texture data (512 tiles = 128x256 sheet).
    let mut t0 = vec![0u8; 512 * 64];
    let mut t1 = vec![0u8; 512 * 64];
    let mut tm = vec![0u8; 30 * 64];
    let n_tg = mcgx.len() / 32;
    let mut tg = vec![0u8; n_tg * 64];
    let mut tile = [0u8; 64];
    for t in 0..512 {
        decode_4bpp_tile(&tex0[t * 32..], &mut tile);
        t0[t * 64..t * 64 + 64].copy_from_slice(&tile);
        decode_4bpp_tile(&tex1[t * 32..], &mut tile);
        t1[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }
    for t in 0..30 {
        decode_4bpp_tile(&mo[t * 32..], &mut tile);
        tm[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }
    for t in 0..n_tg {
        decode_4bpp_tile(&mcgx[t * 32..], &mut tile);
        tg[t * 64..t * 64 + 64].copy_from_slice(&tile);
    }

    // Palettes: planet textures use CGRAM row 0; OBJ cursor/line use
    // MAP-OBJ.COL row 0 (CGRAM row 15); route label/score chars use BG
    // palette 6 with BG color 0 = CGRAM color 0.
    let pal_map = ps_pal_row(mcol, 0);
    let pal_obj = ps_pal_row(mocol, 0);
    let mut pal_txt = ps_pal_row(mcol, 6);
    pal_txt[0][0] = pal_map[0][0];
    pal_txt[0][1] = pal_map[0][1];
    pal_txt[0][2] = pal_map[0][2];

    let mut atlas = vec![0u8; PS_AW * PS_AH * 4];

    // Planet icons
    for p in 0..16 {
        let ps = &PLANET_SPRS[p];
        let sheet = if ps.sheet != 0 { &t1 } else { &t0 };
        let ax = (p & 7) * 32;
        let ay = (p >> 3) * 32;
        if ps.sphere != 0 {
            ps_blit_sphere(&mut atlas, ax, ay, sheet, 512, ps.cell, &pal_map);
        } else {
            ps_blit_flat(&mut atlas, ax, ay, sheet, 512, ps.cell, &pal_map);
        }
    }

    // Arwing cursor: OAM chars {9,5,13}+0..3 = CGX tiles {7,3,11}+0..3,
    // 2x2 of 8x8 (TL, TR, BL, BR). Angle 0 = up, 1 = diagonal, 2 = right.
    static SHIP_BASE: [usize; 3] = [7, 3, 11];
    for (angle, &base) in SHIP_BASE.iter().enumerate() {
        let ax = angle * 16;
        ps_blit_tile(&mut atlas, ax, 64, &tm[base * 64..], &pal_obj, false);
        ps_blit_tile(
            &mut atlas,
            ax + 8,
            64,
            &tm[(base + 1) * 64..],
            &pal_obj,
            false,
        );
        ps_blit_tile(&mut atlas, ax, 72, &tm[(base + 2) * 64..], &pal_obj, false);
        ps_blit_tile(
            &mut atlas,
            ax + 8,
            72,
            &tm[(base + 3) * 64..],
            &pal_obj,
            false,
        );
    }

    // Course line segment tiles 0 '/', 1 '-', 2 '|'
    for i in 0..3 {
        ps_blit_tile(&mut atlas, 48 + i * 8, 64, &tm[i * 64..], &pal_obj, false);
    }

    // Route labels (drawroutename): 6 chars each, CGX tile = char - 32.
    // whichroute 0 -> $74 (LEVEL2), 1 -> $6e (LEVEL1), 2 -> $7a (LEVEL3).
    static NAME_BASE: [usize; 3] = [0x74, 0x6e, 0x7a];
    for (n, &base) in NAME_BASE.iter().enumerate() {
        for c in 0..6 {
            let tile = base + c;
            if tile >= n_tg {
                continue;
            }
            ps_blit_tile(
                &mut atlas,
                n * 48 + c * 8,
                80,
                &tg[tile * 64..],
                &pal_txt,
                true,
            );
        }
    }
    // Score digits '0'..'9' (BG font CGX tiles $88..$91, consecutive — ROM
    // drawroutename emits `digit + $88` per place, PLANETS.ASM:1620-1624).
    // Laid out at atlas (144 + d*8, 80) so a score place samples column d.
    for d in 0..10usize {
        if 0x88 + d < n_tg {
            ps_blit_tile(
                &mut atlas,
                144 + d * 8,
                80,
                &tg[(0x88 + d) * 64..],
                &pal_txt,
                true,
            );
        }
    }

    Some(atlas)
}

// ---------------------------------------------------------------------------
// GL runtime state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sf2BlasterPalettePhase {
    Cool,
    Warm,
}

impl Sf2BlasterPalettePhase {
    fn at_frame(frame: u32) -> Self {
        if frame % SF2_BLASTER_PALETTE_PHASE_TICKS == 0 {
            Self::Cool
        } else {
            Self::Warm
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sf2GameOverRenderKey {
    track: crate::sf2_game_over::Track,
    frame_index: usize,
    portrait: Option<crate::sf2_game_over::Portrait>,
    brightness: crate::sf2_game_over::Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sf2ResultsRenderKey {
    track: crate::sf2_results::Track,
    frame_index: usize,
    brightness: crate::sf2_game_over::Brightness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sf2PilotSelectionRenderKey {
    screen: crate::sf2_pilot_selection::Screen,
    frame_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sf2TitleRenderKey {
    track: crate::sf2_title::Track,
    frame_index: usize,
}

fn sf2_game_over_brightness(
    choice: Sf2GameOverChoice,
    elapsed_retail_frames: u16,
) -> crate::sf2_game_over::Brightness {
    let end = match choice {
        Sf2GameOverChoice::ContinueWithWingmate => SF2_GAME_OVER_CONTINUE_END_RETAIL_FRAME,
        Sf2GameOverChoice::EndCampaign => SF2_GAME_OVER_RESULTS_END_RETAIL_FRAME,
    };
    match elapsed_retail_frames {
        elapsed if elapsed >= end => crate::sf2_game_over::Brightness::Black,
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_ONE_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::OneFifteenth
        }
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_THREE_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::ThreeFifteenths
        }
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_FIVE_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::FiveFifteenths
        }
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_NINE_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::NineFifteenths
        }
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_ELEVEN_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::ElevenFifteenths
        }
        elapsed if elapsed >= end.saturating_sub(SF2_FADE_THIRTEEN_FRAMES_BEFORE_END) => {
            crate::sf2_game_over::Brightness::ThirteenFifteenths
        }
        _ => crate::sf2_game_over::Brightness::Full,
    }
}

fn sf2_results_brightness(
    phase: Sf2ResultsPhase,
    choice: Sf2ResultsChoice,
    elapsed_retail_frames: u16,
) -> crate::sf2_game_over::Brightness {
    if phase != Sf2ResultsPhase::Leaving {
        return crate::sf2_game_over::Brightness::Full;
    }
    match (choice, elapsed_retail_frames) {
        (_, elapsed) if elapsed >= SF2_RESULTS_EXIT_RETAIL_FRAMES => {
            crate::sf2_game_over::Brightness::Black
        }
        (Sf2ResultsChoice::Retry, elapsed)
            if elapsed >= SF2_RESULTS_RETRY_FADE_SEVEN_RETAIL_FRAME =>
        {
            crate::sf2_game_over::Brightness::SevenFifteenths
        }
        (Sf2ResultsChoice::Retry, elapsed)
            if elapsed >= SF2_RESULTS_RETRY_FADE_THIRTEEN_RETAIL_FRAME =>
        {
            crate::sf2_game_over::Brightness::ThirteenFifteenths
        }
        (Sf2ResultsChoice::Title, elapsed)
            if elapsed >= SF2_RESULTS_TITLE_FADE_NINE_RETAIL_FRAME =>
        {
            crate::sf2_game_over::Brightness::NineFifteenths
        }
        _ => crate::sf2_game_over::Brightness::Full,
    }
}

pub struct Ui {
    base_dir: PathBuf,
    frame: u32, // render-frame counter for blink effects
    ps_tex: Option<TextureId>,
    ps_tried: bool,
    tally_portraits: Option<TextureId>,
    ending_rising_panel: TextureId,
    ending_split_panel: TextureId,
    ending_glyphs: TextureId,
    sf2_deep_space_backdrop: TextureId,
    sf2_eladard_surface_backdrop: TextureId,
    sf2_eladard_interior_backdrop: TextureId,
    sf2_titania_backdrop: TextureId,
    sf2_carrier_backdrop: TextureId,
    sf2_astropolis_void_backdrop: TextureId,
    sf2_briefing_texture: TextureId,
    sf2_briefing_presentation: crate::sf2_briefing::Presentation,
    sf2_briefing_render_frame: Option<usize>,
    sf2_opening_overview_texture: TextureId,
    sf2_opening_overview_presentation: crate::sf2_opening_overview::Presentation,
    sf2_opening_overview_render_frame: Option<usize>,
    sf2_title_texture: TextureId,
    sf2_title_presentation: crate::sf2_title::Presentation,
    sf2_title_render_key: Option<Sf2TitleRenderKey>,
    sf2_pilot_selection_texture: TextureId,
    sf2_pilot_selection_presentation: crate::sf2_pilot_selection::Presentation,
    sf2_pilot_selection_render_key: Option<Sf2PilotSelectionRenderKey>,
    sf2_game_over_texture: TextureId,
    sf2_game_over_presentation: crate::sf2_game_over::Presentation,
    sf2_game_over_render_key: Option<Sf2GameOverRenderKey>,
    sf2_results_texture: TextureId,
    sf2_results_presentation: crate::sf2_results::Presentation,
    sf2_results_render_key: Option<Sf2ResultsRenderKey>,
    sf2_aim_sight: TextureId,
    sf2_hud_glyphs: TextureId,
    sf2_map_glyphs: TextureId,
    sf2_map_post_interception_glyphs: TextureId,
    sf2_map_post_fighter_intercept_glyphs: TextureId,
    sf2_map_post_pigma_glyphs: TextureId,
    sf2_map_post_eladard_glyphs: TextureId,
    sf2_map_post_carrier_glyphs: TextureId,
    sf2_map_post_mirage_glyphs: TextureId,
    sf2_map_damage_glyphs: TextureId,
    sf2_map_damage_warning_glyphs: TextureId,
    sf2_map_damage_post_eladard_glyphs: TextureId,
    sf2_map_sprites: TextureId,
    sf2_map_post_carrier_sprites: TextureId,
    sf2_map_post_leon_sprites: TextureId,
    sf2_map_post_mirage_sprites: TextureId,
    sf2_mission_hud: TextureId,
    sf2_mission_overlay: TextureId,
    sf2_strategic_map: TextureId,
    sf2_strategic_map_escalated: TextureId,
    sf2_strategic_map_post_interception: TextureId,
    sf2_strategic_map_post_fighter_intercept: TextureId,
    sf2_strategic_map_post_pigma: TextureId,
    sf2_strategic_map_post_eladard: TextureId,
    sf2_strategic_map_post_carrier: TextureId,
    sf2_strategic_map_post_leon: TextureId,
    sf2_strategic_map_post_mirage: TextureId,

    // Screen mapping state for the current frame.
    scale: f32,
    ox: i32, // centering offset in SNES units
    scr_w: i32,
    scr_h: i32,
    proj: [f32; 16], // ortho for the current frame (set in begin_2d)
}

impl Ui {
    pub fn new(gpu: &mut Gpu, base_dir: &Path) -> Self {
        let tally_portraits = std::fs::read(base_dir.join("data/sprites/FACE.CGX"))
            .ok()
            .and_then(|bytes| compose_tally_portrait_atlas(&bytes))
            .map(|rgba| {
                gpu.create_texture_rgba(
                    TALLY_PORTRAIT_ATLAS_WIDTH as u32,
                    TALLY_PORTRAIT_ATLAS_HEIGHT as u32,
                    &rgba,
                )
            });
        if tally_portraits.is_none() {
            eprintln!("Ui: tally portraits missing/short (data/sprites/FACE.CGX)");
        }
        let ending_rising_panel_rgba =
            crate::ending::decode_panel(EndingReplayBackdrop::RisingGradient);
        let ending_rising_panel = gpu.create_texture_rgba(
            crate::ending::PANEL_WIDTH as u32,
            crate::ending::PANEL_HEIGHT as u32,
            &ending_rising_panel_rgba,
        );
        let ending_split_panel_rgba =
            crate::ending::decode_panel(EndingReplayBackdrop::SplitGradient);
        let ending_split_panel = gpu.create_texture_rgba(
            crate::ending::PANEL_WIDTH as u32,
            crate::ending::PANEL_HEIGHT as u32,
            &ending_split_panel_rgba,
        );
        let ending_glyphs_rgba = crate::ending::decode_glyph_atlas();
        let ending_glyphs = gpu.create_texture_rgba(
            crate::ending::GLYPH_ATLAS_WIDTH as u32,
            crate::ending::GLYPH_ATLAS_HEIGHT as u32,
            &ending_glyphs_rgba,
        );
        let sf2_backdrop = crate::sf2_backdrop::decode_rgba();
        let sf2_deep_space_backdrop = gpu.create_texture_rgba(
            crate::sf2_backdrop::WIDTH as u32,
            crate::sf2_backdrop::HEIGHT as u32,
            &sf2_backdrop,
        );
        let sf2_eladard_surface_backdrop_rgba = crate::sf2_eladard_surface_backdrop::decode_rgba();
        let sf2_eladard_surface_backdrop = gpu.create_texture_rgba(
            crate::sf2_eladard_surface_backdrop::WIDTH as u32,
            crate::sf2_eladard_surface_backdrop::HEIGHT as u32,
            &sf2_eladard_surface_backdrop_rgba,
        );
        let sf2_eladard_interior_backdrop_rgba =
            crate::sf2_eladard_interior_backdrop::decode_rgba();
        let sf2_eladard_interior_backdrop = gpu.create_texture_rgba(
            crate::sf2_eladard_interior_backdrop::WIDTH as u32,
            crate::sf2_eladard_interior_backdrop::HEIGHT as u32,
            &sf2_eladard_interior_backdrop_rgba,
        );
        let sf2_titania_backdrop_rgba = crate::sf2_titania_backdrop::decode_rgba();
        let sf2_titania_backdrop = gpu.create_texture_rgba(
            crate::sf2_titania_backdrop::WIDTH as u32,
            crate::sf2_titania_backdrop::HEIGHT as u32,
            &sf2_titania_backdrop_rgba,
        );
        let sf2_carrier_backdrop_rgba = crate::sf2_carrier_backdrop::decode_rgba();
        let sf2_carrier_backdrop = gpu.create_texture_rgba(
            crate::sf2_carrier_backdrop::WIDTH as u32,
            crate::sf2_carrier_backdrop::HEIGHT as u32,
            &sf2_carrier_backdrop_rgba,
        );
        let sf2_astropolis_void_backdrop = gpu.create_texture_rgba(1, 1, &SF2_OPAQUE_BLACK_PIXEL);
        let mut sf2_briefing_presentation = crate::sf2_briefing::Presentation::decode();
        let sf2_briefing_initial_rgba = sf2_briefing_presentation.frame_rgba(0);
        let sf2_briefing_texture = gpu.create_texture_rgba(
            crate::sf2_briefing::WIDTH as u32,
            crate::sf2_briefing::HEIGHT as u32,
            &sf2_briefing_initial_rgba,
        );
        let mut sf2_opening_overview_presentation =
            crate::sf2_opening_overview::Presentation::decode();
        let sf2_opening_overview_initial_rgba = sf2_opening_overview_presentation.frame_rgba(0);
        let sf2_opening_overview_texture = gpu.create_texture_rgba(
            crate::sf2_opening_overview::WIDTH as u32,
            crate::sf2_opening_overview::HEIGHT as u32,
            &sf2_opening_overview_initial_rgba,
        );
        let mut sf2_title_presentation = crate::sf2_title::Presentation::decode();
        let sf2_title_initial_rgba =
            sf2_title_presentation.frame_rgba(crate::sf2_title::Track::Mission, 0);
        let sf2_title_texture = gpu.create_texture_rgba(
            crate::sf2_title::WIDTH as u32,
            crate::sf2_title::HEIGHT as u32,
            &sf2_title_initial_rgba,
        );
        let mut sf2_pilot_selection_presentation =
            crate::sf2_pilot_selection::Presentation::decode();
        let sf2_pilot_selection_initial_rgba = sf2_pilot_selection_presentation.frame_rgba(
            crate::sf2_pilot_selection::Screen::Reveal,
            0,
        );
        let sf2_pilot_selection_texture = gpu.create_texture_rgba(
            crate::sf2_pilot_selection::WIDTH as u32,
            crate::sf2_pilot_selection::HEIGHT as u32,
            &sf2_pilot_selection_initial_rgba,
        );
        let sf2_game_over_presentation = crate::sf2_game_over::Presentation::decode();
        let sf2_game_over_initial_rgba = sf2_game_over_presentation.frame_rgba(
            crate::sf2_game_over::Track::Taunt,
            0,
            None,
            crate::sf2_game_over::Brightness::Full,
        );
        let sf2_game_over_texture = gpu.create_texture_rgba(
            crate::sf2_game_over::WIDTH as u32,
            crate::sf2_game_over::HEIGHT as u32,
            &sf2_game_over_initial_rgba,
        );
        let mut sf2_results_presentation = crate::sf2_results::Presentation::decode();
        let sf2_results_initial_rgba = sf2_results_presentation.frame_rgba(
            crate::sf2_results::Track::Reveal,
            0,
            crate::sf2_game_over::Brightness::Full,
        );
        let sf2_results_texture = gpu.create_texture_rgba(
            crate::sf2_results::WIDTH as u32,
            crate::sf2_results::HEIGHT as u32,
            &sf2_results_initial_rgba,
        );
        let sf2_aim_sight_rgba = crate::sf2_aim_sight::decode_rgba();
        let sf2_aim_sight = gpu.create_texture_rgba(
            crate::sf2_aim_sight::WIDTH as u32,
            crate::sf2_aim_sight::HEIGHT as u32,
            &sf2_aim_sight_rgba,
        );
        let sf2_mission_hud_rgba = crate::sf2_mission_hud::decode_rgba();
        let sf2_mission_hud = gpu.create_texture_rgba(
            crate::sf2_mission_hud::WIDTH as u32,
            crate::sf2_mission_hud::HEIGHT as u32,
            &sf2_mission_hud_rgba,
        );
        let sf2_hud_glyphs_rgba = crate::sf2_hud_glyphs::decode_rgba();
        let sf2_hud_glyphs = gpu.create_texture_rgba(
            crate::sf2_hud_glyphs::WIDTH as u32,
            crate::sf2_hud_glyphs::HEIGHT as u32,
            &sf2_hud_glyphs_rgba,
        );
        let sf2_map_glyphs_rgba = crate::sf2_map_glyphs::decode_rgba();
        let sf2_map_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_glyphs_rgba,
        );
        let sf2_map_post_interception_glyphs_rgba =
            crate::sf2_map_glyphs::decode_post_interception_rgba();
        let sf2_map_post_interception_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_interception_glyphs_rgba,
        );
        let sf2_map_post_fighter_intercept_glyphs_rgba =
            crate::sf2_map_glyphs::decode_post_fighter_intercept_rgba();
        let sf2_map_post_fighter_intercept_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_fighter_intercept_glyphs_rgba,
        );
        let sf2_map_post_pigma_glyphs_rgba = crate::sf2_map_glyphs::decode_post_pigma_rgba();
        let sf2_map_post_pigma_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_pigma_glyphs_rgba,
        );
        let sf2_map_post_eladard_glyphs_rgba = crate::sf2_map_glyphs::decode_post_eladard_rgba();
        let sf2_map_post_eladard_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_eladard_glyphs_rgba,
        );
        let sf2_map_post_carrier_glyphs_rgba = crate::sf2_map_glyphs::decode_post_carrier_rgba();
        let sf2_map_post_carrier_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_carrier_glyphs_rgba,
        );
        let sf2_map_post_mirage_glyphs_rgba = crate::sf2_map_glyphs::decode_post_mirage_rgba();
        let sf2_map_post_mirage_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_glyphs::WIDTH as u32,
            crate::sf2_map_glyphs::HEIGHT as u32,
            &sf2_map_post_mirage_glyphs_rgba,
        );
        let sf2_map_damage_glyphs_rgba = crate::sf2_map_damage_glyphs::decode_rgba();
        let sf2_map_damage_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_damage_glyphs::WIDTH as u32,
            crate::sf2_map_damage_glyphs::HEIGHT as u32,
            &sf2_map_damage_glyphs_rgba,
        );
        let sf2_map_damage_warning_glyphs_rgba =
            crate::sf2_map_damage_warning_glyphs::decode_rgba();
        let sf2_map_damage_warning_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_damage_warning_glyphs::WIDTH as u32,
            crate::sf2_map_damage_warning_glyphs::HEIGHT as u32,
            &sf2_map_damage_warning_glyphs_rgba,
        );
        let sf2_map_damage_post_eladard_glyphs_rgba =
            crate::sf2_map_damage_post_eladard_glyphs::decode_rgba();
        let sf2_map_damage_post_eladard_glyphs = gpu.create_texture_rgba(
            crate::sf2_map_damage_post_eladard_glyphs::WIDTH as u32,
            crate::sf2_map_damage_post_eladard_glyphs::HEIGHT as u32,
            &sf2_map_damage_post_eladard_glyphs_rgba,
        );
        let sf2_map_sprites_rgba = crate::sf2_map_sprites::decode_rgba();
        let sf2_map_sprites = gpu.create_texture_rgba(
            crate::sf2_map_sprites::WIDTH as u32,
            crate::sf2_map_sprites::HEIGHT as u32,
            &sf2_map_sprites_rgba,
        );
        let sf2_map_post_carrier_sprites_rgba = crate::sf2_map_post_carrier_sprites::decode_rgba();
        let sf2_map_post_carrier_sprites = gpu.create_texture_rgba(
            crate::sf2_map_post_carrier_sprites::WIDTH as u32,
            crate::sf2_map_post_carrier_sprites::HEIGHT as u32,
            &sf2_map_post_carrier_sprites_rgba,
        );
        let sf2_map_post_leon_sprites_rgba = crate::sf2_map_post_leon_sprites::decode_rgba();
        let sf2_map_post_leon_sprites = gpu.create_texture_rgba(
            crate::sf2_map_post_leon_sprites::WIDTH as u32,
            crate::sf2_map_post_leon_sprites::HEIGHT as u32,
            &sf2_map_post_leon_sprites_rgba,
        );
        let sf2_map_post_mirage_sprites_rgba = crate::sf2_map_post_mirage_sprites::decode_rgba();
        let sf2_map_post_mirage_sprites = gpu.create_texture_rgba(
            crate::sf2_map_post_mirage_sprites::WIDTH as u32,
            crate::sf2_map_post_mirage_sprites::HEIGHT as u32,
            &sf2_map_post_mirage_sprites_rgba,
        );
        let sf2_mission_overlay_rgba = crate::sf2_mission_overlay::decode_rgba();
        let sf2_mission_overlay = gpu.create_texture_rgba(
            crate::sf2_mission_overlay::WIDTH as u32,
            crate::sf2_mission_overlay::HEIGHT as u32,
            &sf2_mission_overlay_rgba,
        );
        let sf2_strategic_map_rgba = crate::sf2_strategic_map::decode_rgba();
        let sf2_strategic_map = gpu.create_texture_rgba(
            crate::sf2_strategic_map::WIDTH as u32,
            crate::sf2_strategic_map::HEIGHT as u32,
            &sf2_strategic_map_rgba,
        );
        let sf2_strategic_map_escalated_rgba = crate::sf2_strategic_map_escalated::decode_rgba();
        let sf2_strategic_map_escalated = gpu.create_texture_rgba(
            crate::sf2_strategic_map_escalated::WIDTH as u32,
            crate::sf2_strategic_map_escalated::HEIGHT as u32,
            &sf2_strategic_map_escalated_rgba,
        );
        let sf2_strategic_map_post_interception_rgba =
            crate::sf2_strategic_map_post_interception::decode_rgba();
        let sf2_strategic_map_post_interception = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_interception::WIDTH as u32,
            crate::sf2_strategic_map_post_interception::HEIGHT as u32,
            &sf2_strategic_map_post_interception_rgba,
        );
        let sf2_strategic_map_post_fighter_intercept_rgba =
            crate::sf2_strategic_map_post_fighter_intercept::decode_rgba();
        let sf2_strategic_map_post_fighter_intercept = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_fighter_intercept::WIDTH as u32,
            crate::sf2_strategic_map_post_fighter_intercept::HEIGHT as u32,
            &sf2_strategic_map_post_fighter_intercept_rgba,
        );
        let sf2_strategic_map_post_pigma_rgba = crate::sf2_strategic_map_post_pigma::decode_rgba();
        let sf2_strategic_map_post_pigma = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_pigma::WIDTH as u32,
            crate::sf2_strategic_map_post_pigma::HEIGHT as u32,
            &sf2_strategic_map_post_pigma_rgba,
        );
        let sf2_strategic_map_post_eladard_rgba =
            crate::sf2_strategic_map_post_eladard::decode_rgba();
        let sf2_strategic_map_post_eladard = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_eladard::WIDTH as u32,
            crate::sf2_strategic_map_post_eladard::HEIGHT as u32,
            &sf2_strategic_map_post_eladard_rgba,
        );
        let sf2_strategic_map_post_carrier_rgba =
            crate::sf2_strategic_map_post_carrier::decode_rgba();
        let sf2_strategic_map_post_carrier = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_carrier::WIDTH as u32,
            crate::sf2_strategic_map_post_carrier::HEIGHT as u32,
            &sf2_strategic_map_post_carrier_rgba,
        );
        let sf2_strategic_map_post_leon_rgba = crate::sf2_strategic_map_post_leon::decode_rgba();
        let sf2_strategic_map_post_leon = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_leon::WIDTH as u32,
            crate::sf2_strategic_map_post_leon::HEIGHT as u32,
            &sf2_strategic_map_post_leon_rgba,
        );
        let sf2_strategic_map_post_mirage_rgba =
            crate::sf2_strategic_map_post_mirage::decode_rgba();
        let sf2_strategic_map_post_mirage = gpu.create_texture_rgba(
            crate::sf2_strategic_map_post_mirage::WIDTH as u32,
            crate::sf2_strategic_map_post_mirage::HEIGHT as u32,
            &sf2_strategic_map_post_mirage_rgba,
        );
        Ui {
            base_dir: base_dir.to_path_buf(),
            frame: 0,
            ps_tex: None,
            ps_tried: false,
            tally_portraits,
            ending_rising_panel,
            ending_split_panel,
            ending_glyphs,
            sf2_deep_space_backdrop,
            sf2_eladard_surface_backdrop,
            sf2_eladard_interior_backdrop,
            sf2_titania_backdrop,
            sf2_carrier_backdrop,
            sf2_astropolis_void_backdrop,
            sf2_briefing_texture,
            sf2_briefing_presentation,
            sf2_briefing_render_frame: None,
            sf2_opening_overview_texture,
            sf2_opening_overview_presentation,
            sf2_opening_overview_render_frame: None,
            sf2_title_texture,
            sf2_title_presentation,
            sf2_title_render_key: None,
            sf2_pilot_selection_texture,
            sf2_pilot_selection_presentation,
            sf2_pilot_selection_render_key: None,
            sf2_game_over_texture,
            sf2_game_over_presentation,
            sf2_game_over_render_key: None,
            sf2_results_texture,
            sf2_results_presentation,
            sf2_results_render_key: None,
            sf2_aim_sight,
            sf2_hud_glyphs,
            sf2_map_glyphs,
            sf2_map_post_interception_glyphs,
            sf2_map_post_fighter_intercept_glyphs,
            sf2_map_post_pigma_glyphs,
            sf2_map_post_eladard_glyphs,
            sf2_map_post_carrier_glyphs,
            sf2_map_post_mirage_glyphs,
            sf2_map_damage_glyphs,
            sf2_map_damage_warning_glyphs,
            sf2_map_damage_post_eladard_glyphs,
            sf2_map_sprites,
            sf2_map_post_carrier_sprites,
            sf2_map_post_leon_sprites,
            sf2_map_post_mirage_sprites,
            sf2_mission_hud,
            sf2_mission_overlay,
            sf2_strategic_map,
            sf2_strategic_map_escalated,
            sf2_strategic_map_post_interception,
            sf2_strategic_map_post_fighter_intercept,
            sf2_strategic_map_post_pigma,
            sf2_strategic_map_post_eladard,
            sf2_strategic_map_post_carrier,
            sf2_strategic_map_post_leon,
            sf2_strategic_map_post_mirage,
            scale: 3.0,
            ox: 0,
            scr_w: 800,
            scr_h: 600,
            proj: IDENTITY,
        }
    }

    fn begin_2d(&mut self, w: i32, h: i32) {
        self.scr_w = w;
        self.scr_h = h;
        self.scale = h as f32 / 224.0;
        self.ox = ((w as f32 / self.scale - 256.0) * 0.5) as i32;
        self.proj = ortho(w as f32, h as f32);
    }

    /// Draw a solid quad given raw screen-pixel corners.
    #[allow(clippy::too_many_arguments)]
    fn quad_px(
        &self,
        gpu: &mut Gpu,
        color: [f32; 4],
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) {
        let verts = [
            Vertex2 {
                pos: [x0, y0],
                uv: [0.0, 0.0],
            },
            Vertex2 {
                pos: [x1, y1],
                uv: [1.0, 0.0],
            },
            Vertex2 {
                pos: [x2, y2],
                uv: [1.0, 1.0],
            },
            Vertex2 {
                pos: [x3, y3],
                uv: [0.0, 1.0],
            },
        ];
        gpu.push_overlay_fan(&verts, &self.proj, &IDENTITY, color, 0, None, WHITE_TEX);
    }

    fn quad_snes(&self, gpu: &mut Gpu, color: [f32; 4], x: i32, y: i32, width: i32, height: i32) {
        let x0 = (x + self.ox) as f32 * self.scale;
        let y0 = y as f32 * self.scale;
        let x1 = (x + self.ox + width) as f32 * self.scale;
        let y1 = (y + height) as f32 * self.scale;
        self.quad_px(gpu, color, x0, y0, x1, y0, x1, y1, x0, y1);
    }

    /// Draw a top-down source-frame texture region. Native UI geometry uses a
    /// bottom-origin logical Y axis, while oracle-captured image rows and SNES
    /// screen coordinates are top-origin; keeping that conversion explicit
    /// prevents generated retail art from being presented upside down.
    fn textured_quad_source_frame(
        &self,
        gpu: &mut Gpu,
        texture: TextureId,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let logical_bottom = SF2_REFERENCE_HEIGHT - y - height;
        let x0 = (x + self.ox) as f32 * self.scale;
        let y0 = logical_bottom as f32 * self.scale;
        let x1 = (x + self.ox + width) as f32 * self.scale;
        let y1 = (logical_bottom + height) as f32 * self.scale;
        let vertices = [
            Vertex2 {
                pos: [x0, y0],
                uv: [0.0, 1.0],
            },
            Vertex2 {
                pos: [x1, y0],
                uv: [1.0, 1.0],
            },
            Vertex2 {
                pos: [x1, y1],
                uv: [1.0, 0.0],
            },
            Vertex2 {
                pos: [x0, y1],
                uv: [0.0, 0.0],
            },
        ];
        gpu.push_overlay_fan(
            &vertices,
            &self.proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            texture,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn textured_quad_source_region(
        &self,
        gpu: &mut Gpu,
        texture: TextureId,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
        atlas_top: i32,
        atlas_width: i32,
        atlas_height: i32,
    ) {
        let logical_bottom = SF2_REFERENCE_HEIGHT - destination_top - height;
        let x0 = (destination_left + self.ox) as f32 * self.scale;
        let y0 = logical_bottom as f32 * self.scale;
        let x1 = (destination_left + self.ox + width) as f32 * self.scale;
        let y1 = (logical_bottom + height) as f32 * self.scale;
        let u0 = atlas_left as f32 / atlas_width as f32;
        let u1 = (atlas_left + width) as f32 / atlas_width as f32;
        let v0 = atlas_top as f32 / atlas_height as f32;
        let v1 = (atlas_top + height) as f32 / atlas_height as f32;
        let vertices = [
            Vertex2 {
                pos: [x0, y0],
                uv: [u0, v1],
            },
            Vertex2 {
                pos: [x1, y0],
                uv: [u1, v1],
            },
            Vertex2 {
                pos: [x1, y1],
                uv: [u1, v0],
            },
            Vertex2 {
                pos: [x0, y1],
                uv: [u0, v0],
            },
        ];
        gpu.push_overlay_fan(
            &vertices,
            &self.proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            texture,
        );
    }

    fn ending_glyph_quad(&self, gpu: &mut Gpu, glyph: usize, x: i32, top: i32) {
        let glyph_size = crate::ending::GLYPH_SIZE as i32;
        let logical_bottom = SF2_REFERENCE_HEIGHT - top - glyph_size;
        let x0 = (x + self.ox) as f32 * self.scale;
        let y0 = logical_bottom as f32 * self.scale;
        let x1 = (x + self.ox + glyph_size) as f32 * self.scale;
        let y1 = (logical_bottom + glyph_size) as f32 * self.scale;
        let u0 =
            (glyph * crate::ending::GLYPH_SIZE) as f32 / crate::ending::GLYPH_ATLAS_WIDTH as f32;
        let u1 = ((glyph + 1) * crate::ending::GLYPH_SIZE) as f32
            / crate::ending::GLYPH_ATLAS_WIDTH as f32;
        let vertices = [
            Vertex2 {
                pos: [x0, y0],
                uv: [u0, 1.0],
            },
            Vertex2 {
                pos: [x1, y0],
                uv: [u1, 1.0],
            },
            Vertex2 {
                pos: [x1, y1],
                uv: [u1, 0.0],
            },
            Vertex2 {
                pos: [x0, y1],
                uv: [u0, 0.0],
            },
        ];
        gpu.push_overlay_fan(
            &vertices,
            &self.proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            self.ending_glyphs,
        );
    }

    fn ending_text(&self, gpu: &mut Gpu, x: i32, top: i32, text: &str, characters: usize) {
        for (column, character) in text.bytes().take(characters).enumerate() {
            self.ending_glyph_quad(
                gpu,
                crate::ending::glyph_index(character),
                x + column as i32 * crate::ending::GLYPH_SIZE as i32,
                top,
            );
        }
    }

    /// Draw the settled retail recap background before native 3D geometry.
    pub(crate) fn render_ending_replay_background(
        &mut self,
        gpu: &mut Gpu,
        backdrop: EndingReplayBackdrop,
        screen_width: i32,
        screen_height: i32,
    ) {
        self.begin_2d(screen_width, screen_height);
        self.quad_snes(
            gpu,
            TALLY_BLACK,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
        let texture = match backdrop {
            EndingReplayBackdrop::RisingGradient => self.ending_rising_panel,
            EndingReplayBackdrop::SplitGradient => self.ending_split_panel,
        };
        self.textured_quad_source_frame(
            gpu,
            texture,
            crate::ending::PANEL_LEFT,
            crate::ending::PANEL_TOP,
            crate::ending::PANEL_WIDTH as i32,
            crate::ending::PANEL_HEIGHT as i32,
        );
    }

    fn render_ending_replay_text(&self, gpu: &mut Gpu, replay: EndingReplayInputs<'_>) {
        self.ending_text(
            gpu,
            ENDING_TEXT_LEFT,
            ENDING_TITLE_TOP,
            replay.title,
            replay.title.len(),
        );
        if let Some(subtitle) = replay.subtitle {
            self.ending_text(
                gpu,
                ENDING_TEXT_LEFT,
                ENDING_SUBTITLE_TOP,
                subtitle,
                subtitle.len(),
            );
        }
        if let Some(location) = replay.location {
            self.ending_text(
                gpu,
                ENDING_TEXT_LEFT,
                ENDING_LOCATION_TOP,
                location,
                location.len(),
            );
        }
        if let Some(location) = replay.location_second_line {
            self.ending_text(
                gpu,
                ENDING_TEXT_LEFT,
                ENDING_LOCATION_SECOND_TOP,
                location,
                location.len(),
            );
        }

        let mut characters_remaining = usize::from(replay.detail_characters_visible);
        for (row, detail) in replay.details.iter().enumerate() {
            let characters = characters_remaining.min(detail.len());
            self.ending_text(
                gpu,
                ENDING_TEXT_LEFT,
                ENDING_DETAILS_TOP + row as i32 * crate::ending::GLYPH_SIZE as i32,
                detail,
                characters,
            );
            characters_remaining = characters_remaining.saturating_sub(detail.len());
        }
    }

    /// Font wrapper that applies the same centering offset.
    #[allow(clippy::too_many_arguments)]
    fn text_snes(
        &self,
        gpu: &mut Gpu,
        font: &mut Font,
        x: i32,
        y: i32,
        s: &str,
        r: f32,
        g: f32,
        b: f32,
    ) {
        font.set_screen_size(self.scr_w, self.scr_h);
        font.draw_string(gpu, x + self.ox, y, s, r, g, b);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_centered(
        &self,
        gpu: &mut Gpu,
        font: &mut Font,
        cx: i32,
        y: i32,
        s: &str,
        r: f32,
        g: f32,
        b: f32,
    ) {
        let len = s.len() as i32;
        self.text_snes(gpu, font, cx - len * 4, y, s, r, g, b);
    }

    /// Title screen UI: blinking "PRESS START" prompt only on the fallback
    /// backdrop (the composed SNES title layer already has "PUSH START").
    fn render_title(&self, gpu: &mut Gpu, font: &mut Font, bg2d: &Bg2d) {
        if !bg2d.has_title() {
            self.text_centered(gpu, font, 128, 170, "STAR FOX", 1.0, 0.85, 0.3);
            if (self.frame / 32) & 1 != 0 {
                self.text_centered(gpu, font, 128, 96, "PRESS START", 1.0, 1.0, 1.0);
            }
        }
    }

    pub(crate) fn render_sf2_mission_background(
        &self,
        gpu: &mut Gpu,
        backdrop: Sf2MissionBackdrop,
    ) {
        let texture = match backdrop {
            Sf2MissionBackdrop::DeepSpace => self.sf2_deep_space_backdrop,
            Sf2MissionBackdrop::EladardSurface => self.sf2_eladard_surface_backdrop,
            Sf2MissionBackdrop::EladardInterior => self.sf2_eladard_interior_backdrop,
            Sf2MissionBackdrop::TitaniaBase => self.sf2_titania_backdrop,
            Sf2MissionBackdrop::CarrierInterior => self.sf2_carrier_backdrop,
            Sf2MissionBackdrop::AstropolisVoid => self.sf2_astropolis_void_backdrop,
        };
        self.textured_quad_source_frame(
            gpu,
            texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    fn render_sf2_title_track(
        &mut self,
        gpu: &mut Gpu,
        track: crate::sf2_title::Track,
        mode_frame: u32,
    ) {
        let frame_index = crate::sf2_title::frame_at_tick(track, mode_frame);
        let key = Sf2TitleRenderKey { track, frame_index };
        if self.sf2_title_render_key != Some(key) {
            let rgba = self.sf2_title_presentation.frame_rgba(track, frame_index);
            gpu.update_texture(self.sf2_title_texture, &rgba);
            self.sf2_title_render_key = Some(key);
        }
        self.textured_quad_source_frame(
            gpu,
            self.sf2_title_texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    fn render_sf2_title(&mut self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let track = match inputs.title_page {
            Sf2TitlePage::MainMenu => match inputs.title_menu_item {
                Sf2TitleMenuItem::Mission => crate::sf2_title::Track::Mission,
                Sf2TitleMenuItem::Records => crate::sf2_title::Track::RecordsMenu,
                Sf2TitleMenuItem::SoundMode => match inputs.audio_output {
                    Sf2AudioOutput::Stereo => crate::sf2_title::Track::Stereo,
                    Sf2AudioOutput::Mono => crate::sf2_title::Track::Mono,
                },
            },
            Sf2TitlePage::Difficulty => match inputs.difficulty {
                Sf2Difficulty::Normal => crate::sf2_title::Track::Normal,
                Sf2Difficulty::Hard => crate::sf2_title::Track::Hard,
                Sf2Difficulty::Expert => crate::sf2_title::Track::Expert,
            },
        };
        self.render_sf2_title_track(gpu, track, inputs.mode_frame);
    }

    fn render_sf2_records(&mut self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        self.render_sf2_title_track(
            gpu,
            crate::sf2_title::Track::RecordsScreen,
            inputs.mode_frame,
        );
    }

    fn render_sf2_game_over(&mut self, gpu: &mut Gpu, _font: &mut Font, inputs: &Sf2FrameInputs) {
        let prompt_track = match inputs.game_over_choice {
            Sf2GameOverChoice::ContinueWithWingmate => crate::sf2_game_over::Track::PromptYes,
            Sf2GameOverChoice::EndCampaign => crate::sf2_game_over::Track::PromptNo,
        };
        let (track, frame_index) =
            crate::sf2_game_over::frame_at_mode_tick(inputs.mode_frame, prompt_track);
        let portrait = (inputs.mode_frame >= crate::sf2_game_over::PILOT_PORTRAIT_REVEAL_TICK)
            .then_some(match inputs.wingmate {
                Some(Sf2Pilot::Fox) => crate::sf2_game_over::Portrait::Fox,
                Some(Sf2Pilot::Falco) => crate::sf2_game_over::Portrait::Falco,
                Some(Sf2Pilot::Peppy) => crate::sf2_game_over::Portrait::Peppy,
                Some(Sf2Pilot::Slippy) => crate::sf2_game_over::Portrait::Slippy,
                Some(Sf2Pilot::Miyu) => crate::sf2_game_over::Portrait::Miyu,
                Some(Sf2Pilot::Fay) => crate::sf2_game_over::Portrait::Fay,
                None => crate::sf2_game_over::Portrait::None,
            });
        let brightness = if inputs.game_over_phase != Sf2GameOverPhase::Leaving {
            crate::sf2_game_over::Brightness::Full
        } else {
            sf2_game_over_brightness(
                inputs.game_over_choice,
                inputs.game_over_transition_retail_frames,
            )
        };
        let key = Sf2GameOverRenderKey {
            track,
            frame_index,
            portrait,
            brightness,
        };
        if self.sf2_game_over_render_key != Some(key) {
            let rgba = self.sf2_game_over_presentation.frame_rgba(
                track,
                frame_index,
                portrait,
                brightness,
            );
            gpu.update_texture(self.sf2_game_over_texture, &rgba);
            self.sf2_game_over_render_key = Some(key);
        }
        self.textured_quad_source_frame(
            gpu,
            self.sf2_game_over_texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    fn render_sf2_results(&mut self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let (track, frame_index) = match (inputs.results_phase, inputs.results_choice) {
            (Sf2ResultsPhase::Revealing, _) => (
                crate::sf2_results::Track::Reveal,
                crate::sf2_results::reveal_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
            (Sf2ResultsPhase::OpeningChoices, _) => (
                crate::sf2_results::Track::Opening,
                crate::sf2_results::opening_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
            (Sf2ResultsPhase::Choosing, Sf2ResultsChoice::Retry) => (
                crate::sf2_results::Track::RetryChoice,
                crate::sf2_results::choice_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
            (Sf2ResultsPhase::Choosing, Sf2ResultsChoice::Title) => (
                crate::sf2_results::Track::TitleChoice,
                crate::sf2_results::choice_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
            (Sf2ResultsPhase::Leaving, Sf2ResultsChoice::Retry) => (
                crate::sf2_results::Track::RetryLeaving,
                crate::sf2_results::leaving_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
            (Sf2ResultsPhase::Leaving, Sf2ResultsChoice::Title) => (
                crate::sf2_results::Track::TitleLeaving,
                crate::sf2_results::leaving_frame_at_retail_frame(
                    inputs.results_presentation_retail_frames,
                ),
            ),
        };
        let brightness = sf2_results_brightness(
            inputs.results_phase,
            inputs.results_choice,
            inputs.results_transition_retail_frames,
        );
        let key = Sf2ResultsRenderKey {
            track,
            frame_index,
            brightness,
        };
        if self.sf2_results_render_key != Some(key) {
            let rgba = self.sf2_results_presentation.frame_rgba(
                track,
                frame_index,
                brightness,
            );
            gpu.update_texture(self.sf2_results_texture, &rgba);
            self.sf2_results_render_key = Some(key);
        }
        self.textured_quad_source_frame(
            gpu,
            self.sf2_results_texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    fn render_sf2_briefing(&mut self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let frame_index = crate::sf2_briefing::frame_at_tick(inputs.mode_frame);
        if self.sf2_briefing_render_frame != Some(frame_index) {
            let rgba = self.sf2_briefing_presentation.frame_rgba(frame_index);
            gpu.update_texture(self.sf2_briefing_texture, &rgba);
            self.sf2_briefing_render_frame = Some(frame_index);
        }
        self.textured_quad_source_frame(
            gpu,
            self.sf2_briefing_texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    const fn sf2_presentation_pilot(pilot: Sf2Pilot) -> crate::sf2_pilot_selection::Pilot {
        match pilot {
            Sf2Pilot::Fox => crate::sf2_pilot_selection::Pilot::Fox,
            Sf2Pilot::Falco => crate::sf2_pilot_selection::Pilot::Falco,
            Sf2Pilot::Peppy => crate::sf2_pilot_selection::Pilot::Peppy,
            Sf2Pilot::Slippy => crate::sf2_pilot_selection::Pilot::Slippy,
            Sf2Pilot::Miyu => crate::sf2_pilot_selection::Pilot::Miyu,
            Sf2Pilot::Fay => crate::sf2_pilot_selection::Pilot::Fay,
        }
    }

    fn render_sf2_pilot_selection(&mut self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        use crate::sf2_pilot_selection::{PrimaryView, Screen};

        let cursor_pilot = match inputs.pilot_selection_cursor {
            Sf2PilotSelectionCursor::Pilot(pilot) => Self::sf2_presentation_pilot(pilot),
            Sf2PilotSelectionCursor::Control => crate::sf2_pilot_selection::Pilot::Fox,
        };
        let primary = inputs
            .primary_pilot
            .map(Self::sf2_presentation_pilot)
            .unwrap_or(cursor_pilot);
        let wingmate = inputs
            .wingmate
            .map(Self::sf2_presentation_pilot)
            .unwrap_or(cursor_pilot);
        let screen = match inputs.pilot_selection_phase {
            Sf2PilotSelectionPhase::Revealing => Screen::Reveal,
            Sf2PilotSelectionPhase::ChoosingPrimary => match inputs.pilot_selection_cursor {
                Sf2PilotSelectionCursor::Pilot(pilot) => {
                    Screen::Primary(PrimaryView::Pilot(Self::sf2_presentation_pilot(pilot)))
                }
                Sf2PilotSelectionCursor::Control => {
                    Screen::Primary(match inputs.flight_control_style {
                        Sf2FlightControlStyle::TypeA => PrimaryView::ControlA,
                        Sf2FlightControlStyle::TypeB => PrimaryView::ControlB,
                    })
                }
            },
            Sf2PilotSelectionPhase::ChoosingWingmate => Screen::Wingmate {
                primary,
                cursor: cursor_pilot,
            },
            Sf2PilotSelectionPhase::Ready => Screen::Ready { primary, wingmate },
            Sf2PilotSelectionPhase::Launching => Screen::Launch { primary, wingmate },
        };
        let frame_index = crate::sf2_pilot_selection::frame_at_tick(screen, inputs.mode_frame);
        let key = Sf2PilotSelectionRenderKey {
            screen,
            frame_index,
        };
        if self.sf2_pilot_selection_render_key != Some(key) {
            let rgba = self
                .sf2_pilot_selection_presentation
                .frame_rgba(screen, frame_index);
            gpu.update_texture(self.sf2_pilot_selection_texture, &rgba);
            self.sf2_pilot_selection_render_key = Some(key);
        }
        self.textured_quad_source_frame(
            gpu,
            self.sf2_pilot_selection_texture,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
    }

    fn render_sf2_strategic_map(
        &mut self,
        gpu: &mut Gpu,
        _font: &mut Font,
        inputs: &Sf2FrameInputs,
    ) {
        if inputs.strategic_phase == Sf2StrategicPhase::Overview {
            let frame_index = crate::sf2_opening_overview::frame_at_tick(
                inputs.strategic_opening_presentation_tick,
            );
            if self.sf2_opening_overview_render_frame != Some(frame_index) {
                let rgba = self
                    .sf2_opening_overview_presentation
                    .frame_rgba(frame_index);
                gpu.update_texture(self.sf2_opening_overview_texture, &rgba);
                self.sf2_opening_overview_render_frame = Some(frame_index);
            }
            self.textured_quad_source_frame(
                gpu,
                self.sf2_opening_overview_texture,
                0,
                0,
                SF2_REFERENCE_WIDTH,
                SF2_REFERENCE_HEIGHT,
            );
            return;
        }

        let backdrop = match inputs.campaign_sorties_completed {
            0 | 1 => self.sf2_strategic_map,
            2 => self.sf2_strategic_map_escalated,
            3 => self.sf2_strategic_map_post_interception,
            4 => self.sf2_strategic_map_post_fighter_intercept,
            5 => self.sf2_strategic_map_post_pigma,
            6 => self.sf2_strategic_map_post_eladard,
            7 => self.sf2_strategic_map_post_carrier,
            8 => self.sf2_strategic_map_post_leon,
            _ => self.sf2_strategic_map_post_mirage,
        };
        self.textured_quad_source_frame(
            gpu,
            backdrop,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            SF2_REFERENCE_HEIGHT,
        );
        self.render_sf2_strategic_actors(gpu, inputs);
        self.render_sf2_strategic_counters(gpu, inputs);
        self.render_sf2_strategic_sprites(gpu, inputs);
    }

    const fn sf2_strategic_actor_atlas_left(actor: Sf2StrategicActor) -> i32 {
        use Sf2StrategicActorAppearance::{
            EscalatedAssault, OpeningAssault, PostEladard, PostFighterIntercept, PostInterception,
            PostPigma,
        };
        use Sf2StrategicActorKind::{
            AttackingFighter, DefensePlatform, EasternInterceptor, EnemyCarrier, EnemyFormation,
            FighterProjectile, Missile, MissileTrail, NorthernInstallation, PatrolShip,
            RivalFighter, SouthernInstallation, UnknownSignal,
        };

        match (actor.kind, actor.appearance) {
            (NorthernInstallation, OpeningAssault) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_OPENING_LEFT
            }
            (NorthernInstallation, EscalatedAssault) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_ESCALATED_LEFT
            }
            (NorthernInstallation, PostInterception) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_POST_INTERCEPTION_LEFT
            }
            (NorthernInstallation, PostFighterIntercept) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT
            }
            (NorthernInstallation, PostPigma) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_POST_PIGMA_LEFT
            }
            (NorthernInstallation, PostEladard) => {
                crate::sf2_map_sprites::NORTH_INSTALLATION_POST_ELADARD_LEFT
            }
            (SouthernInstallation, OpeningAssault) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_OPENING_LEFT
            }
            (SouthernInstallation, EscalatedAssault) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_ESCALATED_LEFT
            }
            (SouthernInstallation, PostInterception) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_POST_INTERCEPTION_LEFT
            }
            (SouthernInstallation, PostFighterIntercept) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_POST_FIGHTER_INTERCEPT_LEFT
            }
            (SouthernInstallation, PostPigma) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_POST_PIGMA_LEFT
            }
            (SouthernInstallation, PostEladard) => {
                crate::sf2_map_sprites::SOUTH_INSTALLATION_POST_ELADARD_LEFT
            }
            (EnemyCarrier, OpeningAssault) => crate::sf2_map_sprites::ENEMY_CARRIER_OPENING_LEFT,
            (EnemyCarrier, EscalatedAssault) => {
                crate::sf2_map_sprites::ENEMY_CARRIER_ESCALATED_LEFT
            }
            (EnemyCarrier, PostInterception) => {
                crate::sf2_map_sprites::ENEMY_CARRIER_POST_INTERCEPTION_LEFT
            }
            (EnemyCarrier, PostFighterIntercept) => {
                crate::sf2_map_sprites::ENEMY_CARRIER_POST_FIGHTER_INTERCEPT_LEFT
            }
            (EnemyCarrier, PostPigma) => crate::sf2_map_sprites::ENEMY_CARRIER_POST_PIGMA_LEFT,
            (EnemyCarrier, PostEladard) => crate::sf2_map_sprites::ENEMY_CARRIER_POST_ELADARD_LEFT,
            (EnemyFormation, OpeningAssault) => {
                crate::sf2_map_sprites::ENEMY_FORMATION_OPENING_LEFT
            }
            (EnemyFormation, EscalatedAssault) => {
                crate::sf2_map_sprites::ENEMY_FORMATION_ESCALATED_LEFT
            }
            (EnemyFormation, PostInterception) => {
                crate::sf2_map_sprites::ENEMY_FORMATION_POST_INTERCEPTION_LEFT
            }
            (EnemyFormation, PostFighterIntercept) => {
                crate::sf2_map_sprites::ENEMY_FORMATION_POST_FIGHTER_INTERCEPT_LEFT
            }
            (EnemyFormation, PostPigma) => crate::sf2_map_sprites::ENEMY_FORMATION_POST_PIGMA_LEFT,
            (EnemyFormation, PostEladard) => {
                crate::sf2_map_sprites::ENEMY_FORMATION_POST_ELADARD_LEFT
            }
            (EasternInterceptor, OpeningAssault) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_OPENING_LEFT
            }
            (EasternInterceptor, EscalatedAssault) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_ESCALATED_LEFT
            }
            (EasternInterceptor, PostInterception) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_POST_INTERCEPTION_LEFT
            }
            (EasternInterceptor, PostFighterIntercept) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_POST_FIGHTER_INTERCEPT_LEFT
            }
            (EasternInterceptor, PostPigma) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_POST_PIGMA_LEFT
            }
            (EasternInterceptor, PostEladard) => {
                crate::sf2_map_sprites::EAST_INTERCEPTOR_POST_ELADARD_LEFT
            }
            (PatrolShip, PostEladard) => crate::sf2_map_sprites::PATROL_SHIP_POST_ELADARD_LEFT,
            (PatrolShip, PostPigma) => crate::sf2_map_sprites::PATROL_SHIP_POST_PIGMA_LEFT,
            (PatrolShip, _) => crate::sf2_map_sprites::PATROL_SHIP_LEFT,
            (MissileTrail, _) => crate::sf2_map_sprites::MISSILE_TRAIL_OPENING_LEFT,
            (Missile, OpeningAssault) => crate::sf2_map_sprites::MISSILE_OPENING_LEFT,
            (
                Missile,
                EscalatedAssault | PostInterception | PostFighterIntercept | PostPigma
                | PostEladard,
            ) => crate::sf2_map_sprites::MISSILE_ESCALATED_LEFT,
            (AttackingFighter, PostInterception) => {
                crate::sf2_map_sprites::ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT
            }
            (AttackingFighter, PostEladard) => {
                crate::sf2_map_sprites::ATTACKING_FIGHTER_POST_ELADARD_LEFT
            }
            (AttackingFighter, OpeningAssault | EscalatedAssault | PostPigma) => {
                crate::sf2_map_sprites::ATTACKING_FIGHTER_POST_INTERCEPTION_LEFT
            }
            (AttackingFighter, PostFighterIntercept) => {
                crate::sf2_map_sprites::ATTACKING_FIGHTER_POST_FIGHTER_INTERCEPT_LEFT
            }
            (RivalFighter, PostPigma) => crate::sf2_map_sprites::RIVAL_FIGHTER_POST_PIGMA_LEFT,
            (RivalFighter, _) => crate::sf2_map_sprites::RIVAL_FIGHTER_POST_PIGMA_LEFT,
            (FighterProjectile, PostFighterIntercept) => {
                crate::sf2_map_sprites::FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT
            }
            (FighterProjectile, PostPigma) => {
                crate::sf2_map_sprites::FIGHTER_PROJECTILE_POST_PIGMA_LEFT
            }
            (FighterProjectile, PostEladard) => {
                crate::sf2_map_sprites::FIGHTER_PROJECTILE_POST_ELADARD_LEFT
            }
            (FighterProjectile, OpeningAssault | EscalatedAssault | PostInterception) => {
                crate::sf2_map_sprites::FIGHTER_PROJECTILE_POST_FIGHTER_INTERCEPT_LEFT
            }
            (UnknownSignal, _) => crate::sf2_map_sprites::UNKNOWN_SIGNAL_POST_ELADARD_LEFT,
            (DefensePlatform, _) => crate::sf2_map_sprites::UNKNOWN_SIGNAL_POST_ELADARD_LEFT,
            (
                _,
                Sf2StrategicActorAppearance::PostCarrier
                | Sf2StrategicActorAppearance::PostLeon
                | Sf2StrategicActorAppearance::PostMirage,
            ) => 0,
        }
    }

    const fn sf2_post_carrier_actor_atlas_left(actor: Sf2StrategicActor) -> i32 {
        use Sf2StrategicActorKind::{
            AttackingFighter, DefensePlatform, EasternInterceptor, EnemyCarrier, EnemyFormation,
            FighterProjectile, Missile, MissileTrail, NorthernInstallation, PatrolShip,
            RivalFighter, SouthernInstallation, UnknownSignal,
        };

        match actor.kind {
            NorthernInstallation => crate::sf2_map_post_carrier_sprites::NORTH_INSTALLATION_LEFT,
            SouthernInstallation => crate::sf2_map_post_carrier_sprites::SOUTH_INSTALLATION_LEFT,
            EnemyCarrier => crate::sf2_map_post_carrier_sprites::DISTANT_CARRIER_LEFT,
            EnemyFormation => crate::sf2_map_post_carrier_sprites::ENEMY_FORMATION_LEFT,
            AttackingFighter => crate::sf2_map_post_carrier_sprites::ATTACKING_FIGHTER_LEFT,
            PatrolShip => crate::sf2_map_post_carrier_sprites::PATROL_SHIP_LEFT,
            UnknownSignal => crate::sf2_map_post_carrier_sprites::UNKNOWN_SIGNAL_LEFT,
            MissileTrail => crate::sf2_map_post_carrier_sprites::MISSILE_TRAIL_LEFT,
            Missile => crate::sf2_map_post_carrier_sprites::MISSILE_LEFT,
            FighterProjectile => crate::sf2_map_post_carrier_sprites::FIGHTER_PROJECTILE_LEFT,
            EasternInterceptor | RivalFighter | DefensePlatform => {
                crate::sf2_map_post_carrier_sprites::UNKNOWN_SIGNAL_LEFT
            }
        }
    }

    const fn sf2_post_leon_actor_atlas_left(actor: Sf2StrategicActor) -> i32 {
        use Sf2StrategicActorKind::{
            AttackingFighter, DefensePlatform, EasternInterceptor, EnemyCarrier, EnemyFormation,
            FighterProjectile, Missile, MissileTrail, NorthernInstallation, PatrolShip,
            RivalFighter, SouthernInstallation, UnknownSignal,
        };

        match actor.kind {
            NorthernInstallation => crate::sf2_map_post_leon_sprites::NORTH_INSTALLATION_LEFT,
            SouthernInstallation => crate::sf2_map_post_leon_sprites::SOUTH_INSTALLATION_LEFT,
            EnemyCarrier => crate::sf2_map_post_leon_sprites::DISTANT_CARRIER_LEFT,
            EnemyFormation => crate::sf2_map_post_leon_sprites::ENEMY_FORMATION_LEFT,
            AttackingFighter => crate::sf2_map_post_leon_sprites::ATTACKING_FIGHTER_LEFT,
            PatrolShip => crate::sf2_map_post_leon_sprites::PATROL_SHIP_LEFT,
            UnknownSignal => crate::sf2_map_post_leon_sprites::UNKNOWN_SIGNAL_LEFT,
            MissileTrail => crate::sf2_map_post_leon_sprites::MISSILE_TRAIL_LEFT,
            Missile => crate::sf2_map_post_leon_sprites::MISSILE_LEFT,
            FighterProjectile => crate::sf2_map_post_leon_sprites::FIGHTER_PROJECTILE_LEFT,
            EasternInterceptor | RivalFighter | DefensePlatform => {
                crate::sf2_map_post_leon_sprites::UNKNOWN_SIGNAL_LEFT
            }
        }
    }

    const fn sf2_post_mirage_actor_atlas_left(actor: Sf2StrategicActor) -> i32 {
        use Sf2StrategicActorKind::{
            AttackingFighter, DefensePlatform, EasternInterceptor, EnemyCarrier, EnemyFormation,
            FighterProjectile, Missile, MissileTrail, NorthernInstallation, PatrolShip,
            RivalFighter, SouthernInstallation, UnknownSignal,
        };

        match actor.kind {
            NorthernInstallation => crate::sf2_map_post_mirage_sprites::NORTH_INSTALLATION_LEFT,
            SouthernInstallation => crate::sf2_map_post_mirage_sprites::SOUTH_INSTALLATION_LEFT,
            EnemyCarrier => crate::sf2_map_post_mirage_sprites::DISTANT_CARRIER_LEFT,
            EnemyFormation => crate::sf2_map_post_mirage_sprites::ENEMY_FORMATION_LEFT,
            AttackingFighter => crate::sf2_map_post_mirage_sprites::ATTACKING_FIGHTER_LEFT,
            PatrolShip => crate::sf2_map_post_mirage_sprites::PATROL_SHIP_LEFT,
            DefensePlatform => crate::sf2_map_post_mirage_sprites::DEFENSE_PLATFORM_LEFT,
            UnknownSignal => crate::sf2_map_post_mirage_sprites::UNKNOWN_SIGNAL_LEFT,
            FighterProjectile => crate::sf2_map_post_mirage_sprites::FIGHTER_PROJECTILE_LEFT,
            EasternInterceptor | RivalFighter | MissileTrail | Missile => {
                crate::sf2_map_post_mirage_sprites::DEFENSE_PLATFORM_LEFT
            }
        }
    }

    fn render_sf2_strategic_actors(&self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        for actor in inputs.strategic_actors.into_iter().flatten().rev() {
            let post_carrier = actor.appearance == Sf2StrategicActorAppearance::PostCarrier;
            let post_leon = actor.appearance == Sf2StrategicActorAppearance::PostLeon;
            let post_mirage = actor.appearance == Sf2StrategicActorAppearance::PostMirage;
            let late_campaign = post_carrier || post_leon || post_mirage;
            let cell_size = if post_mirage {
                crate::sf2_map_post_mirage_sprites::CELL_SIZE
            } else if post_leon {
                crate::sf2_map_post_leon_sprites::CELL_SIZE
            } else if post_carrier {
                crate::sf2_map_post_carrier_sprites::CELL_SIZE
            } else {
                match actor.appearance {
                    Sf2StrategicActorAppearance::PostPigma => {
                        crate::sf2_map_sprites::POST_PIGMA_ACTOR_CELL_SIZE
                    }
                    Sf2StrategicActorAppearance::PostEladard => {
                        crate::sf2_map_sprites::POST_ELADARD_ACTOR_CELL_SIZE
                    }
                    _ => crate::sf2_map_sprites::ACTOR_CELL_SIZE,
                }
            };
            self.textured_quad_source_region(
                gpu,
                if post_mirage {
                    self.sf2_map_post_mirage_sprites
                } else if post_leon {
                    self.sf2_map_post_leon_sprites
                } else if post_carrier {
                    self.sf2_map_post_carrier_sprites
                } else {
                    self.sf2_map_sprites
                },
                i32::from(actor.position.x),
                i32::from(actor.position.y),
                cell_size,
                cell_size,
                if post_mirage {
                    Self::sf2_post_mirage_actor_atlas_left(actor)
                } else if post_leon {
                    Self::sf2_post_leon_actor_atlas_left(actor)
                } else if post_carrier {
                    Self::sf2_post_carrier_actor_atlas_left(actor)
                } else {
                    Self::sf2_strategic_actor_atlas_left(actor)
                },
                if late_campaign {
                    0
                } else {
                    crate::sf2_map_sprites::ACTOR_TOP
                },
                if post_mirage {
                    crate::sf2_map_post_mirage_sprites::WIDTH as i32
                } else if post_leon {
                    crate::sf2_map_post_leon_sprites::WIDTH as i32
                } else if post_carrier {
                    crate::sf2_map_post_carrier_sprites::WIDTH as i32
                } else {
                    crate::sf2_map_sprites::WIDTH as i32
                },
                if post_mirage {
                    crate::sf2_map_post_mirage_sprites::HEIGHT as i32
                } else if post_leon {
                    crate::sf2_map_post_leon_sprites::HEIGHT as i32
                } else if post_carrier {
                    crate::sf2_map_post_carrier_sprites::HEIGHT as i32
                } else {
                    crate::sf2_map_sprites::HEIGHT as i32
                },
            );
        }
    }

    fn render_sf2_map_sprite(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            self.sf2_map_sprites,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_map_sprites::WIDTH as i32,
            crate::sf2_map_sprites::HEIGHT as i32,
        );
    }

    fn render_sf2_post_carrier_map_sprite(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            self.sf2_map_post_carrier_sprites,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_map_post_carrier_sprites::WIDTH as i32,
            crate::sf2_map_post_carrier_sprites::HEIGHT as i32,
        );
    }

    fn render_sf2_post_leon_map_sprite(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            self.sf2_map_post_leon_sprites,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_map_post_leon_sprites::WIDTH as i32,
            crate::sf2_map_post_leon_sprites::HEIGHT as i32,
        );
    }

    fn render_sf2_post_mirage_map_sprite(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            self.sf2_map_post_mirage_sprites,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_map_post_mirage_sprites::WIDTH as i32,
            crate::sf2_map_post_mirage_sprites::HEIGHT as i32,
        );
    }

    const fn sf2_map_pilot_atlas_left(pilot: Sf2Pilot, post_eladard: bool) -> i32 {
        let index = match pilot {
            Sf2Pilot::Fox => 0,
            Sf2Pilot::Falco => 1,
            Sf2Pilot::Peppy => 2,
            Sf2Pilot::Slippy => 3,
            Sf2Pilot::Miyu => 4,
            Sf2Pilot::Fay => 5,
        };
        let atlas_left = if post_eladard {
            crate::sf2_map_sprites::PILOTS_POST_ELADARD_LEFT
        } else {
            crate::sf2_map_sprites::PILOTS_LEFT
        };
        atlas_left + index * crate::sf2_map_sprites::PILOT_SIZE
    }

    fn render_sf2_map_pilot(
        &self,
        gpu: &mut Gpu,
        pilot: Option<Sf2Pilot>,
        destination_left: i32,
        post_eladard: bool,
    ) {
        let Some(pilot) = pilot else {
            return;
        };
        self.render_sf2_map_sprite(
            gpu,
            destination_left,
            SF2_MAP_PILOT_TOP,
            crate::sf2_map_sprites::PILOT_SIZE,
            crate::sf2_map_sprites::PILOT_SIZE,
            Self::sf2_map_pilot_atlas_left(pilot, post_eladard),
        );
    }

    fn render_sf2_map_shield_row(&self, gpu: &mut Gpu, shield: u8, top: i32, post_eladard: bool) {
        let pip_count = if post_eladard {
            SF2_MAP_SHIELD_PIPS_PER_PILOT + 1
        } else {
            SF2_MAP_SHIELD_PIPS_PER_PILOT
        };
        let filled = usize::from(shield)
            .div_ceil(SF2_MAP_SHIELD_PER_PIP)
            .min(pip_count);
        for pip in 0..pip_count {
            let atlas_left = if post_eladard && pip < filled {
                crate::sf2_map_sprites::SHIELD_FULL_POST_ELADARD_LEFT
            } else if post_eladard {
                crate::sf2_map_sprites::SHIELD_EMPTY_POST_ELADARD_LEFT
            } else if pip < filled {
                crate::sf2_map_sprites::SHIELD_FULL_LEFT
            } else {
                crate::sf2_map_sprites::SHIELD_EMPTY_LEFT
            };
            self.render_sf2_map_sprite(
                gpu,
                if post_eladard {
                    SF2_MAP_POST_ELADARD_SHIELD_LEFT
                } else {
                    SF2_MAP_SHIELD_LEFT
                } + pip as i32 * crate::sf2_hud_glyphs::GLYPH_SIZE,
                top,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                atlas_left,
            );
        }
    }

    fn render_sf2_strategic_sprites(&self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let post_eladard = inputs.campaign_sorties_completed >= 6;
        let post_carrier = inputs.campaign_sorties_completed >= 7;
        let post_leon = inputs.campaign_sorties_completed >= 8;
        let post_mirage = inputs.campaign_sorties_completed >= 9;
        self.render_sf2_map_pilot(
            gpu,
            inputs.primary_pilot,
            SF2_MAP_PRIMARY_PILOT_LEFT,
            post_eladard,
        );
        self.render_sf2_map_pilot(gpu, inputs.wingmate, SF2_MAP_WINGMATE_LEFT, post_eladard);
        if post_leon {
            if post_mirage {
                self.render_sf2_post_mirage_map_sprite(
                    gpu,
                    SF2_MAP_WINGMATE_LEFT,
                    SF2_MAP_PILOT_TOP,
                    crate::sf2_map_sprites::PILOT_SIZE,
                    crate::sf2_map_sprites::PILOT_SIZE,
                    crate::sf2_map_post_mirage_sprites::WINGMATE_PILOT_LEFT,
                );
            } else {
                self.render_sf2_post_leon_map_sprite(
                    gpu,
                    SF2_MAP_WINGMATE_LEFT,
                    SF2_MAP_PILOT_TOP,
                    crate::sf2_map_sprites::PILOT_SIZE,
                    crate::sf2_map_sprites::PILOT_SIZE,
                    crate::sf2_map_post_leon_sprites::WINGMATE_PILOT_LEFT,
                );
            }
        }
        if post_carrier {
            let render_late_sprite = |this: &Self,
                                      gpu: &mut Gpu,
                                      destination_left,
                                      destination_top,
                                      width,
                                      height,
                                      atlas_left| {
                if post_mirage {
                    this.render_sf2_post_mirage_map_sprite(
                        gpu,
                        destination_left,
                        destination_top,
                        width,
                        height,
                        atlas_left,
                    );
                } else if post_leon {
                    this.render_sf2_post_leon_map_sprite(
                        gpu,
                        destination_left,
                        destination_top,
                        width,
                        height,
                        atlas_left,
                    );
                } else {
                    this.render_sf2_post_carrier_map_sprite(
                        gpu,
                        destination_left,
                        destination_top,
                        width,
                        height,
                        atlas_left,
                    );
                }
            };
            render_late_sprite(
                self,
                gpu,
                SF2_MAP_POST_ELADARD_SHIELD_LEFT,
                SF2_MAP_PRIMARY_SHIELD_TOP,
                SF2_MAP_SHIELD_ROW_WIDTH,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                if post_mirage {
                    crate::sf2_map_post_mirage_sprites::PRIMARY_SHIELD_LEFT
                } else if post_leon {
                    crate::sf2_map_post_leon_sprites::PRIMARY_SHIELD_LEFT
                } else {
                    crate::sf2_map_post_carrier_sprites::PRIMARY_SHIELD_LEFT
                },
            );
            render_late_sprite(
                self,
                gpu,
                SF2_MAP_POST_ELADARD_SHIELD_LEFT,
                SF2_MAP_WINGMATE_SHIELD_TOP,
                SF2_MAP_SHIELD_ROW_WIDTH,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                if post_mirage {
                    crate::sf2_map_post_mirage_sprites::WINGMATE_SHIELD_LEFT
                } else if post_leon {
                    crate::sf2_map_post_leon_sprites::WINGMATE_SHIELD_LEFT
                } else {
                    crate::sf2_map_post_carrier_sprites::WINGMATE_SHIELD_LEFT
                },
            );
        } else {
            self.render_sf2_map_shield_row(
                gpu,
                inputs.primary_shield,
                SF2_MAP_PRIMARY_SHIELD_TOP,
                post_eladard,
            );
            self.render_sf2_map_shield_row(
                gpu,
                inputs.wingmate_shield,
                SF2_MAP_WINGMATE_SHIELD_TOP,
                post_eladard,
            );
        }

        if inputs.item_count > 0 {
            self.render_sf2_map_sprite(
                gpu,
                SF2_MAP_ITEM_ICON_LEFT,
                SF2_MAP_ITEM_ICON_TOP,
                crate::sf2_map_sprites::PILOT_SIZE,
                crate::sf2_map_sprites::PILOT_SIZE,
                if post_eladard {
                    crate::sf2_map_sprites::ITEM_ICON_POST_ELADARD_LEFT
                } else {
                    crate::sf2_map_sprites::ITEM_ICON_LEFT
                },
            );
        }
        if post_carrier {
            if post_mirage {
                self.render_sf2_post_mirage_map_sprite(
                    gpu,
                    SF2_MAP_GAUGE_LEFT,
                    SF2_MAP_GAUGE_TOP,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_hud_glyphs::GLYPH_SIZE * 2,
                    crate::sf2_map_post_mirage_sprites::GAUGE_BARS_LEFT,
                );
            } else if post_leon {
                self.render_sf2_post_leon_map_sprite(
                    gpu,
                    SF2_MAP_GAUGE_LEFT,
                    SF2_MAP_GAUGE_TOP,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_hud_glyphs::GLYPH_SIZE * 2,
                    crate::sf2_map_post_leon_sprites::GAUGE_BARS_LEFT,
                );
            } else {
                self.render_sf2_post_carrier_map_sprite(
                    gpu,
                    SF2_MAP_GAUGE_LEFT,
                    SF2_MAP_GAUGE_TOP,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_hud_glyphs::GLYPH_SIZE * 2,
                    crate::sf2_map_post_carrier_sprites::GAUGE_BARS_LEFT,
                );
            }
        } else {
            for (top, atlas_left) in [
                (SF2_MAP_GAUGE_TOP, crate::sf2_map_sprites::GAUGE_BAR_LEFT),
                (
                    SF2_MAP_GAUGE_TOP + crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_map_sprites::GAUGE_BAR_FLIPPED_LEFT,
                ),
            ] {
                self.render_sf2_map_sprite(
                    gpu,
                    SF2_MAP_GAUGE_LEFT,
                    top,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    atlas_left,
                );
            }
        }

        if inputs.primary_pilot.is_none() {
            return;
        }
        let craft_left = i32::from(inputs.strategic_player.x);
        let craft_top = i32::from(inputs.strategic_player.y);
        self.render_sf2_map_sprite(
            gpu,
            craft_left,
            craft_top,
            crate::sf2_map_sprites::PILOT_SIZE,
            crate::sf2_map_sprites::PILOT_SIZE,
            crate::sf2_map_sprites::CRAFT_BODY_LEFT,
        );
        if inputs.campaign_sorties_completed == 0 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_FIRST_CRAFT_ACCENT_X_OFFSET,
                craft_top + SF2_MAP_FIRST_CRAFT_ACCENT_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_ACCENT_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 1 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_FIRST_CRAFT_ACCENT_X_OFFSET,
                craft_top + SF2_MAP_FIRST_CRAFT_ACCENT_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_ACCENT_HORIZONTAL_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 2 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_SECOND_CRAFT_ACCENT_X_OFFSET,
                craft_top + SF2_MAP_SECOND_CRAFT_ACCENT_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_ACCENT_VERTICAL_LEFT,
            );
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_SECOND_CRAFT_CURSOR_X_OFFSET,
                craft_top + SF2_MAP_SECOND_CRAFT_CURSOR_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_CURSOR_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 3 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_POST_INTERCEPTION_CRAFT_CURSOR_X_OFFSET,
                craft_top + SF2_MAP_POST_INTERCEPTION_CRAFT_CURSOR_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_CURSOR_POST_INTERCEPTION_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 4 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_POST_FIGHTER_INTERCEPT_CRAFT_CURSOR_X_OFFSET,
                craft_top + SF2_MAP_POST_FIGHTER_INTERCEPT_CRAFT_CURSOR_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_CURSOR_POST_FIGHTER_INTERCEPT_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 5 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_POST_PIGMA_CRAFT_CURSOR_X_OFFSET,
                craft_top + SF2_MAP_POST_PIGMA_CRAFT_CURSOR_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_CURSOR_POST_PIGMA_LEFT,
            );
        } else if inputs.campaign_sorties_completed == 6 {
            self.render_sf2_map_sprite(
                gpu,
                craft_left + SF2_MAP_POST_ELADARD_CRAFT_CURSOR_X_OFFSET,
                craft_top + SF2_MAP_POST_ELADARD_CRAFT_CURSOR_Y_OFFSET,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_map_sprites::CRAFT_CURSOR_POST_ELADARD_LEFT,
            );
        } else {
            if inputs.campaign_sorties_completed >= 9 {
                self.render_sf2_post_mirage_map_sprite(
                    gpu,
                    craft_left + SF2_MAP_POST_MIRAGE_CRAFT_MARKER_X_OFFSET,
                    craft_top + SF2_MAP_POST_MIRAGE_CRAFT_MARKER_Y_OFFSET,
                    SF2_MAP_POST_MIRAGE_CRAFT_MARKER_WIDTH,
                    SF2_MAP_POST_MIRAGE_CRAFT_MARKER_HEIGHT,
                    crate::sf2_map_post_mirage_sprites::CRAFT_MARKER_LEFT,
                );
            } else if inputs.campaign_sorties_completed >= 8 {
                self.render_sf2_post_leon_map_sprite(
                    gpu,
                    craft_left + SF2_MAP_POST_LEON_CRAFT_MARKER_X_OFFSET,
                    craft_top + SF2_MAP_POST_LEON_CRAFT_MARKER_Y_OFFSET,
                    SF2_MAP_POST_LEON_CRAFT_MARKER_WIDTH,
                    SF2_MAP_POST_LEON_CRAFT_MARKER_HEIGHT,
                    crate::sf2_map_post_leon_sprites::CRAFT_MARKER_LEFT,
                );
            } else {
                self.render_sf2_post_carrier_map_sprite(
                    gpu,
                    craft_left + SF2_MAP_POST_CARRIER_CRAFT_CURSOR_X_OFFSET,
                    craft_top + SF2_MAP_POST_CARRIER_CRAFT_CURSOR_Y_OFFSET,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_hud_glyphs::GLYPH_SIZE,
                    crate::sf2_map_post_carrier_sprites::CRAFT_CURSOR_LEFT,
                );
            }
        }
    }

    fn clear_sf2_map_source_region(
        &self,
        gpu: &mut Gpu,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    ) {
        self.quad_snes(
            gpu,
            SF2_MAP_BACKGROUND_COLOR,
            left,
            SF2_REFERENCE_HEIGHT - top - height,
            width,
            height,
        );
    }

    fn render_sf2_map_atlas_region(
        &self,
        gpu: &mut Gpu,
        atlas: TextureId,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            atlas,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_map_glyphs::WIDTH as i32,
            crate::sf2_map_glyphs::HEIGHT as i32,
        );
    }

    fn render_sf2_map_digit(
        &self,
        gpu: &mut Gpu,
        atlas: TextureId,
        destination_left: i32,
        destination_top: i32,
        digit: u8,
        atlas_digits_left: i32,
    ) {
        let glyph_size = crate::sf2_map_glyphs::GLYPH_SIZE;
        self.render_sf2_map_atlas_region(
            gpu,
            atlas,
            destination_left,
            destination_top,
            glyph_size,
            glyph_size,
            atlas_digits_left + i32::from(digit) * glyph_size,
        );
    }

    fn render_sf2_strategic_counters(&self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let map_glyph_texture = if inputs.campaign_sorties_completed >= 9 {
            self.sf2_map_post_mirage_glyphs
        } else if inputs.campaign_sorties_completed >= 7 {
            self.sf2_map_post_carrier_glyphs
        } else if inputs.campaign_sorties_completed >= 6 {
            self.sf2_map_post_eladard_glyphs
        } else if inputs.campaign_sorties_completed >= 5 {
            self.sf2_map_post_pigma_glyphs
        } else if inputs.campaign_sorties_completed == 4 {
            self.sf2_map_post_fighter_intercept_glyphs
        } else if inputs.campaign_sorties_completed == 3 {
            self.sf2_map_post_interception_glyphs
        } else {
            self.sf2_map_glyphs
        };
        let damage_percent = inputs
            .corneria_damage_percent
            .min(SF2_MAP_MAX_DAMAGE_PERCENT);
        let damage_digits = [damage_percent / 10, damage_percent % 10];
        let (
            damage_texture,
            damage_digit_width,
            damage_digit_height,
            damage_texture_width,
            damage_texture_height,
        ) = if inputs.campaign_sorties_completed >= 9
            && damage_percent < SF2_MAP_DAMAGE_WARNING_PERCENT
        {
            (
                self.sf2_map_damage_glyphs,
                crate::sf2_map_damage_glyphs::DIGIT_WIDTH,
                crate::sf2_map_damage_glyphs::DIGIT_HEIGHT,
                crate::sf2_map_damage_glyphs::WIDTH as i32,
                crate::sf2_map_damage_glyphs::HEIGHT as i32,
            )
        } else if inputs.campaign_sorties_completed >= 6 {
            (
                self.sf2_map_damage_post_eladard_glyphs,
                crate::sf2_map_damage_post_eladard_glyphs::DIGIT_WIDTH,
                crate::sf2_map_damage_post_eladard_glyphs::DIGIT_HEIGHT,
                crate::sf2_map_damage_post_eladard_glyphs::WIDTH as i32,
                crate::sf2_map_damage_post_eladard_glyphs::HEIGHT as i32,
            )
        } else if damage_percent >= SF2_MAP_DAMAGE_WARNING_PERCENT {
            (
                self.sf2_map_damage_warning_glyphs,
                crate::sf2_map_damage_warning_glyphs::DIGIT_WIDTH,
                crate::sf2_map_damage_warning_glyphs::DIGIT_HEIGHT,
                crate::sf2_map_damage_warning_glyphs::WIDTH as i32,
                crate::sf2_map_damage_warning_glyphs::HEIGHT as i32,
            )
        } else {
            (
                self.sf2_map_damage_glyphs,
                crate::sf2_map_damage_glyphs::DIGIT_WIDTH,
                crate::sf2_map_damage_glyphs::DIGIT_HEIGHT,
                crate::sf2_map_damage_glyphs::WIDTH as i32,
                crate::sf2_map_damage_glyphs::HEIGHT as i32,
            )
        };
        self.quad_snes(
            gpu,
            SF2_MAP_DAMAGE_BACKGROUND_COLOR,
            SF2_MAP_DAMAGE_LEFT,
            SF2_REFERENCE_HEIGHT - SF2_MAP_DAMAGE_TOP - damage_digit_height,
            damage_digit_width * damage_digits.len() as i32,
            damage_digit_height,
        );
        for (index, digit) in damage_digits.into_iter().enumerate() {
            self.textured_quad_source_region(
                gpu,
                damage_texture,
                SF2_MAP_DAMAGE_LEFT + index as i32 * damage_digit_width,
                SF2_MAP_DAMAGE_TOP,
                damage_digit_width,
                damage_digit_height,
                i32::from(digit) * damage_digit_width,
                0,
                damage_texture_width,
                damage_texture_height,
            );
        }

        let glyph_size = crate::sf2_map_glyphs::GLYPH_SIZE;
        let elapsed_seconds = (inputs.elapsed_campaign_frames
            / SF2_CAMPAIGN_FRAMES_PER_DISPLAY_SECOND)
            .min(SF2_MAP_MAX_DISPLAY_SECONDS);
        let elapsed_digits = [
            (elapsed_seconds / 100) as u8,
            ((elapsed_seconds / 10) % 10) as u8,
            (elapsed_seconds % 10) as u8,
        ];
        self.clear_sf2_map_source_region(
            gpu,
            SF2_MAP_TIME_LEFT,
            SF2_MAP_TIME_TOP,
            glyph_size * elapsed_digits.len() as i32,
            glyph_size,
        );
        for (index, digit) in elapsed_digits.into_iter().enumerate() {
            self.render_sf2_map_digit(
                gpu,
                map_glyph_texture,
                SF2_MAP_TIME_LEFT + index as i32 * glyph_size,
                SF2_MAP_TIME_TOP,
                digit,
                crate::sf2_map_glyphs::TIME_DIGITS_LEFT,
            );
        }

        let score = inputs.score.min(SF2_MAX_SCORE);
        self.clear_sf2_map_source_region(
            gpu,
            SF2_MAP_SCORE_LEFT,
            SF2_MAP_SCORE_TOP,
            glyph_size * SF2_MAP_SCORE_DIGIT_DIVISORS.len() as i32,
            glyph_size,
        );
        for (index, divisor) in SF2_MAP_SCORE_DIGIT_DIVISORS.into_iter().enumerate() {
            let digit = ((score / divisor) % 10) as u8;
            let digits_left = if inputs.campaign_sorties_completed >= 7 && index == 2 && digit == 0
            {
                crate::sf2_map_glyphs::POST_CARRIER_SCORE_ZERO_LEFT
            } else {
                crate::sf2_map_glyphs::SCORE_DIGITS_LEFT
            };
            self.render_sf2_map_digit(
                gpu,
                map_glyph_texture,
                SF2_MAP_SCORE_LEFT + index as i32 * glyph_size,
                SF2_MAP_SCORE_TOP,
                digit,
                digits_left,
            );
        }

        let item_count = inputs.item_count.min(SF2_MISSION_MAX_ITEM_COUNT);
        self.clear_sf2_map_source_region(
            gpu,
            SF2_MAP_ITEM_LEFT,
            SF2_MAP_ITEM_TOP,
            glyph_size,
            crate::sf2_map_glyphs::ITEM_DIGIT_HEIGHT,
        );
        self.render_sf2_map_atlas_region(
            gpu,
            map_glyph_texture,
            SF2_MAP_ITEM_LEFT,
            SF2_MAP_ITEM_TOP,
            glyph_size,
            crate::sf2_map_glyphs::ITEM_DIGIT_HEIGHT,
            crate::sf2_map_glyphs::ITEM_DIGITS_LEFT + i32::from(item_count) * glyph_size,
        );
    }

    fn render_sf2_hud_atlas_region(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        width: i32,
        height: i32,
        atlas_left: i32,
    ) {
        self.textured_quad_source_region(
            gpu,
            self.sf2_hud_glyphs,
            destination_left,
            destination_top,
            width,
            height,
            atlas_left,
            0,
            crate::sf2_hud_glyphs::WIDTH as i32,
            crate::sf2_hud_glyphs::HEIGHT as i32,
        );
    }

    fn render_sf2_hud_digit(
        &self,
        gpu: &mut Gpu,
        destination_left: i32,
        destination_top: i32,
        digit: u8,
        atlas_digits_left: i32,
    ) {
        let glyph_size = crate::sf2_hud_glyphs::GLYPH_SIZE;
        self.render_sf2_hud_atlas_region(
            gpu,
            destination_left,
            destination_top,
            glyph_size,
            glyph_size,
            atlas_digits_left + i32::from(digit) * glyph_size,
        );
    }

    fn render_sf2_mission_hud(&self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        self.textured_quad_source_frame(
            gpu,
            self.sf2_mission_hud,
            0,
            0,
            crate::sf2_mission_hud::WIDTH as i32,
            crate::sf2_mission_hud::HEIGHT as i32,
        );
        self.textured_quad_source_region(
            gpu,
            self.sf2_mission_overlay,
            0,
            0,
            SF2_REFERENCE_WIDTH,
            crate::sf2_mission_overlay::STATIC_HEIGHT,
            0,
            0,
            crate::sf2_mission_overlay::WIDTH as i32,
            crate::sf2_mission_overlay::HEIGHT as i32,
        );
        let blaster_atlas_top = match Sf2BlasterPalettePhase::at_frame(inputs.mode_frame) {
            Sf2BlasterPalettePhase::Cool => crate::sf2_mission_overlay::COOL_ATLAS_TOP,
            Sf2BlasterPalettePhase::Warm => crate::sf2_mission_overlay::WARM_ATLAS_TOP,
        };
        self.textured_quad_source_region(
            gpu,
            self.sf2_mission_overlay,
            crate::sf2_mission_overlay::BLASTER_LEFT,
            crate::sf2_mission_overlay::BLASTER_TOP,
            crate::sf2_mission_overlay::BLASTER_WIDTH,
            crate::sf2_mission_overlay::BLASTER_HEIGHT,
            0,
            blaster_atlas_top,
            crate::sf2_mission_overlay::WIDTH as i32,
            crate::sf2_mission_overlay::HEIGHT as i32,
        );
        self.textured_quad_source_frame(
            gpu,
            self.sf2_aim_sight,
            crate::sf2_aim_sight::LEFT,
            crate::sf2_aim_sight::TOP,
            crate::sf2_aim_sight::WIDTH as i32,
            crate::sf2_aim_sight::HEIGHT as i32,
        );
        let score = format!("{:05}", inputs.score.min(SF2_MAX_SCORE));
        for (index, digit) in score.bytes().enumerate() {
            self.render_sf2_hud_digit(
                gpu,
                SF2_MISSION_SCORE_X + index as i32 * crate::sf2_hud_glyphs::GLYPH_SIZE,
                SF2_MISSION_SCORE_TOP,
                digit - b'0',
                crate::sf2_hud_glyphs::SCORE_DIGITS_LEFT,
            );
        }
        let elapsed_whole = (inputs.mission_elapsed_time_tenths
            / SF2_MISSION_TIMER_TENTHS_PER_UNIT)
            .min(SF2_MISSION_TIMER_MAX_WHOLE);
        let elapsed_fraction =
            inputs.mission_elapsed_time_tenths % SF2_MISSION_TIMER_TENTHS_PER_UNIT;
        let whole_digits = format!("{:02}", elapsed_whole);
        for (index, digit) in whole_digits.bytes().enumerate() {
            self.render_sf2_hud_digit(
                gpu,
                SF2_MISSION_TIMER_X + index as i32 * crate::sf2_hud_glyphs::GLYPH_SIZE,
                SF2_MISSION_TIMER_TOP,
                digit - b'0',
                crate::sf2_hud_glyphs::CLOCK_DIGITS_LEFT,
            );
        }
        self.render_sf2_hud_atlas_region(
            gpu,
            SF2_MISSION_TIMER_SEPARATOR_X,
            SF2_MISSION_TIMER_SEPARATOR_TOP,
            crate::sf2_hud_glyphs::GLYPH_SIZE,
            crate::sf2_hud_glyphs::GLYPH_SIZE,
            crate::sf2_hud_glyphs::CLOCK_SEPARATOR_LEFT,
        );
        self.render_sf2_hud_digit(
            gpu,
            SF2_MISSION_TIMER_FRACTION_X,
            SF2_MISSION_TIMER_TOP,
            elapsed_fraction as u8,
            crate::sf2_hud_glyphs::CLOCK_DIGITS_LEFT,
        );

        self.render_sf2_mission_radar(gpu, inputs);

        let item_digit = inputs.item_count.min(SF2_MISSION_MAX_ITEM_COUNT);
        self.render_sf2_hud_atlas_region(
            gpu,
            SF2_MISSION_ITEM_COUNT_X,
            SF2_MISSION_ITEM_COUNT_TOP,
            crate::sf2_hud_glyphs::GLYPH_SIZE,
            crate::sf2_hud_glyphs::ITEM_DIGIT_HEIGHT,
            crate::sf2_hud_glyphs::ITEM_DIGITS_LEFT
                + i32::from(item_digit) * crate::sf2_hud_glyphs::GLYPH_SIZE,
        );
        let visible_shield_pips = usize::from(inputs.primary_shield.min(SF2_MISSION_MAX_SHIELD))
            .div_ceil(SF2_MISSION_SHIELD_PER_PIP);
        for pip in 0..visible_shield_pips.min(SF2_MISSION_SHIELD_PIP_CAPACITY) {
            let column = pip % SF2_MISSION_SHIELD_PIP_COLUMNS;
            let row = pip / SF2_MISSION_SHIELD_PIP_COLUMNS;
            self.render_sf2_hud_atlas_region(
                gpu,
                SF2_MISSION_SHIELD_PIP_X + column as i32 * crate::sf2_hud_glyphs::GLYPH_SIZE,
                SF2_MISSION_SHIELD_PIP_TOP + row as i32 * crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::GLYPH_SIZE,
                crate::sf2_hud_glyphs::SHIELD_PIP_LEFT,
            );
        }
    }

    fn render_sf2_mission_radar(&self, gpu: &mut Gpu, inputs: &Sf2FrameInputs) {
        let glyph_size = crate::sf2_hud_glyphs::GLYPH_SIZE;
        for contact in inputs.radar_contacts.iter().flatten() {
            let lateral =
                i32::from(contact.lateral).clamp(-SF2_RADAR_WORLD_RANGE, SF2_RADAR_WORLD_RANGE);
            let forward =
                i32::from(contact.forward).clamp(-SF2_RADAR_WORLD_RANGE, SF2_RADAR_WORLD_RANGE);
            let x = SF2_RADAR_PLAYER_LEFT + lateral * SF2_RADAR_AXIS_RADIUS / SF2_RADAR_WORLD_RANGE;
            let top =
                SF2_RADAR_PLAYER_TOP - forward * SF2_RADAR_AXIS_RADIUS / SF2_RADAR_WORLD_RANGE;
            self.render_sf2_hud_atlas_region(
                gpu,
                x,
                top,
                glyph_size,
                glyph_size,
                if contact.friendly {
                    crate::sf2_hud_glyphs::RADAR_PLAYER_LEFT
                } else {
                    crate::sf2_hud_glyphs::RADAR_ENEMY_LEFT
                },
            );
        }
        self.render_sf2_hud_atlas_region(
            gpu,
            SF2_RADAR_PLAYER_LEFT,
            SF2_RADAR_PLAYER_TOP,
            glyph_size,
            glyph_size,
            crate::sf2_hud_glyphs::RADAR_PLAYER_LEFT,
        );
    }

    fn render_sf2(&mut self, gpu: &mut Gpu, font: &mut Font, inputs: &Sf2FrameInputs) {
        match inputs.mode {
            Sf2Mode::Title => self.render_sf2_title(gpu, inputs),
            Sf2Mode::Records => self.render_sf2_records(gpu, inputs),
            Sf2Mode::Briefing => self.render_sf2_briefing(gpu, inputs),
            Sf2Mode::StrategicMap => self.render_sf2_strategic_map(gpu, font, inputs),
            Sf2Mode::PilotSelection => self.render_sf2_pilot_selection(gpu, inputs),
            Sf2Mode::Mission => self.render_sf2_mission_hud(gpu, inputs),
            Sf2Mode::GameOver => self.render_sf2_game_over(gpu, font, inputs),
            Sf2Mode::Results => self.render_sf2_results(gpu, inputs),
            Sf2Mode::Intro | Sf2Mode::Ending => {}
        }
    }

    fn ensure_atlas(&mut self, gpu: &mut Gpu) -> bool {
        if self.ps_tex.is_some() {
            return true;
        }
        if self.ps_tried {
            return false;
        }
        self.ps_tried = true;

        let load = |rel: &str| std::fs::read(self.base_dir.join(rel)).ok();
        let tex0 = load("data/map/tex_0.CGX");
        let tex1 = load("data/map/tex_1.CGX");
        let mo = load("data/map/MAP-OBJ.CGX");
        let mocol = load("data/map/MAP-OBJ.COL");
        let mcgx = load("data/bg/MAP.CGX");
        let mcol = load("data/bg/MAP_C.COL");
        let (Some(tex0), Some(tex1), Some(mo), Some(mocol), Some(mcgx), Some(mcol)) =
            (tex0, tex1, mo, mocol, mcgx, mcol)
        else {
            eprintln!("Ui: planet select assets missing (data/map, data/bg)");
            return false;
        };

        let Some(atlas) = compose_planet_select_atlas(&tex0, &tex1, &mo, &mocol, &mcgx, &mcol)
        else {
            eprintln!("Ui: planet select assets missing (data/map, data/bg)");
            return false;
        };

        self.ps_tex = Some(gpu.create_texture_rgba(PS_AW as u32, PS_AH as u32, &atlas));
        true
    }

    /// Draw an atlas rect at SNES coords (x, y top-left, y down), stretched
    /// with the full-screen 256x224 mapping (mirror of `ps_draw`).
    #[allow(clippy::too_many_arguments)]
    fn ps_draw(
        &self,
        gpu: &mut Gpu,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        ax: i32,
        ay: i32,
        hflip: bool,
        vflip: bool,
    ) {
        let Some(ps_tex) = self.ps_tex else {
            return;
        };
        let sx = self.scr_w as f32 / 256.0;
        let sy = self.scr_h as f32 / 224.0;

        let x0 = x as f32 * sx;
        let x1 = (x + w) as f32 * sx;
        let ytop = (224 - y) as f32 * sy;
        let ybot = (224 - y - h) as f32 * sy;

        let mut u0 = ax as f32 / PS_AW as f32;
        let mut u1 = (ax + w) as f32 / PS_AW as f32;
        let mut v0 = ay as f32 / PS_AH as f32; // atlas top
        let mut v1 = (ay + h) as f32 / PS_AH as f32; // atlas bottom

        if hflip {
            std::mem::swap(&mut u0, &mut u1);
        }
        if vflip {
            std::mem::swap(&mut v0, &mut v1);
        }

        let verts = [
            Vertex2 {
                pos: [x0, ybot],
                uv: [u0, v1],
            },
            Vertex2 {
                pos: [x1, ybot],
                uv: [u1, v1],
            },
            Vertex2 {
                pos: [x1, ytop],
                uv: [u1, v0],
            },
            Vertex2 {
                pos: [x0, ytop],
                uv: [u0, v0],
            },
        ];
        gpu.push_overlay_fan(
            &verts,
            &self.proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            ps_tex,
        );
    }

    /// Planet select route map (PLANETS.ASM planetseq).
    fn render_planet_select(&mut self, gpu: &mut Gpu, inputs: &FrameInputs) {
        if !self.ensure_atlas(gpu) {
            return;
        }
        if self.ps_tex.is_none() {
            return;
        }

        // Planet/sector bitmaps (drawplanetsprites): all 16 slots; slot 14
        // (Out Of This Dimension) only once the nebula route is open.
        for p in 0..16usize {
            if p == 14 && inputs.nebula_on == 0 {
                continue;
            }
            self.ps_draw(
                gpu,
                PLANET_POS[p][0] as i32,
                PLANET_POS[p][1] as i32,
                32,
                32,
                ((p & 7) * 32) as i32,
                ((p >> 3) * 32) as i32,
                false,
                false,
            );
        }

        // Course line (drawplanetlines_l): dotted 8px steps along the
        // selected route's stagepaths, blinking like the select loop.
        let line_on = inputs.stage != 0 || (self.frame & 2) != 0;
        if line_on {
            for &path_id in inputs.route_path_ids {
                if path_id as usize >= UI_PATH_ID_COUNT {
                    continue;
                }
                let geo = &PATH_GEO[path_id as usize];
                let mut px = geo.sx as i32 * 8;
                let mut py = geo.sy as i32 * 8;
                for st in geo.steps {
                    match st.seg {
                        SEG_DIAG_UP => self.ps_draw(gpu, px, py, 8, 8, 48, 64, false, false),
                        SEG_DIAG_DN => self.ps_draw(gpu, px, py, 8, 8, 48, 64, false, true),
                        SEG_HORIZ => self.ps_draw(gpu, px, py, 8, 8, 56, 64, false, false),
                        SEG_VERT => self.ps_draw(gpu, px, py, 8, 8, 64, 64, true, false),
                        _ => {} // hidden step: move only
                    }
                    px += st.dx as i32;
                    py += st.dy as i32;
                }
            }
        }

        // Arwing cursor: 16x16 at startplanetpos + 8, initial shipangle = 1
        // (diagonal), solid during selection.
        if inputs.currentplanet >= 0 && inputs.currentplanet < 16 {
            let p = inputs.currentplanet as usize;
            self.ps_draw(
                gpu,
                START_POS[p][0] as i32 + 8,
                START_POS[p][1] as i32 + 8,
                16,
                16,
                16,
                64,
                false,
                false,
            );
        }

        // Route label (drawroutename) and running-total score line. ROM prints
        // the total's hundreds/tens/ones then two fixed '0's (score shown x100),
        // PLANETS.ASM:1583-1595. `inputs.score` is calctotalscore/tpa.
        {
            let n = if inputs.whichroute < 3 {
                inputs.whichroute as i32
            } else {
                0
            };
            self.ps_draw(gpu, 192, 200, 48, 8, n * 48, 80, false, false);
            for (d, &dig) in score_line_digits(inputs.score).iter().enumerate() {
                self.ps_draw(
                    gpu,
                    192 + d as i32 * 8,
                    192,
                    8,
                    8,
                    144 + dig as i32 * 8,
                    80,
                    false,
                    false,
                );
            }
        }
    }

    /// Retail `end_level_seq` presentation. Coordinates are the source
    /// framebuffer positions plus its 16-pixel horizontal screen inset.
    fn render_tally(&self, gpu: &mut Gpu, font: &mut Font, inputs: &FrameInputs) {
        const FRAMEBUFFER_X_INSET: i32 = 16;
        const TEXT_HEIGHT: i32 = 8;
        const SCORE_TOP: i32 = 24;
        const TOTAL_TOP: i32 = 40;
        const TEAM_LABEL_TOP: i32 = 69;
        const PORTRAIT_TOP: i32 = 88;
        const TEAM_BAR_TOP: i32 = 136;
        const TEAM_BAR_HEIGHT: i32 = 12;
        const LIVE_NAME_TOP: i32 = 150;
        const DEAD_NAME_TOP: i32 = 137;
        const DOWN_LABEL_TOP: i32 = 151;
        const SCORE_LABEL_LEFT: i32 = FRAMEBUFFER_X_INSET + 16;
        const SCORE_GRAPH_LEFT: i32 = FRAMEBUFFER_X_INSET + 60;
        const SCORE_GRAPH_WIDTH: i32 = TALLY_GRAPH_INNER_WIDTH + 4;
        const SCORE_GRAPH_HEIGHT: i32 = 12;
        const SCORE_VALUE_RIGHT: i32 = 224;
        const TEAM_LABEL_LEFT: i32 = FRAMEBUFFER_X_INSET + 48;
        const PORTRAIT_LEFTS: [i32; 3] = [32, 112, 192];
        const TEAM_BAR_LEFTS: [i32; 3] = [27, 107, 187];
        const LIVE_NAME_LEFTS: [i32; 3] = [31, 112, 189];
        const DEAD_NAME_LEFTS: [i32; 3] = [27, 107, 186];
        const PILOT_NAMES: [&str; 3] = ["PEPPY", "FALCO", "SLIPPY"];

        let text_y = |top: i32| SF2_REFERENCE_HEIGHT - top - TEXT_HEIGHT;
        let color = TALLY_WHITE;

        if inputs.tally_bonus_visible {
            self.text_snes(
                gpu,
                font,
                SCORE_LABEL_LEFT,
                text_y(SCORE_TOP),
                "BONUS 1 CREDIT",
                color[0],
                color[1],
                color[2],
            );
        } else {
            self.text_snes(
                gpu,
                font,
                SCORE_LABEL_LEFT,
                text_y(SCORE_TOP),
                "SCORE",
                color[0],
                color[1],
                color[2],
            );
            let graph_bottom = SF2_REFERENCE_HEIGHT - SCORE_TOP - SCORE_GRAPH_HEIGHT;
            self.quad_snes(
                gpu,
                TALLY_WHITE,
                SCORE_GRAPH_LEFT,
                graph_bottom,
                SCORE_GRAPH_WIDTH,
                SCORE_GRAPH_HEIGHT,
            );
            self.quad_snes(
                gpu,
                TALLY_BLACK,
                SCORE_GRAPH_LEFT + 2,
                graph_bottom + 2,
                TALLY_GRAPH_INNER_WIDTH,
                SCORE_GRAPH_HEIGHT - 4,
            );
            let current = inputs
                .tally_current_perc
                .min(inputs.tally_stage_perc)
                .min(100);
            if current > 0 {
                self.quad_snes(
                    gpu,
                    TALLY_CYAN,
                    SCORE_GRAPH_LEFT + 2,
                    graph_bottom + 2,
                    i32::from(current),
                    SCORE_GRAPH_HEIGHT - 4,
                );
            }
            let percentage = format!("{} %", current);
            self.text_snes(
                gpu,
                font,
                SCORE_VALUE_RIGHT - percentage.len() as i32 * 8,
                text_y(SCORE_TOP),
                &percentage,
                color[0],
                color[1],
                color[2],
            );
        }

        self.text_snes(
            gpu,
            font,
            SCORE_LABEL_LEFT,
            text_y(TOTAL_TOP),
            "TOTAL SCORE",
            color[0],
            color[1],
            color[2],
        );
        let total = format!("{}00", inputs.score.min(999));
        self.text_snes(
            gpu,
            font,
            SCORE_VALUE_RIGHT - total.len() as i32 * 8,
            text_y(TOTAL_TOP),
            &total,
            color[0],
            color[1],
            color[2],
        );
        self.text_snes(
            gpu,
            font,
            TEAM_LABEL_LEFT,
            text_y(TEAM_LABEL_TOP),
            "SHIELD OF TEAMMATES",
            color[0],
            color[1],
            color[2],
        );

        for teammate in 0..3 {
            let shield = inputs.tally_teammate_shields[teammate];
            if let Some(texture) = self.tally_portraits {
                let portrait_slot = if shield == 0 {
                    3 + usize::from(inputs.gameframe & 1 != 0)
                } else {
                    teammate
                };
                self.textured_quad_source_region(
                    gpu,
                    texture,
                    PORTRAIT_LEFTS[teammate],
                    PORTRAIT_TOP,
                    TALLY_PORTRAIT_WIDTH as i32,
                    TALLY_PORTRAIT_HEIGHT as i32,
                    (portrait_slot * TALLY_PORTRAIT_WIDTH) as i32,
                    0,
                    TALLY_PORTRAIT_ATLAS_WIDTH as i32,
                    TALLY_PORTRAIT_ATLAS_HEIGHT as i32,
                );
            }

            if shield > 0 {
                let bar_bottom = SF2_REFERENCE_HEIGHT - TEAM_BAR_TOP - TEAM_BAR_HEIGHT;
                self.quad_snes(
                    gpu,
                    TALLY_WHITE,
                    TEAM_BAR_LEFTS[teammate],
                    bar_bottom,
                    TALLY_TEAMMATE_BAR_INNER_WIDTH + 4,
                    TEAM_BAR_HEIGHT,
                );
                self.quad_snes(
                    gpu,
                    TALLY_BLACK,
                    TEAM_BAR_LEFTS[teammate] + 2,
                    bar_bottom + 2,
                    TALLY_TEAMMATE_BAR_INNER_WIDTH,
                    TEAM_BAR_HEIGHT - 4,
                );
                let fill = i32::from(shield.min(TALLY_MAX_TEAMMATE_SHIELD));
                self.quad_snes(
                    gpu,
                    TALLY_PINK,
                    TEAM_BAR_LEFTS[teammate] + 2,
                    bar_bottom + 2,
                    fill,
                    TEAM_BAR_HEIGHT - 4,
                );
                self.text_snes(
                    gpu,
                    font,
                    LIVE_NAME_LEFTS[teammate],
                    text_y(LIVE_NAME_TOP),
                    PILOT_NAMES[teammate],
                    color[0],
                    color[1],
                    color[2],
                );
            } else {
                self.text_snes(
                    gpu,
                    font,
                    DEAD_NAME_LEFTS[teammate],
                    text_y(DEAD_NAME_TOP),
                    PILOT_NAMES[teammate],
                    color[0],
                    color[1],
                    color[2],
                );
                self.text_snes(
                    gpu,
                    font,
                    DEAD_NAME_LEFTS[teammate],
                    text_y(DOWN_LABEL_TOP),
                    "IS DOWN",
                    color[0],
                    color[1],
                    color[2],
                );
            }
        }
    }

    /// Mirror of `Ui_Render`.
    pub fn render(
        &mut self,
        gpu: &mut Gpu,
        font: &mut Font,
        bg2d: &Bg2d,
        inputs: &FrameInputs,
        screen_width: i32,
        screen_height: i32,
    ) {
        self.frame = self.frame.wrapping_add(1);

        if let Some(sf2) = inputs.sf2.as_ref() {
            self.begin_2d(screen_width, screen_height);
            self.render_sf2(gpu, font, sf2);
            return;
        }

        if let Some(replay) = inputs.ending_replay {
            self.begin_2d(screen_width, screen_height);
            self.render_ending_replay_text(gpu, replay);
            return;
        }

        if inputs.tally_active {
            self.begin_2d(screen_width, screen_height);
            self.render_tally(gpu, font, inputs);
            return;
        }

        if inputs.game_state != GameState::Title && inputs.game_state != GameState::PlanetSelect {
            return;
        }

        self.begin_2d(screen_width, screen_height);

        match inputs.game_state {
            GameState::Title => self.render_title(gpu, font, bg2d),
            GameState::PlanetSelect => self.render_planet_select(gpu, inputs),
            _ => {}
        }
    }

    /// Fade overlay: wm_val 0..30 (black/mapfade) or 0..31 (white) maps to
    /// alpha. Mirror of `Ui_RenderFade`.
    pub fn render_fade(
        &mut self,
        gpu: &mut Gpu,
        inputs: &FrameInputs,
        screen_width: i32,
        screen_height: i32,
    ) {
        let mut black_a = 0.0f32;
        let mut white_a = 0.0f32;

        for (i, w) in inputs.windows.iter().enumerate() {
            if inputs.windowmode & (1u8 << i) == 0 {
                continue;
            }
            match w.mode {
                WINDOW_MODE_BLACK | WINDOW_MODE_MAPFADE => {
                    let a = (w.wm_val as f32 / 30.0).min(1.0);
                    if a > black_a {
                        black_a = a;
                    }
                }
                WINDOW_MODE_WHITEFADE | WINDOW_MODE_WHITE2NORM => {
                    let a = (w.wm_val as f32 / 31.0).min(1.0);
                    if a > white_a {
                        white_a = a;
                    }
                }
                _ => {}
            }
        }

        if black_a < 0.004 && white_a < 0.004 {
            return;
        }

        self.begin_2d(screen_width, screen_height);

        if black_a >= 0.004 {
            self.quad_px(
                gpu,
                [0.0, 0.0, 0.0, black_a],
                0.0,
                0.0,
                screen_width as f32,
                0.0,
                screen_width as f32,
                screen_height as f32,
                0.0,
                screen_height as f32,
            );
        }
        if white_a >= 0.004 {
            self.quad_px(
                gpu,
                [1.0, 1.0, 1.0, white_a],
                0.0,
                0.0,
                screen_width as f32,
                0.0,
                screen_width as f32,
                screen_height as f32,
                0.0,
                screen_height as f32,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        compose_tally_portrait_atlas, score_line_digits, sf2_game_over_brightness,
        sf2_results_brightness,
    };
    use crate::renderer::{
        Sf2GameOverChoice, Sf2ResultsChoice, Sf2ResultsPhase,
    };
    use crate::sf2_game_over::Brightness;

    const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
    const FNV_PRIME: u32 = 0x01000193;

    #[test]
    fn campaign_loss_game_over_uses_the_short_retail_fade() {
        let choice = Sf2GameOverChoice::EndCampaign;
        for (elapsed, expected) in [
            (48, Brightness::Full),
            (52, Brightness::ThirteenFifteenths),
            (56, Brightness::ElevenFifteenths),
            (60, Brightness::NineFifteenths),
            (64, Brightness::FiveFifteenths),
            (68, Brightness::ThreeFifteenths),
            (72, Brightness::OneFifteenth),
            (76, Brightness::Black),
        ] {
            assert_eq!(sf2_game_over_brightness(choice, elapsed), expected);
        }
        assert_eq!(
            sf2_game_over_brightness(Sf2GameOverChoice::ContinueWithWingmate, 148),
            Brightness::ThirteenFifteenths
        );
    }

    #[test]
    fn results_destinations_use_their_certified_final_fade_frames() {
        assert_eq!(
            sf2_results_brightness(Sf2ResultsPhase::Leaving, Sf2ResultsChoice::Retry, 120),
            Brightness::ThirteenFifteenths
        );
        assert_eq!(
            sf2_results_brightness(Sf2ResultsPhase::Leaving, Sf2ResultsChoice::Retry, 124),
            Brightness::SevenFifteenths
        );
        assert_eq!(
            sf2_results_brightness(Sf2ResultsPhase::Leaving, Sf2ResultsChoice::Title, 124),
            Brightness::NineFifteenths
        );
        assert_eq!(
            sf2_results_brightness(Sf2ResultsPhase::Choosing, Sf2ResultsChoice::Title, 124),
            Brightness::Full
        );
    }

    #[test]
    fn score_line_zero_is_all_zeros() {
        // Fresh game: hardcoded "00000" behaviour preserved for total 0.
        assert_eq!(score_line_digits(0), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn score_line_reflects_nonzero_total() {
        // A running total of 123 renders "12300" (score shown x100): the
        // first three glyphs are the total's digits, not all '0'.
        assert_eq!(score_line_digits(123), [1, 2, 3, 0, 0]);
        // Single/double-digit totals zero-pad the leading places.
        assert_eq!(score_line_digits(80), [0, 8, 0, 0, 0]);
        assert_eq!(score_line_digits(5), [0, 0, 5, 0, 0]);
        // Any nonzero total selects a nonzero digit somewhere in the line.
        assert_ne!(score_line_digits(240), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn score_line_caps_at_999() {
        // ROM per-place count-down saturates each digit at 9.
        assert_eq!(score_line_digits(2100), [9, 9, 9, 0, 0]);
    }

    #[test]
    fn tally_portraits_decode_from_the_exact_source_frames() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/sprites/FACE.CGX");
        let source = std::fs::read(path).expect("retail-extracted FACE.CGX");
        let atlas = compose_tally_portrait_atlas(&source).expect("complete tally portrait frames");
        if let Some(path) = std::env::var_os("SF1_TALLY_ATLAS_DUMP_PPM") {
            let mut ppm = format!(
                "P6\n{} {}\n255\n",
                super::TALLY_PORTRAIT_ATLAS_WIDTH,
                super::TALLY_PORTRAIT_ATLAS_HEIGHT
            )
            .into_bytes();
            ppm.extend(atlas.chunks_exact(4).flat_map(|pixel| &pixel[..3]));
            std::fs::write(path, ppm).expect("write requested tally atlas dump");
        }
        let hash = atlas.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
            (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
        });
        assert_eq!(
            hash, 0x6FE11C33,
            "the inspected FACE.CGX tally atlas drifted"
        );
    }
}
