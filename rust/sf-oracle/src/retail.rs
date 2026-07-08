//! TIER-2 retail co-execution harness (proof-of-concept).
//!
//! Ground truth = the **retail** cart `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM),
//! which is a *different binary* from the symbol-mapped built ROM
//! (`data/sf.sfc`, 2 MB) — see docs/FUNCTION_LEDGER.md. The built ROM's symbol
//! map therefore does NOT apply to retail; the observable-state addresses used
//! here were re-derived from the retail cart itself.
//!
//! What this module provides:
//!  * [`boot_retail`] — boot the retail cart from its real reset vector and
//!    report how far the 65816 CPU gets (this is a Super-FX game, so the CPU
//!    side is a shell that hands 3D/render to the GSU; a CPU-only core cannot
//!    reach live gameplay — we report exactly where it stalls).
//!  * [`RETAIL_POOL`] — the retail object-array (alien pool) layout, located by
//!    scanning the retail ROM for the `FmtFreeLst` allocator-init signature and
//!    confirming the world-coordinate struct offsets against the built ROM.
//!  * [`snapshot_objects`] — read the object array out of retail WRAM into a
//!    plain [`Vec<ObjState>`], the observable-state snapshot that a Rust-port
//!    diff compares against.
//!  * [`init_object_pool`] — execute the retail cart's own allocator-init
//!    routine on a [`SnesBus`], proving the pool structure is readable from
//!    real retail code (not just seeded by us).

use crate::{call, Entry, SnesBus};

/// Retail object-array (alien pool) layout.
///
/// Derived from the retail cart (NOT the built-ROM symbol map):
///  * The allocator-init routine (`kill_list`/`FmtFreeLst` in the reference
///    source) was located at retail SNES **$02:F4D8** by scanning for the
///    signature `tax; clc; adc #al_size; sta _next,x; dey; bne` preceded by
///    `lda #pool; sta freelst; ldy #count`. Retail emits:
///      `lda #$0336; sta $121F; ldy #$0046; tax; clc; adc #$0036; sta $00,x; …`
///    → pool base **$0336**, freelist-head var **$121F**, count **70**,
///    struct stride **$36 = 54 bytes**.
///  * The built ROM's equivalent (`$02:F03B`) is `pool=$0338, freelst=$12AF,
///    stride=$38=56` — so retail's pool is shifted -2 and each struct is 2
///    bytes shorter (the shrink is in the struct *tail*).
///  * The *field* offsets we observe (shape/flags/worldx/y/z) are UNCHANGED:
///    scanning both ROMs for the world-coord store triple `sta worldx,y;
///    sta worldx+2,y; sta worldx+4,y` gives an identical dominant base of
///    **$0C** in retail and built alike (44 sites each). So al_worldx/y/z =
///    $0C/$0E/$10, al_shape = $04, al_flags = $08 hold for retail too.
pub struct PoolLayout {
    /// WRAM offset (bank $7E low RAM) of the first object block.
    pub base: u32,
    /// Bytes per object block.
    pub stride: u32,
    /// Number of blocks in the pool.
    pub count: u32,
    /// WRAM offset of the free-list head pointer.
    pub freelist_head: u32,
    /// Field offsets within a block.
    pub al_shape: u32,
    pub al_flags: u32,
    pub al_worldx: u32,
    pub al_worldy: u32,
    pub al_worldz: u32,
    /// `_next` link (free-list / active-list) offset within a block.
    pub al_next: u32,
    /// WRAM offset of the active-list head pointer (`allst`).
    pub active_head: u32,
}

/// Retail cart layout (see [`PoolLayout`]).
pub const RETAIL_POOL: PoolLayout = PoolLayout {
    base: 0x0336,
    stride: 0x36, // 54
    count: 70,
    freelist_head: 0x121F,
    al_shape: 0x04,
    al_flags: 0x08,
    al_worldx: 0x0C,
    al_worldy: 0x0E,
    al_worldz: 0x10,
    al_next: 0x00,
    // `allst` sits just below `alfreelst` in the built ROM ($12AD vs $12AF);
    // retail's freelist head is $121F, so allst is $121D by the same adjacency.
    active_head: 0x121D,
};

/// Built-ROM layout, for cross-reference / diffing the two allocators.
pub const BUILT_POOL: PoolLayout = PoolLayout {
    base: 0x0338,
    stride: 0x38, // 56
    count: 70,
    freelist_head: 0x12AF,
    al_shape: 0x04,
    al_flags: 0x08,
    al_worldx: 0x0C,
    al_worldy: 0x0E,
    al_worldz: 0x10,
    al_next: 0x00,
    active_head: 0x12AD,
};

