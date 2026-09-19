# SF2 gameplay coverage — incomplete

Static audit of the production Rust path, 2026-09-19. **Not all SF2 game logic
has been ported.** Typed Rust state, closed extracted script graphs, and passing
recorded scenarios are different properties. None implies complete gameplay.
This audit does not run the original game or add new trace recordings.

## Reproduce the static inventory

From the repository root, with the user-owned retail ROM present:

```sh
nix develop --command python3 tools/sf2/audit_gameplay_static.py
nix develop --command python3 tools/sf2/audit_gameplay_static.py --json
nix develop --command python3 -m unittest discover -s tools/sf2/disasm -p 'test_*.py'
nix develop --command python3 tools/sf2/verify_difficulty_profiles.py
nix develop --command python3 tools/check_native_architecture.py
```

To save clean JSON without the devshell startup banner, redirect inside the
shell: `nix develop --command bash -c 'python3 tools/sf2/audit_gameplay_static.py --json > /tmp/sf2-static-gameplay-audit.json'`.

The inventory reads ROM bytes, follows the existing static extractors, checks
generated path counts, and inspects the normal/build feature dependencies of
`sf-app`. Its JSON includes every discovered logical opcode, handler address,
semantic name, command count, pointer-effect class, and map inline-call target.
It deliberately does not manufacture a whole-game completion percentage or a
native-equivalence claim from those records. Source-navigation hints are text
search results, not a Rust call-graph proof. Exit zero means inventory checks
passed, **not** port complete.

Source ROM SHA-256:
`e134f20f6ee7d422d06faea6b1ae1e4101d1d0a200a0571d715d1cf23d959e8c`.

| Static scope | Current result | What it does not establish |
|---|---|---|
| Discovered map roots | 25 roots, 4,094 commands, 22 opcodes | Whole-ROM entry-point completeness or native mission equivalence |
| Map inline control flow | 262 routines, zero unresolved exits | Called services and external state changes implemented in shipping code |
| Map spawns | 232 records, four initializer targets | Four enemy behaviors; these are generic initialization entry points |
| External phase gates | 237 | Native ownership and scheduling of every phase transition |
| Discovered path roots | 106 roots, 14,220 commands | Exhaustive indexed/dynamic root discovery |
| Path dispatch | 279 logical opcodes, 277 unique handler addresses, zero unresolved handlers/invalid records | General shipping execution of these semantics |
| Difficulty tables | Normal/Hard/Expert: 6/13/15 event records; 6/20/20 assignment rows | Shipping execution of every timed event and arbitrary campaign ordering |
| Shipping dependencies | No `sf2-map`, `sf2-path`, `sf-oracle`, `w65c816`, `oracle-bridge`, or `oracle-data` | Behavioral completeness; this proves a dependency boundary only |

Root discovery has a concrete limit: the indexed title/attract installer at
`$06:A96E` selects records at `$0D:D4C7`; it is analyzed separately in
`extract_intro_paths.py`. See `SF2_HD_RECONSTRUCTION.md`. Therefore the default
106-root graph must not be called all SF2 programs. The reference directory
contains SF1 assembly; analogous SF1 routines are supporting evidence, not
proof that SF2 branches are identical.

## Confirmed production gaps

1. **Staging interpreters are not shipping implementations.**
   `rust/sf2-game/Cargo.toml` enables `sf2-map` and `sf2-path` only through
   `oracle-bridge`; `src/lib.rs` gates the memory-backed hosts/strategies.
   Their handler coverage cannot close production gameplay rows.
2. **Neutral and steered player flight use different implementations.**
   `native/game.rs::update_active_flight` sets
   `departed_certified_neutral_path` on directional input or a mission-specific
   recording boundary. Before that, it dispatches to encounter-specific
   presentation/neutral updates and returns. For example,
   `update_reengagement_presentation` interpolates recorded player/camera
   keyframes; after departure, the general control routine uses live state.
   A neutral replay cannot establish the live control branch's equivalence.
