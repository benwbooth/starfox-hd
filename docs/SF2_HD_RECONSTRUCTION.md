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

This does **not** yet reconstruct the full logo presentation. Actor setup,
material/visibility scheduling, the outline's additional child, sweep motion,
release-driven dispersal and scene camera remain to be integrated. These
controllers are not called by the shipping scene scheduler yet, so the
application still displays its recorded intro. Passing these tests is a
bounded source-behavior result, not an HD-intro or full-SF2 completion claim.

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
nix develop --command bash -c 'cd rust && cargo test -p sf-oracle --test sf2_intro_motion --test sf2_intro_logo'
nix develop --command bash -c 'cd rust && cargo test -p sf2-game && cargo test -p sf-app --bin starfox-hd-rs && cargo test -p sf-render --lib && cargo test -p sf-render --test gl_runtime'
```

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
