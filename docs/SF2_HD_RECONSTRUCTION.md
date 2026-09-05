# SF2 HD reconstruction — incomplete

The shipping game runs typed Rust gameplay, but that is not equivalent to a
complete native presentation. Its intro, title/records, briefing, strategic
opening, pilot selection, game-over, results and ending currently include
retail full-frame image-delta tracks. Those pixels contain geometry,
checkerboarding and animation cadence that a modern renderer cannot change.
Do not report their matching pixel hashes as native scene or HD coverage.

## Verified first tranche

- Native SF2 object allocations now have lifetime tokens distinct from their
  reusable semantic slot IDs. Rendering cannot interpolate a replacement
  object from the retired object's pose.
- Scene changes snap both camera and draw-list endpoints.
- Narrow SF2 windows preserve the entire 2D canvas; wide-window placement is
  unchanged.
- Ordinary SF2 polygon distance colors use the shared continuous HD shading
  path. Explicit object depth overrides and retail dithering remain available.
- GPU regressions exercise real SF2 meshes at two resolutions, camera motion
  between ticks, and pixel-exact preservation of a canvas in a tall window.
  These deliberately distinguish native geometry from enlarged captured images.
- Adjacent SF2 mesh-animation frames now interpolate authored vertex positions
  in HD, including their projected shadows. Reverse and wrapping animations
  are supported; object replacements, shape changes, nonadjacent jumps,
  incompatible topology and explosions do not morph. Integer game state,
  materials and reference-resolution animation remain discrete. A stationary
  mesh GPU regression checks quarter/mid/three-quarter poses, exact source
  endpoints, lifetime replacement and discontinuity behavior.

These changes **do not replace the recorded intro**. Mission geometry already
used camera interpolation before this tranche; the low-rate intro is a
different presentation path.

## Recovered boot root

`tools/sf2/mesen_intro_scene_trace.lua` walks the actual active object list,
filters generic path strategies, records draw/camera state, and
exposes the early path writers. It is oracle-only. Capture inputs are neutral;
settings come from the existing disposable-profile runner.

The live title-path installation occurs at source `$06:A96E`. It is **indexed**,
not an immediate path literal: the bounded selector indexes 30 eight-byte
records at `$0D:D4C7`. Boot selects record six, whose path is `$44:FA11`.
`authored_intro_root()` validates both the selector and install instruction
signatures before reading this table. The older generated root catalog missed
this indexed installation.

Two newly reviewed inline blocks have explicit return continuations:

- `$44:FCB9`: calls the attract-craft drift/roll-settle service `$06:FA04`,
  then returns `$FCC4`. This establishes graph reachability, not a native
  implementation of that service.
- `$44:FDDC`: publishes the selected child auxiliary record, then returns
  `$FDE8`.

An important trace pitfall: a sampled cursor can point **after** an extended
opcode's consumed zero escape. `$FBB6`, for example, belongs to the instruction
starting at `$FBB5`; decoding it as a primary opcode invents a spurious graph.
The verifier permits that proven one-byte adjustment, not arbitrary interior
operand offsets. It starts from the authored root, never sampled cursors.

For the retained 800-video-frame neutral capture, the authored graph contains
718 decoded commands and covers all 45 observed cursors. Two handlers
(`$145`, `$146`) still lack reviewed semantic catalog entries. These counts
are **not** a declaration of completed native choreography or full intro
coverage. Independent host timing, camera control, scene layers, and the
rest of the attract loop still require reconstruction and verification.

The initial **4,560-video-frame** neutral capture contained 167 observed path
cursors; 20 were unreachable from the single intro root. A subsequent probe
at the indexed installer itself established the missing handoff:

- Video frame 156: selector/index six installs `$FA11`.
- Video frame 2,034: selector/index seven installs `$B65B`.

`sf2_intro_scene_installations.txt` records those actual executions.
`--installations` validates each selector, its source clamp, its table entry,
and chronological ordering. It cannot introduce arbitrary sampled path roots.
Four additional inline blocks (`$B796`, `$B869`, `$B7E7`, `$E91B`) now have
signature-checked return continuations. Both phase-test branches are retained.
Together the two installed roots reach **903 commands, all 167 observed
cursors, and no unresolved control-flow blocks** for this neutral corpus.
Four handler identities (`$143`, `$145`, `$146`, `$151`) are not yet registered
in the older generated path semantic catalog; that integration gate stays red.

This is not all-scene coverage. The indexed installer has 24 distinct roots
across its 30 records. An exploratory traversal of all 24 reaches 3,855
commands and 15 still-unreviewed inline blocks.

## Native motion kernels

`rust/sf2-game/src/native/intro_motion.rs` now implements typed counterparts of
the four uncatalogued handlers and the attract camera's yaw-settling service.
`rust/sf-oracle/tests/sf2_intro_motion.rs` executes the original ROM routines
independently and compares the resulting fields, rather than reproducing the
native arithmetic in expected-value fixtures.

- `$145/$146`: copy the current rotation/position into the selected player
  and its retained movement origins. Retained pitch/yaw have eight fractional
  bits; retained roll is integral. Tests exercise every angle byte, signed
  position boundaries, nonzero adjacent bytes, and aliased/distinct objects.
- `$143`: chase each fine camera angle toward the current object's coarse
  angle. `$7F:25A3` takes one eighth of the signed wrapped displacement,
  truncating toward zero, with a minimum nonzero step of one. It is **not** a
  maximum-eight-unit clamp. Differential tests cover all 65,536 targets from
  three wrapping origins and the complete command's per-axis wiring.
- `$151`: copy each fine camera angle's high byte into the current object's
  coarse rotation. This handler enters in eight-bit mode; it does **not**
  copy overlapping words. The test checks that adjacent bytes are unchanged.
- `$07:F52B`: settle fine yaw toward 49,152 using two separately rounded signed
  halves. All 65,536 input yaw values match retail execution. Combining the
  arithmetic into a simple three-quarters multiply is not equivalent.

The oracle replaces only the common post-command dispatch continuation with
a return, so each pose handler and its called helpers run from original bytes
without executing a subsequent script instruction. This harness is confined
to `sf-oracle`; the native types have no machine-state container.

These kernels are **not yet wired into a native attract scene scheduler**.
They do not change the recorded intro currently shown by the application.
The next section describes the newly reconstructed Nintendo assembly and
arrival segments. Host timing, layers and cut/skip transitions still require
integration and verification.

## Native Nintendo-logo assembly and arrival

`rust/sf2-game/src/native/intro_logo.rs` contains domain-state controllers,
not a script interpreter or sampled animation track:

- `NintendoLogoAssembly` reproduces `$9284`'s nine sequential paired glyph
  spawns, signed spacing, final sweep spawn and release timing. The first
  spacing is -70 scaled by eight, not unsigned 186. The sweep's X coordinate
  is an absolute -750, independent of the original parent X.
- The ninth pair, sweep spawn and first hold update occur in the same scene
  update. With the initial update numbered zero, release occurs on update
  100. No wall-clock or display-frame duration is inferred from that count.
