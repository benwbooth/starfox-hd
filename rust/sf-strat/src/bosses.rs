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
use sf_game::game::{Game, PosSndFamilyId, StrategyFn};
use sf_game::vars::{
    COLLTYPE_ENEMY1, GF_BOSSDEAD, GF_PLAYERDEAD, GF_PLAYERDYING, GF_STAGEDONE, HARD_AP, HARD_HP,
};
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
    achase_angle, add_player_z, addrnd2pos_xy, boss_attach_child_to_mother, boss_child_from_index_raw,
    boss_clear_child_link, boss_count_children, boss_dying, boss_find_child_obj, boss_get_mother_obj,
    boss_keeprel_to_player, boss_obj_index_or_null, boss_prune_family_links, bossflags, copy_pos,
    currentlevel, ea_random,
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
/// C `s_add_bossHP x,al_hp` (STRATLIB.INC:562): `m_bossHP += al_hp`. Zeroed
/// each frame in init_strats; drives the HUD boss bar (= m_bossHP/bossmaxhp).
#[inline]
fn add_bosshp(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(hp);
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
/// `flingboss` (ISTRATS.ASM:479 def_istrat, macro-counted 58 = sf-map
/// IS_FLINGBOSS / sf-oracle IS_FLINGBOSS_ISTRAT($39)+1).
pub const IS_FLINGBOSS: usize = 58;
/// `deadflingboss` (ISTRATS.ASM:480 def_istrat, macro-counted 59).
pub const IS_DEADFLINGBOSS: usize = 59;
/// `castanet` "Metal Smasher" (ISTRATS.ASM:549 def_istrat, macro-counted 124 =
/// sf-map route2::rc IS_CASTANET, Route 2 L5). Resolves through
/// `world.istrats[124]` exactly like IS_FLINGBOSS (the map spawns a nullshape
/// proxy carrying this ISTRAT index; the two visible cymbal "bits" are the
/// mother's child objects).
pub const IS_CASTANET: usize = 124;

/// `chicken` (ISTRATS.ASM:541 `def_istrat chicken,boss_d_1`, macro-counted 117
/// = sf-map route3::common IS_CHICKEN, Route 3 L3). Resolves through
/// `world.istrats[117]` exactly like IS_FLINGBOSS/IS_CASTANET (the map spawns
/// the SH_BOSS_D_1 body carrying this ISTRAT index; the neck/head/tail
/// segments and wings are the mother's child objects). sf-oracle symbol
/// IS_CHICKEN_ISTRAT=$74(116); +1 = 117.
pub const IS_CHICKEN: usize = 117;

/// `seadragon2` (ISTRATS.ASM:627 `def_istrat seadragon2,snake_1`). Resolves
/// through `world.istrats[197]` — matches sf-map `route3::common::IS_SEADRAGON2
/// = 197` (level3_3.rs spawns SH_SNAKE_1 objects carrying this index). This is
/// the map-placed root of the sprouting sea-dragon neck.
pub const IS_SEADRAGON2: usize = 197;
/// `lochnessmonster` (ISTRATS.ASM:628 `def_istrat lochnessmonster,nullshape`)
/// → `world.istrats[198]`. Not placed by any ported map (the plan flags its
/// reachability as uncertain); registered so a future map + the seadragon
/// head's underwater `.startnextneck` respawn (D2STRATS.ASM:842) resolve it.
pub const IS_LOCHNESS: usize = 198;

/// `seadragon_istrat` synthetic address (sf-map `consts::STRAT_ADDR_SEADRAGON`
/// = 0x03000E, DSTRATS.ASM:1934). The Route-3 L3 `mother_snakes` spawner
/// (mothers.rs:236) fires children at this address — the plain (non-`sbyte2`)
/// sea-dragon variant.
pub const STRAT_ADDR_SEADRAGON: u32 = 0x03000E;

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

    // boss2top .nfire tail: s_add_bossHP x,al_hp (GBSTRATS.ASM:756).
    add_bosshp(g, idx);
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

    // boss2turret .nfire tail: s_add_bossHP x,al_hp (GBSTRATS.ASM:803).
    add_bosshp(g, idx);
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

/// ASM `jsl enemyupsea_l` -> makesnd (positional, POS_ENEMYUPSEA). (F3)
#[inline]
fn sea_enemy_up_sea(g: &mut Game, idx: u16) {
    let al = &g.objs.aliens[idx as usize];
    let (ox, oz) = (al.worldx, al.worldz);
    g.hooks.make_snd(PosSndFamilyId::EnemyUpSea, ox, oz);
}
/// ASM `jsl enemydownsea_l` -> makesnd (positional, POS_ENEMYDOWNSEA). (F4)
#[inline]
fn sea_enemy_down_sea(g: &mut Game, idx: u16) {
    let al = &g.objs.aliens[idx as usize];
    let (ox, oz) = (al.worldx, al.worldz);
    g.hooks.make_snd(PosSndFamilyId::EnemyDownSea, ox, oz);
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
                sea_enemy_down_sea(g, idx);
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
                sea_enemy_down_sea(g, idx);
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
            sea_enemy_down_sea(g, idx);
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
                sea_enemy_down_sea(g, idx);
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
            sea_enemy_down_sea(g, idx);
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
                sea_enemy_down_sea(g, idx);
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
                    sea_enemy_down_sea(g, idx);
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

    sea_enemy_up_sea(g, idx);
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
    sea_enemy_up_sea(g, idx);

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
    // .move2: s_add_bosshp x,al_hp (D2STRATS.ASM:368) precedes add_playerz.
    add_bosshp(g, self_idx);
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

    // boss8_cont tail: s_add_bossHP x,al_hp (GB3STRAT.ASM:130) — runs from
    // wait/a/b ticks and the boss8die countdown branch.
    add_bosshp(g, idx);
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
// FLINGBOSS_BEGIN — "flingboss" + "deadflingboss" (Route 2 L4 armsmap).
//
// ASM oracle: `flingboss_istrat` / `fling` (DSTRATS.ASM:2951-3545) +
// `deadflingboss_istrat` (DSTRATS.ASM:3650-3687), with the shared arm strat
// `arm_istrat` (DSTRATS.ASM:2444-2650) and the arm removal `sprouty.expl`
// (DSTRATS.ASM:2348-2384). ISTRATS.ASM:479/480 (`def_istrat flingboss` /
// `deadflingboss`) → macro-counted indices 58 / 59 (matches sf-map rc.rs
// IS_FLINGBOSS=58 and the sf-oracle IS_*_ISTRAT+1 convention).
//
// SCOPE NOTE (fidelity caveats, cited inline):
//  * The mother state machine, both fire systems, the two-form transition and
//    `deadflingboss` are ported faithfully tick-for-tick.
//  * The two "arms" are modeled as the two DIRECT arm children the mother
//    links via `al_ptr` / `al_sword1` (DSTRATS.ASM:3183/3190). The recursive
//    grabber-tentacle GROWTH (`arm_istrat` `.nbl` -> `.generate`,
//    DSTRATS.ASM:2618-2637/2806-2859) and the inter-segment spring easing
//    (`arm_istrat.position`, DSTRATS.ASM:2860-2944) are NOT reproduced — they
//    reuse the chicken boss's shared code (unported) and need an oracle diff.
//    Consequence: arms are single-segment. To keep phase-1 killable (ROM
//    routes damage through the grabber segments' `.grabberhit` -> `.passiton`
//    which sets the mother's sflag5, DSTRATS.ASM:2752-2791), the direct arm's
//    collstrat sets the mother's sflag5 here — the same observable effect.
// ============================================================

// STRATEQU.INC:60-62 / :978-979, DSTRATS.ASM:58-59.
const FLINGBOSS1HP: u8 = 24;
const FLINGBOSS2HP: u8 = 80;
const FLINGBOSS_AP: u8 = 32;
const FLINGBOSSWIDTH: i16 = 40;
const ARMLENGTH: i16 = 80;
const ARM_AP: u8 = 10; // DSTRATS.ASM:59 armAP
const FB_DEG11: u8 = 8; // VARS.INC:16 deg11 = 256/32

// BOSSHMISSILE1 = fire_Hmissile1 (GSTRATS.ASM:2627): speed 60, life 100,
// hp=hmissile1HP(2), ap=hmissile1AP(8). HPLASMA = fire_Hplasma
// (GSTRATS.ASM:2517): bouncyball, speed 60, life 50, ap HplasmaAP(10).
const FB_HMISSILE_SPEED: u8 = 60;
const FB_HMISSILE_LIFE: u8 = 100;
const FB_HMISSILE_HP: u8 = 2;
const FB_HMISSILE_AP: u8 = 8;
const FB_HPLASMA_SPEED: u8 = 60;
const FB_HPLASMA_LIFE: u8 = 50;
const FB_HPLASMA_AP: u8 = 10;

// Strategy flags (STRATEQU.INC:912-918 layout mapped onto the Rust bytes as
// boss2/enemy_b do): sflag1..4 -> sflags2 0x10/20/40/80; sflag5/6 -> sflags3
// 0x01/0x02.
const FB_SFLAG1: u8 = 0x10;
const FB_SFLAG2: u8 = 0x20;
const FB_SFLAG3: u8 = 0x40;
const FB_SFLAG4: u8 = 0x80;
const FB_SFLAG5: u8 = 0x01; // sflags3 — mother damage latch / missile homing-lock
const FB_SFLAG6: u8 = 0x02; // sflags3 — phase-2 arm marker

// Shape proxies (arm / bulge). The body keeps the map's SH_FLINGBOSS(12).
const SH_FLINGARM_PROXY: u16 = 274;
const SH_FLINGBULGE_PROXY: u16 = 275;

const FB_SE_SPIN: u8 = 0x39; // trigse $39 (spin/whoosh)
const FB_SE_HIT: u8 = 0x2e; // trigse $2e (mother took an arm hit)

/// s_set_objtobealvar-style read of a mother slot holding an object pointer
/// (index+1 encoding, DSTRATS.ASM `s_set_alvartobeobj`). Returns the live idx.
#[inline]
fn fb_read_obj(g: &Game, raw: u16) -> Option<u16> {
    let idx = boss_child_from_index_raw(raw)?;
    if g.objs.aliens[idx as usize].active {
        Some(idx)
    } else {
        None
    }
}

/// Fixed-step chase toward `target` by `rate`, signed-byte compare
/// (Fchase_A, STRATMAC.INC:559-571). No overshoot clamp (rate 1 sites only).
#[inline]
fn fb_fchase(cur: u8, target: i8, rate: i8) -> u8 {
    let c = cur as i8;
    let r = if c == target {
        c
    } else if c < target {
        c.wrapping_add(rate)
    } else {
        c.wrapping_sub(rate)
    };
    r as u8
}

/// `.wavex` (DSTRATS.ASM:3155-3159): worldx = sign_extend(sintab[gameframe])
/// << 1 (s_set_var2vartab B,B,W,...,sintab,1). Absolute, not additive.
fn flingboss_wavex(g: &mut Game, idx: u16) {
    let gf = (g.vars.gameframe & 0xff) as usize;
    let s = crate::snes_trig::SINTAB[gf] as i16;
    g.objs.aliens[idx as usize].worldx = s << 1;
}

// ---- child positioning (DSTRATS.ASM:3202-3343) ----

/// `.positional` (DSTRATS.ASM:3341-3343): child worldpos = mother worldpos +
/// rotate(offset<<2 by mother rotz,rotx,roty). Reuses the verified
/// `b2_full_offset_pos` (s_add_Roffs2pos flags 1,1,1); the <<2 is the trailing
/// `2,2,2` scale args.
fn fb_positional(g: &mut Game, mother: &Alien, child: u16, offx: i16, offy: i16, offz: i16) {
    b2_full_offset_pos(g, child, mother, offx << 2, offy << 2, offz << 2);
}

/// `.position1` (DSTRATS.ASM:3203-3227) — left arm (mother.al_ptr).
fn flingboss_position1(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        // copyrots_yx then roty += deg90 + sbyte3 ; rotx += sbyte2 + rotz.
        c.rotx = m.rotx.wrapping_add(m.sbyte2).wrapping_add(m.rotz);
        c.roty = m.roty.wrapping_add(DEG90).wrapping_add(m.sbyte3);
        c.rotz = m.rotz;
    }
    fb_positional(g, &m, child, -(FLINGBOSSWIDTH - 10), 0, 2);
}

/// `.position2` (DSTRATS.ASM:3228-3251) — right arm (mother.al_sword1).
fn flingboss_position2(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        // roty += -deg90 - sbyte3 ; rotx += sbyte2 - rotz.
        c.rotx = m.rotx.wrapping_add(m.sbyte2).wrapping_sub(m.rotz);
        c.roty = m.roty.wrapping_sub(DEG90).wrapping_sub(m.sbyte3);
        c.rotz = m.rotz;
    }
    fb_positional(g, &m, child, FLINGBOSSWIDTH - 10, 0, 2);
}

/// `.position5` (DSTRATS.ASM:3314-3340) — phase-2 single arm.
fn flingboss_position5(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        // rotx += -sbyte2 + deg11 - rotz ; roty += -sbyte3 ; rotz += -deg90.
        c.rotx = m
            .rotx
            .wrapping_sub(m.sbyte2)
            .wrapping_add(FB_DEG11)
            .wrapping_sub(m.rotz);
        c.roty = m.roty.wrapping_sub(m.sbyte3);
        c.rotz = m.rotz.wrapping_sub(DEG90);
    }
    fb_positional(g, &m, child, 0, 10, 20);
}

// ---- projectiles ----

/// hmissile1_strat (GSTRATS.ASM:1459-1495): continuous homing toward al_ptr
/// (the player) at >>3, locks (stops homing) once within 300, spins rotz.
fn flingboss_hmissile1_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(10);
    if g.objs.aliens[idx as usize].sflags2 & FB_SFLAG2 == 0 {
        let me = g.objs.aliens[idx as usize];
        let target = fb_read_obj(g, me.ptr);
        if let Some(t) = target {
            let tt = g.objs.aliens[t as usize];
            if crate::common::strat_dist_xz(&me, &tt) < 300 {
                g.objs.aliens[idx as usize].sflags2 |= FB_SFLAG2; // .nac lock
            } else {
                let want_yaw = strat_angle_xz(&me, &tt);
                let want_pitch = strat_pitch_toward(&me, &tt);
                let dyaw = me.roty.wrapping_sub(want_yaw) as i8;
                let dpitch = me.rotx.wrapping_sub(want_pitch) as i8;
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = me.roty.wrapping_sub((dyaw >> 3) as u8);
                al.rotx = me.rotx.wrapping_sub((dpitch >> 3) as u8);
            }
        }
        strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// `.missile` (DSTRATS.ASM:3396-3401): fire BOSSHMISSILE1 from muzzle
/// (0, 20>>ws<<ws = 20, 0) rotated by the mother, initial rots = mother rots +
/// the s_weapon_rot (pitch,yaw) offsets, then homes the player.
fn flingboss_fire_missile(g: &mut Game, idx: u16, wr_pitch: u8, wr_yaw: u8) {
    let me = g.objs.aliens[idx as usize];
    let pitch = me.rotx.wrapping_add(wr_pitch);
    let yaw = me.roty.wrapping_add(wr_yaw);
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        FB_HMISSILE_SPEED,
        FB_HMISSILE_LIFE,
        FB_HMISSILE_AP,
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    b2_full_offset_pos(g, shot, &me, 0, 20, 0);
    let s = sid(g, flingboss_hmissile1_strat);
    let ppt = player_idx(g).map(boss_obj_index_or_null).unwrap_or(0);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.sflags &= !ASF_INVISIBLE;
    al.sflags |= ASF_SHADOW;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.hp = FB_HMISSILE_HP;
    al.ptr = ppt; // al_ptr = playpt (homing target)
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratptr = Some(s);
    strat_gen_vecs_3d(al);
}

/// `.triggermissile2` (DSTRATS.ASM:3370-3374): 1 BOSSHMISSILE1 every 64
/// frames (s_jmp_notdelay 6 -> gf&63==0), s_weapon_rot #deg11,#0.
fn flingboss_triggermissile2(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 63 != 0 {
        return;
    }
    flingboss_fire_missile(g, idx, FB_DEG11, 0);
}

/// `.triggermissile3` (DSTRATS.ASM:3376-3395): phase-2 fire, every 64 frames.
/// hp >= 40 -> 2 missiles at ±(deg45+deg22); hp < 40 (`.harder`) -> 4 missiles.
fn flingboss_triggermissile3(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 63 != 0 {
        return;
    }
    if (g.objs.aliens[idx as usize].hp as i8) < (FLINGBOSS2HP as i8) / 2 {
        // .harder
        flingboss_fire_missile(g, idx, FB_DEG11, DEG45);
        flingboss_fire_missile(g, idx, FB_DEG11, (-(DEG45 as i8)) as u8);
        flingboss_fire_missile(g, idx, 0, 0);
        flingboss_fire_missile(g, idx, DEG22, 0);
    } else {
        let off = DEG45.wrapping_add(DEG22); // deg45+deg22 = 48
        flingboss_fire_missile(g, idx, FB_DEG11, off);
        flingboss_fire_missile(g, idx, FB_DEG11, (-(off as i8)) as u8);
    }
}

/// arm `.fire2` (DSTRATS.ASM:2599-2605): fire HPLASMA (homingflat) from the
/// arm tip, homing the player. Reuses `boss2_hplasma_strat` (= homingflat_Istrat,
/// GSTRATS.ASM:1723). Muzzle z = ((armlength-5)>>ws)<<1 then <<ws (fire_weapon).
fn flingboss_arm_fire_hplasma(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        me.rotx,
        me.roty,
        FB_HPLASMA_SPEED,
        FB_HPLASMA_LIFE,
        FB_HPLASMA_AP,
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    let muzzle_z = (((ARMLENGTH - 5) >> 2) << 1) << 2; // = 144
    b2_full_offset_pos(g, shot, &me, 0, 0, muzzle_z);
    let s = sid(g, boss2_hplasma_strat);
    let ppt = player_idx(g).map(boss_obj_index_or_null).unwrap_or(0);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = me.rotz;
    al.collflags |= COLLTYPE_ENEMY1;
    al.ptr = ppt;
    al.fireobjptr = ppt; // boss2_hplasma_strat homes via fireobjptr
    al.stratptr = Some(s);
    strat_gen_vecs_3d(al);
}

// ---- arm (scoped arm_istrat for shape #arm) ----

/// arm_istrat init (DSTRATS.ASM:2444-2453) for the flingboss arm shapes only.
fn flingboss_arm_init(g: &mut Game, idx: u16) {
    let s_strat = sid(g, flingboss_arm_strat);
    let s_col = sid(g, flingboss_arm_col);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s_strat);
    al.collstratptr = Some(s_col);
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP; // armHP = hardHP (invulnerable body; damage routes to mother)
    al.ap = ARM_AP;
    al.sbyte1 = 0;
    al.sbyte2 = 0;
    al.sbyte3 = 0;
    al.sbyte4 = 32;
    al.collflags |= COLLTYPE_ENEMY1;
    al.type_ &= !ATZREMOVE; // s_clr_altype x,zremove
    // arm_istrat falls into .strat the same tick (s_start_strat re-entry).
    flingboss_arm_strat(g, idx);
}

/// arm collstrat — mirrors `.grabberhit` -> `.passiton` (DSTRATS.ASM:2752-2791,
/// sflag1-clear path): a hit on the arm latches the mother's sflag5.
fn flingboss_arm_col(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_COLLIDE;
        al.sflags |= ASF_HITFLASH;
    }
    if let Some(m) = fb_read_obj(g, g.objs.aliens[idx as usize].childrotobj) {
        g.objs.aliens[m as usize].sflags3 |= FB_SFLAG5;
    }
}

/// arm_istrat `.strat` (DSTRATS.ASM:2558-2650) scoped to shape #arm: the
/// sflag2-triggered bulge windup + HPLASMA fire. Positioning is driven by the
/// mother (`.keepitlinked`). The `.nbl` grabber-growth branch is scoped out.
fn flingboss_arm_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & FB_SFLAG2 != 0 {
        if g.objs.aliens[idx as usize].sflags2 & FB_SFLAG3 == 0 {
            // .nx: begin bulge.
            let al = &mut g.objs.aliens[idx as usize];
            al.animframe = 0;
            al.shape = SH_FLINGBULGE_PROXY;
            al.sflags2 |= FB_SFLAG3;
        } else {
            // .b: advance (dincanim #5), fire at frame 4 (.nxtone -> .fire2).
            let al = &mut g.objs.aliens[idx as usize];
            al.animframe = (al.animframe + 1) % 5;
            if al.animframe == 4 {
                al.sflags2 &= !(FB_SFLAG2 | FB_SFLAG3);
                al.shape = SH_FLINGARM_PROXY;
                flingboss_arm_fire_hplasma(g, idx);
            }
        }
    }
    // .nbl sbyte4 idle counter (decbne -> reset 32). The recursive `.generate`
    // that this gates (DSTRATS.ASM:2637) is scoped out — see the section note.
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte4 = al.sbyte4.wrapping_sub(1);
    if al.sbyte4 == 0 {
        al.sbyte4 = 32;
    }
}

/// arm-fall init — the `sprouty.expl` -> `.fall_istrat` handoff
/// (DSTRATS.ASM:2385-2394): the pulled-off arm gets a random spin, velocity and
/// falls until it detonates. Simplified single segment (no chain).
fn flingboss_arm_fall_init(g: &mut Game, arm: u16) {
    let s = sid(g, flingboss_arm_fall_strat);
    let r = (sfrtl_random(g) & 0xff) as u8;
    {
        let al = &mut g.objs.aliens[arm as usize];
        al.vel = 30;
        al.roty = r; // s_set_alvar2rnd x,al_roty
    }
    strat_gen_vecs_3d(&mut g.objs.aliens[arm as usize]);
    let al = &mut g.objs.aliens[arm as usize];
    al.vy = -10;
    al.stratptr = Some(s);
    al.collstratptr = None;
}

/// `.fall` (DSTRATS.ASM:2394-2411): spin + gravity, detonate on landing.
fn flingboss_arm_fall_strat(g: &mut Game, arm: u16) {
    {
        let al = &mut g.objs.aliens[arm as usize];
        al.rotz = al.rotz.wrapping_add(5);
        al.rotx = al.rotx.wrapping_add(2);
    }
    let landed = boss2_falldown_yvec(g, arm, 2, 3, 0);
    strat_apply_velocity(&mut g.objs.aliens[arm as usize]);
    if landed {
        // .le_fin: s_set_expstrat explode, s_kill_obj -> explode.
        strat_explode(g, arm);
    }
}

// ---- mother spawn/generate ----

/// fling.makeobj (DSTRATS.ASM:3195-3200): allocate an arm shell.
fn flingboss_makeobj(g: &mut Game, mother: u16) -> Option<u16> {
    let arm = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[arm as usize]);
    g.objs.aliens[arm as usize].shape = SH_FLINGARM_PROXY;
    copy_pos(g, arm, mother);
    Some(arm)
}

/// `.generate` (DSTRATS.ASM:3177-3193): spawn the two arms. Returns true when
/// both exist (carry-clear path).
fn flingboss_generate(g: &mut Game, mother: u16) -> bool {
    if fb_read_obj(g, g.objs.aliens[mother as usize].ptr).is_none() {
        let Some(a1) = flingboss_makeobj(g, mother) else {
            return false;
        };
        let s = sid(g, flingboss_arm_init);
        {
            let al = &mut g.objs.aliens[a1 as usize];
            al.stratptr = Some(s);
            al.sword1 = 5;
            al.childrotobj = boss_obj_index_or_null(mother);
        }
        g.objs.aliens[mother as usize].ptr = boss_obj_index_or_null(a1);
        flingboss_position1(g, mother, a1);
    }
    let Some(a2) = flingboss_makeobj(g, mother) else {
        return false;
    };
    let s = sid(g, flingboss_arm_init);
    {
        let al = &mut g.objs.aliens[a2 as usize];
        al.stratptr = Some(s);
        al.sword1 = 5;
        al.childrotobj = boss_obj_index_or_null(mother);
    }
    g.objs.aliens[mother as usize].sword1 = boss_obj_index_or_null(a2) as i16;
    flingboss_position2(g, mother, a2);
    true
}

/// `.generate3` (DSTRATS.ASM:3535-3545): spawn the single phase-2 arm.
fn flingboss_generate3(g: &mut Game, mother: u16) -> bool {
    let Some(arm) = flingboss_makeobj(g, mother) else {
        return false;
    };
    let s = sid(g, flingboss_arm_init);
    {
        let al = &mut g.objs.aliens[arm as usize];
        al.stratptr = Some(s);
        al.sword1 = 6;
        al.sflags3 |= FB_SFLAG6;
        al.childrotobj = boss_obj_index_or_null(mother);
    }
    g.objs.aliens[mother as usize].ptr = boss_obj_index_or_null(arm);
    flingboss_position5(g, mother, arm);
    true
}

/// `.pullthearmsoff` (DSTRATS.ASM:3403-3414): fling both arms off to fall +
/// explode, unlinking them from the mother.
fn flingboss_pullthearmsoff(g: &mut Game, mother: u16) {
    if let Some(a1) = fb_read_obj(g, g.objs.aliens[mother as usize].ptr) {
        flingboss_arm_fall_init(g, a1);
    }
    if let Some(a2) = fb_read_obj(g, g.objs.aliens[mother as usize].sword1 as u16) {
        flingboss_arm_fall_init(g, a2);
    }
    g.objs.aliens[mother as usize].ptr = 0;
    g.objs.aliens[mother as usize].sword1 = 0;
}

// ---- mother fire trigger (arm HPLASMA) ----

#[inline]
fn fb_sword2_lo(g: &Game, idx: u16) -> u8 {
    (g.objs.aliens[idx as usize].sword2 as u16) as u8
}
#[inline]
fn fb_sword2_hi(g: &Game, idx: u16) -> u8 {
    ((g.objs.aliens[idx as usize].sword2 as u16) >> 8) as u8
}
#[inline]
fn fb_set_sword2_lo(g: &mut Game, idx: u16, v: u8) {
    let hi = fb_sword2_hi(g, idx);
    g.objs.aliens[idx as usize].sword2 = (((hi as u16) << 8) | v as u16) as i16;
}
#[inline]
fn fb_set_sword2_hi(g: &mut Game, idx: u16, v: u8) {
    let lo = fb_sword2_lo(g, idx);
    g.objs.aliens[idx as usize].sword2 = (((v as u16) << 8) | lo as u16) as i16;
}

/// `.triggermissile` (DSTRATS.ASM:3349-3368): staggered arm-fire cadence.
/// sword2 hi = 5-frame reload, lo = 3-shot burst, then a ~1.6% random re-arm.
fn flingboss_triggermissile(g: &mut Game, idx: u16) {
    // s_beqdec_alvar sword2+1 -> .firem
    if fb_sword2_hi(g, idx) != 0 {
        let hi = fb_sword2_hi(g, idx).wrapping_sub(1);
        fb_set_sword2_hi(g, idx, hi);
        return;
    }
    // .firem: s_beqdec_alvar sword2 -> .rndbit
    if fb_sword2_lo(g, idx) == 0 {
        // .rndbit: s_jmp_random .no,99 — branch out ~98.4% of the time.
        if (sfrtl_random(g) as u8) < ((99u16 * 255 / 100) as u8) {
            return; // .no
        }
        fb_set_sword2_lo(g, idx, 3);
        g.objs.aliens[idx as usize].sflags2 ^= FB_SFLAG4; // alternate arm
        return;
    }
    let lo = fb_sword2_lo(g, idx).wrapping_sub(1);
    fb_set_sword2_lo(g, idx, lo);
    // fire: trigger the current arm (sflag4 selects sword1-arm vs ptr-arm).
    let arm = if g.objs.aliens[idx as usize].sflags2 & FB_SFLAG4 != 0 {
        fb_read_obj(g, g.objs.aliens[idx as usize].sword1 as u16)
    } else {
        fb_read_obj(g, g.objs.aliens[idx as usize].ptr)
    };
    if let Some(a) = arm {
        g.objs.aliens[a as usize].sflags2 |= FB_SFLAG2;
    }
    fb_set_sword2_hi(g, idx, 4);
}

// ---- mother movement ----

/// `.movebackandforth` (DSTRATS.ASM:3126-3154): oscillate between z-distance
/// ~500 (advance) and ~2000 (retreat), chasing rotx and worldy to the player.
fn flingboss_movebackandforth(g: &mut Game, idx: u16) {
    flingboss_wavex(g, idx);
    let Some(pl) = player_idx(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    if me.sflags2 & FB_SFLAG3 != 0 {
        // retreat
        let py = g.objs.aliens[pl as usize].worldy.wrapping_add(100);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(20);
        al.rotx = fb_fchase(al.rotx, 0, 1);
        al.worldy = chase_proportional(al.worldy, py, 4);
        if !sea_dz_less(g, idx, 2000) {
            g.objs.aliens[idx as usize].sflags2 ^= FB_SFLAG3; // .notflag
        }
    } else {
        // advance
        let py = g.objs.aliens[pl as usize].worldy.wrapping_sub(250);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(40);
        al.rotx = fb_fchase(al.rotx, 20, 1);
        al.worldy = chase_proportional(al.worldy, py, 4);
        if sea_dz_less(g, idx, 500) {
            g.objs.aliens[idx as usize].sflags2 ^= FB_SFLAG3; // .notflag
        }
    }
}

// ---- mother state machine ----

/// `.keepitlinked` tail (DSTRATS.ASM:3021-3029): position both arms +
/// accumulate the HP bar (s_add_bossHP x,al_sbyte4,#flingboss2HP).
fn flingboss_keepitlinked(g: &mut Game, idx: u16) {
    if let Some(a1) = fb_read_obj(g, g.objs.aliens[idx as usize].ptr) {
        flingboss_position1(g, idx, a1);
    }
    if let Some(a2) = fb_read_obj(g, g.objs.aliens[idx as usize].sword1 as u16) {
        flingboss_position2(g, idx, a2);
    }
    let sb4 = g.objs.aliens[idx as usize].sbyte4 as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(sb4 + FLINGBOSS2HP as u16);
}

/// `.fin` common body (DSTRATS.ASM:3008-3029): colanim, sflag5 damage (drains
/// sbyte4 by 2 -> phase 2 at 0), movement, both fire systems, arm positioning.
fn flingboss_fin_body(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].colframe = (g.objs.aliens[idx as usize].colframe + 1) & 3;
    if g.objs.aliens[idx as usize].sflags3 & FB_SFLAG5 != 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags |= ASF_HITFLASH;
            al.sflags3 &= !FB_SFLAG5;
        }
        play_se(g, FB_SE_HIT);
        // Two s_beqdec_alvar sbyte4 -> .crazy2 (branch when already 0).
        for _ in 0..2 {
            if g.objs.aliens[idx as usize].sbyte4 == 0 {
                return flingboss_crazy2(g, idx);
            }
            g.objs.aliens[idx as usize].sbyte4 -= 1;
        }
    }
    add_player_z(g, idx);
    flingboss_movebackandforth(g, idx);
    flingboss_triggermissile(g, idx);
    flingboss_triggermissile2(g, idx);
    flingboss_keepitlinked(g, idx);
}

