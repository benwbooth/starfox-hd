# Tier-2 Retail Co-Execution — Status

**Goal:** certify the Rust port "100% vs the *retail* Star Fox cart" by running the
retail cart's own per-frame game logic on seeded state and diffing the object
array against the port **tick-for-tick** (not just per-function, which is tier-1).

Ground truth = `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM, repo root, gitignored).
This is a **different binary** from the built ROM (`sf-oracle/data/sf.sfc`), so
every retail address here was re-derived from the retail cart itself.

All harness code lives in `rust/sf-oracle/src/{lib.rs,retail.rs}` +
`rust/sf-oracle/tests/coexec_retail.rs` (**63 tests, all green** — **FIVE BOSSES**
certified vs the cart: `boss8` (INIT + 4-child spawn + common per-tick state
machine, UPDATE 12), `boss2` (INIT + 9-child spawn + the state-0 wait state
machine), and `bossg`/`bossseamon`/`boss1` (INIT, UPDATE 13); plus the player-move
**plrot\* accumulator** (UPDATE 12); plus
**15 named strats** + the **PLAYER-MOVEMENT** physics (screen-edge BOUNDS clamp +
boost/brake speed ramp, UPDATE 11) + the **GSU-per-tick AIMING CLASS** (aim angle via the live GSU + aim
velocity + fire-gate timing, UPDATE 8) + the **PROJECTILE-SPAWN + TARGET-SEARCH
machinery** (UPDATE 9) + the **COLLISION SYSTEM** (do_coll response + box-overlap
math + colltype allow-matrix, UPDATE 10) + the runtime RNG stream + the RNG-driven
ENEMY class + the `break_meteorT` death coin, all certified vs retail; see
UPDATE 3-10). The
player-relative + RNG state-seeding frontier is CLEARED (UPDATE 4), the
RNG-driven ENEMY class is CERTIFIED (UPDATE 5), BATCH 3 (UPDATE 6) adds a
STATIC-init scenery strat + three more RNG-driven INIT strats, BATCH 4
(UPDATE 7) adds a zdist state-transition MOVER (`woods`), an RNG + PLAYER-RELATIVE
scenery init (`tree2`), an RNG-reroll firing-enemy init (`shou0`), and the
`break_meteorT` 50% tadpole death coin, and UPDATE 9 lands the last piece of the
firing pipeline: the `s_find_nearobj` target search + the projectile-spawn
allocation observable + the muzzle-offset (and finds one real port fidelity gap
in `find_near_shape`). Nothing committed.

**Firing-enemy pipeline — end-to-end status:** aim angle (GSU-per-tick), aim
velocity, fire-gate timing (UPDATE 8) + target search + projectile alloc/spawn
+ muzzle offset (UPDATE 9) are all certified vs the cartridge. The one remaining
item to run a *whole* `houdai_strat`/`shou0_strat` tick as one call is wiring the
per-weapon `fire_X` body (`gen_weapon` runs the jump-threaded `mulslog` signed
multiply — the muzzle rotation primitive is certified transitively; a byte-exact
surgical run of the 3-stage rotate chain is the only deferred sub-step).

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

## UPDATE 6 — BATCH 3: a STATIC-init scenery strat + three more RNG INIT strats

Applying the batch-2 (masked-scan / body-diff) and firepillar (param-block RNG)
recipes to four more named strats. **Five new tests, all green** (`coexec_retail`
now **32 tests**). All four located as UNIQUE masked-scan hits (skeleton read out
of the built ROM via `symbols.txt` SNES addresses, WRAM/jsl operands wildcarded)
and cross-validated (RNG strats: `jsl RANDOM_L` operand read back == $02:FC58).

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_batch3_addresses` | ✅ | Masked-scans all four (each UNIQUE): `rockhard_Istrat`=**$06:85D9** (EXACT scan — pure struct-offset, byte-identical retail/built), `mine0_Istrat`=**$09:9117**, `big_meteor_Istrat`=**$00:FA62**, `tree1_Istrat`=**$09:95EE**. Reads the single `jsl RANDOM_L` operand of the three RNG strats back out == RETAIL_RANDOM_L ($02:FC58). |
| `retail_rockhard_strat_vs_port` | ✅ **MATCH** | STATIC scenery init (GSTRATS.ASM:663-669), ZERO globals/RNG. Seeds a DIRTY object, runs the retail cart's OWN body ($06:85D9): sets `al_collflags\|=enemy1`, `al_roty=deg180($80)`, `al_HP=$FF`, `al_AP=20`, NULLS `al_stratptr`. roty/hp/ap/null-tick MATCH vs port `rockhard_istrat` (IS 192). |
| `retail_mine0_body_vs_port` | ✅ **MATCH** | RNG INIT (DSTRATS.ASM:1572). Runs the retail cart's OWN `mine0_Istrat` body ($09:9117) — 1 real `jsl RANDOM_L` -> `al_rotz` (full byte). Seeds RNG via the firepillar param-block recipe (X=block PINS rand[3]=$36); `al_rotz` (RNG orientation) + HP=2/AP=10 MATCH vs port `mine0_init` (IS 246) on the same seed. Two seeds (rotz $C3/$CF). |
| `retail_big_meteor_body_vs_port` | ✅ **MATCH** | RNG INIT (D3STRATS.ASM:1069). Runs the retail cart's OWN `big_meteor_Istrat` body ($00:FA62) — 1 real `jsl RANDOM_L` -> `al_sbyte1=(rnd&15)-8`. `sbyte1` (RNG spin datum) + HP=$FF/AP=12 MATCH vs port `big_meteor_init` (IS 234). Two seeds (sbyte1 -8/+6). Confirmed the strat's `s_set_alvar2rnd` really is `jsl random_l` (per STRATMAC.INC:4409); the strat's `lda $18xx`/`$15xx` reads are the cosmetic `s_rots_flat` (view-vector rotx/roty, scoped out of the port), NOT the RNG. |
| `retail_tree1_rng_vs_port` | ✅ **MATCH** | RNG INIT (DSTRATS.ASM:2016). tree1 draws RANDOM once -> `al_sbyte1=(rnd&3)+1` (tree height). Draws one value from the cart's OWN `RANDOM` (via `retail_random_next`), applies `(rnd&3)+1`, and diffs vs port `tree1_init` (IS 204) on the same seed. Four seeds — height in [1,4], MATCH each. (Stream form: tree1's body does sprite/anim table reads after the draw, so the RANDOM stream is the clean surgical cert.) |

### Batch-3 footprint maps + newly-located retail
- **`rockhard`** (static scenery): reads NOTHING; writes `al_collflags`(|=enemy1),
  `al_roty`($80), `al_HP`($FF), `al_AP`(20), `al_stratptr`(=0, null tick). New
  class: a NON-init-only STATIC strat with a real collision/data footprint, no
  globals, no RNG — byte-identical struct-offset body (EXACT-scan hit).
- **`mine0`/`big_meteor`/`tree1`** (RNG INIT): each draws the runtime RNG exactly
  ONCE (`s_set_alvar2rnd`), landing on an orientation/height/spin datum:
  `mine0` -> `al_rotz` (full byte), `big_meteor` -> `al_sbyte1=(rnd&15)-8`,
  `tree1` -> `al_sbyte1=(rnd&3)+1`. All on the just-fixed enemy-lane `sf_random`
  site — three more confirmations the `ea_random`->`sf_random` fix is cartridge-
  faithful. Struct offsets `al_HP`=$2A, `al_AP`=$2B, `al_collflags`=$2E (all
  identical retail↔built↔port; from the mine0/rockhard `sta al_HP/AP` +
  `lda al_collflags` operands).
- **colltype is a representation remap** (like sflags): retail's ASM stores
  `enemy1` in `al_collflags` bit `$10`; the port re-derived its own `obj.h`
  `COLLTYPE_ENEMY1` encoding, and the port object additionally carries
  `strat_init_obj_vars` baseline bits — so colltype is certified by EFFECT
  (both set a colltype), not raw byte==byte, exactly as sflags.

