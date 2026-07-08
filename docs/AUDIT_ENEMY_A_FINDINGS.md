# enemy_a NON-boss strat tick audit findings (2026-07-04, ASM-verified)

Scope: the non-boss1 strategies in `rust/sf-strat/src/enemy_a.rs` — shared
damage/fire/aim/move helpers, hard/scenery objects, raders, pillar3, skillfly,
gates, zaco0/1/2/3/4/szaco2/zacos, worms, houdai, tadpole, spacebar walker+shooter,
up1man/item0, items 5/7 + flashplayer, bomwing, para, carrier, base1, cameleon,
friendexitbase, the clship CL_SHIP/DIVE/UNDER/TURN/BRIDGE/EARTH/CHASE clear demos,
and the EXPSTRAT explosion/death helpers. boss1/turret/back was audited separately
in docs/AUDIT_BOSS_TICKS2_FINDINGS.md and is SKIPPED here.

ASM is the sole ground truth: the C oracle `src/strat/strat_enemy.c` no longer
exists in this checkout (removed post-RIIR). Sources: GASTRATS.ASM, GA2STRAT.ASM,
GSTRATS.ASM, KSTRATS.ASM, DSTRATS.ASM, D2STRATS.ASM, GCSTRATS.ASM, GISTRATS.ASM,
EXPSTRAT.ASM; macros STRATMAC.INC / STRATLIB.INC / STRATEQU.INC / STRATROU.ASM.
The authoritative STRATMAC macro-fact block is at the top of
docs/AUDIT_BOSS_TICKS2_FINDINGS.md (lines 10-29) — cited throughout.

Macro facts most load-bearing for THIS audit (re-verified):
- `s_jmp_notdelay/ifdelay N,label[,off]` = `(gameframe [+off]) & ((1<<N)-1)==0`.
  N is a BIT COUNT (period 2^N), NEVER a modulus. `al1pt` off = per-object stagger.
- `s_achase_alvar` (== `s_Achase_alvar`, case-insensitive; STRATMAC.INC:4939) is the
  PROPORTIONAL chase (`diff>>shift` toward zero). Rust `chase()` is LINEAR
  (common.rs:264); `chase_proportional()` is the proportional one (common.rs:304).
- `s_jmp_higher x,#v` (STRATMAC.INC:3072) = `cmp v; rlbmi` → branch when `worldy < v`;
  `s_jmp_lower x,#v` (STRATMAC.INC:3098) = branch when `worldy >= v`. Smaller y = higher.
- `s_beqdec_alvar var,label` (STRATMAC.INC:6286) = TEST-then-DEC: `if var==0 goto label
  else var-=1`. Contrast `s_decbeq`/`s_decbne` = DEC-then-test (`--var`).
- distmore `|d|>=v` inclusive; distless strict `<`; outdistrng in-range `min<=|d|<max`;
  `s_jmp_XYdistmore` uses the COMBINED `rangexy=|dx|+|dy|` metric, not a per-axis box.
- muzzle offset = `(raw<<scale)>>weapon_scale`, weapon_scale=2 (STRATEQU.INC:838).
- `s_weapon_rndrot m,m` = per-axis `(rnd&m)-m/2`, PITCH(x) drawn before YAW(y).

## High

1. **Shared `strat_fire_relslowlaser` fires at the wrong speed and lifetime — hits
   7+ enemy strats.** ASM `fire_relslowElaser` (GSTRATS.ASM:2548-2561): speed via
   `doelaserspeed` (GSTRATS.ASM:2780 = 48 at level 1, else 60), `lifecnt 40`,
   ap `enemylaserAP=2`. Rust (enemy_a.rs:429-431) hardcodes `speed 52, life 55`.
   The sibling `strat_fire_relslowlaserhome` (enemy_a.rs:443, life 40, speed via
   `strat_relslowelaser_speed` 48/60) is correct — proving intent. Callers of the
   broken non-home helper: zacos (enemy_a.rs:3678,3727), zaco3 (:3922), zaco4 (:4101),
   zaco0 (:4229), cameleon (:4599), szaco2 (:4764). Fix: `spawn_projectile(g,
   Some(idx), 0,0,0, pitch, yaw, strat_relslowelaser_speed(g), 40, 2, ACF_COLLTYPE4)`.
   (See Minor 1 for the missing laser colltype + muzzle Z on both helpers.)

2. **`frame_tick_mod` treats the delay as a modulus, not a bit-count (shared helper).**
   `s_jmp_notdelay N` gates on `gameframe & ((1<<N)-1)==0` (period 2^N;
   STRATMAC.INC:6456-6468). Rust (enemy_a.rs:347-349) returns `gameframe % step == 0`
   and callers pass the ASM `N` directly, so `mod(1)` is always-true (should be every
   2nd frame), `mod(2)` every-2nd (should be 4th), `mod(3)` every-3rd (should be 8th).
   Every clship banking/roll/decel rate runs too fast. clship callers
   enemy_a.rs:5056,5076,5143,5316,5364,5414 vs ASM GCSTRATS.ASM:53,59,70,76,219,223,
   419,423,880,903,907. Fix: `g.vars.gameframe & ((1u16 << step) - 1) == 0`. Shared —
   audit enemy_b.rs callers to confirm they also pass the bit-count before flipping.

