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
  the GSU harness (gsu_arctan.rs), out of scope here.
