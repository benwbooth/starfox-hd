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
  native frame/scene integration. The adjacent special-weapon pickup still
  has the same score/flag/death-order discrepancies and is queued next.

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

Leave a final handoff listing exact tested revisions, launcher commands, tests
actually run, and any unresolved limitations. Never label unfinished work fully
working merely because the user is due back.
