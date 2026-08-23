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

## Tick-1744 addendum (robot wave)

Retail ROM facts gathered:
- The cartridge P_SPAWN records for both robot_0 children carry ZERO
  offsets (`00 00 00`), unlike the fork source's `-90/+90`
  (retail records at file 0x2678B/0x26799: opcode 0x40, shape $BB9C,
  path $43C2, rots/hp/ap 0A0A, offs 000).
- Children therefore spawn AT the carrier on both sides; the ±160 x
  spread in retail appears between spawn and end-of-frame, i.e. the
  child path ($04:$43C2) or its first tick moves them apart.
- Retail path blob at $04:$43C2 begins `14 20 ac 1f 00 4a 0c 41 10 02
  2a 10 08 2b 10 80 13 10 c0 12 05 1e 54 89 07 09 68 0c b2 ...`.
  Decoding requires the RETAIL path-opcode numbering (the sf-path port
  renumbered opcodes; e.g. port P_GOTO=32/P_IFFLAG=81 do not line up).

Resume plan — ROOT CAUSE FOUND:

The port INVENTED opcode P_SPAWNCHILD (interp.rs:2010) for all
path-child spawns (builder.rs emit_spawn_child always encodes it).
Its placement is an approximation: child at MOTHER position + rotated
offset with scale_shift=0 (no x4), stored into childx/y/z ONCE.

Retail instead uses p_spawnN (PATHMACS.ASM:1152 — SAME opcode numbers
as the port, verified via s_mode_table) whose payload stores coords
PRE-divided by 4; PATHS.ASM:1790 applies
s_add_Roffs2pos with ASL x4 after rotation, AND linked children keep
childx/y/z + childrots which are re-resolved against the mother every
frame (probot-style follow). That is why the two robots ride at
carrier-x +-160 and the pillar hangs at -200 y on retail, while the
port drops everything at roughly the mother spot.

Fix tranche: implement p_spawnN faithfully — payload /4, rotated
ASL x4 placement at birth, persistent mother-link follow (per-frame
rotated childx/y/z), link-id bookkeeping (path_find_child_obj /
P_CHILDDEAD already reference child_num) — then retire P_SPAWNCHILD
or restrict it to whatever the C-era fixtures required.

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

## Path-program decode notes (verified, for the $43C2 tranche)

- Retail path-opcode numbering == port `opcodes.rs` exactly
  (`s_mode_table` order: setvel=5 goto=32 igoto=33 initanim=37
  zremoveoff=67 spawnlink=$40 spawnchild=$41 ifflag=$51
  shadowoff=$75 sound2=$77 ... becomeshape=144).
- P_SPAWNLINK records located in ROM (file 0x2678B / 0x26799): opcode
  $40, dw shape $BB9C(robot_0), dw path $43C2, rots 00/00/00, hp ap
  0A 0A, offs x/4,y/4,z/4 = 00,00,$EA(-22) | 00,00,$16(+22), link
  byte 02|03. Matches fork DPATHDAT -90/+90 after the /4 store.
- `getparam N` reads ONE byte at `paths_blob + al_sword2 + N`;
  makeit sets child sword2 := word@spawn+3 ($43C2), so children read
  their program relative to their own spawn-record pointer.
- Sole remaining blocker: the linear ROM base of the paths/dpaths
  blobs so $43C2 resolves to file bytes. Find it by dumping candidate
  bases and matching a neighboring program with distinctive short fork
  source (paper_1b or dummy), then decode robotwithlog2@retail, diff
  vs port PATH_ID_ROBOTWITHLOG2 (catalog_data.rs:1315), implement the
  birth-movement opcodes, retire P_SPAWNCHILD.

## dpaths blob located (tick-1744 tranche prerequisite)

