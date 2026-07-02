//! Common strategy routines used by all object behaviors.
//!
//! Port (C oracle): `src/strat/strat_common.c/h` (STRATROU.ASM -> C):
//! velocity generation (alvelvecs, alvel3vecs), position updates
//! (addvecs, addalvecs), chase functions (Fchase, Achase, speedto),
//! percentage scaling (perc75/62/87/...), distance/angle helpers, object
//! management (makeobj, removeobj) and the shared lightweight projectile.
//!
//! Integer discipline matches C exactly: 16-bit wrapping adds, arithmetic
//! shifts on signed values, float->int truncation where the C build casts,
//! and the same f32 trig constants (`2.0f * 3.14159265f / 256.0f`).
//!
//! Strat-lane globals that `sf_game::vars::GameVars` does not carry as
//! fields are stored in the WRAM mirror (`GameVars::ram`) at the fixed
//! addresses in [`sv`], accessed through [`StratRam`]. This mirrors the
//! SNES (all of these lived in WRAM) and keeps plain-`fn` strategies
//! stateless. See the lane report for the GameVars fields these should
//! eventually become.

use sf_game::alien::{
    Alien, StratId, ACF_FIRSTFRAME, ACF_WEAPON, ASF_INVISIBLE, ATLASER, ATZREMOVE, NUMBER_AL,
};
use sf_game::vars::GameVars;
use sf_game::Game;

// ============================================================
// Strat-lane WRAM variable block
// ============================================================

/// WRAM addresses for strat-lane globals (C `src/game/game_vars.c` globals
/// that are not yet `GameVars` fields, plus the C file-statics of
/// `strat_player.c`). Block 0x0500..0x0570 is unused by the map VM's
/// registered `wm::*` addresses (sf-map `consts::wm` spans 0x0304..0x0320).
pub mod sv {
    /// C `g_rndval` (src/sf_rtl.c:16) — PRNG state, boot value 0.
    pub const RNDVAL: u16 = 0x0500;
    /// Registry id (+1; 0 = not yet registered) of
    /// [`super::strat_projectile_tick`]; the collide handler is id+1.
    pub const SID_PROJ: u16 = 0x0502;
    /// Registry base (+1) of the player strategy block (player.rs).
    pub const SID_PLAYER_BASE: u16 = 0x0504;

    // C strat_player.c file-statics s_plrotx/y/z (8.8 fixed-point).
    pub const PLROTX: u16 = 0x0506;
    pub const PLROTY: u16 = 0x0508;
    pub const PLROTZ: u16 = 0x050A;

