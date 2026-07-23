//! Game clip window + BG scroll clear (ROM MAIN.ASM).
//!
//! On the SNES these write PPU clip/vanish registers and zero BG1–4 H/VOFS.
//! HD keeps the same logical viewport numbers for any consumer that needs
//! them; scroll clear zeroes the port's BG scroll mirrors.

/// ROM `gameNum_col` / `gameNum_row` (VARS.INC:111-112).
pub const GAME_NUM_COL: i16 = 28;
pub const GAME_NUM_ROW: i16 = 24;

/// ROM `gameclipwindow` (MAIN.ASM:1896) — 3D playfield clip + vanish point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameClipWindow {
    pub clx1: i16,
    pub clx2: i16,
    pub cly1: i16,
    pub cly2: i16,
    pub vanishx: i16,
    pub vanishy: i16,
}

impl Default for GameClipWindow {
    fn default() -> Self {
        Self::game()
    }
}

impl GameClipWindow {
    /// ROM `gameclipwindow` defaults.
    pub fn game() -> Self {
        Self {
            clx1: 0,
            clx2: GAME_NUM_COL * 8 - 1, // 223
            cly1: 0,
            cly2: GAME_NUM_ROW * 8 - 1, // 191
            vanishx: GAME_NUM_COL * 4,  // 112
            vanishy: GAME_NUM_ROW * 4,  // 96
        }
    }
}

/// ROM BG1–4 H/VOFS mirrors (cleared by `clearhvofs_l`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BgScrollOffsets {
    pub bg1_hofs: i16,
    pub bg1_vofs: i16,
    pub bg2_hofs: i16,
    pub bg2_vofs: i16,
    pub bg3_hofs: i16,
    pub bg3_vofs: i16,
    pub bg4_hofs: i16,
    pub bg4_vofs: i16,
}

impl BgScrollOffsets {
    /// ROM `clearhvofs_l` (MAIN.ASM:1904) — zero all BG H/VOFS.
    pub fn clear_hvofs(&mut self) {
        *self = Self::default();
    }
}

/// ROM `startsfx_l` (BOOTNMI.ASM:281) — cold-boot PPU register zeroing
/// (INIDISP/OBJSEL/BGMODE/…/HVOFS). HD has no PPU; the observable port
/// effect is clearing BG scroll mirrors (same as [`BgScrollOffsets::clear_hvofs`]).
pub fn start_sfx(scroll: &mut BgScrollOffsets) {
    scroll.clear_hvofs();
}

/// ROM `waitdma_l` / `waitdma224` (PLANETS.ASM:120/136) — spin until the
/// PPU raster hits `line`. HD has no raster; this is a documented no-op
/// that records the requested line for tests / diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WaitDma {
    pub last_line: u16,
}

impl WaitDma {
    pub fn wait(&mut self, line: u16) {
        self.last_line = line;
    }

    /// ROM `waitdma224` — wait for line 222 then a short delay.
    pub fn wait_224(&mut self) {
        self.wait(222);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_clip_window_matches_rom() {
        let w = GameClipWindow::game();
        assert_eq!(w.clx1, 0);
        assert_eq!(w.clx2, 223);
        assert_eq!(w.cly1, 0);
        assert_eq!(w.cly2, 191);
        assert_eq!(w.vanishx, 112);
        assert_eq!(w.vanishy, 96);
    }

    #[test]
    fn clear_hvofs_zeros_all() {
        let mut s = BgScrollOffsets {
            bg1_hofs: 10,
            bg2_vofs: 20,
            bg4_hofs: 30,
            ..Default::default()
        };
        s.clear_hvofs();
        assert_eq!(s, BgScrollOffsets::default());
    }

    #[test]
    fn start_sfx_clears_scroll() {
        let mut s = BgScrollOffsets {
            bg3_hofs: 7,
            ..Default::default()
        };
        start_sfx(&mut s);
        assert_eq!(s, BgScrollOffsets::default());
    }

    #[test]
    fn wait_dma_records_line() {
        let mut w = WaitDma::default();
        w.wait(100);
        assert_eq!(w.last_line, 100);
        w.wait_224();
        assert_eq!(w.last_line, 222);
    }
}
