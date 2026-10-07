# SF1 and SF2 completion plan

Updated 2026-10-06. Status: implementation in progress; neither game is certified.

This is the execution plan for the user's request to finish both Rust ports.
It supersedes the sequencing restriction in `RETAIL_PARITY_PLAN.md` that deferred
all SF2 work until SF1 certification. It does not relax that document's accuracy
standard. Follow `NATIVE_PORT_ARCHITECTURE.md`. The current implementation stage
is source-first: read ASM/disassembled source and port its behavior, not recorded
gameplay. No new gameplay recordings are required to implement this stage.

## Completion contract

A subsystem is complete only after its reachable source behavior is mapped to
Rust, all branches and side effects are implemented, it is connected to the
shipping game, and its tests cover both normal and exceptional outcomes.
Record these as separate states: source inventoried, implemented, source reviewed,
unit verified, shipping integrated, integration verified, user playtested.
Never equate a generated graph, a green build, or a forced-progression soak with
a working game. Missing data and unsupported behavior must remain visible.

The final deliverable is two runnable Rust games with normal controller-driven
progression, not just tests or generated catalogs. No emulated game logic,
recording-driven gameplay, no-op implementations, assumed-success services,
native-generated expected fixtures, or unreviewed compatibility fallbacks may
be used to close a gap. Keep HD presentation separate from simulation state.

## Work queue and exit conditions

| Order | Work item | Required evidence before closing |
| --- | --- | --- |
| 1 | Establish the current baseline and executable gap inventory | Full workspace test results, release/app build, architecture audit, exact generated catalogs; every failure retained and assigned |
| 2 | SF1 source/dispatch and route closure | Every map/path/strategy entry resolves to its intended Rust behavior; audit old boss/proxy claims against current code; source tests for missing entries and incorrect callbacks |
| 3 | SF1 shared contracts and timing | Source-derived scheduling, arithmetic, RNG order, object lifetime, collision, child ownership, camera and control locks; remove neutral-recording timing from production |
| 4 | SF2 source-graph closure | Enumerate all discovered roots and their exact first unsupported operation, then indexed/dynamic installers and external phase writers; complete graphs with source-bound tests |
| 5 | SF2 production frame ownership | Connect native scheduler, authored paths, object creation/retirement, contact dispatch, motion, sounds and scene services to `Game::tick`; preserve callback/yield and random order |
| 6 | SF2 general campaign and live control | Replace prescribed encounter order, recorded spawn/retire/actions and neutral/live split with source state machines; arbitrary mission selection, all difficulties, pilots, Walker/flight and weapons |
| 7 | Whole-game progression and presentation | Controller-driven routes/campaigns, bosses, alternate outcomes, death/restart/continue, endings, save/load where supported, UI, assets and music/SFX; test short and long lifetimes and off-nominal input |
| 8 | Release handoff | Full debug/release regression and real-app smoke checks, usable launch commands for each game, no unexplained failures, accurate known-limitations report, intended commits pushed and remote SHA verified |

Orders 2–6 may interleave when one source contract unlocks another. Each
implementation batch must make progress on an actual blocker. Do not spend the
whole effort accumulating isolated leaf routines while leaving the production
frame owner disconnected. Integrate coherent source-complete slices; do not
switch a whole scene onto an incomplete backend or fabricate missing inputs.

## Starting evidence

- SF2 currently has 149 complete lowered actor roots and eight helpers, covering
  5,962 source commands and 5,903 typed statements. This includes child roots:
  it is not 149 out of the 106 independently discovered roots.
- The native authored path runtime is not yet the production `Game::tick` owner.
  `SF2_GAMEPLAY_COVERAGE.md` identifies recorded action schedules, fixed campaign
  progression, and a neutral/live control split that still need replacement.
- SF1 production `gameplay_timing.rs` still contains neutral-input timing arrays.
  `FINISHING_AUDIT.md` records unresolved verification and integration issues;
  those older results require a fresh baseline before use as current evidence.
- `UNPORTED_BOSSES_PLAN.md` is historical, not a trustworthy current task list.
  For example, current `level1_4.rs` already references the real bossH port.
- The checkout has substantial pre-existing SF1/app/render/audio changes. Preserve
  them. Review overlapping hunks before editing; stage only the current batch.

## Repeatable execution loop

1. Inspect current status, this plan, the specific source and the latest test
   outcome. Resume active verification instead of launching duplicate builds.
2. Select a bounded blocker and trace its caller, state owner, consumers and
   lifecycle. Document the intended source contract beside implementation/tests.
3. Implement real typed Rust behavior and production wiring when the slice's
   dependencies are closed. Test arithmetic boundaries, missing-input ordering,
   shared-state preservation, resource exhaustion and all authored branches.
4. Run focused checks, then the relevant full suite and release build. Do not
   bless a fixture merely because native output changed. Record failures even
   when unrelated; they remain release blockers.
5. Update the ledger with implemented versus integrated versus verified status.
   Commit intended paths, push, and verify the remote ref. Continue with the next
   blocker without asking the user to say continue.

## Verification commands

From the repository root:

```sh
nix develop --command bash -c 'cd rust && cargo test --workspace --no-fail-fast'
nix develop --command bash -c 'cd rust && cargo test -p sf2-game --no-default-features --release && cargo build -p sf-app'
python3 tools/check_native_architecture.py
python3 tools/sf2/generate_native_paths.py --check
python3 tools/sf2/disasm/extract_scene_loaders.py --check
python3 tools/sf2/audit_gameplay_static.py
python3 tools/sf2/path_backlog.py
python3 -m unittest discover -s tools/sf2 -p test_path_backlog.py
python3 -m unittest discover -s tools/sf2 -p test_generate_native_paths.py
python3 -m unittest discover -s tools/sf2/disasm -p 'test_*.py'
```

The historical retail-parity gate is a later independent validation tool, not
the source of gameplay implementation. Existing oracle-test failures must not
be hidden, but new recordings are not a prerequisite for the source-first work.
Whole-game accuracy cannot be inferred from either test family alone.

## Unattended operation and handoff

Use this same task for scheduled continuation while the user is away. Continue
implementation, not just status polling. Keep the user-owned assets and unrelated
changes intact. Stop for a real permission boundary or missing essential input;
otherwise work through another unblocked item. Notify only on a meaningful
milestone, a failure requiring intervention, completion, or required user action.
Disable the continuation schedule once the completion contract is satisfied.

The hourly same-task continuation `finish-sf1-and-sf2-rust-ports` was activated
on 2026-10-05. Local continuation requires the computer and desktop app to remain
running; scheduled work is not a guarantee of a completion date.

### Current execution checkpoint — 2026-10-06

- Connected the entire shared flight update (`$06:E258..E2ED`), including
  roll, ambient motion, throttle effects, surface response, speed, pose,
  translation, recoil, damage, corridor and occupancy response in source
  order. Surface effects supply the actual speed targets. Collision queries
  now publish whether a candidate passed the broad bounds, including rejected
  polygons/heights and earlier movement retries; that observation clears the
  subsequent grid tie bias exactly where the original does. No query is replayed
  and missing caller inputs are not replaced by neutral guesses. Native tests
  cover lazy failure order, fault latching and live cross-stage effects.
  Three unmodified-original groups pass 12,548 whole-routine calls, including
  8,192 independently retained visits, all 577 shape headers, nested effect
  creation, sound order and damage forecasts. Final-tree verification passes
  1,376 native tests and both architecture checks in debug/release, 1,384
  compatibility tests and 35 runtime checks, all 114 original player/movement
  groups in both profiles, 736 source tests, 238 extractor/lowerer tests,
  exact catalog/architecture checks and all three application builds.
  The enclosing input/mode strategy and production frame owner remain the
  next integration blockers.

- Ported the full live occupancy-grid response (`$07:E685..EA14`). It queries
  the actual reserved view and player, preserves ordered gates and contact
  transitions, publishes neighbor probes on the real shared actor, updates
  only changed cell-history axes, and shares heading/lean/return position
  with flight. The source's diagonal comparison is intentionally asymmetric:
  it compares absolute Z cell center with a retained caller bias, not the
  unused X-distance calculation. The bias and actual camera-task gate remain
  explicit caller inputs; they are never replaced by neutral defaults.
  Five original comparison groups pass 272,384 calls covering all mode/contact
  pairs, full-word position/yaw, every diagonal bias and 8,192 independent
  continuous visits. Both tie outcomes and repeated blocked/open visits are
  explicitly required by the fixtures.
  Four native groups cover lazy dependencies, state sharing, partial effects
  and scene fault latching. Final-tree verification passes 1,372 native tests
  and both architecture checks in debug/release, 1,380 compatibility tests
  and 35 runtime checks, all 111 original player/movement groups in both
  profiles, 733 source tests, 238 extractor/lowerer tests, exact catalog and
  architecture checks, and all three application builds.
  The next closure is the enclosing flight frame and its real caller outputs;
  this service is not a claim of production-frame or campaign completion.

- Ported the complete player corridor correction/admission and live region
  installer (`$07:E2F3..E5C7`, `$07:F893..F95E`). The source's wrapped signed
  octant tests, unequal boundary inclusivity, diagonal half rounding, level
  pitch/height order, proxy marker clearing and horizontal-only history writes
  are preserved. Heading uses the actual steering owner. Removed the separate
  displacement-suppression snapshot: authored action flags, corridor admission
  and path movement now share the same state. Five unmodified-original groups
  pass 331,776 calls, covering every heading, all word values, every action/mode
  pair, all heading/offset pairs and 4,096 independently retained two-service
  visits. The continuous fixture explicitly requires both repeated admissions
  and corrections. Four native groups cover lazy dependencies, real movement
  consumers, preserved prefixes and scene fault latching. The oracle now exposes
  returned predicate status, with an independent near/far carry self-test.
  Final-tree validation passes 1,368 native tests and both architecture checks
  in debug/release, 1,376 compatibility tests and 35 runtime checks, all 106
  original player/movement groups in both profiles, both return-harness tests,
  724 source tests, 238 extractor/lowerer checks, exact catalog/architecture
  checks, and all three application builds.
  Occupancy-grid response and the enclosing source player frame remain open;
  this does not establish shipping flight or campaign completion.

- Ported the post-flight recoil and complete surface-damage tail
  (`$06:E273..E2D0`, `$07:E18E..E2F1`) and connected contact-turn/impact
  publication to their actual pose, charge and translation owners. Removed
  the detached contact-turn, bank and charge-flag snapshots; the next pose
  update now consumes those effects. Recoil keeps its one-sample lookup
  advance, single multiplication, retained vertical integration and signed
  decay. Surface damage preserves the first probe's group, uses real
  previous-position/displacement forecasting, updates only specified flags,
  routes protection sounds/random draws in order, and retains signed shield
  clamping without spill-through and unsigned sound selection. Six original
  comparison groups pass 366,624 entry calls, including 4,096 continuous
  capture/pose/translation/recoil/damage visits and all 577 shape headers.
  Five native groups exercise real pose consumption, lazy rejection, missing
  dependencies and fault latching. Final-tree validation passes 1,364 native
  tests and both architecture checks in debug/release, 1,372 compatibility
  tests and 35 runtime checks, all 101 player/movement comparison groups in
  debug/release, 714 source checks, 238 extractor/lowerer checks, exact catalog
  and architecture checks, and all three application builds. The original player frame's map-boundary
  correction and enclosing strategy remain the next integration blockers.

