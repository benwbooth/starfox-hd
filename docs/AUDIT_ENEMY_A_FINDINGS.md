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

1. ~~**Shared `strat_fire_relslowlaser` fires at the wrong speed and lifetime — hits
   7+ enemy strats.**~~ **FIXED (verified tick 150):**
   `doelaserspeed` 48@L1 / 60 else, `lifecnt 40`, AP 2, colltypes
   `enemyweap|laser` (GSTRATS.ASM:2548-2561). Tests
   `enemy_a_weapon_explode_se::relslowlaser_speed_life_colltypes_match_rom`.

2. ~~**`frame_tick_mod` treats the delay as a modulus, not a bit-count (shared helper).**~~
   **FIXED (verified tick 150):** `gameframe & ((1<<N)-1)==0` (period 2^N;
   STRATMAC.INC:6456-6468). Test
   `enemy_a_weapon_explode_se::frame_tick_mod_is_bit_count_not_modulus`.
   Callers pass the ASM bit-count `N` (clship banking/roll/decel).

3. ~~**`houdai_strat` fire gate: wrong mask + dropped `al1pt` stagger.**~~
   **FIXED (verified tick 147):** `(gameframe+idx)&0x0F==0` (GASTRATS.ASM:1309
   `notdelay 4,...,al1pt`). Tests `houdai_cadence.rs`.

4. ~~**Per-object fire staggers with wrong mask AND missing `al1pt` — three sites.**~~
   **FIXED (verified tick 147):**
   - `zaco0_fire` → `(gf+idx)&3==0` (KSTRATS.ASM:241 `notdelay 2,al1pt`)
   - `para2_strat` hop → `(gf+idx)&0x0F==0` (D2STRATS.ASM:587)
   - `cameleon_phase1` → `(gf+idx)&0x0F==0` (DSTRATS.ASM:1546)
   Tests `houdai_cadence.rs` (zaco0c) + prior para/cameleon coverage.

5. ~~**Five inverted `worldy` comparisons (`jmp_higher`/`jmp_lower` misread).**~~
   **FIXED (verified tick 151):** smaller y = higher; clamp/fire/land when
   `worldy >= v` (STRATMAC `s_jmp_higher`/`s_jmp_lower`):
   - `gate2_strat` floor `-60` (GA2STRAT.ASM:2658)
   - `zaco2_cont` ground bounce `0` (GASTRATS.ASM:1097)
   - `zacos_phase0` pitch/fire vs `player_posy-800` (GASTRATS.ASM:950)
   - `zaco3die_strat` land `-100` (KSTRATS.ASM:171)
   - `zaco1_cont` ceiling `0` (GASTRATS.ASM:1219)
   Tests `enemy_a_worldy_higher.rs` (5).

6. ~~**`zaco3_circle` / `zaco4_circle` chase `worldy` linearly; ASM uses proportional
   Achase.**~~ **FIXED (verified tick 152):** `chase_proportional(..., 1)` toward
   `-60`/`-200` (KSTRATS.ASM:139/141). Tests `enemy_a_achase_clship.rs`.

7. ~~**`parajump_strat` chases `worldy`/`worldx` linearly; ASM uses proportional
   Achase.**~~ **FIXED (verified tick 152):** `chase_proportional` worldy shift-2 /
   worldx shift-3 (D2STRATS.ASM:600/604). Tests `enemy_a_achase_clship.rs`.

8. ~~**Every clship position chase is linear; ASM uses proportional Achase.**~~
   **FIXED (verified tick 152):** all WARP/GND/EARTH/CHASE (and SHIP/TURN/…)
   position chases use `chase_proportional` (GCSTRATS.ASM). Tests
   `enemy_a_achase_clship.rs` + `clship_families.rs`.

9. ~~**`clship_chase_cont` transitions to the wrong boost on timer expiry.**~~
   **FIXED (verified tick 152):** `sword1==0` → `clshipboost_enter` (vel 120,
   snd2 `$32`) — general boost, not chaseboost (GCSTRATS.ASM:866/234). Test
   `clship_chase_expires_into_general_boost`.

