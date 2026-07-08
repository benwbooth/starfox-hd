# Route / Level-End / Warp Progression Audit (ROM vs Rust port)

READ-ONLY audit of the "which planet/boss next" map-flow logic in the Rust
port against the SNES 65816 ASM. Scope: level-end (`levelfinished`/`LE_*`)
dispatch, the branching route/warp tree, planet-select advance, and the
death/continue restart target.

- ROM sources: `reference/ultrastarfox/SF/`
- Rust sources: `rust/sf-game/src/{shell.rs,planets.rs,world.rs}`,
  `rust/sf-map/src/{catalog.rs,levels/*}`

Verdict up front: **the route/branch *data* tables are a faithful port, but
the level-end *dispatch* is missing entirely.** The Rust shell treats every
non-zero `levelfinished` value identically ("advance the normal route"), so
all six warp/branch level-end codes (`LE_BHOLE1/2/3`, `LE_SPECIAL`,
`LE_ENTERBHOLE`, `LE_ENTERSPEC`) fall through to the normal next stage. The
`routechange*` callbacks that rewrite the branch tree exist in Rust but are
never invoked.

---

## 1. `LE_*` level-end value table

Definitions: `reference/ultrastarfox/SF/INC/KALCS.INC:91-103`.
Dispatch: `reference/ultrastarfox/SF/ASM/MAIN.ASM:222-298`.

Note the ROM ordering in MAIN.ASM: `LE_GAMEOVER` is handled *first* (no stage
inc); then `inc stage` runs for **every** other level-end (MAIN.ASM:229);
then the value is dispatched.

| Val | Name | Set by | ROM handler (MAIN.ASM) | Next-stage effect |
|----|------|--------|------------------------|-------------------|
| 0 | (playing) | — | `lbeq gameloop2` (:224) | keep playing |
| 1 | (normal end) | `mapend` no-arg (MAPMACS.INC:274) | falls through to `end_level_seq` (:253) | tally, `inc stage`, `planetseq_l` — normal next node |
| 4 | `le_fadetowhite` | end-of-game text seq | `exitspec.white` fade (:276) | fade to white then planetseq |
| 5 | `le_fadedown` | — | `exitspec.dofadedown` (:238) | fade down then planetseq (no tally) |
| 6 | `le_endofgame` | end_game credits path | `end_game_seq` (:264) | ending sequence |
| 7 | `le_startgame` | `mapend__not` (MAPMACS.INC:1990) | `jml gamestart` (:257-260) | Venom-surface hand-off (self-contained loop macro) |
| 8 | `le_endofcreds` | credits map | (credits/MAIN.ASM:553) | end of credits |
| 9 | `le_endtotalscore` | end_game_seq | (MAIN.ASM:519) | total-score screen |
| 10 | `le_gameover` | continue/death path | `gameover_l` (:226-227) | GAME OVER screen, then `planetseq_l` — **no stage inc** |
| 11 | `le_bhole1` | `bholeexit1_istrat` (KSTRATS.ASM:680) | `exittobhole1`: `routechange bhole1` -> `routes[3]=P19`, then `enterbhole` (:306-322) | black-hole spits you out at **P10 = Venom 1 Orbital** |
| 12 | `le_bhole2` | `bholeexit2_istrat` (KSTRATS.ASM:683) | `exittobhole2`: `routes[3]=P18` (:308) | black-hole -> **P4 = Sector Y** |
| 13 | `le_bhole3` | `bholeexit3_istrat` (KSTRATS.ASM:686) | `exittobhole3`: `routes[3]=P20` (:310) | black-hole -> **P14 = Sector Z** |
| 14 | `le_special` | (special-stage route change) | `exittospecial`: `routechange 1` -> `routes[0]=P22` + `nebula_on` (:312) | route re-points to **P22 -> OTHEREND = Out of This Dimension** |
| 15 | `le_enterbhole` | `blackholeexit_Istrat` via GA2STRAT.ASM:2202-2203 (preceded by `routechange 2` -> `routes[1]=P21`) | `enterbhole`: store 101 in `specbuf`, `planetseq_l` (:314-322) | **enter the BLACK HOLE stage** (map=`_blackhole`) via the P21 branch |
| 16 | `le_enterspec` | `PATHDATA.ASM:375` (`P_SET pbyte1,le_enterspec`) | `exitspec.white` fade, `planetseq_l` (:250-251) | **enter the SPECIAL stage** (Out of This Dimension) |