- Ported complete flight translation (`$06:EE0A..F00F`), including protected
  flight's carry/clearance rules, the scripted-view gate, both real reserved
  actor vector publications, signed thrust, axis permissions, free-flight
  cleanup and configuration-nine collision response. Base velocity, summed
  displacement, retained sliding and shared support have distinct canonical
  owners; partial errors fault the scene. Five unmodified-original groups
  cover 335,872 calls, including every speed/thrust pair, all angle pairs,
  full-word clearance/sliding boundaries, all 577 shape headers and 8,192
  continuous visits with live geometry and mode changes. Four native groups
  cover lazy dependencies, prefix preservation and fault latching. The
  near-call oracle harness now honors its declared data bank at either
  accumulator width; a synthetic regression independently checks the harness.
  Validation passes 1,359 native tests and both architecture checks in
  debug/release, 1,367 compatibility unit tests and 35 runtime tests, all
  95 player/movement/harness groups in debug/release, 704 source checks,
  238 extractor/lowerer checks, exact catalog/architecture checks and all
  three application builds. Existing near-call consumers and cinematic/view
  regressions also pass. A reset-prefix fixture's newly compared support
  state was corrected to seed both sides equally before the final rerun.
  Shipping integration still requires the enclosing recoil, surface-damage,
  map-boundary and player-strategy sequence; these are the next blockers,
  not an invitation to substitute neutral state for missing caller outputs.

- Ported the complete shared collision-constrained movement service
  (`$0D:B282..B6B7`), using live surface candidates, authored polygons and
  their normals/footprints. It retains the asymmetric velocity dead zone,
  gravity-after-integration order, word-scaled response, previous-support
  motion, bounded penetration escape/retry/restoration, signed damping,
  carried displacement and one's-complement final length. The scene wrapper
  latches partial failures. All 577 shape headers, 273 collision polygons,
  full-word arithmetic and 16,384 continuous visits are covered by seven
  unmodified-original groups (382,352 calls in total). Explicit eligibility
  assertions prevent excluded fresh colliders from silently weakening these
  checks. The comparisons found and corrected the source's reversed low-word
  borrow in the existing yaw-probe rotation; 43,095 direct original rotations
  verify that correction. Four native groups cover carry rounding, lazy
  errors and fault latching. The caller's inherited tilt remains explicit;
  no normal-derived or neutral tilt is invented. This closes the collision
  dependency of configuration-nine flight, not shipping flight integration.
  Final-tree verification passes 1,355 native tests and both architecture
  checks in debug/release; compatibility mode passes 1,363 unit tests and
  35 runtime tests. All 89 original player/movement groups pass in both
  profiles, along with 698 source tests, 238 extractor/lowerer tests, exact
  catalog/architecture checks and all three application builds.

- Ported the complete flight speed/thrust service (`$06:F05D..F1FB`),
  including all seven pilot tables, boost-before-brake selection, forced
  speed gates, submode selection and inherited surface targets. Directional
  left/right input (not shoulder buttons) uses the original alternating-visit
  speed step. Base-speed easing wraps at byte width; thrust sign-extends both
  inputs before subtraction and updates only its byte, preserving adjacent
  charge flags. Five native groups and five original-comparison groups pass
  307,968 source visits, including every signed thrust pair, every input word,
  retained sequences and 768 continuous surface-to-speed handoffs. All 1,351
  native tests and both architecture checks pass in debug/release;
  oracle-enabled checks pass 1,359 unit tests, 35 runtime tests and both
  architecture checks. All 82 original player-service groups pass in both
  profiles, together with 691 source checks, 209 lowerer tests, exact catalog
  and architecture checks, and builds of all three application binaries.
  This is connected to the scene service owner, not yet shipping `Game::tick`.
  The next movement service includes a separate collision-constrained branch
  in configuration nine; its real geometry response must be implemented,
  not substituted with ordinary free-flight integration.

- Ported the complete two-wing surface response (`$07:DD6F..E0C8`) and
  both wing-effect installers (`$07:D403..D4D4`). It uses the live reserved
  probe actor, original byte-quantized roll/pitch/yaw and wrapping plane
  tests; preserves upper-contact precedence, selective carry suppression,
  byte-only fine-pitch/bank edits, existing recoil and exact sound routing;
  and installs real authored wing paths or the custom material particles.
  The response exposes the real event/height-byte handoff consumed by the
  following speed service. Five unmodified-original groups pass 17,679
  full-response cases, including retained multi-visit state and both fatal
  allocation boundaries. Five native groups cover lazy inputs, partial
  fault latching and all three effect lifetimes through the live scheduler.
  All 1,346 native tests and both architecture checks pass in debug/release;
  oracle-enabled checks pass 1,354 unit tests, 35 runtime tests and both
  architecture checks. All 77 original player-service groups pass in both
  profiles, together with 685 source checks, 209 lowerer tests, exact catalog
  and architecture checks, and builds of all three application binaries.
  The wing installer inherits its child number rather than setting one:
  this is an explicit optional caller input, diagnosed only if that branch
  reaches its read. Its enclosing producer remains open, as do the shared
  carry-mode producer, speed/motion/camera and shipping frame integration.

- Ported both surface-particle installers and complete custom strategies
  (`$07:C6E8..C867`) into the native actor scheduler. Fresh sibling allocation,
  self-referential transform parent, pause exemption, byte-quantized roll/yaw,
  sprite formatting, retained player-motion inheritance and the final parent
  size write match the source. Animation advances before integration; terminal
  frames request deferred removal without moving, and horizontal damping
  preserves the original separately rounded signed half/eighth terms.
  The player initializer now owns the combined flight-displacement record,
  distinct from actor velocity and the shared path-motion publication.
  Four unmodified-original groups pass: 18,944 installer parameter/rotation
  cases, 36 repeated/pressure/fatal-pool calls, every animation byte and every
  signed velocity word, and 512 independently retained installed lifetimes.
  Five native groups cover ordering, partial faults, duplicate siblings and
  both lifetimes through the actual paused scheduler and retirement service.
  All 1,341 native tests and both architecture checks pass in debug/release;
  oracle-enabled checks pass 1,349 unit tests, 35 runtime tests and both
  architecture checks. All 72 original player-service groups pass in both
  profiles, alongside 679 source checks, 209 lowerer tests, the exact catalog
  and architecture checks, and builds of all three application binaries. This closes
  the particle dependency of surface response, not its enclosing flight visit
  or shipping `Game::tick` integration.

- Ported the complete ambient player waveform update (`$06:F2F7..F365`)
  into canonical player-owned state. It preserves the early scripted-view
  skip, independently wrapping/range-checked phases, special-family bank
  hold, signed samples, and wrapping retained-word accumulation. Bank goes
  directly to the existing pose composer; the retained offset is not
  mislabeled as actor or camera height. The player initializer owns the
  new fields and the native scene service latches partial errors.
  Four original-code groups cover 145,408 cases, including all phase pairs,
  all retained words, all mode bytes, high view flags, continuous pauses,
  and continuous ambient-to-pose consumption without feeding native state
  back into the reference. The three new native groups and exact waveform/
  call-order source checks also pass. All 1,336 native tests and both
  architecture checks pass in debug/release; oracle-enabled checks pass
  1,344 unit tests, 35 runtime tests and both architecture checks. All 68
  original player-service groups pass in both profiles, alongside 674
  source checks, 209 lowerer tests, the exact catalog and architecture
  checks, and builds of all three application binaries.
  This removes one more dependency of the enclosing flight visit; surface
  response, speed, motion, camera and production frame integration remain open.

- Audited rapid-shot rejection through the complete original opcode and
  weapon dispatchers. The latter narrows the retained selection before
  entering the launcher. Therefore ordinary player shot-count rejection
  consumes a queued request only when its allocation is not page-aligned;
  fixed linked-view firing still consumes it, and no-weapon levels bypass
  narrowing. The typed capacity allocator now exposes only the needed
  alignment property, and the real player initializer publishes the queue
  policy. No source address or byte arena is retained. Original allocation
  and firing comparisons pass for 5,140 layout/level/linked-mode cases;
  another 1,024 cases cover path-dispatch rejection and diagnostics.
  In path fire, the rejected basic selectors retain a truncated caller
  selection and alternate retains an auxiliary selection: these are not
  native object references. Removed the fabricated reserved-actor fallback
  and report `UnsupportedWeaponRejection` before actor/cursor/publication
  edits. This is an explicit unsupported context, not emulation of corrupt
  source writes or a claim of complete path-rejection support. Authored
  reachability/dataflow for these malformed contexts remains open.
  All 1,333 native tests and both architecture checks pass in debug/release;
  oracle-enabled checks pass 1,341 unit tests, 35 runtime tests and both
  architecture checks. All 64 original player-service groups pass in both
  profiles, alongside 671 source checks, 209 lowerer tests, the exact catalog
  and architecture checks, and builds of all three application binaries.

- Corrected the shared strategy-object exhaustion contract across charged
  fire, consumables, shield feedback, ordinary/rapid weapons, reflection,
  path fire, and all three path-spawn forms. Errors stop at the original
  non-returning fatal allocation boundary while retaining earlier parameter,
  input, shield, action, incoming-shot-disable and random-state writes.
  Failed spawns preserve the previous last-spawn selection; no unrelated
  retained actor is borrowed and no later emitter counter/cursor advances.
  Existing-child and real admission gates still return normally. Map
  allocation remains separate. This supersedes the older graceful-drop,
  null-publication and full-pool fallback expectations below.
  Original execution also exposed a rapid-fire distinction: shot-count
  admission rejection retains the player's non-null auxiliary/view selection,
  so the flight wrapper consumes a queue entry even without a shot. Fatal
  exhaustion never returns, never consumes that entry, and can retain the
  linked view's temporary X/Z position. The native service now preserves
  both outcomes. New source tests exercise complete original installers,
  dispatchers, operand readers, pool code and sound/random routines; no
  allocator or error handler is patched. All 61 original player-service
  comparisons pass in debug/release, including nine new groups with 80,918
  exhaustion/admission fixtures. All 1,331 native tests and two architecture
  checks pass in both profiles; oracle-enabled verification passes 1,339
  unit tests, 35 runtime tests and two architecture checks. All 670 source
  checks, 209 lowerer tests, exact catalog/architecture checks and all three
  application builds pass.
  The later dispatcher audit above supersedes this batch's assumption that
  every player allocation produces a non-null rejection. Production
  flight/frame integration and the previously recorded whole-game blockers
  remain open.

- Ported the complete boost/brake selector, transitions and both cancellation
  entries, including the real numbered boost and paired brake effect
  installers. State is owned by the player allocation; processed input,
  activity, pilot, linked mode, action flags, sound routing and object pool
  remain canonical. Native scene tests run both authored effect families
  through the scheduler and verify cancellation, retirement and resource
  release. Original-code comparisons cover 262,144 transition cases,
  16,384 input/activity cases, all 256 pilot codes across linked modes and
  pool pressures, repeated installers and 2,048 continuous visits.
  The full strategy allocator revealed a previously missed source contract:
  an empty free list enters the non-returning `$00:8032` fatal display.
  Its apparent carry-clear return is unreachable. Throttle now reports and
  latches `ObjectPoolExhausted`, preserving partial child allocation and
  control writes; the oracle compares the real fatal-entry boundary without
  patching code. This discovery prompted the shared caller capacity-failure
  audit recorded above; earlier graceful-drop tests were not proof of this
  contract.
  This closes another dependency of the native flight visit, not its
  production frame integration; ambient motion, surface response, speed,
  movement and camera remain open. All 1,331 native tests and two architecture
  checks pass in debug/release; the oracle-enabled suite passes 1,339 unit
  tests and 35 runtime tests. All 52 original player-service comparisons pass
  in both profiles, alongside 666 source checks, 209 lowerer tests, exact
  catalog/architecture checks and builds of all three application binaries.

- Ported both SF2 pitch-input routines, the complete soft/hard terrain
  controllers, retained full-word input history and the map parameter-copy
  service. State lives on the existing player allocation; pose, fine pitch,
  contact protection, scene height and shared input remain canonical.
  Preserved held-up versus retained-down selection, active neutral fine
  pitch, wrapped height comparisons, signed cosine scaling, sequential
  upper/lower correction, odd-visit lean and the hard limit's zero-pitch
  branch. Scene wrappers select the primary configuration target and latch
  partial failures. Unchanged original code matches 65,536 configurations
  and history updates, 32,768 input/flag combinations, 262,144 protected
  height cases, 524,288 terrain cases, 512 missing-input prefixes and 1,536
  continuous history/pitch/terrain/pose visits. Each side retains its own
  fine orientation; native output is never fed back into the reference.
  Fixed the separate up+down-as-neutral bug in both shipping controllers;
  real game ticks cover both control styles, preserving existing single-key
  behavior and selecting the positive pitch branch for opposing input.
  These legacy controllers remain approximate. The full native flight visit
  still needs ambient motion, boost/brake effects, surface response, speed,
  movement and camera wiring; the map catalog also still needs to call the
  typed configuration service. All 1,323 native tests plus two architecture
  checks pass in debug/release, and the oracle-enabled suite passes 1,331
  unit tests and 35 runtime tests. All 48 original player-service comparisons
  pass in both profiles, with 660 source tests, 209 lowerer tests, exact
  catalog/architecture checks and builds of all three application binaries.

