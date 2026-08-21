//! Player (Arwing) strategy: controls, movement, weapons, barrel roll,
//! damage/death, and the intro/clear sequences.
//!
//! Port (C oracle): `src/strat/strat_player.c/h` (PSTRATS.ASM +
//! PCSTRATS.ASM + PISTRATS.ASM + GISTRATS.ASM viewopening/friendstart3).
//! Function-for-function translation of the do_player_* flow:
//! playermove_srou -> gen_3dvecs -> mode velocity transform ->
//! add_vecs2pos -> playerlimitx_srou -> playerfire_srou ->
//! checkarrows_srou -> viewmove_srou.
//!
//! C `self->stratptr = fn` chaining is mirrored through the sf-game
//! strategy registry: [`install`] registers the block in a fixed order and
//! chaining then assigns `StratId(base + K_*)`. The base is derived from the
//! LIVE registry each time ([`ids_base`] finds the already-registered block by
//! its head function's pointer identity, else registers it) rather than cached
//! in WRAM — the registry is rebuilt on every `World::init` (each level load,
//! C `Strat_RegisterAll`), so a cached base would go stale and alias another
//! lane's strategies. Registration is lazy (first use installs) so no wiring
//! order is imposed on other lanes.

use crate::common::{
    boost_sprite, kill_obj, set_boost_zoff, sf_random, strat_apply_velocity, strat_chase,
    strat_chase_proportional, strat_gen_vecs_3d, strat_make_obj, strat_perc62, strat_perc75,
    strat_perc87, strat_perc93, strat_remove_obj, strat_spawn_projectile, strat_speed_to, sv,
    StratRam,
};
use crate::enemy_a::{
    add_player_z, addrnd2pos_xy, bigparticleexplode_istrat, copy_pos, fire_nuke,
    make_large_exp_obj, make_medium_exp_obj, phitflash_istrat, shiplb1_istrat, sid as ea_sid,
    strat_explode, ASF2_NOEXPSND, ASF2_RELEXPLODE, ASF2_SFLAG3, ASF4_NOPOLYEXP,
};
/// ROM `sflag4` — sflags2 bit 7 (STRATEQU.INC make_sflag after sflag3).
const ASF2_SFLAG4: u8 = 0x80;
use crate::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
use sf_core::pad;
use sf_core::player_view::{PlayerViewMode, PlayerViewOptions};
use sf_core::screen_fill_circle::ScreenFillCircleCenter;
use sf_game::alien::{
    StratId, ACF_COLLTYPE1, ACF_COLLTYPE4, ASF3_REALOBJ, ASF4_PLAYEROBJ, ASF_COLLDISABLE,
    ASF_COLLIDE, ASF_HITFLASH, ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATGND, ATLASER,
    ATZREMOVE, NUMBER_AL,
};
use sf_game::coldet::{PcboxKind, PCBOX_HF_BODY, PCBOX_HF_LWING, PCBOX_HF_RWING};
use sf_game::game::StrategyFn;
use sf_game::vars::{
    CLOSE_VIEW_DISTANCE, GF_NOZREMOVE, GF_PLAYERDEAD, GF_PLAYERDYING, GF_STAGEDONE, GF_STRATDONE1,
    GF_STRATDONE2, GF_VIEWROT, HARD_HP, OUTVIEWDIST, PFM_SHADOWS, PFM_WOBBLE,
    PLAYER_DEATH_FADE_DELAY_TICKS, PSF2_PLAYERHP0, PSF3_ENGINESND, PSF3_INTUNNEL,
    PSF3_NOCOLLISIONS, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, PSTF_NOTDIE, PSTF_NOVDISTC, SPACE_MODE,
    STAY_BLACK_INACTIVE, WATER_MODE,
};
use sf_game::Game;

// ============================================================
// Constants not yet in sf_game::vars (C src/variables.h / obj.h).
// TODO(consolidation): move to sf-game vars.rs when that lane widens.
// ============================================================
const PSF_BRKLWING: u8 = 8;
const PSF_BRKRWING: u8 = 16;
const PSF_NOYCTRL: u8 = 128;
/// ROM `psf_bodycoll` / `psf_Lwingcoll` / `psf_Rwingcoll` (GILESALC.INC).
const PSF_BODYCOLL: u8 = 1;
const PSF_LWINGCOLL: u8 = 2;
const PSF_RWINGCOLL: u8 = 4;

const PSF2_DOUBLASER: u8 = 1;
const PSF2_WIRESHIP: u8 = 2;
const PSF2_NOSPARK: u8 = 4;
const PSF2_FORCEBOOST: u8 = 16;
const PSF2_BOOSTING: u8 = 32;
const PSF2_BRAKING: u8 = 64;

const PSF3_FORCEBRAKE: u8 = 4;
const PSF3_BEAMBALL: u8 = 16;

const PSTF_NOVIEWMOVE: u8 = 4;
/// `pstf_firstframeLcol` — first-frame player laser collision (GILESALC.INC).
const PSTF_FIRSTFRAMELCOL: u8 = 16;

const PFM_DIEFALL: u8 = 1;
const PFM_DIEYROT: u8 = 2;
const PFM_WATER: u8 = 4;

const PML_LWLEFT: u8 = 1;
const PML_RWRIGHT: u8 = 2;
const PML_LWTOP: u8 = 4;
const PML_LWBOTTOM: u8 = 8;
const PML_RWTOP: u8 = 16;
const PML_RWBOTTOM: u8 = 32;
const PML_BTOP: u8 = 64;
const PML_BBOTTOM: u8 = 128;
const PML_ALL: u8 = PML_LWTOP
    | PML_RWTOP
    | PML_LWBOTTOM
    | PML_RWBOTTOM
    | PML_LWLEFT
    | PML_RWRIGHT
    | PML_BTOP
    | PML_BBOTTOM;

const MB_LEFT: u8 = 1;
const MB_RIGHT: u8 = 2;
const MB_BOTTOM: u8 = 8;
const MB_LBOTTOM: u8 = 16;
const MB_LTOP: u8 = 32;
const MB_RTOP: u8 = 64;

const SPRAR_UP: u8 = 1;
const SPRAR_DOWN: u8 = 2;
const SPRAR_LEFT: u8 = 4;
const SPRAR_RIGHT: u8 = 8;

const VIEWTYPE_NORM: u8 = 0;
const VIEWTYPE_TOOBJ: u8 = 1;
const VIEWTYPE_FPOS: u8 = 2;

const DEG180: u8 = 128;
const DEG90: u8 = 64;
const DEG45: u8 = 32;
const DEG22: u8 = 16;
const DEG11: u8 = 8;
const DEG5: u8 = 4;

const BARREL_ROLL_DELAY: u8 = 3;
const SPECIAL_DELAY_FRMS: u8 = 50;
const PLAYER_FIRESPEED: u8 = 2;

/// C ASF2_SFLAG1 / ASF2_SFLAG2 (src/game/obj.h:122-123).
/// Body-box low-shield warning latches (ROM `pcolB_strat` sflag1/sflag2).
const ASF2_SFLAG1: u8 = 0x10;
const ASF2_SFLAG2: u8 = 0x20;
/// Body impact SE when collider AP ≥ 8 (PSTRATS.ASM:182).
const SE_BODY_HIT_HARD: u8 = 0x04;
/// Body impact SE when collider AP < 8 (PSTRATS.ASM:188).
const SE_BODY_HIT_SOFT: u8 = 0x19;
/// Low-shield warning at ≤ playerB_HP/4 (PSTRATS.ASM:225).
const SE_SHIELD_WARN_QUARTER: u8 = 0x1b;
/// Critical-shield warning at ≤ playerB_HP/8 (PSTRATS.ASM:231).
const SE_SHIELD_WARN_EIGHTH: u8 = 0x1c;
/// Left / right wing destruct (SOUNDEQU se_wingdestructleft/right).
const SE_WING_DESTRUCT_LEFT: u8 = 0x05;
const SE_WING_DESTRUCT_RIGHT: u8 = 0x06;
/// Left / right wing soft hit (PSTRATS.ASM:327/473).
const SE_WING_HIT_LEFT: u8 = 0x07;
const SE_WING_HIT_RIGHT: u8 = 0x08;
/// Wire-ship wing scrape (PSTRATS.ASM:123 `pwingcol` .wirewcol).
const SE_WIRE_WING_SCRAPE: u8 = 0x14;
/// Player-down one-shot + death BGM (PSTRATS.ASM:3110/3115).
const SE_PLAYER_DOWN: u8 = 0x03;
const BGM_PLAYER_DOWN: u8 = 0x11;
const SE_PLAYER_EXPLOSION: u8 = 0x03;

/// C SHAPE_MYSHIP_4 / SHAPE_ARWING (src/renderer/shapes.h:42).
const SHAPE_MYSHIP_4: u16 = 2;
const SHAPE_ARWING: u16 = 2;
/// Invisible / nullshape stand-in (shape id 0 is skipped by Draw_BuildList).
const SHAPE_NULL: u16 = 0;
const SHAPE_LINE_SPARK: u16 = 380;

// --- Player ship shape table (GSTRATS.ASM:146-170, STRATEQU.INC:790-796) ---
/// `pshipnum_norm` — default Arwing row.
pub const PSHIPNUM_NORM: u8 = 0;
/// `pshipnum_wire` — wireframe flash / item7.
pub const PSHIPNUM_WIRE: u8 = 1;
/// `pshipnum_null` — invisible.
pub const PSHIPNUM_NULL: u8 = 2;
/// `pshipnum_cockship` — cockpit / my_up.
pub const PSHIPNUM_COCKSHIP: u8 = 3;
/// `pshipnum_tunnel` — tunnel (nullshape row).
pub const PSHIPNUM_TUNNEL: u8 = 4;
/// `pshipnum_black` — silhouette.
pub const PSHIPNUM_BLACK: u8 = 5;
/// `pshipnum_zoom` — zoom / boost silhouette.
pub const PSHIPNUM_ZOOM: u8 = 6;
/// `maxpships` — rows in `player_shapes`.
pub const MAX_PSHIPS: u8 = 7;

/// One `def_playership` row: intact, no-left, no-right, both-wings-gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerShipShapes {
    pub intact: u16,
    pub no_left: u16,
    pub no_right: u16,
    pub both: u16,
}

/// ROM `player_shapes` for the retail `hipolyarwing=0` configuration.
const PLAYER_SHAPES: [PlayerShipShapes; MAX_PSHIPS as usize] = [
    // 0 norm: myship_4, myship_r, myship_l, myship_b
    PlayerShipShapes {
        intact: SHAPE_ARWING,
        no_left: 368,  // myship_r
        no_right: 369, // myship_l
        both: 370,     // myship_b
    },
    // 1 wire: my_w, my_r_w, my_l_w, my_b_w
    PlayerShipShapes {
        intact: 351,   // my_w
        no_left: 352,  // my_r_w
        no_right: 353, // my_l_w
        both: 354,     // my_b_w
    },
    // 2 null
    PlayerShipShapes {
        intact: SHAPE_NULL,
        no_left: SHAPE_NULL,
        no_right: SHAPE_NULL,
        both: SHAPE_NULL,
    },
    // 3 cockship: my_up ×4
    PlayerShipShapes {
        intact: 371,
        no_left: 371,
        no_right: 371,
        both: 371,
    },
    // 4 tunnel: nullshape ×4
    PlayerShipShapes {
        intact: SHAPE_NULL,
        no_left: SHAPE_NULL,
        no_right: SHAPE_NULL,
        both: SHAPE_NULL,
    },
    // 5 black: Bmyship_*
    PlayerShipShapes {
        intact: 372,   // Bmyship_4
        no_left: 373,  // Bmyship_r
        no_right: 374, // Bmyship_l
        both: 375,     // Bmyship_b
    },
    // 6 zoom: myzoom_*
    PlayerShipShapes {
        intact: 376,   // myzoom_4
        no_left: 377,  // myzoom_r
        no_right: 378, // myzoom_l
        both: 379,     // myzoom_b
    },
];

/// ROM `select_ship` (GSTRATS.ASM:224-246): load `playershape{,L,R,LR}` from
/// `player_shapes[ship_num]`. Ship numbers ≥ `maxpships` clamp to 0.
pub fn select_ship(g: &mut Game, ship_num: u8) {
    let row = player_ship_row(ship_num);
    let v = &mut g.vars;
    v.set_sv_u16(sv::PLAYERSHAPE, row.intact);
    v.set_sv_u16(sv::PLAYERSHAPEL, row.no_left);
    v.set_sv_u16(sv::PLAYERSHAPER, row.no_right);
    v.set_sv_u16(sv::PLAYERSHAPELR, row.both);
}

/// ROM `setYplayershape_l` (GSTRATS.ASM:178-203): set `al_shape` from
/// `player_shapes[ship_num]` according to broken-wing `pshipflags`.
pub fn set_y_player_shape(g: &mut Game, idx: u16, ship_num: u8) {
    let row = player_ship_row(ship_num);
    let damage = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING);
    let shape = if damage == 0 {
        row.intact
    } else if damage == PSF_BRKLWING {
        row.no_left
    } else if damage == PSF_BRKRWING {
        row.no_right
    } else {
        row.both
    };
    g.objs.aliens[idx as usize].shape = shape;
}

fn player_ship_row(ship_num: u8) -> PlayerShipShapes {
    let n = if ship_num < MAX_PSHIPS {
        ship_num as usize
    } else {
        0
    };
    PLAYER_SHAPES[n]
}

/// Visible mesh for the player's fired laser/beam bolts.
///
/// The faithful ROM shape is `elaser2` (USHAPES.ASM:278, an animated needle
/// bolt spawned by `fire_Elaser`, GSTRATS.ASM:2346), but `elaser2` has no
/// `def_shape` entry in ISTRATS.ASM so `tools/shape_compiler.py` never assigns
/// it a runtime id and sf-render has no mesh for it. `strat_spawn_projectile`
/// therefore stubbed every projectile as `shape = 0` + `ASF_INVISIBLE`, which
/// `Draw_BuildList` skips on BOTH filters (shape==0 and invisible) — the reason
/// player lasers were invisible in both the C and Rust builds.
///
/// sf-render now registers the faithful `elaser2` needle-bolt mesh (the ROM
/// shape `fire_Elaser` spawns) at `SHAPE_ELASER2` (511), replacing the
/// `largeplasma` 128x128 grey quad that rendered as a giant block. The renderer
/// resolves its exact `bullet_c` animation and Pelaser fixes geometry at frame
/// 4, matching the ROM.
const SHAPE_PLAYER_LASER: u16 = 511;

// --- Rotation speeds (PSTRATS.ASM) ---
const XROT_SPEED: i16 = 0x200;
const ZROT_SPEED: i16 = 0x200;

/// `pshipflags2` bit for the 180-degree U-turn maneuver (flips the banking
/// shove direction). Mirrors `enemy_b::PSF2_TURN180`.
const PSF2_TURN180: u8 = 8;

// --- Speed constants (STRATEQU.INC:345-348) ---
const MIN_PSPEED: i16 = 20;
const MED_PSPEED: i16 = 65;
const MAX_PSPEED: i16 = 85;

// --- Table lengths (strat_player.c:35-36) ---
const PZROTFLOATTAB_LEN: u8 = 28;
const VIEWFLOATTAB_BYTELEN: u8 = 72;

/// ROM `pZrotfloattab` (PSTRATS.ASM:3635) — idle bank wobble under `pfm_wobble`.
const PZROT_FLOAT_TAB: [i8; 28] = [
    0, 1, 2, 3, 4, 4, 5, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -5, -4, -4, -3, -2, -1,
];

// --- Ltunnel fly mode constants (STRATEQU.INC) ---
const LTUNNEL_VIEWCY: i16 = -60;
const LTUNNEL_MINX: i16 = -120;
const LTUNNEL_MAXX: i16 = 120;
const LTUNNEL_MMINX: i16 = -120;
const LTUNNEL_MMAXX: i16 = 120;
const LTUNNEL_MMAXY: i16 = 60 + LTUNNEL_VIEWCY;
const LTUNNEL_MINY: i16 = -60 + LTUNNEL_VIEWCY;
const LTUNNEL_MAXY: i16 = 60 + LTUNNEL_VIEWCY;
const PLAYERB_YSTOP: i16 = -20;
const PLAYER_WING_Y_PADDING: i16 = 5;

/// Bounds + flags for one `s_playerfly_mode` tunnel/exit row (STRATEQU.INC).
#[derive(Clone, Copy)]
struct TunnelFlyMode {
    view_cy: i16,
    min_x: i16,
    max_x: i16,
    mmin_x: i16,
    mmax_x: i16,
    min_y: i16,
    max_y: i16,
    mmax_y: i16,
    /// `pfm_*` bits for this mode.
    flymode: u8,
    /// If true, clear `gf_viewrot`; if false and `viewrot_on`, set it.
    viewrot_off: bool,
    viewrot_on: bool,
    /// Tunnel macro vs exit macro.
    in_tunnel: bool,
}

const FLY_STUNNEL: TunnelFlyMode = TunnelFlyMode {
    view_cy: -60,
    min_x: -60,
    max_x: 60,
    mmin_x: -60,
    mmax_x: 60,
    min_y: -60 + -60,
    max_y: 60 + -60,
    mmax_y: 60 + -60,
    flymode: PFM_DIEFALL | PFM_SHADOWS,
    viewrot_off: true,
    viewrot_on: false,
    in_tunnel: true,
};
const FLY_MTUNNEL: TunnelFlyMode = TunnelFlyMode {
    view_cy: -60,
    min_x: -90,
    max_x: 90,
    mmin_x: -90,
    mmax_x: 90,
    min_y: -60 + -60,
    max_y: 60 + -60,
    mmax_y: 60 + -60,
    flymode: PFM_DIEFALL | PFM_SHADOWS,
    viewrot_off: true,
    viewrot_on: false,
    in_tunnel: true,
};
const FLY_LTUNNEL: TunnelFlyMode = TunnelFlyMode {
    view_cy: LTUNNEL_VIEWCY,
    min_x: LTUNNEL_MINX,
    max_x: LTUNNEL_MAXX,
    mmin_x: LTUNNEL_MMINX,
    mmax_x: LTUNNEL_MMAXX,
    min_y: LTUNNEL_MINY,
    max_y: LTUNNEL_MAXY,
    mmax_y: LTUNNEL_MMAXY,
    flymode: PFM_DIEFALL | PFM_SHADOWS,
    viewrot_off: true,
    viewrot_on: false,
    in_tunnel: true,
};
const FLY_LTEXIT: TunnelFlyMode = TunnelFlyMode {
    view_cy: -60,
    min_x: -70,
    max_x: 70,
    mmin_x: -70,
    mmax_x: 70,
    min_y: -100 + 60 + -60, // -100
    max_y: -25 + 60 + -60,  // -25
    mmax_y: -25 + 60 + -60,
    flymode: PFM_SHADOWS,
    viewrot_off: false,
    viewrot_on: true,
    in_tunnel: false,
};
const FLY_MTEXIT: TunnelFlyMode = TunnelFlyMode {
    view_cy: -60,
    min_x: -50,
    max_x: 50,
    mmin_x: -50,
    mmax_x: 50,
    min_y: -95,
    max_y: -25,
    mmax_y: -25,
    flymode: PFM_SHADOWS,
    viewrot_off: false,
    viewrot_on: true,
    in_tunnel: false,
};
const FLY_STEXIT: TunnelFlyMode = TunnelFlyMode {
    view_cy: -60,
    min_x: -35,
    max_x: 35,
    mmin_x: -35,
    mmax_x: 35,
    min_y: -95,
    max_y: -25,
    mmax_y: -25,
    flymode: PFM_SHADOWS,
    viewrot_off: false,
    viewrot_on: true,
    in_tunnel: false,
};

/// Apply `s_playerfly_mode` for a tunnel/exit row (STRATEQU.INC macros).
fn apply_tunnel_fly_mode(g: &mut Game, idx: u16, mode: TunnelFlyMode) {
    {
        let v = &mut g.vars;
        v.set_sv_i16(sv::VIEWCY, mode.view_cy);
        v.set_sv_i16(sv::MINPMOVEX, mode.min_x);
        v.set_sv_i16(sv::MAXPMOVEX, mode.max_x);
        v.set_sv_i16(sv::MINMMOVEX, mode.mmin_x);
        v.set_sv_i16(sv::MAXMMOVEX, mode.mmax_x);
        v.set_sv_i16(sv::MAXMMOVEY, mode.mmax_y);
        v.set_sv_i16(sv::MINPWMOVEY, mode.min_y);
        v.set_sv_i16(sv::MAXPWMOVEY, mode.max_y + PLAYER_WING_Y_PADDING);
        v.minpmove_y = mode.min_y;
        v.set_sv_i16(sv::MAXPMOVEY, mode.max_y + PLAYERB_YSTOP);
        v.playerflymode = mode.flymode;
        v.set_sv_u8(sv::PMOVELIMITAND, PML_ALL);
        v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_LEFT | MB_RIGHT | MB_BOTTOM);
        if mode.viewrot_on {
            v.gameflags |= GF_VIEWROT;
        }
        if mode.viewrot_off {
            v.gameflags &= !GF_VIEWROT;
        }
        v.pstratflags &= !PSTF_NOVIEWMOVE;
        v.pshipflags3 |= PSF3_ENGINESND;
        if mode.in_tunnel {
            v.pshipflags2 &= !PSF2_NOSPARK;
            v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
            v.pshipflags3 |= PSF3_INTUNNEL;
        } else {
            v.pshipflags2 |= PSF2_NOSPARK;
            v.pstratflags &= !PSTF_INSEQ;
            v.pstratflags |= PSTF_NOTDIE;
            v.pshipflags3 &= !PSF3_INTUNNEL;
        }
    }
    if mode.in_tunnel {
        g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    } else {
        g.objs.aliens[idx as usize].sflags &= !ASF_SHADOW;
    }
}

// --- Weapon offsets/constants (STRATEQU.INC) ---
const PLAYER_W_X: i16 = 33;
const PLAYER_W_Y: i16 = 13;
const PLAYER_W_X_SCALED: i16 = PLAYER_W_X >> 2;
const PLAYER_W_Y_SCALED: i16 = PLAYER_W_Y >> 2;
const INVIEW_LASER_Y_OFF: i16 = 50;
const PLAYER_BODY_HP: u8 = 40;
const PLAYER_HITFLASH_FRMS: u8 = 7;
const SCREENFLASH_BODY_FRMS: u8 = 4;
const SCREENFLASH_BODY_TYPE: u8 = 0;
/// Wing screenflash (STRATEQU.INC:775-776 screenflashwingfrms/type).
const SCREENFLASH_WING_FRMS: u8 = 2;
const SCREENFLASH_WING_TYPE: u8 = 1;
const PLAYER_DEAD_FRAMES: u8 = 60;
const PLAYER_DEATH_FLASH_FRAMES: u8 = 15;
const PLAYER_DEATH_ROLL_ACCELERATION: u8 = 4;
const PLAYER_DEATH_PITCH_TARGET: i16 = 5000;
const PLAYER_DEATH_GROUND_PITCH: i16 = -2000;
const SHIPINTRO_LIFE: u8 = 40;
const SHIPINTRO_BOOST_Z: i16 = 50;
/// Effective straight-ahead distance produced by the retail fixed-point
/// vector path during the two transfer-bound base-player updates.
const LEVEL_INITIALIZATION_FORWARD_STEP: i16 = 63;

// --- ExitBase constants (STRATEQU.INC:305,350,581-597) ---
const PEXITBASE_SPEED: i16 = 50;
const MYBASE_SCALE: i16 = 3;
const EXITBASE_VIEWCY: i16 = -50;
const EXITBASE_MINX: i16 = -500;
const EXITBASE_MAXX: i16 = 500;
const EXITBASE_MMINX: i16 = -500;
const EXITBASE_MMAXX: i16 = 500;
const EXITBASE_MINY: i16 = -600;
const EXITBASE_MAXY: i16 = 0;
const EXITBASE_MMAXY: i16 = 0;

// --- Float tables (strat_player.c:64-76, PISTRATS.ASM) ---
const SHIPINTRO_ROTZ_FLOAT: [i8; 28] = [
    0, 1, 2, 3, 4, 4, 5, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -5, -4, -4, -3, -2, -1,
];

const SHIPINTRO_VIEW_FLOAT: [i16; 36] = [
    0, 1, 2, 3, 4, 4, 5, 5, 6, 6, 6, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -6, -6,
    -6, -5, -5, -4, -4, -3, -2, -1,
];

// ============================================================
// Registry block (C function pointers -> StratId chaining)
// ============================================================

const K_PLAYER: u16 = 0;
const K_PLAYERCOLL: u16 = 1;
const K_PLAYERDEAD_INIT: u16 = 2;
const K_PLAYERDEAD_STRAT: u16 = 3;
const K_SHIPINTRO_INIT: u16 = 4;
const K_SHIPINTRO_STRAT: u16 = 5;
const K_EXITBASE_WAIT: u16 = 6;
const K_EXITBASE_GO: u16 = 7;
const K_EXITBASE_FOLLOW: u16 = 8;
const K_FRIENDSTART3GO: u16 = 9;
const K_CLEARBRIDGE_INIT: u16 = 10;
const K_CLEARBRIDGE_STRAT: u16 = 11;
const K_CLEARBRIDGE3_STRAT: u16 = 12;
const K_ESCNUCLEUS_INIT: u16 = 13;
const K_ESCNUCLEUS_STRAT: u16 = 14;
const K_ESCNUCLEUS2_START: u16 = 15;
const K_OPENING_INIT: u16 = 16;
const K_PLAYERPENING_STRAT: u16 = 17;
const K_OPENINGBOOST_STRAT: u16 = 18;
const K_VIEWOPENING_STRAT: u16 = 19;
// C `Strat_PlayerExitBase` (set_playerExitBase_l): the hangar-launch init that
// re-parks the player at worldz=-200 and hides it. Invoked from the map VM's
// SET_PLAYER_EXITBASE_L callback (registered at STRAT_ADDR_PLAYER_EXITBASE).
const K_EXITBASE_INIT: u16 = 20;
// pcbox proxy strats (ROM pBody_strat / pLWing_strat|pRWing_strat / the
// pcolB_strat family, PSTRATS.ASM). Body/wing re-park each frame; coll routes
// a box hit back onto the ship.
const K_PCBOX_BODY: u16 = 21;
const K_PCBOX_WING: u16 = 22;
const K_PCBOX_COLL: u16 = 23;
/// ROM `lspark_Istrat` / `lspark_strat` (PSTRATS.ASM:54) — wing scrape spark.
const K_LSPARK_INIT: u16 = 24;
const K_LSPARK_STRAT: u16 = 25;
/// ROM `slspark_Istrat` / `slspark_strat` (PSTRATS.ASM:43) — boss2 spark variant
/// (same motion as lspark; colanim step +1 instead of +2).
const K_SLSPARK_INIT: u16 = 26;
const K_SLSPARK_STRAT: u16 = 27;
/// ROM `shrapfall2_Istrat` (PCSTRATS.ASM) — LB1 debris scrap.
const K_SHRAPFALL2: u16 = 28;

const FNS: [StrategyFn; 29] = [
    strat_player,
    playercoll_istrat,
    playerdead_istrat,
    playerdead_strat,
    strat_ship_intro_init,
    shipintro_strat,
    player_exitbase_wait_strat,
    player_exitbase_go_strat,
    player_exitbase_follow_strat,
    friendstart3go_strat,
    strat_player_clear_bridge_init,
    playerclearbridge_strat,
    playerclearbridge3_strat,
    strat_player_escape_nucleus_init,
    player_escape_nucleus_strat,
    player_escape_nucleus2_start,
    strat_player_opening_init,
    playerpening_strat,
    playeropeningboost_strat,
    viewopening_strat,
    strat_player_exit_base,
    pcbox_body_strat,
    pcbox_wing_strat,
    pcbox_coll_strat,
    lspark_istrat,
    lspark_strat,
    slspark_istrat,
    slspark_strat,
    shrapfall2_istrat,
];

fn ids_base(g: &mut Game) -> u16 {
    // The strat registry (`g.world.strat_registry`) is rebuilt on every
    // `World::init` — i.e. every level load (C `Strat_RegisterAll` runs each
    // load, boot.c:85). A base cached in WRAM survives that reset and goes
    // stale: after begin_gameplay's `World::init` the fresh registry no longer
    // holds the player block, so a cached base would alias whatever lane
    // (enemy_a) registered at that index — the player would run e.g.
    // strat_boss1_init. Derive the base from the LIVE registry instead,
    // finding the already-registered FNS block by its head function's pointer
    // identity (matching `enemy_a::sid`), and register the block on first use.
    if let Some(pos) = g
        .world
        .strat_registry
        .iter()
        .position(|&r| r as usize == FNS[0] as usize)
    {
        return pos as u16;
    }
    let base = g.world.strat_registry.len() as u16;
    for f in FNS {
        g.world.register_strategy(f);
    }
    base
}

fn sid(g: &mut Game, k: u16) -> StratId {
    StratId(ids_base(g) + k)
}

/// Registry handles the table lane wires into istrat indices.
pub struct PlayerStratIds {
    /// `Strat_Player` (playeronplanet_strat family entry).
    pub player: StratId,
    /// `playercoll_Istrat`.
    pub player_coll: StratId,
    /// `playerdead_Istrat`.
    pub player_dead: StratId,
    /// `shipintro_Istrat` (`Strat_ShipIntro_Init`).
    pub ship_intro_init: StratId,
    /// `playerclearbridge_Istrat`.
    pub clear_bridge_init: StratId,
    /// `playerEscapeNucleus_Istrat`.
    pub escape_nucleus_init: StratId,
    /// `playeropening_Istrat`.
    pub opening_init: StratId,
    /// `Strat_PlayerExitBase` (set_playerExitBase_l hangar-launch init).
    pub exit_base_init: StratId,
    /// pcbox body/wing/coll proxy strats (ROM pBody/pLWing|pRWing/pcolB), used
    /// by the game-core per-level player-collision setup (`pcbox_attach`).
    pub pcbox_body: StratId,
    pub pcbox_wing: StratId,
    pub pcbox_coll: StratId,
}

/// Register the player strategy block (idempotent) and return the public
/// entry handles.
pub fn install(g: &mut Game) -> PlayerStratIds {
    let base = ids_base(g);
    PlayerStratIds {
        player: StratId(base + K_PLAYER),
        player_coll: StratId(base + K_PLAYERCOLL),
        player_dead: StratId(base + K_PLAYERDEAD_INIT),
        ship_intro_init: StratId(base + K_SHIPINTRO_INIT),
        clear_bridge_init: StratId(base + K_CLEARBRIDGE_INIT),
        escape_nucleus_init: StratId(base + K_ESCNUCLEUS_INIT),
        opening_init: StratId(base + K_OPENING_INIT),
        exit_base_init: StratId(base + K_EXITBASE_INIT),
        pcbox_body: StratId(base + K_PCBOX_BODY),
        pcbox_wing: StratId(base + K_PCBOX_WING),
        pcbox_coll: StratId(base + K_PCBOX_COLL),
    }
}

// ============================================================
// Pad helpers (C g_pad1 / g_pad1_prev / g_pad1_new, src/sf_rtl.c)
// ============================================================

fn pad1(g: &Game) -> u16 {
    g.vars.pad1
}

/// Previous frame's pad — the Game::tick lastcont latch (TRANS.ASM
/// lastcont0/lastcontl0), which the C runtime mirrors as `g_pad1_prev`.
fn pad1_prev(g: &Game) -> u16 {
    ((g.vars.lastcont0 as u16) << 8) | g.vars.lastcontl0 as u16
}

/// C `g_pad1_new` = pad1 & ~pad1_prev (src/sf_rtl.c:145).
fn pad1_new(g: &Game) -> u16 {
    g.vars.pad1 & !pad1_prev(g)
}

fn clamp16(val: i16, lo: i16, hi: i16) -> i16 {
    if val < lo {
        lo
    } else if val > hi {
        hi
    } else {
        val
    }
}

// ============================================================
// Ship intro flyby (MAP1_1A shipintro_Istrat, strat_player.c:93-156)
// ============================================================

/// C `shipintro_dec_lifecnt` (strat_player.c:93).
fn shipintro_dec_lifecnt(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.count = al.count.wrapping_sub(1);
    if al.count == 0 {
        strat_remove_obj(g);
    }
}

/// C `shipintro_float` (strat_player.c:100).
fn shipintro_float(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte3 = al.sbyte3.wrapping_add(1);
    if al.sbyte3 >= SHIPINTRO_ROTZ_FLOAT.len() as u8 {
        al.sbyte3 = 0;
    }
    al.rotz = SHIPINTRO_ROTZ_FLOAT[al.sbyte3 as usize] as u8;

    al.sbyte4 = al.sbyte4.wrapping_add(1);
    if al.sbyte4 >= SHIPINTRO_VIEW_FLOAT.len() as u8 {
        al.sbyte4 = 0;
    }
    // `floatCLship_l` reads the word table with an explicit source scale of
    // one, doubling the authored float before the ship-specific offset.
    al.worldy = SHIPINTRO_VIEW_FLOAT[al.sbyte4 as usize].wrapping_mul(2);
}

/// C `shipintro_strat` (strat_player.c:114).
fn shipintro_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    if g.vars.gameflags & GF_STRATDONE2 != 0 {
        let sbyte1 = g.objs.aliens[i].sbyte1;
        if (sbyte1 as i8) < 0 {
            shipintro_dec_lifecnt(g, idx);
        } else {
            let sbyte1 = sbyte1.wrapping_sub(1);
            g.objs.aliens[i].sbyte1 = sbyte1;
            if sbyte1 == 0 {
                g.objs.aliens[i].sbyte1 = 1;
                g.objs.aliens[i].worldz = g.objs.aliens[i].worldz.wrapping_add(SHIPINTRO_BOOST_Z);
                shipintro_dec_lifecnt(g, idx);
            }
        }
    }

    if g.objs.aliens[i].sbyte1 == 2 {
        // C player_obj_index_or_null(self) — the alien's own slot index.
        g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
        if g.vars.sv_u8(sv::BOOSTZOFF) == 0 {
            set_boost_zoff(g, -30);
        }
        let _ = boost_sprite(g, None);
        g.hooks.play_se(0x32);
    }

    shipintro_float(g, idx);
    let al = &mut g.objs.aliens[i];
    al.worldy = al.worldy.wrapping_add(al.sword1);
    al.worldz = al.worldz.wrapping_add(MED_PSPEED);
}

/// C `Strat_ShipIntro_Init` (strat_player.c:142).
pub fn strat_ship_intro_init(g: &mut Game, idx: u16) {
    let strat = sid(g, K_SHIPINTRO_STRAT);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(strat);
        al.sbyte3 = random_yaw;
        al.sbyte4 = random_pitch;
        al.sbyte2 <<= 1;
        al.sflags |= ASF_SHADOW;
        al.count = SHIPINTRO_LIFE;
        al.type_ &= !ATZREMOVE;
    }
    g.vars.gameflags &= !GF_STRATDONE2;
    shipintro_strat(g, idx);
}

// ============================================================
// Damage / death (strat_player.c:158-260)
// ============================================================

/// C `player_hitflash_update` (strat_player.c:158).
fn player_hitflash_update(g: &mut Game, idx: u16) {
    let i = idx as usize;
    if g.objs.aliens[i].sbyte1 == 0 {
        g.objs.aliens[i].sflags &= !ASF_NOHITAFFECT;
        g.vars.set_sv_u8(sv::VIEWSHAKEX, 0);
        g.vars.set_sv_u8(sv::VIEWSHAKEY, 0);
        g.vars.set_sv_u8(sv::VIEWSHAKEZ, 0);
        return;
    }

    g.objs.aliens[i].sbyte1 -= 1;

    // s_set_var2rnd viewshake{X,Y,Z},#15 / -7
    let rx = ((sf_random(&mut g.vars) & 0x0F) as i32 - 7) as u8;
    let ry = ((sf_random(&mut g.vars) & 0x0F) as i32 - 7) as u8;
    let rz = ((sf_random(&mut g.vars) & 0x0F) as i32 - 7) as u8;
    g.vars.set_sv_u8(sv::VIEWSHAKEX, rx);
    g.vars.set_sv_u8(sv::VIEWSHAKEY, ry);
    g.vars.set_sv_u8(sv::VIEWSHAKEZ, rz);

    // Toggle hitflash every other frame while invulnerable.
    if g.vars.gameframe & 1 == 0 {
        g.objs.aliens[i].sflags |= ASF_HITFLASH;
        g.objs.aliens[i].sflags |= ASF_NOHITAFFECT;
    }
}

