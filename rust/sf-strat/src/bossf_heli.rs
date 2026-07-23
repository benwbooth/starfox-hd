//! Great Commander / Transformer airship and child parts (DSTRATS.ASM).
//!
//! Parent is `airship_istrat` (ledger True); these four `*_ISTRAT` leaves are
//! the remaining `enemy_boss_strat` False rows. Children store mother-relative
//! offsets in `childx/y/z` + `childrotx/y/z` (bytes; world placement is
//! `<< childscale(3)` via the mother's rotpos pass). Mode tables use
//! `al_stratstate`.

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE2, ASF4_SFLAG8, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::{GF_BOSSDEAD, HARD_AP, HARD_HP};
use sf_game::world::World;

use crate::common::{
    kill_obj, sf_random, strat_angle_xz, strat_apply_velocity, strat_gen_vecs_3d, strat_make_obj,
    strat_trig_se,
};
use crate::enemy_a::{
    achase_angle, add_player_z, boss_attach_child_to_mother, boss_find_child_obj,
    boss_get_mother_obj, boss_obj_index_or_null, divorce_family, fire_boss_hmissile1,
    fire_ironball, fire_ironball2, fire_ironball3, firenormringlaser, ironballmissile_istrat,
    player, sid, strat_boss_explode_init, strat_explode, strat_hit_flash, strat_pitch_toward,
    ASF2_SFLAG1, ASF2_SFLAG2, ASF2_SFLAG3, ASF3_SFLAG5, ASF3_SFLAG6, DEG11, DEG180, DEG22, DEG90,
};
use crate::enemy_b::eb_compat::{add_bosshp, set_bossmaxhp};
use crate::snes_trig::{rotate_8xz, SINTAB};

/// ISTRATS.ASM `def_istrat airship,boss_f_4`.
pub const IS_AIRSHIP: usize = 125;
/// Synthetic direct address emitted by TRANSFOR.ASM.
pub const STRAT_ADDR_AIRSHIP: u32 = 0x05002B;

/// Runtime mesh ids.  `boss_f_3`/`boss_f_4` are ordinary def_shape rows;
/// the other SHAPES4-only components use shape_compiler extended slots.
pub const SH_AIRSHIP_FEET: u16 = 81;
pub const SH_AIRSHIP: u16 = 94;
pub const SH_AIRSHIP_HEAD: u16 = 305;
pub const SH_AIRSHIP_BODY: u16 = 306;
pub const SH_AIRSHIP_BODY_TRANSFORMED: u16 = 307;
pub const SH_AIRSHIP_ARM_RIGHT: u16 = 308;
pub const SH_AIRSHIP_ARM_LEFT: u16 = 309;

const BOSSF_BODY: u8 = 1;
const BOSSF_HEAD: u8 = 2;
const BOSSF_FEET: u8 = 3;
const BOSSF_ARM1: u8 = 4;
const BOSSF_ARM2: u8 = 5;

// airship_istrat parent modes, in source table order (DSTRATS:7014-7071).
const AIR_FLY_UP_TO_FRONT: u8 = 0;
const AIR_PAUSE: u8 = 1;
const AIR_MAJOR_CHANGE: u8 = 2;
const AIR_DROP_TO_GROUND: u8 = 3;
const AIR_SWIVEL_180: u8 = 4;
const AIR_ROTATE_ARMS: u8 = 5;
const AIR_JOIN_SOUND: u8 = 6;
const AIR_REJOIN: u8 = 7;
const AIR_MAJOR_CHANGE2: u8 = 8;
const AIR_MOVE_BACK: u8 = 9;
const AIR_START_IRONBALLS: u8 = 10;
const AIR_OPEN_HATCH: u8 = 11;
const AIR_BODY_ROT: u8 = 12;
const AIR_STOP_IT: u8 = 13;
const AIR_LOOP_START: u8 = 14;
const AIR_HURT_LOOP: u8 = 32;
/// First mode after `bossf_heli` (the old child-only port incorrectly used 36).
pub const AIRSHIP_MODE_BOSSF_HELI: u8 = 48;
const AIR_BACK_FORTH: u8 = 49;

fn world_sid(world: &mut World, f: StrategyFn) -> StratId {
    if let Some(pos) = world
        .strat_registry
        .iter()
        .position(|&r| r as usize == f as usize)
    {
        return StratId(pos as u16);
    }
    world.register_strategy(f)
}

/// Register both the ISTRATS row and the direct map symbol used by TRANSFOR.
pub fn register(world: &mut World) {
    let id = world_sid(world, airship_istrat);
    world.istrats[IS_AIRSHIP] = Some(id);
    world.register_strategy_address(STRAT_ADDR_AIRSHIP, id);
}

// ============================================================
// airship_istrat parent — DSTRATS.ASM:6999-7510
// ============================================================

fn airship_child(g: &mut Game, mother: u16, child_num: u8) -> Option<u16> {
    boss_find_child_obj(g, mother, child_num)
}

#[allow(clippy::too_many_arguments)]
fn airship_spawn_child(
    g: &mut Game,
    mother: u16,
    child_num: u8,
    shape: u16,
    x: i8,
    y: i8,
    z: i8,
    rotx: u8,
    roty: u8,
    rotz: u8,
    init: StrategyFn,
) -> Option<u16> {
    let child = g.objs.alloc()?;
    crate::common::strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[child as usize];
        al.shape = shape;
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
        al.childx = x as u8;
        al.childy = y as u8;
        al.childz = z as u8;
        al.childrotx = rotx;
        al.childroty = roty;
        al.childrotz = rotz;
        al.collflags |= ACF_COLLTYPE2;
    }
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    init(g, child);
    Some(child)
}

fn airship_generate(g: &mut Game, idx: u16) {
    let _ = airship_spawn_child(
        g,
        idx,
        BOSSF_BODY,
        SH_AIRSHIP_BODY,
        0,
        0,
        40,
        0,
        0,
        0,
        bossfbody_istrat,
    );
    let _ = airship_spawn_child(
        g,
        idx,
        BOSSF_HEAD,
        SH_AIRSHIP_HEAD,
        0,
        -22,
        10,
        0,
        0,
        0,
        bossfhead_istrat,
    );
    let _ = airship_spawn_child(
        g,
        idx,
        BOSSF_FEET,
        SH_AIRSHIP_FEET,
        0,
        0,
        0,
        0,
        DEG180,
        0,
        bossffeet_istrat,
    );
    airship_position(g, idx);
}