10. ~~**`base1_strat` implements the wrong machine entirely.**~~
    **FIXED (verified tick 153):** hit-triggered door — idle until HF1 →
    open anim 0→8 + `DoorOpen` → wait `sbyte1=5` → `DoorClose` → close
    anim 8→0 → re-init; init null coll/exp, hardhp, ap=2, roty=deg180
    (KSTRATS.ASM:373-408). Tests `base1_door.rs` (3) + sound F1/F2.

## Medium

11. ~~**`zaco2loop_strat` circle turn direction inverted.**~~
    **FIXED (verified tick 154):** leftpl SET → rotz/roty −10/−4; CLEAR → +10/+4
    (GASTRATS.ASM:1120-1127). Test `zaco2loop_turn_follows_leftpl`.

12. ~~**`wormgo_strat` drift direction inverted.**~~
    **FIXED (verified tick 154):** leftpl SET → vx+=1; else vx-=1
    (GASTRATS.ASM:2243-2246). Test `wormgo_drift_follows_leftpl`.

13. ~~**`itemtorange_srou` height comparison inverted.**~~
    **FIXED (verified tick 154):** `worldy+=3` only when `worldy < minpmoveY+50`
    (GASTRATS.ASM:3159-3164). Test `itemtorange_raises_only_when_higher_than_floor`.

14. ~~**`zaco3_attack` / `zaco4_attack` `s_beqdec` off-by-one.**~~
    **FIXED (verified tick 154):** TEST-then-DEC — sbyte1=2 fires twice then
    circles (KSTRATS.ASM:118). Test `zaco3_beqdec_fires_twice_then_circles`.

15. ~~**`cameleon_phase1` `s_beqdec` off-by-one.**~~
    **FIXED (verified tick 154):** TEST-then-DEC → phase2 same tick
    (DSTRATS.ASM:1545). Test `cameleon_beqdec_transitions_when_zero`.

16. ~~**`zaco4_attack` / `zaco4_circle` do not run the next phase the same frame.**~~
    **FIXED (verified tick 155):** `.circle` / `.flyaway` fall through same tick
    (KSTRATS.ASM:127/144). Tests `enemy_a_mediums_16_22.rs`.

17. ~~**`zaco4_flyaway` uses a live worldx compare instead of the view-side flag.**~~
    **FIXED (verified tick 155):** `AF_LEFT_PL` CLEAR → yaw +30 (KSTRATS.ASM:149).
    Test `zaco4_flyaway_uses_leftpl_not_worldx`.

18. ~~**`zaco3_die_strat` rotx pitch cap uses unsigned compare.**~~
    **FIXED (verified tick 155):** signed `(i8)rotx <= deg45` (KSTRATS.ASM:172).
    Test `zaco3die_signed_rotx_cap_climbs_from_negative`.

19. ~~**`zaco3go_strat` regenerates velocity when close; ASM keeps stale vecs.**~~
    **FIXED (verified tick 155):** `|dz|<400` skips `gen_vecs_3d` (KSTRATS.ASM:195).
    Test `zaco3go_keeps_stale_vecs_when_close`.

20. ~~**`para_strat`→para2 transition: missing `s_initface_player`, runs para2 a frame
    early.**~~ **FIXED (verified tick 155):** clears `smflag1`, no para2 same tick
    (D2STRATS.ASM:569-577). Test `para_to_para2_initface_latches_aim`.

21. ~~**`para2_strat` re-aims at the live player; ASM homes toward precomputed angles.**~~
    **FIXED (verified tick 155):** latch sbyte3/4 on first tick, achase stored aim
    (D2STRATS.ASM:580). Test `para_to_para2_initface_latches_aim`.

22. ~~**`para2_strat` gravity magnitude wrong.**~~
    **FIXED (verified tick 155):** `vy += 3` (D2STRATS.ASM:592). Test
    `para2_gravity_adds_three`.

23. ~~**`item5_strat` missing player-dead removal.**~~
    **FIXED (verified tick 156):** `PSF2_PLAYERHP0` → remove (GASTRATS.ASM:2571).
    Test `item5_removes_when_player_hp0`.

