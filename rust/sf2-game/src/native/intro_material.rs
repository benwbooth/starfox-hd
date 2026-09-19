//! Flat polygon material selection after animation/texture dispatch.
//! Palette bytes retain both source dithering nibbles; they are not RGB.

use super::intro_transform::face_shade_index;
use sf2_data::lighting::{SHADE_PAIRS, STANDARD_DEPTH_PAIRS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthGroup {
    Near,
    Middle,
    Far,
    Farthest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlatColor {
    Solid(u8),
    Depth(u8),
    Lit(u8),
}

/// A validated, non-animated flat material from the source catalog.
/// Texture, smooth, animation and out-of-catalog table rows are not flat
/// materials and must be dispatched before this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatMaterial(FlatColor);

impl FlatMaterial {
    pub fn from_word(word: u16) -> Option<Self> {
        match word >> 8 {
            0x3F => Some(Self(FlatColor::Solid(word as u8))),
            0x3E if (word as u8) < 32 => Some(Self(FlatColor::Depth(word as u8))),
            row @ 0..=11 => Some(Self(FlatColor::Lit(row as u8))),
            _ => None,
        }
    }

    /// Original `$01:9E85..9EFC` flat-color branches, with the standard
    /// depth-color family. Depth-group selection is
    /// a separate caller responsibility, not a distance approximation here.
    pub fn palette_pair(
        self,
        group: DepthGroup,
        lighting_enabled: bool,
        normal: [i8; 3],
        object_light: [i8; 3],
    ) -> u8 {
        let bank = match group {
            DepthGroup::Near => 0,
            DepthGroup::Middle => 1,
            DepthGroup::Far => 2,
            DepthGroup::Farthest => 3,
        };
        match self.0 {
            FlatColor::Solid(pair) => pair,
            FlatColor::Depth(index) => STANDARD_DEPTH_PAIRS[bank][usize::from(index)],
            // The original has already replaced r3 with the decoded row
            // before testing the lighting flag. It does not use the word's
            // low byte or a default shade when lighting is disabled.
            FlatColor::Lit(row) if !lighting_enabled => row,
            FlatColor::Lit(row) => {
                let shade = face_shade_index(normal, object_light);
                // The twelve-entry source pointer catalog aliases its final
                // two pointers to the tenth row; this is not an error clamp.
                SHADE_PAIRS[bank][usize::from(row.min(9))][usize::from(shade)]
            }
        }
    }
}
