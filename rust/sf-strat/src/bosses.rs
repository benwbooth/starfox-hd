//! Stage bosses ported in their own C translation units (RIIR wave 3,
//! enemy_b/bosses lane).
//!
//! C oracle (function-for-function, C citations inline):
//! - `src/strat/strat_boss2.c`     — Boss2 spinning-top (9 children,
//!   6-state machine, plasma orbiters)
//! - `src/strat/strat_boss_sea.c`  — Seamon / BossG / seamon fish /
//!   flyingfish (multi-wave gating via gsvar_byte1 + maptrigger bits)
//! - `src/strat/strat_boss8.c`     — Boss8 washmap cast (shell / cover /
//!   beams / launchers / pillars / shrapnel, GF_BOSSDEAD + GF_STAGEDONE
//!   release)
//!
//! Shared helpers come from `crate::enemy_b::eb_compat` (which re-exports
//! the canonical `crate::common` + `crate::enemy_a` items). The C boss
//! files carried their own local copies of a few strat_enemy.c statics
//! (`b8_achase_angle`, `sea_add_player_z`, `boss2_falldown_yvec`,
//! `b8_wallrot`, ...); those are ported as private module fns here.

#![allow(dead_code)]

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE4, ACF_FIRSTFRAME, ACF_WEAPON, AFEXP, ASF3_REALOBJ, ASF4_SFLAG8,
    ASF_COLLDISABLE, ASF_COLLIDE, ASF_HITFLASH, ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATLASER,
    ATMISSILE, ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::{COLLTYPE_ENEMY1, GF_BOSSDEAD, GF_STAGEDONE, HARD_AP, HARD_HP};
use sf_game::world::World;

// Canonical strat_common.c ports.
use crate::common::{
    strat_angle_xz, strat_apply_velocity, strat_gen_vecs_3d, strat_init_obj_vars,
    strat_projectile_on_collide,
};
use crate::common::strat_chase_proportional as chase_proportional;
use crate::common::strat_count_down as count_down;
use crate::common::strat_make_obj as make_obj;
use crate::common::strat_spawn_projectile as spawn_projectile;
use crate::common::strat_speed_to as speed_to;

// Canonical strat_enemy.c helpers (crate::enemy_a pub / pub(crate) surface).
use crate::enemy_a::{
    add_player_z, addrnd2pos_xy, boss_attach_child_to_mother, boss_child_from_index_raw,
    boss_count_children, boss_dying, boss_find_child_obj, boss_get_mother_obj,
    boss_keeprel_to_player, boss_obj_index_or_null, bossflags, copy_pos, currentlevel, ea_random,
    pviewposz, set_bossflags, strat_boss_explode_init, strat_cos, strat_explode, strat_hit_flash,
    strat_pitch_toward, strat_sin,
};

// ============================================================
// Flag constants (C variables.h / obj.h / strat_enemy.h) not carried by
// the shared sf-game/enemy_a surface. Values verbatim; local copies keep
// this lane independent of the concurrently-edited enemy_b::eb_compat.
// ============================================================
const ASF2_RELEXPLODE: u8 = 0x04;
const ASF2_NOEXPSND: u8 = 0x08;
const ASF2_SFLAG1: u8 = 0x10;
const ASF4_NOPOLYEXP: u8 = 0x04;
const BF_DYING: u8 = 16;
const BF_FLAG1: u8 = 1;
const BF_FLAG2: u8 = 2;
const BF_FLAG3: u8 = 4;
const PSF_NOCTRL: u8 = 32;
const PSF_NOFIRE: u8 = 64;
const PSF2_PLAYERHP0: u8 = 128;
const DEG180: u8 = 128;
const DEG90: u8 = 64;
const DEG45: u8 = 32;
const DEG22: u8 = 16;
const COLLTYPE_ENEMY2: u8 = 0x02;
const COLLTYPE_ENEMYWEAP: u8 = 0x04;
const COLLTYPE_ZENEMY: u8 = 0x08;

// ============================================================
// Local WRAM/registry primitives (mirror of the stable eb_compat surface;
// kept local to avoid a cross-lane build dependency on enemy_b).
// ============================================================
mod ebwm {
    /// C `g_gsvar_byte1` (= sf_map wm::GSVAR_BYTE1).
    pub const GSVAR_BYTE1: u16 = 0x0310;
    /// C `g_maptrigger`.
    pub const MAPTRIGGER: u16 = 0x0311;
    /// C `g_bossmaxhp` (wm mirror).
    pub const BOSSMAXHP: u16 = 0x0316;
    /// C `g_bg2Xscroll` (enemy_a 0x1F block).
    pub const BG2XSCROLL: u16 = 0x1F30;
}

#[inline]
fn wm8(g: &Game, addr: u16) -> u8 {
    g.vars.read_ext8(addr)
}
#[inline]
fn wm8_set(g: &mut Game, addr: u16, v: u8) {
    g.vars.write_ext8(addr, v);
}
#[inline]
fn wm16s(g: &Game, addr: u16) -> i16 {
    g.vars.read_ext16(addr) as i16
}
#[inline]
fn wm16s_set(g: &mut Game, addr: u16, v: i16) {
    g.vars.write_ext16(addr, v as u16);
}

/// C `g_bossmaxhp` — GameVars field + wm mirror kept coherent.
#[inline]
fn bossmaxhp(g: &Game) -> u16 {
    g.vars.bossmaxhp
}
#[inline]
fn set_bossmaxhp(g: &mut Game, v: u16) {
    g.vars.bossmaxhp = v;
    g.vars.write_ext16(ebwm::BOSSMAXHP, v);
}

/// C `SfRtl_Random()` — the one shared PRNG cell (0x1F00, enemy_a::wm).
#[inline]
fn sfrtl_random(g: &mut Game) -> u16 {
    ea_random(g)
}

/// C `Obj_GetPlayer` (src/game/obj.c:125): slot 0 when active.
#[inline]
fn player_idx(g: &Game) -> Option<u16> {
    if g.objs.aliens[0].active {
        Some(0)
    } else {
        None
    }
}
#[inline]
fn player(g: &Game) -> Option<Alien> {
    player_idx(g).map(|i| g.objs.aliens[i as usize])
}

#[inline]
fn play_se(g: &mut Game, id: u8) {
    g.hooks.play_se(id);
}

/// Registry id for `f`, registering on first use (C: `self->stratptr = f`).
fn sid(g: &mut Game, f: StrategyFn) -> StratId {
    if let Some(pos) = g
        .world
        .strat_registry
        .iter()
        .position(|&r| r as usize == f as usize)
    {
        StratId(pos as u16)
    } else {
        g.world.register_strategy(f)
    }
}

// ============================================================
// Registration constants (table lane contract) — preserved verbatim.
// ============================================================

/// C `IS_SEAMON` (strat_boss_sea.c:53, ISTRATS.ASM def_Istrat 81).
pub const IS_SEAMON: usize = 81;
/// C `IS_BOSSG` (strat_boss_sea.c:54, def_Istrat 144).
pub const IS_BOSSG: usize = 144;
/// C `IS_BOSS2` (strat_boss2.c:57, def_Istrat 108).
pub const IS_BOSS2: usize = 108;
/// C `IS_NUCLEUSBEAML` (strat_boss8.c:64, def_Istrat 82).
pub const IS_NUCLEUSBEAML: usize = 82;
/// C `IS_BOSS8SHRAP` (strat_boss8.c:65, def_Istrat 83).
pub const IS_BOSS8SHRAP: usize = 83;
/// C `IS_BOSS8` (strat_boss8.c:66, def_Istrat 84).
pub const IS_BOSS8: usize = 84;
/// C `IS_NUCLEUSLAUNCHER` (strat_boss8.c:67, def_Istrat 86).
pub const IS_NUCLEUSLAUNCHER: usize = 86;
/// C `IS_NUCLEUSPILLAR` (strat_boss8.c:68, def_Istrat 87).
pub const IS_NUCLEUSPILLAR: usize = 87;

/// C `STRAT_ADDR_BOSSSEAMON` (strat_boss_sea.c:45).
pub const STRAT_ADDR_BOSSSEAMON: u32 = 0x030005;
/// C `STRAT_ADDR_BOSSG` (strat_boss_sea.c:46).
pub const STRAT_ADDR_BOSSG: u32 = 0x030006;
/// C `B8_STRAT_ADDR_BOSS8` (strat_boss8.c:71).
pub const B8_STRAT_ADDR_BOSS8: u32 = 0x060014;
/// C `B8_STRAT_ADDR_NUCLEUSLAUNCHER` (strat_boss8.c:72).
pub const B8_STRAT_ADDR_NUCLEUSLAUNCHER: u32 = 0x060015;
/// C `B8_STRAT_ADDR_NUCLEUSPILLAR` (strat_boss8.c:73).
pub const B8_STRAT_ADDR_NUCLEUSPILLAR: u32 = 0x060016;

// ============================================================
// Shared WRAM cells (C globals `sf_game::vars` does not carry as fields).
// ============================================================

/// C `g_gsvar_byte1` — sea-monster kill counter AND boss8 wall-rotation
/// speed (distinct levels, one cell). = `ebwm::GSVAR_BYTE1` (0x0310), the
/// same cell the map bytecode pokes via `setvarb` (bossG wave gating).
#[inline]
fn gsvar_byte1(g: &Game) -> u8 {
    wm8(g, ebwm::GSVAR_BYTE1)
}
#[inline]
fn set_gsvar_byte1(g: &mut Game, v: u8) {
    wm8_set(g, ebwm::GSVAR_BYTE1, v);
}
/// C `g_maptrigger` = `ebwm::MAPTRIGGER` (0x0311).
#[inline]
fn maptrigger(g: &Game) -> u8 {
    wm8(g, ebwm::MAPTRIGGER)
}
#[inline]
fn set_maptrigger(g: &mut Game, v: u8) {
    wm8_set(g, ebwm::MAPTRIGGER, v);
}

/// C `g_pviewposx` — the player-lane camera X (`crate::common::sv`
/// 0x053C; the same cell player.rs writes each frame).
const WM_PVIEWPOSX: u16 = 0x053C;
/// C `g_pviewposy` — 0x053E.
const WM_PVIEWPOSY: u16 = 0x053E;
/// C `g_viewposy` — the camera Y (ALCS.INC:265), distinct from pviewposy
/// (GILESALC.INC:178) — sv block 0x0552.
const WM_VIEWPOSY: u16 = 0x0552;
/// C `g_bg2Yscroll` — 0x055A (player-lane sv block).
const WM_BG2YSCROLL: u16 = 0x055A;

#[inline]
fn pviewposx(g: &Game) -> i16 {
    wm16s(g, WM_PVIEWPOSX)
}
#[inline]
fn pviewposy(g: &Game) -> i16 {
    wm16s(g, WM_PVIEWPOSY)
}
#[inline]
fn viewposy(g: &Game) -> i16 {
    wm16s(g, WM_VIEWPOSY)
}

// ============================================================
// BOSS2_BEGIN (strat_boss2.c) — "SPINNING TOP" (Macbeth / Venom1).
// ============================================================

// boss2_scale equ 3; world units are (v << 3).
const BOSS2_SCALE: i16 = 3;
#[inline]
const fn b2u(v: i16) -> i16 {
    v << BOSS2_SCALE
}

// STRATEQU.INC HP/AP constants.
const BOSS2_AP: u8 = 10; // s_set_aldata x,#hardHP,#10
const BOSS2TOP_HP: u8 = 64; // boss2topHP
const BOSS2TOP_AP: u8 = 1; // boss2topAP
const BOSS2TURRET_HP: u8 = 16; // boss2turretHP
const BOSS2TURRET_AP: u8 = 16; // boss2turretAP
const BOSS2PETAL_AP: u8 = 1; // boss2petalAP
const BOSS2PLASMA_AP: u8 = 8; // boss2plasma AP

// Weapon parameters (GSTRATS.ASM fire_* routines).
const RELFASTELASER_SPEED: u8 = 90;
const RELFASTELASER_LIFE: u8 = 40;
const ENEMYLASER_AP: u8 = 2;
const BOSSHMISSILE2_SPEED: u8 = 60;
const BOSSHMISSILE2_LIFE: u8 = 100;
const BOSSHMISSILE2_AP: u8 = 8;
const BOSS2_HPLASMA_SPEED: u8 = 60;
const BOSS2_HPLASMA_LIFE: u8 = 50;
const BOSS2_HPLASMA_AP: u8 = 10;

// Child object numbers (s_make_childobj in boss2_Istrat).
const BOSS2_CHILD_TOP: u8 = 1;
const BOSS2_CHILD_PETAL0: u8 = 2;
const BOSS2_CHILD_PETAL1: u8 = 3;
const BOSS2_CHILD_PETAL2: u8 = 4;
const BOSS2_CHILD_PETAL3: u8 = 5;
const BOSS2_CHILD_TURRET0: u8 = 7;
const BOSS2_CHILD_TURRET1: u8 = 9;
const BOSS2_CHILD_TURRET2: u8 = 11;
const BOSS2_CHILD_TURRET3: u8 = 13;

// Strategy flags sflag1..sflag4 (mapped onto sflags2, cf. strat_boss2.c).
const BOSS2_SFLAG1: u8 = ASF2_SFLAG1; // 0x10
const BOSS2_SFLAG2: u8 = 0x20;
const BOSS2_SFLAG3: u8 = 0x40;
const BOSS2_SFLAG4: u8 = 0x80;

// Shape proxies (strat_boss2.c:112-119).
const SH_BOSS_2_0_PROXY: u16 = 259;
const SH_BOSS_2_1_PROXY: u16 = 260;
const SH_BOSS_2_3_PROXY: u16 = 261;
const SH_BOSS_2_4_PROXY: u16 = 262;
const SH_BOSS_2_5_PROXY: u16 = 263;
const SH_ROCKBEAM_PROXY: u16 = 272;
const SH_L2SMOKE_PROXY: u16 = 273;

// boss2petal_tab (GBSTRATS.ASM): dw boss_2_5, boss_2_4, boss_2_3
const BOSS2PETAL_TAB: [u16; 3] = [SH_BOSS_2_5_PROXY, SH_BOSS_2_4_PROXY, SH_BOSS_2_3_PROXY];

// Explosion size markers.
const B2_EXPSHAPE_MEDIUM: u16 = 2;
const B2_EXPSHAPE_LARGE: u16 = 3;
const B2_EXPSHAPE_FOLARGE: u16 = 4;

// Sound ids (trigse args).
const B2_SE_SPAWN: u8 = 0x95;
const B2_SE_SPIN: u8 = 0x71;
const B2_SE_JUMP: u8 = 0x85;
const B2_SE_LAND: u8 = 0x8E;

// ---- Small local helpers (strat_boss2.c:171-321) ----

