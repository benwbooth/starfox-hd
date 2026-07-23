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

1. ~~**Boost/brake re-fires every frame while X/B held**~~ **FIXED (verified tick 197):**
   `sbyte2!=0` gate + noctrl/stayblack/wipe/noctrlcnt; pulsed 20/30-frame
   burst, SE once per pulse. Tests `player_boost_brake.rs` (3).
   (`m_boostanim>=40` meter gate remains render-lane follow-up.)

2. ~~**`do_player_Yvel125` vertical-velocity multiplier is 1.25×, ROM is 1.375×.**~~ **FIXED**
   (earlier): `vy + vy>>2 + vy>>3`.

3. ~~**`strat_player`'s runtime `do_player_*` dispatch mis-selects the D2 (damped) handler
   for a ROM-faithful planet init.**~~ **FIXED** (tick 125): `playeronplanet_init` clears
   `game_mode` to 0 (planet); `set_player_in_space` / `on_water` set SPACE/WATER.
   `strat_player` uses SPACE→`limit_x`, else→`yvel125`. Dedicated undergnd/tunnel/bridge
   strats keep `yvel_d2`. Tests `planet_yvel_ztilt.rs`.

---

## Medium

4. ~~**`viewmove_srou` drops the `outdist → viewdist` camera-distance ease.**~~ **FIXED**
   (tick 123): chase in `viewmove_srou` + `PSTF_NOVDISTC` gate; camera consumes `outdist`.

5. ~~**Shoulder-hold banking tilt (the L/R "lean") is not ported.**~~ **FIXED** (tick 122):
   `player_Ztilt ±= deg45/3` on TLEFT/TRIGHT.

6. ~~**Only the planet + space fly-mode view formulas are wired; onfield/undergnd/…**~~
   **FIXED** (ticks 123–125): dedicated strats + planet `game_mode=0` so `strat_player`
   picks `yvel125` after exit-base / `set_player_on_planet`.

---

## Minor

7. ~~**`do_player_yvel_d2` halves vy toward −inf, ROM toward zero.**~~ **FIXED** (earlier):
   `adiv2`-style toward-zero in `do_player_yvel_d2` / colony.

8. ~~**Player Y-bounds clamp is exclusive and drops the pml_Bbottom gate.**~~ **FIXED**
   (tick 123): inclusive `<=`/`>=` + `PML_BBOTTOM` gate.

9. ~~**Barrel-roll double-tap window is 1 frame tighter than ROM.**~~ **FIXED** (tick 124):
   `barrel_roll_update` now matches `s_beqdec_var` (branch-if-zero before dec); start only
   while `rolldelay>0`; polarity TLEFT→+32 / TRIGHT→−32. Tests `player_onfield_barrel.rs`.

10. ~~**In-flight `pfm_wobble` ship-roll float not applied.**~~ **FIXED** (tick 123):
    `pZrotfloattab` walk → `al_rotz` (+ broken-wing polarity).

11. ~~**`boostobj` not tagged on the pad-X / force boost.**~~ **FIXED** (tick 123):
    `BOOSTOBJ` set on pad-X and force-boost.

12. ~~**Steering ztilt not suppressed near the ground / at a wing wall.**~~ **FIXED**
    (tick 125): dpad ztilt gated on `!pml_lwleft/rwright` and
    `!(pml_Bbottom && worldy >= maxPmoveY-30)`.

13. ~~**`viewmove_srou` final copy uses `pviewposz`, ROM uses `viewposz`.**~~ **FIXED**
    (tick 126): `GameCamera::update` writes viewposx/y/z to WRAM `$0550`/`$0552`/`$0554`;
    `viewmove_srou` / `playerdead_strat` copy `VIEWPOSZ` → `BGSSCROLLZ` (last-frame camera
    Z; ROM dostrats→getview order). Tests `bgsscroll_viewposz.rs`.

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
- **Barrel-roll advance**: `rollZoff += rollZvel`; `rollZvel ∓2 toward 0`; idle
  `achase(rollZoff,0,3)`; start on fresh L/R SHOULDER while `rolldelay>0` (`s_beqdec`);
  polarity TLEFT→+32 / TRIGHT→−32 — PSTRATS.ASM:2582-2596/2713-2724. ✓ (tick 124).
- **al_rot composition**: rotx=plrotx>>8; roty=plroty>>8+turnrot>>8;
  rotz=plrotz>>8+ztilt+zshake>>8+zstratadd+rollzoff (+ wobble float) — ✓ (tick 123).
- **playerlimitx_srou** inclusive X boundary + LEFT/RIGHT arrows — PSTRATS.ASM:2820-2828
  (player.rs:1135-1142). ✓
- **checkarrows_srou**: PSTF_INSEQ→0; per-arrow AND of pad + PMOVELIMITAND bit —
  PSTRATS.ASM:3482-... (player.rs:1160-1181). ✓
- **strat_speed_to** overflow-safe snap (rate 1/2 here) — common.rs:325-349. ✓
- **gen_3dvecs / apply_velocity** yaw-negation + fixed-point — common.rs:402-461
  (oracle-verified elsewhere). ✓