/// `.mainstrat` (DSTRATS.ASM:3004-3006): sbyte1 countdown into the fling arc.
fn flingboss_main(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        return flingboss_spinlikecrazy(g, idx);
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    flingboss_fin_body(g, idx);
}

/// `.initmain` / `.backforabit` (DSTRATS.ASM:3000-3004): enter the main state.
fn flingboss_initmain(g: &mut Game, idx: u16) {
    let s = sid(g, flingboss_main);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].sbyte1 = 80;
    flingboss_main(g, idx);
}

/// `.spinlikecrazy` (DSTRATS.ASM:3031-3037): begin the spin state.
fn flingboss_spinlikecrazy(g: &mut Game, idx: u16) {
    let s = sid(g, flingboss_spin);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 ^= FB_SFLAG2; // toggle (drives the dead .otherstuff branch)
    al.sbyte1 = 100;
    al.stratptr = Some(s);
    al.sflags2 ^= FB_SFLAG1; // toggle spin direction
    flingboss_spin(g, idx);
}

/// `.spin` (DSTRATS.ASM:3037-3055): spin roty ±8 for ~100 ticks.
fn flingboss_spin(g: &mut Game, idx: u16) {
    let dir_add = g.objs.aliens[idx as usize].sflags2 & FB_SFLAG1 != 0;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = if dir_add {
            al.roty.wrapping_add(8)
        } else {
            al.roty.wrapping_sub(8)
        };
    }
    let roty = g.objs.aliens[idx as usize].roty;
    if roty == 64 || roty == 192 {
        play_se(g, FB_SE_SPIN);
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        // .cmp
        if roty == DEG180 {
            return flingboss_wavearmsabout(g, idx);
        }
        return flingboss_fin_body(g, idx);
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    flingboss_fin_body(g, idx);
}

/// `.wavearmsabout` (DSTRATS.ASM:3056-3059): enter the wave state.
fn flingboss_wavearmsabout(g: &mut Game, idx: u16) {
    let s = sid(g, flingboss_wave);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].sbyte1 = 50;
    flingboss_wave(g, idx);
}

/// `.setvar2tab` (DSTRATS.ASM:3072-3074): sbyte2-scale = sintab[(gf*2)&0xff]
/// with scale -2 (adiv2 twice, toward zero).
#[inline]
fn fb_sintab_gf2_scale2(gf: u16) -> i16 {
    let off = (gf.wrapping_mul(2) & 0xff) as usize;
    let v = crate::snes_trig::SINTAB[off] as i16;
    (v / 2) / 2
}

/// `.wave` (DSTRATS.ASM:3059-3071): undulate sbyte2 from the sine table.
fn flingboss_wave(g: &mut Game, idx: u16) {
    let v = fb_sintab_gf2_scale2(g.vars.gameframe);
    g.objs.aliens[idx as usize].sbyte2 = v as u8;
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        // .chkfin
        if g.objs.aliens[idx as usize].sbyte2 == 0 {
            return flingboss_wavearmsforward(g, idx);
        }
        return flingboss_fin_body(g, idx);
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    flingboss_fin_body(g, idx);
}

/// `.wavearmsforward` (DSTRATS.ASM:3075-3078): enter the wave2 state.
fn flingboss_wavearmsforward(g: &mut Game, idx: u16) {
    let s = sid(g, flingboss_wave2);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].sbyte1 = 140;
    flingboss_wave2(g, idx);
}

/// `.wave2` (DSTRATS.ASM:3078-3089): sweep sbyte3 down to -52.
fn flingboss_wave2(g: &mut Game, idx: u16) {
    let sb3 = g.objs.aliens[idx as usize].sbyte3 as i8;
    if sb3 == -24 {
        play_se(g, FB_SE_SPIN);
    }
    if sb3 != -52 {
        g.objs.aliens[idx as usize].sbyte3 = sb3.wrapping_sub(4) as u8;
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        return flingboss_sidetoside(g, idx);
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    flingboss_fin_body(g, idx);
}

/// `.sidetoside` (DSTRATS.ASM:3092-3095): enter the side-sway state.
fn flingboss_sidetoside(g: &mut Game, idx: u16) {
    let s = sid(g, flingboss_side);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].sbyte1 = 180;
    flingboss_side(g, idx);
}

/// `.side` (DSTRATS.ASM:3095-3114): roty sways ±(sintab/8) around deg180.
fn flingboss_side(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        return flingboss_resetside(g, idx);
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    let v = fb_sintab_gf2_scale2(g.vars.gameframe) / 2; // extra adiv2 (total /8)
    g.objs.aliens[idx as usize].roty = (v + DEG180 as i16) as u8;
    flingboss_fin_body(g, idx);
}

/// `.resetside` (DSTRATS.ASM:3111-3114): reset then loop back to main.
fn flingboss_resetside(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte3 = 0;
    al.roty = DEG180;
    flingboss_initmain(g, idx);
}

// ---- phase 2 (arms pulled off, spinning dying form) ----

/// `.crazy2` (DSTRATS.ASM:3479-3490): pull the arms off then transform.
fn flingboss_crazy2(g: &mut Game, idx: u16) {
    flingboss_pullthearmsoff(g, idx);
    flingboss_andagainif(g, idx);
}

/// `.andagainif` (DSTRATS.ASM:3480-3490): set up the phase-2 form (damageable,
/// hp=flingboss2HP, expstrat=deadflingboss) and spawn the phase-2 arm.
fn flingboss_andagainif(g: &mut Game, idx: u16) {
    let s_body = sid(g, flingboss_almostgone2);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, flingboss_deadflingboss_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_body);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.sflags &= !ASF_NOHITAFFECT; // now directly damageable
        al.hp = FLINGBOSS2HP;
        al.ap = FLINGBOSS_AP;
        al.sword1 = 80;
        al.sbyte2 = 0;
        al.sbyte3 = 0;
    }
    if flingboss_generate3(g, idx) {
        flingboss_almostgone2(g, idx);
    } else {
        let s = sid(g, flingboss_andagainif);
        g.objs.aliens[idx as usize].stratptr = Some(s); // retry next tick
    }
}

/// `.almostgone2` (DSTRATS.ASM:3491-3533): recede then spin faster as it dies.
fn flingboss_almostgone2(g: &mut Game, idx: u16) {
    {
        // s_add_colanim x,#1,#8,NOJUMP,#4 (cap 8, wrap to firstframe 4).
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.colframe + 1;
        al.colframe = if c >= 8 { 4 } else { c };
        // dincanimjmp_x #9 (cosmetic body anim wrap).
        al.animframe = (al.animframe + 1) % 9;
    }
    add_player_z(g, idx);
    let sw1 = g.objs.aliens[idx as usize].sword1;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(sw1);
    // s_beqdec_alvar sword1 -> .backfoth ; else s_dec (total -2/tick).
    if g.objs.aliens[idx as usize].sword1 == 0 {
        flingboss_movebackandforth(g, idx); // .backfoth
        return flingboss_almostgone2_spinning(g, idx);
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    g.objs.aliens[idx as usize].sword1 -= 1;
    flingboss_wavex(g, idx);
    flingboss_almostgone2_spinning(g, idx);
}

/// `.spinning` (DSTRATS.ASM:3503-3530): roty += f(hp), position the arm, fire.
fn flingboss_almostgone2_spinning(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    // Sound gate on the pre-increment roty (cosmetic).
    if (72..96).contains(&me.roty) || (200..224).contains(&me.roty) {
        play_se(g, FB_SE_SPIN);
    }
    // roty += (255 - ((hp>>2)&0xF8)) + 19 + bit1(hp) — spins faster at low hp.
    let hp = me.hp;
    let a1 = (hp >> 2) & 0xF8;
    let carry = (hp >> 1) & 1;
    let delta = (255u8.wrapping_sub(a1)).wrapping_add(19).wrapping_add(carry);
    g.objs.aliens[idx as usize].roty = me.roty.wrapping_add(delta);
    if let Some(arm) = fb_read_obj(g, g.objs.aliens[idx as usize].ptr) {
        flingboss_position5(g, idx, arm);
    }
    flingboss_triggermissile3(g, idx);
    let hp = g.objs.aliens[idx as usize].hp as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(hp);
}

// ---- deadflingboss (post-kill sink + explode) ----

/// `deadflingboss_istrat` init (DSTRATS.ASM:3650-3663): reached as the mother's
/// expstrat when phase-2 hp hits 0. Blows off the last arm, then sinks away.
fn flingboss_deadflingboss_init(g: &mut Game, idx: u16) {
    if let Some(arm) = fb_read_obj(g, g.objs.aliens[idx as usize].ptr) {
        flingboss_arm_fall_init(g, arm); // sprouty.expl on the tentacle
    }
    g.objs.aliens[idx as usize].ptr = 0;
    let s = sid(g, flingboss_deadflingboss_strat);
    let s_exp = sid(g, strat_boss_explode_init); // bossexplode_istrat
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = DEG180;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.stratptr = Some(s);
    al.expstratptr = Some(s_exp);
    al.sword1 = 161;
    flingboss_deadflingboss_strat(g, idx);
}

/// `deadflingboss` `.strat`/`.strat2` (DSTRATS.ASM:3664-3687): wait for the
/// player within 100, recede while tilting back, then kill -> bossexplode.
fn flingboss_deadflingboss_strat(g: &mut Game, idx: u16) {
    // `.strat` gate is a one-shot: sword1==161 means "not yet moving".
    if g.objs.aliens[idx as usize].sword1 == 161 {
        if !sea_dz_less(g, idx, 100) {
            return; // dz >= 100 -> wait (.end)
        }
        // fall through to .strat2 the same tick.
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].sword1 -= 4;
    if g.objs.aliens[idx as usize].sword1 < 40 {
        // .die
        let gf = g.vars.gameflags;
        if gf & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0 {
            return; // wait while the player is dying
        }
        // s_kill_obj -> hp0 + colldisable -> engine fires bossexplode expstrat.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags |= ASF_COLLDISABLE;
            al.hp = 0;
        }
        if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
            g.call_strat(exp, idx);
        }
        return;
    }
    let sw1 = g.objs.aliens[idx as usize].sword1;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(sw1);
    al.rotx = al.rotx.wrapping_add((-20i8) as u8);
}

/// `flingboss_istrat` init (DSTRATS.ASM:2951-2973).
pub fn strat_flingboss_init(g: &mut Game, idx: u16) {
    g.vars.gameflags &= !GF_BOSSDEAD;
    let bf = bossflags(g);
    set_bossflags(g, bf & !BF_DYING);
    set_bossmaxhp(g, (FLINGBOSS1HP + FLINGBOSS2HP) as u16); // 104
    g.vars.meters = 1;

    let s_strat = sid(g, flingboss_approach);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.colframe = 0;
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.sflags |= ASF_NOHITAFFECT; // body invulnerable in phase 1
        al.hp = HARD_HP;
        al.ap = FLINGBOSS_AP;
        al.sbyte4 = FLINGBOSS1HP; // 24 — phase-1 hit reserve
        al.roty = DEG180;
        al.sword2 = 0;
        al.collflags |= COLLTYPE_ENEMY1;
        al.worldz = al.worldz.wrapping_add(4000);
        al.stratstate = 0;
    }

    if flingboss_generate(g, idx) {
        // `.created` -> falls into `.strat` the same tick.
        flingboss_approach(g, idx);
    } else {
        // Retry the whole init next tick (s_set_strat x,flingboss_istrat).
        let s = sid(g, strat_flingboss_init);
        g.objs.aliens[idx as usize].stratptr = Some(s);
    }
}

/// `.strat` approach (DSTRATS.ASM:2974-2998): tumble in on rotx until close +
/// upright, then hand off to `.mainstrat`.
fn flingboss_approach(g: &mut Game, idx: u16) {
    let old = g.objs.aliens[idx as usize].rotx;
    let newrx = old.wrapping_add(DEG22); // += deg22 (16)
    g.objs.aliens[idx as usize].rotx = newrx;
    if (newrx ^ old) & 0x80 != 0 {
        play_se(g, FB_SE_SPIN);
    }
    g.objs.aliens[idx as usize].worldz -= 50;

    if sea_dz_less(g, idx, 2000) {
        // .chk: within 2000 — upright frame hands off to main.
        if g.objs.aliens[idx as usize].rotx == 0 {
            return flingboss_initmain(g, idx);
        }
    }
    // .nochk
    add_player_z(g, idx);
    flingboss_wavex(g, idx);
    flingboss_keepitlinked(g, idx);
}
// FLINGBOSS_END

// ============================================================
// GROUNDVEHICLE_BEGIN — shared ground-vehicle base (build-once infra).
//
// ASM oracle: `trucklaunch_istrat` (DSTRATS.ASM:6260-6310) and
// `fallingtruck_istrat` (DSTRATS.ASM:6313-6336). These two strats are the
// road/ground-lane vehicle base shared between castanet (which turns its
// surviving cymbal-bit into a launching truck on death, DSTRATS.ASM:6094)
// and the still-unported `madtrucker` (DSTRATS.ASM:5233, which reuses
// `trucklaunch_istrat`/`fallingtruck_istrat` verbatim per
// docs/UNPORTED_BOSSES_PLAN.md §4). They are placed here, above castanet, so
// the madtrucker port can call `trucklaunch_init` / `fallingtruck_init`
// directly.
//
// The lower-level ground-lane primitives it composes already exist and are
// reused as-is: `add_player_z` (s_add_playerZ, world scroll),
// `strat_gen_vecs_3d` (dgen3dvecs), `strat_apply_velocity` (daddvecs2pos_x).
// The only genuinely new shared piece is the two vehicle strats + the 16-bit
// fixed chase `gv_fchase16` (s_fchase_alvar W).
// ============================================================

/// s_fchase_alvar W (STRATMAC.INC Fchase_var2A, 16-bit): step `cur` toward
/// `target` by a fixed `rate`, clamping so it never overshoots the target.
fn gv_fchase16(cur: i16, target: i16, rate: i16) -> i16 {
    if cur == target {
        cur
    } else if cur < target {
        let n = cur.wrapping_add(rate);
        if n > target {
            target
        } else {
            n
        }
    } else {
        let n = cur.wrapping_sub(rate);
        if n < target {
            target
        } else {
            n
        }
    }
}

/// s_beqdec_alvar B (STRATMAC.INC): if the byte is 0 return true WITHOUT
/// touching it (the `beq` fires before the `dec`); otherwise decrement and
/// return false.
#[inline]
fn gv_beqdec(v: &mut u8) -> bool {
    if *v == 0 {
        true
    } else {
        *v -= 1;
        false
    }
}

/// `trucklaunch_istrat` init (DSTRATS.ASM:6260-6266). Entered as the strat of
/// castanet's surviving bit (or, later, a madtrucker truck) — the object
/// slides to its launch spot, tilts to the launch angle, spawns a
/// `fallingtruck` and self-destructs into `bossexplode`.
pub fn trucklaunch_init(g: &mut Game, idx: u16) {
    let s = sid(g, trucklaunch_strat);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_boss_explode_init); // bossexplode_istrat
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.sbyte4 = al.hp; // s_copy_alvar2alvar al_sbyte4,al_hp (save bar hp)
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_COLLDISABLE;
        al.sbyte1 = 70;
    }
    // init falls into .strat the same tick.
    trucklaunch_strat(g, idx);
}

/// `trucklaunch_istrat` `.strat` (DSTRATS.ASM:6267-6281): fchase to the launch
/// spot (-150,-200), advance with the world, count down 70 ticks.
fn trucklaunch_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = gv_fchase16(al.worldx, -150, 8);
        al.worldy = gv_fchase16(al.worldy, -200, 8);
    }
    if sea_dz_less(g, idx, 2000) {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(30);
    }
    if gv_beqdec(&mut g.objs.aliens[idx as usize].sbyte1) {
        // .nxtstrat
        let s = sid(g, trucklaunch_strat2);
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte1 = 40;
    }
    // .move
    add_player_z(g, idx);
    let sb4 = g.objs.aliens[idx as usize].sbyte4 as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(sb4);
}

/// `.strat2` (DSTRATS.ASM:6285-6310): swing to the launch orientation over 40
/// ticks, then spawn the falling truck and enter the 1-tick kill state.
fn trucklaunch_strat2(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_achase_alvar B toward -deg45-deg22 (=-48) / -deg45+deg11 (=-24) / 0.
        let mut ry = al.roty;
        achase_angle(&mut ry, (DEG45.wrapping_add(DEG22)).wrapping_neg(), 4);
        al.roty = ry;
        let mut rz = al.rotz;
        achase_angle(&mut rz, DEG45.wrapping_sub(CAST_DEG11).wrapping_neg(), 4);
        al.rotz = rz;
        let mut rx = al.rotx;
        achase_angle(&mut rx, 0, 4);
        al.rotx = rx;
    }
    if gv_beqdec(&mut g.objs.aliens[idx as usize].sbyte1) {
        // .nxtstrat2: spawn the falling truck.
        g.objs.aliens[idx as usize].sbyte1 = 1;
        if let Some(truck) = cast_makeobj(g, idx) {
            copy_pos(g, truck, idx);
            let m = g.objs.aliens[idx as usize];
            let s_ft = sid(g, fallingtruck_init);
            let al = &mut g.objs.aliens[truck as usize];
            al.rotx = m.rotx;
            al.roty = m.roty;
            al.rotz = m.rotz;
            al.shape = SH_CAST_BOSS_E_3_PROXY;
            al.roty = al.roty.wrapping_sub(DEG90);
            // s_copy al_rotx,al_rotz ; s_neg al_rotx ; al_rotz = 0.
            al.rotx = (al.rotz as i8).wrapping_neg() as u8;
            al.rotz = 0;
            al.stratptr = Some(s_ft);
        }
        let s = sid(g, trucklaunch_strat3);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        // falls into .strat3 the next tick.
        return;
    }
    // .move
    add_player_z(g, idx);
    let sb4 = g.objs.aliens[idx as usize].sbyte4 as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(sb4);
}

/// `.strat3` (DSTRATS.ASM:6305-6310): one-tick fuse, then s_kill_obj into
/// bossexplode.
fn trucklaunch_strat3(g: &mut Game, idx: u16) {
    // s_decbne_alvar sbyte1 -> .move (dec first, branch when != 0).
    let sb1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb1;
    if sb1 != 0 {
        add_player_z(g, idx);
        let sb4 = g.objs.aliens[idx as usize].sbyte4 as u16;
        g.vars.bosshp = g.vars.bosshp.wrapping_add(sb4);
        return;
    }
    // s_set_bossmaxHP #0 ; s_kill_obj x (hp0 + colldisable -> bossexplode).
    set_bossmaxhp(g, 0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = 0;
    }
    if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
        g.call_strat(exp, idx);
    }
}

/// `fallingtruck_istrat` init (DSTRATS.ASM:6313-6319).
pub fn fallingtruck_init(g: &mut Game, idx: u16) {
    let s = sid(g, fallingtruck_strat);
    let s_col = sid(g, strat_hit_flash);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_col);
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.sflags |= ASF_COLLDISABLE;
    al.vel = 50;
    al.sbyte1 = 10;
    fallingtruck_strat(g, idx);
}

/// `fallingtruck` `.strat` (DSTRATS.ASM:6320-6336): fly straight for 10 ticks,
/// then level the pitch back toward the ground; scroll with the world.
fn fallingtruck_strat(g: &mut Game, idx: u16) {
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]); // dgen3dvecs
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]); // daddvecs2pos_x
    if gv_beqdec(&mut g.objs.aliens[idx as usize].sbyte1) {
        // .cmpit: rotx -= (deg90+deg22); if that >= deg22, subtract (deg11-deg90-deg22).
        let rotx = g.objs.aliens[idx as usize].rotx;
        let a = rotx.wrapping_sub(DEG90.wrapping_add(DEG22));
        if a >= DEG22 {
            let k = CAST_DEG11.wrapping_sub(DEG90).wrapping_sub(DEG22); // = 184 (i.e. +72)
            g.objs.aliens[idx as usize].rotx = a.wrapping_sub(k);
        }
    }
    add_player_z(g, idx);
}
// GROUNDVEHICLE_END

// ============================================================
// CASTANET_BEGIN — "castanet" / "Metal Smasher" (Route 2 L5).
//
// ASM oracle: `castanet_istrat` (DSTRATS.ASM:5754-6221), the two cymbal
// halves `castbit_istrat` (DSTRATS.ASM:6225-6257), `fireringlaser_l` +
// `ringlaser_istrat` (DSTRATS.ASM:6356-6492), and the ground-vehicle death
// (`trucklaunch`/`fallingtruck`, above). def_istrat index 124 (=IS_CASTANET).
//
// STRUCTURE: an invisible controller (the map's nullshape mother) drives a
// 27-entry `s_mode_table` state machine (DSTRATS.ASM:5770-5814). It links two
// visible cymbal "bit" objects via al_ptr (bit1, boss_e_0) and al_sword1
// (bit2, boss_e_1/1a) and every tick positions them apart/together by
// `al_sbyte2` (separation) at orientation `al_sbyte3`, rotated by the mother's
// rotz — reusing the flingboss `.positional` (s_add_Roffs2pos, fb_positional).
// Both bits carry castanetHP(120); the boss bar = bit1.hp+bit2.hp
// accumulated per tick; bossmaxhp = castanetHP*2. The boss dies when bit2
// (al_sword1) reaches 0 hp: bit1 becomes a `trucklaunch` and the mother is
// removed (DSTRATS.ASM:6085-6096).
//
// SCOPE NOTE (fidelity boundaries, cited):
//  * The mode machine, cymbal positioning, HP-bar/death, roll-in and the
//    aim/fire cadence (bit sflag1 firing gate) are ported tick-for-tick.
//  * ringlaser is ported as a straight scroll-and-fly laser (init + `.strat`,
//    DSTRATS.ASM:6398-6416). Its two trajectory-SHAPING systems are scoped
//    out: the spawn-time ring/cross spread keyed off the global `locusmode`
//    (DSTRATS.ASM:6366-6390) and the mid-flight `powerbuild`-triggered
//    aim-to-player curve (`.aimtoplayer`, DSTRATS.ASM:6417-6471). Both read
//    cross-object global scratch RAM (locusmode/powerbuild) that no other
//    ported strat needs; reproducing them faithfully needs those globals wired
//    into sf-game and an oracle diff. The mother's `locusmode`/`powerbuild`
//    writes (DSTRATS.ASM:5863/5882/6099-6111) are therefore intentional
//    no-ops here. Consequence: lasers fly straight ahead instead of fanning.
//  * `minicastanet` swarm (`.generateminis`, 4×`.launchminicastanets`,
//    DSTRATS.ASM:5955-5999): the ROM spawns four `path_istrat` mini-cymbals
//    on the `minicastanet`/`minicastanetLR` ROM path bytecodes, which are not
//    ported to sf-map. They are spawned here as straight scroll-movers (shape
//    boss_e_4) so the object count and the 3-draws-per-mini RNG consumption
//    stay faithful, but the ROM path curve is scoped out.
// ============================================================

// STRATEQU.INC / DSTRATS.ASM:78-97.
const CASTANET_HP: u8 = 120; // castanetHP
const CASTANET_AP: u8 = 10; // castanetAP
const MINICASTANET_HP: u8 = 2; // minicastanetHP
const MINICASTANET_AP: u8 = 8; // minicastanetAP
const CAST_DEG11: u8 = 8; // deg11

// Strategy flags (same mapping as flingboss): sflag1 -> sflags2 0x10;
// sflag8 -> sflags4 0x20 (ASF4_SFLAG8).
const CAST_SFLAG1: u8 = 0x10;

// Shape proxies (cosmetic; behaviour is shape-independent). The map's mother
// keeps SH_NULLSHAPE.
const SH_CAST_BOSS_E_0_PROXY: u16 = 276; // bit1
const SH_CAST_BOSS_E_1_PROXY: u16 = 277; // bit2 when rotz==128
const SH_CAST_BOSS_E_1A_PROXY: u16 = 278; // bit2 otherwise
const SH_CAST_BOSS_E_3_PROXY: u16 = 279; // fallingtruck
const SH_CAST_BOSS_E_4_PROXY: u16 = 280; // minicastanet
const SH_CAST_RINGLASER_PROXY: u16 = 281; // ringlaser

const CAST_SE_CLANG: u8 = 0x8d; // trigse $8d (cymbals meet)
const CAST_SE_SMASH: u8 = 0x8e; // trigse $8e (smash complete)

// Mode-table indices (DSTRATS.ASM:5770-5814, `ci`-counted from 0).
const M_ROLLONINIT: u8 = 0;
const M_ROLLON: u8 = 1;
const CAST_REPEAT: u8 = 2; // s_mode_entry .moveaway,cast_repeat
const M_LAST: u8 = 26; // .repeat

/// fling.makeobj analog (DSTRATS.ASM:3195, `s_make_obj`): allocate a child
/// shell copied onto the mother's position.
fn cast_makeobj(g: &mut Game, mother: u16) -> Option<u16> {
    let child = make_obj(g, SH_CAST_BOSS_E_0_PROXY)?;
    copy_pos(g, child, mother);
    Some(child)
}

/// s_set_objtobealvar read of a mother pointer slot (index+1 encoding),
/// returning the live child idx (None if unset or dead).
#[inline]
fn cast_read_obj(g: &Game, raw: u16) -> Option<u16> {
    let idx = boss_child_from_index_raw(raw)?;
    if g.objs.aliens[idx as usize].active {
        Some(idx)
    } else {
        None
    }
}

/// `castbit_istrat` init (DSTRATS.ASM:6225-6231).
fn castbit_init(g: &mut Game, idx: u16) {
    let s = sid(g, castbit_strat);
    let s_col = sid(g, castbit_col);
    let s_exp = sid(g, strat_explode); // explode_istrat
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = CASTANET_HP;
        al.ap = CASTANET_AP;
        al.type_ &= !ATZREMOVE; // s_clr_altype zremove
        al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
    }
    castbit_strat(g, idx);
}

/// `castbit_istrat` `.strat` (DSTRATS.ASM:6232-6244): pick the cymbal frame
/// shape, then (when the mother has flagged this bit's sflag1) fire a ringlaser
/// every other frame.
fn castbit_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.shape != SH_CAST_BOSS_E_0_PROXY {
            // bit2: rotz==128 -> boss_e_1 else boss_e_1a.
            al.shape = if al.rotz == 128 {
                SH_CAST_BOSS_E_1_PROXY
            } else {
                SH_CAST_BOSS_E_1A_PROXY
            };
        }
    }
    if g.objs.aliens[idx as usize].sflags2 & CAST_SFLAG1 == 0 {
        return; // .end
    }
    if g.vars.gameframe & 1 != 0 {
        return; // s_jmp_NOTdelay 1 -> only on gf&1==0
    }
    cast_fire_ringlaser(g, idx);
}

/// `castbit_istrat` `.hit` (DSTRATS.ASM:6248-6257): hitflash variant. The HF2
/// hitflag toggles `nohitaffect`; routed through the standard hitflash here
/// (the HF2 damage-vs-graze distinction is a collision-lane detail).
fn castbit_col(g: &mut Game, idx: u16) {
    strat_hit_flash(g, idx);
}

/// `fireringlaser_l` (DSTRATS.ASM:6356-6395) — straight-fly port (see the
/// section scope note: the locusmode ring spread is omitted).
fn cast_fire_ringlaser(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        me.rotx,
        me.roty,
        60, // al_vel = 60
        60, // lifetime (no ROM cap; ringlaser removes via .aimtoplayer, scoped)
        7,  // ringlaserAP
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    let s = sid(g, cast_ringlaser_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = me.rotz;
    al.shape = SH_CAST_RINGLASER_PROXY;
    al.type_ = ATLASER | ATZREMOVE;
    al.sflags &= !ASF_INVISIBLE;
    al.sflags |= ASF_SHADOW;
    al.stratptr = Some(s);
    strat_gen_vecs_3d(al);
}

/// `ringlaser_istrat` `.strat` (DSTRATS.ASM:6398-6416): straight scroll-and-fly
/// (the `.aimtoplayer`/curve branch is scoped out).
fn cast_ringlaser_strat(g: &mut Game, idx: u16) {
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]); // daddvecs2pos_x
    add_player_z(g, idx);
    // s_add_colanim x,#1,#5 (cosmetic frame cycle).
    let al = &mut g.objs.aliens[idx as usize];
    al.colframe = (al.colframe + 1) % 5;
}

// ---- mother helpers ----

/// `.gethp` (DSTRATS.ASM:5850-5858): bit1.hp + bit2.hp.
fn cast_gethp(g: &Game, idx: u16) -> u16 {
    let m = g.objs.aliens[idx as usize];
    let h1 = cast_read_obj(g, m.ptr)
        .map(|b| g.objs.aliens[b as usize].hp as u16)
        .unwrap_or(0);
    let h2 = cast_read_obj(g, m.sword1 as u16)
        .map(|b| g.objs.aliens[b as usize].hp as u16)
        .unwrap_or(0);
    h1 + h2
}

/// `.position1` (DSTRATS.ASM:6153-6180) — bit1 (al_ptr, boss_e_0).
fn cast_position1(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        c.rotx = m.rotx;
        c.roty = m.roty;
        c.rotz = m.rotz;
        c.animframe = m.animframe;
        if m.sflags2 & CAST_SFLAG1 != 0 {
            c.rotx = c.rotx.wrapping_sub(m.sbyte3); // .otherbit1
        } else {
            c.roty = c.roty.wrapping_sub(m.sbyte3);
        }
    }
    // x1 = -42 - sbyte2 (s_varsub_alvar), y1=z1=0; fling.positional (<<2).
    let x1 = (-42i8).wrapping_sub(m.sbyte2 as i8) as i16;
    fb_positional(g, &m, child, x1, 0, 0);
}

/// `.position2` (DSTRATS.ASM:6182-6220) — bit2 (al_sword1, boss_e_1/1a).
fn cast_position2(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        c.rotx = m.rotx;
        c.roty = m.roty;
        c.rotz = m.rotz;
        // animframe = (mother.animframe EOR 7), then dincanim -1 wrap 8.
        c.animframe = (m.animframe ^ 7).wrapping_add(7) % 8; // -1 mod 8
        c.rotz = c.rotz.wrapping_add(DEG180);
        if m.sflags2 & CAST_SFLAG1 != 0 {
            c.rotx = c.rotx.wrapping_add(m.sbyte3); // .otherbit2
        } else {
            c.roty = c.roty.wrapping_add(m.sbyte3);
        }
    }
    // x1 = 42 + sbyte2 (s_varadd_alvar), y1=z1=0.
    let x1 = (42i8).wrapping_add(m.sbyte2 as i8) as i16;
    fb_positional(g, &m, child, x1, 0, 0);
}

