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

Ordinary byte operands now expose the existing packed shape/color animation
controls. Source-authored copies, increments, additions and zero writes use
those same controls, without forcing manual selection or publishing resolved
frames early. Both channels preserve every packed byte, including automatic
mode payloads; unreviewed overlapping word views remain rejected. Source
fixtures pin the operand mapping and seven actual authored uses. Whole-object
tests cover all byte values, arithmetic mode transitions, self-aliasing and
separate render publication. Verification passes 59 lowerer tests, 189 static
path tests, 176 native path tests in debug/release, architecture/dependency
checks and the app build. Complete-root coverage is unchanged.

The retained positional-loop control now lowers into the actor's existing
spatial-sound state. Its source consumer proves this is a nearest-source audio
control, despite the older staging host's misleading `set_trail` name. All
256 authored bytes are preserved; zero disables the candidate without queuing
a one-shot cue. The existing capital-engine mapping remains canonical. Other
controls reach the native selector but explicitly report unsupported PCM in
the app, stopping any previous loop instead of playing a substitute. No new
PCM assets or recordings were produced.

Verification passes 60 lowerer tests, 190 static path tests, 177 native path
tests in debug/release, a separate all-control selector test in both builds,
15 audio-adapter tests per app target, architecture/dependency checks and the
app build. Complete-root coverage remains 11 roots/126 statements; positional
PCM-bank completion and scheduler integration remain separate open work.

Byte-indexed constant-ROM lookups now lower to typed byte operands backed by
decoded 256-entry arrays. Selection reads the live actor byte before any
overlapping destination write, preserving packed animation controls and word
byte views. Decoding retains the entire byte-index domain, including adjacent
instruction bytes when the source reads them as data; it never executes them.
Mutable windows, unresolved bank mappings, low-word wrap out of ROM and
truncated tables still reject the graph. Tests cover all indices, live
self-aliasing, literal/source-data edits and those rejection boundaries.
Verification passes 62 lowerer tests, 191 static path tests, 178 native path
tests in debug/release, architecture/dependency checks and the app build.
Catalog coverage remains 11 complete roots/126 statements.

Word-table lookups now use the same offline constant-data boundary. The source
widens its byte index before doubling, so all 256 little-endian entries remain
distinct. Native tests exercise low/high-byte selectors overlapping the word
destination over repeated live reads. Shape-pointer destinations and tables
crossing the reviewed ROM window remain hard errors. Verification passes 63
lowerer tests, 192 static path tests, 179 native path tests in debug/release,
architecture/dependency checks and the app build; complete-root coverage is
unchanged.

The complete eight-command `SHARED_COUNTDOWN_SERVICE` root is now lowered.
A single world-owned countdown is borrowed by its path operations; absolute
and indexed imports/exports refer to that same typed record. Only the reviewed
countdown operand is accepted, not a generic shared-memory interface. The
service makes itself invisible, masks its contact class, runs while paused,
copies the current countdown to its actor-local part byte and decrements only
when nonzero. Ordinary increment/decrement operations still wrap at byte
width; the service's branch, not the data type, provides its zero guard.

All initial byte values, live resets after zero, whole-actor preservation,
missing shared inputs, IFNOT/RNG preservation and unreviewed-neighbor rejection
are checked. Verification passes 64 lowerer tests, 196 static path tests, 182
native path tests in debug/release, architecture/dependency checks and the app
build. The native catalog now contains **12 complete roots and 134 unique
statements**. Whole-game scheduler integration remains open.

The two far-sort controls now toggle a named actor flag and publish the
source's fixed 15,000-unit bias through the existing native render boundary.
World pose, draw-distance selection, contact state, IFNOT and RNG are
unchanged. Static fixtures pin both commands and the draw-record consumer;
native checks cover every word-width bias, whole-actor preservation and
repeated on/off publication. This establishes the flag-to-render-output
contract, not full renderer parity or another complete authored root.
Verification passes 65 lowerer tests, 197 static path tests, 183 native path
tests plus the render-boundary test in debug/release, architecture/dependency
checks and the app build. Complete-root coverage remains 12 roots/134 statements.

The ten-command `COUNTER_MOTION_EFFECT` root now imports the published player
motion snapshot, negates its horizontal components and follows the selected
player's yaw with fixed pitch/roll. Its initial collision and far-sort setup
runs only once. The player-service snapshot preserves position and chooses
velocity versus retained displacement by auxiliary mode; path reads do not
recompute it from a newer player state. Only the three reviewed motion words
are importable, with unreviewed or misaligned shared operands rejected.

Tests cover every mode byte, every imported word on each axis, missing-snapshot
failures, live snapshots and yaw over repeated yields, self-selection, actor
preservation and IFNOT/RNG retention. Verification passes 66 lowerer tests,
199 static path tests, 186 native path tests in debug/release, the architecture
and dependency audits, and the app build. The catalog now has **13 complete
roots and 144 unique statements**; scheduler integration remains open.

The complete 17-command `PLAYER_CHARGE_ORB` root now retains its two-visit
wait, eight-pass size loop, live active-pilot threshold comparison, ready
shape transition and persistent callback after HOLD. The callback samples
the selected player's complete charge-level byte and linked-mode observation;
it updates the relative reference and retained offsets without changing the
child-list attachment or world pose. The threshold is a separate shared input,
not inferred from the selected player or frozen to the initial pilot.

Static fixtures pin the complete path, callback, threshold table and active
pilot swap. Tests cover all offset words and charge bytes, preserved high-byte
thresholds, missing inputs, self-selection, size wrapping, live threshold
changes, post-HOLD callbacks and IFNOT/RNG preservation. Verification passes
67 lowerer tests, 202 static path tests, 189 native path tests in debug/release,
the architecture/dependency audits and the app build. The catalog now contains
**14 complete roots and 161 unique statements**. This remains static native
translation with scheduler integration open, not a claim of whole-game parity.

