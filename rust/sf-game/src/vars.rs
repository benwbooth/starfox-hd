//! Game-variable state — the C globals the phase-1 game core touches.
//!
//! C oracle: `src/game/game_vars.c/h` (GILESALC.INC allocations) plus the
//! `src/sf_rtl.h` WRAM mirror (`g_ram`) and pad state. Only the globals that
//! `world.c` / `obj.c` / `coldet.c` / `map_exec.c` / the `Nmi_GameTick`
//! subset actually read or write are ported; every field cites its C name.
//! game_vars.c declares 246 globals; 43 are ported here (plus the map-VM
//! state world.c/obj.c export: lastplayz/lastzchange/lastmapobj/
//! specialobjtotal/levelfinished in [`crate::world::World`], the alien
//! pool/lists/aldead in [`crate::obj::Objects`]). The rest stay in C until
//! their lanes (strat/render/audio/windows) come over.

use sf_core::player_view::{PlayerViewMode, PlayerViewOptions};
use sf_core::scene::{DepthColors, DepthThresholds, GamePalette, PaletteFadeTarget, SceneStyle};
use sf_core::screen_fill_circle::ScreenFillCircleState;

// ============================================================
// Flag constants (C `src/variables.h`)
// ============================================================
// gameflags (GILESALC.INC)
pub const GF_NOZREMOVE: u8 = 1;
pub const GF_PLAYERDYING: u8 = 2;
pub const GF_BOSSDEAD: u8 = 4;
pub const GF_STRATDONE1: u8 = 8;
pub const GF_STRATDONE2: u8 = 16;
pub const GF_VIEWROT: u8 = 32;
pub const GF_PLAYERDEAD: u8 = 64;
pub const GF_STAGEDONE: u8 = 128;

// pshipflags
pub const PSF_BROKEN_LEFT_WING: u8 = 8;
pub const PSF_BROKEN_RIGHT_WING: u8 = 16;
pub const PSF_NOCTRL: u8 = 32;
pub const PSF_NOFIRE: u8 = 64;
pub const PSF_STAGE_DAMAGE: u8 = PSF_BROKEN_LEFT_WING | PSF_BROKEN_RIGHT_WING;

// pshipflags2
pub const PSF2_PLAYERHP0: u8 = 128;

// pshipflags3
pub const PSF3_INTUNNEL: u8 = 1;
pub const PSF3_ENGINESND: u8 = 2;
pub const PSF3_NOCOLLISIONS: u8 = 8;
pub const PSF3_NOVIEWCHANGE: u8 = 1 << 5;
pub const PSF3_KEEPPSTRAT: u8 = 64;

// pstratflags
pub const PSTF_NOVDISTC: u8 = 1;
pub const PSTF_INSEQ: u8 = 8;
pub const PSTF_NOTDIE: u8 = 32;

// playerflymode
pub const PFM_SHADOWS: u8 = 8;
pub const PFM_WOBBLE: u8 = 16;

/// `timeuntilfade` installed by the terminal player explosion.
pub const PLAYER_DEATH_FADE_DELAY_TICKS: u8 = 20;

// Game modes (C `src/variables.h`)
pub const SPACE_MODE: u8 = 1;
pub const WATER_MODE: u8 = 2;

/// ROM `palnum` start value (WORLD.ASM:376-379): a 15-frame walk, one
/// palette color per frame, stepping down by 2 to 0.
pub use sf_core::scene::PALETTE_FADE_COUNTER_START as PALFADE_NUM_START;

// bgflags (C `src/game/bgs.h` BGF_*)
pub const BGF_RESTART: u8 = 0x01;
pub const BGF_BG: u8 = 0x04;
pub const BGF_INFO: u8 = 0x08;

// Gameplay constants (C `src/variables.h` / STRATEQU.INC)
pub const OUTVIEWDIST: i16 = 120;
pub const CLOSE_VIEW_DISTANCE: i16 = 60;
pub const FRAMESPERAP: u8 = 10;
/// Source cadence that makes `framescalevecs` an identity transform.
pub const DEFAULT_FRAME_RATE: u8 = 4;
/// Nova bombs granted when a new run starts or a continue is accepted.
pub const DEFAULT_SPECIAL_WEAPON_COUNT: u16 = 3;
/// `stay_black` value used during ordinary interactive gameplay.
pub const STAY_BLACK_INACTIVE: i8 = -1;
pub use sf_map::catalog::BossEncounter;

// Enemy strategy constants (C `src/strat/strat_enemy.h`)
pub const HARD_HP: u8 = 0xFF;
pub const HARD_AP: u8 = 8;
pub const COLLTYPE_ENEMY1: u8 = 0x01;

/// Named variables used by the translated strategy code. These are semantic
/// identifiers, not source-machine addresses. Keeping them distinct fixes a
/// class of bugs in the old byte-array bridge where unrelated globals were
/// assigned overlapping scratch addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyVariable {
    RandomSeed,
    ProjectileStrategy,
    PlayerStrategyBase,
    PlayerRotationX,
    PlayerRotationY,
    PlayerRotationZ,
    PlayerSpeed,
    PlayerTargetSpeed,
    PlayerMediumSpeed,
    PlayerTurnRotation,
    PlayerDepthShake,
    PlayerDepthShakeVelocity,
    PlayerDepthTilt,
    PlayerDepthStrategyOffset,
    PlayerRollVelocity,
    PlayerRollOffset,
    PlayerRollDelay,
    PlayerControlDelay,
    PlayerRollFloatCursor,
    PlayerRollFloat,
    ViewShakeX,
    ViewShakeY,
    ViewShakeZ,
    ScreenFlashCount,
    ScreenFlashKind,
    PlayerHitCount,
    Lives,
    StayBlack,
    WipeActive,
    ArrowFlags,
    ViewCenterY,
    BoostDepthOffset,
    PlayerMinX,
    PlayerMaxX,
    PlayerMaxY,
    MouseMinX,
    MouseMaxX,
    MouseMaxY,
    WaterPlayerMinY,
    WaterPlayerMaxY,
    PlayerMoveLimit,
    PlayerMoveLimitMask,
    MissileBoundaryFlags,
    BoostCount,
    BoostObject,
    PlayerViewX,
    PlayerViewY,
    PlayerViewZ,
    BackgroundScrollZ,
    HudRotation,
    ViewPitch,
    ViewYaw,
    ViewDistance,
    ViewRoll,
    ViewKind,
    FadeDirection,
    ViewTargetObject,
    FixedViewX,
    FixedViewY,
    FixedViewZ,
    PlayerByte1,
    PlayerByte2,
    PlayerByte3,
    NoMaximumBackgroundY,
    BackgroundY,
    StrategyWord1,
    StrategyWord2,
    StrategyWord3,
    PlayerCollisionBody,
    PlayerCollisionLeftWing,
    PlayerCollisionRightWing,
    PlayerShapeIntact,
    PlayerShapeNoLeftWing,
    PlayerShapeNoRightWing,
    PlayerShapeNoWings,
    FireCount,
    FireDelay,
    SpecialDelay,
    SpecialWeaponCount,
    MissileTopLeft,
    MissileBottomLeft,
    MissileTopRight,
    ViewXOffset,
    ViewYOffset,
    SmokeVariable,
    FireSmokeStrategyBase,
    PuffStrategy,
    SparkyStrategy,
    CircleObject,
    PlayerLaserCount,
}

