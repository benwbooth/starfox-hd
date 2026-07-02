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

use glow::HasContext;
use std::path::{Path, PathBuf};

use crate::gl_backend::{self, GlBackend};
use crate::renderer::{FrameInputs, GameState, BGF_BG};
use crate::transform::Transform;

pub const BG2D_W: usize = 256;
pub const BG2D_H: usize = 224;

/// Background ids from levels.c map bytecode (setbg opcode operand).
pub const BG2D_ID_TITLE: u8 = 41;
/// Pseudo-id for the planet-select / briefing map screen.
pub const BG2D_ID_MAP: u8 = 63;
/// Pseudo-id for bg_special (SNES id 44 clashes with the port's BG_TRAINING).
pub const BG2D_ID_SPECIAL: u8 = 62;

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
    bgdef!(4, "bg_1_1c", "data/bg/ST-P.CGX", "data/bg/ST-P.SCR", "data/bg/BG2-D.COL", 232, true),
    bgdef!(3, "bg_3_1c", "data/bg/ST-P.CGX", "data/bg/ST-P.SCR", "data/bg/BG2-G.COL", 232, true),
    bgdef!(44, "bg_training", "data/bg/ST-P.CGX", "data/bg/ST-P.SCR", "data/bg/BG2-D.COL", 232, true),
    // Asteroid-belt space (starfield + cratered moon)
    bgdef!(6, "bg_1_3i", "data/bg/3-4.CGX", "data/bg/3-4.SCR", "data/bg/SPACE.COL", 232, true),
    bgdef!(7, "bg_1_3a", "data/bg/3-4.CGX", "data/bg/3-4.SCR", "data/bg/SPACE.COL", 232, true),
    bgdef!(9, "bg_1_3c", "data/bg/3-4.CGX", "data/bg/3-4.SCR", "data/bg/SPACE.COL", 232, true),
    bgdef!(35, "bg_3_4d", "data/bg/3-4.CGX", "data/bg/3-4.SCR", "data/bg/SPACE.COL", 0, true),
    // Asteroid clear demo (asteroid + planets starfield)
    bgdef!(12, "bg_1_3e", "data/bg/SPACE.CGX", "data/bg/1-3.SCR", "data/bg/SPACE.COL", 232, true),
    // Tunnels (info voff; static base image)
    bgdef!(8, "bg_1_3b", "data/bg/T-SP.CGX", "data/bg/T-SP.SCR", "data/bg/T-M-3.COL", 0, false),
    bgdef!(34, "bg_3_4c", "data/bg/T-SP.CGX", "data/bg/T-SP.SCR", "data/bg/T-M-3.COL", 0, false),
    bgdef!(25, "bg_2_3c", "data/bg/T-SP.CGX", "data/bg/T-F-S.SCR", "data/bg/T-M-3.COL", 0, false),
    bgdef!(29, "bg_2_6c", "data/bg/T-ST.CGX", "data/bg/T-ST.SCR", "data/bg/T-M-3.COL", 0, false),
    // Venom final approach (glowing spheres; info voff)
    bgdef!(17, "bg_1_6c", "data/bg/B-HOLE.CGX", "data/bg/LAST.SCR", "data/bg/BG2-F.COL", 0, false),
    // Fortuna bridge (sky/water backdrop + BG3 water-surface overlay; voff)
    bgdef!(24, "bg_2_3b", "data/bg/B-M.CGX", "data/bg/2-3B.SCR", "data/bg/B-M.COL", 0,
           "data/bg/2-3B.CGX", "data/bg/2-3H.SCR", 24, false),
    // Attract intro (Corneria seen from space; info von,hon)
    bgdef!(40, "bg_intro", "data/bg/DEMO.CGX", "data/bg/DEMO.SCR", "data/bg/BG2-B.COL", 24, true),
    // Continue / controller screen (US CONT-2; info voff)
    bgdef!(42, "bg_cont", "data/bg/CONT-2.CGX", "data/bg/CONT-2.SCR", "data/bg/BG2-E.COL", 0, false),
    // Credits (nebula starfield; info von,hon)
    bgdef!(43, "bg_cred", "data/bg/2-4.CGX", "data/bg/2-4.SCR", "data/bg/BG2-F.COL", 232, true),
    // Planet select / briefing map screen (pseudo-id)
    bgdef!(BG2D_ID_MAP, "planets_map", "data/bg/MAP.CGX", "data/bg/MAP.SCR", "data/bg/MAP_C.COL", 0, false),
    // --- Level-default backgrounds ---
    bgdef!(5, "bg_1_2", "data/bg/STARS.CGX", "data/bg/STARS.SCR", "data/bg/STARS.COL", 232, true),
    bgdef!(13, "bg_1_4", "data/bg/1-4.CGX", "data/bg/1-4.SCR", "data/bg/LIGHT.COL", 232, true),
    bgdef!(14, "bg_1_5", "data/bg/LSB.CGX", "data/bg/LSB.SCR", "data/bg/BG2-C.COL", 164, true),
    bgdef!(37, "bg_3_6", "data/bg/LSB.CGX", "data/bg/LSB.SCR", "data/bg/BG2-C.COL", 164, true),
    bgdef!(15, "bg_1_6a", "data/bg/F-1.CGX", "data/bg/F-1.SCR", "data/bg/BG2-A.COL", 232, true),
    bgdef!(38, "bg_3_7a", "data/bg/F-1.CGX", "data/bg/F-1.SCR", "data/bg/BG2-A.COL", 232, true),
    bgdef!(22, "bg_2_2", "data/bg/2-2.CGX", "data/bg/2-2.SCR", "data/bg/SPACE.COL", 232, true),
    bgdef!(23, "bg_2_3a", "data/bg/2-3.CGX", "data/bg/2-3.SCR", "data/bg/BG2-A.COL", 232, true),
    bgdef!(26, "bg_2_4", "data/bg/2-4.CGX", "data/bg/2-4.SCR", "data/bg/BG2-F.COL", 232, true),
    bgdef!(27, "bg_2_6a", "data/bg/C-M.CGX", "data/bg/T-SS.SCR", "data/bg/T-M-2.COL", 0,
           "data/bg/FS-BG3.CGX", "data/bg/FS-NI.SCR", 0, false),
    bgdef!(30, "bg_3_2", "data/bg/3-2.CGX", "data/bg/3-2.SCR", "data/bg/BG2-B.COL", 24, true),
    bgdef!(31, "bg_3_3a", "data/bg/3-3.CGX", "data/bg/3-3.SCR", "data/bg/BG2-C.COL", 232, true),
    bgdef!(33, "bg_3_4b", "data/bg/3-4.CGX", "data/bg/3-4.SCR", "data/bg/SPACE.COL", 232, true),
    bgdef!(36, "bg_3_5", "data/bg/HOLE-A.CGX", "data/bg/HOLE-A.SCR", "data/bg/HOLE.COL", 272, true),
    // bg_hole: info hon,voff -> static vertical (bhole hofs warp not ported)
    bgdef!(39, "bg_hole", "data/bg/B-HOLE.CGX", "data/bg/B-HOLE.SCR", "data/bg/BG2-D.COL", 0, false),
    bgdef!(BG2D_ID_SPECIAL, "bg_special", "data/bg/M.CGX", "data/bg/M.SCR", "data/bg/SPACE.COL", 448, true),
];