Three relative-frame controls are now statically translated. Capturing the
selected frame uses the source's negated-angle view matrix and separately
truncated products, retains world pose/velocity and enables relative movement.
Clearing the reference and selecting a zeroed self-relative frame do not change
either movement gate, the selected-player identity or child-list attachment.
Tests cover every pitch/yaw pair, wrapped coordinates and angle differences,
self-selection, missing/dangling selection and whole-object preservation.
Verification passes 68 lowerer tests, 205 static path tests, 192 native path
tests in debug/release, architecture/dependency audits and the app build.
Complete-root coverage remains 14 roots/161 statements.

Indexed byte and signed-byte-to-word additions now preserve their compound
source ordering: sample the lookup index, add at destination width, then
increment and bound the selector's live post-write byte. This matters when
the selector overlaps either destination byte. Period zero retains natural
byte wrap. Only fully reviewed immutable lookup windows are accepted; a
later short wrap period is not evidence that the first index is bounded.
Exhaustive period/index tests, whole-object alias checks and immediate-dispatch
checks pass alongside 69 lowerer tests, 206 static path tests, 194 native path
tests in debug/release, architecture/dependency audits and the app build.
This adds command coverage; complete-root coverage is unchanged.

The three selected auxiliary mode-class branches now use the live mode's high
nibble. They ignore its low nibble and the separate action flags, and preserve
a pending IFNOT instead of treating it as an ordinary equality predicate.
All mode/action-byte combinations, missing observations, cursor selection and
actor/RNG preservation are tested. Verification passes 70 lowerer tests, 207
static path tests, 195 native path tests in debug/release, architecture and
dependency audits, and the app build. Complete-root coverage remains 14/161.

Two additional source aliases reuse existing verified statements: the direct
phase-high-byte setter is byte-equal in lowering to an ordinary typed write,
and the source-defined empty subroutine advances immediately without a yield.
The latter is explicitly verified, never a fallback for unported handlers.
All 256 setter literals are checked. The lowerer/static suites now pass
71/208 tests plus both architecture/dependency audits; native code and the
14-root/161-statement catalog are unchanged from the preceding verification.

Linked/mother signaling and numbered-child signaling now set only the shared
pending hit-event latch. Numbered lookup retains the source owner-flag versus
mother choice and first matching full-byte identifier; relative references and
attached-coordinate mode are independent. The child-missing branch requires a
parent: absent parent falls through, while an empty valid search branches.
Neither operation consumes IFNOT. Tests cover all child numbers, duplicates,
self-links, absent/dangling links, cycles, whole-store preservation and one-shot
consumption through the actual hit-event branch. Verification passes 72 lowerer
tests, 211 static path tests, 200 native path tests in debug/release, both
architecture/dependency audits and the app build. Complete-root coverage remains
14/161, with Game scheduling and spawn integration still open.

The primary-target update now statically ports the full selection/publication
service and its marker projection. A typed fine-angle view anchor is distinct
from ordinary actor angles and selected-player pose. The port retains locked
selection, forced ownership, strict unsigned nearest-distance comparison,
signed distance storage, display-side effects on rejected candidates, and the
source's mixed-width auxiliary distance. Screen projection preserves both
authored curves, yaw-dependent edge limits, the yaw-fraction coupling in
vertical interpolation and individually rounded signed shifts.

Tests cover every fine angle across clip boundaries, every control-flag byte,
all coordinate words, negative distances, repeated live selection, self-primary
identity and input failures before mutation. Static fixtures pin the complete
service, geometry helper, distance helper and immutable curve data. Verification
passes 73 lowerer tests, 217 static path tests, the seven reused angle/source
checks, 205 native path tests in debug/release, both architecture/dependency
audits and the app build. The separate linked-field variant remains rejected;
complete-root coverage remains 14/161 and Game integration is still open.

The complete node-gated target-service root now lowers to native Rust. Its
indexed import reads the live active campaign-node flag word, retaining both
bytes despite the source node loader's low-byte-only initialization. The loop
resamples that word and the actor's health-byte bit selector on each visit,
preserves pending IFNOT, yields after target selection, and exits without
requiring target inputs when the selected bit is set. Visibility/collision
setup runs only once, outside the backedge. All 256 selector values retain the
source's wrapped table indexing, including instruction-overlap data entries.

Verification passes 75 lowerer tests, 219 static path tests, 208 native path
tests in debug/release, architecture/dependency audits and the app build.
Tests include every imported word, live flag/health changes, the complete
target-service loop, full actor preservation and missing-input errors.
Catalog coverage is now **15 complete roots and 167 unique statements**.
This is source-only verification; scheduler/spawn integration remains open.

The selected auxiliary action-bit-clear branch is also lowered. It samples
only action bit 2, ignores the auxiliary mode and preserves pending IFNOT.
All mode/action-byte combinations, live dispatcher resampling, missing-input
atomicity and whole-store/RNG preservation are checked. Verification passes
76 lowerer tests, 220 static path tests, 209 native path tests in debug/release,
both architecture/dependency audits and the app build. Catalog coverage stays
15 roots/167 statements; this adds command coverage, not an additional root.

Variable-word swapping now saves both original operands, writes the second
field first and then the first, and advances immediately. Only reviewed typed
word fields are accepted; unmapped and partial-word aliases still fail lowering.
Tests cover all mapped field pairs, same-field swaps, every word value, live
dispatcher inputs and preservation of IFNOT, wait state, actors and RNG.
Verification passes 77 lowerer tests, 221 static path tests, 211 native path
tests in debug/release, both audits and the app build. Root coverage stays 15/167.

Random conditional goto now consumes exactly one shared byte and branches on
values below 127, without yielding or consuming IFNOT. Coincident successors
still consume the draw. The entire random helper is source-pinned, with an
independent widened-arithmetic check of its borrow chain across 2,359,296 seed
combinations. Branch tests cover every possible byte outcome, both inversion
states and whole-actor preservation. Verification passes 78 lowerer tests,
223 static path tests, 214 native path tests in debug/release, both audits and
the app build. Root coverage remains 15/167; no gameplay recording was used.

Six yaw-orbit/radius commands now reach the existing source-derived geometry
kernels through static lowering and the typed dispatcher. Literal angles keep
their full byte; the variable-angle form samples its live byte before changing
an overlapping position. Radius literals are sign-extended, and selected,
linked and local origins remain distinct. Tests cover every angle/amount byte,
self-centering, local versus world publication, repeated live target changes,
and missing/dangling centers before mutation. Verification passes 79 lowerer
tests, 224 static path tests, 217 native path tests in debug/release, both audits
and the app build. No additional complete graph is unlocked: coverage remains
15 roots/167 statements, with game integration still open.

