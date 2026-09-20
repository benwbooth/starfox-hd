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

Further branch transcription covers zero, wrapped second-minus-first variable
comparisons, byte/word mask tests and vertical ordering without consuming IFNOT.
Following the actual common exit routines corrected two misleading catalog
names: opcode 36 branches when selected height is at or below the actor;
opcode 257 branches when it is above. Both use wrapped subtraction sign, not
widened order. Native hit branches consume the shared hit-event latch or just
the matching hit-mask bits. The latter operand is a literal mask, not a bit
number. The verification host's ground predicate now takes the nonnegative
height-plus-offset branch. These are static source corrections; neither
recordings nor original-program execution determined the expected behavior.

Spatial dispatch now samples actual object-store transforms and links at the
statement boundary. Linked-distance tests neither require nor replace selected
target state. The selected right-plane and forward-plane branches reuse the
source byte-rotation/high-product projection, with the selected actor as the
plane's origin and orientation; they are not world-depth comparisons. END
marks deferred removal and clears only exit latches without movement or cursor
advance. PATHHOLD retains its cursor and installs the movement-only behavior.
Neither may substitute for a callback RETURN; invalid nested termination is
reported explicitly. Focused native path coverage now passes 89 release tests;
98 static path checks and three synthetic verification-host condition tests
pass without executing the original programs. Whole authored-program dispatch
and production scheduler replacement remain open.

Path arithmetic now addresses named actor fields directly: world/local
coordinates, velocity, angles, speed, health, attack/hit state, path counters
and part identifiers. Byte views of numeric words preserve the other byte;
copy operands are read before writes, and source byte-to-word copies/adds
sign-extend. Assignment, addition, increment, decrement and negation retain
their destination widths. Ordinary speed/angle variable writes do not invoke
SETVEL's velocity generation. This is a typed operand subset, not a claim
that every encoded actor/world field is lowered or every path now ships.
# Native path catalog dispatch (static-port work)

The typed native path runtime now executes immutable semantic catalogs through
explicit command indices. Actor operands are sampled at execution, including
waits, counted loops, and comparisons. Call/return, movement yields, and callback
returns retain their separate scheduler boundaries; immediate dispatch does not
repeat common-entry player selection. Missing catalog entries and exhausted
diagnostic budgets return errors, never a successful no-op or fabricated tick.
Five synthetic dispatcher regressions pass, as do all 99 native path tests in
release, 105 static path source checks, the app build, and architecture checks.
This is dispatcher infrastructure, not completed authored-catalog lowering or
Game integration. No original CPU execution or recorded gameplay was used.

Source animation controls now have a typed packed-channel kernel preserving
automatic versus manual selection, retained automatic payloads, wrapping adds,
and the single correction/subtraction rule (not modulo). All 16,777,216 byte
combinations pass an independent arithmetic formulation. This kernel still
needs ownership and clock integration in the authored-path actor lifecycle.
SPRITE is wired through typed path dispatch, actor render channels, Game render
objects, and the app's existing scaled-sprite renderer flag. Pure Rust boundary
tests and four new static assembly checks pass. The verification-only host's
incorrect writes to shared bytes were corrected to actor-local channels.

The first complete authored path, alternate exhaust, is now lowered offline
into five typed native statements. The lowerer follows the static graph,
verifies its source installer and handler identities, and rejects an entire
graph if any statement is unsupported. Its native regression runs Sprite,
DO 3, colour-frame addition, NEXT, and END, including precisely two movement
yields and no movement on the final END pass. Path-owned shape/colour controls
are separate from resolved rendering snapshots; publication takes an explicit
world animation clock and never advances it. This is 1 of 106 discovered roots,
not completed Game scheduling or source spawn integration. The static audit
reports these counts separately and checks generated native catalog freshness.

The colour-cycle sprite is the second complete lowered graph (13 statements
across both roots). Static source identifies opcode 92 as collision disable:
its actor flag is the same gate consumed by the existing collision queue.
The eight-command graph preserves INITCOL 0's initial wait, the following
seven-count loop, seven movement yields in total, and final no-movement END.
Source naming now reflects that collision meaning. Remaining roots, Game
scheduling, source spawning and shared-clock ownership are still open.

