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
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_motion --test sf2_intro_logo --test sf2_intro_logo_actor --test sf2_intro_logo_attachment'
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