- Ported the complete SF2 horizontal controller (`$06:E4B1..E7C8`) and
  turning-lean producer (`$06:E8F5..E9E9`), using the existing canonical
  player allocation, pose, input, contact and shoulder owners. Includes
  pilot-specific response and yaw tables, left-first direction arbitration,
  asymmetric held-age updates, neutral lean retention, signed shoulder
  banking, locked-heading recovery and the original signed-sine lateral
  offset. The turn adjustment is explicitly a camera-bank target, verified
  through its view-roll consumer; it is not player speed. The two special
  branches that inherit a response target require an explicit caller input
  instead of assuming zero. Their upstream producer and the complete
  pitch/terrain/speed/motion visit remain open; the full controller is
  scene-hosted, not yet the production frame owner. Unchanged original code
  matches 196,608 word-width cases, 16,384 pilot/input/contact combinations,
  16,384 mode/edge combinations, 1,024 missing-dependency prefixes and 1,920
  continuous shoulder/steering/roll/pose visits. The scene wrapper latches
  partial failures against re-execution. Fixed both shipping flight
  controllers' separate left+right-as-neutral bug: a real `Game::tick`
  regression first reproduced the mismatch and now confirms left-priority
  pose, speed and movement in both controllers. This isolated fix does not
  certify the existing approximate flight controllers. All 1,315 native
  tests and both architecture integration checks pass in debug/release;
  the oracle-enabled suite passes 1,323 unit tests and 35 runtime tests.
  All 41 original player-service comparisons pass in both profiles, along
  with 653 source checks, 209 lowerer tests, catalog/architecture checks
  and builds of all three application binaries.

- Ported the full SF2 flight-pose composer (`$06:ECB0..EE09`) using the
  canonical fine-angle/bank allocation, shared steering publications,
  linked-mode/retained-pitch owner, roll impulse and contact gate. Includes
  wrapped quarter/eighth pitch response, separately rounded bank recovery,
  locked versus integrated yaw, all four additive roll terms, the distinct
  visible-roll decay branch and source-ordered partial failure. Original
  execution matches 131,072 fine-pitch/bank/impulse cases, 8,192 mode/gate
  combinations, 1,280 missing-input prefixes and 1,600 continuous roll/pose
  visits. The scene host latches failures instead of retrying an integrated
  yaw. The enclosing steering/terrain/speed/motion frame remains open, so
  this is not a replacement shipping player mode.
  The audit also exposed incorrect negative bank recovery in both shipping
  encounter controllers. Both now share the verified pose recovery helper;
  all signed byte values converge, and real recurring-attacker game ticks
  test negative values through zero. Updated their generators/templates and
  repaired Mirage Dragon's stale reference to the consolidated arctangent
  table; both generated modules reproduce exactly. All 1,307 native tests
  and both architecture integration checks pass in debug/release; the
  oracle-enabled suite passes 1,315 unit tests and 35 runtime tests. All 35
  original player-storage/input/pose comparisons pass in both profiles.
  All 646 source checks, 209 lowerer tests, generated catalog/architecture
  checks and all three application builds pass.

- Ported SF2 shoulder arbitration (`$06:9075..90D3`) and the double-tap
  roll controller (`$06:E7FF..E8F4`). Both-held input uses the last shoulder
  edge, with right winning simultaneous edges. Roll protection is published
  from the entering impulse: it starts one visit after the roll and remains
  active on the final decay visit. Tap history saturates, active impulses
  wrap, unrelated protection/action bits survive, and missing later state
  keeps the completed prefix. The scene host uses the existing protection
  owner through real projectile reflection, sound and random draws.
  Connected shoulder arbitration to the shipping Walker controller and
  retained it across form changes; this fixes the previous unconditional
  left priority. The original code matches 4,096 shoulder cases, 81,664
  roll cases and 2,560 continuous input/roll visits, plus write ordering.
  The source Walker consumer and shipping controller regression are also
  checked for ordinary turning. Roll pose composition and the complete
  native player-mode/frame owner remain unfinished; the full roll controller
  is not yet shipping gameplay. All 1,298 native tests and both architecture
  integration checks pass in debug/release; the oracle-enabled suite passes
  1,306 unit tests and 35 runtime tests. All 31 original player-storage/input/
  roll comparisons pass (the new Walker comparison was also rerun in debug).
  All 640 source checks, 209 lowerer tests, generated catalog/architecture
  checks and all three application builds pass.

- Added the shared player-entry reset for already-native SF2 scene services
  (`$06:958C..9691`): processed input, published shield, aiming pitch, surface
  and action gates, handoff flags, recovery/protection, horizontal reticle,
  camera tracking/focus, published motion and the environmental plane. The
  reset preserves player storage, handoff coordinates, the other reticle axis,
  homing target, view/options and pending audio. Missing retained owners keep
  completed writes and fault the scene against retry. Original execution
  matches every shield/capacity byte pair and 512 partial-write prefixes;
  write tracing confirms controller selection before clearing and the repeated
  horizontal-only reticle store. The outer initializer is still incomplete:
  movement-mode, script-selector, terrain-render and continuous-audio state,
  death registration and next-strategy ownership are not supplied by this
  deliberately bounded service.
  Also connected radio placement to the live reticle's vertical coordinate
  and boss-bar maximum instead of stale entry observations. Authored maximum
  changes are resampled within an immediate path visit. A continuous original
  reset/hidden-reticle/radio sequence covers 1,024 configurations, including
  contradictory initial observations. All 1,289 native tests and both
  architecture integration checks pass in debug/release; the oracle-enabled
  suite passes 1,297 unit tests and 35 runtime tests. All 26 player-storage,
  input, reset and reticle original-code comparisons pass in both profiles.
  All 634 SF2 source checks, the architecture guard and all three application
  builds pass. These are service-boundary checks, not shipping gameplay
  integration or whole-game certification.

- Replaced the shipping SF1 intro exit's approximate window-intensity fade
  with the source-verified display fade. ENDSEQ writes retained brightness 11
  after the accepting transfer; the following six transfers publish
  9/7/5/3/1/black and only then hand off to title. The existing visible
  brightness and real colour windows survive the seed, and the approximate
  fade cannot keep a second completion clock. Also restored the intro's
  low-byte frame gate, including its recurring input lock after byte wrap
  and its ordering before scripted exits. Unchanged original instructions
  verify 18,432 gate cases against the shipping shell, and an input-only
  full-machine run verifies the complete six-transfer fade and return.
  All 134 SF1 game unit tests plus 78 integration tests pass in debug/release;
  the original fade and caller checks pass in both profiles, as do all 68 SF1
  source checks, architecture and all three application builds. The title's
  96 semantic/object/draw updates still agree, but its distinct wall-clock
  setup assertion remains failing (127 native sampled ticks versus 128
  original). No compensating delay or changed expected fixture was added;
  front-end workload and controller-sampling timing remain unclosed.

- The preceding full debug workspace baseline completed: 3,954 harness tests
  pass, three fail, and the existing autonomous SF2 intro gate remains ignored.
  The title executable also fails its setup-duration assertion. The three
  harness failures remain the two semantic-trace checks and the controller-only
  Corneria route's lost player at level frame 1,499. Training passes all 1,758
  semantic/draw/audio updates and 1,752 bitmap updates; the weapon executable
  passes 338 strategy/controller updates, ten weapon-certified updates and
  26 strict video samples. This run began before the latest reticle and intro
  changes, so it is a baseline, not final-tree certification. The subsequent
  complete release workspace run, including the reticle and intro-exit changes,
  finished with 3,964 harness tests passing, the same three failing and the same
  one ignored. Its title executable retains the 127-versus-128 setup failure;
  training and the strict weapon/video checks pass with the counts above.
  These results cover the working tree, including preserved pre-existing edits,
  not just the committed changes. None of the failing gates was weakened.

- Closed the reticle mode preparer (`$07:B038..B0CD`) and full position
  producer (`$07:A418..A504`), composing the latter with target retention in
  one scene-host display slice. Preparation retains the visiting actor's
  marker state; display copies the primary player's current rotation and
  retained aim into the real aim proxy. Projection uses the fixed view's
  saved render origin and the previously prepared matrix, while separately
  publishing its full fine angles. It does not recompute that matrix from
  the newly published angles. Hidden reticles reset both axes to 100 without
  reading projection inputs. Mode preparation matches 32,768 original cases
  and 512 diagnostic prefixes. Unmodified host execution and both original
  graphics jobs match 75,265 projection cases; 3,072 continuous cases verify
  the composed scene-host projection/retention slice including its sound
  queue. Missing inputs preserve completed writes and latch against retry.
  Shared enable/inhibition and retained matrix/viewport remain explicit
  publications, not fabricated defaults. The preceding instrument services,
  marker drawing and source-owned shipping frame scheduling remain open.
  Validation passes all 1,280 native tests and 23 selected original-execution
  tests in debug/release, 1,288 compatibility-feature unit tests and 35 runtime
  tests, 209 lowerer tests, 629 static tests, catalog/architecture checks and
  all three app builds.

- Closed the numerical reticle-positioning dependency of target retention.
  Individual markers use the source's bounded-division projector at every
  depth, not the mesh's far-depth reciprocal table. The positioning tail
  (`$07:A471..A504`) preserves asymmetric vertical clamping, wrapped signed
  lower comparisons, unsigned upper comparisons and signed half-step easing.
  It updates the shared reticle in source axis order, with scene-host failure
  latching. Original execution verifies 86,016 projector cases, 262,144
  clamp/easing cases, 512 missing-axis prefixes and 3,072 uninterrupted
  positioning-to-retention cases. Five new native tests include the real
  projection/easing/retention chain. Validation passes 1,272 native tests in
  debug and release, all 19 selected original-execution tests in both profiles,
  1,280 compatibility-feature unit tests, 35 runtime tests, 209 lowerer tests,
  625 static tests, catalog and architecture checks, and all three app builds.
  These are complete numerical boundaries,
  not the still-unported outer display gates, aim-proxy/view preparation or
  a substitute for their source-owned scheduling.

- Implemented the source display service's target-lock retention boundary
  (`$07:A50A..A66B`) against the canonical primary-player selection and shared
  homing publication. Retained candidate, acquisition clock, grace period and
  marker style belong to the player's real storage and reset with it; forced
  ownership remains shared with candidate scanning. Cancellation preserves or
  clears the outgoing homing target according to the actual branch. Reticle
  axes remain independently absent until published, and partial errors latch
  in the scene host. Eleven new native tests cover lifecycle, skipped inputs,
  byte wrap, grace expiry, sound-ring overflow and the actual projectile-copy
  command. Unmodified source execution matches 150,528 complete cases, two
  missing-reticle prefixes and exact acquisition/sound/publication write order.
  All 1,267 native tests and all 15 player original-instruction tests pass in
  debug/release; compatibility tests, 209 lowerer tests, 622 source checks,
  generated-catalog freshness, architecture checks and all three app builds pass.
  The original caller is the display service, not actor strategy traversal;
  reticle positioning, the rest of that display service and its shipping frame
  scheduling are still unclosed. No gameplay cadence was invented to wire it.