- `NintendoLogoArrival` reproduces the shared `$943D..$9456` approach and
  settling segment. Twenty translations add 50 depth units each. Each also
  adds eight pitch units, but the final loop iteration immediately enters
  settling and can add another eight in that same update. Settling tests for
  zero before rotating; it does not snap an arbitrary angle to zero.

`rust/sf-oracle/tests/sf2_intro_logo.rs` executes the original paths, including
their allocation, dispatch, loop and yield handlers, without patching those
handlers or their continuations. It checks all 256 initial pitches across
64 updates each, comparing position, all rotation axes and the suspended
path. Four parent origins exercise signed spacing and coordinate wrapping
through the complete spawn-and-release path. Every update checks mesh
identity, child roles 19/20, positions, sweep creation and release timing.
The parent-only test intentionally does not advance its children: child
motion is independently tested by the arrival test.

### Complete glyph and sweep lifecycles

`NintendoLogoActor` now reconstructs the complete `$93F0` glyph path, including
setup, visibility, material override, texture scrolling, release and removal:

- Primary and secondary layers have depth offsets one and three. The primary
  starts invisible, overrides the material table, and becomes visible after
  ten updates. The secondary is visible immediately. Their distinct geometric
  clipping selections are typed as `LogoClipping`, independent of material.
  Plane construction is now reconstructed below; clipping the submitted
  geometry remains a renderer-side integration gate.
- `$9427`'s scheduled target is `$944E`. Its condition kind is 17, and its
  countdown starts at 11. The scheduler decrements before testing; condition
  17 fires at one, giving the ten-update reveal. Neither 17 nor 11 is itself
  the number of hidden updates.
- `$948C` increments texture-scroll Y by four per yielded update. Its target
  variable is `$9A`, resolving to the object's `$1CDB` extension. It does
  **not** advance the palette or color-animation frame.
- Release removes the secondary layer without consuming random values. The
  primary normally consumes exactly four: departure pitch/yaw, then two spin
  increments. It preserves the source's exchange of retained pitch/yaw. An
  explicit exit policy also reproduces the source's no-dispersal branch.
- Forty departure rotations accompany thirty-nine translations: the final
  iteration reaches `End` without the common movement or texture-scroll pass.
  An ended native actor is not drawable and subsequent ticks do nothing.
- Scene scrolling is independent of actor velocity. The selected player's
  horizontal-lock policy suppresses X scrolling, not depth scrolling.

`NintendoLogoSweep` reconstructs `$9378`: initial X/Z offsets, fixed roll,
19-update delay, 13 horizontal advances, then release-controlled removal.
Its final advance can reach removal before the usual scene-scroll pass.

`rust/sf-oracle/tests/sf2_intro_logo_actor.rs` checks **672 glyph cases**:
seven random seeds, both layers, ordinary/outline meshes, three release
times, four initial pitches and both exit policies. Every update compares
position, rotation, velocity, visibility, material, texture scroll, draw
policy, suspended path, all random-state bytes and completion. It verifies
the outline child's spawn without advancing that separate child. Another
15 cases check the complete sweep path at release and wrapping boundaries.
These tests execute unmodified retail handlers and continuations.

### Attached outline and complete logo-family scheduling

`NintendoLogoOutline` reconstructs the `$F01D` child. It uses its own material
table for 37 updates, switches to the settled table on update 37 (numbered
from zero), and holds. Its shape is catalog entry 372 (`$E54C`). It stays
visible while the primary parent is initially hidden, and does not inherit
the parent's depth offset, clipping selector or texture scroll.

The source spawn at `$9414` uses zero local position and rotation offsets.
The common parent pass calls `$7F:2319/$7F:2229` after movement, so this
attachment shares the parent's updated pose. Its own path does not apply
world scrolling again. `End` skips that parent pass: the child's final pose
remains from the preceding update, while its own material clock still runs.
Cleanup `$7F:344F` marks dependent children for removal, and the same list
cleanup traversal frees the outline with its parent. This is distinct from
the damage-related detach service `$7F:2AA4`.

`NintendoLogoLayer` composes the parent with this child.
`NintendoLogoAnimation` now composes the entire actor family: the assembly,
nine paired glyphs, the attached outline and the clipping-plane sweep. It
uses typed fields and fixed arrays, not a source script or emulated pool.
The assembly's common scrolling is preserved as well as each layer's own
movement. New actors run on their creation update. Since quick spawns insert
after the assembly, updates visit the newest glyph pair first, secondary
before primary; the final sweep precedes them all. This ordering determines
which primary letter consumes each random value at dispersal.

`sf-oracle/tests/sf2_intro_logo_attachment.rs` verifies both compositions
using the unmodified retail actor-list update passes `$7F:34E7/$7F:354A`
and cleanup `$7F:402D`, not a hand-ordered loop over isolated paths. The first
pass overlaps work while graphics are busy; the second resumes its saved
cursor. Running the first alone with an idle graphics unit updates no actors.

- 24 parent/outline runs cover three release times, both exit policies and
  four initial pitch states, including a non-settling pitch residue. Every
  update checks both poses, child material and path, independent presentation
  fields, attachment, and same-pass removal.
- Six full-family runs cover three random seeds and both stationary and
  scrolling scenes. All 18 glyph layers, the outline and sweep match retail
  identity, poses, visibility, material, texture scroll, clipping and lifetime.
  The entire random state matches after every update. Native completion
  occurs at update 139, when the source active list is empty.

This does **not** yet reconstruct the full logo presentation. Initial plane
state, polygon clipping integration, the surrounding attract-scene actors,
camera and production scheduling still need integration. The application
still displays its recorded intro. The complete logo family is a bounded
source-behavior result, not an HD-intro or full-SF2 completion claim.

### Authored clipping planes: corrected extraction and native math

The formerly unidentified draw-record field `$1E` is a clipping-plane
selector, not a shading mode. Retail `$01:D1BB` reads its byte and stores it
at `$24DE`. `$01:F2FA` selects the plane, rebases it for the current object,
and computes signed vertex distances. `$01:F379` classifies faces and
`$01:F3A6` handles partial polygon clipping.

The sweep's catalog entry 48 (`$C1DC`) has no visible polygon faces. Its
shape stream contains **two continuing `$68` commands**, which author
opposing planes in slots four and five. Entry 49 (`$C1F8`) similarly authors
slots six and seven. The old extractor called this `procedural` and stopped
at the first command, silently dropping the second plane. The corrected
extractor preserves the slot and both signed local points and continues
parsing; truncation, unknown tail commands, invalid slots and changed dispatch
signatures fail closed. All 577 shapes, 11,860 vertices, 10,524 faces and
1,342 animation frames remain intact.

`sf-render/src/sf2_clipping.rs` reconstructs the source calculation using
ordinary typed plane and transform structs:

- Transform both local points with separately rounded signed Q15 products.
- Subtract the transformed origin from the endpoint and scale by eight,
  preserving signed-word wrapping.
