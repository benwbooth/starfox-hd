//! SNES DMA transfer stand-ins (ROM TRANS.ASM / CONTINUE.ASM).
//!
//! On hardware these copy WRAM → VRAM/OAM via MDMAEN. HD has no PPU DMA;
//! the observable effect is acknowledging a pending transfer (same pattern
//! as [`crate::clip::WaitDma`]).

/// Pending DMA transfer kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaKind {
    /// ROM `dma_sprites_l` — OAM from `spriteblk`.
    Sprites,
    /// ROM `dmabg2voffsets` — BG2 VOFS HDMA bak → VRAM.
    Bg2Voffsets,
    /// ROM `dmahpos` — BG scroll buffer → WRAM HDMA target.
    Hpos,
}

/// Records which DMA leaves were invoked (for tests / shell diagnostics).
#[derive(Debug, Clone, Default)]
pub struct DmaFlush {
    pub sprites: u32,
    pub bg2_voffsets: u32,
    pub hpos: u32,
}

impl DmaFlush {
    pub fn new() -> Self {
        Self::default()
    }

    /// ROM `dma_sprites_l` (CONTINUE.ASM:446).
    pub fn dma_sprites(&mut self) {
        self.sprites = self.sprites.wrapping_add(1);
    }

    /// ROM `dmabg2voffsets_l` (TRANS.ASM:444).
    pub fn dma_bg2_voffsets(&mut self) {
        self.bg2_voffsets = self.bg2_voffsets.wrapping_add(1);
    }

    /// ROM `dmahpos_l` (TRANS.ASM:568).
    pub fn dma_hpos(&mut self) {
        self.hpos = self.hpos.wrapping_add(1);
    }

    pub fn flush(&mut self, kind: DmaKind) {
        match kind {
            DmaKind::Sprites => self.dma_sprites(),
            DmaKind::Bg2Voffsets => self.dma_bg2_voffsets(),
            DmaKind::Hpos => self.dma_hpos(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dma_flush_counts() {
        let mut d = DmaFlush::new();
        d.dma_sprites();
        d.dma_bg2_voffsets();
        d.dma_hpos();
        d.flush(DmaKind::Sprites);
        assert_eq!(d.sprites, 2);
        assert_eq!(d.bg2_voffsets, 1);
        assert_eq!(d.hpos, 1);
    }
}
