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
/// Retail `dostrats` completion marker — the final `RTS`, after the active
/// object walk and the restoring `PLB`. Watching this instruction lets the
/// oracle sample only fully completed logical frames even when one strategy
/// pass spans several video frames.
pub const RETAIL_DOSTRATS_COMPLETE: u32 = RETAIL_DOSTRATS + 52;
/// Retail `build_drawlist_l` entry. At this boundary `showview_l` has filled
/// and rotated the semantic object draw list, while the downstream display
/// builder has not yet consumed its count.
pub const RETAIL_BUILD_DRAWLIST_L: u32 = 0x02_F6D5;
/// Retail `init_strats_l` — per-frame reset (coll/player-move init). $06:81D5
/// (JSL target embedded in `dostrats`; built lives in bank $02, retail in $06).
pub const RETAIL_INIT_STRATS_L: u32 = 0x06_81D5;
/// Retail `update_objects_l` — per-frame scroll/delta update. $03:ED7E
/// (JSL target embedded in `dostrats`).
pub const RETAIL_UPDATE_OBJECTS_L: u32 = 0x03_ED7E;
/// Retail camera position copied by `marioshowview` before each draw-list
/// transform. These direct-page fields are recovered from the three
/// consecutive `sbc` operands in the retail routine.
pub const RETAIL_VIEW_POSITION_X: u32 = 0x00C1;
pub const RETAIL_VIEW_POSITION_Y: u32 = 0x00C3;
pub const RETAIL_VIEW_POSITION_Z: u32 = 0x00C5;
/// Retail logical sound-effect ring populated by `setport3_l`. The writer and
/// its 16-byte event array are recovered from the unique enqueue routine in
/// the Rev 2 cart.
pub const RETAIL_SOUND_EFFECT_WRITE_CURSOR: u32 = 0x1F4D;
pub const RETAIL_SOUND_EFFECT_EVENTS: u32 = 0x1F53;
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

/// Retail map-VM WRAM globals — derived from `newobjex` / `mapobjdo` operands
/// (see `coexec_retail::retail_map_spawn_vm_addresses`). Built-ROM equivalents
/// in `audit_mapvm2.rs` are `$1780`/`$1782`/`$177C`/`$1AF8`.
pub const RETAIL_MAPCNT: u32 = 0x16FB; // `sta mapcnt` in mapobjdo
pub const RETAIL_MAPPTR: u32 = 0x16FD; // `stx mapptr` on mapcnt≠0 exit
pub const RETAIL_LASTPLAYZ: u32 = 0x16FF;
pub const RETAIL_LASTZCHANGE: u32 = 0x1701;
pub const RETAIL_LASTMAPOBJ: u32 = 0x16F7; // `sty lastmapobj` after spawn
pub const RETAIL_MAPBANK: u32 = 0x1FF4; // `lda mapbank` / `pha; plb` in newobjex
/// Retail `shapes[]` long table (mapobjdo `lda.l shapes,x`).
pub const RETAIL_SHAPES: u32 = 0x00_A64B;
/// Retail `istrats[]` long table (mapobjdo `lda.l istrats,x`).
pub const RETAIL_ISTRATS: u32 = 0x00_A83D;
/// Retail palette-fade WRAM (fadetoseado / fadetogrounddo operands).
/// Built `$19EF`/`$19F3`/`$19F5` / lastpalfade `$19F1`.
pub const RETAIL_PALFADE: u32 = 0x1EE9;
pub const RETAIL_LASTPALFADE: u32 = 0x1EEB;
pub const RETAIL_PALNUM: u32 = 0x1EED;
pub const RETAIL_PALCNT: u32 = 0x1EEF;
/// Retail `pshipflags2` ($14D7) — SETBGM HP0 guard (`and #psf2_playerHP0=$80`).
/// Built `$1562`. Adjacent to [`RETAIL_PSHIPFLAGS3`].
pub const RETAIL_PSHIPFLAGS2: u32 = 0x14D7;
/// Retail controller-screen state recovered from the unique `briefing_l`
/// instruction skeleton in the cart. `pshipflags` distinguishes its
/// controller-layout and destination loops; the remaining fields are the
/// source menu choice, training-return latch, and presentation IRQ latch.
pub const RETAIL_PSHIPFLAGS: u32 = 0x14D6;
pub const RETAIL_BRIEFING_CHOICE: u32 = 0x7E_A05A;
pub const RETAIL_DEFAULT_TRAINING: u32 = 0x1FDF;
pub const RETAIL_PLANET_INTERRUPT: u32 = 0x1F0D;
/// Retail edge-triggered controller bytes (`trig0l` / `trig0h`) read by the
/// controller screen's `testjoypad` macro expansion.
pub const RETAIL_CONTROLLER_TRIGGER_LOW: u32 = 0x1209;
pub const RETAIL_CONTROLLER_TRIGGER_HIGH: u32 = 0x120A;
/// Retail initial route-map globals. The `currentplanet` operand is recovered
/// from the cart's unique `lda #-1; sta currentplanet; lda #6; sta.l
/// mspr_pal` skeleton; the adjacent source layout fixes `stage` and
/// `whichroute`. `fadecount` is the route-line animation countdown.
pub const RETAIL_PLANET_STAGE: u32 = 0x16D6;
pub const RETAIL_WHICH_ROUTE: u32 = 0x16D8;
pub const RETAIL_CURRENT_PLANET: u32 = 0x16D9;
pub const RETAIL_PLANET_FADE_COUNT: u32 = 0x15C2;
/// Direct-page route-map ship-flash latch. `planetseq_l` clears it during
/// setup and stores one only after route confirmation.
pub const RETAIL_PLANET_SHIP_FLASH: u32 = 0x34;
/// Retail `planetseq_l` presentation entry points, recovered by exact opcode
/// signatures from the Rev 2 cart and cross-checked against `PLANETS.ASM`.
/// These are oracle-only execution markers; the native port exposes semantic
/// phases and never depends on cartridge addresses.
pub const RETAIL_PLANET_SHIP_FLASH_ENTRY: u32 = 0x03_C05E;
pub const RETAIL_PLANET_MAP_FADE_ENTRY: u32 = 0x03_C087;
pub const RETAIL_PLANET_ISOLATION_ENTRY: u32 = 0x03_C0A3;
pub const RETAIL_PLANET_CENTER_ENTRY: u32 = 0x03_C0F5;
pub const RETAIL_PLANET_BRIEFING_PREP_ENTRY: u32 = 0x03_C12B;
pub const RETAIL_PLANET_ZOOM_ENTRY: u32 = 0x03_C24E;
pub const RETAIL_PLANET_NAME_ENTRY: u32 = 0x03_C320;
pub const RETAIL_PLANET_MESSAGE_ENTRY: u32 = 0x03_C352;
pub const RETAIL_PLANET_DISMISS_ENTRY: u32 = 0x03_C3C2;
pub const RETAIL_PLANET_EXIT_FADE_ENTRY: u32 = 0x03_C3FD;
pub const RETAIL_PLANET_GAME_START_ENTRY: u32 = 0x03_C437;
/// Long-addressed `pepperchars` type-on cursor used by both text loops.
pub const RETAIL_PEPPER_CHARACTERS: u32 = 0x7E_F0C7;
/// Super FX route-presentation radius (`m_radius`).
pub const RETAIL_PLANET_RADIUS: u32 = 0x70_01F2;
/// Retail `bgm_music` / `bgmcnt` (setbgmdo stores). Built `$1A4B`/`$1A4A`.
pub const RETAIL_BGM_MUSIC: u32 = 0x1F47;
pub const RETAIL_BGMCNT: u32 = 0x1F46;
/// Retail `stayblack`, recovered from the `dopause` operand sequence.
pub const RETAIL_STAYBLACK: u32 = 0x1962;
/// Retail map-loop slots (`maploopdo` operands). Built `$17xx` block.
/// `mapaddrs`=$174B, `maploops`=$1743, `nummaploops`=$1753 (word index, ±2).
pub const RETAIL_MAPADDRS: u32 = 0x174B;
pub const RETAIL_MAPLOOPS: u32 = 0x1743;
pub const RETAIL_NUMMAPLOOPS: u32 = 0x1753;
/// Retail map-JSR stack (`mapjsrdo`/`maprtsdo`). Built `$17B7` nummapjsr.
/// Stack words at `$1703` (return mapptr) / `$1705` (bank); Y-index `$1730`
/// (+3 per push); depth counter `$1732` (inc/dec).
pub const RETAIL_MAPJSR_STACK: u32 = 0x1703;
pub const RETAIL_NUMMAPJSR: u32 = 0x1730;
pub const RETAIL_MAPJSR_DEPTH: u32 = 0x1732;
/// Retail small-state WRAM (setzroton/off, setstage, setbg, mapspecial).
/// Built: dozrot `$1776`, stagecnt `$163E`, currentbg `$17C6`, bgflags `$1A17`,
/// specialobjtotal `$17C1`.
pub const RETAIL_DOZROT: u32 = 0x16F1;
pub const RETAIL_STAGECNT: u32 = 0x15B9;
pub const RETAIL_CURRENTBG: u32 = 0x1741;
pub const RETAIL_BGFLAGS: u32 = 0x1F13;
pub const RETAIL_SPECIALOBJTOTAL: u32 = 0x173C;

/// Translate a source BGS table byte offset into the flat native background
/// catalog identity. The retail table starts after one three-byte header and
/// every background record occupies six bytes.
pub fn retail_background_catalog_id(source_offset: u16) -> Option<u16> {
    const FIRST_BACKGROUND_OFFSET: u16 = 3;
    const BACKGROUND_RECORD_BYTES: u16 = 6;

    let relative = source_offset.checked_sub(FIRST_BACKGROUND_OFFSET)?;
    if relative % BACKGROUND_RECORD_BYTES != 0 {
        return None;
    }
    Some(relative / BACKGROUND_RECORD_BYTES)
}
/// Retail VOFS/HOFS/fade WRAM (vofsonplease / sethofson / setfade*do).
/// Built: bg2scroll `$1F32`, dovofs/dohofs in `$19xx`, fadedir/fade `$18xx`.
pub const RETAIL_BG2SCROLL: u32 = 0x194D;
pub const RETAIL_DOHOFS: u32 = 0x1953;
pub const RETAIL_DOVOFS: u32 = 0x1954;
pub const RETAIL_FADEDIR: u32 = 0x18B2;
pub const RETAIL_FADE: u32 = 0x18B3;
/// Retail `xinidisp1` (mapwaitfade compares to `$80`). Built nearby in `$7E`.
pub const RETAIL_XINIDISP1: u32 = 0x7E_45F4;

/// Retail per-frame strat globals (WRAM), auto-derived from the embedded
/// operands of `dostrats` + `do_strat_l`. Built-ROM equivalents in parens.
pub const RETAIL_GAMEFRAME: u32 = 0x15BB; // built $1640
/// Retail `gameflags` ($14D0). The built-ROM symbol is `$155B`; the retail
/// gameplay-global block has the same independently verified -$8B shift as
/// `pshipflags`, `gameframe`, and the player-position mirrors.
pub const RETAIL_GAMEFLAGS: u32 = 0x14D0;
/// Elapsed display-frame count consumed by `framescalevecs`.
pub const RETAIL_FRAMERATE: u32 = 0x14E3;
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

// ------------------------------------------------------------------------
// BATCH 2 — ground family extension (`staydist`, `gnd`).
//
// These extend the certified `stayrel` family. Located by masked signature
// scan of the retail cart (opcodes fixed, WRAM operands wildcarded), skeletons
// first read out of the BUILT ROM (data/sf.sfc, symbol-mapped) then cross-
// validated against retail. See tests/coexec_retail.rs.
// ------------------------------------------------------------------------

/// Retail `staydist_Istrat` ($06:8656) — the per-tick view-tracking ground
/// strat (the Istrat IS the per-tick body; it never swaps `al_stratptr`). Body:
/// `rep #$20; lda al_sword1,x; sta al_worldz,x; sep #$20; rep #$20;
/// lda al_worldz,x; clc; adc pviewposz; sta al_worldz,x; sep #$20;
/// lda al_sflags2,x; ora #$01; sta al_sflags2,x; rtl`
/// (`C2 20 B5 26 95 10 E2 20  C2 20 B5 10 18 6D <pvp> 95 10 E2 20
/// B5 1E 09 01 95 1E 6B`). Net effect: `al_worldz = al_sword1 + pviewposz` each
/// tick (idempotent — re-derives worldz from sword1, tracking the viewer) plus
/// set `colldisable`. UNIQUE masked hit; sits right after `stayrel_strat`
/// ($06:864B + 11 bytes = $8656). Footprint: reads `pviewposz` + `al_sword1`,
/// writes `al_worldz` + `al_sflags2`.
pub const RETAIL_STAYDIST_ISTRAT: u32 = 0x06_8656;
/// Retail `pviewposz` ($14FA) — read straight out of `staydist_Istrat`'s
/// `adc pviewposz` operand. Cross-validated: `pviewvelz`($14F4) + 6 = $14FA,
/// the identical +6 spacing as the built ROM ($157F -> $1585). Written only by
/// PLAYER/camera strats, so a directly-seeded value survives a tick.
pub const RETAIL_PVIEWPOSZ: u32 = 0x14FA;
/// `al_sword1` struct offset ($26) — the staydist desired-Z-offset scratch
/// word (identical retail/built/port; GILESAL.INC `defal sword1,2`).
pub const AL_SWORD1: u32 = 0x26;

/// Retail `gnd_Istrat` ($08:F15D) — the static ground-plane segment strat
/// (GASTRATS.ASM:3720). INIT-ONLY: zeroes `al_stratptr` (so the per-tick body is
/// a no-op), `jsl set_0collptrsx_l` (zeroes the extended-array coll/exp strat
/// pointers), sets `al_type |= gnd($01)` and `al_sflags2 |= colldisable($01)`.
/// Body: `rep #$20; lda #0; sta al_stratptr,x; sep #$20; lda #0;
/// sta al_stratptr+2,x; jsl set_0collptrs; lda al_type,x; ora #$01;
/// sta al_type,x; lda al_sflags2,x; ora #$01; sta al_sflags2,x; rtl`
/// (`C2 20 A9 00 00 95 16 E2 20 A9 00 95 18 22 <set0coll> B5 09 09 01 95 09
/// B5 1E 09 01 95 1E 6B`). UNIQUE masked hit. Footprint: reads NOTHING (no
/// globals); writes `al_stratptr`, `al_type`, `al_sflags2` (+ extended coll/exp
/// ptrs via the leaf).
pub const RETAIL_GND_ISTRAT: u32 = 0x08_F15D;
/// `al_type` struct offset ($09) — from `gnd_Istrat`'s `lda al_type,x` operand.
pub const AL_TYPE: u32 = 0x09;

// ------------------------------------------------------------------------
// BATCH 2 — a pure ROTATE scenery strat (`hardrot`) and a fixed-velocity
// MOVER (`straight`). Both located by masked signature scan.
// ------------------------------------------------------------------------

/// Retail `hardrot_strat` ($06:8614) — spin-in-place scenery: per axis
/// `al_rot* += al_sbyte*`. Body (all 8-bit): `lda al_rotx,x; clc;
/// adc al_sbyte1,x; sta al_rotx,x` × {rotx/sbyte1, roty/sbyte2, rotz/sbyte3};
/// `rtl` (`B5 12 18 7D 22 00 95 12  B5 13 18 7D 23 00 95 13  B5 14 18 7D 24 00
/// 95 14  6B`). Pure struct-offset — byte-identical retail/built (like
/// `addalvecs_l`); UNIQUE scan hit. Footprint: NO globals, NO RNG; reads
/// `al_rotx/y/z` + `al_sbyte1/2/3`, writes `al_rotx/y/z`.
pub const RETAIL_HARDROT_STRAT: u32 = 0x06_8614;
/// Rotation-angle struct offsets ($12/$13/$14) and the per-axis rate scratch
/// bytes ($22/$23/$24) — from `hardrot_strat`'s operands (identical all carts).
pub const AL_ROTX: u32 = 0x12;
pub const AL_ROTY: u32 = 0x13;
pub const AL_ROTZ: u32 = 0x14;
pub const AL_SBYTE1: u32 = 0x22;
pub const AL_SBYTE2: u32 = 0x23;
pub const AL_SBYTE3: u32 = 0x24;

