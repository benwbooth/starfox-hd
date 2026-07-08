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
    ASF_INVISIBLE, ATLASER, ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::world::World;

use crate::enemy_a::{
    achase_angle, add_player_z, boss_attach_child_to_mother, boss_find_child_obj,
    boss_get_mother_obj, copy_pos, ea_random, homingflat_strat, player, sid, strat_aim_3d,
    strat_explode, strat_fire_relslowlaser, strat_hit_flash, strat_move3d, strat_pitch_toward,
    COLLTYPE_ENEMY1, COLLTYPE_ENEMYWEAP, DEG180, DEG45, DEG90,
};
use crate::common::{angle_xz, apply_velocity, gen_vecs_3d, make_obj, projectile_strat_ids};

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
}
