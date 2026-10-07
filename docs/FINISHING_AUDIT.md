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

### Source-bound SF1 launch and warning branch — 2026-10-06

The launch gate now enters both games through their actual first Corneria
strategy boundary, starting from reset with ordinary controller input. It no
longer assumes boot tick 900 or pauses native updates to force agreement.
The first player position, velocity and camera state match independently;
the launch gate additionally checks every camera through its final anchor.

The previous one-update scanout assumption was also invalid after the reference
bus correction: at one strategy boundary the original was still displaying
scene 6 while the test expected scene 7. The gate now associates the original
completed bitmap with its own completed BG1 scanout, including the unwindowed
indices outside the aperture. Native pixels never select the reference frame.
Malformed, partial and mismatching original bitmaps are rejected; full-screen
RGB comparison remains strict after association.

That exposes a real native warning error. Retail Rev 2 `prt_scramble` reads its
counter at `$15B5`, not the oracle's former `$15B7`, and returns when the low
three game-frame bits are less than **or equal to** three. Its visible phases
are therefore 4–7. The modified reconstructed source's branch was not reliable
for this cartridge behavior. The Rust warning now uses the executed retail
branch and skips formatting while an aperture is active, as `do_sprites_l`
requires. An original-instruction gate covers every counter byte at sixteen
phases and every complete frame word: 69,632 cases. Both original-associated
launch anchors (7 and 20) now pass all 57,344 RGB pixels, and the warning's
separate 1,671-pixel layer check passes.

This does **not** close all launch timing. A diagnostic still shows the native
warning countdown decrementing one update early after the wipe: scene 20 is
49 versus the original's 50. Its terminal-count capture and interpolation
also require source-ordered publication, not another recorded delay. The old
claim that scene 21 proves a stale warning with a zero source count was false:
it read the wrong counter address. The corrected gate verifies the real active
counter and sprite layer instead.

The title diagnostic now completes its 96 strategy/object/draw comparisons
before asserting setup duration; those semantic checks pass, but the existing
setup-duration gate still fails (native 130 versus reference 128 ticks). Its
embedded video diagnostic first differs at update 13. No expectations were
blessed and no timing failure was disabled. Other route, production-timing and
SF2 ownership blockers remain as listed in the completion plan.

Verification passes 495 core/game/render tests, 28 oracle library tests and
ten focused launch/entry tests in each of debug and release. The unchanged
release Training replay still passes 1,758 semantic/draw/audio updates and
1,752 scene-region bitmap comparisons. All three app launchers build in both
profiles; all 58 SF1 source checks and the native architecture audit pass.
These results cover the final working tree with its preserved unrelated
changes, not an isolated export of this commit.

Local evidence: `/tmp/sf1-launch-banner-regressions-oct06.log`,
`/tmp/sf1-launch-final-release-oct06.log`,
`/tmp/sf1-launch-candidate-diagnostic-oct06.log`,
`/tmp/sf1-launch-banner-source-oct06.log`, and
`/tmp/sf1-boundary-diagnostics-final-oct06.log`.

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

## Corneria map-owned wipe and display publication (2026-10-06)

Corneria no longer substitutes `initblack_l` call counts for the map's actual
aperture request. Its three map builders retain the unfinished
`nofadenostage` initializer tail, and the outdoor handoff requests the distinct
horizontal reveal. The native transfer sequence separates request selection,
record rendering, end-marker cleanup and sprite-lock release. SCRAMBLE uses
the pre-decrement count and keeps its prepared sprite list between transfers,
including its final visible count-one update. The aperture has priority over
the black color window; the latter publishes its newly prepared color rather
than a one-update-old value.

The display fader is a separate typed level/brightness/blank state. Original
`IRQ.setinidisp` execution checks 65,536 combinations of direction byte, legal
level, game-frame parity/wrap and prior visible display. The source-bound
80-update replay additionally checks every launch aperture record and cleanup,
black-window lifetime/intensity after the initializer resumes, once-only flag,
sprite lock, warning countdown/publication and display brightness/direction.
The video test checks all 57,344 pixels in each of sixteen scenes, 5 through 20,
using original-bitmap-to-original-scanout association; it never selects a
reference by looking at native pixels. The independent warning-layer check
still covers 1,671 opaque pixels.

