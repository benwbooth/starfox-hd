//! Ground-artillery enemy family — RIIR port of Krister's tank strategies
//! (`reference/ultrastarfox/SF/STRAT/KSTRATS.ASM` tank1/tank1a/tank3 +
//! GA2STRAT.ASM tank2 / bazooka). ASM is the sole ground truth: none of
//! these have a C-oracle counterpart (`src/strat/strat_*.c` never ported the
//! tanks), so every cite below is to the 65816 source.
//!
//! ISTRATS.ASM def_Istrat rows (sf-map placement indices; `hard90yrfog` is a
//! real row at 183 — do not skip it when counting tanks):
//!   - `bazookaL`  = 158  (ISTRATS.ASM:587)  — level1_5 / route2 L5 / route3 L6
//!   - `bazookaR`  = 159  (ISTRATS.ASM:588)  — same maps
//!   - `tank2`     = 162  (ISTRATS.ASM:591)  — level1_4
//!   - `hard90yrfog` = 183 (ISTRATS.ASM:613) — enemy_a lane
//!   - `tank1a`    = 184  (ISTRATS.ASM:614)  — level1_4
//!   - `tank0`     = 185  (ISTRATS.ASM:615)  — dead content (no map placement)
//!   - `tank1`     = 186  (ISTRATS.ASM:616)  — dead content (no map placement)
//!   - `tank3`     = 187  (ISTRATS.ASM:617)  — level2_3
//!   - `houdai5f`  = 188  (ISTRATS.ASM:618)
//!
//! State machines (see per-fn cites):
//!   tank0   : wait(<2000z) then jump into tank1 `.forward` — KSTRATS.ASM:449-460
//!   tank1   : hangar roll-out -> chase -> forward/back (fire+fog) — :462-494
//!   tank1a  : wait(<5000z) -> chase(turn to 0) -> forward(fire, z+=17) /
//!             back(z-=7, terminal) — KSTRATS.ASM:418-458
//!   tank3   : wait(<1800z) -> forward -> back -> forwardb -> backb(idle);
//!             fires on the tank1fire gate each active state — KSTRATS.ASM:536-590
//!   tank2   : body backs up / turns / advances releasing 4 zaco_7 turret
//!             drones on an HP-less countdown; drones rise then chase+fire
//!             the player — GA2STRAT.ASM:1266-1408
//!   bazooka : rises from the planet, aims, lobs a 3-shot RELSLOWELASER
//!             burst, then flees up-and-away — GA2STRAT.ASM:1001-1082

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE1, ACF_COLLTYPE4, ACF_FIRSTFRAME, ACF_WEAPON, ASF_COLLDISABLE,
    ASF_COLLIDE, ASF_HITFLASH, ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATGND, ATLASER,
    ATMISSILE, ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, PosSndFamilyId, StrategyFn};
use sf_game::vars::{
    GF_STRATDONE1, GF_STRATDONE2, PSF2_PLAYERHP0, PSF_NOCTRL, PSF_NOFIRE, PSTF_INSEQ, SPFM_INSIDE,
    SPFM_TONORM,
};
use sf_game::world::{World, MAP_ISTRAT_SPINSPACEBAR};

use crate::common::{sv, StratRam};

use crate::common::{
    angle_xz, apply_velocity, dist_xz, gen_vecs_3d, make_obj, makesplash_srou, makessplash_srou,
    projectile_strat_ids, sf_random, spawn_projectile,
    strat_chase_proportional as chase_proportional, strat_gen_vecs_nvecs,
};
use crate::enemy_a::{
    achase_angle, add_player_z, addrnd2pos_xy, boss_attach_child_to_mother, boss_count_children,
    boss_find_child_obj, boss_get_mother_obj, copy_pos, fire_elaser, fire_fakefar_hmissile1,
    fire_hmissile1, fire_stb_hmissile1, hmissile1_strat, homingflat_strat, make_large_exp_obj,
    make_medium_exp_obj, make_xyvec, player, set_hard_vars, sid, speed_to, strat_aim_3d,
    strat_aim_yaw, strat_explode, strat_fire_relslowlaser, strat_fire_relslowlaserhome,
    strat_hit_flash, strat_move3d, strat_nocoll_init, strat_obj_index_or_null, strat_phase_offset,
    strat_pitch_toward, strat_relslowelaser_speed, AF_LEFT_PL, ASF2_NOEXPSND, ASF2_RELEXPLODE,
    ASF2_SMFLAG1, COLLTYPE_ENEMY1, COLLTYPE_ENEMYWEAP, COLLTYPE_ZENEMY, DEG11, DEG180, DEG45,
    DEG90,
};
use crate::snes_trig::rotate_16xz;

/// `Obj_GetPlayer` index (init_strats convention, game.rs:228): the
/// `internal_playpt` slot when active, else slot 0.
fn player_index(g: &Game) -> Option<u16> {
    let pp = g.vars.internal_playpt;
    if pp >= 0 && (pp as usize) < NUMBER_AL && g.objs.aliens[pp as usize].active {
        return Some(pp as u16);
    }
    if g.objs.aliens[0].active {
        return Some(0);
    }
    None
}

// ============================================================
// Constants (verbatim ASM equs).
// ============================================================
const TANK1_HP: u8 = 2; // KSTRATS.ASM:44 tank1HP
const TANK1_AP: u8 = 16; // KSTRATS.ASM:45 tank1AP
const TANK1_FIRERATE: u8 = 50; // KSTRATS.ASM:46 tank1firerate
const TANK2_HP: u8 = 40; // STRATEQU.INC:243 tank2HP
const TANK2_AP: u8 = 32; // STRATEQU.INC:244 tank2AP
const BAZOOKA_HP: u8 = 8; // STRATEQU.INC:237 bazookaHP
const BAZOOKA_AP: u8 = 16; // STRATEQU.INC:238 bazookaAP
const MEDPSPEED: u8 = 65; // STRATEQU.INC:347 medPspeed
const FOGDIST: i16 = 2000; // KSTRATS.ASM:58 fogdist
const DEG270: u8 = 192; // VARS.INC:18 deg270 = deg180+deg90
const SPACE_VIEWCY: i16 = -60; // STRATEQU.INC:494 space_viewCY

/// Hplasma projectile facts (GSTRATS.ASM:2517-2529 fire_Hplasma).
const HPLASMA_AP: u8 = 10; // STRATEQU.INC:87 HplasmaAP
const HPLASMA_SPEED: u8 = 60; // GSTRATS.ASM:2522 s_set_speed #60
const HPLASMA_LIFE: u8 = 50; // GSTRATS.ASM:2523 s_set_lifecnt #50
const RELSLOWELASER_AP: u8 = 2; // strat_enemy.c fire_relslowElaser enemylaserAP
const RELSLOWELASER_LIFE: u8 = 40;

/// ISTRATS.ASM def_Istrat indices (== sf-map placement indices).
const IS_BAZOOKAL: usize = 157;
const IS_EXIT: usize = 12;
const IS_BAZOOKAR: usize = 158;
const IS_TANK2: usize = 161;
const IS_TANK1A: usize = 183; // after hard90yrfog@182 (ISTRATS.ASM:613-614)
const IS_TANK0: usize = 184;
const IS_TANK1: usize = 185;
const IS_TANK3: usize = 186;
const IS_LEFTWALL: usize = 149;
const IS_SAUCER: usize = 226;
const IS_WARP: usize = 160;

/// zaco_7 turret drone shape (sf-map route2 rc.rs `SH_ZACO_7`).
const SH_ZACO_7: u16 = 128;

// ------------------------------------------------------------
// Mobile ground-enemy family (wireman / winglazerman / walking / uperm /
// rockhard). ISTRATS.ASM def_Istrat rows == sf-map placement indices
// (grep sf-map: all five are placed by ported maps — all reachable):
//   - walking       = 77  (ISTRATS.ASM:500)  — level1_4 / route3
//   - wireman       = 87  (ISTRATS.ASM:511)  — route2 rc
//   - winglazerman  = 90  (ISTRATS.ASM:514)  — level1_3/1_5 / route2 / route3
//   - uperM         = 159 (ISTRATS.ASM:589)  — level1_5 / route2 / route3
//   - rockhard      = 192 (ISTRATS.ASM:623)  — route2 / route3
// ASM is the sole ground truth (no C-oracle for these): GASTRATS.ASM
// (wireman:2446-2511, winglazerman:2811-2903), DSTRATS.ASM (walking:860-964),
// GA2STRAT.ASM (uperm:1112-1141), GSTRATS.ASM (rockhard:663-669).
const IS_WALKING: usize = 77;
const IS_WIREMAN: usize = 87;
const IS_WINGLAZERMAN: usize = 90;
const IS_UPERM: usize = 159;
const IS_ROCKHARD: usize = 192;
const IS_FLYPILLARS: usize = 73;
const IS_BASE_1: usize = 229;

// Space Armada / Sector-Z tunnel rows. These are ordinary ISTRATS.ASM rows,
// not the historical 0x0500xx placeholder addresses used by the removed C
// port. Every corresponding map placement must use this byte-sized ABI.
const IS_SHIP1A: usize = 69;
const IS_SHIP2: usize = 70;
const IS_SHIP3: usize = 71;
const IS_SHIP3A: usize = 72;
const IS_CORE1: usize = 110;
const IS_CORE0: usize = 111;
const IS_CRUISER2FIRE: usize = 130;
const IS_CRUISER2: usize = 131;
const IS_SDOOR1: usize = 133;
const IS_SDOOR2: usize = 134;
const IS_LENG0: usize = 135;
const IS_TOPRIGHT1: usize = 144;
const IS_TOPLEFT1: usize = 145;
const IS_BOTRIGHT1: usize = 146;
const IS_BOTLEFT1: usize = 147;
const IS_CRUISER1: usize = 152;
const IS_CRUISER1F: usize = 153;
const IS_WARKER3: usize = 154;
const IS_TWALL0: usize = 163;
const IS_MONOLITH: usize = 215;
const IS_EXITOPENSND2: usize = 230;
const IS_OPENLR: usize = 231;
const IS_UPDOOR: usize = 232;

const WIREMAN_HP: u8 = 4; // STRATEQU.INC wiremanHP
const WIREMAN_AP: u8 = 16; // wiremanAP
const WINGLAZERMAN_HP: u8 = 8; // winglazermanHP
const WINGLAZERMAN_AP: u8 = 16; // winglazermanAP
const WALKING_HP: u8 = 200; // DSTRATS.ASM:863 s_set_aldata #200
const WALKING_AP: u8 = 16; // walkingAP
const UPERM_HP: u8 = 2; // upermHP
const UPERM_AP: u8 = 8; // upermAP
const ROCKHARD_AP: u8 = 20; // rockhardAP
const HARDHP: u8 = 0xFF; // hardHP == -1 (indestructible obstacle)

const DEG22: u8 = 16; // deg360/16 (VARS.INC)
const DEG5: u8 = 4; // deg360/64

/// winglazerman wing-laser muzzle x, ROM `#±27>>weapon_scale` paired with
/// `s_fire_weapon`'s `<<weapon_scale` round-trip (GASTRATS.ASM:2872/2875,
/// weapon_scale=2): the low 2 bits are truncated -> -28 / +24 world.
const WL_MUZZLE_L: i16 = ((-27i16) >> 2) << 2;
const WL_MUZZLE_R: i16 = (27i16 >> 2) << 2;

/// al_hitflags leg/head zones (VARS.INC:167-169 HF1..HF3).
const HF1: u8 = 1;
const HF2: u8 = 2;
const HF3: u8 = 4;

// ============================================================
// Shared geometry / distance helpers (STRATMAC macro semantics).
// ============================================================

/// `jmp_distless`/`jmp_distmore` distance term (STRATMAC.INC:3403-3440):
/// `d = |world[player] - world[self]|` on one axis via a 16-bit sbc + nega.
/// Returns the 16-bit absolute delta as i32 (0..=32768).
fn abs_axis_dist(self_v: i16, player_v: i16) -> i32 {
    let d = player_v.wrapping_sub(self_v);
    (d as i32).abs()
}

/// `s_jmp_Zdistless x,y,#dist` — branch when `|dz| < dist` (rlbmi, strict).
fn zdist_less(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => abs_axis_dist(g.objs.aliens[idx as usize].worldz, p.worldz) < dist as i32,
        None => false,
    }
}

/// `s_jmp_Zdistmore x,y,#dist` — branch when `|dz| >= dist` (rlbpl, inclusive).
fn zdist_more(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => abs_axis_dist(g.objs.aliens[idx as usize].worldz, p.worldz) >= dist as i32,
        None => false,
    }
}

/// `s_jmp_Xdistmore x,y,#dist` — branch when `|dx| >= dist` (inclusive).
fn xdist_more(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => abs_axis_dist(g.objs.aliens[idx as usize].worldx, p.worldx) >= dist as i32,
        None => false,
    }
}

/// `s_jmp_Xdistless x,y,#dist` — branch when `|dx| < dist` (strict).
fn xdist_less(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => abs_axis_dist(g.objs.aliens[idx as usize].worldx, p.worldx) < dist as i32,
        None => false,
    }
}

/// `s_jmp_distmore` (xzdiffs rangexz ≥ dist).
fn xz_dist_more(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => dist_xz(&g.objs.aliens[idx as usize], &p) as i32 >= dist as i32,
        None => true,
    }
}

/// `s_jmp_notdelay N` (STRATMAC.INC:6456): fires when `gameframe & ((1<<N)-1) == 0`.
fn notdelay(g: &Game, bits: u16) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

/// `s_add_Roffs2pos B,y,x,x,offx,offy,offz,1,1,1,s,s,s` (STRATMAC.INC:4098):
/// rotate the local offset by the base's FULL rotation — rotz → rotx → roty
/// via ROM `rotate_8*` (`strat_roffs_full_i16`).
fn rotate_full_offset(base: &Alien, offx: i16, offy: i16, offz: i16) -> (i16, i16, i16) {
    crate::snes_trig::strat_roffs_full_i16(base.rotz, base.rotx, base.roty, offx, offy, offz)
}

/// Place `self` at `base + rotate_full_offset(...)` (the position half of
/// `s_do_childrelpos`; the caller copies rots separately).
fn full_offset_pos(g: &mut Game, self_idx: u16, base: &Alien, offx: i16, offy: i16, offz: i16) {
    let (rx, ry, rz) = rotate_full_offset(base, offx, offy, offz);
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(ry);
    al.worldz = base.worldz.wrapping_add(rz);
}

/// `s_add_Roffs2pos B,...,off,1,1,1,scale,scale,scale` — byte offs then ASL.
fn full_offset_pos_scaled(
    g: &mut Game,
    self_idx: u16,
    base: &Alien,
    offx: i8,
    offy: i8,
    offz: i8,
    scale: u32,
) {
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        base.rotz, base.rotx, base.roty, offx, offy, offz, scale,
    );
    let al = &mut g.objs.aliens[self_idx as usize];
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(ry);
    al.worldz = base.worldz.wrapping_add(rz);
}

/// `s_achase_alvar W,...,rate` (STRATMAC.INC sr16 achase): 16-bit chase with a
/// `nolessrange` pre-clamp (min |step| = 1<<shift), then `d >> shift`
/// (arithmetic, toward -inf). Byte-identical twin of bosses.rs
/// `amoeba_achase16` / mother.rs `achase16`.
fn achase_word(cur: i16, target: i16, shift: u32) -> i16 {
    let mut d = target as i32 - cur as i32;
    if d == 0 {
        return cur;
    }
    let min = 1i32 << shift;
    if d > -min && d < min {
        d = if d < 0 { -min } else { min };
    }
    cur.wrapping_add((d >> shift) as i16)
}

// ============================================================
// Shared tank helpers — tank1lr + tank1fire (KSTRATS.ASM:496-534).
// ============================================================

/// `tank1lr` (KSTRATS.ASM:496-514): sweep the aim ±20 units, dwelling 15
/// frames each way. `al_sbyte3` is the signed dwell counter (+15..0 -> right
/// sweep, -15..0 -> left sweep).
fn tank1lr(g: &mut Game, idx: u16) {
    let sb3 = g.objs.aliens[idx as usize].sbyte3 as i8;
    if sb3 >= 0 {
        // .dec: a = sbyte3 - 1
        let a = sb3 - 1;
        if a == 0 {
            // .right: sbyte3 = -15
            g.objs.aliens[idx as usize].sbyte3 = (-15i8) as u8;
        } else {
            g.objs.aliens[idx as usize].sbyte3 = a as u8;
            // s_achase_alvar B,x,al_roty,#20,3
            let mut roty = g.objs.aliens[idx as usize].roty;
            achase_angle(&mut roty, 20, 3);
            g.objs.aliens[idx as usize].roty = roty;
        }
    } else {
        // sbyte3 < 0: a = sbyte3 + 1
        let a = sb3 + 1;
        if a == 0 {
            // .left: sbyte3 = 15
            g.objs.aliens[idx as usize].sbyte3 = 15;
        } else {
            g.objs.aliens[idx as usize].sbyte3 = a as u8;
            // s_achase_alvar B,x,al_roty,#-20,3  (target 236 == -20)
            let mut roty = g.objs.aliens[idx as usize].roty;
            achase_angle(&mut roty, (-20i8) as u8, 3);
            g.objs.aliens[idx as usize].roty = roty;
        }
    }
}

/// `fire_Hplasma` (GSTRATS.ASM:2517-2529) spawned by `tank1fire`
/// (KSTRATS.ASM:522-534): a homing bouncy-plasma. Muzzle = `s_weapon_pos
/// #0,#-15,#0` (<<weapon_scale(2) => world -60 y) rotated by the firer, shot
/// rots = firer rots + `s_weapon_rot #0,#deg180` (gen_weapon .nrotobj adds the
/// offset, GSTRATS.ASM:2823-2824), and `al_ptr = playpt` -> homingflat homes.
fn fire_hplasma(g: &mut Game, idx: u16) {
    let Some(player_idx) = player_index(g) else {
        return;
    };
    let Some(shot) = make_obj(g, 0) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    // s_weapon_pos #0,#-15,#0 * weapon_scale(<<2) = (0,-60,0), rotated by firer.
    full_offset_pos(g, shot, &me, 0, -60, 0);
    let s_tick = sid(g, homingflat_strat);
    let (_gen_tick, s_coll) = projectile_strat_ids(g);
    // shot rot = firer rot + weapon_rot(#0,#deg180): rotx += 0, roty += deg180.
    let yaw = me.roty.wrapping_add(DEG180);
    let pitch = me.rotx;
    let al = &mut g.objs.aliens[shot as usize];
    al.shape = 0;
    al.sflags |= ASF_INVISIBLE;
    al.type_ |= ATLASER | ATZREMOVE;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 1;
    al.ap = HPLASMA_AP;
    al.vel = HPLASMA_SPEED;
    al.count = HPLASMA_LIFE;
    al.snd2 = 6;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    // s_set_alvar W,y,al_ptr,playpt: homingflat homes toward this target.
    al.fireobjptr = player_idx + 1;
    al.immuneptr = idx;
    al.sbyte1 = yaw;
    al.sbyte2 = pitch;
    al.roty = yaw;
    al.rotx = pitch;
    gen_vecs_3d(al);
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2528).
    g.hooks
        .make_snd(PosSndFamilyId::EnemyBattry, me.worldx, me.worldz);
}

/// `tank1fire` (KSTRATS.ASM:515-534): 3-shot cadence off a `tank1firerate`(50)
/// countdown in `al_sbyte2` — fires at counts 20, 10, and 0 (0 also reloads to
/// 50). Gated: no fire when `|dz| < 500` OR `|dx| >= 300`.
pub fn tank1fire(g: &mut Game, idx: u16) {
    // s_dec_alvar B,x,al_sbyte2
    let sb2 = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte2 = sb2;
    // Fire at counts 20 and 10; count 0 reloads and fires; other counts wait.
    let fire = if sb2 == 20 || sb2 == 10 {
        true
    } else if sb2 == 0 {
        g.objs.aliens[idx as usize].sbyte2 = TANK1_FIRERATE; // .fireset
        true
    } else {
        false
    };
    if !fire {
        return;
    }
    // s_jmp_Zdistless x,y,#500,.nofire ; s_jmp_Xdistmore x,y,#300,.nofire
    if zdist_less(g, idx, 500) || xdist_more(g, idx, 300) {
        return;
    }
    fire_hplasma(g, idx);
}

// ============================================================
// tank1a (IS 184) — KSTRATS.ASM:418-458.
// ============================================================

/// `tank1a_istrat` (KSTRATS.ASM:418-427): the persistent init/wait strategy.
/// Every tick sets base vars + faces deg270; when the player closes to within
/// 5000 z it hands off to `tank1a_strat` this same tick.
pub fn tank1a_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4; // set_sound2 x,#4
        al.hp = TANK1_HP; // s_set_aldata #tank1HP,#tank1AP
        al.ap = TANK1_AP;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.roty = DEG270; // s_set_alvar B,x,al_roty,#deg270
    }
    // s_jmp_Zdistless x,y,#5000,tank1a2_istrat
    if zdist_less(g, idx, 5000) {
        tank1a2_istrat(g, idx);
    }
    // else s_end_strat (stay in init, re-run next tick).
}

/// `tank1a2_istrat` (KSTRATS.ASM:429-434): one-shot handoff — wire tick/hit/
/// explode strats + fire timers, then fall into `tank1a_strat` the same tick.
pub fn tank1a2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tank1a_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte2 = TANK1_FIRERATE; // al_sbyte2 = 50
        al.sbyte3 = 15; // al_sbyte3 = 15 (left/right time)
    }
    tank1a_strat(g, idx);
}

/// `tank1a_strat` (KSTRATS.ASM:435-441): roll out of the hangar (move fwd
/// speed 30) then turn the aim to 0; on arrival advance to the fight loop.
pub fn tank1a_strat(g: &mut Game, idx: u16) {
    // s_move3d_obj x,#30,2,0
    strat_move3d(g, idx, 30, 2);
    // s_beqdec_alvar B,x,al_sbyte1,.chase — sbyte1 defaults 0 -> always .chase.
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 != 0 {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        return;
    }
    // .chase: s_achase_alvar B,x,al_roty,#0,2,.forward
    let mut roty = g.objs.aliens[idx as usize].roty;
    let reached = achase_angle(&mut roty, 0, 2);
    g.objs.aliens[idx as usize].roty = roty;
    if reached {
        // .forward: s_set_strat x,.goforward — fall in same tick.
        let go = sid(g, tank1a_goforward);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank1a_goforward(g, idx);
    }
}

/// `.goforward` (KSTRATS.ASM:444-451): close-quarters attack — sweep, fire,
/// press forward (speed 85) and creep z; retreat when the player pulls beyond
/// 3000 z.
fn tank1a_goforward(g: &mut Game, idx: u16) {
    if zdist_more(g, idx, 3000) {
        // .back: s_set_strat x,.goback — fall in same tick.
        let go = sid(g, tank1a_goback);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank1a_goback(g, idx);
        return;
    }
    tank1lr(g, idx);
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED + 20, 2); // s_move3d_obj x,#medpspeed+20,2,0
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(17); // s_add_alvar W,x,al_worldz,#17
}

/// `.goback` (KSTRATS.ASM:456-458): terminal retreat — z -= 7 forever.
fn tank1a_goback(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(7); // s_sub_alvar W,x,al_worldz,#7
}

// ============================================================
// tank0 (IS 184) / tank1 (IS 185) — KSTRATS.ASM:449-494.
// Dead content (no map placement) but ledger-complete for 100% coverage.
// ============================================================

/// `tank0_istrat` (KSTRATS.ASM:449-460): self-as-tick wait facing 0; when the
/// player closes to 2000 z, jump straight into `tank1_strat.forward` (skipping
/// the hangar roll-out). Otherwise reload fire timers each wait tick.
pub fn tank0_istrat(g: &mut Game, idx: u16) {
    let selfid = sid(g, tank0_istrat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4; // set_sound2 x,#4
                     // s_initfog — cosmetic no-op
        al.stratptr = Some(selfid);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = TANK1_HP;
        al.ap = TANK1_AP;
        al.roty = 0;
    }
    // s_jmp_Zdistless x,y,#2000,tank1_strat.forward
    if zdist_less(g, idx, 2000) {
        let go = sid(g, tank1_goforward);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank1_goforward(g, idx);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = TANK1_FIRERATE;
    al.sbyte3 = 15;
}

/// `tank1_istrat` (KSTRATS.ASM:462-466): wire tick/hit/explode + fire timers,
/// then fall into `tank1_strat` the same tick.
pub fn tank1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tank1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte2 = TANK1_FIRERATE;
        al.sbyte3 = 15;
    }
    tank1_strat(g, idx);
}

/// `tank1_strat` (KSTRATS.ASM:468-475): hangar roll-out (speed 30) then chase
/// roty→0; on arrival enter `.forward`.
pub fn tank1_strat(g: &mut Game, idx: u16) {
    strat_move3d(g, idx, 30, 2);
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 != 0 {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        return;
    }
    let mut roty = g.objs.aliens[idx as usize].roty;
    let reached = achase_angle(&mut roty, 0, 2);
    g.objs.aliens[idx as usize].roty = roty;
    if reached {
        let go = sid(g, tank1_goforward);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank1_goforward(g, idx);
    }
}

/// `.goforward` (KSTRATS.ASM:477-485): sweep+fire at medpspeed, z+=17; retreat
/// when the player pulls beyond 3000 z.
pub fn tank1_goforward(g: &mut Game, idx: u16) {
    if zdist_more(g, idx, 3000) {
        let go = sid(g, tank1_goback);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank1_goback(g, idx);
        return;
    }
    tank1lr(g, idx);
    // s_dofog — cosmetic no-op
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED, 2);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(17);
}

/// `.goback` (KSTRATS.ASM:487-494): keep sweeping/firing while retreating
/// (z-=7) at medpspeed — unlike tank1a's terminal idle retreat.
fn tank1_goback(g: &mut Game, idx: u16) {
    tank1lr(g, idx);
    // s_dofog — cosmetic no-op
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED, 2);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(7);
}

// ============================================================
// tank3 (IS 186) — KSTRATS.ASM:536-590.
// ============================================================

/// `tank3_istrat` (KSTRATS.ASM:536-547): init/wait — set strats immediately
/// (self as tick until close), face 0; when the player closes to 1800 z enter
/// the forward attack.
pub fn tank3_istrat(g: &mut Game, idx: u16) {
    let selfid = sid(g, tank3_istrat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4;
        al.stratptr = Some(selfid); // s_set_alptrs x,tank3_istrat,...
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = TANK1_HP;
        al.ap = TANK1_AP;
        al.roty = 0; // s_set_alvar B,x,al_roty,#0
    }
    if zdist_less(g, idx, 1800) {
        let go = sid(g, tank3_goforward);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank3_goforward(g, idx);
        return;
    }
    // still far: reload the fire timers each wait tick.
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = 11; // s_set_alvar B,x,al_sbyte2,#11
    al.sbyte3 = 15;
}

/// `.goforward` (KSTRATS.ASM:579-587): approach + fire, z += 25; back off when
/// beyond fogdist+100 (2100) z.
fn tank3_goforward(g: &mut Game, idx: u16) {
    if zdist_more(g, idx, FOGDIST + 100) {
        let go = sid(g, tank3_goback);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank3_goback(g, idx);
        return;
    }
    tank1lr(g, idx);
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED + 20, 2);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(25);
}

/// `.goback` (KSTRATS.ASM:589-598): retreat + fire (speed 45); re-approach
/// when back inside fogdist-40 (1960) z.
fn tank3_goback(g: &mut Game, idx: u16) {
    tank1lr(g, idx);
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED - 20, 2); // speed 45
    if zdist_less(g, idx, FOGDIST - 40) {
        let go = sid(g, tank3_goforwardb);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        tank3_goforwardb(g, idx);
    }
}

/// `.goforwardb` (KSTRATS.ASM:600-607): second approach; on pulling beyond
/// 2100 z enter the idle end state.
fn tank3_goforwardb(g: &mut Game, idx: u16) {
    if zdist_more(g, idx, FOGDIST + 100) {
        let go = sid(g, tank3_gobackb);
        g.objs.aliens[idx as usize].stratptr = Some(go);
        return; // .gobackb only runs fog (a no-op here) — nothing else to do.
    }
    tank1lr(g, idx);
    tank1fire(g, idx);
    strat_move3d(g, idx, MEDPSPEED + 20, 2);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(25);
}

/// `.gobackb` (KSTRATS.ASM:609-611): idle terminal (only `s_dofog`, a cosmetic
/// no-op in this port).
fn tank3_gobackb(_g: &mut Game, _idx: u16) {}

// ============================================================
// tank2 (IS 162) — GA2STRAT.ASM:1266-1408. Signed al_vel body + 4 zaco_7
// turret drones.
// ============================================================

/// Signed `s_speedto` (STRATMAC.INC SR_SPEEDTO with al_vel used as an i8, per
/// the `s_jmp_alvarPL al_vel` sign tests at GA2STRAT.ASM:1300/1311). Returns
/// true when at target after the step.
fn signed_speed_to(al: &mut Alien, target: i8, rate: i8) -> bool {
    let cur = al.vel as i8;
    if cur == target {
        return true;
    }
    let diff = (cur as i16 - target as i16).abs();
    if diff < rate as i16 {
        al.vel = target as u8;
    } else if cur > target {
        al.vel = (cur - rate) as u8;
    } else {
        al.vel = (cur + rate) as u8;
    }
    al.vel as i8 == target
}

/// `s_gen_vecs x,al_roty,al_vel` — ROM `nvecs_l` with signed `al_vel`
/// (tank2 body reverses). `strat_nvecs` already sign-extends the vel byte.
fn gen_vecs_2d_signed(al: &mut Alien) {
    strat_gen_vecs_nvecs(al);
}

/// `tank2_Istrat` (GA2STRAT.ASM:1266-1289): wire strats/data, face deg180,
/// speed 30, and spawn the four zaco_7 turret drones at their local offsets
/// with per-drone rise thresholds (`al_sword2`).
pub fn tank2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tank2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 4;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP; // enemy1 + enemyweap
        al.hp = TANK2_HP;
        al.ap = TANK2_AP;
        al.roty = DEG180;
        al.vel = 30; // s_set_speed x,#30
    }
    // Four drones: relpos (x,y,z) then al_sword2 rise threshold.
    tank2_spawn_child(g, idx, 1, -30, -35, 5, -140);
    tank2_spawn_child(g, idx, 2, 30, -35, 5, -140);
    tank2_spawn_child(g, idx, 3, -30, -35, -40, -200);
    tank2_spawn_child(g, idx, 4, 30, -35, -40, -200);
    // Falls through into tank2_strat the same tick (no s_end_strat, :1289-1291).
    tank2_strat(g, idx);
}

fn tank2_spawn_child(
    g: &mut Game,
    mother: u16,
    child_num: u8,
    rx: i8,
    ry: i8,
    rz: i8,
    sword2: i16,
) {
    // s_make_childobj #zaco_7,#N,tank2zaco_Istrat,enemy1
    let Some(child) = make_obj(g, SH_ZACO_7) else {
        return;
    };
    copy_pos(g, child, mother);
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return;
    }
    tank2zaco_istrat(g, child);
    let al = &mut g.objs.aliens[child as usize];
    al.collflags |= COLLTYPE_ENEMY1; // make_childobj colltype arg
                                     // s_set_relpos y,#rx,#ry,#rz
    al.relposx = rx as u8;
    al.relposy = ry as u8;
    al.relposz = rz as u8;
    // s_set_alvar W,y,al_sword2,#threshold
    al.sword2 = sword2;
}

/// `tank2_strat` (GA2STRAT.ASM:1291-1348): 4-state drive + an HP-less child
/// release countdown that runs after the per-state logic in every state.
pub fn tank2_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        // state 0: approach — reload countdown, advance when within 1000 z.
        0 => {
            g.objs.aliens[idx as usize].sbyte2 = 100;
            if zdist_less(g, idx, 1000) {
                next_state(g, idx);
                return;
            }
        }
        // state 1: back up (speed -> -20) + slow left turn, then state 2.
        1 => {
            if signed_speed_to(&mut g.objs.aliens[idx as usize], -20, 1) {
                next_state(g, idx);
                return;
            }
            if (g.objs.aliens[idx as usize].vel as i8) < 0 {
                // s_sub_alvar B,x,al_roty,#1
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = al.roty.wrapping_sub(1);
                // s_jmp_frameMORE 15,31,.nback -> extra +2 when (gf&31) >= 15.
                if (g.vars.gameframe & 31) >= 15 {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.roty = al.roty.wrapping_add(2);
                }
            }
        }
        // state 2: drive forward (speed -> +20) + turn, then state 3.
        2 => {
            if signed_speed_to(&mut g.objs.aliens[idx as usize], 20, 1) {
                next_state(g, idx);
                return;
            }
            if (g.objs.aliens[idx as usize].vel as i8) < 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = al.roty.wrapping_sub(1);
                if (g.vars.gameframe & 31) >= 15 {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.roty = al.roty.wrapping_add(2);
                }
            }
        }
        // state 3: settle heading to deg180+deg45, speed -> 30.
        _ => {
            if g.objs.aliens[idx as usize].roty != DEG180.wrapping_add(DEG45) {
                let al = &mut g.objs.aliens[idx as usize];
                al.roty = al.roty.wrapping_add(1);
                let _ = signed_speed_to(al, 30, 1);
            }
        }
    }

    // .ngo (GA2STRAT.ASM:1328-1345): child-release countdown, all states.
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        // .ninc: skip the release checks (s_beqdec branches on 0 pre-dec).
    } else {
        let sb2 = sb2 - 1;
        g.objs.aliens[idx as usize].sbyte2 = sb2;
        // At 90/70/50/30 wake child 1/2/3/4 into state 1 (rise).
        if sb2 == 90 {
            tank2_set_childstate(g, idx, 1, 1);
        }
        if sb2 == 70 {
            tank2_set_childstate(g, idx, 2, 1);
        }
        if sb2 == 50 {
            tank2_set_childstate(g, idx, 3, 1);
        }
        if sb2 == 30 {
            tank2_set_childstate(g, idx, 4, 1);
        }
    }
    // s_gen_vecs / s_add_vecs2pos / s_add_playerZ
    gen_vecs_2d_signed(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `s_set_childstate #N,#state` (STRATMAC.INC:7028): put child N into `state`.
fn tank2_set_childstate(g: &mut Game, mother: u16, child_num: u8, state: u8) {
    if let Some(child) = boss_find_child_obj(g, mother, child_num) {
        g.objs.aliens[child as usize].stratstate = state;
    }
}

/// `tank2zaco_Istrat` (GA2STRAT.ASM:1350-1356): a turret drone — hp4/ap8,
/// enemyweap collide, anim 0. (`makeengine_srou` engine flame is cosmetic and
/// omitted.)
pub fn tank2zaco_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tank2zaco_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = 4;
    al.ap = 8;
    al.animframe = 0;
    al.collflags |= COLLTYPE_ENEMYWEAP;
}

/// `tank2zaco_strat` (GA2STRAT.ASM:1357-1408): state 0 pinned to the mother,
/// state 1 rises, state 2 detaches and chases + fires the player.
pub fn tank2zaco_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        // state 0: follow the mother at the rotated relpos (or explode if gone).
        0 => {
            match boss_get_mother_obj(g, idx) {
                Some(m) => {
                    let mother = g.objs.aliens[m as usize];
                    // s_do_childrelpos x,1: relpos<<1, rotated by mother, + copy rots.
                    let ox = (g.objs.aliens[idx as usize].relposx as i8 as i16) << 1;
                    let oy = (g.objs.aliens[idx as usize].relposy as i8 as i16) << 1;
                    let oz = (g.objs.aliens[idx as usize].relposz as i8 as i16) << 1;
                    full_offset_pos(g, idx, &mother, ox, oy, oz);
                    let al = &mut g.objs.aliens[idx as usize];
                    al.rotx = mother.rotx;
                    al.roty = mother.roty;
                    al.rotz = mother.rotz;
                }
                None => {
                    // s_set_strat x,explode_Istrat
                    strat_explode(g, idx);
                }
            }
            return; // s_brl .end (no gen_vecs / no playerZ)
        }
        // state 1: rise; when above the sword2 threshold advance to fire.
        1 => {
            // s_jmp_higheralvar x,al_sword2,nextstate -> worldy < sword2.
            let al = g.objs.aliens[idx as usize];
            if al.worldy < al.sword2 {
                next_state(g, idx);
                return;
            }
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = al.worldy.wrapping_add((-5i16) as i16); // rise
                                                                // s_cmp_anim #9 beq .endrel ; s_add_anim x,#1,#10
            if al.animframe != 9 {
                al.animframe = (al.animframe + 1) % 10;
            }
            add_player_z(g, idx); // .endrel
            return;
        }
        // state 2: detach + chase + fire.
        _ => {
            // s_speedto x,#30,1 (positive vel -> shared move path is fine)
            let al = &mut g.objs.aliens[idx as usize];
            if al.vel < 30 {
                al.vel += 1;
            } else if al.vel > 30 {
                al.vel -= 1;
            }
            if !zdist_less(g, idx, 500) {
                if let Some(pl) = player(g) {
                    // s_obj2obj_3dangle x,y,al_roty,al_rotx,2
                    strat_aim_3d(g, idx, &pl, 2);
                    // s_jmp_notdelay 2,.ngo -> fire when (gf&3)==0.
                    if notdelay(g, 2) {
                        tank2zaco_fire(g, idx, &pl);
                    }
                }
            }
        }
    }
    // .ngo: s_gen_3dvecs / s_add_vecs2pos / s_add_playerZ (states 1-fallthrough,2).
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `s_weapon_rndrots2obj y,7,7` + `s_fire_weapon x,RELSLOWELASER`
/// (GA2STRAT.ASM:1399-1401): aim at the player with a ±3 per-axis spread. Draw
/// order pitch-then-yaw (s_weapon_rndrot, STRATMAC.INC:2104-2107).
fn tank2zaco_fire(g: &mut Game, idx: u16, player_al: &Alien) {
    let me = g.objs.aliens[idx as usize];
    let base_yaw = angle_xz(&me, player_al);
    let base_pitch = strat_pitch_toward(&me, player_al);
    let dpitch = ((sf_random(&mut g.vars) as u8 & 7) as i8).wrapping_sub(3);
    let dyaw = ((sf_random(&mut g.vars) as u8 & 7) as i8).wrapping_sub(3);
    let pitch = base_pitch.wrapping_add(dpitch as u8);
    let yaw = base_yaw.wrapping_add(dyaw as u8);
    strat_fire_relslowlaser(g, idx, pitch, yaw);
}

// ============================================================
// bazooka L/R (IS 158/159) — GA2STRAT.ASM:1001-1082.
// ============================================================

/// sflags2 sflag1 bit (STRATEQU.INC:914): the L/R turn-direction latch.
const ASF2_SFLAG1: u8 = 0x10;