Key ROM semantics the port must eventually honor:
- Warp codes 11-16 **skip `end_level_seq` (the tally)** and jump straight to
  `planetseq_l`. Normal (1) and end codes run the tally first.
- `SOUND.ASM:382-390` silences near-object SFX for codes 15,16,11,12,13 — an
  independent confirmation that these five are the "warp in progress" set.
- Codes 11-14 change `routes[]` *before* re-walking; that rewrite is what
  redirects the branch tree.

---

## 2. Route / branch tree (the Star Fox path graph)

ROM table `stagepaths` (`PLANETS.ASM:3695-3898`); walker
`drawplanetlines`/`.chkformore` (`PLANETS.ASM:2535-2637`); route roots
(`PLANETS.ASM:3696-3698`); `routes[]` init (`PLANETS.ASM:218-226`).

The walker starts at a *route root* (selected by `whichroute` after
`convertroute`), then follows `PATHNEXT` (direct) / `PATHCHOICE n` (indirect
via `routes[path_n>>1]`) links `stage` times.

`routes[]` slots (4 words) and their init defaults:

| slot | init | fed to | changed by (map/strat callback) |
|------|------|--------|---------------------------------|
| routes[0] | P12 | `PATHCHOICE 1` (from P11, Venom-3 spine) | `routechange1` -> P22 (+`nebula_on`) |
| routes[1] | P7 | `PATHCHOICE 2` (from P6, Corneria-1 spine) | `routechange2` -> P21 |
| routes[2] | P2 | `PATHCHOICE 3` (from P1, Corneria-2 spine) | `routechange3` -> P17 |
| routes[3] | P19 | `PATHCHOICE 4` (from P17 / P21 hidden branches) | `routechange bhole1/2/3` -> P19/P18/P20 |

Three main routes (post-`convertroute`: gameplay whichroute 0<->1 swap):

- **Route root P6 (Corneria 1 / "route 0"):** P6 -> routes[1]=P7 -> P8 -> P9 -> P10 -> END1
  = Corneria1, Asteroid Belt1, Space Armada, Meteor, Venom1 Orbital, Venom1.
- **Route root P1 (Corneria 2 / "route 1"):** P1 -> routes[2]=P2 -> P3 -> P4 -> P5 -> END2
  = Corneria2, Sector X, Titania, Sector Y, Venom2 Orbital, Venom2 (Highway).
- **Route root P11 (Venom 3 / "route 2"):** P11 -> routes[0]=P12 -> P13 -> P14 -> P15 -> P16 -> END3
  = Venom3, Asteroid Belt3, Fortuna, Sector Z, Macbeth, Venom3 Orbital, Venom3.

Warp branches (only reachable after a `routechange*`):

- **Black-hole entry** (from Asteroid Belt on route 0): `routechange2`
  swaps routes[1] P7->**P21**, so P6 -> P21 -> `PATHCHOICE 4`(routes[3]) ->
  BLACKHOLE stage; the black-hole exit choice sets routes[3] to
  P18 (->Sector Y), **P19 (->Venom 1 Orbital, default)**, or P20 (->Sector Z).
- **Special-stage entry** ("Out of This Dimension"): `routechange1` swaps
  routes[0] P12->**P22**, so the Venom-3 spine detours P11 -> P22 -> OTHEREND
  = SPECIAL map, planet 14, msg 115.
- `P17` is the hidden Sector-X branch fed by `routechange3` (routes[2]->P17).

### ROM vs Rust route table — node-by-node

Rust table: `rust/sf-game/src/planets.rs:104-135` (`STAGE_PATHS`), roots
`planets.rs:99-100`, walker `drawplanetlines` `planets.rs:299-337`.

Every node matches the ROM: planet id, map id, peppermsg, currentlevel,
next-type, choice slot, next-path. Spot verification:

| Path | ROM (`PLANETS.ASM`) | Rust (`planets.rs`) | Match |
|------|---------------------|---------------------|-------|
| P1  | planet 4→conv 0, 2_1, msg88, lvl1, PATHCHOICE 3 | `node(0,M2_1,88,1,RouteChoice,2,-)` | ✓ |
| P6  | 1_1, msg88, lvl0, PATHCHOICE 2 | `node(0,M1_1,88,0,RouteChoice,1,-)` | ✓ |
| P11 | 3_1, msg108, lvl2, PATHCHOICE 1 | `node(0,M3_1,108,2,RouteChoice,0,-)` | ✓ |
| P17 | 2_2, msg89, lvl1, PATHCHOICE 4 | `node(3,M2_2,89,1,RouteChoice,3,-)` | ✓ |
| P18 | blackhole, msg113, PATHNEXT P4 | `node(10,BLACKHOLE,113,0,Direct,-,P4)` | ✓ |
| P19 | blackhole, PATHNEXT P10 | `node(10,BLACKHOLE,113,0,Direct,-,P10)` | ✓ |
| P20 | blackhole, PATHNEXT P14 | `node(10,BLACKHOLE,113,0,Direct,-,P14)` | ✓ |
| P21 | 1_2, msg89, PATHCHOICE 4 | `node(2,M1_2,89,0,RouteChoice,3,-)` | ✓ |
| P22 | 3_2, msg109, PATHNEXT .otherend | `node(1,M3_2,109,2,Direct,-,OTHEREND)` | ✓ |
| OTHEREND | 0,0,14,_special,115,0 | `node(14,SPECIAL,115,0,None,-,-)` | ✓ |

`routechange*` callbacks (`planets.rs:275-293`) also match ROM
`PLANETS.ASM:3107-3155` exactly (routes[0]=P22+nebula, routes[1]=P21,
routes[2]=P17, routes[3]=P19/P18/P20).

**Conclusion: the branch tree data is correct. The defect is entirely in the
control flow that consumes it.**

---

## 3. Findings

### FINDING 1 (CRITICAL — warp-falls-through): shell never dispatches on the `LE_*` value

`rust/sf-game/src/shell.rs:861` gates level completion on
`if self.game.world.levelfinished != 0` and then unconditionally runs
`enter_tally()` -> `advance_stage_after_tally()`
(`shell.rs:869, 910-943`), which does `stage += 1; drawplanetlines()`. There
is **no `match` on the value**. Consequently:

- `LE_ENTERBHOLE` (15), `LE_ENTERSPEC` (16), `LE_BHOLE1/2/3` (11-13),
  `LE_SPECIAL` (14) all take the normal-route path instead of warping.
- The warp codes should *skip the tally* and go straight to the map walk
  (ROM `enterbhole`/`exitspec` jump to `planetseq_l` bypassing
  `end_level_seq`); the port always shows the tally.
- `LE_GAMEOVER` (10) would (if ever set via `levelfinished`) wrongly do a
  stage-advance instead of a GAME OVER screen — but the port routes game-over
  through `GF_PLAYERDEAD` -> `GameState::Continue` (shell.rs:840-852) instead,
  so this is latent, not active (see Finding 5).

There is no `LE_*` constant enum anywhere in the Rust port (grep of
`sf-game`/`sf-map` finds only the `LEVELFINISHED` WRAM address, not the value
enum), so the map builder's `mapend(N)` value is discarded past its boolean
truthiness.

- ASM ref: `MAIN.ASM:222-322`, `KALCS.INC:91-103`.
- Rust ref: `shell.rs:857-873` (`gameplay_progress_tick`), `shell.rs:910-943`.
- Fix: add an `LE_*` value enum (mirror KALCS.INC). In
  `gameplay_progress_tick`, `match world.levelfinished`:
  - `LE_BHOLE1/2/3` -> call `planets.routechangebhole1/2/3()`, then
    `stage += 1`, re-walk (no tally), begin gameplay — this is the black-hole
    *exit* choosing the destination.
  - `LE_SPECIAL` -> `planets.routechange1()`, `stage += 1`, re-walk, begin.
  - `LE_ENTERBHOLE` -> `stage += 1`, re-walk (no tally), begin (the P21 branch
    must already be active; see Finding 2), set the `specbuf=101` /
    black-hole-anim equivalent.
  - `LE_ENTERSPEC` -> `stage += 1`, re-walk (no tally, fade-to-white), begin.
  - default (1) / end codes -> current tally + advance path.
  The existing `// TODO(C parity)` note at `shell.rs:858-860` already flags
  this exact gap.

