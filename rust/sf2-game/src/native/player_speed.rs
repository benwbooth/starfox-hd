//! Flight speed and signed thrust ($06:F05D..F1FB). The preceding surface
//! service supplies inherited targets; base speed and thrust ease differently.

use super::path_fields::{chase_byte, chase_word};
use super::player_surface::SurfaceResponse;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore};

const PILOT_COUNT: u8 = 6;
const MAIN_SPEED: [u8; 6] = [40, 42, 32, 30, 48, 50];
const ALTERNATE_SPEED: [u8; 6] = [30, 32, 24, 23, 38, 38];
const BOOST_THRUST: [i8; 6] = [83, 85, 67, 65, 100, 98];
const SPECIAL_BOOST_THRUST: [i8; 6] = [40, 42, 32, 30, 50, 48];
const BRAKE_THRUST: [i8; 6] = [-32, -32, -28, -26, -41, -43];
const TURNING_SPEED: [u8; 6] = [8, 8, 6, 6, 11, 11];
const ALTERNATE_BOOST_THRUST: [i8; 6] = [105, 107, 92, 90, 125, 123];
const BOOST: u8 = 0x40;
const BRAKE: u8 = 0x20;
const HEADING_LOCKED: u8 = 0x04;
const SUBMODE_MASK: u8 = 0x0F;
const MAIN_SUBMODE: u8 = 1;
const SPECIAL_CONFIGURATION: u8 = 9;
const GATED_SPEED: u8 = 38;
const TURNING_BUTTONS: u16 = Button::Left as u16 | Button::Right as u16;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSpeed {
    /// 6B62 is a signed byte, not a word overlapping charge's linked flags.
    pub thrust: i8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpeedContext {
    pub thrust_target: Option<i8>,
    pub alternate_thrust_target: Option<i8>,
}

impl From<SurfaceResponse> for SpeedContext {
    fn from(response: SurfaceResponse) -> Self {
        Self {
            thrust_target: Some(response.effect_events as i8),
            alternate_thrust_target: response.alternate_thrust_target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedError {
    World(WorldInputError),
    MissingVisit(ObjectId),
    MissingSpeed(ObjectId),
    MissingProcessedInput,
    MissingThrustTarget,
    MissingAlternateThrustTarget,
}

impl From<WorldInputError> for SpeedError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn ease_speed(objects: &mut ObjectStore, owner: ObjectId, target: u8) -> Result<(), SpeedError> {
    let speed = &mut objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .speed;
    *speed = chase_byte(*speed, target);
    Ok(())
}

fn ease_thrust(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    target: Option<i8>,
) -> Result<(), SpeedError> {
    let speed = world
        .player_mut(objects, owner)?
        .speed
        .as_mut()
        .ok_or(SpeedError::MissingSpeed(owner))?;
    let target = target.ok_or(SpeedError::MissingThrustTarget)?;
    // Sign extension precedes subtraction: unlike base speed this does not
    // take a wrapping shortcut between opposite signed-byte extremes.
    speed.thrust = chase_word(i16::from(speed.thrust) as u16, i16::from(target) as u16) as i8;
    Ok(())
}

fn turn_speed(current: u8, target: u8) -> u8 {
    let delta = current.wrapping_sub(target) as i8;
    if delta == 0 {
        current
    } else if delta < 0 {
        let next = current.wrapping_add(1);
        if (next.wrapping_sub(target) as i8) < 0 {
            next
        } else {
            target
        }
    } else {
        let next = current.wrapping_sub(1);
        if (next.wrapping_sub(target) as i8) >= 0 {
            next
        } else {
            target
        }
    }
}

/// Both forced-speed gates skip pilot, throttle, configuration and input.
/// Turning directions have a separate odd-visit speed step; even visits keep
/// the inherited primary thrust target and do not read the movement submode.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    mut context: SpeedContext,
) -> Result<(), SpeedError> {
    if world
        .action_gate
        .ok_or(WorldInputError::MissingActionGate)?
        .code
        != 0
    {
        ease_speed(objects, owner, GATED_SPEED)?;
        return ease_thrust(objects, world, owner, Some(0));
    }
    if world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts
    {
        ease_speed(objects, owner, 0)?;
        return ease_thrust(objects, world, owner, Some(0));
    }
    let record = world.player(objects, owner)?;
    let pilot = record
        .visit
        .ok_or(SpeedError::MissingVisit(owner))?
        .pilot_code;
    let pilot = usize::from(if pilot < PILOT_COUNT { pilot } else { 0 });
    let auxiliary = record
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    if auxiliary.action_flags & BOOST != 0 {
        context.alternate_thrust_target = Some(ALTERNATE_BOOST_THRUST[pilot]);
        context.thrust_target = Some(
            if world
                .scene
                .player_configuration
                .ok_or(WorldInputError::MissingPlayerConfiguration)?
                == SPECIAL_CONFIGURATION
            {
                SPECIAL_BOOST_THRUST[pilot]
            } else {
                BOOST_THRUST[pilot]
            },
        );
    } else if auxiliary.action_flags & BRAKE != 0 {
        context.thrust_target = Some(BRAKE_THRUST[pilot]);
        context.alternate_thrust_target = context.thrust_target;
    } else if auxiliary.action_flags & HEADING_LOCKED == 0 {
        let input = world
            .processed_player_input
            .ok_or(SpeedError::MissingProcessedInput)?;
        if input.held.bits() & TURNING_BUTTONS != 0 {
            if world.strategy_clock & 1 != 0 {
                let speed = &mut objects
                    .get_mut(owner)
                    .ok_or(WorldInputError::MissingActor(owner))?
                    .base
                    .speed;
                *speed = turn_speed(*speed, TURNING_SPEED[pilot]);
                context.thrust_target = Some(0);
            }
            return ease_thrust(objects, world, owner, context.thrust_target);
        }
        context.thrust_target = Some(0);
        context.alternate_thrust_target = Some(0);
    }
    let target = if auxiliary.mode & SUBMODE_MASK == MAIN_SUBMODE {
        MAIN_SPEED[pilot]
    } else {
        // The alternate value is copied before the base-speed write.
        context.thrust_target = Some(
            context
                .alternate_thrust_target
                .ok_or(SpeedError::MissingAlternateThrustTarget)?,
        );
        ALTERNATE_SPEED[pilot]
    };
    ease_speed(objects, owner, target)?;
    ease_thrust(objects, world, owner, context.thrust_target)
}

#[cfg(test)]
#[path = "player_speed_tests.rs"]
mod tests;