3. **Recorded projectile lifetimes still govern actual gameplay.**
   `update_reengagement_projectiles` iterates 33 generated descriptors, spawns
   at `start_retail_frame`, removes at `end_retail_frame`, installs a recorded
   initial pose/collision eligibility, and looks up actions by track/frame.
   `generate_second_sortie_projectiles.py` documents callback-trace provenance.
   Arithmetic and collision use typed current state, but spawn/branch cadence
   is still a recording-derived schedule. This is not simply visual playback.
4. **Rival control flow still includes recorded schedules.**
   `update_final_rival_actor` consumes `final_rivals_flight::FlightPlan`,
   including frame-indexed actions, initial poses, hidden frames, and departure
   time. `generate_final_rivals_flight.py` identifies its trace fixtures and
   fixed encounter windows. Live steering arithmetic does not replace the
   original conditional path control flow.
5. **Campaign selection is constrained by a prescribed opening route.**
   `CampaignRouteStep`, `update_mode`, and `begin_campaign_sortie` sequence the
   opening encounters. Broader destination selection happens in
   `update_post_mirage_encounter_selection`. Travel durations depend on route
   step, and some travel updates interpolate Corneria damage toward fixed
   encounter-specific endpoints. This is not a port of a general strategic
   event scheduler merely because source difficulty tables are decoded.

These are counterexamples sufficient to reject “all gameplay already ported.”
They do not imply all native logic is playback: object allocation, control
arithmetic, combat, mission objectives, and many enemy state transitions
genuinely run in Rust.

## Subsystem coverage ledger

All paths in this table are relative to `rust/sf2-game/src/` unless qualified.
“Partial” means implementation exists but source-equivalent full behavior is
not established. “Unclosed” means this audit has no complete branch mapping;
it is not a claim that every behavior in the row is absent.

| Subsystem | Production evidence | Status / remaining static proof |
|---|---|---|
| Map/path programs and inline services | Feature-gated `map_host`, `path_host`, `strategy`; typed bespoke `native/game.rs` | Staging only for generic interpreters; map source branches to typed production owners |
| Strategic map and Corneria defense | `update_mode`, `update_corneria_defense`, `CampaignRouteStep` | Partial; replace prescribed route/time/damage schedules with source event control flow |
| Difficulty, world assignments, RNG | `native/state.rs::{DifficultyProfile, RandomState}`, `native/campaign_world_assignments.rs` | Data and typed operations present; consumption order and all difficulty branches unclosed |
| Pilot/wingmate selection | `update_pilot_selection`, `PilotCraftProfile`, `Roster` | Present; no whole-source branch or in-mission wingmate-AI closure |
| Arwing flight and follow camera | `update_active_flight`, `update_active_camera`, `update_certified_neutral_flight` | Partial; neutral/live split, timing ownership, full control/input branch mapping |
| Transformation and Walker | `update_player_transformation`, `update_active_walker`, `WalkerJumpState` | Present including easing/jump wind-up; full source equivalence unclosed, not simply “missing Walker” |
| Rapid/charged weapons | `update_player_blaster`, `new_player_projectile` | Present; other weapon, item, targeting and maneuver branches need source census |
| Full controller behavior | `native/input.rs`, production `Button` consumers | Unclosed; no production `Button::A` consumer in `native/game.rs`; shoulder/Y handling exists for Walker and Y boost for carrier corridor, not proof of general flight controls |
| Opening/re-engagement enemies | `update_mission_fighter_combat`, `update_reengagement_targets`, continuation modules | Partial; typed movement plus finite encounter/operation schedules |
| Missile and fighter interception | `update_interception_missiles`, `update_fighter_intercept_targets` | Partial; native actors with presentation/action catalogs; arbitrary timing and outcomes unclosed |
| Pigma/Leon/final rivals/Wolf | Rival update functions, `final_rivals_flight.rs` | Partial; general branching/cadence and off-recording lifetime behavior not fully ported |
| Recurring attackers | `update_pressure_fighter_actors`, `update_live_pressure_fighter_projectiles` | Live typed paths exist; do not classify the whole subsystem as playback; full source coverage unclosed |
| Hostile projectiles | `apply_hostile_projectile_actions_with_collision`, encounter projectile catalogs | Arithmetic present; some production spawn/retire/operation decisions remain recorded |
| Eladard, Titania, Macbeth | `update_eladard_base`, `update_titania_base`, `update_macbeth_base` | Native objectives, switches, rooms and combat present; map/strategy branch-by-branch coverage unclosed |
| Fortuna, Venom, Meteor | Respective `update_*_base`, Kick Gunner, Queen Dragoon and core updates | Native objectives and bosses present; not missing worlds, but full routing/AI/side-effect coverage unclosed |
| Battle Carrier | Corridor gates/defenders/reactor update functions | Native control/combat/objective progression present; source scheduling, alternate outcomes and full traversal unclosed |
| Mirage Dragon and Astropolis | Dragon head/segments, `update_astropolis_assault`, typed mission phases | Partial; native combat plus fixed presentation/destruction windows; full branch correspondence unclosed |
| Collision, damage, death, scoring | `resolve_mission_collisions`, `apply_player_damage`, `update_objects` | Typed implementation exists; special target ordering, all collision classes and source side effects unclosed |
| Continue, results, progression | `update_game_over`, `update_results`, `CampaignProgress`, `native/results.rs` | Native state transitions exist; source-wide persistence/record/continue branch closure not established |
| Intro, menus, messages, ending | Native state transitions plus app presentation tracks | Separate presentation gap; see `SF2_HD_RECONSTRUCTION.md`; recorded frames do not establish native choreography |
| Audio gameplay events | `SoundEvent`, `AudioState`, spatial sound methods | Semantic events exist; all source cue triggers, queue overflow and ordering unclosed |
| Tick/service scheduling | `Game::tick`, per-encounter retail-frame action tables | Partial; constant presentation-frame conversion is not a general source service scheduler |

