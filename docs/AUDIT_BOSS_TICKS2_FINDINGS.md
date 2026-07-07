# boss8 (wash boss) + bossseamon + boss1 tick audit findings (2026-07-07, ASM-verified)
Scope: tick state machines (+ death chains, fire gates, muzzle offsets). ASM authoritative:
GB3STRAT.ASM + GASTRATS.ASM:39-131 (boss8 room), GA2STRAT.ASM:3056-3196 (bossseamon),
GASTRATS.ASM:2046-2140 (seamon, audited alongside), GBSTRATS.ASM:87-462 (boss1),
STRATMAC.INC / STRATLIB.INC / STRATEQU.INC macros. Rust: rust/sf-strat/src/bosses.rs
(boss8, bossseamon, seamon), rust/sf-strat/src/enemy_a.rs (boss1).
Per instructions, sea_not_delay's own semantics and everything bossg were NOT re-audited
(fixed in docs/AUDIT_BOSS_TICKS_FINDINGS.md items already committed).

Macro facts established for this audit (cite these when fixing):
- s_jmp_notdelay/ifdelay N,label[,off] = (gameframe [+off]) & ((1<<N)-1) (STRATMAC.INC:6456/6470).
  N is a BIT COUNT, never a modulus. `al1pt` off = per-object stagger.
- setstateN (STRATROU.ASM:2980-2992) re-enters the strat TOP same tick (s_jmpto_strat).
- s_add_anim amount,max (3-arg) WRAPS at max; 4-arg with label CAPS at max-1 and jumps
  (STRATLIB.INC:180-247).
- s_gen_vecs writes al_vx/al_vz ONLY — never vy (STRATMAC.INC:3637).
- s_jmp_random label = branch when random_l() < 127 (STRATMAC.INC:1407).
- s_set_alvar2rnd obj,var,#m = rnd & m, no centering (STRATMAC.INC:4404).
- s_add_rnd2pos obj,mx,my,mz = per axis (rnd&m)-m/2, one draw PER AXIS even for m=0
  (STRATMAC.INC s_add_rnd2pos).
- s_add_Roffs2pos rot flags apply rotz FIRST, then rotx, then roty; trailing 3 args are
  per-axis ASL counts (STRATMAC.INC:4098). fire_weapon uses flags 1,1,1 + <<weapon_scale(2)
  (GSTRATS.ASM:2795); dobossrot/boss1rots use rotz+roty (no rotx) + <<1 (GBSTRATS.ASM:40-45).
- jmp_distmore branches on |d| >= dist (inclusive); jmp_distless strictly less;
  ABSalvarmore strictly more, ABSalvarless strictly less (beq falls through in both);
  jmp_outdistrng in-range = min <= |d| < max (STRATMAC.INC:3275-3436, 6609-6650).
- Flag byte layout (STRATEQU.INC:896-915): sflags byte2 = colldisable(01) Lcollide(02)
  smflag1(04) noexpsnd(08) sflag1(10) sflag2(20) sflag3(40) sflag4(80). s_not_alsflag +
  s_beq tests the WHOLE byte after EOR, not the bit.

## High

1. boss1: five fire-rate constants misread as `% N` periods — the raw macro N (a bit
   count) was kept as a modulus. All five sites, ASM -> Rust:
   - turret homing gate `s_jmp_IFdelay 4,.home,al1pt` (GBSTRATS.ASM:376) =
     (gameframe+al1pt)&15==0, per-turret stagger. Rust `gameframe % 4 == 0`, no stagger
     (enemy_a.rs:703 BOSS1_TURRET_HOME_DELAY=4, used :2372) — 4x ROM rate.
   - turret normal gate `s_jmp_IFdelay 5,.norm,al1pt` (GBSTRATS.ASM:378) = (gf+al1pt)&31==0.
     Rust `% 5` (enemy_a.rs:704, :2377) — 6.4x ROM rate, all turrets in sync.
   - center twin HMISSILE1 `s_jmp_NOTdelay 6,.nofire,#15` (GBSTRATS.ASM:248) = (gf+15)&63==0.
     Rust `% 15` (enemy_a.rs:705, :1980).
   - back-mode HPLASMA `s_jmp_notdelay 6,.nofire1` (GBSTRATS.ASM:210) = gf&63==0.
     Rust `% 6` (enemy_a.rs:706, :2165) — 10.7x ROM rate.
   - back-mode HMISSILE1 pair `s_jmp_NOTdelay 6,.nofire,#15` (GBSTRATS.ASM:217) = (gf+15)&63==0.
     Rust `% 15` (enemy_a.rs:707, :2173).
   Fix: replace the five consts with mask+offset logic: home `(gf + idx) & 15 == 0`,
   norm `(gf + idx) & 31 == 0` (al1pt -> object index per port convention), center/back
   missiles `(gf + 15) & 63 == 0`, back plasma `gf & 63 == 0`.

