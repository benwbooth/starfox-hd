//! Typed Star Fox route-map and General Pepper presentation state.
//!
//! The retail sequence stores these concepts in overlapping work fields.
//! The Rust port keeps the same game-domain values as ordinary flat struct
//! fields and exposes semantic phases to the renderer.

/// Source display refresh rate used to convert authored presentation spans to
/// real time.
pub const SOURCE_DISPLAY_REFRESHES_PER_SECOND: u16 = 60;
/// The native Star Fox front end advances at 20 Hz while the source display
/// runs at 60 Hz.
pub const RETAIL_VIDEO_FRAMES_PER_GAME_TICK: u16 = 3;

/// Whole-machine duration from controller-screen ownership handoff until the
/// first route-choice loop is ready. `planetseq_l` builds both buffers,
/// transfers palettes and characters, and reveals the initial map here.
pub const INITIAL_ROUTE_MAP_SETUP_TICKS: u16 = 23;

/// `4*20+3` display waits in `PLANETS.ASM`.
pub const SHIP_FLASH_RETAIL_FRAMES: u16 = 83;
/// Native ticks needed to present all authored ship-flash waits.
pub const SHIP_FLASH_TICKS: u16 =
    SHIP_FLASH_RETAIL_FRAMES.div_ceil(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);
/// Five-bit fixed-color fade from the map to the selected planet.
pub const MAP_FADE_STEPS: u8 = 32;
/// Native ticks needed for the one-video-frame fade steps.
pub const MAP_FADE_TICKS: u16 = (MAP_FADE_STEPS as u16).div_ceil(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);
/// Short transfer/window handoff between the map fade and recenter.
pub const PLANET_ISOLATION_TICKS: u16 = 2;
/// Authored selected-planet recenter duration.
pub const PLANET_CENTER_RETAIL_FRAMES: u16 = 32;
pub const PLANET_CENTER_TICKS: u16 =
    PLANET_CENTER_RETAIL_FRAMES.div_ceil(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);
/// Source zoom iterations. The first-game sphere path performs all forty, but
/// the real Super FX drawing workload makes them span 67 sampled 20 Hz ticks.
pub const PLANET_ZOOM_STEPS: u16 = 40;
pub const PLANET_ZOOM_TICKS: u16 = 67;
/// Pepper tile/palette preparation between recentering and the zoom loop.
pub const BRIEFING_PREPARATION_TICKS: u16 = 4;
/// `muttering2` advances the planet heading once per transfer-bound loop.
pub const PLANET_NAME_CHARACTER_TICKS: u16 = 1;
/// The text renderer needs two terminating cursor passes after the visible
/// planet heading has been exhausted.
pub const PLANET_NAME_TERMINATION_TICKS: u16 = 2;
/// The mission-text cursor advances three positions per five native ticks
/// until the 64-character panel workload is saturated, then once every two.
/// The initial remainder aligns the first advances with the retail samples.
pub const BRIEFING_FAST_CURSOR_LIMIT: u8 = 64;
pub const BRIEFING_FAST_CADENCE_NUMERATOR: u8 = 3;
pub const BRIEFING_FAST_CADENCE_DENOMINATOR: u8 = 5;
pub const BRIEFING_FAST_CADENCE_INITIAL_PROGRESS: u8 = 1;
pub const BRIEFING_SETTLED_CADENCE_NUMERATOR: u8 = 1;
pub const BRIEFING_SETTLED_CADENCE_DENOMINATOR: u8 = 2;
/// Input sampling and the four source sound-transfer waits observed between a
/// dismissal edge and the full-screen fade loop.
pub const BRIEFING_DISMISS_HANDOFF_TICKS: u16 = 7;
/// The five-bit exit fade samples two raster positions within each display
/// frame, so all 32 source steps still span 32 retail frames.
pub const PLANET_EXIT_FADE_RETAIL_FRAMES: u16 = MAP_FADE_STEPS as u16;
pub const PLANET_EXIT_TICKS: u16 =
    PLANET_EXIT_FADE_RETAIL_FRAMES.div_ceil(RETAIL_VIDEO_FRAMES_PER_GAME_TICK);

