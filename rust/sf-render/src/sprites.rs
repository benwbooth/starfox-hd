//! SNES OBJ sprite decoder and renderer.
//!
//! Port (C oracle): `src/renderer/sprites.c`. Decodes 4bpp tile data from
//! OBJ-1.CGX and palettes from OBJ-1.COL, builds an indexed (GL_R8) atlas,
//! and renders sprites as textured quads via the HUD shader's
//! palette-lookup mode (uUseTexture == 2). Also owns the radio portrait
//! atlas (FACE.CGX) and the in-game bitmap palette (NIGHT.COL row 0).

use glow::HasContext;
use std::path::Path;

use crate::gl_backend::{self, GlBackend};

// Sprite flip flags (match SNES OAM bits 14-15)
pub const SPR_HFLIP: u8 = 0x01;
pub const SPR_VFLIP: u8 = 0x02;

// OAM palette IDs (0-7, mapped to COL file rows 8-15)
pub const SPR_PAL_DEFAULT: u8 = 0;
pub const SPR_PAL_BLUE: u8 = 1;
pub const SPR_PAL_FOX: u8 = 2;
pub const SPR_PAL_CROSS: u8 = 4;

const ATLAS_COLS: usize = 16;
const ATLAS_ROWS: usize = 9;
const TILE_W: usize = 8;
const TILE_H: usize = 8;
const ATLAS_W: usize = ATLAS_COLS * TILE_W; // 128
const ATLAS_H: usize = ATLAS_ROWS * TILE_H; // 72
const MAX_TILES: usize = ATLAS_COLS * ATLAS_ROWS; // 144
const NUM_PALETTES: usize = 8;
const COLORS_PER_PAL: usize = 16;

const MAX_SPRITE_QUEUE: usize = 256;

// Radio portrait faces (FACE.CGX): 18 frames, 32x40 4bpp, 640 bytes each
// stored as 4 columns of 5 tiles (column-major).
const FACE_FRAMES: usize = 18;
const FACE_W: usize = 32;
const FACE_H: usize = 40;
const FACE_FRAME_SIZE: usize = 640;
const FACE_ATLAS_W: usize = FACE_FRAMES * FACE_W; // 576
const FACE_ATLAS_H: usize = FACE_H;

#[derive(Clone, Copy)]
struct SpriteCmd {
    tile: i32,
    x: i32,
    y: i32,
    palette: i32,
    flags: u8,
}

/// SNES 4bpp planar tile -> 64 linear palette indices.
pub fn decode_4bpp_tile(src: &[u8], dst8x8: &mut [u8; 64]) {
    for row in 0..8 {
        let bp0 = src[row * 2];
        let bp1 = src[row * 2 + 1];
        let bp2 = src[16 + row * 2];
        let bp3 = src[16 + row * 2 + 1];
        for bit in (0..8).rev() {
            let p = ((bp0 >> bit) & 1)
                | (((bp1 >> bit) & 1) << 1)
                | (((bp2 >> bit) & 1) << 2)
                | (((bp3 >> bit) & 1) << 3);
            dst8x8[(7 - bit) + row * 8] = p;
        }
    }
}

/// BGR555 word -> normalized RGBA (alpha 1).
pub fn decode_bgr555_bytes(src: &[u8]) -> [f32; 4] {
    let word = src[0] as u16 | ((src[1] as u16) << 8);
    [
        ((word) & 0x1F) as f32 / 31.0,
        ((word >> 5) & 0x1F) as f32 / 31.0,
        ((word >> 10) & 0x1F) as f32 / 31.0,
        1.0,
    ]
}

pub struct Sprites {
    atlas_tex: Option<glow::Texture>,
    face_tex: Option<glow::Texture>,
    quad_vao: Option<glow::VertexArray>,
    quad_vbo: Option<glow::Buffer>,
    num_tiles: usize,
    screen_w: i32,
    screen_h: i32,
    palettes: [[[f32; 4]; COLORS_PER_PAL]; NUM_PALETTES],
    /// In-game bitmap palette (NIGHT.COL row 0).
    bitmap_palette: [[f32; 4]; COLORS_PER_PAL],
    queue: Vec<SpriteCmd>,
}

