# Enemy-B / ground audit findings (2026-07-07, oracle+ASM-verified)
Source: accuracy-audit agent report. Fix agent: apply in order, flip audit_strats_b.rs
assertions, re-bless eb_parity fixtures after. ASM refs authoritative.

## Critical
1. bossF: playerturn180_Istrat missing entirely. ROM GB2STRAT.ASM:178 (FC2, gated psf2_playerHP0), :256 (FC3 ungated), :286 (bossFCdie2_init). Port playerturn180 + playerbossFdie (GB2STRAT:351-359 — incl. pviewposz -= medpspeed*2 and playerinspace_strat swap) and wire all 4 sites (enemy_b.rs:2664-2669, 2757-2762, 2800, 2853-2862).
2. bossFC2/FC3 objinfront gates INVERTED (enemy_b.rs:2657-2658, 2750-2751). s_jmp_objinfront a,b = lda a.z; cmp b.z; bpl skip (STRATMAC.INC:3445): FC2 turn block runs when me.z < pl.z. Rust runs complement.
3. bossA cup GO-state return INVERTED (enemy_b.rs:1742-1744). ROM .ngo: return home when cup.z < player.z AND zdist >= 200. Rust is complement -> drill run never happens. Also: stop homing inside 1000 units; fly by heading sbyte3/sbyte4 not visual rotx/roty (no strat_aim_3d every tick); NO GO timer (BOSSA_CUP_GO_TIME=45 is invented — delete).
4. bossA cups NEVER fire in ROM (bossacupfire* GB3STRAT:1018/1027 have no callers). Delete the GO-state hmissile fire (enemy_b.rs:1752-1759).
5. bossA turret aim: boss_apply_yaw_offset (enemy_a.rs:1402-1404) stomps turret roty with mother.roty each tick before the achase (enemy_b.rs:1597-1611). ROM never writes turret roty from mother (GB3STRAT:1209+1226). Make the offset position-only for turrets.
6. bossA turret fire gate INVERTED + pattern: ROM s_jmpnot_objpointnegZ fires when roty in 180±45 (STRATMAC:6214-6221); Rust fires at 0±45. Cadence: frames 15 (yaw -deg11) and 30 (+deg11) of &31, homing HPLASMA + weapon_rot spread (GB3STRAT:1191-1207); Rust every 15 aimed straight. NOTE bossFA maps the same macro inverted too — make both consistent with ROM.
7. bossA turret death/resurrection missing: bossAturretexp_Istrat (GB3STRAT:1229-1240) leaves invisible hardHP husk + mother.sbyte3++; bossAcupDOWN_srou resurrects (hp=turrHP, clear invisible, sbyte3--). Rust frees the turret (enemy_b.rs:1558) — core loop broken. DOWN state :1814-1818 needs the husk revive.
8. bossA parent machine: mother.sbyte3 = destroyed-turret COUNT. ==2 -> 3-missile barrage (frames 20/25/30 of &63); !=2 -> retarget turrets from bossATY_tab via sword2 (+3 mod 9) every 32 frames (notdelay 5); ==3 + 3 children -> kill parent. Rust (enemy_b.rs:1967-1990) treats sbyte3 as pattern idx cycling every 5 frames. Lone-turret sweep (turret sbyte2 60/20 toggle, GB3STRAT:1211-1222) missing.
9. bossA GO/IROTATE selection: GO only when last cup (2 dead), else IROTATE (Rust has it backwards, enemy_b.rs:1956-1960).

## High
10. achase_angle rounds toward -inf; ROM toward zero (enemy_a.rs:271-283). Oracle-proven (audit_strats_b.rs vs SR8_ACHASE_ALVAR3/4: 0->100 r3 ROM 12 vs Rust 13). Use the strat_chase_proportional (common.rs:296) toward-zero core. Every enemy-B rotation uses this.
11. s_jmp_notdelay N = gameframe & ((1<<N)-1) (STRATMAC:6456), misread as %N or halved:
    boss7 hatch volley 32 not 5 (enemy_b.rs:679); boss7 launcher missile 32 not 5 (:770);
    bossA retarget 32 not 5 (:1967); bossF turret laser 8 not 4 (:3055); bossFC smoke 8 not 4 (:2439);
    boss7a speedto 4 not 2 (:939); boss7exp spin 8 not 4 (:1295).