/// Retail `straight_Istrat` ($0B:8CE1) — a fixed-heading MOVER. The Istrat
/// installs `straight_strat` and computes vx/vy/vz ONCE from `al_roty/al_rotx/
/// al_vel` via `gen_3dvecs` (the GSU), then FALLS THROUGH into `straight_strat`.
pub const RETAIL_STRAIGHT_ISTRAT: u32 = 0x0B_8CE1;
/// Retail `straight_strat` ($0B:8D00) — the per-tick mover body:
/// `jsl addalvecs_l; jsl sr_addplayerZx; rtl`
/// (`22 BB C7 1F 22 69 DC 1F 6B`) = move by fixed velocity (`al_worldx/y/z +=
/// al_vx/vy/vz`) then scroll with the world (`al_worldz += pviewvelz`). Located
/// by scanning the full `straight_Istrat` signature (UNIQUE) then adding the +31
/// fall-through offset; CROSS-VALIDATED because the Istrat's own `s_set_strat`
/// operand equals this derived address ($0B:8D00). Footprint (per tick): reads
/// `al_vx/vy/vz` + `pviewvelz`, writes `al_worldx/y/z`. NO RNG, NO player-
/// relative, NO GSU in the tick (gen_3dvecs runs only in the Istrat).
pub const RETAIL_STRAIGHT_STRAT: u32 = 0x0B_8D00;

// ------------------------------------------------------------------------
// PLAYER-RELATIVE + RNG FRONTIER — the machine-state seeding infrastructure.
//
// Everything above touches at most `pviewvelz`/`pviewposz` (view-scroll
// globals) or pure struct offsets. The next tier of strats reads the PLAYER
// POSITION mirror (`player_posx/y/z`) and/or draws the runtime RNG. Both need
// their retail WRAM state located + seeded so the port and the cart start
// byte-identical. All addresses below were re-derived from the retail cart.
// ------------------------------------------------------------------------

/// Retail `player_posx/y/z` — the player-position MIRROR globals (built
/// PLAYER_POSX/Y/Z = $1598/$159A/$159C). Written each frame by `init_strats_l`
/// from the player object; read by player-relative enemy strats. Located by:
///  * the same -$8B shift the other `dostrats` globals show (built - $8B), and
///  * INDEPENDENTLY: retail has 37/34/25 absolute reads of $150D/$150F/$1511
///    (matching built's 38/32/24 reads of $1598/$159A/$159C), and the leaf
///    `worldz += $1511` ($07:9808) + `parajump_strat`'s own operands
///    (`lda $150F` / `lda $150D`) read these exact addresses. Contiguous words
///    exactly like built. Port ↔ `g.vars.player_posx/y/z`.
pub const RETAIL_PLAYER_POSX: u32 = 0x150D;
pub const RETAIL_PLAYER_POSY: u32 = 0x150F;
pub const RETAIL_PLAYER_POSZ: u32 = 0x1511;
/// Retail `PLAYPT` ($1238) — the player-OBJECT pointer (built PLAYPT $12C3, the
/// -$8B shift). `do_strat_l`/`init_strats` set it to the player block; strats
/// that need the player's live world coords (`parajump`'s Z-distance gate)
/// `ldy PLAYPT; lda al_worldz,y`. Read straight out of `parajump_strat`'s
/// `ldy` operand. Port ↔ slot 0 (`g.objs.player()` = `aliens[0]`).
pub const RETAIL_PLAYPT: u32 = 0x1238;

/// Retail runtime RNG `RANDOM` ($02:FC5C) + its far wrapper `RANDOM_L`
/// ($02:FC58 = `jsr RANDOM; rtl`, 288 `jsl` refs — the heavily-used runtime
/// PRNG). The algorithm is the 4-byte **subtract-with-borrow chain** proven in
/// tests/random.rs (`A=rand0; clc; sbc rand1->rand1; sbc rand2->rand2;
/// sbc rand3->rand3; sbc rand0(orig)->rand0; return A`) — byte-for-byte the
/// same routine as the built ROM's `RANDOM` ($02:F7BF), EXCEPT the state lives
/// at a DIFFERENT zeropage address: retail `rand` = **$EF-$F2**, built = $DE-$E1.
/// Found by a masked scan of the SWB skeleton with the direct-page operands
/// wildcarded (2 hits; the referenced one, jsr-wrapped, is the live PRNG). The
/// port's `sf_strat::common::sf_random` runs the identical algorithm over
/// `g.vars.rng: [u8;4]`, so seeding both with the same 4 bytes keeps the two
/// streams in lockstep.
pub const RETAIL_RANDOM_L: u32 = 0x02_FC58;
pub const RETAIL_RANDOM: u32 = 0x02_FC5C;
/// Retail `rand` zeropage state ($EF-$F2, 4 bytes). NOTE: this OVERLAPS the
/// [`call`] harness's direct-page param block ($F0-$F5), so a stream of RANDOM
/// calls must re-inject the carried state each call (seed $EF directly + encode
/// $F0/$F1/$F2 into the entry A/X regs) — see [`seed_retail_rng`] and
/// tests/coexec_retail.rs::retail_rng_stream_vs_port.
pub const RETAIL_RAND: u32 = 0x00EF;

/// Retail `parajump_strat` ($04:F851 — same address as the built ROM; this ROM
/// region did not shift). The first PLAYER-POSITION-relative strat certified.
/// Body (all integer, no GSU/trig):
///  * `al_worldy = achase_proportional(al_worldy, player_posy, rate 2)`
///    (leaf $1F:D66F),
///  * `ldy PLAYPT; if |player.worldz - al_worldz| < 200`:
///    `al_worldx = achase_proportional(al_worldx, player_posx, rate 3)`
///    (leaf $1F:D6AB).
/// Reads `player_posy`($150F), `player_posx`($150D), `PLAYPT`($1238)→player Z.
/// Port ↔ `sf_strat::enemy_a::parajump_strat` (a direct composition of the
/// public `common::strat_chase_proportional`).
pub const RETAIL_PARAJUMP_STRAT: u32 = 0x04_F851;

/// Retail `firepillar_Istrat` ($0A:DAE4) — the FIRST certified RNG-DRIVEN ENEMY
/// strat, and the end-to-end proof of the `ea_random`->`sf_random` fix (commit
/// f280388). GA2STRAT.ASM:2039-2062. The init draws the runtime RNG THREE times
/// and reads the player-X mirror:
///  * `jsl RANDOM_L` -> `sta al_worldx (low byte)`          (DRAW 1)
///  * `jsl RANDOM_L; and #3 -> sta al_worldx+1 (high byte)` (DRAW 2)
///    => `al_worldx = draw1 | ((draw2 & 3) << 8)` (0..1023)
///  * `sbc #512`, then `lda player_posx; asra (>>1 signed); clc; adc al_worldx`
///  * `jsl RANDOM_L; cmp #$B2 (178 = 70%); bcs -> set al_sflags2 |= $20`  (DRAW 3)
///    => the "inert" latch fires on the 30% (rnd >= 178) branch.
/// Located by masked signature scan of the retail cart (99-byte skeleton read
/// out of the built ROM at $0A:DABE, RANDOM_L/set0coll/player_posx/strat-ptr
/// operands wildcarded): UNIQUE hit. Cross-validated by reading the operands
/// back: all three `jsl` land on RETAIL_RANDOM_L ($02:FC58), `lda` reads
/// RETAIL_PLAYER_POSX ($150D), the coin is `cmp #$B2`, and the `jml` fall-through
/// target = `firepillar_strat` ($0A:DB47 = Istrat + $63). The 5-byte-longer
/// build offset ($0A:DABE) shifts +$26 in retail; the struct offsets, constants,
/// and RNG-draw sequence are byte-identical.
pub const RETAIL_FIREPILLAR_ISTRAT: u32 = 0x0A_DAE4;
/// Retail `firepillar_strat` ($0A:DB47) — the per-tick body the Istrat installs
/// and falls into (read out of the Istrat's `jml` fall-through operand). Not RNG-
/// driven itself; the RNG lives entirely in the Istrat.
pub const RETAIL_FIREPILLAR_STRAT: u32 = 0x0A_DB47;
/// `al_sflags2` bit `$20` (`asf_sflag2`) — firepillar's permanent "inert" latch,
/// set on the 30% coin. Port ↔ `enemies_ground::ASF2_SFLAG2` (also `$20`).
pub const ASF2_SFLAG2: u8 = 0x20;

// ------------------------------------------------------------------------
// BATCH 3 — static-init scenery (`rockhard`) + RNG-driven INIT strats
// (`mine0`, `big_meteor`, `tree1`). All located by masked signature scan of
// the retail cart (skeleton read out of the built ROM via symbols.txt SNES
// addresses, WRAM/jsl operands wildcarded), each a UNIQUE hit, cross-validated
// by reading the RANDOM_L operand back == RETAIL_RANDOM_L ($02:FC58).
// ------------------------------------------------------------------------

/// Retail `rockhard_Istrat` ($06:85D9) — a STATIC indestructible obstacle
/// (GSTRATS.ASM:663-669). Pure struct-offset, ZERO globals, ZERO RNG, so it is
/// byte-identical retail↔built and located by an EXACT scan (UNIQUE hit). Body:
/// `lda al_collflags,x; ora #enemy1($10); sta; lda #deg180($80); sta al_roty,x;
/// lda #hardHP($FF); sta al_HP,x; lda #rockhardAP($14=20); sta al_AP,x;
/// rep #$20; lda #0; sta al_stratptr,x; sep #$20; lda #0; sta al_stratptr+2,x;
/// rtl`. Footprint: writes al_collflags(|=$10), al_roty($80), al_HP, al_AP,
/// al_stratptr(=0, null tick). Port ↔ `enemies_ground::rockhard_istrat`
/// (IS_ROCKHARD=192).
pub const RETAIL_ROCKHARD_ISTRAT: u32 = 0x06_85D9;

/// Retail `mine0_Istrat` ($09:9117) — a static destructible mine
/// (DSTRATS.ASM:1572-1577). Draws the runtime RNG ONCE for a random orientation:
/// `... jsl set_coll; lda #2; sta al_HP; lda #$0A(10); sta al_AP;
/// lda al_collflags,x; ora #enemy1($10); sta; jsl RANDOM_L; sta al_rotz,x; rtl`.
/// The single `jsl RANDOM_L` (cross-validated == $02:FC58) yields the FULL-byte
/// `al_rotz` (no mask). Footprint: 1 RNG draw → al_rotz; writes HP=2/AP=10/
/// enemy1. Port ↔ `enemies_ground::mine0_init` registered at this exact
/// non-table strategy address.
pub const RETAIL_MINE0_ISTRAT: u32 = 0x09_9117;

/// Retail `big_meteor_Istrat` ($00:FA62) — an indestructible spinning obstacle
/// (D3STRATS.ASM:1069-1077). Draws the runtime RNG ONCE for a (cosmetically
/// unused) spin datum: `... set nohitaffect; HP=$FF; AP=$0C(12); <s_rots_flat:
/// lda $1849/$1547-view-vecs -> al_roty/al_rotx, cosmetic>; jsl RANDOM_L;
/// and #$0F; sta al_sbyte1,x; lda al_sbyte1,x; sec; sbc #8; sta al_sbyte1,x;
/// rtl`. The single `jsl RANDOM_L` (== $02:FC58) yields `al_sbyte1 = (rnd&15)-8`.
/// Footprint: 1 RNG draw → al_sbyte1; writes HP=$FF/AP=12/nohitaffect (+ cosmetic
/// rotx/roty from view vectors, scoped out of the port). Port ↔
/// `enemies_ground::big_meteor_init` (IS_BIG_METEOR=234).
pub const RETAIL_BIG_METEOR_ISTRAT: u32 = 0x00_FA62;

/// Retail `tree1_Istrat` ($09:95EE) — indestructible sprouting-tree scenery
/// (DSTRATS.ASM:2016-2043). Head sets two sflag bits then draws the runtime RNG
/// ONCE for the tree height: `lda al_sflags,x; ora #2; sta; lda al_sflags2,x;
/// ora #$80; sta; jsl RANDOM_L; and #3; sta al_sbyte1,x; inc al_sbyte1,x; ...`.
/// The single `jsl RANDOM_L` (== $02:FC58) yields `al_sbyte1 = (rnd&3)+1`.
/// Footprint (RNG part): 1 RNG draw → al_sbyte1. Port ↔
/// `enemies_ground::tree1_init` (IS_TREE1=204).
pub const RETAIL_TREE1_ISTRAT: u32 = 0x09_95EE;

/// `al_HP` / `al_AP` / `al_collflags` struct offsets (identical all carts;
/// verified against `mine0`/`rockhard` `sta al_HP($2A)/al_AP($2B)` +
/// `lda al_collflags($2E)` operands, and `do_strat_l`'s `al_HP,x`).
pub const AL_HP: u32 = 0x2A;
pub const AL_AP: u32 = 0x2B;
pub const AL_COLLFLAGS: u32 = 0x2E;

// ------------------------------------------------------------------------
// BATCH 4 — a zdist state-transition MOVER (`woods`), an RNG + PLAYER-RELATIVE
// scenery init (`tree2`), an RNG reroll firing-enemy init (`shou0`), and the
// `break_meteorT` tadpole death coin. All located by masked signature scan of
// the retail cart (skeleton read out of the built ROM via symbols.txt, WRAM/jsl
// operands wildcarded), each a UNIQUE hit, cross-validated.
// ------------------------------------------------------------------------

/// Retail `woods_strat` ($08:B7F6, GASTRATS.ASM:1386-1390) — a Zenemy obstacle
/// that waits inert until the player closes within `2100` z, then `jml`s into
/// `woodsgo_init` which converts it into a homing missile. Body:
/// `ldy PLAYPT; rep #$20; lda al_worldz,y; sec; sbc al_worldz,x; bpl+; eor
/// #$FFFF; inc a; cmp #$0834(2100); sep #$20; bpl .stay; jml woodsgo_init; rtl`.
/// Footprint: reads `PLAYPT`->player `al_worldz` + own `al_worldz`; on convert
/// jml's to `woodsgo_init`. Port <-> `enemies_ground::woods_strat` (IS_WOODS=54).
pub const RETAIL_WOODS_STRAT: u32 = 0x08_B7F6;
/// Retail `woodsgo_init` ($08:B813) — the conversion body woods_strat jml's to:
/// installs `al_stratptr = woodsgo_strat ($08:B840)`, sets the extended coll/exp
/// strat ptrs (`$7E:1CD0/1CD2,x`), a leaf `jsl $06:EEEE`, then `al_sbyte1 = 10`
/// (home timer) + snd2 (`$7E:1CE9,x = 2`). Read straight out of woods_strat's
/// `jml` operand.
pub const RETAIL_WOODSGO_INIT: u32 = 0x08_B813;
/// Retail `woodsgo_strat` ($08:B840) — the homing-missile tick woodsgo_init
/// installs (read out of woodsgo_init's `sta al_stratptr` immediate).
pub const RETAIL_WOODSGO_STRAT: u32 = 0x08_B840;
/// The woods conversion Z-distance gate (`s_jmp_Zdistless #2100`).
pub const RETAIL_WOODS_ZGATE: i16 = 2100;

/// Retail `tree2_Istrat` ($09:952F, DSTRATS.ASM:1976-2014) — a destructible
/// tree that (a) draws the runtime RNG ONCE for its height (`al_sbyte1 =
/// (rnd&3)+1`) and (b) tilts toward the player: reads `PLAYPT`->player
/// `al_worldx`, compares it to its own `al_worldx`, and on `enemy_x < player_x`
/// (.otherway) negates `al_sbyte2` (=-deg22) and `al_roty += deg45($20)`, else
/// (.notthatway) `al_roty += -deg45($E0)`. `al_sbyte2` starts = `deg22($10)`.
/// The single `jsl RANDOM_L` (== $02:FC58) is the first instruction. Port <->
/// `enemies_ground::tree2_init` (IS_TREE2=205).
pub const RETAIL_TREE2_ISTRAT: u32 = 0x09_952F;

/// Retail `shou0_Istrat` ($0A:D615, GA2STRAT.ASM:1853-1859) — a rotating plasma
/// turret. Wires strats/data (HP2/AP12/enemy1), then draws the runtime RNG for
/// its fire-pattern selector `al_sbyte1 = rnd&3`, REROLLING while `sbyte1 == 3`
/// (`jml .again` back to the draw) so the result is uniform in {0,1,2}. Falls
/// through into `shou0_strat` ($0A:D646), whose zdist range gate (`[500,2500)`)
/// makes it a clean no-op when the player is far. Port <->
/// `enemies_ground::shou0_init` (IS_SHOU0=178).
pub const RETAIL_SHOU0_ISTRAT: u32 = 0x0A_D615;
/// Retail `shou0_strat` ($0A:D646) — shou0's per-tick body (fall-through target).
pub const RETAIL_SHOU0_STRAT: u32 = 0x0A_D646;

// ------------------------------------------------------------------------
// AIMING CLASS — the GSU-per-tick aiming pipeline (every enemy that aims at
// the player + fires). The aim ANGLE the strat stores each tick is
// `arctan16(dx,dz) >> 8`, computed by the CPU routine `anglexy_l` which copies
// dx/dz into GSU RAM and KICKS the Super-FX chip (`arctan16 -> runmario_l ->
// mcallarctan16`). This is a real GSU call executing INSIDE a strat's aim step
// — the hardest tier-2 frontier. gen_3dvecs (velocity from the angle) is, by
// contrast, pure CPU (sin/cos tables), so the GSU-in-the-tick is arctan alone.
// ------------------------------------------------------------------------

