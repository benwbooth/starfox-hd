//! Map bytecode builders (MapBuilder) and the map VM.
//!
//! Ports (C oracle): `src/map/levels.c` (all level slices + MapBuilder),
//! `src/map/map_exec.c`, `src/map/map_catalog.c`, `src/map/level*_data.h`.
//! Level bytecode must be byte-identical to what the C MapBuilder emits so
//! map-VM differential traces line up tick-for-tick.
//!
//! C source mapping:
//! - `src/map/levels.c` MapBuilder/`mb_*` -> [`builder`]
//! - map opcode / shape / strat constants -> [`consts`]
//! - `build_*_slice()` per level          -> [`levels`]
//! - `Levels_GetMapData` / map ids        -> [`catalog`]

pub mod builder;
pub mod catalog;
pub mod consts;
pub mod levels;
pub mod mothers;