The randomized colour particle is the third complete lowered graph, bringing
the native catalog to 25 statements. Its three centered word mutations draw
from the world's borrowed RNG, first byte high then second byte low, including
when masks are zero. The retained motion-phase word's low-byte counter aliases
the actual motion field, preserves its high byte, and is distinct from the
platform's saved yaw. Literal comparisons retain both source graph edges.
The complete regression consumes six random bytes once, runs six movement
yields from phase zero, and takes the phase-seven early END without a final
move; the next actor continues the same random stream. Dispatcher world inputs
now also supply the animation clock for resolving presentation snapshots.
This remains three statically lowered roots, not production Game scheduling.

The local-jitter sprite adds a fourth root and raises the unique-statement
count to 37. Its earlier-addressed shared subroutine adds three local random
offsets and returns to the caller's continuation; the subsequent IFNOT loop
uses two yielding GOTOs and terminates on phase three. All roots now share
one deduplicated semantic layout so a shared source statement has one cursor
identity regardless of which root reaches it. Source addresses remain solely
in the offline lowerer; native execution uses catalog indices. These paths
still await integration with Game's spawn and strategy scheduling services.

Further static branch review corrected opcode 275: both auxiliary bytes come
from the selected actor's slot, never the current actor's base or slot. Mode
bit 64 permits checking the action flag; without it mode bit 128 rejects.
Eligible modes still require action bit 32, and IFNOT remains pending. The
native predicate and verification-only host now agree with the full source
branch, including raw zero selection in that host. All 65,536 flag pairs are
tested. The semantic name is now `IfSelectedAuxiliaryContinuation`; this does
not yet make the surrounding auxiliary-dependent paths production-complete.

The auxiliary-gated sprite graph is now the fifth lowered root (48 unique
statements). Its sprite-size byte aliases texture X, wraps from 255 to 1,
and reaches 3 and 5 at the following movement boundaries before resetting.
The later loop samples explicit selected-auxiliary observations on each
invocation, preserves IFNOT, and exits when the gate changes. Missing required
observations fail at that branch without silent fallthrough. This tests the
world-input contract, not yet Game's auxiliary-state adapter or live spawning.

The offline lowerer additionally supports literal/zero assignment, word adds,
all byte/word increment/decrement/negation variants, zero/nonzero branches,
variable byte/word DO counts, and literal/variable WAIT. SET's value-first
operand order is kept distinct from ADD's field-first order. Variable-byte
DO uses unsigned widening, unlike signed byte-to-word arithmetic, and snapshots
its count only on entry. A native regression executes zero's complete 65,536
iteration wrap and counts above 127 without sign extension. This leaves the
catalog at five complete roots: the next child-producing graph is still rejected
at SpawnChild rather than publishing an incomplete path. Verification: 117
native path release tests, 15 lowerer tests, 119 source-byte checks, and the
native architecture check pass; no recorded gameplay was used for this work.

Child detachment now lowers the sixth complete graph, the child-detaching
sprite (60 unique statements total). Its auxiliary gate reads action bit 64,
not mode bit 64, and the final child unlink runs only after the extra yielding
GOTO when that gate is clear. Unlinking preserves the active object list, world
pose, extension parent, and owned resources; it clears attachment/lifetime
flags before searching, but clears the base links and child identifier only
after finding the child in its parent's chain. Native tests cover first,
middle, last, sibling-initiated and self unlink, duplicate identifiers,
missing children, and malformed-chain errors. The older verification host's
auxiliary action-byte read was corrected independently from mode mutation.
The Game scheduler/spawn adapter remains open; this milestone establishes
static lowering and typed path execution, not complete gameplay integration.

The child-attachment service now ports the source tail append independently
of active-list allocation. Spawn-parent selection follows exactly one mother
link when the caller's attached-coordinate flag is set; it does not use the
child-refresh flag or recursively seek a root. Attaching sets the authored
identifier and attachment/lifetime flags without changing world pose,
extension parent, resources or active ordering. Tests exercise empty and
nonempty chains, zero/duplicate identifiers, the reversed active insertion
order, subsequent unlinking, and explicit malformed-chain diagnostics.
Verification passes 127 native path release tests and 125 source-byte checks.
This is a prerequisite service, not yet a lowered SpawnChild command: fresh
object defaults, operand evaluation, failure handling, inherited state and
child-program graph closure still need their complete port.