- Compute distance from the transformed origin plus the object's translation.
- Rebase for each mesh before computing its vertex distances. Nonnegative
  distances survive. The opposing plane is calculated independently, not
  approximated by negating its partner; their rounding differs.

`sf-oracle/tests/sf2_clipping.rs` executes the unmodified retail routines
against all four authored planes, 256 combined orientations, four translations
and seven vertices: **4,096 plane constructions and 28,672 vertex distances**
match. The test harness sets up machine state only in the oracle; shipping
plane math contains no emulated address space or processor state.

Independent read-only Mesen verification covers an 800-video-frame neutral
boot. `mesen_logo_draw_policy_oracle.lua` requires both selectors to be
consumed at the verified load and both planes to be installed. It records
236 installations, including their input matrices/translations and resulting
planes. Every row matches native math. The numerical fixture
`tools/sf2/fixtures/logo_clipping_planes.csv` is verification-only, never
an animation track or production source of plane values.

Remaining: initial plane state and draw ordering, geometry clipping, surrounding
scene scheduling, and camera integration.
The current application still displays the recorded intro; these verified
components do not yet constitute a visible HD-intro fix.

## Native opening camera rig

`sf2-game/src/native/intro_camera.rs` implements the complete `$44:FB4C`
camera path as typed state, with five source-authored position cuts and scene
cue equality gates. It preserves the one-update startup wait, the 18-update
slow-flight loop, its five-update wait, the 20-update coordinate chase and the
final indefinite tracking hold. The last loop iteration falls through into
the next command in the same update. A scene cue arriving early is consumed
when its gate is reached; a missed cue does not silently skip the gate.

The scheduled `$FBB1` view update copies the rig's position, aims at a separate
target actor, negates the complete fractional pitch before an arithmetic half,
and levels roll by a signed quarter step with a minimum fractional-unit step.
Yaw retains its fractional part. The existing one-eighth angle chase is not
the roll-leveling rule.

The rig **does not opt into global scene scrolling**. Source `$9F16` gates
that addition on an actor flag absent from this rig's spawn and path. Instead,
the rig imports the scene depth velocity while following, retains it through
the first cut, and clears it on the second. Adding both motions would double
the opening camera speed. View publication occurs after movement, including
on the update when the scheduled handler is installed; the initial wait does
not publish anything.

Automated verification executes the unmodified original camera path and
scheduled handler through the retail actor-list update. Twelve 180-update runs
vary cut spacing, early/late final cues and changing signed scene scroll.
Every update compares position, velocity, fine view angles, waypoint index and
the exact suspended path location. Authored waypoints are checked directly
against the ROM tables. Separate tests cover all 65,536 roll values, every
signed coordinate toward both authored chase targets, and 1,458 combined aim
edge cases, including subtraction overflow and coincident positions.

This reconstructs the camera rig, not its surrounding target actor, scene
controller or renderer. The shipping intro still uses the recorded track.

## Native opening camera target and attached poses

`intro_motion::IntroAttachment` publishes a world pose from retained local
coordinates using the complete source attachment transform. Source `$7F:2229`
calls the GSU matrix builder `$01:92BC`, **not** the view-matrix builder
`$01:9191`. Negating/transposing the existing camera matrix, or successively
rotating a point, is not equivalent under the source's per-product rounding.
All 1,280 combined-rotation/offset cases match the complete original routine,
including its matrix construction, point transform, translation wrapping and
local rotation addition.

`intro_target::OpeningCameraTarget` reconstructs the complete `$44:FAB2`
attached target path through its removal at `$FB07`:

- Select itself as the camera target and wait 100 updates at local depth 300.
- Find the nearest eligible catalog-64 actor using the source X/Z distance
  approximation, a strict 7,000-unit upper limit and active-list tie order.
  Height affects aim but not target selection; negative overflowed distances
  are excluded. Missing targets preserve the source orientation behavior.
- Retain the attachment parent while temporarily aiming at the chosen actor,
  copy the coarse pitch/yaw into the local orientation, fly at speed 50 with
  an additional -5 lateral velocity for 20 updates, then stop.
- Once the opening cue changes, replace the local offset with (-500, 0, 300),
  wait five updates and aim at a catalog-338 actor. The final iteration of its
  30-update flight loop falls through into the cue gate immediately.
- Removal skips local-velocity integration. Parent pose publication occurs
  before the child's own update; local movement becomes visible on the next
  parent pass, not immediately after the child moves.

The full actor-list differential test covers 32 lifetimes with rotating/static
parents, early/late cues, absent, tied, differently spaced and out-of-range
targets. Each update compares world/local poses, speed, velocity, retained
aim identity, parent link, camera selection, path cursor and removal.

## Recovered parallel scene controller

The eight-byte scene installer record contains more than an actor path.
`$06:A94A..A955` copies the neighboring three-byte pointer into the selected
player's timed controller. For boot record six the complete record is
`11 FA DF BE 0D 00 00 00`: actor path `$44:FA11` and companion stream
`$0D:BEDF`. The actor path graph cannot establish coverage of this stream.

`tools/sf2/disasm/extract_intro_controller.py` now decodes that companion
directly from the source table, with installer and dispatcher signature
checks, explicit condition kinds and stream bounds. It preserves service
order and unsigned comparisons; intervals have exclusive upper bounds.
Unknown conditions and invalid/truncated pointers fail closed.

The opening stream contains 14 service records:

- At update 14, `$0D:CF2A` saves the current palette and replaces four authored
  color-ramp entries. Updates 107..139 call the palette-restore step twice.
- At updates **182, 249, 293, 327 and 416**, `$0D:C82F` increments the scene
  cue and resets its local timer. These are authored timings, not Mesen pose
  samples or values invented by a differential harness.
- At update 441, `$0D:CA18` sets the scene-transition request.
- Intervals 169..185, 314..318 and 409..413 run the palette flash step;
  185..217, 324..356 and 417..449 restore toward the saved palette.

`intro_controller::OpeningSceneController` now implements the complete
14-record stream with typed actions and the original ordering. It advances
source-update counters, not display frames, emits the five camera cues and
scene-transition request, and freezes both counters when the main timer
saturates instead of wrapping and replaying the opening. The caller must
withhold scene updates while the controller is suspended.

`OpeningScenePalette` retains typed five-bit artwork colors, the saved palette,
refresh request and three effect-policy booleans. It implements the logo ramp,
warm flash and single/double restoration steps. Two easily missed source
details are preserved: flash skips each palette row's first color but restore
does not, and restoration clears policy flags only on the first **unchanged**
step, not the step that first reaches the saved colors. The warm flash clamps
green and blue to 28, including input values already above that limit.

`sf2_intro_controller` independently executes unmodified source `$0D:BCCF`
and its palette services. Verification includes:

- All authored records and action order read directly from the ROM.
- 24 complete 460-update runs: three palette patterns and all eight effect
  policies, comparing every live/saved color, refresh and transition flags,
  camera cue and both counters after each update.
- Every one of the 32,768 valid BGR555 artwork colors through the flash.
- All 1,024 source/target component pairs on each color channel through
  restoration and its completion step.
- A continuous 65,538-update run checking timer saturation and no replay.

