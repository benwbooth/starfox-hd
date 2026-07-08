# Tier-2 Retail Co-Execution — Status

**Goal:** certify the Rust port "100% vs the *retail* Star Fox cart" by running the
retail cart's own per-frame game logic on seeded state and diffing the object
array against the port **tick-for-tick** (not just per-function, which is tier-1).

Ground truth = `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM, repo root, gitignored).
This is a **different binary** from the built ROM (`sf-oracle/data/sf.sfc`), so
every retail address here was re-derived from the retail cart itself.

All harness code lives in `rust/sf-oracle/src/{lib.rs,retail.rs}` +
`rust/sf-oracle/tests/coexec_retail.rs` (**21 tests, all green** — 6 named strats
certified vs retail; see UPDATE 3). Nothing committed.

## UPDATE — the FULL retail `dostrats` per-frame strat tick now runs (and diffs)

The three blockers below are cleared. We now run the retail cart's **entire
per-frame strat pipeline** (`dostrats` = `init_strats_l` + `update_objects_l` +
active-list walk + `do_strat_l` dispatch) on directly-seeded state, with the GSU
reachable through the RAM trampoline, and diff the object array against the port
tick-for-tick.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `gsu_trampoline_runs_from_ram` | ✅ | Injects the real 35-byte `runmario_l` into WRAM (from its ROM copy-source) and **calls the RAM trampoline itself** (`A`=bank,`X`=PC) — the CPU runs the RAM wait-loop, its `stx mr15` kicks the GSU via the bus, and the ROM `mcrotmatzxy16` program returns the identity matrix. `gsu_kicks==1`. Blocker 1 cleared. |
| `retail_strat_pipeline_addresses` | ✅ | Locates `dostrats` @ **$02:DAF2** by masked signature scan (1 hit) and reads its embedded operands back out — auto-deriving `init_strats_l`=$06:81D5, `update_objects_l`=$03:ED7E, `do_strat_l`=$1F:D26B, plus `allst`/`aldead`/`gameframe`. Cross-validated: `dostrats`'s `ldx allst` operand = **$121D**, byte-identical to the pool `active_head` derived independently from the allocator scan. Blocker 2 cleared. |
| `retail_dostrats_pipeline_runs` | ✅ | Seeds the pool + one object on `allst`, installs the trampoline, and runs the REAL retail `dostrats` ($02:DAF2). After the tick: `gameframe` +1, the object survives, and `stratobj_posx/y/z` (written only by `do_strat_l` from `al_worldx/y/z,x`) hold the object's seeded coords — the whole pipeline executed on retail code without trapping. |
| `retail_dostrats_dispatch_vs_port` | ✅ **MATCH** | Sets the object's own `al_stratptr` ($16/bank $18) = retail `addalvecs_l` ($1F:C7BB), so `dostrats -> do_strat_l` resolves + RTL-dispatches into it exactly as it dispatches a real enemy strat. Diffs the object array vs the port (`strat_apply_velocity`) per field per tick: **MATCH over 8 ticks** — the full retail dispatch machine (allst walk + `do_strat_l` pointer resolution + strat execution + write-back) evolves the object identically to the port. |

### Retail strat-pipeline addresses located (all cross-validated)
| Routine | Retail | Built | How |
|------|------|------|------|
| `dostrats` (near) | $02:DAF2 | $02:D6DE | masked scan, 1 hit |
| `do_strat_l` | $1F:D26B | $1F:D283 | JSL operand in `dostrats`; opcode skeleton matches built |
| `init_strats_l` | $06:81D5 | $02:81CC | JSL operand in `dostrats` (note: retail moved it to bank $06) |
| `update_objects_l` | $03:ED7E | (JSL) | JSL operand in `dostrats` |
| `mapobjdo` (spawn VM) | $03:F79B | $03:EB80 | masked scan, 5-member family, all reuse `ldx allst=$121D` |
| `newobjex` / `newobjs_l` | $03:EDAB / $03:EDA1 | $03:E188 / $03:E17E | masked scan, 1 hit |
| `runmario_l` (ROM copy) | $02:9D56 | $02:9D32 | `sta.l $003034` anchor; byte-identical but `mario_draw_mode` operand |
| `runmario_l` (RAM dest) | $7E:4EE9 | $7E:4F51 | most-common bank-$7E JSL target (63 sites); intra-block +$27/+$6C sub-entries line up with built |

Auto-derived retail strat globals: `gameframe`=$15BB, `aldead`=$1248,
`dummyobj`=$156B, `stratobj_posx/y/z`=$1513/15/17, `al1pt`=$123A,
`mario_draw_mode`=$1260. Struct offset `al_stratptr`=$16 (low word) / $18 (bank),
verified against `al_HP`=$2A and `al_vx`=$2F.

## UPDATE 2 — the FIRST NAMED enemy strat is now certified vs retail

`retail_dostrats_dispatch_vs_port` dispatched `addalvecs_l` (a synthetic
pure-motion strat) through the genuine dispatch path. We have now certified TWO
**real, named** ground enemy strats — `stayrelhard180YR_strat` and
`stayrel_strat` — both surgically AND (for the first) through the full retail
`dostrats` frame, tick-for-tick MATCH vs the port.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_stayrel_family_addresses` | ✅ | Masked-scans + cross-validates the whole family: `sr_addplayerZx`=**$1F:DC69** (the ONE of 8 skeleton matches that is actually `jsl`-referenced — 247 refs, 97 of them `jsl X;rtl` pure-scroll strat bodies), reads its `adc` operand to derive `pviewvelz`=**$14F4**; `stayrel_strat`=**$06:864B** (UNIQUE masked hit) whose `sta` operand pins `al_sflags2`=**$1E** and `ora #$01` confirms `colldisable` = sflag bit 8; `stayrelhard180YR_strat`=**$06:8646** = the pure-scroll body immediately preceding it. |
| `retail_stayrelhard180yr_body_is_jsl_addplayerz` | ✅ | Reads the 5 body bytes of `stayrelhard180YR_strat` out of retail ROM (LoROM): `22 69 DC 1F 6B` = `jsl sr_addplayerZx; rtl`, and a one-tick call advances `worldz` by exactly `pviewvelz`. |
| `retail_stayrelhard180yr_strat_vs_port` | ✅ **MATCH** | Runs the retail cart's OWN `stayrelhard180YR_strat` body ($06:8646) each tick on a seeded object and diffs `worldz` vs the port's `sf_strat::ground` stayrelhard180yr per-tick strat. **MATCH over 60 ticks**, two scenarios including a 16-bit `worldz` wrap (pz=-30000, pvz=-1000 wraps past -32768 identically both sides). |
| `retail_stayrel_strat_vs_port` | ✅ **MATCH** | `stayrel_strat` ($06:864B) = scroll + set `colldisable`. `worldz` MATCH over 40 ticks; each side sets ITS OWN `colldisable` bit (retail `al_sflags2` bit $01 ↔ port `al_sflags` bit $10 — a representation remap, see below). |
| `retail_stayrelhard180yr_dispatch_vs_port` | ✅ **MATCH** | The STRONGEST claim: points an object's `al_stratptr` at the real `stayrelhard180YR_strat` ($06:8646) and runs the ENTIRE retail `dostrats` frame each tick. **MATCH over 8 frames**, and `pviewvelz` survives every frame (-200→-200) — proving nothing in `init_strats_l`/`update_objects_l` clobbers a directly-seeded `pviewvelz` when no player strat runs. |