- Closed the nine omitted scene-to-path bindings: caller reflection, retained
  friend health, targeting upgrades, environmental plane, linked-effect activity,
  attachment-owned protection, button layout, retained homing target and the
  shared countdown. All use live canonical records; unpublished inputs stay
  absent, and recycled actors/replaced player storage cannot inherit bindings.
  The adapter now explicitly constructs every `PathWorld` field, making future
  additions require a binding decision. Source review also corrected the full
  button-layout byte import, the contact gate's alias of the objective word's
  low byte, and protection's rotation-before-gates ordering. Independent
  original-instruction tests cover 24,576 complete protection cases and 256
  partial-prefix cases. Fourteen new native tests include complete authored
  countdown/protection paths, real reflection launches, same-invocation shared
  writes, lazy missing-input gates and failure latching. All 1,256 native tests
  pass in debug/release; catalog regeneration, 209 lowerer tests, 619 source
  checks and the architecture audit pass. Compatibility tests, all ten player
  original-instruction tests in debug/release and all three app builds also
  pass. This closes an adapter omission, not
  player-mode producers, the full player initializer or `Game::tick` integration.

- Completed the SF2 shared player-storage entry through its actual formatter,
  retaining separate boundaries for allocation and formatting. It publishes
  the retained display subject, selects manual shape frame zero, clears the
  path-hold latch and attack power, and applies the exact collision/shadow/
  draw flags without changing visibility, controller side or fixed cameras.
  The composed scene-host entry retains release-before-allocation and latches
  failure before formatting or retry. Two new native tests cover all flag
  combinations and missing actors; existing player-prefix and exhaustion tests
  now also cover the composed entry. Unmodified source matches all 256 flag
  bytes, including the full retained state write set, and eight complete
  allocation-to-formatter cases. All 1,242 native tests pass in debug/release,
  plus compatibility tests, eight player source-comparison tests in both
  profiles, 619 source tests, architecture checks and all three app builds.
  The enclosing shared reset, first player entry/modes and production frame
  owner remain open. The next integration audit found nine omitted `PathWorld`
  bindings, subsequently addressed in the checkpoint above.

- Implemented the complete processed-input service used by player movement
  modes. It selects the live actor's controller side, preserves sampled edges,
  applies flight-only vertical inversion and the full alternate button layout,
  publishes the pre-activity snapshot, then masks inactive controls and consumes
  queued script input. Scripted-view and protection gates clear processed input
  but retain both the queued input and earlier pre-mask publication. Player
  storage installs the actual zeroed injection words; the scene wrapper latches
  partial failures. Six new native tests include exhaustive words/modes/options,
  missing-input ordering, retained controller edges, and remapped charging through
  a real effect and projectile launch. Unmodified source matches all 327,680
  differential cases across controller words, both option maps, all movement modes
  and gate bytes. All 1,236 native tests pass in debug/release, together with
  compatibility checks, six original-instruction tests in both profiles,
  616 source checks, architecture checks and all three application builds.
  The surrounding player-mode call order and shipping frame owner remain open;
  the earlier player prefix intentionally does not resample or prepare controls.

- Connected scripted view transitions to the scene's canonical execution-mode
  word. Authored save/move/restore, fresh allocation defaults, player warning/
  action/fire gates and each actor's strategy admission now observe the same
  live bit, including changes made earlier in the current actor pass. Explicit
  entry observations remain available before the full word is supplied; no
  missing mode or allocation group is fabricated. Seven new tests exhaust all
  mode words and contradictory snapshots, exercise one complete authored view
  transition with allocations and hostile-shot death marking, test scheduler
  admission changes, and retain read/fault ordering for player services.
  All 1,230 native tests pass in debug/release, plus compatibility tests,
  four original player-storage/target tests in both profiles, 616 source checks,
  architecture checks and all three application builds. This closes shared
  scene-mode wiring, not the remaining player-entry/movement implementations
  or the production frame-owner migration. The processed-input producer is
  the next shared dependency for the enclosing player modes.

- SF2 player storage now owns the actual targeting record, and scene paths
  borrow that record from the primary player independently of path selection.
  The separate player target initializer preserves retained range, forced
  owner, angles and display fields while clearing only its documented fields.
  Fixed-view fine angles are read directly from the existing base-field
  aliases, shared with scripted camera motion and view save/restore; the old
  independent warning-heading publication is removed. Six integration/unit
  tests cover primary-player switching, live view changes, snap/chase/restore,
  missing-input resume and failure latching; storage release also invalidates
  targeting before actor reuse. Unmodified source verifies all 65,536 pairs
  of initial control/shared-mode bytes and 10,240 target-selection cases
  across both entry points, including fine-angle and coordinate boundaries.
  All 1,223 native tests pass in debug and release; compatibility tests, four
  original-instruction tests in both profiles, 616 disassembler checks,
  architecture checks and all three application builds pass. This closes a
  missing scene-path consumer and the target initializer dependency, not the
  enclosing player initializer, display-epoch target reset or mode dispatch.

- SF2 now has the real player auxiliary allocation/clear/publication prefix,
  connected to `SceneActors` and its existing player services. It replaces the
  actor's prior owned resource chain, charges the original 472-byte payload,
  initializes every currently ported per-player record, retains the copied
  shield/score and fine orientation, and publishes the selected player only
  after successful allocation. Player and linked-shot borrows validate both
  actor lifetime and allocation identity; releasing programs invalidates them
  before an object slot is recycled. Unmodified original instructions verify
  3,072 replacements across all pilot bytes and four allocation layouts, plus
  three exhausted/fragmented cases stopped at the allocator boundary. Native
  scene tests feed the initialized records into the existing ordered player
  prefix and verify failure latching, full replacement and stale-borrow
  rejection. All 1,217 native tests pass in debug and release; the compatibility
  suite, both source-comparison tests in debug and release, 616 disassembler
  tests, architecture checks and all three application builds pass.
  This closes the resource-backed record producer, not the enclosing player's
  initialization: view selection/formatting at `06:82B7`, shared reset, first
  entry and mode dispatch remain required before production `Game::tick`
  migration. The opening likewise retains its unresolved autonomous display
  scheduling gate; neither incomplete backend has been enabled in shipping.

- Explosion countdowns now store wrapped timer bytes, enter their first visit
  from initialization, and distinguish a kill signal from object removal.
  Factories schedule the original initializer and use the real player follower
  on pool exhaustion, preserving caller writes and random draws. Outward
  vectors use the shared source-exact 3D arithmetic and clear depth velocity.
  Generic explosions release attached fire before sprite allocation; ordinary
  removal retains source marker increments and its required final scrolling.
  The Nucleus boss now shares these services instead of maintaining divergent
  factory/countdown/large-explosion copies. Three original-instruction gates
  cover 8,032 timer, fallback and attachment cases. A fourth runs unmodified
  `TRANS.dostrats` against `Game::run_strategies` for 38,400 frames across four
  explosion entries, six capacity limits, two shape classes and five seeds,
  comparing tracked actor fields, active/free-list order and random state.
  This is an integrated effect-lifecycle check, not a whole-boss certification.
  The old boss1 trace used the bee shape and encoded a missing first visit and
  prematurely removed corpse: its pre-death regression remains, its obsolete
  death suffix is retained as historical data, and the original-code lifecycle
  gate plus a native corpse-lifetime test replace those invalid expectations.
  All 1,588 scoped native tests and 39 original-code tests pass in both debug
  and release, as do 15 source/audit checks and the architecture gate. All
  three app binaries build in both profiles. These are 3,254 Rust test passes
  across 502 test executables; no whole-game runtime claim follows from them.
  Whole-game SF1 timing and SF2 production scene ownership remain open.

- Boss-circle handoff now schedules the actual large particle emitter, retains
  insert-after-parent ordering, executes the circle initializer's first tick,
  and preserves the final scroll after removal. Outward-explosion constructors
  copy all four flag bytes and defer both child initializers to their own
  same-pass visits. Attached fire is released only after allocation attempts.
  Four original-instruction gates cover 5,644 entry cases and 65,792 subsequent
  particle visits, including signed timer wrap, exhausted pools, inherited
  flags, wrapped offsets and deferred callbacks. Two integration gates run
  the real scheduler, draw-list builder and particle pool: 255 particles are
  emitted once at birth and the emitter retires at its source lifetime.
  A reversible source-scoped correction updates the old boss1 circle records
  and forced slot-reuse permutation without changing other state. All 1,587
  scoped native tests and 35 original-code tests pass in debug/release, as do
  15 source/audit checks, architecture and all three app builds in both
  profiles. Separate boss-delay entry/kill semantics and failed outward-sprite
  allocation RNG remain open; the constructor tests do not certify those
  neighboring paths. Whole-game timing and SF2 frame ownership remain open.

- Removed the shared legacy collision-disable, hit-immunity and last-collision
  aliases and migrated their strategy producers and consumers to the source
  flag bytes. Collision-disabled meshes no longer alias particle emitters,
  hit-immune meshes no longer alias text, and tally text no longer sets the
  polygon-suppression and invisibility bits. Mixed flag writes are split by
  byte. Original instructions also exposed and verified corrections to the
  sea-monster's latch/toggle bits, its whole-byte animation test and wing
  initialization's preservation of inherited visibility. Two new original-code
  tests cover 1,024 inherited-state cases; the tally constructor is tested
  through the production draw-list text classification. Seven retained C
  traces received only explicitly scoped flag-layout edits to 2,659 records;
  reversible SHA-256 audits preserve every other byte and the genuine particle
  markers. All 1,585 scoped core/game/strategy/path tests and 31 focused
  original-code tests pass in debug/release; 13 source/audit checks and the
  architecture gate pass, and all three application binaries build in both
  profiles. Nine cartridge-side boss checks also pass, with the boss2 gate
  strengthened to compare exact flag/collision bytes from equal entry states.
  This closed the shared flag-alias integration blocker. The subsequently
  discovered delayed-circle placeholder and initializer fallthrough are now
  corrected and verified in the checkpoint above; the broader emitter
  callsite audit and production timing/frame-ownership work remain open.

- Implemented SF1's 300-slot, object-owned particle pool and connected it to
  completed gameplay/presentation scenes and both render modes. Allocation,
  seeded randomness, retained recycled fields, gravity, fading, clipping,
  transparent colors, owner retirement and painter ordering now follow the
  original routines. Scene initialization reseeds without clearing the pool;
  disabled scenes retain it and extra display frames cannot advance it.
  Original-code gates cover every generator/count combination, all random
  seeds and life/flag pairs, signed-boundary pixel output, a 96-scene sorted
  multi-owner lifecycle, boot reseeding and all seven emitter initializers
  across every inherited flag byte. Movement initialization enables particles
  and game-over initialization disables them. GPU checks cover previous-frame
  retention, transparent pixels, particle dispatch and near/far mesh occlusion.
  The fire initializer's source-verified collision-byte correction required
  changing exactly 100 fields in the retained boss2 trace; a reversible hash
  audit proves that every other fixture byte is unchanged. All 1,797 scoped
  native debug tests and 1,547 core/game/strategy release tests pass, along
  with the release GPU gate, 35 original-code tests in both profiles, nine
  source-contract audits, particle generation and architecture checks. Oracle
  examples and all three application binaries build in debug/release.
  This is not whole-game certification: other legacy collision-disable
  producers still alias the first-byte particle flag, and hit-immunity/last-
  collision aliases also need a shared flag-byte audit. Those are the next
  integration blocker before declaring the particle path production-complete;
  scene-work accounting does not yet replace the neutral-input timing arrays.

- Restored the shared SF1 movement initializer's missing gameplay resets:
  invulnerable player-body data and shadow/cull flags, accumulated rotation,
  slime, roll/shake state, hit/laser counts, special delay, movement speeds and
  background scroll policy. The source's byte-only death-yaw write preserves
  its inherited upper byte. Ship, inventory, shield-proxy HP, camera X/Y,
  firing state and control/roll timers are not indiscriminately reset. A new
  original-code gate compares 43 scalar fields plus object data and player/view
  targets across 1,280 inherited-state/view combinations. The checkpoint
  handoff test also exercises the immediate credits movement visit after reset.
  All 1,543 scoped native tests and 28 weapon/player original-code tests pass
  in debug/release; all three app binaries build in both profiles and the
  architecture check passes. This is integrated gameplay-state closure for
  that initializer, not closure of its still-missing particle activation,
  one-shot red-palette latch or separate source camera-object role.