/// ROM `bazooka1L_Istrat` / `bazookaL_Istrat` (GA2STRAT.ASM:991/1001): L sets
/// sflag1 (turn left in the fire state); both share `bazooka_Icont`.
pub fn bazooka1l_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    bazooka_icont(g, idx);
}

/// ROM `bazooka1R_Istrat` / `bazookaR_Istrat` (GA2STRAT.ASM:994/1004).
pub fn bazooka1r_istrat(g: &mut Game, idx: u16) {
    bazooka_icont(g, idx);
}

/// Compatibility aliases for the istrat table (IS 158/159).
fn bazookal_init(g: &mut Game, idx: u16) {
    bazooka1l_istrat(g, idx);
}
fn bazookar_init(g: &mut Game, idx: u16) {
    bazooka1r_istrat(g, idx);
}

/// `bazooka_Icont` (GA2STRAT.ASM:1008-1018): common init — rise up from the
/// planet with vy=-15, pitched straight up, facing deg180, speed 80.
fn bazooka_icont(g: &mut Game, idx: u16) {
    let tick = sid(g, bazooka_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, bazexp_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.collflags |= COLLTYPE_ENEMY1;
    al.hp = BAZOOKA_HP;
    al.ap = BAZOOKA_AP;
    al.animframe = 0;
    al.vy = -15; // s_set_alvar W,x,al_vy,#-15 (overwritten each tick by gen_3dvecs)
    al.rotx = (-(DEG90 as i8)) as u8; // s_set_alvar B,x,al_rotx,#-deg90 (== 192)
    al.roty = DEG180;
    al.vel = 80; // s_set_speed x,#80
    al.snd2 = 3;
    al.stratstate = 0;
    // Falls through into bazooka_strat the same tick (no s_end_strat, :1017-1019).
    bazooka_strat(g, idx);
}

/// `bazooka_strat` (GA2STRAT.ASM:1019-1082): 4-state pop-up artillery.
fn bazooka_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;

    // s_jmp_ifstate 0/3 skip; states 1,2 chase worldy toward player_posy rate2.
    if state == 1 || state == 2 {
        let py = g.vars.player_posy;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = achase_word(al.worldy, py, 2);
    }

    match state {
        // state 0: rise; once high enough (worldy < 440) level the pitch to 0.
        0 => {
            // s_jmp_lower x,#space_viewCY+500,.nsup -> worldy >= 440 skips.
            if g.objs.aliens[idx as usize].worldy < SPACE_VIEWCY + 500 {
                // s_speedto x,#0,1
                let al = &mut g.objs.aliens[idx as usize];
                if al.vel > 0 {
                    al.vel -= 1;
                }
                // s_achase_alvar B,x,al_rotx,#0,3,nextstate
                let mut rotx = g.objs.aliens[idx as usize].rotx;
                let reached = achase_angle(&mut rotx, 0, 3);
                g.objs.aliens[idx as usize].rotx = rotx;
                if reached {
                    next_state(g, idx);
                    return bazooka_strat(g, idx);
                }
            }
        }
        // state 1: brake + aim yaw at the player with a ±deg45/2 lead offset.
        1 => {
            let al = &mut g.objs.aliens[idx as usize];
            if al.vel > 0 {
                al.vel -= 1;
            }
            al.sbyte1 = DEG90; // turn counter for state 2
                               // s_set_var svar_byte1,#-deg45/2 ; if !sflag1 -> +deg45/2
            let off: i8 = if al.sflags2 & ASF2_SFLAG1 != 0 {
                -((DEG45 / 2) as i8)
            } else {
                (DEG45 / 2) as i8
            };
            // s_obj2obj_angleOFF x,y,al_roty,off,2,nextstate (STRATMAC.INC:2233):
            // subtract off from roty, aim at player (chase rate 2), re-add off;
            // when the achase reports "reached" advance to fire.
            if let Some(pl) = player(g) {
                let base = g.objs.aliens[idx as usize].roty.wrapping_sub(off as u8);
                let target = angle_xz(&g.objs.aliens[idx as usize], &pl);
                let mut cur = base;
                let reached = achase_angle(&mut cur, target, 2);
                g.objs.aliens[idx as usize].roty = cur.wrapping_add(off as u8);
                if reached {
                    next_state(g, idx);
                    return bazooka_strat(g, idx);
                }
            }
        }
        // state 2: fire a 3-frame RELSLOWELASER burst while slowly turning.
        2 => {
            let al = &mut g.objs.aliens[idx as usize];
            // s_speedto x,#0,2
            if al.vel >= 2 {
                al.vel -= 2;
            } else {
                al.vel = 0;
            }
            // s_beqdec_alvar B,x,al_sbyte1,nextstate
            if al.sbyte1 == 0 {
                next_state(g, idx);
                return bazooka_strat(g, idx);
            }
            al.sbyte1 -= 1;
            // sflag1 -> turn left (roty+2) else right (roty-2).
            if al.sflags2 & ASF2_SFLAG1 != 0 {
                al.roty = al.roty.wrapping_add(2);
            } else {
                al.roty = al.roty.wrapping_sub(2);
            }
            // s_add_anim x,#1,#3 ; anim 0 -> no fire, 2 -> left muzzle else right.
            al.animframe = (al.animframe + 1) % 3;
            let anim = al.animframe;
            if anim != 0 {
                // muzzle x = ±(30<<2) world (>>weapon_scale then <<weapon_scale).
                let mx: i16 = if anim == 2 { -(30 << 2) } else { 30 << 2 };
                bazooka_fire(g, idx, mx);
            }
        }
        // state 3: flee up and away (speed 80, level to deg0 / pitch up deg45).
        _ => {
            let al = &mut g.objs.aliens[idx as usize];
            // s_speedto x,#80,4
            if al.vel < 80 {
                al.vel = (al.vel + 4).min(80);
            } else if al.vel > 80 {
                al.vel -= 4;
            }
            let mut roty = g.objs.aliens[idx as usize].roty;
            achase_angle(&mut roty, 0, 3);
            g.objs.aliens[idx as usize].roty = roty;
            let mut rotx = g.objs.aliens[idx as usize].rotx;
            achase_angle(&mut rotx, DEG45, 3);
            g.objs.aliens[idx as usize].rotx = rotx;
        }
    }

    // .end: s_gen_3dvecs / s_add_vecs2pos / s_add_playerZ.
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `s_weapon_rndrot 3,0` + `s_fire_weapon x,RELSLOWELASER` (GA2STRAT.ASM:1074):
/// yaw = firer roty (aimed), pitch = firer rotx + (rnd&3)-1, muzzle x = ±120
/// rotated by the firer.
fn bazooka_fire(g: &mut Game, idx: u16, muzzle_x: i16) {
    let me = g.objs.aliens[idx as usize];
    let dpitch = ((sf_random(&mut g.vars) as u8 & 3) as i8).wrapping_sub(1);
    let pitch = me.rotx.wrapping_add(dpitch as u8);
    let yaw = me.roty;
    // Muzzle offset rotated by the firer, added to its position via the
    // projectile's own spawn (spawn adds the offset to the owner pos).
    let (dx, dy, dz) = rotate_full_offset(&me, muzzle_x, 0, 0);
    let speed = crate::enemy_a::strat_relslowelaser_speed(g);
    let _ = crate::common::spawn_projectile(
        g,
        Some(idx),
        dx,
        dy,
        dz,
        pitch,
        yaw,
        speed,
        RELSLOWELASER_LIFE,
        RELSLOWELASER_AP,
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    );
    // ROM `jsl lasersound_l` via fire_relslowElaser (GSTRATS.ASM:2559).
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
}

/// `bazexp_Istrat` (GA2STRAT.ASM:1055-1063): on death drop a falling debris
/// object (the barrel) then run the standard escapee explosion.
pub fn bazexp_istrat(g: &mut Game, idx: u16) {
    // ROM makes `al_sword1` debris; that shape is cosmetic here, so spawn a
    // bare falling object at the bazooka's pose then explode the bazooka.
    if let Some(child) = make_obj(g, 0) {
        copy_pos(g, child, idx);
        let src = g.objs.aliens[idx as usize];
        let tick = sid(g, bazfall_strat);
        let al = &mut g.objs.aliens[child as usize];
        al.rotx = src.rotx;
        al.roty = src.roty;
        al.rotz = src.rotz;
        // bazfall_Istrat init (GA2STRAT.ASM:1065-1070).
        al.count = 30; // s_set_lifecnt #30
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE; // colldisable
    }
    // s_jmp escapeeexplode_Istrat -> standard explosion.
    strat_explode(g, idx);
}

/// ROM `bazfall_Istrat` (GA2STRAT.ASM:1065).
pub fn bazfall_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bazfall_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 30;
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
    }
}

/// `bazfall_strat` (GA2STRAT.ASM:1071-1082): tumble + fall under gravity for
/// 30 frames then remove.
pub fn bazfall_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(16); // s_add_alvar B,x,al_roty,#16
        al.vy = al.vy.wrapping_add(2); // s_add_alvar W,x,al_vy,#2 (gravity)
    }
    // (s_make_smoke is cosmetic — omitted.)
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos
    add_player_z(g, idx); // s_add_playerz
    let al = &mut g.objs.aliens[idx as usize];
    if al.count > 0 {
        al.count -= 1;
    }
    if al.count == 0 {
        g.objs.aldead = 1;
    }
}

// ============================================================
// Shared small helpers for the mobile family.
// ============================================================

/// `s_jmp_random label` with no factor (STRATMAC.INC:1407-1417): 50% coin —
/// branch when `random_l() < 127`. Draws (advances the RNG) per the codebase's
/// `mt_jmp_random` convention.
fn jmp_random50(g: &mut Game) -> bool {
    (sf_random(&mut g.vars) & 0xff) < 127
}

/// `s_kill_obj` (STRATMAC.INC): hp=0 + colldisable — the engine's death sweep
/// then routes the object through its expstrat.
fn kill_obj(al: &mut Alien) {
    al.hp = 0;
    al.sflags |= ASF_COLLDISABLE;
}

/// `s_make_obj #explosion,... ; s_add_Roffs2pos ... ; s_set_alptrs
/// y,explode,explode,explode ; s_kill_obj y` — drop a self-exploding effect
/// object at `base + rotate_full_offset(off)`. The explosion/nullshape mesh id
/// is cosmetic (not resolvable from ported map data); shape 0 stands in.
fn spawn_explosion_at(g: &mut Game, base_idx: u16, ox: i16, oy: i16, oz: i16) {
    let Some(child) = make_obj(g, 0) else {
        return;
    };
    let base = g.objs.aliens[base_idx as usize];
    full_offset_pos(g, child, &base, ox, oy, oz);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[child as usize];
    al.stratptr = Some(exp);
    al.collstratptr = Some(exp);
    al.expstratptr = Some(exp);
    kill_obj(al);
}

/// `s_jmp_distless x,y,#dist` (STRATMAC.INC:3295): XZ magnitude (`xzdiffs_l`
/// -> `rangexz` == `strat_dist_xz`) strictly less than `dist`.
fn xzdist_less(g: &Game, idx: u16, dist: i16) -> bool {
    match player(g) {
        Some(p) => dist_xz(&g.objs.aliens[idx as usize], &p) < dist,
        None => false,
    }
}

/// `s_jmp_lower x,#h` (STRATMAC.INC:3098, rlbpl): branch when `worldy >= h`.
fn worldy_ge(g: &Game, idx: u16, h: i16) -> bool {
    g.objs.aliens[idx as usize].worldy >= h
}

/// `s_jmp_outZdistrng x,y,#min,#max` (STRATMAC.INC:3315-3339): out-of-range
/// when `|dz| < min` OR `|dz| >= max` (in-range = `min <= |dz| < max`).
fn out_zdist_rng(g: &Game, idx: u16, min: i16, max: i16) -> bool {
    match player(g) {
        Some(p) => {
            let d = abs_axis_dist(g.objs.aliens[idx as usize].worldz, p.worldz);
            d < min as i32 || d >= max as i32
        }
        None => true,
    }
}

// ============================================================
// wireman (IS 88) — GASTRATS.ASM:2446-2511. A wire/lash flyer: homes the
// player, and inside 1500 XZ throws a random evasive roll (pitch-down /
// yaw+roll L or R) for deg180/4 frames; if it sinks to the ground (worldy>=0)
// it pops back up.
// ============================================================

/// `wireman_Istrat` (GASTRATS.ASM:2446-2453): one-time data + shadow, then fall
/// into `wireman_init` (alptrs + speed) and `wireman_strat` the same tick.
fn wireman_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = WIREMAN_HP; // s_set_aldata #wiremanHP,#wiremanAP
        al.ap = WIREMAN_AP;
        al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow
    }
    wireman_init(g, idx);
}

/// `wireman_init` (GASTRATS.ASM:2451-2453): (re)wire strats + speed, then run
/// `wireman_strat`. Re-entered from the dodge/pop-up timers to resume chase.
fn wireman_init(g: &mut Game, idx: u16) {
    let tick = sid(g, wireman_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, wiremandie_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,wireman_strat,hitflash,wiremandie
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.vel = 40; // s_set_speed x,#40
    }
    wireman_strat(g, idx);
}

/// `wireman_strat` (GASTRATS.ASM:2454-2458): aim 3D at the player (rate 2); on
/// closing to 1500 XZ start an evasive roll.
fn wireman_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        strat_aim_3d(g, idx, &pl, 2); // s_obj2obj_3dangle x,y,al_roty,al_rotx,2
    }
    if xzdist_less(g, idx, 1500) {
        wireman2_init(g, idx); // s_jmp_distless x,y,#1500,wireman2_init
    } else {
        wireman_cont(g, idx);
    }
}

/// `wireman_cont` (GASTRATS.ASM:2460-2461): pop up if grounded, else move.
fn wireman_cont(g: &mut Game, idx: u16) {
    if worldy_ge(g, idx, 0) {
        wiremanup_init(g, idx); // s_jmp_lower x,#0,wiremanup_init
    } else {
        wireman_cont2(g, idx);
    }
}

/// `wireman_cont2` (GASTRATS.ASM:2462-2466): gen 3D vecs, move, scroll.
pub fn wireman_cont2(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `wireman2_init` (GASTRATS.ASM:2467-2474): pick an evasive branch. Two coin
/// flips: 50% X (pitch), then 25%/25% between the YL and YR rolls. NOTE the ROM
/// quirk at :2472-2474 — the middle branch sets `stratptr = YR` but jumps to the
/// `YL` code THIS tick, so the first evasive tick always runs the YL motion and
/// only the *next* tick runs YR. Replicated faithfully.
pub fn wireman2_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = DEG180 / 4; // #deg180/4 == 32 (dodge time)
    let x = sid(g, wireman2x_strat);
    let yr = sid(g, wireman2yr_strat);
    let yl = sid(g, wireman2yl_strat);
    g.objs.aliens[idx as usize].stratptr = Some(x); // s_set_strat x,wireman2X_strat
    if jmp_random50(g) {
        // s_jmp_random wireman2X_strat
        wireman2x_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].stratptr = Some(yr); // s_set_strat x,wireman2YR_strat
    if jmp_random50(g) {
        // s_jmp_random wireman2YL_strat (ptr stays YR — the quirk).
        wireman2yl_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].stratptr = Some(yl); // s_set_strat x,wireman2YL_strat
    wireman2yl_strat(g, idx);
}

/// `wireman2YL_strat` (GASTRATS.ASM:2476-2481): yaw+roll left while dodging.
pub fn wireman2yl_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4); // s_add_alvar al_roty,#4
        al.rotz = al.rotz.wrapping_add(4); // s_add_alvar al_rotz,#4
    }
    wireman2_countdown(g, idx);
}

/// `wireman2YR_strat` (GASTRATS.ASM:2482-2487): yaw+roll right while dodging.
pub fn wireman2yr_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_sub(4);
        al.rotz = al.rotz.wrapping_sub(4);
    }
    wireman2_countdown(g, idx);
}

/// `wireman2X_strat` (GASTRATS.ASM:2488-2492): pitch down while dodging.
pub fn wireman2x_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(4);
    wireman2_countdown(g, idx);
}

/// Shared dodge countdown tail (`s_beqdec_alvar al_sbyte1,wireman_init ;
/// s_brl wireman_cont`): when sbyte1 hits 0 resume chase, else keep moving.
fn wireman2_countdown(g: &mut Game, idx: u16) {
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        wireman_init(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        wireman_cont(g, idx);
    }
}

/// `wiremanup_init` (GASTRATS.ASM:2494-2498): reset to ground level, pitch up a
/// touch, and climb for 30 frames.
pub fn wiremanup_init(g: &mut Game, idx: u16) {
    let tick = sid(g, wiremanup_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.worldy = 0; // s_set_alvar W,x,al_worldy,#0
    al.rotx = DEG22.wrapping_neg(); // s_set_alvar B,x,al_rotx,#-deg22 (240)
    al.sbyte1 = 30; // s_set_alvar B,x,al_sbyte1,#30
    wiremanup_strat(g, idx);
}

/// `wiremanup_strat` (GASTRATS.ASM:2499-2502): climb; on timeout resume chase.
/// Uses `wireman_cont2` (NOT `wireman_cont`) so the worldy>=0 pop-up gate does
/// not immediately re-trigger.
pub fn wiremanup_strat(g: &mut Game, idx: u16) {
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        wireman_init(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        wireman_cont2(g, idx);
    }
}

/// `wiremandie_Istrat` (GASTRATS.ASM:2505-2511): drops an `item_6` powerup then
/// explodes.
pub fn wiremandie_istrat(g: &mut Game, idx: u16) {
    wiremandie_strat(g, idx);
}

fn wiremandie_strat(g: &mut Game, idx: u16) {
    if let Some(drop) = make_obj(g, 0) {
        let (px, py, pz) = {
            let me = &g.objs.aliens[idx as usize];
            (me.worldx, me.worldy, me.worldz)
        };
        {
            let al = &mut g.objs.aliens[drop as usize];
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
        }
        item6_istrat(g, drop);
    }
    strat_explode(g, idx);
}

// ============================================================
// winglazerman (IS 91) — GASTRATS.ASM:2811-2903. Wing-laser strafer: homes in,
// spins twice (deg360/4 each), firing paired wing lasers in between; if it
// survives both cycles it flees ("go"). Death drops a laser/beam powerup.
// ============================================================

/// `winglazerman_Istrat` (GASTRATS.ASM:2811-2817): wire strats/data, then run
/// `winglazerman_strat`.
fn winglazerman_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, winglazerman_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, winglazermandie_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WINGLAZERMAN_HP; // s_set_aldata #winglazermanHP,#winglazermanAP
        al.ap = WINGLAZERMAN_AP;
        al.vel = 40; // s_set_speed x,#40
        al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow
        al.sbyte2 = 2; // s_set_alvar al_sbyte2,#2 (spin-cycle budget)
    }
    winglazerman_strat(g, idx);
}

/// `winglazerman_strat` (GASTRATS.ASM:2818-2824): aim 3D at the player; on
/// closing to 1000 Z begin the spin/fire routine.
fn winglazerman_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        strat_aim_3d(g, idx, &pl, 2);
    }
    if zdist_less(g, idx, 1000) {
        winglazerman2_init(g, idx); // s_jmp_Zdistless x,y,#1000,winglazerman2_init
    } else {
        winglazerman_cont(g, idx);
    }
}

/// `winglazerman_cont` (GASTRATS.ASM:2826-2830): gen 3D vecs, move, scroll.
fn winglazerman_cont(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `winglazerman2_init` (GASTRATS.ASM:2844-2848): if the spin budget is spent
/// flee ("go"); else start a full-turn spin.
pub fn winglazerman2_init(g: &mut Game, idx: u16) {
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        winglazermango_init(g, idx); // s_beqdec_alvar al_sbyte2,winglazermango_init
        return;
    }
    let tick = sid(g, winglazerman2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = sb2 - 1;
    al.stratptr = Some(tick);
    al.sbyte1 = DEG90; // s_set_alvar al_sbyte1,#deg360/4 (== 64)
    winglazerman2_strat(g, idx);
}

/// `winglazerman2_strat` (GASTRATS.ASM:2848-2855): slow to 20 while spinning
/// (roty+=4); after a full turn drop into the fire routine.
pub fn winglazerman2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let _ = speed_to(al, 20, 1); // s_speedto x,#20,1
        al.roty = al.roty.wrapping_add(4); // s_add_alvar al_roty,#4
    }
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        winglazerman3_init(g, idx); // s_beqdec_alvar al_sbyte1,winglazerman3_init
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        winglazerman_cont(g, idx);
    }
}

/// `winglazerman3_init` (GASTRATS.ASM:2857-2860): brake to 0 and fire wing
/// lasers for 30 frames.
pub fn winglazerman3_init(g: &mut Game, idx: u16) {
    let tick = sid(g, winglazerman3_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = 30; // s_set_alvar al_sbyte1,#30 (fire window)
    al.sbyte3 = 5; // s_set_alvar al_sbyte3,#5 (pitch spread, narrows per shot)
    winglazerman3_strat(g, idx);
}

/// `winglazerman3_strat` (GASTRATS.ASM:2861-2882): brake, aim, and on the
/// notdelay-2 gate fire two RELSLOWELASERs from the wings; after the window
/// loops back to another spin.
pub fn winglazerman3_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 0, 1); // s_speedto x,#0,1
    if let Some(pl) = player(g) {
        strat_aim_3d(g, idx, &pl, 2); // s_obj2obj_3dangle x,y,al_roty,al_rotx,2
    }
    if notdelay(g, 2) {
        // s_jmp_notdelay 2,.nfire -> fire every 4 frames.
        winglazerman_fire(g, idx);
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte3 = al.sbyte3.wrapping_sub(1); // s_sub_alvar al_sbyte3,#1
    }
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        winglazerman2_init(g, idx); // s_beqdec_alvar al_sbyte1,winglazerman2_init
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        winglazerman_cont(g, idx);
    }
}

/// Two-shot wing volley (GASTRATS.ASM:2870-2877): pitch = firer rotx + sbyte3
/// (svar_byte1), yaw = firer roty ± deg5, muzzles at ±27 (weapon-scale
/// round-trip) rotated by the firer.
fn winglazerman_fire(g: &mut Game, idx: u16) {
    let me = g.objs.aliens[idx as usize];
    let pitch = me.rotx.wrapping_add(me.sbyte3); // s_weapon_rot svar_byte1(=sbyte3),...
    let speed = crate::enemy_a::strat_relslowelaser_speed(g);
    // Left wing: yaw += deg5.
    let (dx, dy, dz) = rotate_full_offset(&me, WL_MUZZLE_L, 0, 0);
    let _ = spawn_projectile(
        g,
        Some(idx),
        dx,
        dy,
        dz,
        pitch,
        me.roty.wrapping_add(DEG5),
        speed,
        RELSLOWELASER_LIFE,
        RELSLOWELASER_AP,
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    );
    // Right wing: yaw -= deg5.
    let (dx, dy, dz) = rotate_full_offset(&me, WL_MUZZLE_R, 0, 0);
    let _ = spawn_projectile(
        g,
        Some(idx),
        dx,
        dy,
        dz,
        pitch,
        me.roty.wrapping_sub(DEG5),
        speed,
        RELSLOWELASER_LIFE,
        RELSLOWELASER_AP,
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    );
    // ROM `s_fire_weapon x,RELSLOWELASER` ×2 → gen_weapon `jsl lasersound_l` each.
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
}

/// `winglazermango_init` (GASTRATS.ASM:2832-2834): flee state — 100-frame
/// lifetime.
pub fn winglazermango_init(g: &mut Game, idx: u16) {
    let tick = sid(g, winglazermango_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.count = 100; // s_set_lifecnt x,#100
    winglazermango_strat(g, idx);
}

/// `winglazermango_strat` (GASTRATS.ASM:2835-2841): accelerate to 50, level
/// out (roty->0, rotx->-5), and expire.
pub fn winglazermango_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let _ = speed_to(al, 50, 1); // s_speedto x,#50,1
        let mut roty = al.roty;
        achase_angle(&mut roty, 0, 4); // s_achase_alvar al_roty,#0,4
        al.roty = roty;
        let mut rotx = al.rotx;
        achase_angle(&mut rotx, (-5i8) as u8, 4); // s_achase_alvar al_rotx,#-5,4
        al.rotx = rotx;
        // s_dec_lifecnt x
        if al.count > 0 {
            al.count -= 1;
        }
        if al.count == 0 {
            g.objs.aldead = 1;
        }
    }
    winglazerman_cont(g, idx);
}

/// `winglazermandie_Istrat` (GASTRATS.ASM:2885-2903): drops `item_7` (laser
/// upgrade) or `item_3`/`item7a` depending on the ship's wing/beam flags,
/// then explodes.
pub fn winglazermandie_istrat(g: &mut Game, idx: u16) {
    winglazermandie_strat(g, idx);
}

fn winglazermandie_strat(g: &mut Game, idx: u16) {
    use crate::enemy_a::{PSF3_BEAMBALL, PSF_BRKLWING, PSF_BRKRWING};
    let brk = g.vars.pshipflags & (PSF_BRKLWING | PSF_BRKRWING);
    let beam = g.vars.pshipflags3 & PSF3_BEAMBALL;
    // ROM: if wings broken OR not beamball → item7; else → item7a helperball.
    let drop_help = brk == 0 && beam != 0;
    if let Some(drop) = make_obj(g, 0) {
        let (px, py, pz) = {
            let me = &g.objs.aliens[idx as usize];
            (me.worldx, me.worldy, me.worldz)
        };
        {
            let al = &mut g.objs.aliens[drop as usize];
            al.worldx = px;
            al.worldy = py;
            al.worldz = pz;
        }
        if drop_help {
            crate::enemy_a::item7a_istrat(g, drop);
        } else {
            crate::enemy_a::strat_item7_init(g, drop);
        }
    }
    strat_explode(g, idx);
}

// ============================================================
// walking (IS 78) — DSTRATS.ASM:860-964. A striding mech: walks a straight
// heading (angle 4) for 200 frames then turns around and slows; shooting its
// legs 5x each topples it over into a wobble/fall/explode death.
// ============================================================

/// `walking_istrat` (DSTRATS.ASM:860-872): full init (hp200/ap16, heading 4 at
/// speed medpspeed+10, prime 2D vecs, leg counters), then fall into
/// `walking_strat` the same tick.
fn walking_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, walking_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WALKING_HP; // s_set_aldata #200,#walkingAP
        al.ap = WALKING_AP;
        al.roty = 4; // s_set_alvar al_roty,#4
        al.vel = MEDPSPEED + 10; // s_set_alvar al_vel,#medpspeed+10 (75)
    }
    strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]); // s_gen_vecs → nvecs_l
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.type_ = 0; // s_set_alvar al_type,#0
        al.sbyte1 = 200; // s_set_alvar al_sbyte1,#200 (walk timer)
        al.sbyte2 = 4; // s_set_alvar al_sbyte2,#4 (left-leg hits)
        al.sbyte3 = 4; // s_set_alvar al_sbyte3,#4 (right-leg hits)
        al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow
        al.snd2 = 0x0d; // set_sound2 x,#$d
    }
    walking_strat(g, idx);
}

/// `walking_strat` (DSTRATS.ASM:874-883): coast along the primed vecs; after
/// 200 frames switch to the turn-around state; every tick run the leg-hit
/// handler.
fn walking_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_jsr daddvecs2pos_x
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        // .walking2_i
        let tick = sid(g, walking2_strat);
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.type_ |= ATZREMOVE; // s_set_alvar al_type,#atzremove
        walking2_strat(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        walking_hit(g, idx);
    }
}

/// `walking2_strat` (DSTRATS.ASM:885-896): turn toward deg180 (roty-=4 until
/// 128) and decelerate (vel-=4 while >=10), then run the leg-hit handler.
pub fn walking2_strat(g: &mut Game, idx: u16) {
    strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]); // s_gen_vecs → nvecs_l
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_jsr daddvecs2pos_x
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.roty != 128 {
            al.roty = al.roty.wrapping_sub(4); // s_add_alvar al_roty,#-4
        }
        if al.vel >= 10 {
            al.vel -= 4; // s_bcc .miss ; s_sub_alvar al_vel,#4
        }
    }
    walking_hit(g, idx);
}

/// `walking_hit` (DSTRATS.ASM:897-911): consume per-leg/head hit flags; a leg's
/// counter reaching 0 topples the mech to that side.
pub fn walking_hit(g: &mut Game, idx: u16) {
    // Left leg (HF1): 5 hits -> fall right.
    if g.objs.aliens[idx as usize].hitflags & HF1 != 0 {
        g.objs.aliens[idx as usize].hitflags &= !HF1; // s_clr_hitflags x,#HF1
        let sb2 = g.objs.aliens[idx as usize].sbyte2;
        if sb2 == 0 {
            walking_right(g, idx); // s_beqdec_alvar al_sbyte2,walking_right
            return;
        }
        g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
    }
    // Right leg (HF2): 5 hits -> fall left.
    if g.objs.aliens[idx as usize].hitflags & HF2 != 0 {
        g.objs.aliens[idx as usize].hitflags &= !HF2;
        let sb3 = g.objs.aliens[idx as usize].sbyte3;
        if sb3 == 0 {
            walking_left(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte3 = sb3 - 1;
    }
    // Head (HF3): just cleared.
    if g.objs.aliens[idx as usize].hitflags & HF3 != 0 {
        g.objs.aliens[idx as usize].hitflags &= !HF3;
    }
}

/// `walking_right` (DSTRATS.ASM:913-924): topple to the right — set the wobble
/// deltas, stagger the body +25 (x2) local, drop a leg explosion, then fall.
/// (The `walker_r` mesh swap is a cosmetic shape id not resolvable from ported
/// map data — scoped out.)
fn walking_right(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = (-2i8) as u8; // s_set_alvar al_sbyte1,#-2
        al.sbyte3 = 1; // s_set_alvar al_sbyte3,#1
    }
    let me = g.objs.aliens[idx as usize];
    full_offset_pos_scaled(g, idx, &me, 25, 0, 0, 1); // #25,#0,#0 <<1
    spawn_explosion_at(g, idx, -100, -80, 0); // #-50,#-40,#0 (<<1)
    walking_fall(g, idx);
}

/// `walking_left` (DSTRATS.ASM:925-935): topple to the left.
fn walking_left(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 2; // s_set_alvar al_sbyte1,#2
        al.sbyte3 = (-1i8) as u8; // s_set_alvar al_sbyte3,#-1
    }
    let me = g.objs.aliens[idx as usize];
    full_offset_pos_scaled(g, idx, &me, -25, 0, 0, 1); // #-25,#0,#0 <<1
    spawn_explosion_at(g, idx, 100, -80, 0); // #50,#-40,#0 (<<1)
    walking_fall(g, idx);
}

/// `walking_fall` (DSTRATS.ASM:936-939): enter the 5-frame wobble.
fn walking_fall(g: &mut Game, idx: u16) {
    let tick = sid(g, wobble_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte2 = 5; // s_set_alvar al_sbyte2,#5
    wobble_strat(g, idx);
}

/// `wobble_strat` (DSTRATS.ASM:940-946): rock in rotz (delta flips each tick)
/// and drift roty for 5 frames, then fall over.
fn wobble_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_sub(al.sbyte3); // s_sub_alvars al_rotz,al_sbyte3
        al.sbyte3 = (al.sbyte3 as i8).wrapping_neg() as u8; // s_neg_alvar al_sbyte3
    }
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        fellit(g, idx); // s_beqdec_alvar al_sbyte2,fellit
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_sub(al.sbyte1); // s_sub_alvars al_roty,al_sbyte1
}

/// `fellit` (DSTRATS.ASM:948-949): enter the 7-frame fall-over.
fn fellit(g: &mut Game, idx: u16) {
    let tick = sid(g, fallover_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte2 = 7; // s_set_alvar al_sbyte2,#7
}

/// `fallover_strat` (DSTRATS.ASM:950-964): topple in rotz for 7 frames, then
/// drop a final explosion and kill the mech.
fn fallover_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_sub(al.sbyte1); // s_sub_alvars al_rotz,al_sbyte1
        al.sbyte1 = al.sbyte1.wrapping_sub(al.sbyte3); // s_sub_alvars al_sbyte1,al_sbyte3
    }
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        // .killit
        spawn_explosion_at(g, idx, 0, -160, 0); // s_add_Roffs2pos #0,#-80,#0 (<<1)
        kill_obj(&mut g.objs.aliens[idx as usize]); // s_kill_obj x
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
}

// ============================================================
// uperm (IS 160) — GA2STRAT.ASM:1112-1141. Pops up out of the planet, levels
// off, and dashes at the player when they are in a Z sweet-spot, spinning in
// rotz the whole time.
// ============================================================

/// `uperm_Istrat` (GA2STRAT.ASM:1112-1122): rise pose (pitched straight up,
/// facing deg180, speed 70), capture the player's y as the level-off datum,
/// then fall into `uperm_strat`.
pub fn uperm_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, uperm_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let py = g.vars.player_posy;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP; // enemy1 + enemyweap
    al.hp = UPERM_HP; // s_set_aldata #upermHP,#upermAP
    al.ap = UPERM_AP;
    al.roty = DEG180; // s_set_alvar al_roty,#deg180
    al.rotx = DEG90.wrapping_neg(); // s_set_alvar al_rotx,#-deg90 (192)
    al.vel = 70; // s_set_speed x,#70
    al.sword1 = py; // s_set_alvar W,x,al_sword1,player_posy
    al.snd2 = 2; // set_sound2 x,#2
    uperm_strat(g, idx);
}

/// `uperm_strat` (GA2STRAT.ASM:1123-1141): once risen `sword1+756` above the
/// player datum, level the pitch and — if the player is 400..1300 Z away —
/// dash at speed 80 while yaw-tracking; always spin rotz and scroll.
pub fn uperm_strat(g: &mut Game, idx: u16) {
    let thr = g.objs.aliens[idx as usize].sword1.wrapping_add(756); // svar_word1
                                                                    // s_jmp_lower x,svar_word1,.nysearch -> skip the dash while worldy >= thr.
    if !worldy_ge(g, idx, thr) {
        // s_achase_alvar al_rotx,#0,3
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, 0, 3);
        g.objs.aliens[idx as usize].rotx = rotx;
        // .nch: s_jmp_outZdistrng x,y,#400,#1300,.nysearch
        if !out_zdist_rng(g, idx, 400, 1300) {
            let _ = speed_to(&mut g.objs.aliens[idx as usize], 80, 2); // s_speedto #80,2
            g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(10); // s_add_alvar al_rotz,#10
            if let Some(pl) = player(g) {
                strat_aim_yaw(g, idx, &pl, 1); // s_obj2obj_angle x,y,al_roty,1
            }
        }
    }
    // .nysearch
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(8); // s_add_alvar al_rotz,#8
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// rockhard (IS 192) — GSTRATS.ASM:663-669. A static, effectively
// indestructible rock obstacle: set data + facing, no per-tick strat.
// ============================================================

/// `rockhard_Istrat` (GSTRATS.ASM:663-669): enemy1 collide, faces deg180,
/// hardHP (255) / rockhardAP (20), then `s_set_strat x,0` — a null tick, so it
/// just sits and collides.
fn rockhard_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
    al.roty = DEG180; // s_set_alvar al_roty,#deg180
    al.hp = HARDHP; // s_set_aldata #hardHP,#rockhardAP
    al.ap = ROCKHARD_AP;
    al.stratptr = None; // s_set_strat x,0 (no tick)
}

// ============================================================
// Space / air-hazard family — meteor set (meteo0 / big_meteor /
// break_meteor / break_meteorT), mine0, and torpedo. ASM is the sole ground
// truth (no C-oracle). ISTRATS.ASM def rows == sf-map placement indices
// (grep sf-map; the reachable index drifts from the raw macro-count past ~162,
// so these trust the placement value):
//   - torpedo        = 80  (ISTRATS.ASM:503)  — route2 rc / route3 level3_3
//   - meteo0         = 195 (ISTRATS.ASM:625)  — route3 level3_2
//   - big_meteor     = 234 (ISTRATS.ASM:673)  — route3 level3_2
//   - break_meteor   = 235 (ISTRATS.ASM:676)  — route1 level1_2 / route3
//   - break_meteorT  = 238 (ISTRATS.ASM:681)  — route1 level1_2 / route3
//   - mine0          = assembled address $09:9117 — route1 level1_5
// All six are placed by ported maps -> reachable.
//
// State machines (per-fn cites):
//   meteo0 : sits inert until the player closes to 1000 z, grows its anim to
//            full (8) shedding nohitaffect, then rains homing lasers on a
//            notdelay-3 gate for a 20-tick budget; death spawns an asteroid1
//            fragment running meteor_strat, then explodes. GA2STRAT.ASM:2130-2168.
//   big_meteor : indestructible (hardHP) spinning obstacle; init randomizes an
//            (unused) spin datum, tick is a pure no-op. D3STRATS.ASM:1069-1078.
//   break_meteor / break_meteorT : destructible asteroid-belt fragments driven
//            in the ROM by the motionless `break2`/`break1` PATHS (DPATHDAT.ASM
//            :1778-1795). The path VM is not wired into this crate, so the port
//            reduces them to their observable effect — a meteorHP/meteorAP
//            destructible that scrolls past (add_playerZ, inferred: the paths
//            carry NO velocity command) with a death trigger: break_meteor emits
//            particles (standard explode); break_meteorT additionally 50%-spawns
//            a tadpole. D3STRATS.ASM:1080-1090 + DPATHDAT.ASM break1/break2.
//   mine0  : static random-oriented destructible mine (enemy1), standard
//            explosion on death. DSTRATS.ASM:1572-1582.
//   torpedo: invisible Zenemy that homes the player's yaw underwater at speed 30;
//            inside 800 z it surfaces (f_fish shape, pitch -deg45, becomes
//            collidable) and levels its pitch back to 0. GASTRATS.ASM:2007-2044.
// ============================================================

/// ISTRATS.ASM def rows (== sf-map placement indices).
const IS_TORPEDO: usize = 79;
const IS_METEO0: usize = 194;
const IS_BIG_METEOR: usize = 233;
const IS_BREAK_METEOR: usize = 234;
const IS_BREAK_METEORT: usize = 237;

/// Hazard-family equs (STRATEQU.INC).
const METEOR_HP: u8 = 2; // STRATEQU.INC:212 meteorHP
const METEOR_AP: u8 = 12; // STRATEQU.INC:213 meteorAP
const MINE0_HP: u8 = 2; // STRATEQU.INC:226 mine0HP
const MINE0_AP: u8 = 10; // STRATEQU.INC:227 mine0AP
const TORPEDO_HP: u8 = 4; // STRATEQU.INC:132 torpedoHP
const TORPEDO_AP: u8 = 4; // STRATEQU.INC:133 torpedoAP
const BIG_METEOR_AP: u8 = 12; // D3STRATS.ASM:1073 s_set_aldata #hardHP,#12
const METEO0_HP: u8 = 2; // GA2STRAT.ASM:2133 s_set_aldata #2,#16
const METEO0_AP: u8 = 16;
const METEO0_BUDGET: u8 = 20; // GA2STRAT.ASM:2136 al_sbyte1 = 20