impl Sprites {
    pub fn new(gl: &glow::Context, base_dir: &Path) -> Self {
        let mut s = Sprites {
            atlas_tex: None,
            face_tex: None,
            quad_vao: None,
            quad_vbo: None,
            num_tiles: 0,
            screen_w: 1280,
            screen_h: 720,
            palettes: [[[0.0; 4]; COLORS_PER_PAL]; NUM_PALETTES],
            bitmap_palette: [[0.0; 4]; COLORS_PER_PAL],
            queue: Vec::with_capacity(MAX_SPRITE_QUEUE),
        };

        // ---- Load tile data from OBJ-1.CGX (raw 4bpp, no header) ----
        let cgx_path = base_dir.join("data/sprites/OBJ-1.CGX");
        match std::fs::read(&cgx_path) {
            Err(_) => {
                eprintln!("Sprites: cannot open {}", cgx_path.display());
                return s;
            }
            Ok(tile_rom) => {
                s.num_tiles = (tile_rom.len() / 32).min(MAX_TILES);

                // Atlas: each pixel stores 0-15 palette index, uploaded as
                // GL_R8 so the shader reconstructs int(r * 255 + 0.5).
                let mut atlas = vec![0u8; ATLAS_W * ATLAS_H];
                let mut tile8x8 = [0u8; 64];
                for t in 0..s.num_tiles {
                    decode_4bpp_tile(&tile_rom[t * 32..], &mut tile8x8);
                    let col = t % ATLAS_COLS;
                    let row = t / ATLAS_COLS;
                    for py in 0..8 {
                        for px in 0..8 {
                            let ax = col * TILE_W + px;
                            let ay = row * TILE_H + py;
                            atlas[ax + ay * ATLAS_W] = tile8x8[px + py * 8];
                        }
                    }
                }

                unsafe {
                    let tex = gl.create_texture().ok();
                    gl.bind_texture(glow::TEXTURE_2D, tex);
                    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                    gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::R8 as i32,
                        ATLAS_W as i32,
                        ATLAS_H as i32,
                        0,
                        glow::RED,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(&atlas)),
                    );
                    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
                    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
                    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
                    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
                    gl.bind_texture(glow::TEXTURE_2D, None);
                    s.atlas_tex = tex;
                }
            }
        }

        // ---- Load palettes from OBJ-1.COL (sprite palettes rows 8-15) ----
        let col_path = base_dir.join("data/sprites/OBJ-1.COL");
        match std::fs::read(&col_path) {
            Err(_) => eprintln!("Sprites: cannot open {}", col_path.display()),
            Ok(col_data) => {
                if col_data.len() >= 512 {
                    for pal in 0..NUM_PALETTES {
                        let row_off = (8 + pal) * 32;
                        for c in 0..COLORS_PER_PAL {
                            s.palettes[pal][c] =
                                decode_bgr555_bytes(&col_data[row_off + c * 2..]);
                        }
                        // Color 0 of every palette is transparent
                        s.palettes[pal][0][3] = 0.0;
                    }
                }
            }
        }

        // ---- Load in-game bitmap palette (NIGHT.COL row 0) ----
        let night_path = base_dir.join("data/sprites/NIGHT.COL");
        match std::fs::read(&night_path) {
            Err(_) => eprintln!("Sprites: cannot open {}", night_path.display()),
            Ok(row0) => {
                if row0.len() >= 32 {
                    for c in 0..COLORS_PER_PAL {
                        s.bitmap_palette[c] = decode_bgr555_bytes(&row0[c * 2..]);
                    }
                }
            }
        }

        // ---- Load portrait frames (FACE.CGX) into an RGBA atlas ----
        let face_path = base_dir.join("data/sprites/FACE.CGX");
        match std::fs::read(&face_path) {
            Err(_) => eprintln!("Sprites: cannot open {}", face_path.display()),
            Ok(face_rom) => {
                if face_rom.len() < FACE_FRAMES * FACE_FRAME_SIZE {
                    eprintln!("Sprites: FACE.CGX short read ({} bytes)", face_rom.len());
                } else {
                    let mut face_atlas = vec![0u8; FACE_ATLAS_W * FACE_ATLAS_H * 4];
                    let mut tile8x8 = [0u8; 64];
                    for fr in 0..FACE_FRAMES {
                        // Frame layout: 4 columns of 5 tiles, column-major
                        // (mcopyface copies 160 bytes per 8px-wide column).
                        for col4 in 0..4 {
                            for trow in 0..5 {
                                let src =
                                    &face_rom[fr * FACE_FRAME_SIZE + col4 * 160 + trow * 32..];
                                decode_4bpp_tile(src, &mut tile8x8);
                                for py in 0..8 {
                                    for px in 0..8 {
                                        let idx = tile8x8[px + py * 8] as usize;
                                        let ax = fr * FACE_W + col4 * 8 + px;
                                        let ay = trow * 8 + py;
                                        let dst = &mut face_atlas
                                            [(ay * FACE_ATLAS_W + ax) * 4..][..4];
                                        // Color 0 = transparent (3D shows through)
                                        dst[0] =
                                            (s.bitmap_palette[idx][0] * 255.0) as u8;
                                        dst[1] =
                                            (s.bitmap_palette[idx][1] * 255.0) as u8;
                                        dst[2] =
                                            (s.bitmap_palette[idx][2] * 255.0) as u8;
                                        dst[3] = if idx != 0 { 255 } else { 0 };
                                    }
                                }
                            }
                        }
                    }
                    unsafe {
                        let tex = gl.create_texture().ok();
                        gl.bind_texture(glow::TEXTURE_2D, tex);
                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            FACE_ATLAS_W as i32,
                            FACE_ATLAS_H as i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(&face_atlas)),
                        );
                        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
                        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
                        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
                        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
                        gl.bind_texture(glow::TEXTURE_2D, None);
                        s.face_tex = tex;
                    }
                }
            }
        }

        // ---- Create unit quad VAO (shared with HUD) ----
        let quad: [f32; 16] = [
            0.0, 0.0, 0.0, 0.0,
            1.0, 0.0, 1.0, 0.0,
            1.0, 1.0, 1.0, 1.0,
            0.0, 1.0, 0.0, 1.0,
        ];
        unsafe {
            let vao = gl.create_vertex_array().ok();
            let vbo = gl.create_buffer().ok();
            gl.bind_vertex_array(vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, vbo);
            let bytes: &[u8] =
                core::slice::from_raw_parts(quad.as_ptr() as *const u8, quad.len() * 4);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 4 * 4, 0);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 4 * 4, 2 * 4);
            gl.enable_vertex_attrib_array(1);
            gl.bind_vertex_array(None);
            s.quad_vao = vao;
            s.quad_vbo = vbo;
        }

        s
    }

    pub fn set_screen_size(&mut self, w: i32, h: i32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    /// Queue an 8x8 sprite (SNES 256x224 coordinates).
    pub fn draw8(&mut self, tile: i32, x: i32, y: i32, palette: u8, flags: u8) {
        if self.queue.len() >= MAX_SPRITE_QUEUE {
            return;
        }
        if tile < 0 || tile as usize >= self.num_tiles {
            return;
        }
        self.queue.push(SpriteCmd {
            tile,
            x,
            y,
            palette: palette as i32,
            flags,
        });
    }

    /// Flush the sprite queue (mirror of `Sprites_RenderHUD`).
    pub fn render_hud(&mut self, gl: &glow::Context, backend: &GlBackend) {
        if self.queue.is_empty() || self.atlas_tex.is_none() {
            self.queue.clear();
            return;
        }

        unsafe {
            gl.use_program(Some(backend.hud));
        }

        let sw = self.screen_w as f32;
        let sh = self.screen_h as f32;
        let mut ortho = [0.0f32; 16];
        ortho[0] = 2.0 / sw;
        ortho[5] = 2.0 / sh;
        ortho[10] = -1.0;
        ortho[12] = -1.0;
        ortho[13] = -1.0;
        ortho[15] = 1.0;
        gl_backend::set_mat4(gl, backend.hud, "uProj", &ortho);

        // Vertices are in screen space; uModel must be identity.
        let mut model = [0.0f32; 16];
        model[0] = 1.0;
        model[5] = 1.0;
        model[10] = 1.0;
        model[15] = 1.0;
        gl_backend::set_mat4(gl, backend.hud, "uModel", &model);

        let sx = sw / 256.0;
        let sy = sh / 224.0;

        gl_backend::set_int(gl, backend.hud, "uUseTexture", 2);
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.atlas_tex);
        }
        gl_backend::set_int(gl, backend.hud, "uTexture", 0);
        unsafe {
            gl.bind_vertex_array(self.quad_vao);
        }

        let mut cur_pal: i32 = -1;
        let queue = std::mem::take(&mut self.queue);
        for cmd in &queue {
            if cmd.palette != cur_pal {
                cur_pal = cmd.palette;
                let pi = if cmd.palette < 0 || cmd.palette as usize >= NUM_PALETTES {
                    0
                } else {
                    cmd.palette as usize
                };
                unsafe {
                    if let Some(loc) = gl.get_uniform_location(backend.hud, "uPalette") {
                        let flat: &[f32] = core::slice::from_raw_parts(
                            self.palettes[pi].as_ptr() as *const f32,
                            COLORS_PER_PAL * 4,
                        );
                        gl.uniform_4_f32_slice(Some(&loc), flat);
                    }
                }
            }

            let acol = (cmd.tile as usize) % ATLAS_COLS;
            let arow = (cmd.tile as usize) / ATLAS_COLS;
            let mut u0 = (acol * TILE_W) as f32 / ATLAS_W as f32;
            let mut v0 = (arow * TILE_H) as f32 / ATLAS_H as f32;
            let mut u1 = (acol * TILE_W + TILE_W) as f32 / ATLAS_W as f32;
            let mut v1 = (arow * TILE_H + TILE_H) as f32 / ATLAS_H as f32;

            if cmd.flags & SPR_HFLIP != 0 {
                std::mem::swap(&mut u0, &mut u1);
            }
            if cmd.flags & SPR_VFLIP != 0 {
                std::mem::swap(&mut v0, &mut v1);
            }

            // SNES Y=0 at top -> OpenGL Y=0 at bottom
            let px = cmd.x as f32 * sx;
            let py = sh - (cmd.y + 8) as f32 * sy;
            let pw = 8.0 * sx;
            let ph = 8.0 * sy;

            let verts: [f32; 16] = [
                px, py, u0, v1,
                px + pw, py, u1, v1,
                px + pw, py + ph, u1, v0,
                px, py + ph, u0, v0,
            ];
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, self.quad_vbo);
                let bytes: &[u8] =
                    core::slice::from_raw_parts(verts.as_ptr() as *const u8, verts.len() * 4);
                gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
                gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
            }
        }

        unsafe {
            gl.bind_vertex_array(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }

        // Reset for next frame (keep the allocation).
        self.queue = queue;
        self.queue.clear();
    }

    /// In-game bitmap palette lookup (NIGHT.COL row 0), alpha forced to 1.
    pub fn bitmap_color(&self, index: usize) -> [f32; 4] {
        let index = if index >= COLORS_PER_PAL { 0 } else { index };
        let c = self.bitmap_palette[index];
        [c[0], c[1], c[2], 1.0]
    }

    /// Draw a radio portrait frame (0-17) at SNES screen coordinates.
    pub fn draw_face(&self, gl: &glow::Context, backend: &GlBackend, frame: i32, x: i32, y: i32) {
        let Some(face_tex) = self.face_tex else {
            return;
        };
        if frame < 0 || frame as usize >= FACE_FRAMES {
            return;
        }

        unsafe {
            gl.use_program(Some(backend.hud));
        }

        let sw = self.screen_w as f32;
        let sh = self.screen_h as f32;
        let mut ortho = [0.0f32; 16];
        ortho[0] = 2.0 / sw;
        ortho[5] = 2.0 / sh;
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

        gl_backend::set_vec4(gl, backend.hud, "uColor", 1.0, 1.0, 1.0, 1.0);
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 1);
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(face_tex));
        }
        gl_backend::set_int(gl, backend.hud, "uTexture", 0);

        let sx = sw / 256.0;
        let sy = sh / 224.0;
        let px = x as f32 * sx;
        let py = sh - (y + FACE_H as i32) as f32 * sy;
        let pw = FACE_W as f32 * sx;
        let ph = FACE_H as f32 * sy;

        let u0 = (frame as usize * FACE_W) as f32 / FACE_ATLAS_W as f32;
        let u1 = ((frame as usize + 1) * FACE_W) as f32 / FACE_ATLAS_W as f32;

        let verts: [f32; 16] = [
            px, py, u0, 1.0,
            px + pw, py, u1, 1.0,
            px + pw, py + ph, u1, 0.0,
            px, py + ph, u0, 0.0,
        ];
        unsafe {
            gl.bind_vertex_array(self.quad_vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, self.quad_vbo);
            let bytes: &[u8] =
                core::slice::from_raw_parts(verts.as_ptr() as *const u8, verts.len() * 4);
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
            gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
            gl.bind_vertex_array(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 0);
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.atlas_tex.take() {
                gl.delete_texture(t);
            }
            if let Some(t) = self.face_tex.take() {
                gl.delete_texture(t);
            }
            if let Some(v) = self.quad_vao.take() {
                gl.delete_vertex_array(v);
            }
            if let Some(b) = self.quad_vbo.take() {
                gl.delete_buffer(b);
            }
        }
    }
}