3. **`houdai_strat` fire gate: wrong mask + dropped `al1pt` stagger.** ASM
   GASTRATS.ASM:1309 `s_jmp_notdelay 4,.nfindobj,al1pt` = fire when `(gameframe+idx)&0x0F
   ==0` (every 16 frames, staggered). Rust (enemy_a.rs:3820) `if gameframe & 3 != 0
   { return }` = every 4 frames, in lockstep (~4x too fast). Fix: `if (g.vars.gameframe
   as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F != 0 { return; }`. The sibling
   spacebarwalker (enemy_a.rs:3385) already does this correctly for the same macro.

4. **Per-object fire staggers with wrong mask AND missing `al1pt` — three sites.**
   `s_jmp_notdelay N,...,al1pt` = `(gameframe+idx)&((1<<N)-1)==0`.
   - `zaco0_fire` enemy_a.rs:4222 `gameframe & 1 == 0` vs KSTRATS.ASM:241
     `notdelay 2,.nfire,al1pt` → `(gf+idx)&3==0` (2x too fast, no stagger).
   - `para2_strat` enemy_a.rs:4357 `gameframe & 3 == 0` vs D2STRATS.ASM:587
     `notdelay 4,.njump,al1pt` → `(gf+idx)&15==0` (hop impulse 4x too frequent).
   - `cameleon_phase1` enemy_a.rs:4594 `gameframe & 3 == 0` vs DSTRATS.ASM:1546
     `notdelay 4,...,al1pt` → `(gf+idx)&15==0`.
   Fix each to `(g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & mask`.

5. **Five inverted `worldy` comparisons (`jmp_higher`/`jmp_lower` misread).** Smaller y
   is higher; `jmp_higher` branches when `worldy < v`, `jmp_lower` when `worldy >= v`.
   - `gate2_strat` ground clamp: ASM GA2STRAT.ASM:2658 `s_jmp_higher x,#-30<<1,.ngnd`
     → clamp `worldy=-60` runs when `worldy >= -60` (a floor). Rust (enemy_a.rs:1211)
     clamps when `worldy <= GATE2_GROUND_Y` — opposite half-space. Fix: `>=`.
   - `zaco2_cont` ground bounce: ASM GASTRATS.ASM:1097 `s_jmp_higher x,#0,.ngnd` →
     block runs when `worldy >= 0`. Rust (enemy_a.rs:2640) `if worldy <= 0`. Fix: `>=`.
     (Sibling `para2_strat` enemy_a.rs:4311 uses the correct `>= 0` for the same clamp.)
   - `zacos_phase0` pitch/fire gate: ASM GASTRATS.ASM:950 `s_jmp_higher x,svar_word1,
     .nup` → block runs when `worldy >= player_posy-800`. Rust (enemy_a.rs:3675)
     `if worldy <= target_y`. Fix: `>=`.
   - `zaco3die_strat` land trigger: ASM KSTRATS.ASM:171 `s_jmp_lower x,#-100,
     zaco3go_init` → branch when `worldy >= -100`. Rust (enemy_a.rs:4011) `if worldy
     < -100`. Fix: `>= -100` (then run zaco3go_init + zaco3go_strat same frame).
   - `zaco1_cont` ceiling clamp: ASM GASTRATS.ASM:1219 `s_jmp_higher x,#0,.hok` →
     `worldy=0` runs when `worldy >= 0` (ceiling). Rust (enemy_a.rs:4802) `if worldy
     < 0`. Fix: `>=`.

6. **`zaco3_circle` / `zaco4_circle` chase `worldy` linearly; ASM uses proportional
   Achase.** ASM KSTRATS.ASM:139/141 `s_Achase_alvar W,x,al_worldy,#-200/#-60,1`
   (16-bit proportional). Rust (enemy_a.rs:3950, 4117) `chase(al.worldy, target, 1)`
   = linear ±1/frame. Fix: `chase_proportional(al.worldy, target_y, 1)`. (Sibling
   `carrierb_strat` enemy_a.rs:4469 correctly uses `chase_proportional`.)

7. **`parajump_strat` chases `worldy`/`worldx` linearly; ASM uses proportional
   Achase.** ASM D2STRATS.ASM:600/604 `s_achase_alvar W,x,al_worldy,player_posy,2`
   and `al_worldx,player_posx,3` (proportional). Rust (enemy_a.rs:4379/4385) uses
   linear `chase`. Fix: `chase_proportional(..., 2)` / `(..., 3)`.