impl StrategyVariable {
    pub const RNDVAL: Self = Self::RandomSeed;
    pub const SID_PROJ: Self = Self::ProjectileStrategy;
    pub const SID_PLAYER_BASE: Self = Self::PlayerStrategyBase;
    pub const PLROTX: Self = Self::PlayerRotationX;
    pub const PLROTY: Self = Self::PlayerRotationY;
    pub const PLROTZ: Self = Self::PlayerRotationZ;
    pub const PLAYER_SPEED: Self = Self::PlayerSpeed;
    pub const PLAYER_TOSPEED: Self = Self::PlayerTargetSpeed;
    pub const PLAYER_MEDSPEED: Self = Self::PlayerMediumSpeed;
    pub const PLAYER_TURNROT: Self = Self::PlayerTurnRotation;
    pub const PLAYER_ZSHAKE: Self = Self::PlayerDepthShake;
    pub const PLAYER_ZSHAKE_VELOCITY: Self = Self::PlayerDepthShakeVelocity;
    pub const PLAYER_ZTILT: Self = Self::PlayerDepthTilt;
    pub const PLAYER_ZSTRATADD: Self = Self::PlayerDepthStrategyOffset;
    pub const PLAYER_ROLLZVEL: Self = Self::PlayerRollVelocity;
    pub const PLAYER_ROLLZOFF: Self = Self::PlayerRollOffset;
    pub const PLAYER_ROLLDELAY: Self = Self::PlayerRollDelay;
    pub const PLAYER_NOCTRLCNT: Self = Self::PlayerControlDelay;
    pub const PLAYER_ZROTFLOATPTR: Self = Self::PlayerRollFloatCursor;
    pub const PLAYER_ZROTFLOAT: Self = Self::PlayerRollFloat;
    pub const VIEWSHAKEX: Self = Self::ViewShakeX;
    pub const VIEWSHAKEY: Self = Self::ViewShakeY;
    pub const VIEWSHAKEZ: Self = Self::ViewShakeZ;
    pub const SCREENFLASHCNT: Self = Self::ScreenFlashCount;
    pub const SCREENFLASHTYPE: Self = Self::ScreenFlashKind;
    pub const PNUMHITS: Self = Self::PlayerHitCount;
    pub const LIVES: Self = Self::Lives;
    pub const STAYBLACK: Self = Self::StayBlack;
    pub const DOINGWIPE: Self = Self::WipeActive;
    pub const ARROWS: Self = Self::ArrowFlags;
    pub const VIEWCY: Self = Self::ViewCenterY;
    pub const BOOSTZOFF: Self = Self::BoostDepthOffset;
    pub const MINPMOVEX: Self = Self::PlayerMinX;
    pub const MAXPMOVEX: Self = Self::PlayerMaxX;
    pub const MAXPMOVEY: Self = Self::PlayerMaxY;
    pub const MINMMOVEX: Self = Self::MouseMinX;
    pub const MAXMMOVEX: Self = Self::MouseMaxX;
    pub const MAXMMOVEY: Self = Self::MouseMaxY;
    pub const MINPWMOVEY: Self = Self::WaterPlayerMinY;
    pub const MAXPWMOVEY: Self = Self::WaterPlayerMaxY;
    pub const PMOVELIMIT: Self = Self::PlayerMoveLimit;
    pub const PMOVELIMITAND: Self = Self::PlayerMoveLimitMask;
    pub const MISSBOUNDFLAGS: Self = Self::MissileBoundaryFlags;
    pub const BOOSTCNT: Self = Self::BoostCount;
    pub const BOOSTOBJ: Self = Self::BoostObject;
    pub const PVIEWPOSX: Self = Self::PlayerViewX;
    pub const PVIEWPOSY: Self = Self::PlayerViewY;
    pub const PVIEWPOSZ: Self = Self::PlayerViewZ;
    pub const BGSSCROLLZ: Self = Self::BackgroundScrollZ;
    pub const HUDROT: Self = Self::HudRotation;
    pub const OUTVX: Self = Self::ViewPitch;
    pub const OUTVY: Self = Self::ViewYaw;
    pub const OUTDIST: Self = Self::ViewDistance;
    pub const OUTVZ: Self = Self::ViewRoll;
    pub const VIEWTYPE: Self = Self::ViewKind;
    pub const FADEDIR: Self = Self::FadeDirection;
    pub const VIEWTOOBJ: Self = Self::ViewTargetObject;
    pub const VIEWPOSX: Self = Self::FixedViewX;
    pub const VIEWPOSY: Self = Self::FixedViewY;
    pub const VIEWPOSZ: Self = Self::FixedViewZ;
    pub const PSVAR_BYTE1: Self = Self::PlayerByte1;
    pub const PSVAR_BYTE2: Self = Self::PlayerByte2;
    pub const PSVAR_BYTE3: Self = Self::PlayerByte3;
    pub const NOMAXBG2YSCROLL: Self = Self::NoMaximumBackgroundY;
    pub const BG2YSCROLL: Self = Self::BackgroundY;
    pub const SVAR_WORD1: Self = Self::StrategyWord1;
    pub const SVAR_WORD2: Self = Self::StrategyWord2;
    pub const SVAR_WORD3: Self = Self::StrategyWord3;
    pub const PCOLLOBJ_B: Self = Self::PlayerCollisionBody;
    pub const PCOLLOBJ_LW: Self = Self::PlayerCollisionLeftWing;
    pub const PCOLLOBJ_RW: Self = Self::PlayerCollisionRightWing;
    pub const PLAYERSHAPE: Self = Self::PlayerShapeIntact;
    pub const PLAYERSHAPEL: Self = Self::PlayerShapeNoLeftWing;
    pub const PLAYERSHAPER: Self = Self::PlayerShapeNoRightWing;
    pub const PLAYERSHAPELR: Self = Self::PlayerShapeNoWings;
    pub const FIRECNT: Self = Self::FireCount;
    pub const FIREDELAY: Self = Self::FireDelay;
    pub const SPECIALDELAY: Self = Self::SpecialDelay;
    pub const SPECWEPCNT: Self = Self::SpecialWeaponCount;
    pub const MISSBTOPLEFT: Self = Self::MissileTopLeft;
    pub const MISSBBOTLEFT: Self = Self::MissileBottomLeft;
    pub const MISSBTOPRIGHT: Self = Self::MissileTopRight;
    pub const VIEWPOSXOFF: Self = Self::ViewXOffset;
    pub const VIEWPOSYOFF: Self = Self::ViewYOffset;
    pub const SMVAR_BYTE1: Self = Self::SmokeVariable;
    pub const SID_FIRE_SMOKE: Self = Self::FireSmokeStrategyBase;
    pub const SID_PUFF: Self = Self::PuffStrategy;
    pub const SID_SPARKY: Self = Self::SparkyStrategy;
    pub const CIRCLEOBJ: Self = Self::CircleObject;
    pub const NUMPLASERS: Self = Self::PlayerLaserCount;
}