- Connected SF1's source cockpit reticle to completed scene snapshots and both
  shipping render paths. The native lines preserve the original rotation's
  low-word subtraction/borrow behavior, ordered mirrored strokes, transparent
  palette zero and independent damaged-wing colors. Strategy initialization
  owns the enable byte, movement owns the roll byte, and ship selection publishes
  damage and the authored invisible cockpit ship with its real collision bounds.
  Original-code checks cover all 16,384 enabled geometry/color/damage cases,
  768 disabled cases, 1,024 complete indexed rasters, and 11,264 CPU-side
  enable/roll/ship-selection cases. Both render paths are GPU-tested, including
  empty object lists and previous-frame retention. All 1,791 scoped native
  debug tests and 1,542 core/game/strategy release tests pass, as does the release
  GPU check; all 29 scoped original-code tests pass in both profiles. The legacy
  C trace keeps its signed-roll column through a documented adapter, with an
  added disabled-HUD assertion; no expected fixtures were changed. Generation
  and architecture checks, oracle examples, and all three app binaries in
  debug/release pass. This closes the cockpit HUD slice, not the incomplete
  movement initializer, unrelated retained parity failures, or game certification.

- Ported the opening's inherited layer-policy producers and common load-table
  visibility reset. Cold boot starts with no visible layers; selecting the
  opening view changes the artwork plane, video initialization clears extra
  character/priority choices while retaining the foreground grid, and accepted
  load dispatch restores standard visibility. Queueing or rejecting a request
  does not change that policy. Original-code checks cover 2,048 inherited-policy
  combinations at all three producer boundaries, and a reset-to-loader check
  no longer borrows layer settings from the reference machine. Both 440-visit
  opening integrations now include layout and lighting alongside artwork,
  palettes and actors; only the documented actor-refresh partition remains
  reference-supplied in the native-frame-barrier variant. All 18 data tests,
  1,213 native tests and two architecture tests pass in debug/release; all 1,258
  compatibility-enabled tests pass in debug. Nine scoped original-code tests
  pass in both profiles, with the pre-existing autonomous-refresh gate still
  ignored. The source suite, generation/architecture checks and all three app
  builds pass. This closes the opening layer-policy prerequisite, not the other
  view-initialization effects, display timing or shipping `Game::tick` adoption.

- Added native scene-mode/layout ownership around the opening artwork loader.
  The four setup requests retain their distinct colour modes and map grids;
  large-character selection is captured at request acceptance while the display
  service reads the current artwork plane, visibility and extra layer policy.
  Every standard artwork service publishes forced blanking without advancing
  actors, palette effects or fades. The final main-loop handoff does not blank
  again. The joined-frame request carries this layout work, retains the real
  buffer barrier and rolls back with failed actor traversal. Missing inherited
  layer policy is an explicit error, not an assumed boot constant. Unmodified
  source checks cover all 262,144 setup flag combinations and 8,192 publication
  combinations; the reset-to-loader test also verifies each actual publication.
  Those isolated service checks start after the horizontal-blank wait and stop
  before audio/IRQ continuation, so they do not certify display timing. All
  18 data tests, 1,211 native game tests and two architecture tests pass in both
  profiles, along with 1,256 compatibility-enabled tests in debug. The eleven
  scoped original-code checks pass in both profiles; the previously ignored
  autonomous-refresh gate remains open. Source/extraction checks, exact
  generation, architecture audit and the three app builds pass. Parent policy
  production, native asset bindings, raster offsets, remaining reset/postload
  effects and shipping frame-owner adoption still require implementation.

- Connected opening lighting selections to the native scene loader. Setup
  publishes the normal depth thresholds while retaining the inherited colour
  family; the opening thresholds and standard colour family are installed by
  a separate main-loop handoff after the sprite palette publication. That
  pending handoff prevents replacement and does not run actors; repeating a
  completed resume does not reinstall state. The joined-frame load path owns
  this continuation too. Native flat shading now accepts the selected family
  and one-based object threshold overrides, with explicit diagnostics for
  missing or unreviewed inputs. All five colour families and fourteen threshold
  records are extracted and checked exactly. Unmodified original code verifies
  917,504 depth selections and 134,400 flat-material cases, plus reset-to-loader
  publication boundaries and all 440 opening visits. The latter still supplies
  original actor-refresh partitions; native display timing is not certified.
  All 18 data tests, 1,206 native game tests and two architecture tests pass in
  debug/release; all 1,251 compatibility-enabled tests pass in debug. The nine
  scoped original-code tests pass in both profiles with the pre-existing
  autonomous-timing gate still ignored. The 619 source/extraction tests, exact
  generation, architecture audit and all three app builds pass. Layer setup,
  the remaining scene-reset/postload effects, non-lighting handoff work and
  `Game::tick` adoption remain open; this is not rendered-frame certification.

- Completed the SF1 wire-shield pickup/drop and selected-ship lifecycle.
  Collection now preserves exposed-player selection, wrapped ranges, flag-byte
  ownership and repeated removal ordering; enemy drops have the real pickup
  mesh and defer their first visit. The selected ship table row now has its
  own typed state, distinct from loading temporary shape rows or retaining a
  shield flag. Expiry preserves the source's signed hit-count gate and rearms
  its countdown after switching back to the normal ship. The ordinary strategy
  pass publishes cockpit HUD colour from the exposed ship's previous colour
  frame, then advances or parks that ship's animation under the original gates.
  This is verified game-state publication; the renderer still lacks its
  cockpit-line consumer. Four original-code tests cover wrapped pickup/death
  cases, failed drops, every expiry countdown at signed hit-count boundaries,
  and all 256 colour bytes across selection/shield/hit-flash branches. All
  1,544 game/path/strategy tests and 24 weapon/pickup original-code tests pass
  in debug/release; all 49 SF1 source tests, the architecture audit and all
  three app builds pass. Existing timing/route and SF2 frame/scene gates remain
  open; the one-up carrier family is not included in these pickup batches.

- Completed SF1's body-health pickup and the two laser-drop callers. Health
  now targets the collision system's live body, enters movement/collection on
  its initializer visit, preserves signed wrapping range and health arithmetic,
  and continues after releasing linked fire on player death. Enemy laser drops
  inherit position before one deferred child visit, retain their real shape,
  and honor the configured no-drop branch for intact, beam-upgraded ships;
  the source disables the previously invented helper-ball alternative.
  Original-code tests cover every health byte, death/cockpit order, wrapped
  ranges, all ship-flag bytes, upgrade branches and allocation failure. Native
  production tests verify one health/laser award and subsequent flash visits.
  All 1,541 game/path/strategy tests and 20 weapon/pickup original-code tests
  pass in debug/release; the source contracts, architecture audit and all three
  app builds pass. No gameplay fixtures changed. Wire-shield selection and its
  HUD consumer remain under review; existing whole-game release gates remain
  open.

- Completed the adjacent SF1 special-weapon pickup and bomber-drop caller.
  Collection now selects the exposed player, preserves the source's wrapped
  range checks and signed word inventory cap, continues after a death marker,
  writes collision-disable in its proper flag byte, and leaves score unchanged.
  Full inventory still consumes the pickup through the flash initializer,
  without restarting the HUD flash or playing the award sound. Enemy drops
  preserve their inherited position offset and defer initialization to one
  same-pass visit after the dying parent. Inventory access now uses its typed
  owner directly. Original-code tests verify all 65,536 inventory words,
  wrapped coordinates, drifting/stationary pickups, cockpit/death ordering,
  failed allocation and the drop's first visit. Native production-pass tests
  verify one award and subsequent flash progression. All 1,538 game/path/
  strategy tests and 17 weapon/pickup original-code tests pass in debug/release;
  all 45 SF1 source tests, the architecture audit and all three app builds pass.
  The exhaustive test resets case inputs while retaining the ROM allocation;
  it does not substitute expected state or skip original execution. Existing
  whole-game timing, route and SF2 integration gates remain open.

- Completed the source-level SF1 repair pickup/ship chain in the shipping
  strategy dispatcher. A failed repair-ship allocation leaves its pickup
  available; all three callers install the child after themselves and defer
  initialization to its own visit. Pickup death markers release linked fire
  and continue the visit, collision-disable uses the correct flag byte, and
  collision/catch tests preserve signed wrapping arithmetic and exposed-player
  selection. The approach still uses separately published player coordinates.
  Catching restores both wing actors' health, attack and handlers before
  clearing only the four wing flags, then defers the flash initializer.
  The laser and helper pickups restore wing actors without clearing those
  flags. The repair-carrying enemy now performs its first falling step on entry.
  Original execution also exposed the laser pickup's invented 100-point award:
  the source `s_score` macro is empty, so that award was removed. Five new
  original-code differential tests cover entries, exhaustion, successful
  allocation, wrapped distance/approach boundaries, countdowns, sounds, wing
  state, upgrade flags and unchanged score. Production-pass tests verify the
  repair child's one deferred birth visit and subsequent countdown. All 1,535
  game/path/strategy tests and all 14 weapon/pickup original-code tests pass
  in debug/release; all 44 SF1 source tests, the architecture audit and all
  three app-binary builds pass. No trace fixtures were regenerated. This does
  not close the retained SF1 semantic/timing or route gates, nor SF2's pending
  native frame/scene integration. The adjacent special-weapon pickup's
  score/flag/death-order discrepancies were queued at this checkpoint and
  are addressed in the subsequent batch above.

- Connected native opening-controller requests and button skips to a typed
  cinematic exit owner. Input holds, the delayed button-request transition,
  all three skip policies, one-shot audio requests, optional destination
  selection and fade-completion handoff now follow the original outer loop.
  The shared fade preserves its signed, wrapping word countdown and waits for
  independently serviced display intensity; it never advances the display
  itself. Exit visits do not advance actors, RNG, artwork or frame buffers.
  Original-code differential tests cover all eleven authored policy settings,
  every word delay, active/audio-latched combinations, input and signal branches,
  cue-ring wraparound, untouched state and complete signal clearing. A reset-to-
  exit check verifies the ten-visit input hold, the controller request at entry
  age 441 (its 442nd visit), and completion after four observed display fades.
  **Display visit timing is still supplied by the original in that check;
  actor-state parity is not asserted there.** The audio transition is an
  explicit request, not implemented PCM playback; alternate destination routing,
  full scene setup/postload, autonomous refresh timing and `Game::tick` adoption
  remain open. All 1,203 native unit tests and two architecture tests pass in
  debug/release; all 1,248 compatibility-enabled tests pass in debug. The six
  scoped exit/display original-code tests pass in both profiles. The four
  enabled opening actor/palette integration tests also pass in release; the
  known autonomous refresh-timing gate remains ignored. The architecture audit
  and all three app-binary builds pass.

- Connected deferred artwork requests to the native opening's ordinary-frame
  barrier. Draw and upload buffers have separate typed ownership, accept
  either completion order, reject duplicate completions, and retain their
  roles across requests. The load gate tests the next upload source after
  both jobs join; it does not test controller age or a prescribed update.
  The latest unstarted artwork request replaces its predecessor without
  changing published assets. Active service sequences cannot be replaced;
  allocation failure rolls back pending artwork and frame ownership too.
  The original boot's full 440-update opening verifies both owners against
  unchanged source execution. It also demonstrates zero, one and two upload
  completions between controller visits, so controller parity is explicitly
  not treated as instantaneous upload state. Native artwork scheduling now
  passes the previously ignored 440-update live/saved-palette check, with
  source service events used only for assertions, not scheduling input.
  **This is joined-frame artwork integration, not native display timing or
  complete scene loading.** Actor-refresh boundaries are still supplied by
  the original in that check; the independent autonomous timing gate remains
  ignored. Rendering, the other scene-setup/reset/postload effects and
  `Game::tick` adoption remain open. All 1,198 native unit tests and two
  architecture integration tests pass in debug/release; all 1,243
  compatibility-enabled tests pass in debug. The seven scoped original-code
  tests pass in debug/release with that one known timing gate still ignored.
  The architecture audit and all three app builds pass.