8. **Every clship position chase is linear; ASM uses proportional Achase.** All clship
   demos chase `al_worldx/y/z` with `s_achase_alvar`; Rust uses `chase()`. The whole
   approach trajectory is wrong (asymptotic vs constant-velocity) every frame. Sites →
   ASM (GCSTRATS.ASM):
   - `clship_warp_cont` z/y enemy_a.rs:5110-5111 → :148,:152
   - `clship_warpc_strat` x enemy_a.rs:5175 → :132
   - `clship_gnd_cont` z/y enemy_a.rs:5146-5147 → :213,:217
   - `clship_gnda/b/c_strat` x enemy_a.rs:5183,5191,5199 → :175,:189,:202
   - `clship_cont` z/y enemy_a.rs:5319-5320 → :413,:417
   - `clship_eartha/b/c_strat` x enemy_a.rs:5338,5347,5356 → :271,:291,:311
   - `clship_chase_cont` z/y enemy_a.rs:5416-5417 → :871,:875
   - `clship_chasea/b/c_strat` x enemy_a.rs:5439,5447,5455 → :824,:844,:858
   Fix: swap `chase(cur,target,N)` → `chase_proportional(cur,target,N)` at each. The
   rotation chases (`achase_angle`) are already correct.

9. **`clship_chase_cont` transitions to the wrong boost on timer expiry.** ASM
   GCSTRATS.ASM:866 `s_beqdec_alvar W,x,al_sword1,clshipboost_Istrat` — on `sword1==0`
   jump to the GENERAL boost (GCSTRATS.ASM:234: `trigse $32`, `set_speed 120`,
   straight-line flyaway, removed after `sbyte2`). Rust (enemy_a.rs:5406-5408) enters
   the chase-specific `clship_chaseboost_enter`/`_step` (speed 20, 2D vecs, never
   removed) — wrong behavior. Fix: on `sword1==0` call `clshipboost_enter(g,idx,true);
   clshipboost_step(g,idx); return;`. Related dead code: `clship_chaseboost_step`
   (enemy_a.rs:5378) re-arms forever instead of transitioning (ASM GCSTRATS.ASM:912
   also `beqdec → clshipboost_Istrat`).

10. **`base1_strat` implements the wrong machine entirely.** ASM `base1_strat`
    (KSTRATS.ASM:380) is a HIT-TRIGGERED door: idle until `s_test_hitflags x,#HF1`,
    then open (anim 0→8) with `dooropensound_l`, wait `sbyte1=5`, close (anim 8→0)
    with `doorclosesound_l`, re-init. Rust (enemy_a.rs:4521) is a free-running timer
    FSM keyed on `BASE1_PHASE_FLAG`/`BASE1_WAIT_FRAMES=10`, SE 0x59/0x5A, never testing
    hit flags. `strat_base1_init` (enemy_a.rs:4506) also diverges: ASM sets
    `alptrs base1_strat,0,0` (null coll+exp), `aldata #hardhp,#2` (ap=2),
    `roty=deg180`; Rust sets coll=hit_flash, exp=explode, ap=HARD_AP(8), no roty, adds
    ASF_NOHITAFFECT. Fix: reimplement as the hit-triggered door; init `collstratptr=None,
    expstratptr=None, ap=2, roty=DEG180`, drop the added flag. CAVEAT: the reference is
    the ultrastarfox hack — confirm base1 against the original disassembly, as the C
    oracle may have targeted a different base1.

## Medium

11. **`zaco2loop_strat` circle turn direction inverted.** ASM GASTRATS.ASM:1120-1127
    `s_jmp_rightofview` branches to `.tright` when `leftpl` CLEAR (STRATMAC.INC:6176);
    `.tright` (leftpl clear) does `rotz+=10; roty+=4`, else `rotz-=10; roty-=4`. Rust
    (enemy_a.rs:2674-2680) has them reversed. Fix: swap the two branches.

12. **`wormgo_strat` drift direction inverted.** ASM GASTRATS.ASM:2243-2246
    `s_leftview_strat x,.gl` branches to `.gl` when `leftpl` SET → `vx+=1`, else
    `vx-=1`. Rust (enemy_a.rs:2837-2841) reversed. Fix: LEFT_PL set → `+1`, else `-1`.

13. **`itemtorange_srou` height comparison inverted.** ASM GASTRATS.ASM:3159-3164
    `s_jmp_lower x,svar_word1,.iny` skips the add when `worldy >= minpmoveY+50`, so
    `worldy+=3` runs only when `worldy < minpmoveY+50`. Rust (enemy_a.rs:2984)
    `if worldy >= min_y { += 3 }` — reversed (affects item7/item7a settling). Fix: `<`.

