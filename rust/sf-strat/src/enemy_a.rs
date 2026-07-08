//! Enemy strategies, front half (enemy_a lane).
//!
//! C oracle: `src/strat/strat_enemy.c`, front-half slices owned by this
//! lane (exact line boundaries against the current C file):
//! - lines 1-1296: statics/hard*, rader, pillar3, skillfly, gates, and the
//!   shared boss-family / homing-projectile helpers (boss_* link walkers,
//!   hmissile1/homingflat/relelaserhome, boss7launcher_fire_hmissile1,
//!   boss7hatchfire_srou),
//! - lines 1975-2787: the full boss1 block through `Strat_Boss1_Init`,
//! - lines 3543-7333: tow0explode, zaco2, worms, items 5/7 + flashplayer,
//!   bomwing, generic aim/fire helpers, tadpole, spacebar walker/shooter,
//!   up1man, zacos, tower0, houdai, zaco3/zaco4/zaco0, para, carrier,
//!   base1, cameleon, HitFlash/Explode, szaco2, zaco1, friendexitbase,
//!   clship CL demos, and the EXPSTRAT explosion block
//!   (delay/circ/bossdelay explode + Boss/QBoss/BossExplode inits).
//!
//! boss7 (1297-1974), bossA (2788-3542), spacepilon/bossF/title (7334-end)
//! belong to the enemy_b/bosses lanes. Shared helpers those lanes need are
//! exported `pub(crate)` from here.
//!
//! Strategy pointers: C assigns function pointers; here every assignment
//! goes through [`sid`], which memoizes the function in the sf-game
//! strategy registry and hands back its `StratId`.

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE1, ACF_COLLTYPE4, ACF_FIRSTFRAME, ACF_WEAPON, AFEXP,
    ASF3_REALOBJ, ASF4_CSPECIAL, ASF4_SFLAG8, ASF4_SPECIAL, ASF_COLLDISABLE, ASF_COLLIDE,
    ASF_HITFLASH, ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATGND, ATLASER, ATMISSILE,
    ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::{
    GF_BOSSDEAD, HARD_AP, HARD_HP, PFM_SHADOWS, PSF2_PLAYERHP0, PSTF_NOTDIE, SPFM_INSIDE,
};

// ============================================================
// Angle constants (C src/variables.h DEG*)
// ============================================================
pub const DEG0: u8 = 0;
pub const DEG11: u8 = 8;
pub const DEG22: u8 = 16;
pub const DEG45: u8 = 32;
pub const DEG90: u8 = 64;
pub const DEG180: u8 = 128;
pub const DEG270: u8 = 192;
pub const DEG360: u16 = 256;

// al_flags bits (C src/variables.h)
pub const AF_LEFT_PL: u8 = 4;

// al_sflags2 bits (C src/game/obj.h ASF2_*)
pub const ASF2_RELEXPLODE: u8 = 0x04;
pub const ASF2_NOEXPSND: u8 = 0x08;
pub const ASF2_SFLAG1: u8 = 0x10;
/// STRATMAC `smflag1` — the strategy-macro latch used by `s_face_player` /
/// `s_initface_player`. Per STRATEQU.INC:910 it is sflags2 bit 0x04 (the same
/// bit as the mislabeled ASF2_RELEXPLODE above; ROM `relexplode` is really a
/// sflags4 bit, so nothing that shares an object with para's smflag1 also uses
/// relexplode). (Audit A #20/#21)
pub const ASF2_SMFLAG1: u8 = 0x04;

// al_sflags3 bits (C src/game/obj.h)
pub const ASF3_CHILDOBJ: u8 = 0x10;
pub const ASF3_MOTHEROBJ: u8 = 0x20;

// al_sflags4 bit read by the renderer (sf-game draw.rs `ASF4_NOPOLYEXP`) to
// suppress the face/poly explosion count.
pub const ASF4_NOPOLYEXP: u8 = 0x04;

// bossflags bits (C src/variables.h BF_*)
pub const BF_FLAG1: u8 = 1;
pub const BF_FLAG2: u8 = 2;
pub const BF_FLAG3: u8 = 4;
pub const BF_DYING: u8 = 16;

// stratflags bits (C src/variables.h)
pub const SF_NOFIRING: u8 = 1;

// pshipflags bits used by items (C src/variables.h)
pub const PSF_LWINGCOLL: u8 = 2;
pub const PSF_RWINGCOLL: u8 = 4;
pub const PSF_BRKLWING: u8 = 8;
pub const PSF_BRKRWING: u8 = 16;
pub const PSF2_DOUBLASER: u8 = 1;
pub const PSF3_BEAMBALL: u8 = 16;

// Collision type helper bits (STRATEQU.INC:950-955). These are aliases of
// the acf_colltype* bits, NOT 0x01/0x02/0x04/0x08 — the old values were
// wrong and, critically, COLLTYPE_ZENEMY==0x08 collided with the player
// laser's ACF_COLLTYPE1 bit, so laser-vs-Zenemy pairs shared a type bit and
// were permanently skipped by the same-category filter.
pub const COLLTYPE_ENEMY1: u8 = 0x10; // acf_colltype2
pub const COLLTYPE_ENEMY2: u8 = 0x20; // acf_colltype3
pub const COLLTYPE_ENEMYWEAP: u8 = 0x40; // acf_colltype4
pub const COLLTYPE_ZENEMY: u8 = 0x01; // acf_colltype6

// ============================================================
// Compat WRAM slots for C globals not yet ported into GameVars.
//
// sf-game API GAP (do not edit sf-game from this lane): the C oracle
// reads/writes these `game_vars.c` globals, but `sf_game::vars::GameVars`
// does not carry them yet. Until they are promoted to GameVars fields,
// this lane stores them in the WRAM mirror (`vars.ram`) at reserved
// addresses — the same mechanism the map VM uses for external vars.
// Everything below 0x0400 is a REAL SNES WRAM address from the C code
// (WM_SKILLFLY); the 0x1Fxx block is lane-reserved scratch.
// ============================================================
pub mod wm {
    /// C `RAM8(WM_SKILLFLY)` (real address, src/strat/strat_enemy.c).
    pub const SKILLFLY: u16 = 0x0304;

    /// C `g_rndval` (u16, src/sf_rtl.c).
    pub const RNDVAL: u16 = 0x1F00;
    /// C `g_bossflags` (u8).
    pub const BOSSFLAGS: u16 = 0x1F02;
    /// C `g_currentlevel` (u8).
    pub const CURRENTLEVEL: u16 = 0x1F03;
    /// C `g_gasflags` (u8).
    pub const GASFLAGS: u16 = 0x1F04;
    /// C `g_stratflags` (u8).
    pub const STRATFLAGS: u16 = 0x1F05;
    /// C `g_playerscore` (u16).
    pub const PLAYERSCORE: u16 = 0x1F06;
    /// C `g_specwepcnt` (u16).
    pub const SPECWEPCNT: u16 = 0x1F08;
    /// C `g_lives` (u8).
    /// Unified with the death path's sv::LIVES store (WRAM 0x0520) — the ROM
    /// has ONE `lives` var (dec PSTRATS.ASM:3266, inc GASTRATS.ASM:2689,
    /// check GSTRATS.ASM:477). The old 0x1F0A address was a third,
    /// disconnected store: 1-UP pickups had no effect.
    pub const LIVES: u16 = 0x0520;
    /// C `g_specials_dead` (u8).
    pub const SPECIALS_DEAD: u16 = 0x1F0B;
    /// C `g_maprestartbank` (u8).
    pub const MAPRESTARTBANK: u16 = 0x1F0C;
    /// C `g_maprestart` (u16).
    pub const MAPRESTART: u16 = 0x1F0E;
    /// C `g_maprestarttemp` (u16).
    pub const MAPRESTARTTEMP: u16 = 0x1F10;
    /// C `g_maprestartbanktemp` (u8).
    pub const MAPRESTARTBANKTEMP: u16 = 0x1F12;
    /// C `g_restartpalfade` (u16).
    pub const RESTARTPALFADE: u16 = 0x1F14;
    /// C `g_lastpalfade` (u16).
    pub const LASTPALFADE: u16 = 0x1F16;
    /// C `g_eroll1` (u8).
    pub const EROLL1: u16 = 0x1F18;
    /// C `g_restartbg` (u16).
    pub const RESTARTBG: u16 = 0x1F1A;
    /// C `g_maxpmoveX` (i16).
    pub const MAXPMOVEX: u16 = 0x1F1C;
    /// C `g_minpmoveX` (i16).
    pub const MINPMOVEX: u16 = 0x1F1E;
    /// C `g_maxpmoveY` (i16).
    pub const MAXPMOVEY: u16 = 0x1F20;
    /// C `g_viewCY` (i16).
    pub const VIEWCY: u16 = 0x1F22;
    /// C `g_pviewposz` (i16).
    pub const PVIEWPOSZ: u16 = 0x1F24;
    /// C `g_pcboxobj_B` (i16 alien index).
    pub const PCBOXOBJ_B: u16 = 0x1F26;
    /// C `g_specflash` (u8) — nova-bomb HUD flash timer. No HUD consumer is
    /// wired yet, so the write is inert, but item5_collect must set it to stay
    /// ROM-faithful (GASTRATS.ASM:2586). (Audit A #24)
    pub const SPECFLASH: u16 = 0x1F28;
    // g_numplasers: consolidated onto `crate::common::sv::NUMPLASERS`
    // (0x0313, the real WM address) — shared with the player-lane laser
    // counter exactly like the single C global.
}

#[inline]
pub fn bossflags(g: &Game) -> u8 {
    g.vars.read_ext8(wm::BOSSFLAGS)
}
#[inline]
pub fn set_bossflags(g: &mut Game, v: u8) {
    g.vars.write_ext8(wm::BOSSFLAGS, v);
}
#[inline]
pub fn gasflags(g: &Game) -> u8 {
    g.vars.read_ext8(wm::GASFLAGS)
}
#[inline]
pub fn set_gasflags(g: &mut Game, v: u8) {
    g.vars.write_ext8(wm::GASFLAGS, v);
}
#[inline]
pub fn currentlevel(g: &Game) -> u8 {
    g.vars.read_ext8(wm::CURRENTLEVEL)
}
#[inline]
pub fn pviewposz(g: &Game) -> i16 {
    g.vars.read_ext16(wm::PVIEWPOSZ) as i16
}

/// C `SfRtl_Random()` (src/sf_rtl.c:192): `g_rndval = rnd*91 + 0x61D7`.
/// The RNG state lives in the compat WRAM slot so the C harness and tests
/// can seed it identically.
pub fn ea_random(g: &mut Game) -> u16 {
    let v = g.vars.read_ext16(wm::RNDVAL);
    let n = v.wrapping_mul(91).wrapping_add(0x61D7);
    g.vars.write_ext16(wm::RNDVAL, n);
    n
}

// ============================================================
// strat_common.c helpers — CONSOLIDATED onto `crate::common`.
//
// The former `ea_compat` duplicate module is gone; every call site now
// goes through the canonical common-lane ports (signatures and byte
// behavior verified against src/strat/strat_common.c). Two deliberate
// notes from the verification:
// - `speed_to`: the old duplicate used `saturating_add`; C (and
//   `crate::common::strat_speed_to`) wrap `vel += rate` at u8 — the
//   canonical version is the C-faithful one.
// - `angle_xz`: stays LOCAL below. C `Strat_AngleXZ` computes
//   `(float)(dst->worldx - src->worldx)` with int promotion (no 16-bit
//   wrap); `crate::common::strat_angle_xz` uses `i16::wrapping_sub`,
//   which diverges once |diff| > 32767. This lane keeps the
//   C-int-promotion form for oracle fidelity.
// ============================================================

pub(crate) use crate::common::{
    apply_velocity, chase_proportional, count_down, dist_xz, gen_vecs_2d, gen_vecs_3d, make_obj,
    spawn_projectile, speed_to,
};
/// Registry-shaped projectile collide strategy (C takes the address of
/// `Strat_ProjectileOnCollide`); same slot ids as the common lane's.
pub(crate) use crate::common::strat_projectile_on_collide as projectile_on_collide_strat;

/// C `Strat_AngleXZ` (strat_common.c:189, anglexy_l): 0-255 yaw from src
/// toward dst. Local (not `crate::common::angle_xz`) — see header note:
/// the C build promotes the int16 difference to int before the float
/// cast, so no 16-bit wrap.
fn angle_xz(src: &Alien, dst: &Alien) -> u8 {
    let dx = (dst.worldx as i32 - src.worldx as i32) as f32;
    let dz = (dst.worldz as i32 - src.worldz as i32) as f32;
    let mut a = dx.atan2(dz);
    if a < 0.0 {
        a += 2.0 * 3.141_592_65_f32;
    }
    ((a * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
}


// ============================================================
// Registry helper — replaces taking a C function's address.
// ============================================================

/// Return the registry id for `f`, registering it on first use. Function
/// pointer identity replaces C `self->stratptr = some_fn`.
pub(crate) fn sid(g: &mut Game, f: StrategyFn) -> StratId {
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

/// Snapshot of the player alien (C `Obj_GetPlayer()`: slot 0 when active).
#[inline]
pub(crate) fn player(g: &Game) -> Option<Alien> {
    g.objs.player().copied()
}

// ============================================================
// Sin/Cos + small shared helpers (C strat_enemy.c:28-68)
// ============================================================

/// C `strat_sin` (strat_enemy.c:28) — same f32 constant folding.
#[inline]
pub fn strat_sin(angle: u8) -> f32 {
    (angle as f32 * (2.0f32 * 3.141_592_65_f32 / 256.0f32)).sin()
}

/// C `strat_cos` (strat_enemy.c:31).
#[inline]
pub fn strat_cos(angle: u8) -> f32 {
    (angle as f32 * (2.0f32 * 3.141_592_65_f32 / 256.0f32)).cos()
}

/// C `achase_angle` (strat_enemy.c:41) — proportional 8-bit angle chase.
/// ROM `Achase_var2A` (STRATMAC.INC:525, `sr8_achase_alvarN`
/// STRATROU.ASM:2760-2795): `diff = target - current`, `nolessrange`
/// pre-clamp (min |step| = 1), then `adiv2` x shift — a signed halve that
/// rounds TOWARD ZERO, not toward -infinity. Oracle-proven against
/// SR8_ACHASE_ALVAR3/4 (sf-oracle tests/audit_strats_b.rs: 0->100 at rate
/// 3 steps 12, not 13).
///
/// Returns true only when already at the target on entry — the ROM
/// macro's reached-branch (`beq`) fires BEFORE stepping, so "reached"
/// reads one tick after arrival, exactly like every `s_achase_alvar
/// ...,label` site.
pub fn achase_angle(current: &mut u8, target: u8, shift: u32) -> bool {
    if *current == target {
        return true;
    }
    let diff = (current.wrapping_sub(target) as i8) as i32;
    let mut step = if diff >= 0 {
        diff >> shift
    } else {
        -((-diff) >> shift)
    };
    if step == 0 {
        step = if diff > 0 { 1 } else { -1 };
    }
    *current = current.wrapping_sub(step as u8);
    false
}

/// C `add_player_z` (strat_enemy.c:55, s_add_playerZ): scroll with world.
pub(crate) fn add_player_z(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(v);
}

/// C `set_hard_vars` (strat_enemy.c:65, s_hardvars).
pub(crate) fn set_hard_vars(al: &mut Alien) {
    al.hp = HARD_HP;
    al.ap = HARD_AP;
}

/// C `strat_points_positive_z` (strat_enemy.c:302).
pub fn strat_points_positive_z(al: &Alien) -> bool {
    let signed_yaw = al.roty as i8;
    signed_yaw >= -(DEG45 as i8) && signed_yaw <= DEG45 as i8
}

/// C `strat_random_centered` (strat_enemy.c:346).
pub(crate) fn strat_random_centered(g: &mut Game, span: u8) -> i8 {
    if span == 0 {
        return 0;
    }
    let rnd = (ea_random(g) % span as u16) as u8;
    (rnd as i8).wrapping_sub((span / 2) as i8)
}

/// C `strat_tab_scaled` (strat_enemy.c:3737): +/-127 table value shifted.
pub fn strat_tab_scaled(angle: u8, use_sin: bool, shift: i32) -> i16 {
    let value = ((if use_sin {
        strat_sin(angle)
    } else {
        strat_cos(angle)
    }) * 127.0f32) as i16;
    if shift < 0 {
        value >> (-shift)
    } else if shift > 0 {
        value << shift
    } else {
        value
    }
}

/// C `frame_tick_mod` — the `s_jmp_notdelay N` gate (STRATMAC.INC:6456-6468):
/// fires when `gameframe & ((1<<N)-1) == 0`, i.e. period 2^N. `step` is the
/// macro's BIT COUNT `N`, NOT a modulus. (Audit A #2)
pub(crate) fn frame_tick_mod(g: &Game, step: u16) -> bool {
    g.vars.gameframe & ((1u16 << step) - 1) == 0
}

/// C `strat_phase_offset` (strat_enemy.c:4574): per-alien phase stagger.
pub(crate) fn strat_phase_offset(idx: u16) -> u8 {
    if (idx as usize) < NUMBER_AL {
        idx as u8
    } else {
        0
    }
}

/// C `strat_obj_index_or_null` (strat_enemy.c:4305) — raw slot index
/// (NOT +1 encoded; 0 doubles as "null", exactly like the C fallback).
pub(crate) fn strat_obj_index_or_null(idx: u16) -> u16 {
    if (idx as usize) < NUMBER_AL {
        idx
    } else {
        0
    }
}

/// C `strat_obj_from_ptr` (strat_enemy.c:4359) — index+1 decode via
/// `Obj_GetByIndex` (bounds-checked, may return an inactive slot).
pub(crate) fn strat_obj_from_ptr(ptr: u16) -> Option<u16> {
    if ptr == 0 {
        return None;
    }
    let idx = ptr as i32 - 1;
    if idx < 0 || idx >= NUMBER_AL as i32 {
        return None;
    }
    Some(idx as u16)
}

/// C `strat_pitch_toward` (strat_enemy.c:4366).
pub(crate) fn strat_pitch_toward(src: &Alien, dst: &Alien) -> u8 {
    let dy = (dst.worldy as i32 - src.worldy as i32) as f32;
    let dx = (dst.worldx as i32 - src.worldx as i32) as f32;
    let dz = (dst.worldz as i32 - src.worldz as i32) as f32;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist <= 1.0 {
        return src.rotx;
    }
    let pitch = dy.atan2(dist);
    ((pitch * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
}

/// C `strat_aim_yaw` (strat_enemy.c:4389).
pub(crate) fn strat_aim_yaw(g: &mut Game, idx: u16, target: &Alien, shift: u32) {
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    achase_angle(&mut roty, angle_xz(&me, target), shift);
    g.objs.aliens[idx as usize].roty = roty;
}

/// C `strat_aim_3d` (strat_enemy.c:4396).
pub(crate) fn strat_aim_3d(g: &mut Game, idx: u16, target: &Alien, shift: u32) {
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    achase_angle(&mut roty, angle_xz(&me, target), shift);
    achase_angle(&mut rotx, strat_pitch_toward(&me, target), shift);
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = roty;
    al.rotx = rotx;
}

/// C `strat_move3d` (strat_enemy.c:4404).
pub(crate) fn strat_move3d(g: &mut Game, idx: u16, speed: u8, accel: u8) {
    let al = &mut g.objs.aliens[idx as usize];
    if accel != 0 {
        let _ = speed_to(al, speed, accel);
    } else {
        al.vel = speed;
    }
    gen_vecs_3d(al);
    apply_velocity(al);
}

/// C `strat_fire_relslowlaser` — ASM `fire_relslowElaser` (GSTRATS.ASM:2548-2561):
/// speed via `doelaserspeed` (48 at level 1, else 60, GSTRATS.ASM:2780),
/// `s_set_lifecnt #40`, `enemylaserAP=2`, and colltypes `enemyweap`
/// (acf_colltype4) AND `laser` (acf_colltype1). The old port hardcoded speed 52
/// / life 55 / enemyweap-only. (Audit A #1, Minor 1)
pub(crate) fn strat_fire_relslowlaser(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let speed = strat_relslowelaser_speed(g);
    let _ = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        40,
        2,
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    );
}

/// C `strat_relslowelaser_speed` (strat_enemy.c:4428).
pub(crate) fn strat_relslowelaser_speed(g: &Game) -> u8 {
    if currentlevel(g) == 1 {
        48
    } else {
        60
    }
}

/// C `strat_fire_relslowlaserhome` (strat_enemy.c:4432).
pub(crate) fn strat_fire_relslowlaserhome(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let speed = strat_relslowelaser_speed(g);
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        RELSLOWELASERHOME_LIFE,
        RELSLOWELASERHOME_AP,
        // ASM fire_relslowElaserHome (GSTRATS.ASM:2569-2571) sets BOTH enemyweap
        // (acf_colltype4) and laser (acf_colltype1). (Audit A Minor 1)
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    ) else {
        return;
    };
    let s_home = sid(g, relelaserhome_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.stratptr = Some(s_home);
    al.rotx = pitch;
    al.roty = yaw;
    al.sbyte1 = pitch;
    al.sbyte2 = yaw;
    al.animframe = 0;
}

/// C `strat_find_near_shape` (strat_enemy.c:4315) — nearest active alien
/// with the given (raw or mapped) shape, walking the active list in order.
pub(crate) fn strat_find_near_shape(
    g: &Game,
    self_idx: u16,
    shape_id: u16,
    exclude: Option<u16>,
    max_z: i16,
    max_xy: i16,
) -> Option<u16> {
    let me = g.objs.aliens[self_idx as usize];
    let mapped_shape = if shape_id < 256 {
        g.world.shapes_table[shape_id as usize]
    } else {
        shape_id
    };
    let mut best: Option<u16> = None;
    let mut best_metric: i32 = 0x7FFF_FFFF;
    for it in g.objs.active_indices() {
        if it == self_idx || Some(it) == exclude {
            continue;
        }
        let al = &g.objs.aliens[it as usize];
        if !al.active {
            continue;
        }
        if !(al.shape == shape_id || al.shape == mapped_shape) {
            continue;
        }
        let dz = (al.worldz as i32 - me.worldz as i32).abs() as i16;
        let dx = (al.worldx as i32 - me.worldx as i32).abs() as i16;
        let dy = (al.worldy as i32 - me.worldy as i32).abs() as i16;
        if dz > max_z || dx > max_xy || dy > max_xy {
            continue;
        }
        let metric = dx as i32 + dy as i32 + dz as i32;
        if metric < best_metric {
            best_metric = metric;
            best = Some(it);
        }
    }
    best
}

/// C `strat_find_near_colltype` (strat_enemy.c:4993).
pub(crate) fn strat_find_near_colltype(
    g: &Game,
    self_idx: u16,
    colltype_mask: u8,
    max_z: i16,
    max_xy: i16,
) -> Option<u16> {
    let me = g.objs.aliens[self_idx as usize];
    let mut best: Option<u16> = None;
    let mut best_metric: i32 = 0x7FFF_FFFF;
    for it in g.objs.active_indices() {
        if it == self_idx {
            continue;
        }
        let al = &g.objs.aliens[it as usize];
        if !al.active || al.collflags & colltype_mask == 0 {
            continue;
        }
        let dz = (al.worldz as i32 - me.worldz as i32).abs() as i16;
        let dx = (al.worldx as i32 - me.worldx as i32).abs() as i16;
        let dy = (al.worldy as i32 - me.worldy as i32).abs() as i16;
        if dz > max_z || dx > max_xy || dy > max_xy {
            continue;
        }
        let metric = dx as i32 + dy as i32 + dz as i32;
        if metric < best_metric {
            best_metric = metric;
            best = Some(it);
        }
    }
    best
}

// ============================================================
// Per-strategy tuning constants (C strat_enemy.c defines)
// ============================================================
const RADER_HP: u8 = 8;
const RADER_AP: u8 = 4;
const SKILLFLY_RADIUS_DEFAULT: i16 = 20 << 2;
const PILLAR3_HP: u8 = 8;
const PILLAR3_FALL_HP: u8 = 4;
const PILLAR3_AP: u8 = 8;
const PILLAR3_DIST: i16 = 500;
const PILLAR3_FALL_FRAMES: u8 = 16;
const ZACOS_HP: u8 = 2;
const ZACOS_AP: u8 = 4;
const ZACO2_HP: u8 = 4;
const ZACO2_AP: u8 = 4;
const SZACO2_HP: u8 = 2;
const SZACO2_AP: u8 = 8;
const SZACO2_SPEED: u8 = 40;
const SZACO2_FIRE_NEAR_Z: i16 = 400;
const SZACO2_FIRE_FAR_Z: i16 = 1500;
const SZACO2_BANK_Z: i16 = 1000;
const SZACO2_DASH_Z: i16 = 600;
const SZACO2_TURN_SHIFT: u32 = 3;
const SZACO2_FIN_SHIFT: u32 = 2;
const SZACO2_FIRE_MASK: u8 = 0x07;
const SZACO2_ANIM_INIT: u8 = 3;
const SZACO2_WPY_OFFSET: i16 = 150;
const ZACO3_HP: u8 = 2;
const ZACO3_AP: u8 = 8;
const ZACO0_HP: u8 = 2;
const ZACO0_AP: u8 = 8;
const ZACO4_HP: u8 = 2;
const ZACO4_AP: u8 = 8;
const HOUDAI_HP: u8 = 8;
const HOUDAI_AP: u8 = 8;
const CAMELEON_HP: u8 = 2;
const CAMELEON_AP: u8 = 8;
const WORM_HP: u8 = 2;
const WORM_AP: u8 = 4;
const WORM2_HP: u8 = 4;
const WORM2_AP: u8 = 2;
const PARA_HP: u8 = 2;
const PARA_AP: u8 = 4;
const PARA_SWINGSPD: i8 = 5;
const PARA_SWINGMAX: i8 = PARA_SWINGSPD * 3;
const CARRIER_HP: u8 = 16;
const CARRIER_AP: u8 = 10;
const CARRIER_RATE: u8 = 30;
const SH_PILLAR3: u16 = 27;
const SH_PARA_0: u16 = 60;
const SH_ZACO_6: u16 = 52;
const SH_HOUDAI_0: u16 = 54;
const SH_ITEM_5: u16 = 158;
const SH_PARA_1_PROXY: u16 = SH_PARA_0;
const GASF_KILLTYPE1: u8 = 0x01;
const GASF_KILLTYPE2: u8 = 0x02;
const GATE3_TOUCH_ZDIST: i16 = 200;
const GATE3_TOUCH_XY: i16 = 25 << 2;
const GATE3_HEAL_AMOUNT: u8 = 20;
const GATE2_TOUCH_ZDIST: i16 = 30 << 1;
const GATE2_TOUCH_XY: i16 = 30 << 1;
const GATE2_HEAL_AMOUNT: u8 = 5;
const GATE2_HEAL_SCORE: u16 = 10;
const GATE2_GROUND_Y: i16 = -30 << 1;
const GATE2_SCROLL_Z: i16 = -50;
const GATE2_TOUCHED_FLAG: u8 = 0x40;
const GATE_HEAL_MAX: u16 = 40;
const GATE_SOUND: u8 = 0x0F;
const GATE3_SOUND: u8 = 0x10;
const GATE_NORM_COLS: u8 = 4;
const GATE_TOUCHED_COL0: u8 = 5;
const GATE_TOUCHED_COLE: u8 = 20;
const BOMWING_HP: u8 = 4;
const BOMWING_AP: u8 = 8;
const BOMWING_SPEED: u8 = 20;
const SHORTPLASMA_SPEED: u8 = 80;
const SHORTPLASMA_LIFE: u8 = 30;
const SHORTPLASMA_AP: u8 = 10;
const HOUDAI_TRACK_MIN_Z: i16 = 200;
const HOUDAI_FIRE_GATE_Z: i16 = 800;
const HOUDAI_TRACK_MAX_Z: i16 = 2000;
const HPLASMA_SPEED: u8 = 60;
const HPLASMA_LIFE: u8 = 50;
const HPLASMA_AP: u8 = 10;
const ITEM5_PICKUP_Z: i16 = 120;
const ITEM5_PICKUP_XY: i16 = 60;
const ITEM5_MAX_SPEC: u16 = 5;
const ITEM5_SCORE: u16 = 100;
const UP1MAN_AP: u8 = 8;
const UP1MAN_PICKUP_X: i16 = 40 * 2;
const UP1MAN_PICKUP_Y: i16 = 60 * 2;
const UP1MAN_PICKUP_Z: i16 = 40 * 2;
const UP1MAN_ACTIVE_Z: i16 = 1500;
const UP1MAN_SCROLL_Z: i16 = 30;
const UP1MAN_ROT_SPEED: u8 = 5;
const UP1MAN_SFLAG1: u8 = 0x10;
const SH_MYSHIP_4: u16 = 2;
// Wireframe Arwing variants proxied on the player mesh (C comment).
const SH_MY_W_PROXY: u16 = SH_MYSHIP_4;
const SH_MY_R_W_PROXY: u16 = SH_MYSHIP_4;
const SH_MY_L_W_PROXY: u16 = SH_MYSHIP_4;
const SH_MY_B_W_PROXY: u16 = SH_MYSHIP_4;
const SH_UP1_MAN_PROXY: u16 = SH_MYSHIP_4;
const ITEM7_PICKUP_Z: i16 = 120;
const ITEM7_PICKUP_XY: i16 = 60;
const ITEM7_SCORE: u16 = 100;
const HF2_MASK: u8 = 0x02;
const CLSHIP_FLAG1: u8 = 0x10;
const CLSHIP_FLAG2: u8 = 0x20;
const CLSHIP_FROGWAIT: i16 = 30;
const CLSHIP_BUNNYWAIT: i16 = 60;
const CLSHIP_COCKWAIT: i16 = 90;
const CLSHIP_GNDWAIT: i16 = 110;
const CLSHIP_WARP_BTIME: i16 = 430;
const GATE3_TOUCHED_FLAG: u8 = 0x10;
const TADPOLE_SIDE_FLAG: u8 = 0x80;
const RELSLOWELASERHOME_LOCK_FLAG: u8 = 0x20;
const RELSLOWELASERHOME_CLOSE_Z: i16 = 800;
const RELSLOWELASERHOME_OFFSCENE_Z: i16 = 12000;
const RELSLOWELASERHOME_LIFE: u8 = 40;
const RELSLOWELASERHOME_AP: u8 = 2;
/// Hit flag HF1 (VARS.INC:167 `HF1 equ 1<<0`) tested by base1's door.
const HF1_MASK: u8 = 0x01;
/// Door open/close sounds — ASM `dooropensound_l`/`doorclosesound_l`
/// (SOUND.ASM:839/850) are multi-channel; approximated by the primary
/// L/C/R channel SE (open 0x54, close 0x52).
const BASE1_DOOR_OPEN_SE: u8 = 0x54;
const BASE1_DOOR_CLOSE_SE: u8 = 0x52;
/// base1 door dwell (ASM `.wait_istrat` `s_set_alvar al_sbyte1,#5`).
const BASE1_WAIT_FRAMES: u8 = 5;
const TADPOLE_HP: u8 = 4;
const TADPOLE_AP: u8 = 10;
const TADPOLE_SPEED: u8 = 30;
const TADPOLE_LIFE: u8 = 60;
const TADPOLE_SWIM_FRAMES: u8 = 40;
const TADPOLE_DIVE_FRAMES: u8 = 20;
const TADPOLE_FIRE_ZDIST: i16 = 1500;
const TADPOLE_BANK_FRAMES: u8 = (DEG180 as u16 + DEG45 as u16) as u8 / 4;
const TADPOLE_ESCAPE_SPEED: u8 = 60;
const BOSS1_HP: u8 = 70;
const BOSS1_AP: u8 = 10;
const BOSS1_TURRET_HP: u8 = 8;
const BOSS1_TURRET_AP: u8 = 16;
const BOSS1_COVER_AP: u8 = 16;
const BOSS1_SPACE_VIEW_CY: i16 = -60;
const BOSS1_CHILD_COVER: u8 = 1;
const BOSS1_CHILD_TL0: u8 = 2;
const BOSS1_CHILD_TL1: u8 = 3;
const BOSS1_CHILD_TL2: u8 = 4;
const BOSS1_CHILD_TL3: u8 = 5;
const BOSS1_CHILD_TR0: u8 = 6;
const BOSS1_CHILD_TR1: u8 = 7;
const BOSS1_CHILD_TR2: u8 = 8;
const BOSS1_CHILD_TR3: u8 = 9;
const BOSS1_PARENT_FLAG_TURRETS_OPEN: u8 = 0x40;
const BOSS1_PARENT_FLAG_SIDE_RIGHT: u8 = 0x80;
const BOSS1_PARENT_FLAG_COVER_BLOCK: u8 = 0x40;
const BOSS1_PARENT_FLAG_COVER_GONE: u8 = 0x80;
const BOSS1_COVER_BLOCK_FRAMES: u8 = 32;
const BOSS1_COVER_CLEAR_FRAMES_EASY: u8 = 50;
const BOSS1_COVER_CLEAR_FRAMES_HARD: u8 = 30;
// boss1 fire cadences are bit-count delay masks applied inline at each site
// (home (gf+idx)&15, normal (gf+idx)&31, center/back missiles (gf+15)&63, back
// plasma gf&63) rather than modulus periods — see boss1turret_common_strat,
// boss1_finish and boss1back_strat (finding #1).
const BOSS1_CLOSE_ZDIST: i16 = 300;
const BOSS1_MISSILE_ZDIST: i16 = 1500;
const BOSS1_COVER_ZOFF: i16 = -300;
// boss_1_0/boss_1_1 raw SHAPES2 flat ids (C comment).
const SH_BOSS_1_0: u16 = 246;
const SH_BOSS_1_1: u16 = 247;
const HMISSILE1_SPEED: u8 = 60;
const HMISSILE1_LIFE: u8 = 100;
const HMISSILE1_AP: u8 = 8;
const HMISSILE1_CLOSE_DIST: i32 = 300;
const HMISSILE1_NOCHASE_FLAG: u8 = 0x01;
const BOSS7_SCALE: u32 = 3;
const ACF_COLLTYPE4_BIT: u8 = ACF_COLLTYPE4;

// ============================================================
// HIT FLASH + EXPLODE (C strat_enemy.c:5895-5950, GSTRATS/EXPSTRAT)
// ============================================================

/// C `Strat_HitFlash` (strat_enemy.c:5895) — damage + flash + death check.
pub fn strat_hit_flash(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp;
    if hp != HARD_HP {
        if hp > 0 {
            g.objs.aliens[idx as usize].hp = hp - 1;
        }
        if g.objs.aliens[idx as usize].hp == 0 {
            match g.objs.aliens[idx as usize].expstratptr {
                Some(exp) => g.call_strat(exp, idx),
                None => strat_explode(g, idx),
            }
            return;
        }
    }
    // ROM hitflash_Istrat (GSTRATS.ASM:895-925): every non-fatal hit plays
    // se_damageenemynear/mid/far ($24/$25/$26) by xzdiffs range to the player
    // (<1000 / <2000 / else). The port was silent.
    play_se_by_range(g, idx, 0x24, 0x25, 0x26);
    g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
}

/// ROM range-selected sound trigger (EXPSTRAT.ASM:855-877 / GSTRATS.ASM:905-925
/// pattern): `xzdiffs_l(self, player)` rangexz < 1000 -> near, < 2000 -> mid,
/// else far.
fn play_se_by_range(g: &mut Game, idx: u16, near: u8, mid: u8, far: u8) {
    let me = g.objs.aliens[idx as usize];
    let se = match g.objs.player() {
        Some(p) => {
            let d = crate::common::strat_dist_xz(&me, p) as u16;
            if d < 1000 {
                near
            } else if d < 2000 {
                mid
            } else {
                far
            }
        }
        None => far,
    };
    g.hooks.play_se(se);
}

/// C `Strat_Explode` (strat_enemy.c:5931, explode_Istrat).
pub fn strat_explode(g: &mut Game, idx: u16) {
    let sflags4 = g.objs.aliens[idx as usize].sflags4;
    if sflags4 & (ASF4_SPECIAL | ASF4_CSPECIAL) != 0 {
        // ROM `s_test_special` (STRATMAC.INC:236-245, run from explode_Icont):
        // increments specials_dead when the object is `special` OR `Cspecial`
        // — the hit-% score numerator (sf-game score.rs). The port only counted
        // CSPECIAL, undercounting the score on stages with plain-special
        // enemies.
        let sd = g.vars.read_ext8(wm::SPECIALS_DEAD);
        if sd < 0xFF {
            g.vars.write_ext8(wm::SPECIALS_DEAD, sd + 1);
        }
        // NOTE (ROM fidelity): the ROM NEVER decrements specialobjtotal (it is
        // set once per stage, WORLD.ASM inc-only, and read by calcteamdamage_l
        // for the score denominator), and GF_BOSSDEAD is set ONLY by the boss
        // explode chains (EXPSTRAT.ASM:56/71/169) — never by a special count.
        // The decrement + specialobjtotal==0 -> GF_BOSSDEAD below is a port
        // invention kept as a legacy boss-death fallback; removing it needs
        // per-boss death-path verification (some bosses may not yet route
        // through strat_boss_explode_init). Score correctness no longer depends
        // on it (sf-game owns a separate non-decremented total_specials).
        if g.world.specialobjtotal > 0 {
            g.world.specialobjtotal -= 1;
        }
        if g.world.specialobjtotal == 0 {
            g.vars.gameflags |= GF_BOSSDEAD;
        }
    }
    // ROM explode chain (EXPSTRAT.ASM:853-877): se_destructenemynear/mid/far
    // ($21/$22/$23) by xzdiffs range to the player, gated on the noexpsnd
    // sflag. The port played $10 (se_itemcatch, the item chime!) on every kill.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_NOEXPSND == 0 {
        play_se_by_range(g, idx, 0x21, 0x22, 0x23);
    }
    g.objs.aldead = 1;
}

// ============================================================
// HARD OBJECT STRATEGIES (GSTRATS.ASM; C strat_enemy.c:357-416)
// ============================================================

/// C `Strat_Hard_Init` (hard_Istrat, GSTRATS.ASM:642-646).
pub fn strat_hard_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    set_hard_vars(al);
    al.stratptr = None;
}

/// C `Strat_Hard180yr_Init` (hard180YR_Istrat, GSTRATS.ASM:654-660).
pub fn strat_hard180yr_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    al.roty = DEG180;
    set_hard_vars(al);
    al.stratptr = None;
}

/// C `Strat_Hard90yr_Init` (KSTRATS.ASM:326-331; writes deg180 like ASM).
pub fn strat_hard90yr_init(g: &mut Game, idx: u16) {
    // ASM hard90YR_Istrat (KSTRATS.ASM:326-331) has NO s_set_colltype (unlike
    // hard180YR); do not make this inert scenery enemy-collidable. (Audit A #34)
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = DEG180;
    set_hard_vars(al);
    al.stratptr = None;
}

/// C `Strat_Hard180yrNZR_Init` (hard180YRNZR_Istrat, GSTRATS.ASM:649-652).
pub fn strat_hard180yr_nzr_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].type_ &= !ATZREMOVE;
    strat_hard180yr_init(g, idx);
}