/// Retail `anglexy_l` / `Yanglexy_l` — the yaw-aim leaf every firing enemy calls
/// via `s_obj2obj_angle obj1,obj2,al_roty,#chase`. Given X=obj1 (aimer/src),
/// Y=obj2 (target/dst) it computes `x1 = worldx[dst]-worldx[src]`,
/// `y1 = worldz[dst]-worldz[src]`, then `jsl arctan16_l` (which drives the GSU)
/// and returns the 16-bit angle in A (0..$FFFF = 0..360deg). The strat macro
/// then takes `>>8` (`xba`) + `nega` as the roty chase target.
/// Body (built $1F:D039, 29 bytes): `phx; phy; rep #$20; lda al_worldx,y; sec;
/// sbc al_worldx,x; sta x1; lda al_worldz,y; sec; sbc al_worldz,x; sta y1;
/// jsl arctan16_l; rep #$30; ply; plx; rtl`
/// (`DA 5A C2 20 B9 0C 00 38 F5 0C 85 <x1> B9 10 00 38 F5 10 85 <y1>
/// 22 <arctan16_l> C2 30 7A FA 6B`). Located by masked scan (the two WRAM
/// scratch operands `x1`/`y1` + the `jsl arctan16_l` target wildcarded), a
/// UNIQUE hit; cross-validated by reading the `jsl` operand back == the
/// derived retail `arctan16_l` ($02:FCF1). The two scratch words land at the
/// SAME direct-page addresses as the built ROM (x1=dp$02, y1=dp$08).
/// Port <-> `common::strat_angle_xz` (== angle_xz).
pub const RETAIL_ANGLEXY_L: u32 = 0x1F_D021;
/// Retail `arctan16_l` ($02:FCF1) — read straight out of `anglexy_l`'s `jsl`
/// operand (built ROM's is $02:F854; the routine shifted +$49D in retail).
pub const RETAIL_ARCTAN16_L: u32 = 0x02_FCF1;
/// Retail `arctan16_l` — the far wrapper `jsr arctan16; a16; rtl` that
/// `anglexy_l` calls; `arctan16` copies `x1/y1` into GSU RAM `m_x1`($62)/
/// `m_y1`($2C), does `lda #mcallarctan16>>16; ldx #mcallarctan16&$ffff;
/// jsl runmario_l` (the RAM GSU trampoline), then reads back `m_cnt`($40).
/// Read straight out of `anglexy_l`'s `jsl` operand (built $02:F854). Not
/// called directly by the harness — `anglexy_l` invokes it internally, so the
/// whole GSU roundtrip runs from one `call(anglexy_l)`.
pub const RETAIL_ARCTAN16_L_BUILT: u32 = 0x02_F854;

/// Retail `n3dvecs_l` ($1F:C41E) — the CPU aim-math step that turns an aim
/// angle into a velocity (`s_gen_3dvecs`; pure sin/cos tables, NO GSU). Reads
/// `troty`/`trotx` (angle bytes) + `tmpz` (magnitude), writes the velocity into
/// the `x1/y1/z1` WRAM scratch. Located by masked scan (the `nega roty; tax
/// rotx; sep #$10` skeleton with all scratch/table operands wildcarded); the
/// retail scratch block SHIFTED (x1/y1 stayed $02/$08 but z1 $8A->$90, tmpz
/// $78->$7E, troty/trotx $1631/$1630->$15A7/$15A6). Port <-> `common::
/// strat_gen_vecs_3d` (vx/vz + |vy| bit-exact; vy sign = renderer convention).
pub const RETAIL_N3DVECS_L: u32 = 0x1F_C41E;
/// Retail `troty`/`trotx` — the `n3dvecs_l` angle inputs (built $1631/$1630).
pub const RETAIL_TROTY: u32 = 0x15A7;
pub const RETAIL_TROTX: u32 = 0x15A6;
/// Retail `alvelvecs_l` ($1F:C09F, STRATROU.ASM:100) — 2D XZ velocity from
/// `al_roty`/`al_vel` (no yaw nega; `vy` zeroed). Built $1F:C0B7 (−$18). Scratch
/// `tmpz`=$7E (same block as `n3dvecs_l`). Port ↔ `strat_gen_vecs_2d`.
pub const RETAIL_ALVELVECS_L: u32 = 0x1F_C09F;
/// Retail `nvecs_l` ($1F:C177, STRATROU.ASM:162) — `s_gen_vecs` XZ from angle in
/// A + magnitude in `tmpz`; table index `−angle+1`. Built $1F:C18F (−$18). Does
/// not write `vy`. Port ↔ `strat_nvecs`.
pub const RETAIL_NVECS_L: u32 = 0x1F_C177;
/// Retail `tmpz` for `alvelvecs_l`/`nvecs_l`/`n3dvecs_l` (built $78 → $7E).
pub const RETAIL_TMPZ: u32 = 0x7E;
/// Retail `z1` velocity scratch (built $8A → $90); `x1`/`y1` stay $02/$08.
pub const RETAIL_Z1: u32 = 0x90;
/// Retail `perc56A_l`…`perc93A_l` (STRATROU.ASM:2494) — signed ASR percentage
/// scales (`tpx`=$3A / `tpy`=$3C / `tpa`=$14C5). Built block at $1F:D4AA (−$18).
/// Port ↔ `strat_perc56`…`strat_perc93`.
pub const RETAIL_PERC56A_L: u32 = 0x1F_D492;
pub const RETAIL_PERC62A_L: u32 = 0x1F_D4A8;
pub const RETAIL_PERC75A_L: u32 = 0x1F_D4BA;
pub const RETAIL_PERC87A_L: u32 = 0x1F_D4C8;
pub const RETAIL_PERC93A_L: u32 = 0x1F_D4DF;
/// The fire-gate timing (`s_jmp_notdelay #delay,label,al1pt`,
/// GASTRATS.ASM:1310) — the pure-integer per-frame fire timer every firing
/// enemy uses: `lda gameframe; clc; adc al1pt; and #(1<<delay)-1; bne .skip`,
/// i.e. FIRE this frame iff `(gameframe + stagger) & ((1<<delay)-1) == 0`. NO
/// GSU, NO RNG — a closed-form decision. Retail has 52 staggered sites (masks
/// {$00,$01,$03,$07,$0F,$1F}); port <-> the identical expression
/// `gameframe.wrapping_add(idx) & mask == 0` (`bossb::notdelay_stag`,
/// `enemy_b.rs:1030`, etc.). See tests/coexec_retail.rs::retail_fire_gate_*.

// ------------------------------------------------------------------------
// PROJECTILE-SPAWN + TARGET-SEARCH machinery — the last piece of the firing
// pipeline (aim + fire-gate already certified in UPDATE 8). A firing enemy's
// fire step is `s_find_nearobj` (walk the active list for the nearest matching
// target) then `s_fire_weapon` -> `fire_weapon_l` (weapon-table dispatch) ->
// per-weapon `fire_X` = `sr_make_obj` (alloc + init + shape) + field sets +
// `gen_weapon` (position the shot at firer + a ROTATED muzzle offset, set its
// rots/speed). All addresses located by masked signature scan of the retail
// cart (skeleton read out of the built ROM via symbols.txt, WRAM/jsl operands
// wildcarded), each a UNIQUE hit, cross-validated by reading operands back.
// ------------------------------------------------------------------------

/// Retail `find_nearobject_l` ($1F:C870, STRATROU.ASM:697) — the `s_find_nearobj`
/// target search. Given X=self, A=shape, `tpz`=min radius, `tpx`=max radius, and
/// `fobj`=active-list head, it walks the `_next` chain and returns Y = the block
/// with matching `al_shape` whose `xzdiffs` **rangexz** is smallest within the
/// `[tpz,tpx)` band (0 = none). Body is byte-identical to the built ROM
/// ($1F:C888) except the WRAM operands (`fobj`,`rangexz`) and the `jsl xzdiffs_l`
/// target — the DP scratch (`x2`=$04,`y2`=$0A,`tpx`=$3A,`tpy`=$3C,`tpz`=$3E) is
/// unchanged. Uses `xzdiffs_l` -> **rangexz** which is an XZ-plane octagonal
/// distance approximation that IGNORES Y. Port <-> `enemy_a::strat_find_near_shape`
/// (which instead uses a 3D box gate + Manhattan `dx+dy+dz` metric — see the cert
/// test for the certified agreement region + the Y-plane divergence).
pub const RETAIL_FIND_NEAROBJECT_L: u32 = 0x1F_C870;
/// Retail `xzdiffs_l` ($1F:D0AB, STRATROU.ASM:1796) — computes `rangexz`, an
/// XZ-plane octagonal-norm distance between objects X and Y (Y coordinate
/// ignored). Read straight out of `find_nearobject_l`'s `jsl` operand.
pub const RETAIL_XZDIFFS_L: u32 = 0x1F_D0AB;
/// Retail `fobj` ($14CA) — the search-list head `find_nearobject_l` walks (built
/// $1555). Read from `find_nearobject_l`'s `ldx fobj` operand.
pub const RETAIL_FOBJ: u32 = 0x14CA;
/// Retail `rangexz` ($1250) — `xzdiffs_l`'s output distance (built $12DB). Read
/// from `find_nearobject_l`'s `lda rangexz` operand.
pub const RETAIL_RANGEXZ: u32 = 0x1250;
/// DP scratch for `find_nearobject_l` (identical retail/built; below the `call`
/// param block $F0-$F5, so surgically seedable): `tpx`=max radius / running best,
/// `tpz`=min radius, `tpy`=best block (return).
pub const RETAIL_TPX: u32 = 0x3A;
pub const RETAIL_TPY: u32 = 0x3C;
pub const RETAIL_TPZ: u32 = 0x3E;

/// Retail `fire_weapon_l` ($1F:D146, STRATROU.ASM:2084) — the `s_fire_weapon`
/// dispatch: honours `stratflags & sf_nofiring` ($14D2 bit 0), else `amul3` the
/// weapon id and RTL-dispatches through the `weapons_data` table ($1F:D17A) to
/// the per-weapon `fire_X` spawn routine (the same RTL-trampoline trick as
/// `do_strat_l`). Located by masked scan (UNIQUE); its `weapons_data+4` operand
/// = $1F:D17E cross-validates the table base.
pub const RETAIL_FIRE_WEAPON_L: u32 = 0x1F_D146;
/// Retail `weapons_data` table base ($1F:D17A) — per-weapon 6-byte records
/// (`chr, fire_X ptr lo/hi, bank`).
pub const RETAIL_WEAPONS_DATA: u32 = 0x1F_D17A;
/// Retail `sr_make_obj` ($1F:D54B, STRATROU.ASM:2568) — the `s_make_obj` alloc:
/// `jsl makeobj_l` (pop the free list), on success `jsl init_objvars_l` (clear
/// the block + set default flags) then `al_shape = tpa`. Located by masked scan
/// (UNIQUE); its `jsl` operands cross-validate `makeobj_l` + `init_objvars_l`.
pub const RETAIL_SR_MAKE_OBJ: u32 = 0x1F_D54B;
/// Retail `makeobj_l` ($1F:D3A9, STRATROU.ASM:2354) — the pool allocator: pops
/// `alfreelst` ($121F == [`RETAIL_POOL`]`.freelist_head`), links the block onto
/// `allst` ($121D == [`RETAIL_POOL`]`.active_head`), returns X=block / carry set
/// (carry clear = pool full). Located by masked scan (UNIQUE) AND as
/// `sr_make_obj`'s first `jsl` operand — cross-validated twice; its `ldx $121F`
/// / `lda $121D` operands match [`RETAIL_POOL`] independently.
pub const RETAIL_MAKEOBJ_L: u32 = 0x1F_D3A9;
/// Retail `init_objvars_l` ($1F:D36E, STRATROU.ASM:2xxx) — zero the alien block
/// (main + extended arrays) and set the default sflags. `sr_make_obj`'s 2nd `jsl`.
pub const RETAIL_INIT_OBJVARS_L: u32 = 0x1F_D36E;
/// Retail `tpa` ($14C5) — the `sr_make_obj` shape scratch (`s_make_obj` writes
/// the shape here; `sr_make_obj` reads it into `al_shape,y`). Read from
/// `sr_make_obj`'s `lda tpa; sta al_shape,y` operand (built $1550).
pub const RETAIL_TPA: u32 = 0x14C5;

/// Retail `gen_weapon` muzzle-placement rotation primitives — the rotated muzzle
/// offset (`s_add_Roffs2pos B,shot,firer,firer, weapx,weapy,weapz, 1,1,1,
/// weapon_scale`) rotates the offset by the firer's FULL rotation in ROM order
/// rotz(`rotate_8yx_l`) -> rotx(`rotate_8yz_l`) -> roty(`rotate_8xz_l`), then
/// `<< weapon_scale` (=2), then adds the firer's world position. All three are
/// CPU sin/cos routines (NO GSU) — the SAME sin/cos rotation primitive certified
/// bit-exact vs retail as `n3dvecs_l`/`arctan16` (UPDATE 8). Located by masked
/// scan (each UNIQUE); costab/sintab = $00:98A5/$98E5-region, cy/sy = $15F3/$15F4.
/// Port muzzle <-> `enemy_a::boss1_rot_offset_pos` (same rotz->rotx->roty order,
/// same `strat_sin`/`strat_cos` used by the certified `gen_vecs_3d`).
pub const RETAIL_ROTATE_8YX_L: u32 = 0x1F_CC78;
pub const RETAIL_ROTATE_8YZ_L: u32 = 0x1F_CAFB;
pub const RETAIL_ROTATE_8XZ_L: u32 = 0x1F_C97B;

// ------------------------------------------------------------------------
// COLLISION SYSTEM — the highest-blast-radius shared surface (every laser
// hit, ship/enemy contact, pickup). Three pieces:
//   * do_coll_l           — the collision RESPONSE (hp decrement + framesperAP
//                           cooldown + in-tunnel hardAP halving + hp bit7
//                           indestructible). ROM-resident ($1F bank), so it is
//                           RUN surgically vs the port.
//   * COLDET macro        — the object-vs-object box-overlap TEST (16-bit
//                           |d|<sum on Z,X,Y). Inlined in `chkcoll`, which is a
//                           RAM-resident routine (SNES $7E:5015; symbol map),
//                           so it can NOT be JSL'd on a non-booted bus. Located
//                           in its ROM COPY-SOURCE (bank $02) and certified
//                           structurally + by grid-diffing the port `aabb_overlap`
//                           against a byte-faithful transcription of this ASM.
//   * chkcoll0 colltype   — the collision ALLOW-MATRIX (who may hit whom).
//                           Also inside the RAM `chkcoll`; located in the ROM
//                           copy-source and certified by matrix-diff.
// All located by masked signature scan of the retail cart, cross-validated by
// reading operands back out. See tests/coexec_retail.rs.
// ------------------------------------------------------------------------

/// Retail `do_coll_l` ($1F:D23A, STRATROU.ASM:2143) — the collision RESPONSE
/// (`JSL`/`RTL`), applied to a victim block X with damage in `x1` (dp $02).
/// Body (8-bit A):
/// `DEC al_collcount,x; BEQ +; JML .skip;      (DEC-then-BNE: damage only when
///                                              collcount reaches 0)
///  LDA pshipflags3; AND #psf3_intunnel; BNE +; JML .ntun;
///  LDA x1; CMP #hardAP; BNE .nhard; CMP #$80; ROR A; STA x1;   (asra: halve
///                                              hardAP damage in a tunnel)
///  .nhard/.ntun: LDA al_HP,x; BMI .o2c;        (hp bit7 set => indestructible)
///  SEC; SBC x1; BPL .nnhc; LDA #0; .nnhc STA al_HP,x;   (clamp at 0)
///  .o2c: LDA tpa; STA al_collcount,x; .skip: RTL`   (reload cooldown = tpa)
/// Located by UNIQUE masked scan (`D6 2D F0 04 5C.. AD..(pshipflags3) 29 01
/// D0 04 5C.. A5 02 C9 08 D0 05 C9 80 6A 85 02 B5 2A 30 09 38 E5 02 10 02 A9 00
/// 95 2A AD..(tpa) 95 2D`). Cross-validated: the `AD` operands read back give
/// `pshipflags3`=$14D8 and `tpa`=$14C5 (== [`RETAIL_TPA`]); struct offsets
/// collcount=$2D, HP=$2A (== [`AL_HP`]) match. Port <-> `Game::do_coll`
/// (coldet.rs:236). RUN surgically vs the port — the response cert.
pub const RETAIL_DO_COLL_L: u32 = 0x1F_D23A;
/// Retail `pshipflags3` ($14D8) — read out of `do_coll_l`'s `LDA pshipflags3`
/// operand (built $1563; the −$8B dostrats-globals shift). Bit $01 = in-tunnel.
/// Port <-> `g.vars.pshipflags3`, `PSF3_INTUNNEL` = $01.
pub const RETAIL_PSHIPFLAGS3: u32 = 0x14D8;
/// `al_collcount` struct offset ($2D) — the do_coll cooldown counter (identical
/// retail/built/port; from `do_coll_l`'s `DEC al_collcount,x` operand).
pub const AL_COLLCOUNT: u32 = 0x2D;
/// `hardAP` (8) / `framesperAP` (10) collision constants (STRATEQU.INC:66/798;
/// identical retail/built/port — port `HARD_AP`/`FRAMESPERAP`).
pub const HARD_AP: u8 = 8;
pub const FRAMESPERAP: u8 = 10;

