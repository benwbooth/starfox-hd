# boss2 (Macbeth spinning top) + bossg (sea boss) tick audit findings (2026-07-07, ASM-verified)
Scope: tick state machines only (inits previously verified). ASM refs authoritative:
GBSTRATS.ASM (boss2 family), D2STRATS.ASM (bossg), STRATMAC.INC / STRATLIB.INC macros.
Rust: rust/sf-strat/src/bosses.rs. Do not commit fixes without re-running boss fights.

Note for fixers: `ifeq 0` in this assembler ASSEMBLES the block (IFEQ expr = true when
expr==0). The turret children and the whole boss2plasma_strat body are LIVE in ROM;
the Rust port already treats them as live — correct, do not "fix" that.

## High
1. m_bossHP per-frame accumulator missing (affects BOTH bosses' HP bars, and every other
   boss). ROM: each part strat adds its HP into m_bossHP every tick — boss2top
   GBSTRATS.ASM:756 `s_add_bossHP x,al_hp`, boss2turret GBSTRATS.ASM:803, bossg .move2
   D2STRATS.ASM:368 `s_add_bosshp x,al_hp` (macro STRATLIB.INC:562 = `m_bossHP += al_hp`;
   m_bossHP is zeroed each frame by the engine and mdrawbossHP draws m_bossHP vs
   m_bossmaxHP). Rust has NO m_bossHP at all: boss2top_strat (bosses.rs:776-817),
   boss2turret_strat (:857-878) and bossg_move2 (:2068-2073) drop the add, and
   sf-game/src/shell.rs:514 feeds `boss_hp_cur: v.bossmaxhp` — the HUD boss bar is
   permanently full. Fix: add a bosshp accumulator var (zeroed per frame before strat
   processing), do `bosshp += al.hp` at the three sites above, wire shell.rs boss_hp_cur
   to it. (bossg consequence: the bar also never reflects the .waitsometime regen.)
2. boss2 muzzle offsets ignore firer rotx/rotz — fire_weapon applies muzzle offsets via
   `s_add_Roffs2pos.w B,y,x,x,weapx,weapy,weapz,1,1,1,...` (GSTRATS.ASM:2795): rotate
   flags 1,1,1 = full 3-axis rotation by the FIRER's rots. boss2_strat state 2 sets
   rotz=deg180 (GBSTRATS.ASM:597) and it is never cleared, so in states 3-5 the boss is
   inverted and the state-4 muzzle (0,-480,0) is rotated by rotz=128 to (0,+480,0) — the
   ROM fires from the ground-facing tip. Rust b2_yaw_offset_pos (bosses.rs:364-381)
   rotates around Y only, so b2_spawn_shot (:526-549) leaves offy at -480: state-4
   RELFASTELASER (:1205) and boss2top's HP<=16 RELSLOWELASERHOME while flipped (:800)
   fire 960 units off in Y. Fix: rotate the muzzle offset by rotx, roty AND rotz in
   b2_spawn_shot (turret fire is unaffected in practice: turrets only exist in state 0,
   rotz=0; missiles fire only under sflag4 = state 0).

## Medium
3. boss2 state-4 laser spread distribution: `s_weapon_rndrots2obj y,7,7`
   (GBSTRATS.ASM:643) = per axis `(random_l() & 7) - 7/2` = [-3,+4]
   (s_weapon_rndrot STRATMAC.INC:2099 -> s_set_var2rnd AND mask, STRATMAC.INC:5489).
   Rust b2_random_signed(g,7) = `rnd % 15 - 7` = [-7,+7] (bosses.rs:316-323 used at
   :558-559 via :1205). Fix: spread = `(sfrtl_random(g) & 7) as i16 - 3` for both axes
   (still 2 RNG draws, pitch then yaw — keep draw order x(rot-x/pitch) first).
4. boss2top missile coin flip: `s_jmp_random .nother` (GBSTRATS.ASM:742, macro
   STRATMAC.INC:1407) = `random_l() < (50*255)/100=127` keeps yaw -deg22 (127/256);
   +deg22 when rnd >= 127 (129/256). Rust uses `sfrtl_random(g) & 1` (bosses.rs:808).
   Fix: `if (sfrtl_random(g) & 0xFF) >= 127 { yaw = DEG22 }` (one draw either way).
