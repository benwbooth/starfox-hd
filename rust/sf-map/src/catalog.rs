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
use sf_core::player_view::{PlayerViewMode, PlayerViewOptions};
use sf_core::screen_wipe::ScreenWipeKind;

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

/// Flat background catalog identities from BGS.ASM / the map bytecode.
pub mod background_id {
    pub const THREE_ONE_OUTDOOR: u16 = 3;
    pub const ONE_ONE_OUTDOOR: u16 = 4;
    pub const ONE_TWO: u16 = 5;
    pub const ONE_THREE_WARP: u16 = 6;
    pub const ONE_THREE_SPACE: u16 = 7;
    pub const ONE_THREE_TUNNEL: u16 = 8;
    pub const ONE_THREE_SPACE_RETURN: u16 = 9;
    pub const ONE_THREE_CLEAR: u16 = 12;
    pub const ONE_FOUR: u16 = 13;
    pub const ONE_FIVE: u16 = 14;
    pub const ONE_SIX_DIVE: u16 = 15;
    pub const ONE_SIX_TUNNEL: u16 = 16;
    pub const ONE_SIX_FINAL: u16 = 17;
    pub const TWO_TWO: u16 = 22;
    pub const TWO_THREE_PLANET: u16 = 23;
    pub const TWO_THREE_BRIDGE: u16 = 24;
    pub const TWO_THREE_TUNNEL: u16 = 25;
    pub const TWO_FOUR: u16 = 26;
    pub const TWO_SIX_COLONY: u16 = 27;
    pub const TWO_SIX_CLEAR: u16 = 28;
    pub const TWO_SIX_TUNNEL: u16 = 29;
    pub const THREE_TWO: u16 = 30;
    pub const THREE_THREE: u16 = 31;
    pub const THREE_FOUR_SPACE: u16 = 33;
    pub const THREE_FOUR_TUNNEL: u16 = 34;
    pub const THREE_FOUR_CLEAR: u16 = 35;
    pub const THREE_FIVE: u16 = 36;
    pub const THREE_SIX: u16 = 37;
    pub const THREE_SEVEN: u16 = 38;
    pub const BLACK_HOLE: u16 = 39;
    pub const INTRO: u16 = 40;
    pub const TITLE: u16 = 41;
    pub const CONTINUE: u16 = 42;
    pub const CREDITS: u16 = 43;
    pub const TRAINING: u16 = 44;
    pub const SPECIAL: u16 = 62;
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
    use background_id as bg;

    match id {
        map_id::M1_1 | map_id::M2_1 => Some(bg::ONE_ONE_OUTDOOR),
        map_id::M1_2 => Some(bg::ONE_TWO),
        map_id::M1_3 => Some(bg::ONE_THREE_WARP),
        map_id::M1_4 => Some(bg::ONE_FOUR),
        map_id::M1_5 | map_id::M2_5 => Some(bg::ONE_FIVE),
        map_id::M1_6 => Some(bg::ONE_SIX_DIVE),
        map_id::M2_2 => Some(bg::TWO_TWO),
        map_id::M2_3 => Some(bg::TWO_THREE_PLANET),
        map_id::M2_4 => Some(bg::TWO_FOUR),
        map_id::M2_6 => Some(bg::TWO_SIX_COLONY),
        map_id::M3_1 => Some(bg::THREE_ONE_OUTDOOR),
        map_id::M3_2 => Some(bg::THREE_TWO),
        map_id::M3_3 => Some(bg::THREE_THREE),
        map_id::M3_4 => Some(bg::THREE_FOUR_SPACE),
        map_id::M3_5 => Some(bg::THREE_FIVE),
        map_id::M3_6 => Some(bg::THREE_SIX),
        map_id::M3_7 => Some(bg::THREE_SEVEN),
        map_id::BLACKHOLE => Some(bg::BLACK_HOLE),
        map_id::SPECIAL => Some(bg::SPECIAL),
        map_id::FINAL => Some(bg::ONE_SIX_FINAL),
        map_id::INTRO => Some(bg::INTRO),
        map_id::TITLE => Some(bg::TITLE),
        map_id::CONTINUE => Some(bg::CONTINUE),
        map_id::CREDITS => Some(bg::CREDITS),
        map_id::TRAINING => Some(bg::TRAINING),
        map_id::NONE | map_id::WAIT | map_id::PLANET => None,
        _ => None,
    }
}