/// Cosmetic child shapes (sf-map route3 common.rs SH_* numbers). These meshes
/// may not resolve to a live model, but the spawned object's *behaviour* (the
/// meteor drift / tadpole placement) is what this lane reproduces.
const SH_ASTEROID1: u16 = 275; // meteo0 death fragment (SH_ASTEROID1_PROXY)
const SH_F_FISH: u16 = 271; // torpedo surfaced shape (SH_F_FISH_PROXY)
const SH_TADPOLE: u16 = 227; // break_meteorT death spawn (SH_TADPOLE)

// ------------------------------------------------------------
// meteo0 (IS 195) — GA2STRAT.ASM:2130-2168.
// ------------------------------------------------------------

/// `meteo0_Istrat` (GA2STRAT.ASM:2130-2137): wire strats/data, face deg180,
/// anim 0, prime the 20-tick fire budget, and set nohitaffect (invulnerable
/// while it grows). The ROM uses `coll_Istrat` (a hitflash that suppresses the
/// visual flash) as the collide handler; this port substitutes the standard
/// `strat_hit_flash` — the damage-gating (nohitaffect) semantics match; only
/// the cosmetic flash differs (GSTRATS.ASM:887-893 coll_Istrat vs hitflash).
/// Init falls into `meteo0_strat` next tick (ROM has `s_start_strat` between,
/// i.e. no fall-through).
fn meteo0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, meteo0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, meteo0_exp);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = METEO0_HP; // s_set_aldata #2,#16
    al.ap = METEO0_AP;
    al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
    al.animframe = 0; // s_init_anim x,#0
    al.sbyte1 = METEO0_BUDGET; // s_set_alvar B,x,al_sbyte1,#20
    al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag x,nohitaffect
                                  // No s_end_strat before .meteo0_strat -> falls into the tick body same frame.
    meteo0_strat(g, idx);
}

/// `.meteo0_strat` (GA2STRAT.ASM:2138-2162): inert until close; grow anim to 8;
/// once maxed clear nohitaffect and rain homing lasers on the notdelay-3 gate,
/// consuming the budget; scroll while active.
fn meteo0_strat(g: &mut Game, idx: u16) {
    // s_jmp_alvarZERO B,x,al_sbyte1,.nclose — budget spent -> inert (no scroll).
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        return;
    }
    // s_set_objtobeplayer y ; s_jmp_Zdistmore x,y,#1000,.nclose — far -> inert.
    if zdist_more(g, idx, 1000) {
        return;
    }
    // s_cmp_anim x,#8 ; s_beq .max ; else s_add_anim x,#1,#9 (wrap 9) ; .done.
    let anim = g.objs.aliens[idx as usize].animframe;
    if anim != 8 {
        g.objs.aliens[idx as usize].animframe = (anim + 1) % 9;
        add_player_z(g, idx); // .done: s_add_playerZ x
        return;
    }
    // .max: s_clr_alsflag x,nohitaffect
    g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
    // s_jmp_notdelay 3,.nfire,al1pt — fire when (gameframe + idx) & 7 == 0
    // (al1pt == per-object index stagger, port convention; enemy_a.rs:2543).
    if g.vars.gameframe.wrapping_add(idx) & 7 == 0 {
        // s_weapon_pos #0,#0,#0 ; s_weapon_rots2obj y ; s_fire_weapon RELSLOWELASERHOME:
        // muzzle at centre, aim the homing laser at the player in full 3D.
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            strat_fire_relslowlaserhome(g, idx, pitch, yaw);
        }
    }
    // .nfire: s_dec_alvar B,x,al_sbyte1
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    g.objs.aliens[idx as usize].sbyte1 = sb1.wrapping_sub(1);
    // .done: s_add_playerZ x
    add_player_z(g, idx);
}

/// `.exp_Istrat` (GA2STRAT.ASM:2163-2168): spawn an asteroid1 fragment running
/// `meteor_Istrat` at this meteor's pose, then run the standard explosion.
fn meteo0_exp(g: &mut Game, idx: u16) {
    // s_make_obj #asteroid1,.badobj ; s_set_strat y,meteor_Istrat ; s_copy_pos y,x
    if let Some(child) = make_obj(g, SH_ASTEROID1) {
        copy_pos(g, child, idx);
        let frag = sid(g, meteor_istrat);
        g.objs.aliens[child as usize].stratptr = Some(frag);
    }
    // .badobj: s_jmp explode_Istrat
    strat_explode(g, idx);
}

// ------------------------------------------------------------
// meteor fragment (meteo0's asteroid1 child) — DSTRATS.ASM:1215-1246. A drifting
// asteroid: random spin/velocity/heading, then each tick drifts toward the
// viewer (worldz -= 60) + spins + coasts. Self-contained; ported here as the
// fragment-spawn model (meteor_Istrat is not otherwise registered).
// ------------------------------------------------------------

/// `meteor_istrat` entry (DSTRATS.ASM:1215-1246): sword1=60 then the shared
/// `.in` init — data, random spin (sbyte1 = rnd&3, negated when the random
/// heading roty >= 128, then 50%-zeroed), random heading + velocity, gen 3D
/// vecs, enemy1 collide, nohitaffect. Falls into `meteor_strat`.
/// (`s_sprite_obj`/`drotsflat_x` are cosmetic billboard/flat-orient ops —
/// scoped out.)
fn meteor_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, meteor_strat);
    let coll = sid(g, meteorcol_istrat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sword1 = 60; // s_set_alvar W,x,al_sword1,#60
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = METEOR_HP; // s_set_aldata #meteorHP,#meteorAP
        al.ap = METEOR_AP;
        al.rotz = 0; // s_set_alvar B,x,al_rotz,#0
    }
    // s_set_alvar2rnd x,al_vel,#7 / al_sbyte1,#3 / al_roty (full byte).
    let vel = (sf_random(&mut g.vars) as u8) & 7;
    let sbyte1 = (sf_random(&mut g.vars) as u8) & 3;
    let roty = sf_random(&mut g.vars) as u8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = vel;
        al.roty = roty;
        // s_cmp_alvar B,al_roty,#128 ; s_bcc .oneway — roty>=128 negates the spin.
        al.sbyte1 = if roty >= 128 {
            (sbyte1 as i8).wrapping_neg() as u8
        } else {
            sbyte1
        };
    }
    // s_jmp_random .noclear — branch (skip) when random<127; else zero the spin.
    if !jmp_random50(g) {
        g.objs.aliens[idx as usize].sbyte1 = 0;
    }
    // meteor_istrat3: s_jsr dgen3dvecs (roty/rotx=0/vel) then fall into strat.
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].collflags |= COLLTYPE_ENEMY1;
    g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
    meteor_strat(g, idx);
}

/// `meteor_strat` (DSTRATS.ASM:1240-1246): drift toward the viewer (worldz -=
/// sword1) unless it is an `asteroid3` variant (never true for this asteroid1
/// fragment, so the guard is inert here), spin (rotz += sbyte1), and coast.
fn meteor_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_cmp_alvar W,al_shape,#asteroid3 ; s_beq .missbit — always false here.
        al.worldz = al.worldz.wrapping_sub(al.sword1); // s_sub_alvars al_worldz,al_sword1
        al.rotz = al.rotz.wrapping_add(al.sbyte1); // s_add_alvars al_rotz,al_sbyte1
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_jsr daddvecs2pos_x
}

// ------------------------------------------------------------
// big_meteor (IS 234) — D3STRATS.ASM:1069-1078.
// ------------------------------------------------------------

/// `big_meteor_istrat` (D3STRATS.ASM:1069-1077): an indestructible (hardHP)
/// obstacle. Sets nohitaffect + ap12, randomizes a spin datum in sbyte1
/// ((rnd&15)-8, which `.strat` never actually reads), then installs the no-op
/// tick. `s_rots_flat` (a view-vector flat orientation) is cosmetic — scoped
/// out. `.strat` is a pure `s_end_strat` no-op.
fn big_meteor_init(g: &mut Game, idx: u16) {
    let tick = sid(g, big_meteor_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    // s_set_alvar2rnd x,al_sbyte1,#15 ; s_sub_alvar B,al_sbyte1,#8.
    let sb1 = ((sf_random(&mut g.vars) as u8) & 15).wrapping_sub(8);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag x,nohitaffect
    al.hp = HARDHP; // s_set_aldata #hardHP,#12 (255 == indestructible)
    al.ap = BIG_METEOR_AP;
    al.sbyte1 = sb1;
}

/// `big_meteor .strat` (D3STRATS.ASM:1077-1078): `s_end_strat` — pure no-op.
fn big_meteor_strat(_g: &mut Game, _idx: u16) {}

// ------------------------------------------------------------
// break_meteor / break_meteorT (IS 235 / 238) — D3STRATS.ASM:1080-1090.
// Path-driven in the ROM; reduced to destructible + scroll + death trigger
// (see family header). The break1/break2 paths carry no velocity (DPATHDAT.ASM
// :1778-1795), so per-tick motion is scroll-only.
// ------------------------------------------------------------

/// `break_meteor_istrat` (D3STRATS.ASM:1080-1084): `s_set_path x,break2` +
/// aldata(2, meteorAP) then `jml path_istrat`. Reduced: destructible meteor,
/// standard explosion (break2's `P_TRIGGER pparticles,WhenDead`).
fn break_meteor_init(g: &mut Game, idx: u16) {
    let tick = sid(g, break_meteor_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = METEOR_HP; // s_set_aldata #2,#meteorAP
    al.ap = METEOR_AP;
    // jml path_istrat runs the path tick same frame; reduced to scroll-only.
    break_meteor_strat(g, idx);
}

/// `break_meteort_istrat` (D3STRATS.ASM:1086-1090): `s_set_path x,break1` +
/// aldata(2, meteorAP) then `jml path_istrat`. break1 == break2 plus a
/// `createtadpole` death trigger (50% spawn a tadpole, DPATHDAT.ASM:1787-1792).
fn break_meteort_init(g: &mut Game, idx: u16) {
    let tick = sid(g, break_meteor_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, break_meteort_exp);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = METEOR_HP;
    al.ap = METEOR_AP;
    break_meteor_strat(g, idx);
}

/// Shared break-meteor tick: the break1/break2 paths have no motion command, so
/// the reduced behaviour is scroll-only (add_playerZ — inferred, see header).
fn break_meteor_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

/// `break1.createtadpole` (DPATHDAT.ASM:1787-1792): `P_RANDOMGOTO .notad` skips
/// the spawn on a 50% coin (branch when random<127), else `P_QSPAWN tadpole,
/// meteor_tadpole` spawns a tadpole (path meteor_tadpole == `P_SETSTRAT
/// tadpole_istrat`). The spawned object crosses directly into the native
/// tadpole initializer, matching the path program's immediate strategy
/// handoff. Then the standard explosion runs.
fn break_meteort_exp(g: &mut Game, idx: u16) {
    // P_RANDOMGOTO branches (skips spawn) when random<127; spawn on the else.
    if !jmp_random50(g) {
        if let Some(t) = make_obj(g, SH_TADPOLE) {
            copy_pos(g, t, idx);
            crate::enemy_a::strat_tadpole_init(g, t);
        }
    }
    strat_explode(g, idx);
}

// ------------------------------------------------------------
// mine0 ($09:9117) — DSTRATS.ASM:1572-1582.
// ------------------------------------------------------------

/// `mine0_istrat` (DSTRATS.ASM:1572-1577): a static destructible mine — hp2/
/// ap10, enemy1 collide, a random full-byte rotz orientation, standard
/// explosion. Installs the no-op `mine0_strat` tick. (NOTE: the ROM mine0 uses
/// plain `explode_istrat`, NOT `mine2exp` — the proximity/beam-burst death
/// belongs to the unrelated `mine2` strat, GA2STRAT.ASM:2560.)
pub fn mine0_istrat(g: &mut Game, idx: u16) {
    mine0_init(g, idx);
}

fn mine0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, mine0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    // s_set_alvar2rnd x,al_rotz (full byte -> any orientation).
    let rotz = sf_random(&mut g.vars) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = MINE0_HP; // s_set_aldata #mine0HP,#mine0AP
    al.ap = MINE0_AP;
    al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,ENEMY1
    al.rotz = rotz;
}

/// `mine0_strat` (DSTRATS.ASM:1578-1580): `s_end_strat` — pure no-op.
fn mine0_strat(_g: &mut Game, _idx: u16) {}

// ------------------------------------------------------------
// torpedo (IS 80) — GASTRATS.ASM:2007-2044.
// ------------------------------------------------------------

/// `torpedo_Istrat` (GASTRATS.ASM:2007-2014): starts invisible (nullshape) and
/// non-collidable (colldisable) underwater, speed 30, Zenemy collide, hp4/ap4.
/// Falls into `torpedo_strat` the same tick (no `s_end_strat` between).
pub fn torpedo_istrat(g: &mut Game, idx: u16) {
    torpedo_init(g, idx);
}

fn torpedo_init(g: &mut Game, idx: u16) {
    let tick = sid(g, torpedo_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = TORPEDO_HP; // s_set_aldata #torpedoHP,#torpedoAP
        al.ap = TORPEDO_AP;
        al.shape = 0; // s_set_alvar W,x,al_shape,#nullshape
        al.vel = 30; // s_set_speed x,#30
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.collflags |= COLLTYPE_ZENEMY; // s_set_colltype x,Zenemy
    }
    torpedo_strat(g, idx);
}

/// `torpedo_strat` (GASTRATS.ASM:2015-2030): small splash each tick; while >800 z
/// yaw-home the player (rate 3) then move; inside 800 z surface via `torpedoa_init`.
pub fn torpedo_strat(g: &mut Game, idx: u16) {
    // s_jsl makeSsplash_srou_l (notdelay gate is commented out in ROM → every tick)
    let _ = makessplash_srou(g, idx);
    // s_jmp_Zdistless x,y,#800,torpedoa_init
    if zdist_less(g, idx, 800) {
        torpedoa_init(g, idx);
        return;
    }
    // s_obj2obj_angle x,y,al_roty,3  (Yanglexy + nega + Achase rate 3)
    if let Some(pl) = player(g) {
        strat_aim_yaw(g, idx, &pl, 3);
    }
    torpedo_cont(g, idx);
}

/// `torpedo_cont` (GASTRATS.ASM:2026-2030): gen 3D vecs, scroll, coast.
fn torpedo_cont(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]); // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    add_player_z(g, idx); // s_add_playerZ x
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos x
}

/// `torpedoa_init` (GASTRATS.ASM:2032-2038): surface — pitch up -deg45, become
/// the visible f_fish, splash + enemyupsea, become collidable, then fall into
/// `torpedoa_strat`.
pub fn torpedoa_init(g: &mut Game, idx: u16) {
    let tick = sid(g, torpedoa_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_strat x,torpedoa_strat
        al.rotx = (-(DEG45 as i8)) as u8; // s_set_alvar B,x,al_rotx,#-deg45
        al.shape = SH_F_FISH; // s_set_alvar W,x,al_shape,#f_fish
        al.sflags &= !ASF_COLLDISABLE; // s_clr_alsflag x,colldisable
    }
    let _ = makesplash_srou(g, idx); // s_jsl makesplash_srou_l
                                     // jsl enemyupsea_l → makesnd POS_ENEMYUPSEA
    {
        let al = &g.objs.aliens[idx as usize];
        let (ox, oz) = (al.worldx, al.worldz);
        g.hooks.make_snd(PosSndFamilyId::EnemyUpSea, ox, oz);
    }
    torpedoa_strat(g, idx);
}

/// `torpedoa_strat` public (GASTRATS.ASM:2039-2042).
pub fn torpedoa_strat(g: &mut Game, idx: u16) {
    // s_achase_alvar B,x,al_rotx,#0,2
    let mut rotx = g.objs.aliens[idx as usize].rotx;
    achase_angle(&mut rotx, 0, 2);
    g.objs.aliens[idx as usize].rotx = rotx;
    // s_brl torpedo_cont
    torpedo_cont(g, idx);
}

// ============================================================
// Base / colony structure set-pieces — base0 / massivebase / colony0-2 /
// colonyexit. ASM is the sole ground truth (no C-oracle): KSTRATS.ASM
// (base0:353-370), D2STRATS.ASM (massivebase:650-681), GA2STRAT.ASM
// (colony0/1/2:1671-1779, colonyexit:3039-3053). ISTRATS.ASM def_Istrat rows
// DRIFT +1 from the sf-map placement past ~row 162 (macro-count vs placed
// value); every index below is the sf-map placement VALUE (grep sf-map):
//   - base0       = 138 (route1 level1_4)        — animated landing-base door.
//   - massivebase = 142 (route2 level2_3 / rc)   — indestructible mega-structure
//                                                   that funnels the player in.
//   - colony0     = 170 (route3 level3_4)        — space-colony approach trigger
//                                                   (cutscene + ambient debris).
//   - colony1     = 171 (route3 level3_4)        — colony piece mirrored on the
//                                                   camera's vertical.
//   - colony2     = 172 (route3 level3_4)        — colony entrance door.
//   - colonyexit  = 236 (route1 level1_3 / route2 level2_6 / rc) — animated
//                                                   exit-tunnel door.
// All six are placed by ported maps -> reachable.
//
// State machines (per-fn cites):
//   base0      : static enemy1 obstacle (hardHP/AP2, faces deg270). Waits inert
//                until the player closes to 2500 z, then plays its open anim
//                (0->8) and holds. Collide/explode both re-run the tick (never
//                explodes). KSTRATS.ASM:353-370.
//   massivebase: colldisable indestructible structure (hardHP/hardAP, faces
//                deg180). Inside 3000 z it forces player control off and drags
//                the player toward x=0 / y=viewcy (funnel). LOD shape swap:
//                kichi_0 (near, <0x3500 z) / kichi_1 (far). D2STRATS.ASM:650-681.
//   colony0    : enemy1/gnd approach trigger. Far (>=1500 z) it sheds ambient
//                wireframe-spacebar debris on a notdelay-4 gate. Inside 800 z it
//                disables collision, forces control off and drags the player to
//                x=0 / y=viewcy; once the player passes it (objinfront) it latches
//                sflag1 + sets GF_STRATDONE1. Always scrolls z-20 + add_playerZ.
//                GA2STRAT.ASM:1671-1730.
//   colony1    : colldisable/gnd piece that pins its worldy to 2*viewcy -
//                viewposy + 50 each tick (mirror of the camera), then runs
//                colony0_cont's z-20 + add_playerZ. GA2STRAT.ASM:1734-1754.
//   colony2    : colldisable/gnd door positioned at its al_ptr parent + 280 z;
//                opens (anim 0->9) when the player is in front or within 40 z.
//                GA2STRAT.ASM:1758-1779.
//   colonyexit : self-recurring colldisable/gnd door (never sets a separate
//                tick). Opens (anim 0->9) as the player approaches from in front
//                and beyond 75 z; snaps shut (anim 0) when the player is behind
//                it or within 75 z. NOT a stage-transition — purely cosmetic
//                (the REACHABLE_UNPORTED guess of an IS_COLONYEXIT level-end is
//                wrong; the ASM sets no LE_/levelfinished). GA2STRAT.ASM:3039-3053.
// ============================================================

/// ISTRATS.ASM def rows resolved to sf-map placement indices.
const IS_BASE0: usize = 137;
const IS_MASSIVEBASE: usize = 142;
const IS_COLONY0: usize = 169;
const IS_COLONY1: usize = 170;
const IS_COLONY2: usize = 171;
const IS_COLONYEXIT: usize = 235;

/// Structure data (STRATEQU.INC:66/68). `HARDHP` (0xFF == -1) is defined above.
const HARD_AP: u8 = 8; // STRATEQU.INC:66 hardAP
const BASE0_AP: u8 = 2; // KSTRATS.ASM:357 s_set_aldata #hardhp,#2

/// LOD shape words. `kichi_0` == 120 (sf-map rc.rs SH_KICHI_0 / shape_data
/// #120). `kichi_1` is an uncompiled wireframe mesh (SHAPES.EXT:287; not in the
/// 236-shape render table) — there is no valid render id for it, so the far-LOD
/// swap reuses kichi_0. Scoped-out: the far low-detail mesh is not shown; the
/// near (playable) LOD is faithful. (massivebase D2STRATS.ASM:675/678.)
const KICHI_0: u16 = 119;
const KICHI_1: u16 = 120;

/// `XPwirespacebar` ambient-debris shape (sf-map consts.rs XPWIRESPACEBAR /
/// shape_data #138). colony0 GA2STRAT.ASM:1716.
const XPWIRESPACEBAR: u16 = 138;

// ------------------------------------------------------------
// objinfront helper (STRATMAC.INC:3445 s_jmp_objinfront: rlbpl on
// al_worldz[a] - al_worldz[b] >= 0, i.e. a.worldz >= b.worldz). Player-absent
// -> false (don't branch), the safe default vs the ROM's garbage compare.
// ------------------------------------------------------------

/// `s_jmp_objinfront self,player` — self is at/beyond the player in z.
fn self_in_front_of_player(g: &Game, idx: u16) -> bool {
    match player(g) {
        Some(p) => g.objs.aliens[idx as usize].worldz >= p.worldz,
        None => false,
    }
}

/// `s_jmp_objinfront player,self` — player is at/beyond self in z.
fn player_in_front_of_self(g: &Game, idx: u16) -> bool {
    match player(g) {
        Some(p) => p.worldz >= g.objs.aliens[idx as usize].worldz,
        None => false,
    }
}

// ------------------------------------------------------------
// base0 (IS 138) — KSTRATS.ASM:353-370.
// ------------------------------------------------------------

/// ROM `base0_Istrat` (KSTRATS.ASM:353): enemy1 collide, faces deg270,
/// hardHP(255)/AP2, anim 0. `s_set_alptrs x,base0_strat,base0_strat,base0_strat`
/// aims tick + collide + explode all at the tick — it has no real death chain
/// (hardHP + explode==tick means it never explodes). No `s_end_strat` before the
/// `base0_strat` label -> falls into the tick this same frame.
pub fn base0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, base0_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.stratptr = Some(tick); // s_set_alptrs x,base0_strat,...
        al.collstratptr = Some(tick);
        al.expstratptr = Some(tick);
        al.hp = HARDHP; // s_set_aldata #hardhp,#2
        al.ap = BASE0_AP;
        al.roty = DEG270; // s_set_alvar B,x,al_roty,#deg270
        al.animframe = 0; // s_init_anim x,#0
    }
    base0_strat(g, idx);
}

/// Compatibility alias for the istrat table registration.
fn base0_init(g: &mut Game, idx: u16) {
    base0_istrat(g, idx);
}

/// ROM `base0_strat` (KSTRATS.ASM:360): inert until the player closes to 2500 z,
/// then hands off to `base0b_strat` (falls in same tick) which grows the open
/// anim to 8 and holds.
pub fn base0_strat(g: &mut Game, idx: u16) {
    // s_jmp_Zdistless x,y,#2500,.start
    if zdist_less(g, idx, 2500) {
        // .start: s_set_strat x,base0b_strat ; no s_end_strat -> fall through.
        let t = sid(g, base0b_strat);
        g.objs.aliens[idx as usize].stratptr = Some(t);
        base0b_strat(g, idx);
    }
    // else s_end_strat (stay waiting).
}

/// ROM `base0b_strat` (KSTRATS.ASM:365): `s_cmp_anim #8` / `s_beq .no` gate over
/// `s_add_anim x,#1,#15` (3-arg wrap-at-15) — increments the open anim each tick
/// and stops at 8 (wrap never reached).
pub fn base0b_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    if al.animframe != 8 {
        al.animframe = (al.animframe + 1) % 15;
    }
}

// ------------------------------------------------------------
// massivebase (IS 142) — D2STRATS.ASM:650-681.
// ------------------------------------------------------------

/// `massivebase_istrat` (D2STRATS.ASM:650-657): tick=.strat, no collide/explode
/// (`s_set_alptrs x,.strat,0,0`), colldisable, hardHP/hardAP, faces deg180, clear
/// the zremove type bit and clears the map trigger. No `s_end_strat` before
/// `.strat` -> falls into the tick this same frame.
fn massivebase_init(g: &mut Game, idx: u16) {
    let tick = sid(g, massivebase_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None; // s_set_alptrs x,.strat,0,0
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.hp = HARDHP; // s_set_aldata #hardHP,#hardAP
        al.ap = HARD_AP;
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        al.type_ &= !ATZREMOVE; // s_clr_altype x,zremove
    }
    g.vars.map.trigger = 0; // s_set_var B,maptrigger,#0
    massivebase_strat(g, idx);
}

/// `.strat` (D2STRATS.ASM:658-681): inside 3000 z force player control off and
/// drag the player toward x=0 / y=viewcy (rate-4 achase); LOD swap kichi_0
/// (near, |dz| < 0x3500) / kichi_1 (far).
fn massivebase_strat(g: &mut Game, idx: u16) {
    // A depth distance below 3,000 applies the funnel.
    if zdist_less(g, idx, 3000) {
        g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE; // s_playerctrl off
        if let Some(pi) = player_index(g) {
            let viewcy = g.vars.sv_i16(sv::VIEWCY);
            let px = g.objs.aliens[pi as usize].worldx;
            g.objs.aliens[pi as usize].worldx = achase_word(px, 0, 4); // ->x=0
            let py = g.objs.aliens[pi as usize].worldy;
            g.objs.aliens[pi as usize].worldy = achase_word(py, viewcy, 4); // ->viewcy
        }
    }
    // The near behavior begins below the source threshold.
    let shape = if zdist_less(g, idx, 0x3500) {
        KICHI_0
    } else {
        KICHI_1
    };
    g.objs.aliens[idx as usize].shape = shape;
}

// ------------------------------------------------------------
// colony0 / colony1 (IS 170 / 171) — GA2STRAT.ASM:1671-1754.
// ------------------------------------------------------------

/// `colony0_Istrat` (GA2STRAT.ASM:1671-1678): tick=colony0_strat, hp/ap 10,
/// clear GF_STRATDONE1, enemy1 collide, gnd, sound2=8. Falls into the tick this
/// frame (no `s_end_strat` before the label).
fn colony0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, colony0_strat);
    g.vars.gameflags &= !GF_STRATDONE1; // s_and_var gameflags,#~gf_stratdone1
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_strat x,colony0_strat
        al.hp = 10; // s_set_aldata #10,#10
        al.ap = 10;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.type_ |= ATGND; // s_set_altype x,gnd
        al.snd2 = 8; // set_sound2 x,#8
    }
    colony0_strat(g, idx);
}

/// `colony0_strat` (GA2STRAT.ASM:1679-1730): the approach-trigger state machine.
fn colony0_strat(g: &mut Game, idx: u16) {
    // s_jmp_alsflag x,sflag1,.nthere — already latched -> just scroll.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        colony0_cont(g, idx);
        return;
    }
    // s_jmp_Zdistmore x,y,#200<<2,.nclose — far (>=800) -> debris path.
    if zdist_more(g, idx, 200 << 2) {
        colony0_nclose(g, idx);
        return;
    }
    // within 800: s_set_alsflag x,colldisable
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    // s_jmp_varAND pshipflags2,#psf2_playerHP0,colony0_cont — player dead: skip.
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        colony0_cont(g, idx);
        return;
    }
    // cutscene funnel: s_or_var pstratflags,#pstf_inseq
    g.vars.pstratflags |= PSTF_INSEQ;
    // splayerflymode INSIDE -> TONORM (+ changeviewmode_l, unported view-mode swap).
    if g.vars.splayerflymode == SPFM_INSIDE {
        g.vars.splayerflymode = SPFM_TONORM;
    }
    // s_playerctrl off
    g.vars.pshipflags |= PSF_NOCTRL | PSF_NOFIRE;
    if let Some(pi) = player_index(g) {
        let viewcy = g.vars.sv_i16(sv::VIEWCY);
        let px = g.objs.aliens[pi as usize].worldx;
        g.objs.aliens[pi as usize].worldx = achase_word(px, 0, 4); // ->x=0
        let py = g.objs.aliens[pi as usize].worldy;
        g.objs.aliens[pi as usize].worldy = achase_word(py, viewcy, 4); // ->viewcy
    }
    // s_jmp_objinfront x,y,.nthere — colony still ahead of the player -> wait.
    if self_in_front_of_player(g, idx) {
        colony0_cont(g, idx);
        return;
    }
    // player has passed through: latch done.
    g.vars.gameflags |= GF_STRATDONE1; // s_or_var gameflags,#gf_stratdone1
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    colony0_cont(g, idx); // s_brl .nthere
}

/// `.nclose` (GA2STRAT.ASM:1713-1724): far-field ambient debris. Spawns a
/// wireframe spacebar on the notdelay-4 gate only when |dz| >= 1500, then
/// randomizes its orientation and scatters it, then installs the shared exact
/// `SPINspacebar_Istrat` registered by sf-game (GA2STRAT.ASM:1531-1547).
fn colony0_nclose(g: &mut Game, idx: u16) {
    // s_jmp_Zdistless x,y,#1500,.nthere
    if zdist_less(g, idx, 1500) {
        colony0_cont(g, idx);
        return;
    }
    // s_jmp_notdelay 4,.nthere — gameframe & 15 == 0.
    if !notdelay(g, 4) {
        colony0_cont(g, idx);
        return;
    }
    // s_make_obj #XPwirespacebar,.nthere
    if let Some(dbr) = make_obj(g, XPWIRESPACEBAR) {
        copy_pos(g, dbr, idx); // s_copy_pos y,x (debris <- colony)
                               // s_set_alvar2rnd y,al_rotz — full-byte random orientation.
        let rotz = (sf_random(&mut g.vars) & 0xFF) as u8;
        // s_set_alvar2rnd y,al_sbyte1,#15 ; s_sub_alvar #7 -> spin rate [-7,+8].
        let sbyte1 = ((sf_random(&mut g.vars) & 15) as i16 - 7) as u8;
        // s_add_rnd2pos y,255,255,0 — per-axis (rnd&m)-m/2; z draws but adds 0.
        let dx = (sf_random(&mut g.vars) & 255) as i16 - 127;
        let dy = (sf_random(&mut g.vars) & 255) as i16 - 127;
        let _dz = (sf_random(&mut g.vars) & 0) as i16; // draw kept for RNG parity
        let al = &mut g.objs.aliens[dbr as usize];
        al.rotz = rotz;
        al.sbyte1 = sbyte1;
        al.roty = DEG90; // s_set_alvar B,y,al_roty,#deg90
        al.worldx = al.worldx.wrapping_add(dx as u16 as i16);
        al.worldy = al.worldy.wrapping_add(dy as u16 as i16);
        // s_set_strat y,SPINspacebar_Istrat. The init runs on the next strategy
        // dispatch, installs hard enemy1 vars, then the exact spacemist/spin/
        // roty-achase tick already shared with map-spawned spacebars.
        al.stratptr = g.world.istrats[MAP_ISTRAT_SPINSPACEBAR];
    }
    colony0_cont(g, idx);
}

/// `colony0_cont` (GA2STRAT.ASM:1727-1730): scroll the structure toward the
/// player (`al_worldz += -20`) and ride the world scroll (`s_add_playerZ`).
fn colony0_cont(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add((-20i16) as u16 as i16);
    add_player_z(g, idx);
}

/// `colony1_Istrat` (GA2STRAT.ASM:1734-1738): colldisable/gnd, tick=colony1_strat.
/// Falls into the tick this frame.
fn colony1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, colony1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_strat x,colony1_strat
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.type_ |= ATGND; // s_set_altype x,gnd
    }
    colony1_strat(g, idx);
}

/// `colony1_strat` (GA2STRAT.ASM:1739-1754): pin worldy to `2*viewcy - viewposy
/// + 50` (mirror the camera vertical), then run colony0_cont.
fn colony1_strat(g: &mut Game, idx: u16) {
    let viewcy = g.vars.sv_i16(sv::VIEWCY) as i32;
    let viewposy = g.vars.sv_i16(sv::VIEWPOSY) as i32;
    // -(viewposy - viewcy) + viewcy + 50 = 2*viewcy - viewposy + 50.
    let worldy = 2 * viewcy - viewposy + 50;
    g.objs.aliens[idx as usize].worldy = worldy as i16;
    colony0_cont(g, idx); // s_brl colony0_cont
}

// ------------------------------------------------------------
// colony2 (IS 172) — GA2STRAT.ASM:1758-1779.
// ------------------------------------------------------------

/// `colony2_Istrat` (GA2STRAT.ASM:1758-1763): colldisable, tick=colony2_strat,
/// anim 0, gnd. Falls into the tick this frame.
fn colony2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, colony2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.stratptr = Some(tick); // s_set_strat x,colony2_strat
        al.animframe = 0; // s_init_anim x,#0
        al.type_ |= ATGND; // s_set_altype x,gnd
    }
    colony2_strat(g, idx);
}

/// `colony2_strat` (GA2STRAT.ASM:1764-1779): position the door at its al_ptr
/// parent + 280 z, and open (anim 0->9) when the player is in front OR within
/// 40 z. Scoped-out: the ported maps do not wire colony2's `al_ptr` parent link,
/// so when unset the door holds its map placement instead of tracking a parent
/// (the ROM's `s_set_objtobealvar y,x,al_ptr` -> `s_copy_pos x,y` is honoured
/// only when the link resolves to a live object).
fn colony2_strat(g: &mut Game, idx: u16) {
    // s_set_objtobealvar y,x,al_ptr ; s_copy_pos x,y (colony2 <- parent).
    let ptr = g.objs.aliens[idx as usize].ptr; // al_ptr, index+1 encoding.
    if ptr != 0 {
        let parent = ptr - 1;
        if (parent as usize) < NUMBER_AL && g.objs.aliens[parent as usize].active {
            copy_pos(g, idx, parent);
        }
    }
    // s_add_alvar W,x,al_worldz,#70<<2
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(70 << 2);
    // s_jmp_objinfront y,x,.open (player in front) ; else s_jmp_Zdistmore #40,.nopen.
    let animate = if player_in_front_of_self(g, idx) {
        true
    } else {
        !zdist_more(g, idx, 10 << 2)
    };
    if animate {
        // .open: s_cmp_anim #9 / s_beq .nopen / s_add_anim x,#1,#10 (wrap 10).
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe != 9 {
            al.animframe = (al.animframe + 1) % 10;
        }
    }
}

// ------------------------------------------------------------
// colonyexit (IS 236) — GA2STRAT.ASM:3039-3053.
// ------------------------------------------------------------

/// `colonyexit_Istrat` (GA2STRAT.ASM:3039-3053): a self-recurring door (it never
/// sets a separate tick, so this function is both the istrat and the per-frame
/// tick). Re-asserts colldisable/gnd, then opens (anim 0->9) when the player is
/// in front and beyond `medpspeed+10` (75) z; snaps shut (anim 0) when the player
/// is behind it or inside 75 z. `s_nodepthcue` is a render fog-exclusion flag.
/// This is NOT a stage-transition — the ASM sets no LE_/levelfinished.
fn colonyexit_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.type_ |= ATGND; // s_set_altype x,gnd
    }
    // s_jmp_objinfront x,y,.close ; s_jmp_Zdistless x,y,#medpspeed+10,.close.
    if self_in_front_of_player(g, idx) || zdist_less(g, idx, (MEDPSPEED as i16) + 10) {
        g.objs.aliens[idx as usize].animframe = 0; // .close: s_init_anim x,#0
        return;
    }
    // s_dooropen_snd 0 ; s_cmp_anim #9 / s_beq .end / s_add_anim x,#1,#10.
    if g.objs.aliens[idx as usize].animframe & 0x7F == 0 {
        door_family_sound(g, idx, PosSndFamilyId::DoorOpen);
    }
    let al = &mut g.objs.aliens[idx as usize];
    if al.animframe != 9 {
        al.animframe = (al.animframe + 1) % 10;
    }
}

// ============================================================
// Environmental hazards — trackcorner / windmill / volcano / firepillar (+
// their volplasma / volrock / volrockdown projectile children). ASM is the
// sole ground truth (no C-oracle): GASTRATS.ASM (trackcorner:1626-1630,
// windmill:3528-3570), GA2STRAT.ASM (volcano:1929-2033, firepillar/
// volrockdown:2039-2127). The runtime uses the exact zero-based ISTRATS.ASM
// rows: trackcorner=49, windmill=66, flypillars=73, volcano=191 and
// firepillar=193. All five are reachable from the ported maps/mother maps.
//
// State machines (per-fn cites):
//   trackcorner : indestructible (hardHP/ap0), NO colltype, NO strat/coll/exp
//                 ptrs — a pure static render marker (the rail corner mesh).
//   windmill    : hp6/ap4 spinner. In [500,2000) z, on the notdelay-1 gate,
//                 turns the whole mill (roty += sword1); every tick scrolls,
//                 coasts along roty at speed 50, and spins the blades (rotz+=4).
//   volcano     : indestructible (hardHP/hardAP) enemy1 emitter facing deg180.
//                 In [600,4000) z it rumbles once (sflag1 latch) and lobs a
//                 homing volplasma on the notdelay-4 gate; a ballistic volrock
//                 is thrown on the notdelay-3 gate REGARDLESS of range (the
//                 out-of-range branch lands past the plasma block).
//   firepillar  : indestructible (hardHP/hardAP) enemy1 pillar. Init randomises
//                 worldx around player_posx/2, faces deg180 upside-down
//                 (rotz=deg180); a 30% coin latches sflag2 = permanently inert.
//                 Active: within 1000 z it latches sflag1 + rumbles once (the
//                 particlefiredown burst is cosmetic); within 800 z it drops a
//                 volrockdown on the notdelay-2 gate.
//   volplasma   : hp2/plasmaAP homing fireball. Within 500 z it 3D-homes the
//                 player (rate 2) and regenerates its velocity; clamps worldy
//                 to <=0; double-integrates (add_vecs2pos, scroll, add_vecs2pos).
//   volrock     : hp2/plasmaAP ballistic rock. Random heading (rnd&127)+64,
//                 speed (rnd&7)+15, upward vy -(rnd&15)-30, then bounces on the
//                 y=0 floor forever (falldown, no removal).
//   volrockdown : hp2/plasmaAP downward-erupting rock. Rises (vy=80) until y>=0,
//                 then scatters (random vx/vy/vz), advances state and falls under
//                 gravity, self-removing when the bounce decays.
//
// Scoped-out cosmetics (never asserted): particlefire/particlefiredown fire
// emitters, make_smoke trails, s_rots_flat billboard orient, s_sprite_obj, the
// windmill's four SLOWELASER "smoke" jets (GASTRATS.ASM:3550-3569, ASM-commented
// "smoke") and windexp's round0p blade shower (GASTRATS.ASM:3573-3599) — routed
// through strat_explode instead.
// ============================================================

const IS_TRACKCORNER: usize = 49;
const IS_WINDMILL: usize = 66;
const IS_VOLCANO: usize = 191;
const IS_FIREPILLAR: usize = 193;

