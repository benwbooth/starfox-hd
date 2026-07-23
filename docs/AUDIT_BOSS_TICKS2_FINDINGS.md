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

1. ~~boss1: five fire-rate constants misread as `% N`~~ **FIXED (verified tick 196):**
   home `(gf+idx)&15`, norm `(gf+idx)&31`, center/back missiles `(gf+15)&63`,
   back plasma `gf&63`. Test `boss1_fire_gates_are_bitmasks_not_modulus`.

2. ~~boss1back_strat hold/retreat INVERTED~~ **FIXED (verified tick 196):**
   `|dz|<1500` retreats (+15); `|dz|>=1500` bombards. Test
   `boss1back_retreats_when_closer_than_1500`.

3. ~~boss1 child/muzzle offsets missing <<1/<<2 + rotz~~ **FIXED (verified tick 196):**
   ring table doubled + `boss1_rot_offset_pos` full rotate_8*; center ±384;
   turret muzzle z+40. Test `boss1_ring_and_muzzle_scales`.

4. ~~m_bossHP per-frame accumulator~~ **FIXED (verified tick 196):**
   mother `boss1_finish`, turret end, `boss8_cont` call `add_bosshp`. Test
   `boss1_and_boss8_add_bosshp_each_tick`.

5. ~~currentlevel is never wired~~ **FIXED (tick 134):** `Shell::begin_gameplay_from_planet_select`
   writes `wm::CURRENTLEVEL` (0x1F03) = `planets.currentlevel + 1` (port encoding so
   `currentlevel() == N` matches ROM `s_jmp_iflevel N`). Also corrected boss7 hard-route
   gates from `== 2` to `== 3` (`s_jmp_ifnotlevel 3`, GB3STRAT.ASM:3169/3539/3645).
   Tests: shell `begin_gameplay_wires_currentlevel_*` + `currentlevel_wram.rs`.

## Medium

6. ~~boss1 back-mode HPLASMA spread + aim base~~ **FIXED (verified tick 135):** firer
   rots + `(rnd&15)-7` pitch-first into `sbyte1/2` (homingflat_Istrat); tests
   `boss1_back_out.rs`.

7. ~~boss1 back-mode HMISSILE1 offset applied to the wrong axis~~ **FIXED (verified
   tick 135):** pitch ±(deg45-deg11) on firer, yaw stays deg180; tests
   `boss1_back_out.rs`.

8. ~~boss1out_strat cycle counter order~~ **FIXED (verified tick 135):** `s_beqdec`
   tests before dec — first out-pass (sbyte3=1) → normal; second (0) → inclose;
   tests `boss1_back_out.rs`.

9. ~~boss1 death chain simplified vs bossexplode_Istrat~~ **FIXED (verified tick 136):**
   `boss1exp_init` → release children + tempstrat spin + `strat_boss_explode_init`
   (`s_boss_dying` $1e+$f1, BF_DYING/PSTF_NOTDIE/SF_NOFIRING, 14 staggered SML/MED/L
   + circdelayexplode, lifecnt 38, bossdelayexplode); rotz += deg90/32 via
   `boss1exp_spin_strat`. Tests `boss1_death.rs` (2).

10. ~~bossseamon state 8 splash-down randomization~~ **FIXED (verified tick 137):**
    draws `vx=(rnd&7)+5` FIRST, then negate when `rnd>=127`. Tests
    `boss_ticks2_mediums.rs`.

11. ~~seamon post-landing surface snap missing~~ **FIXED (verified tick 137):**
    sflag1-latched band clamps `worldy=0, vy=0`. Tests `boss_ticks2_mediums.rs`.

12. ~~seamon swim-shape toggle whole-byte test~~ **FIXED (verified tick 137):**
    post-landing (sflag1+colldisable) forces sea_0_0; pre-landing alternates.
    Tests `boss_ticks2_mediums.rs`.

13. ~~boss8a open-flap animation wraps~~ **FIXED (verified tick 137):** caps at 14
    (`if animframe < 14 { += 1 }`). Tests `boss_ticks2_mediums.rs`.

14. ~~nucleuslauncher missing objinfront gate~~ **FIXED (verified tick 137):** arms
    only while `player.z < launcher.z`. Tests `boss_ticks2_mediums.rs`.

15. ~~boss8die clears bossmaxhp~~ **FIXED (verified tick 137):** die leaves
    `bossmaxhp` alone (ROM never touches it). Tests `boss_ticks2_mediums.rs`.

