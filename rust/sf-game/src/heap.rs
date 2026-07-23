//! Strategy heap + virtual stack (ROM MEM.ASM).
//!
//! SNES used a free-list in bank `$7E` with 2-byte length headers. HD keeps
//! the same *resource-tracking* model (`al_memptr` linked list of blocks,
//! mpush/mpull stacks) without recreating segmented 16-bit addresses —
//! blocks are opaque handles into a growable arena.

use std::collections::HashMap;

/// Minimum free-block size on ROM (`fm_sizeof` = 6).
const FM_SIZEOF: usize = 6;
/// Stack node payload size on ROM (`sp_sizeof` = 6): prev(2) + data(4).
pub const SP_SIZEOF: usize = 6;

/// Opaque block handle (never zero — ROM returns 0 on failure).
pub type BlockId = u32;

/// One allocated block.
#[derive(Debug, Clone)]
struct Block {
    /// Usable payload length (excludes ROM length header).
    len: usize,
    /// Next block in an alien's `memptr` chain (ROM `heap,y` link).
    next_owned: Option<BlockId>,
    /// Stack link: previous stack node (ROM `sp_prev`).
    stack_prev: Option<BlockId>,
    /// Stack payload (ROM `sp_data` 32-bit).
    stack_data: u32,
    /// True while allocated (not on free list).
    live: bool,
}

/// Strategy heap: first-fit allocator + alien-owned lists + virtual stacks.
#[derive(Debug, Default)]
pub struct StratHeap {
    blocks: HashMap<BlockId, Block>,
    next_id: BlockId,
    /// Sum of free payload bytes (approximate, like ROM `avail_l`).
    free_bytes: usize,
    /// Total arena capacity (initial free bytes).
    #[allow(dead_code)]
    capacity: usize,
}

impl StratHeap {
    /// ROM `initialise_memory_l` — start with `capacity` free bytes.
    pub fn new(capacity: usize) -> Self {
        Self {
            blocks: HashMap::new(),
            next_id: 1, // never mint 0
            free_bytes: capacity,
            capacity,
        }
    }

    /// ROM `avail_l` — approximate free memory.
    pub fn avail(&self) -> usize {
        self.free_bytes
    }

    fn mint(&mut self) -> BlockId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    /// Round request like ROM: word-align, +2 header, min `fm_sizeof`.
    fn request_size(len: usize) -> usize {
        let mut n = (len + 1) & !1; // round up to word
        if n == 0 {
            return 0;
        }
        n += 2; // length header
        n.max(FM_SIZEOF)
    }

    /// ROM `alloc_l` — allocate `len` usable bytes; returns handle or None.
    pub fn alloc(&mut self, len: usize) -> Option<BlockId> {
        let need = Self::request_size(len);
        if need == 0 || need > self.free_bytes {
            return None;
        }
        let usable = need - 2;
        self.free_bytes -= need;
        let id = self.mint();
        self.blocks.insert(
            id,
            Block {
                len: usable,
                next_owned: None,
                stack_prev: None,
                stack_data: 0,
                live: true,
            },
        );
        Some(id)
    }

    /// ROM `free_l` — free a block by handle (no-op on 0 / unknown).
    pub fn free(&mut self, id: BlockId) {
        if id == 0 {
            return;
        }
        let Some(b) = self.blocks.remove(&id) else {
            return;
        };
        if b.live {
            self.free_bytes += b.len + 2;
        }
    }

    /// ROM `salloc_l` — alloc for alien; prepend to `memptr` chain.
    /// Returns the usable handle (ROM returns addr+2 after linking).
    pub fn salloc(&mut self, memptr: &mut u16, len: usize) -> Option<BlockId> {
        // ROM: clc; adc #2; jsl alloc_l — extra 2 bytes for next-link word.
        let id = self.alloc(len.saturating_add(2))?;
        let prev = if *memptr == 0 {
            None
        } else {
            Some(*memptr as BlockId)
        };
        if let Some(b) = self.blocks.get_mut(&id) {
            b.next_owned = prev;
        }
        *memptr = id as u16;
        Some(id)
    }

