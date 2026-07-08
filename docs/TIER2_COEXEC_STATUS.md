# Tier-2 Retail Co-Execution — Status

**Goal:** certify the Rust port "100% vs the *retail* Star Fox cart" by running the
retail cart's own per-frame game logic on seeded state and diffing the object
array against the port **tick-for-tick** (not just per-function, which is tier-1).

Ground truth = `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM, repo root, gitignored).
This is a **different binary** from the built ROM (`sf-oracle/data/sf.sfc`), so
every retail address here was re-derived from the retail cart itself.

All harness code lives in `rust/sf-oracle/src/{lib.rs,retail.rs}` +
`rust/sf-oracle/tests/coexec_retail.rs` (**10 tests, all green**). Nothing committed.

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

### What remains — certifying a REAL (non-synthetic) enemy strat
`retail_dostrats_dispatch_vs_port` dispatches `addalvecs_l` **as** the object's
strat (a genuine ROM routine, through the genuine dispatch path), which exercises
the entire pipeline but is a pure motion routine. Diffing a full *named* enemy
strat (e.g. `torpedo_strat`, a ground/rock strat) additionally needs, per strat:
1. that strat's retail address (masked scan — the technique is proven), set as
   the object's `al_stratptr`;
2. the strat's OWN global footprint remapped retail-side (RNG state, player/
   camera object, timers) — many touch dozens of globals whose retail addresses
   shifted; each must be located like the `dostrats` set above; and
3. a port-side equivalent callable in isolation — the port's strats take `&mut
   Game` (full world context), not a lone `Alien`, so a `Game`-vs-retail-WRAM
   seeding shim is needed to line the two up. The pure per-strat helpers
   (`gen_vecs`, `speed_to`, `perc*`, `xzdiffs`) are already tier-1 oracle-verified
   and can be re-certified vs retail surgically (like `addalvecs_l`) if desired.

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
1. **Certify a named enemy strat** (e.g. `torpedo_strat`): masked-scan its retail
   address, set it as an object's `al_stratptr`, and diff vs the port — needs the
   strat's own global footprint remapped retail-side and a `Game`-context seeding
   shim for the port side (the port's strats take `&mut Game`, not a lone Alien).
2. **Exercise the spawn VM**: hand retail `mapobjdo` ($03:F79B) / `newobjs_l`
   ($03:EDA1) a minimal map script so retail SPAWNS objects (instead of
   hand-seeding), then diff the spawn output vs the port map builder.
3. **Re-certify the pure per-strat helpers vs retail** surgically (like
   `addalvecs_l`): `gen_vecs`, `speed_to`, `perc*`, `xzdiffs` — all struct-offset/
   pure-math, so byte-identical in retail and quick to scan + diff.