fn airship_child_rotpos(g: &mut Game, mother: u16, child: u16) {
    let c = g.objs.aliens[child as usize];
    let reference = if c.childrotobj != 0 {
        let r = c.childrotobj - 1;
        if (r as usize) < g.objs.aliens.len() && g.objs.aliens[r as usize].active {
            r
        } else {
            mother
        }
    } else {
        mother
    };
    let base = g.objs.aliens[reference as usize];
    let (rx, ry, rz) = crate::snes_trig::strat_roffs_full_scaled(
        base.rotz,
        base.rotx,
        base.roty,
        c.childx as i8,
        c.childy as i8,
        c.childz as i8,
        3,
    );
    let al = &mut g.objs.aliens[child as usize];
    al.rotx = base.rotx.wrapping_add(c.childrotx);
    al.roty = base.roty.wrapping_add(c.childroty);
    al.rotz = base.rotz.wrapping_add(c.childrotz);
    al.worldx = base.worldx.wrapping_add(rx);
    al.worldy = base.worldy.wrapping_add(ry);
    al.worldz = base.worldz.wrapping_add(rz);
}

fn airship_position(g: &mut Game, idx: u16) {
    for child_num in BOSSF_BODY..=BOSSF_ARM2 {
        if let Some(child) = airship_child(g, idx, child_num) {
            airship_child_rotpos(g, idx, child);
        }
    }
}

fn airship_dz_less(g: &Game, idx: u16, dist: i16) -> bool {
    player(g)
        .map(|p| (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs() < dist as i32)
        .unwrap_or(false)
}

fn airship_move(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
}

fn airship_move2(g: &mut Game, idx: u16) {
    airship_position(g, idx);
    airship_move(g, idx);
}

fn airship_check_rejoin(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        return;
    }
    if let Some(body) = airship_child(g, idx, BOSSF_BODY) {
        if (g.objs.aliens[body as usize].childy as i8) >= -40 {
            g.objs.aliens[body as usize].childy = (-40i8) as u8;
        }
    }
    if let Some(head) = airship_child(g, idx, BOSSF_HEAD) {
        if (g.objs.aliens[head as usize].childy as i8) >= -67 {
            if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 != 0 {
                g.objs.aliens[idx as usize].sflags4 &= !ASF4_SFLAG8;
                strat_trig_se(g, 0x8e);
            }
            g.objs.aliens[head as usize].childy = (-67i8) as u8;
        }
    }
}

fn airship_move3(g: &mut Game, idx: u16) {
    // Signed form of the source's two LSRs/third LSR staging.  Only the low
    // byte is stored in childy.
    let base_y = ((-g.objs.aliens[idx as usize].worldy as i32 - 200) / 8) as i8;
    let phase = g.objs.aliens[idx as usize].sbyte2;
    if let Some(body) = airship_child(g, idx, BOSSF_BODY) {
        let pull = phase.saturating_sub(25) as i8;
        g.objs.aliens[body as usize].childy = base_y.wrapping_sub(pull) as u8;
    }
    if let Some(head) = airship_child(g, idx, BOSSF_HEAD) {
        let pull = phase.min(35);
        let pull = pull.wrapping_add(pull >> 1) as i8;
        g.objs.aliens[head as usize].childy = base_y.wrapping_sub(22).wrapping_sub(pull) as u8;
    }
    airship_check_rejoin(g, idx);
    airship_move2(g, idx);
}

fn airship_change_z(g: &mut Game, idx: u16) {
    for child_num in [BOSSF_HEAD, BOSSF_BODY] {
        if let Some(child) = airship_child(g, idx, child_num) {
            let _ = fchase_i8_byte(&mut g.objs.aliens[child as usize].childz, -10, 2);
        }
    }
}

fn airship_next(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
    airship_strat(g, idx);
}

fn airship_zero_next(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte2 = 0;
    airship_next(g, idx);
}

/// ROM `airship_istrat` — initialize the map-spawned flying shell and enter
/// the transformation mode table in the same tick.
pub fn airship_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, airship_strat);
    let hit = sid(g, airship_hit);
    let exp = sid(g, airship_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_SHADOW | ASF_NOHITAFFECT | ASF_COLLDISABLE;
        al.collflags |= ACF_COLLTYPE2;
        al.stratstate = AIR_FLY_UP_TO_FRONT;
    }
    g.vars.shared.power_build = 0;
    g.vars.gameflags &= !GF_BOSSDEAD;
    strat_trig_se(g, 0x5b);
    airship_strat(g, idx);
}

