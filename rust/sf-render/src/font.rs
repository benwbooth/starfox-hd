//! In-game message font (MOJI_2.fon, 16x16 1bpp glyphs).
//!
//! Port (C oracle): `src/renderer/font.c`. 80 glyphs, 2 bytes per row
//! little-endian with bit 15 = leftmost pixel; drawn in an 8x8 SNES cell.

use glow::HasContext;
use std::path::Path;

use crate::gl_backend::{self, GlBackend};

const FONT_GLYPH_PX: usize = 16;
const FONT_NUM_GLYPHS: usize = 80;
const FONT_ATLAS_COLS: usize = 16;
const FONT_ATLAS_ROWS: usize = 5;
const FONT_ATLAS_W: usize = FONT_ATLAS_COLS * FONT_GLYPH_PX; // 256
const FONT_ATLAS_H: usize = FONT_ATLAS_ROWS * FONT_GLYPH_PX; // 80

/// Advance/draw size in SNES units (256x224 reference).
const FONT_CELL: f32 = 8.0;

const GLYPH_SPACE: u8 = 39;

pub struct Font {
    texture: Option<glow::Texture>,
    vao: Option<glow::VertexArray>,
    vbo: Option<glow::Buffer>,
    screen_w: i32,
    screen_h: i32,
    ascii_to_glyph: [u8; 256],
    initialized: bool,
}

fn build_translation_table() -> [u8; 256] {
    let mut t = [GLYPH_SPACE; 256];
    for i in b'A'..=b'Z' {
        t[i as usize] = i - b'A';
    }
    for i in b'a'..=b'z' {
        t[i as usize] = i - b'a';
    }
    for i in b'0'..=b'9' {
        t[i as usize] = 27 + i - b'0';
    }
    t[b'-' as usize] = 26;
    t[b'!' as usize] = 37;
    t[b'?' as usize] = 38;
    t[b' ' as usize] = GLYPH_SPACE;
    t[b'%' as usize] = 64;
    // No period/comma glyphs in this font; '-' is the closest dash mark.
    t[b'.' as usize] = GLYPH_SPACE;
    t[b',' as usize] = GLYPH_SPACE;
    t
}

