//! Path bytecode data and interpreter.
//!
//! Ports (C oracle): `src/path/paths.c`, `src/path/path_literals.c/h`,
//! `src/path/path_catalog.c`. When the extracted user-owned catalog is
//! available, path data comes directly from the reference assembler's ROM
//! bytes. The builder-emitted literal catalog remains the no-ROM fallback.
//!
//! C source mapping:
//! - `src/path/paths.h` opcode constants  -> [`opcodes`]
//! - `src/path/path_literals.h` path ids  -> [`ids`]
//! - `src/path/path_literals.c` builder   -> [`builder`]
//! - `src/path/path_literals.c` catalog   -> [`literals`] (+ generated
//!   transcription in `catalog_data`)
//! - `src/path/path_catalog.c`            -> [`catalog`]
//! - `src/game/obj.h` Alien struct        -> [`alien`] (path-lane copy; may
//!   migrate to `sf-core` once the game-core lane lands)
//! - `src/game/alien_compat.c`            -> [`alien_compat`]
//! - `src/path/paths.c` interpreter       -> [`interp`]
//!
//! Byte equality with the C build is the acceptance bar: the fixture test in
//! `tests/catalog_bytes.rs` compares the built catalog against a dump produced
//! by compiling `src/path/path_literals.c` directly.

pub mod alien;
pub mod alien_compat;
pub mod builder;
pub mod catalog;
mod catalog_data;
pub mod ids;
pub mod interp;
pub mod literals;
pub mod opcodes;
pub mod rom_catalog_data;
