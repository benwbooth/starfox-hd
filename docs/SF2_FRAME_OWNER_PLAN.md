# SF2 production frame owner — plan

Written 2026-10-07 after the five authored scenes (3, 4, 5, 9, 25) were
verified at path level. Status: **not started**. This is the gap between
"source-accurate parts" and "a game that plays".

## Where things stand

- `Game::tick` (`rust/sf2-game/src/native/game.rs`, ~37,600 lines) is the
  shipping owner. It holds one hand-written field group per encounter or boss
  (Mirage Dragon, Eladard, Titania, Macbeth, Fortuna, rival fighters, ...) and
  drives them with recorded schedules: keyframed camera/player poses for
  re-engagement, generated projectile spawn/retire frames, frame-indexed rival
  flight plans, a prescribed opening route. It does not use the shared actor
  scheduler, the authored path catalog or the scene installer
  (see `SF2_GAMEPLAY_COVERAGE.md`, "Confirmed production gaps").
- The source-accurate parts exist beside it and are verified against the
  original code:
  - actor pass: `strategy_schedule.rs` (`$7F:34E7..367F`), hosted by
    `scene_strategy.rs` (`SceneActors`, `SceneExecution`);
  - path VM and the 160-root catalog (`authored_paths.rs`);
  - indexed scene entry (`scene_install.rs`): scenes 3, 4, 5, 6, 7, 9, 25;
  - the indexed-scene player strategy (`player_scene_entry.rs`,
    `$06:83F1..84BD`): entry reset, install, wait, shared action tail;
  - player services (`player_*.rs`): input, action stream, motion, camera,
    weapons, status, palette, entry/motion reset, post-motion publication;
  - collision passes, common destruction, retirement, sound routing.
- Not yet composed into a per-frame owner: the player main strategy as a whole
  (`$06:9C27` prefix is composed in `player_visit.rs`; the mode dispatcher and
  the rest of the frame order are not), the render hand-off, and the non-path
  actor strategies (enemy/boss strategies are still `Game` special cases).

## First slice status (2026-10-08)

`scene_runner.rs` (`SceneRunner`) now composes a scene frame natively:
collision queue (`$7F:32A1`), primary palette, background scroll, the
strategy epoch with per-visit listener markers, then the ordered collision
pass (`$7F:402D`: deferred retirement, latch roll, detection) that the
source runs during rendering.
`sf-oracle/tests/support/sf2_attract_scene.rs` boots the retail machine to the
attract loop's scenes six and seven, reads each scene's starting state once,
and then runs both engines independently: all 445 and 825 game frames match
at every epoch (every actor's pose, health, list order, the view, action
gate, exit request and RNG).

Findings that shaped it:
- The render-timed entropy refresh (`$7F:058F`, one extra RNG draw, usually
  once per frame, sometimes 0 or 2) can land inside an actor's visit. The
  runner takes its position as draw indices (`EntropyRefresh`); the oracle
  supplies the observed indices. Shipping uses `AfterPass`.
- The excluded actor (14D6) is the source's cue-marker proxy (`$07:B8CA`
  poses it); native markers are typed values, so its pose is not modeled.
- Detection still tests a queued actor that the pass's cleanup just retired,
  from its stale slot; `ObjectStore` keeps the freed record until reuse.
- The fixed view lives outside the source pool; natively it is listed last
  and suspended so insertion after the head and scheduling match.
- Not yet ported for this slice: the scene loader's palette upload
  (`$7F:0A76` DMA from the `$03:C7E8..` load lists), the attract map scripts
  (`$05:FBE7`, `$05:8035`, ...), and the scene-player initializer `$06:82F9`
  (entry at `$06:845C` is ported as `SceneEntryPhase::ClearLaunchCounts`).

## Attract loop status (2026-10-08, later)

The attract loop (scene 6 at map `$05:FBE7`, scene 7 at `$05:8035`,
repeating) now runs entirely natively: `attract_stage.rs` provides the boot
world, the map's scene-player spawn (`$05:8003`), the stage hand-over stores,
the scene selection records, the frame-loop reseed (`$03:8A62`) and the
scene-six view reset (`$03:BF71`); `player_scene_init.rs` ports the
initializer `$06:82F9`. The oracle runs 6 → 7 → 6 from the native boot
world, carrying the world between scenes, and matches every epoch. Not yet
native: the scene loaders' palette and artwork uploads (load `0x8D` =
`$03:C80B`, load `0x03` = `$03:C7B3`, uploaded during the first frames), the
bank-0B sprite/script system whose script ends scene 7 (`INC $1BE4` at
`$0B:E293`, polled by `$03:C2B0`), and the title stage machine (`$03:BC82`
dispatch on 1B76). Shipping still plays the recorded intro video
(`sf-render/src/sf2_intro.rs`); replacing it needs those presentation pieces
plus a 3D render path for the intro mode.

Note: the hand-built `intro_*.rs` OpeningScene reconstructs this same scene
six (its ignored retail test fails at update 101 on entropy timing, which the
draw-indexed `EntropyRefresh` input resolves). It is not used by shipping.

## Star Wolf interception status (2026-10-08, latest)

The Star Wolf interception (location 7, layout 0A) runs natively against
the retail machine from the scene player's initializer until the stage loop
leaves after the player is shot down: every actor and player record, the
frame's display services (`render_view.rs`, `hud_target.rs`), the flight
stage controller with its clocks and exit fade (`stage_controller.rs`,
`$03:C500..C79D`, `$7F:5F39`, `$03:E0FC`), the render-time fade and blank
hold, and the bank-0B presentation director's stage-start iris
(`presentation_director.rs`). The oracle replays the controller, fade and
director visits in retail order because the fade runs per completed render.

