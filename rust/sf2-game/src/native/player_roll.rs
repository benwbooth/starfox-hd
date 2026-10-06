//! Shoulder arbitration ($06:9075..90D3) and double-tap barrel-roll control
//! ($06:E7FF..E8F4). These are separate calls in the player mode: steering
//! runs between them. The roll publishes the existing contact-protection
//! bit before changing its impulse, not a second collision-owned flag.

use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, InputState, ObjectId, ObjectStore};

const SHOULDER_BUTTON_MASK: u8 = 0x30;
const MOST_RECENT_RIGHT: u8 = 0x80;
const SELECTED_RIGHT: u8 = 0x20;
const SELECTED_LEFT: u8 = 0x40;
const ROLL_STARTED: u8 = 0x10;
const TAP_AGE_MASK: u8 = 0x0F;
const TAP_METADATA_MASK: u8 = !TAP_AGE_MASK;
const DOUBLE_TAP_LIMIT: u8 = 7;
const INITIAL_ROLL_IMPULSE: i8 = 32;
const ROLL_IMPULSE_DECAY: i8 = 2;

/// Retained shoulder control (6B7E). Preserve unrelated low bits and the
/// last-edge preference even when neither shoulder remains held.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShoulderControl(u8);

impl ShoulderControl {
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn left_selected(self) -> bool {
        self.0 & SELECTED_LEFT != 0
    }

    pub const fn right_selected(self) -> bool {
        self.0 & SELECTED_RIGHT != 0
    }

    pub fn update(&mut self, input: InputState) {
        // Right wins simultaneous edges; held-both follows that retained
        // choice. Releasing a button does not manufacture a new edge.
        if input.pressed.contains(Button::RightShoulder) {
            self.0 |= MOST_RECENT_RIGHT;
        } else if input.pressed.contains(Button::LeftShoulder) {
            self.0 &= !MOST_RECENT_RIGHT;
        }
        self.0 &= !(SELECTED_LEFT | SELECTED_RIGHT);
        let held = input.held.bits() as u8 & SHOULDER_BUTTON_MASK;
        self.0 |= match held {
            0 => 0,
            SHOULDER_BUTTON_MASK if self.0 & MOST_RECENT_RIGHT != 0 => SELECTED_RIGHT,
            SHOULDER_BUTTON_MASK => SELECTED_LEFT,
            value if value & Button::LeftShoulder as u8 != 0 => SELECTED_LEFT,
            _ => SELECTED_RIGHT,
        };
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerRoll {
    pub shoulders: ShoulderControl,
    /// 6ADC: low nibble is a saturating visit count; bits 4/5 remember
    /// the last tap, while the other two upper bits survive every update.
    pub tap_window: u8,
    /// 6ADD: signed bank contribution. Original decay wraps rather than
    /// clamps, including alternating +/-1 if an odd value was supplied.
    pub impulse: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollError {
    World(WorldInputError),
    MissingRoll(ObjectId),
    MissingProcessedInput,
}

impl From<WorldInputError> for RollError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Called immediately after processed input, not automatically by its
/// producer. All later steering and double-tap decisions share this state.
pub fn prepare_shoulders(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), RollError> {
    let mut roll = world
        .player(objects, owner)?
        .roll
        .ok_or(RollError::MissingRoll(owner))?;
    let input = world
        .processed_player_input
        .ok_or(RollError::MissingProcessedInput)?;
    roll.shoulders.update(input);
    world.player_mut(objects, owner)?.roll = Some(roll);
    Ok(())
}

/// Publish protection from the entering impulse, then advance the roll.
/// Starting a roll does not protect its first visit; its final decay visit
/// remains protected. An active roll never reads input or action flags.
/// Earlier writes survive missing later owners, matching the source prefix.
pub fn advance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), RollError> {
    let records = world.player_mut(objects, owner)?;
    let mut roll = records.roll.ok_or(RollError::MissingRoll(owner))?;
    records
        .protection
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerProtection(owner))?
        .set_projectile_deflection(roll.impulse != 0);
    if roll.impulse != 0 {
        roll.impulse = if roll.impulse < 0 {
            roll.impulse.wrapping_add(ROLL_IMPULSE_DECAY)
        } else {
            roll.impulse.wrapping_sub(ROLL_IMPULSE_DECAY)
        };
        roll.tap_window = (roll.tap_window & TAP_METADATA_MASK) | TAP_AGE_MASK;
        records.roll = Some(roll);
        return Ok(());
    }
    records
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags &= !ROLL_STARTED;
    let input = world
        .processed_player_input
        .ok_or(RollError::MissingProcessedInput)?;
    let pressed = input.pressed.bits() as u8 & SHOULDER_BUTTON_MASK;
    let held = input.held.bits() as u8 & SHOULDER_BUTTON_MASK;
    if pressed != 0 && held != SHOULDER_BUTTON_MASK {
        if roll.tap_window & SHOULDER_BUTTON_MASK == pressed
            && roll.tap_window & TAP_AGE_MASK < DOUBLE_TAP_LIMIT
        {
            roll.impulse = if roll.shoulders.left_selected() {
                INITIAL_ROLL_IMPULSE
            } else {
                -INITIAL_ROLL_IMPULSE
            };
            world
                .player_mut(objects, owner)?
                .auxiliary
                .as_mut()
                .expect("validated auxiliary owner")
                .action_flags |= ROLL_STARTED;
            roll.tap_window &= TAP_METADATA_MASK;
        } else {
            roll.tap_window =
                ((roll.tap_window & !SHOULDER_BUTTON_MASK) | pressed) & TAP_METADATA_MASK;
            world.player_mut(objects, owner)?.roll = Some(roll);
            return Ok(());
        }
    }
    if roll.tap_window & TAP_AGE_MASK != TAP_AGE_MASK {
        roll.tap_window = roll.tap_window.wrapping_add(1);
    }
    world.player_mut(objects, owner)?.roll = Some(roll);
    Ok(())
}

#[cfg(test)]
#[path = "player_roll_tests.rs"]
mod tests;