/// Strategy globals in the same semantic groups and scalar widths as the
/// original allocation records. Runtime callback handles are deliberately in
/// a separate record because they are native-port metadata, not game state.
#[derive(Debug, Clone, Default)]
pub struct StrategyVariables {
    pub random_seed: u16,
    /// Source `framerate`: elapsed display frames used by
    /// `framescalevecs` to compensate player X/Y motion.
    pub frame_rate: u8,
    pub player_rotation: [i16; 3],
    pub player_speed: i16,
    pub player_target_speed: u8,
    pub player_medium_speed: u8,
    pub player_turn_rotation: i16,
    pub player_depth_shake: i16,
    pub player_depth_shake_velocity: i16,
    pub player_depth_tilt: i8,
    pub player_depth_strategy_offset: u8,
    pub player_roll_velocity: i8,
    pub player_roll_offset: i8,
    pub player_roll_delay: u8,
    pub player_control_delay: u8,
    pub player_roll_float_cursor: u16,
    pub player_roll_float: i8,
    pub view_shake: [u8; 3],
    pub view_float_cursor: u16,
    pub view_float_x: i16,
    pub view_float_y: i16,
    pub screen_flash_count: u8,
    pub screen_flash_kind: u8,
    pub player_hit_count: u8,
    pub lives: u8,
    pub stay_black: i8,
    pub wipe_active: u8,
    pub arrow_flags: u8,
    pub view_center_y: i16,
    pub boost_depth_offset: i8,
    pub player_min_x: i16,
    pub player_max_x: i16,
    pub player_max_y: i16,
    pub mouse_min_x: i16,
    pub mouse_max_x: i16,
    pub mouse_max_y: i16,
    pub water_player_min_y: i16,
    pub water_player_max_y: i16,
    pub player_move_limit: u8,
    pub player_move_limit_mask: u8,
    pub missile_boundary_flags: u8,
    pub boost_count: u8,
    pub boost_object: i16,
    pub player_view_position: [i16; 3],
    pub background_scroll_z: i16,
    pub hud_rotation: i16,
    pub view_pitch: i16,
    pub view_yaw: i16,
    pub view_distance: i16,
    pub view_roll: i16,
    pub view_kind: u8,
    pub fade_direction: i8,
    pub view_target_object: i16,
    pub fixed_view_position: [i16; 3],
    pub player_bytes: [u8; 3],
    pub no_maximum_background_y: u8,
    pub background_y: i16,
    pub strategy_words: [i16; 3],
    pub player_collision_objects: [i16; 3],
    pub player_shapes: [u16; 4],
    pub fire_count: u8,
    pub fire_delay: u8,
    pub special_delay: u8,
    pub special_weapon_count: u16,
    pub missile_bounds: [i16; 3],
    pub fixed_view_offset: [i16; 2],
    pub smoke_variable: u16,
    pub circle_object: i16,
    pub player_laser_count: u8,
    pub mother_accumulator: u16,
    /// Retail `exitintro`: the lead fighter has passed the camera and requests
    /// the attract presentation's fade back to the title.
    pub intro_exit_requested: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NativeStrategyBindings {
    pub projectile: u16,
    pub player_base: u16,
    pub enter_cockpit: Option<u16>,
    pub leave_cockpit: Option<u16>,
    pub fire_smoke_base: u16,
    pub puff: u16,
    pub sparky: u16,
    pub path: [u16; 7],
}

/// Globals written by imported map programs. The program decoder translates
/// encoded operands into these fields at the boundary; gameplay code never
/// treats the record as an address space.
#[derive(Debug, Clone, Default)]
pub struct MapVariables {
    pub skill_fly: u8,
    pub stage_clear: u8,
    pub clear_background_two: u8,
    pub level_finished: u8,
    pub one_credit_sprite: u8,
    pub in_fog: u8,
    pub fade_palette: u8,
    pub palette_from: u8,
    pub palette_to: u8,
    pub palette_length: u8,
    pub player_position_x: i16,
    pub global_strategy_byte: u8,
    pub trigger: u8,
    pub horizontal_position_jump: i16,
    pub variable1: u32,
    pub background_vertical_request: u16,
    pub background_horizontal_request: u16,
    /// One-byte credits/background vertical-scroll override.
    pub background_vertical_override: u8,
    pub background_y: i16,
    pub space_scroll_enabled: u8,
}

/// Globals declared together by `DALCS.INC` and shared by path programs.
/// The scalar widths intentionally follow the source allocation record;
/// despite their historical names, `gword1..3` are one byte each.
#[derive(Debug, Clone, Default)]
pub struct PathProgramVariables {
    pub byte1: u8,
    pub byte2: u8,
    pub byte3: u8,
    pub word1: u8,
    pub word2: u8,
    pub word3: u8,
    pub skill_fly: u8,
    pub pepper_fade: u8,
    pub pepper_characters: u8,
    pub pepper_message: u8,
    pub boss_hit_count: u8,
    pub slot_hold: [u8; 3],
    pub slot_spin: [u8; 3],
    pub slot_position: [u8; 3],
}

/// External path variables declared together by `EALCS.INC`.
#[derive(Debug, Clone, Default)]
pub struct EnemyPathVariables {
    pub byte1: u8,
    pub byte2: u8,
    pub flag1: u8,
    pub roll1: u8,
    pub byte3: u8,
    pub word1: u16,
    pub word2: u16,
}

/// Shared strategy and scene globals that were separate named allocations in
/// the original source. They remain separate here even when the former port's
/// synthetic address table accidentally aliased them.
#[derive(Debug, Clone, Default)]
pub struct SharedGameVariables {
    pub boss_flags: u8,
    pub game_flags2: u8,
    pub gas_flags: u8,
    pub strategy_flags: u8,
    pub player_score: u16,
    pub specials_dead: u8,
    pub map_restart: u16,
    pub map_restart_temporary: u16,
    pub restart_palette_fade: u16,
    pub last_palette_fade: u16,
    pub enemy_path: EnemyPathVariables,
    pub restart_background: u16,
    pub special_flash: u8,
    pub power_build: u8,
    pub locus_mode: u8,
    pub background_scroll_x: i16,
    pub background_scroll: u16,
    pub last_rotation: u16,
    pub do_depth_rotation: u8,
    pub no_pitch_rotation: u8,
    pub float_variables: [u8; 2],
    pub slime_count: u8,
    pub arm_mode: u8,
    pub stage: u8,
    pub collision_type: u8,
    pub friends_meter: u8,
    pub current_level: u8,
    pub difficulty_level: u8,
    pub score_ring_count: u16,
    pub path_program: PathProgramVariables,
}

/// The ported game-variable set. One field per C global (cited); default
/// zero-state matches C BSS, [`GameVars::init`] matches `GameVars_Init()`.
pub struct GameVars {
    /// Globals owned by the player/strategy translation units, represented as
    /// typed fields rather than a byte-addressed machine-memory image.
    pub strategy: StrategyVariables,
    /// Native callback handles used to wire translated strategies together.
    pub strategy_bindings: NativeStrategyBindings,
    pub map: MapVariables,
    pub shared: SharedGameVariables,
    // --- Game flags (game_vars.c) ---
    /// C `g_gameflags` (GF_*).
    pub gameflags: u8,

