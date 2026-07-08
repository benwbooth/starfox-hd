//! Ground-artillery enemy family — RIIR port of Krister's tank strategies
//! (`reference/ultrastarfox/SF/STRAT/KSTRATS.ASM` tank1/tank1a/tank3 +
//! GA2STRAT.ASM tank2 / bazooka). ASM is the sole ground truth: none of
//! these have a C-oracle counterpart (`src/strat/strat_*.c` never ported the
//! tanks), so every cite below is to the 65816 source.
//!
//! ISTRATS.ASM def_Istrat rows (macro-counted, matching sf-map placement):
//!   - `bazookaL`  = 158  (ISTRATS.ASM:587)  — level1_5 / route2 L5 / route3 L6
//!   - `bazookaR`  = 159  (ISTRATS.ASM:588)  — same maps
//!   - `tank2`     = 162  (ISTRATS.ASM:591)  — level1_4
//!   - `tank1a`    = 183  (ISTRATS.ASM:614)  — level1_4
//!   - `tank3`     = 186  (ISTRATS.ASM:617)  — level2_3
//! `tank0` (184) and `tank1` (185) have def_Istrat rows but are placed by NO
//! ported map (grep sf-map: only the pathspecial `SH_TANK_1` scenery uses the
//! tank_1 SHAPE, never the IS_TANK1 strategy). They are dead content and are
//! deliberately NOT registered — their shared helpers `tank1lr`/`tank1fire`
//! ARE ported because live tank1a/tank3 call them (KSTRATS.ASM:496/515).
//!
//! State machines (see per-fn cites):
//!   tank1a  : wait(<5000z) -> chase(turn to 0) -> forward(fire, z+=17) /
//!             back(z-=7, terminal) — KSTRATS.ASM:418-458
//!   tank3   : wait(<1800z) -> forward -> back -> forwardb -> backb(idle);
//!             fires on the tank1fire gate each active state — KSTRATS.ASM:566-608
//!   tank2   : body backs up / turns / advances releasing 4 zaco_7 turret
//!             drones on an HP-less countdown; drones rise then chase+fire
//!             the player — GA2STRAT.ASM:1266-1408
//!   bazooka : rises from the planet, aims, lobs a 3-shot RELSLOWELASER
//!             burst, then flees up-and-away — GA2STRAT.ASM:1001-1082

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE1, ACF_COLLTYPE4, ACF_FIRSTFRAME, ACF_WEAPON, ASF_COLLDISABLE,
    ASF_INVISIBLE, ASF_NOHITAFFECT, ASF_SHADOW, ATLASER, ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::world::World;

use crate::enemy_a::{
    achase_angle, add_player_z, boss_attach_child_to_mother, boss_find_child_obj,
    boss_get_mother_obj, copy_pos, ea_random, homingflat_strat, player, sid, speed_to,
    strat_aim_3d, strat_aim_yaw, strat_explode, strat_fire_relslowlaser,
    strat_fire_relslowlaserhome, strat_hit_flash, strat_move3d, strat_pitch_toward,
    COLLTYPE_ENEMY1, COLLTYPE_ENEMYWEAP, COLLTYPE_ZENEMY, DEG180, DEG45, DEG90,
};
use crate::common::{
    angle_xz, apply_velocity, dist_xz, gen_vecs_2d, gen_vecs_3d, make_obj, projectile_strat_ids,
    spawn_projectile,
};

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
const IS_BAZOOKAL: usize = 158;
const IS_BAZOOKAR: usize = 159;
const IS_TANK2: usize = 162;
const IS_TANK1A: usize = 183;
const IS_TANK3: usize = 186;

/// zaco_7 turret drone shape (sf-map route2 rc.rs `SH_ZACO_7`).
const SH_ZACO_7: u16 = 129;