### CERTIFIED VS RETAIL — running total: **12 named strats** + RNG stream + RNG-enemy
| # | Strat | Retail addr | Kind | Global/RNG footprint | Certified fields |
|---|------|------|------|------|------|
| 9 | `rockhard_Istrat` | $06:85D9 | static-init scenery | *(none)* | collflags(enemy1), roty, HP, AP, stratptr=0 |
| 10 | `mine0_Istrat` | $09:9117 | RNG init | 1× `RANDOM_L` | rotz (full byte), HP=2, AP=10 |
| 11 | `big_meteor_Istrat` | $00:FA62 | RNG init | 1× `RANDOM_L` | sbyte1=(rnd&15)-8, HP=$FF, AP=12 |
| 12 | `tree1_Istrat` | $09:95EE | RNG init | 1× `RANDOM_L` | sbyte1=(rnd&3)+1 (tree height) |

### Not certified this pass (recorded footprint + blocker)
The per-tick BODIES of the swing/mover strats are **private to sf-strat** (not
exposed via `world.istrats[]` or an `install()` ids struct), so the port side is
not publicly reachable from the integration test:
- **`wallleft_strat`/`wallright_strat`** (swing): located conceptually — retail
  bodies are pure struct-offset `al_roty += ±16` toward a limit (`b5 13 c9 C0/40
  d0 04 5c <jml> b5 13 18 69 10/F0 95 13 6b`), byte-identical retail/built with
  only the self-`jml` operand shifting. Blocked ONLY on a public port entry point
  for `wallleft_strat`/`wallright_strat`.
- **`volrockdown_strat`** (RNG mover / spread, 3 draws vx/vy/vz scatter): retail
  body ($0A:DC27) confirmed `vx=(rnd&15)-7, vy=(rnd&7)-15, vz=(rnd&15)-7` (3×
  `jsl RANDOM_L`), but reads/writes an object-`stratstate` PARALLEL array at abs
  `$1CDA,x` (base differs retail↔built) and its per-tick vy is post-`falldown`.
  Blocked on a public port entry point + the state-array base.
Both are one small sf-strat change (expose the per-tick fn as a StratId) away
from certifiable with the recipes already proven here.

### The RNG-driven enemy class is now CERTIFIABLE
The infrastructure (RNG stream lockstep + the param-block-collision seeding
recipe above) generalizes to the other rewired enemy RNG strats — `volrockdown`
(3 draws: vx/vy/vz scatter), `mother` (4 draws), the `volrock`/player spread
strats, etc. Each is: locate by masked scan (skeleton from the built ROM,
RANDOM_L operands wildcarded), seed the stream both sides, diff the RNG-derived
kinematic fields. `firepillar` is the template.

## UPDATE 7 — BATCH 4: a zdist state-transition MOVER + RNG/player-relative + death coin

Four more strats/decisions certified vs retail, spanning three NEW classes.
**Five new tests, all green** (`coexec_retail` now **37 tests**). All located by
masked signature scan (skeletons read from the built ROM via `symbols.txt` SNES
addresses, WRAM/jsl operands wildcarded), each a UNIQUE hit, cross-validated by
reading the embedded operands back out.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_batch4_addresses` | ✅ | Masked-scans all four (each UNIQUE): `woods_strat`=**$08:B7F6** (reads back `PLAYPT`=$1238, gate `cmp #2100`, `jml woodsgo_init`=$08:B813, and `woodsgo_init`'s installed `woodsgo_strat`=$08:B840), `tree2_Istrat`=**$09:952F** (RNG-first; `jsl RANDOM_L`==$02:FC58, `deg22`=$10), `shou0_Istrat`=**$0A:D615** (`jsl RANDOM_L`==$02:FC58; the `.again` reroll target = Istrat+31, i.e. it re-loops to the RNG draw). |
| `retail_woods_convert_gate_vs_port` | ✅ **MATCH** | zdist STATE-TRANSITION mover (GASTRATS.ASM:1386-1398). Runs the retail cart's OWN `woods_strat` body across the `\|dz\| < 2100` gate: below the gate it `jml`s `woodsgo_init` which CONVERTS the object into a homing missile (`al_stratptr = woodsgo_strat $08:B840`, `al_sbyte1 = 10` home timer); at/above it stays inert. The conversion DECISION (+ the sbyte1=10 / stratptr swap) MATCHes the port `woods_strat` (reached via its registered `woods_init` fall-through) at 4 distances incl. both sides of the boundary (2100 inclusive). First strat that MUTATES its own strat pointer on a player-Z gate. |
| `retail_tree2_body_vs_port` | ✅ **MATCH** | RNG + PLAYER-RELATIVE (DSTRATS.ASM:1976-2014) — the first strat combining an RNG draw with a player-position branch. Runs the retail cart's OWN `tree2_Istrat` body on seeded RNG + a live player (via PLAYPT): the PLAYER-RELATIVE tilt `(sbyte2, roty)` is an EXACT body match across BOTH branches (`enemy_x < player_x` → sbyte2=-deg22($F0)/roty+=deg45($20); else → sbyte2=deg22($10)/roty+=-deg45($E0)) — the port's `enemy_x-player_x` bit-15 test == retail's `cmp;bpl`. The RNG height `(rnd&3)+1` is stream-certified (port init consumes one RANDOM, `(draw&3)+1`, in [1,4]). |
| `retail_shou0_reroll_vs_port` | ✅ **MATCH** | RNG-REROLL firing-enemy init (GA2STRAT.ASM:1853-1859). Runs the retail cart's OWN `shou0_Istrat` body (player far → the fall-through `shou0_strat` zdist gate no-ops) on seeded RNG: its fire-pattern selector `sbyte1 = rnd&3` with a REROLL-on-3 loop (`jml .again` back to the draw) MATCHes the port `shou0_init` (IS_SHOU0=178) — the first RNG-with-reroll init certified. sbyte1 ∈ {0,1,2}. |
| `retail_break_meteort_coin_vs_port` | ✅ **MATCH** | The `break_meteorT` tadpole DEATH COIN (DPATHDAT.ASM:1787-1792). The spawn lives in the path VM (not a strat address), so certified at the DECISION level: draw one value from the cart's OWN `RANDOM` and compare `draw >= 127` (the 50% `s_jmp_random` threshold) against the PORT's REAL death strat `break_meteort_exp` (reached via its registered `break_meteort_init` expstrat), observed by whether it actually spawns a `SH_TADPOLE`. Both outcomes exercised. |

### Batch-4 footprint maps + newly-located retail
- **`woods`** (zdist state-transition mover): reads `PLAYPT`->player `al_worldz` +
  own `al_worldz`; on `\|dz\| < 2100` jml's `woodsgo_init`, which writes
  `al_stratptr`(=woodsgo_strat), the extended coll/exp ptrs (`$7E:1CD0/1CD2,x`,
  absolute WRAM — benign, like `gnd`), a leaf `jsl $06:EEEE`, `al_sbyte1`(=10) +
  snd2. NEW class: a player-Z gate that MUTATES the object's strat pointer.
- **`tree2`** (RNG + player-relative scenery): 1 RNG draw → height `(rnd&3)+1`;
  reads `PLAYPT`->player `al_worldx` + own `al_worldx`; writes `al_sbyte2`
  (±deg22 = $10/$F0), `al_roty` (±deg45 = $20/$E0). Entered 8-bit-A (`s_start_
  strat` shorta, p=$20). NOTE: the retail BODY's post-init `sbyte1` reads
  `(draw&3)` because it falls through into the sprouty grow tick whose segment
  countdown decrements the stored height once — the sprouty SEGMENT machinery is
  scoped out of the port, so the height is certified at the stream/formula level
  (port `sbyte1` == `(draw&3)+1`), not by diffing the post-tick body byte.
- **`shou0`** (RNG-reroll firing turret init): 1+ RNG draws → `al_sbyte1 = rnd&3`
  with REROLL while `==3` (uniform {0,1,2}); HP2/AP12/enemy1. Falls through into
  `shou0_strat` whose `[500,2500)` zdist gate no-ops when the player is far. The
  fire aim (`angle_xz` trig) + `/16`,`/32` staggered fire gate are the further,
  GSU/trig-touching part (not certified here).
- **`break_meteorT`** (death coin): a 50% `s_jmp_random` (threshold 127) in the
  path VM. Certified as a DECISION (RNG draw + threshold) against the port's real
  `break_meteort_exp`. Newly-confirmed: the port's private per-tick/exp bodies are
  reachable from the registered init via `world.istrats[]` + `expstratptr`.

