---
name: reap-background-jobs
description: Kill background build/game processes when done; they pile up and slow builds
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Long sessions in this repo accumulated ~10 orphaned processes (hung `cargo`
builds + `starfox-hd-rs` game instances up to ~5h old) from timed-out
background commands. Multiple detached `cargo` processes contend for the
`rust/target` lock, which is why builds crawled to a halt. The user noticed
("why are there 10 shells open?", "can you check your tool?").

**Why:** background Bash commands that time out leave the wrapped `nix develop`
/ `cargo` / game process running detached (the timeout only kills the bash
wrapper). `SF_HIDDEN=1` game runs also hang on the wgpu surface (can't present
to a hidden window) and never exit.

**How to apply:**
- Chain `; pkill -9 -f starfox-hd-rs` after any game launch so it self-reaps.
- Prefer visible (not `SF_HIDDEN`) or windowless runs; `SF_HIDDEN` hangs wgpu.
- For game-logic/input verification use a **windowless test** driving
  `sf_game::shell::Shell` directly (see `rust/sf-app/tests/steering.rs`) — no
  window, no wgpu, no hang, and fast/reliable vs headless game runs.
- Periodically `pkill -9 -f 'cargo|rustc|starfox-hd-rs'` if builds slow down;
  stale target-lock contention is the usual cause. See [[build-run-commands]].
