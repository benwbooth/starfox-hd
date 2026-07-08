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

/// Retail cart's per-object motion integrator `addalvecs_l` — the routine the
/// per-frame tick applies to every live object: `worldx/y/z += vx/vy/vz` (16-bit
/// wrapping). Located at retail SNES **$1F:C7BB** by an EXACT byte-signature scan
/// (`c2 20  b5 0C 18 75 2F 95 0C  b5 0E 18 75 31 95 0E  b5 10 18 75 33 95 10
/// e2 20 6b` — REP #$20; LDA/CLC/ADC/STA over the world/vel struct offsets;
/// SEP #$20; RTL). The pattern is byte-identical in the built ROM (its
/// `ADDALVECS_L` sits 0x18 bytes later at $1F:C7D3) because it touches only the
/// world/velocity *struct offsets* ($0C/$0E/$10 and $2F/$31/$33), which are
/// proven identical across both carts — so this is the genuine retail routine,
/// not a built-ROM stand-in. The scan finds exactly one occurrence.
pub const RETAIL_ADDALVECS_L: u32 = 0x1F_C7BB;

/// Velocity field offsets within an object block (struct-relative; identical in
/// retail and built — see [`PoolLayout`] doc). `addalvecs_l` reads these.
pub const AL_VX: u32 = 0x2F;
pub const AL_VY: u32 = 0x31;
pub const AL_VZ: u32 = 0x33;
/// `al_stratptr` — the object's strategy routine pointer (3 bytes: low word at
/// $16, program bank at $18). Struct-relative, identical in retail and built
/// (`GILESAL.INC defal stratptr,3` starts the strat sub-block at $16; verified:
/// $16+$14=$2A=al_HP and $16+$19=$2F=al_vx, both matching `do_strat_l`'s
/// `lda al_HP,x`/`al_collflags,x` operands). `do_strat_l` reads this and
/// RTL-dispatches to it as the object's per-frame strat.
pub const AL_STRATPTR: u32 = 0x16;

// ------------------------------------------------------------------------
// FULL per-frame strat-tick pipeline — retail addresses located by MASKED
// signature scan (opcodes fixed, absolute operands wildcarded) and then
// **cross-validated** by reading the embedded operands back out.
//
// The keystone is `dostrats`: found by scanning retail for the masked skeleton
//   inc gameframe; bne+3; inc gameframe+1; phb; lda #$7e; pha; plb;
//   jsl init_strats_l; jsl update_objects_l; ldx allst; stz aldead;
//   jsl do_strat_l; lda aldead; bne; ldy _next,x; tyx; bne
// (exactly ONE hit in retail → $02:DAF2). Its embedded JSL/absolute operands
// then DIRECTLY yield the addresses below — no separate scan needed — and the
// derivation is self-validating: `ldx allst` reads **$121D**, byte-identical to
// the independently-derived [`RETAIL_POOL`]`.active_head`, and every located
// routine's opcode skeleton matches its built-ROM counterpart.
// ------------------------------------------------------------------------

/// Retail `dostrats` — the per-frame strat walk (near / `RTS`). $02:DAF2.
/// `incw gameframe; phb; ldb #$7e; jsl init_strats_l; jsl update_objects_l;
/// ldx allst; {stz aldead; jsl do_strat_l; ... ldy _next,x; tyx; bne}; plb; rts`.
pub const RETAIL_DOSTRATS: u32 = 0x02_DAF2;
/// Retail `init_strats_l` — per-frame reset (coll/player-move init). $06:81D5
/// (JSL target embedded in `dostrats`; built lives in bank $02, retail in $06).
pub const RETAIL_INIT_STRATS_L: u32 = 0x06_81D5;
/// Retail `update_objects_l` — per-frame scroll/delta update. $03:ED7E
/// (JSL target embedded in `dostrats`).
pub const RETAIL_UPDATE_OBJECTS_L: u32 = 0x03_ED7E;
/// Retail `do_strat_l` — single-object strat dispatch (`JSL`/`RTL`). $1F:D26B
/// (JSL target embedded in `dostrats`; opcode skeleton matches built $1F:D283).
/// Copies `al_worldx/y/z,x -> stratobj_posx/y/z`, sets `al1pt=x`, then computes
/// the object's strat pointer and RTL-jumps to it; a null `al_stratptr` returns
/// cleanly via `.strad (plp; rtl)`.
pub const RETAIL_DO_STRAT_L: u32 = 0x1F_D26B;
/// Retail `mapobjdo` — the map-bytecode spawn VM entry. $03:F79B (first of a
/// 5-member family `bb bd 01 80 8d mapobjnext …`, all reusing `ldx allst=$121D`).
pub const RETAIL_MAPOBJDO: u32 = 0x03_F79B;
/// Retail `newobjex` — object spawner core (`e2 20 ad mapptr …`). $03:EDAB.
pub const RETAIL_NEWOBJEX: u32 = 0x03_EDAB;
/// Retail `newobjs_l` — far spawn entry (`php; sep; rep; phb; jsr newobjex; …`).
/// $03:EDA1 (the 10-byte `JSL`/`RTL` wrapper immediately preceding `newobjex`).
pub const RETAIL_NEWOBJS_L: u32 = 0x03_EDA1;

