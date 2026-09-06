//! Star Fox 2 extracted game data: audio, color/material tables, text, and the
//! mechanically reachable map and object-path programs.
//!
//! Extraction tooling: `tools/sf2/*.py` reads the retail ROM (user-owned) at
//! the recon offsets and emits the generated modules here. Regenerate with:
//! `nix develop --command python3 tools/sf2/extract.py`.
//!
//! ## What is extracted (data-only, high confidence)
//! - [`audio`]  — the exact reset-time sound-driver upload at file `0xD0000`,
//!   the broader upload catalog beginning at `0xCBE1E`, and all 50 decoded
//!   host audio-program records. The shipping player consumes semantic PCM
//!   rendered offline from these records; upload emulation is oracle-only.
//! - [`colors`] — the bank-01 material-word tables (0x806C..0x86F1), decoded
//!   with the same encoding `sf-render`'s color resolver consumes.
//! - [`lighting`] — the exact SF2 depth-colour and per-face light-shade pair
//!   tables used by the polygon renderer.
//! - [`text`]   — the located name/label tables (HUD, credits, boss names,
//!   rival names). Factual game-data tables, not creative prose.
//! - [`map`] — all 25 real script roots, 4,094 reachable commands, 232 exact
//!   spawn records, and 262 resolved inline-code continuations. The two
//!   reset-time ROM-to-WRAM code-copy mappings are included so RAM strategies
//!   map back to their original bytes.
//!
//! - [`shape_data`] — all 577 contiguous retail ShapeHdr records and their
//!   exact Argonaut point/face programs: 11,860 decoded vertices and 10,524
//!   BSP-unioned polygon records, with every vertex index checked. The two
//!   non-polygon procedural shapes are explicitly classified.
//! - [`shape_program_data`] — 4,037 typed face-program nodes retaining the
//!   authored visibility tables, BSP links, face ranges and continuations.
//! - [`point_program_data`] — 3,698 authored point blocks across 1,784 frame
//!   entries, preserving byte/word encoding and mirrored-pair boundaries.
//! - [`textures`] — all 211 exact polygon-texture descriptors, 12 coordinate
//!   layouts, and the three packed-nibble source banks used by those records.
//! - [`palettes`] — the five exact 16-color BGR555 polygon-palette rows;
//!   shipping code selects verified live rows by semantic scene identity.
//! - [`draw`] — the exact counted 38-byte render-list ABI constructed by the
//!   retail 65816 routine at `$02:9201..$02:947D`.
//! - [`path`] — the complete 106-root reachable object-path graph: 11,798 exact
//!   raw commands, 274 dispatch handlers, handler-derived pointer effects, and
//!   CFG successors. Every handler has a proof-gated semantic identity and a
//!   typed implementation. All 42 reachable script-embedded inline blocks have
//!   typed control flow, and all 20 named gameplay service bodies are direct
//!   Rust with isolated retail edge differentials.
//!
//! ## What is DEFERRED
//! - Complete strategy semantics and broader native gameplay coverage are still
//!   being decompiled from the exact targets exposed by [`map`].
//!   `sf2-path` is verification staging, not the shipping game's state
//!   architecture.

pub mod audio;
#[cfg(feature = "oracle-data")]
pub mod collision_data;
pub mod colors;
pub mod compression;
#[cfg(feature = "oracle-data")]
pub mod draw;
pub mod lighting;
#[cfg(feature = "oracle-data")]
pub mod map;
#[cfg(feature = "oracle-data")]
pub mod map_vm;
pub mod opening_artwork;
#[cfg(feature = "oracle-data")]
pub mod oracle_audio;
pub mod palettes;
#[cfg(feature = "oracle-data")]
pub mod path;
pub mod point_program;
pub mod point_program_data;
pub mod shape_data;
pub mod shape_program;
pub mod shape_program_data;
pub mod text;
pub mod textures;