/// sr_addplayerZ (STRATROU.ASM:3092): scroll with the world.
#[inline]
fn b2_add_player_z(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

/// sr_keeprelto_player (STRATROU.ASM:2928).
#[inline]
fn b2_keeprelto_player(g: &mut Game, idx: u16) {
    boss_keeprel_to_player(g, idx);
}

/// [-range, +range] random (strat_boss2.c:202).
fn b2_random_signed(g: &mut Game, range: i16) -> i16 {
    if range <= 0 {
        return 0;
    }
    let span = ((range * 2) + 1) as u16;
    (sfrtl_random(g) % span) as i16 - range
}

/// addyrot2z_srou (GBSTRATS.ASM:59-77) — strat_boss2.c:272.
fn boss2_addyrot2z(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let mut a = al.roty as i16;
    if a < 128 {
        a = -1 - a; // eor #255 then sign-extend
    } else {
        a -= 256; // already negative as signed byte
    }
    a = (a + 64) >> 4; // arithmetic shift
    al.worldz = al.worldz.wrapping_add(a);
}

/// s_falldown_Yvec (STRATMAC.INC:1813) — strat_boss2.c:288. Returns true
/// when the bounce has fully decayed.
fn boss2_falldown_yvec(
    g: &mut Game,
    idx: u16,
    bounce_shift: u32,
    gravity: i16,
    ground: i16,
) -> bool {
    let al = &mut g.objs.aliens[idx as usize];
    al.vy = al.vy.wrapping_add(gravity);
    if al.worldy < ground {
        return false; // s_jmp_higher — still airborne
    }
    al.worldy = ground;
    let mut v = al.vy.wrapping_neg();
    v >>= bounce_shift;
    if (-5..=0).contains(&v) {
        v = 0;
    }
    al.vy = v;
    v == 0
}

/// Place `self` at base + local offset rotated around Y by `roty`
/// (s_add_roffs2pos flags 0,1,0). Does NOT touch rotations (strat_boss2.c:259).
fn b2_yaw_offset_pos(
    g: &mut Game,
    self_idx: u16,
    base: &Alien,
    roty: u8,
    offx: i16,
    offy: i16,
    offz: i16,
) {
    let s = strat_sin(roty);
    let c = strat_cos(roty);
    let rx = ((offx as f32 * c) + (offz as f32 * s)).round() as i16;
    let rz = ((offz as f32 * c) - (offx as f32 * s)).round() as i16;
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(offy);
    al.worldz = base.worldz.wrapping_add(rz);
}

/// Place `self` at base + local offset rotated by the base object's FULL
/// rotation (s_add_Roffs2pos flags 1,1,1, STRATMAC.INC:4098). ROM order:
/// rotate_8yx by rotz (STRATROU.ASM:1128), then rotate_8yz by rotx
/// (STRATROU.ASM:1057), then rotate_8xz by roty (STRATROU.ASM:986, which
/// negates its angle — the roty stage below is byte-identical in convention
/// to the verified `b2_yaw_offset_pos`).
fn b2_full_offset_pos(
    g: &mut Game,
    self_idx: u16,
    base: &Alien,
    offx: i16,
    offy: i16,
    offz: i16,
) {
    // rotz stage (rotate_8yx): x' = x*cos - y*sin, y' = x*sin + y*cos.
    let s = strat_sin(base.rotz);
    let c = strat_cos(base.rotz);
    let x1 = ((offx as f32 * c) - (offy as f32 * s)).round();
    let y1 = ((offx as f32 * s) + (offy as f32 * c)).round();
    // rotx stage (rotate_8yz): y' = y*cos - z*sin, z' = y*sin + z*cos.
    let s = strat_sin(base.rotx);
    let c = strat_cos(base.rotx);
    let y2 = ((y1 * c) - (offz as f32 * s)).round();
    let z2 = ((y1 * s) + (offz as f32 * c)).round();
    // roty stage (rotate_8xz, angle negated in ROM): x' = x*cos + z*sin,
    // z' = z*cos - x*sin — same formula as b2_yaw_offset_pos.
    let s = strat_sin(base.roty);
    let c = strat_cos(base.roty);
    let rx = ((x1 * c) + (z2 * s)).round() as i16;
    let rz = ((z2 * c) - (x1 * s)).round() as i16;
    let ry = y2 as i16;
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(ry);
    al.worldz = base.worldz.wrapping_add(rz);
}

/// s_make_childobj shape,child_num,Istrat,enemy1 (strat_boss2.c:514).
fn boss2_spawn_child(g: &mut Game, mother: u16, shape: u16, child_num: u8, init_fn: StrategyFn) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    g.objs.aliens[child as usize].shape = shape;
    copy_pos(g, child, mother);
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    g.objs.aliens[child as usize].collflags |= COLLTYPE_ENEMY1;
    init_fn(g, child);
    Some(child)
}

// ---- Explosion factories (local copies, strat_boss2.c:569-615) ----

/// makeexpobj_srou (delayexplode child) — strat_boss2.c:569.
fn b2_make_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_obj(g, 0)?;
    let s_tick = sid(g, boss2_delayexplode_strat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags |= ASF_COLLDISABLE;
        al.sflags2 |= ASF2_NOEXPSND | ASF2_RELEXPLODE;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.stratptr = Some(s_tick);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
    }
    copy_pos(g, child, parent);
    Some(child)
}
fn b2_make_large_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let c = b2_make_exp_obj(g, parent)?;
    g.objs.aliens[c as usize].shape = B2_EXPSHAPE_LARGE;
    Some(c)
}
fn b2_make_medium_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let c = b2_make_exp_obj(g, parent)?;
    g.objs.aliens[c as usize].shape = B2_EXPSHAPE_MEDIUM;
    Some(c)
}
fn b2_make_fol_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let c = b2_make_exp_obj(g, parent)?;
    g.objs.aliens[c as usize].shape = B2_EXPSHAPE_FOLARGE;
    Some(c)
}

/// delayexplode_strat (EXPSTRAT.ASM:259-268) — strat_boss2.c:547.
fn boss2_delayexplode_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_HITFLASH;
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.aldead = 1;
        if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
            g.call_strat(exp, idx);
        }
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        b2_add_player_z(g, idx);
    }
}

/// delayremove_strat (GSTRATS.ASM:1188-1193), rel variant — strat_boss2.c:618.
fn boss2_delayremove_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        let bf = bossflags(g);
        set_bossflags(g, bf & !BF_DYING);
        set_bossmaxhp(g, 0);
        g.objs.aldead = 1;
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        b2_add_player_z(g, idx);
    }
}

/// makesmoke_srou_l (GSTRATS.ASM:1265-1275) — strat_boss2.c:641.
fn boss2_make_smoke(g: &mut Game, parent: u16) -> Option<u16> {
    let smoke = make_obj(g, SH_L2SMOKE_PROXY)?;
    let s = sid(g, boss2_smoke_strat);
    {
        let al = &mut g.objs.aliens[smoke as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags |= ASF_COLLDISABLE;
        al.type_ |= ATZREMOVE;
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.count = 8; // smokeP colanim runs 8 steps
    }
    copy_pos(g, smoke, parent);
    Some(smoke)
}

/// smokeP_strat — strat_boss2.c:658.
fn boss2_smoke_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_sub(1);
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// particlefiredown placeholder — strat_boss2.c:675.
fn boss2_particle_strat(_g: &mut Game, _idx: u16) {}

// ---- Weapons (strat_boss2.c:683-883) ----

fn b2_target_obj(g: &Game, self_idx: u16) -> Option<u16> {
    let fp = g.objs.aliens[self_idx as usize].fireobjptr;
    if fp == 0 {
        return None;
    }
    let t = boss_child_from_index_raw(fp)?;
    if g.objs.aliens[t as usize].active {
        Some(t)
    } else {
        None
    }
}

/// Common enemy projectile spawn (strat_boss2.c:696).
#[allow(clippy::too_many_arguments)]
fn b2_spawn_shot(
    g: &mut Game,
    self_idx: u16,
    offx: i16,
    offy: i16,
    offz: i16,
    pitch: u8,
    yaw: u8,
    speed: u8,
    life: u8,
    ap: u8,
) -> Option<u16> {
    let shot = spawn_projectile(g, Some(self_idx), 0, 0, 0, pitch, yaw, speed, life, ap, ACF_COLLTYPE4)?;
    let me = g.objs.aliens[self_idx as usize];
    // fire_weapon muzzle placement rotates by the FIRER's rotx, roty AND rotz
    // (s_add_Roffs2pos.w B,y,x,x,...,1,1,1 — GSTRATS.ASM:2795). boss2 state 2
    // sets rotz=deg180 and never clears it, so the state-4 / flipped-top
    // muzzles fire from the ground-facing tip in ROM.
    b2_full_offset_pos(g, shot, &me, offx, offy, offz);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = pitch;
        al.roty = yaw;
        al.rotz = me.rotz;
    }
    strat_gen_vecs_3d(&mut g.objs.aliens[shot as usize]);
    Some(shot)
}

/// fire_relfastElaser (GSTRATS.ASM:2578) — strat_boss2.c:714.
///
/// Spread comes from s_weapon_rndrots2obj (STRATMAC.INC:2114 ->
/// s_weapon_rndrot STRATMAC.INC:2099): per axis `(random_l() & mask) -
/// mask/2`, X (pitch) drawn FIRST, then Y (yaw). Mask 7 -> [-3,+4].
fn boss2_fire_relfastelaser(g: &mut Game, self_idx: u16, offy: i16, target: Option<u16>, rnd_pitch_mask: u8, rnd_yaw_mask: u8) {
    let dp = ((sfrtl_random(g) as u8 & rnd_pitch_mask) as i16 - (rnd_pitch_mask / 2) as i16) as u8;
    let dy = ((sfrtl_random(g) as u8 & rnd_yaw_mask) as i16 - (rnd_yaw_mask / 2) as i16) as u8;
    let me = g.objs.aliens[self_idx as usize];
    let mut pitch = me.rotx;
    let mut yaw = me.roty;
    if let Some(t) = target.filter(|&t| g.objs.aliens[t as usize].active) {
        let tt = g.objs.aliens[t as usize];
        yaw = strat_angle_xz(&me, &tt).wrapping_add(dy);
        pitch = strat_pitch_toward(&me, &tt).wrapping_add(dp);
    }
    let _ = b2_spawn_shot(g, self_idx, 0, offy, 0, pitch, yaw, RELFASTELASER_SPEED, RELFASTELASER_LIFE, ENEMYLASER_AP);
}

/// relelaserhome tick (strat_boss2.c:730).
fn boss2_homelaser_strat(g: &mut Game, idx: u16) {
    if let Some(t) = b2_target_obj(g, idx) {
        let me = g.objs.aliens[idx as usize];
        let tt = g.objs.aliens[t as usize];
        let want_yaw = strat_angle_xz(&me, &tt);
        let want_pitch = strat_pitch_toward(&me, &tt);
        let dyaw = me.roty.wrapping_sub(want_yaw) as i8;
        let dpitch = me.rotx.wrapping_sub(want_pitch) as i8;
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = me.roty.wrapping_sub((dyaw >> 3) as u8);
        al.rotx = me.rotx.wrapping_sub((dpitch >> 3) as u8);
        strat_gen_vecs_3d(al);
    }
    b2_add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// fire_relslowElaserHome (GSTRATS.ASM:2563) — strat_boss2.c:760.
fn boss2_fire_relslowelaserhome(g: &mut Game, self_idx: u16, offy: i16, target: Option<u16>) {
    let speed = if currentlevel(g) == 1 { 48 } else { 60 };
    let me = g.objs.aliens[self_idx as usize];
    let mut pitch = me.rotx;
    let mut yaw = me.roty;
    if let Some(t) = target.filter(|&t| g.objs.aliens[t as usize].active) {
        let tt = g.objs.aliens[t as usize];
        yaw = strat_angle_xz(&me, &tt);
        pitch = strat_pitch_toward(&me, &tt);
    }
    if let Some(shot) = b2_spawn_shot(g, self_idx, 0, offy, 0, pitch, yaw, speed, RELFASTELASER_LIFE, ENEMYLASER_AP) {
        if let Some(t) = target {
            let s = sid(g, boss2_homelaser_strat);
            let al = &mut g.objs.aliens[shot as usize];
            al.stratptr = Some(s);
            al.fireobjptr = boss_obj_index_or_null(t);
        }
    }
}

/// hmissile2_strat (GSTRATS.ASM:1500) — strat_boss2.c:781.
fn boss2_hmissile2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10); // missile spin
    }
    if g.objs.aliens[idx as usize].count1 > 0 {
        g.objs.aliens[idx as usize].count1 -= 1;
    } else {
        if let Some(t) = b2_target_obj(g, idx) {
            let me = g.objs.aliens[idx as usize];
            let tt = g.objs.aliens[t as usize];
            let want_yaw = strat_angle_xz(&me, &tt);
            let want_pitch = strat_pitch_toward(&me, &tt);
            let dyaw = me.roty.wrapping_sub(want_yaw) as i8;
            let dpitch = me.rotx.wrapping_sub(want_pitch) as i8;
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = me.roty.wrapping_sub((dyaw >> 1) as u8);
            al.rotx = me.rotx.wrapping_sub((dpitch >> 1) as u8);
        }
        strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    b2_add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// fire_bossHmissile2 (GSTRATS.ASM:2675) — strat_boss2.c:820.
fn boss2_fire_bosshmissile2(g: &mut Game, self_idx: u16, offy: i16, yaw: u8, target: Option<u16>) {
    let Some(shot) = spawn_projectile(g, Some(self_idx), 0, offy, 0, 0, yaw, BOSSHMISSILE2_SPEED, BOSSHMISSILE2_LIFE, BOSSHMISSILE2_AP, ACF_COLLTYPE4) else {
        return;
    };
    let s = sid(g, boss2_hmissile2_strat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.stratptr = Some(s);
        al.type_ = ATMISSILE | ATZREMOVE;
        al.sflags &= !ASF_INVISIBLE;
        al.sflags |= ASF_SHADOW;
        al.hp = 2; // hmissile1HP
        al.count1 = 16; // straight-flight frames
        al.fireobjptr = target.map(boss_obj_index_or_null).unwrap_or(0);
    }
    strat_gen_vecs_3d(&mut g.objs.aliens[shot as usize]);
}

/// homingflat tick used by fire_Hplasma (strat_boss2.c:841).
fn boss2_hplasma_strat(g: &mut Game, idx: u16) {
    if let Some(t) = b2_target_obj(g, idx) {
        let me = g.objs.aliens[idx as usize];
        let tt = g.objs.aliens[t as usize];
        if (me.worldz as i32 - tt.worldz as i32).abs() >= 500 {
            let want_yaw = strat_angle_xz(&me, &tt);
            let want_pitch = strat_pitch_toward(&me, &tt);
            let dyaw = me.roty.wrapping_sub(want_yaw) as i8;
            let dpitch = me.rotx.wrapping_sub(want_pitch) as i8;
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = me.roty.wrapping_sub((dyaw >> 4) as u8);
            al.rotx = me.rotx.wrapping_sub((dpitch >> 4) as u8);
            strat_gen_vecs_3d(al);
        }
    }
    b2_add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// fire_Hplasma (GSTRATS.ASM:2517) — strat_boss2.c:871.
fn boss2_fire_hplasma(g: &mut Game, self_idx: u16, offy: i16, offz: i16, target: Option<u16>) {
    let me = g.objs.aliens[self_idx as usize];
    let Some(shot) = b2_spawn_shot(g, self_idx, 0, offy, offz, me.rotx, me.roty, BOSS2_HPLASMA_SPEED, BOSS2_HPLASMA_LIFE, BOSS2_HPLASMA_AP) else {
        return;
    };
    let s = sid(g, boss2_hplasma_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.stratptr = Some(s);
    al.fireobjptr = target.map(boss_obj_index_or_null).unwrap_or(0);
}

// ---- boss2exp (final detonation, strat_boss2.c:891) ----

fn boss2exp_init(g: &mut Game, idx: u16) {
    // s_set_vecs x,#0,#15<<boss2_scale,#0
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = 0;
        al.vy = b2u(15);
        al.vz = 0;
    }
    let offx = g.objs.aliens[idx as usize].vx;
    let offy = g.objs.aliens[idx as usize].vy;
    let offz = g.objs.aliens[idx as usize].vz;

    g.vars.gameflags |= GF_BOSSDEAD;
    play_se(g, 0x1D);

    for _ in 0..10 {
        if let Some(exp) = b2_make_large_exp_obj(g, idx) {
            g.objs.aliens[exp as usize].sflags4 |= ASF4_NOPOLYEXP;
            g.objs.aliens[exp as usize].count = ((sfrtl_random(g) % 15) + 1) as u8;
            addrnd2pos_xy(g, exp);
            let rx = b2_random_signed(g, 64);
            let ry = b2_random_signed(g, 64);
            let al = &mut g.objs.aliens[exp as usize];
            al.worldx = al.worldx.wrapping_add(offx).wrapping_add(rx);
            al.worldy = al.worldy.wrapping_add(offy).wrapping_add(ry);
            al.worldz = al.worldz.wrapping_add(offz);
        }
        if let Some(exp) = b2_make_fol_exp_obj(g, idx) {
            g.objs.aliens[exp as usize].sflags4 |= ASF4_NOPOLYEXP;
            g.objs.aliens[exp as usize].count = ((sfrtl_random(g) % 15) + 8) as u8;
            addrnd2pos_xy(g, exp);
            let rx = b2_random_signed(g, 64);
            let ry = b2_random_signed(g, 64);
            let al = &mut g.objs.aliens[exp as usize];
            al.worldx = al.worldx.wrapping_add(offx).wrapping_add(rx);
            al.worldy = al.worldy.wrapping_add(offy).wrapping_add(ry);
            al.worldz = al.worldz.wrapping_add(offz);
        }
    }

    let s = sid(g, boss2exp_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
    al.sflags2 |= ASF2_RELEXPLODE;
    al.flags |= AFEXP;
    al.count = 11;
}

fn boss2exp_strat(g: &mut Game, idx: u16) {
    boss2_delayremove_strat(g, idx);
}

// ---- boss2top (strat_boss2.c:958) ----

fn boss2top_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss2top_strat);
    let s_coll = sid(g, boss2topcol_coll);
    let s_exp = sid(g, boss2topexp);
    let bmh = bossmaxhp(g).wrapping_add(BOSS2TOP_HP as u16);
    set_bossmaxhp(g, bmh);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = BOSS2TOP_HP;
    al.ap = BOSS2TOP_AP;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.sflags |= ASF_COLLDISABLE;
}

fn boss2top_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
        al.rotx = m.rotx;
        al.roty = m.roty;
        al.rotz = m.rotz;
        if m.sflags2 & BOSS2_SFLAG3 != 0 {
            al.sflags &= !ASF_COLLDISABLE;
        }
    }

    // top critical below 16 HP.
    if g.objs.aliens[idx as usize].hp <= 16 {
        g.objs.aliens[mother as usize].sflags2 |= BOSS2_SFLAG2;
        if g.vars.gameframe & 7 == 0 {
            if let Some(p) = player_idx(g) {
                boss2_fire_relslowelaserhome(g, idx, b2u(-30 - 20), Some(p));
            }
        }
    }

    // .npfly missile fire (sflag4 gate, every 32 frames).
    if g.objs.aliens[mother as usize].sflags2 & BOSS2_SFLAG4 != 0 && g.vars.gameframe & 31 == 0 {
        // s_jmp_random (GBSTRATS.ASM:742, macro STRATMAC.INC:1407): keep
        // -deg22 when random_l() < (50*255)/100 = 127; +deg22 when >= 127.
        let mut yaw = DEG22.wrapping_neg();
        if (sfrtl_random(g) & 0xFF) >= 127 {
            yaw = DEG22;
        }
        let p = player_idx(g);
        let saved_roty = g.objs.aliens[idx as usize].roty;
        g.objs.aliens[idx as usize].roty = 0;
        boss2_fire_bosshmissile2(g, idx, b2u(-30), yaw, p);
        g.objs.aliens[idx as usize].roty = saved_roty;
    }
}