16. ~~boss8die Shyper debris wrong view variable~~ **FIXED (verified tick 138):**
    uses `viewposy` (0x0552), not pviewposy. Tests `boss_ticks2_minors.rs`.

17. ~~boss8shrap RNG stream/order drift~~ **FIXED (verified tick 138):** deferred
    shrapfall Istrat; `b8_add_rnd_xyz` x,y,z <<1; `b8_add_rnd2pos_folexp` 3 draws
    (±63). Tests `boss_ticks2_minors.rs`.

## Minor

18. ~~boss1up rise boundary~~ **FIXED (verified tick 138):** strict `worldy < CY`
    (passes through ==CY). Tests `boss_ticks2_minors.rs`.

19. ~~boss1in / inclose / covdie Zdistmore inclusive~~ **FIXED (verified tick 138):**
    hold at |dz| >= 1000/300; covdie remove at >= 1000. Tests `boss_ticks2_minors.rs`.

20. ~~boss1 center missiles missing enemy1 colltype~~ **FIXED (verified tick 138):**
    center pair `|= COLLTYPE_ENEMY1`; back-mode pair do not. Tests
    `boss_ticks2_minors.rs`.

21. ~~boss1back far-path → boss1_fin~~ **FIXED (tick 138):** far path does
    `add_player_z` + `add_bosshp` only (no finish double-spin / center-fire /
    boss1rots). Near path still uses full `boss1_finish`. Tests
    `boss_ticks2_minors.rs`.

22. ~~bossseamonexp gsvar_byte1 wrap~~ **FIXED (verified tick 138):**
    `wrapping_sub` 0→255. Tests `boss_ticks2_minors.rs`.

23. ~~sea_gen_vecs_angle zeroes vy~~ **FIXED (verified tick 138):** leaves vy
    untouched. Tests `boss_ticks2_minors.rs`.

24. ~~seamon swim vx toward-zero~~ **FIXED (verified tick 138):** `SINTAB / 16`
    (not `>> 4`). Tests `boss_ticks2_minors.rs`.

25. ~~nucleuslauncher X-distance boundary~~ **FIXED (verified tick 138):** fire
    needs |dx| < 200 (strict). Tests `boss_ticks2_minors.rs`.

26. ~~boss8 core colldisable modeling~~ **VERIFIED EQUIV (tick 139):** port folds
    ROM `s_docoll` into coldet; `ASF_COLLDISABLE` + `collstrat=None` ≡ ROM
    `collstrat=0` (coldet skips colldisable; do_strat clears collide when no
    collstrat). Open (`boss8a`) clears disable + sets hitflash; close (`boss8b`)
    restores both. Tests `boss_ticks2_gaps.rs`.

27. ~~boss8a HPLASMA muzzle heading~~ **ACCEPTED (tick 139):** firer-relative
    `#0,#deg180` vs aim-at-player at spawn — port-wide projectile convention;
    homes afterward. No code change.

28. ~~Deferred-Istrat fall-throughs~~ **FIXED (verified tick 139):**
    `strat_bossseamon_init` / `nucleuslauncher_istrat` already call body same
    tick (wallrot / state0). Tests `boss_ticks2_gaps.rs`.

## Known gaps (pre-existing, not new)
- ~~sea_make_splash no-op~~ **FIXED (tick 139):** wires `makesplash_srou`; seamon
  landing also forces splash `worldy=0` (GASTRATS.ASM:2101). Tests
  `boss_ticks2_gaps.rs`.
- ~~boss8 kamimissile simplified strat~~ **FIXED (tick 144):**
  `b8_fire_kamimissile` → push roty=#deg180 + `fire_kami_hmissile1` /
  `hmissile3_*` + restore roty + `al_ptr=playpt` + sflag1; live-count by
  `#zaco_9` shape (GASTRATS.ASM:103-110). Tests `boss8_kami_hmissile3.rs` (3).
- ~~makeMED/L/SML expobj lifecnt defaults~~ **FIXED (tick 140):** `makeexpobj_srou`
  leaves `al_count=0` (ROM never sets lifecnt); boss8die per-tick exps explode on
  first delayexplode tick. `boss8_delayexplode_strat` / `boss2_delayexplode_strat`
  now use `count_down` (= `s_decbpl_lifecnt`: die when entry count was 0, survive
  `count+1` ticks) — was one frame early when lifecnt set (e.g. bigexplode 1..5).
  Tests `expobj_lifecnt.rs` (4).

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