/// `.move` / `.movenochk` common tail (DSTRATS.ASM:6083-6130). Returns true
/// when the mother was removed (a bit died -> trucklaunch) — the caller must
/// then stop ticking the mode machine.
fn cast_move(g: &mut Game, idx: u16) -> bool {
    let m = g.objs.aliens[idx as usize];
    // .move: death test keys on bit2 (al_sword1) hp==0.
    let bit2_dead = cast_read_obj(g, m.sword1 as u16)
        .map(|b| g.objs.aliens[b as usize].hp == 0)
        .unwrap_or(true);
    let ptr_nonzero = m.ptr != 0;
    if bit2_dead {
        // .ok_change_mode: survivor = al_ptr (bit1).
        if let Some(s) = cast_read_obj(g, m.ptr) {
            trucklaunch_init(g, s);
        }
        g.objs.aldead = 1; // s_remove_obj (mother is the current object)
        return true;
    } else if !ptr_nonzero {
        // .ok_change_mode2: survivor = al_sword1 (bit2). (bit1 pointer cleared.)
        if let Some(s) = cast_read_obj(g, m.sword1 as u16) {
            trucklaunch_init(g, s);
        }
        g.objs.aldead = 1;
        return true;
    }
    // .movenochk (both alive). locusmode computation is a no-op here (scoped).
    add_player_z(g, idx);
    if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
        // homing: chase worldx/worldy toward the player (s_achase_alvar2alvar W,4).
        if let Some(pl) = player_idx(g) {
            let p = g.objs.aliens[pl as usize];
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = chase_proportional(al.worldx, p.worldx, 4);
            al.worldy = chase_proportional(al.worldy, p.worldy, 4);
        }
    }
    // .nohomein: position both bits + accumulate the HP bar.
    if let Some(b1) = cast_read_obj(g, g.objs.aliens[idx as usize].ptr) {
        add_bosshp(g, b1);
        cast_position1(g, idx, b1);
    }
    if let Some(b2) = cast_read_obj(g, g.objs.aliens[idx as usize].sword1 as u16) {
        add_bosshp(g, b2);
        cast_position2(g, idx, b2);
    }
    false
}

/// `.launchminicastanets` (DSTRATS.ASM:5962-5999). See the scope note: spawned
/// as a straight scroll-mover with faithful 3-draw RNG consumption.
fn cast_launch_mini(g: &mut Game, mother: u16) {
    let Some(mini) = cast_makeobj(g, mother) else {
        return;
    };
    let s = sid(g, cast_mini_strat);
    copy_pos(g, mini, mother);
    {
        let m = g.objs.aliens[mother as usize];
        let al = &mut g.objs.aliens[mini as usize];
        al.rotx = m.rotx;
        al.roty = m.roty;
        al.rotz = m.rotz;
        al.shape = SH_CAST_BOSS_E_4_PROXY;
        al.hp = MINICASTANET_HP;
        al.ap = MINICASTANET_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.stratptr = Some(s);
    }
    // random_l offsets: worldx, worldy (sign-extended byte adds), then worldz+20.
    let rx = (sfrtl_random(g) as u8 as i8) as i16;
    g.objs.aliens[mini as usize].worldx = g.objs.aliens[mini as usize].worldx.wrapping_add(rx);
    let ry = (sfrtl_random(g) as u8 as i8) as i16;
    g.objs.aliens[mini as usize].worldy = g.objs.aliens[mini as usize].worldy.wrapping_add(ry);
    g.objs.aliens[mini as usize].worldz = g.objs.aliens[mini as usize].worldz.wrapping_add(20);
    // sbyte1 = (random_l & 7) << 2.
    let sb = ((sfrtl_random(g) as u8) & 7) << 2;
    g.objs.aliens[mini as usize].sbyte1 = sb;
}

/// Minimal minicastanet mover (ROM: path_istrat on the minicastanet path —
/// scoped): scroll with the world.
fn cast_mini_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

// ---- mother state machine ----

/// `castanet_istrat` `.generate` (DSTRATS.ASM:6132-6151): spawn the two cymbal
/// bits, linking bit1 via al_ptr and bit2 via al_sword1. Returns true on
/// success (both bits present).
fn cast_generate(g: &mut Game, mother: u16) -> bool {
    if cast_read_obj(g, g.objs.aliens[mother as usize].ptr).is_none() {
        let Some(b1) = cast_makeobj(g, mother) else {
            return false;
        };
        let s = sid(g, castbit_init);
        {
            let al = &mut g.objs.aliens[b1 as usize];
            al.stratptr = Some(s);
            al.shape = SH_CAST_BOSS_E_0_PROXY;
            al.depthoffset = 1;
        }
        g.objs.aliens[mother as usize].ptr = boss_obj_index_or_null(b1);
        cast_position1(g, mother, b1);
    }
    let Some(b2) = cast_makeobj(g, mother) else {
        return false;
    };
    let s = sid(g, castbit_init);
    {
        let al = &mut g.objs.aliens[b2 as usize];
        al.stratptr = Some(s);
        al.shape = SH_CAST_BOSS_E_1_PROXY;
        al.hp = 1; // (overwritten to castanetHP by castbit_init next tick)
        al.depthoffset = 2;
    }
    g.objs.aliens[mother as usize].sword1 = boss_obj_index_or_null(b2) as i16;
    cast_position2(g, mother, b2);
    true
}

/// `castanet_istrat` init (DSTRATS.ASM:5754-5768).
pub fn strat_castanet_init(g: &mut Game, idx: u16) {
    set_bossmaxhp(g, (CASTANET_HP as u16) * 2); // 240
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.animframe = 0;
        al.type_ &= !ATZREMOVE;
    }
    if !cast_generate(g, idx) {
        // .generate failed (pool full) -> retry the whole init next tick.
        let s = sid(g, strat_castanet_init);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        return;
    }
    let s = sid(g, castanet_strat);
    let s_col = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = None;
        al.sflags |= ASF_NOHITAFFECT;
        al.stratstate = 0; // s_mode_change x,#0
    }
    // init falls into .strat the same tick.
    castanet_strat(g, idx);
}

/// `castanet_istrat` `.strat` (DSTRATS.ASM:5769-6130): the 27-entry mode
/// machine. `stratstate` holds the mode index; `.nxtmode`/`.znxtmode` advance
/// and re-enter the same tick, `.move` ends the tick (positioning + HP bar +
/// death check).
fn castanet_strat(g: &mut Game, idx: u16) {
    loop {
        let mode = g.objs.aliens[idx as usize].stratstate as u8;
        match mode {
            // 0 .rolloninit
            M_ROLLONINIT => {
                let al = &mut g.objs.aliens[idx as usize];
                al.worldz = al.worldz.wrapping_add(6000);
                al.worldy = al.worldy.wrapping_add(4000);
                al.sword2 = -500;
                al.sflags4 |= ASF4_SFLAG8;
                // -> .nxtmode
            }
            // 1 .rollon
            M_ROLLON => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    let sw2 = al.sword2;
                    al.worldy = al.worldy.wrapping_add(sw2);
                    al.sword2 = al.sword2.wrapping_add(20);
                    al.worldz = al.worldz.wrapping_sub(150);
                }
                if sea_dz_less(g, idx, 2000) {
                    // -> .nxtmode
                } else {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 3, 9 .generateminis
            3 | 9 => {
                for _ in 0..4 {
                    cast_launch_mini(g, idx);
                }
                // -> .nxtmode
            }
            // 4,10,16,21 .moveapart
            4 | 10 | 16 | 21 => {
                let s_col = sid(g, castbit_col);
                let m = g.objs.aliens[idx as usize];
                if let Some(b1) = cast_read_obj(g, m.ptr) {
                    g.objs.aliens[b1 as usize].collstratptr = Some(s_col);
                }
                if let Some(b2) = cast_read_obj(g, m.sword1 as u16) {
                    g.objs.aliens[b2 as usize].collstratptr = Some(s_col);
                }
                let sb2 = g.objs.aliens[idx as usize].sbyte2.wrapping_add(2);
                g.objs.aliens[idx as usize].sbyte2 = sb2;
                if sb2 >= 40 {
                    // -> .nxtmode
                } else {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 5, 11 .movetoplayer
            5 | 11 => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    let mut sb3 = al.sbyte3;
                    achase_angle(&mut sb3, DEG45, 2);
                    al.sbyte3 = sb3;
                    al.worldz = al.worldz.wrapping_sub(20); // 2000/100
                    al.sbyte1 = al.sbyte1.wrapping_add(1);
                }
                if g.objs.aliens[idx as usize].sbyte1 >= 100 {
                    // .znxtmode
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                } else {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 6,12,19,24 .smashtogether
            6 | 12 | 19 | 24 => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    let mut sb3 = al.sbyte3;
                    achase_angle(&mut sb3, 0, 1);
                    al.sbyte3 = sb3;
                }
                // two s_beqdec_alvar sbyte2 -> .sndnxtmode when it hits 0.
                let mut done = false;
                for _ in 0..2 {
                    if gv_beqdec(&mut g.objs.aliens[idx as usize].sbyte2) {
                        done = true;
                        break;
                    }
                }
                if done {
                    // .sndnxtmode
                    play_se(g, CAST_SE_SMASH);
                } else {
                    let m = g.objs.aliens[idx as usize];
                    if let Some(b1) = cast_read_obj(g, m.ptr) {
                        if g.objs.aliens[b1 as usize].collstratptr.is_some() {
                            play_se(g, CAST_SE_CLANG);
                        }
                        g.objs.aliens[b1 as usize].collstratptr = None;
                    }
                    if let Some(b2) = cast_read_obj(g, m.sword1 as u16) {
                        g.objs.aliens[b2 as usize].collstratptr = None;
                    }
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 2,7,13 .moveaway
            2 | 7 | 13 => {
                g.objs.aliens[idx as usize].sflags4 &= !ASF4_SFLAG8;
                if sea_dz_less(g, idx, 2000) {
                    g.objs.aliens[idx as usize].worldz =
                        g.objs.aliens[idx as usize].worldz.wrapping_add(60);
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
                // dz>=2000 -> .nxtmode
            }
            // 8, 20 .rotate90r
            8 | 20 => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sflags2 |= CAST_SFLAG1;
                    al.rotz = al.rotz.wrapping_add(8);
                    al.sbyte1 = al.sbyte1.wrapping_add(1);
                }
                if g.objs.aliens[idx as usize].sbyte1 == 8 {
                    // .znxtmode
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                } else {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 14, 25 .rotate90l
            14 | 25 => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.rotz = al.rotz.wrapping_sub(8);
                    al.sbyte1 = al.sbyte1.wrapping_add(1);
                }
                if g.objs.aliens[idx as usize].sbyte1 == 8 {
                    // .zznxtmode: clear sflag1 then .znxtmode.
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sflags2 &= !CAST_SFLAG1;
                    al.sbyte1 = 0;
                } else {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 15 .checkhp
            15 => {
                let total = cast_gethp(g, idx);
                if total < (CASTANET_HP as u16) + (CASTANET_HP as u16) / 2 {
                    // < 180 -> .nxtmode (proceed to the aim/fire attack phase)
                } else {
                    // healthy -> .repeat (mode = cast_repeat).
                    g.objs.aliens[idx as usize].stratstate = CAST_REPEAT;
                    continue;
                }
            }
            // 17, 22 .aim
            17 | 22 => {
                let mut sb3 = g.objs.aliens[idx as usize].sbyte3;
                let reached = achase_angle(&mut sb3, DEG22, 2);
                g.objs.aliens[idx as usize].sbyte3 = sb3;
                if reached {
                    // -> .nxtmode
                } else {
                    // powerbuild=0 (scoped no-op)
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
            }
            // 18, 23 .fire
            18 | 23 => {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    let add = al.sbyte1 >> 3;
                    al.animframe = al.animframe.wrapping_add(add) % 8;
                }
                let m = g.objs.aliens[idx as usize];
                if let Some(b1) = cast_read_obj(g, m.ptr) {
                    g.objs.aliens[b1 as usize].sflags2 |= CAST_SFLAG1;
                }
                if let Some(b2) = cast_read_obj(g, m.sword1 as u16) {
                    g.objs.aliens[b2 as usize].sflags2 |= CAST_SFLAG1;
                }
                let sb1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
                g.objs.aliens[idx as usize].sbyte1 = sb1;
                if sb1 < 40 {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
                // powerbuild=1 (scoped); clear both bits' sflag1.
                let m = g.objs.aliens[idx as usize];
                if let Some(b1) = cast_read_obj(g, m.ptr) {
                    g.objs.aliens[b1 as usize].sflags2 &= !CAST_SFLAG1;
                }
                if let Some(b2) = cast_read_obj(g, m.sword1 as u16) {
                    g.objs.aliens[b2 as usize].sflags2 &= !CAST_SFLAG1;
                }
                if sb1 < 60 {
                    if cast_move(g, idx) {
                        return;
                    }
                    return;
                }
                // .znxtmode
                g.objs.aliens[idx as usize].sbyte1 = 0;
            }
            // 26 .repeat
            _ => {
                g.objs.aliens[idx as usize].stratstate = CAST_REPEAT;
                continue;
            }
        }
        // Fell through the match arm without ending the tick: advance mode
        // (.nxtmode / .znxtmode) and re-enter the same tick.
        let next = g.objs.aliens[idx as usize].stratstate + 1;
        g.objs.aliens[idx as usize].stratstate = next;
    }
}
// CASTANET_END

// ============================================================
// CHICKEN_BEGIN — "chicken" (Route 3 L3 boss) + the SHARED grabber-tentacle
// `arm_istrat` (which flingboss also builds on).
//
// ASM oracle: `chicken_istrat` / `chick` (DSTRATS.ASM:3696-4523) and the shared
// arm strat `arm_istrat` / `ars` (DSTRATS.ASM:2444-2944) — init, `.strat`,
// `.chickenheadcol`, `.chickenheadhit`, `.grabberhit`, `.passiton`,
// `.zipthrough`/`.position` spring easing, `.generate`/`.noacc` growth.
// def_istrat index 117 (=IS_CHICKEN).
//
// STRUCTURE: the SH_BOSS_D_1 body (the map's mother, HP=chickenbodyHP=64) drives
// a 29-entry `s_mode_table` machine (DSTRATS.ASM:3721-3763; +7 unreachable
// `flyaway_mode` entries :3754-3761). It links THREE neck chains — al_ptr
// (left neck→head), al_sword1 (right neck→head), al_sword2 (tail) — plus two
// wings (al_sWPx1/al_sWPy1). Each neck is a chain of `arm_istrat` segments that
// GROW outward (`.nbl`→`.generate`, sword1 countdown → head/tail/grabber) and
// are positioned by an inter-segment damped SPRING (`.zipthrough`→`.position`:
// each child eases half-way toward its parent's rots + a decaying momentum term
// on al_sbyte1..3). The body is invulnerable (nohitaffect) until a neck grows
// its head/tail fully — then `.check_fin` (DSTRATS.ASM:4031-4079) clears
// nohitaffect (the red vulnerability window) and the body's hp drains the boss
// bar (`s_add_bossHP x,al_hp`, :4027). Shooting a head shortens its neck
// (`.chickenheadhit`, :2678). `armmode` (a shared WRAM byte, $17f0) gates which
// head fires firebreath and whether growth makes a head/tail vs a grabber.
//
// SCOPE NOTE (fidelity boundaries, cited inline):
//  * The mother mode machine, neck GROWTH (`.generate`/`.nbl`), the inter-
//    segment SPRING easing (`.position`), `.chickenheadcol`/`.chickenheadhit`
//    neck-shortening, `.check_fin` vulnerability + HP-bar drain, `.regrownecks`
//    regrowth, and the grabber routing (`.grabberhit`→`.passiton`→mother
//    sflag5) are ported faithfully.
//  * `.passiton`'s no-sflag1 branch searches for the `#flingboss` body shape
//    (DSTRATS.ASM:2777) — arm_istrat was written for flingboss and chicken
//    reuses it; chicken's chain roots carry sflag1 and route damage UP the
//    parent chain instead, so the flingboss-shape lookup is inert under chicken
//    (faithful — verified: chicken never spawns a #flingboss object). The
//    routing is exercised in tests via a flingboss-shaped stand-in mother.
//  * Sub-objects are spawned faithfully (RNG/object-count parity) but their
//    FLIGHT internals are simplified to straight/gravity movers, exactly as
//    castanet's ringlaser/mini were: `firebreathe_istrat` (the trail-piece
//    fireball, DSTRATS.ASM:4629-4699), `egg_istrat`/`shell_istrat`
//    (:4528-4622) and `wings_istrat` (:4744-4761, a colldisable nohitaffect
//    cosmetic flapper). The arm HPLASMA fire reuses the ported homingflat
//    `boss2_hplasma_strat`.
//  * `s_leftview_strat` (screen-side turn selector, DSTRATS.ASM:4227/4245) is
//    approximated by worldx-vs-player (no projected-screen math); the 5-arg
//    `s_add_anim` firstframe clamp on `.sitdown` (:3913) is read per the
//    AUDIT_BOSS_TICKS2 macro rule (cap→jump) rather than its ambiguous
//    firstframe clamp. Both are cosmetic timing.
// ============================================================

// DSTRATS.ASM:68-71 / :58-59.
const CHICKEN_BODY_HP: u8 = 64; // chickenbodyHP
const CHICKEN_BODY_AP: u8 = HARD_AP; // chickenbodyAP = hardAP
const CHICKEN_HEAD_HP: u8 = 4; // chickenheadHP
const CHICKEN_TAIL_HP: u8 = 2; // chickentailHP
const CHICK_ARM_AP: u8 = 10; // armAP (DSTRATS.ASM:59)
const CHICK_ARMLENGTH: i16 = 80; // armlength (= ARMLENGTH)
const CHICK_HPLASMA_SPEED: u8 = 60;
const CHICK_HPLASMA_LIFE: u8 = 50;
const CHICK_HPLASMA_AP: u8 = 10; // HplasmaAP
const CHICK_DEG11: u8 = 8; // deg11

// sflag mapping (same as flingboss/castanet): sflag1..4 -> sflags2 0x10/20/40/80;
// sflag5/6 -> sflags3 0x01/0x02.
const CH_SFLAG1: u8 = 0x10;
const CH_SFLAG2: u8 = 0x20;
const CH_SFLAG3: u8 = 0x40;
const CH_SFLAG4: u8 = 0x80;
const CH_SFLAG5: u8 = 0x01; // sflags3
const CH_SFLAG6: u8 = 0x02; // sflags3

// Shapes. boss_d_1 (body) = the map's SH_BOSS_D_1 (78, route3::common); the rest
// are behaviour-only proxies (compared for equality only).
const CH_BOSS_D_1: u16 = 78; // body / mother (map SH_BOSS_D_1)
const SH_CHICK_BOSS_D_0: u16 = 282; // head (boss_d_0)
const SH_CHICK_BOSS_D_2: u16 = 283; // tail (boss_d_2)
const SH_CHICK_NECK: u16 = 284; // neck
const SH_CHICK_ARM: u16 = 285; // arm
const SH_CHICK_BULGE: u16 = 286; // bulge (charge/damage travelling shape)
const SH_CHICK_GRABBER: u16 = 287; // grabber
const SH_CHICK_GRABBER2: u16 = 288; // grabber2
const SH_CHICK_EGG: u16 = 289; // egg
const SH_CHICK_FIREBREATH: u16 = 290; // firebreath
const SH_CHICK_BOSS_D_8: u16 = 291; // wing1 (boss_d_8)
const SH_CHICK_BOSS_D_9: u16 = 292; // wing2 (boss_d_9)

// flingboss body shape (sf-map rc.rs SH_FLINGBOSS=12) — the `.passiton` mother
// target for the shared arm code (DSTRATS.ASM:2777 `#flingboss`).
const SH_FLINGBOSS_BODY: u16 = 12;

// C `g_armmode` = ARMMODE ($000017f0) — shared WRAM byte read/written by the
// mother AND every arm segment (they cannot reach the mother directly).
const WM_ARMMODE: u16 = 0x17F0;

const CHICK_SE_FIRE: u8 = 0x3b; // trigse $3b
const CHICK_SE_EGG: u8 = 0x3a; // trigse $3a
const CHICK_SE_WHOOSH: u8 = 0x39; // trigse $39

#[inline]
fn armmode(g: &Game) -> u8 {
    wm8(g, WM_ARMMODE)
}
#[inline]
fn set_armmode(g: &mut Game, v: u8) {
    wm8_set(g, WM_ARMMODE, v);
}

/// adiv2 — signed halve toward zero (STRATMAC.INC adiv2, AUDIT finding 24).
#[inline]
fn adiv2i(v: u8) -> u8 {
    ((v as i8) / 2) as u8
}
/// Repeated toward-zero halving (s_set_alvar2vartab scale -N).
#[inline]
fn adiv_n(mut v: u8, n: u32) -> u8 {
    for _ in 0..n {
        v = adiv2i(v);
    }
    v
}

/// s_copy_rots y,x — copy `src` rots into `dst` (DSTRATS.ASM `fling.copyrots_yx`).
fn chicken_copyrots(g: &mut Game, dst: u16, src: u16) {
    let s = g.objs.aliens[src as usize];
    let d = &mut g.objs.aliens[dst as usize];
    d.rotx = s.rotx;
    d.roty = s.roty;
    d.rotz = s.rotz;
}

/// fling.makeobj — allocate a child shell copied onto the mother's position.
fn chicken_makeobj(g: &mut Game, mother: u16) -> Option<u16> {
    let child = make_obj(g, 0)?;
    copy_pos(g, child, mother);
    Some(child)
}

/// find_y_l — first active object of shape `shape` (DSTRATS.ASM:3589).
fn chicken_find_shape(g: &Game, shape: u16) -> Option<u16> {
    (0..NUMBER_AL)
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
        .map(|i| i as u16)
}
/// find_alptr_l — the object whose al_ptr points at `target` (i.e. `target`'s
/// parent toward the body; DSTRATS.ASM:3548).
fn chicken_find_alptr(g: &Game, target: u16) -> Option<u16> {
    let want = boss_obj_index_or_null(target);
    (0..NUMBER_AL)
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].ptr == want)
        .map(|i| i as u16)
}
/// remove_alptrs_l — clear any al_ptr pointing at `target` (DSTRATS.ASM:2364).
fn chicken_remove_alptrs(g: &mut Game, target: u16) {
    let want = boss_obj_index_or_null(target);
    for i in 0..NUMBER_AL {
        if g.objs.aliens[i].active && g.objs.aliens[i].ptr == want {
            g.objs.aliens[i].ptr = 0;
        }
    }
}

// ============================================================
// SHARED arm_istrat (DSTRATS.ASM:2444-2944).
// ============================================================

/// arm_istrat init (DSTRATS.ASM:2444-2473).
pub fn chicken_arm_init(g: &mut Game, idx: u16) {
    let s_strat = sid(g, chicken_arm_strat);
    let s_exp = sid(g, strat_explode);
    let shape = g.objs.aliens[idx as usize].shape;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = None; // s_set_alptrs .strat,0,explode
        al.expstratptr = Some(s_exp);
        al.hp = HARD_HP; // armHP = hardHP
        al.ap = CHICK_ARM_AP;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sbyte3 = 0;
        al.sbyte4 = 32;
        al.collflags |= COLLTYPE_ENEMY1;
        al.type_ &= !ATZREMOVE; // s_clr_altype zremove
    }
    if shape == SH_CHICK_GRABBER {
        let s = sid(g, chicken_arm_grabberhit);
        g.objs.aliens[idx as usize].collstratptr = Some(s);
    }
    // head/tail data + chickenheadcol collstrat (overrides grabberhit — but
    // grabber is never head/tail so no conflict).
    if shape == SH_CHICK_BOSS_D_2 {
        let s = sid(g, chicken_arm_chickenheadcol);
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = CHICKEN_TAIL_HP + 64;
        al.ap = CHICK_ARM_AP;
        al.collstratptr = Some(s);
    } else if shape == SH_CHICK_BOSS_D_0 {
        let s = sid(g, chicken_arm_chickenheadcol);
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = CHICKEN_HEAD_HP + 64;
        al.ap = CHICK_ARM_AP;
        al.collstratptr = Some(s);
    }
    // init falls into .strat the same tick.
    chicken_arm_strat(g, idx);
}

/// arm_istrat `.strat` (DSTRATS.ASM:2476-2650).
fn chicken_arm_strat(g: &mut Game, idx: u16) {
    let shape = g.objs.aliens[idx as usize].shape;
    let is_head = shape == SH_CHICK_BOSS_D_0;
    let is_tail = shape == SH_CHICK_BOSS_D_2;
    if is_head || is_tail {
        // .taildesu: below the +64 floor means the player damaged it.
        if (g.objs.aliens[idx as usize].hp as u16) < 65 {
            chicken_arm_chickenheadhit(g, idx);
        }
        if is_head {
            // Orient the head upright (DSTRATS.ASM:2494-2504).
            let rotx = g.objs.aliens[idx as usize].rotx;
            let diff = rotx.wrapping_sub(DEG90);
            g.objs.aliens[idx as usize].rotz = if diff & 0x80 == 0 { DEG180 } else { 0 };
            chicken_arm_firebreath(g, idx);
        }
    }
    // .notachicken
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 == 0 {
        // sflag1 CLEAR -> this is a chain ROOT: ease the whole al_ptr chain.
        chicken_arm_zipthrough(g, idx);
    }
    // .nm — sflag2 fire/bulge windup.
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG2 != 0 && chicken_arm_fire_bulge(g, idx) {
        return; // sprouty.expl removed this segment (.end)
    }
    // .nb — sflag5 reverse-bullet (damage travelling up the chain).
    if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG5 != 0 {
        // ddecanim x,#8 (-1 mod 8).
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.animframe = (al.animframe + 8 - 1) % 8;
        }
        if g.objs.aliens[idx as usize].animframe == 7 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.shape = SH_CHICK_ARM;
                al.sflags3 &= !CH_SFLAG5;
            }
            chicken_arm_passiton(g, idx);
        }
    }
    // .nbl — grabber/neck GROWTH gate.
    chicken_arm_nbl(g, idx);
}

/// arm `.strat` head firebreath (DSTRATS.ASM:2517-2557): swap `armmode` head
/// bit, gate on the alternating head, spawn one firebreath if none is live.
fn chicken_arm_firebreath(g: &mut Game, idx: u16) {
    let mut am = armmode(g) ^ 64; // swap head mode
    set_armmode(g, am);
    if am & 1 == 0 {
        return; // rlbeq .notachicken
    }
    if ((am << 1) ^ am) & 64 != 0 {
        return; // bit#64 -> .notachicken
    }
    if chicken_find_shape(g, SH_CHICK_FIREBREATH).is_some() {
        return; // only one firebreath live
    }
    let Some(fb) = chicken_makeobj(g, idx) else {
        return;
    };
    copy_pos(g, fb, idx);
    chicken_copyrots(g, fb, idx);
    let s = sid(g, chicken_firebreath_strat);
    {
        let al = &mut g.objs.aliens[fb as usize];
        al.shape = SH_CHICK_FIREBREATH;
        al.stratptr = Some(s);
        al.vel = 80;
        al.sflags &= !ASF_INVISIBLE;
    }
    play_se(g, CHICK_SE_FIRE);
    am ^= 32; // allow the other head to fire
    set_armmode(g, am);
    // Pitch the firebreath away from the head's facing (DSTRATS.ASM:2547-2556).
    let rotx = g.objs.aliens[idx as usize].rotx;
    let diff = rotx.wrapping_sub(DEG90);
    let off = DEG45.wrapping_add(CHICK_DEG11); // deg45+deg11 = 40
    let add = if diff & 0x80 != 0 {
        off
    } else {
        (-(off as i8)) as u8
    };
    g.objs.aliens[fb as usize].rotx = g.objs.aliens[fb as usize].rotx.wrapping_add(add);
}

/// arm `.nm` sflag2 block (DSTRATS.ASM:2562-2605). Returns true when the segment
/// removed itself (sprouty.expl).
fn chicken_arm_fire_bulge(g: &mut Game, idx: u16) -> bool {
    let shape = g.objs.aliens[idx as usize].shape;
    if shape == SH_CHICK_GRABBER {
        // .fire: muzzle z = (10>>ws)<<1, then fire_weapon <<ws.
        chicken_arm_fire_hplasma(g, idx, ((10i16 >> 2) << 1) << 2);
        g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG2;
        return false;
    }
    if shape == SH_CHICK_BOSS_D_2 {
        // tail drops an egg.
        if let Some(egg) = chicken_makeobj(g, idx) {
            copy_pos(g, egg, idx);
            chicken_copyrots(g, egg, idx);
            let s = sid(g, chicken_egg_strat);
            let al = &mut g.objs.aliens[egg as usize];
            al.shape = SH_CHICK_EGG;
            al.stratptr = Some(s);
            al.sflags &= !ASF_INVISIBLE;
        }
        g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG2;
        return false;
    }
    // .notaneggmate
    if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG5 != 0
        && g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 != 0
    {
        chicken_arm_sprouty_expl(g, idx);
        return true;
    }
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG3 == 0 {
        // .nx: begin bulge.
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.shape = SH_CHICK_BULGE;
        al.sflags2 |= CH_SFLAG3;
    } else {
        // .b: dincanim #5 (+1 mod 5); at frame 4 propagate/fire.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.animframe = (al.animframe + 1) % 5;
        }
        if g.objs.aliens[idx as usize].animframe == 4 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags2 &= !(CH_SFLAG2 | CH_SFLAG3);
                al.shape = SH_CHICK_ARM;
            }
            if let Some(child) = fb_read_obj(g, g.objs.aliens[idx as usize].ptr) {
                // bulge travels to the next segment.
                g.objs.aliens[child as usize].sflags2 |= CH_SFLAG2;
            } else {
                // .fire2: tip fires HPLASMA (muzzle z = ((armlength-5)>>ws)<<1).
                chicken_arm_fire_hplasma(g, idx, (((CHICK_ARMLENGTH - 5) >> 2) << 1) << 2);
                g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG2;
            }
        }
    }
    false
}

/// arm `.nbl` (DSTRATS.ASM:2617-2648): idle-timer + grow gate.
fn chicken_arm_nbl(g: &mut Game, idx: u16) {
    let am = armmode(g);
    if am == 0 {
        // s_decbne_alvar sbyte4 -> .notend (branch while != 0), reset 32 at 0.
        let sb4 = g.objs.aliens[idx as usize].sbyte4.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte4 = sb4;
        if sb4 != 0 {
            return chicken_arm_notend(g, idx);
        }
        g.objs.aliens[idx as usize].sbyte4 = 32;
    }
    // .nowait
    if am & 6 == 6 {
        return; // both head bits set -> .notgrabber2 (.end)
    }
    if g.objs.aliens[idx as usize].ptr != 0 {
        return chicken_arm_notend(g, idx); // already has a forward child
    }
    let shape = g.objs.aliens[idx as usize].shape;
    if shape == SH_CHICK_GRABBER2 {
        return chicken_arm_notend(g, idx);
    }
    if shape == SH_CHICK_BOSS_D_2 || shape == SH_CHICK_BOSS_D_0 {
        return; // head/tail terminus -> .notgrabber2
    }
    if shape == SH_CHICK_GRABBER {
        return chicken_arm_notend(g, idx); // grabber is terminal
    }
    // neck/arm tip -> grow the next segment.
    chicken_arm_generate(g, idx);
}

/// arm `.notend` (DSTRATS.ASM:2638-2648): grabber2 self-rights its rots.
fn chicken_arm_notend(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].shape != SH_CHICK_GRABBER2 {
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = DEG180;
    al.rotx = 0;
    al.rotz = 0;
}

