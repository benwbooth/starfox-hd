---
name: riir-decision
description: User wants full Rust rewrite (RIIR) after the C port is stabilized and verified playable
metadata: 
  node_type: memory
  type: project
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Decided 2026-07-01: the whole project will be rewritten in Rust (user request, "RIIR"). Chosen approach (user-confirmed): **stabilize C first, then RIIR** — finish the playability overhaul, commit, verify the game end-to-end, then port to Rust module-by-module using the working C build as a behavioral oracle. Do NOT start the Rust workspace before the C port is verified playable.

**Why:** the C port serves as the behavioral reference/oracle for the Rust port; rewriting before verification risks porting bugs twice.

**How to apply:** after playability is verified (title → routes completable → ending), plan the Rust workspace (crate per subsystem: renderer, audio/spc, game core, map VM, strats, paths) and port incrementally with differential testing against the C build. snes_spc is C++ — either keep behind FFI initially or use a Rust SPC emulator crate. Related: [[overhaul-phase2-status]]

**Status 2026-07-02:** RIIR essentially complete. ALL crates ported + committed and parity-verified: sf-core, sf-difftest, sf-map (30 maps byte-identical), sf-path (catalog + interpreter trace-identical), sf-render (data + GL runtime), sf-game (world/VM/obj/coldet, 500-tick trace-identical), sf-strat (full strategy corpus incl. all bosses, 35 tests), sf-audio, sf-app (SDL3 shell runs the game). **sf-spc: pure-Rust SPC-700 engine is bit-exact vs the snes_spc oracle (max_abs=0) — native is now default, the LAST C++ dependency is gone** (FFI retained only behind `--features ffi-oracle` for cross-check). Remaining native surface = SDL3 + GL driver + libm (deliberate, for parity). In flight: finish-line differential parity run (table.rs register_all wired into sf-app shell) and SF2 data extraction (sf2-data crate). Post-RIIR: enhanced mode (60Hz+/precision/resolution), gamepad (SC2 via SDL3), SF2 logic (disassembly-gated).

**Wave-2 fleet state (2026-07-01 ~10pm):** goal "/goal finish the riir" active. Wave 1 done (committed: sf-render data, sf-map phase 1, sf-path catalog — all byte-identical; sf-audio FFI + IPL boot test at `1fda346`). Seven lanes launched in parallel: route1/route2/route3 level builders (sf-map/src/levels/routeN/, per-map fixtures rN_*, no shared-file edits — extension traits for missing mb_* emitters, raw (value,name) callback records, consolidation TODOs); path interpreter (sf-path interp.rs, PathHost trait, pi_* trace fixtures); game core phase 1 (sf-game: vars/alien/obj/world+map_exec/coldet/tick, gc_* 500-tick spawn-trace parity vs C harness with empty strategy table); audio protocol (sf-audio boot/player/sound, au_*, offline title+Corneria render checks); render runtime (sf-render glow+SDL3, FrameInputs struct decouples from sf-game, rr_*, pixel-readback checks). Wave 3 after: sf-strat strategies (needs sf-game Ctx), sf-app SDL3 shell (gamepad/SC2), integration difftest vs SF_STATE_DUMP. Prep done: sdl3 in flake.nix devshell (pkg-config 3.4.0), crates.io reachable, catalog route dispatch chains routeN::get(id).