const FLYPILLAR_HP: u8 = 12;
const FLYPILLAR_AP: u8 = 16;

const WINDMILL_HP: u8 = 6; // STRATEQU.INC:102 windmillHP
const WINDMILL_AP: u8 = 4; // STRATEQU.INC:103 windmillAP
                           // HARD_AP (hardAP = 8, STRATEQU.INC:66) is defined once above (colony section).
const PLASMA_AP: u8 = 10; // STRATEQU.INC:86 plasmaAP
const HAZARD_PROJ_HP: u8 = 2; // vol* s_set_aldata #2,#plasmaAP

/// al_sflags2 sflag2 bit (STRATEQU.INC:914) — firepillar's "inert" latch.
const ASF2_SFLAG2: u8 = 0x20;

const SH_FIREBALL: u16 = 402;

/// `s_jmp_random label,#pct` (STRATMAC.INC:1407-1417): branch when
/// `random_l() < (pct*255)/100`. (The 1-arg form == pct 50 -> jmp_random50.)
fn jmp_random_pct(g: &mut Game, pct: u32) -> bool {
    (sf_random(&mut g.vars) & 0xff) < ((pct * 255) / 100) as u16
}

/// `s_jmp_outZdistrng x,y,#min,#max` inverted: TRUE when the player is IN range
/// (`min <= |dz| < max`) — the fall-through side (STRATMAC.INC:3354 ->
/// jmp_outdistrng, out-of-range branches away).
fn zdist_in_range(g: &Game, idx: u16, min: i32, max: i32) -> bool {
    match player(g) {
        Some(p) => {
            let d = abs_axis_dist(g.objs.aliens[idx as usize].worldz, p.worldz);
            d >= min && d < max
        }
        None => false,
    }
}

/// `s_jmp_notdelay N,al1pt` gate with the per-object index stagger (al1pt -> idx,
/// port convention, e.g. meteo0 enemies_ground.rs:1804): TRUE when
/// `(gameframe + idx) & ((1<<N)-1) == 0`.
fn notdelay_staggered(g: &Game, idx: u16, bits: u16) -> bool {
    g.vars.gameframe.wrapping_add(idx) & ((1u16 << bits) - 1) == 0
}

/// `s_falldown_Yvec obj,shift,gravity,ground` (STRATMAC.INC:1813): add gravity
/// to vy; while still above `ground` (worldy < ground) stay airborne; on/below
/// ground snap to it and bounce (vy = (-vy) >> shift; a small residual -> 0).
/// Returns true once the bounce has decayed to rest (the optional `remove`
/// label fires). Byte-identical to bosses.rs boss2_falldown_yvec.
fn falldown_yvec(g: &mut Game, idx: u16, shift: u32, gravity: i16, ground: i16) -> bool {
    let al = &mut g.objs.aliens[idx as usize];
    al.vy = al.vy.wrapping_add(gravity); // s_add_2Yvec
    if al.worldy < ground {
        return false; // s_jmp_higher — still airborne
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

/// `flypillar_Istrat` / `flypillar_strat` (GASTRATS.ASM:2365-2402).
/// The map-placed version rises from a random X around the player, travels
/// forward until its random Z trigger, then levels its pitch and climbs to the
/// ground plane before becoming ordinary removable scenery.
pub fn flypillar_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, flypillar_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, crate::enemy_a::pillar3explode_strat);
    let trigger = (sf_random(&mut g.vars) as i16).wrapping_add(200);
    let xlo = sf_random(&mut g.vars) as u16;
    let xhi = sf_random(&mut g.vars) & 1;
    let worldx = ((xlo | (xhi << 8)) as i16)
        .wrapping_sub(256)
        .wrapping_add(g.vars.player_posx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = FLYPILLAR_HP;
        al.ap = FLYPILLAR_AP;
        al.type_ &= !ATZREMOVE;
        al.rotx = 0u8.wrapping_sub(DEG90);
        al.vz = 60;
        al.sword1 = trigger;
        al.worldx = worldx;
        al.snd2 = 4;
    }
    flypillar_strat(g, idx);
}

fn flypillar_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let trigger = g.objs.aliens[idx as usize].sword1;
    if !zdist_less(g, idx, trigger) {
        achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 2);
        if g.objs.aliens[idx as usize].worldy >= 0 {
            g.hooks.play_se(0x24); // se_damageenemynear
            let al = &mut g.objs.aliens[idx as usize];
            al.stratptr = None;
            al.rotx = 0;
            al.type_ |= ATZREMOVE;
            al.sflags &= !ASF_SHADOW;
            return;
        }
        g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize].worldy.wrapping_add(20);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
}

fn door_family_sound(g: &mut Game, idx: u16, family: PosSndFamilyId) {
    let al = g.objs.aliens[idx as usize];
    g.hooks.make_snd(family, al.worldx, al.worldz);
}

/// Distinct `base_1_Istrat` door (D3STRATS.ASM:1038-1071), used by route 3
/// and training.  This is not KSTRATS' hit-triggered `base1_Istrat`.
pub fn base_1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, base_1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARDHP;
        al.ap = HARD_AP;
        al.sflags |= ASF_NOHITAFFECT;
        al.animframe = 0x80;
        al.sbyte1 = 0;
        al.sflags2 &= !ASF2_SFLAG1;
    }
    base_1_strat(g, idx);
}

pub fn base_1_strat(g: &mut Game, idx: u16) {
    // sflag1 selects the opening/hold half of the autonomous cycle.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        let anim = g.objs.aliens[idx as usize].animframe & 0x7f;
        if anim != 0 {
            // `s_dooropen_snd 7` only fires as frame 7 starts closing.
            if anim == 7 {
                door_family_sound(g, idx, PosSndFamilyId::DoorOpen);
            }
            add_anim_wrap(&mut g.objs.aliens[idx as usize], 0xff, 8);
            return;
        }
        let wait = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
        g.objs.aliens[idx as usize].sbyte1 = wait;
        if wait != 10 {
            return;
        }
        g.objs.aliens[idx as usize].sbyte1 = 0;
        // ROM branches directly to `.other2`, beginning the close half now.
    }

    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
    let anim = g.objs.aliens[idx as usize].animframe & 0x7f;
    if anim == 0 {
        door_family_sound(g, idx, PosSndFamilyId::DoorClose);
    }
    if add_anim_cap(&mut g.objs.aliens[idx as usize], 1, 8) {
        let wait = g.objs.aliens[idx as usize].sbyte1.wrapping_add(1);
        g.objs.aliens[idx as usize].sbyte1 = wait;
        if wait == 10 {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte1 = 0;
            al.sflags2 |= ASF2_SFLAG1;
        }
    }
}

/// `trackcorner_Istrat` (GASTRATS.ASM:1626-1630): a pure static marker — alptrs
/// all 0 (no tick/coll/exp), aldata hardHP/ap0, s_end_strat. It sets no colltype,
/// so it is render-only scenery (the rail-corner mesh).
fn trackcorner_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = None; // s_set_alptrs x,0,0,0
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARDHP; // s_set_aldata #hardHP,#0
    al.ap = 0;
}

/// `windmill_Istrat` (GASTRATS.ASM:3528-3533): wire strats/data, speed 50, sound
/// $f. Death routes through `windexp_Istrat` (blade shower + particle + escapee).
/// No s_end_strat before the tick label -> falls into `windmill_strat`.
fn windmill_init(g: &mut Game, idx: u16) {
    let tick = sid(g, windmill_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, windexp_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WINDMILL_HP; // s_set_aldata #windmillHP,#windmillAP
        al.ap = WINDMILL_AP;
        al.vel = 50; // s_set_speed x,#50
        al.snd2 = 0x0f; // set_sound2 x,#$f
    }
    windmill_strat(g, idx);
}

const SH_ROUND0P: u16 = 400;

/// Spawn one windspin blade at a full-rotation local offset from the mill.
fn spawn_windspin_blade(g: &mut Game, parent: u16, ox: i16, oy: i16, oz: i16, rotz_add: u8) {
    let Some(blade) = make_obj(g, SH_ROUND0P) else {
        return;
    };
    let me = g.objs.aliens[parent as usize];
    {
        let al = &mut g.objs.aliens[blade as usize];
        al.rotx = me.rotx;
        al.roty = me.roty;
        al.rotz = me.rotz.wrapping_add(rotz_add);
    }
    full_offset_pos(g, blade, &me, ox, oy, oz);
    windspin_istrat(g, blade);
}

/// ROM `windexp_Istrat` (GASTRATS.ASM:3573) — four spinning blades + particle
/// shower, then escapeeexplode.
pub fn windexp_istrat(g: &mut Game, idx: u16) {
    // Offsets are `#N<<2` in ASM (already world units after <<2).
    spawn_windspin_blade(g, idx, 0, 120, 0, 0); // #0,#30<<2,#0
    spawn_windspin_blade(g, idx, 0, -120, 0, DEG180); // + rotz deg180
    spawn_windspin_blade(g, idx, -120, 0, 0, DEG90); // + rotz deg90
    spawn_windspin_blade(g, idx, 120, 0, 0, DEG90.wrapping_neg()); // - rotz deg90

    if let Some(p) = make_obj(g, 0) {
        copy_pos(g, p, idx);
        crate::enemy_a::particleexplode_istrat(g, p);
    }
    crate::enemy_a::escapeeexplode_istrat(g, idx);
}

/// ROM `windspin_Istrat` (GASTRATS.ASM:3616) — blade flies outward on its rotz.
pub fn windspin_istrat(g: &mut Game, idx: u16) {
    let s_tick = sid(g, windspin_strat);
    let s_exp = sid(g, strat_explode);
    let angle = g.objs.aliens[idx as usize].rotz;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(s_tick);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
        al.hp = HARDHP;
        al.ap = HARD_AP;
        al.count = 40; // lifecnt
                       // s_set_alvar2rnd sbyte1,#15 ; s_add_alvar sbyte1,#7 → [7..22]
        al.sbyte1 = ((sf_random(&mut g.vars) as u8) & 15).wrapping_add(7);
    }
    crate::enemy_a::make_xyvec(&mut g.objs.aliens[idx as usize], angle, 30);
}

/// ROM `windspin_strat` — spin + coast until lifecnt expires.
pub fn windspin_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        if al.count > 0 {
            al.count = al.count.wrapping_sub(1);
        }
        if al.count == 0 {
            g.objs.aldead = 1;
            return;
        }
        apply_velocity(al);
    }
}

/// `windmill_strat` (GASTRATS.ASM:3534-3570): turn the mill (roty += sword1)
/// while in [500,2000) z on the every-other-frame gate; then scroll, coast along
/// roty, and spin the blades (rotz += 4). The 4 SLOWELASER "smoke" jets are
/// cosmetic (scoped).
fn windmill_strat(g: &mut Game, idx: u16) {
    // s_jmp_OUTZdistrng x,y,#500,#2000,.nrot ; s_jmp_notdelay 1,.nrot ;
    // s_add_alvars B,x,al_roty,x,al_sword1.
    if zdist_in_range(g, idx, 500, 2000) && notdelay(g, 1) {
        let sw = g.objs.aliens[idx as usize].sword1 as u8; // byte add of sword1's low byte
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(sw);
    }
    add_player_z(g, idx); // s_add_playerZ x
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]); // s_gen_3dvecs x,al_roty,al_rotx,al_vel
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos x
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(4);
}

/// `volcano_Istrat` (GA2STRAT.ASM:1929-1941): volcano_strat + alptrs 0,0 (no
/// collide/explode handler — hardHP makes it indestructible anyway), hardHP/
/// hardAP, enemy1, face deg180, clear sflag1. The particlefire child is cosmetic
/// (scoped). No s_end_strat -> falls into `volcano_strat`.
fn volcano_init(g: &mut Game, idx: u16) {
    let tick = sid(g, volcano_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None; // s_set_alptrs x,volcano_strat,0,0
        al.expstratptr = None;
        al.hp = HARDHP; // s_set_aldata #hardHP,#hardAP
        al.ap = HARD_AP;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        al.sflags2 &= !ASF2_SFLAG1; // s_clr_alsflag x,sflag1
    }
    volcano_strat(g, idx);
}

/// `volcano_strat` (GA2STRAT.ASM:1942-1966): in [600,4000) z, rumble once and
/// lob a homing volplasma on the notdelay-4 gate; a ballistic volrock is thrown
/// on the notdelay-3 gate in ALL ranges (the out-of-range `.nfire` branch lands
/// after the plasma block, before the volrock block). Both children spawn at the
/// volcano pose, worldy - 120 (-30<<2).
fn volcano_strat(g: &mut Game, idx: u16) {
    if zdist_in_range(g, idx, 600, 4000) {
        // s_jmp_alsflag x,sflag1,.nsnd ; s_set_alsflag ; trigse $9a.
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            crate::common::strat_trig_se(g, 0x9a);
        }
        // s_jmp_notdelay 4,.nfire,al1pt -> homing volplasma.
        if notdelay_staggered(g, idx, 4) {
            spawn_vol_child(g, idx, true);
        }
    }
    // .nfire: s_jmp_notdelay 3,.nfire2,al1pt -> ballistic volrock (any range).
    if notdelay_staggered(g, idx, 3) {
        spawn_vol_child(g, idx, false);
    }
}

/// Shared volcano child spawn: `s_make_obj #fireball ; s_set_strat y,<Istrat> ;
/// s_copy_pos y,x ; s_add_alvar W,y,al_worldy,#-30<<2`. `plasma` picks the
/// volplasma vs volrock init.
fn spawn_vol_child(g: &mut Game, idx: u16, plasma: bool) {
    if let Some(child) = make_obj(g, SH_FIREBALL) {
        copy_pos(g, child, idx);
        g.objs.aliens[child as usize].worldy =
            g.objs.aliens[child as usize].worldy.wrapping_sub(120);
        let s = if plasma {
            sid(g, volplasma_init)
        } else {
            sid(g, volrock_init)
        };
        g.objs.aliens[child as usize].stratptr = Some(s);
    }
}

/// `volplasma_Istrat` (GA2STRAT.ASM:1969-1980): hp2/plasmaAP homing fireball.
/// ROM stores the 3D aim in al_sbyte1/al_sbyte2 to keep roty/rotx free for the
/// cosmetic s_rots_flat (scoped); the port keeps the aim in roty/rotx so
/// strat_gen_vecs_3d / strat_aim_3d apply directly — roty=deg180 (toward camera),
/// rotx=-deg90 (straight up), speed 50. No s_end_strat -> falls into the tick.
pub fn volplasma_istrat(g: &mut Game, idx: u16) {
    volplasma_init(g, idx);
}

fn volplasma_init(g: &mut Game, idx: u16) {
    let tick = sid(g, volplasma_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HAZARD_PROJ_HP; // s_set_aldata #2,#plasmaAP
        al.ap = PLASMA_AP;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.roty = DEG180; // (== ROM al_sbyte1 = deg180)
        al.rotx = (-(DEG90 as i8)) as u8; // (== ROM al_sbyte2 = -deg90)
        al.vel = 50; // s_set_speed x,#50
        al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]); // s_gen_3dvecs sbyte1,sbyte2,vel
    volplasma_strat(g, idx);
}

/// `volplasma_strat` (GA2STRAT.ASM:1981-1999): within 500 z, 3D-home the player
/// (rate 2) and regenerate the velocity; move, clamp worldy to <=0 (never sinks
/// below ground), scroll, move again. make_smoke / rots_flat are cosmetic.
pub fn volplasma_strat(g: &mut Game, idx: u16) {
    // s_jmp_Zdistless x,y,#500,.cont — inside 500 z home + regen vecs.
    if zdist_less(g, idx, 500) {
        if let Some(pl) = player(g) {
            strat_aim_3d(g, idx, &pl, 2); // s_obj2obj_3dangle sbyte1,sbyte2,2
            gen_vecs_3d(&mut g.objs.aliens[idx as usize]); // s_gen_3dvecs
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos
                                                      // s_jmp_higher x,#0,.high — worldy<0 skip; else pin to ground 0.
    if g.objs.aliens[idx as usize].worldy >= 0 {
        g.objs.aliens[idx as usize].worldy = 0;
    }
    add_player_z(g, idx); // s_add_playerZ
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_Add_vecs2pos (again)
}

/// `volrock_Istrat` (GA2STRAT.ASM:2004-2023): hp2/plasmaAP ballistic rock. Random
/// heading (rnd&127)+64 (ROM al_sbyte1; port stores in roty for nvecs),
/// random speed (rnd&7)+15, then gen the flat vecs and set an upward launch vy =
/// -(rnd&15)-30. No s_end_strat -> falls into the tick.
pub fn volrock_istrat(g: &mut Game, idx: u16) {
    volrock_init(g, idx);
}

fn volrock_init(g: &mut Game, idx: u16) {
    let tick = sid(g, volrock_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    // s_set_alvar2rnd sbyte1,#127 ; s_add_alvar #deg180-64.
    let heading = ((sf_random(&mut g.vars) as u8) & 127).wrapping_add(DEG180.wrapping_sub(64));
    // s_set_var2rnd svar,#7 ; s_add_var #15.
    let speed = ((sf_random(&mut g.vars) as u8) & 7).wrapping_add(15);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HAZARD_PROJ_HP;
        al.ap = PLASMA_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow
        al.roty = heading;
        al.vel = speed;
    }
    strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]); // s_gen_vecs → nvecs_l
                                                            // s_set_alvar2rnd al_vy,#15 ; vy+1=0 ; s_neg_alvar ; s_add_alvar #-30.
    let vy = -(((sf_random(&mut g.vars) as u8) & 15) as i16) - 30;
    g.objs.aliens[idx as usize].vy = vy;
    volrock_strat(g, idx);
}

/// `volrock_strat` (GA2STRAT.ASM:2024-2033): gravity + bounce on the y=0 floor
/// (never removed), then coast. make_smoke / rots_flat are cosmetic.
pub fn volrock_strat(g: &mut Game, idx: u16) {
    let _ = falldown_yvec(g, idx, 1, 2, 0); // s_falldown_Yvec x,1,#2,#0 (no remove label)
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_Add_vecs2pos
}

/// `firepillar_Istrat` (GA2STRAT.ASM:2039-2062): indestructible enemy1 pillar,
/// faces deg180 upside-down (rotz=deg180). Init randomises worldx to a
/// (0..1023)-512 offset then adds player_posx/2 (asra); a 30% coin latches sflag2
/// = permanently inert (the 70%-branch of jmp_random skips setting it). The
/// particlefiredown burst is cosmetic. No s_end_strat -> falls into the tick.
fn firepillar_init(g: &mut Game, idx: u16) {
    let tick = sid(g, firepillar_strat);
    // s_set_alvar2rnd worldx (low byte) ; +1,#3 (high byte & 3) -> 0..1023.
    let lo = (sf_random(&mut g.vars) as u8) as i16;
    let hi = ((sf_random(&mut g.vars) as u8) & 3) as i16;
    let mut wx = lo | (hi << 8);
    wx -= 512; // s_sub_alvar W,x,al_worldx,#512
    wx = wx.wrapping_add(g.vars.player_posx >> 1); // asra player_posx (/2) + worldx
                                                   // jmp_random .ndrop,70 branches (skips the sflag2 set) on 70% -> inert 30%.
    let inert = !jmp_random_pct(g, 70);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None; // s_set_alptrs x,firepillar_strat,0,0
        al.expstratptr = None;
        al.hp = HARDHP; // s_set_aldata #hardhp,#hardAP
        al.ap = HARD_AP;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        al.rotz = DEG180; // s_set_alvar B,x,al_rotz,#deg180
        al.worldx = wx;
        if inert {
            al.sflags2 |= ASF2_SFLAG2; // s_set_alsflag x,sflag2
        }
    }
    firepillar_strat(g, idx);
}

/// `firepillar_strat` (GA2STRAT.ASM:2064-2090): sflag2 pillars are inert. Active:
/// within 1000 z, first time only, latch sflag1 + rumble (particlefiredown
/// cosmetic); within 800 z, on the notdelay-2 gate, drop a volrockdown.
fn firepillar_strat(g: &mut Game, idx: u16) {
    // s_jmp_alsflag x,sflag2,.nfire (== END).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0 {
        return;
    }
    // s_jmp_Zdistmore #1000,.badobj ; s_jmp_alsflag sflag1,.badobj.
    if zdist_less(g, idx, 1000) && g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
        crate::common::strat_trig_se(g, 0x49); // trigse $49
    }
    // s_jmp_Zdistmore #800,.nfire ; s_jmp_notdelay 2,.nfire,al1pt -> volrockdown.
    if zdist_less(g, idx, 800) && notdelay_staggered(g, idx, 2) {
        if let Some(child) = make_obj(g, SH_FIREBALL) {
            copy_pos(g, child, idx); // s_copy_pos y,x (worldy offset line is ASM-commented)
            let s = sid(g, volrockdown_init);
            g.objs.aliens[child as usize].stratptr = Some(s);
        }
    }
}

/// `volrockdown_Istrat` (GA2STRAT.ASM:2095-2101): hp2/plasmaAP rock launched
/// downward (`s_set_vecs #0,#80,#0`). No s_end_strat -> falls into the tick.
pub fn volrockdown_istrat(g: &mut Game, idx: u16) {
    volrockdown_init(g, idx);
}

fn volrockdown_init(g: &mut Game, idx: u16) {
    let tick = sid(g, volrockdown_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HAZARD_PROJ_HP;
        al.ap = PLASMA_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.vx = 0; // s_set_vecs x,#0,#80,#0
        al.vy = 80;
        al.vz = 0;
    }
    volrockdown_strat(g, idx);
}

/// `volrockdown_strat` (GA2STRAT.ASM:2102-2127): state 0 rises (vy=80) until
/// worldy>=0, then scatters (random vx/vy/vz), advances to state 1 and integrates
/// once more; state 1 falls under gravity and self-removes when the bounce decays.
/// The leading add_vecs2pos runs every tick. rots_flat is cosmetic.
pub fn volrockdown_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_Add_vecs2pos (leading)
                                                      // s_jmp_ifnotstate x,0,.nsdown ; s_jmp_higher x,#0,.nsdown (worldy<0 skip).
    if g.objs.aliens[idx as usize].stratstate == 0 && g.objs.aliens[idx as usize].worldy >= 0 {
        let vx = (((sf_random(&mut g.vars) as u8) & 15) as i16) - 7; // (rnd&15)-7
        let vy = (((sf_random(&mut g.vars) as u8) & 7) as i16) - 15; // (rnd&7)-15 (upward pop)
        let vz = (((sf_random(&mut g.vars) as u8) & 15) as i16) - 7; // (rnd&15)-7
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vx = vx;
            al.vy = vy;
            al.vz = vz;
        }
        next_state(g, idx); // s_next_state
        g.objs.aliens[idx as usize].worldy = 0; // s_set_alvar W,x,al_worldy,#0
        apply_velocity(&mut g.objs.aliens[idx as usize]); // s_Add_vecs2pos
    }
    // .nsdown: s_jmp_ifnotstate x,1,.nsbounce ; s_falldown_Yvec x,1,#2,#0,remove.
    if g.objs.aliens[idx as usize].stratstate == 1 && falldown_yvec(g, idx, 1, 2, 0) {
        g.objs.aliens[idx as usize].type_ |= ATZREMOVE; // remove_istrat
    }
}

// ============================================================
// Firing-enemy family — misspod / misstank / szaco0 / szaco5 / houdai5f.
//
// ASM is the sole ground truth (no C-oracle for any of these):
//   misspod   GASTRATS.ASM:3275-3395   misstank  GASTRATS.ASM:1319-1436
//   szaco0    GA2STRAT.ASM:329-357     szaco5    GA2STRAT.ASM:478-527
//   houdai5f  KSTRATS.ASM:588-608      + STRATMAC s_goto_WPpostab:2510-2583.
//
// ISTRATS.ASM def_istrat rows -> sf-map placement indices:
//   misspod  = 68   (level1_5 / route3 common.rs)         [macro-count == 68]
//   szaco0   = 130  (level1_2 / route2_5)                 [macro-count == 130]
//   szaco5   = 156  (level1_3)                            [macro-count == 156]
//   houdai5f = 188  (route3 level3_7 / route2 rc.rs / level1_4)
//                   [ISTRATS.ASM:618; was wrongly 187 when hard90yrfog@183
//                    was skipped in the tank index count]
//   misstank = 51   (ISTRATS.ASM:471, one after trackcorner:470=50)
//                   [macro-count == 51].
//
// KNOWN sf-map BUG (outside this lane's edit scope): route2 `rc.rs:141`
// declares `IS_MISSTANK = 50`, which COLLIDES with trackcorner (correctly 50,
// ISTRATS.ASM:470, already registered+tested here). 50 is a mislabel — the true
// misstank istrat index is 51 (its def_istrat is the very next row). We register
// misstank at the ROM-correct 51 and leave trackcorner's 50 intact; until
// rc.rs's const is fixed to 51, route2's misstank cspecials resolve to
// trackcorner (inert scenery) rather than spawning the tank.
//
// SYNTH IS SKIPPED (false gap): the doc's `IS_SYNTH` is `mothers.rs:146
// const IS_SYNTH = 0x020000` — the synthetic-strategy-address namespace base,
// NOT a placed enemy. No `synth_Istrat` exists anywhere in the reference ASM
// and no map cspecial spawns it; the reachable-doc scanner picked up the
// identifier by name. Nothing to port.
//
// Fidelity scope-outs (cosmetic, asserted-around, never asserted):
//   fog (s_initfog/s_dofog), engine-flame sprites (makeengine_srou), smoke
//   (makesmoke_srou), death-debris meshes (s_set_debrisdata + relexplode),
//   escapee/smark explosion variants (escapeeexplode2 / smarkexplode reduce to
//   the generic explode here), and positional sound. Visual weapon meshes are
//   shape-0 invisible like every other ported projectile.
// ============================================================

const IS_MISSTANK: usize = 50; // ISTRATS.ASM:471 (sf-map rc.rs's 50 is a mislabel — see above)
const IS_MISSPOD: usize = 67;
const IS_SZACO0: usize = 129;
const IS_SZACO5: usize = 155;
const IS_HOUDAI5F: usize = 187;

// HP/AP (INC/STRATEQU.INC + KSTRATS.ASM equs).
const MISSPOD_HP: u8 = 2; // STRATEQU.INC:112 misspodHP
const MISSPOD_AP: u8 = 16; // STRATEQU.INC:113 misspodAP
const MISSTANK_HP: u8 = 4; // STRATEQU.INC:150 misstankHP
const MISSTANK_AP: u8 = 8; // STRATEQU.INC:151 misstankAP
const SZACO0_HP: u8 = 4; // STRATEQU.INC:158 Szaco0HP
const SZACO0_AP: u8 = 8; // STRATEQU.INC:159 Szaco0AP
const SZACO5_HP: u8 = 2; // STRATEQU.INC:162 Szaco5HP
const SZACO5_AP: u8 = 8; // STRATEQU.INC:163 Szaco5AP
const HOUDAI5_HP: u8 = 4; // KSTRATS.ASM:47 houdai5HP
const HOUDAI5_AP: u8 = 6; // KSTRATS.ASM:48 houdai5AP

// missile2 projectile facts (fire_missile2, GSTRATS.ASM:2740-2749).
const MISSILE2_HP: u8 = 2; // STRATEQU.INC:82 missile2HP
const MISSILE2_AP: u8 = 4; // STRATEQU.INC:83 missile2AP
const MISSILE2_SPEED: u8 = 30; // GSTRATS.ASM:2746 s_set_speed #30
const MISSILE2_LIFE: u8 = 100; // GSTRATS.ASM:2747 s_set_lifecnt #100

// houdai5f homing-Hplasma override (KSTRATS.ASM:605-606).
const HOUDAI5F_SHOT_SPEED: u8 = 100; // s_set_speed y,#100
const HOUDAI5F_SHOT_LIFE: u8 = 100; // s_set_lifecnt y,#100
                                    // s_weapon_pos #0,#(-59<<2)>>weapon_scale,#0 -> <<weapon_scale(2) world muzzle.
const HOUDAI5F_MUZZLE_Y: i16 = ((-59i16 << 2) >> 2) << 2; // = -236

// (ASF2_SFLAG2 = sflags2 0x20 is already defined earlier in this module.)

/// `-deg11` misstank turret pitch (VARS.INC deg11 = deg360/32 = 8).
const NEG_DEG11: u8 = 0u8.wrapping_sub(8);

// ============================================================
// missile2 — misspod's launched homing missile (fire_missile2 +
// missile2_Istrat/missile2a_strat, GSTRATS.ASM:2740/1834-1856).
// ============================================================

/// `missile2a_strat` (GSTRATS.ASM:1846-1856): slow homing missile that pitches
/// to `deg180` (dead-ahead, toward the player screen) and speeds up to 100 when
/// within 600 z, spinning `rotz` as it flies. ROM removal is off-screen
/// (`missboundchkexp`); here the `lifecnt`(100) countdown frees it (scope-out).
fn missile2a_strat(g: &mut Game, idx: u16) {
    // s_jmp_alsflag sflag2,.nspi ; s_jmp_Zdistmore #600 ; s_speedto #100,5.
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0 && !zdist_more(g, idx, 600) {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 100, 5);
    }
    // s_gen_3dvecs ; s_add_playerz ; s_add_vecs2pos.
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_dec_lifecnt.
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.count > 0 {
            al.count -= 1;
        }
    }
    // s_achase_2alvars rotx->0 rate2, roty->deg180 rate2 ; rotz += 10.
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, 0, 2);
        let mut roty = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut roty, DEG180, 2);
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = rotx;
        al.roty = roty;
        al.rotz = al.rotz.wrapping_add(10);
    }
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.free(idx);
    }
}

/// One `s_fire_weapon x,missile2` from misspod: build a #missile object at the
/// (full-rotation-rotated) muzzle offset with the firer's current rots saved
/// into sbyte1/sbyte2 (missile2_Istrat, GSTRATS.ASM:1834-1843), running
/// `missile2a_strat`. Reads the firer's live rotx/roty (misspoda sets them per
/// shot) and rotz (forced 0 by misspoda), matching `gen_weapon`.
fn misspod_spawn_missile2(g: &mut Game, owner: u16, mx: i16, my: i16, mz: i16) {
    let me = g.objs.aliens[owner as usize];
    let (rx, ry, rz) = rotate_full_offset(&me, mx, my, mz);
    let Some(shot) = make_obj(g, 0) else {
        return;
    };
    let tick = sid(g, missile2a_strat);
    let coll = sid(g, strat_explode);
    let al = &mut g.objs.aliens[shot as usize];
    al.shape = 0;
    al.sflags |= ASF_INVISIBLE | ASF_SHADOW; // missile2_Istrat s_set_alsflag shadow
    al.type_ |= ATMISSILE | ATZREMOVE; // gen_weapon ATMISSILE + setremove_behind
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(coll);
    al.hp = MISSILE2_HP;
    al.ap = MISSILE2_AP;
    al.vel = MISSILE2_SPEED;
    al.count = MISSILE2_LIFE;
    al.snd2 = 2;
    al.worldx = me.worldx.wrapping_add(rx);
    al.worldy = me.worldy.wrapping_add(ry);
    al.worldz = me.worldz.wrapping_add(rz);
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = 0;
    al.sbyte1 = me.rotx; // missile2_Istrat: al_sbyte1 = orig rotx
    al.sbyte2 = me.roty; // al_sbyte2 = orig roty
    al.immuneptr = owner;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | COLLTYPE_ENEMYWEAP;
    gen_vecs_3d(al);
    // ROM `s_fire_weapon x,missile2` → gen_weapon `jsl missilesound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Missile, me.worldx, me.worldz);
}

// ============================================================
// misspod (IS 68) — GASTRATS.ASM:3275-3395.
// al_sbyte1 = rnd&3 selects the burst pattern: 0/3 = X, 1 = H, 2 = V.
// ============================================================

/// `misspod_Istrat` (GASTRATS.ASM:3275-3283): drift backward (`vz=-10`) facing
/// `deg180`, roll `rotz`, until the player closes.
fn misspod_init(g: &mut Game, idx: u16) {
    let tick = sid(g, misspod_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let r = (sf_random(&mut g.vars) as u8) & 3; // s_set_alvar2rnd al_sbyte1,#3
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = MISSPOD_HP;
    al.ap = MISSPOD_AP;
    al.vx = 0; // s_set_vecs x,#0,#0,#-10
    al.vy = 0;
    al.vz = -10;
    al.roty = DEG180;
    al.sbyte1 = r;
    al.snd2 = 2;
    // s_end_strat: no fall-through (unlike szaco0/szaco5/houdai5f).
}

/// `misspod_strat` (GASTRATS.ASM:3284-3293): coast + roll until the player is
/// within 1000 xz, then hand off to `misspoda_strat` (fires the burst THIS
/// tick — misspoda_init has no s_end_strat, GASTRATS.ASM:3295-3297).
fn misspod_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        if (dist_xz(&g.objs.aliens[idx as usize], &pl) as i32) < 1000 {
            let s = sid(g, misspoda_strat);
            g.objs.aliens[idx as usize].stratptr = Some(s);
            g.hooks.play_se(0x49); // trigse $49
            return misspoda_strat(g, idx);
        }
    }
    // endmisspod: s_add_vecs2pos ; s_add_alvar rotz,#5.
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = al.rotz.wrapping_add(5);
}

/// `misspoda_init` (GASTRATS.ASM:3295-3297): swap tick to the burst strat and
/// fall through (no `s_end_strat`).
pub fn misspoda_init(g: &mut Game, idx: u16) {
    let s = sid(g, misspoda_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.hooks.play_se(0x49); // trigse $49 (GASTRATS.ASM:3297)
    misspoda_strat(g, idx);
}

/// `misspoda_strat` (GASTRATS.ASM:3298-3389): weapon_rot 0, fire the 5-missile
/// burst for this pod's pattern, then `s_kill_obj x` (hp0 + colldisable -> the
/// engine explodes it via `explode_Istrat`). rotz is pushed/zeroed for the
/// muzzle-rotation math and pulled back before the kill.
pub fn misspoda_strat(g: &mut Game, idx: u16) {
    let pattern = g.objs.aliens[idx as usize].sbyte1;
    let saved_rotz = g.objs.aliens[idx as usize].rotz;
    g.objs.aliens[idx as usize].rotz = 0; // s_set_alvar rotz,#0
    match pattern {
        2 => misspod_fire_v(g, idx),
        1 => misspod_fire_h(g, idx),
        _ => misspod_fire_x(g, idx), // 0 or 3
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.rotz = saved_rotz; // s_pull_alvar rotz
                          // s_kill_obj x.
    al.sflags |= ASF_COLLDISABLE;
    al.hp = 0;
}

/// misspodV (GASTRATS.ASM:3310-3335): vertical column — muzzle offsets on Y,
/// the outer pair pitched ±deg90.
fn misspod_fire_v(g: &mut Game, idx: u16) {
    set_pod_aim(g, idx, 0, DEG180);
    misspod_spawn_missile2(g, idx, 0, 0, 120); // #120>>2<<2
    misspod_spawn_missile2(g, idx, 0, 68, 0); // #70>>2<<2 = 68
    misspod_spawn_missile2(g, idx, 0, -68, 0);
    set_pod_aim(g, idx, DEG90, DEG180);
    misspod_spawn_missile2(g, idx, 0, -68, 0);
    set_pod_aim(g, idx, DEG90.wrapping_neg(), DEG180);
    misspod_spawn_missile2(g, idx, 0, -68, 0);
}

/// misspodH (GASTRATS.ASM:3337-3362): horizontal row — muzzle offsets on X, the
/// outer pair yawed ±deg90.
fn misspod_fire_h(g: &mut Game, idx: u16) {
    set_pod_aim(g, idx, 0, DEG180);
    misspod_spawn_missile2(g, idx, 0, 0, 120);
    misspod_spawn_missile2(g, idx, 68, 0, 0);
    misspod_spawn_missile2(g, idx, -68, 0, 0);
    set_pod_aim(g, idx, 0, DEG90);
    misspod_spawn_missile2(g, idx, -68, 0, 0);
    set_pod_aim(g, idx, 0, DEG90.wrapping_neg());
    misspod_spawn_missile2(g, idx, -68, 0, 0);
}

/// misspodX (GASTRATS.ASM:3364-3389): 5-way star from a point muzzle — ahead,
/// yaw ±deg90, pitch ±deg90.
fn misspod_fire_x(g: &mut Game, idx: u16) {
    set_pod_aim(g, idx, 0, DEG180);
    misspod_spawn_missile2(g, idx, 0, 0, 0);
    set_pod_aim(g, idx, 0, DEG90);
    misspod_spawn_missile2(g, idx, 0, 0, 0);
    set_pod_aim(g, idx, 0, DEG90.wrapping_neg());
    misspod_spawn_missile2(g, idx, 0, 0, 0);
    set_pod_aim(g, idx, DEG90, DEG180);
    misspod_spawn_missile2(g, idx, 0, 0, 0);
    set_pod_aim(g, idx, DEG90.wrapping_neg(), DEG180);
    misspod_spawn_missile2(g, idx, 0, 0, 0);
}

/// `s_set_alvar rotx / s_set_alvar roty` before a fire — the shot inherits these
/// via `gen_weapon`'s `s_copy_rots y,x`.
fn set_pod_aim(g: &mut Game, idx: u16, rotx: u8, roty: u8) {
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = rotx;
    al.roty = roty;
}

// ============================================================
// misstank (IS 50) — GASTRATS.ASM:1319-1436.
// A tank carrying one small_m missile on its back; launches it (as a woodsgo
// homing missile) when the player closes to within 1000 z, once.
// ============================================================

/// `misstank_Istrat` (GASTRATS.ASM:1327-1342): make the small_m child, wire it
/// as a hp4 shootable turret, stash it in `al_ptr`, then FALL THROUGH into
/// `misstank_strat` this tick (no s_end_strat before the label).
fn misstank_init(g: &mut Game, idx: u16) {
    let tick = sid(g, misstank_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, misstankexp_strat);
    let child_coll = sid(g, strat_hit_flash);
    let child_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.snd2 = 9;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = MISSTANK_HP;
        al.ap = MISSTANK_AP;
    }
    // s_make_obj #small_m ; wire child ; s_set_alvartobeobj x,al_ptr,y.
    if let Some(child) = make_obj(g, 0) {
        copy_pos(g, child, idx);
        let cal = &mut g.objs.aliens[child as usize];
        cal.rotx = NEG_DEG11; // s_set_alvar y,al_rotx,#-deg11
        cal.collflags |= COLLTYPE_ENEMY1;
        cal.stratptr = None; // s_set_alptrs y,0,... — passive, positioned by the tank
        cal.collstratptr = Some(child_coll);
        cal.expstratptr = Some(child_exp);
        cal.hp = 4; // s_set_aldata y,#4,#4
        cal.ap = 4;
        g.objs.aliens[idx as usize].ptr = child + 1; // al_ptr (index+1, 0 = none)
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= COLLTYPE_ENEMY1; // .badobj: s_set_colltype x,enemy1
        al.vel = 30; // s_set_speed x,#30
        al.sbyte1 = 20; // s_set_alvar al_sbyte1,#20
    }
    misstank_strat(g, idx); // fall-through
}

/// `misstank_strat` (GASTRATS.ASM:1343-1372): count `sbyte1` down 20 frames
/// (no facing), then face the player (roty->deg180) forever after. Each tick,
/// until launched, pins the missile 85 above the tank; when the player is
/// within 1000 z, launches it (woodsgo homing missile) and latches sflag1.
fn misstank_strat(g: &mut Game, idx: u16) {
    // s_beqdec al_sbyte1,.facepl / s_brl .nfacepl.
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        // .facepl: s_achase_alvar roty,#deg180,3 ; s_add_playerZ.
        let mut roty = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut roty, DEG180, 3);
        g.objs.aliens[idx as usize].roty = roty;
        add_player_z(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 -= 1;
    }
    // .nfacepl: s_jmp_alsflag sflag1,.nmiss (skip once launched).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let child_ref = g.objs.aliens[idx as usize].ptr;
        if child_ref != 0 {
            let child = child_ref - 1;
            // ROM writes the child pose unconditionally; we guard on `active`
            // so a shot-down carrier can't corrupt a recycled slot.
            if (child as usize) < NUMBER_AL && g.objs.aliens[child as usize].active {
                let self_roty = g.objs.aliens[idx as usize].roty;
                g.objs.aliens[child as usize].roty = self_roty; // copy roty
                copy_pos(g, child, idx); // s_copy_pos y,x
                let cal = &mut g.objs.aliens[child as usize];
                cal.worldy = cal.worldy.wrapping_add(-(65 + 20)); // #-(65+20)
                                                                  // s_jmp_Zdistmore #1000,.notgo -> launch only when within 1000.
                if !zdist_more(g, idx, 1000) {
                    let wtick = sid(g, woodsgo_strat);
                    let cal = &mut g.objs.aliens[child as usize];
                    cal.stratptr = Some(wtick); // s_set_strat y,woodsgo_strat
                    cal.sbyte2 = 40; // s_set_alvar y,al_sbyte2,#40
                    cal.vel = 60; // s_set_speed y,#60
                    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
                }
            }
        }
    }
    // s_gen_vecs x,al_roty,al_vel ; s_add_vecs2pos (no playerZ at the tail).
    strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// `misstankexp_Istrat` (GASTRATS.ASM:1319-1326): on death, if the missile was