14. **`zaco3_attack` / `zaco4_attack` `s_beqdec` off-by-one (fires once too few, circles
    early).** ASM KSTRATS.ASM:118 `s_beqdec_alvar B,x,al_sbyte1,.circle` = test-then-dec:
    with `sbyte1=2` fires on 2→1 and 1→0 then circles on the 3rd tick. Rust
    (enemy_a.rs:3908, 4091) uses guarded `--x==0` (fires once, circles a tick early).
    Fix: `if sbyte1==0 { circle } else { sbyte1-=1; fire }`.

15. **`cameleon_phase1` `s_beqdec` off-by-one.** ASM DSTRATS.ASM:1545 `s_beqdec_alvar
    B,x,al_sbyte1,...`; Rust (enemy_a.rs:4584-4588) uses `--x==0`. Same fix as 14.

16. **`zaco4_attack` / `zaco4_circle` do not run the next phase the same frame.** ASM
    `.circle` (KSTRATS.ASM:127) and `.flyaway` (:144) fall into their strat bodies the
    same tick. Rust (enemy_a.rs:4094-4104, 4122-4124) only sets the pointer then runs
    `strat_move3d`, deferring one tick. (`zaco3_attack`/`zaco3_circle` do it correctly.)
    Fix: after setup, `zaco4_circle(g,idx); return;` / `zaco4_flyaway(g,idx); return;`.

17. **`zaco4_flyaway` uses a live worldx compare instead of the view-side flag.** ASM
    shares zaco3's `.flyaway` using `s_jmp_rightofview` (`afleftpl`; KSTRATS.ASM:149).
    Rust (enemy_a.rs:4135) `me.worldx > p.worldx`. `zaco3_flyaway` (:3962) does it right.
    Fix: `if target.is_none() || al.flags & AF_LEFT_PL == 0 { target_yaw = 30; }`.

18. **`zaco3_die_strat` rotx pitch cap uses unsigned compare.** ASM KSTRATS.ASM:172
    `s_jmp_alvarMORE B,x,al_rotx,#deg45` is SIGNED (STRATMAC.INC:6652): `+4` while
    `(i8)rotx <= deg45`. Rust (enemy_a.rs:4018) `if al.rotx <= DEG45` is unsigned — stops
    for rotx in [33..255] (e.g. -30=226), never wrapping up through 0. Fix:
    `if (al.rotx as i8) <= DEG45 as i8`.

19. **`zaco3go_strat` regenerates velocity when close; ASM keeps stale vecs.** ASM
    KSTRATS.ASM:195 `s_jmp_Zdistless x,y,#400,zaco3cont` branches PAST `s_gen_3dvecs`
    when `|zdist|<400`. Rust (enemy_a.rs:4047) always calls `gen_vecs_3d`. Fix: only
    gen_vecs when `zdist >= 400`.

20. **`para_strat`→para2 transition: missing `s_initface_player`, runs para2 a frame
    early.** ASM `para2_istrat` (D2STRATS.ASM:569-577) does `s_initface_player`
    (stores sbyte3/sbyte4 aim + smflag1) and ends with `s_end_strat`. Rust
    (enemy_a.rs:4312-4322) omits initface and calls `para2_strat` the same tick. Fix:
    perform initface (store target angles/set smflag1), don't call para2 on the switch.

21. **`para2_strat` re-aims at the live player; ASM homes toward precomputed angles.**
    ASM D2STRATS.ASM:580 `s_face_player x,1,0,.nogen` achases roty→sbyte3, rotx→sbyte4
    (fixed at transition, gated on smflag1). Rust (enemy_a.rs:4332-4335) continuously
    re-aims via `angle_xz`/`strat_pitch_toward`. Fix: achase toward the stored sbyte3/4.

22. **`para2_strat` gravity magnitude wrong.** ASM D2STRATS.ASM:592 `s_falldown_Yvec
    x,1,#3,#0` adds +3 to vy. Rust (enemy_a.rs:4364) adds +1. Fix: `vy += 3`.

23. **`item5_strat` missing player-dead removal.** ASM GASTRATS.ASM:2571
    `s_remove_ifplayerdead x` removes when `pshipflags2 & psf2_playerHP0` is set
    (HP0, not object existence). Rust (enemy_a.rs:2959) only checks `player().is_none()`.
    (`item7_strat` enemy_a.rs:3069 correctly tests PSF2_PLAYERHP0.) Fix: also gate on
    `g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0`.

24. **`item5_collect` missing `specflash = 30`.** ASM GASTRATS.ASM:2586 inside the
    `specwepcnt<5` block: `s_set_var B,specflash,#30`. Rust (enemy_a.rs:2945-2954)
    increments count, plays 0x18, adds score, but never sets specflash (HUD flash).
    Fix: write `specflash = 30` in the `cnt < ITEM5_MAX_SPEC` branch.