2. boss1back_strat hold/retreat condition INVERTED. ASM `s_jmp_Zdistmore x,y,#1500,.nzi`
   (GBSTRATS.ASM:192): |dz| >= 1500 -> .nzi = release cover + spin + fire barrage;
   |dz| < 1500 -> worldz += 15 (retreat) + full boss1_end. Rust (enemy_a.rs:2135-2143):
   attacks when |dz| <= 1500 and retreats (z += 15 forever) when > 1500 — exactly
   backwards. ROM boss backs off to 1500+ then bombards; Rust boss never backs off, or
   recedes to infinity if it enters back mode far away. Fix: swap the branch
   (`if |dz| < 1500 { z += 15; boss1_finish(true); return } /* attack path */`).

3. boss1 child/muzzle offsets: missing <<1 / <<2 scaling and missing rotz rotation.
   - Turret ring + cover slide: boss1rots_srou/dobossrot_srou use
     `s_add_roffs2pos B,...,0,1,1,1,1,1` (GBSTRATS.ASM:40-45, 276-313) = offset bytes
     <<1, rotated by rotz THEN roty. Effective ring: (±110,0,90)/(±250,0,90)/(±180,±50,90),
     cover x = sbyte4<<1. Rust boss1_get_turret_offset returns the unshifted (55,0,45)
     family (enemy_a.rs:1795-1806) and boss_yaw_offset_pos rotates by roty ONLY
     (enemy_a.rs:1459-1476) — ring at half scale and turrets/cover do not orbit while the
     boss spins in rotz (cover-up phase + back mode). Fix: double all table values (and
     cover sbyte4), and rotate offx/offy by mother rotz before the roty rotation.
   - fire_weapon muzzles = raw<<2, rotated rotz,rotx,roty (GSTRATS.ASM:2795): center
     missiles `#(±96<<2)>>weapon_scale` (GBSTRATS.ASM:251/256) = ±384 world; Rust passes
     ±96 (enemy_a.rs:1986-1987). Turret muzzle `#0,#0,#40>>weapon_scale`
     (GBSTRATS.ASM:374) = +40 world z; Rust passes 10 (enemy_a.rs:1951). Fix: multiply
     by 4 and include rotz in the muzzle rotation (same class as prior-audit finding 2).

4. m_bossHP per-frame accumulator sites (lane list for the accumulator fix from
   docs/AUDIT_BOSS_TICKS_FINDINGS.md finding 1). Complete list for THESE bosses — each
   is `s_add_bossHP x,al_hp` executed every tick the part is alive:
   - boss1 mother: boss1_fin (GBSTRATS.ASM:274) — reached from EVERY mother mode
     (up/normal/in/out/inclose/back all end at boss1_end or boss1_fin). Rust
     boss1_finish (enemy_a.rs:1970-1995) has no add.
   - boss1 turrets (x8): boss1turret_end (GBSTRATS.ASM:400) — every turret tick, fire or
     not. Rust boss1turret_common_strat (enemy_a.rs:2340-2381) has no add.
   - boss8 core: boss8_cont (GB3STRAT.ASM:130) — reached from boss8wait/boss8a/boss8b
     AND from boss8die's countdown branch (GB3STRAT.ASM:259), so the bar tracks hp
     through death. Rust boss8_cont (bosses.rs:2847-2869) has no add.
   - bossseamon, seamon: NO s_add_bossHP sites (verified none in GA2STRAT bossseamon /
     GASTRATS seamon) — they use gsvar_byte1 counting, not the boss bar.
   - (boss2/bossg sites already listed in the prior findings doc.)
   Related maxHP sites for the same lane: boss1 turret init `s_add_bossmaxHP x,al_hp`
   (GBSTRATS.ASM:338) — Rust DOES implement (enemy_a.rs:2326); boss1/boss8
   s_set_bossmaxHP inits implemented.

