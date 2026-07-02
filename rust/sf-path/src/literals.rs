//! Path literal catalog (C oracle: `src/path/path_literals.c`
//! `build_path_catalog` / `PathLiterals_GetCatalog`).

use std::sync::OnceLock;

use crate::builder::{PathLiteralBuilder, PATH_MISSING_OFFSET};
use crate::catalog_data;
use crate::ids::PATH_DATA_COUNT_LITERAL;
use crate::opcodes::{P_END, P_REMOVE};

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
}

/// C `build_path_catalog`: prologue + the generated emit sequence + resolve,
/// with the C failure fallback (single P_REMOVE blob, offsets all missing).
pub fn build() -> PathCatalog {
    let mut b = PathLiteralBuilder::new();
    let mut ips = InlineIps::default();

    catalog_data::emit_all(&mut b, &mut ips);
    b.resolve();

    if b.failed {
        return PathCatalog {
            data: vec![P_REMOVE, P_END],
            offsets: vec![PATH_MISSING_OFFSET; PATH_DATA_COUNT_LITERAL as usize],
            ips: InlineIps::default(),
        };
    }

    PathCatalog { data: b.data, offsets: b.offsets, ips }
}

/// C `PathLiterals_GetCatalog` (minus the game-side
/// `seed_runtime_tables`/`register_inline_callbacks`, which belong to the
/// game-core lane).
pub fn get_catalog() -> &'static PathCatalog {
    static CATALOG: OnceLock<PathCatalog> = OnceLock::new();
    CATALOG.get_or_init(build)
}