The camera differential suite also consumes the actual source controller
alongside the typed controller over four 460-update cases, in addition to its
12 adversarial cue schedules. This verifies component composition with the
authored timings, not the complete scene's global actor scheduling.

These controller, palette, target and attachment components are **implemented
and tested but not integrated into the shipping native intro**. They do not
replace the recorded production scene on their own. Child actor choreography,
palette initialization, global scheduling and native rendering remain open.

## Native opening root choreography

`intro_root::OpeningSceneRoot` reconstructs source `$44:FA11..FAB1` as typed
ordered events and persistent root motion. Its direct actor kinds are enums;
independent spawns retain the pre-movement root pose, while attached spawns
carry a local pose and semantic attachment group. The root does not execute
an emulated path stream or use sampled animation coordinates.

- Initialization retains the zero player pose, installs background origins
  (horizontal 176, vertical 400), publishes the derived depth velocity and
  opening cue, then requests the tracking target, logo assembly, camera and
  attached flyby rig in source order.
- After the full 96-update wait, it queues the flyby audio marker before
  requesting the attached craft, free craft and three distinct formation
  members.
- On the first camera cue it yields for one more update, then marks the
  first flyby-rig attachment for removal and requests the second flyby craft.
  The third cue requests the next camera target; the fourth enters Hold.
- Equality gates remain equality gates: a missed cue does not synthesize
  later actors. Hold continues root motion without re-emitting events.

The authored speed **10 is not a displacement of 10**. Both original
fixed-point cosine products execute even at the zero heading, yielding depth
velocity **8**. The native root uses the existing verified flight-velocity
kernel rather than copying the script's speed directly into position updates.
The exported depth velocity is the derived value, too.

`sf2_intro_root` independently executes the unmodified original root strategy,
allocation helpers, movement and attachment publication. Across 6,200 updates
it compares all 11 direct spawn identities and source parameters, ordering,
pre-movement independent poses, local and published attachment poses,
parent/group links, removal flags, queued audio, retained player pose,
background origins, velocity and path phase. Cases include authored cues,
early cues, skipped equality gates, a permanently held opening and signed
coordinate wrapping. Two ROM-free tests also cover composition with the typed
controller and event ordering at a cue boundary.

This verifier deliberately does **not** execute the spawned child strategies.
It proves the root's event producer, not their behavior or the complete scene
scheduler. The flyby rig and streaks now have the native consumer described
below. The independent craft (`$FCC5`) now has a native path and composed
destruction lifetime, also described below. Remaining craft/formation paths
(`$FCF2`, `$FBD0`, `$FDC2`) and the later target (`$FB08`) are still gates before
this root can replace the recorded scene.

## Native flyby rig and streak family

`intro_flyby::OpeningFlybyEffects` implements `$44:FD5C..FDC1` as a typed rig
and three separately scheduled streak actors. After 96 updates the rig spawns
them at local depths -500, -1884 and -3268. It retreats by 20 for 15 iterations,
chases local X toward -400 for eight iterations, then waits for the opening
cue to change and yields once before ending. The final iteration of each loop
falls through to the next phase in the same update. A simultaneous scheduled
pitch decrement stops only at angle 10, with byte wrapping preserved.

Each streak waits one update before selecting catalog mesh 119. It takes 80
local-depth steps of 100, switches to far sorting after the first 35 steps,
and ends after the remaining 45. The source schedule stores 81 but decrements
before executing and suppresses the callback on zero; End also bypasses the
common callback pass. It must not take an 81st motion step.

The far phase changes **sort depth, not visibility**. Original draw-record
code `$7F:122C..123F` reads the flag and writes 15000 rather than the normal
zero/no-override marker. Its exact instructions are guarded in the verifier.
The original mesh is four intersecting narrow polygons, not a captured frame
or a replacement smoke bitmap.

Full-family verification exposed two important scheduling distinctions:

- The streaks are members of the common parent's flat attachment list, but
  their transform owner is the rig. `$7F:2229` prefers the separate owner
  link when present. Publication visits the rig first and then transforms
  streaks through that freshly published pose, before the rig's local motion.
- A removal request does not cancel the current strategy update. Its final
  publication, local motion, schedule and even newly due spawns still run
  before cleanup. Removing the rig does not delete its sibling-list streaks;
  their owner pose is retained while those existing effects finish.

`sf2_intro_flyby` executes the unmodified original actor-list update, resume,
all rig/streak strategies and cleanup. **200 complete family runs** combine
five starting pitches, four cue timings, moving/rotating parent poses, and
natural versus four externally requested removal times. Comparisons cover
creation order, local/world poses, transform-owner and parent/group links,
shape selection, sort policy, active-list removal and path phase (except the
discarded final cursor on externally requested removal). Three ROM-free tests
protect the exact visible/motion lifetime, publication order and removal on
the spawn update. Completed native actors retain identity and cannot restart.

This closes the rig/streak dependency, not the entire opening. Renderer
integration, remaining craft/target behavior, root-wide scheduling, palette
initialization and later scenes remain outstanding.

## Native independent opening craft and auxiliary effect

`intro_free_craft::OpeningFreeCraft` reconstructs `$44:FCC5..FCF1` and its
position-table helper `$44:FBBB..FBCF`. It flies from (-1700, 200, -2300),
waits for the first camera cut before becoming invisible, then reappears at
(200, 800, -2100) only at the third cut. It retains velocity across both
position resets. The source uses speed 20 through its fixed-point heading
calculation, then overrides vertical velocity with -5; speed is not a raw
world-depth step. Invisible updates continue moving. The far-sort override
remains 15000, separate from visibility.

After 14 reappearance updates it configures the selected-player auxiliary
effect and requests audio marker 139, class 2. The effect origin captures the
craft's **pre-movement** pose. `IntroAuxiliaryEffect` implements the complete
configuration service `$07:B6EF`, including its called origin/ownership,
axis-mode and control-field setters. The source doubles only the low byte of
the supplied range, not the whole signed word. It clears tracking even when
configuration is frozen, and its final separate origin refresh still runs
when the frozen effect already belongs to this actor. That final distinction
is absent from the older compatibility helper; verification executes the
original machine code, not that helper.

The final command is **not a hide operation**: it sets health to zero. After
one last movement update, `$7F:3596` transfers the craft to the common death
handler `$03:A055`. Native code emits `request_destruction` and stops running
the craft path; it must not silently substitute disappearance or endless
invisible movement for the destruction consumer. An original-only regression
extends through that handoff and cleanup: this fixture produces two new
effects (catalog shapes 0 and 12, strategy `$03:A279`) and removes the craft.
The composed native consumer below now continues through their full lifetimes.

The path comparison uses **64 cases** covering authored, early, missed and
revisited cue schedules, alternate inherited headings, all frozen/tracking
policies and owned/unowned effects. Each case starts with a craft created by
the original root and QuickSpawn, preserving its real constructor flags.
Other scene actors are excluded from execution for this isolated comparison.
Missed-gate cases run 4000 updates to cover signed-coordinate wrapping;
completed paths are compared through the explicit destruction boundary.
Every update checks pose, velocity, visibility, shape, path phase, marker
request and all configured auxiliary fields. **144 separate service cases**
exercise low-byte carry/wrapping and frozen ownership across both the flyby
and independent-departure auxiliary services. Three ROM-free tests
protect event order, missed gates and the frozen-owner behavior.