/// ROM parent mode table.  Immediate entries tail-call the next mode exactly
/// as `s_jmp .strat`; timed entries return through move/move2/move3.
pub fn airship_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        AIR_FLY_UP_TO_FRONT => {
            if g.objs.aliens[idx as usize].worldy != -200 {
                g.objs.aliens[idx as usize].worldy =
                    g.objs.aliens[idx as usize].worldy.wrapping_add(5);
            }
            if airship_dz_less(g, idx, 2000) {
                let al = &mut g.objs.aliens[idx as usize];
                al.worldz = al.worldz.wrapping_add(20);
                al.worldx = al.worldx.wrapping_add(1);
                airship_move(g, idx);
            } else if g.objs.aliens[idx as usize].roty == DEG180 {
                g.objs.aliens[idx as usize].rotz = 0;
                airship_next(g, idx);
            } else {
                let roll_half = (g.objs.aliens[idx as usize].rotz as i8 / 2) as u8;
                // `adiv2 ; sec ; adc al_roty`: SEC contributes the +1 carry.
                // Omitting it makes the all-zero map spawn a fixed point and
                // strands the boss in mode 0 forever.
                let yaw = g.objs.aliens[idx as usize]
                    .roty
                    .wrapping_add(roll_half)
                    .wrapping_add(1);
                g.objs.aliens[idx as usize].roty = yaw;
                if yaw < DEG90 {
                    g.objs.aliens[idx as usize].rotz =
                        g.objs.aliens[idx as usize].rotz.wrapping_add(1);
                } else if g.objs.aliens[idx as usize].rotz >= 2 {
                    g.objs.aliens[idx as usize].rotz -= 1;
                }
                airship_move(g, idx);
            }
        }
        AIR_PAUSE => {
            let n = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
            g.objs.aliens[idx as usize].sbyte2 = n;
            if n == 20 {
                g.objs.aliens[idx as usize].sbyte2 = 0;
                airship_next(g, idx);
            } else {
                airship_move(g, idx);
            }
        }
        AIR_MAJOR_CHANGE => {
            strat_trig_se(g, 0x8e);
            g.objs.aliens[idx as usize].shape = 0;
            airship_generate(g, idx);
            airship_next(g, idx);
        }
        AIR_DROP_TO_GROUND => {
            if g.objs.aliens[idx as usize].worldy != -120 {
                g.objs.aliens[idx as usize].worldy =
                    g.objs.aliens[idx as usize].worldy.wrapping_add(5);
            } else if g.objs.aliens[idx as usize].sbyte2 == 50 {
                airship_next(g, idx);
                return;
            } else {
                g.objs.aliens[idx as usize].sbyte2 =
                    g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
            }
            airship_move3(g, idx);
        }
        AIR_SWIVEL_180 => {
            let Some(feet) = airship_child(g, idx, BOSSF_FEET) else {
                airship_next(g, idx);
                return;
            };
            if g.objs.aliens[feet as usize].childroty == 0 {
                airship_next(g, idx);
                return;
            }
            g.objs.aliens[feet as usize].childroty =
                g.objs.aliens[feet as usize].childroty.wrapping_add(4);
            if g.objs.aliens[idx as usize].sbyte2 != 66 {
                g.objs.aliens[idx as usize].sbyte2 =
                    g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
            }
            airship_change_z(g, idx);
            airship_move3(g, idx);
        }
        AIR_ROTATE_ARMS => {
            g.objs.aliens[idx as usize].sbyte2 = g.objs.aliens[idx as usize].sbyte2.wrapping_sub(1);
            let Some(body) = airship_child(g, idx, BOSSF_BODY) else {
                airship_next(g, idx);
                return;
            };
            if add_anim_cap(&mut g.objs.aliens[body as usize], 1, 13) {
                airship_next(g, idx);
                return;
            }
            if let Some(head) = airship_child(g, idx, BOSSF_HEAD) {
                let _ = add_anim_cap(&mut g.objs.aliens[head as usize], 1, 4);
            }
            airship_change_z(g, idx);
            airship_move3(g, idx);
        }
        AIR_JOIN_SOUND => {
            strat_trig_se(g, 0x8e);
            g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
            airship_next(g, idx);
        }
        AIR_REJOIN => {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            let even = g.objs.aliens[idx as usize].sbyte2 & !1;
            if even == 0 {
                airship_next(g, idx);
                return;
            }
            g.objs.aliens[idx as usize].sbyte2 = even.wrapping_sub(2);
            airship_change_z(g, idx);
            airship_move3(g, idx);
        }
        AIR_MAJOR_CHANGE2 => {
            let Some(body) = airship_child(g, idx, BOSSF_BODY) else {
                airship_next(g, idx);
                return;
            };
            g.objs.aliens[body as usize].shape = SH_AIRSHIP_BODY_TRANSFORMED;
            init_anim(&mut g.objs.aliens[body as usize], 0);
            for (num, shape, x, init) in [
                (
                    BOSSF_ARM1,
                    SH_AIRSHIP_ARM_LEFT,
                    -40,
                    bossfarm_istrat as StrategyFn,
                ),
                (
                    BOSSF_ARM2,
                    SH_AIRSHIP_ARM_RIGHT,
                    40,
                    bossfarm_istrat as StrategyFn,
                ),
            ] {
                if let Some(arm) = airship_spawn_child(g, idx, num, shape, x, 0, 10, 0, 0, 0, init)
                {
                    g.objs.aliens[arm as usize].childrotobj = boss_obj_index_or_null(body);
                }
            }
            airship_next(g, idx);
        }
        AIR_MOVE_BACK => {
            if let Some(feet) = airship_child(g, idx, BOSSF_FEET) {
                g.objs.aliens[feet as usize].stratstate = FEET_BACKFORTH;
            }
            if !airship_dz_less(g, idx, 2700) {
                airship_next(g, idx);
            } else {
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_add(40);
                airship_move2(g, idx);
            }
        }
        AIR_START_IRONBALLS => {
            for n in [BOSSF_ARM1, BOSSF_ARM2] {
                if let Some(arm) = airship_child(g, idx, n) {
                    g.objs.aliens[arm as usize].stratstate = ARM_FIRERANDOMLY;
                }
            }
            airship_next(g, idx);
        }
        AIR_OPEN_HATCH | 15 | 42 => {
            if let Some(feet) = airship_child(g, idx, BOSSF_FEET) {
                g.objs.aliens[feet as usize].sflags2 &= !ASF2_SFLAG4;
                g.objs.aliens[feet as usize].sflags2 |= ASF2_SFLAG2;
            }
            airship_next(g, idx);
        }
        AIR_BODY_ROT => {
            let Some(body) = airship_child(g, idx, BOSSF_BODY) else {
                airship_next(g, idx);
                return;
            };
            g.objs.aliens[body as usize].sflags2 |= ASF2_SFLAG1;
            let n = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
            g.objs.aliens[idx as usize].sbyte2 = n;
            if n == 200 {
                g.objs.aliens[idx as usize].sbyte2 = 0;
                g.objs.aliens[body as usize].sflags2 &= !ASF2_SFLAG1;
                airship_next(g, idx);
            } else {
                airship_move2(g, idx);
            }
        }
        AIR_STOP_IT => {
            for n in [BOSSF_ARM1, BOSSF_ARM2] {
                if let Some(arm) = airship_child(g, idx, n) {
                    g.objs.aliens[arm as usize].stratstate = ARM_STAYABOVE;
                }
            }
            let rotating = airship_child(g, idx, BOSSF_BODY)
                .map(|body| g.objs.aliens[body as usize].sflags2 & ASF2_SFLAG3 != 0)
                .unwrap_or(false);
            if rotating {
                airship_move2(g, idx);
            } else {
                airship_next(g, idx);
            }
        }
        AIR_LOOP_START | 41 => {
            if let Some(a1) = airship_child(g, idx, BOSSF_ARM1) {
                g.objs.aliens[a1 as usize].stratstate = ARM_OHLORD1;
            }
            if let Some(a2) = airship_child(g, idx, BOSSF_ARM2) {
                g.objs.aliens[a2 as usize].stratstate = ARM_OHLORD2;
            }
            airship_next(g, idx);
        }
        16 | 43 => {
            let Some(body) = airship_child(g, idx, BOSSF_BODY) else {
                airship_next(g, idx);
                return;
            };
            let before = g.objs.aliens[body as usize].childroty;
            let after = before.wrapping_add(DEG22);
            g.objs.aliens[body as usize].childroty = after;
            if after < before {
                strat_trig_se(g, 0x39);
            }
            if let Some(head) = airship_child(g, idx, BOSSF_HEAD) {
                g.objs.aliens[head as usize].childroty = after;
            }
            if g.objs.aliens[idx as usize].sbyte2 == 200 {
                if after == 0 {
                    g.objs.aliens[idx as usize].sbyte2 = 0;
                    airship_next(g, idx);
                } else {
                    airship_move2(g, idx);
                }
            } else {
                g.objs.aliens[idx as usize].sbyte2 =
                    g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
                airship_move2(g, idx);
            }
        }
        17 | 23 | 27 | 30 | 46 => {
            if airship_dz_less(g, idx, 2700) {
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_add(20);
                if !airship_dz_less(g, idx, 2700) {
                    airship_next(g, idx);
                } else {
                    airship_move2(g, idx);
                }
            } else {
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_sub(20);
                if airship_dz_less(g, idx, 2700) {
                    airship_next(g, idx);
                } else {
                    airship_move2(g, idx);
                }
            }
        }
        18 | 44 => {
            if let Some(a1) = airship_child(g, idx, BOSSF_ARM1) {
                g.objs.aliens[a1 as usize].stratstate = ARM_SMACK1;
            }
            if let Some(a2) = airship_child(g, idx, BOSSF_ARM2) {
                g.objs.aliens[a2 as usize].stratstate = ARM_SMACK2;
            }
            airship_next(g, idx);
        }
        19 | 45 => {
            let done = airship_child(g, idx, BOSSF_ARM1)
                .map(|a| g.objs.aliens[a as usize].stratstate == ARM_STAYABOVE)
                .unwrap_or(true);
            if done {
                airship_next(g, idx);
            } else {
                airship_move2(g, idx);
            }
        }
        20 | 35 => {
            if let Some(body) = airship_child(g, idx, BOSSF_BODY) {
                g.objs.aliens[body as usize].sflags2 |= ASF2_SFLAG1;
            }
            airship_next(g, idx);
        }
        21 | 24 | 36 | 38 => airship_lift_arm(g, idx, BOSSF_ARM1),
        22 | 25 | 37 | 39 => airship_lift_arm(g, idx, BOSSF_ARM2),
        26 | 40 => {
            if let Some(body) = airship_child(g, idx, BOSSF_BODY) {
                g.objs.aliens[body as usize].sflags2 &= !ASF2_SFLAG1;
            }
            airship_next(g, idx);
        }
        28 | 33 => {
            if let Some(feet) = airship_child(g, idx, BOSSF_FEET) {
                g.objs.aliens[feet as usize].sflags2 |= ASF2_SFLAG4;
            }
            airship_next(g, idx);
        }
        29 | 34 => {
            let Some(head) = airship_child(g, idx, BOSSF_HEAD) else {
                airship_next(g, idx);
                return;
            };
            g.objs.aliens[head as usize].stratstate = HEAD_RINGLASER;
            let n = g.objs.aliens[idx as usize].sbyte2;
            if n == 100 {
                g.objs.aliens[head as usize].stratstate = HEAD_STAYABOVE;
                airship_zero_next(g, idx);
            } else {
                g.objs.aliens[idx as usize].sbyte2 = n.wrapping_add(1);
                airship_move2(g, idx);
            }
        }
        31 | 47 => {
            let hurt = airship_child(g, idx, BOSSF_FEET)
                .map(|f| g.objs.aliens[f as usize].hp < BOSSF_FEET_HP / 2)
                .unwrap_or(true);
            g.objs.aliens[idx as usize].stratstate =
                if hurt { AIR_HURT_LOOP } else { AIR_LOOP_START };
            airship_strat(g, idx);
        }
        AIR_HURT_LOOP => {
            if let Some(feet) = airship_child(g, idx, BOSSF_FEET) {
                g.objs.aliens[feet as usize].sflags3 |= ASF3_SFLAG6;
            }
            airship_next(g, idx);
        }
        AIRSHIP_MODE_BOSSF_HELI => {
            if let Some(body) = airship_child(g, idx, BOSSF_BODY) {
                g.objs.aliens[body as usize].stratstate = BODY_TOGROUND;
            }
            if let Some(a1) = airship_child(g, idx, BOSSF_ARM1) {
                g.objs.aliens[a1 as usize].stratstate = ARM_HELI1;
            }
            if let Some(a2) = airship_child(g, idx, BOSSF_ARM2) {
                g.objs.aliens[a2 as usize].stratstate = ARM_HELI2;
            }
            if let Some(head) = airship_child(g, idx, BOSSF_HEAD) {
                g.objs.aliens[head as usize].stratstate = HEAD_HELI;
            }
            airship_next(g, idx);
        }
        AIR_BACK_FORTH => {
            let one_way = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0;
            if !one_way {
                if airship_dz_less(g, idx, 800) {
                    g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG2;
                } else {
                    g.objs.aliens[idx as usize].worldz =
                        g.objs.aliens[idx as usize].worldz.wrapping_sub(50);
                }
            } else if !airship_dz_less(g, idx, 2500) {
                g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG2;
            } else {
                g.objs.aliens[idx as usize].worldz =
                    g.objs.aliens[idx as usize].worldz.wrapping_add(50);
            }
            airship_move2(g, idx);
        }
        _ => {
            g.objs.aliens[idx as usize].stratstate = AIR_LOOP_START;
            airship_strat(g, idx);
        }
    }
}