Selected-player auxiliary mode/action state is now one borrowed mutable record,
shared by command updates and subsequent conditions. Three source handlers set
the mode low nibble to one/four or clear action bit zero, preserving all other
bits. The similarly named flag-OR command writes a different auxiliary field
and remains unsupported. Exhaustive byte-pair tests, repeated live dispatch,
missing-input atomicity and actor/IFNOT/wait/RNG preservation pass. Verification
passes 80 lowerer tests, 226 static path tests, 219 native path tests in both
debug/release, architecture/dependency audits and the app build. Catalog coverage
remains 15 complete roots/167 statements; this does not establish game integration.

Literal and variable radio-message requests now call the full source-derived
request-entry service through one shared typed record. Requests replace pending
message identity, wrap authored number zero to catalog index 255, reset the full
panel coordinate, and choose placement from live compact-layout and smoothed
screen-marker inputs. The source's signed byte comparison includes screen Y
0..17 in upper placement; it is not an unsigned threshold test. This ports the
request, not the separate text/portrait presentation service. Verification
passes 81 lowerer tests, 229 static path tests, 222 native path tests in both
debug/release, both audits and the app build. No gameplay recording or execution
of source-machine instructions was used; root coverage remains 15/167.

The encounter-announcement path at `$44:7BC5` is fully lowered, adding live
read-only campaign difficulty and encounter-variant inputs. Campaign setup and
the map's later variant-four override are pinned from source. The path preserves
the initial ten-invocation delay, all difficulty-specific radio branches, and
the variant-four messages at visits 10/41/72 before ending at visit 102. Tests
cover all 256 variants, all three difficulties, late variant resampling, full
byte-width imports, missing inputs and unchanged RNG. Verification passes
83 lowerer tests, 230 static path tests, 225 native path tests in debug/release,
both audits and the app build. Catalog coverage is now **16 complete roots and
195 unique statements**. Game scheduler/spawn and radio presentation integration
remain open; these are source-derived path tests, not recorded gameplay.

The first-time control-guidance path at `$44:04B5` is now fully lowered.
Its shared history is a borrowed, full-width typed word, and its layout input
uses the existing Type A/Type B enum. Normal difficulty waits 16 invocations,
sets the selected auxiliary mode, and checks/sets history bit `0x0100` before
the 30-invocation initial message delay. Hard/Expert exit immediately; already
shown guidance exits after the mode update. The five requests occur at visits
46/112/178/244/310 and the path ends at 375. Type A requests 207 in the fourth
slot; Type B substitutes 213. The shared countdown resets to 80 per request,
but this path does not tick that countdown itself.

Verification covers every history-word value in both transfer directions,
missing-input atomicity, preserved byte widths/IFNOT/RNG, all difficulty/layout
and history branches, and inputs supplied only at the statements needing them.
The source program, full word-transfer helpers, button-layout provenance and
zero-branch handler are pinned statically. All 85 lowerer tests, 233 static path
tests, and 228 native path tests in debug/release pass, as do both audits,
generated-catalog freshness and the app build. Coverage is **17 complete roots,
222 unique statements**; this does not establish Game integration or full
gameplay parity. No recorded gameplay or source-machine execution was used.

Independent authored spawning (`$7F:91A3`, path opcode `$5D`) now lowers to
its own typed spawn service. It copies the caller's live world position,
rotation, selected player and allocation group, applies literal health/attack
bytes, and inserts after the caller without attachment, relative pose, or
velocity inheritance. The new actor starts only when separately scheduled.
Unlike attached-child spawning, a full pool skips the allocation, preserves
the previous last-spawn selection, and immediately continues the caller.
Missing native inputs or catalog entries remain explicit errors. Offline shape
classification still admits only the reviewed transient-effect family.

Verification passes 86 lowerer tests, 234 static path tests and 233 native path
tests in debug/release, both audits and the app build. The complete spawn
handler and pose-copy callees are pinned from static source. The root rescan
finds no newly complete graphs, so coverage remains **17 roots / 222 unique
statements**, with scheduler/spawn integration still open.

The selected-player-gated occupancy branch (`$7F:B73E`) now calls the existing
native world-occupancy service using the owner's live X/Z position. Its selected
player exemption bypasses the map lookup, including the need for a map input.
Both branch edges preserve IFNOT and the wait timer. Missing exemption/map
inputs fault before mutation, rather than assuming an empty world. Static
contracts pin the complete handler and both direct continuations; tests cover
negative coordinates, cell edges, height independence and live map changes.
Verification passes 87 lowerer tests, 235 static path tests, 235 native path
tests in debug/release and all six world-occupancy tests, plus both audits and
the app build. Root coverage remains 17/222 and full Game integration is open.

The surface-height path branch (`$7F:BF86`) now queries the existing native
geometry service through live active objects, preserving source list order,
first-visit/footprint exemptions, fixed-versus-automatic collision animation,
and signed wrapping comparisons. It updates the actor's supporting-object
link and contact flags on both outcomes, clears stale contacts on a miss,
and restores the old contact-group byte. Both edges preserve IFNOT and the
wait timer. Missing world mode or unknown eligible shapes fail explicitly.
The adapter exposes height and actor contacts; response-specific normals and
footprint outputs remain outside this path observation.

The source collision-mode byte is a typed shared input: imports retain all
eight bits, while the surface search tests only the low three. With that
input, `$44:EEED` is fully lowered, including its shape-conditional fixed-player
facing, new-contact callback, ground-plus-surface versus surface-only callback,
and ten-visit loop. Tests cover live search-mode changes without reselecting
the installed callback, missing inputs bypassed by the ground short circuit,
all termination routes, full-byte imports, and unchanged RNG/group state.
Static contracts pin the handlers, mode writers, and entire root program.