- Connected the standard loader's native artwork publications to `OpeningScene`:
  typed background tiles/map cells, separate live/sprite palette ownership, and
  an ordered publication queue which distinguishes main-loop requests from
  display services. Character publication queues the map directly; foreground
  selection and the later sprite request require main-loop resumptions.
  Foreground policy is sampled at its source boundary, the inherited
  background-palette skip is consumed once, overlapping requests fail without
  mutation, and neither publication nor request advances actors or clocks.
  The additional decoded character tile is correctly excluded from upload.
  Original reset-to-loader execution now verifies every publication boundary,
  the actual emulator VRAM character/map transfers, live/saved palettes and
  sprite colors. Another 1,024 original selector executions verify both policy
  states, all selector bytes and unrelated flag bits. A combined 440-update
  opening check passes for palettes, actors, RNG and camera, using observed
  source pass partitions **and artwork service boundaries**, not injected
  colors or a hardcoded update-two installer. It passes in debug/release.
  All 18 native data tests, 1,194 native game unit tests and two architecture
  integration tests pass in debug/release; all 1,239 compatibility-enabled
  game tests pass in debug, 11 scoped oracle tests pass in release, and all
  three app binaries build. The architecture audit also passes.
  At this checkpoint, autonomous artwork scheduling, other scene-setup side
  effects, display timing, render-state handoff and `Game::tick` adoption were
  still open, and both existing ignored autonomous gates were retained.
  This batch did not change
  the shipping intro's recorded presentation controller or certify SF2.

- Separated historical SF1 checkpoint hashing from the expanded live trace
  schema. The nine fields added after those checkpoints remain in full live
  retail/native comparisons; only the old fingerprint uses its original field
  set. No expected hashes or timing comparisons changed. A regression test
  verifies that projection does not mutate live evidence, hide an old-field
  change or silently drop future fields. It passes in debug/release. The
  native replay now reproduces its first three historical hashes and exposes
  a later mismatch at scenario tick 1500, instead of failing on schema drift
  at tick 892. The integrated oracle still disagrees on timing at tick 892
  (four versus three refreshes). **Both SF1 semantic tests remain failing**;
  this diagnostic correction does not certify the route or change gameplay.

- Inventoried the scene-loading dependency of the native map/frame owner:
  all 31 authored map requests select 19 table entries and 17 distinct loader
  routines. A fail-closed control-flow extractor retains 840 instruction nodes,
  both sides of conditional loads, polling loops, shared tails and 42 embedded
  artwork packets. Packet payloads are not instructions; helper return widths
  and seven-byte payload consumption are gated against the reviewed source.
  All 32 distinct referenced artwork streams now match the original Super FX
  decompressor byte-for-byte in their actual character/tile-map destinations,
  with neighboring canaries preserved. The generated oracle fixture stores
  original operands/code hashes, not native-produced expected pixels.
  Decoded length and transfer length are distinct: the two-layer loader copies
  only half of one decoded tile-map stream. All 616 SF2 disassembly/source tests,
  exact fixture regeneration, the architecture audit, 14 native data tests and
  nine artwork/display oracle tests pass; the nine oracle tests also pass in
  release. This closes source inventory and decoder coverage for these loads,
  **not native loader execution, transfer scheduling or shipping integration**.
  In particular, the boot opening selects load-table entry 141 / loader C80B;
  the separate two-layer CB1C loader is not the opening's artwork installer.

- Implemented native scene-display ownership for the two distinct source fade
  services, the separate frame-owned blank hold, entry fade initialization,
  all-band publication and the map's exact readiness predicate. Preserved the
  third band during blank holds, one-visit fade-in completion delay, wrapping
  paced interval, strategy-clock parity, and full-screen requests which remain
  active at either endpoint. Completing a banded fade-out publishes a different
  blank intensity from the map's ready state; no fabricated completion signal
  was added. Seven unit tests and three original-code tests cover the service
  contracts, including 211,712 source calls over all valid intensities, request
  kinds, counter/reload byte pairs and all blank-hold bytes. All 1,191 native
  unit tests and two architecture integration tests pass in debug/release;
  all 1,236 compatibility-enabled tests and ten map/display oracle tests pass
  in debug. The three display oracle tests also pass in release, as do the
  architecture audit and all app builds. These are native services for the
  forthcoming scene host, not yet the shipping frame owner.

- Added the typed native scene-map owner: validated dense-index graphs,
  source-selected branches, retrying display/load waits, retained yield markers,
  current-actor effects, explicit external phase releases and latched service
  faults. No source addresses or machine-state host are used by this owner.
  Ordinary map allocation now has its own no-pressure-sweep pool operation:
  insert after the active head, initialize fresh defaults, preserve zero speed,
  and clear the map's selected actor when allocation fails. The source proof
  caught an incorrect speed initialization before this code was integrated.
  Thirteen native tests cover these contracts and malformed graph/service cases.
  The five dispatch tests now compare the native owner too; together with two
  new allocation tests they perform 238,340 original-code calls, including every
  display-progress/publication byte pair, 256 load-table byte offsets, all 232
  authored ordinary spawns under varied defaults/markers, and all 60 allocations
  followed by exhaustion. Allocation tests execute the unchanged formatter and
  handler; they do not claim to execute or validate the installed strategies.
  All 1,184 native unit tests and two architecture integration tests pass in
  debug/release, all 1,229 compatibility-enabled tests pass in debug, the seven
  original-code tests pass in debug/release, and the app builds. This owner is
  still awaiting the decoded scene catalog, real scene
  services and production frame wiring; it is not full-game integration.

- Recovered SF2's frame-to-map dispatch contract before replacing its staging
  controller: the nonzero delay operand is a retained yield marker, not a
  countdown. The original main loop dispatches again on the next visit; its
  alternate-view branch suppresses map dispatch only when both flags 01 and
  20 are set. Corrected the oracle-only compatibility driver and zero-delay
  marker preservation; no shipping scene has been switched to that driver.
  Four original-code tests perform 170,192 completed calls: all 237 authored
  hold loops, all 134 mode/external-flag branches, every 16-bit delay operand,
  and all 256 frame-flag combinations with four preceding marker values.
  For the frame-owner test only unrelated scene/render/audio services are
  bounded at their call entries; frame and map code remain unchanged. Synthetic
  data operands and STOP successors test boundaries without replacing handlers.
  Native map unit coverage also exhausts all delay operands, and two host
  regressions cover repeated dispatch and the combined suppression gate.
  The full compatibility-enabled game suite (1,216 tests) passes in debug;
  all 11 map tests and four new oracle tests pass in debug/release, and the
  architecture audit and all three app-binary builds pass. The necessary next
  slice is a typed native map owner with
  source-owned scene installers, not generic byte-addressed host reuse.

- Corrected SF1's live body/wing collision contracts: broken-wing wall damage
  uses the body's cooldown and an explicit four-point power; wall entries skip
  object recoil, wire wings skip impact cues, and both intact/broken wings share
  the effect-copy/spark tail with its missing-effect gate. Damage scaling is an
  arithmetic byte shift, and the final clamp tests the wrapped subtraction's
  sign. Adopted the pre-existing quantized eleven-degree shake correction after
  verifying both entry points against the original code; unrelated player and
  collision edits remain unstaged. Replaced the damage test's two handwritten
  mock implementations with calls to the actual shipping function: 132,608
  original executions cover every health/power pair and cooldown byte, and
  2,048 full body/wing source calls cover scaled powers, entry recoil, broken
  wings, walls, effects and pool exhaustion. Four native tests and three source
  assertions cover the same contracts without replay-derived expectations.
  All 1,530 game/path/strategy tests pass in debug/release; the three scoped
  oracle targets (13 tests), all 40 SF1 Python checks, architecture audit and
  app build pass. An isolated staged-only checkout also passes the four native
  collision tests and both new original-code targets, independently of the
  unrelated working-tree edits. The Corneria route still reaches the carrier with two body
  points against the old assertion of one. That assertion originates in the
  native-search commit `1c624fc`, not an independent original-game checkpoint;
  it remains unchanged pending route diagnosis. Neither it nor the separate
  semantic-trace failures is certified by this collision slice.
- Completed two additional SF2 source graphs: target-gated pulse attacker and
  periodic pulse-pair emitter. All 1,045 native tests and two integration tests
  pass in debug/release; app build succeeds.
- Added `tools/sf2/path_backlog.py` with exact blockers and source hashes. It
  identifies 64 registered discovered roots and 42 blocked roots. Three tests
  protect the inventory's distinction between discovered and child roots.
- All 208 lowerer tests pass. The broader 537-test source suite initially caught
  an outdated semantic count: carry-off was added with its source proof but not
  listed among reviewed, currently unused handlers. Corrected that exact set;
  all 537 tests now pass. No reachable-handler check was weakened.
- The fresh full-workspace baseline completed with six failing targets:
  `sf-oracle/semantic_trace`, `sf-oracle/sf1_corneria_route`,
  `sf-path/interp_trace`, and `sf-strat/{bo_parity,ea_parity,eb_parity}`.
  This baseline compiled the `18484df` era code plus the pre-existing dirty
  checkout; subsequent SF2 changes have separate verification below. Preserve
  every failure as a release blocker, not an invitation to bless current output.
- Completed the objective-gated pulse patrol and its wingmate announcement
  helper, including the shared scene-event word's source producers/consumers.
  The catalog now has 150 roots, nine helpers, 6,099 source commands and 6,040
  typed statements. The discovered-root backlog is 65 registered/41 blocked.
  All 1,049 native unit tests and two integration tests pass in debug/release;
  209 lowerer tests, 540 source tests, three backlog tests and the app build pass.
- The Corneria route assertion observes player-body durability of two at Attack
  Carrier arrival, while the test expects one. Diagnose the original
  damage/initialization contract before changing code or expectation.
  The semantic failures involve motion-refresh timing and require independent
  source diagnosis; the in-tree oracle is not automatically authoritative.
- The `ponpon` path trace differs in projectile collision class (80 versus 64).
  Existing dirty path-fire code adds the ordinary-fire enemy class; source
  `PATHS.ASM` independently distinguishes ordinary fire from `CANHIT`. Review
  that entire contract and fixture provenance rather than reverting the bit
  solely to pass an old C-derived trace. The boss2 trace also differs in flag
  placement; assign all three strategy-trace targets to the SF1 source audit.
- The existing full Training-cycle comparison passed 1,758 semantic, draw and
  audio updates, and 1,752 bitmap updates, including course restart. This is
  bounded scenario evidence, not whole-game completion.
- Implemented the complete SF2 path-invocation coordinator: per-statement fresh
  world borrows, ordinary/death-tail movement, mutable callbacks and final
  attachment/carry work. Corrected strategy traversal/immediate retirement to
  use the actual returned actor, with source-byte proofs for both passes.
  Nine driver tests and the returned-actor scheduler test exercise this layer;
  all 1,059 native tests and two integration tests pass in debug/release, the
  app builds, and all 543 source/disassembly tests pass. Catalog totals remain
  unchanged. **This coordinator is not yet connected to `Game::tick`.**
- Next integration work: implement the real scene-owned `InvocationWorld` and
  `StrategyHost` services, including hit/death routing, positional sound, full
  retirement and live contact ownership. Then migrate a source-complete live
  encounter and its spawns. Do not fill absent world inputs with guessed values
  or pretend synchronous rendering implements the original split-frame owner.
- Implemented the shared death service and pooled explosion strategies from
  source: override-before-default dispatch, map-count/continuation ordering,
  distance sounds, global-head allocations, direct-child death marking, scene
  proxy release, animation and trailing delay. Twelve tests compose these with
  the live strategy scheduler and full contact/program retirement, including
  zero/one-slot failures and the huge explosion's zero-health companion.
  All 1,071 native unit tests and two integration tests pass in debug/release;
  549 source tests, exact regeneration, architecture checks and app build pass.
  Production scene adapters, native override/map registrations and the overall
  `Game::tick` connection remain open; this is not a migrated encounter yet.
