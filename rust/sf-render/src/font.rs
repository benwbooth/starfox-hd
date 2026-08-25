//! In-game message font (MOJI_2.fon, 16x16 1bpp glyphs).
//!
//! Port (C oracle): `src/renderer/font.c`. 80 glyphs, 2 bytes per row
//! little-endian with bit 15 = leftmost pixel; drawn in an 8x8 SNES cell.

use std::path::Path;

use crate::gpu::{Gpu, TextureId, Vertex2};

const FONT_GLYPH_PX: usize = 16;
const FONT_NUM_GLYPHS: usize = 80;
const FONT_ATLAS_COLS: usize = 16;
const FONT_ATLAS_ROWS: usize = 5;
const FONT_ATLAS_W: usize = FONT_ATLAS_COLS * FONT_GLYPH_PX; // 256
const FONT_ATLAS_H: usize = FONT_ATLAS_ROWS * FONT_GLYPH_PX; // 80

const SOURCE_MESSAGE_FONT: &[u8; 3_500] =
    include_bytes!("../../../reference/ultrastarfox/SF/DATA/FONT/MOJI_0.FON");
const SOURCE_MESSAGE_GLYPHS: usize = 140;
const SOURCE_MESSAGE_WIDTH_BYTES: usize = SOURCE_MESSAGE_GLYPHS;
const SOURCE_MESSAGE_GLYPH_WIDTH: usize = 16;
const SOURCE_MESSAGE_GLYPH_HEIGHT: usize = 12;
const SOURCE_MESSAGE_GLYPH_BYTES: usize = SOURCE_MESSAGE_GLYPH_HEIGHT * 2;
const SOURCE_MESSAGE_ATLAS_COLUMNS: usize = 14;
const SOURCE_MESSAGE_ATLAS_ROWS: usize =
    SOURCE_MESSAGE_GLYPHS.div_ceil(SOURCE_MESSAGE_ATLAS_COLUMNS);
const SOURCE_MESSAGE_ATLAS_WIDTH: usize = SOURCE_MESSAGE_ATLAS_COLUMNS * SOURCE_MESSAGE_GLYPH_WIDTH;
const SOURCE_MESSAGE_ATLAS_HEIGHT: usize = SOURCE_MESSAGE_ATLAS_ROWS * SOURCE_MESSAGE_GLYPH_HEIGHT;

/// Advance/draw size in SNES units (256x224 reference).
const FONT_CELL: f32 = 8.0;
const SOURCE_SCREEN_WIDTH: f32 = 256.0;
const SOURCE_SCREEN_HEIGHT: f32 = 224.0;

const GLYPH_SPACE: u8 = 39;

/// Scale authored 256 by 224 screen coordinates onto the active HD target.
/// Bitmap portraits, OAM sprites, and message text all share these axes; using
/// the vertical scale for both text axes moves the text left into the portrait
/// on a widescreen target.
#[inline]
fn source_screen_scales(screen_width: i32, screen_height: i32) -> (f32, f32) {
    (
        screen_width as f32 / SOURCE_SCREEN_WIDTH,
        screen_height as f32 / SOURCE_SCREEN_HEIGHT,
    )
}

