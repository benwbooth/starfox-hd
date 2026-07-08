# Tier-2 Retail Co-Execution — Status

**Goal:** certify the Rust port "100% vs the *retail* Star Fox cart" by running the
retail cart's own per-frame game logic on seeded state and diffing the object
array against the port **tick-for-tick** (not just per-function, which is tier-1).

Ground truth = `Star Fox (USA) (Rev 2).sfc` (1 MB LoROM, repo root, gitignored).
This is a **different binary** from the built ROM (`sf-oracle/data/sf.sfc`), so
every retail address here was re-derived from the retail cart itself.

All harness code lives in `rust/sf-oracle/src/{lib.rs,retail.rs}` +
`rust/sf-oracle/tests/coexec_retail.rs` (6 tests, all green). Nothing committed.

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

## Precise summary

**We can diff X but not Y because Z:** we CAN run the retail cart's own object
allocator + real per-object motion integrator on seeded state and diff the whole
object array against the port tick-for-tick (MATCH). We CANNOT yet run the full
`dostrats` AI tick, because (Z) the GSU is reached via a RAM-resident
`runmario_l` trampoline that isn't populated on a directly-called (non-booted)
bus, and the remaining per-tick routines (`dostrats`/`do_strat_l`/spawn VM) still
need their retail addresses re-derived by signature scan.

## Recommended next steps
1. Inject/relocate the retail `runmario_l` stub into WRAM (or bus-intercept the
   `JSL` to it → `gsu_kick`) so GSU-using strats run.
2. Signature-locate retail `dostrats` / `do_strat_l`; drive a single seeded
   object through one real strat tick with the GSU live; diff vs the port strat.
3. Signature-locate `newobjs`/`mapobjdo` and hand a minimal map script so retail
   SPAWNS the objects (instead of hand-seeding), then diff the spawn output.