This is not a completed native opening or a demonstrated visible improvement.
The auxiliary effect's downstream presentation/update service, other
craft/formation actors, complete scene scheduling and native renderer
integration remain necessary before replacing recorded frames.

## Native destruction effects and complete independent-craft lifetime

`intro_destruction::IntroDestructionEffects` reconstructs the standard
destruction-effect constructor `$03:A055`, companion constructor `$03:A62A`
and applicable `$03:A279` animation behavior. It consumes typed shape bounds,
positions, view/listener context and available actor capacity. The independent
craft now composes this consumer through `OpeningFreeCraftSequence`: its
health-zero request completes one movement update, the next scene update
retires the craft and creates the effects, and the sequence remains active
until every effect has been cleaned up.

The constructor chooses catalog sprite 9, 10, 11 or 12 using twice the larger
X/Y bound, with 4, 6, 8 or 8 animation updates. Smaller variants preserve the
source's wrapped subtraction and logical shifts when deriving their extra
sprite-size byte. **All 577 catalog profiles** match the original constructor,
including selected mesh, size byte, initial pose, timing and allocation order.

Full-family execution found an important distinction from reading the
animation routine alone. Large effects also create a meshless companion with
style channels (30, 30, 7), but that constructor leaves its health at zero.
Its one-time newborn exemption permits the first animation update. On the
next update the actor-list dispatcher runs common destruction instead of the
animation routine's apparent 64-update continuation. This creates a third,
small explosion sprite and another sound request, then removes the companion.
In this isolated active-head case, the small sprite runs its first update
immediately, before the older main sprite. The source can reuse the original
craft's freed slot for that sprite; native completed entries are retained as
distinct lifetimes and never reused.
`IntroExplosionBirthTiming` now makes that traversal dependency explicit:
births inserted after an already-visited scene root run on the next update,
as verified by the attached-craft family below.

The animation pass compensates for scene scrolling only in the original
selected-view mode, subtracting twice the horizontal/depth scroll using signed
word wrapping. Common destruction itself does not apply that compensation a
second time to the retiring companion. Sound requests are emitted for the
secondary listener first, if enabled, then the primary listener. Their three
attenuation bands use the original wrapped X/Z approximation and signed
comparison behavior, not Euclidean distance. A separate test compares
**65,536 signed-axis distances**, including translated coordinate wrapping,
height independence and audio-queue wraparound, with `$06:8000`.

Allocation failure is not silently successful: `$7F:2925` enters the original
console diagnostic when no slot is available (the headless executor reaches
its cycle guard there). Signature checks protect that branch. The modern
consumer returns an explicit `IntroDestructionCapacityError` instead of
omitting sprites or emulating the diagnostic console. Capacity is measured
before cleanup; the retiring actor's slot becomes available only afterward.
Scene-wide reclamation of other effects under pressure remains an integration
responsibility, not something certified by these isolated family tests.

Verification now includes **90 complete source/native effect-family runs**
covering eight representative shapes, available capacity, effect suppression
and moving scroll context, plus **eight complete independent-craft runs** with
authored/early cuts and scrolling. Both use the unmodified original actor-list
update, resume and cleanup. Every update compares new-actor order, positions,
sprite/frame/style state, sound queue, removal and independently calculated
free-slot counts, including reuse of the old craft slot. The latter tests begin
with the craft created by the original root/QuickSpawn and end only when all
three effect lifetimes finish. Three ROM-free tests protect the companion
handoff, capacity errors and suppression.

This closes the independent craft's standard destruction **behavior**; it
does not yet render that native sequence in the shipping intro. Sprite and
companion-style rendering, the auxiliary effect's downstream services, other
craft, whole-scene allocation/scheduling, and rendering integration remain.
Attached-child detachment, custom death callbacks and gameplay score/pickup
policies are outside this independent-craft consumer and still require their
own source-backed integration where applicable.

## Later opening camera target and attached effect

`intro_late_target.rs` now reconstructs the root's second camera target
(`$44:FB08`) and its attached effect (`$44:FF98`) as a typed two-actor family.
The target's authored table entry 15 supplies position `(-550, -220, 150)`
and rotation `(20, 128, 0)`, replacing every inherited pose component. It
selects itself as the camera's rotation target, spawns catalog shape 118 at
local offset `(-500, 0, -3936)`, flies for 17 updates and holds for 30 before
ending. Shape 118 is an original crossed-polygon flight effect, not a bitmap
or a sampled animation track.

Both actors double the already-truncated source flight velocity at speed 127;
doubling the speed before direction conversion would not be equivalent. The
effect retains zero local rotation and waits 15 updates before moving in its
local frame. The parent publishes its world pose after its own movement and
before the effect's local movement, preserving the one-update publication
delay. Only equality with the fourth camera cut ends the effect: an earlier
cut during the initial wait is not remembered, and a later cue is not treated
as an implicit match.

Parent End bypasses its motion/publication pass. Its surviving child still
runs its strategy that update, then both are removed by cleanup. An external
parent-removal request likewise does not cancel the current strategy, spawn,
publication or child update. Finished state is retained but inert. The source
keeps the last selected camera-target identity after removal, so the native
family emits no invented target-clear event; resolving that retained identity
belongs to the scene consumer.

`sf2_intro_late_target` executes **176 full family comparisons**: eleven
fourth-cut timings (including absent), sustained versus one-update cues, and
eight parent-removal timings (including natural End). The parent is created
by the original root/QuickSpawn with subtype 15 before isolation. Every update
executes the unmodified original actor-list update, resume, attachment pass
and cleanup, comparing both poses, local transforms, velocity, speed, shape,
ownership, camera selection and lifetimes. Ambient scroll is varied to check
that these actors do not consume it. Three ROM-free tests additionally protect
publication lag, cue equality and final cleanup/idempotence.

This verifies successful-allocation behavior of this family, not whole-scene
capacity policy or its final rendered appearance. Root-wide scheduling,
remaining craft/effect services and rendering integration still gate replacing
the shipping recorded intro.

## First attached craft, departure copy, flare and burst family

`intro_attached_craft::OpeningAttachedCraftSequence` reconstructs the complete
first attached Arwing family: craft `$44:FCF2`, independently moving copy
`$44:FD22`, attached flare `$44:FD52`, scheduled burst callback `$44:C621`,
random sound helper `$44:DC00` and particle animation `$44:C520`. It composes
common destruction instead of dropping actors when their health reaches zero.

Updates below are relative to the attached craft's first strategy update:

| Update | Source behavior |
| --- | --- |
| 0–35 | Advance local depth by 7; root publication precedes local motion. |
| 36 | Spawn the flare and an independent copy; change attached craft style. |
| 44 | Copy starts 20 scheduled horizontal/vertical drift callbacks. |
| 60 | Copy removes its mesh, shifts left, configures departure auxiliary state and starts emitting bursts. |
| 63–64 | Copy requests death, completes its last motion/callback pass, then enters common destruction. |
| 64–68 | Attached craft runs its burst callback, then configures flyby auxiliary state and requests death. |
| 69 | Root publishes the craft's final pose; common destruction retires it. |
| 101 | The retained flare finishes; all family members have been cleaned up. |

The flare is a root-list sibling with the craft as its transform owner. It
starts with an unpublished pose on the spawn update, receives its first world
pose on the following root pass, and outlives the craft at its retained final
pose. Its local offset `(50, 5, 0)`, rotation `(0, 3, 74)`, catalog shape 48
and far-sort policy are source-authored. The independent copy is real catalog
64 geometry until its mesh-removal command; it is not a sprite approximation.
The separate craft style variants retain their authored selections without
assigning an unverified shader interpretation.

The shared burst callback consumes the **already-advanced scene clock**, not
an actor-local age. It emits on even phases, chooses catalog 11/12 from a
second phase bit, consumes the optional sound's random branches before six
coordinate-random calls, and creates an eight-step sprite with size bias 2.
The last color step wraps to zero and ends without an additional visible hold.
Path-created particles run on their birth update. In contrast, common-death
effects are inserted after the already-visited scene root and first run on
the next update, including the companion's secondary small explosion. This
family comparison exposed and now protects that allocator/scheduler distinction.

Selected burst audio retains its pre-randomization source position and exact
sound choice. Its class-two spatial service uses integer Euclidean X/Z
distance, near/middle stereo sectors and centered far audio. That differs
from common destruction's wrapped approximate distance; the two policies
are deliberately separate. The departure auxiliary service likewise retains
its own `(4, 8, 8)` axis modes and undoubled range, with frozen-owner refresh
verified against the original service.

Verification includes **160 complete family runs** (four random seeds, five
scene-clock starts, combined rotating/wrapping parent motion, frozen auxiliary
state and death-effect scrolling). The parent/craft are source-created by the
original root, then unrelated actors are excluded while the root and complete
family run the unmodified update, resume and cleanup routines. Checks cover
each actor lifetime, positions/rotations, local motion, shapes/styles, random
state, auxiliary writes, sound payloads and independently counted free slots.
An additional **3,328 spatial sound cases** cover all listener headings at 13
distance/direction inputs, including threshold and signed-wrap extremes. A
test-only calling-convention adapter invokes the unchanged original sound,
angle and Super FX square-root routines. Four ROM-free regressions protect
birth publication, cadence, retirement/idempotence and explicit split-capacity
failure without a half-created family.

This closes the family's natural source-script lifetime, not rendering or
whole-scene cancellation/reclamation under pressure. The formation craft,
second flyby craft, downstream auxiliary presentation, global scene scheduling
and renderer integration still gate removing the shipping recorded intro.

## Formation craft reconstructed through their authored path lifetimes

`intro_formation` reconstructs all three opening formation members from the
original `FBD0` strategy and its indexed placement/impulse tables. Their four
shots use distinct authored coordinates, headings and durations. Motion is
typed world position, rotation, flight velocity and a separate decaying
impulse; there is no native machine state or captured animation track.

The reconstruction preserves initial roll, per-member speed, first-member
trail toggles, equality-only camera gates, smooth linked-target tracking,
climb/reappearance timing, and the final eighteen movement updates before
End. The final action of one counted maneuver may execute alongside the
first action of the next maneuver, before a single common flight update.
Treating every maneuver boundary as an extra frame delays the choreography.

The third member's zero-duration reappearance entry configures the departure
auxiliary effect, then still performs its last impulse/motion/elapsed update.
It exposes an explicit next-update common-destruction handoff, not immediate
invisibility or an invented normal exit. The scene consumer must compose that
handoff with the existing destruction family and correct actor-list birth
timing; this checkpoint does not claim that scene integration is complete.

Pursuit audio retains its pre-flight source position and the original
fixed-range listener policy. It is not the burst service: there is no distance
attenuation, and its signed wrapped range comparison is retained even at
extreme coordinates. Final output-channel routing belongs to the scene audio
consumer.

Verification covers **180 source-created member runs**, with original update
and resume routines, three cue schedules (including a missing required cue),
four inherited rolls and five target configurations. The comparisons check
every pose, impulse, flight vector, speed, trail/visibility flag, elapsed word,
selected target identity, path continuation, audio request/payload and
end/destruction transition. Target cases include motion, equal-distance ties,
out-of-range rejection and signed-wrap extremes. All twelve placement entries
are checked directly against the original tables. Independent original-code
tests cover **65,536 heading pairs**, **65,536 combined impulse cases**, and
**3,584 spatial sound cases**. Four ROM-free tests protect shared-frame maneuver
boundaries, missing-cue/timer wrapping, End idempotence and the destruction
handoff's final movement.

The second flyby craft, formation destruction/scene composition, downstream
auxiliary/trail presentation and renderer integration remain required before
the shipping recorded opening can be removed.

### Assembly-to-Rust review map for the formation

These are original code/data locations, not observation timestamps. Read the
script together with its actual instruction handlers; opcode labels alone do
not specify operand order or update boundaries.

| Original source | Native translation and reviewed invariant |
| --- | --- |
| `44:FBD0..FC76` | `OpeningFormationCraft::tick`: all four shot phases, equality gates and terminal paths |
| `44:FC7B..FC9F`, `07:FB83..FBEE` | `opening_formation_placement`: member/shot-indexed position, pitch, yaw and duration; placement does not overwrite roll |
| `44:FCA0..FCB8`, `07:FBEF..FC2D` | `load_impulse`: three impulse components and roll; the exit shot deliberately does not reload these |
| `44:FBDD`, handler `7F:89A8` | Initial roll is 246: SetByte takes the value first and destination second, unlike AddByte's operand order |
| `06:FA04..FA65`, `7F:27B5`, `7F:25A3` | `advance_formation_impulse`: coarse roll chase, add current impulse, then decay each signed coordinate toward zero |
| `7F:89EF`, `7F:1EF8`, `7F:21A5`, `7F:2188` | `select_target` / `aim`: nearest eligible X/Z target, first-active tie retention, pitch and coarse-negated yaw followed by shortest-arc easing |
| `7F:9DDE..9F15`, `7F:855F` | Recompute flight from current heading before world integration; no extra integration at End |
| `44:FC19`, `7F:A52A` | `OpeningFormationAudio::spatial`: fixed-range request, integer Euclidean X/Z distance and unattenuated panning |
| `44:FC51..FC62`, `07:B746` | Third-member departure effect precedes its final movement; common destruction begins on the next dispatcher update |

Runtime differential checks supplement this mapping by detecting translation
mistakes. They do not define the choreography, supply sampled poses, or
replace static review of unobserved branches.

## Later flyby camera target reconstructed