/// Retail cart's allocator-init (`kill_list`/`FmtFreeLst`) routine — a
/// `JSL`/`RTL` far call. SNES **$02:F4C9** (the `php` entry): the routine body
/// is `php; rep #$30; stz $121D (allst); lda #$0336; sta $121F (alfreelst);
/// ldy #$0046; …; plp; rtl`. The `FmtFreeLst` core (`tax` at $02:F4D8) is 15
/// bytes in; the callable entry is the `php`. Located by ROM signature scan
/// (see [`PoolLayout`]).
pub const RETAIL_KILL_LIST: u32 = 0x02_F4C9;

/// One object slot's observable state, read from WRAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjState {
    /// Slot index (0..count).
    pub slot: u32,
    pub shape: u16,
    pub flags: u16,
    pub worldx: i16,
    pub worldy: i16,
    pub worldz: i16,
    /// `_next` link value (0 = list terminator).
    pub next: u16,
}

/// Snapshot every slot of the object pool out of `bus`'s WRAM.
///
/// This reads the *raw* pool (all `count` blocks) regardless of whether a slot
/// is on the active or free list — the caller decides which slots are "live"
/// (typically: non-zero shape, or membership of the `allst` active list). This
/// is the observable game state that a Rust-port tick is diffed against.
pub fn snapshot_objects(bus: &SnesBus, pool: &PoolLayout) -> Vec<ObjState> {
    (0..pool.count)
        .map(|slot| {
            let b = pool.base + slot * pool.stride;
            ObjState {
                slot,
                shape: bus.wram_read16(b + pool.al_shape),
                flags: bus.wram_read16(b + pool.al_flags),
                worldx: bus.wram_read16(b + pool.al_worldx) as i16,
                worldy: bus.wram_read16(b + pool.al_worldy) as i16,
                worldz: bus.wram_read16(b + pool.al_worldz) as i16,
                next: bus.wram_read16(b + pool.al_next),
            }
        })
        .collect()
}

/// Walk the free-list starting at `freelist_head` and return the block offsets
/// in link order. Proves the retail allocator built a coherent linked list.
pub fn walk_freelist(bus: &SnesBus, pool: &PoolLayout) -> Vec<u16> {
    let mut out = Vec::new();
    let mut p = bus.wram_read16(pool.freelist_head);
    let mut guard = 0;
    while p != 0 && guard <= pool.count {
        out.push(p);
        p = bus.wram_read16(p as u32 + pool.al_next);
        guard += 1;
    }
    out
}

/// Run the retail cart's own allocator-init routine on `bus` (via the existing
/// `call`/RTL harness), formatting the object pool's free-list in WRAM exactly
/// as the retail game does at level start. After this, [`snapshot_objects`] and
/// [`walk_freelist`] read real retail-produced state.
pub fn init_object_pool(bus: &mut SnesBus) {
    // ai16 entry (16-bit A/X/Y); the routine PHPs/REP #$30 itself but we hand it
    // a native 16-bit context to match.
    call(bus, RETAIL_KILL_LIST, &Entry { p: 0x00, ..Default::default() });
}

// ------------------------------------------------------------------------
// Retail boot-from-reset probe (milestone 1).
// ------------------------------------------------------------------------

use w65c816::{AddressType, Signals, System, CPU};

/// A bus that boots the retail cart from its *real* reset vector (unlike
/// [`SnesBus`], which overrides $FFFC to a bootstrap stub for direct subroutine
/// calls). Hardware registers are lightly stubbed so the boot can make forward
/// progress far enough to characterise where a CPU-only core stalls.
pub struct RetailBootBus {
    inner: SnesBus,
    res_line: bool,
    /// Toggles each PPU-status read so vblank-wait loops don't spin forever.
    vbl_toggle: bool,
}

impl RetailBootBus {
    pub fn new(rom: Vec<u8>) -> Self {
        RetailBootBus { inner: SnesBus::new(rom), res_line: true, vbl_toggle: false }
    }

    fn reg_read(&mut self, off: u16) -> Option<u8> {
        match off {
            // RDNMI ($4210): bit7 = NMI/vblank flag, low nibble = CPU version (2).
            0x4210 => {
                self.vbl_toggle = !self.vbl_toggle;
                Some(if self.vbl_toggle { 0x82 } else { 0x02 })
            }
            // HVBJOY ($4212): bit7 = vblank, bit0 = auto-joypad-read done.
            0x4212 => {
                self.vbl_toggle = !self.vbl_toggle;
                Some(if self.vbl_toggle { 0x81 } else { 0x01 })
            }
            // APU I/O ports ($2140-$2143): return 0 (no SPC handshake modelled).
            0x2140..=0x2143 => Some(0x00),
            _ => None,
        }
    }
}

impl System for RetailBootBus {
    fn read(&mut self, addr: u32, _at: AddressType, _s: &Signals) -> u8 {
        let bank = (addr >> 16) & 0xFF;
        let off = (addr & 0xFFFF) as u16;
        if (bank <= 0x3F || (0x80..=0xBF).contains(&bank)) && (0x2000..0x6000).contains(&off) {
            if let Some(v) = self.reg_read(off) {
                return v;
            }
        }
        self.inner.read8(addr)
    }
    fn write(&mut self, addr: u32, data: u8, _at: AddressType, _s: &Signals) {
        self.inner.write8(addr, data);
    }
    fn res(&mut self) -> bool {
        let r = self.res_line;
        self.res_line = false;
        r
    }
}

