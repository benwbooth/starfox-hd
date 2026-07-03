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
    FrameInputs, GameState, WINDOW_MODE_BLACK, WINDOW_MODE_MAPFADE, WINDOW_MODE_WHITE2NORM,
    WINDOW_MODE_WHITEFADE,
};
use crate::sprites::decode_4bpp_tile;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[inline]
fn ortho(w: f32, h: f32) -> [f32; 16] {
    [
        2.0 / w, 0.0, 0.0, 0.0, 0.0, 2.0 / h, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, -1.0, -1.0, 0.0, 1.0,
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
    [16, 176], [64, 176], [56, 136], [16, 128],
    [112, 144], [40, 64], [128, 96], [184, 144],
    [168, 96], [112, 32], [80, 80], [208, 96],
    [192, 40], [192, 40], [136, 176], [192, 40],
];

/// startplanetpos: course-line/ship anchor per planet.
static START_POS: [[u8; 2]; 16] = [
    [16, 176], [64, 176], [64, 136], [16, 128],
    [112, 144], [48, 64], [128, 104], [184, 144],
    [176, 96], [112, 32], [80, 80], [208, 96],
    [192, 40], [192, 40], [136, 176], [192, 40],
];

/// planetsprs: which SuperFX msprites texture cell each planet uses.
#[derive(Clone, Copy)]
struct PlanetSpr {
    sheet: u8,
    cell: u8,
    sphere: u8,
}

const fn psr(sheet: u8, cell: u8, sphere: u8) -> PlanetSpr {
    PlanetSpr { sheet, cell, sphere }
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

const P_UP: PathStep = PathStep { dx: 0, dy: -8, seg: SEG_VERT };
const P_UR: PathStep = PathStep { dx: 8, dy: -8, seg: SEG_DIAG_UP };
const P_R: PathStep = PathStep { dx: 8, dy: 0, seg: SEG_HORIZ };
const P_UPH: PathStep = PathStep { dx: 0, dy: -8, seg: SEG_HIDDEN };
const P_URH: PathStep = PathStep { dx: 8, dy: -8, seg: SEG_HIDDEN };
const P_DRH: PathStep = PathStep { dx: 8, dy: 8, seg: SEG_HIDDEN };
const P_RH: PathStep = PathStep { dx: 8, dy: 0, seg: SEG_HIDDEN };

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
const GEO_NONE: PathGeo = PathGeo { sx: 0, sy: 0, steps: &[] };

/// Indexed by planets.c PATH_ID_* (1..22, then END3/END2/END1/OTHEREND).
const UI_PATH_ID_COUNT: usize = 27;
static PATH_GEO: [PathGeo; UI_PATH_ID_COUNT] = [
    GEO_NONE,             //  0 invalid
    geo(4, 20, &STEPS1),  //  1 Corneria 2
    geo(4, 14, &STEPS2),  //  2 Sector X
    geo(9, 8, &STEPS3),   //  3 Titania
    geo(18, 5, &STEPS4),  //  4 Sector Y
    GEO_NONE,             //  5 Venom 2 orbital
    geo(6, 21, &STEPS6),  //  6 Corneria 1
    geo(11, 17, &STEPS7), //  7 Asteroid Belt 1
    geo(20, 14, &STEPS8), //  8 Space Armada
    geo(24, 11, &STEPS9), //  9 Meteor
    GEO_NONE,             // 10 Venom 1 orbital
    geo(6, 23, &STEPS11), // 11 Corneria 3
    geo(12, 21, &STEPS12), // 12 Asteroid Belt 3
    geo(18, 19, &STEPS13), // 13 Fortuna
    geo(27, 19, &STEPS14), // 14 Sector Z
    geo(28, 11, &STEPS15), // 15 Macbeth
    GEO_NONE,             // 16 Venom 3 orbital
    geo(6, 15, &STEPS17), // 17 Sector X (black hole entry)
    geo(12, 9, &STEPS18), // 18 Black Hole -> Sector Y
    geo(13, 11, &STEPS19), // 19 Black Hole -> Venom 1
    geo(11, 14, &STEPS20), // 20 Black Hole -> Sector Z
    geo(9, 16, &STEPS21), // 21 Asteroid 1 (black hole entry)
    geo(12, 25, &STEPS22), // 22 Asteroid 3 (OOTD entry)
    GEO_NONE,             // 23 end3
    GEO_NONE,             // 24 end2
    GEO_NONE,             // 25 end1
    GEO_NONE,             // 26 otherend
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
        ps_blit_tile(&mut atlas, ax + 8, 64, &tm[(base + 1) * 64..], &pal_obj, false);
        ps_blit_tile(&mut atlas, ax, 72, &tm[(base + 2) * 64..], &pal_obj, false);
        ps_blit_tile(&mut atlas, ax + 8, 72, &tm[(base + 3) * 64..], &pal_obj, false);
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
            ps_blit_tile(&mut atlas, n * 48 + c * 8, 80, &tg[tile * 64..], &pal_txt, true);
        }
    }
    // Score digit '0' ($88)
    if 0x88 < n_tg {
        ps_blit_tile(&mut atlas, 144, 80, &tg[0x88 * 64..], &pal_txt, true);
    }

    Some(atlas)
}

// ---------------------------------------------------------------------------
// GL runtime state
// ---------------------------------------------------------------------------

pub struct Ui {
    base_dir: PathBuf,
    frame: u32, // render-frame counter for blink effects
    ps_tex: Option<TextureId>,
    ps_tried: bool,

    // Screen mapping state for the current frame.
    scale: f32,
    ox: i32, // centering offset in SNES units
    scr_w: i32,
    scr_h: i32,
    proj: [f32; 16], // ortho for the current frame (set in begin_2d)
}

impl Ui {
    pub fn new(_gpu: &mut Gpu, base_dir: &Path) -> Self {
        Ui {
            base_dir: base_dir.to_path_buf(),
            frame: 0,
            ps_tex: None,
            ps_tried: false,
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
            Vertex2 { pos: [x0, y0], uv: [0.0, 0.0] },
            Vertex2 { pos: [x1, y1], uv: [1.0, 0.0] },
            Vertex2 { pos: [x2, y2], uv: [1.0, 1.0] },
            Vertex2 { pos: [x3, y3], uv: [0.0, 1.0] },
        ];
        gpu.push_overlay_fan(&verts, &self.proj, &IDENTITY, color, 0, None, WHITE_TEX);
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
            Vertex2 { pos: [x0, ybot], uv: [u0, v1] },
            Vertex2 { pos: [x1, ybot], uv: [u1, v1] },
            Vertex2 { pos: [x1, ytop], uv: [u1, v0] },
            Vertex2 { pos: [x0, ytop], uv: [u0, v0] },
        ];
        gpu.push_overlay_fan(&verts, &self.proj, &IDENTITY, [1.0, 1.0, 1.0, 1.0], 1, None, ps_tex);
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

        // Route label (drawroutename) and score line "00000".
        {
            let n = if inputs.whichroute < 3 {
                inputs.whichroute as i32
            } else {
                0
            };
            self.ps_draw(gpu, 192, 200, 48, 8, n * 48, 80, false, false);
            for d in 0..5 {
                self.ps_draw(gpu, 192 + d * 8, 192, 8, 8, 144, 80, false, false);
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

        if inputs.game_state != GameState::Title
            && inputs.game_state != GameState::PlanetSelect
        {
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
