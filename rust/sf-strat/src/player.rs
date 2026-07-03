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
    sf_random, strat_apply_velocity, strat_chase, strat_chase_proportional, strat_gen_vecs_3d,
    strat_make_obj, strat_perc62, strat_perc75, strat_perc87, strat_remove_obj,
    strat_spawn_projectile, strat_speed_to, sv, StratRam,
};
use sf_core::pad;
use sf_game::alien::{
    StratId, ACF_COLLTYPE1, ASF4_PLAYEROBJ, ASF_COLLDISABLE, ASF_COLLIDE, ASF_HITFLASH,
    ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATZREMOVE,
};
use sf_game::game::StrategyFn;
use sf_game::vars::{
    GF_NOZREMOVE, GF_PLAYERDEAD, GF_PLAYERDYING, GF_STRATDONE1, GF_STRATDONE2, GF_VIEWROT,
    OUTVIEWDIST, PFM_SHADOWS, PFM_WOBBLE, PSF2_PLAYERHP0, PSF3_ENGINESND, PSF3_INTUNNEL,
    PSF3_NOCOLLISIONS, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, PSTF_NOTDIE, PSTF_NOVDISTC,
    SPACE_MODE, SPFM_INSIDE,
};
use sf_game::Game;

// ============================================================
// Constants not yet in sf_game::vars (C src/variables.h / obj.h).
// TODO(consolidation): move to sf-game vars.rs when that lane widens.
// ============================================================
const PSF_BRKLWING: u8 = 8;
const PSF_BRKRWING: u8 = 16;
const PSF_NOYCTRL: u8 = 128;

const PSF2_DOUBLASER: u8 = 1;
const PSF2_NOSPARK: u8 = 4;
const PSF2_FORCEBOOST: u8 = 16;
const PSF2_BOOSTING: u8 = 32;
const PSF2_BRAKING: u8 = 64;

const PSF3_FORCEBRAKE: u8 = 4;
const PSF3_BEAMBALL: u8 = 16;

const PSTF_NOVIEWMOVE: u8 = 4;

const PFM_DIEFALL: u8 = 1;
const PFM_DIEYROT: u8 = 2;

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
const DEFAULT_NUKE_COUNT: u16 = 3;
const SPECIAL_DELAY_FRMS: u8 = 50;
const PLAYER_FIRESPEED: u8 = 2;

/// C ASF2_SFLAG1 (src/game/obj.h:122).
const ASF2_SFLAG1: u8 = 0x10;

/// C SHAPE_MYSHIP_4 / SHAPE_ARWING (src/renderer/shapes.h:42).
const SHAPE_MYSHIP_4: u16 = 2;
const SHAPE_ARWING: u16 = 2;

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
/// `largeplasma` 128x128 grey quad that rendered as a giant block. The 9-frame
/// growth animation + `bullet_a1` color flash are follow-ups (task #39).
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

// --- Weapon offsets/constants (STRATEQU.INC) ---
const PLAYER_W_X: i16 = 33;
const PLAYER_W_Y: i16 = 13;
const PLAYER_W_X_SCALED: i16 = PLAYER_W_X >> 2;
const PLAYER_W_Y_SCALED: i16 = PLAYER_W_Y >> 2;
const INVIEW_LASER_Y_OFF: i16 = 50;
const NUKE_Z_OFFSET: i16 = 80 >> 2;
const PLAYER_BODY_HP: u8 = 40;
const PLAYER_HITFLASH_FRMS: u8 = 7;
const SCREENFLASH_BODY_FRMS: u8 = 4;
const SCREENFLASH_BODY_TYPE: u8 = 0;
const PLAYER_DEAD_FRAMES: u8 = 60;
const SHIPINTRO_LIFE: u8 = 40;
const SHIPINTRO_BOOST_Z: i16 = 50;

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
    0, 1, 2, 3, 4, 4, 5, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -5, -4, -4, -3, -2,
    -1,
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

