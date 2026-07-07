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
use crate::gpu::Gpu;
use crate::hud::Hud;
use crate::particles::Particles;
use crate::shapes_gl::ShapeStore;
use crate::sprites::Sprites;
use crate::transform::Transform;
use crate::ui::Ui;

/// Mirror of the C `GameState` enum (src/game/boot.h).
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

    // Shape-palette fade (map-VM FADETOSEA/FADETOGROUND, WORLD.ASM:371-394;
    // consumer fadepalto_l MAIN.ASM:2762). PAL_TARGET_* ids from
    // crate::shapes; sf-game bridges its PALFADE_* values 1:1.
    /// Palette the fade started from (PAL_TARGET_*).
    pub pal_from: u8,
    /// Palette the fade walks toward (PAL_TARGET_*).
    pub pal_target: u8,
    /// Fade progress 0..1 (the ROM's 15-frame palnum walk as a fraction).
    pub pal_fade: f32,

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
}

impl<'a> Default for FrameInputs<'a> {
    fn default() -> Self {
        FrameInputs {
            game_state: GameState::Boot,
            currentbg: 0,
            newmap: 0,
            bgflags: 0,
            bg2_xscroll: 0,
            nomax_bg2_yscroll: false,
            pal_from: 0,
            pal_target: 0,
            pal_fade: 0.0,
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
            msg_count1: 0,
            msg_count2: 0,
            whichfriend: 0,
            friends_meter: 0,
            message_text: None,
            score: 0,
            credits: 0,
            tally_active: false,
            tally_stage_perc: 0,
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
        })
    }

    /// Headless renderer for offscreen tests: renders into a texture that
    /// `read_pixels_rgb` reads back. No window or surface required.
    pub fn new_headless(
        width: i32,
        height: i32,
        config: &RendererConfig,
    ) -> Result<Self, String> {
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

        self.bg2d
            .render(&mut self.gpu, &self.transform, inputs, self.width, self.height);
        // Effective shape palette for this frame: the FADETOSEA/FADETOGROUND
        // crossfade (fadepalto_l, MAIN.ASM:2762) mixed from the bridged
        // map-VM fade state; plain night when no fade ever ran.
        let shape_palette = crate::shapes::mixed_shape_palette(
            inputs.pal_from,
            inputs.pal_target,
            inputs.pal_fade,
        );
        self.draw_list.render(
            &mut self.gpu,
            &self.shapes,
            &mut self.transform,
            prev,
            curr,
            alpha,
            // Per-level BGS.ASM shadowheight (0 everywhere except the
            // Nucleus interiors), keyed off the current setbg id.
            crate::bg2d::shadow_height_for_bg(inputs.currentbg),
            &shape_palette,
        );
        self.particles.render(&mut self.gpu, &self.transform);
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