/// Default bg id per loaded map (mirror of `s_map_default_bg`; map ids are
/// the port's levels.h MAP_ID_* values).
pub static MAP_DEFAULT_BG: &[(u32, u8)] = &[
    (1, 4),   // MAP_ID_1_1
    (2, 5),   // MAP_ID_1_2
    (3, 6),   // MAP_ID_1_3
    (4, 13),  // MAP_ID_1_4
    (5, 14),  // MAP_ID_1_5
    (6, 15),  // MAP_ID_1_6
    (7, 4),   // MAP_ID_2_1
    (8, 22),  // MAP_ID_2_2
    (9, 23),  // MAP_ID_2_3
    (10, 26), // MAP_ID_2_4
    (11, 14), // MAP_ID_2_5
    (12, 27), // MAP_ID_2_6
    (13, 3),  // MAP_ID_3_1
    (14, 30), // MAP_ID_3_2
    (15, 31), // MAP_ID_3_3
    (16, 33), // MAP_ID_3_4
    (17, 36), // MAP_ID_3_5
    (18, 37), // MAP_ID_3_6
    (19, 38), // MAP_ID_3_7
    (20, 39), // MAP_ID_BLACKHOLE
    (21, BG2D_ID_SPECIAL), // MAP_ID_SPECIAL
    (22, 17), // MAP_ID_FINAL
    (23, 40), // MAP_ID_INTRO
    (24, BG2D_ID_TITLE), // MAP_ID_TITLE
    (25, 42), // MAP_ID_CONTINUE
    (28, 43), // MAP_ID_CREDITS
    (29, 44), // MAP_ID_TRAINING
];

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