Graph discovery now closes over independent spawned programs, including
recursive/shared child entries and the seven-byte quick-spawn form, without
turning those dependencies into the parent's control-flow successors. Null
child paths remain absent. Static literal decoding distinguishes the compact
14-byte child record from the rotated 17-byte form: health/attack occupy
bytes 5/6 or 8/9 respectively, position words stay signed, and byte identifiers
and rotations remain unsigned. Full source operand-reader checks and 21
lowerer tests pass. Spawn commands still reject lowering; six roots and 60
native statements remain the supported catalog, not an expanded gameplay claim.

Typed child spawning now composes scoped pool allocation, fresh-record
initialization and child-chain attachment. The new constructor preserves the
source's initial path hold, first-strategy exemption, search eligibility,
draw-admission observation, pause-mode input and group identity. Draw admission
is separate from authored invisibility. Child setup retains local coordinates
without eager world conversion, inherits target/group from the caller (not
necessarily the attachment parent), and updates the shared last-spawn identity.
Last-slot allocation only scans the caller's active-list suffix. Exhaustion
clears last-spawn and reports an explicit native fault: the original continues
writing fields through its null result, which is not reproduced as arbitrary
native memory corruption. Malformed chain diagnostics identify any allocated
child retained at the error boundary. Tests pass for six spawn scenarios,
12 object-pool/default cases, 133 native path cases and 137 static path checks;
the app builds. Catalog dispatch and Game integration of spawning remain open.

Spawn dispatch now advances the parent immediately while leaving the child's
program for a later scheduler visit. Missing initializer inputs, missing child
catalog entries and allocation failures retain the parent cursor and report
errors. The runtime owns the shared last-spawn selection. A spawned child's
one-time path-strategy prefix runs at its first program entry: it enables the
source shadow/contact/footprint-exclusion/draw-distance flags and clears only
the repeat counter, preserving wait, stack, call depth and pending-call state.
This prefix is not reapplied on subsequent entries or callback resumes.
The lowerer supports both child record forms for the reviewed transient-sprite
family; unreviewed shape categories still fail explicitly. The F561 parent now
passes its spawn lowering but is rejected at its child's unported sound cue,
so the published catalog remains six complete graphs, not seven. Verification
passes 138 native path tests in debug and release, 23 lowerer tests, 140 static
path checks, architecture/dependency checks and the app build. World/Game
scheduling and the remaining audio service are still separate open work.

Direct and paired authored sound cues now lower into typed cue/parameter/target
records. The shared audio ring preserves ordering with existing gameplay
sounds, source wraparound, and the routing OR rule: primary-player and fixed
fallback listeners do not clear an already-authored secondary bit. Path entry
and resume retain their different selection semantics. Missing audio inputs
fault before cursor advancement. The app explicitly reports unsupported PCM
mapping for authored cues; this is queue-service coverage, not audible cue-18
coverage, and no original sound program was executed for this milestone.

This enables the complete repeated-child sprite graph and the independently
installed sound/color sprite entry: eight roots, 79 globally deduplicated
statements. A typed-runtime test runs the parent for eleven invocations, checks
three separately scheduled children, and verifies each child's shared-RNG
jitter, one-shot secondary cue, color loop and termination. The two new roots
remain outside Game scheduling. Verification passes 142 native path tests in
release, 19 path-program tests in debug, 25 lowerer tests, 144 static path checks,
14 app audio tests, architecture/dependency checks and the app build. Other
sound classes, remaining source graphs and full gameplay integration stay open.

The callback-gated sprite now lowers as a complete 18-statement graph, bringing
the catalog to nine roots and 97 unique statements. Always-trigger registration
and forced post-callback redirection use the existing typed trigger services.
Its reviewed inline block becomes a named Rust action after full static byte
validation: read the primary player's view-side filter and latch the effect's
low phase byte to one, leaving the high byte and false case untouched. Draw
preparation proves that this is a view filter, not a death/damage flag. Primary
identity is distinct from current target selection, and absent/stale primary
inputs fail explicitly. The source run-while-paused flag is also retained.

