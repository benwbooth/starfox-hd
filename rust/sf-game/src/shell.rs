//! Game-side "shell" glue: boot state machine + frame-input producers.
//!
//! C oracle:
//! - `src/game/boot.c` — `Game_Init` (boot.c:109), `Game_Tick` state switch
//!   (boot.c:222), `TitleScreen_Tick` (boot.c:141),
//!   `Game_BeginGameplayFromPlanetSelect` (boot.c:52),
//!   `Gameplay_ProgressTick` (boot.c:170), the draw-list store
//!   (boot.c:46-47, 287-303).
//! - `src/game/nmi.c` — `Nmi_GameTick` PLAYING-state tick ordering
//!   (nmi.c:49).
//! - `src/sf_rtl.c:142` — `SfRtl_BeginFrame` pad edge semantics.
//!
//! The shell owns the systems that C kept as globals around the game core:
//! [`crate::windows::Windows`], [`crate::strings::Strings`], the sound
//! command queue, [`crate::planets::Planets`], and
//! [`crate::camera::GameCamera`]. Windows/Strings/sounds are shared with
//! the map VM through [`ShellHooks`] (an `Rc<RefCell<..>>`), matching the C
//! direct calls from world.c/levels.c into windows.c/strings.c/sound.c.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use sf_core::{
    pad,
    player_view::{PlayerViewMode, PlayerViewOptions},
    scene::{PaletteFadeTarget, SceneStyle},
    screen_fill_circle::{ScreenFillCircleCenter, ScreenFillCircleState},
    screen_wipe::{ScreenWipeKind, ScreenWipeState},
    sf1_controls::{BriefingChoice, BriefingPhase, ControlType},
    sf1_planets::{
        briefing_text, planet_heading, planet_zoom_step, post_tally_travel_retail_frames,
        PlanetPresentation, PlanetSequencePhase, Sf1Planet, BRIEFING_DISMISS_HANDOFF_TICKS,
        BRIEFING_FAST_CADENCE_DENOMINATOR, BRIEFING_FAST_CADENCE_INITIAL_PROGRESS,
        BRIEFING_FAST_CADENCE_NUMERATOR, BRIEFING_FAST_CURSOR_LIMIT, BRIEFING_PREPARATION_TICKS,
        BRIEFING_SETTLED_CADENCE_DENOMINATOR, BRIEFING_SETTLED_CADENCE_NUMERATOR,
        FINAL_PLANET_RADIUS, INITIAL_ROUTE_MAP_SETUP_TICKS, MAP_FADE_STEPS, MAP_FADE_TICKS,
        PLANET_CENTER_TICKS, PLANET_EXIT_TICKS, PLANET_ISOLATION_TICKS,
        PLANET_NAME_CHARACTER_TICKS, PLANET_NAME_TERMINATION_TICKS, PLANET_ZOOM_TICKS,
        RETAIL_VIDEO_FRAMES_PER_GAME_TICK, SHIP_FLASH_TICKS,
    },
    DrawListEntry,
};

use crate::camera::GameCamera;
use crate::charmap::CharMap;
use crate::game::{Game, Hooks, PosSndFamilyId};
use crate::obj::Objects;
use crate::planets::{Planets, RouteSelectionResult, DEFAULT_LIVES};
use crate::score;
use crate::strings::Strings;
use crate::vars::{
    BossEncounter, GameVars, GF_PLAYERDEAD, PFM_SHADOWS, PLAYER_DEATH_FADE_DELAY_TICKS,
    PSF2_PLAYERHP0, PSF3_ENGINESND, PSF_STAGE_DAMAGE, PSTF_NOTDIE, SPACE_MODE, STAY_BLACK_INACTIVE,
};
use crate::windows::{MapFadeRate, Windows, BLACK_FADE_MAX};
use crate::world::World;
use crate::{bgs, draw};

/// C `LEVEL_CLEAR_SETTLE_TICKS` (src/game/boot.c:39) — 3 s after mapend
/// before the stage advance.
pub const LEVEL_CLEAR_SETTLE_TICKS: i32 = 60;
/// One retail `printspeclp` display step spans roughly two 20 Hz native-port
/// ticks (independent Mesen Rev 2 oracle: 5-6 video frames per loop).
pub const TALLY_DISPLAY_STEP_TICKS: u8 = 2;
/// ROM `cla1 += 3` graph step (MAIN.ASM:1191-1201).
pub const TALLY_PERCENT_STEP: u8 = 3;
/// ROM `clam = 20` delay between reaching the target and committing it.
pub const TALLY_COMMIT_DELAY_STEPS: u8 = 20;
/// ROM `clb1 = 20` delay before a newly crossed bonus is announced.
pub const TALLY_BONUS_DELAY_STEPS: u8 = 20;
/// ROM `plotx1 = 10`; the credit increments after nine visible decrements.
pub const TALLY_BONUS_AWARD_STEPS: u8 = 9;
/// Native unattended safeguard once every retail tally operation is complete.
pub const TALLY_READY_AUTO_TICKS: u16 = 60;
/// Terminal explosion frame plus the retail delay and the native unit-speed
/// black fade before respawn/game-over dispatch.
pub const DEATH_RESPAWN_TICKS: i32 =
    1 + PLAYER_DEATH_FADE_DELAY_TICKS as i32 + BLACK_FADE_MAX as i32;
/// `wipein` holds black for `mapwait 300`; the no-player map lane advances
/// 65 source distance units per 20 Hz update, so the reveal starts after five
/// updates (300 / 65 rounded up).
pub const OPENING_WIPE_BLACK_HOLD_TICKS: u8 = 5;

/// Retail reset-to-attract handoff measured from the full-machine oracle.
/// Boot remains active for 43 complete native 20 Hz ticks; the attract state
/// becomes active on the following tick (video frame 132 in the oracle's
/// zero-based sampled timeline). Input during this interval is ignored.
pub const BOOT_TO_ATTRACT_DELAY_TICKS: u16 = 43;

/// ENDSEQ title hold before the unattended attract intro begins.
pub const TITLE_ATTRACT_DURATION_TICKS: u16 = 880;
/// ENDSEQ ignores START until the title has been active for this many ticks.
pub const TITLE_INPUT_DELAY_TICKS: u16 = 40;
/// Fixed-rate native ticks before the retail title presentation is ready to
/// accept the same START edge. The full-machine oracle reaches ENDSEQ's
/// authored 40-frame gate 65 sampled 20 Hz ticks after title entry because
/// the original presentation workload advances its game frame unevenly.
pub const TITLE_PRESENTATION_INPUT_READY_TICKS: u16 = 65;
/// ENDSEQ ignores an intro skip until this many strategy ticks have elapsed.
pub const INTRO_INPUT_DELAY_TICKS: u16 = 30;
/// ENDSEQ seeds the intro exit fade at this intensity.
pub const INTRO_EXIT_FADE_START: u8 = 11;
/// Retail-observed black presentation between the title fade completing and
/// the controller screen becoming the active background.
pub const TITLE_TO_BRIEFING_BLACK_HOLD_TICKS: u16 = 22;
/// Native sound-catalog identity loaded by both attract-intro entry points.
pub const MUSIC_ATTRACT_INTRO: u8 = 1;
/// Driver cue used while leaving the title.
pub const MUSIC_FADE_OUT: u8 = 241;
/// Native sound-catalog identity used by the controller/training screen.
pub const MUSIC_CONTROLLER_SCREEN: u8 = 3;
/// CONT.ASM ignores the first controller-screen START edges until this tick.
pub const BRIEFING_INPUT_DELAY_TICKS: u16 = 16;
/// The controller screen's CPU-driven normal fade completes in this many
/// sampled 20 Hz port ticks while retaining the source normal fade mode.
pub const BRIEFING_FADE_TICKS: u8 = 16;
/// Controller/destination selection movement cue.
pub const BRIEFING_MOVE_SOUND: u8 = 17;
/// Controller/destination confirmation cue.
pub const BRIEFING_CONFIRM_SOUND: u8 = 16;
/// CONT.ASM lets the training scene run for this many ticks before START exits.
pub const TRAINING_INPUT_DELAY_TICKS: u16 = 20;
/// Controller-screen source loadout.
const BRIEFING_SPECIAL_WEAPON_COUNT: u16 = 3;
/// Training mode always starts with the source single life.
const TRAINING_LIVES: u8 = 1;
/// Source `gf2_ingame` bit used by the training exit guard.
const GAME_FLAG2_INGAME: u8 = 1;

/// Route-map music package.
pub const MUSIC_PLANET_MAP: u8 = 1;
/// Spherical-planet close-up music package.
pub const MUSIC_PLANET_ZOOM: u8 = 11;
/// Flat sector/asteroid close-up music package.
pub const MUSIC_PLANET_ZOOM_SHORT: u8 = 13;
/// Route-map confirmation effect.
pub const PLANET_CONFIRM_SOUND: u8 = 16;
const ROUTE_CONFIRM_BUTTONS: u16 = pad::START | pad::A | pad::B;
const ROUTE_CONFIRMATION_HANDOFF_TICKS: u8 = 2;
/// General Pepper type-on effect.
pub const PEPPER_CHARACTER_SOUND: u8 = 137;
/// General Pepper dismissal effect.
pub const PEPPER_DISMISS_SOUND: u8 = 19;

/// Staff-roll music package in the native audio catalog. The source ending
/// switches to this package immediately before loading the credits map.
pub const MUSIC_STAFF_ROLL: u8 = 32;
/// End-sequence sound package. Its catalog start cue is the source recap song.
pub const MUSIC_END_SEQUENCE: u8 = 31;
/// Source circular wipe duration between replay entries and after the last.
pub const ENDING_REPLAY_TRANSITION_TICKS: u8 = 37;
/// The source begins bringing the detail panel in below this remaining count.
pub const ENDING_REPLAY_SCROLL_TICKS: u16 = 110;
/// The detail text becomes visible at this exact remaining count.
pub const ENDING_REPLAY_DETAILS_TICK: u16 = 100;
/// Source interval between successive stage-score rows.
pub const ENDING_STAGE_ROW_TICKS: u8 = 30;
/// Source wrapped countdown after the final stage row before the summary.
pub const ENDING_STAGE_FINISH_TICKS: u8 = 85;
/// Delay between score-summary label/value groups.
pub const ENDING_SUMMARY_REVEAL_TICKS: u8 = 25;
/// Hold after the pre-recap score summary.
pub const ENDING_SUMMARY_HOLD_TICKS: u8 = 80;
/// Source `400-30` transfer interval before the boss recap.
pub const ENDING_SUMMARY_FADE_TICKS: u16 = 370;
/// Delay between each final-score row/value reveal.
pub const FINAL_SCORE_REVEAL_TICKS: u8 = 10;

/// C `NMI_PLAYER_MAX_HP` (src/game/nmi.c:47) — player body hit points.
pub const NMI_PLAYER_MAX_HP: i32 = 40;

/// ROM `LE_*` level-end codes (`reference/ultrastarfox/SF/INC/KALCS.INC:91-103`).
///
/// The map VM's `mapend(N)` stores one of these into
/// [`World::levelfinished`](crate::world::World::levelfinished);
/// [`Shell::gameplay_progress_tick`] dispatches on the value exactly like ROM
/// `MAIN.ASM:222-322` — normal clears run the tally then advance the route,
/// while the warp codes (11-16) skip the tally and walk straight into the
/// black-hole / special stage (ROM `enterbhole`/`exittobhole*`/`exittospecial`
/// jump to `planetseq_l`, bypassing `end_level_seq`).
pub mod le {
    /// Still playing (mapend has not run).
    pub const PLAYING: u8 = 0;
    /// Normal level clear (`mapend` no-arg, MAPMACS.INC:274) — tally + advance.
    pub const NORMAL: u8 = 1;
    /// Fade to white then planetseq (tally + white fade + advance).
    pub const FADETOWHITE: u8 = 4;
    /// Fade down then planetseq (no tally).
    pub const FADEDOWN: u8 = 5;
    /// End-of-game credits path (`end_game_seq`).
    pub const ENDOFGAME: u8 = 6;
    /// Venom-surface hand-off (`mapend__not`, MAPMACS.INC:1990).
    pub const STARTGAME: u8 = 7;
    /// End of credits.
    pub const ENDOFCREDS: u8 = 8;
    /// Total-score screen.
    pub const ENDTOTALSCORE: u8 = 9;
    /// GAME OVER (ROM handles this first, with NO `inc stage`, MAIN.ASM:226).
    pub const GAMEOVER: u8 = 10;
    /// Black-hole exit -> Venom 1 Orbital (routechange bhole1 -> routes[3]=P19).
    pub const BHOLE1: u8 = 11;
    /// Black-hole exit -> Sector Y (routechange bhole2 -> routes[3]=P18).
    pub const BHOLE2: u8 = 12;
    /// Black-hole exit -> Sector Z (routechange bhole3 -> routes[3]=P20).
    pub const BHOLE3: u8 = 13;
    /// Special-stage route change (routechange 1 -> routes[0]=P22 + nebula_on).
    pub const SPECIAL: u8 = 14;
    /// Enter the BLACK HOLE stage (via the P21 branch, armed upstream).
    pub const ENTERBHOLE: u8 = 15;
    /// Enter the SPECIAL stage ("Out of This Dimension", map SPECIAL/planet 14).
    pub const ENTERSPEC: u8 = 16;
}

/// C `GameState` (src/game/boot.h:17-25), same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Boot,
    /// Retail boot/title attract presentation driven by `INTRO.ASM`.
    AttractIntro,
    Title,
    Briefing,
    PlanetSelect,
    Playing,
    Continue,
    Ending,
    /// End-of-level tally screen (ROM `end_level_seq`, MAIN.ASM:1077-1160):
    /// shows the stage hit % + running total and awards bonus credits before
    /// advancing to the next stage / route select.
    Tally,
}

/// Semantic state of the source level-start handoff inside [`GameState::Playing`].
/// The retail machine spends several display frames completing its first
/// transfer before the opening level update becomes presentation-ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameplayEntryPhase {
    #[default]
    Inactive,
    LevelInitialization,
    ActiveLevel,
}

