//! route3 level builders (lane-owned; see the route porting lane).
//!
//! C oracle: `src/map/levels.c` `build_level3_*` / `build_final_slice()` per
//! the `Levels_GetMapData` switch. Byte-equality is enforced by
//! `tests/route3_parity.rs` against fixtures dumped from the C builders.
//!
//! Callback registrations: the shared `levels::NativeCallback` /
//! `levels::InlineCallback` enums are off-limits to this lane, so the
//! [`BuiltLevel`] callback vectors stay EMPTY for route-3 maps and the
//! registration identities are recorded as raw `(value, &'static str)`
//! pairs on [`Route3Level`] (`inline_regs` / `native_regs`, in the C
//! registration-call order, C function names as strings).
//! TODO(consolidation): once the shared enums can gain route-3 variants,
//! fold these into `BuiltLevel::{native,inline}_callbacks` and delete the
//! wrapper (also consolidate the duplicated submaps/consts in `common`).

pub mod common;
pub mod final_map;
pub mod level3_1;
pub mod level3_2;
pub mod level3_3;
pub mod level3_4;
pub mod level3_5;
pub mod level3_6;
pub mod level3_7;

use std::sync::OnceLock;

use crate::builder::Label;
use crate::catalog::map_id;
use crate::levels::BuiltLevel;

/// A built route-3 level plus its callback-registration record.
pub struct Route3Level {
    /// The bytecode blob + labels (callback vectors intentionally empty,
    /// see the module docs).
    pub built: BuiltLevel,
    /// C `register_*_inline_callbacks()` World_RegisterInlineMapCode calls,
    /// in call order: (CODE65816 script ptr, C callback function name).
    pub inline_regs: Vec<(u16, &'static str)>,
    /// World_RegisterNativeCallback calls, in call order. Route 3 maps
    /// register no natives today (kept for fixture symmetry).
    pub native_regs: Vec<(u32, &'static str)>,
}

/// Shared tail for the route-3 builders: drops zero script ptrs exactly
/// like the C `if (ptr != 0u) World_RegisterInlineMapCode(...)` guards.
pub(crate) fn finish_level(
    data: Vec<u8>,
    labels: Vec<Label>,
    inline_regs: Vec<(u16, &'static str)>,
) -> Route3Level {
    Route3Level {
        built: BuiltLevel {
            data,
            labels,
            native_callbacks: Vec::new(),
            inline_callbacks: Vec::new(),
        },
        inline_regs: inline_regs.into_iter().filter(|&(p, _)| p != 0).collect(),
        native_regs: Vec::new(),
    }
}

macro_rules! cached {
    ($fn_name:ident, $module:ident) => {
        fn $fn_name() -> &'static Route3Level {
            static LEVEL: OnceLock<Route3Level> = OnceLock::new();
            LEVEL.get_or_init($module::build)
        }
    };
}

cached!(built_3_1, level3_1);
cached!(built_3_2, level3_2);
cached!(built_3_3, level3_3);
cached!(built_3_4, level3_4);
cached!(built_3_5, level3_5);
cached!(built_3_6, level3_6);
cached!(built_3_7, level3_7);
cached!(built_final, final_map);

/// Route-3 lane dispatch including the registration record (parity tests).
pub fn get_level(id: u32) -> Option<&'static Route3Level> {
    match id {
        map_id::M3_1 => Some(built_3_1()),
        map_id::M3_2 => Some(built_3_2()),
        map_id::M3_3 => Some(built_3_3()),
        map_id::M3_4 => Some(built_3_4()),
        map_id::M3_5 => Some(built_3_5()),
        map_id::M3_6 => Some(built_3_6()),
        map_id::M3_7 => Some(built_3_7()),
        map_id::FINAL => Some(built_final()),
        _ => None,
    }
}

/// Lane-owned map-id dispatch, chained from `catalog::get_map_data`.
pub fn get(id: u32) -> Option<&'static BuiltLevel> {
    get_level(id).map(|level| &level.built)
}