5. bossg sea_not_delay is %-based with a spurious per-object phase; s_jmp_notdelay N =
   `(gameframe [+offset]) & ((1<<N)-1)` and BOTH bossg call sites pass no offset:
   - .waitsometime HP regen `s_jmp_notdelay 2,.notyet` (D2STRATS.ASM:226) = regen 1 HP
     when gameframe&3==0 (every 4th frame). Rust `!sea_not_delay(idx, 2, gf)` =
     `(gf+idx)%2==0` (bosses.rs:1401-1407 used at :2129) — regen at 2x ROM rate, phase
     shifted by object index.
   - .move2 splash gate `s_jmp_notdelay 1,.nosplash` (D2STRATS.ASM:372) = gameframe&1==0.
     Rust passes period 1 -> sea_not_delay returns false always -> splash every frame
     (:2070). Latent only because sea_make_splash is a no-op today; fix before splashes
     are implemented.
   Fix sea_not_delay to `((gameframe + offset) & ((1<<n)-1)) != 0` with offset an explicit
   parameter (0 here; the al1pt-staggered sea users pass idx), and audit its other callers.

## Minor
6. boss2 Zdistmore off-by-one at equality: jmp_distmore branches on `|pz-z| >= dist`
   (rlbpl incl. equal, STRATMAC.INC:3362-3378).
   - state 0 (GBSTRATS.ASM:550): smoke at >=1100; Rust `near = <= 1100` keeps rel at
     exactly 1100 (bosses.rs:1086) — should be `< 1100`.
   - state 3 (GBSTRATS.ASM:627): advance at >=1100; Rust `> 1100` (:1185).
   - state 4 (GBSTRATS.ASM:649): z-hold when < 500; Rust `<= 500` (:1211).
7. boss2petal death drop misses colldisable: s_kill_obj (GBSTRATS.ASM:831, macro
   STRATMAC.INC:2643) sets colldisable AND hp=0. Rust only sets BOSS2_SFLAG2 + hp=0
   (bosses.rs:915-919). Add `al.sflags |= ASF_COLLDISABLE;`.
