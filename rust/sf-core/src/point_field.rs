//! Typed background point-field declarations shared by game state and rendering.

/// Source-authored point field selected by a background declaration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PointFieldMode {
    /// No projected point field.
    #[default]
    None,
    /// Ground-plane grid used by planetary stages and Training.
    GroundGrid,
    /// Moving depth points used by space backgrounds.
    SpaceDust,
    /// Space-point projection with the Fortuna snow presentation.
    Snow,
    /// Space-point projection with the Venom pollen presentation.
    Pollen,
}

/// One source-resolution point emitted by a projected background field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointPixel {
    pub x: u8,
    pub y: u8,
    pub palette_index: u8,
}

impl PointFieldMode {
    /// Compatibility value used by the translated strategy state.
    pub const fn source_flag(self) -> i16 {
        match self {
            Self::None => 0,
            Self::GroundGrid => 1,
            Self::SpaceDust | Self::Snow | Self::Pollen => -1,
        }
    }
}