    /// ROM `sfree_l` — unlink one block from alien chain and free it.
    pub fn sfree(&mut self, memptr: &mut u16, id: BlockId) {
        if id == 0 {
            return;
        }
        let mut cur = if *memptr == 0 {
            None
        } else {
            Some(*memptr as BlockId)
        };
        let mut prev: Option<BlockId> = None;
        while let Some(c) = cur {
            if c == id {
                let next = self.blocks.get(&c).and_then(|b| b.next_owned);
                match prev {
                    None => *memptr = next.unwrap_or(0) as u16,
                    Some(p) => {
                        if let Some(pb) = self.blocks.get_mut(&p) {
                            pb.next_owned = next;
                        }
                    }
                }
                self.free(c);
                return;
            }
            prev = Some(c);
            cur = self.blocks.get(&c).and_then(|b| b.next_owned);
        }
    }

    /// ROM `sallfree_l` — free entire alien-owned chain.
    pub fn sallfree(&mut self, memptr: &mut u16) {
        let mut cur = if *memptr == 0 {
            None
        } else {
            Some(*memptr as BlockId)
        };
        while let Some(c) = cur {
            let next = self.blocks.get(&c).and_then(|b| b.next_owned);
            self.free(c);
            cur = next;
        }
        *memptr = 0;
    }

    /// ROM `mpush_l` — push `data` onto stack headed by `sp`; returns new head.
    pub fn mpush(&mut self, sp: Option<BlockId>, data: u32) -> Option<BlockId> {
        let id = self.alloc(SP_SIZEOF)?;
        if let Some(b) = self.blocks.get_mut(&id) {
            b.stack_prev = sp;
            b.stack_data = data;
        }
        Some(id)
    }

    /// ROM `mpull_l` — pop stack; returns `(new_sp, data)`.
    pub fn mpull(&mut self, sp: BlockId) -> Option<(Option<BlockId>, u32)> {
        if sp == 0 {
            return None;
        }
        let b = self.blocks.get(&sp)?;
        if !b.live {
            return None;
        }
        let prev = b.stack_prev;
        let data = b.stack_data;
        self.free(sp);
        Some((prev, data))
    }

    /// ROM `smpush_l` — mpush tracked via `salloc` on the alien.
    pub fn smpush(&mut self, memptr: &mut u16, sp: Option<BlockId>, data: u32) -> Option<BlockId> {
        let id = self.salloc(memptr, SP_SIZEOF)?;
        if let Some(b) = self.blocks.get_mut(&id) {
            b.stack_prev = sp;
            b.stack_data = data;
        }
        Some(id)
    }

    /// ROM `smpull_l` — mpull + `sfree` the node from the alien chain.
    pub fn smpull(&mut self, memptr: &mut u16, sp: BlockId) -> Option<(Option<BlockId>, u32)> {
        let (prev, data) = {
            let b = self.blocks.get(&sp)?;
            (b.stack_prev, b.stack_data)
        };
        self.sfree(memptr, sp);
        Some((prev, data))
    }
}

/// ROM `fadetable` (PLANETS.ASM:3568) — coldata intensity ramp for line fades.
pub const FADE_TABLE: [u8; 17] = [
    0xE0 + 31,
    0xE0 + 31,
    0xE0 + 30,
    0xE0 + 29,
    0xE0 + 28,
    0xE0 + 26,
    0xE0 + 24,
    0xE0 + 22,
    0xE0 + 20,
    0xE0 + 18,
    0xE0 + 16,
    0xE0 + 14,
    0xE0 + 12,
    0xE0 + 10,
    0xE0 + 7,
    0xE0 + 4,
    0xE0 + 1,
];

/// Pack BGR555 like ROM `rgbw` macro: `r | (g<<5) | (b<<10)`.
fn rgbw(r: u16, g: u16, b: u16) -> u16 {
    (r & 0x1F) | ((g & 0x1F) << 5) | ((b & 0x1F) << 10)
}