/// BGR555 -> RGB888 (matches the C `* 255 / 31` integer scaling).
pub fn cgram_color(col: &[u8], index: usize) -> [u8; 3] {
    let w = col[index * 2] as u16 | ((col[index * 2 + 1] as u16) << 8);
    [
        (((w) & 0x1F) as u32 * 255 / 31) as u8,
        (((w >> 5) & 0x1F) as u32 * 255 / 31) as u8,
        (((w >> 10) & 0x1F) as u32 * 255 / 31) as u8,
    ]
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
        if scr.len() >= 8192 { 512 } else { 256 }
    } else {
        BG2D_W
    };
    let out_h = if sky { map_h2 as usize } else { BG2D_H };

    let mut rgba = vec![0u8; out_w * out_h * 4];

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
                    }
                }
            }

            let px = &mut rgba[row_off + x * 4..row_off + x * 4 + 4];
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
            px[3] = 255;
        }
    }

    Some((rgba, out_w, out_h))
}

/// CPU half of `build_title_texture`: compose the title screen (BG2 CP
/// backdrop + BG3 TI-3 logo) into a bottom-up 256x224 RGBA image.
pub fn compose_title(
    ti_cgx: &[u8],
    ti_scr: &[u8],
    cp_cgx: &[u8],
    cp_scr: &[u8],
    col: &[u8],
) -> Option<Vec<u8>> {
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

    let mut rgba = vec![0u8; BG2D_W * BG2D_H * 4];

    for y in 0..BG2D_H {
        // Flip vertically so GL row 0 = picture bottom (standard UVs)
        let row_off = (BG2D_H - 1 - y) * BG2D_W * 4;

        let by2 = (y + 1) % 256; // BG2: bg2Yscroll 257 -> effective +1
        let by3 = (y + 9) % 256; // BG3: setbg3vofs 9

        for x in 0..BG2D_W {
            let mut rgb = [0u8; 3]; // pal0palette forced to black

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
                        rgb = cgram_color(col, pal * 16 + v as usize);
                    }
                }
            }

            // --- BG3 (TI-3 logo, 2bpp) over the top ---
            {
                let me = ((by3 / 8) * 32 + (x / 8)) * 2;
                let e = ti_scr[me] as u16 | ((ti_scr[me + 1] as u16) << 8);
                let tile = (e & 0x3FF) as usize;
                let pal = ((e >> 10) & 7) as usize;
                let mut r = by3 & 7;
                let mut c = x & 7;
                if e & 0x8000 != 0 {
                    r = 7 - r;
                }
                if e & 0x4000 != 0 {
                    c = 7 - c;
                }
                if tile < n_ti {
                    let v = ti_px[tile * 64 + r * 8 + c];
                    if v != 0 {
                        rgb = cgram_color(col, pal * 4 + v as usize);
                    }
                }
            }

            let px = &mut rgba[row_off + x * 4..row_off + x * 4 + 4];
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
            px[3] = 255;
        }
    }

    Some(rgba)
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

fn upload_rgba(gl: &glow::Context, rgba: &[u8], w: usize, h: usize, wrap: i32) -> Option<glow::Texture> {
    unsafe {
        let tex = gl.create_texture().ok()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            w as i32,
            h as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(rgba)),
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, wrap);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, wrap);
        gl.bind_texture(glow::TEXTURE_2D, None);
        Some(tex)
    }
}

pub struct Bg2d {
    base_dir: PathBuf,
    title_tex: Option<glow::Texture>,
    def_tex: Vec<Option<glow::Texture>>,
    def_tried: Vec<bool>,
    /// Tilemap pixel size for sky (camera-coupled) textures; 0 for static
    /// pre-baked 256x224 composites.
    def_map_w: Vec<i32>,
    def_map_h: Vec<i32>,
    vao: Option<glow::VertexArray>,
    vbo: Option<glow::Buffer>,
    warned_bgs: u64,
    // g_currentbg staleness workaround state (statics in Bg2d_Render).
    prev_map: u32,
    bg_at_map_start: u16,
}

