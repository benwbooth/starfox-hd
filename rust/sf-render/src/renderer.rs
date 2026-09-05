//! Top-level renderer: owns every pass and mirrors the C pass order.
//!
//! Port (C oracle): `src/renderer/renderer.c`. `Renderer::submit` runs the
//! exact `Renderer_SubmitDrawList` sequence: view lerp -> Bg2d -> DrawList
//! 3D (incl. shadow pass) -> particles -> HUD -> state UI -> fade.
//!
//! Game-state inputs arrive via the plain [`FrameInputs`] struct so this
//! crate does not depend on sf-game; sf-app bridges its globals in.

use std::path::{Path, PathBuf};

use crate::bg2d::Bg2d;
use crate::draw_list::{
    project_draw_object_origin, project_world_origin, DrawListEntry, DrawListRenderer, ShadowStyle,
    SourceSceneCamera,
};
use crate::font::Font;
use crate::gpu::{Gpu, RenderViewport, TextureId, Vertex2, WHITE_TEX};
use crate::hud::Hud;
use crate::particles::Particles;
use crate::shapes_gl::ShapeStore;
use crate::sprites::Sprites;
use crate::transform::Transform;
use crate::ui::Ui;
use sf_core::{
    player_view::PlayerViewMode,
    point_field::PointPixel,
    scene::{
        PaletteFadeTarget, SceneStyle, BG2_HORIZONTAL_OFFSET_ROWS, BG2_VERTICAL_OFFSET_COLUMNS,
    },
    screen_fill_circle::{
        ScreenFillCircleCenter, ScreenFillCircleScope, ScreenFillCircleState, MAX_COLOR_LEVEL,
    },
    screen_wipe::ScreenWipeState,
    sf1_controls::{BriefingChoice, BriefingPhase, ControlType},
    sf1_planets::PlanetPresentation,
    stage_banner::{ScrambleBannerState, StageBannerState},
};

const SOURCE_POLYGON_GAMEPLAY_PRESENTATION_OFFSET: [i16; 2] = [0, 0];
const SOURCE_POLYGON_DEFAULT_PRESENTATION_OFFSET: [i16; 2] = [0, 0];
const SOURCE_BITMAP_LEFT: f32 = 16.0;
const SOURCE_BITMAP_WIDTH: f32 = 224.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SourceBitmapAperture {
    top: f32,
    height: f32,
}

const SF1_TITLE_SOURCE_APERTURE: SourceBitmapAperture = SourceBitmapAperture {
    // The title unblanks immediately after line 16 and blanks immediately
    // after line 206, so its completed source scanout exposes lines 17..=206.
    top: 17.0,
    height: 190.0,
};
const SF1_FLIGHT_SOURCE_APERTURE: SourceBitmapAperture = SourceBitmapAperture {
    top: 16.0,
    height: 190.0,
};

/// Semantic presentation state shared by the native game and renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameState {
    #[default]
    Boot,
    AttractIntro,
    Title,
    Briefing,
    PlanetSelect,
    Playing,
    Continue,
    Ending,
    Tally,
}

/// Native recap artwork selected by the semantic ending state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndingReplayBackdrop {
    RisingGradient,
    SplitGradient,
}

/// Typed ending presentation data. This crosses the game/renderer boundary
/// without exposing source-machine storage or processor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndingReplayInputs<'a> {
    pub backdrop: EndingReplayBackdrop,
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub location: Option<&'a str>,
    pub location_second_line: Option<&'a str>,
    pub details: [&'a str; 3],
    pub detail_characters_visible: u8,
}