### Certified strat #1/#2 — global footprint map (`stayrel` family)
The `stayrel`/`stayrelhard180yr` per-tick body is the smallest possible: its
whole effect is `jsl sr_addplayerZx; rtl`.

| What the strat touches | Retail address | Port | Notes |
|------|------|------|------|
| `pviewvelz` (READ) | WRAM **$14F4** | `g.vars.pviewvelz` | The ONE global. Written only by PLAYER strats; seed it once, it survives a frame. |
| `al_worldz` (RW) | struct **+$10** | `Alien::worldz` | 16-bit wrapping add; directly diffable, byte-identical both sides. |
| `al_sflags2` (RW, `stayrel` only) | struct **+$1E**, bit **$01** | `Alien::sflags` bit **$10** | `colldisable`. NOT raw-diffable: the port's C `obj.h` uses a different sflag bit layout than the ASM (`STRATEQU.INC`). Each side correctly sets its own `colldisable`; the mapping is retail `sflags2:$01` ↔ port `sflags:$10`. |

`sr_addplayerZx` = $1F:DC69 (leaf); `stayrelhard180YR_strat` = $06:8646;
`stayrel_strat` = $06:864B. Struct offsets `al_sflags`=$1D / `al_sflags2`=$1E.

### Reusable recipe — certifying the NEXT named strat vs retail
1. **Read the port strat** (`sf-strat/src/*.rs`) and enumerate its footprint:
   which globals it READS (each = one retail WRAM address to locate) and which
   `al_*` struct fields it writes (offsets are identical retail↔built↔port).
   Prefer strats with the fewest globals (ground/scroll strats read only
   `pviewvelz`; movers add `al_vx/vy/vz`; homing strats add player pos + RNG).