/// arm `.generate` (DSTRATS.ASM:2806-2859): spawn the next segment forward,
/// counting `al_sword1` down to the head/tail/grabber terminus.
fn chicken_arm_generate(g: &mut Game, idx: u16) {
    let Some(y) = chicken_makeobj(g, idx) else {
        return; // bcs .end
    };
    let shape_x = g.objs.aliens[idx as usize].shape;
    let sword1_x = (g.objs.aliens[idx as usize].sword1 as u16) as u8;
    let mut through_notgrabber = false;
    if sword1_x != 1 {
        through_notgrabber = true;
    } else if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG6 != 0 {
        g.objs.aliens[y as usize].shape = SH_CHICK_GRABBER2;
        g.objs.aliens[y as usize].animframe = 0;
        through_notgrabber = true;
    } else {
        let am = armmode(g);
        if am == 0 {
            g.objs.aliens[y as usize].shape = SH_CHICK_GRABBER; // .normgrab3
        } else if shape_x == SH_CHICK_NECK {
            g.objs.aliens[y as usize].shape = SH_CHICK_BOSS_D_0; // .chickenhead (head)
            set_armmode(g, am.wrapping_add(2));
        } else {
            g.objs.aliens[y as usize].shape = SH_CHICK_BOSS_D_2; // tail
            set_armmode(g, am.wrapping_add(2));
        }
    }
    // .notgrabber: a neck parent stamps neck shape onto y.
    if through_notgrabber && shape_x == SH_CHICK_NECK {
        g.objs.aliens[y as usize].shape = SH_CHICK_NECK;
    }
    // .normgrab2 (DSTRATS.ASM:2843-2859).
    let s = sid(g, chicken_arm_init);
    g.objs.aliens[y as usize].stratptr = Some(s);
    g.objs.aliens[y as usize].sword1 = sword1_x.wrapping_sub(1) as i16; // y.sword1 = x.sword1 - 1
    if (sword1_x as i8) >= 3 {
        g.objs.aliens[idx as usize].sword1 = sword1_x.wrapping_sub(1) as i16;
    }
    chicken_copyrots(g, y, idx);
    if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG6 != 0 {
        g.objs.aliens[y as usize].sflags3 |= CH_SFLAG6;
    }
    g.objs.aliens[y as usize].sflags2 |= CH_SFLAG1; // generated segments are non-root
    g.objs.aliens[idx as usize].ptr = boss_obj_index_or_null(y); // link forward
    chicken_arm_noacc(g, idx, y);
}

/// `.noacc` (DSTRATS.ASM:2936-2942): position child at (0,0,(armlength-2)/2)
/// rotated by the parent (fling.positional).
fn chicken_arm_noacc(g: &mut Game, parent: u16, child: u16) {
    let p = g.objs.aliens[parent as usize];
    fb_positional(g, &p, child, 0, 0, (CHICK_ARMLENGTH - 2) / 2);
}

/// `.zipthrough` (DSTRATS.ASM:2794-2805): ease each al_ptr descendant toward its
/// parent's rots.
fn chicken_arm_zipthrough(g: &mut Game, idx: u16) {
    let mut x = idx;
    while let Some(y) = fb_read_obj(g, g.objs.aliens[x as usize].ptr) {
        chicken_arm_position(g, x, y);
        x = y;
    }
}

/// `.position` (DSTRATS.ASM:2860-2934): damped spring — child rot eases half-way
/// toward the parent, with a decaying momentum term on al_sbyte1..3.
pub fn chicken_arm_position(g: &mut Game, parent: u16, child: u16) {
    if g.objs.aliens[parent as usize].shape == SH_CHICK_GRABBER2 {
        return chicken_arm_noacc(g, parent, child); // grabber2 = rigid
    }
    // rotx / roty / rotz each: x1 = adiv2(parent - child); child -= x1 + 1
    // (clc+sbc borrow); if parent != child afterward, momentum += x1.
    chicken_arm_position_axis(g, parent, child, 0);
    chicken_arm_position_axis(g, parent, child, 1);
    chicken_arm_position_axis(g, parent, child, 2);
    // Apply + decay the momentum accumulators (DSTRATS.ASM:2920-2934).
    {
        let al = &mut g.objs.aliens[child as usize];
        al.rotx = al.rotx.wrapping_add(al.sbyte1);
        al.roty = al.roty.wrapping_add(al.sbyte2);
        al.rotz = al.rotz.wrapping_add(al.sbyte3);
        al.sbyte1 = adiv2i(al.sbyte1);
        al.sbyte2 = adiv2i(al.sbyte2);
        al.sbyte3 = adiv2i(al.sbyte3);
    }
    chicken_arm_noacc(g, parent, child);
}

#[inline]
fn chicken_arm_position_axis(g: &mut Game, parent: u16, child: u16, axis: u8) {
    let (pr, cr) = {
        let p = g.objs.aliens[parent as usize];
        let c = g.objs.aliens[child as usize];
        match axis {
            0 => (p.rotx, c.rotx),
            1 => (p.roty, c.roty),
            _ => (p.rotz, c.rotz),
        }
    };
    let x1 = adiv2i(pr.wrapping_sub(cr));
    let new_c = cr.wrapping_sub(x1).wrapping_sub(1); // clc then sbc -> -x1 - 1
    {
        let c = &mut g.objs.aliens[child as usize];
        match axis {
            0 => c.rotx = new_c,
            1 => c.roty = new_c,
            _ => c.rotz = new_c,
        }
    }
    if pr != new_c {
        let c = &mut g.objs.aliens[child as usize];
        match axis {
            0 => c.sbyte1 = c.sbyte1.wrapping_add(x1),
            1 => c.sbyte2 = c.sbyte2.wrapping_add(x1),
            _ => c.sbyte3 = c.sbyte3.wrapping_add(x1),
        }
    }
}

/// `.chickenheadcol` (DSTRATS.ASM:2652-2675): a head/tail hit only counts when
/// it faces the player (else nohitaffect graze); routes through hitflash.
pub fn chicken_arm_chickenheadcol(g: &mut Game, idx: u16) {
    let rotx = g.objs.aliens[idx as usize].rotx;
    let base = if (rotx.wrapping_sub(DEG90) as i8) < 0 {
        g.objs.aliens[idx as usize].roty
    } else {
        g.objs.aliens[idx as usize].roty.wrapping_sub(DEG180)
    };
    if base.wrapping_add(128 + 45) >= 90 {
        g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT; // graze
    } else {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT; // counts
    }
    strat_hit_flash(g, idx);
}

/// `.chickenheadhit` (DSTRATS.ASM:2678-2749): restore the head/tail hp and
/// remove the neck piece behind it (shortening the chain by one).
fn chicken_arm_chickenheadhit(g: &mut Game, idx: u16) {
    play_se(g, CHICK_SE_FIRE);
    if g.objs.aliens[idx as usize].shape == SH_CHICK_BOSS_D_0 {
        let hp = g.objs.aliens[idx as usize].hp;
        g.objs.aliens[idx as usize].hp = hp.wrapping_add(CHICKEN_HEAD_HP);
    } else {
        g.objs.aliens[idx as usize].hp = CHICKEN_TAIL_HP + 64;
    }
    let Some(parent) = chicken_find_alptr(g, idx) else {
        return;
    };
    if g.objs.aliens[parent as usize].shape == CH_BOSS_D_1 {
        return; // parent is the body -> nothing to remove
    }
    let grandparent = chicken_find_alptr(g, parent);
    // Remove the parent neck piece.
    chicken_remove_alptrs(g, parent);
    g.objs.aliens[parent as usize].ptr = 0;
    g.objs.aliens[parent as usize].hp = 0;
    g.objs.aliens[parent as usize].active = false; // kill/remove
    // Relink so the head/tail attaches to the grandparent (or the body).
    match grandparent {
        Some(gp) if gp != idx && g.objs.aliens[gp as usize].shape != CH_BOSS_D_1 => {
            g.objs.aliens[gp as usize].ptr = boss_obj_index_or_null(idx);
        }
        _ => {
            // .mainbody: relink the mother's chain slot to the head/tail.
            let Some(mother) = chicken_find_shape(g, CH_BOSS_D_1) else {
                return;
            };
            if g.objs.aliens[idx as usize].shape == SH_CHICK_BOSS_D_2 {
                g.objs.aliens[mother as usize].sword2 = boss_obj_index_or_null(idx) as i16;
            } else if g.objs.aliens[mother as usize].ptr == 0 {
                g.objs.aliens[mother as usize].ptr = boss_obj_index_or_null(idx);
            } else {
                g.objs.aliens[mother as usize].sword1 = boss_obj_index_or_null(idx) as i16;
            }
        }
    }
}

/// `.grabberhit` (DSTRATS.ASM:2752-2772): a laser hit on a forward-facing
/// grabber latches the damage up the chain.
pub fn chicken_arm_grabberhit(g: &mut Game, idx: u16) {
    let roty = g.objs.aliens[idx as usize].roty;
    if roty.wrapping_add(128 + 45) >= 90 {
        return;
    }
    let rotx = g.objs.aliens[idx as usize].rotx;
    if rotx.wrapping_add(45) >= 90 {
        return;
    }
    // Only lasers (s_jmpNOT_colltype y,laser).
    if let Some(c) = fb_read_obj(g, g.objs.aliens[idx as usize].collobjptr) {
        if g.objs.aliens[c as usize].type_ & ATLASER != 0 {
            chicken_arm_passiton(g, idx);
        }
    }
}

/// `.passiton` (DSTRATS.ASM:2775-2791): propagate the damage latch (sflag5) UP —
/// to the arm/bulge parent (turning it into a travelling bulge) or, for a chain
/// root, to the `#flingboss` mother.
pub fn chicken_arm_passiton(g: &mut Game, idx: u16) {
    let y = if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 != 0 {
        chicken_find_alptr(g, idx) // .tisok — parent
    } else {
        chicken_find_shape(g, SH_FLINGBOSS_BODY) // mother
    };
    let Some(y) = y else {
        return;
    };
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 != 0 {
        let ps = g.objs.aliens[y as usize].shape;
        if ps == SH_CHICK_ARM || ps == SH_CHICK_BULGE {
            if g.objs.aliens[y as usize].sflags3 & CH_SFLAG5 != 0 {
                return; // .dont2 — already latched
            }
            g.objs.aliens[y as usize].animframe = 3;
            g.objs.aliens[y as usize].shape = SH_CHICK_BULGE;
        }
    }
    g.objs.aliens[y as usize].sflags3 |= CH_SFLAG5; // .justset
}

/// `sprouty.expl` (DSTRATS.ASM:2361-2384, simplified): unlink + remove the
/// segment (the falling-tentacle chain is scoped out — see the section note).
fn chicken_arm_sprouty_expl(g: &mut Game, idx: u16) {
    chicken_remove_alptrs(g, idx);
    g.objs.aliens[idx as usize].hp = 0;
    g.objs.aliens[idx as usize].active = false;
}

/// arm HPLASMA fire (homingflat, GSTRATS.ASM:1723 = ported `boss2_hplasma_strat`).
fn chicken_arm_fire_hplasma(g: &mut Game, idx: u16, muzzle_z: i16) {
    let me = g.objs.aliens[idx as usize];
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        me.rotx,
        me.roty,
        CHICK_HPLASMA_SPEED,
        CHICK_HPLASMA_LIFE,
        CHICK_HPLASMA_AP,
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    b2_full_offset_pos(g, shot, &me, 0, 0, muzzle_z);
    let s = sid(g, boss2_hplasma_strat);
    let ppt = player_idx(g).map(boss_obj_index_or_null).unwrap_or(0);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = me.rotz;
    al.collflags |= COLLTYPE_ENEMY1;
    al.ptr = ppt;
    al.fireobjptr = ppt;
    al.stratptr = Some(s);
    strat_gen_vecs_3d(al);
}

// ---- scoped sub-object movers (see the section note) ----

/// firebreathe_istrat (DSTRATS.ASM:4629-4699) — straight fireball mover.
fn chicken_firebreath_strat(g: &mut Game, idx: u16) {
    chicken_gen3dvecs(g, idx);
    add_player_z(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    if !sea_dz_less(g, idx, 4000) {
        g.objs.free(idx);
    }
}

/// egg_istrat (DSTRATS.ASM:4528-4574) — falls, detonates on landing.
fn chicken_egg_strat(g: &mut Game, idx: u16) {
    let landed = boss2_falldown_yvec(g, idx, 2, 4, 0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(al.vy);
    }
    add_player_z(g, idx);
    if landed {
        play_se(g, CHICK_SE_EGG);
        strat_explode(g, idx);
    }
}

/// wings_istrat init (DSTRATS.ASM:4744-4752).
fn chicken_wings_strat_init(g: &mut Game, idx: u16) {
    let s = sid(g, chicken_wings_strat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.expstratptr = Some(s_exp);
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_NOHITAFFECT | ASF_COLLDISABLE | ASF_SHADOW;
        al.sflags &= !ASF_INVISIBLE;
        al.type_ &= !ATZREMOVE;
        al.animframe = 0;
    }
    chicken_wings_strat(g, idx);
}

/// wings_istrat `.strat` (DSTRATS.ASM:4753-4761): fold (sflag1) or flap.
fn chicken_wings_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 != 0 {
        let a = g.objs.aliens[idx as usize].animframe;
        if a > 0 {
            g.objs.aliens[idx as usize].animframe = a - 1;
        }
    } else {
        let na = (g.objs.aliens[idx as usize].animframe + 1) % 15;
        g.objs.aliens[idx as usize].animframe = if na == 14 { 4 } else { na };
    }
}

/// s_gen_3dvecs x,al_roty,al_rotx,al_vel (DSTRATS.ASM:475 dgen3dvecs).
fn chicken_gen3dvecs(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    let ry = (al.roty as f32) * (2.0 * std::f32::consts::PI / 256.0);
    let rx = (al.rotx as f32) * (2.0 * std::f32::consts::PI / 256.0);
    let sp = (al.vel as i8) as f32;
    let a = &mut g.objs.aliens[idx as usize];
    a.vx = (sp * ry.sin() * rx.cos()) as i16;
    a.vy = (sp * (-rx.sin())) as i16;
    a.vz = (sp * ry.cos() * rx.cos()) as i16;
}

/// s_gen_vecs x,al_roty,al_vel (flat; vx/vz only, vy untouched).
fn chicken_gen_vecs_roty(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    let rad = (al.roty as f32) * (2.0 * std::f32::consts::PI / 256.0);
    let sp = (al.vel as i8) as f32;
    let a = &mut g.objs.aliens[idx as usize];
    a.vx = (sp * rad.sin()) as i16;
    a.vz = (sp * rad.cos()) as i16;
}

// ============================================================
// chicken_istrat mother (DSTRATS.ASM:3696-4523).
// ============================================================

const CHICK_UPDOWNTAB: [(i8, i8); 13] = [
    (0, 0),
    (-10, 0),
    (-14, 0),
    (-10, 0),
    (0, 0),
    (-10, 0),
    (-14, 0),
    (-10, 0),
    (7, 5),
    (15, 10),
    (-2, -14),
    (4, -27),
    (15, -35),
];

/// `.addtabtoy` (DSTRATS.ASM:4456-4473): add the body-animation offset row to
/// (y1,z1).
fn chicken_addtabtoy(g: &Game, mother: u16, y1: i16, z1: i16) -> (i16, i16) {
    let i = (g.objs.aliens[mother as usize].animframe & 15) as usize;
    let (a, b) = if i < 13 { CHICK_UPDOWNTAB[i] } else { (0, 0) };
    (y1.wrapping_add(a as i16), z1.wrapping_add(b as i16))
}

/// `.position1` (DSTRATS.ASM:4333-4364) — left neck (al_ptr).
fn chicken_position1(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        c.roty = m.roty.wrapping_sub(m.sbyte3).wrapping_sub(CHICK_DEG11);
        c.rotx = m
            .rotx
            .wrapping_add(m.sbyte2)
            .wrapping_add(m.sbyte3)
            .wrapping_add(m.rotz)
            .wrapping_sub(DEG90 + DEG45 + DEG22); // 112
        c.rotz = m.rotz.wrapping_add(DEG180);
    }
    let (y1, z1) = chicken_addtabtoy(g, mother, -50, -15);
    fb_positional(g, &m, child, -10, y1, z1);
}

/// `.position2` (DSTRATS.ASM:4366-4392) — right neck (al_sword1).
fn chicken_position2(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        c.roty = m.roty.wrapping_add(m.sbyte3).wrapping_add(DEG180 + CHICK_DEG11); // 136
        c.rotx = m
            .rotx
            .wrapping_sub(m.sbyte4)
            .wrapping_sub(m.sbyte3)
            .wrapping_add(m.rotz)
            .wrapping_sub(DEG90 - DEG45 - DEG22); // 16
        c.rotz = m.rotz;
    }
    let (y1, z1) = chicken_addtabtoy(g, mother, -50, -15);
    fb_positional(g, &m, child, 10, y1, z1);
}

/// `.position3` (DSTRATS.ASM:4395-4436) — tail (al_sword2).
fn chicken_position3(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    let y1i = ((g.vars.gameframe << 3) & 0xff) as usize;
    let x1 = adiv_n(crate::snes_trig::SINTAB[y1i] as u8, 2);
    {
        let c = &mut g.objs.aliens[child as usize];
        c.roty = m.roty.wrapping_add(x1);
        c.rotx = m.rotx.wrapping_add(m.rotz).wrapping_add(CHICK_DEG11);
        c.rotz = m.rotz;
    }
    if (x1 == 0 || x1 == 128) && sea_dz_less(g, mother, 2000) {
        play_se(g, CHICK_SE_WHOOSH);
    }
    let (y1, z1) = chicken_addtabtoy(g, mother, -25, 15);
    fb_positional(g, &m, child, 0, y1, z1);
}

/// `.position4`/`.position5` (DSTRATS.ASM:4438-4454) — the two wings.
fn chicken_position4(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    chicken_copyrots(g, child, mother);
    let (y1, z1) = chicken_addtabtoy(g, mother, 0, 0);
    fb_positional(g, &m, child, -18, y1, z1);
}
fn chicken_position5(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    chicken_copyrots(g, child, mother);
    let (y1, z1) = chicken_addtabtoy(g, mother, 0, 0);
    fb_positional(g, &m, child, 18, y1, z1);
}

/// `.generate` (DSTRATS.ASM:4268-4324): spawn the 3 neck chain roots + 2 wings.
/// Returns true on FAILURE (bcs .end -> re-init next tick).
fn chicken_generate(g: &mut Game, idx: u16) -> bool {
    // left neck (al_ptr)
    if g.objs.aliens[idx as usize].ptr == 0 {
        let Some(y) = chicken_makeobj(g, idx) else {
            return true;
        };
        let s = sid(g, chicken_arm_init);
        {
            let al = &mut g.objs.aliens[y as usize];
            al.stratptr = Some(s);
            al.shape = SH_CHICK_NECK;
            al.sword1 = 1;
        }
        g.objs.aliens[idx as usize].ptr = boss_obj_index_or_null(y);
        chicken_position1(g, idx, y);
    }
    // right neck (al_sword1)
    if g.objs.aliens[idx as usize].sword1 == 0 {
        let Some(y) = chicken_makeobj(g, idx) else {
            return true;
        };
        let s = sid(g, chicken_arm_init);
        {
            let al = &mut g.objs.aliens[y as usize];
            al.stratptr = Some(s);
            al.shape = SH_CHICK_NECK;
            al.sword1 = 1;
        }
        g.objs.aliens[idx as usize].sword1 = boss_obj_index_or_null(y) as i16;
        chicken_position2(g, idx, y);
    }
    // tail (al_sword2) — no shape (default) so growth makes a tail (boss_d_2).
    if g.objs.aliens[idx as usize].sword2 == 0 {
        let Some(y) = chicken_makeobj(g, idx) else {
            return true;
        };
        let s = sid(g, chicken_arm_init);
        {
            let al = &mut g.objs.aliens[y as usize];
            al.stratptr = Some(s);
            al.sword1 = 1;
        }
        g.objs.aliens[idx as usize].sword2 = boss_obj_index_or_null(y) as i16;
        chicken_position3(g, idx, y);
    }
    // wing1 (guarded by sflag1 so a retry doesn't double-spawn).
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 == 0 {
        let Some(y) = chicken_makeobj(g, idx) else {
            return true;
        };
        let s = sid(g, chicken_wings_strat_init);
        {
            let al = &mut g.objs.aliens[y as usize];
            al.shape = SH_CHICK_BOSS_D_8;
            al.stratptr = Some(s);
        }
        g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG1;
        g.objs.aliens[idx as usize].swpx1 = boss_obj_index_or_null(y) as i16;
        chicken_position4(g, idx, y);
    }
    // wing2
    {
        let Some(y) = chicken_makeobj(g, idx) else {
            return true;
        };
        let s = sid(g, chicken_wings_strat_init);
        {
            let al = &mut g.objs.aliens[y as usize];
            al.shape = SH_CHICK_BOSS_D_9;
            al.stratptr = Some(s);
        }
        g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG2;
        g.objs.aliens[idx as usize].swpy1 = boss_obj_index_or_null(y) as i16;
        chicken_position5(g, idx, y);
    }
    g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG1; // clr sflag1 (leaves sflag2 set)
    false
}

/// `chicken_istrat` init (DSTRATS.ASM:3696-3717).
pub fn strat_chicken_init(g: &mut Game, idx: u16) {
    set_bossmaxhp(g, CHICKEN_BODY_HP as u16); // m_bossmaxHP = 64
    set_armmode(g, 128);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, chicken_chickenexplode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = CHICKEN_BODY_HP;
        al.ap = CHICKEN_BODY_AP;
        al.count = 16;
        al.count1 = 0;
        al.sflags3 |= CH_SFLAG6;
        al.type_ &= !ATZREMOVE;
        al.animframe = 0;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    if chicken_generate(g, idx) {
        return; // pool full -> re-init next tick (stratptr unchanged)
    }
    let s = sid(g, chicken_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].stratstate = 0; // s_mode_change x,#0
    chicken_strat(g, idx);
}

/// nxtmode_srou (DSTRATS.ASM:3782-3785): re-arm the mode dispatch + mode += 1.
fn chicken_nxtmode(g: &mut Game, idx: u16) {
    let s = sid(g, chicken_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
}

/// `chicken_istrat` `.strat` — the mode-table dispatch (DSTRATS.ASM:3720-3763).
fn chicken_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        0 | 7 | 16 => chicken_mode_foldwings(g, idx),
        1 | 22 => chicken_mode_sitdown(g, idx),
        2 | 15 => chicken_mode_waitfor600z(g, idx),
        3 | 17 | 24 | 31 => chicken_mode_getup(g, idx),
        4 | 12 | 30 => chicken_mode_unfoldwings(g, idx),
        5 | 8 | 27 | 33 => chicken_mode_cometurn(g, idx),
        6 | 19 | 28 | 34 => chicken_mode_goturn(g, idx),
        9 | 20 => chicken_mode_startturn(g, idx),
        10 | 21 => chicken_mode_endturn(g, idx),
        11 => chicken_mode_moveabit(g, idx),
        13 | 18 | 26 | 32 => chicken_mode_waitabit(g, idx),
        14 => chicken_mode_jump(g, idx),
        23 => chicken_mode_fireblaze(g, idx),
        25 => chicken_mode_attack(g, idx),
        35 => chicken_mode_flyflyaway(g, idx),
        36 => chicken_mode_cometurnfly(g, idx),
        // 29 .repeat + any overflow -> s_mode_change x,#0, re-dispatch.
        _ => {
            g.objs.aliens[idx as usize].stratstate = 0;
            chicken_mode_foldwings(g, idx);
        }
    }
}

// ---- mode handlers (DSTRATS.ASM:3766-3993) ----

fn chicken_getwing1(g: &Game, idx: u16) -> Option<u16> {
    fb_read_obj(g, g.objs.aliens[idx as usize].swpx1 as u16)
}
fn chicken_getwing2(g: &Game, idx: u16) -> Option<u16> {
    fb_read_obj(g, g.objs.aliens[idx as usize].swpy1 as u16)
}

fn chicken_mode_foldwings(g: &mut Game, idx: u16) {
    if let Some(w) = chicken_getwing1(g, idx) {
        g.objs.aliens[w as usize].sflags2 |= CH_SFLAG1;
    }
    if let Some(w) = chicken_getwing2(g, idx) {
        g.objs.aliens[w as usize].sflags2 |= CH_SFLAG1;
    }
    chicken_nxtmode(g, idx);
    chicken_nomove(g, idx);
}
fn chicken_mode_unfoldwings(g: &mut Game, idx: u16) {
    if let Some(w) = chicken_getwing1(g, idx) {
        g.objs.aliens[w as usize].sflags2 &= !CH_SFLAG1;
    }
    if let Some(w) = chicken_getwing2(g, idx) {
        g.objs.aliens[w as usize].sflags2 &= !CH_SFLAG1;
    }
    chicken_nxtmode(g, idx);
    chicken_nomove(g, idx);
}

/// `.cometurn` (DSTRATS.ASM:3771-3777): advance only when turning finished
/// (sflag3 set, sflag1 clear); always `.move`.
fn chicken_mode_cometurn(g: &mut Game, idx: u16) {
    let f = g.objs.aliens[idx as usize].sflags2;
    if f & CH_SFLAG3 != 0 && f & CH_SFLAG1 == 0 {
        chicken_nxtmode(g, idx);
    }
    chicken_move(g, idx);
}
/// `.goturn` (DSTRATS.ASM:3788-3791).
fn chicken_mode_goturn(g: &mut Game, idx: u16) {
    let f = g.objs.aliens[idx as usize].sflags2;
    if f & CH_SFLAG3 == 0 && f & CH_SFLAG1 == 0 {
        chicken_nxtmode(g, idx);
    }
    chicken_move(g, idx);
}
/// `.startturn` (DSTRATS.ASM:3902-3904): toggle sflag3, advance, `.move`.
fn chicken_mode_startturn(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 ^= CH_SFLAG3;
    chicken_nxtmode(g, idx);
    chicken_move(g, idx);
}
/// `.endturn` (DSTRATS.ASM:3907-3909).
fn chicken_mode_endturn(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 == 0 {
        chicken_nxtmode(g, idx);
    }
    chicken_move(g, idx);
}
/// `.moveabit` (DSTRATS.ASM:3834-3840).
fn chicken_mode_moveabit(g: &mut Game, idx: u16) {
    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb >= 20 {
        chicken_nxtmode(g, idx);
    }
    chicken_move(g, idx);
}
/// `.waitfor600z` (DSTRATS.ASM:3867-3873).
fn chicken_mode_waitfor600z(g: &mut Game, idx: u16) {
    if sea_dz_less(g, idx, 600) {
        chicken_nxtmode(g, idx);
    }
    chicken_nomove(g, idx);
}
/// `.sitdown` (DSTRATS.ASM:3912-3914): sit animation caps at 12 then advances
/// (5-arg firstframe clamp read as cap→jump — see section note).
fn chicken_mode_sitdown(g: &mut Game, idx: u16) {
    let a = g.objs.aliens[idx as usize].animframe;
    if a + 1 >= 13 {
        g.objs.aliens[idx as usize].animframe = 12;
        chicken_nxtmode(g, idx);
    } else {
        g.objs.aliens[idx as usize].animframe = a + 1;
    }
    chicken_nomove(g, idx);
}
/// `.getup` (DSTRATS.ASM:3916-3923).
fn chicken_mode_getup(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].animframe < 7 {
        chicken_nxtmode(g, idx);
        chicken_nomove(g, idx);
        return;
    }
    let a = g.objs.aliens[idx as usize].animframe;
    let na = (a + 12 - 1) % 12; // ddecanim #12
    g.objs.aliens[idx as usize].animframe = na;
    if na == 7 {
        chicken_nxtmode(g, idx);
    }
    chicken_nomove(g, idx);
}

/// `.waitabit` (DSTRATS.ASM:3842-3851).
fn chicken_mode_waitabit(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.sbyte2 = 0;
        al.sbyte4 = 0;
        al.sbyte1 = 10;
    }
    let am = armmode(g) & 0xFE; // s_and_var armmode,#-2
    set_armmode(g, am);
    let s = sid(g, chicken_waitabit_strat4);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_waitabit_strat4(g, idx);
}
fn chicken_waitabit_strat4(g: &mut Game, idx: u16) {
    // s_beqdec_alvar sbyte1 -> .nxtmode2 (branch when already 0).
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        chicken_nxtmode(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
    }
    chicken_nomove(g, idx);
}

/// `.fireblaze` (DSTRATS.ASM:3854-3864).
fn chicken_mode_fireblaze(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 ^= CH_SFLAG4;
    let am = armmode(g) | 1; // s_or_var armmode,#1
    set_armmode(g, am);
    let s = sid(g, chicken_fireblaze_strat5);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_fireblaze_strat5(g, idx);
}
fn chicken_fireblaze_strat5(g: &mut Game, idx: u16) {
    if sea_dz_less(g, idx, 1500) {
        chicken_nxtmode(g, idx);
    }
    chicken_nomove(g, idx);
}

/// `.jump` (DSTRATS.ASM:3808-3830).
fn chicken_mode_jump(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].animframe = 7;
    let s = sid(g, chicken_jump_strat2);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_jump_strat2(g, idx);
}
fn chicken_jump_strat2(g: &mut Game, idx: u16) {
    // s_add_anim x,#8,#10,.stopanim (cap 9 + jump).
    {
        let al = &mut g.objs.aliens[idx as usize];
        let na = al.animframe.wrapping_add(8);
        al.animframe = if na >= 10 { 9 } else { na };
    }
    if sea_dz_less(g, idx, 600) {
        chicken_jump_leap(g, idx);
    } else {
        chicken_nomove(g, idx);
    }
}
fn chicken_jump_leap(g: &mut Game, idx: u16) {
    play_se(g, CHICK_SE_FIRE);
    let s = sid(g, chicken_jump_strat3);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.vy = -60;
    }
    chicken_jump_strat3(g, idx);
}
fn chicken_jump_strat3(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(40);
        al.worldy = al.worldy.wrapping_add(al.vy);
    }
    let landed = boss2_falldown_yvec(g, idx, 2, 4, 0); // dfallyvec_x
    if landed {
        chicken_nxtmode(g, idx);
    }
    chicken_nomove(g, idx);
}