24. ~~**`item5_collect` missing `specflash = 30`.**~~
    **FIXED (verified tick 156):** `specflash=#30` in `specwepcnt<5` block
    (GASTRATS.ASM:2586). Test `item5_collect_sets_specflash_30`.

25. ~~**`item7_strat` repair path diverges from ASM.**~~
    **FIXED (tick 203):** broken-wing pickup `s_make_obj #ripair_w` →
    `ripair_Istrat` (SE `$8b`); repair + `$17` deferred to ripair catch;
    intact path keeps `$15`/score/doublaser/beamball; `item7_Istrat` falls
    through. Tests `item7_ripair_spawn.rs` (4).
    (Was: ACCEPTED inline repair simplification.)

26. ~~**`up1man_strat` scrolls worldz when `sbyte3==0`.**~~
    **FIXED (verified tick 156):** early-out when `sbyte3==0` (GASTRATS.ASM:2728).
    Test `up1man_static_while_sbyte3_zero`.

27. ~~**`clship_cont` chases the player during the space-boost countdown.**~~
    **FIXED (verified tick 156):** FLAG1+player-sflag4 path skips chase
    (GCSTRATS.ASM:397). Test `clship_cont_countdown_skips_chase`.

28. ~~**`clship_warp_cont` omits the boost sound.**~~
    **FIXED (verified tick 156):** `clshipboost_enter(..., true)` → snd2 `$32`
    (GCSTRATS.ASM:143/234). Test `clship_warp_boost_plays_sound`.

