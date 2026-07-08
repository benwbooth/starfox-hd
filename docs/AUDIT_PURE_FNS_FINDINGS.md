# TIER-1 pure-function differential fuzz sweep — findings

Harness: `rust/sf-oracle/tests/fuzz_pure_fns.rs`. Each pure/leaf ROM routine is
executed from the built ROM through the `sf-oracle` 65816 core and diffed
bit-exact against its Rust port over a broad + boundary input grid, extending
the `mulslog_oracle.rs` pattern.

Run:
```
nix develop --command bash -c \
  "cd rust && cargo test -p sf-oracle --test fuzz_pure_fns 2>&1 \
   | grep -E 'test result|PROBE|DIVERGE'"
```
The default run is GREEN (5 passed, 2 ignored). The 2 `#[ignore]`d tests pin the
latent divergences below; run `-- --ignored` to reproduce them.

## Summary

- **Functions swept: 18** — `sr8_achase_alvar1..7` (7), `sr16_achase_alvar1..7`
  (7), `addvecs_l`, `addvecs2_l`, `addvecs4_l`, `add_objvecs_l`.
- **Bit-exact over the reachable input regime: 18 / 18.**
- **Latent divergences (boundary bugs): 2** distinct classes, both the same
  signed-negation-overflow family as the mulslog bugs, both confined to
  boundary inputs (assert-guarded out of the green suite, `#[ignore]`d + TODO).

### Top 3

1. **`sr8_achase_alvarN` vs `achase_angle` — antipodal (exact-180) turn flips
   direction.** Reachable. When the 8-bit signed angle gap is exactly ±128
   (target 180° from current), ROM and Rust pick OPPOSITE turn directions.
2. **`sr16_achase_alvarN` vs `strat_chase_proportional` — ±32768 diff flips
   direction.** Same class, 16-bit. Effectively unreachable (needs two 16-bit
   quantities exactly 32768 apart) but a real boundary bug.
3. **`sr8_achase_alvar7` (rate 7) min-step sign overflow.** ROM-side quirk:
   `2^7 = 128` is not a positive `i8`, so `nolessrange`'s `lda #128` loads
   `0x80 = -128` and corrupts the step sign for every input. Unreachable — no
   8-bit chase in the game uses rate 7 (ROM strat tables + `achase_angle`
   callers top out at rate 6).

---

## Per-function detail

### `sr8_achase_alvar1..7`  — ROM $1FD876.. (STRATROU.ASM:2763)
- **Rust equivalent:** `sf_strat::enemy_a::achase_angle` (enemy_a.rs:299), 8-bit.
- **ABI:** entry A(8-bit)=target, `mem[tpx=$3A]`=current; result → `mem[tpx]`.
  Body is `Achase_var2A` (STRATMAC.INC:525): `diff = target-current`;
  `nolessrange -(1<<N),(1<<N)` (min |step| 1); `REPT N adiv2` (signed halve
  rounding TOWARD ZERO, STRATMAC.INC:712); `new = current + diff`.
- **Input grid:** every `current` 0..255 × a boundary+stepped target set × rates
  1..7 (62,720 triples).
- **VERDICT:** bit-exact for **rates 1-6, any non-antipodal gap** (the entire
  reachable regime — 8-bit chases in ROM/Rust only ever use rates 1-6).
  - **DIVERGES @ |8-bit diff| == 128 (antipodal, all rates):** ROM computes
    `current + adiv2^N(target-current)`; `achase_angle` computes
    `current - adiv2^N(current-target)`. Algebraically equal EXCEPT at the 8-bit
    MIN diff -128, where `-(-128) == -128` overflows so the two forms turn
    opposite ways. e.g. `cur=0 tgt=128 rate1`: ROM=192, RUST=64 (both land 64
    from the target — convergence still happens, only the path/direction
    differs). Cite: STRATMAC.INC:525 (`sec/sbc` sign) + enemy_a.rs:303.
    Test: `sr8_achase_antipodal_divergence` (`#[ignore]`d).
  - **DIVERGES @ rate 7 (unreachable):** `nolessrange` `lda #(1<<7)` = `lda #128`
    loads `0x80 = -128`, so the forced min-step is negative regardless of the
    real gap sign. Not a Rust bug — a ROM-side quirk that never fires because
    rate-7 8-bit chase is unused.