2. **Masked-scan the strat body + its leaf routines.** The strat body is often a
   couple of `jsl <leaf>` calls + a few field sets. Scan the ROM for the opcode
   skeleton with absolute/global operands WILDCARDED, then (a) read the operands
   back to derive the retail global addresses, and (b) DISAMBIGUATE duplicate
   skeleton hits by reference count — the genuine callable leaf is the one that
   is `jsl`-referenced (inlined motifs have zero refs). A UNIQUE masked hit (like
   `stayrel_strat`) also anchors adjacent routines by address order.
   Helpers `masked_scan` / `rom_off_to_snes` / `snes_to_rom_off` live in the test.
3. **Seed BOTH sides identically.** Retail: fresh `SnesBus`, `wram_write16` the
   read-globals + the object's fields at `RETAIL_POOL.base + slot*stride + off`.
   Port: `Game::new()`, `sf_strat::<mod>::install(&mut g)`, `objs.alloc()`, set
   the same fields + `g.vars.<global>`, run the Istrat once to arm `stratptr`.
4. **Tick + diff.** Surgical: `call(&mut bus, <strat_addr>, Entry{x: blk, p:0})`
   per tick; port `g.call_strat(tick, idx)`; diff the numeric object fields
   (skip sflag/flag BYTES — the port's bit layout differs; compare semantic bits
   individually). Full-pipeline (stronger): set `al_stratptr`=strat_addr, put the
   object on `allst`, run `RETAIL_DOSTRATS` per frame — but first confirm the
   strat's globals SURVIVE a `dostrats` frame (they do for `pviewvelz`; a global
   that a player/camera routine recomputes each frame would need re-seeding or a
   surgical-only diff).

## UPDATE 3 — BATCH 2: four MORE named strats certified vs retail

