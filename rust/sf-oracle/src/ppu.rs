//! Small, deterministic SNES PPU state model used by the retail-host bridge.
//!
//! This is intentionally scoped to the CPU-visible PPU contract Star Fox 2
//! uses: VRAM/CGRAM/OAM ports, the BG and sprite registers, and a software
//! compositor for the ordinary tiled modes plus Mode 7.  Timing remains in
//! `RetailBootBus`; this module owns only state that persists across writes.

const VRAM_SIZE: usize = 0x1_0000;
const CGRAM_SIZE: usize = 0x0200;
const OAM_SIZE: usize = 544;
pub const FRAME_WIDTH: usize = 256;
pub const FRAME_HEIGHT: usize = 224;

/// A complete CPU-visible PPU snapshot and its composited native-resolution
/// frame.  The raw memories remain available for independent oracle tests.
#[derive(Debug, Clone)]
pub struct PpuFrame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub registers: [u8; 0x40],
    pub vram: Vec<u8>,
    pub cgram: Vec<u8>,
    pub oam: Vec<u8>,
    pub bg_hofs: [u16; 4],
    pub bg_vofs: [u16; 4],
}

pub(crate) struct Ppu {
    registers: [u8; 0x40],
    vram: Vec<u8>,
    cgram: Vec<u8>,
    oam: Vec<u8>,
    vram_address: u16,
    cgram_address: u8,
    cgram_high: bool,
    oam_address: u16,
    oam_latch: u8,
    /// Shared BG horizontal/vertical write latch (`_hvScrollLatchValue` in
    /// Mesen) plus the separate low-three-bit horizontal latch.
    scroll_latch: u8,
    h_scroll_latch: u8,
    bg_hofs: [u16; 4],
    bg_vofs: [u16; 4],
    mode7_latch: u8,
    mode7: [u16; 6],
    scanout_rgba: Vec<u8>,
    completed_rgba: Vec<u8>,
    /// Last non-forced-blank display value.  The CPU commonly sets forced
    /// blank during vblank after the visible frame has already been scanned;
    /// a snapshot compositor must retain that completed frame's brightness.
    last_visible_inidisp: Option<u8>,
}

impl Ppu {
    pub(crate) fn new() -> Self {
        let mut blank = vec![0; FRAME_WIDTH * FRAME_HEIGHT * 4];
        for pixel in blank.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        Self {
            registers: [0; 0x40],
            vram: vec![0; VRAM_SIZE],
            cgram: vec![0; CGRAM_SIZE],
            oam: vec![0; OAM_SIZE],
            vram_address: 0,
            cgram_address: 0,
            cgram_high: false,
            oam_address: 0,
            oam_latch: 0,
            scroll_latch: 0,
            h_scroll_latch: 0,
            bg_hofs: [0; 4],
            bg_vofs: [0; 4],
            mode7_latch: 0,
            mode7: [0; 6],
            scanout_rgba: blank.clone(),
            completed_rgba: blank,
            last_visible_inidisp: None,
        }
    }

