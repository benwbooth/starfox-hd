//! Publication ownership for the standard scene loader's artwork.
//!
//! Source: the artwork operations in 03:C80B and its D4FD continuation.
//! Main-loop loader resumptions and display-service publications are different
//! events. In particular the character publication queues the map publication
//! without a main-loop visit, whereas the foreground and sprite requests each
//! require one. Nothing here assigns an update number or a display duration.
//!
//! This owns artwork only. The frame host still owns scene-mode setup, layer
//! configuration, display blanking, palette effects and the loader's final
//! render-state handoff. The last main-loop resume is explicit so the host
//! can perform that handoff after, not during, the sprite publication.

use std::sync::Arc;

use sf2_data::opening_artwork::{
    BackgroundCell, BackgroundTile, ForegroundPaletteId, OpeningArtwork, BACKGROUND_CELL_COUNT,
    BACKGROUND_TILE_COUNT, SPRITE_COLORS,
};
use sf2_data::palettes::PolygonPaletteId;

use super::intro_controller::{IntroColor, OpeningScenePalette};

/// Published scene assets. Previous resources remain visible until their own
/// publication event; replacing characters does not also replace the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneArtwork {
    pub characters: Option<Arc<[BackgroundTile; BACKGROUND_TILE_COUNT]>>,
    pub map: Option<Arc<[BackgroundCell; BACKGROUND_CELL_COUNT]>>,
    pub sprite_colors: [IntroColor; SPRITE_COLORS],
    /// Consumed and cleared by the next character publication even if it skips
    /// the background palette copy. This policy is inherited across loaders.
    pub skip_next_background_palette: bool,
}