### `sr16_achase_alvar1..7`  — ROM $1FD654.. (STRATROU.ASM:2740)
- **Rust equivalent:** `sf_strat::common::strat_chase_proportional`
  (common.rs:304).
- **ABI:** entry A(16-bit)=target, `mem[tpx]`=current(word); result → `mem[tpx]`.
  Same `Achase_var2A` body at 16-bit accumulator width.
- **Input grid:** 33 signed magnitudes (incl. `i16::MIN/MAX`, ±16384 and the
  exact values that make `diff = ±32768`) squared × rates 1..7 (7,623 triples).
- **VERDICT:** bit-exact for **every diff whose magnitude ≠ 32768**.
  - **DIVERGES @ |current-target| == 32768:** `strat_chase_proportional` does
    `diff = current.wrapping_sub(target)` as **i16**, then branches on
    `diff >= 0`. When the true diff is ±32768 the i16 wraps to `i16::MIN`, so the
    SIGN test uses the wrong sign and the chase steps the wrong way — the i32
    widening in the negative branch (common.rs:315) does NOT help, because the
    branch was already chosen from the wrapped i16. e.g. `cur=0 tgt=-32768
    rate1`: ROM=-16384, RUST=+16384. Cite: common.rs:308/312 vs
    STRATMAC.INC:525. Test: `sr16_achase_antipodal_divergence` (`#[ignore]`d).
  - **Fix (TODO, not applied — src not owned):** compute `diff` in i32 and branch
    on the i32 sign, for both `achase_angle` and `strat_chase_proportional`.

### `addvecs_l`  — ROM $1FC7B9 (STRATROU.ASM:497)
- **Rust equivalent:** `sf_strat::common::strat_add_to_pos` (common.rs:464).
- **ABI:** p=$20 (shorta/longi). X=alien base; `al_worldx/y/z += x1/y1/z1` (16-bit,
  wrapping). Grid: 7 positions × 8 vectors squared incl. `i16::MIN/MAX` wraps.
- **VERDICT: bit-exact.**

### `addvecs2_l` / `addvecs4_l`  — ROM $1FC7B1 / $1FC7A1 (STRATROU.ASM:491/482)
- **Rust equivalent:** none dedicated — the ×2 / ×4 pre-shift (`asl x1..` before
  the add) is inlined at call sites / folded into `apply_velocity`. Validated
  against a local spec = `strat_add_to_pos` with the vec `wrapping_shl(1|2)`.
- **VERDICT: bit-exact** vs `vec<<{1,2}` + add (confirms the shift-then-add
  contract, incl. wrap).

### `add_objvecs_l`  — ROM $1FD009 (STRATROU.ASM:1576)
- **Rust equivalent:** none dedicated (inlined at the few call sites). ABI: p=$20,
  X=obj2, Y=obj1; `obj2.al_vx/vy/vz += obj1.al_vx/vy/vz` (16-bit wrapping).
- **VERDICT: bit-exact** vs the `obj2.vel += obj1.vel` spec.

---

## Notes / non-targets

- Already covered elsewhere (not re-run here): `mulslog` (mulslog_oracle.rs, the
  original 3-bug find), gen-vecs 2d/3d/side/front (audit_trig, vector_family,
  gen_3dvecs), `perc56/62/75/87/93` (audit_trig), `sr_speedto` (audit_trig
  speedto_sweep + speedto.rs), SIN/COS tables (audit_trig), `xzdiffs_l`/`dist_xz`
  (audit_coldet), `addalvecs_l`/`apply_velocity` (apply_vel.rs), and achase rates
  2/3/4 small-case (audit_strats_b — this sweep widens it to all rates + all
  boundaries and split-buckets the divergence regimes).
- The `anglexy_*` / `arctan16` family cannot be validated through the 65816
  `call` harness: `arctan16_l` ($03:8550) is an `rtl` stub whose real body runs on
  the SuperFX (`runmario_l`), which this core does not execute — the ROM side
  returns a constant 0 (see audit_trig.rs `angle_xz_vs_rom`). Validate those via
  the GSU harness (gsu_arctan.rs) — done in BATCH 2 below, at the emulator-exact
  directions.

---

# BATCH 2 — GSU-side leaves + score/scale math

