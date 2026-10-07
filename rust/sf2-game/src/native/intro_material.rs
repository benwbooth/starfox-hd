//! Flat polygon material selection after animation/texture dispatch.
//! Palette bytes retain both source dithering nibbles; they are not RGB.

use super::intro_transform::face_shade_index;
use sf2_data::lighting::{DEPTH_PAIRS, DEPTH_THRESHOLDS, SHADE_PAIRS};

/// A decoded depth-colour family, not an address in the source palette bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthColorFamily(usize);

impl DepthColorFamily {
    pub const STANDARD: Self = Self(0);
    pub const SURFACE_UNCARRIED: Self = Self(1);

    pub fn from_catalog_index(index: usize) -> Option<Self> {
        (index < DEPTH_PAIRS.len()).then_some(Self(index))
    }

    pub const fn catalog_index(self) -> usize {
        self.0
    }
}

/// The three signed high-byte thresholds in one authored depth record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthThresholdTable(usize);

impl DepthThresholdTable {
    pub const NORMAL: Self = Self(4);
    pub const OPENING: Self = Self(9);
    pub const SURFACE_UNCARRIED: Self = Self(7);
    pub const SURFACE_CARRIED: Self = Self(8);

    pub fn from_catalog_index(index: usize) -> Option<Self> {
        (index < DEPTH_THRESHOLDS.len()).then_some(Self(index))
    }

    pub const fn catalog_index(self) -> usize {
        self.0
    }