/// boss2topcol_Istrat (strat_boss2.c:1041).
fn boss2topcol_coll(g: &mut Game, idx: u16) {
    if let Some(exp) = b2_make_medium_exp_obj(g, idx) {
        let al = &mut g.objs.aliens[exp as usize];
        al.worldy = al.worldy.wrapping_add(b2u(60));
        al.vy = -20;
        al.count = 1;
    }
    strat_hit_flash(g, idx);
}

/// boss2topexp_Istrat (strat_boss2.c:1063).
fn boss2topexp(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(b2u(60));
    }
    strat_explode(g, idx);
}

// ---- boss2turret (strat_boss2.c:1077) ----

fn boss2turret_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss2turret_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, boss2turretexp);
    let bmh = bossmaxhp(g).wrapping_add(BOSS2TURRET_HP as u16);
    set_bossmaxhp(g, bmh);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = BOSS2TURRET_HP;
    al.ap = BOSS2TURRET_AP;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.sflags |= ASF_SHADOW;
}

fn boss2turret_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
        al.rotx = m.rotx;
        al.roty = m.roty.wrapping_add(al.sbyte2);
        al.rotz = m.rotz;
    }
    boss2_addyrot2z(g, idx);

    if g.objs.aliens[idx as usize].roty == DEG180 {
        let p = player_idx(g);
        boss2_fire_hplasma(g, idx, (-26i16).wrapping_mul(8), (36i16).wrapping_mul(8), p);
    }
}

/// boss2turretexp_Istrat (strat_boss2.c:1130).
fn boss2turretexp(g: &mut Game, idx: u16) {
    if let Some(exp) = b2_make_large_exp_obj(g, idx) {
        let me = g.objs.aliens[idx as usize];
        b2_yaw_offset_pos(g, exp, &me, me.roty, 0, b2u(5 - 30), b2u(35));
        g.objs.aliens[exp as usize].count = 1;
    }
    g.objs.aldead = 1;
}

// ---- boss2petal (strat_boss2.c:1155) ----

fn boss2petal_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss2petal_strat);
    let s_exp = sid(g, boss2petalexp_init);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
    al.ap = BOSS2PETAL_AP;
    al.stratstate = 0;
    al.sbyte3 = 0;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.sflags |= ASF_SHADOW;
}

fn boss2petal_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let m = g.objs.aliens[mother as usize];

    // top destroyed: petals die. s_kill_obj (GBSTRATS.ASM:831, macro
    // STRATMAC.INC:2643) sets colldisable AND hp=0.
    if m.sflags2 & BOSS2_SFLAG2 != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.sflags2 |= BOSS2_SFLAG2;
        al.hp = 0;
        return;
    }

    let target_open = m.sbyte3;
    if m.sflags2 & BOSS2_SFLAG1 != 0 {
        // open
        if g.vars.gameframe & 3 == 0 && g.objs.aliens[idx as usize].sbyte3 != target_open {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = al.sbyte3.wrapping_add(2);
            if (al.sbyte3 >> 1) < 3 {
                al.shape = BOSS2PETAL_TAB[(al.sbyte3 >> 1) as usize];
            }
        }
    } else {
        // close
        if g.vars.gameframe & 3 == 0 && g.objs.aliens[idx as usize].sbyte3 != 0 {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = al.sbyte3.wrapping_sub(2);
            if (al.sbyte3 >> 1) < 3 {
                al.shape = BOSS2PETAL_TAB[(al.sbyte3 >> 1) as usize];
            }
        }
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
        al.rotx = m.rotx;
        al.roty = m.roty.wrapping_add(al.sbyte2);
        al.rotz = m.rotz;
    }
    boss2_addyrot2z(g, idx);

    // launch one orbiting plasma per petal when the strafe phase starts.
    if m.sflags2 & BOSS2_SFLAG3 != 0 && g.objs.aliens[idx as usize].sflags2 & BOSS2_SFLAG1 == 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags |= ASF_COLLDISABLE;
            al.sflags2 |= BOSS2_SFLAG1;
        }
        if let Some(plasma) = make_obj(g, SH_ROCKBEAM_PROXY) {
            boss2plasma_init(g, plasma, idx);
        }
    }
}

/// boss2petalexp_Istrat (strat_boss2.c:1245).
fn boss2petalexp_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss2petalexp_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.expstratptr = Some(s);
    al.count = 50;
    al.sflags |= ASF_COLLDISABLE;
}

/// boss2petalexp_strat (strat_boss2.c:1257).
fn boss2petalexp_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = al.rotx.wrapping_sub(4);
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

// ---- boss2plasma (strat_boss2.c:1277) ----

fn boss2plasma_init(g: &mut Game, plasma: u16, petal: u16) {
    let s = sid(g, boss2plasma_strat);
    let petal_pos = g.objs.aliens[petal as usize];
    let al = &mut g.objs.aliens[plasma as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = BOSS2PLASMA_AP;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
    al.sword1 = boss_obj_index_or_null(petal) as i16;
    al.sbyte1 = petal_pos.roty;
    al.sbyte2 = 0; // orbit distance
    al.roty = DEG180;
    al.sword2 = petal_pos.worldy;
    al.type_ &= !ATZREMOVE;
    al.worldx = petal_pos.worldx;
    al.worldy = petal_pos.worldy;
    al.worldz = petal_pos.worldz;
}

fn boss2plasma_strat(g: &mut Game, idx: u16) {
    let petal = g.objs.aliens[idx as usize].sword1 as u16;
    let petal = match boss_child_from_index_raw(petal) {
        Some(p) if g.objs.aliens[p as usize].active && g.objs.aliens[p as usize].sflags2 & BOSS2_SFLAG2 == 0 => p,
        _ => {
            g.objs.aldead = 1;
            return;
        }
    };

    let dist = g.objs.aliens[idx as usize].sbyte2;
    let sbyte1 = g.objs.aliens[idx as usize].sbyte1;
    let pp = g.objs.aliens[petal as usize];
    b2_yaw_offset_pos(g, idx, &pp, sbyte1, 0, 0, (dist as i16) << 4);

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180;
    }

    // rotation speed falls off as the spiral widens.
    let mut spin: u8 = 1;
    if dist <= 30 {
        spin = spin.wrapping_add(4);
    }
    if dist <= 60 {
        spin = spin.wrapping_add(3);
    }
    if dist <= 90 {
        spin = spin.wrapping_add(2);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = al.sbyte1.wrapping_add(spin);
        al.sbyte2 = al.sbyte2.wrapping_add(1);
        if al.sbyte2 >= 120 {
            al.sbyte2 = 0;
        }
        al.worldy = al.sword2;
    }
    let hover = chase_proportional(g.objs.aliens[idx as usize].sword2, g.vars.player_posy, 2);
    g.objs.aliens[idx as usize].sword2 = hover;
}

// ---- boss2_strat mother state machine (strat_boss2.c:1371) ----

fn boss2_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    let pl_idx = player_idx(g);
    let nchildren = boss_count_children(g, idx);

    // -------- state 0 --------
    if g.objs.aliens[idx as usize].stratstate == 0 {
        if nchildren <= 5 + 2 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags2 |= BOSS2_SFLAG4;
                al.roty = al.roty.wrapping_add(2);
            }
            if g.objs.aliens[idx as usize].sflags2 & BOSS2_SFLAG1 == 0 {
                play_se(g, B2_SE_SPIN);
            }
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags2 |= BOSS2_SFLAG1;
            al.sbyte3 = 2; // petals: half open
            if nchildren == 5 + 1 {
                al.roty = al.roty.wrapping_add(2);
            }
        }

        if nchildren == 5 {
            g.objs.aliens[idx as usize].stratstate += 1;
        } else {
            // s_jmp_Zdistmore #1100 (GBSTRATS.ASM:550) branches to smoke on
            // |dz| >= 1100 (rlbpl includes equal), so "near" is strictly <.
            let me_z = g.objs.aliens[idx as usize].worldz;
            let near = pl
                .map(|p| p.active && (me_z as i32 - p.worldz as i32).abs() < 1100)
                .unwrap_or(false);
            if near {
                b2_keeprelto_player(g, idx);
                b2_add_player_z(g, idx);
            } else {
                if let Some(smoke) = boss2_make_smoke(g, idx) {
                    addrnd2pos_xy(g, smoke);
                    let al = &mut g.objs.aliens[smoke as usize];
                    al.worldz = al.worldz.wrapping_sub(100);
                    al.worldy = al.worldy.wrapping_add(b2u(-35));
                }
                if let Some(smoke) = boss2_make_smoke(g, idx) {
                    addrnd2pos_xy(g, smoke);
                    let al = &mut g.objs.aliens[smoke as usize];
                    al.worldz = al.worldz.wrapping_sub(100);
                    al.worldy = al.worldy.wrapping_add(b2u(-20));
                }
            }
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = al.roty.wrapping_add(2);
            return;
        }
    }

    // -------- state 1: leap --------
    if g.objs.aliens[idx as usize].stratstate == 1 {
        play_se(g, B2_SE_JUMP);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags2 &= !BOSS2_SFLAG4;
            al.vx = 0;
            al.vy = -80;
            al.vz = 0;
            al.sword2 = 0; // ground
            al.stratstate += 1;
            al.ptr = 0;
        }
        if let Some(particle) = make_obj(g, 0) {
            let s = sid(g, boss2_particle_strat);
            {
                let al = &mut g.objs.aliens[particle as usize];
                al.stratptr = Some(s);
                al.sflags |= ASF_COLLDISABLE;
                al.sflags3 &= !ASF3_REALOBJ;
            }
            copy_pos(g, particle, idx);
            g.objs.aliens[idx as usize].ptr = boss_obj_index_or_null(particle);
        }
    }

    // -------- state 2: flip + slam --------
    if g.objs.aliens[idx as usize].stratstate == 2 {
        let particle = boss_child_from_index_raw(g.objs.aliens[idx as usize].ptr)
            .filter(|&p| g.objs.aliens[p as usize].active);
        if let Some(p) = particle {
            copy_pos(g, p, idx);
        }

        if g.objs.aliens[idx as usize].worldy < -1000 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = DEG180;
                al.sword2 = b2u(-60);
            }
            let chase_x = chase_proportional(g.objs.aliens[idx as usize].worldx, g.vars.player_posx, 4);
            g.objs.aliens[idx as usize].worldx = chase_x;
            let target_z = g.vars.player_posz.wrapping_add(200);
            let chase_z = chase_proportional(g.objs.aliens[idx as usize].worldz, target_z, 5);
            g.objs.aliens[idx as usize].worldz = chase_z;
        }

        b2_add_player_z(g, idx);
        strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

        let ground = g.objs.aliens[idx as usize].sword2;
        if boss2_falldown_yvec(g, idx, 2, 2, ground) {
            if let Some(p) = particle {
                g.objs.free(p);
            }
            let al = &mut g.objs.aliens[idx as usize];
            al.ptr = 0;
            al.stratstate += 1;
        } else if g.objs.aliens[idx as usize].worldy >= ground {
            play_se(g, B2_SE_LAND);
        }
    }

    // -------- state 3: back away --------
    if g.objs.aliens[idx as usize].stratstate == 3 {
        b2_add_player_z(g, idx);
        let chase_x = chase_proportional(g.objs.aliens[idx as usize].worldx, 0, 3);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = chase_x;
            al.worldz = al.worldz.wrapping_add(30);
            al.sbyte4 = 25;
        }
        // s_jmp_Zdistmore #1100,nextstate (GBSTRATS.ASM:627): advance at >= 1100.
        let far = pl
            .map(|p| p.active && (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() >= 1100)
            .unwrap_or(false);
        if far {
            g.objs.aliens[idx as usize].stratstate += 1;
        }
    }

    // -------- state 4: strafe circle --------
    if g.objs.aliens[idx as usize].stratstate == 4 {
        b2_add_player_z(g, idx);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte4 = al.sbyte4.wrapping_sub(1);
            if al.sbyte4 == 0 {
                al.sbyte4 = 100;
            }
        }
        if g.objs.aliens[idx as usize].sbyte4 <= 25 {
            if g.vars.gameframe & 1 == 0 {
                if let Some(p) = pl_idx {
                    boss2_fire_relfastelaser(g, idx, b2u(-60), Some(p), 7, 7);
                }
            }
        } else {
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            // s_jmp_Zdistmore #500,.nminz (GBSTRATS.ASM:649): skip the z-hold
            // at >= 500, so hold only while strictly < 500.
            let hold = pl
                .map(|p| p.active && (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() < 500)
                .unwrap_or(false);
            if hold {
                let vz = g.objs.aliens[idx as usize].vz;
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(vz);
            }
        }

        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = al.roty.wrapping_add(2);
            al.sflags2 |= BOSS2_SFLAG1;
            al.sbyte3 = 4;
            al.roty = al.roty.wrapping_add(2);
        }
        if g.objs.aliens[idx as usize].sflags2 & BOSS2_SFLAG3 == 0 {
            play_se(g, B2_SE_SPIN);
        }
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags2 |= BOSS2_SFLAG3;
            al.sflags &= !ASF_COLLDISABLE;
        }
        // circle-strafe velocity from sin/cos (amplitude 127).
        // s_set_alvar2alvartab ...,sintab,-3 / costab,-1 (GBSTRATS.ASM:665-666)
        // sign-extends the table byte then applies adiv2 x3 / x1, which
        // truncates TOWARD ZERO — Rust `/` matches; `>>` would floor negatives
        // (e.g. -100: ROM -12, `>>3` gives -13).
        let sb2 = g.objs.aliens[idx as usize].sbyte2;
        let vx = ((strat_sin(sb2) * 127.0) as i16) / 8;
        let vz = ((strat_cos(sb2) * 127.0) as i16) / 2;
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vx = vx;
            al.vz = vz;
            al.sbyte2 = al.sbyte2.wrapping_add(4);
        }

        if boss_find_child_obj(g, idx, BOSS2_CHILD_TOP).is_none() {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratstate += 1;
            al.vx = 0;
            al.vy = 10;
            al.vz = 30;
        }
    }

    // -------- state 5: topple + die --------
    if g.objs.aliens[idx as usize].stratstate == 5 {
        if g.vars.pshipflags2 & PSF2_PLAYERHP0 == 0 {
            boss_dying(g);
            if boss2_falldown_yvec(g, idx, 1, 1, b2u(-30)) {
                boss2exp_init(g, idx);
                return;
            }
            if let Some(exp) = b2_make_large_exp_obj(g, idx) {
                g.objs.aliens[exp as usize].sflags4 |= ASF4_NOPOLYEXP;
                addrnd2pos_xy(g, exp);
                let al = &mut g.objs.aliens[exp as usize];
                al.worldy = al.worldy.wrapping_add(b2u(15));
                al.vy = -20;
                al.count = 1;
            }
            b2_add_player_z(g, idx);
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            if g.vars.gameframe & 1 == 0 {
                g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
            }
        } else {
            let _ = boss2_falldown_yvec(g, idx, 1, 1, b2u(-30));
            b2_add_player_z(g, idx);
        }
    }

    // .end
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(2);
}

