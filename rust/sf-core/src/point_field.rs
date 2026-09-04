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
    /// Presentation correspondence only; never used by the source raster or
    /// simulation. Clipping can change the output list's indices each tick.
    pub identity: PointIdentity,
}

/// Stable identity of a projected point, including the second pixel of a
/// near point. Grid cells wrap with the source world's coordinate range;
/// respawned dust gets a new lifetime instead of streaking across the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PointIdentity {
    #[default]
    Untracked,
    Ground {
        column: u8,
        row: u8,
        lower: bool,
    },
    Dust {
        slot: u8,
        generation: u64,
        lower: bool,
    },
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
