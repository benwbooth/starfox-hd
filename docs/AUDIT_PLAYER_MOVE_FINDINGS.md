# Player movement / control tick audit (2026-07-07, ASM-verified)

Scope: the player (Arwing) flight/steer/boost/brake/bank + view-follow tick chain,
line-diffed against the 65816 ASM. Death chain (`playerdead_strat`) and pcbox proxies
were audited in prior passes and are NOT re-audited here.

ASM authoritative: `SF/STRAT/PSTRATS.ASM` (playermove_srou 1771-2814, viewmove_srou
1614-1679, do_player_* 3363-3478, playerlimitx_srou 2819-2829, the per-mode fly strats
playeronplanet/inspace/onfield/undergnd/onwater/onbridge/intunnel/oncont 663-1090),
`SF/INC/STRATMAC.INC` (macros), `SF/INC/STRATEQU.INC` (flymode/speed equates),
`SF/INC/GILESALC.INC` / `SF/INC/ALCS.INC` (var + pfm layout), `SF/INC/VARS.INC` (key bits).
Rust: `rust/sf-strat/src/player.rs` + shared helpers in `rust/sf-strat/src/common.rs`.

## Macro / equate facts established for this audit (cite when fixing)

- `medPspeed=65 minPspeed=20 maxPspeed=85` (STRATEQU.INC:346-348). Rust MIN/MED/MAX_PSPEED match.
- `planet_flymode = pfm_diefall!pfm_dieYrot!pfm_shadows!pfm_wobble` (STRATEQU.INC:566).
  **pfm_diefall(1) and pfm_dieYrot(2) are SET during NORMAL planet flight** — they are
  mode capabilities (death-anim style), NOT a "currently dying" flag. `undergnd_flymode`,
  `water_flymode`, `field_flymode` also set both; `space_flymode`/`bigspace_flymode`
  set dieYrot only; `cont_flymode` = dieYrot; the tunnel modes set diefall only
  (STRATEQU.INC:502/566/613/659/731/753/364).
- Each ROM player strat hard-wires ONE `do_player_*` velocity handler via its strat
  pointer — there is no runtime flag test that selects the handler:
  playeronplanet→`do_player_Yvel125`, playerinspace/onfield→`do_player_limitX`,
  playerundergnd/intunnel→`do_playerYvelD2`, playeronwater→`do_player_Yvel125`,
  playeronBridge→`do_player_bridge`(=D2), playeroncont→`do_player_limitX`.
- `do_player_Yvel125` scales vy by **vy + vy>>2 + vy>>3 = ×1.375** (PSTRATS.ASM:3378-3388),
  despite the "125" name. `do_playerYvelD2`: vx=perc62(0.625×), vy=adiv2(vy) (0.5× toward 0).
- Key bits (VARS.INC:55-60): `key_leftl/rightl` = $20/$10 = **TLEFT/TRIGHT shoulders**;
  `key_jleft/jright` = $02/$01 = dpad. `s_jmp_keyup left`/`anyLRkey` = shoulders;
  `s_jmp_keyup jleft` = dpad. Steering (dpad) and shoulder-tilt are separate inputs.
- `s_beqdec_var var,label` (STRATMAC.INC:6391): branch-if-zero is tested BEFORE the
  decrement; when nonzero it decrements and falls through.
- `s_jmp_higher obj,h` (STRATMAC.INC:3072) = jump if worldy<h; `s_jmp_lower obj,h`
  (3098) = jump if worldy>=h.
- viewmove's `s_speedto x,player_tospeed,2` = rate 2; `s_Fchase pviewvelz,al_vz,1` = rate 1.

---

## High

1. **Boost/brake re-fires every frame while X/B held — SFX spam, no cooldown, speed
   pinned.** ROM gates the pad-X/pad-B boost/brake behind two conditions Rust drops:
   `s_jmp_alvarNOTZERO B,x,al_sbyte2,.npsd` (PSTRATS.ASM:2103 — skip the whole boost/brake
   block while the boost/brake timer `sbyte2` is nonzero) and the no-control gate
   `s_jmp_varNE stayblack,#-1 / doingwipe / psf_noctrl / player_noctrlcnt` (2088-2097).
   Rust `boost_brake_update` (player.rs:928-944) checks neither: `if pad1 & X`/`if pad1 & B`
   fire unconditionally. Because it also decrements `sbyte2` at the top (901) and then
   the held key re-sets `sbyte2=20`/`30` (913/923), `sbyte2` is pinned ~19-20, the
   boosting/braking flag never clears, `al.vel` is pinned to max/min, and `play_se(0x32)`
   /`(0x33)` replays EVERY frame. ROM behavior is a pulsed 20-frame boost burst that
   cannot re-trigger until `sbyte2` expires, with the SFX played once
   (`s_jmp_varAND psf2_boosting,.boost` skips the `trigse` when already boosting, 2162-2163),
   and disabled entirely during cutscenes/wipes. Fix: wrap the pad-X/pad-B branches (and
   their `play_se`) in `if al.sbyte2 == 0 { … }`, and skip them when no-control is active
   (mirror the `no_ctrl` gate `playermove_srou` already computes). ROM also gates on the
   boost animation `lda m_boostanim; cmp #40; bcc .npsd` (2105-2107) — include if
   `m_boostanim` is available.