const FNS: [StrategyFn; 21] = [
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
    al.worldy = SHIPINTRO_VIEW_FLOAT[al.sbyte4 as usize];
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
                g.objs.aliens[i].worldz =
                    g.objs.aliens[i].worldz.wrapping_add(SHIPINTRO_BOOST_Z);
                shipintro_dec_lifecnt(g, idx);
            }
        }
    }

    if g.objs.aliens[i].sbyte1 == 2 {
        // C player_obj_index_or_null(self) — the alien's own slot index.
        g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
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
    let r1 = (sf_random(&mut g.vars) & 15) as u8;
    let r2 = (sf_random(&mut g.vars) & 7) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(strat);
        al.sbyte3 = r1;
        al.sbyte4 = r2;
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
        g.objs.aliens[i].sflags &= !ASF_COLLIDE;
        return;
    }

    if g.objs.aliens[i].sflags & ASF_NOHITAFFECT != 0 {
        g.objs.aliens[i].sflags &= !ASF_COLLIDE;
        return;
    }

    let hits = g.vars.sv_u8(sv::PNUMHITS);
    if hits < 0xFF {
        g.vars.set_sv_u8(sv::PNUMHITS, hits + 1);
    }

    g.objs.aliens[i].sbyte1 = PLAYER_HITFLASH_FRMS;
    g.objs.aliens[i].sflags |= ASF_HITFLASH;
    g.objs.aliens[i].sflags |= ASF_NOHITAFFECT;
    g.vars.set_sv_u8(sv::SCREENFLASHCNT, SCREENFLASH_BODY_FRMS);
    g.vars.set_sv_u8(sv::SCREENFLASHTYPE, SCREENFLASH_BODY_TYPE);

    if g.objs.aliens[i].hp == 0 {
        playerdead_istrat(g, idx);
    }

    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
}

/// C `playerdead_strat` (strat_player.c:209).
fn playerdead_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;
    if g.vars.gameflags & GF_PLAYERDEAD != 0 {
        return;
    }

    let mut speed = g.vars.sv_i16(sv::PLAYER_SPEED);
    if g.vars.gameframe & 1 == 0 && speed > 0 {
        speed = strat_chase(speed, 0, 1);
        g.vars.set_sv_i16(sv::PLAYER_SPEED, speed);
    }
    g.objs.aliens[i].vel = clamp16(speed, 0, MAX_PSPEED) as u8;

    let zadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD).wrapping_add(4);
    g.vars.set_sv_u8(sv::PLAYER_ZSTRATADD, zadd);
    g.objs.aliens[i].sflags |= ASF_COLLDISABLE;
    g.objs.aliens[i].sflags &= !ASF_SHADOW;

    if g.objs.aliens[i].sbyte1 < 0xFF {
        g.objs.aliens[i].sbyte1 += 1;
    }
    if g.objs.aliens[i].sbyte1 < PLAYER_DEAD_FRAMES {
        return;
    }

    g.vars.gameflags &= !GF_PLAYERDYING;
    g.vars.gameflags |= GF_PLAYERDEAD;
    let lives = g.vars.sv_u8(sv::LIVES);
    if lives > 0 {
        g.vars.set_sv_u8(sv::LIVES, lives - 1);
    }
}

/// C `playerdead_istrat` (strat_player.c:237).
fn playerdead_istrat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        return;
    }

    g.vars.pshipflags2 |= PSF2_PLAYERHP0;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.pstratflags |= PSTF_INSEQ;
    g.vars.pshipflags3 &= !PSF3_ENGINESND;
    g.vars.gameflags |= GF_PLAYERDYING;
    g.vars.gameflags &= !GF_PLAYERDEAD;

    let dead = sid(g, K_PLAYERDEAD_STRAT);
    let coll = sid(g, K_PLAYERCOLL);
    let exp = sid(g, K_PLAYERDEAD_INIT);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = 10;
    al.ap = 0;
    al.sbyte1 = 0;
    al.sflags &= !ASF_COLLIDE;
    al.stratptr = Some(dead);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);

    g.hooks.play_se(0x03);
}