- Closed two additional actor-identity handoff gaps before scene wiring:
  hit response now returns the assigned strategy's actual actor, and every
  path-world borrow receives the current program actor as well as its selected
  player. Borrowed actors resolve their own attachment/weapon records, including
  on missing-input resume. All 1,073 native tests and two integration tests pass
  in debug/release; all 550 source tests, architecture, exact regeneration and
  the app build pass. Production integration status is unchanged.
- Reviewed the existing SF1 path-fire correction against all six source
  handlers: ordinary fire ORs the ENEMY1 class after construction; CANHIT
  neither adds nor clears that class and leaves constructor immunity intact.
  An opcode-level test covers all six forms, existing class bits, inactive
  targets, slot-zero targets and allocation failure. The `ponpon` trace's 225
  post-construction projectile records now add that one bit; a source test
  reverses only that field and verifies the original complete-file hash.
  This closes the `sf-path/interp_trace` baseline failure without generating
  expected data from native output. All 38 path tests pass in debug/release;
  the three source/fixture-audit tests, architecture check and app build pass.
  The other five failing targets remain open.
- Reviewed and completed the SF1 relative-explosion/polygon-suppression flag
  migration, source sprite flags, homing-shot saved speed, radar initialization
  fallthrough and terminal lifetime stores. Fixed an additional control-flow
  error: `s_kill_obj` marks death but does not return, so debris still moves and
  the bomb still applies falling acceleration on its death visit. Boundary
  tests cover all 256 lifetime values, manual detonation, coordinate wrapping
  and independence of the macro-facing and relative-explosion flags.
  All 14 retained enemy/boss regression traces pass. Seven complete-file
  inverse-hash audits preserve every field outside these reviewed corrections;
  native-output blessing was removed from the four reviewed trace harnesses.
  Full `sf-game`, `sf-path` and `sf-strat` suites pass in debug/release, all 30
  SF1 Python checks pass, and the architecture and app checks pass. An isolated
  staged-only tree also passes all 1,510 tests from those three packages when
  supplied the same user-owned reference/catalog assets (the initial export
  correctly exposed their absence), and its app build succeeds independently
  of the working tree. Unrelated local changes remain unstaged.
  The original `bo_parity`, `ea_parity` and `eb_parity` failures are closed;
  `semantic_trace` and `sf1_corneria_route` remain release blockers.
- Next SF1 source gap found during this review: `relflatmiss_Istrat`,
  `flatmiss_Istrat` and `helpballhome_Istrat` fall through to their movement
  strategies in ASM, but their Rust initializers currently stop after setup.
  This must be corrected and verified from source, including birth-frame
  movement/lifetime, before treating the weapon family as complete. Retained
  legacy traces may encode that old delay and are not authority for preserving it.
- Closed the two flat-missile initializer gaps, including camera-facing angles
  for the non-relative variant. A new source-built ROM differential test covers
  30 combinations of initial count, signed speed, aim and wrapping coordinates;
  both entry points now agree in debug and release. Seven firing constructors
  defer initialization until the object's own same-pass strategy visit, and
  list insertion is after the firer. This also makes post-construction aim and
  randomized speed effective before vector generation. A production scheduler
  test verifies exactly one birth visit and one subsequent visit for all seven.
  The Houdai fixture was advanced by the source's constant movement/lifetime
  rules, not regenerated from native output. Its inverse restores all original
  records (including four terminal records) and the prior complete-file hash.
  All 1,272 strategy tests pass in debug/release; all 243 game/path tests,
  33 SF1 Python tests, the architecture check and app build pass in the current
  checkout. These are subsystem results, not a new whole-workspace certificate.
  `helpballhome_Istrat` and source missile-boundary comparisons remain next;
  the two earlier oracle release blockers and SF2 production wiring remain open.
- Closed the orbiting and homing helpball entry/retirement slice. Both
  initializers enter their strategies immediately; pickup and child constructors
  install those initializers for the objects' own same-pass visits. Corrected
  sprite/target flag domains, preserved unrelated flags and the sprite colour
  high byte, separated homing aim from visible camera-facing angles, retained
  counters on initialization, restored wrapping comparisons/decrements, and
  retained source target locks when allocation fails. A terminal orbit visit
  continues through movement/search after marking removal, as in the source.
  The five weapon-entry differential tests now cover 95 source executions,
  including six complete CPU-to-Super-FX homing calls and 48 orbit boundary
  cases. They install the original boot-copied math trampoline and require a
  verified subroutine return; new oracle result metadata detects partial calls
  and unexpected stops. A production-loop test verifies three helper children
  each initialize exactly once after their parent. All 1,516 game/path/strategy
  tests pass in debug, all 1,273 strategy tests pass in release, and 29 scoped
  oracle tests pass in both profiles. All 33 SF1 Python tests, the architecture
  audit and app build pass. The remaining pickup behavior beyond constructor
  ordering, missile boundaries, earlier oracle blockers and SF2 wiring are not
  certified by this slice.
- Closed the shared missile-boundary branch gap. Restored wrapping word
  comparisons, inclusive top/bottom edges, the strict left player gates, both
  top gates when combined, and the actual exposed-player relationship instead
  of slot zero. A source-built ROM differential exhausts all 256 flag values
  across 228,096 coordinate/player combinations, including signed extremes;
  three native tests keep representative edge contracts available without ROM
  assets. All 1,519 game/path/strategy tests pass in debug, all 1,276 strategy
  tests pass in release, both weapon oracle targets pass in debug/release,
  and the 33 SF1 Python tests, architecture audit and app build pass. The source
  boundary proof does not certify absent-player scene states or unrelated bomb
  logic. Continue with the two remaining SF1 oracle blockers and SF2 scene wiring.
- Added a concrete scene-owned path-world adapter with generation-checked
  player auxiliary and linked-shot records, fresh per-statement relationships,
  real shared queues/stores, and explicit faults for unpublished services.
  Callback traversal now expires entries before reading predicate inputs and
  retains a prepared candidate across missing-input retries, without advancing
  its timer twice. Carry and part-target binding use live surface relationships
  and preserve the source's early exits before reading auxiliary records.
  The integration review exposed a duplicate representation of source 1CEA:
  authored part/counter operands now share the surface group's actual byte.
  Nine new tests cover these boundaries and a complete charge-orb invocation
  sequence; all 1,082 native tests and two integration tests pass in both
  profiles, along with 551 source tests, exact regeneration, architecture and
  the app build. **The scene adapter is not yet wired into `Game::tick`;**
  this remains a service-integration milestone, not a migrated encounter.
- Added the scene actor host composing source strategy selection, the shared
  path driver, hit/death services, paired contact cleanup and full retirement
  over one live object/resource world. Empty and suspended passes still publish
  one strategy clock; missing services latch a terminal diagnostic so an outer
  retry cannot repeat damage, map counts or allocations. Scene-specific native
  callback and map-continuation providers are required, with no default success.
  Added the source positional-loop owner: sample after each strategy, retain
  first-on-tie order and all authored control bits, preserve sounds from actors
  retired later in the pass, and keep accumulator reset/publication/freeze
  separate. Nine composed-host tests, three audio tests and four source proofs
  cover this layer. All 1,094 native tests plus two integration tests pass in
  debug/release; all 555 source tests, exact regeneration, architecture and app
  build pass. **The tested host uses explicit test callback providers; real
  scene/native registrations, render-work scheduling and `Game::tick` adoption
  remain open.** Do not represent the new host as a migrated live encounter.
- Bound the native player handler to real new/continuing/separation auxiliary
  registrations, using the same counted resource table as other actor data.
  The scene host now uses these concrete contact services by default. Contact
  feedback borrows the actual path particle byte and primary hit record;
  activity, mode, protection, action gates, sound and reflection use live scene
  owners. Missing inputs retain source read order, stale registration lookup
  faults instead of falling back to default damage, and separation preserves
  the shared damage/parameter values before releasing either contact.
  Nine composed tests include partial registration allocation failure,
  reflection/RNG, selected-versus-primary ownership and full retirement.
  All 1,103 native tests and two integration tests pass in debug/release;
  558 source tests, exact regeneration, architecture and app build pass.
  **Native death registrations, map continuations and production frame-loop
  adoption remain open.** Player-contact binding is not a complete player
  initializer or a migrated live encounter.
- Implemented the source charged-fire controller over the live player record,
  shared weapon parameters, real projectile allocator and numbered charge-orb
  installer. Authored orb callbacks now observe the same fractional charge
  state as the player service. Preserved processed press/hold distinctions,
  signed-byte decay, full-pool side effects, Walker recoil suppression, source
  cue order and terminal diagnostics after partially executed visits.
  Nine tests cover every fine-charge word, every control-byte/signed-level
  combination, real charged projectiles and an authored orb's complete
  scheduler/resource lifetime. Seven source-byte proofs include the input
  producer and the complete effect installer. All 1,112 native tests and two
  integration tests pass in debug/release; 565 source tests, exact regeneration,
  architecture and app build pass. **The outer player strategy, preceding
  rapid-fire/aiming service and production frame-loop adoption remain open;**
  this does not replace `Game::update_player_blaster` yet.
- Implemented the following rapid-fire tail and flight launcher, including
  full-byte queue admission, launch-rejection retry, processed alternate-fire
  edges, linked fixed-view muzzle relocation, publication ordering and the
  source's exceptional no-weapon-level return behavior. Firing observations
  now bind the actual actor's equipment, retained aim, roll and live shot count
  independently of path selection and attachment. Added scene-owned surface,
  impact and occupancy bindings needed by the real projectile paths.
  Eight tests cover all queue/equipment bytes, rejection/fault lifetimes,
  alternate variants and a complete launched rapid projectile's scheduler,
  count and resource lifetime, followed by a real charged-release cooldown.
  Seven source-byte checks cover the whole tail and flight helper contracts.
  All 1,120 native tests and two integration tests pass in debug/release;
  572 source tests, exact regeneration, architecture and app build pass.
  **Aiming publication, retained-target updates, consumable service and outer
  player/frame ownership remain open.** These services are not yet adopted
  by the shipping player update.
- Implemented weapon-pitch publication and its complete ordered target scan.
  Preserved source class/flag domains, wrapping signed range comparisons,
  first-on-tie selection, half-open angle windows, separate signed rounding,
  proxy overwrite/alias behavior and early writes before diagnostic faults.
  Twelve tests exhaust both class bytes, range bounds, angle windows, fine
  pitches and non-Walker modes; a composed check carries freshly published
  Walker pitch into a real alternate projectile. All 1,132 native tests and
  two integration tests pass in debug/release; 580 source checks, exact
  regeneration, architecture and app build pass. **The retained forward aim
  point has a separate producer, and consumable/outer player/frame ownership
  remain open.** These are still scene services, not shipping-loop adoption.
- Implemented the separately scheduled retained forward-point producer. It
  uses the live caller's position/yaw, published pitch, high-byte turning lead
  and three byte-quantized rotations, then updates the real shared proxy and
  that caller's retained aim. It does not perform another target search.
  Five tests cover every pitch/yaw pair and turning word, proxy/owner aliasing,
  partial-fault ordering and a subsequent real rapid launch after the player
  has moved. All 1,137 native tests and two integration tests pass in debug
  and release from the final tree; 584 source tests, exact regeneration,
  architecture and app build pass. Consumable use also installs a parallel
  timed player-action stream; its state and actual service calls must be
  ported before the enclosing weapon/player sequence is source-complete.
- Implemented the triggered consumable's parallel player-action stream over
  the live player record and shared projectile trigger. Preserved authored
  command order, same-visit early detonation, full-word palette snapshots,
  selective restoration flags, termination's final increment and overflow
  freeze. Eight tests cover every clock word, trigger byte, configuration and
  palette word, source read gates, non-replay faults and shared path ownership.
  All 1,145 native tests and two integration tests pass in debug/release;
  590 source tests, exact regeneration, architecture and app build pass.
  The consumable-use caller and its concrete effect installers are next;
  the outer player strategy and production frame-loop migration remain open.