/// Retail map setup/fade before the first post-mission Arwing movement.
pub const POST_TALLY_MAP_REVEAL_RETAIL_FRAMES: u16 = 57;
/// Source render-loop waits after successive course targets are reached.
/// The three-part bitmap transfer produces this repeating cadence.
pub const COURSE_TARGET_HANDOFF_RETAIL_FRAMES: [u16; 3] = [4, 6, 6];
/// Route-map coordinates use eight-pixel character cells.
pub const ROUTE_MAP_CELL_SIZE: i16 = 8;
/// Source path positions target the Arwing's upper-left rather than the
/// character cell containing the line segment.
pub const ROUTE_SHIP_TARGET_X_OFFSET: i16 = 16;
pub const ROUTE_SHIP_TARGET_Y_OFFSET: i16 = 8;

/// Source map sprite radius before the close-up zoom.
pub const INITIAL_PLANET_RADIUS: u8 = 15;
/// Radius reached by the 40-step zoom from the authored initial value.
pub const FINAL_PLANET_RADIUS: u8 = INITIAL_PLANET_RADIUS + PLANET_ZOOM_STEPS as u8;

/// Map elapsed native presentation ticks onto the source's forty zoom
/// iterations. This keeps radius and portrait timing in source-step units
/// while honoring the measured 67-tick Super FX workload.
pub fn planet_zoom_step(phase_tick: u16) -> u16 {
    phase_tick
        .min(PLANET_ZOOM_TICKS)
        .saturating_mul(PLANET_ZOOM_STEPS)
        .div_ceil(PLANET_ZOOM_TICKS)
}

/// Source `PATH_ID_*` identities shared by route progression and rendering.
pub mod route_path {
    pub const INVALID: u16 = 0;
    pub const P1: u16 = 1;
    pub const P2: u16 = 2;
    pub const P3: u16 = 3;
    pub const P4: u16 = 4;
    pub const P5: u16 = 5;
    pub const P6: u16 = 6;
    pub const P7: u16 = 7;
    pub const P8: u16 = 8;
    pub const P9: u16 = 9;
    pub const P10: u16 = 10;
    pub const P11: u16 = 11;
    pub const P12: u16 = 12;
    pub const P13: u16 = 13;
    pub const P14: u16 = 14;
    pub const P15: u16 = 15;
    pub const P16: u16 = 16;
    pub const P17: u16 = 17;
    pub const P18: u16 = 18;
    pub const P19: u16 = 19;
    pub const P20: u16 = 20;
    pub const P21: u16 = 21;
    pub const P22: u16 = 22;
    pub const END3: u16 = 23;
    pub const END2: u16 = 24;
    pub const END1: u16 = 25;
    pub const OTHEREND: u16 = 26;
    pub const COUNT: u16 = 27;
}

/// Pixel position in the authored 256 by 224 route map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RouteMapPoint {
    pub x: i16,
    pub y: i16,
}

const fn point(x: i16, y: i16) -> RouteMapPoint {
    RouteMapPoint { x, y }
}

/// Top-left positions of the 32-pixel planet pictures.
pub const PLANET_MAP_POSITIONS: [RouteMapPoint; 16] = [
    point(16, 176),
    point(64, 176),
    point(56, 136),
    point(16, 128),
    point(112, 144),
    point(40, 64),
    point(128, 96),
    point(184, 144),
    point(168, 96),
    point(112, 32),
    point(80, 80),
    point(208, 96),
    point(192, 40),
    point(192, 40),
    point(136, 176),
    point(192, 40),
];

/// Source Arwing positions when a post-mission course movement begins.
pub const PLANET_SHIP_START_POSITIONS: [RouteMapPoint; 16] = [
    point(16, 176),
    point(64, 176),
    point(64, 136),
    point(16, 128),
    point(112, 144),
    point(48, 64),
    point(128, 104),
    point(184, 144),
    point(176, 96),
    point(112, 32),
    point(80, 80),
    point(208, 96),
    point(192, 40),
    point(192, 40),
    point(136, 176),
    point(192, 40),
];