`intro_second_camera_target` implements the independent target spawned by the
second flyby at `44:FE82`. The source path `44:FB2E..FB4B` selects this actor
once, waits 17 updates, installs pitch 20/yaw 226/speed 60 with a five-unit
speed approach toward zero, waits 27 updates, then installs pitch zero/yaw
236/speed 30 for 40 updates before a persistent zero-speed hold. Inherited
position and roll are preserved; the held actor is not retired.

The shared path movement assembly at `7F:9DE8..9E1C` applies the speed approach
before movement on the same update that installs it. Its descending comparison
retains the approach when speed lands exactly on zero; only the following
overshoot clears it. This one-update distinction is retained explicitly in
native state. The later speed-30 segment consequently does not keep slowing.

Verification executes the original QuickSpawn command and constructor, then
isolates the created target and compares its complete path for 400 updates
across 16 inherited poses, including signed-coordinate boundaries and eight
combined rotations. Checks cover position, velocity, each rotation axis,
speed, approach state, camera identity, path continuation and absence of an
End/removal request. Three ROM-free tests cover first-frame deceleration,
exact-zero versus overshoot and persistent-hold idempotence. This proves the
target in isolation, not the surrounding craft's choreography or rendering.

The second flyby's statically decoded graph has 299 reachable commands with
no decoder failures, including its recursive attached actors, animation,
auxiliary services and departures. The remaining children must still
be translated and composed with the parent; this graph count is reachability evidence, not
a completion certificate.

## Later flyby authored placements and attached trail

`intro_second_flyby` now contains the four typed placements used by the parent
and its departing craft: source indices 16–19 from `44:BDAA..BDCE`, backed by
the six position/rotation tables at `07:FECC`, `07:FF0A`, `07:FF48`, `07:FF86`,
`07:FFA5` and `07:FFC4`. All six pose channels are replaced by that helper,
including roll; this differs from the formation helper, which leaves roll
alone. The tests check each channel directly against the original tables.

The attached trail spawned at `44:FE58` is a separate native actor with shape
119, local offset `(0, 30, -984)` and the source depth offset 5. Its script
`44:FF8D..FF97` adds 100 to local depth twenty times, then Ends in the same
update as the final increment. World pose publication belongs to the parent's
earlier attachment pass, so local motion does not immediately redraw a new
world pose. The completed actor neither republishes nor restarts.

Four focused comparisons execute the original spawn command, constructor,
parent waiting loop and child path with stationary/rotating and ordinary/
wrapping parent inputs. They verify all local/world pose channels, shape,
depth policy, path continuation and End timing. A ROM-free test additionally
protects birth-before-publication and terminal idempotence. Birth ordering in
the complete naturally scheduled flyby still needs scene-level verification.

Static review of the inline code at `44:FDDC` and `44:F9A1` identifies an
additional required relationship: both write the current actor as the newly
spawned child's secondary link (original child field `1C`), independently of
the primary attachment link. `intro_chain` now preserves both relationships
as typed identities; treating every link as the attachment owner would change
subsequent linked-object selection.

## Later flyby parent choreography reconstructed

`intro_second_flyby_craft::OpeningSecondFlybyCraft` translates the complete
parent path `44:FDC2..FF43`: initial rocking, exact camera-cue gates, counted
pitch/yaw/roll maneuvers, animation, speed approach, authored pose replacements
and the persistent final hold. Counted-loop boundaries execute the final body
and following commands in the same update. Speed approach retains its step
for one additional update when subtraction reaches exactly zero.

Ordered typed events describe six child constructions, two parent camera
selections, child-control initialization and pitch settling, and seven sound
markers. Attached construction preserves separate attachment groups and the
chain's secondary parent relationship. Independent camera/wing constructions
inherit the pose at the command, before common motion. The first sound kind
uses the class-two spatial service; the remaining kinds use direct queuing.

The focused parent comparison constructs this actor through the original root
path, then executes its original strategy for 600 updates across five cue
schedules and three listener configurations. It checks all pose and velocity
channels, speed/approach state, animation, continuations, ordered child
construction and attachment metadata, camera selection, child controls and
ordered audio payloads. Three ROM-free tests protect immediate-cut ordering,
missing-cue behavior and the completed path's persistent hold.

This is a parent-only comparison: children are constructed by the original
code but their strategies are deliberately not executed. Recursive attached
children, formation destruction composition and native scene
scheduling/rendering remain unfinished. The shipping intro still uses recorded
frames; this component does not establish a completed native intro or SF2 port.

## Later flyby wings and destruction lifetimes reconstructed

`intro_second_flyby_wings` implements the attached wing at `44:FF45..FF62`
and independent departing wing at `44:FF63..FF8C`, with typed actor/parent
identities. The original spawn commands at `44:FEA8` and `44:FEC4` construct
both with shape 89. The first actor waits 27 updates under parent publication,
then unlinks attachment group 6 and drifts `(5, 0, 40)` while rotating eighteen
times. Its final counted body configures the departure auxiliary effect and
zeros velocity before motion. The second actor replaces its inherited pose
using placement 19, enables its trail, turns while hidden and then visible,
and rolls away. It recomputes flight velocity after each turn, including the
last update that requests destruction.

Static handler review is important here: opcode `04` enables per-update flight
velocity recalculation; it is not a one-update wait. The two auxiliary calls
also differ: the attached wing invokes `07:B746` with range zero, while the
departing wing invokes `07:B6EF` with range one (low-byte doubled to two).
Neither path's final health write is an immediate hide or End.

`OpeningWingSequence` composes both paths with the existing common destruction
consumer through final effect cleanup. Initial and recursive effect birth
timing is explicit: allocation behind a live parent can miss this traversal,
whereas a dying list-head actor's children can run immediately. Insufficient
allocation capacity is an explicit error and preserves the pending handoff.

Sixteen original-spawn/actor-list comparisons cover both paths, rotating and
wrapping parent poses, frozen/unfrozen auxiliary state and existing ownership.
They check position, rotation, velocity, local attachment data, detachment,
visibility, trail state, path continuation and the next-update destruction
boundary. Eight more comparisons cover complete lifetimes, both allocation
birth timings, scroll compensation, ordered explosion audio and final removal.
Independent construction checks all inherited position and rotation channels.
Three ROM-free tests protect boundary timing, terminal idempotence and capacity
failure. These isolated families do not yet compose the entire flyby scene;
the full scheduler and rendering remain required. The native chain family is
described below.

## Linked-chain geometry and constructor boundaries

`intro_motion::follow_intro_predecessor` implements the chain's source follower
operation at `7F:BD13..BDC6`, used at `44:F863` and in its following loop.
It computes pitch and yaw from the pre-move position, then places the follower
at its signed depth along the predecessor's pitch and yaw. The two source
rotation helpers (`7F:3A4E` and `7F:38A9`) use byte-sized multiplication and
intermediate results, followed by an eight-unit scale and wrapping position
addition. This is not the ordinary attachment matrix or the wider integer
rotation kernel. Follower roll survives; predecessor roll is ignored.