/// Retail per-frame strat globals (WRAM), auto-derived from the embedded
/// operands of `dostrats` + `do_strat_l`. Built-ROM equivalents in parens.
pub const RETAIL_GAMEFRAME: u32 = 0x15BB; // built $1640
pub const RETAIL_ALDEAD: u32 = 0x1248; //    built $12D3
pub const RETAIL_DUMMYOBJ: u32 = 0x156B; //  built $15F6
pub const RETAIL_STRATOBJ_POSX: u32 = 0x1513; // built $159E
pub const RETAIL_AL1PT: u32 = 0x123A; //     built $12C5
pub const RETAIL_MARIO_DRAW_MODE: u32 = 0x1260; // built $12EB

// ------------------------------------------------------------------------
// FIRST NAMED ENEMY-STRAT CERTIFICATION — the `stayrel` ground family.
//
// These are the simplest per-tick enemy strats in the port: a pure world-Z
// scroll (`worldz += pviewvelz`). Their ENTIRE per-tick body is a single
// `jsl sr_addplayerZx; rtl`, so the retail computation is fully captured by
// `sr_addplayerZx` — a leaf routine that touches exactly ONE global (`pviewvelz`)
// and one struct field (`al_worldz`). All addresses located by masked scan and
// cross-validated (see tests/coexec_retail.rs::retail_stayrel_family_addresses).
// ------------------------------------------------------------------------

/// Retail `sr_addplayerZx` ($1F:DC69) — the leaf routine every scroll strat
/// calls: `s_add_alvar W,x,al_worldz,pviewvelz; rtl`, i.e. 16-bit
/// `al_worldz,x += pviewvelz`. Body bytes: `C2 20 (rep #$20)  B5 10 (lda
/// al_worldz,x)  18 (clc)  6D F4 14 (adc pviewvelz)  95 10 (sta al_worldz,x)
/// E2 20 (sep #$20)  6B (rtl)`. Located by scanning for that skeleton with the
/// ADC operand wildcarded: 8 byte-matches, but this is the ONLY one that is
/// actually CALLED (247 `jsl` references / 97 of them `jsl X; rtl` pure-scroll
/// strat bodies) — the other 7 are inlined `worldz += <other global>` motifs
/// with zero references. Its embedded `adc` operand IS the retail `pviewvelz`.
pub const RETAIL_SR_ADDPLAYERZX: u32 = 0x1F_DC69;
/// Retail `pviewvelz` ($14F4) — the view-Z scroll velocity, read straight out
/// of `sr_addplayerZx`'s `adc pviewvelz` operand. Written only by the PLAYER
/// strats (PSTRATS/PCSTRATS/PISTRATS), never by `update_objects_l`/
/// `init_strats_l`, so a directly-seeded value survives a strat tick.
pub const RETAIL_PVIEWVELZ: u32 = 0x14F4;
/// Retail `stayrelhard180YR_strat` ($06:8646) — pure scroll strat body,
/// `jsl sr_addplayerZx; rtl` (`22 69 DC 1F 6B`). Identified as the pure-scroll
/// routine (one of 97) immediately preceding the UNIQUE `stayrel_strat`.
pub const RETAIL_STAYRELHARD180YR_STRAT: u32 = 0x06_8646;
/// Retail `stayrel_strat` ($06:864B) — scroll + set the `colldisable` sflag:
/// `jsl sr_addplayerZx; lda al_sflags2,x; ora #$01; sta al_sflags2,x; rtl`
/// (`22 69 DC 1F  B5 1E 09 01 95 1E  6B`). Located by masked scan: exactly ONE
/// hit. Its `sta` operand pins `al_sflags2 = $1E` (so `al_sflags = $1D`), and
/// `ora #$01` confirms `colldisable` = sflag bit 8 (`asf_colldisable`>>8 = $01),
/// i.e. it lives in the SECOND sflags byte — a different bit layout from the
/// port's C `obj.h` (colldisable = `al_sflags` bit `$10`); see the cert test.
pub const RETAIL_STAYREL_STRAT: u32 = 0x06_864B;
/// `al_sflags` / `al_sflags2` struct offsets (from `stayrel_strat`'s operand).
pub const AL_SFLAGS: u32 = 0x1D;
pub const AL_SFLAGS2: u32 = 0x1E;