### FINDING 2 (CRITICAL — missing-branch): `routechange*` callbacks are never fired

The six `routechange*` methods (`planets.rs:275-293`) are correct but their
own doc comment says *"Not yet reachable — the map lane has not registered
ROUTECHANGE native callbacks in sf-map"*. In the ROM these are triggered by:

- `routechange 2` inside the black-hole-approach strat, immediately before it
  sets `levelfinished=LE_ENTERBHOLE` (`GA2STRAT.ASM:2202-2203`). This swaps
  routes[1] P7->P21 so the walk detours into the BLACKHOLE stage.
- `routechange bhole1/2/3` inside MAIN's `exittobhole*` handlers
  (`MAIN.ASM:306-311`), driven by `LE_BHOLE1/2/3` from the black-hole exit
  strats.
- `routechange 1` inside `exittospecial` (`MAIN.ASM:312`).

Because none fire, `routes[]` keeps its init defaults forever
(`planets.rs:222-235`: routes[1]=P7, routes[0]=P12, routes[3]=P19), so **even
if Finding 1 were fixed, `LE_ENTERBHOLE` on route 0 would still walk into
Space Armada (P8), not the black hole**, since the P21 branch is never armed.

- ASM ref: `GA2STRAT.ASM:2202-2203`, `MAIN.ASM:306-312`,
  `PLANETS.ASM:3107-3155`.
- Rust ref: `planets.rs:272-293` (callbacks), `shell.rs` (no caller).
- Fix: (a) fire `routechangebhole1/2/3` and `routechange1` from the shell's
  `LE_BHOLE*`/`LE_SPECIAL` dispatch (Finding 1); (b) wire `routechange2` (and
  `routechange3` for the Sector-X branch) from the black-hole-approach strat
  once sf-strat lands, mirroring the `routechange` that precedes the
  `LE_ENTERBHOLE` store. This is blocked on sf-strat (the black-hole strats
  `bholeexit*_istrat`, `blackholeexit_Istrat` in KSTRATS.ASM and the GA2STRAT
  approach are unported) and on sf-path (`LE_ENTERSPEC` is set from
  PATHDATA.ASM:375, an unported path callback).

### FINDING 3 (MEDIUM — wrong value in map data): `level1_5` emits `mapend(6)` instead of 7

Both ROM `LEVEL1_5.ASM:13` and `LEVEL2_5.ASM:9` use `mapend__not`, which sets
`levelfinished = 7` (`MAPMACS.INC:1989-1990`). The port matches for 2_5
(`rust/sf-map/src/levels/route2/level2_5.rs:19` -> `b.mapend(7)`) but **1_5
uses the wrong code**: `rust/sf-map/src/levels/route1/level1_5.rs:82`
-> `b.mapend(6)` with a comment "sets levelfinished=6". `6` is `LE_ENDOFGAME`;
the correct value is `7` (`LE_STARTGAME`).

Currently harmless (Finding 1 makes all non-zero values equivalent), but it
will send the Venom-1-orbital -> Venom-1-surface transition to the *ending
sequence* the moment `LE_*` dispatch is implemented.

- ASM ref: `LEVEL1_5.ASM:13`, `MAPMACS.INC:1989-1990`.
- Rust ref: `rust/sf-map/src/levels/route1/level1_5.rs:81-82`.
- Fix: `b.mapend(7)` (and correct the comment).

### FINDING 4 (LOW — representation drift): `nebula_on` value differs from ROM

ROM `routechange1_l` stores `nebula_on = stagepaths.path22-stagepaths` (a byte
*offset* into the table, `PLANETS.ASM:3110-3112`). Rust `routechange1` stores
`nebula_on = path_id::P22` (the enum value 22, `planets.rs:277`). `nebula_on`
is used downstream only as a background/render flag, so the exact magnitude is
unlikely to matter, but it is not a faithful mirror. Confirm the consumer
treats it as boolean before relying on this.

- ASM ref: `PLANETS.ASM:3110-3112`.
- Rust ref: `planets.rs:276-278`.
- Fix: verify the `nebula_on` reader; if it compares against a specific value,
  reconcile the representation.

### FINDING 5 (INFO — divergent-but-equivalent): game-over path does not use `levelfinished`

