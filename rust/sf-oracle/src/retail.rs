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
/// enemy1. Port ↔ `enemies_ground::mine0_init` (IS_MINE0=246).
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
pub fn seed_player_relative_state(
    bus: &mut SnesBus,
    px: i16,
    py: i16,
    pz: i16,
    rng_seed: [u8; 4],
) {
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
