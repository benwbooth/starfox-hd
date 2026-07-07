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

use sf_core::{pad, DrawListEntry};

use crate::camera::GameCamera;
use crate::game::{Game, Hooks};
use crate::obj::Objects;
use crate::planets::{Planets, DEFAULT_LIVES};
use crate::score;
use crate::strings::Strings;
use crate::vars::{
    GameVars, GF_PLAYERDEAD, GF_PLAYERDYING, PALFADE_NUM_START, PFM_SHADOWS, PSF2_PLAYERHP0,
    PSF3_ENGINESND, SPACE_MODE, SPFM_NORM,
};
use crate::windows::Windows;
use crate::world::World;
use crate::{bgs, draw};

/// C `LEVEL_CLEAR_SETTLE_TICKS` (src/game/boot.c:39) — 3 s after mapend
/// before the stage advance.
pub const LEVEL_CLEAR_SETTLE_TICKS: i32 = 60;
/// Minimum frames the end-of-level tally screen is shown before it can be
/// dismissed (ROM `end_level_seq` animates the percent count-up over many
/// frames, MAIN.ASM:1101-1210; no single ROM constant — a display hold).
pub const TALLY_HOLD_TICKS: i32 = 120;
/// C `DEATH_RESPAWN_TICKS` (src/game/boot.c:40) — 2.5 s after
/// GF_PLAYERDEAD before reload.
pub const DEATH_RESPAWN_TICKS: i32 = 50;

/// C `NMI_PLAYER_MAX_HP` (src/game/nmi.c:47) — player body hit points.
pub const NMI_PLAYER_MAX_HP: i32 = 40;

/// C `GameState` (src/game/boot.h:17-25), same order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Boot,
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

impl GameState {
    /// Numeric code in boot.h enum order (BOOT=0 .. ENDING=6).
    ///
    /// The tally screen has no distinct render state yet; it reports
    /// `PlanetSelect` (3) so sf-app renders the map screen underneath and
    /// ui.rs overlays the tally figures via `FrameInputs::tally_active`.
    pub fn code(self) -> u8 {
        match self {
            GameState::Boot => 0,
            GameState::Title => 1,
            GameState::Briefing => 2,
            GameState::PlanetSelect => 3,
            GameState::Playing => 4,
            GameState::Continue => 5,
            GameState::Ending => 6,
            GameState::Tally => 3,
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
    PlayImmediate(u8),
    StopMusic,
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
    /// Shape-palette fade source (vars PALFADE_*, FADETOSEA/FADETOGROUND).
    pub pal_from: u8,
    /// Shape-palette fade target (vars PALFADE_*).
    pub pal_target: u8,
    /// Fade progress 0..1 (ROM palnum 15-frame walk as a fraction).
    pub pal_fade: f32,
    pub windowmode: u8,
    pub windows: [WindowSlot; 8],
    pub meters: u16,
    pub stayblack: i8,
    pub gameflags: u8,
    pub gameframe: u16,
    pub boostcnt: u8,
    pub arrows: u8,
    pub splayerflymode: u8,
    pub stage: u16,
    pub shield_cur: i32,
    pub shield_max: i32,
    pub boss_hp_cur: i32,
    pub boss_hp_max: i32,
    pub lives: i32,
    pub bombs: i32,
    pub msg_count1: u8,
    pub msg_count2: u8,
    pub whichfriend: u8,
    pub friends_meter: u8,
    pub message_text: Option<String>,
    pub whichroute: u8,
    pub currentplanet: i16,
    pub nebula_on: u16,
    pub route_path_ids: Vec<u16>,
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
}

/// State shared between the shell and the map-VM hooks (the C globals that
/// windows.c/strings.c/sound.c exposed to world.c/levels.c).
struct ShellState {
    windows: Windows,
    strings: Strings,
    sound: Vec<SoundCmd>,
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
            strings: Strings::new(),
            sound: Vec::new(),
            path_warned: vec![false; 512],
            shape_extents: HashMap::new(),
        }
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
        self.state.borrow_mut().sound.push(SoundCmd::PlayMusic(track_id));
    }

    fn play_se(&mut self, sound_id: u8) {
        // C Sound_PlaySE (level inline callbacks).
        self.state.borrow_mut().sound.push(SoundCmd::PlaySe(sound_id));
    }