/// never launched (sflag1 clear) kill the carried child, then explode.
pub fn misstankexp_istrat(g: &mut Game, idx: u16) {
    misstankexp_strat(g, idx);
}

fn misstankexp_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let child_ref = g.objs.aliens[idx as usize].ptr;
        if child_ref != 0 {
            let child = child_ref - 1;
            if (child as usize) < NUMBER_AL && g.objs.aliens[child as usize].active {
                let cal = &mut g.objs.aliens[child as usize];
                cal.sflags |= ASF_COLLDISABLE; // s_kill_obj y
                cal.hp = 0;
            }
        }
    }
    strat_explode(g, idx); // s_jmp smarkexplode_Istrat (smark cosmetic)
}

/// `woodsgo_strat` (GASTRATS.ASM:1398-1425): the launched missile — roll,
/// accelerate to 80, and (only while still `>=400` z from the player) count
/// `sbyte1` down and nudge its heading at the player once it wraps. Smoke trail
/// is cosmetic (omitted).
pub fn woodsgo_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(8); // s_add_alvar rotz,#8
    }
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 80, 1); // s_speedto #80,1
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
    // s_jmp_Zdistless #400,.nfire -> the decbne/home only runs when |dz| >= 400.
    if !zdist_less(g, idx, 400) {
        let sb1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte1 = sb1;
        if sb1 == 0 {
            g.objs.aliens[idx as usize].sbyte1 = 1;
            if let Some(pl) = player(g) {
                strat_aim_3d(g, idx, &pl, 2); // s_obj2obj_3dangle roty,rotx,2
            }
        }
    }
}

// ============================================================
// szaco0 (IS 130) — GA2STRAT.ASM:329-357 + s_goto_WPpostab
// (STRATMAC.INC:2510-2583). A waypoint follower: rnd&3 picks one of 4
// player-relative flight paths; waypoint 1 fires RELSLOWELASER at the player.
// ============================================================

const WP_FIRE: u8 = 1; // STRATEQU.INC:826 wp_fire

/// szaco0 waypoint tables (GA2STRAT.ASM:296-327). Each entry: (x, y, z, flags);
/// z is added to `player_posz` at runtime (no wp_fixpos flag).
type Wp = (i16, i16, i16, u8);
const SZACO0A: [Wp; 5] = [
    (0, 0, 2500, 0),
    (0, 0, 1600, WP_FIRE),
    (-400, -300, 1000, 0),
    (400, 400, 1000, 0),
    (0, 0, 200, 0),
];
const SZACO0B: [Wp; 5] = [
    (0, 0, 2500, 0),
    (0, 0, 1600, WP_FIRE),
    (400, -300, 1000, 0),
    (-400, 400, 1000, 0),
    (0, 0, 200, 0),
];
const SZACO0C: [Wp; 5] = [
    (0, 0, 2500, 0),
    (0, 0, 1600, WP_FIRE),
    (0, 0, 1000, 0),
    (400, -400, 1000, 0),
    (0, 0, 200, 0),
];
const SZACO0D: [Wp; 5] = [
    (0, 0, 2500, 0),
    (0, 0, 1600, WP_FIRE),
    (-200, 0, 1000, 0),
    (-400, 200, 1000, 0),
    (0, 0, 200, 0),
];

/// `s_obj2WP_angle` yaw: `nega(anglexy_abs)` (twin of enemy_a szaco2).
fn wp_yaw(dx: i16, dz: i16) -> u8 {
    sf_core::aim_angle::yanglexy_nega(dx, dz)
}

/// `s_obj2WP_angle` pitch: ROM `Xanglexabs_l` (Manhattan adjacent).
fn wp_pitch(dy: i16, dx: i16, dz: i16) -> u8 {
    sf_core::aim_angle::xanglexabs(dy, dx, dz)
}

/// `rangexz` from `xzdiffs_l` (STRATROU.ASM) — the approximate-Euclidean value
/// the WPdistmore compare reads.
fn wp_rangexz(dx: i16, dz: i16) -> i16 {
    sf_core::aim_angle::xzdiffs(dx, dz)
}

/// `s_next_state x,#max` (STRATMAC.INC): inc; keep if `<= max`, else wrap to 1.
fn next_state_cap(g: &mut Game, idx: u16, max: u8) {
    let s = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
    g.objs.aliens[idx as usize].stratstate = if s <= max { s } else { 1 };
}

/// `s_goto_WP x,x,1,#40,1,2,3,#300,#40,.fin` (STRATMAC.INC:2470-2503) as used by
/// szaco0's WPpostab: add velocity, brake/accelerate toward the WP, aim (rate 2)
/// with a skid-lagged heading (rate 3). Returns true when in range AND at min
/// speed (the `.fin` -> next-waypoint branch).
fn szaco0_goto_wp(g: &mut Game, idx: u16, wx: i16, wy: i16, wz: i16) -> bool {
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos
    let me = g.objs.aliens[idx as usize];
    let rxz = wp_rangexz(wx.wrapping_sub(me.worldx), wz.wrapping_sub(me.worldz));
    let hwp = (me.worldy as i32 - wy as i32).abs();
    let reached = if (rxz as i32) >= 300 || hwp >= 300 {
        // .f: s_speedto #40,1 (max speed, out of range).
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 1);
        false
    } else {
        // in range: s_speedto #40,2,.fin (min-dist speed; carry -> next WP).
        speed_to(&mut g.objs.aliens[idx as usize], 40, 2)
    };
    // .n: s_obj2WP_angle rate 2.
    let me = g.objs.aliens[idx as usize];
    let dx = wx.wrapping_sub(me.worldx);
    let dy = wy.wrapping_sub(me.worldy);
    let dz = wz.wrapping_sub(me.worldz);
    let mut roty = me.roty;
    achase_angle(&mut roty, wp_yaw(dx, dz), 2);
    let mut rotx = me.rotx;
    achase_angle(&mut rotx, wp_pitch(dy, dx, dz), 2);
    // skid = 3: al_skidy chases roty (rate 3); vecs generated from skidy.
    let mut skidy = me.skidy;
    achase_angle(&mut skidy, roty, 3);
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = roty;
    al.rotx = rotx;
    al.skidy = skidy;
    // s_gen_3dvecs x,al_skidy,al_rotx,al_vel (heading = skidy, not roty).
    let saved = al.roty;
    al.roty = al.skidy;
    gen_vecs_3d(al);
    al.roty = saved;
    reached
}

/// `s_goto_WPpostab` body (STRATMAC.INC:2520-2578) specialised to szaco0's
/// fixed params. `stratstate` indexes the waypoint; at `state == len` the path
/// is done (coast). The wp_fire waypoint fires RELSLOWELASER at the player (±3
/// per-axis spread) on the `notdelay 2` gate.
fn szaco0_goto_wppostab(g: &mut Game, idx: u16, table: &[Wp]) {
    let len = table.len() as u8;
    let state = g.objs.aliens[idx as usize].stratstate;
    if state >= len {
        // s_jmp_IFNOTstate len,.cont false -> s_add_vecs2pos ; s_brl .end.
        apply_velocity(&mut g.objs.aliens[idx as usize]);
        return;
    }
    let (wx, wy, wz0, flags) = table[state as usize];
    // wp_fire: s_jmp_notdelay 2 ; weapon_pos 0 ; rnd ±3 spread ; RELSLOWELASER.
    if flags & WP_FIRE != 0 && notdelay(g, 2) {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let base_pitch = strat_pitch_toward(&me, &pl); // svar_byte1 offsets pitch
            let base_yaw = angle_xz(&me, &pl); // svar_byte2 offsets yaw
            let dpitch = (((sf_random(&mut g.vars) as u8) & 7) as i8).wrapping_sub(3);
            let dyaw = (((sf_random(&mut g.vars) as u8) & 7) as i8).wrapping_sub(3);
            strat_fire_relslowlaser(
                g,
                idx,
                base_pitch.wrapping_add(dpitch as u8),
                base_yaw.wrapping_add(dyaw as u8),
            );
        }
    }
    // Waypoint z is player-relative (no wp_fixpos): wpposZ = player_posz.
    let wz = wz0.wrapping_add(g.vars.player_posz);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = wx; // s_set_WP absolute
        al.swpy1 = wy;
        al.swpz1 = wz;
    }
    if szaco0_goto_wp(g, idx, wx, wy, wz) {
        next_state_cap(g, idx, len); // .fin: s_next_state x,#len
    }
}

/// `szaco0_Istrat` (GA2STRAT.ASM:329-337): pick the flight path (rnd&3), arm a
/// 340-frame `sword1` scroll timer, then FALL THROUGH into `szaco0_strat`.
fn szaco0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, szaco0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode); // escapeeexplode2 -> generic (escapee cosmetic)
    let r = (sf_random(&mut g.vars) as u8) & 3; // s_set_alvar2rnd al_sbyte1,#3
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SZACO0_HP;
        al.ap = SZACO0_AP;
        al.sbyte1 = r;
        al.snd2 = 0xf;
        al.sword1 = 340; // s_set_alvar W,al_sword1,#340
    }
    szaco0_strat(g, idx); // fall-through
}

/// `szaco0_strat` (GA2STRAT.ASM:338-356): run the selected WPpostab, then run
/// the `sword1` scroll timer — for its first 340 frames the object scrolls with
/// the world (`s_add_playerZ`); once expired it holds its z.
fn szaco0_strat(g: &mut Game, idx: u16) {
    let table: &[Wp] = match g.objs.aliens[idx as usize].sbyte1 {
        0 => &SZACO0A,
        1 => &SZACO0B,
        2 => &SZACO0C,
        _ => &SZACO0D, // 3
    };
    szaco0_goto_wppostab(g, idx, table);
    // s_beqdec_alvar W,al_sword1,.nrel ; s_add_playerZ.
    if g.objs.aliens[idx as usize].sword1 != 0 {
        g.objs.aliens[idx as usize].sword1 -= 1;
        add_player_z(g, idx);
    }
}

// ============================================================
// szaco5 (IS 156) — GA2STRAT.ASM:478-527. A fighter that snaps to face the
// player and fires, spins up, loops (pitch a full turn), then banks to the
// player. States fall through in-tick (linear next_state, no re-dispatch except
// the state-2 -> state-3 `nextstate` jmptostrat).
// ============================================================

/// `sr_banktoplayer` (STRATROU.ASM) as used by szaco5 state 3 — roll toward the
/// player each frame (`rotz += dx>>6`), then on the `notdelay 2` gate step the
/// yaw/pitch one unit toward the player. Twin of enemy_a `szaco2_bank_to_player`.
fn szaco5_bank(g: &mut Game, idx: u16) {
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

/// `s_add_anim x,#amt,#max` (STRATLIB.INC:180): advance the low-7 anim frame,
/// single-subtract wrap at `max`, keeping the 0x80 "active" flag.
fn add_anim_wrap(al: &mut Alien, amount: u8, maxframes: u8) {
    let mut f = (al.animframe & 0x7F).wrapping_add(amount);
    if f >= maxframes {
        f -= maxframes;
    }
    al.animframe = 0x80 | f;
}

/// `szaco5_Istrat` (GA2STRAT.ASM:478-488): face `deg180`, anim 0, speed 30,
/// then FALL THROUGH into `szaco5_strat`.
fn szaco5_init(g: &mut Game, idx: u16) {
    let tick = sid(g, szaco5_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SZACO5_HP;
        al.ap = SZACO5_AP;
        al.roty = DEG180;
        al.collflags |= COLLTYPE_ENEMY1;
        al.animframe = 0x80; // s_init_anim x,#0
        al.vel = 30; // s_set_speed x,#30
    }
    szaco5_strat(g, idx); // fall-through
}

/// `szaco5_strat` (GA2STRAT.ASM:489-527).
fn szaco5_strat(g: &mut Game, idx: u16) {
    let pl = player(g);

    // state 0: aim 3d at the player (rate 2); within 1500 z fire once + advance.
    if g.objs.aliens[idx as usize].stratstate == 0 {
        if let Some(p) = pl {
            strat_aim_3d(g, idx, &p, 2); // s_obj2obj_3dangle roty,rotx,2
        }
        if !zdist_more(g, idx, 1500) {
            if let Some(p) = pl {
                let me = g.objs.aliens[idx as usize];
                let yaw = angle_xz(&me, &p); // s_weapon_rots2obj y
                let pitch = strat_pitch_toward(&me, &p);
                strat_fire_relslowlaser(g, idx, pitch, yaw);
            }
            next_state(g, idx); // -> 1 (falls through this tick)
        }
    }

    // state 1: arm the loop counter (sbyte1 = 32), spin the anim to 9, and once
    // within 200 z dash (speed 40) with a random ±15 yaw kick, then advance.
    if g.objs.aliens[idx as usize].stratstate == 1 {
        g.objs.aliens[idx as usize].sbyte1 = 32; // deg360/8
        if g.objs.aliens[idx as usize].animframe & 0x7F != 9 {
            add_anim_wrap(&mut g.objs.aliens[idx as usize], 1, 10);
        }
        if !zdist_more(g, idx, 200) {
            g.objs.aliens[idx as usize].vel = 40;
            next_state(g, idx); // -> 2
            let r = (sf_random(&mut g.vars) as u8) & 31;
            let sb2 = r.wrapping_sub(15);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte2 = sb2;
            al.roty = al.roty.wrapping_add(sb2);
        }
    }

    // state 2: pitch a full loop (rotx -= 8 for 32 frames); on completion the
    // ROM `nextstate` label re-dispatches the strat top at state 3.
    if g.objs.aliens[idx as usize].stratstate == 2 {
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            next_state(g, idx); // -> 3
            return szaco5_strat(g, idx); // s_brl jmptostrat
        }
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 -= 1;
        al.rotx = al.rotx.wrapping_sub(8);
    }

    // state 3: bank toward the player.
    if g.objs.aliens[idx as usize].stratstate == 3 {
        szaco5_bank(g, idx);
    }

    // tail: s_gen_3dvecs ; s_add_vecs2pos ; s_add_playerZ.
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// houdai5f (IS 188) — KSTRATS.ASM:588-608. A fog turret: every 32 frames, if
// the player is at least 400 z away, fires a homing Hplasma (speed/life 100).
// ============================================================

/// houdai5f homing Hplasma (KSTRATS.ASM:599-606): fire_Hplasma (homingflat,
/// muzzle rotated by firer + `s_weapon_rot #0,#deg180`) then override the shot
/// to home the player at speed/life 100. Twin of `fire_hplasma` with houdai5f's
/// muzzle/speed/life.
fn houdai5f_fire(g: &mut Game, idx: u16) {
    let Some(player_idx) = player_index(g) else {
        return;
    };
    let Some(shot) = make_obj(g, 0) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    full_offset_pos(g, shot, &me, 0, HOUDAI5F_MUZZLE_Y, 0);
    let s_tick = sid(g, homingflat_strat);
    let (_gen_tick, s_coll) = projectile_strat_ids(g);
    let yaw = me.roty.wrapping_add(DEG180); // s_weapon_rot #0,#deg180
    let pitch = me.rotx;
    let al = &mut g.objs.aliens[shot as usize];
    al.shape = 0;
    al.sflags |= ASF_INVISIBLE;
    al.type_ |= ATLASER | ATZREMOVE;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 1;
    al.ap = HPLASMA_AP;
    al.vel = HOUDAI5F_SHOT_SPEED;
    al.count = HOUDAI5F_SHOT_LIFE;
    al.snd2 = 6;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.fireobjptr = player_idx + 1; // s_set_alvar y,al_ptr,playpt (homing target)
    al.immuneptr = idx;
    al.sbyte1 = yaw;
    al.sbyte2 = pitch;
    al.roty = yaw;
    al.rotx = pitch;
    gen_vecs_3d(al);
    // ROM `jsl enemybattrysound_l` (GSTRATS.ASM:2528).
    g.hooks
        .make_snd(PosSndFamilyId::EnemyBattry, me.worldx, me.worldz);
}

/// `houdai5f_Istrat` (KSTRATS.ASM:588-593): anim 0, then FALL THROUGH into
/// `houdai5f_strat`.
pub fn houdai5f_istrat(g: &mut Game, idx: u16) {
    houdai5f_init(g, idx);
}

fn houdai5f_init(g: &mut Game, idx: u16) {
    let tick = sid(g, houdai5f_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode); // smarkexplode -> generic (smark cosmetic)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HOUDAI5_HP;
        al.ap = HOUDAI5_AP;
        al.animframe = 0x80; // s_init_anim x,#0
    }
    houdai5f_strat(g, idx); // fall-through
}

/// `houdai5f_strat` (KSTRATS.ASM:594-607): spin the anim (0..11), and on the
/// `(gameframe & 31) == 0` gate — only when the player is at least 400 z away —
/// fire a homing Hplasma. Fog is cosmetic.
pub fn houdai5f_strat(g: &mut Game, idx: u16) {
    add_anim_wrap(&mut g.objs.aliens[idx as usize], 1, 12); // s_add_anim x,#1,#12
                                                            // s_jmp_notANDframe #31 ; s_jmp_Zdistless #400 -> fire when far, gated /32.
    if g.vars.gameframe & 31 == 0 && !zdist_less(g, idx, 400) {
        houdai5f_fire(g, idx);
    }
}

const HOUDAI5_MUZZLE_Y: i16 = -100;
const HOUDAI5_MUZZLE_Z: i16 = -10;
const HOUDAI5_SHOT_SPEED: u8 = 100;
const HOUDAI5_SHOT_LIFE: u8 = 100;

/// `houdai5_Istrat` (KSTRATS.ASM:613-631). This non-fog turret shares the
/// animated body and durability of `houdai5f`, but fires ordinary plasma at
/// the player when the target is outside its near-Z dead zone and within the
/// turret's horizontal firing lane.
pub fn houdai5_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, houdai5_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = HOUDAI5_HP;
    al.ap = HOUDAI5_AP;
    al.animframe = 0x80;
    houdai5_strat(g, idx);
}

pub fn houdai5_strat(g: &mut Game, idx: u16) {
    add_anim_wrap(&mut g.objs.aliens[idx as usize], 1, 12);
    if g.vars.gameframe & 31 != 0 || zdist_less(g, idx, 400) || xdist_more(g, idx, 400) {
        return;
    }
    let Some(player_idx) = player_index(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let target = g.objs.aliens[player_idx as usize];
    let saved_pitch = me.rotx;
    let saved_yaw = me.roty;
    {
        let firer = &mut g.objs.aliens[idx as usize];
        firer.rotx = strat_pitch_toward(&me, &target);
        firer.roty = angle_xz(&me, &target);
    }
    let shot = crate::enemy_a::fire_plasma(g, idx);
    {
        let firer = &mut g.objs.aliens[idx as usize];
        firer.rotx = saved_pitch;
        firer.roty = saved_yaw;
    }
    if let Some(shot) = shot {
        full_offset_pos(g, shot, &me, 0, HOUDAI5_MUZZLE_Y, HOUDAI5_MUZZLE_Z);
        let projectile = &mut g.objs.aliens[shot as usize];
        projectile.vel = HOUDAI5_SHOT_SPEED;
        projectile.count = HOUDAI5_SHOT_LIFE;
        gen_vecs_3d(projectile);
    }
}

// ============================================================
// State helper.
// ============================================================

/// `nextstate` (STRATROU.ASM:2977-2979): `al_stratstate += 1` then re-enter
/// the strat top the same tick. Callers do the re-enter explicitly.
fn next_state(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
}

// ============================================================
// Door / wall / tree / woods scenery family.
// ASM oracle (no C-oracle for any of these): GASTRATS.ASM (woods:1381-1435),
// D2STRATS.ASM (kdoor/kdoor2:686-721), DSTRATS.ASM (walls:968-1053,
// trees:1970-2063). Constants: STRATEQU.INC:66-68/148-149/210-211/980,
// DSTRATS.ASM:98-99. Macro semantics per docs/AUDIT_BOSS_TICKS2_FINDINGS.
//
// Exact zero-based ISTRAT rows (the macro definition itself is not a row), also
// used by sf-map as the runtime `world.istrats[]` dispatch key:
//   woods=53, wallleftright=74, walll=75, wallr=76, kdoor=139,
//   kichi2=140, kdoor2=141, tree1=203, tree2=204.
// sf-map's `istrat_reference` integration test checks these mechanically
// against the authoritative ISTRATS.ASM ordering.
//
// STATE MACHINES (per-fn cites):
//   woods  : Zenemy obstacle; inside 2100 z converts ITSELF into a woodsgo
//            homing missile (reuses the already-ported `woodsgo_strat`).
//   kdoor  : proximity airlock — open anim 0->8 inside 600 z, close 8->0 out.
//   kdoor2 : kdoor + sflag1: once fully open, restores player control and
//            removes the kichi_0 (massivebase) object it gated.
//   walll/ : swinging walls — swing roty toward -64 (left) / +64 (right) when
//   wallr    the player is within wall1DIST(600) xz; toggle lean on being hit.
//   wallleftright : oscillating wall — flips its lean every 16 frames.
//   tree1/ : indestructible (tree1HP=hardHP=-1) ENEMY1 sprouting-tree scenery.
//   tree2    Only the base trunk GROW is modelled (grow anim 0->8, then hold);
//            the sprouty segment-chain (.strat2/.strat3), leaf/flower bloom
//            (.bloom/.flower/createleaf/leaf_istrat) and fall-on-death are
//            cosmetic spawn-in visuals SCOPED OUT — exactly as bosses.rs scoped
//            the tree branch out of its snake-only sprouty port (bosses.rs:6920).
// ============================================================

const IS_WOODS: usize = 53;
const IS_KDOOR: usize = 139;
const IS_KICHI2: usize = 140;
const IS_KDOOR2: usize = 141;
const IS_WALLLEFTRIGHT: usize = 74;
const IS_WALLL: usize = 75;
const IS_WALLR: usize = 76;
const IS_TREE1: usize = 203;
const IS_TREE2: usize = 204;

const WOODS_HP: u8 = 2; // STRATEQU.INC:148 woodsHP
const WOODS_AP: u8 = 8; // STRATEQU.INC:149 woodsAP
const WALL1_AP: u8 = 16; // STRATEQU.INC:210 wall1AP
const WALL1_DIST: i16 = 600; // STRATEQU.INC:211 wall1DIST
const TREE1_AP: u8 = 8; // DSTRATS.ASM:99 tree1AP (tree1HP = hardHP = -1)
const SPROUT_MAXY: i16 = 80; // STRATEQU.INC:980 sprout_maxy

/// `s_add_anim x,#amt,#max,label` (STRATLIB.INC:178, 4-arg jmp form): advance
/// the low-7 anim frame; when it reaches `max`, CLAMP to `max-1` (keeping the
/// 0x80 flag) and return true (the ROM branches to `label`). Otherwise store the
/// new frame and return false. (Distinct from the 3-arg wrap form add_anim_wrap.)
fn add_anim_cap(al: &mut Alien, amount: u8, maxframes: u8) -> bool {
    let f = (al.animframe & 0x7F).wrapping_add(amount);
    if f >= maxframes {
        al.animframe = 0x80 | (maxframes - 1);
        true
    } else {
        al.animframe = 0x80 | f;
        false
    }
}

/// `s_init_colanim x,#v` (STRATLIB.INC:83): al_colframe = v | 0x80.
fn init_colanim(al: &mut Alien, v: u8) {
    al.colframe = v | 0x80;
}

/// `s_add_colanim x,#amt,#max` (STRATLIB.INC:100, 3-arg wrap form): advance the
/// low-7 collision-frame, single-subtract wrap at `max`, keep the 0x80 flag.
fn add_colanim_wrap(al: &mut Alien, amount: u8, maxframes: u8) {
    let mut f = (al.colframe & 0x7F).wrapping_add(amount);
    if f >= maxframes {
        f -= maxframes;
    }
    al.colframe = 0x80 | f;
}

/// `find_Y_l` (STRATROU.ASM): first ACTIVE alien whose shape == `shape`, or
/// None (ROM returns dummyobj -> the `cpy dummyobj / beq` no-op).
fn find_by_shape(g: &Game, shape: u16) -> Option<u16> {
    (0..NUMBER_AL)
        .find(|&i| g.objs.aliens[i].active && g.objs.aliens[i].shape == shape)
        .map(|i| i as u16)
}

// ------------------------------------------------------------
// woods (IS 54) — GASTRATS.ASM:1381-1435.
// ------------------------------------------------------------

/// `woods_Istrat` (GASTRATS.ASM:1381-1385): Zenemy obstacle, woodsHP/AP, hit=
/// hitflash, death=woodsexp. No `s_end_strat` before `woods_strat` -> falls into
/// the tick this frame.
pub fn woods_istrat(g: &mut Game, idx: u16) {
    woods_init(g, idx);
}

fn woods_init(g: &mut Game, idx: u16) {
    let tick = sid(g, woods_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, woodsexp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,woods_strat,hitflash,woodsexp
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WOODS_HP; // s_set_aldata #woodsHP,#woodsAP
        al.ap = WOODS_AP;
        al.collflags |= COLLTYPE_ZENEMY; // s_set_colltype x,Zenemy
    }
    woods_strat(g, idx); // fall-through
}

/// `woods_strat` (GASTRATS.ASM:1386-1390): wait inert until the player closes to
/// within 2100 z, then hand off to `woodsgo_init`.
pub fn woods_strat(g: &mut Game, idx: u16) {
    // s_jmp_Zdistless x,y,#2100,woodsgo_init
    if zdist_less(g, idx, 2100) {
        woods_woodsgo_init(g, idx);
    }
    // else s_end_strat (keep waiting).
}

/// `woodsgo_init` (GASTRATS.ASM:1392-1398): convert THIS object into a homing
/// missile — swap its tick to the ported `woodsgo_strat`, arm the 10-frame home
/// timer, sound2=2. `makeMEDexpobj_srou` (launch-flash mesh) is cosmetic. Ends
/// the tick (does NOT fall into woodsgo_strat this frame).
pub fn woodsgo_init(g: &mut Game, idx: u16) {
    woods_woodsgo_init(g, idx);
}

fn woods_woodsgo_init(g: &mut Game, idx: u16) {
    let wtick = sid(g, woodsgo_strat);
    let exp = sid(g, strat_explode); // s_set_expstrat x,stopexplode_Istrat -> generic.
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(wtick); // s_set_strat x,woodsgo_strat
    al.expstratptr = Some(exp);
    al.sbyte1 = 10; // s_set_alvar al_sbyte1,#10
    al.snd2 = 2; // set_sound2 x,#$2
}

/// `woodsexp_Istrat` (GASTRATS.ASM:1431-1435): remove the al_ptr child (woods
/// has none, so this is a no-op in practice), then explode (smarkexplode ->
/// generic).
pub fn woodsexp_istrat(g: &mut Game, idx: u16) {
    woodsexp_strat(g, idx);
}

fn woodsexp_strat(g: &mut Game, idx: u16) {
    let child_ref = g.objs.aliens[idx as usize].ptr;
    if child_ref != 0 {
        let child = child_ref - 1;
        if (child as usize) < NUMBER_AL && g.objs.aliens[child as usize].active {
            g.objs.free(child); // s_remove_obj y
        }
    }
    strat_explode(g, idx); // s_jmp smarkexplode_Istrat
}

/// ROM `missgo_Istrat` (GASTRATS.ASM:1375) — empty stub (start+end only).
pub fn missgo_istrat(_g: &mut Game, _idx: u16) {}

// ============================================================
// ripman (GASTRATS.ASM:2997-3023) — falling repair-pod carrier.
// ============================================================

const RIPMAN_HP: u8 = 4; // STRATEQU.INC:175
const RIPMAN_AP: u8 = 16; // STRATEQU.INC:176
const SH_RIPAIR_W: u16 = 401;

/// ROM `ripman_Istrat` — shadow + enemyweap, falls until y>=-30.
pub fn ripman_istrat(g: &mut Game, idx: u16) {
    let s_tick = sid(g, ripman_strat);
    let s_hit = sid(g, strat_hit_flash);
    let s_exp = sid(g, ripmanexp_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_hit);
    al.expstratptr = Some(s_exp);
    al.hp = RIPMAN_HP;
    al.ap = RIPMAN_AP;
    al.sflags |= ASF_SHADOW;
    al.collflags |= COLLTYPE_ENEMYWEAP;
}

/// ROM `ripman_strat` — tumble + drift while airborne (`worldy < -30`).
pub fn ripman_strat(g: &mut Game, idx: u16) {
    // s_jmp_lower x,#-30,.gnd — skip motion when worldy >= -30.
    if worldy_ge(g, idx, -30) {
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(16);
    al.worldy = al.worldy.wrapping_add(3);
    al.worldx = al.worldx.wrapping_add(4);
    al.worldz = al.worldz.wrapping_add(35);
}

/// ROM `ripmanexp_Istrat` — spawn ripair repair ship, then explode.
pub fn ripmanexp_istrat(g: &mut Game, idx: u16) {
    g.hooks.play_se(0x0a);
    if let Some(pod) = make_obj(g, SH_RIPAIR_W) {
        crate::enemy_a::ripair_istrat(g, pod);
    }
    strat_explode(g, idx);
}

// ------------------------------------------------------------
// kdoor (IS 140) / kdoor2 (IS 141) — D2STRATS.ASM:686-721.
// ------------------------------------------------------------

/// `kdoor2_istrat` (D2STRATS.ASM:686-689): set sflag1, then FALL THROUGH into
/// kdoor_istrat (no s_end_strat before the label).
fn kdoor2_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    kdoor_init(g, idx);
}

/// `kdoor_istrat` (D2STRATS.ASM:690-696): tick=.strat, hit=hitflash, death=
/// explode, hardHP/hardAP, colldisable, anim 0. Falls into the tick this frame.
fn kdoor_init(g: &mut Game, idx: u16) {
    let tick = sid(g, kdoor_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,.strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARDHP; // s_set_aldata #hardHP,#hardAP
        al.ap = HARD_AP;
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
        al.animframe = 0x80; // s_init_anim x,#0
    }
    kdoor_strat(g, idx); // fall-through
}

/// `.strat` (D2STRATS.ASM:696-721): open the door anim (0->8, clamp) while the
/// player is within 600 z; else close it (8->0). When fully open, `.remove`
/// runs (kdoor2 only): restore control + drop the kichi_0.
fn kdoor_strat(g: &mut Game, idx: u16) {
    // The door opens below a depth distance of 600.
    if zdist_less(g, idx, 600) {
        // .open: s_dooropen_snd 0 ; s_add_anim x,#1,#8,.remove
        if g.objs.aliens[idx as usize].animframe & 0x7F == 0 {
            door_family_sound(g, idx, PosSndFamilyId::DoorOpen);
        }
        if add_anim_cap(&mut g.objs.aliens[idx as usize], 1, 8) {
            kdoor_remove(g, idx);
        }
    } else {
        // close: s_doorclose_snd 7 ; s_cmp_anim #0 beq .end ;
        // s_add_anim x,#-1,#8 (3-arg wrap; the #0 guard keeps it from wrapping).
        if g.objs.aliens[idx as usize].animframe & 0x7F == 7 {
            door_family_sound(g, idx, PosSndFamilyId::DoorClose);
        }
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe & 0x7F != 0 {
            add_anim_wrap(al, 0xFF, 8); // -1
        }
    }
}

/// `.remove` (D2STRATS.ASM:711-721): only the sflag1 variant (kdoor2) restores
/// player control and removes the gated kichi_0 (massivebase) object.
fn kdoor_remove(g: &mut Game, idx: u16) {
    // s_jmp_notalsflag x,sflag1,.noflagclr
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        return;
    }
    g.vars.pshipflags &= !(PSF_NOCTRL | PSF_NOFIRE); // s_playerctrl on
                                                     // Remove the matching object unless lookup returned the sentinel.
    if let Some(k) = find_by_shape(g, KICHI_0) {
        g.objs.free(k);
    }
}

// ------------------------------------------------------------
// walll / wallr (IS 76 / 77) + wallleftright (IS 75) — DSTRATS.ASM:968-1053.
// wall_l / wall_r cosmetic mesh ids (used by the swing) are NOT def_shape'd in
// ISTRATS.ASM and unresolvable here, so the shape swap is omitted (the swing —
// the gameplay behaviour — is faithful). movewallsound / trigse are cosmetic.
// ------------------------------------------------------------

/// `wallleftright_istrat` (DSTRATS.ASM:968-975): anim 0, tick=wall2_strat, hp=-1
/// (indestructible), wall1AP, faces deg180, colanim 0. Falls into the tick.
pub fn wallleftright_istrat(g: &mut Game, idx: u16) {
    wallleftright_init(g, idx);
}

fn wallleftright_init(g: &mut Game, idx: u16) {
    let tick = sid(g, wall2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0x80; // s_init_anim x,#0
        al.stratptr = Some(tick); // s_set_alptrs x,wall2_strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARDHP; // s_set_aldata #-1,#wall1AP
        al.ap = WALL1_AP;
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        init_colanim(al, 0); // s_init_colanim x,#0
    }
    wall2_strat(g, idx); // fall-through
}

/// `walll_istrat` (DSTRATS.ASM:987-990): anim 1 (leans left), then `wallin`.
pub fn walll_istrat(g: &mut Game, idx: u16) {
    walll_init(g, idx);
}

fn walll_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].animframe = 0x81; // s_init_anim x,#1
    wall_in(g, idx);
}

/// `wallr_istrat` (DSTRATS.ASM:991-993): anim 0 (leans right), then `wallin`.
pub fn wallr_istrat(g: &mut Game, idx: u16) {
    wallr_init(g, idx);
}

fn wallr_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].animframe = 0x80; // s_init_anim x,#0
    wall_in(g, idx);
}

/// `wallin` (DSTRATS.ASM:994-998): tick=wall1_strat, hardHP, wall1AP, faces
/// deg180, colanim 4, nohitaffect. Falls into the tick this frame.
fn wall_in(g: &mut Game, idx: u16) {
    let tick = sid(g, wall1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,wall1_strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARDHP; // s_set_aldata #hardHP,#wall1AP
        al.ap = WALL1_AP;
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        init_colanim(al, 4); // s_init_colanim x,#4
        al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag x,nohitaffect
    }
    wall1_strat(g, idx); // fall-through
}

/// `wall2_strat` (DSTRATS.ASM:976-982): the wallleftright oscillator — advance
/// colanim, and every 16 frames (`notdelay 4`) run `wallchk` (flip the lean bit),
/// then fall into `wallnothit`. On being hit (HF1) skip straight to wallnothit.
pub fn wall2_strat(g: &mut Game, idx: u16) {
    // s_test_hitflags x,#HF1 ; s_bne wallnothit
    if g.objs.aliens[idx as usize].hitflags & HF1 != 0 {
        wall_nothit(g, idx);
        return;
    }
    add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, 4); // s_add_colanim x,#1,#4
                                                              // s_jmp_notdelay 4,wallnothit -> reach wallchk only on the /16 tick.
    if notdelay(g, 4) {
        wall_chk(&mut g.objs.aliens[idx as usize]); // s_jmp wallchk
    }
    wall_nothit(g, idx);
}