5. currentlevel is never wired: enemy_a.rs `currentlevel()` reads WRAM 0x1F03
   (enemy_a.rs:107,174) but nothing outside tests writes that slot; planets.rs keeps
   `g_currentlevel` (0-based, planets.rs:164) privately. Result: every level gate in all
   three bosses resolves to "not level 1" — boss8 always HP*2 + never auto-closes easy
   path (GB3STRAT.ASM:44-48,157), boss1 always full HP (GBSTRATS.ASM:98-102), cover
   clear frames always 30 (GBSTRATS.ASM:432-434), relslowelaser always speed 60,
   nucleus launcher always 2-missile cap, beam switch never explodes-on-hit easy rule
   (GB3STRAT.ASM:422). Fix: write the level into 0x1F03 on map start and settle the
   convention (ASM `s_jmp_iflevel N` == raw currentlevel == N-1; Rust compares ==1, so
   the writer must store level+1, matching what the parity tests already assume).

## Medium

6. boss1 back-mode HPLASMA spread + aim base (GBSTRATS.ASM:210-214):
   `s_weapon_rndrot 15,15` = per-axis (rnd&15)-7 -> [-7,+8], PITCH drawn first
   (STRATMAC.INC:2099), applied to the FIRER's rots (roty=deg180) — no aim at player.
   Rust (enemy_a.rs:2165-2171 + boss1_random_signed :1721) aims at the player and adds
   rnd%31-15 -> [-15,+15], YAW drawn first. Fix: spread = (rnd&15)-7, draw pitch first,
   base = firer rotx/roty (the shot homes afterward via al_ptr=player anyway).

7. boss1 back-mode HMISSILE1 offset applied to the wrong axis (GBSTRATS.ASM:217-226):
   `s_weapon_rot #deg45-deg11,#0` = PITCH offset ±24 on the firer's rots (yaw stays
   deg180); Rust adds ±24 to the YAW of an aim-at-player solution and uses aimed pitch
   (enemy_a.rs:2177-2198). ROM: two missiles ±24 above/below straight ahead; Rust: two
   missiles ±24 left/right of the player bearing.

8. boss1out_strat cycle counter order (GBSTRATS.ASM:170): `s_beqdec_alvar sbyte3,
   boss1inclose_init` branches when sbyte3==0 BEFORE decrementing — with init sbyte3=1
   the first out-pass decs to 0 and still goes to boss1normal_init; inclose happens on
   the NEXT out-pass. Rust decrements first then tests (enemy_a.rs:2100-2108), entering
   inclose one full in/out cycle early. Fix: `if sbyte3 == 0 { inclose } else { sbyte3
   -= 1; normal }`.

9. boss1 death chain simplified vs bossexplode_Istrat (EXPSTRAT.ASM:78-140): ROM plays
   s_boss_dying ($1e + bgm $f1 + pstf_notdie + fire off), spawns 14 staggered
   SML/MED/L explosions (lifecnts 5..34), a circdelayexplode child, self lifecnt 38,
   then bossdelayexplode. Rust boss1exp_init/boss1exp_strat (enemy_a.rs:2394-2426) has
   the count 38 and rotz spin (deg90/32 == boss1exp_Istrat GBSTRATS.ASM:87-90) but
   spawns no explosion barrage, no music, and plays $10 instead of $1e. Cosmetic/audio
   but very visible on the kill.

10. bossseamon state 8 splash-down randomization (GA2STRAT.ASM:3178-3181): ROM draws
    vx=(rnd&7) FIRST (then +5), THEN the negate coin `s_jmp_random` = negate when
    rnd >= 127 (129/256). Rust draws the coin first as `rnd & 1` then vx
    (bosses.rs:1782-1788). Fix: `let vxr = (sfrtl_random(g) & 7) + 5; let neg =
    (sfrtl_random(g) & 0xFF) >= 127;` (keeps draw order and the <127 coin).