25. **`item7_strat` repair path diverges from ASM.** ASM GASTRATS.ASM:2934-2956 spawns
    a `ripair_w` pod (`ripair_Istrat`) that flies to the player over ~30 frames, plays
    `$17`, clears the 4 wing flags, and re-inits the wing objects; the not-broken branch
    re-inits `pLWing_Istrat`/`pRWing_Istrat`. Rust (enemy_a.rs:3094-3111) clears the
    flags instantly and skips the pod + wing re-init. Flag set matches; timing/re-init
    differ. Likely intentional simplification — reported for fidelity.

26. **`up1man_strat` scrolls worldz when `sbyte3==0`.** ASM GASTRATS.ASM:2728
    `s_jmp_alvarZERO B,x,al_sbyte3,.ninrng` early-outs to strat END when `sbyte3==0`,
    skipping rotz, `worldz+=30`, and the item spawn. Rust (enemy_a.rs:3566-3574) guards
    only the rotz add; the scroll runs unconditionally. Since `sbyte3` starts 0 (never
    set in init), the mother should be static until a child is hit. Fix: `if me.sbyte3
    == 0 { return; }` after the player fetch.

27. **`clship_cont` chases the player during the space-boost countdown.** ASM
    GCSTRATS.ASM:397 `s_decbne_alvar B,x,al_sbyte1,.nplayerchase` branches PAST the
    normal chase (:411-425) while `sbyte1` counts down, doing only `add_playerZ`. Rust
    (enemy_a.rs:5300-5313) only returns when `sbyte1==0`; while counting down it falls
    through to the chase. Fix: whenever the `sflag1 && player-sflag4` branch is taken,
    skip the chase (move `add_player_z; return;` to the end of the whole `if`, not just
    the `sbyte1==0` case).

28. **`clship_warp_cont` omits the boost sound.** ASM `clshipWARP_cont` (:143) jumps to
    `clshipboost_Istrat` (:234 `trigse $32`). Rust (enemy_a.rs:5102) calls
    `clshipboost_enter(g,idx,false)` — no `snd2=0x32`. Fix: pass `true`.

29. **`clship_chaseboost_enter` plays a sound the ASM does not.** ASM
    `clshipCHASEboost_Istrat` (GCSTRATS.ASM:891-894) has no `trigse`. Rust
    (enemy_a.rs:5395) sets `snd2=0x32`. Fix: drop it (folds into the boost-transition
    fix, item 9).

30. **`zaco1_phase2` zeroes `sword2`/`ptr` with no ASM basis.** ASM `zaco1b_strat`
    (GASTRATS.ASM:1238-1276) sets sword2/ptr only in `.circ` and never zeroes them
    elsewhere; leaving `.circ` the last spiral offsets keep being added by zaco1_cont.
    Rust (enemy_a.rs:4944-4945, 4948-4949) resets both in the mid/far branches. Fix:
    remove the `sword2 = 0; ptr = 0;` from the two non-`.circ` branches.

31. **`friendexitbase_strat` left/right channel sound inverted at the boundary.** ASM
    GISTRATS.ASM:326-332 `s_beqdec_alvar B,x,al_sbyte2,.left` — always decrement
    (wrapping), play LEFT `0x51` only the single frame the result hits 0, else RIGHT
    `0xB1`. Rust (enemy_a.rs:4992-4997) never wraps, plays right until sbyte2 reaches 0
    then left every frame after. Fix: `al.sbyte2 = al.sbyte2.wrapping_sub(1); al.snd1 =
    if al.sbyte2 == 0 { 0x51 } else { 0xB1 };`.

32. **`gate2_strat` touch test uses a per-axis box; ASM uses combined XY distance.**
    ASM GA2STRAT.ASM:2670 `s_jmp_XYdistmore x,y,#30<<1,.ntouch` → touch when
    `rangexy=|dx|+|dy| < 60`. Rust (enemy_a.rs:1228-1231) `dx<=60 && dy<=60` (square,
    inclusive). Fix: require `|dx|+|dy| < GATE2_TOUCH_XY`.

33. **`skillfly_strat` "flew-behind" removal wrongly decrements the ring counter.** ASM
    DSTRATS.ASM:8465-8479: the `jmp_objinfront → .rem` path removes WITHOUT
    `s_dec_var skillfly`; only the caught path decrements. Rust (enemy_a.rs:1036-1039)
    calls `skillfly_remove(g)` (which decrements) on the behind path — corrupting the
    skill-ring bonus. Fix: set `g.objs.aldead = 1;` directly there (no decrement).

34. **`strat_hard90yr_init` adds a colltype the ASM does not.** ASM `hard90YR_Istrat`
    (KSTRATS.ASM:326-331) has no `s_set_colltype` (unlike hard180YR). Rust
    (enemy_a.rs:829) sets `COLLTYPE_ENEMY1`, making inert scenery enemy-collidable.
    Fix: drop the `collflags |= COLLTYPE_ENEMY1`.

