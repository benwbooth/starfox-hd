//! Deterministic controller tapes used by whole-scenario SF1 oracle runs.
//!
//! These are input-only: applying a tape never writes game state, changes
//! durability, forces a kill, or synchronizes the native port to the oracle.

use sf_core::pad;

pub const CORNERIA_INPUT_SEGMENT_FRAMES: u16 = 30;
pub const CORNERIA_ATTACK_CARRIER_FRAME: u16 = 2_177;
pub const CORNERIA_ATTACK_CARRIER_SHAPE: u16 = 55;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const FRONT_END_LAST_CONFIRM_TICK: u32 = 360;
const GAME_DESTINATION_SELECT_TICK: u32 = 380;
const GAME_DESTINATION_CONFIRM_TICK: u32 = 420;
const ROUTE_SELECTION_CONFIRM_TICK: u32 = 500;
const ROUTE_SELECTION_CONFIRM_HOLD_TICKS: u32 = 12;
const PLANET_DISMISS_START_TICK: u32 = 840;
const PLANET_DISMISS_END_TICK: u32 = 900;
const PLANET_DISMISS_CADENCE_TICKS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PilotAction {
    Center,
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

impl PilotAction {
    pub const ALL: [Self; 9] = [
        Self::Center,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::UpLeft,
        Self::UpRight,
        Self::DownLeft,
        Self::DownRight,
    ];

    pub const fn pad_bits(self) -> u16 {
        match self {
            Self::Center => 0,
            Self::Up => pad::UP,
            Self::Down => pad::DOWN,
            Self::Left => pad::LEFT,
            Self::Right => pad::RIGHT,
            Self::UpLeft => pad::UP | pad::LEFT,
            Self::UpRight => pad::UP | pad::RIGHT,
            Self::DownLeft => pad::DOWN | pad::LEFT,
            Self::DownRight => pad::DOWN | pad::RIGHT,
        }
    }
}

/// Controller-only route found by the deterministic search probe. The native
/// game reaches the real Corneria Attack Carrier with one body-durability
/// point remaining. Retail equivalence is established separately by the
/// paired semantic trace; this tape is input, not a native-state fixture.
pub const CORNERIA_ATTACK_CARRIER_TAPE: [PilotAction; 120] = [
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownLeft,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::Down,
    PilotAction::DownLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::Up,
    PilotAction::Left,
    PilotAction::Right,
    PilotAction::UpLeft,
    PilotAction::DownRight,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::Center,
    PilotAction::DownLeft,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::UpRight,
    PilotAction::UpLeft,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::Down,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpLeft,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::UpRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownRight,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
    PilotAction::DownLeft,
];

pub fn corneria_attack_carrier_input(game_frame: u16) -> u16 {
    CORNERIA_ATTACK_CARRIER_TAPE
        .get(usize::from(game_frame / CORNERIA_INPUT_SEGMENT_FRAMES))
        .map_or(0, |action| action.pad_bits())
}

pub fn corneria_front_end_input(tick: u32) -> u16 {
    if (GAME_DESTINATION_SELECT_TICK..GAME_DESTINATION_SELECT_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::DOWN;
    }
    if (GAME_DESTINATION_CONFIRM_TICK..GAME_DESTINATION_CONFIRM_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if tick <= FRONT_END_LAST_CONFIRM_TICK
        && tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS
    {
        return pad::START;
    }
    if (ROUTE_SELECTION_CONFIRM_TICK
        ..ROUTE_SELECTION_CONFIRM_TICK + ROUTE_SELECTION_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if (PLANET_DISMISS_START_TICK..PLANET_DISMISS_END_TICK).contains(&tick) {
        return if (tick - PLANET_DISMISS_START_TICK) % PLANET_DISMISS_CADENCE_TICKS == 0 {
            pad::B
        } else {
            0
        };
    }
    0
}
