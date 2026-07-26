# Continuous porting / verification loop

**Goal:** 100% ROM→Rust port + verification. Leaf nodes first, then walk up the call graph.
**Ground truth:** retail `Star Fox (USA) (Rev 2).sfc` (tier-2) + built ultrastarfox ROM for tier-1 symbol `call`.
**Style:** meaningful names, structs for memory — do **not** recreate the segmented 16-bit model.

## Loop cadence
- **Primary:** Cursor project `stop` hook (`.cursor/hooks.json` →
  `porting-loop-stop.sh`). Arm with `touch .cursor/porting-loop-on`; disarm with
  `rm .cursor/porting-loop-on` (or say “stop the porting loop”). One-turn skip:
  `touch .cursor/skip-porting-loop`. `loop_limit` 500.
- **Why not `/loop 0m` alone:** one-shot `sleep 0` + `notify_on_output` wakes are
  unreliable — Cursor often injects a generic “Briefly inform…” completion
  notice instead of the wake prompt, so the agent stops without continuing.
  The stop hook’s `followup_message` is a real auto-submitted user turn.
- Session logs: `re_loop_sessions/` (restored from Claude Code 2026-07-01…08).
- Tracker: this file + `docs/function_ledger.tsv` + `docs/ACCURACY_AUDIT.md`.

## Method (each tick)
1. Pick the highest-priority **unchecked** leaf in ## Queue below.
2. Disasm / ABI from `reference/ultrastarfox/SF/`.
3. Find or port the Rust equivalent (structs + clear names).
4. Add/extend `sf-oracle` fuzz or tier-2 coexec test; fix divergences.
5. Mark `[x]`, append `SESSION_SUMMARY` line to `re_loop_sessions/YYYY-MM-DD.txt`.
6. Re-arm the 0m wake.

## Queue — tier-1 math leaves (BATCH 3)

Already swept (BATCH 1–2): achase8/16, addvecs*, mulslog, gen_vecs*, perc*, speedto,
dist_xz, msqrt16/32, mcalcperc, calcstageperc, framescalevecs, anglexy axis/diag.

- [x] `ROTATE_16XZ_L` ($1FCE08) — ported `snes_trig::rotate_16xz`, fuzz_rotate16 30k exact
- [x] `ROTATE_16YZ_L` ($1FCE9C) — ported `snes_trig::rotate_16yz`, fuzz_rotate16 30k exact
- [x] `ANGLEXY_OFF_L` ($1FCF33) — exported, never called (structural); skip
- [x] `ANGLEXY_ABS_L` ($1FCF5B) — `strat_angle_xz_abs`; compose tests green
- [x] `XANGLEXY_L` / `XANGLEXABS_L` — `strat_angle_yz` / `strat_angle_yz_abs` (atan2(dy, dist_xz))
- [x] `YANGLEXY_L` / `YANGLEXABS_L` — alias of anglexy / anglexy_abs (already `strat_angle_xz`)
- [x] `CALCDXDY` ($03BDE9) — leaf `dividebynum` ported as `planets::divide_by_num` + `calc_scroll_step`; fuzz_calcdxdy 1848 exact. Full calcdxdy needs planetpos table wiring in UI.
- [x] `DGENVECS` ($098285) — DSTRATS macro → `s_gen_vecs` (2D); covered by gen_vecs_2d
- [x] `NALVELVECS_L` / `NALVEL3VECS_L` / `NVELVECS_L` / `NVECS_L` / `NVEL3VECS_L` — fall-through into nvecs/n3dvecs; covered by vector_family / gen_3dvecs
- [x] `SR_GEN_3DVECS` / `SR_GEN_3DVECS1..3` — ported `strat_gen_vecs_3d_scaled`; fuzz_sr_gen_3dvecs exact for vel<128 (vel≥128 = known mulslogmac latent)
- [x] `CROTMAT16_L` — covered by gsu_rotmat.rs (mcrotmatzxy16 Δ=0)
- [x] `WMATROTP16_L` — `snes_trig::wmat_rot_point` (GSU FMULT>>15 per-term); fuzz_wmatrotp16 91854 exact
- [x] `NUCLEUSWALLROT(_2)_SROU_L` — b8_wallrot now uses rotate_16xz; fuzz_nucleuswallrot 256 exact
- [x] `ROTATE_8XZ/YZ/YX_L` — snes_trig::rotate_8* + mulslog_mac8; fuzz_rotate8 28611 exact (gen_weapon muzzle leaves)
- [x] `DIVORCEFAMILY_L` — `Objects::divorce_family` wired into `Obj_Free` + `strat_explode`; tests green
- [x] `PCHASE*` / `PLANET_ROTZ` — structural (path labels / data table), not callable leaves
- [x] `SGENSPARK_SROU(_L)` — `player::sgen_spark` + `lspark_*`; wired into pcbox wing hit; tests green
- [x] `MSH_ROTPOINTS16` / `MSH_ROTPOINTSX16` — `msh_rot_points16` / `msh_rot_points_x16`
- [x] `MSH_ROTPOINTS8` / `X8` / `8_16` / `X8_16` — packed MULT + scale→FMULT helpers (`msh_rot_point8`, `msh_rot_points8_16`)
- [x] `ROTPROJ_L` — local offset leaf `rotproj_local_offset` (rotate_8 chain); full needs projectlog
- [x] `MCOREZROT_SROU` — `mcore_zrot` (`rotz += vz>>3`)
- [x] `FADECHARSIN` / `CLROTHERHALF_L` / `GENFOXSPR` / `PTRN_RING.TRN_ROT1` — structural (SNES OAM/GSU clear / path `P_ADD rotz`)
- [x] `MCALC_CIRCLE` — `mcalc_circle_edges` Bresenham midpoint; tests green
- [x] `MGENUVLIST*` / `MGENZLIST` — structural (GSU planet-sphere UV; HD uses `ps_blit_sphere`)

**math_helper False rows: 0** (BATCH 3 math leaves closed).

## Queue — known open bugs (from last Claude session, Jul 8)
- [x] Camera wobble: i16-quantized viewpos stair-steps → fractional FP16.16 pull-back + consume chased `outdist` (camera.rs)
- [x] Objects float above ground — camera pitch/yaw/roll now ROM `viewrot*w` from `outvx`/`outvy-turnrot`/`outvz-plrotz` (+ `noxrot`/`dozrot` gates); pull-back uses `outvx>>8`. Horizon +18 stays. Unit: `pitch_follows_outvx_*` + `yaw_follows_outvy_*` + `float_ground_rom_pitch_*`; oracle `getview_viewrot_vs_rom` MATCH.
- [x] `achase` antipodal ±128 / ±32768 — sr8 already fixed; sr16 now uses ROM `current+adiv2(target-current)` with i16 wrap (fuzz_pure_fns 7623 exact)
- [x] GSU `msqrt32` high-domain — faithful 16-bit overflow (hardware), not emu bug (`fuzz_pure_fns2`)
- [x] GSU `arctan16` off-axis — BGE/BLT fix; off_axis_grid maxΔ16≤51, maxΔ8≤1 (`gsu_arctan.rs`)

## Queue — player ship shape leaves
- [x] `SETYPLAYERSHAPE_L` / `SELECT_SHIP_L` — `player::select_ship` + `set_y_player_shape` + `PlayerShipShapes` table; wired into spawn; tests `player_shapes.rs`. Broken-wing/wire/zoom meshes still proxy `myship_4` until catalog has them.
- [x] `PLAYERTOMIDDLE1/4_SROU_L` + `SET_PLAYERTOCSLOW_L` — `player_to_middle1/4` + `set_player_to_cslow`; tests `player_to_middle.rs`.
- [x] Tunnel SET_PLAYER* — `apply_tunnel_fly_mode` + `set_player_in_{s,m,l}tunnel` / `{s,m,l}texit` + `player_in_tunnel_strat` / `player_in_texit_strat`; tests `player_tunnel.rs`. Opening init now shares Ltunnel helper.
- [x] Colony / nucleus / washent — `set_player_in_colony` / `set_player_in_nucleus` install their authored live/collision/death handoffs; `set_player_clear_colony` / `set_player_washent` (dupplayer leaf); tests `player_colony_nucleus.rs`.

- [x] Water / bridge / undergnd / space / turn180 / escape_nucleus SET_PLAYER* — fly-mode inits + strats; tests `player_fly_modes.rs`.

- [x] Cockpit enter/exit and player view cycle — typed exterior/cockpit modes and per-background cycles; exact Select edge/gates; `changeviewmode_l` dispatcher; `set_player_into_cock` / `out_of_cock` + phase strats + `make_all_med_pspeed`; scripted carrier/colony/map transitions; tests `player_view.rs`, `player_view_modes.rs`, `player_cockpit.rs`.

- [x] Cockpit death/ejection handoff — lethal cockpit hits run `playeroutofcock` for the source 23-count transition before the ordinary crash; straight-flight body, count-19 props, death roll, sequence-preserving space setup, dedicated hit-flash/explosion callbacks, and one-shot audio/life accounting; tests `player_cockpit.rs`, `player_damage_sfx.rs`, and all three unattended routes.

- [x] Terminal player explosion — `pexplode_Istrat` creates the flat inert player/camera anchor and separate large-particle explosion, transfers semantic references, installs the real explosion lifecycle, preserves the dying/dead flags, decrements one life, and drives the exact typed additive-red circle around the projected null-shape anchor through the 20-tick delay and complete black fade; strategy, shell-boundary, trace-parity, unit, and real GPU readback tests.

- [x] Generic `explode_Istrat` lifecycle — generate semantic ShapeHdr metrics and the seven missing explosion meshes; classify exact small/medium/large/oversized boundaries; preserve attached-fire removal, score/sound/no-polygon behavior, independent sprite/polygon counters, random rotations, and half-rate oversized debris; replace the packed sprite flag with `ObjectVisualKind::ScaledSprite`; billboard and size-adjust in the renderer; exact lifecycle, four-class, frozen-trace, matrix, and real GPU regressions.

- [x] Opening `pstrat` dispatch — typed catalog for every spawned BGS background; all 27 route/special/presentation maps install the authored initializer; ground dive builds its linked duplicate ship and camera; tests `catalog`, `player_view_modes.rs`, `player_flyin.rs`, and all three unattended routes.