/// Source Arwing destinations after the final stage-path character.
pub const PLANET_SHIP_END_POSITIONS: [RouteMapPoint; 16] = [
    point(16, 176),
    point(64, 176),
    point(56, 136),
    point(16, 128),
    point(112, 144),
    point(40, 64),
    point(128, 104),
    point(184, 144),
    point(168, 104),
    point(112, 32),
    point(80, 80),
    point(208, 96),
    point(192, 40),
    point(192, 40),
    point(136, 176),
    point(192, 40),
];

/// Route-line character drawn for a path step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutePathSegment {
    Hidden,
    DiagonalUp,
    DiagonalDown,
    Horizontal,
    Vertical,
}

/// One authored eight-pixel stage-path displacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutePathStep {
    pub dx: i16,
    pub dy: i16,
    pub segment: RoutePathSegment,
}

const fn path_step(dx: i16, dy: i16, segment: RoutePathSegment) -> RoutePathStep {
    RoutePathStep { dx, dy, segment }
}

const PATH_UP: RoutePathStep = path_step(0, -8, RoutePathSegment::Vertical);
const PATH_UP_RIGHT: RoutePathStep = path_step(8, -8, RoutePathSegment::DiagonalUp);
const PATH_RIGHT: RoutePathStep = path_step(8, 0, RoutePathSegment::Horizontal);
const PATH_HIDDEN_UP: RoutePathStep = path_step(0, -8, RoutePathSegment::Hidden);
const PATH_HIDDEN_UP_RIGHT: RoutePathStep = path_step(8, -8, RoutePathSegment::Hidden);
const PATH_HIDDEN_DOWN_RIGHT: RoutePathStep = path_step(8, 8, RoutePathSegment::Hidden);
const PATH_HIDDEN_RIGHT: RoutePathStep = path_step(8, 0, RoutePathSegment::Hidden);

const PATH_STEPS_1: [RoutePathStep; 1] = [PATH_UP];
const PATH_STEPS_2: [RoutePathStep; 3] = [PATH_UP, PATH_UP, PATH_UP_RIGHT];
const PATH_STEPS_3: [RoutePathStep; 5] = [
    PATH_UP_RIGHT,
    PATH_UP_RIGHT,
    PATH_UP_RIGHT,
    PATH_RIGHT,
    PATH_RIGHT,
];
const PATH_STEPS_4: [RoutePathStep; 4] = [PATH_RIGHT, PATH_RIGHT, PATH_RIGHT, PATH_RIGHT];
const PATH_STEPS_6: [RoutePathStep; 1] = [PATH_UP_RIGHT];
const PATH_STEPS_7: [RoutePathStep; 5] = [
    PATH_UP_RIGHT,
    PATH_UP_RIGHT,
    PATH_UP_RIGHT,
    PATH_RIGHT,
    PATH_RIGHT,
];
const PATH_STEPS_8: [RoutePathStep; 1] = [PATH_RIGHT];
const PATH_STEPS_9: [RoutePathStep; 1] = [PATH_UP];
const PATH_STEPS_11: [RoutePathStep; 2] = [PATH_RIGHT, PATH_RIGHT];
const PATH_STEPS_12: [RoutePathStep; 2] = [PATH_UP_RIGHT, PATH_UP_RIGHT];
const PATH_STEPS_13: [RoutePathStep; 5] =
    [PATH_RIGHT, PATH_RIGHT, PATH_RIGHT, PATH_RIGHT, PATH_RIGHT];
