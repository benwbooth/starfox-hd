//! Top-level renderer: owns every pass and mirrors the C pass order.
//!
//! Port (C oracle): `src/renderer/renderer.c`. `Renderer::submit` runs the
//! exact `Renderer_SubmitDrawList` sequence: view lerp -> Bg2d -> DrawList
//! 3D (incl. shadow pass) -> particles -> HUD -> state UI -> fade.
//!
//! Game-state inputs arrive via the plain [`FrameInputs`] struct so this
//! crate does not depend on sf-game; sf-app bridges its globals in.

use glow::HasContext;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::bg2d::Bg2d;
use crate::draw_list::{DrawListEntry, DrawListRenderer};
use crate::font::Font;
use crate::gl_backend::GlBackend;
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
    /// Directory containing `flat.vert.glsl` / `flat.frag.glsl` (the C
    /// tree's `src/renderer/shaders/`). Falls back to embedded sources.
    pub shader_dir: Option<PathBuf>,
    /// Root containing the `data/` asset tree (repo root in the C build).
    pub asset_root: PathBuf,
}

pub struct Renderer {
    gl: Arc<glow::Context>,
    width: i32,
    height: i32,

    pub backend: GlBackend,
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
        gl: Arc<glow::Context>,
        width: i32,
        height: i32,
        config: &RendererConfig,
    ) -> Result<Self, String> {
        let backend = GlBackend::new(&gl, config.shader_dir.as_deref())?;
        let mut transform = Transform::new();
        let draw_list = DrawListRenderer::new();
        let hud = Hud::new(&gl);
        let particles = Particles::new(&gl);
        let font = Font::new(&gl, &config.asset_root);
        let sprites = Sprites::new(&gl, &config.asset_root);
        let bg2d = Bg2d::new(&gl, &config.asset_root);
        let ui = Ui::new(&gl, &config.asset_root);

        // Set initial projection (also done in resize)
        transform.set_projection(width, height);

        // Register built-in shapes (Arwing, etc.)
        let mut shapes = ShapeStore::new();
        shapes.register_builtins(&gl);

        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            // Disable backface culling — Star Fox shape winding is not
            // guaranteed CCW.
            gl.disable(glow::CULL_FACE);

            gl.viewport(0, 0, width, height);
            gl.clear_color(0.0, 0.0, 0.05, 1.0); // Deep space blue-black
        }

        println!("Renderer initialized ({width}x{height})");

        Ok(Renderer {
            gl,
            width,
            height,
            backend,
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

    pub fn gl(&self) -> &glow::Context {
        &self.gl
    }

    /// Mirror of `Renderer_Resize`.
    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
        self.transform.set_projection(width, height);
        self.font.set_screen_size(width, height);
    }

    /// Mirror of `Renderer_BeginFrame`.
    pub fn begin_frame(&self) {
        unsafe {
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }
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
        let gl = Arc::clone(&self.gl);

        // Rebuild the interpolated view matrix first: the BG layer derives
        // the painted-horizon scroll from the render-frame camera, so it
        // must see the same camera the 3D pass uses (DrawList render's own
        // set_view_lerp with the same alpha is then a no-op).
        self.transform.set_view_lerp(alpha);

        self.bg2d.render(
            &gl,
            &self.backend,
            &self.transform,
            inputs,
            self.width,
            self.height,
        );
        self.draw_list.render(
            &gl,
            &mut self.backend,
            &self.shapes,
            &mut self.transform,
            prev,
            curr,
            alpha,
        );
        self.particles.render(&gl, &self.backend, &self.transform);
        self.hud.render(
            &gl,
            &self.backend,
            &mut self.sprites,
            &mut self.font,
            inputs,
            self.width,
            self.height,
        );
        self.ui.render(
            &gl,
            &self.backend,
            &mut self.font,
            &self.bg2d,
            inputs,
            self.width,
            self.height,
        );
        self.ui
            .render_fade(&gl, &self.backend, inputs, self.width, self.height);
    }

    /// Mirror of `Renderer_EndFrame` (post-processing hook; the C's
    /// SF_DUMP_PPM debug dump is not ported — tests read pixels directly).
    pub fn end_frame(&self) {}

    /// Read back the framebuffer as tightly-packed RGB rows, top-down
    /// (equivalent to the C MaybeDumpFrame PPM layout). Test/debug helper.
    pub fn read_pixels_rgb(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut pixels = vec![0u8; w * h * 3];
        unsafe {
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                self.width,
                self.height,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
        }
        // Flip vertically (GL rows are bottom-up)
        let mut flipped = vec![0u8; w * h * 3];
        for y in 0..h {
            flipped[y * w * 3..(y + 1) * w * 3]
                .copy_from_slice(&pixels[(h - 1 - y) * w * 3..(h - y) * w * 3]);
        }
        flipped
    }

    /// Mirror of `Renderer_Shutdown`.
    pub fn shutdown(&mut self) {
        let gl = Arc::clone(&self.gl);
        self.ui.destroy(&gl);
        self.bg2d.destroy(&gl);
        self.sprites.destroy(&gl);
        self.font.destroy(&gl);
        self.particles.destroy(&gl);
        self.hud.destroy(&gl);
        self.shapes.destroy(&gl);
        self.backend.destroy(&gl);
        println!("Renderer shut down");
    }
}

/// Convenience: shader dir + asset root from a repo checkout root.
pub fn config_from_repo_root(root: &Path) -> RendererConfig {
    RendererConfig {
        shader_dir: Some(root.join("src/renderer/shaders")),
        asset_root: root.to_path_buf(),
    }
}