Verification passes 89 lowerer tests, 238 static path tests, 239 native path
tests and nine surface tests in debug/release, both audits, catalog freshness
and the app build. Coverage is **18 complete roots / 246 unique statements**;
Game scheduler/spawn integration and complete collision response remain open.
No recorded gameplay or source-machine execution was used.

The primary-motion surface-limited path (`$44:EE10`) is also fully lowered.
It sets health 10, attack 4, speed 80 and a forty-visit loop, calls the existing
primary horizontal-motion inheritance subroutine exactly once, and configures
shape 31, sprite color/size 0/5, spatial-loop control 12 and positioned cue 115.
Its remaining callbacks share the surface/ground/contact graph above. Tests
exercise both velocity- and displacement-based inheritance, unchanged world
position at scheduler boundaries, secondary marker routing, a single sound
request, and no repeated initialization when the primary later changes.
All 90 lowerer tests, 239 static path tests and 240 native path tests in
debug/release pass, with both audits, catalog freshness and the app build.
Coverage is **19 complete roots / 256 unique statements**. This remains a
source-derived static port milestone, not a claim of runtime Game integration.

The dedicated world-X/Y/Z and pitch/yaw/roll add commands, plus signed-byte
addition to an actor word, now lower to the existing typed mutations. Rotation
wraps as a byte; world displacement sign-extends the literal before wrapping
word addition. Complete handler signatures and every literal-byte lowering are
checked, with exhaustive rotation-byte pairs and coordinate edge cases in Rust.
Verification passes 91 lowerer tests, 240 static path tests and 241 native path
tests in debug/release, both audits, catalog freshness and the app build.
These commands alone do not complete another root; coverage remains 19/256.

The occupancy/surface-limited projectile path (`$44:EC98`) is fully lowered,
including its independent sprite path at `$44:F5A1`. Native metadata admits
shape 19 as an effect only with that reviewed path; other uses of the same
shape remain rejected instead of being broadly classified as effects. The
effect has its own three-visit color loop and is not pool-pressure-reclaimable.

Whole-path tests cover all difficulties (attack 2/4/6), both auxiliary pitch
branches, spawn-before-pitch ordering, initial health/shape/quadrupled velocity,
one positioned sound, the first three collision-disabled visits, collision
reenabling at visit 3, and expiry at visit 53. They independently test occupancy,
ground, surface and new-contact exits, with later inputs omitted when an
earlier branch short-circuits them. Parent and effect are scheduled separately;
their world positions are not advanced by the path dispatcher.

Verification passes 92 lowerer tests, 241 static path tests and 242 native path
tests in debug/release, both audits, catalog freshness and the app build.
Coverage is **20 complete roots / 295 unique statements**. Game integration
remains open; no recorded gameplay or source-machine execution was used.

The three shared homing-projectile roots (`$44:EE2D`, `$44:EE3B`, `$44:EE4C`)
are now fully lowered. The first two preserve their distinct speed/doubling
prefixes and primary horizontal-motion inheritance; the third selects attack
2/3/4 by difficulty and rejects targets at or beyond 12,000 horizontal units.
The shared setup enables per-step velocity generation, creates its independent
sprite, and selects the lifetime branch using the whole mode byte. Subsequent
speed assignments retain the inherited velocity until the scheduler's movement
entry regenerates it; path dispatch does not perform that movement itself.

Zero mode performs three radius contractions per chase yield until the target
is within 1,000 units, then enters a forty-iteration loop with two-tick smooth
steering and player-crossing callbacks. A crossing cancels those two callbacks
and waits fifteen visits, leaving occupancy/counter and new-contact callbacks
active. The retained word counter is incremented, not initialized, and ends
the path only on equality with 60; occupancy short-circuits the increment.
Nonzero mode retains both authored new-contact registrations and enters the
shared ground/surface loop for 33 or 45 iterations, depending on its entry.

Full-path Rust tests cover every entry and difficulty, mode bytes 0/1/8,
normal expiry, contact, occupancy, counter wrap/threshold, crossing cancellation
and ground exit; additional tests cover the range boundaries, intermediate
inherited velocity, chase yields, independent spawn ordering and one sound.
Verification passes 93 lowerer tests, 242 static path tests and 245 native path
tests in debug/release, both audits, catalog freshness and the app build.
Coverage is **23 complete roots / 363 unique statements**. This is static
source-derived catalog coverage, not full Game integration or runtime parity.

The variant-guided projectile (`$44:EF2D`) and its attached sprite (`$44:F306`)
are now completely lowered. The parent rejects targets at 10,000 units,
selects shapes 110/109/111 by its authored parameter, gives variant 1 ten
rise/roll steps, and chooses either speed 45 or acceleration toward 20. Its
child offsets are -60/-480/-120; the child's initial health determines sprite
size before it resets health to 100 and runs its color loop. Spawn classification
admits shape 19 only for this and the previously reviewed independent effect.

The always callback reenables collision before its selected-yaw and projected
forward-plane tests. It can disable collision again, end through the shared
tail, mark a surface/ground hit, or wait thirty visits after entering the
variant-specific near range. The tail ends when distant or clears health and
holds when within 1,000 units. Tests cover all variants plus the default-byte
case, mode bytes 0/1/8, all lifetime exits, the ten-step rise, all 256 selected
yaws, independent child scheduling, and the child's own zero-health ending.
The direct cue is routed through the listener, without requiring marker data.

The source hit-marker command updates the existing contact marker independently
of hit suppression. The child inline action retains a typed death-effect
suppression flag, with its source consumer pinned; the general death-effect
pipeline remains unported, so this does not claim its production consumption.
Verification passes 94 lowerer tests, 243 static path tests and 248 native path
tests in debug/release, both audits, catalog freshness and the app build.
Coverage is **24 complete roots / 444 unique statements**. No recorded gameplay
or source-machine execution was used; scheduler/Game integration remains open.

Selected-player offset aiming (`$7F:C1B8..C2B2`) is ported as a typed operation.
The offline lowerer combines three contiguous literal offset preparations and
their immediate consumer, after verifying each handler, complete record and
control edge. Incomplete sequences, reordered writes, intervening statements,
and external entries into the middle are rejected. Bank-wrapped sequences are
handled without retaining native scratch fields or temporary source objects.

