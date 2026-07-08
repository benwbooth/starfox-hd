# RE Accuracy Audit — coverage matrix

Goal: 100% behavioral accuracy to the original SNES ROM. Every function/subsystem
verified against the ASM (line-by-line logic diff) and, where isolatable, the
sf-oracle differential harness (65816 `call`/`call_near`, GSU `Gsu::run`).

Status legend: VERIFIED (oracle-proven or line-diffed vs ASM, regression test
kept) · FIXED (bug found + fixed + test) · PARTIAL (some fns verified) ·
UNVERIFIED (not yet audited) · N/A (no ROM counterpart, e.g. HD glue).

| Subsystem | Rust | ROM source | Status | Evidence |
|---|---|---|---|---|
| Core trig/tables (mulslog, SINTAB/COSTAB, sin/cos) | sf-strat snes_trig | MACROS.INC, SGDATA.ASM | VERIFIED (reachable range) | audit_trig.rs 7 tests bit-exact; latent ≥128 divergence documented |
| perc56/62/75/87/93 | sf-strat common | STRATROU.ASM:2494 | VERIFIED | audit_trig.rs |
| gen_vecs 2D/3D/side/front | sf-strat common | STRATROU.ASM alvelvecs/n3dvecs/sidevecs/frontvecs | VERIFIED | audit_trig.rs, gen_3dvecs.rs |
| speed_to | sf-strat common | STRATROU.ASM sr_speedto | VERIFIED (reachable) | speedto.rs; ≥128 wrap documented |
| chase / chase8 / chase_proportional | sf-strat common | STRATMAC adiv2 | FIXED (i16::MIN overflow) | commit a3414b9; adiv2 toward-zero verified |
| dist_xz | sf-strat common | STRATROU.ASM xzdiffs_l:1796 | FIXED (Manhattan→scaled-Euclid) | audit_coldet.rs bit-exact |
| angle_xz | sf-strat common | GSU arctan16 | PARTIAL | gsu_arctan.rs (cardinal exact; shallow-angle refinement WIP) |
| RNG | sf-strat common | RANDOM SWB chain | VERIFIED | random.rs bit-exact |
| apply_velocity / add_to_pos | sf-strat common | STRATROU addalvecs/addvecs | VERIFIED | apply_vel.rs |
| init_objvars | sf-game obj.rs | STRATROU init_objvars_l:2311 | FIXED (realobj/inviewpl/animframe/colframe/type_) | audit_boss.rs ROM dump |
| obj alloc/free/kill lists | sf-game obj.rs | OBJ/MAIN FmtFreeLst | FIXED (kill_all order) | audit_coldet.rs |
| do_coll | sf-game coldet | do_coll_l $1FD252 | VERIFIED | do_coll.rs |
| coldet_run (filters, immunity, collcount) | sf-game coldet | COLDET.ASM 172/518/523/829 | FIXED (collcount seed, immunity sentinel, category filter, double dispatch) + player↔friend skip | commit 592de5d + freeze fix |
| player collision routing (pcbox objects) | — | PSTRATS pcboxobj_B/LW/RW:152 | UNVERIFIED — port uses simplified direct model; friend-skip is a stopgap | memory: collision-model gap |
| Player movement (steering, banking, decays, clamps, bounds) | sf-strat player | PSTRATS 2280-2736 | VERIFIED | audit_player.rs + player_bounds.rs; Zrotfloat wobble term missing (minor, noted) |
| Player weapons (cadence, caps, single/double/beam) | sf-strat player | PSTRATS playerfire_srou:2836 | VERIFIED | line-diff (session log) |
| Player spawn defaults (outdist etc.) | sf-strat player | MAPMACS/GSTRATS | FIXED (outdist=120 seed removed) | audit_player agent + re-bless |
| Camera matrix (rotation) | sf-render transform | GSU mcrotmatzxy16 MWCROT.MC | FIXED (ZXY, was ZYX+flipped yaw) | gsu_rotmat.rs Δ=0 |
| Projection/FOV | sf-render transform | GSU mdo_project MOBJ.MC:5156, cscrc=112 | FIXED (60°→47.2°, scale 256) | derivation in commit 2d368da; cross-validated by bg2d focal=256 |
| 2D-bg scroll (horizon coupling) | sf-render bg2d | GSTRATS.ASM calcbgscroll_l:3190 | FIXED (ROM-exact: linear -6px/pitch + clamp + BGS base; dropped tan() + the +18 fudge; horizontal yaw·8 verified) | commit + gl_runtime golden; frame-verified |
| Camera view source (getview_l) | sf-game camera | GAME.ASM:6-58 | AUDITED; outv* port reverted for feel (accumulators unpopulated in normal flight), follow-cam kept | audit_player.rs proves ROM formula |
| Map-VM spawn opcodes (QOBJ/OBJ8/DOBJ/MAPOBJ/MOTHER/OBJZROT coords) | sf-game game.rs | WORLD.ASM | VERIFIED | audit_mapvm.rs 5 ROM-diff tests |
| Map-VM non-spawn opcodes (71-op table) | sf-game game.rs/world.rs | WORLD.ASM | FIXED (maploop count-1, REMOVE one-match+ref-clears, SETBGM hp0 guard, WAIT2-zero, SETVAROBJ-invalid); MOTHER submap PORTED (bemother oracle bit-exact, asteroid waves restored); FADETOSEA/GROUND palette crossfade IMPLEMENTED (SEA/GROUND.COL row-4 walk, HD-smooth lerp) | audit_mapvm2 17/17 + audit_mother 2/2 |
| Map scroll/advance (lastplayz/mapcnt) | sf-game game.rs:244 | WORLD.ASM:50-90 | VERIFIED (incl. exitbase lastplayz=0 reset) | freeze-fix agent trace |
| Level bytecode data (30 maps) | sf-map levels/ | LEVEL*.ASM | VERIFIED (byte-identical port) | route1/2/3_parity tests |
| Path system (opcodes, interp, catalog) | sf-path (98 fns) | PATHS.ASM + path data | FIXED (8 of 10: proportional chases, spawn /4*4 Z-X-Y, ADDW, accel count1, space coupling, ifbetween, childdead, sound2 + set0/ifzero collapses; 8 paths byte-clean vs ROM). Wave-3: gotopos triggers, explode children, linkchild, friend weights, spawnchild /4*8 | audit_path.rs 8/8 |
| Enemy strats A (rader/zaco*/houdai/tower/para…) | sf-strat enemy_a | GASTRATS/KSTRATS | FIXED (4 bugs: zacos3, zaco1 homing, houdai aim/gate, zaco3/4 XZ) + inits verified | audit_strats.rs |
| Enemy strats A — remaining tick logic | sf-strat enemy_a | GASTRATS/KSTRATS | PARTIAL (audited set only; houdai cadence mask-15+al1pt follow-up) | |
| Enemy strats B (boss7/bossA/bossF families, spacepilon) | sf-strat enemy_b | GB2STRAT/GB3STRAT/DSTRATS | FIXED (all 26 audit findings: playerturn180 port, bossA husk/resurrect machine, inverted gates, 7 notdelay masks, achase toward-zero, live mines, death chains) | AUDIT_ENEMY_B_FINDINGS.md + audit_strats_b.rs |
| Ground strats | sf-strat ground | GSTRATS | VERIFIED (stayrel/gnd/stayrelhard180yr exact; staydist fixed to per-tick tracking) | audit wave 3 |
| Boss inits (boss2/bossg/boss8) | sf-strat bosses | GBSTRATS/D2STRATS/GB3STRAT | VERIFIED | audit_boss agent line-diff |
| Boss tick state machines | sf-strat bosses/enemy_b | *STRATS | PARTIAL (boss7/bossA/bossF wave-3; boss2/bossg 9 fixes wave-4 incl. muzzle-rotz + regen rate + BLACK_C flicker; boss8/seamon/boss1 audit in flight) | AUDIT_BOSS_TICKS_FINDINGS.md |
| Route/planet progression | sf-game planets/shell | PLANETS.ASM | FIXED (convertroute bracket) | level_clear test |
| Draw/showview (culling, shadow, AF flags, depth) | sf-game draw.rs | MAIN.ASM alienflags_l:2009 + GSU mallrotzsort | FIXED (all 6: camera-anchored cull+zmax margin, frontpl clear, invisible skip, leftpl basis, zaco3 rightofview, per-level shadowheight) | audit_showview.rs 4/4 |
| Shape colors/palettes | sf-render shapes | COLTAB/LIGHT.ASM | VERIFIED | color_resolution.rs 10 tests |
| Shape mesh data | sf-render shape data | USHAPES etc. | VERIFIED (compiler-generated; builtins hand-checked) | |
| HUD/score/lives logic | sf-game/sf-render | MAIN/SPRITES.ASM | UNVERIFIED | |
| SFX trigger mapping (play_se ids) | sf-strat/sf-audio | SOUND.ASM | UNVERIFIED (laser 0x35 fixed) | |
| SPC engine | sf-spc | SPC700 program | VERIFIED (plays correctly; bit-exactness N/A scope) | |
| Frame scaling | sf-game | framescale | VERIFIED | framescale.rs |