    pub const fn thresholds(self) -> [i8; 3] {
        DEPTH_THRESHOLDS[self.0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneLightingError {
    MissingSceneThresholds,
    UnknownObjectThreshold(u8),
    MissingDepthColors,
}

/// Scene-owned lighting selections. Boot has not published either table;
/// mode setup changes only thresholds, while the main loader's final handoff
/// installs both. Loading a palette or an object's material does not select
/// these tables implicitly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneLighting {
    pub thresholds: Option<DepthThresholdTable>,
    pub depth_colors: Option<DepthColorFamily>,
}

impl SceneLighting {
    pub fn setup_scene(&mut self) {
        self.thresholds = Some(DepthThresholdTable::NORMAL);
    }

    pub fn finish_opening_load(&mut self) {
        self.thresholds = Some(DepthThresholdTable::OPENING);
        self.depth_colors = Some(DepthColorFamily::STANDARD);
    }

    /// Zero inherits the scene's table; all nonzero selectors are one-based.
    /// The submitted low byte is authoritative. Unreviewed selectors are a
    /// diagnostic, never a clamp to the nearest supported table.
    pub fn depth_group(
        self,
        camera_depth: i16,
        object_selector: u8,
    ) -> Result<DepthGroup, SceneLightingError> {
        let thresholds = if object_selector == 0 {
            self.thresholds
                .ok_or(SceneLightingError::MissingSceneThresholds)?
        } else {
            DepthThresholdTable::from_catalog_index(usize::from(object_selector - 1))
                .ok_or(SceneLightingError::UnknownObjectThreshold(object_selector))?
        };
        Ok(DepthGroup::for_camera_depth(
            camera_depth,
            thresholds.thresholds(),
        ))
    }

    pub fn palette_pair(
        self,
        material: FlatMaterial,
        group: DepthGroup,
        lighting_enabled: bool,
        normal: [i8; 3],
        object_light: [i8; 3],
    ) -> Result<u8, SceneLightingError> {
        // Solid and face-lit materials never read the depth-colour family.
        let family = if matches!(material.0, FlatColor::Depth(_)) {
            self.depth_colors
                .ok_or(SceneLightingError::MissingDepthColors)?
        } else {
            DepthColorFamily::STANDARD
        };
        Ok(
            material.palette_pair_with_family(
                family,
                group,
                lighting_enabled,
                normal,
                object_light,
            ),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthGroup {
    Near,
    Middle,
    Far,
    Farthest,
}

impl DepthGroup {
    /// Select the material bank from camera-space object depth and the three
    /// authored threshold bytes (`$01:94DE..951A`). The source adds each byte
    /// in the high half of a word, then tests only the wrapped result's sign.
    /// This deliberately is not a widened comparison against positive
    /// distances: negative depth and word overflow retain source behavior.
    /// The caller resolves the scene default or object's threshold override.
    pub fn for_camera_depth(depth: i16, thresholds: [i8; 3]) -> Self {
        for (threshold, group) in thresholds
            .into_iter()
            .zip([Self::Near, Self::Middle, Self::Far])
        {
            if depth.wrapping_add(i16::from(threshold) << 8) < 0 {
                return group;
            }
        }
        Self::Farthest
    }
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

    /// Resolve an object's selected threshold record before flat shading.
    /// This keeps bank selection and material evaluation on the same native
    /// camera-space depth, without a renderer-side distance approximation.
    pub fn palette_pair_at_depth(
        self,
        depth: i16,
        thresholds: [i8; 3],
        lighting_enabled: bool,
        normal: [i8; 3],
        object_light: [i8; 3],
    ) -> u8 {
        self.palette_pair(
            DepthGroup::for_camera_depth(depth, thresholds),
            lighting_enabled,
            normal,
            object_light,
        )
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
        self.palette_pair_with_family(
            DepthColorFamily::STANDARD,
            group,
            lighting_enabled,
            normal,
            object_light,
        )
    }

    /// Select the scene's actual depth-colour family. The shade-row catalog
    /// is shared across families; only depth-indexed materials use this input.
    pub fn palette_pair_with_family(
        self,
        family: DepthColorFamily,
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
            FlatColor::Depth(index) => DEPTH_PAIRS[family.0][bank][usize::from(index)],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_and_handoff_change_their_own_lighting_selections() {
        let inherited = DepthColorFamily::from_catalog_index(4).unwrap();
        let mut lighting = SceneLighting {
            thresholds: DepthThresholdTable::from_catalog_index(13),
            depth_colors: Some(inherited),
        };
        lighting.setup_scene();
        assert_eq!(lighting.thresholds, Some(DepthThresholdTable::NORMAL));
        assert_eq!(lighting.depth_colors, Some(inherited));
        lighting.finish_opening_load();
        assert_eq!(lighting.thresholds, Some(DepthThresholdTable::OPENING));
        assert_eq!(lighting.depth_colors, Some(DepthColorFamily::STANDARD));
    }

    #[test]
    fn object_override_is_one_based_and_does_not_require_a_scene_default() {
        let mut lighting = SceneLighting::default();
        assert_eq!(
            lighting.depth_group(0, 0),
            Err(SceneLightingError::MissingSceneThresholds)
        );
        for selector in 1..=14 {
            let table = DepthThresholdTable::from_catalog_index(usize::from(selector - 1)).unwrap();
            for depth in [i16::MIN, -1, 0, 255, 256, 4095, 4096, i16::MAX] {
                assert_eq!(
                    lighting.depth_group(depth, selector),
                    Ok(DepthGroup::for_camera_depth(depth, table.thresholds()))
                );
            }
        }
        for selector in 15..=u8::MAX {
            assert_eq!(
                lighting.depth_group(0, selector),
                Err(SceneLightingError::UnknownObjectThreshold(selector))
            );
        }
        lighting.setup_scene();
        assert_eq!(lighting.depth_group(4096, 0), Ok(DepthGroup::Farthest));
        lighting.finish_opening_load();
        assert_eq!(lighting.depth_group(4096, 0), Ok(DepthGroup::Near));
        // An actor's explicit normal record still overrides the opening table.
        assert_eq!(lighting.depth_group(4096, 5), Ok(DepthGroup::Farthest));
        assert_eq!(DepthThresholdTable::from_catalog_index(14), None);
        assert_eq!(DepthColorFamily::from_catalog_index(5), None);
    }

    #[test]
    fn only_depth_materials_require_the_selected_depth_color_family() {
        let lighting = SceneLighting::default();
        let pair = |word, enabled| {
            lighting.palette_pair(
                FlatMaterial::from_word(word).unwrap(),
                DepthGroup::Near,
                enabled,
                [0; 3],
                [0; 3],
            )
        };
        assert_eq!(pair(0x3F67, false), Ok(0x67));
        assert_eq!(pair(0x0900, false), Ok(9));
        assert!(pair(0x0900, true).is_ok());
        assert_eq!(
            pair(0x3E00, false),
            Err(SceneLightingError::MissingDepthColors)
        );
    }
}