    fn trig_se(&mut self, sound_id: u8) {
        // C Strat_TrigSE == Sound_PlaySE (src/strat/strat_common.c:323).
        self.state.borrow_mut().sound.push(SoundCmd::PlaySe(sound_id));
    }

    fn send_message(&mut self, msg_id: u8) {
        // C Strings_SendMessage (map sendmsg, CLfriendmsg builtins).
        self.state.borrow_mut().strings.send_message(msg_id);
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
        self.state.borrow_mut().windows.init_black();
    }

    fn init_fade_white2norm(&mut self) {
        self.state.borrow_mut().windows.init_fade_white2norm();
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

    fn player_exit_base(&mut self) {
        // TODO(wave3): hangar launch chain is strat-lane
        // (C Strat_PlayerExitBase, src/strat/strat_player.c).
    }
}

/// The boot/frame shell around [`crate::game::Game`] — one instance per
/// running game, ticked at 20 Hz by sf-app.
pub struct Shell {
    pub game: Game,
    state: Rc<RefCell<ShellState>>,
    game_state: GameState,
    /// C `s_title_loaded` (boot.c:50).
    title_loaded: bool,
    planets: Planets,
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
    /// Frames spent on the tally screen (GameState::Tally hold timer).
    tally_ticks: i32,
    /// The just-finished stage's hit percentage, for the tally overlay
    /// (ROM `cla2`, MAIN.ASM:1071).
    tally_stage_perc: u8,
    /// C `g_rndval` (sf_rtl.c:16) — strings face animation PRNG.
    rndval: u16,
    /// Warn-once set for unported map ids (C levels.c warn-once END stub).
    warned_maps: Vec<u32>,

