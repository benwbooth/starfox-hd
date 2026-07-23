//! Level catalog — the `Levels_GetMapData` equivalent.
//!
//! C oracle: `src/map/levels.c` `Levels_GetMapData()` /
//! `ensure_literal_levels_built()` and `src/map/levels.h` MAP_ID_*.
//!
//! Every retail gameplay, shell, training, secret, and ending map has a
//! byte-equal Rust builder. `MAP_ID_NONE` remains the intentional one-byte
//! empty program.

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

/// Semantic identity recorded by each completed boss marker and consumed by
/// the post-campaign replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossEncounter {
    Route1Stage1,
    Route1Stage2,
    Route1Stage3,
    Route1Stage4,
    Route1Stage5,
    Route1Stage6,
    Route2Stage1,
    Route2Stage2,
    Route2Stage3,
    Route2Stage4,
    Route2Stage5,
    Route2Stage6,
    Route3Stage1,
    Route3Stage2,
    Route3Stage3,
    Route3Stage4,
    Route3Stage5,
    Route3Stage6,
    Route3Stage7,
    FinalBattle,
}

/// Translate the encounter-marker ordinal within a loaded catalog map. Most
/// maps contain one marker; the two routes whose last catalog entry includes
/// both its stage boss and the shared final battle contain two.
pub fn boss_encounter_for_marker(map: u32, marker_ordinal: u8) -> Option<BossEncounter> {
    use BossEncounter::*;
    Some(match (map, marker_ordinal) {
        (map_id::M1_1, _) => Route1Stage1,
        (map_id::M1_2, _) => Route1Stage2,
        (map_id::M1_3, _) => Route1Stage3,
        (map_id::M1_4, _) => Route1Stage4,
        (map_id::M1_5, _) => Route1Stage5,
        (map_id::M1_6, _) => Route1Stage6,
        (map_id::M2_1, _) => Route2Stage1,
        (map_id::M2_2, _) => Route2Stage2,
        (map_id::M2_3, _) => Route2Stage3,
        (map_id::M2_4, _) => Route2Stage4,
        (map_id::M2_5, _) => Route2Stage5,
        (map_id::M2_6, 0) => Route2Stage6,
        (map_id::M2_6, 1) => FinalBattle,
        (map_id::M3_1, _) => Route3Stage1,
        (map_id::M3_2, _) => Route3Stage2,
        (map_id::M3_3, _) => Route3Stage3,
        (map_id::M3_4, _) => Route3Stage4,
        (map_id::M3_5, _) => Route3Stage5,
        (map_id::M3_6, _) => Route3Stage6,
        (map_id::M3_7, 0) => Route3Stage7,
        (map_id::M3_7, 1) => FinalBattle,
        (map_id::FINAL, _) => FinalBattle,
        _ => return None,
    })
}

/// Opening background selected by each map's `initlevel` source macro.
///
/// A few builders intentionally omit the common wrapper bytes because their
/// byte fixtures were captured from an earlier extraction. Gameplay still
/// needs the source background state before the first map instruction, so the
/// shell applies this typed catalog value at load time. IDs are the flat
/// background catalog used by the Rust map data.
pub fn opening_background(id: u32) -> Option<u16> {
    match id {
        map_id::M1_1 | map_id::M2_1 => Some(4),
        map_id::M1_2 => Some(5),
        map_id::M1_3 => Some(6),
        map_id::M1_4 => Some(13),
        map_id::M1_5 | map_id::M2_5 => Some(14),
        map_id::M1_6 => Some(15),
        map_id::M2_2 => Some(22),
        map_id::M2_3 => Some(23),
        map_id::M2_4 => Some(26),
        map_id::M2_6 => Some(27),
        map_id::M3_1 => Some(3),
        map_id::M3_2 => Some(30),
        map_id::M3_3 => Some(31),
        map_id::M3_4 => Some(33),
        map_id::M3_5 => Some(36),
        map_id::M3_6 => Some(37),
        map_id::M3_7 => Some(38),
        map_id::BLACKHOLE => Some(39),
        map_id::SPECIAL => Some(62),
        map_id::FINAL => Some(17),
        map_id::INTRO => Some(40),
        map_id::TITLE => Some(41),
        map_id::CONTINUE => Some(42),
        map_id::CREDITS => Some(43),
        map_id::TRAINING => Some(44),
        map_id::NONE | map_id::WAIT | map_id::PLANET => None,
        _ => None,
    }
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

/// `Levels_GetMapData` equivalent for the complete retail catalog.
///
/// Differences from C (documented, intentional):
/// - C rebuilds callback registrations on every call; here the
///   registration lists are part of [`BuiltLevel`] and the caller (map VM
///   loader) applies them. For `MAP_ID_WAIT` the C code returns the title
///   blob WITHOUT registering title callbacks; the wait entry point
///   (`title.waitmap` label) never reaches the CODE65816 hook, so using
///   the shared registration list is behavior-identical.
/// - Unknown ids return `None` instead of a warn-once END stub.
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
) -> Option<(
    &'static [(u32, &'static str)],
    &'static [(u16, &'static str)],
)> {
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
