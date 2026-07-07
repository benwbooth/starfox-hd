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
1. Score/credit system missing entirely. ROM has NO per-enemy point score — `s_score` is a
   no-op macro (STRATLIB.INC:1098-1101, body only checks NARG). The real score is the hit
   percentage chain: `s_test_special` (STRATMAC.INC:225-234) increments `specials_dead` when
   a special OR Cspecial dies (invoked from explode_Icont, EXPSTRAT.ASM:703);
   `calctotalscore` (MAIN.ASM:780-800) sums stage percentages; end-of-level tally screen
   (MAIN.ASM:1077-1160) prints stage % + total; `checkbonus` (MAIN.ASM:1367-1383) compares
   the total against `bonertab dw 2100,1900,...,300,100,0` and on crossing a threshold
   awards `inc credits` + fox sprite (`onecredspr`, MAIN.ASM:1136-1140, drawn by do_1credit
   SPRITES.ASM:1031-1052) + bonus SFX `trigse $1a` (MAIN.ASM:1149).
   Rust: none of this exists. `specials_dead` (wm 0x1F0B) is written but never read;
   ui.rs:648 hard-codes the map-screen score line to "00000"; no tally screen, no credits,
   no bonus. FIX: count specials_dead per ROM rule (special OR Cspecial — see finding 3),
   port calctotalscore/checkbonus/bonertab, add credits to Planets, wire trigse $1a.
2. Continue flow ignores credits. ROM: new game starts `lives=numlives(3)`, `credits=0`
   (PLANETS.ASM:207-211, CONFIG/GAME.INC:33); with 0 credits the continue screen is skipped
   -> game over (CONTINUE.ASM:55-56); accepting costs `dec credits` (CONTINUE.ASM:286) with
   `trigse $67` + `startbgm $f1` (:288-289), then lives refilled to 3 (:313-314).
   Rust shell.rs:437-444 (GameState::Continue): START/A always continues, free, infinite.
   FIX: gate Continue on planets.credits > 0, decrement on accept, else Title/game-over.
3. Lives state is split across three unconnected stores:
   - death decrement writes sv::LIVES = WRAM 0x0520 (player.rs:511-513);
   - 1-UP pickup writes wm::LIVES = WRAM 0x1F0A (enemy_a.rs:3388-3390);
   - respawn/game-over decision and HUD read `planets.lives` (shell.rs:516,767) which no
     gameplay code ever decrements or increments.
   Net effect: player never reaches game over from gameplay, HUD lives counter frozen
   ("x2" forever), the 1-UP item ($0e + inc, GASTRATS.ASM:2688-2689 item0_Istrat) does
   nothing visible. ROM has ONE `lives` var: dec on death (PSTRATS.ASM:3266), checked after
   death fade (GSTRATS.ASM:477 `lda lives / lbne .notdeadyet`), inc on 1-UP
   (GASTRATS.ASM:2689, no cap). FIX: single canonical lives store (planets.lives or one
   WRAM slot) + one accessor used by all three lanes.
   Related bookkeeping bug in the same function: strat_explode (enemy_a.rs:741-755)
   DEcrements `specialobjtotal` and counts only ASF4_CSPECIAL into specials_dead. ROM
   never decrements specialobjtotal (it is the stage denominator, set at map build), and
   s_test_special counts asf_special OR asf_Cspecial (STRATMAC.INC:225-234).

### High
4. Boss HP bar never drains. shell.rs:514 feeds `boss_hp_cur: v.bossmaxhp` ("full bar while
   bossmaxhp nonzero"). ROM: mdrawbossHP (MARIO/MDRAWLIS.MC:985-1057) fills from `m_bossHP`,
   which is zeroed after every draw and re-accumulated EACH frame by the boss strats via
   `s_add_bossHP x,al_hp` (STRATLIB.INC; 15+ sites: GBSTRATS.ASM:274/400/756/803,
   GB2STRAT.ASM:449, GB3STRAT.ASM:130/1000/1185/1303/2059/2200/2224/2640,
   D3STRATS.ASM:581-582). `s_set_bossmaxHP` zeroes m_bossHP (STRATLIB.INC:519-543).
   FIX: add a per-frame bosshp accumulator to GameVars, port the s_add_bossHP calls in
   sf-strat boss ticks, surface it as boss_hp_cur, zero it after frame snapshot.
   Geometry nits vs MDRAWLIS.MC:985-1057: Rust clamps fill to max (hud.rs:554); ROM only
   resets m_bossHP to 0 when it exceeds max+10, otherwise draws unclamped. Everything else
   (w=max+4, halve both if max>=128, x=222-w [gameNum_col*8-2, VARS.INC:111], y=2+16,
   colors 14/2, fill h=2) matches.

### Medium
5. Shield bar misses the wireframe-shield color. calcmeters copies `shieldup` to
   m_shieldup (TRANS.ASM:604-605); mdamagemeter draws the fill in color 7 instead of 2
   while it is set (MDRAWLIS.MC:911-926). Rust hud.rs:512-523 always uses color 2 and
   FrameInputs has no shieldup. (Root cause: the shield item chain — item6 wireframe pickup,
   `TRIGSE $16` GASTRATS.ASM:2622 — is unported.) Box geometry/clamp are correct.
6. strings.rs talk-frame formula diverges from ROM for the rnd==0 flash frame.
   ROM friends_messages_l (CONTINUE.ASM:721-744): `random&31==0 -> lda #4 -> .doit`
   (ABSOLUTE frame 4, the "anyone" flash); otherwise frame = openingframes(5) +
   whichfriend*2 + mouth (mouth=rnd&1, forced 0 when msg_count1<30).
   strings.rs:352-361 computes `4` then STILL adds OPENING_FRAMES + (whichfriend<<1) ->
   wrong portrait frame roughly 1/32 of talk ticks. hud.rs:333-346 implements the same
   logic CORRECTLY (face_talk = 4 absolute) — two divergent copies; fix strings.rs and/or
   consolidate.
7. Bomb-icon flash missing: newest nova icon blinks via `specflash`
   (SPRITES.ASM:673-678,700-706). hud.rs:571-573 draws all icons solid.

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
8. Enemy damage feedback ($24/$25/$26) missing — the most common combat sound in the game.
   ROM hitflash_Istrat, the default enemy coll strat, plays se_damageenemynear/mid/far by
   range (<1000/<2000/else) on EVERY non-fatal hit (GSTRATS.ASM:895-925); variants
   hitflashSexp/Mexp/Lexp add an explosion child + `trigse $24` (GSTRATS.ASM:855-884) and
   hitflashBOSSd plays $80 (or $24 when nohitaffect) (GSTRATS.ASM:843-855).
   Rust strat_hit_flash (enemy_a.rs:723-738) is silent. FIX: add the 3-range damage SE
   (share a helper `trig_se_by_range(g, idx, $24,$25,$26, 1000, 2000)`).
9. Enemy destruction plays the WRONG SOUND: strat_explode (enemy_a.rs:756) fires
   `play_se(0x10)` (= se_itemcatch, the item chime) on every explosion. ROM explode chain
   (EXPSTRAT.ASM:853-877) plays se_destructenemynear/mid/far $21/$22/$23 by range
   (<1000/<2000/else), gated on the alien's `noexpsnd` sflag (Rust ASF2_NOEXPSND exists —
   bigexplode_istrat toggles it — but nothing reads it). explode_Istrat itself triggers no
   SE. FIX: replace 0x10 with the ranged $21/$22/$23 + ASF2_NOEXPSND gate.