/// Retail box-overlap `COLDET` macro expansion (SNES $02:A1BF) — the
/// object-vs-object AABB test, in the ROM copy-source of the RAM-resident
/// `chkcoll` (SNES $7E:5015). Three consecutive 16-bit axis tests in Z, X, Y
/// order, each: `lda cl_Nmax,x; clc; adc Ncol; sta rangexz; lda tp<N>;
/// sec; sbc <N>p; bpl+4; eor #$FFFF; inc a;  (16-bit two's-complement abs)
/// sec; sbc rangexz; bmi .in; jmp .notcollided`. Overlap iff on ALL three axes
/// `|pos2 - pos1| < (cl_Nmax_1 + cl_Nmax_2)` — a STRICTLY-LESS boundary. The
/// summed half-extents `cl_Nmax` come from the per-SHAPE size table
/// (`generate_collist_l` copies `sh_xmax/ymax/zmax`), NOT a per-object size.
/// `rangexz`=$1250 (== [`RETAIL_RANGEXZ`]); position DP scratch tpz/tpx/tpy
/// =$3E/$3A/$3C, zp/xp/ys=$6E/$6C/$74. Byte-structurally IDENTICAL to the port
/// `aabb_overlap` (coldet.rs:166): same 16-bit width, same Z/X/Y order, same
/// two's-complement abs, same strictly-less boundary. Certified structurally +
/// by grid-diff (the RAM residency of `chkcoll` blocks a live JSL).
pub const RETAIL_COLDET_OVERLAP: u32 = 0x02_A1BF;

/// Retail `chkcoll0` colltype ALLOW-MATRIX filter (SNES $02:A159) — the
/// who-may-hit-whom gate, in `chkcoll`'s ROM copy-source. Body:
/// `lda al_collflags,y; and al_collflags,x; and #colltype_mask; beq +;
///  brl chkcollnxt` — i.e. a pair is SKIPPED iff it shares ANY collision-type
/// bit (`cf_a & cf_b & $F8 != 0`). NO "both zero => skip" (an object with no
/// type bit still collides). Cross-validated: the `and #imm` operand = $00F8 =
/// `colltype1|2|3|4|5`. Followed immediately by the immunity checks
/// (`cmp al_immuneptr,x`, NO nonzero guard; immuneptr=$19) and the same-shape
/// gate (see [`RETAIL_CURRSHAPE`]). Port <-> `Game::coldet_run` colltype filter
/// (coldet.rs:310 — `if a_types & b_types != 0 continue`, TYPE_MASK=$F8). MATCH.
pub const RETAIL_CHKCOLL_COLLTYPE: u32 = 0x02_A159;
/// `colltype1|colltype2|colltype3|colltype4|colltype5` mask ($F8) — the retail
/// allow-matrix typemask, read out of the `chkcoll0` `and #$00F8` operand.
/// Semantics (STRATEQU.INC:943-954): colltype1=lasers, 2=enemy1, 3=enemy2,
/// 4=enemy-weapons, 5=friend. Identical to the port `TYPE_MASK`.
pub const COLLTYPE_MASK: u8 = 0xF8;
/// `al_immuneptr` struct offset ($19) — from `chkcoll0`'s `cmp al_immuneptr,x`.
pub const AL_IMMUNEPTR: u32 = 0x19;
/// `al_sflags3` struct offset ($1F) + the `sameshapecollide` bit ($80). The
/// retail same-shape gate (`chkcoll0`, right after immunity): unless BOTH
/// objects carry `sflags3 & $80`, a pair whose `al_shape` == `currshape`
/// ($1F03) is SKIPPED (`lda al_shape,x; cmp currshape; beq -> brl chkcollnxt`).
/// The port `coldet_run` has NO shape gate — a REAL (narrow-blast-radius)
/// divergence characterized in `retail_same_shape_skip_divergence`.
pub const AL_SFLAGS3: u32 = 0x1F;
pub const ASF3_SAMESHAPECOLLIDE: u8 = 0x80;
/// Retail `currshape` ($1F03) — the primary object's shape, cached at the top of
/// `chkcoll`, that the same-shape gate compares each candidate against.
pub const RETAIL_CURRSHAPE: u32 = 0x1F03;

// ------------------------------------------------------------------------
// PLAYER MOVEMENT — the per-frame physics the ship runs every frame it is
// alive: steering -> rotation accumulators -> velocity, position integration,
// the screen-edge BOUNDS clamp, and the boost/brake speed ramp. The two
// runnable cores located here are the pieces that are pure/ROM-resident and so
// can be RUN surgically vs the port:
//   * playerlimitx_srou — the screen-edge X bounds clamp (+ edge arrows). The
//     "known concern": inclusive vs exclusive boundary + which arrow fires.
//   * sr_speedto        — the boost/brake speed ramp (`al_vel` -> tospeed).
// The rest of the pipeline is certified elsewhere: the steering->velocity map
// is `gen_3dvecs`/`n3dvecs_l` (UPDATE 8) and the position integrator is
// `addalvecs_l` (UPDATE 1), both already MATCH vs retail. Located by masked
// signature scan of the retail cart, cross-validated by reading operands back.
// ------------------------------------------------------------------------

/// Retail `playerlimitx_srou` ($0B:DF21, PSTRATS.ASM:2819-2829) — the player's
/// screen-edge X clamp, run every frame after `add_vecs2pos`. Body (8-bit A in,
/// REP #$20 for each 16-bit compare):
/// `lda arrows; and #$F3; sta arrows;                 (clear left|right arrows)
///  rep; lda al_worldX,x; cmp minpmoveX; sep;
///  beq .clmpMin; bmi .clmpMin; jml .nminX;           (clamp iff worldX <= min)
///  .clmpMin: rep; lda minpmoveX; sta al_worldX,x; sep; lda arrows; ora #$04; sta;
///  .nminX: rep; lda al_worldX,x; cmp maxpmoveX; sep;
///  bpl .clmpMax; jml .nmaxX;                          (clamp iff worldX >= max)
///  .clmpMax: rep; lda maxpmoveX; sta al_worldX,x; sep; lda arrows; ora #$08; sta;
///  .nmaxX: rts`. BOTH boundaries are INCLUSIVE: the min side clamps on `<=`
/// (BEQ+BMI), the max side on `>=` (BPL after CMP). Located by masked scan
/// (UNIQUE) with the arrows/min/max operands wildcarded; the AND #$F3, the
/// F0/30/5C boundary triple, and the ORA #$04/#$08 arrow sets pin it. Port <->
/// `player::playerlimit_x_srou` (X portion). NOTE: the port ALSO clamps Y
/// (miny/maxy) in the same fn — that is an HD-runtime addition NOT present in
/// the ROM `playerlimitx_srou`, so only the X clamp + LEFT/RIGHT arrows are
/// certified here.
pub const RETAIL_PLAYERLIMITX_SROU: u32 = 0x0B_DF21;
/// Retail `arrows` ($1FC7, built $1ACB) — the edge-arrow HUD bitfield, read out
/// of `playerlimitx_srou`'s `lda/sta arrows` operands. Bits: up=1, down=2,
/// left=4, right=8.
pub const RETAIL_ARROWS: u32 = 0x1FC7;
/// Retail `minpmoveX`/`maxpmoveX` ($156F/$1571, built $15F9/$15FB) — the
/// player's X movement box, read out of `playerlimitx_srou`'s `cmp`/`lda`
/// operands (contiguous words, identical +2 spacing as built). Port <->
/// `g.vars` MINPMOVEX/MAXPMOVEX slots.
pub const RETAIL_MINPMOVEX: u32 = 0x156F;
pub const RETAIL_MAXPMOVEX: u32 = 0x1571;
/// `sprar_left` (4) / `sprar_right` (8) — the edge arrows `playerlimitx_srou`
/// sets, from its `ora #imm` operands. Port <-> `player::SPRAR_LEFT/RIGHT`.
pub const SPRAR_LEFT: u8 = 0x04;
pub const SPRAR_RIGHT: u8 = 0x08;

/// Retail `sr_speedto` ($1F:D60D, STRATROU.ASM:2707-2733) — the speed ramp
/// (`JSL`/`RTL`, 8-bit A). Given A=rate on entry, `tpa`=target speed, X=object:
/// `sta tpx(=rate); lda al_vel,x; sec; sbc tpa; beq .nsc(->sec;rtl);
///  bpl+; nega;                                        (|vel - target|)
///  cmp tpx; bpl .sc;                                  (|diff| >= rate -> step)
///  lda tpa; bra .fs;                                  (|diff| < rate -> SNAP)
///  .sc: lda al_vel,x; cmp tpa; <Fchase_A tpx>;        (step by rate, no overshoot)
///  .fs: sta al_vel,x; clc; rtl`. The snap-when-near guard is why the step
/// never over/undershoots (the port's overflow fix mirrors this). Located by
/// masked scan (UNIQUE); its three `tpa` operands all read back == [`RETAIL_TPA`]
/// ($14C5) — the SAME `tpa` scratch as `do_coll`/`sr_make_obj`, an independent
/// cross-validation. `al_vel` struct offset = $15; `tpx` (rate) = dp $3A.
/// Port <-> `common::strat_speed_to`. This is the boost/brake ramp: `viewmove_srou`
/// calls it as `strat_speed_to(al, player_tospeed, 2)` each frame (boost sets
/// tospeed=MAX_PSPEED=85, brake sets MIN_PSPEED=20).
pub const RETAIL_SR_SPEEDTO: u32 = 0x1F_D60D;
/// `al_vel` struct offset ($15) — the ship's scalar forward speed the boost/
/// brake ramp drives (identical retail/built/port; from `sr_speedto`'s
/// `lda al_vel,x` operand). Distinct from `al_vx/vy/vz` ($2F/$31/$33), the
/// 3D velocity components `gen_3dvecs` derives from it + the rotations.
pub const AL_VEL: u32 = 0x15;

// ------------------------------------------------------------------------
// BOSS8 — the "washing machine" wash boss (GB3STRAT.ASM:42-204, Sector Z /
// Venom). The largest remaining behavioral-coverage gap: a multi-phase BOSS
// with a child family. The pieces certifiable WITHOUT the GSU are:
//   * boss8_Istrat  — INIT: HP/AP (level-gated), bossmaxHP, colltype, sbyte4
//                     timer, cleared sflags, gsvar_byte1=0, stratptr=boss8wait.
//   * boss8_cont    — the COMMON per-tick body every phase (wait/a/b) converges
//                     to: worldz = 1680 + player_posz (view-track), an sbyte4
//                     countdown 150->0 that toggles sflag1 + reloads 150, and a
//                     gsvar_byte1 speed accumulator that ramps +/-1 toward +/-5
//                     (gated on gameframe&7, direction from sflag1). Pure CPU:
//                     NO GSU, NO RNG — reads player_posz + gameframe + the
//                     gsvar_byte1 global; ends with s_add_bossHP (a bank-$70
//                     accumulator add, harmless to the object diff).
// The phase-transition machine (boss8wait/boss8a/boss8b) is gated on the beam
// CHILDREN's sflag1 and is documented as the remaining gap. All addresses
// located by masked signature scan of the retail cart (skeleton read out of the
// built ROM via symbols.txt, WRAM/jml operands wildcarded), cross-validated by
// reading the operands back.
// ------------------------------------------------------------------------

/// Retail `boss8_Istrat` ($07:919C, GB3STRAT.ASM:42) — the boss8 shell INIT.
/// `lda #boss8HP($20); sta al_HP; lda #hardAP($08); sta al_AP;
///  <s_set_bossmaxHP $20 -> $70:019A>; lda currentlevel; cmp #0; bne .easy;
///  <2x branch: HP=$40, bossmaxHP=$40>; .easy: rep; lda #boss8wait_strat($9359);
///  sta al_stratptr; sep; lda #$07; sta al_stratptr+2; <make 4 children>;
///  set colltype enemy2|enemyweap; al_sbyte4=150; clr sflag1|sflag2;
///  gsvar_byte1=0; init_anim; brl boss8_cont`. UNIQUE masked hit (+$0C shift
/// from the built $079190). Port <-> `bosses::strat_boss8_init` (IS_BOSS8=84).
pub const RETAIL_BOSS8_ISTRAT: u32 = 0x07_919C;
/// Retail `boss8wait_strat` ($07:9359) — the phase-wait tick the Istrat installs
/// (read straight out of `boss8_Istrat`'s `s_set_strat` immediate = $07:9359).
/// Routes to `boss8_cont` (beam child sflag1 clear) or `boss8a_init` (all beams'
/// sflag1 set / gone). The child-gated phase machine is the documented gap.
pub const RETAIL_BOSS8WAIT_STRAT: u32 = 0x07_9359;
/// Retail `boss8_cont` ($07:93BB, GB3STRAT.ASM:108) — the COMMON per-tick body.
/// `rep; lda #$0690(210<<3); sta al_worldz; sep; rep; lda al_worldz; clc;
///  adc player_posz; sta al_worldz; sep; dec al_sbyte4; bne .nchg;
///  lda #150; sta al_sbyte4; lda al_sflags2; eor #sflag1($10); sta al_sflags2;
///  .nchg: lda gameframe; and #$07; bne .donespeed;
///    lda al_sflags2; and #sflag1; bne .speeddown;
///      lda gsvar_byte1; cmp #5; beq .donespeed; inc gsvar_byte1; bra .donespeed;
///    .speeddown: lda gsvar_byte1; cmp #-5; beq .donespeed; dec gsvar_byte1;
///  .donespeed: <s_add_bossHP: $70:0170 += al_HP>; rtl`. UNIQUE masked hit
/// (+$0C shift from built $0793AF). Reads `player_posz`($1511), `gameframe`
/// ($15BB), `gsvar_byte1`($154F). Port <-> `bosses::boss8_cont`.
pub const RETAIL_BOSS8_CONT: u32 = 0x07_93BB;
/// Retail `gsvar_byte1` ($154F, built $15DA) — the boss8 wall-rotation speed
/// accumulator, read straight out of `boss8_cont`'s `lda/inc/dec gsvar_byte1`
/// operands (all three agree). Port <-> ext-WRAM cell $0310 (`ebwm::GSVAR_BYTE1`,
/// `g.vars.read_ext8(0x0310)`), a representation remap of the same logical cell.
pub const RETAIL_GSVAR_BYTE1: u32 = 0x154F;
/// Retail `currentlevel` ($1FFD, built $1B01) — the difficulty gate boss8_Istrat
/// reads (`cmp #0` -> level 1 = easy = boss8HP; else = boss8HP*2). Read out of
/// `boss8_Istrat`'s `lda currentlevel` operand.
pub const RETAIL_CURRENTLEVEL: u32 = 0x1FFD;
/// `al_sbyte4` struct offset ($25) — boss8's phase-toggle countdown, from
/// `boss8_cont`'s `dec al_sbyte4,x` operand (identical retail/built/port; it is
/// `al_sbyte3`($24) + 1).
pub const AL_SBYTE4: u32 = 0x25;
/// boss8's `sflag1` = `al_sflags2` bit $10 (from `boss8_cont`'s `eor #$10`).
/// Port <-> `bosses::B8_SFLAG1` (= 0x10).
pub const B8_SFLAG1: u8 = 0x10;
/// boss8's `sflag4` = `al_sflags2` bit $80 (from `boss8a_strat`'s `ora #$80` /
/// `boss8b_strat`'s `and #$7F`). Port <-> `bosses::B8_SFLAG4`.
pub const B8_SFLAG4: u8 = 0x80;
/// boss8's `sflag5` = `al_sflags3` bit $01 (from `boss8a_strat`'s `lda sflags3;
/// and #$01`). Port <-> `bosses::B8_SFLAG5` on `sflags3`.
pub const B8_SFLAG5: u8 = 0x01;
/// Retail `boss8a_init` ($07:9422) — open-flap phase entry (jml target from
/// `boss8wait_strat` when beam3 is gone or has sflag1). Sets stratptr=boss8a,
/// collstrat=hitflash, sbyte2=100, trigse $73; falls into `boss8a_strat`.
pub const RETAIL_BOSS8A_INIT: u32 = 0x07_9422;
/// Retail `boss8a_strat` ($07:9451) — open-flap per-tick (read out of
/// `boss8a_init`'s `s_set_strat` immediate). Sets sflag4; HPLASMA on frames
/// 25/30; on hard (`currentlevel!=0`) closes via sbyte2 countdown or any beam
/// with sflag1 clear → `boss8b_init`.
pub const RETAIL_BOSS8A_STRAT: u32 = 0x07_9451;
/// Retail `boss8b_init` ($07:9539) — close-flap phase entry (jml from
/// `boss8a_strat`). Clears collstrat, installs boss8b, sbyte2=15, clears beam
/// sflag1 ×3, trigse $72; falls into `boss8b_strat`.
pub const RETAIL_BOSS8B_INIT: u32 = 0x07_9539;
/// Retail `boss8b_strat` ($07:95A6) — close-flap per-tick (read out of
/// `boss8b_init`'s `s_set_strat` immediate). Clears sflag4; sbyte2 countdown
/// → `boss8wait_init`.
pub const RETAIL_BOSS8B_STRAT: u32 = 0x07_95A6;