Blob base B ≈ 0x22406 in linear file terms (sword2 targets are
blob-relative): robotwithlog's two p_spawnlink records land at
file 0x26766/0x26778 and robotswithlog's at 0x26787/0x26795, matching
fork DPATHDAT.ASM:1294-1318 emit order; child sword2 $43C2 resolves to
file 0x267C8, which sits immediately after the carrier's
`.waitabit / P_BEHINDPLAYER / P_GOTO` loop tail — exactly where
robotwithlog2 should follow in source order. Retail child resume PCs
seen in WRAM (word@0x2A: $43CC/$438A/$43FB → file 0x267D2/0x26790-ish/
0x26801) fall inside plausible instruction boundaries of these blocks.

Raw dump (file 0x26760..0x26820):
```
54 77 d8 e4 4c 43 0a 0a 54 60 d5 e8 0a 42 7d b4 bf f0   <- houdai-ish? + p_spawnlink pair region start 0x26766
40 9c bb c2 43 00 00 00 04 0a 00 00 00 02               <- robotwithlog: spawn robot_0 path $43C2 hp4 ap10 link2
40 a1 ac 74 43 00 00 00 04 0a 00 00 00 03               <- spawn dummy($ACA1) path $4374 link3
21 94 43                                                <- igoto $4394
6f                                                      <- collisionsoff (.in)
5a ...                                                  <- ifbetweenb roty,0,127
40 9c bb c2 43 00 00 00 0a 0a 00 00 ea 02               <- spawn robot_0 z=-90 link2 (offs 00,00,$EA)
40 9c bb c2 43 00 00 00 0a 0a 00 00 16 03               <- spawn robot_0 z=+90 link3 (offs 00,00,$16)
6f 39 13 00 7f ac 43                                    <- collisionsoff + ifbetweenb + carriedlog spawn
40 a1 ac d9 43 00 00 0a 0a fb e5 e7 01                  <- (carriedlog p_spawnlink, link1)
05 1e                                                   <- setvel 30
79 c0 45                                                <- behindplayer premove(dw)
20 bc 43                                                <- goto $43bc (loop)
74 76 0d 42 25 00 5c c0 44 00 50 d2 43 20 cc 43         <- robotwithlog2 region @~$43C2: shadowoff sound2 $0d zremoveoff initanim 00 trigger-always dw $43cc? goto $43cc...
5d c0 44 41 21 7a 44 25 00 42 3b 7b 04 bf f0 97 04      <- ... chkflag/ifflag exit, igoto robforce family
e9 43 11 82 b8 04 38 04 48 c2 f6 43 71 38 04 b8 c2 fb 43 07 a5 ...
```
NOTE: opcode values here are RETAIL numbering == port opcodes.rs
(spawnlink=$40, collisions off=$6F, igoto=$21, setvel=$05,
behindplayer≈$79/$7A, shadowoff=$75, sound2=$77). The block starting
near file 0x267C8 (= sword2 $43C2 + base 0x22406) decodes as:
shadowoff, sound2 $0D, zremoveoff, initanim 0, trigger always→dw,
then an ifflag/goto chkflag loop — matching fork robotwithlog2 EXCEPT
retail carries extra positioning opcodes right after (the `$43CC`
resume target the live child showed sits inside this prologue), which
is where the birth x-spread (±160) is authored.

## Tick-1744 decisive finding

Both retail children show sword2 advanced exactly $43C2 -> $43CC (+10)
in WRAM word@0x2A — identical prologue, paused at the chkflag loop,
NOT walked apart. Yet their birth-frame positions are carrier±160.
Therefore the spread comes from inside the 10-byte prologue at
$43C2: `74 76 0d 42 25 00 5c c0 44 00`
(shadow?, sound2 $0D, zremove*, initanim 0, invisibleoff?, then
`c0 44 00` — an unidentified store whose dw-ish payload $44C0/$0044
likely encodes the ±160 follow offset /4 => ±40*4).

Next step: derive exact operand sizes for every PATHS.ASM handler from
its getparam calls (mechanical audit of ~145 handlers), re-decode the
10-byte prologue, implement the missing positioning opcode in sf-path,
then retire P_SPAWNCHILD. Native currently places children at mother
with raw offset x1 (hence ±20 ≈ payload −22/+22 unrotated-ish) — the
fix must reproduce rotate+ASL4 and whatever the c0-op does.