/// C `playercoll_istrat` (strat_player.c:181).
fn playercoll_istrat(g: &mut Game, idx: u16) {
    let i = idx as usize;
    if g.vars.pshipflags3 & PSF3_NOCOLLISIONS != 0 {
        g.objs.aliens[i].hitflags = 0;
        g.objs.aliens[i].sflags &= !ASF_COLLIDE;
        // PSTRATS.ASM `.ncoll` ends with `s_jmpto_strat`: collision handling
        // is a prefix to the normal player strategy, not a replacement for
        // the frame.  Returning here can pin the ship inside a tunnel wall:
        // the player never advances, so the overlap is recreated forever.
        if let Some(strat) = g.objs.aliens[i].stratptr {
            g.call_strat(strat, idx);
        }
        return;
    }

    let hits = g.vars.sv_u8(sv::PNUMHITS);
    if hits < 0xFF {
        g.vars.set_sv_u8(sv::PNUMHITS, hits + 1);
    }

    // Shield/wire ship collisions only proceed against hard-HP geometry.
    // PSTRATS.ASM:3286-3293 also plays the scrape sound before the gate.
    let partner = g.objs.aliens[i].collobjptr;
    if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
        g.hooks.play_se(SE_WIRE_WING_SCRAPE);
        if partner as usize >= g.objs.aliens.len()
            || !g.objs.aliens[partner as usize].active
            || g.objs.aliens[partner as usize].hp != HARD_HP
        {
            g.objs.aliens[i].hitflags = 0;
            g.objs.aliens[i].sflags &= !ASF_COLLIDE;
            if let Some(strat) = g.objs.aliens[i].stratptr {
                g.call_strat(strat, idx);
            }
            return;
        }
    }

    if g.objs.aliens[i].sflags & ASF_NOHITAFFECT == 0 {
        // Barrel-roll laser deflection (PSTRATS.ASM:3297-3328).
        if g.vars.sv_u8(sv::PLAYER_ROLLZVEL) != 0
            && (partner as usize) < g.objs.aliens.len()
            && g.objs.aliens[partner as usize].active
            && g.objs.aliens[partner as usize].type_ & ATLASER != 0
            && g.objs.aliens[partner as usize].hp != HARD_HP
        {
            let rotx = ((sf_random(&mut g.vars) & 63) as u8).wrapping_sub(31);
            let mut roty = (sf_random(&mut g.vars) & 31) as u8;
            if sf_random(&mut g.vars) & 1 == 0 {
                roty = roty.wrapping_sub(24); // deg90 - 40
            } else {
                roty = roty.wrapping_add(24);
            }
            {
                let shot = &mut g.objs.aliens[partner as usize];
                shot.rotx = rotx;
                shot.roty = roty;
                shot.vel = 60;
                strat_gen_vecs_3d(shot);
                for _ in 0..4 {
                    strat_apply_velocity(shot);
                }
                shot.sflags &= !ASF_COLLIDE;
                shot.sflags |= ASF_COLLDISABLE;
                shot.count = 30;
            }
            crate::enemy_a::relflatmiss_istrat(g, partner);
            g.hooks.play_se(SE_WIRE_WING_SCRAPE);
        } else if g.coldet.pcbox.player == Some(idx) {
            // Exact playerB_col routing: collision detection writes HF1/HF2/HF3
            // on the ship; these colldisable proxies receive the collision and
            // run their own pcol strategies later in the same strategy pass.
            let flags = g.objs.aliens[i].hitflags;
            for (mask, slot) in [
                (PCBOX_HF_BODY, g.coldet.pcbox.body),
                (PCBOX_HF_LWING, g.coldet.pcbox.lwing),
                (PCBOX_HF_RWING, g.coldet.pcbox.rwing),
            ] {
                if flags & mask != 0 {
                    if let Some(slot) = slot {
                        let proxy = &mut g.objs.aliens[slot as usize];
                        proxy.sflags |= ASF_COLLIDE | ASF_HITFLASH;
                        proxy.collobjptr = partner;
                    }
                }
            }
        } else {
            // Compatibility fallback for sf-game-only/headless callers that
            // spawn a player without the per-level MAPP pcbox objects.
            g.objs.aliens[i].sbyte1 = PLAYER_HITFLASH_FRMS;
            g.objs.aliens[i].sflags |= ASF_HITFLASH | ASF_NOHITAFFECT;
            g.vars.set_sv_u8(sv::SCREENFLASHCNT, SCREENFLASH_BODY_FRMS);
            g.vars.set_sv_u8(sv::SCREENFLASHTYPE, SCREENFLASH_BODY_TYPE);
            if g.objs.aliens[i].hp == 0 {
                playerdead_istrat(g, idx);
                return;
            }
        }
    }

    g.objs.aliens[idx as usize].hitflags = 0;
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    if let Some(strat) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(strat, idx);
    }
}

/// ROM `pexplode_Istrat` — replace the crashing ship with its terminal
/// explosion and hand the typed death state to the shell.
fn player_explode_istrat(g: &mut Game, idx: u16) {
    if g.vars.gameflags & GF_PLAYERDEAD != 0 {
        return;
    }

    let ship = g.objs.aliens[idx as usize];
    if let Some(anchor) = strat_make_obj(g, 0) {
        {
            let object = &mut g.objs.aliens[anchor as usize];
            object.sflags3 &= !ASF3_REALOBJ;
            object.sflags4 |= ASF4_PLAYEROBJ;
            object.worldx = ship.worldx.wrapping_add(ship.vx);
            object.worldy = ship.worldy.wrapping_add(ship.vy);
            object.worldz = ship.worldz.wrapping_add(ship.vz);
            object.vx = ship.vx;
            object.vy = ship.vy;
            object.vz = ship.vz;
            object.stratptr = None;
        }

        // The source writes viewpt, playpt, viewtoobj, and internalPLAYPT to
        // this same object. The flat port keeps one authoritative player
        // object plus the independently named camera/circle targets.
        g.vars.internal_playpt = anchor as i16;
        g.vars.strategy.view_target_object = anchor as i16;
        g.vars.strategy.circle_object = anchor as i16;
        g.vars.screen_fill_circle.begin_red(
            sf_core::screen_fill_circle::ScreenFillCircleCenter::Object(anchor + 1),
        );
    }

    let explode = ea_sid(g, strat_explode);
    {
        let player = &mut g.objs.aliens[idx as usize];
        player.sflags &= !ASF_SHADOW;
        player.vx = 0;
        player.vy = 0;
        player.vz = 0;
        player.stratptr = Some(explode);
        player.collstratptr = Some(explode);
        player.expstratptr = Some(explode);
        kill_obj(player);
    }

    if let Some(particle) = strat_make_obj(g, 0) {
        let particle_init = ea_sid(g, bigparticleexplode_istrat);
        let object = &mut g.objs.aliens[particle as usize];
        object.worldx = ship.worldx;
        object.worldy = ship.worldy;
        object.worldz = ship.worldz;
        object.stratptr = Some(particle_init);
    }

    g.hooks.play_se(SE_PLAYER_EXPLOSION);
    g.vars.gameflags |= GF_PLAYERDEAD;
    let lives = g.vars.sv_u8(sv::LIVES);
    g.vars.set_sv_u8(sv::LIVES, lives.wrapping_sub(1));
    g.vars.player_death_fade_delay = PLAYER_DEATH_FADE_DELAY_TICKS;
}

/// C `playerdead_strat` (PSTRATS.ASM:3287-3370) — the dying crash sequence:
/// hitflash blink, nose-dive + roll spin while above the ground, ground slam
/// at worldy 0, forward speed decay, ship keeps MOVING via gen_3dvecs +
/// add_vecs2pos, camera z pinned to the ship. At 60 frames -> pexplode
/// (ship becomes the explosion, GF_PLAYERDEAD for the shell respawn flow).
/// The old port froze the ship motionless for 3s then hard-reloaded, which
/// read as "collisions just stop the arwing completely".
fn playerdead_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;
    if g.vars.gameflags & GF_PLAYERDEAD != 0 {
        return;
    }

    // s_copy_var2var W,bgsscrollZ,viewposz (PSTRATS.ASM — last frame's
    // getview camera Z; GameCamera::update writes WRAM $0554 each tick).
    let vpz = g.vars.sv_i16(sv::VIEWPOSZ);
    g.vars.set_sv_i16(sv::BGSSCROLLZ, vpz);

    // s_add_alvar B,x,al_sbyte1,#1 ; == 10*6 -> pexplode_Istrat
    if g.objs.aliens[i].sbyte1 < u8::MAX {
        g.objs.aliens[i].sbyte1 += 1;
    }
    if g.objs.aliens[i].sbyte1 >= PLAYER_DEAD_FRAMES {
        player_explode_istrat(g, idx);
        return;
    }

    // sbyte1 < 15: blink hitflash every other frame (PSTRATS:3297-3301).
    if g.objs.aliens[i].sbyte1 < PLAYER_DEATH_FLASH_FRAMES && g.vars.gameframe & 1 == 0 {
        g.objs.aliens[i].sflags |= ASF_HITFLASH;
    }

    // Die-fall (pfm_diefall): nose over + roll spin while airborne; slam flat
    // at the ground plane (PSTRATS:3321-3341). SNES +y is down; ground = 0.
    if g.vars.playerflymode & PFM_DIEFALL != 0 {
        if g.objs.aliens[i].worldy < 0 {
            // s_jmp_NOTdelay 1 / s_Achase_var W,plrotx,#5000,4 — nose down.
            if g.vars.gameframe & 1 == 0 {
                let rx = strat_chase_proportional(
                    g.vars.sv_i16(sv::PLROTX),
                    PLAYER_DEATH_PITCH_TARGET,
                    4,
                );
                g.vars.set_sv_i16(sv::PLROTX, rx);
            }
            // s_add_var B,player_Zstratadd,#4 — accelerating roll spin.
            let zadd = g
                .vars
                .sv_u8(sv::PLAYER_ZSTRATADD)
                .wrapping_add(PLAYER_DEATH_ROLL_ACCELERATION);
            g.vars.set_sv_u8(sv::PLAYER_ZSTRATADD, zadd);
        } else {
            // Ground slam: plrotx=-2000, pin to the deck (sparks in ROM).
            g.vars.set_sv_i16(sv::PLROTX, PLAYER_DEATH_GROUND_PITCH);
            g.objs.aliens[i].worldy = 0;
        }
    }

    // Every 4th frame: forward speed decays toward 0 (s_Fchase_var rate 1).
    let mut speed = g.vars.sv_i16(sv::PLAYER_SPEED);
    if g.vars.gameframe & 3 == 0 && speed > 0 {
        speed = strat_chase_proportional(speed, 0, 1);
        g.vars.set_sv_i16(sv::PLAYER_SPEED, speed);
    }

    // Rots from the pl* accumulators + the spin; then move the ship.
    {
        let rotx = (g.vars.sv_i16(sv::PLROTX) >> 8) as u8;
        let roty = (g.vars.sv_i16(sv::PLROTY) >> 8) as u8;
        let rotz = (g.vars.sv_i16(sv::PLROTZ) >> 8) as u8;
        let zadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD);
        let al = &mut g.objs.aliens[i];
        al.rotx = rotx;
        al.roty = roty;
        al.rotz = rotz.wrapping_add(zadd);
        al.vel = clamp16(speed, 0, MAX_PSPEED) as u8;
        strat_gen_vecs_3d(al);
        al.vx = 0; // s_set_alvar W,x,al_vx,#0
        strat_apply_velocity(al);
    }

    // Camera: pviewposz follows the crashing ship; Y eases after it in
    // dieYrot mode (s_achase_var2alvar W,x,pviewposY,al_worldy,3).
    let (wy, wz) = {
        let al = &g.objs.aliens[i];
        (al.worldy, al.worldz)
    };
    g.vars.set_sv_i16(sv::PVIEWPOSZ, wz);
    if g.vars.playerflymode & PFM_DIEYROT != 0 {
        let py = strat_chase_proportional(g.vars.sv_i16(sv::PVIEWPOSY), wy, 3);
        g.vars.set_sv_i16(sv::PVIEWPOSY, py);
    }
}

/// C `playerdead_istrat` (strat_player.c:237).
fn playerdead_istrat(g: &mut Game, idx: u16) {
    if g.vars.gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0
        || (g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0
            && g.vars.player_view_mode == PlayerViewMode::LeavingCockpit)
    {
        // Keep the player on the installed transition/crash callback if an
        // overlapping object tries to route through the death initializer
        // again.
        if g.objs.aliens[idx as usize].hp == 0 {
            g.objs.aliens[idx as usize].hp = 10;
        }
        return;
    }

    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    g.vars.gameflags &= !GF_PLAYERDEAD;

    // PSTRATS.ASM:3031-3045 disables and detaches the three HP proxies.
    g.pcbox_detach();

    {
        let player = &mut g.objs.aliens[idx as usize];
        player.hp = 10;
        player.ap = 0;
        player.sflags &= !ASF_COLLIDE;
        player.collflags = (player.collflags
            & !(sf_game::alien::ACF_COLLTYPE1
                | sf_game::alien::ACF_COLLTYPE2
                | sf_game::alien::ACF_COLLTYPE3
                | sf_game::alien::ACF_COLLTYPE4
                | sf_game::alien::ACF_COLLTYPE5))
            | ACF_COLLTYPE4;
    }

    let first_entry = g.vars.pshipflags2 & PSF2_PLAYERHP0 == 0;
    if first_entry {
        g.vars.pshipflags2 |= PSF2_PLAYERHP0;
        g.hooks.play_se(SE_PLAYER_DOWN);
        g.hooks.play_music(BGM_PLAYER_DOWN);
    }

    // `spfm_inside` does not begin the crash in place. It first installs the
    // authored cockpit ejection and returns; that transition re-enters this
    // initializer after its final frame.
    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
        g.vars.player_view_mode = PlayerViewMode::LeavingCockpit;
        set_player_out_of_cock(g, idx);
        return;
    }

    let dead = sid(g, K_PLAYERDEAD_STRAT);
    let coll = ea_sid(g, phitflash_istrat);
    let exp = ea_sid(g, player_explode_istrat);
    {
        let player = &mut g.objs.aliens[idx as usize];
        player.sbyte1 = 0;
        player.sflags &= !ASF_COLLDISABLE;
        player.stratptr = Some(dead);
        player.collstratptr = Some(coll);
        player.expstratptr = Some(exp);
    }
    g.vars.gameflags |= GF_PLAYERDYING;

    // The initializer falls through into `playerdead_strat` in the source.
    playerdead_strat(g, idx);
}

// ============================================================
// pcbox proxy boxes (PSTRATS.ASM pBody/pLWing/pRWing + pcolB family)
// ============================================================

/// Public entry: build the three player collision-proxy boxes and route the
/// ship's body collisions through them (ROM per-level player setup,
/// GSTRATS.ASM:100-125 -> pBody_Istrat/pLWing_Istrat/pRWing_Istrat).
///
/// `player` is the ship slot. Registers the box strats (idempotent) and hands
/// their [`StratId`]s to the game-core [`Game::pcbox_attach`], which allocates
/// the colldisable HP proxies and leaves the ship's multi-box collider live.
/// The real shell invokes the registry-backed equivalent at every level start.
pub fn pcbox_attach(g: &mut Game, player: u16) -> bool {
    let body = sid(g, K_PCBOX_BODY);
    let wing = sid(g, K_PCBOX_WING);
    let coll = sid(g, K_PCBOX_COLL);
    g.pcbox_attach(player, body, wing, coll)
}

/// Rotate a wing offset `(ox, oy)` about the ship's Z axis by `rotz`
/// (ROM `s_add_Roffs2pos ...,0,0,1` — rotz on, rotx/roty off,
/// PSTRATS.ASM:283/419). Z has offset 0 (playerW_z), so only X/Y rotate.
fn rotz_offset(ox: i16, oy: i16, rotz: u8) -> (i16, i16) {
    let (x, y, _) = crate::snes_trig::strat_roffs_roll(rotz, ox as i8, oy as i8, 0);
    (x, y)
}

/// ROM `s_gen_flatvecs` (STRATMAC.INC:3666) via `nvecs_l`: XZ speed from a
/// negated angle (+1 table quirk), written into `vx`/`vy` (Z unused).
fn gen_flatvecs(rotz: u8, vel: u8) -> (i16, i16) {
    let angle = rotz.wrapping_neg().wrapping_add(1) as usize;
    let v = vel as i8;
    let vx = mulslog_mac8(v, SINTAB[angle]) as i16;
    let vy = mulslog_mac8(v, COSTAB[angle]) as i16;
    (vx, vy)
}

/// ROM `sgenspark_srou` (PSTRATS.ASM:72): spawn a short-lived scrape spark at
/// `at`'s position unless `psf2_nospark`.
pub fn sgen_spark(g: &mut Game, at: u16) {
    if g.vars.pshipflags2 & PSF2_NOSPARK != 0 {
        return;
    }
    let Some(spark) = strat_make_obj(g, SHAPE_LINE_SPARK) else {
        return;
    };
    let src = g.objs.aliens[at as usize];
    let rotz = (sf_random(&mut g.vars) & 0xFF) as u8;
    let (vx, vy) = gen_flatvecs(rotz, 15);
    let outvx = g.vars.sv_i16(sv::OUTVX);
    let outvy = g.vars.sv_i16(sv::OUTVY);
    let turn = (g.vars.sv_i16(sv::PLAYER_TURNROT) >> 8) as u8;
    let strat = sid(g, K_LSPARK_INIT);
    {
        let al = &mut g.objs.aliens[spark as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotz = rotz;
        al.vel = 15;
        al.count = 5; // s_set_lifecnt #5
        al.vx = vx;
        al.vy = vy;
        al.vz = 0;
        al.sflags |= ASF_COLLDISABLE;
        // s_rots_flat: billboard to camera (outvy+1 negated + 180 + turn).
        al.roty = (outvy >> 8) as u8;
        al.roty = al.roty.wrapping_neg().wrapping_add(128).wrapping_add(turn);
        al.rotx = (outvx >> 8) as u8;
        al.stratptr = Some(strat);
    }
}

/// ROM `lspark_Istrat` (PSTRATS.ASM:54).
pub fn lspark_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratptr = Some(sid(g, K_LSPARK_STRAT));
}

/// ROM `lspark_strat` / `lspark_cont` (PSTRATS.ASM:59): scroll with player Z,
/// integrate velocity, then expire.
pub fn lspark_strat(g: &mut Game, idx: u16) {
    // s_add_colanim x,#2,#16 — cosmetic colour cycle; HD path ignores.
    lspark_cont(g, idx);
}

/// Shared `lspark_cont` body (PSTRATS.ASM:62).
pub fn lspark_cont(g: &mut Game, idx: u16) {
    let pz = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(pz);
    al.worldx = al.worldx.wrapping_add(al.vx);
    al.worldy = al.worldy.wrapping_add(al.vy);
    al.worldz = al.worldz.wrapping_add(al.vz);
    if al.count > 0 {
        al.count = al.count.wrapping_sub(1);
    }
    if al.count == 0 {
        strat_remove_obj(g);
    }
}

/// ROM `slspark_Istrat` (PSTRATS.ASM:43).
fn slspark_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratptr = Some(sid(g, K_SLSPARK_STRAT));
}

/// ROM `slspark_strat` (PSTRATS.ASM:48): colanim +1 then same cont as lspark.
fn slspark_strat(g: &mut Game, idx: u16) {
    lspark_cont(g, idx);
}

/// ROM `boss2spark_srou` body (GBSTRATS.ASM:1028) — empty in the source, but the
/// intended spawn used `slspark_Istrat` + speed 20. Exposed for boss2 wiring.
pub fn sgen_slspark(g: &mut Game, at: u16) {
    let Some(spark) = strat_make_obj(g, SHAPE_LINE_SPARK) else {
        return;
    };
    let src = g.objs.aliens[at as usize];
    let rotz = (sf_random(&mut g.vars) & 0xFF) as u8;
    let (vx, vy) = gen_flatvecs(rotz, 20);
    let outvx = g.vars.sv_i16(sv::OUTVX);
    let outvy = g.vars.sv_i16(sv::OUTVY);
    let turn = (g.vars.sv_i16(sv::PLAYER_TURNROT) >> 8) as u8;
    let strat = sid(g, K_SLSPARK_INIT);
    {
        let al = &mut g.objs.aliens[spark as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotz = rotz;
        al.vel = 20;
        al.count = 5;
        al.vx = vx;
        al.vy = vy;
        al.vz = 0;
        al.sflags |= ASF_COLLDISABLE;
        al.roty = (outvy >> 8) as u8;
        al.roty = al.roty.wrapping_neg().wrapping_add(128).wrapping_add(turn);
        al.rotx = (outvx >> 8) as u8;
        al.stratptr = Some(strat);
    }
}

/// Re-park the body box on the ship every frame (ROM `pBody_strat`,
/// PSTRATS.ASM:151-168): copy the ship's position and rotations (offset 0).
fn pcbox_body_strat(g: &mut Game, idx: u16) {
    let Some(player) = g.coldet.pcbox.player else {
        return;
    };
    let p = g.objs.aliens[player as usize];
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = p.worldx;
    al.worldy = p.worldy;
    al.worldz = p.worldz;
    al.rotx = p.rotx;
    al.roty = p.roty;
    al.rotz = p.rotz;
}

/// Re-park a wing box on the ship every frame (ROM `pLWing_strat`/
/// `pRWing_strat`, PSTRATS.ASM:270-310/415-...): ship position + the rotz-
/// rotated wing offset. Left wing uses -X, right wing +X.
fn pcbox_wing_strat(g: &mut Game, idx: u16) {
    let Some(player) = g.coldet.pcbox.player else {
        return;
    };
    let kind = g.coldet.pcbox.kind_of(idx);
    let p = g.objs.aliens[player as usize];
    let off_x = match kind {
        Some(PcboxKind::LWing) => -sf_game::coldet::PCBOX_WING_X,
        _ => sf_game::coldet::PCBOX_WING_X,
    };
    let (dx, dy) = rotz_offset(off_x, sf_game::coldet::PCBOX_WING_Y, p.rotz);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = p.worldx.wrapping_add(dx);
    al.worldy = p.worldy.wrapping_add(dy);
    al.worldz = p.worldz.wrapping_add(sf_game::coldet::PCBOX_WING_Z);
    al.rotx = p.rotx;
    al.roty = p.roty;
    al.rotz = p.rotz;
}

/// Refresh all attached player damage proxies from the ship's current typed
/// transform. The source performs these follower updates after player motion
/// on each startup strategy pass.
pub fn refresh_player_collision_proxies(g: &mut Game) {
    if let Some(body) = g.coldet.pcbox.body {
        pcbox_body_strat(g, body);
    }
    if let Some(left_wing) = g.coldet.pcbox.lwing {
        pcbox_wing_strat(g, left_wing);
    }
    if let Some(right_wing) = g.coldet.pcbox.rwing {
        pcbox_wing_strat(g, right_wing);
    }
}

/// Advance the base player during the transfer-bound level startup without
/// running the active map or interactive flight systems.
pub fn advance_player_during_level_initialization(g: &mut Game, idx: u16) {
    g.vars.gameframe = g.vars.gameframe.wrapping_add(1);
    let _ = g.sync_player_snapshot();
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(LEVEL_INITIALIZATION_FORWARD_STEP);
    if g.vars.dummyobj > 0 {
        refresh_player_collision_proxies(g);
    }
}

/// Body-hit impact SE (ROM `pcolB_Istrat` PSTRATS.ASM:178-191).
/// Wire-ship skips the hard/soft branch (ROM `.dsn` after wireship gate).
fn play_body_hit_se(g: &mut Game, partner: Option<u16>) {
    if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
        return;
    }
    match partner {
        Some(y) if g.objs.aliens[y as usize].ap >= 8 => g.hooks.play_se(SE_BODY_HIT_HARD),
        _ => g.hooks.play_se(SE_BODY_HIT_SOFT),
    }
}

/// Low-shield warning latches on the body box (ROM `pcolB_strat` :220-233).
/// `$1b` once when HP drops through playerB_HP/4; `$1c` through /8.
/// Clears the latch when HP recovers above the threshold.
fn play_body_shield_warn_se(g: &mut Game, body_idx: u16) {
    let hp = g.objs.aliens[body_idx as usize].hp;
    if hp == 0 {
        return;
    }
    // Quarter warning.
    if hp > PLAYER_BODY_HP / 4 {
        g.objs.aliens[body_idx as usize].sflags2 &= !ASF2_SFLAG1;
    } else if g.objs.aliens[body_idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        g.objs.aliens[body_idx as usize].sflags2 |= ASF2_SFLAG1;
        g.hooks.play_se(SE_SHIELD_WARN_QUARTER);
    }
    // Eighth warning.
    if hp > PLAYER_BODY_HP / 8 {
        g.objs.aliens[body_idx as usize].sflags2 &= !ASF2_SFLAG2;
    } else if g.objs.aliens[body_idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
        g.objs.aliens[body_idx as usize].sflags2 |= ASF2_SFLAG2;
        g.hooks.play_se(SE_SHIELD_WARN_EIGHTH);
    }
}

/// Shared registry trampoline for the three proxy objects. The game-core only
/// needs one opaque strategy ID; this dispatches to the ROM-named body/left/
/// right collide entries, and doubles as their hp==0 explosion strategy.
fn pcbox_coll_strat(g: &mut Game, idx: u16) {
    let Some(kind) = g.coldet.pcbox.kind_of(idx) else {
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
        return;
    };
    if g.objs.aliens[idx as usize].hp != 0 {
        match kind {
            PcboxKind::Body => pcolb_istrat(g, idx),
            PcboxKind::LWing => pcollw_istrat(g, idx),
            PcboxKind::RWing => pcolrw_istrat(g, idx),
        }
        return;
    }

    // hp==0: pcolBexp_Istrat / PLWbrk_Istrat / PRWbrk_Istrat.
    match kind {
        PcboxKind::Body => {
            if g.vars.pshipflags & PSF_NOCTRL != 0 || g.vars.pstratflags & PSTF_NOTDIE != 0 {
                g.objs.aliens[idx as usize].hp = 1;
            } else if let Some(player) = g.coldet.pcbox.player {
                // `s_kill_obj y`: the player's own exp strategy runs on its
                // next strategy pass and performs the full detach/death init.
                g.objs.aliens[player as usize].hp = 0;
            }
        }
        PcboxKind::LWing | PcboxKind::RWing => {
            if g.vars.pshipflags3 & PSF3_NOCOLLISIONS == 0 {
                let (coll_bit, broken_bit, se) = if kind == PcboxKind::LWing {
                    (PSF_LWINGCOLL, PSF_BRKLWING, SE_WING_DESTRUCT_LEFT)
                } else {
                    (PSF_RWINGCOLL, PSF_BRKRWING, SE_WING_DESTRUCT_RIGHT)
                };
                g.hooks.play_se(se);
                let al = &mut g.objs.aliens[idx as usize];
                al.hp = HARD_HP; // ROM writes -1
                al.ap = 0;
                g.vars.pshipflags &= !coll_bit;
                g.vars.pshipflags |= broken_bit;
                if let Some(body) = g.coldet.pcbox.body {
                    g.objs.aliens[body as usize].collcount = sf_game::vars::FRAMESPERAP;
                }
            }
            pcbox_wing_strat(g, idx);
        }
    }
}

// ============================================================
// ROM-named pcbox collide / end-collide leaves (PSTRATS.ASM)
// ============================================================

fn pcollobj_valid(ptr: i16) -> bool {
    ptr > 0
}

fn play_wing_hit_se(g: &mut Game, partner: Option<u16>, soft_se: u8) {
    if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
        g.hooks.play_se(soft_se);
        return;
    }
    match partner {
        Some(y) if g.objs.aliens[y as usize].ap >= 8 => g.hooks.play_se(0x04),
        _ => g.hooks.play_se(soft_se),
    }
}

fn spawn_spexplod_fx(g: &mut Game, box_idx: u16) -> Option<u16> {
    const SH_SPEXPLOD: u16 = 367;
    let fx = strat_make_obj(g, SH_SPEXPLOD)?;
    {
        let al = &mut g.objs.aliens[fx as usize];
        al.type_ &= !ATZREMOVE;
        al.sflags |= ASF_COLLDISABLE;
    }
    let src = g.objs.aliens[box_idx as usize];
    let al = &mut g.objs.aliens[fx as usize];
    al.worldx = src.worldx;
    al.worldy = src.worldy;
    al.worldz = src.worldz;
    Some(fx)
}

/// ROM `pwingcol` scrape path (PSTRATS.ASM:104) — spark + keep wing parked.
fn pwingcol(g: &mut Game, idx: u16) {
    // PSTRATS.ASM:104-126. At the wing cooldown boundary, also route half
    // attacker AP to the body (quarter AP for wire ship). The wing itself
    // takes exactly one damage per cooldown; wire ship takes none.
    if g.objs.aliens[idx as usize].collcount == 1 {
        let partner = g.objs.aliens[idx as usize].collobjptr;
        if let Some(body) = g.coldet.pcbox.body {
            g.objs.aliens[body as usize].collobjptr = partner;
            if pcollobj_valid(partner as i16)
                && (partner as usize) < g.objs.aliens.len()
                && g.objs.aliens[partner as usize].active
            {
                let ap = g.objs.aliens[partner as usize].ap;
                let shift = if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
                    2
                } else {
                    1
                };
                g.coldet_apply_damage(body, ap, shift);
            }
        }
    }
    if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
        g.hooks.play_se(SE_WIRE_WING_SCRAPE);
    } else {
        g.coldet_apply_damage(idx, 1, 0);
    }

    sgen_spark(g, idx);
    // Copy spexplod FX to box pos if sword1 holds one.
    let sword = g.objs.aliens[idx as usize].sword1;
    if sword > 0 {
        let src = g.objs.aliens[idx as usize];
        let fx = &mut g.objs.aliens[sword as usize];
        if fx.active {
            fx.worldx = src.worldx;
            fx.worldy = src.worldy;
            fx.worldz = src.worldz;
        }
    }
    pcbox_wing_strat(g, idx);
}

/// ROM `brkpwingcol` (PSTRATS.ASM:91) — broken wing: bounce hit onto body box.
fn brkpwingcol(g: &mut Game, idx: u16) {
    if let Some(body) = g.coldet.pcbox.body {
        let partner = g.objs.aliens[idx as usize].collobjptr;
        g.objs.aliens[body as usize].collobjptr = partner;
        g.objs.aliens[body as usize].sflags |= ASF_COLLIDE;
    }
    pcbox_wing_strat(g, idx);
}

/// ROM `pendcolB_Istrat` (PSTRATS.ASM:241).
pub fn pendcolb_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_i16(sv::PCOLLOBJ_B, 0);
    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcolb_istrat));
    g.vars.pshipflags &= !PSF_BODYCOLL;
    pcbox_body_strat(g, idx);
}

/// ROM `pcolB_Istrat` — body hit entry (subset used by pendcol re-arm).
pub fn pcolb_istrat(g: &mut Game, idx: u16) {
    let partner = {
        let p = g.objs.aliens[idx as usize].collobjptr;
        if pcollobj_valid(p as i16) {
            Some(p)
        } else {
            None
        }
    };
    play_body_hit_se(g, partner);
    if let Some(y) = partner {
        if (y as usize) < g.objs.aliens.len() && g.objs.aliens[y as usize].ap == 0 {
            pcbox_body_strat(g, idx);
            return;
        }
    }
    if let Some(player) = g.coldet.pcbox.player {
        g.objs.aliens[player as usize].sbyte1 = PLAYER_HITFLASH_FRMS;
    }
    g.vars.set_sv_u8(sv::SCREENFLASHCNT, SCREENFLASH_BODY_FRMS);
    g.vars.set_sv_u8(sv::SCREENFLASHTYPE, SCREENFLASH_BODY_TYPE);
    g.vars.pshipflags |= PSF_BODYCOLL;
    g.vars.set_sv_i16(
        sv::PCOLLOBJ_B,
        g.objs.aliens[idx as usize].collobjptr as i16,
    );
    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcolb_strat));
    g.objs.aliens[idx as usize].endcollstratptr = Some(ea_sid(g, pendcolb_istrat));
    // The ROM label falls straight through into pcolB_strat.
    pcolb_strat(g, idx);
}

/// ROM `pcolB_strat` — sustain body collide while partner live.
pub fn pcolb_strat(g: &mut Game, idx: u16) {
    let partner = g.vars.sv_i16(sv::PCOLLOBJ_B);
    if partner != 0 {
        if let Some(player) = g.coldet.pcbox.player {
            g.objs.aliens[player as usize].sbyte1 = PLAYER_HITFLASH_FRMS;
        }
        if partner > 0
            && (partner as usize) < g.objs.aliens.len()
            && g.objs.aliens[partner as usize].active
        {
            let ap = g.objs.aliens[partner as usize].ap;
            let shift = u8::from(g.vars.pshipflags2 & PSF2_WIRESHIP != 0);
            g.coldet_apply_damage(idx, ap, shift);
        }
        play_body_shield_warn_se(g, idx);
    }
    pcbox_body_strat(g, idx);
}

/// ROM `pendcolLW_Istrat` (PSTRATS.ASM:370).
pub fn pendcollw_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_i16(sv::PCOLLOBJ_LW, 0);
    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcollw_istrat));
    g.vars.pshipflags &= !PSF_LWINGCOLL;
    let sword = g.objs.aliens[idx as usize].sword1;
    if sword > 0 {
        g.objs.aliens[idx as usize].sword1 = 0;
        if (sword as usize) < g.objs.aliens.len() && g.objs.aliens[sword as usize].active {
            g.objs.free(sword as u16);
        }
    }
    pcbox_wing_strat(g, idx);
}

/// ROM `pcolLW_Istrat` (PSTRATS.ASM:308) — left-wing hit entry.
pub fn pcollw_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_u8(sv::SCREENFLASHCNT, SCREENFLASH_WING_FRMS);
    g.vars.set_sv_u8(sv::SCREENFLASHTYPE, SCREENFLASH_WING_TYPE);

    let partner = {
        let p = g.objs.aliens[idx as usize].collobjptr;
        if pcollobj_valid(p as i16) {
            Some(p)
        } else {
            None
        }
    };
    play_wing_hit_se(g, partner, SE_WING_HIT_LEFT);

    if let Some(y) = partner {
        if g.objs.aliens[y as usize].ap == 0 {
            pcbox_wing_strat(g, idx);
            return;
        }
    }

    if let Some(player) = g.coldet.pcbox.player {
        let rotz = g.objs.aliens[player as usize].rotz as i8;
        let player_pitch = g.vars.sv_i16(sv::PLROTX);
        let pitch_whole = (player_pitch >> 8) as i8;
        if rotz >= 0 {
            // .nur: plrotx+1 += 8, nudge +X, Zshake
            g.vars
                .set_sv_i16(sv::PLROTX, player_pitch.wrapping_add((8i16) << 8));
            g.objs.aliens[player as usize].worldx =
                g.objs.aliens[player as usize].worldx.wrapping_add(10);
            // -deg11*256
            g.vars.set_sv_i16(sv::PLAYER_ZSHAKE, -((11i16) << 8));
            let _ = pitch_whole;
        } else {
            g.vars
                .set_sv_i16(sv::PLROTX, player_pitch.wrapping_sub((8i16) << 8));
        }
    }

    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcollw_strat));
    g.vars.set_sv_i16(
        sv::PCOLLOBJ_LW,
        g.objs.aliens[idx as usize].collobjptr as i16,
    );
    g.objs.aliens[idx as usize].endcollstratptr = Some(ea_sid(g, pendcollw_istrat));
    g.vars.pshipflags |= PSF_LWINGCOLL;
    g.objs.aliens[idx as usize].sword1 = 0;
    if g.vars.playerflymode & PFM_WATER == 0 {
        if let Some(fx) = spawn_spexplod_fx(g, idx) {
            g.objs.aliens[idx as usize].sword1 = fx as i16;
        }
    }
    // The ROM entry falls through into pcolLW_strat/pwingcol.
    pcollw_strat(g, idx);
}

/// ROM `pcolLW_strat`.
pub fn pcollw_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags & PSF_BRKLWING != 0 {
        brkpwingcol(g, idx);
    } else {
        pwingcol(g, idx);
    }
}

/// ROM `pendcolRW_Istrat` (PSTRATS.ASM:516).
pub fn pendcolrw_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_i16(sv::PCOLLOBJ_RW, 0);
    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcolrw_istrat));
    g.vars.pshipflags &= !PSF_RWINGCOLL;
    let sword = g.objs.aliens[idx as usize].sword1;
    if sword > 0 {
        g.objs.aliens[idx as usize].sword1 = 0;
        if (sword as usize) < g.objs.aliens.len() && g.objs.aliens[sword as usize].active {
            g.objs.free(sword as u16);
        }
    }
    pcbox_wing_strat(g, idx);
}