// ------------------------------------------------------------------------
// BOSS2 — the "spinning top" (Macbeth spider / Venom1, GBSTRATS.ASM:484-... ).
// A multi-phase BOSS with a NINE-child family (1 top + 4 petals + 4 turrets).
// The pieces certifiable WITHOUT the GSU are:
//   * boss2_Istrat — INIT: spawns the 9-child family, then a clean scalar init
//                    (HP=$FF, AP=10, colltype enemy1|enemyweap, lifecnt=50,
//                    bossmaxHP=0, sflags2|=colldisable, sflags|=shadow,
//                    stratptr=boss2_strat, expstratptr=boss2exp_Istrat). Unlike
//                    boss8 the Istrat does NOT fall into the per-tick body — it
//                    RTLs, so worldz is untouched by init.
//   * boss2_strat state 0 (the "wait/idle" phase) — the common per-tick body the
//                    boss runs while its top child lives: counts children into
//                    svar_byte5, and while <=7 sets sflag4, sflag1, sbyte3=2 and
//                    accumulates roty += 4/tick; the near branch (|dz|<1100) does
//                    keeprelto_player + add_playerZ = worldz += playervel_z. Pure
//                    CPU, NO GSU, NO RNG on the near path (the far path spawns
//                    RNG smoke — deferred). Reads player_posz-via-PLAYPT (zdist
//                    gate) + playervel_z + pviewvelz + the child-link chain.
// The phase-transition machine (states 1..5: leap/slam/backaway/strafe/die) is
// gated on child liveness + player HP and is documented as the remaining gap.
// All addresses located by masked signature scan of the retail cart (skeleton
// read out of the built ROM via symbols.txt, WRAM/jml operands wildcarded).
// ------------------------------------------------------------------------

/// Retail `boss2_Istrat` ($08:8BBE, GBSTRATS.ASM:484 — built $08:8BBA, +4 shift)
/// — the boss2 shell INIT. Spawns 9 children (`s_make_childobj` ×9: 1 top, 4
/// petals, 4 turrets), then `s_set_alptrs x,boss2_strat,0,boss2exp_Istrat;
/// s_set_aldata x,#hardHP($FF),#10; s_set_colltype enemy1|enemyweap;
/// s_set_lifecnt #50; s_set_bossmaxHP #0; s_set_alsflag colldisable(sflags2 $01);
/// s_set_alsflag shadow(sflags $08); trigse $95; rtl`. UNIQUE masked hit (the
/// tail scalar-init anchor). Port <-> `bosses::strat_boss2_init` (IS_BOSS2=108).
pub const RETAIL_BOSS2_ISTRAT: u32 = 0x08_8BBE;
/// Retail `boss2_strat` ($08:8E3C — built $08:8E38, +4) — the per-tick state
/// machine the Istrat installs (read straight out of `boss2_Istrat`'s
/// `s_set_alptrs` stratptr immediate). State 0 = the wait/idle body certified
/// here; states 1..5 (leap/slam/backaway/strafe/die) are the documented gap.
pub const RETAIL_BOSS2_STRAT: u32 = 0x08_8E3C;
/// Retail `boss2exp_Istrat` ($08:9391 — built $08:938D, +4) — the death/explosion
/// strat boss2_Istrat installs as `expstratptr` (read out of the Istrat's
/// extended-array `sta expstratptr` immediate). Lives in the parallel xalblks
/// array (like `gnd`/`woods`), so certified by the installed pointer value.
pub const RETAIL_BOSS2EXP_ISTRAT: u32 = 0x08_9391;
/// Retail `bossflags` ($14D3) — `s_boss_dying` gate/set (`and/ora #bf_dying=$10`)
/// in boss2 state-5 `.dodie` (and every other `s_boss_dying` site). Built $1F02.
/// Port <-> `enemy_a::bossflags` / `wm::BOSSFLAGS`.
pub const RETAIL_BOSSFLAGS: u32 = 0x14D3;
/// Retail `pstratflags` ($14DD) — `s_boss_dying` sets `pstf_notdie=$20`.
/// Port <-> `GameVars::pstratflags` / `PSTF_NOTDIE`.
pub const RETAIL_PSTRATFLAGS: u32 = 0x14DD;
/// Retail `kill_Istrat` ($06:8D07) — boss2 state-5 falldown settle target
/// (`s_falldown_Yvec …,kill_Istrat`). Port <-> `common::kill_istrat`.
pub const RETAIL_KILL_ISTRAT: u32 = 0x06_8D07;
/// Retail `playervel_z` ($14EA — built $1575, the −$8B dostrats-globals shift) —
/// the player's Z velocity, read by boss2's `s_keeprelto_player` leaf ($1F:DB21:
/// `al_worldz += playervel_z − pviewvelz`). Located + cross-validated by masked
/// scan of that leaf (`al_worldz += $14EA − $14F4`), UNIQUE + jsl-referenced.
/// Port <-> `g.vars.playervel_z`.
pub const RETAIL_PLAYERVEL_Z: u32 = 0x14EA;
/// Retail `s_keeprelto_player` leaf ($1F:DB21) — `rep; lda playervel_z; sec;
/// sbc pviewvelz; clc; adc al_worldz,x; sta al_worldz,x; sep; rtl`. boss2's
/// state-0 near branch calls it (then `s_add_playerZ` = `worldz += pviewvelz`),
/// so the net per-tick worldz change is `+= playervel_z`. Port <->
/// `enemy_a::boss_keeprel_to_player`.
pub const RETAIL_KEEPRELTO_PLAYER: u32 = 0x1F_DB21;
/// `al_lifecnt` / `al_count` struct offset ($0A) — the boss lifetime/anim counter
/// (identical retail/built/port; from `boss2_Istrat`'s `sta al_lifecnt,x`
/// operand). Port <-> `Alien::count`.
pub const AL_LIFECNT: u32 = 0x0A;
/// boss2's `sflag4` = `al_sflags2` bit $80, `sflag1` = bit $10 (from
/// `boss2_strat` state 0's `ora #$80` / `ora #$10`). Port <-> `bosses::
/// BOSS2_SFLAG4` / `BOSS2_SFLAG1` (same bit positions — raw-diffable).
pub const B2_SFLAG4: u8 = 0x80;
pub const B2_SFLAG1: u8 = 0x10;
/// boss2's `sflag3` = `al_sflags2` bit $40 (from state-4 `ora #$40` / sound gate).
pub const B2_SFLAG3: u8 = 0x40;
/// Retail `al_stratstate` in the xalblks parallel array (`$1CDC,x` — from
/// `boss2_strat`'s `lda/sta $1CDC,x` state gates). Port <-> `Alien::stratstate`.
pub const RETAIL_AL_STRATSTATE: u32 = 0x1CDC;
/// Retail `svar_byte5` ($1530) — boss2's child-count scratch (from
/// `boss2_strat`'s `stz/inc/lda $1530`). Port recomputes via `boss_count_children`.
pub const RETAIL_SVAR_BYTE5: u32 = 0x1530;
/// `al_ptr` struct offset ($06) — boss2 state-2 particle link (STRUCTS.INC
/// `defal ptr,2` after shape@$04). Port <-> `Alien::ptr`.
pub const AL_PTR: u32 = 0x06;
/// `al_sword2` struct offset ($28) — boss2 ground Y / state scratch (after
/// `al_sword1`@$26). Port <-> `Alien::sword2`.
pub const AL_SWORD2: u32 = 0x28;

// ------------------------------------------------------------------------
// BOSSG / BOSSSEAMON — the route-2 sea bosses (D2STRATS.ASM / GA2STRAT.ASM).
//   * bossg_istrat ($04:EE35) — a CLEAN scalar INIT (no RNG, no children):
//     stratptr=bossg_strat, coll=hitflash, exp=bossgexplode, HP=$FF, AP=8,
//     init_anim, sflags|=shadow, collflags|=enemy1, stratmem(mode)=0, trigse.
//     It FALLS THROUGH into its `s_mode_table` body (mode 0 = wait-until-near),
//     which on a far player is a clean `worldz -= 40; return` (no GSU, no spawn).
//   * bossseamon_istrat ($0A:F2D1) — draws the RNG ONCE (`sbyte2`) then a scalar
//     init (HP=2, AP=4, roty=deg180, collflags|=enemyweap, type&=~ATZREMOVE,
//     sbyte3=60, sbyte4=3, stratptr=bossseamon_strat) and FALLS THROUGH into its
//     player-relative body. The RNG-`sbyte2` + the fall-through body are the gap;
//     the stable scalar init (HP/AP/roty/collflags/stratptr) is certified.
// bossg is at the SAME address in retail as built (bank $04 unshifted, like
// parajump); bossseamon shifted +$2A (bank $0A, like firepillar). All located by
// masked signature scan of the retail cart.
// ------------------------------------------------------------------------

/// Retail `bossg_istrat` ($04:EE35, D2STRATS.ASM:54 — SAME as built, bank $04
/// unshifted). Port <-> `bosses::strat_bossg_init` (IS_BOSSG=144 / 0x030006).
pub const RETAIL_BOSSG_ISTRAT: u32 = 0x04_EE35;
/// Retail `bossg_strat` ($04:EE85 — the `.strat s_mode_table` per-tick body the
/// Istrat installs, read out of the Istrat's `s_set_alptrs` immediate). Mode 0 =
/// `.waituntilalmosthitplayer` (`worldz -= 40` until |dz| < 150).
pub const RETAIL_BOSSG_STRAT: u32 = 0x04_EE85;
/// Retail `al_tx` in xalblks (`$1CF4,x` — from bossg `.scrollmsg`'s
/// `lda/adc #4/sta $1CF4,x`). Port <-> `Alien::tx`.
pub const RETAIL_AL_TX: u32 = 0x1CF4;
/// Retail `al_animframe` in xalblks (`$1CE7,x` — from bossg_istrat's
/// `s_init_anim` `ora #$80 / sta $1CE7,x`). Port <-> `Alien::animframe`.
pub const RETAIL_AL_ANIMFRAME: u32 = 0x1CE7;
/// Retail `bossgexplode_istrat` ($04:F326 — the death strat, read out of the
/// Istrat's extended-array `sta expstratptr` immediate).
pub const RETAIL_BOSSGEXPLODE_ISTRAT: u32 = 0x04_F326;
/// Retail `bossgs_istrat` ($04:F55E — shadow-clone INIT installed by
/// `.generateshadows`'s `s_set_strat y,bossgs_istrat`). Port <-> `bossgs_init`.
pub const RETAIL_BOSSGS_ISTRAT: u32 = 0x04_F55E;
/// Retail `maptrigger` ($176D — built $17F2, the −$85 shift) — bossg zeroes it in
/// its INIT (`stz maptrigger`), read out of the Istrat's `stz` operand.
pub const RETAIL_MAPTRIGGER: u32 = 0x176D;

/// Retail `flyingfish_istrat` ($00:FAD6 — D3STRATS.ASM; unique
/// `roty+=deg180` + HP=4/AP=8 anchor). Port <-> `flyingfish_init`.
pub const RETAIL_FLYINGFISH_ISTRAT: u32 = 0x00_FAD6;
/// Retail `flyingfish_strat` body (`.strat` after istrat `set_alptrs`, $00:FB03).
pub const RETAIL_FLYINGFISH_STRAT: u32 = 0x00_FB03;
/// Retail `flyingfish` `.flying` body ($00:FC29 — `s_set_strat x,.flying`).
pub const RETAIL_FLYINGFISH_FLYING: u32 = 0x00_FC29;

/// Retail `bossseamon_istrat` ($0A:F2D1, GA2STRAT.ASM — built $0A:F2A7, +$2A).
/// Port <-> `bosses::strat_bossseamon_init` (typed direct-strategy key).
pub const RETAIL_BOSSSEAMON_ISTRAT: u32 = 0x0A_F2D1;
/// Retail `bossseamon_strat` ($0A:F31E — built $0A:F2F4, +$2A — the player-
/// relative per-tick body the Istrat installs + falls through into).
pub const RETAIL_BOSSSEAMON_STRAT: u32 = 0x0A_F31E;
/// Retail `bossseamonexp_istrat` ($0A:F675 — built $0A:F64B, +$2A).
pub const RETAIL_BOSSSEAMONEXP_ISTRAT: u32 = 0x0A_F675;

// ------------------------------------------------------------------------
// BOSS1 — the barricader (Corneria boss, GBSTRATS.ASM). A NINE-child family
// (8 turrets + 1 cover) with a level-gated HP (like boss8). Its per-tick phase
// strats fall through a GSU turret-repositioning tail (`boss1rots_srou`) — the
// harder body, documented as the gap. The INIT is a clean self-contained routine
// (ends RTL, no fall-through into the GSU body): level-gated HP, AP=10,
// roty=deg180, collflags|=enemy1, type|=gnd, sflags|=shadow|colldisable, +9-child
// spawn. Located by masked signature scan (built $08:816A, retail +4).
// ------------------------------------------------------------------------

/// Retail `boss1_istrat` ($08:816E — built $08:816A, +4). Port <->
/// `enemy_a::strat_boss1_init` (IS_BOSS1). HP is level-gated: retail
/// `currentlevel==0` -> HP=$23(35), else HP=$46(70) (a level-encoding remap of
/// the port's `currentlevel==1 -> 35 else 70`, same class as boss8).
pub const RETAIL_BOSS1_ISTRAT: u32 = 0x08_816E;
/// Retail `boss1up_strat` ($08:8413 — built $08:840F, +4 — the per-tick body the
/// Istrat installs; its phase strats fall through the GSU `boss1rots_srou` tail).
pub const RETAIL_BOSS1UP_STRAT: u32 = 0x08_8413;

// ------------------------------------------------------------------------
// PLAYER-MOVE plrot* ACCUMULATOR — the pad-read -> roty/rotz tilt accumulation
// inside `playermove_srou` (PSTRATS.ASM:2334-2703). The deferred player-move
// sub-step (UPDATE 11 left "the plrot* accumulator body" as the only remaining
// uncertified player-move piece). Per frame, controllable flight does:
//   * LEFT  held: plrotz += Zrotspeed($200); plroty += Zrotspeed
//   * RIGHT held: plrotz -= Zrotspeed;       plroty -= Zrotspeed
//   * decay: plroty = Achase(plroty, 0, rate 3); plrotz = Achase(plrotz, 0,
//     rate 4); then LIMIT plrotz to [-$600, +$600].
// The Achase primitive is `strat_chase_proportional`, already certified vs the
// retail cart (parajump, UPDATE 4). `playermove_srou` is a ~600-byte routine
// that reads the pad + a dozen player globals and threads no clean RTL through
// the plrot block, so — exactly like the RAM-resident COLDET and the inline
// fire-gate (UPDATES 8/10) — the plrot accumulator is certified by CROSS-
// VALIDATING its constants (step, clamp, plrotz/y addresses) against the retail
// ROM BYTES + the certified decay primitive, then grid-diffing the composed
// per-frame update. All blocks located by masked signature scan (UNIQUE), the
// plrotz/plroty absolute operands wildcarded then read back.
// ------------------------------------------------------------------------