fn airship_lift_arm(g: &mut Game, idx: u16, child_num: u8) {
    let Some(arm) = airship_child(g, idx, child_num) else {
        airship_next(g, idx);
        return;
    };
    g.objs.aliens[arm as usize].stratstate = ARM_LIFTFIRE;
    let n = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
    g.objs.aliens[idx as usize].sbyte2 = n;
    if n == 70 {
        let power = g.vars.shared.power_build | 0x80;
        g.vars.shared.power_build = power;
        g.objs.aliens[arm as usize].stratstate = ARM_FIRE2;
        airship_zero_next(g, idx);
    } else {
        airship_move2(g, idx);
    }
}

fn airship_hit(g: &mut Game, idx: u16) {
    strat_trig_se(g, 0x27);
    strat_hit_flash(g, idx);
}

fn airship_explode(g: &mut Game, idx: u16) {
    if airship_dz_less(g, idx, 3000) {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(100);
        airship_move2(g, idx);
        return;
    }
    for n in [BOSSF_ARM1, BOSSF_ARM2, BOSSF_BODY] {
        if let Some(child) = airship_child(g, idx, n) {
            kill_obj(&mut g.objs.aliens[child as usize]);
        }
    }
    g.objs.aliens[idx as usize].expstratptr = Some(sid(g, strat_boss_explode_init));
}

/// ROM `bossffeetHP` (DSTRATS.ASM:92).
const BOSSF_FEET_HP: u8 = 80;
/// ROM `bossfheadHP` / `bossfheadAP` / `bossfheadHP2`.
const BOSSF_HEAD_HP: u8 = 6;
const BOSSF_HEAD_AP: u8 = 10;
const BOSSF_HEAD_HP2: u8 = 60;

