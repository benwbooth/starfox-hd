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
| 2D-bg horizon coupling | sf-render bg2d | TRANS calcbgscroll | PARTIAL (focal now ROM-exact via FOV fix; full formula unaudited) | |
| Camera view source (getview_l) | sf-game camera | GAME.ASM:6-58 | AUDITED; outv* port reverted for feel (accumulators unpopulated in normal flight), follow-cam kept | audit_player.rs proves ROM formula |
| Map-VM spawn opcodes (QOBJ/OBJ8/DOBJ/MAPOBJ/MOTHER/OBJZROT coords) | sf-game game.rs | WORLD.ASM | VERIFIED | audit_mapvm.rs 5 ROM-diff tests |
| Map-VM non-spawn opcodes (jumps, vars, waits, triggers, ~40 ops) | sf-game game.rs/world.rs | WORLD.ASM | UNVERIFIED | |
| Map scroll/advance (lastplayz/mapcnt) | sf-game game.rs:244 | WORLD.ASM:50-90 | VERIFIED (incl. exitbase lastplayz=0 reset) | freeze-fix agent trace |
| Level bytecode data (30 maps) | sf-map levels/ | LEVEL*.ASM | VERIFIED (byte-identical port) | route1/2/3_parity tests |
| Path system (opcodes, interp, catalog) | sf-path (98 fns) | PATHS.ASM + path data | UNVERIFIED (catalog_bytes + interp_trace tests exist vs C, not ROM) | |
| Enemy strats A (rader/zaco*/houdai/tower/para…) | sf-strat enemy_a | GASTRATS/KSTRATS | FIXED (4 bugs: zacos3, zaco1 homing, houdai aim/gate, zaco3/4 XZ) + inits verified | audit_strats.rs |
| Enemy strats A — remaining tick logic | sf-strat enemy_a | GASTRATS/KSTRATS | PARTIAL (audited set only; houdai cadence mask-15+al1pt follow-up) | |
| Enemy strats B | sf-strat enemy_b | GBSTRATS etc. | UNVERIFIED (eb_parity is Rust-regression only) | |
| Ground strats | sf-strat ground | GSTRATS | UNVERIFIED | |
| Boss inits (boss2/bossg/boss8) | sf-strat bosses | GBSTRATS/D2STRATS/GB3STRAT | VERIFIED | audit_boss agent line-diff |
| Boss tick state machines (all bosses) | sf-strat bosses | *STRATS | UNVERIFIED | |
| Route/planet progression | sf-game planets/shell | PLANETS.ASM | FIXED (convertroute bracket) | level_clear test |
| Draw/showview (culling, shadow, AF flags, depth) | sf-game draw.rs | MAIN.ASM showview | UNVERIFIED | |
| Shape colors/palettes | sf-render shapes | COLTAB/LIGHT.ASM | VERIFIED | color_resolution.rs 10 tests |
| Shape mesh data | sf-render shape data | USHAPES etc. | VERIFIED (compiler-generated; builtins hand-checked) | |
| HUD/score/lives logic | sf-game/sf-render | MAIN/SPRITES.ASM | UNVERIFIED | |
| SFX trigger mapping (play_se ids) | sf-strat/sf-audio | SOUND.ASM | UNVERIFIED (laser 0x35 fixed) | |
| SPC engine | sf-spc | SPC700 program | VERIFIED (plays correctly; bit-exactness N/A scope) | |
| Frame scaling | sf-game | framescale | VERIFIED | framescale.rs |

## Wave log
- 2026-07-04 wave 1: trig/player/coldet/strats/boss/mapvm audits — ~18 fixes, all committed (see rom-oracle-plan memory).
- 2026-07-07 wave 2 (this): path system, map-VM non-spawn opcodes, draw/showview. Next: enemy-B/ground/boss ticks, HUD, SFX map.