11. seamon post-landing surface snap missing (GASTRATS.ASM:2091-2101): in the
    worldy in [-30,-1] band with vy>=0 and sflag1 ALREADY latched, ROM jumps to .nds
    which clamps worldy=0, vy=0 — the seamon teleports flush to the surface one tick
    after the landing latch. Rust (bosses.rs:1878-1889) only handles the latch tick and
    otherwise lets it drift up by velocity. Fix: add the else-branch clamp
    (`if sflag1 already set && vy >= 0 { worldy = 0; vy = 0; }`).

12. seamon swim-shape toggle is a whole-byte test in ROM (GASTRATS.ASM:2077-2079):
    `s_not_alsflag x,sflag2` + `s_beq .nsc` branches on the EOR result of the ENTIRE
    sflags byte2 (which also holds colldisable 0x01, sflag1 0x10 — STRATEQU.INC:906-913).
    Before the first landing the byte is only sflag2, so the shape alternates
    sea_0_1/sea_0_0; after the first landing sflag1+colldisable are set between jumps,
    the byte is never 0, and the shape is ALWAYS forced back to sea_0_0 (the sea_0_1
    frame never shows again). Rust tests only the sflag2 bit (bosses.rs:1859-1862) and
    alternates forever. Fix (ROM-faithful): mirror the byte test — set shape sea_0_1,
    toggle sflag2, then `if (sflags2 & (COLLDISABLE_EQ|SFLAG1|SFLAG2|...byte2 bits)) != 0
    { shape = sea_0_0 }` using whatever Rust bits correspond to ROM byte2 for this
    object (colldisable lives in a different Rust field — reproduce the OBSERVABLE rule:
    alternate only while sflag1 latch and colldisable are both clear).

13. boss8a open-flap animation wraps instead of capping (GB3STRAT.ASM:146):
    `s_add_anim x,#1,#15,.nanim` is the 4-arg/label form = CAP at 14 and branch
    (STRATLIB.INC:180 label variant loads max-1). Rust `animframe = (animframe+1) % 15`
    (bosses.rs:2891) loops 14->0 — the fully-open pose keeps replaying. Fix: `if
    animframe < 14 { animframe += 1 }`.

14. nucleuslauncher missing the objinfront gate (GASTRATS.ASM:60): ROM only arms when
    player.z < launcher.z (`s_jmp_objinfront y,x,nuclaunch_cont` branches out on
    player.z >= launcher.z). Rust fire condition (bosses.rs:3345-3358) has no such
    check — launchers keep firing kamikaze missiles after the player passes them.