The inventory does not yet enumerate every whole-ROM callable gameplay routine.
That missing denominator is itself an open coverage item. No percentage is
appropriate until direct, indirect, indexed and dynamically installed entries
and their continuation edges have been accounted for.

## Static-first completion order

1. Expand source entry/dispatch inventory, including indexed installers and
   external map phase writers. Record unresolved targets explicitly.
2. Recover predicates, waits, calls, child creation, lifetime and random-state
   effects of each gameplay path. Use the 279-handler catalog as navigation,
   not permission to ship the verification memory model.
3. Implement those decisions as typed Rust state machines. Begin with generic
   player control and the projectile/rival schedules identified above. Keep
   ROM-authored constants/data; replace recording-derived decisions.
4. Port the strategic event scheduler and destination-driven campaign routing,
   then close each world's and boss's reachable strategy branches.
5. Check source-to-Rust correspondence, boundary arithmetic and feature wiring.
   Use runtime comparisons only for specific ambiguities or independent final
   validation, not as the primary source of executable gameplay schedules.

Verification for this audit: all 49 existing disassembly/static-source tests
passed without skips, difficulty-table verification passed, and the inventory
and architecture checks passed. Those results do not certify an input-only
campaign playthrough or fix earlier workspace/runtime regressions. Existing
tests that directly move actors or apply damage prove narrower state-machine
behavior, not an unassisted campaign.

## Static port implementation checkpoints

The contact subsystem now has ROM-extracted profiles for all 61 shapes using
compound object-contact boxes, word-exact center/overlap math, directional
contact storage and separation, callback-driven hit response, and the ordered
collision queue/epoch pass. `native/collision_pass.rs` uses the ordinary object
store: profiles are snapshotted at queue build, while poses, links, shape
equality and exclusion groups are read during detection. Each probe box visits
all later candidates before the next box, preserving repeated contact counts.

Source frame branches at `$03:8027` and `$03:80AE` build the queue, run cleanup
and detection through the render-work wrappers, then enter the strategy pass.
Cleanup copies pending-hit into previous-hit and clears pending-hit; detection
sets pending-hit again; strategy dispatch reads pending-hit, not previous-hit.
Thus collision animation uses the shared clock before the strategy increment.
Both the branch ordering and the flag distinction are checked against assembly
bytes, not recorded frame timing.