10. Positional helper layer absent — silent enemy weapons/doors/walls:
    - lasersound_l ($44-$48): enemy laser spawns fire_Elaser/relslow/relfast/slowElaser
      (GSTRATS.ASM:2383,2559,2574,2589,2603 `jsl lasersound_l`). Rust
      boss2_fire_relfastelaser / _relslowelaserhome (bosses.rs:551-641), the sea and
      enemy_a laser spawners: no sound at all.
    - enemybattrysound_l ($49/$4a/$4b): 8 sites GSTRATS.ASM:2417-2544.
    - missilesound_l ($3c-$3e): 9 sites GSTRATS.ASM:2623-2763 (incl. fire_bossHmissile2
      GSTRATS:2663 vs silent bosses.rs:643).
    - hitwallsound_l ($27-$29): GSTRATS.ASM:763 (.solidhit, generic object-hits-ground)
      and :2399. Rust has exactly one flat `trig_se(0x27)` (enemy_b.rs:2715).
    - movewallsound_l ($3f-$43): DSTRATS.ASM:1033/1046; ringlasersound_l ($5c-$5e)
      GSTRATS.ASM:2497; dooropensound_l/doorclosesound_l ($54/$55, $52/$53)
      KSTRATS.ASM:387/402.
    FIX: port makesnd (SOUND.ASM:899-945: L/C/R within range 2000 split at x±170, far to
    3149, silence beyond) into sf-strat common and call it from the ported spawn sites.
11. Sea surface/dive sounds flattened to fixed centre variants: bosses.rs:1457/1461 play
    0x69/0x75 unconditionally. ROM enemyupsea_l/enemydownsea_l (SOUND.ASM:861-885) select
    $68-$6c/$74-$78 by pan+range through makesnd — including SILENCE beyond 3150 — at
    D2STRATS.ASM:845/822, D3STRATS.ASM:1141, GASTRATS.ASM:2038/2099/2132,
    GA2STRAT.ASM:3078-3147, DSTRATS.ASM:2130. Same fix as 10.

### High
12. Player damage/wing SFX all missing — playercoll_istrat (player.rs:453-480) is silent.
    ROM PSTRATS: body hit `TRIGSE $04` when collider AP>=8 else `$19` (PSTRATS.ASM:182,188);
    low-shield warnings `$1b` below playerB_HP/4, `$1c` below /8 (:225,231); wing damage
    $07/$08 (:327,473); wing destruct $05/$06 (:390,536); wing-scrapes-wall $14
    (:123,3287,3323); wing collisions also emit $04 (:321,467).
13. Nova-bomb detonation missing $30: nukeexp_Istrat does `trigse $30` + `set_sound2 x,#0`
    (GSTRATS.ASM:2121-2129). Rust spawns the nuke (player.rs:1033-1042, launch $31 correct)
    but has no detonation strat sound.
14. Player-death music missing: both ROM death paths do `startbgm $11` alongside
    se_playerdown (PSTRATS.ASM:3052-3054, 3115-3110 region). Rust playerdead_istrat plays
    only $03; sf-audio then reboots the level bank on respawn. Add play_music(0x11) at
    death.
15. HUD arrow beep $8A is queued but never played: hud.rs pushes 0x8A into
    Hud::pending_sounds (hud.rs:322-330, ROM SPRITES.ASM:872 trigse $8a), but nothing in
    sf-app/sf-render drains take_pending_sounds() into the audio layer.
16. Pause SFX unwired: Sound::set_pause_snd exists (sound.rs:360) but has zero callers.
    ROM dopause writes se_pauseon $02 / se_pauseoff $01 to pausesnd (MAIN.ASM:1393,1424),
    which flushes the ring. Same for set_nosetport3 (ROM path scripts / ENDSEQ.ASM:358) —
    zero callers. Wire when pause lands in the shell.

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