/// C `setcurrpshape` (strat_player.c:262).
fn setcurrpshape(g: &mut Game, idx: u16) {
    let damage = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING);
    let pick = |addr: u16, g: &Game| {
        let v = g.vars.sv_u16(addr);
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
        let mut delay = g.vars.sv_u8(sv::PLAYER_ROLLDELAY);
        if delay > 0 {
            delay -= 1;
            g.vars.set_sv_u8(sv::PLAYER_ROLLDELAY, delay);
        }

        if allow_start && delay == 0 && lr_down && !lr_prev {
            if pad1(g) & pad::TRIGHT != 0 {
                g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, 32);
            } else {
                g.vars.set_sv_u8(sv::PLAYER_ROLLZVEL, (-32i8) as u8);
            }
            g.vars.set_sv_u8(sv::PLAYER_ROLLZOFF, 0);
        }

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
fn boost_brake_update(g: &mut Game, idx: u16) {
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

    if pad1(g) & pad::X != 0 {
        g.vars.pshipflags2 |= PSF2_BOOSTING;
        g.vars.set_sv_u8(sv::BOOSTCNT, 1);
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
        let ztilt_term = if lr_held { (ztilt as i8 as i16) >> 3 } else { 0 };
        let s7 = plrotz >> 7;
        let plrotz_term = if s7 >= 0 { s7 >> 1 } else { -((-s7) >> 1) };
        let shove = plrotz_term.wrapping_add(ztilt_term);
        let turn180 = g.vars.pshipflags2 & PSF2_TURN180 != 0;
        let al = &mut g.objs.aliens[i];
        if turn180 {
            al.worldx = al.worldx.wrapping_sub(shove);
        } else {
            al.worldx = al.worldx.wrapping_add(shove);
        }
    }

    if !no_ctrl {
        if pad1(g) & pad::LEFT != 0 {
            plrotz = plrotz.wrapping_add(ZROT_SPEED);
            plroty = plroty.wrapping_add(ZROT_SPEED);
            ztilt = ((ztilt as i8) as i32 + (DEG45 as i32 / 15)) as u8;
            if ztilt as i8 > DEG90 as i8 {
                ztilt = DEG90;
            }
        }
        if pad1(g) & pad::RIGHT != 0 {
            plrotz = plrotz.wrapping_sub(ZROT_SPEED);
            plroty = plroty.wrapping_sub(ZROT_SPEED);
            ztilt = ((ztilt as i8) as i32 - (DEG45 as i32 / 15)) as u8;
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

    let turnrot = g.vars.sv_i16(sv::PLAYER_TURNROT);
    let zshake = g.vars.sv_i16(sv::PLAYER_ZSHAKE);
    let zstratadd = g.vars.sv_u8(sv::PLAYER_ZSTRATADD) as i8;
    let rollzoff = g.vars.sv_u8(sv::PLAYER_ROLLZOFF) as i8;

    let al = &mut g.objs.aliens[i];
    al.rotx = (plrotx >> 8) as u8;
    al.roty = ((plroty >> 8) as i32 + (turnrot >> 8) as i32) as u8;
    al.rotz = ((plrotz >> 8) as i32
        + ztilt as i8 as i32
        + (zshake >> 8) as i32
        + zstratadd as i32
        + rollzoff as i32) as u8;

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
    if g.objs.aliens[i].worldx < minx {
        g.objs.aliens[i].worldx = minx;
        arrows |= SPRAR_LEFT;
    }
    if g.objs.aliens[i].worldx > maxx {
        g.objs.aliens[i].worldx = maxx;
        arrows |= SPRAR_RIGHT;
    }

    // Keep vertical gameplay stable in the HD runtime.
    let miny = g.vars.minpmove_y;
    let maxy = g.vars.sv_i16(sv::MAXPMOVEY);
    if g.objs.aliens[i].worldy < miny {
        g.objs.aliens[i].worldy = miny;
        arrows |= SPRAR_UP;
    }
    if g.objs.aliens[i].worldy > maxy {
        g.objs.aliens[i].worldy = maxy;
        arrows |= SPRAR_DOWN;
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

    if g.vars.splayerflymode == SPFM_INSIDE {
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
        // Laser/beam bolts (Y-fire): make them visible. `strat_spawn_projectile`
        // stubs every projectile as shape 0 + ASF_INVISIBLE, which the draw
        // list skips; give player bolts a real shape and clear the flag so they
        // render (the nuke, track=false, keeps the stub — its faithful shape is
        // likewise unregistered).
        let owner_vel = g.objs.aliens[i].vel;
        let laser = &mut g.objs.aliens[shot as usize];
        laser.shape = SHAPE_PLAYER_LASER;
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

        let nuke =
            spawn_player_projectile(g, idx, 0, 0, NUKE_Z_OFFSET, 56, 80, 8, false, false);
        if let Some(nuke) = nuke {
            g.objs.aliens[nuke as usize].sbyte4 = 2;
        }
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
            80,
            45,
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
            80,
            45,
            2,
            true,
            false,
        );
        g.hooks.play_se(0x34);
        return;
    }

    spawn_player_projectile(g, idx, 0, 0, 80, 80, 45, 2, true, false);
    g.hooks.play_se(0x60);
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
        al.vy = vy.wrapping_add(vy >> 2);
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
        al.vy >>= 1;
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
    if g.vars.pstratflags & PSTF_NOVIEWMOVE != 0 {
        let z = g.vars.sv_i16(sv::PVIEWPOSZ);
        g.vars.set_sv_i16(sv::BGSSCROLLZ, z);
        return;
    }

    let mut pviewposz = g
        .vars
        .sv_i16(sv::PVIEWPOSZ)
        .wrapping_add(g.vars.pviewvelz);
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

    if g.vars.splayerflymode == SPFM_INSIDE {
        let al = g.objs.aliens[i];
        g.vars.set_sv_i16(sv::PVIEWPOSX, al.worldx);
        g.vars.set_sv_i16(sv::PVIEWPOSY, al.worldy);
        g.vars.set_sv_i16(sv::PVIEWPOSZ, al.worldz);
    }

    let z = g.vars.sv_i16(sv::PVIEWPOSZ);
    g.vars.set_sv_i16(sv::BGSSCROLLZ, z);
}

/// C `update_viewxy_for_mode` (strat_player.c:682).
fn update_viewxy_for_mode(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    let view_cy = g.vars.sv_i16(sv::VIEWCY);
    if g.vars.game_mode == SPACE_MODE {
        if g.vars.splayerflymode == SPFM_INSIDE {
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
    }

    if g.vars.gameflags & GF_PLAYERDYING != 0 || g.vars.gameflags & GF_PLAYERDEAD != 0 {
        let dead_id = sid(g, K_PLAYERDEAD_STRAT);
        if g.objs.aliens[idx as usize].stratptr == Some(dead_id) {
            playerdead_strat(g, idx);
        }
        return;
    }

    boost_brake_update(g, idx);

    if g.vars.game_mode == SPACE_MODE {
        do_player_limit_x(g, idx);
    } else if g.vars.playerflymode & PFM_DIEFALL != 0 || g.vars.playerflymode & PFM_DIEYROT != 0 {
        do_player_yvel_d2(g, idx);
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

    // C `g_stayblack` defaults to -1 (= "not in a screen-black sequence",
    // normal control). The Rust WRAM slot defaults to 0, and the map VM sets
    // a proxy byte (GSVAR_BYTE1) rather than this slot, so playermove_srou's
    // `STAYBLACK != -1` gate read 0 and locked out ALL steering. Seed -1.
    v.set_sv_i8(sv::STAYBLACK, -1);

    // Same uninitialized-nonzero-default class (audit): the C seeds these in
    // GameVars_Init but the Rust WRAM slots default to 0.
    // g_outdist = OUTVIEWDIST = 120 (bridge-clear pull-out starts here).
    if v.sv_i16(sv::OUTDIST) == 0 {
        v.set_sv_i16(sv::OUTDIST, 120);
    }
    v.set_sv_u8(sv::FIRECNT, 3);
    v.set_sv_u8(sv::FIREDELAY, 1);
    v.set_sv_u8(sv::SPECIALDELAY, 1);
    v.set_sv_u16(sv::SPECWEPCNT, DEFAULT_NUKE_COUNT);
    v.set_sv_u8(sv::PNUMHITS, 0);
    v.set_sv_u8(sv::ARROWS, 0);

    v.set_sv_u16(sv::PLAYERSHAPE, SHAPE_ARWING);
    v.set_sv_u16(sv::PLAYERSHAPEL, SHAPE_ARWING);
    v.set_sv_u16(sv::PLAYERSHAPER, SHAPE_ARWING);
    v.set_sv_u16(sv::PLAYERSHAPELR, SHAPE_ARWING);

    v.internal_playpt = idx as i16;

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
fn playeronplanet_init(g: &mut Game, idx: u16) {
    // s_playerctrl on
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);

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

    // lda #0 / sta.l alx_snd1,x — stop the hangar engine channel
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
        // clshipbridgeboost not yet ported; colldisable placeholder (C parity).
        g.objs.aliens[dup as usize].sflags |= ASF_COLLDISABLE;
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

    // boost_sprite — visual boost effect (not yet ported, skip)

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
}

/// C `viewopening_strat` (GISTRATS.ASM:629-683): multi-state camera that
/// chases toward player position offsets, then drives viewpos verbatim.
fn viewopening_strat(g: &mut Game, idx: u16) {
    let i = idx as usize;

    // s_set_var W,svar_word1,player_posx / word2,#-30 / word3,player_posz
    let mut w1 = g.vars.player_posx;
    let mut w2: i16 = -30;
    let mut w3 = g.vars.player_posz;

    match g.objs.aliens[i].stratstate {
        0 => {
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
                // s_next_state x
                g.objs.aliens[i].stratstate = 1;
                // s_set_alvar B,x,al_sbyte1,#80
                g.objs.aliens[i].sbyte1 = 80;
                // s_or_var B,gameflags,#gf_stratdone2
                g.vars.gameflags |= GF_STRATDONE2;
                // s_or_var B,pshipflags3,#psf3_enginesnd
                g.vars.pshipflags3 |= PSF3_ENGINESND;
            }
        }
        _ => {
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
    // boost_sprite — SNES sprite overlay, not applicable in HD
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
    // Byte index / 2 = word index into viewfloattab
    g.objs.aliens[i].worldy = SHIPINTRO_VIEW_FLOAT[(b2 / 2) as usize];

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
    {
        let v = &mut g.vars;
        v.set_sv_i16(sv::VIEWCY, LTUNNEL_VIEWCY);
        v.set_sv_i16(sv::MINPMOVEX, LTUNNEL_MINX);
        v.set_sv_i16(sv::MAXPMOVEX, LTUNNEL_MAXX);
        v.set_sv_i16(sv::MINMMOVEX, LTUNNEL_MMINX);
        v.set_sv_i16(sv::MAXMMOVEX, LTUNNEL_MMAXX);
        v.set_sv_i16(sv::MAXMMOVEY, LTUNNEL_MMAXY);
        v.set_sv_i16(sv::MINPWMOVEY, LTUNNEL_MINY);
        v.set_sv_i16(sv::MAXPWMOVEY, LTUNNEL_MAXY + 5);
        v.minpmove_y = LTUNNEL_MINY;
        v.set_sv_i16(sv::MAXPMOVEY, LTUNNEL_MAXY + PLAYERB_YSTOP);
        v.playerflymode = PFM_DIEFALL | PFM_SHADOWS;
        v.set_sv_u8(sv::PMOVELIMITAND, PML_ALL);
        v.set_sv_u8(sv::MISSBOUNDFLAGS, MB_LEFT | MB_RIGHT | MB_BOTTOM);
        // Ltunnel_gameflagsON = 0 (no-op)
        v.gameflags &= !GF_VIEWROT; // ~Ltunnel_gameflagsOFF
        v.pstratflags &= !PSTF_NOVIEWMOVE;
        v.pshipflags3 |= PSF3_ENGINESND;
        // Ltunnel_macro
        v.pshipflags2 &= !PSF2_NOSPARK;
        v.pstratflags &= !(PSTF_INSEQ | PSTF_NOTDIE);
    }
    g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    g.vars.pshipflags3 |= PSF3_INTUNNEL;

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

    // lda #2 / sta fadedir
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
}

// Re-export the sv module for tests and other-lane wiring convenience.
pub use crate::common::sv as player_sv;