    /// C `g_player_speed` (int16).
    pub const PLAYER_SPEED: u16 = 0x050C;
    /// C `g_player_tospeed` (uint8).
    pub const PLAYER_TOSPEED: u16 = 0x050E;
    /// C `g_player_medspeed` (uint8).
    pub const PLAYER_MEDSPEED: u16 = 0x050F;
    /// C `g_player_turnrot` (int16).
    pub const PLAYER_TURNROT: u16 = 0x0510;
    /// C `g_player_zshake` (int16).
    pub const PLAYER_ZSHAKE: u16 = 0x0512;
    /// C `g_player_Ztilt` (uint8, int8 semantics).
    pub const PLAYER_ZTILT: u16 = 0x0514;
    /// C `g_player_zstratadd` (uint8).
    pub const PLAYER_ZSTRATADD: u16 = 0x0515;
    /// C `g_player_rollZvel` (uint8, int8 semantics).
    pub const PLAYER_ROLLZVEL: u16 = 0x0516;
    /// C `g_player_rollZoff` (uint8, int8 semantics).
    pub const PLAYER_ROLLZOFF: u16 = 0x0517;
    /// C `g_player_rolldelay` (uint8).
    pub const PLAYER_ROLLDELAY: u16 = 0x0518;
    /// C `g_player_noctrlcnt` (uint8).
    pub const PLAYER_NOCTRLCNT: u16 = 0x0519;
    /// C `g_viewshakeX/Y/Z` (uint8).
    pub const VIEWSHAKEX: u16 = 0x051A;
    pub const VIEWSHAKEY: u16 = 0x051B;
    pub const VIEWSHAKEZ: u16 = 0x051C;
    /// C `g_screenflashcnt` / `g_screenflashtype` (uint8).
    pub const SCREENFLASHCNT: u16 = 0x051D;
    pub const SCREENFLASHTYPE: u16 = 0x051E;
    /// C `g_pnumhits` (uint8).
    pub const PNUMHITS: u16 = 0x051F;
    /// C `g_lives` (uint8).
    pub const LIVES: u16 = 0x0520;
    /// C `g_stayblack` (int8; -1 = normal gameplay control path).
    pub const STAYBLACK: u16 = 0x0521;
    /// C `g_doingwipe` (uint8).
    pub const DOINGWIPE: u16 = 0x0522;
    /// C `g_arrows` (uint8, SPRAR_*).
    pub const ARROWS: u16 = 0x0523;
    /// C `g_viewCY` (int16).
    pub const VIEWCY: u16 = 0x0524;
    /// C `g_minpmoveX` / `g_maxpmoveX` (int16). (`g_minpmoveY` is the
    /// GameVars field `minpmove_y`.)
    pub const MINPMOVEX: u16 = 0x0526;
    pub const MAXPMOVEX: u16 = 0x0528;
    /// C `g_maxpmoveY` (int16).
    pub const MAXPMOVEY: u16 = 0x052A;
    /// C `g_minMmoveX` / `g_maxMmoveX` / `g_maxMmoveY` (int16).
    pub const MINMMOVEX: u16 = 0x052C;
    pub const MAXMMOVEX: u16 = 0x052E;
    pub const MAXMMOVEY: u16 = 0x0530;
    /// C `g_minpWmoveY` / `g_maxpWmoveY` (int16).
    pub const MINPWMOVEY: u16 = 0x0532;
    pub const MAXPWMOVEY: u16 = 0x0534;
    /// C `g_pmovelimit` / `g_pmovelimitAND` (uint8, PML_*).
    pub const PMOVELIMIT: u16 = 0x0536;
    pub const PMOVELIMITAND: u16 = 0x0537;
    /// C `g_missboundflags` (uint8, MB_*).
    pub const MISSBOUNDFLAGS: u16 = 0x0538;
    /// C `g_boostcnt` (uint8).
    pub const BOOSTCNT: u16 = 0x0539;
    /// C `g_boostobj` (int16 alien index).
    pub const BOOSTOBJ: u16 = 0x053A;
    /// C `g_pviewposx/y/z` (int16).
    pub const PVIEWPOSX: u16 = 0x053C;
    pub const PVIEWPOSY: u16 = 0x053E;
    pub const PVIEWPOSZ: u16 = 0x0540;
    /// C `g_bgsscrollZ` (int16).
    pub const BGSSCROLLZ: u16 = 0x0542;
    /// C `g_hudrot` (int16).
    pub const HUDROT: u16 = 0x0544;
    /// C `g_outvx` / `g_outvy` / `g_outdist` (int16).
    pub const OUTVX: u16 = 0x0546;
    pub const OUTVY: u16 = 0x0548;
    pub const OUTDIST: u16 = 0x054A;
    /// C `g_viewtype` (uint8, VIEWTYPE_*).
    pub const VIEWTYPE: u16 = 0x054C;
    /// C `g_fadedir` (int8).
    pub const FADEDIR: u16 = 0x054D;
    /// C `g_viewtoobj` (int16 alien index).
    pub const VIEWTOOBJ: u16 = 0x054E;
    /// C `g_viewposx/y/z` (int16) — fixed-camera position.
    pub const VIEWPOSX: u16 = 0x0550;
    pub const VIEWPOSY: u16 = 0x0552;
    pub const VIEWPOSZ: u16 = 0x0554;
    /// C `g_psvar_byte1/2/3` (uint8). (psvar_word1..4 are GameVars fields.)
    pub const PSVAR_BYTE1: u16 = 0x0556;
    pub const PSVAR_BYTE2: u16 = 0x0557;
    pub const PSVAR_BYTE3: u16 = 0x0558;
    /// C `g_nomaxbg2Yscroll` (uint8).
    pub const NOMAXBG2YSCROLL: u16 = 0x0559;
    /// C `g_bg2Yscroll` (int16).
    pub const BG2YSCROLL: u16 = 0x055A;
    /// C `g_svar_word1/2/3` (int16).
    pub const SVAR_WORD1: u16 = 0x055C;
    pub const SVAR_WORD2: u16 = 0x055E;
    pub const SVAR_WORD3: u16 = 0x0560;
    /// C `g_playershape{,L,R,LR}` (uint16).
    pub const PLAYERSHAPE: u16 = 0x0562;
    pub const PLAYERSHAPEL: u16 = 0x0564;
    pub const PLAYERSHAPER: u16 = 0x0566;
    pub const PLAYERSHAPELR: u16 = 0x0568;
    /// C `g_firecnt` / `g_firedelay` / `g_specialdelay` (uint8).
    pub const FIRECNT: u16 = 0x056A;
    pub const FIREDELAY: u16 = 0x056B;
    pub const SPECIALDELAY: u16 = 0x056C;
    /// C `g_specwepcnt` (uint16) — nova bomb inventory.
    pub const SPECWEPCNT: u16 = 0x056E;