/// `wall1_strat` (DSTRATS.ASM:1000-1025): the walll/wallr wall — decay the hit
/// debounce (sbyte4), keep colanim >= 4, and on a fresh HF1 hit (debounce clear)
/// flip the lean bit + arm a 10-frame debounce. Always falls into wallnothit.
pub fn wall1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_beqdec al_sbyte4,.noinc (.noinc is the next line -> just decrements).
        if al.sbyte4 != 0 {
            al.sbyte4 -= 1;
        }
        add_colanim_wrap(al, 1, 8); // s_add_colanim x,#1,#8
                                    // s_cmp_colanim #4 ; bcs .ok ; s_init_colanim #4 (clamp colanim >= 4).
        if al.colframe & 0x7F < 4 {
            init_colanim(al, 4);
        }
    }
    // s_test_hitflags x,#HF1 ; s_beq wallnothit
    if g.objs.aliens[idx as usize].hitflags & HF1 == 0 {
        wall_nothit(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].hitflags &= !HF1; // s_clr_hitflags x,#HF1
                                                  // s_jmp_alvarNOTZERO al_sbyte4,wallnothit (debounce still active -> skip flip).
    if g.objs.aliens[idx as usize].sbyte4 != 0 {
        wall_nothit(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte4 = 10; // s_set_alvar al_sbyte4,#10
    wall_chk(&mut g.objs.aliens[idx as usize]); // wallchk
    wall_nothit(g, idx);
}

/// `wallchk` (DSTRATS.ASM:1014-1019): toggle animframe bit 0 (lean flip); trigse
/// $57 is cosmetic.
fn wall_chk(al: &mut Alien) {
    al.animframe ^= 1;
}

/// `wallnothit` (DSTRATS.ASM:1021-1025 + walllr_i/wallleft/wallright:1027-1053):
/// once the player is within wall1DIST(600) xz, latch the swing tick — animframe
/// bit0 == 0 swings right (roty -> +64), else left (roty -> -64).
pub fn wallnothit(g: &mut Game, idx: u16) {
    wall_nothit(g, idx);
}

fn wall_nothit(g: &mut Game, idx: u16) {
    // s_jmp_distless x,y,#wall1DIST,walllr_i
    if !xzdist_less(g, idx, WALL1_DIST) {
        return; // s_end_strat
    }
    // walllr_i: s_cmp_anim #0 ; s_beq wallright_i (else wallleft_i).
    // ROM wallleft_i / wallright_i: jsl movewallsound_l once on latch.
    let (ox, oz) = {
        let al = &g.objs.aliens[idx as usize];
        (al.worldx, al.worldz)
    };
    if g.objs.aliens[idx as usize].animframe & 0x7F == 0 {
        let t = sid(g, wallright_strat);
        g.objs.aliens[idx as usize].stratptr = Some(t); // s_set_strat x,wallright_strat
        g.hooks.make_snd(PosSndFamilyId::MoveWall, ox, oz);
        wallright_strat(g, idx); // falls into it this frame
    } else {
        let t = sid(g, wallleft_strat);
        g.objs.aliens[idx as usize].stratptr = Some(t); // s_set_strat x,wallleft_strat
        g.hooks.make_snd(PosSndFamilyId::MoveWall, ox, oz);
        wallleft_strat(g, idx);
    }
}

/// `wallleft_strat` (DSTRATS.ASM:1034-1040): swing roty toward -64 (=192) by +16.
pub fn wallleft_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    if al.roty != 192 {
        al.roty = al.roty.wrapping_add(16); // s_add_alvar B,x,al_roty,#16
    }
}

/// `wallright_strat` (DSTRATS.ASM:1047-1053): swing roty toward +64 by -16.
pub fn wallright_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    if al.roty != 64 {
        al.roty = al.roty.wrapping_sub(16); // s_add_alvar B,x,al_roty,#-16
    }
}

/// ROM `wallrnd_istrat` (DSTRATS.ASM:984-986) — random lean left or right.
pub fn wallrnd_istrat(g: &mut Game, idx: u16) {
    if sf_random(&mut g.vars) & 1 != 0 {
        wallr_init(g, idx);
    } else {
        walll_init(g, idx);
    }
}

// ------------------------------------------------------------
// walkright / walker1 / walker2 / duct (GASTRATS / GA2STRAT / DSTRATS)
// ------------------------------------------------------------

const WALKER1_HP: u8 = 5;
const WALKER2_HP: u8 = 10;
const WALKER2_AP: u8 = 8;
const MTUNNEL_MIN_X: i16 = -90;
const MTUNNEL_MAX_X: i16 = 90;
const WALKER1_MUZZLE_Y: i16 = -50 >> 2; // #-50>>weapon_scale

/// ROM `walkright_Istrat` / `walkright_strat` (GASTRATS.ASM:229-257).
pub fn walkright_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, walkright_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 30; // s_set_lifecnt #30
        al.stratptr = Some(tick);
    }
    walkright_strat(g, idx);
}

pub fn walkright_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(20);
        al.worldz = al.worldz.wrapping_sub(1);
    }
    add_player_z(g, idx);
    {
        use crate::snes_trig::SINTAB;
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(10);
        al.roty = 0u8.wrapping_sub(DEG90);
        // s_set_alvar2alvartab B,B,W,...,sintab,-5 then force worldy negative.
        let mut y = (SINTAB[al.sbyte1 as usize] as i16) / 32; // >>5 toward zero
        if y >= 0 {
            y = y.wrapping_neg();
        }
        al.worldy = y;
        al.sbyte1 = al.sbyte1.wrapping_add(1);
    }
    // s_dec_lifecnt — remove when count hits 0 after dec.
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = c;
    if c == 0 {
        g.objs.aldead = 1;
    }
}

/// ROM `Lwalker1_Istrat` — face -(90+45) then walker1.
pub fn lwalker1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].roty = 0u8.wrapping_sub(DEG90.wrapping_add(DEG45));
    walker1_istrat(g, idx);
}

/// ROM `Rwalker1_Istrat` — face +(90+45) then walker1.
pub fn rwalker1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].roty = DEG90.wrapping_add(DEG45);
    walker1_istrat(g, idx);
}

/// ROM `walker1_Istrat` — coast then fire HMISSILE1 when close.
pub fn walker1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, walker1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 200;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WALKER1_HP;
        al.ap = 0;
        al.vel = 10;
        al.snd2 = 3;
    }
    strat_gen_vecs_nvecs(&mut g.objs.aliens[idx as usize]); // s_gen_vecs → nvecs_l
}

/// ROM `walker1_strat` — addvecs; if xzdist < 3000 fire HMISSILE1 → move_strat.
pub fn walker1_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    if xzdist_less(g, idx, 3000) {
        if let Some(p) = player_index(g) {
            g.objs.aliens[idx as usize].ptr = p;
            walker1_fire_hmissile(g, idx, p);
            let tick = sid(g, move_strat);
            g.objs.aliens[idx as usize].stratptr = Some(tick);
        }
    }
}

fn walker1_fire_hmissile(g: &mut Game, idx: u16, player_idx: u16) {
    let Some(shot) = make_obj(g, 0) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    full_offset_pos(g, shot, &me, 0, WALKER1_MUZZLE_Y, 0);
    let s_tick = sid(g, hmissile1_strat);
    let (_gen, s_coll) = projectile_strat_ids(g);
    {
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
        al.immuneptr = idx;
        al.fireobjptr = player_idx + 1;
    }
    gen_vecs_3d(&mut g.objs.aliens[shot as usize]);
    // ROM `s_fire_weapon x,HMISSILE1` → gen_weapon `jsl missilesound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Missile, me.worldx, me.worldz);
}

/// ROM `move_strat` (GSTRATS.ASM:973-976) — coast; damagesmoke cosmetic omitted.
pub fn move_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `walker2_Istrat` / `walker2_strat` — tunnel X chase.
pub fn walker2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, walker2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = WALKER2_HP;
        al.ap = WALKER2_AP;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sbyte1 = 15 * 2;
    }
}

pub fn walker2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_limit_alvar W,x,al_worldx,(Mtunnel_minx+30),(Mtunnel_maxx-30)
        let lo = MTUNNEL_MIN_X + 30;
        let hi = MTUNNEL_MAX_X - 30;
        if al.worldx < lo {
            al.worldx = lo;
        } else if al.worldx > hi {
            al.worldx = hi;
        }
        let px = g.vars.player_posx;
        al.worldx = chase_proportional(al.worldx, px, 2);
    }
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(30);
}

/// ROM `leftwall_istrat` (DSTRATS.ASM:8288-8291) — face deg180 then nocoll.
pub fn leftwall_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].roty = DEG180;
    strat_nocoll_init(g, idx);
}

/// ROM `duct_istrat` (DSTRATS.ASM:8297-8299) — `jml nocoll_istrat`.
pub fn duct_istrat(g: &mut Game, idx: u16) {
    strat_nocoll_init(g, idx);
}

// ============================================================
// wl (double-laser powerup releaser) — GASTRATS.ASM:2629-2645.
// ============================================================

const WL_HP: u8 = 8; // STRATEQU.INC:173 wlHP
const WL_AP: u8 = 16; // STRATEQU.INC:174 wlAP
const SH_ITEM_7: u16 = 160; // shape_data item_7

/// `wl_Istrat` (GASTRATS.ASM:2629-2633): wire tick/hit/die + wlHP/AP.
pub fn wl_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, wl_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, wldie_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = WL_HP;
    al.ap = WL_AP;
}

/// `wl_strat` (GASTRATS.ASM:2635-2637): inert body — waits to be shot.
pub fn wl_strat(_g: &mut Game, _idx: u16) {}

/// `wldie_Istrat` (GASTRATS.ASM:2639-2645): drop `item_7` at death pos, explode.
pub fn wldie_istrat(g: &mut Game, idx: u16) {
    if let Some(drop) = make_obj(g, SH_ITEM_7) {
        crate::enemy_a::strat_item7_init(g, drop);
        let me = g.objs.aliens[idx as usize];
        let al = &mut g.objs.aliens[drop as usize];
        al.worldx = me.worldx;
        al.worldy = me.worldy;
        al.worldz = me.worldz;
    }
    strat_explode(g, idx);
}

/// `spacetest_Istrat` (GASTRATS.ASM:255-261): face 180° / roll -90°, hardHP/AP1,
/// scroll with player. Debug/test scenery leaf.
pub fn spacetest_istrat(g: &mut Game, idx: u16) {
    use sf_game::vars::HARD_HP;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180;
        al.rotz = 0u8.wrapping_sub(DEG90); // #-deg90
        al.hp = HARD_HP;
        al.ap = 1; // s_set_aldata #hardHP,#1
    }
    add_player_z(g, idx);
}

// ============================================================
// saucer1 (DSTRATS.ASM:637-726) — fly-to-WP → face → spin-fire → peel.
// saucer  (IS 227; DSTRATS.ASM:1587-1648) — splash bounce then circle.
// ============================================================

const SAUCER1_HP: u8 = 12; // STRATEQU.INC:193
const SAUCER1_AP: u8 = 1;
const SAUCER1_FC: u8 = 2;
const FLY_DIST: i16 = 700; // STRATEQU.INC:189 flyDIST
const SAUCER_HP: u8 = 10; // STRATEQU.INC:232
const SAUCER_AP: u8 = 4;

/// `saucer1_istrat` (DSTRATS.ASM:637-647).
pub fn saucer1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, saucer1_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SAUCER1_HP;
        al.ap = SAUCER1_AP;
        al.sbyte2 = SAUCER1_FC;
        al.sword1 = al.worldx; // stash spawn x
        al.ptr = al.worldy as u16; // stash spawn y in al_ptr
        al.sflags |= ASF_SHADOW;
        al.animframe = 0;
    }
}

/// `saucer1_strat` (DSTRATS.ASM:649-665): close anim, then WP chase to player
/// offset (sword1, ptr_y, flyDIST); on arrival → istrat2.
pub fn saucer1_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    if g.objs.aliens[idx as usize].animframe != 0 {
        // ddecanim #7
        let a = g.objs.aliens[idx as usize].animframe;
        g.objs.aliens[idx as usize].animframe = (a + 7 - 1) % 7;
        return;
    }
    let (ox, oy) = {
        let al = &g.objs.aliens[idx as usize];
        (al.sword1, al.ptr as i16)
    };
    // s_set_WP x,y,1,sword1,ptr,#flyDIST — relative to player
    if let Some(pl) = player(g) {
        let (rx, rz) = rotate_16xz(pl.roty, ox, FLY_DIST);
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = pl.worldx.wrapping_add(rx);
        al.swpy1 = pl.worldy.wrapping_add(oy);
        al.swpz1 = pl.worldz.wrapping_add(rz);
    }
    let (wx, wy, wz) = {
        let al = &g.objs.aliens[idx as usize];
        (al.swpx1, al.swpy1, al.swpz1)
    };
    // s_goto_WP #40,2,3,0,#500,#0,.chk1
    if saucer1_goto_wp(g, idx, wx, wy, wz) {
        saucer1_istrat2(g, idx);
    }
}

fn saucer1_goto_wp(g: &mut Game, idx: u16, wx: i16, wy: i16, wz: i16) -> bool {
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
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 2);
        false
    };
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    achase_angle(&mut roty, sf_core::aim_angle::yanglexy_nega(dx, dz), 3);
    achase_angle(&mut rotx, sf_core::aim_angle::xanglexabs(dy, dx, dz), 3);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        gen_vecs_3d(al);
    }
    reached
}

/// `saucer1_istrat2` (DSTRATS.ASM:667-669).
pub fn saucer1_istrat2(g: &mut Game, idx: u16) {
    let tick = sid(g, saucer1_strat2);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags2 &= !ASF2_SMFLAG1; // s_initface_player
    }
    saucer1_strat2(g, idx);
}

/// `saucer1_strat2` (DSTRATS.ASM:671-675): face player → istrat3.
pub fn saucer1_strat2(g: &mut Game, idx: u16) {
    if saucer1_face_player(g, idx, 2, 0) {
        saucer1_istrat3(g, idx);
        return;
    }
    add_player_z(g, idx);
}

fn saucer1_face_player(g: &mut Game, idx: u16, chase: u32, delay_bits: u16) -> bool {
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
            let yaw = angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
    }
    false
}

/// `saucer1_istrat3` (DSTRATS.ASM:677-679).
pub fn saucer1_istrat3(g: &mut Game, idx: u16) {
    let tick = sid(g, saucer1_strat3);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    saucer1_strat3(g, idx);
}

/// `saucer1_strat3` (DSTRATS.ASM:680-700): spin + open anim; fire ELASER at
/// frame 6; after FC shots peel (istrat4) else return to approach.
pub fn saucer1_strat3(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(10);
    if g.objs.aliens[idx as usize].animframe != 6 {
        // dincanim #7
        let a = g.objs.aliens[idx as usize].animframe;
        g.objs.aliens[idx as usize].animframe = (a + 1) % 7;
        return;
    }
    // .miss: fire on notdelay 5
    if g.vars.gameframe & 31 != 0 {
        return;
    }
    let _ = fire_elaser(g, idx);
    // s_decbne_alvar sbyte2,.missset
    let sb2 = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte2 = sb2;
    if sb2 != 0 {
        // .missset → back to approach
        let tick = sid(g, saucer1_strat);
        g.objs.aliens[idx as usize].stratptr = Some(tick);
    } else {
        saucer1_istrat4(g, idx);
    }
}

/// `saucer1_istrat4` (DSTRATS.ASM:702-706).
pub fn saucer1_istrat4(g: &mut Game, idx: u16) {
    let tick = sid(g, saucer1_strat4);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vel = 0;
        al.count = 100; // s_set_lifecnt #100
    }
    saucer1_strat4(g, idx);
}

/// `saucer1_strat4` (DSTRATS.ASM:707-726): pitch up, close anim, then accelerate
/// away until life expires.
pub fn saucer1_strat4(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, (-10i8) as u8, 1);
        g.objs.aliens[idx as usize].rotx = rotx;
    }
    let mut roty = g.objs.aliens[idx as usize].roty;
    let yaw_done = achase_angle(&mut roty, 0, 3);
    g.objs.aliens[idx as usize].roty = roty;
    if !yaw_done {
        if g.objs.aliens[idx as usize].animframe != 0 {
            let a = g.objs.aliens[idx as usize].animframe;
            g.objs.aliens[idx as usize].animframe = (a + 7 - 1) % 7;
        }
        return;
    }
    // .fin
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].vel = g.objs.aliens[idx as usize].vel.wrapping_add(1);
    let c = g.objs.aliens[idx as usize].count;
    if c == 0 {
        g.objs.aldead = 1;
    } else {
        g.objs.aliens[idx as usize].count = c - 1;
        if g.objs.aliens[idx as usize].count == 0 {
            g.objs.aldead = 1;
        }
    }
}

/// `saucer_istrat` (DSTRATS.ASM:1587-1595).
pub fn saucer_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, saucer_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SAUCER_HP;
        al.ap = SAUCER_AP;
        al.vx = 0;
        al.vz = 30;
        al.vy = -20;
        al.sflags |= ASF_SHADOW;
        // makesplash / make_shadow — cosmetic omitted
    }
}

/// `saucer_strat` (DSTRATS.ASM:1597-1620): fall/bounce; at rest → strat2.
pub fn saucer_strat(g: &mut Game, idx: u16) {
    // move_shadow / drotsflat — cosmetic
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let bounced = falldown_yvec(g, idx, 1, 1, 0);
    if g.objs.aliens[idx as usize].worldy != 0 && !bounced {
        return;
    }
    // .saucerbounce
    // makesplash — cosmetic
    if g.objs.aliens[idx as usize].vy != 0 {
        return;
    }
    // .chigaustrat
    let tick = sid(g, saucer_strat2);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.vel = 40;
    al.sbyte1 = 0;
    al.sbyte2 = 16;
    al.vy = 0;
}

/// `saucer_strat2` (DSTRATS.ASM:1621-1648): circle on heading sbyte1.
pub fn saucer_strat2(g: &mut Game, idx: u16) {
    // splash countdown
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 != 0 {
        g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
        // makesplash — cosmetic
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_gen_vecs x,al_sbyte1,al_vel — 2D from heading sbyte1 via nvecs_l
        let heading = al.sbyte1;
        al.roty = heading;
        strat_gen_vecs_nvecs(al);
        apply_velocity(al);
    }
    if g.objs.aliens[idx as usize].flags & AF_LEFT_PL != 0 {
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(8);
    } else {
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(8);
    }
    // splash when heading in certain arcs — cosmetic omitted
}

// ============================================================
// jump0/1 + sokuten + item3 + rightwall + mine1 (GASTRATS / DSTRATS)
// ============================================================

const JUMP0_HP: u8 = 2;
const JUMP0_AP: u8 = 4;
const JUMP1_AP: u8 = 8;
const SOKUTEN_HP: u8 = 16;
const SOKUTEN_AP: u8 = 16;
const PLAYER_B_HP: u8 = 40; // STRATEQU.INC:325 playerB_HP

/// `jump1_Istrat` (GASTRATS.ASM:1633-1640): hard/static scenery facing 180°.
pub fn jump1_istrat(g: &mut Game, idx: u16) {
    use sf_game::vars::HARD_HP;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = None;
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARD_HP;
    al.ap = JUMP1_AP;
    al.roty = DEG180;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
}

/// `jump0_Istrat` (GASTRATS.ASM:1641-1649).
pub fn jump0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, jump0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = JUMP0_HP;
    al.ap = JUMP0_AP;
    al.roty = DEG180;
    al.sflags |= ASF_COLLDISABLE;
    al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP | COLLTYPE_ZENEMY;
}

/// `jump0_strat` — wait until player closes to 1500 z.
pub fn jump0_strat(g: &mut Game, idx: u16) {
    if zdist_less(g, idx, 1500) {
        jump0a_init(g, idx);
    }
}

/// `jump0a_init` / `jump0a_strat` — hop then fire one HMISSILE1 at apex.
pub fn jump0a_init(g: &mut Game, idx: u16) {
    let tick = sid(g, jump0a_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.objs.aliens[idx as usize].vy = -30;
    jump0a_strat(g, idx);
}

pub fn jump0a_strat(g: &mut Game, idx: u16) {
    // s_jmp_lower x,#-90,.njump — clear colldisable only while still higher
    // than -90 (worldy < -90); skip once at/below that height.
    if g.objs.aliens[idx as usize].worldy < -90 {
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
    }
    // Fire once when vy crosses from ascending (neg) to falling (non-neg).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 && g.objs.aliens[idx as usize].vy >= 0
    {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
        if let Some(shot) = fire_hmissile1(g, idx) {
            if let Some(p) = player_index(g) {
                g.objs.aliens[shot as usize].ptr = p + 1;
            }
        }
    }
    let _ = falldown_yvec(g, idx, 4, 2, -35);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// `sokuten_Istrat` / `sokuten_strat` (GASTRATS.ASM:2329-2355).
pub fn sokuten_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, sokuten_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = SOKUTEN_HP;
    al.ap = SOKUTEN_AP;
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.vel = 30;
    al.sbyte1 = 0u8.wrapping_sub(DEG90); // -deg90
}

pub fn sokuten_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let heading = al.sbyte1;
        al.roty = heading;
        strat_gen_vecs_nvecs(al); // s_gen_vecs → nvecs_l
        al.roty = heading; // s_copy_alvar2alvar al_roty,al_sbyte1
    }
    // s_beqdec_alvar sbyte2,.doturn — TEST then DEC (stay in .doturn at 0).
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 != 0 {
        g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
    } else {
        // .doturn: clear coll, then restore+dec while heading ≥ -deg180 (u8 ≥ 0x80).
        g.objs.aliens[idx as usize].collstratptr = None;
        let sb1 = g.objs.aliens[idx as usize].sbyte1;
        if sb1 >= DEG180 {
            // not yet past -deg180
            let coll = sid(g, strat_hit_flash);
            g.objs.aliens[idx as usize].collstratptr = Some(coll);
            g.objs.aliens[idx as usize].sbyte1 = sb1.wrapping_sub(1);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(DEG90);
        al.rotz = al.rotz.wrapping_sub(4);
        apply_velocity(al);
    }
    add_player_z(g, idx);
}

/// `item3_Istrat` / `item3_strat` — +5 body HP pickup.
pub fn item3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, item3_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

pub fn item3_strat(g: &mut Game, idx: u16) {
    if player(g).is_none() || g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotz = al.rotz.wrapping_add(4);
        al.worldz = al.worldz.wrapping_add(20);
    }
    let Some(pl) = player(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    if (me.worldz as i32 - pl.worldz as i32).abs() >= 120 {
        return;
    }
    let xydist =
        (me.worldx as i32 - pl.worldx as i32).abs() + (me.worldy as i32 - pl.worldy as i32).abs();
    if xydist >= 60 {
        return;
    }
    // Heal body pcbox (pcboxobj_B) by +5, clamp to playerB_HP.
    let box_idx = g.vars.strategy.player_collision_objects[0];
    if box_idx >= 0 && (box_idx as usize) < NUMBER_AL {
        let b = box_idx as usize;
        if g.objs.aliens[b].active {
            let hp = g.objs.aliens[b].hp.saturating_add(5);
            g.objs.aliens[b].hp = hp.min(PLAYER_B_HP);
        }
    }
    g.hooks.play_se(0x10);
    crate::enemy_a::flashplayer_istrat(g, idx); // s_set_strat flashplayer; s_jmpto_strat
}

/// `rightwall_istrat` — jml nocoll_istrat (DSTRATS.ASM:8293).
pub fn rightwall_istrat(g: &mut Game, idx: u16) {
    strat_nocoll_init(g, idx);
}

/// `mine1_Istrat` — empty stub (GASTRATS.ASM:2435-2437).
pub fn mine1_istrat(_g: &mut Game, _idx: u16) {}

/// `minumusi_Istrat` — empty stub (GASTRATS.ASM:2439-2441).
pub fn minumusi_istrat(_g: &mut Game, _idx: u16) {}

// ============================================================
// door1 open/close wait (GASTRATS.ASM:1015-1070)
// ============================================================

const DOOR1_AP: u8 = 8; // STRATEQU.INC:166
const DOOR1_CLOSED_FRAME: u8 = 9;
const DOOR1_OPENING_CUE_FRAME: u8 = 1;
const DOOR1_ANIMATION_FRAMES: u8 = 10;

/// Radius search used by door1: first active alien within `max_d` of `idx`
/// (excluding self / player), matching ROM `s_find_radiusobj` loosely.
fn find_radius_obj(g: &Game, idx: u16, max_d: i32) -> Option<u16> {
    let me = &g.objs.aliens[idx as usize];
    let mut best: Option<(i32, u16)> = None;
    for i in 0..NUMBER_AL {
        if i == idx as usize || !g.objs.aliens[i].active {
            continue;
        }
        if (i as i16) == g.vars.internal_playpt {
            continue;
        }
        let o = &g.objs.aliens[i];
        let dx = (o.worldx as i32 - me.worldx as i32).abs();
        let dy = (o.worldy as i32 - me.worldy as i32).abs();
        let dz = (o.worldz as i32 - me.worldz as i32).abs();
        let d = dx + dy + dz;
        if d <= max_d {
            match best {
                Some((bd, _)) if bd <= d => {}
                _ => best = Some((d, i as u16)),
            }
        }
    }
    best.map(|(_, i)| i)
}

/// `door1_Istrat` (GASTRATS.ASM:1015-1021) → falls into openwait.
pub fn door1_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARDHP;
    al.ap = DOOR1_AP;
    al.animframe = 0;
    al.roty = DEG180;
    door1openwait_init(g, idx);
}

pub fn door1openwait_init(g: &mut Game, idx: u16) {
    let tick = sid(g, door1openwait_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags &= !ASF_COLLDISABLE;
}

pub fn door1openwait_strat(g: &mut Game, idx: u16) {
    // ROM `s_doorclose_snd 9` runs before the animation decrement.
    if g.objs.aliens[idx as usize].animframe & 0x7F == DOOR1_CLOSED_FRAME {
        door_family_sound(g, idx, PosSndFamilyId::DoorClose);
    }
    {
        let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
        if anim != 0 {
            g.objs.aliens[idx as usize].animframe = anim.wrapping_sub(1);
        }
    }
    if zdist_less(g, idx, 500) {
        door1closewait_init(g, idx);
        return;
    }
    if let Some(y) = find_radius_obj(g, idx, 500) {
        if g.objs.aliens[y as usize].vel != 0 {
            door1closewait_init(g, idx);
        }
    }
}

pub fn door1closewait_init(g: &mut Game, idx: u16) {
    let tick = sid(g, door1closewait_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags |= ASF_COLLDISABLE;
}

pub fn door1closewait_strat(g: &mut Game, idx: u16) {
    // ROM `s_doorclose_snd 1` runs before the animation increment.
    if g.objs.aliens[idx as usize].animframe & 0x7F == DOOR1_OPENING_CUE_FRAME {
        door_family_sound(g, idx, PosSndFamilyId::DoorClose);
    }
    {
        let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
        if anim != DOOR1_CLOSED_FRAME {
            g.objs.aliens[idx as usize].animframe = (anim + 1) % DOOR1_ANIMATION_FRAMES;
        }
    }
    // Stay closed while player within 500z.
    if zdist_less(g, idx, 500) {
        return;
    }
    // Stay closed while any non-zero-vel object is in radius; else reopen.
    let me = &g.objs.aliens[idx as usize];
    for i in 0..NUMBER_AL {
        if i == idx as usize || !g.objs.aliens[i].active {
            continue;
        }
        if (i as i16) == g.vars.internal_playpt {
            continue;
        }
        let o = &g.objs.aliens[i];
        let d = (o.worldx as i32 - me.worldx as i32).abs()
            + (o.worldy as i32 - me.worldy as i32).abs()
            + (o.worldz as i32 - me.worldz as i32).abs();
        if d <= 500 && o.vel != 0 {
            return;
        }
    }
    door1openwait_init(g, idx);
}

// ============================================================
// leng0 (GA2STRAT.ASM:605-625) — animated barrier that unlocks collide.
// ============================================================

/// `leng0_Istrat` / `leng0_strat` — hard barrier; opens anim when player <1300z.
pub fn leng0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, leng0_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = None;
    al.expstratptr = None;
    al.hp = HARDHP;
    al.ap = HARD_AP;
    al.animframe = 0x80;
    al.sflags |= ASF_COLLDISABLE;
}

pub fn leng0_strat(g: &mut Game, idx: u16) {
    if !zdist_less(g, idx, 1300) {
        // |dz| > 1300 → stay shut
        return;
    }
    // Within 1300: play open SFX once (frame 0), advance anim to 10, then enable coll.
    let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
    if anim == 0 {
        g.hooks.play_se(0x54);
    }
    if anim == 10 {
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
        return;
    }
    if anim < 10 {
        g.objs.aliens[idx as usize].animframe = 0x80 | (anim + 1);
    }
}

// ============================================================
// Tunnel quadrant pieces (DSTRATS.ASM:8267-8287).
// ============================================================

fn tunnel_quadrant_istrat(g: &mut Game, idx: u16, rotz: u8) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180;
        al.rotz = rotz;
    }
    strat_nocoll_init(g, idx);
}

pub fn topright1_istrat(g: &mut Game, idx: u16) {
    tunnel_quadrant_istrat(g, idx, 0u8.wrapping_sub(DEG180));
}

pub fn topleft1_istrat(g: &mut Game, idx: u16) {
    tunnel_quadrant_istrat(g, idx, DEG90);
}

pub fn botright1_istrat(g: &mut Game, idx: u16) {
    tunnel_quadrant_istrat(g, idx, 0u8.wrapping_sub(DEG90));
}

pub fn botleft1_istrat(g: &mut Game, idx: u16) {
    tunnel_quadrant_istrat(g, idx, 0);
}

// ============================================================
// twall0 / warker3 (GA2STRAT.ASM:814-846,878-952).
// ============================================================

/// Vertical oscillating hard wall used inside the medium tunnel.
pub fn twall0_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, twall0_strat);
    let al = &mut g.objs.aliens[idx as usize];
    set_hard_vars(al);
    al.stratptr = Some(tick);
    al.roty = DEG180;
    al.sflags |= ASF_SHADOW;
    al.snd2 = 0x0e;
    al.stratstate = 0;
}

pub fn twall0_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    match al.stratstate {
        0 => {
            al.worldy = al.worldy.wrapping_sub(5);
            if al.worldy == -100 {
                al.stratstate = 1;
            }
        }
        1 => {
            al.worldy = al.worldy.wrapping_add(5);
            if al.worldy == -20 {
                al.stratstate = 0;
            }
        }
        _ => al.stratstate = 0,
    }
}

/// Fire the tunnel walker's RELSLOWELASER from local `(0,-40,80)`: the
/// explicit weapon position plus the laser routine's own Z muzzle offset.
fn warker3_fire(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
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
        RELSLOWELASER_LIFE,
        RELSLOWELASER_AP,
        ACF_COLLTYPE4 | ACF_COLLTYPE1,
    ) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    full_offset_pos(g, shot, &me, 0, -40, 80);
    g.hooks
        .make_snd(PosSndFamilyId::Laser, me.worldx, me.worldz);
}

pub fn warker3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, warker3_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = 8;
    al.ap = 4;
    al.roty = DEG180;
    al.animframe = 0x80;
    al.collflags |= COLLTYPE_ENEMYWEAP;
    al.stratstate = 0;
}

pub fn warker3_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            if zdist_less(g, idx, 700) {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate = 1;
                al.vz = MEDPSPEED as i16 - 15;
                al.vy = -10;
            }
        }
        1 => {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = achase_word(al.worldx, 0, 2);
            al.vy = al.vy.wrapping_add(1);
            // `s_jmp_higher #0`: negative Y is above the tunnel centre.
            if al.worldy >= 0 {
                al.vy = 0;
                al.stratstate = 2;
            }
        }
        2 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags2 |= ASF2_SFLAG1;
                al.vz = MEDPSPEED as i16 + 10;
            }
            if !zdist_less(g, idx, 1000) {
                if let Some(p) = player(g) {
                    let me = g.objs.aliens[idx as usize];
                    warker3_fire(g, idx, strat_pitch_toward(&me, &p), angle_xz(&me, &p));
                }
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate = 3;
                al.sbyte1 = (sf_random(&mut g.vars) as u8) & 3;
            }
        }
        3 => {
            let close = zdist_less(g, idx, 1000);
            let (px, py) = (g.vars.player_posx, g.vars.player_posy);
            let minx = g.vars.sv_i16(sv::MINPMOVEX);
            let maxx = g.vars.sv_i16(sv::MAXPMOVEX);
            let al = &mut g.objs.aliens[idx as usize];
            al.vy = 0;
            if !close {
                al.vz = 0;
            }
            match al.sbyte1 {
                0 | 1 => {
                    achase_angle(&mut al.rotz, DEG180, 1);
                    al.worldy = achase_word(al.worldy, -120, 2);
                    al.worldx = achase_word(al.worldx, px, 2);
                }
                2 => {
                    achase_angle(&mut al.rotz, DEG90, 1);
                    al.worldy = achase_word(al.worldy, py, 2);
                    al.worldx = achase_word(al.worldx, minx, 2);
                }
                _ => {
                    achase_angle(&mut al.rotz, 0u8.wrapping_sub(DEG90), 1);
                    al.worldy = achase_word(al.worldy, py, 2);
                    al.worldx = achase_word(al.worldx, maxx, 2);
                }
            }
        }
        _ => g.objs.aliens[idx as usize].stratstate = 0,
    }

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        let anim = g.objs.aliens[idx as usize].animframe & 0x7f;
        g.objs.aliens[idx as usize].animframe = 0x80 | ((anim + 1) % 11);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

// ============================================================
// meteor_istrat2 + meteorcol (DSTRATS.ASM:1210 / 1250)
// ============================================================

/// Fragment meteor spawned by meteorexp — random spin/vel, then meteor_istrat3 body.
pub fn meteor_istrat2(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = (sf_random(&mut g.vars) as u8) & 7;
    g.objs.aliens[idx as usize].roty = sf_random(&mut g.vars) as u8;
    g.objs.aliens[idx as usize].vel = (sf_random(&mut g.vars) as u8) & 15;
    g.objs.aliens[idx as usize].sword1 = 60;
    meteor_istrat3(g, idx);
}

fn meteor_istrat3(g: &mut Game, idx: u16) {
    let tick = sid(g, meteor_strat);
    let coll = sid(g, meteorcol_istrat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        if al.hp == 0 {
            al.hp = METEOR_HP;
        }
        if al.ap == 0 {
            al.ap = METEOR_AP;
        }
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].collflags |= COLLTYPE_ENEMY1;
    g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
    meteor_strat(g, idx);
}

/// `meteorcol_istrat` — one framesperAP damage tick, then resume strat.
pub fn meteorcol_istrat(g: &mut Game, idx: u16) {
    // s_docoll x,#framesperAP — approx: subtract AP from hp when hittable.
    if g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT == 0 {
        let ap = g.objs.aliens[idx as usize].ap.max(1);
        let hp = g.objs.aliens[idx as usize].hp;
        if hp != HARDHP && hp > ap {
            g.objs.aliens[idx as usize].hp = hp - ap;
        } else if hp != HARDHP {
            g.objs.aliens[idx as usize].hp = 0;
        }
    }
    g.objs.aliens[idx as usize].sflags |= ASF_HITFLASH;
}

// ============================================================
// fly family (DSTRATS.ASM:419-595) + highfly/distantfly + flydead.
// ============================================================

const FLY_HP: u8 = 2; // STRATEQU.INC:187
const FLY_AP: u8 = 4;
const FLY_FC: u8 = 4;

/// `fly_istrat` (DSTRATS.ASM:419-432).
pub fn fly_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fly_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, flydead_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.collflags |= COLLTYPE_ENEMY1;
        al.hp = FLY_HP;
        al.ap = FLY_AP;
        al.sbyte1 = (-1i8) as u8;
        if al.flags & AF_LEFT_PL == 0 {
            // not left → right side: sbyte1 = +1
            al.sbyte1 = 1;
        }
        al.sword1 = -30;
        al.sbyte2 = FLY_FC;
        al.sflags |= ASF_SHADOW;
    }
}

/// `fly_strat` (DSTRATS.ASM:436-450): roll/pitch dive until sword1≥0 → flylr.
pub fn fly_strat(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 1 == 0 {
        // s_jmp_notdelay 1,.noadd — only on delay gate
        let sb1 = g.objs.aliens[idx as usize].sbyte1;
        g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(sb1);
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_sub(1);
        let sw = g.objs.aliens[idx as usize].sword1.wrapping_add(2);
        g.objs.aliens[idx as usize].sword1 = sw;
        if (sw as i16) >= 0 {
            flylr_istrat(g, idx);
            return;
        }
    }
    // .noadd
    let sw = g.objs.aliens[idx as usize].sword1;
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(sw);
    add_player_z(g, idx);
}

/// `flylr_istrat` (DSTRATS.ASM:453-461).
pub fn flylr_istrat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 10;
        al.sword1 = al.worldx;
        al.ptr = al.worldy as u16;
        al.sbyte1 = (-32i8) as u8;
    }
    // s_rightview_strat → not leftpl
    if g.objs.aliens[idx as usize].flags & AF_LEFT_PL == 0 {
        flyr_istrat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = 32;
    flyr_istrat(g, idx);
}

/// `flyr_istrat` / `flyr_strat` (DSTRATS.ASM:463-477).
pub fn flyr_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, flyr_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.objs.aliens[idx as usize].sflags |= ASF_SHADOW;
    flyr_strat(g, idx);
}

pub fn flyr_strat(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
    let target = g.objs.aliens[idx as usize].sbyte1;
    let mut roty = g.objs.aliens[idx as usize].roty;
    let _ = achase_angle(&mut roty, target, 1);
    g.objs.aliens[idx as usize].roty = roty;
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 40, 3);
    if let Some(pl) = player(g) {
        if dist_xz(&g.objs.aliens[idx as usize], &pl) > 1000 {
            fly2_istrat(g, idx);
        }
    }
}

/// `highfly_istrat` (DSTRATS.ASM:481-500).
pub fn highfly_istrat(g: &mut Game, idx: u16) {
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, flydead_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.stratptr = None;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = FLY_HP;
        al.ap = FLY_AP;
        al.sbyte2 = FLY_FC;
        // rnd x in [-256,255]
        let lo = sf_random(&mut g.vars) as u8 as u16;
        let hi = ((sf_random(&mut g.vars) as u8) & 1) as u16;
        let mut sx = ((hi << 8) | lo) as i16;
        sx = sx.wrapping_sub(256);
        al.sword1 = sx;
        // rnd y: (rnd&63) - 80 → [-80,-17]
        let py = ((sf_random(&mut g.vars) as u8) & 63) as i16;
        al.ptr = (py - 80) as u16;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    fly2_istrat(g, idx);
}

/// `distantfly_istrat` (DSTRATS.ASM:503-510).
pub fn distantfly_istrat(g: &mut Game, idx: u16) {
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, flydead_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.stratptr = None;
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = FLY_HP;
        al.ap = FLY_AP;
        al.sbyte2 = FLY_FC;
        al.sword1 = al.worldx;
        al.ptr = al.worldy as u16;
    }
    fly2_istrat(g, idx);
}

/// `fly2_istrat` / `fly2_strat` (DSTRATS.ASM:512-526).
pub fn fly2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fly2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    fly2_strat(g, idx);
}

pub fn fly2_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let (ox, oy) = {
        let al = &g.objs.aliens[idx as usize];
        (al.sword1, al.ptr as i16)
    };
    if let Some(pl) = player(g) {
        let (rx, rz) = rotate_16xz(pl.roty, ox, FLY_DIST);
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = pl.worldx.wrapping_add(rx);
        al.swpy1 = pl.worldy.wrapping_add(oy);
        al.swpz1 = pl.worldz.wrapping_add(rz);
    }
    let (wx, wy, wz) = {
        let al = &g.objs.aliens[idx as usize];
        (al.swpx1, al.swpy1, al.swpz1)
    };
    // goto_WP #40,5,3,0,#500,#0,.chk1
    if fly_goto_wp(g, idx, wx, wy, wz, 40, 5) {
        fly3_istrat(g, idx);
    }
}