Applying the UPDATE-2 recipe to the next batch. **Six new tests, all green**
(`coexec_retail` now 21 tests). Four more real, named strats certified
tick-for-tick vs the retail cart, spanning three *new* footprint shapes.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_batch2_ground_addresses` | ✅ | Masked-scans `staydist_Istrat`=**$06:8656** (UNIQUE) — its `adc` operand gives `pviewposz`=**$14FA**, cross-validated as `pviewvelz`($14F4)+6 (identical +6 spacing as built $157F→$1585) and by adjacency (`stayrel_strat`$864B + 11-byte body = $8656). Also `gnd_Istrat`=**$08:F15D** (UNIQUE), whose `jsl` operand gives `set_0collptrsx_l`=$1F:D450. |
| `retail_staydist_strat_vs_port` | ✅ **MATCH** | Runs the retail `staydist_Istrat` body ($06:8656) each tick: `al_worldz = al_sword1 + pviewposz` (viewer-tracking, idempotent). worldz MATCH over 40 ticks × 2 scenarios, incl. a 16-bit wrap AND a **mid-run `pviewposz` change on both sides** (proving worldz tracks the global, not a frozen one-shot). colldisable set each side (retail sflags2 bit$01 ↔ port sflags bit$10). |
| `retail_gnd_strat_vs_port` | ✅ **MATCH** | INIT-ONLY strat. Seeds a DIRTY object, runs retail `gnd_Istrat` ($08:F15D): zeroes `al_stratptr` (per-tick becomes a no-op), `jsl set_0collptrsx_l`, sets `al_type\|=gnd($01)` + colldisable. Semantic MATCH vs port `strat_gnd_init` (both zero stratptr/coll/exp ptrs, both flag ground + colldisable). |
| `retail_batch2_rotate_mover_addresses` | ✅ | Masked-scans `hardrot_strat`=**$06:8614** (UNIQUE; pure struct-offset, byte-identical retail/built like `addalvecs_l`) and `straight_Istrat`=**$0B:8CE1** (UNIQUE full-signature scan). Derives `straight_strat`=**$0B:8D00** via the +31 fall-through offset, **self-cross-validated** because the Istrat's own `s_set_strat` operand equals that derived address. |
| `retail_hardrot_strat_vs_port` | ✅ **MATCH** | Pure spin-in-place scenery: `al_rot{x,y,z} += al_sbyte{1,2,3}` (8-bit). rotx/y/z MATCH over **300 ticks** (full 8-bit wrap on every axis). ZERO globals, ZERO RNG — the simplest non-scroll footprint. (Harness note: called with `p=$20` — 8-bit A / 16-bit X — because the body is a mid-strat fragment that assumes `s_start_strat`'s `shorta` and does no rep/sep of its own.) |
| `retail_straight_strat_vs_port` | ✅ **MATCH** | The canonical fixed-velocity MOVER: `al_worldx/y/z += al_vx/vy/vz` (addalvecs) then `al_worldz += pviewvelz` (scroll). vx/vy/vz seeded DIRECTLY (bypassing the Istrat's one-shot `gen_3dvecs` → no GSU needed). worldx/y/z MATCH over 30 ticks incl. a 16-bit worldx wrap. Port equiv = `strat_apply_velocity` ∘ scroll — exactly `straight_strat`'s two `jsl`s. |

### CERTIFIED VS RETAIL — running list (6 named strats, tick-for-tick MATCH)
| # | Strat | Retail addr | Kind | Global footprint | Struct fields | Ticks |
|---|------|------|------|------|------|------|
| 1 | `stayrelhard180YR_strat` | $06:8646 | scroll | `pviewvelz` | worldz | 60 |
| 2 | `stayrel_strat` | $06:864B | scroll+flag | `pviewvelz` | worldz, sflags2 | 40 |
| 3 | `staydist_Istrat` | $06:8656 | view-track | `pviewposz` | worldz, sword1, sflags2 | 40 |
| 4 | `gnd_Istrat` | $08:F15D | init-only | *(none)* | stratptr, type, sflags2 (+coll/exp) | 1 |
| 5 | `hardrot_strat` | $06:8614 | rotate | *(none)* | rotx/y/z, sbyte1/2/3 | 300 |
| 6 | `straight_strat` | $0B:8D00 | mover+scroll | `pviewvelz` | worldx/y/z, vx/vy/vz | 30 |

Plus the synthetic-but-dispatched `addalvecs_l` motion integrator and the full
`dostrats` pipeline (UPDATE 1). **Newly-located retail globals/offsets:**
`pviewposz`=$14FA; struct offsets `al_type`=$09, `al_rotx/y/z`=$12/$13/$14,
`al_sbyte1/2/3`=$22/$23/$24, `al_sword1`=$26 (all struct-offsets identical
retail↔built↔port). Leaf `set_0collptrsx_l`=$1F:D450.

### Batch-2 footprint maps
- **`staydist`** (view-tracking ground): reads `pviewposz`($14FA) + `al_sword1`
  ($26); writes `al_worldz`(+$10) + `al_sflags2`(+$1E bit$01). Body is
  IDEMPOTENT (`worldz = sword1 + pviewposz` re-derived each tick), so it TRACKS
  a changing global rather than accumulating — the mid-run pviewposz swap proves
  the read dependency. Second global beyond `pviewvelz`, same "survives a frame"
  property (player/camera-written only).
- **`gnd`** (static ground plane): reads NOTHING; writes `al_stratptr`(=0),
  `al_type`(\|=1), `al_sflags2`(\|=colldisable), + extended-array `collstratptr`/
  `expstratptr`(=0 via the leaf). INIT-ONLY — per-tick is a no-op. The extended
  coll/exp pointers live in the *separate* `xalblks` array (not the main pool);
  the leaf write lands in valid zeroed WRAM so the call never traps, but that
  array's retail base is not mapped — cert covers the main-pool observables +
  the port's `Option::None` clears.
- **`hardrot`** (spin scenery): reads/writes `al_rotx/y/z`($12/$13/$14), reads
  `al_sbyte1/2/3`($22/$23/$24). ZERO globals — the cleanest footprint yet.
- **`straight`** (fixed-velocity mover): reads `al_vx/vy/vz`($2F/$31/$33) +
  `pviewvelz`; writes `al_worldx/y/z`. The Istrat's `gen_3dvecs` (GSU) sets the
  velocity ONCE; the per-tick body is pure CPU move+scroll, so seeding vx/vy/vz
  directly sidesteps the GSU entirely.

### Remaining blockers for HARDER strats (beyond `stayrel`)
- **RNG-driven strats** (dodge/aim jitter): need the retail RNG state global
  located + seeded, and the runtime SWB chain (see commit 67a4524) matched.
- **Player/camera-relative strats** (homing, `staydist` reads `pviewposz`):
  need those globals located + seeded; some are recomputed inside `dostrats`
  (player strats write `pviewvelz`/`pviewposz`), so a full-pipeline diff would
  require a live player object — surgical per-strat diff sidesteps this.
- **sflag/flag raw diffs are off the table** — the port re-derived its own
  `obj.h` bit layout; certify sflag EFFECTS bit-by-bit (semantic), not byte==byte.

The historical blocker analysis below is retained for context.

---


## What works end-to-end (the first retail-vs-port per-tick diff)

Test `retail_vs_port_per_tick_object_diff` runs the full loop for one scenario:

| Step | Status | How |
|------|--------|-----|
| 1. GSU wired into the bus | ✅ | `SnesBus::enable_gsu()` maps the GSU registers $3000-$303F (banks $00-3F/$80-BF). Writing R15-high ($301F) kicks `gsu::Gsu` from `pbr:R15`, sharing bank-$70 RAM in/out; the SFR poll ($3030 bit 5) falls through when the chip STOPs — exactly the `runmario_l` protocol (`sta m_pbr; stx mr15; .wait lda m_sfr; and #$20; bne .wait`). |
| 2. Object pool seeded | ✅ | Retail's OWN allocator (`init_object_pool` → `$02:F4C9 FmtFreeLst`) formats the 70-block free-list; the harness pops 3 blocks and builds an `allst` active list at the retail stride, each block carrying shape + world pos + velocity. |
| 3. Tick updates objects | ✅ | Each frame, walk the retail active list and run the **real retail** motion integrator `addalvecs_l` (`$1F:C7BB`) on every live block, then `snapshot_objects` the whole pool. Object worldz scrolled 8000 → 2000 over 30 ticks (Δ = vz·N), proving retail game code advanced the seeded state. |
| 4. Diff vs the port | ✅ **MATCH** | Same 3 objects set up as `sf_game::Alien` + `sf_strat::common::strat_apply_velocity`, ticked in lockstep. worldx/y/z + shape compared per slot per tick: **MATCH for all 3 slots over 30 ticks**, including a 16-bit X-wrap case (32000 + 1000·30 wraps identically in both). |