/// ROM `pcolRW_Istrat` (PSTRATS.ASM:455) — right-wing hit entry.
pub fn pcolrw_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_u8(sv::SCREENFLASHCNT, SCREENFLASH_WING_FRMS);
    g.vars.set_sv_u8(sv::SCREENFLASHTYPE, SCREENFLASH_WING_TYPE);

    let partner = {
        let p = g.objs.aliens[idx as usize].collobjptr;
        if pcollobj_valid(p as i16) {
            Some(p)
        } else {
            None
        }
    };
    play_wing_hit_se(g, partner, SE_WING_HIT_RIGHT);

    if let Some(y) = partner {
        if g.objs.aliens[y as usize].ap == 0 {
            pcbox_wing_strat(g, idx);
            return;
        }
    }

    if let Some(player) = g.coldet.pcbox.player {
        let rotz = g.objs.aliens[player as usize].rotz as i8;
        let player_pitch = g.vars.sv_i16(sv::PLROTX);
        if rotz < 0 {
            // .nur
            g.vars
                .set_sv_i16(sv::PLROTX, player_pitch.wrapping_add((8i16) << 8));
            g.objs.aliens[player as usize].worldx =
                g.objs.aliens[player as usize].worldx.wrapping_sub(10);
            g.vars.set_sv_i16(sv::PLAYER_ZSHAKE, (11i16) << 8); // deg11*256
        } else {
            g.vars
                .set_sv_i16(sv::PLROTX, player_pitch.wrapping_sub((8i16) << 8));
        }
    }

    g.objs.aliens[idx as usize].collstratptr = Some(ea_sid(g, pcolrw_strat));
    g.vars.set_sv_i16(
        sv::PCOLLOBJ_RW,
        g.objs.aliens[idx as usize].collobjptr as i16,
    );
    g.objs.aliens[idx as usize].endcollstratptr = Some(ea_sid(g, pendcolrw_istrat));
    g.vars.pshipflags |= PSF_RWINGCOLL;
    g.objs.aliens[idx as usize].sword1 = 0;
    if g.vars.playerflymode & PFM_WATER == 0 {
        if let Some(fx) = spawn_spexplod_fx(g, idx) {
            g.objs.aliens[idx as usize].sword1 = fx as i16;
        }
    }
    // The ROM entry falls through into pcolRW_strat/pwingcol.
    pcolrw_strat(g, idx);
}

/// ROM `pcolRW_strat`.
pub fn pcolrw_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags & PSF_BRKRWING != 0 {
        brkpwingcol(g, idx);
    } else {
        pwingcol(g, idx);
    }
}

/// C `setcurrpshape` (strat_player.c:262).
fn setcurrpshape(g: &mut Game, idx: u16) {
    let damage = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING);
    let pick = |variable: sv, g: &Game| {
        let v = g.vars.sv_u16(variable);
        if v != 0 {
            v
        } else {
            SHAPE_ARWING
        }
    };
    let shape = if damage == 0 {
        pick(sv::PLAYERSHAPE, g)
    } else if damage == PSF_BRKLWING {
        pick(sv::PLAYERSHAPEL, g)
    } else if damage == PSF_BRKRWING {
        pick(sv::PLAYERSHAPER, g)
    } else {
        pick(sv::PLAYERSHAPELR, g)
    };
    g.objs.aliens[idx as usize].shape = shape;
}

// ============================================================
// Barrel roll / boost / brake (strat_player.c:276-359)
// ============================================================

/// C `barrel_roll_update` (playermove_srou .nroll/.zroll blocks).
///
/// The barrel roll triggers on the L/R SHOULDER buttons only, matching the
/// ASM `s_jmp_anyLRkeyup`/`s_jmp_anyLRkeydown` macros (STRATMAC.INC:1389-1403),
/// which test `contl0 & (key_leftl|key_rightl)`. Those bits are `$20|$10` in
/// the pad low byte = bits 5,4 = `pad_TLEFT`/`pad_TRIGHT` (VARS.INC:55-56,72-73)
/// — the "LR" is Left/Right *shoulder*, NOT the dpad. The frozen C port's
/// `barrel_roll_update` wrongly OR-ed in `PAD_LEFT|PAD_RIGHT` (dpad steering),
/// so on a gamepad, steering left/right spuriously triggered rolls. Steering
/// is handled separately by the `PAD_LEFT`/`PAD_RIGHT` blocks in
/// `playermove_srou`.
fn barrel_roll_update(g: &mut Game, allow_start: bool) {
    let lr_mask = pad::TLEFT | pad::TRIGHT;
    let lr_down = pad1(g) & lr_mask != 0;
    let lr_prev = pad1_prev(g) & lr_mask != 0;

    let roll_zvel = g.vars.sv_u8(sv::PLAYER_ROLLZVEL);
    if roll_zvel as i8 == 0 {
        // ROM `s_beqdec_var player_rolldelay,.lragain` (PSTRATS.ASM:2584 /
        // STRATMAC.INC:6391): branch-if-zero BEFORE decrement. Start path runs
        // only while the window is open (`rolldelay>0`); delay==0 skips straight
        // to `.lragain` (reload-only). Old port decremented first then required
        // post-dec delay==0, tightening the double-tap window by one frame
        // (audit Minor #9).
        let delay = g.vars.sv_u8(sv::PLAYER_ROLLDELAY);
        if delay != 0 {
            g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, delay - 1);
            // Fresh shoulder edge while window open → start roll.
            // Polarity: default −32, then `s_jmp_keyup left,.isright` keeps it
            // when TLEFT is up; TLEFT down overrides to +32 (PSTRATS.ASM:2587-93).
            if allow_start && lr_down && !lr_prev {
                let zvel: i8 = if pad1(g) & pad::TLEFT != 0 { 32 } else { -32 };
                g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, zvel as u8);
                g.vars.set_sv_u8(sv::PLAYER_ROLLZOFF, 0);
            }
        }

        // .lragain: while either shoulder is held, reload the double-tap window.
        if lr_down {
            g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, BARREL_ROLL_DELAY);
        }

        let off = g.vars.sv_u8(sv::PLAYER_ROLLZOFF) as i8;
        let off = strat_chase_proportional(off as i16, 0, 3) as u8;
        g.vars.set_sv_u8(sv::PLAYER_ROLLZOFF, off);
        return;
    }

    let off = g.vars.sv_u8(sv::PLAYER_ROLLZOFF) as i8;
    g.vars
        .set_sv_u8(sv::PLAYER_ROLLZOFF, off.wrapping_add(roll_zvel as i8) as u8);
    if (roll_zvel as i8) < 0 {
        g.vars
            .set_sv_u8(sv::PLAYER_ROLLZVEL, (roll_zvel as i8).wrapping_add(2) as u8);
    } else {
        g.vars
            .set_sv_u8(sv::PLAYER_ROLLZVEL, (roll_zvel as i8).wrapping_sub(2) as u8);
    }
}

/// C `boost_brake_update` (strat_player.c:313).
pub fn boost_brake_update(g: &mut Game, idx: u16) {
    let i = idx as usize;
    if g.objs.aliens[i].sbyte2 > 0 {
        g.objs.aliens[i].sbyte2 -= 1;
        if g.objs.aliens[i].sbyte2 == 0 {
            g.vars.pshipflags2 &= !(PSF2_BOOSTING | PSF2_BRAKING);
        }
    }

    if g.vars.pshipflags2 & PSF2_FORCEBOOST != 0 {
        g.vars.pshipflags2 &= !PSF2_FORCEBOOST;
        g.vars.pshipflags2 |= PSF2_BOOSTING;
        g.vars.set_sv_u8(sv::BOOSTCNT, 1);
        g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
        g.objs.aliens[i].vel = MAX_PSPEED as u8;
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
        g.objs.aliens[i].sbyte2 = 20;
        g.hooks.play_se(0x32);
        return;
    }

    if g.vars.pshipflags3 & PSF3_FORCEBRAKE != 0 {
        g.vars.pshipflags3 &= !PSF3_FORCEBRAKE;
        g.vars.pshipflags2 |= PSF2_BRAKING;
        g.vars.set_sv_u8(sv::BOOSTCNT, 1);
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MIN_PSPEED as u8);
        g.objs.aliens[i].sbyte2 = 30;
        g.hooks.play_se(0x33);
        return;
    }

    // No-control gate (PSTRATS.ASM:2088-2097): the pad-X/pad-B boost/brake is
    // dead during black-screen / wipe / noctrl sequences. Force boost/brake
    // above run BEFORE this gate (ROM `brl .boost`/`.brake` bypasses it).
    // player_noctrlcnt is only READ here — playermove_srou owns the
    // once-per-frame decrement (it runs later this tick via do_player_*).
    let no_ctrl = g.vars.pshipflags & PSF_NOCTRL != 0
        || g.vars.sv_i8(sv::STAYBLACK) != -1
        || g.vars.sv_u8(sv::DOINGWIPE) != 0
        || g.vars.sv_u8(sv::PLAYER_NOCTRLCNT) != 0;
    if no_ctrl {
        return;
    }

    // Speed gate (PSTRATS.ASM:2103, `s_jmp_alvarNOTZERO al_sbyte2,.npsd`):
    // while the boost/brake timer sbyte2 is nonzero, holding X/B does NOTHING.
    // The boost is a pulsed 20-frame (brake: 30) burst that cannot re-trigger
    // until sbyte2 decrements back to 0 (which also clears the boosting/braking
    // flag, top of this fn). Without this gate the held key re-set sbyte2/vel
    // every frame, pinning speed to max/min and replaying the SFX each frame.
    // (ROM also gates on m_boostanim>=40, the boost-meter charge, PSTRATS.ASM:
    //  2105-2107 — that value lives in the sf-render HUD, not sf-strat, so it is
    //  not checked here; see audit follow-up.)
    if g.objs.aliens[i].sbyte2 != 0 {
        return;
    }

    if pad1(g) & pad::X != 0 {
        g.vars.pshipflags2 |= PSF2_BOOSTING;
        g.vars.set_sv_u8(sv::BOOSTCNT, 1);
        g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16); // PSTRATS.ASM:2173
        g.objs.aliens[i].vel = MAX_PSPEED as u8;
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
        g.objs.aliens[i].sbyte2 = 20;
        g.hooks.play_se(0x32);
        return;
    }

    if pad1(g) & pad::B != 0 {
        g.vars.pshipflags2 |= PSF2_BRAKING;
        g.vars.set_sv_u8(sv::BOOSTCNT, 1);
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MIN_PSPEED as u8);
        g.objs.aliens[i].sbyte2 = 30;
        g.hooks.play_se(0x33);
    }
}

// ============================================================
// playermove_srou (strat_player.c:362-421)
// ============================================================

fn playermove_srou(g: &mut Game, idx: u16) {
    let i = idx as usize;
    player_hitflash_update(g, idx);

    // Wire-ship (Shield Power-up) hit check — PSTRATS.ASM:1775-1796.
    // While psf2_wireship: if pnumhits >= 3, flash shieldup for wireendflash
    // frames then clear the wire bit; else arm wireendflash=50.
    if g.vars.pshipflags2 & PSF2_WIRESHIP != 0 {
        let hits = g.vars.sv_u8(sv::PNUMHITS);
        if hits >= 3 {
            let flash = g.vars.wireendflash;
            if flash == 0 {
                g.vars.shieldup = 0;
                select_ship(g, PSHIPNUM_NORM);
                g.vars.pshipflags2 &= !PSF2_WIRESHIP;
            } else {
                let remaining = flash.wrapping_sub(1);
                g.vars.wireendflash = remaining;
                // Blink: shieldup on when (wireendflash & 3) != 0.
                if remaining & 3 != 0 {
                    g.vars.shieldup = 1;
                    select_ship(g, PSHIPNUM_WIRE);
                } else {
                    g.vars.shieldup = 0;
                    select_ship(g, PSHIPNUM_NORM);
                }
            }
        } else {
            g.vars.wireendflash = 50;
        }
    }

    let mut no_ctrl = g.vars.pshipflags & PSF_NOCTRL != 0
        || g.vars.sv_i8(sv::STAYBLACK) != -1
        || g.vars.sv_u8(sv::DOINGWIPE) != 0;

    let noctrlcnt = g.vars.sv_u8(sv::PLAYER_NOCTRLCNT);
    if noctrlcnt > 0 {
        g.vars.set_sv_u8(sv::PLAYER_NOCTRLCNT, noctrlcnt - 1);
        no_ctrl = true;
    }

    let mut plrotx = g.vars.sv_i16(sv::PLROTX);
    let mut plroty = g.vars.sv_i16(sv::PLROTY);
    let mut plrotz = g.vars.sv_i16(sv::PLROTZ);
    let mut ztilt = g.vars.sv_u8(sv::PLAYER_ZTILT);

    // Banking -> lateral worldx shove (PSTRATS.ASM:2278-2317). Each tick the
    // ship's roll directly nudges its X position; this is what makes steering
    // responsive and gives the sideways glide as `plrotz` decays after you let
    // go. Missing from the C port (and thus the RIIR copy) — steering felt
    // sluggish without it. Uses the pre-increment plrotz/ztilt (this frame's
    // roll carried from last frame). `adiv2` is a signed /2 that rounds toward
    // zero (STRATMAC.INC:712), so plrotz is `(plrotz>>7)` then toward-zero /2.
    {
        let lr_held = pad1(g) & (pad::LEFT | pad::RIGHT) != 0;
        let ztilt_term = if lr_held {
            (ztilt as i8 as i16) >> 3
        } else {
            0
        };
        let s7 = plrotz >> 7;
        let plrotz_term = if s7 >= 0 { s7 >> 1 } else { -((-s7) >> 1) };
        let shove = plrotz_term.wrapping_add(ztilt_term);
        // ROM negates the shove (`nega`, PSTRATS.ASM:2306) then normal flight
        // ADDs it (worldx -= shove) and turn180 SUBs it (worldx += shove).
        // LEFT raises plrotz (+), so worldx must DECREASE (= screen-left). An
        // earlier port of this dropped the nega, inverting left/right.
        let turn180 = g.vars.pshipflags2 & PSF2_TURN180 != 0;
        let al = &mut g.objs.aliens[i];
        if turn180 {
            al.worldx = al.worldx.wrapping_add(shove);
        } else {
            al.worldx = al.worldx.wrapping_sub(shove);
        }
    }

    if !no_ctrl {
        // Dpad steer ztilt (deg45/15) — ROM gates on wing-not-against-wall
        // (`pmovelimit` pml_lwleft/rwright) and, when `pml_Bbottom` is armed,
        // skips the bank when `worldy >= maxPmoveY-30` (`s_jmp_lower`,
        // PSTRATS.ASM:2320-2358). plrotz/plroty still update either way.
        let pmove_limit = g.vars.sv_u8(sv::PMOVELIMIT);
        let pmove_and = g.vars.sv_u8(sv::PMOVELIMITAND);
        let max_y = g.vars.sv_i16(sv::MAXPMOVEY);
        let near_floor =
            pmove_and & PML_BBOTTOM != 0 && g.objs.aliens[i].worldy >= max_y.wrapping_sub(30);

        if pad1(g) & pad::LEFT != 0 {
            plrotz = plrotz.wrapping_add(ZROT_SPEED);
            plroty = plroty.wrapping_add(ZROT_SPEED);
            if pmove_limit & PML_LWLEFT == 0 && !near_floor {
                ztilt = ((ztilt as i8) as i32 + (DEG45 as i32 / 15)) as u8;
                if ztilt as i8 > DEG90 as i8 {
                    ztilt = DEG90;
                }
            }
        }
        if pad1(g) & pad::RIGHT != 0 {
            plrotz = plrotz.wrapping_sub(ZROT_SPEED);
            plroty = plroty.wrapping_sub(ZROT_SPEED);
            if pmove_limit & PML_RWRIGHT == 0 && !near_floor {
                ztilt = ((ztilt as i8) as i32 - (DEG45 as i32 / 15)) as u8;
                if (ztilt as i8) < -(DEG90 as i8) {
                    ztilt = (-(DEG90 as i8)) as u8;
                }
            }
        }

        // Shoulder-hold bank lean (PSTRATS.ASM:2626-2639): while L/R shoulder
        // held, `player_Ztilt ±= deg45/3`, clamp ±deg90. Distinct from the
        // smaller dpad-steer term above (deg45/15) and from the double-tap
        // barrel roll in [`barrel_roll_update`].
        if pad1(g) & pad::TLEFT != 0 {
            ztilt = ((ztilt as i8) as i32 + (DEG45 as i32 / 3)) as u8;
            if ztilt as i8 > DEG90 as i8 {
                ztilt = DEG90;
            }
        } else if pad1(g) & pad::TRIGHT != 0 {
            ztilt = ((ztilt as i8) as i32 - (DEG45 as i32 / 3)) as u8;
            if (ztilt as i8) < -(DEG90 as i8) {
                ztilt = (-(DEG90 as i8)) as u8;
            }
        }

        if g.vars.pshipflags & PSF_NOYCTRL == 0 {
            if pad1(g) & pad::UP != 0 {
                plrotx = plrotx.wrapping_add(XROT_SPEED);
            }
            if pad1(g) & pad::DOWN != 0 {
                plrotx = plrotx.wrapping_sub(XROT_SPEED);
            }
        }
    }

    // Write back before barrel_roll_update (it reads/writes sv itself).
    g.vars.set_sv_i16(sv::PLROTX, plrotx);
    g.vars.set_sv_i16(sv::PLROTY, plroty);
    g.vars.set_sv_i16(sv::PLROTZ, plrotz);
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, ztilt);

    barrel_roll_update(g, !no_ctrl);

    let ztilt = strat_chase_proportional(g.vars.sv_u8(sv::PLAYER_ZTILT) as i8 as i16, 0, 3) as u8;
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, ztilt);
    let plroty = strat_chase_proportional(g.vars.sv_i16(sv::PLROTY), 0, 3);
    g.vars.set_sv_i16(sv::PLROTY, plroty);
    let mut plrotz = strat_chase_proportional(g.vars.sv_i16(sv::PLROTZ), 0, 4);
    let plrotx = strat_chase_proportional(g.vars.sv_i16(sv::PLROTX), 0, 3);
    g.vars.set_sv_i16(sv::PLROTX, plrotx);

    g.objs.aliens[i].vel = clamp16(g.objs.aliens[i].vel as i16, MIN_PSPEED, MAX_PSPEED) as u8;
    plrotz = clamp16(plrotz, -0x600, 0x600);
    g.vars.set_sv_i16(sv::PLROTZ, plrotz);

    // gf_viewrot view-lean accumulators (PSTRATS.ASM:1928-1966). When the
    // gf_viewrot flag is set (all-range / U-turn / boss arenas), directional
    // input leans the *view* accumulators outvx/outvy, which getview_l feeds
    // straight to the camera; both then decay toward 0 (achase rate 3) each
    // frame. This is the source the camera should follow — NOT the ship's own
    // rotation. (`centoutrots` is the same decay used elsewhere; not called
    // here, so the achase is not double-applied.)
    if g.vars.gameflags & GF_VIEWROT != 0 {
        // psf_noctrl | psf_noYctrl gate: skip the input nudge, still decay.
        if g.vars.pshipflags & (PSF_NOCTRL | PSF_NOYCTRL) == 0 {
            let pad = pad1(g);
            // up: outvx += -256 ; down: outvx -= -256 (= +256). (Signs per ROM
            // s_add_var/s_sub_var W,outvx,#-256; jup/jdown are the held keys.)
            if pad & pad::UP != 0 {
                let v = g.vars.sv_i16(sv::OUTVX).wrapping_add(-256);
                g.vars.set_sv_i16(sv::OUTVX, v);
            }
            if pad & pad::DOWN != 0 {
                let v = g.vars.sv_i16(sv::OUTVX).wrapping_sub(-256);
                g.vars.set_sv_i16(sv::OUTVX, v);
            }
            // left/right yaw lean only when the ship is within 300 of the X
            // movement boundary (svar_word1 = minPmoveX+300, svar_word2 =
            // maxPmoveX-300): s_jmp_alvarmore/less gate it to the screen edge.
            let worldx = g.objs.aliens[i].worldx;
            let lo = g.vars.sv_i16(sv::MINPMOVEX).wrapping_add(300);
            let hi = g.vars.sv_i16(sv::MAXPMOVEX).wrapping_sub(300);
            if worldx <= lo && pad & pad::LEFT != 0 {
                let v = g.vars.sv_i16(sv::OUTVY).wrapping_sub(200);
                g.vars.set_sv_i16(sv::OUTVY, v);
            }
            if worldx >= hi && pad & pad::RIGHT != 0 {
                let v = g.vars.sv_i16(sv::OUTVY).wrapping_add(200);
                g.vars.set_sv_i16(sv::OUTVY, v);
            }
        }
        // .zerovrot: s_achase_var W,outvx,#0,3 / s_achase_var W,outvy,#0,3.
        let vx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 3);
        let vy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 3);
        g.vars.set_sv_i16(sv::OUTVX, vx);
        g.vars.set_sv_i16(sv::OUTVY, vy);
    }

    let turnrot = g.vars.sv_i16(sv::PLAYER_TURNROT);
    let zshake = g.vars.sv_i16(sv::PLAYER_ZSHAKE);
    let zstratadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD) as i8;
    let rollzoff = g.vars.sv_u8(sv::PLAYER_ROLLZOFF) as i8;

    // Idle bank wobble (PSTRATS.ASM:2728-2741): under `pfm_wobble`, walk
    // `pZrotfloattab` into `player_Zrotfloat` and add it to al_rotz. Broken
    // wing(s) use the table as-is; intact wings negate the sample.
    let mut zrot_float: i8 = 0;
    if g.vars.playerflymode & PFM_WOBBLE != 0 {
        let mut ptr = g.vars.sv_u8(sv::PLAYER_ZROTFLOATPTR);
        if (ptr as usize) >= PZROT_FLOAT_TAB.len() {
            ptr = 0;
        }
        let sample = PZROT_FLOAT_TAB[ptr as usize];
        zrot_float = if g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING) != 0 {
            sample
        } else {
            sample.wrapping_neg()
        };
        g.vars.set_sv_u8(sv::PLAYER_ZROTFLOAT, zrot_float as u8);
        ptr = ptr.wrapping_add(1);
        if ptr >= PZROTFLOATTAB_LEN {
            ptr = 0;
        }
        g.vars.set_sv_u8(sv::PLAYER_ZROTFLOATPTR, ptr);
    }

    let al = &mut g.objs.aliens[i];
    al.rotx = (plrotx >> 8) as u8;
    al.roty = ((plroty >> 8) as i32 + (turnrot >> 8) as i32) as u8;
    al.rotz = ((plrotz >> 8) as i32
        + ztilt as i8 as i32
        + (zshake >> 8) as i32
        + zstratadd as i32
        + rollzoff as i32
        + zrot_float as i32) as u8;

    // spfm_inside (PSTRATS.ASM:2749-2755): carry the ship's roll (sign-extended
    // player_Ztilt byte + full 16-bit player_Zshake) into outvz so getview_l
    // rolls the inside-tunnel camera with the ship. Runs after the al_rot* set
    // (matching ROM order); outvz is not read again this tick.
    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        let ztilt_v = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8 as i16;
        let zshake_v = g.vars.sv_i16(sv::PLAYER_ZSHAKE);
        g.vars.set_sv_i16(sv::OUTVZ, ztilt_v.wrapping_add(zshake_v));
    }

    let hudrot = al.rotz as i8 as i16;
    let vel = al.vel;
    g.vars.set_sv_i16(sv::HUDROT, hudrot);
    g.vars.set_sv_i16(sv::PLAYER_SPEED, vel as i16);
    g.vars.set_sv_u8(sv::PMOVELIMIT, 0);

    setcurrpshape(g, idx);
}

// ============================================================
// playerlimitX / checkarrows (strat_player.c:424-466)
// ============================================================

/// C `playerlimitX_srou`.
fn playerlimit_x_srou(g: &mut Game, idx: u16) {
    let i = idx as usize;
    let mut arrows = g.vars.sv_u8(sv::ARROWS) & !(SPRAR_RIGHT | SPRAR_LEFT);

    let minx = g.vars.sv_i16(sv::MINPMOVEX);
    let maxx = g.vars.sv_i16(sv::MAXPMOVEX);
    // ROM playerlimitx_srou ($BDF1C): BEQ+BMI / BEQ+BPL -> clamp+arrow fire at
    // the INCLUSIVE boundary (worldX <= min / >= max). The port used `<`/`>`
    // (exclusive) and dropped the edge arrow when pinned exactly at the limit.
    // Oracle-confirmed (sf-oracle tests/player_bounds.rs). Task #34.
    if g.objs.aliens[i].worldx <= minx {
        g.objs.aliens[i].worldx = minx;
        arrows |= SPRAR_LEFT;
    }
    if g.objs.aliens[i].worldx >= maxx {
        g.objs.aliens[i].worldx = maxx;
        arrows |= SPRAR_RIGHT;
    }

    // The ROM's ordinary lower-screen clamp is active when the body-bottom
    // collision lane is clear. When that lane is set, detailed body collision
    // owns the floor instead (PSTRATS.ASM:1912-1922).
    let miny = g.vars.minpmove_y;
    let maxy = g.vars.sv_i16(sv::MAXPMOVEY);
    let limit_and = g.vars.sv_u8(sv::PMOVELIMITAND);
    if limit_and & PML_BBOTTOM == 0 && g.objs.aliens[i].worldy >= maxy {
        g.objs.aliens[i].worldy = maxy;
        arrows |= SPRAR_DOWN;
    }
    if g.objs.aliens[i].worldy <= miny {
        g.objs.aliens[i].worldy = miny;
        arrows |= SPRAR_UP;
    }

    g.vars.set_sv_u8(sv::ARROWS, arrows);
}

/// C `checkarrows_srou`.
fn checkarrows_srou(g: &mut Game) {
    if g.vars.pstratflags & PSTF_INSEQ != 0 {
        g.vars.set_sv_u8(sv::ARROWS, 0);
        return;
    }

    let mut arrows = g.vars.sv_u8(sv::ARROWS);
    let limit_and = g.vars.sv_u8(sv::PMOVELIMITAND);
    if pad1(g) & pad::DOWN == 0 || limit_and & PML_BTOP == 0 {
        arrows &= !SPRAR_UP;
    }
    if pad1(g) & pad::UP == 0 || limit_and & PML_BBOTTOM == 0 {
        arrows &= !SPRAR_DOWN;
    }
    if pad1(g) & pad::LEFT == 0 || limit_and & PML_LWLEFT == 0 {
        arrows &= !SPRAR_LEFT;
    }
    if pad1(g) & pad::RIGHT == 0 || limit_and & PML_RWRIGHT == 0 {
        arrows &= !SPRAR_RIGHT;
    }
    g.vars.set_sv_u8(sv::ARROWS, arrows);
}

// ============================================================
// playerfire (strat_player.c:468-606)
// ============================================================

/// C `spawn_player_projectile` (strat_player.c:468).
#[allow(clippy::too_many_arguments)]
fn spawn_player_projectile(
    g: &mut Game,
    idx: u16,
    off_x: i16,
    off_y: i16,
    off_z: i16,
    speed: u8,
    lifetime: u8,
    ap: u8,
    track_in_numplasers: bool,
    inside_beam_extra_z: bool,
) -> Option<u16> {
    let i = idx as usize;
    let mut dx = off_x;
    let mut dy = off_y;
    let mut dz = off_z;

    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        let al = &g.objs.aliens[i];
        dx = dx.wrapping_add(g.vars.sv_i16(sv::PVIEWPOSX).wrapping_sub(al.worldx));
        dy = dy.wrapping_add(
            g.vars
                .sv_i16(sv::PVIEWPOSY)
                .wrapping_add(INVIEW_LASER_Y_OFF)
                .wrapping_sub(al.worldy),
        );
        dz = dz.wrapping_add(g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_sub(al.worldz));
        if inside_beam_extra_z {
            dz = dz.wrapping_add(200);
        }
    }

    let (rot_x, rot_y) = {
        let al = &g.objs.aliens[i];
        (al.rotx, al.roty)
    };

    let shot = strat_spawn_projectile(
        g,
        Some(idx),
        dx,
        dy,
        dz,
        rot_x,
        rot_y,
        speed,
        lifetime,
        ap,
        ACF_COLLTYPE1,
    )?;

    if track_in_numplasers {
        // The beam and ordinary laser use distinct retail shapes.
        let owner_vel = g.objs.aliens[i].vel;
        let laser = &mut g.objs.aliens[shot as usize];
        laser.shape = if inside_beam_extra_z {
            415 // playerbeam
        } else {
            SHAPE_PLAYER_LASER
        };
        if !inside_beam_extra_z {
            // Pelaser_Istrat: s_init_anim x,#4. Bit 7 keeps the authored
            // elaser2_P frame instead of following gameframe.
            laser.animframe = 0x80 | 4;
        }
        laser.sflags &= !ASF_INVISIBLE;
        laser.sbyte6 |= 1;
        // ROM `Pelaser_Istrat` (GSTRATS.ASM:2023) builds the bolt velocity from
        // two stacked vectors: `gen_3dvecs scale 2` (x4) at the bolt's al_vel
        // PLUS `addgen_3dvecs` at the OWNER's speed (al_sbyte3 = player al_vel).
        // strat_spawn_projectile only gives it one x1 vector (~ship speed), so
        // the bolt crept along with the ship and hung at the muzzle as an
        // end-on dot — "can barely see the lasers". Rebuild it faithfully.
        laser.vel = 66; // ROM Pelaser al_vel (GSTRATS.ASM:2350)
        strat_gen_vecs_3d(laser);
        let (bx, by, bz) = (
            laser.vx.wrapping_mul(4),
            laser.vy.wrapping_mul(4),
            laser.vz.wrapping_mul(4),
        );
        laser.vel = owner_vel;
        strat_gen_vecs_3d(laser);
        laser.vx = laser.vx.wrapping_add(bx);
        laser.vy = laser.vy.wrapping_add(by);
        laser.vz = laser.vz.wrapping_add(bz);
        laser.vel = 66;
        let n = g.vars.sv_u8(sv::NUMPLASERS);
        if n < 0xFF {
            g.vars.set_sv_u8(sv::NUMPLASERS, n + 1);
        }
    }

    Some(shot)
}

/// C `playerfire_srou` (strat_player.c:507).
fn playerfire_srou(g: &mut Game, idx: u16) {
    if g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING) != 0 {
        g.vars.pshipflags2 &= !PSF2_DOUBLASER;
        g.vars.pshipflags3 &= !PSF3_BEAMBALL;
    }

    if g.vars.pshipflags & PSF_NOFIRE != 0 {
        return;
    }
    if g.vars.sv_i8(sv::STAYBLACK) != -1 || g.vars.sv_u8(sv::DOINGWIPE) != 0 {
        return;
    }

    // A button = nova bomb
    let mut can_use_special = true;
    let mut specialdelay = g.vars.sv_u8(sv::SPECIALDELAY);
    if specialdelay > 0 {
        specialdelay -= 1;
        g.vars.set_sv_u8(sv::SPECIALDELAY, specialdelay);
        can_use_special = specialdelay == 0;
    }

    let specwepcnt = g.vars.sv_u16(sv::SPECWEPCNT);
    if can_use_special && pad1_new(g) & pad::A != 0 && specwepcnt > 0 {
        g.vars.set_sv_u8(sv::SPECIALDELAY, SPECIAL_DELAY_FRMS);
        g.vars.set_sv_u16(sv::SPECWEPCNT, specwepcnt - 1);

        // ROM fire_nuke (GSTRATS.ASM:2333) — speed 50, life 28, hp2/ap8.
        let _ = fire_nuke(g, idx);
        g.hooks.play_se(0x31);
    }
    if g.vars.sv_u8(sv::SPECIALDELAY) == 0 {
        g.vars.set_sv_u8(sv::SPECIALDELAY, 1);
    }

    // Y button = laser/beam
    if pad1(g) & pad::Y == 0 {
        g.vars.set_sv_u8(sv::FIRECNT, 3);
        g.vars.set_sv_u8(sv::FIREDELAY, 1);
        return;
    }

    let firedelay = g.vars.sv_u8(sv::FIREDELAY);
    if firedelay > 0 {
        g.vars.set_sv_u8(sv::FIREDELAY, firedelay - 1);
        return;
    }
    g.vars.set_sv_u8(sv::FIREDELAY, PLAYER_FIRESPEED);

    let firecnt = g.vars.sv_u8(sv::FIRECNT);
    if firecnt == 0 {
        return;
    }
    g.vars.set_sv_u8(sv::FIRECNT, firecnt - 1);

    let beam = g.vars.pshipflags3 & PSF3_BEAMBALL != 0;
    let dbl = g.vars.pshipflags2 & PSF2_DOUBLASER != 0;
    let max_active: u8 = if beam || dbl { 7 } else { 3 };
    if g.vars.sv_u8(sv::NUMPLASERS) > max_active {
        return;
    }

    if beam {
        spawn_player_projectile(
            g,
            idx,
            -PLAYER_W_X_SCALED,
            PLAYER_W_Y_SCALED,
            10,
            64,
            55,
            4,
            true,
            true,
        );
        spawn_player_projectile(
            g,
            idx,
            PLAYER_W_X_SCALED,
            PLAYER_W_Y_SCALED,
            10,
            64,
            55,
            4,
            true,
            true,
        );
        g.hooks.play_se(0x36);
        return;
    }

    if dbl {
        spawn_player_projectile(
            g,
            idx,
            -PLAYER_W_X_SCALED,
            PLAYER_W_Y_SCALED,
            80,
            66,
            10,
            2,
            true,
            false,
        );
        spawn_player_projectile(
            g,
            idx,
            PLAYER_W_X_SCALED,
            PLAYER_W_Y_SCALED,
            80,
            66,
            10,
            2,
            true,
            false,
        );
        g.hooks.play_se(0x34);
        return;
    }

    // Lifetime 10 = ROM Pelaser `s_set_lifecnt #10` (GSTRATS.ASM:2351). Was
    // 45, so bolts lingered ~2.25s and the 3-shot cap blocked re-firing.
    spawn_player_projectile(g, idx, 0, 0, 80, 66, 10, 2, true, false);
    // ROM single-fire: `trigse se_laser` (= $35). Port had $60 (wrong SFX =
    // the "wrong noise"). PSTRATS.ASM playerfire_srou. Double=$34, beam=$36,
    // nova=$31 already correct.
    g.hooks.play_se(0x35);
}

// ============================================================
// do_player_* mode ticks (strat_player.c:608-650)
// ============================================================

/// C `framescalevecs` — fixed-step runtime no-op.
fn framescalevecs(_g: &mut Game, _idx: u16) {}

fn do_player_limit_x(g: &mut Game, idx: u16) {
    playermove_srou(g, idx);
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    framescalevecs(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    playerlimit_x_srou(g, idx);
    playerfire_srou(g, idx);
    checkarrows_srou(g);
}

fn do_player_yvel125(g: &mut Game, idx: u16) {
    playermove_srou(g, idx);
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);

    {
        let al = &mut g.objs.aliens[idx as usize];
        let vy = al.vy;
        // do_player_Yvel125 (PSTRATS.ASM:3374-3395): vy = vy + vy>>2 + vy>>3
        // (= x1.375, despite the "125" name). The ROM uses `asra` (arithmetic
        // shift right, toward -inf); i16 `>>` is arithmetic too, so the negative
        // (climbing, SNES +y=down) case matches the ROM. Previously only added
        // vy>>2 (x1.25), so every pitch climb/dive was ~9% weaker than ROM.
        al.vy = vy.wrapping_add(vy >> 2).wrapping_add(vy >> 3);
    }

    framescalevecs(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    playerlimit_x_srou(g, idx);
    playerfire_srou(g, idx);
    checkarrows_srou(g);
}

fn do_player_yvel_d2(g: &mut Game, idx: u16) {
    playermove_srou(g, idx);
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = strat_perc62(al.vx);
        // do_playerYvelD2 (PSTRATS.ASM:3417-3419): `adiv2` = signed halve that
        // rounds TOWARD ZERO (STRATMAC.INC:712), not the toward-(-inf) arithmetic
        // `>>`. They differ by 1 each frame for upward (vy<0) motion.
        let vy = al.vy;
        al.vy = if vy >= 0 {
            vy >> 1
        } else {
            -((-(vy as i32) >> 1) as i16)
        };
    }

    framescalevecs(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    playerlimit_x_srou(g, idx);
    playerfire_srou(g, idx);
    checkarrows_srou(g);
}

/// ROM `do_playerYvel_colony` (PSTRATS.ASM:3428): perc62 on vx, adiv2 on vy.
fn do_player_yvel_colony(g: &mut Game, idx: u16) {
    playermove_srou(g, idx);
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = strat_perc62(al.vx);
        let vy = al.vy;
        al.vy = if vy >= 0 {
            vy >> 1
        } else {
            -((-(vy as i32) >> 1) as i16)
        };
    }

    framescalevecs(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    playerlimit_x_srou(g, idx);
    playerfire_srou(g, idx);
    checkarrows_srou(g);
}

// ============================================================
// viewmove (strat_player.c:653-697)
// ============================================================

/// C `viewmove_srou` (Z chase/speed logic).
fn viewmove_srou(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // PSTRATS.ASM:1616-1629. This is before the PSTF_NOVIEWMOVE gate in the
    // ROM, so cutscene/player modes still publish the current engine pitch.
    g.vars.player_snd_flag = 0b0000_0100;
    if g.objs.aliens[i].sbyte2 != 0 {
        if g.vars.pshipflags2 & PSF2_BOOSTING != 0 {
            g.vars.player_snd_flag = 0b0000_1000;
        } else if g.vars.pshipflags2 & PSF2_BRAKING != 0 {
            g.vars.player_snd_flag = 0b0000_1100;
        }
    }

    if g.vars.pstratflags & PSTF_NOVIEWMOVE != 0 {
        let z = g.vars.sv_i16(sv::VIEWPOSZ);
        g.vars.set_sv_i16(sv::BGSSCROLLZ, z);
        return;
    }

    // View-distance ease (PSTRATS.ASM:1636-1638): unless PSTF_NOVDISTC, each
    // frame `outdist` chases `viewdist` at rate 3 so the camera pull-back eases
    // to a new distance instead of snapping. `sf-game` camera.rs reads
    // `outdist` (sv::OUTDIST) for the pull-back length.
    if g.vars.pstratflags & PSTF_NOVDISTC == 0 {
        let od = strat_chase_proportional(g.vars.sv_i16(sv::OUTDIST), g.vars.viewdist, 3);
        g.vars.set_sv_i16(sv::OUTDIST, od);
    }

    let mut pviewposz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(g.vars.pviewvelz);
    g.vars.pviewvelz = strat_chase(g.vars.pviewvelz, g.objs.aliens[i].vz, 1);

    let mut z_diff = pviewposz.wrapping_sub(g.vars.player_posz);
    z_diff = clamp16(z_diff, -200, 50);
    pviewposz = g.vars.player_posz.wrapping_add(z_diff);

    if g.objs.aliens[i].sbyte2 <= 10 {
        let med = g.vars.sv_u8(sv::PLAYER_MEDSPEED);
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, med);
        pviewposz = strat_chase_proportional(pviewposz, g.vars.player_posz, 3);
    }
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pviewposz);

    let tospeed = g.vars.sv_u8(sv::PLAYER_TOSPEED);
    strat_speed_to(&mut g.objs.aliens[i], tospeed, 2);

    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        let al = g.objs.aliens[i];
        g.vars.set_sv_i16(sv::PVIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::PVIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::PVIEWPOSZ, al.worldz);
    }

    // PSTRATS.ASM:1676 — bgsscrollZ ← viewposz (last getview camera Z).
    let z = g.vars.sv_i16(sv::VIEWPOSZ);
    g.vars.set_sv_i16(sv::BGSSCROLLZ, z);
}