Harness: `rust/sf-oracle/tests/fuzz_pure_fns2.rs`. Same doctrine as BATCH 1, but
these leaves live on the GSU ("MARIO" chip), so they run through `sf-oracle`'s
**GSU core** (`gsu::Gsu::run`) instead of the 65816 `call`. GSU leaves that end
in `jmp r11` are returned by seeding `r11` with a scanned STOP address;
`mcall`-wrapper entries (`mcalcperc`, `mcallarctan16`) already end in STOP.

Run:
```
nix develop --command bash -c \
  "cd rust && cargo test -p sf-oracle --test fuzz_pure_fns2 2>&1 \
   | grep -E 'test result|PROBE|DIVERGE'"
```
Default run is GREEN (7 passed, 1 ignored). The `#[ignore]`d test pins the one
flagged (emulator-side, not port-side) divergence.

## Summary

- **Functions swept: 7** — `msqrt16`, `msqrt32`, `mcalcperc`, `calcstageperc`
  (whole `calc_stage_perc`), `framescalevecs`, `addvecs0_l`, `anglexy_l`/
  `arctan16`.
- **Bit-exact over the tested/reachable regime: 7 / 7.** No new *port* latents
  found — the ports that exist (`calc_stage_perc`, `strat_add_to_pos`,
  `strat_angle_xz`) are bit-exact against the real ROM math.
- **Flagged divergence: 1**, and it is an **emulator** (GSU-core) fidelity limit,
  not a Rust-port bug: `msqrt32` reads ~3 LSB low for inputs `>= 2^28`.

### Top findings

1. **`msqrt16` is bit-exact floor-sqrt over its ENTIRE 16-bit domain (all 65 536
   inputs)** AND the f32 distance path the port actually uses
   (`(x as f32).sqrt() as u16`) agrees with it bit-for-bit — swapping in the
   integer ROM routine would change nothing. Strong validation of the GSU core.
2. **`mcalcperc` (the hit-% divide) is bit-exact vs `calc_stage_perc`'s hit ratio
   over the FULL byte domain (65 280 dead×total pairs)**, and the whole
   `calc_stage_perc` (ratio + 5%/teammate + clamp-100) matches a ROM-faithful
   oracle (real GSU divide composed with the verbatim 65816 wrapper) across 576
   `(dead,total,teammates)` combos.
3. **`msqrt32` GSU-core divergence (FLAGGED, emulator not port).** A bit-by-bit
   integer sqrt is exact by construction, yet this GSU core reads low by up to
   ~3 LSB (growing with magnitude) for `x >= 2^28`, concentrated at `x = s^2-1`.
   Because `msqrt16` is exact everywhere, the defect is isolated to `msqrt32`'s
   wider 3-register shift (rol/sbc carry across the r0 top word) in `gsu.rs` — a
   suspected carry-propagation bug in the emulator. No Rust port exists (integer
   sqrt is unused — distances go through f32 `sqrtf`), so no game code is
   affected. Reproducer: `msqrt32_high_domain_divergence` (`#[ignore]`d).

---

## Per-function detail

### `msqrt16` — GSU $018058 (MMATHS.MC:48)
- **ABI (GSU):** in `rsqr=r3`; out `rsqrt=r6`. Leaf, returns via `jmp r11`.
- **Rust equivalent:** NONE dedicated (distance uses libm f32 `sqrtf`, e.g.
  enemy_a.rs / camera.rs). Validated as a ROM/emulator proof + a check that the
  float path agrees.
- **Input grid:** EXHAUSTIVE, all 65 536 u16 inputs.
- **VERDICT: bit-exact** integer floor-sqrt, and bit-identical to the port's
  `(x as f32).sqrt() as u16` on every input.

### `msqrt32` — GSU $018086 (MMATHS.MC:109)
- **ABI (GSU):** in `rsqr=r5` (low), `rsqrhi=r4` (high, `<=$7FFF`); out
  `rsqrt=r6`. Leaf, `jmp r11`.
- **Rust equivalent:** NONE (same f32 path). Input grid: perfect squares `s^2`
  and `s^2±1` for stepped `s<=46340`, plus 32-bit boundaries (2544 inputs, domain
  capped at `$7FFFFFFF`).
