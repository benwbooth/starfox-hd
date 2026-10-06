# SF1 and SF2 completion plan

Updated 2026-10-05. Status: implementation in progress; neither game is certified.

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

### Current execution checkpoint — 2026-10-05

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

Leave a final handoff listing exact tested revisions, launcher commands, tests
actually run, and any unresolved limitations. Never label unfinished work fully
working merely because the user is due back.