/// C `update_viewxy_for_mode` (strat_player.c:682).
fn update_viewxy_for_mode(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    if g.vars.game_mode == SPACE_MODE {
        if g.vars.player_view_mode == PlayerViewMode::Cockpit {
            g.vars.set_sv_i16(sv::PVIEWPOSX, al.worldx);
            g.vars.set_sv_i16(sv::PVIEWPOSY, al.worldy);
            return;
        }

        g.vars.set_sv_i16(sv::PVIEWPOSX, strat_perc75(al.worldx));
        g.vars.set_sv_i16(
            sv::PVIEWPOSY,
            strat_perc62(al.worldy.wrapping_sub(view_cy)).wrapping_add(view_cy),
        );
        return;
    }

    g.vars.set_sv_i16(sv::PVIEWPOSX, strat_perc87(al.worldx));
    g.vars.set_sv_i16(
        sv::PVIEWPOSY,
        strat_perc75(al.worldy.wrapping_sub(view_cy)).wrapping_add(view_cy),
    );
}

// ============================================================
// Strat_Player / Strat_SpawnPlayer (strat_player.c:699-785)
// ============================================================

/// C `Strat_Player` — the main per-tick player strategy
/// (playeronplanet_strat family entry).
pub fn strat_player(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].hp == 0 {
        playerdead_istrat(g, idx);
        return;
    }

    if g.vars.gameflags & GF_PLAYERDYING != 0 || g.vars.gameflags & GF_PLAYERDEAD != 0 {
        let dead_id = sid(g, K_PLAYERDEAD_STRAT);
        if g.objs.aliens[idx as usize].stratptr == Some(dead_id) {
            playerdead_strat(g, idx);
        }
        return;
    }

    boost_brake_update(g, idx);

    // The ROM picks the do_player_* velocity handler by the active player STRAT
    // pointer — each mode's strat hard-wires exactly one handler (PSTRATS.ASM
    // do_player_*), there is no runtime flag test. It is NOT selected by the
    // pfm_diefall/pfm_dieYrot bits: those are SET during NORMAL planet/water/
    // undergnd flight (planet_flymode, STRATEQU.INC:566) as mode *capabilities*
    // (death-anim style), not a live "dying" flag. Dying is handled earlier by
    // playerdead_strat, which returns before reaching here. game_mode is the
    // reachable proxy for the strat identity:
    //   SPACE_MODE           -> playerinspace              -> do_player_limitX
    //   else (planet/water)  -> playeronplanet/playeronwater -> do_player_Yvel125
    // (undergnd/tunnel -> do_playerYvelD2 and onbridge -> do_player_bridge are
    //  driven by their own dedicated strats, e.g. playeronbridge_strat.)
    if g.vars.game_mode == SPACE_MODE {
        do_player_limit_x(g, idx);
    } else {
        do_player_yvel125(g, idx);
    }

    update_viewxy_for_mode(g, idx);
    viewmove_srou(g, idx);
}

/// C `Strat_SpawnPlayer` (strat_player.c:725) — spawn the player alien and
/// reset all player globals.
pub fn strat_spawn_player(g: &mut Game) -> Option<u16> {
    let player_id = sid(g, K_PLAYER);
    let coll_id = sid(g, K_PLAYERCOLL);
    let exp_id = sid(g, K_PLAYERDEAD_INIT);

    let idx = g.objs.alloc()?;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SHAPE_ARWING;
        al.stratptr = Some(player_id);
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.vel = MED_PSPEED as u8;
        al.hp = PLAYER_BODY_HP;
        al.ap = 0;
        al.type_ = 0;
        al.sflags = ASF_SHADOW;
        al.sflags4 = ASF4_PLAYEROBJ;
        al.collstratptr = Some(coll_id);
        al.expstratptr = Some(exp_id);
        al.endcollstratptr = None;
        al.animframe = 0xFF;
        al.colframe = 0xFF;
    }

    let v = &mut g.vars;
    v.set_sv_i16(sv::PLROTX, 0);
    v.set_sv_i16(sv::PLROTY, 0);
    v.set_sv_i16(sv::PLROTZ, 0);

    v.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
    v.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
    v.set_sv_i16(sv::PLAYER_SPEED, MED_PSPEED);
    v.playervel_z = MED_PSPEED;
    v.pviewvelz = MED_PSPEED;
    v.set_sv_i16(sv::PVIEWPOSX, 0);
    v.set_sv_i16(sv::PVIEWPOSY, 0);
    v.set_sv_i16(sv::PVIEWPOSZ, 0);

    // A fresh stage must not inherit a fixed/look-at camera installed by the
    // previous clear sequence.  ROM playercred_Istrat resets this camera block
    // before handing control to the stage player strategy (PSTRATS.ASM:577-593).
    // The Rust shell preserves GameVars across stages, so leaving these slots
    // untouched freezes viewpos at the preceding cutscene and eventually makes
    // live objects cross the signed-Z cull seam while still ahead of the ship.
    v.set_sv_i16(sv::VIEWPOSX, 0);
    v.set_sv_i16(sv::VIEWPOSY, 0);
    v.set_sv_i16(sv::VIEWPOSZ, 0);
    v.set_sv_i16(sv::OUTVX, 0);
    v.set_sv_i16(sv::OUTVY, 0);
    v.set_sv_i16(sv::OUTVZ, 0);
    v.set_sv_i16(sv::OUTDIST, 0);
    v.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    v.set_sv_i16(sv::VIEWTOOBJ, 0);

    v.set_sv_i16(sv::PLAYER_TURNROT, 0);
    v.set_sv_u8(sv::PLAYER_ZTILT, 0);
    v.set_sv_i16(sv::PLAYER_ZSHAKE, 0);
    v.set_sv_u8(sv::PLAYER_ZSTRATADD, 0);
    v.set_sv_u8(sv::PLAYER_ROLLZVEL, 0);
    v.set_sv_u8(sv::PLAYER_ROLLZOFF, 0);
    v.set_sv_u8(sv::PLAYER_ROLLDELAY, 0);
    v.set_sv_u8(sv::PLAYER_NOCTRLCNT, 0);
    v.set_sv_i16(sv::VIEWCY, 0);

    v.set_sv_u8(sv::NUMPLASERS, 0);

    // Seed the onplanet movement box (planet_minX/maxX, STRATEQU.INC:559-564).
    // The C map VM sets this via `mapplayermode onplanet` (LEVEL1_1.ASM:66),
    // an opcode not yet ported, and the exit-base -> playeronplanet_init
    // handoff that would otherwise set it isn't reliably reached. Without a
    // valid box, playerlimit_x_srou clamps the ship to [0,0] every frame and
    // steering does nothing.
    v.set_sv_i16(sv::MINPMOVEX, -500);
    v.set_sv_i16(sv::MAXPMOVEX, 500);
    v.minpmove_y = -210 - 45;
    v.set_sv_i16(sv::MAXPMOVEY, PLAYERB_YSTOP);

    // A stage starts on the ordinary interactive-control path.
    v.set_sv_i8(sv::STAYBLACK, STAY_BLACK_INACTIVE);

    // NOTE: outdist stays 0 at spawn. The ROM assigns OUTVIEWDIST(120) only
    // to `viewdist` (mapplayeroutdist MAPMACS.INC:1033, changeviewmode_l
    // GSTRATS.ASM:3090) — the parallel `outdist` writes are commented out. The
    // only live outdist writers are the intro fly-in (+3/frame from 0), spfm
    // inside (=60), boss cam, and the bridge-clear pull-out (chase 500), which
    // ramps from 0. Seeding 120 here was a port bug (audit_player oracle).
    v.set_sv_u8(sv::FIRECNT, 3);
    v.set_sv_u8(sv::FIREDELAY, 1);
    v.set_sv_u8(sv::SPECIALDELAY, 1);
    v.set_sv_u8(sv::PNUMHITS, 0);
    v.set_sv_u8(sv::ARROWS, 0);
    v.set_sv_u8(sv::NOMAXBG2YSCROLL, 0);

    v.pshipflags3 &= !(PSF3_INTUNNEL | PSF3_FORCEBRAKE | PSF3_NOCOLLISIONS);
    v.pshipflags3 |= PSF3_ENGINESND;
    v.internal_playpt = idx as i16;

    // ROM select_ship_l #pshipnum_norm — fills playershape{,L,R,LR}.
    select_ship(g, PSHIPNUM_NORM);
    set_y_player_shape(g, idx, PSHIPNUM_NORM);

    Some(idx)
}

/// Select the source-defined initial strategy for an already spawned player.
///
/// Keeping this separate from [`strat_spawn_player`] lets the native shell
/// reproduce `initgame_l`: the background creates the base player first, then
/// the map's opening declaration changes its behavior after the transfer-bound
/// level setup has completed.
pub fn initialize_player_for_map(g: &mut Game, map_id: u32, idx: u16) {
    if let Some(view) = sf_map::catalog::opening_player_view(map_id) {
        g.vars.player_view_mode = view.mode;
        g.vars.player_view_options = view.options;
        g.apply_player_view_mode(idx);
    }
    use sf_map::catalog::OpeningPlayerStrategy as Strategy;
    match sf_map::catalog::opening_player_strategy(map_id) {
        Some(Strategy::HangarLaunch) => strat_player_opening_init(g, idx),
        Some(Strategy::InteriorSpaceFlyIn) => player_inside_space_flyin_istrat(g, idx),
        Some(Strategy::HyperspaceExit) => player_warp_out_istrat(g, idx),
        Some(Strategy::PlanetFlyIn) => player_planet_flyin_istrat(g, idx),
        Some(Strategy::GroundDive) => player_divegnd_istrat(g, idx),
        Some(Strategy::PlanetFlight) => set_player_on_planet(g, idx),
        Some(Strategy::SpaceFlyIn) => player_space_flyin_istrat(g, idx),
        Some(Strategy::ColonyFlyIn) => player_colony_flyin_istrat(g, idx),
        Some(Strategy::UndergroundFlight) => set_player_undergnd(g, idx),
        Some(Strategy::LongTunnelExit) => set_player_in_ltexit(g, idx),
        Some(Strategy::ContinuePresentation) => queue_player_on_cont_istrat(g, idx),
        Some(Strategy::PassivePresentation) => player_cred_istrat(g, idx),
        None => {}
    }
}

/// Spawn and select the source-defined initial player strategy for a loaded
/// map. Presentation paths use this convenience entry point; gameplay startup
/// calls the two typed stages separately through the shell.
pub fn strat_spawn_player_for_map(g: &mut Game, map_id: u32) -> Option<u16> {
    let idx = strat_spawn_player(g)?;
    initialize_player_for_map(g, map_id, idx);
    Some(idx)
}

// ============================================================
// playerExitBase (PISTRATS.ASM:620-720, strat_player.c:787-1054)
// ============================================================

/// C `playerflymode_exitbase` (s_playerfly_mode exitbase).
fn playerflymode_exitbase(g: &mut Game, idx: u16) {
    let v = &mut g.vars;
    v.set_sv_i16(sv::VIEWCY, EXITBASE_VIEWCY);
    v.set_sv_i16(sv::MINPMOVEX, EXITBASE_MINX);
    v.set_sv_i16(sv::MAXPMOVEX, EXITBASE_MAXX);
    v.set_sv_i16(sv::MINMMOVEX, EXITBASE_MMINX);
    v.set_sv_i16(sv::MAXMMOVEX, EXITBASE_MMAXX);
    v.set_sv_i16(sv::MAXMMOVEY, EXITBASE_MMAXY);
    v.set_sv_i16(sv::MINPWMOVEY, EXITBASE_MINY);
    v.set_sv_i16(sv::MAXPWMOVEY, EXITBASE_MAXY);
    v.minpmove_y = EXITBASE_MINY;
    v.set_sv_i16(sv::MAXPMOVEY, EXITBASE_MAXY + PLAYERB_YSTOP);
    v.playerflymode = PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS;
    v.set_sv_u8(sv::PMOVELIMITAND, PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM);
    v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_BOTTOM);
    v.gameflags |= GF_VIEWROT; // exitbase_gameflagsON
                               // exitbase_macro
    v.pshipflags2 &= !PSF2_NOSPARK;
    v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
    v.pstratflags |= PSTF_NOTDIE;
    g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    g.vars.pshipflags3 &= !PSF3_INTUNNEL;
}

/// C `Strat_PlayerExitBase` (set_playerExitBase_l, PISTRATS.ASM:621-655).
/// Invoked by the MAP_CB_SET_PLAYER_EXITBASE_L callback (the sf-game hook
/// `player_exit_base`) after the scramble opening.
pub fn strat_player_exit_base(g: &mut Game, idx: u16) {
    // (s_set_var W,lastplayZ,#0 and jsl clearmap_l are performed by the
    //  world map callback before this is invoked.)

    // s_set_var W,Bg2Yscroll,#232
    g.vars.set_sv_i16(sv::BG2YSCROLL, 232);

    // s_or_var B,gameflags,#gf_nozremove
    g.vars.gameflags |= GF_NOZREMOVE;

    // s_playerfly_mode exitbase
    playerflymode_exitbase(g, idx);

    // s_set_alptrs x,playerExitBasewait_strat,playercoll_Istrat,playerdead_Istrat
    let wait_id = sid(g, K_EXITBASE_WAIT);
    let coll_id = sid(g, K_PLAYERCOLL);
    let exp_id = sid(g, K_PLAYERDEAD_INIT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(wait_id);
        al.collstratptr = Some(coll_id);
        al.expstratptr = Some(exp_id);
    }

    // s_and_var B,gameflags,#~gf_viewrot
    g.vars.gameflags &= !GF_VIEWROT;
    // s_playerctrl off
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;

    // Fixed camera in front of the base, aimed at the (player) ship.
    g.vars
        .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    g.vars.set_sv_i16(sv::VIEWTOOBJ, idx as i16);
    g.vars.set_sv_i16(sv::VIEWPOSX, -400);
    g.vars.set_sv_i16(sv::VIEWPOSY, -145);
    g.vars.set_sv_i16(sv::VIEWPOSZ, 1000);

    // s_set_var B,psvar_byte1,#15+((1000/pexitbasespeed)*2)
    g.vars
        .set_sv_u8(sv::PSVAR_BYTE1, (15 + (1000 / PEXITBASE_SPEED) * 2) as u8);
    // s_set_var B,player_medspeed,#0 — hold the ship in the hangar
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, 0);

    // s_set_pos x,#-27<<mybase_scale,#-39<<mybase_scale,#-200
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = -27 << MYBASE_SCALE;
        al.worldy = -39 << MYBASE_SCALE;
        al.worldz = -200;
        // s_set_alsflag x,invisible — hidden inside the hangar
        al.sflags |= ASF_INVISIBLE;
    }

    // s_set_var B,nomaxbg2Yscroll,#1
    g.vars.set_sv_u8(sv::NOMAXBG2YSCROLL, 1);

    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
}

/// C `playerExitBasewait_strat` (PISTRATS.ASM:658-664).
fn player_exitbase_wait_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = -27 << MYBASE_SCALE;
        al.worldy = -39 << MYBASE_SCALE;
        al.worldz = -200;
    }

    // s_decbne_var B,psvar_byte1,playeronplanet_strat
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
    if b1 != 0 {
        strat_player(g, idx);
        return;
    }
    player_exitbase_go_init(g, idx);
}

/// C `playerExitBaseGo_init` (PISTRATS.ASM:667-675).
fn player_exitbase_go_init(g: &mut Game, idx: u16) {
    // s_set_var B,psvar_byte1,#((80-pexitbasespeed)*2)
    g.vars
        .set_sv_u8(sv::PSVAR_BYTE1, ((80 - PEXITBASE_SPEED) * 2) as u8);
    // s_set_strat x,playerExitBaseGo_strat
    let go_id = sid(g, K_EXITBASE_GO);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(go_id);
        // s_clr_alsflag x,invisible — the ship emerges from the hangar
        al.sflags &= !ASF_INVISIBLE;
        // set_sound2 x,#0
        al.snd2 = 0;
        // s_set_alvar B,x,al_snd1,#%10110001 — engine roar, panned
        al.snd1 = 0xB1;
    }
    // s_set_var B,stagecnt,#50
    g.vars.stagecnt = 50;
    // s_set_var B,psvar_byte2,#0
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, 0);
    // ASM falls through into playerExitBaseGo_strat
    player_exitbase_go_strat(g, idx);
}

/// C `playerExitBaseGo_strat` (PISTRATS.ASM:676-693).
fn player_exitbase_go_strat(g: &mut Game, idx: u16) {
    // Engine sound pan/attenuation steps as the ship pulls away
    let b2 = g.vars.sv_u8(sv::PSVAR_BYTE2).wrapping_add(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, b2);
    if b2 == 12 {
        g.objs.aliens[idx as usize].snd1 = 0x91; // %10010001
    }
    if b2 == 20 {
        g.objs.aliens[idx as usize].snd1 = 0x81; // %10000001
    }
    if b2 == 30 {
        g.objs.aliens[idx as usize].snd1 = 0xA1; // %10100001
    }

    // s_set_var B,player_medspeed,#pexitbasespeed — launch speed
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, PEXITBASE_SPEED as u8);

    // s_decbne_var B,psvar_byte1,playeronplanet_strat
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
    if b1 != 0 {
        strat_player(g, idx);
        return;
    }
    player_exitbase_follow_init(g, idx);
}

/// C `playerExitBaseFollow_init` (PISTRATS.ASM:695-697).
fn player_exitbase_follow_init(g: &mut Game, idx: u16) {
    let follow_id = sid(g, K_EXITBASE_FOLLOW);
    g.objs.aliens[idx as usize].stratptr = Some(follow_id);
    // s_or_var B,pshipflags3,#psf3_enginesnd
    g.vars.pshipflags3 |= PSF3_ENGINESND;
    player_exitbase_follow_strat(g, idx);
}

/// C `playeronplanet_init` (playeronplanet_Istrat, PSTRATS.ASM:751-760) —
/// resume normal flight.
pub fn set_player_on_planet(g: &mut Game, idx: u16) {
    playeronplanet_init(g, idx);
}

fn playeronplanet_init(g: &mut Game, idx: u16) {
    // s_playerctrl on
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);

    // Planet is not SPACE_MODE — map CB SET_PLAYER_ONPLANET_L sets game_mode=0.
    // Without this, exit-base → playeronplanet_init left SPACE_MODE from spawn
    // and `strat_player` wrongly picked do_player_limitX (High #3).
    g.vars.game_mode = 0;

    // s_playerfly_mode planet (STRATEQU.INC:558-578)
    let v = &mut g.vars;
    v.set_sv_i16(sv::VIEWCY, -50); // planet_viewCY
    v.set_sv_i16(sv::MINPMOVEX, -500);
    v.set_sv_i16(sv::MAXPMOVEX, 500);
    v.set_sv_i16(sv::MINMMOVEX, -500);
    v.set_sv_i16(sv::MAXMMOVEX, 500);
    v.set_sv_i16(sv::MAXMMOVEY, 0);
    v.set_sv_i16(sv::MINPWMOVEY, -210 - 45);
    v.set_sv_i16(sv::MAXPWMOVEY, 0);
    v.minpmove_y = -210 - 45;
    v.set_sv_i16(sv::MAXPMOVEY, PLAYERB_YSTOP);
    v.playerflymode = PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS | PFM_WOBBLE;
    v.set_sv_u8(sv::PMOVELIMITAND, PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM);
    v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_BOTTOM);
    v.gameflags |= GF_VIEWROT; // planet_gameflagsON
                               // planet_macro
    v.pshipflags2 &= !PSF2_NOSPARK;
    v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
    g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    g.vars.pshipflags3 &= !PSF3_INTUNNEL;

    // s_set_alptrs x,playeronplanet_strat,playercoll_Istrat,playerdead_Istrat
    let player_id = sid(g, K_PLAYER);
    let coll_id = sid(g, K_PLAYERCOLL);
    let exp_id = sid(g, K_PLAYERDEAD_INIT);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(player_id);
    al.collstratptr = Some(coll_id);
    al.expstratptr = Some(exp_id);
}

/// C `playerExitBaseFollow_strat` (PISTRATS.ASM:698-720).
fn player_exitbase_follow_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;
    let view_cy = g.vars.sv_i16(sv::VIEWCY);

    // Center the ship: s_achase_alvar W,x,al_worldx,#0,2 / al_worldy,viewcy,2
    {
        let al = &mut g.objs.aliens[i];
        al.worldx = strat_chase_proportional(al.worldx, 0, 2);
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 2);
    }

    // Camera chases in behind the ship.
    let vx = strat_chase_proportional(g.vars.sv_i16(sv::VIEWPOSX), 0, 2);
    let vy = strat_chase_proportional(g.vars.sv_i16(sv::VIEWPOSY), view_cy, 2);
    let mut vz = strat_chase_proportional(g.vars.sv_i16(sv::VIEWPOSZ), g.vars.player_posz, 3);
    g.vars.set_sv_i16(sv::VIEWPOSX, vx);
    g.vars.set_sv_i16(sv::VIEWPOSY, vy);

    // s_varadd_alvar W,x,viewposz,al_vz — camera keeps pace with the ship
    vz = vz.wrapping_add(g.objs.aliens[i].vz);
    g.vars.set_sv_i16(sv::VIEWPOSZ, vz);

    // s_jmp_Zdistmore x,y(viewpt),#outviewdist+pexitbasespeed,playeronplanet_strat
    {
        let mut zdist = g.objs.aliens[i].worldz.wrapping_sub(vz);
        if zdist < 0 {
            zdist = zdist.wrapping_neg();
        }
        if zdist > OUTVIEWDIST + PEXITBASE_SPEED {
            strat_player(g, idx);
            return;
        }
    }

    // Camera caught up — hand off to the normal follow camera.
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    g.vars.gameflags &= !GF_NOZREMOVE;
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);

    // s_make_obj #myship_4 — the 4th wingman catches up from behind
    if let Some(w) = strat_make_obj(g, SHAPE_MYSHIP_4) {
        let (self_z, self_roty) = {
            let al = &g.objs.aliens[i];
            (al.worldz, al.roty)
        };
        let wal = &mut g.objs.aliens[w as usize];
        wal.sflags |= ASF_COLLDISABLE;
        wal.worldx = -50;
        wal.worldy = -100;
        wal.worldz = self_z.wrapping_sub(200);
        wal.roty = self_roty;
        friendstart3_istrat(g, w);
    }

    // Stop the hangar engine channel.
    g.objs.aliens[i].snd1 = 0;

    // s_set_var B,nomaxbg2Yscroll,#0
    g.vars.set_sv_u8(sv::NOMAXBG2YSCROLL, 0);

    // s_jmp playeronplanet_Istrat
    playeronplanet_init(g, idx);
    strat_player(g, idx);
}

// ============================================================
// friendstart3 (GISTRATS.ASM:275-305) — 4th wingman catch-up.
// ============================================================

/// C `friendstart3go_strat`.
fn friendstart3go_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_jmp_objinfront y(player),x,.nch — only steer/boost while the
    // player is not yet ahead of the wingman.
    let player_infront = g
        .objs
        .player()
        .map(|p| p.worldz > g.objs.aliens[i].worldz)
        .unwrap_or(false);
    if !player_infront {
        // One-shot boost effect (sflag1 latch)
        if g.objs.aliens[i].sflags2 & ASF2_SFLAG1 == 0 {
            g.objs.aliens[i].sflags2 |= ASF2_SFLAG1;
            g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
            if g.vars.sv_u8(sv::BOOSTZOFF) == 0 {
                set_boost_zoff(g, -30);
            }
            let _ = boost_sprite(g, None);
            g.hooks.play_se(0x32);
        }
        // s_achase_alvar B,x,al_rotz,#0,4
        let rotz = g.objs.aliens[i].rotz as i8;
        g.objs.aliens[i].rotz = strat_chase_proportional(rotz as i16, 0, 4) as u8;
        // s_jmp_notdelay 2,.nch — every 4th frame
        if g.vars.gameframe & 3 == 0 {
            // s_achase_alvar B,x,al_rotx,#-deg90,4
            let rotx = g.objs.aliens[i].rotx as i8;
            g.objs.aliens[i].rotx =
                strat_chase_proportional(rotx as i16, (-(DEG90 as i8)) as i16, 4) as u8;
        }
    }

    // s_gen_3dvecs / s_add_vecs2pos / s_add_playerZ
    strat_gen_vecs_3d(&mut g.objs.aliens[i]);
    strat_apply_velocity(&mut g.objs.aliens[i]);
    let pviewvelz = g.vars.pviewvelz;
    g.objs.aliens[i].worldz = g.objs.aliens[i].worldz.wrapping_add(pviewvelz);

    // s_dec_lifecnt x
    if g.objs.aliens[i].count > 0 {
        g.objs.aliens[i].count -= 1;
    } else {
        strat_remove_obj(g);
    }
}

/// C `friendstart3_Istrat`.
fn friendstart3_istrat(g: &mut Game, idx: u16) {
    let go_id = sid(g, K_FRIENDSTART3GO);
    let al = &mut g.objs.aliens[idx as usize];
    // s_setnoremove_behind x
    al.type_ &= !ATZREMOVE;
    // s_set_alsflag x,colldisable
    al.sflags |= ASF_COLLDISABLE;
    // s_set_strat x,friendstart3go_strat
    al.stratptr = Some(go_id);
    // s_set_lifecnt x,#70
    al.count = 70;
    // s_set_speed x,#30
    al.vel = 30;
    // s_set_alvar B,x,al_rotz,#-deg90
    al.rotz = (-(DEG90 as i8)) as u8;
}

// ============================================================
// PCSTRATS.ASM — player clear / escape strategies
// ============================================================

/// C `centoutrots` (PCSTRATS.ASM:1151-1154).
fn centoutrots(g: &mut Game) {
    let vx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 3);
    let vy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 3);
    g.vars.set_sv_i16(sv::OUTVX, vx);
    g.vars.set_sv_i16(sv::OUTVY, vy);
}

/// ROM `playertomiddle1_srou_l` (PCSTRATS.ASM): chase player X→0, Y→ViewCY, shift 1.
pub fn player_to_middle1(g: &mut Game, player: u16) {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    let al = &mut g.objs.aliens[player as usize];
    al.worldy = strat_chase_proportional(al.worldy, view_cy, 1);
    al.worldx = strat_chase_proportional(al.worldx, 0, 1);
}

/// ROM `playertomiddle4_srou_l` (PCSTRATS.ASM): same as middle1 with shift 4.
pub fn player_to_middle4(g: &mut Game, player: u16) {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    let al = &mut g.objs.aliens[player as usize];
    al.worldy = strat_chase_proportional(al.worldy, view_cy, 4);
    al.worldx = strat_chase_proportional(al.worldx, 0, 4);
}

/// ROM `playertoCslow_Istrat`: center the player, then run the strategy saved
/// by `s_push_stratptr`.
fn player_to_cslow_tick(g: &mut Game, player: u16) {
    g.vars.viewdist = OUTVIEWDIST;
    player_to_middle4(g, player);
    if let Some(temp) = g.objs.aliens[player as usize].tempstratptr {
        g.call_strat(temp, player);
    }
}

/// ROM `set_playertoCslow_l` / `playertoCslow_Istrat` (PCSTRATS.ASM): push
/// the current strategy, disable control, and install the centering wrapper.
pub fn set_player_to_cslow(g: &mut Game, player: u16) {
    let tick = ea_sid(g, player_to_cslow_tick);
    let old = g.objs.aliens[player as usize].stratptr;
    g.objs.aliens[player as usize].tempstratptr = old;
    g.objs.aliens[player as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.viewdist = OUTVIEWDIST;
    player_to_cslow_tick(g, player);
}

// ============================================================
// Tunnel / tunnel-exit SET_PLAYER* (PSTRATS.ASM)
// ============================================================

/// ROM `playerintunnel_strat`: chase outv→0, YvelD2, pviewposx≈0.875·x, fixed Y.
pub fn player_in_tunnel_strat(g: &mut Game, idx: u16) {
    let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 2);
    let ovy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 2);
    g.vars.set_sv_i16(sv::OUTVX, ovx);
    g.vars.set_sv_i16(sv::OUTVY, ovy);

    do_player_yvel_d2(g, idx);

    let wx = g.objs.aliens[idx as usize].worldx;
    // Three signed halvings produce x/2 + x/4 + x/8 = 0.875x.
    let pview_x = (wx >> 1).wrapping_add(wx >> 2).wrapping_add(wx >> 3);
    g.vars.set_sv_i16(sv::PVIEWPOSX, pview_x);
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    g.vars.set_sv_i16(sv::PVIEWPOSY, view_cy);
    viewmove_srou(g, idx);
}

/// ROM `playerinTexit_strat`: chase worldy→viewcy then tunnel strat.
pub fn player_in_texit_strat(g: &mut Game, idx: u16) {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 2);
    }
    player_in_tunnel_strat(g, idx);
}

fn set_player_in_tunnel_mode(g: &mut Game, idx: u16, mode: TunnelFlyMode) {
    let tick = ea_sid(g, player_in_tunnel_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    apply_tunnel_fly_mode(g, idx, mode);
}

/// ROM `set_playerInStunnel_l` / `playerInStunnel_Istrat`.
pub fn set_player_in_stunnel(g: &mut Game, idx: u16) {
    set_player_in_tunnel_mode(g, idx, FLY_STUNNEL);
}

/// ROM `set_playerInMtunnel_l` / `playerInMtunnel_Istrat`.
pub fn set_player_in_mtunnel(g: &mut Game, idx: u16) {
    set_player_in_tunnel_mode(g, idx, FLY_MTUNNEL);
}

/// ROM `set_playerInLtunnel_l` / `playerInLtunnel_Istrat`.
pub fn set_player_in_ltunnel(g: &mut Game, idx: u16) {
    set_player_in_tunnel_mode(g, idx, FLY_LTUNNEL);
}

/// ROM `set_playerInSTexit_l`.
pub fn set_player_in_stexit(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_texit_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    apply_tunnel_fly_mode(g, idx, FLY_STEXIT);
    g.vars.pshipflags |= PSF_NOYCTRL;
}

/// ROM `set_playerInMTexit_l`.
pub fn set_player_in_mtexit(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_texit_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    apply_tunnel_fly_mode(g, idx, FLY_MTEXIT);
    g.vars.pshipflags |= PSF_NOYCTRL;
}

/// ROM `set_playerInLTexit_l`.
pub fn set_player_in_ltexit(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_texit_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    apply_tunnel_fly_mode(g, idx, FLY_LTEXIT);
    g.vars.pshipflags |= PSF_NOYCTRL;
}

// ============================================================
// Colony / nucleus SET_PLAYER* (PSTRATS.ASM / STRATEQU.INC)
// ============================================================

const BOSS8_SCALE: i32 = 3; // <<3
const NUCLEUS_VIEWCY: i16 = -60;
const NUCLEUS_MINX: i16 = (-110i32 << BOSS8_SCALE) as i16; // -880
const NUCLEUS_MAXX: i16 = (110i32 << BOSS8_SCALE) as i16; // 880
const NUCLEUS_MMINX: i16 = ((-110i32 << BOSS8_SCALE) - 1000) as i16;
const NUCLEUS_MMAXX: i16 = ((110i32 << BOSS8_SCALE) + 1000) as i16;

/// ROM `set_playerInColony_l` / colony fly-mode init.
pub fn set_player_in_colony(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_colony_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    {
        let v = &mut g.vars;
        v.set_sv_i16(sv::VIEWCY, -60);
        v.set_sv_i16(sv::MINPMOVEX, -170); // -120-50
        v.set_sv_i16(sv::MAXPMOVEX, 120);
        v.set_sv_i16(sv::MINMMOVEX, -5000);
        v.set_sv_i16(sv::MAXMMOVEX, 120);
        v.set_sv_i16(sv::MAXMMOVEY, 0);
        v.set_sv_i16(sv::MINPWMOVEY, -120);
        v.set_sv_i16(sv::MAXPWMOVEY, 5);
        v.minpmove_y = -120;
        v.set_sv_i16(sv::MAXPMOVEY, PLAYERB_YSTOP);
        v.playerflymode = PFM_DIEFALL | PFM_SHADOWS;
        v.set_sv_u8(sv::PMOVELIMITAND, PML_RWRIGHT | PML_BBOTTOM | PML_BTOP);
        v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_RIGHT | MB_LBOTTOM | MB_LTOP);
        v.gameflags &= !GF_VIEWROT;
        v.pstratflags &= !PSTF_NOVIEWMOVE;
        v.pshipflags3 |= PSF3_ENGINESND;
        // colony_macro
        v.pshipflags2 &= !PSF2_NOSPARK;
        v.set_sv_i16(sv::MISSBTOPLEFT, -140);
        v.set_sv_i16(sv::MISSBBOTLEFT, -140);
        v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
        v.pstratflags |= PSTF_FIRSTFRAMELCOL;
        v.pshipflags3 |= PSF3_INTUNNEL;
    }
    {
        let player = &mut g.objs.aliens[idx as usize];
        player.stratptr = Some(tick);
        player.collstratptr = Some(coll);
        player.expstratptr = Some(exp);
        player.sflags |= ASF_SHADOW;
    }
}

/// ROM `playerincolony_strat`.
pub fn player_in_colony_strat(g: &mut Game, idx: u16) {
    do_player_yvel_colony(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    g.vars.set_sv_i16(sv::PVIEWPOSX, strat_perc62(wx));
    g.vars.set_sv_i16(sv::PVIEWPOSY, -60); // Stunnel_ViewCY
    viewmove_srou(g, idx);
}

/// ROM `set_playerInNucleus_l` / nucleus fly-mode init.
pub fn set_player_in_nucleus(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_nucleus_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    {
        let v = &mut g.vars;
        v.set_sv_i16(sv::VIEWCY, NUCLEUS_VIEWCY);
        v.set_sv_i16(sv::MINPMOVEX, NUCLEUS_MINX);
        v.set_sv_i16(sv::MAXPMOVEX, NUCLEUS_MAXX);
        v.set_sv_i16(sv::MINMMOVEX, NUCLEUS_MMINX);
        v.set_sv_i16(sv::MAXMMOVEX, NUCLEUS_MMAXX);
        v.set_sv_i16(sv::MAXMMOVEY, 0);
        v.set_sv_i16(sv::MINPWMOVEY, -120);
        v.set_sv_i16(sv::MAXPWMOVEY, 5);
        v.minpmove_y = -120;
        v.set_sv_i16(sv::MAXPMOVEY, PLAYERB_YSTOP);
        v.playerflymode = PFM_SHADOWS;
        v.set_sv_u8(sv::PMOVELIMITAND, PML_LWLEFT | PML_RWRIGHT);
        v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_RIGHT | MB_LEFT);
        v.gameflags &= !GF_VIEWROT;
        v.pstratflags &= !PSTF_NOVIEWMOVE;
        v.pshipflags3 |= PSF3_ENGINESND;
        // nucleus_macro
        v.pshipflags2 &= !PSF2_NOSPARK;
        v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
        v.pshipflags3 &= !PSF3_INTUNNEL;
    }
    {
        let player = &mut g.objs.aliens[idx as usize];
        player.stratptr = Some(tick);
        player.collstratptr = Some(coll);
        player.expstratptr = Some(exp);
        player.sflags |= ASF_SHADOW;
        player.worldy = NUCLEUS_VIEWCY;
    }
}