/// `bossf_scale` == `childscale` (3) → `(n << scale) >> childscale` ≡ `n`.
const BOSSF_CHILD_Y_TOGROUND: i8 = -10;
const BOSSF_CHILD_Z_RETRACT: i8 = -40;
const BOSSF_CHILD_Z_REST: i8 = 10;
const BOSSF_CHILD_Y_RISE_DONE: i8 = -35;
const BOSS_HEAD_FLY_HEIGHT: i8 = -15;
const FLIGHT_RAD: i8 = 70;

/// Hit-zone HF1 (VARS.INC).
const HF1: u8 = 0x01;
const HF2: u8 = 0x02;

/// sflag4 lives in sflags2 bit 0x80 (same mapping as bossb / player).
const ASF2_SFLAG4: u8 = 0x80;

// ---- body mode indices (DSTRATS.ASM:7725-7732) ----
const BODY_STAYABOVE: u8 = 0;
const BODY_TOGROUND: u8 = 1;
const BODY_SPIN: u8 = 2;
const BODY_RISEUP: u8 = 3;
const BODY_BACKFORTH: u8 = 4;
const BODY_MOVE2: u8 = 5;

// ---- feet ----
const FEET_STAYABOVE: u8 = 0;
const FEET_BACKFORTH: u8 = 1;

// ---- arm (DSTRATS.ASM:7952-7972) ----
const ARM_STAYABOVE: u8 = 0;
const ARM_FIRERANDOMLY: u8 = 1;
const ARM_OHLORD1: u8 = 2;
const ARM_OHLORD2: u8 = 3;
const ARM_SMACK1: u8 = 4;
const ARM_SMACK2: u8 = 5;
const ARM_LIFTFIRE: u8 = 6;
const ARM_FIRE2: u8 = 7;
const ARM_HELI1: u8 = 8;
const ARM_HELI2: u8 = 9;

// ---- head ----
const HEAD_STAYABOVE: u8 = 0;
const HEAD_RINGLASER: u8 = 1;
const HEAD_HELI: u8 = 2;
const HEAD_ANIMATE: u8 = 3;
const HEAD_BEGIN_ROTATE: u8 = 4;
const HEAD_ROTATE: u8 = 5;
const HEAD_LOOPROUND: u8 = 6;

// ============================================================
// Shared helpers
// ============================================================

fn anim_lo(al: &Alien) -> u8 {
    al.animframe & 0x7f
}

fn init_anim(al: &mut Alien, frame: u8) {
    al.animframe = 0x80 | (frame & 0x7f);
}

/// 4-arg `s_add_anim x,#delta,#max,.label` CAP form: returns true if capped.
fn add_anim_cap(al: &mut Alien, delta: i8, max: u8) -> bool {
    let mut f = anim_lo(al) as i16;
    f = f.wrapping_add(delta as i16);
    if f < 0 {
        f = 0;
    }
    let capped = f as u8 >= max;
    if capped {
        f = (max.saturating_sub(1)) as i16;
    }
    al.animframe = 0x80 | ((f as u8) & 0x7f);
    capped
}

fn fchase_i8_byte(cur: &mut u8, target: i8, step: i8) -> bool {
    let mut c = *cur as i8;
    let d = target.wrapping_sub(c);
    if d == 0 {
        return true;
    }
    if d > step {
        c = c.wrapping_add(step);
    } else if d < -step {
        c = c.wrapping_sub(step);
    } else {
        c = target;
    }
    *cur = c as u8;
    c == target
}

fn delay_open(g: &Game, bits: u32) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

fn aim_at_player(g: &mut Game, idx: u16) {
    let Some(pl) = player(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let yaw = strat_angle_xz(&me, &pl);
    let pitch = strat_pitch_toward(&me, &pl);
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = yaw;
    al.rotx = pitch;
}

/// ROM `launchbigmissile` (DSTRATS.ASM:8357).
fn launch_big_missile(g: &mut Game, firer: u16) {
    let Some(shot) = strat_make_obj(g, 412) else {
        return;
    };
    {
        let src = g.objs.aliens[firer as usize];
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotx = src.rotx;
        al.roty = src.roty;
        al.rotz = src.rotz;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
    }
    let src = g.objs.aliens[firer as usize];
    let (rx, ry, rz) =
        crate::snes_trig::strat_roffs_full_scaled(src.rotz, src.rotx, src.roty, 0, 0, 120, 2);
    let al = &mut g.objs.aliens[shot as usize];
    al.worldx = src.worldx.wrapping_add(rx);
    al.worldy = src.worldy.wrapping_add(ry);
    al.worldz = src.worldz.wrapping_add(rz);
    aim_at_player(g, shot);
    ironballmissile_istrat(g, shot);
}

/// ROM `launchhead` (DSTRATS.ASM:8383).
fn launch_head(g: &mut Game, firer: u16) {
    let Some(shot) = strat_make_obj(g, 413) else {
        return;
    };
    {
        let src = g.objs.aliens[firer as usize];
        let al = &mut g.objs.aliens[shot as usize];
        al.worldx = src.worldx;
        al.worldy = src.worldy;
        al.worldz = src.worldz;
        al.rotx = src.rotx;
        al.roty = src.roty;
        al.rotz = src.rotz;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.sflags |= ASF_SHADOW;
    }
    aim_at_player(g, shot);
    headfire_istrat(g, shot);
}

// ============================================================
// bossfbody — DSTRATS.ASM:7715+
// ============================================================

/// ROM `bossfbody_istrat`.
pub fn bossfbody_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossfbody_strat);
    let hit = sid(g, bossfbody_hit);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.sflags |= ASF_SHADOW | ASF_NOHITAFFECT;
        al.stratstate = BODY_STAYABOVE;
        init_anim(al, 0);
    }
    bossfbody_strat(g, idx);
}

fn bossfbody_hit(g: &mut Game, idx: u16) {
    strat_trig_se(g, 0x27);
    strat_hit_flash(g, idx);
}

/// ROM `bossfbody` `.strat` mode table.
pub fn bossfbody_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        BODY_STAYABOVE => bossfbody_move(g, idx),
        BODY_TOGROUND => bossfbody_toground(g, idx),
        BODY_SPIN => bossfbody_spin(g, idx),
        BODY_RISEUP => bossfbody_riseup(g, idx),
        BODY_BACKFORTH => {
            bossfbody_move3(g, idx);
        }
        BODY_MOVE2 => bossfbody_move2(g, idx),
        _ => bossfbody_move(g, idx),
    }
}

fn bossfbody_nxtmode(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
    bossfbody_strat(g, idx);
}

fn bossfbody_toground(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
    g.objs.aliens[idx as usize].sbyte4 = 0;
    let y_done = {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.childy, BOSSF_CHILD_Y_TOGROUND as u8, 2)
    };
    {
        let al = &mut g.objs.aliens[idx as usize];
        let _ = achase_angle(&mut al.childroty, 0, 3);
    }
    if y_done {
        bossfbody_nxtmode(g, idx);
        return;
    }
    bossfbody_move2(g, idx);
}

