# Retail fidelity audit — 2026-09-04

Status: the port is not certified. This is a review of the verification
architecture and selected source paths, not a claim to have reviewed every
Rust function. The removed C implementation is historical evidence; the pinned
retail ROM is the final behavioral authority.

## Findings that change the finishing strategy

| Priority | Finding | Evidence and consequence |
| --- | --- | --- |
| P0 | Runtime timing replays one neutral-input recording | `rust/sf-game/src/gameplay_timing.rs` indexes two 983-element arrays by Corneria frame number, independently of input, objects, or render work, then falls back to four refreshes. Recorded timing is useful oracle evidence but cannot establish correct timing for different play. Replace this with source-derived state/workload timing and test different input tapes. Keep actual retail elapsed time as an explicit input in isolated routine tests, not as copied gameplay state. |
| P0 | Independent semantic verification was outside the advertised gate | `scripts/verify_retail_parity.sh` previously omitted Mesen. It now invokes it and the comparator tests before the Rust suite. A failure is retained, not interpreted as a completed release. |
| P0 | Similar byte windows were accepted as path correspondence | `verify_corneria_semantic_oracle.py` previously guessed a nearby path offset. The verifier now rejects differing catalogs without a verified mapping; byte similarity remains a diagnostic suggestion only. The required replacement maps exact source instructions and operands to native path instructions, with ROM/catalog hashes and boundary checks. |
| P0 | Restart capture mixed object-pool epochs | At scene 944, the retained Mesen artifact declares six active objects but emits no object records: its draw-list assertion refers to the preceding scene's objects. The native trace emits the rebuilt pool. The verifier now checks each capture's inventory independently and fails on this missing evidence. Capture simulation and completed drawing at distinct boundaries with explicit object generations. |
| P1 | All-route completion is an assisted soak | `rust/sf-app/tests/full_route_sim.rs` restores durability, suppresses death, and applies synthetic damage. Preserve it for progression testing, but require controller-only retail/native route replays for combat, death, checkpoints, and endings. |
| P1 | Shapes, pixels, and sound are not jointly certified | The Mesen semantic comparator still excludes source/native shape encodings and the raw pointer; it does not compare ordered audio events, PCM, or native raster output. Those are explicitly outside its current claim. Add exact asset/strategy identity mappings and independent presentation channels. |
| P1 | Byte-identical rebuild is not full reconstruction | The roundtrip manifests currently assemble 24 SF1 bytes and zero SF2 bytes; the remaining bytes come from the input ROM. Keep this as an integrity check and report promoted source bytes separately from behavior coverage. |
| P1 | Architecture checks do not enforce all source-style rules | `tools/check_native_architecture.py` passes in the reviewed tree, but does not enforce named constants/enums or decimal notation across the port. The native build graph and feature boundaries also need verification beyond source regex checks. |

## Live verification from this audit

- Fresh `cargo test --workspace` reached `sf-oracle/tests/semantic_trace.rs`
  and failed two tests. The independent integrated result reported
  `fields.timing.motion_refreshes` at sequence 892: reference 4, native 3.
  The frozen native semantic hash also failed at sequence 892. No fixtures
  were blessed. Later workspace targets were not run after Cargo stopped.
- The independent Mesen executable initially failed because its X11 runtime
  dependency was missing from the development environment. The flake now
  supplies Mesen's X11 and ICU dependencies in the dev shell only.
- A fresh retail restart trace confirms that the collision-bank response runs
  while the control lock skips lateral movement. `PSTRATS.ASM` at
  `player_collmove`, `.no_pctrl`, and the intervening left/right movement
  block explains the behavior. Rust now respects that gate. Focused tests
  cover ship-control, black-screen, wipe, and recovery locks without clearing
  the collision state.
- A fresh native replay now has player X=0 at scene 945, matching retail,
  where the previous native capture had X=2. This is evidence for the specific
  fix, not a full 1–983 parity claim: incomplete scene-944 drawing evidence
  and unverified path correspondence still block that claim.
- Temporary restart diagnostic prints were removed. The architecture check
  and the new comparator/inventory rejection tests pass. An export of only the
  staged files passes both focused player tests and all 19 verifier tests.
- A separate gameplay-package run passed the game targets and then failed the
  legacy C `ponpon` path trace on collision flags (64 versus 80). This is
  recorded as an unresolved fixture/source-contract discrepancy; the expected
  trace was not regenerated from Rust.

Local detailed evidence: `/tmp/sf1-workspace-audit-20260904.log`,
`/tmp/sf1-restart-review-fixed-env-20260904/`, and
`/tmp/sf1-native-control-gate-20260904.txt`. These paths are local diagnostic
artifacts, not reproducible release fixtures or distributed ROM data.

## Work order

1. **Establish trustworthy boundaries.** Split simulation snapshots from
   completed draw/audio output; record object generation plus allocation order.
   Pin input, ROM, catalog, and adapter versions. Compare every update in a
   scenario. Reject missing records, duplicate fields, guessed mappings, and
   skips. Preserve both raw captures and the earliest causal difference.
2. **Review shared semantics before individual enemies.** Recover contracts
   for signed widths, wrapping arithmetic, shifts and rounding, state-machine
   fallthrough, control gates, flag layout, allocation/freeing, parent/child
   relationships, collision dispatch, and timing. Use exhaustive tests for
   small domains and deterministic boundary/fuzz inputs against retail for
   larger routines. A C translation is supporting evidence only.
3. **Make the next source behavior reachable by normal input.** Expand replay
   manifests from boot through each route, difficulty, special exit, boss,
   death, restart, continue, and ending. Include observe, hit, destroy, evade,
   and parent/child interaction variants. Save legal retail checkpoints to
   reduce iteration time; replay the full route after local repairs.
4. **Close presentation independently.** Compare source-resolution completed
   frames, exact asset identity and draw order, dialogue/UI layout, ordered
   music/SFX events, and intended PCM output. Keep HD interpolation and optional
   visual effects outside the authoritative simulation and separately tested.
5. **Track proof, not ported-function counts.** Each source behavior needs its
   Rust location, reviewed contract, independent test, reached branch/event
   evidence, and current pass/fail result. Distinguish implemented, reviewed,
   routine-tested, controller-replay-tested, and presentation-tested. A green
   row cannot be inferred from a build, a native fixture, or an assisted soak.

Completion remains the release conditions in `RETAIL_PARITY_PLAN.md`: no
unexplained differences across the declared full corpus and coverage, an
unassisted runtime gate, architecture compliance, and a verified published
revision. SF2 certification follows SF1. Finite test runs alone do not prove
equivalence for every possible input sequence; universal claims require
stronger routine contracts/proofs as well as measured coverage.