const PATH_STEPS_14: [RoutePathStep; 2] = [PATH_UP_RIGHT, PATH_UP];
const PATH_STEPS_15: [RoutePathStep; 1] = [PATH_UP];
const PATH_STEPS_17: [RoutePathStep; 3] = [
    PATH_HIDDEN_UP_RIGHT,
    PATH_HIDDEN_UP_RIGHT,
    PATH_HIDDEN_UP_RIGHT,
];
const PATH_STEPS_18: [RoutePathStep; 2] = [PATH_HIDDEN_UP_RIGHT, PATH_HIDDEN_UP_RIGHT];
const PATH_STEPS_19: [RoutePathStep; 9] = [
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_UP_RIGHT,
    PATH_HIDDEN_UP_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
];
const PATH_STEPS_20: [RoutePathStep; 12] = [
    PATH_HIDDEN_DOWN_RIGHT,
    PATH_HIDDEN_DOWN_RIGHT,
    PATH_HIDDEN_DOWN_RIGHT,
    PATH_HIDDEN_DOWN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
];
const PATH_STEPS_21: [RoutePathStep; 3] = [PATH_HIDDEN_UP, PATH_HIDDEN_UP, PATH_HIDDEN_UP_RIGHT];
const PATH_STEPS_22: [RoutePathStep; 6] = [
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
    PATH_HIDDEN_RIGHT,
];

/// Start cell and displacement stream for one source `stagepaths` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutePathGeometry {
    pub start_cell_x: i16,
    pub start_cell_y: i16,
    pub steps: &'static [RoutePathStep],
}

const fn path_geometry(
    start_cell_x: i16,
    start_cell_y: i16,
    steps: &'static [RoutePathStep],
) -> RoutePathGeometry {
    RoutePathGeometry {
        start_cell_x,
        start_cell_y,
        steps,
    }
}

const EMPTY_PATH_GEOMETRY: RoutePathGeometry = path_geometry(0, 0, &[]);

/// Exact route-line geometry indexed by source path identity.
pub const fn route_path_geometry(path: u16) -> RoutePathGeometry {
    match path {
        route_path::P1 => path_geometry(4, 20, &PATH_STEPS_1),
        route_path::P2 => path_geometry(4, 14, &PATH_STEPS_2),
        route_path::P3 => path_geometry(9, 8, &PATH_STEPS_3),
        route_path::P4 => path_geometry(18, 5, &PATH_STEPS_4),
        route_path::P6 => path_geometry(6, 21, &PATH_STEPS_6),
        route_path::P7 => path_geometry(11, 17, &PATH_STEPS_7),
        route_path::P8 => path_geometry(20, 14, &PATH_STEPS_8),
        route_path::P9 => path_geometry(24, 11, &PATH_STEPS_9),
        route_path::P11 => path_geometry(6, 23, &PATH_STEPS_11),
        route_path::P12 => path_geometry(12, 21, &PATH_STEPS_12),
        route_path::P13 => path_geometry(18, 19, &PATH_STEPS_13),
        route_path::P14 => path_geometry(27, 19, &PATH_STEPS_14),
        route_path::P15 => path_geometry(28, 11, &PATH_STEPS_15),
        route_path::P17 => path_geometry(6, 15, &PATH_STEPS_17),
        route_path::P18 => path_geometry(12, 9, &PATH_STEPS_18),
        route_path::P19 => path_geometry(13, 11, &PATH_STEPS_19),
        route_path::P20 => path_geometry(11, 14, &PATH_STEPS_20),
        route_path::P21 => path_geometry(9, 16, &PATH_STEPS_21),
        route_path::P22 => path_geometry(12, 25, &PATH_STEPS_22),
        _ => EMPTY_PATH_GEOMETRY,
    }
}

/// Source route-map planet and sector slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Sf1Planet {
    #[default]
    Corneria = 0,
    AsteroidBeltOne = 1,
    AsteroidBeltThree = 2,
    SectorX = 3,
    Fortuna = 4,
    Titania = 5,
    SpaceArmada = 6,
    SectorZ = 7,
    Meteor = 8,
    SectorY = 9,
    BlackHole = 10,
    Macbeth = 11,
    VenomRouteOne = 12,
    VenomRouteTwo = 13,
    OutOfThisDimension = 14,
    VenomRouteThree = 15,
}