/// Result of booting the retail cart from reset.
#[derive(Debug, Clone)]
pub struct BootReport {
    /// CPU instructions retired (approx; counted at opcode fetches).
    pub steps: u64,
    /// Final program-bank:PC.
    pub final_pbr: u8,
    pub final_pc: u16,
    /// Whether the CPU entered a tight self-loop (same small PC set revisited).
    pub stalled_in_loop: bool,
    /// The looping PC range if `stalled_in_loop`.
    pub loop_lo: u16,
    pub loop_hi: u16,
    pub loop_bank: u8,
    /// Whether the CPU executed a STP/hung.
    pub stopped: bool,
    /// Distinct (bank<<16|pc) opcode addresses visited — a coarse "how much
    /// real code ran" measure.
    pub distinct_pcs: usize,
    /// The most-revisited opcode address and its hit count — the "hot spot" the
    /// CPU parks in once boot hands off to hardware it can't service.
    pub hottest_pc: u32,
    pub hottest_hits: u64,
    /// First few opcode addresses after reset (bank<<16 | pc), for a sanity
    /// trace that the reset really vectored into bank $1F boot code.
    pub head_trace: Vec<u32>,
}

/// Boot the retail cart from its real reset vector and run up to `max_steps`
/// opcode fetches, detecting a stall (tight loop) so we can report precisely how
/// far a CPU-only core gets on this Super-FX title.
pub fn boot_retail(rom: Vec<u8>, max_steps: u64) -> BootReport {
    let mut bus = RetailBootBus::new(rom);
    let mut cpu = CPU::new();

    let mut steps: u64 = 0;
    let mut head_trace = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut freq: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    // Loop detection: track opcode-fetch addresses in a sliding window and flag
    // when the same tiny window of addresses repeats many times.
    let mut recent: std::collections::VecDeque<u32> = std::collections::VecDeque::new();
    let mut repeat_run = 0u32;
    let mut last_window_sig = 0u64;
    let mut stalled = false;
    let mut loop_lo = 0u16;
    let mut loop_hi = 0u16;
    let mut loop_bank = 0u8;

    let mut prev_pc = u32::MAX;
    let mut cycles = 0u64;
    let cyc_cap = max_steps.saturating_mul(64);
    while steps < max_steps && cycles < cyc_cap {
        cpu.cycle(&mut bus);
        cycles += 1;
        let cur = ((cpu.pbr() as u32) << 16) | cpu.pc() as u32;
        // Count an instruction boundary each time PC moves to a new fetch after
        // the core settled (coarse but adequate for "how far").
        if cur != prev_pc {
            prev_pc = cur;
            steps += 1;
            visited.insert(cur);
            *freq.entry(cur).or_insert(0) += 1;
            if head_trace.len() < 24 {
                head_trace.push(cur);
            }
            recent.push_back(cur);
            if recent.len() > 8 {
                recent.pop_front();
            }
            // Window signature over the last 8 addresses.
            let lo = *recent.iter().min().unwrap();
            let hi = *recent.iter().max().unwrap();
            let sig = ((lo as u64) << 32) | hi as u64;
            if recent.len() == 8 && (hi - lo) < 0x40 && sig == last_window_sig {
                repeat_run += 1;
                if repeat_run > 2000 {
                    stalled = true;
                    loop_lo = (lo & 0xFFFF) as u16;
                    loop_hi = (hi & 0xFFFF) as u16;
                    loop_bank = (lo >> 16) as u8;
                    break;
                }
            } else {
                repeat_run = 0;
                last_window_sig = sig;
            }
        }
        if cpu.stopped() {
            break;
        }
    }

    let (hottest_pc, hottest_hits) =
        freq.iter().max_by_key(|(_, &c)| c).map(|(&a, &c)| (a, c)).unwrap_or((0, 0));
    // Treat "ran a lot but revisited very few addresses" as a stall even if the
    // tight-window heuristic didn't trip (boot wait loops can span >0x40 or bank
    // boundaries).
    if !stalled && steps > 100_000 && visited.len() < 400 {
        stalled = true;
        loop_bank = (hottest_pc >> 16) as u8;
        loop_lo = (hottest_pc & 0xFFFF) as u16;
        loop_hi = loop_lo;
    }

    BootReport {
        steps,
        final_pbr: cpu.pbr(),
        final_pc: cpu.pc(),
        hottest_pc,
        hottest_hits,
        stalled_in_loop: stalled,
        loop_lo,
        loop_hi,
        loop_bank,
        stopped: cpu.stopped(),
        distinct_pcs: visited.len(),
        head_trace,
    }
}
