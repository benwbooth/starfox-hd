# Reachable unported strats — prioritized porting queue

**Status (2026-07-09):** All 54 `IS_*` names listed below are now **mentioned and
registered** in `sf-strat` (enemies_ground / bosses / table). This file is kept
as historical priority notes; re-scan before treating any row as a gap.

Generated originally by scanning sf-map for `IS_*` placements vs what sf-strat
registers. Verify each against the ROM before porting; a few may be map-side or
alias an already-registered strat under a different name.

## Route-warp arming (DONE — bosses.rs)
`IS_BHOLEEXIT1/2/3`, `IS_BLACKHOLE` — ported + istrats wired. Also
`IS_COLONYEXIT`, `IS_IRIS` in enemies_ground.

## Ground/tank enemies (DONE — enemies_ground.rs)
`IS_TANK1A`, `IS_TANK2`, `IS_TANK3`, `IS_WALKING`, `IS_WALLL`, `IS_WALLR`,
`IS_WALLLEFTRIGHT`, `IS_WIREMAN`, `IS_WINGLAZERMAN`, `IS_BAZOOKAL`,
`IS_BAZOOKAR`, `IS_ROCKHARD`, `IS_UPERM`, `IS_TRUCK`.

## Base / structure set-pieces (DONE — enemies_ground / enemy_a)
`IS_BASE0`, `IS_BASE1`, `IS_MASSIVEBASE`, `IS_COLONY0/1/2`, `IS_MISSPOD`,
`IS_MISSTANK`, `IS_SYNTH`, `IS_KDOOR`, `IS_KDOOR2`, `IS_SHOU0`, `IS_SHOU0A`,
`IS_KICHI2`.

## Projectiles / hazards (DONE)
`IS_TORPEDO`, `IS_MINE0`, `IS_FIREPILLAR`, `IS_FLYPILLARS`, `IS_HOUDAI5F`,
`IS_SZACO0`, `IS_SZACO5`, `IS_METEO0`, `IS_BIG_METEOR`, `IS_BREAK_METEOR`,
`IS_BREAK_METEORT`, `IS_VOLCANO`, `IS_WINDMILL`, `IS_TRACKCORNER`.

## Scenery collide (DONE / cosmetic)
`IS_TREE1`, `IS_TREE2`, `IS_WOODS`, `IS_ITEM6`.

## Clear-demo / cutscene ships (DONE — table.rs)
`IS_CLSHIPSHIPA/B/C`, `IS_CLSHIPUNDERA/B/C`, `IS_CLSHIPDIVEA/B/C`,
`IS_CLSHIPTURNA/B/C`, `IS_CLSHIPBRIDGEA/B/C`.

## Already-handled-elsewhere
`IS_PLAYER_DEAD` (player.rs).

## Capstone bosses
`BOSSBROB*`/`BOSSB*` (Andross), `BOSSH*` (gggy) — ported in bosses/bossb/bossh;
ledger may still list sub-labels as False.

## Next gaps (not this IS_ list)
- ~~Float-above-ground~~ — cam pitch/yaw/roll = ROM `outv*` (ticks 120–121).
- ~~Ledger False rows~~ — 2169/2169 True (tick 119).
- Tier-2 retail coexec expansion.