/// C `hardrot_strat` (GSTRATS.ASM:678-683).
fn hardrot_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = al.rotx.wrapping_add(al.sbyte1);
    al.roty = al.roty.wrapping_add(al.sbyte2);
    al.rotz = al.rotz.wrapping_add(al.sbyte3);
}

/// C `Strat_HardRot_Init` (hardrot_Istrat, GSTRATS.ASM:673-677).
pub fn strat_hardrot_init(g: &mut Game, idx: u16) {
    let s = sid(g, hardrot_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    set_hard_vars(al);
    al.stratptr = Some(s);
}

/// C `Strat_NoColl_Init` (nocoll_Istrat, GSTRATS.ASM:735-739).
pub fn strat_nocoll_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = None;
}

// ============================================================
// RADERS (C strat_enemy.c:418-443)
// ============================================================

/// C `rader0_strat` (strat_enemy.c:418).
fn rader0_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(8);
}

/// C `Strat_Rader0_Init` (strat_enemy.c:425).
pub fn strat_rader0_init(g: &mut Game, idx: u16) {
    let s = sid(g, rader0_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = RADER_HP;
    al.ap = RADER_AP;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ZENEMY;
}

/// C `Strat_Rader1_Init` (strat_enemy.c:437).
pub fn strat_rader1_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].collflags |= COLLTYPE_ENEMY1;
    strat_hard_init(g, idx);
}

// ============================================================
// PILLAR3 (C strat_enemy.c:445-541)
// ============================================================

/// C `pillar3stay_strat` (strat_enemy.c:445) — inert placeholder tick.
fn pillar3stay_strat(_g: &mut Game, _idx: u16) {}

/// C `pillar3stay_init` (strat_enemy.c:449).
fn pillar3stay_init(g: &mut Game, idx: u16) {
    let s = sid(g, pillar3stay_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !(ASF_NOHITAFFECT | ASF_SHADOW);
        al.stratptr = Some(s);
    }
    g.hooks.play_se(0x49);
}

/// C `pillar3fall_strat` (strat_enemy.c:459).
fn pillar3fall_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        pillar3stay_init(g, idx);
    }
}

/// C `pillar3_enter_fall` (strat_enemy.c:473).
fn pillar3_enter_fall(g: &mut Game, idx: u16) {
    let s = sid(g, pillar3fall_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sflags |= ASF_NOHITAFFECT | ASF_SHADOW;
    al.sbyte1 = 4;
    if al.flags & AF_LEFT_PL != 0 {
        al.sbyte1 = (-4i8) as u8;
    }
    al.sbyte2 = PILLAR3_FALL_FRAMES;
}

/// C `pillar3_strat` (strat_enemy.c:487).
fn pillar3_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    if dist_xz(&me, &pl) < PILLAR3_DIST
        || me.hp < PILLAR3_FALL_HP
        || me.hitflags & HF2_MASK != 0
    {
        pillar3_enter_fall(g, idx);
    }
}

/// C `pillar3explode_strat` — ASM `pillarexplode_Istrat` (EXPSTRAT.ASM:1078-1113):
/// spawn 8 medium-exp children along a rotz-rotated line (step = `rotate_8yx_l`
/// of the point (0,-40) by rotz), each with a staggered delayexplode count
/// 0..7, `nopolyexp`, worldz-10; then set the pillar `lifecnt=7` and hand it to
/// `delayremove_Istrat`. The ROM plays NO direct sound here (the old
/// `play_se(0x10)` was the item-catch chime) and sets NO AFEXP on the pillar —
/// the children ARE the explosion. (Audit A #36, Minor 13)
fn pillar3explode_strat(g: &mut Game, idx: u16) {
    use crate::snes_trig::{mulslog, COSTAB, SINTAB};
    let (rotz, sword2) = {
        let al = &g.objs.aliens[idx as usize];
        (al.rotz, al.sword2)
    };
    // rotate_8yx_l(x1=0, y1=-10<<2, angle=rotz) (STRATROU.ASM:1128): with x1=0,
    //   x2 = mulslog(y1, sintab[rotz]);  y2 = mulslog(y1, costab[rotz]).
    let sy = SINTAB[rotz as usize] as i32;
    let cy = COSTAB[rotz as usize] as i32;
    let x2 = mulslog(-40, sy) as i16;
    let y2 = mulslog(-40, cy) as i16;
    let (mut ox, mut oy) = (0i16, 0i16);
    for k in 0..8u8 {
        if let Some(child) = make_medium_exp_obj(g, idx) {
            let al = &mut g.objs.aliens[child as usize];
            // ASM: s_clr_alsflag relexplode, s_set_alsflag nopolyexp.
            al.sflags2 &= !ASF2_RELEXPLODE;
            al.sflags4 |= ASF4_NOPOLYEXP;
            al.worldx = al.worldx.wrapping_add(ox);
            al.worldy = al.worldy.wrapping_add(oy).wrapping_add(sword2);
            al.worldz = al.worldz.wrapping_sub(10);
            al.count = k; // count = 8 - z1, z1 = 8..1
        }
        ox = ox.wrapping_add(x2);
        oy = oy.wrapping_add(y2);
    }
    // s_set_lifecnt x,#7; s_jmp delayremove_Istrat (decbne — fires at count, not
    // count+1; the old pillar3explode_wait used count_down and lingered a frame).
    let s = sid(g, delayremove_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.collflags = 0;
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.count = 7;
}

/// C `Strat_Pillar3_Init` (strat_enemy.c:530).
pub fn strat_pillar3_init(g: &mut Game, idx: u16) {
    let s = sid(g, pillar3_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, pillar3explode_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = PILLAR3_HP;
        al.ap = PILLAR3_AP;
    }
    pillar3_strat(g, idx);
}

// ============================================================
// SKILLFLY (C strat_enemy.c:543-605)
// ============================================================

/// C `skillfly_remove` (strat_enemy.c:543).
fn skillfly_remove(g: &mut Game) {
    let v = g.vars.read_ext8(wm::SKILLFLY);
    if v > 0 {
        g.vars.write_ext8(wm::SKILLFLY, v - 1);
    }
    g.objs.aldead = 1;
}

/// C `skillfly_strat` (strat_enemy.c:550).
fn skillfly_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    if (me.worldz as i32 - pl.worldz as i32).abs() < 200 {
        let mut radius = me.sword1;
        if radius < 0 {
            radius = radius.wrapping_neg();
        }
        let dx = {
            let d = me.worldx.wrapping_sub(pl.worldx);
            if d < 0 {
                d.wrapping_neg()
            } else {
                d
            }
        };
        let dy = {
            let d = me.worldy.wrapping_sub(pl.worldy);
            if d < 0 {
                d.wrapping_neg()
            } else {
                d
            }
        };
        if dx < radius && dy < radius {
            skillfly_remove(g);
            return;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(1000);
    }
    if pl.worldz >= g.objs.aliens[idx as usize].worldz {
        // ASM `s_jmp_objinfront y,x,.rem` (DSTRATS.ASM:8471) reaches `.rem`
        // WITHOUT the `s_dec_var skillfly` — only the caught path (:8459-8460)
        // decrements the skill-ring counter. (Audit A #33)
        g.objs.aldead = 1;
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(1000);
}

/// C `Strat_Skillfly_Init` (strat_enemy.c:592).
pub fn strat_skillfly_init(g: &mut Game, idx: u16) {
    let s = sid(g, skillfly_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        if al.shape == 0 {
            al.sflags |= ASF_INVISIBLE;
        }
        al.stratptr = Some(s);
    }
    let v = g.vars.read_ext8(wm::SKILLFLY);
    g.vars.write_ext8(wm::SKILLFLY, v.wrapping_add(1));
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sword1 == 0 {
            al.sword1 = SKILLFLY_RADIUS_DEFAULT;
        }
    }
    // ASM skillfly_istrat (DSTRATS.ASM:8438) ends `.strat s_start_strat` — the
    // Istrat falls straight through into the strat body on the spawn frame.
    // (Audit A #37)
    skillfly_strat(g, idx);
}

// ============================================================
// GATES (C strat_enemy.c:607-798)
// ============================================================

/// C `gate3_player_box` (strat_enemy.c:607): `Obj_GetByIndex(g_pcboxobj_B)`.
fn gate3_player_box(g: &Game) -> Option<u16> {
    let idx = g.vars.read_ext16(wm::PCBOXOBJ_B) as i16 as i32;
    if idx < 0 || idx >= NUMBER_AL as i32 {
        return None;
    }
    if !g.objs.aliens[idx as usize].active {
        return None;
    }
    Some(idx as u16)
}

/// C `gate_heal_player_box` (strat_enemy.c:617).
fn gate_heal_player_box(g: &mut Game, sound_id: u8, heal_amount: u8) -> bool {
    let Some(box_idx) = gate3_player_box(g) else {
        return false;
    };
    if g.objs.aliens[box_idx as usize].hp == 0 {
        return false;
    }
    let mut hp = g.objs.aliens[box_idx as usize].hp as u16 + heal_amount as u16;
    if hp > GATE_HEAL_MAX {
        hp = GATE_HEAL_MAX;
    }
    g.objs.aliens[box_idx as usize].hp = hp as u8;
    g.hooks.play_se(sound_id);
    true
}

/// C `gate3_strat` (strat_enemy.c:635).
fn gate3_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(8);
        al.roty = al.roty.wrapping_add(6);
        al.rotz = al.rotz.wrapping_add(12);
    }
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    // ASM gate3_strat (GA2STRAT.ASM:2616-2617): `s_jmp_Zdistmore x,y,#200` skips
    // when |dz|>=200 (touch strictly |dz|<200); `s_jmp_outxydistrng ...,#0,#100`
    // in-range is [0,100) so touch requires the XY distance strictly < 100.
    // (Audit A Minor 3)
    if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) >= GATE3_TOUCH_ZDIST {
        return;
    }
    if ((me.worldx as i32 - pl.worldx as i32).abs() as i16) >= GATE3_TOUCH_XY
        || ((me.worldy as i32 - pl.worldy as i32).abs() as i16) >= GATE3_TOUCH_XY
    {
        return;
    }
    if me.sflags2 & GATE3_TOUCHED_FLAG != 0 {
        g.objs.aliens[idx as usize].colframe = 4;
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= GATE3_TOUCHED_FLAG;
    if !gate_heal_player_box(g, GATE3_SOUND, GATE3_HEAL_AMOUNT) {
        return;
    }
    g.objs.aliens[idx as usize].colframe = 4;
}

/// C `Strat_Gate3_Init` (strat_enemy.c:671).
pub fn strat_gate3_init(g: &mut Game, idx: u16) {
    let s = sid(g, gate3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
        al.colframe = 0;
    }
    gate3_strat(g, idx);
}

/// C `gate_spin_strat` (strat_enemy.c:682).
fn gate_spin_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = al.rotz.wrapping_add(al.sbyte1);
    al.sbyte1 = al.sbyte1.wrapping_add(1);
    if al.colframe < GATE_TOUCHED_COL0 || al.colframe >= GATE_TOUCHED_COLE {
        al.colframe = GATE_TOUCHED_COL0;
    } else {
        al.colframe += 1;
    }
}

/// C `gate_strat` (strat_enemy.c:697).
fn gate_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    // ASM gate_strat (DSTRATS.ASM:1755-1758): `dzdistless #200` touches only
    // |dz|<200; `s_jmp_outxydistrng ...,#0,#100` touches only XY distance < 100
    // — both strict. (Audit A Minor 3)
    if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < GATE3_TOUCH_ZDIST
        && ((me.worldx as i32 - pl.worldx as i32).abs() as i16) < GATE3_TOUCH_XY
        && ((me.worldy as i32 - pl.worldy as i32).abs() as i16) < GATE3_TOUCH_XY
        && gate_heal_player_box(g, GATE_SOUND, GATE3_HEAL_AMOUNT)
    {
        let s = sid(g, gate_spin_strat);
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.colframe = 4;
        // Checkpoint latch (C gate_strat, strat_enemy.c:715-719).
        let temp = g.vars.read_ext16(wm::MAPRESTARTTEMP);
        g.vars.write_ext16(wm::MAPRESTART, temp);
        let tbank = g.vars.read_ext8(wm::MAPRESTARTBANKTEMP);
        g.vars.write_ext8(wm::MAPRESTARTBANK, tbank);
        let bg = g.vars.currentbg;
        g.vars.write_ext16(wm::RESTARTBG, bg);
        let lpf = g.vars.read_ext16(wm::LASTPALFADE);
        g.vars.write_ext16(wm::RESTARTPALFADE, lpf);
        g.vars.write_ext8(wm::EROLL1, 1);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.colframe = al.colframe.wrapping_add(1) % GATE_NORM_COLS;
}

/// C `Strat_Gate_Init` (strat_enemy.c:726).
pub fn strat_gate_init(g: &mut Game, idx: u16) {
    let s = sid(g, gate_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.hp = 1;
        al.ap = 0;
        al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
        al.colframe = 0;
    }
    g.vars.write_ext8(wm::EROLL1, 0);
    let mp = g.vars.mapptr;
    g.vars.write_ext16(wm::MAPRESTARTTEMP, mp);
    g.vars.write_ext8(wm::MAPRESTARTBANKTEMP, 0);
}