/// Retail `runmario_l` — the RAM-resident GSU trampoline. Two addresses:
///  * ROM copy-source `$02:9D56` — where the 35-byte routine is stored in the
///    cart (`sta.l m_pbr; phb; ldb #0; lda mario_draw_mode; ora #$18;
///    sta m_scmr; stx mr15; .wait lda m_sfr; and #$20; bne; sta m_scmr; plb;
///    rtl`). Byte-identical to the built copy at `$02:9D32` except the
///    `mario_draw_mode` operand ($1260 vs $12EB).
///  * RAM destination `$7E:4EE9` — where the boot copies it and where every
///    `jsl runmario_l` in retail code points (63 call sites; the single most
///    common bank-$7E JSL target — built's is `$7E:4F51`, 58 sites). The
///    intra-block sub-entries line up too: retail `$7E:4F10`/`$7E:4F55` are
///    built `$7E:4F78`/`$7E:4FBD` at the identical +$27/+$6C offsets.
pub const RETAIL_RUNMARIO_L_ROM: u32 = 0x02_9D56;
pub const RETAIL_RUNMARIO_RAM: u32 = 0x7E_4EE9;
/// Built-ROM `runmario_l`: ROM copy-source `$02:9D32`, RAM dest `$7E:4F51`.
pub const BUILT_RUNMARIO_L_ROM: u32 = 0x02_9D32;
pub const BUILT_RUNMARIO_RAM: u32 = 0x7E_4F51;
/// Length of the `runmario_l` routine in bytes (`sta.l`…`rtl`).
pub const RUNMARIO_LEN: u32 = 0x23;