    /// C `g_numplasers` (uint8) — shared with the map VM at the sf-map
    /// `consts::wm::NUMPLASERS` address so map-script writes stay visible.
    pub const NUMPLASERS: u16 = 0x0313;
}

/// Typed WRAM-mirror access for the [`sv`] block (little-endian, matching
/// C `world_read_ext16`).
pub trait StratRam {
    fn sv_u8(&self, addr: u16) -> u8;
    fn sv_i8(&self, addr: u16) -> i8;
    fn sv_u16(&self, addr: u16) -> u16;
    fn sv_i16(&self, addr: u16) -> i16;
    fn set_sv_u8(&mut self, addr: u16, v: u8);
    fn set_sv_i8(&mut self, addr: u16, v: i8);
    fn set_sv_u16(&mut self, addr: u16, v: u16);
    fn set_sv_i16(&mut self, addr: u16, v: i16);
}

impl StratRam for GameVars {
    fn sv_u8(&self, addr: u16) -> u8 {
        self.read_ext8(addr)
    }
    fn sv_i8(&self, addr: u16) -> i8 {
        self.read_ext8(addr) as i8
    }
    fn sv_u16(&self, addr: u16) -> u16 {
        self.read_ext16(addr)
    }
    fn sv_i16(&self, addr: u16) -> i16 {
        self.read_ext16(addr) as i16
    }
    fn set_sv_u8(&mut self, addr: u16, v: u8) {
        self.write_ext8(addr, v);
    }
    fn set_sv_i8(&mut self, addr: u16, v: i8) {
        self.write_ext8(addr, v as u8);
    }
    fn set_sv_u16(&mut self, addr: u16, v: u16) {
        self.write_ext16(addr, v);
    }
    fn set_sv_i16(&mut self, addr: u16, v: i16) {
        self.write_ext16(addr, v as u16);
    }
}

/// C `SfRtl_Random()` (src/sf_rtl.c:192) with `PRNG_NEXT`
/// (src/types.h:57): `rnd * 91 + 0x61D7`, 16-bit. State lives at
/// [`sv::RNDVAL`] (boot value 0, like the C global).
pub fn sf_random(vars: &mut GameVars) -> u16 {
    let v = vars
        .sv_u16(sv::RNDVAL)
        .wrapping_mul(91)
        .wrapping_add(0x61D7);
    vars.set_sv_u16(sv::RNDVAL, v);
    v
}

// ============================================================
// Trig (C strat_common.c s_sin/s_cos tables)
// ============================================================

/// C `init_tables()` constant: `2.0f * 3.14159265f / 256.0f`, folded in
/// f32 exactly like the C compiler does.
const ANGLE_TO_RAD: f32 = 2.0f32 * 3.141_592_65_f32 / 256.0f32;

/// `s_sin[angle]` — computed on demand; identical to the C table entry
/// because the input expression and libm call match bit-for-bit.
#[inline]
pub fn snes_sin(angle: u8) -> f32 {
    (angle as f32 * ANGLE_TO_RAD).sin()
}

/// `s_cos[angle]`.
#[inline]
pub fn snes_cos(angle: u8) -> f32 {
    (angle as f32 * ANGLE_TO_RAD).cos()
}

// ============================================================
// Chase / Transition (C strat_common.c:36-80)
// ============================================================

/// C `Strat_Chase` (Fchase_A, STRATMAC.INC). 16-bit wrapping step.
pub fn strat_chase(mut current: i16, target: i16, rate: i16) -> i16 {
    if current < target {
        current = current.wrapping_add(rate);
        if current > target {
            current = target;
        }
    } else if current > target {
        current = current.wrapping_sub(rate);
        if current < target {
            current = target;
        }
    }
    current
}

