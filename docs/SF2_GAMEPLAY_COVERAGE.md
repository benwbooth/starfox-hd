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