/// Native Star Fox 2 presentation phase. These semantic values cross the
/// game/renderer boundary; no source-machine mode numbers enter the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2Mode {
    Intro,
    Title,
    Records,
    Briefing,
    StrategicMap,
    PilotSelection,
    Mission,
    GameOver,
    Results,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2GameOverChoice {
    ContinueWithWingmate,
    EndCampaign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2GameOverPhase {
    AndrossTaunt,
    Choosing,
    Leaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2ResultsChoice {
    Retry,
    Title,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2ResultsPhase {
    Revealing,
    OpeningChoices,
    Choosing,
    Leaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2EndingPhase {
    StaffRoll,
    EndScreen,
    Leaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2Pilot {
    Fox,
    Falco,
    Peppy,
    Slippy,
    Miyu,
    Fay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2PilotSelectionPhase {
    Revealing,
    ChoosingPrimary,
    ChoosingWingmate,
    Ready,
    Launching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2PilotSelectionCursor {
    Pilot(Sf2Pilot),
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2FlightControlStyle {
    TypeA,
    TypeB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2TitlePage {
    MainMenu,
    Difficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2TitleMenuItem {
    Mission,
    Records,
    SoundMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2Difficulty {
    Normal,
    Hard,
    Expert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2AudioOutput {
    Stereo,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2StrategicPhase {
    Overview,
    Tutorial,
    Planning,
    Traveling,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sf2MapPoint {
    pub x: i16,
    pub y: i16,
}

pub const SF2_STRATEGIC_MAP_ACTOR_CAPACITY: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2StrategicActorKind {
    NorthernInstallation,
    SouthernInstallation,
    EnemyCarrier,
    EnemyFormation,
    EasternInterceptor,
    PatrolShip,
    MissileTrail,
    Missile,
    AttackingFighter,
    RivalFighter,
    FighterProjectile,
    UnknownSignal,
    DefensePlatform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2StrategicActorAppearance {
    OpeningAssault,
    EscalatedAssault,
    PostInterception,
    PostFighterIntercept,
    PostPigma,
    PostEladard,
    PostCarrier,
    PostLeon,
    PostMirage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sf2StrategicActor {
    pub kind: Sf2StrategicActorKind,
    pub appearance: Sf2StrategicActorAppearance,
    pub position: Sf2MapPoint,
}

pub const SF2_RADAR_CONTACT_CAPACITY: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sf2RadarContact {
    pub lateral: i16,
    pub forward: i16,
    pub friendly: bool,
}

/// Semantic retail PPU background selected for a native SF2 mission scene.
/// The 3D environment remains in the ordinary draw list; these variants hold
/// only the independently isolated background layer behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MissionBackdrop {
    DeepSpace,
    VenomSurface,
    EladardSurface,
    EladardInterior,
    TitaniaBase,
    MacbethSurface,
    MeteorSurface,
    FortunaSurface,
    CarrierInterior,
    AstropolisVoid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MissionMessage {
    FlyFasterByPressingYButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MissionMessageIrisFrame {
    ThinLine,
    EmptyPanel,
    SparseInterference,
    DenseInterference,
    FullInterference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sf2MissionMessagePhase {
    Opening(Sf2MissionMessageIrisFrame),
    Open { portrait_talking: bool },
    Closing(Sf2MissionMessageIrisFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sf2MissionMessageInputs {
    pub message: Sf2MissionMessage,
    pub phase: Sf2MissionMessagePhase,
}

/// Plain typed SF2 state consumed by native UI passes. It deliberately mirrors
/// the game-domain fields rather than a memory block or processor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sf2FrameInputs {
    pub mode: Sf2Mode,
    pub intro_presentation_tick: u16,
    pub intro_title_menu_countdown: Option<u8>,
    pub polygon_palette: crate::shapes::Sf2PolygonPalette,
    pub mission_backdrop: Sf2MissionBackdrop,
    pub title_page: Sf2TitlePage,
    pub title_menu_item: Sf2TitleMenuItem,
    pub difficulty: Sf2Difficulty,
    pub audio_output: Sf2AudioOutput,
    pub pilot_selection_phase: Sf2PilotSelectionPhase,
    pub pilot_selection_cursor: Sf2PilotSelectionCursor,
    pub flight_control_style: Sf2FlightControlStyle,
    pub primary_pilot: Option<Sf2Pilot>,
    pub wingmate: Option<Sf2Pilot>,
    pub game_over_phase: Sf2GameOverPhase,
    pub game_over_choice: Sf2GameOverChoice,
    pub game_over_transition_retail_frames: u16,
    pub results_phase: Sf2ResultsPhase,
    pub results_choice: Sf2ResultsChoice,
    pub results_presentation_retail_frames: u32,
    pub results_transition_retail_frames: u16,
    pub ending_phase: Sf2EndingPhase,
    pub ending_presentation_tick: u32,
    pub ending_transition_retail_frames: u16,
    pub primary_shield: u8,
    pub wingmate_shield: u8,
    pub item_count: u8,
    pub target_count: u8,
    pub mission_elapsed_time_tenths: u16,
    pub mission_message: Option<Sf2MissionMessageInputs>,
    pub radar_contacts: [Option<Sf2RadarContact>; SF2_RADAR_CONTACT_CAPACITY],
    pub mode_frame: u32,
    pub elapsed_campaign_frames: u64,
    pub corneria_damage_percent: u8,
    pub score: u32,
    pub campaign_sorties_completed: u16,
    pub strategic_opening_presentation_tick: u16,
    pub strategic_phase: Sf2StrategicPhase,
    pub strategic_marker_phase: u8,
    pub strategic_player: Sf2MapPoint,
    pub strategic_destination: Sf2MapPoint,
    pub strategic_actors: [Option<Sf2StrategicActor>; SF2_STRATEGIC_MAP_ACTOR_CAPACITY],
}

// BGS.ASM flag (src/game/bgs.h).
pub const BGF_BG: u8 = 0x04;

// WINDOWS.ASM fade modes (src/game/game_vars.h).
pub const WINDOWARRAY_SIZE: usize = 8;
pub const WINDOW_MODE_NONE: u8 = 0;
pub const WINDOW_MODE_BLACK: u8 = 1;
pub const WINDOW_MODE_WHITEFADE: u8 = 2;
pub const WINDOW_MODE_WHITE2NORM: u8 = 3;
pub const WINDOW_MODE_MAPFADE: u8 = 4;
/// Full source display brightness.
pub const DISPLAY_BRIGHTNESS_MAX: u8 = 15;
/// Maximum source fixed-colour component.
const FIXED_COLOR_COMPONENT_MAX: u8 = 31;
const SOURCE_CLEAR_COMPONENT: f32 = 0.0;
const SF2_SIDEBAR_BLUE_COMPONENT: f32 = 0.05;

/// Mirror of the C `WindowState` (fade slot).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowState {
    pub mode: u8,
    pub wm_val: u8,
    pub stayblack: u8,
}

/// Per-frame game-state inputs for the render passes — the plain-data
/// replacement for the C globals each pass read directly.
#[derive(Debug, Clone)]
pub struct FrameInputs<'a> {
    /// Present only when rendering the native Star Fox 2 game.
    pub sf2: Option<Sf2FrameInputs>,

    /// Present the unfiltered 256-by-224 SF1 source frame, including the
    /// hardware-authored flight-playfield mask. The normal HD presentation
    /// leaves this false so optional visual styling remains independent.
    pub source_resolution: bool,
    /// Completed-scene pitch used by the source background transfer. This is
    /// separate from the later presentation camera during strict capture.
    pub source_background_pitch: Option<u16>,
    /// Optional completed-scene camera for strict source capture. This keeps
    /// source bitmap projection independent from the later live BG2 camera.
    pub source_scene_camera: Option<SourceSceneCamera>,

    // Global state (boot.h / game_vars.h)
    pub game_state: GameState,
    /// g_currentbg (last setbg operand).
    pub currentbg: u16,
    /// g_newmap (currently loaded map id, levels.h MAP_ID_*).
    pub newmap: u32,
    /// g_bgflags (BGF_*).
    pub bgflags: u8,
    /// g_bg2Xscroll.
    pub bg2_xscroll: i32,
    /// Source Mode-2 vertical offset for each eight-pixel display column.
    pub bg2_vertical_offsets: Option<[i16; BG2_VERTICAL_OFFSET_COLUMNS]>,
    /// Source rotating-ground horizontal placement for every display row.
    pub bg2_horizontal_offsets: Option<[i16; BG2_HORIZONTAL_OFFSET_ROWS]>,
    /// g_nomaxbg2Yscroll.
    pub nomax_bg2_yscroll: bool,
    /// Typed polygon lighting and shadow-plane state selected by the active
    /// background script.
    pub scene_style: SceneStyle,
    /// Source-resolution pixels emitted by the native point-field simulation.
    pub point_pixels: &'a [PointPixel],
    /// Preceding fixed-update point field used during the open interpolation
    /// interval. `None` means the caller has no presentation history.
    pub previous_point_pixels: Option<&'a [PointPixel]>,

    // Background palette-row fade (map-VM FADETOSEA/FADETOGROUND,
    // WORLD.ASM:371-394; consumer fadepalto_l MAIN.ASM:2762).
    pub pal_target: Option<PaletteFadeTarget>,
    /// ROM `palnum` remaining (starts 30, two bytes per tick to zero).
    pub palfade_num: u16,

    // Window / fade state (WINDOWS.ASM)
    /// g_windowmode bitmask of allocated slots.
    pub windowmode: u8,
    pub windows: [WindowState; WINDOWARRAY_SIZE],
    /// Typed 0..15 source display brightness. This is visible game state,
    /// not a source-machine register model.
    pub display_brightness: u8,
    /// Typed scene-transfer blanking, independent of visible brightness.
    pub display_forced_blank: bool,
    /// Typed fixed-colour subtraction published by the black-window lane.
    pub display_black_subtraction: u8,
    /// Native source-authored playfield reveal.
    pub screen_wipe: ScreenWipeState,
    /// Retail fixed-colour circle presentation.
    pub screen_fill_circle: ScreenFillCircleState,

    // HUD state
    /// g_meters.
    pub meters: u16,
    /// g_stayblack (-1 = normal gameplay control path).
    pub stayblack: i8,
    /// g_gameflags (GF_*).
    pub gameflags: u8,
    /// g_gameframe.
    pub gameframe: u16,
    /// Typed CONT.ASM controller-screen interaction phase.
    pub briefing_phase: BriefingPhase,
    /// Typed CONT.ASM TRAINING/GAME selection.
    pub briefing_choice: BriefingChoice,
    /// Active one of the four source controller layouts.
    pub control_type: ControlType,
    /// g_boostcnt.
    pub boostcnt: u8,
    /// g_arrows (SPRAR_*).
    pub arrows: u8,
    /// Typed player camera mode.
    pub player_view_mode: PlayerViewMode,
    /// g_stage.
    pub stage: u16,
    /// Typed stage announcement selected and timed by the native game.
    pub stage_banner: Option<StageBannerState>,
    /// Typed launch warning selected and timed by the native game.
    pub scramble_banner: Option<ScrambleBannerState>,
    /// Hud_SetShield current value (player HP).
    pub shield_cur: i32,
    pub shield_max: i32,
    pub boss_hp_cur: i32,
    pub boss_hp_max: i32,
    pub lives: i32,
    pub bombs: i32,
    /// ROM `specflash` — nova-bomb HUD blink timer (SPRITES.ASM:673).
    pub specflash: u8,
    /// ROM `shieldup` — wireframe-shield meter fill uses color 7 when set.
    pub shieldup: u8,

    // Radio messages (CONTINUE.ASM)
    /// g_msg_count1.
    pub msg_count1: u8,
    /// g_msg_count2.
    pub msg_count2: u8,
    /// Typed portrait frame selected by the radio-message state machine.
    pub radio_face_frame: u8,
    /// g_whichfriend.
    pub whichfriend: u8,
    /// g_friends_meter.
    pub friends_meter: u8,
    /// Strings_GetActiveMessageText().
    pub message_text: Option<&'a str>,

    // Planet select (PLANETS.ASM)
    /// g_whichroute.
    pub whichroute: u8,
    /// g_currentplanet.
    pub currentplanet: i16,
    /// g_nebula_on.
    pub nebula_on: u16,
    /// Planets_GetRoutePathIds(g_whichroute) — PATH_ID_* sequence.
    pub route_path_ids: &'a [u16],
    /// Typed route-map and General Pepper presentation.
    pub planet_presentation: PlanetPresentation,

    // Score / tally (MAIN.ASM end_level_seq; PLANETS.ASM drawroutename)
    /// Running total hit-percentage score (ROM calctotalscore/tpa). Drawn on
    /// the map screen as 3 digits + "00" (PLANETS.ASM:1583-1595).
    pub score: u16,
    /// Bonus continue credits (ROM `credits`).
    pub credits: u8,
    /// End-of-level tally screen active — overlay the stage % + total.
    pub tally_active: bool,
    /// Just-finished stage hit percentage for the tally overlay (ROM cla2).
    pub tally_stage_perc: u8,
    /// Animated stage graph value (ROM cla1).
    pub tally_current_perc: u8,
    /// Peppy, Falco, and Slippy shield values in tally-screen order.
    pub tally_teammate_shields: [u8; 3],
    /// True after the delayed `BONUS 1 CREDIT` announcement replaces SCORE.
    pub tally_bonus_visible: bool,
    /// Active native boss recap, including exact text-reveal progress.
    pub ending_replay: Option<EndingReplayInputs<'a>>,
}

impl<'a> Default for FrameInputs<'a> {
    fn default() -> Self {
        FrameInputs {
            sf2: None,
            source_resolution: false,
            source_background_pitch: None,
            source_scene_camera: None,
            game_state: GameState::Boot,
            currentbg: 0,
            newmap: 0,
            bgflags: 0,
            bg2_xscroll: 0,
            bg2_vertical_offsets: None,
            bg2_horizontal_offsets: None,
            nomax_bg2_yscroll: false,
            scene_style: SceneStyle::default(),
            point_pixels: &[],
            previous_point_pixels: None,
            pal_target: None,
            palfade_num: 0,
            windowmode: 0,
            windows: [WindowState::default(); WINDOWARRAY_SIZE],
            display_brightness: DISPLAY_BRIGHTNESS_MAX,
            display_forced_blank: false,
            display_black_subtraction: 0,
            screen_wipe: ScreenWipeState::inactive(),
            screen_fill_circle: ScreenFillCircleState::inactive(),
            meters: 0,
            stayblack: -1,
            gameflags: 0,
            gameframe: 0,
            briefing_phase: BriefingPhase::ControlType,
            briefing_choice: BriefingChoice::Training,
            control_type: ControlType::A,
            boostcnt: 0,
            arrows: 0,
            player_view_mode: PlayerViewMode::Exterior,
            stage: 0,
            stage_banner: None,
            scramble_banner: None,
            shield_cur: 40,
            shield_max: 40,
            boss_hp_cur: 0,
            boss_hp_max: 0,
            lives: 3,
            bombs: 3,
            specflash: 0,
            shieldup: 0,
            msg_count1: 0,
            msg_count2: 0,
            radio_face_frame: 0,
            whichfriend: 0,
            friends_meter: 0,
            message_text: None,
            score: 0,
            credits: 0,
            tally_active: false,
            tally_stage_perc: 0,
            tally_current_perc: 0,
            tally_teammate_shields: [0; 3],
            tally_bonus_visible: false,
            ending_replay: None,
            whichroute: 0,
            currentplanet: -1,
            nebula_on: 0,
            route_path_ids: &[],
            planet_presentation: PlanetPresentation::default(),
        }
    }
}

/// Renderer configuration: where to find shader files and game assets.
#[derive(Debug, Clone, Default)]
pub struct RendererConfig {
    /// Directory containing `flat.vert.glsl` / `flat.frag.glsl` (the
    /// canonical GLSL lives in `rust/sf-render/shaders/`). Falls back to
    /// embedded sources (byte-equal) when `None` or missing.
    pub shader_dir: Option<PathBuf>,
    /// Root containing the `data/` asset tree (repo root).
    pub asset_root: PathBuf,
    /// HD polygon-shading and ground-shadow presentation.
    pub shadow_style: ShadowStyle,
}

pub struct Renderer {
    pub gpu: Gpu,
    width: i32,
    height: i32,

    pub transform: Transform,
    pub shapes: ShapeStore,
    pub draw_list: DrawListRenderer,
    pub bg2d: Bg2d,
    pub font: Font,
    pub sprites: Sprites,
    pub hud: Hud,
    pub ui: Ui,
    pub particles: Particles,
    native_frame_texture: Option<TextureId>,
    native_frame_size: (u32, u32),
    shadow_style: ShadowStyle,
}

const SOURCE_FRAME_WIDTH: u32 = 256;
const SOURCE_FRAME_HEIGHT: u32 = 224;

fn source_frame_viewport(width: i32, height: i32) -> RenderViewport {
    let output_width = width.max(1) as u32;
    let output_height = height.max(1) as u32;
    let (viewport_width, viewport_height) = if u64::from(output_width)
        * u64::from(SOURCE_FRAME_HEIGHT)
        > u64::from(output_height) * u64::from(SOURCE_FRAME_WIDTH)
    {
        let scaled_width = (u64::from(output_height) * u64::from(SOURCE_FRAME_WIDTH)
            + u64::from(SOURCE_FRAME_HEIGHT / 2))
            / u64::from(SOURCE_FRAME_HEIGHT);
        (scaled_width as u32, output_height)
    } else {
        let scaled_height = (u64::from(output_width) * u64::from(SOURCE_FRAME_HEIGHT)
            + u64::from(SOURCE_FRAME_WIDTH / 2))
            / u64::from(SOURCE_FRAME_WIDTH);
        (output_width, scaled_height as u32)
    };
    RenderViewport {
        x: (output_width - viewport_width) / 2,
        y: (output_height - viewport_height) / 2,
        width: viewport_width,
        height: viewport_height,
    }
}

impl Renderer {
    /// Mirror of `Renderer_Init`: build shaders, init every pass, set the
    /// projection, register all shapes, set global GL state.
    pub fn new(
        mut gpu: Gpu,
        width: i32,
        height: i32,
        config: &RendererConfig,
    ) -> Result<Self, String> {
        let width = width.max(1);
        let height = height.max(1);
        let mut transform = Transform::new();
        let draw_list = DrawListRenderer::new();
        let hud = Hud::new(&mut gpu);
        let particles = Particles::new(&mut gpu);
        let font = Font::new(&mut gpu, &config.asset_root);
        let sprites = Sprites::new(&mut gpu, &config.asset_root);
        let bg2d = Bg2d::new(&mut gpu, &config.asset_root);
        let ui = Ui::new(&mut gpu, &config.asset_root);

        // Set initial projection (also done in resize)
        transform.set_projection(width, height);

        // Register built-in shapes (Arwing, etc.)
        let mut shapes = ShapeStore::new();
        shapes.register_builtins(&mut gpu);

        // CGRAM color zero is black in the retail game. Scene backdrops and
        // point fields supply any authored sky color above that base.
        gpu.set_clear_color(0.0, 0.0, 0.0, 1.0);

        println!("Renderer initialized ({width}x{height})");

        Ok(Renderer {
            gpu,
            width,
            height,
            transform,
            shapes,
            draw_list,
            bg2d,
            font,
            sprites,
            hud,
            ui,
            particles,
            native_frame_texture: None,
            native_frame_size: (0, 0),
            shadow_style: config.shadow_style,
        })
    }

    /// Headless renderer for offscreen tests: renders into a texture that
    /// `read_pixels_rgb` reads back. No window or surface required.
    pub fn new_headless(width: i32, height: i32, config: &RendererConfig) -> Result<Self, String> {
        let gpu = Gpu::new_headless(width.max(1) as u32, height.max(1) as u32)?;
        Self::new(gpu, width, height, config)
    }

    /// Mirror of `Renderer_Resize`.
    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.gpu.resize(self.width as u32, self.height as u32);
        self.transform.set_projection(self.width, self.height);
        self.font.set_screen_size(self.width, self.height);
    }

    /// Advance the HD presentation history for source-authored per-line
    /// background placement. The integer tables remain unchanged in game
    /// state and strict source-resolution rendering.
    pub fn advance_background_offset_tables(
        &mut self,
        vertical: Option<[i16; BG2_VERTICAL_OFFSET_COLUMNS]>,
        horizontal: Option<[i16; BG2_HORIZONTAL_OFFSET_ROWS]>,
    ) {
        self.bg2d.advance_offset_tables(vertical, horizontal);
    }

    pub fn snap_background_offset_tables(&mut self) {
        self.bg2d.snap_offset_tables();
    }

    /// Mirror of `Renderer_BeginFrame`: acquire the frame + reset draw lists.
    /// The actual color/depth clear happens in `Gpu::end_frame`'s render pass.
    pub fn begin_frame(&mut self) {
        self.gpu.begin_frame();
    }

    /// Mirror of `Renderer_SubmitDrawList`: pass order is BG layer -> 3D
    /// (obj_id-keyed interpolation + shadow pass) -> particles -> HUD ->
    /// state UI -> fade.
    pub fn submit(
        &mut self,
        prev: &[DrawListEntry],
        curr: &[DrawListEntry],
        alpha: f32,
        inputs: &FrameInputs,
    ) {
        // Screen artwork shares one square-source-pixel canvas. Only active
        // HD flight expands its world view; window aspect must never stretch
        // portraits, fonts, sprites, menus, or the HUD.
        // SF2 already centers its UI horizontally in wide targets. Preserve
        // that authored placement, but fit narrow targets before composing:
        // its source-height scale alone otherwise crops the left/right edges.
        let canvas = if inputs.sf2.is_some()
            && i64::from(self.width) * i64::from(SOURCE_FRAME_HEIGHT)
                >= i64::from(self.height) * i64::from(SOURCE_FRAME_WIDTH)
        {
            RenderViewport {
                x: 0,
                y: 0,
                width: self.width.max(1) as u32,
                height: self.height.max(1) as u32,
            }
        } else {
            source_frame_viewport(self.width, self.height)
        };
        let canvas_width = canvas.width as i32;
        let canvas_height = canvas.height as i32;
        let expanded_world = inputs.sf2.is_none()
            && !inputs.source_resolution
            && matches!(inputs.game_state, GameState::Playing | GameState::Tally);
        let (scene_width, scene_height) = if expanded_world {
            (self.width.max(1), self.height.max(1))
        } else {
            (canvas_width, canvas_height)
        };
        self.gpu
            .set_draw_viewport(if expanded_world { None } else { Some(canvas) });
        self.transform.set_projection(scene_width, scene_height);
        self.font.set_screen_size(scene_width, scene_height);
        let clear_blue = if inputs.sf2.is_some() {
            SF2_SIDEBAR_BLUE_COMPONENT
        } else {
            SOURCE_CLEAR_COMPONENT
        };
        self.gpu.set_clear_color(
            SOURCE_CLEAR_COMPONENT,
            SOURCE_CLEAR_COMPONENT,
            clear_blue,
            1.0,
        );
        self.gpu.set_display_presentation(
            inputs.display_brightness,
            inputs.display_forced_blank,
            inputs
                .display_black_subtraction
                .min(FIXED_COLOR_COMPONENT_MAX),
        );
        // Rebuild the interpolated view matrix first: the BG layer derives
        // the painted-horizon scroll from the render-frame camera, so it
        // must see the same camera the 3D pass uses (DrawList render's own
        // set_view_lerp with the same alpha is then a no-op).
        self.transform.set_view_lerp(alpha);

        if let Some(replay) = inputs.ending_replay {
            self.ui.render_ending_replay_background(
                &mut self.gpu,
                replay.backdrop,
                scene_width,
                scene_height,
            );
        } else {
            self.bg2d.render(
                &mut self.gpu,
                &self.transform,
                inputs,
                alpha,
                scene_width,
                scene_height,
            );
        }
        if let Some(sf2) = inputs.sf2.filter(|sf2| sf2.mode == Sf2Mode::Mission) {
            self.ui
                .render_sf2_mission_background(&mut self.gpu, sf2.mission_backdrop);
        }
        if inputs.screen_fill_circle.scope == ScreenFillCircleScope::Background {
            self.render_screen_fill_circle(
                inputs.screen_fill_circle,
                prev,
                curr,
                alpha,
                scene_width,
                scene_height,
            );
        }
        self.shapes.set_scene_style(inputs.scene_style);
        // Polygon colors use the independent BGS-selected game palette.
        // FADETOSEA/FADETOGROUND changes background palette row 4 in Bg2d;
        // routing that state through this palette tinted every 3D object.
        let shape_palette = if let Some(sf2) = inputs.sf2.filter(|sf2| sf2.mode == Sf2Mode::Mission)
        {
            crate::shapes::sf2_polygon_shape_palette(sf2.polygon_palette)
        } else {
            crate::shapes::decode_shape_palette(crate::shapes::game_palette_bgr(
                inputs.scene_style.game_palette,
            ))
        };
        let source_gameplay_meter_palette = (inputs.source_resolution
            && inputs.game_state == GameState::Playing
            && inputs.meters != 0)
            .then(|| {
                self.bg2d
                    .gameplay_meter_palette_for_bg(inputs.currentbg)
                    .map(crate::shapes::decode_shape_palette)
            })
            .flatten();
        let presented_point_pixels = if alpha < 1.0 {
            inputs.previous_point_pixels.unwrap_or(inputs.point_pixels)
        } else {
            inputs.point_pixels
        };
        if !inputs.source_resolution {
            let points = crate::point_field::interpolate_points(
                inputs.previous_point_pixels,
                inputs.point_pixels,
                alpha,
            );
            self.ui.render_point_field(
                &mut self.gpu,
                &points,
                &shape_palette,
                scene_width,
                scene_height,
            );
        }
        let sf2_mission = inputs.sf2.is_some_and(|sf2| sf2.mode == Sf2Mode::Mission);
        let sf1_briefing = inputs.game_state == GameState::Briefing;
        if sf2_mission {
            let viewport = source_frame_viewport(self.width, self.height);
            self.gpu.set_draw_viewport(Some(viewport));
            self.transform
                .set_projection(viewport.width as i32, viewport.height as i32);
        } else if sf1_briefing {
            self.transform.set_projection_source_center(
                scene_width,
                scene_height,
                SOURCE_FRAME_WIDTH as f32,
                SOURCE_FRAME_HEIGHT as f32,
                crate::sf1_briefing::VANISH_X,
                crate::sf1_briefing::VANISH_Y,
            );
        }
        self.draw_list.render(
            &mut self.gpu,
            &self.shapes,
            &mut self.transform,
            prev,
            curr,
            alpha,
            // Per-level BGS.ASM shadowheight (0 everywhere except the
            // Nucleus interiors), keyed off the current setbg id.
            f32::from(inputs.scene_style.shadow_height),
            &shape_palette,
            &mut self.font,
            inputs
                .source_resolution
                .then_some(if inputs.game_state == GameState::Playing {
                    // Projection and clipping remain in the source-local
                    // coordinate system; this offset models only final layer
                    // placement and is zero for the retail gameplay bitmap.
                    SOURCE_POLYGON_GAMEPLAY_PRESENTATION_OFFSET
                } else {
                    SOURCE_POLYGON_DEFAULT_PRESENTATION_OFFSET
                }),
            self.hud.source_bitmap_clear(inputs),
            inputs.source_scene_camera,
            presented_point_pixels,
            source_gameplay_meter_palette.as_ref(),
            self.shadow_style,
        );
        self.particles.render(&mut self.gpu, &self.transform);
        if inputs.game_state == GameState::Title {
            self.bg2d
                .render_title_foreground(&mut self.gpu, scene_width, scene_height);
        }
        if inputs.screen_fill_circle.scope == ScreenFillCircleScope::Scene {
            self.render_screen_fill_circle(
                inputs.screen_fill_circle,
                prev,
                curr,
                alpha,
                scene_width,
                scene_height,
            );
        }
        if sf2_mission || sf1_briefing {
            self.transform.set_projection(scene_width, scene_height);
        }
        self.gpu.set_draw_viewport(Some(canvas));
        self.hud.render(
            &mut self.gpu,
            &mut self.sprites,
            &mut self.font,
            inputs,
            &shape_palette,
            canvas_width,
            canvas_height,
            alpha,
        );
        if inputs.source_resolution {
            let aperture = match inputs.game_state {
                GameState::Title => Some(SF1_TITLE_SOURCE_APERTURE),
                GameState::Playing => Some(SF1_FLIGHT_SOURCE_APERTURE),
                _ => None,
            };
            if let Some(aperture) = aperture {
                // Apply the completed source scanout aperture after the scene
                // and bitmap-resident HUD have been composed.
                self.render_source_bitmap_mask(aperture, canvas_width, canvas_height);
            }
        }
        self.ui.render(
            &mut self.gpu,
            &mut self.font,
            &self.bg2d,
            inputs,
            canvas_width,
            canvas_height,
        );
        if expanded_world && !inputs.screen_wipe.active {
            // A uniform flash has no artwork aspect and must cover the
            // expanded world too, not leave unfaded side strips.
            self.gpu.set_draw_viewport(None);
            self.ui
                .render_fade(&mut self.gpu, inputs, scene_width, scene_height);
        } else {
            if expanded_world {
                self.render_canvas_bars(canvas);
            }
            self.ui
                .render_fade(&mut self.gpu, inputs, canvas_width, canvas_height);
        }
        self.gpu.set_draw_viewport(None);
        self.transform
            .set_projection(self.width.max(1), self.height.max(1));
    }

    /// A source-aspect wipe masks the expanded world outside its aperture.
    fn render_canvas_bars(&mut self, canvas: RenderViewport) {
        let width = self.width as f32;
        let height = self.height as f32;
        let left = canvas.x as f32;
        let right = (canvas.x + canvas.width) as f32;
        let top = canvas.y as f32;
        let bottom = (canvas.y + canvas.height) as f32;
        let mut identity = [0.0; 16];
        crate::transform::identity(&mut identity);
        self.gpu.set_draw_viewport(None);
        for [x0, y0, x1, y1] in [
            [0.0, 0.0, left, height],
            [right, 0.0, width, height],
            [left, 0.0, right, top],
            [left, bottom, right, height],
        ] {
            if x0 == x1 || y0 == y1 {
                continue;
            }
            let vertices = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|[x, y]| Vertex2 {
                pos: [x * 2.0 / width - 1.0, 1.0 - y * 2.0 / height],
                uv: [0.0; 2],
            });
            self.gpu.push_overlay_fan(
                &vertices,
                &identity,
                &identity,
                [0.0, 0.0, 0.0, 1.0],
                0,
                None,
                WHITE_TEX,
            );
        }
        self.gpu.set_draw_viewport(Some(canvas));
    }

    /// Mask a completed 224-pixel-wide source bitmap after scene drawing and
    /// before HUD/OAM presentation.
    fn render_source_bitmap_mask(
        &mut self,
        aperture: SourceBitmapAperture,
        width: i32,
        height: i32,
    ) {
        const SOURCE_FRAME_WIDTH: f32 = 256.0;
        const SOURCE_FRAME_HEIGHT: f32 = 224.0;
        const IDENTITY: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];

        let output_width = width as f32;
        let output_height = height as f32;
        let scale = output_height / SOURCE_FRAME_HEIGHT;
        let source_left = (output_width - SOURCE_FRAME_WIDTH * scale) * 0.5;
        let left = source_left + SOURCE_BITMAP_LEFT * scale;
        let top = aperture.top * scale;
        let right = left + SOURCE_BITMAP_WIDTH * scale;
        let bottom = top + aperture.height * scale;
        let projection = [
            2.0 / output_width,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / output_height,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            -1.0,
            -1.0,
            0.0,
            1.0,
        ];
        for [x, y, width, height] in [
            [0.0, 0.0, left, output_height],
            [right, 0.0, output_width - right, output_height],
            [left, 0.0, right - left, top],
            [left, bottom, right - left, output_height - bottom],
        ] {
            if width <= 0.0 || height <= 0.0 {
                continue;
            }
            let render_y = output_height - y - height;
            let vertices = [
                Vertex2 {
                    pos: [x, render_y],
                    uv: [0.0, 0.0],
                },
                Vertex2 {
                    pos: [x + width, render_y],
                    uv: [0.0, 0.0],
                },
                Vertex2 {
                    pos: [x + width, render_y + height],
                    uv: [0.0, 0.0],
                },
                Vertex2 {
                    pos: [x, render_y + height],
                    uv: [0.0, 0.0],
                },
            ];
            self.gpu.push_overlay_fan(
                &vertices,
                &projection,
                &IDENTITY,
                [0.0, 0.0, 0.0, 1.0],
                0,
                None,
                WHITE_TEX,
            );
        }
    }

    /// Present the source fixed-colour addition inside the authored circle.
    /// Coordinates are expressed in the source playfield and scaled directly
    /// to the output; no source command pointer crosses into the renderer.
    fn render_screen_fill_circle(
        &mut self,
        state: ScreenFillCircleState,
        prev: &[DrawListEntry],
        curr: &[DrawListEntry],
        alpha: f32,
        width: i32,
        height: i32,
    ) {
        if !state.is_active()
            || state.radius == 0
            || (state.red == 0 && state.green == 0 && state.blue == 0)
        {
            return;
        }

        const CIRCLE_SEGMENTS: usize = 64;
        const SOURCE_HALF_HEIGHT: f32 = 112.0;
        const WORLD_TO_RENDER_FIXED_SHIFT: u32 = 16;
        const IDENTITY: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];

        let (center_x, center_y) = match state.center {
            ScreenFillCircleCenter::Screen => (0.0, 0.0),
            ScreenFillCircleCenter::Object(object_id) => {
                project_draw_object_origin(&self.transform, prev, curr, object_id, alpha)
                    .unwrap_or((0.0, 0.0))
            }
            ScreenFillCircleCenter::World { x, y, z } => project_world_origin(
                &self.transform,
                i32::from(x) << WORLD_TO_RENDER_FIXED_SHIFT,
                i32::from(y) << WORLD_TO_RENDER_FIXED_SHIFT,
                i32::from(z) << WORLD_TO_RENDER_FIXED_SHIFT,
            )
            .unwrap_or((0.0, 0.0)),
        };
        let radius_x = f32::from(state.radius) / SOURCE_HALF_HEIGHT * height.max(1) as f32
            / width.max(1) as f32;
        let radius_y = f32::from(state.radius) / SOURCE_HALF_HEIGHT;
        let mut fan = Vec::with_capacity(CIRCLE_SEGMENTS + 2);
        fan.push(Vertex2 {
            pos: [center_x, center_y],
            uv: [0.0, 0.0],
        });
        for point in 0..=CIRCLE_SEGMENTS {
            let angle = std::f32::consts::TAU * point as f32 / CIRCLE_SEGMENTS as f32;
            fan.push(Vertex2 {
                pos: [
                    center_x + angle.cos() * radius_x,
                    center_y + angle.sin() * radius_y,
                ],
                uv: [0.0, 0.0],
            });
        }
        self.gpu.push_overlay_additive_fan(
            &fan,
            &IDENTITY,
            &IDENTITY,
            [
                f32::from(state.red) / f32::from(MAX_COLOR_LEVEL),
                f32::from(state.green) / f32::from(MAX_COLOR_LEVEL),
                f32::from(state.blue) / f32::from(MAX_COLOR_LEVEL),
                1.0,
            ],
        );
    }

    /// Present a native SNES framebuffer as a 4:3, nearest-neighbour image.
    /// This is used for SF2's exact retail-host screens while their tile/UI
    /// state is promoted into HD-native passes.
    pub fn submit_native_frame(&mut self, width: u32, height: u32, rgba: &[u8]) {
        if width == 0 || height == 0 || rgba.len() < width as usize * height as usize * 4 {
            return;
        }
        if self.native_frame_texture.is_none() || self.native_frame_size != (width, height) {
            self.native_frame_texture = Some(self.gpu.create_texture_rgba(width, height, rgba));
            self.native_frame_size = (width, height);
        } else if let Some(texture) = self.native_frame_texture {
            self.gpu.update_texture(texture, rgba);
        }
        let Some(texture) = self.native_frame_texture else {
            return;
        };

        let screen_w = self.width.max(1) as f32;
        let screen_h = self.height.max(1) as f32;
        let target_aspect = 4.0 / 3.0;
        let (draw_w, draw_h) = if screen_w / screen_h > target_aspect {
            (screen_h * target_aspect, screen_h)
        } else {
            (screen_w, screen_w / target_aspect)
        };
        let left = (screen_w - draw_w) * 0.5;
        let top = (screen_h - draw_h) * 0.5;
        let proj = [
            2.0 / screen_w,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / screen_h,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            -1.0,
            -1.0,
            0.0,
            1.0,
        ];
        const IDENTITY: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        let vertices = [
            Vertex2 {
                pos: [left, top],
                uv: [0.0, 0.0],
            },
            Vertex2 {
                pos: [left + draw_w, top],
                uv: [1.0, 0.0],
            },
            Vertex2 {
                pos: [left + draw_w, top + draw_h],
                uv: [1.0, 1.0],
            },
            Vertex2 {
                pos: [left, top + draw_h],
                uv: [0.0, 1.0],
            },
        ];
        self.gpu.push_overlay_fan(
            &vertices,
            &proj,
            &IDENTITY,
            [1.0, 1.0, 1.0, 1.0],
            1,
            None,
            texture,
        );
    }

    /// Drain HUD-queued one-shot SE (arrow beep $8A from do_arrows wrap).
    pub fn take_pending_hud_sounds(&mut self) -> Vec<u8> {
        self.hud.take_pending_sounds()
    }

    /// Mirror of `Renderer_EndFrame`: upload the frame's geometry and present.
    pub fn end_frame(&mut self) {
        self.gpu.end_frame();
    }

    /// Read back the framebuffer as tightly-packed RGB rows, top-down.
    /// Only valid on a headless (offscreen) Gpu; returns black otherwise.
    pub fn read_pixels_rgb(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        match self.gpu.read_pixels() {
            Some((rw, rh, rgba)) => {
                let (rw, rh) = (rw as usize, rh as usize);
                let mut rgb = vec![0u8; rw * rh * 3];
                for i in 0..rw * rh {
                    rgb[i * 3] = rgba[i * 4];
                    rgb[i * 3 + 1] = rgba[i * 4 + 1];
                    rgb[i * 3 + 2] = rgba[i * 4 + 2];
                }
                rgb
            }
            None => vec![0u8; w * h * 3],
        }
    }

    /// Indexed polygon bitmap from the most recent strict source render.
    pub fn source_bitmap_indices(&self) -> &[u8] {
        self.draw_list.source_bitmap_indices()
    }

    pub fn source_bitmap_rgba(&self) -> &[u8] {
        self.draw_list.source_bitmap_rgba()
    }

    pub fn source_bitmap_owners(&self) -> &[u16] {
        self.draw_list.source_bitmap_owners()
    }

    pub fn source_bitmap_faces(&self) -> &[u16] {
        self.draw_list.source_bitmap_faces()
    }

    pub fn source_frame_workload(&self) -> crate::source_raster::SourceFrameWorkload {
        self.draw_list.source_frame_workload()
    }

    /// Mirror of `Renderer_Shutdown` (wgpu frees GPU resources on drop).
    pub fn shutdown(&mut self) {
        println!("Renderer shut down");
    }
}

/// Convenience: shader dir + asset root from a repo checkout root.
pub fn config_from_repo_root(root: &Path) -> RendererConfig {
    RendererConfig {
        shader_dir: Some(root.join("rust/sf-render/shaders")),
        asset_root: root.to_path_buf(),
        shadow_style: ShadowStyle::RetailDithered,
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;

    #[test]
    fn source_frame_is_centered_in_a_widescreen_target() {
        assert_eq!(
            source_frame_viewport(1_280, 720),
            RenderViewport {
                x: 228,
                y: 0,
                width: 823,
                height: 720,
            }
        );
    }

    #[test]
    fn source_frame_is_centered_in_a_tall_target() {
        assert_eq!(
            source_frame_viewport(640, 720),
            RenderViewport {
                x: 0,
                y: 80,
                width: 640,
                height: 560,
            }
        );
    }
}