/// C `Strat_Chase8`.
pub fn strat_chase8(mut current: u8, target: u8, rate: u8) -> u8 {
    if current < target {
        current = current.wrapping_add(rate);
        if current > target {
            current = target;
        }
    } else if current > target {
        if current - target < rate {
            current = target;
        } else {
            current -= rate;
        }
    }
    current
}

/// C `Strat_ChaseProportional` (Achase macros): step = diff >> shift,
/// minimum magnitude 1.
pub fn strat_chase_proportional(current: i16, target: i16, shift: u32) -> i16 {
    if current == target {
        return current;
    }
    let diff = current.wrapping_sub(target);
    let mut step = diff >> shift;
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    current.wrapping_sub(step)
}

/// C `Strat_SpeedTo` (sr_speedto, STRATROU.ASM:2707-2733). Returns true
/// when the target speed is reached.
pub fn strat_speed_to(al: &mut Alien, target_speed: u8, rate: u8) -> bool {
    if al.vel == target_speed {
        return true;
    }
    if al.vel < target_speed {
        al.vel = al.vel.wrapping_add(rate);
        if al.vel > target_speed {
            al.vel = target_speed;
        }
    } else if al.vel - target_speed < rate {
        al.vel = target_speed;
    } else {
        al.vel -= rate;
    }
    al.vel == target_speed
}

// ============================================================
// Percentage scaling (C strat_common.c:84-107, STRATROU.ASM perc*)
// ============================================================

/// C `Strat_Perc75`: val * 3/4 = val/2 + val/4 (arithmetic shifts).
pub fn strat_perc75(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 2)
}

/// C `Strat_Perc87`: val * 7/8.
pub fn strat_perc87(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 2).wrapping_add(val >> 3)
}

/// C `Strat_Perc62`: val * 5/8.
pub fn strat_perc62(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 3)
}

/// C `Strat_Perc56`: val * 9/16.
pub fn strat_perc56(val: i16) -> i16 {
    (val >> 1).wrapping_add(val >> 4)
}

/// C `Strat_Perc93`: val * 15/16.
pub fn strat_perc93(val: i16) -> i16 {
    val.wrapping_sub(val >> 4)
}

// ============================================================
// Velocity vector generation (C strat_common.c:111-161)
// ============================================================

/// C `Strat_GenVecs2D` (alvelvecs_l, STRATROU.ASM:100-148): XZ velocity
/// from Y rotation + speed; vy = 0.
pub fn strat_gen_vecs_2d(al: &mut Alien) {
    let angle = al.roty;
    let speed = al.vel as i16 as f32;
    al.vx = (speed * snes_sin(angle)) as i16;
    al.vy = 0;
    al.vz = (speed * snes_cos(angle)) as i16;
}

/// C `Strat_GenVecs3D` (alvel3vecs_l, STRATROU.ASM:221-283): 3D velocity
/// from pitch (negated rotx, ASM convention) and yaw.
pub fn strat_gen_vecs_3d(al: &mut Alien) {
    let yaw = al.roty;
    let pitch = (al.rotx as i8).wrapping_neg() as u8;
    let speed = al.vel as i16 as f32;

    let cy = snes_cos(pitch);
    let sy = snes_sin(pitch);

    al.vx = (speed * snes_sin(yaw) * cy) as i16;
    al.vy = (speed * sy) as i16;
    al.vz = (speed * snes_cos(yaw) * cy) as i16;
}

/// C `Strat_GenSideVecs` (sidevecs_l): sideways vector, angle rotated 90
/// degrees (64 - roty).
pub fn strat_gen_side_vecs(al: &Alien) -> (i16, i16) {
    let angle = 64u8.wrapping_sub(al.roty);
    let speed = al.vel as i16 as f32;
    ((speed * snes_sin(angle)) as i16, (speed * snes_cos(angle)) as i16)
}

/// C `Strat_GenFrontVecs` (frontvecs_l).
pub fn strat_gen_front_vecs(al: &Alien) -> (i16, i16) {
    let angle = al.roty;
    let speed = al.vel as i16 as f32;
    ((speed * snes_sin(angle)) as i16, (speed * snes_cos(angle)) as i16)
}

// ============================================================
// Position update (C strat_common.c:165-177)
// ============================================================

