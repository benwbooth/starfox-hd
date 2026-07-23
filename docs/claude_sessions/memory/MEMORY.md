# Star Fox HD - Session Memory

- [Overhaul phase 2 status](overhaul-phase2-status.md) — queue drained but game unplayable; 4-lane fix (colors/audio/2D/progression) root causes and file map
- [RIIR decision](riir-decision.md) — full Rust rewrite planned AFTER C port verified playable; C build is the oracle
- [Steam Controller 2](user-hardware-steam-controller-2.md) — Ben's gamepad needs SDL3/dev SDL; gamepad work is post-RIIR in sf-app
- [Star Fox 2 scope](starfox2-scope.md) — SF2 ROM in repo, recon done (docs/SF2_RECON.md); blocked on acquiring a disassembly (Gigaleak source)

- [sf-map regs bless gotcha](sfmap-regs-bless-gotcha.md) — re-wiring a map placement that changes bytecode length needs a manual .regs.txt update; SF_BLESS only rewrites .bin

## Project
Star Fox SNES 65816 ASM -> C/OpenGL HD reimplementation.

## Build
- `nix develop --command bash -c "cmake --build build"`
- No python3 available directly; use `nix develop --command python3` or `uv run python`

## Queue System
- `automation/port_queue.tsv` - task queue
- `scripts/port_queue.sh` - claim-task, set-status, ready, stats, list
- One `in_progress` per lane (map, strat, path, renderer)
- Reference ASM at `reference/ultrastarfox/SF/`

## RIIR Progress
- sf-render RUNTIME ported (2026-07-01): full C renderer pipeline in Rust (glow, GL 3.3 core).
  Modules gl_backend/transform/shapes_gl/draw_list/bg2d/font/sprites/hud/ui/particles/renderer;
  `Renderer::submit(prev, curr, alpha, &FrameInputs)` mirrors Renderer_SubmitDrawList. FrameInputs
  is the plain-data bridge for game globals (no sf-game dep). 15 tests green incl. offscreen GL
  (sdl3 dev-dep, hidden window, needs DISPLAY; sf-render/build.rs adds SDL3 rpath for tests).
  Title golden 8x8 grid (from C SF_DUMP_PPM) lives in rust/sf-render/tests/common/mod.rs.

## Key Files
- `src/map/levels.c` - MapBuilder API, all map bytecodes
- `src/strat/strat_enemy.c` - enemy strategies
- `src/strat/strat_player.c` - player strategies
- `src/strat/strat_table.c` - strategy registration
- `src/path/path_literals.c/h` - path bytecodes
- `src/renderer/shapes.c` - shape registration and rendering
- `src/renderer/shape_data.h` - generated shape data (236 shapes)
- `tools/shape_compiler.py` - ASM→C shape compiler

## Shape System
- Shape IDs use "MACRO-counted" numbering (def_shape MACRO = ID 0) to match g_istrat_shape_defaults
- 236 of 248 shapes compiled; 12 missing are wireframe-only (Face2)
- Hardcoded builtins (Arwing at ID 2, Boss7 at IDs 56/240-245/480+) override ASM data
- Run `nix develop --command python3 tools/shape_compiler.py` to regenerate shape_data.h

## Porting Patterns
- MapBuilder: `mapobj` -> `mb_mapobj`, `pathobj` -> `mb_pathobj`, `mapwait` -> `mb_mapwait`
- Clear demos: `append_cl_*_submap()` static functions, friend alive checks via MAP_CB_*
- Strategy: `Strat_*_Init(Alien *self)` + `*_strat(Alien *self)` tick
- ISTRAT indices from `reference/ultrastarfox/SF/STRAT/ISTRATS.ASM`
- CL_SHIP/UNDER/DIVE/BRIDG/TURN indices: 25-45 (SHIP=25-27, TURN=31-33, BRIDGE=34-36, DIVE=40-42, UNDER=43-45)

## User Preferences
- Use Claude agents for porting, not Codex
- Port tasks directly in session via background agents
- [Reap background jobs](reap-background-jobs.md) — kill orphaned build/game procs; use windowless shell tests for input/logic
- [ROM oracle plan](rom-oracle-plan.md) — automated ROM-differential accuracy harness (user priority); dosbox-x+xvfb symbols, w65c816 per-function diff
- [Git archaeology hazard](git-archaeology-hazard.md) — old-commit checkouts detach HEAD + orphan the chain; recover via reflog