/// ROM `playerinnucleus_strat`.
pub fn player_in_nucleus_strat(g: &mut Game, idx: u16) {
    // player_posx >> 7 added into player_Ztilt (byte).
    let tilt_add = (g.vars.player_posx >> 7) as i8;
    let ztilt = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
    g.vars
        .set_sv_u8(sv::PLAYER_ZTILT, ztilt.wrapping_add(tilt_add) as u8);

    do_player_yvel_d2(g, idx);

    let wx = g.objs.aliens[idx as usize].worldx;
    let xoff = g.vars.sv_i16(sv::VIEWPOSXOFF);
    let yoff = g.vars.sv_i16(sv::VIEWPOSYOFF);
    g.vars
        .set_sv_i16(sv::PVIEWPOSX, strat_perc93(wx).wrapping_add(xoff));
    g.vars
        .set_sv_i16(sv::PVIEWPOSY, NUCLEUS_VIEWCY.wrapping_add(yoff));
    viewmove_srou(g, idx);
}

/// ROM `set_playerClearColony_l` / `playerclearcolony_Istrat` leaf:
/// disable ctrl, dupplayer, assign pshipcolony cutscene strat.
pub fn set_player_clear_colony(g: &mut Game, idx: u16) -> Option<u16> {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let tick = ea_sid(g, player_clear_colony_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    let dup = dupplayer(g, idx)?;
    pshipcolony_istrat(g, dup);
    Some(dup)
}

/// ROM `playerclearcolony_strat` / `playerwashent_strat` body (Z scroll only).
pub fn player_clear_colony_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz =
        g.objs.aliens[idx as usize].worldz.wrapping_add(MED_PSPEED);
}

/// ROM `set_playerwashent_l` — dup + pshipwashent cutscene.
pub fn set_player_washent(g: &mut Game, idx: u16) -> Option<u16> {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let tick = ea_sid(g, player_clear_colony_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    let dup = dupplayer(g, idx)?;
    pshipwashent_istrat(g, dup);
    Some(dup)
}

/// Strategy-shaped entry used by the map/background callback registry.
pub fn player_washent_istrat(g: &mut Game, idx: u16) {
    let _ = set_player_washent(g, idx);
}

/// Strategy-shaped entry used by the bg_2_6b background callback bridge.
pub fn player_clear_colony_istrat(g: &mut Game, idx: u16) {
    let _ = set_player_clear_colony(g, idx);
}

// ============================================================
// ClearShip / ClearShip2 / playernull (PCSTRATS.ASM / PSTRATS.ASM)
// ============================================================

/// ROM `s_playerfly_mode ClearShip` + `ClearShip_macro` (STRATEQU.INC).
fn apply_clearship_fly_mode(g: &mut Game, idx: u16) {
    // Preserve INSEQ + ctrl flags — s_playerfly_mode does not touch them
    // (unlike planet-style which re-enables control / clears INSEQ|NOTDIE).
    let keep_inseq = g.vars.pstratflags & PSTF_INSEQ;
    let keep_ctrl = g.vars.pshipflags & (PSF_NOCTRL | PSF_NOFIRE);
    apply_planet_style_fly_mode(
        g,
        idx,
        SPACE_VIEWCY, // ClearShip_viewCY = -60
        -10000,
        10000,
        -10000,
        10000,
        -10000,
        10000,
        10000,
        0, // ClearShip_flymode
        0,
        0,
        false, // gameflagsOFF = gf_viewrot
        false, // no shadow
        true,  // clear intunnel (ClearShip_macro)
    );
    // ClearShip_macro: s_or_var B,pstratflags,#pstf_notdie
    g.vars.pstratflags |= PSTF_NOTDIE | keep_inseq;
    g.vars.pshipflags |= keep_ctrl;
}

/// Shared ClearShip Icont tail (PCSTRATS.ASM:337-349).
fn player_clear_ship_icont(g: &mut Game, idx: u16) {
    g.vars.shared.background_scroll_x = 0;
    apply_clearship_fly_mode(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_STAGEDONE;
    g.vars.pstratflags |= PSTF_NOVDISTC;
    g.objs.aliens[idx as usize].sbyte3 = 100;
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG4;
}

/// ROM `set_playerClearShip_l` / `playerClearShip_Istrat`.
pub fn set_player_clear_ship(g: &mut Game, idx: u16) {
    player_clear_ship_istrat(g, idx);
}

/// ROM `playerClearShip_Istrat` (PCSTRATS.ASM:319-349).
pub fn player_clear_ship_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let pview_z = g.vars.sv_i16(sv::PVIEWPOSZ);
    let spawn_z = pview_z.wrapping_sub(200);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = SPACE_VIEWCY.wrapping_sub(40);
        al.worldz = spawn_z;
        al.vel = MAX_PSPEED as u8;
        al.stratstate = 0;
    }
    g.vars.pstratflags |= PSTF_INSEQ;
    player_clear_ship_icont(g, idx);
}

/// ROM `playerClearShip_strat` (PCSTRATS.ASM:352-449).
pub fn player_clear_ship_strat(g: &mut Game, idx: u16) {
    // Every frame: bg2scroll=#232, lastrot=0
    g.vars.shared.background_scroll = 232;
    g.vars.shared.last_rotation = 0;

    // s_decbne_alvar B,x,al_sbyte3,.nturn
    let sb = g.objs.aliens[idx as usize].sbyte3.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte3 = sb;
    if sb == 0 {
        g.objs.aliens[idx as usize].sbyte3 = 1;
        let mut scroll = g.vars.shared.background_scroll_x as u8;
        let mut do_maxsc = scroll == 254;
        if !do_maxsc {
            scroll = scroll.wrapping_add(2);
            g.vars.shared.background_scroll_x = i16::from(scroll);
            if scroll == 254 - 32 {
                // Boost FX at scroll==222, then fall into .maxsc
                g.vars.pshipflags3 &= !PSF3_ENGINESND;
                g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
                g.hooks.play_se(0x32);
                g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG4;
                do_maxsc = true;
            } else if scroll > 254 - 32 {
                // BCC fail after cmp #222 → fall through .maxsc
                do_maxsc = true;
            }
            // scroll < 222 → .donesc (skip maxsc)
        }
        if do_maxsc {
            let plroty = g.vars.sv_i16(sv::PLROTY).wrapping_add(256 / 32);
            g.vars.set_sv_i16(sv::PLROTY, plroty);
            let plrotx = g.vars.sv_i16(sv::PLROTX).wrapping_sub(256);
            g.vars.set_sv_i16(sv::PLROTX, plrotx);
            g.vars.gameflags |= GF_STAGEDONE;
            g.objs.aliens[idx as usize].worldz =
                g.objs.aliens[idx as usize].worldz.wrapping_add(100);
            g.objs.aliens[idx as usize].worldy =
                g.objs.aliens[idx as usize].worldy.wrapping_add(-10);
            g.objs.aliens[idx as usize].vel = 120;
        }
        // .donesc: always in turn path
        let ztilt = g.vars.sv_u8(sv::PLAYER_ZTILT).wrapping_sub(DEG45 / 8);
        g.vars.set_sv_u8(sv::PLAYER_ZTILT, ztilt);
        let outvy = g.vars.sv_i16(sv::OUTVY);
        if (outvy >> 8) as i8 >= -(DEG11 as i8) {
            g.vars.set_sv_i16(sv::OUTVY, outvy.wrapping_sub(32));
        }
    } else {
        // .nturn: speedto med+2 every 4 frames
        if notdelay(g, 2) {
            strat_speed_to(&mut g.objs.aliens[idx as usize], (MED_PSPEED + 2) as u8, 1);
        }
    }

    let od = g.vars.sv_i16(sv::OUTDIST);
    if od >= 50 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_sub(1));
    }

    playermove_srou(g, idx);

    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(MED_PSPEED);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);

    let wz = g.objs.aliens[idx as usize].worldz;
    let pview_z = g.vars.sv_i16(sv::PVIEWPOSZ);
    if (wz as i32).wrapping_sub(pview_z as i32) >= 4000 {
        g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    }

    let wx = g.objs.aliens[idx as usize].worldx;
    let pview_x = (wx >> 1).wrapping_add(wx >> 2).wrapping_add(wx >> 3);
    g.vars.set_sv_i16(sv::PVIEWPOSX, pview_x);
    let pview_y = strat_chase_proportional(g.vars.sv_i16(sv::PVIEWPOSY), SPACE_VIEWCY, 4);
    g.vars.set_sv_i16(sv::PVIEWPOSY, pview_y);

    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `set_playerClearShip2_l` / `playerClearShip2_Istrat`.
pub fn set_player_clear_ship2(g: &mut Game, idx: u16) {
    player_clear_ship2_istrat(g, idx);
}

/// ROM `playerClearShip2_Istrat` (PCSTRATS.ASM:462-485).
pub fn player_clear_ship2_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_ship2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars
        .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    g.vars.gameflags |= GF_NOZREMOVE;
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 60 + 35);
    g.vars.psvar_word1 = g.objs.aliens[idx as usize].shape as i16;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
        al.worldx = 0;
        al.worldy = SPACE_VIEWCY;
        al.stratstate = 0;
    }
    let wz = g.objs.aliens[idx as usize].worldz;
    g.vars.set_sv_i16(sv::VIEWPOSZ, wz.wrapping_add(1832));
    let vx = g.vars.sv_i16(sv::VIEWPOSX).wrapping_add(-20);
    g.vars.set_sv_i16(sv::VIEWPOSX, vx);
    apply_clearship_fly_mode(g, idx);
}

/// ROM `playerClearShip2_strat` (PCSTRATS.ASM:487-564).
pub fn player_clear_ship2_strat(g: &mut Game, idx: u16) {
    // State 0: black silhouette flash
    if g.objs.aliens[idx as usize].stratstate == 0 {
        let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
        if b1 == 0 {
            set_y_player_shape(g, idx, PSHIPNUM_BLACK);
            g.vars.set_sv_u8(sv::PSVAR_BYTE1, 15);
            g.objs.aliens[idx as usize].stratstate = 1;
        }
    }
    // State 1: restore shape, lock scroll
    if g.objs.aliens[idx as usize].stratstate == 1 {
        let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
        if b1 == 0 {
            g.objs.aliens[idx as usize].shape = g.vars.psvar_word1 as u16;
            g.vars.shared.background_scroll_x = 254;
            g.objs.aliens[idx as usize].stratstate = 2;
            g.vars.set_sv_u8(sv::PSVAR_BYTE1, 200);
        }
    }
    // State 2: viewposz nudge → boost
    if g.objs.aliens[idx as usize].stratstate == 2 {
        let vz = g.vars.sv_i16(sv::VIEWPOSZ).wrapping_add(10);
        g.vars.set_sv_i16(sv::VIEWPOSZ, vz);
        let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
        if b1 == 0 {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG4;
            g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS);
            g.vars.pshipflags3 &= !PSF3_ENGINESND;
            g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
            g.hooks.play_se(0x32);
            g.objs.aliens[idx as usize].stratstate = 3;
            g.vars.set_sv_u8(sv::PSVAR_BYTE1, 45 + 40);
        }
    }
    // State 3: boost Z + stagedone
    if g.objs.aliens[idx as usize].stratstate == 3 {
        let vz = g.vars.sv_i16(sv::VIEWPOSZ).wrapping_add(10);
        g.vars.set_sv_i16(sv::VIEWPOSZ, vz);
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(100);
        let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
        if b1 == 0 {
            g.vars.gameflags |= GF_STAGEDONE;
        }
    }

    // Always: cruise Z + viewposz + viewposy floor
    g.objs.aliens[idx as usize].worldz =
        g.objs.aliens[idx as usize].worldz.wrapping_add(MED_PSPEED);
    let vz = g
        .vars
        .sv_i16(sv::VIEWPOSZ)
        .wrapping_add(MED_PSPEED.wrapping_sub(15));
    g.vars.set_sv_i16(sv::VIEWPOSZ, vz);
    let vy = g.vars.sv_i16(sv::VIEWPOSY);
    if vy >= -200 {
        g.vars.set_sv_i16(sv::VIEWPOSY, vy.wrapping_sub(1));
    }
}

/// ROM `playernull_Istrat` (PSTRATS.ASM:1602-1607).
pub fn playernull_istrat(g: &mut Game, idx: u16) {
    g.vars.pviewvelz = MED_PSPEED;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(g.vars.pviewvelz);
    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(g.vars.pviewvelz);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);
}

// ============================================================
// ClearTurn / ClearUnder / ClearEarth (PCSTRATS.ASM)
// ============================================================

// Registry dispatch uses `StrategyFn = fn(&mut Game, u16)`.  Several public
// state bodies also return a transition boolean for focused tests/callers;
// these thin entries preserve that API while providing the ROM-style void
// strategy pointer.
fn player_clear_turn_tick(g: &mut Game, idx: u16) {
    let _ = player_clear_turn_strat(g, idx);
}
fn player_clear_under_tick(g: &mut Game, idx: u16) {
    let _ = player_clear_under_strat(g, idx);
}
fn player_clear_earth2_tick(g: &mut Game, idx: u16) {
    let _ = player_clear_earth2_strat(g, idx);
}
fn player_clear_demo_tick(g: &mut Game, idx: u16) {
    let _ = player_clear_demo_strat(g, idx);
}
fn player_dive_tick(g: &mut Game, idx: u16) {
    let _ = player_dive_strat(g, idx);
}
fn player_clear_chase_tick(g: &mut Game, idx: u16) {
    let _ = player_clear_chase_strat(g, idx);
}
fn player_warp_tick(g: &mut Game, idx: u16) {
    let _ = player_warp_strat(g, idx);
}
fn player_warp_out_tick(g: &mut Game, idx: u16) {
    let _ = player_warp_out_strat(g, idx);
}
fn player_into_cock_tick(g: &mut Game, idx: u16) {
    let _ = player_into_cock_strat(g, idx);
}
fn player_into_cock2_tick(g: &mut Game, idx: u16) {
    let _ = player_into_cock2_strat(g, idx);
}
fn player_out_of_cock_tick(g: &mut Game, idx: u16) {
    let _ = player_out_of_cock_strat(g, idx);
}

const UNDERGND_VIEWCY: i16 = -60;

/// ROM `set_playerClearTurn_l` / `playerClearTurn_Istrat`.
pub fn set_player_clear_turn(g: &mut Game, idx: u16) {
    player_clear_turn_istrat(g, idx);
}

/// ROM `playerClearTurn_Istrat` (PCSTRATS.ASM:577-586).
pub fn player_clear_turn_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_turn_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.psvar_word1 = 100 + 140 + 10 + 20; // 270
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.playerflymode |= PFM_WOBBLE;
}

/// ROM `playerClearTurn_strat` (PCSTRATS.ASM:589-598).
/// Returns `true` when phase-2 (ClearTurn2) starts this frame.
pub fn player_clear_turn_strat(g: &mut Game, idx: u16) -> bool {
    centoutrots(g);
    // s_beqdec_var W: TEST-then-DEC
    let w1 = g.vars.psvar_word1;
    if w1 == 0 {
        let _ = player_clear_turn2_init(g, idx);
        player_clear_turn2_strat(g, idx);
        return true;
    }
    g.vars.psvar_word1 = w1.wrapping_sub(1);
    let od = g.vars.sv_i16(sv::OUTDIST);
    if od >= 100 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_sub(1));
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_add(20), 4);
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
    }
    player_in_space_strat(g, idx);
    false
}

/// ROM `playerClearTurn2_strat` (PCSTRATS.ASM:607-609).
pub fn player_clear_turn2_strat(g: &mut Game, idx: u16) {
    player_in_space_strat(g, idx);
}

/// ROM `set_playerClearUNDER_l` / `playerclearUNDER_Istrat`.
pub fn set_player_clear_under(g: &mut Game, idx: u16) {
    player_clear_under_istrat(g, idx);
}

/// ROM `playerclearUNDER_Istrat` (PCSTRATS.ASM:753-769).
pub fn player_clear_under_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_under_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.minpmove_y = -10000;
    g.vars.set_sv_i16(sv::MINPWMOVEY, -10000);
    g.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
    g.vars.psvar_word1 = 140 + 54; // 194
    g.vars.psvar_word2 = 0;
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
}

/// ROM `playerclearUNDER_strat` (PCSTRATS.ASM:771-785).
/// Returns `true` when UNDER2 starts this frame.
pub fn player_clear_under_strat(g: &mut Game, idx: u16) -> bool {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, UNDERGND_VIEWCY.wrapping_sub(20), 3);
        al.worldx = strat_chase_proportional(al.worldx, 0, 3);
    }
    let od = g.vars.sv_i16(sv::OUTDIST);
    if od <= 500 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(4));
    }
    let outvy = g.vars.sv_i16(sv::OUTVY).wrapping_add(169);
    g.vars.set_sv_i16(sv::OUTVY, outvy);

    // s_beqdec_var W: TEST-then-DEC
    let w1 = g.vars.psvar_word1;
    if w1 == 0 {
        if let Some(dup) = player_under2_init(g, idx) {
            g.objs.aliens[dup as usize].sflags2 |= ASF2_SFLAG2;
        }
        player_under2_strat(g, idx);
        return true;
    }
    g.vars.psvar_word1 = w1.wrapping_sub(1);
    player_undergnd_strat(g, idx);
    false
}

/// ROM `playerUNDER2_strat` (PCSTRATS.ASM:794-796).
pub fn player_under2_strat(g: &mut Game, idx: u16) {
    player_undergnd_strat(g, idx);
}

/// ROM `set_playerClearEarth_l` — enters ClearEarth2.
pub fn set_player_clear_earth(g: &mut Game, idx: u16) {
    player_clear_earth2_istrat(g, idx);
}

/// ROM `playerClearEarth2_Istrat` (PCSTRATS.ASM:274-279).
pub fn player_clear_earth2_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_earth2_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 20);
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
}

/// ROM `playerClearEarth2_strat` (PCSTRATS.ASM:280-286).
/// Returns `true` when ClearEarth phase starts this frame.
pub fn player_clear_earth2_strat(g: &mut Game, idx: u16) -> bool {
    centoutrots(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = strat_chase_proportional(al.worldx, 0, 3);
        al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_sub(40), 3);
    }
    // s_beqdec_var B: TEST-then-DEC
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
    if b1 == 0 {
        player_clear_earth_istrat(g, idx);
        return true;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1.wrapping_sub(1));
    player_in_space_strat(g, idx);
    false
}

/// ROM `playerClearEarth_Istrat` (PCSTRATS.ASM:289-296) → ClearShip Icont + strat.
pub fn player_clear_earth_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_earth_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 40);
    g.objs.aliens[idx as usize].vel = (MAX_PSPEED - 5) as u8;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.playerflymode |= PFM_WOBBLE;
    // ROM brl playerClearShip_Icont (falls into ClearShip_strat same frame).
    player_clear_ship_icont(g, idx);
    player_clear_ship_strat(g, idx);
}

/// ROM `playerClearEarth_strat` (PCSTRATS.ASM:297-307).
pub fn player_clear_earth_strat(g: &mut Game, idx: u16) {
    g.vars.playerflymode |= PFM_WOBBLE;
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
    // s_beqdec: TEST then DEC — if was 0, skip chase; else dec and chase
    if b1 != 0 {
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1.wrapping_sub(1));
        centoutrots(g);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_sub(40), 4);
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
    }
    player_clear_ship_strat(g, idx);
}

// ============================================================
// ClearDemo / DIVE / ClearChase (PCSTRATS.ASM)
// ============================================================

const PLANET_VIEWCY: i16 = -50;

/// Planet-mode body used by ClearDemo (do_player_Yvel125 + view + viewmove).
fn player_on_planet_body(g: &mut Game, idx: u16) {
    do_player_yvel125(g, idx);
    update_viewxy_for_mode(g, idx);
    viewmove_srou(g, idx);
}

/// ROM `set_playerClearDemo_l` / `playercleardemo_Istrat`.
pub fn set_player_clear_demo(g: &mut Game, idx: u16) {
    player_clear_demo_istrat(g, idx);
}

/// ROM `playercleardemo_Istrat` (PCSTRATS.ASM:162-179).
pub fn player_clear_demo_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_demo_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, 110);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.minpmove_y = -10000;
    g.vars.set_sv_i16(sv::MINPWMOVEY, -10000);
    g.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, 180);
    g.vars.psvar_word1 = 0;
    g.vars.psvar_word2 = 0;
}

/// ROM `playercleardemo_strat` (PCSTRATS.ASM:182-194).
/// Returns `true` when demo2 starts this frame.
pub fn player_clear_demo_strat(g: &mut Game, idx: u16) -> bool {
    // s_beqdec_var B,psvar_byte2
    let b2 = g.vars.sv_u8(sv::PSVAR_BYTE2);
    if b2 == 0 {
        player_clear_demo2_init(g, idx);
        player_clear_demo2_strat(g, idx);
        return true;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, b2.wrapping_sub(1));
    centoutrots(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, PLANET_VIEWCY, 4);
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
    }
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
    let od = g.vars.sv_i16(sv::OUTDIST);
    if od <= 300 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(4));
    }
    let b3 = g.vars.sv_u8(sv::PSVAR_BYTE3).wrapping_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, b3);
    player_on_planet_body(g, idx);
    false
}

/// ROM `playercleardemo2_strat` (PCSTRATS.ASM:202-262) — full body (replaces stub).
pub fn player_clear_demo2_strat(g: &mut Game, idx: u16) {
    let outvy = g.vars.sv_i16(sv::OUTVY);
    if (outvy >> 8) as i8 >= -(DEG90 as i8) {
        g.vars.set_sv_i16(sv::OUTVY, outvy.wrapping_sub(32));
    }
    let b2 = g.vars.sv_u8(sv::PSVAR_BYTE2);
    if b2 != 0 {
        let b2n = b2.wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE2, b2n);
        let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1.wrapping_add(b2n));
    }
    let od = g.vars.sv_i16(sv::OUTDIST);
    if od <= 600 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(4));
    }
    let plrotx = g.vars.sv_i16(sv::PLROTX).wrapping_sub(2 * 256);
    g.vars.set_sv_i16(sv::PLROTX, plrotx);
    let scroll = g.vars.sv_i16(sv::BG2YSCROLL).wrapping_sub(1);
    g.vars.set_sv_i16(sv::BG2YSCROLL, scroll);

    playermove_srou(g, idx);

    // s_beqdec_var B,psvar_byte3,.nb — TEST-then-DEC; dup when post-dec == 1
    let b3 = g.vars.sv_u8(sv::PSVAR_BYTE3);
    if b3 != 0 {
        let b3n = b3.wrapping_sub(1);
        g.vars.set_sv_u8(sv::PSVAR_BYTE3, b3n);
        if b3n == 1 {
            let _ = dupplayer(g, idx);
            // clshipboost_Istrat on dup — leaf body is enemy_a; mark engines off
            g.vars.pshipflags3 &= !PSF3_ENGINESND;
        }
    }

    viewmove_srou(g, idx);

    let wx = g.objs.aliens[idx as usize].worldx;
    let pview_x = (wx >> 1).wrapping_add(wx >> 2).wrapping_add(wx >> 3);
    g.vars.set_sv_i16(sv::PVIEWPOSX, pview_x);
    g.vars
        .set_sv_i16(sv::PVIEWPOSY, g.objs.aliens[idx as usize].worldy);

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `set_playerDIVE_l` / `playerDIVE_Istrat`.
pub fn set_player_dive(g: &mut Game, idx: u16) {
    player_dive_istrat(g, idx);
}

/// ROM `playerDIVE_Istrat` (PCSTRATS.ASM:621-632).
pub fn player_dive_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_dive_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.psvar_word2 = 0;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_NOVDISTC;
    g.vars.psvar_word1 = 210 + 56 + 20; // 286
    g.vars.playerflymode |= PFM_WOBBLE;
}

/// ROM `playerDIVE_strat` (PCSTRATS.ASM:635-649).
/// Returns `true` when DIVE2 starts this frame.
pub fn player_dive_strat(g: &mut Game, idx: u16) -> bool {
    centoutrots(g);
    let w1 = g.vars.psvar_word1;
    if w1 == 0 {
        let _ = player_dive2_init(g, idx);
        player_dive2_strat(g, idx);
        return true;
    }
    g.vars.psvar_word1 = w1.wrapping_sub(1);
    // After dec: if 30 < word1 <= 60, bump bg2Yscroll
    let w1n = g.vars.psvar_word1;
    if w1n <= 60 && w1n >= 30 {
        let scroll = g.vars.sv_i16(sv::BG2YSCROLL).wrapping_add(1);
        g.vars.set_sv_i16(sv::BG2YSCROLL, scroll);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_add(20), 4);
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
    }
    player_in_space_strat(g, idx);
    false
}

/// ROM `playerDIVE2_strat` (PCSTRATS.ASM:656-658).
pub fn player_dive2_strat(g: &mut Game, idx: u16) {
    player_in_space_strat(g, idx);
}

/// ROM `set_playerClearCHASE_l` / `playerclearCHASE_Istrat`.
pub fn set_player_clear_chase(g: &mut Game, idx: u16) {
    player_clear_chase_istrat(g, idx);
}

/// ROM `playerclearCHASE_Istrat` (PCSTRATS.ASM:672-696).
pub fn player_clear_chase_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_chase_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.minpmove_y = -10000;
    g.vars.set_sv_i16(sv::MINPWMOVEY, -10000);
    g.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
    g.vars.psvar_word1 = 246 + 54; // 300
    g.vars.psvar_word2 = 0;
    g.vars.psvar_word3 = 218;
    g.vars.psvar_word4 = 5;
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
    g.vars.gameflags |= GF_NOZREMOVE;
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
}

/// ROM `playerclearCHASE_strat` (PCSTRATS.ASM:698-734).
/// Returns `true` when CHASE2 starts this frame.
pub fn player_clear_chase_strat(g: &mut Game, idx: u16) -> bool {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 3);
        al.worldx = strat_chase_proportional(al.worldx, 0, 3);
    }
    let od = g.vars.sv_i16(sv::OUTDIST);
    if od <= 500 {
        g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(4));
    }

    let w1 = g.vars.psvar_word1;
    // outvx: 120..=220 -=64; else 20..=120 +=64 (ROM MORE/LESS windows)
    if (120..=220).contains(&w1) {
        let vx = g.vars.sv_i16(sv::OUTVX).wrapping_sub(64);
        g.vars.set_sv_i16(sv::OUTVX, vx);
    } else if (20..=120).contains(&w1) {
        let vx = g.vars.sv_i16(sv::OUTVX).wrapping_add(64);
        g.vars.set_sv_i16(sv::OUTVX, vx);
    }
    // MORE #9 / #4 → act when w1 <= 9 / <= 4
    if w1 <= 9 {
        g.vars.psvar_word3 = g.vars.psvar_word3.wrapping_sub(21);
    }
    if w1 <= 4 {
        g.vars.psvar_word4 = g.vars.psvar_word4.wrapping_sub(1);
    }
    let outvy = g.vars.sv_i16(sv::OUTVY).wrapping_add(g.vars.psvar_word3);
    g.vars.set_sv_i16(sv::OUTVY, outvy);
    if w1 <= 56 {
        let scroll = g
            .vars
            .sv_i16(sv::BG2YSCROLL)
            .wrapping_add(g.vars.psvar_word4);
        g.vars.set_sv_i16(sv::BG2YSCROLL, scroll);
    }

    // s_beqdec_var W,psvar_word1
    if w1 == 0 {
        let _ = player_chase2_init(g, idx);
        player_chase2_strat(g, idx);
        return true;
    }
    g.vars.psvar_word1 = w1.wrapping_sub(1);
    player_in_space_strat(g, idx);
    false
}

/// ROM `playerCHASE2_strat` (PCSTRATS.ASM:741-743).
pub fn player_chase2_strat(g: &mut Game, idx: u16) {
    player_in_space_strat(g, idx);
}

// ============================================================
// Warp / WarpOut (PCSTRATS.ASM / PISTRATS.ASM)
// ============================================================

const PSTF_FLAG1: u8 = 2;
/// ROM `set_playerWarp_l` / `playerwarp_Istrat`.
pub fn set_player_warp(g: &mut Game, idx: u16) {
    player_warp_istrat(g, idx);
}

/// ROM `playerwarp_Istrat` (PCSTRATS.ASM:805-823).
pub fn player_warp_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_warp_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_NOVDISTC | PSTF_INSEQ;
    g.vars.psvar_word2 = 0;
    g.vars.gameflags |= GF_NOZREMOVE;
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.psvar_word1 = 200;
    g.vars.playerflymode |= PFM_WOBBLE;
    g.vars.pshipflags3 |= PSF3_NOCOLLISIONS;
    g.objs.aliens[idx as usize].stratstate = 0;
}

/// Shared Z boost used by warp1 / warp body (PCSTRATS.ASM:playerwarp).
fn player_warp_zboost(g: &mut Game, idx: u16) {
    // Cap word2 growth at 400
    if g.vars.psvar_word2 <= 400 {
        g.vars.psvar_word2 = g.vars.psvar_word2.wrapping_add(1);
    }
    let add = g.vars.psvar_word2;
    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(add);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(add);
    player_in_space_strat(g, idx);
}

/// ROM `playerwarp_strat` (PCSTRATS.ASM:825-897) — 3-state approach → turn → boost.
/// Returns `true` when warp1 starts this frame.
pub fn player_warp_strat(g: &mut Game, idx: u16) -> bool {
    // State 0: close outdist, center, countdown → state 1
    if g.objs.aliens[idx as usize].stratstate == 0 {
        if notdelay(g, 2) {
            let od = g.vars.sv_i16(sv::OUTDIST);
            g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_sub(1));
        }
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_add(20), 4);
            al.worldx = strat_chase_proportional(al.worldx, 0, 4);
        }
        centoutrots(g);
        // s_decbne_var W: DEC then BNE — stay in state if nonzero after dec
        let w1 = g.vars.psvar_word1.wrapping_sub(1);
        g.vars.psvar_word1 = w1;
        if w1 != 0 {
            // fall through to later states only if advanced; stay in 0
        } else {
            g.objs.aliens[idx as usize].stratstate = 1;
            g.vars.psvar_word1 = 256 - 30; // 226
            g.vars.playerflymode &= !PFM_WOBBLE;
        }
    }

    // State 1: spin outvy, open outdist, spawn hyperspace → state 2
    if g.objs.aliens[idx as usize].stratstate == 1 {
        if g.vars.psvar_word1 == 20 {
            g.hooks.play_music(0xf1); // bgm_fadeout stand-in
        }
        let od = g.vars.sv_i16(sv::OUTDIST);
        if od <= 500 {
            g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(5));
        }
        let outvy = g.vars.sv_i16(sv::OUTVY).wrapping_add(256);
        g.vars.set_sv_i16(sv::OUTVY, outvy);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_add(20), 4);
            al.worldx = strat_chase_proportional(al.worldx, 0, 4);
        }
        let w1 = g.vars.psvar_word1.wrapping_sub(1);
        g.vars.psvar_word1 = w1;
        if w1 != 0 {
            // stay
        } else {
            g.objs.aliens[idx as usize].stratstate = 2;
            if let Some(hs) = strat_make_obj(g, 0) {
                crate::enemy_a::hyperspace_istrat(g, hs);
            }
            g.vars.psvar_word1 = 60;
            g.hooks.play_music(8);
        }
    }

    // State 2: chase outvy→0, close outdist, drop noZremove → warp1
    if g.objs.aliens[idx as usize].stratstate == 2 {
        let outvy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 4);
        g.vars.set_sv_i16(sv::OUTVY, outvy);
        g.vars.gameflags &= !GF_NOZREMOVE;
        let od = g.vars.sv_i16(sv::OUTDIST).wrapping_sub(9);
        g.vars.set_sv_i16(sv::OUTDIST, od);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY.wrapping_add(80), 3);
        }
        // s_beqdec_var W → warp1_init
        let w1 = g.vars.psvar_word1;
        if w1 == 0 {
            let _ = player_warp1_init(g, idx);
            player_warp1_strat(g, idx);
            return true;
        }
        g.vars.psvar_word1 = w1.wrapping_sub(1);
        player_warp_zboost(g, idx);
        return false;
    }

    // States 0/1 (and post-advance fall-through) end via space when not in state 2 boost path
    if g.objs.aliens[idx as usize].stratstate != 2 {
        player_in_space_strat(g, idx);
    }
    false
}

/// ROM `playerwarp1_strat` (PCSTRATS.ASM:920-941).
pub fn player_warp1_strat(g: &mut Game, idx: u16) {
    g.vars.psvar_word2 = g.vars.psvar_word2.wrapping_add(2);
    // s_decbne_var B,psvar_byte1,playerwarp
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
    if b1 != 0 {
        player_warp_zboost(g, idx);
        return;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 1);
    g.vars.pstratflags |= PSTF_FLAG1;
    g.vars.dotsflag = 0;
    // m_clrbitmaps = 0 — HD has no SNES bitmap clear; skip
    // playerwarpnoadd: Z boost without word2++
    let add = g.vars.psvar_word2;
    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(add);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(add);
    player_in_space_strat(g, idx);
}

/// ROM `playerwarp2_strat` (PCSTRATS.ASM:948-951).
pub fn player_warp2_strat(g: &mut Game, idx: u16) {
    player_in_space_strat(g, idx);
}

/// ROM `set_playerWarpOut_l` / `playerWarpOut_Istrat` (PISTRATS.ASM:366-397).
pub fn set_player_warp_out(g: &mut Game, idx: u16) {
    player_warp_out_istrat(g, idx);
}

/// ROM `playerWarpOut_Istrat`.
pub fn player_warp_out_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_warp_out_tick);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_NOVDISTC;
    if let Some(hs) = strat_make_obj(g, 0) {
        crate::enemy_a::hyperspaceout_istrat(g, hs);
    }
    g.objs.aliens[idx as usize].vel = 120;
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, 120);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 64);
    g.vars.psvar_word2 = 128;
    g.vars.set_sv_i16(sv::OUTDIST, 400);
    // `s_playerfly_mode space` only applies the ROM fly-mode macro.  The Rust
    // helper also installs playerinspace_strat, so do it before the explicit
    // `s_set_alptrs` below or the WarpOut countdown callback is lost and
    // control remains disabled for the whole Space Armada stage.
    set_player_in_space(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    // re-assert cutscene flags cleared by space fly-mode
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ | PSTF_NOVDISTC | PSTF_FLAG1;
}

/// ROM `playerWarpOut_strat` (PISTRATS.ASM:399-440).
/// Returns `true` when warp-out ends and hands off to space.
pub fn player_warp_out_strat(g: &mut Game, idx: u16) -> bool {
    // fadewhite2norm — window slot; HD Windows path is map-CB driven; skip leaf
    let od = g.vars.sv_i16(sv::OUTDIST).wrapping_sub(5);
    g.vars.set_sv_i16(sv::OUTDIST, od);
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
    if b1 == 0 {
        g.vars.pstratflags &= !PSTF_NOVDISTC;
        g.vars.viewdist = OUTVIEWDIST;
        g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
        g.vars.player_view_options = PlayerViewOptions::ExteriorAndCockpit;
        // PISTRATS.ASM `.warpoutend`: `s_set_strat x,playerinspace_strat`.
        // Calling the body without replacing the installed WarpOut callback
        // makes the byte countdown wrap to 255 on the next frame and applies
        // an ever-decreasing (eventually negative) hyperspace Z delta.
        let space_tick = ea_sid(g, player_in_space_strat);
        g.objs.aliens[idx as usize].stratptr = Some(space_tick);
        g.vars.player_view_mode = PlayerViewMode::EnteringCockpit;
        // The view dispatcher starts the authored transition, which owns the
        // eventual control release after the cockpit zoom finishes.
        g.apply_player_view_mode(idx);
        g.hooks.play_music(4);
        player_in_space_strat(g, idx);
        return true;
    }
    let add = g.vars.psvar_word2;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(add);
    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(add);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);
    g.vars.psvar_word2 = add.wrapping_sub(2);
    player_in_space_strat(g, idx);
    false
}

// ============================================================
// pshipcolony / pshipwashent (GCSTRATS.ASM:1829-2021)
// ============================================================

/// `pshipcolonyrot_tab` (GCSTRATS.ASM:1897) — duration,rotx pairs; `0` duration → straight.
const PSHIP_COLONY_ROT_TAB: &[u8] = &[
    0x0e, 0x00, 0x04, 0x00, 0x09, 0xeb, 0x0e, 0xd6, 0x09, 0xeb, 0x09, 0x00, 0x09, 0x15, 0x09, 0x2a,
    0x0d, 0x40, 0x09, 0x40, 0x0c, 0x2a, 0x0c, 0x15, 0x09, 0x00, 0x09, 0xeb, 0x09, 0xd6, 0x09, 0xc0,
    0x09, 0xd6, 0x04, 0xeb, 0x09, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x18, 0x00, 0x18, 0x00, 0x18,
    0x00, 0x18, 0x00, 0x00,
];

/// `pshipwashentrot_tab` (GCSTRATS.ASM:2001).
const PSHIP_WASHENT_ROT_TAB: &[u8] = &[
    0x0e, 0x00, 0x04, 0x00, 0x09, 0xeb, 0x0e, 0xd6, 0x09, 0xeb, 0x09, 0x00, 0x09, 0x15, 0x10, 0x2a,
    0x04, 0x15, 0x09, 0x00, 0x09, 0x00, 0x09, 0x00, 0x00,
];