12. bossFtur fire-window INVERTED (enemy_b.rs:3043): ROM fires only when sbyte2 <= 15; Rust returns then.
13. bossFA/FB vz scale: ROM s_scale_alvar W,vz,1 = asl = x2 (STRATMAC:4604); Rust >>1 (enemy_b.rs:3181, 3268) — 4x too slow.
14. bossFA/FB combine chase: ROM Achase rate 4 trunc+min-step; Rust >>2 floor (enemy_b.rs:3131-3133, 3214-3216). Use strat_chase_proportional(_,_,4).
15. boss7 s_jmp_lower gates INVERTED (branches when worldy >= value, STRATMAC:3098): boss7a_strat (:954) rise while < -320; boss7launch_cont (:1008) rise while < -240; boss7alldead (:1174) inverted AND threshold is -40<<2=-160 not -40<<3.
16. boss7d/e loop amplitude: ROM sintab>>3 (±15) dy, costab>>1 (±63) dz (GB3STRAT:3494-3500); Rust sin*8/cos*2 (enemy_b.rs:1089-1090, 1124-1125).
17. bossFC intro: s_decbne sbyte2 200-frame countdown gates states 0/1 (GB2STRAT:82-95) — Rust descends immediately (enemy_b.rs:2539-2559). State-2 sound $8E when roty REACHES 0, latch on ASF4_SFLAG8 (:2570-2574).
18. bossFC2_cont: skip smoke AND twin-Hplasma until sbyte2 >= 3 turrets destroyed (GB2STRAT:185-201); fire straight weapon_rot #0,#0 (homing ptr aims), not pre-aimed (enemy_b.rs:2687-2700).
19. bossFB mines inert: ROM mine0_istrat (DSTRATS:1572-1580) colltype ENEMY1, hitflash/explode strats, random rotz, no lifetime. Rust colldisable+colltype4+stratptr None+count 60 (enemy_b.rs:3252-3261). HP/AP 2/10 correct.

## Medium
20. spacepilon scatter: ROM s_add_rnd2pos x,255,255,255,2,2,1 = (rnd&255-127)<<2/<<2/<<1 via random_l; Rust deterministic idx*37/53/17 unscaled (enemy_b.rs:2319-2322).
21. spacepilonP state-0 chase: inline >>3 floor; use toward-zero trunc helper (enemy_b.rs:2129-2143).
22. Death sequences stubbed: boss7 parts skip boss7fall detach/bounce (:614); shield removal s_kill_obj (hp=0 -> shieldexp fall) not obj_free (:922); boss7exp per-tick L-explosions + end on worldy>=0 -> bossbigoutexplode, not 24-frame timer (:1291-1310); bossA death 3-piece tank breakup (bossAexp2/L/M/R, GB3STRAT:800-880) vs spin+timer (:2024-2038).
23. bossa_strat intro: roty +1 every 2 frames (notdelay 1); slide-in applies vx every tick, decel (vx+=1) only once worldx <= 210 (enemy_b.rs:1997-2004, 1857-1864).
24. ground.rs staydist: re-runs every frame (worldz = sword1 + pviewposz tracks viewer, GSTRATS:704-710); Rust computes once + stratptr None (ground.rs:65-71).
25. bossFCdie2 rubble: X offset <<1 (enemy_b.rs:2819-2826).
26. boss7 parent motion yaw from sbyte2 not roty (GB3STRAT:3227; enemy_b.rs:916).

## Minor
- bossAup/cover sounds $73/$72 gate on sflag3 all-cups-dead flag (maintain it) (enemy_b.rs:1876, 1901).
- bossAcover DOWN while sbyte2 >= 20 (not > 20) (:1913).
- bossA parent collstrat: none in ROM (no hit_flash) (:2050).
- bossA turret M sbyte3 overwritten to 0 by Icont (Rust keeps DEG180) (:1633).
- Cup home Z offset -2<<bossA_scale in bossa_cup_set_home (:1450-1474); cup open anim cap 6 not 7.
- Muzzle offsets 4x too small at bossFC2 fire (:2692-2698) + bossFA (:3164-3172) — use effective-value convention like boss7launcher/bossA turret sites.

## Verified correct (don't touch)
boss7 phase graph/timers/fan-shot quirk/HP; bossA child layout/HP/cup chase rates+lifts/rotz/timers; bossF turret table/fire frames/FC2-FC3 roty targets/FCdie structure/sounds; spacepilon tick structure; ground stayrel/gnd/stayrelhard180yr; SPACE_VIEWCY=-60.
