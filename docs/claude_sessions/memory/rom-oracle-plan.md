---
name: rom-oracle-plan
description: "Build an automated, scalable ROM-oracle differential test harness (user's top priority)"
metadata: 
  node_type: memory
  type: project
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

**User directive (2026-07-03):** "automate as much of the accuracy improvement
as possible so that it scales." Stop hand-auditing ASM and guessing signs
(that repeatedly mis-fixed steering). Build a harness that verifies Rust
against the **real ROM** automatically.

**Why:** manual ASM reading kept producing subtly-wrong fixes. The retail ROM
is the ground truth: `Star Fox (USA) (Rev 2).sfc` (repo root, gitignored).

**Two layers:** game logic runs on the **65816** (movement/init/collision/
map-VM/strats — where the bugs are); 3D math+render runs on **Super-FX/GSU**
(the white-laser/flicker/"faces-vs-moves" render bugs). The 65816 oracle
covers logic only.

**Feasibility (all confirmed):**
- Retail ROM present. Disassembly `reference/ultrastarfox/` **assembles** with
  its bundled DOS toolchain (`BIN/ARGSFXX.EXE`+`ARGLINK.EXE`, `-z` exports
  symbols) — wine can't run it headless (no display/advapi32), but native
  `nixpkgs#dosbox-x` under `nixpkgs#xvfb-run` should. Build entry:
  `dosbox-x -fastlaunch build.bat` from `reference/ultrastarfox/`.
- Rust crates `w65c816` / `wdc65816` for a pure-Rust, headless, in-test-suite
  CPU (no display → reliable here; game/emu GUI runs HANG headless).
- `mesen`/`ares`/`snes9x` in nixpkgs for a full-game differential (heavier).
- `SF_STATE_DUMP` diff scaffolding already exists (`sf-app/src/statedump.rs`,
  `sf-difftest`).