/// `.attack` (DSTRATS.ASM:3875-3899): swipe with the wings.
fn chicken_mode_attack(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = 200;
    let s = sid(g, chicken_attack_strat6);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_attack_strat6(g, idx);
}
fn chicken_attack_strat6(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 31 == 0 {
        g.objs.aliens[idx as usize].sflags3 ^= CH_SFLAG5;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = (al.animframe + 8 - 1) % 8; // ddecanim #8
    }
    add_player_z(g, idx);
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        chicken_nxtmode(g, idx);
        chicken_move(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if g.objs.aliens[idx as usize].sbyte1 < 160 {
        // .strike — set the two wing swipe angles by sflag5.
        let al = &mut g.objs.aliens[idx as usize];
        if al.sflags3 & CH_SFLAG5 != 0 {
            al.sbyte2 = (-(DEG22 as i8)) as u8;
            al.sbyte4 = DEG22;
        } else {
            al.sbyte4 = (-(DEG22 as i8)) as u8;
            al.sbyte2 = DEG22;
        }
    }
    chicken_nomove(g, idx);
}

// ---- flyaway modes (DSTRATS.ASM:3942-3993 — unreachable, ported for parity) ----

fn chicken_mode_flyflyaway(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = -70;
        al.sbyte1 = 20;
    }
    let s = sid(g, chicken_flyflyaway_strat7);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_flyflyaway_strat7(g, idx);
}
fn chicken_flyflyaway_strat7(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(40);
        al.worldy = al.worldy.wrapping_add(al.vy);
    }
    add_player_z(g, idx);
    boss2_falldown_yvec(g, idx, 2, 4, 0);
    // vy ramp toward -60 (DSTRATS.ASM:3953-3967) — cosmetic.
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut v = (al.vy as i8).wrapping_sub(20);
        if v >= 0 || v < -60 {
            v = -60;
        }
        if !(al.worldy < 0 && al.worldy >= -100) {
            al.vy = v as i16;
        }
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        chicken_nxtmode(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
    }
    chicken_nomove(g, idx);
}
fn chicken_mode_cometurnfly(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].vel = (-20i8) as u8;
    let s = sid(g, chicken_cometurnfly_strat8);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    chicken_cometurnfly_strat8(g, idx);
}
fn chicken_cometurnfly_strat8(g: &mut Game, idx: u16) {
    if (g.objs.aliens[idx as usize].vel as i8) != -80 {
        g.objs.aliens[idx as usize].vel = (g.objs.aliens[idx as usize].vel as i8).wrapping_sub(1) as u8;
    }
    g.objs.aliens[idx as usize].sbyte1 = 10;
    chicken_gen3dvecs(g, idx);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let ry = g.objs.aliens[idx as usize].roty;
    if ry != 0 {
        g.objs.aliens[idx as usize].roty = ry.wrapping_sub(8);
    }
    chicken_nomove(g, idx);
}

// ---- movement + the common .move/.nomove tails (DSTRATS.ASM:4002-4029) ----

/// `.movebackandforth` (DSTRATS.ASM:4196-4223): oscillate between z≈2500 and
/// z≈600, turning to face the player.
fn chicken_movebackandforth(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = (al.animframe + 1) % 8; // dincanim #8
    }
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG3 != 0 {
        g.objs.aliens[idx as usize].vel = (-20i8) as u8;
        chicken_needtorotate(g, idx, true);
        if !sea_dz_less(g, idx, 2500) {
            g.objs.aliens[idx as usize].sflags2 ^= CH_SFLAG3; // dz>=2500 -> flip
        }
    } else {
        g.objs.aliens[idx as usize].vel = (-40i8) as u8;
        chicken_needtorotate(g, idx, false);
        if sea_dz_less(g, idx, 600) {
            g.objs.aliens[idx as usize].sflags2 ^= CH_SFLAG3; // dz<600 -> flip
        }
    }
    chicken_gen_vecs_roty(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(al.vz);
    al.worldx = al.worldx.wrapping_add(al.vx);
}

/// s_leftview_strat approximation (see section note): player to the left.
fn chicken_leftview(g: &Game, idx: u16) -> bool {
    player(g)
        .map(|p| g.objs.aliens[idx as usize].worldx > p.worldx)
        .unwrap_or(false)
}
fn chicken_leftviewchk(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG1;
    if chicken_leftview(g, idx) {
        g.objs.aliens[idx as usize].sflags2 |= CH_SFLAG2; // .otherway
    } else {
        g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG2;
    }
}
/// `.needtorotate1/2` + `.rotate` (DSTRATS.ASM:4233-4265).
fn chicken_needtorotate(g: &mut Game, idx: u16, mode1: bool) {
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG1 == 0 {
        let roty = g.objs.aliens[idx as usize].roty;
        let done = if mode1 { roty == DEG180 } else { roty == 0 };
        if done {
            return; // .yohohoandabarrelofrum
        }
        chicken_leftviewchk(g, idx);
        if mode1 {
            g.objs.aliens[idx as usize].sflags2 ^= CH_SFLAG2; // needtorotate1 extra
        }
    }
    // .rotate
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG2 != 0 {
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_sub(CHICK_DEG11);
    } else {
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(CHICK_DEG11);
    }
    if g.objs.aliens[idx as usize].roty & 127 == 0 {
        g.objs.aliens[idx as usize].sflags2 &= !CH_SFLAG1;
    }
}

/// `.move` (DSTRATS.ASM:4002-4004).
fn chicken_move(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    chicken_movebackandforth(g, idx);
    chicken_nomove(g, idx);
}
/// `.nomove` (DSTRATS.ASM:4006-4029): triggeregg + waveheads + position 1..5 +
/// check_fin + regrownecks + HP bar.
fn chicken_nomove(g: &mut Game, idx: u16) {
    chicken_triggeregg(g, idx);
    // .nomove2
    chicken_waveheads(g, idx);
    if let Some(c) = fb_read_obj(g, g.objs.aliens[idx as usize].ptr) {
        chicken_position1(g, idx, c);
    }
    if let Some(c) = fb_read_obj(g, g.objs.aliens[idx as usize].sword1 as u16) {
        chicken_position2(g, idx, c);
    }
    if let Some(c) = fb_read_obj(g, g.objs.aliens[idx as usize].sword2 as u16) {
        chicken_position3(g, idx, c);
    }
    if let Some(w) = chicken_getwing1(g, idx) {
        chicken_position4(g, idx, w);
    }
    if let Some(w) = chicken_getwing2(g, idx) {
        chicken_position5(g, idx, w);
    }
    chicken_check_fin(g, idx);
    chicken_regrownecks(g, idx);
    add_bosshp(g, idx); // s_add_bossHP x,al_hp
}

/// `.waveheads` (DSTRATS.ASM:4326-4332).
fn chicken_waveheads(g: &mut Game, idx: u16) {
    let y1 = ((g.vars.gameframe << 2) & 0xff) as usize;
    let v = crate::snes_trig::SINTAB[y1] as u8;
    g.objs.aliens[idx as usize].sbyte3 = adiv_n(v, 3);
}

/// `.triggeregg` (DSTRATS.ASM:4489-4503).
fn chicken_triggeregg(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & CH_SFLAG3 == 0 {
        return;
    }
    // s_jmp_random .dontdo,99 — fire only ~1.6% of ticks.
    if (sfrtl_random(g) as u8) < ((99u16 * 255 / 100) as u8) {
        return;
    }
    if let Some(y) = fb_read_obj(g, g.objs.aliens[idx as usize].sword2 as u16) {
        if g.objs.aliens[y as usize].sflags2 & CH_SFLAG2 != 0 {
            return;
        }
        g.objs.aliens[y as usize].sflags2 |= CH_SFLAG2;
    }
}

/// `.check_fin` (DSTRATS.ASM:4031-4079): the mother becomes vulnerable + flashes
/// red once a neck has grown its head/tail.
fn chicken_check_fin(g: &mut Game, idx: u16) {
    let m = g.objs.aliens[idx as usize];
    let tail_grown = fb_read_obj(g, m.sword2 as u16)
        .map(|c| g.objs.aliens[c as usize].shape == SH_CHICK_BOSS_D_2)
        .unwrap_or(false);
    let both_heads = fb_read_obj(g, m.ptr)
        .map(|c| g.objs.aliens[c as usize].shape == SH_CHICK_BOSS_D_0)
        .unwrap_or(false)
        && fb_read_obj(g, m.sword1 as u16)
            .map(|c| g.objs.aliens[c as usize].shape == SH_CHICK_BOSS_D_0)
            .unwrap_or(false);
    if tail_grown || both_heads {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT; // vulnerable
        if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG6 == 0 {
            g.objs.aliens[idx as usize].count = 56; // red time
            g.objs.aliens[idx as usize].count1 = g.objs.aliens[idx as usize].count1.wrapping_add(1);
        }
        g.objs.aliens[idx as usize].sflags3 |= CH_SFLAG6;
    } else {
        g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT; // invulnerable
    }
    // Red/normal coltab flash (gf&1) is cosmetic — omitted.
}

/// `.regrownecks` (DSTRATS.ASM:4083-4123).
fn chicken_regrownecks(g: &mut Game, idx: u16) {
    // s_decbne_alvar count -> .nogrow (dec; return while != 0).
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = c;
    if c != 0 {
        return;
    }
    // Reset the interval by sflag6 (fast/slow), then grow the 3 chains.
    if g.objs.aliens[idx as usize].sflags3 & CH_SFLAG6 != 0 {
        g.objs.aliens[idx as usize].count = 4; // fast
    } else {
        g.objs.aliens[idx as usize].count = 80; // slow
    }
    let mut finished = 0u8;
    finished += chicken_regrowneck(g, idx, ChickenSlot::Ptr, 3);
    finished += chicken_regrowneck(g, idx, ChickenSlot::Sword1, 3);
    finished += chicken_regrowneck(g, idx, ChickenSlot::Sword2, 5);
    if finished == 3 {
        g.objs.aliens[idx as usize].sflags3 &= !CH_SFLAG6; // slow the grow rate
    }
}

#[derive(Clone, Copy)]
enum ChickenSlot {
    Ptr,
    Sword1,
    Sword2,
}
fn chicken_slot_get(g: &Game, idx: u16, slot: ChickenSlot) -> u16 {
    match slot {
        ChickenSlot::Ptr => g.objs.aliens[idx as usize].ptr,
        ChickenSlot::Sword1 => g.objs.aliens[idx as usize].sword1 as u16,
        ChickenSlot::Sword2 => g.objs.aliens[idx as usize].sword2 as u16,
    }
}
fn chicken_slot_set(g: &mut Game, idx: u16, slot: ChickenSlot, raw: u16) {
    match slot {
        ChickenSlot::Ptr => g.objs.aliens[idx as usize].ptr = raw,
        ChickenSlot::Sword1 => g.objs.aliens[idx as usize].sword1 = raw as i16,
        ChickenSlot::Sword2 => g.objs.aliens[idx as usize].sword2 = raw as i16,
    }
}

/// `.regrowneck` (DSTRATS.ASM:4125-4192): walk one chain, and if it is shorter
/// than its target length, insert a fresh neck/arm segment before the terminus.
/// Returns 1 when the chain is already long enough (`.clrtheflag`).
fn chicken_regrowneck(g: &mut Game, idx: u16, slot: ChickenSlot, target_base: u8) -> u8 {
    let count1 = g.objs.aliens[idx as usize].count1;
    let t = (target_base as i16) - (count1 as i16);
    let target = if t < 2 { 2u16 } else { t as u16 };
    // Walk to the head/tail terminus, counting the neck length.
    let mut cur = fb_read_obj(g, chicken_slot_get(g, idx, slot));
    let mut len = 0u16;
    let mut last: Option<u16> = None;
    let mut terminus: Option<u16> = None;
    while let Some(c) = cur {
        let sh = g.objs.aliens[c as usize].shape;
        if sh == SH_CHICK_BOSS_D_0 || sh == SH_CHICK_BOSS_D_2 {
            terminus = Some(c);
            break;
        }
        len += 1;
        last = Some(c);
        cur = fb_read_obj(g, g.objs.aliens[c as usize].ptr);
    }
    let Some(term) = terminus else {
        return 0; // still growing its terminus — nothing to regrow yet
    };
    if len >= target {
        return 1; // .clrtheflag
    }
    let Some(y) = chicken_makeobj(g, idx) else {
        return 0; // .okdone (pool full)
    };
    let s = sid(g, chicken_arm_init);
    g.objs.aliens[y as usize].stratptr = Some(s);
    g.objs.aliens[y as usize].shape = if g.objs.aliens[term as usize].shape == SH_CHICK_BOSS_D_0 {
        SH_CHICK_NECK
    } else {
        SH_CHICK_ARM
    };
    if len == 0 {
        // .mainbody: insert between the mother slot and the terminus.
        g.objs.aliens[y as usize].ptr = boss_obj_index_or_null(term);
        chicken_slot_set(g, idx, slot, boss_obj_index_or_null(y));
    } else {
        // insert between the last neck and the terminus.
        let last = last.unwrap();
        g.objs.aliens[y as usize].ptr = boss_obj_index_or_null(term);
        g.objs.aliens[last as usize].ptr = boss_obj_index_or_null(y);
        chicken_copyrots(g, y, last);
        copy_pos(g, y, last);
    }
    0
}

/// `.chickenexplode` (DSTRATS.ASM:4505-4519): kill every chain + wing, then
/// hand off to bossexplode.
pub fn chicken_chickenexplode(g: &mut Game, idx: u16) {
    chicken_kill_alptr_list(g, g.objs.aliens[idx as usize].ptr);
    chicken_kill_alptr_list(g, g.objs.aliens[idx as usize].sword1 as u16);
    chicken_kill_alptr_list(g, g.objs.aliens[idx as usize].sword2 as u16);
    if let Some(w) = chicken_getwing1(g, idx) {
        g.objs.free(w);
    }
    if let Some(w) = chicken_getwing2(g, idx) {
        g.objs.free(w);
    }
    let s = sid(g, strat_boss_explode_init);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    strat_boss_explode_init(g, idx); // bossexplode_istrat
}
/// kill_alptr_list_y_l (DSTRATS.ASM:4891): free the al_ptr chain from `start`.
fn chicken_kill_alptr_list(g: &mut Game, start_raw: u16) {
    let mut cur = fb_read_obj(g, start_raw);
    while let Some(c) = cur {
        let next = fb_read_obj(g, g.objs.aliens[c as usize].ptr);
        g.objs.free(c);
        cur = next;
    }
}
// CHICKEN_END

// ============================================================
// SEADRAGON_BEGIN — seadragon / seadragon2 / lochnessmonster (Route 3 L3)
// ASM oracle: DSTRATS.ASM:1926-2395 (`lochnessmonster_istrat` :1926 /
// `seadragon2_istrat` :1931 / `seadragon_istrat` :1934 / `seadragon_istrat2`
// :1950 and the shared `sprouty` growth machine `sprout2_istrat`/`sprouty`
// :2093-2395) + D2STRATS.ASM:732-861 (`snake_istrat`, the fire-breathing
// head). Constants: D2STRATS.ASM:29-30, DSTRATS.ASM:57/100-101,
// STRATEQU.INC:66-68/980. Macro semantics per docs/AUDIT_BOSS_TICKS2_FINDINGS.
//
// MECHANISM (how it reuses / differs from the ported worm):
//  * The ported `worm`/`worm2` (enemy_a.rs) is a SELF-CONTAINED splitter — it
//    does NOT use `sproutstrat`. The sea dragon instead uses the engine's
//    `sprouty` SEGMENT-GROWTH primitive (a vertical al_ptr chain), the same
//    label family flingboss/chicken arms lean on. That primitive was not yet
//    ported (flingboss/chicken ported the separate `arm_istrat`), so the
//    seadragon-relevant subset is ported fresh here and linked exactly like
//    the flingboss/chicken child chains: `al_ptr` (raw u16, index+1; 0 = none,
//    0xFFFF = "grew past the play field / topmost").
//  * A map-placed root (seadragon2 = IS_SEADRAGON2, or a mother-spawned plain
//    seadragon at STRAT_ADDR_SEADRAGON) GROWS a neck upward: each ~2 ticks a
//    segment finishes its stretch anim (`sprouty.strat` -> `.finished`) and
//    `.strat2` spawns the next segment above it (roffs offset along its own
//    rots), links parent.al_ptr -> child, hands the child `sproutstrat`
//    (= seadragon_istrat2) and turns itself into a body piece (`.strat3`).
//    `al_sbyte1` (init (rnd&3)+2) counts the neck height down per generation
//    (`seadragon_istrat2` beqdec); at 0 the top segment stops (`.stopstrat`).
//  * When the player is within z<1000 (`dzdistless`), the growing root spawns
//    the snake_0 fire-breathing HEAD (`.nobluff`) and links head.al_ptr = root.
//    Necks are hardHP(255)+nohitaffect (near-unkillable); killing the HEAD
//    (hp=4) runs `snake_istrat.explode`, which sets the neck's sflag5 -> the
//    neck `.withdraw`s (shrinks + sinks) and unlinks. That is the kill.
//
// SCOPE / FIDELITY BOUNDARIES (honest, cited inline):
//  * Only the SNAKE path of the shared `sprouty` machine is ported. The
//    tree1/tree2/tree3 (sflag6), tunnel-sprouter (sflag7) and flower/leaf
//    (`.bloom`/`leaf_istrat`) branches (DSTRATS.ASM:1970-2064, 2188-2192,
//    2288-2311, 2398-2416) are OUT of scope — different enemies that reuse the
//    same code. The snake-only branches are taken verbatim.
//  * Distinct snake FRAMES snake_0/snake_3/snake_4 collapse to SH_SNAKE_1
//    (201) — the renderer only models snake_1 (sf-map route3::common). Shape
//    ids are behaviour-inert here (no branch reads them), so this is purely a
//    visual approximation; firebreath reuses SH_CHICK_FIREBREATH.
//  * `make_splash` (sea_make_splash, a no-op like the rest of the sea lane),
//    `enemyupsea`/`enemydownsea` (sound only), and the `.bluff` idle are
//    modelled per the existing sea-boss port conventions.
//  * NO boss HP bar: verified there is no `s_add_bossHP`/`s_set_bossmaxHP`
//    anywhere in the seadragon/snake/sprouty spans — each segment/head is an
//    individually shootable object, so no `set_bossmaxhp`/`add_bosshp` calls
//    (unlike the generic boss template). Death routes through the simple
//    `explode_istrat` (strat_explode), not bossexplode.
// ============================================================

// --- constants ---
const SD_SEADRAGON_HP: u8 = 4; // D2STRATS.ASM:29 seadragonHP
const SD_SEADRAGON_AP: u8 = 6; // D2STRATS.ASM:30 seadragonAP
const SD_SEANECK_HP: u8 = 255; // DSTRATS.ASM:100 seaneckHP = hardHP (-1)
const SD_SEANECK_AP: u8 = 16; // DSTRATS.ASM:101 seaneckAP
const SD_SPROUT_MAXY: i16 = 80; // STRATEQU.INC:980 sprout_maxy
const SD_ANIM_SPEED: u8 = 4; // DSTRATS.ASM:1947 al_sword1 (anim speed)
const SD_TAIL_TIMER: u8 = 255; // DSTRATS.ASM:1938 al_sword1+1 (tail delay)
const SD_ANIM_MAX: u8 = 8; // s_add_anim ...,#8

// sflag mapping (same as flingboss/castanet/chicken): sflag1..4 ->
// sflags2 0x10/20/40/80; sflag5 -> sflags3 0x01; sflag8 -> sflags4 ASF4_SFLAG8.
const SD_SFLAG1: u8 = 0x10; // sflags2 — "always splash"
const SD_SFLAG2: u8 = 0x20; // sflags2 — "it's a dragon" (snake path marker)
const SD_SFLAG3: u8 = 0x40; // sflags2 — head created once
const SD_SFLAG4: u8 = 0x80; // sflags2 — fire-breathing (cosmetic here)
const SD_SFLAG5: u8 = 0x01; // sflags3 — sink/withdraw request

// Shapes (see scope note — collapse to snake_1 for rendering).
const SH_SNAKE_1: u16 = 201; // sf-map route3::common SH_SNAKE_1
const SD_SNAKE_HEAD: u16 = SH_SNAKE_1; // snake_0 (fire head) — proxy
const SD_SNAKE_BODY: u16 = SH_SNAKE_1; // snake_4 (sproutbody) — proxy
const SD_SNAKE_TAIL: u16 = SH_SNAKE_1; // snake_3 (sprouttail, lochness) — proxy
const SD_FIREBREATH: u16 = SH_CHICK_FIREBREATH; // reuse firebreath shape

const SD_SE_FIRE: u8 = 0x2e; // trigse $2e (D2STRATS.ASM:797)

// --- al_sword1 byte accessors (ROM treats al_sword1 low = anim speed,
// al_sword1+1 high = tail timer; STRUCTS.INC word-in-two-bytes). ---
#[inline]
fn sd_sword1_lo(al: &Alien) -> u8 {
    (al.sword1 as u16 & 0x00ff) as u8
}
#[inline]
fn sd_sword1_hi(al: &Alien) -> u8 {
    ((al.sword1 as u16 >> 8) & 0xff) as u8
}
#[inline]
fn sd_set_sword1_lo(al: &mut Alien, v: u8) {
    al.sword1 = ((al.sword1 as u16 & 0xff00) | v as u16) as i16;
}
#[inline]
fn sd_set_sword1_hi(al: &mut Alien, v: u8) {
    al.sword1 = ((al.sword1 as u16 & 0x00ff) | ((v as u16) << 8)) as i16;
}

/// `s_add_Roffs2pos B,y,y,y,#0,y1,#0,1,1,1,0,0,0` — position `obj` at its own
/// world pos + rotate(offset by its own rotz,rotx,roty), scale 0 (no shift).
fn sd_roffs(g: &mut Game, obj: u16, offx: i16, offy: i16, offz: i16) {
    let base = g.objs.aliens[obj as usize];
    b2_full_offset_pos(g, obj, &base, offx, offy, offz);
}

// ==== inits ====

/// `seadragon_istrat` (DSTRATS.ASM:1934) — plain sea dragon
/// (STRAT_ADDR_SEADRAGON, mother-spawned). Falls into `.missheight`.
fn sd_seadragon_init(g: &mut Game, idx: u16) {
    let r = (sfrtl_random(g) & 3) as u8; // s_set_alvar2rnd sbyte1,#3
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte1 = r.wrapping_add(2); // s_add_alvar sbyte1,#2 -> [2,5]
    sd_set_sword1_hi(al, SD_TAIL_TIMER); // sword1+1 = 255
    sd_missheight(g, idx);
}

/// `seadragon2_istrat` (DSTRATS.ASM:1931) — map-placed root (IS_SEADRAGON2).
/// Sets sbyte2=15 then falls into `seadragon_istrat` (`seady`).
fn sd_seadragon2_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 15; // s_set_alvar sbyte2,#15
    sd_seadragon_init(g, idx);
}

/// `lochnessmonster_istrat` (DSTRATS.ASM:1926). Sets sflag8 + sprouttail=snake_3
/// and jumps to `seady.missheight` (skips the sbyte1/tail-timer randomization,
/// so sbyte1 stays 0 -> `seadragon_istrat2` takes its sflag8 early-out and
/// never runs the height countdown).
fn sd_lochness_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8; // sflag8 "lock ness"
    // sprouttail=#snake_3 is a global; collapsed to the snake_1 proxy here.
    sd_missheight(g, idx);
}

/// `seady.missheight` (DSTRATS.ASM:1939-1948): snake flags + shapes + drop the
/// root half a segment, then fall into `seadragon_istrat2`.
fn sd_missheight(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= SD_SFLAG2 | SD_SFLAG4; // dragon + fire-breathing
        al.worldy = al.worldy.wrapping_sub(SD_SPROUT_MAXY / 2); // -40
        sd_set_sword1_lo(al, SD_ANIM_SPEED); // anim speed = 4
        al.sflags |= ASF_COLLDISABLE; // colldisable
    }
    sd_seadragon_istrat2(g, idx);
}

/// `seadragon_istrat2` (`sead`, DSTRATS.ASM:1950-1968) — the `sproutstrat`
/// handed to every new segment AND the root's continuation. Sets the growth
/// strat/coll/exp ptrs, then either bails to `sprouty.strat` (lochness / still
/// growing) or, when the height counter hits 0, becomes the topmost `.stop`.
fn sd_seadragon_istrat2(g: &mut Game, idx: u16) {
    let s_strat = sid(g, sprouty_strat);
    let s_coll = sid(g, strat_hit_flash); // hitflash_istrat
    let s_exp = sid(g, strat_explode); // explode_istrat (simple)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = SD_SEANECK_HP; // sproutiHP (255)
        al.ap = SD_SEANECK_AP;
        al.sflags |= ASF_NOHITAFFECT; // nohitaffect
        al.collflags |= COLLTYPE_ENEMY1;
    }
    sea_anim_set(&mut g.objs.aliens[idx as usize], 0); // s_init_anim #0
    if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 != 0 {
        // lochness -> straight to growth
        sprouty_strat(g, idx);
        return;
    }
    // s_beqdec_alvar sbyte1,.stop : branch at 0 BEFORE decrement.
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        let s_stop = sid(g, sd_stopstrat);
        g.objs.aliens[idx as usize].stratptr = Some(s_stop);
        sd_stopstrat(g, idx); // .stop falls straight into .stopstrat same tick
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    sprouty_strat(g, idx);
}

/// `.stopstrat` (DSTRATS.ASM:1965-1968): the topmost segment idles until its
/// head is killed (sflag2 && sflag5) -> withdraw.
fn sd_stopstrat(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    if al.sflags2 & SD_SFLAG2 != 0 && al.sflags3 & SD_SFLAG5 != 0 {
        sprouty_withdraw_init(g, idx);
    }
}

// ==== sprouty growth machine (snake path) ====

/// `sprouty.strat` (DSTRATS.ASM:2107-2168) — the per-tick growth driver for a
/// still-growing segment. Snake path only (see scope note).
fn sprouty_strat(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let sflag8 = me.sflags4 & ASF4_SFLAG8 != 0;
    // s_jmp_NOTalsflag sflag2,.notsnake2 — seadragon always has sflag2, so the
    // snake branch is taken. sflag8 || sbyte2!=0 -> jump straight to .lochness
    // (grow now, no distance gate); else gate on dzdistless(1000).
    let reached_grow = if sflag8 || me.sbyte2 != 0 {
        true // .lochness (jump over the distance check)
    } else if !sea_dz_less(g, idx, 1000) {
        // .chksnake: far away — splash only (cosmetic), stay submerged.
        if g.vars.gameframe & 7 == 0 {
            sea_make_splash(g, idx); // s_make_splash; y.worldz-=10 (no-op)
        }
        return;
    } else {
        true // close -> fall into .lochness label
    };
    if reached_grow {
        // .lochness (DSTRATS.ASM:2125-2145)
        if me.sflags2 & SD_SFLAG3 == 0 {
            // head not yet created — decide nobluff / bluff.
            let do_nobluff = if sflag8 || me.sbyte2 != 0 {
                true
            } else if (sfrtl_random(g) & 0xff) < 127 {
                // s_jmp_random .bluff (branch when random < 127)
                sprouty_bluff_init(g, idx);
                return;
            } else {
                sea_enemy_up_sea(g, idx); // enemyupsea then .nobluff
                true
            };
            if do_nobluff {
                sprouty_make_head(g, idx);
            }
        }
    }
    // .notsnake: grow the stretch animation.
    sprouty_animate_growth(g, idx);
}

/// `.nobluff` (DSTRATS.ASM:2132-2146): create the snake_0 fire head, link it
/// head.al_ptr -> this segment, mark this segment "always splash".
fn sprouty_make_head(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= SD_SFLAG3; // only do this once
    if let Some(head) = make_obj(g, SD_SNAKE_HEAD) {
        copy_pos(g, head, idx);
        chicken_copyrots(g, head, idx); // fling.copyrots_yx
        let s_head = sid(g, sd_snake_head_init);
        {
            let seg = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[head as usize];
            al.stratptr = Some(s_head); // snake_istrat
            al.collflags |= COLLTYPE_ENEMY1;
            al.ptr = boss_obj_index_or_null(idx); // head.al_ptr = segment
            al.sbyte2 = seg.sbyte2; // copy sbyte2 (fire counter)
            al.sbyte3 = sd_sword1_hi(&seg); // copy sword1+1 -> head sbyte3
            al.sflags &= !ASF_INVISIBLE;
            if seg.sflags4 & ASF4_SFLAG8 != 0 {
                al.sflags2 |= SD_SFLAG1; // nessie head: set sflag1
            }
        }
    }
    // .failed: this segment shows snake_1 + becomes collidable.
    let al = &mut g.objs.aliens[idx as usize];
    al.shape = SH_SNAKE_1;
    al.sflags &= !ASF_COLLDISABLE;
    g.objs.aliens[idx as usize].sflags2 |= SD_SFLAG1; // always splash (segment)
}

/// `.notsnake` growth anim (DSTRATS.ASM:2147-2159): `s_add_anim x,sword1_lo,#8,
/// .finished`. 4-arg/label form -> cap at max-1 and jump (AUDIT rule). On the
/// finish tick decide withdraw vs. spawn-next.
fn sprouty_animate_growth(g: &mut Game, idx: u16) {
    let amt = sd_sword1_lo(&g.objs.aliens[idx as usize]); // svar_byte1
    let cur = g.objs.aliens[idx as usize].animframe & 0x7f;
    let next = cur.wrapping_add(amt);
    if next >= SD_ANIM_MAX {
        // .finished
        sea_anim_set(&mut g.objs.aliens[idx as usize], SD_ANIM_MAX - 1); // cap 7 (|bit7)
        let al = g.objs.aliens[idx as usize];
        if al.sflags2 & SD_SFLAG2 != 0 && al.sflags3 & SD_SFLAG5 != 0 {
            sprouty_withdraw_init(g, idx);
        } else {
            let s = sid(g, sprouty_strat2);
            g.objs.aliens[idx as usize].stratptr = Some(s); // -> .strat2 next tick
        }
    } else {
        sea_anim_set(&mut g.objs.aliens[idx as usize], next);
    }
}