The constructor graph `44:F831/F83D -> F965 -> F9AE` increments a shared
counter on the main craft, giving nine segments their ordinals. Their primary
owner remains the main craft, their secondary link names their predecessor,
and each segment selects itself as its transform owner. Child insertion after
the current segment allows all nine to initialize in one actor-list update.
The tail uses shape 342; the other eight use shape 340. Authored local depths
are -11 for the first segment and -25 for the remainder.

`sf2_intro_chain` checks those constructor/link/shape/depth assertions through
16 real actor-list updates and compares the native follower kernel through
the subsequent nine-segment traversals. A separate 25,600-case comparison
executes the original authored follower command over angular, signed-depth,
wrapped-coordinate and coincident-position boundaries. Two ROM-free tests
protect pre-move facing, roll ownership, byte overflow and quantization.
Those primitive checks are supplemented by the native family below; they do
not on their own establish whole-scene integration.

## Native linked-chain lifecycle

`intro_chain::OpeningChainFamily` implements the recursive nine-segment family
and its departure sprites. Callers reserve nine distinct identities; only the
head is initially live, and the other segments initialize in predecessor
order during the first family update. Initial parent publication can reach
the head before its constructor; later children begin unpublished. Thereafter
each segment owns its world pose while retaining separate main-craft and
predecessor identities.

The source path `44:F85A..F8A9` hides each segment for one update, then reveals
it with health 15, contact classification, trail style and a persistent
zero-health response. InvisibleOn/Off independently disable/enable contact,
superseding the optional initial contact-suppression branch. Sort override is
sampled once on reveal; following controls execute in source order: predecessor
geometry, depth-offset change, three pitch-settling steps, three per-part bank
steps plus yaw, then optional pitch leveling. Departure takes precedence.

The auxiliary entry created at `7F:C395 -> 2360` is ordinary contact payload
1, consumed by `0D:DF90..DFAD`, not a visual allocation. The callback registered
at `44:F876` uses condition 12 (health zero), not a twelve-update timer. Its
source dispatcher at `7F:9AA8/9CEB` restores health 10 and disables contact.
`set_health_at_strategy_entry` explicitly requires the enclosing engine to
have selected the strategy: this response does not bypass the engine's earlier
common-destruction routing at `7F:35B5`.

Departure `44:F8AA..F8E4` cancels that response before the trigger pass, faces
the main craft, captures signed speed 216 with fourfold velocity, and draws
three random angles from the shared scene generator. Each part then performs
`20 - ordinal` world-motion/spin bodies. Local offset drift is separate and
omitted on the final End update. The final world pose is inherited by a shape
11 burst executing `44:C520..C532`: eight color-animation updates, fixed size
bias 32, zero size delta. QuickSpawn's actual insertion after the current
segment (`7F:91B3..91CB`) runs that burst in the same traversal. Retirement is
reconciled with the original separate cleanup pass (`7F:402D`).

Allocation is not silently ignored. An empty pool enters the original
diagnostic at `00:8032`; the native family returns a capacity error without
consuming its pending update. Consuming the final slot invokes the source
effect-retirement sweep (`7F:2979..29BB`), which omits the actual list-tail
actor. The caller supplies whether the oldest burst occupies that boundary;
the returned pressure event also permits handling other eligible scene actors.

Thirty-nine original-constructor/update/cleanup scenarios cover six control
schedules, fixed/wrapping poses, three random seeds, repeated zero-health
callbacks, cancellation on departure, both pressure-sweep tail boundaries,
and the expected exhaustion diagnostic. Four ROM-free tests protect birth,
sampling/precedence, persistent callbacks, retirement order, capacity recovery
and terminal idempotence. Scene-wide allocation, collision routing and rendering
integration remain separate required work; the shipping front end is still
recorded rather than this native scene.

## Reproduce without manual play

From the repository root, with the user-owned SF2 ROM present:

```bash
SF2_INTRO_TRACE_STOP=4560 uv run python tools/sf2/run_mesen_oracle.py \
  tools/sf2/mesen_intro_scene_trace.lua --timeout 60 --quiet
```

Use the printed `MESEN_SCRIPT_DATA` directory for the following trace argument:

```bash
uv run python tools/sf2/disasm/extract_intro_paths.py \
  /path/to/sf2_intro_scene_trace.txt \
  --installations /path/to/sf2_intro_scene_installations.txt --summary
uv run python -m unittest discover -s tools/sf2/disasm -p 'test_extract*py'
uv run python tools/sf2/disasm/extract_intro_controller.py --scene 6
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_second_camera_target'
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_second_flyby'
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_second_flyby_craft'
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_second_flyby_wings'
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_chain'
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_motion --test sf2_intro_camera --test sf2_intro_controller --test sf2_intro_root --test sf2_intro_flyby --test sf2_intro_free_craft --test sf2_intro_destruction --test sf2_intro_late_target --test sf2_intro_attached_craft --test sf2_intro_formation --test sf2_intro_attachment --test sf2_intro_target --test sf2_intro_logo --test sf2_intro_logo_actor --test sf2_intro_logo_attachment'
nix develop --command bash -c 'cd rust && cargo test -p sf2-game && cargo test -p sf-app --bin starfox-hd-rs && cargo test -p sf-render --lib && cargo test -p sf-render --test gl_runtime'
```

Clipping extraction and source/live plane verification:

```bash
uv run python -m unittest discover -s tools/sf2 -p test_extract_shapes.py
uv run python tools/sf2/run_mesen_oracle.py \
  tools/sf2/mesen_logo_draw_policy_oracle.lua --timeout 30 --quiet
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_clipping'
```

To check the fresh Mesen output as well as the committed fixture, set
`SF2_CLIPPING_TRACE` to its printed script-data directory's
`logo_clipping_planes.csv` for the final command. The verifier requires a
complete default 800-frame capture and checks every row.

Unknown control flow, missing observed cursors, or changed source signatures
fail the graph check. `--require-reviewed-semantics` additionally rejects
uncatalogued handlers; it currently fails intentionally. No observed-cursor
filter or ignore-failures option is provided. ROM-backed extraction tests
require the local ROM. The new GPU checks require an actual adapter.

## Remaining completion gates

1. Recover the intro's typed actor/camera/effect systems and source-authored
   choreography. Preserve original discrete gameplay timing and interpolate
   presentation only. Do not substitute sampled poses, frame blending, image
   upscaling or an emulated machine for native scene reconstruction.
2. Recover separate background layers, sprites, text, fades and clipping from
   source data. Do not let a full-screen reference texture hide the native
   geometry pass.
3. Replace one complete scene at a time and verify source-time object identity,
   poses, cameras, ordered effects/audio events and rendered composition. Add
   tests for skipped intros, scene cuts, resolution and multiple interpolation
   fractions, not just an unattended captured frame.
4. Apply the same gate to all remaining captured front-end and ending tracks.
   Move full-scene recordings to verification-only resources as each production
   dependency is eliminated. Keep legitimate original 2D artwork separate.
5. Exercise playable missions and input variants independently of these scene
   gates. Native Rust and a successful boot do not certify feature parity.

No fully-HD SF2 completion claim is warranted until these gates pass.