    // --- Player ship flags ---
    /// C `g_pshipflags` (PSF_*).
    pub pshipflags: u8,
    /// C `g_pshipflags2` (PSF2_*).
    pub pshipflags2: u8,
    /// C `g_pshipflags3` (PSF3_*).
    pub pshipflags3: u8,
    /// C `g_pstratflags` (PSTF_*).
    pub pstratflags: u8,
    /// C `g_playerflymode` (PFM_*).
    pub playerflymode: u8,
    /// ROM `splayerflymode`, represented as typed player-camera state.
    pub player_view_mode: PlayerViewMode,
    /// ROM `splayerflymodeopt`, the active background's selectable cycle.
    pub player_view_options: PlayerViewOptions,
    /// ROM `inatunnel` (ALCS.INC) — background environment mode consumed by
    /// `playersnd`: 0 normal/space, 1 tunnel, 2 water. This is deliberately
    /// separate from `PSF3_INTUNNEL`; takeoff and background transitions can
    /// change the sound environment without changing player collision mode.
    pub in_a_tunnel: u8,
    /// ROM `playersndflag` (KALCS.INC) — engine pitch bits written at the
    /// start of `viewmove_srou` (4 cruise, 8 boost, 12 brake).
    pub player_snd_flag: u8,

    // --- Player state mirrors (written by init_strats_l, GSTRATS.ASM) ---
    /// C `g_player_posx/y/z`.
    pub player_posx: i16,
    pub player_posy: i16,
    pub player_posz: i16,
    /// C `g_playervelZ`.
    pub playervel_z: i16,
    /// C `g_pviewvelz` — view Z velocity, read by the spacebar strats.
    pub pviewvelz: i16,

    // --- Counters ---
    /// C `g_gameframe`.
    pub gameframe: u16,
    /// Runtime RNG state — the ROM's `rand` ($DE-$E1), a 4-byte
    /// subtract-with-borrow chain (`RANDOM` $2F7BF). See `sf_random`. Boot
    /// value 0 (matches the ROM's cleared `rand`).
    pub rng: [u8; 4],
    /// C `g_freezestrats` (bit 0 freezes the strategy update).
    pub freezestrats: u8,
    /// C `g_internalPLAYPT` — authoritative player alien index.
    pub internal_playpt: i16,
    /// Source `timeuntilfade` as a named countdown rather than a memory slot.
    pub player_death_fade_delay: u8,
    /// C `g_dummyobj` — do_strat_l skip index (STRATROU.ASM dummyobj).
    pub dummyobj: i16,

    // --- Player strategy variables (world.c set_player_* callbacks) ---
    /// C `g_psvar_word1..4`.
    pub psvar_word1: i16,
    pub psvar_word2: i16,
    pub psvar_word3: i16,
    pub psvar_word4: i16,
    /// C `g_minpmoveY`.
    pub minpmove_y: i16,
    /// C `g_viewdist`.
    pub viewdist: i16,

    // --- Map VM state (game_vars.c) ---
    /// C `g_mapcnt` — distance countdown to next map-script execution.
    pub mapcnt: u16,
    /// C `g_mapptr` — map bytecode instruction pointer.
    pub mapptr: u16,
    /// C `g_stagecnt` (setstage opcode).
    pub stagecnt: i16,
    /// C `g_dotsflag` (-1 space dust, 0 none, 1 ground dots).
    pub dotsflag: i16,
    /// C `g_othmusic`.
    pub othmusic: u8,

    // --- Background state ---
    /// C `g_currentbg`.
    pub currentbg: u16,
    /// C `g_bgflags` (BGF_*).
    pub bgflags: u8,
    /// C `g_bg_dmalist`.
    pub bg_dmalist: u16,
    /// C `g_bgtransspeed`.
    pub bgtransspeed: u16,
    /// ROM `dovofs` — BG2 vertical-offset HDMA enable (WORLD.ASM vofsonplease).
    pub dovofs: u8,
    /// ROM `dohofs` — BG2 horizontal-offset enable (WORLD.ASM sethofson/off).
    pub dohofs: u8,
    /// ROM `bgmode` — PPU BGMODE mirror written by vofs on/off (1=off, 2=on).
    pub bgmode: u8,
    /// ROM `bg2vofs` — BG2 vertical scroll latch copied from the shared
    /// background-scroll field.
    pub bg2vofs: u16,
    /// Typed mirror of the independent polygon-palette, distance-colour,
    /// distance-threshold, and shadow-plane selections made by BGS.ASM.
    pub scene_style: SceneStyle,

    // --- Shape-palette fade (WORLD.ASM fadetoseado/fadetogrounddo) ---
    /// ROM `palnum` — remaining fade-walk words. Armed to
    /// [`PALFADE_NUM_START`] by FADETOSEA/FADETOGROUND, stepped -2 once per
    /// frame by the `fadepalto_l` mirror in `Game::step_palette_fade`
    /// (MAIN.ASM:2773-2776);
    /// 0 = idle/finished.
    pub palfade_num: u16,
    /// Source row selected by the ROM `palfade` cursor. `None` means no
    /// scripted sea/ground walk has been armed yet.
    pub palfade_target: Option<PaletteFadeTarget>,

    // --- Boss/HUD mirrors written by level inline callbacks ---
    /// C `g_bossmaxhp`.
    pub bossmaxhp: u16,
    /// C `m_bossHP` (MVARS.MC:251) — per-frame boss HP accumulator. Zeroed
    /// each frame in `init_strats` (ROM zeroes it in `mdrawbossHP`,
    /// MDRAWLIS.MC:1057, after the bar draws) and re-summed by every living
    /// boss part's `s_add_bossHP x,al_hp` (STRATLIB.INC:562). The HUD boss
    /// bar fill = `bosshp / bossmaxhp`.
    pub bosshp: u16,
    /// ROM `shieldup` (ALCS.INC) — wireframe-shield meter color flag
    /// (TRANS.ASM:604 → m_shieldup; MDRAWLIS.MC color 7 vs 2).
    pub shieldup: u8,
    /// ROM `wireendflash` — countdown while wire-ship is ending (PSTRATS.ASM:1780).
    pub wireendflash: u8,
    /// C `g_meters`.
    pub meters: u16,
    /// C `g_circleanim`.
    pub circleanim: i16,
    /// Live semantic replacement for the source fixed-colour circle cursor.
    pub screen_fill_circle: ScreenFillCircleState,
    /// C `g_oncewipe`.
    pub oncewipe: u8,

    // --- Game mode ---
    /// C `g_game_mode` (SPACE_MODE / WATER_MODE).
    pub game_mode: u8,

    // --- Friend HP (world.c friend-alive / CLfriendmsg callbacks) ---
    /// C `g_frog_hp`.
    pub frog_hp: u8,
    /// C `g_bunny_hp`.
    pub bunny_hp: u8,
    /// C `g_falcon_hp` ("cock" in ASM).
    pub falcon_hp: u8,
    /// C `g_numendok` (KSTRATS.ASM theenddead state).
    pub numendok: u8,

    /// ROM `boss_seq` layout — ordered end-sequence encounter marks.
    pub boss_seq: [Option<BossEncounter>; 40],
    /// ROM `boss_ptr` — word count into [`Self::boss_seq`] (0 = empty).
    pub boss_seq_len: u8,