- [x] LB out/in, divegnd, cred, tunnel→planet, VIEWLB3MOVE — tests `player_lb_cred.rs`.
- [x] Cutscene phase-2 inits (CHASE2/START/CLEARTURN2/UNDER2/WARP1/2/MOVE/DIVE2/CLEARDEMO2) — tests `player_cutscene_inits.rs`.
- [x] **player_cam False rows: 0** (SET_PLAYER* + cutscene inits closed this session).

- [x] `SHRAPFALL2_ISTRAT` / `SHRAPNEL_SROU_L` — LB1 debris; wired into `player_out_of_lb1_strat`; tests `shrapnel.rs`.
- [x] `CHILDREMOVE_ISTRAT` / `FLASH_STRAT` — `common::child_remove_istrat` + `flash_istrat`/`flash_strat`; tests `child_flash.rs`.
- [x] Map-CB flag inits already in `game.rs`: ONPLANET, CLEAR{TURN,UNDER,EARTH,CHASE,SHIP2,BRIDGE,DEMO}, DIVE, WARP, WARPOUT — ledger marked ported (full cutscene strats may still deepen).
- [x] `KILL_ISTRAT`/`KILL_STRAT` + `MAKEFIRE_SROU_L` + `FIRE_ISTRAT`/`FIRE_STRAT` + `PUFF_*` — shared in `common.rs` (smokeP/makesmoke already True; now canonical); `AFONFIRE` in `alien.rs`; tests `kill_fire_smoke.rs`.
- [x] `ROTSFLATSTAY_ISTRAT` + `SPARKY_*` / `ENDSPARKY_STRAT` — `common::rotsflatstay_istrat` / `sparky_*`; tests `sparky_rotsflat.rs`.
- [x] Hitflash variants — `hitflash_{m,s,l,bossd}_istrat` + `misscol_istrat` / `mchitflash_strat`; `strat_hit_flash` clears collide + respects nohitaffect; tests `hitflash_variants.rs`.

## Queue — after math leaves
1. ~~Reachable unported IS_*~~ — all 54 listed names now registered (2026-07-09 rescan); see `REACHABLE_UNPORTED.md`.
2. Tier-2 expand coexec coverage (strats not yet MATCH); stale same-shape / rangexz audits re-certified MATCH (ticks 119–122).
3. ~~Float-above-ground~~ — cam pitch/yaw/roll = ROM `outvx`/`outvy`/`outvz` (+ `noxrot`/`dozrot`); horizon +18 kept.
4. ~~Remaining ledger False~~ — **2169/2169 True** (tick 119).
5. SF2 after SF1 certification.