/// Chase high byte of a 16-bit outv toward `target_hi` (ROM `s_achase_var B,outv+1`).
fn achase_outv_hi(g: &mut Game, variable: sv, target_hi: u8, shift: u32) {
    let v = g.vars.sv_i16(variable) as u16;
    let mut hi = (v >> 8) as u8;
    crate::enemy_a::achase_angle(&mut hi, target_hi, shift);
    g.vars
        .set_sv_i16(variable, ((hi as u16) << 8 | (v & 0xff)) as i16);
}

fn pipe_follow_bank(g: &mut Game, idx: u16, tab: &[u8]) -> bool {
    // Bank with rotx: rotz / outvz_hi chase (rotx<<1); outvx_hi chase −rotx.
    let rotx = g.objs.aliens[idx as usize].rotx;
    let bank = rotx.wrapping_mul(2); // s_scale_var B,1
    let mut rz = g.objs.aliens[idx as usize].rotz;
    crate::enemy_a::achase_angle(&mut rz, bank, 4);
    g.objs.aliens[idx as usize].rotz = rz;
    achase_outv_hi(g, sv::OUTVZ, bank, 4);
    achase_outv_hi(g, sv::OUTVX, 0u8.wrapping_sub(rotx), 2);

    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        let off = g.objs.aliens[idx as usize].sbyte2 as usize;
        let dur = *tab.get(off).unwrap_or(&0);
        let ang = *tab.get(off.wrapping_add(1)).unwrap_or(&0);
        g.objs.aliens[idx as usize].sbyte1 = dur;
        g.objs.aliens[idx as usize].sbyte3 = ang;
        g.objs.aliens[idx as usize].sbyte2 = g.objs.aliens[idx as usize].sbyte2.wrapping_add(2);
        if dur == 0 {
            return true; // → .straight
        }
    }
    let mut rx = g.objs.aliens[idx as usize].rotx;
    let tgt = g.objs.aliens[idx as usize].sbyte3;
    crate::enemy_a::achase_angle(&mut rx, tgt, 3);
    g.objs.aliens[idx as usize].rotx = rx;
    false
}

fn pipe_follow_cont(g: &mut Game, idx: u16, scroll_bg: bool) {
    {
        let al = &g.objs.aliens[idx as usize];
        g.vars.set_sv_i16(sv::PVIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::PVIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::PVIEWPOSZ, al.worldz);
    }
    if scroll_bg {
        let z = g.vars.sv_i16(sv::BGSSCROLLZ).wrapping_add(MED_PSPEED);
        g.vars.set_sv_i16(sv::BGSSCROLLZ, z);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
    }
}

/// ROM `pshipcolony_Istrat` (GCSTRATS.ASM:1829).
pub fn pshipcolony_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipcolony_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 1;
        al.sbyte2 = 0;
        al.stratstate = 0;
        al.stratptr = Some(tick);
        al.vel = MED_PSPEED as u8;
        al.sflags |= ASF_SHADOW;
    }
}

/// ROM `pshipcolony_strat` (GCSTRATS.ASM:1835) — pipe weave then L-tunnel handoff.
pub fn pshipcolony_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        if pipe_follow_bank(g, idx, PSHIP_COLONY_ROT_TAB) {
            g.objs.aliens[idx as usize].stratstate = 1;
            g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
            g.hooks.play_se(0x32);
            g.objs.aliens[idx as usize].vel = 120;
            g.objs.aliens[idx as usize].sbyte1 = 30;
            // Fall into .straightstrat same frame (ROM).
        } else {
            pipe_follow_cont(g, idx, true);
            return;
        }
    }

    // .straightstrat
    achase_outv_hi(g, sv::OUTVZ, 0, 4);
    achase_outv_hi(g, sv::OUTVX, 0, 4);
    let mut rz = g.objs.aliens[idx as usize].rotz;
    let mut rx = g.objs.aliens[idx as usize].rotx;
    crate::enemy_a::achase_angle(&mut rz, 0, 4);
    crate::enemy_a::achase_angle(&mut rx, 0, 4);
    g.objs.aliens[idx as usize].rotz = rz;
    g.objs.aliens[idx as usize].rotx = rx;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(150);
    let wy = g.objs.aliens[idx as usize].worldy;
    g.objs.aliens[idx as usize].worldy = strat_chase_proportional(wy, LTUNNEL_VIEWCY, 4);

    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        let p = g.vars.internal_playpt;
        if p >= 0 && (p as usize) < NUMBER_AL {
            let pidx = p as u16;
            set_player_in_ltunnel(g, pidx);
            g.vars.shared.do_depth_rotation = 0;
            g.objs.aliens[pidx as usize].sflags &= !ASF_INVISIBLE;
            g.objs.aliens[pidx as usize].worldx = 0;
            g.objs.aliens[pidx as usize].worldy = LTUNNEL_VIEWCY;
            let wz = g.objs.aliens[idx as usize].worldz;
            g.objs.aliens[pidx as usize].worldz = wz.wrapping_add(120);
        }
        g.objs.aldead = 1;
    }
    pipe_follow_cont(g, idx, true);
}

/// ROM `pshipwashent_Istrat` (GCSTRATS.ASM:1933).
pub fn pshipwashent_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipwashent_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 1;
        al.sbyte2 = 0;
        al.stratstate = 0;
        al.stratptr = Some(tick);
        al.vel = MED_PSPEED as u8;
        al.sflags |= ASF_SHADOW;
    }
}

/// ROM `pshipwashent_strat` (GCSTRATS.ASM:1939) — pipe weave then nucleus handoff.
pub fn pshipwashent_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        if pipe_follow_bank(g, idx, PSHIP_WASHENT_ROT_TAB) {
            g.objs.aliens[idx as usize].stratstate = 1;
            g.objs.aliens[idx as usize].sbyte1 = 30;
            g.hooks.play_se(0x33);
            // Fall into .straightstrat same frame.
        } else {
            pipe_follow_cont(g, idx, false);
            return;
        }
    }

    let od = g.vars.sv_i16(sv::OUTDIST);
    g.vars
        .set_sv_i16(sv::OUTDIST, strat_chase_proportional(od, 0, 3));
    let wy = g.objs.aliens[idx as usize].worldy;
    g.objs.aliens[idx as usize].worldy = strat_chase_proportional(wy, NUCLEUS_VIEWCY, 3);
    achase_outv_hi(g, sv::OUTVZ, 0, 4);
    achase_outv_hi(g, sv::OUTVX, 0, 4);
    let mut rz = g.objs.aliens[idx as usize].rotz;
    let mut rx = g.objs.aliens[idx as usize].rotx;
    crate::enemy_a::achase_angle(&mut rz, 0, 4);
    crate::enemy_a::achase_angle(&mut rx, 0, 4);
    g.objs.aliens[idx as usize].rotz = rz;
    g.objs.aliens[idx as usize].rotx = rx;

    let p = g.vars.internal_playpt;
    if p >= 0 && (p as usize) < NUMBER_AL {
        g.objs.aliens[p as usize].worldz = g.objs.aliens[idx as usize].worldz;
    }

    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        if p >= 0 && (p as usize) < NUMBER_AL {
            let pidx = p as u16;
            set_player_in_nucleus(g, pidx);
            g.vars.shared.do_depth_rotation = 0;
            g.objs.aliens[pidx as usize].sflags &= !ASF_INVISIBLE;
            g.objs.aliens[pidx as usize].worldx = 0;
            g.objs.aliens[pidx as usize].worldz =
                g.objs.aliens[pidx as usize].worldz.wrapping_add(MED_PSPEED);
        }
        g.objs.aldead = 1;
    }
    pipe_follow_cont(g, idx, false);
}

// ============================================================
// Water / bridge / undergnd / space SET_PLAYER* (PSTRATS.ASM)
// ============================================================

/// Shared planet-style fly-mode apply (water/bridge/undergnd/space).
fn apply_planet_style_fly_mode(
    g: &mut Game,
    idx: u16,
    view_cy: i16,
    min_x: i16,
    max_x: i16,
    mmin_x: i16,
    mmax_x: i16,
    min_y: i16,
    max_y: i16,
    mmax_y: i16,
    flymode: u8,
    pmove_and: u8,
    miss_flags: u8,
    viewrot_on: bool,
    shadow: bool,
    clear_intunnel: bool,
) {
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    {
        let v = &mut g.vars;
        v.set_sv_i16(sv::VIEWCY, view_cy);
        v.set_sv_i16(sv::MINPMOVEX, min_x);
        v.set_sv_i16(sv::MAXPMOVEX, max_x);
        v.set_sv_i16(sv::MINMMOVEX, mmin_x);
        v.set_sv_i16(sv::MAXMMOVEX, mmax_x);
        v.set_sv_i16(sv::MAXMMOVEY, mmax_y);
        v.set_sv_i16(sv::MINPWMOVEY, min_y);
        v.set_sv_i16(sv::MAXPWMOVEY, max_y + PLAYER_WING_Y_PADDING);
        v.minpmove_y = min_y;
        v.set_sv_i16(sv::MAXPMOVEY, max_y + PLAYERB_YSTOP);
        v.playerflymode = flymode;
        v.set_sv_u8(sv::PMOVELIMITAND, pmove_and);
        v.set_sv_u8(sv::MISSBOUNDFLAGS, miss_flags);
        if viewrot_on {
            v.gameflags |= GF_VIEWROT;
        } else {
            v.gameflags &= !GF_VIEWROT;
        }
        v.pstratflags &= !PSTF_NOVIEWMOVE;
        v.pshipflags3 |= PSF3_ENGINESND;
        v.pshipflags2 &= !PSF2_NOSPARK;
        v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
        if clear_intunnel {
            v.pshipflags3 &= !PSF3_INTUNNEL;
        }
    }
    if shadow {
        g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    } else {
        g.objs.aliens[idx as usize].sflags &= !ASF_SHADOW;
    }
}

/// ROM `set_playerOnWater_l` / water fly-mode.
pub fn set_player_on_water(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_on_water_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    g.vars.game_mode = WATER_MODE;
    apply_planet_style_fly_mode(
        g,
        idx,
        -50,
        -500,
        500,
        -500,
        500,
        -255, // -210-45
        0,
        0,
        PFM_DIEFALL | PFM_DIEYROT | PFM_WATER | PFM_SHADOWS | PFM_WOBBLE,
        PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM,
        MB_BOTTOM,
        true,
        true,
        true,
    );
}

/// ROM `playeronwater_strat` — live path uses Yvel125 (ifeq 1 block is dead).
pub fn player_on_water_strat(g: &mut Game, idx: u16) {
    do_player_yvel125(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    g.vars.set_sv_i16(sv::PVIEWPOSX, wx >> 1);
    g.vars.set_sv_i16(sv::PVIEWPOSY, -50); // water / planet_ViewCY style
    viewmove_srou(g, idx);
}

/// ROM `set_playerOnBridge_l` / bridge fly-mode.
pub fn set_player_on_bridge(g: &mut Game, idx: u16) {
    apply_planet_style_fly_mode(
        g,
        idx,
        -60,
        -200,
        200,
        -200,
        200,
        -120,
        0,
        0,
        PFM_DIEFALL | PFM_WATER | PFM_SHADOWS | PFM_WOBBLE,
        PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM,
        MB_BOTTOM | MB_LTOP | MB_RTOP,
        false,
        true,
        true,
    );
    g.vars.set_sv_i16(sv::MISSBTOPLEFT, -90);
    g.vars.set_sv_i16(sv::MISSBTOPRIGHT, 90);
}

/// ROM `set_playerUnderGnd_l` / undergnd fly-mode.
pub fn set_player_undergnd(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_undergnd_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    apply_planet_style_fly_mode(
        g,
        idx,
        -60,
        -500,
        500,
        -500,
        500,
        -120,
        0,
        0,
        PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS | PFM_WOBBLE,
        PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM,
        MB_BOTTOM,
        false,
        true,
        true,
    );
}

/// ROM `playerundergnd_strat`.
pub fn player_undergnd_strat(g: &mut Game, idx: u16) {
    do_player_yvel_d2(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    // view X = perc87-ish / asra chain — undergnd uses same as tunnel 0.875
    let pview_x = (wx >> 1).wrapping_add(wx >> 2).wrapping_add(wx >> 3);
    g.vars.set_sv_i16(sv::PVIEWPOSX, pview_x);
    g.vars.set_sv_i16(sv::PVIEWPOSY, g.vars.sv_i16(sv::VIEWCY));
    viewmove_srou(g, idx);
}

/// ROM `set_playerInSpace_l` / space fly-mode.
pub fn set_player_in_space(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_in_space_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    apply_space_flight_mode(g, SpaceFlightSetup::Interactive);
}

/// ROM `playerinspace_strat`.
pub fn player_in_space_strat(g: &mut Game, idx: u16) {
    do_player_limit_x(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    let wy = g.objs.aliens[idx as usize].worldy;
    // Typical space: pview tracks player (simplified; ROM uses viewmove after limitX)
    g.vars.set_sv_i16(sv::PVIEWPOSX, wx);
    g.vars.set_sv_i16(sv::PVIEWPOSY, wy);
    viewmove_srou(g, idx);
}

/// Public bridge strat (already used by clear-bridge sequence).
pub fn player_on_bridge_strat(g: &mut Game, idx: u16) {
    playeronbridge_strat(g, idx);
}

/// ROM `set_playerTurn180_l` — arm the 74-frame turn (body in `enemy_b`).
pub fn set_player_turn180(g: &mut Game, idx: u16) {
    let _ = idx;
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 64 + 10);
}

/// ROM `set_playerEscapeNucleus_l` — delegates to existing escape init.
pub fn set_player_escape_nucleus(g: &mut Game, idx: u16) {
    strat_player_escape_nucleus_init(g, idx);
}

// ============================================================
// Cockpit enter / exit (PSTRATS.ASM)
// ============================================================

const SPACE_VIEWCY: i16 = -60;
const SPACE_MIN_X: i16 = -240;
const SPACE_MAX_X: i16 = 240;
const SPACE_MIN_Y: i16 = -190;
const SPACE_MAX_Y: i16 = 80;
const SPACE_MAX_WORLD_Y: i16 = 10000;
pub const COCKPIT_EXIT_FRAMES: u8 = 23;
const COCKPIT_PROP_SPAWN_FRAME: u8 = 19;
const COCKPIT_PROP_REAR_OFFSET: i16 = 105;
const ZOOM_SHIP_REAR_OFFSET: i16 = 611;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpaceFlightSetup {
    Interactive,
    PreserveSequence,
}

/// Apply the common `space` flight bounds. The cockpit transition uses the
/// source's `nomacro` form so death/sequence flags survive until its handoff.
fn apply_space_flight_mode(g: &mut Game, setup: SpaceFlightSetup) {
    g.vars.game_mode = SPACE_MODE;
    g.vars.set_sv_i16(sv::VIEWCY, SPACE_VIEWCY);
    g.vars.set_sv_i16(sv::MINPMOVEX, SPACE_MIN_X);
    g.vars.set_sv_i16(sv::MAXPMOVEX, SPACE_MAX_X);
    g.vars.set_sv_i16(sv::MINMMOVEX, SPACE_MIN_X);
    g.vars.set_sv_i16(sv::MAXMMOVEX, SPACE_MAX_X);
    g.vars.set_sv_i16(sv::MAXMMOVEY, SPACE_MAX_WORLD_Y);
    g.vars.set_sv_i16(sv::MINPWMOVEY, SPACE_MIN_Y);
    g.vars
        .set_sv_i16(sv::MAXPWMOVEY, SPACE_MAX_Y + PLAYER_WING_Y_PADDING);
    g.vars.minpmove_y = SPACE_MIN_Y;
    g.vars
        .set_sv_i16(sv::MAXPMOVEY, SPACE_MAX_Y + PLAYERB_YSTOP);
    g.vars.playerflymode = PFM_DIEYROT | PFM_WOBBLE;
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0);
    g.vars.set_sv_u8(sv::MISSBOUNDFLAGS, 0);
    g.vars.gameflags |= GF_VIEWROT;
    g.vars.pstratflags &= !PSTF_NOVIEWMOVE;
    g.vars.pshipflags3 |= PSF3_ENGINESND;

    if setup == SpaceFlightSetup::Interactive {
        g.vars.pshipflags2 &= !PSF2_NOSPARK;
        g.vars.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
        g.vars.pshipflags3 &= !PSF3_INTUNNEL;
    }
}

/// ROM `makeallmedpspeed`.
pub fn make_all_med_pspeed(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].vel = MED_PSPEED as u8;
    g.vars.pviewvelz = MED_PSPEED;
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
    g.vars.playervel_z = MED_PSPEED;
}

/// ROM `set_playerintocock_l` / `playerintocock_Istrat` init.
pub fn set_player_into_cock(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_into_cock_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_NOVDISTC;
    make_all_med_pspeed(g, idx);
}

/// ROM `playerintocock_strat` — chase toward center; when outdist→0, phase 2.
/// Returns `true` if phase-2 should start.
pub fn player_into_cock_strat(g: &mut Game, idx: u16) -> bool {
    let mut reached = false;
    for _ in 0..2 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = strat_chase_proportional(al.worldx, 0, 1);
            al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY, 1);
        }
        let ztilt = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
        let ztilt2 = strat_chase_proportional(ztilt as i16, 0, 1) as i8;
        g.vars.set_sv_u8(sv::PLAYER_ZTILT, ztilt2 as u8);
        let plrotz = strat_chase_proportional(g.vars.sv_i16(sv::PLROTZ), 0, 1);
        g.vars.set_sv_i16(sv::PLROTZ, plrotz);
        let od = g.vars.sv_i16(sv::OUTDIST);
        let od2 = strat_chase_proportional(od, 0, 1);
        g.vars.set_sv_i16(sv::OUTDIST, od2);
        if od2 == 0 {
            reached = true;
        }
    }
    if reached {
        player_into_cock2_init(g, idx);
        return true;
    }
    player_in_space_strat(g, idx);
    false
}

/// ROM `playerintocock2_init`.
pub fn player_into_cock2_init(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_into_cock2_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_i16(sv::OUTDIST, 0);
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::OUTVZ, 0);

    let pview_z = g.vars.sv_i16(sv::PVIEWPOSZ);
    g.objs.aliens[idx as usize].worldz = pview_z.wrapping_add(CLOSE_VIEW_DISTANCE);

    if let Some(dup) = dupplayer(g, idx) {
        set_y_player_shape(g, dup, PSHIPNUM_ZOOM);
        // Place dup: pviewz + (worldz-pviewz)<<4
        let wz = g.objs.aliens[idx as usize].worldz;
        let delta = wz.wrapping_sub(pview_z);
        g.objs.aliens[dup as usize].worldz = pview_z.wrapping_add(delta.wrapping_shl(4));
        cockdumpl_istrat(g, dup);
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 20);
}

/// ROM `playerintocock2_strat` — countdown then enter inside view.
pub fn player_into_cock2_strat(g: &mut Game, idx: u16) -> bool {
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
    if b1 != 0 {
        g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1 - 1);
        // playerstraight_strat: keep chasing center via space strat
        player_in_space_strat(g, idx);
        return false;
    }
    // Enter cockpit: bigspace + spfm_inside
    apply_planet_style_fly_mode(
        g,
        idx,
        -60,
        -600,
        600,
        -600,
        600,
        -190,
        80,
        10000,
        PFM_DIEYROT | PFM_WOBBLE,
        0,
        0,
        true,
        false,
        true,
    );
    g.vars.player_view_mode = PlayerViewMode::Cockpit;
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    g.vars.pstratflags &= !PSTF_NOVDISTC;
    g.objs.aliens[idx as usize].sbyte2 = 10;
    let tick = ea_sid(g, player_in_space_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    player_in_space_strat(g, idx);
    true
}

// ============================================================
// Cockpit dump / out props (PSTRATS.ASM:1424-1605)
// ============================================================

const SH_COCKPIT: u16 = 322;

/// ROM `cockdumpl_Istrat` — zoom-ship trail that spawns a cockpit prop once.
pub fn cockdumpl_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, cockdumpl_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARD_HP;
        al.ap = 0;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(tick);
        al.sflags2 &= !ASF2_SFLAG1;
        al.count = 8; // lifecnt
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
    }
}

/// ROM `cockdumpl_strat`.
pub fn cockdumpl_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count;
        al.count = c.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(160);
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        return;
    }
    let p = g.vars.internal_playpt;
    if p < 0 || (p as usize) >= g.objs.aliens.len() || !g.objs.aliens[p as usize].active {
        return;
    }
    let p = p as u16;
    let me_z = g.objs.aliens[idx as usize].worldz;
    let pl_z = g.objs.aliens[p as usize].worldz;
    // .docock when player in front of dump OR |dz|<50.
    let player_in_front = pl_z >= me_z;
    let close = (me_z as i32 - pl_z as i32).abs() < 50;
    if !(player_in_front || close) {
        return;
    }
    if let Some(cock) = strat_make_obj(g, SH_COCKPIT) {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
        {
            let src = g.objs.aliens[p as usize];
            let c = &mut g.objs.aliens[cock as usize];
            c.worldx = src.worldx;
            c.worldy = src.worldy;
            c.worldz = src.worldz;
        }
        cockpit_istrat(g, cock);
    }
}

/// ROM `cockpit_Istrat` / `cockpit_strat`.
pub fn cockpit_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, cockpit_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARD_HP;
        al.ap = 0;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(tick);
    }
}

pub fn cockpit_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count;
        al.count = c.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(10);
}

/// ROM `cockshipout_Istrat` / `cockshipout_strat`.
pub fn cockshipout_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, cockshipout_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARD_HP;
        al.ap = 0;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(tick);
        al.type_ &= !ATZREMOVE;
        al.count = 19;
        al.sword1 = 60;
    }
}

pub fn cockshipout_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        let zadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD);
        g.objs.aliens[idx as usize].rotz = zadd;
        if g.vars.gameframe & 1 == 0 {
            g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = g.vars.player_posx;
        al.worldy = g.vars.player_posy;
        let c = al.count;
        al.count = c.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(al.sword1);
        al.sword1 = al.sword1.wrapping_add(10);
    }
}

/// ROM `cockpitout_Istrat` / `cockpitout_strat`.
pub fn cockpitout_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, cockpitout_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARD_HP;
        al.ap = 0;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(tick);
        al.type_ &= !ATZREMOVE;
        al.count = 8;
    }
}

pub fn cockpitout_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        let zadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD);
        g.objs.aliens[idx as usize].rotz = zadd;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = g.vars.player_posx;
        al.worldy = g.vars.player_posy;
        let c = al.count;
        al.count = c.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(20);
}

/// ROM `set_playeroutofcock_l` / `playeroutofcock_Istrat` init.
pub fn set_player_out_of_cock(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_out_of_cock_tick);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.player_view_mode = PlayerViewMode::LeavingCockpit;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags &= !PSTF_NOVDISTC;
    g.vars.viewdist = OUTVIEWDIST;
    g.vars.set_sv_i16(sv::OUTDIST, OUTVIEWDIST);
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::OUTVZ, 0);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, COCKPIT_EXIT_FRAMES);
    make_all_med_pspeed(g, idx);
    // The source initializer has no end marker and executes the first
    // transition body immediately.
    let _ = player_out_of_cock_strat(g, idx);
}

/// ROM `playeroutofcock_strat` — chase center; spawn props at byte1==19; finish at 0.
pub fn player_out_of_cock_strat(g: &mut Game, idx: u16) -> bool {
    for _ in 0..2 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = strat_chase_proportional(al.worldx, 0, 1);
            al.worldy = strat_chase_proportional(al.worldy, SPACE_VIEWCY, 1);
        }
        let ztilt = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
        g.vars.set_sv_u8(
            sv::PLAYER_ZTILT,
            strat_chase_proportional(ztilt as i16, 0, 1) as i8 as u8,
        );
        let plrotz = strat_chase_proportional(g.vars.sv_i16(sv::PLROTZ), 0, 1);
        g.vars.set_sv_i16(sv::PLROTZ, plrotz);
    }

    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
    if b1 == COCKPIT_PROP_SPAWN_FRAME {
        // Spawn cockpit + zoom ship props with real out strats.
        if let Some(cock) = strat_make_obj(g, SH_COCKPIT) {
            let src = g.objs.aliens[idx as usize];
            {
                let c = &mut g.objs.aliens[cock as usize];
                c.worldx = src.worldx;
                c.worldy = src.worldy;
                c.worldz = src.worldz.wrapping_sub(COCKPIT_PROP_REAR_OFFSET);
            }
            cockpitout_istrat(g, cock);
        }
        if let Some(ship) = strat_make_obj(g, 0) {
            set_y_player_shape(g, ship, PSHIPNUM_ZOOM);
            let src = g.objs.aliens[idx as usize];
            {
                let s = &mut g.objs.aliens[ship as usize];
                s.worldx = src.worldx;
                s.worldy = src.worldy;
                s.worldz = src.worldz.wrapping_sub(ZOOM_SHIP_REAR_OFFSET);
            }
            cockshipout_istrat(g, ship);
        }
    }

    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        let roll = g
            .vars
            .sv_u8(sv::PLAYER_ZSTRATADD)
            .wrapping_add(PLAYER_DEATH_ROLL_ACCELERATION);
        g.vars.set_sv_u8(sv::PLAYER_ZSTRATADD, roll);
    }

    let remaining = b1.saturating_sub(1);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, remaining);
    if remaining != 0 {
        player_straight_strat(g, idx);
        return false;
    }

    // `s_playerfly_mode space,nomacro`: update the space bounds while
    // preserving the transition/death flags until the branch below.
    apply_space_flight_mode(g, SpaceFlightSetup::PreserveSequence);
    g.objs.aliens[idx as usize].sflags &= !ASF_INVISIBLE;
    g.vars.player_view_mode = PlayerViewMode::Exterior;

    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        playerdead_istrat(g, idx);
        return true;
    }

    let tick = ea_sid(g, player_in_space_strat);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    {
        let player = &mut g.objs.aliens[idx as usize];
        player.stratptr = Some(tick);
        player.collstratptr = Some(coll);
        player.expstratptr = Some(exp);
        player.sflags &= !ASF_HITFLASH;
    }
    if g.vars.pstratflags & PSTF_INSEQ == 0 {
        g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    }
    g.vars.viewdist = OUTVIEWDIST;
    player_in_space_strat(g, idx);
    true
}

// ============================================================
// LB out / dive / cred / tunnel→planet (PCSTRATS / PISTRATS / PSTRATS)
// ============================================================

const DEG90_256: i16 = 64 * 256; // deg90*256

/// ROM `set_playerOutOfLB1_l` / init.
pub fn set_player_out_of_lb1(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.gameflags &= !GF_STRATDONE1;
    apply_tunnel_fly_mode(g, idx, FLY_LTUNNEL);
    // Sequence flag after tunnel macro (which clears pstf_inseq).
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.hooks.play_se(0x1e);
}

/// ROM `playerOutofLB1_strat`.
pub fn player_out_of_lb1_strat(g: &mut Game, idx: u16) {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 4);
    }
    // shrapnel_srou — debris + explosion bursts while exiting LB1.
    shrapnel_srou(g, idx);
    player_in_tunnel_strat(g, idx);
}

// ============================================================
// Shrapnel (PCSTRATS.ASM) — LB1 debris
// ============================================================

const SHAPE_SHRAP1: u16 = 267;

/// `s_jmp_notdelay N` is true when `gameframe & ((1<<N)-1) == 0`.
fn notdelay(g: &Game, bits: u16) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

/// ROM `shrapfall2_Istrat` — scrap drifts toward camera (−30 Z/frame).
fn shrapfall2_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(30);
}

/// ROM `shrapnel_srou` / `shrapnel_srou_l` (PCSTRATS.ASM).
pub fn shrapnel_srou(g: &mut Game, parent: u16) {
    // Every 8 frames: spawn shrap1 scrap.
    if notdelay(g, 3) {
        if let Some(y) = strat_make_obj(g, SHAPE_SHRAP1) {
            let view_cy = g.vars.sv_i16(sv::VIEWCY);
            let parent_z = g.objs.aliens[parent as usize].worldz;
            {
                let al = &mut g.objs.aliens[y as usize];
                al.worldx = (sf_random(&mut g.vars) as u8 as i16).wrapping_sub(128);
                al.worldy = (sf_random(&mut g.vars) as u8 as i16)
                    .wrapping_sub(128)
                    .wrapping_add(view_cy);
                al.roty = sf_random(&mut g.vars) as u8;
                al.rotx = sf_random(&mut g.vars) as u8;
                al.worldz = parent_z.wrapping_add(2000);
                al.sflags |= ASF_COLLDISABLE;
            }
            let s = sid(g, K_SHRAPFALL2);
            g.objs.aliens[y as usize].stratptr = Some(s);
        }
    }

    // Every 2 frames: large + medium explosion bursts.
    if notdelay(g, 1) {
        if let Some(y) = make_large_exp_obj(g, parent) {
            {
                let al = &mut g.objs.aliens[y as usize];
                al.sflags4 |= ASF4_NOPOLYEXP;
                al.sflags2 &= !ASF2_NOEXPSND;
                al.vz = 150;
                al.worldz = al.worldz.wrapping_sub(200);
            }
            addrnd2pos_xy(g, y);
        }
        if let Some(y) = make_medium_exp_obj(g, parent) {
            {
                let al = &mut g.objs.aliens[y as usize];
                al.sflags4 |= ASF4_NOPOLYEXP;
                al.sflags2 &= !ASF2_NOEXPSND;
                al.vz = 150;
                al.worldz = al.worldz.wrapping_sub(200);
            }
            addrnd2pos_xy(g, y);
        }
    }
}

/// Public alias for tests.
pub fn shrapfall2_tick(g: &mut Game, idx: u16) {
    shrapfall2_istrat(g, idx);
}

/// ROM `set_playerOutOfLB2a_l` / init.
pub fn set_player_out_of_lb2a(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_out_of_lb2a_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
    g.objs.aliens[idx as usize].vel = MAX_PSPEED as u8;
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
    if g.vars.sv_u8(sv::BOOSTZOFF) == 0 {
        set_boost_zoff(g, -30);
    }
    let _ = boost_sprite(g, None);
    g.hooks.play_se(0x32);
}

/// ROM `playeroutofLB2a_strat`.
pub fn player_out_of_lb2a_strat(g: &mut Game, idx: u16) {
    let add = g.vars.sv_u8(sv::PLAYER_ZSTRATADD) as i8;
    g.vars
        .set_sv_u8(sv::PLAYER_ZSTRATADD, add.wrapping_sub(4) as u8);
    player_in_tunnel_strat(g, idx);
}

/// ROM `set_playerOutOfLB2_l` / init.
pub fn set_player_out_of_lb2(g: &mut Game, idx: u16) {
    g.vars.set_sv_i16(sv::OUTVX, -DEG90_256);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.pstratflags |= PSTF_INSEQ;
    playeronplanet_init(g, idx);
    // Re-apply sequence flags after planet init cleared them.
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.gameflags |= GF_NOZREMOVE;
    g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    g.vars.set_sv_i16(sv::OUTVX, -DEG90_256);
    g.vars.set_sv_i16(sv::BG2YSCROLL, 232 - 32);
    g.vars.gameflags &= !GF_STRATDONE1;
}

/// ROM `playerOutofLB2_strat`.
pub fn player_out_of_lb2_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz =
        g.objs.aliens[idx as usize].worldz.wrapping_add(MED_PSPEED);
}

/// ROM `set_playerOutOfLB3_l` / init.
pub fn set_player_out_of_lb3(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_space(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.gameflags |= GF_NOZREMOVE;
    g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    g.vars.gameflags &= !GF_STRATDONE1;
    g.vars.set_sv_i16(sv::BG2YSCROLL, 232 + 60);
}

/// ROM `playerOutofLB3_strat` — BG2YSCROLL ease + Z cruise (PCSTRATS player leaf).
pub fn player_out_of_lb3_strat(g: &mut Game, idx: u16) {
    let scroll = g.vars.sv_i16(sv::BG2YSCROLL);
    if scroll != 232 - 60 {
        g.vars.set_sv_i16(sv::BG2YSCROLL, scroll.wrapping_sub(1));
    }
    g.objs.aliens[idx as usize].worldz =
        g.objs.aliens[idx as usize].worldz.wrapping_add(MED_PSPEED);
}

/// BG2YSCROLL chase used by the player LB3 leaf (not ROM `viewlb3move_srou`).
pub fn view_lb3_move(g: &mut Game) {
    let scroll = g.vars.sv_i16(sv::BG2YSCROLL);
    if scroll != 232 - 60 {
        g.vars.set_sv_i16(sv::BG2YSCROLL, scroll.wrapping_sub(1));
    }
}

// ============================================================
// pshipoutofLB3 / viewoutofLB3 (GCSTRATS.ASM:1597-1826)
// ============================================================

const GF2_STRATFLAG1: u8 = 1;
const LE_ENDOFGAME: u8 = 6; // KALCS.INC
const LE_ENDTOTALSCORE: u8 = 9;

/// |worldz − viewposz| — HD stand-in for Zdist vs ROM `viewpt` camera.
fn zdist_viewpt(g: &Game, idx: u16) -> i32 {
    let wz = g.objs.aliens[idx as usize].worldz as i32;
    let vz = g.vars.sv_i16(sv::VIEWPOSZ) as i32;
    (wz - vz).abs()
}

/// ROM `viewlb3move_srou` (GCSTRATS.ASM:1821) — pin pviewpos to viewtoobj + Z cruise.
pub fn viewlb3move_srou(g: &mut Game) {
    let view = g.vars.sv_i16(sv::VIEWTOOBJ);
    if view < 0 || (view as usize) >= NUMBER_AL {
        return;
    }
    let (wx, wy, wz) = {
        let v = &g.objs.aliens[view as usize];
        (v.worldx, v.worldy, v.worldz)
    };
    g.vars.set_sv_i16(sv::PVIEWPOSX, wx);
    g.vars.set_sv_i16(sv::PVIEWPOSY, wy);
    g.vars
        .set_sv_i16(sv::PVIEWPOSZ, wz.wrapping_add(MED_PSPEED.wrapping_add(15)));
}

/// ROM `pshipoutofLB3_Istrat` (GCSTRATS.ASM:1597).
pub fn pshipoutoflb3_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipoutoflb3_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    // viewtoobj is map/caller-owned (ROM copies Y; HD leaves existing VIEWTOOBJ).
    g.vars
        .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    g.vars.gameflags &= !GF_STRATDONE1;
    g.objs.aliens[idx as usize].type_ |= ATGND;
    g.objs.aliens[idx as usize].stratstate = 0;
}

/// ROM `pshipoutofLB3_strat` (GCSTRATS.ASM:1604).
pub fn pshipoutoflb3_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            if g.vars.sv_u8(sv::VIEWTYPE) == VIEWTYPE_NORM {
                g.objs.aliens[idx as usize].stratstate = 1;
            } else {
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                    .worldz
                    .wrapping_add(MED_PSPEED.wrapping_add(19));
                if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
                    if zdist_viewpt(g, idx) < 500 {
                        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
                    } else {
                        g.objs.aliens[idx as usize].worldz =
                            g.objs.aliens[idx as usize].worldz.wrapping_add(11);
                    }
                }
                if zdist_viewpt(g, idx) <= 3000 {
                    g.objs.aliens[idx as usize].worldx =
                        g.objs.aliens[idx as usize].worldx.wrapping_add(1);
                }
            }
        }
        1 => {
            let gf2 = g.vars.shared.game_flags2;
            if gf2 & GF2_STRATFLAG1 != 0 {
                g.objs.aliens[idx as usize].stratstate = 2;
            } else {
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                    .worldz
                    .wrapping_add(MED_PSPEED.wrapping_add(19))
                    .wrapping_add(g.objs.aliens[idx as usize].sword1);
                g.vars.pviewvelz = MED_PSPEED.wrapping_add(19 - 3);
                if g.objs.aliens[idx as usize].sword1 != -3 && notdelay(g, 2) {
                    g.objs.aliens[idx as usize].sword1 =
                        g.objs.aliens[idx as usize].sword1.wrapping_sub(1);
                }
            }
        }
        2 => {
            g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                .worldz
                .wrapping_add(MED_PSPEED.wrapping_add(19 - 3));
            let sb = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte2 = sb;
            if sb == 0 {
                g.vars.pshipflags3 &= !PSF3_ENGINESND;
                g.hooks.play_music(0xf1); // bgm_fadeout stand-in
                g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
                g.hooks.play_se(0x32);
                g.objs.aliens[idx as usize].stratstate = 3;
                g.objs.aliens[idx as usize].sbyte1 = 15;
            }
        }
        3 => {
            g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                .worldz
                .wrapping_add(MED_PSPEED.wrapping_add(19 - 3));
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            if g.objs.aliens[idx as usize].vz != 150 {
                g.objs.aliens[idx as usize].vz = g.objs.aliens[idx as usize].vz.wrapping_add(15);
            }
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.objs.aliens[idx as usize].sbyte1 = 1;
                if g.objs.aliens[idx as usize].vy != -40 {
                    g.objs.aliens[idx as usize].vy =
                        g.objs.aliens[idx as usize].vy.wrapping_add(-5);
                }
                let mut rx = g.objs.aliens[idx as usize].rotx;
                crate::enemy_a::achase_angle(&mut rx, 0u8.wrapping_sub(DEG22), 3);
                g.objs.aliens[idx as usize].rotx = rx;
            }
            if zdist_viewpt(g, idx) >= 2000 {
                g.vars.gameflags &= !GF_NOZREMOVE;
                g.vars.gameflags |= GF_STRATDONE1;
            }
        }
        _ => {}
    }
}