/// Measured 20 Hz ticks from the retail `gamestart` entry to the first active
/// opening-level update boundary.
pub const LEVEL_INITIALIZATION_TICKS: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TallyPhase {
    Counting,
    CommitDelay { steps_remaining: u8 },
    BonusDelay { steps_remaining: u8 },
    BonusAward { steps_remaining: u8 },
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttractDestination {
    Intro,
    Title,
    Briefing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AttractSequence {
    level_loaded: bool,
    phase_ticks: u16,
    fade_destination: Option<AttractDestination>,
    handoff_ticks_remaining: Option<u16>,
}

impl Default for AttractSequence {
    fn default() -> Self {
        Self {
            level_loaded: false,
            phase_ticks: 0,
            fade_destination: None,
            handoff_ticks_remaining: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BriefingFadeDestination {
    Training,
    PlanetSelect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BriefingSequence {
    level_loaded: bool,
    phase: BriefingPhase,
    choice: BriefingChoice,
    control_type: ControlType,
    fade_destination: Option<BriefingFadeDestination>,
    control_confirmation_pending: bool,
}

impl Default for BriefingSequence {
    fn default() -> Self {
        Self {
            level_loaded: false,
            phase: BriefingPhase::ControlType,
            choice: BriefingChoice::Training,
            control_type: ControlType::A,
            fade_destination: None,
            control_confirmation_pending: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TrainingSequence {
    returning_to_briefing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TallyState {
    target_percent: u8,
    current_percent: u8,
    teammate_shields: [u8; 3],
    phase: TallyPhase,
    display_tick: u8,
    ready_ticks: u16,
    bonus_visible: bool,
}

/// Post-campaign presentation after the boss replay has handed off to the
/// retail credits map. Kept as semantic flat state rather than reusing any of
/// the source machine's scratch storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndingPhase {
    ScoreParade,
    ScoreSummary,
    ScoreHold,
    ScoreFade,
    BossReplay,
    BossTransition,
    StaffRoll,
    FinalScore,
}

/// The two source background/palette selections used by every boss recap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndingReplayBackdrop {
    RisingGradient,
    SplitGradient,
}

/// Exact English text attached to one semantic boss recap. Strings remain
/// ordinary typed presentation data; the renderer owns glyph rasterization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndingReplayText {
    pub title: &'static str,
    pub subtitle: Option<&'static str>,
    pub location: Option<&'static str>,
    pub location_second_line: Option<&'static str>,
    pub details: [&'static str; 3],
}

/// Source timing and camera setup for one recorded boss recap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndingReplaySpec {
    pub duration_ticks: u16,
    pub warmup_ticks: u16,
    pub initial_view_distance: i16,
    pub target_view_distance: i16,
    pub view_height: i16,
    pub backdrop: EndingReplayBackdrop,
}

/// Fixed, inspectable presentation state consumed by the native renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndingReplayPresentation {
    pub encounter: BossEncounter,
    pub backdrop: EndingReplayBackdrop,
    pub text: EndingReplayText,
    /// Number of detail characters already emitted. The retail NMI writes one
    /// printable character per game tick after the countdown crosses 100.
    pub detail_characters_visible: u8,
}

/// Exact handler constants from `ENDSEQ.ASM` for every semantic encounter.
pub fn ending_replay_spec(encounter: BossEncounter) -> EndingReplaySpec {
    use BossEncounter::*;
    use EndingReplayBackdrop::{RisingGradient, SplitGradient};

    match encounter {
        Route1Stage1 | Route2Stage1 => EndingReplaySpec {
            duration_ticks: 180,
            warmup_ticks: 200,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: RisingGradient,
        },
        Route1Stage2 => EndingReplaySpec {
            duration_ticks: 195,
            warmup_ticks: 0,
            initial_view_distance: 1_000,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route2Stage2 => EndingReplaySpec {
            duration_ticks: 195,
            warmup_ticks: 0,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route1Stage3 | Route3Stage4 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 0,
            initial_view_distance: 1_500,
            target_view_distance: 1_500,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route1Stage4 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 230,
            initial_view_distance: 2_500,
            target_view_distance: 2_300,
            view_height: 200,
            backdrop: RisingGradient,
        },
        Route1Stage5 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 40,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route1Stage6 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 0,
            initial_view_distance: 1_000,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route2Stage3 => EndingReplaySpec {
            duration_ticks: 180,
            warmup_ticks: 0,
            initial_view_distance: 200,
            target_view_distance: 400,
            view_height: 0,
            backdrop: RisingGradient,
        },
        Route2Stage4 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 0,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route2Stage5 => EndingReplaySpec {
            duration_ticks: 220,
            warmup_ticks: 0,
            initial_view_distance: 1_000,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route2Stage6 => EndingReplaySpec {
            duration_ticks: 220,
            warmup_ticks: 50,
            initial_view_distance: 500,
            target_view_distance: 1_000,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route3Stage1 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 230,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: -400,
            backdrop: RisingGradient,
        },
        Route3Stage2 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 50,
            initial_view_distance: 1_000,
            target_view_distance: 2_300,
            view_height: 0,
            backdrop: SplitGradient,
        },
        Route3Stage3 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 0,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: -300,
            backdrop: RisingGradient,
        },
        Route3Stage5 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 200,
            initial_view_distance: 1_000,
            target_view_distance: 2_300,
            view_height: -300,
            backdrop: SplitGradient,
        },
        Route3Stage6 => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 300,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: -200,
            backdrop: SplitGradient,
        },
        Route3Stage7 => EndingReplaySpec {
            duration_ticks: 250,
            warmup_ticks: 50,
            initial_view_distance: 1_500,
            target_view_distance: 2_300,
            view_height: -300,
            backdrop: SplitGradient,
        },
        FinalBattle => EndingReplaySpec {
            duration_ticks: 200,
            warmup_ticks: 200,
            initial_view_distance: 2_000,
            target_view_distance: 2_300,
            view_height: -300,
            backdrop: SplitGradient,
        },
    }
}

/// Exact `ENDSEQ.ASM` strings for every route/stage identity.
pub fn ending_replay_text(encounter: BossEncounter) -> EndingReplayText {
    use BossEncounter::*;

    let (title, location, location_second_line, details) = match encounter {
        Route1Stage1 => (
            "LEVEL 1",
            Some("CORNERIA"),
            None,
            [
                "NAME   - ATTACK CARRIER",
                "WEAPON - MISSILE BLASTER",
                "SIZE   - H70*W100*D150",
            ],
        ),
        Route2Stage1 => (
            "LEVEL 2",
            Some("CORNERIA"),
            None,
            [
                "NAME   - ATTACK CARRIER",
                "WEAPON - MISSILE BLASTER",
                "SIZE   - H70*W100*D150",
            ],
        ),
        Route3Stage1 => (
            "LEVEL 3",
            Some("CORNERIA"),
            None,
            [
                "NAME   - DESTRUCTOR",
                "WEAPON - PLASMA",
                "SIZE   - H45*W150*D90",
            ],
        ),
        Route1Stage2 => (
            "LEVEL 1",
            Some("ASTEROID"),
            None,
            [
                "NAME   - ROCK CRUSHER",
                "WEAPON - LASER",
                "SIZE   - H60*W86*D45",
            ],
        ),
        Route2Stage2 => (
            "LEVEL 2",
            Some("SECTOR %"),
            None,
            [
                "NAME   - ROCK CRUSHER",
                "WEAPON - LASER",
                "SIZE   - H60*W86*D45",
            ],
        ),
        Route3Stage2 => (
            "LEVEL 3",
            Some("ASTEROID"),
            None,
            [
                "NAME   - BLADE BARRIER",
                "WEAPON - WEB ATTACK",
                "SIZE   - H90*W90*D65",
            ],
        ),
        Route1Stage3 => (
            "LEVEL 1",
            Some("SPACE"),
            Some("ARMADA"),
            [
                "NAME   - ATOMIC BASE",
                "WEAPON - LASER",
                "SIZE   - H600*W850*D1200",
            ],
        ),
        Route2Stage3 => (
            "LEVEL 2",
            Some("TITANIA"),
            None,
            [
                "NAME   - PROFESSOR HANGER",
                "WEAPON - SHADOW THRUSTER",
                "SIZE   - H25*W18*D30",
            ],
        ),
        Route3Stage3 => (
            "LEVEL 3",
            Some("FORTUNA"),
            None,
            [
                "NAME   - MONARCH DODORA",
                "WEAPON - FIRE BREATH",
                "SIZE   - H85*W160*D200",
            ],
        ),
        Route1Stage4 => (
            "LEVEL 1",
            Some("METEOR"),
            None,
            [
                "NAME   - DANCING INSECTOR",
                "WEAPON - FIRE BLASTER",
                "SIZE   - H120*W87*D72",
            ],
        ),
        Route2Stage4 => (
            "LEVEL 2",
            Some("SECTOR $"),
            None,
            [
                "NAME   - PLASMA HYDRA",
                "WEAPON - PLASMA SPEWER",
                "SIZE   - H96*W280*D55",
            ],
        ),
        Route3Stage4 => (
            "LEVEL 3",
            Some("SECTOR #"),
            None,
            [
                "NAME   - ATOMIC BASE II",
                "WEAPON - LASER",
                "SIZE   - H92*W90*D1100",
            ],
        ),
        Route1Stage5 => (
            "LEVEL 1",
            Some("VENOM"),
            None,
            [
                "NAME   - PHANTRON",
                "WEAPON - LASER",
                "SIZE   - H25*W22*D31",
            ],
        ),
        Route2Stage5 => (
            "LEVEL 2",
            Some("VENOM"),
            None,
            [
                "NAME   - METAL SMASHER",
                "WEAPON - CRUSH ATTACK",
                "SIZE   - H17*W20*D38",
            ],
        ),
        Route3Stage5 => (
            "LEVEL 3",
            Some("MACBETH"),
            None,
            [
                "NAME   - SPINNING CORE",
                "WEAPON - LASER",
                "SIZE   - H63*W52*D45",
            ],
        ),
        // The retail Route 1 stage 6 record deliberately reuses
        // `boss15txt2`, including its PHANTRON identification.
        Route1Stage6 => (
            "LEVEL 1",
            Some("VENOM"),
            None,
            [
                "NAME   - PHANTRON",
                "WEAPON - LASER",
                "SIZE   - H25*W22*D31",
            ],
        ),
        Route2Stage6 => (
            "LEVEL 2",
            Some("VENOM"),
            None,
            [
                "NAME   - GALACTIC RIDER",
                "WEAPON - AIR BIKERS",
                "SIZE   - H80*W61*D25",
            ],
        ),
        Route3Stage6 => (
            "LEVEL 3",
            Some("VENOM"),
            None,
            [
                "NAME   - GREAT COMMANDER",
                "WEAPON - LASER",
                "SIZE   - H73*W97*D250",
            ],
        ),
        Route3Stage7 => (
            "LEVEL 3",
            Some("VENOM"),
            None,
            [
                "NAME   - GREAT COMMANDER",
                "WEAPON - IRON BALLS",
                "SIZE   - H73*W97*D250",
            ],
        ),
        FinalBattle => {
            return EndingReplayText {
                title: "FINAL",
                subtitle: Some("STAGE"),
                location: None,
                location_second_line: None,
                details: [
                    "NAME   - ANDROSS...",
                    "WEAPON - TELEKINESIS",
                    "SIZE   - H100*W80*D30",
                ],
            };
        }
    };

    EndingReplayText {
        title,
        subtitle: None,
        location,
        location_second_line,
        details,
    }
}

/// Semantic pieces emitted by the source final-score presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndingScorePart {
    StageScore {
        stage_number: u8,
        score: u8,
        row: u8,
    },
    ParadeTotalLabel,
    ParadeTotalValue,
    ParadeAverageLabel,
    ParadeAverageValue,
    TotalLabel,
    TotalValue,
    AverageLabel,
    AverageValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalScoreStep {
    WaitingForCredits,
    TotalValue,
    AverageLabel,
    AverageValue,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoreSummaryStep {
    TotalValue,
    AverageLabel,
    AverageValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EndingState {
    phase: EndingPhase,
    replay_index: u8,
    replay_ticks_remaining: u16,
    replay_transition_ticks: u8,
    replay_anchor: Option<u16>,
    replay_encounter: Option<BossEncounter>,
    replay_view_height: i16,
    replay_target_view_distance: i16,
    replay_backdrop: EndingReplayBackdrop,
    replay_detail_characters_visible: u8,
    score_stage_index: u8,
    score_stage_ticks: u8,
    score_summary_step: ScoreSummaryStep,
    score_summary_ticks: u8,
    score_fade_ticks: u16,
    final_score_step: FinalScoreStep,
    reveal_ticks: u8,
}

impl Default for EndingState {
    fn default() -> Self {
        Self {
            phase: EndingPhase::StaffRoll,
            replay_index: 0,
            replay_ticks_remaining: 0,
            replay_transition_ticks: 0,
            replay_anchor: None,
            replay_encounter: None,
            replay_view_height: 0,
            replay_target_view_distance: 0,
            replay_backdrop: EndingReplayBackdrop::RisingGradient,
            replay_detail_characters_visible: 0,
            score_stage_index: 0,
            score_stage_ticks: 0,
            score_summary_step: ScoreSummaryStep::TotalValue,
            score_summary_ticks: 0,
            score_fade_ticks: 0,
            final_score_step: FinalScoreStep::WaitingForCredits,
            reveal_ticks: 0,
        }
    }
}

impl Default for TallyState {
    fn default() -> Self {
        Self {
            target_percent: 0,
            current_percent: 0,
            teammate_shields: [0; 3],
            phase: TallyPhase::Ready,
            display_tick: 0,
            ready_ticks: 0,
            bonus_visible: false,
        }
    }
}

impl GameState {
    /// Numeric code in boot.h enum order (BOOT=0 .. ENDING=6), followed by
    /// the port's semantic tally and attract presentation states.
    pub fn code(self) -> u8 {
        match self {
            GameState::Boot => 0,
            GameState::Title => 1,
            GameState::Briefing => 2,
            GameState::PlanetSelect => 3,
            GameState::Playing => 4,
            GameState::Continue => 5,
            GameState::Ending => 6,
            GameState::Tally => 7,
            GameState::AttractIntro => 8,
        }
    }
}

/// Sound command emitted this tick, drained by sf-app.
///
/// Call-site mapping (C `src/game/sound.h`):
/// - `PlayMusic` — `Sound_PlayMusic` (map VM setbgm hook).
/// - `PlaySe` — `Sound_PlaySE` (boot.c:159, strings.c:61/141, level inline
///   callbacks) and `Strat_TrigSE` (strat_common.c:323, which is just
///   `Sound_PlaySE`).
/// - `PlayImmediate` — `Sound_Play` (sound.c:255; no shell-layer call site
///   yet, reserved for sf-strat).
/// - `StopMusic` — `Sound_StopMusic` (sound.c:286; no shell-layer call
///   site yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCmd {
    PlayMusic(u8),
    PlaySe(u8),
    /// `Sound_MakeSnd` (SOUND.ASM makesnd): positional one-shot SE keyed to a
    /// `*sound_l` family, using the source object's world XZ. sf-app resolves
    /// the family to the `sf_audio::sound::POS_*` table and bands it against
    /// the live player position.
    MakeSnd {
        family: PosSndFamilyId,
        x: i16,
        z: i16,
    },
    PlayImmediate(u8),
    StopMusic,
    /// ROM `pausesnd` (MAIN.ASM dopause → IRQ.ASM drain): flush the SE ring
    /// and force port3 to `se_pauseon` ($02) / `se_pauseoff` ($01).
    PauseSnd(u8),
    /// ROM `nosetport3` (SOUND.ASM:955 gate; PATHDATA bird_touch / ENDSEQ /
    /// PLANETS.ASM:257 `stz`). When true, `play_se` drops into the void.
    NoSetPort3(bool),
}

/// C `WindowState` (src/game/game_vars.h:263-268).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowSlot {
    pub mode: u8,
    pub wm_val: u8,
    pub stayblack: u8,
}

/// Camera output for one tick — the C `Transform_SetCamera` arguments
/// (game.c:141-142) plus the `Transform_SnapCamera` cut flag
/// (game.c:146-153). x/y/z are FP16.16 (`FP16_FROM_INT`, types.h:44).
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraSnapshot {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub rx: i16,
    pub ry: i16,
    pub rz: i16,
    pub snap: bool,
}

/// Per-tick observable state for sf-app (HUD, background, fades, messages,
/// route map, camera, and the sf-audio `SoundGameState` inputs).
#[derive(Debug, Clone, Default)]
pub struct FrameSnapshot {
    pub game_state_code: u8,
    pub currentbg: u16,
    pub newmap: u32,
    pub bgflags: u8,
    pub bg2_xscroll: i32,
    pub nomax_bg2_yscroll: bool,
    pub scene_style: SceneStyle,
    /// Background palette-row source selected by FADETOSEA/FADETOGROUND.
    pub pal_target: Option<PaletteFadeTarget>,
    /// ROM `palnum` remaining counter. Starts at 30 and steps by two while
    /// the renderer copies background palette entries 15 down to 1.
    pub palfade_num: u16,
    pub windowmode: u8,
    pub windows: [WindowSlot; 8],
    /// Typed source-authored playfield reveal, if one is being presented.
    pub screen_wipe: ScreenWipeState,
    /// Typed retail fixed-colour circle presentation.
    pub screen_fill_circle: ScreenFillCircleState,
    pub meters: u16,
    pub stayblack: i8,
    pub gameflags: u8,
    pub gameframe: u16,
    /// Typed level-start handoff; source execution state remains oracle-only.
    pub gameplay_entry_phase: GameplayEntryPhase,
    /// Typed CONT.ASM controller-screen interaction phase.
    pub briefing_phase: BriefingPhase,
    /// Typed CONT.ASM TRAINING/GAME selection.
    pub briefing_choice: BriefingChoice,
    /// Active one of the four source controller layouts.
    pub control_type: ControlType,
    pub boostcnt: u8,
    pub arrows: u8,
    pub player_view_mode: PlayerViewMode,
    pub stage: u16,
    pub shield_cur: i32,
    pub shield_max: i32,
    pub boss_hp_cur: i32,
    pub boss_hp_max: i32,
    pub lives: i32,
    pub bombs: i32,
    /// ROM `specflash` — nova-bomb HUD blink timer (SPRITES.ASM do_spec_weap).
    pub specflash: u8,
    /// ROM `shieldup` — wireframe-shield meter fill color (MDRAWLIS color 7).
    pub shieldup: u8,
    pub msg_count1: u8,
    pub msg_count2: u8,
    pub whichfriend: u8,
    pub friends_meter: u8,
    pub message_text: Option<String>,
    pub whichroute: u8,
    pub currentplanet: i16,
    pub nebula_on: u16,
    pub route_path_ids: Vec<u16>,
    /// Typed `planetseq_l` route-map and General Pepper presentation.
    pub planet_presentation: PlanetPresentation,
    pub camera: CameraSnapshot,
    // sound-layer inputs (sf-app feeds these to sf-audio's SoundGameState)
    pub player_dead: bool,
    pub player_hp0: bool,
    pub engine_snd: bool,
    pub level_finished: u8,
    pub space_mode: bool,
    pub pviewposx: i16,
    /// Running total hit-percentage score (ROM `calctotalscore`/tpa), drawn on
    /// the map screen by `drawroutename` (PLANETS.ASM:1547).
    pub score_total: u16,
    /// Bonus continue credits (ROM `credits`).
    pub credits: u8,
    /// True while the end-of-level tally screen is showing (GameState::Tally).
    pub tally_active: bool,
    /// The just-finished stage's hit percentage, shown on the tally screen.
    pub tally_stage_perc: u8,
    /// Animated graph value (ROM `cla1`).
    pub tally_current_perc: u8,
    /// Peppy, Falco, and Slippy shield values in screen order.
    pub tally_teammate_shields: [u8; 3],
    /// `BONUS 1 CREDIT` replaces the graph after the bonus delay.
    pub tally_bonus_visible: bool,
    /// Active route-specific boss recap, including its typed background and
    /// exact detail-panel reveal boundary.
    pub ending_replay: Option<EndingReplayPresentation>,
    /// The credits map has completed and the permanent final-score
    /// presentation is active.
    pub ending_final_score_visible: bool,
    /// All four final-score parts have been emitted and the permanent ending
    /// presentation is fully assembled.
    pub ending_final_score_complete: bool,
}

/// State shared between the shell and the map-VM hooks (the C globals that
/// windows.c/strings.c/sound.c exposed to world.c/levels.c).
struct ShellState {
    windows: Windows,
    /// Native replacement for the source `circletab` opening-wipe cursor.
    screen_wipe: ScreenWipeState,
    /// Remaining fully-closed presentation ticks before the aperture advances.
    screen_wipe_hold: u8,
    /// The Corneria launch maps request a second reveal at their explicit
    /// `initblack_l` handoff after the scramble corridor.
    pending_init_black_wipe: Option<ScreenWipeKind>,
    /// A catalog-managed opening owns the first common-wrapper `initblack_l`;
    /// suppress that duplicate black window if the builder retained it.
    suppress_next_init_black: bool,
    strings: Strings,
    sound: Vec<SoundCmd>,
    /// Last `setcharmap*_l` layout (HD stand-in for SNES VRAM tilemap upload).
    charmap: CharMap,
    /// C `s_missing_path_warned` (src/path/paths.c) — warn-once bitmap for
    /// `resolve_path_start`.
    path_warned: Vec<bool>,
    /// Per-shape collision half-extents (C `load_collision_extents`, from the
    /// shape meshes), keyed by shape id. Populated by [`Shell::set_shape_extents`]
    /// from the renderer's shape store; missing shapes fall back to the coldet
    /// DEFAULT_COLL_EXTENT.
    shape_extents: HashMap<u16, (i16, i16, i16)>,
}

impl ShellState {
    fn new() -> Self {
        ShellState {
            windows: Windows::new(),
            screen_wipe: ScreenWipeState::inactive(),
            screen_wipe_hold: 0,
            pending_init_black_wipe: None,
            suppress_next_init_black: false,
            strings: Strings::new(),
            sound: Vec::new(),
            charmap: CharMap::new(),
            path_warned: vec![false; 512],
            shape_extents: HashMap::new(),
        }
    }

    fn begin_screen_wipe(&mut self, kind: ScreenWipeKind, black_hold: u8) {
        self.screen_wipe.begin(kind);
        self.screen_wipe_hold = black_hold;
    }

    fn configure_opening_wipe(&mut self, plan: sf_map::catalog::OpeningWipePlan) {
        self.screen_wipe = ScreenWipeState::inactive();
        self.screen_wipe_hold = 0;
        self.pending_init_black_wipe = plan.on_init_black;
        self.suppress_next_init_black = false;

        if let Some(kind) = plan.initial {
            let is_launch_sequence = plan.on_init_black.is_some();
            self.begin_screen_wipe(
                kind,
                if is_launch_sequence {
                    0
                } else {
                    OPENING_WIPE_BLACK_HOLD_TICKS
                },
            );
            self.suppress_next_init_black = !is_launch_sequence;
        }
    }

    fn step_screen_wipe(&mut self) -> bool {
        if !self.screen_wipe.active {
            return false;
        }
        if self.screen_wipe_hold > 0 {
            self.screen_wipe_hold -= 1;
            return true;
        }
        let active = self.screen_wipe.advance();
        if !active {
            self.suppress_next_init_black = false;
        }
        active
    }
}

/// Map-VM outward-effect hooks (see [`crate::game::Hooks`]) wired to the
/// shell's shared Windows/Strings/sound-queue state.
struct ShellHooks {
    state: Rc<RefCell<ShellState>>,
}

impl Hooks for ShellHooks {
    fn play_music(&mut self, track_id: u8) {
        // C Sound_PlayMusic (map setbgm, world.c setbgmdo).
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlayMusic(track_id));
    }

    fn play_se(&mut self, sound_id: u8) {
        // C Sound_PlaySE (level inline callbacks).
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlaySe(sound_id));
    }

    fn make_snd(&mut self, family: PosSndFamilyId, obj_worldx: i16, obj_worldz: i16) {
        // C makesnd (SOUND.ASM:899): positional one-shot SE, banded by sf-app.
        self.state.borrow_mut().sound.push(SoundCmd::MakeSnd {
            family,
            x: obj_worldx,
            z: obj_worldz,
        });
    }

    fn trig_se(&mut self, sound_id: u8) {
        // C Strat_TrigSE == Sound_PlaySE (src/strat/strat_common.c:323).
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlaySe(sound_id));
    }

    fn set_nosetport3(&mut self, disabled: bool) {
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::NoSetPort3(disabled));
    }

    fn send_message(&mut self, msg_id: u8) {
        // C Strings_SendMessage (map sendmsg, CLfriendmsg builtins).
        self.state.borrow_mut().strings.send_message(msg_id);
    }

    fn set_friends_meter(&mut self, value: u8) {
        self.state.borrow_mut().strings.friends_meter = value;
    }

    fn fade_to_black(&mut self, speed: i32) {
        self.state.borrow_mut().windows.fade_to_black(speed);
    }

    fn fade_from_black(&mut self, speed: i32) {
        self.state.borrow_mut().windows.fade_from_black(speed);
    }

    fn is_map_fade_active(&self) -> bool {
        self.state.borrow().windows.is_map_fade_active()
    }

    fn init_black(&mut self) {
        let mut state = self.state.borrow_mut();
        if let Some(kind) = state.pending_init_black_wipe.take() {
            state.begin_screen_wipe(kind, OPENING_WIPE_BLACK_HOLD_TICKS);
            return;
        }
        if state.suppress_next_init_black {
            state.suppress_next_init_black = false;
            return;
        }
        state.windows.init_black();
    }

    fn init_fade_white2norm(&mut self) {
        self.state.borrow_mut().windows.init_fade_white2norm();
    }

    fn boss_flash(&mut self) {
        self.state.borrow_mut().windows.boss_flash();
    }

    fn flash_turq(&mut self) {
        self.state.borrow_mut().windows.flash_turq();
    }

    fn flash_turq2(&mut self) {
        self.state.borrow_mut().windows.flash_turq2();
    }

    fn flash_red(&mut self) {
        self.state.borrow_mut().windows.flash_red();
    }

    fn hitflash_off(&mut self) {
        self.state.borrow_mut().windows.hitflash_off();
    }

    fn set_charmap_game(&mut self) {
        self.state.borrow_mut().charmap.set_game();
    }

    fn set_charmap_plan(&mut self) {
        self.state.borrow_mut().charmap.set_plan();
    }

    fn set_charmap_fox(&mut self) {
        self.state.borrow_mut().charmap.set_fox();
    }

    fn resolve_path_start(&mut self, path_id: u16) -> u16 {
        // C Paths_ResolveStart (src/path/paths.c:2930) against the sf-path
        // literal catalog: valid offset -> offset; missing/out-of-range ->
        // warn once and return 0 (the remove-stub start).
        let catalog = sf_path::literals::get_catalog();
        if (path_id as usize) < catalog.offsets.len() {
            let off = catalog.offsets[path_id as usize];
            if off != 0xFFFF {
                return off;
            }
        }
        let mut st = self.state.borrow_mut();
        if (path_id as usize) < st.path_warned.len() && !st.path_warned[path_id as usize] {
            st.path_warned[path_id as usize] = true;
            println!("Paths: path id {path_id} is not ported yet; using remove-stub");
        }
        0
    }

    fn shape_extents(&self, shape: u16) -> Option<(i16, i16, i16)> {
        // C load_collision_extents (coldet.c:43): real per-shape AABB
        // half-extents from the shape meshes, injected via set_shape_extents.
        self.state.borrow().shape_extents.get(&shape).copied()
    }
}

