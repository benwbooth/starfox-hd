//! Level catalog — the `Levels_GetMapData` equivalent.
//!
//! C oracle: `src/map/levels.c` `Levels_GetMapData()` /
//! `ensure_literal_levels_built()` and `src/map/levels.h` MAP_ID_*.
//!
//! Ported subset (phase 1): MAP_ID_NONE, MAP_ID_1_1, MAP_ID_TITLE,
//! MAP_ID_CONTINUE, MAP_ID_WAIT (title-blob entry points) and
//! MAP_ID_PLANET. All other ids return `None`.
//!
//! Not yet ported (C `build_*` still the only source):
//! 1_2, 1_3, 1_4, 1_5, 1_6, 2_1, 2_2, 2_3, 2_4, 2_5, 2_6,
//! 3_1, 3_2, 3_3, 3_4, 3_5, 3_6, 3_7, BLACKHOLE, SPECIAL, FINAL,
//! INTRO, CREDITS, TRAINING.

use std::sync::OnceLock;

use crate::levels::{self, BuiltLevel};

/// Planet map IDs (matches PLANETS.ASM path table values; levels.h).
pub mod map_id {
    pub const NONE: u32 = 0;
    pub const M1_1: u32 = 1;
    pub const M1_2: u32 = 2;
    pub const M1_3: u32 = 3;
    pub const M1_4: u32 = 4;
    pub const M1_5: u32 = 5;
    pub const M1_6: u32 = 6;
    pub const M2_1: u32 = 7;
    pub const M2_2: u32 = 8;
    pub const M2_3: u32 = 9;
    pub const M2_4: u32 = 10;
    pub const M2_5: u32 = 11;
    pub const M2_6: u32 = 12;
    pub const M3_1: u32 = 13;
    pub const M3_2: u32 = 14;
    pub const M3_3: u32 = 15;
    pub const M3_4: u32 = 16;
    pub const M3_5: u32 = 17;
    pub const M3_6: u32 = 18;
    pub const M3_7: u32 = 19;
    pub const BLACKHOLE: u32 = 20;
    pub const SPECIAL: u32 = 21;
    pub const FINAL: u32 = 22;
    pub const INTRO: u32 = 23;
    pub const TITLE: u32 = 24;
    pub const CONTINUE: u32 = 25;
    pub const WAIT: u32 = 26;
    pub const PLANET: u32 = 27;
    pub const CREDITS: u32 = 28;
    pub const TRAINING: u32 = 29;
}

fn empty_level() -> &'static BuiltLevel {
    static LEVEL: OnceLock<BuiltLevel> = OnceLock::new();
    LEVEL.get_or_init(levels::build_empty)
}

fn level1_1() -> &'static BuiltLevel {
    static LEVEL: OnceLock<BuiltLevel> = OnceLock::new();
    LEVEL.get_or_init(levels::level1_1::build)
}

fn title() -> &'static BuiltLevel {
    static LEVEL: OnceLock<BuiltLevel> = OnceLock::new();
    LEVEL.get_or_init(levels::title::build)
}

fn planet() -> &'static BuiltLevel {
    static LEVEL: OnceLock<BuiltLevel> = OnceLock::new();
    LEVEL.get_or_init(levels::planet::build)
}

/// `Levels_GetMapData` equivalent for the ported subset.
///
/// Differences from C (documented, intentional):
/// - C rebuilds callback registrations on every call; here the
///   registration lists are part of [`BuiltLevel`] and the caller (map VM
///   loader) applies them. For `MAP_ID_WAIT` the C code returns the title
///   blob WITHOUT registering title callbacks; the wait entry point
///   (`title.waitmap` label) never reaches the CODE65816 hook, so using
///   the shared registration list is behavior-identical.
/// - Unported ids return `None` instead of a warn-once END stub.
pub fn get_map_data(id: u32) -> Option<&'static BuiltLevel> {
    match id {
        map_id::NONE => Some(empty_level()),
        map_id::M1_1 => Some(level1_1()),
        // TITLE, CONTINUE and WAIT share one blob with three entry points;
        // see levels::title for the `title.contmap` / `title.waitmap` labels.
        map_id::TITLE | map_id::CONTINUE | map_id::WAIT => Some(title()),
        map_id::PLANET => Some(planet()),
        // Route lanes register their levels in their own modules so the
        // porting lanes never edit shared files concurrently.
        _ => crate::levels::route1::get(id)
            .or_else(|| crate::levels::route2::get(id))
            .or_else(|| crate::levels::route3::get(id)),
    }
}

/// The route-lane callback registration records for a map id, as raw
/// `(native regs, inline regs)` name-keyed pairs (C registration-call order).
///
/// The route lanes stash their `World_RegisterNativeCallback` /
/// `World_RegisterInlineMapCode` registrations on their lane-local level
/// wrappers (see `Route{1,2,3}Level`) rather than on the shared [`BuiltLevel`],
/// whose callback vectors they leave EMPTY. The loader must consult these so a
/// route level's inline CODE65816 hooks (e.g. `level_scramble_keep_player_strat`
/// at the launch/exit-base handoff) are registered — otherwise the map VM halts
/// permanently at the first unregistered inline op and the opening sequence
/// never hands control back to the player.
///
/// Returns `None` for the non-route maps (NONE/M1_1/TITLE/PLANET), which
/// already carry their callbacks on [`BuiltLevel`] directly.
#[allow(clippy::type_complexity)]
pub fn get_map_callback_regs(
    id: u32,
) -> Option<(&'static [(u32, &'static str)], &'static [(u16, &'static str)])> {
    if let Some(l) = crate::levels::route1::get_full(id) {
        return Some((&l.native_regs, &l.inline_regs));
    }
    if let Some(l) = crate::levels::route2::get_route2(id) {
        return Some((&l.native, &l.inline));
    }
    if let Some(l) = crate::levels::route3::get_level(id) {
        return Some((&l.native_regs, &l.inline_regs));
    }
    None
}