/// ROM `viewoutofLB3_Istrat` (GCSTRATS.ASM:1675).
pub fn viewoutoflb3_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, viewoutoflb3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vel = MED_PSPEED as u8;
        al.sbyte1 = 50;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
        al.stratstate = 0;
    }
    let gf2 = g.vars.shared.game_flags2;
    g.vars.shared.game_flags2 = gf2 & !GF2_STRATFLAG1;
}

/// ROM `viewoutofLB3_strat` (GCSTRATS.ASM:1682) — close → swing → endgame camera.
pub fn viewoutoflb3_strat(g: &mut Game, idx: u16) {
    // Fall-through states (ROM ifnotstate chain).
    if g.objs.aliens[idx as usize].stratstate == 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            strat_gen_vecs_3d(al);
            strat_apply_velocity(al);
        }
        {
            let al = &g.objs.aliens[idx as usize];
            g.vars.set_sv_i16(sv::VIEWPOSX, al.worldx);
            g.vars.set_sv_i16(sv::VIEWPOSY, al.worldy);
            g.vars.set_sv_i16(sv::VIEWPOSZ, al.worldz);
        }
        let view = g.vars.sv_i16(sv::VIEWTOOBJ);
        let close = if view >= 0 && (view as usize) < NUMBER_AL {
            let me = g.objs.aliens[idx as usize];
            let t = g.objs.aliens[view as usize];
            (me.worldz as i32 - t.worldz as i32).abs() < 250
        } else {
            false
        };
        if close {
            let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MED_PSPEED as u8 + 15, 1);
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.objs.aliens[idx as usize].stratstate = 1;
                g.vars.set_sv_i16(sv::OUTDIST, 280);
                g.vars
                    .set_sv_i16(sv::OUTVY, ((-234i16).wrapping_mul(256)).wrapping_add(256));
                g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
                g.objs.aliens[idx as usize].sword1 = 230;
            }
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 1 {
        g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_add(1);
        let hi = (SINTAB[g.objs.aliens[idx as usize].sbyte3 as usize] as i16) / 4; // sintab,-2
        let target = (hi as i16) << 8;
        let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), target, 3);
        g.vars.set_sv_i16(sv::OUTVX, ovx);
        let ovy = g.vars.sv_i16(sv::OUTVY).wrapping_sub(256);
        g.vars.set_sv_i16(sv::OUTVY, ovy);
        viewlb3move_srou(g);
        if g.objs.aliens[idx as usize].sword1 >= 100 {
            let od = g.vars.sv_i16(sv::OUTDIST);
            if od <= 600 {
                g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(2));
            }
        } else {
            let od = g.vars.sv_i16(sv::OUTDIST);
            g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_sub(1));
        }
        let sw = g.objs.aliens[idx as usize].sword1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sword1 = sw;
        if sw == 0 {
            g.objs.aliens[idx as usize].stratstate = 2;
            g.objs.aliens[idx as usize].sbyte1 = 70;
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 2 {
        let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 5);
        let ovy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 5);
        g.vars.set_sv_i16(sv::OUTVX, ovx);
        g.vars.set_sv_i16(sv::OUTVY, ovy);
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.world.levelfinished = LE_ENDOFGAME;
            g.objs.aliens[idx as usize].stratstate = 3;
        }
        viewlb3move_srou(g);
    }

    if g.objs.aliens[idx as usize].stratstate == 3 {
        let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 3);
        let ovy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 3);
        g.vars.set_sv_i16(sv::OUTVX, ovx);
        g.vars.set_sv_i16(sv::OUTVY, ovy);
        viewlb3move_srou(g);
        if g.world.levelfinished == LE_ENDTOTALSCORE {
            g.objs.aliens[idx as usize].stratstate = 4;
            g.objs.aliens[idx as usize].sbyte1 = (256u16 - DEG45 as u16) as u8; // deg360-deg45
            g.objs.aliens[idx as usize].sbyte3 = 0;
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 4 {
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        let sb = g.objs.aliens[idx as usize].sbyte1;
        if sb == DEG22 {
            g.objs.aliens[idx as usize].stratstate = 5;
        } else {
            if sb == (0u8.wrapping_sub(DEG45).wrapping_sub(DEG90)) {
                g.hooks.play_se(0x0d);
            }
            let od = g.vars.sv_i16(sv::OUTDIST);
            g.vars.set_sv_i16(sv::OUTDIST, od.wrapping_add(1));
            g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_add(1);
            let hi = (SINTAB[g.objs.aliens[idx as usize].sbyte3 as usize] as i16) / 4;
            let target = hi << 8;
            let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), target, 3);
            g.vars.set_sv_i16(sv::OUTVX, ovx);
            let ovy = g.vars.sv_i16(sv::OUTVY).wrapping_sub(256);
            g.vars.set_sv_i16(sv::OUTVY, ovy);
            viewlb3move_srou(g);
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 5 {
        g.objs.aliens[idx as usize].sbyte1 = 60;
        g.objs.aliens[idx as usize].stratstate = 6;
    }

    if g.objs.aliens[idx as usize].stratstate == 6 {
        let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 5);
        let ovy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 5);
        let od = strat_chase_proportional(g.vars.sv_i16(sv::OUTDIST), 100, 5);
        g.vars.set_sv_i16(sv::OUTVX, ovx);
        g.vars.set_sv_i16(sv::OUTVY, ovy);
        g.vars.set_sv_i16(sv::OUTDIST, od);
        viewlb3move_srou(g);
        let vpz = g.vars.sv_i16(sv::VIEWPOSZ);
        g.vars
            .set_sv_i16(sv::VIEWPOSZ, vpz.wrapping_add(MED_PSPEED.wrapping_add(15)));
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].sbyte1 = 1;
            let gf2 = g.vars.shared.game_flags2;
            g.vars.shared.game_flags2 = gf2 | GF2_STRATFLAG1;
            g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS);
        }
    }
}

// ============================================================
// pshipOutofLB1 / viewOutofLB1 (GCSTRATS.ASM:1246-1527)
// ============================================================

const SH_MY_DEMOS: u16 = 224; // my_demos
const SH_MY_DEMO_BS: u16 = 410;
const SH_MY_DEMO_S: u16 = 411;
const SH_LAST_B_0: u16 = 212;
const SH_LAST_B_3: u16 = 214;

/// `((deg180+deg11+deg5)/3)+1` — turn duration after base remove.
const PSHIP_LB1_TURN_FRAMES: u8 = ((128u16 + 8 + 4) / 3 + 1) as u8;

fn find_shape_and_remove(g: &mut Game, shape: u16) {
    for i in 0..NUMBER_AL {
        if g.objs.aliens[i].active && g.objs.aliens[i].shape == shape {
            g.objs.free(i as u16);
            return;
        }
    }
}

fn spawn_shiplb1_friend(
    g: &mut Game,
    src: u16,
    ox: i16,
    oy: i16,
    oz: i16,
    sword1: i16,
    sbyte1: u8,
) {
    let Some(y) = strat_make_obj(g, SH_MY_DEMOS) else {
        return;
    };
    {
        let s = g.objs.aliens[src as usize];
        let al = &mut g.objs.aliens[y as usize];
        al.worldx = s.worldx.wrapping_add(ox);
        al.worldy = s.worldy.wrapping_add(oy);
        al.worldz = s.worldz.wrapping_add(oz);
        al.rotx = s.rotx;
        al.roty = s.roty;
        al.rotz = s.rotz;
        al.vx = s.vx;
        al.vy = s.vy;
        al.vz = s.vz;
        al.sword1 = sword1;
        al.sbyte1 = sbyte1;
        al.stratstate = 0;
    }
    shiplb1_istrat(g, y);
}

/// ROM `pshipOutofLB1_Istrat` (GCSTRATS.ASM:1246).
pub fn pshipoutoflb1_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipoutoflb1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        // This cinematic owns DM_END's completion signal. Its final turn
        // briefly places the ship origin just behind the near plane before
        // the countdown publishes that signal, so retain the object for the
        // complete authored sequence.
        al.type_ &= !ATZREMOVE;
        al.vel = MED_PSPEED as u8;
        al.rotx = 0u8.wrapping_sub(DEG90);
        al.stratstate = 0;
    }
    g.vars.set_sv_i16(sv::VIEWTOOBJ, idx as i16);
    g.vars.gameflags &= !GF_STRATDONE1;

    // The initializer label falls directly into pshipOutofLB1_strat in the
    // source, so creation includes the first movement tick.
    pshipoutoflb1_strat(g, idx);
}

/// ROM `pshipOutofLB1_strat` (GCSTRATS.ASM:1254).
pub fn pshipoutoflb1_strat(g: &mut Game, idx: u16) {
    // Fall-through ifnotstate chain + nextstate re-entry for state 2→3.
    'strat: loop {
        if g.objs.aliens[idx as usize].stratstate == 0 {
            g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_sub(4);
            // s_jmp_lower x,#-1000,.nsup — branch away when worldy >= -1000
            if g.objs.aliens[idx as usize].worldy < -1000 {
                let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MAX_PSPEED as u8, 1);
                g.objs.aliens[idx as usize].sbyte1 = 5 + 18;
                if g.objs.aliens[idx as usize].rotx == 0u8.wrapping_sub(DEG45) {
                    g.hooks.play_music(0x07);
                    g.objs.aliens[idx as usize].stratstate = 1;
                    continue 'strat;
                }
                g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(2);
            }
        }

        if g.objs.aliens[idx as usize].stratstate == 1 {
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.objs.aliens[idx as usize].sbyte1 = 1;
                g.objs.aliens[idx as usize].shape = SH_MY_DEMO_BS;
            }
            let mut rz = g.objs.aliens[idx as usize].rotz;
            let mut rx = g.objs.aliens[idx as usize].rotx;
            crate::enemy_a::achase_angle(&mut rz, 0, 3);
            let lined_up = crate::enemy_a::achase_angle(&mut rx, 0, 5);
            g.objs.aliens[idx as usize].rotz = rz;
            g.objs.aliens[idx as usize].rotx = rx;
            if lined_up {
                g.objs.aliens[idx as usize].shape = SH_MY_DEMO_S;
                g.objs.aliens[idx as usize].sbyte1 = 70;
                g.objs.aliens[idx as usize].stratstate = 2;
            }
        }

        if g.objs.aliens[idx as usize].stratstate == 2 {
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.objs.aliens[idx as usize].stratstate = 3;
                g.objs.aliens[idx as usize].sbyte1 = PSHIP_LB1_TURN_FRAMES;
                // nextstate → fall into state 3 same frame after .nend? ROM brl .nend
                // so state 3 runs next frame only. Break to .nend.
                break 'strat;
            }
        }

        if g.objs.aliens[idx as usize].stratstate == 3 {
            find_shape_and_remove(g, SH_LAST_B_0);
            find_shape_and_remove(g, SH_LAST_B_3);
            if g.vars.frog_hp != 0 {
                spawn_shiplb1_friend(g, idx, 450 + 400, -1000, 250, 30, 30 + 20);
            }
            if g.vars.falcon_hp != 0 {
                spawn_shiplb1_friend(g, idx, 450 + 200 + 400, 50, 500, 70, 30 + 10 + 5 + 20 + 2);
            }
            if g.vars.bunny_hp != 0 {
                spawn_shiplb1_friend(g, idx, 450 + 600, 1000, 400, 50, 30 + 30 + 2);
            }
            g.objs.aliens[idx as usize].stratstate = 4;
        }

        if g.objs.aliens[idx as usize].stratstate == 4 {
            g.objs.aliens[idx as usize].vel = 10;
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.objs.aliens[idx as usize].sbyte1 = 110;
                g.objs.aliens[idx as usize].stratstate = 5;
                continue 'strat; // nextstate
            }
            g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(3);
            if g.objs.aliens[idx as usize].rotz != DEG45 && notdelay(g, 1) {
                g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(1);
            }
        }

        if g.objs.aliens[idx as usize].stratstate == 5 {
            g.objs.aliens[idx as usize].vel = 15;
            if g.objs.aliens[idx as usize].sbyte1 <= 60 && notdelay(g, 1) {
                g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(1);
            }
            g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS);
            let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte1 = sb;
            if sb == 0 {
                g.vars.gameflags |= GF_STRATDONE1;
            }
        }

        break;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
    }
}

/// ROM `viewOutofLB1_Istrat` (GCSTRATS.ASM:1380).
pub fn viewoutoflb1_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, viewoutoflb1_strat);
    let outdist = g.vars.sv_i16(sv::OUTDIST);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        al.sword1 = outdist; // Z offset
        al.sword2 = 0; // Y offset
        al.ptr = 0; // X offset
        al.rotx = 0u8.wrapping_sub(DEG45);
    }
    g.vars
        .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);
    let gf2 = g.vars.shared.game_flags2;
    g.vars.shared.game_flags2 = gf2 & !GF2_STRATFLAG1;

    // The source initializer falls through to viewOutofLB1_strat and
    // publishes/moves the camera on its creation frame.
    viewoutoflb1_strat(g, idx);
}

/// ROM `viewOutofLB1_strat` (GCSTRATS.ASM:1394) — follow pship; explode mapvar1.
pub fn viewoutoflb1_strat(g: &mut Game, idx: u16) {
    const LAST_STAGE_TRIGGER_HEIGHT: i16 = -1000;
    const LAST_STAGE_CIRCLE_SOUND: u8 = 29;

    // Explosion burst when gf2_stratflag1 and !sflag3
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG3 == 0 {
        let gf2 = g.vars.shared.game_flags2;
        if gf2 & GF2_STRATFLAG1 != 0 {
            if let Some(parent) = mapvar1_obj(g) {
                if let Some(y) = make_large_exp_obj(g, parent) {
                    addrnd2pos_xy(g, y);
                    g.objs.aliens[y as usize].sflags2 &= !ASF2_RELEXPLODE;
                    g.objs.aliens[y as usize].sflags4 |= ASF4_NOPOLYEXP;
                    g.objs.aliens[y as usize].worldy =
                        g.objs.aliens[y as usize].worldy.wrapping_sub(400);
                }
                if let Some(y) = make_large_exp_obj(g, parent) {
                    g.objs.aliens[y as usize].sflags2 &= !ASF2_RELEXPLODE;
                    g.objs.aliens[y as usize].sflags4 |= ASF4_NOPOLYEXP;
                    addrnd2pos_xy(g, y);
                    g.objs.aliens[y as usize].worldy =
                        g.objs.aliens[y as usize].worldy.wrapping_sub(800);
                }
            }
        }
    }

    let view = g.vars.sv_i16(sv::VIEWTOOBJ);
    if view >= 0 && (view as usize) < NUMBER_AL {
        let vstate = g.objs.aliens[view as usize].stratstate;
        match vstate {
            0 => {
                let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], (MAX_PSPEED - 5) as u8, 2);
            }
            1 => {
                let mut rx = g.objs.aliens[idx as usize].rotx;
                crate::enemy_a::achase_angle(&mut rx, 0, 5);
                g.objs.aliens[idx as usize].rotx = rx;
            }
            4 => {
                g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG3;
                g.objs.aliens[idx as usize].vel = 0;
            }
            _ => {}
        }

        // Base explosion starts once the viewed ship climbs past the authored
        // negative height. Its anchor begins at mapvar1 and follows this camera
        // object's vertical velocity on every later tick.
        let move_circle_anchor = if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
            true
        } else if g.objs.aliens[view as usize].worldy >= LAST_STAGE_TRIGGER_HEIGHT {
            false
        } else {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            if let Some(parent) = mapvar1_obj(g) {
                if let Some(anchor) = strat_make_obj(g, 0) {
                    copy_pos(g, anchor, parent);
                    let object_id = anchor + 1;
                    g.vars.strategy.circle_object = object_id as i16;
                    g.vars
                        .screen_fill_circle
                        .begin_last_stage(ScreenFillCircleCenter::Object(object_id));
                    g.hooks.play_se(LAST_STAGE_CIRCLE_SOUND);
                }
            }
            true
        };

        if move_circle_anchor {
            let object_id = g.vars.strategy.circle_object;
            if object_id > 0 {
                let anchor = object_id as u16 - 1;
                if (anchor as usize) < NUMBER_AL && g.objs.aliens[anchor as usize].active {
                    let vertical_velocity = g.objs.aliens[idx as usize].vy;
                    let object = &mut g.objs.aliens[anchor as usize];
                    object.worldy = object.worldy.wrapping_add(vertical_velocity);
                }
            }
        }
    }

    {
        let al = &g.objs.aliens[idx as usize];
        g.vars.set_sv_i16(sv::VIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::VIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::VIEWPOSZ, al.worldz);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
    }
}

// ============================================================
// pshipIntoLB1 / viewIntoLB1 (GISTRATS.ASM:340-499)
// ============================================================

/// `maxpspeed - (inviewdist + maxpspeed)` = −inviewdist.
const VIEW_INTOLB1_SWORD2_TARGET: i16 = -CLOSE_VIEW_DISTANCE;

/// Map object variables use the ROM/C `index + 1` encoding so zero remains
/// the invalid/null value. `s_set_objtobevar` decodes that representation.
fn mapvar1_obj(g: &Game) -> Option<u16> {
    let encoded = g.vars.map.variable1 as u16;
    let idx = encoded.checked_sub(1)?;
    ((idx as usize) < NUMBER_AL && g.objs.aliens[idx as usize].active).then_some(idx)
}

/// ROM `pshipIntoLB1_Istrat` (GISTRATS.ASM:340).
pub fn pshipintolb1_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipintolb1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        al.vel = MED_PSPEED as u8;
        al.sbyte1 = DEG90 / 2; // 32
        al.stratstate = 0;
    }
}

/// ROM `pshipIntoLB1_strat` (GISTRATS.ASM:346) — climb/roll into L-tunnel.
pub fn pshipintolb1_strat(g: &mut Game, idx: u16) {
    // Fall-through ifnotstate; state 2→4 handoff can complete same frame.
    if g.objs.aliens[idx as usize].stratstate == 0 {
        let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MIN_PSPEED as u8, 1);
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(2);
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb != 0 {
            // stay in .nsup
        } else {
            g.objs.aliens[idx as usize].stratstate = 1;
            g.objs.aliens[idx as usize].sbyte1 = DEG180 / 2; // 64
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 1 {
        if g.objs.aliens[idx as usize].sbyte1 == 10 {
            let gf2 = g.vars.shared.game_flags2;
            g.vars.shared.game_flags2 = gf2 | GF2_STRATFLAG1;
        }
        g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(8);
        let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MIN_PSPEED as u8, 1);
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(2);
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].stratstate = 2;
            g.objs.aliens[idx as usize].vel = MAX_PSPEED as u8;
            g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
            g.hooks.play_se(0x32);
            g.objs.aliens[idx as usize].sbyte1 = 40;
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 2 {
        if let Some(mapvar) = mapvar1_obj(g) {
            let tx = g.objs.aliens[mapvar as usize].worldx;
            let tz = g.objs.aliens[mapvar as usize].worldz;
            let wx = g.objs.aliens[idx as usize].worldx;
            let wz = g.objs.aliens[idx as usize].worldz;
            g.objs.aliens[idx as usize].worldz = strat_chase_proportional(wz, tz, 4);
            g.objs.aliens[idx as usize].worldx = strat_chase_proportional(wx, tx, 4);
        }
        let mut rz = g.objs.aliens[idx as usize].rotz;
        crate::enemy_a::achase_angle(&mut rz, 0, 3);
        g.objs.aliens[idx as usize].rotz = rz;
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].stratstate = 4;
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 4 {
        let p = g.vars.internal_playpt;
        if p >= 0 && (p as usize) < NUMBER_AL {
            let pidx = p as u16;
            set_player_in_ltunnel(g, pidx);
            g.objs.aliens[pidx as usize].sflags &= !ASF_INVISIBLE;
            g.vars.set_sv_i16(sv::VIEWTOOBJ, pidx as i16);
            g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
            g.vars.pviewvelz = MAX_PSPEED;
            g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);
            g.vars.set_sv_i16(sv::OUTDIST, CLOSE_VIEW_DISTANCE);
            g.vars.viewdist = CLOSE_VIEW_DISTANCE;
            g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
            g.objs.aliens[pidx as usize].vel = MAX_PSPEED as u8;
            g.objs.aliens[pidx as usize].sbyte2 = 1; // boost delay
            g.objs.aliens[pidx as usize].worldy = LTUNNEL_VIEWCY - 5;
            g.objs.aliens[pidx as usize].worldx = 0;
            let wz = g.objs.aliens[pidx as usize].worldz;
            g.vars.player_posz = wz;
            g.vars
                .set_sv_i16(sv::PVIEWPOSZ, wz.wrapping_sub(MAX_PSPEED));
            g.vars.set_sv_i16(sv::PVIEWPOSY, LTUNNEL_VIEWCY);
            g.vars.set_sv_i16(sv::PVIEWPOSX, 0);
            g.vars.set_sv_i16(sv::OUTVX, 0);
            g.vars.set_sv_i16(sv::OUTVY, 0);
            g.vars.set_sv_i16(sv::OUTVZ, 0);
            g.vars.gameflags &= !GF_NOZREMOVE;
            g.vars.shared.float_variables = [0; 2];
        }
        g.objs.aldead = 1;
        return;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
    }
}

/// ROM `viewIntoLB1_Istrat` (GISTRATS.ASM:434).
pub fn viewintolb1_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, viewintolb1_strat);
    let outdist = g.vars.sv_i16(sv::OUTDIST);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        al.vel = MED_PSPEED as u8;
        al.sbyte1 = DEG90 / 3; // 21
        al.sword1 = outdist; // Z offset
        al.sword2 = 0; // Y offset
        al.ptr = 0; // X offset (signed via i16 cast)
    }
    // 50% chance set sflag1 (left/right drift).
    if (sf_random(&mut g.vars) as u8) >= ((50u16 * 255) / 100) as u8 {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    }
}

/// ROM `viewIntoLB1_strat` (GISTRATS.ASM:449) — camera offsets follow pship state.
pub fn viewintolb1_strat(g: &mut Game, idx: u16) {
    let view = g.vars.sv_i16(sv::VIEWTOOBJ);
    if view >= 0 && (view as usize) < NUMBER_AL {
        let vstate = g.objs.aliens[view as usize].stratstate;
        match vstate {
            0 => {
                g.objs.aliens[idx as usize].sword1 =
                    g.objs.aliens[idx as usize].sword1.wrapping_add(4);
                let mut px = g.objs.aliens[idx as usize].ptr as i16;
                if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
                    px = px.wrapping_sub(2);
                } else {
                    px = px.wrapping_add(2);
                }
                g.objs.aliens[idx as usize].ptr = px as u16;
            }
            1 => {
                let s1 = g.objs.aliens[idx as usize].sword1;
                g.objs.aliens[idx as usize].sword1 = strat_chase_proportional(s1, 5, 4);
                g.objs.aliens[idx as usize].sword2 =
                    g.objs.aliens[idx as usize].sword2.wrapping_sub(8);
            }
            2 => {
                let s2 = g.objs.aliens[idx as usize].sword2;
                g.objs.aliens[idx as usize].sword2 =
                    strat_chase_proportional(s2, VIEW_INTOLB1_SWORD2_TARGET, 4);
                let mut px = g.objs.aliens[idx as usize].ptr as i16;
                px = strat_chase_proportional(px, 0, 3);
                g.objs.aliens[idx as usize].ptr = px as u16;
            }
            3 => {}
            4 => {
                g.objs.aldead = 1;
                return;
            }
            _ => {}
        }

        let src = g.objs.aliens[view as usize];
        let s1 = g.objs.aliens[idx as usize].sword1;
        let s2 = g.objs.aliens[idx as usize].sword2;
        let px = g.objs.aliens[idx as usize].ptr as i16;
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = src.worldx.wrapping_add(px);
            al.worldy = src.worldy.wrapping_add(s2);
            al.worldz = src.worldz.wrapping_sub(s1);
        }
    }

    {
        let al = &g.objs.aliens[idx as usize];
        g.vars.set_sv_i16(sv::VIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::VIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::VIEWPOSZ, al.worldz);
        g.vars.set_sv_i16(sv::BGSSCROLLZ, al.worldy);
    }
}

// ============================================================
// pshipDIVEGND / viewDIVEGND (GCSTRATS.ASM:1037-1190)
// ============================================================

const PLANET_VIEW_CY: i16 = -50 - 105 - 60; // STRATEQU.INC:558 = -215
const PSHIP_DIVEGND_Y: i16 = -2853 + 105 + PLANET_VIEW_CY; // -2963
const VIEW_DIVEGND_Y: i16 = -2692 + 105 + PLANET_VIEW_CY; // -2802
const PSHIP_DIVEGND_COUNT: u8 = 50 - 16 + 20; // 54

fn divegnd_target(g: &Game, idx: u16) -> Option<u16> {
    let t = g.objs.aliens[idx as usize].sword1;
    if t >= 0 && (t as usize) < NUMBER_AL && g.objs.aliens[t as usize].active {
        Some(t as u16)
    } else {
        None
    }
}

fn divegnd_zdist(a: &sf_game::alien::Alien, b: &sf_game::alien::Alien) -> i32 {
    (a.worldz as i32 - b.worldz as i32).abs()
}

/// ROM `pshipDIVEGND_Istrat` (GCSTRATS.ASM:1037).
pub fn pshipdivegnd_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, pshipdivegnd_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = DEG90;
        al.stratptr = Some(tick);
        al.vel = 50;
        al.worldy = PSHIP_DIVEGND_Y;
        al.sflags |= ASF_SHADOW;
        al.type_ &= !ATZREMOVE;
        al.sbyte1 = PSHIP_DIVEGND_COUNT;
    }
}

/// ROM `pshipDIVEGND_strat` (GCSTRATS.ASM:1048) — dive then hand off to on-planet.
pub fn pshipdivegnd_strat(g: &mut Game, idx: u16) {
    if let Some(view) = divegnd_target(g, idx) {
        let dz = {
            let me = &g.objs.aliens[idx as usize];
            let v = &g.objs.aliens[view as usize];
            divegnd_zdist(me, v)
        };
        if dz >= OUTVIEWDIST as i32 {
            // Far from view cam → remove both, restore player on planet.
            g.objs.free(view);
            g.objs.aldead = 1;
            let p = g.vars.internal_playpt;
            if p >= 0 && (p as usize) < NUMBER_AL {
                let pidx = p as u16;
                playeronplanet_init(g, pidx);
                {
                    let src = g.objs.aliens[idx as usize];
                    let pl = &mut g.objs.aliens[pidx as usize];
                    pl.worldx = src.worldx;
                    pl.worldy = src.worldy;
                    pl.worldz = src.worldz;
                    pl.sflags &= !ASF_INVISIBLE;
                    pl.vel = MED_PSPEED as u8;
                }
            }
            g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
            g.vars.pviewvelz = MED_PSPEED;
            g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
            g.vars.set_sv_i16(sv::OUTDIST, OUTVIEWDIST);
            g.vars.viewdist = OUTVIEWDIST;
            g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
            return;
        }
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
    }

    // s_beqdec → .fin2
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_TOOBJ);
        divegnd_fin(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;

    if g.objs.aliens[idx as usize].sbyte1 < 20 {
        divegnd_fin(g, idx);
        return;
    }

    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_sub(8);
    let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], 20, 1);
}

fn divegnd_fin(g: &mut Game, idx: u16) {
    g.vars.gameflags &= !GF_NOZREMOVE;
    let mut rx = g.objs.aliens[idx as usize].rotx;
    crate::enemy_a::achase_angle(&mut rx, 0, 5);
    g.objs.aliens[idx as usize].rotx = rx;
    let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MED_PSPEED as u8, 1);
    let mut rz = g.objs.aliens[idx as usize].rotz;
    crate::enemy_a::achase_angle(&mut rz, 0, 4);
    g.objs.aliens[idx as usize].rotz = rz;
}

/// ROM `viewDIVEGND_Istrat` (GCSTRATS.ASM:1104).
pub fn viewdivegnd_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, viewdivegnd_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.type_ &= !ATZREMOVE;
        al.worldy = VIEW_DIVEGND_Y;
        al.vel = 20;
        al.rotx = DEG90;
        al.sbyte1 = 90;
    }
    g.vars.set_sv_i16(sv::OUTVX, -DEG90_256);
    g.vars.set_sv_i16(sv::OUTVZ, 0);
}

/// ROM `viewDIVEGND_strat` (GCSTRATS.ASM:1116) — camera dive tracking pship.
pub fn viewdivegnd_strat(g: &mut Game, idx: u16) {
    if let Some(ship) = divegnd_target(g, idx) {
        let svz = g.objs.aliens[ship as usize].vz;
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(svz);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_3d(al);
        // ROM only adds vy to worldy (not full add_vecs2pos).
        al.worldy = al.worldy.wrapping_add(al.vy);
    }
    {
        let al = &g.objs.aliens[idx as usize];
        g.vars.set_sv_i16(sv::PVIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::PVIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::PVIEWPOSZ, al.worldz);
    }
    let p = g.vars.internal_playpt;
    if p >= 0 && (p as usize) < NUMBER_AL {
        let src = g.objs.aliens[idx as usize];
        let pl = &mut g.objs.aliens[p as usize];
        pl.worldx = src.worldx;
        pl.worldy = src.worldy;
        pl.worldz = src.worldz;
    }

    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(3);
        viewdivegnd_fin(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;

    if g.objs.aliens[idx as usize].sbyte1 < 40 {
        viewdivegnd_fin(g, idx);
        return;
    }

    let ovz = g.vars.sv_i16(sv::OUTVZ).wrapping_sub(4 * 256);
    g.vars.set_sv_i16(sv::OUTVZ, ovz);
}

fn viewdivegnd_fin(g: &mut Game, idx: u16) {
    let mut rx = g.objs.aliens[idx as usize].rotx;
    crate::enemy_a::achase_angle(&mut rx, 0, 5);
    g.objs.aliens[idx as usize].rotx = rx;
    let _ = strat_speed_to(&mut g.objs.aliens[idx as usize], MED_PSPEED as u8, 1);
    let ovz = strat_chase_proportional(g.vars.sv_i16(sv::OUTVZ), 0, 3);
    g.vars.set_sv_i16(sv::OUTVZ, ovz);
}

/// ROM `set_playerTunneltoOnPlanet_l` / init.
pub fn set_player_tunnel_to_on_planet(g: &mut Game, idx: u16) {
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    playeronplanet_init(g, idx);
    g.vars.set_sv_i16(sv::VIEWCY, -60); // LTexit_viewCY
}

/// ROM `playerTunneltoOnPlanet_strat` — chase viewCY to planet; then onplanet.
pub fn player_tunnel_to_on_planet_strat(g: &mut Game, idx: u16) -> bool {
    let cy = g.vars.sv_i16(sv::VIEWCY);
    let next = strat_chase_proportional(cy, -50, 4); // planet_viewCY
    g.vars.set_sv_i16(sv::VIEWCY, next);
    if next == -50 {
        playeronplanet_init(g, idx);
        return true;
    }
    // Continue as on-planet flight while viewCY eases.
    strat_player(g, idx);
    false
}

/// ROM `set_playerDIVEGND_l` / init (cutscene dive; stayblack gate).
pub fn set_player_dive_gnd(g: &mut Game, idx: u16) {
    let stay = g.vars.sv_i8(sv::STAYBLACK);
    if stay > 11 {
        playeronplanet_init(g, idx);
        return;
    }
    g.vars.gameflags |= GF_NOZREMOVE;
    playeronplanet_init(g, idx);
    g.vars.gameflags &= !GF_VIEWROT;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let player_tick = ea_sid(g, player_divegnd_strat);
    g.objs.aliens[idx as usize].stratptr = Some(player_tick);

    if let Some(ship) = dupplayer(g, idx) {
        {
            let duplicate = &mut g.objs.aliens[ship as usize];
            duplicate.worldx = 0;
            duplicate.worldy = 0;
            duplicate.worldz = 0;
        }
        pshipdivegnd_istrat(g, ship);
        g.vars.set_sv_i16(sv::VIEWTOOBJ, ship as i16);

        if let Some(view) = strat_make_obj(g, 0) {
            {
                let camera = &mut g.objs.aliens[view as usize];
                camera.worldx = 0;
                camera.worldy = 0;
                camera.worldz = 0;
                camera.sword1 = ship as i16;
            }
            viewdivegnd_istrat(g, view);
            g.objs.aliens[ship as usize].sword1 = view as i16;
        }
    }
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = 0;
    g.objs.aliens[idx as usize].worldz = 0;
    g.world.lastplayz = 0;
    g.vars.set_sv_i16(sv::OUTVX, 0);
}

/// ROM `set_playercred_l` / `playercred_Istrat` (PSTRATS.ASM:565).
pub fn set_player_cred(g: &mut Game, idx: u16) {
    player_cred_istrat(g, idx);
}

/// ROM `playercred_Istrat`.
pub fn player_cred_istrat(g: &mut Game, idx: u16) {
    g.world.lastplayz = 0;
    g.vars.set_sv_i16(sv::VIEWPOSZ, 0);
    g.vars.set_sv_i16(sv::PLAYER_TURNROT, 0);
    g.vars.gameflags &= !GF_NOZREMOVE;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldy = 0;
        al.worldz = 0;
        al.sflags |= ASF_INVISIBLE;
        al.vel = MED_PSPEED as u8;
    }
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::OUTDIST, OUTVIEWDIST);
    g.vars.viewdist = OUTVIEWDIST;
    g.vars.set_sv_i16(sv::PVIEWPOSX, 0);
    g.vars.set_sv_i16(sv::PVIEWPOSY, 0);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, 0);
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_space(g, idx);
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
    g.vars.pviewvelz = MED_PSPEED;
    g.vars.set_sv_i16(sv::PLROTZ, 0);
    let s = ea_sid(g, player_cred_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// ROM `playercred_strat`.
pub fn player_cred_strat(g: &mut Game, idx: u16) {
    do_player_limit_x(g, idx);
    viewmove_srou(g, idx);
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    g.vars.set_sv_i16(sv::OUTVZ, 0);
    g.vars.set_sv_i16(sv::PLROTZ, 0);
}

/// ROM `set_playerIntoLB1_l` — start chase toward mapvar1 then LB1 cutscene.
pub fn set_player_into_lb1(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_into_lb1a_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.internal_playpt = idx as i16;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
}

/// ROM `playerIntoLB1a_strat` — chase center; hand off when Z-near map target.
pub fn player_into_lb1a_strat(g: &mut Game, idx: u16) {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 4);
    }
    if let Some(target) = mapvar1_obj(g) {
        let dz = g.objs.aliens[idx as usize].worldz as i32
            - g.objs.aliens[target as usize].worldz as i32;
        if dz.abs() < 1785 {
            player_into_lb1_istrat(g, idx);
            return;
        }
    }
    strat_player(g, idx);
}

/// ROM `playerIntoLB1_Istrat` (PISTRATS.ASM:770-794): hide the real player,
/// create the cinematic ship and camera objects, then let the real player
/// continue scrolling while the duplicate performs the entrance maneuver.
pub fn player_into_lb1_istrat(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_into_lb1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = None;
    }
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags &= !GF_STRATDONE1;
    g.vars.playerflymode &= !PFM_WOBBLE;
    g.vars.pstratflags |= PSTF_INSEQ;

    if let Some(ship) = dupplayer(g, idx) {
        pshipintolb1_istrat(g, ship);
        g.vars.set_sv_i16(sv::VIEWTOOBJ, ship as i16);
        g.vars
            .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_FPOS | VIEWTYPE_TOOBJ);

        if let Some(view) = strat_make_obj(g, 0) {
            let src = g.objs.aliens[idx as usize];
            let dst = &mut g.objs.aliens[view as usize];
            dst.worldx = src.worldx;
            dst.worldy = src.worldy;
            dst.worldz = src.worldz;
            viewintolb1_istrat(g, view);
        }
    }

    g.vars.gameflags |= GF_NOZREMOVE;
    player_into_lb1_strat(g, idx);
}

/// ROM `playerIntoLB1_strat`: the hidden real player remains the map-scroll
/// anchor while its cinematic duplicate flies into the base.
pub fn player_into_lb1_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz =
        g.objs.aliens[idx as usize].worldz.wrapping_add(MED_PSPEED);
}

// ============================================================
// Cutscene phase-2 inits (PCSTRATS.ASM / PSTRATS.ASM)
// ============================================================

/// ROM `playerstart_init_l` — reset ship flags + select norm ship + nuke count.
pub fn player_start_init(g: &mut Game) {
    g.vars.reset_player_run_state();
    select_ship(g, PSHIPNUM_NORM);
}

