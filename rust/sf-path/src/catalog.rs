//! Path catalog wrapper (C oracle: `src/path/path_catalog.c`).
//!
//! The C file is a thin indirection over `PathLiterals_GetCatalog`; this
//! module re-exports the literal catalog under the same role.

pub use crate::literals::{get_catalog, PathCatalog};