/// C `gate2_strat` (strat_enemy.c:741).
fn gate2_strat(g: &mut Game, idx: u16) {
    let maxx = g.vars.read_ext16(wm::MAXPMOVEX) as i16;
    let minx = g.vars.read_ext16(wm::MINPMOVEX) as i16;
    let maxy = g.vars.read_ext16(wm::MAXPMOVEY) as i16;
    let miny = g.vars.minpmove_y;
    let viewcy = g.vars.read_ext16(wm::VIEWCY) as i16;
    {
        let me = g.objs.aliens[idx as usize];
        if me.worldx > maxx || me.worldx < minx || me.worldy > maxy || me.worldy <= miny {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = chase_proportional(al.worldx, 0, 4);
            al.worldy = chase_proportional(al.worldy, viewcy, 4);
        }
    }
    // ASM GA2STRAT.ASM:2658 `s_jmp_higher x,#-30<<1,.ngnd` skips the clamp when
    // worldy < -60 (smaller y = higher); the `worldy=-60` floor therefore runs
    // when worldy >= -60. (Audit A #5)
    if g.vars.playerflymode & PFM_SHADOWS != 0
        && g.objs.aliens[idx as usize].worldy >= GATE2_GROUND_Y
    {
        g.objs.aliens[idx as usize].worldy = GATE2_GROUND_Y;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(8);
        al.roty = al.roty.wrapping_add(6);
        al.rotz = al.rotz.wrapping_add(12);
    }
    let pl = player(g);
    let me = g.objs.aliens[idx as usize];
    if me.sflags2 & GATE2_TOUCHED_FLAG != 0 {
        g.objs.aliens[idx as usize].colframe = 0;
    } else if let Some(pl) = pl.filter(|p| {
        // ASM GA2STRAT.ASM:2669 `s_jmp_Zdistmore x,y,#30<<1,.ntouch`: touch only
        // when |dz| strictly < 60. (Audit A Minor 3)
        ((me.worldz as i32 - p.worldz as i32).abs() as i16) < GATE2_TOUCH_ZDIST
    }) {
        // ASM GA2STRAT.ASM:2670 `s_jmp_XYdistmore x,y,#30<<1,.ntouch` uses the
        // COMBINED rangexy=|dx|+|dy| metric (not a per-axis box): touch when
        // rangexy < 60. (Audit A #32)
        let dx = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
        let dy = (me.worldy as i32 - pl.worldy as i32).abs() as i16;
        if dx.wrapping_add(dy) < GATE2_TOUCH_XY
            && gate_heal_player_box(g, GATE3_SOUND, GATE2_HEAL_AMOUNT)
        {
            let score = g.vars.read_ext16(wm::PLAYERSCORE);
            g.vars
                .write_ext16(wm::PLAYERSCORE, score.wrapping_add(GATE2_HEAL_SCORE));
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags2 |= GATE2_TOUCHED_FLAG;
            al.colframe = 0;
        } else {
            g.objs.aliens[idx as usize].colframe = 4;
        }
    } else {
        g.objs.aliens[idx as usize].colframe = 4;
    }
    add_player_z(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(GATE2_SCROLL_Z);
}

/// C `Strat_Gate2_Init` (strat_enemy.c:790).
pub fn strat_gate2_init(g: &mut Game, idx: u16) {
    let s = sid(g, gate2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
    }
    gate2_strat(g, idx);
}

// ============================================================
// BOSS FAMILY LINK HELPERS (C strat_enemy.c:831-1046)
// Shared with the enemy_b/bosses lanes (pub(crate)).
// Child links: `ptr` holds mother index+1, `sword1` chains sibling
// index+1 values, `sbyte1` is the child slot number.
// ============================================================

/// C `boss_obj_index_or_null` (strat_enemy.c:831) — index+1 encoding.
pub(crate) fn boss_obj_index_or_null(idx: u16) -> u16 {
    if (idx as usize) < NUMBER_AL {
        idx + 1
    } else {
        0
    }
}

/// C `boss_child_from_index_raw` (strat_enemy.c:837).
pub(crate) fn boss_child_from_index_raw(index: u16) -> Option<u16> {
    if index == 0 {
        return None;
    }
    let idx = index - 1;
    if idx as usize >= NUMBER_AL {
        return None;
    }
    Some(idx)
}

/// C `boss_clear_child_link` (strat_enemy.c:844).
pub(crate) fn boss_clear_child_link(g: &mut Game, child: u16) {
    let al = &mut g.objs.aliens[child as usize];
    al.sflags3 &= !ASF3_CHILDOBJ;
    al.ptr = 0;
    al.sword1 = 0;
}

/// C `boss_prune_family_links` (strat_enemy.c:851).
pub(crate) fn boss_prune_family_links(g: &mut Game, mother: u16) {
    if g.objs.aliens[mother as usize].sflags3 & ASF3_MOTHEROBJ == 0 {
        return;
    }
    let mother_idx = boss_obj_index_or_null(mother);
    let mut prev_idx: u16 = 0;
    let mut idx = g.objs.aliens[mother as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != 0 && guard > 0 {
        guard -= 1;
        let Some(raw) = boss_child_from_index_raw(idx) else {
            break;
        };
        let raw_al = g.objs.aliens[raw as usize];
        let next_idx = raw_al.sword1 as u16;
        let valid = raw_al.active
            && raw_al.sflags3 & ASF3_CHILDOBJ != 0
            && raw_al.ptr == mother_idx;
        if !valid {
            if prev_idx == 0 {
                g.objs.aliens[mother as usize].sword1 = next_idx as i16;
            } else if let Some(prev) = boss_child_from_index_raw(prev_idx) {
                g.objs.aliens[prev as usize].sword1 = next_idx as i16;
            }
            if raw_al.ptr == mother_idx {
                boss_clear_child_link(g, raw);
            }
            idx = next_idx;
            continue;
        }
        prev_idx = idx;
        idx = next_idx;
    }
    if g.objs.aliens[mother as usize].sword1 as u16 == 0 {
        g.objs.aliens[mother as usize].sflags3 &= !ASF3_MOTHEROBJ;
    }
}

/// C `boss_get_mother_obj` (strat_enemy.c:904).
pub(crate) fn boss_get_mother_obj(g: &mut Game, child: u16) -> Option<u16> {
    if g.objs.aliens[child as usize].sflags3 & ASF3_CHILDOBJ == 0 {
        return None;
    }
    let ptr = g.objs.aliens[child as usize].ptr;
    match boss_child_from_index_raw(ptr) {
        Some(m) if g.objs.aliens[m as usize].active => Some(m),
        _ => {
            boss_clear_child_link(g, child);
            None
        }
    }
}

/// C `boss_find_child_obj` (strat_enemy.c:922).
pub(crate) fn boss_find_child_obj(g: &mut Game, mother: u16, child_num: u8) -> Option<u16> {
    boss_prune_family_links(g, mother);
    let mut idx = g.objs.aliens[mother as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != 0 && guard > 0 {
        guard -= 1;
        let child = boss_child_from_index_raw(idx)?;
        let al = g.objs.aliens[child as usize];
        if al.active
            && al.sflags3 & ASF3_CHILDOBJ != 0
            && al.ptr == boss_obj_index_or_null(mother)
            && al.sbyte1 == child_num
        {
            return Some(child);
        }
        idx = al.sword1 as u16;
    }
    None
}

/// C `boss_count_children` (strat_enemy.c:949).
pub(crate) fn boss_count_children(g: &mut Game, mother: u16) -> u8 {
    boss_prune_family_links(g, mother);
    let mut count = 0u8;
    let mut idx = g.objs.aliens[mother as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != 0 && guard > 0 {
        guard -= 1;
        let Some(child) = boss_child_from_index_raw(idx) else {
            break;
        };
        count += 1;
        idx = g.objs.aliens[child as usize].sword1 as u16;
    }
    count
}

/// C `boss_attach_child_to_mother` (strat_enemy.c:973).
pub(crate) fn boss_attach_child_to_mother(
    g: &mut Game,
    mother: u16,
    child: u16,
    child_num: u8,
) -> bool {
    if mother == child
        || !g.objs.aliens[mother as usize].active
        || !g.objs.aliens[child as usize].active
    {
        return false;
    }
    g.objs.aliens[mother as usize].sflags3 |= ASF3_MOTHEROBJ;
    {
        let al = &mut g.objs.aliens[child as usize];
        al.sflags3 |= ASF3_CHILDOBJ;
        al.sbyte1 = child_num;
        al.ptr = boss_obj_index_or_null(mother);
        al.sword1 = 0;
    }
    let child_idx = boss_obj_index_or_null(child);
    if child_idx == 0 {
        boss_clear_child_link(g, child);
        return false;
    }
    if g.objs.aliens[mother as usize].sword1 as u16 == 0 {
        g.objs.aliens[mother as usize].sword1 = child_idx as i16;
        return true;
    }
    let mut idx = g.objs.aliens[mother as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != 0 && guard > 0 {
        guard -= 1;
        let Some(it) = boss_child_from_index_raw(idx) else {
            break;
        };
        if g.objs.aliens[it as usize].sword1 as u16 == 0 {
            g.objs.aliens[it as usize].sword1 = child_idx as i16;
            return true;
        }
        idx = g.objs.aliens[it as usize].sword1 as u16;
    }
    g.objs.aliens[mother as usize].sword1 = child_idx as i16;
    true
}

/// C `boss_keeprel_to_player` (strat_enemy.c:1017).
pub(crate) fn boss_keeprel_to_player(g: &mut Game, idx: u16) {
    let d = g.vars.playervel_z.wrapping_sub(g.vars.pviewvelz);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(d);
}

/// Position half of ROM `s_add_roffs2pos ...,0,1,0` (yaw-rotate the local
/// offset, write position only — NO rotation copy). The bossA turrets
/// (GB3STRAT.ASM:1226) use exactly this: their `roty` is aim state driven
/// by the Achase toward `sbyte3` and must never be stomped by the mother.
pub(crate) fn boss_yaw_offset_pos(
    g: &mut Game,
    self_idx: u16,
    mother: &Alien,
    offx: i16,
    offy: i16,
    offz: i16,
) {
    let s = strat_sin(mother.roty);
    let c = strat_cos(mother.roty);
    let rx = ((offx as f32 * c) + (offz as f32 * s)).round() as i16;
    let rz = ((offz as f32 * c) - (offx as f32 * s)).round() as i16;
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = mother.worldx.wrapping_add(rx);
    al.worldy = mother.worldy.wrapping_add(offy);
    al.worldz = mother.worldz.wrapping_add(rz);
}

/// C `boss_apply_yaw_offset` (strat_enemy.c:1024): place `self` at
/// mother + yaw-rotated local offset, copying the mother rotation
/// (ROM sites that pair `s_copy_rots` with the offset macro).
pub(crate) fn boss_apply_yaw_offset(
    g: &mut Game,
    self_idx: u16,
    mother: &Alien,
    offx: i16,
    offy: i16,
    offz: i16,
) {
    boss_yaw_offset_pos(g, self_idx, mother, offx, offy, offz);
    let al = &mut g.objs.aliens[self_idx as usize];
    al.rotx = mother.rotx;
    al.roty = mother.roty;
    al.rotz = mother.rotz;
}

/// C `boss7hatchfire_srou` (strat_enemy.c:1048): spawn a zaco2 out of a
/// boss7 hatch. Lives here because zaco2 is an enemy_a strategy; the
/// enemy_b boss7 port calls this.
pub(crate) fn boss7hatchfire_srou(g: &mut Game, self_idx: u16) -> Option<u16> {
    let child = make_obj(g, SH_ZACO_6)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(
        g,
        child,
        &me,
        -(10 << BOSS7_SCALE),
        7 << BOSS7_SCALE,
        0,
    );
    let s = sid(g, zaco2_istrat);
    let al = &mut g.objs.aliens[child as usize];
    al.rotx = al.rotx.wrapping_add(3);
    al.stratptr = Some(s);
    Some(child)
}

// ============================================================
// HOMING PROJECTILES (C strat_enemy.c:1066-1295)
// ============================================================

/// C `hmissile1_remove` (strat_enemy.c:1066).
pub(crate) fn hmissile1_remove(g: &mut Game, _idx: u16) {
    g.objs.aldead = 1;
}

/// Registry-shaped wrapper for expstrat slots holding hmissile1_remove.
pub(crate) fn hmissile1_remove_strat(g: &mut Game, idx: u16) {
    hmissile1_remove(g, idx);
}

/// C `projectile_target_obj` (strat_enemy.c:1073).
pub(crate) fn projectile_target_obj(g: &Game, idx: u16) -> Option<u16> {
    let ptr = g.objs.aliens[idx as usize].fireobjptr;
    if ptr == 0 {
        return None;
    }
    let t = ptr as i32 - 1;
    if t < 0 || t >= NUMBER_AL as i32 {
        return None;
    }
    Some(t as u16)
}

/// C `boss7launcher_fire_hmissile1` (strat_enemy.c:1080). Also used by
/// zaco2 (loop attack) in this lane.
pub(crate) fn boss7launcher_fire_hmissile1(
    g: &mut Game,
    self_idx: u16,
    target: u16,
) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(
        g,
        shot,
        &me,
        17 << BOSS7_SCALE,
        -(5 << BOSS7_SCALE),
        0,
    );
    let s_tick = sid(g, hmissile1_strat);
    let s_coll = sid(g, projectile_on_collide_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 2;
    al.ap = HMISSILE1_AP;
    al.vel = HMISSILE1_SPEED;
    al.count = HMISSILE1_LIFE;
    al.snd2 = 2;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.sflags |= ASF_SHADOW;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    gen_vecs_3d(al);
    Some(shot)
}

/// C `hmissile1_strat` (strat_enemy.c:1149).
pub(crate) fn hmissile1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    let target = projectile_target_obj(g, idx);
    if g.objs.aliens[idx as usize].sflags2 & HMISSILE1_NOCHASE_FLAG == 0 {
        let target_al = target
            .map(|t| g.objs.aliens[t as usize])
            .filter(|t| t.active);
        if let Some(t) = target_al {
            let me = g.objs.aliens[idx as usize];
            let dist = (me.worldx as i32 - t.worldx as i32).abs()
                + (me.worldy as i32 - t.worldy as i32).abs()
                + (me.worldz as i32 - t.worldz as i32).abs();
            if dist < HMISSILE1_CLOSE_DIST {
                g.objs.aliens[idx as usize].sflags2 |= HMISSILE1_NOCHASE_FLAG;
            } else {
                strat_aim_3d(g, idx, &t, 3);
            }
        }
        gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        hmissile1_remove(g, idx);
        return;
    }
    let Some(pl) = player(g) else { return };
    let dz = g.objs.aliens[idx as usize].worldz.wrapping_sub(pl.worldz);
    if dz < -12000 || dz > 12000 {
        hmissile1_remove(g, idx);
        return;
    }
    if g.vars.gameflags & GF_BOSSDEAD != 0 || bossflags(g) & BF_DYING != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = 0;
        hmissile1_remove(g, idx);
    }
}

/// C `homingflat_strat` (strat_enemy.c:1204).
pub(crate) fn homingflat_strat(g: &mut Game, idx: u16) {
    let target = projectile_target_obj(g, idx)
        .map(|t| g.objs.aliens[t as usize])
        .filter(|t| t.active);
    if let Some(t) = target {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - t.worldz as i32).abs() >= 500 {
            let mut sb1 = me.sbyte1;
            let mut sb2 = me.sbyte2;
            achase_angle(&mut sb1, angle_xz(&me, &t), 4);
            achase_angle(&mut sb2, strat_pitch_toward(&me, &t), 4);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = sb1;
            al.sbyte2 = sb2;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.sbyte1;
        al.rotx = al.sbyte2;
        gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        hmissile1_remove(g, idx);
        return;
    }
    let Some(pl) = player(g) else { return };
    let dz = g.objs.aliens[idx as usize].worldz.wrapping_sub(pl.worldz);
    if dz < -12000 || dz > 12000 {
        hmissile1_remove(g, idx);
        return;
    }
    if g.vars.gameflags & GF_BOSSDEAD != 0 || bossflags(g) & BF_DYING != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.hp = 0;
        hmissile1_remove(g, idx);
    }
}

/// C `relelaserhome_strat` (strat_enemy.c:1251).
pub(crate) fn relelaserhome_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe < 4 {
            al.animframe += 2;
            if al.animframe > 4 {
                al.animframe = 4;
            }
        }
    }
    let pl = player(g);
    if g.objs.aliens[idx as usize].sflags2 & RELSLOWELASERHOME_LOCK_FLAG == 0 {
        if let Some(pl) = pl {
            let me = g.objs.aliens[idx as usize];
            if ((me.worldz as i32 - pl.worldz as i32).abs() as i16)
                <= RELSLOWELASERHOME_CLOSE_Z
            {
                g.objs.aliens[idx as usize].sflags2 |= RELSLOWELASERHOME_LOCK_FLAG;
            }
            strat_aim_3d(g, idx, &pl, 1);
            gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    if g.objs.aliens[idx as usize].count == 0 {
        hmissile1_remove(g, idx);
        return;
    }
    let Some(pl) = pl else { return };
    let dz = g.objs.aliens[idx as usize].worldz.wrapping_sub(pl.worldz);
    if dz < -RELSLOWELASERHOME_OFFSCENE_Z || dz > RELSLOWELASERHOME_OFFSCENE_Z {
        hmissile1_remove(g, idx);
    }
}

// ============================================================
// BOSS1 — asteroid belt boss (GBSTRATS.ASM; C strat_enemy.c:1975-2787)
// ============================================================

/// C `boss1_cover_clear_frames` (strat_enemy.c:1987).
fn boss1_cover_clear_frames(g: &Game) -> u8 {
    if currentlevel(g) == 1 {
        BOSS1_COVER_CLEAR_FRAMES_EASY
    } else {
        BOSS1_COVER_CLEAR_FRAMES_HARD
    }
}

/// C `boss1_spawn_child` (strat_enemy.c:2005).
fn boss1_spawn_child(g: &mut Game, mother: u16, child_num: u8, init_fn: StrategyFn) -> Option<u16> {
    let shape = if child_num == BOSS1_CHILD_COVER {
        SH_BOSS_1_0
    } else {
        SH_BOSS_1_1
    };
    let child = g.objs.alloc()?;
    sf_game::obj::strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    g.objs.aliens[child as usize].shape = shape;
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    init_fn(g, child);
    Some(child)
}

/// C `boss1_release_children` (strat_enemy.c:2030).
fn boss1_release_children(g: &mut Game, self_idx: u16) {
    boss_prune_family_links(g, self_idx);
    let mut idx = g.objs.aliens[self_idx as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while idx != 0 && guard > 0 {
        guard -= 1;
        let Some(child) = boss_child_from_index_raw(idx) else {
            break;
        };
        let next_idx = g.objs.aliens[child as usize].sword1 as u16;
        boss_clear_child_link(g, child);
        g.objs.free(child);
        idx = next_idx;
    }
    let al = &mut g.objs.aliens[self_idx as usize];
    al.sword1 = 0;
    al.sflags3 &= !ASF3_MOTHEROBJ;
}

/// C `boss1_cover_obj` (strat_enemy.c:2059).
fn boss1_cover_obj(g: &mut Game, self_idx: u16) -> Option<u16> {
    boss_find_child_obj(g, self_idx, BOSS1_CHILD_COVER)
}

/// C `boss1_child_bank_alive` (strat_enemy.c:2063).
fn boss1_child_bank_alive(g: &mut Game, self_idx: u16, first: u8, last: u8) -> bool {
    for child_num in first..=last {
        if boss_find_child_obj(g, self_idx, child_num).is_some() {
            return true;
        }
    }
    false
}

/// C `boss1_live_turret_count` (strat_enemy.c:2078).
fn boss1_live_turret_count(g: &mut Game, self_idx: u16) -> u8 {
    let mut count = 0u8;
    for child_num in BOSS1_CHILD_TL0..=BOSS1_CHILD_TR3 {
        if boss_find_child_obj(g, self_idx, child_num).is_some() {
            count += 1;
        }
    }
    count
}

/// C `boss1_get_turret_offset` (strat_enemy.c:2095). The ring positions are
/// the boss1rots `s_set_3vars` values (GBSTRATS.ASM:291-312) AFTER the
/// dobossrot `s_add_roffs2pos ...,0,1,1,1,1,1` per-axis `<<1` scaling
/// (STRATMAC.INC:4174-4180) — i.e. table halves doubled to world units.
fn boss1_get_turret_offset(child_num: u8) -> Option<(i16, i16, i16)> {
    match child_num {
        n if n == BOSS1_CHILD_TL0 => Some((110, 0, 90)),
        n if n == BOSS1_CHILD_TL1 => Some((250, 0, 90)),
        n if n == BOSS1_CHILD_TL2 => Some((180, -50, 90)),
        n if n == BOSS1_CHILD_TL3 => Some((180, 50, 90)),
        n if n == BOSS1_CHILD_TR0 => Some((-250, 0, 90)),
        n if n == BOSS1_CHILD_TR1 => Some((-110, 0, 90)),
        n if n == BOSS1_CHILD_TR2 => Some((-180, -50, 90)),
        n if n == BOSS1_CHILD_TR3 => Some((-180, 50, 90)),
        _ => None,
    }
}

/// Position `self` at `base` + `off` rotated by the base's FULL rotation in ROM
/// order rotz(rotate_8yx) -> rotx(rotate_8yz) -> roty(rotate_8xz). Mirrors
/// `b2_full_offset_pos` (bosses.rs). Covers both boss1 muzzle placement
/// (fire_weapon `s_add_Roffs2pos` flags 1,1,1, GSTRATS.ASM:2795) and the ring/
/// cover placement (dobossrot/boss1rots flags 0,1,1 = rotz+roty; boss1's rotx is
/// always 0 so the rotx stage is identity there). POSITION ONLY — callers assign
/// the child/shot rotations separately.
fn boss1_rot_offset_pos(g: &mut Game, self_idx: u16, base: &Alien, offx: i16, offy: i16, offz: i16) {
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
    // z' = z*cos - x*sin.
    let s = strat_sin(base.roty);
    let c = strat_cos(base.roty);
    let rx = ((x1 * c) + (z2 * s)).round() as i16;
    let rz = ((z2 * c) - (x1 * s)).round() as i16;
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(y2 as i16);
    al.worldz = base.worldz.wrapping_add(rz);
}

/// C `boss1_update_child_positions` (strat_enemy.c:2147).
fn boss1_update_child_positions(g: &mut Game, self_idx: u16) {
    let cover = boss1_cover_obj(g, self_idx);
    let me = g.objs.aliens[self_idx as usize];
    if let Some(cover) = cover {
        // boss1rots: cover offset (sbyte4<<1, 0, 0) rotated rotz+roty, then z-300
        // (GBSTRATS.ASM:283-285; sbyte4 carries the ±(32*4/2) slide already, the
        // `<<1` is the dobossrot per-axis scale). Rotations copied from mother.
        let sb4 = (g.objs.aliens[cover as usize].sbyte4 as i8 as i16) << 1;
        boss1_rot_offset_pos(g, cover, &me, sb4, 0, 0);
        let al = &mut g.objs.aliens[cover as usize];
        al.worldz = al.worldz.wrapping_add(BOSS1_COVER_ZOFF);
        al.rotx = me.rotx;
        al.roty = me.roty;
        al.rotz = me.rotz;
    } else {
        g.objs.aliens[self_idx as usize].sflags4 &= !BOSS1_PARENT_FLAG_COVER_BLOCK;
    }
    for child_num in BOSS1_CHILD_TL0..=BOSS1_CHILD_TR3 {
        let Some(child) = boss_find_child_obj(g, self_idx, child_num) else {
            continue;
        };
        let Some((offx, offy, offz)) = boss1_get_turret_offset(child_num) else {
            continue;
        };
        let me = g.objs.aliens[self_idx as usize];
        // dobossrot: ring offset rotated rotz+roty (only rotz copied to the
        // turret; roty stays deg180 from init — GBSTRATS.ASM:398 copies only rotz).
        boss1_rot_offset_pos(g, child, &me, offx, offy, offz);
        g.objs.aliens[child as usize].rotz = me.rotz;
    }
}

/// C `boss1_fire_hmissile1` (strat_enemy.c:2183).
#[allow(clippy::too_many_arguments)]
fn boss1_fire_hmissile1(
    g: &mut Game,
    self_idx: u16,
    target: u16,
    offx: i16,
    offy: i16,
    offz: i16,
    pitch: u8,
    yaw: u8,
) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    // fire_weapon muzzle: offset rotated by the firer's full rots (flags 1,1,1).
    boss1_rot_offset_pos(g, shot, &me, offx, offy, offz);
    let s_tick = sid(g, hmissile1_strat);
    let s_coll = sid(g, projectile_on_collide_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 2;
    al.ap = HMISSILE1_AP;
    al.vel = HMISSILE1_SPEED;
    al.count = HMISSILE1_LIFE;
    al.snd2 = 2;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.sflags |= ASF_SHADOW;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    gen_vecs_3d(al);
    Some(shot)
}

/// C `boss1_fire_hplasma` (strat_enemy.c:2218).
#[allow(clippy::too_many_arguments)]
fn boss1_fire_hplasma(
    g: &mut Game,
    self_idx: u16,
    target: u16,
    offx: i16,
    offy: i16,
    offz: i16,
    pitch: u8,
    yaw: u8,
) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    // fire_weapon muzzle: offset rotated by the firer's full rots (flags 1,1,1).
    boss1_rot_offset_pos(g, shot, &me, offx, offy, offz);
    let s_tick = sid(g, homingflat_strat);
    let s_coll = sid(g, projectile_on_collide_strat);
    let s_exp = sid(g, hmissile1_remove_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = 1;
    al.ap = HPLASMA_AP;
    al.vel = HPLASMA_SPEED;
    al.count = HPLASMA_LIFE;
    al.snd2 = 6;
    al.type_ = ATLASER | ATZREMOVE;
    al.sflags |= ASF_INVISIBLE;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    al.sbyte1 = yaw;
    al.sbyte2 = pitch;
    al.rotx = 0;
    al.roty = DEG180;
    Some(shot)
}

/// C `boss1_fire_relslowlaser` (strat_enemy.c:2256).
fn boss1_fire_relslowlaser(g: &mut Game, self_idx: u16, target: Option<u16>, homing: bool) {
    let me = g.objs.aliens[self_idx as usize];
    let mut pitch = me.rotx;
    let mut yaw = me.roty;
    if homing {
        if let Some(t) = target.map(|t| g.objs.aliens[t as usize]).filter(|t| t.active) {
            yaw = angle_xz(&me, &t);
            pitch = strat_pitch_toward(&me, &t);
        }
    }
    let speed = strat_relslowelaser_speed(g);
    let Some(shot) = spawn_projectile(
        g,
        Some(self_idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        RELSLOWELASERHOME_LIFE,
        RELSLOWELASERHOME_AP,
        ACF_COLLTYPE4_BIT,
    ) else {
        return;
    };
    let me = g.objs.aliens[self_idx as usize];
    // Turret muzzle `#0,#0,#40>>weapon_scale` = z+40 world after fire_weapon's
    // <<weapon_scale(2) (GBSTRATS.ASM:374), rotated by the firer's full rots.
    boss1_rot_offset_pos(g, shot, &me, 0, 0, 40);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = pitch;
        al.roty = yaw;
        al.rotz = me.rotz;
        gen_vecs_3d(al);
    }
    if homing {
        let s_home = sid(g, relelaserhome_strat);
        let al = &mut g.objs.aliens[shot as usize];
        al.stratptr = Some(s_home);
        al.sbyte1 = pitch;
        al.sbyte2 = yaw;
        al.animframe = 0;
    }
}

/// C `boss1_finish` (strat_enemy.c:2296).
/// C `s_add_bossHP x,al_hp` (STRATLIB.INC:562): `m_bossHP += al_hp`. The
/// accumulator is zeroed each frame in `init_strats`, so every living boss
/// part re-adds its current HP; the HUD bar = m_bossHP / m_bossmaxHP.
fn add_bosshp(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(hp);
}

fn boss1_finish(g: &mut Game, self_idx: u16, allow_center_fire: bool) {
    let left_alive = boss1_child_bank_alive(g, self_idx, BOSS1_CHILD_TL0, BOSS1_CHILD_TL3);
    let right_alive = boss1_child_bank_alive(g, self_idx, BOSS1_CHILD_TR0, BOSS1_CHILD_TR3);
    if g.objs.aliens[self_idx as usize].sflags4 & BOSS1_PARENT_FLAG_COVER_BLOCK != 0 {
        let al = &mut g.objs.aliens[self_idx as usize];
        al.rotz = al.rotz.wrapping_add(DEG90 / 32);
    }
    // boss1_end .fire: `s_jmp_NOTdelay 6,.nofire,#15` = fire when (gf+15)&63==0
    // (GBSTRATS.ASM:248). Muzzles at `#(±96<<2)>>weapon_scale` = ±384 world
    // (GBSTRATS.ASM:251/256), each shot given the extra enemy1 colltype
    // (`s_set_colltype y,enemy1`, GBSTRATS.ASM:253/258 — shootable center pair).
    if allow_center_fire
        && currentlevel(g) != 1
        && (!left_alive || !right_alive)
        && (g.vars.gameframe.wrapping_add(15)) & 63 == 0
    {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[self_idx as usize];
            let yaw = angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            if let Some(shot) = boss1_fire_hmissile1(g, self_idx, 0, -384, 0, 0, pitch, yaw) {
                g.objs.aliens[shot as usize].collflags |= COLLTYPE_ENEMY1;
            }
            if let Some(shot) = boss1_fire_hmissile1(g, self_idx, 0, 384, 0, 0, pitch, yaw) {
                g.objs.aliens[shot as usize].collflags |= COLLTYPE_ENEMY1;
            }
        }
    }
    boss1_update_child_positions(g, self_idx);
    if boss1_live_turret_count(g, self_idx) == 0 {
        boss1back_init(g, self_idx);
    }
    add_player_z(g, self_idx);
    // boss1_fin: s_add_bossHP x,al_hp (GBSTRATS.ASM:274) — every mother
    // mode (up/normal/in/out/inclose/back) ends here.
    add_bosshp(g, self_idx);
}

/// C `boss1normal_init` (strat_enemy.c:2333).
fn boss1normal_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1normal_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = 30;
    al.sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

/// C `boss1in_init` (strat_enemy.c:2342).
fn boss1in_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1in_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

/// C `boss1out_init` (strat_enemy.c:2350).
fn boss1out_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1out_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `boss1inclose_init` (strat_enemy.c:2357).
fn boss1inclose_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1inclose_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte3 = 2;
    al.stratptr = Some(s);
    al.sflags2 &= !BOSS1_PARENT_FLAG_TURRETS_OPEN;
}

/// C `boss1back_init` (strat_enemy.c:2366).
fn boss1back_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1back_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `boss1up_strat` (strat_enemy.c:2373).
fn boss1up_strat(g: &mut Game, idx: u16) {
    // s_jmp_higher x,#space_viewCY (GBSTRATS.ASM:138) branches STRICTLY when
    // worldy < space_viewCY (rlbmi, STRATMAC.INC:3081) — passes through ==CY.
    if g.objs.aliens[idx as usize].worldy < BOSS1_SPACE_VIEW_CY {
        boss1normal_init(g, idx);
        boss1normal_strat(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(10);
    }
    boss1_finish(g, idx, true);
}

/// C `boss1normal_strat` (strat_enemy.c:2388).
fn boss1normal_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 != 0 {
        boss1_finish(g, idx, true);
        return;
    }
    boss1in_init(g, idx);
    boss1in_strat(g, idx);
}

/// C `boss1in_strat` (strat_enemy.c:2405).
fn boss1in_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(15);
    }
    // s_jmp_Zdistmore x,y,#1000,boss1_end (GBSTRATS.ASM:159) holds INCLUSIVELY:
    // |dz| >= 1000 -> boss1_end (keep advancing in); advance to out only when < 1000.
    let me = g.objs.aliens[idx as usize];
    let close = player(g)
        .map(|p| ((me.worldz as i32 - p.worldz as i32).abs() as i16) < 1000)
        .unwrap_or(false);
    if !close {
        boss1_finish(g, idx, true);
        return;
    }
    boss1out_init(g, idx);
    boss1out_strat(g, idx);
}