impl Font {
    pub fn new(gl: &glow::Context, base_dir: &Path) -> Self {
        let mut font = Font {
            texture: None,
            vao: None,
            vbo: None,
            screen_w: 800,
            screen_h: 600,
            ascii_to_glyph: build_translation_table(),
            initialized: false,
        };

        let path = base_dir.join("data/font/MOJI_2.fon");
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Font: cannot open {}", path.display());
                return font;
            }
        };
        if data.len() < FONT_NUM_GLYPHS * 32 {
            eprintln!("Font: short read ({} bytes)", data.len());
            return font;
        }

        // Decode 1bpp 16x16 glyphs into an RGBA atlas (white on transparent)
        let mut atlas = vec![0u8; FONT_ATLAS_W * FONT_ATLAS_H * 4];
        for g in 0..FONT_NUM_GLYPHS {
            let gx = (g % FONT_ATLAS_COLS) * FONT_GLYPH_PX;
            let gy = (g / FONT_ATLAS_COLS) * FONT_GLYPH_PX;
            let glyph = &data[g * 32..g * 32 + 32];
            for row in 0..FONT_GLYPH_PX {
                let bits = glyph[row * 2] as u16 | ((glyph[row * 2 + 1] as u16) << 8);
                for col in 0..FONT_GLYPH_PX {
                    if bits & (0x8000u16 >> col) == 0 {
                        continue;
                    }
                    let idx = ((gy + row) * FONT_ATLAS_W + gx + col) * 4;
                    atlas[idx] = 255;
                    atlas[idx + 1] = 255;
                    atlas[idx + 2] = 255;
                    atlas[idx + 3] = 255;
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
                glow::RGBA as i32,
                FONT_ATLAS_W as i32,
                FONT_ATLAS_H as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&atlas)),
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
            gl.bind_texture(glow::TEXTURE_2D, None);
            font.texture = tex;

            // Single reusable quad (pos.xy + uv.xy), updated per glyph
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
            font.vao = vao;
            font.vbo = vbo;
        }

        font.initialized = true;
        font
    }

    pub fn set_screen_size(&mut self, w: i32, h: i32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    /// Mirror of `Font_DrawString` (bottom-left origin, height-scaled).
    pub fn draw_string(
        &self,
        gl: &glow::Context,
        backend: &GlBackend,
        x: i32,
        y: i32,
        text: &str,
        r: f32,
        g: f32,
        b: f32,
    ) {
        if !self.initialized || text.is_empty() {
            return;
        }
        let Some(texture) = self.texture else {
            return;
        };

        // Scale: SNES reference is 256x224; scale by screen height
        let scale = self.screen_h as f32 / 224.0;
        let gw = FONT_CELL * scale;
        let gh = FONT_CELL * scale;

        unsafe {
            gl.use_program(Some(backend.hud));
        }

        let mut ortho = [0.0f32; 16];
        ortho[0] = 2.0 / self.screen_w as f32;
        ortho[5] = 2.0 / self.screen_h as f32;
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

        gl_backend::set_vec4(gl, backend.hud, "uColor", r, g, b, 1.0);
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 1);
        gl_backend::set_int(gl, backend.hud, "uTexture", 0);

        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.bind_vertex_array(self.vao);
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo);
        }

        let mut cx = x as f32 * scale;
        let cy = y as f32 * scale;

        for &byte in text.as_bytes() {
            let mut glyph = self.ascii_to_glyph[byte as usize];
            if glyph as usize >= FONT_NUM_GLYPHS {
                glyph = GLYPH_SPACE;
            }

            if glyph != GLYPH_SPACE {
                let u0 = (glyph as usize % FONT_ATLAS_COLS * FONT_GLYPH_PX) as f32
                    / FONT_ATLAS_W as f32;
                let v0 = (glyph as usize / FONT_ATLAS_COLS * FONT_GLYPH_PX) as f32
                    / FONT_ATLAS_H as f32;
                let u1 = u0 + FONT_GLYPH_PX as f32 / FONT_ATLAS_W as f32;
                let v1 = v0 + FONT_GLYPH_PX as f32 / FONT_ATLAS_H as f32;

                // Atlas row 0 is the glyph top; it is uploaded at GL v=0
                // (texture bottom), so the quad's TOP edge samples v0.
                let verts: [f32; 16] = [
                    cx, cy, u0, v1,           // bottom-left
                    cx + gw, cy, u1, v1,      // bottom-right
                    cx + gw, cy + gh, u1, v0, // top-right
                    cx, cy + gh, u0, v0,      // top-left
                ];
                unsafe {
                    let bytes: &[u8] = core::slice::from_raw_parts(
                        verts.as_ptr() as *const u8,
                        verts.len() * 4,
                    );
                    gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, bytes);
                    gl.draw_arrays(glow::TRIANGLE_FAN, 0, 4);
                }
            }

            cx += gw; // fixed-width advance
        }

        unsafe {
            gl.bind_vertex_array(None);
        }
        gl_backend::set_int(gl, backend.hud, "uUseTexture", 0);
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    /// Mirror of `Font_DrawNumber` (right-aligned decimal).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_number(
        &self,
        gl: &glow::Context,
        backend: &GlBackend,
        x: i32,
        y: i32,
        value: i32,
        digits: usize,
    ) {
        if !self.initialized {
            return;
        }
        let digits = digits.min(15);
        let mut buf: Vec<u8> = Vec::with_capacity(16);
        let mut v = value.unsigned_abs();
        loop {
            buf.push(b'0' + (v % 10) as u8);
            v /= 10;
            if v == 0 || buf.len() >= 15 {
                break;
            }
        }
        while buf.len() < digits {
            buf.push(b' ');
        }
        buf.reverse();
        let s = String::from_utf8(buf).unwrap_or_default();
        self.draw_string(gl, backend, x, y, &s, 1.0, 1.0, 1.0);
    }

    pub fn destroy(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.texture.take() {
                gl.delete_texture(t);
            }
            if let Some(v) = self.vao.take() {
                gl.delete_vertex_array(v);
            }
            if let Some(b) = self.vbo.take() {
                gl.delete_buffer(b);
            }
        }
        self.initialized = false;
    }
}
