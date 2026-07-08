# Function coverage ledger — ROM → Rust (path to 100%)

Ground-truth target (user decision 2026-07-07): **retail cart** `Star Fox (USA) (Rev 2).sfc`.
Machine-readable data: `docs/function_ledger.tsv` (label, addr, name-match, subsystem).

## CRITICAL FINDING: retail ROM ≠ symbol-mapped built ROM

- `Star Fox (USA) (Rev 2).sfc` (retail) = **1 MB**.
- `rust/sf-oracle/data/sf.sfc` (ultrastarfox source-built, what `symbols.txt` addresses) = **2 MB**.
- They are **different binaries**: ~50% raw byte match, and **every 32 KB bank differs**. The built
  ROM is a source *reconstruction*, reassembled at a different layout — not a retail byte-repro. At
  least one *behavioral* delta is already known (the `base1` door — flagged "ultrastarfox hack may
  differ from ROM").

**Implication for the oracle strategy:**
- Our entire symbol map + `sf-oracle` `call(addr, …)` infra is keyed to the **built** ROM. Per-function
  differential testing (tier-1) therefore runs against the **built** ROM. For the *pure* math/logic
  layer this is equivalent to retail (reassembly doesn't change arithmetic) — that is where the
  mulslog / gen_vecs / speedto / rotmat proofs already live and are trustworthy.
- To make **retail** the ground truth we need either (a) a retail address map (we don't have one; it's
  a reverse-engineering effort), or (b) **tier-2 whole-game co-execution** which boots the *retail* ROM
  and diffs *observable game state* (object array, spawns, events, draw list, SE triggers) tick-for-tick
  — this needs no symbol correspondence, only the retail object-array base + a few struct offsets
  (a small, bounded RE task) to read the observables. Tier-2 is the better fit for "retail as truth".

## Symbol classification (13,623 total)

| class | count | notes |
|---|---|---|
| ROM-resident (`addr&0xFFFF ≥ $8000`) | 8,147 | code + ROM data tables |
| WRAM vars (`bank 00/7E/7F, off < $2000`) | 4,423 | the `sv::`/`wm::` variable space |
| MMIO / low-RAM | 744 | |
| other | 309 | |

Of the ROM-resident labels, **2,169 are "function-like"** by naming heuristic
(`*_STRAT/_ISTRAT/_SROU/_INIT/_CONT/_L`, `FIRE*`, `EXPLODE*`, `CHASE*`, `MAKE*`, …).

## Coverage estimate (HONEST caveats)

**923 / 2,169 (≈43%)** function-like labels have a name/citation match in the Rust source.
**This 43% is a rough FLOOR, not "43% done":**
- It *undercounts* real coverage: many ROM labels collapse to one Rust fn (`SR8_ACHASE_ALVAR1..7`
  → one `chase_proportional`; `ADDVECS*/GEN*` → `strat_gen_vecs_3d`), and render/DMA/audio labels
  (`DMABG2VOFFSETS`, `DMA_SPRITES`, sound drivers) are covered structurally-differently by the wgpu
  renderer + sf-spc, so they never name-match.
- It also contains *false-unported* sub-labels of ported bosses (`BOSS7COLL`, `BOSS1MAKECHILD`,
  `CASTBIT.HIT`, `ZACO0STRAT`) that roll up to functions we ported.
- A *true* 1:1 ledger needs per-label verification (does a Rust fn reproduce this label's behavior?),
  which tier-2 coverage + targeted tier-1 diffs will fill in. This first pass is the roadmap, not the
  certificate.

Unported-by-subsystem (name-heuristic): enemy/boss strat **831**, other 209, math-helper 66,
render/DMA 59, player/cam 50, audio 25, map/menu 6. The render/audio/math/player buckets are
largely structural-difference false-unporteds; **the enemy/boss-strat bucket is the real gameplay gap.**

## Actionable roadmap — real unported gameplay strats

Beyond the bosses already ported this session (boss1/2/7/8/A/F/g, seamon, flingboss, castanet,
chicken, seadragon, webmonster, madtrucker = **17/~22 bosses**), the ledger surfaces a **long tail of
unported regular enemies + mini-bosses** not previously tracked. Deduped top-level families:

**Capstone bosses (XL):**
- `BOSSBROB*` / `BOSSB*` — Andross + the Andross **robot** (≈80 sub-labels: jump/kick/pounce/split/
  scream/spin/foot/dodge/reappear…). The largest single remaining port.
- `BOSSH*` — "gggy" legged Macbeth boss (leg/top/hitcount).

**Reachable mini-bosses / set-pieces (M–L):** amoeba (`AMOEBASTICK/HOME/COL`), cruiser1/2
(`CRUISER1FALL/CRUISER2LAUNCHER`), `CORE`/`MCORE` (base cores), `CRANE`, `MONOLITH`, `SOKUTEN`.

**Regular enemies — the volume (S–M each):** `AIRCAR`, `BEE1`, `CAMELEON2`/`CAM2DASH`/`CAM2HIDE`,
`CRAB`, `DRAGONFLY`/`SDRAGONFLY`, `DUCT`, `EVADER`, `FASTFIGHTER`, `FZACO`, `TZACO7`, `SZACO`,
`HELPBALL`, `HOVER`, `IRONBALL`, `KAMI`, `WALKER`/`LWALKER`/`RWALKER`, `WIREMAN`, `WINGLAZERMAN`,
`SAUCER` (11 labels), `SHARK`, `SFISH`, `RIPMAN`, `STARBULL`, `TANK`/`TANK1`/`TANK2`, `TELEPORTER`,
`TORPEDO`, `TREE`/`WOODS` (scenery-collide), `VOLROCK`/`VOLPLASM`, `POLE`, `MISSPOD`/`MISSILE2`,
`SPARKY`, `STBHMISSILE`/`QHMISSILE` (projectiles).

**Cutscene / flow (verify reachability first):** `THEEND_*` (ending), `HYPERSPACE*`, `PLAYERWARP*`,
`BLACKHOLEEXIT` (← unblocks the route-warp *arming*, Route Finding 2), many `PLAYER*FLYIN`/clear-demo
transitions (several already ported as submaps — need per-label check).

## Recommended sequencing

1. **This ledger** (done) — defines the denominator + roadmap.
2. **Tier-1 pure-function fuzz sweep** (built ROM): batch-expand the mulslog pattern over the ~66
   math-helper + other pure labels. Cheap, parallel, drains the latent-bug class. Trustworthy vs retail
   for pure logic.
3. **Tier-2 retail co-execution**: locate the retail object-array base + struct offsets, boot the retail
   ROM in sf-oracle, run scripted inputs per level/route, diff observable state each tick vs the Rust
   port. First divergence = exact function+field. This is what certifies behavioral parity vs retail.
4. **Port the unported enemy long tail + the BOSSBROB/BOSSH capstones**, using tier-2 to drive coverage
   and catch divergences. Each is a self-contained strat port (no new engine features needed — confirmed).
5. **Refine the ledger** per-label as tier-2 exercises code paths, converting name-match → verified.
