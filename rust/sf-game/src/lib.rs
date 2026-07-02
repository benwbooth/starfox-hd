//! Game core: world/object lifecycle, map VM execution, collision,
//! game state machine.
//!
//! Ports (C oracle): `src/game/world.c`, `src/map/map_exec.c`,
//! `src/game/obj.c`, `src/game/coldet.c`, `src/game/nmi.c`,
//! `src/game/boot.c`, `src/game/game.c` (camera), `src/game/game_vars.c`.
//! Per-tick behavior is validated against the C oracle's SF_STATE_DUMP
//! trace via sf-difftest.
