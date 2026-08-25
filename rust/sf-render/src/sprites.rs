//! SNES OBJ sprite decoder and renderer.
//!
//! Port (C oracle): `src/renderer/sprites.c`. Decodes 4bpp tile data from
//! OBJ-1.CGX and palettes from OBJ-1.COL, builds an indexed (GL_R8) atlas,
//! and renders sprites as textured quads via the HUD shader's
//! palette-lookup mode. Also owns the indexed radio portrait atlas
//! (FACE.CGX); the active scene supplies its game palette at draw time.

use std::path::Path;

use crate::gpu::{Gpu, TextureId, Vertex2};
use crate::shapes::ShapePaletteRgb;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
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
pub(crate) const FACE_W: usize = 32;
pub(crate) const FACE_H: usize = 40;
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
    let normalized = |component: u16| {
        let expanded = (component << 3) | (component >> 2);
        f32::from(expanded) / 255.0
    };
    [
        normalized(word & 0x1F),
        normalized((word >> 5) & 0x1F),
        normalized((word >> 10) & 0x1F),
        1.0,
    ]
}

pub struct Sprites {
    atlas_tex: Option<TextureId>,
    face_tex: Option<TextureId>,
    num_tiles: usize,
    screen_w: i32,
    screen_h: i32,
    palettes: [[[f32; 4]; COLORS_PER_PAL]; NUM_PALETTES],
    queue: Vec<SpriteCmd>,
}

impl Sprites {
    pub fn new(gpu: &mut Gpu, base_dir: &Path) -> Self {
        let mut s = Sprites {
            atlas_tex: None,
            face_tex: None,
            num_tiles: 0,
            screen_w: 1280,
            screen_h: 720,
            palettes: [[[0.0; 4]; COLORS_PER_PAL]; NUM_PALETTES],
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

                s.atlas_tex = Some(gpu.create_texture_r8(ATLAS_W as u32, ATLAS_H as u32, &atlas));
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
                            s.palettes[pal][c] = decode_bgr555_bytes(&col_data[row_off + c * 2..]);
                        }
                        // Color 0 of every palette is transparent
                        s.palettes[pal][0][3] = 0.0;
                    }
                }
            }
        }

        // ---- Load portrait frames (FACE.CGX) into an indexed atlas ----
        let face_path = base_dir.join("data/sprites/FACE.CGX");
        match std::fs::read(&face_path) {
            Err(_) => eprintln!("Sprites: cannot open {}", face_path.display()),
            Ok(face_rom) => {
                if face_rom.len() < FACE_FRAMES * FACE_FRAME_SIZE {
                    eprintln!("Sprites: FACE.CGX short read ({} bytes)", face_rom.len());
                } else {
                    let mut face_atlas = vec![0u8; FACE_ATLAS_W * FACE_ATLAS_H];
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
                                        let index = tile8x8[px + py * 8];
                                        let ax = fr * FACE_W + col4 * 8 + px;
                                        let ay = trow * 8 + py;
                                        face_atlas[ay * FACE_ATLAS_W + ax] = index;
                                    }
                                }
                            }
                        }
                    }
                    s.face_tex = Some(gpu.create_texture_r8(
                        FACE_ATLAS_W as u32,
                        FACE_ATLAS_H as u32,
                        &face_atlas,
                    ));
                }
            }
        }

        s
    }

    pub fn set_screen_size(&mut self, w: i32, h: i32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    /// ROM `clearsprites_l` (CONTINUE.ASM:433) — wipe the OAM staging buffer
    /// before `dma_sprites_l`. HD clears the sprite draw queue.
    pub fn clear_sprites(&mut self) {
        self.queue.clear();
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
    pub fn render_hud(&mut self, gpu: &mut Gpu, source_y_adjustment: i32) {
        let Some(atlas_tex) = self.atlas_tex else {
            self.queue.clear();
            return;
        };
        if self.queue.is_empty() {
            return;
        }

        let sw = self.screen_w as f32;
        let sh = self.screen_h as f32;
        let proj = ortho(sw, sh);

        let sx = sw / 256.0;
        let sy = sh / 224.0;

        let queue = std::mem::take(&mut self.queue);
        for cmd in &queue {
            let pi = if cmd.palette < 0 || cmd.palette as usize >= NUM_PALETTES {
                0
            } else {
                cmd.palette as usize
            };

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
            let py = sh - (cmd.y + source_y_adjustment + 8) as f32 * sy;
            let pw = 8.0 * sx;
            let ph = 8.0 * sy;

            let verts = [
                Vertex2 {
                    pos: [px, py],
                    uv: [u0, v1],
                },
                Vertex2 {
                    pos: [px + pw, py],
                    uv: [u1, v1],
                },
                Vertex2 {
                    pos: [px + pw, py + ph],
                    uv: [u1, v0],
                },
                Vertex2 {
                    pos: [px, py + ph],
                    uv: [u0, v0],
                },
            ];
            // uUseTexture == 2: palette-indexed R8; color is ignored by mode 2.
            gpu.push_overlay_fan(
                &verts,
                &proj,
                &IDENTITY,
                [1.0, 1.0, 1.0, 1.0],
                2,
                Some(&self.palettes[pi]),
                atlas_tex,
            );
        }

        // Reset for next frame (keep the allocation).
        self.queue = queue;
        self.queue.clear();
    }

    /// Draw a radio portrait frame (0-17) at SNES screen coordinates.
    pub fn draw_face(&self, gpu: &mut Gpu, frame: i32, x: i32, y: i32, palette: &ShapePaletteRgb) {
        let Some(face_tex) = self.face_tex else {
            return;
        };
        if frame < 0 || frame as usize >= FACE_FRAMES {
            return;
        }

        let sw = self.screen_w as f32;
        let sh = self.screen_h as f32;
        let proj = ortho(sw, sh);

        let sx = sw / 256.0;
        let sy = sh / 224.0;
        let px = x as f32 * sx;
        let py = sh - (y + FACE_H as i32) as f32 * sy;
        let pw = FACE_W as f32 * sx;
        let ph = FACE_H as f32 * sy;

        let u0 = (frame as usize * FACE_W) as f32 / FACE_ATLAS_W as f32;
        let u1 = ((frame as usize + 1) * FACE_W) as f32 / FACE_ATLAS_W as f32;

        let verts = [
            Vertex2 {
                pos: [px, py],
                uv: [u0, 1.0],
            },
            Vertex2 {
                pos: [px + pw, py],
                uv: [u1, 1.0],
            },
            Vertex2 {
                pos: [px + pw, py + ph],
                uv: [u1, 0.0],
            },
            Vertex2 {
                pos: [px, py + ph],
                uv: [u0, 0.0],
            },
        ];
        let indexed_palette: [[f32; 4]; COLORS_PER_PAL] = std::array::from_fn(|index| {
            let color = palette[index];
            [color[0], color[1], color[2], 1.0]
        });
        gpu.push_overlay_fan(
            &verts,
            &proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            2,
            Some(&indexed_palette),
            face_tex,
        );
    }
}
