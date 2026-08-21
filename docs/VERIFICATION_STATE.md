# Retail verification replay — live state

**Test:** `cargo test -p sf-oracle --test semantic_trace
retail_front_end_and_corneria_opening_match_native_semantic_state`
(boot → attract → title → briefing → Corneria, retail ROM vs port,
per-tick LevelSnapshot + RNG equality).

## Current frontier

First divergence: **tick 1744** (game_frame 851).

The map wave spawning at z≈−25505 via the `robotswithlog` path chain
lands with different x/y on the port:

| slot | shape    | retail birth        | native birth        |
|------|----------|---------------------|---------------------|
| 14   | carrier  | (1172,0,−25505)     | same                |
| 30   | robot_0  | (1012,0,−25505)     | (1152,0,−25505)     |
| 23   | robot_0  | (1332,0,−25505)     | (1192,0,−25505)     |
| 31   | pillar3_ns | (1356,−200,−25481)| (1195,−25,−25502)   |

Carrier matches; children spread ±160 x in retail vs ±20 native (8×),
and the carried pillar sits far off in native.

## Resume checklist

1. `P_SPAWN` macro = `[x,y,z,xrot,yrot,zrot,shape,path,hp,ap,link]`
   (PATHMACS.ASM:1152); payload stores coords **/4**, interpreter scales
   ×4 after full rotation (`path_add_rotated_offset`, scale_shift=2).
   `robotswithlog` spawns robots with z offsets −90/+90 (payload −23/+23).
2. Children also walk immediately (`P_SETVEL 20` inside robotwithlog2 /
   probot), and they run their first tick on the spawn pass
   (insert-after-current). Birth-tick position = spawn point + first
   self-move; both sides must be compared mid-frame, not just end-frame.
3. `carriedlog` imports its shape from gword1 (`P_IMPORT`) and defaults
   to pillar3_ns; check gword1 export/import (`P_EXPORT`) in the adapter.
4. Suspect list: rotation frame of P_SPAWN offsets (parent roty≈180°?),
   scale application, and first-walk direction after `P_SETVEL`.

## Fixed en route to 1744 (all pushed)

- 5b783bd death-cascade/scheduling (delayexplode kill_obj semantics,
  zaco3die double add_playerZ, pillar3 inline delayremove tail, smoke
  shape 357 + insert-after + init fall-through).
- d57aa4e path-lane hit_flash: real partner-AP do_coll under the
  FRAMESPERAP cooldown (retail ship ap=8 kills the tower on ram).
- 5309c25 tow0explode immediate pillarexplode chain; Corneria
  mapwait decimal transcription (0800→800, 2000→2000);
  semantic_trace direct words robot_0 $BB9C→420 / pillar3_ns
  $B882→452, catalog scan widened to 512.

## Backlog surfaced by this arc

- `route3/level3_5.rs` still carries hex-vs-decimal transcription
  misreads of the same class (mapwait/mapobj literals like 0x0700 vs
  ASM decimal 700). Audit every MAP*.ASM ↔ levels/*.rs pair.
- ea_parity fixtures are pacing-sensitive through level boot; re-bless
  (SF_BLESS_FIXTURES=1) whenever map timing or early strat behavior
  changes, then eyeball the fixture diff for sanity.

## Known-failing hand-written asserts (pre-existing, bisected)

Three `sf-strat/tests/enemies_ground.rs` firing tests fail since the
same-frame spawn-scheduling commit b2e0a6f (codex WIP), NOT since any
of this arc's commits — bisected via worktrees at 8230b8c (green) vs
b2e0a6f (red):

- meteo0_fires_homing_laser_on_notdelay_gate
- szaco0_fires_at_the_fire_waypoint
- szaco5_fires_and_advances_when_in_range

These drive one strategy tick through `Game::call_strat` directly (no
run_strategies loop) and assert a just-fired weapon exists afterwards.
Under same-frame scheduling the fired laser's own init/first-tick now
runs inside the fire call; whether the laser is then legitimately
elsewhere/dead or genuinely broken needs a per-case ASM check
(GASTRATS laser inits + remove_offscn/lifecnt semantics) before either
fixing code or updating the asserts. Do NOT bless these blindly — they
are behavioral assertions, not captured traces.