/// boss2_Istrat (strat_boss2.c:1642).
pub fn strat_boss2_init(g: &mut Game, idx: u16) {
    g.vars.gameflags &= !GF_BOSSDEAD;
    let bf = bossflags(g);
    set_bossflags(g, bf & !BF_DYING);

    set_bossmaxhp(g, 0);
    g.vars.meters = 1;

    let _ = boss2_spawn_child(g, idx, SH_BOSS_2_1_PROXY, BOSS2_CHILD_TOP, boss2top_init);

    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL0, boss2petal_init) {
        g.objs.aliens[c as usize].sbyte2 = 0;
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL1, boss2petal_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG90;
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL2, boss2petal_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG180;
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_5_PROXY, BOSS2_CHILD_PETAL3, boss2petal_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG180.wrapping_add(DEG90);
    }

    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET0, boss2turret_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG45;
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET1, boss2turret_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG45.wrapping_add(DEG90);
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET2, boss2turret_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG180.wrapping_add(DEG45);
    }
    if let Some(c) = boss2_spawn_child(g, idx, SH_BOSS_2_0_PROXY, BOSS2_CHILD_TURRET3, boss2turret_init) {
        g.objs.aliens[c as usize].sbyte2 = DEG180.wrapping_add(DEG90).wrapping_add(DEG45);
    }

    let s = sid(g, boss2_strat);
    let s_exp = sid(g, boss2exp_init);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
    al.ap = BOSS2_AP;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
    al.count = 50;
    al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
    al.stratstate = 0;

    play_se(g, B2_SE_SPAWN);
}
// BOSS2_END

// ============================================================
// BOSSSEA_BEGIN (strat_boss_sea.c) — Seamon / BossG / fish.
// ============================================================

const SH_SEA_0_0: u16 = 32;
const SH_SEA_0_PROXY: u16 = 257;
const SH_SEA_0_1_PROXY: u16 = 258;
const SH_BOSS_G_0: u16 = 121;
const SH_BOSS_G_S: u16 = 270;
const SH_F_FISH_PROXY: u16 = 271;
const SH_NULLSHAPE_WORD: u16 = 1;

const BOSSG_HP: u8 = 120;
const BOSSG_AP: u8 = 8;
const BOSSSEAMON_HP: u8 = 2;
const BOSSSEAMON_AP: u8 = 4;
const SEAMON_HP: u8 = 4;
const SEAMON_AP: u8 = 8;
const FISH_HP: u8 = 4;
const FISH_AP: u8 = 8;

const SEA_BRIDGE_MINX2: i16 = -400;
const SEA_BRIDGE_MAXX2: i16 = 400;

const SEA_ELASER_LIFE: u8 = 40;
const SEA_ELASER_AP: u8 = 2;

// al_sflags2 free bits.
const SEA_SFLAG1: u8 = 0x20; // seamon: splashed-down latch
const SEA_SFLAG2: u8 = 0x40; // seamon swim toggle / fish landed latch
const SEA_SFLAG3: u8 = 0x80; // fish: launch to +X side
const SEA_SFLAG8: u8 = ASF4_SFLAG8; // bossg sflag8 (al_sflags4)

const SND_UPSEA: u8 = 0x69;
const SND_DOWNSEA: u8 = 0x75;

// bossg mode-table indices (D2STRATS.ASM:66-109).
const BOSSG_MODE_WAITHITPLAYER: u16 = 0;
const BOSSG_MODE_SCROLLMSG: u16 = 1;
const BOSSG_MODE_SF9E_A: u16 = 2;
const BOSSG_MODE_LOOPBACK_PT: u16 = 3;
const BOSSG_MODE_DISAPPEAR: u16 = 4;
const BOSSG_MODE_WAITSOMETIME: u16 = 5;
const BOSSG_MODE_APPEAR: u16 = 6;
const BOSSG_MODE_MOVETO600H: u16 = 7;
const BOSSG_MODE_GENSHADOWS: u16 = 31;
const BOSSG_MODE_WAITABIT: u16 = 32;
const BOSSG_MODE_SF9E_B: u16 = 33;
const BOSSG_MODE_RUNAWAY_B: u16 = 34;
const BOSSG_MODE_LOOPBACK: u16 = 35;

const SEA_EXPSHAPE_SMALL: u16 = 1;

// ---- Small helpers (strat_boss_sea.c:154-346) ----

#[inline]
fn sea_add_player_z(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

/// s_jmp_notdelay N,label[,offset] (STRATMAC.INC:6456): "not yet" when
/// `(gameframe + offset) & ((1<<N)-1) != 0`. `offset` is the ROM's optional
/// al1pt stagger — callers whose ASM site passes al1pt use the object index
/// as the stagger; sites without it (both bossg uses, D2STRATS.ASM:226/372)
/// pass 0.
fn sea_not_delay(offset: u16, n: u8, gameframe: u16) -> bool {
    gameframe.wrapping_add(offset) & ((1u16 << n) - 1) != 0
}

/// dzdistless_l (DSTRATS.ASM:151-163).
fn sea_dz_less(g: &Game, self_idx: u16, dist: i32) -> bool {
    let Some(p) = player(g) else {
        return false;
    };
    ((g.objs.aliens[self_idx as usize].worldz as i32) - (p.worldz as i32)).abs() < dist
}

/// s_gen_vecs obj,anglevar,al_vel (strat_boss_sea.c:192).
fn sea_gen_vecs_angle(g: &mut Game, idx: u16, angle: u8) {
    let rad = (angle as f32) * (2.0f32 * 3.141_592_65_f32 / 256.0f32);
    let al = &mut g.objs.aliens[idx as usize];
    let speed = (al.vel as i16) as f32;
    al.vx = (speed * rad.sin()) as i16;
    // s_gen_vecs writes al_vx/al_vz ONLY, never vy (STRATMAC.INC:3637) —
    // finding #23. Leave vy untouched.
    al.vz = (speed * rad.cos()) as i16;
}

/// relslowelaser speed (strat_boss_sea.c:225).
fn sea_relslowelaser_speed(g: &Game) -> u8 {
    if currentlevel(g) == 1 {
        48
    } else {
        60
    }
}

/// s_fire_weapon RELSLOWELASER at the player (strat_boss_sea.c:231).
fn sea_fire_relslowelaser(g: &mut Game, self_idx: u16, target: u16) {
    if !g.objs.aliens[target as usize].active {
        return;
    }
    let me = g.objs.aliens[self_idx as usize];
    let tt = g.objs.aliens[target as usize];
    let yaw = strat_angle_xz(&me, &tt);
    let pitch = strat_pitch_toward(&me, &tt);
    let speed = sea_relslowelaser_speed(g);
    if let Some(shot) = spawn_projectile(g, Some(self_idx), 0, 0, 0, pitch, yaw, speed, SEA_ELASER_LIFE, SEA_ELASER_AP, ACF_COLLTYPE4) {
        g.objs.aliens[shot as usize].rotz = me.rotz;
    }
}

/// makesplash — visual no-op hook (strat_boss_sea.c:259).
#[inline]
fn sea_make_splash(_g: &mut Game, _idx: u16) {}

#[inline]
fn sea_enemy_up_sea(g: &mut Game) {
    play_se(g, SND_UPSEA);
}
#[inline]
fn sea_enemy_down_sea(g: &mut Game) {
    play_se(g, SND_DOWNSEA);
}

/// find_y_l equivalent: first active alien with the given shape word.
fn sea_find_shape(g: &Game, shape: u16) -> Option<u16> {
    for it in g.objs.active_indices() {
        let al = &g.objs.aliens[it as usize];
        if al.active && al.shape == shape {
            return Some(it);
        }
    }
    None
}

/// s_dec_var B,gsvar_byte1 (strat_boss_sea.c:285).
fn sea_dec_gsvar_byte1(g: &mut Game) {
    // s_dec_var B,gsvar_byte1 (GA2STRAT.ASM:3195) wraps 0->255 — finding #22.
    let v = gsvar_byte1(g);
    set_gsvar_byte1(g, v.wrapping_sub(1));
}

/// s_init_anim (bit 7 marker) (strat_boss_sea.c:294).
fn sea_anim_get(al: &Alien) -> u8 {
    al.animframe & 0x7F
}
fn sea_anim_set(al: &mut Alien, frame: u8) {
    al.animframe = 0x80 | (frame & 0x7F);
}

/// s_add_rnd2pos y,15,15,15 (strat_boss_sea.c:303).
fn sea_add_rnd2pos(g: &mut Game, idx: u16, range: i16) {
    let span = (range * 2 + 1) as u16;
    let rx = (sfrtl_random(g) % span) as i16 - range;
    let ry = (sfrtl_random(g) % span) as i16 - range;
    let rz = (sfrtl_random(g) % span) as i16 - range;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(rx);
    al.worldy = al.worldy.wrapping_add(ry);
    al.worldz = al.worldz.wrapping_add(rz);
}

/// delayexplode child (strat_boss_sea.c:312).
fn sea_expchild_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.aldead = 1;
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        sea_add_player_z(g, idx);
    }
}

fn sea_make_small_expobj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_obj(g, SEA_EXPSHAPE_SMALL)?;
    let s = sid(g, sea_expchild_strat);
    let p = g.objs.aliens[parent as usize];
    let al = &mut g.objs.aliens[child as usize];
    al.sflags3 &= !ASF3_REALOBJ;
    al.sflags |= ASF_COLLDISABLE;
    al.sflags2 |= ASF2_NOEXPSND | ASF2_RELEXPLODE;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.count = 2;
    al.worldx = p.worldx;
    al.worldy = p.worldy;
    al.worldz = p.worldz;
    Some(child)
}

// ---- bossseamon (strat_boss_sea.c:353) ----

pub fn strat_bossseamon_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossseamon_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, bossseamonexp_init);
    let r = sfrtl_random(g) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = BOSSSEAMON_HP;
    al.ap = BOSSSEAMON_AP;
    al.sbyte2 = r;
    al.roty = DEG180;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.type_ &= !ATZREMOVE;
    al.sbyte3 = 60;
    al.sbyte4 = 3;
    al.stratstate = 0;
    al.shape = SH_SEA_0_0;
    // bossseamon_Istrat falls into bossseamon_strat in one pass (GA2STRAT.ASM:3056)
    // — run the body on the spawn tick — finding #28.
    bossseamon_strat(g, idx);
}

fn bossseamon_strat(g: &mut Game, idx: u16) {
    let player = player(g);
    let gf = g.vars.gameframe;

    'restart: loop {
        let state = g.objs.aliens[idx as usize].stratstate;

        // state 0
        if state == 0 {
            g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
            if g.objs.aliens[idx as usize].sbyte3 == 0 {
                g.objs.aliens[idx as usize].stratstate = 2;
                continue 'restart;
            }
            g.objs.aliens[idx as usize].sbyte3 -= 1;
            let _ = speed_to(&mut g.objs.aliens[idx as usize], 20, 1);
            let sb2 = g.objs.aliens[idx as usize].sbyte2;
            sea_gen_vecs_angle(g, idx, sb2);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.vz = -40;
                al.sbyte2 = al.sbyte2.wrapping_add(4);
            }
            if !sea_not_delay(idx, 5, gf) {
                sea_enemy_down_sea(g);
                sea_make_splash(g, idx);
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte1 = 10;
                al.shape = SH_SEA_0_1_PROXY;
                al.stratstate += 1;
            }
        }

        // state 1
        if g.objs.aliens[idx as usize].stratstate == 1 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.vx = 0;
                al.vy = 0;
                al.vz = 0;
                al.sbyte1 = al.sbyte1.wrapping_sub(1);
            }
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 0;
                g.objs.aliens[idx as usize].shape = SH_SEA_0_0;
                sea_enemy_down_sea(g);
                sea_make_splash(g, idx);
            }
        }

        // state 2
        if g.objs.aliens[idx as usize].stratstate == 2 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags &= !ASF_COLLDISABLE;
                al.vz = 0;
                al.vx = al.vx.wrapping_neg();
                al.vy = -15;
                al.shape = SH_SEA_0_PROXY;
            }
            sea_enemy_down_sea(g);
            sea_make_splash(g, idx);
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            if g.objs.aliens[idx as usize].sbyte4 == 0 {
                g.objs.aliens[idx as usize].stratstate = 5;
                continue 'restart;
            }
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte4 -= 1;
            al.stratstate += 1;
        }

        // state 3
        if g.objs.aliens[idx as usize].stratstate == 3 {
            if g.objs.aliens[idx as usize].vy >= 0 && !sea_not_delay(idx, 3, gf) {
                if let Some(p) = player.filter(|p| p.active) {
                    let pi = player_idx(g).unwrap_or(0);
                    let _ = p;
                    sea_fire_relslowelaser(g, idx, pi);
                }
            }
            g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(1);
            if g.objs.aliens[idx as usize].worldy >= 0 {
                sea_enemy_down_sea(g);
                sea_make_splash(g, idx);
                let al = &mut g.objs.aliens[idx as usize];
                al.vy = 0;
                al.shape = SH_SEA_0_0;
                al.vel = 0;
                al.sbyte3 = 30;
                al.stratstate += 1;
            }
        }

        // state 4
        if g.objs.aliens[idx as usize].stratstate == 4 {
            g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
            if g.objs.aliens[idx as usize].sbyte3 == 0 {
                g.objs.aliens[idx as usize].stratstate = 2;
                continue 'restart;
            }
            g.objs.aliens[idx as usize].sbyte3 -= 1;
        }

        // state 5
        if g.objs.aliens[idx as usize].stratstate == 5 {
            sea_enemy_down_sea(g);
            sea_make_splash(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.vy = -20;
                al.shape = SH_SEA_0_PROXY;
            }
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            g.objs.aliens[idx as usize].stratstate += 1;
        }

        // state 6
        if g.objs.aliens[idx as usize].stratstate == 6 {
            g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(2);
            if g.objs.aliens[idx as usize].worldy >= 0 {
                sea_enemy_down_sea(g);
                sea_make_splash(g, idx);
                let al = &mut g.objs.aliens[idx as usize];
                al.vy = 0;
                al.shape = SH_SEA_0_1_PROXY;
                al.stratstate += 1;
            }
        }

        // state 7
        if g.objs.aliens[idx as usize].stratstate == 7 {
            if let Some(p) = player {
                let me_z = g.objs.aliens[idx as usize].worldz;
                if (me_z as i32 - p.worldz as i32).abs() >= 200 || me_z >= p.worldz {
                    let me = g.objs.aliens[idx as usize];
                    let ang = strat_angle_xz(&me, &p);
                    {
                        let al = &mut g.objs.aliens[idx as usize];
                        al.sbyte2 = ang;
                        al.vel = 20;
                    }
                    sea_gen_vecs_angle(g, idx, ang);
                    g.objs.aliens[idx as usize].stratstate = 5;
                } else {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.stratstate = 8;
                    al.vx = 0;
                    al.vy = 0;
                    al.vz = 50;
                    al.worldy = -150;
                    al.shape = SH_SEA_0_PROXY;
                }
            }
        }

        // state 8
        if g.objs.aliens[idx as usize].stratstate == 8 {
            g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(2);
            if g.objs.aliens[idx as usize].worldy >= 0 {
                // ROM draws vx=(rnd&7)+5 FIRST (GA2STRAT.ASM:3178-3179), THEN the
                // negate coin s_jmp_random (branch when random<127, so negate when
                // random>=127) — finding #10. Keep this draw order and the <127 coin.
                let vxr = (sfrtl_random(g) & 7) as i16 + 5;
                let neg = (sfrtl_random(g) & 0xFF) >= 127;
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte4 = 2;
                al.stratstate = 2;
                al.vy = 0;
                al.vx = if neg { vxr.wrapping_neg() } else { vxr };
                al.shape = SH_SEA_0_0;
            }
        }

        // common tail
        {
            let al = &mut g.objs.aliens[idx as usize];
            if al.worldx < SEA_BRIDGE_MINX2 {
                al.worldx = SEA_BRIDGE_MINX2;
            } else if al.worldx > SEA_BRIDGE_MAXX2 {
                al.worldx = SEA_BRIDGE_MAXX2;
            }
        }
        strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
        sea_add_player_z(g, idx);
        break;
    }
}