Runtime tests cover all phase bytes with four high-byte patterns, contradictory
primary/selected observations, unchanged inversion/RNG, repeated callback
returns, and both exits through deferred redirection. Unsupported inline
actions and modified source signatures remain hard lowering errors. All 144
native path tests pass in debug/release, with 27 lowerer tests, 148 static path
checks, architecture/dependency checks and the app build. Game scheduling,
view-filter production/render integration and other graphs remain open.

Trigger lowering now covers conditional, unsigned-relative and timed
registration, cancellation, and clear-all. All 18 source condition selectors
become named Rust predicates; other selectors are rejected. Timed registration
retains the separate duration byte and uses the existing wrapping duration-plus-
one constructor. Relative destinations wrap within path data. Cancellation
requires an existing catalog identity rather than manufacturing an executable
edge for an otherwise undiscovered path. This extends lowering support without
claiming more complete roots: the catalog stays at nine roots/97 statements.
Verification passes 31 lowerer tests, 151 static path checks, 14 focused native
trigger/condition tests in release, and the architecture/dependency audit.

Variable-to-variable copies and adds now lower for all eight byte/word opcode
forms, including the byte-add alias. Full static helper/handler checks establish
destination-first/source-second operand ordering, signed byte-to-word extension,
and low-byte-only word-to-byte writes. Reviewed speed, repeat, health, attack,
relative-rotation and actor-part operands map to their existing typed fields.
Overlapping reads/writes are checked across every low byte and four high-byte
patterns, preserving unrelated object state. Verification passes 33 lowerer
tests, 153 static path checks, six arithmetic tests in debug/release, 145 native
path tests in release, architecture/dependency checks and the app build. This
does not enlarge the nine-root catalog or claim Game scheduling completion.

Variable-bit set, clear and test now lower into typed word operations. Source
review establishes a one-based byte selector with wrapping doubling: values
128 apart alias, and values beyond the sixteen conventional bits read adjacent
source bytes as masks. The offline generator decodes all 128 reachable words
into constant data only when needed; gameplay never executes those bytes or
resolves their source addresses. Tests cover every selector, both overlapping
byte views, multi-bit masks, immediate branch continuation and preservation of
the shared IFNOT latch. Verification passes 35 lowerer tests, 156 static path
checks, 147 native path tests in debug/release, architecture/dependency checks
and the app build. Catalog coverage remains nine roots/97 statements; full
gameplay integration and unported world services remain open.

The catalog lowerer now connects eight reviewed motion commands (speed,
displacement-follow and per-step-velocity toggles, banking toggles and velocity
quadrupling), BREAK, stack-pair discard, and both consuming hit branches to the
existing native services. Hit masks follow the target word in source data and
remain literal masks. Signed byte/word halving is newly implemented with
rounding toward zero; logical byte halving is separate. All 65,536 word values
and 256 byte values pass source-rule arithmetic checks. The complete checks
pass 38 lowerer tests, 159 static path checks, 148 native path tests in both
debug/release, the architecture/dependency audit and the app build. This adds
handler coverage but no new complete root or Game integration claim.

Authored invisibility/visibility now lowers to a named native appearance
command that updates both visibility and collision participation, as the source
does. Collision re-enabling, shadow toggles and maximum-draw-distance toggles
remain independent commands. Static fixtures cover complete handlers and the
draw-list invisibility gate; native tests preserve draw-admission observations,
contact latches, IFNOT, elapsed waits, RNG and all unrelated object state across
the immediate command sequence. Verification passes 39 lowerer tests, 161
static path checks, 150 native path tests in debug/release, the architecture/
dependency audit and the app build. The catalog is still nine roots/97 unique
statements, not a claim of complete scene or rendering integration.

Literal shape assignment and equality now decode aligned source headers into
semantic ShapeIds during extraction; raw shape pointers remain unavailable to
runtime arithmetic or variable copies. Every one of the 577 catalog headers
is checked for assignment/comparison lowering, invalid headers are rejected,
and native tests prove that changing shape leaves visibility, sprite state,
animation and other actor state intact. Equality consumes IFNOT normally, and
ordinary full-width literal word equality also lowers. Verification passes 41
lowerer tests, 162 static path checks, 151 native path tests in debug/release,
architecture/dependency checks and the app build. These are handler additions;
the complete native catalog remains nine roots/97 statements.