// ------------------------------------------------------------
// Mobile ground-enemy family (wireman / winglazerman / walking / uperm /
// rockhard). ISTRATS.ASM def_Istrat rows == sf-map placement indices
// (grep sf-map: all five are placed by ported maps — all reachable):
//   - walking       = 78  (ISTRATS.ASM:500)  — level1_4 / route3
//   - wireman       = 88  (ISTRATS.ASM:511)  — route2 rc
//   - winglazerman  = 91  (ISTRATS.ASM:514)  — level1_3/1_5 / route2 / route3
//   - uperM         = 160 (ISTRATS.ASM:589)  — level1_5 / route2 / route3
//   - rockhard      = 192 (ISTRATS.ASM:623)  — route2 / route3
// ASM is the sole ground truth (no C-oracle for these): GASTRATS.ASM
// (wireman:2446-2511, winglazerman:2811-2903), DSTRATS.ASM (walking:860-964),
// GA2STRAT.ASM (uperm:1112-1141), GSTRATS.ASM (rockhard:663-669).
const IS_WALKING: usize = 78;
const IS_WIREMAN: usize = 88;
const IS_WINGLAZERMAN: usize = 91;
const IS_UPERM: usize = 160;
const IS_ROCKHARD: usize = 192;

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

/// `s_jmp_notdelay N` (STRATMAC.INC:6456): fires when `gameframe & ((1<<N)-1) == 0`.
fn notdelay(g: &Game, bits: u16) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

/// `s_add_Roffs2pos B,y,x,x,offx,offy,offz,1,1,1,s,s,s` (STRATMAC.INC:4098):
/// rotate the (pre-shifted) local offset by the base's FULL rotation —
/// rotz first, then rotx, then roty — and add it to the base world pos.
/// Byte-identical form to bosses.rs `b2_full_offset_pos` (replicated here so
/// this lane stays independent of bosses.rs's private surface).
fn rotate_full_offset(base: &Alien, offx: i16, offy: i16, offz: i16) -> (i16, i16, i16) {
    let sin = |a: u8| (a as f32 * (2.0f32 * std::f32::consts::PI / 256.0f32)).sin();
    let cos = |a: u8| (a as f32 * (2.0f32 * std::f32::consts::PI / 256.0f32)).cos();
    // rotz stage.
    let s = sin(base.rotz);
    let c = cos(base.rotz);
    let x1 = ((offx as f32 * c) - (offy as f32 * s)).round();
    let y1 = ((offx as f32 * s) + (offy as f32 * c)).round();
    // rotx stage.
    let s = sin(base.rotx);
    let c = cos(base.rotx);
    let y2 = ((y1 * c) - (offz as f32 * s)).round();
    let z2 = ((y1 * s) + (offz as f32 * c)).round();
    // roty stage (ROM negates the yaw).
    let s = sin(base.roty);
    let c = cos(base.roty);
    let rx = ((x1 * c) + (z2 * s)).round() as i16;
    let rz = ((z2 * c) - (x1 * s)).round() as i16;
    (rx, y2 as i16, rz)
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
}

/// `tank1fire` (KSTRATS.ASM:515-534): 3-shot cadence off a `tank1firerate`(50)
/// countdown in `al_sbyte2` — fires at counts 20, 10, and 0 (0 also reloads to
/// 50). Gated: no fire when `|dz| < 500` OR `|dx| >= 300`.
fn tank1fire(g: &mut Game, idx: u16) {
    // s_dec_alvar B,x,al_sbyte2
    let sb2 = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte2 = sb2;
    // cmp 20 -> fire; cmp 10 -> fire; cmp 0 -> fireset(reload)+fire; else rts.
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
// tank1a (IS 183) — KSTRATS.ASM:418-458.
// ============================================================

/// `tank1a_istrat` (KSTRATS.ASM:418-427): the persistent init/wait strategy.
/// Every tick sets base vars + faces deg270; when the player closes to within
/// 5000 z it hands off to `tank1a_strat` this same tick.
fn tank1a_init(g: &mut Game, idx: u16) {
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
        tank1a2_setup(g, idx);
    }
    // else s_end_strat (stay in init, re-run next tick).
}