/// `.bluff` (DSTRATS.ASM:2162-2168): the snake fakes out — idle + splash,
/// never emerges (a legit ROM outcome for a fraction of snakes).
fn sprouty_bluff_init(g: &mut Game, idx: u16) {
    let s = sid(g, sprouty_bluff_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    sprouty_bluff_strat(g, idx);
}
fn sprouty_bluff_strat(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 7 == 0 {
        sea_make_splash(g, idx);
    }
}

/// `.strat2` (DSTRATS.ASM:2170-2210): a finished segment spawns the next
/// segment above it, links parent.al_ptr -> child, hands the child the
/// `sproutstrat` (seadragon_istrat2), then becomes a `.strat3` body piece.
fn sprouty_strat2(g: &mut Game, idx: u16) {
    sea_anim_set(&mut g.objs.aliens[idx as usize], SD_ANIM_MAX); // s_init_anim #8
    let Some(child) = make_obj(g, SD_SNAKE_BODY) else {
        return; // .end (alloc failed)
    };
    copy_pos(g, child, idx);
    chicken_copyrots(g, child, idx);
    {
        let x = g.objs.aliens[idx as usize];
        let y = &mut g.objs.aliens[child as usize];
        y.sbyte1 = x.sbyte1;
        y.sbyte2 = x.sbyte2;
        y.sbyte3 = x.sbyte3;
        y.sword1 = x.sword1;
        y.sflags = x.sflags; // s_copy_sflags (all 4 bytes)
        y.sflags2 = x.sflags2;
        y.sflags3 = x.sflags3;
        y.sflags4 = x.sflags4;
    }
    // .roffs (offset -37 up along child's own rots) — first application.
    sd_roffs(g, child, 0, -(SD_SPROUT_MAXY - 5) / 2, 0);
    // .snakemiss (DSTRATS.ASM:2193-2198): tilt the child back a little.
    {
        let sflag8 = g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 != 0;
        let y = &mut g.objs.aliens[child as usize];
        if sflag8 {
            y.rotx = y.rotx.wrapping_sub(DEG22); // -deg22
            y.sflags2 &= !SD_SFLAG1; // don't always splash
        } else {
            y.rotx = y.rotx.wrapping_sub(DEG22 / 2); // -deg11
        }
    }
    // .normmiss: second .roffs, link, bounds check.
    sd_roffs(g, child, 0, -(SD_SPROUT_MAXY - 5) / 2, 0);
    let s_child = sid(g, sd_seadragon_istrat2); // sproutstrat
    g.objs.aliens[child as usize].stratptr = Some(s_child);
    if sprouty_out_of_bounds(g, child) {
        // .finishitup: parent marks "topmost" and drops the child.
        g.objs.aliens[idx as usize].ptr = 0xffff; // al_ptr = -1
        g.objs.free(child);
    } else {
        g.objs.aliens[idx as usize].ptr = boss_obj_index_or_null(child); // link
    }
    // .strat3_i: this segment is now a body piece.
    let s3 = sid(g, sprouty_strat3);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = SD_SEANECK_HP; // sproutiHP
    al.shape = SD_SNAKE_BODY;
    al.stratptr = Some(s3);
}

/// `.boundscheck .nottunnel` (DSTRATS.ASM:2239-2246): the snake path just
/// tests worldy — `s_bpl .setit` marks out-of-bounds when worldy >= 0 (the
/// neck grows UP into negative Y and stays in-bounds; a falling piece is
/// "out" once it sinks back to/below the water plane y=0). (The tunnel branch,
/// sflag7, is scoped out.)
fn sprouty_out_of_bounds(g: &Game, obj: u16) -> bool {
    g.objs.aliens[obj as usize].worldy >= 0
}

/// `.strat3` (DSTRATS.ASM:2266-2295) — a body segment. Splash at the top,
/// fall when the child is destroyed, count the tail timer down.
fn sprouty_strat3(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    if me.sflags2 & SD_SFLAG2 == 0 {
        return; // .notsnakey (non-snake) — not our case
    }
    if me.sflags3 & SD_SFLAG5 != 0 {
        sprouty_withdraw_init(g, idx);
        return;
    }
    // splash when topmost (al_ptr==-1) or "always splash" (sflag1).
    let ptr = me.ptr;
    let want_splash = ptr == 0xffff || me.sflags2 & SD_SFLAG1 != 0;
    if want_splash && g.vars.gameframe & 7 == 0 {
        sea_make_splash(g, idx); // + child worldy=0, worldz-=10 (no-op)
    }
    // .notsnakey: child destroyed (al_ptr==0) -> fall.
    if ptr == 0 {
        sprouty_fall_init(g, idx);
        return;
    }
    // s_beqdec_alvar sword1+1,.animate : tail timer.
    let hi = sd_sword1_hi(&g.objs.aliens[idx as usize]);
    if hi == 0 {
        sprouty_tail_init(g, idx);
        return;
    }
    sd_set_sword1_hi(&mut g.objs.aliens[idx as usize], hi - 1);
    // (sflag6 tree / sflag4 leaf branch is scoped out.)
}

/// `.withdraw_i`/`.withdraw` (DSTRATS.ASM:2252-2264): shrink the stretch anim
/// to 0, then latch sflag5 on whoever points at us and remove ourselves —
/// propagating the sink down the neck.
fn sprouty_withdraw_init(g: &mut Game, idx: u16) {
    let s = sid(g, sprouty_withdraw_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.shape = SH_SNAKE_1; // sprouthead
    sprouty_withdraw_strat(g, idx);
}
fn sprouty_withdraw_strat(g: &mut Game, idx: u16) {
    let frame = g.objs.aliens[idx as usize].animframe & 0x7f;
    if frame == 0 {
        // .finishedshrink: find whoever links to us, set its sflag5, remove us.
        if let Some(parent) = sprouty_find_alptr(g, idx) {
            g.objs.aliens[parent as usize].sflags3 |= SD_SFLAG5;
        }
        g.objs.aldead = 1; // s_remove_obj x (current)
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    let nf = frame.saturating_sub(4);
    al.animframe = 0x80 | (nf & 0x7f); // s_sub_alvar animframe,#4
}

/// `find_alptr_l`-style scan: the alien whose al_ptr points at `target`.
fn sprouty_find_alptr(g: &Game, target: u16) -> Option<u16> {
    let want = boss_obj_index_or_null(target);
    for i in g.objs.active_indices() {
        if g.objs.aliens[i as usize].active && g.objs.aliens[i as usize].ptr == want {
            return Some(i);
        }
    }
    None
}

/// `.animate`/`.finishup` (DSTRATS.ASM:2313-2345): the tail piece appears and
/// retracts. Rarely reached (255-tick timer) but ported for completeness.
fn sprouty_tail_init(g: &mut Game, idx: u16) {
    let s = sid(g, sprouty_tail_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        // animframe = 128 - sword1_lo (ROM: eor -1; inc; +128).
        let lo = sd_sword1_lo(al);
        al.animframe = 0x80 | (128u8.wrapping_sub(lo) & 0x7f);
    }
    // sflag8 splash branch (D2STRATS-style) omitted (cosmetic).
    if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
        g.objs.aliens[idx as usize].hp = 1; // s_set_alvar al_hp,#1
    }
    sprouty_tail_strat(g, idx);
}
fn sprouty_tail_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].ptr == 0 {
        sprouty_fall_init(g, idx);
        return;
    }
    let lo = sd_sword1_lo(&g.objs.aliens[idx as usize]);
    let frame = g.objs.aliens[idx as usize].animframe & 0x7f;
    let nf = frame.wrapping_add(lo);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SD_SNAKE_TAIL; // sprouttail
        al.animframe = 0x80 | (nf & 0x7f);
    }
    if nf & 0x7f >= SD_ANIM_MAX {
        g.objs.aldead = 1; // .rem (current)
    }
}

/// `.fall_istrat`/`.fall` (DSTRATS.ASM:2359-2381): a detached segment tumbles
/// with gravity + spin, then explodes when it leaves the field.
fn sprouty_fall_init(g: &mut Game, idx: u16) {
    let s = sid(g, sprouty_fall_strat);
    let roty = (sfrtl_random(g) & 0xff) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.vel = 30;
        al.roty = roty; // s_set_alvar2rnd al_roty
    }
    // dgen3dvecs + vy = -10.
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].vy = -10;
    // remove_alptrs: unlink anything pointing at us (avoid dangling neck).
    if let Some(p) = sprouty_find_alptr(g, idx) {
        g.objs.aliens[p as usize].ptr = 0;
    }
    sprouty_fall_strat(g, idx);
}
fn sprouty_fall_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(5);
        al.rotx = al.rotx.wrapping_add(2);
        al.vy = al.vy.wrapping_add(3); // s_falldown_Yvec (gravity 3)
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    if sprouty_out_of_bounds(g, idx) {
        // .le_fin: s_set_expstrat explode + s_kill_obj (strat_explode marks
        // the current object dead via aldead).
        let s = sid(g, strat_explode);
        g.objs.aliens[idx as usize].expstratptr = Some(s);
        strat_explode(g, idx);
    }
}

// ==== snake head (snake_istrat, D2STRATS.ASM:732-861) ====

/// `snake_istrat` init (D2STRATS.ASM:732-745).
fn sd_snake_head_init(g: &mut Game, idx: u16) {
    let s = sid(g, sd_snake_head_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, sd_snake_head_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = SD_SEADRAGON_HP; // 4
        al.ap = SD_SEADRAGON_AP; // 6
        al.collflags |= COLLTYPE_ENEMY1;
    }
    sea_make_splash(g, idx);
    // Face sideways toward the player: roty += (x < player.x ? -deg90 : +deg90).
    if let Some(p) = player(g) {
        let me = &mut g.objs.aliens[idx as usize];
        if me.worldx < p.worldx {
            me.roty = me.roty.wrapping_sub(DEG90);
        } else {
            me.roty = me.roty.wrapping_add(DEG90);
        }
    }
}

/// `snake_istrat.getneck` (D2STRATS.ASM:849-861): follow al_ptr to the neck,
/// climbing to the next piece while the current one still has a child.
/// Returns None (and removes the head) when the neck is gone.
fn sd_head_getneck(g: &mut Game, idx: u16) -> Option<u16> {
    let neck_raw = g.objs.aliens[idx as usize].ptr;
    let Some(neck) = boss_child_from_index_raw(neck_raw) else {
        // al_ptr == 0 -> .removeit (remove head).
        g.objs.aldead = 1;
        return None;
    };
    // If this neck piece has a live child, climb up to it (head.al_ptr = child).
    let cptr = g.objs.aliens[neck as usize].ptr;
    if let Some(child) = boss_child_from_index_raw(cptr) {
        g.objs.aliens[idx as usize].ptr = boss_obj_index_or_null(child);
        return Some(child);
    }
    Some(neck)
}

/// `snake_istrat.strat` (D2STRATS.ASM:746-806): aim, ride the top of the neck,
/// breathe fire (seadragon2 only).
fn sd_snake_head_strat(g: &mut Game, idx: u16) {
    let neck_raw = g.objs.aliens[idx as usize].ptr;
    // if neck.al_ptr == -1 -> underwater (the neck grew off the top).
    if let Some(neck) = boss_child_from_index_raw(neck_raw) {
        if g.objs.aliens[neck as usize].ptr == 0xffff {
            sd_head_underwater_init(g, idx);
            return;
        }
    }
    // Aim: sflag1 uses the neck's rots offset; else obj2obj toward player.
    if g.objs.aliens[idx as usize].sflags2 & SD_SFLAG1 != 0 {
        if let Some(neck) = sd_head_getneck(g, idx) {
            let n = g.objs.aliens[neck as usize];
            let me = &mut g.objs.aliens[idx as usize];
            me.rotx = n.rotx;
            me.roty = n.roty.wrapping_add(DEG180);
            me.rotz = n.rotz;
            me.rotx = (0i8.wrapping_sub(me.rotx as i8)) as u8; // negate
            me.rotx = me.rotx.wrapping_sub(DEG90).wrapping_sub(DEG22); // -deg90-deg22
        } else {
            return; // head removed
        }
    } else if let Some(p) = player_idx(g) {
        let me = g.objs.aliens[idx as usize];
        let pl = g.objs.aliens[p as usize];
        let yaw = strat_angle_xz(&me, &pl);
        let pitch = strat_pitch_toward(&me, &pl);
        let a = &mut g.objs.aliens[idx as usize];
        a.roty = yaw;
        a.rotx = pitch;
    }
    // Position at the top of the neck (offset from neck's stretch anim).
    let Some(neck) = sd_head_getneck(g, idx) else {
        return; // head removed
    };
    let animval = (g.objs.aliens[neck as usize].animframe & 0x7f) as i16;
    let y1 = 40 - animval * 10; // ROM: 40 - anim*10 (signed byte)
    let n = g.objs.aliens[neck as usize];
    b2_full_offset_pos(g, idx, &n, 0, y1, -10);
    // Fire: sbyte2==0 never; sbyte2==1 fire every gf&15==0; else countdown.
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        return;
    }
    if sb2 == 1 {
        if g.vars.gameframe & 15 == 0 {
            sd_head_fire(g, idx);
        }
    } else {
        g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
    }
}

/// `.notfire`/firebreath spawn (D2STRATS.ASM:792-798): reuse the firebreath
/// mover (a straight tracked fireball; see chicken scope note).
fn sd_head_fire(g: &mut Game, idx: u16) {
    let Some(fb) = make_obj(g, SD_FIREBREATH) else {
        return;
    };
    copy_pos(g, fb, idx);
    chicken_copyrots(g, fb, idx);
    let s = sid(g, chicken_firebreath_strat);
    {
        let al = &mut g.objs.aliens[fb as usize];
        al.shape = SD_FIREBREATH;
        al.stratptr = Some(s);
        al.vel = 120; // firebreathe2 vel
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags &= !ASF_INVISIBLE;
    }
    play_se(g, SD_SE_FIRE);
}

/// `snake_istrat.explode` (D2STRATS.ASM:810-817): tell the neck to sink
/// (sflag5), unlink, then run the simple explode.
fn sd_snake_head_explode(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & SD_SFLAG1 == 0 {
        if let Some(neck) = sd_head_getneck(g, idx) {
            g.objs.aliens[neck as usize].sflags3 |= SD_SFLAG5; // sink it back down
        }
    }
    g.objs.aliens[idx as usize].ptr = 0; // al_ptr = 0
    let s = sid(g, strat_explode);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    strat_explode(g, idx);
}

/// `.underwater`/`.swimabit`/`.startnextneck` (D2STRATS.ASM:820-847): the head
/// dives, swims a beat, then spawns a fresh lochness root at the surface and
/// removes itself. (Modelled with sound + a simple dive; see scope note.)
fn sd_head_underwater_init(g: &mut Game, idx: u16) {
    let s = sid(g, sd_head_swim_strat);
    sea_enemy_down_sea(g, idx);
    let neck_raw = g.objs.aliens[idx as usize].ptr;
    let neck_rots = boss_child_from_index_raw(neck_raw).map(|n| {
        let a = g.objs.aliens[n as usize];
        (a.rotx, a.roty, a.rotz)
    });
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.shape = 0; // nullshape
        if let Some((rx, ry, rz)) = neck_rots {
            al.rotx = rx;
            al.roty = ry;
            al.rotz = rz;
        }
        al.vel = (-20i8) as u8;
        al.sbyte1 = 10;
        al.rotx = 0;
    }
    sea_gen_vecs_angle(g, idx, g.objs.aliens[idx as usize].roty);
    sd_head_swim_strat(g, idx);
}
fn sd_head_swim_strat(g: &mut Game, idx: u16) {
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        // .startnextneck: spawn a lochness root at the surface, remove self.
        if let Some(root) = make_obj(g, 0) {
            copy_pos(g, root, idx);
            let s = sid(g, sd_lochness_init);
            let sbyte3 = g.objs.aliens[idx as usize].sbyte3;
            let al = &mut g.objs.aliens[root as usize];
            al.stratptr = Some(s);
            al.worldy = 0;
            sd_set_sword1_hi(al, sbyte3);
        }
        sea_enemy_up_sea(g, idx);
        g.objs.aldead = 1; // .dekinai -> .remove (current)
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
}
// SEADRAGON_END

// ============================================================
// WEBMONSTER_BEGIN — "webmonster" (Route 3 L2 spider boss) + its child strats.
//
// ASM oracle: DSTRATS.ASM —
//   webmonster_istrat     :6504 (mother; intro descent / .mainstrat / .move /
//                                .bossdead / .generate / .position)
//   drill_istrat          :6587 (the "web_fan" child; .strat spin+sweep+turret
//                                sequencing / .drillattack / .drillretrieve /
//                                .launchweb / .hit)
//   launchatplayer_istrat :6787 (fan detach on death)
//   web_istrat            :6800 (launched grab projectile)
//   propturret_istrat     :6886 (the 6 damageable turrets)
// Child ring layout: DSTRATS.ASM:6572-6583 (.generate). Child numbers
// web_turret1..6 = 1..6, web_fan = 7 (DSTRATS.ASM:6496-6502).
// ISTRATS.ASM:548 `def_istrat webmonster,boss_0_1` -> macro-counted index 123
// (= sf-map route3::common IS_WEBMONSTER; the map spawns SH_BOSS_0_1(85)
// carrying this ISTRAT index, resolved through world.istrats[123] exactly like
// flingboss/castanet/chicken — level3_2.rs:234).
//
// HP MODEL (no dedicated per-body HP; the bar is turret-driven):
//  * The mother is invulnerable — hp = hardHP, collstrat = hitflash
//    (DSTRATS.ASM:6513-6514). Direct hits only flash it.
//  * The boss HP bar = m_bossHP, re-accumulated each tick from the 6 LIVE
//    propturrets (each `s_add_bossHP x,al_hp`, DSTRATS.ASM:6933). bossmaxHP =
//    0 at mother init (:6508) + 6 * propturretHP(20) from each turret's
//    `s_add_bossmaxHP` (:6890) = 120. As turrets die the bar drains.
//  * The boss "dies" when all 6 turrets are dead (`s_jmp_childrendead
//    web_turret1..6 -> .bossdead`, DSTRATS.ASM:6531).
//
// SCOPE NOTES (fidelity caveats, cited inline):
//  * `.hit`'s RebElasercol branch (DSTRATS.ASM:6696) — a player-laser reflect —
//    is scoped to plain hitflash (collision-lane detail; the reflect body is
//    commented out in the ROM anyway, :6687-6694).
//  * web_istrat's player GRAB (drag player.worldx/y to the web, DSTRATS.ASM:
//    6837-6842) and the SHAKE-FREE input reader (:6845-6863) are player-lane:
//    the observable web motion / sflag transitions / sbyte3 timeout are ported,
//    the shake counter uses the available `player_rollZvel` cell; the
//    dir-key-change contribution (cont0^lastcont0) is scoped out (no live
//    `cont0` mirror). The player-position drag is applied faithfully when a
//    live player object exists.
//  * The death `bgm_music/bgmcnt` poke (DSTRATS.ASM:6560-6562) is audio-only;
//    only the `trigse $1e` sound is emitted (no music-cell plumbing here).
// ============================================================

/// `webmonster` ISTRATS.ASM:548 def_istrat, macro-counted 123 (= sf-map
/// route3::common IS_WEBMONSTER; level3_2.rs:234 spawns SH_BOSS_0_1 carrying it).
pub const IS_WEBMONSTER: usize = 123;

// Child numbers (DSTRATS.ASM:6496-6502).
const WM_TURRET1: u8 = 1;
const WM_TURRET6: u8 = 6;
const WM_FAN: u8 = 7;

// propturretHP/AP (DSTRATS.ASM:81-82).
const WM_PROPTURRET_HP: u8 = 20;
const WM_PROPTURRET_AP: u8 = 4;

// Shape proxies (next free after chicken's 292). The mother keeps the map's
// SH_BOSS_0_1(85); these back the boss_0_2/0_0/0_0a/0_3 shapes set in code.
const SH_WM_BOSS_0_2: u16 = 293; // turret
const SH_WM_BOSS_0_0: u16 = 294; // fan (rest / boss_0_0)
const SH_WM_BOSS_0_0A: u16 = 295; // fan (spinning / boss_0_0a)
const SH_WM_BOSS_0_3: u16 = 296; // launched web (boss_0_3)

// Strategy flags mapped onto the Rust bytes exactly like flingboss/boss2:
// sflag1..3 -> sflags2 0x10/20/40, sflag8 -> sflags4 (ASF4_SFLAG8).
const WM_SFLAG1: u8 = 0x10; // turret: spinning(=invuln) ; fan/web: banks/grab
const WM_SFLAG2: u8 = 0x20; // turret: armed-to-fire ; fan: bank toggle ; web: attach
const WM_SFLAG3: u8 = 0x40; // turret: fade-out ; web: released
const WM_SFLAG8: u8 = ASF4_SFLAG8; // web: shake edge-latch

// deg constants absent from the shared surface (VARS.INC:20-23): deg120=85,
// deg60=42, deg240=170, deg300=212.
const WM_DEG60: u8 = 42;
const WM_DEG120: u8 = 85;

// HMISSILE1 = fire_Hmissile1 (GSTRATS.ASM:2627): speed 60, life 100,
// hp=hmissile1HP(2), ap=hmissile1AP(8), strat hmissile1_Istrat (homing).
const WM_HMISSILE_SPEED: u8 = 60;
const WM_HMISSILE_LIFE: u8 = 100;
const WM_HMISSILE_HP: u8 = 2;
const WM_HMISSILE_AP: u8 = 8;

// trigse ids used by the boss.
const WM_SE_SPIN: u8 = 0x4f; // fan spin whir
const WM_SE_DRILL: u8 = 0x50; // metal drill
const WM_SE_DRILLGO: u8 = 0x8f; // drill launch
const WM_SE_GRAB: u8 = 0x51; // web grab
const WM_SE_DEATH: u8 = 0x1e; // boss death

// ---- shared child-flag / dead helpers (STRATLIB.INC:736/770/803) ----

/// `s_set/clr_childsflag mother,sflag1..3,begin,end` — the flags used all live
/// on sflags2. `set=true` sets, `false` clears, over child numbers [begin,end].
fn wm_child_sflag2_range(g: &mut Game, mother: u16, bit: u8, begin: u8, end: u8, set: bool) {
    for n in begin..=end {
        if let Some(c) = boss_find_child_obj(g, mother, n) {
            if set {
                g.objs.aliens[c as usize].sflags2 |= bit;
            } else {
                g.objs.aliens[c as usize].sflags2 &= !bit;
            }
        }
    }
}

/// `s_jmp_childrendead mother,begin,end` (STRATLIB.INC:803): true only when
/// EVERY child number in [begin,end] is dead (absent).
fn wm_children_dead(g: &mut Game, mother: u16, begin: u8, end: u8) -> bool {
    for n in begin..=end {
        if boss_find_child_obj(g, mother, n).is_some() {
            return false;
        }
    }
    true
}

/// find_y_l (DSTRATS.ASM:3589): first active object whose al_shape == `shape`.
fn wm_find_by_shape(g: &Game, shape: u16) -> Option<u16> {
    for i in 0..NUMBER_AL {
        let a = &g.objs.aliens[i];
        if a.active && a.shape == shape {
            return Some(i as u16);
        }
    }
    None
}

/// dzdistless (DSTRATS.ASM:156): |obj.worldz - player.worldz| < dist.
fn wm_dz_less(g: &Game, obj: u16, dist: i16) -> bool {
    let Some(p) = player_idx(g) else {
        return false;
    };
    let dz = g.objs.aliens[obj as usize]
        .worldz
        .wrapping_sub(g.objs.aliens[p as usize].worldz) as i32;
    dz.abs() < dist as i32
}

/// Manhattan-blend XY distance mirroring `strat_dist_xz` but on worldx/worldy
/// (the game's s_jmp_outxydistrng uses the same rangexy blend on X/Y).
fn wm_dist_xy(a: &Alien, b: &Alien) -> i16 {
    let mut x1 = b.worldx.wrapping_sub(a.worldx);
    if x1 < 0 {
        x1 = x1.wrapping_neg();
    }
    let mut y1 = b.worldy.wrapping_sub(a.worldy);
    if y1 < 0 {
        y1 = y1.wrapping_neg();
    }
    x1 >>= 1;
    y1 >>= 1;
    let rangexy = (y1.wrapping_add(x1)).wrapping_shl(1);
    let m = if y1 < x1 { x1 } else { y1 };
    let t = m.wrapping_add(rangexy);
    let acc = (t >> 1).wrapping_add(t.wrapping_shl(2));
    ((acc >> 1) >> 1) >> 1
}

/// dincanimjmp_x #max (DSTRATS.ASM:137, 4-arg s_add_anim CAP): increment
/// animframe, capping at max-1 (STRATLIB.INC:180 label variant).
fn wm_dinc_anim_cap(g: &mut Game, idx: u16, max: u8) {
    let al = &mut g.objs.aliens[idx as usize];
    if al.animframe < max - 1 {
        al.animframe += 1;
    }
}

/// ddecanim_x #n (DSTRATS.ASM:146, 3-arg s_add_anim -1 WRAP): decrement mod n.
fn wm_ddec_anim(g: &mut Game, idx: u16, n: u8) {
    let al = &mut g.objs.aliens[idx as usize];
    al.animframe = (al.animframe + n - 1) % n;
}

// ---- child ring positioning (child_rotpos_l / rotpos_allchildren_l,
//      DSTRATS.ASM:6939-6991) ----

/// child_rotpos_l: child rots = mother rots + stored child rot offsets; child
/// worldpos = mother worldpos + rotate(childXYZ << childscale(3)) by the
/// mother's FULL rotation (s_add_Roffs2pos flags 1,1,1, scale childscale).
fn wm_child_rotpos(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    let c = g.objs.aliens[child as usize];
    let rotx = m.rotx.wrapping_add(c.childrotx);
    let roty = m.roty.wrapping_add(c.childroty);
    let rotz = m.rotz.wrapping_add(c.childrotz);
    let ox = ((c.childx as i8) as i16) << 3;
    let oy = ((c.childy as i8) as i16) << 3;
    let oz = ((c.childz as i8) as i16) << 3;
    {
        let a = &mut g.objs.aliens[child as usize];
        a.rotx = rotx;
        a.roty = roty;
        a.rotz = rotz;
    }
    b2_full_offset_pos(g, child, &m, ox, oy, oz);
}

/// `.position` (DSTRATS.ASM:6568-6570) = s_rotpos_allchildren: reposition every
/// live child (turrets 1-6 + fan 7).
fn wm_position(g: &mut Game, mother: u16) {
    for n in WM_TURRET1..=WM_FAN {
        if let Some(c) = boss_find_child_obj(g, mother, n) {
            wm_child_rotpos(g, mother, c);
        }
    }
}

// ============================================================
// propturret (the 6 damageable turrets) — DSTRATS.ASM:6886-6934.
// ============================================================

/// propturret_istrat init (DSTRATS.ASM:6886-6891): HP/AP, add to the boss bar's
/// max, depthoffset=1. Falls into `.strat` the same tick.
fn wm_propturret_init(g: &mut Game, idx: u16) {
    let s = sid(g, wm_propturret_strat);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = WM_PROPTURRET_HP;
        al.ap = WM_PROPTURRET_AP;
        al.depthoffset = 1;
    }
    set_bossmaxhp(g, bossmaxhp(g).wrapping_add(WM_PROPTURRET_HP as u16)); // s_add_bossmaxHP
    wm_propturret_strat(g, idx);
}

/// fire HMISSILE1 from the turret (DSTRATS.ASM:6912-6917): s_weapon_rot
/// #0,#-deg180 (pitch 0, yaw +deg180), muzzle 0,0,0, then homes al_ptr=player.
fn wm_propturret_fire(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let pitch = me.rotx;
    let yaw = me.roty.wrapping_add(DEG180);
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        WM_HMISSILE_SPEED,
        WM_HMISSILE_LIFE,
        WM_HMISSILE_AP,
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    // hmissile1_Istrat homing is the flingboss port (identical weapon strat).
    let s = sid(g, flingboss_hmissile1_strat);
    let ppt = player_idx(g).map(boss_obj_index_or_null).unwrap_or(0);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.sflags &= !ASF_INVISIBLE;
    al.sflags |= ASF_SHADOW;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.hp = WM_HMISSILE_HP;
    al.ptr = ppt; // s_set_alvar al_ptr,playpt
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratptr = Some(s);
    strat_gen_vecs_3d(al);
}

/// propturret_istrat `.strat` (DSTRATS.ASM:6892-6934).
fn wm_propturret_strat(g: &mut Game, idx: u16) {
    // fade-out window (sflag3): every 8 frames step depthoffset down to 1.
    if g.objs.aliens[idx as usize].sflags2 & WM_SFLAG3 != 0 && g.vars.gameframe & 7 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        if al.depthoffset == 1 {
            al.sflags2 &= !WM_SFLAG3; // .clrflag
        } else {
            al.depthoffset -= 1;
        }
    }
    // .fannotfading
    if g.objs.aliens[idx as usize].sflags2 & WM_SFLAG1 != 0 {
        // .fanspinning: invulnerable, arm the shot, fade in (depthoffset -> 4).
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags |= ASF_NOHITAFFECT;
            al.sflags2 |= WM_SFLAG2;
        }
        if g.objs.aliens[idx as usize].sflags2 & WM_SFLAG3 == 0 && g.vars.gameframe & 15 == 0 {
            let al = &mut g.objs.aliens[idx as usize];
            if al.depthoffset != 4 {
                al.depthoffset += 1;
            }
        }
    } else {
        // not spinning: vulnerable, and fire once when armed.
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        if g.objs.aliens[idx as usize].sflags2 & WM_SFLAG2 != 0 {
            wm_propturret_fire(g, idx);
            g.objs.aliens[idx as usize].sflags2 &= !WM_SFLAG2;
        }
    }
    // .fannotspinning
    add_bosshp(g, idx); // s_add_bossHP x,al_hp
}

// ============================================================
// drill / web_fan (child #7) — DSTRATS.ASM:6587-6784.
// ============================================================

/// drill_istrat init (DSTRATS.ASM:6587-6592). Falls into `.strat`.
fn wm_drill_init(g: &mut Game, idx: u16) {
    let s = sid(g, wm_drill_strat);
    let s_col = sid(g, wm_drill_hit);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.hp = HARD_HP;
        al.ap = 2;
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.sflags |= ASF_NOHITAFFECT;
    }
    wm_drill_strat(g, idx);
}

/// drill_istrat `.strat` (DSTRATS.ASM:6593-6682): spin the fan (childrotz +=
/// sbyte2), then branch on sbyte3 (fade-out settle) vs `.carryon` (sweep).
fn wm_drill_strat(g: &mut Game, idx: u16) {
    let mother = boss_get_mother_obj(g, idx);
    // spin fan.
    {
        let al = &mut g.objs.aliens[idx as usize];
        let (crz, carry) = al.childrotz.overflowing_add(al.sbyte2);
        al.childrotz = crz;
        if carry {
            play_se(g, WM_SE_SPIN);
        }
    }
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        wm_drill_carryon(g, idx, mother);
    } else {
        wm_drill_settle(g, idx, mother);
    }
}

/// `.carryon` (DSTRATS.ASM:6646-6681): advance the sweep angle sbyte2, launch
/// the drill at 63, restart the turret-fire fade window at the 0 wrap.
fn wm_drill_carryon(g: &mut Game, idx: u16, mother: Option<u16>) {
    let sbyte2 = g.objs.aliens[idx as usize].sbyte2;
    // a = sbyte2 + 1; if a >= 42 -> a = sbyte2 + 2; if a >= 85 -> a = 0.
    let mut a = sbyte2.wrapping_add(1);
    if a >= 42 {
        a = sbyte2.wrapping_add(2);
    }
    if a >= WM_DEG120 {
        a = 0;
    }
    g.objs.aliens[idx as usize].sbyte2 = a;

    if a == 63 {
        // deg360/6 + deg360/12 -> drill launch.
        if g.objs.aliens[idx as usize].rotx == 0 {
            wm_drill_launchweb(g, idx);
            play_se(g, WM_SE_DRILLGO);
        }
        // s_set_strat x,.drillattack ; s_end_strat (runs .drillattack next tick).
        let s = sid(g, wm_drill_attack);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        return;
    }
    if a == 0 {
        // .clr: toggle the bank marker, open the 50-tick fade window, rest shape.
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 ^= WM_SFLAG2;
        al.sbyte3 = 50;
        al.shape = SH_WM_BOSS_0_0;
    } else {
        // spinning: spinning fan shape + all turrets spin (invuln).
        g.objs.aliens[idx as usize].shape = SH_WM_BOSS_0_0A;
        if let Some(m) = mother {
            wm_child_sflag2_range(g, m, WM_SFLAG1, WM_TURRET1, WM_TURRET6, true);
        }
    }
}