### CERTIFIED VS RETAIL — running total: **15 named strats** + RNG stream + RNG-enemy + death coin
| # | Strat | Retail addr | Kind | Global/RNG footprint | Certified fields |
|---|------|------|------|------|------|
| 13 | `woods_strat` | $08:B7F6 | zdist state-transition mover | `PLAYPT`->player Z | conversion gate (stratptr swap→woodsgo_strat, sbyte1=10) |
| 14 | `tree2_Istrat` | $09:952F | RNG + player-relative | 1× `RANDOM_L`, `PLAYPT`->player X | sbyte2/roty tilt (body, both branches), height (rnd&3)+1 (stream) |
| 15 | `shou0_Istrat` | $0A:D615 | RNG-reroll firing init | 1+× `RANDOM_L` (reroll on 3) | sbyte1 ∈ {0,1,2} |
| — | `break_meteorT` coin | (path VM) | RNG death decision | 1× `RANDOM_L`, thresh 127 | tadpole spawn iff draw≥127 (both outcomes) |

Newly-located retail: `woods_strat`=$08:B7F6, `woodsgo_init`=$08:B813,
`woodsgo_strat`=$08:B840, `tree2_Istrat`=$09:952F, `shou0_Istrat`=$0A:D615,
`shou0_strat`=$0A:D646. Reused: `RANDOM_L`=$02:FC58, `PLAYPT`=$1238, struct
offsets `al_sbyte2`=$23, `al_roty`=$13.

### Still uncertified (recorded footprint + blocker)
> **NOTE (UPDATE 8):** the GSU-in-the-tick aim ANGLE + aim VELOCITY + fire-GATE
> timing of the `shou0`/`bazooka`/`houdai5f`/`torpedo` class are now CERTIFIED
> (see UPDATE 8). The blockers below are narrowed to the object-SEARCH +
> projectile-SPAWN machinery around the (now-certified) aim.
- **`torpedo`** (GASTRATS.ASM:2007-2044): homing underwater mover — the yaw-home
  aim (`s_obj2obj_angle` -> GSU arctan) + `s_gen_3dvecs` are certified (UPDATE 8);
  the full per-tick body still needs the CPU `Achase` + underwater-mover glue.
- **`shou0`/`bazooka`/`houdai5f` FULL BODY**: aim angle (GSU), aim velocity, and
  fire-gate timing are certified (UPDATE 8); the whole tick additionally needs
  `s_find_nearobj` (target search) + `spawn_projectile`.
- **`volrockdown`/`mother`** (multi-draw RNG scatter): still blocked on the
  object-`stratstate` parallel-array base (volrockdown) / public port entry.
- **`wallleft`/`wallright`** (swing): still blocked on a public port entry point.

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

## UPDATE 8 — GSU-PER-TICK AIMING CLASS CERTIFIED (the hardest frontier)

The largest remaining uncertified class — **every enemy that aims at the player
and fires** (`houdai`, `shou0`, `bazooka`, `torpedo`, …) — is now certified end-
to-end vs retail. Its per-tick aim step runs the **Super-FX chip inside the tick**
(the arctan), which was the standing blocker (UPDATE 7). **Four new tests, all
green** (`coexec_retail` now **41 tests**). The GSU ran live, from a real retail
CPU aim routine, once per aim.

### The aiming pipeline (retail addresses located)
A firing enemy's tick is `s_obj2obj_angle` (aim) + `s_gen_3dvecs` (velocity) +
`s_jmp_notdelay` (fire gate). The **only GSU-in-the-tick is the arctan**:

| Routine | Retail | How | GSU? |
|------|------|------|------|
| `anglexy_l` / `Yanglexy_l` (aim yaw) | **$1F:D021** | masked scan (UNIQUE); x1=dp$02, y1=dp$08 (== built) | drives it |
| `arctan16_l` (GSU wrapper) | **$02:FCF1** | `anglexy_l`'s `jsl` operand (built $02:F854) | `->runmario_l->mcallarctan16` |
| `n3dvecs_l` (aim vel, `gen_3dvecs`) | **$1F:C41E** | masked scan (UNIQUE); scratch shifted (z1 $8A->$90, tmpz $78->$7E, troty/trotx $1631/30->$15A7/A6) | **no** (CPU sin/cos) |
| fire gate `s_jmp_notdelay` | 52 sites | scan `lda gameframe; clc; adc al1pt; and #mask` | no |

`anglexy_l`(X=aimer,Y=target) computes `dx=worldx[tgt]-worldx[aimer]`,
`dz=worldz[tgt]-worldz[aimer]`, copies them into GSU RAM and `jsl arctan16_l`,
which does `sta m_x1/m_y1; lda #mcallarctan16>>16; ldx #…; jsl runmario_l` — the
RAM GSU trampoline — then reads back `m_cnt`. The strat stores `arctan16>>8` as
its yaw target.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_aiming_pipeline_addresses` | ✅ | Locates `anglexy_l`=$1F:D021 (UNIQUE) + reads its `jsl` operand -> retail `arctan16_l`=$02:FCF1. Confirms the x1/y1 scratch avoids the harness param block ($F0-$F5) / retail `rand` ($EF-$F2), so a GSU roundtrip survives a surgical call. |
| `retail_aiming_angle_gsu_vs_port` | ✅ **MATCH (GOLD)** | Runs the retail cart's OWN `anglexy_l` on a seeded (enemy, player) pair over a 20-position grid (all quadrants, shallow+steep). Each call **KICKS THE GSU** (`gsu_kicks == 20`, one per aim) through the RAM trampoline; the ROM `arctan16` runs on the Super-FX chip and returns the 16-bit angle via shared bank-$70 RAM. Diffs the stored aim angle (`arctan16>>8`) vs the port `common::strat_angle_xz` (== `angle_xz`): **max 8-bit delta = 1** (the documented float-vs-fixed tolerance; the ROM's 512-entry arctan table quantises the low bits). This is a real GSU call executing inside a strat's aim step, certified vs the cartridge. |
| `retail_gen_3dvecs_vs_port` | ✅ **MATCH** | Completes the aim-math pipeline (angle -> velocity). Runs the retail cart's OWN `n3dvecs_l` ($1F:C41E, pure CPU sin/cos) on seeded (roty,rotx,vel) over the tests/gen_3dvecs.rs spread: **vx/vz and \|vy\| bit-exact** vs port `common::strat_gen_vecs_3d` (the vy SIGN is the renderer Y convention — port negates pitch, ROM does not — identical to the built-ROM cert). Re-derived the shifted retail scratch block from the routine's own operands. |
| `retail_fire_gate_notdelay_vs_port` | ✅ **MATCH** | The pure-integer fire GATE. Locates **52** staggered `s_jmp_notdelay` sites in retail (`lda gameframe; clc; adc al1pt; and #mask`; masks {$00,$01,$03,$07,$0F,$1F} = fire every 1/2/4/8/16/32 f), all `(1<<delay)-1` low-bit masks. Certifies the decision `(gameframe+stagger) & mask == 0` vs the port's identical expression (`bossb::notdelay_stag` / enemy_b) over **27,648** (gameframe,stagger,mask) combos — every combo agrees. |

### The aiming class — footprint
- **Aim angle** (`s_obj2obj_angle`): reads target+aimer `al_worldx/z`; the ONLY
  GSU-in-the-tick step. Certified as `arctan16>>8` (retail GSU) == port
  `angle_xz` within ±1. The downstream `xba; nega` + `Achase_alvar2a` is the
  chase-direction convention applied identically both sides.
- **Aim velocity** (`s_gen_3dvecs`): CPU sin/cos tables (`n3dvecs_l`), NO GSU —
  vx/vz + |vy| bit-exact vs port.
- **Fire gate** (`s_jmp_notdelay #delay,…,al1pt`): pure integer
  `(gameframe + al1pt_stagger) & ((1<<delay)-1) == 0`; bit-exact vs port.