fn bossseamonexp_init(g: &mut Game, idx: u16) {
    sea_dec_gsvar_byte1(g);
    strat_explode(g, idx);
}

// ---- seamon (small, strat_boss_sea.c:579) ----

pub fn strat_seamon_init(g: &mut Game, idx: u16) {
    let s = sid(g, seamon_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let r = sfrtl_random(g) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = SEAMON_HP;
    al.ap = SEAMON_AP;
    al.vz = 30;
    al.roty = DEG180;
    al.sbyte1 = 10;
    al.sbyte2 = r;
    al.sbyte4 = 1;
    al.sword2 = -10;
    al.sbyte3 = 40;
    al.stratstate = 2;
    al.collflags |= COLLTYPE_ZENEMY;
    al.shape = SH_SEA_0_0;
}

fn seamon_strat(g: &mut Game, idx: u16) {
    let player = player(g);
    let gf = g.vars.gameframe;

    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

    // surface swim wiggle/animation
    if g.objs.aliens[idx as usize].stratstate == 3 || g.objs.aliens[idx as usize].worldy == 0 {
        if g.objs.aliens[idx as usize].sbyte3 == 0 {
            let sb2 = g.objs.aliens[idx as usize].sbyte2;
            // s_set_alvar2alvartab ...,sintab,-4 (GASTRATS.ASM:2071): sign-extend
            // then adiv2 x4 (toward ZERO). `/16` truncates toward zero; `>>4`
            // floors negatives — finding #24.
            let vx = (crate::snes_trig::SINTAB[sb2 as usize] as i16) / 16;
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.vx = vx;
                al.sbyte2 = al.sbyte2.wrapping_add(4);
                al.sbyte1 = al.sbyte1.wrapping_sub(1);
            }
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte1 = 10;
                al.shape = SH_SEA_0_1_PROXY;
                al.sflags2 ^= SEA_SFLAG2;
                // ROM `s_not_alsflag x,sflag2` + `s_beq` tests the WHOLE sflags
                // byte2 after the EOR, not just sflag2 (GASTRATS.ASM:2077-2079,
                // macro fact #28) — finding #12. Byte2 holds colldisable, sflag1
                // (the splash-down latch) and sflag2. Pre-landing only sflag2 is
                // live so the frame alternates sea_0_1/sea_0_0; once the landing
                // latch sets sflag1+colldisable the byte is never 0 and the frame
                // is forced to sea_0_0 forever. colldisable lives in `sflags` in
                // this port, so reproduce the observable rule from those bits.
                let byte2_nonzero = al.sflags2 & (SEA_SFLAG2 | SEA_SFLAG1) != 0
                    || al.sflags & ASF_COLLDISABLE != 0;
                if byte2_nonzero {
                    al.shape = SH_SEA_0_0;
                }
            }
        } else {
            g.objs.aliens[idx as usize].sbyte3 -= 1;
        }
    }

    // airborne / surface handling
    if g.objs.aliens[idx as usize].worldy >= -1 {
        if g.objs.aliens[idx as usize].vy >= 0 {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = 0;
            al.vy = 0;
        }
    } else {
        g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(2);
        if g.objs.aliens[idx as usize].worldy >= -30 {
            if g.objs.aliens[idx as usize].vy >= 0 {
                g.objs.aliens[idx as usize].shape = SH_SEA_0_0;
                if g.objs.aliens[idx as usize].sflags2 & SEA_SFLAG1 == 0 {
                    g.objs.aliens[idx as usize].sflags2 |= SEA_SFLAG1;
                    sea_enemy_down_sea(g);
                    sea_make_splash(g, idx);
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte3 = 10;
                    al.sflags |= ASF_COLLDISABLE;
                } else {
                    // sflag1 ALREADY latched -> ROM jumps to .nds and snaps flush
                    // to the surface (GASTRATS.ASM:2097->2105-2108) — finding #11.
                    let al = &mut g.objs.aliens[idx as usize];
                    al.worldy = 0;
                    al.vy = 0;
                }
            }
        } else {
            g.objs.aliens[idx as usize].shape = SH_SEA_0_PROXY;
            if !sea_not_delay(idx, 4, gf) {
                if let Some(p) = player.filter(|p| p.active) {
                    let me = g.objs.aliens[idx as usize];
                    let zd = (me.worldz as i32 - p.worldz as i32).abs();
                    let xd = (me.worldx as i32 - p.worldx as i32).abs();
                    if (500..1000).contains(&zd) && xd < 200 {
                        let pi = player_idx(g).unwrap_or(0);
                        sea_fire_relslowelaser(g, idx, pi);
                    }
                }
            }
        }
    }

    // .nzv jump countdown
    g.objs.aliens[idx as usize].sbyte4 = g.objs.aliens[idx as usize].sbyte4.wrapping_sub(1);
    if g.objs.aliens[idx as usize].sbyte4 != 0 {
        return;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate += 1;
        if al.stratstate > 3 {
            al.stratstate = 1;
        }
    }
    let state = g.objs.aliens[idx as usize].stratstate;
    if state == 1 || state == 2 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte4 = 40;
        al.vy = -15;
    } else if state == 3 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte4 = 60;
        al.vy = -25;
    }

    sea_enemy_up_sea(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_COLLDISABLE;
        al.vx = 0;
        al.sflags2 &= !SEA_SFLAG1;
    }
    sea_make_splash(g, idx);
}

// ---- flyingfish (strat_boss_sea.c:715) ----

fn flyingfish_init(g: &mut Game, idx: u16) {
    let s = sid(g, flyingfish_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.roty = al.roty.wrapping_add(DEG180);
    al.hp = FISH_HP;
    al.ap = FISH_AP;
    sea_anim_set(al, 0);
    al.type_ |= ATZREMOVE;
}

fn flyingfish_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & SEA_SFLAG2 != 0 {
        return;
    }

    sea_make_splash(g, idx);

    if g.objs.aliens[idx as usize].worldy < 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(2);
        al.worldy = al.worldy.wrapping_add(al.vy);
        if al.worldy >= 0 {
            al.worldy = 0;
        }
    }

    sea_add_player_z(g, idx);

    if g.objs.aliens[idx as usize].sflags2 & SEA_SFLAG3 == 0 {
        let newx = chase_proportional(g.objs.aliens[idx as usize].worldx, -200, 3);
        g.objs.aliens[idx as usize].worldx = newx;
        if newx != -200 && g.objs.aliens[idx as usize].worldx >= -150 {
            return;
        }
    } else {
        let newx = chase_proportional(g.objs.aliens[idx as usize].worldx, 200, 3);
        g.objs.aliens[idx as usize].worldx = newx;
        if newx != 200 && g.objs.aliens[idx as usize].worldx < 150 {
            return;
        }
    }

    // .jumping
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        g.objs.aliens[idx as usize].roty = strat_angle_xz(&me, &p);
    }
    let s = sid(g, flyingfish_flying_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        sea_anim_set(al, 0);
        al.stratptr = Some(s);
        al.vel = 70;
    }
    let roty = g.objs.aliens[idx as usize].roty;
    sea_gen_vecs_angle(g, idx, roty);
    g.objs.aliens[idx as usize].vy = -15;
    sea_make_splash(g, idx);
    sea_enemy_up_sea(g);

    flyingfish_flying_strat(g, idx);
}

fn flyingfish_flying_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(2);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    sea_add_player_z(g, idx);

    if g.objs.aliens[idx as usize].worldy >= 0 {
        sea_make_splash(g, idx);
        g.objs.aliens[idx as usize].sflags2 |= SEA_SFLAG2;
        if g.objs.aliens[idx as usize].worldy > 300 {
            g.objs.aldead = 1;
        }
    }
}

// ---- bossgs (shadow clones, strat_boss_sea.c:819) ----

fn bossgs_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossgs_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.sbyte1 = 40;
    al.type_ |= ATZREMOVE;
}

/// BLACK_C (ASM/COLTABS.ASM:31, 64x COLNORM 9,9). The port encodes coltabs
/// as small IDs (sf-render ID_0_C..ID_5_C = 0..5); 6 is the next free slot.
/// Until sf-render maps it, unknown IDs resolve to the debug material — the
/// game-state flicker below is still the ROM-accurate part.
const COLTAB_BLACK_C: u16 = 6;

fn bossgs_strat(g: &mut Game, idx: u16) {
    // Shadow-clone flicker (D2STRATS.ASM:481-486): coltab = BLACK_C on odd
    // gameframes, cleared on even.
    {
        let al = &mut g.objs.aliens[idx as usize];
        if g.vars.gameframe & 1 != 0 {
            al.coltab = COLTAB_BLACK_C;
        } else {
            al.coltab = 0;
        }
    }

    // s_fchase_alvar2alvar W,worldx,sword1,5 (D2STRATS.ASM:488) is Fchase_A
    // (STRATMAC.INC:559): fixed +-5 step with NO overshoot clamp — only an
    // exact-equal compare stops it, so the ROM oscillates within +-5 of
    // sword1 once close. strat_chase would clamp to the target.
    let sword1 = g.objs.aliens[idx as usize].sword1;
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.worldx < sword1 {
            al.worldx = al.worldx.wrapping_add(5);
        } else if al.worldx > sword1 {
            al.worldx = al.worldx.wrapping_sub(5);
        }
    }

    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(50);
        if let Some(p) = player(g) {
            if (p.worldz as i32) - (g.objs.aliens[idx as usize].worldz as i32) > 2000 {
                g.objs.aldead = 1;
            }
        }
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    sea_add_player_z(g, idx);
}

// ---- bossg (strat_boss_sea.c:869) ----

pub fn strat_bossg_init(g: &mut Game, idx: u16) {
    set_maptrigger(g, 0);
    let s = sid(g, bossg_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, bossgexplode_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = HARD_HP;
        al.ap = BOSSG_AP;
        sea_anim_set(al, 0);
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
        al.stratmem = 0;
    }
    play_se(g, 0x9D);
    g.objs.aliens[idx as usize].shape = SH_BOSS_G_0;
}

/// .genspark helper — sprite spark not ported (strat_boss_sea.c:900).
fn bossg_genspark(_g: &mut Game, _idx: u16) {}

/// .launchfish (strat_boss_sea.c:905).
fn bossg_launch_fish(g: &mut Game, self_idx: u16) -> Option<u16> {
    let fish = make_obj(g, SH_F_FISH_PROXY)?;
    let me = g.objs.aliens[self_idx as usize];
    {
        let al = &mut g.objs.aliens[fish as usize];
        al.worldx = me.worldx;
        al.worldy = me.worldy.wrapping_add(30);
        al.worldz = me.worldz;
        al.rotx = me.rotx;
        al.roty = me.roty;
        al.rotz = me.rotz;
    }
    flyingfish_init(g, fish);
    Some(fish)
}

/// .generateshadows (strat_boss_sea.c:923).
fn bossg_generate_shadows(g: &mut Game, self_idx: u16) {
    const OFFSETS: [i16; 3] = [-100, 0, 100];
    play_se(g, 0x2D);
    for &off in OFFSETS.iter() {
        let Some(shadow) = make_obj(g, SH_BOSS_G_S) else {
            return;
        };
        let me = g.objs.aliens[self_idx as usize];
        {
            let al = &mut g.objs.aliens[shadow as usize];
            al.worldx = me.worldx;
            al.worldy = me.worldy;
            al.worldz = me.worldz.wrapping_sub(50);
            al.rotx = me.rotx;
            al.roty = me.roty;
            al.rotz = me.rotz;
            al.sword1 = off;
        }
        bossgs_init(g, shadow);
    }
}

/// .move2 (strat_boss_sea.c:948).
fn bossg_move2(g: &mut Game, self_idx: u16) {
    sea_add_player_z(g, self_idx);
    // s_jmp_notdelay 1,.nosplash (D2STRATS.ASM:372, no al1pt): gameframe&1==0.
    if !sea_not_delay(0, 1, g.vars.gameframe) {
        sea_make_splash(g, self_idx);
    }
}