Still compared with masks: the HUD service's buffer parity (1B9C bit 01),
the HUD lane (record 1FD2, run by `$04:93AC` -> `$0B:A842`) after it starts,
and the retained-shield acknowledgement of the HUD gauge (`$04:95CB`).

Path to a shipping stage (replacing the recorded Leon duel in `game.rs`):
1. Mission launch `$03:B90E` (location table `$03:BA17`, map `$05:E995` for
   location 7) and the stage loop prologue `$03:BE74` up to the first frame.
2. Stage-start inputs from the campaign: nearly all seeds are zero, campaign
   values (pilot, shield, score, equipment, input settings) or encounter
   values written before launch (D744, D766, D77C/D, D7A1, D7D2, D7F4, 1D74,
   1DE0, 1E08/9); their producers are on the strategic map.
3. `Game` hosts a `SceneRunner` for the visit: live pad input, camera from the
   fixed view, render objects from the runner's store, HUD inputs from the
   world, and the stage exit (`StageVisit::Leave`) mapped to the campaign.
4. The HUD service `$04:8301` (mostly drawing) and the bank-0B sprite system
   for in-stage presentation.

## Smallest coherent first slice

Run one installed scene end to end on the shared scheduler, with a live
player, and expose it through `Game`:

1. `SceneFrame` (new module): owns `ObjectStore`, `ScenePathWorld`,
   `SceneExecution`, catalog; `tick(controller)` = input prep, strategy epoch
   (`begin`, `run_overlapping`, render readiness, `run_remainder`), retirement.
2. Assigned-strategy callbacks for the behaviors these scenes use (player
   main, view, path follow/movement). Everything else must fault visibly.
3. A test that installs scene 9 and ticks it for the full action length with
   scripted pad input, comparing against the original per frame (the harness in
   `sf-oracle/tests/support/sf2_scene_*.rs` already does this without the
   player and view; the player is skipped there).
4. Replace the matching recorded-cutscene branch in `Game` and delete the
   recording it used.

## The original frame order (bank 03, `$03:8040..8160`)

The mission frame is a fixed call sequence; a native `SceneFrame` must follow
it. Normal path, in order: `$07:BD46`, `$07:EA67` (player palette, ported),
`$03:B0C3`, `$07:AA8C`, `$07:A326`, `$7F:539C` (radar region), `$04:8301`,
`$03:8FC9`, `$0D:D5FA`, then the strategy epoch `$7F:34E7` (overlapping half,
ported in `strategy_schedule.rs`) and `$7F:354A` (remainder, ported), then
`$7F:11BC`, `$7F:7B33` and a bank-03 service at `$03:820C`. The alternate paths
(`$03:809F` and `$03:80D5`) add `$7F:32A1`, `$7F:7980`, `$07:A337`, `$03:D87D`,
`$7F:1118`, `$04:FCC8`, `$07:950E`, `$7F:79F8`, `$7F:148D`, `$7F:7918` and
`$7F:11C6` around the same epoch. Entries with no Rust annotation at their
address (several may be ported under another name; confirm before porting):
`$07:BD46 $03:B0C3 $07:AA8C $07:A326 $04:8301 $03:8FC9 $0D:D5FA $7F:11BC
$7F:7B33 $03:820C $7F:32A1 $7F:7980 $07:A337 $03:D87D $7F:1118 $04:FCC8
$07:950E $7F:79F8 $7F:148D $7F:7918 $7F:11C6`.

Ported so far: `$03:B0C3` (background scroll, `frame_background.rs`) and
`$0D:D5FA` with its region scan `$0D:D95B` and group retirement `$0D:D8DD`
(`map_streaming.rs`). The streaming port reproduces a source quirk: its
second "unchanged cell" test compares the first view's position with the
second view's saved cell. Records with attached data (flag bit 2, `$7F:2360`)
fault until that attachment is ported. The map-loader producers (opcodes
`$90` records, `$94` regions) are not yet wired to the native map.

The attract loop alternates indexed scenes 6 and 7; both rows, their action
streams and the player scene strategy are now verified against the original
code. Scene 6 needed six new path operations (selected-player pose copies,
published-motion export, D767 shape hand-off, relative-offset drain, the
script-value link swap) and two reviewed opcodes (0x145, 0x146).

During cutscenes the player runs the action-only strategies
(`Behavior::PlayerActionWait`, ported), so a first scene runner does not need
the flight-mode dispatcher (`$06:9D09`, mode table at `$06:9D1A`).

## Inputs the shipping game must start publishing

Campaign phase (1BE0), encounter signals (D77D), projected-camera word (1E3C),
live scene selection (1D73), cue listener identity (CF1F vs 12C3/033F),
background scroll writes (1E4E, 193A), campaign variant cells (1C06/1C07),
plus everything already listed as `Option` world inputs in `ScenePathWorld`
(absent means fault, never a default).

## Known limits to keep visible

- Indexed scene table: 7 of 30 entries; others fault after allocation.
- The two closed-gate player exits (`$06:8525`, `$06:84EF`) fault as
  unported after their leading writes.
- Scene six parks an attachment link in its script-value word; the native
  link is typed and faults if a nonzero number would be reinterpreted.
- Past-the-end selectors in bank-end lookup tables fault (e.g. wingmate 255 in
  scene 3) because the original reads mutable low-bank memory there.
- Scene 25's removal of child 20 is a reviewed no-op (child never created).
- Per-visit checks cover path/object state, pools, random bytes, sound queue
  and the listed publications, not rendered output.

## Order of work after the first slice

Remaining 25 scene rows; enemy/boss strategies as native `assigned` handlers
(replace per-boss `Game` fields); live control branch (remove the
neutral/steered split); campaign state machine (remove the prescribed route);
then presentation and release checks per `PORT_COMPLETION_PLAN.md` items 5-8.