/// C `Strat_ApplyVelocity` (addalvecs_l, STRATROU.ASM:518-535).
pub fn strat_apply_velocity(al: &mut Alien) {
    al.worldx = al.worldx.wrapping_add(al.vx);
    al.worldy = al.worldy.wrapping_add(al.vy);
    al.worldz = al.worldz.wrapping_add(al.vz);
}

/// C `Strat_AddToPos` (addvecs_l, STRATROU.ASM:497).
pub fn strat_add_to_pos(al: &mut Alien, dx: i16, dy: i16, dz: i16) {
    al.worldx = al.worldx.wrapping_add(dx);
    al.worldy = al.worldy.wrapping_add(dy);
    al.worldz = al.worldz.wrapping_add(dz);
}

// ============================================================
// Distance / angle (C strat_common.c:181-198)
// ============================================================

/// C `Strat_DistXZ` (xzdiffs_l): approximate Manhattan distance in the XZ
/// plane. Widths mirror the C int-promotion then int16 truncation.
pub fn strat_dist_xz(a: &Alien, b: &Alien) -> i16 {
    let dx = ((b.worldx as i32 - a.worldx as i32).abs()) as i16;
    let dz = ((b.worldz as i32 - a.worldz as i32).abs()) as i16;
    dx.wrapping_add(dz)
}

/// C `Strat_AngleXZ` (anglexy_l): 0-255 angle from src to dst in XZ.
/// Uses f32 atan2 exactly like the C build; the final cast truncates
/// through i32 like the x86 float->uint8 conversion.
pub fn strat_angle_xz(src: &Alien, dst: &Alien) -> u8 {
    let dx = dst.worldx.wrapping_sub(src.worldx) as f32;
    let dz = dst.worldz.wrapping_sub(src.worldz) as f32;
    let mut angle = dx.atan2(dz);
    if angle < 0.0 {
        angle += 2.0f32 * 3.141_592_65_f32;
    }
    ((angle * (256.0f32 / (2.0f32 * 3.141_592_65_f32))) as i32) as u8
}

// ============================================================
// Object management (C strat_common.c:202-242)
// ============================================================

/// C `Strat_RemoveObj` (s_remove_obj / s_doremove): flag the current alien
/// for removal by the dostrats loop.
pub fn strat_remove_obj(g: &mut Game) {
    g.objs.aldead = 1;
}

/// C `Strat_InitObjVars` (init_objvars_l) — re-exported from sf-game where
/// the map-VM spawn path already carries it.
pub use sf_game::obj::strat_init_obj_vars;

/// C `Strat_MakeObj` (s_make_obj + makeobj_l): alloc + init vars + shape.
pub fn strat_make_obj(g: &mut Game, shape_id: u16) -> Option<u16> {
    let idx = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].shape = shape_id;
    Some(idx)
}

/// C `Strat_CountDown`: decrement `al->count`, true when it has reached 0.
pub fn strat_count_down(al: &mut Alien) -> bool {
    if al.count > 0 {
        al.count -= 1;
        return false;
    }
    true
}

// ============================================================
// Shared lightweight projectile (C strat_common.c:244-319)
// ============================================================

/// C `projectile_mark_dead` (strat_common.c:244).
fn projectile_mark_dead(g: &mut Game, idx: u16) {
    let sbyte6 = g.objs.aliens[idx as usize].sbyte6;
    if sbyte6 & 1 != 0 {
        g.objs.aliens[idx as usize].sbyte6 = sbyte6 & !1u8;
        let n = g.vars.sv_u8(sv::NUMPLASERS);
        if n > 0 {
            g.vars.set_sv_u8(sv::NUMPLASERS, n - 1);
        }
    }
    g.objs.aldead = 1;
}

/// C `Strat_ProjectileOnCollide`.
pub fn strat_projectile_on_collide(g: &mut Game, idx: u16) {
    projectile_mark_dead(g, idx);
}

/// C `Strat_ProjectileTick`.
pub fn strat_projectile_tick(g: &mut Game, idx: u16) {
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

    let count = g.objs.aliens[idx as usize].count;
    let count = if count > 0 { count - 1 } else { count };
    g.objs.aliens[idx as usize].count = count;
    if count == 0 {
        projectile_mark_dead(g, idx);
        return;
    }

    // Obj_GetPlayer(): slot 0 when active (src/game/obj.c:125).
    if let Some(player) = g.objs.player() {
        let dz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_sub(player.worldz);
        if !(-12000..=12000).contains(&dz) {
            projectile_mark_dead(g, idx);
        }
    }
}