/// C `boss1out_strat` (strat_enemy.c:2424).
fn boss1out_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(15);
    }
    let me = g.objs.aliens[idx as usize];
    let far = player(g)
        .map(|p| ((me.worldz as i32 - p.worldz as i32).abs() as i16) >= BOSS1_MISSILE_ZDIST)
        .unwrap_or(false);
    if !far {
        boss1_finish(g, idx, true);
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= BOSS1_PARENT_FLAG_TURRETS_OPEN;
    // s_beqdec_alvar B,x,al_sbyte3,boss1inclose_init (GBSTRATS.ASM:170) tests
    // BEFORE decrementing: sbyte3==0 -> inclose (no dec); else dec and go normal.
    // Init sbyte3=1 so the first out-pass decs to 0 -> normal; inclose the NEXT pass.
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        boss1inclose_init(g, idx);
        boss1inclose_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte3 -= 1;
    boss1normal_init(g, idx);
    boss1normal_strat(g, idx);
}

/// C `boss1inclose_strat` (strat_enemy.c:2453).
fn boss1inclose_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(25);
    }
    // s_jmp_Zdistmore x,y,#300,boss1_end.nofire (GBSTRATS.ASM:182) is inclusive:
    // hold at |dz| >= 300, advance to out only when < 300.
    let me = g.objs.aliens[idx as usize];
    let close = player(g)
        .map(|p| ((me.worldz as i32 - p.worldz as i32).abs() as i16) < BOSS1_CLOSE_ZDIST)
        .unwrap_or(false);
    if !close {
        boss1_finish(g, idx, false);
        return;
    }
    boss1out_init(g, idx);
    boss1out_strat(g, idx);
}

/// C `boss1back_strat` (strat_enemy.c:2472).
fn boss1back_strat(g: &mut Game, idx: u16) {
    // s_jmp_Zdistmore x,y,#1500,.nzi (GBSTRATS.ASM:192) is inclusive: |dz| >= 1500
    // -> .nzi (release cover, spin, bombard); |dz| < 1500 -> retreat (worldz += 15)
    // then the full boss1_end (finding #2 — the ROM branch was inverted in the port).
    let me = g.objs.aliens[idx as usize];
    let pl = player(g);
    let far = pl
        .map(|p| ((me.worldz as i32 - p.worldz as i32).abs() as i16) >= BOSS1_MISSILE_ZDIST)
        .unwrap_or(false);
    if !far {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_add(15);
        }
        boss1_finish(g, idx, true);
        return;
    }
    if g.objs.aliens[idx as usize].sflags4 & BOSS1_PARENT_FLAG_COVER_GONE == 0 {
        if let Some(cover) = boss1_cover_obj(g, idx) {
            let s_die = sid(g, boss1covdie_strat);
            {
                let al = &mut g.objs.aliens[cover as usize];
                al.stratptr = Some(s_die);
                al.collstratptr = None;
            }
            boss_clear_child_link(g, cover);
            boss_prune_family_links(g, idx);
            g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
            g.hooks.play_se(0x85);
        }
        g.objs.aliens[idx as usize].sflags4 |= BOSS1_PARENT_FLAG_COVER_GONE;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = al.colframe.wrapping_add(1) & 3;
        al.rotz = al.rotz.wrapping_add(DEG90 / 32);
    }
    // HPLASMA barrage: `s_jmp_notdelay 6,.nofire1` (GBSTRATS.ASM:210) = fire when
    // gf&63==0 (finding #1). `s_weapon_rndrot 15,15` = per-axis (rnd&15)-7 spread on
    // the FIRER's rots, PITCH drawn first (STRATMAC.INC:2099-2108, finding #6) — no
    // aim at player; the shot homes via its al_ptr=player target afterward.
    if g.vars.gameframe & 63 == 0 {
        let me = g.objs.aliens[idx as usize];
        let pitch = me.rotx.wrapping_add(((ea_random(g) & 15) as i16 - 7) as u8);
        let yaw = me.roty.wrapping_add(((ea_random(g) & 15) as i16 - 7) as u8);
        let _ = boss1_fire_hplasma(g, idx, 0, 0, 0, 0, pitch, yaw);
    }
    // HMISSILE1 pair: `s_jmp_NOTdelay 6,.nofire,#15` (GBSTRATS.ASM:217) = fire when
    // (gf+15)&63==0 (finding #1). `s_weapon_rot #±(deg45-deg11),#0` = PITCH offset
    // ±24 on the firer's rots, yaw stays the firer's deg180 (finding #7); homes at
    // the player afterward.
    if (g.vars.gameframe.wrapping_add(15)) & 63 == 0 {
        let me = g.objs.aliens[idx as usize];
        let _ = boss1_fire_hmissile1(
            g,
            idx,
            0,
            0,
            0,
            0,
            me.rotx.wrapping_add(DEG45 - DEG11),
            me.roty,
        );
        if currentlevel(g) != 1 {
            let me = g.objs.aliens[idx as usize];
            let _ = boss1_fire_hmissile1(
                g,
                idx,
                0,
                0,
                0,
                0,
                me.rotx.wrapping_sub(DEG45 - DEG11),
                me.roty,
            );
        }
    }
    // .nofire -> boss1_fin (GBSTRATS.ASM:227): straight to s_add_playerZ. NO second
    // rotz spin, no boss1rots child recount, no center-fire (finding #21).
    add_player_z(g, idx);
}

/// C `boss1cov_coll` (strat_enemy.c:2523).
fn boss1cov_coll(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.hitflags = 0;
    al.sflags &= !ASF_COLLIDE;
}

/// C `boss1cov_init` (strat_enemy.c:2531).
fn boss1cov_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss1cov_strat);
    let s_coll = sid(g, boss1cov_coll);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = BOSS1_COVER_AP;
    al.roty = DEG180;
    al.sbyte2 = 33;
    al.sbyte3 = 10;
    al.sbyte4 = ((32 * 4) / 2 - 12) as u8;
    al.collflags |= COLLTYPE_ENEMY1;
    al.type_ |= ATGND;
}

/// C `boss1cov_strat` (strat_enemy.c:2549).
fn boss1cov_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let bf = bossflags(g);
    set_bossflags(g, bf | BF_FLAG1);
    g.objs.aliens[mother as usize].sflags4 &= !BOSS1_PARENT_FLAG_COVER_BLOCK;
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 != 0 {
        g.objs.aliens[mother as usize].sflags4 |= BOSS1_PARENT_FLAG_COVER_BLOCK;
        let side_right =
            g.objs.aliens[mother as usize].sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT != 0;
        let al = &mut g.objs.aliens[idx as usize];
        if side_right {
            al.sbyte4 = al.sbyte4.wrapping_add(4);
        } else {
            al.sbyte4 = al.sbyte4.wrapping_sub(4);
        }
    } else {
        g.objs.aliens[idx as usize].sbyte2 = 1;
        {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte3 > 0 {
                al.sbyte3 -= 1;
            }
        }
        if g.objs.aliens[idx as usize].sbyte3 == 0 {
            g.hooks.play_se(0x2F);
            let clear_frames = boss1_cover_clear_frames(g);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte2 = BOSS1_COVER_BLOCK_FRAMES;
                al.sbyte3 = clear_frames;
            }
            g.objs.aliens[mother as usize].sflags2 ^= BOSS1_PARENT_FLAG_SIDE_RIGHT;
            g.objs.aliens[mother as usize].sflags4 |= BOSS1_PARENT_FLAG_COVER_BLOCK;
            let side_right =
                g.objs.aliens[mother as usize].sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT != 0;
            let al = &mut g.objs.aliens[idx as usize];
            if side_right {
                al.sbyte4 = al.sbyte4.wrapping_add(4);
            } else {
                al.sbyte4 = al.sbyte4.wrapping_sub(4);
            }
        }
    }
    boss1_update_child_positions(g, mother);
}

/// C `boss1covdie_strat` (strat_enemy.c:2598).
fn boss1covdie_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        // s_jmp_Zdistmore x,y,#1000,remove_Istrat (GBSTRATS.ASM:461) is inclusive:
        // remove when behind the player AND |dz| >= 1000 (finding #19).
        let me = g.objs.aliens[idx as usize];
        if me.worldz < pl.worldz
            && ((me.worldz as i32 - pl.worldz as i32).abs() as i16) >= 1000
        {
            g.objs.aldead = 1;
            return;
        }
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(20);
}

/// C `boss1turcol_coll` (strat_enemy.c:2616).
fn boss1turcol_coll(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags = 0;
        al.sflags &= !ASF_COLLIDE;
        return;
    }
    strat_hit_flash(g, idx);
}

/// C `boss1turret_init_common` (strat_enemy.c:2628).
fn boss1turret_init_common(g: &mut Game, idx: u16, strat: StrategyFn) {
    let s = sid(g, strat);
    let s_coll = sid(g, boss1turcol_coll);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = BOSS1_TURRET_HP;
    al.ap = BOSS1_TURRET_AP;
    al.roty = DEG180;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
    al.sflags |= ASF_NOHITAFFECT;
    al.type_ |= ATGND;
    al.colframe = 0;
    g.vars.bossmaxhp = g.vars.bossmaxhp.wrapping_add(BOSS1_TURRET_HP as u16);
}

/// C `boss1turretL_init` (strat_enemy.c:2646).
fn boss1turret_l_init(g: &mut Game, idx: u16) {
    boss1turret_init_common(g, idx, boss1turret_l_strat);
}

/// C `boss1turretR_init` (strat_enemy.c:2650).
fn boss1turret_r_init(g: &mut Game, idx: u16) {
    boss1turret_init_common(g, idx, boss1turret_r_strat);
}

/// C `boss1turret_common_strat` (strat_enemy.c:2654).
fn boss1turret_common_strat(g: &mut Game, idx: u16, right_side: bool) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    // boss1turret_end: s_add_bossHP x,al_hp (GBSTRATS.ASM:400) — reached by
    // EVERY turret path (nfire, cover-hold, fire). al_hp is constant across
    // the tick, so accumulate once up front to cover all exits.
    add_bosshp(g, idx);
    let m = g.objs.aliens[mother as usize];
    let side_matches = m.sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT != 0;
    if side_matches != right_side
        || m.sflags2 & BOSS1_PARENT_FLAG_TURRETS_OPEN == 0
        || m.sflags4 & BOSS1_PARENT_FLAG_COVER_BLOCK != 0
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = 0;
        al.sflags |= ASF_NOHITAFFECT;
        boss1_update_child_positions(g, mother);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = al.colframe.wrapping_add(1) & 3;
        al.sflags &= !ASF_NOHITAFFECT;
    }
    let cover = boss1_cover_obj(g, mother);
    if let Some(cover) = cover {
        if g.objs.aliens[cover as usize].sbyte3 >= 20 {
            boss1_update_child_positions(g, mother);
            return;
        }
    }
    // Fire gates are bit-count delays with a per-turret al1pt stagger (== object
    // index): home `s_jmp_IFdelay 4,.home,al1pt` = (gf+idx)&15==0; normal
    // `s_jmp_IFdelay 5,.norm,al1pt` = (gf+idx)&31==0 (GBSTRATS.ASM:376/378,
    // finding #1). bf_flag1-set + home-frame consumes the flag and fires homing;
    // otherwise it falls through to the normal gate exactly as the ASM does.
    let phase = strat_phase_offset(idx) as u16;
    let pl_idx = if g.objs.aliens[0].active { Some(0u16) } else { None };
    if bossflags(g) & BF_FLAG1 != 0
        && pl_idx.is_some()
        && (g.vars.gameframe.wrapping_add(phase)) & 15 == 0
    {
        let bf = bossflags(g);
        set_bossflags(g, bf & !BF_FLAG1);
        boss1_fire_relslowlaser(g, idx, pl_idx, true);
    } else if (g.vars.gameframe.wrapping_add(phase)) & 31 == 0 {
        boss1_fire_relslowlaser(g, idx, pl_idx, false);
    }
    boss1_update_child_positions(g, mother);
}

/// C `boss1turretL_strat` (strat_enemy.c:2702).
fn boss1turret_l_strat(g: &mut Game, idx: u16) {
    boss1turret_common_strat(g, idx, false);
}

/// C `boss1turretR_strat` (strat_enemy.c:2706).
fn boss1turret_r_strat(g: &mut Game, idx: u16) {
    boss1turret_common_strat(g, idx, true);
}

/// boss1's death entry point (ROM expstratptr = `bossexplode_Istrat`, wired at
/// GBSTRATS.ASM:96). ROM plays `s_boss_dying` ($1e + boss-dying bgm + PSTF_NOTDIE
/// + fire-off), scatters the staggered SML/MED/L explosion barrage + a
/// circdelayexplode child, sets self lifecnt 38, then runs `bossdelayexplode`
/// (finding #9). The rotz spin is carried by the pushed `boss1exp_Istrat`
/// (s_push_stratptr, GBSTRATS.ASM:92-94); bossdelayexplode_strat runs it each
/// countdown tick via the tempstrat slot (EXPSTRAT.ASM:60-64).
fn boss1exp_init(g: &mut Game, idx: u16) {
    boss1_release_children(g, idx);
    let spin = sid(g, boss1exp_spin_strat);
    g.objs.aliens[idx as usize].tempstratptr = Some(spin);
    strat_boss_explode_init(g, idx);
}

/// C `boss1exp_Istrat` (GBSTRATS.ASM:87-90): the pushed death-animation tick —
/// just spins rotz at deg90/32. Driven by bossdelayexplode_strat's tempstrat
/// call each frame (which already does the add_playerZ).
fn boss1exp_spin_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = al.rotz.wrapping_add(DEG90 / 32);
}

/// C `Strat_Boss1_Init` (strat_enemy.c:2744).
pub fn strat_boss1_init(g: &mut Game, idx: u16) {
    let hp = if currentlevel(g) == 1 {
        BOSS1_HP / 2
    } else {
        BOSS1_HP
    };
    let s = sid(g, boss1up_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, boss1exp_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = hp;
        al.ap = BOSS1_AP;
        al.roty = DEG180;
        al.collflags |= COLLTYPE_ENEMY1;
        al.type_ |= ATGND;
        al.colframe = 0;
        al.sflags |= ASF_SHADOW | ASF_COLLDISABLE;
        al.sbyte3 = 1;
        al.sflags2 &= !(BOSS1_PARENT_FLAG_TURRETS_OPEN | BOSS1_PARENT_FLAG_SIDE_RIGHT);
        al.sflags4 &= !(BOSS1_PARENT_FLAG_COVER_BLOCK | BOSS1_PARENT_FLAG_COVER_GONE);
    }
    g.vars.gameflags &= !GF_BOSSDEAD;
    let bf = bossflags(g);
    set_bossflags(g, bf & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
    g.vars.bossmaxhp = hp as u16;
    g.vars.meters = 1;
    // Spawn cover last (C comment: it must tick before the turrets in the
    // head-inserted active list to drive same-frame fire gating).
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL0, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL1, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL2, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL3, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR0, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR1, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR2, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR3, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_COVER, boss1cov_init);
    boss1_update_child_positions(g, idx);
    g.hooks.play_se(0x82);
}

// ============================================================
// TOW_0 EXPLODE (C strat_enemy.c:3543-3575)
// ============================================================

/// C `tow0explode_wait` (strat_enemy.c:3543).
fn tow0explode_wait(g: &mut Game, idx: u16) {
    if count_down(&mut g.objs.aliens[idx as usize]) {
        g.objs.aldead = 1;
    }
}

/// C `Strat_Tow0Explode` (strat_enemy.c:3552).
pub fn strat_tow0_explode(g: &mut Game, idx: u16) {
    // tow_0 flags its linked tow_1 child via `ptr` (C comment: mirrors the
    // path WHENDEAD script until generic handling is ported).
    let ptr = g.objs.aliens[idx as usize].ptr;
    if ptr != 0 && (ptr as usize) < NUMBER_AL && g.objs.aliens[ptr as usize].active {
        g.objs.aliens[ptr as usize].sflags4 |= ASF4_SFLAG8;
    }
    g.hooks.play_se(0x10);
    let s = sid(g, tow0explode_wait);
    let al = &mut g.objs.aliens[idx as usize];
    al.flags |= AFEXP;
    al.sflags |= ASF_COLLDISABLE;
    al.collflags = 0;
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.count = 7;
}

// ============================================================
// ZACO2 (C strat_enemy.c:3585-3735) — boss7 hatch fighter.
// ============================================================

/// C `zaco2_reset_main` (strat_enemy.c:3585).
fn zaco2_reset_main(g: &mut Game, idx: u16) {
    let s = sid(g, zaco2_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
}

/// C `zaco2_Istrat` (strat_enemy.c:3595).
pub(crate) fn zaco2_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = ZACO2_HP;
        al.ap = ZACO2_AP;
        al.vel = 50;
        al.sbyte1 = 15;
        al.sbyte2 = 3;
        al.collflags |= COLLTYPE_ENEMYWEAP | COLLTYPE_ENEMY1 | COLLTYPE_ZENEMY;
        al.type_ &= !ATZREMOVE;
        al.snd2 = 0x0F;
        gen_vecs_3d(al);
    }
    zaco2_reset_main(g, idx);
}

/// C `zaco2_strat` (strat_enemy.c:3612).
fn zaco2_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 != 0 {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        zaco2_cont(g, idx);
        return;
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - pl.worldz as i32).abs() < 500 {
            zaco2loop_init(g, idx);
            return;
        }
        strat_aim_3d(g, idx, &pl, 3);
    }
    zaco2_cont(g, idx);
}

/// C `zaco2_cont` (strat_enemy.c:3637).
fn zaco2_cont(g: &mut Game, idx: u16) {
    if bossflags(g) & BF_DYING != 0 {
        g.objs.aldead = 1;
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
        // ASM GASTRATS.ASM:1097 `s_jmp_higher x,#0,.ngnd` skips the clamp when
        // worldy < 0; the ground bounce runs when worldy >= 0. (Audit A #5)
        if al.worldy >= 0 {
            al.worldy = 0;
            al.rotx = (al.rotx as i8).wrapping_neg() as u8;
        }
    }
    add_player_z(g, idx);
}

/// C `zaco2loop_init` (strat_enemy.c:3656).
fn zaco2loop_init(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        zaco2dash_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    let s = sid(g, zaco2loop_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte1 = DEG180 / 4;
    }
    if currentlevel(g) != 1 {
        // ASM GASTRATS.ASM:1107-1114 fires HMISSILE1 unconditionally on level!=1
        // (target = player); there is no aliens[0].active guard. (Audit A Minor 8)
        let _ = boss7launcher_fire_hmissile1(g, idx, 0);
    }
    zaco2loop_strat(g, idx);
}

/// C `zaco2loop_strat` (strat_enemy.c:3681).
fn zaco2loop_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 -= 1;
        // ASM GASTRATS.ASM:1120-1127: `s_jmp_rightofview x,.tright` branches to
        // `.tright` when leftpl is CLEAR (STRATMAC.INC:6176). `.tright` (leftpl
        // clear) adds +10/+4; the fall-through (leftpl SET) subtracts -10/-4.
        // (Audit A #11)
        if al.flags & AF_LEFT_PL != 0 {
            al.rotz = al.rotz.wrapping_sub(10);
            al.roty = al.roty.wrapping_sub(4);
        } else {
            al.rotz = al.rotz.wrapping_add(10);
            al.roty = al.roty.wrapping_add(4);
        }
        zaco2_cont(g, idx);
        return;
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        // ASM GASTRATS.ASM:1132 `s_jmp_Zdistmore x,y,#2000,zaco2_init` resets
        // when |dz| >= 2000 (inclusive). (Audit A Minor 3)
        if (me.worldz as i32 - pl.worldz as i32).abs() >= 2000 {
            zaco2_reset_main(g, idx);
            zaco2_strat(g, idx);
            return;
        }
    }
    zaco2_cont(g, idx);
}

/// C `zaco2dash_init` (strat_enemy.c:3712).
fn zaco2dash_init(g: &mut Game, idx: u16) {
    let s = sid(g, zaco2dash_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.count = 30;
    }
    zaco2dash_strat(g, idx);
}

/// C `zaco2dash_strat` (strat_enemy.c:3721).
fn zaco2dash_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            al.sflags |= ASF_COLLDISABLE;
            al.hp = 0;
        }
    }
    zaco2_cont(g, idx);
}

// ============================================================
// WORMS (GASTRATS.ASM; C strat_enemy.c:3749-3953)
// ============================================================

/// C `worm_kill` (strat_enemy.c:3749).
fn worm_kill(g: &mut Game) {
    g.objs.aldead = 1;
}