impl Sf1Planet {
    pub const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::AsteroidBeltOne,
            2 => Self::AsteroidBeltThree,
            3 => Self::SectorX,
            4 => Self::Fortuna,
            5 => Self::Titania,
            6 => Self::SpaceArmada,
            7 => Self::SectorZ,
            8 => Self::Meteor,
            9 => Self::SectorY,
            10 => Self::BlackHole,
            11 => Self::Macbeth,
            12 => Self::VenomRouteOne,
            13 => Self::VenomRouteTwo,
            14 => Self::OutOfThisDimension,
            15 => Self::VenomRouteThree,
            _ => Self::Corneria,
        }
    }

    pub const fn index(self) -> u8 {
        self as u8
    }

    pub const fn is_sphere(self) -> bool {
        matches!(
            self,
            Self::Corneria | Self::Fortuna | Self::Titania | Self::Macbeth | Self::VenomRouteThree
        )
    }
}

/// General Pepper message selected by one source stage-path record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Sf1BriefingMessage {
    #[default]
    None = 0,
    CounterattackVenom = 88,
    DestroyRockCrusher = 89,
    DestroyArmadaCores = 90,
    UseRetros = 91,
    DestroyAndrossCore = 92,
    RetakeWeatherControl = 106,
    EscapeAmoeba = 107,
    CourseThree = 108,
    EscapeTractorBeam = 109,
    FortunaCreatures = 110,
    ProceedToMacbeth = 111,
    PreventMacbethBase = 112,
    BlackHoleWarning = 113,
    VenomBackDoor = 114,
    ProtectCorneria = 115,
    DestroyAndross = 116,
}

fn travel_target(
    geometry: RoutePathGeometry,
    target_index: usize,
    selected_planet: Sf1Planet,
    previous_target: RouteMapPoint,
) -> RouteMapPoint {
    if target_index < geometry.steps.len() {
        let mut cursor = point(
            geometry.start_cell_x * ROUTE_MAP_CELL_SIZE,
            geometry.start_cell_y * ROUTE_MAP_CELL_SIZE,
        );
        for step in &geometry.steps[..=target_index] {
            cursor.x += step.dx;
            cursor.y += step.dy;
        }
        point(
            cursor.x - ROUTE_SHIP_TARGET_X_OFFSET,
            cursor.y - ROUTE_SHIP_TARGET_Y_OFFSET,
        )
    } else {
        let mut destination = PLANET_SHIP_END_POSITIONS[usize::from(selected_planet.index())];
        // `moveshipalongpath` preserves the current vertical coordinate when
        // the final planet anchor is above the last route character.
        if destination.y < previous_target.y {
            destination.y = previous_target.y;
        }
        destination
    }
}

const fn travel_distance(from: RouteMapPoint, to: RouteMapPoint) -> u16 {
    let horizontal = (to.x - from.x).unsigned_abs();
    let vertical = (to.y - from.y).unsigned_abs();
    if horizontal > vertical {
        horizontal
    } else {
        vertical
    }
}

fn move_toward(from: RouteMapPoint, to: RouteMapPoint, distance: u16) -> RouteMapPoint {
    let move_axis = |start: i16, end: i16| {
        let delta = end - start;
        let amount = i16::try_from(distance.min(delta.unsigned_abs())).unwrap_or(i16::MAX);
        if delta < 0 {
            start - amount
        } else {
            start + amount
        }
    };
    point(move_axis(from.x, to.x), move_axis(from.y, to.y))
}

/// Retail display frames from post-tally map entry through the point where the
/// next mission is waiting for confirmation.
pub fn post_tally_travel_retail_frames(
    path: u16,
    previous_planet: Sf1Planet,
    selected_planet: Sf1Planet,
) -> u16 {
    let geometry = route_path_geometry(path);
    let target_count = geometry.steps.len() + 1;
    let mut position = PLANET_SHIP_START_POSITIONS[usize::from(previous_planet.index())];
    let mut frames = POST_TALLY_MAP_REVEAL_RETAIL_FRAMES;
    for target_index in 0..target_count {
        let target = travel_target(geometry, target_index, selected_planet, position);
        frames = frames
            .saturating_add(travel_distance(position, target))
            .saturating_add(
                COURSE_TARGET_HANDOFF_RETAIL_FRAMES
                    [target_index % COURSE_TARGET_HANDOFF_RETAIL_FRAMES.len()],
            );
        position = target;
    }
    frames
}