29. ~~**`clship_chaseboost_enter` plays a sound the ASM does not.**~~
    **FIXED (superseded tick 152 High #9):** chase expiry routes to general
    `clshipboost` only; chaseboost path removed.

30. ~~**`zaco1_phase2` zeroes `sword2`/`ptr` with no ASM basis.**~~
    **FIXED (verified tick 156):** mid/far bands retain spiral offsets
    (GASTRATS.ASM:1238-1276). Test `zaco1_phase2_retains_spiral_offsets_outside_circ`.

31. ~~**`friendexitbase_strat` left/right channel sound inverted at the boundary.**~~
    **FIXED (verified tick 156):** `s_beqdec` — RIGHT while dec, LEFT while
    `sbyte2==0` (GISTRATS.ASM:326-332). Test `friendexitbase_beqdec_snd_channels`.

32. ~~**`gate2_strat` touch test uses a per-axis box; ASM uses combined XY distance.**~~
    **FIXED (verified tick 156):** `|dx|+|dy| < 60` (GA2STRAT.ASM:2670). Test
    `gate2_touch_uses_combined_rangexy`.

33. ~~**`skillfly_strat` "flew-behind" removal wrongly decrements the ring counter.**~~
    **FIXED (verified tick 156):** behind → `aldead=1` only, no `skillfly` dec
    (DSTRATS.ASM:8465-8479). Test `skillfly_behind_removes_without_decrement`.

34. ~~**`strat_hard90yr_init` adds a colltype the ASM does not.**~~ **FIXED (verified tick 157):**
    no `COLLTYPE_ENEMY1` (unlike hard180YR). Test `hard90yr_has_no_enemy1_colltype`.

35. ~~**`delayexplode_strat` / `bossdelayexplode_strat` / `circdelayexplode_strat` fire one
    frame early.**~~ **FIXED (verified tick 157):** `count_down` / `s_decbpl` — entry
    count 1 survives first tick. Test `delayexplode_count_one_survives_first_tick`
    (+ `expobj_lifecnt.rs`).

36. ~~**`pillar3explode_strat` drops the 8-object explosion chain and plays a wrong
    sound.**~~ **FIXED (verified tick 157):** 8 nopolyexp medium children along rotz
    line, lifecnt 7 → delayremove, no direct SE. Test
    `pillar3explode_spawns_eight_silent_children`.
    ~~(bouncyball on fall still Minor.)~~ **FIXED (tick 208):** `pillar3fall_i` /
    `pillar3ffall_i` spawn bouncyball→explode×3→kill_obj (z−10 on pillar3 only);
    pillar3f rightview roll sign corrected. Tests `pillar3_fall_bouncyball.rs` (5).

37. ~~**Missing init `_Istrat → _strat` same-frame fall-through (first-tick delay).**~~
    **FIXED (verified tick 157):** skillfly/spacebarshoot/houdai/zacos/zaco1 (+walker/
    item0) call strat body on spawn frame. Tests `*_init_runs_*_same_frame` (5).
    (`houdaiNS`/`tower0` correctly do NOT.)

## Minor

1. ~~**Shared relslowlaser helpers: missing laser colltype + muzzle Z offset.**~~
   **FIXED (tick 158):** colltypes `enemyweap|laser`; muzzle local Z 80
   (`elaserfireZoff` after `<<weapon_scale`) rotated by firer full rots.
   Tests `relslowlaser_muzzle_*` + prior colltype test.

2. ~~**`relelaserhome_strat` lock latch boundary inclusive.**~~ **FIXED (tick 158):**
   latch when `|dz|<800` (ASM `Zdistmore #800` skips at `>=`). Test
   `relelaserhome_lock_strict_less_than_800`.

3. ~~**Item5/item7/item0/tadpole/up1man distance boundaries inclusive where ASM is
   strict.**~~ **FIXED (tick 159):** `jmp_distmore` proceed-path is `|d|<dist`.
   Fixed remaining tadpole fire `<1500` and zaco1_phase0 `>=1000`; other sites
   (item5/7/0, up1man, bomwing, zaco2loop, carrier, gates) already correct.
   Tests `item5_pickup_strict_less_than_120`, `tadpole_fire_strict_less_than_1500`,
   `zaco1_phase0_transitions_at_dz_1000`.

4. ~~**`zaco1_phase2` mid-band upper bound inclusive.**~~ **FIXED (verified tick 159):**
   `(1400..1800)` excludes 1800. Test `zaco1_phase2_midband_excludes_1800`.

5. ~~**`zaco0_fire` random spread: modulo vs mask, wrong draw order.**~~
   **FIXED (verified tick 159):** `(rnd&3)-1` pitch-then-yaw. Test
   `zaco0_fire_spread_mask_pitch_then_yaw`.

6. ~~**`zacos` laser muzzle Z-offset dropped.**~~ **FIXED (tick 160):**
   `s_weapon_pos #0,#0,#40>>weapon_scale` on top of elaserfireZoff → world
   muzzle Z 120. Test `zacos_muzzle_is_weapon_pos_plus_elaserfirezoff`.

7. ~~**`clship_flyinleft`/`flyinright`: `sflag1` set not gated by the notdelay.**~~
   **FIXED (verified tick 160):** both flag-set and vx step inside `notdelay 1`.
   Tests `clship_flyin*_sflag1_gated_by_notdelay`.

8. ~~**`zaco2loop_init` adds an `aliens[0].active` guard before firing.**~~
   **FIXED (verified tick 160):** HMISSILE1 fires unconditionally on level!=1.
   Test `zaco2loop_fires_hmissile_on_non_easy_unconditionally`.

9. ~~**`bomwing`/`cameleon`/`strat_bomwing_init` extra colltypes vs ASM.**~~
   **FIXED (tick 161):** no `COLLTYPE_ENEMY1` (ASM sets none). Tests
   `bomwing_init_has_no_enemy1_colltype`, `cameleon_init_has_no_enemy1_colltype`.

10. ~~**`flashplayer_istrat` writes `colframe=0` (not in ASM).**~~ **FIXED (tick 161):**
    leaves colframe untouched. Test `flashplayer_istrat_leaves_colframe_untouched`.

11. ~~**`strat_gate_init` hardcodes restart map bank 0.**~~ **FIXED (tick 161):**
    `MAPRESTARTBANKTEMP = $7E` (HD map VM WRAM bank). Test `gate_init_stores_mapbank_7e`.

12. ~~**`gate_strat`/`gate3_strat` skip the spin/colanim step on the touch frame.**~~
    **FIXED (tick 161):** gate touch falls through to `gate_spin_strat` same frame.
    (gate3 already spun before touch.) Test `gate_touch_runs_spin_same_frame`.

13. ~~**`pillar3explode_wait` counts one frame long.**~~ **FIXED (superseded tick 157
    Medium #36):** pillar3explode uses `delayremove` (`decbne`) with lifecnt=7.

14. ~~**`strat_explode` may omit the in-view gate + special→gate_2 reward.**~~
    **FIXED (tick 161):** special → spawn `gate_2`+`gate2_Istrat`; not `inviewpl` →
    silent remove (no destruct SE). Tests `explode_special_spawns_gate2`,
    `explode_not_inview_removes_silently`.

15. ~~**`zaco1`/`zaco4`/`zaco3go`/`para2` etc. cosmetic/1-tick class items:**~~
    **FIXED (tick 162, actionable subset):** zaco1 phase0→1 / phase1→2 fall-through
    same frame; phase2 spiral uses `bee_tab_scaled` SINTAB/COSTAB toward-zero
    (`sintab,-3` / `costab,-2`); `szaco2` init sets `ASF2_RELEXPLODE`.
    **FIXED (tick 164):** zaco3die/zaco3go `makesmoke` (notdelay 1 / 2 + go smoke
    `vz=#40`); szaco2 `debrisshape=105` (`zaco_8` stand-in for missing `zaco_8p`);
    zaco3die double `add_player_z` removed.
    **FIXED (tick 165):** movement aim uses `nega(Yanglexy)` — `zaco1_phase2`,
    `strat_aim_yaw`/`strat_aim_3d`, para2 initface latch — matching ROM
    `s_obj2obj_3Dangle` + `gen_vecs_3d` yaw negation (angle_xz VERIFIED tick 163).
    Weapon fire keeps raw `angle_xz` (fire_weapon Yanglexabs has no nega).
    **FIXED (tick 166):** remaining inline `s_obj2obj_*` / `s_face_player` /
    projectile-obj2obj sites — headfire, helpballhome, homingflat, spacebarwalker
    body, stbfp/bee1a latch, evader, cam2dash, sdragonfly `3DangleOFF`, blowcube
    `3DangleOFF`, bonfire + ironball2/3/4 aim — all store `nega(Yanglexy)`.
    **FIXED (tick 167):** true `zaco_8p` mesh — `SHAPE_EXT_ZACO_8P=283` from
    SHAPES2.ASM via `tools/shape_compiler.py` EXTENDED_SHAPES; szaco2
    `debrisshape` uses it (no longer zaco_8/105 stand-in).
    **FIXED (tick 171):** para2 `s_gen_vecs` → `strat_gen_vecs_nvecs` (no longer
    `gen_vecs_2d` which zeroed vy); first `add_vecs2pos` is full xyz matching
    ASM. zaco0_sweep worldy climb/clamp use unsigned `s_cmp`/`cmp #-30` semantics.
    Tests `enemy_a_minors_15_16.rs` + `enemy_a_minors_15_smoke.rs` +
    `enemy_a_minors_15_yaw.rs` + `enemy_a_inline_aim_yaw.rs` + `zaco_8p_debris.rs` +
    `enemy_a_minors_15_leftovers.rs`. **#15 leftovers closed.**
16. ~~**`strat_hard_init` always sets `COLLTYPE_ENEMY1` (table-lane check).**~~
    **FIXED (tick 162):** `hard_Istrat` drops enemy1; `hardenemy1_Istrat` keeps it;
    ISTRATS index 104 wired via `IS_HARDENEMY1`. Tests
    `hard_init_has_no_enemy1_colltype`, `hardenemy1_sets_enemy1_and_is_registered`.

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
- rader0/1, pillar3 (dist/hp/HF2 fall triggers, sbyte1=±4, $49 landing, fall
  bouncyball→explode flash), gate3/gate/gate2 heal+checkpoint latch cores.
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