8. boss2 state-4 circle velocities round toward -inf on the negative half:
   ROM `s_set_alvar2alvartab B,B,W,vx,sbyte2,sintab,-3` / `vz,costab,-1`
   (GBSTRATS.ASM:665-666) = sign-extended TABLE BYTE (amp 127) then adiv2 x3 / x1
   (toward ZERO). Rust `((strat_sin(sb2)*127.0) as i16) >> 3` / `>> 1`
   (bosses.rs:1236-1237): `>>` floors negatives (e.g. -100: ROM -12, Rust -13). Use a
   toward-zero shift (same class as strat_chase_proportional's documented fix). The f32
   sin vs. the ROM byte table remains the port-wide accepted approximation.
9. bossgs shadow-clone flicker missing (part strat spawned by .generateshadows): ROM
   sets coltab BLACK_C on odd gameframes, clears it on even (D2STRATS.ASM:481-486).
   Rust bossgs_strat (bosses.rs:1983-1999) has no coltab handling. Cosmetic.
10. bossgs x-chase clamps but ROM oscillates: `s_fchase_alvar2alvar W,worldx,sword1,5`
    (D2STRATS.ASM:488) is Fchase_A (STRATMAC.INC:559) — fixed +-5 step, NO overshoot
    clamp (only exact-equal stops), so ROM jitters within +-5 of sword1 once close.
    Rust strat_chase clamps to target (common.rs:264-277). Sub-pixel; fix only if
    oracle-diffing bossgs.

## Known gaps (pre-existing placeholders, not new findings)
- bossg .genspark is a stub (bosses.rs:2025); ROM copies boss pos to dummyobj, y-=60,
  sgenspark_srou_l sprite spark (D2STRATS.ASM:343-352).
- sea_make_splash is a no-op (bosses.rs:1453); ROM .move2 additionally spawns the splash
  at worldz+30 with allst temporarily = self and forces splash worldy=0
  (D2STRATS.ASM:373-380, s_make_splash STRATMAC.INC:4738) — implement together.
- .scrollmsg only advances the tx counter; the actual message scroll is elsewhere.
- boss2 particlefiredown_Istrat is a placeholder tick (bosses.rs:507).

## Verified correct (don't touch)
- boss2 state-machine shape: sequential fall-through == ROM (ASM `nextstate`
  STRATROU.ASM:2977 re-enters the strat top after s_next_state; same-tick chaining and
  the single trailing roty+=2 match Rust's if-chain + return placement).
- boss2 gameframe masks: state-4 fire &1, state-5 hitflash &1 (even frames), top laser
  &7, top missile &31, petal anim &3; petal top-death `s_jmp_NOTdelay 0,...,al1pt` has
  mask 0 -> never branches -> Rust's unconditional kill is right.
- state 2: s_jmp_lower #-1000 polarity (chase block only while worldy < -1000, checklist
  confirmed), achase x>>4 / z>>5 toward player_posz+200 via toward-zero
  chase_proportional, sword2 ground handling (0 from state 1, -480 in dive), falldown
  order (add_playerZ then vecs), land-sound placement (worldy>=ground after non-final
  bounce), particle follow/remove.
- boss2_falldown_yvec == s_falldown_Yvec (STRATMAC.INC:1813): gravity add, higher-skip,
  snap to ground, -vy>>shift with [-5..0]->0 clamp, done-on-zero; call params (2,2) and
  (1,1,-240) match.
- state 0: children thresholds (>7 skip / ==6 extra spin / ==5 advance, alvarmore is
  strictly-greater: beq skip + rlbpl), sflag1 sound latch ($71), sbyte3=2, smoke pair
  (L2smoke, addrnd2posy = +-127 on x/y exactly like addrnd2pos_xy, z-100, y -280/-160).
- state 4: decbne sbyte4 reset-to-100, fire window sbyte4<=25 (strict-more), vz
  self-subtract hold, sflag3 sound latch, colldisable clear, sbyte2+=4, top-death exit
  vecs (0,10,30); state 5 player-alive polarity (psf2_playerHP0 clear = alive = dodie),
  boss_dying, per-tick L-exp (y+120, vy-20, lifecnt 1), landing -> boss2exp.
- boss2top: pos/rots copy, colldisable release on mother sflag3, HP<=16 gate
  (strictly-more 16), mother sflag2 set, roty push/0/pull around missile fire, weapon
  ptr=player; boss2turret: roty==deg180 exact gate, addyrot2z, muzzle (0,-208,+288).
- Muzzle-offset magnitudes at ALL boss2 sites use the effective-value convention
  correctly: weapon_scale=2 (STRATEQU.INC:838), fire applies <<2 (GSTRATS.ASM:2795), so
  ASM `(-60<<3)>>2` == Rust b2u(-60) etc. Only the rotation flags are wrong (finding 2).
- boss2plasma: LIVE in ROM (`ifeq 0` assembles true) — Rust's orbit is correct: spin
  brackets 10/6/3/1 (strict-more 30/60/90), dist<<4 offset around petal yaw sbyte1,
  roty=deg180 rewrite, dist wrap at 120, worldy=sword2 then achase sword2->player_posy>>2,
  removal when petal gone or petal sflag2. Turret children spawned likewise (init
  matches the assembled-true block).
- petal open/close: mother-sflag1 select, target = mother sbyte3 (2 then 4), +-2 steps,
  shape = boss2petal_tab[sbyte3>>1] (byte offset into word table), plasma launch gate
  (mother sflag3 && !self sflag1) with colldisable+sflag1 latch.
- bossg mode table: indices 0-7, opentrunk 8/14/20/26, launch L/R 9-10/15-16/21-22/27-28,
  waitabit2 11/13/17/19/23/25/29 (4th cycle has NO trailing waitabit2 — commented out in
  ASM), closetrunk 12/18/24/30, genshadows 31, waitabit 32, sf9e 33, runaway 34,
  loopback 35 -> bossg_loopback=3 (s_mode_entry labels the .runaway entry). Same-tick
  mode chaining (.nxtmode re-dispatch) == Rust loop/continue.
- trunk anim: s_add_anim +1 limit 10 == Rust "advance at anim==9" (both take 10 ticks,
  anim caps at 9); sounds $5a at anim==0 / $59 at anim==9; waitabit 70 / waitabit2 10
  with znxtmode zeroing sbyte1 (and moveto600h's z8nxtmode correctly NOT zeroing).
- dzdistless (DSTRATS.ASM:151) is strictly-less vs player-z; all thresholds/steps match:
  waituntil z-=40 then <150; scrollmsg <140 back 40, tx+=4 &127; runaway spark <1000,
  z+=70, advance at !(<4000); moveto600h $9d latch <1500 on sflag8, advance <600 +
  sflag8 clear, else z-=40; explode topple <350.
- waitsometime: regen only when bossmaxhp!=0 and hp!=120; advance needs maptrigger bit0
  clear AND sea_0/sea_0_0/sea_0_1 all absent; wait path move vs move2 by bossmaxhp==0
  (same split in runaway). disappear: nullshape + maptrigger|=1. appear: shape restore,
  HP/AP/bossmaxhp reset only when bossmaxhp==0.
- launchfish (+30 y, copy pos+rots, right fish sflag3); generateshadows: 3 clones at
  z-50 with sword1 -100/0/+100, alloc-fail bails to next mode.
- bossgexplode: boss_g_s purge, init falls through to first tick, 3 small exps (y-60,
  rnd+-15), anim cap, dz<350 -> maptrigger|=2 + bossexplode; the post-jml rebounce code
  in ASM is unreachable and correctly omitted.
- bossgs dash: beqdec order (dash at 0 without decrement, else dec + add_playerz).