/// `tank1a2_istrat` (KSTRATS.ASM:429-434): one-shot handoff — wire tick/hit/
/// explode strats + fire timers, then fall into `tank1a_strat` the same tick.
fn tank1a2_setup(g: &mut Game, idx: u16) {
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
fn tank1a_strat(g: &mut Game, idx: u16) {
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
// tank3 (IS 186) — KSTRATS.ASM:566-608.
// ============================================================

/// `tank3_istrat` (KSTRATS.ASM:566-577): init/wait — set strats immediately
/// (self as tick until close), face 0; when the player closes to 1800 z enter
/// the forward attack.
fn tank3_init(g: &mut Game, idx: u16) {
    let selfid = sid(g, tank3_init);
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

/// `s_gen_vecs x,al_roty,al_vel` with al_vel read SIGNED (tank2 body reverses).
/// Matches common `strat_gen_vecs_2d` bit-math but with an i8 velocity.
fn gen_vecs_2d_signed(al: &mut Alien) {
    use crate::snes_trig::{mulslog, COSTAB, SINTAB};
    let angle = al.roty as usize;
    let vel = (al.vel as i8) as i32;
    al.vx = mulslog(vel, SINTAB[angle] as i32) as i16;
    al.vy = 0;
    al.vz = mulslog(vel, COSTAB[angle] as i32) as i16;
}

/// `tank2_Istrat` (GA2STRAT.ASM:1266-1289): wire strats/data, face deg180,
/// speed 30, and spawn the four zaco_7 turret drones at their local offsets
/// with per-drone rise thresholds (`al_sword2`).
fn tank2_init(g: &mut Game, idx: u16) {
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
    tank2zaco_init(g, child);
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
fn tank2_strat(g: &mut Game, idx: u16) {
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
fn tank2zaco_init(g: &mut Game, idx: u16) {
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
fn tank2zaco_strat(g: &mut Game, idx: u16) {
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
    let dpitch = ((ea_random(g) as u8 & 7) as i8).wrapping_sub(3);
    let dyaw = ((ea_random(g) as u8 & 7) as i8).wrapping_sub(3);
    let pitch = base_pitch.wrapping_add(dpitch as u8);
    let yaw = base_yaw.wrapping_add(dyaw as u8);
    strat_fire_relslowlaser(g, idx, pitch, yaw);
}

// ============================================================
// bazooka L/R (IS 158/159) — GA2STRAT.ASM:1001-1082.
// ============================================================

/// sflags2 sflag1 bit (STRATEQU.INC:914): the L/R turn-direction latch.
const ASF2_SFLAG1: u8 = 0x10;

/// `bazookaL_Istrat`/`bazookaR_Istrat` (GA2STRAT.ASM:1001-1018): L sets sflag1
/// (turn left in the fire state); both share `bazooka_Icont`.
fn bazookal_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // s_set_alsflag x,sflag1
    bazooka_icont(g, idx);
}
fn bazookar_init(g: &mut Game, idx: u16) {
    bazooka_icont(g, idx);
}

/// `bazooka_Icont` (GA2STRAT.ASM:1008-1018): common init — rise up from the
/// planet with vy=-15, pitched straight up, facing deg180, speed 80.
fn bazooka_icont(g: &mut Game, idx: u16) {
    let tick = sid(g, bazooka_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, bazexp_strat);
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
    let dpitch = ((ea_random(g) as u8 & 3) as i8).wrapping_sub(1);
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
}

/// `bazexp_Istrat` (GA2STRAT.ASM:1055-1063): on death drop a falling debris
/// object (the barrel) then run the standard escapee explosion.
fn bazexp_strat(g: &mut Game, idx: u16) {
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

/// `bazfall_strat` (GA2STRAT.ASM:1071-1082): tumble + fall under gravity for
/// 30 frames then remove.
fn bazfall_strat(g: &mut Game, idx: u16) {
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
    (ea_random(g) & 0xff) < 127
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
fn wireman_cont2(g: &mut Game, idx: u16) {
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// `wireman2_init` (GASTRATS.ASM:2467-2474): pick an evasive branch. Two coin
/// flips: 50% X (pitch), then 25%/25% between the YL and YR rolls. NOTE the ROM
/// quirk at :2472-2474 — the middle branch sets `stratptr = YR` but jumps to the
/// `YL` code THIS tick, so the first evasive tick always runs the YL motion and
/// only the *next* tick runs YR. Replicated faithfully.
fn wireman2_init(g: &mut Game, idx: u16) {
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
fn wireman2yl_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4); // s_add_alvar al_roty,#4
        al.rotz = al.rotz.wrapping_add(4); // s_add_alvar al_rotz,#4
    }
    wireman2_countdown(g, idx);
}

/// `wireman2YR_strat` (GASTRATS.ASM:2482-2487): yaw+roll right while dodging.
fn wireman2yr_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_sub(4);
        al.rotz = al.rotz.wrapping_sub(4);
    }
    wireman2_countdown(g, idx);
}

/// `wireman2X_strat` (GASTRATS.ASM:2488-2492): pitch down while dodging.
fn wireman2x_strat(g: &mut Game, idx: u16) {
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
fn wiremanup_init(g: &mut Game, idx: u16) {
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
fn wiremanup_strat(g: &mut Game, idx: u16) {
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 == 0 {
        wireman_init(g, idx);
    } else {
        g.objs.aliens[idx as usize].sbyte1 = sb1 - 1;
        wireman_cont2(g, idx);
    }
}

/// `wiremandie_Istrat` (GASTRATS.ASM:2505-2511): drops an `item_6` powerup then
/// explodes. The item-pickup strat (`item6_Istrat`) is unported (out of this
/// lane's file scope), so the drop is scoped out; the explosion is faithful.
fn wiremandie_strat(g: &mut Game, idx: u16) {
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
fn winglazerman2_init(g: &mut Game, idx: u16) {
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
fn winglazerman2_strat(g: &mut Game, idx: u16) {
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
fn winglazerman3_init(g: &mut Game, idx: u16) {
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
fn winglazerman3_strat(g: &mut Game, idx: u16) {
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
}

/// `winglazermango_init` (GASTRATS.ASM:2832-2834): flee state — 100-frame
/// lifetime.
fn winglazermango_init(g: &mut Game, idx: u16) {
    let tick = sid(g, winglazermango_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.count = 100; // s_set_lifecnt x,#100
    winglazermango_strat(g, idx);
}

/// `winglazermango_strat` (GASTRATS.ASM:2835-2841): accelerate to 50, level
/// out (roty->0, rotx->-5), and expire.
fn winglazermango_strat(g: &mut Game, idx: u16) {
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
/// upgrade) or `item_3` (beam ball) depending on the ship's wing/beam flags,
/// then explodes. The item-pickup strats are unported (out of file scope), so
/// the powerup drop is scoped out; the explosion is faithful.
fn winglazermandie_strat(g: &mut Game, idx: u16) {
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
    gen_vecs_2d(&mut g.objs.aliens[idx as usize]); // s_gen_vecs x,al_roty,al_vel
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
fn walking2_strat(g: &mut Game, idx: u16) {
    gen_vecs_2d(&mut g.objs.aliens[idx as usize]); // s_gen_vecs x,al_roty,al_vel
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
fn walking_hit(g: &mut Game, idx: u16) {
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
    full_offset_pos(g, idx, &me, 50, 0, 0); // s_add_Roffs2pos #25,#0,#0 (<<1)
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
    full_offset_pos(g, idx, &me, -50, 0, 0); // s_add_Roffs2pos #-25,#0,#0 (<<1)
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
fn uperm_istrat(g: &mut Game, idx: u16) {
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
fn uperm_strat(g: &mut Game, idx: u16) {
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
//   - mine0          = 246 (ISTRATS.ASM:684-ish) — route1 level1_5
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
const IS_TORPEDO: usize = 80;
const IS_METEO0: usize = 195;
const IS_BIG_METEOR: usize = 234;
const IS_BREAK_METEOR: usize = 235;
const IS_BREAK_METEORT: usize = 238;
const IS_MINE0: usize = 246;

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
const SH_TADPOLE: u16 = 228; // break_meteorT death spawn (SH_TADPOLE)

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
    let coll = sid(g, strat_hit_flash);
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
    let vel = (ea_random(g) as u8) & 7;
    let sbyte1 = (ea_random(g) as u8) & 3;
    let roty = ea_random(g) as u8;
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
    let sb1 = ((ea_random(g) as u8) & 15).wrapping_sub(8);
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
/// tadpole_istrat`). `tadpole_istrat` is unported (out of this lane's scope),
/// so the tadpole is placed inert (no AI). Then the standard explosion runs.
fn break_meteort_exp(g: &mut Game, idx: u16) {
    // P_RANDOMGOTO branches (skips spawn) when random<127; spawn on the else.
    if !jmp_random50(g) {
        if let Some(t) = make_obj(g, SH_TADPOLE) {
            copy_pos(g, t, idx);
            // tadpole_istrat unported: leave inert + non-colliding.
            g.objs.aliens[t as usize].sflags |= ASF_COLLDISABLE;
        }
    }
    strat_explode(g, idx);
}

// ------------------------------------------------------------
// mine0 (IS 246) — DSTRATS.ASM:1572-1582.
// ------------------------------------------------------------

/// `mine0_istrat` (DSTRATS.ASM:1572-1577): a static destructible mine — hp2/
/// ap10, enemy1 collide, a random full-byte rotz orientation, standard
/// explosion. Installs the no-op `mine0_strat` tick. (NOTE: the ROM mine0 uses
/// plain `explode_istrat`, NOT `mine2exp` — the proximity/beam-burst death
/// belongs to the unrelated `mine2` strat, GA2STRAT.ASM:2560.)
fn mine0_init(g: &mut Game, idx: u16) {
    let tick = sid(g, mine0_strat);
    let coll = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    // s_set_alvar2rnd x,al_rotz (full byte -> any orientation).
    let rotz = ea_random(g) as u8;
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

/// `torpedo_strat` (GASTRATS.ASM:2015-2030): while > 800 z, yaw-home the player
/// (rate 3) then move; inside 800 z surface via `torpedoa_init`.
/// (`makeSsplash` is cosmetic — scoped out.)
fn torpedo_strat(g: &mut Game, idx: u16) {
    // s_jmp_Zdistless x,y,#800,torpedoa_init
    if zdist_less(g, idx, 800) {
        torpedoa_init(g, idx);
        return;
    }
    // s_obj2obj_angle x,y,al_roty,3
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
/// the visible f_fish, become collidable, then fall into `torpedoa_strat`.
/// (`makesplash`/`enemyupsea` are cosmetic — scoped out.)
fn torpedoa_init(g: &mut Game, idx: u16) {
    let tick = sid(g, torpedoa_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick); // s_set_strat x,torpedoa_strat
    al.rotx = (-(DEG45 as i8)) as u8; // s_set_alvar B,x,al_rotx,#-deg45
    al.shape = SH_F_FISH; // s_set_alvar W,x,al_shape,#f_fish
    al.sflags &= !ASF_COLLDISABLE; // s_clr_alsflag x,colldisable
    torpedoa_strat(g, idx);
}

/// `torpedoa_strat` (GASTRATS.ASM:2039-2042): after surfacing, level the pitch
/// back to 0 (rate 2) then coast (torpedo_cont — note it no longer yaw-homes).
fn torpedoa_strat(g: &mut Game, idx: u16) {
    // s_achase_alvar B,x,al_rotx,#0,2
    let mut rotx = g.objs.aliens[idx as usize].rotx;
    achase_angle(&mut rotx, 0, 2);
    g.objs.aliens[idx as usize].rotx = rotx;
    // s_brl torpedo_cont
    torpedo_cont(g, idx);
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
    world.istrats[IS_BAZOOKAL] = Some(wsid(world, bazookal_init));
    world.istrats[IS_BAZOOKAR] = Some(wsid(world, bazookar_init));
    world.istrats[IS_TANK2] = Some(wsid(world, tank2_init));
    world.istrats[IS_TANK1A] = Some(wsid(world, tank1a_init));
    world.istrats[IS_TANK3] = Some(wsid(world, tank3_init));
    // tank0 (184) / tank1 (185): placed by no ported map — intentionally
    // unregistered (dead content, see module doc).

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
    world.istrats[IS_MINE0] = Some(wsid(world, mine0_init));
    world.istrats[IS_TORPEDO] = Some(wsid(world, torpedo_init));
}
