//! Scene-owned tile layout and display publications.
//!
//! A main-loop setup request captures the large-character selection. The
//! display service samples the current layer policy later. These are separate
//! operations, with no invented refresh duration or implicit actor update.
//! Source: setup helpers 03:D570..D609 and display jobs 7F:0B74..0C57.
//!
//! Publications describe native layers and tile grids, not video-memory
//! addresses. Asset binding, raster offsets, other scene resets and the timing
//! of the display service are separate owners.

use super::scene_display::DisplayBand;

const SMALL_GRID_EDGE: u16 = 32;
const LARGE_GRID_EDGE: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkPlane {
    First,
    Second,
}

impl ArtworkPlane {
    fn large_characters(self) -> [bool; 4] {
        match self {
            Self::First => [true, false, false, false],
            Self::Second => [false, true, false, false],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileMapGrid {
    Square,
    Wide,
    Tall,
    LargeSquare,
}

impl TileMapGrid {
    pub const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Square => (SMALL_GRID_EDGE, SMALL_GRID_EDGE),
            Self::Wide => (LARGE_GRID_EDGE, SMALL_GRID_EDGE),
            Self::Tall => (SMALL_GRID_EDGE, LARGE_GRID_EDGE),
            Self::LargeSquare => (LARGE_GRID_EDGE, LARGE_GRID_EDGE),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneLayerMask(u8);

impl SceneLayerMask {
    pub const FIRST: Self = Self(1 << 0);
    pub const SECOND: Self = Self(1 << 1);
    pub const THIRD: Self = Self(1 << 2);
    pub const FOURTH: Self = Self(1 << 3);
    pub const SPRITES: Self = Self(1 << 4);
    pub const STANDARD_SCENE: Self = Self(Self::FIRST.0 | Self::SECOND.0 | Self::SPRITES.0);

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, layer: Self) -> bool {
        self.0 & layer.0 == layer.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneTileMode {
    /// Two four-bit backgrounds and a two-bit foreground.
    Layered,
    /// Two four-bit backgrounds with a separate column-offset map.
    ColumnOffsets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneModeSetup {
    LayeredSmallTiles,
    LayeredLargeTiles,
    OffsetWideMap,
    OffsetTallMap,
}

/// Inputs inherited from the scene's current layer policy. Resource binding
/// belongs to the asset owner; grid dimensions here describe the published
/// map, not an address encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneLayerPolicy {
    pub artwork_plane: ArtworkPlane,
    pub visible_layers: SceneLayerMask,
    /// Additional large-character choices apply only to layered mode.
    pub layered_large_characters: [bool; 4],
    pub layered_foreground_priority: bool,
    pub third_map_grid: TileMapGrid,
}

impl SceneLayerPolicy {
    /// Policy at machine reset, before selecting a view or dispatching a
    /// scene-load sequence. This is not a default for an entered scene.
    pub const fn cold_boot() -> Self {
        Self {
            artwork_plane: ArtworkPlane::Second,
            visible_layers: SceneLayerMask::from_bits(0),
            layered_large_characters: [false; 4],
            layered_foreground_priority: false,
            third_map_grid: TileMapGrid::Square,
        }
    }

    /// Layer choice of the opening view preset (03:B40B..B497). Other view
    /// geometry, resource bindings and scene flags belong to their owners.
    pub fn select_opening_artwork_plane(&mut self) {
        self.artwork_plane = ArtworkPlane::First;
    }

    /// The video initialization at 03:AE33 clears only these extra mode
    /// choices. The foreground grid extension and visibility are retained.
    pub fn initialize_video_layers(&mut self) {
        self.layered_large_characters = [false; 4];
        self.layered_foreground_priority = false;
    }

    /// Common load-table dispatch (03:9D5B), before any selected loader runs.
    /// Individual loaders can subsequently replace this visibility policy.
    pub fn begin_load_sequence(&mut self) {
        self.visible_layers = SceneLayerMask::STANDARD_SCENE;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingMode {
    mode: SceneTileMode,
    large_characters: [bool; 4],
    artwork_map_grid: TileMapGrid,
}

/// The layout changes made by one completed setup service. This is a
/// publication, not a snapshot of untouched map bindings or scroll channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneModePublication {
    pub mode: SceneTileMode,
    pub large_characters: [bool; 4],
    pub foreground_priority: bool,
    pub visible_layers: SceneLayerMask,
    pub artwork_plane: ArtworkPlane,
    pub artwork_map_grid: TileMapGrid,
    pub third_map_grid: TileMapGrid,
    /// The setup resets this channel even when artwork is on the first plane.
    pub second_layer_vertical_scroll: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneVideoError {
    SetupPending,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneVideo {
    pending: Option<PendingMode>,
    last_setup: Option<SceneModePublication>,
    /// Current whole-screen output. This does not replace the retained fade
    /// progress or the three independently published scanline bands.
    output: Option<DisplayBand>,
}

impl SceneVideo {
    pub const fn setup_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub const fn last_setup(&self) -> Option<SceneModePublication> {
        self.last_setup
    }

    pub const fn output(&self) -> Option<DisplayBand> {
        self.output
    }

    /// Main-loop request. The source's wide-map offset mode is distinct from
    /// tall-map offset mode; the map-shape choice is not the colour mode.
    pub fn request_setup(
        &mut self,
        setup: SceneModeSetup,
        artwork_plane: ArtworkPlane,
    ) -> Result<(), SceneVideoError> {
        if self.pending.is_some() {
            return Err(SceneVideoError::SetupPending);
        }
        let mode = match setup {
            SceneModeSetup::LayeredSmallTiles | SceneModeSetup::LayeredLargeTiles => {
                SceneTileMode::Layered
            }
            SceneModeSetup::OffsetWideMap | SceneModeSetup::OffsetTallMap => {
                SceneTileMode::ColumnOffsets
            }
        };
        self.pending = Some(PendingMode {
            mode,
            large_characters: if setup == SceneModeSetup::LayeredSmallTiles {
                [false; 4]
            } else {
                artwork_plane.large_characters()
            },
            artwork_map_grid: if setup == SceneModeSetup::OffsetTallMap {
                TileMapGrid::Tall
            } else {
                TileMapGrid::Wide
            },
        });
        Ok(())
    }

    /// Visit only at the host's display-service boundary, after its blanking
    /// wait. The current layer choice can differ from the earlier tile choice.
    /// The palette and other scene-setup resets are handled by their owners.
    pub fn publish_setup(&mut self, policy: SceneLayerPolicy) -> Option<SceneModePublication> {
        let pending = self.pending.take()?;
        let layered = pending.mode == SceneTileMode::Layered;
        let publication = SceneModePublication {
            mode: pending.mode,
            large_characters: std::array::from_fn(|index| {
                pending.large_characters[index]
                    || (layered && policy.layered_large_characters[index])
            }),
            foreground_priority: layered && policy.layered_foreground_priority,
            visible_layers: policy.visible_layers,
            artwork_plane: policy.artwork_plane,
            artwork_map_grid: pending.artwork_map_grid,
            third_map_grid: policy.third_map_grid,
            second_layer_vertical_scroll: 0,
        };
        self.publish_transfer_blank();
        self.last_setup = Some(publication);
        Some(publication)
    }

    /// Every standard loader character, map and palette service forces this
    /// output before its copy. It leaves fade-band state untouched.
    pub fn publish_transfer_blank(&mut self) {
        self.output = Some(DisplayBand::BLANK_FULL);
    }

    /// An independently scheduled display owner publishes its actual output.
    /// This does not consume a pending setup or infer a fade visit.
    pub fn publish_display(&mut self, band: DisplayBand) {
        self.output = Some(band);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_display::Intensity;

    fn policy() -> SceneLayerPolicy {
        SceneLayerPolicy {
            artwork_plane: ArtworkPlane::First,
            visible_layers: SceneLayerMask::STANDARD_SCENE,
            layered_large_characters: [false, false, true, true],
            layered_foreground_priority: true,
            third_map_grid: TileMapGrid::LargeSquare,
        }
    }

    #[test]
    fn boot_view_and_load_dispatch_remain_distinct_policy_transitions() {
        let mut policy = SceneLayerPolicy::cold_boot();
        assert_eq!(policy.artwork_plane, ArtworkPlane::Second);
        assert_eq!(policy.visible_layers.bits(), 0);
        policy.select_opening_artwork_plane();
        assert_eq!(policy.artwork_plane, ArtworkPlane::First);
        assert_eq!(policy.visible_layers.bits(), 0);
        policy.layered_large_characters = [true; 4];
        policy.layered_foreground_priority = true;
        policy.third_map_grid = TileMapGrid::Tall;
        policy.visible_layers = SceneLayerMask::THIRD;
        policy.initialize_video_layers();
        assert_eq!(policy.layered_large_characters, [false; 4]);
        assert!(!policy.layered_foreground_priority);
        assert_eq!(policy.third_map_grid, TileMapGrid::Tall);
        assert_eq!(policy.visible_layers, SceneLayerMask::THIRD);
        policy.begin_load_sequence();
        assert_eq!(policy.visible_layers, SceneLayerMask::STANDARD_SCENE);
        assert_eq!(policy.artwork_plane, ArtworkPlane::First);
        assert_eq!(policy.third_map_grid, TileMapGrid::Tall);
    }

    #[test]
    fn four_setups_preserve_mode_map_and_tile_selection_independently() {
        for (setup, mode, grid, large) in [
            (
                SceneModeSetup::LayeredSmallTiles,
                SceneTileMode::Layered,
                TileMapGrid::Wide,
                false,
            ),
            (
                SceneModeSetup::LayeredLargeTiles,
                SceneTileMode::Layered,
                TileMapGrid::Wide,
                true,
            ),
            (
                SceneModeSetup::OffsetWideMap,
                SceneTileMode::ColumnOffsets,
                TileMapGrid::Wide,
                true,
            ),
            (
                SceneModeSetup::OffsetTallMap,
                SceneTileMode::ColumnOffsets,
                TileMapGrid::Tall,
                true,
            ),
        ] {
            for initial_plane in [ArtworkPlane::First, ArtworkPlane::Second] {
                for current_plane in [ArtworkPlane::First, ArtworkPlane::Second] {
                    for mask in 0..=u8::MAX {
                        let mut video = SceneVideo::default();
                        video.request_setup(setup, initial_plane).unwrap();
                        assert_eq!(video.last_setup(), None);
                        assert_eq!(video.output(), None);
                        let publication = video
                            .publish_setup(SceneLayerPolicy {
                                artwork_plane: current_plane,
                                visible_layers: SceneLayerMask::from_bits(mask),
                                ..policy()
                            })
                            .unwrap();
                        assert_eq!(publication.mode, mode);
                        assert_eq!(publication.artwork_map_grid, grid);
                        assert_eq!(publication.artwork_plane, current_plane);
                        assert_eq!(publication.visible_layers.bits(), mask);
                        assert_eq!(
                            publication.large_characters,
                            [
                                large && initial_plane == ArtworkPlane::First,
                                large && initial_plane == ArtworkPlane::Second,
                                mode == SceneTileMode::Layered,
                                mode == SceneTileMode::Layered,
                            ]
                        );
                        assert_eq!(
                            publication.foreground_priority,
                            mode == SceneTileMode::Layered
                        );
                        assert_eq!(publication.third_map_grid, TileMapGrid::LargeSquare);
                        assert_eq!(publication.second_layer_vertical_scroll, 0);
                        assert_eq!(video.output(), Some(DisplayBand::BLANK_FULL));
                        assert!(!video.setup_pending());
                    }
                }
            }
        }
    }

    #[test]
    fn duplicate_requests_and_publications_leave_state_unchanged() {
        let mut video = SceneVideo::default();
        let visible = DisplayBand {
            blanked: false,
            intensity: Intensity::new(7),
        };
        video.publish_display(visible);
        video
            .request_setup(SceneModeSetup::OffsetTallMap, ArtworkPlane::First)
            .unwrap();
        let pending = video;
        assert_eq!(
            video.request_setup(SceneModeSetup::LayeredSmallTiles, ArtworkPlane::Second),
            Err(SceneVideoError::SetupPending)
        );
        assert_eq!(video, pending);
        video.publish_setup(policy()).unwrap();
        video.publish_display(visible);
        let completed = video;
        assert_eq!(video.publish_setup(policy()), None);
        assert_eq!(video, completed);
        video.publish_transfer_blank();
        assert_eq!(video.last_setup(), completed.last_setup());
        assert_eq!(video.output(), Some(DisplayBand::BLANK_FULL));
    }
}