/// C `worm_common_init` (strat_enemy.c:3756).
fn worm_common_init(g: &mut Game, idx: u16, expstrat: StrategyFn) {
    let s = sid(g, worm_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, expstrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.roty = DEG180;
    al.hp = WORM_HP;
    al.ap = WORM_AP;
    al.collflags |= COLLTYPE_ENEMY1;
    al.vz = -10;
}

/// C `Strat_Wormhead_Init` (strat_enemy.c:3771).
pub fn strat_wormhead_init(g: &mut Game, idx: u16) {
    let gf = gasflags(g);
    set_gasflags(g, gf & !(GASF_KILLTYPE1 | GASF_KILLTYPE2));
    g.objs.aliens[idx as usize].snd2 = 5;
    worm_common_init(g, idx, wormheadexp_strat);
}

/// C `Strat_Worm_Init` (strat_enemy.c:3781).
pub fn strat_worm_init(g: &mut Game, idx: u16) {
    worm_common_init(g, idx, wormexp_strat);
}

/// C `worm_strat` (strat_enemy.c:3785).
fn worm_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = strat_tab_scaled(al.sbyte2, true, -4);
        al.vy = strat_tab_scaled(al.sbyte2, false, -4);
        al.sbyte2 = al.sbyte2.wrapping_add(4);
    }
    let link = strat_obj_from_ptr(g.objs.aliens[idx as usize].sword1 as u16)
        .map(|l| g.objs.aliens[l as usize]);
    let link_dead_hp0 = matches!(&link, Some(l) if l.active && l.hp == 0);
    if !link_dead_hp0 {
        if gasflags(g) & GASF_KILLTYPE2 != 0 {
            wormsplit_init(g, idx);
            return;
        }
    } else if gasflags(g) & GASF_KILLTYPE1 != 0 {
        worm_kill(g);
        return;
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// C `wormexp_strat` (strat_enemy.c:3811).
fn wormexp_strat(g: &mut Game, idx: u16) {
    let gf = gasflags(g);
    if gf & GASF_KILLTYPE1 == 0 {
        set_gasflags(g, gf | GASF_KILLTYPE2);
    }
    strat_explode(g, idx);
}

/// C `wormheadexp_strat` (strat_enemy.c:3818).
fn wormheadexp_strat(g: &mut Game, idx: u16) {
    let gf = gasflags(g);
    set_gasflags(g, gf | GASF_KILLTYPE1);
    strat_explode(g, idx);
}

/// C `wormsplit_init` (strat_enemy.c:3823).
fn wormsplit_init(g: &mut Game, idx: u16) {
    let r1 = ((ea_random(g) & 63) as i16) - 32;
    let r2 = ((ea_random(g) & 63) as i16) - 32;
    let s = sid(g, wormsplit_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.vx = r1;
    al.vy = r2;
    al.vz = 0;
    al.sbyte2 = 18;
    al.stratptr = Some(s);
}

/// C `wormsplit_strat` (strat_enemy.c:3835).
fn wormsplit_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        wormgo_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// C `wormgo_init` (strat_enemy.c:3850).
fn wormgo_init(g: &mut Game, idx: u16) {
    let s = sid(g, wormgo_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.vx = 0;
    al.vy = 0;
    al.vz = -10;
}

/// C `wormgo_strat` (strat_enemy.c:3861).
fn wormgo_strat(g: &mut Game, idx: u16) {
    {
        // ASM wormgo_strat (GASTRATS.ASM:2243-2246): `s_leftview_strat x,.gl`
        // branches to `.gl` (vx+=1) when leftpl is SET; else vx-=1. (Audit A #12)
        let al = &mut g.objs.aliens[idx as usize];
        if al.flags & AF_LEFT_PL != 0 {
            al.vx = al.vx.wrapping_add(1);
        } else {
            al.vx = al.vx.wrapping_sub(1);
        }
        apply_velocity(al);
    }
}

/// C `Strat_Worm2_Init` (strat_enemy.c:3875).
pub fn strat_worm2_init(g: &mut Game, idx: u16) {
    let s = sid(g, worm2_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let rnd = ea_random(g) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = WORM2_HP;
    al.ap = WORM2_AP;
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratstate = 0;
    al.sbyte3 = 10;
    al.rotz = rnd;
    al.count = 120;
    al.snd2 = 5;
    al.type_ &= !ATZREMOVE;
}

/// C `worm2_strat` (strat_enemy.c:3894).
fn worm2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(3);
        if al.sbyte3 > 0 {
            al.sbyte3 -= 1;
        }
        if al.sbyte3 == 0 {
            al.sbyte3 = 32;
            al.stratstate = al.stratstate.wrapping_add(1);
            if al.stratstate > 4 {
                al.stratstate = 1;
            }
        }
        match al.stratstate {
            1 => {
                al.vx = strat_tab_scaled(al.sbyte1, true, -3);
                al.vy = strat_tab_scaled(al.sbyte1, false, -3);
                al.vz = 0;
                al.sbyte1 = al.sbyte1.wrapping_add(4);
            }
            2 => {
                al.vx = strat_tab_scaled(al.sbyte1, true, -3);
                al.vy = strat_tab_scaled(al.sbyte1, false, -3);
                al.vz = strat_tab_scaled(al.sbyte1, false, 0);
                al.sbyte1 = al.sbyte1.wrapping_add(4);
            }
            3 => {
                al.vx = strat_tab_scaled(al.sbyte1, true, -3);
                al.vy = strat_tab_scaled(al.sbyte1, false, -3);
                al.vz = strat_tab_scaled(al.sbyte1, false, 1);
                al.sbyte1 = al.sbyte1.wrapping_add(4);
            }
            4 => {
                al.vx = strat_tab_scaled(al.sbyte1, true, -3);
                al.vy = strat_tab_scaled(al.sbyte1, false, -3);
                al.vz = strat_tab_scaled(al.sbyte1, false, 2);
                al.sbyte1 = al.sbyte1.wrapping_add(4);
            }
            _ => {}
        }
    }
    if g.vars.gameframe & 1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// ITEMS 5/7 + FLASHPLAYER (C strat_enemy.c:3955-4176)
// ============================================================

/// C `item5_init` (strat_enemy.c:3955).
fn item5_init(g: &mut Game, idx: u16) {
    let s = sid(g, item5_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

/// C `Strat_Item5_Init` (strat_enemy.c:3966).
pub fn strat_item5_init(g: &mut Game, idx: u16) {
    item5_init(g, idx);
    item5_strat(g, idx);
}

/// C `item5_collect` (strat_enemy.c:3975).
fn item5_collect(g: &mut Game, idx: u16) {
    let cnt = g.vars.read_ext16(wm::SPECWEPCNT);
    if cnt < ITEM5_MAX_SPEC {
        g.vars.write_ext16(wm::SPECWEPCNT, cnt + 1);
        // ASM GASTRATS.ASM:2586 `s_set_var B,specflash,#30` inside the
        // specwepcnt<5 block. (Audit A #24)
        g.vars.write_ext8(wm::SPECFLASH, 30);
        g.hooks.play_se(0x18);
        let score = g.vars.read_ext16(wm::PLAYERSCORE);
        g.vars
            .write_ext16(wm::PLAYERSCORE, score.wrapping_add(ITEM5_SCORE));
    }
    flashplayer_istrat(g, idx);
}

/// C `item5_strat` (strat_enemy.c:3989).
fn item5_strat(g: &mut Game, idx: u16) {
    // ASM item5_strat (GASTRATS.ASM:2571) leads with `s_remove_ifplayerdead x`,
    // which removes on `pshipflags2 & psf2_playerHP0` (HP0), not object
    // existence. (Audit A #23)
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(20);
    }
    let me = g.objs.aliens[idx as usize];
    // ASM `s_jmp_Zdistmore x,y,#60*2` / `s_jmp_XYdistmore x,y,#30*2` skip when
    // |dz|>=120 / |dx|+|dy|>=60 — pickup requires strictly less. (Audit A Minor 3)
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist >= ITEM5_PICKUP_Z {
        return;
    }
    let mut xydist = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    xydist = xydist.wrapping_add((me.worldy as i32 - pl.worldy as i32).abs() as i16);
    if xydist >= ITEM5_PICKUP_XY {
        return;
    }
    item5_collect(g, idx);
}

/// C `itemtorange_srou` (strat_enemy.c:4022).
fn itemtorange_srou(g: &mut Game, idx: u16) {
    // ASM itemtorange_srou (GASTRATS.ASM:3159-3163): `s_jmp_lower x,svar_word1,
    // .iny` skips the add when worldy >= minpmoveY+50, so `worldy+=3` runs only
    // when worldy < minpmoveY+50. (Audit A #13)
    let min_y = g.vars.minpmove_y.wrapping_add(50);
    let al = &mut g.objs.aliens[idx as usize];
    if al.worldy < min_y {
        al.worldy = al.worldy.wrapping_add(3);
    }
}

/// C `item_repair_player_wings` (strat_enemy.c:4035).
fn item_repair_player_wings(g: &mut Game) {
    g.vars.pshipflags &= !(PSF_BRKLWING | PSF_LWINGCOLL | PSF_BRKRWING | PSF_RWINGCOLL);
}

/// C `flashplayer_wire_shape` (strat_enemy.c:4040).
fn flashplayer_wire_shape(g: &Game) -> u16 {
    let wing_breaks = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING);
    if wing_breaks == 0 {
        SH_MY_W_PROXY
    } else if wing_breaks == PSF_BRKLWING {
        SH_MY_R_W_PROXY
    } else if wing_breaks == PSF_BRKRWING {
        SH_MY_L_W_PROXY
    } else {
        SH_MY_B_W_PROXY
    }
}

/// C `Strat_Item7_Init` (strat_enemy.c:4057).
pub fn strat_item7_init(g: &mut Game, idx: u16) {
    let s = sid(g, item7_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

/// C `flashplayer_Istrat` (strat_enemy.c:4068).
fn flashplayer_istrat(g: &mut Game, idx: u16) {
    if g.vars.splayerflymode == SPFM_INSIDE {
        g.objs.aldead = 1;
        return;
    }
    let s = sid(g, flashplayer_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 20;
    al.sflags |= ASF_COLLDISABLE;
    al.colframe = 0;
    al.stratptr = Some(s);
}

/// C `flashplayer_strat` (strat_enemy.c:4084).
fn flashplayer_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = pl.rotx;
        al.roty = pl.roty;
        al.rotz = pl.rotz;
        al.worldx = pl.worldx;
        al.worldy = pl.worldy;
        al.worldz = pl.worldz;
    }
    if g.vars.gameframe & 1 == 0 {
        g.objs.aliens[idx as usize].shape = 0;
    } else {
        let shape = flashplayer_wire_shape(g);
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = shape;
        al.colframe = al.colframe.wrapping_add(1) & 3;
    }
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

/// C `item7_strat` (strat_enemy.c:4119).
fn item7_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(20);
    }
    itemtorange_srou(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotz = al.rotz.wrapping_add(4);
    }
    let me = g.objs.aliens[idx as usize];
    // ASM item7 (GASTRATS.ASM) `s_jmp_Zdistmore`/`s_jmp_XYdistmore` skip on
    // |dz|>=120 / |dx|+|dy|>=60 — pickup strictly less. (Audit A Minor 3)
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist >= ITEM7_PICKUP_Z {
        return;
    }
    let mut xydist = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    xydist = xydist.wrapping_add((me.worldy as i32 - pl.worldy as i32).abs() as i16);
    if xydist >= ITEM7_PICKUP_XY {
        return;
    }
    let needs_repair = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING) != 0;
    item_repair_player_wings(g);
    if needs_repair {
        // C comment: ripair_Istrat effect applied inline.
        g.hooks.play_se(0x17);
        flashplayer_istrat(g, idx);
        return;
    }
    g.hooks.play_se(0x15);
    let score = g.vars.read_ext16(wm::PLAYERSCORE);
    g.vars
        .write_ext16(wm::PLAYERSCORE, score.wrapping_add(ITEM7_SCORE));
    if g.vars.pshipflags2 & PSF2_DOUBLASER == 0 {
        g.vars.pshipflags2 |= PSF2_DOUBLASER;
    } else {
        g.vars.pshipflags3 |= PSF3_BEAMBALL;
    }
    flashplayer_istrat(g, idx);
}

// ============================================================
// BOMWING (C strat_enemy.c:4178-4303)
// ============================================================

/// C `bomwing_move_scroll_only` (strat_enemy.c:4178).
fn bomwing_move_scroll_only(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(35);
}

/// C `bomwing_reset_phase1` (strat_enemy.c:4188).
fn bomwing_reset_phase1(g: &mut Game, idx: u16) {
    let s = sid(g, bomwing_phase1);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    if al.expstratptr.is_none() {
        al.expstratptr = Some(s_exp);
    }
    al.sbyte1 = 20;
}

/// C `bomwing_enter_phase2` (strat_enemy.c:4198).
fn bomwing_enter_phase2(g: &mut Game, idx: u16) {
    let s = sid(g, bomwing_phase2);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte1 = ((DEG360 / 3) / 2) as u8;
}

/// C `bomwing_fire` (strat_enemy.c:4206).
fn bomwing_fire(g: &mut Game, idx: u16, player_idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let shot = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        me.rotx.wrapping_sub(DEG22),
        me.roty,
        HPLASMA_SPEED,
        HPLASMA_LIFE,
        HPLASMA_AP,
        ACF_COLLTYPE4,
    );
    if let Some(shot) = shot {
        g.objs.aliens[shot as usize].ptr = strat_obj_index_or_null(player_idx);
    }
}

/// C `bomwing_phase1` (strat_enemy.c:4223).
fn bomwing_phase1(g: &mut Game, idx: u16) {
    // s_beqdec_alvar branches before the decrement (C comment).
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bomwing_enter_phase2(g, idx);
        bomwing_phase2(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 -= 1;
        gen_vecs_2d(al);
        apply_velocity(al);
    }
    bomwing_move_scroll_only(g, idx);
}

/// C `bomwing_phase2` (strat_enemy.c:4241).
fn bomwing_phase2(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        // ASM bomwing2_strat (GASTRATS.ASM:2540) `s_jmp_Zdistmore x,y,#3000,.nfire`
        // skips fire when |dz|>=3000 — fire strictly < 3000. (Audit A Minor 3)
        if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < 3000
            && !strat_points_positive_z(&me)
            && g.vars.gameframe & 7 == 0
        {
            bomwing_fire(g, idx, 0);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(2);
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bomwing_reset_phase1(g, idx);
        bomwing_phase1(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    bomwing_move_scroll_only(g, idx);
}

/// C `bomwing_die` (strat_enemy.c:4271).
fn bomwing_die(g: &mut Game, idx: u16) {
    if let Some(drop) = make_obj(g, SH_ITEM_5) {
        item5_init(g, drop);
        let me = g.objs.aliens[idx as usize];
        let al = &mut g.objs.aliens[drop as usize];
        al.worldx = me.worldx;
        al.worldy = me.worldy.wrapping_sub(20);
        al.worldz = me.worldz;
    }
    strat_explode(g, idx);
}

/// C `Strat_Bomwing_Init` (strat_enemy.c:4289).
pub fn strat_bomwing_init(g: &mut Game, idx: u16) {
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, bomwing_die);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = BOMWING_HP;
        al.ap = BOMWING_AP;
        al.vel = BOMWING_SPEED;
        al.roty = DEG45;
        al.collflags |= COLLTYPE_ENEMY1;
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
    }
    bomwing_reset_phase1(g, idx);
    bomwing_phase1(g, idx);
}

// ============================================================
// TADPOLE (GA2STRAT.ASM; C strat_enemy.c:4468-4572)
// ============================================================

/// C `tadpole_strat` (strat_enemy.c:4468).
fn tadpole_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sflags2 & TADPOLE_SIDE_FLAG != 0 {
                    let mut roty = al.roty;
                    achase_angle(&mut roty, DEG90, 4);
                    al.roty = roty;
                    al.rotx = al.rotx.wrapping_sub(2);
                } else {
                    let mut roty = al.roty;
                    achase_angle(&mut roty, DEG90.wrapping_neg(), 4);
                    al.roty = roty;
                    al.rotx = al.rotx.wrapping_add(2);
                }
                if al.sbyte1 == 0 {
                    al.stratstate += 1;
                } else {
                    al.sbyte1 -= 1;
                }
            }
        }
        1 => {
            g.objs.aliens[idx as usize].sbyte1 = TADPOLE_DIVE_FRAMES;
            if let Some(pl) = pl {
                strat_aim_3d(g, idx, &pl, 3);
                let me = g.objs.aliens[idx as usize];
                if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) <= TADPOLE_FIRE_ZDIST
                {
                    let (rx, ry) = (me.rotx, me.roty);
                    strat_fire_relslowlaserhome(g, idx, rx, ry);
                    g.objs.aliens[idx as usize].stratstate += 1;
                }
            }
        }
        2 => {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotz = al.rotz.wrapping_add(2);
            al.rotx = al.rotx.wrapping_add(2);
            if al.sbyte1 > 0 {
                al.sbyte1 -= 1;
            }
            if al.sbyte1 == 0 {
                al.stratstate += 1;
                al.sbyte1 = TADPOLE_BANK_FRAMES;
            }
        }
        3 => {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte1 == 0 {
                al.stratstate += 1;
            } else {
                al.sbyte1 -= 1;
                al.rotz = al.rotz.wrapping_add(8);
                al.rotx = al.rotx.wrapping_sub(4);
            }
        }
        _ => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = al.rotz.wrapping_add(8);
                al.rotx = al.rotx.wrapping_sub(1);
                if al.count > 0 {
                    al.count -= 1;
                    if al.count == 0 {
                        g.objs.aldead = 1;
                        return;
                    }
                }
                let _ = speed_to(al, TADPOLE_ESCAPE_SPEED, 1);
                al.roty = al.roty.wrapping_sub(2);
                if al.sflags2 & TADPOLE_SIDE_FLAG != 0 {
                    al.roty = al.roty.wrapping_add(4);
                }
            }
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// C `Strat_Tadpole_Init` (strat_enemy.c:4551).
pub fn strat_tadpole_init(g: &mut Game, idx: u16) {
    let s = sid(g, tadpole_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = TADPOLE_HP;
        al.ap = TADPOLE_AP;
        al.vel = TADPOLE_SPEED;
        al.count = TADPOLE_LIFE;
        al.collflags |= COLLTYPE_ENEMY1;
        al.stratstate = 0;
        al.sbyte1 = TADPOLE_SWIM_FRAMES;
        if al.worldx >= 0 {
            al.sflags2 |= TADPOLE_SIDE_FLAG;
        }
    }
    tadpole_strat(g, idx);
}

// ============================================================
// SPACEBAR WALKER / SHOOTER (GA2STRAT.ASM; C strat_enemy.c:4581-4675)
// ============================================================

/// C `spacebarshoot_apply_spacemist` (strat_enemy.c:4581, s_spacemist).
fn spacebarshoot_apply_spacemist(g: &mut Game, idx: u16) {
    let pvz = pviewposz(g);
    let al = &mut g.objs.aliens[idx as usize];
    let bucket = ((al.worldz as i32 - pvz as i32 + 500) >> 9) as i16;
    let mut frame = bucket as u8;
    if (frame.wrapping_sub(8) as i8) >= 0 {
        frame = 7;
    }
    al.colframe = 0x80 | frame;
}

/// C `spacebarwalker_strat` (strat_enemy.c:4598).
fn spacebarwalker_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        achase_angle(&mut roty, angle_xz(&me, &pl), 1);
        g.objs.aliens[idx as usize].roty = roty;
    }
    let me = g.objs.aliens[idx as usize];
    if pl.worldz >= me.worldz {
        return;
    }
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F != 0 {
        return;
    }
    let fire_pitch = strat_pitch_toward(&me, &pl);
    let fire_yaw = angle_xz(&me, &pl);
    let _ = spawn_projectile(
        g,
        Some(idx),
        0,
        -20,
        0,
        fire_pitch,
        fire_yaw,
        52,
        55,
        2,
        ACF_COLLTYPE4,
    );
}

/// C `Strat_Spacebarwalker_Init` (strat_enemy.c:4631).
pub fn strat_spacebarwalker_init(g: &mut Game, idx: u16) {
    let s = sid(g, spacebarwalker_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = 4;
        al.ap = 4;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    // ASM spacebarwalker_Istrat (GA2STRAT.ASM:1788) ends with `spacebarwalker_strat
    // s_start_strat` immediately after — the body runs on the spawn frame. (Audit A #37)
    spacebarwalker_strat(g, idx);
}

/// C `spacebarshoot_strat` (GA2STRAT.ASM:1818-1825).
fn spacebarshoot_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        al.worldx = al.worldx.wrapping_add(al.sword1);
        al.worldy = al.worldy.wrapping_add(al.sword2);
    }
    spacebarshoot_apply_spacemist(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
        if al.count == 0 {
            g.objs.aldead = 1;
        }
    }
}

/// C `Strat_Spacebarshoot_Init` (GA2STRAT.ASM:1809-1816).
pub fn strat_spacebarshoot_init(g: &mut Game, idx: u16) {
    let s = sid(g, spacebarshoot_strat);
    let s_coll = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = None;
        set_hard_vars(al);
        al.collflags |= COLLTYPE_ENEMY1;
        al.count = 80;
    }
    // ASM spacebarshoot_Istrat (GA2STRAT.ASM:1814) ends with `spacebarshoot_strat
    // s_start_strat` immediately after — the body runs on the spawn frame. (Audit A #37)
    spacebarshoot_strat(g, idx);
}

// ============================================================
// UP1MAN + ITEM0 (C strat_enemy.c:4682-4896)
// ============================================================

/// C `item0_Istrat` (strat_enemy.c:4682).
fn item0_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, item0_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.rotx = DEG90.wrapping_neg();
    }
    // ASM item0_Istrat (GASTRATS.ASM:2678) ends with `item0_strat s_start_strat`
    // immediately after — the body runs on the spawn frame. (Audit A #37)
    item0_strat(g, idx);
}

/// C `up1man_remove_child_slot` (strat_enemy.c:4694).
fn up1man_remove_child_slot(g: &mut Game, mother: u16, child_num: u8) {
    let Some(child) = boss_find_child_obj(g, mother, child_num) else {
        return;
    };
    boss_clear_child_link(g, child);
    boss_prune_family_links(g, mother);
    g.objs.free(child);
}

/// C `item0_strat` (strat_enemy.c:4711).
fn item0_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(UP1MAN_SCROLL_Z);
    }
    let me = g.objs.aliens[idx as usize];
    // ASM item0_strat (GASTRATS.ASM:2685-2687): `s_jmp_Zdistmore #80` /
    // `s_jmp_Ydistmore #120` / `s_jmp_Xdistmore #80` skip pickup when the axis
    // distance is >= the bound — pickup requires strictly less. (Audit A Minor 3)
    if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) >= UP1MAN_PICKUP_Z {
        return;
    }
    if ((me.worldy as i32 - pl.worldy as i32).abs() as i16) >= UP1MAN_PICKUP_Y {
        return;
    }
    if ((me.worldx as i32 - pl.worldx as i32).abs() as i16) >= UP1MAN_PICKUP_X {
        return;
    }
    g.hooks.play_se(0x0E);
    let lives = g.vars.read_ext8(wm::LIVES);
    g.vars.write_ext8(wm::LIVES, lives.wrapping_add(1));
    let mother = boss_child_from_index_raw(me.sword1 as u16);
    if let Some(m) = mother.filter(|&m| g.objs.aliens[m as usize].active) {
        up1man_remove_child_slot(g, m, 1);
        up1man_remove_child_slot(g, m, 3);
        up1man_remove_child_slot(g, m, 4);
    }
    g.objs.aldead = 1;
}

/// C `up1man_spawn_child` (strat_enemy.c:4750).
fn up1man_spawn_child(
    g: &mut Game,
    mother: u16,
    child_num: u8,
    off_x: i8,
    off_y: i8,
    rot_off: u8,
) -> Option<u16> {
    let child = make_obj(g, SH_UP1_MAN_PROXY)?;
    let s = sid(g, up1manchild_strat);
    let s_coll = sid(g, up1manhit_istrat);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = UP1MAN_AP;
        al.collflags |= COLLTYPE_ENEMYWEAP;
        al.sbyte2 = off_x as u8;
        al.sbyte3 = off_y as u8;
        al.sbyte4 = rot_off;
    }
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    up1manchild_strat(g, child);
    Some(child)
}

/// C `Strat_Up1man_Init` (strat_enemy.c:4785).
pub fn strat_up1man_init(g: &mut Game, idx: u16) {
    let s = sid(g, up1man_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.sbyte2 = UP1MAN_ROT_SPEED;
        al.ptr = 0;
        al.sword1 = 0;
        al.sbyte1 = 0;
    }
    let _ = up1man_spawn_child(g, idx, 1, -80, 75, DEG45.wrapping_add(DEG90));
    let _ = up1man_spawn_child(g, idx, 2, 80, 75, DEG45.wrapping_add(DEG180));
    let _ = up1man_spawn_child(g, idx, 3, 0, -90, 0);
    up1man_strat(g, idx);
}

/// C `up1man_strat` (strat_enemy.c:4805).
fn up1man_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    // ASM up1man_strat (GASTRATS.ASM:2728) `s_jmp_alvarZERO B,x,al_sbyte3,.ninrng`
    // early-outs to the strat END when sbyte3==0, skipping the rotz spin,
    // worldz+=30, and the item spawn. sbyte3 starts 0 (never set in init) and is
    // only bumped when a child is hit, so the mother is static until then.
    // (Audit A #26)
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte2);
    }
    let me = g.objs.aliens[idx as usize];
    // ASM :2732 `s_jmp_Zdistmore x,y,#1500,.npos` scrolls only when |dz|<1500.
    // (Audit A Minor 3)
    if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < UP1MAN_ACTIVE_Z {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(UP1MAN_SCROLL_Z);
    }
    let me = g.objs.aliens[idx as usize];
    if me.sbyte3 != 3 || me.sflags2 & UP1MAN_SFLAG1 != 0 {
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= UP1MAN_SFLAG1;
    let Some(item) = make_obj(g, SH_MYSHIP_4) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    {
        let al = &mut g.objs.aliens[item as usize];
        al.worldx = me.worldx;
        al.worldy = me.worldy;
        al.worldz = me.worldz;
        al.sword1 = boss_obj_index_or_null(idx) as i16;
    }
    item0_istrat(g, item);
}

/// C `up1manhit_Istrat` (strat_enemy.c:4843).
fn up1manhit_istrat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & UP1MAN_SFLAG1 == 0 {
        g.hooks.play_se(0x10);
        if let Some(mother) = boss_get_mother_obj(g, idx) {
            let al = &mut g.objs.aliens[mother as usize];
            al.sbyte2 = al.sbyte2.wrapping_add(2);
            al.sbyte3 = al.sbyte3.wrapping_add(1);
        }
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= UP1MAN_SFLAG1;
        al.sflags &= !ASF_COLLIDE;
        al.sflags |= ASF_COLLDISABLE;
    }
    up1manchild_strat(g, idx);
}

/// C `up1manchild_strat` (strat_enemy.c:4865).
fn up1manchild_strat(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let m = g.objs.aliens[mother as usize];
    let s = strat_sin(m.rotz);
    let c = strat_cos(m.rotz);
    let me = g.objs.aliens[idx as usize];
    let off_x = ((me.sbyte2 as i8) as f32 * c - (me.sbyte3 as i8) as f32 * s) as i16;
    let off_y = ((me.sbyte2 as i8) as f32 * s + (me.sbyte3 as i8) as f32 * c) as i16;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = m.worldx.wrapping_add(off_x);
        al.worldy = m.worldy.wrapping_add(off_y);
        al.worldz = m.worldz;
        al.rotz = m.rotz.wrapping_add(al.sbyte4);
    }
    if g.objs.aliens[idx as usize].sflags2 & UP1MAN_SFLAG1 != 0
        && (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 1 == 0
    {
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
    }
}

// ============================================================
// ZACOS (C strat_enemy.c:4898-4975)
// ============================================================

/// C `zacos_move` (strat_enemy.c:4898).
fn zacos_move(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// C `Strat_Zacos_Init` (strat_enemy.c:4907).
pub fn strat_zacos_init(g: &mut Game, idx: u16) {
    let s = sid(g, zacos_phase0);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let posx = g.vars.player_posx;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = ZACOS_HP;
        al.ap = ZACOS_AP;
        al.vel = 40;
        al.rotx = DEG90;
        al.roty = DEG180;
        al.worldx = al.worldx.wrapping_add(posx);
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        al.snd2 = 0x0F;
    }
    // ASM zacos_Istrat (GASTRATS.ASM:942) ends `set_sound2 x,#$f` then
    // `zacos_strat s_start_strat` immediately — the body runs on the spawn
    // frame. (Audit A #37)
    zacos_phase0(g, idx);
}