2. **`do_player_Yvel125` vertical-velocity multiplier is 1.25×, ROM is 1.375×.** ROM
   (PSTRATS.ASM:3378-3388): `vy = vy + (vy>>2) + (vy>>3)` (= ×1.375). Rust
   `do_player_yvel125` (player.rs:1435-1436): `al.vy = vy.wrapping_add(vy >> 2)` — only
   `vy + vy>>2` (×1.25), **missing the `+ (vy>>3)` term**. This is the live handler for
   normal on-planet / on-water flight, so every pitch climb/dive is ~9% weaker than ROM
   (vertical control feels sluggish). Fix:
   `al.vy = vy.wrapping_add(vy >> 2).wrapping_add(vy >> 3)`.

3. **`strat_player`'s runtime `do_player_*` dispatch mis-selects the D2 (damped) handler
   for a ROM-faithful planet init.** The ROM picks the velocity handler by strat pointer;
   Rust `strat_player` (player.rs:1554-1560) instead heuristically selects with
   `game_mode==SPACE_MODE → limit_x; playerflymode & (PFM_DIEFALL|PFM_DIEYROT) → yvel_d2;
   else → yvel125`. But `planet_flymode` (and undergnd/water/field) SET diefall+dieYrot
   during normal flight (STRATEQU.INC:566), so the ROM-faithful planet init
   `playeronplanet_init` (player.rs:1851, which sets
   `PFM_DIEFALL|PFM_DIEYROT|PFM_SHADOWS|PFM_WOBBLE`) routes to `do_player_yvel_d2`
   (vx×0.625, vy×0.5) instead of `do_player_yvel125` (vy×1.375) — opposite damping.
   Currently masked only because the live entry path, map callback `SET_PLAYER_ONPLANET_L`
   (game.rs:657-664), deliberately sets `game_mode=0` and `playerflymode |= PFM_SHADOWS`
   WITHOUT the die bits — a workaround, not ROM behavior. Any path through the faithful
   exit-base launch (`player_exitbase_follow_strat` → `playeronplanet_init`) hits the wrong
   handler. Fix: dispatch on the fly-mode identity (a stored SPFM/strat id), not on the
   diefall/dieYrot capability bits; reserve `yvel_d2` for undergnd/tunnel/bridge and
   `limit_x` for space/field/cont, matching the per-strat pointers listed in the facts block.

---

## Medium

4. **`viewmove_srou` drops the `outdist → viewdist` camera-distance ease.** ROM
   (PSTRATS.ASM:1636-1638): `s_jmp_varAND pstratflags,#pstf_novdistC,.novc /
   s_achase_var W,outdist,viewdist,3` — each frame `outdist` chases `viewdist` at rate 3
   (unless PSTF_NOVDISTC). Rust `viewmove_srou` (player.rs:1468-1505) omits it entirely,
   and the camera reads `viewdist` directly (`sf-game/src/camera.rs:244`
   `let dist = if vars.viewdist>0 {vars.viewdist} else {OUTVIEWDIST}`). Result: the camera
   pull-back distance SNAPS to a new `viewdist` instead of easing, so view-distance changes
   (boost, boss-arena entry, mode swaps) read as a camera pop. Fix: restore the
   `outdist = chase_proportional(outdist, viewdist, 3)` step (gated on !PSTF_NOVDISTC) in
   `viewmove_srou` and have the camera consume `outdist` (cross-file: `camera.rs`).

5. **Shoulder-hold banking tilt (the L/R "lean") is not ported.** ROM keeps a keyflags
   tracker + a large ztilt bank driven by the L/R SHOULDER buttons: while a shoulder is
   held, `player_Ztilt += ±deg45/3` (=±10), clamped ±deg90 (PSTRATS.ASM:2599-2640, adds at
   2628/2635; `left`/`right` = key_leftl/rightl = shoulders). Rust's shoulder handling
   (`barrel_roll_update`) only produces the double-tap roll; it never leans `player_Ztilt`,
   and the `player.rs` ztilt only gets the smaller dpad-steer term (deg45/15, 1000/1008).
   So holding L/R no longer banks/leans the Arwing (and the held-shoulder ztilt no longer
   feeds the lateral banking shove). Fix: port the keyflags block — track fresh vs held
   L/R shoulder, add ±deg45/3 to `PLAYER_ZTILT` while held, clamp ±deg90.

