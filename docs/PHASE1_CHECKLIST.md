# Phase 1 Checklist

Phase 1 is the literal ASM->C port.

Rules:
- gameplay/content source-of-truth must be C, not generated Python output
- rewrite 16-bit memory model quirks into flat-memory C only where required
- preserve ASM file structure, section boundaries, macro intent, and table order
- one queue row = one bounded ASM file/section
- shared runtime fixes are allowed only when a real slice needs them

## Snapshot

- Maps: `MAP_ID_1_1`, `MAP_ID_1_2`, `MAP_ID_2_1`, and `MAP_ID_3_1` are live in [levels.c](/home/ben/src/starfox-hd/src/map/levels.c).
- Paths: 54 literal path ids are active in [path_literals.h](/home/ben/src/starfox-hd/src/path/path_literals.h).
- Strategies: 57 explicit registrations are present in [strat_table.c](/home/ben/src/starfox-hd/src/strat/strat_table.c); the active subset is aligned to `ISTRATS.ASM`.
- Shapes: runtime still uses a small built-in mesh registry in [shapes.c](/home/ben/src/starfox-hd/src/renderer/shapes.c); there is no general `def_shape` loader yet.

## Immediate Front Of Queue

- [x] Port the Sector X opening path ids used by `MAP2_2.ASM`: `check`, `egu6`, `chase2_1`, `chase2_2`.
  - Queue row: `sf-path-pathdata-sectorx-opening` (`done`)
- [x] Port the opening `MAPMACS.INC` space-bar macro/runtime subset needed by `MAP2_2.ASM` through the first reward block.
  - Queue row: `sf-map-spacebar-runtime-core` (`done`)
- [x] Port `shipintro_Istrat`, which `MAP1_1A` still depends on.
  - Queue row: `sf-istrat-gistrats-shipintro` (`done`)
- [x] Port the `LEVEL1_3.ASM` opening wrapper slice plus `cl_warpout` and `map1_3a`.
  - Queue row: `sf-map-level1_3` (`done`)
- [x] Port the next Sector X middle path ids used by later `MAP2_2` / `MAP3_2` slices: `chase3_1`, `chase3_2`, `e_shieldr`.
  - Queue row: `sf-path-pathdata-sectorx-middle` (`done`)
- [x] Port the `egu6` friend variants used by later Sector X and route-3 space slices.
  - Queue row: `sf-path-pathdata-sectorx-egu-friend` (`done`)
- [x] Port `e_flower` and `e_flopen`, including the bounded `P_POLLEN` bridge they need.
  - Queue row: `sf-path-pathdata-e_flower` (`done`)
- [x] Add bounded raw/16-bit shape-word resolution for the live literal slices.
  - Queue row: `sf-runtime-shape-word-resolution` (`done`)
- [x] Port `spacebarwalker_Istrat`, which the `MAP2_2` opening already uses.
  - Queue row: `sf-istrat-ga2strat-spacebarwalker` (`done`)
- [x] Port the live 1-up core: `item0_Istrat` through `up1manchild_strat`.
  - Queue row: `sf-istrat-gastrats-up1man` (`done`)
- [x] Unify active runtime shape ids with `def_shape` order where live slices still require proxies.
  - Queue row: `sf-runtime-defshape-alignment` (`done`)
- [x] Port the `MAP2_2` opening through the first `up1man` reward block.
  - Queue row: `sf-map-map2_2-opening` (`done`)
- [x] Port `spacebarshoot_Istrat` for the later shooter space-bar macros.
  - Queue row: `sf-istrat-ga2strat-spacebarshoot` (`done`)
- [x] Port the middle `spacebar` macro/runtime subset needed before the first `chase3_1` block in `MAP2_2`.
  - Queue row: `sf-map-spacebar-runtime-extended` (`done`)
- [x] Port the bounded boss7 shape-frame animation subset used by the live boss7 fight.
  - Queue row: `sf-renderer-boss7-shape-anim` (`done`)
- [x] Port `itachi_a`, which the first bounded `MAP3_2` slice already needs.
  - Queue row: `sf-path-pathdata-itachi-a` (`done`)
- [x] Port the `MAP2_2` middle slice through the first `chase3_1` / `e_shieldr` block.
  - Queue row: `sf-map-map2_2-middle` (`done`)
- [ ] Port `szaco2_Istrat` for the first bounded `MAP3_2` opening slice.
  - Queue row: `sf-istrat-ga2strat-szaco2` (`in_progress`)
- [ ] Port `bird_meteor` and `itachi_b` for the later `MAP3_2` opening slices.
  - Queue row: `sf-path-pathdata-itachi-b-bird` (`in_progress`)
- [ ] Port the broader generic `colanim` / `color_table` subset after the boss7 frame blocker.
  - Queue row: `sf-renderer-colanim-minimal` (`in_progress`)
- [ ] Port the tail-only static `spacebar` pattern used after the first `chase3_1` block.
  - Queue row: `sf-map-spacebar-runtime-tail` (`in_progress`)