### GSU wiring proof
`gsu_kicks_through_bus_registers` drives a real ROM GSU program
(`mcrotmatzxy16`, `$01:8295`) entirely through CPU-visible register writes — no
direct `Gsu::run` — and reads the 3×3 identity matrix back out of shared RAM.
`gsu_kicks == 1`.

### Retail address derivation (no built-ROM symbols apply to retail)
`RETAIL_ADDALVECS_L = $1F:C7BB` was found by an **exact byte-signature scan**
(`c2 20  b5 0C 18 75 2F 95 0C  b5 0E 18 75 31 95 0E  b5 10 18 75 33 95 10  e2 20
6b`). The pattern is byte-identical in the built ROM (its `ADDALVECS_L` is 0x18
bytes later at `$1F:C7D3`) because it touches only the world/velocity **struct
offsets** ($0C/$0E/$10, $2F/$31/$33), which are proven identical across carts.
Exactly one occurrence in retail → this is the genuine retail routine.

## What is NOT yet done — the precise blocker for the FULL per-frame tick

The scenario above ticks objects through `addalvecs_l`, the CPU-only motion
integrator every strat applies each frame. Driving the **entire** retail
`dostrats` loop (per-object AI, spawns, 3D) on seeded state is blocked on:

1. **`runmario_l` is a RAM-resident trampoline.** The CPU reaches the GSU through
   `runmario_l`, which the boot copies into WRAM (built `$7E:4F51`). Calling
   `dostrats` via the `call()` harness on a non-booted bus would `JSL` into empty
   RAM. Fix: either (a) locate + inject the retail `runmario_l` stub into WRAM,
   or (b) intercept the `JSL` to that RAM address in `SnesBus` and route straight
   to `gsu_kick()`. The chip itself is now wired; only this trampoline is missing.