    // --- Pad latch (TRANS.ASM lastcont; C sf_rtl.h g_pad1) ---
    /// C `g_pad1`.
    pub pad1: u16,
    /// C `g_lastcont0`.
    pub lastcont0: u8,
    /// C `g_lastcontl0`.
    pub lastcontl0: u8,
}

impl Default for GameVars {
    fn default() -> Self {
        GameVars {
            strategy: StrategyVariables {
                frame_rate: DEFAULT_FRAME_RATE,
                ..StrategyVariables::default()
            },
            strategy_bindings: NativeStrategyBindings::default(),
            map: MapVariables::default(),
            shared: SharedGameVariables::default(),
            gameflags: 0,
            rng: [0, 0, 0, 0],
            pshipflags: 0,
            pshipflags2: 0,
            pshipflags3: 0,
            pstratflags: 0,
            playerflymode: 0,
            player_view_mode: PlayerViewMode::Exterior,
            player_view_options: PlayerViewOptions::Unconfigured,
            in_a_tunnel: 0,
            player_snd_flag: 0,
            player_posx: 0,
            player_posy: 0,
            player_posz: 0,
            playervel_z: 0,
            pviewvelz: 0,
            gameframe: 0,
            freezestrats: 0,
            internal_playpt: 0,
            player_death_fade_delay: 0,
            dummyobj: 0,
            psvar_word1: 0,
            psvar_word2: 0,
            psvar_word3: 0,
            psvar_word4: 0,
            minpmove_y: 0,
            viewdist: 0,
            mapcnt: 0,
            mapptr: 0,
            stagecnt: 0,
            dotsflag: 0,
            othmusic: 0,
            currentbg: 0,
            bgflags: 0,
            bg_dmalist: 0,
            bgtransspeed: 0,
            dovofs: 0,
            dohofs: 0,
            bgmode: 1, // ROM idle = Mode 1 (vofsoffplease)
            bg2vofs: 0,
            scene_style: SceneStyle::default(),
            palfade_num: 0,
            palfade_target: None,
            bossmaxhp: 0,
            bosshp: 0,
            shieldup: 0,
            wireendflash: 0,
            meters: 0,
            circleanim: 0,
            screen_fill_circle: ScreenFillCircleState::inactive(),
            oncewipe: 0,
            game_mode: 0,
            frog_hp: 0,
            bunny_hp: 0,
            falcon_hp: 0,
            numendok: 0,
            boss_seq: [None; 40],
            boss_seq_len: 0,
            pad1: 0,
            lastcont0: 0,
            lastcontl0: 0,
        }
    }
}

impl GameVars {
    /// Advance the source game's four-byte runtime random stream once.
    ///
    /// The state is ordinary flat game data. Each byte is replaced in the
    /// source-defined subtract-with-borrow chain, and the new first byte is
    /// the generated value.
    pub fn advance_random(&mut self) -> u8 {
        let original_first = self.rng[0];
        let sources = [self.rng[1], self.rng[2], self.rng[3], original_first];
        let destinations = [1usize, 2, 3, 0];
        let mut value = original_first;
        let mut borrow = true;
        for (source, destination) in sources.into_iter().zip(destinations) {
            let (difference, source_borrow) = value.overflowing_sub(source);
            let (difference, carried_borrow) = difference.overflowing_sub(u8::from(borrow));
            borrow = source_borrow || carried_borrow;
            value = difference;
            self.rng[destination] = value;
        }
        self.rng[0]
    }

    /// Append an ending replay mark, suppressing a duplicate consecutive
    /// marker exactly like the source sequence recorder.
    pub fn mark_boss_encounter(&mut self, encounter: BossEncounter) {
        let count = usize::from(self.boss_seq_len);
        if count > 0 && self.boss_seq[count - 1] == Some(encounter) {
            return;
        }
        let Some(slot) = self.boss_seq.get_mut(count) else {
            return;
        };
        *slot = Some(encounter);
        self.boss_seq_len = self.boss_seq_len.saturating_add(1);
    }

    /// C `GameVars_Init()` (src/game/game_vars.c:348) — the subset covering
    /// the ported fields, with the same default values.
    pub fn init() -> Self {
        GameVars {
            playerflymode: PFM_SHADOWS, // shadows on by default
            player_view_mode: PlayerViewMode::Exterior,
            minpmove_y: -60,
            game_mode: SPACE_MODE,
            frog_hp: 3,
            bunny_hp: 3,
            falcon_hp: 3,
            oncewipe: 1,
            ..GameVars::default()
        }
    }

    /// Apply the source `playerstart_init_l` state owned by the game core.
    ///
    /// This is a run/continue boundary, not a per-stage player-spawn reset:
    /// special weapons and ship damage state persist while advancing between
    /// stages and reset only when the player starts a new run or continues.
    pub fn reset_player_run_state(&mut self) {
        self.strategy.special_weapon_count = DEFAULT_SPECIAL_WEAPON_COUNT;
        self.strategy.stay_black = STAY_BLACK_INACTIVE;
        self.pshipflags = 0;
        self.pshipflags2 = 0;
        self.pshipflags3 = 0;
        self.shieldup = 0;
        self.wireendflash = 0;
        self.player_death_fade_delay = 0;
        self.screen_fill_circle.clear();
    }

    /// Apply the terminal environment macro from a `BGS.ASM` background
    /// script. The HD map catalog uses normalized background ids 0..44; blink
    /// entries (1, 2, 21 and 32) contain no environment macro and therefore
    /// retain the preceding value, as the ROM does.
    pub fn set_sound_environment_for_bg(&mut self, bg_id: u16) {
        self.in_a_tunnel = match bg_id {
            // tunnel / nucleus / final / undergnd
            0 | 8 | 10 | 11 | 16 | 17 | 18 | 25 | 28 | 29 | 34 | 36 => 1,
            // water / colony
            24 | 27 => 2,
            // planet / space (including shell backgrounds and training)
            3..=7 | 9 | 12..=15 | 19..=20 | 22..=23 | 26 | 30..=31 | 33 | 35 | 37..=44 | 62 => 0,
            // BGS blink/reserved entries have no terminal environment macro.
            _ => return,
        };
    }

    /// Apply the visual state selected by one complete BGS.ASM background.
    ///
    /// Every full background starts through `init_bg`, whose mode setup resets
    /// the live game palette and distance thresholds to normal before the
    /// background-specific operations run. Blink-only entries contain no
    /// setup and intentionally retain the preceding scene.
    pub fn set_scene_style_for_bg(&mut self, bg_id: u16) {
        if matches!(bg_id, 1 | 2 | 21 | 32) {
            return;
        }

        let game_palette = match bg_id {
            3 | 15..=19 | 28..=29 | 36 | 38 => GamePalette::Red,
            4 | 23 | 31 | 44 => GamePalette::Blue,
            _ => GamePalette::Night,
        };
        let depth_colors = match bg_id {
            3 | 16 | 29 | 36 => DepthColors::Red,
            18 | 24 => DepthColors::Marine,
            23 | 31 => DepthColors::Mist,
            _ => DepthColors::Night,
        };
        let depth_thresholds = match bg_id {
            0 => DepthThresholds::Tunnel,
            4 | 44 => DepthThresholds::StageOne,
            13 | 23 => DepthThresholds::Mist,
            _ => DepthThresholds::Normal,
        };
        let shadow_height = if matches!(bg_id, 10 | 11) { 400 } else { 0 };

        self.scene_style = SceneStyle {
            game_palette,
            depth_colors,
            depth_thresholds,
            shadow_height,
        };
    }

