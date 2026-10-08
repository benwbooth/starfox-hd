//! Typed effects of authored map records and the scene host that applies
//! them. Every effect names its owner; presentation requests are recorded in
//! `MapPresentation` for the display/loader owners, never dropped.

use super::path_program::ActionGate;
use super::scene_map::{
    allocate_map_actor, MapActorSpawn, MapCatalog, MapCondition, MapCursor, SceneMap,
    SceneMapHost,
};
use super::scene_path_world::ScenePathWorld;
use super::{ObjectId, ObjectStore};

/// Display request byte (F3) written by `$03:9A7C` and `$03:9A71`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayModeRequest {
    /// FE: blank the scene display.
    Blank,
    /// 02: show the scene.
    Scene,
}

/// Map-written presentation bytes whose consumers are presentation owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationByte {
    /// 18BB, read by the scene display services (`$02:8E79`, `$03:DD86`).
    SceneStyle,
    /// 1B49, the title stage's layout byte.
    TitleLayout,
    /// 1D57, latched by the scene-player prologue.
    PlayerCountLatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapEffect {
    DisplayMode(DisplayModeRequest),
    /// `$03:9D3C`: scene-load request (169B) and its loader bit (1D27 bit 04).
    SceneLoad(u16),
    /// `$03:984C`: loader hold bit (1D27 bit 08).
    LoaderHold,
    /// `$03:90DD`: ambient particle control's low byte (7001BC).
    AmbientControl(u8),
    ActionGate(u8),
    SceneSelection(u8),
    HandoffFlags(u8),
    /// 1D78/1D77: the continuation restored by `$7F:BF3D`.
    SaveContinuation(MapCursor),
    /// 1E44: the authored vertical projection bias.
    CameraProjectionBase(i16),
    Presentation(PresentationByte, u8),
    /// `$03:DD6F`: display mode 02, fade progress (F4) clear, scene style 02.
    ResetSceneDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSpawn {
    Actor(MapActorSpawn),
}

/// Presentation state written by maps for the display and loader owners.
/// Conditions read here are supplied by those owners each frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapPresentation {
    pub display_mode: Option<DisplayModeRequest>,
    pub fade_progress_cleared: bool,
    pub scene_load: Option<u16>,
    pub loader_hold: bool,
    pub scene_style: Option<u8>,
    pub title_layout: Option<u8>,
    pub player_count_latch: Option<u8>,
    /// Supplied by the display owner (F4 clear and the fade at full, 7F007C).
    pub display_ready: Option<bool>,
    /// Supplied by the loader owner (the selected load-table entry is clear).
    pub load_table_idle: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapHostError {
    MissingHandoff,
    MissingAmbientControl,
    MissingSpawnDefaults,
    MissingSinglePlayerPolicy,
    MissingDisplayReadiness,
    MissingLoaderReadiness,
    UnsupportedCondition(MapCondition),
    /// An effect that changes the map itself, applied by `MapScene` instead.
    MapOwned,
}

/// The scene world as seen by map records. Continuations are returned to
/// the caller, which owns the `SceneMap`, after each record.
pub struct MapWorld<'a> {
    pub objects: &'a mut ObjectStore,
    pub world: &'a mut ScenePathWorld,
    pub presentation: &'a mut MapPresentation,
    pub continuation: Option<MapCursor>,
}

impl SceneMapHost<MapEffect, MapSpawn> for MapWorld<'_> {
    type Error = MapHostError;

    fn condition(&self, condition: MapCondition) -> Result<bool, Self::Error> {
        match condition {
            // 1AA6 bit 02 is the same shared byte as the reflection policy.
            MapCondition::SinglePlayer => self
                .world
                .reflect_all_contacts
                .ok_or(MapHostError::MissingSinglePlayerPolicy),
            MapCondition::DisplayReady => self
                .presentation
                .display_ready
                .ok_or(MapHostError::MissingDisplayReadiness),
            MapCondition::LoadTableIdle => self
                .presentation
                .load_table_idle
                .ok_or(MapHostError::MissingLoaderReadiness),
            other => Err(MapHostError::UnsupportedCondition(other)),
        }
    }

    fn apply(&mut self, effect: &MapEffect) -> Result<(), Self::Error> {
        let presentation = &mut *self.presentation;
        match *effect {
            MapEffect::DisplayMode(mode) => presentation.display_mode = Some(mode),
            MapEffect::SceneLoad(load) => presentation.scene_load = Some(load),
            MapEffect::LoaderHold => presentation.loader_hold = true,
            MapEffect::AmbientControl(low) => {
                let control = self
                    .world
                    .render_environment
                    .ambient_control
                    .as_mut()
                    .ok_or(MapHostError::MissingAmbientControl)?;
                *control = super::player_surface_render::AmbientParticleControl::from_bits(
                    (control.bits() & 0xFF00) | u16::from(low),
                );
            }
            MapEffect::ActionGate(code) => self.world.action_gate = Some(ActionGate { code }),
            MapEffect::SceneSelection(selection) => self.world.scene_selection = Some(selection),
            MapEffect::HandoffFlags(flags) => {
                self.world
                    .handoff
                    .as_mut()
                    .ok_or(MapHostError::MissingHandoff)?
                    .player_flags = flags
            }
            MapEffect::SaveContinuation(target) => self.continuation = Some(target),
            MapEffect::CameraProjectionBase(bias) => self.world.camera_projection_base = Some(bias),
            MapEffect::Presentation(byte, value) => match byte {
                PresentationByte::SceneStyle => presentation.scene_style = Some(value),
                PresentationByte::TitleLayout => presentation.title_layout = Some(value),
                PresentationByte::PlayerCountLatch => presentation.player_count_latch = Some(value),
            },
            MapEffect::ResetSceneDisplay => {
                const RESET_SCENE_STYLE: u8 = 2;
                presentation.display_mode = Some(DisplayModeRequest::Scene);
                presentation.fade_progress_cleared = true;
                presentation.scene_style = Some(RESET_SCENE_STYLE);
            }
        }
        Ok(())
    }

    fn apply_to_current(&mut self, _: ObjectId, effect: &MapEffect) -> Result<(), Self::Error> {
        self.apply(effect)
    }

    fn spawn(&mut self, specification: &MapSpawn) -> Result<Option<ObjectId>, Self::Error> {
        let MapSpawn::Actor(spawn) = *specification;
        let defaults = self
            .world
            .spawn_defaults()
            .ok_or(MapHostError::MissingSpawnDefaults)?;
        Ok(allocate_map_actor(self.objects, defaults, spawn))
    }
}

/// Visit the map once with the given world, then publish a continuation it
/// saved to the map owner (`SceneMap::save_continuation`).
pub fn visit(
    map: &mut SceneMap,
    catalog: &MapCatalog<'_, MapEffect, MapSpawn>,
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    presentation: &mut MapPresentation,
    budget: usize,
) -> Result<super::scene_map::MapReport, super::scene_map::MapError<MapHostError>> {
    let mut host = MapWorld {
        objects,
        world,
        presentation,
        continuation: None,
    };
    let report = map.visit(catalog, &mut host, Default::default(), budget)?;
    if let Some(target) = host.continuation {
        map.save_continuation(catalog, target)
            .map_err(|_| super::scene_map::MapError::Faulted)?;
    }
    Ok(report)
}