/// ROM `rgbws` — scale RGB by percent then pack.
fn rgbws(r: u16, g: u16, b: u16, scale: u16) -> u16 {
    let sr = r.saturating_mul(scale) / 100;
    let sg = g.saturating_mul(scale) / 100;
    let sb = b.saturating_mul(scale) / 100;
    rgbw(sr, sg, sb)
}

/// ROM `fadetab0` (PLANETS.ASM:3539) — 16 BGR555 words fading colour 0
/// from `$06,$08,$0a` down (15 steps of −100/16%) then black.
pub fn fade_tab0() -> [u16; 16] {
    let mut out = [0u16; 16];
    let mut scale: i32 = 100;
    for i in 0..15 {
        scale -= 100 / 16;
        out[i] = rgbws(0x06, 0x08, 0x0A, scale.max(0) as u16);
    }
    out[15] = rgbws(0, 0, 0, 100);
    out
}

/// ROM `fadecol0` — look up `fadetab0[index]` (index is even byte offset/2).
pub fn fade_col0(index: usize) -> u16 {
    let tab = fade_tab0();
    tab[index.min(15)]
}

/// Mario BG/CHR decompress stand-in (ROM `dec_chr_l` / `dec_bg_l` / `dec_bg3_l`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecTarget {
    /// `dec_chr_l` — `m_decaddr = dec_base`.
    Chr,
    /// `dec_bg_l` — offset `scr_offset`, addr `dec_base+6144`.
    Bg,
    /// `dec_bg3_l` — offset 0, addr `dec_base+6144`.
    Bg3,
}

/// Counts Mario decompress invocations (HD has no GSU `mdecrunch`).
#[derive(Debug, Clone, Default)]
pub struct DecRun {
    pub chr: u32,
    pub bg: u32,
    pub bg3: u32,
}

impl DecRun {
    pub fn run(&mut self, target: DecTarget) {
        match target {
            DecTarget::Chr => self.chr = self.chr.wrapping_add(1),
            DecTarget::Bg => self.bg = self.bg.wrapping_add(1),
            DecTarget::Bg3 => self.bg3 = self.bg3.wrapping_add(1),
        }
    }
}

/// ROM `fadelines` / `fadelines2` — disabled on cart (`IFEQ 1`); HD no-op
/// that records the call for ledger coverage.
#[derive(Debug, Clone, Default)]
pub struct FadeLines {
    pub calls: u32,
}

impl FadeLines {
    pub fn fade_lines(&mut self) {
        self.calls = self.calls.wrapping_add(1);
    }
    pub fn fade_lines2(&mut self) {
        self.calls = self.calls.wrapping_add(1);
    }
}

/// ROM `p_init_sprites_l` — clear OAM staging (same as `clearsprites_l`).
pub fn p_init_sprites(clear: &mut dyn FnMut()) {
    clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_avail() {
        let mut h = StratHeap::new(256);
        assert_eq!(h.avail(), 256);
        let a = h.alloc(8).unwrap();
        assert!(h.avail() < 256);
        h.free(a);
        assert_eq!(h.avail(), 256);
    }

    #[test]
    fn salloc_chain_and_sallfree() {
        let mut h = StratHeap::new(512);
        let mut mp = 0u16;
        let a = h.salloc(&mut mp, 4).unwrap();
        let b = h.salloc(&mut mp, 4).unwrap();
        assert_eq!(mp, b as u16);
        h.sfree(&mut mp, a);
        h.sallfree(&mut mp);
        assert_eq!(mp, 0);
        assert_eq!(h.avail(), 512);
    }

    #[test]
    fn mpush_mpull_lifo() {
        let mut h = StratHeap::new(512);
        let s0 = h.mpush(None, 0x1111_2222).unwrap();
        let s1 = h.mpush(Some(s0), 0x3333_4444).unwrap();
        let (sp, d) = h.mpull(s1).unwrap();
        assert_eq!(d, 0x3333_4444);
        assert_eq!(sp, Some(s0));
        let (sp2, d2) = h.mpull(sp.unwrap()).unwrap();
        assert_eq!(d2, 0x1111_2222);
        assert_eq!(sp2, None);
    }
}