/// `.strat` sbyte3 branch (DSTRATS.ASM:6605-6645): settle childrotz to a deg120
/// sector boundary; each alignment decrements sbyte3 and releases (un-spins) a
/// turret bank so it becomes vulnerable and fires.
fn wm_drill_settle(g: &mut Game, idx: u16, mother: Option<u16>) {
    let crz = g.objs.aliens[idx as usize].childrotz;
    let sflag2 = g.objs.aliens[idx as usize].sflags2 & WM_SFLAG2 != 0;
    let mut a = if sflag2 {
        crz.wrapping_add(WM_DEG60)
    } else {
        crz
    };
    // .rechk: reduce a by deg120 while >= deg120 (keeps residue), stop on a==0.
    let do_nochange;
    loop {
        if a == 0 {
            do_nochange = true;
            break;
        }
        let (res, borrow) = a.overflowing_sub(WM_DEG120);
        a = res;
        if !borrow {
            continue; // carry set -> loop
        }
        // borrowed: a is the wrapped residue.
        if a < 252 {
            // .noz: step childrotz down by 4, aligned; skip the sbyte3 dec.
            let nc = crz.wrapping_sub(4) & 0xFC;
            g.objs.aliens[idx as usize].childrotz = nc;
            return;
        }
        // a in [252,255]: snap exactly to the boundary, then .nochange.
        let nc = crz.wrapping_sub(a);
        g.objs.aliens[idx as usize].childrotz = nc;
        do_nochange = true;
        break;
    }
    if do_nochange {
        g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_sub(1);
        if let Some(m) = mother {
            if sflag2 {
                wm_child_sflag2_range(g, m, WM_SFLAG1, 4, 6, false);
            } else {
                wm_child_sflag2_range(g, m, WM_SFLAG1, 1, 3, false);
            }
        }
    }
}

/// `.launchweb` (DSTRATS.ASM:6775-6784): spawn a web_istrat grab projectile at
/// the drill's pos/rots (fling.makeobj + copyrots/copypos).
fn wm_drill_launchweb(g: &mut Game, idx: u16) {
    let Some(web) = make_obj(g, SH_WM_BOSS_0_3) else {
        return;
    };
    let d = g.objs.aliens[idx as usize];
    let s = sid(g, wm_web_init);
    let al = &mut g.objs.aliens[web as usize];
    al.stratptr = Some(s);
    al.rotx = d.rotx;
    al.roty = d.roty;
    al.rotz = d.rotz;
    al.worldx = d.worldx;
    al.worldy = d.worldy;
    al.worldz = d.worldz;
}

/// `.moveagain` (DSTRATS.ASM:6731-6745): spin the fan + drill/whir sound.
fn wm_drill_moveagain(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let (crz, carry) = al.childrotz.overflowing_add(al.sbyte2);
    al.childrotz = crz;
    let anim = al.animframe;
    if !carry {
        // ROM `bcs .nosound2` -> emits only when NO carry.
        if anim >= 8 {
            play_se(g, WM_SE_DRILL);
        } else {
            play_se(g, WM_SE_SPIN);
        }
    }
}

/// `.drillattack` (DSTRATS.ASM:6700-6745): while the launched web lives, open
/// the drill (anim toward 10) and lunge the whole boss toward the player.
fn wm_drill_attack(g: &mut Game, idx: u16) {
    let Some(web) = wm_find_by_shape(g, SH_WM_BOSS_0_3) else {
        // web gone -> retrieve (s_jmp .drillretrieve same tick).
        let s = sid(g, wm_drill_retrieve);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        wm_drill_retrieve(g, idx);
        return;
    };
    // The web's sflag2 (attach window) gates the drill-open anim.
    if g.objs.aliens[web as usize].sflags2 & WM_SFLAG2 != 0 && g.vars.gameframe & 3 == 0 {
        wm_dinc_anim_cap(g, idx, 11);
    }
    // .ok23: at fully-open (anim 10), lunge the mother toward the player.
    if g.objs.aliens[idx as usize].animframe == 10 {
        if let Some(m) = boss_get_mother_obj(g, idx) {
            if !wm_dz_less(g, m, 350) {
                g.objs.aliens[m as usize].worldz =
                    g.objs.aliens[m as usize].worldz.wrapping_sub(20);
            }
        }
    }
    wm_drill_moveagain(g, idx);
}

/// `.drillretrieve` (DSTRATS.ASM:6747-6773): close the drill, pull the boss
/// back; once >=1200 away re-arm the turrets (sflag3) and return to `.strat`.
fn wm_drill_retrieve(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].animframe != 0 {
        wm_ddec_anim(g, idx, 11);
        wm_drill_moveagain(g, idx);
        return;
    }
    // .noanim
    if let Some(m) = boss_get_mother_obj(g, idx) {
        g.objs.aliens[m as usize].worldz = g.objs.aliens[m as usize].worldz.wrapping_add(80);
        if !wm_dz_less(g, m, 1200) {
            // fully retracted: re-arm turrets + resume the main loop.
            wm_child_sflag2_range(g, m, WM_SFLAG3, WM_TURRET1, WM_TURRET6, true);
            let s = sid(g, wm_drill_strat);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            wm_drill_strat(g, idx);
            return;
        }
    }
    wm_drill_moveagain(g, idx);
}

/// drill_istrat `.hit` (DSTRATS.ASM:6684-6698). RebElasercol reflect scoped to
/// hitflash (see section note); the rest-shape path is plain hitflash.
fn wm_drill_hit(g: &mut Game, idx: u16) {
    strat_hit_flash(g, idx);
}

// ============================================================
// launchatplayer (fan detach on death) — DSTRATS.ASM:6787-6798.
// ============================================================

/// launchatplayer_istrat init (DSTRATS.ASM:6788-6790): sbyte2=80, -> `.strat`.
fn wm_launchatplayer_init(g: &mut Game, idx: u16) {
    let s = sid(g, wm_launchatplayer_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = 80;
    al.stratptr = Some(s);
}

/// launchatplayer_istrat `.strat`/`.strat2` (DSTRATS.ASM:6791-6798): count
/// sbyte2 down, then detach from the mother and idle.
fn wm_launchatplayer_strat(g: &mut Game, idx: u16) {
    let sb = g.objs.aliens[idx as usize].sbyte2;
    if sb != 0 {
        g.objs.aliens[idx as usize].sbyte2 = sb - 1; // s_beqdec_alvar
        return;
    }
    // .more: remove from the mother, switch to the inert .strat2.
    if let Some(m) = boss_get_mother_obj(g, idx) {
        // s_remove_child x,y — sever the child link so the fan floats free.
        boss_prune_family_links(g, m);
    }
    boss_clear_child_link(g, idx);
    let s = sid(g, wm_launchatplayer_strat2);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// `.strat2` (DSTRATS.ASM:6797-6798): inert.
fn wm_launchatplayer_strat2(_g: &mut Game, _idx: u16) {}

// ============================================================
// web (launched grab projectile) — DSTRATS.ASM:6800-6883.
// ============================================================

/// web_istrat init (DSTRATS.ASM:6800-6808). Falls into `.strat`.
fn wm_web_init(g: &mut Game, idx: u16) {
    let s = sid(g, wm_web_strat);
    let s_col = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = HARD_HP;
        al.ap = 0;
        al.sflags |= ASF_NOHITAFFECT | ASF_COLLDISABLE;
        al.collflags |= COLLTYPE_ENEMY1;
        al.animframe = 0;
        al.sbyte3 = 100;
    }
    wm_web_strat(g, idx);
}

/// web_istrat `.chkhit` (DSTRATS.ASM:6869-6883): set sflag1 when within 50 in z
/// and inside the [0,100) XY grab ring of the player.
fn wm_web_chkhit(g: &mut Game, idx: u16) {
    let Some(p) = player_idx(g) else {
        g.objs.aliens[idx as usize].sflags2 &= !WM_SFLAG1;
        return;
    };
    let within_z = wm_dz_less(g, idx, 50);
    let me = g.objs.aliens[idx as usize];
    let pl = g.objs.aliens[p as usize];
    let xy = wm_dist_xy(&me, &pl);
    let in_ring = (0..(25 << 2)).contains(&xy); // [0,100)
    if within_z && in_ring {
        if me.sflags2 & WM_SFLAG2 == 0 {
            play_se(g, WM_SE_GRAB);
        }
        g.objs.aliens[idx as usize].sflags2 |= WM_SFLAG1;
    } else {
        g.objs.aliens[idx as usize].sflags2 &= !WM_SFLAG1;
    }
}

/// web_istrat `.strat` (DSTRATS.ASM:6809-6867).
fn wm_web_strat(g: &mut Game, idx: u16) {
    wm_web_chkhit(g, idx);

    if wm_dz_less(g, idx, 600) {
        // within 600: open toward the player (dincanimjmp cap 8).
        wm_dinc_anim_cap(g, idx, 8);
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].sflags2 |= WM_SFLAG2;

    let sflag3 = g.objs.aliens[idx as usize].sflags3 & WM_SFLAG3 != 0;
    let sflag1 = g.objs.aliens[idx as usize].sflags2 & WM_SFLAG1 != 0;
    // sflag3 set OR (sflag3 clear && sflag1 clear) -> .gopast.
    if sflag3 || !sflag1 {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(40);
        al.sflags2 &= !WM_SFLAG2;
    }
    // .move
    wm_web_move(g, idx);
    g.objs.aliens[idx as usize].sflags2 &= !WM_SFLAG1; // .noachase clr sflag1
}

/// `.move` (DSTRATS.ASM:6829-6865): homing (sflag2 set) vs player-grab drag
/// (sflag2 clear) + the shake-free timeout.
fn wm_web_move(g: &mut Game, idx: u16) {
    let Some(p) = player_idx(g) else {
        return;
    };
    if g.objs.aliens[idx as usize].sflags2 & WM_SFLAG2 != 0 {
        // homing: fchase the player + keep player fire enabled.
        let pl = g.objs.aliens[p as usize];
        let me = g.objs.aliens[idx as usize];
        let nx = fb_fchase_word(me.worldx, pl.worldx, 12);
        let ny = fb_fchase_word(me.worldy, pl.worldy, 6);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = nx;
        al.worldy = ny;
        return; // .noachase
    }
    // .nofchase: grab — drag the player onto the web, ease the web toward
    // centre, pin z to the player, then run the shake/timeout.
    {
        let me = g.objs.aliens[idx as usize];
        let pl = &mut g.objs.aliens[p as usize];
        pl.worldx = me.worldx;
        pl.worldy = me.worldy;
    }
    let pz = g.objs.aliens[p as usize].worldz;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = fb_fchase_word(al.worldx, 0, 1);
        al.worldy = fb_fchase_word(al.worldy, 0, 1);
        al.worldz = pz.wrapping_add(40);
    }
    // s_beqdec_alvar sbyte3 -> .finishup on the zero tick.
    let sb3 = g.objs.aliens[idx as usize].sbyte3;
    if sb3 == 0 {
        g.objs.aliens[idx as usize].sflags3 |= WM_SFLAG3; // .finishup
        return;
    }
    g.objs.aliens[idx as usize].sbyte3 = sb3 - 1;

    // shake detection (player_rollZvel path faithful; dir-key edge scoped out).
    let rollzvel = wm8(g, 0x0516) as i8; // g_player_rollZvel
    if rollzvel != 0 {
        if g.objs.aliens[idx as usize].sflags4 & WM_SFLAG8 == 0 {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags4 |= WM_SFLAG8;
            al.sbyte4 = al.sbyte4.wrapping_add(13);
        }
    } else {
        g.objs.aliens[idx as usize].sflags4 &= !WM_SFLAG8;
    }
    // .doneshake: sbyte4 >= 50 -> release.
    if (g.objs.aliens[idx as usize].sbyte4 as i8) >= 50 {
        g.objs.aliens[idx as usize].sflags3 |= WM_SFLAG3;
    }
}

/// Fixed-step chase of a word coordinate toward `target` by `rate`
/// (s_fchase_alvar / s_fchase_alvar2alvar, no overshoot at these rates).
#[inline]
fn fb_fchase_word(cur: i16, target: i16, rate: i16) -> i16 {
    if cur == target {
        cur
    } else if cur < target {
        cur.wrapping_add(rate)
    } else {
        cur.wrapping_sub(rate)
    }
}

// ============================================================
// webmonster mother — DSTRATS.ASM:6504-6566.
// ============================================================

/// `.generate` (DSTRATS.ASM:6572-6583): make the mother + spawn the 6 turret
/// ring + the fan, storing each child's ring offset (childXYZ bytes = value<<
/// boss00_scale(2)>>childscale(3)) and rot offset.
fn wm_generate(g: &mut Game, mother: u16) {
    // (child_num, childx, childy, childz, childrotz, shape, init) in ASM order.
    let turrets: [(u8, i8, i8, i8, u8); 6] = [
        (6, 0, -33, 0, 0),                     // turret6, rotz 0
        (3, 28, -16, 0, (-(WM_DEG60 as i16)) as u8), // rotz -deg60
        (5, 28, 16, 0, (-(WM_DEG120 as i16)) as u8), // rotz -deg120
        (2, 0, 32, 0, DEG180),                 // rotz -deg180 (==128)
        (4, -28, 16, 0, (-(170i16)) as u8),    // rotz -deg240
        (1, -28, -16, 0, (-(212i16)) as u8),   // rotz -deg300
    ];
    for &(num, cx, cy, cz, crz) in &turrets {
        wm_spawn_child(g, mother, SH_WM_BOSS_0_2, num, cx, cy, cz, crz, wm_propturret_init);
    }
    // web_fan (child 7): fan shape, offset (0,0,-5<<3), no rot offset.
    wm_spawn_child(g, mother, SH_WM_BOSS_0_0, WM_FAN, 0, 0, -5, 0, wm_drill_init);
    wm_position(g, mother);
}

/// s_make_childobjrotpos (STRATLIB.INC:670): alloc + attach + store the ring
/// offsets + set the child's strat (NOT run same tick).
#[allow(clippy::too_many_arguments)]
fn wm_spawn_child(
    g: &mut Game,
    mother: u16,
    shape: u16,
    child_num: u8,
    cx: i8,
    cy: i8,
    cz: i8,
    crz: u8,
    init_fn: StrategyFn,
) -> Option<u16> {
    let child = make_obj(g, shape)?;
    copy_pos(g, child, mother);
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    let s = sid(g, init_fn);
    let al = &mut g.objs.aliens[child as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    al.childx = cx as u8;
    al.childy = cy as u8;
    al.childz = cz as u8;
    al.childrotx = 0;
    al.childroty = 0;
    al.childrotz = crz;
    al.stratptr = Some(s);
    Some(child)
}

/// webmonster_istrat init (DSTRATS.ASM:6504-6514). Falls into `.strat`.
pub fn strat_webmonster_init(g: &mut Game, idx: u16) {
    set_bossmaxhp(g, 0); // s_set_bossmaxHP #0 (turrets add 6*20 = 120)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.depthoffset = 1;
        al.collflags |= COLLTYPE_ENEMY1;
        al.rotx = DEG90 + DEG45; // 96
        al.worldy = 1000;
    }
    wm_generate(g, idx);
    let s = sid(g, webmonster_strat);
    let s_col = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col); // hitflash_istrat
        al.expstratptr = None; // 0
        al.hp = HARD_HP; // hardHP (invulnerable body)
        al.ap = HARD_AP; // hardAP
    }
    webmonster_strat(g, idx);
}

/// webmonster_istrat `.strat`/`.mainstrat` (DSTRATS.ASM:6515-6537): the intro
/// descent (rotx 96->0, worldy 1000->0), then the "all turrets dead" death gate.
fn webmonster_strat(g: &mut Game, idx: u16) {
    let al = g.objs.aliens[idx as usize];
    if al.rotx != 0 {
        // .missthat -> .missthat2
        let a = &mut g.objs.aliens[idx as usize];
        a.rotx = a.rotx.wrapping_sub(1);
        a.worldy = a.worldy.wrapping_sub(10);
        wm_move(g, idx);
        return;
    }
    if al.worldy != 0 {
        // .missthat2
        g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize].worldy.wrapping_sub(10);
        wm_move(g, idx);
        return;
    }
    // .mainstrat: all 6 turrets dead -> .bossdead.
    if wm_children_dead(g, idx, WM_TURRET1, WM_TURRET6) {
        wm_bossdead(g, idx);
        return;
    }
    wm_move(g, idx);
}

/// `.move` (DSTRATS.ASM:6533-6537): slow spin, scroll with the player, position
/// the ring.
fn wm_move(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_sub(1);
    add_player_z(g, idx);
    boss_keeprel_to_player(g, idx);
    wm_position(g, idx);
}

/// `.bossdead` (DSTRATS.ASM:6539-6566): fling the fan at the player, spin up,
/// spew medium explosions, then bossexplode at sbyte2==30.
fn wm_bossdead(g: &mut Game, idx: u16) {
    // sbyte2 == 0 (first entry): launch the fan child at the player.
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        if let Some(fan) = boss_find_child_obj(g, idx, WM_FAN) {
            let s = sid(g, wm_launchatplayer_init);
            g.objs.aliens[fan as usize].stratptr = Some(s);
        }
    }
    // rotz += sbyte2 (pre-increment) ; sbyte2 += 1.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte2);
        al.sbyte2 = al.sbyte2.wrapping_add(1);
    }
    let sb = g.objs.aliens[idx as usize].sbyte2;
    if sb == 30 {
        strat_boss_explode_init(g, idx); // bossexplode_istrat
        return;
    }
    if sb < 20 {
        if sb == 1 && g.vars.pshipflags2 & PSF2_PLAYERHP0 == 0 {
            // boss death sting (bgm_music/bgmcnt poke is audio-only, scoped).
            play_se(g, WM_SE_DEATH);
        }
        // makemedexpobj_srou + addrnd2posy_srou.
        if let Some(exp) = b2_make_medium_exp_obj(g, idx) {
            let rx = (sfrtl_random(g) & 0xff) as i8 as i16;
            g.objs.aliens[exp as usize].worldx =
                g.objs.aliens[exp as usize].worldx.wrapping_add(rx);
            let ry = (sfrtl_random(g) & 0xff) as i8 as i16;
            g.objs.aliens[exp as usize].worldy =
                g.objs.aliens[exp as usize].worldy.wrapping_add(ry);
        }
    }
    // .noexp -> jmp .move
    wm_move(g, idx);
}
// WEBMONSTER_END

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

// ============================================================
// MADTRUCKER_BEGIN — "madtrucker" boss family (Route 2 L6 trucker road) +
// madbiker/bike2 escort + the dropped barrier mines.
//
// ASM oracle: `madtrucker_istrat` (DSTRATS.ASM:5233-5717), `madbiker_istrat` /
// `bike2_istrat` (DSTRATS.ASM:4947-5222) and `barrier_istrat`
// (DSTRATS.ASM:5720-5751). Map placement: TRUCKER.ASM:31 (`boss_9_5`,
// madtrucker) + :2/:3/:8/:9 (`air_1`, madbiker), which the Rust map mirrors in
// route2/submaps.rs:312 (STRAT_ADDR_MADTRUCKER) and :284/:285/:295/:296
// (STRAT_ADDR_MADBIKER). def_istrat rows 120/119 are NOT used by the map — the
// objects resolve through the synthetic 0x0500xx strategy-address table exactly
// like the seadragon/bossg/boss8 rows below.
//
// STRUCTURE: the map's `boss_9_5` object is an INVISIBLE controller mother
// (nullshape proxy, ENEMY1 collision, HP=madtruckerHP=64). `.generate`
// (DSTRATS.ASM:5506) spawns ONE visible truck-body child (`boss_9_0`,
// `hard_istrat` = inert/invincible), linked via `al_ptr`, and repositions it on
// the mother every tick (`.position1`, s_add_Roffs2pos flags 1,1,1 offset
// (0,0,-19)<<2 — the reused `fb_positional`). The mother runs a 17-entry
// `s_mode_table` machine (DSTRATS.ASM:5250-5272) that barges the truck up/down
// the road lanes (`.rightlane`/`.leftlane`/`.farleftlane`), opens/closes its
// armour (`.openback`/`.closeback`, the truck-body's anim 0..12), spawns two
// escort bikes (`.maketwobikes` -> `bike2_istrat`), drops barrier mines
// (`.dropmines` -> `barrier_istrat`) and chases the player's worldz with a
// fixed-accel `truck_accel`. The BOSS HP BAR is real: init does
// `s_set_bossmaxHP #madtruckerHP` (DSTRATS.ASM:5245) and every `.move`/`.nomove`
// runs `s_add_bossHP x,al_hp` (:5611). Death is the mother's own
// `.explode` -> `.swerveviolently` -> `.skid` -> `.flippul` chain (:5616-5717,
// NOT trucklaunch/fallingtruck — those belong only to castanet, verified: no
// trucklaunch/fallingtruck reference exists inside the madtrucker span).
//
// SCOPE NOTE (fidelity boundaries, cited inline):
//  * The mother mode machine, truck-body positioning, lane movement, armour
//    open/close vulnerability gate, HP-bar/maxHP, the bike-escort + mine spawns
//    and the full swerve/skid/flip death chain are ported tick-for-tick.
//  * The `.hit` damage gate (DSTRATS.ASM:5580-5603) keys off per-sub-box
//    hitflags HF1 (truck body) / HF2 (mother weak spot) that only the REAL
//    `boss_9_5`/`boss_9_0` collision meshes emit; the map uses SH_NULLSHAPE
//    proxies (route2/rc.rs:112) so in-game the gate currently sees no
//    sub-box hitflags (undamageable until the real shapes are wired — a
//    map-proxy caveat of the same class as castanet's). The gate is ported
//    faithfully and exercised in tests by setting hitflags directly.
//  * Cosmetic-only ROM calls that read cross-object global scratch RAM or
//    unported sprite/particle systems are intentional no-ops, exactly as
//    castanet scoped its ringlaser/mini spread: `sgenspark`/`genspark2`
//    (spark puffs, :5696-5716), `bigwhiteFOsprite`/`circleobj`/`rumble` (the
//    white death flash, :5631-5634), madbiker's `makeengine`/`updateengine`
//    (the engine-flame child, :4970/:5219) and `float64_srou` (the hover bob,
//    :5210 — reads global floatvar1/floatvar2 oscillators). `set_sound2` audio
//    init is likewise dropped (no audio hook in the parity harness).
//  * The escort-bike/mine SHAPES are cosmetic proxies (SH_MT_* below); the map's
//    nullshape `air_1`/`boss_9_5` proxies are overwritten to them on init so the
//    `find_y #air_1` bike lookups (`.waitforbikes`/`.destroybikes`) resolve
//    uniformly for both map-placed and truck-spawned bikes.
// ============================================================

// DSTRATS.ASM:72-77.
const MADTRUCKER_HP: u8 = 64; // madtruckerHP
const MADTRUCKER_AP: u8 = 2; // madtruckerAP
const MADBIKER_HP: u8 = 10; // madbikerHP
const MADBIKER_AP: u8 = 4; // madbikerAP
const BARRIER_HP: u8 = 6; // barrierHP
const BARRIER_AP: u8 = 12; // barrierAP

/// `madtrucker_istrat` / `madbiker_istrat` synthetic addresses (sf-map
/// route2/rc.rs:204-205). Resolved through the strat-address table, like
/// STRAT_ADDR_SEADRAGON.
pub const STRAT_ADDR_MADTRUCKER: u32 = 0x050009;
pub const STRAT_ADDR_MADBIKER: u32 = 0x050008;

// Shape proxies (cosmetic; behaviour is shape-independent). Continue the
// castanet 276-281 proxy range.
const SH_MT_BOSS_9_0: u16 = 282; // truck body (hard child)
const SH_MT_AIR_1: u16 = 283; // escort bike
const SH_MT_BARRIER: u16 = 284; // dropped mine

// Strategy flags (same STRATEQU.INC:912-918 mapping as flingboss/castanet):
// sflag1/sflag2 -> sflags2 0x10/0x20; sflag6 -> sflags3 0x02.
const MT_SFLAG1: u8 = 0x10;
const MT_SFLAG2: u8 = 0x20;
const MT_SFLAG6: u8 = 0x02;

// Hit-flag sub-box bits (VARS.INC:167-168).
const MT_HF1: u8 = 0x01;
const MT_HF2: u8 = 0x02;

// Sound effects.
const MT_SE_OPEN: u8 = 0x5a; // trigse $5a (armour opening)
const MT_SE_CLOSE: u8 = 0x59; // trigse $59 (armour closing)
const MT_SE_SKID: u8 = 0x1d; // trigse $1d (death skid)

// g_maxpmoveX WRAM mirror (common::sv::MAXPMOVEX).
const MT_MAXPMOVEX: u16 = 0x0528;

// ---- anim helpers (0x80 "initialised" marker, STRATLIB.INC:67-90/262-274) ----

#[inline]
fn mt_anim_get(al: &Alien) -> u8 {
    al.animframe & 0x7f
}
#[inline]
fn mt_anim_set(al: &mut Alien, frame: u8) {
    al.animframe = 0x80 | (frame & 0x7f);
}
/// 4-arg `s_add_anim obj,#amount,#max,label` (STRATLIB.INC:180-255): CAP at
/// max-1 and return true (the jump) once the frame would reach `max`.
#[inline]
fn mt_add_anim_cap(al: &mut Alien, amount: u8, max: u8) -> bool {
    let f = mt_anim_get(al).wrapping_add(amount);
    if f < max {
        mt_anim_set(al, f);
        false
    } else {
        mt_anim_set(al, max - 1);
        true
    }
}

// ---- shared small helpers ----

/// `s_jmp_random label,#pct` (STRATMAC.INC:1407-1417): branch when
/// random_l()&0xff < (pct*255)/100.
#[inline]
fn mt_jmp_random(g: &mut Game, pct: u32) -> bool {
    let thresh = ((pct * 255) / 100) as u16;
    (sfrtl_random(g) & 0xff) < thresh
}

#[inline]
fn mt_player_z(g: &Game) -> i16 {
    player(g).map(|p| p.worldz).unwrap_or(0)
}
#[inline]
fn mt_player_x(g: &Game) -> i16 {
    player(g).map(|p| p.worldx).unwrap_or(0)
}

/// Live truck-body child (mother.al_ptr, index+1 encoding).
#[inline]
fn mt_child(g: &Game, mother: u16) -> Option<u16> {
    fb_read_obj(g, g.objs.aliens[mother as usize].ptr)
}

/// `truck_accel` (DSTRATS.ASM:174-197): step al_sword1 (a signed velocity
/// accumulator) toward +10 when the truck is BEHIND the target-z (`behind` =
/// the ROM `bmi` on `(worldz-offset) - player_worldz`) or toward -20 when ahead,
/// then apply it to worldz.
#[inline]
fn mt_truck_accel(al: &mut Alien, behind: bool) {
    let v = al.sword1 as i32;
    al.sword1 = if behind {
        (v + 1).min(10) as i16
    } else {
        (v - 2).max(-20) as i16
    };
    al.worldz = al.worldz.wrapping_add(al.sword1);
}

// ---- lane / armour movement subroutines (return the ROM carry = "secured") ----

/// `.rightlane` (DSTRATS.ASM:5315-5328): creep +2 until worldx>=30.
#[inline]
fn mt_rightlane(al: &mut Alien) -> bool {
    if al.worldx >= 30 {
        true
    } else {
        al.worldx = al.worldx.wrapping_add(2);
        false
    }
}
/// `.leftlane` (DSTRATS.ASM:5330-5344): creep -4 until worldx< -70.
#[inline]
fn mt_leftlane(al: &mut Alien) -> bool {
    if al.worldx < -70 {
        true
    } else {
        al.worldx = al.worldx.wrapping_sub(4);
        false
    }
}
/// `.farleftlane` (DSTRATS.ASM:5393-5408): creep -5 until worldx< -160, then set
/// maptrigger bit0.
fn mt_farleftlane(g: &mut Game, idx: u16) -> bool {
    let wx = g.objs.aliens[idx as usize].worldx;
    if wx < -160 {
        let mt = maptrigger(g) | 1;
        set_maptrigger(g, mt);
        true
    } else {
        g.objs.aliens[idx as usize].worldx = wx.wrapping_sub(5);
        false
    }
}
/// `.openback` (DSTRATS.ASM:5351-5361): raise the truck-body's armour anim by 1
/// (cap 12), chime on anim==1, secure when it caps.
fn mt_openback(g: &mut Game, mother: u16) -> bool {
    let Some(c) = mt_child(g, mother) else {
        return true;
    };
    if mt_anim_get(&g.objs.aliens[c as usize]) == 1 {
        play_se(g, MT_SE_OPEN);
    }
    mt_add_anim_cap(&mut g.objs.aliens[c as usize], 1, 13)
}
/// `.closeback` (DSTRATS.ASM:5367-5377): lower the armour anim by 1, chime on
/// anim==10, secure when it hits 0.
fn mt_closeback(g: &mut Game, mother: u16) -> bool {
    let Some(c) = mt_child(g, mother) else {
        return true;
    };
    let a = mt_anim_get(&g.objs.aliens[c as usize]);
    if a == 10 {
        play_se(g, MT_SE_CLOSE);
    }
    if a == 0 {
        return true;
    }
    mt_anim_set(&mut g.objs.aliens[c as usize], a - 1);
    false
}

// ---- `.position1` / `.move` / `.nomove` (DSTRATS.ASM:5525-5612) ----

/// `.position1` (DSTRATS.ASM:5525-5531): copy the mother's rots onto the child,
/// then place it at (0,0,-19)<<2 rotated by the mother (reusing `fb_positional`).
fn mt_position1(g: &mut Game, mother: u16, child: u16) {
    let m = g.objs.aliens[mother as usize];
    {
        let c = &mut g.objs.aliens[child as usize];
        c.rotx = m.rotx;
        c.roty = m.roty;
        c.rotz = m.rotz;
    }
    // z1 = -24 + (20/4) = -19.
    fb_positional(g, &m, child, 0, 0, -19);
}

/// `.nomove` (DSTRATS.ASM:5607-5612): reposition the child + drain the boss bar.
fn mt_nomove(g: &mut Game, idx: u16) {
    if let Some(c) = mt_child(g, idx) {
        mt_position1(g, idx, c);
    }
    add_bosshp(g, idx); // s_add_bossHP x,al_hp
}
/// `.move` (DSTRATS.ASM:5605-5612): scroll with the world, then `.nomove`.
fn mt_move(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    mt_nomove(g, idx);
}

/// `.move2` (DSTRATS.ASM:5569-5578): truck_accel toward player-z at offset 600.
fn mt_move2(g: &mut Game, idx: u16) {
    let pz = mt_player_z(g);
    let wz = g.objs.aliens[idx as usize].worldz;
    let behind = wz.wrapping_sub(600).wrapping_sub(pz) < 0;
    mt_truck_accel(&mut g.objs.aliens[idx as usize], behind);
    mt_move(g, idx);
}
/// `.move4` (DSTRATS.ASM:5534-5543): truck_accel at offset 1200.
fn mt_move4(g: &mut Game, idx: u16) {
    let pz = mt_player_z(g);
    let wz = g.objs.aliens[idx as usize].worldz;
    let behind = wz.wrapping_sub(1200).wrapping_sub(pz) < 0;
    mt_truck_accel(&mut g.objs.aliens[idx as usize], behind);
    mt_move(g, idx);
}
/// `.move3` (DSTRATS.ASM:5545-5567): truck_accel at offset 120, then snap to
/// player-z+50 if the truck has fallen more than 50 behind.
fn mt_move3(g: &mut Game, idx: u16) {
    let pz = mt_player_z(g);
    let wz = g.objs.aliens[idx as usize].worldz;
    let behind = wz.wrapping_sub(120).wrapping_sub(pz) < 0;
    mt_truck_accel(&mut g.objs.aliens[idx as usize], behind);
    let wz2 = g.objs.aliens[idx as usize].worldz;
    if wz2.wrapping_sub(50).wrapping_sub(pz) < 0 {
        g.objs.aliens[idx as usize].worldz = pz.wrapping_add(50);
    }
    mt_move(g, idx);
}