/// Authored Arwing position at one display frame of the post-tally route-map
/// movement. This models the source's one-pixel-per-display-frame movement
/// and its three-part bitmap-transfer handoff cadence.
pub fn post_tally_ship_position(
    path: u16,
    previous_planet: Sf1Planet,
    selected_planet: Sf1Planet,
    retail_frame: u16,
) -> RouteMapPoint {
    let geometry = route_path_geometry(path);
    let target_count = geometry.steps.len() + 1;
    let mut position = PLANET_SHIP_START_POSITIONS[usize::from(previous_planet.index())];
    let mut remaining = retail_frame.saturating_sub(POST_TALLY_MAP_REVEAL_RETAIL_FRAMES);
    if retail_frame <= POST_TALLY_MAP_REVEAL_RETAIL_FRAMES {
        return position;
    }

    for target_index in 0..target_count {
        let target = travel_target(geometry, target_index, selected_planet, position);
        let distance = travel_distance(position, target);
        if remaining <= distance {
            return move_toward(position, target, remaining);
        }
        remaining -= distance;
        position = target;

        let handoff = COURSE_TARGET_HANDOFF_RETAIL_FRAMES
            [target_index % COURSE_TARGET_HANDOFF_RETAIL_FRAMES.len()];
        if remaining <= handoff {
            return position;
        }
        remaining -= handoff;
    }
    position
}

/// English planet heading selected by the retail `planetnames` table.
pub const fn planet_heading(planet: Sf1Planet) -> &'static str {
    match planet {
        Sf1Planet::Corneria => "        CORNERIA - THE BASE",
        Sf1Planet::AsteroidBeltOne | Sf1Planet::AsteroidBeltThree => "           ASTEROID BELT",
        Sf1Planet::SectorX => "              SECTOR  X",
        Sf1Planet::Fortuna => "       THE PLANET FORTUNA",
        Sf1Planet::Titania => "        THE PLANET TITANIA",
        Sf1Planet::SpaceArmada => "     THE ANDROSS SPACE ARMADA",
        Sf1Planet::SectorZ => "              SECTOR  Z",
        Sf1Planet::Meteor => "      THE BATTLE BASE METEOR",
        Sf1Planet::SectorY => "              SECTOR  Y",
        Sf1Planet::BlackHole => "      THE AWESOME BLACK HOLE",
        Sf1Planet::Macbeth => "        THE PLANET MACBETH",
        Sf1Planet::VenomRouteOne | Sf1Planet::VenomRouteTwo | Sf1Planet::VenomRouteThree => {
            "      VENOM - THE FINAL GOAL"
        }
        Sf1Planet::OutOfThisDimension => "      OUT OF THIS DIMENSION",
    }
}