- **VERDICT: bit-exact for `x < 2^28`** (the realistic squared-distance domain).
  **DIVERGES (FLAGGED, emulator-side)** for `x >= 2^28`: reads low by up to ~3
  LSB, at `x = s^2-1`. Isolated to the GSU core's 32-bit shift chain (msqrt16 is
  exact), so a suspected `gsu.rs` carry bug, not a ROM or port bug. `#[ignore]`d
  reproducer `msqrt32_high_domain_divergence`.

### `mcalcperc` — GSU $01B6B2 (MTXTPRT.MC:355)
- **ABI (GSU):** in RAM `m_x1`=specials_dead, `m_y1`=specialobjtotal; out
  `m_x1 = floor(dead*100 / total)` (via `mcall mdivu3216`). Wrapper ends in STOP.
- **Rust equivalent:** the hit-ratio term of `sf_game::score::calc_stage_perc`
  (`specials_dead*100 / total`).
- **Input grid:** every `dead × total` over the byte domain the 65816 masks them
  to (`total` 1..=255, `dead` 0..=255 — 65 280 pairs; `total==0` is guarded
  upstream so the divide never sees it).
- **VERDICT: bit-exact.**

### `calcstageperc` — 65816 $02E7AD (MAIN.ASM:1031)
- **Not diffable through the 65816 core directly** (its ratio comes from
  `call_mario mcalcperc`, a GSU dispatch the 65816 core can't run). Instead
  composed a ROM-faithful oracle = the **real GSU `mcalcperc` divide** (proven
  above) + the verbatim 65816 wrapper (`+5` per living teammate, `cmp #100`
  clamp, MAIN.ASM:1037-1070) and diffed the WHOLE Rust
  `sf_game::score::calc_stage_perc` against it.
- **Input grid:** 12 `dead` × 12 `total` (incl. 0) × 4 teammate counts = 576.
- **VERDICT: bit-exact** — the full stage-% function matches the ROM divide +
  clamp, incl. the `total==0` (no-specials) branch and the >100 clamp path.

### `framescalevecs` — 65816 $0BEA90 (PSTRATS.ASM:3529)
- **ABI:** p=$20, X=alien; scales `al_vx/al_vy` by `framerate/4` via
  `new = adiv2^3( mulslog(vel<<8, framerate) )`. **No dedicated Rust port** — the
  fixed-step port makes it a no-op, correct only at base `framerate=4`.
- Pinned the EXACT ROM formula bit-exact against a spec built from the
  already-proven `mulslog` + round-toward-zero `adiv2`, over the full signed-byte
  `vel` domain × 12 framerates (0,1,2,3,4,5,8,15,16,24,32,60 — 3072 combos).
- **VERDICT: bit-exact** vs the mulslog+adiv2³ spec, and **identity at
  `framerate=4`** on every input — so the no-op port is correct at the base rate
  (extends framescale.rs from a spot-check to a full-grid proof of the contract).

### `addvecs0_l` — 65816 $01C7BB (STRATROU.ASM:493)
- The raw shared body of the addvecs family (entered with A already 16-bit;
  `addvecs_l/2/4` fall through here after their `a16`/shifts). ABI p=$00, X=alien;
  `al_worldx/y/z += x1/y1/z1`. Rust equiv: `strat_add_to_pos`. Grid: 6 positions ×
  6 vectors × 6 positions incl. `i16::MIN/MAX` wraps (216 combos).
- **VERDICT: bit-exact** (confirms the shared entry matches, complementing the
  BATCH-1 `addvecs_l/2/4` proofs).

### `anglexy_l` / `arctan16` — GSU $0181AA (MMATHS.MC:587/618)
- The aim angle every homing/aiming enemy uses: `anglexy_l` = `arctan16(dx,dz)`
  then the HIGH byte → 8-bit angle. **Rust equiv:** `sf_strat::common::
  strat_angle_xz` (f32 `atan2`, mapped 0..256).
- The GSU core runs the real ROM octant/quadrant + table code and is bit-exact
  vs `atan2` **only at the `dz=0` axis and the 4 diagonals** (marctan16's
  `deg90`/`deg45` special cases, no divide); the `dx=0` axis and all off-axis
  angles go through the shift-subtract divide **refinement**, which the GSU core
  still gets wrong (see gsu_arctan.rs) — so those are documented-BLOCKED, not
  diffed.
- **Input grid:** the 6 emulator-exact directions × 6 magnitudes (1..16000) = 36.
- **VERDICT: bit-exact** (`d==0` on all 36) at the trustworthy directions —
  `strat_angle_xz` reproduces `arctan16>>8` exactly there. Full-grid off-axis
  validation is blocked on the GSU divide-refinement fix (emulator WIP).

### Non-targets discovered
- **`ANGLEXZ` ($0012DD) is NOT a function** — it is an `alc`-allocated RAM
  variable (ALCS.INC:141, the current XZ angle), so there is nothing to diff.
- **`ADDALVECS_L`** already covered by apply_vel.rs (`apply_velocity`).

## GSU-core follow-up — `msqrt32` high-domain divergence: RESOLVED (ROM limit, not an emulator bug)

Harness: `rust/sf-oracle/tests/fuzz_pure_fns2.rs`
(`msqrt32_high_domain_is_faithful_16bit_overflow`, now GREEN — replaces the old
`#[ignore]`d `msqrt32_high_domain_divergence` reproducer).

**Prior hypothesis (WRONG):** the batch-2 sweep flagged `msqrt32` (GSU $018086,
MMATHS.MC:109) as reading low by up to ~3 LSB for x >= 2^28 (concentrated at
`x == s^2-1`) and suspected a **carry-propagation bug in a GSU opcode** in
`gsu.rs` (rol / sbc / the wide shift chain).

**Actual root cause:** an **inherent limitation of the ROM routine**, faithfully
reproduced by the emulator. `msqrt32` is a bit-by-bit sqrt that keeps its running
remainder in a **single 16-bit register** (`rt` = r8). The shift chain is
`rsqr`(r5,lo) -> `rsqrhi`(r4,hi) -> `rt`(r8): each iteration does
`add rsqr ; rol rsqrhi ; rol rt` twice, shifting 2 input bits up into `rt`. The
carry rolled out the **top** of `rt` has nowhere to go — there is no 17th-bit /
high-word register for the remainder (r7 `rt2` is only scratch for the test value
`2*root`). For 32-bit inputs the true remainder reaches ~9e7, and both `rt` and
the test value `rt2 = 2*root` (up to 92680) **overflow 16 bits once x >= 2^28**,
corrupting the compare/subtract by 1..3 LSB. `msqrt16` never overflows (root <=
255, remainder <= ~1020), which is exactly why it is bit-exact over its whole
domain — the defect is specific to the *width*, not to any opcode.

Because real GSU registers are 16-bit with no hidden 17th bit, **real hardware
produces the same low-by-1..3 result**. Making `gsu.rs` return exact floor-sqrt
would make the core UN-faithful to hardware — the opposite of what the ROM oracle
needs. So **`gsu.rs` is left unchanged**; there was no carry bug to fix.

**Proof (in-test, self-contained):**
- (a) A faithful 16-bit model of `msqrt32` reproduces the emulator **bit-exact**
  across the high domain (12,841 inputs, `emu==16bit-model diffs=0`) — confirming
  `gsu.rs` is a correct 16-bit GSU (`rol`/`add`/`sub`/`sbc`/carry all faithful).
- (b) The **identical** algorithm with **wide (un-truncated) remainder
  registers** yields exact floor-sqrt (`wide-model==floor diffs=0`) — proving
  register **width**, not carry handling, is the sole cause.
- The `add_flags`/`sub_flags` carry logic (gsu.rs:132-149), `ROL` (0x04), and
  `SBC`/`CMP` (0x60-0x6F) were audited against Super-FX semantics and are correct.

**Verdict:** GSU suite stays fully GREEN; `msqrt32` is now a *characterized*
faithful behavior (exact for x < 2^28, the only realistic domain; the >=2^28
quirk is a ROM property). **No Rust port is affected** — integer sqrt is unused
(enemy/camera distances use libm f32 `sqrtf`).

**Off-axis `anglexy`/`arctan16` — still BLOCKED (separate issue).** This fix does
NOT unblock the deferred off-axis `arctan16` divide validation: that divergence
is in the arctan shift-subtract **divide refinement** (a different routine/loop,
see gsu_arctan.rs), not the sqrt remainder path. It warrants the same
"inherent-16-bit vs genuine-emulator-bug" bisection performed here (compare the
emulator against a faithful narrow model and a wide model) before any gsu.rs
change is attempted; scoped as its own task.

---

## RESOLVED (2026-07-08): off-axis `arctan16` — real emulator bug (swapped BGE/BLT), fixed

The deferred off-axis `arctan16` divergence was **NOT** ROM precision and **NOT**
the divide refinement (`mdivu3115` @ $8192). It was a genuine GSU-emulator
control-flow bug: **SuperFX branch opcodes $06/$07 were swapped.**

### Bug
`rust/sf-oracle/src/gsu.rs` had `$06 => BLT (S!=OV)` and `$07 => BGE (S==OV)`.
The retail-built ROM (`data/sf.sfc`) encodes `blt marctan3` (`marctan16`, source
`MMATHS.MC`) as byte **$07** at $01:81ED. Correct SuperFX: **$06 = BGE (S==OV),
$07 = BLT (S!=OV)**.

### Proof (bisection, ROM as oracle)
Traced `arctan16(x=0, y=100)` (should be exactly 0 = arctan(0)). At $81ED the
`cmp r6` gives `r0-r6 = 100-0 = 100` → S=0, OV=0. The buggy emulator ran $07 as
BGE (S==OV → true) and **took** `blt marctan3`, skipping the mandatory
`mexg r6,r0,r4` operand swap. Result: the divisor `r6` stayed **0** through the
entire `mdivu3115` loop (verified in trace: `r6=0000` every iteration) → divide
by zero → garbage angle (232deg instead of 0). The algorithm *requires* the swap
whenever |y|>|x| so the larger magnitude becomes the divisor; on real hardware
(and every shipping SuperFX emulator, since the game aims correctly) byte $07
must branch on S!=OV. This is a control-flow divide-by-zero, categorically
different from the msqrt32 case (a few-LSB 16-bit-precision effect) — a real bug.

The flag computation itself (CMP/SUB setting S, OV, CY) was already correct; only
the two branch-condition assignments were transposed. Fix = swap them. The
project's own disassembler `tools/sf2/disasm/gsu.py` shares the same mistaken
$06=BLT/$07=BGE table (pre-existing; not touched here, but noted as wrong).

### Verification
- Fix: `gsu.rs` branch table now `$06=BGE (S==OV)`, `$07=BLT (S!=OV)`.
- Axis-aligned/diagonal cases (which early-exit and never hit this branch) stay
  bit-exact; **all 28 sf-oracle test binaries stay green** (incl. `gsu_rotmat`,
  an independent GSU routine — confirms the swap is globally correct, not a
  local hack).
- New `arctan16_off_axis_grid` (575 off-axis (x,y) points): GSU vs `atan2`
  **max 16-bit delta = 51/65536**, **max 8-bit delta = 1**. The 51-unit 16-bit
  residual is genuine ROM table quantization (`arctantab` = 512 entries fed by
  `quotient>>5`), i.e. real ROM precision, not an emulator defect.

### Q1 verdict — EMULATOR: real bug, fixed
Not faithful-ROM-precision: a swapped-branch divide-by-zero. Minimal 2-line fix
in `gsu.rs`; off-axis now matches atan2 to ROM-quantization precision.

### Q2 verdict — PORT: VERIFIED within +/-1 8-bit unit
`sf_strat` `angle_xz` / `strat_angle_xz` compute `atan2(dx,dz)*256/(2*PI)`
truncated to u8 — the ROM's `arctan16>>8` convention (256 = full circle). New
`arctan16_matches_port_angle` compares the port's exact float formula against
ROM `arctan16>>8` over the off-axis grid: **max delta = 1 8-bit unit**
(worst e.g. x=-4000,y=-300: ROM=189, port=188). This is the intended, acceptable
float-vs-fixed boundary-rounding difference (libm `atan2f` truncated vs the ROM's
quantized table). Enemy aiming is VERIFIED-within-precision; no port divergence.

### Gameplay impact
None on the shipping C/Rust port (which uses libm `atan2f`, always correct). The
bug lived only in the GSU *oracle*, so it had blocked *validation* of GSU angle
math, not gameplay. It is now unblocked: enemy-aiming angles are proven correct
against the real ROM routine to ±1 in the 8-bit units the game uses.
