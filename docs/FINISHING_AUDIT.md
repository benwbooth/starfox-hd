# Retail fidelity audit — 2026-09-04

Status: the port is not certified. This is a review of the verification
architecture and selected source paths, not a claim to have reviewed every
Rust function. The removed C implementation is historical evidence; the pinned
retail ROM is the final behavioral authority.

## Findings that change the finishing strategy

| Priority | Finding | Evidence and consequence |
| --- | --- | --- |
| P0 | Runtime timing replays one neutral-input recording | `rust/sf-game/src/gameplay_timing.rs` indexes two 983-element arrays by Corneria frame number, independently of input, objects, or render work, then falls back to four refreshes. Recorded timing is useful oracle evidence but cannot establish correct timing for different play. Replace this with source-derived state/workload timing and test different input tapes. Keep actual retail elapsed time as an explicit input in isolated routine tests, not as copied gameplay state. |
| P0 | Independent semantic verification was outside the advertised gate | `scripts/verify_retail_parity.sh` previously omitted Mesen. It now invokes it and the comparator tests before the Rust suite. A failure is retained, not interpreted as a completed release. |
| P0 | Similar byte windows were accepted as path correspondence | `verify_corneria_semantic_oracle.py` previously guessed a nearby path offset. The verifier now rejects differing catalogs without a verified mapping; byte similarity remains a diagnostic suggestion only. The required replacement maps exact source instructions and operands to native path instructions, with ROM/catalog hashes and boundary checks. |
| P0 | Restart capture mixed object-pool epochs | At scene 944, the retained Mesen artifact declares six active objects but emits no object records: its draw-list assertion refers to the preceding scene's objects. The native trace emits the rebuilt pool. The verifier now checks each capture's inventory independently and fails on this missing evidence. Capture simulation and completed drawing at distinct boundaries with explicit object generations. |
| P1 | All-route completion is an assisted soak | `rust/sf-app/tests/full_route_sim.rs` restores durability, suppresses death, and applies synthetic damage. Preserve it for progression testing, but require controller-only retail/native route replays for combat, death, checkpoints, and endings. |
| P1 | Shapes, pixels, and sound are not jointly certified | The Mesen semantic comparator still excludes source/native shape encodings and the raw pointer; it does not compare ordered audio events, PCM, or native raster output. Those are explicitly outside its current claim. Add exact asset/strategy identity mappings and independent presentation channels. |
| P1 | Byte-identical rebuild is not full reconstruction | The roundtrip manifests currently assemble 24 SF1 bytes and zero SF2 bytes; the remaining bytes come from the input ROM. Keep this as an integrity check and report promoted source bytes separately from behavior coverage. |
| P1 | Architecture checks do not enforce all source-style rules | `tools/check_native_architecture.py` passes in the reviewed tree, but does not enforce named constants/enums or decimal notation across the port. The native build graph and feature boundaries also need verification beyond source regex checks. |

## Live verification from this audit

- Fresh `cargo test --workspace` reached `sf-oracle/tests/semantic_trace.rs`
  and failed two tests. The independent integrated result reported
  `fields.timing.motion_refreshes` at sequence 892: reference 4, native 3.
  The frozen native semantic hash also failed at sequence 892. No fixtures
  were blessed. Later workspace targets were not run after Cargo stopped.
- The independent Mesen executable initially failed because its X11 runtime
  dependency was missing from the development environment. The flake now
  supplies Mesen's X11 and ICU dependencies in the dev shell only.
- A fresh retail restart trace confirms that the collision-bank response runs
  while the control lock skips lateral movement. `PSTRATS.ASM` at
  `player_collmove`, `.no_pctrl`, and the intervening left/right movement
  block explains the behavior. Rust now respects that gate. Focused tests
  cover ship-control, black-screen, wipe, and recovery locks without clearing
  the collision state.
- A fresh native replay now has player X=0 at scene 945, matching retail,
  where the previous native capture had X=2. This is evidence for the specific
  fix, not a full 1–983 parity claim: incomplete scene-944 drawing evidence
  and unverified path correspondence still block that claim.
- Temporary restart diagnostic prints were removed. The architecture check
  and the new comparator/inventory rejection tests pass. An export of only the
  staged files passes both focused player tests and all 19 verifier tests.
