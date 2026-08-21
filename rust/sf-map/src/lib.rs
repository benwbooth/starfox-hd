//! Map bytecode builders (MapBuilder) and the map VM.
//!
//! The original Rust transcription used the removed C port as a scaffold.
//! The authoritative encoding is now `reference/ultrastarfox/SF/INC/MAPMACS.INC`
//! plus the retail map VM in `WORLD.ASM`; fixture blobs are source-correct
//! regression snapshots of the typed builders.
//!
//! Port mapping:
//! - `MAPMACS.INC` map macros             -> [`builder`]
//! - map opcode / shape / strat constants -> [`consts`]
//! - `build_*_slice()` per level          -> [`levels`]
//! - `Levels_GetMapData` / map ids        -> [`catalog`]

pub mod builder;
pub mod catalog;
pub mod consts;
pub mod istrat_shapes;
pub mod levels;
pub mod mothers;