- Implemented consumable dispatch and the actual healing/triggered installers,
  including wrapped type aliases, full-pool asymmetry, caller shield versus
  published capacity, numbered-child rejection and the shared effect formatter.
  Bound healing requests and primary target control to live scene owners;
  unified contact/path pitch recoil instead of retaining independent copies.
  Twelve new tests include both complete effect families through scheduling
  and resource retirement, same-update early detonation, every item/count,
  shield/capacity and protection/control combination, and read/fault ordering.
  All 1,157 native tests and two integration tests pass in debug/release;
  599 source checks, exact regeneration, architecture and app build pass.
  The outer input-delay/placement helper is not yet connected: its original
  relative-auxiliary-index writes require ownership analysis, not an assumed
  player-position update. Outer player/frame ownership remains open.
- Connected the ordered player-strategy prefix: pilot-specific limits and
  signed shield clamp, obstacle warnings, low-shield cues, gated shot reset,
  parallel action, equipment/motion publication and saturating visit age.
  Its shared-motion producer exposed and corrected duplicated scene inputs:
  path following and explosion scrolling now read the actual publication,
  while primary-motion inheritance reads the live actor's displacement.
  Eight tests cover every pilot/shield, clock/shield, mode and age value,
  real warning/action ordering, partial faults, caller identity and composed
  path/explosion consumers. All 1,165 native tests plus two integration tests
  pass in debug/release; 605 source checks, exact catalog regeneration,
  architecture and all three app launcher builds pass. **The following
  player-mode dispatcher and `Game::tick` adoption are still open.** This
  ordered prefix must not stand in for an unported movement strategy.
- Connected shared recovery requests to the real caller's shield and the
  source's numbered primary-following feedback actor. Requests clear before
  reads, addition wraps before capacity clamping, and exhausted effect slots
  do not undo healing. Existing feedback is neither restarted nor reformatted.
  Six tests cover shield/capacity boundaries, zero/missing-input order, full
  allocation pools, caller-versus-primary ownership, the final same-visit
  follow/retirement, and all three healing-emitter pulses through actual
  feedback lifetimes. All 1,171 native tests and two integration tests pass
  in debug/release; 607 source checks, exact regeneration, architecture and
  the app build pass. The enclosing post-movement player service still needs
  its remaining branches before these services can replace shipping control.
- Corrected the shipping SF1 helper pickup and shared pickup flash: immediate
  initializer visits, real exposed-player ownership, wrapped distance/height
  tests, correct collision-flag byte, allocation-gated sound/wing restoration,
  retained ship flags, source colour encoding and wrapping lifetimes. Removal
  marks do not return early, and linked fire is retired before subsequent
  allocation. The repair ship retains its separate deferred flash transition.
  Seven native tests include full pools, nonzero player slots, cleanup/reuse,
  successful wing handlers and a complete twenty-visit flash lifetime through
  the real strategy pass. Nine original-instruction differential tests now
  cover 2,067 executions, including every colour/lifetime and ship-flag byte;
  the harness now supplies the source strategy loop's actual WRAM data bank
  when probing higher object slots. The source's unused left-wing display
  transfer snapshot is deliberately omitted, with its lack of gameplay
  consumers checked separately. All 1,526 game/path/strategy tests pass in
  debug/release; the nine source-execution tests pass in both profiles; all
  37 SF1 source checks, architecture checks and three app launcher builds pass.
  This does not certify the other pickup families, repair-pod construction/
  movement, the two outstanding SF1 route/oracle failures, or SF2 frame wiring.

- Refreshed the complete workspace baseline after the player-input integration:
  3,866 harness tests pass, three fail, and one existing autonomous SF2 gate
  remains ignored. Cargo also runs standalone example checks; the failing
  targets are SF1 `semantic_trace`, `sf1_corneria_route`, and
  `sf1_training_trace` (forward velocity 64 versus 63 at tick 443). The earlier
  path and strategy fixture failures no longer occur. Both release launchers
  also completed a separate 240-tick hidden startup/readback smoke test with
  dummy audio; this proves startup, not picture or audio parity.
- Corrected the verification-only reference bus's missing DRAM-refresh stalls
  and low-bank FastROM speed, and made the SF1 timing diagnostic enter through
  actual source boundaries instead of fixed boot tick 890. Five new bus tests
  include every bank class and 682 start phases of the original horizontal
  wait. A fresh independent Mesen capture and reset-to-gameplay test agree on
  the first four Corneria counts `3, 3, 3, 4` in debug/release. Exact clock
  agreement and production source-derived timing remain open. The corrected
  reference exposes additional failures, which are retained: a full release
  oracle run passes 446 harness tests, fails seven and retains one ignored
  gate, with seven failing targets including its standalone examples. These
  are `semantic_trace`, `sf1_corneria_route`, `sf1_launch_video`,
  `sf2_intro_native_scene`, `sf1_title_trace`, `sf1_training_trace` and
  `sf1_weapon_trace`. The subsequently added independent timing test passes
  separately in both profiles, as do all 26 oracle library tests and the
  original bitmap-clear bus test. All 56 SF1 Python checks, the architecture
  audit and three app builds in debug/release pass. See `FINISHING_AUDIT.md` for exact scope and
  evidence. No native expectations, timing arrays or ignored tests were changed;
  the next work is source-bound synchronization and the exposed timing/RNG
  contracts, not accepting the new outputs as expected results.

- Corrected the SF2 opening's observed entropy-order contract: original
  refreshes can interrupt one actor between its random draws, so completed
  actor visits alone cannot determine value assignment. Added a native random
  source that preserves draw-level interleaving without observed RNG values or
  actor-state injection. The three original-code 440-update opening gates now
  pass with precise ordering. A separate independent Mesen comparison passes
  all 440 updates for slots, actor poses, camera and final RNG using Mesen's
  different refresh order; its palette/pixels/audio remain outside scope.
  All 1,240 native tests plus two architecture tests, 41 focused oracle/parser
  tests, the independent comparison and all app builds pass in debug/release;
  616 source checks and the architecture audit pass.
  The autonomous timing gate and shipping coarse refresh are still open.
  Source-bound SF1 synchronization and production timing remain next blockers.

- Fixed the Training handoff's duplicate movement initialization. The actual
  map queues a planet strategy and the original background-request routine
  installs it without resetting shared camera/weapon state. Two native tests,
  256 original-instruction cases and two source checks protect that boundary.
  The unchanged first-course gate passes 1,758 logic/draw/audio updates and
  1,752 scene-region bitmap comparisons through all thirteen required shape
  families and the restart, in debug and release. All 1,553 game/path/strategy
  tests, forty original strategy tests, 58 SF1 source checks, architecture and
  all three app builds pass. Front-end timing and the remaining route/oracle
  failures remain open; this is not whole-game or full-screen certification.

- Replaced fixed-boot and fixed-latency SF1 launch comparisons with actual
  strategy entry and original-bitmap-to-original-scanout association. The
  comparison still requires full-screen pixel equality and never selects a
  reference using native pixels. This exposed and fixed the retail launch
  warning's reversed blink half-cycle and missing aperture gate; the oracle's
  warning-counter address was also wrong. A new original-instruction test
  covers 69,632 counter/frame cases. Both launch anchors and the separate
  1,671-pixel warning layer pass, with every sampled launch camera checked.
  The first source-bound player state also agrees. Title logic now reaches
  all 96 checks before the retained duration failure. The warning countdown
  still advances early after the aperture, and its terminal publication and
  interpolation remain open. Source-ordered wipe/HUD lifecycle and the larger
  recorded timing/initialization replacements remain required; this is not
  full launch, front-end or campaign certification. All 495 core/game/render
  tests, 28 oracle library tests and ten focused launch/entry tests pass in
  both profiles; the release Training replay, 58 SF1 source checks,
  architecture audit and all three app builds in both profiles also pass.

- Replaced Corneria's initializer-count-based aperture trigger with the
  source map request, prepared-record, rendered-record and cleanup lifecycle.
  The three Corneria maps now retain the `nofadenostage` initializer tail and
  distinguish the later horizontal reveal. The original 80-update launch
  replay checks all sixteen star records, sprite locking, the last retained
  warning, black-window state and display fade. A separate original-code fade
  gate covers 65,536 input cases. The composed-video gate now compares sixteen
  full-screen scenes (5 through 20), retaining its separate warning-layer
  check. Corrected the verification PPU's brightness quantization from Mesen's
  independently implemented five-bit arithmetic, and corrected native black
  color publication to the current prepared transfer. The read-only Mesen
  capture agrees on the inspected fade colors but still differs on aperture/
  scanout boundary pixels; this is not independent whole-frame certification.
  The pre-resume initializer black hold, non-Corneria legacy initialization,
  production timing, reference color math and full campaign coverage remain
  open. The three refreshed map regression blobs change only one wipe operand
  and append the 24-byte initializer continuation; no original-code or pixel
  expectations were blessed. Focused original-code/video gates and native
  core/game/map/render regressions pass in both profiles; unchanged release
  Training, 62 SF1 source checks, architecture and all application builds also
  pass. The whole release workspace run still fails the four known semantic,
  controller-route, title-setup and weapon-entry targets (3,898 passing tests,
  three failing tests, one ignored test and two failing executable examples).
  Testing includes the preserved pre-existing working-tree changes.

- Independently closed the remaining launch aperture/scanout row discrepancy.
  All sixteen original normalized aperture buffers already agreed; the
  verification PPU was applying H-blank state one row early, and the native
  renderer compensated with a minus-one-row wipe offset. Corrected the
  publication boundary and removed that compensation. The independent run
  also exposed the sprite layer's missing first-visible-row delay, now fixed
  in both reference sampling and native scaled/source rendering. A new
  repeatable two-reset Mesen gate pins the ROM, specifies every controller
  button, disables frame skipping and requires identical original captures.
  Original BG1 bitmap identity and settled original scanouts select all
  sixteen full frames without consulting native pixels. All 57,344 pixels
  per scene 5..20 and every byte of all sixteen aperture records pass in
  debug and release. Original window state and sprite-Y wrap/flip have focused tests;
  unchanged release Training, renderer/GPU regressions, 64 SF1 Python checks,
  architecture and all app builds pass. The full release workspace records
  3,902 passing tests, three failing tests, one ignored test and two failing
  executable examples, retaining the same four failing semantic, route, title
  and weapon targets. The run includes preserved pre-existing working-tree
  edits. This does not certify production elapsed
  timing, other scenes or general reference-PPU accuracy.

- Closed the first-laser gate's fixed-boot entry failure using actual Corneria
  strategy entry and a shared gameplay-counter input tape. Every one of 338
  updates checks the original latched pad and both game counters; next-visit
  buttons must be presented before the original draw boundary because its IRQ
  can already sample them during strategy work. Ten laser-life updates, camera
  and laser draw commands, one firing sound and all 26 composed frames now pass.
  Two fresh Mesen resets independently check all controller visits and produce
  byte-identical RGB/VRAM evidence. All 57,344 pixels in each scene 312..337
  match a reference selected only by original bitmap identity and settled
  original scanouts. No weapon code, original expectations or native state was
  changed to obtain these matches. `tools/sf1/run_weapon_scanout_oracle.py`
  reproduces the independent gate. The existing launch decoder coverage is
  retained; independent debug/release comparisons, the 150-update banking
  branch, 65 SF1 Python tests and architecture checks pass. The full release
  workspace retains 3,902 passing tests, three failing tests and one ignored
  test, with one failing executable example. Only `semantic_trace`,
  `sf1_corneria_route` and `sf1_title_trace` remain failing targets. The full
  Training replay remains green; these results include preserved pre-existing
  working-tree changes. This does not close production
  timing, the remaining route/front-end failures or full weapon/campaign scope.

Leave a final handoff listing exact tested revisions, launcher commands, tests
actually run, and any unresolved limitations. Never label unfinished work fully
working merely because the user is due back.