    /// Titania's post-fog helper is not a complete background load. It
    /// explicitly changes only these four scene selections.
    pub fn set_titania_clear_scene(&mut self) {
        self.scene_style = SceneStyle {
            game_palette: GamePalette::Red,
            depth_colors: DepthColors::Red,
            depth_thresholds: DepthThresholds::Normal,
            shadow_height: 0,
        };
    }

    /// Decode an 8-bit variable operand from retained map/path source data.
    /// This is an import boundary over typed fields, not a memory read.
    pub fn read_ext8(&self, encoded: u16) -> u8 {
        let low = |value: u16| value as u8;
        let high = |value: u16| (value >> 8) as u8;
        match encoded {
            0x0304 => self.map.skill_fly,
            0x0305 => self.map.stage_clear,
            0x0306 => self.map.clear_background_two,
            0x0307 => self.map.level_finished,
            0x0308 => self.map.one_credit_sprite,
            0x0309 => self.map.in_fog,
            0x030A => self.map.fade_palette,
            0x030B => self.map.palette_from,
            0x030C => self.map.palette_to,
            0x030D => self.map.palette_length,
            0x030E => low(self.map.player_position_x as u16),
            0x030F => high(self.map.player_position_x as u16),
            0x0310 => self.map.global_strategy_byte,
            0x0311 => self.map.trigger,
            0x0312 => self.numendok,
            0x0313 => self.strategy.player_laser_count,
            0x0316 => low(self.bossmaxhp),
            0x0317 => high(self.bossmaxhp),
            0x0320 => self.map.variable1 as u8,
            0x0321 => (self.map.variable1 >> 8) as u8,
            0x0322 => (self.map.variable1 >> 16) as u8,
            0x0510 => low(self.strategy.player_turn_rotation as u16),
            0x0511 => high(self.strategy.player_turn_rotation as u16),
            0x0520 => self.strategy.lives,
            0x053C => low(self.strategy.player_view_position[0] as u16),
            0x053D => high(self.strategy.player_view_position[0] as u16),
            0x053E => low(self.strategy.player_view_position[1] as u16),
            0x053F => high(self.strategy.player_view_position[1] as u16),
            0x0540 => low(self.strategy.player_view_position[2] as u16),
            0x0541 => high(self.strategy.player_view_position[2] as u16),
            0x0546 => low(self.strategy.view_pitch as u16),
            0x0547 => high(self.strategy.view_pitch as u16),
            0x0548 => low(self.strategy.view_yaw as u16),
            0x0549 => high(self.strategy.view_yaw as u16),
            0x054A => low(self.strategy.view_distance as u16),
            0x054B => high(self.strategy.view_distance as u16),
            0x054C => self.strategy.view_kind,
            0x054D => self.strategy.fade_direction as u8,
            0x054E => low(self.strategy.view_target_object as u16),
            0x054F => high(self.strategy.view_target_object as u16),
            0x0550 => low(self.strategy.fixed_view_position[0] as u16),
            0x0551 => high(self.strategy.fixed_view_position[0] as u16),
            0x0552 => low(self.strategy.fixed_view_position[1] as u16),
            0x0553 => high(self.strategy.fixed_view_position[1] as u16),
            0x0554 => low(self.strategy.fixed_view_position[2] as u16),
            0x0555 => high(self.strategy.fixed_view_position[2] as u16),
            0x056E => low(self.strategy.special_weapon_count),
            0x056F => high(self.strategy.special_weapon_count),
            0x155C => self.shared.game_flags2,
            0x1569 => self.shared.float_variables[0],
            0x156A => self.shared.float_variables[1],
            0x162B => self.shared.slime_count,
            0x1721 => low(self.map.background_y as u16),
            0x1722 => high(self.map.background_y as u16),
            0x1727 => self.map.space_scroll_enabled,
            0x175B => self.shared.stage,
            0x1776 => self.shared.do_depth_rotation,
            0x17F0 => self.shared.arm_mode,
            0x1948 => low(self.strategy.view_roll as u16),
            0x1949 => high(self.strategy.view_roll as u16),
            0x1962 => self.strategy.stay_black as u8,
            0x1A13 => self.shared.collision_type,
            0x1A39 => low(self.map.background_vertical_request),
            0x1A3A => high(self.map.background_vertical_request),
            0x1A3B => low(self.map.background_horizontal_request),
            0x1A3C => high(self.map.background_horizontal_request),
            0x1ACA => self.shared.no_pitch_rotation,
            0x1AE1 => self.map.background_vertical_override,
            0x1AE6 => low(self.map.horizontal_position_jump as u16),
            0x1AE7 => high(self.map.horizontal_position_jump as u16),
            0x1AFF => self.shared.friends_meter,
            0x1B01 => self.shared.current_level,
            0x1F00 => low(self.strategy.random_seed),
            0x1F01 => high(self.strategy.random_seed),
            0x1F02 => self.shared.boss_flags,
            0x1F03 => self.shared.difficulty_level,
            0x1F04 => self.shared.gas_flags,
            0x1F05 => self.shared.strategy_flags,
            0x1F06 => low(self.shared.player_score),
            0x1F07 => high(self.shared.player_score),
            0x1F08 => low(self.strategy.special_weapon_count),
            0x1F09 => high(self.strategy.special_weapon_count),
            0x1F0B => self.shared.specials_dead,
            0x1F0E => low(self.shared.map_restart),
            0x1F0F => high(self.shared.map_restart),
            0x1F10 => low(self.shared.map_restart_temporary),
            0x1F11 => high(self.shared.map_restart_temporary),
            0x1F14 => low(self.shared.restart_palette_fade),
            0x1F15 => high(self.shared.restart_palette_fade),
            0x1F16 => low(self.shared.last_palette_fade),
            0x1F17 => high(self.shared.last_palette_fade),
            0x1F18 | 0x2302 | 0xF168 => self.shared.enemy_path.roll1,
            0x1F1A => low(self.shared.restart_background),
            0x1F1B => high(self.shared.restart_background),
            0x1F1C => low(self.strategy.player_max_x as u16),
            0x1F1D => high(self.strategy.player_max_x as u16),
            0x1F1E => low(self.strategy.player_min_x as u16),
            0x1F1F => high(self.strategy.player_min_x as u16),
            0x1F20 => low(self.strategy.player_max_y as u16),
            0x1F21 => high(self.strategy.player_max_y as u16),
            0x1F22 => low(self.strategy.view_center_y as u16),
            0x1F23 => high(self.strategy.view_center_y as u16),
            0x1F24 => low(self.strategy.player_view_position[2] as u16),
            0x1F25 => high(self.strategy.player_view_position[2] as u16),
            0x1F26 => low(self.strategy.player_collision_objects[0] as u16),
            0x1F27 => high(self.strategy.player_collision_objects[0] as u16),
            0x1F28 => self.shared.special_flash,
            0x1F29 => self.shared.power_build,
            0x1F2A => self.shared.locus_mode,
            0x1F30 => low(self.shared.background_scroll_x as u16),
            0x1F31 => high(self.shared.background_scroll_x as u16),
            0x1F32 => low(self.shared.background_scroll),
            0x1F33 => high(self.shared.background_scroll),
            0x1F34 => low(self.shared.last_rotation),
            0x1F35 => high(self.shared.last_rotation),
            0x2300 => low(self.shared.score_ring_count),
            0x2301 => high(self.shared.score_ring_count),
            0x2303 | 0xF169 => self.shared.enemy_path.byte3,
            0xF165 => self.shared.enemy_path.byte1,
            0xF166 => self.shared.enemy_path.byte2,
            0xF167 => self.shared.enemy_path.flag1,
            0xF16A => self.shared.enemy_path.word1 as u8,
            0xF16B => (self.shared.enemy_path.word1 >> 8) as u8,
            0xF16C => self.shared.enemy_path.word2 as u8,
            0xF16D => (self.shared.enemy_path.word2 >> 8) as u8,
            0xF147 => self.shared.path_program.byte1,
            0xF148 => self.shared.path_program.byte2,
            0xF149 => self.shared.path_program.byte3,
            0xF14A => self.shared.path_program.word1,
            0xF14B => self.shared.path_program.word2,
            0xF14C => self.shared.path_program.word3,
            0xF14D => self.shared.path_program.skill_fly,
            0xF14E => self.shared.path_program.pepper_fade,
            0xF14F => self.shared.path_program.pepper_characters,
            0xF150 => self.shared.path_program.pepper_message,
            0xF151 => self.shared.path_program.boss_hit_count,
            0xF152 => self.shared.path_program.slot_hold[0],
            0xF153 => self.shared.path_program.slot_hold[1],
            0xF154 => self.shared.path_program.slot_hold[2],
            0xF155 => self.shared.path_program.slot_spin[0],
            0xF156 => self.shared.path_program.slot_spin[1],
            0xF157 => self.shared.path_program.slot_spin[2],
            0xF158 => self.shared.path_program.slot_position[0],
            0xF159 => self.shared.path_program.slot_position[1],
            0xF15A => self.shared.path_program.slot_position[2],
            // Imported-path literal sintabs (`PATH_EXT_SINTAB` from the
            // native builder, and the retail blob's copy at $8B62 in the
            // path bank); same 127-amplitude Q8 table as STRATROU `sintab`.
            0x2200..=0x22FF => sf_core::snes_trig::SINTAB[(encoded & 0x00FF) as usize] as u8,
            0x8B62..=0x8C61 => sf_core::snes_trig::SINTAB[(encoded - 0x8B62) as usize] as u8,
            _ => panic!("untranslated imported 8-bit variable operand {encoded:#06x}"),
        }
    }