ROM reaches GAME OVER via `levelfinished = LE_GAMEOVER` (10) checked at
`MAIN.ASM:226`; the value is set on the death/continue path (CONT.ASM:220
compares it). The Rust port instead drives game-over through the
`GF_PLAYERDEAD` gameflag: `shell.rs:840-852` counts `DEATH_RESPAWN_TICKS`,
then reloads the current map if `lives>0` or enters `GameState::Continue` if
not. This is a different mechanism but reaches the same states. **Checkpoint/
continue restart target is correct**: both death-respawn (`shell.rs:846-848`)
and continue (`shell.rs:472-479`, refill lives) call
`begin_gameplay_from_planet_select()`, which preserves `stage`/`whichroute`/
`newmap` and reloads the current stage — matching the ROM's per-stage restart.
No action required unless a map/strat is found that sets
`levelfinished=LE_GAMEOVER` directly (none in the ported set).

---

## 4. Verified correct

- **Route/branch tree data**: `STAGE_PATHS` (`planets.rs:104-135`), route
  roots (`planets.rs:99-100`), and the walker's PATHNEXT/PATHCHOICE traversal
  (`planets.rs:299-337`) match ROM `stagepaths` + `.chkformore`
  (`PLANETS.ASM:2543-2637, 3695-3898`) node-for-node (see §2 table).
- **`routes[]` init defaults**: P12/P7/P2/P19 (`planets.rs:222-227`) ==
  ROM `PLANETS.ASM:219-226`.
- **`routechange*` targets**: all six (`planets.rs:275-293`) == ROM
  `PLANETS.ASM:3107-3155` (routes and slots correct; only `nebula_on`
  representation drifts — Finding 4).
- **`convertroute` 0<->1 swap** (`planets.rs:264-270`) == ROM
  `PLANETS.ASM:3159-3171`; applied on planetseq entry and again before
  gamestart (shell.rs:933-935 brackets the walk with two converts, matching
  `PLANETS.ASM:251` + `:1090`).
- **Planet-select navigation**: left/up = prev, right/down = next, wrap 0..3
  (`planets.rs:403-418`) == ROM `PLANETS.ASM:463-482`; launch on START/A/B
  resets stage and converts back (`planets.rs:420-429`) == ROM.
- **`inc stage` on level clear** (`shell.rs:925`) == ROM `MAIN.ASM:229`
  (for the normal path; the warp paths need their own stage handling per
  Finding 1).
- **Normal-route ordering** verified by `planets.rs` tests
  (`route1_walk_matches_c_table`, `black_hole_reroute`) and `shell.rs` test
  `level_clear_advances_stage` (1_1 -> 1_2). Route 0 spine
  Corneria1 -> AsteroidBelt1 -> SpaceArmada -> Meteor -> Venom1Orbital -> Venom1
  is correct.
- **Death/continue restart target**: current stage preserved (Finding 5).

---

## Summary

- Findings: **5** total — 2 critical (warp dispatch missing; routechange
  callbacks never fire), 1 medium (wrong `mapend` code in 1_5), 1 low
  (`nebula_on` representation), 1 info (game-over via gameflag, equivalent).
- Route/branch *data* is a faithful, verified port; the *dispatch* layer that
  consumes `LE_*` is absent. Both critical findings are the same root cause
  and largely blocked on sf-strat/sf-path (the warp triggers are unported).

**Top 3 wrong destinations (all stem from Finding 1 — every warp code falls
through to the normal next stage):**

1. `LE_ENTERBHOLE` (15): should **enter the BLACK HOLE stage** (via the P21
   branch armed by `routechange2`); instead advances to the normal next node
   (e.g. on route 0, Asteroid Belt 1 -> Space Armada). Compounded by Finding 2
   (P21 branch never armed).
2. `LE_ENTERSPEC` (16): should **enter the SPECIAL stage** ("Out of This
   Dimension", map `SPECIAL`/planet 14); instead advances normally.
3. `LE_BHOLE1/2/3` (11-13): the black-hole *exit* should redirect routes[3] to
   Venom 1 Orbital (P19) / Sector Y (P18) / Sector Z (P20); instead ignored,
   so the black-hole exit destination is never selectable. `LE_SPECIAL` (14)
   shares this failure (should point routes[0] -> P22 -> Out of This Dimension).