    // --- C globals surfaced in FrameSnapshot but owned by lanes that are
    // not ported yet; held here with GameVars_Init defaults. ---
    /// C `g_specwepcnt` (game_vars.c:429, DEFAULT_NUKE_COUNT).
    /// TODO(sf-strat): nuke pickup/fire mutates this.
    specwepcnt: u16,
    /// C `g_stayblack` (game_vars.c:369, default -1). TODO(sf-strat).
    stayblack: i8,
    /// C `g_arrows` (game_vars.c:367). TODO(sf-strat).
    arrows: u8,
    /// C `g_boostcnt` (game_vars.c:430). TODO(sf-strat).
    boostcnt: u8,
    /// C `g_doingwipe` (game_vars.c:368; boot.c:68 clears it).
    doingwipe: u8,
    /// C `g_bg2Xscroll` (game_vars.c:234). TODO(sf-strat/render).
    bg2_xscroll: i16,
    /// C `g_nomaxbg2Yscroll` (game_vars.c:195). TODO(sf-strat/render).
    nomax_bg2_yscroll: u8,

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
}

impl Shell {
    pub fn new() -> Self {
        let state = Rc::new(RefCell::new(ShellState::new()));
        let hooks = ShellHooks { state: Rc::clone(&state) };
        Shell {
            game: Game::with_hooks(Box::new(hooks)),
            state,
            game_state: GameState::Boot, // C g_game_state init (boot.c:33)
            title_loaded: false,
            planets: Planets::new(),
            camera: GameCamera::new(),
            draw_list: Vec::new(),
            cam_snapshot: CameraSnapshot::default(),
            prev_pad: 0,
            pad1_new: 0,
            levelclear_ticks: 0,
            death_ticks: 0,
            tally_ticks: 0,
            tally_stage_perc: 0,
            rndval: 0,
            warned_maps: Vec::new(),
            specwepcnt: 3,
            stayblack: -1,
            arrows: 0,
            boostcnt: 0,
            doingwipe: 0,
            bg2_xscroll: 0,
            nomax_bg2_yscroll: 0,
            register_strats: None,
            spawn_player: None,
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
        self.pad1_new = pad1 & !self.prev_pad;
        self.prev_pad = pad1;
        self.game.vars.pad1 = pad1;

        // Live friend-HP mirror for the send_message hook path: C
        // strings.c reads g_bunny_hp/g_falcon_hp/g_frog_hp at call time;
        // GameVars is canonical for those (the map VM friend-alive
        // callbacks read them there). Nothing mutates them mid-tick until
        // sf-strat lands, so a top-of-tick sync is exact.
        {
            let mut st = self.state.borrow_mut();
            st.strings.bunny_hp = self.game.vars.bunny_hp;
            st.strings.falcon_hp = self.game.vars.falcon_hp;
            st.strings.frog_hp = self.game.vars.frog_hp;
        }

        // C Game_Tick state switch (boot.c:226-276).
        match self.game_state {
            GameState::Boot => self.game_init(),
            GameState::Title => self.title_tick(),
            GameState::Briefing => {
                // Use start/A/B to advance into route select (boot.c:235).
                if self.pad1_new & (pad::START | pad::A | pad::B) != 0 {
                    self.planets_init();
                    self.game_state = GameState::PlanetSelect;
                }
            }
            GameState::PlanetSelect => {
                // No 3D scene on the map screen (boot.c:243-248).
                self.draw_list.clear();
                if self.planets.update(self.pad1_new) {
                    // C Planets_Update calls
                    // Game_BeginGameplayFromPlanetSelect directly
                    // (planets.c:345).
                    self.begin_gameplay_from_planet_select();
                }
            }
            GameState::Playing => {
                // Core gameplay tick + progression bridge (boot.c:250-255).
                self.nmi_game_tick();
                self.gameplay_progress_tick();
            }
            GameState::Continue => {
                // CONTINUE.ASM semantics (boot.c:257-268).
                if self.pad1_new & (pad::START | pad::A) != 0 {
                    self.planets.lives = DEFAULT_LIVES;
                    self.begin_gameplay_from_planet_select();
                } else if self.pad1_new & (pad::B | pad::SELECT) != 0 {
                    self.game_state = GameState::Title;
                }
            }
            GameState::Ending => {
                // Minimal ending flow (boot.c:270-275).
                if self.pad1_new & (pad::START | pad::A | pad::B) != 0 {
                    self.game_state = GameState::Title;
                }
            }
            GameState::Tally => {
                // End-of-level tally screen (ROM end_level_seq).
                self.draw_list.clear();
                self.tally_tick();
            }
        }

        // Every tick after the state switch (boot.c:278-284):
        // Bgs_Update, Windows_Update, Strings_Update.
        bgs::update(&mut self.game.vars);
        {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            st.windows
                .update(&mut self.game.vars.oncewipe, &mut self.game.vars.circleanim);
            st.strings
                .update(self.game.vars.gameflags, &mut self.rndval, &mut st.sound);
        }
    }

    pub fn state(&self) -> GameState {
        self.game_state
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

        FrameSnapshot {
            game_state_code: self.game_state.code(),
            currentbg: v.currentbg,
            newmap: self.planets.newmap,
            bgflags: v.bgflags,
            bg2_xscroll: self.bg2_xscroll as i32,
            nomax_bg2_yscroll: self.nomax_bg2_yscroll != 0,
            pal_from: v.palfade_from,
            pal_target: v.palfade_target,
            pal_fade: (PALFADE_NUM_START.saturating_sub(v.palfade_num)) as f32
                / PALFADE_NUM_START as f32,
            windowmode: st.windows.windowmode,
            windows: st.windows.slots,
            meters: v.meters,
            stayblack: self.stayblack,
            gameflags: v.gameflags,
            gameframe: v.gameframe,
            boostcnt: self.boostcnt,
            arrows: self.arrows,
            splayerflymode: v.splayerflymode,
            stage: self.planets.stage,
            shield_cur,
            shield_max,
            boss_hp_cur: v.bosshp as i32,
            boss_hp_max: v.bossmaxhp as i32,
            lives: self.planets.lives as i32,
            bombs: self.specwepcnt as i32,
            msg_count1: st.strings.msg_count1,
            msg_count2: st.strings.msg_count2,
            whichfriend: st.strings.whichfriend,
            friends_meter: st.strings.friends_meter,
            message_text: st.strings.active_text.map(String::from),
            whichroute: self.planets.whichroute,
            currentplanet: self.planets.currentplanet,
            nebula_on: self.planets.nebula_on,
            route_path_ids: self.planets.route_path_ids(self.planets.whichroute),
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
            tally_stage_perc: self.tally_stage_perc,
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
        // TODO(wave3): sf_strat::table::register_all(&mut self.game)
        //   (C Strat_RegisterAll, boot.c:129).
        // Sound_Init (boot.c:130) is app-side: sf-audio owns the SPC state.
        {
            let mut st = self.state.borrow_mut();
            st.windows.init(); // Windows_Init (boot.c:131)
            st.strings.init(); // Strings_Init (boot.c:133)
        }
        bgs::init(&mut self.game.vars); // Bgs_Init (boot.c:132)

        // GameVars_Init resets of C globals mirrored shell-side.
        self.specwepcnt = 3; // DEFAULT_NUKE_COUNT (game_vars.c:429)
        self.stayblack = -1; // game_vars.c:369
        self.arrows = 0;
        self.boostcnt = 0;
        self.doingwipe = 0;
        self.bg2_xscroll = 0;
        self.nomax_bg2_yscroll = 0;
        self.planets = Planets::new(); // route/stage defaults (game_vars.c:443-458)
        self.levelclear_ticks = 0;
        self.death_ticks = 0;
        self.rndval = 0; // sf_rtl.c:52

        self.game_state = GameState::Title; // boot.c:135
        self.title_loaded = false;

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
    }

    /// C `Planets_Init` call sites (boot.c:160/238): planets.c:306 also
    /// zeroes `g_bg_dmalist`, which lives in GameVars.
    fn planets_init(&mut self) {
        self.planets.init();
        self.game.vars.bg_dmalist = 0;
    }

    /// C `TitleScreen_Tick()` (src/game/boot.c:141).
    fn title_tick(&mut self) {
        if !self.title_loaded {
            // boot.c:143-149.
            self.game.objs = Objects::init();
            self.camera.init(&mut self.game.vars);
            self.game.world = World::init();
            self.reregister_strats();
            // TODO(wave3): sf_strat::table::register_all(&mut self.game)
            //   (C Strat_RegisterAll, boot.c:146).
            self.load_map(sf_map::catalog::map_id::TITLE);
            self.title_loaded = true;
        }

        // boot.c:152-155: clear list, Obj_RunStrategies directly (no
        // freezestrats guard, no coldet), camera, draw build.
        self.draw_list.clear();
        self.game.run_strategies();
        self.cam_snapshot = self.camera.update(&self.game.vars, &self.game.objs);
        // Cull anchors on the camera viewpos published by camera.update
        // (ROM viewposx/z, game.c:94-98), with the coldet shape-extents
        // table supplying the sh_zmax margin (alienflags_l MAIN.ASM:2030).
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

        // Check for Start to enter gameplay (boot.c:158-164).
        if self.pad1_new & pad::START != 0 {
            self.state.borrow_mut().sound.push(SoundCmd::PlaySe(0x10));
            self.planets_init();
            self.game_state = GameState::PlanetSelect;
            self.title_loaded = false;
        }
    }

    /// C `Game_BeginGameplayFromPlanetSelect()` (src/game/boot.c:52).
    fn begin_gameplay_from_planet_select(&mut self) {
        // Per-run player/game flag reset (boot.c:55-69).
        let v = &mut self.game.vars;
        // Seed the unified WRAM lives store (sv::LIVES 0x0520 — the ROM's one
        // `lives` var: death decs it in playerdead_strat, the 1-UP item incs
        // it) from the shell-persistent copy; the shell mirrors it back every
        // tick so respawn/game-over/HUD all see gameplay changes.
        v.write_ext8(0x0520, self.planets.lives);
        v.gameflags &= !(GF_PLAYERDYING | GF_PLAYERDEAD);
        v.pshipflags = 0;
        v.pshipflags2 = 0;
        v.pshipflags3 = 0;
        v.pstratflags = 0;
        v.playerflymode = PFM_SHADOWS;
        v.splayerflymode = SPFM_NORM;
        v.freezestrats = 0;
        v.bossmaxhp = 0;
        v.meters = 0;
        // g_maptrigger / g_screenflashcnt (boot.c:65-66) are unported
        // strat/render-lane globals; nothing in the Rust port reads them
        // yet. TODO(sf-strat)/TODO(sf-render).
        v.circleanim = 0;
        v.oncewipe = 0;
        self.doingwipe = 0;
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
        // TODO(wave3): sf_strat::table::register_all(&mut self.game)
        //   (C Strat_RegisterAll, boot.c:85).
        self.load_map(self.planets.newmap);

        // Spawn the player alien (C Strat_SpawnPlayer, boot.c:89-102). The
        // spawn hook is injected from the app layer alongside register_strats
        // (sf-strat depends on sf-game; a direct call would be circular).
        // LEVEL1_1/2_1/3_1 open with `pstrat playeropening`, so pass the
        // newmap so the hook can run Strat_PlayerOpening_Init for those.
        if let Some(hook) = self.spawn_player.take() {
            hook(&mut self.game, self.planets.newmap);
            self.spawn_player = Some(hook);
        }

        self.game_state = GameState::Playing; // boot.c:104
    }

    /// C `Nmi_GameTick()` (src/game/nmi.c:49) — PLAYING-state tick, exact
    /// ordering.
    fn nmi_game_tick(&mut self) {
        // Game_ClearDrawList + dostrats gate (nmi.c:58-61).
        self.draw_list.clear();
        if self.game.vars.freezestrats & 0x01 == 0 {
            self.game.run_strategies();
        }

        // Store last frame's controller state (nmi.c:65-66,
        // TRANS.ASM:158-161).
        self.game.vars.lastcont0 = (self.game.vars.pad1 >> 8) as u8;
        self.game.vars.lastcontl0 = (self.game.vars.pad1 & 0xFF) as u8;

        // getview_l (nmi.c:70).
        self.cam_snapshot = self.camera.update(&self.game.vars, &self.game.objs);

        // dosounds_l (nmi.c:73) runs app-side: sf-app feeds the
        // FrameSnapshot sound-layer inputs to sf-audio each tick.

        // calcmeters / HUD (nmi.c:77-90): shield/lives/bombs/boss values
        // are surfaced through frame() instead of Hud_Set* calls.

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
                .map(|e| format!("sh={} f={:x} xyz=({},{},{})", e.shape_id, e.flags, e.x >> 16, e.y >> 16, e.z >> 16))
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
        // Mirror the unified WRAM lives store back to the shell-persistent
        // copy (survives map reloads, feeds the HUD + respawn/game-over).
        self.planets.lives = self.game.vars.read_ext8(0x0520);

        if self.game.vars.gameflags & GF_PLAYERDEAD != 0 {
            self.levelclear_ticks = 0;
            self.death_ticks += 1;
            if self.death_ticks < DEATH_RESPAWN_TICKS {
                return;
            }
            if self.planets.lives > 0 {
                // Reload the current map; route/stage/newmap untouched.
                self.begin_gameplay_from_planet_select();
            } else {
                self.death_ticks = 0;
                self.game_state = GameState::Continue;
            }
            return;
        }
        self.death_ticks = 0;

        // --- Level completion (boot.c:200-218) ---
        // mapend runs after the clear-demo submap + wipe script. TODO(C
        // parity): LE_ENTERBHOLE/LE_ENTERSPEC warp values currently advance
        // the normal route (boot.c:197-199 has the same caveat).
        if self.game.world.levelfinished != 0 {
            self.levelclear_ticks += 1;
            if self.levelclear_ticks < LEVEL_CLEAR_SETTLE_TICKS {
                return;
            }
            // ROM end_level_seq runs the tally screen before advancing
            // (MAIN.ASM:1077). Compute + record this stage's score, then hand
            // off to GameState::Tally; the stage advance happens on tally exit.
            self.enter_tally();
            return;
        }
        self.levelclear_ticks = 0;
    }

    /// ROM `end_level_seq` entry (MAIN.ASM:1077-1090): compute this stage's hit
    /// percentage (`calcstageperc`), append it to `specbuf`, run `checkbonus`
    /// (awarding a credit + bonus SFX on a `bonertab` crossing), and switch to
    /// the tally screen state.
    fn enter_tally(&mut self) {
        // Numerator: per-stage special kills (WRAM 0x1F0B, written by the
        // sf-strat explode strat). Denominator: the stable map-build count.
        let specials_dead = self.game.vars.read_ext8(score::SPECIALS_DEAD);
        let total_specials = self.game.world.total_specials;
        let teammates = score::teammates_alive(
            self.game.vars.bunny_hp,
            self.game.vars.frog_hp,
            self.game.vars.falcon_hp,
        );
        let perc = score::calc_stage_perc(specials_dead, total_specials, teammates);
        self.tally_stage_perc = perc;

        // Append to specbuf + checkbonus (MAIN.ASM:1213-1219).
        if self.planets.record_stage_score(perc) {
            // Bonus threshold crossed: credit awarded, play trigse $1a
            // (MAIN.ASM:1149).
            self.state
                .borrow_mut()
                .sound
                .push(SoundCmd::PlaySe(score::SE_BONUS));
        }

        self.levelclear_ticks = 0;
        self.tally_ticks = 0;
        self.game_state = GameState::Tally;
    }

    /// GameState::Tally hold + exit. Holds the tally figures for
    /// [`TALLY_HOLD_TICKS`] (or until START/A/B), then performs the ROM
    /// stage-advance / route-resolve that used to follow mapend directly.
    fn tally_tick(&mut self) {
        self.tally_ticks += 1;
        let held = self.tally_ticks >= TALLY_HOLD_TICKS;
        let pressed = self.pad1_new & (pad::START | pad::A | pad::B) != 0;
        // Dismiss on START/A/B once the figures have been shown, or auto-advance
        // after twice the hold so headless/AI runs still progress without input.
        if (held && pressed) || self.tally_ticks >= TALLY_HOLD_TICKS.saturating_mul(2) {
            self.advance_stage_after_tally();
        }
    }

    /// ROM post-tally stage advance (MAIN.ASM:229 `inc stage` + planetseq
    /// re-entry). Extracted from the old level-clear path.
    fn advance_stage_after_tally(&mut self) {
        self.tally_ticks = 0;
        self.planets.stage = self.planets.stage.wrapping_add(1);
        // ROM: level-finish does `inc stage` (MAIN.ASM:229) then re-enters
        // planetseq_l, which calls `convertroute` on entry (PLANETS.ASM:251)
        // and again on `.continuewithgame` before gamestart
        // (PLANETS.ASM:1090). The map walk therefore runs with the
        // *converted* route (e.g. gameplay whichroute 0 -> 1 -> root P6 ->
        // MAP_ID_1_2), then converts back for gameplay. Without this bracket
        // the walk used the raw gameplay route (P1 -> MAP_ID_2_2).
        self.planets.convertroute();
        let advanced = self.planets.drawplanetlines();
        self.planets.convertroute();
        if advanced {
            // Stage graph resolved the next map on this route.
            self.begin_gameplay_from_planet_select();
        } else {
            self.levelclear_ticks = 0;
            self.game_state = GameState::Ending;
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
    use sf_map::catalog::map_id;

    /// Scripted-pad state walk matching boot.c: BOOT tick only runs
    /// Game_Init; START at the title enters planet select; START at planet
    /// select launches gameplay.
    #[test]
    fn scripted_pad_state_walk() {
        let mut sh = Shell::new();
        assert_eq!(sh.state().code(), 0); // GAME_STATE_BOOT

        // The BOOT case only runs Game_Init in that tick (boot.c:227-229).
        sh.tick(0);
        assert_eq!(sh.state().code(), 1); // GAME_STATE_TITLE

        // Title lazy-load + run (no input).
        sh.tick(0);
        assert_eq!(sh.state().code(), 1);
        assert!(sh.game.world.map_loaded);

        // Tap START: SE 0x10 + Planets_Init -> planet select (boot.c:158).
        sh.tick(pad::START);
        assert_eq!(sh.state().code(), 3); // GAME_STATE_PLANET_SELECT
        let sounds = sh.drain_sound();
        assert!(sounds.contains(&SoundCmd::PlaySe(0x10)));

        // One idle planet-select tick (converts route 0 -> 1 and resolves
        // MAP_ID_1_1, planets.c:314-319). Release START first for the edge.
        sh.tick(0);
        assert_eq!(sh.state().code(), 3);
        assert_eq!(sh.frame().newmap, map_id::M1_1);

        // Tap START to launch (planets.c:336-346).
        sh.tick(pad::START);
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
        let mut sh = Shell::new();
        sh.tick(0); // boot -> title
        sh.tick(0); // title load
        sh.tick(pad::START); // -> planet select
        sh.tick(0);
        sh.tick(pad::START); // -> playing (route 0 -> MAP 1_1)
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
        // Hold the tally, then dismiss with START to advance the stage.
        for _ in 0..TALLY_HOLD_TICKS {
            sh.tick(0);
        }
        sh.tick(pad::START);
        let f = sh.frame();
        assert_eq!(f.stage, 1);
        assert_eq!(sh.state().code(), 4);
        // Route 0 (spine PATH_ID_6 -> routes[1]=PATH_ID_7): stage 1 is
        // MAP_ID_1_2 (planets.c:100).
        assert_eq!(f.newmap, map_id::M1_2);
    }

    /// Death path: GF_PLAYERDEAD + no lives -> CONTINUE after
    /// DEATH_RESPAWN_TICKS; continue restarts with refilled lives.
    #[test]
    fn death_to_continue_and_back() {
        let mut sh = Shell::new();
        sh.tick(0);
        sh.tick(0);
        sh.tick(pad::START);
        sh.tick(0);
        sh.tick(pad::START);
        assert_eq!(sh.state().code(), 4);

        // Lives are unified in WRAM 0x0520 during gameplay (the shell mirrors
        // it back each tick), so exhaust the canonical store.
        sh.planets.lives = 0;
        sh.game.vars.write_ext8(0x0520, 0);
        sh.game.vars.gameflags |= GF_PLAYERDEAD;
        for _ in 0..DEATH_RESPAWN_TICKS {
            sh.tick(0);
        }
        assert_eq!(sh.state().code(), 5); // GAME_STATE_CONTINUE

        sh.tick(0); // release edge
        sh.tick(pad::START); // continue: refill lives, retry stage
        assert_eq!(sh.state().code(), 4);
        assert_eq!(sh.frame().lives, DEFAULT_LIVES as i32);
        // Begin-gameplay cleared the dead flag (boot.c:55).
        assert!(!sh.frame().player_dead);
    }

    /// Drive a game into gameplay (BOOT -> PLAYING).
    fn into_gameplay() -> Shell {
        let mut sh = Shell::new();
        sh.tick(0); // boot -> title
        sh.tick(0); // title load
        sh.tick(pad::START); // -> planet select
        sh.tick(0);
        sh.tick(pad::START); // -> playing
        assert_eq!(sh.state().code(), 4);
        let _ = sh.drain_sound(); // clear launch SFX
        sh
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
        sh.game.vars.write_ext8(score::SPECIALS_DEAD, 10);
        sh.game.vars.bunny_hp = 3;
        sh.game.vars.frog_hp = 3;
        sh.game.vars.falcon_hp = 3;

        sh.enter_tally();
        assert_eq!(sh.state(), GameState::Tally);
        assert_eq!(sh.tally_stage_perc, 100);
        let f = sh.frame();
        assert_eq!(f.tally_stage_perc, 100);
        assert_eq!(f.score_total, 100); // calctotalscore
        assert_eq!(f.credits, 1); // crossed bonertab 100 -> +1 credit
        assert!(f.tally_active);
        assert!(sh.drain_sound().contains(&SoundCmd::PlaySe(score::SE_BONUS)));

        // A second, weaker stage (50%, no teammates) accumulates into the
        // running total but stays below the next threshold (300) -> no credit.
        sh.game.world.total_specials = 10;
        sh.game.vars.write_ext8(score::SPECIALS_DEAD, 5);
        sh.game.vars.bunny_hp = 0;
        sh.game.vars.frog_hp = 0;
        sh.game.vars.falcon_hp = 0;
        sh.enter_tally();
        assert_eq!(sh.tally_stage_perc, 50);
        assert_eq!(sh.frame().score_total, 150); // 100 + 50, calctotalscore
        assert_eq!(sh.frame().credits, 1); // 150 < 300, no new credit
        assert!(!sh.drain_sound().contains(&SoundCmd::PlaySe(score::SE_BONUS)));
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
            sh.game.vars.write_ext8(score::SPECIALS_DEAD, 1);
            sh.enter_tally();
            assert_eq!(sh.frame().score_total, expect_total);
            if sh.drain_sound().contains(&SoundCmd::PlaySe(score::SE_BONUS)) {
                credited += 1;
            }
        }
        assert_eq!(credited, 2); // 100 and 300 thresholds
        assert_eq!(sh.frame().credits, 2);
    }
}