35. **`delayexplode_strat` / `bossdelayexplode_strat` / `circdelayexplode_strat` fire one
    frame early.** ASM uses `s_decbpl_lifecnt` (EXPSTRAT.ASM:262,:53,:280) = die when the
    decrement goes NEGATIVE (entry count 0), surviving count+1 ticks. Rust inline
    (enemy_a.rs:5620-5624, 5702-5706, 5669-5673) `if count>0 {count-=1} if count==0
    {die}` fires at entry count 1 — one frame early. Fix: use `strat_count_down(al)`
    (common.rs:534, already correct). `delayremove_strat` (enemy_a.rs:5637) is correct
    (`decbne`) and must stay inline. (Same root cause makes `strat_qboss_explode_init`
    enemy_a.rs:5734 fire the circexp a frame late — Minor.)

36. **`pillar3explode_strat` drops the 8-object explosion chain and plays a wrong
    sound.** ASM `pillarexplode_Istrat` (EXPSTRAT.ASM:1078-1113) spawns 8 medium-exp
    children along a rotz-rotated line (counts `8-z1`, nopolyexp, worldz-10), sets
    pillar `lifecnt=7`, jumps to `delayremove_Istrat`, and plays NO direct sound (each
    child has noexpsnd). Rust (enemy_a.rs:960-971) spawns none, plays `play_se(0x10)`
    (the item-catch chime), and sets AFEXP (not in ASM). Fix: drop `play_se(0x10)`;
    spawn the 8 children (or route the visual to the renderer). Related:
    `pillar3_enter_fall` (enemy_a.rs:928) omits the bouncyball spawn (DSTRATS.ASM:804).

37. **Missing init `_Istrat → _strat` same-frame fall-through (first-tick delay).** In
    ROM these `_Istrat` blocks have their `_strat` label immediately after with no
    `s_end_strat`, so the body runs on the spawn frame. tadpole/up1man/pillar3/gate*
    already model this; these do NOT:
    - `strat_spacebarwalker_init` enemy_a.rs:3406 → GA2STRAT.ASM:1788
    - `strat_spacebarshoot_init` enemy_a.rs:3438 → GA2STRAT.ASM:1814
    - `strat_zacos_init` enemy_a.rs:3652 → GASTRATS.ASM:942
    - `strat_houdai_init` enemy_a.rs:3840 → GASTRATS.ASM:1292
    - `item0_istrat` enemy_a.rs:3455 → GASTRATS.ASM:2678
    - `strat_skillfly_init` enemy_a.rs:1045 → DSTRATS.ASM:8438
    - `strat_zaco1l/r_init` (via zaco1_common_init) enemy_a.rs:4813 → GASTRATS.ASM:1227
    Fix: append the matching strat call at the end of each init. (`houdaiNS`/`tower0`
    correctly do NOT — their `_Istrat` ends with `s_end_strat`, GASTRATS.ASM:1153,1285.)

## Minor

