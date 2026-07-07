# Unported Bosses — Porting Plan (roadmap to boss parity)

Scope: every boss / major-enemy ISTRAT in `reference/ultrastarfox/SF/STRAT/`
cross-referenced against the Rust `sf-strat` crate
(`rust/sf-strat/src/{enemy_a,enemy_b,bosses}.rs` + `table.rs`). Verified from
scratch — the AUDIT_BOSS_TICKS2 bottom note was correct but incomplete; this
doc is the definitive list.

Method: enumerated `def_istrat`/`def_Istrat` in `ISTRATS.ASM` (index = `ci`
counter, one per entry; boss indices ≥69 verified to match the Rust `IS_*`
constants — IS_BOSS1=69, IS_BOSS8=84, IS_BOSSA=85, IS_BOSS7=99, IS_BOSS2=108,
IS_BOSSF=116, IS_BOSSG=144 all agree). Located each boss's Istrat label + span
in the `*STRATS.ASM` files, its map placement in `reference/.../MAPS/`, and
whether the corresponding Rust map (`rust/sf-map/src/levels/`) already spawns it.

---

## 1. Definitive ported-vs-unported table

### PORTED (11 boss/major-enemy strats)
| Boss | ISTRAT idx | Rust lane | ASM |
|---|---|---|---|
| boss1 (Corneria) | 69 | enemy_a | GBSTRATS.ASM:92 |
| boss2 (+top/turret/petal) | 108 | bosses | GBSTRATS.ASM:484 |
| boss7 (full phase machine) | 99 | enemy_b | GISTRATS.ASM boss7 chain |
| bossA (turrets/cups) | 85 | enemy_b | GB3STRAT.ASM:521 |
| bossF "King Joh" (core/halves/6 turrets) | 116 | enemy_b | GB2STRAT.ASM:48 |
| bossg (Attack Carrier + bossgs) | 144 | bosses | D2STRATS.ASM:54 |
| boss8 "Great Commander" (+nucleus beam/launcher/pillar/cov/switch/shrap) | 84 | bosses | GB3STRAT.ASM:42 |
| seamon | 81 | bosses | GASTRATS.ASM:2046 |
| bossseamon (+exp) | (addr 0x030005) | bosses | GA2STRAT.ASM:3056 |
| spacepilon (mother + pilons) | (addr 0x030004) | enemy_b | GA3STRAT.ASM:38 |
| title | (addr 0x050020) | enemy_b | — |

### UNPORTED
| Boss | ISTRAT idx | ASM label | Reachable? | Map ported & spawns it? | Effort |
|---|---|---|---|---|---|
| **bossB + bossBrob** (Andross, final boss) | 115 / 118 | GB3STRAT.ASM:1252 / :1921 | YES (Route 1 L5→L6, Venom) | YES — level1_5.rs:251 (STRAT_ADDR_BOSSB), level1_6/MAP1_6A stub (bossBrob) | **XL** |
| **bossh** ("gggy" legged Macbeth boss) | (D3STRAT only¹) | D3STRATS.ASM:67 (+legs :589, top :868) | YES (Route 1 L4, Macbeth) | Map ported but **mis-stubbed as IS_BOSS2** (level1_4.rs:312) | **XL** |
| **chicken** (Route 3 L3 boss) | 117 | DSTRATS.ASM:3696 | YES (Route 3 L3) | YES — level3_3.rs:299 (IS_CHICKEN) | **L** |
| **castanet** "Metal Smasher" | 124 | DSTRATS.ASM:5754 | YES (Route 2 L5) | YES — level2_5.rs:215 (IS_CASTANET) | **L** |
| **webmonster** (+web/drill/6 turrets) | 123 | DSTRATS.ASM:6504 (web :6800) | YES (Route 3 L2) | YES — level3_2.rs:234 (IS_WEBMONSTER) | **L** |
| **flingboss + deadflingboss** | 58 / 59 | DSTRATS.ASM:2951 / :3650 | YES (Route 2 L4, armsmap) | YES — level2_4.rs:158 (IS_FLINGBOSS) | **M** |
| **seadragon + seadragon2** | 197 / 198² | DSTRATS.ASM:1934 / :1931 | YES (Route 3 L3) | seadragon2 YES (level3_3.rs:175); seadragon via mother (STRAT_ADDR_SEADRAGON reserved, mothers.rs:236) | **L** |
| **madtrucker** (+trucklaunch :6260, fallingtruck :6313, madbiker :4961) | 120 / 119 | DSTRATS.ASM:5233 | YES (Route 2 L6, trucker) | YES — submaps.rs:312 (STRAT_ADDR_MADTRUCKER); madbiker spawned by trucker strat | **L** |
| **amoeba** (+amoebacol/stick) | 128 | GA2STRAT.ASM:126 | YES (Route 2 L4 + blackhole) | YES — spawned via mother (mothers.rs:206 AMOEBA, map_amoebas) | **M** |
| **cruiser1 / cruiser2** (+cruiser2fire/launcher) | 153 / 132 (fire 131) | GA2STRAT.ASM:629 / :373 | YES (Route 1 L3) | cruiser2 YES (level1_3.rs:463, STRAT_ADDR_CRUISER2); cruiser1 not yet placed | **M** |
| **lochnessmonster** | 198² | DSTRATS.ASM:1926 | UNCERTAIN — no map placement; only `s_set_strat` from D2STRATS.ASM:842 | no | **S** |
| ~~sokuten~~ | 56 | GASTRATS.ASM:2329 | **CUT** — `def_istrat` exists (flingboss shape) but **no map placement anywhere** | no | none |
| ~~boss5~~ | (no def_istrat³) | GBSTRATS.ASM:1051 | **CUT** — no `def_istrat`, no map placement (only COLBOXES) | no | none |