/// Player-view declaration attached to a map's opening background `pstrat`.
///
/// This is the typed counterpart of the `flymode` and `max fly mode`
/// arguments in BGS.ASM. Backgrounds without those arguments intentionally
/// preserve the preceding declaration and therefore do not appear here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningPlayerView {
    pub mode: PlayerViewMode,
    pub options: PlayerViewOptions,
}

/// Exact opening `pstrat` player-view declaration for every playable map.
pub fn opening_player_view(id: u32) -> Option<OpeningPlayerView> {
    use PlayerViewMode::{CloseExterior, Exterior};
    use PlayerViewOptions::{ExteriorAndCockpit, ExteriorViews};

    let (mode, options) = match id {
        map_id::M1_1 | map_id::M2_1 | map_id::M3_1 | map_id::TRAINING => (Exterior, ExteriorViews),
        map_id::M1_2
        | map_id::M1_5
        | map_id::M2_2
        | map_id::M3_2
        | map_id::M3_4
        | map_id::M3_6
        | map_id::SPECIAL
        | map_id::CREDITS
        | map_id::INTRO
        | map_id::TITLE => (CloseExterior, ExteriorAndCockpit),
        map_id::M1_3
        | map_id::M1_6
        | map_id::M2_3
        | map_id::M2_6
        | map_id::M3_7
        | map_id::FINAL
        | map_id::CONTINUE => (Exterior, ExteriorViews),
        map_id::M1_4 | map_id::M3_3 | map_id::M3_5 => (CloseExterior, ExteriorViews),
        map_id::M2_4 | map_id::M2_5 | map_id::BLACKHOLE => (Exterior, ExteriorAndCockpit),
        _ => return None,
    };
    Some(OpeningPlayerView { mode, options })
}

/// View declaration installed when a runtime `setbg` executes a BGS.ASM
/// background carrying `pstrat` mode arguments.
///
/// Corneria's outdoor backgrounds (3/4) and the parameterless clear/credits
/// helpers deliberately return `None`: those source blocks preserve the
/// declaration that was already active.
pub fn background_player_view(background: u16) -> Option<OpeningPlayerView> {
    use background_id as bg;
    use PlayerViewMode::{CloseExterior, Exterior};
    use PlayerViewOptions::{ExteriorAndCockpit, ExteriorViews};

    let (mode, options) = match background {
        // Exterior-only-cycle background declarations.
        bg::ONE_THREE_WARP
        | bg::ONE_THREE_TUNNEL
        | bg::ONE_SIX_DIVE
        | bg::ONE_SIX_TUNNEL
        | bg::ONE_SIX_FINAL
        | bg::TWO_THREE_PLANET
        | bg::TWO_THREE_BRIDGE
        | bg::TWO_THREE_TUNNEL
        | bg::TWO_SIX_COLONY
        | bg::TWO_SIX_CLEAR
        | bg::TWO_SIX_TUNNEL
        | bg::THREE_FOUR_TUNNEL
        | bg::THREE_SEVEN
        | bg::CONTINUE
        | bg::TRAINING => (Exterior, ExteriorViews),
        // Exterior/cockpit-cycle declarations beginning outside.
        bg::ONE_THREE_SPACE | bg::ONE_THREE_SPACE_RETURN | bg::TWO_FOUR | bg::BLACK_HOLE => {
            (Exterior, ExteriorAndCockpit)
        }
        // Exterior-only-cycle declarations beginning at the close distance.
        bg::ONE_FOUR | bg::THREE_THREE | bg::THREE_FIVE => (CloseExterior, ExteriorViews),
        // Exterior/cockpit-cycle declarations beginning at the close distance.
        bg::ONE_TWO
        | bg::ONE_FIVE
        | bg::TWO_TWO
        | bg::THREE_TWO
        | bg::THREE_FOUR_SPACE
        | bg::THREE_SIX
        | bg::INTRO
        | bg::TITLE
        | bg::CREDITS
        | bg::SPECIAL => (CloseExterior, ExteriorAndCockpit),
        _ => return None,
    };
    Some(OpeningPlayerView { mode, options })
}