The operation byte-rotates X/Z by the selected player's yaw, preserves signed
Y, scales by sixteen, wraps world coordinates, and steers pitch/yaw by eighth
steps with a minimum nonzero step. It clears the shared steering result but
does not alter relative rotation, velocity, wait count or pending IFNOT.
Tests exercise all offset bytes and yaw values, wrapped arithmetic, self-target
aliasing, missing-input atomicity, and complete source-handler signatures.
The audit now distinguishes source-command counts from folded Rust statements.
Verification passes 96 lowerer tests, 244 static path tests and 252 native path
tests in debug/release, both audits, catalog freshness and the app build.
No additional root is included in this primitive checkpoint; coverage remains
24 roots, 444 source commands and 444 native statements.

### Offset-guided projectile: complete static graph

The `$44:ECF7` root now lowers all 93 reachable source commands to 87 typed
statements, including both offset-argument folds. Whole-graph source-byte pins
cover setup, both chase loops, the held path, timed steering, the nonzero-mode
waits, all callbacks, cleanup and the relative-pitch table. Duplicate per-step
velocity-generation commands are deliberately retained.

Native tests cover every random seed byte at nine exact distance-band edges,
difficulty-scaled attack power, random draw order, persistent relative angles,
two offset aims in the chase-transition visit, the 13-run callback and its
expiration, and the 110-count exit including word wraparound. The nonzero-mode
branch runs its callback during the initial three collision-disabled visits;
it needs no selected-object input and subsequently waits fifty visits. Contact,
counter, occupancy, ground and surface exits are exercised in both applicable
mode branches, with live surface-mode inputs. Source cleanup cancels the
zero-mode callback even when the nonzero-mode callback caused the exit; tests
preserve that asymmetry. Path execution does not advance movement by itself.

Verification passes 97 lowerer tests, 245 static path tests and 254 native path
tests in debug/release, both audits, catalog freshness and the app build.
Coverage is **25 complete roots / 534 source commands / 528 native statements**.
This is static lowering and native path execution, not yet general Game
scheduler/spawn integration, whole-game completeness or recorded-gameplay proof.

### Typed path-variable saves and restores

The four source handlers `$7F:A752..A7CD` now lower byte/word variable
saves/restores to decoded actor fields. Values share the actor's existing
call/loop stack and resource pool: one saved value consumes one four-unit
entry, every eighth entry reallocates, and an emptied stack keeps its storage
until actor cleanup. Pair discard may discard saved values as well as calls
and loops. Callback-root return does not consume a suspended main-path save.

This contract preserves all word bits and accepts a byte restore from the low
byte of a saved word. It deliberately rejects word restore from only a byte
save, or numeric reinterpretation of a typed continuation: those cases would
require unreviewed stale source temporaries or source addresses. Such errors
do not silently widen values, consume entries or alter the destination field.
No additional complete source root is claimed by this primitive checkpoint.

Tests cover all 65,536 word values, all byte values, interleaved calls/loops/
saves, pair discard, allocation-pressure failure, and a three-iteration path
whose saves span movement and nested callback calls. Budget-zero, missing-path
and incompatible-restore checks preserve state; wait/repeat and pending IFNOT
remain untouched. Verification passes 98 lowerer tests, 246 static path tests,
256 native path tests and 10 program-stack tests in debug/release, both audits,
catalog freshness and the app build. Coverage remains 25 complete roots,
534 source commands and 528 lowered statements.

### Linked protection effect: source gates and complete lifecycle

The `$44:F2B9` root now lowers its complete 23-command graph. A typed
`DeflectionProtection` preserves the player's full protection control while
exposing the five-bit count and projectile-deflection flag. The effect spins
its relative pitch/roll, then reads its **attachment owner**, not the current
selected actor. Special-character/full-surface-mode blocking clears only the
effect's low phase byte and bypasses linked-player access. An explicit minimum
override, **or disabled contacts**, replaces a non-minimal protection byte with
one. Otherwise zero remaining protection clears bit 20; that bit's producer is
not established here and is described only as a clear-on-expiry latch.

The shared linked-effect activity byte can suppress startup animation/audio;
the active loop clears it on every visit. Normal startup performs nine
animation advances over eight yielded visits. Counts zero/one follow the
authored 256-entry flicker lookup and increment the phase high byte, including
wraparound; higher counts return without advancing it. The main loop observes
zero and ends on its next visit, after resetting the shape frame to nine.

Tests cover all protection bytes and gate combinations, all initial spin
bytes, missing/mismatched linked inputs, selected-versus-attached identity,
startup activity values 0/1/2/255, live minimum/blocked/expiry transitions,
flicker-table boundaries and byte wrap. The separately ported countdown
subsection (`$06:9F0D..9F20`) tests every control byte against every shared-clock
phase; it decrements only nonzero low-five-bit counts on eighth updates. Its
surrounding player-update admission gates remain the caller's responsibility.

Verification passes 99 lowerer tests, 250 static path tests and 263 native path
tests in debug/release, both audits, catalog freshness and the app build.
Coverage is **26 complete roots / 557 source commands / 551 native statements**.
The general Game scheduler, live protection-state ownership/producer wiring,
and automatic source protection-effect spawning remain integration work.

### Triggered attached projectile: complete parent, child and callbacks

`$44:F48B` now lowers its full 47-command graph to 44 typed statements. The
parent creates the shape-seven child at relative Y -10/pitch 231, excludes
the primary player from pair contacts, and follows the separately selected
player while aiming toward a 127-unit forward offset. Its callback continues
through both the twelve-pass trigger polling loop and the final twenty-update
wait. An early trigger discards the outstanding loop entry before waiting.

The child retains its primary collision-exclusion link independently of its
attachment parent. Shared activation, a new contact, occupied terrain or the
surface condition enters the same transition: cancel the contact callback,
publish shared activation, initialize primary pitch recoil only when zero,
replace and lock the primary target configuration, emit the second cue and
stop. Its remaining loop continuously refreshes the target origin; it does
not contain an END. The always callback survives, adding four to relative
pitch and adding vertical velocity before ordinary attached integration adds
that velocity again. End-of-parent handling is not automatic child retirement.

