//! Game core: world/object lifecycle, map VM execution, collision,
//! game state machine.
//!
//! Ports (C oracle): `src/game/world.c`, `src/map/map_exec.c`,
//! `src/game/obj.c`, `src/game/coldet.c`, `src/game/nmi.c`,
//! `src/game/boot.c`, `src/game/game.c` (camera), `src/game/game_vars.c`.
//! Per-tick behavior is validated against the C oracle's SF_STATE_DUMP
//! trace via sf-difftest.
//!
//! Phase 1 (this crate today) is the deterministic skeleton:
//! - [`vars`]   — GameVars (the ported subset of game_vars.c globals + WRAM)
//! - [`alien`]  — the Alien struct (game-core copy; consolidation TODO with
//!   sf-path's) and flag constants
//! - [`alien_compat`] — SNES byte-offset access used by map bytecode
//! - [`obj`]    — alien pool, free/active lists (obj.c)
//! - [`world`]  — map executor state, registries, builtin spacebar istrats
//! - [`game`]   — [`game::Game`]: tick entry (Nmi_GameTick subset), the
//!   dostrats strategy loop, and the map-VM opcode dispatcher (world.c)
//! - [`coldet`] — collision list + AABB pair testing (coldet.c)
//!
//! Strategies are NOT here: the strategy table starts empty (objects spawn
//! inert, exactly as the C build supports); sf-strat will register entries
//! through `World::register_strategy` / `World::istrats`.
//!
//! Verification: `tests/trace_parity.rs` replays a scripted fake player
//! (worldz += MEDPSPEED per tick) over MAP_ID_1_1 and asserts per-tick
//! equality with fixtures dumped from a C harness that compiled the real
//! world.c/map_exec.c/obj.c/coldet.c/game_vars.c/levels.c.

pub mod alien;
pub mod alien_compat;
pub mod coldet;
pub mod game;
pub mod obj;
pub mod trig8;
pub mod vars;
pub mod world;

// Shell/glue layer (C oracle: boot.c state machine, draw.c Draw_BuildList,
// game.c camera, bgs.c/windows.c/strings.c/planets.c frame-input producers).
pub mod bgs;
pub mod camera;
pub mod charmap;
pub mod clip;
pub mod debug_draw;
pub mod dma;
pub mod draw;
pub mod foxy;
pub mod gameplay_timing;
pub mod heap;
pub mod planets;
pub mod point_field;
pub mod presentation;
pub mod score;
pub mod shell;
pub mod strings;
pub mod windows;

pub use game::{Game, Hooks, NullHooks};