**INFRASTRUCTURE DONE (2026-07-03), committed on wgpu-backend:**
`rust/sf-oracle/` — pure-Rust, headless 65816 execution harness.
- `SnesBus`: LoROM + 128KB WRAM `System`; `call(bus, target_snes, &Entry)`
  reset-pulses the `w65c816` core, boots a WRAM stub at $00:0200 (CLC/XCE
  native, REP #$30, LDA/LDX/LDY $F0/$F2/$F4 param block, JSL target, STP),
  runs to STP, reads regs+WRAM. Reset needs a **one-shot `res()` pulse** and
  `stopped()`==`stp` (true at power-on — must pulse res). `a()` returns 8-bit.
- Symbols: `data/symbols.txt` (committed, 13623 syms, `LABEL\t$00bbaaaa` =
  **SNES LoROM addr**, NOT a file offset). `load_symbols()` -> name->u32.
  Key: N3DVECS_L=$1FC436, PLAYERMOVE_SROU=$0BD33C, DGEN3DVECS=$098272,
  PLAYERMOVE_INIT=$0BD1FF.
- ROM: `data/sf.sfc` (gitignored ROM data) = the **built 2MB enhanced**
  ultrastarfox ROM the symbols refer to; its gameplay 65816 == retail (the
  disassembly the port is written from). Retail 1MB ROM has DIFFERENT layout —
  do NOT use it with these symbols.
- Verified end-to-end: N3DVECS_L bytes == STRATROU.ASM opening (stz/stz/stz/
  stx/sty/phb). 4 tests green.

**How to regenerate sf.sfc + symbols.txt (dosbox recipe, fiddly):**
`cd reference/ultrastarfox && nix shell nixpkgs#dosbox-x nixpkgs#xvfb-run -c
'xvfb-run -a dosbox-x -set cpu cycles=max -set cpu core=dynamic -fastlaunch
BUILD.BAT'` — outputs `SF.SFC` + `SYMBOLS.TXT` at that dir (BUILD.BAT `pause`s
at end; reap dosbox). wine dosbox FAILS headless; SDL dummy-video mounts C:
wrong; kill stale Xvfb between runs (they block new launches). Build ~5-8 min.

**FIRST DIFFERENTIAL TEST DONE — `tests/gen_3dvecs.rs` (ROM n3dvecs vs Rust
strat_gen_vecs_3d), 8 cases green.** Harness upgrades it forced:
- `Entry.p` sets M/X width; stub does `SEP #(p&0x30)` (n3dvecs enters 8-bit A).
- **Emulated CPU math regs** in SnesBus: $4202/03 mul, $4204-06/$4214-17 div.
  n3dvecs' `mulslog` uses the HARDWARE multiplier (`sta $4202; sta $4203; lda
  $4217`) — without it every product was 0. Any fn using hw mul/div needs this.
- ABI: n3dvecs inputs troty=$1631, trotx=$1630, tmpz=$78 (bytes); outputs
  x1=$02,y1=$08,z1=$8A (16-bit). Enter p=0x20. Sine tabs SINTAB=$8b62/
  COSTAB=$8ba2, logtabs $8306/8406/8606 all in ROM.

**Findings (oracle earning its keep):**
1. STEERING SIGN CONFIRMED correct vs ROM (roty=64->vx&x1 both neg). Yaw fix
   validated against ground truth — stop second-guessing it.
2. **Precision ~2%**: ROM uses hardware /128 fixed-point (`mulslog`), Rust uses
   float (98 vs 100). THIS is the user's "imprecise/wobbly" complaint. FIX:
   port mulslog (`(|a|*|b|)>>7 * sign`) into strat_gen_vecs_3d for bit-exact
   vectors. High value.
3. vy sign: ROM does NOT negate pitch; Rust does (renderer Y convention, up/
   down works). Verify whether the negation belongs in the renderer instead.

**DONE since:** ported `mulslog`+SINTAB/COSTAB to `sf-strat/src/snes_trig.rs`;
`strat_gen_vecs_3d/2d/side/front` + `path_adapter` genvecs now bit-exact (was
float, ~2% drift = the wobble). Bugs the oracle FOUND+FIXED:
- path_adapter genvecs_3d missing yaw negation (space-lane steering inversion).
- frontvecs: angle = roty+1 (ROM `inx`); sidevecs: angle = 65-roty. Port
  omitted the +1. `tests/vector_family.rs` now all EXACT.

**Alien-based ABI (for vector_family tests):** X = alien slot base; fields
al_rotx=$12, al_roty=$13, al_rotz=$14, al_vel=$15, al_worldx=$0C (offsets from
X). Outputs x1=$02,y1=$08,z1=$8A (DP, 16-bit). Enter p=0x20 (8-bit A). Put the
test alien at XBASE=$0100 (clear of DP scratch). frontvecs/sidevecs take
speed in A; alvelvecs reads al_vel.

**GSU BOUNDARY:** `arctan16_l` (GAME.ASM:550) dispatches to the Super-FX via
`jsl runmario_l` (mcallarctan16). The 65816 oracle can't run GSU code. Before
testing a fn, grep its ROM body for `runmario_l`/`mcall` — if present it's GSU.
GSU code is in `SF/MARIO/*.MC` (assembled into the ROM); opcode map in
`tools/sf2/disasm/gsu.py` (disassembler only).

**GSU EMULATOR STARTED — `rust/sf-oracle/src/gsu.rs` (WIP foundation):**
Prefix-modal core (FROM/TO/WITH + ALT1/2/3), R0-R15 (R15=PC, R14=GETB ptr),
SFR Z/CY/S/OV/G flags. Implemented: IBT/IWT, ADD/ADC/SUB/SBC/CMP, AND/BIC/OR/
XOR/NOT, MULT/UMULT, INC/DEC, LSR/ASR/ROL/ROR/DIV2/SEX/LOB/HIB/SWAP/MERGE,
branches, LDW/LDB, GETB, JMP/LJMP, LOOP, STOP. Self-test green ((5+3)*2=16).
**GSU RUNS REAL ROM arctan16 — core validated.** Added memory ops (LM/SM/LMS/
SMS/STW), FMULT/LMULT, MOVE/MOVES, LINK/SBK, GETB/ROMB. `tests/gsu_arctan.rs`
runs mcallarctan16 @$01:81AA with the real GSU RAM ABI (m_x1=ram[$62], m_y1=
ram[$2C], m_cnt=ram[$40]) and matches atan2 EXACTLY for all cardinal+diagonal
angles. Bugs found doing this: MOVES is `Dreg<-Rn` (was backwards); ROMB is
ALT3 (alt1&alt2), not ALT2.

**GSU DELAY SLOTS implemented (big fix):** GSU branches (Bxx/LOOP/JMP) execute
the NEXT instruction (delay slot) before jumping — proven by MMATHS.MC
`mdivu3115` (`loop`/`jmp` each followed by a `rol`/`lsr` the cycle comments
count). That trailing `rol` is the per-iteration r0 shift; without it the
divide quotient stayed 0. Now the divide runs all 16 iters. `pending_branch`
applies after the following step; `trace_range` field for debugging.

**GSU arctan off-axis — DEEPER DIAGNOSIS (needs a reference GSU emu to finish):**
For x=50,y=87 (|y|>|x|) the divide correctly gives |y|/|x|=1.74 (Q14=$6F5C).
`$8207 FROM r4; LSR x5` -> index 890, which OVERFLOWS the 432-byte ARCTANTAB
($9274-$9424) -> garbage fine angle -> 88deg not 29. BOTH my emu and the ROM
take the no-swap path for |y|>|x| ($81ED BGE, verified S==OV), and the swap
($81F0) makes dividend=larger either way, so the ratio is ALWAYS >1 (overflow).
mdivu3216 source literally says "NB this is not accurate!" — so the ROM arctan
may be genuinely approximate here and my 88deg might MATCH real hardware (GETB
reads the same ROM). To confirm whether my emu diverges or the ROM is just
approx, run x=50,y=87 through a REFERENCE GSU emulator (bsnes/higan/Mesen
Lua) and compare m_cnt. If they match, my emu is CORRECT and strat_angle_xz's
float atan2 is the DIVERGENCE (ROM is approx) — port the approx instead.
GSU CORE is validated (delay slots + cardinal/diagonal exact); this is an
arctan-specific approximation question, not an emu-core bug.

**Chase validated (2026-07-03):** decoded SR8_ACHASE_ALVAR3 ($1FD8D5) — adiv2
(CMP #$80; ROR; BPL; ADC #0 = round-toward-zero) x shift + min-step (clamp
small diff to 2^shift) + exact-at-target. `strat_chase_proportional` MATCHES
(the rtz + min-step-1 fix is correct).

**PERC93 BUG FOUND+FIXED (oracle, 2026-07-03):** ROM `perc93a_l` = sum of
shifted halves (val>>1+>>2+>>3+>>4), port did `val-val>>4` — differ by
truncation. perc56/62/75/87 all match (camera-follow scaling CONFIRMED
correct). Added `Exit.c` (16-bit accumulator) for A->A funcs. tests/perc.rs.
Oracle ABI trick: A->A funcs use entry.p=0x00 (16-bit A), read exit.c.

**USER COMPLAINTS (2026-07-03, still present after all fixes) + SUSPECTS:**
1. Still wobbly — vector math is bit-exact now, so it's the 20fps->60fps
   INTERPOLATION (task #40), not the sim. Camera bob (pfm_wobble) is authentic.
2. "left/right moves arwing but CAMERA moves opposite" — ROOT CAUSE FOUND via
   ASM (GAME.ASM `getview_l`, NOT a guess; negating cry alone made it WORSE —
   reverted). The ROM camera rotation:
     viewrotxw = outvx
     viewrotyw = outvy - player_turnrot   (16-bit)
     viewrotzw = outvz - plrotz           (16-bit)
   then the matrix NEGATES ALL THREE (matxw=-viewrotxw, matyw=-viewrotyw,
   matzw=-viewrotzw; GAME.ASM ~68-95). `outvz = player_Ztilt + player_Zshake`
   (PSTRATS.ASM:2755, spfm_inside); outvx/outvy set at GAME.ASM 141-147 (toobj:
   Xanglexy_l/Yanglexy_l) — need the NORMAL-mode outvx/outvy source too.
   Rust `camera.rs` (~268-270) is SIMPLIFIED + WRONG: rot_x=player.rotx,
   rot_y=player.roty-(turnrot>>8), **rot_z=0** (no camera roll — the ROM rolls
   the camera with banking!). FIX = faithful port of getview_l: rot_z =
   outvz-plrotz, use outv* not player.rot*, and make transform.rs negate all 3
   view angles (currently pitch+roll only). Multi-step (trace player_Ztilt/
   Zshake/Zrotfloat + outv* per mode). This is the wobble/steering-feel too.
3. Lasers white + wrong sound — white = GSU-side face color (render); wrong
   sound = wrong SFX id in playerfire (65816, check play_se id vs ROM). NEXT.

**PRE-EXISTING (long-standing, NOT this session) — bo_parity RED.** All 3 boss
parity tests (bossg/boss2/boss8, sf-strat/tests/bo_parity.rs) diverge at LINE 1:
`al.type_` (dump field `t`) = 8 in Rust vs 0 in the C fixture, at boss init tick
1 (T1 A00), everything else matching. Confirmed failing at session-start commit
af43e34 too. So the boss init sets type_=8 where the C oracle had 0 — a boss-port
bug from tasks #9-11, or a common spawn/init path. Investigate what sets al_type
on boss spawn. NOTE: this masks any RNG/speed_to boss-parity effect (fails before
those run), so bo_parity can't currently validate RNG-dependent boss behavior.

**RNG SWAP DECISION (C-oracle vs ROM-oracle) — PENDING USER.** sf_random kept on
the C-oracle LCG (×91+$61D7); ROM runtime RNG is the SWB chain (proven,
tests/random.rs). Swapping is ROM-correct but diverges RNG-dependent bosses from
the frozen (unregenerable) C fixtures. User must choose C-parity vs ROM-fidelity.

**HIGH-PRIORITY FOLLOW-UP — runtime RNG uses WRONG algorithm.** Rust sf_random
(common.rs) = LCG rndval*91+$61D7, but that's a BUILD-TIME assembler macro
(MACROS.INC) for baking static data. ROM RUNTIME RNG = RANDOM ($2F7BF, called
32x in strat code, e.g. EXPSTRAT explosions): a 4-byte subtract-with-borrow
chain over $DE-$E1: `A=DE; CLC; A=A-DF-!C->DF; -E0->E0; -E1->E1; -DE(orig)->DE;
ret A`. PROVEN bit-exact in tests/random.rs. TO INTEGRATE: add 4-byte RNG state
to GameVars (replace u16 RNDVAL), swap sf_random to the SWB, return u8 (all 9
callers mask low bits: &0x0F/&7/&15, so u8 is fine). Seed: ROM re-seeds rand
from vblank/IRQ entropy (BOOTNMI/GAME/IRQ.ASM) so EXACT sequence is
unmatchable; use a fixed seed (C port used 0) — the win is the correct
distribution, not bit-exact sequence.

**Systematic validation status:** VALIDATED bit-exact vs ROM: gen_vecs_3d/2d/
side/front, perc56/62/75/87/93, chase (SR8_ACHASE), addalvecs_l (apply_velocity
tests/apply_vel.rs). Oracle infra (65816+GSU) proven. Many leaf strat subs
remain ($1FD range: SR_BANKTOPLAYER, SR_MAKE_XYVEC, etc.) + playermove_srou
@$0BD33C. SR_SPEEDTO@$1FD625 FIXED (overflow, tests/speedto.rs). do_coll FIXED.

**COMMITTED this session (all on master, ASM/oracle-grounded):** perc93 fix;
single-fire laser SFX $60->$35 (se_laser, PSTRATS playerfire_srou); laser
color -> bullet_a1 colanim (was static white; shapes.rs CA_6/FX109); camera
roll rot_z = (outvz-plrotz)>>8 (getview_l GAME.ASM, was rot_z=0 -> 'camera
opposite'); + earlier gen_vecs mulslog, front/side +1, path-lane inversion.

**framescalevecs — RESOLVED (no-op is CORRECT):** oracle (tests/framescale.rs
via new call_near) shows framescalevecs = vx*framerate/4, identity at
framerate=4. SF base rate ~15fps = 60/4 -> framec=4 -> identity at base. Rust
no-op (player.rs:1090) confirmed correct. NOT the wobble.

**WOBBLE (task #40) — localized to render interpolation, root cause NOT yet
pinned.** Ruled out: sim math (bit-exact), framescalevecs (no-op ok), object
interp (draw_list.rs obj_id-keyed + lerp_angle8 wrap-aware = correct), camera
interp (transform.rs set_view_lerp EXISTS; big_jump thresholds 600 world-units
& 48/256 angle(~67deg/tick) are too high to trip in normal flight), present
mode (AutoVsync = regular pacing). NEXT: needs in-game observation of WHAT
wobbles & WHEN (terrain scroll? a specific object? during bank vs straight?).
Candidates left: ground/terrain scroll interp, starfield, per-object at high
world-Z, or the inherent 20Hz sim showing through on fast lateral motion.
call_near (sf-oracle) now enables testing RTS/near strat subs.

**playerlimitx_srou (PSTRATS.ASM $BDF1C) — FIXED (oracle-confirmed, task #34).**
Disasm: `LDA $1ACB; AND #$F3; STA $1ACB` (8-bit A) then `REP #$20; LDA $0C,X;
CMP $15F9; SEP #$20; BEQ clamp; BMI clamp; JML .nminX` -> clamps + sets arrow at
worldX <= min (INCLUSIVE; BEQ+BPL for max). Port used `<`/`>` -> dropped the
edge arrow at the exact limit. Fixed to `<=`/`>=`. tests/player_bounds.rs (0
diffs). KEY ABI LESSON: some strat subs enter in 8-bit A (p=$20), NOT ai16 —
they do byte ops (arrows) before REP #$20. call_near default entry must match;
disassemble the first bytes to pick M/X width before oracle-testing.
Y-bounds RESOLVED (no change): the ROM's minpWmoveY/maxpWmoveY clamp (PSTRATS
296/441) belongs to pLWing_strat — the player's WING object (W = Wing, not
water), setting pml_lwtop/bottom + water splash. The player's own Y clamp
(PSTRATS 163-167) is COMMENTED OUT in the ROM. So the Rust Y clamp in
playerlimit_x_srou has no faithful ROM player-Y equivalent; it's a justified HD
addition — leave it. do_coll_l (coldet.rs) FIXED (oracle tests/do_coll.rs, 5/6->0 diffs): ROM
`DEC collcount; BNE exit` (dec-then-check; port did check-then-dec = off-by-one
damage every hit); indestructible = any hp bit7 set ($80-$FF via BMI), port
only matched $FF. Collision DETECTION single-box arithmetic VALIDATED correct (COLDET macro
COLDET.ASM:10): rangexz = ext1+ext2 (sum BOTH), overlap when |d| < sum (strict
<, via sbc+bmi); Rust aabb_overlap matches (>= sum -> reject). GAP: ROM
supports multi-box per object (cl_colbox list, normalcol vs box-list path) +
animated boxes (cb_frame, per gameframe) for complex objects/bosses; Rust
flattens to ONE AABB (xmax/ymax/zmax). So boss/multi-part hitboxes differ ->
possible residual "pass through" on complex objects. Follow-up: port the cl_colbox multi-box list (LARGE, dedicated pass).
SCOPE: SF/ASM/COLBOXES.ASM (862 lines, ~492 colbox defs). colbox macro =
next,xoff,yoff,zoff,rot(x/y/z|norot),xmax,ymax,zmax,setflags,clrflags[,scale].
cb struct (18B): cb_next$0, cb_frame$2, cb_xoff$3, cb_xmax$A, cb_sizeof$12.
cl_colbox=$7E2F52 (RAM, built at runtime). Player itself uses 3 boxes
(playerB_col body + playerLW/RW wings, hit flags HF1/HF2/HF3 -> wing-break
mechanics); Tunnel + many enemies/bosses too. Needs: (1) extract 492 boxes,
(2) per-box offset+rotz transform, (3) HF1-5 body-part hit flags, (4) wire the
normalcol-vs-boxlist branch (COLDET.ASM:557 cl_colbox test). Rust today = 1
AABB/object. Oracle suite: 15 green
(lib 5, apply_vel, framescale, gen_3dvecs, gsu_arctan, perc, player_bounds,
vector_family 3).

**DISPLAY / LAUNCHER — RESOLVED. Run the game with `./play.sh`.** The app was
using Xwayland (X11); now runs NATIVE Wayland + Vulkan, no X server needed
(moot that I deleted /tmp/.X11-unix/X0 during dosbox builds — never rm
/tmp/.X11-unix/*). Fix committed: flake runtimeLibs += vulkan-loader, wayland,
libdecor; scripts/run.sh exports SDL_VIDEO_DRIVER=wayland + WGPU_BACKEND=vulkan
+ /run/opengl-driver/lib + VK_ICD_FILENAMES pinned to the GPU (AMD=radeon_icd)
when WAYLAND_DISPLAY set; ./play.sh = build+launch. libdecor is REQUIRED (SDL3
won't open a Wayland window without it). User is on KDE (kwin), AMD GPU (RADV).

**RENDER-MATH ORACLE (the SCALABLE render-accuracy solution) — WORKING.** The
camera matrix + vertex projection are GSU routines (MARIO/*.MC), runnable in the
GSU emulator like arctan16 -> diff the ROM's real render math vs the Rust
AUTOMATICALLY (no pixel/visual guessing). PROVEN: tests/gsu_rotmat.rs runs
mcrotmatzxy16 (camera rotation matrix, MWCROT.MC:50) at GSU entry $8295 (NOT
$829F), inputs=angles at GSU RAM $20/$22/$24, matrix out at $D2 (9x i16). Gives
exact matrices: rot(0,0,0)=identity(0x7FFE=1.0), pitch 22.5deg->cos .9239/sin
.3827. ROM order = ZXY, 16-bit angles (65536=360deg), 16-bit sin/cos.
CAMERA MATRIX — FIXED via the oracle. Diff proved the port used ZYX + a
sign-flipped yaw (pitch/roll Δ=0 but yaw Δ=0.765, combined Δ=1.677). Derived +
proved the correct formula: M = Ry·Rx·Rz (ZXY), positive angles, ROM per-axis
signs (Ry=[cy,0,+sy;0,1,0;-sy,0,cy]) reproduces the ROM matrix EXACTLY (Δ=0).
Ported to BOTH build_view_matrix_f AND the object-matrix fn in transform.rs.
Expanded elements: [0]=cy*cz+sy*sx*sz [1]=-cy*sz+sy*sx*cz [2]=sy*cx [4]=cx*sz
[5]=cx*cz [6]=-sx [8]=-sy*cz+cy*sx*sz [9]=sy*sz+cy*sx*cz [10]=cy*cx. Shared
transform behind entities-too-high / horizon disconnect / camera-opposite.
PROJECTION FOV — FIXED. ROM GSU mdo_project (MOBJ.MC:5156) projects
screen=coord*256/z; vertical center cscrc=112 (RAMSTUFF.ASM). So vertical FOV =
2*atan(112/256)=47.2deg and projection[5]=256/112=2.286. Port had guessed 60deg
(over-wide -> objects floated high). Fixed transform.rs set_projection; keep
horizontal f/aspect for widescreen. Frame centroid unchanged (2D bg dominates)
but ROM-exact now. DUAL FIX + cross-validation: bg2d.rs:678 computes the 2D
horizon focal = projection[5]*(BG2D_H/2) with BG2D_H=224, so the fix makes it
(256/112)*112 = 256 EXACTLY = ROM projection scale (was ~194 at 60deg). So the
FOV fix also corrects the 2D/3D horizon disconnect (shadow-above-horizon) AND
the 2D-focal landing on 256 confirms the 47.2deg value is right (not off by 2^n). wmatrotp16/mdo_project full GSU point-project oracle still
possible but GSU RAM is union-packed (zmalc, need assembled addrs); matrix
already verified so the rotate step is sound.
NEXT oracle targets: (2) wmatrotp16_l ($03AE62 -> GSU) for vertex PROJECTION
(pipeline: crotmat16 build matrix -> copymat -> wmatrotp16 rotate point; wmat in
GSU RAM). (3) getview_l 65816 viewrot setup via call() (+ the angle conventions
Rust rot_x/y/z vs ROM viewrotxw 16-bit). (4) 2D-bg horizon formula. Each -> a
failing test, not a guess.

**ACCURACY WAVES (goal: 100% ROM accuracy; tracker = docs/ACCURACY_AUDIT.md in
repo — read it first, it supersedes this summary).** Method proven at scale:
parallel audit agents (each writes sf-oracle differential tests + reports
confirmed divergences w/ ASM refs) -> fix agents pointed at findings files.
Wave 2 (2026-07-07, COMPLETE, ~20 fixes pushed): path system (proportional
ease-out chases w/ short-way wrap — was linear; spawn /4*4 Z-X-Y scale; ADDW;
accel count1; ifbetween; sound2; 8 ROM paths byte-clean), map-VM (maploop
count-1 off-by-one affected EVERY level loop; REMOVE one-match player-exempt;
SETBGM hp0 guard; WAIT2; SETVAROBJ), showview (camera-anchored cull + sh_zmax
via shape_extents; frontpl/leftpl/invisible; per-level shadowheight), bg2d
scroll ROM-exact (linear -6px/pitch, dropped tan + 18px fudge).
Wave 3 (in flight): enemy-B/ground fix agent applying 26 findings from
docs/AUDIT_ENEMY_B_FINDINGS.md (bossF playerturn180 missing, bossA turret
husk/resurrect loop, 7x notdelay mask misreads = gameframe&((1<<N)-1) NOT %N,
inverted objinfront/lower gates, achase_angle toward-zero rounding); MOTHER
submap agent porting the asteroid-wave interpreter (MOTHER.ASM) + fixing
STRAT_ADDR_MOTHER1==istrat-0 player collision.
Wave 3 remaining after: HUD/score, SFX set_sound2 map vs SOUND.ASM, pcbox
player-collision routing, FADETOSEA/GROUND palette fades, path leftovers
(gotopos triggers, explode children, linkchild, friend weights, spawnchild),
mulslog/speedto >=128 latents + sp_common re-bless.
HORIZON BASE (+18) IS LOAD-BEARING — DO NOT REMOVE AGAIN. bg2d.rs
sky_uv_window: painted-horizon base shift SNES_HORIZON_ROW(130)-112 = +18 rows.
Removing it for "ROM purity" (the BGS vofs bases genuinely put the painted
green at ~row 130 vs vanishing line 112) made ground objects visibly FLOAT —
user reported it within one session, same complaint as the original task #22.
On a CRT the 18-row gap reads as haze; on the clean port output it reads as
floating. Keep: ROM-exact LINEAR slope (-6px/pitch, clamp [-56,232]) + the +18
display-compensated base. gl_runtime top-row golden = (49,98,156) with it.
2026-07-07 also: intro/title ship "spins faster and faster over time" reported —
diagnosis agent was killed by session limit; suspects: rate accumulation in an
intro strat, object stacking, or render-side u8-angle lerp wrap drift. Title
tit strat should be ROM rotz+=2 w/ ENDSEQ.ASM:1799 init angles (port does
roty+=1 from 0). Probe test file rust/sf-app/tests/title_spin_probe.rs
(untracked) was left by the agent. HUD/lives/score/SFX fix batch (docs/
AUDIT_HUD_SFX_FINDINGS.md) + boss8/seamon/boss1 audit also killed mid-run by
the limit (resets 11:10am PT) — their partial edits were REVERTED (tree clean);
redo from the committed findings docs.
KEY LESSONS: (1) All C-era fixtures are suspect — the C shared bugs with the
Rust (maploop, achase rounding, float trig); ROM ASM/oracle is the only truth;
SF_BLESS_FIXTURES=1 re-blesses every parity suite (sf-map bins, sf-strat
ea/eb/bo, sf-game trace, sp_player via SF_BLESS_SP_PLAYER). (2) STRATMAC macro
semantics to check at every port site: s_jmp_notdelay N = mask (1<<N)-1;
s_Achase_* = proportional adiv2-toward-zero (vs s_chase_* linear); s_jmp_lower
branches on >=; s_scale_alvar = ASL (x2 not /2); s_jmp_objinfront = bpl on
a.z-b.z. (3) Give fix agents the findings as a repo file, not inline prompt.

**COLLISION MODEL GAP (important, 2026-07-04).** The coldet collcount fix made
collisions deal real damage (was 0). But the port uses a SIMPLIFIED direct
player-object collision, whereas the ROM makes the player `colldisable` and
routes hits through separate box objects (pcboxobj_B/LW/RW, PSTRATS.ASM:152) with
`playercoll_Istrat` applying NO body-collision HP damage. So after the fix the
player took damage from things it shouldn't. FIRST symptom fixed: player was
DYING from bumping the friendly escort (Slippy/Frog friendship_4) during the
exit-base launch -> froze the exit-base fixed camera -> looked like "camera
doesn't follow the arwing, gets stuck." Fix (coldet.rs coldet_run): skip
player<->friend pairs (friend path objs carry nonzero al_sbyte4; player has
ASF4_PLAYEROBJ; enemies sbyte4==0 so enemy dmg unaffected). If the player keeps
dying to things it shouldn't (terrain, other non-enemy objs), the real fix is to
port the ROM's pcbox player-collision routing. The camera ROTATION source was a
red herring (reverted outv* -> player.rotx/roty; both ~0 in normal flight).

**SYSTEMATIC ORACLE-AUDIT WAVE (2026-07-04, goal: diff each ASM fn vs Rust).**
Parallel general-purpose agents each diff a subsystem via sf-oracle + write a
kept regression test. Method scales; confirmed+FIXED bugs:
- strat_dist_xz: was Manhattan |dx|+|dz|; ROM xzdiffs_l is scaled-Euclidean
  ((max+((x1>>1+z1>>1)<<1)) *4.5)>>3. Broke every enemy proximity/aim gate.
- strat_init_obj_vars: ROM init_objvars_l sets realobj(sflags3|=$08)+
  inviewpl(flags|=$10); port never set realobj -> spatial audio wrong.
- Obj::kill_all: reversed free list; ROM FmtFreeLst is forward (head=slot0).
- shell stage-advance: route0 stage1 gave M2_2(8) not M1_2(2); ROM planetseq_l
  brackets the map walk with convertroute (0<->1) — port dropped both. Fixed.
- Camera source (getview_l): normal view used player.rotx/roty (ship rot); ROM
  uses view accumulators: viewrotx=noxrot?0:outvx, viewroty=outvy-turnrot,
  viewrotz=dozrot?(outvz-plrotz):0 (all >>8). player.roty already has turnrot so
  the port CANCELLED it -> camera never yawed on U-turns. Full outv* port in
  progress (user approved): OUTVZ sv slot + gf_viewrot nudge in playermove_srou.
- Enemy strats (audit_strats.rs): zacos missing zacos3 barrage state + should
  banktoplayer not aim_3d; zaco1 fires RELSLOWELASERHOME not plain, every 4th
  frame not 2nd; houdai aim is achase shift1 not snap + XZ gate not Z-only;
  zaco3 fire gate XZ not Manhattan-3D. (fix agent running)
VERIFIED CORRECT (no bug): core trig/math (mulslog/sin/cos/gen_vecs/speed_to/
perc, audit_trig.rs 7 tests), apply_vel, add_to_pos, do_coll, aabb_overlap, many
enemy HP/AP consts + rader/tower/zaco3-circle logic. tests: audit_trig,
audit_coldet, audit_player, audit_strats in rust/sf-oracle/tests/.

**FRAME ORACLE (render inspection) — WORKS.** Capture a headless rendered frame:
`SF_START_PLAYING=1 SF_START_MAP=1 SF_DUMP_PPM=/home/ben/frame.ppm
SF_DUMP_PPM_TICK=150 SF_MAX_TICKS=160 ./play.sh` -> 1280x720 P6 PPM, analyze
with python. KEY: SF_DUMP_PPM now routes the app through the HEADLESS Gpu
(main.rs) because read_pixels_rgb only works offscreen (windowed surface returns
black). SF_START_PLAYING (shell.rs) skips title->planet-select into gameplay;
SF_START_MAP=<id> picks the map (M1_1=1 Corneria). NEXT for render bugs
(entities-high, horizon disconnect, laser color, wobble): need a SNES REFERENCE
frame (emulator at same state) to diff against — the Rust frame alone shows
values but not right-vs-wrong.

**CAMERA fixes:** roll now gated on dozrot ($1776) — no roll in Corneria.
Laser/shape anim fixed (obj.rs colframe/animframe default 0 not 0xFF).

**CAMERA VIEWROT — FIXED (ticks 120–121).** `GameCamera` matches ROM
`getview_l`: `viewrotxw=outvx` (+`noxrot`), `viewrotyw=outvy-player_turnrot`,
`viewrotzw=outvz-plrotz` when `dozrot`. Horizon +18 kept. Verified:
`pitch_follows_outvx_not_player_rotx`, `yaw_follows_outvy_not_player_roty`,
`float_ground_rom_pitch_keeps_pullback_level`, `audit_player::getview_viewrot_vs_rom` MATCH.

**HARNESS LESSONS (why launching looked broken for ages):**
1. `pkill -f starfox-hd-rs` MATCHES MY OWN BASH SHELL (its cmdline contains the
   string) and kills it -> "Exit code 1, no output". ALWAYS use `pkill -x
   starfox-hd-rs` (exact process name).
2. Each Bash tool call has its OWN ephemeral FS view — a file written in call A
   is invisible in call B. To read game output: redirect to a file AND cat it
   in the SAME Bash call.
3. The game's stdout/stderr is otherwise swallowed; capture via
   `timeout N ./play.sh > /home/ben/sfout.log 2>&1; grep ... sfout.log` all in
   one call, with `dangerouslyDisableSandbox: true` on the Bash tool.
4. nix develop is NOT sandboxed (writes /home, sees wayland socket); the Bash
   TOOL is. Use dangerouslyDisableSandbox to launch the GUI game.

**Still-testable 65816 NEXT targets:**
- `strat_apply_velocity` (addalvecs_l) — worldx/y/z += vx/vy/vz. Simple.
- chase: `s_Achase_var`, strat_chase_proportional (I edited it for the wobble
  round-toward-zero fix — validate).
- `playermove_srou`@$0BD33C, `GameVars_Init` (init routine), collision.
Pattern per fn: decode ABI from ASM + empirical WRAM dump, `call()` ROM vs
Rust, assert, fuzz. (n3dvecs worked because its mul is the 65816 HW multiplier,
NOT the GSU.)

**Already proven correct by windowless tests (not oracle, but reliable):**
`tests/steering.rs` (pad::LEFT → worldx screen-left) + the render-direction
test — game logic + renderer are correct; the residual left/right inversion
is the SC2 analog-X axis (SDL unmapped), NOT the sim. D-pad is correct.
See [[reap-background-jobs]] for the windowless-test approach.