2. **Per-strat AI needs routine addresses re-derived.** `dostrats`,
   `do_strat_l`, `update_objects_l`, `newobjs`/`mapobjdo` (the map-bytecode spawn
   VM) must each be located in retail by signature scan. Only the allocator and
   `addalvecs_l` are located so far. `addalvecs_l` was easy because it uses pure
   struct offsets; routines that touch WRAM globals (whose addresses shifted
   between carts) need more careful signature work.

3. **Cold-boot → live gameplay needs input injection.** `boot_retail` marches
   the real cart ~230 frames into the main loop but parks in the attract/
   forced-blank path (`$00:8164`/`$00:8199`) with no gameplay objects (the 5
   "objects" seen have zero world coords — GSU-less attract garbage). Reaching a
   real level-start spawn from reset additionally needs auto-joypad input
   (`$4218/9`; `RetailBootBus::set_pad1` exists) to leave attract. The surgical
   path here (seed the pool directly, drive the tick) sidesteps this and is the
   recommended route.

## Precise summary (historical — superseded by the UPDATE at the top)

At the time this section was written: we could run the allocator + motion
integrator and diff tick-for-tick, but not the full `dostrats` tick. **All three
blockers below are now cleared** (see the UPDATE section at the top): the
`runmario_l` trampoline is injected into WRAM and verified to drive the GSU; the
`dostrats`/`do_strat_l`/`init_strats_l`/`update_objects_l`/`mapobjdo`/`newobjs`
retail addresses are located by masked signature scan and cross-validated; and
the full retail `dostrats` per-frame tick runs on seeded state and diffs MATCH
against the port through the real `do_strat_l` dispatch path.

## Recommended next steps (remaining)
1. ✅ **DONE — Certify named enemy strats.** 6 certified vs retail:
   `stayrelhard180YR`/`stayrel` (UPDATE 2) + `staydist`/`gnd`/`hardrot`/
   `straight` (UPDATE 3), covering scroll / view-track / init-only / rotate /
   fixed-velocity-mover footprints. The MOVER (`al_vx/vy/vz` + `pviewvelz`) is
   `straight_strat`. Next frontier: an RNG or player-relative strat — the harder
   blockers (RNG state, player-recomputed globals, GSU-in-the-tick) are itemised
   below.
2. **Exercise the spawn VM**: hand retail `mapobjdo` ($03:F79B) / `newobjs_l`
   ($03:EDA1) a minimal map script so retail SPAWNS objects (instead of
   hand-seeding), then diff the spawn output vs the port map builder.
3. **Re-certify the pure per-strat helpers vs retail** surgically (like
   `addalvecs_l`): `gen_vecs`, `speed_to`, `perc*`, `xzdiffs` — all struct-offset/
   pure-math, so byte-identical in retail and quick to scan + diff.