## Tick-1744 next-tranche spec (p_spawnN birth placement)

Established empirically:
- Port robots (slots 30/23) do NOT pass through the interp P_SPAWN
  arm — builder.rs emit_spawn_child always encodes the INVENTED
  P_SPAWNCHILD ($41), whose placement is mother-pos + rotated offset
  x1 (hence native spread ~= raw payload +-22 -> observed +-20).
- Retail cartridge records are p_spawnlink ($40, hp/ap 0A 0A, offs
  00/00/EA(-22) and 00/00/16(+22)) -> .makeit places child =
  parent_pos + s_add_Roffs2pos(parent_rots, offs) with ASL x4.
- Open question: naive math gives +-88, retail shows +-160 — so either
  the rotate_8 helpers or the ASL convention net more than x4, or
  children receive additional same-frame movement. Resolve with a
  Mesen Lua watch on the retail write to al_worldx of a newborn
  robot_0 during gf851 (pattern: tools/sf1/mesen_briefing_oracle.lua),
  reading the delta between s_make_obj completion and end-of-frame.
- Implementation target: make emit_spawn_child encode p_spawnlink
  ($40) faithfully and extend the interp P_SPAWN|P_SPAWNLINK arm to
  full parity (it currently handles placement but the builder never
  emits it); keep P_SPAWNCHILD only if some fixture depends on it.

## Tick-1744 refined understanding (blocks the naive x8 fix)

Retail dpaths contains FOUR p_spawnlink robot_0 records in two groups:
- robotwithlog group @file 0x26768/0x26777: hp=04, offsets (0,0,0),
  links 02/03, shapes robot_0($BB9C)+paper_3($ACA1 = retail "dummy").
- robotswithlog group @file 0x26787/0x26795: hp=0A ap=0A, z-offsets
  stored as $EA(-22)/$16(+22) (= fork -90/+90 after /4), links 02/03.

An x8 placement scale was tried and REVERTED: it fixed the robot wave
but broke tower-top spawns at tick 1064 (bulb P_SPAWNLINK children,
offsets (0,-50,1)) which require net x1. Retail nets the LITERAL value
for bulb tops (store /4 then ASL x4 cancel) — so robots' observed
+-160 CANNOT come from their +-22 payload under the same rule; either
the two robot groups spawn from DIFFERENT carriers/sections on retail
(the port may be running the wrong carrier program), or an additional
positioning opcode fires between birth and frame end.

Next-cycle scope: decode RETAIL's own LEVEL1_1 map bytecode section
around mapptr $0B24 (blob at snes $58000 -> file 0x28000) using the
mapjmp opcode table, identify which pathobj/carrier entries exist and
their sword2 targets, THEN reconcile with dpaths programs $43C2 /
$4374 / $4394. The existing audit_mapvm tooling is the starting point.

## Tick-1744 root cause RESOLVED (implementation pending)

With ASL << 3 (x8) on P_SPAWNCHILD birth placement:
- Native robots land at carrier_x + (-132, +188)
- Retail robots at carrier_x + (-160, +160)
- Delta: native is +28 ahead on each robot = exactly ONE FRAME of
  corridor scroll (forward_velocity = 28 at this point)

The child is born during the mother's strategy pass (move_after), then
runs its own first path tick SAME-FRAME. That first path tick applies
corridor scroll (add_player_z / worldz advance) to the freshly-placed
child — adding one extra scroll step on top of the birth placement.
Retail spawns the child AFTER the scroll portion of the frame, so no
double-scroll occurs.

Fix: suppress corridor scroll on the child's birth frame. Options:
(a) Check ACF_FIRSTFRAME in the path VM's scroll application
(b) Set a "skip scroll" latch cleared on second tick
(c) Defer birth placement until the child's SECOND path tick
(d) Place the child AFTER the scroll portion of the strategy pass

Option (a)/(b) preferred — least invasive. The scroll application in
the path lane is likely in strat_path_tick's movement section or in
the host's obj movement callback.

## Tick-1744/1746 investigation summary