    /// Finish the prior raster frame and clear the new scanout buffer.
    pub(crate) fn begin_frame(&mut self) {
        self.completed_rgba.clone_from(&self.scanout_rgba);
        for pixel in self.scanout_rgba.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0, 0, 0, 255]);
        }
    }

    #[inline]
    fn register_index(address: u16) -> Option<usize> {
        (0x2100..=0x213F)
            .contains(&address)
            .then_some(usize::from(address - 0x2100))
    }

    #[inline]
    fn vram_increment(&self) -> u16 {
        match self.registers[0x15] & 3 {
            0 => 1,
            1 => 32,
            _ => 128,
        }
    }

    #[inline]
    fn advance_vram(&mut self, high_port: bool) {
        let increment_on_high = self.registers[0x15] & 0x80 != 0;
        if increment_on_high == high_port {
            self.vram_address = self.vram_address.wrapping_add(self.vram_increment());
        }
    }

    #[inline]
    fn vram_byte_address(&self, high: bool) -> usize {
        (usize::from(self.vram_address) * 2 + usize::from(high)) & 0xFFFF
    }

    fn write_scroll(&mut self, bg: usize, vertical: bool, value: u8) {
        if vertical {
            let combined = ((u16::from(value) << 8) | u16::from(self.scroll_latch)) & 0x03FF;
            self.bg_vofs[bg] = combined;
            self.scroll_latch = value;
        } else {
            let combined = ((u16::from(value) << 8)
                | u16::from(self.scroll_latch & !7)
                | u16::from(self.h_scroll_latch & 7))
                & 0x03FF;
            self.bg_hofs[bg] = combined;
            self.scroll_latch = value;
            self.h_scroll_latch = value;
        }
    }

    fn write_mode7(&mut self, index: usize, value: u8) {
        let combined = u16::from(self.mode7_latch) | (u16::from(value) << 8);
        self.mode7_latch = value;
        self.mode7[index] = combined;
    }

    #[inline]
    fn oam_storage_address(address: u16) -> usize {
        if address & 0x0200 == 0 {
            usize::from(address & 0x01FF)
        } else {
            // The 32-byte high table is mirrored throughout $220-$3ff.
            0x0200 + usize::from(address & 0x001F)
        }
    }

    #[inline]
    fn advance_oam(&mut self) {
        self.oam_address = self.oam_address.wrapping_add(1) & 0x03FF;
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) {
        let Some(index) = Self::register_index(address) else {
            return;
        };
        self.registers[index] = value;
        match address {
            0x2100 if value & 0x80 == 0 => self.last_visible_inidisp = Some(value),
            0x2102 => self.oam_address = (self.oam_address & 0x0200) | (u16::from(value) << 1),
            0x2103 => {
                self.oam_address = (self.oam_address & 0x01FE) | (u16::from(value & 1) << 9);
            }
            0x2104 => {
                let address = self.oam_address;
                let odd = address & 1 != 0;
                self.advance_oam();
                if address & 0x0200 != 0 {
                    self.oam[Self::oam_storage_address(address)] = value;
                } else if odd {
                    // Low-table OAM writes are paired: the even byte is held
                    // in a latch and both bytes commit on the odd write.
                    self.oam[Self::oam_storage_address(address & !1)] = self.oam_latch;
                    self.oam[Self::oam_storage_address(address)] = value;
                } else {
                    self.oam_latch = value;
                }
            }
            0x210D..=0x2114 => {
                let slot = usize::from(address - 0x210D);
                self.write_scroll(slot / 2, slot & 1 != 0, value);
            }
            0x2116 => self.vram_address = (self.vram_address & 0xFF00) | u16::from(value),
            0x2117 => self.vram_address = (self.vram_address & 0x00FF) | (u16::from(value) << 8),
            0x2118 | 0x2119 => {
                let high = address == 0x2119;
                let target = self.vram_byte_address(high);
                self.vram[target] = value;
                self.advance_vram(high);
            }
            0x211B..=0x2120 => self.write_mode7(usize::from(address - 0x211B), value),
            0x2121 => {
                self.cgram_address = value;
                self.cgram_high = false;
            }
            0x2122 => {
                let address = usize::from(self.cgram_address) * 2 + usize::from(self.cgram_high);
                self.cgram[address & 0x1FF] = if self.cgram_high { value & 0x7F } else { value };
                self.cgram_high = !self.cgram_high;
                if !self.cgram_high {
                    self.cgram_address = self.cgram_address.wrapping_add(1);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn read(&mut self, address: u16) -> Option<u8> {
        let value = match address {
            0x2138 => {
                let value = self.oam[Self::oam_storage_address(self.oam_address)];
                self.advance_oam();
                value
            }
            0x2139 | 0x213A => {
                let high = address == 0x213A;
                let value = self.vram[self.vram_byte_address(high)];
                self.advance_vram(high);
                value
            }
            0x213B => {
                let address = usize::from(self.cgram_address) * 2 + usize::from(self.cgram_high);
                let value = self.cgram[address & 0x1FF];
                self.cgram_high = !self.cgram_high;
                if !self.cgram_high {
                    self.cgram_address = self.cgram_address.wrapping_add(1);
                }
                value
            }
            _ => return None,
        };
        Some(value)
    }

    pub(crate) fn frame(&self) -> PpuFrame {
        PpuFrame {
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            rgba: self.completed_rgba.clone(),
            registers: self.registers,
            vram: self.vram.clone(),
            cgram: self.cgram.clone(),
            oam: self.oam.clone(),
            bg_hofs: self.bg_hofs,
            bg_vofs: self.bg_vofs,
        }
    }

    pub(crate) fn snapshot_rgba(&self) -> Vec<u8> {
        self.render_rgba()
    }

    pub(crate) fn snapshot_bg_rgba(&self, bg: usize) -> Vec<u8> {
        let mut rgba = vec![0; FRAME_WIDTH * FRAME_HEIGHT * 4];
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                let output = self
                    .bg_pixel(bg, x, y)
                    .map_or([0, 0, 0, 255], |(color, _)| self.color(usize::from(color)));
                let offset = (y * FRAME_WIDTH + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&output);
            }
        }
        rgba
    }

    pub(crate) fn snapshot_bg_indices(&self, bg: usize) -> Vec<u8> {
        let mut indices = vec![u8::MAX; FRAME_WIDTH * FRAME_HEIGHT];
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                if let Some((color, _)) = self.bg_pixel(bg, x, y) {
                    indices[y * FRAME_WIDTH + x] = color;
                }
            }
        }
        indices
    }

    fn color(&self, index: usize) -> [u8; 4] {
        let offset = (index & 0xFF) * 2;
        let raw = u16::from_le_bytes([self.cgram[offset], self.cgram[offset + 1]]);
        let expand = |component: u16| -> u8 {
            let five = component & 31;
            ((five << 3) | (five >> 2)) as u8
        };
        let brightness =
            u16::from(self.last_visible_inidisp.unwrap_or(self.registers[0]) & 0x0F) + 1;
        let scale = |component: u8| ((u16::from(component) * brightness) / 16) as u8;
        [
            scale(expand(raw)),
            scale(expand(raw >> 5)),
            scale(expand(raw >> 10)),
            255,
        ]
    }

    fn bg_bpp(mode: u8, bg: usize) -> Option<usize> {
        match mode {
            0 => Some(2),
            1 => match bg {
                0 | 1 => Some(4),
                2 => Some(2),
                _ => None,
            },
            2 => (bg < 2).then_some(4),
            3 => match bg {
                0 => Some(8),
                1 => Some(4),
                _ => None,
            },
            4 => match bg {
                0 => Some(8),
                1 => Some(2),
                _ => None,
            },
            5 => match bg {
                0 => Some(4),
                1 => Some(2),
                _ => None,
            },
            6 => (bg == 0).then_some(4),
            _ => None,
        }
    }

    fn bg_character_base(&self, bg: usize) -> usize {
        let packed = self.registers[0x0B + bg / 2];
        let nibble = if bg & 1 == 0 {
            packed & 0x0F
        } else {
            packed >> 4
        };
        // PPU tile-data addresses are 16-bit VRAM word addresses; this model's
        // backing memory is byte-addressed.
        usize::from(nibble) * 0x2000
    }

    fn bg_map_word(&self, bg: usize, tile_x: usize, tile_y: usize) -> u16 {
        let sc = self.registers[0x07 + bg];
        // BGxSC likewise names a word address (`bits 2..7 << 10`).
        let base = usize::from(sc & 0xFC) << 9;
        let width = if sc & 1 != 0 { 64 } else { 32 };
        let height = if sc & 2 != 0 { 64 } else { 32 };
        let x = tile_x % width;
        let y = tile_y % height;
        let screen_x = x / 32;
        let screen_y = y / 32;
        let screens_per_row = if width == 64 { 2 } else { 1 };
        let screen = screen_y * screens_per_row + screen_x;
        let entry = screen * 0x800 + ((y & 31) * 32 + (x & 31)) * 2;
        let address = (base + entry) & 0xFFFF;
        u16::from_le_bytes([self.vram[address], self.vram[(address + 1) & 0xFFFF]])
    }

    fn vram_word(&self, address: usize) -> u16 {
        let byte = (address & 0x7FFF) * 2;
        u16::from_le_bytes([self.vram[byte], self.vram[(byte + 1) & 0xFFFF]])
    }

    /// Modes 2/4/6 use BG3 as an offset-per-tile table for BG1/BG2. The PPU
    /// fetches a column's tilemap words before that column's offset words, so
    /// tile column N uses the offset fetched for column N-1.
    fn offset_per_tile_scroll(&self, bg: usize, x: usize) -> (usize, usize) {
        let mode = self.registers[5] & 7;
        if !matches!(mode, 2 | 4 | 6) || bg > 1 {
            return (
                usize::from(self.bg_hofs[bg] & 0x03FF),
                usize::from(self.bg_vofs[bg] & 0x03FF),
            );
        }

        let base_h = self.bg_hofs[bg] & 0x03FF;
        let mut hscroll = base_h;
        let mut vscroll = self.bg_vofs[bg] & 0x03FF;
        let lookup = (x + usize::from(base_h & 7)) >> 3;
        if lookup == 0 {
            return (usize::from(hscroll), usize::from(vscroll));
        }

        let offset_column = lookup - 1;
        let config = self.registers[0x09]; // BG3SC
        let base = usize::from(config & 0x7C) << 8; // word address
        let width_mask = if config & 1 != 0 { 0x3F } else { 0x1F };
        let height_mask = if config & 2 != 0 { 0x3F } else { 0x1F };
        let column = (offset_column + usize::from((self.bg_hofs[2] & !7) >> 3)) & width_mask;
        let row = usize::from((self.bg_vofs[2] >> 3) & height_mask as u16);
        let tile_offset = column + (row << 5);
        let horizontal = self.vram_word(base + tile_offset);
        let enable = if bg == 0 { 0x2000 } else { 0x4000 };

        if mode == 4 {
            if horizontal & enable != 0 {
                if horizontal & 0x8000 == 0 {
                    hscroll = (hscroll & 7) | (horizontal & 0x03F8);
                } else {
                    vscroll = horizontal & 0x03FF;
                }
            }
        } else {
            let vertical_mask = if config & 2 != 0 { 0x07FF } else { 0x03FF };
            let vertical = self.vram_word(base + ((tile_offset + 0x20) & vertical_mask));
            if horizontal & enable != 0 {
                hscroll = (hscroll & 7) | (horizontal & 0x03F8);
            }
            if vertical & enable != 0 {
                vscroll = vertical & 0x03FF;
            }
        }
        (usize::from(hscroll), usize::from(vscroll))
    }

    fn tile_pixel(&self, base: usize, tile: usize, bpp: usize, x: usize, y: usize) -> u8 {
        let bytes_per_tile = bpp * 8;
        let start = (base + tile * bytes_per_tile) & 0xFFFF;
        let bit = 7 - (x & 7);
        let mut value = 0u8;
        for plane in 0..bpp {
            let pair = plane / 2;
            let plane_byte = plane & 1;
            let address = (start + pair * 16 + (y & 7) * 2 + plane_byte) & 0xFFFF;
            value |= ((self.vram[address] >> bit) & 1) << plane;
        }
        value
    }

    fn bg_pixel(&self, bg: usize, x: usize, y: usize) -> Option<(u8, bool)> {
        let mode = self.registers[5] & 7;
        let bpp = Self::bg_bpp(mode, bg)?;
        if self.registers[0x2C] & (1 << bg) == 0 {
            return None;
        }
        let large = self.registers[5] & (0x10 << bg) != 0;
        let tile_size = if large { 16 } else { 8 };
        let (hscroll, vscroll) = self.offset_per_tile_scroll(bg, x);
        let sx = (x + hscroll) & 0x3FF;
        let sy = (y + vscroll) & 0x3FF;
        let map = self.bg_map_word(bg, sx / tile_size, sy / tile_size);
        let mut px = sx % tile_size;
        let mut py = sy % tile_size;
        if map & 0x4000 != 0 {
            px = tile_size - 1 - px;
        }
        if map & 0x8000 != 0 {
            py = tile_size - 1 - py;
        }
        let mut tile = usize::from(map & 0x03FF);
        if large {
            tile = tile.wrapping_add((px / 8) + (py / 8) * 16);
        }
        let value = self.tile_pixel(self.bg_character_base(bg), tile, bpp, px, py);
        if value == 0 {
            return None;
        }
        let palette = usize::from((map >> 10) & 7);
        let color = match bpp {
            2 if mode == 0 => bg * 32 + palette * 4 + usize::from(value),
            2 => palette * 4 + usize::from(value),
            4 => palette * 16 + usize::from(value),
            _ => usize::from(value),
        } as u8;
        Some((color, map & 0x2000 != 0))
    }

    fn mode7_pixel(&self, x: usize, y: usize) -> Option<u8> {
        if self.registers[0x2C] & 1 == 0 {
            return None;
        }
        let signed = |value: u16| value as i16 as i32;
        let a = signed(self.mode7[0]);
        let b = signed(self.mode7[1]);
        let c = signed(self.mode7[2]);
        let d = signed(self.mode7[3]);
        let cx = signed(self.mode7[4]);
        let cy = signed(self.mode7[5]);
        let mut sx = x as i32;
        let mut sy = y as i32;
        if self.registers[0x1A] & 1 != 0 {
            sx = 255 - sx;
        }
        if self.registers[0x1A] & 2 != 0 {
            sy = 255 - sy;
        }
        let dx = sx + i32::from(self.bg_hofs[0] as i16) - cx;
        let dy = sy + i32::from(self.bg_vofs[0] as i16) - cy;
        let world_x = ((a * dx + b * dy) >> 8) + cx;
        let world_y = ((c * dx + d * dy) >> 8) + cy;
        let wx = world_x.rem_euclid(1024) as usize;
        let wy = world_y.rem_euclid(1024) as usize;
        let map_address = (((wy >> 3) * 128 + (wx >> 3)) * 2) & 0xFFFF;
        let tile = usize::from(self.vram[map_address]);
        let pixel_address = (tile * 128 + (wy & 7) * 16 + (wx & 7) * 2 + 1) & 0xFFFF;
        let color = self.vram[pixel_address];
        (color != 0).then_some(color)
    }

    fn sprite_sizes(value: u8) -> ((usize, usize), (usize, usize)) {
        match value >> 5 {
            0 => ((8, 8), (16, 16)),
            1 => ((8, 8), (32, 32)),
            2 => ((8, 8), (64, 64)),
            3 => ((16, 16), (32, 32)),
            4 => ((16, 16), (64, 64)),
            5 => ((32, 32), (64, 64)),
            6 => ((16, 32), (32, 64)),
            _ => ((16, 32), (32, 32)),
        }
    }

    fn sprite_pixel(&self, x: usize, y: usize) -> Option<(u8, u8)> {
        if self.registers[0x2C] & 0x10 == 0 {
            return None;
        }
        let obsel = self.registers[1];
        let (small, large) = Self::sprite_sizes(obsel);
        let base = usize::from(obsel & 7) * 0x4000;
        let name_select = (1 + usize::from((obsel >> 3) & 3)) * 0x2000;
        let mut best: Option<(u8, u8)> = None;
        // Later OAM entries are behind earlier entries on equal priority.
        for sprite in (0..128).rev() {
            let o = sprite * 4;
            let hi = self.oam[512 + sprite / 4];
            let shift = (sprite & 3) * 2;
            let x_high = (hi >> shift) & 1;
            let is_large = (hi >> (shift + 1)) & 1 != 0;
            let (w, h) = if is_large { large } else { small };
            let sx = i32::from(self.oam[o]) | (i32::from(x_high) << 8);
            let sx = if sx >= 256 { sx - 512 } else { sx };
            let sy = i32::from(self.oam[o + 1]);
            let mut px = x as i32 - sx;
            let mut py = y as i32 - sy;
            if py < 0 {
                py += 256;
            }
            if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                continue;
            }
            let attr = self.oam[o + 3];
            if attr & 0x40 != 0 {
                px = w as i32 - 1 - px;
            }
            if attr & 0x80 != 0 {
                py = h as i32 - 1 - py;
            }
            let tile_x = px as usize / 8;
            let tile_y = py as usize / 8;
            let mut tile = usize::from(self.oam[o + 2]) + tile_x + tile_y * 16;
            let mut character_base = base;
            if attr & 1 != 0 {
                tile &= 0xFF;
                character_base = character_base.wrapping_add(name_select);
            }
            let value = self.tile_pixel(character_base, tile, 4, px as usize, py as usize);
            if value == 0 {
                continue;
            }
            let palette = usize::from((attr >> 1) & 7);
            let color = (128 + palette * 16 + usize::from(value)) as u8;
            let priority = (attr >> 4) & 3;
            if best.is_none_or(|(_, old_priority)| priority >= old_priority) {
                best = Some((color, priority));
            }
        }
        best
    }

    /// Evaluate OAM once for an entire scanline. The old per-pixel path walked
    /// all 128 sprites 57,344 times per frame; this produces identical ordering
    /// (lower OAM index wins equal priority) with only the covered pixels.
    fn sprite_scanline(&self, y: usize) -> [Option<(u8, u8)>; FRAME_WIDTH] {
        let mut output = [None; FRAME_WIDTH];
        if self.registers[0x2C] & 0x10 == 0 {
            return output;
        }
        let obsel = self.registers[1];
        let (small, large) = Self::sprite_sizes(obsel);
        let base = usize::from(obsel & 7) * 0x4000;
        let name_select = (1 + usize::from((obsel >> 3) & 3)) * 0x2000;
        for sprite in (0..128).rev() {
            let o = sprite * 4;
            let hi = self.oam[512 + sprite / 4];
            let shift = (sprite & 3) * 2;
            let x_high = (hi >> shift) & 1;
            let is_large = (hi >> (shift + 1)) & 1 != 0;
            let (w, h) = if is_large { large } else { small };
            let sx = i32::from(self.oam[o]) | (i32::from(x_high) << 8);
            let sx = if sx >= 256 { sx - 512 } else { sx };
            let sy = i32::from(self.oam[o + 1]);
            let mut py = y as i32 - sy;
            if py < 0 {
                py += 256;
            }
            if py < 0 || py >= h as i32 {
                continue;
            }
            let attr = self.oam[o + 3];
            if attr & 0x80 != 0 {
                py = h as i32 - 1 - py;
            }
            for screen_x in sx.max(0)..(sx + w as i32).min(FRAME_WIDTH as i32) {
                let mut px = screen_x - sx;
                if attr & 0x40 != 0 {
                    px = w as i32 - 1 - px;
                }
                let tile_x = px as usize / 8;
                let tile_y = py as usize / 8;
                let mut tile = usize::from(self.oam[o + 2]) + tile_x + tile_y * 16;
                let mut character_base = base;
                if attr & 1 != 0 {
                    tile &= 0xFF;
                    character_base = character_base.wrapping_add(name_select);
                }
                let value = self.tile_pixel(character_base, tile, 4, px as usize, py as usize);
                if value == 0 {
                    continue;
                }
                let palette = usize::from((attr >> 1) & 7);
                let color = (128 + palette * 16 + usize::from(value)) as u8;
                let priority = (attr >> 4) & 3;
                let slot = &mut output[screen_x as usize];
                if slot.is_none_or(|(_, old_priority)| priority >= old_priority) {
                    *slot = Some((color, priority));
                }
            }
        }
        output
    }

    fn background_color_index(&self, mode: u8, x: usize, y: usize) -> u8 {
        let mut color = 0u8;
        if mode == 7 {
            if let Some(value) = self.mode7_pixel(x, y) {
                color = value;
            }
        } else {
            for high in [false, true] {
                for bg in (0..4).rev() {
                    if let Some((value, priority)) = self.bg_pixel(bg, x, y) {
                        if priority == high {
                            color = value;
                        }
                    }
                }
            }
        }
        color
    }

    fn pixel_color_index(&self, mode: u8, x: usize, y: usize) -> u8 {
        let mut color = self.background_color_index(mode, x, y);
        if let Some((sprite, _priority)) = self.sprite_pixel(x, y) {
            color = sprite;
        }
        color
    }

    /// Render one visible scanline using the registers in effect for that
    /// raster line. HDMA changes therefore affect only subsequent lines.
    pub(crate) fn render_scanline(&mut self, y: usize) {
        if y >= FRAME_HEIGHT {
            return;
        }
        let forced_blank = self.registers[0] & 0x80 != 0;
        let mode = self.registers[5] & 7;
        let sprites = self.sprite_scanline(y);
        for x in 0..FRAME_WIDTH {
            let output = if forced_blank {
                [0, 0, 0, 255]
            } else {
                let mut color = self.background_color_index(mode, x, y);
                if let Some((sprite, _priority)) = sprites[x] {
                    color = sprite;
                }
                self.color(usize::from(color))
            };
            let offset = (y * FRAME_WIDTH + x) * 4;
            self.scanout_rgba[offset..offset + 4].copy_from_slice(&output);
        }
    }

    #[allow(dead_code)]
    fn render_rgba(&self) -> Vec<u8> {
        let mut rgba = vec![0; FRAME_WIDTH * FRAME_HEIGHT * 4];
        if self.registers[0] & 0x80 != 0 && self.last_visible_inidisp.is_none() {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            return rgba;
        }
        let mode = self.registers[5] & 7;
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                let color = self.pixel_color_index(mode, x, y);
                let output = self.color(usize::from(color));
                let offset = (y * FRAME_WIDTH + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&output);
            }
        }
        rgba
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vram_ports_follow_increment_mode() {
        let mut ppu = Ppu::new();
        ppu.write(0x2115, 0x80);
        ppu.write(0x2116, 0x34);
        ppu.write(0x2117, 0x12);
        ppu.write(0x2118, 0xAA);
        ppu.write(0x2119, 0x55);
        assert_eq!(ppu.vram[0x2468..0x246A], [0xAA, 0x55]);
        assert_eq!(ppu.vram_address, 0x1235);
    }

    #[test]
    fn cgram_port_masks_high_bit_and_advances_after_pair() {
        let mut ppu = Ppu::new();
        ppu.write(0x2121, 7);
        ppu.write(0x2122, 0xEF);
        ppu.write(0x2122, 0xFF);
        assert_eq!(ppu.cgram[14..16], [0xEF, 0x7F]);
        assert_eq!(ppu.cgram_address, 8);
    }

    #[test]
    fn oam_low_table_commits_pairs_and_high_table_mirrors() {
        let mut ppu = Ppu::new();
        ppu.write(0x2102, 0);
        ppu.write(0x2103, 0);
        ppu.write(0x2104, 0x12);
        assert_eq!(ppu.oam[0], 0, "even low-table byte remains latched");
        ppu.write(0x2104, 0x34);
        assert_eq!(ppu.oam[..2], [0x12, 0x34]);

        ppu.write(0x2102, 0x10);
        ppu.write(0x2103, 1); // byte address $220, mirroring high-table byte 0
        ppu.write(0x2104, 0xA5);
        assert_eq!(ppu.oam[0x200], 0xA5);
        assert_eq!(ppu.oam_address, 0x221);
    }

    #[test]
    fn bg_scroll_ports_use_shared_hv_and_horizontal_latches() {
        let mut ppu = Ppu::new();
        // BG1HOFS low/high writes for 39, followed by BG1VOFS 0. These are the
        // final retail SF2 values in the independent Mesen frame-1000 oracle.
        ppu.write(0x210D, 39);
        ppu.write(0x210D, 0);
        ppu.write(0x210E, 0);
        ppu.write(0x210E, 0);
        assert_eq!(ppu.bg_hofs[0], 39);
        assert_eq!(ppu.bg_vofs[0], 0);

        ppu.write(0x2112, 224);
        ppu.write(0x2112, 0);
        assert_eq!(ppu.bg_vofs[2], 224);
    }
}