## Wave log
- 2026-07-04 wave 1: trig/player/coldet/strats/boss/mapvm audits — ~18 fixes, all committed (see rom-oracle-plan memory).
- 2026-07-07 wave 2 COMPLETE: path (8), map-VM (5), showview (6), bg2d. 229/0.
- 2026-07-07 wave 9: LE_* warp/route dispatch (black-hole/special routes now reachable; strat-side arming still blocked on unported bholeexit); webmonster spider (route-3 3_2) -> boss parity 16/~22. Workspace 310/0. Remaining bosses: amoeba, cruiser1/2, madtrucker, seadragon2-variants, Andross(bossB)+bossh capstones.
- 2026-07-07 wave 8: seadragon/seadragon2/lochness (route-3 3_3, sprouty neck chain) -> boss parity 15/~22; audits banked (sound-IDs: 0 wrong-id/4 wrong-type/2 stray; route-progression: LE_* dispatch missing = warps unreachable). Workspace 301/0.
- 2026-07-07 wave 7: castanet 'Metal Smasher' + shared ground-vehicle base
  (route-2 2_5); chicken + shared arm_istrat grabber (route-3 3_3) -> boss parity
  14/~22; enemy-A ground/common batch (10 High +~27, ROM-correct laser speed/fire
  masks/worldy/Achase across nearly every regular enemy; #31 correctly skipped as
  a bad-doc-fix; base1 door caveated). 6 fixtures re-blessed single-cause
  (RNG-aligned). Workspace 296/0.