    /// Decode a 16-bit variable operand from retained map/path source data.
    pub fn read_ext16(&self, encoded: u16) -> u16 {
        match encoded {
            0x030E => self.map.player_position_x as u16,
            0x0316 => self.bossmaxhp,
            0x0320 => self.map.variable1 as u16,
            0x0510 => self.strategy.player_turn_rotation as u16,
            0x0524 => self.strategy.view_center_y as u16,
            0x0526 => self.strategy.player_min_x as u16,
            0x0528 => self.strategy.player_max_x as u16,
            0x052A => self.strategy.player_max_y as u16,
            0x053C => self.strategy.player_view_position[0] as u16,
            0x053E => self.strategy.player_view_position[1] as u16,
            0x0540 => self.strategy.player_view_position[2] as u16,
            0x0546 => self.strategy.view_pitch as u16,
            0x0548 => self.strategy.view_yaw as u16,
            0x054A => self.strategy.view_distance as u16,
            0x054C => self.strategy.view_kind.into(),
            0x054E => self.strategy.view_target_object as u16,
            0x0550 => self.strategy.fixed_view_position[0] as u16,
            0x0552 => self.strategy.fixed_view_position[1] as u16,
            0x0554 => self.strategy.fixed_view_position[2] as u16,
            0x055A => self.strategy.background_y as u16,
            0x056E => self.strategy.special_weapon_count,
            0x1721 => self.map.background_y as u16,
            0x1948 => self.strategy.view_roll as u16,
            0x1A39 => self.map.background_vertical_request,
            0x1A3B => self.map.background_horizontal_request,
            0x1AE6 => self.map.horizontal_position_jump as u16,
            0x1F00 => self.strategy.random_seed,
            0x1F06 => self.shared.player_score,
            0x1F08 => self.strategy.special_weapon_count,
            0x1F0E => self.shared.map_restart,
            0x1F10 => self.shared.map_restart_temporary,
            0x1F14 => self.shared.restart_palette_fade,
            0x1F16 => self.shared.last_palette_fade,
            0x1F1A => self.shared.restart_background,
            0x1F1C => self.strategy.player_max_x as u16,
            0x1F1E => self.strategy.player_min_x as u16,
            0x1F20 => self.strategy.player_max_y as u16,
            0x1F22 => self.strategy.view_center_y as u16,
            0x1F24 => self.strategy.player_view_position[2] as u16,
            0x1F26 => self.strategy.player_collision_objects[0] as u16,
            0x1F30 => self.shared.background_scroll_x as u16,
            0x1F32 => self.shared.background_scroll,
            0x1F34 => self.shared.last_rotation,
            0x2300 => self.shared.score_ring_count,
            0xF16A => self.shared.enemy_path.word1,
            0xF16C => self.shared.enemy_path.word2,
            _ => {
                u16::from(self.read_ext8(encoded))
                    | (u16::from(self.read_ext8(encoded.wrapping_add(1))) << 8)
            }
        }
    }