/// C `zacos_phase0` (strat_enemy.c:4926).
fn zacos_phase0(g: &mut Game, idx: u16) {
    let target_y = g.vars.player_posy.wrapping_sub(800);
    // ASM zacos_strat (GASTRATS.ASM:950) `s_jmp_higher x,svar_word1,.nup` skips
    // when worldy < player_posy-800; the pitch/fire block runs when
    // worldy >= player_posy-800 (smaller y = higher). (Audit A #5)
    if g.objs.aliens[idx as usize].worldy >= target_y {
        if g.objs.aliens[idx as usize].rotx == 0 {
            let me = g.objs.aliens[idx as usize];
            strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
            let s = sid(g, zacos_phase1);
            g.objs.aliens[idx as usize].stratptr = Some(s);
        } else {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = al.rotx.wrapping_sub(2);
        }
    }
    zacos_move(g, idx);
}

/// C `zacos_phase1` (strat_enemy.c:4946).
fn zacos_phase1(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - pl.worldz as i32).abs() < 2000 {
            let s = sid(g, zacos_phase2);
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = al.rotx.wrapping_sub(4);
            al.stratptr = Some(s);
        }
    }
    zacos_move(g, idx);
}

/// C `zacos3_strat` (GASTRATS.ASM:981-996) — attack-dive laser barrage.
/// Each frame `rotx -= 4` (full 256-step loop). Fires RELSLOWELASER on the
/// frame rotx wraps to 0, otherwise every 4th frame while pitch is in the
/// forward arc (rotx not minus and not below -deg90). Transitions to zacos4
/// only after rotx wraps back to -4 (0xFC).
fn zacos_phase2(g: &mut Game, idx: u16) {
    let rotx = {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_sub(4); // s_add_alvar B,x,al_rotx,#-4
        al.rotx
    };
    // Fire gate (GASTRATS.ASM:986-989).
    let fire = if rotx == 0 {
        true // s_beq .fire
    } else if rotx & 0x80 != 0 {
        false // s_jmp_alvarmi -> .nfire
    } else if rotx.wrapping_sub(DEG90.wrapping_neg()) & 0x80 != 0 {
        false // s_jmp_alvarless #-deg90 -> .nfire
    } else {
        g.vars.gameframe & 3 == 0 // s_jmp_notdelay 2 -> .nfire
    };
    if fire {
        // s_weapon_rot #0,#0 -> fire forward relative to object orientation.
        let me = g.objs.aliens[idx as usize];
        strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
    }
    if rotx == 4u8.wrapping_neg() {
        // s_jmp_alvarEQ B,x,al_rotx,#-4,zacos4_init
        let s = sid(g, zacos_phase3);
        g.objs.aliens[idx as usize].stratptr = Some(s);
    }
    zacos_move(g, idx);
}

/// C `zacos4_strat` (GASTRATS.ASM:1001-1007) — s_speedto #60,1 + s_banktoplayer.
fn zacos_phase3(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
    szaco2_bank_to_player(g, idx);
    zacos_move(g, idx);
}

// ============================================================
// TOWER0 (C strat_enemy.c:4977-4991)
// ============================================================

/// C `tower0_strat` (strat_enemy.c:4977).
fn tower0_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(8);
}

/// C `Strat_Tower0_Init` (strat_enemy.c:4984).
pub fn strat_tower0_init(g: &mut Game, idx: u16) {
    let s = sid(g, tower0_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.ap = HARD_AP / 2;
    al.stratptr = Some(s);
}

// ============================================================
// HOUDAI (GASTRATS.ASM; C strat_enemy.c:5032-5113)
// ============================================================

/// C `houdai_target` (strat_enemy.c:5032).
fn houdai_target(g: &Game, idx: u16) -> Option<u16> {
    strat_obj_from_ptr(g.objs.aliens[idx as usize].sword1 as u16)
}

/// C `houdai_fire` (strat_enemy.c:5036).
fn houdai_fire(g: &mut Game, idx: u16) {
    let roty = g.objs.aliens[idx as usize].roty;
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        -62 >> 2,
        40 >> 2,
        DEG22.wrapping_neg(),
        roty,
        SHORTPLASMA_SPEED,
        SHORTPLASMA_LIFE,
        SHORTPLASMA_AP,
        ACF_COLLTYPE1 | ACF_COLLTYPE4,
    ) else {
        return;
    };
    let al = &mut g.objs.aliens[shot as usize];
    al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4;
    al.snd2 = 1;
}

/// C `houdai_strat` (strat_enemy.c:5057).
fn houdai_strat(g: &mut Game, idx: u16) {
    if let Some(target) =
        strat_find_near_colltype(g, idx, COLLTYPE_ENEMY2, HOUDAI_TRACK_MAX_Z, 10000)
    {
        g.objs.aliens[idx as usize].sword1 = (target + 1) as i16;
    }
    let target = houdai_target(g, idx).map(|t| g.objs.aliens[t as usize]);
    if let Some(t) = target.filter(|t| t.active) {
        let me = g.objs.aliens[idx as usize];
        // s_jmp_Zdistless x,y,#200,.nfindobj: tracked target too close ->
        // skip aim AND fire (jumps to end of strat).
        if ((me.worldz as i32 - t.worldz as i32).abs() as i16) < HOUDAI_TRACK_MIN_Z {
            return;
        }
        // s_obj2obj_angle x,y,al_roty,1: achase yaw toward target (shift 1),
        // not an instant angle snap.
        strat_aim_yaw(g, idx, &t, 1);
    }
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    // s_jmp_distless x,y,#800: XZ range (rangexz), not Z-only.
    if dist_xz(&me, &pl) < HOUDAI_FIRE_GATE_Z {
        return;
    }
    // ASM GASTRATS.ASM:1309 `s_jmp_notdelay 4,.nfindobj,al1pt` fires when
    // (gameframe+idx)&0x0F==0 (every 16 frames, per-object staggered), NOT
    // every 4 frames. (Audit A #3)
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F != 0 {
        return;
    }
    houdai_fire(g, idx);
}

/// C `Strat_HoudaiNS_Init` (strat_enemy.c:5089).
pub fn strat_houdai_ns_init(g: &mut Game, idx: u16) {
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = None;
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = HOUDAI_HP;
    al.ap = HOUDAI_AP;
    al.collflags |= COLLTYPE_ENEMY1;
}

/// C `Strat_Houdai_Init` (strat_enemy.c:5102).
pub fn strat_houdai_init(g: &mut Game, idx: u16) {
    let s = sid(g, houdai_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = HOUDAI_HP;
        al.ap = HOUDAI_AP;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    // ASM houdai_Istrat (GASTRATS.ASM:1292) ends `s_set_colltype x,enemy1` then
    // `houdai_strat s_start_strat` immediately — the body runs on the spawn
    // frame. (`houdaiNS` correctly does NOT; its Istrat ends s_end_strat.)
    // (Audit A #37)
    houdai_strat(g, idx);
}

// ============================================================
// ZACO3 / ZACO4 (C strat_enemy.c:5115-5433)
// ============================================================

/// C `zaco34_target` (strat_enemy.c:5115): raw `ptr` slot index.
fn zaco34_target(g: &Game, idx: u16) -> Option<u16> {
    let ptr = g.objs.aliens[idx as usize].ptr;
    if ptr as usize >= NUMBER_AL {
        return None;
    }
    if !g.objs.aliens[ptr as usize].active {
        return None;
    }
    Some(ptr)
}

/// C `Strat_Zaco3_Init` (strat_enemy.c:5133).
pub fn strat_zaco3_init(g: &mut Game, idx: u16) {
    let Some(target) = strat_find_near_shape(g, idx, SH_HOUDAI_0, None, 10000, 10000) else {
        g.objs.aliens[idx as usize].stratptr = None;
        return;
    };
    let s = sid(g, zaco3_attack);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, zaco3die_init);
    let al = &mut g.objs.aliens[idx as usize];
    al.ptr = strat_obj_index_or_null(target);
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = ZACO3_HP;
    al.ap = ZACO3_AP;
    al.rotz = DEG90;
    al.sbyte1 = 2;
    al.sbyte2 = 140;
    al.sbyte3 = 3;
    al.collflags |= COLLTYPE_ENEMY1;
    al.snd2 = 1;
}

/// C `zaco3_attack` (strat_enemy.c:5160).
fn zaco3_attack(g: &mut Game, idx: u16) {
    let target = zaco34_target(g, idx).map(|t| g.objs.aliens[t as usize]);
    let Some(t) = target else {
        let me = g.objs.aliens[idx as usize];
        let mut rotx = me.rotx;
        achase_angle(&mut rotx, DEG45.wrapping_neg(), 3);
        g.objs.aliens[idx as usize].rotx = rotx;
        strat_move3d(g, idx, 40, 2);
        return;
    };
    strat_aim_3d(g, idx, &t, 4);
    let me = g.objs.aliens[idx as usize];
    // s_jmp_distless y,x,#1300 (KSTRATS.ASM:115): XZ rangexz, not Manhattan-3D.
    let dist = dist_xz(&me, &t) as i32;
    if dist < 1300 && g.vars.gameframe & 7 == 0 {
        // ASM KSTRATS.ASM:118 `s_beqdec_alvar B,x,al_sbyte1,.circle`: TEST-then-DEC.
        // circle when sbyte1==0 (falls straight into zaco3circle_strat, :127);
        // else sbyte1-=1 and fire. Old port did DEC-then-test (fired once too
        // few, circled a tick early). (Audit A #14)
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            let s = sid(g, zaco3_circle);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratptr = Some(s);
                al.sbyte1 = 30;
                al.rotx = 0;
            }
            zaco3_circle(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        let me = g.objs.aliens[idx as usize];
        strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
    }
    strat_move3d(g, idx, 40, 2);
}

/// C `zaco3_circle` (strat_enemy.c:5194).
fn zaco3_circle(g: &mut Game, idx: u16) {
    if let Some(t) = zaco34_target(g, idx).map(|t| g.objs.aliens[t as usize]) {
        strat_aim_yaw(g, idx, &t, 4);
    }
    {
        let me = g.objs.aliens[idx as usize];
        let mut rotx = me.rotx;
        achase_angle(&mut rotx, 0, 2);
        g.objs.aliens[idx as usize].rotx = rotx;
    }
    if g.objs.aliens[idx as usize].sbyte1 > 0 {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            let s = sid(g, zaco3_flyaway);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            zaco3_flyaway(g, idx);
            return;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let target_y = if al.sbyte3 == 3 { -60 } else { -200 };
        // ASM KSTRATS.ASM:139/141 `s_Achase_alvar W,x,al_worldy,#-200/#-60,1` is
        // the 16-bit PROPORTIONAL chase, not the linear one. (Audit A #6)
        al.worldy = chase_proportional(al.worldy, target_y, 1);
    }
    strat_move3d(g, idx, 30, 2);
}

/// C `zaco3_flyaway` (strat_enemy.c:5220).
fn zaco3_flyaway(g: &mut Game, idx: u16) {
    let target = zaco34_target(g, idx);
    let mut target_yaw = (-30i8) as u8;
    // ROM s_jmp_rightofview (KSTRATS.ASM:149, STRATMAC.INC s_rightview_strat):
    // "right of view" = al_flags & afleftpl CLEAR — the showview view-side
    // flag, not a live worldx-vs-player compare.
    if target.is_none() || g.objs.aliens[idx as usize].flags & AF_LEFT_PL == 0 {
        target_yaw = 30;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut roty = al.roty;
        achase_angle(&mut roty, target_yaw, 4);
        al.roty = roty;
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, (-30i8) as u8, 2);
        al.rotx = rotx;
    }
    strat_move3d(g, idx, 20, 2);
    add_player_z(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    if al.sbyte2 > 0 {
        al.sbyte2 -= 1;
        if al.sbyte2 == 0 {
            g.objs.aldead = 1;
        }
    }
}

/// C `zaco3go_init` (strat_enemy.c:5249).
fn zaco3go_init(g: &mut Game, idx: u16) {
    let s = sid(g, zaco3go_strat);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_exp);
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
}

/// C `zaco3die_init` (strat_enemy.c:5260).
fn zaco3die_init(g: &mut Game, idx: u16) {
    let s = sid(g, zaco3die_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = Some(s);
    }
    g.hooks.play_se(0x10);
}

/// C `zaco3die_strat` (strat_enemy.c:5271).
fn zaco3die_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 1);
    // ASM KSTRATS.ASM:171 `s_jmp_lower x,#-100,zaco3GO_init` branches to the land
    // path when worldy >= -100 (smaller y = higher). (Audit A #5)
    if g.objs.aliens[idx as usize].worldy >= -100 {
        zaco3go_init(g, idx);
        zaco3go_strat(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // ASM KSTRATS.ASM:172 `s_jmp_alvarMORE B,x,al_rotx,#deg45` is SIGNED
        // (STRATMAC.INC:6652): add +4 while (i8)rotx <= deg45. (Audit A #18)
        if (al.rotx as i8) <= DEG45 as i8 {
            al.rotx = al.rotx.wrapping_add(4);
        }
        gen_vecs_3d(al);
        al.rotz = al.rotz.wrapping_add(4);
    }
    add_player_z(g, idx);
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// C `zaco3go_strat` (strat_enemy.c:5293).
fn zaco3go_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(4);
    }
    // ASM KSTRATS.ASM:190-200: |dz|>=3000 sets rotx=0/roty=deg180 then gens vecs
    // (.far); [400,3000) aims then gens vecs; |dz|<400 `s_jmp_Zdistless x,y,#400,
    // zaco3cont` branches PAST s_gen_3dvecs, keeping the stale vecs. (Audit A #19)
    let mut do_gen = true;
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist >= 3000 {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = 0;
            al.roty = DEG180;
        } else if zdist >= 400 {
            strat_aim_3d(g, idx, &pl, 2);
        } else {
            do_gen = false;
        }
    }
    if do_gen {
        gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// C `Strat_Zaco4_Init` (strat_enemy.c:5324).
pub fn strat_zaco4_init(g: &mut Game, idx: u16) {
    let Some(target) = strat_find_near_shape(g, idx, SH_PILLAR3, None, 10000, 10000) else {
        g.objs.aliens[idx as usize].stratptr = None;
        return;
    };
    let s = sid(g, zaco4_attack);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.ptr = strat_obj_index_or_null(target);
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = ZACO4_HP;
    al.ap = ZACO4_AP;
    al.rotz = DEG90;
    al.sbyte1 = 2;
    al.sbyte2 = 140;
    al.sbyte3 = 4;
    al.collflags |= COLLTYPE_ENEMY1;
    al.snd2 = 1;
}

/// C `zaco4_attack` (strat_enemy.c:5351).
fn zaco4_attack(g: &mut Game, idx: u16) {
    let target = zaco34_target(g, idx).map(|t| g.objs.aliens[t as usize]);
    let Some(t) = target else {
        let me = g.objs.aliens[idx as usize];
        let mut rotx = me.rotx;
        achase_angle(&mut rotx, DEG45.wrapping_neg(), 3);
        g.objs.aliens[idx as usize].rotx = rotx;
        strat_move3d(g, idx, 40, 2);
        return;
    };
    strat_aim_3d(g, idx, &t, 4);
    let me = g.objs.aliens[idx as usize];
    // s_jmp_distless y,x,#1300 (KSTRATS.ASM:115): XZ rangexz, not Manhattan-3D.
    let dist = dist_xz(&me, &t) as i32;
    if dist < 1300 && g.vars.gameframe & 7 == 0 {
        // ASM KSTRATS.ASM:118 `s_beqdec_alvar` (TEST-then-DEC) + `.circle` (:127)
        // falls into zaco4circle_strat the same tick. Old port did DEC-then-test
        // and deferred the circle a frame. (Audit A #14, #16)
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            let s = sid(g, zaco4_circle);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte1 = 30;
                al.rotx = 0;
                al.stratptr = Some(s);
            }
            zaco4_circle(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        let me = g.objs.aliens[idx as usize];
        strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
    }
    strat_move3d(g, idx, 40, 2);
}

/// C `zaco4_circle` (strat_enemy.c:5385).
fn zaco4_circle(g: &mut Game, idx: u16) {
    if let Some(t) = zaco34_target(g, idx).map(|t| g.objs.aliens[t as usize]) {
        strat_aim_yaw(g, idx, &t, 4);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, 0, 2);
        al.rotx = rotx;
        // ASM KSTRATS.ASM:139 `s_Achase_alvar W,x,al_worldy,#-200,1` is the 16-bit
        // PROPORTIONAL chase, not linear. (Audit A #6)
        al.worldy = chase_proportional(al.worldy, -200, 1);
    }
    if g.objs.aliens[idx as usize].sbyte1 > 0 {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            // ASM `.flyaway` (KSTRATS.ASM:144) falls into its strat body the same
            // tick. (Audit A #16)
            let s = sid(g, zaco4_flyaway);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            zaco4_flyaway(g, idx);
            return;
        }
    }
    strat_move3d(g, idx, 30, 2);
}

/// C `zaco4_flyaway` (strat_enemy.c:5407).
fn zaco4_flyaway(g: &mut Game, idx: u16) {
    let target = zaco34_target(g, idx);
    let mut target_yaw = (-30i8) as u8;
    // ASM shares zaco3's `.flyaway`, using `s_jmp_rightofview` (leftpl CLEAR,
    // KSTRATS.ASM:149) — the view-side flag, NOT a live worldx-vs-player compare.
    // (Audit A #17; matches zaco3_flyaway.)
    if target.is_none() || g.objs.aliens[idx as usize].flags & AF_LEFT_PL == 0 {
        target_yaw = 30;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut roty = al.roty;
        achase_angle(&mut roty, target_yaw, 4);
        al.roty = roty;
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, (-30i8) as u8, 2);
        al.rotx = rotx;
    }
    strat_move3d(g, idx, 20, 2);
    add_player_z(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    if al.sbyte2 > 0 {
        al.sbyte2 -= 1;
        if al.sbyte2 == 0 {
            g.objs.aldead = 1;
        }
    }
}

// ============================================================
// ZACO0 (C strat_enemy.c:5450-5553)
// ============================================================

/// C `Strat_Zaco0_Init` (strat_enemy.c:5450).
pub fn strat_zaco0_init(g: &mut Game, idx: u16) {
    let s = sid(g, zaco0_sweep);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = ZACO0_HP;
    al.ap = ZACO0_AP;
    al.roty = DEG270;
    al.rotx = DEG90;
    al.sbyte1 = 10;
    al.snd2 = 3;
    al.collflags |= COLLTYPE_ENEMY1;
}

/// C `zaco0_sweep` (strat_enemy.c:5467).
fn zaco0_sweep(g: &mut Game, idx: u16) {
    let pl = player(g);
    if let Some(pl) = pl {
        if g.objs.aliens[idx as usize].worldy < pl.worldy {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = al.worldy.wrapping_add(20);
            if al.worldy > -30 {
                al.worldy = -30;
            }
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(43);
    }
    if let Some(pl) = pl {
        if g.objs.aliens[idx as usize].worldx >= pl.worldx {
            let s = sid(g, zaco0_turn_in);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            zaco0_turn_in(g, idx);
        }
    }
}

/// C `zaco0_turn_in` (strat_enemy.c:5489).
fn zaco0_turn_in(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_sub(8);
        al.roty = al.roty.wrapping_sub(8);
    }
    if g.objs.aliens[idx as usize].roty == DEG180 {
        let s = sid(g, zaco0_fire);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        zaco0_fire(g, idx);
    }
}

/// C `zaco0_fire` (strat_enemy.c:5502).
fn zaco0_fire(g: &mut Game, idx: u16) {
    let pl = player(g);
    // ASM KSTRATS.ASM:241 `s_jmp_notdelay 2,.nfire,al1pt` fires when
    // (gameframe+idx)&3==0 (every 4 frames, staggered), not every 2. (Audit A #4)
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 3 == 0 {
        if let Some(pl) = pl {
            // ASM `s_weapon_rndrots2obj y,3,3` (KSTRATS.ASM:244) = per-axis
            // (rnd&3)-1, PITCH(x) drawn BEFORE YAW(y). Old port used (rnd%3)-1,
            // yaw-first. (Audit A Minor 5)
            let spread_pitch = ((ea_random(g) & 3) as i8).wrapping_sub(1);
            let spread_yaw = ((ea_random(g) & 3) as i8).wrapping_sub(1);
            let me = g.objs.aliens[idx as usize];
            let fire_pitch = strat_pitch_toward(&me, &pl).wrapping_add(spread_pitch as u8);
            let fire_yaw = angle_xz(&me, &pl).wrapping_add(spread_yaw as u8);
            strat_fire_relslowlaser(g, idx, fire_pitch, fire_yaw);
        }
    }
    if g.objs.aliens[idx as usize].sbyte1 > 0 {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            let s = sid(g, zaco0_turn_out);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            zaco0_turn_out(g, idx);
            return;
        }
    }
    if let Some(pl) = pl {
        let me = g.objs.aliens[idx as usize];
        if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < 300 {
            let s = sid(g, zaco0_turn_out);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            zaco0_turn_out(g, idx);
        }
    }
}

/// C `zaco0_turn_out` (strat_enemy.c:5532).
fn zaco0_turn_out(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(8);
        al.roty = al.roty.wrapping_add(8);
    }
    if g.objs.aliens[idx as usize].roty == DEG270 {
        let s = sid(g, zaco0_flyaway);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        zaco0_flyaway(g, idx);
    }
}

/// C `zaco0_flyaway` (strat_enemy.c:5545).
fn zaco0_flyaway(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(19);
        al.worldx = al.worldx.wrapping_add(40);
    }
    strat_move3d(g, idx, 50, 2);
}

// ============================================================
// PARA / CARRIER (C strat_enemy.c:5555-5761)
// ============================================================

/// C `Strat_Para_Init` (strat_enemy.c:5555).
pub fn strat_para_init(g: &mut Game, idx: u16) {
    let s = sid(g, para_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = PARA_HP;
    al.ap = PARA_AP;
    al.sbyte1 = (-PARA_SWINGMAX) as u8;
    al.sflags |= ASF_SHADOW;
}

/// C `para_strat` (strat_enemy.c:5569).
fn para_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(10);
        let mut swing = al.sbyte1 as i8;
        if al.rotz < 128 {
            if swing != -PARA_SWINGMAX {
                swing = swing.wrapping_sub(PARA_SWINGSPD);
            }
        } else if swing != PARA_SWINGMAX {
            swing = swing.wrapping_add(PARA_SWINGSPD);
        }
        al.sbyte1 = swing as u8;
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
    }
    if g.objs.aliens[idx as usize].worldy >= 0 {
        let s = sid(g, para2_strat);
        let shape = g.world.shapes_table[SH_PARA_1_PROXY as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.worldy = 0;
        al.rotz = 0;
        al.shape = shape;
        al.vel = 10;
        // ASM para2_istrat (D2STRATS.ASM:569-577) does `s_initface_player` (clears
        // smflag1 so the first para2 tick latches the aim) and ends with
        // `s_end_strat` — para2_strat does NOT run this frame. (Audit A #20)
        al.sflags2 &= !ASF2_SMFLAG1;
    }
}

/// C `para2_strat` (strat_enemy.c:5601).
fn para2_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    // ASM para2_strat (D2STRATS.ASM:580) `s_face_player x,1,0,.nogen` homes toward
    // the FIXED sbyte3/sbyte4 aim, latched on the first tick (when smflag1 clear)
    // via `s_obj2obj_3Dangle ...,0` (instant snap). delay 0 => never re-aims. Old
    // port re-aimed at the live player every frame. (Audit A #21)
    let mut aligned = false;
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1 == 0 {
        if let Some(pl) = pl {
            let me = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SMFLAG1;
    } else {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        let r1 = achase_angle(&mut roty, me.sbyte3, 1);
        let mut rotx = me.rotx;
        let r2 = achase_angle(&mut rotx, me.sbyte4, 1);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        aligned = r1 && r2;
    }
    if !aligned {
        gen_vecs_2d(&mut g.objs.aliens[idx as usize]);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(al.vx);
        al.worldz = al.worldz.wrapping_add(al.vz);
    }
    if let Some(pl) = pl {
        let me = g.objs.aliens[idx as usize];
        if dist_xz(&me, &pl) < 400 {
            let s = sid(g, parajump_strat);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            parajump_strat(g, idx);
            return;
        }
    }
    // ASM D2STRATS.ASM:587 `s_jmp_notdelay 4,.njump,al1pt` sets vy=-15 when
    // (gameframe+idx)&0x0F==0 (every 16 frames, staggered), not every 4. (Audit A #4)
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F == 0 {
        g.objs.aliens[idx as usize].vy = -15;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(al.vx);
    al.worldy = al.worldy.wrapping_add(al.vy);
    al.worldz = al.worldz.wrapping_add(al.vz);
    // ASM D2STRATS.ASM:592 `s_falldown_Yvec x,1,#3,#0` adds +3 to vy. (Audit A #22)
    al.vy = al.vy.wrapping_add(3);
    if al.worldy >= 0 {
        al.worldy = 0;
        al.vy = -(al.vy / 2);
        if al.vy > -5 {
            al.vy = 0;
        }
    }
}

/// C `parajump_strat` (strat_enemy.c:5646).
fn parajump_strat(g: &mut Game, idx: u16) {
    let posy = g.vars.player_posy;
    {
        let al = &mut g.objs.aliens[idx as usize];
        // ASM D2STRATS.ASM:600 `s_achase_alvar W,x,al_worldy,player_posy,2` is the
        // PROPORTIONAL chase. (Audit A #7)
        al.worldy = chase_proportional(al.worldy, posy, 2);
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) <= 200 {
            let al = &mut g.objs.aliens[idx as usize];
            // ASM D2STRATS.ASM:604 `s_achase_alvar W,x,al_worldx,player_posx,3`
            // (proportional). (Audit A #7)
            al.worldx = chase_proportional(al.worldx, pl.worldx, 3);
        }
    }
}

/// C `carrier_spawn_para` (strat_enemy.c:5661).
fn carrier_spawn_para(g: &mut Game, idx: u16) {
    let shape = g.world.shapes_table[SH_PARA_0 as usize];
    let Some(child) = make_obj(g, shape) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    {
        let al = &mut g.objs.aliens[child as usize];
        al.worldx = me.worldx;
        al.worldy = me.worldy.wrapping_add(90);
        al.worldz = me.worldz;
        al.immuneptr = strat_obj_index_or_null(idx);
    }
    g.objs.aliens[idx as usize].immuneptr = strat_obj_index_or_null(child);
    strat_para_init(g, child);
}

