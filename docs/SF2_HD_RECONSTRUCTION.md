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

A fresh **4,560-video-frame** neutral capture exercises the full retained
attract duration. It contains 167 observed path cursors, of which 20 are not
reachable from the single recovered intro root (the `$B6F7..B868` family and
`$DB2E`). The completion check correctly exits with failure for this corpus.
The same indexed installer has 24 distinct path roots across its 30 records;
recovering later scene handoffs is necessary, not something to hide by seeding
the graph with arbitrary sampled offsets. An exploratory traversal of all 24
table roots reaches 3,826 commands and 19 still-unreviewed inline blocks.

## Reproduce without manual play

From the repository root, with the user-owned SF2 ROM present:

```bash
SF2_INTRO_TRACE_STOP=4560 uv run python tools/sf2/run_mesen_oracle.py \
  tools/sf2/mesen_intro_scene_trace.lua --timeout 60 --quiet
```

Use the printed `MESEN_SCRIPT_DATA` directory for the following trace argument:

```bash
uv run python tools/sf2/disasm/extract_intro_paths.py \
  /path/to/sf2_intro_scene_trace.txt --summary
uv run python -m unittest discover -s tools/sf2/disasm -p 'test_extract*py'
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