fn fly_goto_wp(
    g: &mut Game,
    idx: u16,
    wx: i16,
    wy: i16,
    wz: i16,
    max_speed: u8,
    accel: u8,
) -> bool {
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
        let _ = speed_to(&mut g.objs.aliens[idx as usize], max_speed, accel);
        false
    };
    let me = g.objs.aliens[idx as usize];
    let mut roty = me.roty;
    let mut rotx = me.rotx;
    achase_angle(&mut roty, sf_core::aim_angle::yanglexy_nega(dx, dz), 3);
    achase_angle(&mut rotx, sf_core::aim_angle::xanglexabs(dy, dx, dz), 3);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = roty;
        al.rotx = rotx;
        gen_vecs_3d(al);
    }
    reached
}

/// `fly3_istrat` / `fly3_strat` (DSTRATS.ASM:530-553).
pub fn fly3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fly3_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.stratptr = Some(tick);
        al.sflags2 &= !ASF2_SMFLAG1;
    }
}

pub fn fly3_strat(g: &mut Game, idx: u16) {
    if fly_face_player(g, idx, 1, 0) {
        // .fireit
        if g.vars.gameframe & 31 != 0 {
            add_player_z(g, idx);
            return;
        }
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.vx = 0;
            al.vy = 0;
            al.vz = 0;
        }
        let _ = fire_elaser(g, idx);
        let sb2 = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte2 = sb2;
        if sb2 != 0 {
            fly2_istrat(g, idx);
        } else {
            fly4_istrat(g, idx);
        }
        return;
    }
    add_player_z(g, idx);
}

fn fly_face_player(g: &mut Game, idx: u16, chase: u32, delay_bits: u16) -> bool {
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
            let yaw = angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = yaw;
            al.sbyte4 = pitch;
        }
    }
    false
}

/// `fly4_istrat` / `fly4_strat` (DSTRATS.ASM:556-570).
pub fn fly4_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, fly4_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.stratptr = Some(tick);
        al.vel = 5;
        al.count = 100;
    }
}

pub fn fly4_strat(g: &mut Game, idx: u16) {
    let c = g.objs.aliens[idx as usize].count;
    if c == 0 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].count = c - 1;
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.aldead = 1;
        return;
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, (-10i8) as u8, 4);
        g.objs.aliens[idx as usize].rotx = rotx;
        let mut roty = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut roty, 0, 4);
        g.objs.aliens[idx as usize].roty = roty;
    }
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 30, 5);
    add_player_z(g, idx);
}

/// `flydead_istrat` / `flydead_strat` (DSTRATS.ASM:572-582).
pub fn flydead_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, flydead_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(tick);
    flydead_strat(g, idx);
}

pub fn flydead_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotx = 64;
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 30, 2);
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_jmp_lower x,#0 → when worldy >= 0
    if g.objs.aliens[idx as usize].worldy >= 0 {
        flyhitgnd_istrat(g, idx);
    }
}

/// `flyhitgnd_istrat` / `flyhitgnd_strat` (DSTRATS.ASM:584-595).
pub fn flyhitgnd_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, flyhitgnd_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_SHADOW;
        al.expstratptr = Some(tick);
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.worldy = 0;
        al.animframe = 0;
        al.rotx = 0;
    }
}

pub fn flyhitgnd_strat(_g: &mut Game, _idx: u16) {
    // s_damagefire — cosmetic
}

// ============================================================
// szaco3 (GA2STRAT.ASM:2871-2897) — fast zaco bank/aim until life expires.
// ============================================================

/// `szaco3_Istrat`.
pub fn szaco3_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, szaco3_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = SZACO5_HP;
        al.ap = SZACO5_AP;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.vel = 70;
        al.animframe = 9;
        al.collflags |= COLLTYPE_ENEMY1;
        al.roty = DEG180;
        al.count = 70;
        al.snd2 = 0x0f;
        // debris / relexplode cosmetic
    }
}

/// `szaco3_strat`.
pub fn szaco3_strat(g: &mut Game, idx: u16) {
    let c = g.objs.aliens[idx as usize].count;
    if c == 0 {
        g.objs.aldead = 1;
        return;
    }
    g.objs.aliens[idx as usize].count = c - 1;
    if g.objs.aliens[idx as usize].count == 0 {
        g.objs.aldead = 1;
        return;
    }
    if zdist_more(g, idx, 400) {
        if let Some(pl) = player(g) {
            strat_aim_3d(g, idx, &pl, 1);
        }
    } else {
        szaco5_bank(g, idx);
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// sfish (GA2STRAT.ASM:2702-2865) — school fish / alone evasive fish.
// ============================================================

const SFISH_HP: u8 = 100;
const SHAPE_ELASER2: u16 = 511;
const SHAPE_NUKE: u16 = 407;

fn sfish_achase_i16(cur: i16, target: i16, shift: u32) -> i16 {
    cur.wrapping_add(target.wrapping_sub(cur) >> shift)
}

fn sfish_mother_ok(g: &Game, idx: u16) -> Option<u16> {
    let ptr = g.objs.aliens[idx as usize].ptr;
    if ptr == 0 {
        return None;
    }
    let mi = ptr.wrapping_sub(1);
    if (mi as usize) < NUMBER_AL && g.objs.aliens[mi as usize].active {
        Some(mi)
    } else {
        None
    }
}

/// `sfish_Istrat` — together (ptr mother) or alone swim setup.
pub fn sfish_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, sfish_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        al.hp = SFISH_HP;
        al.ap = HARD_AP;
        al.rotz = sf_random(&mut g.vars) as u8;
    }
    if sfish_mother_ok(g, idx).is_some() {
        // Random offset ±128 around mother.
        let rx = (sf_random(&mut g.vars) as i16).wrapping_sub(128);
        let ry = (sf_random(&mut g.vars) as i16).wrapping_sub(128);
        let rz = (sf_random(&mut g.vars) as i16).wrapping_sub(128);
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = rx;
        al.vy = ry;
        al.vz = rz;
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.type_ &= !ATZREMOVE;
        al.count = 200;
        al.sbyte1 = 200;
        al.vx = 20;
        al.vy = 10;
        al.vz = -10;
    }
}

/// `sfish_strat` — together: orbit mother (or flee laser/nuke); alone: bounce/swim.
pub fn sfish_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(8);

    if let Some(mother) = sfish_mother_ok(g, idx) {
        sfish_together(g, idx, mother);
    } else {
        sfish_alone(g, idx);
    }
    add_player_z(g, idx);
}

fn sfish_together(g: &mut Game, idx: u16, mother: u16) {
    let m = g.objs.aliens[mother as usize];
    // Mother sword1/sword2 carry nuke/laser targets when set by alone fish — skip.
    let nuke = m.sword1 as u16;
    let laser = m.sword2 as u16;

    let (ox, oy, oz, scale) = if nuke != 0 && (nuke.wrapping_sub(1) as usize) < NUMBER_AL {
        // Flee toward stored nuke object (mother.sword1).
        let ni = nuke.wrapping_sub(1);
        let n = g.objs.aliens[ni as usize];
        g.objs.aliens[idx as usize].worldx =
            sfish_achase_i16(g.objs.aliens[idx as usize].worldx, n.worldx, 4);
        g.objs.aliens[idx as usize].worldy =
            sfish_achase_i16(g.objs.aliens[idx as usize].worldy, n.worldy, 4);
        g.objs.aliens[idx as usize].worldz =
            sfish_achase_i16(g.objs.aliens[idx as usize].worldz, n.worldz, 4);
        // Face away from previous position roughly.
        let (px, py, pz) = (
            g.objs.aliens[idx as usize].worldx,
            g.objs.aliens[idx as usize].worldy,
            g.objs.aliens[idx as usize].worldz,
        );
        g.objs.aliens[idx as usize].swpx1 = px;
        g.objs.aliens[idx as usize].swpy1 = py;
        g.objs.aliens[idx as usize].swpz1 = pz;
        return;
    } else if laser != 0 {
        // Laser flee: scale offsets ×4.
        let sb = (sf_random(&mut g.vars) as u8) & 7;
        g.objs.aliens[idx as usize].sbyte1 = sb.wrapping_add(5);
        (
            g.objs.aliens[idx as usize].vx.wrapping_mul(4),
            g.objs.aliens[idx as usize].vy.wrapping_mul(4),
            g.objs.aliens[idx as usize].vz.wrapping_mul(4),
            true,
        )
    } else {
        // s_beqdec sbyte1 → if zero take laser-style (scaled) path without randomize.
        let sb1 = g.objs.aliens[idx as usize].sbyte1;
        if sb1 != 0 {
            g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
            (
                g.objs.aliens[idx as usize].vx,
                g.objs.aliens[idx as usize].vy,
                g.objs.aliens[idx as usize].vz,
                false,
            )
        } else {
            (
                g.objs.aliens[idx as usize].vx.wrapping_mul(4),
                g.objs.aliens[idx as usize].vy.wrapping_mul(4),
                g.objs.aliens[idx as usize].vz.wrapping_mul(4),
                true,
            )
        }
    };
    let _ = scale;
    let tx = m.worldx.wrapping_add(ox);
    let ty = m.worldy.wrapping_add(oy);
    let tz = m.worldz.wrapping_add(oz);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = sfish_achase_i16(al.worldx, tx, 3);
        al.worldy = sfish_achase_i16(al.worldy, ty, 3);
        al.worldz = sfish_achase_i16(al.worldz, tz, 3);
        let mut rx = al.rotx;
        let mut ry = al.roty;
        achase_angle(&mut rx, m.rotx, 4);
        achase_angle(&mut ry, m.roty, 4);
        al.rotx = rx;
        al.roty = ry;
    }
}

fn sfish_alone(g: &mut Game, idx: u16) {
    // Scan nuke / elaser2 into sword1 / sword2 (index+1).
    g.objs.aliens[idx as usize].sword1 = 0;
    g.objs.aliens[idx as usize].sword2 = 0;
    let mut found_nuke: u16 = 0;
    let mut found_laser: u16 = 0;
    let (mx, my, mz) = {
        let me = &g.objs.aliens[idx as usize];
        (me.worldx, me.worldy, me.worldz)
    };
    for i in 0..NUMBER_AL {
        if i == idx as usize || !g.objs.aliens[i].active {
            continue;
        }
        let o = &g.objs.aliens[i];
        if found_nuke == 0 && o.shape == SHAPE_NUKE && o.ap == 8 && o.vel == 50 {
            found_nuke = (i as u16).wrapping_add(1);
        }
        if o.shape == SHAPE_ELASER2 {
            let d = (o.worldx as i32 - mx as i32).abs()
                + (o.worldy as i32 - my as i32).abs()
                + (o.worldz as i32 - mz as i32).abs();
            if d <= 200 {
                found_laser = (i as u16).wrapping_add(1);
            }
        }
    }
    g.objs.aliens[idx as usize].sword1 = found_nuke as i16;
    g.objs.aliens[idx as usize].sword2 = found_laser as i16;

    let (ox, oy, oz) = {
        let al = &g.objs.aliens[idx as usize];
        (al.worldx, al.worldy, al.worldz)
    };
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.swpx1 = ox;
        al.swpy1 = oy;
        al.swpz1 = oz;
        let dy = oy.wrapping_sub(al.worldy);
        // Face opposite of motion (obj2wp to previous pos + 180 yaw).
        let dummy = Alien {
            worldx: ox,
            worldy: oy,
            worldz: oz,
            ..Alien::default()
        };
        al.roty = angle_xz(al, &dummy).wrapping_add(DEG180);
        let mut rx = al.rotx;
        let target_pitch = if dy > 20 {
            DEG45.wrapping_neg()
        } else if dy < -20 {
            DEG45
        } else {
            0
        };
        achase_angle(&mut rx, target_pitch, 2);
        al.rotx = rx.wrapping_neg();
    }

    // Bounce X/Y at ±400.
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.worldx < -400 || al.worldx >= 400 {
            al.vx = al.vx.wrapping_neg();
        }
        if al.worldy < -400 || al.worldy >= 400 {
            al.vy = al.vy.wrapping_neg();
        }
    }

    // Z swim relative to player — after swim timer, lock vz=-20 and shrink life.
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        g.objs.aliens[idx as usize].sbyte1 = 1;
        g.objs.aliens[idx as usize].vz = -20;
        let c = g.objs.aliens[idx as usize].count;
        if c > 0 {
            g.objs.aliens[idx as usize].count = c - 1;
        }
        if g.objs.aliens[idx as usize].count == 0 {
            g.objs.aldead = 1;
        }
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        let pz = g.vars.player_posz;
        let zrel = g.objs.aliens[idx as usize].worldz.wrapping_sub(pz);
        if zrel < 700 || zrel >= 1500 {
            g.objs.aliens[idx as usize].vz = g.objs.aliens[idx as usize].vz.wrapping_neg();
        }
    }
}

// ============================================================
// exit / exitcoll (GASTRATS.ASM:3701-3717)
// ============================================================

pub fn exit_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = None;
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

pub fn exitcoll_istrat(g: &mut Game, _idx: u16) {
    g.objs.aldead = 1;
}

// ============================================================
// openlr + openlrcol (GA2STRAT.ASM:2982-3003)
// ============================================================

pub fn openlr_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, openlr_strat);
    let coll = sid(g, openlrcol_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = None;
    al.hp = HARDHP;
    al.ap = HARD_AP;
}

pub fn openlr_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        g.objs.aliens[idx as usize].animframe = 0x80;
        return;
    }
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
    if anim != 9 {
        g.objs.aliens[idx as usize].animframe = 0x80 | ((anim + 1) % 10);
    }
}

pub fn openlrcol_istrat(g: &mut Game, idx: u16) {
    g.hooks.play_se(0x57);
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    strat_hit_flash(g, idx);
}

// ============================================================
// LSEQDOOR1 / LSEQDOOR2 (GCSTRATS.ASM:1151-1194) — last-seq doors.
// ============================================================

const LSEQDOOR_OPEN_Z: i16 = (MEDPSPEED as i16) * 9; // medpspeed*9
const GF2_STRATFLAG1: u8 = 1;

/// ROM `lseqdoor1_Istrat` — entrance: open when |dz| < medpspeed*9.
pub fn lseqdoor1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    if zdist_more(g, idx, LSEQDOOR_OPEN_Z) {
        g.objs.aliens[idx as usize].animframe = 0;
        return;
    }
    let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
    if anim == 0 {
        g.hooks.play_se(0x55);
    }
    if anim != 9 {
        g.objs.aliens[idx as usize].animframe = (anim + 1) % 10;
    }
}

/// ROM `lseqdoor2_Istrat` — exit: open vs viewtoobj Y; sets gf2_stratflag1.
pub fn lseqdoor2_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        lseqdoor2_open_anim(g, idx);
        return;
    }

    let view_y = {
        let v = g.vars.sv_i16(crate::common::sv::VIEWTOOBJ);
        if v >= 0 && (v as usize) < NUMBER_AL && g.objs.aliens[v as usize].active {
            g.objs.aliens[v as usize].worldy
        } else {
            player(g).map(|p| p.worldy).unwrap_or(0)
        }
    };
    let dy = abs_axis_dist(g.objs.aliens[idx as usize].worldy, view_y);
    if dy < 400 {
        let gf = g.vars.shared.game_flags2;
        g.vars.shared.game_flags2 = gf | GF2_STRATFLAG1;
    }
    if dy >= 200 {
        g.objs.aliens[idx as usize].animframe = 0;
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    lseqdoor2_open_anim(g, idx);
}

fn lseqdoor2_open_anim(g: &mut Game, idx: u16) {
    let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
    if anim == 0 {
        g.hooks.play_se(0x55);
    }
    if anim != 10 {
        g.objs.aliens[idx as usize].animframe = (anim + 1) % 11;
    }
}

// ============================================================
// SDOOR1 / SDOOR2 (GA2STRAT.ASM:536-600) — Space Armada hangar doors.
// ============================================================

fn sdoor_ship(g: &Game, idx: u16) -> Option<u16> {
    let raw = g.objs.aliens[idx as usize].sword1 as u16;
    crate::enemy_a::strat_obj_from_ptr(raw).filter(|&ship| g.objs.aliens[ship as usize].active)
}

/// ROM `sdoor1_Istrat` — entrance door, opening in front of the player and
/// closing when the guide ship aborts with `gf_stratdone2`.
pub fn sdoor1_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, sdoor1_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = Some(tick);
    al.animframe = 0;
    al.type_ |= ATGND;
}

pub fn sdoor1_strat(g: &mut Game, idx: u16) {
    if let Some(ship) = sdoor_ship(g, idx) {
        let pos = g.objs.aliens[ship as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pos.worldx;
        al.worldy = pos.worldy;
        al.worldz = pos.worldz.wrapping_sub(350);
    }
    add_player_z(g, idx);

    let gf2 = g.vars.shared.game_flags2;
    let near = player(g).is_some_and(|pl| {
        let me = g.objs.aliens[idx as usize];
        pl.worldz >= me.worldz || (me.worldz as i32 - pl.worldz as i32).abs() < (80i32 << 2)
    });
    let open = g.vars.gameflags & GF_STRATDONE2 == 0 && (gf2 & GF2_STRATFLAG1 != 0 || near);
    let anim = g.objs.aliens[idx as usize].animframe & 0x7f;
    if open {
        if anim == 0 {
            g.hooks.play_se(0x55);
        }
        if anim != 9 {
            g.objs.aliens[idx as usize].animframe = (anim + 1) % 10;
        } else if gf2 & GF2_STRATFLAG1 != 0 {
            g.vars.shared.game_flags2 = gf2 & !GF2_STRATFLAG1;
        }
    } else {
        if anim != 0 {
            g.hooks.play_se(0x56);
            g.objs.aliens[idx as usize].animframe = anim - 1;
        }
    }
}

/// ROM `sdoor2_Istrat` — exit door paired to the same guide ship.
pub fn sdoor2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, sdoor2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = Some(tick);
    al.animframe = 0;
    al.type_ |= ATGND;
    g.vars.gameflags &= !GF_STRATDONE2;
}

pub fn sdoor2_strat(g: &mut Game, idx: u16) {
    if g.vars.gameflags & GF_STRATDONE2 != 0 {
        g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    } else {
        g.objs.aliens[idx as usize].sflags &= !ASF_INVISIBLE;
    }
    if let Some(ship) = sdoor_ship(g, idx) {
        let pos = g.objs.aliens[ship as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pos.worldx;
        al.worldy = pos.worldy;
        al.worldz = pos.worldz.wrapping_sub(40);
    }

    let near = player(g).is_some_and(|pl| {
        let me = g.objs.aliens[idx as usize];
        (me.worldz as i32 - pl.worldz as i32).abs() <= (10i32 << 2)
    });
    if near {
        let anim = g.objs.aliens[idx as usize].animframe & 0x7f;
        if anim == 0 {
            g.hooks.play_se(0x55);
        }
        if anim != 9 {
            g.objs.aliens[idx as usize].animframe = (anim + 1) % 10;
        }
    }
}

// ============================================================
// CRUISER1 / CRUISER1FALL (GA2STRAT.ASM:629-688)
// ============================================================

const CRUISER1_HP: u8 = 30;
const CRUISER1_SPEED: u8 = 20;

/// ROM `cruiser1_Istrat` — sets sflag1 then falls into F/init + same-tick strat.
pub fn cruiser1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    cruiser1f_istrat(g, idx);
}

/// ROM `cruiser1F_Istrat` — fighter-gun variant (no sflag1 fire suppress).
pub fn cruiser1f_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, cruiser1_strat);
    let coll = sid(g, crate::enemy_a::hitflash_mexp_istrat);
    let exp = sid(g, cruiser1fall_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = CRUISER1_HP;
        al.ap = HARD_AP;
        al.vel = CRUISER1_SPEED;
        al.roty = (-(DEG90 as i8)) as u8;
        al.collflags |= COLLTYPE_ENEMY1;
        al.snd2 = 0x0a;
    }
    cruiser1_strat(g, idx);
}

/// ROM `cruiser1_strat` — bank turn + dual STBHMISSILE1, then cont.
pub fn cruiser1_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar B,al_sbyte1,.turn — TEST then DEC; zero → turn.
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    if s1 == 0 {
        if notdelay(g, 2) {
            g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_sub(1);
        }
    } else {
        g.objs.aliens[idx as usize].sbyte1 = s1.wrapping_sub(1);
    }

    // Fire when sflag1 clear and player in Z band [1000,2000).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0
        && !out_zdist_rng(g, idx, 1000, 2000)
        && notdelay_staggered(g, idx, 5)
    {
        let play_ptr = player_index(g).map_or(0, |p| p.wrapping_add(1));
        if let Some(shot) = fire_stb_hmissile1(g, idx) {
            let al = &mut g.objs.aliens[shot as usize];
            al.sbyte1 = (-(DEG90 as i8)) as u8;
            al.sbyte2 = 0;
            al.ptr = play_ptr;
        }
        if let Some(shot) = fire_stb_hmissile1(g, idx) {
            let al = &mut g.objs.aliens[shot as usize];
            al.sbyte1 = DEG90;
            al.sbyte2 = 0;
            al.ptr = play_ptr;
        }
    }
    cruiser1_cont(g, idx);
}

/// ROM `cruiser1_cont` — gen_3dvecs + move + playerZ.
pub fn cruiser1_cont(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// ROM `cruiser1fall_Istrat` — death tip: Lexp + armed fall strat.
pub fn cruiser1fall_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, cruiser1fall_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(tick);
    if let Some(e) = make_large_exp_obj(g, idx) {
        g.objs.aliens[e as usize].sflags2 &= !ASF2_NOEXPSND;
    }
    g.hooks.play_se(0x21);
    cruiser1fall_strat(g, idx);
}

/// ROM `cruiser1fall_strat` — tip rotx→deg45, smoke (cosmetic), cont.
pub fn cruiser1fall_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].rotx != DEG45 {
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(1);
    }
    // s_make_smoke 2 — cosmetic omitted
    cruiser1_cont(g, idx);
}

// ============================================================
// CRUISER2 / CRUISER2FIRE / CRUISER2LAUNCHER (GA2STRAT.ASM:360-460)
// ============================================================

const CRUISER2_LAUNCHER_HP: u8 = 4; // STRATEQU.INC:236
const SH_HOU_3: u16 = 409;

fn cruiser2_spawn_launcher(
    g: &mut Game,
    mother: u16,
    child_num: u8,
    rx: i8,
    ry: i8,
    rz: i8,
    delay: u8,
) {
    let Some(child) = make_obj(g, SH_HOU_3) else {
        return;
    };
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        return;
    }
    {
        let al = &mut g.objs.aliens[child as usize];
        al.relposx = rx as u8;
        al.relposy = ry as u8;
        al.relposz = rz as u8;
        al.sbyte2 = delay;
        al.collflags |= COLLTYPE_ENEMY1;
    }
    cruiser2launcher_istrat(g, child);
}

/// ROM `cruiser2fire_Istrat` — spawn 3 launchers then shared init.
pub fn cruiser2fire_istrat(g: &mut Game, idx: u16) {
    cruiser2_spawn_launcher(g, idx, 1, 20, -10, -15, 1);
    cruiser2_spawn_launcher(g, idx, 2, 20, -10, 0, 20);
    cruiser2_spawn_launcher(g, idx, 3, 20, -10, 15, 40);
    let tick = sid(g, cruiser2fire_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    cruiser2_icont(g, idx);
}

/// ROM `cruiser2_Istrat`.
pub fn cruiser2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, cruiser2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    cruiser2_icont(g, idx);
}

fn cruiser2_icont(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = HARDHP;
        al.ap = HARD_AP;
        al.rotz = (-((DEG22 + DEG11) as i8)) as u8;
        al.roty = DEG180.wrapping_add(DEG22).wrapping_add(DEG11);
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_COLLDISABLE;
        al.snd2 = 0x0b;
    }
    // s_jmpto_strat — run newly installed tick same frame
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
}

/// ROM `cruiser2fire_strat` — when children gone: accelerate, smoke, tip pitch.
pub fn cruiser2fire_strat(g: &mut Game, idx: u16) {
    if boss_count_children(g, idx) == 0 {
        let _ = speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
        if let Some(e) = make_medium_exp_obj(g, idx) {
            addrnd2pos_xy(g, e);
        }
        // frame destruct SFX + make_smoke 1 — cosmetic
        if g.objs.aliens[idx as usize].rotx != DEG45 {
            g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(1);
        }
    }
    cruiser2_strat(g, idx);
}

/// ROM `cruiser2_strat` / cont.
pub fn cruiser2_strat(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// ROM `cruiser2launcher_Istrat`.
pub fn cruiser2launcher_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, cruiser2launcher_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = CRUISER2_LAUNCHER_HP;
    al.ap = HARD_AP;
}

/// ROM `cruiser2launcher_strat` — childrelpos×4, aim tip, timed FAKEFARHMISSILE1.
pub fn cruiser2launcher_strat(g: &mut Game, idx: u16) {
    if let Some(m) = boss_get_mother_obj(g, idx) {
        let mother = g.objs.aliens[m as usize];
        // s_do_childrelpos x,4 → rel << 4
        let ox = (g.objs.aliens[idx as usize].relposx as i8 as i16) << 4;
        let oy = (g.objs.aliens[idx as usize].relposy as i8 as i16) << 4;
        let oz = (g.objs.aliens[idx as usize].relposz as i8 as i16) << 4;
        full_offset_pos(g, idx, &mother, ox, oy, oz);
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = mother.rotx;
        al.roty = mother.roty;
        al.rotz = mother.rotz;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_add_alvar B,al_rotx,#-deg45+deg22+deg11
        let pitch = (-(DEG45 as i8))
            .wrapping_add(DEG22 as i8)
            .wrapping_add(DEG11 as i8);
        al.rotx = al.rotx.wrapping_add(pitch as u8);
        al.roty = al.roty.wrapping_sub(DEG90);
    }

    // s_decbne_alvar B,al_sbyte2,.nfire
    let s2 = g.objs.aliens[idx as usize].sbyte2;
    if s2 != 0 {
        let next = s2.wrapping_sub(1);
        g.objs.aliens[idx as usize].sbyte2 = next;
        if next != 0 {
            return;
        }
    }
    g.objs.aliens[idx as usize].sbyte2 = 60;

    let right_of_view = g.objs.aliens[idx as usize].flags & AF_LEFT_PL == 0;
    let play_ptr = player_index(g).map_or(0, strat_obj_index_or_null);

    if !right_of_view && xdist_less(g, idx, 1000) {
        // .nfirecheat push world/rots → far cheat fire from z-1000, x=0
        let save_x = g.objs.aliens[idx as usize].worldx;
        let save_z = g.objs.aliens[idx as usize].worldz;
        let save_roty = g.objs.aliens[idx as usize].roty;
        let save_rotx = g.objs.aliens[idx as usize].rotx;
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.roty = DEG180;
            al.rotx = 0;
            al.worldx = 0;
            al.worldz = al.worldz.wrapping_sub(1000);
        }
        // weapon_rndrot 7,7 applied via temporary rots on firer
        let (dp, dy) = {
            let dp = ((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3);
            let dy = ((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3);
            (dp, dy)
        };
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(dp);
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(dy);
        if let Some(shot) = fire_fakefar_hmissile1(g, idx) {
            let al = &mut g.objs.aliens[shot as usize];
            al.ptr = play_ptr;
            al.collflags |= COLLTYPE_ENEMY1;
        }
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.rotx = save_rotx;
            al.roty = save_roty;
            al.worldz = save_z;
            al.worldx = save_x;
        }
        return;
    }

    if xdist_more(g, idx, 700) {
        return;
    }
    if let Some(shot) = fire_fakefar_hmissile1(g, idx) {
        let al = &mut g.objs.aliens[shot as usize];
        al.ptr = play_ptr;
        al.collflags |= COLLTYPE_ENEMY1;
    }
}

// ============================================================
// UPDOOR + UPDOORCOL (GA2STRAT.ASM:3006-3036)
// ============================================================

/// ROM `updoor_Istrat`.
pub fn updoor_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, updoor_strat);
    let coll = sid(g, updoorcol_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = None;
    al.hp = HARDHP;
    al.ap = HARD_AP;
}

/// ROM `updoor_strat` — open anim when |dz|<700; sbyte1 collide gate.
pub fn updoor_strat(g: &mut Game, idx: u16) {
    if zdist_more(g, idx, 700) {
        g.objs.aliens[idx as usize].animframe = 0;
    } else {
        g.objs.aliens[idx as usize].sbyte1 = 100;
        let anim = g.objs.aliens[idx as usize].animframe & 0x7F;
        if anim != 8 {
            g.objs.aliens[idx as usize].animframe = (anim + 1) % 10;
        }
    }
    // s_beqdec_alvar B,al_sbyte1,.coll — TEST then DEC, label empty → end
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    if s1 != 0 {
        g.objs.aliens[idx as usize].sbyte1 = s1.wrapping_sub(1);
    }
}

/// ROM `updoorcol_Istrat` — if sbyte1≠0 → coll (damage, no flash latch);
/// else flip door (rotz+=180, sbyte1=5) + hitflash.
pub fn updoorcol_istrat(g: &mut Game, idx: u16) {
    g.hooks.play_se(0x57);
    if g.objs.aliens[idx as usize].sbyte1 != 0 {
        // coll_Istrat: damage path without forcing hitflash flag path first —
        // strat_hit_flash already mirrors docoll+Icont SE.
        strat_hit_flash(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = 5;
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(DEG180);
    strat_hit_flash(g, idx);
}

// ============================================================
// MINE2 (GA2STRAT.ASM:2532-2558) — rising proximity mine.
// ============================================================

const MINE2_HP: u8 = 2;
const MINE2_AP: u8 = 8;

/// ROM `mine2_Istrat`.
pub fn mine2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, mine2_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, crate::enemy_a::mine2expnofire_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = MINE2_HP;
        al.ap = MINE2_AP;
        al.vy = -45;
        al.roty = sf_random(&mut g.vars) as u8;
        al.sflags2 |= ASF2_RELEXPLODE;
        al.collflags |= COLLTYPE_ENEMYWEAP;
    }
}

/// ROM `mine2_strat` — rise vy -45→+15 then mine2exp_Istrat.
pub fn mine2_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);

    if g.objs.aliens[idx as usize].vy == 15 {
        crate::enemy_a::mine2exp_istrat(g, idx);
        return;
    }
    if (g.objs.aliens[idx as usize].vy as i16) >= 0 {
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(2);
    } else {
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(12);
    }
    g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(1);
}

// ============================================================
// DOMA / DOMB (GASTRATS.ASM:3635-3679) — space debris drones.
// ============================================================

const DOMA_HP: u8 = 2; // STRATEQU.INC:106
const DOMA_SPEED: u8 = 30;

/// ROM `doma_Istrat` / `domb_Istrat` (same entry).
pub fn doma_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, doma_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = DOMA_HP;
        al.ap = 0;
        al.roty = DEG180;
        al.vel = DOMA_SPEED;
        al.sbyte1 = sf_random(&mut g.vars) as u8;
        al.stratstate = 0;
        gen_vecs_3d(al);
    }
}

/// Alias: ROM `domb_Istrat` shares doma.
pub fn domb_istrat(g: &mut Game, idx: u16) {
    doma_istrat(g, idx);
}

/// ROM `doma_strat`.
pub fn doma_strat(g: &mut Game, idx: u16) {
    if xz_dist_more(g, idx, 2000) {
        if let Some(p) = player(g) {
            let px = p.worldx;
            let py = p.worldy;
            let al = &mut g.objs.aliens[idx as usize];
            al.worldx = px;
            al.worldy = py;
        }
    } else if g.objs.aliens[idx as usize].stratstate == 0 {
        g.objs.aliens[idx as usize].stratstate = 1;
        g.objs.aliens[idx as usize].vx = 21;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.vx >= 20 {
            al.vx = al.vx.wrapping_sub(1);
        }
        if al.vx <= -20 {
            al.vx = al.vx.wrapping_add(1);
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// ============================================================
// SHIPS (GA2STRAT.ASM:464-475) — short-lived scrolling debris ships.
// ============================================================

/// ROM `ships_Istrat` — life 70, no collide, drifts on sword1/2 + medpspeed-5.
pub fn ships_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, ships_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags |= ASF_COLLDISABLE;
    al.count = 70;
}

/// ROM `ships_strat`.
pub fn ships_strat(g: &mut Game, idx: u16) {
    // s_dec_lifecnt
    let next = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = next;
    if next == 0 {
        g.objs.aldead = 1;
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = al.worldx.wrapping_add(al.sword1);
        al.worldy = al.worldy.wrapping_add(al.sword2);
        al.worldz = al.worldz.wrapping_add((MEDPSPEED as i16) - 5);
    }
}

// ============================================================
// SPEEDLINES (DSTRATS.ASM:3629-3638) — cosmetic streak.
// ============================================================

/// ROM `speedlines_istrat` (+ `.strat` body same entry after init fields).
pub fn speedlines_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, speedlines_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_COLLDISABLE;
        al.rotx = DEG90;
        // s_init_colanim #0 — cosmetic
    }
    speedlines_strat(g, idx);
}

fn speedlines_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_sub(120);
}

// ============================================================
// INTRO1PFALL (GISTRATS.ASM:42-66) — intro ship tip/pitch fall.
// ============================================================

/// ROM `intro1pfall_Istrat`.
pub fn intro1pfall_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, intro1pfall_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags |= ASF_COLLDISABLE;
}

/// ROM `intro1pfall_strat` — countdown sbyte1 then falling init.
pub fn intro1pfall_strat(g: &mut Game, idx: u16) {
    // s_beqdec_alvar B,al_sbyte1,intro1pfalling_init
    let s1 = g.objs.aliens[idx as usize].sbyte1;
    if s1 == 0 {
        intro1pfalling_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 = s1.wrapping_sub(1);
}

/// ROM `intro1pfalling_init`.
pub fn intro1pfalling_init(g: &mut Game, idx: u16) {
    let tick = sid(g, intro1pfalling_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    if let Some(e) = make_medium_exp_obj(g, idx) {
        g.objs.aliens[e as usize].worldy = g.objs.aliens[e as usize].worldy.wrapping_sub(60);
    }
    intro1pfalling_strat(g, idx);
}

/// ROM `intro1pfalling_strat` — ramp pitch rate to 6, tip until rotx>deg90.
pub fn intro1pfalling_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte2 != 6 && notdelay(g, 1) {
        g.objs.aliens[idx as usize].sbyte2 = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
    }
    // s_jmp_alvarMORE B,al_rotx,#deg90 — unsigned > deg90
    if g.objs.aliens[idx as usize].rotx <= DEG90 {
        let add = g.objs.aliens[idx as usize].sbyte2;
        g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(add);
    }
}

// ============================================================
// pillar3f fall/stay (KSTRATS.ASM:634-670) — fog pillar variant leaves.
// ============================================================

const PILLAR3F_HP: u8 = 8;
const PILLAR3F_AP: u8 = 8;
const PILLAR3F_DIST: i16 = 500;
const PILLAR3FFALL_HP: u8 = 4;

pub fn pillar3f_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, pillar3f_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = PILLAR3F_HP;
    al.ap = PILLAR3F_AP;
}

pub fn pillar3f_strat(g: &mut Game, idx: u16) {
    if zdist_less(g, idx, PILLAR3F_DIST)
        || g.objs.aliens[idx as usize].hp < PILLAR3FFALL_HP
        || g.objs.aliens[idx as usize].hitflags & 0x02 != 0
    {
        pillar3ffall_init(g, idx);
    }
}

/// ASM `pillar3ffall_i` (KSTRATS.ASM:648-659).
fn pillar3ffall_init(g: &mut Game, idx: u16) {
    let tick = sid(g, pillar3ffall_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_SHADOW;
        // s_rightview_strat: leftpl clear (right of view) keeps +4; left → -4.
        al.sbyte1 = 4;
        if al.flags & AF_LEFT_PL != 0 {
            al.sbyte1 = (-4i8) as u8;
        }
        al.sbyte2 = 16;
    }
    // ROM: s_make_obj #bouncyball; copypos (no z−10); alptrs explode×3; kill_obj.
    if let Some(ball) = make_obj(g, 0) {
        let (px, py, pz) = {
            let me = &g.objs.aliens[idx as usize];
            (me.worldx, me.worldy, me.worldz)
        };
        let exp = sid(g, strat_explode);
        let al = &mut g.objs.aliens[ball as usize];
        al.worldx = px;
        al.worldy = py;
        al.worldz = pz;
        al.stratptr = Some(exp);
        al.collstratptr = Some(exp);
        al.expstratptr = Some(exp);
        kill_obj(al);
    }
}

pub fn pillar3ffall_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(al.sbyte1);
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        pillar3fstay_istrat(g, idx);
    }
}

pub fn pillar3fstay_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, pillar3fstay_wait);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags &= !ASF_SHADOW;
    al.stratptr = Some(tick);
}

fn pillar3fstay_wait(_g: &mut Game, _idx: u16) {}

// ============================================================
// warp (IS 161; GA2STRAT.ASM:1144-1255) — 6-state space fighter.
// ============================================================

const WARP_HP: u8 = 4; // STRATEQU.INC:241
const WARP_AP: u8 = 8; // STRATEQU.INC:242

/// warppostab (GA2STRAT.ASM:1244-1252): (x,y) pairs; sbyte1 is byte index.
const WARP_POS: [(i16, i16); 8] = [
    (500, -500 - SPACE_VIEWCY),
    (-500, -500 - SPACE_VIEWCY),
    (-500, 500 - SPACE_VIEWCY),
    (500, 500 - SPACE_VIEWCY),
    (0, 0 - SPACE_VIEWCY),
    (0, -250 - SPACE_VIEWCY),
    (250, 250 - SPACE_VIEWCY),
    (-250, 250 - SPACE_VIEWCY),
];

/// `warp_Istrat`.
pub fn warp_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, warp_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
        al.hp = WARP_HP;
        al.ap = WARP_AP;
        al.rotx = DEG90;
        al.sbyte1 = ((sf_random(&mut g.vars) as u8) & 7) << 2; // scale <<2
        al.sbyte2 = 1;
        al.sbyte3 = 0;
        al.sbyte4 = 4;
        al.stratstate = 0;
    }
}

/// `warp_strat` — 6-state machine.
pub fn warp_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => warp_state0(g, idx),
        1 => warp_state1(g, idx),
        2 => warp_state2(g, idx),
        3 => warp_state3(g, idx),
        4 => {
            let ang = sf_random(&mut g.vars) as u8;
            make_xyvec(&mut g.objs.aliens[idx as usize], ang, 40);
            g.objs.aliens[idx as usize].vz = -30;
            g.objs.aliens[idx as usize].stratstate = 5;
        }
        _ => {
            apply_velocity(&mut g.objs.aliens[idx as usize]);
        }
    }
    // shape morph from sbyte3 — cosmetic omitted
    add_player_z(g, idx);
}