The typed primary-target transition ports `$07:B67D..B6EE`, including temporary
unlock, ownership replacement even when previously locked, zero range, rates
3/3/2, limits 25/25/31, a ten-update transition delay and final relock. The
independent pitch-recoil leaf `$07:9AAB..9AEE` wraps its word negation, then
damps toward zero by sixteen. Exhaustive word tests cover recoil initialization
and damping; byte tests cover shared activation without boolean narrowing.
Missing live primary identity or required state faults before statement writes.

Whole-graph tests cover every trigger route, absent activation, early and late
parent triggers, selected-versus-primary identity, retained recoil, initial
and subsequent target ownership, both movement/callback phases, cue order and
unchanged random state. Static signatures pin the entire graph, inline call,
target helper, recoil helper, source installer and shared-trigger producers.

Verification passes 102 lowerer tests, 255 static path tests and 268 native path
tests in debug and release, plus generated freshness, architecture/audit checks
and the application build. Coverage is **27 complete roots / 604 source
commands / 595 native statements**. General Game scheduling, automatic weapon
spawning/retirement, live player-state ownership, target-transition countdown
consumption and player-service recoil scheduling remain integration work.

The broader library run passes 632 of 633 tests. The unrelated
`reengagement_guidance_message_matches_the_retail_timeline_and_cues` assertion
also fails in an isolated clean checkout of prior commit `5ebf050`: the cue
batch contains two `HostileLaser` events before `RadioMessageClose`, while the
test expects only the close cue. This baseline failure is not counted as a
passing whole-library verification and is unchanged by this path port.
No recorded gameplay, CPU execution or graphics-coprocessor execution was used.

### Attached-effect motion and depth primitives

The source inline helpers `$06:FAAE..FB81` now have typed Rust actions and
reviewed lowering for `$44:F3F0`, `$44:F45B` and `$44:F46E`. Settle chases
relative Z toward 200 twice and relative Y toward zero twice. Center chases
relative X toward the live script word once, Z toward zero twice and Y toward
zero once. Each chase retains its own wrapped difference and toward-zero
rounding; two calls are not combined into a single larger step. Tumble wraps
relative pitch by -8, relative Y/Z by +10/-10, and yaw/roll by phase low byte.
These helpers do not update world pose, velocity, player selection or randomness.

Depth operands now share the genuine sixteen-bit authored field: word 87 and
bytes 87/88 alias one value, independently of the adjacent animation controls.
Sprite colour overwrites only the low byte, and render publication truncates
to that byte. Exhaustive word/byte tests and the Game render-boundary test
cover retained high bytes, signed wrapping and unrelated-state preservation.
Dispatcher tests cover immediate continuation, zero-budget atomicity and
preservation of wait/repeat/IFNOT/random state. Static tests pin all three
helper bodies, the chase leaf and returns, operand mapping, and byte-only
sprite/render consumers; every bit of each inline signature is mutation-tested.

Verification: 104 lowerer tests, 260 static path tests, 273 native path tests
in debug and release, render-boundary test, catalog freshness, architecture
check and application build pass. This is a primitive checkpoint, not another
complete effect root: coverage remains **27 complete roots / 604 source
commands / 595 native statements**. The parent effect's shared action gate,
recovery requests, altitude input and shape metadata still require review.

### Complete attached recovery effect

`$44:F3AD` now lowers its full 66-command parent/child/callback graph. The
parent emits cue 55, spawns shapes 112/113 as collision-disabled effects and
zeros its world pitch/roll for 45 visits. Children settle for five visits,
center during a 25-visit callback, publish three replace-not-add shield
requests of 40, alternate authored depth using the shared clock, and wait
for independently authored three/eight-visit delays. The 35-visit action-gate
callback preserves phase low byte on its stack and redirects to END after
the callback batch; requests first arriving after expiry do not redirect.

The terminal branch tests the whole surface-mode byte and a wrapped signed
world-Y difference against the environmental plane. The self-frame route
keeps the existing attachment/child relationship, zeros relative pose, chases
world roll toward saved relative X and moves world Y by -4 for ten visits.
The alternate route draws one shared random byte, tumbles for at most ten
visits, and can finish early at the surface. Both normal routes clear health
and switch to PATHHOLD; they do not emit END or directly retire the actor.

`ShieldRecoveryRequest` ports the `$06:9F36..9F53` consumer: clear the request,
wrap the reserve-shield byte addition, then clamp unsigned to capacity. A
nonzero request asks the caller for recovery feedback even when addition
wraps to zero. All 16,777,216 amount/current/capacity combinations are tested.
Missing live requests, action-gate input or environmental height fault before
statement mutation. Full-width import tests, every clock/mask/IFNOT combination,
and complete-path tests cover both children, early/expired gates, three
request visits, callback lifetimes, both exits, surface interception, signed
comparison wrap, retained attachment identity, depth flicker and random order.
The whole-graph tests explicitly exercise path/callback service visits rather
than implicitly simulating the surrounding world or player scheduler.

Verification passes 106 lowerer tests, 265 static path tests and 277 native
path tests in debug/release, plus exhaustive shield recovery, catalog freshness,
architecture/static audit and application build. Coverage is now **28 complete
roots / 670 source commands / 661 native statements**. The full library passes
642/643 tests with only the previously reproduced radio-cue baseline failure.
General Game scheduling, automatic source spawning and the live player-service
recovery/feedback call remain integration work. No gameplay recordings or
original-code execution were used.

### Radar marker metadata and projection

The inherited `TRAIL` opcode actually stores SF2's radar-marker byte (1CEE).
It now lowers to a typed `RadarMarker` appearance command without changing
texture coordinates, sprite depth/colour, movement or particle state. Static
tests tie the writer to the radar draw loop and the marker-134 zoom search;
the compatibility host's unrelated 1CCC interpretation is not reused.

Native radar leaves port `$7F:51EE..5265` appearance and `$7F:52A3..5329`
projection: disabled markers, direct and altitude-coloured glyphs, the special
focus-count markers, signed wrapped altitude thresholds, arithmetic-floor
halving, wrapping doubling, clipping against unscaled offsets, independent
screen-coordinate bytes and the authored tile/attribute add. Minimum signed
scale, large shift counts and wrapped clipping comparisons remain defined.
The decoded appearance, view and plot records are public typed APIs; this
does not claim HUD publication, focus scheduling or automatic marker setup.

