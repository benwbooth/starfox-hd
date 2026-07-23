# Enemy-B / ground audit findings (2026-07-07, oracle+ASM-verified)
Source: accuracy-audit agent report. Fix agent: apply in order, flip audit_strats_b.rs
assertions, re-bless eb_parity fixtures after. ASM refs authoritative.

## Critical
1. ~~bossF: playerturn180_Istrat missing entirely.~~ **FIXED (verified tick 191):**
   `playerturn180_*` + `bossf_set_player_turn180` wired at FC2 (HP0-gated), FC3
   (ungated), FCdie2. Tests `bossfc_objinfront.rs`.
2. ~~bossFC2/FC3 objinfront gates INVERTED.~~ **FIXED (verified tick 191):**
   FC2 runs turn when `me.z < pl.z`; FC3 swapped args (`pl.z < me.z`). Tests
   `bossfc_objinfront.rs`.
3. ~~bossA cup GO-state return INVERTED.~~ **FIXED (verified tick 192):**
   return when `me.z < pl.z` && `|dz|>=200`; homing via sbyte3/sbyte4; no GO
   timer; no GO fire. Tests `bossa_cup_criticals.rs`.
4. ~~bossA cups NEVER fire in ROM (GO hmissile).~~ **FIXED (verified tick 192):**
   GO path has no weapon spawn. Test `bossa_cup_go_does_not_fire`.
5. ~~bossA turret aim: mother.roty stomp.~~ **FIXED (prior + tick 192):**
   `bossa_update_turret_position` is pos-only; cont Achases toward sbyte3.
   Covered by `bossaturret_lmr.rs`.
6. ~~bossA turret fire gate INVERTED + pattern.~~ **FIXED (prior + tick 192):**
   fires when roty in 180±45; frames 15/30 of &31 with ±deg11. `bossaturret_lmr.rs`.
7. ~~bossA turret death/resurrection missing.~~ **FIXED (verified tick 192):**
   husk (`invisible`+hardHP) + mother.sbyte3++; DOWN revives. Test
   `bossa_turret_husk_and_down_revive`.
8. ~~bossA parent machine sbyte3 wrong.~~ **FIXED (verified tick 192):**
   sbyte3 = destroyed-turret count; ==2 missile barrage; else retarget &31;
   ==3+3 children kill parent. Lone-turret sweep in cont.
9. ~~bossA GO/IROTATE selection backwards.~~ **FIXED (verified tick 192):**
   GO only when `dead_cups==2`, else IROTATE. Test `bossa_attack_go_only_when_last_cup`.

## High
10. ~~achase_angle rounds toward -inf.~~ **FIXED (verified tick 193 + oracle):**
    `achase_angle` → `achase_angle_8` toward-zero; fuzz/audit_strats_b MATCH.
    Test `achase_toward_zero_0_to_100_rate3`.
11. ~~s_jmp_notdelay N misread.~~ **FIXED (verified tick 193):** hatch/launcher/
    retarget `&31`, Ftur/FC smoke/exp `&7`, boss7a speedto `&3`. Code + prior tests.
12. ~~bossFtur fire-window INVERTED.~~ **FIXED (verified tick 191):**
    fires only when `sbyte2 <= 15`; notdelay 3 (= every 8 frames). Test
    `bossftur_fires_only_when_sbyte2_le_15`.
13. ~~bossFA/FB vz scale >>1.~~ **FIXED (verified tick 193):** `vz <<= 1` (ASL).
    Test `bossfa_vz_scale_asl_x2`.
14. ~~bossFA/FB combine chase >>2 floor.~~ **FIXED (verified tick 193):**
    `strat_chase_proportional(_,_,4)`. Test `bossfa_combine_uses_chase_rate4`.
15. ~~boss7 s_jmp_lower gates INVERTED.~~ **FIXED (code):** rise/advance when
    `worldy >=` threshold (−320/−240/−160).
16. ~~boss7d/e loop amplitude wrong.~~ **FIXED (verified tick 193):**
    sintab>>3 / costab>>1 via `adiv2n`. Test `boss7d_loop_sintab_scaled`.