fn warp_state0(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(12);
    let i = (g.objs.aliens[idx as usize].sbyte1 / 4) as usize % WARP_POS.len();
    let (tx, ty) = WARP_POS[i];
    let x = achase_word(g.objs.aliens[idx as usize].worldx, tx, 2);
    let y = achase_word(g.objs.aliens[idx as usize].worldy, ty, 2);
    g.objs.aliens[idx as usize].worldx = x;
    g.objs.aliens[idx as usize].worldy = y;
    let z_tgt = g.objs.aliens[idx as usize]
        .sword1
        .wrapping_add(g.vars.player_posz)
        .wrapping_add(1500);
    g.objs.aliens[idx as usize].worldz = achase_word(g.objs.aliens[idx as usize].worldz, z_tgt, 2);
    if x == tx && y == ty {
        next_state(g, idx);
    }
}

fn warp_state1(g: &mut Game, idx: u16) {
    let mut rotz = g.objs.aliens[idx as usize].rotz;
    achase_angle(&mut rotz, 0, 2);
    g.objs.aliens[idx as usize].rotz = rotz;
    g.objs.aliens[idx as usize].sbyte2 = 20;
    if g.vars.gameframe & 3 != 0 {
        return;
    }
    if g.objs.aliens[idx as usize].sbyte3 == 6 {
        next_state(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_add(2);
}

fn warp_state2(g: &mut Game, idx: u16) {
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, 0, 2);
        g.objs.aliens[idx as usize].rotx = rotx;
        let mut rotz = g.objs.aliens[idx as usize].rotz;
        achase_angle(&mut rotz, 0, 2);
        g.objs.aliens[idx as usize].rotz = rotz;
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
    }
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 == 0 {
        next_state(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 = sb2 - 1;
    // notdelay 3 with phase — simplified: gameframe&7==0
    if g.vars.gameframe & 7 != 0 {
        return;
    }
    // jmp_random .norm,70 → 70% normal, else home
    let rnd = (sf_random(&mut g.vars) as u8) as u16 * 100 / 256;
    if rnd < 70 {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&me, &pl)
                .wrapping_add(((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3));
            let pitch = strat_pitch_toward(&me, &pl)
                .wrapping_add(((sf_random(&mut g.vars) as u8) & 7).wrapping_sub(3));
            strat_fire_relslowlaser(g, idx, pitch, yaw);
        }
    } else if let Some(_pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        strat_fire_relslowlaserhome(g, idx, me.rotx, me.roty);
    }
}

fn warp_state3(g: &mut Game, idx: u16) {
    {
        let mut rotx = g.objs.aliens[idx as usize].rotx;
        achase_angle(&mut rotx, DEG90, 2);
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = rotx;
        al.rotz = al.rotz.wrapping_add(12);
        al.sflags |= ASF_COLLDISABLE;
    }
    if g.vars.gameframe & 3 != 0 {
        return;
    }
    // random sword1 sign-extended <<1
    let r = sf_random(&mut g.vars) as i8 as i16;
    g.objs.aliens[idx as usize].sword1 = r.wrapping_shl(1);
    g.objs.aliens[idx as usize].sbyte3 = g.objs.aliens[idx as usize].sbyte3.wrapping_sub(2);
    if g.objs.aliens[idx as usize].sbyte3 != 0 {
        return;
    }
    g.objs.aliens[idx as usize].stratstate = 0;
    let sb4 = g.objs.aliens[idx as usize].sbyte4.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte4 = sb4;
    if sb4 == 0 {
        g.objs.aliens[idx as usize].stratstate = 4;
    }
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(4);
    if g.objs.aliens[idx as usize].sbyte1 == 32 {
        g.objs.aliens[idx as usize].sbyte1 = 0;
    }
}

// ------------------------------------------------------------
// tree1 (IS 204) / tree2 (IS 205) — DSTRATS.ASM:1976-2063.
// Indestructible (tree1HP = hardHP = -1) ENEMY1 sprouting-tree scenery. Only the
// base-trunk grow is modelled (see the section doc for the scoped-out sprouty
// segment-chain / leaf-flower bloom). sflag3/4/5/6 (leaf/flower/kinky markers)
// are consumed ONLY by that scoped-out bloom code, so they are not set here.
// ------------------------------------------------------------

/// `tree1_istrat` (DSTRATS.ASM:2016-2043): flower/leaf tree — random height
/// (rnd&3)+1, lower the root by sprout_maxy/2, anim speed 2 / tail timer 255,
/// ENEMY1 + nohitaffect + hp=-1, anim 0. Falls into the grow tick.
pub fn tree1_istrat(g: &mut Game, idx: u16) {
    tree1_init(g, idx);
}

fn tree1_init(g: &mut Game, idx: u16) {
    let r = (sf_random(&mut g.vars) as u8) & 3; // s_set_alvar2rnd al_sbyte1,#3
    tree_setup(g, idx, r.wrapping_add(1)); // s_inc_alvar -> [1,4]
                                           // tree1 has no player-relative tilt (unlike tree2); s_not_alsflag sflag3 and
                                           // the leaf/flower flags drive only the scoped-out bloom.
}

/// `tree2_istrat` (DSTRATS.ASM:1976-2014): as tree1 but tilts toward the player
/// (roty ±deg45, sbyte2 = ±deg22 overhang) and casts a shadow.
pub fn tree2_istrat(g: &mut Game, idx: u16) {
    tree2_init(g, idx);
}

/// ROM `tree3_istrat` (DSTRATS.ASM:1971) — height 255, forced into tree2 entry.
pub fn tree3_istrat(g: &mut Game, idx: u16) {
    // s_set_alvar B,al_sbyte1,#255 ; jmp tree2.forcedentry (skip random height).
    // Pre-set sbyte2 like tree2 before tilt; height forced to 255.
    let mut sbyte2 = DEG22;
    let self_x = g.objs.aliens[idx as usize].worldx;
    let px = player(g).map(|p| p.worldx).unwrap_or(0);
    if self_x.wrapping_sub(px) < 0 {
        sbyte2 = sbyte2.wrapping_neg();
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(DEG45);
    } else {
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize]
            .roty
            .wrapping_add(0u8.wrapping_sub(DEG45));
    }
    tree_setup(g, idx, 255);
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = sbyte2;
    al.sflags |= ASF_SHADOW;
}

/// Alias of tree1_istrat2 / tree2_istrat2 (same body after tilt setup).
pub fn tree1_istrat2(g: &mut Game, idx: u16) {
    tree1_init(g, idx);
}
pub fn tree2_istrat2(g: &mut Game, idx: u16) {
    tree2_init(g, idx);
}

fn tree2_init(g: &mut Game, idx: u16) {
    let r = (sf_random(&mut g.vars) as u8) & 3;
    let mut sbyte2 = DEG22; // s_set_alvar al_sbyte2,#deg22
                            // s_cmp_alvars W,x,al_worldx,y,al_worldx ; s_bmi .otherway.
    let self_x = g.objs.aliens[idx as usize].worldx;
    let px = player(g).map(|p| p.worldx).unwrap_or(0);
    if self_x.wrapping_sub(px) < 0 {
        // .otherway: s_neg_alvar sbyte2 ; s_add_alvar al_roty,#deg45
        sbyte2 = sbyte2.wrapping_neg();
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(DEG45);
    } else {
        // .notthatway: s_add_alvar al_roty,#-deg45
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize]
            .roty
            .wrapping_add(0u8.wrapping_sub(DEG45));
    }
    tree_setup(g, idx, r.wrapping_add(1));
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte2 = sbyte2;
    al.sflags |= ASF_SHADOW; // s_set_alsflag x,shadow (tree2_istrat2)
}

/// Shared tree init body (DSTRATS.ASM:1985-2043, tree-common part): sprout root
/// lowering + destructible-scenery wiring + the grow tick. `height` = sbyte1
/// (number of scoped-out segment generations, stored for fidelity).
fn tree_setup(g: &mut Game, idx: u16, height: u8) {
    let tick = sid(g, tree_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = height;
        al.worldy = al.worldy.wrapping_sub(SPROUT_MAXY / 2); // s_sub al_worldy,#sprout_maxy/2
                                                             // al_sword1 lo = anim speed 2, hi = tail timer 255 (sword1 = 0xFF02).
        al.sword1 = 0xFF02u16 as i16;
        al.stratptr = Some(tick); // s_set_alptrs x,sprouty.strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = HARDHP; // s_set_aldata sproutiHP(=tree1HP=-1),#tree1ap
        al.ap = TREE1_AP;
        al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag x,nohitaffect
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,ENEMY1
        al.animframe = 0x80; // s_init_anim x,#0
    }
    tree_strat(g, idx); // jmp sprouty.strat (falls into the grow tick)
}

/// `sprouty.strat` .notsnake grow (DSTRATS.ASM:2147-2150), scoped to the base
/// trunk: grow the anim by the anim speed (sword1 lo) toward the cap 8, then hold
/// (the ROM's `.finished` -> `.strat2` segment spawn is the scoped-out chain).
fn tree_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    let speed = (al.sword1 as u16 & 0xff) as u8; // svar_byte1 = al_sword1 lo
    let cur = al.animframe & 0x7F;
    if cur != 8 {
        // s_add_anim x,svar_byte1,#8 (clamp/hold at the cap in this scoped port).
        let f = cur.wrapping_add(speed).min(8);
        al.animframe = 0x80 | f;
    }
}

// ============================================================
// Final niche enemies/objects — shou0 / shou0a (rotating plasma turrets),
// iris (damage-triggered aperture door), truck (rail ground vehicle), item6
// (wireframe-ship power-up), and kichi2. ASM is the sole
// ground truth (no C-oracle): GA2STRAT.ASM:1850-1897 (shou0), DSTRATS.ASM:1375-
// 1399 + D3STRATS.ASM:1090-1093 (iris/iris_1), GASTRATS.ASM:1575-1623 (truck),
// GASTRATS.ASM:2598-2621 (item6), D2STRATS.ASM:725-728 (kichi2).
//
// INDEX DISCIPLINE (sf-map placement == exact zero-based ISTRATS.ASM row):
// iris=47, truck=48, kichi2=140, item6=175, shou0=177, shou0a=178.
// `kichi2_istrat` jumps directly to `nocoll_istrat`, so row 140 deliberately
// reuses the collision-disabled initializer while remaining distinct from
// kdoor=139 and kdoor2=141.
//
// STATE MACHINES (per-fn cites):
//   shou0  : enemy1 plasma turret. Init rolls sbyte1 in {0,1,2} (reroll on 3).
//            Each tick, while 500<=|dz|<2500, spins two of its three rot axes by
//            +6 (which pair is sbyte1-selected) and — on a per-object /16 gate —
//            fires a player-aimed PLASMA. shou0a = shou0 + sflag1: fires on the
//            slower /32 gate (GA2STRAT.ASM:1874-1897).
//   iris   : aperture door, hp 127. Sealed while hp>=125; once damaged below
//            (127-irisHP=125) it animates open 0->8 and holds. The iris_1 inner
//            mesh child (fling.makeobj) is a passive colldisable no-op with an
//            unresolvable shape — SCOPED OUT (DSTRATS.ASM:1375-1399).
//   truck  : Zenemy rail vehicle. Drives along sbyte1 heading (speed 30), body
//            roty chasing sbyte1. In 1000<=|dz|<3000 it lobs ONE homing HMISSILE1
//            at the player (sflag2 one-shot latch). On hitting a rail_4 it snaps
//            to the rail and turns ±deg90 by the rail's sbyte1 (GASTRATS.ASM:
//            1575-1623).
//   item6  : wireframe-ship power-up (colldisable). Drifts +z (until sbyte1 set)
//            and spins roty+=4; when the player closes within 120 z & 60 xy it
//            grants the wireframe ship, chimes ($16) and removes itself. The full
//            ship-swap (curr_ship/select_ship_l, shieldup, pnumhits) is player-
//            progression machinery not modelled in sf-game; only the modelled
//            pshipflags2|=psf2_wireship bit + chime + self-remove are applied
//            here — SCOPED, see item6_strat (GASTRATS.ASM:2598-2621).
// ============================================================

const IS_IRIS: usize = 47;
const IS_TRUCK: usize = 48;
const IS_ITEM6: usize = 175;
const IS_SHOU0: usize = 177;
const IS_SHOU0A: usize = 178;

const SHOU0_HP: u8 = 2; // STRATEQU.INC:249 shou0HP
const SHOU0_AP: u8 = 12; // STRATEQU.INC:250 shou0AP
const TRUCK_HP: u8 = 4; // STRATEQU.INC:142 truckHP
const TRUCK_AP: u8 = 8; // STRATEQU.INC:143 truckAP
/// iris opens once hp < 127-irisHP; irisHP=2 (STRATEQU.INC:218) -> 125.
const IRIS_OPEN_HP: u8 = 125;
/// PLASMA weapon facts (fire_plasma, GSTRATS.ASM:2406-2414): speed 80, life
/// 30+70, ap plasmaAP(=PLASMA_AP=10, defined above). Modelled as a plain flat
/// projectile via spawn_projectile — the `relflatmiss` player-relative scroll
/// is approximated exactly as szaco0's rel-laser is (strat_fire_relslowlaser).
const PLASMA_SPEED: u8 = 80;
const PLASMA_LIFE: u8 = 100;
/// HMISSILE1 facts (STRATEQU.INC / enemy_a): speed 60, life 100, ap 8.
const HMISSILE1_SPEED: u8 = 60;
const HMISSILE1_LIFE: u8 = 100;
const HMISSILE1_AP: u8 = 8;
/// truck muzzle `s_weapon_pos #0,#-105>>weapon_scale,#100>>weapon_scale`
/// (weapon_scale=2 -> /4): (0, -26, 25).
const TRUCK_MUZZLE_Y: i16 = -26; // -105>>2
const TRUCK_MUZZLE_Z: i16 = 25; // 100>>2
/// rail_4 collision partner shape (sf-map SH_RAIL_4, route3 common.rs:298).
const SH_RAIL_4: u16 = 5;
/// psf2_wireship (GILESALC.INC:85) — the wireframe-ship power-up bit.
const PSF2_WIRESHIP: u8 = 2;

// ------------------------------------------------------------
// shou0 / shou0a (IS 178 / 179) — GA2STRAT.ASM:1850-1897.
// ------------------------------------------------------------

/// `shou0a_Istrat` (GA2STRAT.ASM:1850-1852): set sflag1 (the /32 "type-a" fire
/// cadence), then FALL THROUGH into `shou0_Istrat`.
fn shou0a_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    shou0_init(g, idx);
}

/// `shou0_Istrat` (GA2STRAT.ASM:1853-1859): wire strats/data, enemy1, and roll
/// sbyte1 in {0,1,2} (`.again`: rnd&3, reroll on 3). No s_end_strat -> falls into
/// the tick this frame.
pub fn shou0_istrat(g: &mut Game, idx: u16) {
    shou0_init(g, idx);
}

fn shou0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, shou0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    // .again: s_set_alvar2rnd sbyte1,#3 ; s_jmp_alvarEQ #3,.again (reroll on 3).
    let mut sb1 = (sf_random(&mut g.vars) as u8) & 3;
    while sb1 == 3 {
        sb1 = (sf_random(&mut g.vars) as u8) & 3;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,shou0_strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = SHOU0_HP; // s_set_aldata x,#shou0HP,#shou0AP
        al.ap = SHOU0_AP;
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,enemy1
        al.sbyte1 = sb1;
    }
    shou0_strat(g, idx); // fall-through
}

/// `shou0_strat` (GA2STRAT.ASM:1860-1897): while the player is in [500,2500) z,
/// spin the sbyte1-selected pair of rot axes by +6, then fire a player-aimed
/// PLASMA on the fire gate (shou0 /16, shou0a /32, both al1pt-staggered).
pub fn shou0_strat(g: &mut Game, idx: u16) {
    // s_jmp_OUTZdistrng x,y,#500,#2500,.nospin (fall-through == IN range).
    if !zdist_in_range(g, idx, 500, 2500) {
        return; // .nospin -> s_end_strat
    }
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    {
        let al = &mut g.objs.aliens[idx as usize];
        match sb1 {
            0 => {
                // .nsp0 not taken: roty+=6, rotx+=6.
                al.roty = al.roty.wrapping_add(6);
                al.rotx = al.rotx.wrapping_add(6);
            }
            1 => {
                // .nsp1 not taken: roty+=6, rotz+=6.
                al.roty = al.roty.wrapping_add(6);
                al.rotz = al.rotz.wrapping_add(6);
            }
            _ => {
                // .nsp1: rotx+=6, rotz+=6.
                al.rotx = al.rotx.wrapping_add(6);
                al.rotz = al.rotz.wrapping_add(6);
            }
        }
    }
    // .fire: s_jmp_alsflag sflag1,.typea. shou0 -> notdelay 4, shou0a -> 5.
    let bits = if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        5
    } else {
        4
    };
    if notdelay_staggered(g, idx, bits) {
        shou0_fire(g, idx);
    }
}

/// `s_weapon_pos #0,#0,#0 ; s_weapon_rots2obj y ; s_fire_weapon x,PLASMA`
/// (GA2STRAT.ASM:1888-1891/1896): muzzle at the turret centre, shot rotation
/// aimed straight at the player, a PLASMA laser (`fire_plasma` → enemybattrysound).
fn shou0_fire(g: &mut Game, idx: u16) {
    let Some(p) = player(g) else { return };
    let me = g.objs.aliens[idx as usize];
    let yaw = angle_xz(&me, &p); // s_weapon_rots2obj y (3D aim, yaw) — raw, not nega
    let pitch = strat_pitch_toward(&me, &p); // (pitch)
    let _ = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        PLASMA_SPEED,
        PLASMA_LIFE,
        PLASMA_AP,
        // fire_plasma colltype laser+enemyweap (GSTRATS.ASM:2411-2412).
        ACF_COLLTYPE1 | ACF_COLLTYPE4,
    );
    // ROM `jsl enemybattrysound_l` via fire_plasma (GSTRATS.ASM:2417).
    g.hooks
        .make_snd(PosSndFamilyId::EnemyBattry, me.worldx, me.worldz);
}

// ------------------------------------------------------------
// iris (IS 48) — DSTRATS.ASM:1375-1399 + D3STRATS.ASM:1090-1093.
// ------------------------------------------------------------

// ------------------------------------------------------------
// tunnela (DSTRATS.ASM:1347) — HF5 toggles between animating / idle.
// ------------------------------------------------------------

const TUNNEL_HP: u8 = 20; // STRATEQU.INC:65 tunnelHP
const HF5: u8 = 1 << 4; // VARS.INC:171

/// ROM `tunnela_Istrat` (DSTRATS.ASM:1347) — falls through into `tunnela_strat`.
pub fn tunnela_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, tunnela_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = TUNNEL_HP;
        al.ap = HARD_AP;
        al.animframe = 0x80; // s_init_anim x,#0
    }
    tunnela_strat(g, idx);
}

/// ROM `tunnela_strat` — dincanim #16; HF5 → switch to tunnela2.
pub fn tunnela_strat(g: &mut Game, idx: u16) {
    add_anim_wrap(&mut g.objs.aliens[idx as usize], 1, 16);
    if g.objs.aliens[idx as usize].hitflags & HF5 == 0 {
        return;
    }
    g.objs.aliens[idx as usize].hitflags &= !HF5;
    let s2 = sid(g, tunnela2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s2);
}

/// ROM `tunnela2_strat` — idle; HF5 → back to tunnela_strat.
pub fn tunnela2_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].hitflags & HF5 == 0 {
        return;
    }
    g.objs.aliens[idx as usize].hitflags &= !HF5;
    let s = sid(g, tunnela_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// `iris_istrat` (DSTRATS.ASM:1375-1391): faces deg180, wires strats/data
/// (hp 127, hardAP), anim 0. The iris_1 inner aperture child (fling.makeobj ->
/// iris_1_istrat, a passive colldisable no-op with an unresolvable shape) is
/// SCOPED OUT — it has zero gameplay effect. No s_end_strat -> falls into the
/// tick this frame.
pub fn iris_istrat(g: &mut Game, idx: u16) {
    iris_init(g, idx);
}

/// `iris_1_istrat` — passive colldisable aperture child (no-op gameplay shell).
pub fn iris_1_istrat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = None;
    al.collstratptr = None;
    al.expstratptr = None;
    al.sflags |= ASF_COLLDISABLE;
}

fn iris_init(g: &mut Game, idx: u16) {
    let tick = sid(g, iris_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180; // s_set_alvar B,x,al_roty,#deg180
        al.stratptr = Some(tick); // s_set_alptrs x,iris_strat,hitflash,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = 127; // s_set_aldata x,#127,#hardAP
        al.ap = HARD_AP;
        al.animframe = 0x80; // s_init_anim x,#0
    }
    iris_strat(g, idx); // fall-through
}

/// `iris_strat` (DSTRATS.ASM:1392-1399): stays sealed while hp>=125
/// (`s_cmp_alvar al_hp,#127-irisHP ; bcs .miss`); once damaged below that, opens
/// the aperture anim toward 8 (dincanimjmp, the 4-arg jmp form -> clamp at the
/// max-1 frame 7) and holds. The door-open sound is cosmetic.
pub fn iris_strat(g: &mut Game, idx: u16) {
    // s_cmp_alvar B,x,al_hp,#125 ; bcs .miss (hp>=125 == sealed).
    if g.objs.aliens[idx as usize].hp >= IRIS_OPEN_HP {
        return;
    }
    // Advance animation toward the cap of 8, then hold.
    add_anim_cap(&mut g.objs.aliens[idx as usize], 1, 8);
}

// ------------------------------------------------------------
// truck (IS 49) — GASTRATS.ASM:1575-1623.
// ------------------------------------------------------------

/// `s_gen_vecs x,al_sbyte1,al_vel` — flat (vx,vz) from the sbyte1 HEADING (not
/// roty; roty lags behind as the visual body-turn chase). Unsigned velocity.
fn truck_gen_vecs(al: &mut Alien) {
    use crate::snes_trig::{mulslog, COSTAB, SINTAB};
    let angle = al.sbyte1 as usize;
    let vel = al.vel as i32;
    al.vx = mulslog(vel, SINTAB[angle] as i32) as i16;
    al.vy = 0;
    al.vz = mulslog(vel, COSTAB[angle] as i32) as i16;
}

/// `truck_Istrat` (GASTRATS.ASM:1575-1583): wire strats (tick / truckcol / explode)
/// + data, seed the heading sbyte1 from roty, speed 30, gen the drive vecs, and
/// mark Zenemy. s_end_strat — does NOT run the tick this frame.
fn truck_init(g: &mut Game, idx: u16) {
    let tick = sid(g, truck_strat);
    let coll = sid(g, truckcol_strat);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,truck_strat,truckcol,explode
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = TRUCK_HP; // s_set_aldata x,#truckHP,#truckAP
        al.ap = TRUCK_AP;
        al.sbyte1 = al.roty; // s_copy_alvar2alvar B,x,al_sbyte1,x,al_roty
        al.vel = 30; // s_set_speed x,#30
        truck_gen_vecs(al); // s_gen_vecs x,al_sbyte1,al_vel
        al.collflags |= COLLTYPE_ZENEMY; // s_set_colltype x,Zenemy
    }
    // s_end_strat (no fall-through).
}

/// `truck_strat` (GASTRATS.ASM:1584-1587): clear the rail-turn debounce (sflag1)
/// then run `truck_norm`.
fn truck_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1; // s_clr_alsflag x,sflag1
    truck_norm(g, idx); // s_brl truck_norm
}

/// `truck_cont` (GASTRATS.ASM:1588-1589): regenerate the drive vecs from the
/// (possibly just-turned) sbyte1 heading, then fall into `truck_norm`.
pub fn truck_cont(g: &mut Game, idx: u16) {
    truck_gen_vecs(&mut g.objs.aliens[idx as usize]); // s_gen_vecs x,al_sbyte1,al_vel
    truck_norm(g, idx);
}

/// `truck_norm` (GASTRATS.ASM:1590-1604): in 1000<=|dz|<3000, on the global /16
/// gate, fire ONE homing HMISSILE1 at the player (sflag2 one-shot latch). Always
/// chase roty toward the heading sbyte1 (rate 1) and drive.
fn truck_norm(g: &mut Game, idx: u16) {
    // s_jmp_outZdistrng x,y,#1000,#3000,.nfire
    if zdist_in_range(g, idx, 1000, 3000)
        // s_jmp_NOTdelay 4,.nfire (no al1pt — global /16 gate).
        && notdelay(g, 4)
        // s_jmp_alsflag x,sflag2,.nfire (already fired once).
        && g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 == 0
    {
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG2; // s_set_alsflag x,sflag2
        truck_fire_missile(g, idx);
    }
    // .nfire: s_achase_alvar2alvar B,x,al_roty,x,al_sbyte1,1 (body-turn toward heading).
    let mut roty = g.objs.aliens[idx as usize].roty;
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    achase_angle(&mut roty, sb1, 1);
    g.objs.aliens[idx as usize].roty = roty;
    apply_velocity(&mut g.objs.aliens[idx as usize]); // s_add_vecs2pos x
}

/// `s_weapon_pos #0,#-105>>2,#100>>2 ; s_weapon_rot #0,#0 ; s_fire_weapon
/// x,HMISSILE1 ; s_set_alvar y,al_ptr,playpt` (GASTRATS.ASM:1598-1602): a homing
/// missile from the truck-relative muzzle, aligned with the truck's facing,
/// targeting the player. Mirrors boss7launcher_fire_hmissile1's wiring.
fn truck_fire_missile(g: &mut Game, idx: u16) {
    let Some(player_idx) = player_index(g) else {
        return;
    };
    let Some(shot) = make_obj(g, 0) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    // s_weapon_pos muzzle rotated by the firer's full rots (gen_weapon flags 1,1,1).
    full_offset_pos(g, shot, &me, 0, TRUCK_MUZZLE_Y, TRUCK_MUZZLE_Z);
    let s_tick = sid(g, hmissile1_strat);
    let (_gen, s_coll) = projectile_strat_ids(g);
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.rotx = me.rotx; // s_weapon_rot #0,#0 -> shot rots == firer rots
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
        al.immuneptr = idx;
        al.fireobjptr = player_idx + 1; // s_set_alvar y,al_ptr,playpt (homing target)
    }
    gen_vecs_3d(&mut g.objs.aliens[shot as usize]);
    // ROM `s_fire_weapon x,HMISSILE1` → gen_weapon `jsl missilesound_l`.
    g.hooks
        .make_snd(PosSndFamilyId::Missile, me.worldx, me.worldz);
}

/// `truckcol_Istrat` (GASTRATS.ASM:1606-1623): the collision handler. On hitting
/// anything other than a rail_4, take a normal hit (hitflash). On a rail_4:
/// clear the collide flag, and (once per contact, sflag1-debounced) snap onto the
/// rail and turn ±deg90 chosen by the RAIL's sbyte1 (==1 -> -deg90, else +deg90),
/// then run truck_cont.
pub fn truckcol_istrat(g: &mut Game, idx: u16) {
    truckcol_strat(g, idx);
}

fn truckcol_strat(g: &mut Game, idx: u16) {
    // s_set_objtobealvar y,x,al_collobjptr ; s_jmp_alvarne.w al_shape,#rail_4,hitflash.
    let partner = g.objs.aliens[idx as usize].collobjptr;
    let is_rail = (partner as usize) < NUMBER_AL
        && g.objs.aliens[partner as usize].active
        && g.objs.aliens[partner as usize].shape == SH_RAIL_4;
    if !is_rail {
        strat_hit_flash(g, idx); // hitflash_Istrat (normal damage)
        return;
    }
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE; // s_clr_alsflag x,collide
                                                        // s_jmp_alsflag x,sflag1,truck_cont (already turned this contact -> just recompute).
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        truck_cont(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    copy_pos(g, idx, partner); // s_copy_pos x,y (snap the truck onto the rail)
                               // s_jmp_alvarNE.w B,y,al_sbyte1,#1,.not_right — branch on the RAIL's sbyte1.
    let rail_sb1 = g.objs.aliens[partner as usize].sbyte1;
    if rail_sb1 == 1 {
        // s_add_alvar B,x,al_sbyte1,#-deg90 (turn one way).
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(DEG90);
    } else {
        // .not_right: s_sub_alvar B,x,al_sbyte1,#-deg90 (== +deg90, the other way).
        g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_add(DEG90);
    }
    truck_cont(g, idx);
}

// ------------------------------------------------------------
// item6 (IS 176) — GASTRATS.ASM:2598-2621. Wireframe-ship power-up.
// ------------------------------------------------------------

/// `item6_Istrat` (GASTRATS.ASM:2598-2601): tick=item6_strat (no collide/explode),
/// colldisable. There is NO s_start_strat/s_end_strat guard here — it falls
/// straight into `item6_strat` the same frame.
/// `item6_Istrat` public entry (IS 176 / GASTRATS.ASM:2598).
pub fn item6_istrat(g: &mut Game, idx: u16) {
    item6_init(g, idx);
}

fn item6_init(g: &mut Game, idx: u16) {
    let tick = sid(g, item6_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick); // s_set_alptrs x,item6_strat,0,0
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
    }
    item6_strat(g, idx); // fall-through
}

/// `item6_strat` (GASTRATS.ASM:2602-2621): remove on player death; drift +z
/// (until sbyte1 is set) and spin roty+=4; when the player closes within 120 z &
/// 60 xy, grant wireframe ship (`psf2_wireship` + `shieldup=1` + `pnumhits=0`),
/// chime `$16`, and self-remove. `curr_ship`/`select_ship_l` mesh swap is scoped.
pub fn item6_strat(g: &mut Game, idx: u16) {
    // s_remove_ifplayerdead x (mirrors item5: removes on pshipflags2 HP0 / no player).
    let Some(pl) = player(g) else {
        g.objs.aldead = 1;
        return;
    };
    if g.vars.pshipflags2 & PSF2_PLAYERHP0 != 0 {
        g.objs.aldead = 1;
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        // s_jmp_alvarNOTZERO B,x,al_sbyte1,.stop ; s_add_alvar W,x,al_worldz,#20.
        if al.sbyte1 == 0 {
            al.worldz = al.worldz.wrapping_add(20);
        }
        al.roty = al.roty.wrapping_add(4); // s_add_alvar B,x,al_roty,#4
    }
    // s_set_objtobeplayer y ; s_jmp_Zdistmore #60*2 ; s_jmp_XYdistmore #30*2 (skip
    // when |dz|>=120 or |dx|+|dy|>=60 — pickup needs strictly less).
    let me = g.objs.aliens[idx as usize];
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs();
    if zdist >= 120 {
        return;
    }
    let xydist =
        (me.worldx as i32 - pl.worldx as i32).abs() + (me.worldy as i32 - pl.worldy as i32).abs();
    if xydist >= 60 {
        return;
    }
    // Pickup: grant the wireframe ship, set shieldup, chime, remove.
    // ROM GASTRATS.ASM:2615-2620 — pnumhits=0, curr_ship/select_ship scoped out;
    // modelled: psf2_wireship + shieldup=1 + $16.
    g.vars.set_sv_u8(sv::PNUMHITS, 0);
    g.vars.pshipflags2 |= PSF2_WIRESHIP; // s_or_var pshipflags2,#psf2_wireship
    g.vars.shieldup = 1; // s_set_var B,shieldup,#1
    g.hooks.play_se(0x16); // TRIGSE $16
    g.objs.aldead = 1; // s_jmp remove_Istrat
}

// ============================================================
// Registration (table lane hookup).
// ============================================================

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

/// Populate the ground-artillery `g_istrats` rows. Called from
/// `table::register_all` right after `bosses::register`.
pub fn register(world: &mut World) {
    world.istrats[IS_EXIT] = Some(wsid(world, exit_istrat));

    // Space Armada ship interiors and their medium-tunnel obstacle family.
    // All of these names have real ISTRATS.ASM rows; register the rows before
    // address-map construction so compact retail map objects execute them.
    world.istrats[IS_SHIP1A] = Some(wsid(world, crate::enemy_a::ship1a_istrat));
    world.istrats[IS_SHIP2] = Some(wsid(world, crate::enemy_a::ship2_istrat));
    world.istrats[IS_SHIP3] = Some(wsid(world, crate::enemy_a::ship3_istrat));
    world.istrats[IS_SHIP3A] = Some(wsid(world, crate::enemy_a::ship3a_istrat));
    world.istrats[IS_CORE1] = Some(wsid(world, crate::enemy_a::core1_istrat));
    world.istrats[IS_CORE0] = Some(wsid(world, crate::enemy_a::core0_istrat));
    world.istrats[IS_CRUISER2FIRE] = Some(wsid(world, cruiser2fire_istrat));
    world.istrats[IS_CRUISER2] = Some(wsid(world, cruiser2_istrat));
    world.istrats[IS_SDOOR1] = Some(wsid(world, sdoor1_istrat));
    world.istrats[IS_SDOOR2] = Some(wsid(world, sdoor2_istrat));
    world.istrats[IS_LENG0] = Some(wsid(world, leng0_istrat));
    world.istrats[IS_TOPRIGHT1] = Some(wsid(world, topright1_istrat));
    world.istrats[IS_TOPLEFT1] = Some(wsid(world, topleft1_istrat));
    world.istrats[IS_BOTRIGHT1] = Some(wsid(world, botright1_istrat));
    world.istrats[IS_BOTLEFT1] = Some(wsid(world, botleft1_istrat));
    world.istrats[IS_CRUISER1] = Some(wsid(world, cruiser1_istrat));
    world.istrats[IS_CRUISER1F] = Some(wsid(world, cruiser1f_istrat));
    world.istrats[IS_WARKER3] = Some(wsid(world, warker3_istrat));
    world.istrats[IS_TWALL0] = Some(wsid(world, twall0_istrat));
    world.istrats[IS_MONOLITH] = Some(wsid(world, crate::enemy_a::monolith_istrat));
    world.istrats[IS_EXITOPENSND2] = Some(wsid(world, crate::enemy_a::exitopensnd2_istrat));
    world.istrats[IS_OPENLR] = Some(wsid(world, openlr_istrat));
    world.istrats[IS_UPDOOR] = Some(wsid(world, updoor_istrat));

    world.istrats[IS_BAZOOKAL] = Some(wsid(world, bazookal_init));
    world.istrats[IS_BAZOOKAR] = Some(wsid(world, bazookar_init));
    world.istrats[IS_TANK2] = Some(wsid(world, tank2_istrat));
    world.istrats[IS_TANK1A] = Some(wsid(world, tank1a_istrat));
    world.istrats[IS_TANK0] = Some(wsid(world, tank0_istrat));
    world.istrats[IS_TANK1] = Some(wsid(world, tank1_istrat));
    world.istrats[IS_TANK3] = Some(wsid(world, tank3_istrat));
    world.istrats[IS_LEFTWALL] = Some(wsid(world, leftwall_istrat));
    world.istrats[IS_SAUCER] = Some(wsid(world, saucer_istrat));
    world.istrats[IS_WARP] = Some(wsid(world, warp_istrat));

    // Mobile ground-enemy family (all placed by ported maps -> reachable).
    world.istrats[IS_WALKING] = Some(wsid(world, walking_istrat));
    world.istrats[IS_WIREMAN] = Some(wsid(world, wireman_istrat));
    world.istrats[IS_WINGLAZERMAN] = Some(wsid(world, winglazerman_istrat));
    world.istrats[IS_UPERM] = Some(wsid(world, uperm_istrat));
    world.istrats[IS_ROCKHARD] = Some(wsid(world, rockhard_istrat));

    // Space / air-hazard family (all placed by ported maps -> reachable).
    world.istrats[IS_METEO0] = Some(wsid(world, meteo0_init));
    world.istrats[IS_BIG_METEOR] = Some(wsid(world, big_meteor_init));
    world.istrats[IS_BREAK_METEOR] = Some(wsid(world, break_meteor_init));
    world.istrats[IS_BREAK_METEORT] = Some(wsid(world, break_meteort_init));
    let mine0 = wsid(world, mine0_istrat);
    world.register_direct_strategy(sf_map::consts::DirectStrategy::Mine0, mine0);
    world.istrats[IS_TORPEDO] = Some(wsid(world, torpedo_init));

    // Base / colony structure set-pieces (all placed by ported maps -> reachable).
    world.istrats[IS_BASE0] = Some(wsid(world, base0_init));
    world.istrats[IS_MASSIVEBASE] = Some(wsid(world, massivebase_init));
    world.istrats[IS_COLONY0] = Some(wsid(world, colony0_init));
    world.istrats[IS_COLONY1] = Some(wsid(world, colony1_init));
    world.istrats[IS_COLONY2] = Some(wsid(world, colony2_init));
    world.istrats[IS_COLONYEXIT] = Some(wsid(world, colonyexit_strat));

    // Environmental hazards — all placed by ported route3 (+ route1 windmill)
    // maps -> reachable.
    world.istrats[IS_TRACKCORNER] = Some(wsid(world, trackcorner_init));
    world.istrats[IS_WINDMILL] = Some(wsid(world, windmill_init));
    world.istrats[IS_FLYPILLARS] = Some(wsid(world, flypillar_istrat));
    world.istrats[IS_VOLCANO] = Some(wsid(world, volcano_init));
    world.istrats[IS_FIREPILLAR] = Some(wsid(world, firepillar_init));
    world.istrats[IS_BASE_1] = Some(wsid(world, base_1_istrat));

    // Firing-enemy family (all placed by ported maps -> reachable). synth is a
    // false gap (mothers.rs IS_SYNTH is the 0x020000 synthetic-address base,
    // not a placed enemy) -> intentionally not registered.
    world.istrats[IS_MISSTANK] = Some(wsid(world, misstank_init));
    world.istrats[IS_MISSPOD] = Some(wsid(world, misspod_init));
    world.istrats[IS_SZACO0] = Some(wsid(world, szaco0_init));
    world.istrats[IS_SZACO5] = Some(wsid(world, szaco5_init));
    world.istrats[IS_HOUDAI5F] = Some(wsid(world, houdai5f_init));

    // Door / wall / tree / woods scenery family, all at exact ISTRATS rows.
    world.istrats[IS_WOODS] = Some(wsid(world, woods_init));
    world.istrats[IS_KDOOR] = Some(wsid(world, kdoor_init));
    world.istrats[IS_KICHI2] = Some(wsid(world, strat_nocoll_init));
    world.istrats[IS_KDOOR2] = Some(wsid(world, kdoor2_init));
    world.istrats[IS_WALLLEFTRIGHT] = Some(wsid(world, wallleftright_init));
    world.istrats[IS_WALLL] = Some(wsid(world, walll_init));
    world.istrats[IS_WALLR] = Some(wsid(world, wallr_init));
    world.istrats[IS_TREE1] = Some(wsid(world, tree1_init));
    world.istrats[IS_TREE2] = Some(wsid(world, tree2_init));

    // Final niche enemies/objects (sf-map IS_FOO == these rows).
    world.istrats[IS_SHOU0] = Some(wsid(world, shou0_init));
    world.istrats[IS_SHOU0A] = Some(wsid(world, shou0a_init));
    world.istrats[IS_IRIS] = Some(wsid(world, iris_init));
    world.istrats[IS_TRUCK] = Some(wsid(world, truck_init));
    world.istrats[IS_ITEM6] = Some(wsid(world, item6_init));
}