- A separate gameplay-package run passed the game targets and then failed the
  legacy C `ponpon` path trace on collision flags (64 versus 80). This is
  recorded as an unresolved fixture/source-contract discrepancy; the expected
  trace was not regenerated from Rust.

Local detailed evidence: `/tmp/sf1-workspace-audit-20260904.log`,
`/tmp/sf1-restart-review-fixed-env-20260904/`, and
`/tmp/sf1-native-control-gate-20260904.txt`. These paths are local diagnostic
artifacts, not reproducible release fixtures or distributed ROM data.

### Independent early-scene recheck — 2026-09-19

The new read-only `sf1_timing_boundary` example reproduces the integrated
oracle's handoff and logs actual refresh-counter writes. At scenario tick 892,
both games have completed strategy scene 1, but the in-tree `RetailMachine`
reports 4 refreshes while native Rust publishes 3. Differences also remain at
later routine-aligned boundaries, so sampling alignment alone does not explain
the in-tree oracle's cadence.

A fresh independent Mesen 2.1.1 capture of scenes 1–12 reports 3 refreshes for
completed scene 1 and 2 at that scene's strategy entry. These agree with the
native boundary handling and the retained Mesen timing record. All twelve
scenes pass `verify_corneria_semantic_oracle.py`, including its object-inventory
checks and all fields within that comparator's declared coverage. This does
not certify source/native shape identity, rendered pixels, audio, later scenes,
or input-dependent pacing. It establishes that changing native scene 1 to the
in-tree oracle's value would contradict the independent reference.

Reproduce the in-tree diagnostic with:

```sh
nix develop --command cargo run --manifest-path rust/Cargo.toml \
  -p sf-oracle --example sf1_timing_boundary
```

The integrated semantic test and its frozen fingerprints remain unchanged and
failing. Their timing source needs independent correction; this result must
not be used to remove the counter comparison or bless new native-only hashes.
Local artifacts: `/tmp/sf1-timing-boundary.log`,
`/tmp/sf1-mesen-handoff-capture.txt`, and `/tmp/sf1-native-handoff.txt`.

A subsequent fresh capture reaches scene 983. Its full comparison still fails
the object-inventory gate at restart scene 944 (six active slots have no object
records). Before that boundary, scenes 1–191 pass the existing semantic
comparator; scene 192 requires a verified path-cursor mapping (retail 13864,
native 13867 for slot 28). Neither gate was bypassed. The extended capture and
native output are retained in `/tmp/sf1-independent-scenes-zJ1L1s/`.

### Reference-bus correction and early-scene recheck — 2026-10-06

