//! Typed Star Fox controller-screen state and logical input layouts.
//!
//! `CONT.ASM` exposes four controller types. The source input routine keeps
//! the directional/face-button meaning in logical game controls by swapping
//! the physical B/Y pair, the physical Up/Down pair, or both. These semantic
//! values are shared by the game shell and renderer; no source-machine state
//! crosses that boundary.

use crate::pad;

/// The four controller layouts shown by the source controller screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ControlType {
    #[default]
    A,
    B,
    C,
    D,
}

impl ControlType {
    /// SELECT advances through the four source layouts and wraps to A.
    pub const fn next(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::C,
            Self::C => Self::D,
            Self::D => Self::A,
        }
    }

    /// Column of this layout in the source 2-by-2 controller tilemap.
    pub const fn panel_column(self) -> usize {
        match self {
            Self::A | Self::C => 0,
            Self::B | Self::D => 1,
        }
    }

    /// Row of this layout in the source 2-by-2 controller tilemap.
    pub const fn panel_row(self) -> usize {
        match self {
            Self::A | Self::B => 0,
            Self::C | Self::D => 1,
        }
    }

    /// Convert physical SNES buttons into the source layout's logical input.
    pub const fn map_pad(self, physical: u16) -> u16 {
        let mut logical = physical;
        if matches!(self, Self::B | Self::D) {
            logical = swap_buttons(logical, pad::B, pad::Y);
        }
        if matches!(self, Self::C | Self::D) {
            logical = swap_buttons(logical, pad::UP, pad::DOWN);
        }
        logical
    }
}

const fn swap_buttons(value: u16, first: u16, second: u16) -> u16 {
    let first_set = value & first != 0;
    let second_set = value & second != 0;
    let cleared = value & !(first | second);
    cleared | if first_set { second } else { 0 } | if second_set { first } else { 0 }
}

/// Source controller screen interaction phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BriefingPhase {
    /// Choose one of the four controller layouts.
    #[default]
    ControlType,
    /// Choose TRAINING or GAME.
    Destination,
}

/// Destination selected on the controller screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BriefingChoice {
    #[default]
    Training,
    Game,
}

impl BriefingChoice {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Training => Self::Game,
            Self::Game => Self::Training,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_type_cycle_matches_the_four_source_panels() {
        assert_eq!(ControlType::A.next(), ControlType::B);
        assert_eq!(ControlType::B.next(), ControlType::C);
        assert_eq!(ControlType::C.next(), ControlType::D);
        assert_eq!(ControlType::D.next(), ControlType::A);
        assert_eq!(
            [
                ControlType::A.panel_column(),
                ControlType::B.panel_column(),
                ControlType::C.panel_column(),
                ControlType::D.panel_column(),
            ],
            [0, 1, 0, 1]
        );
        assert_eq!(
            [
                ControlType::A.panel_row(),
                ControlType::B.panel_row(),
                ControlType::C.panel_row(),
                ControlType::D.panel_row(),
            ],
            [0, 0, 1, 1]
        );
    }

    #[test]
    fn controller_types_remap_only_the_authored_pairs() {
        let unchanged = pad::A | pad::X | pad::LEFT | pad::START;
        assert_eq!(ControlType::A.map_pad(unchanged), unchanged);
        assert_eq!(
            ControlType::B.map_pad(unchanged | pad::B),
            unchanged | pad::Y
        );
        assert_eq!(
            ControlType::C.map_pad(unchanged | pad::UP),
            unchanged | pad::DOWN
        );
        assert_eq!(
            ControlType::D.map_pad(unchanged | pad::B | pad::UP),
            unchanged | pad::Y | pad::DOWN
        );
    }
}