Production geometry queries use the extracted box catalog. The generic queue,
contact-response callback hosts, and split strategy scheduler are implemented
services but are **not yet the production Game frame owner**. Their presence
does not close collision/damage/lifecycle or service-scheduling coverage above;
authored flag initialization, callback registration, full world retirement, and
frame-service integration remain required. The queue checkpoint has ten Rust
tests passing in debug/release, six assembly-byte tests, and successful native
architecture and application builds. These are static/synthetic checks only.

`native/player_hit_control.rs` now transcribes the two contact-protection
countdowns, light/heavy impact state, reserve absorption, impact cue selection,
deflection feedback/cooldown, and contact-bearing turn arithmetic. Recovery
decrements every strategy visit; the shared clock only controls blink marking
and bank-impulse direction. Reserve absorption has no hull-damage spill. These
leaves have seven synthetic Rust tests in debug/release and seven source-byte
checks, including the reused native bearing table. The whole player-contact
callback, reflected-weapon path, and production player frame owner remain open;
this is not replacement of the legacy frame-based recovery schedule yet.

`native/player_contact.rs` now implements the complete player contact callback
control flow: early and global gates, timed versus projectile-only deflection,
sound-before-random ordering, part-hit feedback consumption, contact-turn gates,
heavy impact and reserve absorption. Its required world interface dispatches
reflection and sounds; it does not silently omit either operation. Six synthetic
tests exercise all protection-byte values, all box-hit flag values, and actual
composition with new/continuing hit response and separation. Five assembly-byte
checks cover the branch and dispatch ordering. Reflected-weapon world operations
and production Game integration are still required; these interface tests do
not demonstrate live reflected projectiles.

Shared weapon geometry now includes source-exact reflected pitch/yaw offsets
and optional two-byte scatter. Hostile-launch classification retains the
primary-player yaw half-plane test, conditional random collision suppression,
and two wrapping counters. Nine Rust weapon-geometry tests pass, including
exhaustive angle pairs and random-byte pairs. The multi-reflection loop still
requires its allocator and launch-origin data-flow port; these math leaves do
not imply that the production game emits reflected weapons.

The scene-proxy pool and world-retirement ordering are now typed services.
`native/scene_proxy.rs` retains the source's independent 512-entry pool,
after-head insertion, pose/continuation snapshots, deferred actor detachment,
and explicit immediate release. `native/retirement.rs` orders proxy detachment,
both contact callbacks, relationship cleanup, required program release, and
only then actor-slot recycling. Synthetic full-pool tests verify that contact
callbacks cannot allocate the retiring slot, while its programs and child links
are still available. Five proxy tests, three retirement tests and six source
checks cover this checkpoint; production world ownership and program-allocation
adapters remain to be connected.

`native/program_resources.rs` now ports shared resource capacity, first-fit
free-list priority, high-end allocation, the six-byte whole-block threshold,
adjacent coalescing, and newest-first actor ownership release. Payloads remain
typed Rust values; no source addresses or byte storage are exposed. Six
synthetic allocator tests and six assembly-byte checks cover this layer; the
three retirement tests now use this resource pool rather than unconstrained
callback vectors. Debug/release tests, architecture checks, and the app build
pass. The source's empty-free-list branch returns a nonzero cost as though it
were an allocation; this malformed-source-state case is explicitly reported
as `EmptyFreeList`, not silently treated as a valid resource. Descriptor
construction, resize/copy semantics, and production world integration remain
open. These results do not claim live gameplay completion.

Program resource replacement now allocates before releasing the old record,
preserving peak allocation pressure and ownership ordering. Typed callers
preserve meaningful fields; this is not a generic byte-copy service.
`native/program_state.rs` adds the shared subroutine/count-loop stack, with
one entry for a call and two for a loop, zero-count wrapping, paired loop
completion/exit, retained empty storage, and reallocation on every eighth
push. That reallocation can shrink after earlier pops. Tests cover mixed
call/loop nesting, replacement pressure, partial loop setup on allocation
failure, and explicit rejection of corrupt overflowing stack counts. Thirteen
focused Rust tests pass in both profiles, with twelve static source checks.
Callback-root return gates, callback descriptor construction/dispatch, and
production path integration are still outstanding.