Verification passes 107 lowerer tests, 269 static path tests, 278 native path
tests and four radar tests in debug/release, plus freshness, architecture,
static audit and application build. Radar tests cover every marker/altitude
combination, all words at representative/extreme shifts, clipping boundaries,
screen wrapping, statement atomicity and unrelated-state preservation.
Complete-root coverage remains **28 roots / 670 source commands / 661 native
statements**; the newly supported radar opcode is a primitive checkpoint.

### Strategy suspension after the current movement visit

The inherited `SetFlag26Bit40AndHold` name does not describe PATHHOLD.
Its complete `$7F:BC80..BC88` handler sets the suspension flag and enters the
ordinary movement routine. Native `SuspendAndMove` preserves the assigned
behavior, hold latch, wait counter and logical terminal cursor. The current
movement, callback batch, relative integration and contact-latch cleanup still
run. Both portions of the shared strategy schedule now read the authored
actor flag directly and skip subsequent visits without retirement or hiding.
Host-owned suspension remains an additional, independent gate.

Verification passes 108 lowerer tests, 270 static path tests, 279 native path
tests and seven scheduler tests in debug/release, plus freshness, architecture,
static audit and application build. Tests cover zero-budget atomicity,
callback-terminal rejection, both initial hold-latch values, ordinary/relative
movement, callback completion, every scheduler split and live child traversal
after suspension. Complete-root coverage remains **28 roots / 670 source
commands / 661 native statements**. Production Game scheduler integration
remains open; this checkpoint does not claim runtime gameplay completion.

### Proximity-warning candidate controls and scan

Source 26 bit 20 now has its consumer-derived meaning: proximity-warning
membership, not collision participation or automatic steering. Both path
controls lower to this typed flag; neither resets the distinct repeat-warning
latch. The previously uncatalogued clear opcode is checked against its full
handler before being admitted to lowering.

The native `$06:A647..A84D` service ports entry guards, live actor order,
wrapped absolute height, source approximate player range, fixed-primary-view
projection, forward clipping, rearming and the best-two scan. Ranking retains
the unsigned interpretation of **signed lateral + forward**, and the second
candidate's direction deliberately uses the nearest candidate's sign. Shape
size's high byte forces a centered cue; this is not a bounds test. Cue 171
retains side parameters, primary/secondary routing and the shared audio queue.
No visibility, health, collision or strategy-suspension filters are invented.
Unknown selected shape metadata fails before state/audio mutation.

Verification passes 109 lowerer tests, 275 static path tests, 280 native path
tests and six warning tests in debug/release, plus freshness, architecture,
static audit and application build. Coverage includes every bearing and word
projection (16,777,216 cases), all movement/transition byte combinations,
candidate thresholds, stable ties, best-two replacement, wrapped origins,
distinct view/player positions, large shapes, retained audio, statement
atomicity and repeat-warning rearming. Complete-root coverage remains **28
roots / 670 source commands / 661 native statements**. The service is public
native code; production Game invocation is not claimed by this checkpoint.

### Literal path material selection

The two material-table assignments in shared helper `$09:81B6` now lower to
`AppearanceCommand::MaterialSet`. Full static handler checks verify the
literal word store and operand-to-field mapping, plus the source draw-record
copy. Only the two reviewed table roots are accepted; byte writes, generic
word arithmetic and other literal roots still fail lowering.

Budget-zero atomicity, retained IFNOT/random/wait/contact/animation state and
the actual Game render boundary are tested. The command changes only the
material selection. Verification passes 110 lowerer tests, 276 static path
tests and 281 native path tests in debug/release, with the render-boundary
test in both profiles. Architecture, static audit, catalog freshness and
application build also pass. This is a primitive checkpoint, not a claim
that the complete shared helper or another root has been integrated.

### Complete scene-material scenery child graph

`SCENE_MATERIAL_SCENERY` now includes all 25 source statements in the child
path and shared material-selection helper. The generator verifies its
installation through the reachable child-spawn record in parent `$44:787D`;
it does not treat a scanned address as an installer or claim the parent is
lowered. Redirecting that source spawn away from the child rejects publication.

Typed, separately optional scene observations retain the complete player
configuration and encounter-location bytes. The original save, import,
IFNOT, comparisons, material assignments, restore and return remain distinct
statements. Configuration other than nine bypasses the location import.
Shadow removal, three-shape warning membership, health, collision, contact
class, draw-distance and suspension commands then execute in source order.

Native tests cover all 65,536 selector pairs, each warning-shape boundary,
retained warning latches/membership, the skipped missing-input case, a failed
location import resumed with a fresh value, saved-phase restoration, full-byte
import preservation and zero-budget atomicity. Complete-root coverage is now
**29 roots / 695 source commands / 686 native statements**. These are decoded
native catalog and service tests; Game scheduler/spawn integration remains
open, and no original instructions or recorded gameplay were executed.

Checkpoint validation passes 112 lowerer tests, 277 static path tests and
284 native path tests in both debug and release, plus generated freshness,
architecture/static audits and the application build.

### Complete distance-gated scenery graphs

`DISTANCE_GATED_SCENERY` and `HEALTH_ROTATED_DISTANCE_SCENERY` now retain
their complete 45/46-statement source graphs, including the material helper,
nested footprint-admission helper and both yielding distance loops. Their
independent strategy installers are verified before catalog publication.
The second entry copies original health to yaw before the shared initializer
replaces health; both capture the low byte of original world Y as the bit
selector, then clear world Y. Near mode begins below 512 units and persists
until 600; far mode persists through the 512..599 interval.

The scene-owned proximity mask has separate full-byte import and export
commands. Missing state faults at the exact command without partial writes;
selector zero bypasses the shared record. The intervening bit operation
retains its source word width, including changes to the high phase byte,
before export stores only the low byte. Import/mutate/export are not fused:
a resumed export publishes the previously imported value even if another
scene writer has since changed the shared record.