// ---- escort bike / mine spawns ----

/// Escort-bike lookups (`find_y #air_1`).
fn mt_bikes_alive(g: &Game) -> bool {
    sea_find_shape(g, SH_MT_AIR_1).is_some()
}
/// `.destroybikes` (DSTRATS.ASM:5690-5694): s_kill_obj the first live bike.
fn mt_destroybikes(g: &mut Game) {
    if let Some(b) = sea_find_shape(g, SH_MT_AIR_1) {
        let al = &mut g.objs.aliens[b as usize];
        al.hp = 0;
        al.sflags |= ASF_COLLDISABLE;
    }
}

/// `.maketwobikes` inner (DSTRATS.ASM:5445-5463): spawn one `bike2` at
/// (x1,10,0)<<2 relative to the mother.
fn mt_spawn_bike(g: &mut Game, mother: u16, x1: i16) {
    let Some(child) = make_obj(g, SH_MT_AIR_1) else {
        return;
    };
    copy_pos(g, child, mother);
    let s = sid(g, bike2_strat);
    {
        let m = g.objs.aliens[mother as usize];
        let c = &mut g.objs.aliens[child as usize];
        c.rotx = m.rotx; // copyrots_yx
        c.roty = m.roty;
        c.rotz = m.rotz;
        c.shape = SH_MT_AIR_1;
        c.stratptr = Some(s);
    }
    let m = g.objs.aliens[mother as usize];
    fb_positional(g, &m, child, x1, 10, 0);
}

/// `.dropmines` (DSTRATS.ASM:5290-5300): spawn one `barrier` mine at (0,0,-40)<<2.
fn mt_spawn_barrier(g: &mut Game, mother: u16) {
    let Some(child) = make_obj(g, SH_MT_BARRIER) else {
        return;
    };
    copy_pos(g, child, mother);
    let s = sid(g, barrier_init);
    {
        let m = g.objs.aliens[mother as usize];
        let c = &mut g.objs.aliens[child as usize];
        c.rotx = m.rotx; // copyrots_yx
        c.roty = m.roty;
        c.rotz = m.rotz;
        c.shape = SH_MT_BARRIER;
        c.stratptr = Some(s);
    }
    let m = g.objs.aliens[mother as usize];
    fb_positional(g, &m, child, 0, 0, -40);
}

// ---- truck-body child (hard_istrat = inert/invincible) ----

/// Inert repositioned-by-mother truck body (ROM `hard_istrat`). No per-tick
/// behaviour: the mother's `.position1` drives it.
fn mt_truckbody_strat(_g: &mut Game, _idx: u16) {}

/// `.generate` (DSTRATS.ASM:5506-5522): spawn the visible truck body, link it as
/// the mother's al_ptr and position it. Returns false when the pool is full.
fn mt_generate(g: &mut Game, mother: u16) -> bool {
    let Some(child) = make_obj(g, SH_MT_BOSS_9_0) else {
        return false;
    };
    copy_pos(g, child, mother);
    let s = sid(g, mt_truckbody_strat);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.stratptr = Some(s); // s_set_strat y,hard_istrat
        al.collstratptr = None;
        al.expstratptr = None;
        mt_anim_set(al, 0); // s_init_anim y,#0
        al.type_ &= !ATZREMOVE; // s_clr_altype zremove
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype ENEMY1
        al.hp = HARD_HP; // hard_istrat set_hard_vars (invincible)
        al.ap = HARD_AP;
        al.shape = SH_MT_BOSS_9_0;
    }
    g.objs.aliens[mother as usize].ptr = boss_obj_index_or_null(child);
    mt_position1(g, mother, child);
    true
}

/// `madtrucker_istrat` init (DSTRATS.ASM:5233-5246).
pub fn madtrucker_init(g: &mut Game, idx: u16) {
    set_maptrigger(g, 0); // s_set_var maptrigger,#0
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = MADTRUCKER_HP; // s_set_aldata
        al.ap = MADTRUCKER_AP;
        al.type_ &= !ATZREMOVE; // s_clr_altype zremove
        mt_anim_set(al, 0); // s_init_anim #0
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype ENEMY1
        al.sflags |= ASF_SHADOW; // s_set_alsflag shadow
    }
    if !mt_generate(g, idx) {
        // s_bcs .end: pool full -> retry the whole init next tick (the map's
        // strat pointer is already this init).
        let s = sid(g, madtrucker_init);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        return;
    }
    let s = sid(g, madtrucker_strat);
    let sc = sid(g, madtrucker_hit);
    let se = sid(g, madtrucker_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s); // s_set_alptrs .strat,.hit,.explode
        al.collstratptr = Some(sc);
        al.expstratptr = Some(se);
        al.stratstate = 0; // s_mode_change #0
    }
    set_bossmaxhp(g, MADTRUCKER_HP as u16); // s_set_bossmaxHP #madtruckerHP
    madtrucker_strat(g, idx); // fall into .strat
}

/// `madtrucker_istrat` `.strat` (DSTRATS.ASM:5247-5567): the 17-entry mode
/// machine. `stratstate` holds the mode; arms that end the tick call a `.move*`
/// and return; arms that fall out advance the mode (.nxtmode); `.chkbikesagain`
/// / `.repeat` / the no-bikes branch jump to a specific mode and re-enter.
fn madtrucker_strat(g: &mut Game, idx: u16) {
    loop {
        match g.objs.aliens[idx as usize].stratstate {
            // 0 .bargeforward
            0 => {
                if !mt_rightlane(&mut g.objs.aliens[idx as usize]) {
                    mt_move2(g, idx);
                    return;
                }
            }
            // 1 .maketwobikes
            1 => {
                mt_spawn_bike(g, idx, 7);
                mt_spawn_bike(g, idx, -7);
            }
            // 2 .openup
            2 => {
                if !mt_openback(g, idx) {
                    mt_move(g, idx);
                    return;
                }
            }
            // 3 .moveleftlane
            3 => {
                if !mt_leftlane(&mut g.objs.aliens[idx as usize]) {
                    mt_move2(g, idx);
                    return;
                }
            }
            // 4 .close
            4 => {
                if !mt_closeback(g, idx) {
                    mt_move(g, idx);
                    return;
                }
            }
            // 5 .movefarleft
            5 => {
                if !mt_farleftlane(g, idx) {
                    mt_move2(g, idx);
                    return;
                }
            }
            // 6 .waitforbikes (bikesagain)
            6 => {
                if mt_jmp_random(g, 1) {
                    // rare random advance -> .nxtmode
                } else if mt_bikes_alive(g) {
                    mt_move2(g, idx);
                    return;
                } else {
                    g.objs.aliens[idx as usize].stratstate = 13; // madbikesdead
                    continue;
                }
            }
            // 7 .movefarforward
            7 => {
                if sea_dz_less(g, idx, 1200) {
                    if g.objs.aliens[idx as usize].sflags2 & MT_SFLAG1 != 0 {
                        mt_leftlane(&mut g.objs.aliens[idx as usize]);
                    } else {
                        mt_rightlane(&mut g.objs.aliens[idx as usize]);
                    }
                    mt_move4(g, idx);
                    return;
                }
                // dz>=1200 -> .nxttime: toggle sflag1, then .nxtmode.
                g.objs.aliens[idx as usize].sflags2 ^= MT_SFLAG1;
            }
            // 8 .bumpitup
            8 => {
                g.objs.aliens[idx as usize].worldy =
                    g.objs.aliens[idx as usize].worldy.wrapping_sub(2);
                mt_openback(g, idx);
                mt_openback(g, idx);
                if !mt_openback(g, idx) {
                    mt_move(g, idx);
                    return;
                }
            }
            // 9 .dropmines
            9 => {
                mt_spawn_barrier(g, idx);
            }
            // 10 .waitabit4
            10 => {
                let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
                g.objs.aliens[idx as usize].sbyte1 = sb;
                if sb == 15 {
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                } else {
                    mt_move4(g, idx);
                    return;
                }
            }
            // 11 .bumpitdown
            11 => {
                g.objs.aliens[idx as usize].worldy =
                    g.objs.aliens[idx as usize].worldy.wrapping_add(2);
                mt_closeback(g, idx);
                mt_closeback(g, idx);
                if !mt_closeback(g, idx) {
                    mt_move(g, idx);
                    return;
                }
            }
            // 12 .chkbikesagain
            12 => {
                g.objs.aliens[idx as usize].stratstate = 6; // bikesagain
                continue;
            }
            // 13 .hangback (madbikesdead)
            13 => {
                if mt_rightlane(&mut g.objs.aliens[idx as usize]) {
                    let mt = maptrigger(g) | 1;
                    set_maptrigger(g, mt);
                } else {
                    mt_move3(g, idx);
                    return;
                }
            }
            // 14 .waitabit3
            14 => {
                let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
                g.objs.aliens[idx as usize].sbyte1 = sb;
                if sb == 55 {
                    g.objs.aliens[idx as usize].sbyte1 = 0;
                } else {
                    mt_move3(g, idx);
                    return;
                }
            }
            // 15 .movefarleft3
            15 => {
                if !mt_farleftlane(g, idx) {
                    mt_move3(g, idx);
                    return;
                }
            }
            // 16 .repeat
            _ => {
                g.objs.aliens[idx as usize].stratstate = 0;
                continue;
            }
        }
        // .nxtmode: advance and re-enter the same tick.
        g.objs.aliens[idx as usize].stratstate =
            g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
    }
}

/// `.hit` (DSTRATS.ASM:5580-5603): a hit only counts when the truck-body armour
/// is OPEN (child anim!=0), the body's HF1 sub-box did not absorb it, and the
/// mother's HF2 weak-spot sub-box was struck. Otherwise `nohitaffect` is set and
/// the hit is cosmetic (ROM `hitflash_Istrat` `.nocol`).
fn madtrucker_hit(g: &mut Game, idx: u16) {
    let actual = match mt_child(g, idx) {
        None => false,
        Some(c) => {
            if mt_anim_get(&g.objs.aliens[c as usize]) == 0 {
                false // armour closed -> invulnerable
            } else if g.objs.aliens[c as usize].hitflags & MT_HF1 != 0 {
                g.objs.aliens[c as usize].hitflags &= !MT_HF1; // body armour absorbed
                false
            } else if g.objs.aliens[idx as usize].hitflags & MT_HF2 != 0 {
                g.objs.aliens[idx as usize].hitflags &= !MT_HF2; // weak spot hit
                true
            } else {
                false
            }
        }
    };
    if actual {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        strat_hit_flash(g, idx); // s_docoll: drain hp, may route to .explode
    } else {
        // nohitaffect -> hitflash_Istrat .nocol (GSTRATS.ASM:899/925): drop the
        // collide flag, no damage. Hitflags are NOT cleared here.
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_NOHITAFFECT;
        al.sflags &= !ASF_COLLIDE;
    }
}

// ---- death chain (.explode -> .swerveviolently -> .skid -> .flippul) ----

/// `.explode` (DSTRATS.ASM:5616-5624): the mother's expstrat. Mark the truck
/// body for removal, arm the 35-tick swerve, play the boss-dying stinger, then
/// run the swerve same tick.
fn madtrucker_explode(g: &mut Game, idx: u16) {
    if let Some(c) = mt_child(g, idx) {
        g.objs.aliens[c as usize].type_ |= ATZREMOVE;
    }
    let s = sid(g, madtrucker_swerve);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.expstratptr = Some(s);
        al.sbyte1 = 35;
    }
    boss_dying(g); // trigse $1e + startbgm $f1 (BF_DYING-guarded)
    madtrucker_swerve(g, idx);
}

/// `.swerveviolently` (DSTRATS.ASM:5625-5649): weave the truck (roty from a
/// scaled sintab) while creeping far-left for `sbyte1` ticks, then flip to the
/// skid state.
fn madtrucker_swerve(g: &mut Game, idx: u16) {
    // s_decbne_alvar sbyte1,.swervy: dec THEN branch when !=0.
    let sb1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb1;
    if sb1 != 0 {
        // .swervy: roty = sintab[(gameframe<<4)&0xff] >> 3.
        let gf = (g.vars.gameframe as u8) << 4;
        let v = crate::snes_trig::SINTAB[gf as usize] >> 3;
        g.objs.aliens[idx as usize].roty = v as u8;
        mt_farleftlane(g, idx);
        mt_destroybikes(g);
        // .genspark scoped.
        mt_move(g, idx);
        return;
    }
    // sbyte1 hit 0 -> transition to .skid.
    let s = sid(g, madtrucker_skid);
    let mt = maptrigger(g) | 2;
    set_maptrigger(g, mt);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.expstratptr = Some(s);
        al.sword1 = 20;
    }
    // bigwhiteFOsprite / circleobj / rumble scoped.
    play_se(g, MT_SE_SKID);
    madtrucker_skid(g, idx); // s_jmp .skid
}

/// `.skid` + `.flippul` (DSTRATS.ASM:5651-5688): once the player passes the
/// wreck (player-z >= truck-z) it is removed; until then the wreck swings to
/// the skid heading, slides left, flips over on rotz and coasts on sword1.
fn madtrucker_skid(g: &mut Game, idx: u16) {
    let pz = mt_player_z(g);
    let wz = g.objs.aliens[idx as usize].worldz;
    if pz.wrapping_sub(wz) >= 0 {
        // s_remove_obj x (player has driven past the wreck).
        g.objs.aldead = 1;
        return;
    }
    // .notbehind
    let mt = maptrigger(g) | 2;
    set_maptrigger(g, mt);
    let mut roty = g.objs.aliens[idx as usize].roty;
    let target = (DEG45.wrapping_add(DEG22)).wrapping_neg(); // -(deg45+deg22)
    let reached = achase_angle(&mut roty, target, 2);
    g.objs.aliens[idx as usize].roty = roty;
    if !reached {
        g.objs.aliens[idx as usize].sword1 = 40;
    }
    // .flippul
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_sub(4);
        if al.rotz != DEG90 {
            al.rotz = al.rotz.wrapping_add(8); // deg11
        }
        al.worldz = al.worldz.wrapping_add(al.sword1);
        if al.sword1 >= 0 {
            al.sword1 = al.sword1.wrapping_sub(15);
        }
    }
    mt_destroybikes(g);
    // .genspark / .genspark2 scoped.
    mt_nomove(g, idx); // s_jmp .nomove (no world scroll here)
}

// ============================================================
// barrier_istrat (DSTRATS.ASM:5720-5751) — the dropped mine.
// ============================================================

/// `barrier_istrat` init (DSTRATS.ASM:5720-5727).
fn barrier_init(g: &mut Game, idx: u16) {
    let s = sid(g, barrier_strat);
    let sc = sid(g, barrier_hit);
    let se = sid(g, strat_explode); // explode_istrat
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(sc);
        al.expstratptr = Some(se);
        al.hp = BARRIER_HP;
        al.ap = BARRIER_AP;
        al.roty = DEG180;
        mt_anim_set(al, 0);
        al.vy = 0;
    }
    barrier_strat(g, idx);
}
/// `.strat` (DSTRATS.ASM:5728-5732): fall under gravity; on landing switch to the
/// settle animation.
fn barrier_strat(g: &mut Game, idx: u16) {
    // dfallyvec_x = s_falldown_Yvec x,2,#4,#0.
    if boss2_falldown_yvec(g, idx, 2, 4, 0) {
        let s = sid(g, barrier_strat2);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldy = al.worldy.wrapping_add(al.vy);
}
/// `.strat2` (DSTRATS.ASM:5734-5736): play the deploy anim (0->15), then loop.
fn barrier_strat2(g: &mut Game, idx: u16) {
    if mt_add_anim_cap(&mut g.objs.aliens[idx as usize], 1, 16) {
        let s = sid(g, barrier_strat3);
        g.objs.aliens[idx as usize].stratptr = Some(s);
    }
}
/// `.strat3` (DSTRATS.ASM:5738-5740): loop the settled anim in [13,16)
/// (s_add_anim x,#1,#16,NOJUMP,#13).
fn barrier_strat3(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let mut f = mt_anim_get(al).wrapping_add(1);
    if f >= 16 {
        f = f - 16 + 13;
    }
    mt_anim_set(al, f);
}
/// `.hit` (DSTRATS.ASM:5742-5750): only HF1 (the shootable core) does damage.
fn barrier_hit(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].hitflags & MT_HF1 != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags &= !MT_HF1;
        al.sflags &= !ASF_NOHITAFFECT;
        strat_hit_flash(g, idx);
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_NOHITAFFECT;
        al.sflags &= !ASF_COLLIDE;
    }
}

// ============================================================
// bike2_istrat / madbiker_istrat (DSTRATS.ASM:4947-5222) — the escort bikes.
// ============================================================

/// `bike2_istrat` (DSTRATS.ASM:4947-4960): the 11-tick spawn-drop phase; both
/// the "init" and the tick (re-sets colltype/anim each frame), then hands off to
/// `madbiker_istrat`.
fn bike2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype ENEMY1
        al.shape = SH_MT_AIR_1;
        mt_anim_set(al, 7); // s_init_anim #7
    }
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 < 5 {
            al.sword1 = -20;
        }
        al.worldz = al.worldz.wrapping_sub(5);
        al.sbyte1 = al.sbyte1.wrapping_add(1);
    }
    if g.objs.aliens[idx as usize].sbyte1 == 11 {
        madbiker_init(g, idx); // s_beq madbiker_istrat
    }
}

/// `madbiker_istrat` init (DSTRATS.ASM:4961-4978).
pub fn madbiker_init(g: &mut Game, idx: u16) {
    let s = sid(g, madbiker_strat);
    let sc = sid(g, madbiker_hit);
    let se = sid(g, madbiker_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s); // s_set_alptrs .strat,.hit,.explode
        al.collstratptr = Some(sc);
        al.expstratptr = Some(se);
        al.hp = MADBIKER_HP; // s_set_aldata
        al.ap = MADBIKER_AP;
        al.type_ &= !ATZREMOVE; // s_clr_altype zremove
        mt_anim_set(al, 7); // s_init_anim #7
        al.collflags |= COLLTYPE_ENEMY2; // s_set_colltype ENEMY2
        al.sflags |= ASF_SHADOW; // s_set_alsflag shadow
        al.shape = SH_MT_AIR_1; // (map proxy is nullshape; keep the find_y shape)
        al.sword2 = al.worldy; // s_copy_alvar2alvar sword2,worldy
        al.stratstate = 0; // s_mode_change #0
    }
    // makeengine (engine-flame child) scoped.
    madbiker_strat(g, idx); // fall into .strat
}

/// `madbiker_istrat` `.strat` (DSTRATS.ASM:4979-5122): the 12-entry escort mode
/// machine. Most modes end the tick via `.move`; `.initboost` falls straight
/// into `.doboost`; `.randomjump` re-enters at repeathere/boostit.
fn madbiker_strat(g: &mut Game, idx: u16) {
    loop {
        match g.objs.aliens[idx as usize].stratstate {
            // 0,6 .movealongside
            0 | 6 => {
                if g.objs.aliens[idx as usize].sflags2 & MT_SFLAG1 != 0 {
                    // .gitaway
                    g.objs.aliens[idx as usize].sbyte1 = 7;
                    g.objs.aliens[idx as usize].stratstate = 9; // playaway
                } else {
                    let pz = mt_player_z(g);
                    let wz = g.objs.aliens[idx as usize].worldz;
                    let behind = wz.wrapping_sub(120).wrapping_sub(pz) < 0;
                    mt_truck_accel(&mut g.objs.aliens[idx as usize], behind);
                    let sb = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
                    g.objs.aliens[idx as usize].sbyte2 = sb;
                    if sb == 30 {
                        g.objs.aliens[idx as usize].sbyte2 = 0;
                        g.objs.aliens[idx as usize].stratstate += 1;
                    }
                }
                madbiker_move(g, idx);
                return;
            }
            // 1,7 .shuntplayer
            1 | 7 => {
                g.objs.aliens[idx as usize].sbyte1 = 5;
                if g.objs.aliens[idx as usize].sflags2 & MT_SFLAG1 != 0 {
                    // .spnxtmode -> .nxtmode
                    g.objs.aliens[idx as usize].stratstate += 1;
                    madbiker_move(g, idx);
                    return;
                }
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.worldz = al.worldz.wrapping_sub(15);
                    al.rotz = al.rotz.wrapping_sub(4);
                }
                let px = mt_player_x(g);
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    if al.worldx.wrapping_sub(px) >= 0 {
                        al.rotz = al.rotz.wrapping_add(8);
                    }
                }
                let rotz = g.objs.aliens[idx as usize].rotz;
                if rotz != 0 && rotz.wrapping_add(DEG45) < DEG90 {
                    madbiker_move(g, idx);
                    return;
                }
                // .nmode
                g.objs.aliens[idx as usize].stratstate += 1;
                g.objs.aliens[idx as usize].sflags2 |= MT_SFLAG2;
                madbiker_move(g, idx);
                return;
            }
            // 2,10 .rightwayup
            2 | 10 => {
                let rotz = g.objs.aliens[idx as usize].rotz;
                if rotz == 0 {
                    g.objs.aliens[idx as usize].stratstate += 1;
                } else {
                    let mut v = rotz;
                    if (rotz as i8) >= 0 {
                        v = v.wrapping_sub(8);
                    }
                    v = v.wrapping_add(4);
                    g.objs.aliens[idx as usize].rotz = v;
                }
                madbiker_move(g, idx);
                return;
            }
            // 3 .initboost (falls straight into .doboost)
            3 => {
                g.objs.aliens[idx as usize].stratstate += 1;
                continue;
            }
            // 4 .doboost
            4 => {
                if g.objs.aliens[idx as usize].sflags2 & MT_SFLAG1 != 0 {
                    g.objs.aliens[idx as usize].sbyte1 = 7; // .sp2nxtmode
                    g.objs.aliens[idx as usize].stratstate += 1;
                    madbiker_move(g, idx);
                    return;
                }
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_add(20);
                if gv_beqdec(&mut g.objs.aliens[idx as usize].sbyte1) {
                    // .nxtmodeclr -> .nxtmode
                    g.objs.aliens[idx as usize].sflags2 |= MT_SFLAG2;
                    g.objs.aliens[idx as usize].stratstate += 1;
                }
                madbiker_move(g, idx);
                return;
            }
            // 5,9 .awayfromplayer
            5 | 9 => {
                if g.objs.aliens[idx as usize].sflags2 & MT_SFLAG2 != 0 {
                    // .nnmode
                    g.objs.aliens[idx as usize].sflags2 &= !MT_SFLAG2;
                    g.objs.aliens[idx as usize].stratstate += 1;
                    madbiker_move(g, idx);
                    return;
                }
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_add(10);
                let px = mt_player_x(g);
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    let sb4: i8 = if al.worldx.wrapping_sub(px) >= 0 { -8 } else { 8 };
                    al.rotz = al.rotz.wrapping_add(sb4 as u8);
                }
                // s_decbne_alvar sbyte1,.move
                let sb1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
                g.objs.aliens[idx as usize].sbyte1 = sb1;
                if sb1 == 0 {
                    g.objs.aliens[idx as usize].stratstate += 1;
                }
                madbiker_move(g, idx);
                return;
            }
            // 8 .waittwosecs
            8 => {
                let pz = mt_player_z(g);
                let wz = g.objs.aliens[idx as usize].worldz;
                let behind = wz.wrapping_add(10).wrapping_sub(pz) < 0;
                mt_truck_accel(&mut g.objs.aliens[idx as usize], behind);
                let sb = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
                g.objs.aliens[idx as usize].sbyte2 = sb;
                if sb == 8 {
                    g.objs.aliens[idx as usize].sbyte2 = 0;
                    g.objs.aliens[idx as usize].stratstate += 1;
                }
                madbiker_move(g, idx);
                return;
            }
            // 11 .randomjump
            11 => {
                g.objs.aliens[idx as usize].stratstate = if mt_jmp_random(g, 90) {
                    6 // .repeat -> repeathere
                } else {
                    2 // boostit
                };
                continue;
            }
            _ => {
                g.objs.aliens[idx as usize].stratstate = 6;
                continue;
            }
        }
    }
}

/// `.move` (DSTRATS.ASM:5193-5221): steer worldx by the lean (rotz), ease worldy
/// toward the player, scroll, wall-bounce, clear sflag1.
fn madbiker_move(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // worldx += sign_extend((-rotz) >> 2).
        let neg = (al.rotz as i8).wrapping_neg();
        let delta = (neg >> 2) as i16;
        al.worldx = al.worldx.wrapping_add(delta);
    }
    let py = player(g).map(|p| p.worldy).unwrap_or(0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sword2 = gv_fchase16(al.sword2, py, 1); // s_fchase_alvar2alvar sword2,player.worldy,1
        al.worldy = al.sword2; // s_copy_alvar2alvar worldy,sword2
    }
    // float64 hover bob scoped.
    add_player_z(g, idx);
    madbiker_boundscheck(g, idx);
    g.objs.aliens[idx as usize].sflags2 &= !MT_SFLAG1; // s_clr_alsflag sflag1
    // updateengine scoped.
}

/// `.boundscheck` (DSTRATS.ASM:5166-5190): clamp against the right wall
/// (maxpmoveX-16) and spin rotz on the bounce.
fn madbiker_boundscheck(g: &mut Game, idx: u16) {
    let maxx = wm16s(g, MT_MAXPMOVEX);
    let wx = g.objs.aliens[idx as usize].worldx;
    if wx.wrapping_add(15).wrapping_sub(maxx) >= 0 {
        // .hitwall
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = maxx.wrapping_sub(16);
        // .genspark scoped.
        al.rotz = al.rotz.wrapping_add(8);
    }
}

/// `.hit` (DSTRATS.ASM:5125-5133): only weapon collisions damage the bike;
/// player-body contact sets nohitaffect. Always latches sflag1 (a "hit this
/// frame" marker cleared in `.move`).
fn madbiker_hit(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= MT_SFLAG1;
    let is_weapon = match fb_read_obj(g, g.objs.aliens[idx as usize].collobjptr) {
        Some(c) => g.objs.aliens[c as usize].collflags & ACF_WEAPON != 0,
        None => false,
    };
    if is_weapon {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        strat_hit_flash(g, idx);
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_NOHITAFFECT;
        al.sflags &= !ASF_COLLIDE;
    }
}

/// `.explode` (DSTRATS.ASM:5136-5140): revive to 1hp under a spinning-crash
/// strat so the wreck tumbles before the real explosion.
fn madbiker_explode(g: &mut Game, idx: u16) {
    let s = sid(g, madbiker_konostrat);
    let sc = sid(g, strat_hit_flash);
    let se = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(sc);
        al.expstratptr = Some(se);
        al.sword2 = 0;
        al.hp = 1;
        al.collflags &= !COLLTYPE_ENEMY2;
    }
    madbiker_konostrat(g, idx);
}
/// `.konostrat` (DSTRATS.ASM:5141-5164): drift back (sword2 accel), fall,
/// tumble rotx, and flip over via sflag6 before dying into explode_istrat.
fn madbiker_konostrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(al.sword2);
        al.sword2 = al.sword2.wrapping_add(3);
    }
    // s_falldown_Yvec x,1,#4,#-25,explode_istrat.
    if boss2_falldown_yvec(g, idx, 1, 4, -25) {
        strat_explode(g, idx);
        return;
    }
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(al.vy);
        al.rotx = al.rotx.wrapping_add(4);
    }
    if g.objs.aliens[idx as usize].sflags3 & MT_SFLAG6 != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(24);
        if al.rotx < 24 {
            // .killit: hp0 + colldisable -> explode_istrat next tick.
            al.hp = 0;
            al.sflags |= ASF_COLLDISABLE;
            return;
        }
    }
    // .noflip: latch sflag6 once (worldy+25) low byte is non-negative.
    let low = (g.objs.aliens[idx as usize].worldy.wrapping_add(25) as u16 & 0xff) as u8;
    if low & 0x80 == 0 {
        g.objs.aliens[idx as usize].sflags3 |= MT_SFLAG6;
    }
}
// MADTRUCKER_END

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
    // flingboss (Route 2 L4 armsmap). deadflingboss is reached as the mother's
    // expstrat, but is registered too so the address map resolves it.
    world.istrats[IS_FLINGBOSS] = Some(wsid(world, strat_flingboss_init));
    world.istrats[IS_DEADFLINGBOSS] = Some(wsid(world, flingboss_deadflingboss_init));
    // castanet "Metal Smasher" (Route 2 L5). Resolves through world.istrats[124]
    // exactly like flingboss; the shared trucklaunch/fallingtruck ground-vehicle
    // base is reached from the mother's death (not a def_istrat row of its own).
    world.istrats[IS_CASTANET] = Some(wsid(world, strat_castanet_init));
    // chicken (Route 3 L3). Resolves through world.istrats[117] exactly like
    // flingboss/castanet; the map spawns the SH_BOSS_D_1 body carrying this
    // ISTRAT index. The neck/head/tail segments + wings are child objects.
    world.istrats[IS_CHICKEN] = Some(wsid(world, strat_chicken_init));
    // seadragon2 (Route 3 L3): the map-placed root of a sprouting sea-dragon
    // neck. Resolves through world.istrats[197] exactly like the others.
    // lochnessmonster (198) is registered too — not yet map-placed but reached
    // by the underwater head respawn.
    world.istrats[IS_SEADRAGON2] = Some(wsid(world, sd_seadragon2_init));
    world.istrats[IS_LOCHNESS] = Some(wsid(world, sd_lochness_init));
    // webmonster (Route 3 L2). Resolves through world.istrats[123] exactly like
    // flingboss/castanet/chicken (level3_2.rs:234 spawns SH_BOSS_0_1 carrying
    // this ISTRAT index). The 6 turrets + fan are the mother's child objects.
    world.istrats[IS_WEBMONSTER] = Some(wsid(world, strat_webmonster_init));

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
    // seadragon plain variant — mother-spawned children (mothers.rs mother_snakes).
    let sd = wsid(world, sd_seadragon_init);
    world.register_strategy_address(STRAT_ADDR_SEADRAGON, sd);
    // madtrucker family (Route 2 L6 trucker road). Both the truck and the escort
    // bikes resolve through the synthetic 0x0500xx strategy-address table
    // (TRUCKER.ASM / route2/submaps.rs) — no def_istrat row is used. madbiker is
    // ALSO reached as a runtime transition from bike2 (the truck's spawned
    // escorts), but is registered here for the map-placed bikes too.
    let mt = wsid(world, madtrucker_init);
    world.register_strategy_address(STRAT_ADDR_MADTRUCKER, mt);
    let mb = wsid(world, madbiker_init);
    world.register_strategy_address(STRAT_ADDR_MADBIKER, mb);
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