1. **Shared relslowlaser helpers: missing laser colltype + muzzle Z offset.** ASM
   `fire_relslowElaser`/`Home` (GSTRATS.ASM:2554-2557, 2569-2572) set BOTH
   `enemyweap`(acf_colltype4) AND `laser`(acf_colltype1=8) colltypes and add a Z muzzle
   `elaserfireZoff(80)>>weapon_scale(2)=20` / `80>>2=20` (rotated by the firer's rots).
   Rust helpers (enemy_a.rs:430, 445) pass only `ACF_COLLTYPE4` and off_z=0. Fix: OR in
   `ACF_COLLTYPE1` and pass a +20 forward muzzle offset (rotated) if bit-exact.

2. **`relelaserhome_strat` lock latch boundary inclusive.** ASM GSTRATS.ASM:1916
   `s_jmp_Zdistmore x,y,#800,.nmin` skips the lock when `|dz|>=800`, so lock latches at
   strictly `|dz|<800`. Rust (enemy_a.rs:1689-1690) latches at `<= 800`. Fix: `<`.

3. **Item5/item7/item0/tadpole/up1man distance boundaries inclusive where ASM is
   strict.** `jmp_distmore` proceed-path is `|d| < dist`.
   - item5 pickup enemy_a.rs:2969,2974 (`> 120/60`) → GASTRATS.ASM (should collect only
     `< 120/60`); item7 :3086,3091 same.
   - item0 pickup enemy_a.rs:3488,3491,3494 (`>`) → GASTRATS.ASM:2685-2687.
   - up1man scroll gate enemy_a.rs:3571 (`<= 1500`) → GASTRATS.ASM:2731.
   - tadpole fire enemy_a.rs:3274 (`<= 1500`) → GA2STRAT.ASM:2937.
   - bomwing_phase2 fire enemy_a.rs:3188 (`<= 3000`) → GASTRATS.ASM:2542 (fire `< 3000`).
   - zaco2loop reset enemy_a.rs:2686 (`> 2000`) → GASTRATS.ASM:1132 (reset `>= 2000`).
   - carrier_strat enemy_a.rs:4433 (`> 3000`) → KSTRATS.ASM:284 (`>= 3000`).
   - zaco1_phase0 enemy_a.rs:4887 (`> 1000`) → GASTRATS.ASM:1215 (`>= 1000`).
   - gate3 dz/dxy enemy_a.rs:1106,1109; gate dz/dxy :1153-1155; gate2 dz :1226 —
     inclusive vs strict per GA2STRAT.ASM:2616-2617,:2669 / DSTRATS.ASM:1755,1758.
   Fix: use `<` / `>=` (or drop constants by 1) at each.

4. **`zaco1_phase2` mid-band upper bound inclusive.** ASM fires `1400<=|zdist|<1800`
   (GASTRATS.ASM:1243 excludes 1800). Rust (enemy_a.rs:4932) `(1400..=1800)`. Fix:
   `(1400..1800)`.

5. **`zaco0_fire` random spread: modulo vs mask, wrong draw order.** ASM
   `s_weapon_rndrots2obj y,3,3` (KSTRATS.ASM:244) = per-axis `(rnd&3)-1` ∈ {-1,0,1,2},
   PITCH then YAW. Rust (enemy_a.rs:4225-4228) `strat_random_centered(3)` = `(rnd%3)-1`
   ∈ {-1,0,1}, YAW then PITCH. Fix: `(rnd&3)-1` per axis, pitch-first draw order.

6. **`zacos` laser muzzle Z-offset dropped.** ASM zacos2/3 fire with
   `s_weapon_pos #0,#0,#40>>weapon_scale` (=+10 z) before RELSLOWELASER
   (GASTRATS.ASM:967,991). Rust (enemy_a.rs:3678,3727) spawns at (0,0,0). Cosmetic.

7. **`clship_flyinleft`/`flyinright`: `sflag1` set not gated by the notdelay.** ASM
   GCSTRATS.ASM:53-57 puts both the `vx==-5→set sflag1` and `vx-=1` inside the
   `notdelay 1` gate. Rust (enemy_a.rs:5062-5066, 5082-5086) gates only `vx-=1`. Fix:
   gate the flag-set too. (Masked today by High 2.)

8. **`zaco2loop_init` adds an `aliens[0].active` guard before firing.** ASM
   GASTRATS.ASM:1107-1114 fires HMISSILE1 unconditionally on level!=1. Rust
   (enemy_a.rs:2661-2665) wraps it in `if aliens[0].active`, which can suppress a
   missile the ROM spawns. Fix: fire unconditionally (target = player).

9. **`bomwing`/`cameleon`/`strat_bomwing_init` extra colltypes vs ASM.** ASM
   `bomwing_Istrat` (GASTRATS.ASM:2515-2523) and `cameleon_istrat` (DSTRATS.ASM:1527)
   set no colltype. Rust adds `COLLTYPE_ENEMY1` (enemy_a.rs:3231, 4631). Reconcile
   against object/shape spawn defaults; if defaults don't already include enemy1, the
   ROM objects are not laser-collidable and the Rust add is a deviation.

10. **`flashplayer_istrat` writes `colframe=0` (not in ASM).** ASM `flashplayer_Istrat`
    (GASTRATS.ASM:3130-3132) does not touch colframe. Rust (enemy_a.rs:3028) adds it.
    Cosmetic color-anim phase.

11. **`strat_gate_init` hardcodes restart map bank 0.** ASM `gate_Istrat`
    (DSTRATS.ASM:1746) stores `maprestartbanktemp = mapbank`. Rust (enemy_a.rs:1192)
    writes 0. Harmless only if map data is always bank 0.

12. **`gate_strat`/`gate3_strat` skip the spin/colanim step on the touch frame.** ASM
    falls through the heal branch into the spin (DSTRATS.ASM:1786; GA2STRAT gate3
    continues to `init_colanim`). Rust (enemy_a.rs:1172) `return`s after the checkpoint
    latch, deferring one frame (colframe 4 vs ROM 5). Cosmetic.

13. **`pillar3explode_wait` counts one frame long.** Uses `count_down` (fires at
    count+1); ASM sets `lifecnt=7`+`delayremove` (`decbne`, fires at count),
    EXPSTRAT.ASM:1112. Fix: inline `decbne`.

14. **`strat_explode` may omit the in-view gate + special→gate_2 reward (needs
    verification).** ASM `explode_Icont` (EXPSTRAT.ASM:705) silently `remove_strat`s
    when not `inviewpl`; `explode_Istrat` (:693) spawns a `gate_2` heal ring for
    `special` objects. Rust `strat_explode` (enemy_a.rs:768-803) does neither. Confirm
    whether the port creates the explosion sprite / gate reward in the collision/object
    layer before acting.

15. **`zaco1`/`zaco4`/`zaco3go`/`para2` etc. cosmetic/1-tick class items:** newly-set
    zaco1 phase runs a frame late (enemy_a.rs:4890,4908 vs GASTRATS.ASM:1227,1235);
    zaco1_phase2 spiral uses float sin/cos + arithmetic (toward -inf) shift instead of
    ROM tables + toward-zero (enemy_a.rs:4929 vs GASTRATS.ASM:1257); zaco3die/zaco3go
    omit `makesmoke` particles (KSTRATS.ASM:168,186); para2 first `add_vecs2pos` omits
    vy (enemy_a.rs:4344, benign while gen_vecs_2d ran); zaco0_sweep signed-vs-unsigned
    worldy compares (enemy_a.rs:4184,4187, benign in-band); `zaco1_phase2` roty target
    not negated vs `s_obj2obj_3Dangle`'s `nega(Yanglexy)` — inconsistent with
    szaco2_waypoint_yaw (:4648) and zaco1_phase0 (:4870) which do negate; verify the
    arctan16 sign convention before flipping enemy_a.rs:4954. `szaco2` init defers the
    `relexplode` sflag (GA2STRAT.ASM:238) — known debris gap.

16. **`strat_hard_init` always sets `COLLTYPE_ENEMY1` (table-lane check).** ASM `hard_Istrat`
    (GSTRATS.ASM:642) has no colltype; `hardenemy1_Istrat` (:639) does. Both are
    registered (ISTRATS.ASM:528,663). If the ISTRATS row for plain `hard` maps to
    `strat_hard_init`, it wrongly gains enemy1. Depends on table wiring outside this file.

## Verified correct (don't touch)
- Shared: `achase_angle` (proportional, toward-zero, reached-before-step), `strat_aim_yaw/3d`,
  `strat_move3d`, `strat_pitch_toward`, `angle_xz` (int-promotion no-wrap), `add_player_z`,
  `set_hard_vars`, `strat_relslowelaser_speed` (48/60), `relelaserhome_strat` anim/lock/homing
  (aside from Minor 2), `homingflat_strat`.
- Damage/death: `strat_hit_flash` (HARD_HP invuln guard + $24/$25/$26 range sounds),
  `strat_explode` specials accounting ($21/$22/$23 + noexpsnd gate; aside from Minor 14),
  hard180yr/nzr/hardrot/nocoll inits, `addrnd2pos_xy`, `copy_pos`, `make_*_exp_obj`,
  `boss_dying` ($1e+$f1+flags order), `delayremove_strat`, `strat_boss_explode_init` (all 14
  timed children in factory order, lifecnts 5..34, circdelayexplode proxy, count 38 tail,
  GF_BOSSDEAD only in bossdelayexplode).
- rader0/1, pillar3 (dist/hp/HF2 fall triggers, sbyte1=±4, $49 landing; aside from the
  bouncyball spawn), gate3/gate/gate2 heal+checkpoint latch cores.
- zaco2 (istrat data, beqdec order, Zdistless<500, dash count 30 kill), worms (link-dead
  killtype1/2 split logic, sin/cos shift -4, wormsplit 2-draw (rnd&63)-32, worm2 next_state
  wrap 4→1), item5/item7 progression, flashplayer copy+toggle, bomwing phase FSM
  (aside from the noted gates), tadpole full FSM (achase deg90, sflag1 latch, TADPOLE_BANK
  =40, constants), spacebarwalker/shoot (objinfront, spacemist bucket, correct notdelay),
  zacos phases 1/2/3, tower0, houdai target/aim/muzzle (aside from High 3), item0/up1man
  child offsets + hit handler + child positioning, zaco3/4 no-target branches + init data,
  zaco0 turn_in/out/flyaway + fire transitions, para swing FSM + worldy>=0 transition,
  carrier spawn/init/carrierb (chase_proportional!)/carrierc, cameleon_phase2.
- szaco2 (init data, next_state 0→1→2→3, all state transitions + shifts 3/2/2, bank_to_player
  vs sr_banktoplayer, fire band [400,1500) mask 7 + al1pt), zaco1 L/R + common init + phase0/1,
  friendexitbase init (lifecnt 30, sbyte2=11), ALL clship inits (frogwait 30/bunny 60/cock 90/
  gnd 110/warp_btime 430, EARTH rnd&15 then rnd&7 order), all clship `_cont` z/y offset signs
  and rotation chase shifts, clshipboost_step removal countdown, clship_cont dead-check
  (|z-viewposZ|>=4000) + once-only $32 latch, clship_float2 table cycling.
