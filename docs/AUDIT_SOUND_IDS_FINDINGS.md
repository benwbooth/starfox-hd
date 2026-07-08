# Audit: Sound-effect IDs & call semantics — ported Rust strategies vs SNES ROM

READ-ONLY audit. Scope: every `play_se(...)` in
`rust/sf-strat/src/{player,enemy_a,enemy_b,bosses,common}.rs` plus the sf-game
sound sites, cross-checked against the ASM (`reference/ultrastarfox/SF/`).

Authoritative sources:
- SE-id equates: `INC/SOUNDEQU.INC`
- Call macros: `INC/MACROS.INC:4201` (`trigse`), `INC/STRATMAC.INC:248` (`set_sound2`)
- Positional layer + selectors: `ASM/SOUND.ASM:735-945` (`*sound_l` → `makesnd`)
- POS_* id tables (already correct in port): `rust/sf-audio/src/sound.rs:91-141`

## Three ROM call TYPES

| ROM form | Semantics | Rust equivalent |
|---|---|---|
| `trigse #id` (`MACROS.INC:4201`) | one-shot, priority-gated `lda #id; jsl setport3_l` | `play_se(id)` (flat) — CORRECT |
| `jsl *sound_l` → `makesnd` (`SOUND.ASM:899`) | positional: picks L/C/R/mid/far band from object x/z distance | `make_snd(&st, &obj, ox, oz, &POS_*)` |
| `set_sound2 obj,#N` (`STRATMAC.INC:248`) | writes object's `alx_snd2` looping/engine-voice slot (N is a voice index 0-15, **not** an SE id) | (not an SE id — out of play_se scope) |

`set_sound2 x,#N` sites (walker/tank/aircar/truck engine loops, GASTRATS/GA2STRAT/
GSTRATS/DSTRATS) are the continuous-voice slot, not one-shot SE — excluded from the
play_se id check.

## ROM SE-id table (name → number → default TYPE)