/// The boot/frame shell around [`crate::game::Game`] — one instance per
/// running game, ticked at 20 Hz by sf-app.
pub struct Shell {
    pub game: Game,
    state: Rc<RefCell<ShellState>>,
    game_state: GameState,
    /// Typed reset/loading interval before BOOTNMI enters the attract sequence.
    boot_ticks: u16,
    /// Typed ENDSEQ attract-level load and fade handoff state.
    attract: AttractSequence,
    /// Typed CONT.ASM controller-layout and destination state.
    briefing: BriefingSequence,
    /// Training-mode fade and return handoff.
    training: TrainingSequence,
    planets: Planets,
    /// Flat typed route-map and General Pepper presentation fields.
    planet_presentation: PlanetPresentation,
    /// Source-domain level initialization phase and measured presentation
    /// duration. This delays publication of the active native level without
    /// retaining any source-machine execution state.
    gameplay_entry_phase: GameplayEntryPhase,
    gameplay_initialization_ticks_remaining: u8,
    camera: GameCamera,
    /// C `s_draw_list` (boot.c:46).
    draw_list: Vec<DrawListEntry>,
    cam_snapshot: CameraSnapshot,
    /// Previous latched pad (C `g_pad1_prev`, sf_rtl.c).
    prev_pad: u16,
    /// C `g_pad1_new`.
    pad1_new: u16,
    /// C `s_levelclear_ticks` (boot.c:42).
    levelclear_ticks: i32,
    /// C `s_death_ticks` (boot.c:43).
    death_ticks: i32,
    /// Typed native equivalent of `cla1`/`cla2`/`clam`/`clb1`/`plotx1`.
    tally: TallyState,
    /// Typed post-campaign staff-roll/final-score state.
    ending: EndingState,
    /// C `g_rndval` (sf_rtl.c:16) — strings face animation PRNG.
    rndval: u16,
    /// Warn-once set for unported map ids (C levels.c warn-once END stub).
    warned_maps: Vec<u32>,

    /// ROM `dopause` latch (MAIN.ASM:1386) — START toggles; freezes nmi tick.
    paused: bool,

    /// Strategy-registration hook (C `Strat_RegisterAll`, boot.c). Injected
    /// by the app layer because `sf-strat` depends on `sf-game`, so this
    /// crate can't call `sf_strat::table::register_all` directly. Invoked
    /// after every `World::init()` reset (game_init/title_tick/gameplay
    /// start), mirroring how C re-runs Strat_RegisterAll on each level load.
    register_strats: Option<Box<dyn Fn(&mut Game)>>,

    /// Player-spawn hook (C `Strat_SpawnPlayer` + `Strat_PlayerOpening_Init`,
    /// boot.c:89-102). Injected by the app layer (same circular-dep reason as
    /// `register_strats`). Called with the newmap id at gameplay start so it
    /// can run the opening strategy for LEVEL1_1/2_1/3_1.
    spawn_player: Option<Box<dyn Fn(&mut Game, u32)>>,

    /// Final-score object-spawn hook, injected by sf-app because the concrete
    /// path-text strategy lives in sf-strat.
    ending_score_part: Option<Box<dyn Fn(&mut Game, EndingScorePart, u16, u16)>>,

    /// Boss-recap object producer. The sf-strat implementation installs the
    /// real encounter initializer and returns the camera-anchor object.
    ending_boss_replay: Option<Box<dyn Fn(&mut Game, BossEncounter) -> Option<u16>>>,
}