/// Retail `plroty` / `plrotz` — the player yaw/roll tilt accumulators (16-bit,
/// `>>8` -> `al_roty`/`al_rotz`). Built $12BD/$12BF; retail = built − $8B (the
/// same dostrats-globals shift as player_pos*), read straight out of the plrot
/// accumulation block's `lda/sta` operands. Port <-> `sv::PLROTY`/`sv::PLROTZ`.
pub const RETAIL_PLROTY: u32 = 0x1232;
pub const RETAIL_PLROTZ: u32 = 0x1234;
/// Retail plrot LEFT-steer accumulation block ($0B:DA79): `rep; lda plrotz; clc;
/// adc #$0200; sta plrotz; sep; rep; lda plroty; clc; adc #$0200; sta plroty;
/// sep`. UNIQUE masked hit; its `adc` immediates read back = Zrotspeed ($200).
pub const RETAIL_PLROT_ACCUM_LEFT: u32 = 0x0B_DA79;
/// Retail plrot RIGHT-steer accumulation block ($0B:DACA): the same, `sec; sbc
/// #$0200` (subtract). UNIQUE masked hit.
pub const RETAIL_PLROT_ACCUM_RIGHT: u32 = 0x0B_DACA;
/// Retail plrotz LIMIT block ($0B:DD8E): `rep; lda plrotz; cmp #$0000; bmi ...;
/// cmp #$0600; bmi ...` then the `lda #$FA00 (-$600); sta plrotz` lower clamp —
/// i.e. `s_limit_var W,plrotz,-$600,$600`. UNIQUE masked hit; the `cmp` operand
/// reads back = $0600.
pub const RETAIL_PLROT_CLAMP: u32 = 0x0B_DD8E;
/// `Zrotspeed`/`Xrotspeed` = $0200 (PSTRATS.ASM:1684-1685) — the per-frame plrot
/// steering step, read out of the accumulation block. Port <-> `player::
/// XROT_SPEED`/`ZROT_SPEED` (= 0x200).
pub const RETAIL_ZROTSPEED: i16 = 0x0200;
/// plrotz roll clamp magnitude ($600), read out of the LIMIT block. Port <->
/// `player.rs:1067` `clamp16(plrotz, -0x600, 0x600)`.
pub const RETAIL_PLROTZ_CLAMP: i16 = 0x0600;

/// Seed the player-relative + RNG machine state into retail WRAM so a
/// player-aware / RNG-drawing strat starts byte-identical to the port. Writes
/// the `player_posx/y/z` mirror globals and the 4-byte `rand` SWB state.
///
/// The port equivalent is: `g.vars.player_posx = px; ...player_posy = py;
/// ...player_posz = pz; g.vars.rng = rng_seed;` (+ a live player object at
/// slot 0 if the strat reads the player's world coords through `PLAYPT`).
pub fn seed_player_relative_state(bus: &mut SnesBus, px: i16, py: i16, pz: i16, rng_seed: [u8; 4]) {
    bus.wram_write16(RETAIL_PLAYER_POSX, px as u16);
    bus.wram_write16(RETAIL_PLAYER_POSY, py as u16);
    bus.wram_write16(RETAIL_PLAYER_POSZ, pz as u16);
    seed_retail_rng(bus, rng_seed);
}

/// Write the retail runtime-RNG state (`rand`, $EF-$F2) into WRAM.
pub fn seed_retail_rng(bus: &mut SnesBus, rng_seed: [u8; 4]) {
    for (i, &b) in rng_seed.iter().enumerate() {
        bus.write8(RETAIL_RAND + i as u32, b);
    }
}

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
    call(
        bus,
        RETAIL_KILL_LIST,
        &Entry {
            p: 0x00,
            ..Default::default()
        },
    );
}

// ------------------------------------------------------------------------
// Retail boot-from-reset probe (milestone 1).
// ------------------------------------------------------------------------

use crate::ppu::{Ppu, PpuFrame, FRAME_HEIGHT};
use sf_spc::{Filter, SnesSpc, BASS_NORM, GAIN_UNIT, IPL_ROM};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use w65c816::{AddressType, Signals, System, CPU};

/// Thread-safe PCM queue produced by the retail SPC-700 and consumed by the
/// SDL audio callback. Samples are signed 16-bit interleaved stereo at the
/// native SNES 32 kHz rate.
pub type RetailPcmQueue = Arc<Mutex<VecDeque<i16>>>;

const SNES_MASTER_CLOCK_HZ: u64 = 21_477_272;
const SPC_CLOCK_HZ: u64 = 1_024_000;
const APU_OUTPUT_CAPACITY: usize = 2048;
const APU_QUEUE_CAPACITY: usize = 32_000 * 2 * 4;

/// The retail cartridge's real SPC-700/S-DSP, synchronized to the PPU raster.
///
/// `SnesSpc` writes through a raw pointer, so the output allocation is boxed
/// and never moved. Only completed PCM is shared with the audio thread; the
/// emulation core itself remains owned by the SNES bus.
struct RetailApu {
    spc: Box<SnesSpc>,
    filter: Filter,
    output: Box<[i16; APU_OUTPUT_CAPACITY]>,
    pcm: RetailPcmQueue,
    frame_start_clock: u64,
    generated_samples: u64,
    last_cpu_reads: [u8; 4],
    last_cpu_writes: [u8; 4],
}

impl RetailApu {
    fn new() -> Self {
        let mut spc = SnesSpc::new();
        spc.init_rom(&IPL_ROM);
        spc.reset();
        let mut filter = Filter::new();
        filter.set_gain(GAIN_UNIT);
        filter.set_bass(BASS_NORM);
        let mut apu = Self {
            spc,
            filter,
            output: Box::new([0; APU_OUTPUT_CAPACITY]),
            pcm: Arc::new(Mutex::new(VecDeque::with_capacity(APU_QUEUE_CAPACITY))),
            frame_start_clock: 0,
            generated_samples: 0,
            last_cpu_reads: [0; 4],
            last_cpu_writes: [0; 4],
        };
        apu.arm_output();
        apu
    }

    #[inline]
    fn clock_at_master(master_clock: u64) -> u64 {
        // A rational conversion keeps the 32 kHz audio clock phase-locked to
        // the SNES master oscillator over long runs.
        ((u128::from(master_clock) * u128::from(SPC_CLOCK_HZ)) / u128::from(SNES_MASTER_CLOCK_HZ))
            as u64
    }

    #[inline]
    fn time_in_frame(&self, master_clock: u64) -> i32 {
        Self::clock_at_master(master_clock)
            .saturating_sub(self.frame_start_clock)
            .min(i32::MAX as u64) as i32
    }

    fn arm_output(&mut self) {
        // SAFETY: `output` is a boxed allocation owned by `self`, so its data
        // pointer remains stable until the next end-of-frame re-arm.
        unsafe {
            self.spc
                .set_output(self.output.as_mut_ptr(), APU_OUTPUT_CAPACITY as i32)
        };
    }

    fn read_port(&mut self, master_clock: u64, port: usize) -> u8 {
        let value = self.spc.read_port(self.time_in_frame(master_clock), port) as u8;
        self.last_cpu_reads[port] = value;
        value
    }

    fn write_port(&mut self, master_clock: u64, port: usize, value: u8) {
        self.spc
            .write_port(self.time_in_frame(master_clock), port, i32::from(value));
        self.last_cpu_writes[port] = value;
    }

    fn finish_video_frame(&mut self, master_clock: u64) {
        let end_clock = Self::clock_at_master(master_clock);
        let end_time = end_clock.saturating_sub(self.frame_start_clock) as i32;
        self.spc.end_frame(end_time);

        let count = ((self.spc.sample_count().max(0) as usize).min(APU_OUTPUT_CAPACITY)) & !1;
        self.filter.run(&mut self.output[..count]);
        if count != 0 {
            let mut pcm = self
                .pcm
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let excess = pcm
                .len()
                .saturating_add(count)
                .saturating_sub(APU_QUEUE_CAPACITY);
            let discard = excess.min(pcm.len());
            pcm.drain(..discard);
            pcm.extend(self.output[..count].iter().copied());
            self.generated_samples = self.generated_samples.wrapping_add(count as u64);
        }

        self.frame_start_clock = end_clock;
        self.arm_output();
    }

    fn pcm_queue(&self) -> RetailPcmQueue {
        Arc::clone(&self.pcm)
    }

    fn pcm_stats(&self) -> (usize, usize, i16, i16, u64) {
        let pcm = self
            .pcm
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut nonzero = 0usize;
        let mut minimum = i16::MAX;
        let mut maximum = i16::MIN;
        let mut fnv1a = 0xCBF2_9CE4_8422_2325u64;
        for &sample in pcm.iter() {
            nonzero += usize::from(sample != 0);
            minimum = minimum.min(sample);
            maximum = maximum.max(sample);
            for byte in sample.to_le_bytes() {
                fnv1a = (fnv1a ^ u64::from(byte)).wrapping_mul(0x100_0000_01B3);
            }
        }
        if pcm.is_empty() {
            minimum = 0;
            maximum = 0;
        }
        (pcm.len(), nonzero, minimum, maximum, fnv1a)
    }
}

/// One completed SNES DMA transfer observed by [`RetailBootBus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaEvent {
    pub channel: u8,
    pub source: u32,
    /// CPU WRAM destination when BBAD is WMDATA (`$2180`), otherwise `None`.
    pub wram_destination: Option<u32>,
    pub length: u32,
    pub dmap: u8,
    pub bbad: u8,
    /// Number of nonzero bytes observed on the transfer bus.
    pub nonzero_bytes: u32,
    /// FNV-1a of the transferred byte stream.
    pub fnv1a: u32,
}