fn bossfbody_spin(g: &mut Game, idx: u16) {
    if delay_open(g, 1) {
        g.objs.aliens[idx as usize].sbyte4 = g.objs.aliens[idx as usize].sbyte4.wrapping_add(1);
    }
    if g.objs.aliens[idx as usize].sbyte4 == DEG22.wrapping_add(DEG11) {
        bossfbody_nxtmode(g, idx);
        return;
    }
    bossfbody_move2(g, idx);
}

fn bossfbody_riseup(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG2;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.childy = (al.childy as i8).wrapping_sub(1) as u8;
        if al.childy as i8 == BOSSF_CHILD_Y_RISE_DONE {
            bossfbody_nxtmode(g, idx);
            return;
        }
    }
    bossfbody_move2(g, idx);
}

fn bossfbody_move3(g: &mut Game, idx: u16) {
    // s_jmp_random .move2,99 — ~1/100 chance to launch head.
    if (sf_random(&mut g.vars) & 0xff) as u8 >= 99 {
        launch_head(g, idx);
    }
    bossfbody_move2(g, idx);
}

fn bossfbody_move2(g: &mut Game, idx: u16) {
    let spin = g.objs.aliens[idx as usize].sbyte4;
    let before = g.objs.aliens[idx as usize].childroty;
    g.objs.aliens[idx as usize].childroty = before.wrapping_add(spin);
    // Carry into unsigned add ≈ BCC nosnd when no wrap past 255.
    if (before as u16) + (spin as u16) > 255 {
        strat_trig_se(g, 0x39);
    }
}

fn bossfbody_move(g: &mut Game, idx: u16) {
    let s2 = g.objs.aliens[idx as usize].sflags2;
    if s2 & ASF2_SFLAG1 != 0 {
        // .backandforth
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG3;
        let target = if s2 & ASF2_SFLAG2 != 0 {
            DEG22
        } else {
            0u8.wrapping_sub(DEG22)
        };
        let reached = {
            let al = &mut g.objs.aliens[idx as usize];
            fchase_i8_byte(&mut al.childroty, target as i8, 1)
        };
        if reached {
            g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG2;
        }
    } else if s2 & ASF2_SFLAG3 != 0 {
        let reached = {
            let al = &mut g.objs.aliens[idx as usize];
            fchase_i8_byte(&mut al.childroty, 0, 1)
        };
        if reached {
            g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG3;
        }
    }
}

// ============================================================
// bossffeet — DSTRATS.ASM:7806+
// ============================================================

/// ROM `bossffeet_istrat`.
pub fn bossffeet_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossffeet_strat);
    let hit = sid(g, bossffeet_hit);
    let exp = sid(g, bossffeet_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.hp = BOSSF_FEET_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_SHADOW;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.stratstate = FEET_STAYABOVE;
        init_anim(al, 0);
    }
    set_bossmaxhp(g, BOSSF_FEET_HP as u16);
    bossffeet_strat(g, idx);
}

/// ROM `bossffeet` `.strat`.
pub fn bossffeet_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        FEET_BACKFORTH => bossffeet_backandforth(g, idx),
        _ => bossffeet_move(g, idx),
    }
}

fn bossffeet_backandforth(g: &mut Game, idx: u16) {
    let Some(mother) = boss_get_mother_obj(g, idx) else {
        bossffeet_move(g, idx);
        return;
    };
    // Mother worldx += -childroty/2 (signed).
    let croty = g.objs.aliens[idx as usize].childroty as i8;
    let dx = -((croty as i16) / 2);
    g.objs.aliens[mother as usize].worldx = g.objs.aliens[mother as usize].worldx.wrapping_add(dx);

    let Some(pl) = player(g) else {
        bossffeet_move(g, idx);
        return;
    };
    let x1 = g.objs.aliens[mother as usize]
        .worldx
        .wrapping_sub(pl.worldx);

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.childroty = al.childroty.wrapping_add(1);
        if (al.childroty as i8) >= 32 {
            al.childroty = 32;
        }
        if x1 >= -50 {
            // fall through
        } else {
            al.sflags2 |= ASF2_SFLAG1;
        }
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.childroty = al.childroty.wrapping_sub(1);
        if (al.childroty as i8) <= -32 {
            al.childroty = (-32i8) as u8;
        }
        if x1 < 35 {
            // fall through
        } else {
            al.sflags2 &= !ASF2_SFLAG1;
        }
    }
    bossffeet_move(g, idx);
}

fn bossffeet_move(g: &mut Game, idx: u16) {
    let s2 = g.objs.aliens[idx as usize].sflags2;
    let s3 = g.objs.aliens[idx as usize].sflags3;
    if s2 & ASF2_SFLAG4 != 0 || (s2 & ASF2_SFLAG2 != 0 && s3 & ASF3_SFLAG5 == 0) {
        // .fullopen / open path
        if add_anim_cap(&mut g.objs.aliens[idx as usize], 1, 10) {
            g.objs.aliens[idx as usize].sflags3 ^= ASF3_SFLAG5; // not sflag3
        }
    } else if s2 & ASF2_SFLAG2 != 0 && s3 & ASF3_SFLAG5 != 0 {
        // .close
        if anim_lo(&g.objs.aliens[idx as usize]) != 0 {
            let _ = add_anim_cap(&mut g.objs.aliens[idx as usize], -1, 10);
        } else {
            g.objs.aliens[idx as usize].sflags3 ^= ASF3_SFLAG5;
        }
    } else if anim_lo(&g.objs.aliens[idx as usize]) != 0 {
        let _ = add_anim_cap(&mut g.objs.aliens[idx as usize], -1, 10);
    }

    // At the eighth opening frame, the source's BOSSHMISSILE1 weapon fires.
    if g.objs.aliens[idx as usize].sflags3 & ASF3_SFLAG5 != 0
        && anim_lo(&g.objs.aliens[idx as usize]) == 8
    {
        if (sf_random(&mut g.vars) % 100) as u8 >= 70 {
            let _ = fire_boss_hmissile1(g, idx);
        }
        g.objs.aliens[idx as usize].sflags3 &= !ASF3_SFLAG5;
    }

    if g.objs.aliens[idx as usize].sflags3 & ASF3_SFLAG6 != 0 {
        launch_big_missile(g, idx);
        g.objs.aliens[idx as usize].sflags3 &= !ASF3_SFLAG6;
    }

    add_bosshp(g, idx);
}