- [ ] Keep the next ready fallback rows queued behind the active workers:
  - `LEVEL1_3` ship1 first bounded slice
  - first bounded `MAP3_2` opening slice

## Shared Map Helpers

- [x] `MAP1_1A.ASM`
  - Shared scramble submap used by `LEVEL1_1`, `LEVEL2_1`, and `LEVEL3_1`.
- [x] `CL_GND.ASM`
  - Shared clear-demo map used after route-1 ground stages.
- [x] `CL_WARP.ASM`
  - Shared clear-demo map used by `LEVEL1_2`.
- [x] `CL_EARTH.ASM`
  - Shared clear-demo map used by `LEVEL2_2`.
- [x] `CL_CHASE.ASM`
  - Shared clear-demo map used by `LEVEL3_2`.
- [ ] `CL_SHIP.ASM`, `CL_UNDER.ASM`, `CL_DIVE.ASM`
  - Keep these behind the first real map that uses them.

## Route 1

- [x] `LEVEL1_1.ASM:L17-L67`
  - Post-scramble opening through `mapjsr map1_1b`.
- [x] `1-1.ASM:L7-L236`
  - Main `MAP1_1B` include body.
- [x] `MAP1_1B.ASM:fadeoutbgm -> maprts`
  - Boss block and `mapwaitboss` bridge.
- [x] `LEVEL1_1.ASM:L69-L74`
  - Tail after `map1_1b`: delayed BU spawns, `cl_ground`, `mapend`.
- [x] `LEVEL1_2.ASM`
  - Wrapper around `MAP1_2` and `cl_warp`.
- [x] `MAP1_2.ASM:L7-L42`
  - Opening through first mini-worm slice.
- [x] `PATHDATA.ASM:chase1_1 -> chase1_2`
  - Route 2 friend-path pair used by `MAP1_2.ASM:L46-L109`.
- [x] `PATHDATA.ASM:pyonta`
  - Route 2 walker path used by `MAP1_2.ASM:L46-L109`.
- [x] `PATHDATA.ASM:chase4_1 -> chase4_3`
  - Route 2 friend-path trio used by `MAP1_2.ASM:L46-L109`.
- [x] `MAP1_2.ASM:L46-L109`
  - Cameleon/item/friend block.
- [x] `MAP1_2.ASM:L110-L157`
  - Worm/tadpole/black-hole setup.
- [x] `MAP1_2.ASM:map12boss -> maprts`
  - Boss block.
- [ ] `LEVEL1_3.ASM`
  - Wrapper plus `map1_3a*`, `map1_3b*`, `map1_3c`, `map1_3d`, `washent`.
- [ ] `LEVEL1_4.ASM`
- [ ] `LEVEL1_5.ASM` plus `MAP1_5.ASM`
- [ ] `LEVEL1_6.ASM`

## Route 2

- [x] `LEVEL2_1.ASM`
  - Wrapper through `mapjsr map2_1b`.
- [x] `2-1.ASM:2-1-1 -> opening 2-1-3`
  - Current live C slice stops before the first `zaco0` formation block.
- [x] `2-1.ASM:remaining 2-1-3 tail`
  - Route-2 ground body is live through EOF before `MAP2_1B.ASM` boss logic.
- [x] `MAP2_1B.ASM:fadeoutbgm -> maprts`
  - Boss7 reuse block.
- [ ] `LEVEL2_2.ASM` plus `MAP2_2.ASM`
- [ ] `MAP2_3A.ASM`, `MAP2_3B.ASM`, `MAP2_3C.ASM`
- [ ] `MAP2_4.ASM`
- [ ] `MAP2_5.ASM`
- [ ] `LEVEL2_6.ASM`

## Route 3

- [x] `LEVEL3_1.ASM`
  - Wrapper through `mapjsr map3_1b`.
- [x] `3-1.ASM:3-1-1 -> opening 3-1-2 friend block`
  - Current live C slice stops before the first `base_1` section.
- [x] `3-1.ASM:remaining 3-1-2 base/city slices`
  - Live C slice now reaches the first `CITY1` gate block and stops before `;3-1-3`.
- [x] `3-1.ASM:carrier_robot -> EOF`
  - Route-3 opening ground body is live through EOF before `MAP3_1B.ASM`.
- [x] `MAP3_1B.ASM:fadeoutbgm -> maprts`
  - BossA block.
- [ ] `LEVEL3_2.ASM` plus `MAP3_2.ASM`
- [ ] `MAP3_3A.ASM`, `MAP3_3B.ASM`
- [ ] `MAP3_4A.ASM`, `MAP3_4B.ASM`, `MAP3_4C.ASM`, `3-4-T.ASM`
- [ ] `MAP3_5.ASM`
- [ ] `MAP3_6.ASM`
- [ ] `MAP3_7A.ASM`, `MAP3_7B.ASM`, `MAP3_7C.ASM`

## PATHDATA.ASM