/// ROM `playermove_init_l` — view/play pointers + outviewdist.
pub fn player_move_init(g: &mut Game, idx: u16) {
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    g.vars.set_sv_i16(sv::VIEWTOOBJ, idx as i16);
    g.vars.internal_playpt = idx as i16;
    g.vars.viewdist = OUTVIEWDIST;
    g.vars.set_sv_i16(sv::OUTDIST, OUTVIEWDIST);
}

/// ROM `playerCHASE2_init` — dup + silence engines; continue as space.
pub fn player_chase2_init(g: &mut Game, idx: u16) -> Option<u16> {
    let tick = ea_sid(g, player_chase2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    dupplayer(g, idx)
}

/// ROM `playerClearTurn2_init`.
pub fn player_clear_turn2_init(g: &mut Game, idx: u16) -> Option<u16> {
    let tick = ea_sid(g, player_clear_turn2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    let dup = dupplayer(g, idx)?;
    // clshipTurn sbyte1 = (deg180+deg45+deg22+deg11)/4 = (128+32+16+8)/4 = 46
    g.objs.aliens[dup as usize].sbyte1 = 46;
    g.vars.set_sv_i16(sv::OUTVY, 0);
    Some(dup)
}

/// ROM `playerUNDER2_init`.
pub fn player_under2_init(g: &mut Game, idx: u16) -> Option<u16> {
    let tick = ea_sid(g, player_under2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    dupplayer(g, idx)
}

/// ROM `playerwarp1_init` (PCSTRATS.ASM:907) — dupplayer → clshipboostnosnd.
pub fn player_warp1_init(g: &mut Game, idx: u16) -> Option<u16> {
    let tick = ea_sid(g, player_warp1_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.hooks.play_music(0xe);
    let dup = dupplayer(g, idx)?;
    g.objs.aliens[dup as usize].sbyte2 = 19;
    crate::enemy_a::clshipboostnosnd_istrat(g, dup);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 20);
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    Some(dup)
}

/// ROM `playerwarp2_init` — hand off to space strat (no extra state).
pub fn player_warp2_init(_g: &mut Game, _idx: u16) {}

/// ROM `playerDIVE2_init`.
pub fn player_dive2_init(g: &mut Game, idx: u16) -> Option<u16> {
    let tick = ea_sid(g, player_dive2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    dupplayer(g, idx)
}

/// ROM `playercleardemo2_init`.
pub fn player_clear_demo2_init(g: &mut Game, idx: u16) {
    let tick = ea_sid(g, player_clear_demo2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 0);
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, 250);
}

/// C `dupplayer` (PSTRATS.ASM:3516-3525): copy of the player ship object;
/// the player goes invisible, the duplicate gets colldisable+shadow.
fn dupplayer(g: &mut Game, idx: u16) -> Option<u16> {
    let dup = strat_make_obj(g, 0)?;
    let src = g.objs.aliens[idx as usize];
    {
        let d = &mut g.objs.aliens[dup as usize];
        d.shape = src.shape;
        d.worldx = src.worldx;
        d.worldy = src.worldy;
        d.worldz = src.worldz;
        d.rotx = src.rotx;
        d.roty = src.roty;
        d.rotz = src.rotz;
        d.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
    }
    g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    Some(dup)
}

/// C `playeronbridge_strat` (PSTRATS.ASM:1002-1027): resumes normal bridge
/// control: do_player_bridge, vy/=2, perc62 view, viewmove.
fn playeronbridge_strat(g: &mut Game, idx: u16) {
    // Restore player control and fly mode (bridge).
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    g.vars.pstratflags &= !(PSTF_INSEQ | PSTF_NOVDISTC);

    // do_player_bridge -> same as do_player_yvelD2
    do_player_yvel_d2(g, idx);

    // view X = perc62(worldx), view Y = viewCY
    let wx = g.objs.aliens[idx as usize].worldx;
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    g.vars.set_sv_i16(sv::PVIEWPOSX, strat_perc62(wx));
    g.vars.set_sv_i16(sv::PVIEWPOSY, view_cy);

    viewmove_srou(g, idx);
}

/// C `playerclearbridge2_init` (PCSTRATS.ASM:77-83).
fn playerclearbridge2_init(g: &mut Game, idx: u16) {
    // dupplayer — create wingman copy for bridge boost
    if let Some(dup) = dupplayer(g, idx) {
        // s_set_strat y,clshipbridgeboost_Istrat
        let boost = ea_sid(g, crate::enemy_a::clship_bridgeboost_istrat);
        g.objs.aliens[dup as usize].stratptr = Some(boost);
    }
    // s_set_strat x,playerclearbridge3_strat
    let id3 = sid(g, K_CLEARBRIDGE3_STRAT);
    g.objs.aliens[idx as usize].stratptr = Some(id3);
    playerclearbridge3_strat(g, idx);
}

/// C `playerclearbridge3_strat` (PCSTRATS.ASM:83-86).
fn playerclearbridge3_strat(g: &mut Game, idx: u16) {
    // s_add_alvar W,x,al_worldx,#3
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(3);
    // s_jmp playeronbridge_strat
    playeronbridge_strat(g, idx);
}

/// C `playerclearbridge_strat` (PCSTRATS.ASM:64-76).
fn playerclearbridge_strat(g: &mut Game, idx: u16) {
    // jsr centoutrots
    centoutrots(g);

    // s_beqdec_var B,psvar_byte1,playerclearbridge2_init
    let b1 = g.vars.sv_u8(sv::PSVAR_BYTE1);
    if b1 == 0 {
        playerclearbridge2_init(g, idx);
        return;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1 - 1);

    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_achase_alvar.w W,x,al_worldy,ViewCY,4
        al.worldy = strat_chase_proportional(al.worldy, view_cy, 4);
        // s_achase_alvar.w W,x,al_worldx,#0,4
        al.worldx = strat_chase_proportional(al.worldx, 0, 4);
    }

    // s_set_var B,player_tospeed,#maxpspeed
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MAX_PSPEED as u8);

    // s_jmp_varmore W,outdist,#500,.nda
    let outdist = g.vars.sv_i16(sv::OUTDIST);
    if outdist <= 500 {
        // s_add_var W,outdist,#4
        g.vars.set_sv_i16(sv::OUTDIST, outdist.wrapping_add(4));
    }
    // .nda: s_jmp playeronbridge_strat
    playeronbridge_strat(g, idx);
}

/// C `Strat_PlayerClearBridge_Init` (playerclearbridge_Istrat,
/// PCSTRATS.ASM:48-62).
pub fn strat_player_clear_bridge_init(g: &mut Game, idx: u16) {
    // s_set_strat x,playerclearbridge_strat
    let strat_id = sid(g, K_CLEARBRIDGE_STRAT);
    g.objs.aliens[idx as usize].stratptr = Some(strat_id);

    // s_playerctrl off
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;

    // s_set_var W,minPmoveY,#-10000 / minPWmoveY,#-10000
    g.vars.minpmove_y = -10000;
    g.vars.set_sv_i16(sv::MINPWMOVEY, -10000);

    // s_or_var B,pstratflags,#pstf_novdistC / #pstf_inseq
    g.vars.pstratflags |= PSTF_NOVDISTC;
    g.vars.pstratflags |= PSTF_INSEQ;

    // s_set_var W,psvar_word1,#0 / psvar_word2,#0
    g.vars.psvar_word1 = 0;
    g.vars.psvar_word2 = 0;

    // s_set_var B,psvar_byte1,#125+38
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 125 + 38);
}

// ============================================================
// playerEscapeNucleus (PCSTRATS.ASM:97-150)
// ============================================================

/// C `playerEscapeNucleus_cont` (PCSTRATS.ASM:137-150).
fn player_escape_nucleus_cont(g: &mut Game, idx: u16) {
    let i = idx as usize;
    // s_speedto x,#30,1
    strat_speed_to(&mut g.objs.aliens[i], 30, 1);

    // s_set_var W,pviewvelz,#medpspeed
    g.vars.pviewvelz = MED_PSPEED;

    // s_add_var W,pviewposz,pviewvelz
    let z = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(g.vars.pviewvelz);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, z);

    // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    strat_gen_vecs_3d(&mut g.objs.aliens[i]);

    // s_add_alvar W,x,al_vz,#medpspeed
    g.objs.aliens[i].vz = g.objs.aliens[i].vz.wrapping_add(MED_PSPEED);

    // s_add_vecs2pos x
    strat_apply_velocity(&mut g.objs.aliens[i]);
}

/// C `playerEscapeNucleus2_start` (PCSTRATS.ASM:121-134).
fn player_escape_nucleus2_start(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_decbne_alvar B,x,al_sbyte3,playerEscapeNucleus_cont
    g.objs.aliens[i].sbyte3 = g.objs.aliens[i].sbyte3.wrapping_sub(1);
    if g.objs.aliens[i].sbyte3 != 0 {
        player_escape_nucleus_cont(g, idx);
        return;
    }

    // s_set_alvar B,x,al_sbyte3,#1
    g.objs.aliens[i].sbyte3 = 1;

    // s_jmp_alvarEQ B,x,al_rotx,#-deg11,.nxdec
    if g.objs.aliens[i].rotx as i8 != -(DEG11 as i8) {
        g.objs.aliens[i].rotx = g.objs.aliens[i].rotx.wrapping_sub(1);
    }

    // s_jmp_alvarEQ B,x,al_rotz,#deg45,.nzinc
    if g.objs.aliens[i].rotz != DEG45 {
        g.objs.aliens[i].rotz = g.objs.aliens[i].rotz.wrapping_add(2);
    }

    // s_jmp_alvarEQ B,x,al_roty,#deg180+(deg22+deg11),.boost
    if g.objs.aliens[i].roty == DEG180.wrapping_add(DEG22).wrapping_add(DEG11) {
        player_escape_nucleus_cont(g, idx);
        return;
    }

    // s_add_alvar B,x,al_roty,#8
    g.objs.aliens[i].roty = g.objs.aliens[i].roty.wrapping_add(8);

    player_escape_nucleus_cont(g, idx);
}

/// C `playerEscapeNucleus2_init` (PCSTRATS.ASM:116-120).
fn player_escape_nucleus2_init(g: &mut Game, idx: u16) {
    // s_set_strat x,playerEscapeNucleus2_start
    let start_id = sid(g, K_ESCNUCLEUS2_START);
    g.objs.aliens[idx as usize].stratptr = Some(start_id);

    // s_set_alvar B,x,al_sbyte3,#15
    g.objs.aliens[idx as usize].sbyte3 = 15;

    // s_set_vartobeobj boostobj,x
    g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
    if g.vars.sv_u8(sv::BOOSTZOFF) == 0 {
        set_boost_zoff(g, -30);
    }
    let _ = boost_sprite(g, None);

    player_escape_nucleus2_start(g, idx);
}

/// C `playerEscapeNucleus_strat` (PCSTRATS.ASM:107-114).
fn player_escape_nucleus_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_jmp_alvarEQ B,x,al_roty,#-(deg45-deg22),playerEscapeNucleus2_init
    if g.objs.aliens[i].roty == (DEG45 - DEG22).wrapping_neg() {
        player_escape_nucleus2_init(g, idx);
        return;
    }

    // s_add_alvar B,x,al_roty,#-1 / al_rotz,#-1
    g.objs.aliens[i].roty = g.objs.aliens[i].roty.wrapping_sub(1);
    g.objs.aliens[i].rotz = g.objs.aliens[i].rotz.wrapping_sub(1);

    // s_jmp_alvarEQ B,x,al_rotx,#deg11,.nxinc
    if g.objs.aliens[i].rotx != DEG11 {
        g.objs.aliens[i].rotx = g.objs.aliens[i].rotx.wrapping_add(1);
    }
    player_escape_nucleus_cont(g, idx);
}

/// C `Strat_PlayerEscapeNucleus_Init` (playerEscapeNucleus_Istrat,
/// PCSTRATS.ASM:97-106).
pub fn strat_player_escape_nucleus_init(g: &mut Game, idx: u16) {
    // s_or_var B,pstratflags,#pstf_inseq
    g.vars.pstratflags |= PSTF_INSEQ;

    // s_set_strat x,playerEscapeNucleus_strat
    let strat_id = sid(g, K_ESCNUCLEUS_STRAT);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(strat_id);
    // s_set_speed x,#0
    al.vel = 0;
    // s_set_alvar B,x,al_roty,#0
    al.roty = 0;
    // s_set_alsflag x,colldisable
    al.sflags |= ASF_COLLDISABLE;
}

// ============================================================
// PISTRATS.ASM — player opening (intro sequence) strategies
// ============================================================

/// C `viewopening_Istrat` (GISTRATS.ASM:617-628) — camera fly-in object.
fn viewopening_istrat(g: &mut Game, idx: u16) {
    let strat_id = sid(g, K_VIEWOPENING_STRAT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_set_alsflag x,invisible
        al.sflags |= ASF_INVISIBLE;
        // s_set_strat x,viewopening_strat
        al.stratptr = Some(strat_id);
        // s_add_alvar W,x,al_worldx,#-600*2 / worldy,#-1000*2 / worldz,#3500
        al.worldx = al.worldx.wrapping_add(-600 * 2);
        al.worldy = al.worldy.wrapping_add(-1000 * 2);
        al.worldz = al.worldz.wrapping_add(3500);
        // s_set_alvar B,x,al_roty,#deg180 / al_rotx,#deg5 / al_sbyte1,#90
        al.roty = DEG180;
        al.rotx = DEG5;
        al.sbyte1 = 90;
    }
    // s_AND_var B,gameflags,#~gf_stratdone1
    g.vars.gameflags &= !GF_STRATDONE1;
    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g.vars.pshipflags3 &= !PSF3_ENGINESND;

    // The source initializer label falls through directly into the per-frame
    // strategy body, so the camera performs its first chase on creation.
    viewopening_strat(g, idx);
}

/// C `viewopening_strat` (GISTRATS.ASM:629-683): multi-state camera that
/// chases toward player position offsets, then drives viewpos verbatim.
fn viewopening_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_set_var W,svar_word1,player_posx / word2,#-30 / word3,player_posz
    let mut w1 = g.vars.player_posx;
    let mut w2: i16 = -30;
    let mut w3 = g.vars.player_posz;

    let mut run_state_one = g.objs.aliens[i].stratstate != 0;
    if !run_state_one {
        w1 = w1.wrapping_add(-400);
        w2 = w2.wrapping_add(-700);
        w3 = w3.wrapping_add(-700);
        {
            let al = &mut g.objs.aliens[i];
            al.worldx = strat_chase_proportional(al.worldx, w1, 5);
            al.worldy = strat_chase_proportional(al.worldy, w2, 5);
            al.worldz = strat_chase_proportional(al.worldz, w3, 5);
        }
        // s_decbne_alvar B,x,al_sbyte1
        let mut advance = true;
        if g.objs.aliens[i].sbyte1 > 0 {
            g.objs.aliens[i].sbyte1 -= 1;
            if g.objs.aliens[i].sbyte1 != 0 {
                advance = false;
            }
        }
        if advance {
            // The source's state-zero block falls through to the state-one
            // block after changing state; its three scratch targets retain
            // the offsets already applied above for this transition frame.
            g.objs.aliens[i].stratstate = 1;
            g.objs.aliens[i].sbyte1 = 80;
            g.vars.gameflags |= GF_STRATDONE2;
            g.vars.pshipflags3 |= PSF3_ENGINESND;
            run_state_one = true;
        }
    }
    if run_state_one {
        // State 1 (GISTRATS.ASM:652-668)
        g.objs.aliens[i].sbyte1 = g.objs.aliens[i].sbyte1.wrapping_sub(1);
        if g.objs.aliens[i].sbyte1 == 0 {
            // Signals MAP1_1A's `mapif chkstratdone1,.fin` loop.
            g.vars.gameflags |= GF_STRATDONE1;
        }
        // .ndone: s_AND_var B,gameflags,#~gf_nozremove
        g.vars.gameflags &= !GF_NOZREMOVE;
        // s_add_var W,svar_word2,#20 / svar_word3,#-300
        w2 = w2.wrapping_add(20);
        w3 = w3.wrapping_sub(300);
        let al = &mut g.objs.aliens[i];
        al.worldy = strat_chase_proportional(al.worldy, w2, 4);
        al.worldx = strat_chase_proportional(al.worldx, w1, 3);
        if al.worldx == w1 {
            // .zoom: s_add_alvar W,x,al_worldz,#10
            al.worldz = al.worldz.wrapping_add(10);
        } else {
            // .nfrick: s_achase_alvar W,x,al_worldz,svar_word3,4
            al.worldz = strat_chase_proportional(al.worldz, w3, 4);
        }
    }

    g.vars.set_sv_i16(sv::SVAR_WORD1, w1);
    g.vars.set_sv_i16(sv::SVAR_WORD2, w2);
    g.vars.set_sv_i16(sv::SVAR_WORD3, w3);

    // .end: s_add_alvar W,x,al_worldz,#medpspeed
    let al = &mut g.objs.aliens[i];
    al.worldz = al.worldz.wrapping_add(MED_PSPEED);
    // s_copy_alvar2var W,x,viewposx/viewposy/viewposz — drive the camera.
    let (wx, wy, wz) = (al.worldx, al.worldy, al.worldz);
    g.vars.set_sv_i16(sv::VIEWPOSX, wx);
    g.vars.set_sv_i16(sv::VIEWPOSY, wy);
    g.vars.set_sv_i16(sv::VIEWPOSZ, wz);
}

/// C `playeropeningboost_strat` (PISTRATS.ASM:108-112).
fn playeropeningboost_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    // s_achase_alvar B,x,al_rotz,#0,3
    let diff = al.rotz as i8;
    let mut step = diff >> 3;
    if diff != 0 && step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    al.rotz = (al.rotz as i8).wrapping_sub(step) as u8;
    // s_add_Alvar W,x,al_worldz,#medpspeed+50
    al.worldz = al.worldz.wrapping_add(MED_PSPEED + 50);
}

/// C `playeropeningboost_init` (PISTRATS.ASM:99-107).
fn playeropeningboost_init(g: &mut Game, idx: u16) {
    // s_set_strat x,playeropeningboost_strat
    let boost_id = sid(g, K_OPENINGBOOST_STRAT);
    g.objs.aliens[idx as usize].stratptr = Some(boost_id);
    // s_set_vartobeobj boostobj,x
    g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
    if g.vars.sv_u8(sv::BOOSTZOFF) == 0 {
        set_boost_zoff(g, -30);
    }
    let _ = boost_sprite(g, None);
    // trigse $32
    g.hooks.play_se(0x32);
    // s_and_var B,pshipflags3,#~psf3_enginesnd
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
}

/// C `playerpening_strat` (PISTRATS.ASM:74-97; original ASM typo kept).
fn playerpening_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_inc_var B,psvar_byte1
    let mut b1 = g.vars.sv_u8(sv::PSVAR_BYTE1).wrapping_add(1);
    if b1 >= PZROTFLOATTAB_LEN {
        b1 = 0;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, b1);
    g.objs.aliens[i].rotz = SHIPINTRO_ROTZ_FLOAT[b1 as usize] as u8;

    // s_add_var B,psvar_byte2,#2
    let mut b2 = g.vars.sv_u8(sv::PSVAR_BYTE2).wrapping_add(2);
    if b2 >= VIEWFLOATTAB_BYTELEN {
        b2 = 0;
    }
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, b2);
    // The source table macro uses the byte offset to select a word and then
    // applies its explicit scale of one (multiply by two).
    g.objs.aliens[i].worldy = SHIPINTRO_VIEW_FLOAT[(b2 / 2) as usize].wrapping_mul(2);

    // s_add_alvar W,x,al_worldy,#-30
    g.objs.aliens[i].worldy = g.objs.aliens[i].worldy.wrapping_add(-30);

    // s_jmpNOT_varAND B,gameflags,#gf_stratdone2,.nboost
    if g.vars.gameflags & GF_STRATDONE2 != 0 {
        // s_beqdec_var B,psvar_byte3,playeropeningboost_init
        let b3 = g.vars.sv_u8(sv::PSVAR_BYTE3);
        if b3 == 0 {
            playeropeningboost_init(g, idx);
            return;
        }
        g.vars.set_sv_u8(sv::PSVAR_BYTE3, b3 - 1);
    }

    // s_add_Alvar W,x,al_worldz,#medpspeed
    g.objs.aliens[i].worldz = g.objs.aliens[i].worldz.wrapping_add(MED_PSPEED);
    // jsl makeBG2black_l — SNES-specific, no-op in HD
}

/// C `Strat_PlayerOpening_Init` (playeropening_Istrat, PISTRATS.ASM:46-72).
pub fn strat_player_opening_init(g: &mut Game, idx: u16) {
    // s_playerctrl off
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;

    // s_playerfly_mode Ltunnel
    apply_tunnel_fly_mode(g, idx, FLY_LTUNNEL);

    // s_set_alptrs x,playerpening_strat,0,0
    let pening_id = sid(g, K_PLAYERPENING_STRAT);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(pening_id);
        al.collstratptr = None;
        al.expstratptr = None;
    }

    // stz gameframe
    g.vars.gameframe = 0;

    // Select fade direction 2.
    g.vars.set_sv_i8(sv::FADEDIR, 2);

    // s_set_var W,outvx,#-deg11*256
    g.vars.set_sv_i16(sv::OUTVX, -(DEG11 as i16 * 256));

    // s_or_var B,gameflags,#gf_nozremove
    g.vars.gameflags |= GF_NOZREMOVE;

    // s_set_var B,viewtype,#viewtype_toobj!viewtype_Fpos
    g.vars
        .set_sv_u8(sv::VIEWTYPE, VIEWTYPE_TOOBJ | VIEWTYPE_FPOS);
    // viewtoobj defaults to playpt (MAIN.ASM) — aim the camera at the ship.
    g.vars.set_sv_i16(sv::VIEWTOOBJ, idx as i16);

    // s_make_obj #nullshape / s_set_strat y,viewopening_Istrat / s_copy_pos y,x
    if let Some(cam) = strat_make_obj(g, 0) {
        let src = g.objs.aliens[idx as usize];
        {
            let c = &mut g.objs.aliens[cam as usize];
            c.worldx = src.worldx;
            c.worldy = src.worldy;
            c.worldz = src.worldz;
        }
        viewopening_istrat(g, cam);
        // Seed the fixed camera position so the first rendered frame
        // (before the strat's first tick) already uses the fly-in cam.
        let c = g.objs.aliens[cam as usize];
        g.vars.set_sv_i16(sv::VIEWPOSX, c.worldx);
        g.vars.set_sv_i16(sv::VIEWPOSY, c.worldy);
        g.vars.set_sv_i16(sv::VIEWPOSZ, c.worldz);
    }

    // s_AND_var B,gameflags,#~gf_stratdone2
    g.vars.gameflags &= !GF_STRATDONE2;

    // s_set_var B,psvar_byte3,#70
    g.vars.set_sv_u8(sv::PSVAR_BYTE3, 70);

    // s_set_alvar W,x,al_shape,#Imyship_4
    g.objs.aliens[idx as usize].shape = SHAPE_MYSHIP_4;

    // s_set_var B,psvar_byte1,#0 / psvar_byte2,#0
    g.vars.set_sv_u8(sv::PSVAR_BYTE1, 0);
    g.vars.set_sv_u8(sv::PSVAR_BYTE2, 0);

    // `playeropening_Istrat` falls through into `playerpening_strat` in the
    // original source, advancing the ship on the initializer frame.
    playerpening_strat(g, idx);
    refresh_player_collision_proxies(g);
}

// Re-export the sv module for tests and other-lane wiring convenience.
pub use crate::common::sv as player_sv;

// ============================================================
// Fly-in / straight / speed / on-cont (PISTRATS.ASM / PSTRATS.ASM)
// ============================================================

fn flyin_med_speed_setup(g: &mut Game, idx: u16) {
    g.vars.pviewvelz = MED_PSPEED;
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, MED_PSPEED as u8);
    g.vars.set_sv_u8(sv::PLAYER_MEDSPEED, MED_PSPEED as u8);
    g.objs.aliens[idx as usize].vel = MED_PSPEED as u8;
}

fn flyin_chase_y_done(g: &mut Game, idx: u16, shift: u32) -> bool {
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    let y = g.objs.aliens[idx as usize].worldy;
    let ny = strat_chase_proportional(y, view_cy, shift);
    g.objs.aliens[idx as usize].worldy = ny;
    ny == view_cy
}

/// ROM `playerspaceflyin_Istrat` (PISTRATS.ASM:116).
pub fn player_space_flyin_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_space(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let s = ea_sid(g, player_space_flyin_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.worldx = 0;
        al.worldy = -400;
        al.worldz = 0;
    }
    g.vars.pstratflags |= PSTF_NOVDISTC;
    flyin_med_speed_setup(g, idx);
}

/// ROM `playerspaceflyin_strat`.
pub fn player_space_flyin_strat(g: &mut Game, idx: u16) {
    let od = g.vars.sv_i16(sv::OUTDIST).wrapping_add(3);
    g.vars.set_sv_i16(sv::OUTDIST, od);
    if flyin_chase_y_done(g, idx, 3) {
        g.vars.pstratflags &= !PSTF_NOVDISTC;
        set_player_in_space(g, idx);
        return;
    }
    player_in_space_strat(g, idx);
}

/// ROM `playerinsidespaceflyin_Istrat` (PISTRATS.ASM:139).
pub fn player_inside_space_flyin_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_space(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let s = ea_sid(g, player_inside_space_flyin_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.worldx = 0;
        al.worldy = -400;
        al.worldz = 0;
    }
    g.vars.pstratflags |= PSTF_NOVDISTC;
    let wz = g.objs.aliens[idx as usize].worldz;
    g.vars.set_sv_i16(sv::PVIEWPOSZ, wz);
    g.vars.set_sv_i16(sv::OUTDIST, CLOSE_VIEW_DISTANCE);
    g.vars.viewdist = CLOSE_VIEW_DISTANCE;
    flyin_med_speed_setup(g, idx);
}

/// ROM `playerinsidespaceflyin_strat`.
pub fn player_inside_space_flyin_strat(g: &mut Game, idx: u16) {
    let od = g.vars.sv_i16(sv::OUTDIST).wrapping_add(3);
    g.vars.set_sv_i16(sv::OUTDIST, od);
    if flyin_chase_y_done(g, idx, 3) {
        g.vars.pstratflags &= !PSTF_NOVDISTC;
        g.vars.player_view_mode = PlayerViewMode::EnteringCockpit;
        g.apply_player_view_mode(idx);
        player_in_space_strat(g, idx);
        return;
    }
    player_in_space_strat(g, idx);
}

/// ROM `playerplanetflyin_Istrat` (PISTRATS.ASM:168).
pub fn player_planet_flyin_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_on_planet(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let s = ea_sid(g, player_planet_flyin_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.worldx = 0;
        al.worldy = -400;
        al.worldz = 0;
    }
    g.vars.pstratflags |= PSTF_NOVDISTC;
    flyin_med_speed_setup(g, idx);
}

/// ROM `playerplanetflyin_strat`.
pub fn player_planet_flyin_strat(g: &mut Game, idx: u16) {
    let od = g.vars.sv_i16(sv::OUTDIST).wrapping_add(3);
    g.vars.set_sv_i16(sv::OUTDIST, od);
    if flyin_chase_y_done(g, idx, 3) {
        g.vars.pstratflags &= !PSTF_NOVDISTC;
        set_player_on_planet(g, idx);
        return;
    }
    player_on_planet_body(g, idx);
}

/// ROM `playerLtunnelflyin_Istrat` (PISTRATS.ASM:191).
pub fn player_ltunnel_flyin_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_ltunnel(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let s = ea_sid(g, player_ltunnel_flyin_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.worldy = -120;
        al.vel = MAX_PSPEED as u8;
    }
}

/// ROM `playerLtunnelflyin_strat`.
pub fn player_ltunnel_flyin_strat(g: &mut Game, idx: u16) {
    if flyin_chase_y_done(g, idx, 3) {
        set_player_in_ltunnel(g, idx);
        return;
    }
    player_in_tunnel_strat(g, idx);
}

/// ROM `playerColonyflyin_Istrat` (PISTRATS.ASM:219).
pub fn player_colony_flyin_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    set_player_in_colony(g, idx);
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    let s = ea_sid(g, player_colony_flyin_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.worldy = -120;
        al.vel = MAX_PSPEED as u8;
    }
}

/// ROM `playerColonyflyin_strat`.
pub fn player_colony_flyin_strat(g: &mut Game, idx: u16) {
    if flyin_chase_y_done(g, idx, 4) {
        set_player_in_colony(g, idx);
        return;
    }
    player_in_colony_strat(g, idx);
}

/// ROM `playerstraight_strat` (PSTRATS.ASM:623) — lock view/ship to med cruise.
pub fn player_straight_strat(g: &mut Game, idx: u16) {
    g.vars.set_sv_u8(sv::ARROWS, 0);
    g.vars.set_sv_u8(sv::VIEWTYPE, VIEWTYPE_NORM);
    g.vars.set_sv_i16(sv::PLROTX, 0);
    g.vars.set_sv_i16(sv::PLROTY, 0);
    g.vars.set_sv_i16(sv::PLROTZ, 0);
    let ovx = strat_chase_proportional(g.vars.sv_i16(sv::OUTVX), 0, 3);
    let ovy = strat_chase_proportional(g.vars.sv_i16(sv::OUTVY), 0, 3);
    g.vars.set_sv_i16(sv::OUTVX, ovx);
    g.vars.set_sv_i16(sv::OUTVY, ovy);
    g.vars.set_sv_i16(sv::OUTVZ, 0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
        al.vel = MED_PSPEED as u8;
        strat_gen_vecs_3d(al);
        strat_apply_velocity(al);
        al.worldx = 0;
        al.worldy = g.vars.sv_i16(sv::VIEWCY);
    }
    g.vars.set_sv_u8(sv::VIEWSHAKEX, 0);
    g.vars.set_sv_u8(sv::VIEWSHAKEY, 0);
    g.vars.set_sv_u8(sv::VIEWSHAKEZ, 0);
    g.vars.pviewvelz = MED_PSPEED;
    g.vars.set_sv_i16(sv::PVIEWPOSX, 0);
    g.vars.set_sv_i16(sv::PVIEWPOSY, g.vars.sv_i16(sv::VIEWCY));
    let pz = g.vars.sv_i16(sv::PVIEWPOSZ).wrapping_add(MED_PSPEED);
    g.vars.set_sv_i16(sv::PVIEWPOSZ, pz);
}

/// ROM `playerspeedup_Istrat` (PSTRATS.ASM:1297) — boost player then remove self.
pub fn player_speedup_istrat(g: &mut Game, idx: u16) {
    let p = g.vars.internal_playpt;
    if p >= 0 {
        let p = p as u16;
        let boost = ((MAX_PSPEED - MED_PSPEED) / 2) + MED_PSPEED;
        g.objs.aliens[p as usize].vel = MAX_PSPEED as u8;
        g.vars.pviewvelz = boost;
        g.objs.aliens[p as usize].sbyte2 = 20;
    }
    g.objs.aldead = 1;
    let _ = idx;
}

/// ROM `playerspeedstop_Istrat` (PSTRATS.ASM:1307).
pub fn player_speedstop_istrat(g: &mut Game, idx: u16) {
    g.vars.set_sv_u8(sv::PLAYER_TOSPEED, 0);
    g.objs.aldead = 1;
    let _ = idx;
}

/// ROM `set_playerOnfield_l` / field fly-mode (STRATEQU.INC:723-742).
pub fn set_player_on_field(g: &mut Game, idx: u16) {
    apply_planet_style_fly_mode(
        g,
        idx,
        -60,  // field_viewCY
        -500, // field_minX
        500,
        -500,
        500,
        -120, // field_minY
        0,    // field_maxY
        0,    // field_MmaxY
        PFM_DIEFALL | PFM_DIEYROT | PFM_SHADOWS | PFM_WOBBLE,
        PML_LWBOTTOM | PML_RWBOTTOM | PML_BBOTTOM,
        MB_BOTTOM,
        false, // field_gameflagsOFF = gf_viewrot
        true,
        true,
    );
}

/// ROM `playeronfield_Istrat` (PSTRATS.ASM:806).
pub fn player_on_field_istrat(g: &mut Game, idx: u16) {
    set_player_on_field(g, idx);
    let s = ea_sid(g, player_on_field_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    // coll/dead ptrs stay on the shared player paths from spawn.
    player_on_field_strat(g, idx);
}

/// ROM `playeronfield_strat` (PSTRATS.ASM:816): limitX, viewX=perc87,
/// viewY=ViewCY, viewmove. Source is under `ifeq 1` (not in retail image) but
/// matches the live `playeronplanet` X formula; the `gf2_viewclose`→perc93
/// branch is commented out in ASM, so always perc87.
pub fn player_on_field_strat(g: &mut Game, idx: u16) {
    do_player_limit_x(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    g.vars.set_sv_i16(sv::PVIEWPOSX, strat_perc87(wx));
    g.vars.set_sv_i16(sv::PVIEWPOSY, g.vars.sv_i16(sv::VIEWCY));
    viewmove_srou(g, idx);
}

/// ROM `playeroncont_Istrat` (PSTRATS.ASM:722).
pub const CONTINUE_VIEW_DISTANCE: i16 = 200;
const CONTINUE_VIEW_CENTER_Y: i16 = -50;
const CONTINUE_HORIZONTAL_LIMIT: i16 = 0;
const CONTINUE_VERTICAL_LIMIT: i16 = -50;

/// Install the controller-demonstration initializer for the first strategy
/// pass. `initgame_l` writes the source `pstrat` entry to the player object;
/// it does not execute that initializer before the first `transfer_l`.
pub fn queue_player_on_cont_istrat(g: &mut Game, idx: u16) {
    let initializer = ea_sid(g, player_on_cont_istrat);
    g.objs.aliens[idx as usize].stratptr = Some(initializer);
}

pub fn player_on_cont_istrat(g: &mut Game, idx: u16) {
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
    {
        // Exact `s_playerfly_mode cont` fields from STRATEQU.INC.
        let vars = &mut g.vars;
        vars.set_sv_i16(sv::VIEWCY, CONTINUE_VIEW_CENTER_Y);
        vars.set_sv_i16(sv::MINPMOVEX, CONTINUE_HORIZONTAL_LIMIT);
        vars.set_sv_i16(sv::MAXPMOVEX, CONTINUE_HORIZONTAL_LIMIT);
        vars.set_sv_i16(sv::MINMMOVEX, CONTINUE_HORIZONTAL_LIMIT);
        vars.set_sv_i16(sv::MAXMMOVEX, CONTINUE_HORIZONTAL_LIMIT);
        vars.set_sv_i16(sv::MAXMMOVEY, CONTINUE_VERTICAL_LIMIT);
        vars.set_sv_i16(sv::MINPWMOVEY, CONTINUE_VERTICAL_LIMIT);
        vars.set_sv_i16(
            sv::MAXPWMOVEY,
            CONTINUE_VERTICAL_LIMIT + PLAYER_WING_Y_PADDING,
        );
        vars.minpmove_y = CONTINUE_VERTICAL_LIMIT;
        vars.set_sv_i16(sv::MAXPMOVEY, CONTINUE_VERTICAL_LIMIT + PLAYERB_YSTOP);
        vars.playerflymode = PFM_DIEYROT;
        vars.set_sv_u8(sv::PMOVELIMITAND, 0);
        vars.set_sv_u8(sv::MISSBOUNDFLAGS, 0);
        vars.gameflags |= GF_VIEWROT;
        vars.pstratflags &= !PSTF_NOVIEWMOVE;
        vars.pshipflags3 |= PSF3_ENGINESND;

        // `cont_macro`: clear spark/tunnel/sequence state and make the demo
        // player immortal.
        vars.pshipflags2 &= !PSF2_NOSPARK;
        vars.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
        vars.pshipflags3 &= !PSF3_INTUNNEL;
        vars.pstratflags |= PSTF_NOTDIE;
    }
    let s = ea_sid(g, player_on_cont_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.vars.pviewvelz = MED_PSPEED;
    player_on_cont_strat(g, idx);
}

/// ROM `playeroncont_strat`.
pub fn player_on_cont_strat(g: &mut Game, idx: u16) {
    do_player_limit_x(g, idx);
    let wx = g.objs.aliens[idx as usize].worldx;
    let wy = g.objs.aliens[idx as usize].worldy;
    g.vars.set_sv_i16(sv::PVIEWPOSX, wx);
    g.vars.set_sv_i16(sv::PVIEWPOSY, wy);
    viewmove_srou(g, idx);
    g.vars.set_sv_i16(sv::OUTVX, 0);
    g.vars.set_sv_i16(sv::OUTVY, 0);
    g.vars.set_sv_i16(sv::OUTDIST, CONTINUE_VIEW_DISTANCE);
    g.vars.viewdist = CONTINUE_VIEW_DISTANCE;
    g.objs.aliens[idx as usize].shape = SH_MY_DEMO_S;
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
}

/// ROM `playerDIVEGND_Istrat` — extend `set_player_dive_gnd` with stratptrs.
pub fn player_divegnd_istrat(g: &mut Game, idx: u16) {
    set_player_dive_gnd(g, idx);
}

/// ROM `playerDIVEGND_strat` — the visible duplicate and camera own motion.
pub fn player_divegnd_strat(_g: &mut Game, _idx: u16) {}
