# Tier-2 Retail Co-Execution — Status

**Goal:** certify the Rust port "100% vs the *retail* Star Fox cart" by running the
retail cart's own per-frame game logic on seeded state and diffing the object
array against the port **tick-for-tick** (not just per-function, which is tier-1).

Ground truth = `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM, repo root, gitignored).
This is a **different binary** from the built ROM (`sf-oracle/data/sf.sfc`), so
every retail address here was re-derived from the retail cart itself.

All harness code lives in `rust/sf-oracle/src/{lib.rs,retail.rs}` +
`rust/sf-oracle/tests/coexec_retail.rs` (**27 tests, all green** — 8 named strats
+ the runtime RNG stream + the FIRST RNG-driven ENEMY strat certified vs retail;
see UPDATE 3, UPDATE 4 and UPDATE 5). The player-relative + RNG state-seeding
frontier is CLEARED (UPDATE 4), and the RNG-driven ENEMY class is now CERTIFIABLE
and certified (UPDATE 5). Nothing committed.

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
| 7 | `parajump_strat` | $04:F851 | player-relative | `player_posx/y`, `PLAYPT`→player Z | worldx, worldy | 90 |

Plus the runtime **RNG stream** (`RANDOM` $02:FC5C) certified bit-exact vs the
port `sf_random` — see UPDATE 4 for the player-pos + RNG state-seeding frontier.

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

## UPDATE 4 — FRONTIER CLEARED: player-relative + RNG state seeding

The hardest tier-2 step is done: the retail machine state a **player-relative**
or **RNG-driven** strat depends on is now LOCATED, SEEDED, and CERTIFIED against
the port. Three new tests (`coexec_retail` now **24, all green**), the seeding
helper `seed_player_relative_state`, and the first player-position-relative
strat certified tick-for-tick vs retail.

### Located globals — retail ↔ port
| Global | Retail WRAM | Built | Port | How located / cross-validated |
|------|------|------|------|------|
| `player_posx` | **$150D** | $1598 | `g.vars.player_posx` | 37 abs reads (built 38); `parajump_strat` `lda $150D` operand |
| `player_posy` | **$150F** | $159A | `g.vars.player_posy` | 34 abs reads (built 32); `parajump_strat` `lda $150F` operand |
| `player_posz` | **$1511** | $159C | `g.vars.player_posz` | 25 abs reads (built 24); leaf `worldz += $1511` ($07:9808) |
| `PLAYPT` (player-obj ptr) | **$1238** | $12C3 | slot 0 (`objs.player()`=`aliens[0]`) | `parajump_strat` `ldy $1238` operand |
| RNG `rand` (SWB state) | **$EF-$F2** (zeropage) | $DE-$E1 | `g.vars.rng: [u8;4]` | masked SWB-skeleton scan (dp operands wildcarded) |
| `RANDOM` / `RANDOM_L` | **$02:FC5C / $02:FC58** | $02:F7BF / $02:F7BB | `common::sf_random` | wrapper `jsr;rtl` is `jsl`-called 288× (the runtime PRNG) |

`player_pos*` are a contiguous x,y,z word triple (identical shape to built);
most `dostrats` globals shifted retail = built − $8B (`player_pos`, `PLAYPT`,
`pviewvelz/posz`), but NOT all (`gameframe` shifted −$85), so each was derived
independently, never by a blanket offset. **KEY DISCOVERY:** retail's runtime
`rand` moved to zeropage **$EF-$F2** (built $DE-$E1) — and that OVERLAPS the
`call` harness's direct-page param block ($F0-$F5), so a RANDOM stream must
carry its SWB state manually (seed $EF directly + inject $F0/$F1/$F2 via the
entry A/X regs each call). `seed_retail_rng` / `retail_random_next` handle this.

### Seeding infrastructure (`retail.rs`)
`seed_player_relative_state(bus, px, py, pz, rng_seed)` writes the three
`player_pos*` mirror globals + the 4-byte `rand` state into retail WRAM. Port
equivalent: `g.vars.player_pos* = …; g.vars.rng = rng_seed;` (+ a live player
object at slot 0 if the strat reads the player's world coords via `PLAYPT`).

### Certified (frontier)
| # | Cert (test) | Status | What it proves |
|---|------|------|------|
| — | `retail_player_rng_globals` | ✅ | Locates + cross-validates all 6 addresses above by signature scan. |
| 7 | `retail_rng_stream_vs_port` | ✅ **MATCH** | Retail `RANDOM` ($02:FC5C) vs port `common::sf_random`, seeded identically: bit-identical streams over 16 draws × 4 seeds (incl. all-0 and the all-$FF fixed point). **RNG seeding infra proven — streams stay in lockstep.** |
| 8 | `retail_parajump_player_relative_vs_port` | ✅ **MATCH** | First PLAYER-POSITION-relative strat. Retail `parajump_strat` ($04:F851) reads `player_posy`/`player_posx` (proportional chases) + `PLAYPT`→player Z (distance gate). Seeded both sides; `worldx`+`worldy` **MATCH over 90 ticks**, converging to the seeded player pos (5000,−3000). Called DIRECTLY (surgical, X=enemy block) so seeded `player_pos*` survives (a full `dostrats` walk would rerun the player strats that recompute them). |

### Generalized method — certifying a player-relative / RNG strat vs retail
1. **Read the port strat's footprint** (`sf-strat/src/*.rs`): which `player_pos*`
   it reads, whether it draws RNG (and how many draws, in what order), whether
   it reads the live player object (→ needs `PLAYPT` + a seeded slot-0/slot-1
   object). Pure-integer player-pos strats are easiest; trig/aim (`angle_xz`)
   and GSU-in-tick are harder.
2. **Locate the retail body** by masked scan; read its `lda player_pos*` / `ldy
   PLAYPT` / `jsl RANDOM_L` operands back out to self-validate the addresses.
3. **Seed both sides identically** with `seed_player_relative_state`; if the
   strat reads the player object, `wram_write16(PLAYPT, block)` + seed that block.
4. **Call the strat DIRECTLY** (surgical, `call(bus, addr, Entry{x:block})`) —
   NOT the full `dostrats` walk — so the seeded `player_pos*` is not clobbered by
   the frame's player strats. Diff object fields per tick. For an RNG strat,
   carry the `rand` state across the harness param-block collision.

### GAP MAP — the RNG lane WAS only HALF wired (RESOLVED — commit f280388, certified UPDATE 5)
> **STATUS: FIXED + CERTIFIED.** Commit f280388 rewired all 61 enemy/boss RNG
> sites `ea_random` → `sf_random`. UPDATE 5 below certifies the enemy lane
> (`firepillar`) against the retail cart, both coin branches. The finding as
> originally written:

The port had **two** RNG implementations, and the enemy lane was on the WRONG one:
- `common::sf_random(&mut g.vars.rng)` = the correct 4-byte **SWB chain** (proven
  == retail here). Used by the boss/player lanes (commit 67a4524).
- `enemy_a::ea_random(g)` = the OLD **build-time LCG** (`rnd*91+$61D7`) over a
  separate compat-WRAM slot (`RNDVAL`), NOT the runtime PRNG. **Every enemy-lane
  RNG-driven strat still calls `ea_random`** (e.g. `firepillar_init` draws 3,
  `mother` 4, `volrock`/`player` spread strats) — so those would DESYNC from
  retail's SWB stream. This is the precise remaining gap for RNG-driven ENEMY
  strats: swap `ea_random` → `sf_random` (over `g.vars.rng`) at those call sites,
  then re-certify with the now-proven RNG seeding infra. (The seeding + stream
  infra is done; the port-side rewire is a sf-strat change, out of scope here.)

## UPDATE 5 — RNG-DRIVEN ENEMY CLASS CERTIFIED: `firepillar` vs retail (closes the fix)

The `ea_random`→`sf_random` fix (commit f280388) is now proven end-to-end
against the cartridge. The FIRST RNG-driven ENEMY strat, `firepillar`, is
certified vs retail — both coin branches — extending tier-2 coverage to the
RNG-driven enemy class. Three new tests (`coexec_retail` now **27, all green**).

`firepillar_Istrat` (retail **$0A:DAE4**, GA2STRAT.ASM:2039-2062) draws the
runtime RNG THREE times on init and reads the player-X mirror:
- DRAW 1 → `al_worldx` low byte; DRAW 2 `& 3` → high byte ⇒ `worldx = d1|((d2&3)<<8)` (0..1023)
- `worldx += -512 + (player_posx asra 1)` (signed `>>1`)
- DRAW 3 coin `cmp #$B2 (178)` → 30% (rnd≥178) latches `al_sflags2` bit **$20** ("inert"); 70% leaves it clear.

Port ↔ `sf_strat::enemies_ground::firepillar_init` (IS_FIREPILLAR row 193), whose
three `sf_random(&mut g.vars)` calls ARE the just-fixed enemy-lane sites.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_firepillar_addresses` | ✅ | Masked-scans `firepillar_Istrat`=**$0A:DAE4** (UNIQUE, 99-byte skeleton from built $0A:DABE, +$26 shift). Reads the operands back: all THREE `jsl` draws land on **RANDOM_L $02:FC58** (the routine `retail_rng_stream_vs_port` proved == port `sf_random`), the `lda` reads **player_posx $150D**, the coin is `cmp #$B2` (178), and the `jml` fall-through = `firepillar_strat`=**$0A:DB47** (Istrat+$63). Confirms the exact RNG-draw sequence + global read the port consumes. |
| `retail_firepillar_rng_vs_port` | ✅ **MATCH** | Draws firepillar's 3-value sequence from the cart's OWN `RANDOM` (carried across the param-block collision by `retail_random_next`), applies the cross-validated formula → cartridge-faithful `(worldx, inert)`, and diffs the PORT's real `firepillar_init` (the fixed `sf_random` site) on the SAME seed. Two seeds drive both coin branches (`[1,2,3,4]`→active, `[171,205,239,18]`→inert); retail and port take the SAME branch and same worldx each time. Direct proof the fix is cartridge-faithful. |
| `retail_firepillar_body_vs_port` | ✅ **MATCH** (GOLD) | Runs the retail cart's OWN `firepillar_Istrat` body ($0A:DAE4) — 3 real `jsl RANDOM_L` draws — on seeded RNG + player_posx, and diffs `(worldx, inert)` vs the port. Both branches MATCH (`[200,1,2,54]`→active worldx=-1558, `[99,88,77,54]`→inert worldx=-1977). This is the strongest form: the actual cartridge enemy AI == the port. |

### Seeding an RNG strat body vs retail — the param-block collision, solved
Running an RNG strat body surgically hits the documented `rand`($EF-$F2) ↔ `call`
param-block ($F0-$F5) overlap. The strat needs **X = object block**, which PINS
`$F2 = rand[3] =` the block's low byte ($36 for pool base $0336). So: seed
`rand[0]`(@$EF, below the block) directly; ride `rand[1..3]` in via `entry.a`
($F0/$F1) and `entry.x`-low ($F2); pick seeds whose 4th byte = the block low byte
($36 = 54). The first 3 state bytes stay free — enough to drive each coin branch.
A distant player (via `PLAYPT`) keeps the fall-through `firepillar_strat` tick
(no RNG) a clean no-op so it never perturbs worldx/sflag2. (The draw-sequence
cert `retail_firepillar_rng_vs_port` sidesteps the pin entirely via
`retail_random_next`, so it can use any seed.)

### CERTIFIED VS RETAIL — running total: **8 named strats** + RNG stream + RNG-enemy
| # | Strat | Retail addr | Kind | RNG draws | Certified fields |
|---|------|------|------|------|------|
| 8 | `firepillar_Istrat` | $0A:DAE4 | **RNG-driven enemy** | 3× `RANDOM_L` | worldx (draws 1&2 + player_posx), inert sflag2 (draw 3) — both coin branches |

Newly-located retail: `firepillar_Istrat`=$0A:DAE4, `firepillar_strat`=$0A:DB47,
`asf_sflag2`=`al_sflags2` bit $20. Reused: `RANDOM_L`=$02:FC58, `player_posx`=$150D,
`set_0collptrsx_l`=$1F:D450 (the firepillar init's coll-ptr-zero leaf).

### The RNG-driven enemy class is now CERTIFIABLE
The infrastructure (RNG stream lockstep + the param-block-collision seeding
recipe above) generalizes to the other rewired enemy RNG strats — `volrockdown`
(3 draws: vx/vy/vz scatter), `mother` (4 draws), the `volrock`/player spread
strats, etc. Each is: locate by masked scan (skeleton from the built ROM,
RANDOM_L operands wildcarded), seed the stream both sides, diff the RNG-derived
kinematic fields. `firepillar` is the template.

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