/// Install the `runmario_l` GSU trampoline into WRAM so a `jsl runmario_l` from
/// within game code (e.g. a strat inside `dostrats`) reaches a live routine
/// instead of empty RAM. Copies the `RUNMARIO_LEN`-byte routine from its ROM
/// copy-source (`rom_src`) to its RAM destination (`ram_dst`) — exactly what the
/// cart's boot does. The bytes drive the memory-mapped GSU registers the bus
/// already wires ([`SnesBus::enable_gsu`]): `sta.l m_pbr`, `stx mr15` (the R15
/// high-byte write kicks the chip), `.wait lda m_sfr; and #$20; bne`.
pub fn inject_runmario_trampoline(bus: &mut SnesBus, rom_src: u32, ram_dst: u32) {
    for i in 0..RUNMARIO_LEN {
        let b = bus.read8(rom_src + i);
        bus.write8(0x7E_0000 | (ram_dst & 0xFFFF).wrapping_add(i), b);
    }
}

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
///
/// The PPU shim models a **free-running raster** — a dot counter advanced once
/// per CPU clock (see [`boot_retail`]) that sweeps H (0..341) and V (0..262) —
/// so that scanline/vblank spin loops (e.g. the `$03:BD97` OPVCT raster-wait)
/// actually satisfy instead of parking forever. This is the *minimal* hardware
/// needed to march the boot into the per-frame game loop; it is NOT a real PPU
/// (no framebuffer, no rendering, no OAM/CGRAM effects).
pub struct RetailBootBus {
    inner: SnesBus,
    res_line: bool,
    /// Free-running dot counter (advanced by [`boot_retail`] each CPU clock).
    /// H = `dot % DOTS_PER_LINE`, V = `(dot / DOTS_PER_LINE) % LINES_PER_FRAME`.
    pub dot: u64,
    /// Latched H/V counters (set by a read of $2137 SLHV, or by any OPHCT/OPVCT
    /// read — hardware latches on H/V read too). Consumed low-then-high-bit via
    /// the read toggles below.
    latched_h: u16,
    latched_v: u16,
    ophct_hi: bool,
    opvct_hi: bool,
    /// Sticky NMI-occurred flag ($4210 bit7): set when V crosses into vblank,
    /// cleared on read (real RDNMI semantics). Lets `bit $4210 / bpl` frame
    /// waits both arm and re-arm.
    nmi_latch: bool,
    prev_vblank: bool,
    /// --- Minimal SPC700 upload-handshake shim (ports $2140-$2143) ---
    /// The retail boot uploads several audio blocks (driver, samples, sequences)
    /// through a Nintendo-IPL-style protocol. Two states:
    ///  * **Idle** (`!apu_active`): ports read $AA/$BB — the "SPC ready" signal
    ///    the `$03:B12E` `CMP #$BBAA` loop waits for. A `$FF` write (the driver
    ///    "re-arm" nudge at `$03:B11E`) is ignored; any other $2140 write (the
    ///    `$CC` kick) enters Active.
    ///  * **Active** (`apu_active`): $2140 echoes the last value written — which
    ///    satisfies the `$CC` start-echo AND every per-byte index-echo wait of
    ///    the block-upload, no real SPC700 needed.
    /// The upload routine terminates each block with `STZ $2141/$2142/$2143`
    /// (`$03:B204`); a `$00` write to $2143 returns us to Idle so the NEXT
    /// block's ready-check passes. This models the upload port protocol ONLY —
    /// not the running SPC music engine (no per-frame command responses).
    apu_active: bool,
    apu_echo: u8,
    /// NMITIMEN ($4200) bit7 — vblank NMI enable. (Star Fox actually drives its
    /// frame timing off the H/V-counter IRQ, not NMI — see `irq_enabled` — but
    /// we honour NMI too in case a code path uses it.)
    nmi_enabled: bool,
    /// NMITIMEN ($4200) bits 4/5 — H/V-counter IRQ enable. Star Fox writes
    /// $4200 = $31 (H+V IRQ + auto-joypad). When set we fire the CPU IRQ line
    /// once per frame at the programmed scanline so the game's IRQ handler
    /// ($00:010C RAM trampoline -> $02:88xx) runs, sets the frame-ready flag
    /// $18BB the main loop ($02:DA3B) spins on, and RTIs — turning the top-of-
    /// frame wait into an actual per-frame tick.
    irq_enabled: bool,
    /// Programmed V-count IRQ line ($4209/$420A VTIME); default = vblank start.
    irq_vtime: u16,
    /// IRQ request latched at the target scanline, cleared by a $4211 ack read.
    irq_pending: bool,
    /// Auto-joypad-read result presented on $4218 (JOY1L) / $4219 (JOY1H).
    /// Bit layout (16-bit): B Y Sel Start Up Dn Lt Rt A X L R 0 0 0 0.
    /// Default 0 = no buttons; set via [`RetailBootBus::set_pad1`] to script
    /// input for the co-exec harness.
    pad1: u16,
}

/// Dots per scanline (SNES: 341 dots, 340 on some lines — we use the nominal).
const DOTS_PER_LINE: u64 = 341;
/// Scanlines per frame (NTSC nominal).
const LINES_PER_FRAME: u64 = 262;
/// First scanline of vblank (NTSC: 225 = $E1 after 224 visible lines).
const VBLANK_START_LINE: u64 = 225;

impl RetailBootBus {
    pub fn new(rom: Vec<u8>) -> Self {
        RetailBootBus {
            inner: SnesBus::new(rom),
            res_line: true,
            dot: 0,
            latched_h: 0,
            latched_v: 0,
            ophct_hi: false,
            opvct_hi: false,
            nmi_latch: false,
            prev_vblank: false,
            apu_active: false,
            apu_echo: 0,
            nmi_enabled: false,
            irq_enabled: false,
            irq_vtime: VBLANK_START_LINE as u16,
            irq_pending: false,
            pad1: 0,
        }
    }

    /// Set the controller-1 button state presented to the auto-joypad registers.
    pub fn set_pad1(&mut self, buttons: u16) {
        self.pad1 = buttons;
    }