fn bossffeet_hit(g: &mut Game, idx: u16) {
    let anim = anim_lo(&g.objs.aliens[idx as usize]);
    let hf = g.objs.aliens[idx as usize].hitflags;
    if anim >= 5 && hf & HF1 != 0 {
        g.objs.aliens[idx as usize].hitflags &= !(HF1 | HF2);
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        g.objs.aliens[idx as usize].sflags3 |= ASF3_SFLAG5;
        strat_trig_se(g, 0x80);
    } else {
        g.objs.aliens[idx as usize].hitflags &= !(HF1 | HF2);
        g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
        strat_trig_se(g, 0x27);
    }
    strat_hit_flash(g, idx);
}

fn bossffeet_explode(g: &mut Game, idx: u16) {
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        g.objs.aliens[mother as usize].stratstate = AIRSHIP_MODE_BOSSF_HELI;
    }
    strat_explode(g, idx);
}

// ============================================================
// bossfarm — DSTRATS.ASM:7942+
// ============================================================

/// ROM `bossfarm_istrat`.
pub fn bossfarm_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossfarm_strat);
    let hit = sid(g, bossfarm_hit);
    let exp = sid(g, bossfarm_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_NOHITAFFECT | ASF_SHADOW;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.stratstate = ARM_STAYABOVE;
        init_anim(al, 0);
    }
    bossfarm_strat(g, idx);
}

/// ROM `bossfarm` `.strat`.
pub fn bossfarm_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        ARM_STAYABOVE => {
            let _ = fchase_i8_byte(&mut g.objs.aliens[idx as usize].sbyte2, 0, 1);
            bossfarm_move(g, idx);
        }
        ARM_FIRERANDOMLY => bossfarm_firerandomly(g, idx),
        ARM_OHLORD1 => {
            let _ = fchase_i8_byte(&mut g.objs.aliens[idx as usize].childroty, DEG90 as i8, 1);
            bossfarm_firerandomly3(g, idx);
            bossfarm_move(g, idx);
        }
        ARM_OHLORD2 => {
            let _ = fchase_i8_byte(
                &mut g.objs.aliens[idx as usize].childroty,
                -(DEG90 as i8),
                1,
            );
            bossfarm_firerandomly3(g, idx);
            bossfarm_move(g, idx);
        }
        ARM_SMACK1 | ARM_SMACK2 => {
            if fchase_i8_byte(&mut g.objs.aliens[idx as usize].childroty, 0, DEG22 as i8) {
                g.objs.aliens[idx as usize].stratstate = ARM_STAYABOVE;
                bossfarm_strat(g, idx);
                return;
            }
            bossfarm_move(g, idx);
        }
        ARM_LIFTFIRE => {
            if fchase_i8_byte(&mut g.objs.aliens[idx as usize].sbyte2, -(DEG90 as i8), 1) {
                if delay_open(g, 2) {
                    let _ = fire_ironball2(g, idx);
                }
            }
            bossfarm_move(g, idx);
        }
        ARM_FIRE2 => {
            if g.objs.aliens[idx as usize].sbyte2 == 0 {
                if delay_open(g, 3) {
                    g.objs.aliens[idx as usize].sbyte4 =
                        g.objs.aliens[idx as usize].sbyte4.wrapping_add(1);
                    if g.objs.aliens[idx as usize].sbyte4 == 3 {
                        g.objs.aliens[idx as usize].sbyte4 = 0;
                        g.objs.aliens[idx as usize].stratstate = ARM_STAYABOVE;
                        bossfarm_strat(g, idx);
                        return;
                    }
                    let _ = fire_ironball3(g, idx);
                }
            } else {
                // .stayabove branch when sbyte2 nonzero
                let _ = fchase_i8_byte(&mut g.objs.aliens[idx as usize].sbyte2, 0, 1);
            }
            bossfarm_move(g, idx);
        }
        ARM_HELI1 => {
            let al = &mut g.objs.aliens[idx as usize];
            let _ = achase_angle(&mut al.childrotz, 0u8.wrapping_sub(DEG90), 5);
            let _ = achase_angle(&mut al.childroty, DEG90, 5);
            let _ = achase_angle(&mut al.childrotx, 0, 5);
            // .move2 → end
        }
        ARM_HELI2 => {
            let al = &mut g.objs.aliens[idx as usize];
            let _ = achase_angle(&mut al.childrotz, 0u8.wrapping_sub(DEG90), 5);
            let _ = achase_angle(&mut al.childroty, 0u8.wrapping_sub(DEG90), 5);
            let _ = achase_angle(&mut al.childrotx, 0, 5);
        }
        _ => bossfarm_move(g, idx),
    }
}

fn bossfarm_firerandomly(g: &mut Game, idx: u16) {
    if (sf_random(&mut g.vars) & 0xff) as u8 >= 99 {
        let _ = fire_ironball(g, idx);
        g.objs.aliens[idx as usize].childz = BOSSF_CHILD_Z_RETRACT as u8;
        g.objs.aliens[idx as usize].sbyte3 = 0u8.wrapping_sub(DEG22);
    }
    let _ = fchase_i8_byte(&mut g.objs.aliens[idx as usize].sbyte2, -(DEG22 as i8), 1);
    bossfarm_move(g, idx);
}

fn bossfarm_firerandomly3(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].roty == DEG180 {
        let _ = fire_ironball3(g, idx);
    }
}

fn bossfarm_move(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let _ = fchase_i8_byte(&mut al.childz, BOSSF_CHILD_Z_REST, 1);
        let _ = fchase_i8_byte(&mut al.sbyte3, 0, 1);
        al.rotx = al.sbyte2;
        al.rotx = al.rotx.wrapping_add(al.sbyte3);
        al.rotx = al.rotx.wrapping_add(al.sword2 as u8);
        // s_fchase_alvar B sword2 → 0 (signed word used as recoil byte).
        if al.sword2 > 0 {
            al.sword2 -= 1;
        } else if al.sword2 < 0 {
            al.sword2 += 1;
        }
    }
}

fn bossfarm_hit(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sword2 = -16;
    strat_trig_se(g, 0x27);
    strat_hit_flash(g, idx);
}

fn bossfarm_explode(g: &mut Game, idx: u16) {
    divorce_family(g, idx);
    let exp = sid(g, bossfarm_expstrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 100;
        strat_gen_vecs_3d(al);
        al.expstratptr = Some(exp);
    }
    bossfarm_expstrat(g, idx);
}

fn bossfarm_expstrat(g: &mut Game, idx: u16) {
    let explode = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(4);
        if al.worldy >= 0 {
            al.worldy = 0;
            al.expstratptr = Some(explode);
            return;
        }
        strat_apply_velocity(al);
    }
}

// ============================================================
// bossfhead — DSTRATS.ASM:7565+
// ============================================================