`native/path_triggers.rs` now owns typed trigger records in the same program
resource pool as path stacks. Registration retains duplicates and allocates
replacement storage before freeing the old list; cancellation removes only
the first matching path and does not shrink storage. The mutable pass retains
the source's current-entry deletion budget adjustment, timer expiry before
predicate evaluation, and stale timer observation for indefinite triggers.
Tests cover cancellation before/current/after the active entry, additions and
clear during callbacks, shared allocation pressure, and the explicit fault at
64 records where the source's byte-sized allocation cost wraps. Eight Rust
tests pass in debug and release, plus seven assembly checks. The list/pass
mechanics do not yet evaluate trigger predicates or dispatch path callbacks,
and are not yet connected to production Game execution.

Callback return/redirect ownership is now ported in `native/path_calls.rs`,
including root returns without a stack pop, ordinary versus callback depth
accounting, per-callback redirect reset, forced continuation replacement, and
deferred calls through the shared actor stack. Pending redirect mode survives
a pass with no eligible callbacks, matching the source's lack of an entry reset.
`native/path_trigger_conditions.rs` evaluates all eighteen authored predicate
cases (including seven periods) from current typed world observations. It
preserves consumed versus persistent flags, player selection/rearming, and the
source's asymmetric secondary-player part-target branches. Twelve focused
Rust tests pass in debug and release, including composition of actual trigger
storage, predicates, callback returns, redirects, and a parent loop stack.
Sixteen assembly checks cover the storage/control/predicate source blocks.
Production world adapters and decoded path command dispatch remain required;
synthetic composition tests are not evidence of live gameplay completion.

Static review corrected decoded opcode `063` from the erroneous
`AccumulateObject1cde` label to `DoVariableWord`. It pushes the body continuation
and a full variable word onto the shared path stack; it does not add values to
the stack handle. The generated catalog and verification-only path dispatcher
now agree with that source flow, and the obsolete arithmetic host operation
is removed. A synthetic dispatcher test covers all 65,536 counts and nested
word loops in debug/release; six stack source checks and eleven extractor
checks pass. No recorded gameplay or original-program execution was used.

`native/path_runtime.rs` now connects callback storage, predicates, calls and
redirects to real `ObjectStore` actors. Object extensions own their path stack,
trigger registrations and condition state; the shared coordinator owns pool
capacity, call depth, timer observation and selected-player context. Callback
entry refreshes player selection even for nonselecting predicates. Immediate
redirect effects modify live strategy/wait/repeat fields, while the path
destination remains deferred until batch completion. Hit response now borrows
the same object's health and contact flags, avoiding a second mutable health
record or post-callback copy-back. Five integration tests cover nested returns,
parent loops, fresh health predicates, redirects, mutable registration, program
release, invalid host ordering, and damage-to-trigger attribution; they pass
in debug/release. Seventeen trigger assembly checks cover source control flow.
This connects the typed systems to real actor records, but does **not** yet
replace `Game`'s recorded mission controllers or provide general decoded path
command execution. Those production gaps remain open.

`native/path_commands.rs` now executes decoded wait, single-step wait, byte
repeat, jump/goto, call/return, word loop, pair discard, trigger registration,
cancellation and deferred-redirection statements against those live actors.
Each statement carries typed continuations and values; there is no generic
memory or encoded-operand dispatch. Immediate NEXT refreshes player selection;
ordinary jumps do not. Byte LOOP compares before incrementing, independently
of the word-counted loop stack. Static review also corrected the forced-path
reset field from a presumed steering value to that byte LOOP counter, and
removed an overstrict loop-frame requirement on pair discard: the source also
permits discarding two call continuations. The contact callback parameter now
aliases the actual speed-target field, matching the shared source byte.
Seven command tests (including every wait/repeat byte pair), seven stack tests
and seven command assembly checks cover this layer. Complete authored-program
lowering, the remaining statement families, movement/service orchestration,
and replacing production recorded controllers remain open.

