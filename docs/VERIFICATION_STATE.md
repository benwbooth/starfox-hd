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