/// Source-authored screen transitions surrounding a map's opening.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpeningWipePlan {
    /// Reveal performed by the common `initlevel` wrapper before map content.
    pub initial: Option<ScreenWipeKind>,
    /// Reveal requested by a later explicit `wipein` in the map body.
    pub on_init_black: Option<ScreenWipeKind>,
}

/// Exact `initlevel` / opening `wipein` assignment from `MAPS/*.ASM`.
///
/// The three Corneria routes first use `mstarwipe` around the common level
/// initializer, then `mscramwipe` when the launch corridor hands control to
/// the outdoor scene. Other listed maps have a single initializer wipe.
pub fn opening_wipe_plan(id: u32) -> OpeningWipePlan {
    use ScreenWipeKind::{HorizontalReveal, StarReveal};

    match id {
        map_id::M1_1 | map_id::M2_1 | map_id::M3_1 => OpeningWipePlan {
            initial: Some(StarReveal),
            on_init_black: Some(HorizontalReveal),
        },
        map_id::M1_2
        | map_id::M1_5
        | map_id::M2_2
        | map_id::M2_4
        | map_id::M2_5
        | map_id::M3_2
        | map_id::M3_4
        | map_id::M3_6
        | map_id::TRAINING => OpeningWipePlan {
            initial: Some(StarReveal),
            on_init_black: None,
        },
        map_id::M1_4 | map_id::M2_3 | map_id::M3_3 | map_id::M3_5 => OpeningWipePlan {
            initial: Some(HorizontalReveal),
            on_init_black: None,
        },
        _ => OpeningWipePlan::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::player_view::{PlayerViewMode as Mode, PlayerViewOptions as Options};

    #[test]
    fn every_source_background_with_a_spawned_player_has_a_view_declaration() {
        for map in (map_id::M1_1..=map_id::CONTINUE).chain([map_id::CREDITS, map_id::TRAINING]) {
            assert!(
                opening_player_view(map).is_some(),
                "missing opening player view for map {map}"
            );
        }
        assert_eq!(opening_player_view(map_id::NONE), None);
        assert_eq!(opening_player_view(map_id::WAIT), None);
        assert_eq!(opening_player_view(map_id::PLANET), None);
    }

    #[test]
    fn representative_declarations_match_bgs_source() {
        let source = include_str!("../../../reference/ultrastarfox/SF/ASM/BGS.ASM");
        for declaration in [
            "pstrat\tplayeropening,a,ab",
            "pstrat\tplayerinsidespaceflyin,b,abc",
            "pstrat\tplayerspaceflyin,a,abc",
            "pstrat\tplayerplanetflyin,b,ab",
            "pstrat\tplayercolonyflyin,a,ab",
            "pstrat\tplayerundergnd,b,ab",
        ] {
            assert!(
                source.contains(declaration),
                "missing BGS declaration: {declaration}"
            );
        }

        assert_eq!(
            opening_player_view(map_id::M1_2),
            Some(OpeningPlayerView {
                mode: Mode::CloseExterior,
                options: Options::ExteriorAndCockpit,
            })
        );
        assert_eq!(
            opening_player_view(map_id::M2_4),
            Some(OpeningPlayerView {
                mode: Mode::Exterior,
                options: Options::ExteriorAndCockpit,
            })
        );
        assert_eq!(
            opening_player_view(map_id::M3_5),
            Some(OpeningPlayerView {
                mode: Mode::CloseExterior,
                options: Options::ExteriorViews,
            })
        );

        assert_eq!(
            background_player_view(background_id::THREE_FOUR_TUNNEL),
            Some(OpeningPlayerView {
                mode: Mode::Exterior,
                options: Options::ExteriorViews,
            })
        );
        assert_eq!(
            background_player_view(background_id::THREE_FOUR_CLEAR),
            None
        );
    }
}