    /// Apply an 8-bit imported variable operand to its typed destination.
    pub fn write_ext8(&mut self, encoded: u16, value: u8) {
        let replace_low = |word: &mut u16| *word = (*word & 0xFF00) | u16::from(value);
        let replace_high = |word: &mut u16| *word = (*word & 0x00FF) | (u16::from(value) << 8);
        match encoded {
            0x0304 => self.map.skill_fly = value,
            0x0305 => self.map.stage_clear = value,
            0x0306 => self.map.clear_background_two = value,
            0x0307 => self.map.level_finished = value,
            0x0308 => self.map.one_credit_sprite = value,
            0x0309 => self.map.in_fog = value,
            0x030A => self.map.fade_palette = value,
            0x030B => self.map.palette_from = value,
            0x030C => self.map.palette_to = value,
            0x030D => self.map.palette_length = value,
            0x0310 => self.map.global_strategy_byte = value,
            0x0311 => self.map.trigger = value,
            0x0312 => self.numendok = value,
            0x0313 => self.strategy.player_laser_count = value,
            0x0320 => self.map.variable1 = (self.map.variable1 & 0xFFFF_FF00) | u32::from(value),
            0x0321 => {
                self.map.variable1 = (self.map.variable1 & 0xFFFF_00FF) | (u32::from(value) << 8)
            }
            0x0322 => {
                self.map.variable1 = (self.map.variable1 & 0xFF00_FFFF) | (u32::from(value) << 16)
            }
            0x0520 => self.strategy.lives = value,
            0x054C => self.strategy.view_kind = value,
            0x054D => self.strategy.fade_direction = value as i8,
            0x155C => self.shared.game_flags2 = value,
            0x1569 => self.shared.float_variables[0] = value,
            0x156A => self.shared.float_variables[1] = value,
            0x162B => self.shared.slime_count = value,
            0x1727 => self.map.space_scroll_enabled = value,
            0x175B => self.shared.stage = value,
            0x1776 => self.shared.do_depth_rotation = value,
            0x17F0 => self.shared.arm_mode = value,
            0x1962 => self.strategy.stay_black = value as i8,
            0x1A13 => self.shared.collision_type = value,
            0x1AE1 => self.map.background_vertical_override = value,
            0x1ACA => self.shared.no_pitch_rotation = value,
            0x1AFF => self.shared.friends_meter = value,
            0x1B01 => self.shared.current_level = value,
            0x1F03 => self.shared.difficulty_level = value,
            0x1F02 => self.shared.boss_flags = value,
            0x1F04 => self.shared.gas_flags = value,
            0x1F05 => self.shared.strategy_flags = value,
            0x1F0B => self.shared.specials_dead = value,
            0x1F18 | 0x2302 | 0xF168 => self.shared.enemy_path.roll1 = value,
            0x1F28 => self.shared.special_flash = value,
            0x1F29 => self.shared.power_build = value,
            0x1F2A => self.shared.locus_mode = value,
            0x2303 | 0xF169 => self.shared.enemy_path.byte3 = value,
            0xF165 => self.shared.enemy_path.byte1 = value,
            0xF166 => self.shared.enemy_path.byte2 = value,
            0xF167 => self.shared.enemy_path.flag1 = value,
            0xF147 => self.shared.path_program.byte1 = value,
            0xF148 => self.shared.path_program.byte2 = value,
            0xF149 => self.shared.path_program.byte3 = value,
            0xF14A => self.shared.path_program.word1 = value,
            0xF14B => self.shared.path_program.word2 = value,
            0xF14C => self.shared.path_program.word3 = value,
            0xF14D => self.shared.path_program.skill_fly = value,
            0xF14E => self.shared.path_program.pepper_fade = value,
            0xF14F => self.shared.path_program.pepper_characters = value,
            0xF150 => self.shared.path_program.pepper_message = value,
            0xF151 => self.shared.path_program.boss_hit_count = value,
            0xF152 => self.shared.path_program.slot_hold[0] = value,
            0xF153 => self.shared.path_program.slot_hold[1] = value,
            0xF154 => self.shared.path_program.slot_hold[2] = value,
            0xF155 => self.shared.path_program.slot_spin[0] = value,
            0xF156 => self.shared.path_program.slot_spin[1] = value,
            0xF157 => self.shared.path_program.slot_spin[2] = value,
            0xF158 => self.shared.path_program.slot_position[0] = value,
            0xF159 => self.shared.path_program.slot_position[1] = value,
            0xF15A => self.shared.path_program.slot_position[2] = value,
            _ => {
                let base = encoded & !1;
                let mut word = self.read_ext16(base);
                if encoded & 1 == 0 {
                    replace_low(&mut word)
                } else {
                    replace_high(&mut word)
                }
                self.write_ext16(base, word);
            }
        }
    }

    /// Apply a 16-bit imported variable operand to its typed destination.
    pub fn write_ext16(&mut self, encoded: u16, value: u16) {
        match encoded {
            0x030E => self.map.player_position_x = value as i16,
            0x0316 => self.bossmaxhp = value,
            0x0320 => self.map.variable1 = (self.map.variable1 & 0xFFFF_0000) | u32::from(value),
            0x0510 => self.strategy.player_turn_rotation = value as i16,
            0x0524 => self.strategy.view_center_y = value as i16,
            0x0526 | 0x1F1E => self.strategy.player_min_x = value as i16,
            0x0528 | 0x1F1C => self.strategy.player_max_x = value as i16,
            0x052A | 0x1F20 => self.strategy.player_max_y = value as i16,
            0x053C => self.strategy.player_view_position[0] = value as i16,
            0x053E => self.strategy.player_view_position[1] = value as i16,
            0x0540 | 0x1F24 => self.strategy.player_view_position[2] = value as i16,
            0x0546 => self.strategy.view_pitch = value as i16,
            0x0548 => self.strategy.view_yaw = value as i16,
            0x054A => self.strategy.view_distance = value as i16,
            0x054C => self.strategy.view_kind = value as u8,
            0x054E => self.strategy.view_target_object = value as i16,
            0x0550 => self.strategy.fixed_view_position[0] = value as i16,
            0x0552 => self.strategy.fixed_view_position[1] = value as i16,
            0x0554 => self.strategy.fixed_view_position[2] = value as i16,
            0x055A => self.strategy.background_y = value as i16,
            0x056E | 0x1F08 => self.strategy.special_weapon_count = value,
            0x1721 => self.map.background_y = value as i16,
            0x1948 => self.strategy.view_roll = value as i16,
            0x1A39 => self.map.background_vertical_request = value,
            0x1A3B => self.map.background_horizontal_request = value,
            0x1AE6 => self.map.horizontal_position_jump = value as i16,
            0x1F00 => self.strategy.random_seed = value,
            0x1F06 => self.shared.player_score = value,
            0x1F0E => self.shared.map_restart = value,
            0x1F10 => self.shared.map_restart_temporary = value,
            0x1F14 => self.shared.restart_palette_fade = value,
            0x1F16 => self.shared.last_palette_fade = value,
            0x1F1A => self.shared.restart_background = value,
            0x1F22 => self.strategy.view_center_y = value as i16,
            0x1F26 => self.strategy.player_collision_objects[0] = value as i16,
            0x1F30 => self.shared.background_scroll_x = value as i16,
            0x1F32 => self.shared.background_scroll = value,
            0x1F34 => self.shared.last_rotation = value,
            0x2300 => self.shared.score_ring_count = value,
            0xF16A => self.shared.enemy_path.word1 = value,
            0xF16C => self.shared.enemy_path.word2 = value,
            0x0304..=0x0312 | 0xF147..=0xF159 => {
                self.write_ext8(encoded, value as u8);
                self.write_ext8(encoded + 1, (value >> 8) as u8);
            }
            _ => panic!("untranslated imported 16-bit variable operand {encoded:#06x}"),
        }
    }

    /// ROM `vofsonplease` (WORLD.ASM:1157) — latch BG2 VOFS from `bg2scroll`,
    /// enable dovofs HDMA, set BGMODE=2.
    pub fn vofs_on_please(&mut self) {
        self.bg2vofs = self.shared.background_scroll;
        self.dovofs = 1;
        self.bgmode = 2;
    }

    /// ROM `vofsoffplease` (WORLD.ASM:1180) — clear dovofs, BGMODE=1, still
    /// latch `bg2vofs` from `bg2scroll`.
    pub fn vofs_off_please(&mut self) {
        self.dovofs = 0;
        self.bgmode = 1;
        self.bg2vofs = self.shared.background_scroll;
    }
}