From `SOUNDEQU.INC` (named) plus raw literals observed in STRAT/*.ASM (unnamed):

| id | name (SOUNDEQU) | notes / TYPE seen |
|---|---|---|
| $03 | se_playerdown | trigse |
| $04 | se_playerdamage | trigse |
| $05/$06 | se_wingdestruct l/r | trigse |
| $07/$08 | se_wingdamage l/r | trigse |
| $09/$0a | se_wingtouch l/r | trigse |
| $0b/$0c | se_warning1/2 | trigse |
| $0e | (item/ring) | trigse (item0) |
| $0f | se_gateofring | trigse (gate) |
| $10 | se_itemcatch | trigse (1up, gate3, item-max) — **also the placeholder-chime** |
| $11 | se_cursor | menu |
| $14 | (box/item) | trigse (pcbox, item) |
| $15/$17/$18 | (item pickups) | trigse |
| $1a | (bonus) | trigse — MAIN.ASM:1149 (score) |
| $1d | (big/boss explosion) | trigse — makebosscircexp_srou, delayexplode |
| $1e/$1f/$20 | se_destructboss near/mid/far | **makesnd** (destbosssound_l) / boss_dying uses $1e |
| $21/$22/$23 | se_destructenemy near/mid/far | **makesnd** (destenemysound_l) — port ranges these (known) |
| $24/$25/$26 | se_damageenemy near/mid/far | **makesnd** (damenemysound_l) — port ranges these (known) |
| $27/$28/$29 | se_hitwall near/mid/far | **makesnd** (hitwallsound_l) |
| $2b | (spawn/appear) | trigse (cameleon, uperm) |
| $2c/$2d/$2e | (boss B / paratrooper hits) | trigse |
| $2f | (boss1 cover) | trigse |
| $30 | se_specialweapon | trigse |
| $31 | se_abutton | trigse (player fire tap) |
| $32/$33 | se_speedup / se_speeddown | trigse (boost/brake) |
| $34/$35/$36 | (player fire variants) / $35 = se_laser | trigse |
| $39/$3a/$3b | (mother-boss whoosh/egg/fire) | trigse (DSTRATS) |
| $3c/$3d/$3e | se_missile near/mid/far | **makesnd** (missilesound_l) |
| $3f-$43 | se_movingwall l/c/r/mid/far | **makesnd** (movewallsound_l) |
| $44-$48 | se_laser l/c/r/mid/far | **makesnd** (lasersound_l) |
| $49/$4a/$4b | se_enemybattry near/mid/far | **makesnd** (enemybattrysound_l); also flat trigse $49 (pillar, misspod, separatemissile) |
| $4c/$4d | (boss B rob land/jump; boss7) | trigse |
| $52/$53 | door-close near/mid,far | **makesnd** (doorclosesound_l) |
| $54/$55 | door-open near/mid,far | **makesnd** (dooropensound_l); also flat trigse $54 (sdoor2) |
| $56/$57 | (misc enemy) | trigse |
| $58/$59/$5a | (boss7 / bossA / bossFC events) | trigse |
| $5b | (boss7 spawn) | trigse (GB3STRAT:3195) |
| $5c/$5d/$5e | ring-laser near/mid/far | **makesnd** (ringlasersound_l) |
| $66 | (bossA cup) | trigse |
| $68-$6c | enemy-up-sea l/c/r/mid/far | **makesnd** (enemyupsea_l); center = $69 |
| $6d/$6e/$6f | se_dop r/c/l | trigse (SOUND.ASM:534) |
| $70/$71 | (nucleus/beam on/off) | trigse |
| $72/$73 | (bossA up/cover, boss8) | trigse |
| $74-$78 | enemy-down-sea l/c/r/mid/far | **makesnd** (enemydownsea_l); center = $75 |
| $80/$81 | (boss B) | trigse |
| $82/$85 | (boss1 spawn / back) | trigse |
| $83 | (bossA spawn, boss8) | trigse |
| $86/$87/$88 | (boss7 blowcube) | trigse |
| $8b | (zacos) | trigse |
| $8d/$8e | (castanet clang/smash; boss land) | trigse |
| $95/$96 | (boss2 spawn / bossFC die) | trigse |
| $97/$98/$99 | (D3 boss events) | trigse |
| $9a | (walker2) | trigse |
| $9d/$9e | (bossG events) | trigse (D2STRATS) |
| $9f/$a0/$a1 | (boss7 final) | trigse |

## Per-strat verification table

Status: OK = id and TYPE both match the ROM strat; TYPE = wrong call type; EXTRA =
ROM plays no sound here.

| Rust strat (file:line) | ROM strat (ASM) | ROM expected | Rust actual | Status |
|---|---|---|---|---|
| shipintro_strat (player.rs:405) | boost | trigse $32 | play_se $32 | OK |
| playerdead_strat/istrat (player.rs:532,647) | PSTRATS:3054 | trigse se_playerdown $03 | play_se $03 | OK |
| pcbox_coll_strat (player.rs:794) | PSTRATS:123 | trigse $14 | play_se $14 | OK |
| boost_brake_update (player.rs:914-943) | PSTRATS:2163-2259 | trigse $32/$33 | $32/$33 | OK |
| playerfire_srou (player.rs:1312-1409) | PSTRATS:2894-2972 | trigse $31,$34,$35,$36 | $31,$36,$34,$35 | OK ($35 fixed from $60) |
| friendstart3go / openingboost (player.rs:1957,2387) | boost | trigse $32 | $32 | OK |
| strat_hit_flash range (enemy_a.rs:784) | damenemysound_l | $24/$25/$26 | ranged play_se | OK (known) |
| strat_explode range (enemy_a.rs:842) | destenemysound_l | $21/$22/$23 gated noexpsnd | ranged | OK (known) |
| pillar3stay_init (enemy_a.rs:953) | DSTRATS:823 pillar3stay_istrat | **trigse $49** (flat) | play_se $49 | OK |
| gate_heal (GATE_SOUND) (enemy_a.rs:1240) | DSTRATS:1772 | trigse se_gateofring $0f | $0f | OK |
| gate_heal (GATE3_SOUND) (enemy_a.rs:1200,1323) | GA2STRAT:2630 gate3_strat | trigse $10 | $10 | OK |
| boss1back_strat (enemy_a.rs:2316) | GBSTRATS:201 | trigse $85 | $85 | OK |
| boss1cov_strat (enemy_a.rs:2429) | GBSTRATS:434 | trigse $2f | $2f | OK |
| strat_boss1_init (enemy_a.rs:2637) | GBSTRATS:112 | trigse $82 | $82 | OK |
| **strat_tow0_explode (enemy_a.rs:2659)** | EXPSTRAT:1070 tow0explode→pillarexplode | **no sound** | play_se $10 | **EXTRA (F5)** |
| item5/7/0 (enemy_a.rs:3053,3214,3218,3633) | GASTRATS:2587,3121,2939,2688 | trigse $18,$17,$15,$0e | match | OK |
| up1manhit_istrat (enemy_a.rs:3741) | GASTRATS:2667/2754 | trigse $10 (1up) | $10 | OK |
| **zaco3die_init (enemy_a.rs:4174)** | KSTRATS:162 zaco3die_istrat | **no sound** (makeMEDexpobj_srou = rtl) | play_se $10 | **EXTRA (F6)** |
| **base1_strat door-open (enemy_a.rs:4763)** | KSTRATS:387 | **jsl dooropensound_l (makesnd, POS_DOOROPEN $54/$55/$55)** | flat play_se $54 | **TYPE (F1)** |
| **base1_wait door-close (enemy_a.rs:4790)** | KSTRATS:402 | **jsl doorclosesound_l (makesnd, POS_DOORCLOSE $52/$53/$53)** | flat play_se $52 | **TYPE (F2)** |
| strat_cameleon_init (enemy_a.rs:4882) | GASTRATS:1452 | trigse $2b | $2b | OK |
| boss_dying (enemy_a.rs:5860) | boss death chain | trigse $1e | $1e | OK (known) |
| circdelayexplode_strat (enemy_a.rs:5928) | EXPSTRAT:144 | trigse $1d | $1d | OK |
| boss7fall_init/strat (enemy_b.rs:776,792) | GB3STRAT boss7e | trigse $21,$4c | $21,$4c | OK |
| boss7hatch / launcher (enemy_b.rs:928-1041) | GB3STRAT boss7e_init | trigse $5a,$59 | $5a,$59 | OK |
| boss7 open/spawn (enemy_b.rs:1173,1652) | GB3STRAT boss7exp $58 / :3195 $5b | $58,$5b | $58,$5b | OK |
| bossA cup/up/cover (enemy_b.rs:2112,2286,2315,2335) | GB3STRAT boss8 | $66,$73,$72,$73 | match | OK |
| strat_bossa_init (enemy_b.rs:2619) | GB3STRAT boss8b | $83 | $83 | OK |
| bossFC family (enemy_b.rs:2979,3194,3269,3377,3495,3535) | GB2STRAT bossFC | $23,$8e,$58,$58,$96,$21 | match | OK |
| bossFB (enemy_b.rs:3916) | GB2STRAT bossFB | $58 | $58 | OK |
| boss2exp_init (bosses.rs:797) | GBSTRATS:703→EXPSTRAT bossbigoutexplode | trigse $1d (boss big-exp) | $1d | OK |
| boss2 spin/jump/land/spawn (bosses.rs:1162-1436) | GBSTRATS:524,572,609 | $95,$71,$85,$8e | match | OK |
| **sea_enemy_up_sea (bosses.rs:1558)** | D2/D3/GA2 enemyupsea_l | **jsl enemyupsea_l (makesnd, POS_ENEMYUPSEA $68-$6c)** | flat play_se $69 | **TYPE (F3)** |
| **sea_enemy_down_sea (bosses.rs:1562)** | D2/GA2 enemydownsea_l | **jsl enemydownsea_l (makesnd, POS_ENEMYDOWNSEA $74-$78)** | flat play_se $75 | **TYPE (F4)** |
| strat_bossg / bossg_strat (bosses.rs:2172-2375) | D2STRATS bossg | $9d,$9e,$5a,$59 | match | OK |
| bossg_generate_shadows (bosses.rs:2199) | D2STRATS | $2d | $2d | OK |
| boss8a/8b (bosses.rs:2976,3039) | GB3STRAT | $73,$72 | match | OK |
| nucleusbeam l/col (bosses.rs:3236-3292) | GB3STRAT:416-418 | $71 off,$70 on | $71,$70,$71 | OK |
| flingboss (bosses.rs FB_SE_*) | DSTRATS | $39 spin, $2e hit | match | OK |
| castanet (bosses.rs CAST_SE_*) | DSTRATS:6052/6060 | $8d clang, $8e smash | match | OK |
| chicken (bosses.rs CHICK_SE_*) | DSTRATS | $3b fire, $3a egg, $39 whoosh | match | OK |

sf-game layer (in scope, spot-checked OK): `game.rs:763` play_se($0b) warning;
`game.rs:553` blocksnd $49; `score.rs`/`planets.rs` bonus $1a (MAIN.ASM:1149);
`shell.rs:231` is the play_se sink; `common.rs:668`/`path_adapter.rs:501` are the
generic bytecode `trigse` pass-throughs.

## Numbered findings

### TYPE — flat play_se where ROM uses positional makesnd

**F1. base1 door-open — `enemy_a.rs:4763`**
ROM `base1_strat` (`KSTRATS.ASM:387`) does `jsl dooropensound_l` → `makesnd`
(POS_DOOROPEN = near $54 / mid,far $55). Port fires flat `play_se(0x54)`. The near id
is right, but the door should attenuate/pan by distance.
Fix: `make_snd(&state, &self_obj, ox, oz, &sf_audio::POS_DOOROPEN)`.

**F2. base1 door-close — `enemy_a.rs:4790`**
ROM `.close` (`KSTRATS.ASM:402`) does `jsl doorclosesound_l` → `makesnd`
(POS_DOORCLOSE = near $52 / mid,far $53). Port fires flat `play_se(0x52)`.
Fix: `make_snd(.., &sf_audio::POS_DOORCLOSE)`.

**F3. sea up — `bosses.rs:1558` (`sea_enemy_up_sea`)**
ROM sea necks (`D2STRATS.ASM:845`, `D3STRATS.ASM:1141`, `GASTRATS:2038/2132`) do
`jsl enemyupsea_l` → `makesnd` (POS_ENEMYUPSEA $68/$69/$6a/$6b/$6c, center $69). Port
fires flat `play_se(0x69)` (center only, no L/R/far bands).
Fix: `make_snd(.., &sf_audio::POS_ENEMYUPSEA)`.

**F4. sea down — `bosses.rs:1562` (`sea_enemy_down_sea`)**
ROM (`D2STRATS.ASM:822`, `GA2STRAT:3078-3147`) do `jsl enemydownsea_l` → `makesnd`
(POS_ENEMYDOWNSEA $74/$75/$76/$77/$78, center $75). Port fires flat `play_se(0x75)`.
Fix: `make_snd(.., &sf_audio::POS_ENEMYDOWNSEA)`.

### EXTRA — port plays a sound the ROM strat does not

**F5. strat_tow0_explode — `enemy_a.rs:2659`**
ROM `tow0explode_Istrat` (`EXPSTRAT.ASM:1070`) sets sword2=160 then `s_brl
pillarexplode_istrat`; that routine spawns exp children and plays **no** direct sound.
Port fires `play_se(0x10)` (item-catch chime). This is the identical leftover
placeholder that was removed from the sibling `pillar3explode_strat` (Audit A #36 — see
the comment at `enemy_a.rs:1000`). tow0explode shares the same `pillarexplode` tail, so
its `play_se(0x10)` is the same bug, uncaught.
Fix: delete `g.hooks.play_se(0x10);` at `enemy_a.rs:2659`.

**F6. zaco3die_init — `enemy_a.rs:4174`**
ROM `zaco3die_istrat` (`KSTRATS.ASM:162`) sets the exp-strat and calls
`makeMEDexpobj_srou`, which is `s_rtl` (empty — no sound). No `trigse` anywhere in the
zaco3die/zaco3DIE/zaco3GO chain. Port fires `play_se(0x10)` (item-catch chime) at init —
another leftover placeholder. The kamikaze's actual boom comes later via
`explode_istrat` (destructenemy positional), not at death-init.
Fix: delete `g.hooks.play_se(0x10);` at `enemy_a.rs:4174`.

### WRONG-ID
None found. Every sampled `play_se(0xNN)` id matches the `trigse`/selector id in its
ROM strat (including the previously-fixed player-laser $35 and the ranged
explode/hit-flash families).

### MISSING (positional families not yet wired — known follow-up)
The `make_snd` POS_* layer exists in `sf-audio/src/sound.rs` but still has **no callers**
in sf-strat. Beyond F1-F4 (which currently mis-fire as flat), the following ROM
positional selectors have no port coverage yet; the strats that own them should call
`make_snd` once wired:
- `lasersound_l` (POS_LASER $44-$48) — enemy laser fire, `GSTRATS.ASM:2559-2603`
- `missilesound_l` (POS_MISSILE $3c-$3e) — enemy missiles, `GSTRATS.ASM:2623-2763`
- `enemybattrysound_l` (POS_ENEMYBATTRY $49-$4b) — turret/battery fire, `GSTRATS.ASM:2417-2544`
- `hitwallsound_l` (POS_HITWALL $27-$29) — wall grazes, `GSTRATS.ASM:763,2399`
- `movewallsound_l` (POS_MOVEWALL $3f-$43) — moving walls, `DSTRATS.ASM:1033/1046`
- `ringlasersound_l` (POS_RINGLASER $5c-$5e) — ring lasers, `GSTRATS.ASM:2497`
- `separatemissile_l` (POS_SEPARATEMISSILE $49-$4b) — missile split, `SOUND.ASM:887`

## Strats that SHOULD call make_snd (feeds the makesnd-wiring lane)

Priority order (top four already mis-fire flat and just need the call swapped):
1. `base1_strat` door-open → `POS_DOOROPEN` — `enemy_a.rs:4763` (F1)
2. `base1_wait_strat` door-close → `POS_DOORCLOSE` — `enemy_a.rs:4790` (F2)
3. `sea_enemy_up_sea` → `POS_ENEMYUPSEA` — `bosses.rs:1558` (F3)
4. `sea_enemy_down_sea` → `POS_ENEMYDOWNSEA` — `bosses.rs:1562` (F4)
5. enemy laser-fire weapon routines → `POS_LASER`
6. enemy missile-fire routines → `POS_MISSILE`
7. enemy turret/battery-fire routines → `POS_ENEMYBATTRY`
8. wall-graze / hitwall strats → `POS_HITWALL`
9. moving-wall strats → `POS_MOVEWALL`
10. ring-laser strat → `POS_RINGLASER`
11. missile-separate → `POS_SEPARATEMISSILE`

(Note: `set_sound2 x,#N` engine-loop voices — walker/tank/aircar/truck — are a separate
looping-voice slot, not one-shot SE, and are out of scope for this play_se audit.)