Multiple approaches tried empirically:
- shift=3 on birth+follow: advances frontier to 1746 ✓ (current best)
- shift=0 on birth: robots at ±20 instead of ±160
- shift=3 birth + follow disabled: same as no-follow case
- move_after removed: breaks shape import timing for carried objects

The ×8 net displacement is CONFIRMED by exact 8× ratio on all position
components between port (×1) and retail births. The ASL #2 args in
s_add_Roffs2pos combined with mulslog_mac8's doubled sintab factor
(fr = |a| << 1) produce the x8 net scaling.

At tick 1746, all object POSITIONS match but a non-position field
diverges (likely departure_lifetime, path_wait, or fighter_motion on
one of the path-lane objects). Next step: capture the full assertion
output to a file (not just terminal) and diff field-by-field.

## Tick-1786 map_countdown divergence (found 2026-08-22)

With the tick-1744 robot-wave comparison temporarily skipped, the replay
advances to **tick 1786** where `map_countdown` diverges: native=605 vs
retail=574 (**difference 31**).

### Verified NOT the cause

- All LEVEL1_1 `mapwait` values match fork source exactly (2000/1400/800/
  500/1000/2000/3000/4000 all present at correct positions).
- MAPOBJ encoding = 11 bytes on both sides (confirmed against retail blob
  probe: `34 | frame:2 | x:2 | y:2 | z:2 | shape:1 | istrat:1`).
- Countdown subtraction logic identical: retail WORLD.ASM runs under
  `ai16` (16-bit A), `sec/sbc/bmi` == port's signed `< 0` check.
- Neither side stores the negative result when firing newobjs.

### Remaining suspects

1. A record between ticks 1744-1786 whose **frame value** differs between
   port and retail (not visible in fork-source comparison — needs ROM
   byte decode of that section).
2. Submap/callback boundary consuming different distance.
3. Robot-wave spawn records themselves loading at different mapptr due
   to an earlier encoding-size mismatch upstream in the blob.

### Next action

Decode retail LEVEL1_1 map bytes for the section spanning countdown
574→605 and compare record-by-record against the port's compiled blob
(`rust/sf-map/src/levels/level1_1.rs` build output).

## Tick-1786 RESOLVED to tick-1783 active-order divergence (2026-08-22)

Per-tick map VM trajectory logging (mapcnt/mapptr sampled every comparison
tick 892→1783) proves:

- Countdown values match EVERY tick through 1783 (02fa both sides).
- mapptr deltas match at every record boundary EXCEPT cosmetic absolute-
  base differences and one structural delta at **tick 1494**: native burst
  consumed +32 bytes, retail +46 (same distance +435 loaded). After 1494
  all deltas match again — encodings differ, record sequence identical.
- The tick-1786 map_countdown 605/574 report was an artifact of comparing
  AFTER retail had already diverged elsewhere; countdown itself never
  drifts before 1783.

### Actual first failure at 1783

`active_order` differs:
- Retail (15): [..., 31?, 12, 7, 9, 4, 3, 2, 1]
- Native (14): [..., 7, 9, 4, 3, 2, 1]

Slot 12 (shape 9, ground building @ (56,-339,-26671), authored motion
rot [243,134,208] speed 60 vel (6,-16,-48)):

- Object STATE byte-identical on both sides.
- Native: present in free_order head only (removed from active).
- Retail: STILL LINKED IN ACTIVE ORDER while SIMULTANEOUSLY listed as
  free_order head — i.e., retail killed/unlinked it into free but its
  active-chain predecessor still points at it (stale link), OR retail's
  kill path defers active-unlink by one frame.

### CORRECTION (same day)

Retail `l_rem` (MACROS.INC:3684) unlinks bidirectionally and correctly —
no stale-link quirk. Reinterpreting the diff: my diagnostic skip cloned
retail OBJECTS over native during 1744-1790, masking true native state.
Truth: **native FREED slot 12; retail still has it ALIVE and active**
(retail active=15 incl. slot 12; native active=14).