/// English General Pepper message selected by the stage-path table.
pub const fn briefing_text(message: Sf1BriefingMessage) -> &'static str {
    match message {
        Sf1BriefingMessage::CounterattackVenom => {
            "STAR FOX TEAM, OUR LAST RESORT IS TO COUNTER ATTACK VENOM!  GOOD LUCK!"
        }
        Sf1BriefingMessage::DestroyRockCrusher => {
            "ANDROSS'S FORCES INTEND TO BUILD A BASE IN THIS AREA!  DESTROY THEIR ROCK CRUSHER!"
        }
        Sf1BriefingMessage::DestroyArmadaCores => {
            "THE SPACE ARMADA CONSISTS OF POWERFUL BATTLESHIPS! DESTROY THEIR ENERGY CORES!"
        }
        Sf1BriefingMessage::UseRetros => {
            "BE SURE TO USE YOUR RETROS IF YOU'RE GOING TOO FAST!  BE CAREFUL WITH MY ARWINGS!"
        }
        Sf1BriefingMessage::DestroyAndrossCore => {
            "ANDROSS IS HIDING ON VENOM!  FOX, YOU MUST FIND HIS CORE BRAIN AND DESTROY IT!"
        }
        Sf1BriefingMessage::RetakeWeatherControl => {
            "CORNERIA'S RESOURCE WORLD HAS BEEN OVERRUN!  YOU MUST RE-TAKE THE WEATHER CONTROL UNIT!"
        }
        Sf1BriefingMessage::EscapeAmoeba => {
            "HOW ARE THE ARWINGS HANDLING?  IF AN AMOEBA CLINGS TO YOUR SHIP, USE L OR R TO GET RID OF IT."
        }
        Sf1BriefingMessage::CourseThree => {
            "YOU'VE CHOSEN COURSE THREE...  A GOOD CHOICE TO TAKE VENOM BY SURPRISE!"
        }
        Sf1BriefingMessage::EscapeTractorBeam => {
            "USE THE L OR R BUTTON TO ESCAPE THE TRACTOR BEAM OF THE ENEMY BATTLESHIP! YOU CAN DO IT, FOX!"
        }
        Sf1BriefingMessage::FortunaCreatures => {
            "ANDROSS HAS TAKEN CONTROL OF THE HUGE CREATURES WHO LIVE ON FORTUNA!  TAKE CARE, FOX!"
        }
        Sf1BriefingMessage::ProceedToMacbeth => {
            "YOUR TEAM IS DOING WELL, FOX!  I HOPE YOU'RE TAKING GOOD CARE OF MY ARWINGS!  GO FOR MACBETH!"
        }
        Sf1BriefingMessage::PreventMacbethBase => {
            "THE HOLLOW INTERIOR OF MACBETH IS IDEAL FOR A BASE!  PREVENT ANDROSS FROM BUILDING HERE!"
        }
        Sf1BriefingMessage::BlackHoleWarning => {
            "THIS SPACE GRAVE YARD, CREATED BY ANDROSS'S EXPERIMENTS, IS WHERE YOUR FATHER VANISHED, FOX!"
        }
        Sf1BriefingMessage::VenomBackDoor => {
            "IS EVERYONE ALL RIGHT, FOX?!  YOU'RE ON COURSE TO SNEAK INTO VENOM'S BACK DOOR!"
        }
        Sf1BriefingMessage::ProtectCorneria => {
            "COME IN, ARWINGS!  FOX, WHERE ARE YOU?!  WE NEED YOU TO PROTECT CORNERIA!"
        }
        Sf1BriefingMessage::DestroyAndross => {
            "YOU'VE MADE IT THIS FAR... IT'S YOUR FATE TO DESTROY ANDROSS!  WE'RE COUNTING ON YOU, FOX!"
        }
        Sf1BriefingMessage::None => "",
    }
}

/// Semantic phase of `planetseq_l`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanetSequencePhase {
    /// Initial route-map construction and reveal before input is sampled.
    InitialSetup,
    /// First-game route choice with blinking route lines.
    #[default]
    RouteSelection,
    /// Later-stage Arwing movement along the completed course segment.
    Traveling,
    /// Later-stage map hold while the next mission awaits confirmation.
    AwaitingConfirmation,
    /// Confirmed Arwing palette flash.
    ShipFlash,
    /// Fade everything except the selected planet.
    FadingMap,
    /// Clear the route-map bitmap and retain only the selected planet.
    IsolatingPlanet,
    /// Smooth 32-frame move to the authored close-up center.
    CenteringPlanet,
    /// Transfer Pepper's portrait/background assets before the close-up loop.
    PreparingBriefing,
    /// Forty-step planet close-up and light rotation.
    ZoomingPlanet,
    /// Type-on planet heading.
    RevealingPlanetName,
    /// General Pepper mission briefing.
    Briefing,
    /// Dismissal sound and transfer handoff before the exit fade.
    DismissingBriefing,
    /// Full-screen fade into gameplay.
    FadingOut,
}