impl Shell {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(ShellState::new()));
        let hooks = ShellHooks {
            state: Rc::clone(&state),
        };
        Shell {
            game: Game::with_hooks(Box::new(hooks)),
            state,
            game_state: GameState::Boot, // C g_game_state init (boot.c:33)
            boot_ticks: 0,
            attract: AttractSequence::default(),
            briefing: BriefingSequence::default(),
            training: TrainingSequence::default(),
            planets: Planets::new(),
            planet_presentation: PlanetPresentation::default(),
            gameplay_entry_phase: GameplayEntryPhase::Inactive,
            gameplay_initialization_ticks_remaining: 0,
            camera: GameCamera::new(),
            draw_list: Vec::new(),
            cam_snapshot: CameraSnapshot::default(),
            prev_pad: 0,
            pad1_new: 0,
            levelclear_ticks: 0,
            death_ticks: 0,
            tally: TallyState::default(),
            ending: EndingState::default(),
            rndval: 0,
            warned_maps: Vec::new(),
            paused: false,
            register_strats: None,
            spawn_player: None,
            ending_score_part: None,
            ending_boss_replay: None,
        }
    }

    /// Install the strategy-registration hook (app layer passes
    /// `sf_strat::table::register_all`). Runs it once now for the current
    /// world, then re-runs it after every `World::init()` reset.
    pub fn set_register_strats(&mut self, hook: Box<dyn Fn(&mut Game)>) {
        hook(&mut self.game);
        self.register_strats = Some(hook);
    }

    /// Install the per-shape collision half-extents table (app layer builds it
    /// from the renderer's shape store). Mirrors the C `load_collision_extents`
    /// data path: coldet reads real AABB half-extents for known shapes and
    /// keeps DEFAULT_COLL_EXTENT for the rest.
    pub fn set_shape_extents(&mut self, table: HashMap<u16, (i16, i16, i16)>) {
        self.state.borrow_mut().shape_extents = table;
    }

    /// Install the player-spawn hook (app layer passes a closure that calls
    /// `sf_strat::player::strat_spawn_player` + `strat_player_opening_init`).
    /// Called at gameplay start with the newmap id.
    pub fn set_spawn_player(&mut self, hook: Box<dyn Fn(&mut Game, u32)>) {
        self.spawn_player = Some(hook);
    }

    /// Install the source 3D final-score object producer.
    pub fn set_ending_score_part(
        &mut self,
        hook: Box<dyn Fn(&mut Game, EndingScorePart, u16, u16)>,
    ) {
        self.ending_score_part = Some(hook);
    }

    /// Install the source encounter object producer used by the ending recap.
    pub fn set_ending_boss_replay(
        &mut self,
        hook: Box<dyn Fn(&mut Game, BossEncounter) -> Option<u16>>,
    ) {
        self.ending_boss_replay = Some(hook);
    }

    /// Re-run the registration hook after a `World::init()` reset (C re-runs
    /// Strat_RegisterAll on each level load).
    fn reregister_strats(&mut self) {
        if let Some(hook) = self.register_strats.take() {
            hook(&mut self.game);
            self.register_strats = Some(hook);
        }
    }

    /// One full C `Game_Tick` (boot.c:222) at 20 Hz. Caller passes the
    /// latched pad; pad1_new is computed from the previous tick's pad
    /// (SfRtl_BeginFrame edge semantics, sf_rtl.c:142-147) and pad1 is
    /// stored into `game.vars.pad1`.
    pub fn tick(&mut self, pad1: u16) {
        let circle_was_active = self.game.vars.screen_fill_circle.is_active();
        self.game.vars.screen_fill_circle.advance();
        if circle_was_active && !self.game.vars.screen_fill_circle.is_active() {
            self.game.vars.strategy.circle_object = 0;
            self.game.vars.circleanim = 0;
        }
        // The frame assembled after this update presents the newly selected
        // record. Advancing before simulation lets a wipe started by this
        // tick's map code retain its authored frame zero for one full frame.
        let (wipe_was_active, wipe_active) = {
            let mut state = self.state.borrow_mut();
            let was_active = state.screen_wipe.active;
            (was_active, state.step_screen_wipe())
        };
        self.game.vars.strategy.wipe_active = u8::from(wipe_active);
        if wipe_was_active && !wipe_active && self.game.vars.circleanim == 1 {
            self.game.vars.circleanim = 0;
        }

        // IRQ.ASM `getcont0` maps the physical pad through the selected
        // controller layout before every consumer sees held/edge state.
        let pad1 = self.briefing.control_type.map_pad(pad1);
        let trace_state = self.game_state;
        self.pad1_new = pad1 & !self.prev_pad;
        self.prev_pad = pad1;
        self.game.vars.pad1 = pad1;

        // Live friend-HP mirror for the send_message hook path: C
        // strings.c reads g_bunny_hp/g_falcon_hp/g_frog_hp at call time;
        // GameVars is canonical for those (the map VM friend-alive
        // callbacks read them there). Nothing mutates them mid-tick until
        // sf-strat lands, so a top-of-tick sync is exact.
        let friends_meter = {
            let mut st = self.state.borrow_mut();
            st.strings.bunny_hp = self.game.vars.bunny_hp;
            st.strings.falcon_hp = self.game.vars.falcon_hp;
            st.strings.frog_hp = self.game.vars.frog_hp;
            st.strings.friends_meter
        };
        self.game.vars.shared.friends_meter = friends_meter;

        // C Game_Tick state switch (boot.c:226-276).
        match self.game_state {
            GameState::Boot => {
                if self.boot_ticks < BOOT_TO_ATTRACT_DELAY_TICKS {
                    self.boot_ticks += 1;
                } else {
                    self.game_init();
                }
            }
            GameState::AttractIntro => self.attract_intro_tick(),
            GameState::Title => self.title_tick(),
            GameState::Briefing => self.briefing_tick(),
            GameState::PlanetSelect => {
                // No 3D scene on the map screen (boot.c:243-248).
                self.draw_list.clear();
                self.planet_sequence_tick();
            }
            GameState::Playing => {
                if self.gameplay_entry_phase == GameplayEntryPhase::LevelInitialization {
                    self.gameplay_initialization_tick();
                } else if self.planets.newmap == sf_map::catalog::map_id::TRAINING {
                    self.nmi_game_tick();
                    self.training_progress_tick();
                } else {
                    // ROM gameloop START → dopause
                    // (MAIN.ASM:200-211 / 1386-1426).
                    self.try_toggle_pause();
                    if !self.paused {
                        self.nmi_game_tick();
                        self.gameplay_progress_tick();
                    }
                }
            }
            GameState::Continue => {
                // CONTINUE.ASM `foxy_continue_l`: accept costs one credit
                // (`dec credits` + `trigse $67` + `startbgm $f1`), then refill
                // lives and restart. Zero credits → Title (ROM `.end2`).
                if self.pad1_new & (pad::START | pad::A) != 0 {
                    if self.planets.credits > 0 {
                        self.planets.credits -= 1;
                        {
                            let mut st = self.state.borrow_mut();
                            st.sound.push(SoundCmd::PlaySe(0x67));
                            st.sound.push(SoundCmd::PlayMusic(0xf1));
                        }
                        self.planets.lives = DEFAULT_LIVES;
                        self.game.vars.reset_player_run_state();
                        self.begin_gameplay_from_planet_select();
                    } else {
                        self.enter_title();
                    }
                } else if self.pad1_new & (pad::B | pad::SELECT) != 0 {
                    self.enter_title();
                }
            }
            GameState::Ending => {
                self.ending_tick();
            }
            GameState::Tally => {
                // End-of-level tally screen (ROM end_level_seq).
                // The retail transfer loop keeps presenting the stage-clear
                // scene behind the framebuffer tally. Retain the final typed
                // draw list instead of replacing it with the planet map.
                self.tally_tick();
            }
        }

        let attract_fade_was_active =
            matches!(self.game_state, GameState::AttractIntro | GameState::Title)
                && self.attract.fade_destination.is_some()
                && self.state.borrow().windows.is_map_fade_active();

        // Every tick after the state switch (boot.c:278-284):
        // Bgs_Update, Windows_Update, Strings_Update.
        bgs::update(&mut self.game.vars);
        let friends_meter = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            st.windows
                .update(&mut self.game.vars.oncewipe, &mut self.game.vars.circleanim);
            st.strings
                .update(self.game.vars.gameflags, &mut self.rndval, &mut st.sound);
            st.strings.friends_meter
        };
        self.game.vars.shared.friends_meter = friends_meter;

        // ENDSEQ checks `fadedir` after `transfer_l`; that transfer includes
        // the video interrupt which can complete the fade. Windows_Update is
        // the port's equivalent step, so finish the handoff on this tick when
        // it releases the fade instead of waiting for the next game tick.
        if matches!(self.game_state, GameState::AttractIntro | GameState::Title)
            && self.attract.fade_destination.is_some()
            && attract_fade_was_active
            && !self.state.borrow().windows.is_map_fade_active()
        {
            self.finish_attract_fade_if_ready();
        }

        // Direct strategy callbacks use the source-layout `circleanim` field
        // to request the default star wipe. Promote that request into typed
        // presentation state; smart-bomb value 2 is a separate color/radius
        // effect and is deliberately not mistaken for an opening aperture.
        if self.game.vars.circleanim == 1 {
            let mut state = self.state.borrow_mut();
            if !state.screen_wipe.active {
                state.begin_screen_wipe(ScreenWipeKind::StarReveal, 0);
                self.game.vars.strategy.wipe_active = 1;
            }
        }

        // Diagnostic: report state transitions + the level entered. Low-volume
        // (a handful of transitions per session); makes a "stuck after X" report
        // pinpoint the exact state without a debugger.
        if self.game_state != trace_state {
            println!(
                "[state] {:?} -> {:?}  (frame {}, route {}, stage {}, level {})",
                trace_state,
                self.game_state,
                self.game.vars.gameframe,
                self.planets.whichroute,
                self.planets.stage,
                self.planets.currentlevel,
            );
        }
    }

    pub fn state(&self) -> GameState {
        self.game_state
    }

    /// ROM `dopause` latch — true while gameplay is frozen on START pause.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// C `Game_GetDrawList` (boot.c:287) — the current tick's entries.
    pub fn draw_list(&self) -> &[DrawListEntry] {
        &self.draw_list
    }

    /// Drain sound commands emitted this tick (C Sound_PlaySE /
    /// Sound_PlayMusic call sites in boot.c/windows.c/strings.c/planets.c
    /// and the map VM setbgm hook).
    pub fn drain_sound(&mut self) -> Vec<SoundCmd> {
        std::mem::take(&mut self.state.borrow_mut().sound)
    }

    /// Assemble the per-tick observable snapshot for sf-app.
    pub fn frame(&self) -> FrameSnapshot {
        let st = self.state.borrow();
        let v = &self.game.vars;

        // calcmeters/HUD inputs (nmi.c:77-90): shield from the player
        // alien when present (NMI_PLAYER_MAX_HP cap), else full defaults;
        // boss meter fill = m_bossHP / g_bossmaxhp (mdrawbossHP,
        // MDRAWLIS.MC:985-1057): bosshp is re-summed each frame by the boss
        // part strats, bossmaxhp is set once at boss init.
        let (shield_cur, shield_max) = match self.game.objs.player() {
            Some(p) => (p.hp as i32, NMI_PLAYER_MAX_HP),
            None => (NMI_PLAYER_MAX_HP, NMI_PLAYER_MAX_HP),
        };

        let mut screen_fill_circle = v.screen_fill_circle;
        if let ScreenFillCircleCenter::Object(object_id) = screen_fill_circle.center {
            let object_index = i32::from(object_id).wrapping_sub(1);
            if let Some(object) = self
                .game
                .objs
                .get(object_index)
                .filter(|object| object.active)
            {
                screen_fill_circle.center = ScreenFillCircleCenter::World {
                    x: object.worldx,
                    y: object.worldy,
                    z: object.worldz,
                };
            }
        }

        FrameSnapshot {
            game_state_code: self.game_state.code(),
            currentbg: v.currentbg,
            newmap: match self.game_state {
                GameState::AttractIntro => sf_map::catalog::map_id::INTRO,
                GameState::Title => sf_map::catalog::map_id::TITLE,
                GameState::Briefing => sf_map::catalog::map_id::CONTINUE,
                _ => self.planets.newmap,
            },
            bgflags: v.bgflags,
            bg2_xscroll: v.shared.background_scroll_x as i32,
            nomax_bg2_yscroll: v.strategy.no_maximum_background_y != 0,
            scene_style: v.scene_style,
            pal_target: v.palfade_target,
            palfade_num: v.palfade_num,
            windowmode: st.windows.windowmode,
            windows: st.windows.slots,
            screen_wipe: st.screen_wipe,
            screen_fill_circle,
            meters: v.meters,
            stayblack: v.strategy.stay_black,
            gameflags: v.gameflags,
            gameframe: v.gameframe,
            gameplay_entry_phase: if self.game_state == GameState::Playing {
                self.gameplay_entry_phase
            } else {
                GameplayEntryPhase::Inactive
            },
            briefing_phase: self.briefing.phase,
            briefing_choice: self.briefing.choice,
            control_type: self.briefing.control_type,
            boostcnt: v.strategy.boost_count,
            arrows: v.strategy.arrow_flags,
            player_view_mode: v.player_view_mode,
            stage: self.planets.stage,
            shield_cur,
            shield_max,
            boss_hp_cur: v.bosshp as i32,
            boss_hp_max: v.bossmaxhp as i32,
            lives: self.planets.lives as i32,
            // Live strategy bomb count during gameplay; shell default otherwise.
            bombs: v.strategy.special_weapon_count as i32,
            specflash: self.game.vars.shared.special_flash,
            shieldup: self.game.vars.shieldup,
            msg_count1: st.strings.msg_count1,
            msg_count2: st.strings.msg_count2,
            whichfriend: st.strings.whichfriend,
            friends_meter: st.strings.friends_meter,
            message_text: st.strings.active_text.map(String::from),
            whichroute: self.planets.whichroute,
            currentplanet: self.planets.currentplanet,
            nebula_on: self.planets.nebula_on,
            route_path_ids: self.planets.route_path_ids(self.planets.whichroute),
            planet_presentation: self.planet_presentation,
            camera: self.cam_snapshot,
            player_dead: v.gameflags & GF_PLAYERDEAD != 0,
            player_hp0: v.pshipflags2 & PSF2_PLAYERHP0 != 0,
            engine_snd: v.pshipflags3 & PSF3_ENGINESND != 0,
            level_finished: self.game.world.levelfinished,
            space_mode: v.game_mode == SPACE_MODE,
            pviewposx: self.camera.vars.pviewposx,
            score_total: self.planets.total_score(),
            credits: self.planets.credits,
            tally_active: self.game_state == GameState::Tally,
            tally_stage_perc: self.tally.target_percent,
            tally_current_perc: self.tally.current_percent,
            tally_teammate_shields: self.tally.teammate_shields,
            tally_bonus_visible: self.tally.bonus_visible,
            ending_replay: if self.game_state == GameState::Ending
                && self.ending.phase == EndingPhase::BossReplay
            {
                self.ending
                    .replay_encounter
                    .map(|encounter| EndingReplayPresentation {
                        encounter,
                        backdrop: self.ending.replay_backdrop,
                        text: ending_replay_text(encounter),
                        detail_characters_visible: self.ending.replay_detail_characters_visible,
                    })
            } else {
                None
            },
            ending_final_score_visible: self.game_state == GameState::Ending
                && self.ending.phase == EndingPhase::FinalScore,
            ending_final_score_complete: self.game_state == GameState::Ending
                && self.ending.final_score_step == FinalScoreStep::Complete,
        }
    }

    // ============================================================
    // Internals
    // ============================================================

    /// C `Game_Init()` (src/game/boot.c:109).
    fn game_init(&mut self) {
        // initialise_ram + GameVars_Init (boot.c:112-117): GameVars::init
        // recreates the zeroed WRAM mirror and all ported defaults.
        self.game.vars = GameVars::init();
        // MAIN.ASM initializes the player's run-wide inventory and ship state
        // once after the global variable reset. Stage transitions do not.
        self.game.vars.reset_player_run_state();
        // Obj_Init / GameCamera_Init / World_Init (boot.c:120-122).
        self.game.objs = Objects::init();
        self.camera = GameCamera::new();
        self.camera.init(&mut self.game.vars);
        self.game.world = World::init();
        self.reregister_strats();
        // Paths_Init + Paths_LoadData (boot.c:123-127): the sf-path literal
        // catalog is a static singleton consumed via
        // Hooks::resolve_path_start — no per-run state to reset.
        // MapExec_Init (boot.c:128): covered by World::init + load_map.
        // Sound_Init (boot.c:130) is app-side: sf-audio owns the SPC state.
        {
            let mut st = self.state.borrow_mut();
            st.windows.init(); // Windows_Init (boot.c:131)
            st.strings.init(); // Strings_Init (boot.c:133)
        }
        bgs::init(&mut self.game.vars); // Bgs_Init (boot.c:132)

        self.planets = Planets::new(); // route/stage defaults (game_vars.c:443-458)
        self.planet_presentation = PlanetPresentation::default();
        self.levelclear_ticks = 0;
        self.death_ticks = 0;
        self.rndval = 0; // sf_rtl.c:52

        // BOOTNMI enters `intro_l` before the first `titleseq_l`.
        self.enter_attract_intro();

        // DEBUG: SF_START_PLAYING skips title/briefing/planet-select and drops
        // straight into gameplay (Corneria / LEVEL1_1 by default) — for
        // headless frame capture (SF_DUMP_PPM) and accuracy testing. Optional
        // SF_START_MAP=<id> overrides the starting map id.
        if std::env::var_os("SF_START_PLAYING").is_some() {
            self.planets_init();
            if let Ok(m) = std::env::var("SF_START_MAP") {
                if let Ok(id) = m.parse::<u32>() {
                    self.planets.newmap = id;
                }
            }
            self.begin_gameplay_from_planet_select();
        }
    }

    /// C `MapExec_LoadLevel()` (src/map/map_exec.c:14): unported ids fall
    /// back to the empty (mapend-only) level with a warn-once, matching
    /// the C warn-once END stub in levels.c.
    fn load_map(&mut self, map_id: u32) {
        // The original `register_level_special_inline_callbacks` resets this
        // native KSTRATS counter every time the secret level is loaded.
        if map_id == sf_map::catalog::map_id::SPECIAL {
            self.game.vars.numendok = 0;
        }
        // `register_level2_6_inline_callbacks` also clears MAPTRIGGER before
        // the Mad Trucker script starts consuming its road-block/death bits.
        if map_id == sf_map::catalog::map_id::M2_6 {
            self.game.vars.map.trigger = 0;
        }
        let level = match sf_map::catalog::get_map_data(map_id) {
            Some(level) => level,
            None => {
                if !self.warned_maps.contains(&map_id) {
                    self.warned_maps.push(map_id);
                    println!("Levels: map id {map_id} is not ported yet; using empty level");
                }
                sf_map::catalog::get_map_data(sf_map::catalog::map_id::NONE)
                    .expect("empty level always available")
            }
        };
        self.game.load_level(level);
        self.game.vars.mapptr = sf_map::catalog::map_entry_offset(map_id);
        self.game.world.loaded_map_id = Some(map_id);

        let opening_wipe = sf_map::catalog::opening_wipe_plan(map_id);
        self.state.borrow_mut().configure_opening_wipe(opening_wipe);
        self.game.vars.circleanim = if opening_wipe.initial.is_some() { 1 } else { 0 };
        self.game.vars.strategy.wipe_active = u8::from(opening_wipe.initial.is_some());

        if let Some(background) = sf_map::catalog::opening_background(map_id) {
            self.game.vars.currentbg = background;
            self.game.vars.set_sound_environment_for_bg(background);
            self.game.vars.set_scene_style_for_bg(background);
        }

        // Wire the route lanes' name-keyed callback registrations (they leave
        // BuiltLevel's typed vectors empty). Without this the map VM halts at
        // the first unregistered inline CODE65816 op — for the launch levels
        // that is `level_scramble_keep_player_strat`, which sits before the
        // exit-base setup, so the opening never returns control to the player.
        // Must run after load_level (which seeds the lists from BuiltLevel).
        if let Some((natives, inlines)) = sf_map::catalog::get_map_callback_regs(map_id) {
            self.game
                .world
                .register_named_callbacks(natives, inlines, &level.labels);
        }
    }

    /// C `Planets_Init` call sites (boot.c:160/238): planets.c:306 also
    /// zeroes `g_bg_dmalist`, which lives in GameVars.
    fn planets_init(&mut self) {
        self.planets.init();
        self.game.vars.bg_dmalist = 0;
        // ROM planetseq_l (PLANETS.ASM:257): stz nosetport3 — re-enable SFX
        // after a warp/endseq path that may have set it.
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::NoSetPort3(false));
    }

    fn begin_initial_planet_sequence(&mut self) {
        let selected_planet = Sf1Planet::from_index(self.planets.currentplanet.max(0) as u8);
        self.planet_presentation = PlanetPresentation {
            phase: PlanetSequencePhase::InitialSetup,
            selected_planet,
            briefing_message: self.planets.briefing_message,
            ..PlanetPresentation::default()
        };
        // `planetseq_l` takes ownership of the display after its caller's
        // transfer fade. The Rust window pass is the typed equivalent of that
        // retired caller state, so it must not remain allocated over the map.
        self.state.borrow_mut().windows.init();
        self.game_state = GameState::PlanetSelect;
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlayMusic(MUSIC_PLANET_MAP));
    }

    fn begin_later_planet_sequence(&mut self, previous_planet: Sf1Planet, travel_path_id: u16) {
        let selected_planet = Sf1Planet::from_index(self.planets.currentplanet.max(0) as u8);
        let travel_retail_frames =
            post_tally_travel_retail_frames(travel_path_id, previous_planet, selected_planet);
        self.planet_presentation = PlanetPresentation {
            phase: PlanetSequencePhase::Traveling,
            selected_planet,
            briefing_message: self.planets.briefing_message,
            previous_planet,
            travel_path_id,
            travel_retail_frames,
            ..PlanetPresentation::default()
        };
        // Post-mission `planetseq_l` likewise replaces the tally/gameplay
        // display state. Its own authored black setup interval is represented
        // by `travel_retail_frame`, not by a stale gameplay color window.
        self.state.borrow_mut().windows.init();
        self.game_state = GameState::PlanetSelect;
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlayMusic(MUSIC_PLANET_MAP));
    }

    fn set_planet_phase(&mut self, phase: PlanetSequencePhase) {
        self.planet_presentation.phase = phase;
        self.planet_presentation.phase_tick = 0;
        if phase == PlanetSequencePhase::Briefing {
            self.planet_presentation.briefing_cadence_progress =
                BRIEFING_FAST_CADENCE_INITIAL_PROGRESS;
            self.planet_presentation.briefing_dismissal_pending = false;
        }
    }

    fn emit_pepper_character_sound(&mut self, character: u8) {
        if character != b' ' {
            self.state
                .borrow_mut()
                .sound
                .push(SoundCmd::PlaySe(PEPPER_CHARACTER_SOUND));
        }
    }

    fn begin_planet_dismissal(&mut self) {
        self.set_planet_phase(PlanetSequencePhase::DismissingBriefing);
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlaySe(PEPPER_DISMISS_SOUND));
    }

    /// Typed `planetseq_l` presentation and input loop.
    fn planet_sequence_tick(&mut self) {
        self.planet_presentation.rotation_tick =
            self.planet_presentation.rotation_tick.wrapping_add(1);

        match self.planet_presentation.phase {
            PlanetSequencePhase::InitialSetup => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= INITIAL_ROUTE_MAP_SETUP_TICKS {
                    self.planets.begin_route_selection();
                    self.set_planet_phase(PlanetSequencePhase::RouteSelection);
                }
            }
            PlanetSequencePhase::RouteSelection => {
                let route_input = if self.planet_presentation.route_confirmation_ticks_remaining > 0
                {
                    self.planet_presentation.route_confirmation_ticks_remaining -= 1;
                    if self.planet_presentation.route_confirmation_ticks_remaining == 0 {
                        pad::START
                    } else {
                        0
                    }
                } else if self.pad1_new & ROUTE_CONFIRM_BUTTONS != 0 {
                    self.planet_presentation.route_confirmation_ticks_remaining =
                        ROUTE_CONFIRMATION_HANDOFF_TICKS;
                    0
                } else {
                    self.pad1_new
                };
                match self.planets.route_selection_input(route_input) {
                    RouteSelectionResult::Idle => {}
                    RouteSelectionResult::Changed => {
                        self.planet_presentation.briefing_message = self.planets.briefing_message;
                        self.state
                            .borrow_mut()
                            .sound
                            .push(SoundCmd::PlaySe(BRIEFING_MOVE_SOUND));
                    }
                    RouteSelectionResult::Confirmed => {
                        self.planet_presentation.selected_planet =
                            Sf1Planet::from_index(self.planets.currentplanet.max(0) as u8);
                        self.planet_presentation.briefing_message = self.planets.briefing_message;
                        self.set_planet_phase(PlanetSequencePhase::ShipFlash);
                        let mut state = self.state.borrow_mut();
                        state.sound.push(SoundCmd::PlayMusic(MUSIC_FADE_OUT));
                        state.sound.push(SoundCmd::PlaySe(PLANET_CONFIRM_SOUND));
                    }
                }
            }
            PlanetSequencePhase::Traveling => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                self.planet_presentation.travel_retail_frame = self
                    .planet_presentation
                    .phase_tick
                    .saturating_mul(RETAIL_VIDEO_FRAMES_PER_GAME_TICK)
                    .min(self.planet_presentation.travel_retail_frames);
                if self.planet_presentation.travel_retail_frame
                    >= self.planet_presentation.travel_retail_frames
                {
                    self.set_planet_phase(PlanetSequencePhase::AwaitingConfirmation);
                }
            }
            PlanetSequencePhase::AwaitingConfirmation => {
                if self.pad1_new & (pad::B | pad::START | pad::Y | pad::A | pad::X) != 0 {
                    self.set_planet_phase(PlanetSequencePhase::ShipFlash);
                    let mut state = self.state.borrow_mut();
                    state.sound.push(SoundCmd::PlayMusic(MUSIC_FADE_OUT));
                    state.sound.push(SoundCmd::PlaySe(PLANET_CONFIRM_SOUND));
                }
            }
            PlanetSequencePhase::ShipFlash => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= SHIP_FLASH_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::FadingMap);
                }
            }
            PlanetSequencePhase::FadingMap => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                let retail_frames = self
                    .planet_presentation
                    .phase_tick
                    .saturating_mul(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);
                self.planet_presentation.map_fade_level =
                    retail_frames.min(u16::from(MAP_FADE_STEPS - 1)) as u8;
                if self.planet_presentation.phase_tick >= MAP_FADE_TICKS {
                    self.planet_presentation.map_fade_level = MAP_FADE_STEPS - 1;
                    self.set_planet_phase(PlanetSequencePhase::IsolatingPlanet);
                }
            }
            PlanetSequencePhase::IsolatingPlanet => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= PLANET_ISOLATION_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::CenteringPlanet);
                }
            }
            PlanetSequencePhase::CenteringPlanet => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= PLANET_CENTER_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::PreparingBriefing);
                }
            }
            PlanetSequencePhase::PreparingBriefing => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= BRIEFING_PREPARATION_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::ZoomingPlanet);
                    let music = if self.planet_presentation.selected_planet.is_sphere() {
                        MUSIC_PLANET_ZOOM
                    } else {
                        MUSIC_PLANET_ZOOM_SHORT
                    };
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlayMusic(music));
                }
            }
            PlanetSequencePhase::ZoomingPlanet => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                let zoom_step = planet_zoom_step(self.planet_presentation.phase_tick);
                self.planet_presentation.planet_radius =
                    sf_core::sf1_planets::INITIAL_PLANET_RADIUS
                        .saturating_add(zoom_step as u8)
                        .min(FINAL_PLANET_RADIUS);
                if self.planet_presentation.phase_tick >= PLANET_ZOOM_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::RevealingPlanetName);
                }
            }
            PlanetSequencePhase::RevealingPlanetName => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                let heading = planet_heading(self.planet_presentation.selected_planet);
                let previous = self.planet_presentation.planet_name_characters;
                let visible = (self.planet_presentation.phase_tick / PLANET_NAME_CHARACTER_TICKS)
                    .min(heading.len() as u16) as u8;
                self.planet_presentation.planet_name_characters = visible;
                if visible > previous {
                    self.emit_pepper_character_sound(heading.as_bytes()[usize::from(visible - 1)]);
                }
                let heading_ticks = u16::try_from(heading.len())
                    .expect("planet heading fits the presentation counter")
                    .saturating_mul(PLANET_NAME_CHARACTER_TICKS)
                    .saturating_add(PLANET_NAME_TERMINATION_TICKS);
                if self.planet_presentation.phase_tick >= heading_ticks {
                    self.set_planet_phase(PlanetSequencePhase::Briefing);
                }
            }
            PlanetSequencePhase::Briefing => {
                if self.planet_presentation.briefing_dismissal_pending {
                    self.planet_presentation.briefing_dismissal_pending = false;
                    self.begin_planet_dismissal();
                    return;
                }
                if self.pad1_new & (pad::START | pad::B | pad::A) != 0 {
                    self.planet_presentation.briefing_dismissal_pending = true;
                    return;
                }

                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                let message = briefing_text(self.planet_presentation.briefing_message);
                let (numerator, denominator) =
                    if self.planet_presentation.briefing_characters < BRIEFING_FAST_CURSOR_LIMIT {
                        (
                            BRIEFING_FAST_CADENCE_NUMERATOR,
                            BRIEFING_FAST_CADENCE_DENOMINATOR,
                        )
                    } else {
                        (
                            BRIEFING_SETTLED_CADENCE_NUMERATOR,
                            BRIEFING_SETTLED_CADENCE_DENOMINATOR,
                        )
                    };
                self.planet_presentation.briefing_cadence_progress = self
                    .planet_presentation
                    .briefing_cadence_progress
                    .saturating_add(numerator);
                if self.planet_presentation.briefing_cadence_progress >= denominator {
                    self.planet_presentation.briefing_cadence_progress -= denominator;
                    if self.planet_presentation.briefing_characters == u8::MAX {
                        self.begin_planet_dismissal();
                        return;
                    }
                    self.planet_presentation.briefing_characters += 1;
                    if self.planet_presentation.briefing_characters == BRIEFING_FAST_CURSOR_LIMIT {
                        self.planet_presentation.briefing_cadence_progress = 0;
                    }
                    let visible = usize::from(self.planet_presentation.briefing_characters);
                    if visible <= message.len() {
                        self.emit_pepper_character_sound(message.as_bytes()[visible - 1]);
                    }
                }
            }
            PlanetSequencePhase::DismissingBriefing => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= BRIEFING_DISMISS_HANDOFF_TICKS {
                    self.set_planet_phase(PlanetSequencePhase::FadingOut);
                }
            }
            PlanetSequencePhase::FadingOut => {
                self.planet_presentation.phase_tick =
                    self.planet_presentation.phase_tick.saturating_add(1);
                if self.planet_presentation.phase_tick >= PLANET_EXIT_TICKS {
                    self.planets.finish_map_sequence();
                    self.begin_gameplay_from_planet_select();
                }
            }
        }
    }

    /// C `TitleScreen_Tick()` (src/game/boot.c:141).
    fn title_tick(&mut self) {
        if !self.attract.level_loaded {
            self.load_presentation_map(sf_map::catalog::map_id::TITLE, true);
            self.game.vars.oncewipe = 1;
            self.attract.level_loaded = true;
        }
        self.attract.phase_ticks = self.attract.phase_ticks.saturating_add(1);

        self.presentation_scene_tick();

        if self.finish_attract_fade_if_ready() {
            return;
        }
        if self.attract.fade_destination.is_some() {
            return;
        }

        let timed_out = self.game.vars.gameframe >= TITLE_ATTRACT_DURATION_TICKS;
        let start_pressed = self.game.vars.gameframe >= TITLE_INPUT_DELAY_TICKS
            && self.attract.phase_ticks >= TITLE_PRESENTATION_INPUT_READY_TICKS
            && self.game.vars.pad1 & pad::START != 0;
        if !timed_out && !start_pressed {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            if start_pressed {
                state.sound.push(SoundCmd::PlaySe(16));
            }
            state.sound.push(SoundCmd::PlayMusic(MUSIC_FADE_OUT));
        }
        let destination = if start_pressed {
            AttractDestination::Briefing
        } else {
            AttractDestination::Intro
        };
        self.begin_attract_fade(destination, MapFadeRate::Slow, 0);
    }

    /// Retail `intro_l` presentation loop.
    fn attract_intro_tick(&mut self) {
        if !self.attract.level_loaded {
            self.load_presentation_map(sf_map::catalog::map_id::INTRO, true);
            self.game.vars.oncewipe = 0;
            self.state
                .borrow_mut()
                .sound
                .push(SoundCmd::PlayMusic(MUSIC_ATTRACT_INTRO));
            self.attract.level_loaded = true;
        }

        self.presentation_scene_tick();

        if self.finish_attract_fade_if_ready() {
            return;
        }
        if self.attract.fade_destination.is_some()
            || self.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS
        {
            return;
        }

        if self.game.vars.strategy.intro_exit_requested || self.game.vars.pad1 != 0 {
            self.begin_attract_fade(
                AttractDestination::Title,
                MapFadeRate::Quick,
                INTRO_EXIT_FADE_START,
            );
        }
    }

    /// Retail `briefing_l` controller-layout and TRAINING/GAME selection.
    fn briefing_tick(&mut self) {
        if !self.briefing.level_loaded {
            self.game.vars.oncewipe = 1;
            self.load_presentation_map(sf_map::catalog::map_id::CONTINUE, true);
            self.game.vars.strategy.special_weapon_count = BRIEFING_SPECIAL_WEAPON_COUNT;
            self.state
                .borrow_mut()
                .sound
                .push(SoundCmd::PlayMusic(MUSIC_CONTROLLER_SCREEN));
            self.briefing.level_loaded = true;
        }

        self.presentation_scene_tick();

        if self.finish_briefing_fade_if_ready() || self.briefing.fade_destination.is_some() {
            return;
        }

        if self.briefing.control_confirmation_pending {
            self.briefing.control_confirmation_pending = false;
            self.briefing.phase = BriefingPhase::Destination;
            return;
        }

        match self.briefing.phase {
            BriefingPhase::ControlType => {
                if self.pad1_new & pad::SELECT != 0 {
                    self.briefing.control_type = self.briefing.control_type.next();
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlaySe(BRIEFING_MOVE_SOUND));
                }
                if self.game.vars.gameframe >= BRIEFING_INPUT_DELAY_TICKS
                    && self.pad1_new & pad::START != 0
                {
                    self.briefing.control_confirmation_pending = true;
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlaySe(BRIEFING_CONFIRM_SOUND));
                }
            }
            BriefingPhase::Destination => {
                let previous_choice = self.briefing.choice;
                if self.pad1_new & pad::SELECT != 0 {
                    self.briefing.choice = self.briefing.choice.toggled();
                } else if self.pad1_new & pad::UP != 0 {
                    self.briefing.choice = BriefingChoice::Training;
                } else if self.pad1_new & pad::DOWN != 0 {
                    self.briefing.choice = BriefingChoice::Game;
                }
                if self.briefing.choice != previous_choice {
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlaySe(BRIEFING_MOVE_SOUND));
                }

                if self.pad1_new & (pad::X | pad::Y) != 0 {
                    self.briefing.phase = BriefingPhase::ControlType;
                    return;
                }

                if self.pad1_new & (pad::START | pad::A | pad::B) != 0 {
                    let destination = match self.briefing.choice {
                        BriefingChoice::Training => BriefingFadeDestination::Training,
                        BriefingChoice::Game => BriefingFadeDestination::PlanetSelect,
                    };
                    {
                        let mut state = self.state.borrow_mut();
                        state.sound.push(SoundCmd::PlaySe(BRIEFING_CONFIRM_SOUND));
                        state.sound.push(SoundCmd::PlayMusic(MUSIC_FADE_OUT));
                        state.windows.fade_to_black_over(
                            MapFadeRate::Normal,
                            0,
                            BRIEFING_FADE_TICKS,
                        );
                    }
                    self.briefing.fade_destination = Some(destination);
                }
            }
        }
    }

    fn finish_briefing_fade_if_ready(&mut self) -> bool {
        let Some(destination) = self.briefing.fade_destination else {
            return false;
        };
        if self.state.borrow().windows.is_map_fade_active() {
            return false;
        }

        match destination {
            BriefingFadeDestination::Training => self.begin_training(),
            BriefingFadeDestination::PlanetSelect => {
                self.game.vars.reset_player_run_state();
                self.planets_init();
                self.begin_initial_planet_sequence();
                self.briefing.level_loaded = false;
                self.briefing.fade_destination = None;
            }
        }
        true
    }

    fn enter_briefing(&mut self, phase: BriefingPhase) {
        self.game_state = GameState::Briefing;
        self.attract = AttractSequence::default();
        self.briefing.level_loaded = false;
        self.briefing.phase = phase;
        if phase == BriefingPhase::ControlType {
            self.briefing.choice = BriefingChoice::Training;
        }
        self.briefing.fade_destination = None;
        self.briefing.control_confirmation_pending = false;
    }

    fn begin_training(&mut self) {
        self.game.vars.reset_player_run_state();
        self.planets.stage = 0;
        self.planets.currentlevel = 0;
        self.planets.currentplanet = -1;
        self.planets.newmap = sf_map::catalog::map_id::TRAINING;
        self.planets.lives = TRAINING_LIVES;
        self.training = TrainingSequence::default();
        self.begin_gameplay_from_planet_select();
    }

    /// CONT.ASM's embedded training transfer loop and return to briefing.
    fn training_progress_tick(&mut self) {
        if self.game.world.levelfinished == le::GAMEOVER {
            self.briefing.choice = BriefingChoice::Training;
            self.enter_briefing(BriefingPhase::Destination);
            self.training = TrainingSequence::default();
            return;
        }

        if self.training.returning_to_briefing {
            if !self.state.borrow().windows.is_map_fade_active() {
                self.game.vars.reset_player_run_state();
                self.briefing.choice = BriefingChoice::Game;
                self.enter_briefing(BriefingPhase::Destination);
                self.training = TrainingSequence::default();
            }
            return;
        }

        let training_exit_enabled = self.game.vars.shared.game_flags2 & GAME_FLAG2_INGAME == 0
            || self.game.vars.pshipflags2 & PSF2_PLAYERHP0 == 0;
        if self.game.vars.gameframe >= TRAINING_INPUT_DELAY_TICKS
            && training_exit_enabled
            && self.pad1_new & pad::START != 0
        {
            {
                let mut state = self.state.borrow_mut();
                state.sound.push(SoundCmd::PlayMusic(MUSIC_FADE_OUT));
                state
                    .windows
                    .fade_to_black_over(MapFadeRate::Normal, 0, BRIEFING_FADE_TICKS);
            }
            self.training.returning_to_briefing = true;
        }
    }

    fn load_presentation_map(&mut self, map_id: u32, spawn_player: bool) {
        self.game.objs = Objects::init();
        self.camera.init(&mut self.game.vars);
        self.game.world = World::init();
        self.reregister_strats();
        {
            let mut state = self.state.borrow_mut();
            state.windows.init();
            state.strings.init();
            state.charmap.set_fox();
        }
        self.load_map(map_id);
        if spawn_player {
            if let Some(hook) = self.spawn_player.take() {
                hook(&mut self.game, map_id);
                self.spawn_player = Some(hook);
            }
        }
        self.game.vars.meters = 0;
        self.game.vars.gameframe = 0;
        self.game.vars.strategy.intro_exit_requested = false;
        self.game.vars.pshipflags3 &= !PSF3_ENGINESND;
    }

    fn presentation_scene_tick(&mut self) {
        self.draw_list.clear();
        self.game.run_strategies();
        self.cam_snapshot = self.camera.update(&mut self.game.vars, &self.game.objs);
        self.game.step_palette_fade();
        draw::build_list(
            &mut self.game.objs,
            self.game.vars.playerflymode,
            self.game.vars.gameframe,
            self.camera.vars.viewposx,
            self.camera.vars.viewposz,
            self.cam_snapshot.ry as u8,
            self.game.vars.gameflags,
            &|shape| self.game.hooks.shape_extents(shape),
            &mut self.draw_list,
        );
    }

    fn begin_attract_fade(
        &mut self,
        destination: AttractDestination,
        rate: MapFadeRate,
        intensity: u8,
    ) {
        self.state
            .borrow_mut()
            .windows
            .fade_to_black_from(rate, intensity);
        self.attract.fade_destination = Some(destination);
        self.attract.handoff_ticks_remaining = None;
    }

    fn finish_attract_fade_if_ready(&mut self) -> bool {
        let Some(destination) = self.attract.fade_destination else {
            return false;
        };
        if self.state.borrow().windows.is_map_fade_active() {
            return false;
        }

        if destination == AttractDestination::Briefing {
            match self.attract.handoff_ticks_remaining {
                None => {
                    self.attract.handoff_ticks_remaining = Some(TITLE_TO_BRIEFING_BLACK_HOLD_TICKS);
                    return false;
                }
                Some(ticks_remaining) if ticks_remaining > 1 => {
                    self.attract.handoff_ticks_remaining = Some(ticks_remaining - 1);
                    return false;
                }
                Some(_) => {}
            }
        }

        match destination {
            AttractDestination::Intro => self.enter_attract_intro(),
            AttractDestination::Title => self.enter_title(),
            AttractDestination::Briefing => self.enter_briefing(BriefingPhase::ControlType),
        }
        true
    }

    fn enter_attract_intro(&mut self) {
        self.game_state = GameState::AttractIntro;
        self.attract = AttractSequence::default();
    }

    fn enter_title(&mut self) {
        self.game_state = GameState::Title;
        self.attract = AttractSequence::default();
    }

    /// Enter the retail level-start handoff. The source reaches `gamestart`
    /// before its transfer-bound initialization reaches the active opening
    /// update, so publish the gameplay state now and activate the flat native
    /// level only at the measured completion boundary.
    fn begin_gameplay_from_planet_select(&mut self) {
        self.game_state = GameState::Playing;
        self.gameplay_entry_phase = GameplayEntryPhase::LevelInitialization;
        self.gameplay_initialization_ticks_remaining = LEVEL_INITIALIZATION_TICKS;
        self.draw_list.clear();

        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::NoSetPort3(false));

        // Retail publishes the incoming background identity at `gamestart`,
        // before the expensive level initialization completes.
        if let Some(background) = sf_map::catalog::opening_background(self.planets.newmap) {
            self.game.vars.currentbg = background;
            self.game.vars.set_sound_environment_for_bg(background);
            self.game.vars.set_scene_style_for_bg(background);
        }
    }

    fn gameplay_initialization_tick(&mut self) {
        if self.gameplay_initialization_ticks_remaining > 1 {
            self.gameplay_initialization_ticks_remaining -= 1;
            return;
        }

        self.gameplay_initialization_ticks_remaining = 0;
        self.complete_gameplay_initialization();
    }

    fn complete_gameplay_initialization(&mut self) {
        // Per-run player/game flag reset (boot.c:55-69).
        let v = &mut self.game.vars;
        // Seed the unified lives field from the shell-persistent copy. Death
        // decrements it and the 1-UP item increments it; the shell mirrors it
        // back every tick so respawn, game-over, and the HUD stay consistent.
        v.strategy.lives = self.planets.lives;
        // Strategy difficulty is one-based; route/path level is zero-based,
        // matching the two distinct original globals.
        v.shared.difficulty_level = self.planets.currentlevel.wrapping_add(1);
        v.shared.current_level = self.planets.currentlevel;
        v.shared.stage = self.planets.stage as u8;
        // A newly loaded stage starts with fresh transient gameplay flags.
        // Keeping the previous level's bits leaks cutscene-only state such as
        // ClearShip's GF_NOZREMOVE into the next map, disabling behind-camera
        // culling until the object pool is exhausted.  Planet/space setup and
        // the map callbacks re-arm the flags required by the new stage.
        v.gameflags = 0;
        v.pstratflags = 0;
        // Wing damage is run state; collision contacts and cinematic control
        // locks occupy the remaining bits of the same source-layout field and
        // must not cross the HD shell's direct tally-to-stage transition.
        v.pshipflags &= PSF_STAGE_DAMAGE;
        v.playerflymode = PFM_SHADOWS;
        v.player_view_mode = PlayerViewMode::Exterior;
        v.player_view_options = PlayerViewOptions::Unconfigured;
        v.freezestrats = 0;
        v.bossmaxhp = 0;
        v.meters = 0;
        self.paused = false;
        v.map.trigger = 0;
        // g_screenflashcnt remains renderer-lane state.
        v.circleanim = 0;
        v.player_death_fade_delay = 0;
        v.screen_fill_circle.clear();
        v.oncewipe = 0;
        v.strategy.wipe_active = 0;
        self.state.borrow_mut().windows.init(); // Windows_Init (boot.c:70)

        self.levelclear_ticks = 0;
        self.death_ticks = 0;

        // Gameplay subsystem re-init, preserving route/stage (boot.c:76-86).
        self.game.objs = Objects::init();
        self.camera.init(&mut self.game.vars);
        self.game.world = World::init();
        self.reregister_strats();
        // Paths_Init + Paths_LoadData (boot.c:79-83): static catalog via
        // Hooks::resolve_path_start.
        // MapExec_Init (boot.c:84): covered by World::init + load_map.
        self.load_map(self.planets.newmap);

        // Spawn the player alien (C Strat_SpawnPlayer, boot.c:89-102). The
        // spawn hook is injected from the app layer alongside register_strats
        // (sf-strat depends on sf-game; a direct call would be circular).
        // LEVEL1_1/2_1/3_1 open with `pstrat playeropening`, so pass the
        // newmap so the hook can run Strat_PlayerOpening_Init for those.
        if let Some(hook) = self.spawn_player.take() {
            hook(&mut self.game, self.planets.newmap);
            self.spawn_player = Some(hook);

            // Per-level player-collision setup (ROM GSTRATS.ASM:100-125, the
            // `mapplayermode` player setup): build the three pcbox proxy boxes
            // that route enemy hits onto the ship. The ROM does this in the
            // level-init/exit-base sequence; the port wires it here at gameplay
            // start (right after the spawn hook creates the player at playpt) so
            // the ship actually takes damage. Without it the boxes were never
            // attached and every enemy shot/contact passed through the ship.
            // No-op in sf-game-only harnesses that never register the strat lane
            // (pcbox_strats is None). Player is slot playpt (Strat_SpawnPlayer).
            let player = self.game.vars.internal_playpt;
            if player >= 0 && self.game.objs.aliens[player as usize].active {
                self.game.pcbox_attach_player(player as u16);
            }
        }

        self.gameplay_entry_phase = GameplayEntryPhase::ActiveLevel;
    }

    /// ROM `dopause` (MAIN.ASM:1386-1426) — START edge toggles pause.
    /// Skipped during wipe / stayblack≠−1 / boss dying / pstf_notdie.
    /// Queues `pausesnd` se_pauseon ($02) / se_pauseoff ($01).
    fn try_toggle_pause(&mut self) {
        if self.pad1_new & pad::START == 0 {
            return;
        }
        if self.game.vars.strategy.wipe_active != 0
            || self.game.vars.strategy.stay_black != STAY_BLACK_INACTIVE
        {
            return;
        }
        const BF_DYING: u8 = 16;
        if self.game.vars.shared.boss_flags & BF_DYING != 0 {
            return;
        }
        if self.game.vars.pstratflags & PSTF_NOTDIE != 0 {
            return;
        }
        if !self.paused {
            self.paused = true;
            self.state.borrow_mut().sound.push(SoundCmd::PauseSnd(0x02)); // se_pauseon
        } else {
            self.paused = false;
            self.state.borrow_mut().sound.push(SoundCmd::PauseSnd(0x01)); // se_pauseoff
        }
    }

    /// C `Nmi_GameTick()` (src/game/nmi.c:49) — PLAYING-state tick, exact
    /// ordering.
    fn nmi_game_tick(&mut self) {
        // Game_ClearDrawList + dostrats gate (nmi.c:58-61).
        self.draw_list.clear();
        if self.game.vars.freezestrats & 0x01 == 0 {
            self.game.run_strategies();
        }

        // ENDSEQ `dostratlist` pins the recap camera lane to the active boss
        // after all strategies have moved it and before `getview` consumes the
        // position. This is typed scene state, not a machine-memory alias.
        self.update_ending_replay_anchor();

        // Store last frame's controller state (nmi.c:65-66,
        // TRANS.ASM:158-161).
        self.game.vars.lastcont0 = (self.game.vars.pad1 >> 8) as u8;
        self.game.vars.lastcontl0 = (self.game.vars.pad1 & 0xFF) as u8;

        // getview_l (nmi.c:70).
        self.cam_snapshot = self.camera.update(&mut self.game.vars, &self.game.objs);

        // dosounds_l (nmi.c:73) runs app-side: sf-app feeds the
        // FrameSnapshot sound-layer inputs to sf-audio each tick.

        // TRANS.ASM:166-167: palgoto_l then fadepalto_l. palgoto_l is inert
        // in retail; advance the typed background-palette cursor here so the
        // real shell path does not bypass Game::tick's fade step.
        self.game.step_palette_fade();

        // calcmeters / HUD (nmi.c:77-90): shield/lives/bombs/boss values
        // are surfaced through frame() instead of Hud_Set* calls.
        // ROM do_spec_weap (SPRITES.ASM:673-675) decrements specflash once/frame.
        let flash = self.game.vars.shared.special_flash;
        if flash > 0 {
            self.game.vars.shared.special_flash = flash.wrapping_sub(1);
        }

        // showview_l (nmi.c:96). Cull anchor = camera viewpos (published by
        // getview above), sh_zmax margin from the coldet shape-extents table.
        draw::build_list(
            &mut self.game.objs,
            self.game.vars.playerflymode,
            self.game.vars.gameframe,
            self.camera.vars.viewposx,
            self.camera.vars.viewposz,
            self.cam_snapshot.ry as u8,
            self.game.vars.gameflags,
            &|shape| self.game.hooks.shape_extents(shape),
            &mut self.draw_list,
        );
        if std::env::var_os("SF_DEBUG_DRAW").is_some() && self.game.vars.gameframe % 10 == 0 {
            let n = self.draw_list.len();
            let ships: Vec<String> = self
                .draw_list
                .iter()
                .take(6)
                .map(|e| {
                    format!(
                        "sh={} f={:x} xyz=({},{},{})",
                        e.shape_id,
                        e.flags,
                        e.x >> 16,
                        e.y >> 16,
                        e.z >> 16
                    )
                })
                .collect();
            eprintln!(
                "DRAW gf={} n={} p0.shape={} p0.sflags={:x} {}",
                self.game.vars.gameframe,
                n,
                self.game.objs.aliens[0].shape,
                self.game.objs.aliens[0].sflags,
                ships.join(" | ")
            );
        }

        // Particles_Update (nmi.c:99) is renderer-lane (sf-render).

        // generate_collist_l + collision run (nmi.c:103-107).
        self.game.coldet_generate_list();
        self.game.coldet_run();
    }

    /// C `Gameplay_ProgressTick()` (src/game/boot.c:170) — level
    /// completion and death/respawn/game-over bridge.
    fn gameplay_progress_tick(&mut self) {
        // --- Death / respawn / game over (boot.c:174-192) ---
        // Mirror the unified lives field back to the shell-persistent
        // copy (survives map reloads, feeds the HUD + respawn/game-over).
        self.planets.lives = self.game.vars.strategy.lives;

        if self.game.vars.gameflags & GF_PLAYERDEAD != 0 {
            self.levelclear_ticks = 0;
            self.death_ticks += 1;
            if self.game.vars.player_death_fade_delay == 0 {
                self.state.borrow_mut().windows.fade_to_black(1);
            }
            if self.death_ticks < DEATH_RESPAWN_TICKS {
                return;
            }
            if self.planets.lives > 0 {
                // Reload the current map; route/stage/newmap untouched.
                self.game.vars.reset_player_run_state();
                self.begin_gameplay_from_planet_select();
            } else {
                self.death_ticks = 0;
                // ROM CONTINUE.ASM:55-56 — no credits skips the continue screen.
                if self.planets.credits > 0 {
                    self.game_state = GameState::Continue;
                } else {
                    self.enter_title();
                }
            }
            return;
        }
        self.death_ticks = 0;

        // --- Level completion / ROM LE_* level-end dispatch
        // (MAIN.ASM:222-322) ---
        // The map VM's `mapend(N)` stores N into `world.levelfinished`; ROM
        // branches on that LE_* value (see [`le`]). mapend runs after the
        // clear-demo submap + wipe script.
        let lf = self.game.world.levelfinished;
        if lf == le::PLAYING {
            self.levelclear_ticks = 0;
            return;
        }

        // le_gameover (10): ROM skips `inc stage` and shows GAME OVER
        // (MAIN.ASM:226-227). The port normally reaches game-over through the
        // GF_PLAYERDEAD path above (Finding 5); no ported map sets
        // levelfinished=10, so this arm is defensive only — it must NOT
        // advance the stage.
        if lf == le::GAMEOVER {
            self.levelclear_ticks = 0;
            self.death_ticks = 0;
            if self.planets.credits > 0 {
                self.game_state = GameState::Continue;
            } else {
                self.enter_title();
            }
            return;
        }

        // All remaining codes hold on screen for the clear settle first (ROM
        // animates the clear demo; the port uses a fixed hold).
        self.levelclear_ticks += 1;
        if self.levelclear_ticks < LEVEL_CLEAR_SETTLE_TICKS {
            return;
        }

        match lf {
            // Warp codes (MAIN.ASM:238-322): re-point routes[], record the
            // skipped-stage marker, and walk straight into the warp stage
            // WITHOUT the end-of-level tally.
            le::BHOLE1 | le::BHOLE2 | le::BHOLE3 | le::SPECIAL | le::ENTERBHOLE | le::ENTERSPEC => {
                self.warp_advance(lf)
            }
            // Normal clear (1) + end codes (4/6/7/…): ROM `end_level_seq` runs
            // the tally screen before advancing (MAIN.ASM:253). Compute +
            // record this stage's score, hand off to GameState::Tally; the
            // stage advance happens on tally exit.
            _ => self.enter_tally(),
        }
    }

    /// ROM warp level-end handlers (`MAIN.ASM:238-322`). The black-hole and
    /// special-stage LE_* codes re-point `routes[]`, append the skipped-stage
    /// sentinel (`specbuf`=101, [`score::STAGE_SKIPPED`]), and walk the map
    /// graph WITHOUT the end-of-level tally — ROM
    /// `exittobhole*`/`exittospecial`/`enterbhole` jump straight to
    /// `planetseq_l`, bypassing `end_level_seq`.
    ///
    /// Finding 2 (closed tick 198): exit codes re-point routes[] here; ENTER
    /// codes also arm here (`routechange2` / `routechange1`) because planets
    /// is Shell-owned — the blackhole/path strats only store levelfinished.
    /// PATHDATA bird_touch / blackhole2 still set LE_ENTERSPEC / LE_ENTERBHOLE
    /// on the Game side (sf-strat/sf-path).
    fn warp_advance(&mut self, lf: u8) {
        // MAIN.ASM:302-312: the exit codes rewrite routes[] before re-walking.
        match lf {
            le::BHOLE1 => self.planets.routechangebhole1(), // routes[3]=P19 -> Venom 1 Orbital
            le::BHOLE2 => self.planets.routechangebhole2(), // routes[3]=P18 -> Sector Y
            le::BHOLE3 => self.planets.routechangebhole3(), // routes[3]=P20 -> Sector Z
            le::SPECIAL => self.planets.routechange1(), // routes[0]=P22 -> Out of This Dimension
            // ROM `routechange 2` (GA2STRAT.ASM:2202) arms routes[1]=P21 before
            // LE_ENTERBHOLE — the black-hole approach strat (blackhole2_strat)
            // only holds &mut Game, so the routes[1] arm lands here on dispatch
            // (nothing reads routes[1] between the strat's trigger frame and the
            // stage-advance walk). Closes Route Finding 2's ENTER branch.
            le::ENTERBHOLE => self.planets.routechange2(),
            // PATHDATA bird_touch sets LE_ENTERSPEC + routechange 1; apply the
            // route arm here (same pattern as ENTERBHOLE).
            le::ENTERSPEC => self.planets.routechange1(),
            _ => {}
        }

        // MAIN.ASM:314-320 `enterbhole`: codes 11-15 append the skipped-stage
        // sentinel so the black-hole/special stage is excluded from the score
        // tally score buffer. LE_ENTERSPEC (16, `exitspec.white`) does
        // not store it.
        if lf != le::ENTERSPEC {
            self.planets.stage_scores.push(score::STAGE_SKIPPED);
        }

        // ROM `inc stage` (MAIN.ASM:229) + `planetseq_l` walk, no tally.
        self.advance_stage_and_walk();
    }

    /// ROM `end_level_seq` entry (MAIN.ASM:1077-1101): compute the target
    /// percentage and initialise the animated graph. Score-buffer and credit
    /// mutations remain deferred to their retail display boundaries.
    fn enter_tally(&mut self) {
        // Numerator: per-stage special kills. Denominator: the stable
        // map-build count.
        let specials_dead = self.game.vars.shared.specials_dead;
        let total_specials = self.game.world.total_specials;
        let teammates = score::teammates_alive(
            self.game.vars.bunny_hp,
            self.game.vars.frog_hp,
            self.game.vars.falcon_hp,
        );
        let perc = score::calc_stage_perc(specials_dead, total_specials, teammates);
        self.tally = TallyState {
            target_percent: perc,
            current_percent: 0,
            // MAIN.ASM `friends_hp` iteration order is Peppy, Falco, Slippy.
            teammate_shields: [
                self.game.vars.bunny_hp,
                self.game.vars.falcon_hp,
                self.game.vars.frog_hp,
            ],
            phase: if perc == 0 {
                TallyPhase::CommitDelay {
                    steps_remaining: TALLY_COMMIT_DELAY_STEPS,
                }
            } else {
                TallyPhase::Counting
            },
            display_tick: 0,
            ready_ticks: 0,
            bonus_visible: false,
        };

        self.levelclear_ticks = 0;
        self.game_state = GameState::Tally;
    }

    /// Retail `printspeclp`: count by three, settle, commit the score, delay a
    /// crossed bonus, and award its credit. The enum is the flat typed port of
    /// the source's overlapping display counters.
    fn tally_tick(&mut self) {
        let pressed = self.pad1_new & (pad::START | pad::A | pad::B) != 0;
        if self.tally.phase == TallyPhase::Ready {
            self.tally.ready_ticks = self.tally.ready_ticks.saturating_add(1);
            if pressed || self.tally.ready_ticks >= TALLY_READY_AUTO_TICKS {
                self.advance_stage_after_tally();
            }
            return;
        }

        self.tally.display_tick = self.tally.display_tick.saturating_add(1);
        if self.tally.display_tick < TALLY_DISPLAY_STEP_TICKS {
            return;
        }
        self.tally.display_tick = 0;

        match self.tally.phase {
            TallyPhase::Counting => {
                self.state
                    .borrow_mut()
                    .sound
                    .push(SoundCmd::PlaySe(score::SE_TALLY_COUNT));
                self.tally.current_percent = self
                    .tally
                    .current_percent
                    .saturating_add(TALLY_PERCENT_STEP)
                    .min(self.tally.target_percent);
                if self.tally.current_percent == self.tally.target_percent {
                    self.tally.phase = TallyPhase::CommitDelay {
                        steps_remaining: TALLY_COMMIT_DELAY_STEPS,
                    };
                }
            }
            TallyPhase::CommitDelay { steps_remaining } => {
                let next = steps_remaining.saturating_sub(1);
                if next == 2 {
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlaySe(score::SE_TALLY_COMMIT));
                    let crossed = self.planets.append_stage_score(self.tally.target_percent);
                    self.tally.phase = if crossed {
                        TallyPhase::BonusDelay {
                            steps_remaining: TALLY_BONUS_DELAY_STEPS,
                        }
                    } else {
                        TallyPhase::Ready
                    };
                } else {
                    self.tally.phase = TallyPhase::CommitDelay {
                        steps_remaining: next,
                    };
                }
            }
            TallyPhase::BonusDelay { steps_remaining } => {
                let next = steps_remaining.saturating_sub(1);
                if next == 2 {
                    self.state
                        .borrow_mut()
                        .sound
                        .push(SoundCmd::PlaySe(score::SE_BONUS));
                    self.tally.bonus_visible = true;
                    self.tally.phase = TallyPhase::BonusAward {
                        steps_remaining: TALLY_BONUS_AWARD_STEPS,
                    };
                } else {
                    self.tally.phase = TallyPhase::BonusDelay {
                        steps_remaining: next,
                    };
                }
            }
            TallyPhase::BonusAward { steps_remaining } => {
                if steps_remaining <= 1 {
                    self.planets.award_bonus_credit();
                    self.tally.phase = TallyPhase::Ready;
                } else {
                    self.tally.phase = TallyPhase::BonusAward {
                        steps_remaining: steps_remaining - 1,
                    };
                }
            }
            TallyPhase::Ready => unreachable!("ready handled before display stepping"),
        }
    }

    /// ROM post-tally stage advance (MAIN.ASM:229 `inc stage` + planetseq
    /// re-entry). Extracted from the old level-clear path.
    fn advance_stage_after_tally(&mut self) {
        self.tally = TallyState::default();
        self.advance_stage_and_walk();
    }

    /// ROM `inc stage` (MAIN.ASM:229) + `planetseq_l` re-entry: increment the
    /// stage, re-walk the map graph, and present the authored Arwing travel and
    /// next General Pepper briefing (or enter the ending when the route is
    /// exhausted). Shared by the normal post-tally advance and the warp
    /// ([`Shell::warp_advance`]) path.
    ///
    /// planetseq_l calls `convertroute` on entry (PLANETS.ASM:251) and again
    /// on `.continuewithgame` before gamestart (PLANETS.ASM:1090). The map
    /// walk therefore runs with the *converted* route (e.g. gameplay
    /// whichroute 0 -> 1 -> root P6 -> MAP_ID_1_2), then converts back for
    /// gameplay. Without this bracket the walk used the raw gameplay route
    /// (P1 -> MAP_ID_2_2).
    fn advance_stage_and_walk(&mut self) {
        let previous_planet = Sf1Planet::from_index(self.planets.currentplanet.max(0) as u8);
        self.planets.stage = self.planets.stage.wrapping_add(1);
        self.game.vars.shared.stage = self.planets.stage as u8;
        if let Some(travel_path_id) = self.planets.begin_stage_map_walk() {
            self.begin_later_planet_sequence(previous_planet, travel_path_id);
        } else {
            self.planets.finish_map_sequence();
            self.levelclear_ticks = 0;
            self.begin_ending_score_parade();
        }
    }

    /// Begin the source's pre-recap stage-score parade without resetting the
    /// final gameplay object lane. Two path-text objects and one formatted
    /// score message are added for each recorded stage at 30-tick intervals.
    fn begin_ending_score_parade(&mut self) {
        self.ending = EndingState {
            phase: EndingPhase::ScoreParade,
            ..EndingState::default()
        };
        self.game.vars.meters = 0;
        self.paused = false;
        self.game_state = GameState::Ending;
    }

    fn tick_ending_score_parade(&mut self) {
        self.nmi_game_tick();
        self.game.vars.meters = 0;

        if self.ending.score_stage_ticks > 0 {
            self.ending.score_stage_ticks -= 1;
            if self.ending.score_stage_ticks > 0 {
                return;
            }
        }

        let index = usize::from(self.ending.score_stage_index);
        if let Some(&stage_score) = self.planets.stage_scores.get(index) {
            self.emit_ending_score_part(EndingScorePart::StageScore {
                stage_number: self.ending.score_stage_index.saturating_add(1),
                score: stage_score,
                row: self.ending.score_stage_index,
            });
            self.ending.score_stage_index = self.ending.score_stage_index.saturating_add(1);
            self.ending.score_stage_ticks =
                if usize::from(self.ending.score_stage_index) < self.planets.stage_scores.len() {
                    ENDING_STAGE_ROW_TICKS
                } else {
                    ENDING_STAGE_FINISH_TICKS
                };
            return;
        }

        self.ending.phase = EndingPhase::ScoreSummary;
        self.ending.score_summary_step = ScoreSummaryStep::TotalValue;
        self.ending.score_summary_ticks = ENDING_SUMMARY_REVEAL_TICKS;
        self.emit_ending_score_part(EndingScorePart::ParadeTotalLabel);
    }

    fn tick_ending_score_summary(&mut self) {
        self.nmi_game_tick();
        self.game.vars.meters = 0;
        if self.ending.score_summary_ticks > 0 {
            self.ending.score_summary_ticks -= 1;
            return;
        }

        match self.ending.score_summary_step {
            ScoreSummaryStep::TotalValue => {
                self.emit_ending_score_part(EndingScorePart::ParadeTotalValue);
                self.ending.score_summary_step = ScoreSummaryStep::AverageLabel;
                self.ending.score_summary_ticks = ENDING_SUMMARY_REVEAL_TICKS;
            }
            ScoreSummaryStep::AverageLabel => {
                self.emit_ending_score_part(EndingScorePart::ParadeAverageLabel);
                self.ending.score_summary_step = ScoreSummaryStep::AverageValue;
                self.ending.score_summary_ticks = ENDING_SUMMARY_REVEAL_TICKS;
            }
            ScoreSummaryStep::AverageValue => {
                self.emit_ending_score_part(EndingScorePart::ParadeAverageValue);
                self.ending.phase = EndingPhase::ScoreHold;
                self.ending.score_summary_ticks = ENDING_SUMMARY_HOLD_TICKS;
            }
        }
    }

    fn tick_ending_score_hold(&mut self) {
        self.nmi_game_tick();
        self.game.vars.meters = 0;
        if self.ending.score_summary_ticks > 0 {
            self.ending.score_summary_ticks -= 1;
            return;
        }
        self.ending.phase = EndingPhase::ScoreFade;
        self.ending.score_fade_ticks = ENDING_SUMMARY_FADE_TICKS;
        self.game.world.levelfinished = le::ENDTOTALSCORE;
    }

    fn tick_ending_score_fade(&mut self) {
        self.nmi_game_tick();
        self.game.vars.meters = 0;
        if self.ending.score_fade_ticks > 0 {
            self.ending.score_fade_ticks -= 1;
            return;
        }
        self.begin_ending_replay();
    }

    /// Start the recorded end-sequence boss recap. Encounter identity was
    /// captured at each retail marker while the route ran; the ending consumes
    /// that typed ordered list rather than reconstructing it from a route id.
    fn begin_ending_replay(&mut self) {
        self.ending = EndingState {
            phase: EndingPhase::BossReplay,
            ..EndingState::default()
        };
        self.paused = false;
        self.levelclear_ticks = 0;
        self.death_ticks = 0;
        self.game_state = GameState::Ending;
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlayMusic(MUSIC_END_SEQUENCE));
        // ROM ENDSEQ mutes sound effects throughout the recap and staff roll.
        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::NoSetPort3(true));

        if !self.start_ending_replay_entry(0) {
            self.begin_staff_roll();
        }
    }

    /// Reset the source's object/camera lane and instantiate one semantic
    /// encounter. Warm-up strategy ticks happen off-screen exactly as specified
    /// by that encounter's handler.
    fn start_ending_replay_entry(&mut self, index: u8) -> bool {
        let Some(encounter) = self
            .game
            .vars
            .boss_seq
            .get(usize::from(index))
            .copied()
            .flatten()
        else {
            return false;
        };
        let spec = ending_replay_spec(encounter);

        self.draw_list.clear();
        self.game.vars.gameflags = 0;
        self.game.vars.pstratflags = 0;
        self.game.vars.freezestrats = 0;
        self.game.vars.meters = 0;
        self.game.vars.map.trigger = 0;
        self.game.vars.circleanim = 0;
        self.game.vars.oncewipe = 0;
        self.game.vars.strategy.wipe_active = 0;
        self.game.objs = Objects::init();
        self.camera.init(&mut self.game.vars);
        self.game.world = World::init();
        self.reregister_strats();
        self.state.borrow_mut().windows.init();

        // The source creates its invisible credits player before the replay
        // boss so slot zero remains the canonical player/view anchor.
        if let Some(hook) = self.spawn_player.take() {
            hook(&mut self.game, sf_map::catalog::map_id::CREDITS);
            self.spawn_player = Some(hook);
        }

        let anchor = if let Some(hook) = self.ending_boss_replay.take() {
            let anchor = hook(&mut self.game, encounter);
            self.ending_boss_replay = Some(hook);
            anchor
        } else {
            None
        };
        let Some(anchor) = anchor else {
            return false;
        };

        self.game.vars.viewdist = spec.initial_view_distance;
        self.game.vars.strategy.view_distance = spec.initial_view_distance;
        self.game.vars.strategy.view_pitch = 0;
        self.game.vars.strategy.view_yaw = 0;
        self.game.vars.strategy.view_roll = 0;
        self.ending = EndingState {
            phase: EndingPhase::BossReplay,
            replay_index: index,
            replay_ticks_remaining: spec.duration_ticks,
            replay_transition_ticks: 0,
            replay_anchor: Some(anchor),
            replay_encounter: Some(encounter),
            replay_view_height: spec.view_height,
            replay_target_view_distance: spec.target_view_distance,
            replay_backdrop: spec.backdrop,
            replay_detail_characters_visible: 0,
            ..EndingState::default()
        };

        for _ in 0..spec.warmup_ticks {
            self.game.run_strategies();
            self.update_ending_replay_anchor();
            self.game.vars.meters = 0;
        }
        true
    }

    fn update_ending_replay_anchor(&mut self) {
        if self.game_state != GameState::Ending || self.ending.phase == EndingPhase::StaffRoll {
            return;
        }
        let Some(index) = self.ending.replay_anchor else {
            return;
        };
        let Some(anchor) = self.game.objs.aliens.get(index as usize) else {
            return;
        };
        if !anchor.active {
            return;
        }
        self.game.vars.strategy.player_view_position = [
            anchor.worldx,
            anchor.worldy.wrapping_add(self.ending.replay_view_height),
            anchor.worldz,
        ];
    }

    fn tick_ending_replay(&mut self) {
        if self.ending.replay_ticks_remaining == 0 {
            self.ending.phase = EndingPhase::BossTransition;
            self.ending.replay_transition_ticks = ENDING_REPLAY_TRANSITION_TICKS;
            return;
        }

        self.ending.replay_ticks_remaining -= 1;
        if self.ending.replay_ticks_remaining < ENDING_REPLAY_SCROLL_TICKS {
            let target = self.ending.replay_target_view_distance;
            let current = self.game.vars.strategy.view_distance;
            self.game.vars.strategy.view_distance =
                current.wrapping_add(target.wrapping_sub(current) >> 2);
            self.game.vars.viewdist = self.game.vars.strategy.view_distance;
        }
        if self.ending.replay_ticks_remaining < ENDING_REPLAY_DETAILS_TICK {
            self.ending.replay_detail_characters_visible =
                (ENDING_REPLAY_DETAILS_TICK - self.ending.replay_ticks_remaining) as u8;
        }
        self.nmi_game_tick();
        self.game.vars.meters = 0;
        if self.ending.replay_ticks_remaining == 0 {
            self.ending.phase = EndingPhase::BossTransition;
            self.ending.replay_transition_ticks = ENDING_REPLAY_TRANSITION_TICKS;
        }
    }

    fn tick_ending_replay_transition(&mut self) {
        if self.ending.replay_transition_ticks > 0 {
            self.ending.replay_transition_ticks -= 1;
            self.nmi_game_tick();
            self.game.vars.meters = 0;
            if self.ending.replay_transition_ticks > 0 {
                return;
            }
        }

        let next = self.ending.replay_index.saturating_add(1);
        if !self.start_ending_replay_entry(next) {
            self.begin_staff_roll();
        }
    }

    /// Load the exact retail staff-roll map after the end-sequence replay.
    ///
    /// The boss replay and pre-credits score parade are modelled by the
    /// surrounding ending state machine; this entry point owns the source
    /// `initgame` handoff to `CREDITS`, including the invisible credits
    /// player, background, map callbacks, and staff music.
    fn begin_staff_roll(&mut self) {
        self.ending = EndingState {
            phase: EndingPhase::StaffRoll,
            ..EndingState::default()
        };
        self.planets.newmap = sf_map::catalog::map_id::CREDITS;
        self.paused = false;
        self.levelclear_ticks = 0;
        self.death_ticks = 0;
        self.draw_list.clear();

        // The ending keeps run-wide score and encounter state, but initgame
        // starts a fresh object/map/camera lane for the staff roll.
        self.game.vars.gameflags = 0;
        self.game.vars.pstratflags = 0;
        self.game.vars.freezestrats = 0;
        self.game.vars.meters = 0;
        self.game.vars.map.trigger = 0;
        self.game.vars.circleanim = 0;
        self.game.vars.oncewipe = 0;
        self.game.vars.strategy.wipe_active = 0;
        self.game.objs = Objects::init();
        self.camera.init(&mut self.game.vars);
        self.game.world = World::init();
        self.reregister_strats();
        self.state.borrow_mut().windows.init();
        self.load_map(sf_map::catalog::map_id::CREDITS);

        if let Some(hook) = self.spawn_player.take() {
            hook(&mut self.game, sf_map::catalog::map_id::CREDITS);
            self.spawn_player = Some(hook);
        }

        self.state
            .borrow_mut()
            .sound
            .push(SoundCmd::PlayMusic(MUSIC_STAFF_ROLL));
        self.game_state = GameState::Ending;
    }

    /// Tick the staff-roll map with the normal strategy/camera/draw ordering.
    /// The retail credits script stores `ENDOFCREDS`; from then on the game
    /// keeps presenting the final score indefinitely rather than returning to
    /// the title screen on input.
    fn ending_tick(&mut self) {
        match self.ending.phase {
            EndingPhase::ScoreParade => {
                self.tick_ending_score_parade();
                return;
            }
            EndingPhase::ScoreSummary => {
                self.tick_ending_score_summary();
                return;
            }
            EndingPhase::ScoreHold => {
                self.tick_ending_score_hold();
                return;
            }
            EndingPhase::ScoreFade => {
                self.tick_ending_score_fade();
                return;
            }
            EndingPhase::BossReplay => {
                self.tick_ending_replay();
                return;
            }
            EndingPhase::BossTransition => {
                self.tick_ending_replay_transition();
                return;
            }
            EndingPhase::StaffRoll | EndingPhase::FinalScore => {}
        }
        self.nmi_game_tick();
        if self.ending.phase == EndingPhase::StaffRoll
            && self.game.world.levelfinished == le::ENDOFCREDS
        {
            self.ending.phase = EndingPhase::FinalScore;
            self.emit_ending_score_part(EndingScorePart::TotalLabel);
            self.ending.final_score_step = FinalScoreStep::TotalValue;
            self.ending.reveal_ticks = FINAL_SCORE_REVEAL_TICKS;
            return;
        }

        if self.ending.phase != EndingPhase::FinalScore
            || self.ending.final_score_step == FinalScoreStep::Complete
        {
            return;
        }
        if self.ending.reveal_ticks > 0 {
            self.ending.reveal_ticks -= 1;
            return;
        }

        let (part, next) = match self.ending.final_score_step {
            FinalScoreStep::TotalValue => {
                (EndingScorePart::TotalValue, FinalScoreStep::AverageLabel)
            }
            FinalScoreStep::AverageLabel => {
                (EndingScorePart::AverageLabel, FinalScoreStep::AverageValue)
            }
            FinalScoreStep::AverageValue => {
                (EndingScorePart::AverageValue, FinalScoreStep::Complete)
            }
            FinalScoreStep::WaitingForCredits | FinalScoreStep::Complete => return,
        };
        self.emit_ending_score_part(part);
        self.ending.final_score_step = next;
        self.ending.reveal_ticks = if next == FinalScoreStep::Complete {
            0
        } else {
            FINAL_SCORE_REVEAL_TICKS
        };
    }

    fn emit_ending_score_part(&mut self, part: EndingScorePart) {
        if let Some(hook) = self.ending_score_part.take() {
            hook(
                &mut self.game,
                part,
                self.planets.total_score(),
                self.planets.average_score(),
            );
            self.ending_score_part = Some(hook);
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::screen_wipe::ScreenWipeKind::{HorizontalReveal, StarReveal};
    use sf_map::catalog::map_id;

    #[test]
    fn frame_resolves_null_shape_circle_anchor_to_flat_world_position() {
        const ANCHOR_POSITION: [i16; 3] = [37, -18, 640];

        let mut shell = Shell::new();
        let anchor = shell.game.objs.alloc().expect("circle anchor slot");
        {
            let object = &mut shell.game.objs.aliens[anchor as usize];
            object.worldx = ANCHOR_POSITION[0];
            object.worldy = ANCHOR_POSITION[1];
            object.worldz = ANCHOR_POSITION[2];
        }
        shell
            .game
            .vars
            .screen_fill_circle
            .begin_red(ScreenFillCircleCenter::Object(anchor + 1));

        assert_eq!(
            shell.game.objs.aliens[anchor as usize].shape, 0,
            "anchor remains intentionally invisible"
        );
        assert_eq!(
            shell.frame().screen_fill_circle.center,
            ScreenFillCircleCenter::World {
                x: ANCHOR_POSITION[0],
                y: ANCHOR_POSITION[1],
                z: ANCHOR_POSITION[2],
            }
        );
        assert_eq!(
            shell.game.vars.screen_fill_circle.center,
            ScreenFillCircleCenter::Object(anchor + 1),
            "game state retains the semantic object identity"
        );
    }

    fn finish_active_screen_wipe(shell: &mut Shell) {
        const MAX_PRESENTATION_TICKS: usize = 64;
        for _ in 0..MAX_PRESENTATION_TICKS {
            if !shell.frame().screen_wipe.active {
                return;
            }
            shell.tick(0);
        }
        panic!("screen wipe did not finish within its authored frame budget");
    }

    #[test]
    fn boot_ignores_input_until_the_retail_attract_handoff() {
        let mut shell = Shell::new();

        for _ in 0..BOOT_TO_ATTRACT_DELAY_TICKS {
            shell.tick(pad::START);
            assert_eq!(shell.state(), GameState::Boot);
        }

        shell.tick(pad::START);
        assert_eq!(shell.state(), GameState::AttractIntro);
        assert_eq!(
            shell.game.vars.pad1, 0,
            "boot input must not leak into play"
        );
        assert_eq!(shell.attract.fade_destination, None);
    }

    #[test]
    fn attract_handoff_observes_fade_completion_after_the_transfer_step() {
        const RETAIL_FADE_TICKS: usize = 10;

        let mut shell = Shell::new();
        while shell.state() == GameState::Boot {
            shell.tick(0);
        }
        shell.tick(0);
        shell.game.vars.gameframe = INTRO_INPUT_DELAY_TICKS;

        shell.tick(pad::A);
        assert_eq!(shell.state(), GameState::AttractIntro);
        for _ in 1..RETAIL_FADE_TICKS {
            shell.tick(0);
        }

        assert_eq!(shell.state(), GameState::Title);
    }

    fn advance_to_loaded_title(shell: &mut Shell) {
        const MAX_INTRO_TRANSITION_TICKS: usize = 64;

        while shell.state() == GameState::Boot {
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::AttractIntro);
        shell.tick(0);
        while shell.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
            shell.tick(pad::A);
        }
        for _ in 0..MAX_INTRO_TRANSITION_TICKS {
            if shell.state() == GameState::Title {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::Title);
        shell.tick(0);
        assert_eq!(shell.game.world.loaded_map_id, Some(map_id::TITLE));
        while shell.attract.phase_ticks < TITLE_PRESENTATION_INPUT_READY_TICKS {
            shell.tick(0);
        }
        shell.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;
    }

    fn advance_to_planet_select(shell: &mut Shell) {
        const MAX_PRESENTATION_TRANSITION_TICKS: usize = 64;

        advance_to_loaded_title(shell);
        shell.tick(pad::START);
        for _ in 0..MAX_PRESENTATION_TRANSITION_TICKS {
            if shell.state() == GameState::Briefing {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::Briefing);
        shell.tick(0);
        shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
        shell.tick(pad::START);
        shell.tick(0);
        shell.tick(pad::DOWN);
        shell.tick(0);
        shell.tick(pad::START);
        for _ in 0..MAX_PRESENTATION_TRANSITION_TICKS {
            if shell.state() == GameState::PlanetSelect {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::PlanetSelect);
    }

    fn advance_to_training(shell: &mut Shell) {
        const MAX_PRESENTATION_TRANSITION_TICKS: usize = 64;

        advance_to_loaded_title(shell);
        shell.tick(pad::START);
        for _ in 0..MAX_PRESENTATION_TRANSITION_TICKS {
            if shell.state() == GameState::Briefing {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::Briefing);
        shell.tick(0);
        shell.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
        shell.tick(pad::START);
        shell.tick(0);
        assert_eq!(shell.briefing.phase, BriefingPhase::Destination);
        shell.tick(pad::START);
        for _ in 0..MAX_PRESENTATION_TRANSITION_TICKS {
            if shell.state() == GameState::Playing {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::Playing);
        assert_eq!(shell.planets.newmap, map_id::TRAINING);
        assert_eq!(shell.planets.lives, 1);
        finish_gameplay_initialization(shell);
    }

    #[test]
    fn training_exit_uses_a_fresh_start_edge_and_returns_with_game_selected() {
        const MAX_PRESENTATION_TRANSITION_TICKS: usize = 64;

        let mut shell = Shell::new();
        advance_to_training(&mut shell);

        // A START edge before the source gate must not become valid merely by
        // remaining held across the gate.
        shell.game.vars.gameframe = TRAINING_INPUT_DELAY_TICKS - 2;
        shell.tick(pad::START);
        assert!(!shell.training.returning_to_briefing);
        shell.tick(pad::START);
        assert!(!shell.training.returning_to_briefing);

        // While in the active-game lane, zero player health also blocks exit.
        shell.tick(0);
        shell.game.vars.shared.game_flags2 |= GAME_FLAG2_INGAME;
        shell.game.vars.pshipflags2 |= PSF2_PLAYERHP0;
        shell.tick(pad::START);
        assert!(!shell.training.returning_to_briefing);

        shell.tick(0);
        shell.game.vars.pshipflags2 &= !PSF2_PLAYERHP0;
        shell.tick(pad::START);
        assert!(shell.training.returning_to_briefing);
        for _ in 0..MAX_PRESENTATION_TRANSITION_TICKS {
            if shell.state() == GameState::Briefing {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(shell.state(), GameState::Briefing);
        assert_eq!(shell.briefing.phase, BriefingPhase::Destination);
        assert_eq!(shell.briefing.choice, BriefingChoice::Game);
    }

    #[test]
    fn catalog_opening_wipe_holds_black_then_presents_every_record() {
        let mut shell = Shell::new();
        shell.load_map(map_id::M1_2);
        assert_eq!(shell.frame().screen_wipe.kind, StarReveal);
        assert_eq!(shell.frame().screen_wipe.frame, 0);
        assert!(shell.frame().screen_wipe.active);

        for _ in 0..OPENING_WIPE_BLACK_HOLD_TICKS {
            assert!(shell.state.borrow_mut().step_screen_wipe());
            assert_eq!(shell.frame().screen_wipe.frame, 0);
        }
        for expected_frame in 1..StarReveal.frame_count() {
            assert!(shell.state.borrow_mut().step_screen_wipe());
            assert_eq!(shell.frame().screen_wipe.frame, expected_frame);
        }
        assert!(!shell.state.borrow_mut().step_screen_wipe());
        assert!(!shell.frame().screen_wipe.active);
    }

    #[test]
    fn corneria_init_black_handoff_selects_horizontal_reveal() {
        let mut shell = Shell::new();
        shell.load_map(map_id::M1_1);
        assert_eq!(shell.frame().screen_wipe.kind, StarReveal);

        shell.game.hooks.init_black();
        let frame = shell.frame();
        assert_eq!(frame.screen_wipe.kind, HorizontalReveal);
        assert_eq!(frame.screen_wipe.frame, 0);
        assert_eq!(shell.state.borrow().windows.windowmode, 0);
    }

    #[test]
    fn catalog_transition_suppresses_only_its_duplicate_black_window() {
        let mut shell = Shell::new();
        shell.load_map(map_id::M1_4);
        shell.game.hooks.init_black();
        assert_eq!(shell.state.borrow().windows.windowmode, 0);
        assert_eq!(shell.frame().screen_wipe.kind, HorizontalReveal);

        shell.load_map(map_id::M1_3);
        shell.game.hooks.init_black();
        assert_ne!(shell.state.borrow().windows.windowmode, 0);
        assert!(!shell.frame().screen_wipe.active);
    }

    /// The make_snd hook (positional SE, findings F1-F4) routes through the
    /// shell's sound queue as a distinct SoundCmd carrying the family selector
    /// and the source object's world XZ, alongside one-shot play_se.
    #[test]
    fn make_snd_hook_queues_positional_soundcmd() {
        let mut sh = Shell::new();
        sh.game.hooks.make_snd(PosSndFamilyId::DoorOpen, 111, 222);
        sh.game.hooks.play_se(0x35); // one-shot still works alongside
        sh.game
            .hooks
            .make_snd(PosSndFamilyId::EnemyDownSea, -50, 900);
        let sounds = sh.drain_sound();
        assert_eq!(
            sounds,
            vec![
                SoundCmd::MakeSnd {
                    family: PosSndFamilyId::DoorOpen,
                    x: 111,
                    z: 222
                },
                SoundCmd::PlaySe(0x35),
                SoundCmd::MakeSnd {
                    family: PosSndFamilyId::EnemyDownSea,
                    x: -50,
                    z: 900
                },
            ],
        );
    }

    #[test]
    fn frame_reads_live_typed_hud_and_background_state() {
        let mut shell = Shell::new();
        shell.game.vars.shared.background_scroll_x = -77;
        shell.game.vars.strategy.no_maximum_background_y = 1;
        shell.game.vars.strategy.stay_black = STAY_BLACK_INACTIVE;
        shell.game.vars.strategy.boost_count = 9;
        shell.game.vars.strategy.arrow_flags = 5;
        shell.game.vars.strategy.special_weapon_count = 2;

        let frame = shell.frame();
        assert_eq!(frame.bg2_xscroll, -77);
        assert!(frame.nomax_bg2_yscroll);
        assert_eq!(frame.stayblack, STAY_BLACK_INACTIVE);
        assert_eq!(frame.boostcnt, 9);
        assert_eq!(frame.arrows, 5);
        assert_eq!(frame.bombs, 2);
    }

    #[test]
    fn playing_nmi_advances_the_background_palette_walk() {
        let mut shell = Shell::new();
        shell.game.world.map_loaded = false;
        shell.game.vars.palfade_target = Some(PaletteFadeTarget::Sea);
        shell.game.vars.palfade_num = sf_core::scene::PALETTE_FADE_COUNTER_START;

        shell.nmi_game_tick();

        assert_eq!(
            shell.game.vars.palfade_num,
            sf_core::scene::PALETTE_FADE_COUNTER_START - 2
        );
    }

    #[test]
    fn title_map_writes_the_typed_screen_black_counter() {
        let mut shell = Shell::new();
        advance_to_loaded_title(&mut shell);
        for _ in 0..40 {
            shell.tick(0);
            if shell.game.vars.strategy.stay_black == 10 {
                break;
            }
        }

        assert_eq!(shell.game.vars.strategy.stay_black, 10);
        assert_eq!(shell.frame().stayblack, 10);
        assert_eq!(shell.game.vars.map.global_strategy_byte, 0);
    }

    #[test]
    fn stage_load_preserves_run_inventory_and_ship_upgrades() {
        const DOUBLE_LASER: u8 = 1;
        const BROKEN_LEFT_WING: u8 = 8;
        const STALE_CONTROL_LOCKS: u8 = 32 | 64 | 128;

        let mut shell = Shell::new();
        shell.game.vars.strategy.special_weapon_count = 1;
        shell.game.vars.pshipflags2 = DOUBLE_LASER;
        shell.game.vars.pshipflags = BROKEN_LEFT_WING | STALE_CONTROL_LOCKS;
        shell.begin_gameplay_from_planet_select();
        finish_gameplay_initialization(&mut shell);

        assert_eq!(shell.game.vars.strategy.special_weapon_count, 1);
        assert_eq!(shell.game.vars.pshipflags2 & DOUBLE_LASER, DOUBLE_LASER);
        assert_eq!(shell.game.vars.pshipflags, BROKEN_LEFT_WING);
    }

    /// ROM dopause (MAIN.ASM:1386): START while Playing → pausesnd $02 / $01.
    #[test]
    fn playing_start_toggles_pause_snd() {
        let mut sh = Shell::new();
        advance_to_planet_select(&mut sh);
        let _ = sh.drain_sound();
        launch_from_planet_select(&mut sh);
        let _ = sh.drain_sound();
        assert_eq!(sh.state(), GameState::Playing);
        assert!(!sh.is_paused());
        finish_active_screen_wipe(&mut sh);

        // Pause on.
        sh.tick(pad::START);
        assert!(sh.is_paused());
        assert!(sh.drain_sound().contains(&SoundCmd::PauseSnd(0x02)));

        // Frozen: gameframe must not advance while paused.
        let gf = sh.frame().gameframe;
        sh.tick(0);
        sh.tick(0);
        assert_eq!(sh.frame().gameframe, gf);

        // Pause off.
        sh.tick(pad::START);
        assert!(!sh.is_paused());
        assert!(sh.drain_sound().contains(&SoundCmd::PauseSnd(0x01)));
    }

    /// ROM dopause gates: stayblack≠−1 / doingwipe / bf_dying / pstf_notdie.
    #[test]
    fn pause_blocked_while_stayblack_or_notdie() {
        let mut sh = Shell::new();
        advance_to_planet_select(&mut sh);
        let _ = sh.drain_sound();
        launch_from_planet_select(&mut sh);
        let _ = sh.drain_sound();

        // The source `doingwipe` lock rejects pause during the opening reveal.
        sh.tick(0);
        sh.tick(pad::START);
        assert!(!sh.is_paused());
        assert!(!sh
            .drain_sound()
            .iter()
            .any(|command| matches!(command, SoundCmd::PauseSnd(_))));
        finish_active_screen_wipe(&mut sh);

        sh.game.vars.strategy.stay_black = 0;
        sh.tick(pad::START);
        assert!(!sh.is_paused());
        assert!(!sh
            .drain_sound()
            .iter()
            .any(|c| matches!(c, SoundCmd::PauseSnd(_))));

        sh.game.vars.strategy.stay_black = STAY_BLACK_INACTIVE;
        sh.game.vars.pstratflags |= PSTF_NOTDIE;
        sh.tick(0);
        sh.tick(pad::START);
        assert!(!sh.is_paused());
        assert!(!sh
            .drain_sound()
            .iter()
            .any(|c| matches!(c, SoundCmd::PauseSnd(_))));

        sh.game.vars.pstratflags &= !PSTF_NOTDIE;
        sh.tick(0);
        sh.tick(pad::START);
        assert!(sh.is_paused());
        assert!(sh.drain_sound().contains(&SoundCmd::PauseSnd(0x02)));
    }

    /// Scripted-pad state walk matching BOOTNMI/ENDSEQ/CONT through intro,
    /// title, controller selection, planet select, and gameplay.
    #[test]
    fn scripted_pad_state_walk() {
        let mut sh = Shell::new();
        assert_eq!(sh.state().code(), 0); // GAME_STATE_BOOT

        // BOOTNMI runs the intro before its first title sequence.
        while sh.state() == GameState::Boot {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 8);
        assert_eq!(sh.state(), GameState::AttractIntro);
        while sh.game.vars.gameframe < INTRO_INPUT_DELAY_TICKS {
            sh.tick(pad::A);
        }
        while sh.state() != GameState::Title {
            sh.tick(0);
        }
        sh.tick(0);
        assert_eq!(sh.game.world.loaded_map_id, Some(map_id::TITLE));
        while sh.attract.phase_ticks < TITLE_PRESENTATION_INPUT_READY_TICKS {
            sh.tick(0);
        }
        sh.game.vars.gameframe = TITLE_INPUT_DELAY_TICKS;

        // START fades down into the controller screen.
        sh.tick(pad::START);
        while sh.state() == GameState::Title {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 2); // GAME_STATE_BRIEFING
        sh.tick(0);
        assert_eq!(sh.game.world.loaded_map_id, Some(map_id::CONTINUE));

        // Choose the source GAME destination after the controller-layout gate.
        sh.game.vars.gameframe = BRIEFING_INPUT_DELAY_TICKS - 1;
        sh.tick(pad::START);
        sh.tick(0);
        assert_eq!(sh.frame().briefing_phase, BriefingPhase::Destination);
        sh.tick(pad::DOWN);
        assert_eq!(sh.frame().briefing_choice, BriefingChoice::Game);
        sh.tick(0);
        sh.tick(pad::START);
        while sh.state() == GameState::Briefing {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 3); // GAME_STATE_PLANET_SELECT
        let sounds = sh.drain_sound();
        assert!(sounds.contains(&SoundCmd::PlaySe(0x10)));

        // Route selection becomes interactive only after the source map setup,
        // then route 0 is converted to the displayed source route 1.
        while sh.frame().planet_presentation.phase == PlanetSequencePhase::InitialSetup {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 3);
        assert_eq!(sh.frame().whichroute, 1);
        assert_eq!(sh.frame().stage, 10);

        // Confirm, present the authored map close-up and General Pepper
        // briefing, then dismiss it with a fresh START edge.
        launch_from_planet_select(&mut sh);
        assert_eq!(sh.state().code(), 4); // GAME_STATE_PLAYING
        let f = sh.frame();
        assert_eq!(f.game_state_code, 4);
        assert_eq!(f.newmap, map_id::M1_1);
        assert_eq!(f.lives, 3);
        assert_eq!(f.bombs, 3);
        assert_eq!(f.shield_cur, 40); // no player spawned yet (wave3)
        assert_eq!(f.whichroute, 0); // converted back before launch
        assert!(sh.game.world.map_loaded);

        // Inert gameplay ticks advance the map VM (lastzchange=65 without
        // a player) and keep the state machine in PLAYING.
        for _ in 0..10 {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 4);
        assert!(sh.frame().gameframe >= 10);
    }

    /// Level-clear progression: forcing levelfinished walks the settle
    /// timer and advances the stage into the next route map.
    #[test]
    fn level_clear_advances_stage() {
        let mut sh = into_gameplay();
        assert_eq!(sh.state().code(), 4);
        assert_eq!(sh.frame().stage, 0);

        // Force the level-finished latch (mapend) and run the settle timer.
        // After the settle the shell enters the tally screen (GameState::Tally).
        sh.game.world.levelfinished = 1;
        for _ in 0..LEVEL_CLEAR_SETTLE_TICKS {
            sh.tick(0);
            // Reassert: load_map on the stage advance clears the flag.
            if sh.frame().stage == 0 {
                sh.game.world.levelfinished = 1;
            }
        }
        assert_eq!(sh.state(), GameState::Tally);
        assert_eq!(sh.frame().stage, 0); // stage advance deferred until tally exits
                                         // Run the retail count/commit phases, then dismiss with START.
        run_tally_to_ready(&mut sh);
        sh.tick(pad::START);
        let map_frame = sh.frame();
        assert_eq!(map_frame.stage, 1);
        assert_eq!(sh.state(), GameState::PlanetSelect);
        assert_eq!(
            map_frame.windowmode, 0,
            "the retired tally/gameplay transition must not cover planetseq_l"
        );
        assert_eq!(
            map_frame.planet_presentation.phase,
            PlanetSequencePhase::Traveling
        );
        assert_eq!(map_frame.newmap, map_id::M1_2);
        assert_eq!(
            map_frame.planet_presentation.travel_path_id,
            crate::planets::path_id::P6
        );
        let travel_ticks = map_frame
            .planet_presentation
            .travel_retail_frames
            .div_ceil(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);
        for tick in 1..=travel_ticks {
            sh.tick(0);
            if tick < travel_ticks {
                assert_eq!(
                    sh.frame().planet_presentation.phase,
                    PlanetSequencePhase::Traveling
                );
            }
        }
        assert_eq!(
            sh.frame().planet_presentation.phase,
            PlanetSequencePhase::AwaitingConfirmation
        );
        launch_from_planet_select(&mut sh);
        let f = sh.frame();
        assert_eq!(f.stage, 1);
        assert_eq!(sh.state().code(), 4);
        // Route 0 (spine PATH_ID_6 -> routes[1]=PATH_ID_7): stage 1 is
        // MAP_ID_1_2 (planets.c:100).
        assert_eq!(f.newmap, map_id::M1_2);
    }

    /// Death path: GF_PLAYERDEAD + no lives + credits → CONTINUE after
    /// DEATH_RESPAWN_TICKS; accept spends one credit and refills lives.
    #[test]
    fn death_to_continue_and_back() {
        let mut sh = into_gameplay();
        assert_eq!(sh.state().code(), 4);

        // Lives are unified in WRAM 0x0520 during gameplay (the shell mirrors
        // it back each tick), so exhaust the canonical store.
        sh.planets.lives = 0;
        sh.game.vars.strategy.lives = 0;
        sh.planets.credits = 2; // CONTINUE.ASM requires credits > 0
        sh.game.vars.gameflags |= GF_PLAYERDEAD;
        for _ in 0..DEATH_RESPAWN_TICKS {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 5); // GAME_STATE_CONTINUE
        assert_eq!(sh.frame().credits, 2);

        sh.tick(0); // release edge
        let _ = sh.drain_sound();
        sh.tick(pad::START); // continue: dec credits, refill lives, retry
        assert_eq!(sh.state().code(), 4);
        finish_gameplay_initialization(&mut sh);
        assert_eq!(sh.frame().lives, DEFAULT_LIVES as i32);
        assert_eq!(sh.frame().credits, 1);
        // Begin-gameplay cleared the dead flag (boot.c:55).
        assert!(!sh.frame().player_dead);
        let snd = sh.drain_sound();
        assert!(snd.contains(&SoundCmd::PlaySe(0x67)));
        assert!(snd.contains(&SoundCmd::PlayMusic(0xf1)));
    }

    /// Zero continue credits: death with no lives skips Continue → Title.
    #[test]
    fn death_without_credits_goes_to_title() {
        let mut sh = into_gameplay();
        assert_eq!(sh.state().code(), 4);

        sh.planets.lives = 0;
        sh.game.vars.strategy.lives = 0;
        sh.planets.credits = 0;
        sh.game.vars.gameflags |= GF_PLAYERDEAD;
        for _ in 0..DEATH_RESPAWN_TICKS {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 1); // GAME_STATE_TITLE
    }

    #[test]
    fn terminal_death_waits_twenty_ticks_then_completes_the_black_fade() {
        let mut sh = into_gameplay();
        sh.planets.lives = 0;
        sh.game.vars.strategy.lives = 0;
        sh.planets.credits = 1;
        sh.game.vars.gameflags |= GF_PLAYERDEAD | crate::vars::GF_PLAYERDYING;
        sh.game.vars.player_death_fade_delay = PLAYER_DEATH_FADE_DELAY_TICKS;

        for _ in 1..PLAYER_DEATH_FADE_DELAY_TICKS {
            sh.tick(0);
        }
        assert_eq!(sh.game.vars.player_death_fade_delay, 1);
        assert!(!sh
            .frame()
            .windows
            .iter()
            .any(|window| window.mode == crate::windows::WINDOW_MODE_MAPFADE));

        sh.tick(0);
        let fade_start = sh.frame();
        assert_eq!(sh.game.vars.player_death_fade_delay, 0);
        assert!(fade_start.windows.iter().any(|window| {
            window.mode == crate::windows::WINDOW_MODE_MAPFADE && window.wm_val == 1
        }));

        for _ in 1..BLACK_FADE_MAX {
            sh.tick(0);
        }
        assert_eq!(sh.state(), GameState::Playing);
        assert!(sh.frame().windows.iter().any(|window| {
            window.mode == crate::windows::WINDOW_MODE_MAPFADE && window.wm_val == BLACK_FADE_MAX
        }));

        sh.tick(0);
        assert_eq!(sh.state(), GameState::Playing);
        sh.tick(0);
        assert_eq!(sh.state(), GameState::Continue);
    }

    /// Drive a game into gameplay (BOOT -> PLAYING).
    fn into_gameplay() -> Shell {
        let mut sh = Shell::new();
        advance_to_planet_select(&mut sh);
        launch_from_planet_select(&mut sh);
        assert_eq!(sh.state().code(), 4);
        let _ = sh.drain_sound(); // clear launch SFX
        sh
    }

    fn launch_from_planet_select(shell: &mut Shell) {
        const MAX_PLANET_SEQUENCE_TICKS: usize = 512;

        shell.tick(0);
        if shell.frame().planet_presentation.phase == PlanetSequencePhase::InitialSetup {
            for _ in 0..MAX_PLANET_SEQUENCE_TICKS {
                if shell.frame().planet_presentation.phase == PlanetSequencePhase::RouteSelection {
                    break;
                }
                shell.tick(0);
            }
            assert_eq!(
                shell.frame().planet_presentation.phase,
                PlanetSequencePhase::RouteSelection
            );
        }
        if shell.frame().planet_presentation.phase == PlanetSequencePhase::Traveling {
            for _ in 0..MAX_PLANET_SEQUENCE_TICKS {
                if shell.frame().planet_presentation.phase
                    == PlanetSequencePhase::AwaitingConfirmation
                {
                    break;
                }
                shell.tick(0);
            }
            assert_eq!(
                shell.frame().planet_presentation.phase,
                PlanetSequencePhase::AwaitingConfirmation
            );
        }
        shell.tick(pad::START);
        shell.tick(0);
        shell.tick(0);
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::ShipFlash
        );

        for _ in 0..MAX_PLANET_SEQUENCE_TICKS {
            if shell.frame().planet_presentation.phase == PlanetSequencePhase::Briefing {
                break;
            }
            shell.tick(0);
        }
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::Briefing
        );

        shell.tick(0);
        shell.tick(pad::START);
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::Briefing
        );
        shell.tick(0);
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::DismissingBriefing
        );
        for _ in 0..MAX_PLANET_SEQUENCE_TICKS {
            if shell.state() == GameState::Playing {
                finish_gameplay_initialization(shell);
                return;
            }
            shell.tick(0);
        }
        panic!("planet sequence did not hand off to gameplay");
    }

    fn finish_gameplay_initialization(shell: &mut Shell) {
        for _ in 0..LEVEL_INITIALIZATION_TICKS {
            if shell.gameplay_entry_phase == GameplayEntryPhase::ActiveLevel {
                return;
            }
            shell.tick(0);
        }
        assert_eq!(
            shell.gameplay_entry_phase,
            GameplayEntryPhase::ActiveLevel,
            "level initialization did not reach its measured completion boundary"
        );
    }

    #[test]
    fn level_initialization_uses_the_measured_retail_handoff() {
        let mut shell = Shell::new();
        shell.planets.newmap = map_id::M1_1;
        shell.begin_gameplay_from_planet_select();

        assert_eq!(
            shell.frame().gameplay_entry_phase,
            GameplayEntryPhase::LevelInitialization
        );
        assert_ne!(shell.game.world.loaded_map_id, Some(map_id::M1_1));

        for _ in 1..LEVEL_INITIALIZATION_TICKS {
            shell.tick(0);
            assert_eq!(
                shell.frame().gameplay_entry_phase,
                GameplayEntryPhase::LevelInitialization
            );
        }
        shell.tick(0);

        assert_eq!(
            shell.frame().gameplay_entry_phase,
            GameplayEntryPhase::ActiveLevel
        );
        assert_eq!(shell.game.world.loaded_map_id, Some(map_id::M1_1));
        assert_eq!(shell.frame().gameframe, 0);
    }

    fn assert_planet_phase_duration(
        shell: &mut Shell,
        phase: PlanetSequencePhase,
        ticks: u16,
        next_phase: PlanetSequencePhase,
    ) {
        assert_eq!(shell.frame().planet_presentation.phase, phase);
        for elapsed in 1..=ticks {
            shell.tick(0);
            if elapsed < ticks {
                assert_eq!(shell.frame().planet_presentation.phase, phase);
            }
        }
        assert_eq!(shell.frame().planet_presentation.phase, next_phase);
    }

    #[test]
    fn initial_planet_sequence_uses_the_authored_phase_boundaries() {
        let mut shell = Shell::new();
        advance_to_planet_select(&mut shell);
        let entry_sounds = shell.drain_sound();
        assert!(entry_sounds.contains(&SoundCmd::PlayMusic(MUSIC_PLANET_MAP)));
        assert_eq!(
            shell.frame().windowmode,
            0,
            "the retired controller-screen fade must not cover planetseq_l"
        );

        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::InitialSetup,
            INITIAL_ROUTE_MAP_SETUP_TICKS,
            PlanetSequencePhase::RouteSelection,
        );

        shell.tick(0);
        shell.tick(pad::START);
        shell.tick(0);
        shell.tick(0);
        assert_eq!(shell.frame().whichroute, 1);
        let confirmation_sounds = shell.drain_sound();
        assert!(confirmation_sounds.contains(&SoundCmd::PlayMusic(MUSIC_FADE_OUT)));
        assert!(confirmation_sounds.contains(&SoundCmd::PlaySe(PLANET_CONFIRM_SOUND)));

        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::ShipFlash,
            SHIP_FLASH_TICKS,
            PlanetSequencePhase::FadingMap,
        );
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::FadingMap,
            MAP_FADE_TICKS,
            PlanetSequencePhase::IsolatingPlanet,
        );
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::IsolatingPlanet,
            PLANET_ISOLATION_TICKS,
            PlanetSequencePhase::CenteringPlanet,
        );
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::CenteringPlanet,
            PLANET_CENTER_TICKS,
            PlanetSequencePhase::PreparingBriefing,
        );
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::PreparingBriefing,
            BRIEFING_PREPARATION_TICKS,
            PlanetSequencePhase::ZoomingPlanet,
        );
        assert!(shell
            .drain_sound()
            .contains(&SoundCmd::PlayMusic(MUSIC_PLANET_ZOOM)));
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::ZoomingPlanet,
            PLANET_ZOOM_TICKS,
            PlanetSequencePhase::RevealingPlanetName,
        );
        let heading_ticks = u16::try_from(planet_heading(Sf1Planet::Corneria).len())
            .expect("planet heading fits presentation counter")
            * PLANET_NAME_CHARACTER_TICKS
            + PLANET_NAME_TERMINATION_TICKS;
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::RevealingPlanetName,
            heading_ticks,
            PlanetSequencePhase::Briefing,
        );
        assert_eq!(shell.frame().whichroute, 1);

        shell.tick(0);
        shell.tick(pad::B);
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::Briefing
        );
        shell.tick(0);
        assert_eq!(
            shell.frame().planet_presentation.phase,
            PlanetSequencePhase::DismissingBriefing
        );
        assert!(shell
            .drain_sound()
            .contains(&SoundCmd::PlaySe(PEPPER_DISMISS_SOUND)));
        assert_planet_phase_duration(
            &mut shell,
            PlanetSequencePhase::DismissingBriefing,
            BRIEFING_DISMISS_HANDOFF_TICKS,
            PlanetSequencePhase::FadingOut,
        );
        for elapsed in 1..=PLANET_EXIT_TICKS {
            shell.tick(0);
            if elapsed < PLANET_EXIT_TICKS {
                assert_eq!(
                    shell.frame().planet_presentation.phase,
                    PlanetSequencePhase::FadingOut
                );
            }
        }
        assert_eq!(shell.state(), GameState::Playing);
        assert_eq!(shell.frame().whichroute, 0);
    }

    /// AUDIT_BOSS_TICKS2 High #5: begin_gameplay writes strat `currentlevel`
    /// (WRAM 0x1F03) as planets.currentlevel+1 so `== N` matches ROM
    /// `s_jmp_iflevel N` (raw == N-1). Default map route is easy (raw 0).
    #[test]
    fn begin_gameplay_wires_currentlevel_easy() {
        let sh = into_gameplay();
        assert_eq!(sh.planets.currentlevel, 0);
        assert_eq!(sh.game.vars.shared.difficulty_level, 1);
    }

    /// Hard route (whichroute 2 / MAP_ID_3_*) stores raw currentlevel 2 →
    /// port WRAM 3 so boss7 `s_jmp_ifnotlevel 3` and mcore1 level-3 HP fire.
    #[test]
    fn begin_gameplay_wires_currentlevel_hard_route() {
        let mut sh = Shell::new();
        advance_to_planet_select(&mut sh);
        while sh.frame().planet_presentation.phase == PlanetSequencePhase::InitialSetup {
            sh.tick(0);
        }
        sh.tick(pad::RIGHT); // whichroute 1→2 = hard
        launch_from_planet_select(&mut sh);
        assert_eq!(sh.state(), GameState::Playing);
        assert_eq!(sh.planets.currentlevel, 2);
        assert_eq!(sh.game.vars.shared.difficulty_level, 3);
    }

    fn run_ending_to_staff_roll(sh: &mut Shell) {
        const MAX_ENDING_HANDOFF_TICKS: usize = 2_000;
        for _ in 0..MAX_ENDING_HANDOFF_TICKS {
            if sh.state() == GameState::Ending && sh.frame().newmap == map_id::CREDITS {
                return;
            }
            sh.tick(0);
        }
        panic!("ending did not hand off to the staff-roll map");
    }

    /// Completing the post-campaign score parade and recorded recap mutes
    /// ordinary effects, switches to staff-roll music, and loads the exact
    /// credits map.
    #[test]
    fn route_exhaust_enters_the_staff_roll() {
        let mut sh = into_gameplay();
        let _ = sh.drain_sound();
        // Exhaust the route: stage past END1 so drawplanetlines fails.
        sh.planets.stage = 20;
        sh.advance_stage_and_walk();
        assert_eq!(sh.state(), GameState::Ending);
        assert_ne!(sh.frame().newmap, map_id::CREDITS);
        run_ending_to_staff_roll(&mut sh);
        assert_eq!(sh.frame().newmap, map_id::CREDITS);
        assert_eq!(sh.frame().currentbg, 43);
        let sounds = sh.drain_sound();
        assert!(
            sounds.contains(&SoundCmd::NoSetPort3(true)),
            "ending sequence enables the sound-effect mute"
        );
        assert!(sounds.contains(&SoundCmd::PlayMusic(MUSIC_STAFF_ROLL)));
    }

    /// The source final screen is an infinite presentation, not a shortcut
    /// back to the title on controller input.
    #[test]
    fn completed_staff_roll_stays_on_the_final_score() {
        let mut sh = into_gameplay();
        sh.planets.stage = 20;
        sh.advance_stage_and_walk();
        run_ending_to_staff_roll(&mut sh);
        sh.game.world.levelfinished = le::ENDOFCREDS;

        sh.tick(0);
        assert!(sh.frame().ending_final_score_visible);
        sh.tick(pad::START | pad::A | pad::B);
        assert_eq!(sh.state(), GameState::Ending);
        assert!(sh.frame().ending_final_score_visible);
    }

    #[test]
    fn ending_replay_text_preserves_route_specific_records() {
        let sector = ending_replay_text(BossEncounter::Route2Stage4);
        assert_eq!(sector.title, "LEVEL 2");
        assert_eq!(sector.location, Some("SECTOR $"));
        assert_eq!(sector.details[0], "NAME   - PLASMA HYDRA");

        let armada = ending_replay_text(BossEncounter::Route1Stage3);
        assert_eq!(armada.location, Some("SPACE"));
        assert_eq!(armada.location_second_line, Some("ARMADA"));

        let final_record = ending_replay_text(BossEncounter::FinalBattle);
        assert_eq!(final_record.title, "FINAL");
        assert_eq!(final_record.subtitle, Some("STAGE"));
        assert_eq!(final_record.details[0], "NAME   - ANDROSS...");
    }

    #[test]
    fn ending_detail_record_reveals_one_character_after_countdown_100() {
        let mut sh = Shell::new();
        sh.game_state = GameState::Ending;
        sh.ending = EndingState {
            phase: EndingPhase::BossReplay,
            replay_ticks_remaining: ENDING_REPLAY_DETAILS_TICK + 1,
            replay_encounter: Some(BossEncounter::Route1Stage1),
            ..EndingState::default()
        };

        sh.tick(0);
        assert_eq!(
            sh.frame()
                .ending_replay
                .expect("recap remains active")
                .detail_characters_visible,
            0
        );
        sh.tick(0);
        assert_eq!(
            sh.frame()
                .ending_replay
                .expect("recap remains active")
                .detail_characters_visible,
            1
        );
        sh.tick(0);
        assert_eq!(
            sh.frame()
                .ending_replay
                .expect("recap remains active")
                .detail_characters_visible,
            2
        );
    }

    /// Drive `n` gameplay ticks with a held level-end code, so the clear
    /// settle timer elapses and the LE_* dispatch fires.
    fn run_settle(sh: &mut Shell, lf: u8) {
        sh.game.world.levelfinished = lf;
        for _ in 0..LEVEL_CLEAR_SETTLE_TICKS {
            sh.tick(0);
        }
    }

    fn run_tally_to_ready(sh: &mut Shell) {
        const MAX_TALLY_TICKS: usize = 1_000;
        for _ in 0..MAX_TALLY_TICKS {
            if sh.tally.phase == TallyPhase::Ready {
                return;
            }
            sh.tick(0);
        }
        panic!("tally did not reach its ready phase");
    }

    /// Finding 1/2: the black-hole *exit* codes (LE_BHOLE1/2/3) skip the tally
    /// and re-point routes[3] to the ROM exit destination (Venom 1 Orbital /
    /// Sector Y / Sector Z) — MAIN.ASM:306-311 + PLANETS.ASM:3107-3155.
    #[test]
    fn bhole_exit_codes_repoint_routes3() {
        use crate::planets::path_id;
        for (lf, expect) in [
            (le::BHOLE1, path_id::P19), // -> Venom 1 Orbital
            (le::BHOLE2, path_id::P18), // -> Sector Y
            (le::BHOLE3, path_id::P20), // -> Sector Z
        ] {
            let mut sh = into_gameplay();
            run_settle(&mut sh, lf);
            assert_eq!(sh.planets.routes[3], expect, "LE code {lf} routes[3]");
            // Warp path skips the tally: never enters GameState::Tally.
            assert_ne!(sh.state(), GameState::Tally, "LE code {lf} showed tally");
            assert_eq!(sh.state(), GameState::PlanetSelect);
            launch_from_planet_select(&mut sh);
            assert_eq!(sh.state().code(), 4, "LE code {lf} back to Playing");
        }
    }

    /// Finding 1/2: LE_SPECIAL re-points routes[0] -> P22 (Out of This
    /// Dimension) and sets nebula_on (routechange 1, MAIN.ASM:312).
    #[test]
    fn special_code_repoints_routes0_and_nebula() {
        use crate::planets::path_id;
        let mut sh = into_gameplay();
        run_settle(&mut sh, le::SPECIAL);
        assert_eq!(sh.planets.routes[0], path_id::P22);
        assert_eq!(sh.planets.nebula_on, path_id::P22);
        assert_eq!(sh.state(), GameState::PlanetSelect);
    }

    /// Finding 1: the `routechange 2` arm paired with LE_ENTERBHOLE walks
    /// straight into the BLACK HOLE stage.
    #[test]
    fn enterbhole_reaches_black_hole_stage() {
        let mut sh = into_gameplay();
        // The black-hole approach arms the P21 branch (routes[1]=P21).
        // Route 0, stage 1 -> after inc stage 2:
        // P6(0) -> routes[1]=P21(1) -> routes[3]=P19(2) = BLACKHOLE.
        sh.planets.routechange2();
        sh.planets.stage = 1;
        run_settle(&mut sh, le::ENTERBHOLE);
        assert_eq!(sh.frame().newmap, map_id::BLACKHOLE);
        assert_eq!(sh.state(), GameState::PlanetSelect);
    }

    /// Finding 1: with routechange 1 applied upstream, LE_ENTERSPEC walks into
    /// the SPECIAL stage ("Out of This Dimension", map SPECIAL / planet 14).
    #[test]
    fn enterspec_reaches_special_stage() {
        let mut sh = into_gameplay();
        // Venom-3 spine (route 2) with routes[0]=P22: P11(0) -> P22(1) ->
        // OTHEREND(2) = SPECIAL. Stage 1 -> after inc stage 2.
        sh.planets.whichroute = 2;
        sh.planets.routechange1();
        sh.planets.stage = 1;
        run_settle(&mut sh, le::ENTERSPEC);
        assert_eq!(sh.frame().newmap, map_id::SPECIAL);
        assert_eq!(sh.frame().currentplanet, 14);
        assert_eq!(sh.state(), GameState::PlanetSelect);
    }

    /// End-of-level tally: computes calcstageperc from specials_dead /
    /// total_specials + living teammates, records it, and awards a bonus
    /// credit + SFX on a bonertab threshold crossing.
    #[test]
    fn tally_records_stage_score_and_awards_bonus_credit() {
        let mut sh = into_gameplay();

        // 10 specials, all destroyed, all three teammates alive -> 100 + 15
        // capped to 100.
        sh.game.world.total_specials = 10;
        sh.game.vars.shared.specials_dead = 10;
        sh.game.vars.bunny_hp = 3;
        sh.game.vars.frog_hp = 3;
        sh.game.vars.falcon_hp = 3;

        sh.enter_tally();
        assert_eq!(sh.state(), GameState::Tally);
        assert_eq!(sh.tally.target_percent, 100);
        let f = sh.frame();
        assert_eq!(f.tally_stage_perc, 100);
        assert_eq!(f.tally_current_perc, 0);
        assert_eq!(f.score_total, 0); // commit is delayed like retail
        assert_eq!(f.credits, 0);
        assert!(f.tally_active);
        assert!(sh.drain_sound().is_empty());

        run_tally_to_ready(&mut sh);
        let f = sh.frame();
        assert_eq!(f.tally_current_perc, 100);
        assert_eq!(f.score_total, 100);
        assert_eq!(f.credits, 1);
        assert!(f.tally_bonus_visible);
        let sounds = sh.drain_sound();
        assert!(sounds.contains(&SoundCmd::PlaySe(score::SE_TALLY_COUNT)));
        assert!(sounds.contains(&SoundCmd::PlaySe(score::SE_TALLY_COMMIT)));
        assert!(sounds.contains(&SoundCmd::PlaySe(score::SE_BONUS)));

        // A second, weaker stage (50%, no teammates) accumulates into the
        // running total but stays below the next threshold (300) -> no credit.
        sh.game.world.total_specials = 10;
        sh.game.vars.shared.specials_dead = 5;
        sh.game.vars.bunny_hp = 0;
        sh.game.vars.frog_hp = 0;
        sh.game.vars.falcon_hp = 0;
        sh.enter_tally();
        assert_eq!(sh.tally.target_percent, 50);
        assert_eq!(sh.frame().score_total, 100);
        run_tally_to_ready(&mut sh);
        assert_eq!(sh.frame().score_total, 150);
        assert_eq!(sh.frame().credits, 1); // 150 < 300, no new credit
        assert!(!sh.frame().tally_bonus_visible);
        assert!(!sh
            .drain_sound()
            .contains(&SoundCmd::PlaySe(score::SE_BONUS)));
    }

    /// Accumulating stages across a bonertab boundary awards a second credit.
    #[test]
    fn tally_credit_awarded_when_total_crosses_next_threshold() {
        let mut sh = into_gameplay();
        sh.game.world.total_specials = 1;
        sh.game.vars.bunny_hp = 0;
        sh.game.vars.frog_hp = 0;
        sh.game.vars.falcon_hp = 0;

        // Three 100% stages -> totals 100, 200, 300. Credits at 100 and 300.
        let mut credited = 0;
        for expect_total in [100u16, 200, 300] {
            sh.game.vars.shared.specials_dead = 1;
            sh.enter_tally();
            run_tally_to_ready(&mut sh);
            assert_eq!(sh.frame().score_total, expect_total);
            if sh
                .drain_sound()
                .contains(&SoundCmd::PlaySe(score::SE_BONUS))
            {
                credited += 1;
            }
        }
        assert_eq!(credited, 2); // 100 and 300 thresholds
        assert_eq!(sh.frame().credits, 2);
    }
}