impl Bg2d {
    pub fn new(gl: &glow::Context, base_dir: &Path) -> Self {
        let n = BG_DEFS.len();
        let mut bg = Bg2d {
            base_dir: base_dir.to_path_buf(),
            title_tex: None,
            def_tex: vec![None; n],
            def_tried: vec![false; n],
            def_map_w: vec![0; n],
            def_map_h: vec![0; n],
            vao: None,
            vbo: None,
            warned_bgs: 0,
            prev_map: 0xFFFF_FFFF,
            bg_at_map_start: 0,
        };
        bg.build_title_texture(gl);

        unsafe {
            // Dynamic quad (pos.xy + uv.xy)
            let vao = gl.create_vertex_array().ok();
            let vbo = gl.create_buffer().ok();
            gl.bind_vertex_array(vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, vbo);
            gl.buffer_data_size(glow::ARRAY_BUFFER, 4 * 4 * 4, glow::DYNAMIC_DRAW);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 4 * 4, 0);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 4 * 4, 2 * 4);
            gl.enable_vertex_attrib_array(1);
            gl.bind_vertex_array(None);
            bg.vao = vao;
            bg.vbo = vbo;
        }
        bg
    }

    pub fn has_title(&self) -> bool {
        self.title_tex.is_some()
    }

    fn build_title_texture(&mut self, gl: &glow::Context) {
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
        match compose_title(&ti_cgx, &ti_scr, &cp_cgx, &cp_scr, &col) {
            Some(rgba) => {
                self.title_tex =
                    upload_rgba(gl, &rgba, BG2D_W, BG2D_H, glow::CLAMP_TO_EDGE as i32);
            }
            None => eprintln!("Bg2d: title assets missing/short, using fallback backdrop"),
        }
    }

    fn build_bg_texture(&mut self, gl: &glow::Context, idx: usize) {
        let def = &BG_DEFS[idx];
        let cgx = load_file(&self.base_dir, def.cgx);
        let scr = load_file(&self.base_dir, def.scr);
        let col = load_file(&self.base_dir, def.col);
        let cgx3 = def.cgx3.and_then(|p| load_file(&self.base_dir, p));
        let scr3 = def.scr3.and_then(|p| load_file(&self.base_dir, p));

        let (Some(cgx), Some(scr), Some(col)) = (cgx, scr, col) else {
            eprintln!("Bg2d: {} assets missing/short, using fallback backdrop", def.name);
            return;
        };

        match compose_bg(
            &cgx,
            &scr,
            &col,
            cgx3.as_deref(),
            scr3.as_deref(),
            def.vofs,
            def.vofs3,
            def.sky,
        ) {
            Some((rgba, out_w, out_h)) => {
                let wrap = if def.sky {
                    glow::REPEAT as i32
                } else {
                    glow::CLAMP_TO_EDGE as i32
                };
                self.def_tex[idx] = upload_rgba(gl, &rgba, out_w, out_h, wrap);
                if def.sky {
                    self.def_map_w[idx] = out_w as i32;
                    self.def_map_h[idx] = out_h as i32;
                }
            }
            None => {
                eprintln!("Bg2d: {} assets missing/short, using fallback backdrop", def.name);
            }
        }
    }

    /// Lazily build the texture for a bg id; returns the def index or None.
    fn layer_index_for_id(&mut self, gl: &glow::Context, id: u8) -> Option<usize> {
        let idx = BG_DEFS.iter().position(|d| d.id == id)?;
        if !self.def_tried[idx] {
            self.def_tried[idx] = true;
            self.build_bg_texture(gl, idx);
        }
        Some(idx)
    }

    fn set_ortho(&self, gl: &glow::Context, backend: &GlBackend, w: i32, h: i32) {
        let mut ortho = [0.0f32; 16];
        ortho[0] = 2.0 / w as f32;
        ortho[5] = 2.0 / h as f32;
        ortho[10] = -1.0;
        ortho[12] = -1.0;
        ortho[13] = -1.0;
        ortho[15] = 1.0;
        gl_backend::set_mat4(gl, backend.hud, "uProj", &ortho);

        let mut model = [0.0f32; 16];
        model[0] = 1.0;
        model[5] = 1.0;
        model[10] = 1.0;
        model[15] = 1.0;
        gl_backend::set_mat4(gl, backend.hud, "uModel", &model);
    }

    fn draw_quad_uv(
        &self,
        gl: &glow::Context,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        let verts: [f32; 16] = [
            x, y, u0, v0,
            x + w, y, u1, v0,
            x + w, y + h, u1, v1,
            x, y + h, u0, v1,
        ];
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo);
            let bytes: &[u8] =
                core::slice::from_raw_parts(verts.as_ptr() as *const u8, verts.len() * 4);
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
            gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
        }
    }

    fn draw_quad(&self, gl: &glow::Context, x: f32, y: f32, w: f32, h: f32) {
        self.draw_quad_uv(gl, x, y, w, h, 0.0, 0.0, 1.0, 1.0);
    }

    /// Starfield-ish dark gradient fallback (deterministic star placement).
    fn draw_fallback_backdrop(&self, gl: &glow::Context, backend: &GlBackend, w: i32, h: i32) {
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 0);

        // Vertical gradient: deep space black at top -> dark blue near bottom
        let bands = 8;
        for i in 0..bands {
            let t = i as f32 / (bands - 1) as f32;
            gl_backend::set_vec4(
                gl,
                backend.hud,
                "uColor",
                0.01 + 0.03 * (1.0 - t),
                0.01 + 0.03 * (1.0 - t),
                0.05 + 0.10 * (1.0 - t),
                1.0,
            );
            let band_h = h as f32 / bands as f32;
            self.draw_quad(gl, 0.0, band_h * i as f32, w as f32, band_h + 1.0);
        }

        // Stars: deterministic pseudo-random spread
        gl_backend::set_vec4(gl, backend.hud, "uColor", 0.85, 0.85, 0.95, 1.0);
        let mut seed: u32 = 0x123_4567;
        let sx = w as f32 / 256.0;
        let sy = h as f32 / 224.0;
        for i in 0..64 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let px = ((seed >> 8) & 0xFF) as i32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let py = ((seed >> 8) % 224) as i32;
            let size = if i & 7 == 0 { 2.0 } else { 1.0 };
            self.draw_quad(gl, px as f32 * sx, py as f32 * sy, size * sx, size * sy);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_layer_texture(
        &self,
        gl: &glow::Context,
        backend: &GlBackend,
        tex: Option<glow::Texture>,
        w: i32,
        h: i32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
    ) {
        let Some(tex) = tex else {
            self.draw_fallback_backdrop(gl, backend, w, h);
            return;
        };
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 1);
        gl_backend::set_int(gl, backend.hud, "uTexture", 0);
        gl_backend::set_vec4(gl, backend.hud, "uColor", 1.0, 1.0, 1.0, 1.0);
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        }
        self.draw_quad_uv(gl, 0.0, 0.0, w as f32, h as f32, u0, v0, u1, v1);
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 0);
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
        let mut hofs = inputs.bg2_xscroll as f32;

        if with_camera {
            let cam = transform.render_camera();
            // Fractional signed SNES angle units (render-frame interpolated, so
            // the painted horizon glides with the 3D view instead of stepping
            // once per 20 Hz tick — see Transform::render_camera_angles_f).
            let (rx, ry) = transform.render_camera_angles_f(); // pitch +up, yaw +right

            // Vertical: use the port projection's real focal length so the
            // painted horizon sits exactly on the 3D ground plane's
            // vanishing line (focal*tan(pitch)).
            let focal = transform.projection()[5] * (BG2D_H as f32 * 0.5);
            let mut vdelta = -focal * (rx * (std::f32::consts::PI / 128.0)).tan();
            if !inputs.nomax_bg2_yscroll {
                vdelta = vdelta.clamp(-56.0, 232.0);
            }
            vofs += vdelta;

            // SNES-vs-port horizon alignment (the missing half of the
            // horizon lock). The BGS.ASM bg2Yscroll bases (def.vofs) were
            // tuned so the painted horizon fell on the SNES 3D engine's
            // horizon, which sat at scanline ~130 of the 224-line frame
            // (every von/hon bg shares that one engine horizon: measured
            // green-ground row - vofs = 132 for ST-P/bg_1_1c, 128 for
            // 3-3/bg_3_3a). The port's symmetric 60-deg frustum instead
            // vanishes the y=0 ground plane at screen centre (row 112) at
            // pitch 0 — geometrically exact but 18 lines above the SNES
            // horizon the texture was painted for. Without correcting that,
            // ground objects (worldy 0, drawn on the y=0 plane) converge on
            // the mountain-haze band while the painted green ground sits ~18
            // lines lower, so they read as floating above the terrain.
            // Shift the window up by the difference so the painted horizon
            // locks to the port's y=0 vanishing line (bg2d.c calcbgscroll_l
            // locked only the per-pitch slope, never this base offset).
            const SNES_HORIZON_ROW: f32 = 130.0;
            vofs += SNES_HORIZON_ROW - BG2D_H as f32 * 0.5;

            // Horizontal: m_scrollxoff = bg2Xscroll + yaw*8 px, plus the
            // `hofmode rotate` HDMA base of worldx>>3.
            hofs += ry * 8.0 + (cam.x >> 16) as f32 / 8.0;
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
        gl: &glow::Context,
        backend: &GlBackend,
        transform: &Transform,
        inputs: &FrameInputs,
        screen_width: i32,
        screen_height: i32,
    ) {
        let bg_active = inputs.bgflags & BGF_BG != 0;
        let mut draw = false;
        let mut couple = false; // apply the per-frame camera scroll coupling
        let mut idx: Option<usize> = None; // BG_DEFS index (None: title/fallback)
        let mut tex: Option<glow::Texture> = None; // None -> fallback backdrop

        match inputs.game_state {
            GameState::Title => {
                // Title map's setbg opcode selects BG_TITLE; draw the logo
                // layer even if the map script hasn't reached setbg yet.
                draw = true;
                tex = self.title_tex;
            }
            GameState::PlanetSelect | GameState::Briefing => {
                // PLANETS.ASM map screen backdrop.
                draw = true;
                idx = self.layer_index_for_id(gl, BG2D_ID_MAP);
            }
            GameState::Continue => {
                // bg_cont_1 controller screen backdrop.
                draw = true;
                idx = self.layer_index_for_id(gl, 42);
            }
            GameState::Ending => {
                draw = true;
                idx = self.layer_index_for_id(gl, (inputs.currentbg & 63) as u8);
            }
            _ if bg_active || inputs.game_state == GameState::Playing => {
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
                    if let Some(&(_, bg)) =
                        MAP_DEFAULT_BG.iter().find(|(m, _)| *m == inputs.newmap)
                    {
                        id = bg;
                    }
                }

                if id == BG2D_ID_TITLE {
                    tex = self.title_tex;
                } else {
                    idx = self.layer_index_for_id(gl, id);
                    couple = true; // gameplay: slave sky layers to the camera
                    let missing = match idx {
                        None => true,
                        Some(i) => self.def_tex[i].is_none(),
                    };
                    if missing && self.warned_bgs & (1u64 << id) == 0 {
                        self.warned_bgs |= 1u64 << id;
                        println!(
                            "Bg2d: no layer data for bg id {id}, using fallback backdrop"
                        );
                    }
                }
            }
            _ => {}
        }

        if !draw {
            return;
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

        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.disable(glow::BLEND);
            gl.use_program(Some(backend.hud));
        }
        self.set_ortho(gl, backend, screen_width, screen_height);
        unsafe {
            gl.bind_vertex_array(self.vao);
        }

        self.draw_layer_texture(gl, backend, tex, screen_width, screen_height, u0, v0, u1, v1);

        unsafe {
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
            gl.enable(glow::DEPTH_TEST);
        }
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.title_tex.take() {
                gl.delete_texture(t);
            }
            for t in self.def_tex.iter_mut() {
                if let Some(t) = t.take() {
                    gl.delete_texture(t);
                }
            }
            if let Some(vao) = self.vao.take() {
                gl.delete_vertex_array(vao);
            }
            if let Some(vbo) = self.vbo.take() {
                gl.delete_buffer(vbo);
            }
        }
    }
}