fn bossg_strat(g: &mut Game, idx: u16) {
    loop {
        let mode = g.objs.aliens[idx as usize].stratmem;
        match mode {
            m if m == BOSSG_MODE_WAITHITPLAYER => {
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(40);
                if sea_dz_less(g, idx, 150) {
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                return;
            }
            m if m == BOSSG_MODE_SCROLLMSG => {
                if sea_dz_less(g, idx, 140) {
                    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(40);
                }
                g.objs.aliens[idx as usize].tx = g.objs.aliens[idx as usize].tx.wrapping_add(4);
                if g.objs.aliens[idx as usize].tx & 127 == 0 {
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                sea_add_player_z(g, idx);
                return;
            }
            m if m == BOSSG_MODE_SF9E_A || m == BOSSG_MODE_SF9E_B => {
                play_se(g, 0x9E);
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            m if m == BOSSG_MODE_LOOPBACK_PT || m == BOSSG_MODE_RUNAWAY_B => {
                if sea_dz_less(g, idx, 1000) {
                    bossg_genspark(g, idx);
                }
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(70);
                if !sea_dz_less(g, idx, 4000) {
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                if bossmaxhp(g) == 0 {
                    sea_add_player_z(g, idx);
                } else {
                    bossg_move2(g, idx);
                }
                return;
            }
            m if m == BOSSG_MODE_DISAPPEAR => {
                g.objs.aliens[idx as usize].shape = SH_NULLSHAPE_WORD;
                let mt = maptrigger(g);
                set_maptrigger(g, mt | 1);
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            m if m == BOSSG_MODE_WAITSOMETIME => {
                // s_jmp_notdelay 2,.notyet (D2STRATS.ASM:226, no al1pt):
                // regen 1 HP only when gameframe&3==0 (every 4th frame).
                if bossmaxhp(g) != 0
                    && !sea_not_delay(0, 2, g.vars.gameframe)
                    && g.objs.aliens[idx as usize].hp != BOSSG_HP
                {
                    g.objs.aliens[idx as usize].hp += 1;
                }
                if maptrigger(g) & 1 == 0
                    && sea_find_shape(g, SH_SEA_0_PROXY).is_none()
                    && sea_find_shape(g, SH_SEA_0_0).is_none()
                    && sea_find_shape(g, SH_SEA_0_1_PROXY).is_none()
                {
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                if bossmaxhp(g) == 0 {
                    sea_add_player_z(g, idx);
                } else {
                    bossg_move2(g, idx);
                }
                return;
            }
            m if m == BOSSG_MODE_APPEAR => {
                g.objs.aliens[idx as usize].shape = SH_BOSS_G_0;
                if bossmaxhp(g) == 0 {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.hp = BOSSG_HP;
                    al.ap = BOSSG_AP;
                    set_bossmaxhp(g, BOSSG_HP as u16);
                }
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            m if m == BOSSG_MODE_MOVETO600H => {
                if g.objs.aliens[idx as usize].sflags4 & SEA_SFLAG8 == 0 && sea_dz_less(g, idx, 1500) {
                    play_se(g, 0x9D);
                    g.objs.aliens[idx as usize].sflags4 |= SEA_SFLAG8;
                }
                if sea_dz_less(g, idx, 600) {
                    g.objs.aliens[idx as usize].sflags4 &= !SEA_SFLAG8;
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(40);
                bossg_move2(g, idx);
                return;
            }
            // .opentrunk
            8 | 14 | 20 | 26 => {
                let anim = sea_anim_get(&g.objs.aliens[idx as usize]);
                if anim == 0 {
                    play_se(g, 0x5A);
                }
                if anim >= 9 {
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                sea_anim_set(&mut g.objs.aliens[idx as usize], anim + 1);
                bossg_move2(g, idx);
                return;
            }
            // .launchleftfish
            9 | 15 | 21 | 27 => {
                let _ = bossg_launch_fish(g, idx);
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            // .launchrightfish
            10 | 16 | 22 | 28 => {
                if let Some(fish) = bossg_launch_fish(g, idx) {
                    g.objs.aliens[fish as usize].sflags2 |= SEA_SFLAG3;
                }
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            // .waitabit2
            11 | 13 | 17 | 19 | 23 | 25 | 29 => {
                g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
                if g.objs.aliens[idx as usize].sbyte1 == 10 {
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                bossg_move2(g, idx);
                return;
            }
            // .closetrunk
            12 | 18 | 24 | 30 => {
                let anim = sea_anim_get(&g.objs.aliens[idx as usize]);
                if anim == 9 {
                    play_se(g, 0x59);
                }
                if anim == 0 {
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                sea_anim_set(&mut g.objs.aliens[idx as usize], anim - 1);
                bossg_move2(g, idx);
                return;
            }
            m if m == BOSSG_MODE_GENSHADOWS => {
                bossg_generate_shadows(g, idx);
                g.objs.aliens[idx as usize].stratmem += 1;
                continue;
            }
            m if m == BOSSG_MODE_WAITABIT => {
                g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
                if g.objs.aliens[idx as usize].sbyte1 == 70 {
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                    g.objs.aliens[idx as usize].stratmem += 1;
                    continue;
                }
                bossg_move2(g, idx);
                return;
            }
            // .loopback (BOSSG_MODE_LOOPBACK) + default
            _ => {
                g.objs.aliens[idx as usize].stratmem = BOSSG_MODE_LOOPBACK_PT;
                continue;
            }
        }
    }
}

// ---- bossgexplode (strat_boss_sea.c:1163) ----

fn bossgexplode_init(g: &mut Game, idx: u16) {
    while let Some(shadow) = sea_find_shape(g, SH_BOSS_G_S) {
        g.objs.free(shadow);
    }
    let s = sid(g, bossgexplode_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    bossgexplode_strat(g, idx);
}

fn bossgexplode_strat(g: &mut Game, idx: u16) {
    for _ in 0..3 {
        if let Some(child) = sea_make_small_expobj(g, idx) {
            g.objs.aliens[child as usize].sflags4 |= ASF4_NOPOLYEXP;
            g.objs.aliens[child as usize].worldy = g.objs.aliens[child as usize].worldy.wrapping_sub(60);
            sea_add_rnd2pos(g, child, 15);
        }
    }

    let anim = sea_anim_get(&g.objs.aliens[idx as usize]);
    if anim < 9 {
        sea_anim_set(&mut g.objs.aliens[idx as usize], anim + 1);
    }

    if sea_dz_less(g, idx, 350) {
        let mt = maptrigger(g);
        set_maptrigger(g, mt | 2);
        strat_boss_explode_init(g, idx);
        return;
    }

    sea_make_splash(g, idx);
}
// BOSSSEA_END

// ============================================================
// BOSS8_BEGIN (strat_boss8.c) — the "washing machine" wash boss.
// ============================================================

const BOSS8_SCALE: i16 = 3;
const BOSS8_HP: u8 = 32;
const NUCLEUS_LAUNCH_AP: u8 = 8;
const NUCLEUS_BEAML_AP: u8 = 8;
const NUCLEUS_HEIGHT: i16 = (100 / 2) << BOSS8_SCALE; // 400
const NUCLEUS_VIEWCY: i16 = -60;
const BOSS8_CIRC: i16 = 210 << BOSS8_SCALE; // 1680

const SH_BOSS_8_5: u16 = 44;
const SH_HOU_4: u16 = 45;
const SH_BOSS_8_4: u16 = 46;
const SH_BOSS_8_0: u16 = 47;
const SH_BOSS_8_1: u16 = 264;
const SH_BOSS_8_1C: u16 = 265;
const SH_SPARKLAS: u16 = 266;
const SH_SHRAP1: u16 = 267;
const SH_SHYPER: u16 = 268;
const SH_ZACO_9: u16 = 269;

// sflags (mapped onto sflags2).
const B8_SFLAG1: u8 = ASF2_SFLAG1; // 0x10
const B8_SFLAG2: u8 = 0x20;
const B8_SFLAG4: u8 = 0x40;
const B8_SFLAG5: u8 = 0x80;
const B8_SHOT_NOCHASE: u8 = 0x01;

const B8_EXPSHAPE_MEDIUM: u16 = 2;
const B8_EXPSHAPE_LARGE: u16 = 3;
const B8_EXPSHAPE_FOLARGE: u16 = 4;

const B8_CHILD_COVER: u8 = 1;
const B8_CHILD_BEAM1: u8 = 2;
const B8_CHILD_BEAM2: u8 = 3;
const B8_CHILD_BEAM3: u8 = 4;

// ---- Small helpers (strat_boss8.c:144-220) ----

#[inline]
fn b8_add_player_z(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

/// achase (proportional angle chase) — strat_boss8.c:156.
fn b8_achase_angle(current: &mut u8, target: u8, shift: u32) {
    if *current == target {
        return;
    }
    let diff = current.wrapping_sub(target) as i8;
    let mut step = diff >> shift;
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    *current = current.wrapping_sub(step as u8);
}

/// s_obj2obj_3dangle (strat_boss8.c:214).
fn b8_aim_3d(g: &mut Game, idx: u16, target: &Alien, shift: u32) {
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    b8_achase_angle(&mut roty, strat_angle_xz(&me, target), shift);
    b8_achase_angle(&mut rotx, strat_pitch_toward(&me, target), shift);
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = roty;
    al.rotx = rotx;
}

/// s_make_childobj shape,num,strat,colltype (strat_boss8.c:317).
fn b8_spawn_child(g: &mut Game, mother: u16, shape: u16, child_num: u8, init_fn: StrategyFn, colltype: u8) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    g.objs.aliens[child as usize].shape = shape;
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    g.objs.aliens[child as usize].collflags |= colltype;
    init_fn(g, child);
    Some(child)
}

// ---- Explosion factories (strat_boss8.c:343-411) ----

fn b8_make_exp_obj(g: &mut Game, parent: u16, size_shape: u16) -> Option<u16> {
    let child = make_obj(g, 0)?;
    let s_tick = sid(g, boss8_delayexplode_strat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags |= ASF_COLLDISABLE;
        al.sflags2 |= ASF2_NOEXPSND | ASF2_RELEXPLODE;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.stratptr = Some(s_tick);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
    }
    copy_pos(g, child, parent);
    g.objs.aliens[child as usize].shape = size_shape;
    Some(child)
}

fn b8_add_rnd_xy(g: &mut Game, idx: u16) {
    addrnd2pos_xy(g, idx);
}

/// `s_add_rnd2pos y,127,127,0` (GB3STRAT.ASM:472): per axis `(rnd&mask)-mask/2`,
/// one draw PER AXIS even for mask=0 (STRATMAC.INC:7336, macro fact #19). So x/y
/// get `(rnd&127)-63` and z draws a random masked to 0 (net 0) — finding #17.
fn b8_add_rnd2pos_folexp(g: &mut Game, idx: u16) {
    let rx = (sfrtl_random(g) & 127) as i16 - 63;
    let ry = (sfrtl_random(g) & 127) as i16 - 63;
    let _rz = sfrtl_random(g) & 0; // masked to 0, but the draw still happens
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(rx);
    al.worldy = al.worldy.wrapping_add(ry);
}

fn b8_add_rnd_xyz(g: &mut Game, idx: u16) {
    // addrnd2posxyz2_srou (EXPSTRAT.ASM:359-382): draw x, y, z IN THAT ORDER,
    // each a sign-extended byte then `asl a` (<<1, ±254 spread) — finding #17.
    let rx = ((sfrtl_random(g) & 0xFF) as u8 as i8 as i16) << 1;
    let ry = ((sfrtl_random(g) & 0xFF) as u8 as i8 as i16) << 1;
    let rz = ((sfrtl_random(g) & 0xFF) as u8 as i8 as i16) << 1;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(rx);
    al.worldy = al.worldy.wrapping_add(ry);
    al.worldz = al.worldz.wrapping_add(rz);
}

/// delayexplode_strat (strat_boss8.c:382).
fn boss8_delayexplode_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_HITFLASH;
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.aldead = 1;
        if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
            g.call_strat(exp, idx);
        }
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        b8_add_player_z(g, idx);
    }
}

// ---- wallrot (strat_boss8.c:419-438) ----

fn b8_wallrot_common(g: &mut Game, idx: u16, zbase: i16, zref: i16) {
    let al = g.objs.aliens[idx as usize];
    let a = (al.sbyte2 as f32) * (2.0f32 * 3.141_592_65_f32 / 256.0f32);
    let dist = al.sword2 as f32;
    let x2 = (dist * a.sin()).round() as i16;
    let z2 = (dist * a.cos()).round() as i16;
    let gsv = gsvar_byte1(g);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = x2;
    al.worldz = z2.wrapping_mul(2).wrapping_add(zbase).wrapping_add(zref);
    al.sbyte2 = al.sbyte2.wrapping_add(gsv);
}

fn b8_wallrot(g: &mut Game, idx: u16) {
    let zref = g.vars.player_posz;
    b8_wallrot_common(g, idx, 160 << BOSS8_SCALE, zref);
}

fn b8_wallrot2(g: &mut Game, idx: u16) {
    let zref = pviewposz(g);
    b8_wallrot_common(g, idx, 210 << BOSS8_SCALE, zref);
}

// ---- Projectiles (strat_boss8.c:446-624) ----

fn b8_fire_hplasma(g: &mut Game, self_idx: u16, target: u16) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = make_obj(g, 0)?;
    let s = sid(g, boss8_homing_shot_strat);
    let s_coll = strat_projectile_on_collide_sid(g);
    copy_pos(g, shot, self_idx);
    // ROM `s_weapon_rot #0,#deg180` (GB3STRAT.ASM:152) launches relative to the
    // FIRER's rots (yaw = firer.roty + deg180, pitch = firer.rotx + 0); the shot
    // then homes on the player — finding #27. Don't aim straight at the player at
    // spawn.
    let firer = g.objs.aliens[self_idx as usize];
    let ang = firer.roty.wrapping_add(DEG180);
    let pit = firer.rotx;
    let al = &mut g.objs.aliens[shot as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 1;
    al.ap = 10;
    al.vel = 60;
    al.count = 50;
    al.snd2 = 6;
    al.type_ = ATLASER | ATZREMOVE;
    al.sflags |= ASF_INVISIBLE;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = boss_obj_index_or_null(self_idx);
    al.fireobjptr = boss_obj_index_or_null(target);
    al.sbyte1 = ang;
    al.sbyte2 = pit;
    Some(shot)
}

fn boss8_homing_shot_strat(g: &mut Game, idx: u16) {
    let target = {
        let fp = g.objs.aliens[idx as usize].fireobjptr;
        if fp != 0 {
            g.objs.get((fp - 1) as i32).copied()
        } else {
            None
        }
    };
    if let Some(t) = target.filter(|t| t.active) {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - t.worldz as i32).abs() >= 500 {
            let mut sb1 = me.sbyte1;
            let mut sb2 = me.sbyte2;
            b8_achase_angle(&mut sb1, strat_angle_xz(&me, &t), 4);
            b8_achase_angle(&mut sb2, strat_pitch_toward(&me, &t), 4);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = sb1;
            al.sbyte2 = sb2;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.sbyte1;
        al.rotx = al.sbyte2;
    }
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    b8_add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    let Some(p) = player(g) else {
        return;
    };
    let dz = g.objs.aliens[idx as usize].worldz.wrapping_sub(p.worldz);
    if !(-12000..=12000).contains(&dz) {
        g.objs.aldead = 1;
        return;
    }
    if g.vars.gameflags & GF_BOSSDEAD != 0 || bossflags(g) & BF_DYING != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = 0;
        g.objs.aldead = 1;
    }
}

fn b8_fire_kamimissile(g: &mut Game, self_idx: u16, target: u16) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = make_obj(g, SH_ZACO_9)?;
    let s = sid(g, boss8_kamimissile_strat);
    let s_coll = strat_projectile_on_collide_sid(g);
    copy_pos(g, shot, self_idx);
    let al = &mut g.objs.aliens[shot as usize];
    al.roty = DEG180;
    al.rotx = 0;
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 2;
    al.ap = 8;
    al.vel = 60;
    al.count = 100;
    al.snd2 = 2;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.sflags |= ASF_SHADOW;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = boss_obj_index_or_null(self_idx);
    al.fireobjptr = boss_obj_index_or_null(target);
    al.sflags2 |= B8_SFLAG1;
    Some(shot)
}

fn boss8_kamimissile_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    if g.objs.aliens[idx as usize].sflags2 & B8_SHOT_NOCHASE == 0 {
        let target = {
            let fp = g.objs.aliens[idx as usize].fireobjptr;
            if fp != 0 {
                g.objs.get((fp - 1) as i32).copied()
            } else {
                None
            }
        };
        if let Some(t) = target.filter(|t| t.active) {
            let me = g.objs.aliens[idx as usize];
            let man = (me.worldx as i32 - t.worldx as i32).abs()
                + (me.worldy as i32 - t.worldy as i32).abs()
                + (me.worldz as i32 - t.worldz as i32).abs();
            if man < 300 {
                g.objs.aliens[idx as usize].sflags2 |= B8_SHOT_NOCHASE;
            } else {
                b8_aim_3d(g, idx, &t, 3);
            }
        }
        strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    b8_add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);

    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    let Some(p) = player(g) else {
        return;
    };
    let dz = g.objs.aliens[idx as usize].worldz.wrapping_sub(p.worldz);
    if !(-12000..=12000).contains(&dz) {
        g.objs.aldead = 1;
        return;
    }
    if g.vars.gameflags & GF_BOSSDEAD != 0 || bossflags(g) & BF_DYING != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = 0;
        g.objs.aldead = 1;
    }
}

/// zaco_9 live-count gate (strat_boss8.c:613).
fn b8_count_kamimissiles(g: &mut Game) -> u8 {
    let target = sid(g, boss8_kamimissile_strat);
    let mut n = 0u8;
    for i in 0..NUMBER_AL {
        let al = &g.objs.aliens[i];
        if al.active && al.stratptr == Some(target) {
            n = n.wrapping_add(1);
        }
    }
    n
}

/// Registry id of the shared projectile collide handler.
fn strat_projectile_on_collide_sid(g: &mut Game) -> StratId {
    sid(g, strat_projectile_on_collide)
}

// ---- straight_Istrat (strat_boss8.c:630) ----

fn boss8_straight_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, boss8_straight_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    boss8_straight_strat(g, idx);
}

fn boss8_straight_strat(g: &mut Game, idx: u16) {
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    b8_add_player_z(g, idx);
}

// ---- boss8 shell (strat_boss8.c:652) ----

pub fn strat_boss8_init(g: &mut Game, idx: u16) {
    let hp = if currentlevel(g) != 1 { BOSS8_HP * 2 } else { BOSS8_HP };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = hp;
        al.ap = HARD_AP;
    }
    g.vars.gameflags &= !GF_BOSSDEAD;
    let bf = bossflags(g);
    set_bossflags(g, bf & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
    set_bossmaxhp(g, hp as u16);
    g.vars.meters = 1;

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_BOSS_8_0;
        al.worldy = (-50 << BOSS8_SCALE) + NUCLEUS_HEIGHT; // 0
    }

    let s = sid(g, boss8wait_strat);
    let s_exp = sid(g, boss8die_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
    }

    let _ = b8_spawn_child(g, idx, SH_BOSS_8_1C, B8_CHILD_COVER, boss8cov_istrat, COLLTYPE_ENEMY2);

    if let Some(c) = b8_spawn_child(g, idx, SH_BOSS_8_5, B8_CHILD_BEAM1, nucleusbeaml_istrat, COLLTYPE_ENEMY2) {
        let al = &mut g.objs.aliens[c as usize];
        al.worldy = (-75 << BOSS8_SCALE) + NUCLEUS_HEIGHT;
        al.worldz = BOSS8_CIRC;
        al.sbyte2 = DEG45.wrapping_add(DEG22);
    }
    if let Some(c) = b8_spawn_child(g, idx, SH_BOSS_8_5, B8_CHILD_BEAM2, nucleusbeaml_istrat, COLLTYPE_ENEMY2) {
        let al = &mut g.objs.aliens[c as usize];
        al.worldy = (-35 << BOSS8_SCALE) + NUCLEUS_HEIGHT;
        al.worldz = BOSS8_CIRC;
        al.sbyte2 = DEG180.wrapping_add(DEG22);
    }
    if let Some(c) = b8_spawn_child(g, idx, SH_BOSS_8_5, B8_CHILD_BEAM3, nucleusbeaml_istrat, COLLTYPE_ENEMY2) {
        let al = &mut g.objs.aliens[c as usize];
        al.worldy = (-75 << BOSS8_SCALE) + NUCLEUS_HEIGHT;
        al.worldz = BOSS8_CIRC;
        al.sbyte2 = 0u8.wrapping_sub(DEG45.wrapping_add(DEG22));
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
        al.sbyte4 = 150;
        al.sflags2 &= !(B8_SFLAG1 | B8_SFLAG2);
    }
    set_gsvar_byte1(g, 0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.sflags |= ASF_COLLDISABLE;
    }

    boss8_cont(g, idx);
}

fn boss8wait_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss8wait_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    boss8wait_strat(g, idx);
}

fn boss8wait_strat(g: &mut Game, idx: u16) {
    let c1 = boss_find_child_obj(g, idx, B8_CHILD_BEAM1);
    if let Some(c) = c1 {
        if g.objs.aliens[c as usize].sflags2 & B8_SFLAG1 == 0 {
            boss8_cont(g, idx);
            return;
        }
    }
    let c2 = boss_find_child_obj(g, idx, B8_CHILD_BEAM2);
    if let Some(c) = c2 {
        if g.objs.aliens[c as usize].sflags2 & B8_SFLAG1 == 0 {
            boss8_cont(g, idx);
            return;
        }
    }
    let c3 = boss_find_child_obj(g, idx, B8_CHILD_BEAM3);
    match c3 {
        None => {
            boss8a_init(g, idx);
            return;
        }
        Some(c) if g.objs.aliens[c as usize].sflags2 & B8_SFLAG1 != 0 => {
            boss8a_init(g, idx);
            return;
        }
        _ => {}
    }
    boss8_cont(g, idx);
}

fn boss8_cont(g: &mut Game, idx: u16) {
    let ppz = g.vars.player_posz;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = (210i16 << BOSS8_SCALE).wrapping_add(ppz);
        al.sbyte4 = al.sbyte4.wrapping_sub(1);
        if al.sbyte4 == 0 {
            al.sbyte4 = 150;
            al.sflags2 ^= B8_SFLAG1;
        }
    }
    if g.vars.gameframe & 7 == 0 {
        let sflag1 = g.objs.aliens[idx as usize].sflags2 & B8_SFLAG1 != 0;
        let gsv = gsvar_byte1(g) as i8;
        if sflag1 {
            if gsv != -5 {
                set_gsvar_byte1(g, (gsv - 1) as u8);
            }
        } else if gsv != 5 {
            set_gsvar_byte1(g, (gsv + 1) as u8);
        }
    }
}

fn boss8a_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss8a_strat);
    let s_coll = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.sflags &= !ASF_COLLDISABLE;
        al.sbyte2 = 100;
        al.animframe = 0;
    }
    play_se(g, 0x73);
    boss8a_strat(g, idx);
}

