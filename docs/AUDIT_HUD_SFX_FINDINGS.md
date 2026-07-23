# HUD/score/lives + SFX-map audit findings (2026-07-07, ASM-verified)
Scope: (A) HUD/score/lives vs SPRITES.ASM / TRANS.ASM calcmeters / MARIO/MDRAWLIS.MC /
MAIN.ASM / CONTINUE.ASM / PSTRATS-GSTRATS-GASTRATS; (B) sound-effect trigger map vs
trigse/setport3 (MACROS.INC:4201, SOUND.ASM:951), al_snd2/set_sound2 (STRATMAC.INC:248,
SOUND.ASM nearobjs), and the positional *sound_l helpers (SOUND.ASM:735-897).
ASM refs authoritative (reference/ultrastarfox/SF/). Do not commit fixes without re-audit.

Mechanism ground truth (for fixers):
- `trigse $id` = one-shot: lda #id, jsl setport3_l -> 16-entry ring g_sdport3, drained to
  APU port3 with echo-ack in IRQ.ASM:1612-1645. Gated by nosetport3 and (ingame && playerHP0).
  `trigse_rge` (MACROS.INC:4226) has ZERO call sites — ignore it.
- `set_sound2 obj,#n` = attached continuous sound: al_snd2 low nibble is the family id
  (se2_other=1..burst=7, plus raw 8-$f used by GA2STRAT/GASTRATS/DSTRATS/KSTRATS/GB3STRAT);
  SOUND.ASM nearobjs picks the NEAREST snd1/snd2 alien each frame, ORs pan (pantab via
  anglexy high byte: <=124 -> $40, 125-129 -> $80, else $C0) and range band ($00/$10/$20/$30,
  thresholds 250/650/1150, cutoff 3150) onto port 2. Force-sound shapes (ship_1/5s/5m/5/0_c)
  use their own protocol/thresholds.
- Positional one-shot families (SOUND.ASM:735-897): destboss $1e-$20, destenemy $21-$23,
  damenemy $24-$26, hitwall $27-$29, missile $3c-$3e, movewall $3f-$43, laser $44-$48,
  enemybattry $49-$4b, ringlaser $5c-$5e, dooropen $54/$55, doorclose $52/$53,
  enemyupsea $68-$6c, enemydownsea $74-$78. Common tail `makesnd` picks L/C/R (range<2000,
  x-offset +-170), far (2000..3149) or silence (>=3150), then setport3. destenemy/damenemy
  are ALSO fired directly with inline 1000/2000 range checks (EXPSTRAT.ASM:853-877,
  GSTRATS.ASM:895-925).

## PART A — HUD / score / lives

### Critical
1. ~~Score/credit system missing entirely.~~ **FIXED (verified tick 149):**
   `s_test_special` → `specials_dead`; `score::calc_stage_perc` /
   `calc_total_score` / `crossed_bonus_threshold` + `BONERTAB`;
   `Shell::enter_tally` initializes typed display state; the tally counts by
   three with `$12`, commits through `Planets::append_stage_score` at the
   delayed `$11` boundary, then plays `$1a` and awards the credit after the
   retail bonus delay. Tests `score.rs` unit +
   `tally_records_stage_score_and_awards_bonus_credit` +
   `hud_score_bosshp.rs` (explode → perc → credit).
2. ~~Continue flow ignores credits.~~ **FIXED** (tick 128): Continue entered only when
   `credits > 0`; accept does `dec credits` + `trigse $67` + `startbgm $f1` then refill
   lives; zero credits → Title (CONTINUE.ASM:55-56/286-289). Tests
   `death_to_continue_and_back` / `death_without_credits_goes_to_title`.
3. ~~Lives state is split across three unconnected stores~~ **FIXED (tick 148
   explode bookkeeping + prior lives unify):**
   - Lives: single WRAM `sv::LIVES` / `wm::LIVES` = 0x0520; shell seeds from
     `planets.lives` and mirrors back each frame; 1-UP incs the same slot.
   - `strat_explode` / `s_test_special`: increments `specials_dead` for
     `ASF4_SPECIAL | ASF4_CSPECIAL`; **never** decrements `specialobjtotal`;
     **never** sets `GF_BOSSDEAD` from special count (ROM: WORLD.ASM inc-only
     total; boss explode chains alone set GF_BOSSDEAD).
   Tests `explode_specials.rs` (4); `ea_rader0` fixture re-blessed.