    #[inline]
    fn cur_h(&self) -> u16 {
        (self.dot % DOTS_PER_LINE) as u16
    }
    #[inline]
    fn cur_v(&self) -> u16 {
        ((self.dot / DOTS_PER_LINE) % LINES_PER_FRAME) as u16
    }
    #[inline]
    fn in_vblank(&self) -> bool {
        (self.cur_v() as u64) >= VBLANK_START_LINE
    }

    /// Advance the raster by one CPU clock and update the sticky NMI flag +
    /// scanline IRQ latch on the relevant raster edges. Called once per
    /// `cpu.cycle` by [`boot_retail`].
    pub fn tick_raster(&mut self) {
        let prev_v = self.cur_v();
        self.dot = self.dot.wrapping_add(1);
        let v = self.cur_v();
        let vb = self.in_vblank();
        if vb && !self.prev_vblank {
            self.nmi_latch = true; // vblank just began -> NMI would fire
        }
        self.prev_vblank = vb;
        // Latch a scanline IRQ once, when V first reaches the programmed line.
        if self.irq_enabled && v == self.irq_vtime && prev_v != self.irq_vtime {
            self.irq_pending = true;
        }
    }

    fn reg_read(&mut self, off: u16) -> Option<u8> {
        match off {
            // SLHV ($2137): reading latches the current H/V counters.
            0x2137 => {
                self.latched_h = self.cur_h();
                self.latched_v = self.cur_v();
                self.ophct_hi = false;
                self.opvct_hi = false;
                Some(0)
            }
            // OPHCT ($213C): H counter, low byte then high bit, toggling.
            0x213C => {
                let v = if self.ophct_hi {
                    (self.latched_h >> 8) & 0x01
                } else {
                    self.latched_h & 0xFF
                };
                self.ophct_hi = !self.ophct_hi;
                Some(v as u8)
            }
            // OPVCT ($213D): V counter, low byte then high bit, toggling. A read
            // also latches (hardware latches H/V on OPHCT/OPVCT access), so a
            // loop that skips $2137 still sweeps.
            0x213D => {
                if !self.opvct_hi {
                    self.latched_v = self.cur_v();
                }
                let v = if self.opvct_hi {
                    (self.latched_v >> 8) & 0x01
                } else {
                    self.latched_v & 0xFF
                };
                self.opvct_hi = !self.opvct_hi;
                Some(v as u8)
            }
            // RDNMI ($4210): bit7 = NMI-occurred (sticky, cleared on read),
            // low nibble = CPU version (2).
            0x4210 => {
                let b7 = if self.nmi_latch { 0x80 } else { 0x00 };
                self.nmi_latch = false;
                Some(b7 | 0x02)
            }
            // TIMEUP ($4211): bit7 = H/V-IRQ occurred; reading acks (clears)
            // the IRQ line, exactly as the IRQ handler does to dismiss it.
            0x4211 => {
                let b7 = if self.irq_pending { 0x80 } else { 0x00 };
                self.irq_pending = false;
                Some(b7)
            }
            // HVBJOY ($4212): bit7 = vblank, bit6 = hblank, bit0 = auto-joypad
            // busy (0 = ready). Reflects the real raster so both `bmi`/`bpl`
            // spin directions resolve.
            0x4212 => {
                let mut v = 0u8;
                if self.in_vblank() {
                    v |= 0x80;
                }
                // hblank: dots outside the active 0..256 region.
                if self.cur_h() >= 274 || self.cur_h() < 1 {
                    v |= 0x40;
                }
                Some(v) // bit0 = 0: auto-joypad read is "done"
            }
            // APUIO0 ($2140): Idle -> $AA (ready); Active -> echo last write.
            0x2140 => Some(if self.apu_active { self.apu_echo } else { 0xAA }),
            // APUIO1 ($2141): Idle -> $BB (pairs with $AA for the $BBAA ready
            // check); Active the CPU only writes it (data-out).
            0x2141 => Some(if self.apu_active { 0x00 } else { 0xBB }),
            // APUIO2/3 ($2142/$2143): address-in ports, CPU writes only.
            0x2142..=0x2143 => Some(0x00),
            // JOY1L/JOY1H ($4218/$4219): auto-joypad-read controller-1 state.
            0x4218 => Some(self.pad1 as u8),
            0x4219 => Some((self.pad1 >> 8) as u8),
            _ => None,
        }
    }