fn boss8a_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= B8_SFLAG4;

    if g.objs.aliens[idx as usize].sflags2 & B8_SFLAG5 != 0 {
        // s_add_anim x,#1,#15,.nanim is the 4-arg/label form: CAP at max-1 (14)
        // and jump, NOT wrap (STRATLIB.INC:180-247, GB3STRAT.ASM:146) — finding
        // #13. The fully-open pose holds instead of looping 14->0.
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe < 14 {
            al.animframe += 1;
        }
    }

    let frame = (g.vars.gameframe & 31) as u8;
    if frame == 25 || frame == 30 {
        if let Some(p) = player_idx(g) {
            if g.objs.aliens[p as usize].active {
                let _ = b8_fire_hplasma(g, idx, p);
            }
        }
    }

    if currentlevel(g) == 1 {
        boss8_cont(g, idx);
        return;
    }

    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        boss8b_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;

    for n in B8_CHILD_BEAM1..=B8_CHILD_BEAM3 {
        if let Some(c) = boss_find_child_obj(g, idx, n) {
            if g.objs.aliens[c as usize].sflags2 & B8_SFLAG1 == 0 {
                boss8b_init(g, idx);
                return;
            }
        }
    }

    boss8_cont(g, idx);
}

fn boss8b_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss8b_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(s);
        al.sbyte2 = 15;
    }
    for n in B8_CHILD_BEAM1..=B8_CHILD_BEAM3 {
        if let Some(c) = boss_find_child_obj(g, idx, n) {
            g.objs.aliens[c as usize].sflags2 &= !B8_SFLAG1;
        }
    }
    play_se(g, 0x72);
    boss8b_strat(g, idx);
}

fn boss8b_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 &= !B8_SFLAG4;

    if g.objs.aliens[idx as usize].sflags2 & B8_SFLAG5 == 0 {
        g.objs.aliens[idx as usize].animframe = 0;
    }

    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        boss8wait_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;

    boss8_cont(g, idx);
}

// ---- boss8die (strat_boss8.c:915) ----

fn boss8die_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, boss8die_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    g.vars.gameflags |= GF_BOSSDEAD;
    g.objs.aliens[idx as usize].sbyte2 = 30;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    g.vars.gameflags |= GF_STAGEDONE;
    boss_dying(g);
    // ROM boss8die_Istrat (GB3STRAT.ASM:208-220) never touches bossmaxHP, and
    // s_boss_dying doesn't either — finding #15. Leave the bar's max alone
    // (its drain is the finding-#4 accumulator lane, out of this scope).
    boss8die_strat(g, idx);
}

fn boss8die_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player_idx(g) {
        if g.objs.aliens[p as usize].active {
            let py = chase_proportional(g.objs.aliens[p as usize].worldy, NUCLEUS_VIEWCY + 20, 3);
            let px = chase_proportional(g.objs.aliens[p as usize].worldx, 0, 3);
            g.objs.aliens[p as usize].worldy = py;
            g.objs.aliens[p as usize].worldx = px;
        }
    }

    if g.vars.gameframe & 1 == 0 {
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;

        if let Some(e) = b8_make_exp_obj(g, idx, B8_EXPSHAPE_MEDIUM) {
            b8_add_rnd_xy(g, e);
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            if g.vars.gameframe & 3 == 0 {
                g.objs.aliens[e as usize].sflags2 &= !ASF2_NOEXPSND;
            }
        }
        if let Some(e) = b8_make_exp_obj(g, idx, B8_EXPSHAPE_LARGE) {
            b8_add_rnd_xy(g, e);
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            if g.vars.gameframe & 3 == 0 {
                g.objs.aliens[e as usize].sflags2 &= !ASF2_NOEXPSND;
            }
        }

        if let Some(e) = make_obj(g, SH_SHYPER) {
            let s = sid(g, boss8_straight_istrat);
            // ROM `s_set_alvar W,y,al_worldy,viewposy` (GB3STRAT.ASM:254) uses the
            // camera viewposy, NOT pviewposy — finding #16.
            let vy = viewposy(g);
            let rz = sfrtl_random(g) as u8;
            {
                let al = &mut g.objs.aliens[e as usize];
                al.sflags |= ASF_COLLDISABLE;
                al.stratptr = Some(s);
                al.vel = 40;
            }
            copy_pos(g, e, idx);
            let al = &mut g.objs.aliens[e as usize];
            al.worldy = vy;
            al.roty = DEG180;
            al.rotz = rz;
        }
    }

    g.objs.aliens[idx as usize].sbyte2 -= 1;
    if g.objs.aliens[idx as usize].sbyte2 != 0 {
        boss8_cont(g, idx);
        return;
    }

    if let Some(e) = make_obj(g, 0) {
        boss8shrap_istrat(g, e);
    }

    boss8_bigexplode(g, idx);
}

/// bigexplode_Istrat (strat_boss8.c:1000).
fn boss8_bigexplode(g: &mut Game, idx: u16) {
    for i in 0..5u8 {
        if let Some(e) = b8_make_exp_obj(g, idx, B8_EXPSHAPE_LARGE) {
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            b8_add_rnd_xy(g, e);
            g.objs.aliens[e as usize].count = i + 1;
            if i == 1 || i == 3 {
                g.objs.aliens[e as usize].sflags2 &= !ASF2_NOEXPSND;
            }
        }
    }
    let s = sid(g, boss8_delayexplode_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 4;
    al.sflags2 |= ASF2_RELEXPLODE;
    al.expstratptr = Some(s);
    al.stratptr = Some(s);
    al.collstratptr = None;
}

// ---- boss8cov (strat_boss8.c:1032) ----

fn boss8cov_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, boss8cov_strat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
        al.hp = HARD_HP;
        al.ap = 10;
    }
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        copy_pos(g, idx, mother);
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldy = NUCLEUS_HEIGHT;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.animframe = 0;
}

fn boss8cov_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };

    if g.objs.aliens[mother as usize].sflags2 & B8_SFLAG4 != 0 {
        g.objs.aliens[idx as usize].shape = SH_BOSS_8_1;
        if g.objs.aliens[idx as usize].animframe != 17 {
            let al = &mut g.objs.aliens[idx as usize];
            al.animframe = al.animframe.wrapping_add(1) % 18;
        } else {
            g.objs.aliens[mother as usize].sflags2 |= B8_SFLAG5;
        }
    } else if g.objs.aliens[idx as usize].animframe != 0 {
        g.objs.aliens[idx as usize].animframe -= 1;
    } else {
        g.objs.aliens[idx as usize].shape = SH_BOSS_8_1C;
        g.objs.aliens[mother as usize].sflags2 &= !B8_SFLAG5;
    }

    let gsv = gsvar_byte1(g);
    let ppz = g.vars.player_posz;
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(gsv);
    al.worldz = (210i16 << BOSS8_SCALE).wrapping_add(ppz);
}

// ---- nucleusbeamL (strat_boss8.c:1097) ----

fn nucleusbeaml_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, nucleusbeaml_strat);
    let s_coll = sid(g, nucleusbeamcol_istrat);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
    al.ap = NUCLEUS_BEAML_AP;
    al.sword2 = BOSS8_CIRC;
    al.type_ &= !ATZREMOVE;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.sbyte3 = 50;
    al.sflags |= ASF_NOHITAFFECT;
}

fn nucleusbeaml_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte4 -= 1;
        al.sflags |= ASF_COLLDISABLE;
    }

    if g.objs.aliens[idx as usize].sbyte3 == 2 {
        play_se(g, 0x71);
    }

    g.objs.aliens[idx as usize].sbyte3 -= 1;
    if g.objs.aliens[idx as usize].sbyte3 != 0 {
        g.objs.aliens[idx as usize].colframe = 4;
    } else {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = 1;
            al.sflags &= !ASF_NOHITAFFECT;
        }

        if g.vars.gameflags & GF_BOSSDEAD != 0 {
            strat_explode(g, idx);
            return;
        }

        if g.objs.aliens[idx as usize].sflags2 & B8_SFLAG1 != 0 {
            g.objs.aliens[idx as usize].colframe = 4;
        } else {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.colframe = (al.colframe + 1) & 3;
            }
            if g.vars.gameframe & 3 == 0 {
                if let Some(e) = make_obj(g, SH_SPARKLAS) {
                    let me = g.objs.aliens[idx as usize];
                    {
                        let al = &mut g.objs.aliens[e as usize];
                        al.sbyte2 = me.sbyte2;
                        al.sword2 = me.sword2;
                    }
                    nucleusbeam_istrat(g, e);
                    copy_pos(g, e, idx);
                    g.objs.aliens[e as usize].roty = me.roty;
                }
            }
        }
    }

    b8_wallrot(g, idx);
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    g.objs.aliens[idx as usize].roty = sb2.wrapping_add(DEG180);
}

fn nucleusbeamcol_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_COLLIDE;
        al.sflags |= ASF_HITFLASH;
        al.sflags2 ^= B8_SFLAG1;
    }
    if g.objs.aliens[idx as usize].sflags2 & B8_SFLAG1 != 0 {
        play_se(g, 0x70);
    } else {
        play_se(g, 0x71);
    }
    g.objs.aliens[idx as usize].sbyte4 = 10;

    if currentlevel(g) == 1 {
        strat_explode(g, idx);
        return;
    }
    nucleusbeaml_strat(g, idx);
}

// ---- nucleusbeam bolts (strat_boss8.c:1203) ----

fn nucleusbeam_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, nucleusbeam_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.type_ |= ATLASER;
    al.type_ &= !ATZREMOVE;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
}

fn nucleusbeam_strat(g: &mut Game, idx: u16) {
    b8_wallrot2(g, idx);
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = sb2.wrapping_add(DEG180);
        al.sword2 = al.sword2.wrapping_sub(50);
    }
    if g.objs.aliens[idx as usize].sword2 < (100 << BOSS8_SCALE) {
        g.objs.aldead = 1;
    }
}

// ---- boss8shrap (strat_boss8.c:1242) ----

fn boss8shrap_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, boss8shrap_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.sflags |= ASF_COLLDISABLE;
    al.type_ &= !ATZREMOVE;
    al.sbyte1 = 50;
}