/// A bus that boots the retail cart from its *real* reset vector (unlike
/// [`SnesBus`], which overrides $FFFC to a bootstrap stub for direct subroutine
/// calls). Hardware registers are lightly stubbed so the boot can make forward
/// progress far enough to characterise where a CPU-only core stalls.
///
/// The PPU models a **free-running raster** driven by the SNES master clock,
/// with address-dependent 6/8/12-clock CPU bus cycles, sweeping H (0..341)
/// and V (0..262) —
/// so that scanline/vblank spin loops (e.g. the `$03:BD97` OPVCT raster-wait)
/// actually satisfy instead of parking forever. This is the *minimal* hardware
/// needed to march the boot into the per-frame game loop; it is NOT a real PPU
/// (no framebuffer, no rendering, no OAM/CGRAM effects).
pub struct RetailBootBus {
    inner: SnesBus,
    /// CPU-visible PPU registers and backing video memories.  Raster timing is
    /// still owned by this bus; the PPU object captures port semantics and
    /// produces the native 256x224 reference frame.
    ppu: Ppu,
    res_line: bool,
    /// Free-running dot counter (advanced by [`boot_retail`] each CPU clock).
    /// H = `dot % DOTS_PER_LINE`, V = `(dot / DOTS_PER_LINE) % LINES_PER_FRAME`.
    pub dot: u64,
    /// NTSC master oscillator count. CPU bus accesses take 6, 8, or 12 master
    /// clocks; four master clocks advance one nominal PPU dot.
    master_clock: u64,
    /// Duration selected by the address touched during the current 65816
    /// microcycle. Internal/invalid cycles use the CPU's fixed 6-clock speed.
    cpu_cycle_master_clocks: u8,
    /// Synchronous DMA time charged after the `$420B` initiating CPU write.
    dma_master_clocks_pending: u64,
    /// MEMSEL `$420D` bit 0, controlling 6-clock FastROM accesses.
    fast_rom: bool,
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
    /// Real SPC-700/S-DSP instance. The retail 65816 uploads and commands its
    /// own SF2 driver through $2140-$2143; no host protocol is synthesized.
    apu: RetailApu,
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
    /// The CPU crate samples IRQ as a level but (unlike silicon) can re-enter
    /// while I is set.  Present each latched request once; TIMEUP remains
    /// sticky until the handler reads it.
    irq_line_delivered: bool,
    /// Current 65816 P.I state, supplied by the boot harness before each CPU
    /// cycle.  `w65c816` 0.1.17 incorrectly enters a non-WAI IRQ when I is set;
    /// gating the bus line preserves the hardware mask while leaving TIMEUP
    /// pending until the game executes CLI.
    cpu_irq_masked: bool,
    /// Auto-joypad-read result presented on $4218 (JOY1L) / $4219 (JOY1H).
    /// Bit layout (16-bit): B Y Sel Start Up Dn Lt Rt A X L R 0 0 0 0.
    /// Default 0 = no buttons; set via [`RetailBootBus::set_pad1`] to script
    /// input for the co-exec harness.
    pad1: u16,
    /// CPU-visible WRAM data-port address (`$2181..$2183`, 17 bits).
    wram_port_addr: u32,
    /// DMA channel registers `$43x0..$43xF`.  Boot-time ROM-to-WRAM DMA is
    /// required before IRQ/NMI vectors can safely enter their low-WRAM
    /// trampolines; PPU-targeted transfers are consumed but otherwise ignored.
    dma: [[u8; 16]; 8],
    hdma_channels: u8,
    hdma_finished: [bool; 8],
    hdma_do_transfer: [bool; 8],
    dma_events: Vec<DmaEvent>,
    /// Last real opcode fetch observed on the bus and its monotonic sequence
    /// number.  CPU register accessors can expose transient microcycle state,
    /// so archaeology tools use this pair for instruction-accurate tracing.
    last_opcode_address: u32,
    opcode_fetch_count: u64,
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
            ppu: Ppu::new(),
            res_line: true,
            dot: 0,
            master_clock: 0,
            cpu_cycle_master_clocks: 6,
            dma_master_clocks_pending: 0,
            fast_rom: false,
            latched_h: 0,
            latched_v: 0,
            ophct_hi: false,
            opvct_hi: false,
            nmi_latch: false,
            prev_vblank: false,
            apu: RetailApu::new(),
            nmi_enabled: false,
            irq_enabled: false,
            irq_vtime: VBLANK_START_LINE as u16,
            irq_pending: false,
            irq_line_delivered: false,
            cpu_irq_masked: true,
            pad1: 0,
            wram_port_addr: 0,
            dma: [[0; 16]; 8],
            hdma_channels: 0,
            hdma_finished: [false; 8],
            hdma_do_transfer: [false; 8],
            dma_events: Vec::new(),
            last_opcode_address: 0,
            opcode_fetch_count: 0,
        }
    }

    /// Set the controller-1 button state presented to the auto-joypad registers.
    pub fn set_pad1(&mut self, buttons: u16) {
        self.pad1 = buttons;
    }

    /// Supply the CPU interrupt-disable flag before the next [`CPU::cycle`].
    /// This is required only because the current CPU dependency does not mask
    /// ordinary IRQ entry itself.
    pub fn set_cpu_irq_masked(&mut self, masked: bool) {
        self.cpu_irq_masked = masked;
    }

    /// Attach the Super FX core to a reset-boot bus.  Most host-side boot
    /// archaeology only needs the hardware-register shims, but SF2 reaches
    /// GSU jobs much earlier than SF1 and some captures need the real job to
    /// complete before the 65816 continues.
    pub fn enable_gsu(&mut self) {
        self.inner.enable_gsu();
    }

    /// Read the CPU-visible address space without advancing either processor.
    /// This is intentionally a narrow observation API for reset-boot capture
    /// tools; normal oracle calls should continue to use [`SnesBus`].
    pub fn peek8(&self, addr: u32) -> u8 {
        self.inner.read8(addr)
    }


    /// Snapshot the CPU-visible video state and composite a native SNES frame.
    pub fn ppu_frame(&self) -> PpuFrame {
        self.ppu.frame()
    }

    pub fn ppu_snapshot_rgba(&self) -> Vec<u8> {
        self.ppu.snapshot_rgba()
    }

    pub fn ppu_snapshot_bg_rgba(&self, bg: usize) -> Vec<u8> {
        self.ppu.snapshot_bg_rgba(bg)
    }

    pub fn ppu_snapshot_bg_indices(&self, bg: usize) -> Vec<u8> {
        self.ppu.snapshot_bg_indices(bg)
    }

    /// Write the CPU-visible address space without advancing either processor.
    /// Used to seed input/state in reset-boot archaeology tools.
    pub fn poke8(&mut self, addr: u32, value: u8) {
        self.inner.write8(addr, value);
    }

    /// Return `(fetch_sequence, address)` for the most recent opcode fetch.
    pub fn opcode_fetch_state(&self) -> (u64, u32) {
        (self.opcode_fetch_count, self.last_opcode_address)
    }

    /// Drain DMA transfers completed since the previous observation point.
    pub fn take_dma_events(&mut self) -> Vec<DmaEvent> {
        std::mem::take(&mut self.dma_events)
    }

    /// Internal APU state for reset-boot archaeology diagnostics: generated
    /// PCM count, queued PCM count, last CPU reads, and last CPU writes.
    pub fn apu_debug_state(&self) -> (u64, usize, [u8; 4], [u8; 4]) {
        let queued = self
            .apu
            .pcm
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len();
        (
            self.apu.generated_samples,
            queued,
            self.apu.last_cpu_reads,
            self.apu.last_cpu_writes,
        )
    }

    pub fn apu_pcm_queue(&self) -> RetailPcmQueue {
        self.apu.pcm_queue()
    }

    pub fn timing_debug_state(&self) -> (bool, bool, u16, bool, bool, u16, u16) {
        (
            self.nmi_enabled,
            self.irq_enabled,
            self.irq_vtime,
            self.irq_pending,
            self.cpu_irq_masked,
            self.cur_h(),
            self.cur_v(),
        )
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

    #[inline]
    fn wram_port_cpu_address(&self) -> u32 {
        let address = self.wram_port_addr & 0x1_FFFF;
        if address < 0x1_0000 {
            0x7E_0000 | address
        } else {
            0x7F_0000 | (address & 0xFFFF)
        }
    }

    #[inline]
    fn advance_wram_port(&mut self) {
        self.wram_port_addr = (self.wram_port_addr + 1) & 0x1_FFFF;
    }

    fn read_b_bus(&mut self, register: u8) -> u8 {
        match register {
            0x80 => {
                let value = self.inner.read8(self.wram_port_cpu_address());
                self.advance_wram_port();
                value
            }
            0x00..=0x3F => self.ppu.read(0x2100 | u16::from(register)).unwrap_or(0),
            _ => 0,
        }
    }

    fn write_b_bus(&mut self, register: u8, value: u8) {
        match register {
            0x80 => {
                self.inner.write8(self.wram_port_cpu_address(), value);
                self.advance_wram_port();
            }
            0x00..=0x3F => self.ppu.write(0x2100 | u16::from(register), value),
            _ => {}
        }
    }

    fn run_dma(&mut self, enabled: u8) {
        const B_PATTERNS: [&[u8]; 8] = [
            &[0],
            &[0, 1],
            &[0, 0],
            &[0, 0, 1, 1],
            &[0, 1, 2, 3],
            &[0, 1, 0, 1],
            &[0, 0],
            &[0, 0, 1, 1],
        ];

        let mut dma_clocks = if enabled != 0 {
            let start = self
                .master_clock
                .saturating_add(u64::from(self.cpu_cycle_master_clocks));
            8 - (start & 7)
        } else {
            0
        };
        for channel in 0..8 {
            if enabled & (1 << channel) == 0 {
                continue;
            }
            let regs = self.dma[channel];
            let dmap = regs[0];
            let bbad = regs[1];
            let mut a_addr = u16::from_le_bytes([regs[2], regs[3]]);
            let a_bank = regs[4];
            let mut remaining = u16::from_le_bytes([regs[5], regs[6]]) as u32;
            if remaining == 0 {
                remaining = 0x1_0000;
            }
            dma_clocks = dma_clocks.saturating_add(8 + u64::from(remaining) * 8);
            let source = ((a_bank as u32) << 16) | u32::from(a_addr);
            let wram_destination = (bbad == 0x80).then(|| self.wram_port_cpu_address());
            let pattern = B_PATTERNS[(dmap & 7) as usize];
            let fixed = dmap & 0x08 != 0;
            let decrement = dmap & 0x10 != 0;
            let b_to_a = dmap & 0x80 != 0;
            let mut nonzero_bytes = 0u32;
            let mut fnv1a = 0x811C_9DC5u32;

            for i in 0..remaining {
                let b_register = bbad.wrapping_add(pattern[i as usize % pattern.len()]);
                let a_cpu = ((a_bank as u32) << 16) | a_addr as u32;
                if b_to_a {
                    let value = self.read_b_bus(b_register);
                    self.inner.write8(a_cpu, value);
                } else {
                    let value = self.inner.read8(a_cpu);
                    self.write_b_bus(b_register, value);
                    nonzero_bytes += u32::from(value != 0);
                    fnv1a = (fnv1a ^ u32::from(value)).wrapping_mul(0x0100_0193);
                }
                if !fixed {
                    a_addr = if decrement {
                        a_addr.wrapping_sub(1)
                    } else {
                        a_addr.wrapping_add(1)
                    };
                }
            }

            self.dma[channel][2..4].copy_from_slice(&a_addr.to_le_bytes());
            self.dma[channel][5] = 0;
            self.dma[channel][6] = 0;
            self.dma_events.push(DmaEvent {
                channel: channel as u8,
                source,
                wram_destination,
                length: remaining,
                dmap,
                bbad,
                nonzero_bytes,
                fnv1a,
            });
        }
        if dma_clocks != 0 {
            let cpu_speed = u64::from(self.cpu_cycle_master_clocks);
            dma_clocks = dma_clocks.saturating_add(cpu_speed - dma_clocks % cpu_speed);
            self.dma_master_clocks_pending =
                self.dma_master_clocks_pending.saturating_add(dma_clocks);
        }
    }

    fn hdma_read_table(&mut self, bank: u8, address: u16) -> u8 {
        self.inner
            .read8((u32::from(bank) << 16) | u32::from(address))
    }

    /// Initialize enabled HDMA channels at the start of an SNES frame.
    fn init_hdma(&mut self) {
        self.hdma_finished.fill(false);
        self.hdma_do_transfer.fill(false);
        if self.hdma_channels == 0 {
            return;
        }
        let mut clocks = 8u64;
        for channel in 0..8 {
            self.hdma_do_transfer[channel] = true;
            if self.hdma_channels & (1 << channel) == 0 {
                continue;
            }
            let bank = self.dma[channel][4];
            let mut table = u16::from_le_bytes([self.dma[channel][2], self.dma[channel][3]]);
            let counter = self.hdma_read_table(bank, table);
            table = table.wrapping_add(1);
            clocks += 8;
            self.dma[channel][0x0A] = counter;
            self.hdma_finished[channel] = counter == 0;
            if self.dma[channel][0] & 0x40 != 0 {
                let lo = self.hdma_read_table(bank, table);
                table = table.wrapping_add(1);
                clocks += 8;
                let hi = self.hdma_read_table(bank, table);
                table = table.wrapping_add(1);
                clocks += 8;
                self.dma[channel][5..7]
                    .copy_from_slice(&u16::from_le_bytes([lo, hi]).to_le_bytes());
            }
            self.dma[channel][8..10].copy_from_slice(&table.to_le_bytes());
        }
        self.dma_master_clocks_pending = self.dma_master_clocks_pending.saturating_add(clocks);
    }

    /// Execute one H-blank DMA pass. This follows the SNES two-phase order:
    /// all channel transfers happen first, then every active channel advances
    /// its line counter/table state for the following scanline.
    fn run_hdma_scanline(&mut self) {
        const B_PATTERNS: [&[u8]; 8] = [
            &[0],
            &[0, 1],
            &[0, 0],
            &[0, 0, 1, 1],
            &[0, 1, 2, 3],
            &[0, 1, 0, 1],
            &[0, 0],
            &[0, 0, 1, 1],
        ];
        if self.hdma_channels == 0 {
            return;
        }
        let mut clocks = 8u64;

        for channel in 0..8 {
            if self.hdma_channels & (1 << channel) == 0
                || self.hdma_finished[channel]
                || !self.hdma_do_transfer[channel]
            {
                continue;
            }
            let dmap = self.dma[channel][0];
            let pattern = B_PATTERNS[(dmap & 7) as usize];
            let bbad = self.dma[channel][1];
            let indirect = dmap & 0x40 != 0;
            let bank = if indirect {
                self.dma[channel][7]
            } else {
                self.dma[channel][4]
            };
            let mut source = if indirect {
                u16::from_le_bytes([self.dma[channel][5], self.dma[channel][6]])
            } else {
                u16::from_le_bytes([self.dma[channel][8], self.dma[channel][9]])
            };
            for &offset in pattern {
                let source_address = source;
                source = source.wrapping_add(1);
                if dmap & 0x80 != 0 {
                    let value = self.read_b_bus(bbad.wrapping_add(offset));
                    self.inner
                        .write8((u32::from(bank) << 16) | u32::from(source_address), value);
                } else {
                    let value = self.hdma_read_table(bank, source_address);
                    self.write_b_bus(bbad.wrapping_add(offset), value);
                }
                clocks += 8;
            }
            if indirect {
                self.dma[channel][5..7].copy_from_slice(&source.to_le_bytes());
            } else {
                self.dma[channel][8..10].copy_from_slice(&source.to_le_bytes());
            }
        }

        for channel in 0..8 {
            if self.hdma_channels & (1 << channel) == 0 || self.hdma_finished[channel] {
                continue;
            }
            let mut counter = self.dma[channel][0x0A].wrapping_sub(1);
            self.hdma_do_transfer[channel] = counter & 0x80 != 0;
            let bank = self.dma[channel][4];
            let mut table = u16::from_le_bytes([self.dma[channel][8], self.dma[channel][9]]);
            let next_counter = self.hdma_read_table(bank, table);
            clocks += 8;
            if counter & 0x7F == 0 {
                counter = next_counter;
                table = table.wrapping_add(1);
                if self.dma[channel][0] & 0x40 != 0 {
                    let lo = self.hdma_read_table(bank, table);
                    table = table.wrapping_add(1);
                    let hi = self.hdma_read_table(bank, table);
                    table = table.wrapping_add(1);
                    clocks += 16;
                    self.dma[channel][5..7]
                        .copy_from_slice(&u16::from_le_bytes([lo, hi]).to_le_bytes());
                }
                if counter == 0 {
                    self.hdma_finished[channel] = true;
                }
                self.hdma_do_transfer[channel] = true;
            }
            self.dma[channel][0x0A] = counter;
            self.dma[channel][8..10].copy_from_slice(&table.to_le_bytes());
        }

        self.dma_master_clocks_pending = self.dma_master_clocks_pending.saturating_add(clocks);
    }

    #[inline]
    fn cpu_access_speed(&self, address: u32, address_type: AddressType) -> u8 {
        if address_type == AddressType::Invalid {
            return 6;
        }
        let bank = ((address >> 16) & 0xFF) as u8;
        let offset = address as u16;
        match bank {
            0x40..=0x7F => 8,
            0xC0..=0xFF => {
                if self.fast_rom {
                    6
                } else {
                    8
                }
            }
            _ => match offset {
                0x0000..=0x1FFF => 8,
                0x2000..=0x3FFF => 6,
                0x4000..=0x41FF => 12,
                0x4200..=0x5FFF => 6,
                0x6000..=0x7FFF => 8,
                _ => {
                    if self.fast_rom {
                        6
                    } else {
                        8
                    }
                }
            },
        }
    }

    #[inline]
    fn record_cpu_access(&mut self, address: u32, address_type: AddressType) {
        self.cpu_cycle_master_clocks = self.cpu_access_speed(address, address_type);
    }

    fn advance_ppu_dot(&mut self) {
        // The GSU scheduler accounts for CLSR, opcode fetches, and cache-line
        // refills in SNES master clocks. One nominal PPU dot contributes four.
        self.inner.tick_gsu(4);
        let prev_v = self.cur_v();
        let prev_frame = self.dot / (DOTS_PER_LINE * LINES_PER_FRAME);
        self.dot = self.dot.wrapping_add(1);
        let new_frame = self.dot / (DOTS_PER_LINE * LINES_PER_FRAME) != prev_frame;
        if new_frame {
            self.apu.finish_video_frame(self.master_clock);
            self.ppu.begin_frame();
            self.init_hdma();
        }
        let v = self.cur_v();
        if v != prev_v {
            // HDMA runs in the prior scanline's H-blank and updates registers
            // for the line now becoming visible. SNES scanlines 1..224 map to
            // the 224-line native output.
            if u64::from(prev_v) < VBLANK_START_LINE {
                self.run_hdma_scanline();
            }
            if (1..=FRAME_HEIGHT as u16).contains(&v) {
                self.ppu.render_scanline(usize::from(v - 1));
            }
        }
        let vb = self.in_vblank();
        if vb && !self.prev_vblank {
            self.nmi_latch = true;
        }
        self.prev_vblank = vb;
        if self.irq_enabled && v == self.irq_vtime && prev_v != self.irq_vtime {
            self.irq_pending = true;
            self.irq_line_delivered = false;
        }
    }

    fn advance_master_clocks(&mut self, clocks: u64) {
        let target = self.master_clock.saturating_add(clocks);
        while self.dot < target / 4 {
            self.master_clock = (self.dot + 1) * 4;
            self.advance_ppu_dot();
        }
        self.master_clock = target;
    }

    /// Complete one 65816 microcycle using the address-dependent duration
    /// observed by [`System::read`] or [`System::write`]. DMA time is charged
    /// here as well, while the PPU, SPC, and Super FX continue concurrently.
    pub fn tick_raster(&mut self) {
        let clocks =
            u64::from(self.cpu_cycle_master_clocks).saturating_add(self.dma_master_clocks_pending);
        self.cpu_cycle_master_clocks = 6;
        self.dma_master_clocks_pending = 0;
        self.advance_master_clocks(clocks);
    }

    fn reg_read(&mut self, off: u16) -> Option<u8> {
        if let Some(value) = self.ppu.read(off) {
            return Some(value);
        }
        match off {
            // WMDATA ($2180): read at the 17-bit WMADD pointer, then increment.
            0x2180 => Some(self.read_b_bus(0x80)),
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
                self.irq_line_delivered = false;
                Some(b7)
            }
            // HVBJOY ($4212): bit7 = vblank, bit6 = hblank, bit0 = auto-joypad
            // busy.  Auto-read starts at vblank and remains busy for a short
            // hardware window.  SF2's $03:8F8F synchronizer deliberately
            // waits for both edges (`bit #1; beq`, then `bit #1; bne`), so a
            // permanent "ready" value deadlocks even though simple polling
            // code is satisfied by it.
            0x4212 => {
                let mut v = 0u8;
                if self.in_vblank() {
                    v |= 0x80;
                }
                // hblank: dots outside the active 0..256 region.
                if self.cur_h() >= 274 || self.cur_h() < 1 {
                    v |= 0x40;
                }
                if self.cur_v() as u64 == VBLANK_START_LINE && self.cur_h() < 128 {
                    v |= 0x01;
                }
                Some(v)
            }
            // CPU/APU communication ports. Reads advance the real SPC-700 to
            // the bus's current audio clock before returning the driver value.
            0x2140..=0x2143 => Some(
                self.apu
                    .read_port(self.master_clock, (off - 0x2140) as usize),
            ),
            // JOY1L/JOY1H ($4218/$4219): auto-joypad-read controller-1 state.
            0x4218 => Some(self.pad1 as u8),
            0x4219 => Some((self.pad1 >> 8) as u8),
            _ => None,
        }
    }

    /// Intercept writes to the APU ports to drive the upload-handshake shim.
    fn reg_write(&mut self, off: u16, v: u8) {
        if (0x2100..=0x2133).contains(&off) {
            self.ppu.write(off, v);
            return;
        }
        match off {
            // WMDATA/WMADD ($2180-$2183).
            0x2180 => self.write_b_bus(0x80, v),
            0x2181 => self.wram_port_addr = (self.wram_port_addr & 0x1_FF00) | v as u32,
            0x2182 => self.wram_port_addr = (self.wram_port_addr & 0x1_00FF) | ((v as u32) << 8),
            0x2183 => {
                self.wram_port_addr = (self.wram_port_addr & 0x0_FFFF) | (((v as u32) & 1) << 16)
            }
            // NMITIMEN ($4200): bit7 = NMI enable, bits 5/4 = V/H-IRQ enable.
            0x4200 => {
                self.nmi_enabled = (v & 0x80) != 0;
                self.irq_enabled = (v & 0x30) != 0;
            }
            // MDMAEN: data transfer is synchronous; its master-clock cost is
            // charged after the initiating CPU write.
            0x420B => self.run_dma(v),
            // HDMAEN: channels are initialized automatically at the next
            // frame boundary and transfer during each visible H-blank.
            0x420C => self.hdma_channels = v,
            // MEMSEL: bit 0 enables 6-master-clock accesses in FastROM areas.
            0x420D => self.fast_rom = v & 1 != 0,
            // VTIME ($4209 low / $420A high bit): programmed V-IRQ scanline.
            0x4209 => self.irq_vtime = (self.irq_vtime & 0x100) | v as u16,
            0x420A => self.irq_vtime = (self.irq_vtime & 0x0FF) | (((v as u16) & 1) << 8),
            0x2140..=0x2143 => {
                self.apu
                    .write_port(self.master_clock, (off - 0x2140) as usize, v);
            }
            0x4300..=0x437F => {
                let channel = ((off - 0x4300) >> 4) as usize;
                let register = ((off - 0x4300) & 0xF) as usize;
                self.dma[channel][register] = v;
            }
            _ => {}
        }
    }
}

impl System for RetailBootBus {
    fn read(&mut self, addr: u32, at: AddressType, _s: &Signals) -> u8 {
        self.record_cpu_access(addr, at);
        if at == AddressType::Opcode {
            self.last_opcode_address = addr;
            self.opcode_fetch_count = self.opcode_fetch_count.wrapping_add(1);
        }
        let bank = (addr >> 16) & 0xFF;
        let off = (addr & 0xFFFF) as u16;
        if (bank <= 0x3F || (0x80..=0xBF).contains(&bank)) && (0x2000..0x6000).contains(&off) {
            if let Some(v) = self.reg_read(off) {
                return v;
            }
        }
        self.inner.read8(addr)
    }
    fn write(&mut self, addr: u32, data: u8, at: AddressType, _s: &Signals) {
        self.record_cpu_access(addr, at);
        let bank = (addr >> 16) & 0xFF;
        let off = (addr & 0xFFFF) as u16;
        if (bank <= 0x3F || (0x80..=0xBF).contains(&bank))
            && ((0x2140..=0x2143).contains(&off)
                || (0x2100..=0x2133).contains(&off)
                || (0x2180..=0x2183).contains(&off)
                || off == 0x4200
                || off == 0x4209
                || off == 0x420A
                || off == 0x420B
                || off == 0x420C
                || off == 0x420D
                || (0x4300..=0x437F).contains(&off))
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
        // TIMEUP is a level request: it remains asserted until the interrupt
        // handler acknowledges $4211. `cpu_irq_masked` prevents the CPU-core
        // quirk described above from re-entering while P.I is set.  Do not
        // suppress the level merely because the core sampled it once during a
        // non-interruptible microcycle; doing so loses SF2's frame IRQ and
        // leaves the main loop waiting forever on WRAM $1B92 bit 2.
        let assert = self.irq_enabled && self.irq_pending && !self.cpu_irq_masked;
        if assert {
            self.irq_line_delivered = true;
        }
        assert
    }
}

