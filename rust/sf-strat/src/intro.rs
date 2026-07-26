//! Retail attract-intro object strategies (`GISTRATS.ASM:691-1054`).
//!
//! These are direct map symbols rather than ISTRATS rows. The intro map
//! creates three Arwing flyby objects, a paired-laser controller, a fighter
//! wave, and a lead fighter whose pass requests the return to the title.

use sf_game::alien::{Alien, ASF_COLLDISABLE, ASF_HITFLASH, ATZREMOVE};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::HARD_AP;

use crate::common::{
    apply_velocity, gen_vecs_3d, make_smoke_on_cadence, sf_random, speed_to, SmokeCadence,
};
use crate::enemy_a::{
    achase_angle, add_player_z, fastexplodedebris_istrat, fire_relfastelaser_weapon_pos,
    make_medium_exp_obj, player, sid, strat_aim_3d, strat_obj_from_ptr, strat_pitch_toward,
    ASF2_NOEXPSND, DEG22, DEG45, DEG90,
};

const OLD_TYPE_SHAPE: u16 = 323;
const INTRO_CRAFT_HP: u8 = 1;
const INTRO_CRAFT_SPEED: u8 = 120;
const INTRO_CRAFT_SOUND: u8 = 1;
const INTRO_CRAFT_INITIAL_TILT_FRAMES: u8 = 33;
const INTRO_CRAFT_FIRST_HIT_DELAY: i16 = 16;
const INTRO_CRAFT_SECOND_HIT_DELAY: i16 = 20;
const INTRO_CRAFT_SMOKE_VELOCITY_X: i16 = 40;
const INTRO_CRAFT_SMOKE_SCATTER_MASK: u16 = 127;
const INTRO_CRAFT_SMOKE_SCATTER_CENTER: i16 = (INTRO_CRAFT_SMOKE_SCATTER_MASK / 2) as i16;
const INTRO_CRAFT_VISIBLE_CHASE_RATE: u32 = 4;
const INTRO_CRAFT_ROLL_CHASE_RATE: u32 = 3;
const INTRO_CRAFT_FLIGHT_CHASE_RATE: u32 = 5;

const WING_CRAFT_INITIAL_TILT_FRAMES: u8 = 30;
const WING_CRAFT_LIFETIME: u8 = 70;
const WING_CRAFT_HP: u8 = HARD_AP;

const LASER_CONTROLLER_LIFETIME: u8 = 35;
const LASER_LIFETIME: u8 = 60;
const LASER_MUZZLE_X: i8 = 50;
const LASER_SPREAD_X: i16 = 350;
const EVERY_FOURTH_FRAME_MASK: u16 = 3;

const ZACO_ANIMATION_FRAMES: u8 = 12;
const ANIMATION_ACTIVE: u8 = 128;
const ANIMATION_FRAME_MASK: u8 = ANIMATION_ACTIVE - 1;
const ZACO_INITIAL_TILT_MASK: u8 = 15;
const ZACO_INITIAL_TILT_BASE: u8 = DEG90 - 7;
const ZACO_TURN_DELAY: u8 = 10;
const ZACO_INITIAL_SPEED: u8 = 60;
const ZACO_TARGET_SPEED: u8 = 120;
const ZACO_ACCELERATION: u8 = 2;
const ZACO_LIFETIME: u8 = 60;
const ZACO_SOUND: u8 = 3;
const ZACO_ROLL_SPEED: u8 = 6;

const LEADER_INITIAL_SPEED: u8 = 45;
const LEADER_FIRST_PHASE_FRAMES: u8 = 30;
const LEADER_SECOND_PHASE_FRAMES: u8 = 42;
const LEADER_FINAL_PHASE_FRAMES: u8 = 20;
const LEADER_EXIT_DELAY: u8 = 6;
const LEADER_TUMBLE_PITCH: u8 = 4;
const LEADER_TUMBLE_YAW: u8 = 9;
const LEADER_FIRE_SPREAD_MASK: u8 = 7;
const LEADER_FIRE_MIN_DISTANCE: i32 = 50;
const LEADER_FIRE_MAX_DISTANCE: i32 = 1000;
const LEADER_AIM_DISTANCE: i32 = 500;
const LEADER_AIM_RATE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlayerDownPhase {
    InitialDive,
    FirstHit,
    DamagedFlight,
}