6. **Only the planet + space fly-mode view formulas are wired; onfield/undergnd/onwater
   (alt)/intunnel/oncont are not.** Rust collapses everything into `strat_player` +
   `update_viewxy_for_mode` (player.rs:1508-1531), which covers exactly two ROM strats:
   SPACE_MODE = `playerinspace` (perc75 X, perc62(worldy−viewcy)+viewcy Y, inside=direct,
   `do_player_limitX`; PSTRATS.ASM:673-711) and non-space = `playeronplanet` (perc87 X,
   perc75(worldy−viewcy)+viewcy Y, `do_player_Yvel125`; 769-787) — both verified correct.
   But `playeronfield`/`undergnd`/`intunnel` fix view-Y at `viewCY` (Rust makes Y follow
   the player), `intunnel` also decays `outvx/outvy` at rate 2 (`s_achase_var …,2`,
   PSTRATS.ASM:1036-1037) and uses `do_playerYvelD2`, and `playeroncont` (all-range) uses
   direct worldx/worldy view + `outdist/viewdist=200` (728-740). None of these are
   reachable on level 1 (planet) but they are wrong for those later modes. Fix: add the
   per-mode strats/branches with their own view-Y and velocity handler.

---

## Minor

7. **`do_player_yvel_d2` halves vy toward −inf, ROM toward zero.** ROM `do_playerYvelD2`
   (PSTRATS.ASM:3418) `lda al_vy; adiv2` = signed halve toward zero; Rust (player.rs:1453)
   `al.vy >>= 1` = arithmetic shift toward −inf. Off-by-one each frame for upward motion
   (vy<0). (vx uses `perc62` correctly.) Fix: adiv2-style `if vy>=0 {vy>>1} else {-((-vy)>>1)}`.