The reference PPU's old RGB8 `(brightness + 1) / 16` operation was incorrect.
It now applies the independently documented Mesen five-bit
`component * brightness / 15` operation before expansion. See
[Mesen's pinned implementation](https://github.com/SourMesen/Mesen2/blob/b9fa69ddc6d0a331fb103fdb5eef6904305703c2/Core/SNES/SnesPpu.cpp#L1382).
`tools/sf1/mesen_launch_display_oracle.lua` captures a separate reset/input run
without state writes. It confirms the inspected reveal colors, including
RGB `(140,189,198)` at brightness 12, but does **not** certify the full raster:
the inspected scene-five crop differs on 53 pixels, scene six on 36 and scene
nineteen on 30. The Mesen output is 256-by-239, with the matching scene rows
offset by six; this diagnostic alignment is not an accepted golden fixture.
Window/scanout boundaries and the reference's missing general color-math
implementation need their own independent verification.

Scope limits remain explicit: the first two launch updates still omit the
pre-resume source black-window hold; only the three Corneria initializers use
this new transfer fade owner. Other initializer schedules, restart details,
production source timing, full-screen independent comparison and complete
campaign coverage are not established. Map fixtures were updated only for the
source-reviewed horizontal operand and appended 24-byte continuation; the
500-tick native regression changed only its retained horizontal command value.
These fixtures are regression checks, not original-game evidence.

Verification on the current working tree: the original fade, warning, launch
video and 80-update wipe gates pass in debug and release, as do the core/game/
render library suites, map suites, renderer integration checks and both
500-tick native traces. The unchanged release Training gate still passes
1,758 semantic/draw/audio updates and 1,752 bitmap comparisons. All 62 SF1
source checks, the architecture audit and all three application builds in
both profiles pass. The complete release workspace run reports 3,898 passing
tests, three failing tests and one ignored test; its two failing executable
examples bring the failed-target count to four: `semantic_trace`,
`sf1_corneria_route`, `sf1_title_trace` and `sf1_weapon_trace`. The title still
enters two sampled ticks late; the weapon trace fails at the gameplay-entry
boundary. These existing failures are not waived. This run includes unrelated
pre-existing working-tree changes; it is not a clean-revision certification.

## Independent launch scanline and sprite boundary (2026-10-06)

The independent aperture discrepancy above is now resolved for all sixteen
launch scenes. Every byte of Mesen's sixteen 896-byte normalized aperture
buffers matches the original-program runner, and the native spans match all
192 used rows of each record. The error was in presentation: the verification
PPU applied the next line's H-blank writes before rendering the preceding
line. The native source renderer had acquired a compensating minus-one-row
wipe offset. The reference now completes a line before publishing its next
H-blank settings, and the native offset is removed. Flight display blanking
retains visible hardware lines 17 through 206. A bus-level test independently
guards the H-blank publication order and permanently blank hardware line zero.

An additional original sprite difference was previously hidden by both
implementations: a sprite's stored Y identifies the row before its first
visible pixel. The reference sprite samplers now account for the wrapped
one-row delay, and the native sprite layer places its first visible row
accordingly in both source-resolution and scaled output. The warning begins
at line 73, not 72; its 1,671 opaque pixels are unchanged. The sprite sampler
test exhausts all stored/display Y bytes with both vertical-flip choices.
These conventions are independently visible in Mesen's original-game register
trace and pixels, and in its
[line/sprite publication](https://github.com/SourMesen/Mesen2/blob/b9fa69ddc6d0a331fb103fdb5eef6904305703c2/Core/SNES/SnesPpu.cpp#L364)
and [H-blank scheduling](https://github.com/SourMesen/Mesen2/blob/b9fa69ddc6d0a331fb103fdb5eef6904305703c2/Core/SNES/SnesMemoryManager.cpp#L213).

Run the repeatable independent gate from the Nix development shell:

```sh
nix develop --command python3 tools/sf1/run_launch_video_oracle.py
```

It pins the original ROM, starts two disposable Mesen profiles with every
controller button specified and frame skipping disabled, and requires
byte-identical manifests, RGB captures, VRAM and aperture buffers. The native
comparison selects a settled reference from two consecutive identical original
scanouts containing the complete original BG1 bitmap. Native pixels never
participate in selection. Mesen's documented `scanline + 6` non-overscan output
placement maps the diagnostic hardware-line 0..223 view; the crop is no longer
chosen by image similarity. All 57,344 pixels in each scene 5..20, plus all
sixteen aperture buffers, pass this independent gate in debug and release. Captures are
retained in temporary evidence directories, not shipping assets or expected
native fixtures. This verifies these rendered scenes, not elapsed production
timing, all later scenes or full reference-PPU behavior.

The unchanged release Training gate still passes all 1,758 semantic/draw/audio
updates and 1,752 bitmap comparisons after these shared changes. The renderer
library and GPU checks, 64 SF1 Python checks, architecture audit and all three
application builds pass. The full release workspace run records 3,902 passing
tests, three failing tests, one ignored test and two failing executable
examples. The four failing targets remain `semantic_trace`,
`sf1_corneria_route`, `sf1_title_trace` and `sf1_weapon_trace`: fixed-boot
phase/checkpoint assumptions, controller-route player loss, title setup
duration and weapon-entry alignment are still open. These results include
preserved pre-existing working-tree edits, not a clean-revision certification.

## Source-bound first laser and independent scanouts (2026-10-06)

The first-laser gate no longer treats a recorded boot tick as gameplay entry.
Both implementations reach the actual first Corneria strategy visit through
their own front ends and legal controller input, without state injection.
Every one of 338 strategy updates checks the game counter and the original
latched controller word. The physical buttons for the next visit are presented
throughout the current transfer: `IRQ.getcont0` can run before the draw-list
boundary. Setting those buttons only after drawing missed a release sample
and produced an extra original shot. This was a harness input error, not a
reason to change native weapon logic or bless a new expected trace.

The gate now passes ten laser-life updates including allocation/list order,
position, velocity, rotation, owner, lifetime, damage/collision fields and
animation; the camera, laser draw commands and one firing sound also agree.
All 26 full scene draw lists and composed frames 312..337 pass. An additional
independent gate uses two fresh Mesen resets, checks its latched pad at every
strategy visit from scene zero through 340, and requires byte-identical
manifests, RGB frames and VRAM. It compares all 57,344 pixels per scene, selecting
the reference solely from complete original BG1 bitmap identity and two settled
original scanouts. It does not use the older fixed-three-display-frame capture
delay or native pixels for alignment.

```sh
nix develop --command python3 tools/sf1/run_weapon_scanout_oracle.py
```

The shared decoder now accepts a named original-video manifest and retains its
launch coverage. Repeatability checks reject missing image/VRAM pairs, and Lua
callback failures explicitly terminate this new capture instead of allowing
other callbacks to continue. Captures remain diagnostic temporary files, not
shipping playback data. The raw polygon-bitmap comparison still differs where
the native renderer intentionally separates HUD layers; the composed-frame
comparison has no such exclusions and remains exact. This certifies the bounded
laser scenario, not all weapons, elapsed production cadence or the campaign.

### Steering-route timing diagnosis — 2026-10-06

A fresh source-bound comparison reaches the independent Mesen capture through
Corneria scene 1500 without injecting game state. Scenes 1–191 pass the existing
full semantic/object comparator. Scene 192 still stops at the unverified
retail/native path-cursor correspondence; that guard has not been bypassed.
An explicitly globals-only diagnostic first differs in published motion timing
at scene 210, then in player position at scene 326 after the scene-325 timing
divergence. The native production timing still replays the neutral recording.
Both versions lose the player on this steering tape, at different times; its
old native boss-arrival assertion is not independent proof of the correct
outcome. Existing route, checkpoint and title failures remain unchanged.

The in-tree hardware runner also still disagrees with Mesen: a temporary
strategy-boundary diagnostic first reports eight motion refreshes at scene 24,
while independent neutral and route captures and the native table report seven.
The first 40 independent route snapshots match, including objects. This is a
runner timing defect, not grounds to change gameplay to eight refreshes. The
temporary diagnostic was removed without changing any established assertion.

Local evidence: `/tmp/sf1-corneria-source-strategy-published-oct06.log`,
`/tmp/sf1-corneria-route1500-mesen-oct06.log`,
`/tmp/sf1-corneria-route1500-native-oct06.log`, and the original capture under
`/tmp/nix-shell.HLK96i/starfox-mesen-profile.1g2f2gyl/Mesen2/LuaScriptData/mesen_corneria_timing_oracle/`.
These results diagnose timing; they do not certify the later route, shape
identity, pixels, audio, or source-derived production scheduling.
The independent comparison passes in debug and release, and the two fresh
captures are byte-identical. The banking branch separately passes 150 updates
of roll and all 32 background-offset columns. All 65 SF1 Python tests and the
architecture audit pass. The full release workspace retains 3,902 passing
tests, three failing tests and one ignored test, but only one executable
example now fails: `sf1_title_trace`. The other two failing targets remain
`semantic_trace` and `sf1_corneria_route`; no assertions in those gates changed.
The unchanged full Training replay also passes its 1,758 semantic/draw/audio
updates and 1,752 bitmap updates. As before, workspace results include the
preserved pre-existing dirty changes rather than certifying a clean revision.

## Refreshed workspace and live camera source audit (2026-10-06)

The full release workspace at `d0c9c9a`, including preserved pre-existing
working-tree changes, completes with 4,342 passing tests, three failing tests,
one ignored test and one failing executable example. No expected fingerprints
or assertions were changed. The failures remain:

- `semantic_trace`: the retained native fingerprint differs at tick 1,500;
  live front-end comparison first differs at sequence 126, with the original
  still in the intro and native already at the title.
- `sf1_corneria_route`: the controller tape loses the ship at level frame
  1,499. The tape was originally selected using a native search, so its asserted
  survival is not by itself independent evidence of original-game survival.
- `sf1_title_trace`: setup takes 127 native sampled ticks versus 128 original;
  its separate 96 source-bound semantic/object/draw updates still agree.

The unchanged Training check still passes 1,758 semantic/draw/audio updates
and 1,752 scene-region bitmap updates. The first-laser executable also passes
its existing source-bound and composed-video checks. These results are not
whole-game certification. Full baseline log:
`/tmp/starfox-workspace-status-engine-oct06.log`.

The subsequent camera audit found two actual shipping shortcuts: the space
strategy copied ship X/Y in exterior mode, and water used the disabled
half-width/fixed-height branch. Both now use typed source-level camera
anchors. The generic space anchor also now uses `Space_ViewCY`, not the live
`viewCY`. Planet-specific callers select their actual surface formula, and
underground reuses its independently checked fixed-center anchor.

`sf1_player_view_anchor` executes unmodified camera tails from the pinned
source-built ROM, after movement and before `viewmove_srou`. Across four
groups, 1,703,936 cases cover every coordinate word, every camera mode and
center/overflow boundaries, preserving camera Z, ship position and the live
center. This checks lateral anchoring, not movement, depth chase, hardware
timing or complete retail camera behavior. Seven native tests also check the
shipping strategy call order, disabled depth motion and engine publication.
The legacy C fixture is preserved byte-for-byte; its obsolete space-camera Y
expectation is corrected explicitly from its own recorded coordinates, then
retained through its non-rotating death sequence. No expected field comes from
the native execution under test.

All 1,314 strategy tests and the four new original-code groups pass in debug
and release, along with 73 SF1 Python tests, architecture, the unchanged release
Training check and all three app binaries in both profiles. Focused log:
`/tmp/sf1-player-view-anchor-final-validation-oct06.log`. The whole-workspace
baseline above predates this camera change; the three failing targets and the
larger source-derived timing replacement remain open.

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