impl PlayerDownPhase {
    fn from_state(state: u8) -> Self {
        match state {
            0 => Self::InitialDive,
            1 => Self::FirstHit,
            _ => Self::DamagedFlight,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeaderPhase {
    InitialRoll,
    Tumble,
    Attack,
}

impl LeaderPhase {
    fn from_state(state: u8) -> Self {
        match state {
            0 => Self::InitialRoll,
            1 => Self::Tumble,
            _ => Self::Attack,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WingDirection {
    Left,
    Right,
}

impl WingDirection {
    fn yaw_step(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Right => 1u8.wrapping_neg(),
        }
    }

    fn yaw_target(self) -> u8 {
        match self {
            Self::Left => DEG90,
            Self::Right => DEG90.wrapping_neg(),
        }
    }

    fn roll_target(self) -> u8 {
        match self {
            Self::Left => DEG45,
            Self::Right => DEG45.wrapping_neg(),
        }
    }
}

fn init_animation(object: &mut Alien) {
    object.animframe = ANIMATION_ACTIVE;
}

fn advance_animation(object: &mut Alien) {
    let frame = (object.animframe & ANIMATION_FRAME_MASK).wrapping_add(1) % ZACO_ANIMATION_FRAMES;
    object.animframe = ANIMATION_ACTIVE | frame;
}

fn decrement_lifetime(game: &mut Game, object: u16) -> bool {
    let remaining = game.objs.aliens[object as usize].count.wrapping_sub(1);
    game.objs.aliens[object as usize].count = remaining;
    if remaining == 0 {
        game.objs.aldead = 1;
        true
    } else {
        false
    }
}

fn generate_saved_heading_velocity(object: &mut Alien) {
    let visible_pitch = object.rotx;
    let visible_yaw = object.roty;
    object.rotx = object.sbyte4;
    object.roty = object.sbyte3;
    gen_vecs_3d(object);
    object.rotx = visible_pitch;
    object.roty = visible_yaw;
}

fn player_down_shared_motion(game: &mut Game, object: u16) {
    {
        let craft = &mut game.objs.aliens[object as usize];
        let mut flight_yaw = craft.sbyte3;
        let mut flight_pitch = craft.sbyte4;
        achase_angle(&mut flight_yaw, craft.roty, INTRO_CRAFT_FLIGHT_CHASE_RATE);
        achase_angle(&mut flight_pitch, craft.rotx, INTRO_CRAFT_FLIGHT_CHASE_RATE);
        craft.sbyte3 = flight_yaw;
        craft.sbyte4 = flight_pitch;
        generate_saved_heading_velocity(craft);
        apply_velocity(craft);
    }
    add_player_z(game, object);
}

fn add_intro_smoke_scatter(game: &mut Game, smoke: u16) {
    let offset_x = (sf_random(&mut game.vars) & INTRO_CRAFT_SMOKE_SCATTER_MASK) as i16
        - INTRO_CRAFT_SMOKE_SCATTER_CENTER;
    let offset_y = (sf_random(&mut game.vars) & INTRO_CRAFT_SMOKE_SCATTER_MASK) as i16
        - INTRO_CRAFT_SMOKE_SCATTER_CENTER;
    let offset_z = (sf_random(&mut game.vars) & INTRO_CRAFT_SMOKE_SCATTER_MASK) as i16
        - INTRO_CRAFT_SMOKE_SCATTER_CENTER;
    let smoke = &mut game.objs.aliens[smoke as usize];
    smoke.worldx = smoke.worldx.wrapping_add(offset_x);
    smoke.worldy = smoke.worldy.wrapping_add(offset_y);
    smoke.worldz = smoke.worldz.wrapping_add(offset_z);
}

/// `playerdownintro_Istrat`: initialize the center Arwing and fall through to
/// its first retail movement tick.
pub fn player_down_intro_init(game: &mut Game, object: u16) {
    let tick = sid(game, player_down_intro_tick as StrategyFn);
    {
        let craft = &mut game.objs.aliens[object as usize];
        craft.sflags |= ASF_COLLDISABLE;
        craft.rotx = DEG22;
        craft.type_ &= !ATZREMOVE;
        craft.stratptr = Some(tick);
        craft.sbyte3 = craft.roty;
        craft.sbyte4 = craft.rotx;
        craft.sbyte1 = INTRO_CRAFT_INITIAL_TILT_FRAMES;
        craft.sword2 = INTRO_CRAFT_FIRST_HIT_DELAY;
        craft.debrisshape = OLD_TYPE_SHAPE;
        craft.vel = INTRO_CRAFT_SPEED;
        craft.hp = INTRO_CRAFT_HP;
        craft.ap = HARD_AP;
        craft.snd2 = INTRO_CRAFT_SOUND;
        craft.stratstate = PlayerDownPhase::InitialDive as u8;
    }
    player_down_intro_tick(game, object);
}

/// `playerdownintro_strat`: source-ordered dive, first hit, smoke, and
/// destruction sequence.
pub fn player_down_intro_tick(game: &mut Game, object: u16) {
    let mut phase = PlayerDownPhase::from_state(game.objs.aliens[object as usize].stratstate);
    if phase == PlayerDownPhase::InitialDive {
        if game.objs.aliens[object as usize].sbyte1 == 0 {
            game.objs.aliens[object as usize].stratstate = PlayerDownPhase::FirstHit as u8;
            phase = PlayerDownPhase::FirstHit;
        } else {
            let craft = &mut game.objs.aliens[object as usize];
            craft.sbyte1 = craft.sbyte1.wrapping_sub(1);
            craft.rotz = craft.rotz.wrapping_add(1);
            if SmokeCadence::EveryFourthFrame.is_due(game.vars.gameframe) {
                craft.rotx = craft.rotx.wrapping_sub(1);
            }
        }
    }

    if phase == PlayerDownPhase::FirstHit {
        game.objs.aliens[object as usize].sword2 = INTRO_CRAFT_SECOND_HIT_DELAY;
        if let Some(explosion) = make_medium_exp_obj(game, object) {
            game.objs.aliens[explosion as usize].sflags2 &= !ASF2_NOEXPSND;
        }
        game.objs.aliens[object as usize].sflags |= ASF_HITFLASH;
        game.objs.aliens[object as usize].stratstate = PlayerDownPhase::DamagedFlight as u8;
        phase = PlayerDownPhase::DamagedFlight;
    }

    if phase == PlayerDownPhase::DamagedFlight {
        if game.objs.aliens[object as usize].sword2 == 0 {
            fastexplodedebris_istrat(game, object);
            return;
        }
        game.objs.aliens[object as usize].sword2 =
            game.objs.aliens[object as usize].sword2.wrapping_sub(1);
        if let Some(smoke) = make_smoke_on_cadence(game, object, SmokeCadence::EveryFrame) {
            game.objs.aliens[smoke as usize].vx = INTRO_CRAFT_SMOKE_VELOCITY_X;
            add_intro_smoke_scatter(game, smoke);
        }
        let craft = &mut game.objs.aliens[object as usize];
        achase_angle(&mut craft.roty, DEG90, INTRO_CRAFT_VISIBLE_CHASE_RATE);
        achase_angle(&mut craft.rotx, 0, INTRO_CRAFT_VISIBLE_CHASE_RATE);
        achase_angle(&mut craft.rotz, DEG45, INTRO_CRAFT_ROLL_CHASE_RATE);
    }

    player_down_shared_motion(game, object);
}

fn player_down_wing_init(game: &mut Game, object: u16, direction: WingDirection) {
    let tick = match direction {
        WingDirection::Left => player_down_left_intro_tick as StrategyFn,
        WingDirection::Right => player_down_right_intro_tick as StrategyFn,
    };
    let tick = sid(game, tick);
    {
        let craft = &mut game.objs.aliens[object as usize];
        craft.sflags |= ASF_COLLDISABLE;
        craft.rotx = DEG22;
        craft.type_ &= !ATZREMOVE;
        craft.stratptr = Some(tick);
        craft.sbyte3 = craft.roty;
        craft.sbyte4 = craft.rotx;
        craft.sbyte1 = WING_CRAFT_INITIAL_TILT_FRAMES;
        craft.vel = INTRO_CRAFT_SPEED;
        craft.hp = WING_CRAFT_HP;
        craft.ap = HARD_AP;
        craft.count = WING_CRAFT_LIFETIME;
        craft.snd2 = INTRO_CRAFT_SOUND;
        craft.stratstate = PlayerDownPhase::InitialDive as u8;
    }
    player_down_wing_tick(game, object, direction);
}

fn player_down_wing_tick(game: &mut Game, object: u16, direction: WingDirection) {
    if decrement_lifetime(game, object) {
        return;
    }

    let mut phase = PlayerDownPhase::from_state(game.objs.aliens[object as usize].stratstate);
    if phase == PlayerDownPhase::InitialDive {
        if game.objs.aliens[object as usize].sbyte1 == 0 {
            game.objs.aliens[object as usize].stratstate = PlayerDownPhase::FirstHit as u8;
            phase = PlayerDownPhase::FirstHit;
        } else {
            let craft = &mut game.objs.aliens[object as usize];
            craft.sbyte1 = craft.sbyte1.wrapping_sub(1);
            if SmokeCadence::EveryFourthFrame.is_due(game.vars.gameframe) {
                craft.rotx = craft.rotx.wrapping_sub(1);
                craft.roty = craft.roty.wrapping_add(direction.yaw_step());
            }
        }
    }

    if phase != PlayerDownPhase::InitialDive {
        let craft = &mut game.objs.aliens[object as usize];
        achase_angle(
            &mut craft.roty,
            direction.yaw_target(),
            INTRO_CRAFT_VISIBLE_CHASE_RATE,
        );
        achase_angle(&mut craft.rotx, 0, INTRO_CRAFT_VISIBLE_CHASE_RATE);
        achase_angle(
            &mut craft.rotz,
            direction.roll_target(),
            INTRO_CRAFT_ROLL_CHASE_RATE,
        );
    }

    player_down_shared_motion(game, object);
}

pub fn player_down_left_intro_init(game: &mut Game, object: u16) {
    player_down_wing_init(game, object, WingDirection::Left);
}

pub fn player_down_left_intro_tick(game: &mut Game, object: u16) {
    player_down_wing_tick(game, object, WingDirection::Left);
}

pub fn player_down_right_intro_init(game: &mut Game, object: u16) {
    player_down_wing_init(game, object, WingDirection::Right);
}

pub fn player_down_right_intro_tick(game: &mut Game, object: u16) {
    player_down_wing_tick(game, object, WingDirection::Right);
}

/// `playerfireintro_Istrat`: initialize the invisible paired-laser controller.
pub fn player_fire_intro_init(game: &mut Game, object: u16) {
    let tick = sid(game, player_fire_intro_tick as StrategyFn);
    {
        let controller = &mut game.objs.aliens[object as usize];
        controller.stratptr = Some(tick);
        controller.type_ &= !ATZREMOVE;
        controller.count = LASER_CONTROLLER_LIFETIME;
    }
    player_fire_intro_tick(game, object);
}

pub fn player_fire_intro_tick(game: &mut Game, object: u16) {
    if decrement_lifetime(game, object) {
        return;
    }

    let phase = object & EVERY_FOURTH_FRAME_MASK;
    if game.vars.gameframe.wrapping_add(phase) & EVERY_FOURTH_FRAME_MASK == 0 {
        let target_ref = game.objs.aliens[object as usize].sword1 as u16;
        if let Some(target) = strat_obj_from_ptr(target_ref) {
            let source = game.objs.aliens[object as usize];
            let target_state = game.objs.aliens[target as usize];
            let pitch = strat_pitch_toward(&source, &target_state);
            let yaw = sf_core::aim_angle::yanglexy(
                target_state.worldx.wrapping_sub(source.worldx),
                target_state.worldz.wrapping_sub(source.worldz),
            );
            for (muzzle_x, spread_x) in [
                (LASER_MUZZLE_X.wrapping_neg(), -LASER_SPREAD_X),
                (LASER_MUZZLE_X, LASER_SPREAD_X),
            ] {
                if let Some(laser) =
                    fire_relfastelaser_weapon_pos(game, object, pitch, yaw, muzzle_x, 0, 0)
                {
                    let laser = &mut game.objs.aliens[laser as usize];
                    laser.type_ &= !ATZREMOVE;
                    laser.count = LASER_LIFETIME;
                    laser.worldx = laser.worldx.wrapping_add(spread_x);
                }
            }
        }
    }
    add_player_z(game, object);
}

/// `zacointro_Istrat`: initialize one tumbling fighter and execute its first
/// animation/movement tick.
pub fn zaco_intro_init(game: &mut Game, object: u16) {
    let pitch = (sf_random(&mut game.vars) as u8 & ZACO_INITIAL_TILT_MASK)
        .wrapping_add(ZACO_INITIAL_TILT_BASE);
    let yaw = sf_random(&mut game.vars) as u8;
    let roll = sf_random(&mut game.vars) as u8;
    let tick = sid(game, zaco_intro_tick as StrategyFn);
    {
        let fighter = &mut game.objs.aliens[object as usize];
        fighter.stratptr = Some(tick);
        fighter.sflags |= ASF_COLLDISABLE;
        fighter.rotx = pitch;
        fighter.roty = yaw;
        fighter.rotz = roll;
        fighter.sbyte1 = ZACO_TURN_DELAY;
        fighter.vel = ZACO_INITIAL_SPEED;
        fighter.count = ZACO_LIFETIME;
        fighter.snd2 = ZACO_SOUND;
        init_animation(fighter);
    }
    zaco_intro_tick(game, object);
}

pub fn zaco_intro_tick(game: &mut Game, object: u16) {
    advance_animation(&mut game.objs.aliens[object as usize]);
    if decrement_lifetime(game, object) {
        return;
    }

    let turn_delay = game.objs.aliens[object as usize].sbyte1.wrapping_sub(1);
    game.objs.aliens[object as usize].sbyte1 = turn_delay;
    if turn_delay == 0 {
        let fighter = &mut game.objs.aliens[object as usize];
        fighter.sbyte1 = 1;
        achase_angle(&mut fighter.roty, 0, INTRO_CRAFT_ROLL_CHASE_RATE);
        achase_angle(&mut fighter.rotx, 0, INTRO_CRAFT_ROLL_CHASE_RATE);
        achase_angle(&mut fighter.rotz, 0, INTRO_CRAFT_VISIBLE_CHASE_RATE);
        speed_to(fighter, ZACO_TARGET_SPEED, ZACO_ACCELERATION);
    } else {
        game.objs.aliens[object as usize].rotz = game.objs.aliens[object as usize]
            .rotz
            .wrapping_add(ZACO_ROLL_SPEED);
    }
    {
        let fighter = &mut game.objs.aliens[object as usize];
        gen_vecs_3d(fighter);
        apply_velocity(fighter);
    }
    add_player_z(game, object);
}

/// `zaco2intro_Istrat`: initialize the lead fighter and execute its first
/// source tick.
pub fn zaco_leader_intro_init(game: &mut Game, object: u16) {
    let tick = sid(game, zaco_leader_intro_tick as StrategyFn);
    {
        let leader = &mut game.objs.aliens[object as usize];
        leader.stratptr = Some(tick);
        leader.sflags |= ASF_COLLDISABLE;
        leader.vel = LEADER_INITIAL_SPEED;
        leader.rotx = DEG90;
        leader.sbyte1 = LEADER_FIRST_PHASE_FRAMES;
        leader.sbyte2 = LEADER_EXIT_DELAY;
        leader.snd2 = ZACO_SOUND;
        leader.stratstate = LeaderPhase::InitialRoll as u8;
        init_animation(leader);
    }
    zaco_leader_intro_tick(game, object);
}

pub fn zaco_leader_intro_tick(game: &mut Game, object: u16) {
    advance_animation(&mut game.objs.aliens[object as usize]);

    let mut phase = LeaderPhase::from_state(game.objs.aliens[object as usize].stratstate);
    if phase == LeaderPhase::InitialRoll {
        let leader = &mut game.objs.aliens[object as usize];
        leader.rotz = leader.rotz.wrapping_add(ZACO_ROLL_SPEED);
        leader.sbyte1 = leader.sbyte1.wrapping_sub(1);
        if leader.sbyte1 == 0 {
            leader.stratstate = LeaderPhase::Tumble as u8;
            leader.sbyte1 = LEADER_SECOND_PHASE_FRAMES;
            phase = LeaderPhase::Tumble;
        }
    }

    if phase == LeaderPhase::Tumble {
        let leader = &mut game.objs.aliens[object as usize];
        leader.rotx = leader.rotx.wrapping_add(LEADER_TUMBLE_PITCH);
        leader.roty = leader.roty.wrapping_add(LEADER_TUMBLE_YAW);
        leader.rotz = leader.rotz.wrapping_add(ZACO_ROLL_SPEED);
        leader.sbyte1 = leader.sbyte1.wrapping_sub(1);
        if leader.sbyte1 == 0 {
            leader.stratstate = LeaderPhase::Attack as u8;
            leader.sbyte1 = LEADER_FINAL_PHASE_FRAMES;
            phase = LeaderPhase::Attack;
        }
    }

    if phase == LeaderPhase::Attack {
        game.objs.aliens[object as usize].rotz = game.objs.aliens[object as usize]
            .rotz
            .wrapping_add(ZACO_ROLL_SPEED);
        if let Some(player_state) = player(game) {
            let source = game.objs.aliens[object as usize];
            let depth_distance = (source.worldz as i32 - player_state.worldz as i32).abs();
            if (LEADER_FIRE_MIN_DISTANCE..LEADER_FIRE_MAX_DISTANCE).contains(&depth_distance)
                && SmokeCadence::EveryFourthFrame.is_due(game.vars.gameframe)
            {
                let pitch_spread = (sf_random(&mut game.vars) as u8 & LEADER_FIRE_SPREAD_MASK)
                    .wrapping_sub(LEADER_FIRE_SPREAD_MASK / 2);
                let yaw_spread = (sf_random(&mut game.vars) as u8 & LEADER_FIRE_SPREAD_MASK)
                    .wrapping_sub(LEADER_FIRE_SPREAD_MASK / 2);
                let pitch = strat_pitch_toward(&source, &player_state).wrapping_add(pitch_spread);
                let yaw = sf_core::aim_angle::yanglexy(
                    player_state.worldx.wrapping_sub(source.worldx),
                    player_state.worldz.wrapping_sub(source.worldz),
                )
                .wrapping_add(yaw_spread);
                let _ = fire_relfastelaser_weapon_pos(game, object, pitch, yaw, 0, 0, 0);
            }
            if depth_distance < LEADER_AIM_DISTANCE {
                strat_aim_3d(game, object, &player_state, LEADER_AIM_RATE);
            }
        }
    }

    {
        let leader = &mut game.objs.aliens[object as usize];
        gen_vecs_3d(leader);
        apply_velocity(leader);
    }
    add_player_z(game, object);

    if let Some(player_state) = player(game) {
        if game.objs.aliens[object as usize].worldz < player_state.worldz {
            game.vars.strategy.intro_exit_requested = true;
        }
    }
}
