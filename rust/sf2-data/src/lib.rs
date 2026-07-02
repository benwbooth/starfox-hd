//! Star Fox 2 extracted game data (audio banks, color/material tables, text,
//! and 3D-shape candidates). Data-only phase 1 of the SF2 port — reuses the
//! SF1 engine formats located in docs/SF2_RECON.md.
//!
//! Extraction tooling: `tools/sf2/*.py` reads the retail ROM (user-owned) at
//! the recon offsets and emits the generated modules here. Regenerate with:
//! `nix develop --command python3 tools/sf2/extract.py`.
//!
//! ## What is extracted (data-only, high confidence)
//! - [`audio`]  — the SPC upload-blob manifest (0xCBE1E chain). The block
//!   format is byte-identical to SF1, so `sf-audio`'s `Booter` can upload each
//!   `data/sf2/snd/SF2SND##.BIN` unchanged.
//! - [`colors`] — the bank-01 material-word tables (0x806C..0x86F1), decoded
//!   with the same encoding `sf-render`'s color resolver consumes.
//! - [`text`]   — the located name/label tables (HUD, credits, boss names,
//!   rival names). Factual game-data tables, not creative prose.
//!
//! ## What is DEFERRED (needs a clean-room ROM disassembly)
//! - [`shape_data`] — SF1's point/face byte-grammar does NOT resolve SF2's 3D
//!   shapes (0 self-consistent face streams parse; SF2's GSU-2 data is
//!   reordered/re-encoded, likely compressed). Only unverified point-block
//!   candidates are emitted; real geometry extraction lands with the shape
//!   header/pointer layout once a disassembly pins it.
//! - Gameplay logic (map VM, strategies, paths, the strategic-map layer) is
//!   entirely out of scope for this phase and lands later as `sf2-map` /
//!   `sf2-strat` / `sf2-meta` (see docs/SF2_RECON.md sections 3–6).

pub mod audio;
pub mod colors;
pub mod shape_data;
pub mod text;
