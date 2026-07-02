//! route1 level builders (lane-owned; see the route porting lane).
//!
//! Wave-2 ROUTE 1 ports: MAP_ID_1_2, 1_3, 1_4, 1_5, 1_6, BLACKHOLE, INTRO.
//! C oracle: `src/map/levels.c` `build_level1_2_wrapper_slice()` etc.
//!
//! Levels register here so porting lanes never edit shared files:
//! add `pub mod levelX_Y;` plus a match arm in [`get_full`].

pub mod blackhole;
pub mod intro;
pub mod level1_2;
pub mod level1_3;
pub mod level1_4;
pub mod level1_5;
pub mod level1_6;

use std::sync::OnceLock;

use crate::catalog::map_id;
use crate::levels::BuiltLevel;

/// A route1-owned level: the shared [`BuiltLevel`] plus raw callback
/// registration records.
///
/// TODO(consolidation): the shared `levels::NativeCallback` /
/// `levels::InlineCallback` enums live in `levels/mod.rs`, which this lane
/// must not edit. Until the route lanes merge, route1 levels record their
/// callback registrations as raw `(addr24/script_ptr, C function name)`
/// pairs here, in the exact C registration-call order (the parity test
/// `tests/route1_parity.rs` verifies that order against the fixtures).
/// The `BuiltLevel::native_callbacks` / `inline_callbacks` lists are left
/// EMPTY for route1 levels; the loader must consult these fields instead
/// once the executor is wired up, at which point the identities below
/// should become enum variants in `levels/mod.rs`.
pub struct Route1Level {
    /// Blob + labels (typed callback lists intentionally empty; see above).
    pub level: BuiltLevel,
    /// `World_RegisterNativeCallback` calls: (MAP_CB_* addr24, C fn name),
    /// in C registration-call order.
    pub native_regs: Vec<(u32, &'static str)>,
    /// `World_RegisterInlineMapCode` calls: (CODE65816 script ptr, C fn
    /// name), in C registration-call order.
    pub inline_regs: Vec<(u16, &'static str)>,
}

/// Lane-owned map-id dispatch, chained from `catalog::get_map_data`.
pub fn get(id: u32) -> Option<&'static BuiltLevel> {
    get_full(id).map(|l| &l.level)
}

/// Full route1 dispatch, exposing the raw callback registration records
/// (used by the parity test and, later, the loader).
pub fn get_full(id: u32) -> Option<&'static Route1Level> {
    match id {
        map_id::M1_2 => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(level1_2::build))
        }
        map_id::M1_3 => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(level1_3::build))
        }
        map_id::M1_4 => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(level1_4::build))
        }
        map_id::M1_5 => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(level1_5::build))
        }
        map_id::M1_6 => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(level1_6::build))
        }
        map_id::BLACKHOLE => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(blackhole::build))
        }
        map_id::INTRO => {
            static LEVEL: OnceLock<Route1Level> = OnceLock::new();
            Some(LEVEL.get_or_init(intro::build))
        }
        _ => None,
    }
}