/// Registry ids of the projectile tick/collide strategies, registering
/// them on first use (C takes the function addresses directly).
pub fn projectile_strat_ids(g: &mut Game) -> (StratId, StratId) {
    let stored = g.vars.sv_u16(sv::SID_PROJ);
    if stored != 0 {
        return (StratId(stored - 1), StratId(stored));
    }
    let tick = g.world.register_strategy(strat_projectile_tick);
    let coll = g.world.register_strategy(strat_projectile_on_collide);
    debug_assert_eq!(coll.0, tick.0 + 1);
    g.vars.set_sv_u16(sv::SID_PROJ, tick.0 + 1);
    (tick, coll)
}

/// C `Strat_SpawnProjectile` (strat_common.c:278). `owner` is an alien
/// slot index (C passes the pointer). Returns the new slot.
#[allow(clippy::too_many_arguments)]
pub fn strat_spawn_projectile(
    g: &mut Game,
    owner: Option<u16>,
    off_x: i16,
    off_y: i16,
    off_z: i16,
    rot_x: u8,
    rot_y: u8,
    speed: u8,
    lifetime: u8,
    ap: u8,
    coll_type_bit: u8,
) -> Option<u16> {
    let (tick_id, coll_id) = projectile_strat_ids(g);
    let idx = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);

    let owner_al = owner
        .filter(|&o| (o as usize) < NUMBER_AL)
        .map(|o| g.objs.aliens[o as usize]);

    let al = &mut g.objs.aliens[idx as usize];
    al.shape = 0; // Visual weapon meshes are not wired yet.
    al.sflags |= ASF_INVISIBLE;
    al.type_ |= ATLASER | ATZREMOVE;

    al.stratptr = Some(tick_id);
    al.collstratptr = Some(coll_id);
    al.expstratptr = Some(coll_id);

    al.count = if lifetime != 0 { lifetime } else { 40 };
    al.hp = 1;
    al.ap = ap;
    al.vel = speed;
    al.rotx = rot_x;
    al.roty = rot_y;

    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | coll_type_bit;

    match (owner, owner_al) {
        (Some(o), Some(own)) => {
            al.worldx = own.worldx.wrapping_add(off_x);
            al.worldy = own.worldy.wrapping_add(off_y);
            al.worldz = own.worldz.wrapping_add(off_z);
            // C: al->immuneptr = (uint16)(owner - g_aliens) — raw index.
            al.immuneptr = o;
        }
        _ => {
            al.worldx = off_x;
            al.worldy = off_y;
            al.worldz = off_z;
        }
    }

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    Some(idx)
}

// ============================================================
// Sound (C strat_common.c:323)
// ============================================================

/// C `Strat_TrigSE` (trigse macro, MACROS.INC).
pub fn strat_trig_se(g: &mut Game, sound_id: u8) {
    g.hooks.play_se(sound_id);
}

// ============================================================
// Unprefixed aliases — the canonical short names the other strat lanes'
// compat modules document (`crate::common::chase` etc.); consolidation
// swaps their private duplicates for these.
// ============================================================
pub use self::strat_add_to_pos as add_to_pos;
pub use self::strat_angle_xz as angle_xz;
pub use self::strat_apply_velocity as apply_velocity;
pub use self::strat_chase as chase;
pub use self::strat_chase8 as chase8;
pub use self::strat_chase_proportional as chase_proportional;
pub use self::strat_count_down as count_down;
pub use self::strat_dist_xz as dist_xz;
pub use self::strat_gen_front_vecs as gen_front_vecs;
pub use self::strat_gen_side_vecs as gen_side_vecs;
pub use self::strat_gen_vecs_2d as gen_vecs_2d;
pub use self::strat_gen_vecs_3d as gen_vecs_3d;
pub use self::strat_make_obj as make_obj;
pub use self::strat_perc56 as perc56;
pub use self::strat_perc62 as perc62;
pub use self::strat_perc75 as perc75;
pub use self::strat_perc87 as perc87;
pub use self::strat_perc93 as perc93;
pub use self::strat_projectile_on_collide as projectile_on_collide;
pub use self::strat_projectile_tick as projectile_tick;
pub use self::strat_remove_obj as remove_obj;
pub use self::strat_spawn_projectile as spawn_projectile;
pub use self::strat_speed_to as speed_to;
pub use self::strat_trig_se as trig_se;