17. ~~bossFC intro descends immediately.~~ **FIXED (verified tick 193):**
    200-frame `sbyte2` countdown gates states 0/1. Test
    `bossfc_intro_countdown_gates_descent`.
18. ~~bossFC2_cont fires before 3 turrets.~~ **FIXED (verified tick 193):**
    smoke+Hplasma held until `sbyte2 >= 3`. Test
    `bossfc2_cont_holds_fire_until_3_turrets`.
19. ~~bossFB mines inert.~~ **FIXED (tick 193):** live mines (hitflash/explode,
    no lifetime) + **colltype fix** `enemy_a::COLLTYPE_ENEMY1` (0x10), not
    mislabeled vars 0x01. Also swept other enemy_b ENEMY1 sites. Test
    `bossfb_spawns_live_mines`.

## Medium
20. ~~spacepilon scatter: ROM s_add_rnd2pos …~~ **FIXED (verified tick 194):**
    `((rnd&255)-127)<<2/<<2/<<1` via `sfrtl_random`. Test
    `spacepilon_scatter_rnd2pos_scales`.
21. ~~spacepilonP state-0 chase: inline >>3 floor~~ **FIXED (verified tick 194):**
    `achase_angle` toward-zero on relposy. Test
    `spacepilonp_state0_achase_relposy`.
22. ~~Death sequences stubbed~~ **FIXED (verified tick 194):**
    `boss7fall_*` detach/bounce; shield `s_kill_obj`; `bossAexp*` 3-piece
    breakup. Test `death_sequences_boss7fall_and_bossa_breakup`.
23. ~~bossa_strat intro~~ **FIXED (verified tick 194):**
    roty+1 every 2 frames; vx decel only when `worldx<=210`. Test
    `bossa_intro_roty_and_vx_decel`.
24. ~~ground.rs staydist~~ **FIXED (verified tick 194 + prior ea_units):**
    per-tick `worldz = sword1 + pviewposz`. Test
    `staydist_tracks_pviewposz_each_tick`.
25. ~~bossFCdie2 rubble X offset~~ **FIXED (verified tick 194):**
    X `<<1`. Test `bossfcdie2_rubble_x_offset_asl`.
26. ~~boss7 parent motion yaw~~ **FIXED (verified tick 194):**
    `gen_vecs` from `sbyte2` not `roty`. Test
    `boss7_parent_yaw_from_sbyte2_not_roty`.

## Minor
- ~~bossAup/cover sounds $73/$72 gate on sflag3~~ **FIXED (verified tick 195):**
  gated on `BOSSA_PARENT_FLAG_CUPS_DEAD`. Test
  `bossa_up_cover_sounds_gated_on_cups_dead`.
- ~~bossAcover DOWN while sbyte2 >= 20~~ **FIXED (verified tick 195):**
  `>= 20` not `> 20`. Test `bossacover_down_at_sbyte2_eq_20`.
- ~~bossA parent collstrat: none~~ **FIXED (verified tick 195):**
  `collstratptr = None` in `strat_bossa_init`. Test
  `bossa_parent_collstrat_none`.
- ~~bossA turret M sbyte3 overwritten to 0~~ **FIXED (verified tick 195):**
  Icont sets sbyte3=0 for all turrets. Test
  `bossaturretm_icont_sbyte3_zero`.
- ~~Cup home Z offset -2<<scale; open anim cap 6~~ **FIXED (verified tick 195):**
  Tests `bossa_cup_home_z_and_open_anim_cap6`.
- ~~Muzzle offsets 4x too small at bossFC2/FA~~ **FIXED (verified tick 195):**
  effective `±20<<bossF_scale`. Test
  `bossf_muzzle_offsets_effective_scale`.

## Verified correct (don't touch)
boss7 phase graph/timers/fan-shot quirk/HP; bossA child layout/HP/cup chase rates+lifts/rotz/timers; bossF turret table/fire frames/FC2-FC3 roty targets/FCdie structure/sounds; spacepilon tick structure; ground stayrel/gnd/stayrelhard180yr; SPACE_VIEWCY=-60.