### High
4. ~~Boss HP bar never drains.~~ **FIXED (verified tick 149):**
   `GameVars.bosshp` zeroed in `init_strats` each frame; boss parts
   `add_bosshp`; `FrameSnapshot.boss_hp_cur = v.bosshp` (not bossmaxhp).
   Tests `ea_units::boss_hp_bar_accumulates_and_drains`,
   `boss_ticks1_verify`, `hud_score_bosshp.rs`.

### Medium
5. ~~Shield bar misses the wireframe-shield color.~~ **FIXED** (tick 130):
   `GameVars.shieldup` set by item6; playermove wire-end blink; HUD fill color 7
   when shieldup (MDRAWLIS.MC:911-926). Tests `shield_meter_uses_color_seven_*`
   + `item6_sets_shieldup_*`.
6. ~~strings.rs talk-frame formula diverges from ROM for the rnd==0 flash frame.~~
   **FIXED** (tick 128): `random&31==0` → absolute face frame 4 (CONTINUE.ASM `.doit`);
   otherwise openingframes + whichfriend*2 + mouth. Matches hud.rs. Test
   `talk_flash_frame_is_absolute_four`.
7. ~~Bomb-icon flash missing: newest nova icon blinks via `specflash`~~
   **FIXED** (tick 129): FrameSnapshot.specflash + shell dec/frame; HUD
   `bomb_icon_draw_count` matches SPRITES.ASM:673-717; item5 writes canonical
   SPECWEPCNT (0x056E). Test `bomb_flash_hides_newest_until_blink_window`.

### Part A verified correct (don't touch)
- do_lives: displays lives-1, lives==0 clamped to show 0, tiles $3d/$62/$63+n at
  (16,17)/(24,17)/(32,17) = livesPos $1110 (+8,+8) (SPRITES.ASM:116-146,
  CONFIG/GRAPHICS.INC:52-54; hud.rs:584-592).
- Shield meter geometry: box (8,176) 40x8 color 13, fill +2/+2 h4 color 2, clamp 36
  (MDRAWLIS.MC:876-930, GRAPHICS.INC:43-45). Boost meter (176,176) color 6, boostanim
  -2 while boostcnt (zeroing boostcnt at 0), +1 recover to 40 (TRANS.ASM:613-632).
- Boss ENEMY sprite tiles at 200-v (224-24-v, halve if bit7) (SPRITES.ASM:815-849).
- Bombs from (225,182) step 9 (bombIconPos $b6e1, GRAPHICS.INC:63-66).
- playerB_HP=40 == NMI_PLAYER_MAX_HP; wings separate at playerW_HP=5 (STRATEQU.INC:324-325).
- DEFAULT_LIVES=3 == numlives; continue refills 3 (CONTINUE.ASM:313-314).
- Radio triggers: send_message gates on live speaker (friends_hp-1,y != 0,
  CONTINUE.ASM:675-676), msg_count1=50/msg_count2=0, friends_meter only when prior value
  $FF (CONTINUE.ASM:663-672); face sound table (24 entries $60/$7c/$7d/$62/... $8c) and
  open-at-openingframes / close `trigse $64` timing (CONTINUE.ASM:820-838, 745-747);
  chkmeter 16px push for bit7/Andross/meter (CONTINUE.ASM:841-858); teammate meter
  (mshowteammate2) gating. All match strings.rs/hud.rs except finding 6.

## PART B — SFX trigger map

Rust architecture matches ROM in kind: hooks.play_se == trigse/setport3 one-shots,
al.snd2 == set_sound2 attached sounds, sf-audio sound.rs is a faithful port of
SOUND.ASM dosounds_l/playersnd/nearobjs/do_obstacles/setport3_l + IRQ drain (ring
indices, echo-ack, pause flush, HP0/nosetport3 gates, pan table 124/129 split, range
bands 250/650/1150/3150, ship-force protocol 500/5000/10000/10000/11000 — all verified
against SOUND.ASM/IRQ.ASM line by line; no discrepancies found in sound.rs itself).
The discrepancies are all at the trigger sites and in the missing positional layer:

### Critical
8. ~~Enemy damage feedback ($24/$25/$26) missing~~ **FIXED (verified tick 150):**
   `strat_hit_flash` → `play_se_by_range($24/$25/$26)` by xzdiffs (<1000/<2000/else)
   (GSTRATS.ASM:895-925). Test
   `enemy_a_weapon_explode_se::hit_flash_plays_damage_se_by_range`.
9. ~~Enemy destruction plays the WRONG SOUND~~ **FIXED (verified tick 150):**
   `strat_explode` / `explode_icont` → `play_se_by_range($21/$22/$23)` gated on
   `ASF2_NOEXPSND` (EXPSTRAT.ASM:853-877). Test
   `enemy_a_weapon_explode_se::explode_plays_destruct_se_by_range_unless_noexpsnd`.