Path motion now operates directly on actor fields before callbacks, selecting
ordinary versus local relative heading, applying acceleration before bank
turning, then player displacement and optional per-step velocity regeneration.
Relative-position integration is a separate post-callback operation and reads
the latest flags and velocity; a callback can intentionally cause both world
and relative integration in one invocation. Six decoded motion-configuration
statements preserve immediate versus deferred regeneration. Eleven movement
tests and eight command tests pass in debug/release, with seven new assembly
checks. The catalog's former `RelativeToPlayerOn/Off` names are corrected to
`GenerateVelocityEachStepOn/Off`; displacement following uses the separate
`FollowPlayerDisplacementOn/Off` pair. Catalog command/root counts and raw
program bytes are unchanged. Child refresh, selected-target carry correction,
path-exit latch cleanup and the complete movement-service caller remain to be
integrated; this does not claim that the production scheduler now runs all
authored paths.

The post-callback movement service now publishes the source first-child chain
(not siblings), integrates retained relative coordinates, applies selected-player
platform carry and clears the path-exit latches. Attachment publication reuses
the source matrix shared with the intro: extension-parent precedence, extension
self-reference bypass, and base-self-reference angle publication order are
explicit. Carry retains wrapped horizontal scaling, signed truncation, unrotated
vertical displacement, and the source's distinct byte/word snapshot writes.
Twenty-three focused attachment/carry/movement tests pass in debug and release;
thirteen movement assembly checks, the architecture check and `sf-app` build
pass. No recorded gameplay or original-program execution was used. The common
post-callback service is available to the native path owner; complete authored
program lowering, pre/post movement orchestration through that owner, and
replacing `Game`'s recorded controllers remain unfinished.

`PathRuntime` now owns the movement invocation across its callback batch and
resolves selected-player carry after callbacks, enforcing pre/post ordering
and rejecting nested or prematurely completed movement. Six typed facing
commands cover selected, fixed-player and linked targets with their distinct
immediate/quarter-step/eighth-step rules. Linked absence is a source no-op;
selected full-facing and yaw-only variants have different shared-result effects,
and only selected/fixed variants update retained relative yaw. Tests exhaust
all byte-angle pairs for both chase rates; focused runtime and command tests
exercise real actor/callback state. The authored catalog still needs lowering
to these statements and the production game still needs its recorded controllers
replaced; native command kernels alone are not completion.

Static review corrected three more catalog meanings: `ContractSelectedRadius`,
`ContractLinkedRadius` and `ContractLocalRadius` scale a three-axis radius;
they are not pitch rotations. The linked form reads a signed literal byte,
not a variable. Native commands now apply the source normalization kernel to
world or retained local coordinates, and the verification dispatcher uses the
same corrected semantics. Horizontal orbit commands use the source's full
Q15 matrix, including zero-angle truncation, rather than the lower-precision
byte-trig rotation helper. Nine steering tests pass in debug/release, and the
literal-versus-variable regression passes in the Rust verification dispatcher.
Nine steering assembly checks pass; regenerated catalog counts and raw script
bytes are unchanged. This is static-source coverage, not recorded-gameplay or
original-executable evidence, and full production path dispatch remains open.

Native branch commands now share the source IFNOT latch across actor and
callback entry. Repeated IFNOT sets rather than toggles it; equality, wrapped
numeric intervals, horizontal distance and ground-threshold tests consume it,
while nonzero, proximity and angular tests preserve it. A missing linked
distance target skips before consumption. The two proximity forms deliberately
remain distinct: geometry length ignores height, while the range predicate
bounds depth and wrapped X/Y Manhattan distance. Angular predicates retain
their opposite bearing directions and different heading owners. Seven numeric
and spatial tests, the live command-dispatch regression and eight assembly
checks cover this layer; the focused native path suite passes 83 tests.
These statements still need authored-program lowering and production dispatch.