A new independent Mesen 2.1.1 capture confirms completed-scene refresh counts
`3, 3, 3, 4` for neutral Corneria scenes 1–4, with entry motion `2, 3, 3, 3`.
The in-tree runner omitted the 40-master-clock DRAM-refresh stall on every
scanline. Its horizontal safe-window polling could consequently take several
display frames. The runner now includes the phase-aligned refresh stalls and
restricts FastROM acceleration to the high-bank cartridge windows, following
the independent [Mesen memory-manager implementation](https://github.com/SourMesen/Mesen2/blob/b9fa69ddc6d0a331fb103fdb5eef6904305703c2/Core/SNES/SnesMemoryManager.cpp).
The raster and Super FX continue during CPU stalls. Five new bus tests cover
exact boundaries, repeated and DMA-spanning refreshes, all bank-speed classes,
and the original horizontal-wait loop at all 682 two-clock start phases.

The timing probe now follows the original Corneria game-start and first
counter-reset entries rather than assuming tick 890. It includes the first
completed scene instead of skipping it. The new independent-reference test
executes from reset without state injection and passes in debug and release.
Its four sampled counts agree with Mesen; exact master-clock durations still
differ. This does **not** certify the rest of the hardware runner or replace
the production timing recordings.

This correction also changes existing reference results. The integrated SF1
semantic gate now exposes an earlier title/attract phase difference at tick
127; the fixed-boot title, launch-video and weapon comparisons fail. The three
observed-pass SF2 opening comparisons now fail on glyph position at update 101,
so observed actor-pass partitions alone no longer establish that integration.
Their expectations and the separate ignored autonomous gate remain unchanged.
The native Corneria hash still first differs at tick 1500 and the controller
route still loses the player at level frame 1499. Do not treat the corrected
early timing counts as closure of any of these gates.

Reproduce the bounded independent capture with the existing Lua script, using
`SF1_MESEN_CORNERIA_INPUT=neutral`, `SF1_MESEN_CORNERIA_FIRST_SCENE=1`,
`SF1_MESEN_CORNERIA_LAST_SCENE=4` and `SF1_MESEN_CORNERIA_GSU_JOBS=1`.
The ROM SHA-256 is
`82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548`.
The Mesen executable SHA-256 is
`6a0d2e16708adf65f468e3a36edf2009e85bdb747a536b960b0ea642f9f065c1`.
Local evidence: `/tmp/sf1-timing-mesen-oct06.txt`,
`/tmp/sf1-timing-retail-source-handoff-oct06.log`,
`/tmp/sf1-refresh-independent-test-oct06.log`, and
`/tmp/sf-oracle-refresh-full-oct06.log`. These are diagnostic artifacts, not
distributed game assets or whole-game certification.

### Opening entropy ordering and independent actor recheck — 2026-10-06

The SF2 update-101 difference was not a new glyph-motion arithmetic error.
The original can call its background entropy service **inside** an actor's
visit. The read-only instruction-entry probe captured the service between
the lower-case E's two direction draws and two spin draws. Counting only
completed actor visits placed that refresh before the actor instead. Both
orders finish with the same RNG state, but assign different values to motion.

The typed opening now accepts refresh boundaries measured in consumer draws,
through a scoped native random source shared by logo, burst and chain code.
It still generates every value itself. This supports intra-actor ordering
without injecting observed values, coordinates, allocations or results.
The existing coarse between-actor API remains available, with its limitation
documented. Repeated/zero boundaries, tail refreshes, spin-versus-direction
separation and capacity-failure rollback have regression coverage.

All three original-code opening integration checks again pass 440 updates,
including consumer draw counts, actor poses, active-list order, camera and
palette/publication checks. They now explicitly supply observed entropy order,
not a misleading actor-pass approximation. The separate autonomous timing
gate remains unchanged and ignored; shipping `tick()` still uses its coarse
tail refresh and is **not** certified by these observed-order tests.

An independent Mesen 2.1.1 run also passes a new native comparison for all
440 updates: every active slot, actor position/rotation, camera and final RNG
state. Its refresh boundaries differ from the in-tree runner. In particular,
at update 164 it refreshes after two of one burst actor's eight draws. This
confirms that mid-actor ordering is a real source contract, not an in-tree
timing artifact. The comparison supplies only the initial RNG seed and refresh
order. Palette, pixels, audio and autonomous timing are outside this separate
independent comparison; it does not certify all opening presentation.

Reproduce the independent capture and comparison:

```sh
entropy_profile=$(mktemp -d /tmp/sf2-opening-entropy.XXXXXX)
nix develop --command python3 tools/sf2/run_mesen_oracle.py \
  tools/sf2/mesen_opening_entropy_oracle.lua \
  --profile "$entropy_profile" --timeout 180
nix develop --command cargo run --manifest-path rust/Cargo.toml \
  -p sf-oracle --release --example sf2_opening_entropy_trace -- \
  "$entropy_profile/Mesen2/LuaScriptData/mesen_opening_entropy_oracle/sf2_opening_entropy.txt"
```

The capture rejects nested generator arithmetic rather than pretending it is
an atomic draw. The comparator rejects incomplete traces and duplicate fields,
and checks the exact consumer draw count. The SF2 ROM SHA-256 is
`e134f20f6ee7d422d06faea6b1ae1e4101d1d0a200a0571d715d1cf23d959e8c`.
The local capture is
`/tmp/sf2-opening-independent-oct06.AnZFSR/Mesen2/LuaScriptData/mesen_opening_entropy_oracle/sf2_opening_entropy.txt`
(SHA-256 `f3077e605d35a511541a1ff149b5636e23090cfb9f8f6110fd94fb133e20bc6f`).
All 1,240 native tests and two native architecture tests pass in debug/release,
as do 41 focused oracle/parser tests (one pre-existing autonomous gate remains
ignored), the independent Mesen comparison in both profiles, all 616 SF2 source
checks, the architecture audit and all three app launcher builds.
Other evidence: `/tmp/sf2-opening-visits-oct06.log`,
`/tmp/sf2-opening-independent-mesen-oct06.log`,
`/tmp/sf2-entropy-final-counted-checks-oct06.log` and
`/tmp/sf2-entropy-static-oct06.log`.

### SF1 Training handoff — 2026-10-06

The corrected reference exposed a real duplicate movement reset. Training's
`BGS.bg_training_1` uses the `pstrat` macro to queue `playeronplanet_Istrat`;
`WORLD.setbginforeq_l` installs that pointer without rerunning
`PSTRATS.playermove_init`. The native Training branch nevertheless called
`player_move_init` after its base-player transfer pass, replacing camera speed
64 with 65. Removing that extra call preserves the following source speeds
and view depths (63, 107, then 63, 165). Other initialization remains intact.

Two new native tests cover the completed base-player handoff and retained
movement/weapon fields at signed velocity boundaries. A new differential test
executes the unmodified source background-request routine for all 256 seeded
byte values, checks the installed strategy pointer, and compares retained
speed, rotation and weapon state. Two source-contract checks bind this test
to the actual Training map macro and base/planet initializer call order.

The existing complete first-course gate now passes unchanged in debug and
release: 1,758 semantic/draw/audio-event updates, 1,752 source-bitmap updates,
129 object births, all thirteen required shape families and the course restart.
Each bitmap comparison covers the 224-by-152 scene region (34,048 pixels), not
the lower radio/HUD region or a newly independent Mesen full-screen capture.
No expected state or fixture was regenerated.

All 1,553 game/path/strategy tests, all forty original-instruction strategy
tests, the complete Training gate and all three app builds pass in both
profiles. All 58 SF1 source checks and the architecture audit also pass.
Evidence: `/tmp/sf1-training-handoff-final-oct06.log`. These checks used the
current working tree, retaining its unrelated pre-existing changes. This
closes the Training velocity failure, not the front-end timing, controller
route or remaining whole-game gates.

## Work order

1. **Establish trustworthy boundaries.** Split simulation snapshots from
   completed draw/audio output; record object generation plus allocation order.
   Pin input, ROM, catalog, and adapter versions. Compare every update in a
   scenario. Reject missing records, duplicate fields, guessed mappings, and
   skips. Preserve both raw captures and the earliest causal difference.
2. **Review shared semantics before individual enemies.** Recover contracts
   for signed widths, wrapping arithmetic, shifts and rounding, state-machine
   fallthrough, control gates, flag layout, allocation/freeing, parent/child
   relationships, collision dispatch, and timing. Use exhaustive tests for
   small domains and deterministic boundary/fuzz inputs against retail for
   larger routines. A C translation is supporting evidence only.
3. **Make the next source behavior reachable by normal input.** Expand replay
   manifests from boot through each route, difficulty, special exit, boss,
   death, restart, continue, and ending. Include observe, hit, destroy, evade,
   and parent/child interaction variants. Save legal retail checkpoints to
   reduce iteration time; replay the full route after local repairs.
4. **Close presentation independently.** Compare source-resolution completed
   frames, exact asset identity and draw order, dialogue/UI layout, ordered
   music/SFX events, and intended PCM output. Keep HD interpolation and optional
   visual effects outside the authoritative simulation and separately tested.
5. **Track proof, not ported-function counts.** Each source behavior needs its
   Rust location, reviewed contract, independent test, reached branch/event
   evidence, and current pass/fail result. Distinguish implemented, reviewed,
   routine-tested, controller-replay-tested, and presentation-tested. A green
   row cannot be inferred from a build, a native fixture, or an assisted soak.

Completion remains the release conditions in `RETAIL_PARITY_PLAN.md`: no
unexplained differences across the declared full corpus and coverage, an
unassisted runtime gate, architecture compliance, and a verified published
revision. SF2 certification follows SF1. Finite test runs alone do not prove
equivalence for every possible input sequence; universal claims require
stronger routine contracts/proofs as well as measured coverage.