- 2026-07-07 wave 6: path friend-weights (weighted RNG tree); pcbox 3-box
  collision-proxy layer (gated); unported-boss roadmap (docs/UNPORTED_BOSSES_PLAN
  .md, 12 ported/~11 unported/2 cut); mulslog bit-exact vs ROM (3 latent >=128
  bugs, 11264-pair oracle proof); flingboss+deadflingboss ported (boss parity
  12/~22, IS 58/59, route-2 2_4). Workspace 284/0.
- 2026-07-07 wave 5: boss1 (12 fixes) + boss8/seamon (15) + score/credits/tally
  (hit-% + bonertab + real HUD score) + makesnd positional SE layer (infra) +
  m_bossHP accumulator (boss bar now drains, 12 sites) + s_test_special count fix.
  Workspace 267/0. Remaining: makesnd call-site wiring, pcbox routing, path
  leftovers, mulslog/speedto >=128 latents, unported bosses (bossB/D3/DSTRATS).
- 2026-07-07 wave 4 (user-report day): opening cinematic restored (view matrix
  direct world->camera + yaw-rotated cull; view_matrix_guard test); death crash
  sequence + lives unification (game-over reachable); explosion/hitflash ranged
  SFX; horizon +18 base restored (load-bearing); title spin fixed (camera
  co-rotation + ROM tit pose); boss2/bossg 9 fixes + BLACK_C; FADETOSEA/GROUND
  palette crossfade. Workspace 248/0.
- 2026-07-07 wave 3 COMPLETE: enemy-B/ground all 26 findings + MOTHER submap ported (asteroid waves restored). Workspace 241/0. Remaining queue: HUD/score, SFX set_sound2 map, pcbox collision, palette fades, path leftovers, remaining boss ticks, >=128 latents.