/// Persistent reset-to-gameplay machine for the retail Star Fox 2 host code.
///
/// Unlike [`boot_retail`], this owns the CPU between calls and advances by
/// complete NTSC video frames.  It is the production bridge used by the HD
/// front end for the still-unlifted title, pilot-select, strategic-map, and
/// mission-lifecycle code while preserving the original cartridge semantics.
pub struct RetailMachine {
    bus: RetailBootBus,
    cpu: CPU,
    cycles: u64,
    cpu_execution_watch: Vec<u32>,
    cpu_execution_hits: Vec<u32>,
    /// Optional single-address WRAM write watch (address, last value hi/lo,
    /// collected (pc, value) hits). Armed from tests to identify writers.
    wram_write_watch: Option<(u32, u16, Vec<(u32, u16)>)>,
}

impl RetailMachine {
    /// Construct a retail machine with the Super FX core enabled.  The first
    /// call to [`Self::tick_video_frames`] consumes the cartridge reset pulse.
    pub fn new(rom: Vec<u8>) -> Self {
        let mut bus = RetailBootBus::new(rom);
        bus.enable_gsu();
        Self {
            bus,
            cpu: CPU::new(),
            cycles: 0,
            cpu_execution_watch: Vec::new(),
            cpu_execution_hits: Vec::new(),
            wram_write_watch: None,
        }
    }

    /// Install oracle-only instruction-entry markers. Hits accumulate until
    /// [`Self::take_cpu_execution_watch_hits`] is called.
    pub fn watch_cpu_execution(&mut self, addresses: &[u32]) {
        self.cpu_execution_watch.clear();
        self.cpu_execution_watch.extend_from_slice(addresses);
        self.cpu_execution_watch.sort_unstable();
        self.cpu_execution_watch.dedup();
        self.cpu_execution_hits.clear();
    }

    /// Drain the watched instruction entries reached since the preceding
    /// sample, preserving execution order and suppressing repeated bus cycles.
    pub fn take_cpu_execution_watch_hits(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.cpu_execution_hits)
    }

    /// Advance `frames` native video frames while presenting one held joypad
    /// state.  Input edges are still derived by the retail code from its own
    /// previous-frame state.
    pub fn tick_video_frames(&mut self, pad1: u16, frames: u32) -> Result<(), String> {
        self.bus.set_pad1(pad1);
        let frame_dots = DOTS_PER_LINE * LINES_PER_FRAME;
        let target = self
            .bus
            .dot
            .saturating_add(frame_dots.saturating_mul(u64::from(frames)));
        while self.bus.dot < target {
            self.tick_cpu_cycle()?;
        }
        Ok(())
    }

    /// Advance until `address` is fetched or the supplied video-frame budget
    /// expires. This is intended for oracle synchronization at semantic
    /// routine boundaries; the shipping port does not depend on it.
    pub fn tick_until_cpu_execution(
        &mut self,
        pad1: u16,
        address: u32,
        max_video_frames: u32,
    ) -> Result<bool, String> {
        self.bus.set_pad1(pad1);
        let frame_dots = DOTS_PER_LINE * LINES_PER_FRAME;
        let target = self
            .bus
            .dot
            .saturating_add(frame_dots.saturating_mul(u64::from(max_video_frames)));
        while self.bus.dot < target {
            if self.tick_cpu_cycle()? == Some(address) {
                return Ok(true);
            }
        }
        Ok(false)
    }


    /// Arm a write-watch on a WRAM address (`$7E:xxxx`). While armed, every
    /// value change records `(pc-after-instruction, value)`.
    pub fn arm_wram_write_watch(&mut self, addr: u32) {
        let cur = self.peek16(0x7E_0000 | (addr & 0xFFFF));
        self.wram_write_watch = Some((addr & 0xFFFF, cur, Vec::new()));
    }

    /// Disarm and return collected (pc, value) pairs.
    pub fn take_wram_write_watch(&mut self) -> Vec<(u32, u16)> {
        match self.wram_write_watch.take() {
            Some((_, _, hits)) => hits,
            None => Vec::new(),
        }
    }

    fn tick_cpu_cycle(&mut self) -> Result<Option<u32>, String> {
        self.bus.set_cpu_irq_masked(self.cpu.p() & 0x04 != 0);
        self.cpu.cycle(&mut self.bus);
        let instruction = (self.cpu.tcu() == 0)
            .then(|| (u32::from(self.cpu.pbr()) << 16) | u32::from(self.cpu.pc().wrapping_sub(1)));
        if let Some(instruction) = instruction {
            if !self.cpu_execution_watch.is_empty()
                && self.cpu_execution_watch.binary_search(&instruction).is_ok()
                && self.cpu_execution_hits.last().copied() != Some(instruction)
            {
                self.cpu_execution_hits.push(instruction);
            }
        }
        if let Some((addr, last, hits)) = self.wram_write_watch.as_mut() {
            let a = 0x7E_0000 | u32::from(*addr);
            let cur =
                u16::from(self.bus.inner.read8(a)) | (u16::from(self.bus.inner.read8(a + 1)) << 8);
            if cur != *last {
                hits.push((
                    (u32::from(self.cpu.pbr()) << 16)
                        | u32::from(self.cpu.pc().wrapping_sub(1)),
                    cur,
                ));
                *last = cur;
            }
        }
        self.bus.tick_raster();
        self.cycles = self.cycles.wrapping_add(1);
        if self.cpu.stopped() {
            return Err(format!(
                "retail CPU stopped at {:02X}:{:04X} after {} cycles",
                self.cpu.pbr(),
                self.cpu.pc(),
                self.cycles
            ));
        }
        Ok(instruction)
    }

    pub fn video_frame(&self) -> u64 {
        self.bus.dot / (DOTS_PER_LINE * LINES_PER_FRAME)
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn master_clock(&self) -> u64 {
        self.bus.master_clock
    }

    pub fn pc(&self) -> u32 {
        (u32::from(self.cpu.pbr()) << 16) | u32::from(self.cpu.pc())
    }

    pub fn peek8(&self, address: u32) -> u8 {
        self.bus.peek8(address)
    }

    pub fn peek16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.peek8(address), self.peek8(address.wrapping_add(1))])
    }

    /// Read a byte of the attached GSU's work RAM (`M_DRAWLIST` lives at
    /// `$70:1960` in GSU space during SF1 gameplay). Returns 0 when no GSU
    /// is installed.
    pub fn peek_gsu_ram(&self, addr: usize) -> u8 {
        match self.bus.inner.gsu_ref() {
            Some(gsu) if addr < gsu.ram.len() => gsu.ram[addr],
            _ => 0,
        }
    }


    /// Snapshot every retail object slot for semantic oracle adapters.
    pub fn object_snapshot(&self) -> Vec<ObjState> {
        snapshot_objects(&self.bus.inner, &RETAIL_POOL)
    }

    /// Walk the retail active-object list and return stable pool-slot indices.
    pub fn active_object_slots(&self) -> Vec<u16> {
        const WORK_RAM: u32 = 0x7E_0000;

        let objects = self.object_snapshot();
        let mut current = self.peek16(WORK_RAM | RETAIL_POOL.active_head);
        let mut active = Vec::new();
        while current != 0 && active.len() < RETAIL_POOL.count as usize {
            let address = u32::from(current);
            let Some(relative) = address.checked_sub(RETAIL_POOL.base) else {
                break;
            };
            if relative % RETAIL_POOL.stride != 0 {
                break;
            }
            let slot = relative / RETAIL_POOL.stride;
            let Some(object) = objects.get(slot as usize) else {
                break;
            };
            active.push(slot as u16);
            current = object.next;
        }
        active
    }

    pub fn ppu_frame(&self) -> PpuFrame {
        self.bus.ppu_frame()
    }

    pub fn ppu_snapshot_rgba(&self) -> Vec<u8> {
        self.bus.ppu_snapshot_rgba()
    }

    pub fn ppu_snapshot_bg_rgba(&self, bg: usize) -> Vec<u8> {
        self.bus.ppu_snapshot_bg_rgba(bg)
    }

    pub fn ppu_snapshot_bg_indices(&self, bg: usize) -> Vec<u8> {
        self.bus.ppu_snapshot_bg_indices(bg)
    }

    /// Clone the native 32 kHz stereo PCM queue consumed by the front end.
    pub fn audio_queue(&self) -> RetailPcmQueue {
        self.bus.apu_pcm_queue()
    }

    pub fn audio_debug_state(&self) -> (u64, usize, [u8; 4], [u8; 4]) {
        self.bus.apu_debug_state()
    }

    /// `(queued, nonzero, minimum, maximum, FNV-1a)` over currently buffered
    /// PCM. This is a read-only health/oracle surface; it does not drain audio.
    pub fn audio_pcm_stats(&self) -> (usize, usize, i16, i16, u64) {
        self.bus.apu.pcm_stats()
    }

    pub fn gsu_plot_count(&self) -> u64 {
        self.bus.inner.gsu_plot_count()
    }

    pub fn gsu_screen_state(&self) -> Option<(u8, u8, u8, u8, u64, u64, u64, u8, u16)> {
        self.bus.inner.gsu_screen_state()
    }

    pub fn watch_gsu_execution(&mut self, pbr: u8, pc: u16) {
        self.bus.inner.watch_gsu_execution(pbr, pc);
    }

    pub fn watch_gsu_execution_with_ram_mask(
        &mut self,
        pbr: u8,
        pc: u16,
        ram_address: u16,
        value: u32,
        mask: u32,
    ) {
        self.bus
            .inner
            .watch_gsu_execution_with_ram_mask(pbr, pc, ram_address, value, mask);
    }

    pub fn gsu_execution_watch_hit(&self) -> bool {
        self.bus.inner.gsu_execution_watch_hit()
    }

    pub fn gsu_execution_watch_state(&self) -> Option<(u64, u32, bool)> {
        self.bus.inner.gsu_execution_watch_state()
    }

    pub fn gsu_execution_watch_values(&self) -> Vec<u32> {
        self.bus.inner.gsu_execution_watch_values()
    }

    pub fn gsu_run_debug_state(&self) -> Option<((u8, u16), (u8, u16, u16), u64, bool, u64)> {
        self.bus.inner.gsu_run_debug_state()
    }

    pub fn gsu_last_run_samples(&self) -> Vec<(u64, u8, u16, u16, u16, u16, u16, u16, u16)> {
        self.bus.inner.gsu_last_run_samples()
    }

    pub fn gsu_last_entry_ram_probe(&self) -> [u16; 4] {
        self.bus.inner.gsu_last_entry_ram_probe()
    }

    pub fn gsu_recent_runs(&self) -> Vec<crate::GsuRunEvent> {
        self.bus.inner.gsu_recent_runs()
    }

    pub fn gsu_first_cd99_entry_ram(&self) -> Option<Vec<u8>> {
        self.bus.inner.gsu_first_cd99_entry_ram()
    }

    pub fn gsu_first_cd99_exit_ram(&self) -> Option<Vec<u8>> {
        self.bus.inner.gsu_first_cd99_exit_ram()
    }

    pub fn gsu_first_cd99_pc_trace(&self) -> Option<Vec<u32>> {
        self.bus.inner.gsu_first_cd99_pc_trace()
    }

    pub fn gsu_first_cd99_register_trace(&self) -> Option<Vec<String>> {
        self.bus.inner.gsu_first_cd99_register_trace()
    }

    pub fn gsu_first_cd99_point_states(
        &self,
    ) -> Option<Vec<(u32, [u16; 16], u16, usize, usize, bool, bool, bool)>> {
        self.bus.inner.gsu_first_cd99_point_states()
    }

    pub fn gsu_first_ce37_entry_ram(&self) -> Option<Vec<u8>> {
        self.bus.inner.gsu_first_ce37_entry_ram()
    }

    pub fn gsu_first_ce37_exit_ram(&self) -> Option<Vec<u8>> {
        self.bus.inner.gsu_first_ce37_exit_ram()
    }

    pub fn gsu_first_ce37_pc_trace(&self) -> Option<Vec<u32>> {
        self.bus.inner.gsu_first_ce37_pc_trace()
    }

    pub fn gsu_first_ce37_register_trace(&self) -> Option<Vec<String>> {
        self.bus.inner.gsu_first_ce37_register_trace()
    }

    pub fn gsu_first_d9ff_register_trace(&self) -> Option<Vec<String>> {
        self.bus.inner.gsu_first_d9ff_register_trace()
    }

    pub fn take_dma_events(&mut self) -> Vec<DmaEvent> {
        self.bus.take_dma_events()
    }

    pub fn timing_debug_state(&self) -> (bool, bool, u16, bool, bool, u16, u16, u8) {
        let (nmi, irq, vtime, pending, masked, h, v) = self.bus.timing_debug_state();
        (nmi, irq, vtime, pending, masked, h, v, self.cpu.p())
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
        bus.set_cpu_irq_masked(cpu.p() & 0x04 != 0);
        cpu.cycle(&mut bus);
        // Advance all concurrent hardware by this address-dependent CPU bus
        // cycle so scanline/vblank waits observe master-clock time.
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

    let (hottest_pc, hottest_hits) = freq
        .iter()
        .max_by_key(|(_, &c)| c)
        .map(|(&a, &c)| (a, c))
        .unwrap_or((0, 0));
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

#[cfg(test)]
mod retail_boot_bus_tests {
    use super::*;

    fn bus() -> RetailBootBus {
        RetailBootBus::new(vec![0; 0x10_0000])
    }

    #[test]
    fn dma_channel_zero_copies_cpu_memory_through_wram_port() {
        let mut bus = bus();
        bus.poke8(0x7E_1000, 0x12);
        bus.poke8(0x7E_1001, 0x34);
        bus.poke8(0x7E_1002, 0x56);

        bus.reg_write(0x2181, 0x00);
        bus.reg_write(0x2182, 0x02);
        bus.reg_write(0x2183, 0x00);
        bus.reg_write(0x4300, 0x00); // A -> B, increment, transfer mode 0
        bus.reg_write(0x4301, 0x80); // B-bus WMDATA ($2180)
        bus.reg_write(0x4302, 0x00);
        bus.reg_write(0x4303, 0x10);
        bus.reg_write(0x4304, 0x7E);
        bus.reg_write(0x4305, 0x03);
        bus.reg_write(0x4306, 0x00);
        bus.reg_write(0x420B, 0x01);

        assert_eq!(bus.peek8(0x7E_0200), 0x12);
        assert_eq!(bus.peek8(0x7E_0201), 0x34);
        assert_eq!(bus.peek8(0x7E_0202), 0x56);
        assert_eq!(&bus.dma[0][2..7], &[0x03, 0x10, 0x7E, 0x00, 0x00]);
    }

    #[test]
    fn real_apu_boots_the_ipl_and_produces_native_pcm() {
        let mut bus = bus();
        let mut ready = false;
        for _ in 0..20_000 {
            if bus.reg_read(0x2140) == Some(0xAA) && bus.reg_read(0x2141) == Some(0xBB) {
                ready = true;
                break;
            }
            bus.tick_raster();
        }
        assert!(
            ready,
            "the real IPL ROM must publish the $BBAA ready signature"
        );

        for _ in 0..(DOTS_PER_LINE * LINES_PER_FRAME) {
            bus.tick_raster();
        }
        let (generated, queued, reads, _) = bus.apu_debug_state();
        assert!(
            generated >= 1000,
            "one video frame should produce stereo PCM"
        );
        assert_eq!(queued as u64, generated);
        assert_eq!(reads[..2], [0xAA, 0xBB]);
    }

    #[test]
    fn irq_stays_pending_while_cpu_interrupt_mask_is_set() {
        let mut bus = bus();
        bus.irq_enabled = true;
        bus.irq_pending = true;
        bus.irq_line_delivered = false;
        bus.set_cpu_irq_masked(true);

        assert!(!System::irq(&mut bus));
        assert!(bus.irq_pending);
        bus.set_cpu_irq_masked(false);
        assert!(System::irq(&mut bus));
        assert!(System::irq(&mut bus), "TIMEUP remains a level until ack");
        bus.set_cpu_irq_masked(true);
        assert!(!System::irq(&mut bus), "P.I masks the pending level");
        assert_eq!(bus.reg_read(0x4211), Some(0x80));
        assert!(!bus.irq_pending);
    }
}
