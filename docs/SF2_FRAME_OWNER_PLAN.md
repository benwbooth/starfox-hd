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
  - indexed scene entry (`scene_install.rs`): scenes 3, 4, 5, 9, 25 only;
  - player services (`player_*.rs`): input, action stream, motion, camera,
    weapons, status, palette, entry/motion reset, post-motion publication;
  - collision passes, common destruction, retirement, sound routing.
- Not yet composed into a per-frame owner: the player main strategy as a whole
  (`$06:9C27` prefix is composed in `player_visit.rs`; the mode dispatcher and
  the rest of the frame order are not), the render hand-off, and the non-path
  actor strategies (enemy/boss strategies are still `Game` special cases).

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

## Inputs the shipping game must start publishing

Campaign phase (1BE0), encounter signals (D77D), projected-camera word (1E3C),
live scene selection (1D73), cue listener identity (CF1F vs 12C3/033F),
background scroll writes (1E4E, 193A), campaign variant cells (1C06/1C07),
plus everything already listed as `Option` world inputs in `ScenePathWorld`
(absent means fault, never a default).

## Known limits to keep visible

- Indexed scene table: 5 of 30 entries; others fault after allocation.
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