fn boss8shrap_strat(g: &mut Game, idx: u16) {
    let pvx = pviewposx(g);
    let pvy = pviewposy(g);
    let pvz = pviewposz(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pvx;
        al.worldy = pvy;
        al.worldz = pvz.wrapping_add(1000);
    }

    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        if let Some(e) = b8_make_exp_obj(g, idx, B8_EXPSHAPE_FOLARGE) {
            g.objs.aliens[e as usize].worldz = g.objs.aliens[e as usize].worldz.wrapping_sub(1000);
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            b8_add_rnd2pos_folexp(g, e);
        }
    } else {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
    }

    if g.vars.gameframe & 7 == 0 {
        if let Some(e) = make_obj(g, SH_SHRAP1) {
            // ROM defers `s_set_strat y,shrapfall_Istrat` (GB3STRAT.ASM:486): the
            // Istrat's 3 RNG draws (worldx, sword1, roty) run when the object is
            // next dispatched, AFTER this frame's boss8shrap draws below — finding
            // #17. Mirror the deferred-Istrat convention (like the Shyper handoff)
            // rather than running the init inline this tick.
            let s = sid(g, boss8_shrapfall_istrat);
            g.objs.aliens[e as usize].stratptr = Some(s);
        }
    }

    if g.vars.gameframe & 1 == 0 {
        if let Some(e) = b8_make_exp_obj(g, idx, B8_EXPSHAPE_LARGE) {
            b8_add_rnd_xyz(g, e);
        }
    }

    let bx = (sfrtl_random(g) & 7) as i16;
    let by = ((sfrtl_random(g) & 3) + 248) as i16;
    wm16s_set(g, ebwm::BG2XSCROLL, bx);
    wm16s_set(g, WM_BG2YSCROLL, by);
}

// ---- shrapfall (strat_boss8.c:1309) ----

fn boss8_shrapfall_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, boss8_shrapfall_strat);
    let pvy = pviewposy(g);
    let pvx = pviewposx(g);
    let wx = ((sfrtl_random(g) & 0xFF) as i16).wrapping_sub(128).wrapping_add(pvx);
    let sw1 = (((sfrtl_random(g) & 0xFF) as i16) << 1) as i16;
    let rroty = sfrtl_random(g) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.worldy = pvy.wrapping_sub(500);
    al.sflags |= ASF_COLLDISABLE;
    al.count = 26;
    al.worldx = wx;
    al.sword1 = sw1;
    al.roty = rroty;
}

fn boss8_shrapfall_strat(g: &mut Game, idx: u16) {
    let pvz = pviewposz(g);
    let sw1 = g.objs.aliens[idx as usize].sword1;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = pvz.wrapping_add(300).wrapping_add(sw1);
        al.worldy = al.worldy.wrapping_add(35);
    }
    if count_down(&mut g.objs.aliens[idx as usize]) {
        g.objs.aldead = 1;
    }
}

// ---- nucleuslauncher (strat_boss8.c:1346) ----

fn nucleuslauncher_istrat(g: &mut Game, idx: u16) {
    let sb3 = ((sfrtl_random(g) % 5) + 1) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARD_HP;
        al.ap = NUCLEUS_LAUNCH_AP;
        al.sword2 = BOSS8_CIRC;
        al.type_ &= !ATZREMOVE;
        al.sbyte3 = sb3;
        al.collflags |= COLLTYPE_ENEMYWEAP;
        al.worldy = (-50 << BOSS8_SCALE) + NUCLEUS_HEIGHT;
        if al.shape == 0 {
            al.shape = SH_HOU_4;
        }
    }
    nucleuslauncher_init(g, idx);
    // nucleuslauncher_Istrat falls through init INTO nucleuslauncher_strat in one
    // pass (GASTRATS.ASM:39-51) — run the body on the spawn tick so the first
    // wallrot placement isn't a frame late — finding #28.
    nucleuslauncher_strat(g, idx);
}

fn nucleuslauncher_init(g: &mut Game, idx: u16) {
    let s = sid(g, nucleuslauncher_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.animframe = 0;
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
}

fn nucleuslauncher_strat(g: &mut Game, idx: u16) {
    let player = player(g);
    let fire = if let Some(p) = player.filter(|p| p.active) {
        let me = g.objs.aliens[idx as usize];
        // ROM `s_jmp_objinfront y,x,nuclaunch_cont` (GASTRATS.ASM:60) branches out
        // when player.z >= launcher.z, i.e. arm only while the player is still in
        // front (player.z < launcher.z) — finding #14. And `s_jmp_Xdistmore
        // x,y,#200` (GASTRATS.ASM:63) needs |dx| < 200 (strict) — finding #25.
        p.worldz < me.worldz
            && (me.worldx as i32).abs() >= 700
            && (me.worldx as i32).abs() <= 900
            && (me.worldx as i32 - p.worldx as i32).abs() < 200
    } else {
        false
    };

    if fire {
        let limit = if currentlevel(g) == 1 { 1 } else { 2 };
        if b8_count_kamimissiles(g) < limit {
            g.objs.aliens[idx as usize].sbyte3 -= 1;
            if g.objs.aliens[idx as usize].sbyte3 == 0 {
                let s = sid(g, nucleuslauncherfire_strat);
                g.objs.aliens[idx as usize].stratptr = Some(s);
                nucleuslauncherfire_strat(g, idx);
                return;
            }
        }
    }

    nuclaunch_cont(g, idx);
}

fn nucleuslauncherfire_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = al.animframe.wrapping_add(1) % 4;
    }
    if g.objs.aliens[idx as usize].animframe == 3 {
        if let Some(p) = player_idx(g) {
            if g.objs.aliens[p as usize].active {
                let _ = b8_fire_kamimissile(g, idx, p);
            }
        }
        let s = sid(g, nucleuslauncherclose_strat);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        nucleuslauncherclose_strat(g, idx);
        return;
    }
    nuclaunch_cont(g, idx);
}

fn nucleuslauncherclose_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte3 = 5;
    if g.objs.aliens[idx as usize].animframe != 0 {
        g.objs.aliens[idx as usize].animframe -= 1;
    }
    if g.objs.aliens[idx as usize].animframe != 0 {
        nuclaunch_cont(g, idx);
        return;
    }
    nucleuslauncher_init(g, idx);
    nucleuslauncher_strat(g, idx);
}

fn nuclaunch_cont(g: &mut Game, idx: u16) {
    b8_wallrot(g, idx);
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    g.objs.aliens[idx as usize].roty = sb2.wrapping_add(DEG180);
    if g.vars.gameflags & GF_BOSSDEAD != 0 {
        strat_explode(g, idx);
    }
}

// ---- nucleuspillar (strat_boss8.c:1475) ----

fn nucleuspillar_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, nucleuspillar_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.sword2 = BOSS8_CIRC;
    al.type_ &= !ATZREMOVE;
    al.sflags |= ASF_COLLDISABLE;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    if al.shape == 0 {
        al.shape = SH_BOSS_8_4;
    }
}

fn nucleuspillar_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldy = NUCLEUS_HEIGHT;
    b8_wallrot(g, idx);
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    g.objs.aliens[idx as usize].roty = sb2;
}
// BOSS8_END

// ============================================================
// install / register (C: StratBoss2_Register + StratBossSea_Register +
// StratBoss8_Register, called from Strat_RegisterAll, strat_table.c).
// ============================================================

/// Public strategy handles for the table lane (models `enemy_a::install`).
pub struct BossStratIds {
    /// C `boss2_Istrat` (strat_boss2.c) — IS_BOSS2.
    pub boss2: StratId,
    /// C `seamon_Istrat` (strat_boss_sea.c) — IS_SEAMON.
    pub seamon: StratId,
    /// C `bossseamon_Istrat` — STRAT_ADDR_BOSSSEAMON.
    pub bossseamon: StratId,
    /// C `bossg_istrat` — IS_BOSSG / STRAT_ADDR_BOSSG.
    pub bossg: StratId,
    /// C `boss8_Istrat` — IS_BOSS8 / B8_STRAT_ADDR_BOSS8.
    pub boss8: StratId,
    /// C `nucleusbeamL_Istrat` — IS_NUCLEUSBEAML.
    pub nucleusbeaml: StratId,
    /// C `boss8shrap_Istrat` — IS_BOSS8SHRAP.
    pub boss8shrap: StratId,
    /// C `nucleuslauncher_Istrat` — IS_NUCLEUSLAUNCHER / 0x060015.
    pub nucleuslauncher: StratId,
    /// C `nucleuspillar_Istrat` — IS_NUCLEUSPILLAR / 0x060016.
    pub nucleuspillar: StratId,
}

/// Register this lane's strategy entry points (idempotent — [`sid`]
/// memoizes on function identity) and return the public handles.
pub fn install_bosses(g: &mut Game) -> BossStratIds {
    BossStratIds {
        boss2: sid(g, strat_boss2_init),
        seamon: sid(g, strat_seamon_init),
        bossseamon: sid(g, strat_bossseamon_init),
        bossg: sid(g, strat_bossg_init),
        boss8: sid(g, strat_boss8_init),
        nucleusbeaml: sid(g, nucleusbeaml_istrat),
        boss8shrap: sid(g, boss8shrap_istrat),
        nucleuslauncher: sid(g, nucleuslauncher_istrat),
        nucleuspillar: sid(g, nucleuspillar_istrat),
    }
}

/// Register a strategy on the world registry, deduping by fn identity
/// (World-level twin of [`sid`], used when only the World is available).
fn wsid(world: &mut World, f: StrategyFn) -> StratId {
    if let Some(pos) = world
        .strat_registry
        .iter()
        .position(|&r| r as usize == f as usize)
    {
        StratId(pos as u16)
    } else {
        world.register_strategy(f)
    }
}

/// Table-lane registration entry: populates the boss `g_istrats` rows and
/// the synthetic 0x03/0x06 address map (C `StratBoss2_Register` +
/// `StratBossSea_Register` + `StratBoss8_Register`).
pub fn register(world: &mut World) {
    // ISTRATS.ASM def_Istrat rows.
    world.istrats[IS_SEAMON] = Some(wsid(world, strat_seamon_init));
    world.istrats[IS_BOSSG] = Some(wsid(world, strat_bossg_init));
    world.istrats[IS_BOSS2] = Some(wsid(world, strat_boss2_init));
    world.istrats[IS_NUCLEUSBEAML] = Some(wsid(world, nucleusbeaml_istrat));
    world.istrats[IS_BOSS8SHRAP] = Some(wsid(world, boss8shrap_istrat));
    world.istrats[IS_BOSS8] = Some(wsid(world, strat_boss8_init));
    world.istrats[IS_NUCLEUSLAUNCHER] = Some(wsid(world, nucleuslauncher_istrat));
    world.istrats[IS_NUCLEUSPILLAR] = Some(wsid(world, nucleuspillar_istrat));

    // Synthetic addresses referenced by the MAP2_3 / washmap literal map
    // data (src/map/levels.c).
    let sea = wsid(world, strat_bossseamon_init);
    world.register_strategy_address(STRAT_ADDR_BOSSSEAMON, sea);
    let bg = wsid(world, strat_bossg_init);
    world.register_strategy_address(STRAT_ADDR_BOSSG, bg);
    let b8 = wsid(world, strat_boss8_init);
    world.register_strategy_address(B8_STRAT_ADDR_BOSS8, b8);
    let nl = wsid(world, nucleuslauncher_istrat);
    world.register_strategy_address(B8_STRAT_ADDR_NUCLEUSLAUNCHER, nl);
    let np = wsid(world, nucleuspillar_istrat);
    world.register_strategy_address(B8_STRAT_ADDR_NUCLEUSPILLAR, np);
}

// ============================================================
// Unit tests for the ported local helpers.
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (Game, u16) {
        let mut g = Game::new();
        let idx = g.objs.alloc().expect("pool");
        strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
        (g, idx)
    }

    #[test]
    fn b8_achase_angle_matches_c_shift() {
        // diff = (int8)(0-128) = -128, step = -128>>3 = -16 -> cur = 16.
        let mut cur = 0u8;
        b8_achase_angle(&mut cur, 128, 3);
        assert_eq!(cur, 16);
        // Small positive diff floors to zero -> forced step of 1.
        let mut cur = 5u8;
        b8_achase_angle(&mut cur, 2, 3);
        assert_eq!(cur, 4);
        // Already on target is a no-op.
        let mut cur = 42u8;
        b8_achase_angle(&mut cur, 42, 3);
        assert_eq!(cur, 42);
        // Short-way wrap (8-bit signed diff).
        let mut cur = 250u8;
        b8_achase_angle(&mut cur, 10, 3);
        assert!(cur > 250 || cur < 10, "cur={cur}");
    }

    #[test]
    fn falldown_yvec_gravity_bounce_and_decay() {
        let (mut g, idx) = fresh();
        // Airborne: gravity applied, no clamp, returns false.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vy = 5;
            al.worldy = -100;
        }
        assert!(!boss2_falldown_yvec(&mut g, idx, 2, 2, 0));
        assert_eq!(g.objs.aliens[idx as usize].vy, 7); // 5 + gravity 2
        assert_eq!(g.objs.aliens[idx as usize].worldy, -100); // untouched

        // At/below ground: clamp + bounce vy = (-vy) >> shift.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vy = 60;
            al.worldy = 10; // >= ground 0
        }
        // vy -> 62, worldy clamped to 0, v = (-62)>>2 = -16 (not tiny -> kept).
        assert!(!boss2_falldown_yvec(&mut g, idx, 2, 2, 0));
        assert_eq!(g.objs.aliens[idx as usize].worldy, 0);
        assert_eq!(g.objs.aliens[idx as usize].vy, -16);

        // Tiny bounce ([-5,0]) snaps to 0 and reports decayed.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vy = 0; // +1 gravity -> 1, -1 >> 1 = -1 -> in [-5,0] -> 0
            al.worldy = 5;
        }
        assert!(boss2_falldown_yvec(&mut g, idx, 1, 1, 0));
        assert_eq!(g.objs.aliens[idx as usize].vy, 0);
    }

    #[test]
    fn addyrot2z_folds_signed_yaw() {
        let (mut g, idx) = fresh();
        // roty = 0 -> a = -1-0 = -1, (-1+64)>>4 = 3; worldz += 3.
        g.objs.aliens[idx as usize].roty = 0;
        g.objs.aliens[idx as usize].worldz = 0;
        boss2_addyrot2z(&mut g, idx);
        assert_eq!(g.objs.aliens[idx as usize].worldz, 3);
        // roty = 128 -> a = 128-256 = -128, (-128+64)>>4 = -4; worldz += -4.
        g.objs.aliens[idx as usize].roty = 128;
        g.objs.aliens[idx as usize].worldz = 0;
        boss2_addyrot2z(&mut g, idx);
        assert_eq!(g.objs.aliens[idx as usize].worldz, -4);
    }

    #[test]
    fn wallrot_common_places_and_advances_angle() {
        let (mut g, idx) = fresh();
        g.vars.write_ext8(ebwm::GSVAR_BYTE1, 5);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte2 = 0; // angle 0: sin=0, cos=1
            al.sword2 = 100; // radius
        }
        // x2 = round(100*sin0)=0; z2 = round(100*cos0)=100.
        // worldz = 100*2 + zbase(20) + zref(30) = 250.
        b8_wallrot_common(&mut g, idx, 20, 30);
        assert_eq!(g.objs.aliens[idx as usize].worldx, 0);
        assert_eq!(g.objs.aliens[idx as usize].worldz, 250);
        assert_eq!(g.objs.aliens[idx as usize].sbyte2, 5); // += gsvar_byte1
    }

    #[test]
    fn install_and_register_are_idempotent() {
        let mut g = Game::new();
        let a = install_bosses(&mut g);
        let b = install_bosses(&mut g);
        // Stable ids across calls (sid memoizes on fn identity).
        assert_eq!(a.boss2, b.boss2);
        assert_eq!(a.bossg, b.bossg);
        assert_eq!(a.boss8, b.boss8);
        // Distinct entry points.
        assert_ne!(a.boss2, a.bossg);
        assert_ne!(a.boss8, a.nucleuslauncher);

        register(&mut g.world);
        assert!(g.world.istrats[IS_BOSS2].is_some());
        assert!(g.world.istrats[IS_BOSSG].is_some());
        assert!(g.world.istrats[IS_BOSS8].is_some());
        assert_eq!(
            g.world.find_strategy_address(STRAT_ADDR_BOSSG),
            g.world.istrats[IS_BOSSG]
        );
        assert!(g
            .world
            .find_strategy_address(B8_STRAT_ADDR_NUCLEUSPILLAR)
            .is_some());
    }
}