pub struct Font {
    texture: Option<TextureId>,
    source_message_texture: TextureId,
    screen_w: i32,
    screen_h: i32,
    ascii_to_glyph: [u8; 256],
    initialized: bool,
}

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

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

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
    pub fn new(gpu: &mut Gpu, base_dir: &Path) -> Self {
        let source_message_texture = create_source_message_texture(gpu);
        let mut font = Font {
            texture: None,
            source_message_texture,
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

        font.texture =
            Some(gpu.create_texture_rgba(FONT_ATLAS_W as u32, FONT_ATLAS_H as u32, &atlas));

        font.initialized = true;
        font
    }

    /// Draw the retail variable-width 12-pixel message font at top-origin
    /// source-screen coordinates.  This is the typed native presentation of
    /// `mfprintstr`/`msprintstr`; the authored glyph pixels are embedded at
    /// build time and are not recovered from machine state.
    pub fn draw_source_message_line(
        &self,
        gpu: &mut Gpu,
        x: i32,
        y_top: i32,
        text: &str,
        color: [f32; 4],
    ) {
        let (scale_x, scale_y) = source_screen_scales(self.screen_w, self.screen_h);
        let proj = ortho(self.screen_w as f32, self.screen_h as f32);
        let mut source_x = x;
        for byte in text.bytes() {
            let glyph = source_message_glyph(byte);
            if byte != b' ' {
                let column = usize::from(glyph) % SOURCE_MESSAGE_ATLAS_COLUMNS;
                let row = usize::from(glyph) / SOURCE_MESSAGE_ATLAS_COLUMNS;
                let u0 = (column * SOURCE_MESSAGE_GLYPH_WIDTH) as f32
                    / SOURCE_MESSAGE_ATLAS_WIDTH as f32;
                let v0 =
                    (row * SOURCE_MESSAGE_GLYPH_HEIGHT) as f32 / SOURCE_MESSAGE_ATLAS_HEIGHT as f32;
                let u1 = u0 + SOURCE_MESSAGE_GLYPH_WIDTH as f32 / SOURCE_MESSAGE_ATLAS_WIDTH as f32;
                let v1 =
                    v0 + SOURCE_MESSAGE_GLYPH_HEIGHT as f32 / SOURCE_MESSAGE_ATLAS_HEIGHT as f32;
                let left = source_x as f32 * scale_x;
                let right = left + SOURCE_MESSAGE_GLYPH_WIDTH as f32 * scale_x;
                let top = self.screen_h as f32 - y_top as f32 * scale_y;
                let bottom = top - SOURCE_MESSAGE_GLYPH_HEIGHT as f32 * scale_y;
                let vertices = [
                    Vertex2 {
                        pos: [left, bottom],
                        uv: [u0, v1],
                    },
                    Vertex2 {
                        pos: [right, bottom],
                        uv: [u1, v1],
                    },
                    Vertex2 {
                        pos: [right, top],
                        uv: [u1, v0],
                    },
                    Vertex2 {
                        pos: [left, top],
                        uv: [u0, v0],
                    },
                ];
                gpu.push_overlay_fan(
                    &vertices,
                    &proj,
                    &IDENTITY,
                    color,
                    1,
                    None,
                    self.source_message_texture,
                );
            }
            source_x += i32::from(source_message_advance(byte));
        }
    }

    pub fn set_screen_size(&mut self, w: i32, h: i32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    fn draw_string_pixels(
        &self,
        gpu: &mut Gpu,
        x: f32,
        y: f32,
        text: &str,
        cell_px: f32,
        color: [f32; 4],
    ) {
        if !self.initialized || text.is_empty() {
            return;
        }
        let Some(texture) = self.texture else {
            return;
        };

        let proj = ortho(self.screen_w as f32, self.screen_h as f32);
        let mut cx = x;
        let cy = y;

        for &byte in text.as_bytes() {
            let mut glyph = self.ascii_to_glyph[byte as usize];
            if glyph as usize >= FONT_NUM_GLYPHS {
                glyph = GLYPH_SPACE;
            }

            if glyph != GLYPH_SPACE {
                let u0 =
                    (glyph as usize % FONT_ATLAS_COLS * FONT_GLYPH_PX) as f32 / FONT_ATLAS_W as f32;
                let v0 =
                    (glyph as usize / FONT_ATLAS_COLS * FONT_GLYPH_PX) as f32 / FONT_ATLAS_H as f32;
                let u1 = u0 + FONT_GLYPH_PX as f32 / FONT_ATLAS_W as f32;
                let v1 = v0 + FONT_GLYPH_PX as f32 / FONT_ATLAS_H as f32;

                // Atlas row 0 is the glyph top; it is uploaded at GL v=0
                // (texture bottom), so the quad's TOP edge samples v0.
                let verts = [
                    Vertex2 {
                        pos: [cx, cy],
                        uv: [u0, v1],
                    }, // bottom-left
                    Vertex2 {
                        pos: [cx + cell_px, cy],
                        uv: [u1, v1],
                    }, // bottom-right
                    Vertex2 {
                        pos: [cx + cell_px, cy + cell_px],
                        uv: [u1, v0],
                    }, // top-right
                    Vertex2 {
                        pos: [cx, cy + cell_px],
                        uv: [u0, v0],
                    }, // top-left
                ];
                gpu.push_overlay_fan(&verts, &proj, &IDENTITY, color, 1, None, texture);
            }

            cx += cell_px; // fixed-width advance
        }
    }

    /// Mirror of `Font_DrawString` (bottom-left origin, height-scaled).
    pub fn draw_string(&self, gpu: &mut Gpu, x: i32, y: i32, text: &str, r: f32, g: f32, b: f32) {
        self.draw_string_sized(gpu, x, y, text, FONT_CELL, [r, g, b, 1.0]);
    }

    /// Draw fixed-width text with a caller-selected cell size in the 256x224
    /// UI reference space. SF2's compact strategic-map HUD uses a narrower
    /// cell than the dialogue font while sharing the same decoded glyphs.
    pub fn draw_string_sized(
        &self,
        gpu: &mut Gpu,
        x: i32,
        y: i32,
        text: &str,
        cell_ref: f32,
        color: [f32; 4],
    ) {
        // SNES UI coordinates use a 256x224 reference frame and preserve
        // vertical scale on widescreen displays.
        let scale = self.screen_h as f32 / 224.0;
        self.draw_string_pixels(
            gpu,
            x as f32 * scale,
            y as f32 * scale,
            text,
            cell_ref * scale,
            color,
        );
    }

    /// Draw MARIO `msprint` text centered on a projected 3D point.
    /// `ndc_*` are the projected model origin and `cell_ref` is the scaled
    /// character width in SNES 256x224 reference pixels.
    pub fn draw_string_scaled_centered_ndc(
        &self,
        gpu: &mut Gpu,
        ndc_x: f32,
        ndc_y: f32,
        text: &str,
        cell_ref: f32,
        color: [f32; 4],
    ) {
        if !ndc_x.is_finite() || !ndc_y.is_finite() || cell_ref <= 0.0 {
            return;
        }
        let scale = self.screen_h as f32 / 224.0;
        let cell_px = cell_ref * scale;
        let center_x = (ndc_x + 1.0) * self.screen_w as f32 * 0.5;
        let center_y = (ndc_y + 1.0) * self.screen_h as f32 * 0.5;
        let x = center_x - text.len() as f32 * cell_px * 0.5;
        let y = center_y - cell_px * 0.5;
        self.draw_string_pixels(gpu, x, y, text, cell_px, color);
    }

    /// Mirror of `Font_DrawNumber` (right-aligned decimal).
    pub fn draw_number(&self, gpu: &mut Gpu, x: i32, y: i32, value: i32, digits: usize) {
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
        self.draw_string(gpu, x, y, &s, 1.0, 1.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{source_screen_scales, SOURCE_SCREEN_HEIGHT};

    #[test]
    fn source_message_uses_the_same_widescreen_axes_as_portraits() {
        const TEST_SCREEN_WIDTH: i32 = 1_280;
        const TEST_SCREEN_HEIGHT: i32 = 720;

        let (scale_x, scale_y) = source_screen_scales(TEST_SCREEN_WIDTH, TEST_SCREEN_HEIGHT);
        assert_eq!(scale_x, 5.0);
        assert!((scale_y - TEST_SCREEN_HEIGHT as f32 / SOURCE_SCREEN_HEIGHT).abs() < f32::EPSILON);

        const PORTRAIT_RIGHT: f32 = 48.0 + 16.0 + 32.0;
        const MESSAGE_LEFT: f32 = 82.0 + 16.0;
        assert!(MESSAGE_LEFT * scale_x > PORTRAIT_RIGHT * scale_x);
    }
}

fn create_source_message_texture(gpu: &mut Gpu) -> TextureId {
    let mut atlas = vec![0u8; SOURCE_MESSAGE_ATLAS_WIDTH * SOURCE_MESSAGE_ATLAS_HEIGHT * 4];
    for glyph in 0..SOURCE_MESSAGE_GLYPHS {
        let atlas_x = (glyph % SOURCE_MESSAGE_ATLAS_COLUMNS) * SOURCE_MESSAGE_GLYPH_WIDTH;
        let atlas_y = (glyph / SOURCE_MESSAGE_ATLAS_COLUMNS) * SOURCE_MESSAGE_GLYPH_HEIGHT;
        let data_start = SOURCE_MESSAGE_WIDTH_BYTES + glyph * SOURCE_MESSAGE_GLYPH_BYTES;
        for row in 0..SOURCE_MESSAGE_GLYPH_HEIGHT {
            let row_start = data_start + row * 2;
            let bits = u16::from_le_bytes([
                SOURCE_MESSAGE_FONT[row_start],
                SOURCE_MESSAGE_FONT[row_start + 1],
            ]);
            for column in 0..SOURCE_MESSAGE_GLYPH_WIDTH {
                if bits & (1 << (15 - column)) == 0 {
                    continue;
                }
                let pixel = ((atlas_y + row) * SOURCE_MESSAGE_ATLAS_WIDTH + atlas_x + column) * 4;
                atlas[pixel..pixel + 4].fill(255);
            }
        }
    }
    gpu.create_texture_rgba(
        SOURCE_MESSAGE_ATLAS_WIDTH as u32,
        SOURCE_MESSAGE_ATLAS_HEIGHT as u32,
        &atlas,
    )
}

fn source_message_glyph(byte: u8) -> u8 {
    match byte {
        b' ' => 39,
        b'!' => 70,
        b'"' => 0,
        b'#' => 67,
        b'%' => 37,
        b'\'' => 68,
        b',' => 131,
        b'-' => 66,
        b'.' => 38,
        b'?' => 36,
        b'0'..=b'9' => byte - b'0',
        b':' => 137,
        b';' => 132,
        b'A'..=b'Z' => 10 + byte - b'A',
        b'^' => 135,
        b'a'..=b'z' => 40 + byte - b'a',
        _ => 0,
    }
}

pub fn source_message_advance(byte: u8) -> u8 {
    SOURCE_MESSAGE_FONT[usize::from(source_message_glyph(byte))]
}