10. ~~Positional helper layer absent — silent enemy weapons/doors/walls.~~
    **FIXED** (ticks 128–130 / 146 / 210 / **204–207 / 209**): `make_snd` wired for Laser /
    EnemyBattry / Missile / RingLaser / MoveWall / DoorOpen/Close / HitWall /
    EnemyUpSea/DownSea. Tick 204–207: custom fire paths that bypassed `fire_*`
    helpers (missiles, RELSLOW/HPLASMA/SHORTPLASMA/RELFAST across ground/boss
    lanes). Tick 209: bossB `fire_home` SE by ROM weapon family (HMISSILE* →
    Missile, RELSLOWELASERHOME → Laser; spinend close laser-home). Tests
    `sound_wiring_tests`, `hitwall_separatemissile`,
    `wire_shield_movewall`, `sound_ids_f3f4`, `misspod_walker_truck_missile_se`,
    `boss_custom_hmissile_se`, `custom_laser_plasma_se`, `more_custom_weapon_se`,
    `bossb_fire_home_se`.
11. ~~Sea surface/dive sounds flattened to fixed centre variants.~~
    **FIXED** (tick 210 / SOUND_IDS F3–F4): `sea_enemy_up_sea` /
    `sea_enemy_down_sea` → `make_snd(EnemyUpSea/EnemyDownSea)` positional
    families `$68-$6c` / `$74-$78` (SOUND.ASM:861-885). Tests `sound_ids_f3f4.rs`.
    (Was: bosses.rs flat `play_se(0x69/0x75)`.)

### High
12. ~~Player damage/wing SFX all missing.~~ **FIXED** (tick 131): `pcbox_coll_strat`
    body `$04`/`$19` (AP≥8), shield warn `$1b`/`$1c` (sflag1/2 latches), wing hit
    `$07`/`$08`/`$04`, wing destruct `$05`/`$06`, wire scrape `$14`; leave COLLIDE for
    LCOLLIDE so sustained overlaps don't re-fire impact SE.
13. ~~Nova-bomb detonation missing $30.~~ **FIXED** (tick 131, verify): `nukeexp_istrat`
    already `play_se(0x30)` + `snd2=0` (GSTRATS.ASM:2121-2129); covered by
    `player_damage_sfx::nukeexp_plays_detonation_30`.
14. ~~Player-death music missing.~~ **FIXED** (tick 131): `playerdead_istrat` →
    `play_se(0x03)` + `play_music(0x11)` (PSTRATS.ASM:3110-3115).
15. ~~HUD arrow beep $8A is queued but never played.~~ **FIXED** (tick 132):
    `sf-app` drains `Renderer::take_pending_hud_sounds()` after submit into
    `AudioSys::play_hud_se` (SPRITES.ASM:872). HUD unit test covers wrap queue.
16. ~~Pause SFX unwired.~~ **FIXED** (tick 132): Playing START → `dopause` latch;
    `SoundCmd::PauseSnd($02/$01)` → `Sound::set_pause_snd`; gates wipe /
    stayblack / bf_dying / pstf_notdie (MAIN.ASM:1386-1426).
    **Also FIXED** (tick 133): `SoundCmd::NoSetPort3` + `Hooks::set_nosetport3`;
    path `bird_touch` sets true (PATHDATA.ASM:378); `planets_init` /
    `begin_gameplay` clear (PLANETS.ASM:257 / MAIN.ASM:120).

### Medium / notes
17. s_boss_dying port is CORRECT ($1e + startbgm $f1 + bf_dying + notdie,
    STRATMAC.INC:7760-7768 == enemy_a.rs:5494 SE_BOSS_DYING=0x1E/BGM_BOSS_DYING=0xF1) —
    listed because the id looks confusable with the nearby boss2exp `play_se(0x1D)`
    (bosses.rs:716), which is also right (EXPSTRAT.ASM:144 trigse $1d in the bossexplode
    chain). Don't "fix" either.
