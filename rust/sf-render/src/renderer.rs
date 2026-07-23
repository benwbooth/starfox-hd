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
use crate::draw_list::{DrawListEntry, DrawListRenderer};
use crate::font::Font;
use crate::gpu::{Gpu, RenderViewport, TextureId, Vertex2};
use crate::hud::Hud;
use crate::particles::Particles;
use crate::shapes_gl::ShapeStore;
use crate::sprites::Sprites;
use crate::transform::Transform;
use crate::ui::Ui;
use sf_core::scene::{PaletteFadeTarget, SceneStyle};

/// Semantic presentation state shared by the native game and renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameState {
    #[default]
    Boot,
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
    EladardSurface,
    EladardInterior,
    TitaniaBase,
    CarrierInterior,
    AstropolisVoid,
}

/// Plain typed SF2 state consumed by native UI passes. It deliberately mirrors
/// the game-domain fields rather than a memory block or processor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sf2FrameInputs {
    pub mode: Sf2Mode,
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
    pub primary_shield: u8,
    pub wingmate_shield: u8,
    pub item_count: u8,
    pub target_count: u8,
    pub mission_elapsed_time_tenths: u16,
    pub radar_contacts: [Option<Sf2RadarContact>; SF2_RADAR_CONTACT_CAPACITY],
    pub mode_frame: u32,
    pub elapsed_campaign_frames: u64,
    pub corneria_damage_percent: u8,
    pub score: u32,
    pub campaign_sorties_completed: u16,
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
    /// g_nomaxbg2Yscroll.
    pub nomax_bg2_yscroll: bool,
    /// Typed polygon lighting and shadow-plane state selected by the active
    /// background script.
    pub scene_style: SceneStyle,

    // Background palette-row fade (map-VM FADETOSEA/FADETOGROUND,
    // WORLD.ASM:371-394; consumer fadepalto_l MAIN.ASM:2762).
    pub pal_target: Option<PaletteFadeTarget>,
    /// ROM `palnum` remaining (starts 30, two bytes per tick to zero).
    pub palfade_num: u16,

    // Window / fade state (WINDOWS.ASM)
    /// g_windowmode bitmask of allocated slots.
    pub windowmode: u8,
    pub windows: [WindowState; WINDOWARRAY_SIZE],

    // HUD state
    /// g_meters.
    pub meters: u16,
    /// g_stayblack (-1 = normal gameplay control path).
    pub stayblack: i8,
    /// g_gameflags (GF_*).
    pub gameflags: u8,
    /// g_gameframe.
    pub gameframe: u16,
    /// g_boostcnt.
    pub boostcnt: u8,
    /// g_arrows (SPRAR_*).
    pub arrows: u8,
    /// g_splayerflymode (SPFM_*).
    pub splayerflymode: u8,
    /// g_stage.
    pub stage: u16,
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
            game_state: GameState::Boot,
            currentbg: 0,
            newmap: 0,
            bgflags: 0,
            bg2_xscroll: 0,
            nomax_bg2_yscroll: false,
            scene_style: SceneStyle::default(),
            pal_target: None,
            palfade_num: 0,
            windowmode: 0,
            windows: [WindowState::default(); WINDOWARRAY_SIZE],
            meters: 0,
            stayblack: -1,
            gameflags: 0,
            gameframe: 0,
            boostcnt: 0,
            arrows: 0,
            splayerflymode: 0,
            stage: 0,
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

        // Deep space blue-black (was gl.clear_color(0,0,0.05,1)). Depth test
        // and no culling are baked into the wgpu pipelines.
        gpu.set_clear_color(0.0, 0.0, 0.05, 1.0);

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
        self.width = width;
        self.height = height;
        self.gpu.resize(width.max(0) as u32, height.max(0) as u32);
        self.transform.set_projection(width, height);
        self.font.set_screen_size(width, height);
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
        // Rebuild the interpolated view matrix first: the BG layer derives
        // the painted-horizon scroll from the render-frame camera, so it
        // must see the same camera the 3D pass uses (DrawList render's own
        // set_view_lerp with the same alpha is then a no-op).
        self.transform.set_view_lerp(alpha);

        if let Some(replay) = inputs.ending_replay {
            self.ui.render_ending_replay_background(
                &mut self.gpu,
                replay.backdrop,
                self.width,
                self.height,
            );
        } else {
            self.bg2d.render(
                &mut self.gpu,
                &self.transform,
                inputs,
                self.width,
                self.height,
            );
        }
        if let Some(sf2) = inputs.sf2.filter(|sf2| sf2.mode == Sf2Mode::Mission) {
            self.ui
                .render_sf2_mission_background(&mut self.gpu, sf2.mission_backdrop);
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
        let sf2_mission = inputs.sf2.is_some_and(|sf2| sf2.mode == Sf2Mode::Mission);
        if sf2_mission {
            let viewport = source_frame_viewport(self.width, self.height);
            self.gpu.set_draw_viewport(Some(viewport));
            self.transform
                .set_projection(viewport.width as i32, viewport.height as i32);
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
        );
        self.particles.render(&mut self.gpu, &self.transform);
        if sf2_mission {
            self.gpu.set_draw_viewport(None);
            self.transform.set_projection(self.width, self.height);
        }
        self.hud.render(
            &mut self.gpu,
            &mut self.sprites,
            &mut self.font,
            inputs,
            self.width,
            self.height,
        );
        self.ui.render(
            &mut self.gpu,
            &mut self.font,
            &self.bg2d,
            inputs,
            self.width,
            self.height,
        );
        self.ui
            .render_fade(&mut self.gpu, inputs, self.width, self.height);
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