/// C `Strat_Carrier_Init` (strat_enemy.c:5681).
pub fn strat_carrier_init(g: &mut Game, idx: u16) {
    let s = sid(g, carrier_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = CARRIER_HP;
    al.ap = CARRIER_AP;
    al.snd2 = 14;
}

/// C `carrier_strat` (strat_enemy.c:5694).
fn carrier_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(3);
        al.worldy = al.worldy.wrapping_add(2);
        al.worldz = al.worldz.wrapping_add(30);
    }
    if let Some(pl) = pl {
        let me = g.objs.aliens[idx as usize];
        // ASM KSTRATS.ASM:284 `s_jmp_Zdistmore x,y,#3000` transitions when
        // |dz| >= 3000 (inclusive). (Audit A Minor 3)
        if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) >= 3000 {
            let s = sid(g, carrierb_strat);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratptr = Some(s);
                al.sbyte1 = 32;
                al.sbyte2 = 1;
            }
            carrierb_strat(g, idx);
            return;
        }
    }
    add_player_z(g, idx);
}

/// C `carrierb_strat` (strat_enemy.c:5717).
fn carrierb_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        g.objs.aliens[idx as usize].sbyte2 = CARRIER_RATE;
        carrier_spawn_para(g, idx);
    }
    if let Some(pl) = player(g) {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, pl.worldx, 3);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, 0, 4);
        al.rotx = rotx;
        al.worldy = chase_proportional(al.worldy, -320, 5);
        al.roty = al.roty.wrapping_add(4);
    }
    strat_move3d(g, idx, 30, 1);
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(15);
        if al.sbyte1 > 0 {
            al.sbyte1 -= 1;
        } else {
            return;
        }
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        let s = sid(g, carrierc_strat);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        carrierc_strat(g, idx);
    }
}

/// C `carrierc_strat` (strat_enemy.c:5752).
fn carrierc_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(10);
        al.worldy = al.worldy.wrapping_sub(3);
        al.roty = al.roty.wrapping_add(4);
    }
    add_player_z(g, idx);
}

// ============================================================
// BASE1 (C strat_enemy.c:5763-5817)
// ============================================================

/// C `Strat_Base1_Init` — ASM `base1_Istrat` (KSTRATS.ASM:373-379):
/// `s_set_alptrs base1_strat,0,0` (null collide+explode), `s_set_aldata
/// #hardhp,#2` (ap=2), `roty=deg180`, `init_anim 0`. The old port set a
/// hit_flash collide, an explode strat, ap=HARD_AP(8), added ASF_NOHITAFFECT,
/// and had no roty. (Audit A #10 — CAVEAT: the reference is the ultrastarfox
/// hack; confirm base1 against the original disassembly, as the removed C
/// oracle may have targeted a different base1.)
pub fn strat_base1_init(g: &mut Game, idx: u16) {
    let s = sid(g, base1_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = 2;
    al.roty = DEG180;
    al.animframe = 0;
}

/// C `base1_strat` — ASM `base1_strat` (KSTRATS.ASM:380): idle until the door
/// is hit (`s_test_hitflags x,#HF1`), then open (anim 0->8) with the door-open
/// sound, wait, close (anim 8->0) with the door-close sound, re-init. (Audit A #10)
fn base1_strat(g: &mut Game, idx: u16) {
    // s_test_hitflags x,#HF1 / s_bne .anim_istrat — idle until struck.
    if g.objs.aliens[idx as usize].hitflags & HF1_MASK == 0 {
        return;
    }
    // .anim_istrat: clear HF1, door-open sound, switch to open (falls through).
    let s = sid(g, base1_open_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags &= !HF1_MASK;
        al.stratptr = Some(s);
    }
    g.hooks.play_se(BASE1_DOOR_OPEN_SE);
    base1_open_strat(g, idx);
}

/// ASM `.anim_strat` (KSTRATS.ASM:389): anim 0->8, then wait.
fn base1_open_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].animframe == 8 {
        // .wait_istrat: switch to wait, sbyte1=5, fall through.
        let s = sid(g, base1_wait_strat);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratptr = Some(s);
            al.sbyte1 = BASE1_WAIT_FRAMES;
        }
        base1_wait_strat(g, idx);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.animframe = al.animframe.wrapping_add(1);
}

/// ASM `.wait_strat` (KSTRATS.ASM:397): dwell `sbyte1` frames, then close.
fn base1_wait_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        // .close: door-close sound, switch to close, fall through.
        let s = sid(g, base1_close_strat);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        g.hooks.play_se(BASE1_DOOR_CLOSE_SE);
        base1_close_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
}

/// ASM `.closeit` (KSTRATS.ASM:403): anim 8->0, then re-init to idle.
fn base1_close_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].animframe == 0 {
        // s_beq base1_istrat: re-init back to the idle door.
        strat_base1_init(g, idx);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.animframe = al.animframe.wrapping_sub(1);
}

// ============================================================
// CAMELEON (C strat_enemy.c:5821-5888)
// ============================================================

/// C `cameleon_phase1` (strat_enemy.c:5821).
fn cameleon_phase1(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].rotx != DEG180 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = al.rotx.wrapping_add(16);
        }
        add_player_z(g, idx);
        return;
    }
    if g.objs.aliens[idx as usize].rotz != DEG90 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotz = al.rotz.wrapping_add(4);
        }
        add_player_z(g, idx);
        return;
    }
    // ASM DSTRATS.ASM:1545 `s_beqdec_alvar B,x,al_sbyte1,.cameleon_strat3_i`:
    // TEST-then-DEC — transition to phase2 (falls straight through, :1553-1555)
    // when sbyte1==0, else sbyte1-=1 and run the fire gate. Old port did
    // DEC-then-test. (Audit A #15)
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        let s = sid(g, cameleon_phase2);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        cameleon_phase2(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    // ASM DSTRATS.ASM:1546 `s_jmp_NOTdelay 4,.camstrat_end,al1pt` fires when
    // (gameframe+idx)&0x0F==0 (every 16 frames, staggered), not every 4. (Audit A #4)
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F == 0 {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let pitch = strat_pitch_toward(&me, &pl);
            let yaw = angle_xz(&me, &pl);
            strat_fire_relslowlaser(g, idx, pitch, yaw);
        }
    }
    add_player_z(g, idx);
}

/// C `cameleon_phase2` (strat_enemy.c:5861).
fn cameleon_phase2(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].roty != DEG180 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = al.roty.wrapping_add(16);
        }
        add_player_z(g, idx);
        return;
    }
    g.objs.aldead = 1;
}

/// C `Strat_Cameleon_Init` (strat_enemy.c:5875).
pub fn strat_cameleon_init(g: &mut Game, idx: u16) {
    let s = sid(g, cameleon_phase1);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = CAMELEON_HP;
        al.ap = CAMELEON_AP;
        al.sbyte1 = 20;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    g.hooks.play_se(0x2B);
}

// ============================================================
// SZACO2 (GA2STRAT.ASM; C strat_enemy.c:5958-6126)
// ============================================================

/// C `szaco2_waypoint_yaw` (strat_enemy.c:5958): negated waypoint angle.
fn szaco2_waypoint_yaw(al: &Alien) -> u8 {
    let dx = (al.swpx1 as i32 - al.worldx as i32) as f32;
    let dz = (al.swpz1 as i32 - al.worldz as i32) as f32;
    let mut angle = dx.atan2(dz);
    if angle < 0.0 {
        angle += 2.0 * 3.141_592_65_f32;
    }
    (-((angle * (256.0 / (2.0 * 3.141_592_65_f32))) as i32)) as u8
}

/// C `szaco2_waypoint_pitch` (strat_enemy.c:5978).
fn szaco2_waypoint_pitch(al: &Alien) -> u8 {
    let dx = (al.swpx1 as i32 - al.worldx as i32) as f32;
    let dy = (al.swpy1 as i32 - al.worldy as i32) as f32;
    let dz = (al.swpz1 as i32 - al.worldz as i32) as f32;
    let dist = (dx * dx + dz * dz).sqrt();
    if dist <= 1.0 {
        return al.rotx;
    }
    let pitch = dy.atan2(dist);
    ((pitch * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8
}

/// C `szaco2_bank_to_player` (strat_enemy.c:6001, sr_banktoplayer).
fn szaco2_bank_to_player(g: &mut Game, idx: u16) {
    let posx = g.vars.player_posx;
    let posy = g.vars.player_posy;
    let dx = {
        let al = &mut g.objs.aliens[idx as usize];
        let dx = al.worldx.wrapping_sub(posx);
        al.rotz = al.rotz.wrapping_add(((dx >> 6) as i8) as u8);
        dx
    };
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x03 != 0 {
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    if dx >= 0 {
        al.roty = al.roty.wrapping_add(1);
    } else {
        al.roty = al.roty.wrapping_sub(1);
    }
    let dy = al.worldy.wrapping_sub(posy);
    if dy >= 0 {
        al.rotx = al.rotx.wrapping_add(1);
    } else {
        al.rotx = al.rotx.wrapping_sub(1);
    }
}

/// C `szaco2_cont` (GA2STRAT.ASM:287-291).
fn szaco2_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// C `szaco2_strat` (GA2STRAT.ASM:241-286).
fn szaco2_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    match g.objs.aliens[idx as usize].stratstate {
        0 => {
            let al = &mut g.objs.aliens[idx as usize];
            al.swpz1 = al.worldz.wrapping_sub(10);
            let yaw_t = szaco2_waypoint_yaw(al);
            let mut roty = al.roty;
            achase_angle(&mut roty, yaw_t, SZACO2_TURN_SHIFT);
            al.roty = roty;
            let pitch_t = szaco2_waypoint_pitch(al);
            let mut rotx = al.rotx;
            achase_angle(&mut rotx, pitch_t, SZACO2_TURN_SHIFT);
            al.rotx = rotx;
            if al.worldy < al.swpy1 {
                al.stratstate = 1;
            }
        }
        1 => {
            let al = &mut g.objs.aliens[idx as usize];
            let mut roty = al.roty;
            let yaw_done = achase_angle(&mut roty, DEG180, SZACO2_FIN_SHIFT);
            al.roty = roty;
            let mut rotx = al.rotx;
            let pitch_done = achase_angle(&mut rotx, 0, SZACO2_FIN_SHIFT);
            al.rotx = rotx;
            if yaw_done && pitch_done {
                al.stratstate = 2;
            }
        }
        2 => {
            if let Some(pl) = pl {
                let me = g.objs.aliens[idx as usize];
                let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
                if zdist < SZACO2_BANK_Z {
                    szaco2_bank_to_player(g, idx);
                    if zdist < SZACO2_DASH_Z {
                        g.objs.aliens[idx as usize].stratstate = 3;
                    }
                }
            }
        }
        3 => {
            let al = &mut g.objs.aliens[idx as usize];
            let mut roty = al.roty;
            achase_angle(&mut roty, DEG180, SZACO2_FIN_SHIFT);
            al.roty = roty;
        }
        _ => {}
    }
    if let Some(pl) = pl {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist >= SZACO2_FIRE_NEAR_Z && zdist < SZACO2_FIRE_FAR_Z {
            if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx))
                & SZACO2_FIRE_MASK
                != 0
            {
                szaco2_cont(g, idx);
                return;
            }
            let me = g.objs.aliens[idx as usize];
            strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
        }
    }
    szaco2_cont(g, idx);
}

/// C `Strat_Szaco2_Init` (GA2STRAT.ASM:229-240).
pub fn strat_szaco2_init(g: &mut Game, idx: u16) {
    let s = sid(g, szaco2_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let posy = g.vars.player_posy;
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = SZACO2_HP;
    al.ap = SZACO2_AP;
    al.stratptr = Some(s);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.vel = SZACO2_SPEED;
    al.swpy1 = al
        .swpy1
        .wrapping_add(SZACO2_WPY_OFFSET)
        .wrapping_add(posy);
    al.animframe = 0x80 | SZACO2_ANIM_INIT;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.snd2 = 3;
    // relexplode/zaco_8p death debris deferred (C comment).
}

// ============================================================
// ZACO1 (GASTRATS.ASM; C strat_enemy.c:6148-6364)
// ============================================================

/// C `zaco1_cont` (GASTRATS.ASM:1217-1226).
fn zaco1_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        // ASM GASTRATS.ASM:1219 `s_jmp_higher x,#0,.hok` skips the clamp when
        // worldy < 0; `worldy=0` runs when worldy >= 0. (Audit A #5)
        if al.worldy >= 0 {
            al.worldy = 0;
        }
        al.worldy = al.worldy.wrapping_add(al.sword2);
        al.worldx = al.worldx.wrapping_add(al.ptr as i16);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// C `zaco1_common_init` (zaco1_Icont, GASTRATS.ASM:1192-1206).
fn zaco1_common_init(g: &mut Game, idx: u16) {
    let s = sid(g, zaco1_phase0);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    let posx = g.vars.player_posx;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.worldx = al.worldx.wrapping_add(posx);
        al.sword1 = al.sword1.wrapping_add(posx);
        al.stratptr = Some(s);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = ZACO1_HP;
        al.ap = ZACO1_AP;
        al.vel = 60;
        al.type_ &= !ATZREMOVE;
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP | COLLTYPE_ZENEMY;
        al.swpx1 = al.sword1;
        al.snd2 = 3;
    }
    // ASM zaco1_Icont (GASTRATS.ASM:1206) ends `set_sound2 x,#3` then
    // `zaco1_strat s_start_strat` immediately — the body runs on the spawn
    // frame. (Audit A #37)
    zaco1_phase0(g, idx);
}

const ZACO1_HP: u8 = 2;
const ZACO1_AP: u8 = 4;

/// C `Strat_Zaco1L_Init` (zaco1L_Istrat, GASTRATS.ASM:1182-1186).
pub fn strat_zaco1l_init(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sword1 = 1000;
        al.sbyte2 = 6;
    }
    zaco1_common_init(g, idx);
}

/// C `Strat_Zaco1R_Init` (zaco1R_Istrat, GASTRATS.ASM:1187-1190).
pub fn strat_zaco1r_init(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sword1 = -1000;
        al.sbyte2 = (-6i8) as u8;
    }
    zaco1_common_init(g, idx);
}

/// C `zaco1_phase0` (zaco1_strat, GASTRATS.ASM:1208-1226).
fn zaco1_phase0(g: &mut Game, idx: u16) {
    {
        let posz = g.vars.player_posz;
        let al = &mut g.objs.aliens[idx as usize];
        al.swpz1 = posz.wrapping_add(1500);
        // s_obj2WP_angle: chase yaw/pitch toward the waypoint.
        let dx = (al.swpx1 as i32 - al.worldx as i32) as f32;
        let dz = (al.swpz1 as i32 - al.worldz as i32) as f32;
        let mut angle = dx.atan2(dz);
        if angle < 0.0 {
            angle += 2.0 * 3.141_592_65_f32;
        }
        let target_yaw = (-((angle * (256.0 / (2.0 * 3.141_592_65_f32))) as i32)) as u8;
        let mut roty = al.roty;
        achase_angle(&mut roty, target_yaw, 3);
        al.roty = roty;
        let dy = (0i32 - al.worldy as i32) as f32;
        let dist_xz = (dx * dx + dz * dz).sqrt();
        if dist_xz > 1.0 {
            let pitch = dy.atan2(dist_xz);
            let target_pitch = ((pitch * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8;
            let mut rotx = al.rotx;
            achase_angle(&mut rotx, target_pitch, 3);
            al.rotx = rotx;
        }
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs();
        if zdist > 1000 {
            let s = sid(g, zaco1_phase1);
            let al = &mut g.objs.aliens[idx as usize];
            al.stratptr = Some(s);
            al.type_ |= ATZREMOVE; // s_setremove_behind
        }
    }
    zaco1_cont(g, idx);
}

/// C `zaco1_phase1` (zaco1a_strat, GASTRATS.ASM:1230-1234).
fn zaco1_phase1(g: &mut Game, idx: u16) {
    let reached = {
        let al = &mut g.objs.aliens[idx as usize];
        let mut roty = al.roty;
        let reached = achase_angle(&mut roty, DEG0, 3);
        al.roty = roty;
        al.rotx = al.rotx.wrapping_sub(1);
        reached
    };
    if reached {
        let s = sid(g, zaco1_phase2);
        g.objs.aliens[idx as usize].stratptr = Some(s);
    }
    zaco1_cont(g, idx);
}

/// C `zaco1_phase2` (zaco1b_strat, GASTRATS.ASM:1238-1276).
fn zaco1_phase2(g: &mut Game, idx: u16) {
    let pl = player(g);
    let zdist: i32 = match pl {
        Some(p) => {
            (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs()
        }
        None => 0,
    };
    if zdist < 1400 {
        // .circ — spiral attack (GASTRATS.ASM:1253-1259).
        let al = &mut g.objs.aliens[idx as usize];
        let _ = speed_to(al, 45, 1);
        al.sbyte1 = al.rotz;
        al.sbyte1 = al.sbyte1.wrapping_sub(DEG90);
        al.sword2 = ((strat_sin(al.sbyte1) * 127.0f32) as i16) >> 3;
        al.ptr = (((strat_cos(al.sbyte1) * 127.0f32) as i16) >> 2) as u16;
        al.rotz = al.rotz.wrapping_add(al.sbyte2);
    } else if (1400..1800).contains(&zdist) {
        // Fire RELSLOWELASERHOME every 4th frame (s_jmp_notdelay 2 = mask 3),
        // aimed at the player in full 3D (s_weapon_rots2obj y). Band is
        // 1400<=|dz|<1800 — the ASM `s_jmp_Zdistmore x,y,#1800` excludes 1800.
        // (Audit A Minor 4). The mid/far bands do NOT zero sword2/ptr — only
        // `.circ` writes them, and leaving `.circ` keeps the last spiral offsets
        // (zaco1_cont keeps adding them). (Audit A #30)
        if g.vars.gameframe & 3 == 0 {
            if let Some(p) = pl {
                let me = g.objs.aliens[idx as usize];
                let fire_yaw = angle_xz(&me, &p);
                let fire_pitch = strat_pitch_toward(&me, &p);
                strat_fire_relslowlaserhome(g, idx, fire_pitch, fire_yaw);
            }
        }
    }
    if zdist >= 700 {
        if let Some(p) = pl {
            let me = g.objs.aliens[idx as usize];
            let target_yaw = angle_xz(&me, &p);
            let mut roty = me.roty;
            achase_angle(&mut roty, target_yaw, 3);
            g.objs.aliens[idx as usize].roty = roty;
            let dy = (p.worldy as i32 - me.worldy as i32) as f32;
            let dx = (p.worldx as i32 - me.worldx as i32) as f32;
            let dz = (p.worldz as i32 - me.worldz as i32) as f32;
            let dist_xz = (dx * dx + dz * dz).sqrt();
            if dist_xz > 1.0 {
                let pitch = dy.atan2(dist_xz);
                let target_pitch =
                    ((pitch * (256.0 / (2.0 * 3.141_592_65_f32))) as i32) as u8;
                let mut rotx = g.objs.aliens[idx as usize].rotx;
                achase_angle(&mut rotx, target_pitch, 3);
                g.objs.aliens[idx as usize].rotx = rotx;
            }
        }
    }
    zaco1_cont(g, idx);
}

// ============================================================
// FRIEND EXIT BASE (GISTRATS.ASM; C strat_enemy.c:6373-6429)
// ============================================================

const PEXITBASE_SPEED: i16 = 50;

/// C `friendexitbase_strat` (GISTRATS.ASM:321-337).
fn friendexitbase_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 > 0 {
            al.sbyte1 -= 1;
            if al.sbyte1 > 0 {
                return; // .nowt
            }
        }
        al.sbyte1 = 1;
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
            al.snd1 = 0xB1; // right-channel sound
        } else {
            al.snd1 = 0x51; // left-channel sound
        }
        al.worldz = al.worldz.wrapping_add(PEXITBASE_SPEED);
        al.sflags &= !ASF_INVISIBLE;
        if al.count > 0 {
            al.count -= 1;
        } else {
            g.objs.aldead = 1;
        }
    }
}

/// C `Strat_FriendExitBase_Init` (GISTRATS.ASM:314-320).
pub fn strat_friendexitbase_init(g: &mut Game, idx: u16) {
    let s = sid(g, friendexitbase_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.count = (1500 / PEXITBASE_SPEED) as u8;
    al.stratptr = Some(s);
    al.sflags |= ASF_SHADOW;
    al.sflags |= ASF_INVISIBLE;
    al.sbyte2 = 11;
}

// ============================================================
// CLEAR-DEMO SHIPS (GCSTRATS.ASM; C strat_enemy.c:6440-6807)
// ============================================================

/// C `clshipboost_step` (strat_enemy.c:6440).
fn clshipboost_step(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 != 0 {
            al.sbyte2 -= 1;
            if al.sbyte2 == 1 {
                g.objs.aldead = 1;
                return;
            }
        }
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2);
}

/// C `clshipboost_enter` (strat_enemy.c:6454).
fn clshipboost_enter(g: &mut Game, idx: u16, play_sound: bool) {
    let s = sid(g, clshipboost_step);
    let al = &mut g.objs.aliens[idx as usize];
    if play_sound {
        al.snd2 = 0x32;
    }
    al.stratptr = Some(s);
    al.vel = 120;
}

/// C `clship_flyinleft` (strat_enemy.c:6462).
fn clship_flyinleft(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    let tick3 = frame_tick_mod(g, 3);
    let al = &mut g.objs.aliens[idx as usize];
    if al.sflags2 & CLSHIP_FLAG1 == 0 {
        if al.worldx >= -30 {
            al.rotz = al.rotz.wrapping_add(2);
            // ASM flyinleft_srou (GCSTRATS.ASM:53-57): BOTH the vx==-5 flag-set
            // AND vx-=1 sit inside the `s_jmp_notdelay 1` gate. (Audit A Minor 7)
            if tick1 {
                if al.vx == -5 {
                    al.sflags2 |= CLSHIP_FLAG1;
                } else {
                    al.vx -= 1;
                }
            }
        }
    } else if al.vx != 0 && tick3 {
        al.vx += 1;
    }
    al.worldx = al.worldx.wrapping_add(al.vx);
}

/// C `clship_flyinright` (strat_enemy.c:6479).
fn clship_flyinright(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    let tick3 = frame_tick_mod(g, 3);
    let al = &mut g.objs.aliens[idx as usize];
    if al.sflags2 & CLSHIP_FLAG1 == 0 {
        if al.worldx <= 30 {
            al.rotz = al.rotz.wrapping_sub(2);
            // ASM flyinright_srou (GCSTRATS.ASM:66-70): BOTH the vx==5 flag-set
            // AND vx+=1 sit inside the `s_jmp_notdelay 1` gate. (Audit A Minor 7)
            if tick1 {
                if al.vx == 5 {
                    al.sflags2 |= CLSHIP_FLAG1;
                } else {
                    al.vx += 1;
                }
            }
        }
    } else if al.vx != 0 && tick3 {
        al.vx -= 1;
    }
    al.worldx = al.worldx.wrapping_add(al.vx);
}

/// C `clship_warp_cont` (strat_enemy.c:6496).
fn clship_warp_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    let pl = player(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sword1 > 0 {
            al.sword1 -= 1;
            if al.sword1 == 0 {
                // ASM clshipWARP_cont (GCSTRATS.ASM:143) jumps to clshipboost_Istrat
                // (:234 `trigse $32`) — the warp boost DOES play the sound. (Audit A #28)
                clshipboost_enter(g, idx, true);
                clshipboost_step(g, idx);
                return;
            }
        }
    }
    if let Some(pl) = pl {
        let al = &mut g.objs.aliens[idx as usize];
        // ASM GCSTRATS.ASM:148,152 `s_achase_alvar al_worldz/al_worldy` are the
        // PROPORTIONAL chase. (Audit A #8)
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 4);
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut rotz = al.rotz;
        achase_angle(&mut rotz, 0, 5);
        al.rotz = rotz;
    }
    add_player_z(g, idx);
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2);
}

/// C `clship_gnd_cont` (strat_enemy.c:6519).
fn clship_gnd_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    let pl = player(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sword1 > 0 {
            al.sword1 -= 1;
            if al.sword1 == 0 {
                clshipboost_enter(g, idx, true);
                clshipboost_step(g, idx);
                return;
            }
        }
    }
    if let Some(pl) = pl {
        let tick1 = frame_tick_mod(g, 1);
        let tick2 = frame_tick_mod(g, 2);
        let al = &mut g.objs.aliens[idx as usize];
        // ASM GCSTRATS.ASM:213,217 `s_achase_alvar al_worldz/al_worldy` are the
        // PROPORTIONAL chase. (Audit A #8)
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 3);
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 2);
        if tick1 {
            let mut rotz = al.rotz;
            achase_angle(&mut rotz, pl.rotz, 5);
            al.rotz = rotz;
        }
        if tick2 {
            let mut rotx = al.rotx;
            achase_angle(&mut rotx, pl.rotx, 5);
            al.rotx = rotx;
        }
    }
    add_player_z(g, idx);
}

fn clship_warpa_strat(g: &mut Game, idx: u16) {
    clship_flyinleft(g, idx);
    clship_warp_cont(g, idx, 100, -20);
}

fn clship_warpb_strat(g: &mut Game, idx: u16) {
    clship_flyinright(g, idx);
    clship_warp_cont(g, idx, 200, -20);
}

fn clship_warpc_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:132 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 4);
    }
    clship_warp_cont(g, idx, 300, -30);
}

fn clship_gnda_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:175 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, -50, 4);
    }
    clship_gnd_cont(g, idx, -200, 20);
}

fn clship_gndb_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:189 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 50, 4);
    }
    clship_gnd_cont(g, idx, -100, 40);
}

fn clship_gndc_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:202 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 4);
    }
    clship_gnd_cont(g, idx, -300, 50);
}

/// C `clship_common_init` (strat_enemy.c:6575).
fn clship_common_init(g: &mut Game, idx: u16, strat: StrategyFn) {
    let s = sid(g, strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_SHADOW;
    al.type_ &= !ATZREMOVE;
    al.stratptr = Some(s);
}

/// C `Strat_ClshipWARPA_Init` (strat_enemy.c:6581).
pub fn strat_clship_warpa_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_warpa_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = CLSHIP_WARP_BTIME - CLSHIP_BUNNYWAIT - 5;
    al.sbyte2 = 20;
    al.vx = 10;
    al.rotz = DEG90.wrapping_neg();
}

/// C `Strat_ClshipWARPB_Init` (strat_enemy.c:6589).
pub fn strat_clship_warpb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_warpb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = CLSHIP_WARP_BTIME - CLSHIP_FROGWAIT - 16;
    al.sbyte2 = 20;
    al.vx = -10;
    al.rotz = DEG90;
}

/// C `Strat_ClshipWARPC_Init` (strat_enemy.c:6597).
pub fn strat_clship_warpc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_warpc_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = CLSHIP_WARP_BTIME - CLSHIP_COCKWAIT - 27;
    al.sbyte2 = 20;
}