/// ROM `bossfhead_istrat`.
pub fn bossfhead_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossfhead_strat);
    let hit = sid(g, bossfhead_hit);
    let exp = sid(g, bossfhead_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.sflags |= ASF_NOHITAFFECT;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.stratstate = HEAD_STAYABOVE;
        init_anim(al, 0);
    }
    bossfhead_strat(g, idx);
}

/// ROM `bossfhead` `.strat`.
pub fn bossfhead_strat(g: &mut Game, idx: u16) {
    match g.objs.aliens[idx as usize].stratstate {
        HEAD_STAYABOVE => {}
        HEAD_RINGLASER => bossfhead_ringlasereyes(g, idx),
        HEAD_HELI => {
            if achase_angle(&mut g.objs.aliens[idx as usize].childroty, 0, 5) {
                g.objs.aliens[idx as usize].stratstate = HEAD_ANIMATE;
                let hit = sid(g, bossfhead_hit);
                g.objs.aliens[idx as usize].collstratptr = Some(hit);
                bossfhead_strat(g, idx);
                return;
            }
        }
        HEAD_ANIMATE => {
            if anim_lo(&g.objs.aliens[idx as usize]) == 0 {
                g.objs.aliens[idx as usize].stratstate = HEAD_BEGIN_ROTATE;
                bossfhead_strat(g, idx);
                return;
            }
            let _ = add_anim_cap(&mut g.objs.aliens[idx as usize], -1, 10);
        }
        HEAD_BEGIN_ROTATE => bossfhead_movetobeginrotate(g, idx),
        HEAD_ROTATE => bossfhead_rotate(g, idx),
        HEAD_LOOPROUND => {
            g.objs.aliens[idx as usize].stratstate = HEAD_BEGIN_ROTATE;
            bossfhead_strat(g, idx);
            return;
        }
        _ => {}
    }
}

fn bossfhead_nxtmode(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].stratstate = g.objs.aliens[idx as usize].stratstate.wrapping_add(1);
    bossfhead_strat(g, idx);
}

fn bossfhead_ringlasereyes(g: &mut Game, idx: u16) {
    if delay_open(g, 3) {
        if let Some(shot) = firenormringlaser(g, idx) {
            aim_at_player(g, shot);
            g.objs.aliens[shot as usize].collflags |= ACF_COLLTYPE2; // ROM ENEMY1
            g.objs.aliens[shot as usize].worldx =
                g.objs.aliens[shot as usize].worldx.wrapping_sub(80);
        }
        if let Some(shot) = firenormringlaser(g, idx) {
            aim_at_player(g, shot);
            g.objs.aliens[shot as usize].collflags |= ACF_COLLTYPE2; // ROM ENEMY1
            g.objs.aliens[shot as usize].worldx =
                g.objs.aliens[shot as usize].worldx.wrapping_add(80);
        }
    }
}

fn bossfhead_movetobeginrotate(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_SHADOW;
        al.hp = BOSSF_HEAD_HP2;
        al.sflags &= !ASF_NOHITAFFECT;
        let _ = achase_angle(&mut al.childrotx, 0, 4);
        let _ = achase_angle(&mut al.childroty, DEG90, 4);
        let _ = achase_angle(&mut al.childrotz, 0, 4);
        let _ = achase_angle(&mut al.childx, 0, 4);
        let _ = achase_angle(&mut al.childy, BOSS_HEAD_FLY_HEIGHT as u8, 4);
        let _ = achase_angle(&mut al.childz, FLIGHT_RAD as u8, 4);
        al.sbyte3 = 0;
        al.sbyte2 = 0;
        al.sbyte4 = al.sbyte4.wrapping_add(1);
    }
    set_bossmaxhp(g, BOSSF_HEAD_HP2 as u16);
    add_bosshp(g, idx);
    if g.objs.aliens[idx as usize].sbyte4 == 50 {
        g.objs.aliens[idx as usize].sbyte4 = 0;
        bossfhead_nxtmode(g, idx);
    }
}

fn bossfhead_rotate(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        let sin_idx = al.sbyte2;
        let sy = SINTAB[sin_idx as usize] >> 4; // sintab,-4
        al.childy = (sy as i8).wrapping_add(BOSS_HEAD_FLY_HEIGHT) as u8;
        al.sbyte2 = al.sbyte2.wrapping_add(16);
        al.sbyte3 = al.sbyte3.wrapping_add(4);
        let (cx, cz) = rotate_8xz(al.sbyte3, 0, FLIGHT_RAD);
        al.childx = cx as u8;
        al.childz = cz as u8;
        al.childroty = al.sbyte3.wrapping_add(DEG90);
        al.childrotz = DEG11;
    }
    add_bosshp(g, idx);
}

fn bossfhead_hit(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags & ASF_NOHITAFFECT == 0 {
        strat_trig_se(g, 0x80);
    }
    strat_hit_flash(g, idx);
}

fn bossfhead_explode(g: &mut Game, idx: u16) {
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        kill_obj(&mut g.objs.aliens[mother as usize]);
    }
    strat_trig_se(g, 0x81);
    strat_explode(g, idx);
}

// ============================================================
// headfire (launched head projectile) — DSTRATS.ASM:8407+
// ============================================================

fn headfire_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, headfire_strat);
    let hit = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(hit);
        al.expstratptr = Some(exp);
        al.hp = BOSSF_HEAD_HP;
        al.ap = BOSSF_HEAD_AP;
    }
    headfire_strat(g, idx);
}

fn headfire_strat(g: &mut Game, idx: u16) {
    let next = sid(g, headfire_strat2);
    let reached = {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(4);
        strat_apply_velocity(al);
        al.worldy >= 0
    };
    if reached {
        g.objs.aliens[idx as usize].worldy = 0;
        g.objs.aliens[idx as usize].stratptr = Some(next);
        strat_trig_se(g, 0x8f);
        aim_at_player(g, idx);
        g.objs.aliens[idx as usize].vel = 120;
        strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
        return;
    }
    add_player_z(g, idx);
}

fn headfire_strat2(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize]
        .rotx
        .wrapping_add(DEG22.wrapping_add(DEG22)); // deg45
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_init_sets_hard_and_shadow() {
        let mut g = Game::new();
        let idx = g.objs.alloc().unwrap();
        bossfbody_istrat(&mut g, idx);
        let al = &g.objs.aliens[idx as usize];
        assert_eq!(al.hp, HARD_HP);
        assert_ne!(al.sflags & ASF_SHADOW, 0);
        assert_ne!(al.sflags & ASF_NOHITAFFECT, 0);
        assert_eq!(al.stratstate, BODY_STAYABOVE);
        assert!(al.stratptr.is_some());
    }
}