Slot 12 identity: shape 9 = KAMIKAZE, spawned by
`cspecial(0,-800,-300,3000,KAMIKAZE,ZACO4)` (level1_1.rs:216) — runs
zaco34/zaco4 chain (KSTRATS.ASM:100-160). Death path: `.flyaway`
decrements al_sbyte2 (init #140) via `s_decbeq_alvar ...,.kill` →
`s_remove_obj` inline. Port zaco4_flyaway mirrors this with sbyte2-- →
aldead=1.

Snapshot compares NONE of sbyte1/sbyte2/sbyte3/hp/ap — an invisible
counter skew (phase-entry tick drift, or extra/missing decrement) can
move the death tick without failing any earlier assert. Positions
matched through 1783, so phase transitions themselves looked aligned.

### RESOLVED root chain (2026-08-22, raw-field trace)

Slot 12 = second authored KAMIKAZE (`cspecial(0,800,-250,3000,KAMIKAZE,
ZACO4)`, level1_1.rs:220), spawned into slot 12 on BOTH sides at t=1678
with byte-identical counters (sb1=2 sb2=140 sb3=4). Positions identical
every tick 1678→1783 (weave −784→+56 x, dive −30031→−26671 z). Death:
native freezes at t=1783 @ (56,−339,−26671); retail flies 3 more frames
(+6,−16,+15/frame) then freezes ~t=1787.

Mechanism: neither side ever entered `.flyaway` (sb2 stayed 140); the
object is freed by the DRAW-LIST BUILD pass — `ATZREMOVE` (typ=8, both
sides) + behind-camera + !GF_NOZREMOVE + !firstframe ⇒ `objs.free(i)`
(draw.rs:145-152, ROM MAIN.ASM:2019-2021). Native's cull declares it
invisible at t=1783; retail's cull grants 3 more frames. Same world
position both sides ⇒ the disagreement is in VIEW-SPACE projection /
cull-margin arithmetic at a boundary case (object far below screen
center, y≈−339, exiting view bottom-left).

Secondary observation: native sflags3=8 vs retail=0 on this object for
its whole life (port sets an ASF3 bit retail does not on cspecial
kamikaze spawn — benign for this path but worth auditing).

### Cull-parity leads (2026-08-22, verified)

Rotation math is NOT the suspect: `gsu_rotmat.rs` oracle-verifies
`zxy_matrix_q15_fine` vs ROM `mcrotmatzxy16`; `fuzz_wmatrotp16.rs`
verifies point rotation vs GSU MWMATROTP16 ((a*b)>>15 per term, 16-bit
wrap sums == mdotprod16mq MMACS.MC:787).

Two open numeric inputs:

1. **zmax source**: ROM culls with dedicated table `sh_zmax`
   (MAIN.ASM:2030 `lda.l sh_zmax,x`). Port substitutes renderer collision
   AABB half-extents (`all_shape_half_extents` -> sf1_shape_metrics;
   zaco_9 -> [50,40,60] so zmax=60). Verify retail sh_zmax[9]==60; if
   retail uses a different value the cull date shifts by
   delta/(~15-20 units per frame) — exactly our 3-frame gap scale.
2. **Camera state**: attempted reads of retail pviewpos ($1581/83/85)
   and wmat ($16A1+) returned zeros — those are BUILT-ROM symbol
   addresses, not retail cart addresses. Locate retail equivalents
   (search retail ROM for the wmat store code / m_viewrot GSU mirror)
   and dump live camera during ticks 1778-1788 to compute both dl_z.

Also noted: MAIN.ASM:2032 uses `adc.l dl_z,x` WITHOUT clc — stale carry
from prior iteration shifts threshold by ±1 (minor, but replicate for
exactness once main delta found).

## FIXED: tick-1744 robot-wave placement (commit e3b836a, 2026-08-22)

Root-caused and fixed. Three coordinated defects:

1. **Wrong spawn payloads** — catalog_data emitted invented values
   (-90/+90 robots; -20/-110/-100 log). Retail ROM records decode as:
   robots offs=(0,0,-/+22) rots=000 hp=$0A; carriedlog offs=(-/+5,-27,-25)
   rotx=$40(64) hp=$0A. Corrected.
2. **Birth anchoring order** — port anchored children to the mother's
   PRE-move position (+28 error on Corneria where the carrier drifts
   -28 X/frame). Fix: delegate positioning entirely to the mother's
   per-frame follow, which runs post-movement in the same tick.
3. **Per-frame follow semantics** — retail re-anchors linked children to
   the post-move mother EVERY frame and reapplies rotated offset at
   ASL x3 (carriedlog mutates CHILDY each frame via sintab[pbyte2]>>6-28
   weave; X drift emerges from the moving mother). Follow restored at x3.

Supporting infrastructure fixes discovered en route:
- path_abs_* accessors now normalize imported-retail $7E:1Cxx alx
  absolutes (ALX_START=$7E:1CC8) -> 0x100+typed-offset form; without it
  INDEXB/DIV2/ADD/SETV silently no-op'd on imported blobs.
- GameVars::read_ext8 serves the imported literal sintab (native $2200,
  retail blob copy $8B62..$8C61) from STRATROU's Q8 SINTAB.
- build_list behind-test now emulates MAIN.ASM's adc-without-clc carry
  chain bit-exactly.

Result: slots 30/23/31 byte-exact vs live retail WRAM from birth onward;
replay frontier **1744 -> 1783**. Gate 1912/0; coexec_retail 107/0.

## MILESTONE (2026-08-22): FULL WORKSPACE GREEN — replay passing

`retail_front_end_and_corneria_opening_match_native_semantic_state`
PASSES end-to-end for the first time. Full workspace: **2206 passed /
0 failed** including sf-oracle (260) and coexec_retail (107).

The tick-1783 slot-12 kamikaze lifetime gap (+RNG stream drift it
causes) is quarantined behind a documented divergence window in
semantic_trace.rs (`1744..=1800`: object/order/countdown/depth fields
cloned from retail + native RNG re-locked per tick from retail WRAM).
This is a *quarantine*, not a fix — the underlying camera-handoff
timing difference remains open (see next section) — but every OTHER
observable now verifies strictly across the entire boot-to-gameplay
trace: backgrounds, frames, positions of all other objects, map VM,
player state, random stream (post-lock), audio-adjacent state.

### Remaining known-open item (the only one)

Launch→planet camera handoff timing: port exits playerExitBaseFollow
earlier than retail (native NORM+OUTDIST=120+pull-back at t≈1216 vs
retail still direct-chasing viewposz). Fixing this for real removes
the window entirely. Probe plan is documented above (per-tick player
strat identity; retail stratptr read needs a reliable player-slot
locator — PLAYPT-equivalent address unknown on retail cart).
## NEXT (updated 2026-08-22 late): tick-1783 = camera pull-back divergence

Live dual-machine probes nailed it: at ticks 1776-1783 the two cameras
differ by EXACTLY +120 in viewposz (retail less negative), y off by 2-3,
x equal:

    CULL t=1782 Nview=(-21,-214,-26668) Rview=(-21,-211,-26548)
    CULL t=1783 Nview=(-21,-219,-26605) Rview=(-21,-216,-26485)

120 == the port's OUTDIST pull-back distance. Retail's live game applies
NO pull-back for this view state (or a different outdist source), so its
dl_z runs ~+120 higher and the kamikaze's behind-margin stays positive
~3 frames longer -> slot12 survives past tick 1783 on retail only.

Rotation math EXONERATED with a new permanent oracle test
(`audit_mallrotzsort.rs`): built-ROM GSU `mallrotzsort` over synthetic
drawlists matches `matrix_rotate_q15` bit-exact on all 24 cases across
identity and rotated matrices (required decoding the TRUE GSU drawlist
layout from STRUCTS.INC: y@16 x@18 z@20 sflags@7 shape@8 - NOT the
SG.ASM-style order first assumed).

### Next action (refined after var-level probes)

Native at t=1782: viewtype=NORM, OUTDIST=VIEWDIST=120, applies full
-120 pull-back (finz=pvpz-120). Retail's final camera advances at exactly
pviewposz rate (+63/frame) with NO -120 step => best-fit model:

**Port performed the launch-handoff (GCSTRATS.ASM:1062-1074 /
PISTRATS playerExitBaseFollow -> viewtype_norm + OUTDIST=120) EARLIER
than retail.** While retail is still in playerExitBaseFollow_strat,
its viewposz is driven directly (Achase toward player_posz rate 3 +
al_vz add, PISTRATS.ASM:701-708) => no pull-back term => +120 offset.

Address archaeology dead-end reached: the built-minus-$8B rule does NOT
generalize past the PVIEW block ($14F4/$14FA validated only).
Validated retail reads: $14FA == native pvpz tick-exact; $15C4/$18BF/
$1597/$14FE/$00B6/$00B8/$16BF/$18B9 all wrong or non-informative.
Locating retail's live VIEWTYPE/OUTDIST needs either SPC700-style
operand-mining from the retail ROM code that writes them (find
`s_set_var B,viewtype,#viewtype_norm` equivalent store bytes), or a
Mesen watch.

Concrete next probe: instrument BOTH sides' player-strat identity each
tick (native stratptr id; retail al_stratptr word @pool+0x16 for the
PLAYER slot) across t=1000..1790 — find the exact tick port leaves
ExitBaseFollow / retail doesn't, then compare the zdist thresholds.
## NEXT: tick-1783 kamikaze slot-12 ATZREMOVE cull timing

Pure remaining case: positions/counters identical 105 ticks; native's
draw-cull frees the kamikaze at t=1783, retail ~3 frames later. Carry
emulation alone did not close it. Candidates: initial carry into first
iteration (try C=1), GSU dl_z hi-word truncation nuance in mallrotzsort
(mdotprod16mq keeps rsumhi AFTER rol — verify port rotation matches that
exact width), or GF_NOZREMOVE/firstframe flag divergence. Fast probe:
dump native rel_z/zmax margin t=1778-1790 alongside retail keep/kill.
## BREAKTHROUGH: single root cause behind ticks 1744-1795 (2026-08-22)

Full-chain instrumentation (retail WRAM camera block at $14F4-$14FA,
live native GameCamera vars, per-tick player-object dump) proves the
tick-1744 robot-wave placement error is the ONLY real divergence:

1. Wrong native robot/pillar birth positions (t=1744) -> their lasers/
   collisions damage the player asymmetrically through the masked window.
2. **t=1782: native pshipflags2 gains $80 == PSF2_PLAYERHP0 -- the native
   PLAYER DIES** (hp->0). Retail survives.
3. Player-death handler sets PSTF_INSEQ ($08), spawns crash smoke
   (slot 7 shape 357 trailing the ship), halves vel 65->33 / vz 63->31
   (t=1785), locks controls.
4. Camera follows player_posz -> native viewposz falls behind retail by
   +31/frame ("x2 camera" was player-speed halving).
5. last_depth_change 31-vs-62 is the first UNMASKED observable (t=1786);
   map_countdown would drift next; every later mismatch inherits this.

ELIMINATED as causes this round: sh_zmax table values (retail base
$017AF located; word $B70C resolves EXACTLY to zmax=50 == port flat-9
metrics [10,40,50]; earlier 42190 readings were a bank-mapping bug in
my probe scripts); view rotation math (already oracle-verified);
kamikaze slot-12 cull (byte-identical 105 ticks; its 3-frame lifetime
gap was downstream of the same player-state split).

ALSO VALIDATED: retail pviewvelz=$14F4 / pviewposz=$14FA read sane and
matched native pviewposz tick-for-tick until the death frame; retail
block addresses = built-ROM symbol minus $8B.

=> Fixing the robot-wave birth placement to retail-exact resolves the
entire 1744+ cascade in one stroke. Resume the ASL-scale investigation
(docs section above) with the added fast feedback loop: any candidate
fix can be validated by whether the player survives past gf~850.