Lowering now covers literal byte/word ranges, all four variable equality/less
forms, and ten spatial predicates for selected/linked distance, ground, range,
yaw, height and selected planes. These reuse the reviewed native predicates:
wrapped subtraction signs and per-predicate IFNOT behavior are not replaced
with generic host-language comparisons. Shape/raw-pointer arithmetic remains
rejected. Verification passes 44 lowerer tests, 164 static path checks, 151
native path tests in release, and architecture/dependency checks. No additional
complete root is claimed; unported world services still reject whole graphs.

PATHHOLD now lowers to the existing movement-only terminal command without
inventing a continuation. Actor byte 27 is a separate typed script parameter:
source paths explicitly copy health or height's low byte into it, then use it
as saved state or a bit selector. It does not alias health, position, wait or
LOOP counters. Static authored-byte fixtures and exhaustive native copy tests
cover those contracts. Verification passes 46 lowerer tests, 165 static path
checks, 152 native path tests in debug/release, architecture/dependency checks
and the app build. Complete catalog coverage remains nine roots/97 statements.

The six facing forms now lower into the native dispatcher. World inputs keep
the selected actor, primary selection pointer and two fixed player actors
separate; linked forms read the owner's attachment live. Integration tests
change selection and target positions between immediate resumes and compare
whole actor state against the reviewed facing service. Missing required inputs
fault, while an absent attachment follows the source-defined no-turn advance.
Verification passes 47 lowerer tests and 154 native path tests in debug/release,
plus architecture/dependency checks and the app build. Existing static facing
fixtures remain in the 165-test static path suite. No new complete root or
scheduler integration is claimed.

The path-owned working word is now represented by one typed value with aliased
low/high-byte access. Source fixtures cover signed numeric constants, bit-set
testing and explicit clearing; native tests exhaust all 65,536 values and both
byte views while preserving the separate phase, parameter and loop count.
Verification passes 48 lowerer tests, 166 static path checks, 155 native path
tests in release, and architecture/dependency checks. Indexed imports and
world-state services remain unsupported; this field addition does not emulate
source addresses or add a complete root.

The shared inline primary-motion action is now a named Rust operation. Its
source call/return signature and complete helper are checked statically. The
primary auxiliary mode selects either primary velocity or retained displacement;
only horizontal owner velocity changes, with word wrapping. Tests cover every
mode byte, repeated live sampling, owner/primary aliasing and missing-input
faults, preserving vertical velocity, position, waits, IFNOT and RNG. Checks
pass 49 lowerer tests, 167 static path tests, 157 native path tests in both
debug/release, architecture/dependency checks and the app build. The native
catalog remains nine complete roots; no recorded gameplay was used.

The contact-class and hit-marker controls complete the 20-statement
`PRIMARY_MOTION_GROUND_LIMITED` path, including its shared inline helper and
new-contact callback. Class updates maintain both exclusion-group membership
and its side-attribution/non-damage aliases. Mutations to unreviewed class bit
02 still fail generation. Next-epoch contact suppression remains distinct from
current-epoch suppression and marker visibility. Exhaustive native masks and
whole-path tests cover all three exits: loop exhaustion, ground threshold, and
callback redirection. The native catalog now contains **10 complete roots and
117 unique statements**; the Game scheduler/spawn integration is still open.

Verification passes 51 lowerer tests, 169 static path tests, 160 native path
tests in debug/release, architecture/dependency checks and the app build. The
broader native library suite passes 516/517 tests. Its guidance-message cue
test fails with two extra HostileLaser cues; an isolated clean checkout of
pre-change commit `5a3eec4` reproduces the identical failure. That pre-existing
timeline test was not changed or weakened as part of this static path port.