18. game.rs:748 MapwaitbossTrigse -> play_se(0x0B) is CORRECT: the ROM source is the
    `mapwaitboss` MAP macro (INC/MAPMACS.INC:1086-1092 `trigse $0b`), not a strat.
    (SOUNDEQU's se_warning1 has no STRAT/ASM trigse site — it lives in map data.)
19. Attached-sound (snd2) values used by Rust — 1, 2, 5, 6, 0x0F — all exist at ROM
    set_sound2 sites (se2 families 1-7 plus raw 8-$f; #$f cluster GA2STRAT.ASM:335-2882,
    GASTRATS.ASM:942/1080/3533). No wrong families found in the sampled sites
    (enemy_a.rs:1521/1834/1876/2497/2641/2754/3560/3683/3781, enemy_b.rs:558/600/890,
    bosses.rs:2472/2557). One-shot vs attached usage is architecturally consistent.
20. SETBGM map-op HP0 gate matches WORLD.ASM:194-206 (game.rs:951). play_music raw-vs-boot
    split (sound.rs:340) matches setbgmdo semantics.

### Part B spot-check table (Rust id -> ASM site, all VERIFIED MATCHES)
| id | Rust site | ASM site |
|----|-----------|----------|
| $35 single laser | player.rs:1139 | PSTRATS.ASM:2943 trigse se_laser |
| $34 twin laser | player.rs:1129 | PSTRATS.ASM:2958 |
| $36 beam-ball | player.rs:1100 | PSTRATS.ASM:2972 |
| $31 nova launch | player.rs:1042 | PSTRATS.ASM:2894 (se_abutton) |
| $32/$33 boost/brake | player.rs:644-673,392,1687,2117 | PSTRATS.ASM:2163/2182, 2247/2259; PISTRATS.ASM:106; GCSTRATS/PCSTRATS sites |
| $03 player down | player.rs:542 | PSTRATS.ASM:3054/3110/3259 |
| $0E 1-UP | enemy_a.rs:3388 | GASTRATS.ASM:2688 (item0_Istrat) |
| $10 item catch / gate heal | enemy_a.rs:757,917,2374,2459,3488,3896; GATE3_SOUND | GASTRATS.ASM:2667/2754/2981, GA2STRAT.ASM:2630/2683 |
| $15/$17/$18 item7/repair/item5 | enemy_a.rs:2993/2989/2840 | GASTRATS.ASM:2939/3121/2587 |
| $49 pillar/battery | enemy_a.rs:866 | DSTRATS.ASM:825 region; also mapblocksnd MAPMACS.INC:2006 |
| $58 boss7 alldead / bossFC2/FC3 / proximity | enemy_b.rs:1162,3244,3352,3886 | GB3STRAT.ASM:3319; GB2STRAT.ASM:157/239/604 |
| $59/$5A close/open | enemy_b.rs:944/921,1032/1017; bosses.rs:2218/2179 | GB3STRAT.ASM:3594·3707/3569·3682; D2STRATS.ASM:137/128 |
| $5B boss7 spawn | enemy_b.rs:1637 | GB3STRAT.ASM:3195 |
| $66 tsumami | enemy_b.rs:2089 | GB3STRAT.ASM:958, GISTRATS.ASM:144 |
| $70/$71 nucleus beam on/off | bosses.rs:3098/3100,3044 | GB3STRAT.ASM:418/416,381 |
| $72/$73 Andross voice | bosses.rs:2851/2793; enemy_b.rs:2290/2261,2310 | GB3STRAT.ASM:191·592/139·573·607 |
| $82/$85 boss1 appear/cover | enemy_a.rs:2437/2122 | GBSTRATS.ASM:112/201·572 |
| $83 bossA form | enemy_b.rs:2594 | GB3STRAT.ASM:548 |
| $8E large explosion | enemy_b.rs:3169; bosses.rs B2_SE_LAND | GB2STRAT.ASM:102; GBSTRATS.ASM:609 |
| $96 + bgm $F0 bossF death | enemy_b.rs:3469-3470 | GB2STRAT.ASM:316-317 (startbgm $f0 + trigse $96) |
| $9D/$9E whale roars | bosses.rs:2020,2163/2100 | D2STRATS.ASM:63·114·328/118 |
| $2D/$2F/$2C/$4C/$21/$23/$27/$1D | bosses.rs:2047; enemy_a.rs:2228; enemy_b.rs:2664·2856,785,769,3510/2954/2715; bosses.rs:716 | D2STRATS.ASM:161; GBSTRATS.ASM:434; GB3STRAT.ASM:1480+,2348+,3802; GB2STRAT.ASM:344/216; GB3STRAT.ASM:2070+; EXPSTRAT.ASM:144 |
| $0B boss warning | game.rs:748 | MAPMACS.INC:1092 (mapwaitboss) |
| $95/$71/$85/$8E boss2 set | bosses.rs:297-300 | GBSTRATS.ASM:524/541·661/201·572/609 |

Also correct: strings.rs FACE_SOUNDS table == CONTINUE.ASM:864-888 byte-for-byte;
MSG_CLOSE_SFX $64 == CONTINUE.ASM:747.