15. boss8die clears bossmaxhp, ROM does not: boss8die_Istrat (GB3STRAT.ASM:208-220)
    never touches bossmaxHP (s_boss_dying doesn't either, STRATMAC.INC:7758). Rust
    boss8die_istrat calls set_bossmaxhp(0) (bosses.rs:2970) — HP bar vanishes at the
    kill frame instead of draining/persisting. Remove the call (drive the bar from the
    finding-4 accumulator instead).

16. boss8die Shyper debris uses the wrong view variable (GB3STRAT.ASM:254):
    `s_set_alvar W,y,al_worldy,viewposy` — viewposy (ALCS.INC:265) is a distinct camera
    variable from pviewposy (GILESALC.INC:178). Rust uses pviewposy (bosses.rs:3004,
    3014). Plumb viewposy or document the substitution.

17. boss8shrap RNG stream/order drift:
    - shrapfall spawn (GB3STRAT.ASM:484-486): ROM defers `s_set_strat y,shrapfall_Istrat`;
      the Istrat's 3 draws (worldx, sword1, roty — GB2STRAT.ASM:633-663) happen NEXT
      tick, interleaved with that tick's draws. Rust runs boss8_shrapfall_istrat at
      spawn (bosses.rs:3264-3267) — draws 1 tick early (also samples pviewpos 1 frame
      younger). Same class 1-tick init shift (no RNG) applies to nucleusbeam spawn and
      the boss8die->boss8shrap handoff (those are benign).
    - large-exp scatter `addrnd2posxyz2_srou` (EXPSTRAT.ASM:359-382) = draws x, y, z in
      that order, each sign-extended byte <<1 (±254 spread). Rust b8_add_rnd_xyz
      (bosses.rs:2488-2493) draws z FIRST, then x,y via addrnd2pos_xy, spreads ±127
      unshifted.
    - folexp scatter `s_add_rnd2pos y,127,127,0` (GB3STRAT.ASM:472) = (rnd&127)-63 on
      x/y PLUS a third draw for the z arm (masked to 0). Rust b8_add_rnd_xy = 2 draws,
      ±127 sign-extended (bosses.rs:3258). Wrong spread and one missing draw.

## Minor

18. boss1up rise boundary (GBSTRATS.ASM:138): s_jmp_higher branches when worldy <
    space_viewCY (strict, rlbmi — STRATMAC.INC:3072). Spawned at CY+1000 stepping -10,
    ROM passes through worldy==CY (continues to CY-10, switches next tick); Rust `<=`
    (enemy_a.rs:2037) stops at CY — final rest height differs by 10 permanently.

19. boss1in advance boundary (GBSTRATS.ASM:159): Zdistmore is inclusive — |dz| >= 1000
    holds (boss1_end); advance to out only when < 1000. Rust `<= 1000` advances at
    exactly 1000 (enemy_a.rs:2073). Same class in boss1inclose (GBSTRATS.ASM:182):
    hold at >= 300, Rust advances at ==300 (enemy_a.rs:2121). And boss1covdie
    (GBSTRATS.ASM:461): remove at |dz| >= 1000; Rust `> 1000` (enemy_a.rs:2289).

20. boss1 center missiles missing `s_set_colltype y,enemy1` (GBSTRATS.ASM:253/258) —
    ROM's boss1_end pair get the extra enemy1 colltype (shootable); back-mode pair do
    not. Rust boss1_fire_hmissile1 gives all missiles the same collflags
    (enemy_a.rs:1871).

21. boss1back tail: ASM goes straight to boss1_fin (GBSTRATS.ASM:227) — no sflag1 rotz
    spin, no boss1rots, no turret recount. Rust routes through boss1_finish(false)
    (enemy_a.rs:2200): first tick after cover release COVER_BLOCK is still latched ->
    one extra +deg90/32 rotz; also re-runs child positioning (no-op) and back_init
    (harmless). Also ROM aims center/back shots from the offset muzzle, Rust from the
    boss center (angle differs slightly at ±384 offsets).

22. bossseamonexp gsvar_byte1 decrement (GA2STRAT.ASM:3194): `s_dec_var B` wraps 0->255;
    Rust sea_dec_gsvar_byte1 saturates at 0 (bosses.rs:1535-1540). Only observable on a
    double-death frame; match ROM with a wrapping_sub.

23. sea_gen_vecs_angle zeroes vy (bosses.rs:1482); s_gen_vecs never writes vy
    (STRATMAC.INC:3637). Benign at all current call sites (vy provably 0 when called:
    bossseamon states 0/7, flyingfish sets vy after) — fix the helper anyway so a new
    caller can't be bitten.

24. seamon swim vx table read (GASTRATS.ASM:2071): `s_set_alvar2alvartab ...,sintab,-4`
    = sign-extended table byte then 4x adiv (toward ZERO); Rust `SINTAB[sb2] >> 4`
    floors negatives (bosses.rs:1848). Same class as prior-audit finding 8 — use a
    toward-zero shift.

25. nucleuslauncher X-distance boundary (GASTRATS.ASM:63): Xdistmore >= 200 branches
    out — fire needs |dx| < 200; Rust `<= 200` (bosses.rs:3351). (The |worldx| 700/900
    gates are strict ABS more/less and Rust's >=700 / <=900 are EXACT — don't touch.)

26. boss8 core colldisable modeling: Rust toggles mother ASF_COLLDISABLE
    (strat_boss8_init bosses.rs:2805, boss8a_init :2878 clears, boss8b_init :2931 sets);
    ASM gates damage only by swapping collstrat between hitflash_Istrat and 0
    (GB3STRAT.ASM:136, 174-176) with no colldisable. Likely an intentional port
    equivalence — verify the engine's damage path treats collstrat=None as no-damage
    before "fixing" either representation.

27. boss8a HPLASMA muzzle heading: ASM `s_weapon_rot #0,#deg180` fires relative to the
    firer's rots (GB3STRAT.ASM:152-155) and homes afterward; Rust b8_fire_hplasma aims
    directly at the player at spawn (bosses.rs:2543-2569). Port-wide projectile
    convention (same note as bossseamon/seamon RELSLOWELASER via sea_fire_relslowelaser)
    — initial heading differs until homing converges. Accepted approximation unless
    oracle-diffing.

28. Deferred-Istrat fall-throughs where ROM runs init+first-body in one pass and Rust
    splits or shifts them by a tick: bossseamon_Istrat (GA2STRAT.ASM:3056 falls into
    bossseamon_strat; Rust init bosses.rs:1601 doesn't call the body),
    nucleuslauncher_Istrat (GASTRATS.ASM:39 falls through init INTO the strat; Rust
    istrat bosses.rs:3316 sets fields only — first wallrot placement is 1 tick late).
    boss8a/b/wait/die and launcherfire/close chains DO call their bodies (correct).

## Known gaps (pre-existing, not new)
- sea_make_splash still a no-op (bosses.rs:1512) — bossseamon states 0/1/2/3/5/6 and
  seamon landing all call it; ROM also forces splash worldy=0 at the seamon landing site
  (GASTRATS.ASM:2097). Implement together with the prior doc's splash gap.
- boss8 kamimissile / homing-shot internals (b8_fire_kamimissile params vs the
  KAMIHMISSILE1 weapon-table entry, homing shift/thresholds) mirror the C-port weapon
  lane and were not line-diffed here — weapons-lane audit territory.
- makeMED/L/SML expobj lifecnt defaults (makeexpobj_srou) not verified against
  b8_make_exp_obj's count handling (boss8die per-tick exps leave count=0 -> explode next
  tick in Rust); verify when the explosion pipeline is audited.

## s_add_bossHP site list (for the accumulator lane)
| Boss | ASM site | Runs | Rust fn missing the add |
|---|---|---|---|
| boss1 mother | GBSTRATS.ASM:274 (boss1_fin) | every tick, all modes incl. back | enemy_a.rs boss1_finish (1970) |
| boss1 turret x8 | GBSTRATS.ASM:400 (boss1turret_end) | every turret tick | enemy_a.rs boss1turret_common_strat (2340) |
| boss8 core | GB3STRAT.ASM:130 (boss8_cont) | wait/a/b ticks + die countdown | bosses.rs boss8_cont (2847) |
| bossseamon | — none | — | — |
| seamon | — none | — | — |
| boss2 top/turret, bossg | see prior doc finding 1 | | |
boss1 cover, boss8 cover/beams/launcher/pillar contribute NOTHING to m_bossHP (verified:
no add in boss1cov_strat, boss8cov_strat, nucleusbeamL/beam, nucleuslauncher,
nucleuspillar). boss1 turret init adds to bossmaxHP (GBSTRATS.ASM:338) — already ported.

## Verified correct (don't touch)
boss1:
- Mode graph and same-tick chaining: up->normal->in->out->{inclose|normal}->..., each
  ASM init falls into its strat and Rust calls the strat after init identically; back
  entered from boss1_end when svar_byte5 (live-turret count from the 8 dobossrot calls)
  is 0 == Rust live_turret_count()==0 -> boss1back_init.
- normal_strat decbne sbyte2 (30) semantics; in/out speeds -15/+15/-25; inclose target
  boss1_end.nofire == boss1_finish(allow_center_fire=false).
- Center-fire trigger set: fires when NOT(left bank alive AND right bank alive), banks =
  children 2-5 / 6-9; level-1 suppression; weapon_rots2obj player aim for center pair.
- Cover cycle: sbyte2 33->reset 32, sbyte3 10 then 30 (hard) / 50 (easy, ifnotlevel
  polarity checked), sflag3 toggle before the ±4 step, sflag1 set only on moving ticks
  and cleared during the pause window, bf_flag1 set every cover tick and consumed by
  exactly one homing turret shot; trigse $2f placement.
- Turret gating: L fires on sflag3 clear / R on set; nfire resets colanim + nohitaffect;
  fire path colanim &3 wrap + nohitaffect clear BEFORE the cover.sbyte3 gate; cover
  sbyte3 < 20 strictly (alvarLESS) == Rust `>= 20 -> hold`; no-cover -> fire; rotz
  copied from mother every tick; turret roty stays deg180 (mother roty fixed at 180).
- covdie motion (-20 z) and the objinfront polarity (remove only when behind AND far).
- Init: HP 70 / easy 35 with matching bossmaxHP, AP 10, cover AP 16 = boss2covAP,
  turret HP 8 / AP 16, sbyte3=1, colldisable until back mode clears it, cover spawned
  so it ticks before turrets (deliberate list-order note in Rust), boss1exp spin rate
  deg90/32 and count 38 == bossexplode lifecnt 58-20.
bossseamon:
- All 9 states' structure, fall-through same-tick chaining, and setstate2/setstate5
  same-tick re-entry (Rust 'restart loop) — including the state-7 -> state-8 same-tick
  execution and state-7 .ok NOT re-running state 5 the same tick.
- state 0: colldisable set, beqdec sbyte3->setstate2, speedto(20,1), gen_vecs THEN
  vz=-40 THEN sbyte2+=4; notdelay 5 (mask 31, al1pt->idx) dive trigger; sbyte1=10,
  shape sea_0_1, next_state.
- state 1 vecs zero + decbne; state 2 vx negate / vy -15 / add_vecs / beqdec sbyte4;
  state 3 fire gate order (vy>=0 then (gf+idx)&7==0), vy+=1, jmp_higher landing at
  worldy>=0, landing resets (vel 0, sbyte3 30); state 4 == state 0 wait shape; states
  5/6 jump arc (vy -20, +2 gravity, shapes); state 7 Zdistmore>=200-or-infront
  polarity and the chase re-aim (sbyte2, vel 20); state 8 gravity +2 and landing
  assignments (sbyte4=2, state 2, vy=0, shape sea_0_0).
- Tail: s_limit worldx to bridge_minX*2..maxX*2 = ±400 BEFORE add_vecs2pos + add_playerZ.
- (rnd&7)+5 vx magnitude (alvar2rnd has no centering — checklist item does NOT apply).
- Init values (HP2/AP4, rnd sbyte2, roty 180, sbyte3 60, sbyte4 3, noremove_behind).
seamon:
- add_vecs2pos FIRST; swim gate (state==3 || worldy==0); beqdec sbyte3 hold; sbyte2+=4;
  sbyte1 10-tick shape cadence; jmp_lower boundaries at -1 / -30 (branch on >=);
  vy+=2 gravity placement; airborne shape sea_0; fire gate (gf+al1pt)&15 == Rust
  sea_not_delay(idx,4); outZdistrng in-range [500,1000) EXACT in Rust; Xdistmore <200;
  landing latch contents (sflag1, downsea, splash, sbyte3=10, colldisable); .nds clamp
  when vy>=0; jump countdown decbne sbyte4 (wrapping); s_next_state x,#3 wrap ->1;
  per-state jump params (40/-15, 40/-15, 60/-25); upsea + colldisable clear + vx=0 +
  sflag1 clear + splash order.
boss8:
- boss8wait 3-beam gate incl. objptrbad fall-through chain (dead beams 2/3 skip to the
  next check; beam-4-dead -> open).
- boss8_cont: worldz = 210<<3 + player_posz; decbne sbyte4 -> reset 150 + sflag1 toggle;
  speed ramp gate gf&7==0 (no offset in ASM, none in Rust); sflag1 -> ramp toward -5
  else toward +5 with equality stops; gsvar signed compare.
- boss8a: sflag4 set, fire on gf&31 in {25,30}, iflevel-1 -> never close, beqdec
  sbyte2(100) -> close, early close when ANY live beam has sflag1 clear; $73 on open.
- boss8b: sflag4 clear, anim reset only when sflag5 clear, beqdec sbyte2(15) ->
  boss8wait_init, beam sflag1 mass-clear + $72 in init, init falls into body (Rust
  calls it).
- boss8die: achase player worldy->nucleus_viewCY+20 (=-40) and worldx->0, shift 3
  toward-zero (chase_proportional); gf&1 explosion pairs with gf&3 sound un-mute;
  MED+L pair per burst; Shyper (speed 40, roty 180, rotz rnd AT SPAWN — draw order ok,
  colldisable, straight-Istrat deferred == Rust istrat-as-strat); decbne sbyte2(30) ->
  boss8_cont during死 countdown; nullshape -> boss8shrap handoff; playerctrl off ==
  PSF_NOCTRL|PSF_NOFIRE (s_setpctrl_off STRATMAC.INC:7627); boss_dying == s_boss_dying
  (sound $1e + bgm + BF_DYING + PSTF_NOTDIE + fire-off, latched).
- bigexplode == boss8_bigexplode: 5 L-exps lifecnt 1..5, sounds enabled on #2/#4,
  addrnd2posy ±127 x/y, self count 4 + relexplode + delayexplode handoff.
- boss8cov: open/close cap logic (cmp 17/0 BEFORE add — no wrap possible), shape swap
  at the ends only, sflag5 set/clear at the extremes, roty += gsvar_byte1, worldz =
  210<<3 + player_posz; mother-gone -> dead (port addition, safe).
- nucleuswallrot: x = dist*sin, z = dist*cos*2 + 160<<3 + player_posz (yes player_posz —
  the ASM comment says pviewposz but the CODE uses player_posz; Rust b8_wallrot matches
  the code); wallrot2 = *2 + 210<<3 + pviewposz; sbyte2 += gsvar_byte1 after.
- nucleusbeamL: sbyte4 colldisable window (beqdec -> clear at 0); $71 exactly at
  sbyte3==2 BEFORE the dec; sbyte3 latch to 1; nohitaffect clear; bossdead -> explode;
  sflag1 -> colframe=4 hold; colanim &3 wrap (s_add_colanim no-label variant wraps);
  spark spawn gate gf&3==0 with sbyte2/sword2 copy then pos/roty; tail roty =
  sbyte2+deg180 after wallrot.
- nucleusbeamcol: collide clear + hitflash + sflag1 toggle, $70/$71 by new state,
  sbyte4=10, level-1 -> explode, jmpto_strat == calling nucleusbeaml_strat.
- nucleusbeam bolt: wallrot2, roty=sbyte2+180, sword2 -= 50, remove when sword2 <
  100<<3 (strict alvarless == Rust `<`).
- boss8shrap: pviewpos pin + z+1000; beqdec sbyte1(50) -> FOLexp (z-1000, nopolyexp);
  shrap spawn gf&7==0; L-exp gf&1==0; bg2Xscroll = rnd&7 then bg2Yscroll = (rnd&3)+248
  (draw order x then y).
- shrapfall: worldy = pviewposy-500, count 26, worldx = rnd-128+pviewposx, sword1 =
  rnd<<1 (0..510), roty = rnd, draw order x/sword1/roty; tick z = pviewposz+300+sword1,
  y += 35, countdown removal.
- nucleuslauncher: |worldx| in (700..900) via strict ABSmore/less == Rust >=700/<=900
  EXACT; zaco_9 cap (level1: none alive; else count!=2) == Rust count < {1,2}; beqdec
  sbyte3 (rnd%5+1) -> fire mode; fire anim wrap-at-4 with fire exactly at anim==3
  (matches ASM wrap + cmp 3), roty push/deg180/pull == shot roty=180; missile sflag1 ==
  B8_SFLAG1; close: sbyte3=5 every tick, dec-to-0 -> reinit + base strat same tick;
  nuclaunch_cont wallrot + roty=sbyte2+180 + bossdead -> explode; pillar worldy =
  nucleusheight + wallrot + roty=sbyte2 (no +180) — all match.
- boss8 HP doubling: level 1 -> 32 else 64 with matching bossmaxHP (gated on finding 5).
