//! Path literal catalog (C oracle: `src/path/path_literals.c`
//! `build_path_catalog` / `PathLiterals_GetCatalog`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::builder::{PathLiteralBuilder, PATH_MISSING_OFFSET};
use crate::catalog_data;
use crate::ids::PATH_DATA_COUNT_LITERAL;
use crate::opcodes::{P_END, P_REMOVE};
use crate::rom_catalog_data;

/// Inline CODE65816 script pointers captured during the build (C: the
/// file-static `s_*_ip` variables). Offsets into the catalog data blob;
/// [`PATH_MISSING_OFFSET`] until the owning script is emitted.
///
/// The game-core lane maps these to native callbacks at load time (C:
/// `register_inline_callbacks`); until it lands they are data only.
#[derive(Debug, Clone, Copy)]
pub struct InlineIps {
    pub tow_0_set_expstrat: u16,
    pub robexplode_nopolyexp: u16,
    pub dsmoke_init_colanim: u16,
    pub dsmoke_add_colanim: u16,
    pub pbooston_makeengine: u16,
    pub pboostcode_updateengine: u16,
    pub makepollen: u16,
    pub e_big_bird_touch: u16,
    pub dintro1_zoom_to_centre: u16,
    pub dintro1_keep_distance: u16,
    pub checkifend1: u16,
    pub checkifend2: u16,
    pub checkifend3: u16,
    pub checkifend4: u16,
    pub checkifend5: u16,
    pub checkifend6: u16,
    pub checkifend7: u16,
}

impl Default for InlineIps {
    fn default() -> Self {
        InlineIps {
            tow_0_set_expstrat: PATH_MISSING_OFFSET,
            robexplode_nopolyexp: PATH_MISSING_OFFSET,
            dsmoke_init_colanim: PATH_MISSING_OFFSET,
            dsmoke_add_colanim: PATH_MISSING_OFFSET,
            pbooston_makeengine: PATH_MISSING_OFFSET,
            pboostcode_updateengine: PATH_MISSING_OFFSET,
            makepollen: PATH_MISSING_OFFSET,
            e_big_bird_touch: PATH_MISSING_OFFSET,
            dintro1_zoom_to_centre: PATH_MISSING_OFFSET,
            dintro1_keep_distance: PATH_MISSING_OFFSET,
            checkifend1: PATH_MISSING_OFFSET,
            checkifend2: PATH_MISSING_OFFSET,
            checkifend3: PATH_MISSING_OFFSET,
            checkifend4: PATH_MISSING_OFFSET,
            checkifend5: PATH_MISSING_OFFSET,
            checkifend6: PATH_MISSING_OFFSET,
            checkifend7: PATH_MISSING_OFFSET,
        }
    }
}

/// The built literal catalog (C `PathLiteralCatalog`).
pub struct PathCatalog {
    /// Path bytecode blob (byte-identical to the C build).
    pub data: Vec<u8>,
    /// Per-path-id start offsets; 0xFFFF for unported ids.
    pub offsets: Vec<u16>,
    /// Captured inline script pointers.
    pub ips: InlineIps,
    /// Native action offset to source-level continuation offset.
    pub inline_continuations: Vec<(u16, u16)>,
}

fn fnv1a(data: &[u8]) -> u32 {
    data.iter().fold(0x811C_9DC5, |value, &byte| {
        (value ^ byte as u32).wrapping_mul(0x0100_0193)
    })
}

fn catalog_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("STARFOX_PATH_CATALOG") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(PathBuf::from("data/path_catalog.bin"));
    candidates.push(PathBuf::from("../data/path_catalog.bin"));
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("data/path_catalog.bin"),
    );
    candidates
}

/// Load the exact assembled PATHDATA/DPATHDAT/KPATHDAT blob extracted by
/// `tools/extract_sf1_paths.py`. Its 16-bit branch operands and generated
/// start offsets are relative to the beginning of `paths`, matching the
/// original interpreter's path-table lookup.
fn load_assembled_catalog() -> Option<PathCatalog> {
    // Test/packaging hook: exercise the ROM-less fallback even when a local
    // user-owned extraction is present in the repository.
    if std::env::var_os("SF_FORCE_PATH_FALLBACK").is_some() {
        return None;
    }
    for path in catalog_candidates() {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        if data.len() != rom_catalog_data::ROM_PATH_CATALOG_SIZE {
            eprintln!(
                "Paths: ignoring {} ({} bytes, expected {})",
                path.display(),
                data.len(),
                rom_catalog_data::ROM_PATH_CATALOG_SIZE,
            );
            continue;
        }
        if fnv1a(&data) != rom_catalog_data::ROM_PATH_SECTION_FNV1A {
            eprintln!(
                "Paths: ignoring {} (reference path checksum mismatch)",
                path.display()
            );
            continue;
        }
        return Some(PathCatalog {
            data,
            offsets: rom_catalog_data::offsets(),
            ips: rom_catalog_data::inline_ips(),
            inline_continuations: rom_catalog_data::inline_continuations().to_vec(),
        });
    }
    None
}

/// Prefer the reference assembler's byte-exact ROM catalog. If the user-owned
/// extraction is absent, retain the former builder catalog so ordinary builds
/// and focused unit tests remain usable without distributing ROM bytes.
pub fn build() -> PathCatalog {
    if let Some(catalog) = load_assembled_catalog() {
        return catalog;
    }

    build_fallback()
}

/// Build the source-level catalog retained for ROM-less development and for
/// regression tests of the former hand-transcribed path programs.
pub fn build_fallback() -> PathCatalog {
    let mut b = PathLiteralBuilder::new();
    let mut ips = InlineIps::default();

    catalog_data::emit_all(&mut b, &mut ips);
    b.resolve();

    if b.failed {
        return PathCatalog {
            data: vec![P_REMOVE, P_END],
            offsets: vec![PATH_MISSING_OFFSET; PATH_DATA_COUNT_LITERAL as usize],
            ips: InlineIps::default(),
            inline_continuations: Vec::new(),
        };
    }

    let inline_continuations = b.inline_continuations();
    PathCatalog {
        data: b.data,
        offsets: b.offsets,
        ips,
        inline_continuations,
    }
}

/// C `PathLiterals_GetCatalog` (minus the game-side
/// `seed_runtime_tables`/`register_inline_callbacks`, which belong to the
/// game-core lane).
pub fn get_catalog() -> &'static PathCatalog {
    static CATALOG: OnceLock<PathCatalog> = OnceLock::new();
    CATALOG.get_or_init(build)
}