¹ bossh has no `def_istrat` row in `ISTRATS.ASM`; it is referenced only from the
Route-1 D3STRATS bank and the level1_4 map, which is why the Rust map currently
substitutes IS_BOSS2 as a placeholder proxy.
² seadragon/seadragon2/lochnessmonster share DSTRATS indices 197/198 region and
the `sproutstrat` segment mechanism.
³ `boss5_Istrat` is orphaned code (has collision boxes but is never placed) — cut
content from an early build.

**Count: 11 ported, ~11 unported real bosses/major-enemies (+2 cut).**
Of the unported: **all reachable except lochnessmonster (uncertain), sokuten (cut),
boss5 (cut)** — i.e. **~9–10 are reachable in normal play**, and their maps are
already ported in `sf-map` (spawning a nullshape/proxy or a wrong-strat stub today).

---

## 2. Per-boss dossier

### bossB + bossBrob — Andross (final boss) · XL · port LAST (capstone)
- **ASM**: GB3STRAT.ASM:1252 `bossB_Istrat` → ~:2650; 18 Istrat labels:
  bossB / bossBdodgecol :1404 / bossBscream :1557 / bossBspinend :1615 /
  bossBspinendcol :1706 / bossBescape :1728 / bossBentlong :1774 / bossBent :1787 /
  bossBrobdemo :1846 / bossBrob :1921 / bossBentsplit(2) :2041/:2075 / bossBrobchg :2178
  (+chg2 :2206, chg3 :2230) / bossBrobrndpos :2623.
- **Where**: Route 1 L5 (MAP1_5.ASM:… face form) → L6 Venom (MAP1_6A.ASM:292
  `bossBrob_Istrat` robot form) + FINALMAP tunnel. Both maps already ported
  (level1_5.rs:251, level1_6.rs) with `SH_BOSS_B_1_PROXY = nullshape` stubs.
- **Complexity**: two full forms (face + robot), phase changes (`robchg`/`robchg2/3`),
  a screen/model **split** phase (`bossBentsplit`), scream/dodge/spin/escape states,
  ~42 child/state/shape ops. The marquee boss; highest risk.
- **Deps**: child-linking (have it), boss-explode chain (have it), hitflash
  (`hitflashBOSSd_Istrat` GSTRATS.ASM:843 — dedicated hitflash variant, may need
  porting). The "split" phase needs verifying against renderer (model-halving) but
  no GSU/damyscr macro is used — should be plain transform math.
- **Effort: XL.** Do after the medium/large bosses so the shared infra is proven.

### bossh — "gggy" legged boss (Macbeth) · XL
- **ASM**: D3STRATS.ASM:67 `bossh_istrat`, 5 legs `bosshleg_istrat` :589
  (spawned :479-487 at deg-spaced offsets using the `#gggy` height constant — this
  is the "gggy" the audit note referenced), top `bosshtop_istrat` :868,
  teleporter :181. 12 Istrat labels, 7 `s_make_childobj`, ~1185 lines.
- **Where**: Route 1 L4 (MAP1_4.ASM:217 `boss_h_0`). Rust level1_4.rs:312 currently
  spawns **IS_BOSS2 as a wrong placeholder** — needs a real IS_BOSSH row + address.
- **Deps**: 5-child radial leg linking + a top part + a teleport move; child-linking
  infra exists (boss7/A pattern). Needs a new IS_BOSSH ISTRAT index/address wired.