/// Flat presentation fields consumed by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetPresentation {
    pub phase: PlanetSequencePhase,
    /// Native ticks elapsed in the active semantic phase.
    pub phase_tick: u16,
    /// Monotonic map-loop step used for planet surface rotation and line blink.
    pub rotation_tick: u16,
    /// Planet retained by the source window and used for the briefing.
    pub selected_planet: Sf1Planet,
    /// Mission-briefing message selected by the stage-path walk.
    pub briefing_message: Sf1BriefingMessage,
    /// Planet occupied before a later-stage course movement.
    pub previous_planet: Sf1Planet,
    /// Source stage-path identity traversed by the Arwing.
    pub travel_path_id: u16,
    /// Retail display frame currently represented by the active course move.
    pub travel_retail_frame: u16,
    /// Retail display frames through the mission-confirmation hold.
    pub travel_retail_frames: u16,
    /// Five-bit map fade level, from fully visible to black.
    pub map_fade_level: u8,
    /// Source planet radius used by the close-up renderer.
    pub planet_radius: u8,
    /// Number of planet-heading characters currently visible.
    pub planet_name_characters: u8,
    /// Number of General Pepper briefing characters currently visible.
    pub briefing_characters: u8,
    /// Fractional mission-text cursor progress in source presentation units.
    pub briefing_cadence_progress: u8,
    /// A sampled dismissal edge awaiting the source's next presentation pass.
    pub briefing_dismissal_pending: bool,
    /// Fixed-rate handoff ticks from a sampled route-confirmation edge into
    /// the source ship-flash sequence.
    pub route_confirmation_ticks_remaining: u8,
}

impl Default for PlanetPresentation {
    fn default() -> Self {
        Self {
            phase: PlanetSequencePhase::RouteSelection,
            phase_tick: 0,
            rotation_tick: 0,
            selected_planet: Sf1Planet::Corneria,
            briefing_message: Sf1BriefingMessage::None,
            previous_planet: Sf1Planet::Corneria,
            travel_path_id: 0,
            travel_retail_frame: 0,
            travel_retail_frames: 0,
            map_fade_level: 0,
            planet_radius: INITIAL_PLANET_RADIUS,
            planet_name_characters: 0,
            briefing_characters: 0,
            briefing_cadence_progress: BRIEFING_FAST_CADENCE_INITIAL_PROGRESS,
            briefing_dismissal_pending: false,
            route_confirmation_ticks_remaining: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_zoom_ticks_cover_every_source_step() {
        assert_eq!(planet_zoom_step(0), 0);
        assert_eq!(planet_zoom_step(PLANET_ZOOM_TICKS), PLANET_ZOOM_STEPS);
        assert_eq!(planet_zoom_step(PLANET_ZOOM_TICKS + 1), PLANET_ZOOM_STEPS);

        let mut previous_step = 0;
        for tick in 1..=PLANET_ZOOM_TICKS {
            let step = planet_zoom_step(tick);
            assert!(step >= previous_step);
            assert!(step <= PLANET_ZOOM_STEPS);
            previous_step = step;
        }
    }

    #[test]
    fn hard_route_first_travel_matches_retail_oracle() {
        let frames = post_tally_travel_retail_frames(
            route_path::P11,
            Sf1Planet::Corneria,
            Sf1Planet::AsteroidBeltOne,
        );
        assert_eq!(frames, 121);

        assert_eq!(
            post_tally_ship_position(
                route_path::P11,
                Sf1Planet::Corneria,
                Sf1Planet::AsteroidBeltOne,
                57,
            ),
            point(16, 176)
        );
        assert_eq!(
            post_tally_ship_position(
                route_path::P11,
                Sf1Planet::Corneria,
                Sf1Planet::AsteroidBeltOne,
                81,
            ),
            point(40, 176)
        );
        assert_eq!(
            post_tally_ship_position(
                route_path::P11,
                Sf1Planet::Corneria,
                Sf1Planet::AsteroidBeltOne,
                93,
            ),
            point(48, 176)
        );
        assert_eq!(
            post_tally_ship_position(
                route_path::P11,
                Sf1Planet::Corneria,
                Sf1Planet::AsteroidBeltOne,
                frames,
            ),
            point(64, 176)
        );
    }
}