8. **Player Y-bounds clamp is exclusive and drops the pml_Bbottom gate.** ROM clamps Y
   inclusively inside `playermove_srou` — top `s_jmp_alvarmore worldy,minpmoveY` (clamp
   when worldy<=min, PSTRATS.ASM:1920-1922) and bottom `s_jmp_alvarless worldy,maxpmoveY`
   (clamp when worldy>=max, 1914-1916) — with the bottom clamp gated on
   `pmovelimitAND & pml_Bbottom` (1912-1913). Rust `playerlimit_x_srou` (player.rs:1147-1154)
   uses `<`/`>` (exclusive, loses the edge frame — same class as the fixed X boundary,
   task #34) and clamps the bottom unconditionally. Fix: use `<=`/`>=` and gate the
   bottom clamp on PMOVELIMITAND & PML_BBOTTOM.

9. **Barrel-roll double-tap window is 1 frame tighter than ROM.** ROM
   `s_beqdec_var player_rolldelay,.lragain` (PSTRATS.ASM:2584; branch-if-zero BEFORE
   decrement) lets a fresh shoulder press start the roll whenever `rolldelay>0`; Rust
   (player.rs:860-876) decrements first and only starts when the post-dec delay is exactly
   0, so it needs one extra frame of release between taps. Minor responsiveness loss on the
   defensive roll.

10. **In-flight `pfm_wobble` ship-roll float not applied.** ROM adds the `pZrotfloattab`
    entry `player_Zrotfloat` to `al_rotz` every frame in wobble mode
    (PSTRATS.ASM:2730-2741, add at 2736). Rust `playermove_srou`'s rotz sum
    (player.rs:1095-1099) omits it, so the Arwing loses its idle banking wobble (the const
    `PZROTFLOATTAB_LEN` exists but the table is only used by the intro/opening strats).

11. **`boostobj` not tagged on the pad-X / force boost.** ROM `.boost` does
    `s_set_vartobeobj boostobj,x` (PSTRATS.ASM:2173); Rust `boost_brake_update`
    (player.rs:907-936) sets BOOSTOBJ in neither the force-boost nor the pad-X path
    (only `shipintro`/`friendstart3` set it). The boost visual-effect object is untagged
    on a normal player boost.

12. **Steering ztilt not suppressed near the ground / at a wing wall.** ROM only adds the
    dpad-steer `player_Ztilt` (deg45/15) when not near the floor
    (`s_jmp_lower x,maxPmoveY-30` guard, PSTRATS.ASM:2329-2333/2354-2358) and skips it when
    the wing is against a wall (pml_lwleft/rwright, 2325/2350). Rust adds it on any
    dpad LEFT/RIGHT (player.rs:1000/1008). Cosmetic banking difference near ground/walls.

13. **`viewmove_srou` final copy uses `pviewposz`, ROM uses `viewposz`.** ROM ends with
    `s_copy_var2var W,bgsscrollZ,viewposz` (PSTRATS.ASM:1676; `viewposz` = the fixed-camera
    Z, ALCS.INC:266 — distinct from `pviewposz`, GILESALC.INC:179). Rust copies `pviewposz`
    into BGSSCROLLZ (player.rs:1503-1504, and the PSTF_NOVIEWMOVE early-out 1471-1472).
    Likely an intentional port choice (bg parallax following player depth), but it diverges
    from the ASM — verify against the BGSSCROLLZ consumer in the camera/bg path.

---

## Verified correct (ASM-matched)

- **Banking → lateral shove** (the core steering-glide): `worldx -(adiv2(plrotz>>7) +
  (ztilt>>3 when dpad L/R held))` normal / `+` under turn180, sign and nega/adiv2 toward
  zero all match PSTRATS.ASM:2278-2317 (player.rs:977-994). The `anyJLRkey` gate on the
  ztilt term = dpad L/R (player.rs `lr_held = LEFT|RIGHT`). ✓
- **gf_viewrot view-lean**: outvx `±(-256)` on up/down, outvy `±200` gated to within 300 of
  the X move bound (`minPmoveX+300 / maxPmoveX-300`, jmp direction), noctrl/noYctrl skip,
  both decay `achase …,0,3` — PSTRATS.ASM:1928-1962 (player.rs:1051-1085). ✓
- **Dpad steering**: LEFT `plrotz+=ZROT, plroty+=ZROT, ztilt+=deg45/15 (clamp deg90)`,
  RIGHT the negatives — PSTRATS.ASM:2334-2359 (player.rs:996-1012). ✓
- **Rotation-decay achase rates**: ztilt 3, plroty 3, plrotz 4, plrotx 3 —
  PSTRATS.ASM:2647/2658/2684/2694 (player.rs:1032-1037). ✓
- **Clamps**: vel [20,85], plrotz [−0x600,0x600] — PSTRATS.ASM:2701-2703 (player.rs:1040-1042). ✓
- **al_rot composition**: rotx=plrotx>>8; roty=plroty>>8+turnrot>>8;
  rotz=plrotz>>8+ztilt+zshake>>8+zstratadd+rollzoff — PSTRATS.ASM:2704-2724
  (player.rs:1093-1099). ✓ (except the wobble float, Minor 10.)
- **viewmove Z chase**: `pviewposz+=pviewvelz`; `pviewvelz=Fchase(pviewvelz,al_vz,1)`;
  `range −200,50` on (pviewposz−player_posz); `sbyte2<=10 → tospeed=medspeed +
  achase(pviewposz,player_posz,3)`; `speedto tospeed,2` — PSTRATS.ASM:1641-1663
  (player.rs:1476-1494). ✓
- **update_viewxy**: SPACE=playerinspace (perc75 X, perc62 Y, inside=direct), non-space=
  playeronplanet (perc87 X, perc75 Y) — PSTRATS.ASM:673-711 / 769-787 (player.rs:1508-1531). ✓
- **spfm_inside outvz** = ztilt + zshake — PSTRATS.ASM:2749-2756 (player.rs:1105-1109);
  and `viewmove` inside copy of worldx/y/z into pviewpos — 1668-1671 (player.rs:1496-1501). ✓
- **Barrel-roll advance**: `rollZoff += rollZvel`; `rollZvel ∓2 toward 0`; idle
  `achase(rollZoff,0,3)`; start on fresh L/R SHOULDER (TLEFT|TRIGHT) with right→+32/left→−32
  — PSTRATS.ASM:2582-2596/2713-2724 (player.rs:853-895); shoulder-mask confirmed vs
  STRATMAC.INC:1389-1403. ✓ (double-tap timing is Minor 9.)
- **playerlimitx_srou** inclusive X boundary + LEFT/RIGHT arrows — PSTRATS.ASM:2820-2828
  (player.rs:1135-1142). ✓
- **checkarrows_srou**: PSTF_INSEQ→0; per-arrow AND of pad + PMOVELIMITAND bit —
  PSTRATS.ASM:3482-... (player.rs:1160-1181). ✓
- **strat_speed_to** overflow-safe snap (rate 1/2 here) — common.rs:325-349. ✓
- **gen_3dvecs / apply_velocity** yaw-negation + fixed-point — common.rs:402-461
  (oracle-verified elsewhere). ✓