Both reviewed inline footprint helpers now lower to a named contact command.
Every signature byte and return target is checked. The real surface-query
test verifies candidate admission independently of visibility and damage
collision. Native graph tests cover all 256 selectors for both roots through
eight threshold transitions, zero-selector short-circuiting, original-health
yaw, initializer flags, retained random state and command fault/resume cases.

Coverage is **31 complete roots / 728 source commands / 719 native
statements**. Validation passes 115 lowerer tests, 279 static path tests and
289 native path tests in debug and release, generated freshness,
architecture/static audits and the application build. This remains static
lowering and native service verification, not completed Game scheduler/spawn
integration or recorded-gameplay verification.

### Targeting-upgrade primitives and independent glow child

The upgrade ownership branch and acquisition command now use a typed
pilot-relative targeting record. Static checks cover both complete handlers,
the high-bit target-lock gate and the pilot-exchange bit mapping. Acquisition
preserves the other seven bits; the ownership branch takes a direct source
edge and does not consume IFNOT. Native dispatch tests cover every initial
byte with both IFNOT states, missing input, budget-zero atomicity and complete
actor/random-state preservation.

`TARGETING_UPGRADE_GLOW` independently lowers its complete five-statement
child graph: disable collision, select color frame two, yield once, select
frame three, and yield back to frame two. Its reachable parent spawn is
verified, and changing that installer rejects publication. A native test
checks 256 independent scheduling visits without executing the parent.
Classification of the two presentation-child shapes is restricted to their
reviewed child paths, not generalized to unrelated uses of either shape.

The parent pickup remains **unpublished**. Its remaining `RemoveChild`
command sets the child's deferred-removal flag, but unlike the reviewed
signal/unlink handlers it does not guard a failed lookup. The null-target
scratch-write case still needs static review; the lowerer rejects it rather
than substituting an immediate deletion or an unproved no-op.

Coverage is **32 complete roots / 733 source commands / 724 native
statements**. Validation passes 116 lowerer tests, 280 static path tests and
291 native path tests in debug/release, generated freshness, architecture
and static audits, and the application build. Target-lock service and Game
scheduler integration are not claimed by this primitive/child checkpoint.

### Attachment-absence branch from source, not the legacy interpreter

The source `ShapeDead` handler (`7F:8D9F`) now lowers to the typed
`AttachmentAbsent` statement. The name is misleading: it reads only the
actor's attachment pointer and jumps directly on zero. It does not inspect
the target's health, retirement flag or allocation status, and neither exit
consumes IFNOT. The legacy encoded interpreter's use of its generic
condition consumer is not used as the behavioral specification.

Static tests pin the complete handler and both dispatch continuations.
Native tests cover absent, live, retiring, released-slot and self links,
both IFNOT states, three health boundaries for both actors, zero-budget
atomicity and preservation of all state except the program cursor.

This is an additional reviewed command, not another complete root:
coverage remains **32 complete roots / 733 source commands / 724 native
statements**. Validation passes 117 lowerer tests, 281 static path tests,
292 native path tests in debug and release, generated freshness, the
architecture/static audits and the application build. No source-machine
execution or recorded gameplay was used.

### Complete hit-toggle sprite child paths

`HIT_TOGGLE_SPRITE` and `COUNTED_HIT_TOGGLE_SPRITE` now publish the complete
shared graph at `44:8488`/`44:8486`, including both hit callbacks and the
health-counted exit. Their parent spawns at `44:01CA` and `44:097D` are
reachable from independently discovered roots. Both parents remain outside
the complete-root allow-list. The shared sprite shape is classified as an
effect only for the reviewed paths; adjacent/unreviewed entries still fail.

The first three visits fade through color frames 1, 2 and 3. The sprite then
alternates frames 0 and 1 indefinitely when its parameter is zero, or for
the snapshotted health count otherwise. The alternate entry increments the
parameter with byte wrapping: 255 therefore chooses the indefinite branch.
The byte health operand is widened into a word loop counter, so zero takes
65,536 iterations. A hit consumes the hit-event latch and hides the sprite;
after callback completion the path replaces its trigger and holds. The next
hit restores visibility, keeps collision disabled, and restarts the fade
without resetting sprite size. The apparent NOP has a verified source RTS
body and is not an unimplemented-command placeholder.

Native tests cover all parameter/size byte values, health boundaries, full
zero-count completion, both entry points, and repeated hide/show events
during each of the initial eight visits. Source tests pin the entire graph,
both spawn records and the actual NOP helper; installer mutations fail.

Coverage is **34 complete roots / 771 source commands / 762 native
statements**. Validation passes 118 lowerer tests, 282 static path tests and
294 native path tests in debug and release, generated freshness,
architecture/static audits and the application build. This is static
source lowering and native service verification, not whole-game completion.

### Growing sprite hold and shape-filtered scenery children

`GROWING_SPRITE_HOLD` (`44:0059`) has a complete eight-statement graph and a
verified parent spawn at `44:0210`. It disables collision, initializes size
16, adds two for each of fifteen iterations, yields once, then selects shape
18 and holds at size 46. Selecting that shape does **not** clear sprite mode.
Native tests cover all retained wait/depth-high byte values, every growth
visit and subsequent holds, while preserving position, health, attack power,
animation and IFNOT. The source spawn's shape 16 is classified as an effect
only with this path.

`SHAPE_FILTERED_SCENERY` (`44:7F78`) reuses the scene-material and surface
helpers within its complete 31-statement graph; its parent spawn is
`44:5916`. It adds proximity-warning membership only for shapes 145 and 201,
preserving membership already set for another shape. It enables footprint
search, disables damage collision, sets health 100 and attack power 4, and
enters ordinary movement hold rather than strategy suspension. Tests cover
all 577 shapes, both initial warning states, material-selector branches,
saved phase restoration and repeated holds. The source spawn's shape 239
is classified as scenery only for this path.

Both parent graphs remain unpublished. Complete entry/spawn bytes are
pinned by static tests, and altered child targets reject generation.
Coverage is **36 complete roots / 786 source commands / 777 native
statements**. Validation passes 119 lowerer tests, 283 static path tests,
296 native path tests in debug and release, generated freshness,
architecture/static audits and the application build. Game scheduling and
whole-game completion remain separate work.