    /// Intercept writes to the APU ports to drive the upload-handshake shim.
    fn reg_write(&mut self, off: u16, v: u8) {
        match off {
            // NMITIMEN ($4200): bit7 = NMI enable, bits 5/4 = V/H-IRQ enable.
            0x4200 => {
                self.nmi_enabled = (v & 0x80) != 0;
                self.irq_enabled = (v & 0x30) != 0;
            }
            // VTIME ($4209 low / $420A high bit): programmed V-IRQ scanline.
            0x4209 => self.irq_vtime = (self.irq_vtime & 0x100) | v as u16,
            0x420A => self.irq_vtime = (self.irq_vtime & 0x0FF) | (((v as u16) & 1) << 8),
            0x2140 => {
                if self.apu_active {
                    // Echo every index/kick back on the next $2140 read.
                    self.apu_echo = v;
                } else if v != 0xFF {
                    // Idle: the $CC kick starts a block; the $FF re-arm nudge is
                    // ignored (we are already presenting "ready").
                    self.apu_active = true;
                    self.apu_echo = v;
                }
            }
            // Block terminate is `STZ $2143`; a $00 write here returns us to
            // Idle so the next block's $BBAA ready-check passes.
            0x2143 => {
                if v == 0x00 {
                    self.apu_active = false;
                }
            }
            _ => {}
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
        let bank = (addr >> 16) & 0xFF;
        let off = (addr & 0xFFFF) as u16;
        if (bank <= 0x3F || (0x80..=0xBF).contains(&bank))
            && ((0x2140..=0x2143).contains(&off)
                || off == 0x4200
                || off == 0x4209
                || off == 0x420A)
        {
            self.reg_write(off, data);
        }
        self.inner.write8(addr, data);
    }
    fn res(&mut self) -> bool {
        let r = self.res_line;
        self.res_line = false;
        r
    }
    fn nmi(&mut self) -> bool {
        // Assert NMI through vblank (the core edge-triggers on the rising edge,
        // so it fires once per frame) while the game has NMI enabled.
        self.nmi_enabled && self.in_vblank()
    }
    fn irq(&mut self) -> bool {
        // Level-sensitive: held until the handler acks via a $4211 read.
        self.irq_enabled && self.irq_pending
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
    /// Periodic (step, pc) samples so we can see the boot march forward through
    /// distinct code regions instead of only the final resting place.
    pub progress: Vec<(u64, u32)>,
    /// Final raster dot count reached (frames ≈ dot / (341*262)).
    pub final_dot: u64,
    /// Peak number of live object slots (shape != 0) seen during the run.
    pub max_live_objects: usize,
    /// Object-pool snapshot taken at the step where `max_live_objects` peaked.
    pub objects_at_peak: Vec<ObjState>,
    /// Step at which the live-object peak was observed.
    pub peak_step: u64,
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
    let mut progress: Vec<(u64, u32)> = Vec::new();
    let mut last_sample = 0u64;
    let mut last_pool_sample = 0u64;
    let mut max_live_objects = 0usize;
    let mut objects_at_peak: Vec<ObjState> = Vec::new();
    let mut peak_step = 0u64;
    let cyc_cap = max_steps.saturating_mul(64);
    while steps < max_steps && cycles < cyc_cap {
        cpu.cycle(&mut bus);
        // Advance the free-running raster once per CPU clock so scanline/vblank
        // spin loops satisfy.
        bus.tick_raster();
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
            // Periodic progress sample (bounded).
            if steps - last_sample >= 25_000 && progress.len() < 400 {
                progress.push((steps, cur));
                last_sample = steps;
            }
            // Periodically snapshot the object pool and track the live peak.
            if steps - last_pool_sample >= 50_000 {
                last_pool_sample = steps;
                let snap = snapshot_objects(&bus.inner, &RETAIL_POOL);
                let live = snap.iter().filter(|o| o.shape != 0).count();
                if live > max_live_objects {
                    max_live_objects = live;
                    objects_at_peak = snap;
                    peak_step = steps;
                }
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
                // Threshold set well above the game's longest finite busy-wait
                // (a `LDX #$0000; DEX; BNE` delay = 65536 iters ≈ 131 072 PC
                // steps) so real countdown delays pass; only a genuinely
                // unbounded hardware-poll trips this.
                if repeat_run > 400_000 {
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
        progress,
        final_dot: bus.dot,
        max_live_objects,
        objects_at_peak,
        peak_step,
    }
}