impl Default for SceneArtwork {
    fn default() -> Self {
        Self {
            characters: None,
            map: None,
            sprite_colors: [IntroColor::default(); SPRITE_COLORS],
            skip_next_background_palette: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForegroundSelection {
    pub use_catalog: bool,
    pub entry: u8,
}

impl ForegroundSelection {
    pub const STANDARD: Self = Self {
        use_catalog: false,
        entry: 0,
    };

    pub const fn palette(self) -> ForegroundPaletteId {
        ForegroundPaletteId::select(self.use_catalog, self.entry)
    }
}

/// The artwork portion of one display-service operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkPublication {
    PolygonPalette,
    BackgroundCharacters,
    BackgroundMap,
    ForegroundPalette(ForegroundPaletteId),
    SpritePalette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkLoadPhase {
    Pending(ArtworkPublication),
    RequestBackground,
    SelectForeground,
    RequestSprites,
    RenderHandoff,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtworkResume {
    WaitingForPublication,
    Queued(ArtworkPublication),
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningArtworkLoad {
    artwork: Arc<OpeningArtwork>,
    phase: ArtworkLoadPhase,
}

impl OpeningArtworkLoad {
    /// Begin after the host accepts the standard scene-load request. The first
    /// publication belongs to the setup service's polygon-palette operation.
    pub fn new(artwork: Arc<OpeningArtwork>) -> Self {
        Self {
            artwork,
            phase: ArtworkLoadPhase::Pending(ArtworkPublication::PolygonPalette),
        }
    }

    pub fn phase(&self) -> ArtworkLoadPhase {
        self.phase
    }

    /// Resume the main-loop loader after the host's scene-service gates pass.
    /// Foreground policy is sampled only after both background publications,
    /// then retained even if the caller's policy changes while it is queued.
    pub fn resume(&mut self, foreground: ForegroundSelection) -> ArtworkResume {
        use ArtworkLoadPhase::*;
        let publication = match self.phase {
            Pending(_) => return ArtworkResume::WaitingForPublication,
            Complete => return ArtworkResume::Complete,
            RenderHandoff => {
                self.phase = Complete;
                return ArtworkResume::Complete;
            }
            RequestBackground => ArtworkPublication::BackgroundCharacters,
            SelectForeground => ArtworkPublication::ForegroundPalette(foreground.palette()),
            RequestSprites => ArtworkPublication::SpritePalette,
        };
        self.phase = Pending(publication);
        ArtworkResume::Queued(publication)
    }

    /// Publish exactly the queued artwork operation at a host-owned service
    /// boundary. No operation is repeated while awaiting a main-loop request.
    /// Palette-effect restoration remains a distinct subsequent service.
    pub fn publish(
        &mut self,
        target: &mut SceneArtwork,
        palette: &mut OpeningScenePalette,
    ) -> Option<ArtworkPublication> {
        use ArtworkLoadPhase::*;
        use ArtworkPublication::*;
        let Pending(publication) = self.phase else {
            return None;
        };
        self.phase = match publication {
            PolygonPalette => {
                palette.install_polygon_palette(PolygonPaletteId::Standard);
                RequestBackground
            }
            BackgroundCharacters => {
                if !target.skip_next_background_palette {
                    palette.install_background(&self.artwork.palettes);
                }
                target.skip_next_background_palette = false;
                target.characters = Some(Arc::clone(&self.artwork.characters));
                // The original character service advances directly to the
                // map service without waking the main-loop loader.
                Pending(BackgroundMap)
            }
            BackgroundMap => {
                target.map = Some(Arc::clone(&self.artwork.map));
                SelectForeground
            }
            ForegroundPalette(id) => {
                palette.install_foreground(&self.artwork.palettes, id);
                RequestSprites
            }
            SpritePalette => {
                target.sprite_colors = self.artwork.palettes.sprites.map(IntroColor::from_bgr555);
                RenderHandoff
            }
        };
        Some(publication)
    }
}

#[cfg(test)]
mod tests {
    use super::super::intro_material::{DepthColorFamily, DepthThresholdTable};
    use super::super::intro_scene::{OpeningArtworkRequestError, OpeningScene};
    use super::*;

    fn artwork() -> Arc<OpeningArtwork> {
        let colors: Vec<_> = (0..0x24C0u16 / 2).flat_map(u16::to_le_bytes).collect();
        Arc::new(OpeningArtwork::from_decoded(&vec![255; 8224], &vec![85; 4096], &colors).unwrap())
    }

    #[test]
    fn publications_wait_for_the_correct_owner_and_preserve_unrelated_state() {
        let artwork = artwork();
        let mut load = OpeningArtworkLoad::new(artwork.clone());
        let mut target = SceneArtwork::default();
        let mut palette = OpeningScenePalette::new([IntroColor::from_bgr555(0x1234); 128]);
        palette.saved_colors.fill(IntroColor::from_bgr555(0x4321));
        palette.effects.restoring = true;
        palette.effects.persistent_highlight = true;
        palette.refresh_requested = true;
        let initial = palette.clone();

        assert_eq!(
            load.resume(ForegroundSelection::STANDARD),
            ArtworkResume::WaitingForPublication
        );
        assert_eq!(
            load.publish(&mut target, &mut palette),
            Some(ArtworkPublication::PolygonPalette)
        );
        assert_eq!(palette.colors[..112], initial.colors[..112]);
        assert_eq!(load.phase(), ArtworkLoadPhase::RequestBackground);
        let after_polygon = palette.clone();
        assert_eq!(load.publish(&mut target, &mut palette), None);
        assert_eq!(palette, after_polygon);
        assert!(target.characters.is_none() && target.map.is_none());

        assert_eq!(
            load.resume(ForegroundSelection::STANDARD),
            ArtworkResume::Queued(ArtworkPublication::BackgroundCharacters)
        );
        assert_eq!(
            load.publish(&mut target, &mut palette),
            Some(ArtworkPublication::BackgroundCharacters)
        );
        assert!(Arc::ptr_eq(
            target.characters.as_ref().unwrap(),
            &artwork.characters
        ));
        assert!(target.map.is_none());
        assert_eq!(
            palette.colors[..64],
            artwork.palettes.background.map(IntroColor::from_bgr555)
        );
        assert_eq!(palette.colors[64..], after_polygon.colors[64..]);
        assert_eq!(
            load.resume(ForegroundSelection::STANDARD),
            ArtworkResume::WaitingForPublication
        );
        assert_eq!(
            load.publish(&mut target, &mut palette),
            Some(ArtworkPublication::BackgroundMap)
        );
        assert!(Arc::ptr_eq(target.map.as_ref().unwrap(), &artwork.map));
        assert_eq!(load.publish(&mut target, &mut palette), None);

        let alternate = ForegroundSelection {
            use_catalog: true,
            entry: 1,
        };
        assert_eq!(
            load.resume(alternate),
            ArtworkResume::Queued(ArtworkPublication::ForegroundPalette(
                ForegroundPaletteId::CatalogOne
            ))
        );
        // A later policy change cannot change an already queued descriptor.
        assert_eq!(
            load.resume(ForegroundSelection::STANDARD),
            ArtworkResume::WaitingForPublication
        );
        assert_eq!(
            load.publish(&mut target, &mut palette),
            Some(ArtworkPublication::ForegroundPalette(
                ForegroundPaletteId::CatalogOne
            ))
        );
        assert_eq!(
            palette.colors[64..112],
            artwork
                .palettes
                .foreground(ForegroundPaletteId::CatalogOne)
                .map(IntroColor::from_bgr555)
        );
        assert_eq!(palette.colors[112..], after_polygon.colors[112..]);
        assert_eq!(load.publish(&mut target, &mut palette), None);
        assert_eq!(target.sprite_colors, [IntroColor::default(); SPRITE_COLORS]);
        assert_eq!(
            load.resume(ForegroundSelection::STANDARD),
            ArtworkResume::Queued(ArtworkPublication::SpritePalette)
        );
        assert_eq!(
            load.publish(&mut target, &mut palette),
            Some(ArtworkPublication::SpritePalette)
        );
        assert_eq!(
            target.sprite_colors,
            artwork.palettes.sprites.map(IntroColor::from_bgr555)
        );
        assert_eq!(load.phase(), ArtworkLoadPhase::RenderHandoff);
        let final_state = (target.clone(), palette.clone());
        assert_eq!(load.resume(alternate), ArtworkResume::Complete);
        assert_eq!(load.phase(), ArtworkLoadPhase::Complete);
        assert_eq!(load.publish(&mut target, &mut palette), None);
        assert_eq!((target, palette.clone()), final_state);
        assert_eq!(palette.saved_colors, initial.saved_colors);
        assert_eq!(palette.effects, initial.effects);
        assert_eq!(palette.refresh_requested, initial.refresh_requested);
    }

    #[test]
    fn background_skip_is_consumed_without_skipping_characters_or_map() {
        let mut load = OpeningArtworkLoad::new(artwork());
        let mut target = SceneArtwork {
            skip_next_background_palette: true,
            ..Default::default()
        };
        let mut palette = OpeningScenePalette::new([IntroColor::from_bgr555(0x1234); 128]);
        load.publish(&mut target, &mut palette);
        load.resume(ForegroundSelection::STANDARD);
        load.publish(&mut target, &mut palette);
        assert_eq!(palette.colors[..64], [IntroColor::from_bgr555(0x1234); 64]);
        assert!(!target.skip_next_background_palette);
        assert!(target.characters.is_some());
        load.publish(&mut target, &mut palette);
        assert!(target.map.is_some());
    }

    #[test]
    fn opening_scene_owns_publications_without_advancing_actors_or_accepting_overlapping_loads() {
        let artwork = artwork();
        let mut scene = OpeningScene::default();
        assert_eq!(scene.lighting().thresholds, None);
        assert_eq!(scene.lighting().depth_colors, None);
        assert_eq!(
            scene.resume_artwork_load(ForegroundSelection::STANDARD),
            Err(OpeningArtworkRequestError::NotStarted)
        );
        assert_eq!(scene.publish_artwork(), None);
        scene.begin_artwork_load(artwork.clone(), false).unwrap();
        let before = scene.clone();
        assert_eq!(
            scene.begin_artwork_load(artwork.clone(), true),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene, before);
        for expected in [
            ArtworkPublication::PolygonPalette,
            ArtworkPublication::BackgroundCharacters,
            ArtworkPublication::BackgroundMap,
            ArtworkPublication::ForegroundPalette(ForegroundPaletteId::Standard),
            ArtworkPublication::SpritePalette,
        ] {
            scene
                .resume_artwork_load(ForegroundSelection::STANDARD)
                .unwrap();
            assert_eq!(scene.publish_artwork(), Some(expected));
            assert_eq!(
                scene.lighting().thresholds,
                Some(DepthThresholdTable::NORMAL)
            );
            assert_eq!(scene.lighting().depth_colors, None);
        }
        assert_eq!(scene.global_clock(), 0);
        assert_eq!(scene.controller(), before.controller());
        assert_eq!(
            scene.actors().collect::<Vec<_>>(),
            before.actors().collect::<Vec<_>>()
        );
        assert_eq!(
            scene.artwork_load_phase(),
            Some(ArtworkLoadPhase::RenderHandoff)
        );
        let before_handoff = scene.clone();
        assert_eq!(
            scene.begin_artwork_load(artwork.clone(), false),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(
            scene.queue_artwork_load(artwork.clone(), true),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene.publish_artwork(), None);
        assert_eq!(scene, before_handoff);
        scene
            .resume_artwork_load(ForegroundSelection::STANDARD)
            .unwrap();
        assert_eq!(scene.artwork_load_phase(), Some(ArtworkLoadPhase::Complete));
        assert_eq!(
            scene.lighting().thresholds,
            Some(DepthThresholdTable::OPENING)
        );
        assert_eq!(
            scene.lighting().depth_colors,
            Some(DepthColorFamily::STANDARD)
        );
        let complete = scene.clone();
        scene
            .resume_artwork_load(ForegroundSelection::STANDARD)
            .unwrap();
        assert_eq!(scene, complete);
        let old_artwork = scene.artwork().clone();
        scene.begin_artwork_load(artwork, false).unwrap();
        assert_eq!(scene.artwork(), &old_artwork);
        assert_eq!(scene.lighting(), complete.lighting());
        assert_eq!(
            scene.publish_artwork(),
            Some(ArtworkPublication::PolygonPalette)
        );
        assert_eq!(
            scene.lighting().thresholds,
            Some(DepthThresholdTable::NORMAL)
        );
        assert_eq!(
            scene.lighting().depth_colors,
            Some(DepthColorFamily::STANDARD)
        );
    }
}