- [x] Escapee / explodedebris / exppiece — tests `explodedebris_escapee.rs`.
- [x] Pelaser collide / pollen / explodegate2 — tests `pelaser_pollen_gate2.rs`.
- [x] Elaser2die / pelaser2die / playerbeamdie — tests `elaser2die.rs`.
- [x] Nuke family — `nuke_*` / `nukeexp_*` / `removenuke` / `fire_nuke` + missbound; wired into player A-button; tests `nuke.rs`.
- [x] Pbeam / Pelaser / fire_playerbeam / fire_elaser / miss_end — tests `pbeam_pelaser.rs`.
- [x] Relelaser / relflatmiss / flatmiss + fire_friend/reb/plasma/beamball — tests `relelaser_flatmiss.rs`.
- [x] Oval/ring/shortplasma fire + elaser + Yhoming/fire_yhplasma — tests `oval_ring_yhoming.rs`.
- [x] Helpball family — `helpball_*` / `helpballhome_*` / Hcoll / Hrem + `ASF3_LOCKON`; tests `helpball.rs`.
- [x] Missile fire family — `fire_missile1/2` + `fire_Hmissile1/2` / FakeFar / bossH1 + `hmissile2_*` / `missile1_*` / `missile2a_*`; tests `missile_fire.rs`.
- [x] Specialty hmissiles — kami/`hmissile3_*` + chick/STB/QH fire + strats; tests `specialty_hmissile.rs`.
- [x] Spread + DefElaserCol — `fire_spread` / `spread_*` / `spreada_init` + `defelasercol_istrat`; tests `spread_defelaser.rs`.
- [x] Bonfire + ironball4 — `fire_bonfire` / `bonfire_*` + `fire_ironball4` / `ironball_*` / `ironballmissile_*`; tests `bonfire_ironball.rs`.
- [x] Fling ironball 1/2/3 — `fire_ironball` / `2` / `3` (muzzle + sflag3/2/1 + powerbuild); tests `fling_ironball.rs`.
- [x] Explode tick + baz/headfire — `explode_end` / `explode_strat` / `lexplode_strat` + public `bazexp`/`bazfall` + `headfire_*`; coltab FIRE_*/EXPLODE_*_C marked structural; tests `explode_end_baz_head.rs`.
- [x] Bossbigoutexplode + mfire1 + cube + misstankexp — `bossbigoutexplode{,off,_icont}` + BIGparticle; `mfire1*` fireface family; `cubefall`/`cubeexp`/`cubecoll`; public `misstankexp_istrat`; tests `bossbigout_mfire1_cube.rs`.
- [x] Ripman + item4/ripair + woodsgo — `ripman_*` / `item4_*` / `ripair_*` (repair chain); public `woodsgo_*` / `woodsexp` / `missgo`; tests `ripman_woodsgo.rs`.
- [x] Windexp/windspin + cupfire + tank1fire/misspoda — `windexp`/`windspin_*`; `bossacupfire{,miss}_srou`; public `tank1fire`/`misspoda_*`; `ship1aexp`/`mine2expnofire`; tests `windexp_cupfire.rs`.
- [x] Core/mine/blowcube explodes — `core1exp{,_strat,_col}` / `mcore1exp{,_strat,_col}` / `monolithexp` / `mine2exp` / `blowcube_*`; tests `core_mine_exp.rs`.
- [x] BossBrob fire + death explode — `bossbrobfire{p1,1,2}_{init,strat}` + `bossbrobsepexp_*` / `bossbrobexp_init` / `bossbpwaitexp_*` / `bossbpexp{,2}_*`; tests `bossb_fire_exp.rs`.
- [x] Monolith eye explode + col — `makelefteyeexp_srou` / `makerighteyeexp_srou` / `monolithcol_istrat` + `rebelasercol_istrat` body; tests `monolith_eye_exp.rs`.
- [x] Path coin/particle explode — `pcoinexplode` (+`.coin`) / `explodeparticles`/`pparticles`; `PROBEXPLODE` = `robexplode` alias verified; tests `path_explode_leaves.rs`.
- [x] BossBrob jump/land/kick/start — `bossbrob{start,start2,jump1,jump2,land,farjump1,farjump2,farland,kick}_{init,strat}`; fireP1→start2, fire2→jump1; tests `bossb_jump_land.rs`.
- [x] Boss1 turret fire labels — `boss1turret_nfire` / `boss1turretfire_end` (+ `_end`); tests `boss1_turret_fire.rs`.
- [x] BossBrob pounce/rndpos/foot/ment — `bossbrobpounce{pos,2}_*` / `bossbrobreappear_*` / `bossbrobrndpos{,2}_*` / `bossbrobfoot_*` / `bossbrobment{,2}_srou`; tests `bossb_pounce_rndpos.rs`.
- [x] BossBrob morph/split/demo/undead — `bossbrobchg{,2,3,4}_*` / `bossbrobvecs_cont{,2,3,4}` / `bossbrob{2,split,split2,sep,demo,undead,die}_*` / frontplayer/ouch/col/cent; death→chg chain; tests `bossb_chg_split.rs` (10).
- [x] BossB face spin/scream/bent — `bossbspin{1,2,end,end2}_*` / `bossbscream{,2,end}_*` / `bossbent{,long,_cont}_*` / `bossbentsplit{,2,col,cont}_*`; tests `bossb_spin_bent.rs` (8).
- [x] Cameleon2/cam2 + base0/bazooka1 — `cameleon2_*` / `cam2{hide,dash,nextpos}_*` ported; `base0{,b}_*` / `bazooka1{l,r}_istrat` publicized; tests `cameleon2_base0.rs` (5).
- [x] Crab B/L/T/R walker — `crab{b,l,t,r}_{istrat,init,strat}` / `crab_{init,cont}`; edge turns + MISSILE2 fire; tests `crab.rs` (9).
- [x] Bee1 + dragonfly — `bee1{,a,b}_{istrat,init,strat}` costab orbit→face→dive; `dragonfly_{istrat,strat}` 3-state fly-by; tests `bee1_dragonfly.rs` (5).
- [x] Crane0 + tzaco7 go/fall/cat — `crane0_{istrat,strat,col}` hardHP carrier; `tzaco7{go,fall,cat}_*`; tests `crane0.rs` (7).
- [x] Zaco7 + Sdragonfly + Zaco0 aliases — `zaco7_*` bank/aim; `sdragonfly_*` + `makeSdrag`; `zaco0{,b,c,c2,d}_*` publicized; tests `zaco7_sdragonfly.rs` (7).
- [x] Fastfighter + exitlight + blackholeexit — `fastfighter{1,2,3,_init}_*` + `dofighter`; `exitlight{1..6,a,b}_*`; `blackholeexit_istrat` publicized; tests `fastfighter_exitlight.rs` (9).
- [x] Kami + halfd + zacos2/cont — `kami{,_cont,die,go}_*` weave→dive→chase; `halfd_*` door anim; `zacos{2,_cont}_*` publicized; tests `kami_halfd_zacos.rs` (7).
- [x] Pole0 + cock dump/out — `pole0_{istrat,strat,col}` spinner; `cockdumpl_*` / `cockpit_*` / `cock{ship,pit}out_*` wired into into/out-of-cock; tests `pole0_cockpit.rs` (9).
- [x] Evader + truck1/2 + truck_cont/col — `evader{,a,_init,_cont}_*` WP dodge + home laser; `truck{1,2}_*` air trucks; `truck_cont`/`truckcol_istrat` publicized; tests `evader_truck.rs` (7).
- [x] Shark + fzaco + hardenemy1/hard90yrfog + zaco3_strat — `shark{,_cont,_cont2,a}_*` mine-drop climb; `fzaco{,2,3,_cont,_cont2}_*` brake→aim→climb; hard stubs; `zaco3_strat` alias; tests `shark_fzaco.rs` (7).
- [x] Aircar1–5 — `aircar{1..5}_{istrat,strat}` colony cars (skid/barrier/weave/speedup/wall); tests `aircar.rs` (5).
- [x] Amoeba aliases + chick + lastb2/3/4 — public `amoeba{home,col,stick,go}_*`; `chick_{istrat,strat}`; `lastb{2,3,4}_istrat` doors; tests `chick_lastb.rs` (5).
- [x] Walkright + walker1/2 + duct + wall/shou0/bholecoll — `walkright_*` / `walker{1,2}_*` / `l/rwalker1` / `duct_istrat`; public wall/walking/shou0/bholecoll; tests `walker_wall.rs` (6).
- [x] Tank0/1 + tank1a2/tank2/tank3 publicize + leftwall/wl/spacetest/bomwingdie — `tank{0,1}_*` hangar/forward/back; public `tank1a2`/`tank2`/`tank2zaco`/`tank3`; `leftwall`/`wl`/`wldie`/`spacetest`; `bomwingdie` alias; tests `tank0_wl.rs` (5).
- [x] Spacebar2 + starbull family — `spacebar2_{istrat,strat}` (world.rs parent tip + XangleXY rotz + mist); `starbull_{istrat,strat}` / `stbfp_strat` / `stbgo_{init,strat}` WP chase→face/fire→peel; tests `spacebar2_starbull.rs` (4).
- [x] Saucer1 + saucer — `saucer1_{istrat,strat,istrat2..4,strat2..4}` WP→face→spin-fire→peel; `saucer_{istrat,strat,strat2}` bounce→circle; tests `saucer.rs` (3).
- [x] Fly family + highfly/distantfly + szaco3 + warp — `fly{,_lr,r,2,3,4,dead,hitgnd}_*` / `highfly`/`distantfly`; `szaco3_*` bank/aim; `warp_*` 6-state; tests `fly_warp_szaco3.rs` (5).
- [x] Jump0/1 + sokuten + item3/6 + core0/1 + rightwall/mine1 + fog — `jump{0,1,0a}_*` hop→HMISSILE1; `sokuten_*` heading turn; `item3` +5 body HP / public `item6` wireship; `core{0,1}_*` gasf_flag1 flash; stubs `rightwall`/`mine1`/`fog`; tests `jump_sokuten_item3.rs` (5).
- [x] Item7a + door1 + woods/wireman + friend0/1 + minumusi — `item7a_*` helpball+wing repair; `door1{,open,close}wait_*`; public `woods_*`/`wireman{2x,2yr,2yl,up,die,cont2}_*`; `friend{0,02,1,kill}_*`; `minumusi` stub; tests `item7a_door1_friend.rs` (4).
- [x] Friend2 + leng0 + meteor2/col + winglazerman/tree/uperm/iris — `friend2_*` Zenemy lock+ELASER; `leng0_*` open anim; `meteor{istrat2,col}`; public `winglazerman{2,3,go,die}`/`tree{1,2}`/`uperm`/`iris{,_1}`; tests `friend2_leng0_meteor.rs` (4).
- [x] Sfish + exit/openlr + hyperspace + pillar3f + torpedoa — `sfish_*` school/alone; `exit{,coll}`; `openlr{,col}`; `hyperspace{,out}`/`hyper`/`phitflash`; `pillar3f{,fall,stay}`; public `torpedoa_*`; tests `sfish_exit_hyper.rs` (3).
- [x] Cruiser1/2 + fall/launcher + updoorcol + mine2 + doma + dpilar — `cruiser{1,1f,1fall,2,2fire,2launcher}_*`; `updoor{,col}_*`; `mine2_*`; `doma`/`domb`; `dpilar`=halfd alias; tests `cruiser_mine2_doma.rs` (3).
- [x] Suckbits/cube + lseqdoor + volrock/plasma/down + tree3 — `suck{bits,cube,obj,objfast}_*`; `lseqdoor{1,2}`; public `vol{rock,plasma,rockdown}_*`; `tree3`→tree2 forced; tests `suck_lseq_volrock.rs` (3).
- [x] Ships + intro1pfall + speedlines + monolithpart + castbit.hit + lspark + door1 inits — `ships_*`; `intro1pfall{,ing}_*`; `speedlines`; `monolithpart{,L,_srou}_*`; public `castbit_hit`/`lspark_*`/`door1{open,close}wait_init`; tests `ships_intro_monolith.rs` (3).
- [x] CLSHIP1–3 + TURN2 + dive/under boost + floatCLship + up1manchild1–3 + firenormringlaser + boss2spark — public `clship{1,2,3}_*`; `clship_turn2_*`; `clship_dive{boost,_cont2}`; `clship_underboost_*`; ROM-faithful `float_clship{,2}`; `up1manchild{1,2,3}_istrat`; `firenormringlaser` vel120; `boss2spark_*` (+ dead srou); tests `clship_up1man_spark.rs` (2).
- [x] Ship1/ship1a/col + ship3b/c/cont + boss2rots/doboss* — `ship1_*` peel-down; `ship1a_*` shoot/smoke body; `ship1col` HF2; `ship3_*` rise/fire + cont→b/c; `dobossrot{,x4}` / `doboss2rot` / `boss2rots`; tests `ship1_ship3_boss2rots.rs` (2).
- [x] Ship2 entrance — `ship2_*` / `ship2fire_cont` / `ship2{into,outside}_{init,strat}`; on-axis→into guide, off-axis→outside peel; tests `ship2_entrance.rs` (2).
- [x] MCORE1 body + TUNNELA + NULL — `mcore1_{istrat,strat}` wait→zoom→flip→center (setstate re-entry); `tunnela{,2}_*` HF5 toggle + dincanim#16; `null_strat` no-op; tests `mcore1_tunnela.rs` (2).
- [x] Teleporter + BOSSH ledger — `teleporter_{istrat,strat}` rise/bonfire/sflag1 retract; ledger-flip `bossh`/`bosshleg`/`bosshtop`/`bosshhitcount` (already tested); tests `teleporter.rs` (3).
- [x] ShipLB1 + shipOutofLB3 + boss1makechild — `shiplb1_*` / `shiplb1ychase`; `shipoutoflb3_*` cruise→wait→boost; extract `boss1makechild` from boss1 init; tests `shiplb1_outoflb3.rs` (3).
- [x] Pship/view OutofLB3 — `pshipoutoflb3_*` cruise/wait/boost; `viewoutoflb3_*` close→swing→endgame camera; ROM `viewlb3move_srou` (pviewpos pin); tests `pship_view_outoflb3.rs` (3).
- [x] Pship/view OutofLB1 — `pshipoutoflb1_*` climb→lineup→friends→turn→boost; `viewoutoflb1_*` follow pship + mapvar1 Lexp; tests `pship_view_outoflb1.rs` (3).
- [x] Pship/view DiveGnd — `pshipdivegnd_*` dive→spin→level→hand off to on-planet; `viewdivegnd_*` track + outvz roll + fin chase; tests `pship_view_divegnd.rs` (3).
- [x] Pship/view IntoLB1 — `pshipintolb1_*` climb→roll/open→boost→chase mapvar1→Ltunnel handoff; `viewintolb1_*` Z/Y/X offsets follow pship state; tests `pship_view_intolb1.rs` (3).
- [x] Pship colony/washent — `pshipcolony_*` / `pshipwashent_*` pipe-rot tables → straight handoff (Ltunnel / nucleus); wired into clearcolony/washent dup; tests `pship_colony_washent.rs` (3).
- [x] BossA cup srou/istrat — public `bossacup{open,up,uplow}_srou` / `getbossacupchild_srou` / `bossacupper{l,m,r}_istrat`; wired into cup strat UP/ROTATE/RETURN/DOWN; tests `bossacup_srou.rs` (2).
- [x] BossA turret L/M/R — public `bossaturret{l,m,r}_{istrat,strat}`; Icont + cont (fire/sweep/offs); tests `bossaturret_lmr.rs` (2).
- [x] BossB face core + bossflash — public `bossb_{istrat,strat,cont*}` / dodge / escape / range / pointdir / addpz/bhp; `bossflash_l` → dyingred window; tests `bossb_core.rs` (4).
- [x] ClearShip / ClearShip2 / playernull — `set_player_clear_ship{,2}` / `player_clear_ship{,2}_{istrat,strat}` / `playernull_istrat`; bg2 scroll ramp + stagedone; tests `player_clearship_null.rs` (4).
- [x] ClearTurn / ClearUnder / ClearEarth — `set_player_clear_{turn,under,earth}` + phase strats; Earth→ClearShip Icont; Under→UNDER2+sflag2; tests `player_clear_turn_under_earth.rs` (4).
- [x] ClearDemo / DIVE / ClearChase — full `player_clear_demo{,2}_*`; `player_dive{,2}_*`; `player_clear_chase_*` + chase2; tests `player_clear_demo_dive_chase.rs` (3).
- [x] Warp / WarpOut — `player_warp_{istrat,strat}` 3-state + hyperspace; `player_warp1/2_strat`; `player_warp_out_*`; tests `player_warp.rs` (4).
- [x] Boss7 col/b2/intropart + Boss8_STRAT — public `boss7hatchcol`/`boss7launchercol`/`boss7coll`/`boss7b2_{init,strat}`/`boss7intropart`; `boss8_strat`≡cont; tests `boss7_col_b2.rs` (6).
- [x] THEEND zoom/fin/flip/flyaway/check — `theend_{zoom,zoom2,fin,fin2,flip,flyaway,check}_*`; tumble push/pull + distraction spawn; tests `theend.rs` (7).
- [x] Player fly-in / straight / speed / on-cont / cred / divegnd — space/inside/planet/Ltunnel/colony flyin; straight cruise; speedup/stop; oncont; full cred strat; divegnd noop body; + ledger-flip already-ported LB/tunnel leaves; tests `player_flyin.rs` (7).
- [x] setboss / initgame_strats / drill.launchweb — `setboss_l` boss_seq append; `initgame_strats_l` flag/view reset; public `drill_launchweb`; shape/path/local-label structural flips (fireface/fireball/M*_STRAT/nofire_aggv/…); tests `setboss_initgame_drill.rs` (3).
- [x] PCOLRW / PENDCOL* — ROM-named `pcolrw_{istrat,strat}` + `pendcol{b,lw,rw}_istrat` (+ LW/B helpers); wing flash/Zshake/spexplod FX; broken-wing bounce to body; tests `pcol_pendcol.rs` (5).
- [x] BossF airship heli parts — `bossf{body,feet,arm,head}_{istrat,strat}` (+ hit/explode/headfire); mode tables; feet→heli mother; arm ironballs; head ringlaser/rotate; tests `bossf_heli_parts.rs` (8). **enemy_boss_strat False: 0.**
- [x] MAKEENGINE / MAKESPLASH / MAKESSPLASH / MAKESDRAG — `makeengine_srou`/`updateengine_srou` + splash/Ssplash + public `make_sdrag`; float-ground centroid A/B diagnostic; tests `makeengine_splash.rs` (6) + camera unit A/B.
- [x] CLSHIPBOOSTNOSND + UPDATEENGINE/DOMAKESPL/SR_MAKE_* — public `clshipboost{,nosnd}_istrat` / `clshipboost_strat`; `player_warp1_init` wires nosnd; structural flips `UPDATEENGINE_SROU_L`/`DOMAKESPL`/`SR_MAKE_{OBJ,CHILD}`; tests `clshipboost_nosnd.rs` (4).
- [x] FLASHTURQ/2 + FLASHRED + SR8/SR16 achase rates + SR_REMOVE — `Windows::{flash_turq,flash_turq2,flash_red,hitflash_off}` + strat wrappers; structural flips all `SR{8,16}_ACHASE_ALVAR*` / `SR_ACHASE_ALVAR_{END,FIN}` / `SR_REMOVE_OBJ{X,Y}` (fuzz_pure_fns + `strat_remove_obj`); tests `flash_hitflash.rs` (4).
- [x] DO_BGM_INIT/CONTINUE + VOFSON/OFFPLEASE + SOUND5LEN — `Sound::do_bgm_{init,continue}`; `GameVars::{vofs_on,off}_please` + map `VOFSON`/`VOFSOFF`; `boot::SOUND5_LEN`; tests unit_core vofs (3) + audio unit (2). **audio False: 0.**
- [x] MAKETOTALSCORE/2 + MAPWAITFADEDO + SETCHARMAP{GAME,PLAN,FOX}_L — `score::calc_average_score` / `score_digits` + `Planets::average_score`; waitfade park/advance test; `charmap::{CharMap,CharMapScreen}` + Hooks/shell; **map_menu False: 0.**
- [x] XYDIFFS(_ABS) + MAKEENDOBJ*/MAKENUM* + CHECKIFIAMEND + GAMECLIPWINDOW/CLEARHVOFS — `xy_diffs(_abs)` Manhattan (fixed bossBrange); `endscore::{makeendobj*,makenum*}`; `clip::{GameClipWindow,BgScrollOffsets}`; tests `endscore_xydiffs.rs` (7).
- [x] ADD2POSOBJYFOBJX + MOVEOBJTOEND + READ_JOYPAD(T) + SETFADE* + ARCTAN16 — `add2pos_obj_y_from_obj_x`; `Objects::move_obj_to_end`; `pad::read_joypad`; SETFADE* already map FADE*/QFADE*; ARCTAN16 via GSU/`strat_angle_xz`; tests `add2pos_joypad_fade.rs` (5).
- [x] XZDIFFS_ABS/OFF + DOBGREQ + TRANSSWAP + STARTSFX — `xz_diffs_abs`/`xz_diffs_off` Manhattan; `bgs::{do_bg_req,trans_swap}`; `clip::start_sfx`; tests `xzdiffs_bgs_sfx.rs` (4).
- [x] BUILD_DRAWLIST + WAITDMA(224) + CLEARSPRITES + FADEHALF2NORM + CALCBG2VOFFSETS — `draw::build_list` (ROM stub); `WaitDma` no-op; `Sprites::clear_sprites`; `Windows::fade_half_to_norm`; `bgs::calc_bg2_voffsets` gate; tests `drawlist_waitdma_fade.rs` (4).
- [x] FADETONORM + GAMEOVERINIT + DMA_SPRITES/DMABG2VOFFSETS/DMAHPOS + FADERED — `fade_to_norm_l` / `gameover_init_l`; `DmaFlush`; `fade_red_palette` BGR555; tests `gameover_dma_fadered.rs` (4).
- [x] FOXY_CONTINUE/TRANS + FOX_SPRITES + DRAWSOME3D + CLRONEHALF + FIND_OBJECT/TARGET + JUMPTOSTATE + ENDTRANS — `foxy::{foxy_continue_enter,FoxyContinue,EndTrans}`; `find_object` / `jump_to_state`; tests `foxy_find_jump.rs` (7).
- [x] ALLOC/SFREE/SALLOC/SALLFREE/AVAIL + MPUSH/MPULL/SMPUSH/SMPULL + FADETAB*/FADECOL0 + FADELINES + DEC_* + P_INIT_SPRITES — `heap::StratHeap` + fade tables + `DecRun`/`FadeLines`; tests `heap_alloc_fade.rs` (5).
- [x] PRINT* + CLIP_PLOT + PROJECTLOG/PROJLOG + COPYCHARS/COPY_TO_0101 + PALGOTO + DMA256*/PEPPER + DRAWPLANET* + MOVESHIPALONGPATH — `debug_draw::{DebugPrint,project_log,PlanetScreenDma,…}`; tests `debug_print_project.rs` (4).
- [x] FIND_NEAR*/RADIUS*/MOBJECT/SWORD1 + FIND_WINDOW_PRI + DYINGRED + MODECHANGE + PRINTAW + DRAWLINESBITBYBIT + HDMA_* — `find_near_object` family; `Windows::{dying_red,find_window_pri}`; tests `find_near_dyingred.rs` (6).
- [x] FLOAT256/128/32 + FLOAT_CONT/FLOUT + COUNT_SHAPES + WINGLAZER*INIT + INITGAME/3D/SCREEN/SPRITES + FNMI + MMAKE/MDRAW* — `float*_srou` / `count_shapes`; `BootInit`/`MarioDraw`; tests `float_count_init.rs` (4).
- [x] PERC*A + SETBG/INFO/RESTARTFADE + SET_*COLLPTRS + XFLYTOPOS + INITWMAT/MARIO/MEM/DUST + SETINIDISP/PAL* + DO_CIRCLE/WIPE/HPOS + RESET_SPRITES — `bgs`/`DisplayFx`/`set_*_collptrs`; tests `set_init_display.rs` (5).
- [x] SETOBJTOBECHILD{XY,YX} + SET_RESTART_POSITION + SETUP_PLANETS/TITLESEQ + EXITSPEC.DOFADEDOWN + SPROUTY.WITHDRAW_I + MSHOW*/MGRDRAWDOT*/OOPSHDMA + shape/path labels (MYZOOM/WALL/LFDIE/HOU/PE_*) — `set_obj_to_be_child_*` / `World::{set_restart_position,apply_restart}` / `BootInit`/`DisplayFx`/`MarioDraw`/`HdmaRegion`; tests `child_restart_title.rs` (4). **Ledger 2169/2169 True** (`other`/`render_dma` False: 0).
- [x] Float-above-ground cam pitch — `GameCamera` `rot_x`/`pull-back` from `outvx>>8` (+ `noxrot`); roll `outvz-plrotz` when `dozrot`; tests camera.rs (3) + `audit_player::getview_viewrot_vs_rom` MATCH.
- [x] Cam yaw residual — `rot_y = (outvy - player_turnrot)>>8` for all objects (was chase-feel `player.roty`); test `yaw_follows_outvy_not_player_roty`.
- [x] Shoulder-hold bank lean + houdai/zaco3 rangexz re-cert — `player_Ztilt ±= deg45/3` on TLEFT/TRIGHT (PSTRATS.ASM:2626); `audit_strats` MATCH vs ROM `xzdiffs_l`; tests `shoulder_ztilt.rs` + `houdai_zaco3_gates_match_rom_rangexz`.
- [x] Player-move Mediums — Y-bounds inclusive + `PML_BBOTTOM`; `BOOSTOBJ` on pad-X/force boost; `pfm_wobble` `pZrotfloattab` → `al_rotz`; outdist ease already present (re-tested). Tests `player_move_mediums.rs` (4).
- [x] Barrel-roll `s_beqdec` window + onfield view — start only while `rolldelay>0` (branch-before-dec); TLEFT→+32 / TRIGHT→−32; `set_player_on_field` / `player_on_field_strat` (perc87 X, fixed ViewCY; ASM under `ifeq 1`). Tests `player_onfield_barrel.rs` (4).
- [x] High #3 planet dispatch + Minor #12 ztilt gates — `playeronplanet_init` clears `game_mode` (exit-base no longer stuck in SPACE→limit_x); dpad ztilt skipped near floor / wing wall; water/space set `WATER_MODE`/`SPACE_MODE`. Tests `planet_yvel_ztilt.rs` (2).
- [x] Minor #13 bgsscrollZ ← viewposz — `GameCamera::update` WRAM-writes viewposx/y/z; `viewmove`/`playerdead` copy `VIEWPOSZ` (last getview). Tests `bgsscroll_viewposz.rs` + camera WRAM assert.
- [x] find_near `xzdiffs_l` — `find_near_object` / `strat_find_near_{shape,colltype}` rank+gate with `strat_dist_xz` (not Manhattan); Y ignored. Tests `find_near_xzdiffs.rs` (2) + `retail_find_nearobject_vs_port` MATCH.
- [x] HUD/SFX Critical #2 + Medium #6 + battery SE — Continue gated on `credits>0` (`dec` + `$67`/`$f1` on accept, else Title); talk flash frame absolute 4; `fire_*` plasma/oval/ring/yhplasma → `make_snd(EnemyBattry|RingLaser)`. Tests shell (2) + strings (1) + sound_wiring (1).
- [x] Laser/Missile/HitWall SE + bomb flash — `fire_friend/reb/slow` + `strat_fire_relslow*` → `make_snd(Laser|HitWall)`; missile family → `Missile`; boss2/sea/bossb laser+missile sites; HUD `specflash` blink (do_spec_weap). Tests sound_wiring + `bomb_flash_hides_newest_*`.
- [x] Shield wireframe + MoveWall SE — `item6` sets `shieldup=1`/`pnumhits=0`; playermove wire-end flash; HUD fill color 7; `wallleft/right_i` → `make_snd(MoveWall)`. Tests `wire_shield_movewall.rs` (3) + hud shield color.
- [x] Player damage/wing SE + death BGM + nova $30 — body `$04`/`$19` + shield `$1b`/`$1c`; wing `$07`/`$08`/`$05`/`$06` + wire `$14`; `playerdead` `play_music($11)`; nukeexp `$30` verified. Tests `player_damage_sfx.rs` (10).
- [x] HUD arrow $8A drain + pause SFX — submit→`take_pending_hud_sounds`→`play_hud_se`; Playing START `PauseSnd($02/$01)` + freeze nmi; gates wipe/stayblack/bf_dying/notdie. Tests hud arrow + shell pause (2).
- [x] nosetport3 + bird_touch — `SoundCmd::NoSetPort3` / `Hooks::set_nosetport3`; path inline CB registers; bird_touch sets LE_ENTERSPEC+nosetport3+bgm$2; planets_init/gameplay clear; ENTERSPEC→routechange1. Tests `nosetport3_bird_touch.rs` (4).
- [x] currentlevel WRAM wire + boss7 hard gate — begin_gameplay writes 0x1F03 = planets.currentlevel+1; boss7 `s_jmp_ifnotlevel 3` → port `== 3` (was wrongly `== 2`). Tests shell (2) + `currentlevel_wram.rs` (2).
- [x] ENDSEQ nosetport3 + boss1 Mediums 6–8 verify — route exhaust → Ending sets `NoSetPort3(true)` (ENDSEQ.ASM:358); boss1back firer-rots plasma/missile + boss1out beqdec cycle. Tests shell (1) + `boss1_back_out.rs` (2).
- [x] boss1 death Medium #9 — `boss1exp_init` → bossexplode (`$1e`/`$f1` + 15-child barrage + lifecnt 38 + deg90/32 spin). Tests `boss1_death.rs` (2).
- [x] AUDIT_BOSS Mediums 10–15 verify — bossseamon splash RNG order; seamon surface snap + swim-shape byte test; boss8a anim cap; nucleuslauncher objinfront; boss8die preserves bossmaxhp. Tests `boss_ticks2_mediums.rs` (6).
- [x] AUDIT_BOSS Minors 16–25 — viewposy Shyper; shrap RNG; boss1up/in/covdie boundaries; center enemy1 colltype; boss1back→boss1_fin bosshp fix; gsvar wrap; sea_gen vy; swim /16; launcher |dx|<200. Tests `boss_ticks2_minors.rs` (10).
- [x] AUDIT_BOSS Minors 26–28 + sea splash — colldisable≡collstrat=0; HPLASMA aim accepted; Istrat fall-through verify; `sea_make_splash`→`makesplash_srou` + landing worldy=0. Tests `boss_ticks2_gaps.rs` (5).
- [x] makeexpobj / delayexplode lifecnt — default count=0 explodes first tick; `s_decbpl` via `count_down` (survive count+1); fixed boss8/boss2 delayexplode early-fire. Tests `expobj_lifecnt.rs` (4).
- [x] AUDIT_BOSS_TICKS (boss2/bossg) High/Medium/Minor 1–8 + splash + kami vel — bosshp / full muzzle / rndrots / coin≥127 / sea_not_delay / Zdistmore / petal colldisable / toward-zero /8; bossg +30z splash; b8 kami vel 40. Tests `boss_ticks1_verify.rs` (8).
- [x] AUDIT_BOSS_TICKS Minors 9–10 + `.genspark` — bossgs BLACK_C flicker + Fchase_A overshoot; bossg spark at y−60 via `sgen_spark`. Tests `boss_ticks1_gaps.rs` (4).
- [x] AUDIT_BOSS_TICKS placeholders — boss2 leap → `particlefiredown_istrat` (3/4/9); bossg `.scrollmsg` tx+=4 is the texture scroll. Tests `boss_ticks1_placeholders.rs` (3). **AUDIT_BOSS_TICKS closed.**
- [x] boss8 kami → hmissile3 — `b8_fire_kamimissile` uses `fire_kami_hmissile1` + sflag1/al_ptr/zaco_9; twin-laser weave verified. Tests `boss8_kami_hmissile3.rs` (3). **AUDIT_BOSS_TICKS2 known gaps closed.**
- [x] AUDIT_SOUND_IDS F1–F6 — door/sea → positional `make_snd`; tow0/zaco3die silent (no bogus $10). Tests `sound_wiring_tests` (6) + `sound_ids_f3f4.rs` (2).
- [x] AUDIT_SOUND_IDS hitwall + separatemissile — `pelasercollide` solid → HitWall; `separatemissile_l` dead (0 STRAT callers). Tests `hitwall_separatemissile.rs` (3). **Positional SE families closed.**
- [x] AUDIT_ENEMY_A High #3/#4 notdelay+al1pt — houdai `(gf+idx)&15`; zaco0 `(gf+idx)&3`; para2/cameleon `&15`. Tests `houdai_cadence.rs` (3).
- [x] explode `s_test_special` — special|Cspecial → specials_dead++; no specialobjtotal dec / no GF_BOSSDEAD; lives WRAM 0x0520. Tests `explode_specials.rs` (4); `ea_rader0` re-blessed.
- [x] AUDIT_HUD Critical #1 + High #4 — score/tally/checkbonus/bonertab+$1a; bosshp zero+`boss_hp_cur`. Tests `hud_score_bosshp.rs` (2) + shell tally. **HUD Criticals closed.**
- [x] AUDIT_ENEMY_A High #1/#2 + HUD Crit #8/#9 — relslowlaser 48/60 life40 colltypes; `frame_tick_mod` bit-count; hitflash $24–26 + explode $21–23/noexpsnd. Tests `enemy_a_weapon_explode_se.rs` (4).
- [x] AUDIT_ENEMY_A High #5 — `jmp_higher`/`jmp_lower` worldy half-spaces (gate2/zaco2/zacos/zaco3die/zaco1). Tests `enemy_a_worldy_higher.rs` (5).
- [x] AUDIT_ENEMY_A High #6–#9 — Achase proportional (zaco3/4 circle, parajump, clship) + chase→clshipboost. Tests `enemy_a_achase_clship.rs` (5).
- [x] AUDIT_ENEMY_A High #10 — base1 HF1 door FSM (open/wait/close + DoorOpen/Close). Tests `base1_door.rs` (3). **enemy_a Highs closed.**
- [x] AUDIT_ENEMY_A Mediums #11–#15 — zaco2loop/wormgo leftpl, itemtorange height, zaco3/cameleon beqdec. Tests `enemy_a_mediums_11_15.rs` (5).
- [x] AUDIT_ENEMY_A Mediums #16–#22 — zaco4 fallthrough/leftpl, zaco3die signed pitch, zaco3go stale vecs, para initface/aim/gravity. Tests `enemy_a_mediums_16_22.rs` (7).
- [x] AUDIT_ENEMY_A Mediums #23–#33 — item5 HP0/specflash, up1man, clship cont/warp, zaco1 spiral, friendexitbase, gate2 rangexy, skillfly behind (#25 accepted, #29 superseded). Tests `enemy_a_mediums_23_33.rs` (9).
- [x] AUDIT_ENEMY_A Mediums #34–#37 — hard90yr no enemy1; delayexplode s_decbpl; pillar3explode 8-child silent; init→strat fall-through. Tests `enemy_a_mediums_34_37.rs` (8). **enemy_a Mediums closed.**
- [x] AUDIT_ENEMY_A Minors #1–#2 — relslowlaser muzzle Z80 rotated + laser colltype; relelaserhome lock `|dz|<800`. Tests `enemy_a_minors_1_2.rs` (4).
- [x] AUDIT_ENEMY_A Minors #3–#5 — jmp_distmore strict bounds (tadpole/zaco1_phase0 fixes); zaco1 mid-band `[1400,1800)`; zaco0 `(rnd&3)-1` pitch-then-yaw. Tests `enemy_a_minors_3_5.rs` (5).
- [x] AUDIT_ENEMY_A Minors #6–#8 — zacos muzzle Z120; clship flyin sflag1 notdelay gate; zaco2loop HMISSILE1 level!=1. Tests `enemy_a_minors_6_8.rs` (4).
- [x] AUDIT_ENEMY_A Minors #9–#14 — bomwing/cameleon no enemy1; flashplayer colframe; gate bank $7E + touch→spin; explode special→gate2 + inviewpl (#13 superseded). Tests `enemy_a_minors_9_14.rs` (7).
- [x] AUDIT_ENEMY_A Minors #15–#16 — zaco1 phase fall-through + SINTAB spiral; szaco2 relexplode; hard_Istrat no enemy1 + hardenemy1@104. Tests `enemy_a_minors_15_16.rs` (6). **enemy_a Minors closed.**
- [x] ISTRATS shark@60 / fzaco@113 / hard90yrfog@183 + tank/houdai5f off-by-one — was skipping hard90yrfog so tank1a..houdai5f sat one low; maps + `enemies_ground` aligned to ROM. Tests `istrat_shark_fog_tanks.rs` (5).
- [x] AUDIT_ENEMY_A Minor #15 smoke/debris leftovers — zaco3die/go `makesmoke` + go `vz=40`; szaco2 `debrisshape=zaco_8`; die double `add_player_z` fix. Tests `enemy_a_minors_15_smoke.rs` (3).
- [x] AUDIT_ENEMY_A Minor #15 yaw nega — `strat_aim_yaw`/`_3d`, zaco1_phase2, para2 latch store `nega(Yanglexy)` for movement; fire keeps raw angle_xz. Tests `enemy_a_minors_15_yaw.rs` (2). **#15 closed.**
- [x] Inline obj2obj / face_player yaw nega sweep — headfire, helpballhome, homingflat, spacebarwalker body, stbfp/bee1a latch, evader, cam2dash, sdragonfly OFF, blowcube OFF, bonfire/ironball2/3/4 projectile aim all store `nega(Yanglexy)`; weapon_rots2obj fire stays raw. Tests `enemy_a_inline_aim_yaw.rs` (6).
- [x] True `zaco_8p` mesh extract — `EXTENDED_SHAPES` slot 283 via `shape_compiler.py`; szaco2 `debrisshape=SH_ZACO_8P` (was zaco_8/105 stand-in). Tests `zaco_8p_debris.rs` (2) + `shape_data_parity` + smoke re-bless.
- [x] Boss-lane inline obj2obj yaw nega — bossA cup GO `sbyte3`, boss8 `b8_aim_3d` + homing shot, bossbrob body/frontplayerZ/rndpos store `nega(Yanglexy)`. Weapon fire stays raw. Tests `boss_inline_aim_yaw.rs` (1) + bossb/cup suites re-blessed.
- [x] Boss2 / flingboss / chick / sd_head yaw nega — `boss2_homelaser` / `hmissile2` / `hplasma` / `flingboss_hmissile1` chase `nega(Yanglexy)`; `chick_istrat` + `sd_head` player aim store nega; `chicken_gen3dvecs` → `strat_gen_vecs_3d` (n3dvecs). Fire spawn yaw stays raw. Tests `boss_inline_aim_yaw.rs` (3) + chick/chicken/boss2 re-blessed. Accepted #15 leftovers: para2 xz-only add, zaco0_sweep unsigned compares.
- [x] Sea `s_gen_vecs` → `nvecs_l` + flyingfish/bossseamon yaw nega — `strat_nvecs` (`-angle+1`); `sea_gen_vecs_angle` / `chicken_gen_vecs_roty` use it; flyingfish jump + bossseamon state7 store `nega(Yanglexy)`. Fire stays raw. Tests `sea_nvecs_yaw.rs` (3) + boss_ticks2/chicken re-blessed. Accepted #15 leftovers unchanged.
- [x] AUDIT #15 leftovers closed — para2 `nvecs` + full first `add_vecs2pos` (was xz-only workaround for `gen_vecs_2d` zeroing vy); zaco0_sweep unsigned worldy compare/clamp. Tests `enemy_a_minors_15_leftovers.rs` (4) + mediums/yaw re-blessed.
- [x] `s_gen_vecs` → `nvecs_l` sweep — bomwing phase1, clship bridge/underboost, walking/walking2, volrock, misstank, walker1, sokuten/heading-sbyte1 paths, tank2 `gen_vecs_2d_signed`. Tests `s_gen_vecs_nvecs_sweep.rs` (3) + enemies_ground/clship/bomwing re-blessed.
- [x] `s_add_Roffs2pos` full → `rotate_8*` — `strat_roffs_full`/`_scaled`/`_i16` in snes_trig; enemies_ground/bosses/enemy_a float helpers replaced; relslowlaser/zacos/ironball muzzles use byte+ASL. Tests `roffs_full_rotate8.rs` (3) + muzzle suites re-blessed.
- [x] `s_add_Roffs2pos` yaw-only → `rotate_8xz` — `strat_roffs_yaw`/`_scaled`/`_i16` + roll+yaw `strat_roffs_roll_yaw*` (flags 0,1,1); wired `boss_yaw_offset_pos` / `b2_yaw_offset_pos`; bossF full Roffs + smoke; spacepilon/up1man → `rotate_8yx`. Tests `roffs_full_rotate8.rs` (6) + bossA turret re-blessed.
- [x] helpball Z-orbit + boss2 circle sintab — `strat_roffs_roll` (flags 0,0,1); helpball → `rotate_8yx`; fixed smoke flags 0,1,1 to roll+yaw (was mislabeled pitch+yaw); boss2 state4 vx/vz from SINTAB/COSTAB adiv2. Tests `roffs_full_rotate8.rs` (8) + `helpball.rs` orbit + `boss_ticks1_verify` re-blessed.
- [x] boss2plasma non-uniform Roffs + `strat_tab_scaled` SINTAB — `strat_roffs_yaw_scaled_xyz` (scales 2,0,4); plasma orbit no longer pre-`<<4` then rotate; `strat_tab_scaled` uses SINTAB/COSTAB + toward-zero `/` (was float×127 + arith `>>`). Tests `roffs_full_rotate8.rs` (9) + `boss2plasma_roffs.rs` (1) + `ea_units` re-blessed.
- [x] Roffs flags 1,1,0 + dobossrot 0,1,1 — `strat_roffs_pitch_yaw*` (updateengine); `dobossrot`/`x4`/`doboss2rot` → `strat_roffs_roll_yaw_scaled` (was yaw-only + pre-shift). Tests `roffs_full_rotate8.rs` (11) + `makeengine_splash` + `ship1_ship3_boss2rots` re-blessed.
- [x] `boost_Istrat`/`boost_strat` + pcbox wing `rotate_8yx` — flame parks via pitch+yaw + `boostZoff`; `boost_sprite` wired into `clshipboost_enter`; wing `rotz_offset` → `strat_roffs_roll`. Tests `boost_flame_roffs.rs` (4) + pcbox/clshipboost re-blessed.
- [x] `boost_sprite` call sites + spacepilonP roll×3 — wire flame into lb2a / escape_nucleus2 / openingboost / shipintro / friendstart3 / shipoutoflb3 (`#10` then `boostZoff=-80`); `strat_roffs_roll_scaled` for spacepilonP flags 0,0,1 scales 3,3,3. Tests `boost_flame_roffs.rs` (6) + `roffs_full_rotate8.rs` (12); eb_parity re-blessed.
- [x] spacebar2 Roffs fidelity — flags 0,0,1 via `rotate_8yx` on **rotz** (was float yaw on roty); B-mode `#Xspacebarlen/2` → i8 −6 (was 250). `sf-game::trig8` mirrors mulslog/rotate_8yx for world-lane builtins. Tests `spacebar2_starbull.rs` (5).
- [x] spacebar3 `rotate_16xz` — replace float XY park with ROM `rotate_16xz(nega(parent.rotz), sword1, sword2)` (z′→worldy) + spacemist; friendstart1/2 skipped (`ifeq 1` + commented ISTRATS). Tests `spacebar2_starbull.rs` (6).
- [x] spacebar/SPINspacebar/spacebar1 spacemist — replace bogus `pviewvelz` scroll with ROM `s_spacemist` (no add_playerZ; spacebar2 keeps scroll). Tests `unit_core` spacebar (3).
- [x] trig8→sf-core + world achase ROM — shared `sf_core::snes_trig` (SINTAB/mulslog/rotate_8yx/16xz/achase_angle_8); `sf-game::trig8` + `sf-strat::snes_trig` re-export; world SPINspacebar achase matches ROM antipodal; removed dead float `snes_sin`/`strat_sin`. Tests `unit_core` achase+spacebar (4) + roffs/spacebar2 green.
- [x] path P_SPAWN Roffs→rotate_8* + rotate_8xz/yz/16yz→sf-core — `path_add_rotated_offset` uses `strat_roffs_full_scaled`; shared `rotate_8xz`/`rotate_8yz`/`rotate_16yz`/`strat_roffs_full*` in `sf_core::snes_trig`. Re-blessed `pi_tow_0`. Tests `path_roffs_rotate8` (3) + `roffs_full_rotate8` (12) + `interp_trace`.
- [x] aim_angle→sf-core + Xanglexabs Manhattan — shared `xzdiffs`/`xzdiffs_abs_manhattan`/`yanglexy(_nega)`/`xanglexy`/`xanglexabs`; fixed WP/obj pitch (was float hypot) + world spacebar2 elev (was crude |dx|+|dz|); `strat_angle_yz_abs` now Manhattan per ROM. Tests `aim_angle` (3) + `anglexy_compose` (4) + yaw/spacebar suites green.
- [x] s_goto_WP / P_GOTOPOS / face* aim_angle — saucer1/fly/starbull/evader WP + path GOTOPOS use `yanglexy_nega`+`xanglexabs`; FACEPLAYER/FACESHAPE/WAITFACEPLAYER apply Yanglexy nega (s_obj2obj_3dangle). Dropped path libm atan2. Re-blessed pi_ponpon (+ tow_0). Tests `goto_wp_aim_angle` (2) + evader/starbull/interp_trace green.
- [x] enemy_a angle_xz→yanglexy + retail muzzle rotate8 chain — local i32-promote twin → ROM i16 wrapping `yanglexy`; surgical retail `rotate_8yx→yz→xz` (z1=$90/z2=$15C2) MATCH `strat_roffs_full` 8/8 (closes UPDATE 9 deferred muzzle sub-step). Tests `enemy_a_angle_xz_yanglexy` (2) + `retail_muzzle_rotate8_chain_vs_port`.
- [x] sound makesnd/nearobjs → `sf_core::aim_angle` — `xzdiffs_rangexz` → `xzdiffs`; nearobjs pan `angle_xz` → `yanglexy` (drop float hypot/atan2 twins). Test `sound::tests::sound_aim_helpers_match_sf_core` (+ `sf-audio` lib).
- [x] camera VIEWTYPE_TOOBJ look-at → aim_angle — ROM `nega(Xanglexy)`+`Yanglexy`+raw `outvz` (GAME.ASM:133-147); drop float hypot/atan2; write look-at words back to `outvx`/`outvy`. Tests `toobj_lookat_*` (2) + prior camera suite (9).
- [x] camera pull-back → `rotate_16yz`/`rotate_16xz` — ROM getview X-then-Y mulslog chain on `(0,0,-outdist)` (GAME.ASM:66-113); drop float sin/cos; yaw from `outvy-turnrot` now affects pull-back. Tests `pullback_uses_rotate16_*` (2) + camera suite (10).
- [x] bossFC2/FC3 objinfront + playerturn180 + bossFtur fire window — verify AUDIT_ENEMY_B Criticals #1–#2 + High #12 (already ported): FC2 `me.z<pl.z` / FC3 swapped / HP0 gate; turret `sbyte2<=15` + notdelay 3. Tests `bossfc_objinfront.rs` (5).
- [x] bossA Criticals #3–#9 verify — GO return/no-fire, GO-only-last-cup vs IROTATE, turret husk+DOWN revive (parent sbyte3 count); #5–#6 covered by prior `bossaturret_lmr`. Tests `bossa_cup_criticals.rs` (5). AUDIT_ENEMY_B Criticals closed.
- [x] enemy_b Highs #10–#19 verify — achase toward-zero, FA vz ASL + chase rate4, boss7d sintab scales, FC intro countdown, FC2 fire gate `sb2>=3`; **fix** bossFB/other ENEMY1 colltype `0x10` (was vars `0x01`). Tests `enemy_b_highs.rs` (7). AUDIT_ENEMY_B Highs closed.
- [x] enemy_b Mediums #20–#26 verify — spacepilon rnd2pos/Achase, boss7fall+bossA breakup, bossa intro, staydist per-tick, FCdie2 X<<1, boss7 yaw=`sbyte2`. Tests `enemy_b_mediums.rs` (7). AUDIT_ENEMY_B Mediums closed.
- [x] enemy_b Minors verify — up/cover SE gate, cover DOWN `>=20`, parent no collstrat, turretM sbyte3=0, cup home Z−2<<scale + open cap6, FC2/FA muzzle `±20<<bossF_scale`. Tests `enemy_b_minors.rs` (6). **AUDIT_ENEMY_B closed.**
- [x] AUDIT_BOSS_TICKS2 Highs #1–#4 verify — boss1 fire bitmasks (+al1pt/+15), back retreat `|dz|<1500`, ring/muzzle <<1/±384/z+40 + full rotate8, mother/turret/boss8 `add_bosshp`. Tests `boss1_highs.rs` (4). **AUDIT_BOSS_TICKS2 Highs closed.**
- [x] AUDIT_PLAYER_MOVE High #1 verify — boost/brake `sbyte2` pulse + noctrl/stayblack/wipe gates (SE once per burst). Tests `player_boost_brake.rs` (3). **AUDIT_PLAYER_MOVE Highs closed.**
- [x] AUDIT_ROUTE_PROGRESSION Findings 1–3 verify — `shell::le` + `warp_advance` (BHOLE/SPECIAL/ENTER skip tally + `routechange*`); `level1_5` `mapend(7)`; blackhole strat sets LE_ENTERBHOLE. Tests shell warp (4) + `blackhole.rs` (8) + `mapend_sets_levelfinished_le_startgame`. **AUDIT_ROUTE_PROGRESSION closed** (#4–#5 accepted).
- [x] draw `build_list` yaw cull → `rotate_16xz` — behind/leftpl use mulslog SINTAB/COSTAB (drop float sin/cos); HUD sea #11 marked FIXED via prior `make_snd`. Tests `draw::tests` yaw cull (2) + prior (2).
- [x] volrockdown scatter + wall swing public entry — `wallleftright_istrat` pub; apex RNG `(rnd&15)-7/(rnd&7)-15/(rnd&15)-7`; left/right hold 192/64. Close TIER2 wall/volrock blockers + AUDIT_PURE_FNS antipodal FIXED + HUD #10 FIXED. Tests `volrockdown_wall_swing.rs` (4) + `fuzz_pure_fns` (7).
- [x] torpedo full-body Achase + splash/upsea — pub `torpedo_istrat`/`torpedo_strat`; `makeSsplash` submerged; surface `makesplash`+`EnemyUpSea`; yaw Achase rate-3 + n3dvecs cont; pitch Achase rate-2. Tests `torpedo_achase_splash.rs` (3). **TIER2 torpedo closed.**
- [x] shou0/bazooka/houdai5f full-body fire — shou0 PLASMA `EnemyBattry` + raw Yanglexy aim; bazooka RELSLOW `Laser`; pub `houdai5f_*`; (player-aim, not find_nearobj). Tests `shou0_bazooka_houdai5f_fire.rs` (4). **TIER2 firing FULL BODY closed.**
- [x] item7 broken-wing → ripair spawn — drop inline repair; spawn `ripair_Istrat` (`$8b`), defer `$17`/flag clear to catch; intact `$15`/score/upgrade; Istrat fall-through. Tests `item7_ripair_spawn.rs` (4). **AUDIT_ENEMY_A #25 FIXED.**
- [x] misspod/walker1/truck custom fire → `missilesound_l` — `misspod_spawn_missile2` + walker1/truck HMISSILE1 spawns `make_snd(Missile)`; `misspoda_init` restores `trigse $49`. Tests `misspod_walker_truck_missile_se.rs` (4).
- [x] boss custom HMISSILE → `missilesound_l` — `boss1_fire_hmissile1` (enemy_a+b) + `boss7launcher_fire_hmissile1` + flingboss/propturret fire `make_snd(Missile)`. Tests `boss_custom_hmissile_se.rs` (4).
- [x] custom RELSLOW/HPLASMA/SHORTPLASMA SE — winglazerman + boss1turret + bossFA `Laser`; bomwing/houdai/chicken-arm `EnemyBattry`. Tests `custom_laser_plasma_se.rs` (6).
- [x] more custom weapon SE — boss2spark/spacebarwalker/bossFtur/ship3 `Laser`; spacepilon/bossbrobP1/bossHtop `EnemyBattry`. Tests `more_custom_weapon_se.rs` (7).
- [x] bouncyball projectile/impact family — `fire_plasma`, `fire_beamball`, `fire_shortplasma`, `fire_hplasma`, `pillar3_enter_fall`, and `pillar3ffall_init` all allocate the authored `bouncyball` mesh through one semantic shape constant; pillar3 uses copypos z−10, pillar3f uses copypos without an offset, and the fog-pillar initializer now executes its authored first body in the same frame. Exact shape, position, callback, kill-state, timing, and weapon-stat regressions pass in `pillar3_fall_bouncyball.rs`, `relelaser_flatmiss.rs`, and `oval_ring_yhoming.rs`. **AUDIT_ENEMY_A #36 bouncyball leftover closed without a null-shape substitute.**
- [x] bazooka destruction debris chain — both `bazz_1` initializers and both map-used bazooka initializers store their authored body-debris and detachable-barrel meshes in named flat object fields; `bazexp_istrat` creates the pose-matched barrel with its 30-tick fall behavior before the generic explosion emits the two body pieces. `bazz_1p`, `bazz_1q`, `bazooka1`, and `bazooka2` are compiler-generated directly from PSHAPES, bringing deterministic coverage to 472 shapes. Exact initializer, debris count, shape, position, rotation, countdown, and collision regressions pass with the full strategy/workspace/routes/coexec/build/architecture matrix.
- [x] walking-mech topple presentation — leg failure selects the exact `walker_r` or `walker_l` body, preserves the authored local shift/wobble/fall chain, uses the visible `explosion` mesh for the right-fall and final effects, and deliberately retains `nullshape` for the left-fall effect. `walker_l` is generated directly from SHAPES, bringing deterministic coverage to 473 meshes. Exact direction, asymmetric effect, final-impact, stable-ID, and 17/21 versus 18/23 geometry regressions pass with the full strategy/workspace/routes/coexec/build/architecture matrix.
- [x] flat sprite weapon presentation — every `s_sprite_obj` plasma, beamball, oval, ring, short-plasma, H-plasma, and Y-homing constructor carries typed `ScaledSprite` presentation; the tank-local H-plasma duplicate allocates visible `bouncyball` instead of an invisible null shape. Exact mesh/statistics/visibility/presentation regressions pass across 129 focused tests with the full strategy/workspace/routes/coexec/build/architecture matrix.
- [x] remaining SF1 ordnance presentation — custom misspod, walker, truck, boss1, bossA, boss7/zaco2, flingboss, and webmonster missiles allocate the visible authored missile mesh; `misstank` keeps its distinct `small_m` carrier; player beam, nova bomb, helper balls, boss2 plasma, and custom H-plasma paths select their exact source shapes and typed `ScaledSprite` presentation. Exact mesh/visibility/presentation/statistics/sound regressions pass across 149 focused tests with the full strategy/workspace/routes/coexec/build/architecture matrix.
- [x] complete SF1 software-sprite presentation — all 44 strategy `s_sprite_obj` sites carry the exact typed visual kind, depth colour, and optional size, including boost, engine, splash, smoke/puff/flash, meteor/volcano, bonfire/ironball, chicken, amoeba, and blackhole families. The path `.sprite` opcode and game bridge use the same flat typed concept; no packed software-sprite bit remains in shipping Rust. Exact family, opcode, trace, and round-trip regressions pass with the full workspace/routes/coexec/build/architecture matrix.
- [x] bossB `fire_home` SE by weapon family — HMISSILE1 / CHICKHMISSILE1 / BOSSHMISSILE1 → `Missile`; RELSLOWELASERHOME → `Laser`; spinend close uses laser-home (was bare RELSLOW). Tests `bossb_fire_home_se.rs` (5).
- [x] chicken `egg_istrat` hatch chain — fall → hatch (shell `boss_d_6` + chick `boss_d_4` + `$3a`) / wait-to-hit / bounce→`nothing_istrat` (was instant `strat_explode`). Tests `chicken_egg_hatch.rs` (4).
- [x] chicken `firebreathe_istrat` trail/ground-bounce — vel 80/120 → trail `short_istrat` each tick; ground `worldy>=0` re-aim Yanglexy+nega → `.backagain`; OUTLIMIT `|x|` / Z≥4000 → short fade. Arm + seadragon head wired. Tests `chicken_firebreath.rs` (5).
- [x] chicken `wings_istrat` flap/fold — hardHP/AP + hitflash/explode; 0x80 anim; flap `#1,#15` reset at 14→4; sflag1 fold `#-1,#15` hold at 0. Tests `chicken_wings.rs` (4).
- [x] chicken arm `sprouty.expl` → `.fall_istrat` — chain walk set_strat fall; head `explode_istrat`+kill; children vel30/rnd yaw/vy−10 spin+gravity → explode on land (was instant remove). Fixed `chick_istrat` ENEMY1→`ACF_COLLTYPE2`. Tests `chicken_arm_sprouty_expl.rs` (4).
- [x] flingboss `sprouty.expl` shared fall-chain — `pullthearmsoff` / `deadflingboss` use deferred kill (no nested `aldead`); `chicken_arm_init` ENEMY1→`ACF_COLLTYPE2`. Tests `flingboss_sprouty_expl.rs` (3) + flingboss death green.
- [x] bossB `bossB_cont` image trail — sflag1 + sflag4 every-other-frame spawn `bossBent` / `bossBspinend` (sflag2); sword1 hi slot copy/inc; fixed `newent`/`spinend_cont` hi-byte. `flingboss_arm_init` ENEMY1→`ACF_COLLTYPE2`. Tests `bossb_image_trail.rs` (5).
- [x] ENEMY1 colltype + bossB sflag5 — bosses.rs sweep `COLLTYPE_ENEMY1`(0x01)→`ACF_COLLTYPE2`(0x10); scream/ouch/sepcol/start use `ASF3_SFLAG5` (sflags3 bit0; was image `ASF2_SFLAG1`). Tests `enemy1_sflag5.rs` (6).
- [x] bossH / bossF heli ENEMY1 — `bossh` + `bossf_heli` drop vars `COLLTYPE_ENEMY1`(0x01)→`ACF_COLLTYPE2`; drop unused enemy_b vars import; madtrucker test expects 0x10. Tests `bossh` + `bossf_heli_parts` (9) + `madtrucker` (10).
- [x] spacebar ENEMY1 + `palfade_num` bridge — world spacebar hardvars `ACF_COLLTYPE2`; shell/render pass ROM `palfade_num` (u16) not f32 fraction; `mixed_shape_palette_from_num`. Tests `unit_core` spacebar + `color_resolution` (3 fade).
- [x] retail `mapobjdo` spawn VM coexec — derive `RETAIL_MAPCNT/PTR/BANK/LASTMAPOBJ`; `newobjex` MAPOBJ vs `Game::map_exec` world coords MATCH. Tests `coexec_retail` `retail_map_*` (2).
- [x] multi-op `newobjex` coexec — MAPOBJ×2 (frame0 continue) + MAPOBJ→WAIT; world set + mapcnt/mapptr MATCH. Tests `retail_mapobj_multi_spawn` + `retail_mapobj_then_wait` (4 map total).
- [x] mapobjdo shape/stratptr encoding — retail `shapes[]`/`istrats[]` words applied; port flat id + `StratId` at index 166 (spacebar); istrat shape_byte=145. Test `retail_mapobj_shape_stratptr_encoding`.
- [x] retail pure helpers `nvecs`/`alvelvecs`/`perc*` — locate + coexec MATCH vs `strat_nvecs` / `strat_gen_vecs_2d` / `strat_perc*`; closes TIER2 leftover after speed_to/xzdiffs/n3dvecs. Tests `retail_nvecs_*` + `retail_perc_*` (4).
- [x] retail non-spawn map opcodes — WAIT/WAIT2/END + FADETOSEA/GROUND + SETBGM HP0 vs cart; palfade/bgm WRAM derived; END notes port `levelfinished` latch (ROM `stx mapptr;rts` only). Tests `retail_map_wait_*` / `fadetosea_*` / `setbgm_*` / `nonspawn_addresses` (4).
- [x] retail map LOOP/SETVAR/JMPVAR — `maploopdo` slots `$174B/$1743/$1753`; SETVARB/W/L ext WRAM; loop stored-C→C+1 waits; JMPVARLESS/MORE/EQ signed compare. Tests `retail_map_loop_*` / `setvar_*` / `jmpvar_*` (4).
- [x] retail map JSR/RTS/GOTO + SETALVAR/SETVAROBJ — stack `$1703`/`$1730`/`$1732`; JSR return+wait; SETALVARB/W/L + invalid skip; SETVAROBJ sentinel. Tests `retail_map_jsr_*` / `setalvar_*` / `setvarobj_*` (4).
- [x] retail map REMOVE + small state — first shape match only; rot/zrot/setstage/setbg/special(+cspecial); WRAM `dozrot=$16F1`/`stagecnt=$15B9`/`currentbg=$1741`; SPECIAL sflags→sflags4 remap. Tests `retail_map_remove_*` / `small_state_*` (3).
- [x] retail map IF/CODEJSL/SETPATH — IF SEC→else / CLC→+6,mapcnt=1 (WRAM stubs); port unknown≡SEC; CODEJSL advance; SETPATH mapptr+3 (path-resolve remap). Tests `retail_map_if_*` / `setpath_*` (3).
- [x] retail map VOFS/HOFS/fade — WRAM `bg2scroll=$194D` `dohofs=$1953` `dovofs=$1954` `fadedir=$18B2` `fade=$18B3` `xinidisp1=$7E45F4`; HOFSON/OFF latch `GameVars.dohofs`; FADE*/QFADE* → fade hooks ±1/±2; WAITFADE park/advance. Tests `retail_map_vofs_*` / `fade_waitfade_*` (3) + `op_hofson_off_latches_dohofs`.
- [x] retail boss8 phase machine — fix `B8_SFLAG4=$80`/`B8_SFLAG5=sflags3:$01` (ROM make_sflag); wait↔a↔b coexec (beam sflag1 gates, sbyte2 countdown, sflag4 open/close, beam clears). Tests `retail_boss8_phase_*` (2) + mediums/gaps.
- [x] retail boss2 states 1–3 — `stratstate@$1CDC,x` / `svar_byte5=$1530`; call() honors `Entry.dbr` ($7E for xalblks); leap entry→2, slam flip+falldown, back-away achase. Tests `retail_boss2_state_*` (2). States 4–5 (laser/die) remain gap.
- [x] retail boss2 states 4–5 — circle non-fire (sintab/costab vx/vz, top child held); no-top →5 transition vecs; player-dead falldown+add_playerZ. Test `retail_boss2_states_4_5_vs_port`. Fire-band closed tick 238; alive-path exp remain gap.
- [x] retail bossg modes 0/1/11 — `al_stratstate@$1CDC,x` ↔ port `stratmem`; `al_tx@$1CF4,x`; mode0 far wz−40; mode0→1 near gate into scrollmsg; mode1 tx+=4+add_playerZ; mode11 waitabit2 sbyte1++/move2 (odd gameframe skips splash). Test `retail_bossg_modes_vs_port`. Fish/shadow spawn modes remain gap.
- [x] retail bossg modes 3/4→5/6→7/7/32 — runaway stay (+70+pvz, bossmaxhp=0); disappear→waitsometime (maptrigger|1, nullshape remap); appear→moveto600h (HP=120/AP=8/bossmaxhp); moveto600h far; waitabit sbyte1++. Test `retail_bossg_modes_more_vs_port`. Fish/shadow/opentrunk spawn remain gap.
- [x] retail bossg trunk/sf9e — `al_animframe@$1CE7,x`; mode2→3 sf9e; mode8 opentrunk mid anim++ + open@9→11 fish cascade (fish undiffed); mode12 closetrunk mid anim−− + close@0→13. Test `retail_bossg_trunk_anim_vs_port`. Shadow-gen + fish body remain gap.
- [x] retail bossg generateshadows + bossgs — mode31 spawns 3 clones (sword1 −100/0/100, wz−50, rots copy, stratptr=`bossgs_istrat@$04F55E`) → waitabit; bossgs body Fchase worldx→sword1 ±5 + sbyte1−− + add_playerZ. Test `retail_bossg_genshadows_vs_port`. Fish AI remain gap.
- [x] retail flyingfish — fix sflag bits to ROM make_sflag (landed=$20/sflag2, side=$40/sflag3); INIT HP/AP/roty+180; swim achase ±200; `.flying` vy+2+addvecs. Test `retail_flyingfish_vs_port`. bossg mode-table closed.
- [x] retail boss8a HPLASMA — `gameframe&31∈{25,30}` fire (26 quiet); shot HP=1/AP=10/vel=60/life=50, yaw=firer.roty+deg180, `al_ptr=playpt`. Easy path → cont. Test `retail_boss8a_hplasma_vs_port`.
- [x] retail boss2 fire-band — state4 `sbyte4≤25` + even `gameframe` → `RELFASTELASER` (HP=1/AP=2/vel=90/life=40); `sbyte4>25` / odd frame quiet. Test `retail_boss2_fireband_vs_port`. Alive-path exp/RNG remain gap.
- [x] SF1 Macbeth bossH full choreography — replace the condensed timer path with typed 22-mode mother and 15-mode leg machines; restore child-scale full-transform placement, raised-leg gates, impacts/smoke, protected/red leg collision phases, teleporter follow/retraction, oriented plasma fire, falling leg explosions, and coupled boss bar/death. Tests `bossh.rs` (9) + `teleporter.rs` (3) + `more_custom_weapon_se.rs` (7), full `sf-strat`, all three unattended routes, 107/107 retail coexec, workspace, and architecture gates.
- [x] SF1 Route 1 Andross retail recertification — restore active-bit animation, yaw-only jump generation, exact fall/bounce and countdown timing, ordinary one-based split links, parent shutdown/collision resume, zone-specific walking-form reactions, damage smoke, and the source `boss_b_0`/`boss_b_6`/`boss_b_7` meshes. Focused encounter and 468-shape parity regressions, full `sf-strat`, all three unattended routes, 107/107 retail coexec, workspace/GPU, deterministic generation, and architecture gates.
- [x] SF1 hyperspace retail recertification — restore same-tick initializer fallthrough, exact five-draw screen-space placement and roll offset, deferred streak movement, 64-tick `hyper`→`hyper4` shrink phases, the three missing source meshes, and MOBJ's unconditional Face2 line semantics. Exact strategy/mesh regressions, deterministic 468-shape generation, full `sf-strat`, all three unattended routes, 107/107 retail coexec, workspace/app/GPU, default build, and architecture gates.
- [x] SF1 Macbeth madtrucker route gate and escort effects — replace the erroneous `line_2` gate with the named `air_1` mesh identity; advance both shared float phases under the authored wobble gate; restore madbiker sound, typed engine child/update, hover, and wall sparks plus the wreck's rear/side scrape sparks. Exact map, oscillator, engine, hover, and spark regressions pass with Route 2 parity, complete SF1 packages, unattended routes, 107/107 retail coexec, workspace/build, and architecture gates.
- [x] SF1 object-anchored fill circles — replace the red-only screen-centred special case with flat RGB/radius/phase state, resolve typed object identities to world coordinates without exposing invisible helpers to the draw list, preserve the player-death red fill, and restore Macbeth's three-phase white crash fill plus wreck-pass cleanup. Exact phase, anchor, strategy, and real GPU placement regressions pass.
- [x] Route 2 exact mesh identities — audit all 19 stale `*_PROXY` aliases against the generated retail catalog, replace them with shared semantic constants, and prove each map identity selects the expected named nonempty mesh. The emitted map bytecode remains exact across all 11 Route 2 oracle fixtures.

## Conventions
- Prefer `sf-oracle` differential tests over hand-reading alone.
- Keep each commit to one verified leaf or tightly coupled family, then push
  and confirm the remote branch resolves to that exact commit.
- Update `docs/ACCURACY_AUDIT.md` when a subsystem flips VERIFIED/FIXED.

- [x] Particle/mark explodes — `particleexplode`/`fast`/`big`/`circ2`/`circ` + `particlefire*` + `S/M/Lmarkexplode`; tests `particle_explode.rs`.
- [x] Hover / implode / stopexplode / weapcollide — tests `hover_implode_weap.rs`.
