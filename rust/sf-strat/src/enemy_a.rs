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

use sf_core::player_view::PlayerViewMode;
use sf_core::screen_fill_circle::ScreenFillCircleCenter;
use sf_core::sf1_shape_metrics::sf1_shape_metrics;
use sf_game::alien::{
    Alien, ExplosionSize, ObjectVisualKind, StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3,
    ACF_COLLTYPE4, ACF_COLLTYPE5, ACF_FIRSTFRAME, ACF_WEAPON, AFEXP, AFONFIRE, ASF2_COLLDISABLE,
    ASF3_NOHITAFFECT, ASF3_REALOBJ, ASF4_CSPECIAL, ASF4_INVISIBLE, ASF4_SFLAG8, ASF_COLLDISABLE,
    ASF_COLLIDE, ASF_HITFLASH, ASF_NOHITAFFECT, ASF_PARTOBJ, ASF_SHADOW, ASF_SPECIAL, ASF_SSPRITE,
    ATGND, ATLASER, ATMISSILE, ATNUKED, ATZREMOVE, NUMBER_AL,
};
use sf_game::coldet::PCBOX_WING_HP;
use sf_game::game::{Game, PosSndFamilyId, StrategyFn};
use sf_game::vars::{
    GF_BOSSDEAD, GF_STRATDONE1, GF_STRATDONE2, HARD_AP, HARD_HP, PFM_SHADOWS, PSF2_PLAYERHP0,
    PSF3_ENGINESND, PSF3_INTUNNEL, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, PSTF_NOTDIE,
};
use sf_map::consts::sh;

// ============================================================
// Angle constants (C src/variables.h DEG*)
// ============================================================
pub const DEG0: u8 = 0;
pub const DEG5: u8 = 4;
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
pub const ASF2_SFLAG2: u8 = 0x20;
pub const ASF2_SFLAG3: u8 = 0x40;
/// ROM `sflag5` / `sflag6` / `sflag7` live in `al_sflags3` bits 0/1/2
/// (STRATEQU.INC make_sflag packing after sflag4).
pub const ASF3_SFLAG5: u8 = 0x01;
pub const ASF3_SFLAG6: u8 = 0x02;
pub const ASF3_SFLAG7: u8 = 0x04;
/// STRATMAC `smflag1` — the strategy-macro latch used by `s_face_player` /
/// `s_initface_player`. Per STRATEQU.INC:910 it is sflags2 bit 0x04 (the same
/// bit as the mislabeled ASF2_RELEXPLODE above; ROM `relexplode` is really a
/// sflags4 bit, so nothing that shares an object with para's smflag1 also uses
/// relexplode). (Audit A #20/#21)
pub const ASF2_SMFLAG1: u8 = 0x04;

// al_sflags3 bits — re-export from sf-game (single source of truth).
pub use sf_game::alien::{ASF4_CHILDOBJ, ASF4_MOTHEROBJ};

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
// Encoded source-variable operands retained for oracle and integration-test
// fixtures. Native strategy code uses the typed `GameVars` fields directly;
// these values are not runtime storage locations.
// ============================================================
pub mod wm {
    /// C `RAM8(WM_SKILLFLY)` (real address, src/strat/strat_enemy.c).
    pub const SKILLFLY: u16 = 0x0304;

    /// C `g_rndval` (u16, src/sf_rtl.c).
    pub const RNDVAL: u16 = 0x1F00;
    /// C `g_bossflags` (u8).
    pub const BOSSFLAGS: u16 = 0x1F02;
    /// Strat difficulty byte (port encoding = ROM `currentlevel` + 1).
    /// Shell writes this from `Planets.currentlevel` on gameplay start so
    /// `s_jmp_iflevel N` ports as `currentlevel() == N`.
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
    /// C `g_maprestart` (u16).
    pub const MAPRESTART: u16 = 0x1F0E;
    /// C `g_maprestarttemp` (u16).
    pub const MAPRESTARTTEMP: u16 = 0x1F10;
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
    /// C `g_specflash` (u8) — nova-bomb HUD flash timer (SPRITES.ASM do_spec_weap).
    /// item5_collect sets #30; shell decrements each Playing tick and surfaces
    /// via FrameSnapshot.specflash for the HUD blink.
    pub const SPECFLASH: u16 = 0x1F28;
    /// ROM `powerbuild` (DSTRATS.ASM ironball2 / castanet scratch) — u8.
    pub const POWERBUILD: u16 = 0x1F29;
    /// ROM `locusmode` (DSTRATS.ASM castanet ring-laser pattern) — u8.
    pub const LOCUSMODE: u16 = 0x1F2A;
    // g_numplasers: consolidated onto `crate::common::sv::NUMPLASERS`
    // (0x0313, the real WM address) — shared with the player-lane laser
    // counter exactly like the single C global.
}

#[inline]
pub fn bossflags(g: &Game) -> u8 {
    g.vars.shared.boss_flags
}
#[inline]
pub fn set_bossflags(g: &mut Game, v: u8) {
    g.vars.shared.boss_flags = v;
}
#[inline]
pub fn gasflags(g: &Game) -> u8 {
    g.vars.shared.gas_flags
}
#[inline]
pub fn set_gasflags(g: &mut Game, v: u8) {
    g.vars.shared.gas_flags = v;
}
#[inline]
pub fn currentlevel(g: &Game) -> u8 {
    g.vars.shared.difficulty_level
}
#[inline]
pub fn pviewposz(g: &Game) -> i16 {
    g.vars.strategy.player_view_position[2]
}

/// C `SfRtl_Random()` (src/sf_rtl.c:192): `g_rndval = rnd*91 + 0x61D7`.
/// The RNG state lives in the compat WRAM slot so the C harness and tests
/// can seed it identically.
pub fn ea_random(g: &mut Game) -> u16 {
    let v = g.vars.strategy.random_seed;
    let n = v.wrapping_mul(91).wrapping_add(0x61D7);
    g.vars.strategy.random_seed = n;
    n
}

// ============================================================
// strat_common.c helpers — CONSOLIDATED onto `crate::common`.
//
// The former `ea_compat` duplicate module is gone; every call site now
// goes through the canonical common-lane ports (signatures and byte
// behavior verified against src/strat/strat_common.c). Notes:
// - `speed_to`: the old duplicate used `saturating_add`; C (and
//   `crate::common::strat_speed_to`) wrap `vel += rate` at u8 — the
//   canonical version is the C-faithful one.
// - `angle_xz`: now `sf_core::aim_angle::yanglexy` with i16 wrapping
//   deltas (ROM `Yanglexy_l` SBC). The old local twin used i32 promotion
//   (C float cast); that diverged from ROM once |diff| > 32767.
// ============================================================

/// Registry-shaped projectile collide strategy (C takes the address of
/// `Strat_ProjectileOnCollide`); same slot ids as the common lane's.
pub(crate) use crate::common::strat_projectile_on_collide as projectile_on_collide_strat;
pub(crate) use crate::common::{
    apply_velocity, chase_proportional, count_down, damage_smoke_srou, dist_xz, gen_vecs_3d,
    make_obj, sf_random, spawn_projectile, speed_to, strat_gen_vecs_nvecs, SmokeCadence, StratRam,
};

/// ROM `Yanglexy_l` / `anglexy_l`: 0-255 yaw from src→dst (i16 wrapping deltas).
#[inline]
fn angle_xz(src: &Alien, dst: &Alien) -> u8 {
    sf_core::aim_angle::yanglexy(
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
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
    sf_core::snes_trig::achase_angle_8(current, target, shift)
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

/// KSTRATS.ASM `fogdist`: fog-authored objects keep their source mesh only
/// while they are nearer than this Z distance to the player.
pub(crate) const FOG_VISIBILITY_DISTANCE: i16 = 2000;

/// KSTRATS.ASM `s_initfog`: retain the object's ordinary flat shape identity
/// in the source-owned `sword1` field.
pub(crate) fn init_fog_visibility(al: &mut Alien) {
    al.sword1 = al.shape as i16;
}

/// KSTRATS.ASM `s_dofog`: when the current map enables fog, hide objects at
/// the inclusive distance boundary and restore their retained shape inside
/// it. Disabling fog leaves the current shape untouched, matching the macro's
/// early exit.
pub(crate) fn update_fog_visibility(g: &mut Game, idx: u16) {
    if g.vars.map.in_fog == 0 {
        return;
    }
    let Some(player) = player(g) else {
        return;
    };
    let object = g.objs.aliens[idx as usize];
    let distance = player.worldz.wrapping_sub(object.worldz) as i32;
    g.objs.aliens[idx as usize].shape = if distance.abs() >= i32::from(FOG_VISIBILITY_DISTANCE) {
        0
    } else {
        object.sword1 as u16
    };
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
    let rnd = (sf_random(&mut g.vars) % span as u16) as u8;
    (rnd as i8).wrapping_sub((span / 2) as i8)
}

/// C `strat_tab_scaled` / ROM `s_set_alvar2alvartab … sintab/costab,scale`:
/// table byte then `<< scale` or toward-zero `/ 2^|scale|` (adiv2 × N).
pub fn strat_tab_scaled(angle: u8, use_sin: bool, shift: i32) -> i16 {
    use crate::snes_trig::{COSTAB, SINTAB};
    let value = if use_sin {
        SINTAB[angle as usize] as i16
    } else {
        COSTAB[angle as usize] as i16
    };
    if shift < 0 {
        value / (1i16 << (-shift))
    } else if shift > 0 {
        value << shift
    } else {
        value
    }
}

/// C `frame_tick_mod` — the `s_jmp_notdelay N` gate (STRATMAC.INC:6456-6468):
/// fires when `gameframe & ((1<<N)-1) == 0`, i.e. period 2^N. `step` is the
/// macro's BIT COUNT `N`, NOT a modulus. (Audit A #2)
pub fn frame_tick_mod(g: &Game, step: u16) -> bool {
    g.vars.gameframe & ((1u16 << step) - 1) == 0
}

/// C `strat_phase_offset` (strat_enemy.c:4574): per-alien phase stagger.
const OBJECT_UPDATE_PHASE_SEED: u8 = 54;
const OBJECT_UPDATE_PHASE_STEP: u8 = 54;

pub(crate) fn strat_phase_offset(idx: u16) -> u8 {
    if (idx as usize) < NUMBER_AL {
        // Retail schedules staggered strategy work from the low byte of each
        // object's source-pool identity. Preserve only the resulting phase in
        // the flat port: the first object has phase 54 and each following
        // object advances by the retail record width of 54.
        OBJECT_UPDATE_PHASE_SEED.wrapping_add((idx as u8).wrapping_mul(OBJECT_UPDATE_PHASE_STEP))
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

/// ROM `Xanglexy_l` elevation (was C hypot; adjacent is `xzdiffs_l`).
pub(crate) fn strat_pitch_toward(src: &Alien, dst: &Alien) -> u8 {
    sf_core::aim_angle::xanglexy(
        dst.worldy.wrapping_sub(src.worldy),
        dst.worldx.wrapping_sub(src.worldx),
        dst.worldz.wrapping_sub(src.worldz),
    )
}

/// C `strat_aim_yaw` (strat_enemy.c:4389).
pub(crate) fn strat_aim_yaw(g: &mut Game, idx: u16, target: &Alien, shift: u32) {
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    // ROM `s_obj2obj_angle` / Yanglexy path also `nega`s before Achase.
    achase_angle(&mut roty, angle_xz(&me, target).wrapping_neg(), shift);
    g.objs.aliens[idx as usize].roty = roty;
}

/// C `strat_aim_3d` (strat_enemy.c:4396).
pub(crate) fn strat_aim_3d(g: &mut Game, idx: u16, target: &Alien, shift: u32) {
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    // ROM `s_obj2obj_3Dangle` (STRATMAC.INC:2374-2377): Yanglexy then `nega`
    // before Achase into al_roty. `gen_vecs_3d` also negates yaw when indexing
    // sintab — storing -Yanglexy is required to fly toward the target.
    // (Audit A Minor 15 / angle_xz VERIFIED tick 163)
    achase_angle(&mut roty, angle_xz(&me, target).wrapping_neg(), shift);
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

/// Local Z muzzle for `fire_relslowElaser`/`Home` after gen_weapon's
/// `<<weapon_scale(2)`: `elaserfireZoff(80)>>2` then ASL×2 → world ~80
/// (GSTRATS.ASM:2557 / :2572 + :2795). Rotated by the firer's full rots.
const RELSLOWELASER_MUZZLE_Z: i16 = 80;
const RELSLOWELASER_Z_BYTE: i8 = 80 >> 2;
const WEAPON_SCALE: u32 = 2;
const PLAYER_OBJECT_SLOT: u16 = 0;
const SHAPE_ENEMY_LASER: u16 = 478;
const SHAPE_LARGE_LASER_FLASH: u16 = 479;
const SHAPE_MEDIUM_LASER_FLASH: u16 = 480;
const SHAPE_SMALL_LASER_FLASH: u16 = 481;
const SMALL_LASER_FLASH_DISTANCE: i16 = 500;
const MEDIUM_LASER_FLASH_DISTANCE: i16 = 1_000;

/// ROM `laserflash` (GSTRATS.ASM:2845): add a distance-scaled flash directly
/// after the newly created bolt. Both objects then initialize later in the
/// current source-order strategy pass.
fn make_laser_flash(g: &mut Game, shot: u16) -> Option<u16> {
    let shape = g.objs.player().map_or(SHAPE_LARGE_LASER_FLASH, |player| {
        let depth = (g.objs.aliens[shot as usize].worldz as i32 - player.worldz as i32).abs();
        if depth < i32::from(SMALL_LASER_FLASH_DISTANCE) {
            SHAPE_SMALL_LASER_FLASH
        } else if depth < i32::from(MEDIUM_LASER_FLASH_DISTANCE) {
            SHAPE_MEDIUM_LASER_FLASH
        } else {
            SHAPE_LARGE_LASER_FLASH
        }
    });
    let flash = make_obj(g, shape)?;
    g.objs.active_move_after(flash, shot);
    let shot_position = {
        let bolt = g.objs.aliens[shot as usize];
        (bolt.worldx, bolt.worldy, bolt.worldz)
    };
    let init = sid(g, crate::common::flash_istrat);
    let effect = &mut g.objs.aliens[flash as usize];
    effect.worldx = shot_position.0;
    effect.worldy = shot_position.1;
    effect.worldz = shot_position.2;
    effect.roty = DEG180;
    effect.stratptr = Some(init);
    Some(flash)
}

/// Exact ROM `fire_relslowElaser` constructor with the caller's current
/// `s_weapon_pos` scratch bytes. The weapon routine adds its own Z=20 byte,
/// rotates the combined byte vector by the firer's full rotations, then
/// applies `weapon_scale`. This form is shared with King Joh's offset twin
/// muzzles in enemy_b.
pub(crate) fn fire_relslowlaser_weapon_pos(
    g: &mut Game,
    idx: u16,
    pitch: u8,
    yaw: u8,
    offx: i8,
    offy: i8,
    offz: i8,
) -> Option<u16> {
    let speed = strat_relslowelaser_speed(g);
    let shot = make_obj(g, SHAPE_ENEMY_LASER)?;
    let me = g.objs.aliens[idx as usize];
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        me.rotz,
        me.rotx,
        me.roty,
        offx,
        offy,
        offz.wrapping_add(RELSLOWELASER_Z_BYTE),
        WEAPON_SCALE,
    );
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = me.worldx.wrapping_add(rx);
        al.worldy = me.worldy.wrapping_add(ry);
        al.worldz = me.worldz.wrapping_add(rz);
        al.rotx = pitch;
        al.roty = yaw;
        al.rotz = 0;
        al.sbyte1 = pitch;
        al.sbyte2 = yaw;
        al.sbyte3 = me.vel;
        al.hp = 1;
        al.ap = ENEMYLASER_AP;
        al.vel = speed;
        al.count = 40;
        // `gen_weapon` replaces the default behind-camera-removal class with
        // the source weapon class. Laser presentation comes from the shape;
        // this field controls object lifecycle and weapon-family tests.
        al.type_ = ATMISSILE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collflags |= ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4 | ACF_COLLTYPE1;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.immuneptr = idx;
    }
    g.objs.active_move_after(shot, idx);
    g.objs.aliens[idx as usize].immuneptr = shot;
    let init = sid(g, relelaser_istrat);
    g.objs.aliens[shot as usize].stratptr = Some(init);
    make_firer_snd(g, idx, PosSndFamilyId::Laser);
    let _ = make_laser_flash(g, shot);
    Some(shot)
}

/// Exact ROM `fire_relfastElaser` constructor (GSTRATS.ASM:2578). It shares
/// the relative-laser strategy and full-rotation muzzle transform with the
/// slow variant, but uses fixed speed 90 and a Z muzzle contribution of 20
/// (`80 >> weapon_scale`).
pub(crate) fn fire_relfastelaser_weapon_pos(
    g: &mut Game,
    idx: u16,
    pitch: u8,
    yaw: u8,
    offx: i8,
    offy: i8,
    offz: i8,
) -> Option<u16> {
    let shot = make_obj(g, SHAPE_ENEMY_LASER)?;
    let me = g.objs.aliens[idx as usize];
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        me.rotz,
        me.rotx,
        me.roty,
        offx,
        offy,
        offz.wrapping_add(80 >> WEAPON_SCALE),
        WEAPON_SCALE,
    );
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = me.worldx.wrapping_add(rx);
        al.worldy = me.worldy.wrapping_add(ry);
        al.worldz = me.worldz.wrapping_add(rz);
        al.rotx = pitch;
        al.roty = yaw;
        al.rotz = 0;
        al.sbyte1 = pitch;
        al.sbyte2 = yaw;
        al.sbyte3 = me.vel;
        al.hp = 1;
        al.ap = ENEMYLASER_AP;
        al.vel = 90;
        al.count = 40;
        al.type_ = ATMISSILE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collflags |= ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4 | ACF_COLLTYPE1;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.immuneptr = idx;
    }
    g.objs.active_move_after(shot, idx);
    g.objs.aliens[idx as usize].immuneptr = shot;
    let init = sid(g, relelaser_istrat);
    g.objs.aliens[shot as usize].stratptr = Some(init);
    make_firer_snd(g, idx, PosSndFamilyId::Laser);
    let _ = make_laser_flash(g, shot);
    Some(shot)
}

/// C `strat_fire_relslowlaser` — ASM `fire_relslowElaser` (GSTRATS.ASM:2548-2561):
/// speed via `doelaserspeed` (48 at level 1, else 60, GSTRATS.ASM:2780),
/// `s_set_lifecnt #40`, `enemylaserAP=2`, colltypes `enemyweap`+`laser`, and
/// muzzle `elaserfireZoff>>weapon_scale` rotated by firer. (Audit A #1, Minor 1)
pub fn strat_fire_relslowlaser(g: &mut Game, idx: u16, pitch: u8, yaw: u8) -> Option<u16> {
    fire_relslowlaser_weapon_pos(g, idx, pitch, yaw, 0, 0, 0)
}

/// zacos2/3 fire: `s_weapon_pos #0,#0,#40>>weapon_scale` then RELSLOWELASER
/// (GASTRATS.ASM:967,991) → muzzle world Z 120. (Audit A Minor 6)
fn zacos_fire_relslowlaser(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    // s_weapon_pos #0,#0,#40>>weapon_scale stores 10; the weapon adds 20.
    let _ = fire_relslowlaser_weapon_pos(g, idx, pitch, yaw, 0, 0, 10);
}

/// C `strat_relslowelaser_speed` (strat_enemy.c:4428).
pub fn strat_relslowelaser_speed(g: &Game) -> u8 {
    if currentlevel(g) == 1 {
        48
    } else {
        60
    }
}

/// C `strat_fire_relslowlaserhome` — ASM `fire_relslowElaserHome`
/// (GSTRATS.ASM:2563-2576): same colltypes + `#80>>weapon_scale` muzzle as
/// the non-home helper. (Audit A Minor 1)
pub fn strat_fire_relslowlaserhome(g: &mut Game, idx: u16, pitch: u8, yaw: u8) -> Option<u16> {
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
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    ) else {
        return None;
    };
    g.objs.active_move_after(shot, idx);
    let me = g.objs.aliens[idx as usize];
    let z_byte = (RELSLOWELASER_MUZZLE_Z >> WEAPON_SCALE) as i8;
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        me.rotz,
        me.rotx,
        me.roty,
        0,
        0,
        z_byte,
        WEAPON_SCALE,
    );
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = me.worldx.wrapping_add(rx);
        al.worldy = me.worldy.wrapping_add(ry);
        al.worldz = me.worldz.wrapping_add(rz);
    }
    let init = sid(g, relelaserhome_istrat);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.shape = SHAPE_ENEMY_LASER;
        al.type_ = ATMISSILE;
        al.stratptr = Some(init);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sflags2 |= ASF2_RELEXPLODE;
        al.rotx = pitch;
        al.roty = yaw;
        al.sbyte1 = pitch;
        al.sbyte2 = yaw;
        al.animframe = 0;
    }
    g.objs.aliens[idx as usize].immuneptr = shot;
    // ROM `jsl lasersound_l` (GSTRATS.ASM:2574).
    make_firer_snd(g, idx, PosSndFamilyId::Laser);
    let _ = make_laser_flash(g, shot);
    Some(shot)
}

/// Fire the homing enemy bolt using the source routine's object-target mode.
/// The authored initializer measures from the completed muzzle position, uses
/// its Manhattan pitch approximation, and stores an absolute target heading.
fn fire_relslowlaserhome_at_target(
    g: &mut Game,
    idx: u16,
    target: u16,
    pitch_offset: u8,
    yaw_offset: u8,
) -> Option<u16> {
    let shot = strat_fire_relslowlaserhome(g, idx, pitch_offset, yaw_offset)?;
    let target = g.objs.aliens[target as usize];
    let projectile = &mut g.objs.aliens[shot as usize];
    let dx = target.worldx.wrapping_sub(projectile.worldx);
    let dy = target.worldy.wrapping_sub(projectile.worldy);
    let dz = target.worldz.wrapping_sub(projectile.worldz);
    projectile.rotx = sf_core::aim_angle::xanglexabs(dy, dx, dz).wrapping_add(pitch_offset);
    projectile.roty = sf_core::aim_angle::yanglexy_nega(dx, dz).wrapping_add(yaw_offset);
    projectile.sbyte1 = projectile.rotx;
    projectile.sbyte2 = projectile.roty;
    Some(shot)
}

/// Positional one-shot SE from a firer's world XZ (`makesnd` / `*sound_l`).
#[inline]
fn make_firer_snd(g: &mut Game, firer: u16, family: PosSndFamilyId) {
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(family, fx, fz);
}

/// C `strat_find_near_shape` (strat_enemy.c:4315) — nearest active alien
/// with the given (raw or mapped) shape, walking the active list in order.
///
/// Ranking/gate match ROM `find_nearobject_l` (STRATROU.ASM:697):
/// `rangexz = xzdiffs_l` ([`crate::common::strat_dist_xz`]), keep if
/// `0 <= rangexz < max_r`. `max_z` is the max radius (callers historically
/// passed a Z/XY box; isotropic sites use equal args). `max_xy` is unused
/// for the radius band (kept for call-site compatibility).
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
    let max_r = max_z;
    let _ = max_xy;
    let mut best: Option<u16> = None;
    let mut best_r = max_r;
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
        let r = crate::common::strat_dist_xz(&me, al);
        // Tighten the best range only when this candidate is in bounds.
        if r >= best_r || r < 0 {
            continue;
        }
        best_r = r;
        best = Some(it);
    }
    best
}

/// C `strat_find_near_colltype` (strat_enemy.c:4993).
/// Same XZ `xzdiffs_l` ranking as [`strat_find_near_shape`].
pub(crate) fn strat_find_near_colltype(
    g: &Game,
    self_idx: u16,
    colltype_mask: u8,
    max_z: i16,
    max_xy: i16,
) -> Option<u16> {
    let me = g.objs.aliens[self_idx as usize];
    let max_r = max_z;
    let _ = max_xy;
    let mut best: Option<u16> = None;
    let mut best_r = max_r;
    for it in g.objs.active_indices() {
        if it == self_idx {
            continue;
        }
        let al = &g.objs.aliens[it as usize];
        if !al.active || al.collflags & colltype_mask == 0 {
            continue;
        }
        let r = crate::common::strat_dist_xz(&me, al);
        if r >= best_r || r < 0 {
            continue;
        }
        best_r = r;
        best = Some(it);
    }
    best
}

// ============================================================
// Per-strategy tuning constants (C strat_enemy.c defines)
// ============================================================
const RADER_HP: u8 = 8;
const RADER_AP: u8 = 4;
const SKILLFLY_RADIUS_DEFAULT: i16 = 20 << 2;
const SKILLFLY_DEPTH_RANGE: u16 = 200;
const SKILLFLY_BEHIND_PROBE: i16 = 1000;
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
/// ROM `#zaco_8p` debris mesh — `SHAPE_EXT_ZACO_8P` (tools/shape_compiler.py).
pub const SH_ZACO_8P: u16 = 283;
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
const SH_PARA_0: u16 = 59;
const SH_ZACO_6: u16 = 52;
const SH_HOUDAI_0: u16 = 54;
const SH_ITEM_5: u16 = 158;
const SH_PARA_1_PROXY: u16 = 350;
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
const SH_MY_W_PROXY: u16 = 351;
const SH_MY_R_W_PROXY: u16 = 352;
const SH_MY_L_W_PROXY: u16 = 353;
const SH_MY_B_W_PROXY: u16 = 354;
const SH_UP1_MAN_PROXY: u16 = 355;
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
/// ASM `sflag2` on the home laser — lock latch (GSTRATS.ASM:1914-1917).
pub const RELSLOWELASERHOME_LOCK_FLAG: u8 = 0x20;
const RELSLOWELASERHOME_CLOSE_Z: i16 = 800;
const RELSLOWELASERHOME_OFFSCENE_Z: i16 = 12000;
const RELSLOWELASERHOME_LIFE: u8 = 40;
const RELSLOWELASERHOME_AP: u8 = 2;
/// Hit flag HF1 (VARS.INC:167 `HF1 equ 1<<0`) tested by base1's door.
const HF1_MASK: u8 = 0x01;
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
const SH_BOSS_1_0: u16 = sh::BOSS_1_0;
const SH_BOSS_1_1: u16 = sh::BOSS_1_1;
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
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    if g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT != 0 {
        // ROM `.nocol: s_jmpto_strat`: collision handlers do not consume the
        // object's normal update.  This matters for hard/no-hit boss bodies
        // that overlap the player for many consecutive frames.
        if let Some(strat) = g.objs.aliens[idx as usize].stratptr {
            g.call_strat(strat, idx);
        }
        return;
    }
    let partner = g.objs.aliens[idx as usize].collobjptr;
    if (partner as usize) < g.objs.aliens.len() && g.objs.aliens[partner as usize].active {
        let attack_power = g.objs.aliens[partner as usize].ap;
        g.coldet_apply_damage(idx, attack_power, 0);
    }
    // ROM hitflash_Istrat (GSTRATS.ASM:895-925): every effective hit plays
    // se_damageenemynear/mid/far ($24/$25/$26) by xzdiffs range to the player
    // (<1000 / <2000 / else). The port was silent.
    play_se_by_range(g, idx, 0x24, 0x25, 0x26);
    g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
    // hitflash_Istrat.Icont ends in `s_jmpto_strat`, so movement/state logic
    // still runs during the collision frame. A hit that reaches zero health
    // enters its explosion strategy on the next object-strategy pass.
    if let Some(strat) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(strat, idx);
    }
}

/// Spawn an explosion at the collision partner (`al_collobjptr`), if any.
fn hitflash_exp_at_collobj(
    g: &mut Game,
    idx: u16,
    make: fn(&mut Game, u16) -> Option<u16>,
) -> Option<u16> {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    if partner == 0 || partner as usize >= NUMBER_AL || !g.objs.aliens[partner as usize].active {
        return None;
    }
    make(g, partner)
}

/// ROM `hitflashBOSSd_Istrat` (GSTRATS.ASM:843) — MED exp + $80, or $24 if nohitaffect.
pub fn hitflash_bossd_istrat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT != 0 {
        g.hooks.play_se(0x24);
        strat_hit_flash(g, idx);
        return;
    }
    g.hooks.play_se(0x80);
    if let Some(exp) = hitflash_exp_at_collobj(g, idx, make_medium_exp_obj) {
        g.objs.aliens[exp as usize].sflags4 |= ASF4_NOPOLYEXP;
    }
    strat_hit_flash(g, idx);
}

/// ROM `hitflashMexp_Istrat` (GSTRATS.ASM:858).
pub fn hitflash_mexp_istrat(g: &mut Game, idx: u16) {
    let _ = hitflash_exp_at_collobj(g, idx, make_medium_exp_obj);
    g.hooks.play_se(0x24);
    strat_hit_flash(g, idx);
}

/// ROM `hitflashSexp_Istrat` (GSTRATS.ASM:867).
pub fn hitflash_sexp_istrat(g: &mut Game, idx: u16) {
    let _ = hitflash_exp_at_collobj(g, idx, make_small_exp_obj);
    g.hooks.play_se(0x24);
    strat_hit_flash(g, idx);
}

/// ROM `hitflashLexp_Istrat` (GSTRATS.ASM:877) — falls into `coll_Istrat`.
pub fn hitflash_lexp_istrat(g: &mut Game, idx: u16) {
    let _ = hitflash_exp_at_collobj(g, idx, make_large_exp_obj);
    g.hooks.play_se(0x24);
    strat_hit_flash(g, idx);
}

/// ROM `misscol_Istrat` (GSTRATS.ASM:930) — missile-vs-missile flash, else kill.
pub fn misscol_istrat(g: &mut Game, idx: u16) {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    if partner != 0
        && (partner as usize) < NUMBER_AL
        && g.objs.aliens[partner as usize].active
        && g.objs.aliens[partner as usize].shape == g.objs.aliens[idx as usize].shape
    {
        return; // nomcol — same-shape ignore
    }
    // s_docoll folded into strat_hit_flash / kill path below.
    let partner_is_missile = partner != 0
        && (partner as usize) < NUMBER_AL
        && g.objs.aliens[partner as usize].type_ & ATMISSILE != 0;
    if partner_is_missile {
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
        let mch = sid(g, mchitflash_strat);
        g.objs.aliens[idx as usize].collstratptr = Some(mch);
        return;
    }
    crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
}

/// ROM `mchitflash_strat` (GSTRATS.ASM:950) — one-shot flash then restore misscol.
pub fn mchitflash_strat(g: &mut Game, idx: u16) {
    // s_docoll — apply one more damage tick when hittable.
    if g.objs.aliens[idx as usize].sflags3 & ASF3_NOHITAFFECT == 0 {
        let hp = g.objs.aliens[idx as usize].hp;
        if hp != HARD_HP && hp > 0 {
            g.objs.aliens[idx as usize].hp = hp - 1;
        }
    }
    let miss = sid(g, misscol_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags &= !(ASF_HITFLASH | ASF_COLLIDE);
    al.collstratptr = Some(miss);
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
    // ROM explode_Istrat (EXPSTRAT.ASM:685): divorce family before the rest.
    if g.objs.aliens[idx as usize].sflags4 & (ASF4_CHILDOBJ | ASF4_MOTHEROBJ) != 0 {
        divorce_family(g, idx);
    }
    // ROM: s_jmpNOT_alsflag special → skip; else spawn gate_2 + gate2_Istrat.
    // (Audit A Minor 14)
    if g.objs.aliens[idx as usize].sflags & ASF_SPECIAL != 0 {
        let _ = spawn_gate2(g, idx);
    }
    // ROM: s_jmp_alvarNOTZERO W,x,al_debrisshape,explodedebris_Istrat
    if g.objs.aliens[idx as usize].debrisshape != 0 {
        explodedebris_istrat(g, idx);
        return;
    }
    explode_icont(g, idx);
}

const EXPLOSION_SMALL_THRESHOLD: u16 = 64;
const EXPLOSION_MEDIUM_THRESHOLD: u16 = 128;
const EXPLOSION_LARGE_THRESHOLD: u16 = 256;
const EXPLOSION_POLYGON_TICKS: u8 = 12;
const EXPLOSION_SMALL_SPRITE_TICKS: u8 = 4;
const EXPLOSION_MEDIUM_SPRITE_TICKS: u8 = 6;
const EXPLOSION_LARGE_SPRITE_TICKS: u8 = 8;
const EXPLOSION_SMALL_ADJUSTMENT_SHIFT: u32 = 3;
const EXPLOSION_MEDIUM_ADJUSTMENT_SHIFT: u32 = 4;
const EXPLOSION_LARGE_ADJUSTMENT_SHIFT: u32 = 5;
const EXPLOSION_NEAR_SOUND: u8 = 33;
const EXPLOSION_MID_SOUND: u8 = 34;
const EXPLOSION_FAR_SOUND: u8 = 35;
const SH_EXPLOSION_SMALL_SPRITE: u16 = 461;
const SH_EXPLOSION_MEDIUM_SPRITE: u16 = 462;
const SH_EXPLOSION_LARGE_SPRITE: u16 = 463;
const SH_EXPLOSION_OVERSIZED_SPRITE: u16 = 464;
const SH_EXPLOSION_SMALL_POLYGONS: u16 = 465;
const SH_EXPLOSION_MEDIUM_POLYGONS: u16 = 466;
const SH_EXPLOSION_LARGE_POLYGONS: u16 = 467;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplosionSizeClass {
    Small,
    Medium,
    Large,
    Oversized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExplosionPresentation {
    polygon_shape: u16,
    sprite_shape: u16,
    sprite_ticks: u8,
    sprite_scale_adjustment: u8,
    half_rate_polygons: bool,
}

fn explosion_presentation_from_extent(
    visual_extent: u16,
    coordinate_shift: u8,
) -> ExplosionPresentation {
    let class = if visual_extent < EXPLOSION_SMALL_THRESHOLD {
        ExplosionSizeClass::Small
    } else if visual_extent < EXPLOSION_MEDIUM_THRESHOLD {
        ExplosionSizeClass::Medium
    } else if visual_extent < EXPLOSION_LARGE_THRESHOLD {
        ExplosionSizeClass::Large
    } else {
        ExplosionSizeClass::Oversized
    };
    let (polygon_shape, sprite_shape, sprite_ticks, ceiling, adjustment_shift) = match class {
        ExplosionSizeClass::Small => (
            SH_EXPLOSION_SMALL_POLYGONS,
            SH_EXPLOSION_SMALL_SPRITE,
            EXPLOSION_SMALL_SPRITE_TICKS,
            EXPLOSION_SMALL_THRESHOLD,
            EXPLOSION_SMALL_ADJUSTMENT_SHIFT,
        ),
        ExplosionSizeClass::Medium => (
            SH_EXPLOSION_MEDIUM_POLYGONS,
            SH_EXPLOSION_MEDIUM_SPRITE,
            EXPLOSION_MEDIUM_SPRITE_TICKS,
            EXPLOSION_MEDIUM_THRESHOLD,
            EXPLOSION_MEDIUM_ADJUSTMENT_SHIFT,
        ),
        ExplosionSizeClass::Large => (
            SH_EXPLOSION_LARGE_POLYGONS,
            SH_EXPLOSION_LARGE_SPRITE,
            EXPLOSION_LARGE_SPRITE_TICKS,
            EXPLOSION_LARGE_THRESHOLD,
            EXPLOSION_LARGE_ADJUSTMENT_SHIFT,
        ),
        ExplosionSizeClass::Oversized => (
            SH_EXPLOSION_LARGE_POLYGONS,
            SH_EXPLOSION_OVERSIZED_SPRITE,
            EXPLOSION_LARGE_SPRITE_TICKS,
            visual_extent,
            0,
        ),
    };
    let sprite_scale_adjustment = if class == ExplosionSizeClass::Oversized {
        0
    } else {
        visual_extent
            .wrapping_sub(ceiling)
            .wrapping_shr(u32::from(coordinate_shift))
            .wrapping_shr(adjustment_shift) as u8
    };
    ExplosionPresentation {
        polygon_shape,
        sprite_shape,
        sprite_ticks,
        sprite_scale_adjustment,
        half_rate_polygons: class == ExplosionSizeClass::Oversized,
    }
}

fn explosion_presentation(source: Alien) -> Option<ExplosionPresentation> {
    let (visual_extent, coordinate_shift) = match source.visual_kind {
        ObjectVisualKind::ExplosionEnvelope(size) => (size.source_extent() as u16, 0),
        ObjectVisualKind::Mesh | ObjectVisualKind::ScaledSprite => {
            let metrics = sf1_shape_metrics(source.shape)?;
            (metrics.visual_extent, metrics.coordinate_shift)
        }
    };
    Some(explosion_presentation_from_extent(
        visual_extent,
        coordinate_shift,
    ))
}

#[cfg(test)]
mod explosion_presentation_tests {
    use super::*;

    const SMALL_SOURCE_SHAPE: u16 = 5;
    const MEDIUM_SOURCE_SHAPE: u16 = 2;
    const LARGE_SOURCE_SHAPE: u16 = 1;
    const OVERSIZED_SOURCE_SHAPE: u16 = 11;
    const SMALL_SOURCE_ADJUSTMENT: u8 = 255;
    const MEDIUM_SOURCE_ADJUSTMENT: u8 = 253;
    const LARGE_SOURCE_ADJUSTMENT: u8 = 252;

    #[test]
    fn all_source_size_classes_select_their_exact_presentations() {
        for (source_shape, expected) in [
            (
                SMALL_SOURCE_SHAPE,
                ExplosionPresentation {
                    polygon_shape: SH_EXPLOSION_SMALL_POLYGONS,
                    sprite_shape: SH_EXPLOSION_SMALL_SPRITE,
                    sprite_ticks: EXPLOSION_SMALL_SPRITE_TICKS,
                    sprite_scale_adjustment: SMALL_SOURCE_ADJUSTMENT,
                    half_rate_polygons: false,
                },
            ),
            (
                MEDIUM_SOURCE_SHAPE,
                ExplosionPresentation {
                    polygon_shape: SH_EXPLOSION_MEDIUM_POLYGONS,
                    sprite_shape: SH_EXPLOSION_MEDIUM_SPRITE,
                    sprite_ticks: EXPLOSION_MEDIUM_SPRITE_TICKS,
                    sprite_scale_adjustment: MEDIUM_SOURCE_ADJUSTMENT,
                    half_rate_polygons: false,
                },
            ),
            (
                LARGE_SOURCE_SHAPE,
                ExplosionPresentation {
                    polygon_shape: SH_EXPLOSION_LARGE_POLYGONS,
                    sprite_shape: SH_EXPLOSION_LARGE_SPRITE,
                    sprite_ticks: EXPLOSION_LARGE_SPRITE_TICKS,
                    sprite_scale_adjustment: LARGE_SOURCE_ADJUSTMENT,
                    half_rate_polygons: false,
                },
            ),
            (
                OVERSIZED_SOURCE_SHAPE,
                ExplosionPresentation {
                    polygon_shape: SH_EXPLOSION_LARGE_POLYGONS,
                    sprite_shape: SH_EXPLOSION_OVERSIZED_SPRITE,
                    sprite_ticks: EXPLOSION_LARGE_SPRITE_TICKS,
                    sprite_scale_adjustment: 0,
                    half_rate_polygons: true,
                },
            ),
        ] {
            let mut source = Alien::default();
            source.shape = source_shape;
            assert_eq!(explosion_presentation(source), Some(expected));
        }
    }

    #[test]
    fn abstract_source_envelopes_use_their_shape_header_extents() {
        for (size, expected_sprite, expected_adjustment) in [
            (ExplosionSize::Small, SH_EXPLOSION_SMALL_SPRITE, 254),
            (ExplosionSize::Medium, SH_EXPLOSION_MEDIUM_SPRITE, 253),
            (ExplosionSize::Large, SH_EXPLOSION_LARGE_SPRITE, 254),
            (ExplosionSize::Oversized, SH_EXPLOSION_OVERSIZED_SPRITE, 0),
        ] {
            let source = Alien {
                visual_kind: ObjectVisualKind::ExplosionEnvelope(size),
                ..Alien::default()
            };
            let presentation = explosion_presentation(source).expect("envelope presentation");
            assert_eq!(presentation.sprite_shape, expected_sprite);
            assert_eq!(presentation.sprite_scale_adjustment, expected_adjustment);
        }
    }
}

fn remove_attached_fire(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].flags & AFONFIRE == 0 {
        return;
    }
    let fire = g.objs.aliens[idx as usize].fireobjptr.wrapping_sub(1);
    if (fire as usize) < NUMBER_AL && g.objs.aliens[fire as usize].active {
        g.objs.free(fire);
    }
    let object = &mut g.objs.aliens[idx as usize];
    object.fireobjptr = 0;
    object.flags &= !AFONFIRE;
}

/// ROM `explode_Icont` — special score, exact size-selected mesh/sprite
/// handoff, sound, and the two independently timed explosion lifecycles.
fn explode_icont(g: &mut Game, idx: u16) {
    let object = g.objs.aliens[idx as usize];
    if object.sflags & ASF_SPECIAL != 0 || object.sflags4 & ASF4_CSPECIAL != 0 {
        // ROM `s_test_special` (STRATMAC.INC:236-245, run from explode_Icont):
        // increments specials_dead when the object is `special` OR `Cspecial`
        // — the hit-% score numerator (sf-game score.rs).
        let sd = g.vars.shared.specials_dead;
        if sd < u8::MAX {
            g.vars.shared.specials_dead = sd + 1;
        }
        // ROM never decrements specialobjtotal (WORLD.ASM inc-only; MAIN.ASM
        // score denominator) and never sets GF_BOSSDEAD from special count
        // (only boss explode chains). Removed the port invention that did both
        // (AUDIT_HUD_SFX Critical #3 / tick 148). Score uses World::total_specials.
    }
    // ASM explode_Icont (EXPSTRAT.ASM:705): not inviewpl → silent remove_strat
    // (no destruct SE / no AFEXP visual). (Audit A Minor 14)
    use sf_game::draw::AF_INVIEW_PL;
    if g.objs.aliens[idx as usize].flags & AF_INVIEW_PL == 0 {
        g.objs.aldead = 1;
        return;
    }
    let source = g.objs.aliens[idx as usize];
    let Some(presentation) = explosion_presentation(source) else {
        // Every retail visual shape has a generated profile. A non-catalog
        // native id has no ShapeHdr behavior to reproduce and cannot safely
        // enter a guessed size class.
        g.objs.aldead = 1;
        return;
    };
    let Some(sprite) = make_obj(g, 0) else {
        g.objs.aldead = 1;
        return;
    };
    // `s_make_obj` uses the source list's insert-after-current operation, so
    // the sprite runs its newly installed explosion strategy later in this
    // same object pass.
    g.objs.active_move_after(sprite, idx);
    remove_attached_fire(g, idx);

    let explode_tick = sid(g, explode_strat);
    let large_explode_tick = sid(g, lexplode_strat);
    let sprite_rotation = crate::common::flat_billboard_rotation(&g.vars);
    {
        let object = &mut g.objs.aliens[idx as usize];
        object.flags |= AFEXP;
        object.hp = 0;
        object.visual_kind = ObjectVisualKind::Mesh;
        object.sflags &= !ASF_SSPRITE;
        object.sflags2 |= ASF2_COLLDISABLE;
        object.shape = presentation.polygon_shape;
        object.expstratptr = Some(if presentation.half_rate_polygons {
            large_explode_tick
        } else {
            explode_tick
        });
        object.rotx = sf_random(&mut g.vars) as u8;
        object.roty = sf_random(&mut g.vars) as u8;
        object.count = 0;
        object.count1 = EXPLOSION_POLYGON_TICKS;
        crate::common::init_colanim(object, 0);
    }
    {
        let object = &mut g.objs.aliens[sprite as usize];
        object.sflags = source.sflags & !(ASF_HITFLASH | ASF_SHADOW | ASF_SPECIAL);
        object.sflags2 = source.sflags2;
        object.sflags3 = source.sflags3 & !ASF3_REALOBJ;
        object.sflags4 = source.sflags4 & !ASF4_CSPECIAL;
        object.visual_kind = ObjectVisualKind::ScaledSprite;
        object.shape = presentation.sprite_shape;
        object.tx = presentation.sprite_scale_adjustment;
        object.worldx = source.worldx;
        object.worldy = source.worldy;
        object.worldz = source.worldz;
        object.vx = source.vx;
        object.vy = source.vy;
        object.vz = source.vz;
        object.rotx = sprite_rotation[0];
        object.roty = sprite_rotation[1];
        object.stratptr = Some(explode_tick);
        object.collstratptr = None;
        object.expstratptr = None;
        object.hp = HARD_HP;
        object.ap = HARD_AP;
        object.sflags |= ASF_SSPRITE;
        object.sflags2 |= ASF2_COLLDISABLE;
        object.count = 0;
        object.count1 = presentation.sprite_ticks;
        crate::common::init_colanim(object, 0);
    }
    // ROM explode chain (EXPSTRAT.ASM:853-877): se_destructenemynear/mid/far
    // ($21/$22/$23) by xzdiffs range to the player, gated on the noexpsnd
    // sflag. The port played $10 (se_itemcatch, the item chime!) on every kill.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_NOEXPSND == 0 {
        play_se_by_range(
            g,
            idx,
            EXPLOSION_NEAR_SOUND,
            EXPLOSION_MID_SOUND,
            EXPLOSION_FAR_SOUND,
        );
    }
    if g.objs.aliens[idx as usize].sflags4 & ASF4_NOPOLYEXP != 0 {
        g.objs.aldead = 1;
    }
}

/// ROM `explode_end` (EXPSTRAT.ASM:902) — shared mesh-explosion tick tail.
pub fn explode_end(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let (c, c1) = {
        let al = &g.objs.aliens[idx as usize];
        (al.count, al.count1)
    };
    // s_cmp_alvars count,count1 ; s_bpl remove_strat
    if (c as i8) >= (c1 as i8) {
        g.objs.aldead = 1;
    }
}

/// ROM `explode_strat` (EXPSTRAT.ASM:897) — advance colanim + count, then end.
pub fn explode_strat(g: &mut Game, idx: u16) {
    crate::common::add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 8);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    explode_end(g, idx);
}

/// ROM `Lexplode_strat` (EXPSTRAT.ASM:891) — half-rate colanim (notdelay 1).
pub fn lexplode_strat(g: &mut Game, idx: u16) {
    // s_jmp_NOTdelay 1,explode_end — skip anim when (gameframe&1)!=0
    if g.vars.gameframe & 1 != 0 {
        explode_end(g, idx);
        return;
    }
    crate::common::add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 8);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    explode_end(g, idx);
}

// ============================================================
// HEADFIRE (DSTRATS.ASM:8405) — boss-F severed head projectile
// ============================================================

const BOSS_F_HEAD_HP: u8 = 6;
const BOSS_F_HEAD_AP: u8 = 10;
const HEADFIRE_GROUND_SPEED: u8 = 120;
const GROUND_Y: i16 = 0;
const DEFAULT_FALL_GRAVITY: i16 = 4;
const DEFAULT_BOUNCE_SHIFT: u32 = 2;
const MINIMUM_BOUNCE_VELOCITY: i16 = -5;

/// Source `s_falldown_Yvec`: apply gravity and bounce off a horizontal plane.
fn fall_down_y_vector(object: &mut Alien, bounce_shift: u32, gravity: i16, ground: i16) {
    object.vy = object.vy.wrapping_add(gravity);
    if object.worldy < ground {
        return;
    }
    object.worldy = ground;
    let mut bounce = object.vy.wrapping_neg() >> bounce_shift;
    if (MINIMUM_BOUNCE_VELOCITY..=0).contains(&bounce) {
        bounce = 0;
    }
    object.vy = bounce;
}

/// ROM `headfire_istrat` (DSTRATS.ASM:8405) — fall to y=0, then dash at player.
pub fn headfire_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, headfire_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = BOSS_F_HEAD_HP;
        al.ap = BOSS_F_HEAD_AP;
        al.sbyte1 = 0; // phase: 0 = falling, 1 = dash
    }
}

/// Fall under gravity until worldy>=0, then aim + dash.
pub fn headfire_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        fall_down_y_vector(
            &mut g.objs.aliens[idx as usize],
            DEFAULT_BOUNCE_SHIFT,
            DEFAULT_FALL_GRAVITY,
            GROUND_Y,
        );
        apply_velocity(&mut g.objs.aliens[idx as usize]);
        if g.objs.aliens[idx as usize].worldy == GROUND_Y {
            g.objs.aliens[idx as usize].sbyte1 = 1;
            g.hooks.play_se(0x8f);
            if let Some(pl) = player(g) {
                let me = g.objs.aliens[idx as usize];
                // ROM `dobj2obj3dangle_xy` = `s_obj2obj_3dangle` (Yanglexy+nega).
                let yaw = angle_xz(&me, &pl).wrapping_neg();
                let pitch = strat_pitch_toward(&me, &pl);
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = yaw;
                al.rotx = pitch;
                al.vel = HEADFIRE_GROUND_SPEED;
                crate::common::strat_gen_vecs_3d(al);
            }
        }
        add_player_z(g, idx);
        return;
    }
    // .strat2 — tumble pitch + coast
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(DEG45);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// PARTICLE / MARK EXPLODES (EXPSTRAT.ASM)
// ============================================================

/// ROM `s_particle_data` (STRATMAC.INC): partobj + type/amount/life in sbyte3/1/2.
fn particle_data(al: &mut Alien, typ: u8, amount: u8, life: u8) {
    al.sflags |= ASF_PARTOBJ;
    al.sbyte3 = typ;
    al.sbyte1 = amount;
    al.sbyte2 = life;
    al.sflags &= !ASF_SHADOW;
}

/// Clear particle payload (`s_particle_data x,0,0,0`).
fn particle_data_clear(al: &mut Alien) {
    particle_data(al, 0, 0, 0);
}

pub const MEDPSPEED_I16: i16 = 65;

/// Shared particle-explode init: colldisable + AFEXP + expstrat + particle payload.
fn particle_explode_init(g: &mut Game, idx: u16, tick: StrategyFn, typ: u8, amount: u8, life: u8) {
    let s = sid(g, tick);
    let al = &mut g.objs.aliens[idx as usize];
    al.expstratptr = Some(s);
    al.sflags2 |= ASF2_COLLDISABLE;
    al.flags |= AFEXP;
    particle_data(al, typ, amount, life);
}

/// ROM `particleexplode_Istrat` (EXPSTRAT.ASM:914).
pub fn particleexplode_istrat(g: &mut Game, idx: u16) {
    particle_explode_init(g, idx, particleexplode_strat, 6, 60, 30);
}

/// ROM `particleexplode_strat` (EXPSTRAT.ASM:922).
pub fn particleexplode_strat(g: &mut Game, idx: u16) {
    particle_data_clear(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    if g.objs.aliens[idx as usize].count == 40 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(MEDPSPEED_I16.wrapping_sub(30));
}

/// ROM `fastparticleexplode_Istrat` (EXPSTRAT.ASM:934).
pub fn fastparticleexplode_istrat(g: &mut Game, idx: u16) {
    particle_explode_init(g, idx, fastparticleexplode_strat, 7, 60, 40);
}

/// ROM `fastparticleexplode_strat` (EXPSTRAT.ASM:943) — clear payload only.
pub fn fastparticleexplode_strat(g: &mut Game, idx: u16) {
    particle_data_clear(&mut g.objs.aliens[idx as usize]);
}

/// ROM `BIGparticleexplode_Istrat` (EXPSTRAT.ASM:951).
pub fn bigparticleexplode_istrat(g: &mut Game, idx: u16) {
    particle_explode_init(g, idx, bigparticleexplode_strat, 4, 255, 100);
}

/// ROM `BIGparticleexplode_strat` (EXPSTRAT.ASM:959).
pub fn bigparticleexplode_strat(g: &mut Game, idx: u16) {
    particle_data_clear(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    if g.objs.aliens[idx as usize].count == 110 {
        g.objs.aldead = 1;
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
    }
}

/// ROM `CIRC2particleexplode_Istrat` (EXPSTRAT.ASM:973).
pub fn circ2particleexplode_istrat(g: &mut Game, idx: u16) {
    particle_explode_init(g, idx, circ2particleexplode_strat, 6, 250, 60);
}

/// ROM `CIRC2particleexplode_strat` (EXPSTRAT.ASM:981).
pub fn circ2particleexplode_strat(g: &mut Game, idx: u16) {
    particle_data_clear(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    if g.objs.aliens[idx as usize].count == 50 {
        g.objs.aldead = 1;
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
    }
}

/// ROM `CIRCparticleexplode_Istrat` / `CIRCparticleexplode_strat` — body commented out in ROM.
pub fn circparticleexplode_istrat(_g: &mut Game, _idx: u16) {}
pub fn circparticleexplode_strat(_g: &mut Game, _idx: u16) {}

/// ROM `particlefire_Istrat` (EXPSTRAT.ASM:1011).
pub fn particlefire_istrat(g: &mut Game, idx: u16) {
    particle_data(&mut g.objs.aliens[idx as usize], 2, 2, 25);
    particlefire_icont(g, idx);
}

/// ROM `particlefire_Icont` (EXPSTRAT.ASM:1014).
pub fn particlefire_icont(g: &mut Game, idx: u16) {
    let s = sid(g, particlefire_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.expstratptr = Some(s);
    al.sflags |= ASF_COLLDISABLE;
    al.flags |= AFEXP;
}

/// ROM `particlefire_strat` — empty tick.
pub fn particlefire_strat(_g: &mut Game, _idx: u16) {}

const LARGE_PLASMA_ENTRY_SCROLL: i16 = 120;

/// ROM `largeplasma_Istrat` (DSTRATS.ASM:3641-3646). This is a one-shot
/// projectile initializer: make the billboard indestructible and apply the
/// initial forward scroll displacement. Flat billboard orientation is a
/// renderer concern in the HD port.
pub fn largeplasma_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    set_hard_vars(al);
    al.worldz = al.worldz.wrapping_sub(LARGE_PLASMA_ENTRY_SCROLL);
}

/// ROM `particlefiredown_Istrat` (EXPSTRAT.ASM:1025).
pub fn particlefiredown_istrat(g: &mut Game, idx: u16) {
    particle_data(&mut g.objs.aliens[idx as usize], 3, 4, 9);
    particlefire_icont(g, idx);
}

const SH_SMARK_PROXY: u16 = 338;
const SH_MMARK_PROXY: u16 = 339;
const SH_LMARK_PROXY: u16 = 340;

fn markexplode_istrat(g: &mut Game, idx: u16, shape: u16) {
    if let Some(mark) = make_obj(g, shape) {
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[mark as usize];
            al.worldx = src.worldx;
            al.worldy = 0;
            al.worldz = src.worldz;
            al.sflags3 &= !ASF3_REALOBJ;
        }
    }
    strat_explode(g, idx);
}

/// ROM `Smarkexplode_Istrat` (EXPSTRAT.ASM:423).
pub fn smarkexplode_istrat(g: &mut Game, idx: u16) {
    markexplode_istrat(g, idx, SH_SMARK_PROXY);
}

/// ROM `Mmarkexplode_Istrat` (EXPSTRAT.ASM:432).
pub fn mmarkexplode_istrat(g: &mut Game, idx: u16) {
    markexplode_istrat(g, idx, SH_MMARK_PROXY);
}

/// ROM `Lmarkexplode_Istrat` (EXPSTRAT.ASM:441).
pub fn lmarkexplode_istrat(g: &mut Game, idx: u16) {
    markexplode_istrat(g, idx, SH_LMARK_PROXY);
}

/// ROM `hover_Istrat` (GSTRATS.ASM:965) — scroll with player Z, spin yaw.
pub fn hover_istrat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(4);
}

/// ROM `implode_Istrat` (EXPSTRAT.ASM:399) — temporary explode-look, then recover.
pub fn implode_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, implode_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 50;
    al.expstratptr = Some(s);
    al.flags |= AFEXP;
    al.hp = 0;
    al.sflags2 |= ASF2_COLLDISABLE;
}

/// ROM `implode_strat` (EXPSTRAT.ASM:407).
pub fn implode_strat(g: &mut Game, idx: u16) {
    let next = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    if next != 0 {
        g.objs.aliens[idx as usize].count = next;
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.flags &= !AFEXP;
        al.hp = HARD_HP;
        al.sflags2 &= !ASF2_COLLDISABLE;
        al.count = 0;
    }
    g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(4);
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `stopexplode_Istrat` (EXPSTRAT.ASM:667) — undo last move, zero vecs, explode.
pub fn stopexplode_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_RELEXPLODE;
        al.worldx = al.worldx.wrapping_sub(al.vx);
        al.worldy = al.worldy.wrapping_sub(al.vy);
        al.worldz = al.worldz.wrapping_sub(al.vz);
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.vel = 0;
    }
    strat_explode(g, idx);
}

/// ROM `weapcollide_Istrat` (GSTRATS.ASM:769) — weapon hit damage / kill / exp.
pub fn weapcollide_istrat(g: &mut Game, idx: u16) {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    // A raw zero object link normally means "none", but the flat native pool
    // deliberately keeps the player in slot zero. While this collide strategy
    // is being dispatched, a zero link therefore identifies the live player,
    // not a missing partner. The collide bit supplies the otherwise-lost
    // validity tag without introducing ROM pointer encoding into port state.
    let zero_is_live_player = partner == PLAYER_OBJECT_SLOT
        && g.objs.aliens[idx as usize].sflags & ASF_COLLIDE != 0
        && g.coldet.pcbox.player == Some(PLAYER_OBJECT_SLOT);
    let partner_ok = (partner != PLAYER_OBJECT_SLOT || zero_is_live_player)
        && (partner as usize) < NUMBER_AL
        && g.objs.aliens[partner as usize].active;

    if zero_is_live_player {
        // The cartridge's player collision shell carries hard AP and is not a
        // weapon, so `weapcollide_Istrat` takes its `kill_Istrat` branch. The
        // flat port keeps those combat values in typed body/wing proxies and
        // leaves the player object's AP at zero; preserve the authored branch
        // explicitly instead of mistaking it for the zero-AP damage override.
        crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
        return;
    }

    if partner_ok {
        let pap = g.objs.aliens[partner as usize].ap;
        if pap == 0 {
            // s_docollAP x,#framesperAP,#1 — force 1 damage
            let hp = g.objs.aliens[idx as usize].hp;
            if hp != HARD_HP && hp > 0 {
                g.objs.aliens[idx as usize].hp = hp - 1;
            }
        } else if g.objs.aliens[partner as usize].collflags & ACF_WEAPON == 0 {
            crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
            return;
        } else {
            // s_docoll x,#framesperAP — one AP tick
            let hp = g.objs.aliens[idx as usize].hp;
            if hp != HARD_HP && hp > 0 {
                g.objs.aliens[idx as usize].hp = hp.saturating_sub(1.min(pap));
            }
        }
    }

    if g.objs.aliens[idx as usize].hp == 0 {
        match g.objs.aliens[idx as usize].expstratptr {
            Some(exp) => g.call_strat(exp, idx),
            None => strat_explode(g, idx),
        }
    }
}

/// ROM `sr_make_xyvec` via `s_make_xyvec` — XY velocity from angle + speed.
pub fn make_xyvec(al: &mut Alien, angle: u8, speed: u8) {
    use crate::snes_trig::{mulslog, COSTAB, SINTAB};
    // n3dvecs with troty=deg90: vx = speed*sin(angle), vy = speed*cos(angle), vz=0
    // (matches enemy_b::setoutexp_srou).
    let sp = speed as i32;
    al.vx = mulslog(sp, SINTAB[angle as usize] as i32) as i16;
    al.vy = mulslog(sp, COSTAB[angle as usize] as i32) as i16;
    al.vz = 0;
}

/// ROM `s_jmp_random label,pct` — branch (skip) when rnd < pct% of 255.
fn jmp_random_pct(g: &mut Game, pct: u8) -> bool {
    let thr = ((pct as u16) * 255) / 100;
    (sf_random(&mut g.vars) & 0xff) < thr
}

const SH_ESCAPEE_PROXY: u16 = 341;

/// ROM `escapeeexplode2_Istrat` — 10% chance to spawn escapee, then explode.
pub fn escapeeexplode2_istrat(g: &mut Game, idx: u16) {
    if !jmp_random_pct(g, 90) {
        makeescapee_icont(g, idx);
        return;
    }
    strat_explode(g, idx);
}

/// ROM `escapeeexplode_Istrat` — 20% chance to spawn escapee, then explode.
pub fn escapeeexplode_istrat(g: &mut Game, idx: u16) {
    if !jmp_random_pct(g, 80) {
        makeescapee_icont(g, idx);
        return;
    }
    strat_explode(g, idx);
}

/// ROM `ship1aexp_Istrat` (GASTRATS.ASM:604) — become invulnerable, resume tick.
pub fn ship1aexp_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
    jmpto_strat(g, idx);
}

const SHIP1_HP: u8 = 20; // STRATEQU.INC:124
const SHIP1_AP: u8 = 16; // STRATEQU.INC:125
const SHIP3_HP: u8 = HARD_HP; // STRATEQU.INC:128 ship3HP equ hardHP
const SHIP3_AP: u8 = 16;

/// ROM `ship1col_Istrat` (GASTRATS.ASM:609): HF2 → Lexp on collider + hitflash;
/// else clear collide and resume.
pub fn ship1col_istrat(g: &mut Game, idx: u16) {
    let hf = g.objs.aliens[idx as usize].hitflags;
    if hf & HF2_MASK != 0 {
        let partner = g.objs.aliens[idx as usize].collobjptr;
        if partner != 0 && (partner as usize) < NUMBER_AL && g.objs.aliens[partner as usize].active
        {
            if let Some(e) = make_large_exp_obj(g, partner) {
                g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            }
        }
        g.objs.aliens[idx as usize].hitflags = 0;
        g.hooks.play_se(0x24);
        strat_hit_flash(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags = 0;
        al.sflags &= !ASF_COLLIDE;
    }
    jmpto_strat(g, idx);
}

// ============================================================
// shiplb1 / shipoutofLB3 — LB cutscene escort ships (GCSTRATS.ASM)
// ============================================================

/// `((deg180+deg11+deg5)-deg90)/3` — turn1 duration (GCSTRATS.ASM:1210).
const SHIPLB1_TURN1_FRAMES: u8 =
    ((DEG180 as u16 + DEG11 as u16 + DEG5 as u16 - DEG90 as u16) / 3) as u8;

const GF2_STRATFLAG1: u8 = 1;

/// ROM `shiplb1ychase_srou` (GCSTRATS.ASM:1238) — chase viewtoobj.y + sword1.
pub fn shiplb1ychase_srou(g: &mut Game, idx: u16) {
    let view = g.vars.sv_i16(crate::common::sv::VIEWTOOBJ);
    if view < 0 || (view as usize) >= NUMBER_AL {
        return;
    }
    let target = g.objs.aliens[view as usize]
        .worldy
        .wrapping_add(g.objs.aliens[idx as usize].sword1);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldy = chase_proportional(al.worldy, target, 5);
}

/// ROM `shiplb1_Istrat` (GCSTRATS.ASM:1199) — friend escort peel during LB1 exit.
pub fn shiplb1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    // Fall-through into the body (no separate stratptr in ROM — self-looping Istrat).
    let s = sid(g, shiplb1_istrat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    shiplb1_body(g, idx);
}

fn shiplb1_body(g: &mut Game, idx: u16) {
    // state 0 — start
    if g.objs.aliens[idx as usize].stratstate == 0 {
        g.objs.aliens[idx as usize].vel = 20;
        shiplb1ychase_srou(g, idx);
        g.objs.aliens[idx as usize].rotz = DEG45;
        g.objs.aliens[idx as usize].roty = DEG90;
        // s_decbne_alvar — DEC then BNE
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].sbyte1 = SHIPLB1_TURN1_FRAMES;
            g.objs.aliens[idx as usize].stratstate = 1;
        }
    }

    // state 1 — turn1
    if g.objs.aliens[idx as usize].stratstate == 1 {
        shiplb1ychase_srou(g, idx);
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(3);
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].sbyte1 = 40;
            g.objs.aliens[idx as usize].stratstate = 2;
        }
    }

    // state 2 — turn2 / climb
    if g.objs.aliens[idx as usize].stratstate == 2 {
        let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb;
        if sb == 0 {
            g.objs.aliens[idx as usize].sbyte1 = 1;
            // s_jmp_notdelay 1 — skip pitch when gate closed
            if frame_tick_mod(g, 1) {
                g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(1);
            }
        } else {
            shiplb1ychase_srou(g, idx);
        }
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
}

/// ROM `shipoutofLB3_Istrat` (GCSTRATS.ASM:1531) — space fly-past escort.
pub fn shipoutoflb3_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, shipoutoflb3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte1 = 150;
        al.type_ |= ATGND;
        al.stratstate = 0;
    }
}

/// ROM `shipoutofLB3_strat` (GCSTRATS.ASM:1536).
pub fn shipoutoflb3_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            // s_beqdec → nextstate
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 1;
            } else {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                    .worldz
                    .wrapping_add(MEDPSPEED_I16.wrapping_add(35));
            }
        }
        1 => {
            g.objs.aliens[idx as usize].rotz = 0;
            let z_off = g.objs.aliens[idx as usize].ptr as i16;
            let viewposz = g.vars.sv_i16(crate::common::sv::VIEWPOSZ);
            g.objs.aliens[idx as usize].worldz = z_off.wrapping_add(viewposz);
            g.objs.aliens[idx as usize].stratstate = 2;
        }
        2 => {
            let gf2 = g.vars.shared.game_flags2;
            if gf2 & GF2_STRATFLAG1 != 0 {
                g.objs.aliens[idx as usize].stratstate = 3;
            } else {
                shipoutoflb3_dowait(g, idx);
            }
        }
        3 => {
            // s_decbne → .dowait
            let sb = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte2 = sb;
            if sb != 0 {
                shipoutoflb3_dowait(g, idx);
            } else {
                g.vars.set_sv_i16(crate::common::sv::BOOSTOBJ, idx as i16);
                // GCSTRATS.ASM:1569 — boost_sprite #10 then boostZoff=#-80
                let _ = crate::common::boost_sprite(g, Some(10));
                g.hooks.play_se(0x32);
                crate::common::set_boost_zoff(g, -80);
                g.objs.aliens[idx as usize].stratstate = 4;
                g.objs.aliens[idx as usize].sbyte1 = 15;
            }
        }
        4 => {
            g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                .worldz
                .wrapping_add(MEDPSPEED_I16.wrapping_add(15));
            apply_velocity(&mut g.objs.aliens[idx as usize]);
            if g.objs.aliens[idx as usize].vz <= 150 {
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
                achase_angle(&mut rx, DEG22.wrapping_neg(), 3);
                g.objs.aliens[idx as usize].rotx = rx;
            }
        }
        _ => {}
    }
}

fn shipoutoflb3_dowait(g: &mut Game, idx: u16) {
    let view = g.vars.sv_i16(crate::common::sv::VIEWTOOBJ);
    let (vx, vy) = if view >= 0 && (view as usize) < NUMBER_AL {
        let v = &g.objs.aliens[view as usize];
        (v.worldx, v.worldy)
    } else {
        (0, 0)
    };
    let (ox, oy) = {
        let al = &g.objs.aliens[idx as usize];
        (al.sword1, al.sword2)
    };
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = ox.wrapping_add(vx);
    al.worldy = oy.wrapping_add(vy);
    al.worldz = al.worldz.wrapping_add(MEDPSPEED_I16.wrapping_add(15));
}

/// ROM `ship1_Istrat` (GASTRATS.ASM:629) — come from behind and peel down.
pub fn ship1_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, ship1_strat);
    let s_col = sid(g, ship1col_istrat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = SHIP1_HP;
        al.ap = SHIP1_AP;
        al.vel = 10;
        al.count = 100;
    }
}

/// ROM `ship1_strat` (GASTRATS.ASM:635).
pub fn ship1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);

    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz < 1000 {
        return; // .dr
    }
    // s_dec_lifecnt — remove when count hits 0
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            g.objs.aldead = 1;
            return;
        }
        speed_to(al, 60, 1);
    }
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz < 1500 {
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // pitch up toward deg45/3 unless already there
        if al.rotx <= DEG45 / 3 {
            al.rotx = al.rotx.wrapping_add(1);
        }
        match al.sbyte1 {
            1 => al.roty = al.roty.wrapping_sub(1),
            2 => al.roty = al.roty.wrapping_add(1),
            _ => {}
        }
    }
}

/// ROM `ship1a_Istrat` (GASTRATS.ASM:541) — slow approach + shoot.
pub fn ship1a_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, ship1a_strat);
    let s_col = sid(g, ship1col_istrat);
    let s_exp = sid(g, ship1aexp_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = SHIP1_HP;
        al.ap = SHIP1_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        // This is a boss-owned child.  The map-level face is explicitly
        // no-z-remove and the core must survive its hidden approach/reveal
        // cycle even if the camera crosses its origin between those states.
        al.type_ &= !ATZREMOVE;
        gen_vecs_3d(al);
        al.snd2 = 8;
    }
}

/// ROM `ship1a_strat` (GASTRATS.ASM:548).
pub fn ship1a_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize].worldy.wrapping_sub(1);

    let Some(pl) = player(g) else {
        ship1a_cont(g, idx);
        return;
    };
    // s_jmp_objinfront y,x,.nfire — skip fire while player is in front of ship
    let me_z = g.objs.aliens[idx as usize].worldz;
    if pl.worldz >= me_z {
        ship1a_cont(g, idx);
        return;
    }

    let hp = g.objs.aliens[idx as usize].hp;
    if hp == 0 {
        // dying smoke / Lexp path
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            g.hooks.play_se(0x21);
        }
        if frame_tick_mod(g, 1) {
            if let Some(e) = make_large_exp_obj(g, idx) {
                addrnd2pos_xy(g, e);
                g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            }
            let _ = crate::common::makesmoke_srou(g, idx);
        }
        ship1a_cont(g, idx);
        return;
    }

    let fire_gate = if hp > SHIP1_HP / 2 {
        frame_tick_mod(g, 2) // delay 2 when healthy
    } else {
        frame_tick_mod(g, 4) // delay 4 when damaged
    };
    if fire_gate {
        let me = g.objs.aliens[idx as usize];
        let pitch_jitter = (sf_random(&mut g.vars) & 7) as i8 - 3;
        let yaw_jitter = (sf_random(&mut g.vars) & 7) as i8 - 3;
        let pitch = strat_pitch_toward(&me, &pl).wrapping_add(pitch_jitter as u8);
        let yaw = angle_xz(&me, &pl).wrapping_add(yaw_jitter as u8);
        strat_fire_relslowlaser(g, idx, pitch, yaw);
    }
    ship1a_cont(g, idx);
}

/// ROM `ship1a_cont` (GASTRATS.ASM:595).
pub fn ship1a_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `ship3b_strat` (GASTRATS.ASM:456) — lock vz to -pviewvelz and drift.
pub fn ship3b_strat(g: &mut Game, idx: u16) {
    let vz = g.vars.pviewvelz.wrapping_neg();
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vz = vz;
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// ROM `ship3a_strat` (GASTRATS.ASM:422) — final approach into the
/// Space Armada launch bay.  This is an internal state of `ship3_Istrat`,
/// not the similarly named standalone `ship3a_Istrat` fly-away object.
pub fn ship3a_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        ship3_cont(g, idx);
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz > 600 {
        ship3_cont(g, idx);
        return;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.snd2 = 0;
    }
    g.vars.pshipflags &= !PSF_NOCTRL;
    g.vars.gameflags |= GF_STRATDONE1;
    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        g.vars.player_view_mode = PlayerViewMode::LeavingCockpit;
        let player_idx = g.vars.internal_playpt;
        if player_idx >= 0 {
            g.apply_player_view_mode(player_idx as u16);
        }
    }
    let next = sid(g, ship3b_strat);
    g.objs.aliens[idx as usize].stratptr = Some(next);
    ship3b_strat(g, idx);
}

/// ROM `ship3c_init` (GASTRATS.ASM:471).
pub fn ship3c_init(g: &mut Game, idx: u16) {
    let s = sid(g, ship3c_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.vx = 0;
    al.vy = -10;
    al.vz = 30;
}

/// ROM `ship3c_strat` (GASTRATS.ASM:474).
pub fn ship3c_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// ROM `ship3_cont` (GASTRATS.ASM:434) — player-HP0 → ship3c; far → ship3b;
/// else pull player toward space center + noctrl.
pub fn ship3_cont(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        ship3c_init(g, idx);
        ship3c_strat(g, idx);
        return;
    }
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz > (200i16 << 4) {
        // ROM `s_jmp_Zdistmore ...,ship3b_strat` is a same-tick branch, not
        // `s_set_strat`.  Keep `ship3_strat` (or `ship3a_strat`) installed so
        // the distance gate is reevaluated as the player catches the ship.
        ship3b_strat(g, idx);
        return;
    }
    // Pull player toward space_viewCY / x=0 and lock controls.
    let pidx = g.vars.internal_playpt;
    if (pidx as usize) < NUMBER_AL && g.objs.aliens[pidx as usize].active {
        let al = &mut g.objs.aliens[pidx as usize];
        al.worldy = chase_proportional(al.worldy, SPACE_VIEWCY, 3);
        al.worldx = chase_proportional(al.worldx, 0, 4);
    }
    g.vars.pshipflags |= PSF_NOCTRL;
    g.vars.pstratflags |= PSTF_INSEQ;
    // `ship3b_strat` is the fall-through body of ship3_cont in the ROM.
    ship3b_strat(g, idx);
}

const SHIP2_AP: u8 = 16; // STRATEQU.INC:127

/// ROM `ship2_cont` / `ship2fire_cont` (GASTRATS.ASM:294-307) — fire path is
/// commented out in ROM; both labels fall through to vecs + playerZ.
pub fn ship2_cont(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// Public alias for the dead fire-cont label (same body as `ship2_cont`).
pub fn ship2fire_cont(g: &mut Game, idx: u16) {
    ship2_cont(g, idx);
}

/// ROM `ship2outside_init` (GASTRATS.ASM:310).
pub fn ship2outside_init(g: &mut Game, idx: u16) {
    let s = sid(g, ship2outside_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.vars.gameflags |= GF_STRATDONE2;
}

/// ROM `ship2outside_strat` (GASTRATS.ASM:313) — peel left/right of view.
pub fn ship2outside_strat(g: &mut Game, idx: u16) {
    let left = g.objs.aliens[idx as usize].flags & AF_LEFT_PL != 0;
    {
        let al = &mut g.objs.aliens[idx as usize];
        if left {
            if al.vx > -60 {
                al.vx = al.vx.wrapping_sub(5);
            }
        } else if al.vx < 60 {
            al.vx = al.vx.wrapping_add(5);
        }
    }
    ship2_cont(g, idx);
}

/// ROM `ship2into_init` (GASTRATS.ASM:327).
pub fn ship2into_init(g: &mut Game, idx: u16) {
    let s = sid(g, ship2into_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.vars.pstratflags |= PSTF_NOTDIE;
}

/// ROM `ship2into_strat` (GASTRATS.ASM:330) — guide player into the hangar.
pub fn ship2into_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        ship2outside_init(g, idx);
        ship2outside_strat(g, idx);
        return;
    }

    // Lock player/view to medpspeed.
    let pidx = g.vars.internal_playpt;
    if (pidx as usize) < NUMBER_AL && g.objs.aliens[pidx as usize].active {
        g.objs.aliens[pidx as usize].vel = MEDPSPEED_I16 as u8;
    }
    g.vars.pviewvelz = MEDPSPEED_I16;
    g.vars
        .set_sv_u8(crate::common::sv::PLAYER_MEDSPEED, MEDPSPEED_I16 as u8);
    g.vars
        .set_sv_u8(crate::common::sv::PLAYER_TOSPEED, MEDPSPEED_I16 as u8);

    let viewcy = g.vars.sv_i16(crate::common::sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 5);
        al.worldy = chase_proportional(al.worldy, viewcy, 4);
    }

    let Some(pl) = player(g) else {
        ship2_cont(g, idx);
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz < 300 {
        // .doneguide — hand control back, latch stratdone1, fly away.
        let s = sid(g, ship2_doneguide_strat);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        g.vars.gameflags |= GF_STRATDONE1;
        g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE);
        g.objs.aliens[idx as usize].vz = -40;
        ship2_cont(g, idx);
        return;
    }

    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    // Pull player toward space center.
    if (pidx as usize) < NUMBER_AL && g.objs.aliens[pidx as usize].active {
        let al = &mut g.objs.aliens[pidx as usize];
        al.worldy = chase_proportional(al.worldy, SPACE_VIEWCY, 3);
        al.worldx = chase_proportional(al.worldx, 0, 3);
    }
    g.vars.pstratflags |= PSTF_INSEQ;
    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        g.vars.player_view_mode = PlayerViewMode::LeavingCockpit;
        if pidx >= 0 {
            g.apply_player_view_mode(pidx as u16);
        }
    }
    ship2_cont(g, idx);
}

/// Local strat after `.doneguide` (GASTRATS.ASM:366) — keep drifting via cont.
fn ship2_doneguide_strat(g: &mut Game, idx: u16) {
    ship2_cont(g, idx);
}

/// ROM `ship2_Istrat` (GASTRATS.ASM:269) — intermediate entrance ship.
pub fn ship2_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, ship2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = SHIP2_AP;
        al.roty = DEG180;
        al.collflags |= COLLTYPE_ENEMY1;
        al.vz = -40;
        al.sbyte1 = 39;
        al.type_ |= ATGND;
        al.snd2 = 5;
    }
    g.vars.gameflags &= !(GF_STRATDONE1 | GF_STRATDONE2);
}

/// ROM `ship2_strat` (GASTRATS.ASM:281).
pub fn ship2_strat(g: &mut Game, idx: u16) {
    let viewcy = g.vars.sv_i16(crate::common::sv::VIEWCY);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 5);
        al.worldy = chase_proportional(al.worldy, viewcy, 4);
    }

    let Some(pl) = player(g) else {
        ship2_cont(g, idx);
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let dx = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    // Xdistmore #30<<2 = 120
    if dx >= 120 {
        if dz < 2000 {
            ship2outside_init(g, idx);
            ship2outside_strat(g, idx);
            return;
        }
    } else if dz < 2000 {
        ship2into_init(g, idx);
        ship2into_strat(g, idx);
        return;
    }
    ship2fire_cont(g, idx);
}

/// ROM `ship3_Istrat` (GASTRATS.ASM:377) — last big entrance ship.
pub fn ship3_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, ship3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.hp = SHIP3_HP;
        al.ap = SHIP3_AP;
        al.roty = DEG180.wrapping_add(DEG11);
        al.collflags |= COLLTYPE_ENEMY1;
        al.vy = -40;
        al.type_ |= ATGND;
        al.sflags |= ASF_COLLDISABLE;
        al.rotz = DEG90;
        al.snd2 = 8;
    }
}

/// ROM `ship3_strat` (GASTRATS.ASM:388).
pub fn ship3_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut rotz = al.rotz;
        achase_angle(&mut rotz, 0, 6);
        al.rotz = rotz;
        let mut roty = al.roty;
        achase_angle(&mut roty, DEG180, 6);
        al.roty = roty;
    }
    let me = g.objs.aliens[idx as usize];
    // s_jmp_lower x,#space_viewCY+800 — worldy > CY+800 → still rising
    if me.worldy > SPACE_VIEWCY + 800 {
        if g.objs.aliens[idx as usize].vy != 0 {
            g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(1);
        }
    } else {
        g.vars.pstratflags |= PSTF_NOTDIE;
        if frame_tick_mod(g, 1) {
            // s_weapon_rndrots2obj y,7,7: each draw is `(rnd & 7) - 3`.
            if let Some(pl) = player(g) {
                let me = g.objs.aliens[idx as usize];
                let dp = ((sf_random(&mut g.vars) & 7) as i16 - 3) as u8;
                let dy = ((sf_random(&mut g.vars) & 7) as i16 - 3) as u8;
                let pitch = strat_pitch_toward(&me, &pl).wrapping_add(dp);
                let yaw = angle_xz(&me, &pl).wrapping_add(dy);
                let _ = fire_relfastelaser_weapon_pos(
                    g, idx, pitch, yaw, 0, -120, // (-30 << 4) >> weapon_scale
                    0,
                );
            }
        }
    }
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    // Zdistmore #(200/2)<<4 = 1600 → stay on ship3_cont; else → ship3a.
    if dz <= (200i16 / 2) << 4 {
        let next = sid(g, ship3a_strat);
        g.objs.aliens[idx as usize].stratptr = Some(next);
    }
    ship3_cont(g, idx);
}

/// ROM `ship3a_Istrat` (GA2STRAT.ASM:1404) — standalone receding ship used
/// by MAP1_3C.  The original ISTRAT is its own per-frame body.
pub fn ship3a_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.roty = DEG180;
    al.worldz = al.worldz.wrapping_sub(10);
    al.worldy = al.worldy.wrapping_add(8);
    if al.rotx != DEG45 {
        al.rotx = al.rotx.wrapping_add(1);
    }
    if al.worldy >= 10_000 {
        g.objs.aldead = 1;
        return;
    }
    add_player_z(g, idx);
}

/// ROM `ship0cdown_Istrat` (GCSTRATS.ASM:2025) — the damaged carrier/colony
/// backdrop that pitches down while shedding explosions, then removes itself.
pub fn ship0cdown_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, ship0cdown_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = Some(tick);
    al.sbyte1 = 60;
    al.count = 60 + 35 + 20;
}

const SHIP_COUNTDOWN_CIRCLE_SOUND: u8 = 29;

/// ROM `ship0cdown_strat` (GCSTRATS.ASM:2030).
pub fn ship0cdown_strat(g: &mut Game, idx: u16) {
    let life = g.objs.aliens[idx as usize].count;
    if life <= 1 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].count = life - 1;

    let mut pitch = g.objs.aliens[idx as usize].rotx;
    achase_angle(&mut pitch, DEG45, 5);
    g.objs.aliens[idx as usize].rotx = pitch;

    let burning = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0;
    if burning {
        if let Some(e) = make_obj(g, 0) {
            copy_pos(g, e, idx);
            addrnd2pos_xy(g, e);
            g.objs.aliens[e as usize].worldz = g.objs.aliens[e as usize].worldz.wrapping_add(300);
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            fastparticleexplode_istrat(g, e);
        }
    }
    if let Some(e) = make_large_exp_obj(g, idx) {
        addrnd2pos_xy(g, e);
        g.objs.aliens[e as usize].worldz = g.objs.aliens[e as usize].worldz.wrapping_add(300);
        g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
    }

    if !burning {
        let delay = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = delay;
        if delay == 0 {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            let object_id = idx + 1;
            g.vars.strategy.circle_object = object_id as i16;
            g.vars
                .screen_fill_circle
                .begin_last_stage(ScreenFillCircleCenter::Object(object_id));
            g.hooks.play_se(SHIP_COUNTDOWN_CIRCLE_SOUND);
            if let Some(e) = make_obj(g, 0) {
                copy_pos(g, e, idx);
                bigparticleexplode_istrat(g, e);
            }
        }
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(MEDPSPEED_I16 - 10);
        al.worldy = al.worldy.wrapping_add(5);
    }
}

fn exitopen_common_init(g: &mut Game, idx: u16) {
    let tick = sid(g, exitopen_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.type_ |= ATGND;
    al.sflags4 |= 0x02; // ROM `donesnd`.
    al.hp = HARD_HP;
    al.ap = 0;
    al.collflags |= COLLTYPE_ENEMY1;
    al.stratptr = Some(tick);
}

/// ROM `exitopen_Istrat` (GASTRATS.ASM:3786).
pub fn exitopen_istrat(g: &mut Game, idx: u16) {
    exitopen_common_init(g, idx);
}

/// ROM `exitopensnd_Istrat`: use positional sound family 5, then exitopen.
pub fn exitopensnd_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].snd2 = 5;
    exitopen_common_init(g, idx);
}

/// ROM `exitopensnd2_Istrat` / `exitopen_Istrat` (GASTRATS.ASM:3779).
/// MAP1_3C sets `sword2` to the parent ship, `sword1` to the open distance,
/// and `sbyte1` to the signed vertical travel per frame.
pub fn exitopensnd2_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    exitopen_common_init(g, idx);
}

/// ROM `exitopen_strat` (GASTRATS.ASM:3792).
pub fn exitopen_strat(g: &mut Game, idx: u16) {
    if let Some(parent) = strat_obj_from_ptr(g.objs.aliens[idx as usize].sword2 as u16) {
        if g.objs.aliens[parent as usize].active {
            let p = g.objs.aliens[parent as usize];
            let al = &mut g.objs.aliens[idx as usize];
            al.vx = p.vx;
            al.vy = p.vy;
            al.vz = p.vz;
            add_player_z(g, idx);
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);

    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    let dz = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if dz <= me.sword1 {
        if me.sflags2 & ASF2_SFLAG1 != 0 {
            g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
            g.hooks.play_se(0x55);
        }
        let dy = me.sbyte1 as i8 as i16;
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.worldy = al.worldy.wrapping_add(dy);
    }
}

/// ROM `mine2expnofire_Istrat` (GA2STRAT.ASM:2590) — particle shower + explode.
pub fn mine2expnofire_istrat(g: &mut Game, idx: u16) {
    if let Some(p) = make_obj(g, 0) {
        copy_pos(g, p, idx);
        particleexplode_istrat(g, p);
    }
    strat_explode(g, idx);
}

/// ROM `mine2exp_Istrat` (GA2STRAT.ASM:2560) — 5-way oval-beam burst, then nofire.
pub fn mine2exp_istrat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        mine2expnofire_istrat(g, idx);
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let base_yaw = angle_xz(&me, &pl);
    let base_pitch = strat_pitch_toward(&me, &pl);
    // ASM `s_weapon_rots2obj y[,#roty_off,#rotx_off]` order:
    //   plain, #0,#deg11, #0,#-deg11, #-deg11,#0, #deg11,#0
    for &(pitch, yaw) in &[
        (base_pitch, base_yaw),
        (base_pitch.wrapping_add(DEG11), base_yaw),
        (base_pitch.wrapping_sub(DEG11), base_yaw),
        (base_pitch, base_yaw.wrapping_sub(DEG11)),
        (base_pitch, base_yaw.wrapping_add(DEG11)),
    ] {
        let _ = fire_relovalbeam_aimed(g, idx, pitch, yaw);
    }
    mine2expnofire_istrat(g, idx);
}

const GASF_FLAG1: u8 = 0x08; // STRATEQU.INC:964
const CORE1_HP: u8 = 6; // STRATEQU.INC:69

/// ROM `core0_Istrat` / `core0_strat` (GASTRATS.ASM:482-495) — hard shell that
/// hitflashes when the attached core1 col sets gasf_flag1.
pub fn core0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, core0_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= COLLTYPE_ENEMY1;
        al.roty = DEG180;
        al.hp = HARD_HP;
        al.ap = 0;
        al.stratptr = Some(tick);
    }
    set_gasflags(g, gasflags(g) & !GASF_FLAG1);
}

pub fn core0_strat(g: &mut Game, idx: u16) {
    let gf = gasflags(g);
    if gf & GASF_FLAG1 != 0 {
        set_gasflags(g, gf & !GASF_FLAG1);
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
    }
}

/// ROM `core1_Istrat` / `core1_strat` (GASTRATS.ASM:496-510) — vulnerable core
/// that clears nohitaffect once the player is within 1000 z, spins at roty+=8.
pub fn core1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, core1_strat);
    let coll = sid(g, core1col_istrat);
    let exp = sid(g, core1exp_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = CORE1_HP;
        al.ap = HARD_AP;
        al.roty = DEG180;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_NOHITAFFECT;
    }
    g.vars.gameflags &= !GF_STRATDONE1;
}

pub fn core1_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let me = &g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs();
        if zdist <= 1000 {
            g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        }
    }
    g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(8);
}

/// ROM `core1col_Istrat` (GASTRATS.ASM:534) — set gasf_flag1, then hitflash.
pub fn core1col_istrat(g: &mut Game, idx: u16) {
    let gf = gasflags(g);
    set_gasflags(g, gf | GASF_FLAG1);
    strat_hit_flash(g, idx);
}

/// ROM `core1exp_Istrat` (GASTRATS.ASM:512) — Lexp flash, then shrapnel follow.
pub fn core1exp_istrat(g: &mut Game, idx: u16) {
    if let Some(e) = make_large_exp_obj(g, idx) {
        let al = &mut g.objs.aliens[e as usize];
        al.sflags2 |= ASF2_NOEXPSND;
        al.worldy = al.worldy.wrapping_add(-60);
    }
    let s_exp = sid(g, core1exp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.expstratptr = Some(s_exp);
        al.sflags4 |= ASF4_INVISIBLE;
        al.count = 20;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
    }
    g.vars.gameflags |= GF_STRATDONE1;
    g.hooks.play_se(0x70);
    // Fall through into the tick this frame.
    core1exp_strat(g, idx);
}

/// ROM `core1exp_strat` — stick to player + shrapnel; lifecnt unless in tunnel.
pub fn core1exp_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pl.worldx;
        al.worldy = pl.worldy;
        al.worldz = pl.worldz;
    }
    crate::player::shrapnel_srou(g, idx);
    add_player_z(g, idx);
    if g.vars.pshipflags3 & PSF3_INTUNNEL == 0 {
        let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
        g.objs.aliens[idx as usize].count = c;
        if c == 0 {
            g.objs.aldead = 1;
        }
    }
}

/// ROM `monolithexp_Istrat` (GB3STRAT.ASM:4695) — instant remove.
pub fn monolithexp_istrat(g: &mut Game, _idx: u16) {
    g.objs.aldead = 1;
}

/// ROM `#face_0` Andross-face shape (GB3STRAT.ASM eye-offset branch).
/// HD catalog may still proxy this; tests set the id explicitly.
pub const SH_FACE_0: u16 = 431;

/// Hit-zone flags (VARS.INC HF1/HF2/HF3) used by monolith eyes.
const MONOLITH_HF1: u8 = 0x01; // rebelaser reflect
const MONOLITH_HF2: u8 = 0x02; // left eye
const MONOLITH_HF3: u8 = 0x04; // right eye

/// ROM `makelefteyeexp_srou` (GB3STRAT.ASM:4701) — 15 Lexp bursts on left eye.
pub fn makelefteyeexp_srou(g: &mut Game, idx: u16) {
    make_eye_exp_burst(g, idx, true);
}

/// ROM `makerighteyeexp_srou` (GB3STRAT.ASM:4725) — 15 Lexp bursts on right eye.
pub fn makerighteyeexp_srou(g: &mut Game, idx: u16) {
    make_eye_exp_burst(g, idx, false);
}

fn make_eye_exp_burst(g: &mut Game, idx: u16, left: bool) {
    let face0 = g.objs.aliens[idx as usize].shape == SH_FACE_0;
    let (dx, dy) = if left {
        if face0 {
            (-(20i16 << 4), -(30i16 << 4))
        } else {
            (-(15i16 << 4), -(15i16 << 4))
        }
    } else if face0 {
        (20i16 << 4, -(30i16 << 4))
    } else {
        (15i16 << 4, -(15i16 << 4))
    };
    for _ in 0..15 {
        let Some(e) = make_large_exp_obj(g, idx) else {
            break;
        };
        {
            let al = &mut g.objs.aliens[e as usize];
            // ROM relexplode is sflags4; HD make_exp_obj already sets ASF2_RELEXPLODE.
            al.sflags4 |= ASF4_NOPOLYEXP;
            al.worldz = al.worldz.wrapping_add(-20);
            al.worldx = al.worldx.wrapping_add(dx);
            al.worldy = al.worldy.wrapping_add(dy);
        }
        addrnd2pos_xy(g, e);
        let life = (sf_random(&mut g.vars) as u8) & 15;
        g.objs.aliens[e as usize].count = life;
    }
}

/// ROM `RebElaserCol_Istrat` (GSTRATS.ASM:787) — rebound player laser 180° yaw.
pub fn rebelasercol_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    let partner = g.objs.aliens[idx as usize].collobjptr;
    let partner_ok =
        partner != 0 && (partner as usize) < NUMBER_AL && g.objs.aliens[partner as usize].active;
    if partner_ok && g.objs.aliens[partner as usize].shape == SHAPE_ELASER2 {
        let saved = {
            let laser = &g.objs.aliens[partner as usize];
            (laser.roty, laser.rotx, laser.vel, laser.immuneptr)
        };
        {
            let laser = &mut g.objs.aliens[partner as usize];
            laser.vel = 0;
            laser.roty = laser.roty.wrapping_add(DEG180);
            laser.rotx = laser.rotx.wrapping_neg();
        }
        let _ = fire_reb_elaser(g, partner);
        {
            let laser = &mut g.objs.aliens[partner as usize];
            laser.roty = saved.0;
            laser.rotx = saved.1;
            laser.vel = saved.2;
            laser.immuneptr = saved.3;
        }
    }
    jmpto_strat(g, idx);
}

/// ROM `monolithcol_Istrat` (GB3STRAT.ASM:4671) — eye HP + HF zones + rebelaser.
pub fn monolithcol_istrat(g: &mut Game, idx: u16) {
    // s_jmp_alsflag sflag1,.ncoll — skip when strategy latch set.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        g.objs.aliens[idx as usize].hitflags = 0;
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
        jmpto_strat(g, idx);
        return;
    }
    let hf = g.objs.aliens[idx as usize].hitflags;
    if hf & MONOLITH_HF1 != 0 {
        g.objs.aliens[idx as usize].hitflags = 0;
        rebelasercol_istrat(g, idx);
        return;
    }
    // Left eye (HF2): dec sbyte1 → burst when it hits 0.
    if g.objs.aliens[idx as usize].sbyte1 != 0 && hf & MONOLITH_HF2 != 0 {
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
        let left = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = left;
        if left == 0 {
            makelefteyeexp_srou(g, idx);
        }
    }
    // Right eye (HF3): dec sbyte2 → burst when it hits 0.
    if g.objs.aliens[idx as usize].sbyte2 != 0 && hf & MONOLITH_HF3 != 0 {
        g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
        let right = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte2 = right;
        if right == 0 {
            makerighteyeexp_srou(g, idx);
        }
    }
    g.hooks.play_se(0x88);
    g.objs.aliens[idx as usize].hitflags = 0;
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    jmpto_strat(g, idx);
}

// Monolith state numbers are literal GB3STRAT.ASM values.  Keeping them
// explicit makes the sequential IFNOTstate fall-through below auditable
// against the original state table.
const MPARTOUT_I_STATE: u8 = 0;
const MPARTOUT_E_STATE: u8 = 1;
const MCHGFACE_I_STATE: u8 = 2;
const MCHGFACE_STATE: u8 = 3;
const MINITFIRE_STATE: u8 = 4;
const MFIRE1_STATE: u8 = 5;
const MSUCK_I_STATE: u8 = 6;
const MSUCK_STATE: u8 = 7;
const MSUCKSTOP_STATE: u8 = 8;
const MGAG1_STATE: u8 = 9;
const MGAG2_STATE: u8 = 10;
const MGAG3_STATE: u8 = 11;
const MGAG4_STATE: u8 = 12;
const MBLOW_I_STATE: u8 = 13;
const MBLOW_STATE: u8 = 14;
const MIEXP1_STATE: u8 = 15;
const MEXP1_STATE: u8 = 16;
const MCORE1WAIT_STATE: u8 = 17;
const MIMP1_STATE: u8 = 18;
const MLION1_STATE: u8 = 19;
const MLION2_STATE: u8 = 20;
const MLION3_STATE: u8 = 21;

const SH_FACE_B: u16 = 223;
const SH_FACE_0_1_PROXY: u16 = 345;
const SH_FACE_1_PROXY: u16 = 346;
const SH_ANDROSS_CUBE_PROXY: u16 = 344;

#[inline]
fn monolith_next_state(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
}

/// ROM `monolith_Istrat` (GB3STRAT.ASM:3876) — install the complete final
/// Andross controller, initialize both eye counters, then execute state zero.
pub fn monolith_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, monolith_strat);
    let coll = sid(g, monolithcol_istrat);
    let exp = sid(g, monolithexp_istrat);
    let eye_hp = if currentlevel(g) == 1 { 16 } else { 18 };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_FACE_B;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = 40;
        al.sbyte1 = eye_hp;
        al.sbyte2 = eye_hp;
        al.collflags |= COLLTYPE_ENEMY1;
        al.type_ &= !ATZREMOVE;
        al.sflags |= ASF_NOHITAFFECT | ASF_COLLDISABLE;
        al.stratstate = MPARTOUT_I_STATE;
    }
    monolith_strat(g, idx);
}

/// ROM `monolith_strat` (GB3STRAT.ASM:3897-4450).  The original is a
/// sequential state dispatcher: advancing a state can fall into the next
/// state's initializer in the same tick, so this deliberately is not a
/// conventional one-arm `match`.
pub fn monolith_strat(g: &mut Game, idx: u16) {
    // 0: fly the sixteen face tiles into place.
    if g.objs.aliens[idx as usize].stratstate == MPARTOUT_I_STATE {
        set_bossflags(g, 0);
        monolithpart_srou(g, idx, 200);
        g.objs.aliens[idx as usize].sflags4 |= ASF4_INVISIBLE;
        monolith_next_state(g, idx);
    }

    // 1: the lead tile raises BF_flag1 once the assembled face is ready.
    if g.objs.aliens[idx as usize].stratstate == MPARTOUT_E_STATE && bossflags(g) & BF_FLAG1 != 0 {
        let restart_z = g.vars.psvar_word1;
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags4 &= !ASF4_INVISIBLE;
        al.worldz = restart_z;
        set_bossflags(g, bossflags(g) | BF_FLAG2);
        g.objs.aliens[idx as usize].stratstate = MCHGFACE_I_STATE;
    }

    // 2: swap the assembled low-detail face for the animated battle face.
    if g.objs.aliens[idx as usize].stratstate == MCHGFACE_I_STATE {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_sub(20 << 4);
        al.worldy = al.worldy.wrapping_add(15 << 4);
        al.shape = SH_FACE_0;
        al.animframe = 0;
        al.colframe = 0;
        monolith_next_state(g, idx);
    }

    // 3: open the face and create the internal Andross cube/core.
    if g.objs.aliens[idx as usize].stratstate == MCHGFACE_STATE {
        if g.objs.aliens[idx as usize].colframe != 9 {
            g.objs.aliens[idx as usize].colframe += 1;
        }
        let f = g.objs.aliens[idx as usize].animframe;
        if f < 10 {
            g.objs.aliens[idx as usize].animframe = f + 1;
        }
        if g.objs.aliens[idx as usize].animframe == 10 {
            monolith_next_state(g, idx);
            if let Some(core) = make_obj(g, SH_ANDROSS_CUBE_PROXY) {
                copy_pos(g, core, idx);
                g.objs.aliens[core as usize].worldy =
                    g.objs.aliens[core as usize].worldy.wrapping_sub(15 << 4);
                mcore1_istrat(g, core);
                g.objs.aliens[core as usize].sflags4 |= ASF4_INVISIBLE;
                g.objs.aliens[core as usize].sflags |= ASF_COLLDISABLE;
                g.objs.aliens[idx as usize].ptr = core.wrapping_add(1);
            }
        }
    }

    // 4: arm the face for eye hits.
    if g.objs.aliens[idx as usize].stratstate == MINITFIRE_STATE {
        let al = &mut g.objs.aliens[idx as usize];
        al.flags &= !AFEXP;
        al.sflags &= !ASF_COLLDISABLE;
        al.sflags2 &= !ASF2_SFLAG1;
        al.sbyte4 = 100;
        monolith_next_state(g, idx);
    }

    // 5: normal face attack.  Eye HP is owned by monolithcol_Istrat.
    if g.objs.aliens[idx as usize].stratstate == MFIRE1_STATE {
        boss_keeprel_to_player(g, idx);
        let f = g.objs.aliens[idx as usize].animframe;
        g.objs.aliens[idx as usize].animframe = if f >= 20 { 11 } else { f + 1 };
        if g.objs.aliens[idx as usize].sbyte4 == 0 {
            monolith_next_state(g, idx);
        } else {
            g.objs.aliens[idx as usize].sbyte4 -= 1;
        }
        g.objs.aliens[idx as usize].sbyte3 = 30;
        if g.objs.aliens[idx as usize].sbyte1 == 0 && g.objs.aliens[idx as usize].sbyte2 == 0 {
            g.objs.aliens[idx as usize].stratstate = MIEXP1_STATE;
        }
    }

    // 6: begin the inhale cycle.
    if g.objs.aliens[idx as usize].stratstate == MSUCK_I_STATE {
        g.hooks.play_se(0x9f);
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte4 = 60;
        al.shape = SH_FACE_0_1_PROXY;
        al.animframe = 0;
        al.sflags2 |= ASF2_SFLAG1;
        monolith_next_state(g, idx);
    }

    // 7: suck the player and loose projectiles toward the face.
    if g.objs.aliens[idx as usize].stratstate == MSUCK_STATE {
        if g.objs.aliens[idx as usize].animframe < 4 {
            g.objs.aliens[idx as usize].animframe += 1;
        } else {
            for _ in 0..2 {
                if let Some(bit) = make_obj(g, 0) {
                    let (px, py, pz) = (
                        g.vars.player_posx,
                        g.vars.player_posy,
                        g.vars.sv_i16(crate::common::sv::VIEWPOSZ),
                    );
                    let al = &mut g.objs.aliens[bit as usize];
                    al.worldx = px;
                    al.worldy = py;
                    al.worldz = pz.wrapping_add(200);
                    suckbits_istrat(g, bit);
                }
            }
            if g.objs.aliens[idx as usize].sbyte4 >= 15 && g.vars.gameframe & 3 == 0 {
                if let Some(cube) = make_obj(g, 0) {
                    let (px, py, pz) = (g.vars.player_posx, g.vars.player_posy, g.vars.player_posz);
                    let al = &mut g.objs.aliens[cube as usize];
                    al.worldx = px;
                    al.worldy = py.wrapping_sub(1000);
                    al.worldz = pz.wrapping_add(200);
                    suckcube_istrat(g, cube);
                }
            }
            let raw_player = g.vars.internal_playpt;
            if raw_player >= 0
                && (raw_player as usize) < NUMBER_AL
                && g.objs.aliens[raw_player as usize].active
            {
                let pidx = raw_player as u16;
                let source_z = g.objs.aliens[idx as usize].worldz;
                suckobj_srou(g, pidx, source_z);
            }
        }
        if g.objs.aliens[idx as usize].sbyte4 == 0 {
            monolith_next_state(g, idx);
            g.objs.aliens[idx as usize].sbyte4 = 20;
        } else {
            g.objs.aliens[idx as usize].sbyte4 -= 1;
        }
    }

    // 8: pull back before the comic recoil sequence.
    if g.objs.aliens[idx as usize].stratstate == MSUCKSTOP_STATE {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(100);
        if g.objs.aliens[idx as usize].sbyte4 == 0 {
            g.objs.aliens[idx as usize].sword1 = 0;
            monolith_next_state(g, idx);
        } else {
            g.objs.aliens[idx as usize].sbyte4 -= 1;
        }
        if g.objs.aliens[idx as usize].animframe < 9 {
            g.objs.aliens[idx as usize].animframe += 1;
        }
    }

    // 9..12: four alternating head shakes, using the low signed byte of sword1.
    for (state, delta, target) in [
        (MGAG1_STATE, -1i8, -3i8),
        (MGAG2_STATE, 1, 3),
        (MGAG3_STATE, -1, -3),
        (MGAG4_STATE, 1, 10),
    ] {
        if g.objs.aliens[idx as usize].stratstate == state {
            let step = g.objs.aliens[idx as usize].sword1 as i8;
            g.objs.aliens[idx as usize].rotx =
                g.objs.aliens[idx as usize].rotx.wrapping_add(step as u8);
            let next = step.wrapping_add(delta);
            g.objs.aliens[idx as usize].sword1 = next as i16;
            if next == target {
                monolith_next_state(g, idx);
            }
        }
    }

    // 13/14: blow the swallowed cubes back out, then return to face fire.
    if g.objs.aliens[idx as usize].stratstate == MBLOW_I_STATE {
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(2);
        if g.objs.aliens[idx as usize].animframe < 13 {
            g.objs.aliens[idx as usize].animframe += 1;
        }
        if g.objs.aliens[idx as usize].animframe == 13 {
            g.objs.aliens[idx as usize].sbyte4 = 50;
            monolith_next_state(g, idx);
            g.hooks.play_se(0x99);
        }
    }
    if g.objs.aliens[idx as usize].stratstate == MBLOW_STATE {
        let rx = g.objs.aliens[idx as usize].rotx;
        g.objs.aliens[idx as usize].rotx = chase_proportional(rx as i8 as i16, 0, 1) as u8;
        g.objs.aliens[idx as usize].roty = ((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3);
        if let Some(cube) = make_obj(g, 0) {
            copy_pos(g, cube, idx);
            g.objs.aliens[cube as usize].worldy =
                g.objs.aliens[cube as usize].worldy.wrapping_add(10 << 4);
            blowcube_istrat(g, cube);
        }
        if g.objs.aliens[idx as usize].sbyte4 == 0 {
            g.objs.aliens[idx as usize].stratstate = MINITFIRE_STATE;
            g.objs.aliens[idx as usize].roty = 0;
            g.objs.aliens[idx as usize].shape = SH_FACE_0;
        } else {
            g.objs.aliens[idx as usize].sbyte4 -= 1;
        }
    }

    // 15: both eyes are destroyed; expose the core after the explosion shower.
    if g.objs.aliens[idx as usize].stratstate == MIEXP1_STATE {
        for _ in 0..2 {
            if let Some(e) = make_large_exp_obj(g, idx) {
                g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
                g.objs.aliens[e as usize].worldy =
                    g.objs.aliens[e as usize].worldy.wrapping_sub(15 << 4);
                addrnd2pos_xy(g, e);
            }
        }
        if g.objs.aliens[idx as usize].sbyte3 == 0 {
            for delay in [0u8, 2, 4, 6] {
                if let Some(e) = make_fol_exp_obj(g, idx) {
                    g.objs.aliens[e as usize].worldy =
                        g.objs.aliens[e as usize].worldy.wrapping_sub(15 << 4);
                    g.objs.aliens[e as usize].count = delay;
                }
            }
            g.hooks.play_se(0xa0);
            let raw = g.objs.aliens[idx as usize].ptr;
            if raw != 0 {
                let core = raw - 1;
                if (core as usize) < NUMBER_AL && g.objs.aliens[core as usize].active {
                    copy_pos(g, core, idx);
                    let al = &mut g.objs.aliens[core as usize];
                    al.worldy = al.worldy.wrapping_sub(15 << 4);
                    al.sflags4 &= !ASF4_INVISIBLE;
                    al.sflags &= !ASF_COLLDISABLE;
                    al.stratstate = 0;
                }
            }
            monolith_next_state(g, idx);
        } else {
            g.objs.aliens[idx as usize].sbyte3 -= 1;
        }
    }

    // 16: face-poly explosion expands for thirty ticks.
    if g.objs.aliens[idx as usize].stratstate == MEXP1_STATE {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.flags |= AFEXP;
        al.count = al.count.wrapping_add(2);
        al.sword1 = 130;
        if al.count == 60 {
            monolith_next_state(g, idx);
        }
    }

    // 17/18: wait for the core cycle, then re-form the route-specific face.
    if g.objs.aliens[idx as usize].stratstate == MCORE1WAIT_STATE {
        let raw = g.objs.aliens[idx as usize].ptr;
        let core_ok =
            raw != 0 && (raw as usize) <= NUMBER_AL && g.objs.aliens[(raw - 1) as usize].active;
        if !core_ok || g.vars.gameflags & GF_BOSSDEAD != 0 {
            g.objs.aldead = 1;
            return;
        }
        g.objs.aliens[idx as usize].sflags4 |= ASF4_INVISIBLE;
        if g.objs.aliens[idx as usize].sword1 == 0 {
            monolith_next_state(g, idx);
        } else {
            g.objs.aliens[idx as usize].sword1 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].stratstate == MIMP1_STATE {
        let core = g.objs.aliens[idx as usize].ptr.wrapping_sub(1);
        if (core as usize) >= NUMBER_AL || !g.objs.aliens[core as usize].active {
            g.objs.aldead = 1;
            return;
        }
        g.objs.aliens[core as usize].stratstate = 6;
        g.objs.aliens[idx as usize].sflags4 &= !ASF4_INVISIBLE;
        g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_sub(2);
        if g.objs.aliens[idx as usize].count == 0 {
            g.objs.aliens[core as usize].sflags4 |= ASF4_INVISIBLE;
            g.objs.aliens[core as usize].sflags |= ASF_COLLDISABLE;
            g.hooks.play_se(0x87);
            if currentlevel(g) == 3 {
                g.objs.aliens[idx as usize].flags &= !AFEXP;
                g.objs.aliens[idx as usize].sbyte1 = 10;
                g.objs.aliens[idx as usize].sbyte2 = 10;
                if g.objs.aliens[idx as usize].shape == SH_FACE_1_PROXY {
                    g.objs.aliens[idx as usize].stratstate = MLION3_STATE;
                } else {
                    g.objs.aliens[idx as usize].stratstate = MLION1_STATE;
                    g.objs.aliens[idx as usize].sbyte1 = 20;
                    g.objs.aliens[idx as usize].sbyte2 = 20;
                }
            } else {
                g.objs.aliens[idx as usize].stratstate = MINITFIRE_STATE;
                g.objs.aliens[idx as usize].sbyte1 = 8;
                g.objs.aliens[idx as usize].sbyte2 = 8;
            }
        }
    }

    // 19..21: route-3 lion-face transform and attack loop.
    if g.objs.aliens[idx as usize].stratstate == MLION1_STATE {
        if g.objs.aliens[idx as usize].colframe > 0 {
            g.objs.aliens[idx as usize].colframe -= 1;
        }
        if g.objs.aliens[idx as usize].animframe > 0 {
            g.objs.aliens[idx as usize].animframe -= 1;
        }
        if g.objs.aliens[idx as usize].animframe == 0 {
            monolith_next_state(g, idx);
        }
    }
    if g.objs.aliens[idx as usize].stratstate == MLION2_STATE {
        g.objs.aliens[idx as usize].shape = SH_FACE_1_PROXY;
        if g.objs.aliens[idx as usize].colframe < 9 {
            g.objs.aliens[idx as usize].colframe += 1;
        }
        if g.objs.aliens[idx as usize].animframe < 10 {
            g.objs.aliens[idx as usize].animframe += 1;
        }
        if g.objs.aliens[idx as usize].animframe == 10 {
            monolith_next_state(g, idx);
        }
    }
    if g.objs.aliens[idx as usize].stratstate == MLION3_STATE {
        boss_keeprel_to_player(g, idx);
        if g.vars.gameframe & 1 == 0 {
            if let Some(cube) = make_obj(g, 0) {
                copy_pos(g, cube, idx);
                g.objs.aliens[cube as usize].worldy =
                    g.objs.aliens[cube as usize].worldy.wrapping_add(10 << 4);
                blowcube_istrat(g, cube);
            }
        }
        let f = g.objs.aliens[idx as usize].animframe;
        g.objs.aliens[idx as usize].animframe = if f >= 20 { 11 } else { f + 1 };
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
        g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
        g.objs.aliens[idx as usize].sbyte3 = 30;
        if g.objs.aliens[idx as usize].sbyte1 == 0 && g.objs.aliens[idx as usize].sbyte2 == 0 {
            g.objs.aliens[idx as usize].stratstate = MIEXP1_STATE;
        }
    }

    // Shared eye-colour flash tail.
    let state = g.objs.aliens[idx as usize].stratstate;
    if !matches!(
        state,
        MPARTOUT_I_STATE
            | MPARTOUT_E_STATE
            | MCHGFACE_I_STATE
            | MCHGFACE_STATE
            | MLION1_STATE
            | MLION2_STATE
    ) {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = if al.sbyte1 == 0 && al.sbyte2 == 0 {
            16
        } else if al.sbyte1 == 0 {
            if al.sflags2 & ASF2_SFLAG1 != 0 {
                17
            } else {
                12 + (al.colframe.wrapping_add(1) & 1)
            }
        } else if al.sbyte2 == 0 {
            if al.sflags2 & ASF2_SFLAG1 != 0 {
                18
            } else {
                14 + (al.colframe.wrapping_add(1) & 1)
            }
        } else if al.sflags2 & ASF2_SFLAG1 != 0 {
            19
        } else {
            10 + (al.colframe.wrapping_add(1) & 1)
        };
    }

    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

const SH_ANDROSS_PROXY: u16 = 343;

/// ROM `blowcube_Istrat` (GB3STRAT.ASM:4423) — random-aimed hard cube debris.
pub fn blowcube_istrat(g: &mut Game, idx: u16) {
    let s_tick = sid(g, blowcube_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_tick);
        al.hp = HARD_HP;
        al.ap = 4;
        al.collflags |= COLLTYPE_ENEMYWEAP;
        al.vel = 80;
    }
    if let Some(pl) = player(g) {
        let yaw_off = ((sf_random(&mut g.vars) as u8) & 15).wrapping_sub(7);
        let pitch_off = ((sf_random(&mut g.vars) as u8) & 15).wrapping_sub(7);
        let me = g.objs.aliens[idx as usize];
        // ROM `s_obj2obj_3DangleOFF` (chase 0): -Yanglexy + offset.
        let yaw = angle_xz(&me, &pl).wrapping_neg().wrapping_add(yaw_off);
        let pitch = strat_pitch_toward(&me, &pl).wrapping_add(pitch_off);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = yaw;
        al.rotx = pitch;
        gen_vecs_3d(al);
        // Then randomize visual spin axes (ASM after gen_3dvecs).
        al.roty = sf_random(&mut g.vars) as u8;
        al.rotx = sf_random(&mut g.vars) as u8;
    }
}

/// ROM `blowcube_strat` — tumble + coast with player-Z scroll.
pub fn blowcube_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotx = al.rotx.wrapping_add(8);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

// ============================================================
// SUCKBITS / SUCKCUBE / suckobj helpers (GB3STRAT.ASM:4377-4421)
// Debris sucked toward Andross face position.
// ============================================================

const SPACE_VIEWCY: i16 = -60; // STRATEQU.INC:494
/// `(10<<4)+180+space_viewCY` — suck target worldy.
const SUCK_TARGET_Y: i16 = (10 << 4) + 180 + SPACE_VIEWCY;

/// ROM `suckobj_srou` — chase x→0 / y→SUCK_TARGET_Y / z→caller.z.
pub fn suckobj_srou(g: &mut Game, target_idx: u16, source_z: i16) {
    let al = &mut g.objs.aliens[target_idx as usize];
    al.worldx = chase_proportional(al.worldx, 0, 4);
    al.worldy = chase_proportional(al.worldy, SUCK_TARGET_Y, 4);
    al.worldz = chase_proportional(al.worldz, source_z, 5);
}

/// ROM `suckobjfast_srou` — zero vecs, chase xy, z+=50 + playerZ.
pub fn suckobjfast_srou(g: &mut Game, target_idx: u16) {
    {
        let al = &mut g.objs.aliens[target_idx as usize];
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.worldx = chase_proportional(al.worldx, 0, 4);
        al.worldy = chase_proportional(al.worldy, SUCK_TARGET_Y, 4);
        al.worldz = al.worldz.wrapping_add(50);
    }
    add_player_z(g, target_idx);
}

/// ROM `suckcube_Istrat` — life 20 cube tumble into suckbits_cont.
pub fn suckcube_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, suckcube_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 20;
    al.type_ &= !ATZREMOVE; // s_setnoremove_behind
    al.stratptr = Some(tick);
    al.sflags |= ASF_COLLDISABLE;
}

/// ROM `suckcube_strat` — rotx+=12 then suckbits_cont.
pub fn suckcube_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(12);
    suckbits_cont(g, idx);
}

/// ROM `suckbits_Istrat` — life 6 fragment into suckbits_cont.
pub fn suckbits_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, suckbits_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 6;
    al.type_ &= !ATZREMOVE;
    al.stratptr = Some(tick);
    al.sflags |= ASF_COLLDISABLE;
}

/// ROM `suckbits_strat` — fall straight into cont.
pub fn suckbits_strat(g: &mut Game, idx: u16) {
    suckbits_cont(g, idx);
}

/// ROM `suckbits_cont` — z+=60, chase x=0 / y=SUCK_TARGET_Y, playerZ, dec life.
pub fn suckbits_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(60);
        al.worldx = chase_proportional(al.worldx, 0, 2);
        al.worldy = chase_proportional(al.worldy, SUCK_TARGET_Y, 2);
    }
    add_player_z(g, idx);
    // s_dec_lifecnt: s_decbne_alvar B,al_count — DEC then BNE skip remove.
    let next = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = next;
    if next == 0 {
        g.objs.aldead = 1;
    }
}

// ============================================================
// MONOLITHPART (GB3STRAT.ASM:4749 / 4821-4895) — Andross face debris tiles.
// ============================================================

const SH_SFACE_B: u16 = 348;
const SH_SFACE2_B: u16 = 349;
const DEG180_OVER_15: u8 = DEG180 / 15; // 8

/// Part descriptor for `monolithpart_srou` spawn table.
struct MonoPartSpec {
    x_mul: i8,
    y_mul: i8,
    sword2: u8,
}

const MONOLITH_PARTS: [MonoPartSpec; 16] = [
    MonoPartSpec {
        x_mul: -3,
        y_mul: -3,
        sword2: 1,
    },
    MonoPartSpec {
        x_mul: 1,
        y_mul: -1,
        sword2: 4,
    },
    MonoPartSpec {
        x_mul: -3,
        y_mul: -1,
        sword2: 2,
    },
    MonoPartSpec {
        x_mul: 3,
        y_mul: 3,
        sword2: 7,
    },
    MonoPartSpec {
        x_mul: -1,
        y_mul: 1,
        sword2: 4,
    },
    MonoPartSpec {
        x_mul: -3,
        y_mul: 3,
        sword2: 4,
    },
    MonoPartSpec {
        x_mul: 1,
        y_mul: 3,
        sword2: 6,
    },
    MonoPartSpec {
        x_mul: 1,
        y_mul: 1,
        sword2: 5,
    },
    MonoPartSpec {
        x_mul: 1,
        y_mul: -3,
        sword2: 3,
    },
    MonoPartSpec {
        x_mul: 3,
        y_mul: -1,
        sword2: 5,
    },
    MonoPartSpec {
        x_mul: -3,
        y_mul: 1,
        sword2: 3,
    },
    MonoPartSpec {
        x_mul: 3,
        y_mul: -3,
        sword2: 4,
    },
    MonoPartSpec {
        x_mul: 3,
        y_mul: 1,
        sword2: 6,
    },
    MonoPartSpec {
        x_mul: -1,
        y_mul: -1,
        sword2: 3,
    },
    MonoPartSpec {
        x_mul: -1,
        y_mul: 3,
        sword2: 5,
    },
    MonoPartSpec {
        x_mul: -1,
        y_mul: -3,
        sword2: 2,
    },
];

/// ROM `monolithpart_srou` — spawn 16 face tiles; last gets monolithpartL.
pub fn monolithpart_srou(g: &mut Game, mother: u16, vz: i16) {
    let mut delay: u8 = 1;
    for (i, spec) in MONOLITH_PARTS.iter().enumerate() {
        let Some(child) = make_obj(g, SH_SFACE2_B) else {
            delay = delay.wrapping_add(3);
            continue;
        };
        copy_pos(g, child, mother);
        {
            let al = &mut g.objs.aliens[child as usize];
            al.worldx = al
                .worldx
                .wrapping_add(((60i16 << 1) * spec.x_mul as i16) as i16);
            al.worldy = al
                .worldy
                .wrapping_add(((100i16 << 1) * spec.y_mul as i16) as i16);
            al.sbyte1 = delay;
            al.vz = vz;
            al.sword2 = spec.sword2 as i16;
        }
        if i + 1 == MONOLITH_PARTS.len() {
            monolithpartl_istrat(g, child);
        } else {
            monolithpart_istrat(g, child);
        }
        delay = delay.wrapping_add(3);
    }
}

/// ROM `monolithpartL_Istrat` — lead tile (sflag1 + sbyte2=10) then shared init.
pub fn monolithpartl_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_SFLAG1;
        al.sbyte2 = 10;
    }
    monolithpart_istrat(g, idx);
}

/// ROM `monolithpart_Istrat`.
pub fn monolithpart_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, monolithpart_strat);
    let coll = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_COLLDISABLE;
        al.type_ &= !ATZREMOVE;
    }
    let r = (sf_random(&mut g.vars) as u8) & 3;
    let (s3, s4) = match r {
        1 => (DEG180_OVER_15, DEG180_OVER_15),
        2 => (0u8.wrapping_sub(DEG180_OVER_15), DEG180_OVER_15),
        3 => (DEG180_OVER_15, 0u8.wrapping_sub(DEG180_OVER_15)),
        _ => (
            0u8.wrapping_sub(DEG180_OVER_15),
            0u8.wrapping_sub(DEG180_OVER_15),
        ),
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte3 = s3;
        al.sbyte4 = s4;
        al.sword1 = 15; // zoom frames (byte used as W=#15)
    }
}

/// ROM `monolithpart_strat`.
pub fn monolithpart_strat(g: &mut Game, idx: u16) {
    if bossflags(g) & BF_FLAG2 != 0 {
        g.objs.aldead = 1;
        return;
    }

    // s_decbne_alvar B,al_sbyte1,.end — DEC then BNE skip active body
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    let next1 = s1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = next1;
    if next1 != 0 {
        add_player_z(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = 1;

    let zoom = g.objs.aliens[idx as usize].sword1 as u8;
    if zoom == 1 {
        g.hooks.play_se(0x86);
    }
    // s_beqdec_alvar B,al_sword1,.nzoom — TEST then DEC (sword1 as u8)
    if zoom == 0 {
        // .nzoom — settle into face tile
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.shape = SH_SFACE_B;
            al.roty = 0;
            al.rotx = 0;
        }
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
            let bf = bossflags(g);
            set_bossflags(g, bf | BF_FLAG3);
            if g.objs.aliens[idx as usize].sbyte2 == 10 {
                g.hooks.play_se(0x87);
            }
            let s2 = g.objs.aliens[idx as usize].sbyte2;
            let next2 = s2.wrapping_sub(1);
            g.objs.aliens[idx as usize].sbyte2 = next2;
            if next2 == 0 {
                let bf = bossflags(g);
                set_bossflags(g, bf | BF_FLAG1);
                // The lead tile is the assembly oracle for the completed
                // face.  The parent consumes this exact position before it
                // creates the core; omitting it leaves the encounter near
                // WRAM zero and the core never reaches its vulnerable flip.
                g.vars.psvar_word1 = g.objs.aliens[idx as usize].worldz;
            }
        }
    } else {
        g.objs.aliens[idx as usize].sword1 = zoom.wrapping_sub(1) as i16;
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_add(al.vz);
            al.rotx = al.rotx.wrapping_add(al.sbyte3);
            al.roty = al.roty.wrapping_add(al.sbyte4);
        }
    }

    // .end flash path
    if bossflags(g) & BF_FLAG3 != 0 {
        let sw2 = g.objs.aliens[idx as usize].sword2 as u8;
        let next = sw2.wrapping_sub(1);
        g.objs.aliens[idx as usize].sword2 = next as i16;
        if next == 0 {
            g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
        }
    }
    add_player_z(g, idx);
}

/// ROM `mcore1col_Istrat` (GB3STRAT.ASM:4597) — DefElaserCol unless state 5.
pub fn mcore1col_istrat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate != 5 {
        defelasercol_istrat(g, idx);
    } else {
        strat_hit_flash(g, idx);
    }
}

const SH_FACE_BOX_PROXY: u16 = 347;
const MCORE1_AP: u8 = 40;

/// ROM `mcore1_Istrat` (GB3STRAT.ASM:4451) — falls through into `mcore1_strat`.
pub fn mcore1_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, mcore1_strat);
    let s_col = sid(g, mcore1col_istrat);
    let s_exp = sid(g, mcore1exp_istrat);
    let level = currentlevel(g);
    let hp = match level {
        1 => 30,
        2 => 50,
        3 => 100,
        _ => 30, // unset on ROM; keep a sane default for HD
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        // The core is created as a child of the no-remove monolith and remains
        // hidden for the entire eye phase.  Once revealed it may already be
        // behind the ordinary object-reclamation plane, so it must inherit the
        // parent's lifetime rather than being culled on its first visible draw.
        al.type_ &= !ATZREMOVE;
        al.stratptr = Some(s);
        al.collstratptr = Some(s_col);
        al.expstratptr = Some(s_exp);
        al.hp = hp;
        al.ap = MCORE1_AP;
        // The core is made directly rather than through an ISTRATS map row;
        // give it the enemy collision class used by its hit/deflect handlers.
        al.collflags |= COLLTYPE_ENEMY1;
        al.rotx = DEG180;
        al.roty = DEG45.wrapping_neg(); // -deg45
        al.rotz = 0;
        al.stratstate = 0;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sflags2 |= ASF2_RELEXPLODE;
        if level != 1 {
            al.shape = SH_FACE_BOX_PROXY;
            al.colframe = 0;
        }
    }
    mcore1_strat(g, idx); // fall-through
}

/// ROM `mcore1_strat` (GB3STRAT.ASM:4478) — wait → zoom-in → zoom-away → flip → center.
///
/// States are sequential `ifnotstate` fall-throughs (0→1 and 4→5 same frame).
/// `setstate2` / `setstate4` (STRATROU.ASM:2984+) restart from the top.
pub fn mcore1_strat(g: &mut Game, idx: u16) {
    let level = currentlevel(g);

    'strat: loop {
        if level != 1 {
            let hp = g.objs.aliens[idx as usize].hp;
            let col = if hp > 60 {
                0
            } else if hp > 30 {
                1
            } else {
                2
            };
            g.objs.aliens[idx as usize].colframe = col;
        }

        if g.objs.aliens[idx as usize].sflags4 & ASF4_INVISIBLE != 0 {
            add_player_z(g, idx);
            return;
        }

        // state 0 — wait1
        if g.objs.aliens[idx as usize].stratstate == 0 {
            g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
            g.objs.aliens[idx as usize].sbyte1 = 20;
            g.objs.aliens[idx as usize].stratstate = 1;
        }

        // state 1 — wait2 (beqdec → nextstate / jmptostrat)
        if g.objs.aliens[idx as usize].stratstate == 1 {
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 2;
                continue 'strat; // nextstate
            }
            g.objs.aliens[idx as usize].sbyte1 -= 1;
        }

        // state 2 — zoom toward player
        if g.objs.aliens[idx as usize].stratstate == 2 {
            g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte2 = al.sbyte2.wrapping_add(8);
                al.vx = bee_tab_scaled(al.sbyte2, false, -1); // sintab
                al.vy = bee_tab_scaled(al.sbyte2, true, -1); // costab
            }
            if let Some(pl) = player(g) {
                let me = g.objs.aliens[idx as usize];
                let dz = (me.worldz as i32 - pl.worldz as i32).abs();
                if dz < 1500 {
                    g.objs.aliens[idx as usize].stratstate = 3;
                    continue 'strat; // nextstate
                }
            }
            {
                let al = &mut g.objs.aliens[idx as usize];
                let mut rz = al.rotz;
                crate::snes_trig::mcore_zrot(&mut rz, al.vz);
                al.rotz = rz;
                if al.vz != -80 {
                    al.vz = al.vz.wrapping_add(-10);
                }
            }
        }

        // state 3 — zoom away
        if g.objs.aliens[idx as usize].stratstate == 3 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte2 = al.sbyte2.wrapping_add(8);
                al.vx = bee_tab_scaled(al.sbyte2, false, -1);
                al.vy = bee_tab_scaled(al.sbyte2, true, -1);
            }
            if let Some(pl) = player(g) {
                let me = g.objs.aliens[idx as usize];
                let dz = (me.worldz as i32 - pl.worldz as i32).abs();
                if dz > 6000 {
                    g.objs.aliens[idx as usize].stratstate = 2;
                    continue 'strat; // setstate2
                }
            }
            {
                let al = &mut g.objs.aliens[idx as usize];
                let mut rz = al.rotz;
                crate::snes_trig::mcore_zrot(&mut rz, al.vz);
                al.rotz = rz;
                if al.vz != 100 {
                    al.vz = al.vz.wrapping_add(10);
                }
                al.worldx = chase_proportional(al.worldx, 0, 3);
                al.worldy = chase_proportional(al.worldy, SPACE_VIEWCY, 3);
                if al.vz == 0 {
                    al.stratstate = 4;
                    continue 'strat; // setstate4
                }
            }
        }

        // state 4 — flip init (falls into state 5)
        if g.objs.aliens[idx as usize].stratstate == 4 {
            g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
            let flip = if level == 3 { DEG180 / 2 } else { DEG180 / 4 };
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = flip;
            al.vx = 0;
            al.vy = 0;
            al.vz = 0;
            al.stratstate = 5;
        }

        // state 5 — flip (beqdec → setstate2)
        if g.objs.aliens[idx as usize].stratstate == 5 {
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 2;
                continue 'strat;
            }
            g.objs.aliens[idx as usize].sbyte1 -= 1;
            if level == 3 {
                let al = &mut g.objs.aliens[idx as usize];
                al.worldz = al.worldz.wrapping_sub(10);
                let mut rx = al.rotx;
                achase_angle(&mut rx, DEG180, 3);
                al.rotx = rx;
                let mut ry = al.roty;
                achase_angle(&mut ry, DEG45.wrapping_neg(), 3);
                al.roty = ry;
                let mut rz = al.rotz;
                achase_angle(&mut rz, 0, 3);
                al.rotz = rz;
                al.sword2 = al.sword2.wrapping_add(2); // alx_tx += 2
            } else {
                g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(4);
            }
        }

        // state 6 — center / park
        if g.objs.aliens[idx as usize].stratstate == 6 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                let mut rx = al.rotx;
                achase_angle(&mut rx, DEG180, 3);
                al.rotx = rx;
                let mut ry = al.roty;
                achase_angle(&mut ry, DEG45.wrapping_neg(), 3);
                al.roty = ry;
                let mut rz = al.rotz;
                achase_angle(&mut rz, 0, 3);
                al.rotz = rz;
                al.sflags |= ASF_NOHITAFFECT;
                al.worldx = chase_proportional(al.worldx, 0, 3);
            }
            let viewcy = g.vars.sv_i16(crate::common::sv::VIEWCY);
            let target_z = g.vars.player_posz.wrapping_add(2500);
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = chase_proportional(al.worldy, viewcy, 3);
            al.worldz = chase_proportional(al.worldz, target_z, 3);
            al.vx = 0;
            al.vy = 0;
            al.vz = 0;
        }

        apply_velocity(&mut g.objs.aliens[idx as usize]);
        add_player_z(g, idx);
        break;
    }
}

/// ROM `mcore1exp_Istrat` (GB3STRAT.ASM:4601) — BGM + SE, then tumble-to-burst.
pub fn mcore1exp_istrat(g: &mut Game, idx: u16) {
    g.hooks.play_music(0xf0);
    g.hooks.play_se(0xa1);
    let s_exp = sid(g, mcore1exp_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s_exp);
    mcore1exp_strat(g, idx);
}

/// ROM `mcore1exp_strat` — tumble away; past 3000z spawn cubes + qbossexplode.
pub fn mcore1exp_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(60);
        al.rotz = al.rotz.wrapping_add(12);
        al.rotx = al.rotx.wrapping_add(8).wrapping_add(4);
    }
    add_player_z(g, idx);
    let Some(pl) = player(g) else {
        return;
    };
    let dz = (g.objs.aliens[idx as usize].worldz as i32 - pl.worldz as i32).abs();
    if dz < 3000 {
        return;
    }
    let _ = make_fol_exp_obj(g, idx);
    let base_z = g.objs.aliens[idx as usize].worldz;
    for i in 0..6u16 {
        if let Some(c) = make_obj(g, SH_ANDROSS_PROXY) {
            copy_pos(g, c, idx);
            {
                let al = &mut g.objs.aliens[c as usize];
                al.worldz = base_z.wrapping_add((i as i16).wrapping_mul(100));
                al.sflags |= ASF_COLLDISABLE;
            }
            blowcube_istrat(g, c);
        }
    }
    strat_qboss_explode_init(g, idx);
}

/// ROM `makeescapee_Icont` (EXPSTRAT.ASM:499).
pub fn makeescapee_icont(g: &mut Game, idx: u16) {
    if let Some(man) = make_obj(g, SH_ESCAPEE_PROXY) {
        let s = sid(g, escapee_istrat);
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[man as usize];
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
            al.stratptr = Some(s);
        }
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(40);
    }
    strat_explode(g, idx);
}

/// ROM `escapee_Istrat` (EXPSTRAT.ASM:508) — spin rotz.
pub fn escapee_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(8);
}

/// ROM `explodeBigParts_Istrat` (EXPSTRAT.ASM:658).
pub fn explodebigparts_istrat(g: &mut Game, idx: u16) {
    if let Some(p) = make_obj(g, 0) {
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[p as usize];
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
        }
        bigparticleexplode_istrat(g, p);
    }
    strat_explode(g, idx);
}

/// Debris scatter speed from shape size class (ROM sh_size thresholds).
/// Without a shape-size table, use medium (30).
fn debris_scatter_speed(_shape: u16) -> u8 {
    30
}

/// ROM `explodeDebris_Istrat` (EXPSTRAT.ASM:516).
pub fn explodedebris_istrat(g: &mut Game, idx: u16) {
    if let Some(p) = make_obj(g, 0) {
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[p as usize];
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
        }
        particleexplode_istrat(g, p);
    }
    explodedebris_icont(g, idx);
}

/// ROM `FASTexplodeDebris_Istrat` (EXPSTRAT.ASM:483).
pub fn fastexplodedebris_istrat(g: &mut Game, idx: u16) {
    if let Some(p) = make_obj(g, 0) {
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[p as usize];
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
        }
        fastparticleexplode_istrat(g, p);
    }
    explodedebris_icont(g, idx);
}

// ============================================================
// Falling cube (DSTRATS.ASM:605-630) — shadow + gravity + simple remove.
// ============================================================

const CUBE_HP: u8 = 100;
const CUBE_AP: u8 = 16;

/// Local `s_falldown_Yvec` (STRATMAC.INC:1813) for the cube tick.
fn cube_falldown_yvec(al: &mut Alien, shift: u32, gravity: i16, ground: i16) -> bool {
    al.vy = al.vy.wrapping_add(gravity);
    if al.worldy < ground {
        return false;
    }
    al.worldy = ground;
    let mut v = al.vy.wrapping_neg();
    v >>= shift;
    if (-5..=0).contains(&v) {
        v = 0;
    }
    al.vy = v;
    v == 0
}

/// ROM `cubefall_Istrat` (DSTRATS.ASM:605).
pub fn cubefall_istrat(g: &mut Game, idx: u16) {
    let s_tick = sid(g, cubefall_strat);
    let s_coll = sid(g, cubecoll_strat);
    let s_exp = sid(g, cubeexp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.stratptr = Some(s_tick);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = CUBE_HP;
        al.ap = CUBE_AP;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    // Fall through into the tick (no s_end_strat before cubefall_strat).
    cubefall_strat(g, idx);
}

/// ROM `cubefall_strat` — billboard (cosmetic) + gravity onto y=0 + addvecs.
pub fn cubefall_strat(g: &mut Game, idx: u16) {
    // drotsflat_x is cosmetic billboard; HD leaves orientation as-is.
    let _ = cube_falldown_yvec(&mut g.objs.aliens[idx as usize], 1, 1, 0);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `cubeexp_strat` — instant remove (no mesh explode).
pub fn cubeexp_strat(g: &mut Game, idx: u16) {
    let _ = idx;
    g.objs.aldead = 1;
}

/// ROM `cubecoll_strat` — one AP damage tick, clear collide, resume main strat.
pub fn cubecoll_strat(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp;
    if hp != HARD_HP && hp > 0 {
        g.objs.aliens[idx as usize].hp = hp.saturating_sub(1);
    }
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    jmpto_strat(g, idx);
}

/// ROM `explodedebris_Icont` — spawn two debris pieces, then `explode_Icont`.
pub fn explodedebris_icont(g: &mut Game, idx: u16) {
    let shape = g.objs.aliens[idx as usize].debrisshape;
    let speed = debris_scatter_speed(shape);
    let mut angle = (sf_random(&mut g.vars) & 0xff) as u8;
    let parent_sflags = g.objs.aliens[idx as usize].sflags;
    let parent_sflags2 = g.objs.aliens[idx as usize].sflags2;
    let parent_rots = (
        g.objs.aliens[idx as usize].rotx,
        g.objs.aliens[idx as usize].roty,
        g.objs.aliens[idx as usize].rotz,
    );
    let parent_pos = (
        g.objs.aliens[idx as usize].worldx,
        g.objs.aliens[idx as usize].worldy,
        g.objs.aliens[idx as usize].worldz,
    );

    for i in 0..2u8 {
        if i == 1 {
            angle = angle.wrapping_add(85); // deg360/3
        } else {
            angle = angle.wrapping_add(16); // deg360/16 on first
        }
        if let Some(piece) = make_obj(g, shape) {
            {
                let al = &mut g.objs.aliens[piece as usize];
                al.worldx = parent_pos.0;
                al.worldy = parent_pos.1;
                al.worldz = parent_pos.2;
                al.rotx = parent_rots.0;
                al.roty = parent_rots.1;
                al.rotz = parent_rots.2;
                al.sflags = parent_sflags;
                al.sflags2 = parent_sflags2;
                make_xyvec(al, angle, speed);
            }
            exppiece_istrat(g, piece);
        }
    }
    // Clear debrisshape so explode_icont does not re-enter debris.
    g.objs.aliens[idx as usize].debrisshape = 0;
    explode_icont(g, idx);
}

/// ROM `expSpiece_Istrat` / `expSpiece_strat` (EXPSTRAT.ASM:597).
pub fn expspiece_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, expspiece_strat);
    let rotx = (sf_random(&mut g.vars) & 0xff) as u8;
    let rotz = (sf_random(&mut g.vars) & 0xff) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.rotx = rotx;
    al.rotz = rotz;
    al.sflags |= ASF_COLLDISABLE;
    al.sflags &= !ASF_HITFLASH;
    al.count = 15;
}

pub fn expspiece_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(16);
        al.rotz = al.rotz.wrapping_add(6);
        apply_velocity(al);
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
    }
}

/// ROM `exppiece_Istrat` / `exppiece_strat` (EXPSTRAT.ASM:618).
pub fn exppiece_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, exppiece_strat);
    let exp = sid(g, exppieceexp_istrat);
    let life = ((sf_random(&mut g.vars) & 7) as u8).wrapping_add(10);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = 8;
        al.sflags |= ASF_COLLDISABLE;
        al.sflags &= !ASF_HITFLASH;
        al.count = life;
        apply_velocity(al);
    }
}

pub fn exppiece_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(8);
        al.rotz = al.rotz.wrapping_add(4);
        // s_dec_lifecnt x,1 → kill_obj when count hits 0
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            crate::common::kill_obj(al);
            return;
        }
        al.count = c;
        apply_velocity(al);
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0 {
        add_player_z(g, idx);
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_sub(MEDPSPEED_I16 / 2);
    }
}

/// ROM `exppieceexp_Istrat` (EXPSTRAT.ASM:649).
pub fn exppieceexp_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_SPECIAL;
        al.sflags4 &= !ASF4_CSPECIAL;
    }
    strat_explode(g, idx);
}

/// ROM `pelasercollide_Istrat` (GSTRATS.ASM:752) — player laser hit solid → wall SE + kill.
pub fn pelasercollide_istrat(g: &mut Game, idx: u16) {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    let solid =
        if partner != 0 && (partner as usize) < NUMBER_AL && g.objs.aliens[partner as usize].active
        {
            let p = &g.objs.aliens[partner as usize];
            p.sflags & ASF_NOHITAFFECT != 0 || p.hp == HARD_HP || p.collstratptr.is_none()
        } else {
            true
        };
    if solid {
        let me = g.objs.aliens[idx as usize];
        g.hooks
            .make_snd(PosSndFamilyId::HitWall, me.worldx, me.worldz);
    }
    crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
    match g.objs.aliens[idx as usize].expstratptr {
        Some(exp) => g.call_strat(exp, idx),
        None => {}
    }
}

/// ROM `makepollen_srou_l` (EXPSTRAT.ASM:1033).
pub fn makepollen_srou(g: &mut Game, parent: u16) -> Option<u16> {
    let pollen = make_obj(g, 0)?;
    {
        let src = g.objs.aliens[parent as usize];
        let al = &mut g.objs.aliens[pollen as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy.wrapping_sub(120);
        al.worldz = src.worldz;
    }
    particlepollen_istrat(g, pollen);
    Some(pollen)
}

/// ROM `particlepollen_Istrat` (EXPSTRAT.ASM:1041).
pub fn particlepollen_istrat(g: &mut Game, idx: u16) {
    particle_explode_init(g, idx, particlepollen_strat, 6, 60, 250);
}

/// ROM `particlepollen_strat` (EXPSTRAT.ASM:1047).
pub fn particlepollen_strat(g: &mut Game, idx: u16) {
    particle_data_clear(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].count = g.objs.aliens[idx as usize].count.wrapping_add(1);
    if g.objs.aliens[idx as usize].count == 250 {
        g.objs.aldead = 1;
    }
}

/// ROM `#gate_2` shape id (shape_data.rs).
const SH_GATE2: u16 = 210;

/// `s_make_obj #gate_2` inserts after the exploding object and only installs
/// `gate2_Istrat`. The ordinary strategy walk reaches that initializer later
/// in the same pass, preserving both retail list order and first-tick timing.
fn spawn_gate2(g: &mut Game, source: u16) -> Option<u16> {
    let gate = make_obj(g, SH_GATE2)?;
    g.objs.active_move_after(gate, source);
    let src = g.objs.aliens[source as usize];
    let initializer = sid(g, strat_gate2_init);
    let al = &mut g.objs.aliens[gate as usize];
    al.worldx = src.worldx;
    al.worldy = src.worldy;
    al.worldz = src.worldz;
    al.stratptr = Some(initializer);
    Some(gate)
}

/// ROM `explodegate2_Istrat` (EXPSTRAT.ASM:1058) — maybe drop gate_2, then stopexplode.
pub fn explodegate2_istrat(g: &mut Game, idx: u16) {
    // 20% chance to attempt gate spawn (jmp_random .badobjs,80 skips 80%).
    if !jmp_random_pct(g, 80) {
        let partner = g.objs.aliens[idx as usize].collobjptr;
        let do_spawn = if partner == 0
            || (partner as usize) >= NUMBER_AL
            || !g.objs.aliens[partner as usize].active
        {
            true // s_jmp_objptrbad y,.do
        } else {
            g.objs.aliens[partner as usize].type_ & ATLASER != 0
        };
        if do_spawn {
            let _ = spawn_gate2(g, idx);
        }
    }
    stopexplode_istrat(g, idx);
}

/// Proxy for ROM `#lfdie` laser-die flash sprite.
const SH_LFDIE_PROXY: u16 = 342;

/// ROM `elaser2die_Istrat` (GSTRATS.ASM:1962) — attach flat flash child, animate out.
pub fn elaser2die_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].ptr = 0;
    if let Some(flash) = make_obj(g, SH_LFDIE_PROXY) {
        // `s_make_obj` inserts immediately after the current laser. The child
        // initializer consequently runs later in this same strategy pass.
        g.objs.active_move_after(flash, idx);
        crate::common::rotsflatstay_istrat(g, flash);
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[flash as usize];
            al.sflags3 &= !ASF3_REALOBJ;
            al.sflags2 |= ASF2_COLLDISABLE;
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
        }
        g.objs.aliens[idx as usize].ptr = flash.wrapping_add(1);
    }
    let s = sid(g, elaser2die_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    // Fall into first strat tick (ROM has no s_end between istrat and strat label).
    elaser2die_strat(g, idx);
}

/// ROM `elaser2die_strat` (GSTRATS.ASM:1975).
pub fn elaser2die_strat(g: &mut Game, idx: u16) {
    use sf_game::vars::GF_PLAYERDYING;
    if g.objs.aliens[idx as usize].sflags2 & ASF2_RELEXPLODE != 0
        && g.vars.gameflags & GF_PLAYERDYING == 0
    {
        add_player_z(g, idx);
    }
    let frame = g.objs.aliens[idx as usize].animframe & 0x7F;
    if frame == 8 {
        let child = g.objs.aliens[idx as usize].ptr;
        if child != 0 {
            let ci = child.wrapping_sub(1);
            if (ci as usize) < NUMBER_AL
                && g.objs.aliens[ci as usize].active
                && g.objs.aliens[ci as usize].shape == SH_LFDIE_PROXY
                && g.objs.aliens[ci as usize].sflags3 & ASF3_REALOBJ == 0
            {
                // Mark child dead via aldead only affects current strat object;
                // free the flash child directly. Its source slot may already
                // have been retired and reused, so validate the typed flash
                // identity before acting on the retained index.
                g.objs.free(ci);
            }
            g.objs.aliens[idx as usize].ptr = 0;
        }
        g.objs.aldead = 1;
        return;
    }
    // s_add_anim x,#2,#9
    let mut f = frame.wrapping_add(2);
    if f >= 9 {
        f = f.wrapping_sub(9);
    }
    g.objs.aliens[idx as usize].animframe = 0x80 | f;
}

/// ROM `pelaser2die_Istrat` (GSTRATS.ASM:2051).
pub fn pelaser2die_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].type_ &= !ATZREMOVE; // s_setnoremove_behind
    let n = g.vars.sv_u8(crate::common::sv::NUMPLASERS);
    if n > 0 {
        g.vars.set_sv_u8(crate::common::sv::NUMPLASERS, n - 1);
    }
    elaser2die_istrat(g, idx);
}

/// ROM `playerbeamdie_Istrat` (GSTRATS.ASM:2057).
pub fn playerbeamdie_istrat(g: &mut Game, idx: u16) {
    let n = g.vars.sv_u8(crate::common::sv::NUMPLASERS);
    if n > 0 {
        g.vars.set_sv_u8(crate::common::sv::NUMPLASERS, n - 1);
    }
    // s_set_expstrat x,remove_Istrat
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    g.objs.aliens[idx as usize].expstratptr = Some(rem);
}

/// ROM `miss_end` (GSTRATS.ASM:1377) — shared weapon tail: boss-dead kill or missbound.
pub fn miss_end(g: &mut Game, idx: u16) {
    if g.vars.gameflags & GF_BOSSDEAD != 0 || bossflags(g) & BF_DYING != 0 {
        crate::common::kill_istrat(g, idx);
        return;
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        missbound_chk_exp(g, idx);
    }
}

/// `pstf_firstframeLcol` — first-frame player laser collision (GILESALC.INC).
const PSTF_FIRSTFRAMELCOL: u8 = 16;

/// Shared Pelaser/Pbeam init: copy aim from sbyte1/2, gen×4 + mother addgen, anim #4.
fn pelaser_family_init(g: &mut Game, idx: u16, tick: StrategyFn) {
    let s = sid(g, tick);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.rotx = al.sbyte1;
        al.roty = al.sbyte2;
        let mother_spd = al.sbyte3;
        crate::common::strat_gen_vecs_3d_scaled(al, 2);
        let (bx, by, bz) = (al.vx, al.vy, al.vz);
        al.vel = mother_spd;
        crate::common::strat_gen_vecs_3d(al);
        al.vx = al.vx.wrapping_add(bx);
        al.vy = al.vy.wrapping_add(by);
        al.vz = al.vz.wrapping_add(bz);
        al.vel = 66; // restore bolt speed after mother addgen
        al.animframe = 0x80 | 4; // s_init_anim #4
        if g.vars.pstratflags & PSTF_FIRSTFRAMELCOL == 0 {
            al.sflags2 |= ASF2_COLLDISABLE;
        }
    }
}

/// ROM `Pelaser_Istrat` (GSTRATS.ASM:2023).
pub fn pelaser_istrat(g: &mut Game, idx: u16) {
    pelaser_family_init(g, idx, pelaser_strat);
}

/// ROM `Pelaser_strat` (GSTRATS.ASM:2036).
pub fn pelaser_strat(g: &mut Game, idx: u16) {
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_COLLDISABLE;
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_decbne_lifecnt — remove + dec numplasers when count hits 0
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    if c == 0 {
        g.objs.aliens[idx as usize].count = 0;
        g.objs.aldead = 1;
        let n = g.vars.sv_u8(crate::common::sv::NUMPLASERS);
        if n > 0 {
            g.vars.set_sv_u8(crate::common::sv::NUMPLASERS, n - 1);
        }
        return;
    }
    g.objs.aliens[idx as usize].count = c;
    miss_end(g, idx);
}

/// ROM `Pbeam_Istrat` (GSTRATS.ASM:2000) — same init as Pelaser, different tick.
pub fn pbeam_istrat(g: &mut Game, idx: u16) {
    pelaser_family_init(g, idx, pbeam_strat);
}

/// ROM `Pbeam_strat` (GSTRATS.ASM:2014) — zero pitch/yaw, spin rotz, fall into pelaser.
pub fn pbeam_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = 0;
        al.roty = 0;
        al.rotz = al.rotz.wrapping_add(24);
    }
    pelaser_strat(g, idx);
}

/// Extended shape-catalog id for the ROM `playerbeam` sprite.
pub const SH_PLAYER_BEAM: u16 = 415;

/// ROM `fire_playerbeam` (GSTRATS.ASM:2359).
pub fn fire_playerbeam(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_PLAYER_BEAM)?;
    let owner_vel = g.objs.aliens[firer as usize].vel;
    let (ox, oy, oz, rx, ry) = {
        let o = &g.objs.aliens[firer as usize];
        (o.worldx, o.worldy, o.worldz, o.rotx, o.roty)
    };
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    let coll = sid(g, pelasercollide_istrat);
    let exp = sid(g, playerbeamdie_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = ox;
        al.worldy = oy;
        al.worldz = oz.wrapping_add(mz);
        al.hp = 1;
        al.ap = 3; // playerbeamAP
        al.vel = 66;
        al.count = 10;
        al.rotx = rx;
        al.roty = ry;
        al.sbyte1 = rx;
        al.sbyte2 = ry;
        al.sbyte3 = owner_vel;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE5; // laser + friend
        al.type_ = ATMISSILE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    pbeam_istrat(g, shot);
    let n = g.vars.sv_u8(crate::common::sv::NUMPLASERS);
    if n < 0xFF {
        g.vars.set_sv_u8(crate::common::sv::NUMPLASERS, n + 1);
    }
    Some(shot)
}

/// ROM `fire_Elaser` (GSTRATS.ASM:2346) — player laser bolt via Pelaser.
/// Shape 511 = HD `elaser2` stand-in (`SHAPE_ELASER2` in player.rs).
pub fn fire_elaser(g: &mut Game, firer: u16) -> Option<u16> {
    const SHAPE_ELASER2: u16 = 511;
    let shot = make_obj(g, SHAPE_ELASER2)?;
    let owner_vel = g.objs.aliens[firer as usize].vel;
    let (ox, oy, oz, rx, ry) = {
        let o = &g.objs.aliens[firer as usize];
        (o.worldx, o.worldy, o.worldz, o.rotx, o.roty)
    };
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    let coll = sid(g, pelasercollide_istrat);
    let exp = sid(g, pelaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = ox;
        al.worldy = oy;
        al.worldz = oz.wrapping_add(mz);
        al.hp = 1;
        al.ap = 2; // elaserAP
        al.vel = 66;
        al.count = 10;
        al.rotx = rx;
        al.roty = ry;
        al.sbyte1 = rx;
        al.sbyte2 = ry;
        al.sbyte3 = owner_vel;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE5;
        al.type_ = ATMISSILE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.shape = SHAPE_ELASER2;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    pelaser_istrat(g, shot);
    let n = g.vars.sv_u8(crate::common::sv::NUMPLASERS);
    if n < 0xFF {
        g.vars.set_sv_u8(crate::common::sv::NUMPLASERS, n + 1);
    }
    Some(shot)
}

// ============================================================
// RELELASER / RELFLATMISS / FLATMISS + fire_friend / fire_reb / fire_plasma
// (GSTRATS.ASM:1773-1894, 2373-2418)
// ============================================================

const SHAPE_ELASER2: u16 = 511;
/// Authored ROM `bouncyball` shape used by plasma bolts and pillar impacts.
pub const SH_BOUNCYBALL: u16 = 405;
const ELASER_AP: u8 = 2;
const ENEMYLASER_AP: u8 = 2;
const PLASMA_AP: u8 = 10;

/// Shared: copy aim from sbyte1/2, gen×(1<<scale) + mother addgen, init anim.
fn relelaser_family_init(g: &mut Game, idx: u16, tick: StrategyFn, scale: u32, anim0: u8) {
    let s = sid(g, tick);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.rotx = al.sbyte1;
        al.roty = al.sbyte2;
        let bolt_spd = al.vel;
        let mother_spd = al.sbyte3;
        crate::common::strat_gen_vecs_3d_scaled(al, scale);
        let (bx, by, bz) = (al.vx, al.vy, al.vz);
        al.vel = mother_spd;
        crate::common::strat_gen_vecs_3d(al);
        al.vx = al.vx.wrapping_add(bx);
        al.vy = al.vy.wrapping_add(by);
        al.vz = al.vz.wrapping_add(bz);
        al.vel = bolt_spd;
        al.animframe = 0x80 | anim0;
    }
}

/// ROM `relelaser_Istrat` (GSTRATS.ASM:1869) — relative laser, gen scale 1.
pub fn relelaser_istrat(g: &mut Game, idx: u16) {
    relelaser_family_init(g, idx, relelaser_strat, 1, 0);
}

/// ROM `relelaser_strat` (GSTRATS.ASM:1880).
pub fn relelaser_strat(g: &mut Game, idx: u16) {
    // s_cmp_anim #4 / s_add_anim #2,#8
    let frame = g.objs.aliens[idx as usize].animframe & 0x7F;
    if frame != 4 {
        let mut f = frame.wrapping_add(2);
        if f >= 8 {
            f = f.wrapping_sub(8);
        }
        g.objs.aliens[idx as usize].animframe = 0x80 | f;
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    if c == 0 {
        g.objs.aliens[idx as usize].count = 0;
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].count = c;
    miss_end(g, idx);
}

/// ROM `relflatmiss_Istrat` (GSTRATS.ASM:1773) — flat missile, scrolls with player.
pub fn relflatmiss_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, relflatmiss_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte1 = al.roty;
        al.sbyte2 = al.rotx;
        // gen_3dvecs from sbyte1/2 (yaw/pitch stored there)
        al.roty = al.sbyte1;
        al.rotx = al.sbyte2;
        crate::common::strat_gen_vecs_3d(al);
        al.snd2 = 6;
    }
}

/// ROM `relflatmiss_strat` (GSTRATS.ASM:1780).
pub fn relflatmiss_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_dec_lifecnt x,1 → kill_obj when count hits 0
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            crate::common::kill_obj(al);
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `flatmiss_Istrat` (GSTRATS.ASM:1790) — same as relflatmiss but no playerZ scroll.
pub fn flatmiss_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, flatmiss_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte1 = al.roty;
        al.sbyte2 = al.rotx;
        al.roty = al.sbyte1;
        al.rotx = al.sbyte2;
        crate::common::strat_gen_vecs_3d(al);
        al.snd2 = 6;
    }
}

/// ROM `flatmiss_strat` (GSTRATS.ASM:1797).
pub fn flatmiss_strat(g: &mut Game, idx: u16) {
    // s_rots_flat — cosmetic billboard; HD leaves orientation.
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            crate::common::kill_obj(al);
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// Place a weapon at firer + muzzle Z, with aim/speed/life/coll already set by caller.
fn place_weapon_at_firer(g: &mut Game, shot: u16, firer: u16, muzzle_z: i16) {
    let (ox, oy, oz, rx, ry, owner_vel) = {
        let o = &g.objs.aliens[firer as usize];
        (o.worldx, o.worldy, o.worldz, o.rotx, o.roty, o.vel)
    };
    let al = &mut g.objs.aliens[shot as usize];
    al.worldx = ox;
    al.worldy = oy;
    al.worldz = oz.wrapping_add(muzzle_z);
    al.rotx = rx;
    al.roty = ry;
    al.sbyte1 = rx;
    al.sbyte2 = ry;
    al.sbyte3 = owner_vel;
    al.sflags4 &= !ASF4_INVISIBLE;
    // `gen_weapon` marks every spawned shot as a weapon/first-frame object
    // and `s_make_immune` stores the two object pointers in both directions.
    // Object links use one-based values in `ptr`, but collision immunity is a
    // raw slot index in this port (see `coldet::run_collision_pass`).
    al.collflags |= ACF_FIRSTFRAME | ACF_WEAPON;
    al.immuneptr = firer;
    g.objs.aliens[firer as usize].immuneptr = shot;
}

/// ROM `fire_friendElaser` (GSTRATS.ASM:2373).
pub fn fire_friend_elaser(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_ELASER2)?;
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = ELASER_AP;
        al.vel = 66; // 55+11
        al.count = 10;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE5; // laser + friend
        al.type_ = ATMISSILE;
        al.shape = SHAPE_ELASER2;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    relelaser_istrat(g, shot);
    // ROM `jsl lasersound_l` (GSTRATS.ASM:2383).
    make_firer_snd(g, firer, PosSndFamilyId::Laser);
    Some(shot)
}

/// ROM `fire_RebElaser` (GSTRATS.ASM:2387) — rebound / wall laser.
pub fn fire_reb_elaser(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_ELASER2)?;
    const WEAPON_SCALE: i16 = 2;
    // s_add_var svar_weapZ,#80>>weapon_scale on top of default muzzle
    let mz = (80i16 >> WEAPON_SCALE).wrapping_add(80i16 >> WEAPON_SCALE);
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = ENEMYLASER_AP;
        al.vel = 60;
        al.count = 40;
        // enemyweap + laser + enemy1 + enemy2
        al.collflags |= ACF_COLLTYPE4 | ACF_COLLTYPE1 | ACF_COLLTYPE2 | ACF_COLLTYPE3;
        al.type_ = ATMISSILE;
        al.shape = SHAPE_ELASER2;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    relelaser_istrat(g, shot);
    // ROM `jsl hitwallsound_l` (GSTRATS.ASM:2399).
    make_firer_snd(g, firer, PosSndFamilyId::HitWall);
    Some(shot)
}

/// ROM `fire_plasma` / `fire_relbeamball` (GSTRATS.ASM:2405) — relflatmiss plasma.
pub fn fire_plasma(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_BOUNCYBALL)?;
    g.objs.active_move_after(shot, firer);
    place_weapon_at_firer(g, shot, firer, 0);
    let coll = sid(g, weapcollide_istrat);
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = PLASMA_AP;
        al.vel = 80;
        al.count = 100; // 30+70
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4; // laser + enemyweap
        al.type_ = ATMISSILE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(rem);
    }
    relflatmiss_istrat(g, shot);
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2417).
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(PosSndFamilyId::EnemyBattry, fx, fz);
    Some(shot)
}

/// ROM `fire_beamball` (GSTRATS.ASM:2422) — flatmiss (no playerZ scroll).
pub fn fire_beamball(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_BOUNCYBALL)?;
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = 8;
        al.vel = 70;
        al.count = 100;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4;
        al.type_ = ATMISSILE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(rem);
    }
    flatmiss_istrat(g, shot);
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2433).
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(PosSndFamilyId::EnemyBattry, fx, fz);
    Some(shot)
}

/// Shared spawn for flat/relflat beam weapons (oval/ring/shortplasma family).
fn fire_flat_beam(
    g: &mut Game,
    firer: u16,
    shape: u16,
    ap: u8,
    speed: u8,
    life: u8,
    relative: bool,
    family: PosSndFamilyId,
) -> Option<u16> {
    let shot = make_obj(g, shape)?;
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = ap;
        al.vel = speed;
        al.count = life;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4;
        al.type_ = ATMISSILE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(rem);
    }
    if relative {
        relflatmiss_istrat(g, shot);
    } else {
        flatmiss_istrat(g, shot);
    }
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(family, fx, fz);
    Some(shot)
}

/// ROM `fire_relovalbeam` (GSTRATS.ASM:2438).
pub fn fire_relovalbeam(g: &mut Game, firer: u16) -> Option<u16> {
    fire_flat_beam(g, firer, 416, 8, 70, 100, true, PosSndFamilyId::EnemyBattry)
}

/// Fire a relative oval beam aimed at `pitch`/`yaw` (mine2exp burst helper).
fn fire_relovalbeam_aimed(g: &mut Game, firer: u16, pitch: u8, yaw: u8) -> Option<u16> {
    let shot = fire_relovalbeam(g, firer)?;
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = pitch;
        al.roty = yaw;
        al.sbyte1 = pitch;
        al.sbyte2 = yaw;
        crate::common::strat_gen_vecs_3d(al);
        // s_add_rnd2alvar y,al_vel,#31
        al.vel = al.vel.wrapping_add((sf_random(&mut g.vars) as u8) & 31);
    }
    Some(shot)
}

/// ROM `fire_relringlaser` (GSTRATS.ASM:2454).
pub fn fire_relringlaser(g: &mut Game, firer: u16) -> Option<u16> {
    fire_flat_beam(g, firer, 334, 6, 70, 100, true, PosSndFamilyId::EnemyBattry)
}

/// ROM `fire_ovalbeam` (GSTRATS.ASM:2470).
pub fn fire_ovalbeam(g: &mut Game, firer: u16) -> Option<u16> {
    fire_flat_beam(
        g,
        firer,
        416,
        8,
        70,
        100,
        false,
        PosSndFamilyId::EnemyBattry,
    )
}

/// ROM `fire_ringlaser` (GSTRATS.ASM:2486) — ledger already True; body for callers.
pub fn fire_ringlaser(g: &mut Game, firer: u16) -> Option<u16> {
    // ROM `jsl ringlasersound_l` (GSTRATS.ASM:2497).
    fire_flat_beam(g, firer, 334, 6, 70, 100, false, PosSndFamilyId::RingLaser)
}

/// ROM `firenormringlaser_l` (DSTRATS.ASM:6339) — same as `fireringlaser_l` but
/// `al_vel = 120` (vs 60). Uses the flat-beam path (shape/locusmode ring spread
/// is scoped out, matching `fire_ringlaser`).
pub fn firenormringlaser(g: &mut Game, firer: u16) -> Option<u16> {
    fire_flat_beam(g, firer, 334, 6, 120, 100, false, PosSndFamilyId::RingLaser)
}

/// ROM `fire_shortplasma` (GSTRATS.ASM:2502) — plasmaAP, life 30.
pub fn fire_shortplasma(g: &mut Game, firer: u16) -> Option<u16> {
    fire_flat_beam(
        g,
        firer,
        SH_BOUNCYBALL,
        PLASMA_AP,
        80,
        30,
        true,
        PosSndFamilyId::EnemyBattry,
    )
}

/// ROM `homingflat_Istrat` (GSTRATS.ASM:1723): preserve the initial weapon
/// aim in sbyte1/2, billboard the visible object, and home via the shared tick.
pub fn homingflat_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, homingflat_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = al.roty;
    al.sbyte2 = al.rotx;
    al.rotx = 0;
    al.roty = DEG180;
    al.snd2 = 6;
}

/// ROM `fire_Hplasma` (GSTRATS.ASM:2517): bouncyball mesh, AP 10, speed 60,
/// life 50, homingflat strategy. The caller assigns the target pointer just
/// after `s_fire_weapon`, exactly as the assembly call sites do.
fn fire_hplasma_with_rotation(
    g: &mut Game,
    firer: u16,
    pitch_offset: u8,
    yaw_offset: u8,
) -> Option<u16> {
    let shot = make_obj(g, SH_BOUNCYBALL)?;
    // `s_make_obj` uses `l_add`: the projectile follows its firer and reaches
    // homingflat's fall-through movement later in this same strategy pass.
    g.objs.active_move_after(shot, firer);
    place_weapon_at_firer(g, shot, firer, 0);
    let coll = sid(g, weapcollide_istrat);
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = HPLASMA_AP;
        al.vel = 60;
        al.count = 50;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4;
        al.type_ = ATMISSILE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(rem);
        al.rotx = al.rotx.wrapping_add(pitch_offset);
        al.roty = al.roty.wrapping_add(yaw_offset);
    }
    homingflat_istrat(g, shot);
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(PosSndFamilyId::EnemyBattry, fx, fz);
    Some(shot)
}

pub fn fire_hplasma(g: &mut Game, firer: u16) -> Option<u16> {
    fire_hplasma_with_rotation(g, firer, 0, 0)
}

/// ROM `elaser_Istrat` (GSTRATS.ASM:1935) — non-relative laser (no mother addgen, no playerZ).
pub fn elaser_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, elaser_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.rotx = al.sbyte1;
        al.roty = al.sbyte2;
        crate::common::strat_gen_vecs_3d_scaled(al, 1);
        al.animframe = 0x80; // s_init_anim #0
    }
}

/// ROM `elaser_strat` (GSTRATS.ASM:1946).
pub fn elaser_strat(g: &mut Game, idx: u16) {
    let frame = g.objs.aliens[idx as usize].animframe & 0x7F;
    if frame != 4 {
        let mut f = frame.wrapping_add(2);
        if f >= 8 {
            f = f.wrapping_sub(8);
        }
        g.objs.aliens[idx as usize].animframe = 0x80 | f;
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    if c == 0 {
        g.objs.aliens[idx as usize].count = 0;
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].count = c;
    miss_end(g, idx);
}

/// ROM `fire_slowElaser` (GSTRATS.ASM:2593).
pub fn fire_slow_elaser(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_ENEMY_LASER)?;
    const WEAPON_SCALE: i16 = 2;
    let mz = (80i16 >> WEAPON_SCALE).wrapping_add(80i16 >> WEAPON_SCALE);
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, elaser2die_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = ENEMYLASER_AP;
        al.vel = 60;
        al.count = 40;
        al.collflags |= ACF_COLLTYPE4 | ACF_COLLTYPE1;
        al.type_ = ATMISSILE;
        al.shape = SHAPE_ENEMY_LASER;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    g.objs.active_move_after(shot, firer);
    g.objs.aliens[firer as usize].immuneptr = shot;
    let init = sid(g, elaser_istrat);
    g.objs.aliens[shot as usize].stratptr = Some(init);
    // ROM `jsl lasersound_l` (GSTRATS.ASM:2603).
    make_firer_snd(g, firer, PosSndFamilyId::Laser);
    let _ = make_laser_flash(g, shot);
    Some(shot)
}

/// ROM `Yhoming_Istrat` (GSTRATS.ASM:1752) — yaw-only home toward `al_ptr`.
pub fn yhoming_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, yhoming_strat);
    let rnd = (sf_random(&mut g.vars) & 7) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.snd2 = 6;
        al.animframe = 0x80 | rnd;
    }
}

/// ROM `Yhoming_strat` (GSTRATS.ASM:1759).
pub fn yhoming_strat(g: &mut Game, idx: u16) {
    // s_add_anim #1,#8
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut f = (al.animframe & 0x7F).wrapping_add(1);
        if f >= 8 {
            f = f.wrapping_sub(8);
        }
        al.animframe = 0x80 | f;
    }
    // ROM: s_set_objtobealvar y,x,al_ptr (not fireobjptr).
    let ti = {
        let ptr = g.objs.aliens[idx as usize].ptr;
        if ptr == 0 {
            None
        } else {
            let t = ptr as i32 - 1;
            if t >= 0 && (t as usize) < NUMBER_AL {
                Some(t as u16)
            } else {
                None
            }
        }
    };
    if let Some(ti) = ti {
        if g.objs.aliens[ti as usize].active {
            let target = g.objs.aliens[ti as usize];
            // s_obj2obj_angle …,0 → snap yaw (achase shift 0).
            strat_aim_yaw(g, idx, &target, 0);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        crate::common::strat_gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            // s_dec_lifecnt (no ,1) → remove
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `fire_YHplasma` (GSTRATS.ASM:2532).
pub fn fire_yhplasma(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, 266)?;
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    place_weapon_at_firer(g, shot, firer, mz);
    let coll = sid(g, weapcollide_istrat);
    let rem = sid(g, |g, _| {
        g.objs.aldead = 1;
    });
    let target = nuke_player_idx(g).unwrap_or(0);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = 8;
        al.vel = 100;
        al.count = 50;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE4;
        al.type_ |= ATLASER;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
        al.sflags2 |= ASF2_RELEXPLODE;
        al.ptr = target.wrapping_add(1);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(rem);
    }
    yhoming_istrat(g, shot);
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2544).
    let (fx, fz) = {
        let f = &g.objs.aliens[firer as usize];
        (f.worldx, f.worldz)
    };
    g.hooks.make_snd(PosSndFamilyId::EnemyBattry, fx, fz);
    Some(shot)
}

// ============================================================
// HELPBALL (GSTRATS.ASM:2233-2327)
// ============================================================

const NUM_HELP_SHOTS: u8 = 10;
pub const SH_HELPBALL: u16 = 226;
/// Extended shape-catalog id for the source's distinct homing helper shot.
pub const SH_SHELPBALL: u16 = 406;

/// Orbit helpball around player: `s_add_Roffs2pos B,x,player,x,#0,radius,#60,0,0,1`
/// (Z/roll only via `rotate_8yx`, angle = self.rotz).
fn helpball_orbit_pos(g: &mut Game, idx: u16, player: u16, radius: u8) {
    let rotz = g.objs.aliens[idx as usize].rotz;
    let (dx, dy, dz) = crate::snes_trig::strat_roffs_roll(rotz, 0, radius as i8, 60);
    let pl = g.objs.aliens[player as usize];
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = pl.worldx.wrapping_add(dx);
    al.worldy = pl.worldy.wrapping_add(dy);
    al.worldz = pl.worldz.wrapping_add(dz);
}

/// ROM `helpball_Istrat` (GSTRATS.ASM:2235).
pub fn helpball_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, helpball_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_HELPBALL;
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.sbyte3 = 30;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
    }
}

/// ROM `helpball_strat` (GSTRATS.ASM:2241).
pub fn helpball_strat(g: &mut Game, idx: u16) {
    use sf_game::alien::ASF3_LOCKON;
    use sf_game::draw::AF_INVIEW_PL;
    use sf_game::vars::HARD_HP;

    let Some(player) = nuke_player_idx(g) else {
        g.objs.aldead = 1;
        return;
    };

    // Lifetime: after numhelpshots fired, expand radius then remove.
    if g.objs.aliens[idx as usize].sbyte2 >= NUM_HELP_SHOTS {
        let r = g.objs.aliens[idx as usize].sbyte3.wrapping_add(3);
        g.objs.aliens[idx as usize].sbyte3 = r;
        if r >= 120 {
            g.objs.aldead = 1;
            return;
        }
    }

    {
        let radius = g.objs.aliens[idx as usize].sbyte3;
        helpball_orbit_pos(g, idx, player, radius);
        g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(12);
    }

    // Cap concurrent homes at 3 (sbyte1).
    if g.objs.aliens[idx as usize].sbyte1 >= 3 {
        return;
    }

    let self_pos = {
        let al = &g.objs.aliens[idx as usize];
        (al.worldx, al.worldz)
    };
    let actives = g.objs.active_indices();
    for &yi in &actives {
        if yi == idx || yi == player {
            continue;
        }
        if g.objs.aliens[idx as usize].sbyte1 >= 3 {
            break;
        }
        let yal = g.objs.aliens[yi as usize];
        if yal.sflags3 & ASF3_REALOBJ == 0 {
            continue;
        }
        let mut probe = Alien::default();
        probe.worldx = self_pos.0;
        probe.worldz = self_pos.1;
        let d = crate::common::strat_dist_xz(&probe, &yal);
        if d < 300 || d >= 4000 {
            continue;
        }
        if yal.flags & AF_INVIEW_PL == 0 {
            continue;
        }
        if yal.sflags & (ASF_NOHITAFFECT | ASF_COLLDISABLE) != 0 {
            continue;
        }
        if yal.collflags & ACF_COLLTYPE5 != 0 {
            continue; // friend
        }
        if yal.sflags3 & ASF3_LOCKON != 0 {
            continue;
        }
        if yal.hp == HARD_HP {
            continue;
        }

        // Lock and spawn shelpball home.
        g.objs.aliens[yi as usize].sflags3 |= ASF3_LOCKON;
        let Some(home) = make_obj(g, SH_SHELPBALL) else {
            g.objs.aliens[yi as usize].sflags3 &= !ASF3_LOCKON;
            break;
        };
        {
            let src = g.objs.aliens[idx as usize];
            let al = &mut g.objs.aliens[home as usize];
            al.worldx = src.worldx;
            al.worldy = src.worldy;
            al.worldz = src.worldz;
            al.ptr = yi.wrapping_add(1);
            al.sword1 = idx as i16; // mother helpball
        }
        helpballhome_istrat(g, home);
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
        g.objs.aliens[idx as usize].sbyte2 = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
    }
}

/// ROM `helpballhome_Istrat` (GSTRATS.ASM:2285).
pub fn helpballhome_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, helpballhome_strat);
    let coll = sid(g, helpball_hcoll_istrat);
    let exp = sid(g, helpball_hrem_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = 1;
        al.ap = 20;
        al.vel = 40;
        al.count = 70;
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE5; // laser + friend
        al.type_ |= ATLASER;
        al.type_ &= !ATZREMOVE;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.sbyte1 = al.roty;
        al.sbyte2 = al.rotx;
    }
}

/// ROM `helpballhome_strat` (GSTRATS.ASM:2295).
pub fn helpballhome_strat(g: &mut Game, idx: u16) {
    let ti = {
        let ptr = g.objs.aliens[idx as usize].ptr;
        if ptr == 0 {
            None
        } else {
            let t = ptr as i32 - 1;
            if t >= 0 && (t as usize) < NUMBER_AL && g.objs.aliens[t as usize].active {
                Some(t as u16)
            } else {
                None
            }
        }
    };
    let Some(ti) = ti else {
        helpball_hrem_istrat(g, idx);
        return;
    };
    // s_obj2obj_3dangle into sbyte1/sbyte2 with chase 0 (snap) — Yanglexy+nega.
    let target = g.objs.aliens[ti as usize];
    let me = g.objs.aliens[idx as usize];
    let mut yaw = me.sbyte1;
    let mut pitch = me.sbyte2;
    achase_angle(&mut yaw, angle_xz(&me, &target).wrapping_neg(), 0);
    achase_angle(&mut pitch, strat_pitch_toward(&me, &target), 0);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = yaw;
        al.sbyte2 = pitch;
        al.roty = yaw;
        al.rotx = pitch;
        crate::common::strat_gen_vecs_3d(al);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            crate::common::kill_obj(al);
            return;
        }
        al.count = c;
    }
}

/// ROM `helpballHcoll_Istrat` (GSTRATS.ASM:2307).
pub fn helpball_hcoll_istrat(g: &mut Game, idx: u16) {
    let target_ptr = g.objs.aliens[idx as usize].ptr;
    let partner = g.objs.aliens[idx as usize].collobjptr;
    // collobjptr is raw index (not +1) in this port.
    let target_idx = if target_ptr == 0 {
        None
    } else {
        Some(target_ptr.wrapping_sub(1))
    };
    if target_idx == Some(partner) {
        // coll_Istrat → damage self; hp=1 dies into expstrat.
        weapcollide_istrat(g, idx);
    } else {
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
        helpballhome_strat(g, idx);
    }
}

/// ROM `helpballHrem_Istrat` (GSTRATS.ASM:2320).
pub fn helpball_hrem_istrat(g: &mut Game, idx: u16) {
    use sf_game::alien::ASF3_LOCKON;
    // Clear lockon on target.
    let ptr = g.objs.aliens[idx as usize].ptr;
    if ptr != 0 {
        let ti = ptr.wrapping_sub(1);
        if (ti as usize) < NUMBER_AL {
            g.objs.aliens[ti as usize].sflags3 &= !ASF3_LOCKON;
        }
    }
    // Dec mother's active-home count (sbyte1).
    let mother = g.objs.aliens[idx as usize].sword1 as u16;
    if (mother as usize) < NUMBER_AL && g.objs.aliens[mother as usize].active {
        let n = g.objs.aliens[mother as usize].sbyte1;
        if n > 0 {
            g.objs.aliens[mother as usize].sbyte1 = n - 1;
        }
    }
    g.objs.aldead = 1;
}

// ============================================================
// MISSILE / HMISSILE fire family (GSTRATS.ASM:1448-1865, 2609-2764)
// ============================================================

pub const SH_MISSILE: u16 = 403;
const HMISSILE1_HP: u8 = 2;
const MISSILE2_HP: u8 = 2;
const MISSILE2_AP: u8 = 4;
const MISSILE_FIRE_SPEED: u8 = 30;
const HMISSILE_FIRE_SPEED: u8 = 60;
const MISSILE_FIRE_LIFE: u8 = 100;
const HMISSILE2_STRAIGHT_FRAMES: u8 = 25;
const HMISSILE_CLOSE_DIST: i32 = 300;

/// Resolve ROM `al_ptr` (index+1); fall back to `fireobjptr` for boss helpers.
fn missile_home_target(g: &Game, idx: u16) -> Option<u16> {
    let al = &g.objs.aliens[idx as usize];
    for ptr in [al.ptr, al.fireobjptr] {
        if ptr == 0 {
            continue;
        }
        let t = ptr as i32 - 1;
        if t >= 0 && (t as usize) < NUMBER_AL && g.objs.aliens[t as usize].active {
            return Some(t as u16);
        }
    }
    None
}

/// `s_gen_3dvecs` + `s_addgen_3dvecs` with mother speed in `sbyte3`.
fn missile_gen_with_mother(al: &mut Alien) {
    let bolt = al.vel;
    let mother = al.sbyte3;
    crate::common::strat_gen_vecs_3d(al);
    let (bx, by, bz) = (al.vx, al.vy, al.vz);
    al.vel = mother;
    crate::common::strat_gen_vecs_3d(al);
    al.vx = al.vx.wrapping_add(bx);
    al.vy = al.vy.wrapping_add(by);
    al.vz = al.vz.wrapping_add(bz);
    al.vel = bolt;
}

/// Shared gen_weapon-ish setup used by fire_* missile helpers.
fn place_missile_weapon(g: &mut Game, shot: u16, firer: u16) {
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    place_weapon_at_firer(g, shot, firer, mz);
    let firer_ptr = g.objs.aliens[firer as usize].ptr;
    let target = if firer_ptr != 0 {
        firer_ptr
    } else {
        nuke_player_idx(g).unwrap_or(0).wrapping_add(1)
    };
    let al = &mut g.objs.aliens[shot as usize];
    al.rotz = 0;
    al.ptr = target;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.sflags4 &= !ASF4_INVISIBLE;
    al.collflags |= ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4; // enemyweap
                                                                 // Raw slot index; the reciprocal side was written by
                                                                 // `place_weapon_at_firer`, matching ROM `s_make_immune`.
    al.immuneptr = firer;
}

fn finish_missile_fire(g: &mut Game, firer: u16) {
    // ROM `jsl missilesound_l` (GSTRATS.ASM:2623+).
    make_firer_snd(g, firer, PosSndFamilyId::Missile);
}

/// ROM `hmissile1_Istrat` (GSTRATS.ASM:1448) — init for FakeFar / fire_Hmissile1.
pub fn hmissile1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, hmissile1_rom_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        missile_gen_with_mother(al);
    }
}

/// ROM-faithful `hmissile1_strat` body (GSTRATS.ASM:1459) using `al_ptr`.
/// Boss helpers keep the older `hmissile1_strat` (fireobjptr / no mother addgen).
pub fn hmissile1_rom_strat(g: &mut Game, idx: u16) {
    // Optional FakeFar anim when sflag3 set.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG3 != 0 {
        let frame = g.objs.aliens[idx as usize].animframe & 0x7F;
        if frame != 15 {
            let mut f = frame.wrapping_add(1);
            if f >= 16 {
                f = f.wrapping_sub(16);
            }
            g.objs.aliens[idx as usize].animframe = 0x80 | f;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
        if let Some(t) = missile_home_target(g, idx) {
            let me = g.objs.aliens[idx as usize];
            let tgt = g.objs.aliens[t as usize];
            let dist = (me.worldx as i32 - tgt.worldx as i32).abs()
                + (me.worldy as i32 - tgt.worldy as i32).abs()
                + (me.worldz as i32 - tgt.worldz as i32).abs();
            if dist < HMISSILE_CLOSE_DIST {
                g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
            } else {
                strat_aim_3d(g, idx, &tgt, 3);
                missile_gen_with_mother(&mut g.objs.aliens[idx as usize]);
            }
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `hmissile2_Istrat` (GSTRATS.ASM:1500) — straight for 25 frames, then quick turn.
pub fn hmissile2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, hmissile2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        al.sbyte1 = HMISSILE2_STRAIGHT_FRAMES;
        missile_gen_with_mother(al);
    }
}

/// ROM `hmissile2_strat` (GSTRATS.ASM:1512).
pub fn hmissile2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    // s_decbne_alvar sbyte1 — while >0 after dec, skip home (straight flight).
    let delay = g.objs.aliens[idx as usize].sbyte1;
    let after = delay.wrapping_sub(1);
    if after != 0 {
        g.objs.aliens[idx as usize].sbyte1 = after;
    } else {
        g.objs.aliens[idx as usize].sbyte1 = 1;
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
            if let Some(t) = missile_home_target(g, idx) {
                let me = g.objs.aliens[idx as usize];
                let tgt = g.objs.aliens[t as usize];
                let dist = (me.worldx as i32 - tgt.worldx as i32).abs()
                    + (me.worldy as i32 - tgt.worldy as i32).abs()
                    + (me.worldz as i32 - tgt.worldz as i32).abs();
                if dist < HMISSILE_CLOSE_DIST {
                    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
                } else {
                    strat_aim_3d(g, idx, &tgt, 1);
                    missile_gen_with_mother(&mut g.objs.aliens[idx as usize]);
                }
            }
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `missile1_Istrat` (GSTRATS.ASM:1808) — chase stored aim in sbyte1/2.
pub fn missile1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, missile1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
    }
}

/// ROM `missile1_strat` (GSTRATS.ASM:1816).
pub fn missile1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
        al.rotz = al.rotz.wrapping_add(10);
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let (want_x, want_y) = {
            let al = &g.objs.aliens[idx as usize];
            (al.sbyte1, al.sbyte2)
        };
        {
            let mut rotx = g.objs.aliens[idx as usize].rotx;
            let mut roty = g.objs.aliens[idx as usize].roty;
            achase_angle(&mut roty, want_y, 2);
            achase_angle(&mut rotx, want_x, 2);
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = rotx;
            al.roty = roty;
        }
        missile_gen_with_mother(&mut g.objs.aliens[idx as usize]);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    miss_end(g, idx);
}

/// ROM `missile2_Istrat` (GSTRATS.ASM:1834).
pub fn missile2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, missile2a_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.sbyte1;
        al.roty = al.sbyte2;
        al.stratptr = Some(tick);
        al.type_ |= ATZREMOVE; // s_setremove_behind
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
    }
}

/// ROM `missile2a_strat` (GSTRATS.ASM:1846).
pub fn missile2a_strat(g: &mut Game, idx: u16) {
    // Speed up toward 100 when within 600 Z of player (unless sflag2).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let dz = (me.worldz as i32 - pl.worldz as i32).abs();
            if dz <= 600 {
                let _ = speed_to(&mut g.objs.aliens[idx as usize], 100, 5);
            }
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        crate::common::strat_gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        let mut roty = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut rotx, 0, 2);
        achase_angle(&mut roty, DEG180, 2);
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = rotx;
        al.roty = roty;
        al.rotz = al.rotz.wrapping_add(10);
    }
    miss_end(g, idx);
}

/// ROM `fire_FakeFarHmissile1` (GSTRATS.ASM:2609) — hmissile1 + sflag3 anim.
pub fn fire_fakefar_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = HMISSILE1_HP;
        al.ap = HMISSILE1_AP;
        al.vel = HMISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE | ASF2_SFLAG3;
        al.animframe = 0x80; // s_init_anim #0
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    hmissile1_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_Hmissile2` (GSTRATS.ASM:2653).
pub fn fire_hmissile2(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = HMISSILE1_HP;
        al.ap = HMISSILE1_AP;
        al.vel = HMISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    hmissile2_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_bossHmissile1` (GSTRATS.ASM:2667) — fire_Hmissile1 + explodegate2.
pub fn fire_boss_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = fire_hmissile1(g, firer)?;
    let exp = sid(g, explodegate2_istrat);
    g.objs.aliens[shot as usize].expstratptr = Some(exp);
    Some(shot)
}

/// ROM `fire_Hmissile1` (GSTRATS.ASM:2627) — relative homing + smoke puff.
pub fn fire_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = HMISSILE1_HP;
        al.ap = HMISSILE1_AP;
        al.vel = HMISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    hmissile1_istrat(g, shot);
    // Smoke puff at missile position (ROM make_obj #smoke → puff_Istrat).
    if let Some(puff) = make_obj(g, 358) {
        let (px, py, pz) = {
            let m = &g.objs.aliens[shot as usize];
            (m.worldx, m.worldy, m.worldz)
        };
        {
            let al = &mut g.objs.aliens[puff as usize];
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
            al.sflags3 &= !ASF3_REALOBJ;
        }
        crate::common::puff_istrat(g, puff);
    }
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_missile2` (GSTRATS.ASM:2740).
pub fn fire_missile2(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = MISSILE2_HP;
        al.ap = MISSILE2_AP;
        al.vel = MISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    missile2_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_missile1` (GSTRATS.ASM:2754) — no relexplode.
pub fn fire_missile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = MISSILE2_HP;
        al.ap = MISSILE2_AP;
        al.vel = MISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    missile1_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

// ============================================================
// Specialty hmissiles: kami / chick / STB / QH (GSTRATS.ASM:1544-1715, 2682-2735)
// ============================================================

const KAMI_HMISSILE_HP: u8 = 2;
const KAMI_HMISSILE_AP: u8 = 8;
const KAMI_HMISSILE_SPEED: u8 = 40;
const CHICK_HMISSILE_AP: u8 = 40; // hmissile1AP*5
const CHICK_HMISSILE_SPEED: u8 = 30;
const CHICK_HMISSILE_LIFE: u8 = 30;
const QH_MISSILE_AP: u8 = 50;
const STB_AIM_DELAY: u8 = 20;
const STB_CLOSE_DIST: i16 = 600;
const CHICK_NEAR_DIST: i16 = 400;

/// `s_jmp_notdelay N` — true when `(gameframe & ((1<<N)-1)) == 0` (gate open).
fn missile_notdelay(g: &Game, bits: u32) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

/// D-pad held on cont0 (VARS.INC key_j* in low byte of pad1).
fn any_joy_dir_down(g: &Game) -> bool {
    use sf_core::pad;
    g.vars.pad1 & (pad::LEFT | pad::RIGHT | pad::UP | pad::DOWN) != 0
}

/// ROM `hmissile3_Istrat` (GSTRATS.ASM:1544) — kamikaze zaco_9 that shoots lasers.
pub fn hmissile3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, hmissile3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        missile_gen_with_mother(al);
    }
}

/// ROM `hmissile3_strat` (GSTRATS.ASM:1554).
pub fn hmissile3_strat(g: &mut Game, idx: u16) {
    let target = missile_home_target(g, idx);
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
        if let Some(t) = target {
            let me = g.objs.aliens[idx as usize];
            let tgt = g.objs.aliens[t as usize];
            let dz = (me.worldz as i32 - tgt.worldz as i32).abs();
            // Twin RELSLOWELASER when |dz| in [1000,2000) on notdelay-3.
            if (1000..2000).contains(&dz) && missile_notdelay(g, 3) {
                strat_fire_relslowlaser(g, idx, 0, DEG5);
                strat_fire_relslowlaser(g, idx, 0, 0u8.wrapping_sub(DEG5));
            }
            if dz < 500 {
                g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
            } else if (500..2000).contains(&dz) {
                strat_aim_3d(g, idx, &tgt, 0);
                missile_gen_with_mother(&mut g.objs.aliens[idx as usize]);
            }
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `chickhmissile1_Istrat` (GSTRATS.ASM:1595).
pub fn chickhmissile1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, chickhmissile1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        al.rotz = DEG180;
    }
}

/// ROM `chickhmissile1_strat` (GSTRATS.ASM:1603) — homes on player, not al_ptr.
pub fn chickhmissile1_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        miss_end(g, idx);
        return;
    };
    // Chase world x/y toward player_pos (rate 3).
    {
        let px = g.vars.player_posx;
        let py = g.vars.player_posy;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, px, 3);
        al.worldy = chase_proportional(al.worldy, py, 3);
    }
    let near = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0;
    let dist = crate::common::strat_dist_xz(&g.objs.aliens[idx as usize], &pl);
    if near || (dist < CHICK_NEAR_DIST && !any_joy_dir_down(g)) {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
        g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
        {
            let al = &mut g.objs.aliens[idx as usize];
            let c = al.count.wrapping_sub(1);
            if c == 0 {
                g.objs.aldead = 1;
                return;
            }
            al.count = c;
        }
        // Pitch toward/away from viewcy based on player height.
        let viewcy = g.vars.sv_i16(crate::common::sv::VIEWCY);
        if pl.worldy >= viewcy {
            // player lower (or equal) → pitch down (+2)
            g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(2);
        } else {
            g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(2);
        }
    } else {
        strat_aim_3d(g, idx, &pl, 0);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        crate::common::strat_gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    miss_end(g, idx);
}

/// ROM `STBhmissile1_Istrat` (GSTRATS.ASM:1645).
pub fn stbhmissile1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, stbhmissile1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ZENEMY;
        al.vel = 10;
        al.sbyte3 = STB_AIM_DELAY;
    }
}

/// ROM `STBhmissile1_strat` (GSTRATS.ASM:1657) — vector angles in sbyte1/2.
pub fn stbhmissile1_strat(g: &mut Game, idx: u16) {
    {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 2);
    }
    if g.objs.aliens[idx as usize].vel == 60 {
        // ROM: only spins once vel hits 60 (`.nzr` falls through when equal).
        // Actually ASM: jmp_alvarNE vel,#60,.nzr then .nzr always adds rotz —
        // so rotz always increments. Keep always-spin.
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 {
        if let Some(t) = missile_home_target(g, idx) {
            let dist = crate::common::strat_dist_xz(
                &g.objs.aliens[idx as usize],
                &g.objs.aliens[t as usize],
            );
            if dist < STB_CLOSE_DIST {
                g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2;
            } else {
                let delay = g.objs.aliens[idx as usize].sbyte3;
                let after = delay.wrapping_sub(1);
                if after != 0 {
                    g.objs.aliens[idx as usize].sbyte3 = after;
                } else {
                    g.objs.aliens[idx as usize].sbyte3 = 1;
                    let tgt = g.objs.aliens[t as usize];
                    strat_aim_3d(g, idx, &tgt, 4);
                }
                // Chase sbyte1/2 toward live roty/rotx (rate 3).
                let (want_y, want_x) = {
                    let al = &g.objs.aliens[idx as usize];
                    (al.roty, al.rotx)
                };
                {
                    let mut sb1 = g.objs.aliens[idx as usize].sbyte1;
                    let mut sb2 = g.objs.aliens[idx as usize].sbyte2;
                    achase_angle(&mut sb1, want_y, 3);
                    achase_angle(&mut sb2, want_x, 3);
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte1 = sb1;
                    al.sbyte2 = sb2;
                    // gen_3dvecs from sbyte1/sbyte2 (vector angles).
                    let save_rx = al.rotx;
                    let save_ry = al.roty;
                    al.roty = al.sbyte1;
                    al.rotx = al.sbyte2;
                    crate::common::strat_gen_vecs_3d(al);
                    al.rotx = save_rx;
                    al.roty = save_ry;
                }
            }
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `Qhmissile1_Istrat` (GSTRATS.ASM:1697).
pub fn qhmissile1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, qhmissile1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.snd2 = 2;
        al.sflags |= ASF_SHADOW;
    }
}

/// ROM `Qhmissile1_strat` (GSTRATS.ASM:1703) — snap-aim every frame.
pub fn qhmissile1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(10);
    }
    if let Some(t) = missile_home_target(g, idx) {
        let tgt = g.objs.aliens[t as usize];
        strat_aim_3d(g, idx, &tgt, 0);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        crate::common::strat_gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    miss_end(g, idx);
}

/// ROM `fire_kamiHmissile1` (GSTRATS.ASM:2682).
pub fn fire_kami_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, 269)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = KAMI_HMISSILE_HP;
        al.ap = KAMI_HMISSILE_AP;
        al.vel = KAMI_HMISSILE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    hmissile3_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_chickHmissile1` (GSTRATS.ASM:2696).
pub fn fire_chick_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, 417)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = HARD_HP;
        al.ap = CHICK_HMISSILE_AP;
        al.vel = CHICK_HMISSILE_SPEED;
        al.count = CHICK_HMISSILE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    chickhmissile1_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_STBHmissile1` (GSTRATS.ASM:2710).
pub fn fire_stb_hmissile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = HMISSILE1_HP;
        al.ap = HMISSILE1_AP;
        // speed set in istrat (#10); life 100
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collflags |= COLLTYPE_ENEMY2; // enemy2
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    stbhmissile1_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

/// ROM `fire_QHmissile1` (GSTRATS.ASM:2724).
pub fn fire_qh_missile1(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, stopexplode_istrat);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = 1;
        al.ap = QH_MISSILE_AP;
        al.vel = HMISSILE_FIRE_SPEED;
        al.count = MISSILE_FIRE_LIFE;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    qhmissile1_istrat(g, shot);
    finish_missile_fire(g, firer);
    Some(shot)
}

// ============================================================
// SPREAD missile + DefElaserCol (GSTRATS.ASM:814-839, 2190-2228, 2769)
// ============================================================

const SPREAD_SPEED: u8 = 40;
const SPREAD_LIFE: u8 = 50;
const SPREAD_ARM_FRAMES: u8 = 10;

/// Resume the object's main strat after a collide handler (`s_jmpto_strat`).
fn jmpto_strat(g: &mut Game, idx: u16) {
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
}

/// ROM `DefElaserCol_Istrat` (GSTRATS.ASM:814) — deflect player laser as RebElaser.
pub fn defelasercol_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    let partner = g.objs.aliens[idx as usize].collobjptr;
    let partner_ok =
        partner != 0 && (partner as usize) < NUMBER_AL && g.objs.aliens[partner as usize].active;
    if partner_ok && g.objs.aliens[partner as usize].shape == SHAPE_ELASER2 {
        // Fire the rebound from the laser, with yaw+90 and random pitch.
        let saved = {
            let laser = &g.objs.aliens[partner as usize];
            (laser.roty, laser.rotx, laser.vel, laser.immuneptr)
        };
        {
            let laser = &mut g.objs.aliens[partner as usize];
            laser.vel = 0;
            laser.roty = laser.roty.wrapping_add(DEG90);
            laser.rotx = (sf_random(&mut g.vars) & 0xff) as u8;
        }
        let _ = fire_reb_elaser(g, partner);
        {
            let laser = &mut g.objs.aliens[partner as usize];
            laser.roty = saved.0;
            laser.rotx = saved.1;
            laser.vel = saved.2;
            laser.immuneptr = saved.3;
        }
    }
    jmpto_strat(g, idx);
}

/// ROM `spread_Istrat` (GSTRATS.ASM:2190).
pub fn spread_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, spread_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte3 = SPREAD_ARM_FRAMES;
        al.collflags |= ACF_COLLTYPE5 | ACF_COLLTYPE4; // friend + enemyweap
        crate::common::strat_gen_vecs_3d(al);
    }
}

/// ROM `spread_strat` (GSTRATS.ASM:2198) — coast then `spreada_init`.
pub fn spread_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar: branch if already 0, else decrement.
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        spreada_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_sub(1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(8);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        al.count = c;
    }
    add_player_z(g, idx);
    miss_end(g, idx);
}

/// ROM `spreadA_init` (GSTRATS.ASM:2209) — spawn QHMISSILE1 at every valid target, then explode.
pub fn spreada_init(g: &mut Game, idx: u16) {
    let actives = g.objs.active_indices();
    for &yi in &actives {
        if yi == idx {
            continue;
        }
        let yal = g.objs.aliens[yi as usize];
        if yal.collflags & ACF_WEAPON != 0 {
            continue;
        }
        if yal.sflags & ASF_COLLDISABLE != 0 {
            continue;
        }
        if yal.hp == HARD_HP {
            continue;
        }
        if let Some(shot) = fire_qh_missile1(g, idx) {
            g.objs.aliens[shot as usize].ptr = yi.wrapping_add(1);
        }
    }
    strat_explode(g, idx);
}

/// ROM `fire_spread` (GSTRATS.ASM:2769).
pub fn fire_spread(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_MISSILE)?;
    place_missile_weapon(g, shot, firer);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.hp = MISSILE2_HP;
        al.ap = MISSILE2_AP;
        al.vel = SPREAD_SPEED;
        al.count = SPREAD_LIFE;
        // fire_spread does not set relexplode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    spread_istrat(g, shot);
    Some(shot)
}

// ============================================================
// BONFIRE + IRONBALL4 / IRONBALLMISSILE (D3STRATS.ASM:933-1035, DSTRATS:8184)
// ============================================================

const IRONBALL_AP: u8 = 6;
const BALL_MISSILE_HP: u8 = 6;
const BALL_MISSILE_AP: u8 = 16;
const SHAPE_FIREBALL: u16 = 402;
const SHAPE_IRONBALL: u16 = 404;
const BONFIRE_SPEED: u8 = 120;
const BONFIRE_TRAIL_LIFE: u8 = 10;
const IRONBALL_BASE_VEL: u8 = 103 - 7; // + (rnd&7)

/// `s_falldown_Yvec x,2,#1,#0` — gravity + ground bounce (bossh twin).
fn ironball_falldown_yvec(al: &mut Alien) {
    const IRONBALL_BOUNCE_GRAVITY: i16 = 1;
    fall_down_y_vector(al, DEFAULT_BOUNCE_SHIFT, IRONBALL_BOUNCE_GRAVITY, GROUND_Y);
}

/// Chase `current` toward `target` by at most `step` (ROM `s_fchase_alvar2alvar`).
fn fchase_i16(current: &mut i16, target: i16, step: i16) {
    let d = target.wrapping_sub(*current);
    if d > step {
        *current = current.wrapping_add(step);
    } else if d < -step {
        *current = current.wrapping_sub(step);
    } else {
        *current = target;
    }
}

/// ROM `fire_bonfire` (D3STRATS.ASM:933).
pub fn fire_bonfire(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_FIREBALL)?;
    {
        let src = g.objs.aliens[firer as usize];
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = src.worldx;
        al.worldy = 0; // s_set_alvar W,y,al_worldy,#0
        al.worldz = src.worldz;
        al.collflags |= COLLTYPE_ENEMY1;
        al.shape = SHAPE_FIREBALL;
    }
    // Aim at the player from the newly spawned ball.
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[shot as usize];
        let yaw = angle_xz(&me, &pl).wrapping_neg();
        let pitch = strat_pitch_toward(&me, &pl);
        let al = &mut g.objs.aliens[shot as usize];
        al.roty = yaw;
        al.rotx = pitch;
    }
    bonfire_istrat(g, shot);
    g.hooks.play_se(0x99);
    Some(shot)
}

/// ROM `bonfire_istrat` (D3STRATS.ASM:948) — hardHP fireball + trail sparks.
pub fn bonfire_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bonfire_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sflags |= ASF_NOHITAFFECT;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.vel = BONFIRE_SPEED;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
        crate::common::strat_gen_vecs_3d(al);
    }
}

/// ROM bonfire `.strat` (D3STRATS.ASM:957) — spawn trail, move, scroll.
pub fn bonfire_strat(g: &mut Game, idx: u16) {
    if let Some(trail) = make_obj(g, SHAPE_FIREBALL) {
        let (px, py, pz) = {
            let m = &g.objs.aliens[idx as usize];
            (m.worldx, m.worldy, m.worldz)
        };
        {
            let al = &mut g.objs.aliens[trail as usize];
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
            al.sflags |= ASF_COLLDISABLE;
            al.sbyte1 = 0;
        }
        bonfire_trail_istrat(g, trail);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// ROM bonfire `.dummdumm_strat` init (D3STRATS.ASM:967).
fn bonfire_trail_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bonfire_trail_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_COLLDISABLE;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 1;
        al.tx = 0;
    }
}

/// ROM bonfire trail tick — live 10 frames then remove.
pub fn bonfire_trail_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 >= BONFIRE_TRAIL_LIFE {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
    add_player_z(g, idx);
}

/// ROM `fire_ironball4` (D3STRATS.ASM:981) — aim at player + random cone, sflag1.
pub fn fire_ironball4(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_IRONBALL)?;
    {
        let src = g.objs.aliens[firer as usize];
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotx = src.rotx;
        al.roty = src.roty;
        al.rotz = src.rotz;
        al.collflags |= COLLTYPE_ENEMY1;
        al.shape = SHAPE_IRONBALL;
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[shot as usize];
        // ROM `s_obj2obj_3dangle` (chase 0) — Yanglexy+nega into ball rots.
        let yaw = angle_xz(&me, &pl).wrapping_neg();
        let pitch = strat_pitch_toward(&me, &pl);
        let al = &mut g.objs.aliens[shot as usize];
        al.roty = yaw;
        al.rotx = pitch;
    }
    // Random cone: rotx += (rnd&(deg22-1)) - deg11; roty += (rnd&(deg90-1)) - deg45.
    let rx = (sf_random(&mut g.vars) as u8) & (DEG22.wrapping_sub(1));
    let ry = (sf_random(&mut g.vars) as u8) & (DEG90.wrapping_sub(1));
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = al.rotx.wrapping_add(rx).wrapping_sub(DEG11);
        al.roty = al.roty.wrapping_add(ry).wrapping_sub(DEG45);
        al.sflags2 |= ASF2_SFLAG1; // sflag1 → faster
    }
    ironball_istrat(g, shot);
    g.hooks.play_se(0x49);
    Some(shot)
}

/// ROM `ironball_istrat` (DSTRATS.ASM:8184).
pub fn ironball_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, ironball_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let rnd = (sf_random(&mut g.vars) as u8) & 7;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = IRONBALL_AP;
        al.sflags |= ASF_NOHITAFFECT | ASF_SHADOW;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
        al.vel = IRONBALL_BASE_VEL.wrapping_add(rnd);
        if al.sflags2 & ASF2_SFLAG1 != 0 {
            al.vel = al.vel.wrapping_add(20);
        }
        crate::common::strat_gen_vecs_3d(al);
    }
}

/// ROM ironball `.strat` (DSTRATS.ASM:8198) — fall / home / scroll.
pub fn ironball_strat(g: &mut Game, idx: u16) {
    const POWER_BUILD_THRESHOLD: u8 = 128;
    const POWER_BUILD_RISE: i16 = 100;
    const POWER_BUILD_MINIMUM_Y: i16 = -2000;
    const POWER_BUILD_DEPTH_THRESHOLD: i16 = 1000;
    const POWER_BUILD_DEPTH_STEP: i16 = 50;
    const POWER_BUILD_JITTER_MASK: u8 = 7;
    const POWER_BUILD_JITTER_CENTER: u8 = 4;
    const NORMAL_X_CHASE_STEP: i16 = 1;
    const STRONG_X_CHASE_PASSES: usize = 6;
    const STRONG_DEPTH_STEP: i16 = 60;

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0 {
        if let Some(pl) = player(g) {
            let depth_difference = g.objs.aliens[idx as usize].worldz.wrapping_sub(pl.worldz);
            if depth_difference >= POWER_BUILD_DEPTH_THRESHOLD {
                g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
                    .worldz
                    .wrapping_sub(POWER_BUILD_DEPTH_STEP);
            }
            if g.objs.aliens[idx as usize].worldy < POWER_BUILD_MINIMUM_Y {
                g.objs.aliens[idx as usize].worldy = POWER_BUILD_MINIMUM_Y;
            }

            if g.vars.shared.power_build < POWER_BUILD_THRESHOLD {
                g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize]
                    .worldy
                    .wrapping_sub(POWER_BUILD_RISE);
                add_player_z(g, idx);
                return;
            }

            g.vars.shared.power_build = g.vars.shared.power_build.wrapping_sub(1);
            if g.vars.shared.power_build == POWER_BUILD_THRESHOLD {
                g.vars.shared.power_build = 0;
            }

            strat_aim_3d(g, idx, &pl, 0);
            let jitter = ((sf_random(&mut g.vars) as u8) & POWER_BUILD_JITTER_MASK)
                .wrapping_sub(POWER_BUILD_JITTER_CENTER);
            let view_pitch = (g.vars.strategy.view_pitch >> 8) as u8;
            let view_yaw = (g.vars.strategy.view_yaw >> 8) as u8;
            let player_turn = (g.vars.strategy.player_turn_rotation >> 8) as u8;
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = al.roty.wrapping_add(jitter);
                al.rotx = al.rotx.wrapping_add(jitter);
                crate::common::strat_gen_vecs_3d(al);
                al.roty = view_yaw
                    .wrapping_neg()
                    .wrapping_add(DEG180)
                    .wrapping_add(player_turn);
                al.rotx = view_pitch;
                al.vx = al.vx.wrapping_add(al.vx);
                al.vy = al.vy.wrapping_add(al.vy);
                al.vz = al.vz.wrapping_add(al.vz);
                al.sflags2 &= !ASF2_SFLAG2;
                al.sflags2 |= ASF2_SFLAG1;
            }
        } else {
            g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize]
                .worldy
                .wrapping_sub(POWER_BUILD_RISE);
        }
    } else if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        ironball_falldown_yvec(&mut g.objs.aliens[idx as usize]);
        apply_velocity(&mut g.objs.aliens[idx as usize]);
    } else {
        fall_down_y_vector(
            &mut g.objs.aliens[idx as usize],
            DEFAULT_BOUNCE_SHIFT,
            DEFAULT_FALL_GRAVITY,
            GROUND_Y,
        );
        apply_velocity(&mut g.objs.aliens[idx as usize]);
    }

    if let Some(pl) = player(g) {
        let px = pl.worldx;
        {
            let al = &mut g.objs.aliens[idx as usize];
            fchase_i16(&mut al.worldx, px, NORMAL_X_CHASE_STEP);
            if al.sflags2 & ASF2_SFLAG3 != 0 {
                for _ in 1..STRONG_X_CHASE_PASSES {
                    fchase_i16(&mut al.worldx, px, NORMAL_X_CHASE_STEP);
                }
                al.worldz = al.worldz.wrapping_sub(STRONG_DEPTH_STEP);
            }
        }
    }
    add_player_z(g, idx);
}

/// ROM `ironballmissile_istrat` (D3STRATS.ASM:1010) — wait then spray 9 ironballs.
pub fn ironballmissile_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, ironballmissile_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = BALL_MISSILE_HP;
        al.ap = BALL_MISSILE_AP;
        al.sbyte1 = 0; // phase: 0 = approach, 1 = spray
        al.visual_kind = ObjectVisualKind::ScaledSprite;
        al.depthoffset = 0;
        al.tx = 0;
    }
}

/// Approach until |dz|<1000, then fire 9× ironball4 and die.
pub fn ironballmissile_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        if let Some(pl) = player(g) {
            let dz = (g.objs.aliens[idx as usize].worldz as i32 - pl.worldz as i32).abs();
            if dz < 1000 {
                g.objs.aliens[idx as usize].sbyte1 = 1;
            } else {
                return;
            }
        } else {
            return;
        }
    }
    for _ in 0..9 {
        let _ = fire_ironball4(g, idx);
    }
    crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
    g.objs.aldead = 1; // s_kill_obj → remove this frame
}

/// Shared spawn shell for fling `fire_ironball*` (DSTRATS.ASM:8089+):
/// copy pos/rots, ENEMY1, shape ironball — muzzle applied by caller.
fn spawn_ironball_shell(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SHAPE_IRONBALL)?;
    {
        let src = g.objs.aliens[firer as usize];
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotx = src.rotx;
        al.roty = src.roty;
        al.rotz = src.rotz;
        al.collflags |= COLLTYPE_ENEMY1;
        al.shape = SHAPE_IRONBALL;
    }
    Some(shot)
}

/// `fling.positional` with (x1,y1,z1)=(0,0,120) and scale 2,2,2 → world <<2.
/// Temporarily pitches the firer by `pitch_delta` around the muzzle place
/// (ROM does `al_rotx += -deg90` then restores).
fn ironball_fling_muzzle(g: &mut Game, shot: u16, firer: u16, pitch_delta: u8) {
    if pitch_delta != 0 {
        g.objs.aliens[firer as usize].rotx =
            g.objs.aliens[firer as usize].rotx.wrapping_add(pitch_delta);
    }
    let me = g.objs.aliens[firer as usize];
    // s_add_Roffs2pos B,y,x,x,#0,#0,#120,1,1,1,2,2,2
    let (rx, ry, rz) =
        crate::snes_trig::strat_roffs_full_scaled(me.rotz, me.rotx, me.roty, 0, 0, 120, 2);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = me.worldx.wrapping_add(rx);
        al.worldy = me.worldy.wrapping_add(ry);
        al.worldz = me.worldz.wrapping_add(rz);
    }
    if pitch_delta != 0 {
        g.objs.aliens[firer as usize].rotx =
            g.objs.aliens[firer as usize].rotx.wrapping_sub(pitch_delta);
    }
}

/// ROM `fire_ironball` (DSTRATS.ASM:8089) — muzzle + sflag3 (strong X chase).
pub fn fire_ironball(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = spawn_ironball_shell(g, firer)?;
    ironball_fling_muzzle(g, shot, firer, 0u8.wrapping_sub(DEG90));
    // sintab[gameframe] >> 4 added to firer pitch (boss anim side-effect).
    {
        use crate::snes_trig::SINTAB;
        let s = SINTAB[(g.vars.gameframe & 0xff) as usize] as i8;
        let delta = (s >> 4) as u8;
        g.objs.aliens[firer as usize].rotx = g.objs.aliens[firer as usize].rotx.wrapping_add(delta);
    }
    g.objs.aliens[shot as usize].sflags2 |= ASF2_SFLAG3;
    ironball_istrat(g, shot);
    g.hooks.play_se(0x49);
    Some(shot)
}

/// ROM `fire_ironball2` (DSTRATS.ASM:8115) — aim at player, sflag2 + powerbuild++.
pub fn fire_ironball2(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = spawn_ironball_shell(g, firer)?;
    ironball_fling_muzzle(g, shot, firer, 0u8.wrapping_sub(DEG90));
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[shot as usize];
        // ROM `dobj2obj3dangle_xy` — Yanglexy+nega.
        let yaw = angle_xz(&me, &pl).wrapping_neg();
        let pitch = strat_pitch_toward(&me, &pl);
        let al = &mut g.objs.aliens[shot as usize];
        al.roty = yaw;
        al.rotx = pitch;
    }
    let jitter = ((sf_random(&mut g.vars) as u8) & 3).wrapping_sub(2);
    g.objs.aliens[shot as usize].roty = g.objs.aliens[shot as usize].roty.wrapping_add(jitter);
    g.objs.aliens[shot as usize].sflags2 |= ASF2_SFLAG2;
    let pb = g.vars.shared.power_build.wrapping_add(1);
    g.vars.shared.power_build = pb;
    ironball_istrat(g, shot);
    g.hooks.play_se(0x49);
    Some(shot)
}

/// ROM `fire_ironball3` (DSTRATS.ASM:8150) — aim + pitch cone, sflag1 (faster).
pub fn fire_ironball3(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = spawn_ironball_shell(g, firer)?;
    ironball_fling_muzzle(g, shot, firer, 0); // no pitch tilt
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[shot as usize];
        // ROM `dobj2obj3dangle_xy` — Yanglexy+nega.
        let yaw = angle_xz(&me, &pl).wrapping_neg();
        let pitch = strat_pitch_toward(&me, &pl);
        let al = &mut g.objs.aliens[shot as usize];
        al.roty = yaw;
        al.rotx = pitch;
    }
    let rx = (sf_random(&mut g.vars) as u8) & (DEG22.wrapping_sub(1));
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = al.rotx.wrapping_add(rx).wrapping_sub(DEG11);
        al.sflags2 |= ASF2_SFLAG1;
    }
    ironball_istrat(g, shot);
    g.hooks.play_se(0x49);
    Some(shot)
}

// ============================================================
// NUKE / NOVA BOMB (GSTRATS.ASM:2067-2183, fire_nuke:2333)
// ============================================================

/// STRATEQU.INC nuke damage / radius ring.
pub const NUKE_AP: u8 = 8;
pub const NUKE_RATE: i16 = 200;
pub const NUKE_MAX_RADIUS: i16 = 7000;
const NUKE_EXPLOSION_SOUND: u8 = 48;

/// Legacy source-layout discriminator retained for map/window transitions.
const SMARTBOMB_CIRCLE: i16 = 2;

/// MB_* from GILESALC.INC (missile / weapon screen bounds).
const MB_LEFT: u8 = 1;
const MB_RIGHT: u8 = 2;
const MB_TOP: u8 = 4;
const MB_BOTTOM: u8 = 8;
const MB_LBOTTOM: u8 = 16;
const MB_LTOP: u8 = 32;
const MB_RTOP: u8 = 64;

/// `al_flags` front-of-player bit (VARS.INC `affrontpl`).
const AF_FRONT_PL: u8 = 8;

#[inline]
fn nuke_player_idx(g: &Game) -> Option<u16> {
    if g.objs.aliens[0].active {
        Some(0)
    } else {
        None
    }
}

/// ROM `missboundchkexp` (GSTRATS.ASM:1327) — kill weapon if outside miss bounds.
pub fn missbound_chk_exp(g: &mut Game, idx: u16) {
    use crate::common::{sv, StratRam};
    let flags = g.vars.sv_u8(sv::MISSBOUNDFLAGS);
    let wx = g.objs.aliens[idx as usize].worldx;
    let wy = g.objs.aliens[idx as usize].worldy;
    let max_mx = g.vars.sv_i16(sv::MAXMMOVEX);
    let min_mx = g.vars.sv_i16(sv::MINMMOVEX);
    let max_my = g.vars.sv_i16(sv::MAXMMOVEY);
    let min_py = g.vars.minpmove_y;

    let mut kill = false;
    if flags & MB_RIGHT != 0 && wx > max_mx {
        kill = true;
    }
    if !kill && flags & MB_LEFT != 0 && wx < min_mx {
        kill = true;
    }
    if !kill && flags & (MB_TOP | MB_RTOP | MB_LTOP) != 0 && wy < min_py {
        // Top edge: optional left/right player-x gates (colony/bridge).
        let pl = nuke_player_idx(g);
        if flags & MB_RTOP != 0 {
            if let Some(p) = pl {
                let miss_tr = g.vars.sv_i16(sv::MISSBTOPRIGHT);
                if g.objs.aliens[p as usize].worldx <= miss_tr {
                    kill = true;
                }
            } else {
                kill = true;
            }
        } else if flags & MB_LTOP != 0 {
            if let Some(p) = pl {
                let miss_tl = g.vars.sv_i16(sv::MISSBTOPLEFT);
                if g.objs.aliens[p as usize].worldx <= miss_tl {
                    kill = true;
                }
            } else {
                kill = true;
            }
        } else {
            kill = true;
        }
    }
    if !kill && flags & (MB_BOTTOM | MB_LBOTTOM) != 0 && wy > max_my {
        if flags & MB_LBOTTOM != 0 {
            if let Some(p) = nuke_player_idx(g) {
                let miss_bl = g.vars.sv_i16(sv::MISSBBOTLEFT);
                if g.objs.aliens[p as usize].worldx <= miss_bl {
                    kill = true;
                }
            } else {
                kill = true;
            }
        } else {
            kill = true;
        }
    }
    if kill {
        crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
    }
}

/// ROM `nuke_Istrat` (GSTRATS.ASM:2067) — relative to player speed.
pub fn nuke_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, nuke_strat);
    let coll = sid(g, weapcollide_istrat);
    let exp = sid(g, nukeexp_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        // s_gen_3dvecs scale 1 (<<1) at al_vel, then addgen at mother sbyte3.
        let nuke_spd = al.vel;
        let mother_spd = al.sbyte3;
        crate::common::strat_gen_vecs_3d_scaled(al, 1);
        let (bx, by, bz) = (al.vx, al.vy, al.vz);
        al.vel = mother_spd;
        crate::common::strat_gen_vecs_3d(al);
        al.vx = al.vx.wrapping_add(bx);
        al.vy = al.vy.wrapping_add(by);
        al.vz = al.vz.wrapping_add(bz);
        al.vel = nuke_spd;
        al.rotx = 0;
        al.roty = DEG180;
        al.snd2 = 6;
    }
}

/// ROM `nuke_strat` (GSTRATS.ASM:2077).
pub fn nuke_strat(g: &mut Game, idx: u16) {
    use sf_core::pad;
    use sf_game::vars::PSF_NOFIRE;

    // s_remove_ifplayerdead
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    if g.vars.pshipflags & PSF_NOFIRE != 0 {
        removenuke_istrat(g, idx);
        return;
    }

    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_dec_lifecnt x,1 → kill when count hits 0
    {
        let al = &mut g.objs.aliens[idx as usize];
        let c = al.count.wrapping_sub(1);
        if c == 0 {
            crate::common::kill_obj(al);
            return;
        }
        al.count = c;
    }

    const PFM_DIEFALL: u8 = 1;
    if g.vars.playerflymode & PFM_DIEFALL != 0 {
        g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(2);
    }

    // A newly pressed → detonate (s_jmp_keyup / s_jmp_lastkeydown).
    let pad_prev = ((g.vars.lastcont0 as u16) << 8) | g.vars.lastcontl0 as u16;
    let pad_new = g.vars.pad1 & !pad_prev;
    if pad_new & pad::A != 0 {
        crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
        return;
    }

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        missbound_chk_exp(g, idx);
    }
}

/// ROM `removenuke_Istrat` (GSTRATS.ASM:2107) — refund bomb, clear delay, remove.
pub fn removenuke_istrat(g: &mut Game, idx: u16) {
    use crate::common::{sv, StratRam};
    // IFEQ INFBOMBS → refund
    let n = g.vars.sv_u16(sv::SPECWEPCNT);
    g.vars.set_sv_u16(sv::SPECWEPCNT, n.wrapping_add(1));
    g.vars.set_sv_u8(sv::SPECIALDELAY, 1);
    g.objs.aldead = 1;
    let _ = idx;
}

/// ROM `nukeexp_Istrat` (GSTRATS.ASM:2121).
pub fn nukeexp_istrat(g: &mut Game, idx: u16) {
    use crate::common::{sv, StratRam};
    g.objs.aliens[idx as usize].sword1 = NUKE_RATE;
    let exp = sid(g, nukeexp_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(exp);
    g.objs.aliens[idx as usize].shape = 0; // nullshape
    let _ = make_medium_exp_obj(g, idx);
    // s_set_vartobeobj circleobj,x — store index+1
    g.vars
        .set_sv_u16(sv::CIRCLEOBJ, (idx as u16).wrapping_add(1));
    g.vars.circleanim = SMARTBOMB_CIRCLE;
    g.vars
        .screen_fill_circle
        .begin_smart_bomb(ScreenFillCircleCenter::Object(idx + 1));
    g.hooks.play_se(NUKE_EXPLOSION_SOUND);
    g.objs.aliens[idx as usize].snd2 = 0;
}

/// ROM `nukeexp_strat` (GSTRATS.ASM:2132) — expanding damage ring.
pub fn nukeexp_strat(g: &mut Game, idx: u16) {
    let radius = g.objs.aliens[idx as usize].sword1;
    if radius >= NUKE_MAX_RADIUS {
        g.objs.aldead = 1;
        return;
    }
    add_player_z(g, idx);

    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        return;
    }

    let r_hi = radius;
    let r_lo = radius.wrapping_sub(NUKE_RATE);
    let self_pos = {
        let al = &g.objs.aliens[idx as usize];
        (al.worldx, al.worldz)
    };

    // Walk active list; damage realobjs in the annular band [r_lo, r_hi).
    let actives = g.objs.active_indices();
    for &yi in &actives {
        if yi == idx {
            continue;
        }
        let yal = g.objs.aliens[yi as usize];
        if yal.sflags3 & ASF3_REALOBJ == 0 {
            continue;
        }
        // xzdiffs_l vs nuke position (self_pos as synthetic alien).
        let mut probe = Alien::default();
        probe.worldx = self_pos.0;
        probe.worldz = self_pos.1;
        let d = crate::common::strat_dist_xz(&probe, &yal);
        if d >= r_hi || d < r_lo {
            continue;
        }
        if yal.sflags & (ASF_NOHITAFFECT | ASF_COLLDISABLE) != 0 {
            continue;
        }
        if yal.flags & AF_FRONT_PL == 0 {
            continue;
        }
        if (yal.hp as i8) < 0 {
            continue;
        }
        let nhp = yal.hp.saturating_sub(NUKE_AP);
        let al = &mut g.objs.aliens[yi as usize];
        al.hp = nhp;
        al.sflags |= ASF_HITFLASH;
        al.type_ |= ATNUKED;
    }

    g.objs.aliens[idx as usize].sword1 = radius.wrapping_add(NUKE_RATE);
}

/// Extended shape-catalog id for the ROM `nuke` sprite.
pub const SH_NUKE: u16 = 407;

/// ROM `fire_nuke` (GSTRATS.ASM:2333) — spawn player's special weapon.
pub fn fire_nuke(g: &mut Game, firer: u16) -> Option<u16> {
    let shot = make_obj(g, SH_NUKE)?;
    let owner_vel = g.objs.aliens[firer as usize].vel;
    let (ox, oy, oz, rx, ry) = {
        let o = &g.objs.aliens[firer as usize];
        (o.worldx, o.worldy, o.worldz, o.rotx, o.roty)
    };
    // Default gen_weapon muzzle: small Z forward (player uses 80>>weapon_scale).
    const WEAPON_SCALE: i16 = 2;
    let mz = 80i16 >> WEAPON_SCALE;
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = ox;
        al.worldy = oy;
        al.worldz = oz.wrapping_add(mz);
        al.hp = 2;
        al.ap = 8;
        al.vel = 50;
        al.count = 28;
        al.rotx = rx;
        al.roty = ry;
        al.sbyte1 = rx;
        al.sbyte2 = ry;
        al.sbyte3 = owner_vel; // mother speed for addgen
        al.collflags |= ACF_COLLTYPE1 | ACF_COLLTYPE5; // laser + friend
        al.type_ |= ATLASER;
        al.sflags4 &= !ASF4_INVISIBLE;
        al.visual_kind = ObjectVisualKind::ScaledSprite;
    }
    nuke_istrat(g, shot);
    Some(shot)
}

// ============================================================
// HARD OBJECT STRATEGIES (GSTRATS.ASM; C strat_enemy.c:357-416)
// ============================================================

/// C `Strat_Hard_Init` (hard_Istrat, GSTRATS.ASM:642-646).
/// ASM has no `s_set_colltype`; only `hardenemy1_Istrat` sets enemy1. (Audit A #16)
pub fn strat_hard_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    set_hard_vars(al);
    al.stratptr = None;
}

/// ROM `pillar2_strat` (DSTRATS.ASM:1721) — advance the 32-frame extending
/// pillar animation once per strategy tick.
fn pillar2_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.animframe = 0x80 | ((al.animframe & 0x7f).wrapping_add(1) % 32);
}

/// ROM `pillar2_Istrat` (DSTRATS.ASM:1715): hard object variables, animation
/// frame zero, then the extending-pillar animation tick above.
pub fn pillar2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, pillar2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    set_hard_vars(al);
    al.stratptr = Some(tick);
    al.animframe = 0x80;
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
    al.sflags2 |= ASF2_COLLDISABLE;
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
        al.sflags &= !ASF_SHADOW;
        al.sflags3 &= !ASF3_NOHITAFFECT;
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

/// C `pillar3_enter_fall` / ASM `pillar3fall_i` (DSTRATS.ASM:795-810).
fn pillar3_enter_fall(g: &mut Game, idx: u16) {
    let s = sid(g, pillar3fall_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sflags |= ASF_SHADOW;
        al.sflags3 |= ASF3_NOHITAFFECT;
        // s_rightview_strat: leftpl clear (right of view) keeps +4; left → -4.
        al.sbyte1 = 4;
        if al.flags & AF_LEFT_PL != 0 {
            al.sbyte1 = (-4i8) as u8;
        }
        al.sbyte2 = PILLAR3_FALL_FRAMES;
    }
    // ROM: create bouncyball; copy position; worldz-=10; all strategies explode; kill object.
    if let Some(ball) = make_obj(g, SH_BOUNCYBALL) {
        // `s_make_obj`/`l_add` seats the impact immediately after the pillar,
        // so its hp-zero explosion runs later in this same strategy pass.
        g.objs.active_move_after(ball, idx);
        let (px, py, pz) = {
            let me = &g.objs.aliens[idx as usize];
            (me.worldx, me.worldy, me.worldz.wrapping_sub(10))
        };
        let exp = sid(g, strat_explode);
        let al = &mut g.objs.aliens[ball as usize];
        al.worldx = px;
        al.worldy = py;
        al.worldz = pz;
        al.stratptr = Some(exp);
        al.collstratptr = Some(exp);
        al.expstratptr = Some(exp);
        crate::common::kill_obj(al);
    }
}

/// C `pillar3_strat` (strat_enemy.c:487).
fn pillar3_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    if dist_xz(&me, &pl) < PILLAR3_DIST || me.hp < PILLAR3_FALL_HP || me.hitflags & HF2_MASK != 0 {
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
pub(crate) fn pillar3explode_strat(g: &mut Game, idx: u16) {
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
    {
        let al = &mut g.objs.aliens[idx as usize];
        // `delayremove_Istrat` runs `s_hardvars` before installing its tick.
        // The exploding parent must therefore become indestructible while its
        // authored child sequence completes.
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags2 |= ASF2_COLLDISABLE;
        al.collflags = 0;
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.count = 7;
        // ASM delayremove_Istrat opens with `s_clr_alsflag x,relexplode`.
        al.sflags2 &= !ASF2_RELEXPLODE;
    }
    // ASM `s_jmp delayremove_Istrat` is a tail-jump: the initializer falls
    // through into delayremove_strat, so the explosion frame itself applies
    // the first countdown decrement (retail shows lifecnt 6 at the end of
    // the explosion tick and the pillar dying exactly seven frames later).
    delayremove_strat(g, idx);
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
    let v = g.vars.map.skill_fly;
    if v > 0 {
        g.vars.map.skill_fly = v - 1;
    }
    g.objs.aldead = 1;
}

/// Absolute distance in the source's wrapping 16-bit world-coordinate space.
/// Stages legitimately cross the signed boundary; widening before subtraction
/// turns two adjacent points there into points nearly 65,536 units apart.
#[inline]
fn skillfly_axis_distance(first: i16, second: i16) -> u16 {
    first.wrapping_sub(second).unsigned_abs()
}

/// C `skillfly_strat` (strat_enemy.c:550).
fn skillfly_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    if skillfly_axis_distance(me.worldz, pl.worldz) < SKILLFLY_DEPTH_RANGE {
        let radius = me.sword1.unsigned_abs();
        let dx = skillfly_axis_distance(me.worldx, pl.worldx);
        let dy = skillfly_axis_distance(me.worldy, pl.worldy);
        if dx < radius && dy < radius {
            skillfly_remove(g);
            return;
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(SKILLFLY_BEHIND_PROBE);
    }
    if pl.worldz.wrapping_sub(g.objs.aliens[idx as usize].worldz) >= 0 {
        // ASM `s_jmp_objinfront y,x,.rem` (DSTRATS.ASM:8471) reaches `.rem`
        // WITHOUT the `s_dec_var skillfly` — only the caught path (:8459-8460)
        // decrements the skill-ring counter. (Audit A #33)
        g.objs.aldead = 1;
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(SKILLFLY_BEHIND_PROBE);
}

/// C `Strat_Skillfly_Init` (strat_enemy.c:592).
pub fn strat_skillfly_init(g: &mut Game, idx: u16) {
    let s = sid(g, skillfly_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_COLLDISABLE;
        if al.shape == 0 {
            al.sflags4 |= ASF4_INVISIBLE;
        }
        al.stratptr = Some(s);
    }
    let v = g.vars.map.skill_fly;
    g.vars.map.skill_fly = v.wrapping_add(1);
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

#[cfg(test)]
mod skillfly_coordinate_tests {
    use super::*;

    #[test]
    fn checkpoint_catches_player_across_signed_world_depth_boundary() {
        let mut game = Game::new();
        let player = game.objs.alloc().expect("player slot");
        assert_eq!(player, 0);
        game.objs.aliens[player as usize].worldx = 40;
        game.objs.aliens[player as usize].worldy = -60;
        game.objs.aliens[player as usize].worldz = i16::MAX - 7;

        let checkpoint = game.objs.alloc().expect("checkpoint slot");
        let object = &mut game.objs.aliens[checkpoint as usize];
        object.worldx = 40;
        object.worldy = -60;
        object.worldz = i16::MIN + 8;
        object.sword1 = 100;

        strat_skillfly_init(&mut game, checkpoint);

        assert_eq!(skillfly_axis_distance(i16::MAX - 7, i16::MIN + 8), 16);
        assert_ne!(
            game.objs.aliens[checkpoint as usize].sflags2 & ASF2_COLLDISABLE,
            0
        );
        assert_ne!(
            game.objs.aliens[checkpoint as usize].sflags4 & ASF4_INVISIBLE,
            0
        );
        assert_eq!(game.vars.map.skill_fly, 0);
        assert_eq!(game.objs.aldead, 1);
    }
}

// ============================================================
// GATES (C strat_enemy.c:607-798)
// ============================================================

/// C `gate3_player_box` (strat_enemy.c:607): `Obj_GetByIndex(g_pcboxobj_B)`.
fn gate3_player_box(g: &Game) -> Option<u16> {
    let idx = i32::from(g.vars.strategy.player_collision_objects[0]);
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
        let temp = g.vars.shared.map_restart_temporary;
        g.vars.shared.map_restart = temp;
        let bg = g.vars.currentbg;
        g.vars.shared.restart_background = bg;
        g.vars.shared.restart_palette_fade = g.vars.shared.last_palette_fade;
        g.vars.shared.enemy_path.roll1 = 1;
        // ASM falls through into `.strat2` (spin+colanim) the same frame.
        // (Audit A Minor 12)
        gate_spin_strat(g, idx);
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
        al.sflags |= ASF_SHADOW;
        al.sflags2 |= ASF2_COLLDISABLE;
        al.colframe = 0;
    }
    g.vars.shared.enemy_path.roll1 = 0;
    let mp = g.vars.mapptr;
    g.vars.shared.map_restart_temporary = mp;
}

/// C `gate2_strat` (strat_enemy.c:741).
fn gate2_strat(g: &mut Game, idx: u16) {
    let maxx = g.vars.strategy.player_max_x;
    let minx = g.vars.strategy.player_min_x;
    let maxy = g.vars.strategy.player_max_y;
    let miny = g.vars.minpmove_y;
    let viewcy = g.vars.strategy.view_center_y;
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
            let score = g.vars.shared.player_score;
            g.vars.shared.player_score = score.wrapping_add(GATE2_HEAL_SCORE);
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
        al.sflags |= ASF_SHADOW;
        al.sflags2 |= ASF2_COLLDISABLE;
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
    al.sflags4 &= !ASF4_CHILDOBJ;
    al.ptr = 0;
    al.sword1 = 0;
}

/// ROM `divorcefamily_l` (STRATROU.ASM:3000) — see [`sf_game::obj::Objects::divorce_family`].
/// Kept as a strat-lane entry so explode/remove call sites stay local.
pub fn divorce_family(g: &mut Game, idx: u16) {
    g.objs.divorce_family(idx);
}

/// C `boss_prune_family_links` (strat_enemy.c:851).
pub(crate) fn boss_prune_family_links(g: &mut Game, mother: u16) {
    if g.objs.aliens[mother as usize].sflags4 & ASF4_MOTHEROBJ == 0 {
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
        let valid =
            raw_al.active && raw_al.sflags4 & ASF4_CHILDOBJ != 0 && raw_al.ptr == mother_idx;
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
        g.objs.aliens[mother as usize].sflags4 &= !ASF4_MOTHEROBJ;
    }
}

/// C `boss_get_mother_obj` (strat_enemy.c:904).
pub(crate) fn boss_get_mother_obj(g: &mut Game, child: u16) -> Option<u16> {
    if g.objs.aliens[child as usize].sflags4 & ASF4_CHILDOBJ == 0 {
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
            && al.sflags4 & ASF4_CHILDOBJ != 0
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
pub fn boss_attach_child_to_mother(g: &mut Game, mother: u16, child: u16, child_num: u8) -> bool {
    if mother == child
        || !g.objs.aliens[mother as usize].active
        || !g.objs.aliens[child as usize].active
    {
        return false;
    }
    g.objs.aliens[mother as usize].sflags4 |= ASF4_MOTHEROBJ;
    {
        let al = &mut g.objs.aliens[child as usize];
        al.sflags4 |= ASF4_CHILDOBJ;
        al.sbyte1 = child_num;
        al.ptr = boss_obj_index_or_null(mother);
        al.sword1 = 0;
    }
    // `s_make_childobj` allocates through `l_add` while the mother is the
    // current active-list object. Each new child is therefore inserted
    // immediately after its mother (and before older siblings), so the mother
    // advances first and every child consumes that completed pose in the same
    // strategy pass. Leaving allocations at the active-list head made linked
    // parts follow one tick behind fast-moving multipart bosses.
    g.objs.active_move_after(child, mother);
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
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_yaw_i16(mother.roty, offx, offy, offz);
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = mother.worldx.wrapping_add(rx);
    al.worldy = mother.worldy.wrapping_add(ry);
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
    boss_apply_yaw_offset(g, child, &me, -(10 << BOSS7_SCALE), 7 << BOSS7_SCALE, 0);
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
    // C-side boss helpers used fireobjptr; the original weapon macros use
    // al_ptr. They are the same semantic target in the two source lanes.
    for ptr in [
        g.objs.aliens[idx as usize].fireobjptr,
        g.objs.aliens[idx as usize].ptr,
    ] {
        if ptr == 0 {
            continue;
        }
        let t = ptr as i32 - 1;
        if t >= 0 && t < NUMBER_AL as i32 {
            return Some(t as u16);
        }
    }
    None
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
    let shot = make_obj(g, SH_MISSILE)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(g, shot, &me, 17 << BOSS7_SCALE, -(5 << BOSS7_SCALE), 0);
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
    // ROM `s_fire_weapon x,HMISSILE1` → gen_weapon `jsl missilesound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Missile, me.worldx, me.worldz);
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
            // ROM `s_obj2obj_3dangle` into sbyte1/2 — Yanglexy+nega.
            achase_angle(&mut sb1, angle_xz(&me, &t).wrapping_neg(), 4);
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

/// ROM `relelaserhome_Istrat` (GSTRATS.ASM:1897-1906). The initializer ends
/// before the movement label, so a newly inserted bolt does not move on its
/// allocation frame.
pub fn relelaserhome_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, relelaserhome_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.rotx = al.sbyte1;
    al.roty = al.sbyte2;
    crate::common::strat_gen_vecs_3d_scaled(al, 1);
    al.animframe = 0x80;
}

/// C `relelaserhome_strat` (GSTRATS.ASM:1907-1932).
pub fn relelaserhome_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let frame = al.animframe & 0x7F;
        if frame != 4 {
            let mut next = frame.wrapping_add(2);
            if next >= 8 {
                next = next.wrapping_sub(8);
            }
            al.animframe = 0x80 | next;
        }
    }
    let pl = player(g);
    if g.objs.aliens[idx as usize].sflags2 & RELSLOWELASERHOME_LOCK_FLAG == 0 {
        if let Some(pl) = pl {
            let me = g.objs.aliens[idx as usize];
            // ASM `s_jmp_Zdistmore #800,.nmin` skips the lock when |dz|>=800,
            // so latch only when |dz|<800. (Audit A Minor 2)
            if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < RELSLOWELASERHOME_CLOSE_Z {
                g.objs.aliens[idx as usize].sflags2 |= RELSLOWELASERHOME_LOCK_FLAG;
            }
            strat_aim_3d(g, idx, &pl, 1);
            crate::common::strat_gen_vecs_3d_scaled(&mut g.objs.aliens[idx as usize], 1);
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
    al.sflags4 &= !ASF4_MOTHEROBJ;
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
fn boss1_rot_offset_pos(
    g: &mut Game,
    self_idx: u16,
    base: &Alien,
    offx: i16,
    offy: i16,
    offz: i16,
) {
    let (rx, ry, rz) =
        crate::snes_trig::strat_roffs_full_i16(base.rotz, base.rotx, base.roty, offx, offy, offz);
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(ry);
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
    let shot = make_obj(g, SH_MISSILE)?;
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
    // ROM `s_fire_weapon x,HMISSILE1` → gen_weapon `jsl missilesound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Missile, me.worldx, me.worldz);
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
    let shot = make_obj(g, SH_BOUNCYBALL)?;
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
    al.sflags4 &= !ASF4_INVISIBLE;
    al.visual_kind = ObjectVisualKind::ScaledSprite;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    al.sbyte1 = yaw;
    al.sbyte2 = pitch;
    al.rotx = 0;
    al.roty = DEG180;
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2528).
    g.hooks
        .make_snd(PosSndFamilyId::EnemyBattry, me.worldx, me.worldz);
    Some(shot)
}

/// C `boss1_fire_relslowlaser` (strat_enemy.c:2256).
fn boss1_fire_relslowlaser(g: &mut Game, self_idx: u16, target: Option<u16>, homing: bool) {
    let me = g.objs.aliens[self_idx as usize];
    let mut pitch = me.rotx;
    let mut yaw = me.roty;
    if homing {
        if let Some(t) = target
            .map(|t| g.objs.aliens[t as usize])
            .filter(|t| t.active)
        {
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
    // ROM `s_fire_weapon x,RELSLOWELASER(HOME)` → gen_weapon `jsl lasersound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
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
/// Public for AUDIT_BOSS_TICKS2 Minor #18 rise-boundary tests.
pub fn boss1up_strat(g: &mut Game, idx: u16) {
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
/// Public for AUDIT_BOSS_TICKS2 Minor #19 Zdistmore-boundary tests.
pub fn boss1in_strat(g: &mut Game, idx: u16) {
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
pub fn boss1out_strat(g: &mut Game, idx: u16) {
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
/// Public for AUDIT_BOSS_TICKS2 Minor #19 Zdistmore-boundary tests.
pub fn boss1inclose_strat(g: &mut Game, idx: u16) {
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
pub fn boss1back_strat(g: &mut Game, idx: u16) {
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
        let pitch = me
            .rotx
            .wrapping_add(((sf_random(&mut g.vars) & 15) as i16 - 7) as u8);
        let yaw = me
            .roty
            .wrapping_add(((sf_random(&mut g.vars) & 15) as i16 - 7) as u8);
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
    // .nofire -> boss1_fin (GBSTRATS.ASM:227/272-274): s_add_playerZ + s_add_bossHP.
    // NO second rotz spin, no boss1rots child recount, no center-fire (finding #21).
    add_player_z(g, idx);
    add_bosshp(g, idx);
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
        let side_right = g.objs.aliens[mother as usize].sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT != 0;
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
/// Public for AUDIT_BOSS_TICKS2 Minor #19 covdie remove-boundary tests.
pub fn boss1covdie_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        // s_jmp_Zdistmore x,y,#1000,remove_Istrat (GBSTRATS.ASM:461) is inclusive:
        // remove when behind the player AND |dz| >= 1000 (finding #19).
        let me = g.objs.aliens[idx as usize];
        if me.worldz < pl.worldz && ((me.worldz as i32 - pl.worldz as i32).abs() as i16) >= 1000 {
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

/// C `boss1turret_common_strat` (strat_enemy.c:2654) /
/// GBSTRATS.ASM:341-401 (`boss1turretL/R_strat` → `fire_end` / `nfire` / `end`).
fn boss1turret_common_strat(g: &mut Game, idx: u16, right_side: bool) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let m = g.objs.aliens[mother as usize];
    let side_matches = m.sflags2 & BOSS1_PARENT_FLAG_SIDE_RIGHT != 0;
    // L: jmp_alsflag mother,sflag3,nfire — R: jmpNOT_alsflag mother,sflag3,nfire
    // Plus fire_end gates: !sflag2 → nfire, sflag1 → nfire.
    if side_matches != right_side
        || m.sflags2 & BOSS1_PARENT_FLAG_TURRETS_OPEN == 0
        || m.sflags4 & BOSS1_PARENT_FLAG_COVER_BLOCK != 0
    {
        boss1turret_nfire(g, idx, mother);
        return;
    }
    boss1turretfire_end(g, idx, mother);
}

/// ROM `boss1turret_nfire` (GBSTRATS.ASM:393) — idle/closed: colanim 0 + nohitaffect.
pub fn boss1turret_nfire(g: &mut Game, idx: u16, mother: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = 0; // s_init_colanim #4
        al.sflags |= ASF_NOHITAFFECT;
    }
    boss1turret_end(g, idx, mother);
}

/// ROM `boss1turretfire_end` (GBSTRATS.ASM:353) — open: animate, maybe fire, then end.
pub fn boss1turretfire_end(g: &mut Game, idx: u16, mother: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = al.colframe.wrapping_add(1) & 3; // s_add_colanim #1,#4
        al.sflags &= !ASF_NOHITAFFECT;
    }
    let cover = boss1_cover_obj(g, mother);
    if let Some(cover) = cover {
        if g.objs.aliens[cover as usize].sbyte3 >= 20 {
            boss1turret_end(g, idx, mother);
            return;
        }
    }
    // Fire gates: home (gf+idx)&15==0 with bf_flag1; else normal (gf+idx)&31==0.
    let phase = strat_phase_offset(idx) as u16;
    let pl_idx = if g.objs.aliens[0].active {
        Some(0u16)
    } else {
        None
    };
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
    boss1turret_end(g, idx, mother);
}

/// ROM `boss1turret_end` (GBSTRATS.ASM:397) — copy mother rotz, scroll, add boss HP.
fn boss1turret_end(g: &mut Game, idx: u16, mother: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[mother as usize].rotz;
    boss1_update_child_positions(g, mother);
    add_player_z(g, idx);
    add_bosshp(g, idx);
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
pub fn boss1exp_init(g: &mut Game, idx: u16) {
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
    boss1makechild(g, idx);
    g.hooks.play_se(0x82);
}

/// ROM `boss1makechild` (GBSTRATS.ASM:113) — spawn cover + 8 turrets, seat, arm.
pub fn boss1makechild(g: &mut Game, idx: u16) {
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL0, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL1, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL2, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TL3, boss1turret_l_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR0, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR1, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR2, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_TR3, boss1turret_r_init);
    let _ = boss1_spawn_child(g, idx, BOSS1_CHILD_COVER, boss1cov_init);
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    boss1_update_child_positions(g, idx); // boss1rots_srou
    g.objs.aliens[idx as usize].stratstate = 0;
    g.objs.aliens[idx as usize].sbyte3 = 1;
}

// ============================================================
// TOW_0 EXPLODE (C strat_enemy.c:3543-3575)
// ============================================================

/// C `Strat_Tow0Explode` (strat_enemy.c:3552).
pub fn strat_tow0_explode(g: &mut Game, idx: u16) {
    // tow_0 flags its linked tow_1 child via the ordinary zero-safe object
    // handle used throughout the flat object model.
    let child_handle = g.objs.aliens[idx as usize].ptr;
    if let Some(child) = strat_obj_from_ptr(child_handle) {
        if g.objs.aliens[child as usize].active {
            g.objs.aliens[child as usize].sflags4 |= ASF4_SFLAG8;
        }
    }
    // (F5) ASM tow0explode_Istrat -> pillarexplode plays NO direct sound; the
    // former play_se(0x10) here was a leftover placeholder chime. Deleted.
    // ASM tow0explode_Istrat (EXPSTRAT.ASM:1070): `s_set_alvar W,x,al_sword2,
    // #160` then `s_brl pillarexplode_istrat` — the tower explodes RIGHT NOW
    // into the same staggered eight-child chain as pillar3, with child worldy
    // shifted by sword2=160, and its tail hands the corpse to delayremove.
    // The old C-lineage port deferred everything behind a 7-frame wait.
    g.objs.aliens[idx as usize].sword2 = 160;
    pillar3explode_strat(g, idx);
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
pub fn zaco2_istrat(g: &mut Game, idx: u16) {
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
    let random_x_velocity = ((sf_random(&mut g.vars) & 63) as i16) - 32;
    let random_y_velocity = ((sf_random(&mut g.vars) & 63) as i16) - 32;
    let s = sid(g, wormsplit_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.vx = random_x_velocity;
    al.vy = random_y_velocity;
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
    let rnd = sf_random(&mut g.vars) as u8;
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
    use crate::common::{sv, StratRam};
    // Canonical ROM `specwepcnt` (sv 0x056E) — same store player fire / removenuke use.
    let cnt = g.vars.sv_u16(sv::SPECWEPCNT);
    if cnt < ITEM5_MAX_SPEC {
        g.vars.set_sv_u16(sv::SPECWEPCNT, cnt + 1);
        // ASM GASTRATS.ASM:2586 `s_set_var B,specflash,#30` inside the
        // specwepcnt<5 block. (Audit A #24)
        g.vars.shared.special_flash = 30;
        g.hooks.play_se(0x18);
        let score = g.vars.shared.player_score;
        g.vars.shared.player_score = score.wrapping_add(ITEM5_SCORE);
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

/// C `Strat_Item7_Init` / ROM `item7_Istrat` (GASTRATS.ASM:2915-2917).
/// No `s_end_strat` — falls into `item7_strat` the same tick.
pub fn strat_item7_init(g: &mut Game, idx: u16) {
    let s = sid(g, item7_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
    }
    item7_strat(g, idx);
}

// ============================================================
// item4 + ripair (GASTRATS.ASM:3030-3124) — repair-pod pickup chain.
// ============================================================

const SH_RIPAIR_W: u16 = 401;
const ITEM4_PICKUP_Z: i16 = 120; // 60*2
const ITEM4_PICKUP_XY: i16 = 100; // 50*2
const RIPAIR_CATCH_XY: i16 = 20;
const RIPAIR_CATCH_Z: i16 = 30;

/// ROM `item4_Istrat` — spinning repair pod that spawns `ripair` on pickup.
pub fn item4_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, item4_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

/// ROM `item4_strat` — spin, float to range, spawn ripair when player close.
pub fn item4_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotx = al.rotx.wrapping_add(4);
    }
    itemtorange_srou(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(20);

    let me = g.objs.aliens[idx as usize];
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist >= ITEM4_PICKUP_Z {
        return;
    }
    let mut xydist = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    xydist = xydist.wrapping_add((me.worldy as i32 - pl.worldy as i32).abs() as i16);
    if xydist >= ITEM4_PICKUP_XY {
        return;
    }
    if let Some(pod) = make_obj(g, SH_RIPAIR_W) {
        ripair_istrat(g, pod);
    }
    g.objs.aldead = 1;
}

/// ROM `ripair_Istrat` — repair ship approaches from the player's right.
pub fn ripair_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, ripair_strat);
    let (px, py, pz) = (g.vars.player_posx, g.vars.player_posy, g.vars.player_posz);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
        al.worldz = pz.wrapping_add(-200);
        al.worldx = px.wrapping_add(500);
        al.worldy = py;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
        al.vx = 0;
        al.vy = 0;
        al.vz = 30;
        al.rotz = DEG90;
        al.sbyte1 = 30;
    }
    g.hooks.play_se(0x8b);
}

/// ROM `ripair_strat` — chase player, then repair wings on catch.
pub fn ripair_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        return;
    };
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(al.vz);
        // Soft approach chase always runs.
        al.worldx = chase_proportional(al.worldx, g.vars.player_posx, 3);
        al.worldy = chase_proportional(al.worldy, g.vars.player_posy, 3);
    }

    // s_decbne_alvar sbyte1,.nreccoll — while countdown remains, skip catch.
    let next = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = next;
    if next != 0 {
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = 1;

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, g.vars.player_posx, 1);
        al.worldy = chase_proportional(al.worldy, g.vars.player_posy, 1);
    }
    {
        let mut rotz = g.objs.aliens[idx as usize].rotz;
        achase_angle(&mut rotz, pl.rotz, 1);
        g.objs.aliens[idx as usize].rotz = rotz;
    }
    let zdist = (g.objs.aliens[idx as usize].worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist < 500 {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = g.vars.player_posx;
        al.worldy = g.vars.player_posy;
        al.rotz = pl.rotz;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vz = -40;
        al.type_ |= ATZREMOVE; // s_setremove_behind
    }

    // Catch: XY close AND (player ahead OR Z close).
    let me = g.objs.aliens[idx as usize];
    let mut xydist = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    xydist = xydist.wrapping_add((me.worldy as i32 - pl.worldy as i32).abs() as i16);
    if xydist >= RIPAIR_CATCH_XY {
        return;
    }
    let player_ahead = pl.worldz >= me.worldz;
    let zclose = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if !player_ahead && zclose >= RIPAIR_CATCH_Z {
        return;
    }
    item_repair_player_wings(g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pl.worldx;
        al.worldy = pl.worldy;
        al.worldz = pl.worldz;
    }
    g.hooks.play_se(0x17);
    flashplayer_istrat(g, idx);
}

/// C `flashplayer_Istrat` (strat_enemy.c:4068) / ROM GASTRATS item pickup flash.
pub fn flashplayer_istrat(g: &mut Game, idx: u16) {
    if g.vars.player_view_mode == PlayerViewMode::Cockpit {
        g.objs.aldead = 1;
        return;
    }
    let s = sid(g, flashplayer_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.count = 20;
    al.sflags |= ASF_COLLDISABLE;
    // ASM flashplayer_Istrat (GASTRATS.ASM:3130-3132) does not touch colframe.
    // (Audit A Minor 10)
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

/// C `item7_strat` (strat_enemy.c:4119) / ROM GASTRATS.ASM:2918-2956.
/// Broken-wing pickup spawns `ripair_Istrat` (does not repair inline); intact
/// wings take the double-laser / beamball upgrade path.
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
    if needs_repair {
        // GASTRATS.ASM:2934-2937: s_make_obj #ripair_w → ripair_Istrat; .cont flash.
        // Repair + $17 happen later when ripair catches (ripair_strat .catch).
        if let Some(pod) = make_obj(g, SH_RIPAIR_W) {
            ripair_istrat(g, pod);
        }
        flashplayer_istrat(g, idx);
        return;
    }
    // .dlaser: TRIGSE $15, score, re-init wings, doublaser/beamball upgrade.
    g.hooks.play_se(0x15);
    let score = g.vars.shared.player_score;
    g.vars.shared.player_score = score.wrapping_add(ITEM7_SCORE);
    item_repair_player_wings(g); // jsl pLWing_Istrat / pRWing_Istrat
    if g.vars.pshipflags2 & PSF2_DOUBLASER == 0 {
        g.vars.pshipflags2 |= PSF2_DOUBLASER;
    } else {
        g.vars.pshipflags3 |= PSF3_BEAMBALL;
    }
    flashplayer_istrat(g, idx);
}

// ============================================================
// item7a — helpball pickup (GASTRATS.ASM:2961-2990)
// ============================================================

const ITEM7A_PICKUP_Z: i16 = 120; // 60*2
const ITEM7A_PICKUP_XY: i16 = 60; // 30*2

/// ROM `item7a_Istrat` — spinning pickup that spawns a helpball + repairs wings.
pub fn item7a_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, item7a_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

/// ROM `item7a_strat` — drift/spin; on pickup spawn helpball, repair wings, flash.
pub fn item7a_strat(g: &mut Game, idx: u16) {
    let pl = player(g);
    if pl.is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    let pl = pl.unwrap();
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 == 0 {
            al.worldz = al.worldz.wrapping_add(20);
        }
    }
    itemtorange_srou(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotz = al.rotz.wrapping_add(4);
    }
    let me = g.objs.aliens[idx as usize];
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist >= ITEM7A_PICKUP_Z {
        return;
    }
    let mut xydist = (me.worldx as i32 - pl.worldx as i32).abs() as i16;
    xydist = xydist.wrapping_add((me.worldy as i32 - pl.worldy as i32).abs() as i16);
    if xydist >= ITEM7A_PICKUP_XY {
        return;
    }
    // Spawn helper ball at pickup position.
    if let Some(ball) = make_obj(g, SH_HELPBALL) {
        let (px, py, pz) = {
            let m = &g.objs.aliens[idx as usize];
            (m.worldx, m.worldy, m.worldz)
        };
        {
            let al = &mut g.objs.aliens[ball as usize];
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
        }
        helpball_istrat(g, ball);
    }
    g.hooks.play_se(0x10);
    // ROM jsl pLWing_Istrat / pRWing_Istrat on pcbox wings — repair flags.
    item_repair_player_wings(g);
    if let Some(lw) = g.coldet.pcbox.lwing {
        g.objs.aliens[lw as usize].hp = PCBOX_WING_HP;
        g.objs.aliens[lw as usize].sflags |= ASF_COLLDISABLE;
    }
    if let Some(rw) = g.coldet.pcbox.rwing {
        g.objs.aliens[rw as usize].hp = PCBOX_WING_HP;
        g.objs.aliens[rw as usize].sflags |= ASF_COLLDISABLE;
    }
    flashplayer_istrat(g, idx);
}

// ============================================================
// friend0 / friend1 (GASTRATS.ASM:670-760 / 877-920)
// ============================================================

const FRIEND_HP: u8 = 8; // STRATEQU.INC:171
const FRIEND_AP: u8 = 4; // STRATEQU.INC:172
const COLLTYPE_FRIEND: u8 = 0x80; // acf_colltype5
const SH_ZACO_5: u16 = 53;

fn friend_achase_i16(cur: i16, target: i16, shift: u32) -> i16 {
    let d = target.wrapping_sub(cur);
    cur.wrapping_add(d >> shift)
}

/// ROM `friend0_Istrat` — ally zaco that brakes then hunts `#zaco_5`.
pub fn friend0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, friend0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = FZACO_HP;
    al.ap = FZACO_AP;
    al.type_ &= !ATZREMOVE;
    al.vel = 60;
    al.roty = DEG22;
    al.rotx = DEG11;
    al.worldx = al.worldx.wrapping_add(g.vars.player_posx);
    al.worldy = al.worldy.wrapping_add(g.vars.player_posy);
    al.collflags |= COLLTYPE_ENEMYWEAP;
}

/// ROM `friend0_strat` — speedto 30 then friend02.
pub fn friend0_strat(g: &mut Game, idx: u16) {
    if speed_to(&mut g.objs.aliens[idx as usize], 30, 1) {
        friend02_init(g, idx);
    } else {
        friend0_cont(g, idx);
    }
}

pub fn friend02_init(g: &mut Game, idx: u16) {
    let tick = sid(g, friend02_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    friend02_strat(g, idx);
}

/// ROM `friend02_strat` — aim at nearest zaco_5; chase player z+200.
pub fn friend02_strat(g: &mut Game, idx: u16) {
    if let Some(y) =
        (0..NUMBER_AL).find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == SH_ZACO_5)
    {
        let tgt = g.objs.aliens[y];
        strat_aim_3d(g, idx, &tgt, 4);
    }
    if let Some(pl) = player(g) {
        let target_z = pl.worldz.wrapping_add(200);
        let z = g.objs.aliens[idx as usize].worldz;
        g.objs.aliens[idx as usize].worldz = friend_achase_i16(z, target_z, 4);
    }
    friend0_cont(g, idx);
}

pub fn friend0_cont(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `friend1_Istrat` — wingman that mirrors player + fires on player sflag3.
pub fn friend1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, friend1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = FRIEND_HP;
    al.ap = FRIEND_AP;
    al.collflags |= COLLTYPE_FRIEND;
    al.type_ &= !ATZREMOVE;
    al.sflags |= ASF_SHADOW;
}

pub fn friend1_strat(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        return;
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = pl.worldz.wrapping_add(200);
        al.worldy = friend_achase_i16(al.worldy, pl.worldy.wrapping_add(-30), 4);
        al.worldx = friend_achase_i16(al.worldx, pl.worldx, 3);
        al.vx = pl.vx;
        al.vy = pl.vy;
        al.vz = pl.vz;
        let mut rx = al.rotx;
        let mut ry = al.roty;
        let mut rz = al.rotz;
        achase_angle(&mut rx, pl.rotx, 3);
        achase_angle(&mut ry, pl.roty, 3);
        achase_angle(&mut rz, pl.rotz, 3);
        al.rotx = rx;
        al.roty = ry;
        al.rotz = rz;
    }
    if pl.sflags2 & ASF2_SFLAG3 != 0 {
        let _ = fire_elaser(g, idx);
    }
}

/// ROM `friendkill_Istrat` (GISTRATS.ASM:308) — remove shadow then explode.
pub fn friendkill_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags &= !ASF_SHADOW;
    strat_explode(g, idx);
}

/// ROM `friend2_Istrat` (GASTRATS.ASM:720-730) — hard-HP wingman that locks
/// Zenemy targets ahead of the player and fires ELASER.
pub fn friend2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, friend2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = HARD_HP;
    al.ap = FRIEND_AP;
    al.collflags |= COLLTYPE_FRIEND;
    al.type_ &= !ATZREMOVE;
    al.sflags |= ASF_SHADOW;
}

/// ROM `friend2_strat` (GASTRATS.ASM:731-860).
pub fn friend2_strat(g: &mut Game, idx: u16) {
    use sf_game::alien::{ASF3_LOCKON, ASF_HITFLASH};
    {
        gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
        apply_velocity(&mut g.objs.aliens[idx as usize]);
        if g.objs.aliens[idx as usize].worldy >= -30 {
            // s_jmp_higher x,#-30,.ok — snap when not higher than -30
            g.objs.aliens[idx as usize].worldy = -30;
        }
    }
    let Some(pl) = player(g) else {
        return;
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = pl.vel;
        al.worldz = pl.worldz.wrapping_add(200);
    }

    // Resolve / clear locked target in sword1 (index+1 encoding).
    let mut target: Option<u16> = None;
    let sw = g.objs.aliens[idx as usize].sword1 as u16;
    if sw != 0 {
        let yi = sw.wrapping_sub(1);
        if (yi as usize) < NUMBER_AL && g.objs.aliens[yi as usize].active {
            let me_z = g.objs.aliens[idx as usize].worldz;
            let tz = g.objs.aliens[yi as usize].worldz;
            let zdist = (me_z as i32 - tz as i32).abs();
            if zdist < 400 || g.objs.aliens[yi as usize].hp == 0 {
                g.objs.aliens[idx as usize].sword1 = 0;
            } else {
                target = Some(yi);
            }
        } else {
            g.objs.aliens[idx as usize].sword1 = 0;
        }
    }

    if target.is_none() {
        // Find Zenemy ahead of player: |dx| <= dz_from_player, dz>0, not locked.
        let px = g.vars.player_posx;
        let pz = g.vars.player_posz;
        let me = g.objs.aliens[idx as usize];
        let mut best: Option<(i32, u16)> = None;
        for i in 0..NUMBER_AL {
            if i == idx as usize || !g.objs.aliens[i].active {
                continue;
            }
            let o = &g.objs.aliens[i];
            if o.sflags3 & ASF3_LOCKON != 0 {
                continue;
            }
            if o.collflags & COLLTYPE_ZENEMY == 0 {
                continue;
            }
            let dx = (o.worldx as i32 - px as i32).abs();
            let dz = o.worldz as i32 - pz as i32;
            if dz < 0 || dz < dx {
                continue;
            }
            let d = (o.worldx as i32 - me.worldx as i32).abs()
                + (o.worldy as i32 - me.worldy as i32).abs()
                + (o.worldz as i32 - me.worldz as i32).abs();
            if d < 500 || d > 20000 {
                continue;
            }
            match best {
                Some((bd, _)) if bd <= d => {}
                _ => best = Some((d, i as u16)),
            }
        }
        if let Some((_, yi)) = best {
            g.objs.aliens[idx as usize].sword1 = (yi + 1) as i16;
            g.objs.aliens[yi as usize].sflags3 |= ASF3_LOCKON;
            target = Some(yi);
        }
    }

    if let Some(yi) = target {
        // Fire every 4 frames (notdelay 2), then aim.
        if g.vars.gameframe & 3 == 0 {
            let _ = fire_elaser(g, idx);
        }
        g.objs.aliens[yi as usize].sflags |= ASF_HITFLASH;
        let tgt = g.objs.aliens[yi as usize];
        strat_aim_3d(g, idx, &tgt, 3);
    } else {
        {
            let al = &mut g.objs.aliens[idx as usize];
            let mut ry = al.roty;
            let mut rx = al.rotx;
            achase_angle(&mut ry, 0, 5);
            achase_angle(&mut rx, 0, 5);
            al.roty = ry;
            al.rotx = rx;
            al.worldx = friend_achase_i16(al.worldx, g.vars.player_posx, 5);
            al.worldy = friend_achase_i16(al.worldy, g.vars.player_posy, 5);
        }
    }
    // rotz = roty << 2
    let ry = g.objs.aliens[idx as usize].roty;
    g.objs.aliens[idx as usize].rotz = ry.wrapping_shl(2);
}

// ============================================================
// hyperspace trail + hyper streak + phitflash (PISTRATS / PSTRATS)
// ============================================================

const PSTF_FLAG1: u8 = 2; // VARS.INC pstf_flag1
const HYPER_SHAPE: u16 = 408;
const HYPER2_SHAPE: u16 = 470;
const HYPER3_SHAPE: u16 = 471;
const HYPER4_SHAPE: u16 = 472;
// `(remaining >> 4) << 1` is the byte offset into the four-word hypers_tab.
const HYPER_OUT_SHAPES: [u16; 4] = [HYPER4_SHAPE, HYPER3_SHAPE, HYPER2_SHAPE, HYPER_SHAPE];
const HYPER_OUT_TICKS: u8 = 64;
const HYPER_OUT_PHASE_SHIFT: u32 = 4;
const HYPER_WORLD_DISTANCE: i16 = 4000;
const HYPER_RANDOM_HIGH_MASK: u8 = 1;
const HYPER_RANDOM_CENTER: i16 = 256;
const HYPER_ROLL_Y_OFFSET: i8 = 50;
const HYPER_Z_STEP: i16 = -80;

/// ROM `phitflash_Istrat` — alias of hitflash.
pub fn phitflash_istrat(g: &mut Game, idx: u16) {
    strat_hit_flash(g, idx);
}

/// ROM `hyper_Istrat` — streak that drifts −80 z each tick.
pub fn hyper_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(HYPER_Z_STEP);
}

/// ROM `hyperspace_Istrat` — emitter parked ahead of player, spawns hyper streaks.
pub fn hyperspace_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, hyperspace_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sword1 = HYPER_SHAPE as i16;
    }
    // PISTRATS falls directly through from the initializer into the first
    // emitter tick.
    hyperspace_strat(g, idx);
}

/// ROM `s_set_alvar2rnd` pair used for each signed 9-bit screen coordinate.
fn hyperspace_random_coordinate(g: &mut Game) -> i16 {
    let low = sf_random(&mut g.vars) as u8;
    let high = (sf_random(&mut g.vars) as u8) & HYPER_RANDOM_HIGH_MASK;
    i16::from_le_bytes([low, high]).wrapping_sub(HYPER_RANDOM_CENTER)
}

pub fn hyperspace_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].roty = DEG180;
    if let Some(pl) = player(g) {
        g.objs.aliens[idx as usize].worldz = pl.worldz.wrapping_add(HYPER_WORLD_DISTANCE);
    }
    let fire = (g.vars.pstratflags & PSTF_FLAG1 != 0) || (g.vars.gameframe & 1 == 0);
    if !fire {
        return;
    }
    let shape = g.objs.aliens[idx as usize].sword1 as u16;
    if let Some(streak) = make_obj(g, shape) {
        let streak_tick = sid(g, hyper_istrat);
        let emitter_z = g.objs.aliens[idx as usize].worldz;
        let random_x = hyperspace_random_coordinate(g);
        let random_y = hyperspace_random_coordinate(g);
        let roll = sf_random(&mut g.vars) as u8;
        let (roll_x, roll_y, _) =
            crate::snes_trig::strat_roffs_roll(roll, 0, HYPER_ROLL_Y_OFFSET, 0);
        {
            let al = &mut g.objs.aliens[streak as usize];
            al.worldx = random_x.wrapping_add(roll_x);
            al.worldy = random_y.wrapping_add(roll_y).wrapping_add(SPACE_VIEWCY);
            al.worldz = emitter_z;
            al.rotz = roll;
            al.sflags |= ASF_COLLDISABLE;
            al.stratptr = Some(streak_tick);
        }
    }
}

/// ROM `hyperspaceout_Istrat` — shrinking hyper table then hyperspace_strat.
pub fn hyperspaceout_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, hyperspaceout_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sword1 = HYPER_SHAPE as i16;
        al.sbyte1 = HYPER_OUT_TICKS;
    }
    // The source initializer falls through into the decrement, table lookup,
    // and first emitter tick.
    hyperspaceout_strat(g, idx);
}

pub fn hyperspaceout_strat(g: &mut Game, idx: u16) {
    let sb = g.objs.aliens[idx as usize].sbyte1;
    if sb == 0 {
        g.objs.aldead = 1;
        return;
    }
    let remaining = sb - 1;
    let phase = usize::from(remaining >> HYPER_OUT_PHASE_SHIFT);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte1 = remaining;
    al.sword1 = HYPER_OUT_SHAPES[phase] as i16;
    hyperspace_strat(g, idx);
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
    // GASTRATS.ASM:2547-2550 sets a -22.5-degree pitch offset, then invokes
    // the dedicated HPLASMA constructor rather than the generic laser lane.
    let shot = fire_hplasma_with_rotation(g, idx, DEG22.wrapping_neg(), 0);
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
        strat_gen_vecs_nvecs(al); // s_gen_vecs → nvecs_l (GASTRATS.ASM:2528)
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

/// C `bomwing_die` / `bomwingdie_Istrat` (strat_enemy.c:4271; GASTRATS.ASM:2578).
pub fn bomwingdie_istrat(g: &mut Game, idx: u16) {
    bomwing_die(g, idx);
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
        // ASM bomwing_Istrat (GASTRATS.ASM:2515-2523) sets no colltype.
        // (Audit A Minor 9)
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
        1 => {
            g.objs.aliens[idx as usize].sbyte1 = TADPOLE_DIVE_FRAMES;
            if let Some(pl) = pl {
                strat_aim_3d(g, idx, &pl, 3);
                let me = g.objs.aliens[idx as usize];
                // ASM GA2STRAT.ASM:2937 `s_jmp_zdistmore #1500,.ntopl` skips fire
                // when |dz|>=1500 — fire only when |dz|<1500. (Audit A Minor 3)
                if ((me.worldz as i32 - pl.worldz as i32).abs() as i16) < TADPOLE_FIRE_ZDIST {
                    let (rx, ry) = (me.rotx, me.roty);
                    let _ = strat_fire_relslowlaserhome(g, idx, rx, ry);
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
        // ROM `s_obj2obj_angle` — Yanglexy+nega into body roty.
        achase_angle(&mut roty, angle_xz(&me, &pl).wrapping_neg(), 1);
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
    // `s_weapon_rots2obj` uses Yanglexabs (no nega) — keep raw for fire.
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
    // ROM `s_fire_weapon x,RELSLOWELASER` → gen_weapon `jsl lasersound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
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
// STARBULL (GA2STRAT.ASM:40-119) — WP chase → face/fire → peel away.
// ============================================================

const STARBULL_HP: u8 = 16; // STRATEQU.INC:107
const STARBULL_AP: u8 = 1; // STRATEQU.INC:108
const STARBULL_DAMAGE_SMOKE_HP: u8 = STARBULL_HP - 4;

/// ROM `starbull_Istrat` (GA2STRAT.ASM:40-50).
pub fn starbull_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, starbull_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = STARBULL_HP;
        al.ap = STARBULL_AP;
        al.collflags |= COLLTYPE_ZENEMY;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
                                // s_set_wp x,x,1,#0,#0,#0 — WP1 = self pos
        al.swpx1 = al.worldx;
        al.swpy1 = al.worldy;
        al.swpz1 = al.worldz;
        al.stratstate = 0;
        al.snd2 = 2;
    }
}

/// ROM `starbull_strat` (GA2STRAT.ASM:52-61): copy player→WP1, offset, chase.
pub fn starbull_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = pl.worldx;
        al.swpy1 = pl.worldy;
        al.swpz1 = pl.worldz;
        al.swpz1 = al.swpz1.wrapping_add(1000); // al_sWPz1 += 1000
        al.swpy1 = al.swpy1.wrapping_add(-50); // al_sWPy1 += -50
    }
    let (wx, wy, wz) = {
        let al = &g.objs.aliens[idx as usize];
        (al.swpx1, al.swpy1, al.swpz1)
    };
    // s_goto_wp x,x,1,#50,1,2,0,#500,#0,starbull2
    if starbull_goto_wp(g, idx, wx, wy, wz) {
        starbull2(g, idx);
        return;
    }
    starbullc(g, idx);
}

/// `s_goto_wp` specialised: max50/accel1/chase2/skid0/mindist500/minspeed0.
fn starbull_goto_wp(g: &mut Game, idx: u16, wx: i16, wy: i16, wz: i16) -> bool {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let me = g.objs.aliens[idx as usize];
    let dx = wx.wrapping_sub(me.worldx);
    let dy = wy.wrapping_sub(me.worldy);
    let dz = wz.wrapping_sub(me.worldz);
    let rxz = (dx as i32).abs() + (dz as i32).abs();
    let hwp = (dy as i32).abs();
    let in_range = rxz < 500 && hwp < 500;
    let reached = if in_range {
        speed_to(&mut g.objs.aliens[idx as usize], 0, 2)
    } else {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
        false
    };
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    achase_angle(&mut roty, sf_core::aim_angle::yanglexy_nega(dx, dz), 2);
    achase_angle(&mut rotx, sf_core::aim_angle::xanglexabs(dy, dx, dz), 2);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        gen_vecs_3d(al);
    }
    reached
}

/// Shared tail (GA2STRAT.ASM:65-86): scroll, spin when on fire.
fn starbullc(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let _ = damage_smoke_srou(
        g,
        idx,
        STARBULL_DAMAGE_SMOKE_HP,
        SmokeCadence::EveryFourthFrame,
    );
    if g.objs.aliens[idx as usize].flags & AFONFIRE == 0 {
        return;
    }
    // s_jmp_NOTdelay 1,.nns → next_state when (gf&1)==0
    if g.vars.gameframe & 1 == 0 {
        let s = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
        g.objs.aliens[idx as usize].stratstate = if s <= 2 { s } else { 1 };
    }
    if g.objs.aliens[idx as usize].stratstate == 1 {
        g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_sub(8);
    } else {
        g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(8);
    }
}

/// ROM `starbull2` (GA2STRAT.ASM:86-91): enable collide, face-player phase.
fn starbull2(g: &mut Game, idx: u16) {
    let tick = sid(g, stbfp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_COLLDISABLE;
        al.stratptr = Some(tick);
        al.sbyte1 = 20;
        al.sflags2 &= !ASF2_SMFLAG1; // s_initface_player
    }
    starbullc(g, idx);
}

/// ROM `stbfp_strat` (GA2STRAT.ASM:93-107): face player then shoot burst.
pub fn stbfp_strat(g: &mut Game, idx: u16) {
    if starbull_face_player(g, idx, 2, 3) {
        // .stbshoot — s_beqdec_alvar sbyte1,stbgo_init
        let sb1 = g.objs.aliens[idx as usize].sbyte1;
        if sb1 == 0 {
            stbgo_init(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        // s_jmp_NOTdelay 2,.nfire
        if g.vars.gameframe & 3 == 0 {
            let yaw_off = ((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3);
            let me = g.objs.aliens[idx as usize];
            strat_fire_relslowlaser(g, idx, 0, me.roty.wrapping_add(yaw_off));
        }
        starbullc(g, idx);
        return;
    }
    starbullc(g, idx);
}

fn starbull_face_player(g: &mut Game, idx: u16, chase: u32, delay_bits: u16) -> bool {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1 != 0 {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        let yaw_aligned = achase_angle(&mut roty, me.sbyte3, chase);
        let mut rotx = me.rotx;
        let pitch_aligned = achase_angle(&mut rotx, me.sbyte4, chase);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = roty;
            al.rotx = rotx;
        }
        if yaw_aligned && pitch_aligned {
            return true;
        }
    }
    let gate = g.vars.gameframe & ((1u16 << delay_bits) - 1) == 0;
    if gate || g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1 == 0 {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SMFLAG1;
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            // ROM `s_face_player` latch: `s_obj2obj_3Dangle` → sbyte3/4 (nega).
            let yaw = angle_xz(&me, &pl).wrapping_neg();
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
    }
    false
}

/// ROM `stbgo_init` / `stbgo_strat` (GA2STRAT.ASM:109-119): peel away.
pub fn stbgo_init(g: &mut Game, idx: u16) {
    let tick = sid(g, stbgo_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    stbgo_strat(g, idx);
}

/// ROM `stbgo_strat`.
pub fn stbgo_strat(g: &mut Game, idx: u16) {
    // s_remove_offscn: set zremove; if not inviewpl, remove.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.type_ |= ATZREMOVE;
        if al.flags & 0x10 == 0 {
            // not inviewpl
            g.objs.aldead = 1;
            return;
        }
    }
    {
        let mut roty = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut roty, 0, 2);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = al.rotx.wrapping_sub(2);
    }
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 120, 5);
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    starbullc(g, idx);
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
    let lives = g.vars.strategy.lives;
    g.vars.strategy.lives = lives.wrapping_add(1);
    let mother = boss_child_from_index_raw(me.sword1 as u16);
    if let Some(m) = mother.filter(|&m| g.objs.aliens[m as usize].active) {
        up1man_remove_child_slot(g, m, 1);
        up1man_remove_child_slot(g, m, 3);
        up1man_remove_child_slot(g, m, 4);
    }
    g.objs.aldead = 1;
}

/// Shared body for `up1manchild{1,2,3}_Istrat` (GASTRATS.ASM:2763-2786) and the
/// mother spawn path — sets alptrs/HP/AP/offsets then runs the child strat.
fn up1manchild_init(g: &mut Game, idx: u16, off_x: i8, off_y: i8, rot_off: u8) {
    let s = sid(g, up1manchild_strat);
    let s_coll = sid(g, up1manhit_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
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
    up1manchild_strat(g, idx);
}

/// ROM `up1manchild1_Istrat` (GASTRATS.ASM:2763).
pub fn up1manchild1_istrat(g: &mut Game, idx: u16) {
    up1manchild_init(g, idx, -80, 75, DEG45.wrapping_add(DEG90));
}

/// ROM `up1manchild2_Istrat` (GASTRATS.ASM:2771).
pub fn up1manchild2_istrat(g: &mut Game, idx: u16) {
    up1manchild_init(g, idx, 80, 75, DEG45.wrapping_add(DEG180));
}

/// ROM `up1manchild3_Istrat` (GASTRATS.ASM:2779).
pub fn up1manchild3_istrat(g: &mut Game, idx: u16) {
    up1manchild_init(g, idx, 0, -90, 0);
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
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    up1manchild_init(g, child, off_x, off_y, rot_off);
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
    let me = g.objs.aliens[idx as usize];
    let (off_x, off_y) = crate::snes_trig::rotate_8yx(m.rotz, me.sbyte2 as i8, me.sbyte3 as i8);
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
            // The zero-pitch branch enters `zacos2_init`; that label fires
            // and falls through into `zacos2_strat` on this same frame.
            zacos2_init(g, idx);
            return;
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

            // GASTRATS.ASM branches into `zacos3_init`, whose initial pitch
            // step falls straight through into `zacos3_strat` for a second
            // step and the phase-2 movement on this same frame.
            zacos_phase2(g, idx);
            return;
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
        // s_weapon_rot #0,#0 + s_weapon_pos #0,#0,#40>>weapon_scale → RELSLOWELASER
        let me = g.objs.aliens[idx as usize];
        zacos_fire_relslowlaser(g, idx, me.rotx, me.roty);
    }
    if rotx == 4u8.wrapping_neg() {
        // s_jmp_alvarEQ B,x,al_rotx,#-4,zacos4_init
        let s = sid(g, zacos_phase3);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        // `zacos4_init` immediately falls through into `zacos4_strat`, so
        // acceleration, banking, and movement all begin on this frame.
        zacos_phase3(g, idx);
        return;
    }
    zacos_move(g, idx);
}

/// C `zacos4_strat` (GASTRATS.ASM:1001-1007) — s_speedto #60,1 + s_banktoplayer.
fn zacos_phase3(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
    szaco2_bank_to_player(g, idx);
    zacos_move(g, idx);
}

/// ROM `zacos_Istrat` — alias of [`strat_zacos_init`].
pub fn zacos_istrat(g: &mut Game, idx: u16) {
    strat_zacos_init(g, idx);
}

/// ROM `zacos_strat` — climb/pitch phase.
pub fn zacos_strat(g: &mut Game, idx: u16) {
    zacos_phase0(g, idx);
}

/// ROM `zacos_cont` — gen_3dvecs + addvecs + playerZ.
pub fn zacos_cont(g: &mut Game, idx: u16) {
    zacos_move(g, idx);
}

/// ROM `zacos2_init` — fire once then cruise until |dz|<2000.
pub fn zacos2_init(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    zacos_fire_relslowlaser(g, idx, me.rotx, me.roty);
    let s = sid(g, zacos_phase1);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    zacos_phase1(g, idx);
}

/// ROM `zacos2_strat`.
pub fn zacos2_strat(g: &mut Game, idx: u16) {
    zacos_phase1(g, idx);
}

// ============================================================
// HALFD (GA2STRAT.ASM:3201-3218) — hardHP door that opens when player near.
// ============================================================

/// ROM `halfd_Istrat` / `halfd_strat`.
pub fn halfd_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, halfd_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
    }
    halfd_strat(g, idx);
}

/// ROM `dpilar_Istrat` — same entry as `halfd_Istrat` (GA2STRAT.ASM:3200).
pub fn dpilar_istrat(g: &mut Game, idx: u16) {
    halfd_istrat(g, idx);
}

pub fn halfd_strat(g: &mut Game, idx: u16) {
    let far = player(g).map_or(true, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() >= 700
    });
    if far {
        // .close — reset anim when player is far.
        g.objs.aliens[idx as usize].animframe = 0;
    } else {
        // .open — dooropen_snd cosmetic; advance anim to 9.
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe != 9 {
            al.animframe = (al.animframe + 1) % 10;
        }
    }
}

// ============================================================
// POLE0 (GA2STRAT.ASM:3221-3266) — spinning hardHP pole; laser spins it.
// ============================================================

const POLE0_HF1: u8 = 0x01;
const POLE0_HF2: u8 = 0x02;
const POLE0_HF3: u8 = 0x04;

/// ROM `pole0_Istrat`.
pub fn pole0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, pole0_strat);
    let coll = sid(g, pole0col_istrat);
    let spin = if jmp_random_pct(g, 50) {
        3u8
    } else {
        (-3i8) as u8
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sbyte1 = spin;
        al.sbyte2 = 0;
    }
}

/// ROM `pole0_strat` — scroll +Z, spin by sbyte1, debounce sbyte2.
pub fn pole0_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(30);
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        // s_beqdec_alvar sbyte2 — dec when non-zero (debounce).
        if al.sbyte2 != 0 {
            al.sbyte2 -= 1;
        }
    }
}

/// ROM `pole0col_Istrat` — HF2/HF3 nudge spin; HF1 ignore; debounce 6.
pub fn pole0col_istrat(g: &mut Game, idx: u16) {
    let hf = g.objs.aliens[idx as usize].hitflags;
    if hf & POLE0_HF1 == 0 && g.objs.aliens[idx as usize].sbyte2 == 0 {
        g.objs.aliens[idx as usize].sbyte2 = 6;
        g.hooks.play_se(0x57);
        if hf & POLE0_HF2 != 0 {
            let s1 = g.objs.aliens[idx as usize].sbyte1 as i8;
            if s1 >= 0 {
                g.objs.aliens[idx as usize].sbyte1 = s1.wrapping_add(2) as u8;
            } else {
                g.objs.aliens[idx as usize].sbyte1 = 0;
            }
        } else if hf & POLE0_HF3 != 0 {
            let s1 = g.objs.aliens[idx as usize].sbyte1 as i8;
            if s1 == 0 || s1 < 0 {
                g.objs.aliens[idx as usize].sbyte1 = s1.wrapping_sub(2) as u8;
            } else {
                g.objs.aliens[idx as usize].sbyte1 = 0;
            }
        }
    }
    g.objs.aliens[idx as usize].hitflags = 0;
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    jmpto_strat(g, idx);
}

// ============================================================
// GROUNDPILON (GA2STRAT.ASM:3298-3367) — falling/stacking training pillar.
// ============================================================

const GROUNDPILON_HP: u8 = 16;
const GROUNDPILON_AP: u8 = 1;
const GROUNDPILON_LAND_Y: i16 = -35;
const GROUNDPILON_STACK_GAP: i16 = 70;
const GROUNDPILON_KNOCKBACK_RANDOM_MASK: u16 = 15;
const GROUNDPILON_KNOCKBACK_RANDOM_CENTER: i16 = 7;

fn groundpilon_can_stack_on(me: &Alien, candidate: &Alien) -> bool {
    candidate.worldy >= me.worldy
        && dist_xz(me, candidate) < GROUNDPILON_STACK_GAP
        && candidate.worldy.wrapping_sub(me.worldy).wrapping_abs() < GROUNDPILON_STACK_GAP
}

#[cfg(test)]
mod groundpilon_support_tests {
    use super::*;

    #[test]
    fn source_xz_metric_accepts_training_stack_edge() {
        let falling = Alien {
            worldx: 0,
            worldy: -140,
            worldz: 20_797,
            ..Alien::default()
        };
        let support = Alien {
            worldx: 4,
            worldy: -130,
            worldz: 20_877,
            ..Alien::default()
        };

        assert_eq!(dist_xz(&falling, &support), 69);
        assert!(groundpilon_can_stack_on(&falling, &support));
    }
}

pub fn groundpilon_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, groundpilon_strat);
    let hit = sid(g, strat_hit_flash);
    let explode = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(hit);
    al.expstratptr = Some(explode);
    al.hp = GROUNDPILON_HP;
    al.ap = GROUNDPILON_AP;
    al.sbyte1 = GROUNDPILON_HP;
    groundpilon_strat(g, idx);
}

pub fn groundpilon_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].hp != g.objs.aliens[idx as usize].sbyte1 {
        let hp = g.objs.aliens[idx as usize].hp;
        let random_x = (sf_random(&mut g.vars) & GROUNDPILON_KNOCKBACK_RANDOM_MASK) as i16
            - GROUNDPILON_KNOCKBACK_RANDOM_CENTER;
        // The source builds a 16-bit temporary with a second random draw for
        // its high byte, then masks that byte to zero. The value is discarded,
        // but advancing the shared stream remains observable by later logic.
        let _masked_high_byte = sf_random(&mut g.vars) & 0;
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = hp;
        al.vx = random_x;
        al.vy = -60;
        al.vz = 80;
        g.hooks.play_se(20);
    }

    apply_velocity(&mut g.objs.aliens[idx as usize]);

    if g.objs.aliens[idx as usize].worldy >= GROUNDPILON_LAND_Y {
        let had_shadow = g.objs.aliens[idx as usize].sflags & ASF_SHADOW != 0;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = 0;
        al.rotx = 0;
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.sflags &= !ASF_SHADOW;
        if had_shadow {
            g.hooks.play_se(134);
        }
        return;
    }

    g.objs.aliens[idx as usize].sflags &= !ASF_SHADOW;
    if g.objs.aliens[idx as usize].vy >= 0 {
        let me = g.objs.aliens[idx as usize];
        let support = g.objs.active_indices().into_iter().find(|&other| {
            if other == idx {
                return false;
            }
            let candidate = g.objs.aliens[other as usize];
            candidate.shape == sh::PILON && groundpilon_can_stack_on(&me, &candidate)
        });
        if let Some(support) = support {
            let support_y = g.objs.aliens[support as usize].worldy;
            let had_shadow = g.objs.aliens[idx as usize].sflags & ASF_SHADOW != 0;
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = support_y.wrapping_sub(GROUNDPILON_STACK_GAP);
            al.rotx = 0;
            al.vx = 0;
            al.vy = 0;
            al.vz = 0;
            al.sflags &= !ASF_SHADOW;
            if had_shadow {
                g.hooks.play_se(134);
            }
            return;
        }
    }

    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_SHADOW;
    al.vy = al.vy.wrapping_add(5);
    if al.vz != 0 {
        al.rotx = al.rotx.wrapping_add(10);
        al.roty = al.roty.wrapping_add(5);
    }
}

// ============================================================
// KAMI (GASTRATS.ASM:1744-1810) — weaving kamikaze → die dive → hardHP chase.
// ============================================================

const KAMI_HP: u8 = 4;
const KAMI_AP: u8 = 8;

/// ROM `kami_Istrat`.
pub fn kami_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, kami_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, kamidie_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = KAMI_HP;
        al.ap = KAMI_AP;
        al.vel = 20;
        al.sbyte1 = sf_random(&mut g.vars) as u8;
        al.collflags |= COLLTYPE_ZENEMY;
    }
}

/// ROM `kami_strat` — sintab weave on vx; derive yaw/roll from vx low byte.
pub fn kami_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = al.sbyte1.wrapping_add(2);
        al.vz = -14;
        al.vy = 1;
        al.vx = bee_tab_scaled(al.sbyte1, false, -3);
        let vx_lo = al.vx as u8;
        al.roty = vx_lo.wrapping_shl(2).wrapping_add(DEG180);
        al.rotz = vx_lo.wrapping_shl(1).wrapping_add(DEG90);
    }
    kami_cont(g, idx);
}

/// ROM `kami_cont`.
pub fn kami_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `kamiDIE_Istrat` — med-exp flash then dive strat.
pub fn kamidie_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, kamidie_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(tick);
    // makeMEDexpobj_srou — cosmetic flash (optional).
    let _ = make_medium_exp_obj(g, idx);
    kamidie_strat(g, idx);
}

/// ROM `kamiDIE_strat` — smoke + dive until y>=-100 → kamigo.
pub fn kamidie_strat(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 1 == 0 {
        let _ = crate::common::makesmoke_srou(g, idx);
    }
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 1);
    // s_jmp_lower x,#-100,kamigo_init — when worldy >= -100.
    if g.objs.aliens[idx as usize].worldy >= -100 {
        kamigo_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_jmp_alvarMORE B,x,al_rotx,#deg45,.maxx — SIGNED (STRATMAC.INC):
        // add +4 while (i8)rotx <= deg45.
        if (al.rotx as i8) <= DEG45 as i8 {
            al.rotx = al.rotx.wrapping_add(4);
        }
        gen_vecs_3d(al);
        al.rotz = al.rotz.wrapping_add(4);
    }
    kami_cont(g, idx);
}

/// ROM `kamiGO_init` / `kamiGO_strat` — hardHP chase.
pub fn kamigo_init(g: &mut Game, idx: u16) {
    let tick = sid(g, kamigo_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.hp = HARD_HP;
    }
    kamigo_strat(g, idx);
}

pub fn kamigo_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 80, 1);
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(4);
    if g.vars.gameframe & 3 == 0 {
        let _ = crate::common::makesmoke_srou(g, idx);
    }
    let close = player(g).map_or(false, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() < 400
    });
    if close {
        kami_cont(g, idx);
        return;
    }
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 3);
        gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    }
    kami_cont(g, idx);
}

// ============================================================
// EVADER (GASTRATS.ASM:1499-1555) — dodge to random offset WPs; fire when aimed.
// ============================================================

const EVADER_HP: u8 = 8;
const EVADER_AP: u8 = 4;
const EVADER_X_POS: [i16; 4] = [-400, 400, -100, 100];
const EVADER_Y_POS: [i16; 4] = [-300, -200, -100, -50];
const EVADER_Z_POS: [i16; 4] = [1000, 800, 1600, 1200];

/// ROM `evader_Istrat` — arm then pick a random offset WP.
pub fn evader_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, evader_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = EVADER_HP;
        al.ap = EVADER_AP;
        al.collflags |= COLLTYPE_ZENEMY;
    }
    evader_new_pos(g, idx);
}

/// Pick random X/Y/Z offsets into swp*1 / sword1; switch to A phase.
fn evader_new_pos(g: &mut Game, idx: u16) {
    let ix = ((sf_random(&mut g.vars) as u8) & 3) as usize;
    let iy = ((sf_random(&mut g.vars) as u8) & 3) as usize;
    let iz = ((sf_random(&mut g.vars) as u8) & 3) as usize;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = EVADER_X_POS[ix];
        al.swpy1 = EVADER_Y_POS[iy];
        al.sword1 = EVADER_Z_POS[iz];
    }
    let tick = sid(g, evadera_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
}

/// ROM `evaderA_strat` — fly to player_z + sword1 at (swpx1, swpy1).
pub fn evadera_strat(g: &mut Game, idx: u16) {
    let (wx, wy, wz) = {
        let al = &g.objs.aliens[idx as usize];
        let pz = player(g).map(|p| p.worldz).unwrap_or(0);
        (al.swpx1, al.swpy1, pz.wrapping_add(al.sword1))
    };
    // s_goto_wp x,x,1,#50,8,3,0,#500,#0,evader_init
    if evader_goto_wp(g, idx, wx, wy, wz) {
        evader_init(g, idx);
        return;
    }
    evader_cont(g, idx);
}

/// `s_goto_wp` specialised: max50/accel8/chase3/skid0/mindist500/minspeed0.
/// Returns true when in range and speed reached 0 (→ evader_init).
fn evader_goto_wp(g: &mut Game, idx: u16, wx: i16, wy: i16, wz: i16) -> bool {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let me = g.objs.aliens[idx as usize];
    let dx = wx.wrapping_sub(me.worldx);
    let dy = wy.wrapping_sub(me.worldy);
    let dz = wz.wrapping_sub(me.worldz);
    let rxz = (dx as i32).abs() + (dz as i32).abs();
    let hwp = (dy as i32).abs();
    let in_range = rxz < 500 && hwp < 500;
    let reached = if in_range {
        speed_to(&mut g.objs.aliens[idx as usize], 0, 2)
    } else {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 8);
        false
    };
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    // s_obj2WP_angle: nega(Yanglexy) + Xanglexabs
    achase_angle(&mut roty, sf_core::aim_angle::yanglexy_nega(dx, dz), 3);
    achase_angle(&mut rotx, sf_core::aim_angle::xanglexabs(dy, dx, dz), 3);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        // skid=0 → gen from roty directly.
        gen_vecs_3d(al);
    }
    reached
}

/// ROM `evader_init` — switch to aim/fire strat.
pub fn evader_init(g: &mut Game, idx: u16) {
    let tick = sid(g, evader_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    evader_strat(g, idx);
}

/// ROM `evader_strat` — aim at player; fire home laser when aligned.
pub fn evader_strat(g: &mut Game, idx: u16) {
    let aligned = if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        let mut rotx = me.rotx;
        // ROM `s_obj2obj_3dangle` into body rots — Yanglexy+nega.
        let yaw_aligned = achase_angle(&mut roty, angle_xz(&me, &p).wrapping_neg(), 3);
        let pitch_aligned = achase_angle(&mut rotx, strat_pitch_toward(&me, &p), 3);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = roty;
            al.rotx = rotx;
        }
        yaw_aligned && pitch_aligned
    } else {
        false
    };
    if aligned && g.vars.gameframe & 7 == 0 {
        // s_jmp_NOTdelay 3 — fire when (gf&7)==0; weapon_rot #0,#0 = object aim.
        let me = g.objs.aliens[idx as usize];
        let _ = strat_fire_relslowlaserhome(g, idx, me.rotx, me.roty);
    }
    evader_cont(g, idx);
}

/// ROM `evader_cont` — playerZ; dodge if player laser nearby.
pub fn evader_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    // s_find_nearobj y,x,#elaser2,#0,#350 → if found, pick new WP.
    if strat_find_near_shape(g, idx, SHAPE_ELASER2, None, 350, 350).is_some() {
        evader_new_pos(g, idx);
    }
}

// ============================================================
// TRUCK1 / TRUCK2 (GA2STRAT.ASM:2490-2527) — air trucks.
// ============================================================

const AIRTRUCK_HP: u8 = 16;
const TRUCK_AP: u8 = 8;

/// ROM `truck1_Istrat` / `truck1_strat` — drift toward camera.
pub fn truck1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, truck1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = AIRTRUCK_HP;
        al.ap = TRUCK_AP;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
        al.count = 60;
        al.snd2 = 0x04;
    }
}

pub fn truck1_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(35);
}

/// ROM `truck2_Istrat` / `truck2_strat` — drunk weave via sintab.
pub fn truck2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, truck2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = AIRTRUCK_HP;
        al.ap = TRUCK_AP;
        al.vel = 60;
        al.sbyte1 = 50;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
        al.snd2 = 0x04;
    }
}

pub fn truck2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_set_alvar2alvartab B,B,B,...,SINtab,-2 → low byte of scaled sintab.
        al.roty = bee_tab_scaled(al.sbyte1, false, -2) as u8;
        al.sbyte1 = al.sbyte1.wrapping_add(6);
        let mut skidy = al.skidy;
        achase_angle(&mut skidy, al.roty, 3);
        al.skidy = skidy;
        let saved = al.roty;
        al.roty = al.skidy;
        gen_vecs_3d(al);
        al.roty = saved;
        al.vz = -35;
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

// ============================================================
// HARDENEMY1 / HARD90YRFOG (GSTRATS / KSTRATS hardvars stubs)
// ============================================================

/// ROM `hardenemy1_Istrat` (GSTRATS.ASM:639-646) — enemy1 coll + hardvars, inert.
pub fn hardenemy1_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1;
    set_hard_vars(al);
    al.stratptr = None;
}

/// ROM `fog_strat` (KSTRATS.ASM:347-349).
pub fn fog_strat(g: &mut Game, idx: u16) {
    update_fog_visibility(g, idx);
}

/// ROM `hard180YRfog_Istrat` (KSTRATS.ASM:340-345): retain the source shape,
/// face 180 degrees, install hard durability, and run the fog visibility tick.
/// Unlike GSTRATS' ordinary `hard180YR`, this initializer does not assign an
/// enemy collision class.
pub fn hard180yrfog_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fog_strat);
    let al = &mut g.objs.aliens[idx as usize];
    init_fog_visibility(al);
    al.roty = DEG180;
    set_hard_vars(al);
    al.stratptr = Some(tick);
}

/// ROM `hard90YRfog_Istrat` (KSTRATS.ASM:333-338) — face 180°, hardvars, fog tick.
pub fn hard90yrfog_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fog_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = DEG180;
    set_hard_vars(al);
    al.stratptr = Some(tick);
}

// ============================================================
// SHARK (GASTRATS.ASM:1686-1745) — aim at player, anim, mine-drop climb, dash.
// ============================================================

const SHARK_HP: u8 = 4;
const SHARK_AP: u8 = 6;

/// ROM `shark_Istrat`.
pub fn shark_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, shark_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SHARK_HP;
        al.ap = SHARK_AP;
        al.collflags |= COLLTYPE_ENEMYWEAP | COLLTYPE_ENEMY1 | COLLTYPE_ZENEMY;
        // s_set_alvar W,x,al_ptr,playpt — player slot 0.
        al.ptr = 0;
        al.animframe = 0;
    }
}

/// ROM `shark_strat` — advance anim to 15; at |dz|<1100 → sharka, else aim/move.
pub fn shark_strat(g: &mut Game, idx: u16) {
    // s_jmp_notdelay 1,.nmax — every other frame.
    if frame_tick_mod(g, 1) {
        let af = g.objs.aliens[idx as usize].animframe;
        if af != 15 {
            // s_add_anim x,#1,#16
            g.objs.aliens[idx as usize].animframe = af.wrapping_add(1) % 16;
        }
    }
    let target = shark_target(g, idx);
    let dz_close = target.map_or(false, |t| {
        (g.objs.aliens[idx as usize].worldz as i32 - t.worldz as i32).abs() < 1100
    });
    if dz_close {
        sharka_init(g, idx);
        return;
    }
    shark_cont2(g, idx);
}

fn shark_target(g: &Game, idx: u16) -> Option<Alien> {
    let ptr = g.objs.aliens[idx as usize].ptr as usize;
    if ptr < NUMBER_AL && g.objs.aliens[ptr].active {
        Some(g.objs.aliens[ptr])
    } else {
        player(g)
    }
}

/// ROM `shark_cont2` — aim when |dz|>=500, then cont.
pub fn shark_cont2(g: &mut Game, idx: u16) {
    if let Some(t) = shark_target(g, idx) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - t.worldz as i32).abs();
        if dz >= 500 {
            strat_aim_3d(g, idx, &t, 3);
        }
    }
    shark_cont(g, idx);
}

/// ROM `shark_cont` — gen 3dvecs + playerZ + addvecs.
pub fn shark_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `sharka_init` — mine-drop climb phase.
pub fn sharka_init(g: &mut Game, idx: u16) {
    let tick = sid(g, sharka_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 30;
        // s_set_alvar2rnd x,al_sbyte2,#15 ; s_sub #7 → [-7,+8]
        al.sbyte2 = ((sf_random(&mut g.vars) as u8) & 15).wrapping_sub(7);
    }
    sharka_strat(g, idx);
}

/// ROM `sharka_strat` — climb + drop mines; then dash.
pub fn sharka_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar B,x,al_sbyte1,.dash — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        // .dash
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
        shark_cont2(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 2);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_sub(4);
        al.roty = al.roty.wrapping_add(al.sbyte2);
    }
    // s_jmp_NOtdelay 2,.nfire,al1pt
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 3 == 0 {
        shark_drop_mine(g, idx);
    }
    shark_cont(g, idx);
}

/// Drop a `mine0` at shark position (GASTRATS.ASM:1732-1735).
/// Inlined (enemies_ground↔enemy_a would cycle) — same fields as `mine0_init`.
fn shark_drop_mine(g: &mut Game, shark_idx: u16) {
    let Some(mine) = make_obj(g, 0) else {
        return;
    };
    let (x, y, z) = {
        let s = &g.objs.aliens[shark_idx as usize];
        (s.worldx, s.worldy, s.worldz)
    };
    let tick = sid(g, shark_mine0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let rotz = sf_random(&mut g.vars) as u8;
    {
        let al = &mut g.objs.aliens[mine as usize];
        al.worldx = x;
        al.worldy = y;
        al.worldz = z;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = 2; // mine0HP
        al.ap = 10; // mine0AP
        al.collflags |= COLLTYPE_ENEMY1;
        al.rotz = rotz;
    }
}

fn shark_mine0_strat(_g: &mut Game, _idx: u16) {}

// ============================================================
// FZACO (GASTRATS.ASM:811-870) — friend zaco: brake → aim/fire → climb out.
// ============================================================

const FZACO_HP: u8 = 4;
const FZACO_AP: u8 = 8;

/// ROM `fzaco_Istrat`.
pub fn fzaco_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fzaco_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let (px, py) = (g.vars.player_posx, g.vars.player_posy);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = FZACO_HP;
        al.ap = FZACO_AP;
        al.type_ &= !ATZREMOVE; // s_setnoremove_behind
        al.vel = 50;
        al.roty = DEG22;
        al.rotx = 0u8.wrapping_sub(DEG5); // #-deg5
        al.worldx = al.worldx.wrapping_add(px);
        al.worldy = al.worldy.wrapping_add(py);
        al.collflags |= COLLTYPE_ENEMYWEAP;
    }
}

/// ROM `fzaco_strat` / `Fzaco_strat` — brake to 0 then fzaco2.
pub fn fzaco_strat(g: &mut Game, idx: u16) {
    if speed_to(&mut g.objs.aliens[idx as usize], 0, 1) {
        fzaco2_init(g, idx);
        return;
    }
    fzaco_cont(g, idx);
}

/// ROM `fzaco2_init`.
pub fn fzaco2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, fzaco2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte2 = 60;
    }
    fzaco2_strat(g, idx);
}

/// ROM `fzaco2_strat` — aim + fire when facing neg-Z; then fzaco3.
pub fn fzaco2_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 3);
    }
    // s_jmpNOT_objpointnegZ → fire only when roty in [96,160].
    let roty = g.objs.aliens[idx as usize].roty;
    let facing_neg_z = (DEG180 - DEG45..=DEG180 + DEG45).contains(&roty);
    if facing_neg_z && (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 0x0F == 0 {
        let me = g.objs.aliens[idx as usize];
        // s_weapon_rot #0,#0 — fire along object aim.
        strat_fire_relslowlaser(g, idx, me.rotx, me.roty);
    }
    // s_beqdec_alvar B,x,al_sbyte2,fzaco3_init
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        fzaco3_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    fzaco_cont(g, idx);
}

/// ROM `fzaco3_init`.
pub fn fzaco3_init(g: &mut Game, idx: u16) {
    let tick = sid(g, fzaco3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.type_ |= ATZREMOVE; // s_setremove_behind
    }
    fzaco3_strat(g, idx);
}

/// ROM `fzaco3_strat` — speed up + peel when close.
pub fn fzaco3_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
    if let Some(p) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs();
        if dz < 500 {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = al.rotx.wrapping_sub(1);
            al.roty = al.roty.wrapping_sub(1);
        }
    }
    fzaco_cont(g, idx);
}

/// ROM `fzaco_cont` — sintab/costab orbit offset then cont2.
pub fn fzaco_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let tpx = bee_tab_scaled(al.sbyte1, false, -3); // sintab,-3
        let tpy = bee_tab_scaled(al.sbyte1, true, -3); // costab,-3
        al.worldx = al.worldx.wrapping_add(tpx);
        al.worldy = al.worldy.wrapping_add(tpy);
        al.sbyte1 = al.sbyte1.wrapping_add(6);
    }
    fzaco_cont2(g, idx);
}

/// ROM `fzaco_cont2` — gen 3dvecs + addvecs + playerZ.
pub fn fzaco_cont2(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

// ============================================================
// AIRCAR1–5 (GA2STRAT.ASM:2265-2490) — colony air cars.
// ============================================================

const AIRCAR_HP: u8 = 10;
const AIRCAR_AP: u8 = 10;
const COLONY_MAX_X: i16 = 120; // STRATEQU.INC:675
const AIRCAR_BARRIER_HP: u8 = 6;
const AIRCAR_BARRIER_AP: u8 = 12;

fn aircar_arm(g: &mut Game, idx: u16, tick: StrategyFn) {
    let s = sid(g, tick);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = AIRCAR_HP;
    al.ap = AIRCAR_AP;
    al.collflags |= COLLTYPE_ENEMY1;
    al.snd2 = 0x0F;
}

/// ROM `aircar1_Istrat` — side approach, skid, stop.
pub fn aircar1_istrat(g: &mut Game, idx: u16) {
    aircar_arm(g, idx, aircar1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = 0u8.wrapping_sub(DEG90);
        al.sbyte1 = 25;
        al.roty = 1;
        crate::common::init_colanim(al, 0);
    }
}

/// ROM `aircar1_strat`.
pub fn aircar1_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar sbyte1,.stop — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            crate::common::init_colanim(al, 1);
            let mut rz = al.rotz;
            achase_angle(&mut rz, 0, 3);
            al.rotz = rz;
        }
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    let (px, py) = (g.vars.player_posx, g.vars.player_posy);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 > 15 {
            let mut rz = al.rotz;
            achase_angle(&mut rz, 0, 3);
            al.rotz = rz;
        } else {
            al.sflags |= ASF_SHADOW;
            al.rotz = al.rotz.wrapping_add(1);
        }
        al.worldx = chase_proportional(al.worldx, px, 3);
        al.worldy = chase_proportional(al.worldy, py, 3);
    }
    add_player_z(g, idx);
}

/// ROM `aircar2_Istrat` — from behind, stop, drop barrier, peel away.
pub fn aircar2_istrat(g: &mut Game, idx: u16) {
    aircar_arm(g, idx, aircar2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = 0u8.wrapping_sub(DEG90);
        al.sbyte1 = 50;
        al.stratstate = 0;
        crate::common::init_colanim(al, 0);
    }
}

/// ROM `aircar2_strat`.
pub fn aircar2_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        let px = g.vars.player_posx;
        {
            let al = &mut g.objs.aliens[idx as usize];
            let mut rz = al.rotz;
            achase_angle(&mut rz, 0, 4);
            al.rotz = rz;
            al.worldx = chase_proportional(al.worldx, px, 4);
            al.worldz = al.worldz.wrapping_add(20);
        }
        add_player_z(g, idx);
        // s_decbne_alvar sbyte1,.nsto — DEC-then-BNE: stay in state0 while !=0.
        let s1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = s1;
        if s1 != 0 {
            return;
        }
        aircar2_drop_barrier(g, idx);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratstate = al.stratstate.wrapping_add(1);
            al.vx = 0;
            al.vy = 0;
            al.vz = -30;
        }
    }
    if g.objs.aliens[idx as usize].stratstate == 1 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotz = al.rotz.wrapping_add(1);
            al.roty = al.roty.wrapping_add(1);
        }
        add_player_z(g, idx);
        apply_velocity(&mut g.objs.aliens[idx as usize]);
        if g.objs.aliens[idx as usize].vx != -10 {
            g.objs.aliens[idx as usize].vx = g.objs.aliens[idx as usize].vx.wrapping_sub(1);
        }
    }
}

/// Drop barrier at car (inline of `barrier_istrat` — bosses↔enemy_a cycle).
fn aircar2_drop_barrier(g: &mut Game, car: u16) {
    let Some(b) = make_obj(g, 0) else {
        return;
    };
    let (x, y, z, rx, ry, rz) = {
        let c = &g.objs.aliens[car as usize];
        (c.worldx, c.worldy, c.worldz, c.rotx, c.roty, c.rotz)
    };
    let tick = sid(g, aircar_barrier_idle);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[b as usize];
        al.worldx = x;
        al.worldy = y;
        al.worldz = z;
        al.rotx = rx;
        al.roty = ry;
        al.rotz = rz;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = AIRCAR_BARRIER_HP;
        al.ap = AIRCAR_BARRIER_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        // s_make_immune — mutual immuneptr.
        al.immuneptr = car;
    }
    g.objs.aliens[car as usize].immuneptr = b;
}

fn aircar_barrier_idle(_g: &mut Game, _idx: u16) {}

/// ROM `aircar3_Istrat` — maniac weave from the left.
pub fn aircar3_istrat(g: &mut Game, idx: u16) {
    aircar_arm(g, idx, aircar3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 40;
        al.sbyte1 = 50;
        al.sflags |= ASF_SHADOW;
        al.rotz = 0u8.wrapping_sub(DEG90);
        al.stratstate = 0;
        al.sword1 = 0;
        crate::common::init_colanim(al, 0);
    }
}

/// ROM `aircar3_strat`.
pub fn aircar3_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            let mut rz = al.rotz;
            achase_angle(&mut rz, 0, 4);
            al.rotz = rz;
            al.worldx = chase_proportional(al.worldx, 0, 4);
        }
        // s_decbne_alvar sbyte1,.nchp
        let s1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = s1;
        if s1 == 0 {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratstate = 1;
            al.sbyte1 = 0;
        }
    }
    if g.objs.aliens[idx as usize].stratstate == 1 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = bee_tab_scaled(al.sbyte1, false, -2) as u8;
            al.sbyte1 = al.sbyte1.wrapping_add(5);
            al.rotz = bee_tab_scaled(al.sbyte1, false, -3) as u8;
            al.sword1 = al.sword1.wrapping_add(5);
            if al.sword1 >= 256 + 64 {
                al.stratstate = 2;
                al.count = 40; // s_set_lifecnt #40
            }
        }
    }
    if g.objs.aliens[idx as usize].stratstate == 2 {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
        aircar_dec_lifecnt(g, idx);
        if frame_tick_mod(g, 1) {
            g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(1);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        let mut skidy = al.skidy;
        achase_angle(&mut skidy, al.roty, 3);
        al.skidy = skidy;
        let saved = al.roty;
        al.roty = al.skidy;
        gen_vecs_3d(al);
        al.roty = saved;
        al.vz = 12;
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// ROM `aircar4_Istrat` — idle then speed up when player near.
pub fn aircar4_istrat(g: &mut Game, idx: u16) {
    aircar_arm(g, idx, aircar4_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        crate::common::init_colanim(al, 1);
        al.count = 60;
        al.sflags2 &= !ASF2_SFLAG1;
    }
}

/// ROM `aircar4_strat`.
pub fn aircar4_strat(g: &mut Game, idx: u16) {
    {
        let px = g.vars.player_posx;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, px, 4);
    }
    let dz = player(g).map_or(i32::MAX, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs()
    });
    if dz >= 600 {
        let py = g.vars.player_posy;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = chase_proportional(al.worldy, py, 4);
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        if dz < 400 {
            let al = &mut g.objs.aliens[idx as usize];
            crate::common::init_colanim(al, 0);
            al.sflags2 |= ASF2_SFLAG1;
        }
    }
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        // s_speedto #medpspeed+15,5,.go — MEDPSPEED=65 → 80.
        if speed_to(
            &mut g.objs.aliens[idx as usize],
            MEDPSPEED_I16 as u8 + 15,
            5,
        ) {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = al.roty.wrapping_add(1);
                al.rotz = al.rotz.wrapping_add(1);
            }
            aircar_dec_lifecnt(g, idx);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
}

/// ROM `aircar5_Istrat` — from left, hit colony wall, tumble.
pub fn aircar5_istrat(g: &mut Game, idx: u16) {
    aircar_arm(g, idx, aircar5_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 40;
        al.sbyte1 = 50;
        al.sflags |= ASF_SHADOW;
        al.rotz = DEG90;
        al.roty = 0u8.wrapping_sub(DEG22);
        al.stratstate = 0;
        crate::common::init_colanim(al, 0);
    }
}

/// ROM `aircar5_strat`.
pub fn aircar5_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            let mut rz = al.rotz;
            achase_angle(&mut rz, 0, 4);
            al.rotz = rz;
        }
        // s_jmp_alvarless W,x,al_worldx,#colony_maxX-20,.nchp
        if g.objs.aliens[idx as usize].worldx >= COLONY_MAX_X - 20 {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate = 1;
                al.vx = -5;
                al.vy = -10;
                al.vz = -20;
                al.rotz = DEG11;
                al.sbyte1 = 16;
            }
            if let Some(exp) = make_small_exp_obj(g, idx) {
                g.objs.aliens[exp as usize].worldx = COLONY_MAX_X;
            }
        }
    }
    if g.objs.aliens[idx as usize].stratstate == 1 {
        // s_jmp_notdelay 1,.cadd ; s_beqdec sbyte1,.nd ; .cadd chase roty + add rotz
        let do_cadd = if frame_tick_mod(g, 1) {
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                false
            } else {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
                true
            }
        } else {
            true // .cadd when notdelay skips the beqdec
        };
        if do_cadd {
            let al = &mut g.objs.aliens[idx as usize];
            let mut ry = al.roty;
            achase_angle(&mut ry, DEG22, 2);
            al.roty = ry;
            al.rotz = al.rotz.wrapping_add(al.sbyte1);
        }
        // s_falldown_Yvec x,1,#2,#-25
        let _ = aircar_falldown_yvec(&mut g.objs.aliens[idx as usize], 1, 2, -25);
    } else {
        {
            let al = &mut g.objs.aliens[idx as usize];
            gen_vecs_3d(al);
            al.vx <<= 1; // s_scale_alvar W,x,al_vx,1
            al.vz = 20;
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

fn aircar_falldown_yvec(al: &mut Alien, shift: u32, gravity: i16, ground: i16) -> bool {
    al.vy = al.vy.wrapping_add(gravity);
    if al.worldy < ground {
        return false;
    }
    al.worldy = ground;
    let mut v = al.vy.wrapping_neg();
    v >>= shift;
    if (-5..=0).contains(&v) {
        v = 0;
    }
    al.vy = v;
    v == 0
}

/// `s_dec_lifecnt` (no kill flag) — DEC count; remove when result is 0.
fn aircar_dec_lifecnt(g: &mut Game, idx: u16) {
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = c;
    if c == 0 {
        g.objs.aldead = 1;
    }
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
    let me = g.objs.aliens[idx as usize];
    // `s_weapon_rot #-deg22,#0` is relative to the firer's copied rots.
    g.objs.aliens[idx as usize].rotx = me.rotx.wrapping_sub(DEG22);
    let shot = fire_shortplasma(g, idx);
    g.objs.aliens[idx as usize].rotx = me.rotx;
    let Some(shot) = shot else {
        return;
    };
    // `s_weapon_pos #0,#-62>>weapon_scale,#40>>weapon_scale`, followed by
    // gen_weapon's rotate8 chain and ASL×weapon_scale.
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        me.rotz,
        me.rotx,
        me.roty,
        0,
        (-62i16 >> 2) as i8,
        (40i16 >> 2) as i8,
        2,
    );
    let al = &mut g.objs.aliens[shot as usize];
    al.worldx = me.worldx.wrapping_add(rx);
    al.worldy = me.worldy.wrapping_add(ry);
    al.worldz = me.worldz.wrapping_add(rz);
}

/// C `houdai_strat` (strat_enemy.c:5057). Public for Audit A #3 cadence tests.
pub fn houdai_strat(g: &mut Game, idx: u16) {
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
    g.objs.aliens[idx as usize].sbyte3 = 3;
    let Some(target) = strat_find_near_shape(g, idx, SH_HOUDAI_0, None, 10000, 10000) else {
        // `z34exit` returns without replacing the installed initializer. This
        // is a deliberate retry: the target may be authored later in the map.
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
    al.collflags |= COLLTYPE_ENEMY1;
    al.snd2 = 1;
}

/// ROM `zaco3_Istrat` / `zaco3_strat` aliases.
pub fn zaco3_istrat(g: &mut Game, idx: u16) {
    strat_zaco3_init(g, idx);
}

pub fn zaco3_strat(g: &mut Game, idx: u16) {
    zaco3_attack(g, idx);
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
    // KSTRATS.ASM falls through after the medium flash into zaco3DIE_strat,
    // so both the effect and the first dive update happen on the death frame.
    let _ = make_medium_exp_obj(g, idx);
    zaco3die_strat(g, idx);
}

/// C `zaco3die_strat` (strat_enemy.c:5271 / KSTRATS.ASM:166-177).
fn zaco3die_strat(g: &mut Game, idx: u16) {
    // s_jmp_NOTdelay 1,.nsm — smoke every other frame. (Audit A Minor 15)
    if g.vars.gameframe & 1 == 0 {
        let _ = crate::common::makesmoke_srou(g, idx);
    }
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
    // ASM KSTRATS.ASM:175 `s_add_playerZ x` runs inline BEFORE the brl into
    // zaco3cont, which applies add_playerZ a second time. The die frame
    // therefore scrolls the corpse twice (retail tick-1733 corpse moved
    // pviewvelz*2 + vz while a single application lagged one scroll frame).
    add_player_z(g, idx);
    // zaco3cont: s_add_playerZ + s_add_vecs2pos (once each).
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// C `zaco3go_strat` (strat_enemy.c:5293 / KSTRATS.ASM:182-200).
fn zaco3go_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(4);
    }
    // s_jmp_NOTdelay 2,.nsm — smoke every 4th frame; smoke vz=#40. (Audit A #15)
    if g.vars.gameframe & 3 == 0 {
        if let Some(smoke) = crate::common::makesmoke_srou(g, idx) {
            g.objs.aliens[smoke as usize].vz = 40;
        }
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
    g.objs.aliens[idx as usize].sbyte3 = 4;
    let Some(target) = strat_find_near_shape(g, idx, SH_PILLAR3, None, 10000, 10000) else {
        // Keep the initializer installed, matching `z34exit`. Corneria's
        // first kamikaze relies on retrying after its pillars arrive.
        return;
    };
    let s = sid(g, zaco4_attack);
    let s_coll = sid(g, strat_hit_flash);
    // zaco4 shares the source `zaco34_Istrat` tail with zaco3, including its
    // authored dive death rather than the generic explosion strategy.
    let s_exp = sid(g, zaco3die_init);
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
        // ROM `s_cmp_alvars W` + `s_bcs .noadd` — unsigned worldy compare
        // (KSTRATS.ASM:216-217). Signed `<` matches in-band (both negative)
        // but diverges when one side crosses into the high unsigned half.
        if (g.objs.aliens[idx as usize].worldy as u16) < (pl.worldy as u16) {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = al.worldy.wrapping_add(20);
            // ROM `cmp #-30` / `bcc .ok` — unsigned clamp to -30.
            if (al.worldy as u16) >= ((-30i16) as u16) {
                al.worldy = -30;
            }
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(43);
    }
    if let Some(pl) = pl {
        // ROM `s_cmp_alvars` + `s_bpl zaco0b` — signed worldx compare
        // (N flag after CMP); keep signed here.
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
            let spread_pitch = ((sf_random(&mut g.vars) & 3) as i8).wrapping_sub(1);
            let spread_yaw = ((sf_random(&mut g.vars) & 3) as i8).wrapping_sub(1);
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

/// ROM `zaco0_Istrat` — alias of [`strat_zaco0_init`].
pub fn zaco0_istrat(g: &mut Game, idx: u16) {
    strat_zaco0_init(g, idx);
}

/// ROM `zaco0_strat` — sweep toward player X/Y.
pub fn zaco0_strat(g: &mut Game, idx: u16) {
    zaco0_sweep(g, idx);
}

/// ROM `zaco0b_Istrat` / `zaco0b_strat` — turn to face player.
pub fn zaco0b_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, zaco0_turn_in);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    zaco0_turn_in(g, idx);
}

pub fn zaco0b_strat(g: &mut Game, idx: u16) {
    zaco0_turn_in(g, idx);
}

/// ROM `zaco0c_Istrat` — fire burst (same body as strat until c2).
pub fn zaco0c_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, zaco0_fire);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    zaco0_fire(g, idx);
}

/// ROM `zaco0c2_Istrat` / `zaco0c_strat` — turn out.
pub fn zaco0c2_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, zaco0_turn_out);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    zaco0_turn_out(g, idx);
}

pub fn zaco0c_strat(g: &mut Game, idx: u16) {
    zaco0_turn_out(g, idx);
}

/// ROM `zaco0d_Istrat` / `zaco0d_strat` — fly away.
pub fn zaco0d_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, zaco0_flyaway);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    zaco0_flyaway(g, idx);
}

pub fn zaco0d_strat(g: &mut Game, idx: u16) {
    zaco0_flyaway(g, idx);
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
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.worldy = 0;
        al.rotz = 0;
        // Extended-bank ids are already resolved renderer ids; unlike the
        // ROM's 8-bit shape operand they must not be indexed through the
        // 256-entry raw shape-word table.
        al.shape = SH_PARA_1_PROXY;
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
            // ROM `s_obj2obj_3Dangle ...,0` (via s_initface_player) stores
            // nega(Yanglexy) into the aim latch. (Audit A Minor 15)
            let yaw = angle_xz(&me, &pl).wrapping_neg();
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SMFLAG1;
    } else {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        let yaw_aligned = achase_angle(&mut roty, me.sbyte3, 1);
        let mut rotx = me.rotx;
        let pitch_aligned = achase_angle(&mut rotx, me.sbyte4, 1);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        aligned = yaw_aligned && pitch_aligned;
    }
    if !aligned {
        // ROM `s_gen_vecs` → `nvecs_l` (does not zero vy; -roty+1 table index).
        // Old `gen_vecs_2d` (alvelvecs) zeroed vy and forced the first
        // add_vecs2pos to be xz-only — that was a workaround, not ASM.
        strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]);
    }
    // First `s_add_vecs2pos` — full xyz (D2STRATS.ASM:583); hop vy from the
    // previous frame carries through nvecs.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(al.vx);
        al.worldy = al.worldy.wrapping_add(al.vy);
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
    let (ox, oz);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags &= !HF1_MASK;
        al.stratptr = Some(s);
        ox = al.worldx;
        oz = al.worldz;
    }
    // ASM `jsl dooropensound_l` -> makesnd (positional, POS_DOOROPEN). (F1)
    g.hooks.make_snd(PosSndFamilyId::DoorOpen, ox, oz);
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
        let (ox, oz) = {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratptr = Some(s);
            (al.worldx, al.worldz)
        };
        // ASM `jsl doorclosesound_l` -> makesnd (positional, POS_DOORCLOSE). (F2)
        g.hooks.make_snd(PosSndFamilyId::DoorClose, ox, oz);
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
        // ASM cameleon_istrat (DSTRATS.ASM:1527) sets no colltype. (Audit A Minor 9)
    }
    g.hooks.play_se(0x2B);
}

// ============================================================
// CAMELEON2 / CAM2 (GASTRATS.ASM:1440-1495)
// ============================================================

/// ROM `cam2posX_tab` / `cam2posY_tab` (GASTRATS.ASM:1492) — 6 hide positions.
const CAM2_POS: [(i16, i16); 6] = [
    (-300, -60),
    (300, -60),
    (-250, 200),
    (0, -200),
    (250, 200),
    (-300, -60),
];

/// ROM `cameleon2_Istrat` (GASTRATS.ASM:1440).
pub fn cameleon2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, cameleon2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = CAMELEON_HP;
        al.ap = CAMELEON_AP;
        al.collflags |= COLLTYPE_ZENEMY;
        // sbyte1 indexes cam2pos tables as a byte offset into word pairs
        // (0,2,4,...,10) — we store the slot index 0..5 and scale on lookup.
        al.sbyte1 = 0;
    }
    cam2nextpos(g, idx);
}

/// ROM `cameleon2_strat` — flip to deg180, fire, then hide.
pub fn cameleon2_strat(g: &mut Game, idx: u16) {
    let done = achase_angle(&mut g.objs.aliens[idx as usize].rotx, DEG180, 2);
    if !done {
        cameleon2_cont(g, idx);
        return;
    }
    g.hooks.play_se(0x2b);
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let pitch = strat_pitch_toward(&me, &pl);
        let yaw = angle_xz(&me, &pl);
        strat_fire_relslowlaser(g, idx, pitch, yaw);
    }
    cam2hide_init(g, idx);
}

/// ROM `cameleon2_cont` — playerZ scroll only.
pub fn cameleon2_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

/// ROM `cam2hide_init` (GASTRATS.ASM:1460).
pub fn cam2hide_init(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // ROM: s_add_alvar al_sbyte1,#2 then EQ #12 → dash (byte offset into
        // word table). Slot index: +1, dash when slot==6.
        al.sbyte1 = al.sbyte1.wrapping_add(1);
        if al.sbyte1 >= 6 {
            cam2dash_init(g, idx);
            return;
        }
    }
    let tick = sid(g, cam2hide_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    cam2hide_strat(g, idx);
}

/// ROM `cam2hide_strat` — ease rotx to 0, then nextpos.
pub fn cam2hide_strat(g: &mut Game, idx: u16) {
    let done = achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 4);
    if done {
        cam2nextpos(g, idx);
        return;
    }
    cameleon2_cont(g, idx);
}

/// ROM `cam2nextpos` — teleport to next table slot, resume cameleon2_strat.
pub fn cam2nextpos(g: &mut Game, idx: u16) {
    let slot = (g.objs.aliens[idx as usize].sbyte1 as usize) % CAM2_POS.len();
    let (x, y) = CAM2_POS[slot];
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = x;
        al.worldy = y;
    }
    let tick = sid(g, cameleon2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    cameleon2_strat(g, idx);
}

/// ROM `cam2dash_init` / `cam2dash_strat` (GASTRATS.ASM:1474).
pub fn cam2dash_init(g: &mut Game, idx: u16) {
    let tick = sid(g, cam2dash_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    cam2dash_strat(g, idx);
}

pub fn cam2dash_strat(g: &mut Game, idx: u16) {
    let done = achase_angle(&mut g.objs.aliens[idx as usize].rotx, DEG180, 2);
    if !done {
        cameleon2_cont(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(4);
        let _ = speed_to(al, 30, 1);
    }
    // Aim via sbyte1/sbyte2 when far; always addvecs.
    if let Some(pl) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - pl.worldz as i32).abs();
        if dz >= 300 {
            let me = g.objs.aliens[idx as usize];
            // ROM `s_obj2obj_3dangle` into sbyte1/2 — Yanglexy+nega.
            let want_y = angle_xz(&me, &pl).wrapping_neg();
            let want_x = strat_pitch_toward(&me, &pl);
            {
                let al = &mut g.objs.aliens[idx as usize];
                achase_angle(&mut al.sbyte1, want_y, 2);
                achase_angle(&mut al.sbyte2, want_x, 2);
                // gen_3dvecs from sbyte1/sbyte2 aim, not body rots.
                let saved_ry = al.roty;
                let saved_rx = al.rotx;
                al.roty = al.sbyte1;
                al.rotx = al.sbyte2;
                gen_vecs_3d(al);
                al.roty = saved_ry;
                al.rotx = saved_rx;
            }
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    cameleon2_cont(g, idx);
}

// ============================================================
// CRAB (GASTRATS.ASM:1821-1946) — screen-edge walker that fires MISSILE2.
// ============================================================

const CRAB_HP: u8 = 4; // STRATEQU.INC:134
const CRAB_AP: u8 = 4; // STRATEQU.INC:135
const CRAB_WALK_SPEED: i16 = 8; // crabwalkspeed
const CRAB_WALK_F_SPEED: i16 = 8; // crabwalkFspeed

fn crab_min_pmove_x(g: &Game) -> i16 {
    g.vars.sv_i16(crate::common::sv::MINPMOVEX)
}
fn crab_max_pmove_x(g: &Game) -> i16 {
    g.vars.sv_i16(crate::common::sv::MAXPMOVEX)
}
fn crab_min_pmove_y(g: &Game) -> i16 {
    g.vars.minpmove_y
}
fn crab_max_pmove_y(g: &Game) -> i16 {
    g.vars.sv_i16(crate::common::sv::MAXPMOVEY)
}

/// Shared HP/AP/facing for all crab_*_Istrat entries.
pub fn crab_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = CRAB_HP;
    al.ap = CRAB_AP;
    al.roty = DEG180;
    al.sbyte2 = 10;
}

fn crab_wire(g: &mut Game, idx: u16, tick: StrategyFn, sbyte1: u8) {
    let s = sid(g, tick);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, smarkexplode_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte1 = sbyte1;
    }
    crab_init(g, idx);
}

/// ROM `crabB_Istrat` (GASTRATS.ASM:1824).
pub fn crabb_istrat(g: &mut Game, idx: u16) {
    crab_wire(g, idx, crabb_strat, 0);
}

/// ROM `crabL_Istrat` (GASTRATS.ASM:1829).
pub fn crabl_istrat(g: &mut Game, idx: u16) {
    crab_wire(g, idx, crabl_strat, DEG90);
}

/// ROM `crabT_Istrat` (GASTRATS.ASM:1835).
pub fn crabt_istrat(g: &mut Game, idx: u16) {
    crab_wire(g, idx, crabt_strat, DEG180);
}

/// ROM `crabR_Istrat` (GASTRATS.ASM:1841).
pub fn crabr_istrat(g: &mut Game, idx: u16) {
    crab_wire(g, idx, crabr_strat, DEG180.wrapping_add(DEG90));
}

/// ROM `crabB_init` / `crabB_strat` — walk left (−X); turn L at minX.
pub fn crabb_init(g: &mut Game, idx: u16) {
    let tick = sid(g, crabb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = 0;
    crabb_strat(g, idx);
}

pub fn crabb_strat(g: &mut Game, idx: u16) {
    // s_weapon_rot #-deg45,#0 — pitch offset for fire (stored in rotx scratch).
    g.objs.aliens[idx as usize].rotx = (0i8.wrapping_sub(DEG45 as i8)) as u8;
    let dx = -CRAB_WALK_SPEED;
    let dy = 0i16;
    if g.objs.aliens[idx as usize].worldx < crab_min_pmove_x(g) {
        crabl_init(g, idx);
        return;
    }
    crab_cont(g, idx, dx, dy);
}

/// ROM `crabL_init` / `crabL_strat` — walk up (−Y); turn T at minY.
pub fn crabl_init(g: &mut Game, idx: u16) {
    let tick = sid(g, crabl_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = DEG90;
    crabl_strat(g, idx);
}

pub fn crabl_strat(g: &mut Game, idx: u16) {
    let dx = 0i16;
    let dy = -CRAB_WALK_SPEED;
    if g.objs.aliens[idx as usize].worldy < crab_min_pmove_y(g) {
        crabt_init(g, idx);
        return;
    }
    crab_cont(g, idx, dx, dy);
}

/// ROM `crabT_init` / `crabT_strat` — walk right (+X); turn R at maxX.
pub fn crabt_init(g: &mut Game, idx: u16) {
    let tick = sid(g, crabt_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = DEG180;
    crabt_strat(g, idx);
}

pub fn crabt_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotx = DEG45;
    let dx = CRAB_WALK_SPEED;
    let dy = 0i16;
    if g.objs.aliens[idx as usize].worldx > crab_max_pmove_x(g) {
        crabr_init(g, idx);
        return;
    }
    crab_cont(g, idx, dx, dy);
}

/// ROM `crabR_init` / `crabR_strat` — walk down (+Y); turn B at maxY.
pub fn crabr_init(g: &mut Game, idx: u16) {
    let tick = sid(g, crabr_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = DEG180.wrapping_add(DEG90);
    crabr_strat(g, idx);
}

pub fn crabr_strat(g: &mut Game, idx: u16) {
    let dx = 0i16;
    let dy = CRAB_WALK_SPEED;
    if g.objs.aliens[idx as usize].worldy > crab_max_pmove_y(g) {
        crabb_init(g, idx);
        return;
    }
    crab_cont(g, idx, dx, dy);
}

/// ROM `crab_cont` (GASTRATS.ASM:1853) — scroll, edge-walk, fire MISSILE2.
pub fn crab_cont(g: &mut Game, idx: u16, dx: i16, dy: i16) {
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(CRAB_WALK_F_SPEED);
        if al.sflags2 & ASF2_SFLAG1 == 0 {
            al.worldx = al.worldx.wrapping_add(dx);
            al.worldy = al.worldy.wrapping_add(dy);
        }
        let target = al.sbyte1;
        achase_angle(&mut al.rotz, target, 1);
    }

    // Far (>4000z) → remove.
    if let Some(p) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs();
        if dz >= 4000 {
            g.objs.aldead = 1;
            return;
        }
        // Mid-range: rush forward extra.
        if dz >= 1500 {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_add(CRAB_WALK_F_SPEED.wrapping_mul(2));
            return;
        }
    }

    // .chkfire: only fire when |worldx| <= crabwalkspeed/2.
    let wx = g.objs.aliens[idx as usize].worldx;
    let half = CRAB_WALK_SPEED / 2;
    if wx < -half || wx > half {
        g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
        return;
    }

    // .dfire — latch sflag1 + reload timer on first entry.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags2 |= ASF2_SFLAG1;
        al.sbyte2 = 10;
    }

    // s_decbne_alvar al_sbyte2 — when hits 0 clear sflag1.
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 != 0 {
            al.sbyte2 -= 1;
        }
        if al.sbyte2 == 0 {
            al.sflags2 &= !ASF2_SFLAG1;
        }
    }
    // Fire when sbyte2 == 5.
    if g.objs.aliens[idx as usize].sbyte2 == 5 {
        if let Some(shot) = fire_missile2(g, idx) {
            let al = &mut g.objs.aliens[shot as usize];
            al.sflags2 |= ASF2_SFLAG1 | ASF2_SFLAG2;
        }
    }
}

// ============================================================
// BEE1 (GASTRATS.ASM:3177-3269) — circling Zenemy that faces then dives.
// Active path is the `ifeq 0` block (costab/sintab orbit).
// ============================================================

const BEE1_HP: u8 = 4; // STRATEQU.INC:104
const BEE1_AP: u8 = 6; // STRATEQU.INC:105
const BEE_SPACE_VIEWCY: i16 = -60; // STRATEQU.INC:494

/// ROM `s_set_alvar2alvartab ...,costab/sintab,#scale` — sign-extend table byte,
/// then `<< scale` (positive) or `/ 2^|scale|` toward zero (negative).
fn bee_tab_scaled(angle: u8, use_cos: bool, scale: i32) -> i16 {
    use crate::snes_trig::{COSTAB, SINTAB};
    let v = if use_cos {
        COSTAB[angle as usize] as i16
    } else {
        SINTAB[angle as usize] as i16
    };
    if scale > 0 {
        v << scale
    } else if scale < 0 {
        v / (1i16 << (-scale))
    } else {
        v
    }
}

fn achase_i16(cur: &mut i16, target: i16, shift: u32) -> bool {
    let mut d = target as i32 - *cur as i32;
    if d == 0 {
        return true;
    }
    let min = 1i32 << shift;
    if d > -min && d < min {
        d = if d < 0 { -min } else { min };
    }
    *cur = cur.wrapping_add((d >> shift) as i16);
    *cur == target
}

/// ROM `bee1_Istrat` (GASTRATS.ASM:3177).
pub fn bee1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bee1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = BEE1_HP;
        al.ap = BEE1_AP;
        al.collflags |= COLLTYPE_ZENEMY;
        al.vz = -40;
        al.roty = DEG180;
        al.sbyte1 = sf_random(&mut g.vars) as u8;
        al.snd2 = 3;
        al.sflags |= ASF_SHADOW;
    }
}

/// ROM `bee1_strat` — costab/sintab orbit; close → bee1a.
pub fn bee1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = bee_tab_scaled(al.sbyte1, true, 1);
        al.worldy = bee_tab_scaled(al.sbyte2, false, -1).wrapping_add(BEE_SPACE_VIEWCY);
        al.sbyte1 = al.sbyte1.wrapping_add(2);
        al.sbyte2 = al.sbyte2.wrapping_add(8);
    }
    if let Some(p) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs();
        if dz < 500 {
            bee1a_init(g, idx);
            return;
        }
        if dz < 2000 {
            let mut vz = g.objs.aliens[idx as usize].vz;
            achase_i16(&mut vz, 0, 1);
            g.objs.aliens[idx as usize].vz = vz;
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `bee1a_init` / `bee1a_strat` — face player, then dive.
pub fn bee1a_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bee1a_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags2 &= !ASF2_SMFLAG1; // s_initface_player
    }
    bee1a_strat(g, idx);
}

pub fn bee1a_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    // s_copy_alvar2alvar al_sbyte1,al_rotx — stash pitch for bee1b gen_3dvecs.
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].rotx;
    // s_face_player x,2,5,bee1b_init
    if bee_face_player(g, idx, 2, 5) {
        bee1b_init(g, idx);
    }
}

/// `s_face_player` (STRATMAC.INC:2020) — latch aim into sbyte3/4; chase; return
/// true when both axes reached (optional label).
fn bee_face_player(g: &mut Game, idx: u16, chase: u32, delay_bits: u16) -> bool {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1 != 0 {
        let me = g.objs.aliens[idx as usize];
        let mut roty = me.roty;
        let yaw_aligned = achase_angle(&mut roty, me.sbyte3, chase);
        let mut rotx = me.rotx;
        let pitch_aligned = achase_angle(&mut rotx, me.sbyte4, chase);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = roty;
            al.rotx = rotx;
        }
        if yaw_aligned && pitch_aligned {
            return true;
        }
    }
    // Re-latch on notdelay gate (or first entry when smflag1 clear).
    let gate = g.vars.gameframe & ((1u16 << delay_bits) - 1) == 0;
    if gate || g.objs.aliens[idx as usize].sflags2 & ASF2_SMFLAG1 == 0 {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SMFLAG1;
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            // ROM `s_face_player` latch: `s_obj2obj_3Dangle` → sbyte3/4 (nega).
            let yaw = angle_xz(&me, &pl).wrapping_neg();
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
    }
    false
}

/// ROM `bee1b_init` / `bee1b_strat` — accelerate dive using stashed pitch.
pub fn bee1b_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bee1b_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    bee1b_strat(g, idx);
}

pub fn bee1b_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 30, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_gen_3dvecs x,al_roty,al_sbyte1,al_vel — pitch from sbyte1 stash.
        let saved = al.rotx;
        al.rotx = al.sbyte1;
        gen_vecs_3d(al);
        al.rotx = saved;
        al.type_ |= ATZREMOVE; // s_setremove_behind
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(1);
}

// ============================================================
// DRAGONFLY (GA2STRAT.ASM:1418-1463) — 3-state fly-by (child spawn scoped).
// ============================================================

const DRAGONFLY_HP: u8 = 2; // STRATEQU.INC:245
const DRAGONFLY_AP: u8 = 8; // STRATEQU.INC:246

/// ROM `dragonfly_Istrat` (GA2STRAT.ASM:1418).
pub fn dragonfly_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, dragonfly_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = DRAGONFLY_HP;
        al.ap = DRAGONFLY_AP;
        al.vel = 70;
        al.roty = (0i8.wrapping_sub(DEG90 as i8)) as u8; // -deg90
        al.count = 100; // lifecnt
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sbyte1 = 35;
        al.stratstate = 0;
    }
}

/// ROM `dragonfly_strat` — down → up → away; spawn Sdragonfly children on turns.
pub fn dragonfly_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                achase_angle(&mut al.rotx, DEG11, 2);
                achase_angle(&mut al.roty, (0i8.wrapping_sub(DEG90 as i8)) as u8, 2);
            }
            let expired = {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sbyte1 != 0 {
                    al.sbyte1 -= 1;
                }
                al.sbyte1 == 0
            };
            if expired {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte1 = 20;
                    al.stratstate = 1;
                }
                let _ = make_sdrag(g, idx);
            }
        }
        1 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                achase_angle(&mut al.rotx, 0, 2);
                achase_angle(&mut al.roty, DEG90, 2);
            }
            let expired = {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sbyte1 != 0 {
                    al.sbyte1 -= 1;
                }
                al.sbyte1 == 0
            };
            if expired {
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte1 = 20;
                    al.stratstate = 2;
                }
                let _ = make_sdrag(g, idx);
            }
        }
        _ => {
            let al = &mut g.objs.aliens[idx as usize];
            achase_angle(&mut al.rotx, (0i8.wrapping_sub(DEG22 as i8)) as u8, 2);
            achase_angle(&mut al.roty, (0i8.wrapping_sub(DEG45 as i8)) as u8, 2);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_dec_lifecnt — dies when the pre-decrement count was 0 (wraps).
        let c = al.count;
        al.count = c.wrapping_sub(1);
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
        gen_vecs_3d(al);
        apply_velocity(al);
        al.vz = al.vz.wrapping_mul(2); // s_scale_alvar W,x,al_vz,1 → <<1
    }
    add_player_z(g, idx);
}

/// ROM `makeSdrag_srou` (GA2STRAT.ASM:1465) — spawn small dragonfly child.
const SH_F_DRA_1_PROXY: u16 = 356;

/// ROM `makeSdrag_srou` (GA2STRAT.ASM:1465) — spawn small dragonfly child.
pub fn make_sdrag(g: &mut Game, mother: u16) -> Option<u16> {
    let child = make_obj(g, SH_F_DRA_1_PROXY)?;
    copy_pos(g, child, mother);
    sdragonfly_istrat(g, child);
    Some(child)
}

const SDRAGONFLY_HP: u8 = 2; // STRATEQU.INC:247
const SDRAGONFLY_AP: u8 = 4; // STRATEQU.INC:248

/// ROM `Sdragonfly_Istrat` (GA2STRAT.ASM:1472).
pub fn sdragonfly_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, sdragonfly_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SDRAGONFLY_HP;
        al.ap = SDRAGONFLY_AP;
        al.vel = 25;
        al.sbyte1 = 6;
        al.stratstate = 0;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
    }
}

/// Chase `field` toward `target + offset` (ROM `s_obj2obj_3DangleOFF` one axis).
fn achase_angle_off(field: &mut u8, target: u8, offset: u8, shift: u32) -> bool {
    let mut base = (*field).wrapping_sub(offset);
    let reached = achase_angle(&mut base, target, shift);
    *field = base.wrapping_add(offset);
    reached
}

/// ROM `Sdragonfly_strat` (GA2STRAT.ASM:1480) — weave then stop-close.
pub fn sdragonfly_strat(g: &mut Game, idx: u16) {
    let dz = player(g).map_or(i32::MAX, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs()
    });

    let mut yaw_off: u8 = 0;
    let state = g.objs.aliens[idx as usize].stratstate;

    if state == 2 {
        // Hover timer: skip aim/gen until expired, then addvecs every tick.
        let expired = {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte1 != 0 {
                al.sbyte1 -= 1;
            }
            al.sbyte1 == 0
        };
        if expired {
            g.objs.aliens[idx as usize].sbyte1 = 1;
            apply_velocity(&mut g.objs.aliens[idx as usize]);
        }
        add_player_z(g, idx);
        return;
    }

    if dz >= 500 {
        match state {
            0 => {
                yaw_off = (0i8.wrapping_sub(DEG45 as i8)) as u8; // -deg45
                let expired = {
                    let al = &mut g.objs.aliens[idx as usize];
                    if al.sbyte1 != 0 {
                        al.sbyte1 -= 1;
                    }
                    al.sbyte1 == 0
                };
                if expired {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte1 = 12;
                    al.stratstate = 1;
                }
            }
            1 => {
                yaw_off = DEG45;
                let expired = {
                    let al = &mut g.objs.aliens[idx as usize];
                    if al.sbyte1 != 0 {
                        al.sbyte1 -= 1;
                    }
                    al.sbyte1 == 0
                };
                if expired {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sbyte1 = 12;
                    al.stratstate = 0;
                }
            }
            _ => {}
        }
    }

    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 1);
        if dz < 400 {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratstate = 2;
            al.sbyte1 = 20;
        }
        let me = g.objs.aliens[idx as usize];
        // ROM `s_obj2obj_3DangleOFF` into sbyte2/3 — Yanglexy+nega (+ yaw_off).
        let yaw_t = angle_xz(&me, &p).wrapping_neg();
        let pitch_t = strat_pitch_toward(&me, &p);
        {
            let al = &mut g.objs.aliens[idx as usize];
            achase_angle_off(&mut al.sbyte2, yaw_t, yaw_off, 1);
            achase_angle_off(&mut al.sbyte3, pitch_t, 0, 1);
            // gen_3dvecs from sbyte2/sbyte3: swap into roty/rotx, gen, restore.
            let (save_y, save_x) = (al.roty, al.rotx);
            al.roty = al.sbyte2;
            al.rotx = al.sbyte3;
            gen_vecs_3d(al);
            al.roty = save_y;
            al.rotx = save_x;
            apply_velocity(al);
        }
    }
    add_player_z(g, idx);
}

// ============================================================
// ZACO7 (GA2STRAT.ASM:959-985) — bank-then-aim flyer (Szaco5 HP/AP).
// ============================================================

const ZACO7_HP: u8 = 2; // Szaco5HP
const ZACO7_AP: u8 = 8; // Szaco5AP

/// ROM `zaco7_Istrat` (GA2STRAT.ASM:959).
pub fn zaco7_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, zaco7_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = ZACO7_HP;
        al.ap = ZACO7_AP;
    }
}

/// ROM `zaco7_strat` — anim open; bank when far, 3d-aim when close.
pub fn zaco7_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe != 9 {
            al.animframe = (al.animframe + 1) % 10;
        }
    }
    let dz = player(g).map_or(i32::MAX, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs()
    });
    if dz >= 600 {
        szaco2_bank_to_player(g, idx);
    } else if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 3);
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

// ============================================================
// EXITLIGHT (GASTRATS.ASM:3728-3775) — staggered colanim blink cycle.
// ============================================================

/// ROM `exitlight1_Istrat` … `exitlight6_Istrat` — preload sbyte2 delay then init.
pub fn exitlight1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 2;
    exitlight_init(g, idx);
}
pub fn exitlight2_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 4;
    exitlight_init(g, idx);
}
pub fn exitlight3_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 6;
    exitlight_init(g, idx);
}
pub fn exitlight4_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 8;
    exitlight_init(g, idx);
}
pub fn exitlight5_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 10;
    exitlight_init(g, idx);
}
pub fn exitlight6_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 12;
    exitlight_init(g, idx);
}

/// ROM `exitlight_init` — colldisable then fall into A phase.
pub fn exitlight_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    exitlight_a_init(g, idx);
}

/// ROM `exitlightA_init` / `exitlightA_strat`.
pub fn exitlight_a_init(g: &mut Game, idx: u16) {
    let tick = sid(g, exitlight_a_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    exitlight_a_strat(g, idx);
}

pub fn exitlight_a_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar sbyte2 → exitlightB_init (TEST-then-DEC).
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        exitlight_b_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    crate::common::init_colanim(&mut g.objs.aliens[idx as usize], 1);
}

/// ROM `exitlightB_init` / `exitlightB_strat`.
pub fn exitlight_b_init(g: &mut Game, idx: u16) {
    let tick = sid(g, exitlight_b_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 3;
        crate::common::init_colanim(al, 0);
    }
}

pub fn exitlight_b_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar sbyte1,.dwait
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.objs.aliens[idx as usize].sbyte2 = 12;
        exitlight_a_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
}

// ============================================================
// FASTFIGHTER 1/2/3 + dofighter (GASTRATS.ASM:3395-3523).
// ============================================================

const FIGHTER_HP: u8 = 4; // fighterHP = 2+2
const FIGHTER_AP: u8 = 4;
const FF_FIRE_RATE: u8 = 3; // fffirerate

/// Fire SLOWELASER with absolute pitch/yaw (copies into firer then restores).
fn fire_slow_elaser_aimed(g: &mut Game, firer: u16, pitch: u8, yaw: u8) {
    let (save_x, save_y) = {
        let al = &g.objs.aliens[firer as usize];
        (al.rotx, al.roty)
    };
    {
        let al = &mut g.objs.aliens[firer as usize];
        al.rotx = pitch;
        al.roty = yaw;
    }
    let _ = fire_slow_elaser(g, firer);
    {
        let al = &mut g.objs.aliens[firer as usize];
        al.rotx = save_x;
        al.roty = save_y;
    }
}

/// `s_weapon_rndrot #px,#py` — pitch (rnd&px)-(px/2), yaw (rnd&py)-(py/2).
fn weapon_rndrot(g: &mut Game, pitch_mask: u8, yaw_mask: u8) -> (u8, u8) {
    let dp = ((sf_random(&mut g.vars) as u8) & pitch_mask).wrapping_sub(pitch_mask / 2);
    let dy = ((sf_random(&mut g.vars) as u8) & yaw_mask).wrapping_sub(yaw_mask / 2);
    (dp, dy)
}

/// ROM `fastfighter_init` (GASTRATS.ASM:3473).
pub fn fastfighter_init(g: &mut Game, idx: u16) {
    let _ = sf_random(&mut g.vars); // s_set_var2rnd svar_byte1 (discarded)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = FIGHTER_HP;
        al.ap = FIGHTER_AP;
        al.roty = DEG180;
        al.vel = 80;
        al.sflags |= ASF_COLLDISABLE;
        al.snd2 = 2;
        gen_vecs_3d(al);
    }
}

const FIGHTER_MUZZLE_Z: i16 = 30;
const FIGHTER_FIRE_PERIOD_MASK: u8 = 31;

/// ROM `fighter_Istrat` (GASTRATS.ASM:3486-3495). Unlike the fast-fighter
/// variants this enemy begins stationary, is an enemy-weapon collision
/// target, and uses a random per-object firing phase.
pub fn fighter_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fighter_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let firing_phase = sf_random(&mut g.vars) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = FIGHTER_HP;
    al.ap = FIGHTER_AP;
    al.roty = DEG180;
    al.vel = 0;
    al.sbyte1 = firing_phase;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    gen_vecs_3d(al);
}

/// ROM `fighter_strat` (GASTRATS.ASM:3497-3505).
pub fn fighter_strat(g: &mut Game, idx: u16) {
    let firing_phase = g.objs.aliens[idx as usize].sbyte1;
    if (g.vars.gameframe as u8).wrapping_add(firing_phase) & FIGHTER_FIRE_PERIOD_MASK == 0 {
        if let Some(shot) = fire_slow_elaser(g, idx) {
            place_weapon_at_firer(g, shot, idx, FIGHTER_MUZZLE_Z);
        }
    }
    dofighter(g, idx);
}

/// ROM `fastfighter1_Istrat`.
pub fn fastfighter1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fastfighter1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte1 = 8; // shoot 4 times at target (beqdec budget)
    }
    fastfighter_init(g, idx);
}

/// ROM `fastfighter1_strat` — spray or aimed burst at `al_ptr`.
pub fn fastfighter1_strat(g: &mut Game, idx: u16) {
    let target = {
        let p = g.objs.aliens[idx as usize].ptr;
        if p != 0 && (p as usize) < g.objs.aliens.len() && g.objs.aliens[p as usize].active {
            Some(p)
        } else {
            None
        }
    };
    let close = target.map_or(false, |t| {
        (g.objs.aliens[idx as usize].worldz as i32 - g.objs.aliens[t as usize].worldz as i32).abs()
            < 1500
    });
    if !close {
        // Spray: notdelay 3,al1pt → (gf+idx)&7==0.
        if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 7 == 0 {
            let (dp, dy) = weapon_rndrot(g, 7, 15);
            let me = g.objs.aliens[idx as usize];
            fire_slow_elaser_aimed(g, idx, me.rotx.wrapping_add(dp), me.roty.wrapping_add(dy));
        }
    } else if let Some(t) = target {
        // Aimed: notdelay 2; skip if target in front of self; beqdec sbyte1.
        if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 3 == 0 {
            let tgt_in_front =
                g.objs.aliens[t as usize].worldz >= g.objs.aliens[idx as usize].worldz;
            if !tgt_in_front && g.objs.aliens[idx as usize].sbyte1 != 0 {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
                let me = g.objs.aliens[idx as usize];
                let target = g.objs.aliens[t as usize];
                let pitch = strat_pitch_toward(&me, &target);
                let yaw = angle_xz(&me, &target);
                strat_fire_relslowlaser(g, idx, pitch, yaw);
            }
        }
    }
    dofighter(g, idx);
}

/// ROM `fastfighter3_Istrat` / `fastfighter3_strat` — timed spray then → fighter1.
pub fn fastfighter3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fastfighter3_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte1 = FF_FIRE_RATE;
    }
    fastfighter_init(g, idx);
}

pub fn fastfighter3_strat(g: &mut Game, idx: u16) {
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != 0 {
            al.sbyte1 -= 1;
        }
        al.sbyte1 == 0
    };
    if expired {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = FF_FIRE_RATE;
        }
        let (dp, dy) = weapon_rndrot(g, 7, 15);
        let me = g.objs.aliens[idx as usize];
        fire_slow_elaser_aimed(g, idx, me.rotx.wrapping_add(dp), me.roty.wrapping_add(dy));
        let tick = sid(g, fastfighter1_strat);
        g.objs.aliens[idx as usize].stratptr = Some(tick);
    }
    dofighter(g, idx);
}

/// ROM `fastfighter2_Istrat` / `fastfighter2_strat` — yaw-only spray.
pub fn fastfighter2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fastfighter2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
    }
    fastfighter_init(g, idx);
}

pub fn fastfighter2_strat(g: &mut Game, idx: u16) {
    // s_jmp_NOTdelay 3,.nofire,al1pt
    if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & 7 == 0 {
        let dy = ((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3);
        let me = g.objs.aliens[idx as usize];
        fire_slow_elaser_aimed(g, idx, me.rotx, me.roty.wrapping_add(dy));
    }
    dofighter(g, idx);
}

/// ROM `dofighter` (GASTRATS.ASM:3507) — bank when close; damaged spin; addvecs.
fn dofighter(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if dist_xz(&me, &p) < 1000 {
            szaco2_bank_to_player(g, idx);
        }
    }
    // Damaged: HP <= fighterHP-1 → alternate rotz spin via next_state #2.
    if g.objs.aliens[idx as usize].hp <= FIGHTER_HP.saturating_sub(1) {
        if g.vars.gameframe & 1 == 0 {
            // s_next_state x,#2 — wrap to 1 when past max.
            let s = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
            g.objs.aliens[idx as usize].stratstate = if s <= 2 { s } else { 1 };
        }
        if g.objs.aliens[idx as usize].stratstate == 1 {
            g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_sub(5);
        } else {
            g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(5);
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

// ============================================================
// CRANE0 + TZACO7 (GA2STRAT.ASM:693-812) — hardHP crane carrying a zaco_7.
// ============================================================

const SH_ZACO_7: u16 = 128; // shape_data / route2 rc.rs
const CRANE_CHILD_HP: u8 = 6;
const CRANE_CHILD_AP: u8 = 8;
const HF1: u8 = 0x01;
const HF2: u8 = 0x02;

fn copy_rots(g: &mut Game, dst: u16, src: u16) {
    let s = g.objs.aliens[src as usize];
    let al = &mut g.objs.aliens[dst as usize];
    al.rotx = s.rotx;
    al.roty = s.roty;
    al.rotz = s.rotz;
}

/// ROM `crane0_Istrat` (GA2STRAT.ASM:693) — spawn carried zaco_7 child.
pub fn crane0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, crane0_strat);
    let coll = sid(g, crane0col_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = None;
        al.hp = HARD_HP;
        al.ap = 2;
        al.sbyte1 = 6;
        al.stratstate = 0;
        al.ptr = 0;
        al.sflags |= ASF_SHADOW;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    if let Some(child) = make_obj(g, SH_ZACO_7) {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.ptr = child;
        }
        let c_coll = sid(g, strat_hit_flash);
        let c_exp = sid(g, strat_explode);
        {
            let al = &mut g.objs.aliens[child as usize];
            al.sflags |= ASF_COLLDISABLE | ASF_SHADOW;
            al.stratptr = None;
            al.collstratptr = Some(c_coll);
            al.expstratptr = Some(c_exp);
            al.hp = CRANE_CHILD_HP;
            al.ap = CRANE_CHILD_AP;
            al.animframe = 0;
            al.collflags |= COLLTYPE_ENEMY1;
        }
        copy_pos(g, child, idx);
        copy_rots(g, child, idx);
    }
    crane0_strat(g, idx);
}

/// ROM `crane0_strat` — 3-state slide; release child when close.
pub fn crane0_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            let close = player(g).map_or(false, |p| {
                (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() < 1500
            });
            if close {
                g.objs.aliens[idx as usize].roty = DEG90;
                g.objs.aliens[idx as usize].worldx =
                    g.objs.aliens[idx as usize].worldx.wrapping_sub(10);
                let expired = {
                    let al = &mut g.objs.aliens[idx as usize];
                    if al.sbyte1 != 0 {
                        al.sbyte1 -= 1;
                    }
                    al.sbyte1 == 0
                };
                if expired {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.stratstate = 1;
                    // 6000/(medpspeed-10); MEDPSPEED_I16=65 → 109.
                    let den = (MEDPSPEED_I16 as i32 - 10).max(1);
                    al.sbyte1 = (6000 / den).min(255) as u8;
                }
            }
        }
        1 => {
            achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 2);
            add_player_z(g, idx);
            g.objs.aliens[idx as usize].worldz =
                g.objs.aliens[idx as usize].worldz.wrapping_sub(10);
            let expired = {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sbyte1 != 0 {
                    al.sbyte1 -= 1;
                }
                al.sbyte1 == 0
            };
            if expired {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte1 = 6;
                al.stratstate = 2;
            }
        }
        2 => {
            achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG90, 2);
            g.objs.aliens[idx as usize].worldx =
                g.objs.aliens[idx as usize].worldx.wrapping_sub(10);
            let expired = {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sbyte1 != 0 {
                    al.sbyte1 -= 1;
                }
                al.sbyte1 == 0
            };
            if expired {
                g.objs.aliens[idx as usize].stratstate = 3;
            }
        }
        _ => {}
    }

    // Release child into tzaco7go when |dz| < 700.
    if let Some(p) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs();
        let child = g.objs.aliens[idx as usize].ptr;
        if dz < 700 && child != 0 {
            tzaco7go_istrat(g, child);
            g.objs.aliens[idx as usize].ptr = 0;
        }
    }

    // Carry: copy pose onto child while attached.
    let child = g.objs.aliens[idx as usize].ptr;
    if child != 0 && (child as usize) < g.objs.aliens.len() && g.objs.aliens[child as usize].active
    {
        copy_pos(g, child, idx);
        copy_rots(g, child, idx);
    }
}

/// ROM `crane0col_Istrat` (GA2STRAT.ASM:759) — friend laser: HF1 kill / HF2 drop.
pub fn crane0col_istrat(g: &mut Game, idx: u16) {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    let ok = partner != 0 && (partner as usize) < g.objs.aliens.len() && {
        let y = &g.objs.aliens[partner as usize];
        y.collflags & ACF_COLLTYPE1 != 0 && y.collflags & ACF_COLLTYPE5 != 0
    };
    if ok {
        let child = g.objs.aliens[idx as usize].ptr;
        let hf = g.objs.aliens[idx as usize].hitflags;
        if child != 0 && (child as usize) < g.objs.aliens.len() {
            if hf & HF1 != 0 {
                crate::common::kill_obj(&mut g.objs.aliens[child as usize]);
            } else if hf & HF2 != 0 {
                tzaco7fall_istrat(g, child);
                g.objs.aliens[idx as usize].ptr = 0;
            }
        }
    }
    g.objs.aliens[idx as usize].hitflags = 0;
    jmpto_strat(g, idx);
}

/// ROM `tzaco7fall_Istrat` (GA2STRAT.ASM:781) — drop; explode at ground.
pub fn tzaco7fall_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tzaco7fall_istrat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    // s_jmp_lower x,#0,explode — branch when worldy >= 0.
    if g.objs.aliens[idx as usize].worldy >= 0 {
        strat_explode(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(5);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// ROM `tzaco7go_Istrat` / `tzaco7go_strat` (GA2STRAT.ASM:789).
pub fn tzaco7go_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tzaco7go_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vy = 5;
        al.vz = -20;
        al.sflags &= !ASF_COLLDISABLE;
    }
    tzaco7go_strat(g, idx);
}

pub fn tzaco7go_strat(g: &mut Game, idx: u16) {
    let player_y = player(g).map(|p| p.worldy);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe != 9 {
            al.animframe = (al.animframe + 1) % 10;
        }
        if al.vy != 0 {
            al.vy = al.vy.wrapping_sub(1);
        } else if let Some(py) = player_y {
            al.worldy = chase_proportional(al.worldy, py, 3);
        }
    }
    // ASM: s_add_playerZ then s_add_vecs2pos.
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `tzaco7cat_Istrat` / `tzaco7cat_strat` (GA2STRAT.ASM:857) — parked zaco_7.
pub fn tzaco7cat_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tzaco7cat_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.animframe = 0;
        al.hp = 2;
        al.ap = 4;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    tzaco7cat_strat(g, idx);
}

pub fn tzaco7cat_strat(g: &mut Game, idx: u16) {
    let close = player(g).map_or(false, |p| {
        (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() < 500
    });
    if close {
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe != 9 {
            al.animframe = (al.animframe + 1) % 10;
        }
    }
}

// ============================================================
// SZACO2 (GA2STRAT.ASM; C strat_enemy.c:5958-6126)
// ============================================================

/// C `szaco2_waypoint_yaw` (strat_enemy.c:5958): `nega(anglexy_abs)`.
fn szaco2_waypoint_yaw(al: &Alien) -> u8 {
    sf_core::aim_angle::yanglexy_nega(
        al.swpx1.wrapping_sub(al.worldx),
        al.swpz1.wrapping_sub(al.worldz),
    )
}

/// C `szaco2_waypoint_pitch` — ROM `Xanglexabs_l` (Manhattan adjacent).
fn szaco2_waypoint_pitch(al: &Alien) -> u8 {
    sf_core::aim_angle::xanglexabs(
        al.swpy1.wrapping_sub(al.worldy),
        al.swpx1.wrapping_sub(al.worldx),
        al.swpz1.wrapping_sub(al.worldz),
    )
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
            if (g.vars.gameframe as u8).wrapping_add(strat_phase_offset(idx)) & SZACO2_FIRE_MASK
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
    al.swpy1 = al.swpy1.wrapping_add(SZACO2_WPY_OFFSET).wrapping_add(posy);
    al.animframe = 0x80 | SZACO2_ANIM_INIT;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.snd2 = 3;
    // ASM GA2STRAT.ASM:236-238 `s_set_debrisdata #zaco_8p` + `relexplode`.
    // Extended-bank mesh `SHAPE_EXT_ZACO_8P` (283) from SHAPES2.ASM.
    al.debrisshape = SH_ZACO_8P;
    al.sflags2 |= ASF2_RELEXPLODE;
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
        let dx = al.swpx1.wrapping_sub(al.worldx);
        let dz = al.swpz1.wrapping_sub(al.worldz);
        let target_yaw = sf_core::aim_angle::yanglexy_nega(dx, dz);
        let mut roty = al.roty;
        achase_angle(&mut roty, target_yaw, 3);
        al.roty = roty;
        let target_pitch = sf_core::aim_angle::xanglexabs(0i16.wrapping_sub(al.worldy), dx, dz);
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, target_pitch, 3);
        al.rotx = rotx;
    }
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs();
        // ASM GASTRATS.ASM:1215 `s_jmp_Zdistmore #1000,zaco1a_init` → phase1
        // when |dz| >= 1000 (inclusive). (Audit A Minor 3)
        // zaco1a_init falls into zaco1a_strat same frame. (Audit A Minor 15)
        if zdist >= 1000 {
            let s = sid(g, zaco1_phase1);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratptr = Some(s);
                al.type_ |= ATZREMOVE; // s_setremove_behind
            }
            zaco1_phase1(g, idx);
            return;
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
        if !reached {
            al.rotx = al.rotx.wrapping_sub(1);
        }
        reached
    };
    // ASM Achase … zaco1b_init falls into zaco1b_strat same frame. (Audit A #15)
    if reached {
        let s = sid(g, zaco1_phase2);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        zaco1_phase2(g, idx);
        return;
    }
    zaco1_cont(g, idx);
}

/// C `zaco1_phase2` (zaco1b_strat, GASTRATS.ASM:1238-1276).
fn zaco1_phase2(g: &mut Game, idx: u16) {
    let pl = player(g);
    let mut aim_target = pl;
    let zdist: i32 = match pl {
        Some(p) => (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs(),
        None => 0,
    };
    if zdist < 1400 {
        // .circ — spiral attack (GASTRATS.ASM:1253-1259): sintab,-3 / costab,-2
        // toward-zero (not float sin/cos + arithmetic >>). (Audit A Minor 15)
        let al = &mut g.objs.aliens[idx as usize];
        let _ = speed_to(al, 45, 1);
        al.sbyte1 = al.rotz;
        al.sbyte1 = al.sbyte1.wrapping_sub(DEG90);
        al.sword2 = bee_tab_scaled(al.sbyte1, false, -3);
        al.ptr = bee_tab_scaled(al.sbyte1, true, -2) as u16;
        al.rotz = al.rotz.wrapping_add(al.sbyte2);
    } else if (1400..1800).contains(&zdist) {
        // Fire RELSLOWELASERHOME every 4th frame (s_jmp_notdelay 2 = mask 3),
        // aimed at the player in full 3D (s_weapon_rots2obj y). Band is
        // 1400<=|dz|<1800 — the ASM `s_jmp_Zdistmore x,y,#1800` excludes 1800.
        // (Audit A Minor 4). The mid/far bands do NOT zero sword2/ptr — only
        // `.circ` writes them, and leaving `.circ` keeps the last spiral offsets
        // (zaco1_cont keeps adding them). (Audit A #30)
        if g.vars.gameframe & 3 == 0 {
            if pl.is_some() {
                // `s_fire_weapon` leaves the new bolt selected by the source
                // routine. The subsequent .nocirc aim in this same frame
                // therefore follows that bolt, not the player.
                if let Some(shot) =
                    fire_relslowlaserhome_at_target(g, idx, PLAYER_OBJECT_SLOT, 0, 0)
                {
                    aim_target = Some(g.objs.aliens[shot as usize]);
                }
            }
        }
    }
    // The second authored distance gate observes the currently selected
    // target. On a firing frame that is the newly created bolt, which is
    // close enough to suppress the remainder of this frame's aim update.
    let aim_target_depth_distance = match aim_target {
        Some(target) => (g.objs.aliens[idx as usize].worldz as i32 - target.worldz as i32).abs(),
        None => 0,
    };
    if aim_target_depth_distance >= 700 {
        if let Some(p) = aim_target {
            let me = g.objs.aliens[idx as usize];
            // ROM `s_obj2obj_3dangle ...,3` negates Yanglexy into al_roty
            // (same as zaco1_phase0 WP chase / szaco2_waypoint_yaw).
            // (Audit A Minor 15)
            let target_yaw = angle_xz(&me, &p).wrapping_neg();
            let mut roty = me.roty;
            achase_angle(&mut roty, target_yaw, 3);
            g.objs.aliens[idx as usize].roty = roty;
            let target_pitch = strat_pitch_toward(&me, &p);
            let mut rotx = g.objs.aliens[idx as usize].rotx;
            achase_angle(&mut rotx, target_pitch, 3);
            g.objs.aliens[idx as usize].rotx = rotx;
        }
    }
    zaco1_cont(g, idx);
}

#[cfg(test)]
mod zaco1_transition_tests {
    use super::*;

    fn game_at_reached_yaw() -> (Game, u16) {
        let mut game = Game::new();
        let player = game.objs.alloc().expect("player");
        let fighter = game.objs.alloc().expect("fighter");
        game.vars.internal_playpt = player as i16;
        game.vars.pviewvelz = 63;
        game.objs.aliens[player as usize].worldz = 9_766;
        let object = &mut game.objs.aliens[fighter as usize];
        object.worldx = 573;
        object.worldy = -399;
        object.worldz = 9_766;
        object.rotx = 238;
        object.roty = 0;
        object.vel = 60;
        object.sbyte2 = (-6i8) as u8;
        (game, fighter)
    }

    #[test]
    fn reached_yaw_branches_before_the_phase_one_pitch_step() {
        let (mut transitioned, fighter) = game_at_reached_yaw();
        let (mut direct, direct_fighter) = game_at_reached_yaw();
        let phase_two = sid(&mut direct, zaco1_phase2);
        direct.objs.aliens[direct_fighter as usize].stratptr = Some(phase_two);

        zaco1_phase1(&mut transitioned, fighter);
        zaco1_phase2(&mut direct, direct_fighter);

        assert_eq!(
            transitioned.objs.aliens[fighter as usize],
            direct.objs.aliens[direct_fighter as usize]
        );
    }
}

#[cfg(test)]
mod laser_flash_tests {
    use super::*;

    const FAR_PLAYER_DEPTH: i16 = 0;
    const FAR_FIRER_DEPTH: i16 = 1_600;
    const TEST_PITCH: u8 = 8;
    const TEST_YAW: u8 = 4;
    const BOLT_SLOT: usize = 2;
    const FLASH_SLOT: usize = 3;

    #[test]
    fn homing_enemy_laser_keeps_its_shape_and_spawns_the_authored_flash() {
        let mut game = Game::new();
        let player = game.objs.alloc().expect("player");
        let firer = game.objs.alloc().expect("firer");
        game.vars.internal_playpt = player as i16;
        game.objs.aliens[player as usize].worldz = FAR_PLAYER_DEPTH;
        game.objs.aliens[firer as usize].worldz = FAR_FIRER_DEPTH;

        let _ = strat_fire_relslowlaserhome(&mut game, firer, TEST_PITCH, TEST_YAW);

        assert_eq!(game.objs.aliens[BOLT_SLOT].shape, SHAPE_ENEMY_LASER);
        assert_eq!(game.objs.aliens[FLASH_SLOT].shape, SHAPE_LARGE_LASER_FLASH);
        assert_eq!(
            game.objs.active_indices(),
            vec![firer, BOLT_SLOT as u16, FLASH_SLOT as u16, player]
        );
        assert_eq!(
            (
                game.objs.aliens[FLASH_SLOT].worldx,
                game.objs.aliens[FLASH_SLOT].worldy,
                game.objs.aliens[FLASH_SLOT].worldz,
            ),
            (
                game.objs.aliens[BOLT_SLOT].worldx,
                game.objs.aliens[BOLT_SLOT].worldy,
                game.objs.aliens[BOLT_SLOT].worldz,
            )
        );
        assert_eq!(game.objs.aliens[FLASH_SLOT].roty, DEG180);
        assert!(game.objs.aliens[FLASH_SLOT].stratptr.is_some());
    }
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
        al.sflags4 &= !ASF4_INVISIBLE;
        al.count = al.count.wrapping_sub(1);
        if al.count == 0 {
            g.objs.aldead = 1;
        }
    }
}

/// C `Strat_FriendExitBase_Init` (GISTRATS.ASM:314-320).
pub fn strat_friendexitbase_init(g: &mut Game, idx: u16) {
    let s = sid(g, friendexitbase_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= ASF2_COLLDISABLE;
    al.count = (1500 / PEXITBASE_SPEED) as u8;
    al.stratptr = Some(s);
    al.sflags |= ASF_SHADOW;
    al.sflags4 |= ASF4_INVISIBLE;
    al.sbyte2 = 11;
    // The initializer is immediately followed by friendexitbase_strat in the
    // source and therefore performs its first countdown/movement pass now.
    friendexitbase_strat(g, idx);
}

#[cfg(test)]
mod friend_exit_base_tests {
    use super::*;

    #[test]
    fn initializer_falls_through_to_first_movement_pass() {
        let mut game = Game::new();
        let object = game.objs.alloc().expect("friend object");
        game.objs.aliens[object as usize].sbyte1 = 1;
        game.objs.aliens[object as usize].worldz = -400;

        strat_friendexitbase_init(&mut game, object);

        let friend = game.objs.aliens[object as usize];
        assert_eq!(friend.sbyte1, 1);
        assert_eq!(friend.worldz, -350);
        assert_eq!(friend.count, 29);
    }

    #[test]
    fn friend_stays_hidden_until_its_launch_countdown_finishes() {
        let mut game = Game::new();
        let object = game.objs.alloc().expect("friend object");
        game.objs.aliens[object as usize].sbyte1 = 2;
        game.objs.aliens[object as usize].worldz = -400;

        strat_friendexitbase_init(&mut game, object);

        let waiting = game.objs.aliens[object as usize];
        assert_eq!(waiting.sbyte1, 1);
        assert_eq!(waiting.worldz, -400);
        assert_ne!(waiting.sflags4 & ASF4_INVISIBLE, 0);

        friendexitbase_strat(&mut game, object);

        let launched = game.objs.aliens[object as usize];
        assert_eq!(launched.worldz, -350);
        assert_eq!(launched.sflags4 & ASF4_INVISIBLE, 0);
    }

    #[test]
    fn final_movement_pass_marks_the_friend_for_removal() {
        let mut game = Game::new();
        let object = game.objs.alloc().expect("friend object");
        let friend = &mut game.objs.aliens[object as usize];
        friend.sbyte1 = 1;
        friend.count = 1;

        friendexitbase_strat(&mut game, object);

        assert_eq!(game.objs.aliens[object as usize].count, 0);
        assert_eq!(game.objs.aldead, 1);
    }
}

// ============================================================
// CLEAR-DEMO SHIPS (GCSTRATS.ASM; C strat_enemy.c:6440-6807)
// ============================================================

/// C `clshipboost_step` (strat_enemy.c:6440) — ROM `clshipboost_strat`.
pub fn clshipboost_strat(g: &mut Game, idx: u16) {
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
    let s = sid(g, clshipboost_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if play_sound {
            al.snd2 = 0x32;
        }
        al.stratptr = Some(s);
        al.vel = 120;
    }
    // ROM: s_set_vartobeobj boostobj,x ; boost_sprite
    g.vars.set_sv_i16(crate::common::sv::BOOSTOBJ, idx as i16);
    // Default flame Z if unset (PSTRATS uses #-30; GCSTRATS some paths #-80).
    if g.vars.sv_u8(crate::common::sv::BOOSTZOFF) == 0 {
        crate::common::set_boost_zoff(g, -30);
    }
    let _ = crate::common::boost_sprite(g, None);
}

/// ROM `clshipboost_Istrat` (GCSTRATS.ASM:234) — trigse $32 then boost.
pub fn clshipboost_istrat(g: &mut Game, idx: u16) {
    clshipboost_enter(g, idx, true);
    clshipboost_strat(g, idx);
}

/// ROM `clshipboostnosnd_Istrat` (GCSTRATS.ASM:237) — same boost, no engine SE.
pub fn clshipboostnosnd_istrat(g: &mut Game, idx: u16) {
    clshipboost_enter(g, idx, false);
    clshipboost_strat(g, idx);
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
        // s_beqdec (STRATMAC.INC:6286) is TEST-then-DEC: boost when sword1 is
        // ALREADY 0 on entry, else decrement. The old dec-then-test form fired
        // the warp one frame early. (Matches the CHASE variant + ROM.)
        if al.sword1 == 0 {
            // ASM clshipWARP_cont (GCSTRATS.ASM:143) jumps to clshipboost_Istrat
            // (:234 `trigse $32`) — the warp boost DOES play the sound. (Audit A #28)
            clshipboost_enter(g, idx, true);
            clshipboost_strat(g, idx);
            return;
        }
        al.sword1 -= 1;
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
        // s_beqdec TEST-then-DEC (was dec-then-test, one frame early).
        if al.sword1 == 0 {
            clshipboost_enter(g, idx, true);
            clshipboost_strat(g, idx);
            return;
        }
        al.sword1 -= 1;
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

/// ROM `pZrotfloattab` (PSTRATS.ASM:3635) — 28 signed bytes.
const CLSHIP_PZROT_FLOAT: [i8; 28] = [
    0, 1, 2, 3, 4, 4, 5, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -5, -4, -4, -3, -2, -1,
];

/// ROM `viewfloattab` (GSTRATS.ASM:3140) — 36 signed words (`viewfloattab_len` = 72 bytes).
const CLSHIP_VIEW_FLOAT: [i16; 36] = [
    0, 1, 2, 3, 4, 4, 5, 5, 6, 6, 6, 5, 5, 4, 4, 3, 2, 1, 0, -1, -2, -3, -4, -4, -5, -5, -6, -6,
    -6, -5, -5, -4, -4, -3, -2, -1,
];

/// Advance `sbyte3` through `pZrotfloattab` (wrap at 28).
fn clship_advance_pzrot(al: &mut sf_game::alien::Alien) -> i8 {
    al.sbyte3 = al.sbyte3.wrapping_add(1);
    if al.sbyte3 as usize >= CLSHIP_PZROT_FLOAT.len() {
        al.sbyte3 = 0;
    }
    CLSHIP_PZROT_FLOAT[al.sbyte3 as usize]
}

/// Advance `sbyte4` through `viewfloattab` using the ROM `s_scale_alvar +1` /
/// wrap-at-72-bytes / `s_scale_alvar -1` dance. Returns the looked-up word
/// (before any table-scale applied by the caller).
fn clship_advance_viewfloat(al: &mut sf_game::alien::Alien) -> i16 {
    // s_inc; <<1; wrap at viewfloattab_len (72); lookup at byte offset; >>1.
    let mut byte_i = al.sbyte4.wrapping_add(1);
    byte_i = byte_i.wrapping_shl(1);
    if byte_i as usize >= CLSHIP_VIEW_FLOAT.len() * 2 {
        byte_i = 0;
    }
    let word = CLSHIP_VIEW_FLOAT[(byte_i as usize) / 2];
    al.sbyte4 = byte_i >> 1;
    word
}

/// ROM `floatCLship_l` (GCSTRATS.ASM:2069): *set* rotz/worldy from float tables
/// (table scale +1 = ASL once on each looked-up value).
pub fn float_clship(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let rz = clship_advance_pzrot(al);
    al.rotz = (rz as u8).wrapping_shl(1);
    let wy = clship_advance_viewfloat(al);
    al.worldy = wy.wrapping_shl(1);
}

/// ROM `floatCLship2_l` (GCSTRATS.ASM:2088): *add* scaled table deltas to
/// rotz/worldy (table scale -2 = two arithmetic /2 on each looked-up value).
pub fn float_clship2(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let dz = clship_advance_pzrot(al) >> 2;
    al.rotz = al.rotz.wrapping_add(dz as u8);
    let dy = clship_advance_viewfloat(al) >> 2;
    al.worldy = al.worldy.wrapping_add(dy);
}

/// Alias used by EARTH/TURN cont paths (same as `float_clship2`).
fn clship_float2(g: &mut Game, idx: u16) {
    float_clship2(g, idx);
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
        clshipboost_strat(g, idx);
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
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 10;
    al.rotz = DEG90.wrapping_neg();
    al.sbyte3 = random_yaw;
    al.sbyte4 = random_pitch;
}

/// C `Strat_ClshipEARTHB_Init` (strat_enemy.c:6773).
pub fn strat_clship_earthb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_earthb_strat);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 20;
    al.rotz = DEG90.wrapping_add(DEG45);
    al.sbyte3 = random_yaw;
    al.sbyte4 = random_pitch;
}

/// C `Strat_ClshipEARTHC_Init` (strat_enemy.c:6782).
pub fn strat_clship_earthc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_earthc_strat);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1;
    al.sbyte1 = 30;
    al.sbyte3 = random_yaw;
    al.sbyte4 = random_pitch;
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
// CLEAR-DEMO SHIPS — SHIP / TURN / BRIDGE / DIVE / UNDER families
// (GCSTRATS.ASM:318-1033). Appended alongside the WARP/GND/EARTH/CHASE
// families above; they share the same infrastructure (clship_flyinleft/right,
// clship_float2, clship_common_init, clshipboost_enter/_step,
// chase_proportional, achase_angle, add_player_z, gen_vecs_2d/_3d,
// apply_velocity, speed_to). These are non-firing demo fly-through ships;
// behaviour is movement only. The `boost_sprite`/`boostobj` shape-swap and the
// commented-out `set_sound*`/engine-sound audio hooks in the ROM are omitted
// (visual/audio, consistent with the WARP/GND/EARTH/CHASE ports).
// ============================================================

// ---- SHIP family (clshipSHIPa/b/c -> clship1/2/3, GCSTRATS.ASM:318-368) ----
// Structurally EARTH minus the floatCLship2 wobble (and slightly different
// rotz/Y offsets): fly-in via worldx chase, then the shared clship_cont
// player-relative chase + sflag1 space-boost machinery.

fn clship_shipa_strat(g: &mut Game, idx: u16) {
    // ASM clship1_strat (GCSTRATS.ASM:330) `s_achase_alvar al_worldx,#-50,4`.
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, -50, 4);
    // :331-332 svar_word2=100 (Z), svar_word3=20 (Y).
    clship_cont(g, idx, 100, 20);
}

fn clship_shipb_strat(g: &mut Game, idx: u16) {
    // ASM clship2_strat (GCSTRATS.ASM:347).
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 50, 4);
    clship_cont(g, idx, 200, 40);
}

fn clship_shipc_strat(g: &mut Game, idx: u16) {
    // ASM clship3_strat (GCSTRATS.ASM:365).
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 0, 4);
    clship_cont(g, idx, 300, 50);
}

/// C-less port of `clshipSHIPa_Istrat` (GCSTRATS.ASM:318).
pub fn strat_clship_shipa_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipa_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1; // :320 s_set_alsflag sflag1
    al.sbyte1 = 10; // :321
    al.rotz = DEG90.wrapping_neg(); // :326 -deg90
}

/// ROM `clship1_Istrat` (GCSTRATS.ASM:322) — SHIPa without the sflag1/sbyte1
/// space-boost latch (falls through into the same body after those two stores).
pub fn clship1_istrat(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipa_strat);
    g.objs.aliens[idx as usize].rotz = DEG90.wrapping_neg();
}

/// ROM `clship1_strat` (GCSTRATS.ASM:328).
pub fn clship1_strat(g: &mut Game, idx: u16) {
    clship_shipa_strat(g, idx);
}

/// Port of `clshipSHIPb_Istrat` (GCSTRATS.ASM:335).
pub fn strat_clship_shipb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1; // :337
    al.sbyte1 = 20; // :338
    al.rotz = DEG90; // :343
}

/// ROM `clship2_Istrat` (GCSTRATS.ASM:339).
pub fn clship2_istrat(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipb_strat);
    g.objs.aliens[idx as usize].rotz = DEG90;
}

/// ROM `clship2_strat` (GCSTRATS.ASM:345).
pub fn clship2_strat(g: &mut Game, idx: u16) {
    clship_shipb_strat(g, idx);
}

/// Port of `clshipSHIPc_Istrat` (GCSTRATS.ASM:354). No rotz set -> stays 0.
pub fn strat_clship_shipc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipc_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= CLSHIP_FLAG1; // :356
    al.sbyte1 = 30; // :357
}

/// ROM `clship3_Istrat` (GCSTRATS.ASM:358).
pub fn clship3_istrat(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_shipc_strat);
}

/// ROM `clship3_strat` (GCSTRATS.ASM:363).
pub fn clship3_strat(g: &mut Game, idx: u16) {
    clship_shipc_strat(g, idx);
}

// ---- TURN family (clshipTURNa/b/c, GCSTRATS.ASM:435-560) ----
// Fly-in, then clshipTURN_cont (player chase + floatCLship2 wobble). On the
// sword1 timeout it banks (clshipTURN_strat) then rolls away
// (clshipTURN2_strat) and the engine auto-removes it once behind the view.

/// ASM clshipTURN_cont (GCSTRATS.ASM:500). `svar_word2` is ADDED to the
/// player Z here (unlike SHIP/EARTH's clship_cont which subtracts).
fn clship_turn_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    // :502 `s_beqdec_alvar al_sword1,clshipTURN_Istrat` — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sword1 == 0 {
        clship_turn_enter(g, idx);
        clship_turn_step(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    let pl = player(g);
    if let Some(pl) = pl {
        let al = &mut g.objs.aliens[idx as usize];
        // :506-508 worldz chase toward player.worldz + svar_word2, shift 4.
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        // :510-512 worldy chase toward player.worldy + svar_word3, shift 5.
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 5);
        // :514 `s_achase_alvar al_rotz,#0,5`.
        let mut rotz = al.rotz;
        achase_angle(&mut rotz, 0, 5);
        al.rotz = rotz;
    }
    // :516 `jsl floatCLship2_l` — adds to rotz/worldy (unconditional).
    clship_float2(g, idx);
    if let Some(pl) = pl {
        // :518 `s_achase_alvar2alvar al_rotx,y,al_rotx,5`.
        let al = &mut g.objs.aliens[idx as usize];
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
    }
    // :520 add_playerZ (no psvar_word2 add for TURN_cont).
    add_player_z(g, idx);
}

/// ASM clshipTURN_Istrat (GCSTRATS.ASM:525): begin the bank-in.
fn clship_turn_enter(g: &mut Game, idx: u16) {
    let s = sid(g, clship_turn_step);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = 42; // :527
}

/// ASM clshipTURN_strat (GCSTRATS.ASM:528): ramp speed to 32 while yawing.
fn clship_turn_step(g: &mut Game, idx: u16) {
    // :530 `s_beqdec_alvar al_sbyte2,clshipTURN2_Istrat` — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        clship_turn2_enter(g, idx);
        clship_turn2_step(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte2 -= 1;
        speed_to(al, 32, 1); // :531
        al.roty = al.roty.wrapping_sub(1); // :532
        al.rotz = al.rotz.wrapping_sub(1); // :533 add rotz #-1
    }
    // clshipTURN2_cont :556-558.
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// ASM clshipTURN2_Istrat (GCSTRATS.ASM:536): begin the roll-away; the ship is
/// now set to auto-remove once behind the view.
fn clship_turn2_enter(g: &mut Game, idx: u16) {
    let s = sid(g, clship_turn2_step);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    // :538 sbyte1 = (deg180+42)/4 = 170/4 = 42.
    al.sbyte1 = ((DEG180 as u16 + 42) / 4) as u8;
    al.type_ |= ATZREMOVE; // :543 s_setremove_behind
}

/// Public entry for `clshipTURN2_Istrat` (GCSTRATS.ASM:536).
pub fn clship_turn2_istrat(g: &mut Game, idx: u16) {
    clship_turn2_enter(g, idx);
}

/// ASM clshipTURN2_strat (GCSTRATS.ASM:544): roll up for sbyte1 frames, then
/// settle rotz.
fn clship_turn2_step(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // :547 `s_beqdec_alvar al_sbyte1,.nadd` — TEST-then-DEC (local branch).
        if al.sbyte1 == 0 {
            al.rotz = al.rotz.wrapping_sub(2); // .nadd :552
        } else {
            al.sbyte1 -= 1;
            al.rotz = al.rotz.wrapping_add(2); // :548
            al.roty = al.roty.wrapping_add(4); // :549
        }
        // clshipTURN2_cont :556-558.
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// Public entry for `clshipTURN2_strat` / `clshipTURN2_cont` (GCSTRATS.ASM:544/556).
pub fn clship_turn2_strat(g: &mut Game, idx: u16) {
    clship_turn2_step(g, idx);
}

/// Public alias for the shared TURN2 cont tail (vecs + playerZ).
pub fn clship_turn2_cont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

fn clship_turna_strat(g: &mut Game, idx: u16) {
    clship_flyinleft(g, idx); // :451
    clship_turn_cont(g, idx, 100, -20); // :453-454 word2=100, word3=-20
}

fn clship_turnb_strat(g: &mut Game, idx: u16) {
    clship_flyinright(g, idx); // :475
    clship_turn_cont(g, idx, 200, -20);
}

fn clship_turnc_strat(g: &mut Game, idx: u16) {
    // :493 `s_achase_alvar al_worldx,#0,4`.
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 0, 4);
    clship_turn_cont(g, idx, 300, -30);
}

/// sbyte1 = (deg180+deg45+deg22)/4 = 176/4 = 44 (set at init; overwritten in
/// clshipTURN2_Istrat before use, but kept for state fidelity).
const CLSHIP_TURN_SBYTE1: u8 = ((DEG180 as u16 + DEG45 as u16 + DEG22 as u16) / 4) as u8;

/// Port of `clshipTURNa_Istrat` (GCSTRATS.ASM:435).
pub fn strat_clship_turna_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_turna_strat);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.vx = 10; // :440
    al.rotz = DEG90.wrapping_neg(); // :441 -deg90
    al.sword1 = 100 + 130 + 10 - CLSHIP_BUNNYWAIT; // :442 = 180
    al.sbyte1 = CLSHIP_TURN_SBYTE1; // :443
    al.sflags2 |= CLSHIP_FLAG2; // :444 set sflag2 (sound-only, dead)
    al.sbyte3 = random_yaw; // :445
    al.sbyte4 = random_pitch; // :446
}

/// Port of `clshipTURNb_Istrat` (GCSTRATS.ASM:460).
pub fn strat_clship_turnb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_turnb_strat);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.vx = -10; // :465
    al.rotz = DEG90; // :466
    al.sword1 = 100 + 120 + 10 - CLSHIP_FROGWAIT; // :467 = 200
    al.sbyte1 = CLSHIP_TURN_SBYTE1; // :468
    al.sflags2 &= !CLSHIP_FLAG2; // :469 clr sflag2
    al.sbyte3 = random_yaw;
    al.sbyte4 = random_pitch;
}

/// Port of `clshipTURNc_Istrat` (GCSTRATS.ASM:481). No vx/rotz set.
pub fn strat_clship_turnc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_turnc_strat);
    let random_yaw = (sf_random(&mut g.vars) & 15) as u8;
    let random_pitch = (sf_random(&mut g.vars) & 7) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = 100 + 100 + 10 - CLSHIP_COCKWAIT; // :486 = 120
    al.sbyte1 = CLSHIP_TURN_SBYTE1; // :487
    al.sflags2 |= CLSHIP_FLAG2; // :488
    al.sbyte3 = random_yaw;
    al.sbyte4 = random_pitch;
}

// ---- BRIDGE family (clshipBRIDGEa/b/c, GCSTRATS.ASM:564-669) ----
// Player chase (identical shape to CHASE_cont), then a bridge-specific boost
// (clshipBridgeboost_strat) that drifts sideways for 50 frames before handing
// off to the general boost.

/// ASM clshipBRIDGE_cont (GCSTRATS.ASM:616).
fn clship_bridge_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    // :617 `s_beqdec_alvar al_sword1,clshipbridgeboost_Istrat` — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sword1 == 0 {
        clship_bridgeboost_enter(g, idx);
        clship_bridgeboost_step(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    if let Some(pl) = player(g) {
        let tick1 = frame_tick_mod(g, 1);
        let al = &mut g.objs.aliens[idx as usize];
        // :620-622 worldz chase toward player.worldz + svar_word2, shift 4.
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        // :624-626 worldy chase toward player.worldy + svar_word3, shift 5.
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 5);
        // :629 `s_achase_alvar2alvar al_rotx,y,al_rotx,5`.
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
        // :631 `s_jmp_notdelay 1,.ny`.
        if tick1 {
            let mut rotz = al.rotz;
            achase_angle(&mut rotz, pl.rotz, 5); // :632
            al.rotz = rotz;
            let mut roty = al.roty;
            achase_angle(&mut roty, pl.roty, 4); // :633
            al.roty = roty;
        }
    }
    add_player_z(g, idx); // :636
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2); // :637
}

/// ASM clshipBridgeboost_Istrat (GCSTRATS.ASM:642).
fn clship_bridgeboost_enter(g: &mut Game, idx: u16) {
    let s = sid(g, clship_bridgeboost_step);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte1 = 50; // :644
    al.vel = 20; // :645 s_set_speed #20
}

/// Public registry entry for `clshipBridgeboost_Istrat`. The init falls
/// through to the first boost tick in the same strategy dispatch.
pub(crate) fn clship_bridgeboost_istrat(g: &mut Game, idx: u16) {
    clship_bridgeboost_enter(g, idx);
    clship_bridgeboost_step(g, idx);
}

/// ASM clshipBridgeboost_strat (GCSTRATS.ASM:646): sideways drift (vz forced
/// to 0), slow roll, then general boost when sbyte1 expires.
fn clship_bridgeboost_step(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    let tick3 = frame_tick_mod(g, 3);
    {
        let al = &mut g.objs.aliens[idx as usize];
        strat_gen_vecs_nvecs(al); // :649 s_gen_vecs → nvecs_l
        al.vz = 0; // :650 s_set_alvar al_vz,#0
        apply_velocity(al); // :651
        if tick1 {
            al.rotz = al.rotz.wrapping_sub(1); // :654-655
        }
        if tick3 {
            al.roty = al.roty.wrapping_sub(1); // :659
            al.rotx = al.rotx.wrapping_sub(1); // :660
        }
    }
    // :663 `s_decbne_alvar al_sbyte1,.nengineoff` — DEC-then-BNE.
    let engine_off = {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = al.sbyte1.wrapping_sub(1);
        al.sbyte1 == 0
    };
    if engine_off {
        // :664 clear engine sound, :665 brl clshipboost_Istrat (plays $32).
        g.vars.pshipflags3 &= !PSF3_ENGINESND;
        clshipboost_enter(g, idx, true);
        clshipboost_strat(g, idx);
        return;
    }
    add_player_z(g, idx); // :668
}

fn clship_bridgea_strat(g: &mut Game, idx: u16) {
    // :575 `s_achase_alvar al_worldx,#-70,4`.
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, -70, 4);
    clship_bridge_cont(g, idx, -100, 20); // :577-578
}

fn clship_bridgeb_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 70, 4); // :595
    clship_bridge_cont(g, idx, -200, 20);
}

fn clship_bridgec_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 0, 4); // :609
    clship_bridge_cont(g, idx, -300, 30);
}

/// Port of `clshipBRIDGEa_Istrat` (GCSTRATS.ASM:564).
pub fn strat_clship_bridgea_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_bridgea_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90.wrapping_neg(); // :569 -deg90
    al.roty = DEG45; // :570
    al.sword1 = 130 - CLSHIP_BUNNYWAIT; // :571 = 70
}

/// Port of `clshipBRIDGEb_Istrat` (GCSTRATS.ASM:584).
pub fn strat_clship_bridgeb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_bridgeb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90; // :589
    al.roty = DEG45.wrapping_neg(); // :590 -deg45
    al.sword1 = 140 - CLSHIP_FROGWAIT; // :591 = 110
}

/// Port of `clshipBRIDGEc_Istrat` (GCSTRATS.ASM:601). No rotz/roty set.
pub fn strat_clship_bridgec_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_bridgec_strat);
    g.objs.aliens[idx as usize].sword1 = 150 - CLSHIP_COCKWAIT; // :606 = 60
}

// ---- DIVE family (clshipDIVEa/b/c, GCSTRATS.ASM:672-803) ----
// Three sword1 regimes: >60 normal player chase (clshipDIVE_cont); 30..60 the
// dive tilt (rotz/vx/vy nudges + velocity add); <30 velocity-only, snapping
// onto the player at sword1==1 for the fly-past; at 0 the DIVE boost levels
// out (rotx=deg5) and hands to the general boost.

/// ASM clshipDIVE_cont (GCSTRATS.ASM:774): high-altitude player chase, falls
/// through to clshipDIVE_cont2.
fn clship_dive_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    if let Some(pl) = player(g) {
        let al = &mut g.objs.aliens[idx as usize];
        // :779-781 worldz chase toward player.worldz + svar_word2, shift 4.
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        // :783-785 worldy chase toward player.worldy + svar_word3, shift 4.
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 4);
        // :788 `s_achase_alvar al_rotz,#0,5`.
        let mut rotz = al.rotz;
        achase_angle(&mut rotz, 0, 5);
        al.rotz = rotz;
        // :790 `s_achase_alvar2alvar al_rotx,y,al_rotx,5`.
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
    }
    clship_dive_cont2(g, idx);
}

/// ASM clshipDIVE_cont2 (GCSTRATS.ASM:792).
fn clship_dive_cont2(g: &mut Game, idx: u16) {
    // :793 `s_beqdec_alvar al_sword1,clshipDIVEboost_Istrat` — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sword1 == 0 {
        clship_diveboost(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    add_player_z(g, idx); // :794
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2); // :795
}

/// Public entry for `clshipDIVE_cont2` (GCSTRATS.ASM:792).
pub fn clship_dive_cont2_pub(g: &mut Game, idx: u16) {
    clship_dive_cont2(g, idx);
}

/// ASM clshipDIVEboost_Istrat (GCSTRATS.ASM:799): level out then general boost.
fn clship_diveboost(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = 0; // :801
        al.rotx = DEG5; // :802
    }
    clshipboost_enter(g, idx, true); // :803 -> clshipboost_Istrat (trigse $32)
    clshipboost_strat(g, idx);
}

/// Public entry for `clshipDIVEboost_Istrat` (GCSTRATS.ASM:799).
pub fn clship_diveboost_istrat(g: &mut Game, idx: u16) {
    clship_diveboost(g, idx);
}

fn clship_divea_strat(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    let tick2 = frame_tick_mod(g, 2);
    // :681 `s_achase_alvar al_worldx,#-50,4`.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, -50, 4);
    }
    let sword1 = g.objs.aliens[idx as usize].sword1;
    // :686 `s_jmp_alvarMORE al_sword1,#60,.nhigh`.
    if sword1 > 60 {
        clship_dive_cont(g, idx, 100, -20); // :683-684
        return;
    }
    // :687 `s_jmp_alvarLESS al_sword1,#30,.nvy`.
    if sword1 >= 30 {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(2); // :688
        if tick2 {
            al.vx = al.vx.wrapping_sub(1); // :690 add vx #-1
        }
        if tick1 {
            al.vy = al.vy.wrapping_sub(1); // :692 add vy #-1
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]); // .nvy :694
                                                      // :695 `s_jmp_alvarNE al_sword1,#1,.nb`: snap onto player for the fly-past.
    if sword1 == 1 && g.objs.player().is_some() {
        copy_pos(g, idx, 0); // :697 s_copy_pos x,y (player -> me)
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(200); // :698
        al.worldx = al.worldx.wrapping_sub(50); // :699
    }
    clship_dive_cont2(g, idx); // :702
}

fn clship_diveb_strat(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    let tick2 = frame_tick_mod(g, 2);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 50, 4); // :718
    }
    let sword1 = g.objs.aliens[idx as usize].sword1;
    if sword1 > 60 {
        clship_dive_cont(g, idx, 200, -20); // :720-721
        return;
    }
    if sword1 >= 30 {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_sub(2); // :725 sub rotz #2
        if tick2 {
            al.vx = al.vx.wrapping_add(1); // :727 add vx #1
        }
        if tick1 {
            al.vy = al.vy.wrapping_sub(1); // :729 add vy #-1
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]); // :731
    if sword1 == 1 && g.objs.player().is_some() {
        copy_pos(g, idx, 0); // :734
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(200); // :735
        al.worldx = al.worldx.wrapping_add(50); // :736 add worldx #50
    }
    clship_dive_cont2(g, idx); // :739
}

fn clship_divec_strat(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = chase_proportional(al.worldx, 0, 4); // :752
    }
    let sword1 = g.objs.aliens[idx as usize].sword1;
    if sword1 > 60 {
        clship_dive_cont(g, idx, 300, -30); // :753-754
        return;
    }
    // :757 `s_jmp_alvarLESS al_sword1,#30,.nvy`; DIVEc's tilt is Y-only.
    if sword1 >= 30 && tick1 {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_sub(1); // :759 add vy #-1
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]); // :761
    if sword1 == 1 && g.objs.player().is_some() {
        copy_pos(g, idx, 0); // :764
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(200); // :765 (no worldx offset)
    }
    clship_dive_cont2(g, idx); // :768
}

/// Port of `clshipDIVEa_Istrat` (GCSTRATS.ASM:672). No shadow flag in the ROM.
pub fn strat_clship_divea_init(g: &mut Game, idx: u16) {
    let s = sid(g, clship_divea_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.type_ &= !ATZREMOVE; // :675 s_setnoremove_behind
    al.stratptr = Some(s);
    al.sword1 = 180 - CLSHIP_BUNNYWAIT; // :676 = 120
    al.rotz = DEG90.wrapping_neg(); // :677 -deg90
}

/// Port of `clshipDIVEb_Istrat` (GCSTRATS.ASM:709).
pub fn strat_clship_diveb_init(g: &mut Game, idx: u16) {
    let s = sid(g, clship_diveb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.type_ &= !ATZREMOVE; // :712
    al.stratptr = Some(s);
    al.sword1 = 190 - CLSHIP_FROGWAIT; // :713 = 160
    al.rotz = DEG90; // :714
}

/// Port of `clshipDIVEc_Istrat` (GCSTRATS.ASM:744).
pub fn strat_clship_divec_init(g: &mut Game, idx: u16) {
    let s = sid(g, clship_divec_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.type_ &= !ATZREMOVE; // :747
    al.stratptr = Some(s);
    al.sword1 = 170 - CLSHIP_COCKWAIT; // :748 = 80
    al.sbyte2 = 20; // :749 general-boost removal timer
}

// ---- UNDER family (clshipUNDERa/b/c, GCSTRATS.ASM:922-1033) ----
// Player chase (same shape as CHASE/BRIDGE_cont), then an underground boost
// (clshipUNDERboost_strat) that flies off along roty banking per sbyte1. No
// auto-remove — the ship simply flies out of view.

/// ASM clshipUNDER_cont (GCSTRATS.ASM:979).
fn clship_under_cont(g: &mut Game, idx: u16, zoff: i16, yoff: i16) {
    // :980 `s_beqdec_alvar al_sword1,clshipUNDERboost_Istrat` — TEST-then-DEC.
    if g.objs.aliens[idx as usize].sword1 == 0 {
        clship_underboost_enter(g, idx);
        clship_underboost_step(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword1 -= 1;
    if let Some(pl) = player(g) {
        let tick1 = frame_tick_mod(g, 1);
        let al = &mut g.objs.aliens[idx as usize];
        // :983-985 worldz chase toward player.worldz + svar_word2, shift 4.
        al.worldz = chase_proportional(al.worldz, pl.worldz.wrapping_add(zoff), 4);
        // :987-989 worldy chase toward player.worldy + svar_word3, shift 5.
        al.worldy = chase_proportional(al.worldy, pl.worldy.wrapping_add(yoff), 5);
        // :992 `s_achase_alvar2alvar al_rotx,y,al_rotx,5`.
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, pl.rotx, 5);
        al.rotx = rotx;
        // :994 `s_jmp_notdelay 1,.ny`.
        if tick1 {
            let mut rotz = al.rotz;
            achase_angle(&mut rotz, pl.rotz, 5); // :995
            al.rotz = rotz;
            let mut roty = al.roty;
            achase_angle(&mut roty, pl.roty, 4); // :996
            al.roty = roty;
        }
    }
    add_player_z(g, idx); // :999
    let w2 = g.vars.psvar_word2;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(w2); // :1000
}

/// ASM clshipUNDERboost_Istrat (GCSTRATS.ASM:1005): just swaps to the boost
/// strat (sound is commented out in the ROM).
fn clship_underboost_enter(g: &mut Game, idx: u16) {
    let s = sid(g, clship_underboost_step);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// Public entry for `clshipUNDERboost_Istrat` (GCSTRATS.ASM:1005).
pub fn clship_underboost_istrat(g: &mut Game, idx: u16) {
    clship_underboost_enter(g, idx);
}

/// ASM clshipUNDERboost_strat (GCSTRATS.ASM:1011): ramp to speed 40 and fly
/// off along roty, banking per sbyte1 (1 = one way, 2 = the other, 0 = none).
fn clship_underboost_step(g: &mut Game, idx: u16) {
    let tick1 = frame_tick_mod(g, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        speed_to(al, 40, 1); // :1014
        strat_gen_vecs_nvecs(al); // :1015 s_gen_vecs → nvecs_l
        apply_velocity(al); // :1016 (vz NOT zeroed, unlike bridge)
                            // :1018 `s_jmp_alvarZERO al_sbyte1,.nnz`.
        if al.sbyte1 != 0 {
            // :1020 `s_jmp_alvarNE al_sbyte1,#1,.nl`.
            if al.sbyte1 == 1 {
                al.rotz = al.rotz.wrapping_add(1); // :1021
                if tick1 {
                    al.roty = al.roty.wrapping_add(1); // :1023
                }
            } else {
                al.rotz = al.rotz.wrapping_sub(1); // :1026
                if tick1 {
                    al.roty = al.roty.wrapping_sub(1); // :1028
                }
            }
        }
    }
    add_player_z(g, idx); // :1032
}

/// Public entry for `clshipUNDERboost_strat` (GCSTRATS.ASM:1011).
pub fn clship_underboost_strat(g: &mut Game, idx: u16) {
    clship_underboost_step(g, idx);
}

fn clship_undera_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, -70, 4); // :935
    clship_under_cont(g, idx, -100, 20); // :937-938
}

fn clship_underb_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 70, 4); // :957
    clship_under_cont(g, idx, -200, 20);
}

fn clship_underc_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = chase_proportional(al.worldx, 0, 4); // :972
    clship_under_cont(g, idx, -300, 30);
}

/// Port of `clshipUNDERa_Istrat` (GCSTRATS.ASM:922).
pub fn strat_clship_undera_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_undera_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90.wrapping_neg(); // :927 -deg90
    al.roty = DEG45; // :928
    al.sword1 = 140 + 5; // :929 = 145
    al.sbyte1 = 1; // :930 bank one way
    al.sflags2 |= CLSHIP_FLAG2; // :931 set sflag2 (sound-only, dead)
}

/// Port of `clshipUNDERb_Istrat` (GCSTRATS.ASM:944).
pub fn strat_clship_underb_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_underb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = DEG90; // :949
    al.roty = DEG45.wrapping_neg(); // :950 -deg45
    al.sword1 = 140 + 10; // :951 = 150
    al.sbyte1 = 2; // :952 bank the other way
    al.sflags2 &= !CLSHIP_FLAG2; // :953 clr sflag2
}

/// Port of `clshipUNDERc_Istrat` (GCSTRATS.ASM:963). No rotz/roty/sbyte1 set.
pub fn strat_clship_underc_init(g: &mut Game, idx: u16) {
    clship_common_init(g, idx, clship_underc_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = 140 + 15; // :968 = 155
    al.sflags2 |= CLSHIP_FLAG2; // :969
}

// ============================================================
// BOSS EXPLOSION STRATEGIES (EXPSTRAT.ASM; C strat_enemy.c:6815-7333)
// Shared with the enemy_b/bosses lanes (pub/pub(crate)).
// ============================================================

const BGM_BOSS_DYING: u8 = 0xF1;
const SE_BOSS_DYING: u8 = 0x1E;

/// C `addrnd2pos_xy` (strat_enemy.c:6832, addrnd2posy_srou).
pub(crate) fn addrnd2pos_xy(g: &mut Game, idx: u16) {
    let rx = (sf_random(&mut g.vars) & 0xFF) as u8 as i8;
    let ry = (sf_random(&mut g.vars) & 0xFF) as u8 as i8;
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
    // `s_make_obj` links the new object immediately after the current source
    // object. Explosion helpers make `parent` current before calling the
    // source subroutine, so preserve that observable same-pass ordering.
    g.objs.active_move_after(child, parent);
    let s_tick = sid(g, delayexplode_strat);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.sflags3 &= !ASF3_REALOBJ;
        al.sflags2 |= ASF2_COLLDISABLE | ASF2_NOEXPSND | ASF2_RELEXPLODE;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.stratptr = Some(s_tick);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
    }
    copy_pos(g, child, parent);
    Some(child)
}

/// Preserve a retail face-less explosion `ShapeHdr` as typed state. Shape zero
/// deliberately selects the native null mesh while the envelope is waiting.
pub(crate) fn set_explosion_envelope(object: &mut Alien, size: ExplosionSize) {
    object.shape = 0;
    object.visual_kind = ObjectVisualKind::ExplosionEnvelope(size);
}

/// C `make_large_exp_obj` (strat_enemy.c:6903, makeLexpobj_srou).
pub fn make_large_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    set_explosion_envelope(&mut g.objs.aliens[child as usize], ExplosionSize::Large);
    Some(child)
}

/// C `make_medium_exp_obj` (strat_enemy.c:6911, makeMEDexpobj_srou).
pub fn make_medium_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    set_explosion_envelope(&mut g.objs.aliens[child as usize], ExplosionSize::Medium);
    Some(child)
}

/// C `make_small_exp_obj` (strat_enemy.c:6919, makeSMLexpobj_srou).
pub fn make_small_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    set_explosion_envelope(&mut g.objs.aliens[child as usize], ExplosionSize::Small);
    Some(child)
}

/// C `make_fol_exp_obj` (strat_enemy.c:6927, makeFOLexpobj_srou).
pub(crate) fn make_fol_exp_obj(g: &mut Game, parent: u16) -> Option<u16> {
    let child = make_exp_obj(g, parent)?;
    set_explosion_envelope(&mut g.objs.aliens[child as usize], ExplosionSize::Oversized);
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
        let sf = g.vars.shared.strategy_flags;
        g.vars.shared.strategy_flags = sf | SF_NOFIRING;
    }
}

/// C `delayexplode_strat` (EXPSTRAT.ASM:259-268).
pub fn delayexplode_strat(g: &mut Game, idx: u16) {
    // ASM EXPSTRAT.ASM:262 `s_decbpl_lifecnt x,.nd` dies when the decrement goes
    // NEGATIVE (entry count 0), surviving count+1 ticks. The old inline
    // `if count>0{--} if count==0{die}` fired one frame early. (Audit A #35)
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_HITFLASH;
        count_down(al)
    };
    if expired {
        // ASM `s_kill_obj x` (STRATMAC.INC:2643) is colldisable + HP:=0 — a
        // death SIGNAL, not a removal. `s_jmpto_expstrat` then runs the
        // explosion inline within this same do_strat invocation, and for
        // objects without nopolyexp the explosion morphs the corpse into its
        // polygon mesh (which must survive as a live object). Setting the
        // removal flag here freed the freshly morphed corpse one tick early.
        crate::common::kill_obj(&mut g.objs.aliens[idx as usize]);
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

const BOSS_CIRCLE_EXPLOSION_SOUND: u8 = 29;

/// Start the authored boss explosion circle around a stable, ordinary world
/// object. Allocation failure falls back to the screen center, matching the
/// source effect's null-object path.
pub(crate) fn start_boss_explosion_circle(g: &mut Game, idx: u16) -> Option<u16> {
    g.hooks.play_se(BOSS_CIRCLE_EXPLOSION_SOUND);
    g.vars.strategy.circle_object = 0;

    let center = if let Some(anchor) = make_obj(g, 0) {
        copy_pos(g, anchor, idx);
        crate::ground::strat_stayrel_init(g, anchor);
        let object_id = anchor + 1;
        g.vars.strategy.circle_object = object_id as i16;
        ScreenFillCircleCenter::Object(object_id)
    } else {
        ScreenFillCircleCenter::Screen
    };
    g.vars.screen_fill_circle.begin_boss_explosion(center);

    match center {
        ScreenFillCircleCenter::Object(object_id) => Some(object_id - 1),
        ScreenFillCircleCenter::Screen | ScreenFillCircleCenter::World { .. } => None,
    }
}

/// C `circdelayexplode_strat` (EXPSTRAT.ASM:273-294 tick half).
pub(crate) fn circdelayexplode_strat(g: &mut Game, idx: u16) {
    // ASM EXPSTRAT.ASM:280 `s_decbpl_lifecnt x,.nd` dies when the decrement goes
    // NEGATIVE (entry count 0). Old inline fired one frame early. (Audit A #35)
    if count_down(&mut g.objs.aliens[idx as usize]) {
        let _ = start_boss_explosion_circle(g, idx);
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
    /// `exitlight3_Istrat` through `exitlight6_Istrat` (rows 4..=7).
    pub exitlight3: StratId,
    pub exitlight4: StratId,
    pub exitlight5: StratId,
    pub exitlight6: StratId,
    /// Medium/large tunnel exit doors (rows 13 and 14).
    pub exitopen: StratId,
    pub exitopensnd: StratId,
    /// `hard_Istrat` (`Strat_Hard_Init`).
    pub hard: StratId,
    /// `hardenemy1_Istrat` — hardvars + COLLTYPE_ENEMY1, inert.
    pub hardenemy1: StratId,
    /// `hard180YRfog_Istrat` — retained shape + hardvars + fog tick.
    pub hard180yrfog: StratId,
    /// `hard90yrfog_Istrat` — face 180°, hardvars, fog tick (ISTRATS 183).
    pub hard90yrfog: StratId,
    /// `shark_Istrat` (ISTRATS 60).
    pub shark: StratId,
    /// `fzaco_Istrat` (ISTRATS 113).
    pub fzaco: StratId,
    /// Colony highway traffic (ISTRATS 198..=202 and 213..=214).
    pub aircar1: StratId,
    pub aircar2: StratId,
    pub aircar3: StratId,
    pub aircar4: StratId,
    pub aircar5: StratId,
    pub truck1: StratId,
    pub truck2: StratId,
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
    /// `cameleon2_Istrat` (GASTRATS.ASM:1440).
    pub cameleon2: StratId,
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
    /// `clshipSHIPa_Istrat`.
    pub clship_shipa: StratId,
    /// `clshipSHIPb_Istrat`.
    pub clship_shipb: StratId,
    /// `clshipSHIPc_Istrat`.
    pub clship_shipc: StratId,
    /// `clshipTURNa_Istrat`.
    pub clship_turna: StratId,
    /// `clshipTURNb_Istrat`.
    pub clship_turnb: StratId,
    /// `clshipTURNc_Istrat`.
    pub clship_turnc: StratId,
    /// `clshipBRIDGEa_Istrat`.
    pub clship_bridgea: StratId,
    /// `clshipBRIDGEb_Istrat`.
    pub clship_bridgeb: StratId,
    /// `clshipBRIDGEc_Istrat`.
    pub clship_bridgec: StratId,
    /// `clshipDIVEa_Istrat`.
    pub clship_divea: StratId,
    /// `clshipDIVEb_Istrat`.
    pub clship_diveb: StratId,
    /// `clshipDIVEc_Istrat`.
    pub clship_divec: StratId,
    /// `clshipUNDERa_Istrat`.
    pub clship_undera: StratId,
    /// `clshipUNDERb_Istrat`.
    pub clship_underb: StratId,
    /// `clshipUNDERc_Istrat`.
    pub clship_underc: StratId,
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
        exitlight3: sid(g, exitlight3_istrat),
        exitlight4: sid(g, exitlight4_istrat),
        exitlight5: sid(g, exitlight5_istrat),
        exitlight6: sid(g, exitlight6_istrat),
        exitopen: sid(g, exitopen_istrat),
        exitopensnd: sid(g, exitopensnd_istrat),
        hard: sid(g, strat_hard_init),
        hardenemy1: sid(g, hardenemy1_istrat),
        hard180yrfog: sid(g, hard180yrfog_istrat),
        hard90yrfog: sid(g, hard90yrfog_istrat),
        shark: sid(g, shark_istrat),
        fzaco: sid(g, fzaco_istrat),
        aircar1: sid(g, aircar1_istrat),
        aircar2: sid(g, aircar2_istrat),
        aircar3: sid(g, aircar3_istrat),
        aircar4: sid(g, aircar4_istrat),
        aircar5: sid(g, aircar5_istrat),
        truck1: sid(g, truck1_istrat),
        truck2: sid(g, truck2_istrat),
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
        cameleon2: sid(g, cameleon2_istrat),
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
        clship_shipa: sid(g, strat_clship_shipa_init),
        clship_shipb: sid(g, strat_clship_shipb_init),
        clship_shipc: sid(g, strat_clship_shipc_init),
        clship_turna: sid(g, strat_clship_turna_init),
        clship_turnb: sid(g, strat_clship_turnb_init),
        clship_turnc: sid(g, strat_clship_turnc_init),
        clship_bridgea: sid(g, strat_clship_bridgea_init),
        clship_bridgeb: sid(g, strat_clship_bridgeb_init),
        clship_bridgec: sid(g, strat_clship_bridgec_init),
        clship_divea: sid(g, strat_clship_divea_init),
        clship_diveb: sid(g, strat_clship_diveb_init),
        clship_divec: sid(g, strat_clship_divec_init),
        clship_undera: sid(g, strat_clship_undera_init),
        clship_underb: sid(g, strat_clship_underb_init),
        clship_underc: sid(g, strat_clship_underc_init),
        boss_delay_explode: sid(g, strat_boss_delay_explode_init),
        qboss_explode: sid(g, strat_qboss_explode_init),
        boss_explode: sid(g, strat_boss_explode_init),
        hit_flash: sid(g, strat_hit_flash),
        explode: sid(g, strat_explode),
    }
}

#[cfg(test)]
mod sound_wiring_tests {
    //! Findings F1/F2 (door -> positional make_snd) and F5/F6 (stray
    //! placeholder chime deleted) from docs/AUDIT_SOUND_IDS_FINDINGS.md.
    use super::*;
    use sf_game::game::{Hooks, PosSndFamilyId};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum SndEvent {
        PlaySe(u8),
        MakeSnd(PosSndFamilyId, i16, i16),
    }

    #[derive(Clone, Default)]
    struct Rec(Rc<RefCell<Vec<SndEvent>>>);
    impl Hooks for Rec {
        fn play_se(&mut self, id: u8) {
            self.0.borrow_mut().push(SndEvent::PlaySe(id));
        }
        fn trig_se(&mut self, id: u8) {
            self.0.borrow_mut().push(SndEvent::PlaySe(id));
        }
        fn make_snd(&mut self, family: PosSndFamilyId, x: i16, z: i16) {
            self.0.borrow_mut().push(SndEvent::MakeSnd(family, x, z));
        }
    }

    fn game_with_alien() -> (Game, u16, Rc<RefCell<Vec<SndEvent>>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut g = Game::with_hooks(Box::new(Rec(log.clone())));
        let idx = g.objs.alloc().expect("alloc alien");
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.active = true;
            al.worldx = 111;
            al.worldz = 222;
        }
        (g, idx, log)
    }

    #[test]
    fn f1_base1_door_open_fires_positional_dooropen() {
        let (mut g, idx, log) = game_with_alien();
        // s_test_hitflags x,#HF1 must pass to reach the door-open sound.
        g.objs.aliens[idx as usize].hitflags |= HF1_MASK;
        base1_strat(&mut g, idx);
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::DoorOpen, 111, 222)],
            "door-open must go through positional make_snd(POS_DOOROPEN)"
        );
    }

    #[test]
    fn f2_base1_door_close_fires_positional_doorclose() {
        let (mut g, idx, log) = game_with_alien();
        // Drive base1_wait_strat straight into its .close branch (sbyte1 == 0).
        let s = sid(&mut g, base1_wait_strat);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.stratptr = Some(s);
            al.sbyte1 = 0;
            al.animframe = 8; // so base1_close_strat doesn't immediately re-init
        }
        base1_wait_strat(&mut g, idx);
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::DoorClose, 111, 222)],
            "door-close must go through positional make_snd(POS_DOORCLOSE)"
        );
    }

    #[test]
    fn f5_tow0_explode_emits_no_sound() {
        let (mut g, idx, log) = game_with_alien();
        strat_tow0_explode(&mut g, idx);
        assert!(
            log.borrow().is_empty(),
            "tow0explode/pillarexplode plays no direct sound (F5); got {:?}",
            log.borrow()
        );
    }

    #[test]
    fn f6_zaco3die_init_emits_no_sound() {
        let (mut g, idx, log) = game_with_alien();
        zaco3die_init(&mut g, idx);
        assert!(
            log.borrow().is_empty(),
            "zaco3die_init plays no sound (F6); got {:?}",
            log.borrow()
        );
    }

    #[test]
    fn fire_plasma_family_uses_enemybattry_make_snd() {
        let (mut g, firer, log) = game_with_alien();
        let _ = fire_plasma(&mut g, firer).expect("plasma");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, 111, 222)],
            "fire_plasma must jsl enemybattrysound_l via make_snd"
        );
        log.borrow_mut().clear();
        let _ = fire_beamball(&mut g, firer).expect("ball");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, 111, 222)]
        );
        log.borrow_mut().clear();
        let _ = fire_yhplasma(&mut g, firer).expect("yh");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, 111, 222)]
        );
        log.borrow_mut().clear();
        let _ = fire_ringlaser(&mut g, firer).expect("ring");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::RingLaser, 111, 222)],
            "fire_ringlaser must jsl ringlasersound_l via make_snd"
        );
        log.borrow_mut().clear();
        let _ = fire_shortplasma(&mut g, firer).expect("short");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::EnemyBattry, 111, 222)]
        );
    }

    #[test]
    fn fire_laser_missile_use_positional_make_snd() {
        let (mut g, firer, log) = game_with_alien();
        let _ = fire_friend_elaser(&mut g, firer).expect("friend");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::Laser, 111, 222)],
            "fire_friendElaser must jsl lasersound_l via make_snd"
        );
        log.borrow_mut().clear();
        let _ = fire_reb_elaser(&mut g, firer).expect("reb");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::HitWall, 111, 222)],
            "fire_RebElaser must jsl hitwallsound_l via make_snd"
        );
        log.borrow_mut().clear();
        let _ = fire_slow_elaser(&mut g, firer).expect("slow");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::Laser, 111, 222)]
        );
        log.borrow_mut().clear();
        strat_fire_relslowlaser(&mut g, firer, 0, 0);
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::Laser, 111, 222)]
        );
        log.borrow_mut().clear();
        let _ = fire_missile1(&mut g, firer).expect("m1");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::Missile, 111, 222)],
            "fire_missile1 must jsl missilesound_l via make_snd"
        );
        log.borrow_mut().clear();
        let _ = fire_hmissile1(&mut g, firer).expect("hm1");
        assert_eq!(
            *log.borrow(),
            vec![SndEvent::MakeSnd(PosSndFamilyId::Missile, 111, 222)]
        );
    }
}

#[cfg(test)]
mod ship3_flow_tests {
    use super::*;

    #[test]
    fn distant_ship3_branch_does_not_permanently_install_ship3b() {
        let mut g = Game::new();
        let player = g.objs.alloc().expect("player");
        let ship = g.objs.alloc().expect("ship");
        assert_eq!(player, 0);
        g.vars.internal_playpt = player as i16;
        g.vars.pviewvelz = 65;
        g.objs.aliens[player as usize].worldz = 0;
        g.objs.aliens[ship as usize].worldz = 5_000;

        ship3_istrat(&mut g, ship);
        let main = g.objs.aliens[ship as usize].stratptr;
        ship3_cont(&mut g, ship);
        assert_eq!(
            g.objs.aliens[ship as usize].stratptr, main,
            "the far-distance jump runs ship3b for one tick but must retain ship3_strat"
        );

        g.objs.aliens[player as usize].worldz = 3_500;
        ship3_strat(&mut g, ship);
        let approach = sid(&mut g, ship3a_strat);
        assert_eq!(g.objs.aliens[ship as usize].stratptr, Some(approach));

        g.objs.aliens[player as usize].worldz =
            g.objs.aliens[ship as usize].worldz.wrapping_sub(500);
        ship3a_strat(&mut g, ship);
        assert_ne!(g.vars.gameflags & GF_STRATDONE1, 0);
        let departure = g.objs.aliens[ship as usize]
            .stratptr
            .expect("the final approach installs its departure strategy");

        // Function addresses are not semantic identities in optimized Rust:
        // the compiler may merge equal functions or emit multiple copies of
        // one function. Prove that the installed handle performs ship3b's
        // exact locked-scroll movement instead of resolving the function by
        // its address a second time.
        let before = g.objs.aliens[ship as usize];
        g.objs.aliens[ship as usize].vx = 7;
        g.objs.aliens[ship as usize].vy = -9;
        g.objs.aliens[ship as usize].vz = 123;
        g.call_strat(departure, ship);
        let after = g.objs.aliens[ship as usize];
        assert_eq!(after.vx, 7);
        assert_eq!(after.vy, -9);
        assert_eq!(after.vz, -g.vars.pviewvelz);
        assert_eq!(after.worldx, before.worldx.wrapping_add(7));
        assert_eq!(after.worldy, before.worldy.wrapping_sub(9));
        assert_eq!(after.worldz, before.worldz);
    }
}
