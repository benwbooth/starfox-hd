# WIP snapshot classification — 2026-08-23

This branch preserves the complete dirty worktree found after the OpenCode run,
before the strict SF1 conformance-pipeline work began. Its baseline is commit
`d4960d9` on `master`.

## Pre-existing OpenCode experiments

- `rust/sf-game/src/shell.rs` — exposes the typed camera for oracle diagnosis.
- `rust/sf-oracle/tests/semantic_trace.rs` — removes the published retail-state
  substitution window and adds temporary corridor-banking diagnostics.
- `rust/sf-strat/src/player.rs` — tests continued player flight while the source
  dying-mode camera flag is active.

These changes are evidence-bearing experiments, not accepted production fixes.

## Controller launch fix added during inspection

- `rust/sf-app/src/input.rs`
- `rust/sf-app/src/main.rs`

This prevents a Steam Controller Start press from also closing the game when
Steam Input synthesizes Escape. Focused `sf-app` input tests and a live SF1
launch passed before this snapshot.

## Mechanical formatter noise

Running workspace formatting while the experiments were present reformatted
the remaining modified files. No behavior change was intended in these files:

- `rust/sf-game/src/game.rs`
- `rust/sf-game/src/vars.rs`
- `rust/sf-map/src/levels/route1/level1_4.rs`
- `rust/sf-map/src/levels/route2/level2_3.rs`
- `rust/sf-map/src/levels/route2/training.rs`
- `rust/sf-map/src/levels/route3/level3_5.rs`
- `rust/sf-oracle/src/retail.rs`
- `rust/sf-path/src/interp.rs`
- `rust/sf-strat/tests/enemies_ground.rs`
- `rust/sf-strat/tests/enemy_a_mediums_34_37.rs`
- `rust/sf-strat/tests/enemy_b_minors.rs`
- `rust/sf-strat/tests/intro_strategy_fidelity.rs`

Nothing in this snapshot should be merged wholesale. Recover individual changes
only after a strict retail/native scenario proves them.
