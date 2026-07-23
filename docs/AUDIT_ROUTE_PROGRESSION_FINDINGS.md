# Route / Level-End / Warp Progression Audit (ROM vs Rust port)

READ-ONLY audit of the "which planet/boss next" map-flow logic in the Rust
port against the SNES 65816 ASM. Scope: level-end (`levelfinished`/`LE_*`)
dispatch, the branching route/warp tree, planet-select advance, and the
death/continue restart target.

- ROM sources: `reference/ultrastarfox/SF/`
- Rust sources: `rust/sf-game/src/{shell.rs,planets.rs,world.rs}`,
  `rust/sf-map/src/{catalog.rs,levels/*}`

Verdict up front: **FIXED (tick 198).** Route/branch *data* tables were already
faithful; `shell::le` + `warp_advance` now dispatch all six warp codes (skip
tally, fire `routechange*`), `level1_5` uses `mapend(7)`, and blackhole
enter/exit strats set `levelfinished`. Accepted leftovers: `nebula_on` as path
id (#4), game-over via `GF_PLAYERDEAD` (#5).

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

### FINDING 1 (CRITICAL — warp-falls-through): ~~shell never dispatches on the `LE_*` value~~
**FIXED (verified tick 198):** `shell::le` constants + `gameplay_progress_tick`
match on warp codes → `warp_advance` (skip tally); normal → `enter_tally`.
Tests `shell::tests::{bhole_exit_codes_repoint_routes3,special_code_repoints_*,enterbhole_*,enterspec_*}`.

### FINDING 2 (CRITICAL — missing-branch): ~~`routechange*` callbacks are never fired~~
**FIXED (verified tick 198):** `warp_advance` fires `routechangebhole1/2/3`,
`routechange1` (SPECIAL/ENTERSPEC), and `routechange2` (ENTERBHOLE). Strat
`blackhole2_strat` sets `levelfinished=15`; shell arms P21 on dispatch.
Tests shell warp suite + `blackhole.rs` (8).

### FINDING 3 (MEDIUM — wrong value in map data): ~~`level1_5` emits `mapend(6)`~~
**FIXED (verified tick 198):** `b.mapend(7)` (`le_startgame`). Test
`level1_5::tests::mapend_sets_levelfinished_le_startgame`.

### FINDING 4 (LOW — representation drift): `nebula_on` value differs from ROM

ROM `routechange1_l` stores `nebula_on = stagepaths.path22-stagepaths` (a byte
*offset* into the table, `PLANETS.ASM:3110-3112`). Rust `routechange1` stores
`nebula_on = path_id::P22` (the enum value 22, `planets.rs:277`). `nebula_on`
is used downstream only as a background/render flag, so the exact magnitude is
unlikely to matter, but it is not a faithful mirror. Confirm the consumer
treats it as boolean before relying on this.

- ASM ref: `PLANETS.ASM:3110-3112`.
- Rust ref: `planets.rs:276-278`.
- Status: **ACCEPTED** (boolean consumer; no gameplay divergence observed).

### FINDING 5 (INFO — divergent-but-equivalent): game-over path does not use `levelfinished`
**ACCEPTED** — GF_PLAYERDEAD path is equivalent; defensive `le::GAMEOVER` arm
also present in `gameplay_progress_tick`.

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

- Findings: **5** total — Criticals #1–#2 + Medium #3 **FIXED (tick 198)**;
  Low #4 ACCEPTED (`nebula_on` as path id); Info #5 ACCEPTED (GF_PLAYERDEAD).
- Route/branch *data* + *LE_* dispatch + blackhole enter/exit strats are
  verified. **AUDIT_ROUTE_PROGRESSION closed** (accepted leftovers only).

**Warp destinations (now correct):**

1. `LE_ENTERBHOLE` (15) → BLACK HOLE via `routechange2` (P21) on dispatch.
2. `LE_ENTERSPEC` (16) → SPECIAL via `routechange1` (P22).
3. `LE_BHOLE1/2/3` (11-13) → routes[3] = P19/P18/P20; `LE_SPECIAL` (14) → P22.
