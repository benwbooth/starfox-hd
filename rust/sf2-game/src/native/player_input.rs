//! Processed player input (`$06:9457..9585`). Controller edges belong to the
//! display sampler; this service selects a controller and remaps those edges
//! without resampling them. Player modes own its position in their visit.

use super::hit_response::HitSide;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, Buttons, InputState, ObjectId, ObjectStore};

const MOVEMENT_FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const INVERT_FLIGHT_VERTICAL: u8 = 0x80;
const PLAYER_ACTIVE: u8 = 0x01;

/// Both original option bytes survive unchanged. Only flight_style's high
/// bit is read here; every nonzero button_layout selects the alternate map.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerInputSettings {
    pub flight_style: u8,
    pub button_layout: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerInputError {
    World(WorldInputError),
    MissingScriptedViewMode,
    MissingController(HitSide),
    MissingSettings,
    MissingInjectedInput(ObjectId),
}

impl From<WorldInputError> for PlayerInputError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn map_words(input: InputState, map: impl Fn(u16) -> u16) -> InputState {
    InputState {
        held: Buttons::from_bits(map(input.held.bits())),
        pressed: Buttons::from_bits(map(input.pressed.bits())),
    }
}

fn invert_vertical(buttons: u16) -> u16 {
    let up = Button::Up as u16;
    let down = Button::Down as u16;
    (buttons & !(up | down)) | ((buttons & up) >> 1) | ((buttons & down) << 1)
}

fn alternate_buttons(buttons: u16) -> u16 {
    let a = Button::A as u16;
    let b = Button::B as u16;
    let x = Button::X as u16;
    let y = Button::Y as u16;
    (buttons & !(a | b | x | y))
        | ((buttons & a) >> 1)
        | ((buttons & b) >> 8)
        | ((buttons & x) << 8)
        | ((buttons & y) << 1)
}

/// Clear the shared processed words before either gate. On a gated visit,
/// retain the previous pre-mask publication and the queued script input.
/// On an admitted visit, publish the remapped controller words first, then
/// apply the activity mask and consume the caller's one-shot injected input.
/// Partial writes survive diagnostics, so the scene wrapper latches failure.
pub fn prepare(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<InputState, PlayerInputError> {
    world.processed_player_input = Some(InputState::default());
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    if world
        .scripted_view_active()
        .ok_or(PlayerInputError::MissingScriptedViewMode)?
    {
        return Ok(InputState::default());
    }
    if world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection
    {
        return Ok(InputState::default());
    }

    // The actor's existing side flag is also used by contact attribution and
    // view/audio routing. Neither primary selection nor path selection owns it.
    let side = actor.base.contacts.hit_side;
    let controller = match side {
        HitSide::Primary => 0,
        HitSide::Secondary => 1,
    };
    let mut processed =
        world.controller_inputs[controller].ok_or(PlayerInputError::MissingController(side))?;
    world.processed_player_input = Some(processed);
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let settings = world
        .player_input_settings
        .ok_or(PlayerInputError::MissingSettings)?;
    if auxiliary.mode & MOVEMENT_FAMILY_MASK == FLIGHT_FAMILY
        && settings.flight_style & INVERT_FLIGHT_VERTICAL != 0
    {
        processed = map_words(processed, invert_vertical);
        world.processed_player_input = Some(processed);
    }
    if settings.button_layout != 0 {
        processed = map_words(processed, alternate_buttons);
        world.processed_player_input = Some(processed);
    }
    world.unmasked_player_input = Some(processed);
    if auxiliary.action_flags & PLAYER_ACTIVE == 0 {
        processed = map_words(processed, |word| word & Button::Start as u16);
        world.processed_player_input = Some(processed);
    }
    let injected = world
        .player(objects, owner)?
        .injected_input
        .ok_or(PlayerInputError::MissingInjectedInput(owner))?;
    processed.pressed = Buttons::from_bits(processed.pressed.bits() | injected.pressed.bits());
    processed.held = Buttons::from_bits(processed.held.bits() | injected.held.bits());
    world.processed_player_input = Some(processed);
    world.player_mut(objects, owner)?.injected_input = Some(InputState::default());
    Ok(processed)
}

#[cfg(test)]
#[path = "player_input_tests.rs"]
mod tests;