- **Effort: XL** (child count + phase count rival Andross's first form).

### chicken — Route 3 L3 boss · L
- **ASM**: DSTRATS.ASM:3696 `chicken_istrat`, 10 Istrat labels, 24 state-ops,
  0 children, ~1263 lines. Self-contained single-body multi-phase boss
  (uses `bossexplode_istrat` for death, `hitflash_istrat`).
- **Where**: Route 3 L3, level3_3.rs:299 already spawns IS_CHICKEN + SH_BOSS_D_1.
- **Deps**: none new — homing/projectile fire + explode chain already ported.
- **Effort: L** (many states, but no child machinery). Good high-visibility win.

### castanet — "Metal Smasher" · L
- **ASM**: DSTRATS.ASM:5754 `castanet_istrat`, 6 labels, 13 state-ops, 0 children,
  ~750 lines. Ground-lane crushing boss.
- **Where**: Route 2 L5, level2_5.rs:215 spawns IS_CASTANET (nullshape proxy).
- **Deps**: none new (ground movement + explode). Self-contained.
- **Effort: L.**

### webmonster — Route 3 L2 spider · L
- **ASM**: DSTRATS.ASM:6504 `webmonster_istrat` + `web_istrat` :6800, drill_istrat,
  propturret_istrat; 6 Istrat labels, **8 `s_make_childobj`** (6 web turrets + fan +
  drill), 8 state-ops, ~822 lines.
- **Where**: Route 3 L2, level3_2.rs:234 spawns IS_WEBMONSTER + SH_BOSS_0_1.
- **Deps**: 6-turret + fan + drill child ring (child-linking infra exists); needs
  `propturret`/`drill`/`web` sub-strats ported too.
- **Effort: L** (child-heavy but mechanically regular).

### flingboss + deadflingboss · M · **best first win**
- **ASM**: DSTRATS.ASM:2951 `flingboss_istrat` (4 labels, 16 state-ops, 0 children,
  ~699 lines) + `deadflingboss_istrat` :3650 (2 labels, ~46 lines, death form).
- **Where**: Route 2 L4 armsmap; level2_4.rs:158 spawns IS_FLINGBOSS. deadflingboss
  is the post-kill body.
- **Deps**: none new — pure self state machine + explode chain.
- **Effort: M.** Lowest-risk reachable boss; ideal to port first.

### seadragon + seadragon2 · L
- **ASM**: DSTRATS.ASM:1934 `seadragon_istrat` / :1931 `seadragon2_istrat`
  (15 labels shared, ~1025 lines) using `sproutstrat` for the segmented snake body.
- **Where**: Route 3 L3; seadragon2 placed (level3_3.rs:175 IS_SEADRAGON2), seadragon
  via mother (mothers.rs:236, STRAT_ADDR_SEADRAGON reserved).
- **Deps**: segmented/worm-body linking — `worm`/`worm2` (d_body) already ported;
  extend that `sproutstrat` chaining for the dragon.
- **Effort: L** (shares body infra with the ported worm).

### madtrucker (+trucklaunch, fallingtruck, madbiker) · L
- **ASM**: DSTRATS.ASM:5233 `madtrucker_istrat` (3 labels, 7 states, ~521 lines);
  trucklaunch :6260 (launches trucks), fallingtruck :6313; madbiker :4961 spawned by
  the trucker strat (DSTRATS.ASM:4959 `s_beq madbiker_istrat`).
- **Where**: Route 2 L6 trucker map; submaps.rs:312 spawns STRAT_ADDR_MADTRUCKER
  (SH_BOSS_9_5_PROXY). Ground/road lane.
- **Deps**: ground-vehicle motion + child truck launching; shares nothing new beyond
  the ground lane and explode chain. Port madtrucker+madbiker+the two truck children
  as one unit.
- **Effort: L.**

### amoeba (+amoebacol, amoebastick) · M
- **ASM**: GA2STRAT.ASM:126 `amoeba_istrat`, 6 labels, 7 states, ~234 lines. A **swarm**
  enemy spawned in multiples (not a single HP-bar boss).
- **Where**: Route 2 L4 + blackhole; spawned via mother (`map_amoebas`, mothers.rs:206,
  count 250). `amoebastick` sticks to the player.
- **Deps**: a "stick-to-player" attach mechanic (new-ish); otherwise small.
- **Effort: M** (small code, but the mother-spawned swarm + stick behavior is novel).

### cruiser1 / cruiser2 (+cruiser2fire, cruiser2launcher) · M
- **ASM**: GA2STRAT.ASM:373 `cruiser2_istrat` / :629 `cruiser1_istrat` / :360
  cruiser2fire / :414 cruiser2launcher / :676 cruiser1fall; 11 labels, 3 children,
  ~316 lines. Large capital-ship mid-bosses.
- **Where**: Route 1 L3 (MAP1_3A2 cruiser2, MAP1_3C cruiser1). cruiser2 placed
  (level1_3.rs:463 STRAT_ADDR_CRUISER2); cruiser1 not yet placed in the Rust map.
- **Deps**: a few gun children (child-linking exists).
- **Effort: M.** Port cruiser1+cruiser2 together (shared code).

### lochnessmonster · S · uncertain reachability
- **ASM**: DSTRATS.ASM:1926 `lochnessmonster_istrat` (nullshape). No map placement;
  only reached via `s_set_strat y,lochnessmonster_istrat` (D2STRATS.ASM:842, in the
  bossg/underwater region). Likely a hidden helper/sub-behavior.
- **Effort: S**, but **confirm it is actually invoked** before spending time; may be
  vestigial. Low priority.

### CUT / unreachable (port priority = none)
- **sokuten** (idx 56, GASTRATS.ASM:2329) — has a `def_istrat` (flingboss shape) but
  **no map places it**. Cut variant.
- **boss5** (GBSTRATS.ASM:1051) — **no `def_istrat`, no placement** (only COLBOXES).
  Orphaned early-build content.

---

## 3. Recommended port order

Ranked by reachability × (inverse complexity), building shared infra before the
child-heavy and finale bosses:

1. **flingboss (+deadflingboss)** — M, reachable, self-contained. First win.
2. **castanet** — L, reachable, self-contained (no children).
3. **chicken** — L, reachable, self-contained; big Route-3 marquee.
4. **cruiser1 + cruiser2** — M, reachable; small gun-child set.
5. **amoeba** — M; mother-swarm + stick-to-player (build the stick primitive here).
6. **webmonster** — L; exercises the 6-turret + drill child ring.
7. **seadragon (+seadragon2)** — L; extend the ported worm segment chain.
8. **madtrucker (+madbiker/trucklaunch/fallingtruck)** — L; ground-vehicle lane.
9. **bossh** — XL; 5-leg + top radial child boss (fix the IS_BOSS2 mis-stub, add
   IS_BOSSH row).
10. **bossB + bossBrob (Andross)** — XL, capstone/finale. Port last, on top of all
    the proven child-link + explode + hitflash infra.

**Top 3 to port first: flingboss, castanet, chicken** — all reachable, all
self-contained (no child-linking), one per route (2/2/3), immediate parity gain
with the lowest risk.

Skip entirely: sokuten, boss5 (cut). Confirm-then-maybe: lochnessmonster.

---

## 4. Shared infrastructure to build once

Most is already in the port (proven by boss7/bossA/boss8); flagged here so it is
reused, not re-implemented per boss:

- **Boss-explode chain** — `bossexplode_istrat` / `bossbigoutexplode_istrat` /
  `Bossdelayexplode_istrat` (EXPSTRAT.ASM:46/78/162). Ported (enemy_b
  `bossbigoutexplode_istrat`, enemy_a EXPSTRAT block). Every unported boss's death
  routes here — reuse verbatim.
- **hitflash** — `hitflash_istrat` ported; **but bossB needs `hitflashBOSSd_Istrat`
  (GSTRATS.ASM:843)**, a dedicated variant not yet ported — build it in the bossB pass
  (bossh may want it too).
- **`add_bosshp` HP-bar accumulator** — ported (enemy_b:323 / bosses:119). Wire every
  new boss's per-tick `s_add_bossHP`.
- **Child-linking primitive** — `s_make_childobj` / `s_make_childobjrotpos`
  (boss7/bossA/boss8 pattern in enemy_b/bosses). Reused by bossh (7), webmonster (8),
  cruiser (3), bossBrob. No new engine op needed.
- **Segmented-body chain (`sproutstrat`)** — worm/worm2 (d_body) already port this;
  seadragon extends it. Build the reusable segment-follow once here.
- **New primitives that several want**:
  - **stick-to-player attach** (`amoebastick`) — first needed by amoeba; keep it
    generic.
  - **ground-vehicle motion** (castanet, madtrucker/madbiker, trucklaunch/fallingtruck)
    — share a road/ground movement base with the ported truck/truck1/truck2.
  - **IS_BOSSH / IS_BOSSB ISTRAT rows + synthetic addresses** — bossh and bossB need
    real registry rows wired in `table.rs`/`bosses.rs` (today their maps stub
    IS_BOSS2 / nullshape).

No unported boss uses a GSU/`damyscr`/`splitscr` routine (verified: 0 GSU-ish macros
across all their spans) — the renderer/GSU layer needs no new features for boss parity.
</content>
</invoke>