/// C `Strat_ClshipGNDA_Init` (strat_enemy.c:6603).
pub fn strat_clship_gnda_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_gnda_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90.wrapping_neg();
    al.sword1 = CLSHIP_GNDWAIT + 80 - CLSHIP_BUNNYWAIT;
}

/// C `Strat_ClshipGNDB_Init` (strat_enemy.c:6609).
pub fn strat_clship_gndb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_gndb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90;
    al.sword1 = CLSHIP_GNDWAIT + 90 - CLSHIP_FROGWAIT;
}

/// C `Strat_ClshipGNDC_Init` (strat_enemy.c:6615).
pub fn strat_clship_gndc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_gndc_strat);
    g.objs.aliens[idx as usize].sword1 = CLSHIP_GNDWAIT + 100 - CLSHIP_COCKWAIT;
}

/// C `s_clship_rotz_float` (strat_enemy.c:6620).
const CLSHIP_ROTZ_FLOAT: [i8; 15] = [0, 1, 2, 2, 1, 0, -1, -2, -2, -1, 0, 1, 2, 1, 0];

/// C `s_clship_view_float` (strat_enemy.c:6624).
const CLSHIP_VIEW_FLOAT: [i16; 7] = [0, 4, 7, 4, 0, -4, -7];

/// C `clship_float2` (strat_enemy.c:6628).
fn clship_float2(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte3 = al.sbyte3.wrapping_add(1) % 15;
    al.rotz = al.rotz.wrapping_add(CLSHIP_ROTZ_FLOAT[al.sbyte3 as usize] as u8);
    al.sbyte4 = al.sbyte4.wrapping_add(1) % 7;
    al.worldy = al
        .worldy
        .wrapping_add(CLSHIP_VIEW_FLOAT[al.sbyte4 as usize]);
}

/// C `clship_cont` (strat_enemy.c:6635).
fn clship_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    let pl = player(g);
    let pvz = pviewposz(g);
    {
        let me = g.objs.aliens[idx as usize];
        if ((me.worldz as i32 - pvz as i32).abs() as i16) >= 4000 {
            g.objs.aldead = 1;
            return;
        }
    }
    if g.objs.aliens[idx as usize].sflags2 & CLSHIP_FLAG1 != 0
        && pl.map(|p| p.sflags2 & 0x80 != 0).unwrap_or(false)
    {
        // ASM GCSTRATS.ASM:397 `s_decbne_alvar B,x,al_sbyte1,.nplayerchase`: while
        // sbyte1 counts down (result != 0) it branches PAST the normal chase to
        // `.nplayerchase` (only add_playerZ). Only when sbyte1 hits 0 does the
        // space-boost block run — but it ALSO ends at `.nplayerchase`. So whenever
        // this branch is taken the normal chase is skipped. (Audit A #27)
        let still_counting = {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte1 > 0 {
                al.sbyte1 -= 1;
            }
            al.sbyte1 != 0
        };
        if !still_counting {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = 1;
            al.worldz = al.worldz.wrapping_add(100);
            al.worldy = al.worldy.wrapping_sub(10);
            if al.sflags2 & CLSHIP_FLAG2 == 0 {
                al.sflags2 |= CLSHIP_FLAG2;
                al.snd2 = 0x32;
            }
        }
        add_player_z(g, idx);
        return;
    }
    if let Some(pl) = pl {
        let tick1 = frame_tick_mod(g, 1);
        let tick2 = frame_tick_mod(g, 2);
        let al = &mut g.objs.aliens[idx as usize];
        // ASM GCSTRATS.ASM:413,417 `s_achase_alvar al_worldz/al_worldy`
        // (proportional). (Audit A #8)
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_sub(zoff), 4);
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 4);
        if tick1 {
            let mut rotz = al.rotz;
            achase_angle(&mut rotz, pl.rotz, 4);
            al.rotz = rotz;
        }
        if tick2 {
            let mut rotx = al.rotx;
            achase_angle(&mut rotx, pl.rotx, 5);
            al.rotx = rotx;
        }
    }
    add_player_z(g, idx);
}

fn clship_eartha_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:271 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, -50, 4);
    }
    clship_float2(g, idx);
    clship_cont(g, idx, 100, 20);
}

fn clship_earthb_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:291 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 50, 4);
    }
    clship_float2(g, idx);
    clship_cont(g, idx, 200, 50);
}

fn clship_earthc_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:311 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 4);
    }
    clship_float2(g, idx);
    clship_cont(g, idx, 300, 50);
}

// NOTE (Audit A #9/#29): the ROM's `clshipCHASEboost_Istrat` (GCSTRATS.ASM:891)
// is DEAD CODE — nothing in GCSTRATS.ASM ever jumps to it. clshipCHASE_cont's
// timer expiry (:866 `s_beqdec_alvar W,x,al_sword1,clshipboost_Istrat`) goes to
// the GENERAL boost, not this chase-specific one. The old port's
// clship_chaseboost_enter/_step (speed 20, 2D vecs, self-rearming, extra $32
// sound the ASM lacks) are therefore removed.

/// C `clship_chase_cont` (strat_enemy.c:6723).
fn clship_chase_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    let pl = player(g);
    // ASM clshipCHASE_cont (GCSTRATS.ASM:866) `s_beqdec_alvar W,x,al_sword1,
    // clshipboost_Istrat`: TEST-then-DEC — on sword1==0 transition to the GENERAL
    // boost (`trigse $32`, speed 120, straight flyaway), not a chase-specific
    // one. (Audit A #9)
    if g.objs.aliens[idx as usize].sword1 == 0 {
        clshipboost_enter(g, idx, true);
        clshipboost_step(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    if let Some(pl) = pl {
        let tick1 = frame_tick_mod(g, 1);
        let al = &mut g.objs.aliens[idx as usize];
        // ASM GCSTRATS.ASM:871,875 `s_achase_alvar al_worldz/al_worldy`
        // (proportional). (Audit A #8)
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 5);
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
        if tick1 {
            let mut rotz = al.rotz;
            achase_angle(&mut rotz, pl.rotz, 5);
            al.rotz = rotz;
            let mut roty = al.roty;
            achase_angle(&mut roty, pl.roty, 4);
            al.roty = roty;
        }
    }
    add_player_z(g, idx);
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2);
}

fn clship_chasea_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:824 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, -70, 4);
    }
    clship_chase_cont(g, idx, -100, 20);
}

fn clship_chaseb_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:844 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 70, 4);
    }
    clship_chase_cont(g, idx, -200, 20);
}

fn clship_chasec_strat(g: &mut Game, idx: u16) {
    {
        // ASM GCSTRATS.ASM:858 `s_achase_alvar al_worldx` (proportional). (Audit A #8)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 4);
    }
    clship_chase_cont(g, idx, -300, 30);
}

/// C `Strat_ClshipEARTHA_Init` (strat_enemy.c:6764).
pub fn strat_clship_eartha_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_eartha_strat);
    let r1 = (ea_random(g) & 15) as u8;
    let r2 = (ea_random(g) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 10;
    al.rotz = DEG90.wrapping_neg();
    al.sbyte3 = r1;
    al.sbyte4 = r2;
}

/// C `Strat_ClshipEARTHB_Init` (strat_enemy.c:6773).
pub fn strat_clship_earthb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_earthb_strat);
    let r1 = (ea_random(g) & 15) as u8;
    let r2 = (ea_random(g) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 20;
    al.rotz = DEG90.wrapping_add(DEG45);
    al.sbyte3 = r1;
    al.sbyte4 = r2;
}

/// C `Strat_ClshipEARTHC_Init` (strat_enemy.c:6782).
pub fn strat_clship_earthc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_earthc_strat);
    let r1 = (ea_random(g) & 15) as u8;
    let r2 = (ea_random(g) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 30;
    al.sbyte3 = r1;
    al.sbyte4 = r2;
}

/// C `Strat_ClshipCHASEA_Init` (strat_enemy.c:6790).
pub fn strat_clship_chasea_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_chasea_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90.wrapping_neg();
    al.roty = DEG45;
    al.sword1 = 246 + 5 - CLSHIP_FROGWAIT;
}

/// C `Strat_ClshipCHASEB_Init` (strat_enemy.c:6797).
pub fn strat_clship_chaseb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_chaseb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90;
    al.roty = DEG45.wrapping_neg();
    al.sword1 = 246 + 10 - CLSHIP_BUNNYWAIT;
}

/// C `Strat_ClshipCHASEC_Init` (strat_enemy.c:6804).
pub fn strat_clship_chasec_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_chasec_strat);
    g.objs.aliens[idx as usize].sword1 = 246 + 15 - CLSHIP_COCKWAIT;
}

// ============================================================
// BOSS EXPLOSION STRATEGIES (EXPSTRAT.ASM; C strat_enemy.c:6815-7333)
// Shared with the enemy_b/bosses lanes (pub/pub(crate)).
// ============================================================

const BGM_BOSS_DYING: u8 = 0xF1;
const SE_BOSS_DYING: u8 = 0x1E;

// Explosion size markers (C EXPSHAPE_*).
pub const EXPSHAPE_SMALL: u16 = 1;
pub const EXPSHAPE_MEDIUM: u16 = 2;
pub const EXPSHAPE_LARGE: u16 = 3;
pub const EXPSHAPE_FOLARGE: u16 = 4;

/// C `addrnd2pos_xy` (strat_enemy.c:6832, addrnd2posy_srou).
pub(crate) fn addrnd2pos_xy(g: &mut Game, idx: u16) {
    let rx = (ea_random(g) & 0xFF) as u8 as i8;
    let ry = (ea_random(g) & 0xFF) as u8 as i8;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = al.worldx.wrapping_add(rx as i16);
    al.worldy = al.worldy.wrapping_add(ry as i16);
}

/// C `copy_pos` (strat_enemy.c:6850, s_copy_pos).
pub(crate) fn copy_pos(g: &mut Game, dst: u16, src: u16) {
    let s = g.objs.aliens[src as usize];
    let al = &mut g.objs.aliens[dst as usize];
    al.worldx = s.worldx;
    al.worldy = s.worldy;
    al.worldz = s.worldz;
}

/// C `make_exp_obj` (strat_enemy.c:6866, makeexpobj_srou).
pub(crate) fn make_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_obj(g, 0)?;
    let s_tick = sid(g, delayexplode_strat);
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

/// C `make_large_exp_obj` (strat_enemy.c:6903, makeLexpobj_srou).
pub(crate) fn make_large_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    g.objs.aliens[child as usize].shape = EXPSHAPE_LARGE;
    Some(child)
}

/// C `make_medium_exp_obj` (strat_enemy.c:6911, makeMEDexpobj_srou).
pub(crate) fn make_medium_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    g.objs.aliens[child as usize].shape = EXPSHAPE_MEDIUM;
    Some(child)
}

/// C `make_small_exp_obj` (strat_enemy.c:6919, makeSMLexpobj_srou).
pub(crate) fn make_small_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    g.objs.aliens[child as usize].shape = EXPSHAPE_SMALL;
    Some(child)
}

/// C `make_fol_exp_obj` (strat_enemy.c:6927, makeFOLexpobj_srou).
pub(crate) fn make_fol_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    g.objs.aliens[child as usize].shape = EXPSHAPE_FOLARGE;
    Some(child)
}

/// C `boss_dying` (strat_enemy.c:6940, s_boss_dying macro).
pub(crate) fn boss_dying(g: &mut Game) {
    if bossflags(g) & BF_DYING == 0 {
        g.hooks.play_se(SE_BOSS_DYING);
        g.hooks.play_music(BGM_BOSS_DYING);
        let bf = bossflags(g);
        set_bossflags(g, bf | BF_DYING);
        g.vars.pstratflags |= PSTF_NOTDIE;
        let sf = g.vars.read_ext8(wm::STRATFLAGS);
        g.vars.write_ext8(wm::STRATFLAGS, sf | SF_NOFIRING);
    }
}

/// C `delayexplode_strat` (EXPSTRAT.ASM:259-268).
pub(crate) fn delayexplode_strat(g: &mut Game, idx: u16) {
    // ASM EXPSTRAT.ASM:262 `s_decbpl_lifecnt x,.nd` dies when the decrement goes
    // NEGATIVE (entry count 0), surviving count+1 ticks. The old inline
    // `if count>0{--} if count==0{die}` fired one frame early. (Audit A #35)
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_HITFLASH;
        count_down(al)
    };
    if expired {
        g.objs.aldead = 1;
        if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
            g.call_strat(exp, idx);
        }
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
    }
}

/// C `delayremove_strat` (GSTRATS.ASM:1188-1193).
pub(crate) fn delayremove_strat(g: &mut Game, idx: u16) {
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
        add_player_z(g, idx);
    }
}

/// C `circdelayexplode_init` (EXPSTRAT.ASM:273-294 init half).
pub(crate) fn circdelayexplode_init(g: &mut Game, idx: u16) {
    let s = sid(g, circdelayexplode_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
}

/// C `circdelayexplode_strat` (EXPSTRAT.ASM:273-294 tick half).
pub(crate) fn circdelayexplode_strat(g: &mut Game, idx: u16) {
    // ASM EXPSTRAT.ASM:280 `s_decbpl_lifecnt x,.nd` dies when the decrement goes
    // NEGATIVE (entry count 0). Old inline fired one frame early. (Audit A #35)
    if count_down(&mut g.objs.aliens[idx as usize]) {
        // makebosscircexp_srou: circle-fill visual is renderer-side in HD;
        // play the explosion sound (C comment).
        g.hooks.play_se(0x1D);
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
            if let Some(big) = make_obj(g, 0) {
                copy_pos(g, big, idx);
                let s = sid(g, delayremove_strat);
                let al = &mut g.objs.aliens[big as usize];
                al.sflags |= ASF_COLLDISABLE;
                al.sflags2 |= ASF2_RELEXPLODE;
                al.flags |= AFEXP;
                al.count = 110;
                al.stratptr = Some(s);
                al.collstratptr = None;
                al.expstratptr = None;
            }
        }
        g.objs.aldead = 1;
        return;
    }
    add_player_z(g, idx);
}

/// C `bossdelayexplode_strat` (EXPSTRAT.ASM:46-65 tick half).
pub(crate) fn bossdelayexplode_strat(g: &mut Game, idx: u16) {
    // ASM EXPSTRAT.ASM:53 `s_decbpl_lifecnt x,.nd` dies when the decrement goes
    // NEGATIVE (entry count 0). Old inline fired one frame early. (Audit A #35)
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_HITFLASH;
        count_down(al)
    };
    if expired {
        g.objs.aldead = 1;
        let _ = make_fol_exp_obj(g, idx);
        g.vars.gameflags |= GF_BOSSDEAD;
        if let Some(exp) = g.objs.aliens[idx as usize].expstratptr {
            g.call_strat(exp, idx);
        }
        return;
    }
    add_player_z(g, idx);
    if let Some(temp) = g.objs.aliens[idx as usize].tempstratptr {
        g.call_strat(temp, idx);
    }
}

/// C `Strat_BossDelayExplode_Init` (Bossdelayexplode_Istrat).
pub fn strat_boss_delay_explode_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossdelayexplode_strat);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    set_hard_vars(al);
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = Some(s_exp);
    // Caller is responsible for setting `count` (lifecnt) — C comment.
}

/// C `Strat_QBossExplode_Init` (Qbossexplode_Istrat, EXPSTRAT.ASM:68-74).
pub fn strat_qboss_explode_init(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 0;
    }
    g.vars.gameflags |= GF_BOSSDEAD;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_RELEXPLODE;
        al.sflags2 |= ASF2_SFLAG1;
    }
    circdelayexplode_init(g, idx);
}

/// C `Strat_BossExplode_Init` (bossexplode_Istrat, EXPSTRAT.ASM:78-138):
/// staged multi-part boss explosion (timeline in the C comments).
pub fn strat_boss_explode_init(g: &mut Game, idx: u16) {
    boss_dying(g);

    // Timed explosion children: (factory, lifecnt) in C order.
    if let Some(child) = make_small_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        let al = &mut g.objs.aliens[child as usize];
        al.count = 5;
        al.sflags2 &= !ASF2_NOEXPSND; // s_clr_alsflag y,noexpsnd
    }
    if let Some(child) = make_small_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 10;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 15;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 17;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 19;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 22;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 24;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 26;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 28;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 29;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 32;
    }
    if let Some(child) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 32;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 34;
    }
    if let Some(child) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, child);
        g.objs.aliens[child as usize].count = 34;
    }

    // circdelayexplode proxy (s_make_obj #nullshape ...).
    if let Some(proxy) = make_obj(g, 0) {
        let me = g.objs.aliens[idx as usize];
        {
            let al = &mut g.objs.aliens[proxy as usize];
            al.sflags = me.sflags;
            al.sflags2 = me.sflags2;
            al.sflags3 = me.sflags3;
            al.sflags4 = me.sflags4;
            al.sflags3 &= !ASF3_REALOBJ;
            al.sflags |= ASF_COLLDISABLE;
            al.sflags2 |= ASF2_NOEXPSND;
        }
        copy_pos(g, proxy, idx);
        let al = &mut g.objs.aliens[proxy as usize];
        al.count = 15;
        al.sflags2 |= ASF2_SFLAG1;
        circdelayexplode_init(g, proxy);
    }

    g.objs.aliens[idx as usize].count = 38;
    strat_boss_delay_explode_init(g, idx);
}


// ============================================================
// Table-lane registration (mirrors player.rs `install`).
// ============================================================

/// Registry handles for every istrat entry point owned by this lane; the
/// table lane wires them onto the literal ISTRATS.ASM def_Istrat indices
/// (C `Strat_RegisterAll`, strat_table.c). Field order follows the C file.
pub struct EnemyAStratIds {
    /// `hard_Istrat` (`Strat_Hard_Init`).
    pub hard: StratId,
    /// `hard180yr_Istrat` (`Strat_Hard180yr_Init`).
    pub hard180yr: StratId,
    /// `hard90yr_Istrat` (`Strat_Hard90yr_Init`).
    pub hard90yr: StratId,
    /// `hard180yrNZR_Istrat` (`Strat_Hard180yrNZR_Init`).
    pub hard180yr_nzr: StratId,
    /// `hardrot_Istrat` (`Strat_HardRot_Init`).
    pub hardrot: StratId,
    /// `nocoll_Istrat` (`Strat_NoColl_Init`).
    pub nocoll: StratId,
    /// `rader0_Istrat` (`Strat_Rader0_Init`).
    pub rader0: StratId,
    /// `rader1_Istrat` (`Strat_Rader1_Init`).
    pub rader1: StratId,
    /// `pillar3_Istrat` (`Strat_Pillar3_Init`).
    pub pillar3: StratId,
    /// `skillfly_Istrat` (`Strat_Skillfly_Init`).
    pub skillfly: StratId,
    /// `gate3_Istrat` (`Strat_Gate3_Init`).
    pub gate3: StratId,
    /// `gate_Istrat` (`Strat_Gate_Init`).
    pub gate: StratId,
    /// `gate2_Istrat` (`Strat_Gate2_Init`).
    pub gate2: StratId,
    /// `boss1_Istrat` (`Strat_Boss1_Init`).
    pub boss1: StratId,
    /// `tow0explode` entry (`Strat_Tow0Explode`).
    pub tow0_explode: StratId,
    /// `wormhead_Istrat` (`Strat_Wormhead_Init`).
    pub wormhead: StratId,
    /// `worm_Istrat` (`Strat_Worm_Init`).
    pub worm: StratId,
    /// `worm2_Istrat` (`Strat_Worm2_Init`).
    pub worm2: StratId,
    /// `item5_Istrat` (`Strat_Item5_Init`).
    pub item5: StratId,
    /// `item7_Istrat` (`Strat_Item7_Init`).
    pub item7: StratId,
    /// `bomwing_Istrat` (`Strat_Bomwing_Init`).
    pub bomwing: StratId,
    /// `tadpole_Istrat` (`Strat_Tadpole_Init`).
    pub tadpole: StratId,
    /// `spacebarwalker_Istrat` (`Strat_Spacebarwalker_Init`).
    pub spacebarwalker: StratId,
    /// `spacebarshoot_Istrat` (`Strat_Spacebarshoot_Init`).
    pub spacebarshoot: StratId,
    /// `up1man_Istrat` (`Strat_Up1man_Init`).
    pub up1man: StratId,
    /// `zacos_Istrat` (`Strat_Zacos_Init`).
    pub zacos: StratId,
    /// `tower0_Istrat` (`Strat_Tower0_Init`).
    pub tower0: StratId,
    /// `houdaiNS_Istrat` (`Strat_HoudaiNS_Init`).
    pub houdai_ns: StratId,
    /// `houdai_Istrat` (`Strat_Houdai_Init`).
    pub houdai: StratId,
    /// `zaco3_Istrat` (`Strat_Zaco3_Init`).
    pub zaco3: StratId,
    /// `zaco4_Istrat` (`Strat_Zaco4_Init`).
    pub zaco4: StratId,
    /// `zaco0_Istrat` (`Strat_Zaco0_Init`).
    pub zaco0: StratId,
    /// `para_Istrat` (`Strat_Para_Init`).
    pub para: StratId,
    /// `carrier_Istrat` (`Strat_Carrier_Init`).
    pub carrier: StratId,
    /// `base1_Istrat` (`Strat_Base1_Init`).
    pub base1: StratId,
    /// `cameleon_Istrat` (`Strat_Cameleon_Init`).
    pub cameleon: StratId,
    /// `szaco2_Istrat` (`Strat_Szaco2_Init`).
    pub szaco2: StratId,
    /// `zaco1L_Istrat` (`Strat_Zaco1L_Init`).
    pub zaco1l: StratId,
    /// `zaco1R_Istrat` (`Strat_Zaco1R_Init`).
    pub zaco1r: StratId,
    /// `friendexitbase_Istrat` (`Strat_FriendExitBase_Init`).
    pub friendexitbase: StratId,
    /// `clshipWARPA_Istrat`.
    pub clship_warpa: StratId,
    /// `clshipWARPB_Istrat`.
    pub clship_warpb: StratId,
    /// `clshipWARPC_Istrat`.
    pub clship_warpc: StratId,
    /// `clshipGNDA_Istrat`.
    pub clship_gnda: StratId,
    /// `clshipGNDB_Istrat`.
    pub clship_gndb: StratId,
    /// `clshipGNDC_Istrat`.
    pub clship_gndc: StratId,
    /// `clshipEARTHA_Istrat`.
    pub clship_eartha: StratId,
    /// `clshipEARTHB_Istrat`.
    pub clship_earthb: StratId,
    /// `clshipEARTHC_Istrat`.
    pub clship_earthc: StratId,
    /// `clshipCHASEA_Istrat`.
    pub clship_chasea: StratId,
    /// `clshipCHASEB_Istrat`.
    pub clship_chaseb: StratId,
    /// `clshipCHASEC_Istrat`.
    pub clship_chasec: StratId,
    /// `bossdelayexplode_Istrat` (`Strat_BossDelayExplode_Init`).
    pub boss_delay_explode: StratId,
    /// `qbossexplode_Istrat` (`Strat_QBossExplode_Init`).
    pub qboss_explode: StratId,
    /// `bossexplode_Istrat` (`Strat_BossExplode_Init`).
    pub boss_explode: StratId,
    /// `hitflash` collision strategy (`Strat_HitFlash`) — istrat rows
    /// install it as the default collstrat.
    pub hit_flash: StratId,
    /// `explode_Istrat` (`Strat_Explode`).
    pub explode: StratId,
}

/// Register this lane's strategy entry points (idempotent — [`sid`]
/// memoizes on function identity) and return the public handles.
pub fn install(g: &mut Game) -> EnemyAStratIds {
    EnemyAStratIds {
        hard: sid(g, strat_hard_init),
        hard180yr: sid(g, strat_hard180yr_init),
        hard90yr: sid(g, strat_hard90yr_init),
        hard180yr_nzr: sid(g, strat_hard180yr_nzr_init),
        hardrot: sid(g, strat_hardrot_init),
        nocoll: sid(g, strat_nocoll_init),
        rader0: sid(g, strat_rader0_init),
        rader1: sid(g, strat_rader1_init),
        pillar3: sid(g, strat_pillar3_init),
        skillfly: sid(g, strat_skillfly_init),
        gate3: sid(g, strat_gate3_init),
        gate: sid(g, strat_gate_init),
        gate2: sid(g, strat_gate2_init),
        boss1: sid(g, strat_boss1_init),
        tow0_explode: sid(g, strat_tow0_explode),
        wormhead: sid(g, strat_wormhead_init),
        worm: sid(g, strat_worm_init),
        worm2: sid(g, strat_worm2_init),
        item5: sid(g, strat_item5_init),
        item7: sid(g, strat_item7_init),
        bomwing: sid(g, strat_bomwing_init),
        tadpole: sid(g, strat_tadpole_init),
        spacebarwalker: sid(g, strat_spacebarwalker_init),
        spacebarshoot: sid(g, strat_spacebarshoot_init),
        up1man: sid(g, strat_up1man_init),
        zacos: sid(g, strat_zacos_init),
        tower0: sid(g, strat_tower0_init),
        houdai_ns: sid(g, strat_houdai_ns_init),
        houdai: sid(g, strat_houdai_init),
        zaco3: sid(g, strat_zaco3_init),
        zaco4: sid(g, strat_zaco4_init),
        zaco0: sid(g, strat_zaco0_init),
        para: sid(g, strat_para_init),
        carrier: sid(g, strat_carrier_init),
        base1: sid(g, strat_base1_init),
        cameleon: sid(g, strat_cameleon_init),
        szaco2: sid(g, strat_szaco2_init),
        zaco1l: sid(g, strat_zaco1l_init),
        zaco1r: sid(g, strat_zaco1r_init),
        friendexitbase: sid(g, strat_friendexitbase_init),
        clship_warpa: sid(g, strat_clship_warpa_init),
        clship_warpb: sid(g, strat_clship_warpb_init),
        clship_warpc: sid(g, strat_clship_warpc_init),
        clship_gnda: sid(g, strat_clship_gnda_init),
        clship_gndb: sid(g, strat_clship_gndb_init),
        clship_gndc: sid(g, strat_clship_gndc_init),
        clship_eartha: sid(g, strat_clship_eartha_init),
        clship_earthb: sid(g, strat_clship_earthb_init),
        clship_earthc: sid(g, strat_clship_earthc_init),
        clship_chasea: sid(g, strat_clship_chasea_init),
        clship_chaseb: sid(g, strat_clship_chaseb_init),
        clship_chasec: sid(g, strat_clship_chasec_init),
        boss_delay_explode: sid(g, strat_boss_delay_explode_init),
        qboss_explode: sid(g, strat_qboss_explode_init),
        boss_explode: sid(g, strat_boss_explode_init),
        hit_flash: sid(g, strat_hit_flash),
        explode: sid(g, strat_explode),
    }
}