- [x] Current live route-1 coverage:
  - `e_gate`, `ponpon`, `matemsg`, `frog1_1`, `falco_lv1`, `frog_lv1`
  - `korori`, `chase6_1`, `chase6_2`, `chase8_1`, `chase8_2`, `chase8_3`
  - `patrol`, `e_ufo`, `e_rabbit`, `e_frog`, `e_falcon`
- [x] Current route-2 / route-3 support already live:
  - `astemsg`, `mes_message`, `chase7_1`, `chase7_2`
  - `chase1_1`, `chase1_2`, `chase4_1`, `chase4_2`, `chase4_3`
  - `pyonta`, `e_aste`, `e_aste_b`, `e_breaste`, `insekikun`
  - `patret_irab`, `patret_ifro`, `patret_ifal`, `falcon3_1`
- [x] Fidelity pass for already-ported slices
  - `P_MSGWITHMETER` support where macros use `meter`
  - `ponpon` weapon literal fix (`relbeamball`)
- [x] `astemsg -> mes_message`
  - Mostly message-only space support.
- [x] `chase7_1 -> chase7_2`
  - Route-2/route-3 chase pair.
- [x] `patret_irab -> inter_sub`
  - Patrol-family helper block.
- [ ] `e_flower -> e_flopen`
  - Small early slice, but it introduces macro-only pollen behavior.

## DPATHDAT.ASM

- [x] Current live route-1 coverage:
  - `tow_0`, `tow_1`, `dsmoke*`, `robot`, `robexplode`
  - `robotwithlog`, `robotwithlog2`, `robotswithlog`, `carriedlog`, `dummy`
  - `premove`, `pspiralexplode`
- [x] Fidelity pass for already-ported slices
  - `tow_0` spawn argument drift
  - `robexplode` inline flag setup
- [x] `my_bird -> ring`
  - Low-risk space-support row is live in C.
- [ ] Defer dense cutscene-heavy inline-65816 regions until generic callback support is improved.

## Strategies

- [x] Current live subsets:
  - `GSTRATS`: hard/static basics
  - `GISTRATS`: `friendexitbase`
  - `GA2STRAT`: `gate2`, `gate3`, `tadpole`
  - `GASTRATS`: route-1/2/3 subset (`rader*`, `zaco1*`, `zacos`, `tower0`, `zaco4`, `bomwing`, `item5`, `item7`, `worm*`, `houdai*`)
  - `GCSTRATS`: active `clshipGND*` / `clshipWARP*` clear-demo subset
  - `KSTRATS`: `zaco0`, `zaco3`, `carrier`
  - `DSTRATS`: `cameleon`
  - `D2STRATS`: `para`
  - `D3STRATS`: `base_1`
  - `GBSTRATS`: `boss1`
  - `GB3STRAT`: bounded `boss7`, `bossA`
- [x] `ISTRATS.ASM` index audit
  - Active hand-written constants are aligned before further reuse work.
- [x] `GASTRATS.ASM:houdaiNS_Istrat -> houdai_strat`
  - Route-2/route-3 gun emplacements.
- [x] `KSTRATS.ASM:zaco3_Istrat`
  - `zaco3` no longer collides with later rows.
- [x] `DSTRATS.ASM:cameleon_istrat`
- [x] `GASTRATS.ASM:wormhead_Istrat -> item7_strat`
- [x] `GA2STRAT.ASM:tadpole_Istrat -> gate2_strat`
- [x] `GBSTRATS.ASM:boss1_Istrat`
- [x] `GB3STRAT.ASM:bossA_Istrat`
- [x] `GISTRATS.ASM:shipintro_Istrat`
  - Needed by `MAP1_1A`.

## Shapes / Runtime

- [x] Unify runtime shape ids with `def_shape` order.
- [x] Add 16-bit shape-address resolution for map opcodes that still pass raw shape words.
  - Queue row: `sf-runtime-shape-word-resolution` (`done`)
- [x] Unify active runtime shape ids with `def_shape` order.
  - Queue row: `sf-runtime-defshape-alignment` (`done`)
- [x] Replace temporary boss7 path/strategy sentinels and proxies where the live boss7 slice needed them.
- [x] Unify active runtime shape ids with `def_shape` order.
  - Queue row: `sf-runtime-defshape-alignment` (`done`)
- [x] Port the extended `spacebar` macro/runtime subset used after the `MAP2_2` opening.
  - Queue row: `sf-map-spacebar-runtime-extended` (`done`)
- [x] Register real boss7 part meshes from `SHAPES2.ASM`.
- [ ] Add minimal phase-1 shape coverage only when a live slice needs it.
- [ ] Teach renderer to use color tables / explosion-driven shape changes as soon as a live slice depends on them.

## Execution Model

- The executable queue is [automation/port_queue.tsv](/home/ben/src/starfox-hd/automation/port_queue.tsv).
- Use `scripts/port_queue.sh ready` to see which slice is actually runnable now.
- Use `scripts/port_loop.sh` to run one slice at a time.
- Use `scripts/port_workers.sh` to run one worker per lane until the queue drains.
- If a run aborts, use `scripts/port_queue.sh requeue-in-progress` before restarting the loop.