### What remains (the full aiming-strat BODY)
Running a *whole* firing-enemy tick (`houdai_strat`) end-to-end additionally
needs the object-search primitive `s_find_nearobj` (active-list scan for the
nearest `enemy2`) + `spawn_projectile` — machinery around the aim, not the aim
itself. The **aim angle (GSU-per-tick), aim velocity, and fire-gate timing are
all certified**; deferring only the target-search + projectile spawn. That is
the UPDATE-7 milestone ("even the aim-angle + fire-gate-timing certified,
deferring the spawned projectile, is the milestone") — met, with the GSU running
live rather than the aim math alone.

### CERTIFIED VS RETAIL — running total: **15 named strats + the AIMING CLASS**
| Cert | Retail addr | Kind | GSU? | Certified |
|------|------|------|------|------|
| aim angle (`anglexy_l`->`arctan16`) | $1F:D021 -> $02:FCF1 | GSU-per-tick aim | **YES (live)** | `arctan16>>8` == port `angle_xz` ±1, 20 positions, gsu_kicks=20 |
| aim velocity (`n3dvecs_l`) | $1F:C41E | CPU aim vel | no | vx/vz + \|vy\| bit-exact == port `gen_3dvecs` |
| fire gate (`s_jmp_notdelay`) | 52 sites | integer fire timer | no | `(gameframe+stag)&mask==0` == port, 27,648 combos |

Newly-located retail: `anglexy_l`=$1F:D021, `arctan16_l`=$02:FCF1,
`n3dvecs_l`=$1F:C41E, `troty`/`trotx`=$15A7/$15A6, `x1`/`y1`/`z1`/`tmpz` scratch
=$02/$08/$90/$7E. Reused: `runmario_l` RAM trampoline=$7E:4EE9, `gameframe`=$15BB,
`al1pt`=$123A.

## UPDATE 9 — PROJECTILE-SPAWN + TARGET-SEARCH (the last piece of the firing pipeline)

The machinery *around* the certified aim — the object SEARCH a homing/turret
strat runs (`s_find_nearobj`) + the projectile SPAWN (`fire_weapon_l` ->
per-weapon `fire_X` = `sr_make_obj` + `gen_weapon`) — is now located, and its
observable output certified vs the port. **Three new tests, all green**
(`coexec_retail` now **44**). One real port fidelity gap found.

### The spawn/search pipeline (retail addresses located, all UNIQUE + x-validated)
A firing strat's fire step is `s_find_nearobj` (pick a target) then
`s_fire_weapon` -> `fire_weapon_l` (weapon-table RTL dispatch) -> per-weapon
`fire_X` = `sr_make_obj` (alloc+init+shape) + field sets + `gen_weapon` (place
the shot at firer + a ROTATED muzzle offset, set rots/speed).

| Routine | Retail | How located / cross-validated |
|------|------|------|
| `find_nearobject_l` (`s_find_nearobj`) | **$1F:C870** | masked scan (UNIQUE); its `jsl` operand = `xzdiffs_l`, `ldx`/`lda` operands = `fobj`/`rangexz` |
| `xzdiffs_l` (XZ octagonal dist) | **$1F:D0AB** | `find_nearobject_l`'s `jsl` operand; `fobj`=$14CA, `rangexz`=$1250 |
| `fire_weapon_l` (`s_fire_weapon`) | **$1F:D146** | masked scan (UNIQUE); `weapons_data+4` operand = $1F:D17E |
| `sr_make_obj` (`s_make_obj`) | **$1F:D54B** | masked scan (UNIQUE); its two `jsl`s = `makeobj_l`/`init_objvars_l` |
| `makeobj_l` (pool allocator) | **$1F:D3A9** | scan + `sr_make_obj`'s 1st `jsl` (x2); its `ldx $121F`/`lda $121D` = pool `freelist_head`/`active_head` |
| `init_objvars_l` (zero + default sflags) | **$1F:D36E** | `sr_make_obj`'s 2nd `jsl` |
| `gen_weapon` muzzle rotate chain | rotz `rotate_8yx_l`=**$1F:CC78** -> rotx `rotate_8yz_l`=**$1F:CAFB** -> roty `rotate_8xz_l`=**$1F:C97B** | masked scan (each UNIQUE); CPU sin/cos, NO GSU |

Newly-located WRAM: `fobj`=$14CA, `rangexz`=$1250, `tpa`=$14C5, `weapons_data`=
$1F:D17A, `stratflags`=$14D2; DP scratch `tpx`/`tpy`/`tpz`=$3A/$3C/$3E (identical
retail/built, below the `call` param block so surgically seedable).

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_spawn_pipeline_addresses` | ✅ | Locates + cross-validates all 9 routines above (each a UNIQUE masked hit; operands read back match). `makeobj_l`'s `alfreelst`/`allst` operands independently reproduce `RETAIL_POOL`. |
| `retail_find_nearobject_vs_port` | ✅ **MATCH (coplanar) + GAP characterized** | Runs the retail cart's OWN `find_nearobject_l` (with the real `xzdiffs_l` inside) over seeded object lists and diffs the SELECTED target vs the port `strat_find_near_shape`. **8/8 coplanar configs + the radius-band reject MATCH.** A Y-separated config DIVERGES (see below) — characterized, not a failure. |
| `retail_sr_make_obj_spawn_vs_port` | ✅ **MATCH** | Runs the retail cart's OWN `sr_make_obj` on a formatted pool (real `makeobj_l` pop + `init_objvars_l` zero + shape store) and diffs the NEW object's observable fields — `al_shape` == requested, world coords zeroed, free list shrank by one — vs the port `make_obj`. Both materialise a fresh shape=$0042 object at (0,0,0); both pop slot 0 here. |

### REAL FINDING — port `find_near_shape` diverges from retail for Y-separated targets
Retail `find_nearobject_l` ranks + gates candidates by **`xzdiffs`/`rangexz`, an
XZ-plane octagonal-norm distance that IGNORES the Y coordinate entirely** (both
the `[min,max)` radius band and the nearest metric). The port
`enemy_a::strat_find_near_shape` (strat_enemy.c:4315) instead uses a **3D box
gate (`dz≤max_z && dx≤max_xy && dy≤max_xy`) + a 3D Manhattan `dx+dy+dz` metric**
that COUNTS Y. Consequences:
- **Identical for coplanar targets** (targets sharing the searcher's Y-plane —
  the overwhelming in-game case for a same-enemy-type search): certified 8/8.
- **Can pick a DIFFERENT target when candidates differ in Y**: e.g. candidate A
  at (dx=300, dy=7000) vs B at (dx=2000, dy=0) — retail picks A (XZ-nearest,
  ignores Y); the port picks B (A's Y penalises its Manhattan metric). Test
  `retail_find_nearobject_vs_port` asserts this exact divergence.

This is a genuine (minor) port-vs-cartridge fidelity gap: a homing/turret enemy
could lock a *different* target than the retail cart when candidate enemies are
vertically separated. FIX (sf-strat, out of scope here): replace the 3D box +
`dx+dy+dz` in `strat_find_near_shape`/`strat_find_near_colltype` with the
XZ-only `rangexz` octagonal band (port the `xzdiffs_l` formula) so Y is dropped.

### Muzzle offset (`gen_weapon`) — certified transitively; the deferred sub-step
`gen_weapon` places the shot at `firer.pos + (offset rotated rotz->rotx->roty by
the firer's rots) << weapon_scale(=2)`. The rotation primitives `rotate_8yx/8yz/
8xz_l` are **CPU sin/cos** — the SAME sin/cos rotation already certified bit-exact
vs retail as `n3dvecs_l`/`arctan16` (UPDATE 8). The port paths compose exactly
this: the common enemy-laser path passes offset **(0,0,0)** (rotation of zero is
a no-op → shot at firer origin, trivially == retail), and the turret/boss paths
use `enemy_a::boss1_rot_offset_pos` (same rotz->rotx->roty order, same
`strat_sin`/`strat_cos` used by the certified `gen_vecs_3d`). So the muzzle
offset is certified transitively. The only DEFERRED sub-step is a byte-exact
surgical run of the retail 3-stage rotate chain: `rotate_8xz_l` etc. thread
through a shared jump-based `mulslog` signed-multiply continuation whose x2/y2/z2
output-scratch stores are not linearly locatable from the routine head — running
it surgically needs that continuation traced (the primitive itself is already
proven equivalent via `retail_gen_3dvecs_vs_port`).

### CERTIFIED VS RETAIL — running total: **15 named strats + AIMING + SPAWN/SEARCH**
| Cert | Retail addr | Kind | Certified |
|------|------|------|------|
| target search (`find_nearobject_l`) | $1F:C870 -> `xzdiffs_l` $1F:D0AB | XZ octagonal-band nearest | selected target == port `find_near_shape` for all coplanar configs (8/8) + radius reject; Y-separated divergence characterized |
| spawn alloc (`sr_make_obj`) | $1F:D54B -> `makeobj_l` $1F:D3A9 | pool pop + init + shape | new-object observable (shape + zeroed world pos) == port `make_obj` |
| muzzle offset (`gen_weapon`) | rotate `$1F:CC78/CAFB/C97B` | CPU sin/cos rotate + firer pos | transitively (== certified `gen_3dvecs` sin/cos); (0,0,0) common path exact |

## UPDATE 10 — the COLLISION SYSTEM certified vs retail (highest blast radius)

The shared code every laser hit, ship/enemy contact and pickup depends on. Three
pieces located + certified vs the cartridge. **Five new tests, all green**
(`coexec_retail` now **49**). The box-overlap MATH and the colltype ALLOW-MATRIX
both **MATCH** the port exactly; the collision RESPONSE (`do_coll_l`) **MATCHES**
run-for-run; and **one real (narrow-blast-radius) port-vs-cart divergence** is
characterized — the retail **same-shape collision gate** the port omits.

### Retail collision addresses located (all cross-validated)
| Routine | Retail | Residency | How located / cross-validated |
|------|------|------|------|
| `do_coll_l` (response) | **$1F:D23A** | ROM (JSL/RTL) | UNIQUE masked scan; operands read back = `pshipflags3`=$14D8, `tpa`=$14C5 (== `RETAIL_TPA`); offsets collcount=$2D, HP=$2A; consts hardAP=8, intunnel=$01 |
| `COLDET` box-overlap | **$02:A1BF** | ROM copy-source of RAM `chkcoll` ($7E:5015) | axis-pattern scan; `sta/sbc rangexz`=$1250 (== `RETAIL_RANGEXZ`), boundary opcode = **BMI** (strictly-less); 8 axis-tests total |
| `chkcoll0` colltype filter | **$02:A159** | (same copy-source) | UNIQUE scan; `and #imm`=$00F8, `al_collflags`=$2E |
| `chkcoll0` same-shape gate | **$02:A199** | (same copy-source) | scan; `lda al_shape,x; cmp currshape=$1F03; beq->skip`; immuneptr=$19 |

**KEY: `chkcoll` is RAM-resident** (SNES $7E:5015 in the symbol map — the whole
detector is copied to WRAM at boot), so it can NOT be `JSL`'d on a non-booted
bus. The box-overlap + colltype/same-shape logic was therefore located in its
**ROM copy-source** (bank $02) and certified structurally + by grid/matrix-diff.
`do_coll_l` IS ROM-resident, so it is RUN surgically.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_collision_addresses` | ✅ | Locates + cross-validates all four (do_coll_l UNIQUE; colltype filter UNIQUE; COLDET axis-pattern + same-shape gate present), reading every operand back. |
| `retail_docoll_response_vs_port` | ✅ **MATCH** | Runs the cart's OWN `do_coll_l` ($1F:D23A) on a seeded victim over an 11-case grid of (collcount, hp, ap, tunnel) and diffs (collcount, hp) vs the port `Game::do_coll`: the DEC-then-BNE cooldown gate, hp bit-7 (>=$80) indestructible branch, underflow clamp at 0, in-tunnel hardAP halving (`asra`), and framesperAP reload — all MATCH. |
| `retail_box_overlap_vs_port` | ✅ **MATCH** | Grid-diffs the PORT public `aabb_overlap` vs a byte-faithful transcription of the retail `COLDET` macro (16-bit two's-complement abs, Z/X/Y order, strictly-less `|d| < e1+e2`) over boundary-straddling separations on each axis + the i16 wrap edge. 0 mismatches; boundary pinned exactly (sep==sum -> no overlap, sep==sum-1 -> overlap). |
| `retail_colltype_matrix_vs_port` | ✅ **MATCH** | Diffs the port colltype filter (`a_types & b_types != 0 -> skip`) vs the ROM rule (`cf_a & cf_b & $F8 != 0 -> skip`) over the FULL type matrix (4096 combos) + semantic spot-checks (laser vs enemy = collide; laser vs laser / enemy1 vs enemy1 = skip; typeless objects = collide — **no both-zero skip**). |
| `retail_same_shape_skip_divergence` | ⚠️ **DIVERGENCE (characterized)** | Constructs two SAME-shape, DIFFERENT-colltype overlapping objects, runs the port collision pass, and shows the port collides them — where the cart's same-shape gate would SKIP. Pins current port behaviour with a note to flip on the sf-game fix. |

### Box-overlap MATH — MATCH (the size source, axis order, boundary)
- **Size source**: the retail overlap sums the per-SHAPE `cl_xmax/ymax/zmax`
  (`generate_collist_l` copies `sh_xmax/ymax/zmax` from the shape header) — a
  **shape->size table**, NOT a per-object `al_size`. The port reads the identical
  source (`hooks.shape_extents(al.shape)`). **MATCH.**
- **Axis order**: Z, then X, then Y (three early-out `jmp`s) — identical to the
  port `aabb_overlap`.
- **No Z asymmetry**: Z uses the SAME `cl_zmax + zmax` formula as X/Y (no
  different threshold) — the early-out ORDER is Z-first but the math is symmetric.
- **Boundary**: `sec; sbc rangexz; bmi` => in-range iff `(|d| - sum) < 0` i.e.
  `|d| < sum` (STRICTLY less). Port: `if |d| >= sum return false`. Identical,
  including the i16 wrap: two's-complement abs of `i16::MIN` stays `i16::MIN`
  both sides (treated as "in range").

### Colltype ALLOW-MATRIX — MATCH (who may hit whom)
Retail `chkcoll0`: `lda al_collflags,y; and al_collflags,x; and #$F8; bne ->skip`
— a pair is dropped iff it SHARES any collision-type bit; typemask $F8 =
colltype1(lasers)|2(enemy1)|3(enemy2)|4(enemy-weapons)|5(friend). The port uses
the identical rule + identical bit values (`ACF_COLLTYPE1..5` = $08..$80). Crucially
**neither** side has a "both objects typeless -> skip" (the earlier port bug,
already removed) — verified over all 4096 collflag combos. Immunity: retail
`cmp al_immuneptr,x` has NO nonzero guard (immuneptr=$19); the port's WEAPON-based
guard is a documented workaround for its 0-based-slot player representation
(player == slot 0 collides with the "no owner" 0), a representation remap not a
behaviour change.

### REAL FINDING — the retail SAME-SHAPE gate the port omits
Retail `chkcoll0` (SNES $02:A199): `lda al_shape,x; cmp currshape; beq ->
chkcollnxt` — two objects with the **same `al_shape`** are SKIPPED, UNLESS BOTH
carry the `sameshapecollide` sflag (sflags3 bit $80). `sameshapecollide` is set
by essentially nothing in the whole game (1 file / 2 sites: DSTRATS.ASM), so the
cart effectively **never collides two same-shape objects with each other**. The
port `Game::coldet_run` (coldet.rs) has **no shape gate at all**.
- **Blast radius is NARROW**: same-shape objects usually also share a colltype and
  are already dropped by the (certified-matching) colltype filter. The residual
  case that bites is two objects of the SAME shape but DIFFERENT colltype (e.g.
  the same enemy model registered enemy1 vs enemy2) overlapping — the cart skips,
  the port damages both.
- **FIX (sf-game, out of scope here)**: in `coldet_run`, before `aabb_overlap`,
  skip the pair when `a.shape == b.shape` unless both objects carry a
  `sameshapecollide` bit (add `ASF3_SAMESHAPECOLLIDE = $80` to `sflags3`).
  `retail_same_shape_skip_divergence` pins the current behaviour; flip its
  expectation to `!collide` when the gate lands.

### CERTIFIED VS RETAIL — running total: **15 named strats + AIMING + SPAWN/SEARCH + COLLISION**
| Cert | Retail addr | Kind | Certified |
|------|------|------|------|
| collision RESPONSE (`do_coll_l`) | $1F:D23A | hp/cooldown damage | (collcount,hp) == port `do_coll` over the full damage/cooldown/tunnel/indestructible grid |
| box-overlap MATH (`COLDET`) | $02:A1BF (RAM copy-source) | 16-bit AABB | port `aabb_overlap` == ROM formula over the boundary grid (Z/X/Y, strictly-less, shape-table sizes) |
| colltype ALLOW-MATRIX (`chkcoll0`) | $02:A159 | who-hits-whom | port filter == ROM `cf_a&cf_b&$F8` over all 4096 combos; no both-zero skip |
| same-shape gate (`chkcoll0`) | $02:A199 | same-shape skip | **DIVERGENCE** — ROM skips same-shape pairs, port does not (narrow; fix noted) |

## UPDATE 11 — PLAYER MOVEMENT certified vs retail (the per-frame ship physics)

The highest-blast-radius shared system that runs every frame the player is
alive. Two runnable cores located + certified vs the cartridge, plus the rest of
the pipeline confirmed already-certified. **Three new tests, all green**
(`coexec_retail` now **52**). Both cores **MATCH** the port; the known
bounds-clamp parity concern is CONFIRMED already-fixed (inclusive both edges);
one domain-boundary numeric divergence characterized (unreachable in gameplay).

### Retail player-move addresses located (all cross-validated)
| Routine | Retail | How located / cross-validated |
|------|------|------|
| `playerlimitx_srou` (X bounds clamp) | **$0B:DF21** | UNIQUE masked scan; operands read back = `arrows`=$1FC7, `minpmoveX`=$156F, `maxpmoveX`=$1571 (contiguous +2); boundary opcodes min BEQ+BMI (`<=`), max BPL (`>=`); arrow sets ORA #$04/#$08 |
| `sr_speedto` (boost/brake ramp) | **$1F:D60D** | UNIQUE masked scan; all three `tpa` operands read back == `RETAIL_TPA` ($14C5) — the SAME scratch as `do_coll`/`sr_make_obj`, an independent cross-validation; `al_vel`=$15, `tpx`(rate)=dp $3A |
| steering rot-step (`playermove_srou`) | (in $0B) | the `clc; adc #$0200` step immediate (ZROT_SPEED/XROT_SPEED) confirmed present (5 sites) — the port's `XROT_SPEED`/`ZROT_SPEED`=$200 are cartridge-faithful |

The full steering→velocity→position pipeline is
`playermove_srou` (pad → plrot*/ztilt accumulators → `al_rot*`) →
`gen_3dvecs` (→ `al_vx/vy/vz`) → `addalvecs_l` (→ `al_worldx/y/z`) →
`playerlimitx_srou` (clamp). The middle two stages are **already certified vs
retail**: `gen_3dvecs`/`n3dvecs_l` (UPDATE 8, vx/vz + |vy| bit-exact) and
`addalvecs_l` (UPDATE 1, tick-for-tick). This UPDATE lands the clamp + the
boost/brake speed ramp; the plrot* accumulator body is the only remaining
uncertified sub-step (large WRAM + pad-read footprint — its rotation-scale
constants are cross-validated statically here).

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_player_move_addresses` | ✅ | Locates + cross-validates `playerlimitx_srou`=$0B:DF21 + `sr_speedto`=$1F:D60D (each UNIQUE), reading every WRAM operand + boundary opcode back out. `sr_speedto`'s `tpa` independently == `RETAIL_TPA`. |
| `retail_playerlimit_x_bounds_vs_port` | ✅ **MATCH** | Runs the cart's OWN `playerlimitx_srou` over a grid straddling each screen-edge X bound (below/at/above min & max, 3 boxes incl. the degenerate `[0,0]`) and diffs (clamped worldX, arrows) vs the port. **MATCH over 27 cases.** Pins BOTH bounds INCLUSIVE (worldX==min → clamp+LEFT; worldX==max → clamp+RIGHT), and that `AND #$F3` preserves non-L/R arrow bits. The Task-#34 bounds concern is CONFIRMED already-fixed vs the cartridge. |
| `retail_speedto_boost_brake_vs_port` | ✅ **MATCH** | Runs the cart's OWN `sr_speedto` over the reachable player-speed domain (vel/target in the 20..85 boost/brake band, at the rate-2 the player ramp uses + rate-1) and diffs the resulting `al_vel` vs the port `strat_speed_to`. **MATCH over all cases** — the snap-when-near guard, the directional step, and the already-at-target fixed point all agree (the port's overflow fix is cartridge-faithful). |

### BOUNDS clamp — the known concern, MATCH + exact limits
- **Both bounds INCLUSIVE.** min side clamps on `worldX <= min` (ROM BEQ+BMI),
  max side on `worldX >= max` (ROM BPL after CMP). The port's `<=`/`>=` match
  exactly. (This is the Task-#34 fix; certified here vs the *retail* cart, not
  just the built ROM.)
- **Edge arrows**: at the min edge the ROM sets `sprar_left` ($04) + clamps; at
  the max edge `sprar_right` ($08) + clamps; the top `AND #$F3` clears only
  left|right and preserves other arrow bits — port `& !(RIGHT|LEFT)` identical.
- **X only**: the ROM `playerlimitx_srou` clamps only X + the L/R arrows. The
  port's Y clamp (miny/maxy → up/down arrows) in the same fn is an HD-runtime
  addition NOT present in the ROM routine, so it is excluded from this cert.

### CHARACTERIZED (domain-boundary, UNREACHABLE) — CMP sign-bit wrap
The ROM compares `worldX` to the bound with a 16-bit `CMP` + `BMI`/`BPL`, which
tests only the SIGN bit of the subtraction (the 65816 `CMP` sets no V flag), so
when `|worldX − bound| > 32767` the comparison wraps. The port uses a TRUE i16
`<=`/`>=`. They diverge past that overflow edge: e.g. `worldX=32700`,
box `[-500,500]` → retail clamps to **−500 + LEFT** (32700−(−500)=33200 wraps
negative), port clamps to **500 + RIGHT**. `worldX` cannot reach ~+32700 in one
frame under the per-frame clamp, so this is **unreachable in gameplay** — it is
recorded (test eprintln) but NOT asserted as a bug.

### CERTIFIED VS RETAIL — running total: **15 named strats + AIMING + SPAWN/SEARCH + COLLISION + PLAYER-MOVE**
| Cert | Retail addr | Kind | Certified |
|------|------|------|------|
| player X BOUNDS clamp (`playerlimitx_srou`) | $0B:DF21 | screen-edge clamp | (clamped worldX, arrows) == port `playerlimit_x_srou` (X) over 27 cases; both bounds INCLUSIVE; CMP-wrap edge unreachable |
| boost/brake speed ramp (`sr_speedto`) | $1F:D60D | `al_vel` -> tospeed | `al_vel` == port `strat_speed_to` over the reachable 20..85 domain (rate 1-2) |
| steering->velocity (`gen_3dvecs`) | $1F:C41E | *(UPDATE 8)* | vx/vz + \|vy\| bit-exact == port (already certified) |
| position integrator (`addalvecs_l`) | $1F:C7BB | *(UPDATE 1)* | worldx/y/z tick-for-tick == port (already certified) |

## UPDATE 12 — the FIRST BOSS certified vs retail (`boss8`) + the plrot* accumulator

The largest remaining behavioral-coverage gap — a multi-phase **BOSS** with a
child family — is broken open: **`boss8`** (the "washing machine" wash boss,
GB3STRAT.ASM:42-204, Sector Z / Venom) has its INIT, its child spawn, and its
common per-tick STATE MACHINE certified tick-for-tick vs the cartridge. Plus the
deferred player-move sub-step (UPDATE 11's one open item) — the `playermove_srou`
**plrot\* accumulator** — is closed. **Four new tests, all green** (`coexec_retail`
now **56**). All boss addresses located by masked signature scan (skeleton read
from the built ROM via symbols.txt, WRAM/jml operands wildcarded), each a UNIQUE
hit, cross-validated by reading the operands back.

### Why `boss8` (tractability)
Of the ~15 bosses, `boss8`'s per-tick machine converges (from all three phases —
`boss8wait`/`boss8a`/`boss8b`) into ONE common body, **`boss8_cont`**, that is
pure CPU: **NO GSU, NO RNG**. It reads `player_posz` + `gameframe` + one global
(`gsvar_byte1`) and evolves a rich multi-field state machine — ideal to isolate.
Its INIT is a clean scalar init + a 4-child spawn. (boss1's phase strats all fall
through a GSU turret-repositioning tail `boss1rots_srou`; the Andross/`bossA`
forms are the multi-child GSU-heavy targets — deferred, see the gap map.)

### Retail boss8 addresses located (all cross-validated)
| Routine | Retail | How located / cross-validated |
|------|------|------|
| `boss8_cont` (common per-tick body) | **$07:93BB** | UNIQUE masked scan (+$0C from built $0793AF); operands read back = `player_posz`=$1511 + `gameframe`=$15BB (both already-certified globals) and DERIVE `gsvar_byte1`=$154F (lda/inc/dec all agree) |
| `boss8_Istrat` (INIT) | **$07:919C** | UNIQUE masked scan; operands = `currentlevel`=$1FFD, installed `boss8wait_strat`=$07:9359, HP=$20 easy/$40 hard, AP=$08 |
| `boss8wait_strat` (phase-wait tick) | **$07:9359** | read straight out of `boss8_Istrat`'s `s_set_strat` immediate |
| `gsvar_byte1` | **$154F** (built $15DA) | `boss8_cont`'s `lda/inc/dec` operands; port ext-WRAM cell $0310 |

Newly-located: `al_sbyte4`=$25 (struct offset), boss8 `sflag1` = `al_sflags2`
bit $10, `currentlevel`=$1FFD, `gsvar_byte1`=$154F.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_boss8_addresses` | ✅ | Locates + cross-validates all three boss8 routines (each UNIQUE), reads back the three per-tick globals + the INIT constants (HP level-gate $20/$40, AP $08, `boss8wait` pointer). |
| `retail_boss8_init_vs_port` | ✅ **MATCH** | Runs the cart's OWN `boss8_Istrat` ($07:919C) on a formatted pool (boss block popped off the free list) and diffs the boss's INIT scalar fields vs the port `strat_boss8_init` (IS_BOSS8=84): HP (level-gated), AP=$08, sbyte4 phase timer, colltype (enemy2\|enemyweap), cleared sflag1\|sflag2, `gsvar_byte1`=0, `stratptr`=boss8wait, and the init-tail `boss8_cont` worldz = 1680+player_posz — all MATCH, **both difficulty branches** (retail currentlevel 0=easy/1=hard <-> port 1/2, a level-encoding remap). **Spawn observable**: the boss makes exactly **4 children** (cover + 3 nucleus beams) — the free list shrank by 4. |
| `retail_boss8_cont_body_vs_port` | ✅ **MATCH (GOLD)** | Runs the cart's OWN `boss8_cont` ($07:93BB) over a long horizon on a seeded boss and diffs its evolving STATE MACHINE tick-for-tick vs the port (reached through the armed `boss8wait_strat` with the beam-child sflag1 cleared so the wait routes into `boss8_cont`). Three fields: **worldz** = 1680+player_posz view-track (incl. an i16 wrap), the **sbyte4** countdown that reloads 150 and TOGGLES sflag1 at 0, and the **gsvar_byte1** speed accumulator that ramps +1 toward +5 (sflag1 clear) / -1 toward -5 (sflag1 set), gated on `gameframe & 7`. Two scenarios: a full 150-tick countdown -> sflag1 toggle -> gsvar ramps +5 then REVERSES to -5 (200 ticks); and an early toggle + worldz wrap (40 ticks). **MATCH every tick.** |
| `retail_plrot_accumulator_vs_port` | ✅ **MATCH** | Closes UPDATE 11's deferred player-move sub-step. Locates the `playermove_srou` plrot steering blocks — LEFT (`plrotz/plroty += $200`), RIGHT (`-= $200`), and the plrotz LIMIT (`±$600`), each a UNIQUE masked hit — and reads back the step (Zrotspeed=**$0200**), the roll clamp (**$0600**) and the `plrotz`/`plroty` addresses (**$1234/$1232** = built $12BF/$12BD − $8B) from the retail BYTES. The decay is `strat_chase_proportional` (already certified vs the cart, UPDATE 4); a grid-diff confirms the port primitive == a byte-faithful `Achase` at the plrot rates (3/4), and the composed per-frame plrot(y,z) update (accumulate ±$200, decay, clamp ±$600) is cartridge-faithful (ramp-under-hold, saturate plrotz at +$600, decay-to-0 on release, LEFT+RIGHT cancels). |

### boss8 footprint map
- **`boss8_Istrat`** (INIT): reads `currentlevel`($1FFD); writes `al_HP`
  (level-gated $20/$40), `al_AP`($08), `bossmaxhp`(bank-$70 $019A), `al_stratptr`
  (=boss8wait), `al_collflags`(enemy2\|enemyweap), `al_sbyte4`(=150),
  `al_sflags2`(clr sflag1\|sflag2), `gsvar_byte1`(=0); spawns 4 children
  (`s_make_childobj` cover + 3 nucleus beams); falls into `boss8_cont`.
- **`boss8_cont`** (common per-tick body): reads `player_posz`($1511),
  `gameframe`($15BB), `gsvar_byte1`($154F); writes `al_worldz`(=1680+player_posz),
  `al_sbyte4`(countdown/reload), `al_sflags2` sflag1($10, toggle),
  `gsvar_byte1`(±1 toward ±5). Tail `s_add_bossHP x,al_hp` (`$70:0170 += al_HP`)
  is a bank-$70 HUD accumulator, harmless to the object diff.

### REMAINING BOSS GAP (precise map)
Certified for `boss8`: INIT (+ child spawn count) + the common `boss8_cont`
per-tick body. **Not** certified (documented gap):
- **boss8 PHASE-TRANSITION machine** (`boss8wait` -> `boss8a` -> `boss8b` ->
  `boss8wait`): gated on the beam CHILDREN's sflag1 (set/cleared by the nucleus
  beams' own strats) + the `boss8a` HPLASMA fire + anim frames. Reaching the
  transitions needs the child family's per-tick beams ticked in lockstep both
  sides — the same "multi-child family" step deferred here. The common body all
  phases share IS certified; the phase-select + child-flag gates are the residual.
- **Other bosses**: `boss1` (barricader; phase strats fall through a GSU
  turret-repositioning tail `boss1rots_srou` — needs the aim/rotate GSU per tick,
  8 turret children), `boss2`/`bossg`/`bossseamon`/`bossA`/`bossF`/`bossH`
  (each a multi-child family + several with GSU-per-tick aim). The RECIPES for all
  of them are proven (RNG, player-relative, GSU aim, spawn, collision, and now a
  boss INIT + common-body state machine); each remaining boss is "locate by
  masked scan + seed the children + tick the family" — mechanical, not blocked.

### CERTIFIED VS RETAIL — running total: **15 named strats + AIMING + SPAWN/SEARCH + COLLISION + PLAYER-MOVE + BOSS8**
| Cert | Retail addr | Kind | Certified |
|------|------|------|------|
| `boss8` INIT (`boss8_Istrat`) | $07:919C | boss shell init + 4-child spawn | HP(level-gate)/AP/sbyte4/colltype/sflags/gsvar/stratptr/worldz == port `strat_boss8_init`; 4 children spawned, both difficulty branches |
| `boss8` per-tick (`boss8_cont`) | $07:93BB | boss common state machine | worldz view-track + sbyte4 countdown/reload + sflag1 toggle + gsvar ±5 speed ramp == port, tick-for-tick over 200 ticks |
| plrot* accumulator (`playermove_srou`) | $0B:DA79/DACA/DD8E | player steering tilt | step $0200 + clamp $0600 + plrotz/plroty addrs from retail bytes; decay == certified `strat_chase_proportional`; composed plrot(y,z) cartridge-faithful |

## UPDATE 13 — FOUR MORE BOSSES certified vs retail (`boss2` full + `bossg`/`bossseamon`/`boss1` INIT)

Extending the boss8 boss-certification recipe to the remaining boss families.
**Seven new tests, all green** (`coexec_retail` now **63**). `boss2` (Macbeth
spider / Venom1) gets the FULL treatment — INIT + a per-tick state-machine phase,
like boss8; `bossg`, `bossseamon` and `boss1` get INIT certification (with precise
body gaps documented). All boss addresses located by masked signature scan of the
retail cart (tail scalar-init / struct-offset anchors read out of the built ROM
via symbols.txt, WRAM/jml operands wildcarded), each a UNIQUE hit, cross-validated
by reading the installed strat/exp pointers + globals back out.

### Retail boss addresses located (all cross-validated)
| Boss | Istrat | Per-tick body | Exp | Shift | How |
|------|------|------|------|------|------|
| `boss2` | **$08:8BBE** | `boss2_strat` $08:8E3C | `boss2exp_Istrat` $08:9391 | +4 | tail scalar-init anchor (UNIQUE); reads back stratptr/exp/HP=$FF/AP=$0A/lifecnt=50 |
| `bossg` | **$04:EE35** | `bossg_strat` $04:EE85 | `bossgexplode` $04:F326 | 0 (bank $04) | HP=$FF/anim/sflags/collflags/mode/trigse anchor (UNIQUE); `maptrigger`=$176D |
| `bossseamon` | **$0A:F2D1** | `bossseamon_strat` $0A:F31E | `bossseamonexp` $0A:F675 | +$2A (bank $0A) | HP=2/AP=4/`jsl RANDOM`/roty/collflags/type/sbyte3/4 anchor (UNIQUE); RNG == `RANDOM_L` $02:FC58 |
| `boss1` | **$08:816E** | `boss1up_strat` $08:8413 | (backup slot) | +4 | roty/collflags/type/anim/sflags4/trigse anchor (UNIQUE); `currentlevel`=$1FFD, HPdef=$23 |

Newly-located retail: `playervel_z`=$14EA (built $1575 −$8B; boss2's
`s_keeprelto_player` leaf $1F:DB21 reads it), `maptrigger`=$176D, `AL_LIFECNT`=$0A
(boss lifetime/anim counter), `B2_SFLAG4`/`B2_SFLAG1` = `al_sflags2` $80/$10.

| New milestone (test) | Status | What it proves |
|------|------|------|
| `retail_boss2_addresses` | ✅ | Locates + cross-validates `boss2_Istrat`/`boss2_strat`/`boss2exp_Istrat` (UNIQUE tail anchor) + the state-0 near-branch globals (`playervel_z`/`pviewvelz` via the `s_keeprelto_player` leaf $1F:DB21). |
| `retail_boss2_init_vs_port` | ✅ **MATCH** | Runs the cart's OWN `boss2_Istrat` on a formatted pool and diffs the INIT scalar fields (HP=$FF, AP=10, lifecnt=50, colltype enemy1\|enemyweap, sflags2 colldisable + sflags shadow, stratptr=boss2_strat) + the **9-child spawn** (top + 4 petals + 4 turrets — free list shrank by 9) vs the port `strat_boss2_init`. |
| `retail_boss2_wait_body_vs_port` | ✅ **MATCH (GOLD)** | Runs the cart's OWN `boss2_strat` **state 0** (the wait/idle phase) on the near branch (child count 0, player near) over a horizon and diffs the STATE MACHINE tick-for-tick vs the port: `roty += 4`/tick, `sflags2` latches sflag4\|sflag1 (raw-diffable), `sbyte3`=2, and `worldz += playervel_z` (keeprelto_player + add_playerZ view-track). 2 scenarios (static worldz over 30 ticks + a −40/tick drift over 25) — MATCH every tick. sflag1 pre-set so the once-only `trigse` is skipped. |
| `retail_seaboss_and_boss1_addresses` | ✅ | Locates + cross-validates all of `bossg`/`bossseamon`/`boss1` (each UNIQUE), reading the installed strat/exp pointers + `maptrigger`/`RANDOM_L`/`currentlevel` operands back. |
| `retail_bossg_init_vs_port` | ✅ **MATCH** | Runs the cart's OWN `bossg_istrat` (CLEAN scalar init — no RNG, no children) on a seeded boss + a FAR player, so its mode-table fall-through (mode 0 = `.waituntilalmosthitplayer`) is a clean `worldz -= 40; return`. Diffs HP=$FF/AP=8/colltype enemy1/sflags shadow/stratptr=bossg_strat + the mode-0 `worldz`(−40) vs the port init + one `bossg_strat` tick. |
| `retail_bossseamon_init_vs_port` | ✅ **MATCH (partial)** | Runs the cart's OWN `bossseamon_istrat` (draws the RNG once, then falls into its player-relative body) and diffs the STABLE scalar init fields the body never touches — HP=2, AP=4, roty=deg180, collflags enemyweap, stratptr=bossseamon_strat — vs the port. The RNG-derived `sbyte2` + the player-relative fire-loop body are the documented gap. |
| `retail_boss1_init_vs_port` | ✅ **MATCH** | Runs the cart's OWN `boss1_istrat` (a self-contained RTL init — does NOT fall into the GSU body) on a formatted pool and diffs the **level-gated HP** (retail currentlevel 0→$23=35 / 1→$46=70, the boss8-class remap ↔ port 1/2) + AP=10/roty=deg180/colltype enemy1/type gnd/stratptr=boss1up_strat + the **9-child spawn** (8 turrets + 1 cover) vs the port `strat_boss1_init`, both difficulty branches. |

### boss2 footprint map
- **`boss2_Istrat`** (INIT): spawns 9 children (`s_make_childobj` ×9), then a clean
  scalar init (HP=$FF, AP=10, lifecnt=50, bossmaxHP=0, colltype enemy1\|enemyweap,
  sflags2 colldisable + sflags shadow, stratptr=boss2_strat, expstratptr=boss2exp).
  Ends RTL — the Istrat does NOT run the per-tick body (unlike boss8).
- **`boss2_strat` state 0** (wait/idle): counts children into `svar_byte5`; while
  count ≤ 7 sets sflag4, sflag1, sbyte3=2, roty += 2; the near branch (|dz| < 1100
  via PLAYPT) does `s_keeprelto_player` + `s_add_playerZ` (net `worldz += playervel_z`)
  + a final roty += 2; the far branch spawns RNG smoke (deferred). Reads
  `PLAYPT`→player Z, `playervel_z`($14EA), `pviewvelz`($14F4), the child-link chain.

### REMAINING BOSS GAP (updated precise map)
Certified: `boss8` (INIT + `boss8_cont`), `boss2` (INIT + state-0 wait body),
`bossg`/`bossseamon`/`boss1` (INIT). **Not** certified (documented per boss):
- **boss2 states 1..5** (leap / flip+slam / back-away / strafe-circle / topple+die):
  each is player-relative + spawns explosions/particles/lasers (RNG) and transitions
  on child liveness / player HP. The common state-0 body IS certified; states 1-5 +
  the smoke/laser child spawns are the residual (all recipes proven).
- **bossg mode table** (17-entry `s_mode_table`: scrollmsg / sf9e / runaway / appear /
  opentrunk+launchfish ×3 …): mode 0 (wait) is certified via the fall-through;
  the fish-launch + shadow-gen modes need the child family ticked in lockstep.
- **bossseamon** RNG-`sbyte2` + player-relative fire-loop body (states 0..5:
  dive/surface/fire `relslowElaser`): the stable scalar init is certified; the
  RNG draw (needs the firepillar param-block seeding) + the body are the residual.
- **boss1** phase strats (`boss1up_strat` → the GSU turret-repositioning tail
  `boss1rots_srou`, 8 turret children): the INIT (level-gated HP + 9-child spawn) is
  certified; the per-tick body needs the GSU trampoline per tick + the turret family.
- **Other bosses**: `bossA` (Andross), `bossF`, `bossH` — each a multi-child family,
  several with GSU-per-tick aim; recipes all proven, mechanical to extend.

### CERTIFIED VS RETAIL — running total: **15 named strats + AIMING + SPAWN/SEARCH + COLLISION + PLAYER-MOVE + 5 BOSSES**
| Cert | Retail addr | Kind | Certified |
|------|------|------|------|
| `boss2` INIT (`boss2_Istrat`) | $08:8BBE | boss shell init + 9-child spawn | HP/AP/lifecnt/colltype/sflags/stratptr == port; 9 children (top+4 petals+4 turrets) |
| `boss2` state-0 (`boss2_strat`) | $08:8E3C | boss wait/idle state machine | roty+=4 + sflag4\|sflag1 + sbyte3 + worldz+=playervel_z == port, tick-for-tick |
| `bossg` INIT (`bossg_istrat`) | $04:EE35 | clean sea-boss init + mode-0 tick | HP/AP/colltype/sflags/stratptr + mode-0 worldz(−40) == port |
| `bossseamon` INIT (`bossseamon_istrat`) | $0A:F2D1 | RNG sea-boss init (partial) | stable HP/AP/roty/colltype/stratptr == port (RNG sbyte2 + body = gap) |
| `boss1` INIT (`boss1_istrat`) | $08:816E | 9-child barricader init | level-gated HP + AP/roty/colltype/type + 9-child spawn == port, both branches |

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
