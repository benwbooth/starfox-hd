//! route2 level builders (lane-owned; see the route porting lane).
//!
//! Levels register here so porting lanes never edit shared files:
//! add `pub mod levelX_Y;` plus a match arm in [`get`].
//!
//! Ported (wave 2): MAP_ID_2_1..2_6, SPECIAL, CREDITS, TRAINING.
//! C oracle: `src/map/levels.c` `build_level2_*`, `build_level_special_slice`,
//! `build_credits_slice`, `build_training_slice` per `Levels_GetMapData`.

pub mod credits;
pub mod level2_1;
pub mod level2_2;
pub mod level2_3;
pub mod level2_4;
pub mod level2_5;
pub mod level2_6;
pub mod rc;
pub mod special;
pub mod submaps;
pub mod training;

use std::sync::OnceLock;

use crate::catalog::map_id;
use crate::levels::BuiltLevel;

/// Route2-local wrapper around [`BuiltLevel`].
///
/// TODO(consolidation): the shared `NativeCallback` / `InlineCallback` enums
/// in `levels/mod.rs` are off-limits to route lanes, so callback identities
/// are recorded here as raw `(value, C-function-name)` pairs in the C
/// registration-call order (C `register_*_inline_callbacks()`). When the
/// route lanes land, fold these into the shared enums and populate
/// `BuiltLevel::native_callbacks` / `inline_callbacks` instead.
pub struct Route2Level {
    pub level: BuiltLevel,
    /// (MAP_CB_* addr24, C callback fn name), in registration-call order.
    pub native: Vec<(u32, &'static str)>,
    /// (CODE65816 script ptr, C callback fn name), in registration-call
    /// order. Entries with ptr 0 are dropped, mirroring the C
    /// `if (ptr != 0u)` guards.
    pub inline: Vec<(u16, &'static str)>,
}

impl Route2Level {
    pub(crate) fn new(
        data: Vec<u8>,
        labels: Vec<crate::builder::Label>,
        native: Vec<(u32, &'static str)>,
        inline: Vec<(u16, &'static str)>,
    ) -> Route2Level {
        Route2Level {
            level: BuiltLevel {
                data,
                labels,
                // TODO(consolidation): see the struct docs — registrations
                // live in `native`/`inline` until the shared enums open up.
                native_callbacks: Vec::new(),
                inline_callbacks: Vec::new(),
            },
            native,
            inline: inline.into_iter().filter(|&(ptr, _)| ptr != 0).collect(),
        }
    }
}

macro_rules! route2_static {
    ($fn_name:ident, $module:ident) => {
        fn $fn_name() -> &'static Route2Level {
            static LEVEL: OnceLock<Route2Level> = OnceLock::new();
            LEVEL.get_or_init($module::build)
        }
    };
}

route2_static!(level2_1_level, level2_1);
route2_static!(level2_2_level, level2_2);
route2_static!(level2_3_level, level2_3);
route2_static!(level2_4_level, level2_4);
route2_static!(level2_5_level, level2_5);
route2_static!(level2_6_level, level2_6);
route2_static!(special_level, special);
route2_static!(credits_level, credits);
route2_static!(training_level, training);

/// Full route2 dispatch, including the lane-local callback registrations.
pub fn get_route2(id: u32) -> Option<&'static Route2Level> {
    match id {
        map_id::M2_1 => Some(level2_1_level()),
        map_id::M2_2 => Some(level2_2_level()),
        map_id::M2_3 => Some(level2_3_level()),
        map_id::M2_4 => Some(level2_4_level()),
        map_id::M2_5 => Some(level2_5_level()),
        map_id::M2_6 => Some(level2_6_level()),
        map_id::SPECIAL => Some(special_level()),
        map_id::CREDITS => Some(credits_level()),
        map_id::TRAINING => Some(training_level()),
        _ => None,
    }
}

/// Lane-owned map-id dispatch, chained from `catalog::get_map_data`.
pub fn get(id: u32) -> Option<&'static BuiltLevel> {
    get_route2(id).map(|entry| &entry.level)
}