Arithmetic chase now supports literal and variable byte/word targets plus the
waiting byte form. Differences wrap at the field width before signed minimum
step selection and division by eight. Waiting checks equality before updating:
the final nonzero step still yields, and the next invocation advances without
clearing elapsed wait state. Full handler signatures, all byte pairs, every
word difference at five wrap-boundary origins, self-aliasing and dispatcher
yield behavior are checked. Verification passes 52 lowerer tests, 172 static
path tests, 162 native path tests in debug/release, architecture/dependency
checks and the app build. Catalog coverage remains 10 roots/117 statements.

Selected world-position and world-rotation copies now lower to distinct named
commands. They sample the live selected object, including self-selection, and
copy only three coordinates or three angle bytes. Tests preserve neighboring
child-number/wait bytes, relative transforms, velocity, IFNOT and RNG; missing
selection faults before mutation. Complete source helper signatures are pinned.
Verification passes 53 lowerer tests, 173 static path tests, 163 native path
tests in debug/release, architecture/dependency checks and the app build.
This does not yet add another complete root or live scheduler integration.

Four positional cue operations now lower through live, typed fixed-marker
observations. Marker side comes from the selected actor's view-side flag, not
the path owner's retained player selection; marker bearing remains separate
from actor yaw. The banded forms preserve centered far cues and additive
stereo parameters (including middle-left 0x40), while range-limited forms
retain the signed wrapped comparison and omit distance attenuation. Missing
audio/marker inputs fault before mutation. Tests cover every cue ID/bearing,
all 65,536 range comparisons, fresh marker selection, coordinate wrapping,
queue order, suppression and whole-actor/IFNOT/RNG preservation. Full source
handler/helper signatures are pinned without executing recorded gameplay.
Verification passes 54 lowerer tests, 179 static path tests, 167 native path
tests in debug/release, architecture/dependency checks and the app build.
Catalog coverage remains 10 complete roots/117 statements; these operations
remove blockers from larger graphs but do not establish scheduler integration.

Linked-rotation refresh now lowers as a separate relationship command. It
reads the live attachment link regardless of coordinate-mode flags, stores
three wrapped angle differences in relative rotation, and preserves world
rotation and all neighboring state. An absent link advances unchanged; a
self-link produces zero differences; invalid native links fault before writes.
All 65,536 angle pairs and dispatcher link changes are covered. Verification
passes 55 lowerer tests, 180 static path tests, 169 native path tests in both
debug/release, architecture/dependency checks and the app build. The complete
catalog remains 10 roots/117 statements, without new recorded-gameplay input.

The one-byte manual-zero-frame command now lowers to the same shape-channel
initializer as `INITANIM 0`; its source write is pinned beside that initializer's
handler. This reuses the existing animation state and does not confuse manual
zero with automatic clock-driven animation. All 56 lowerer and 181 static path
tests pass, with unchanged generated complete-root coverage and native code.

The nine-command `PRIMARY_TARGET_FOLLOWER` root is now fully lowered, including
both reviewed inline calls. A shared, typed primary target-control record owns
configuration, origin and limits; these are not duplicated in actor path state.
Configuration clears the offset flag before its lock check, and matching-owner
origin refresh still runs when configuration is locked. The follower samples
the live primary pose and link mode on all eight iterations, using the source's
two signed-byte rotations for the optional 80-unit forward offset. Zero angles
do not bypass coefficient truncation. The cue and initial configuration occur
once; selection, velocity and world rotation remain independent of following.

Full helper signatures, every range word under lock/ownership combinations,
all pitch/yaw pairs, self-aliasing, missing-input faults and the complete eight-
iteration graph are checked statically/native-only. Verification passes 57
lowerer tests, 187 static path tests, 174 native path tests in debug/release,
architecture/dependency checks and the app build. The catalog now has **11
complete roots and 126 unique statements**. Game scheduler/spawn integration
and whole-game behavioral completion remain open; no new recordings were used.

Both alternate primary target-configuration forms are now lowered as well.
One doubles only the low byte of the authored range, retaining its original
high byte; the other changes the three axis rates without altering the range.
They share the reviewed lock/ownership behavior above. Every word value under
all lock/ownership combinations and dispatcher self-aliasing is checked.
Verification passes 58 lowerer tests, 188 static path tests, 175 native path
tests in debug/release, architecture/dependency checks and the app build.
Complete-root coverage remains 11 roots/126 statements.
