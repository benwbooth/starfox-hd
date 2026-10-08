//! Typed effects of authored map records and the scene host that applies
//! them. Every effect names its owner; presentation requests are recorded in
//! `MapPresentation` for the display/loader owners, never dropped.

use super::path_program::ActionGate;
use super::scene_map::{
    allocate_map_actor, MapActorSpawn, MapCatalog, MapCondition, MapCursor, SceneMap,
    SceneMapHost,
};
use super::map_streaming::{MapProgram, MapRecord, MapRecordFlags, MapRegion, RegionError};
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
    /// 1E68, the backdrop program selector dispatched at `$02:F05B`.
    BackdropProgram,
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
    /// 1D78/1D77 naming bytes that are not a map record.
    SaveUnrunnableContinuation,
    /// 1E44: the authored vertical projection bias.
    CameraProjectionBase(i16),
    Presentation(PresentationByte, u8),
    /// `$03:DD6F`: display mode 02, fade progress (F4) clear, scene style 02.
    ResetSceneDisplay,
    /// `$03:9692`: the current actor's path (byte 2B).
    InstallPath(super::PathCursor),
    /// `$03:9A87` at 2D/2E: the current actor's health or attack.
    ActorHitPoints(u8),
    ActorAttackPower(u8),
    /// `$03:953F`: a proximity-streamed path record in the current region.
    DeclarePathRecord(PathRecord),
    /// `$03:90EF`: write region slot `index` and count one more region.
    RegisterRegion { index: u8, region: MapRegion },
    /// `$06:9A2F`: the primary player's map-selected vertical profile.
    PlayerVerticalProfile(super::player_vertical::VerticalProfile),
    /// `$06:9A5F`: the primary player's camera pitch limits (6BFD/6BFE),
    /// skipped without a primary player. Its other stores (6BFB, 6B49,
    /// 6B59) have no reader and are not modeled.
    CameraPitchProfile(super::player_camera_angles::CameraPitchProfile),
    /// `$06:9B04` / `$06:9B20`: the primary player's occupancy exemption
    /// (6BEB bit 80).
    OccupancyExempt(bool),
    /// 1E32 / 1E34: one half of the authored camera height limits.
    CameraHeightLimit(HeightLimit, i16),
    /// 1E0F: the shared environmental reference plane.
    EnvironmentPlane(i16),
    /// 18B9: the shared rendering plane.
    RenderPlane(i16),
    /// 1E13: the shared carry context byte.
    PlayerCarryMode(u8),
    /// 1DE2: the shared character/mode byte.
    PlayerConfiguration(u8),
    /// D739: the map-record streaming radius ceiling.
    StreamingRadiusLimit(u16),
    /// 1DE3: the player-configuration variant (bit 80 selects the record's
    /// alternate first word at `$06:85F9`).
    PlayerConfigurationVariant(u8),
    /// 1DE4/1DE6/1DE8: one coordinate of the mission-entry placement.
    PlacementCoordinate(super::path_fields::Axis, i16),
    /// 1DEA: the mission-entry heading.
    PlacementHeading(u8),
    /// `$06:9A92`: place the primary player at the mission-entry placement.
    PlacePrimaryPlayer,
    /// 1D75 / 1D76: the scene selections the stage-exit player actions
    /// (`$0D:CD11`, `$0D:CD1A`) publish to 1D73; 1D76 = FE marks no
    /// alternate selection (`$0D:BBE6`).
    ExitSceneSelection(ExitScene, u8),
    /// 1D7B/1D7A: the map script a stage-exit action (`$0D:CDC2`) installs.
    StageExitScript(MapCursor),
    /// 1D80: the scene load a later stage action issues (`$0D:C721`).
    DeferredSceneLoad(u16),
    /// 1E5A: the altitude gauge's divisor (`$07:AA59`).
    AltitudeGaugeScale(u16),
    /// GSU scene parameters read by the backdrop program.
    GsuParameter(GsuParameter, u16),
    /// Inline `$06:9ACD`-style block: transition flag 40 (6BEC) for the
    /// primary player and, unless the stage layout word (1916) is C0, the
    /// secondary player.
    LinkPilotTransitions,
    /// Inline GSU select: the backdrop program table (700050) chosen by the
    /// display flag word (1B9C bit 20), resolved by the presentation owner.
    SelectBackdropTable,
    /// `$0D:DA7A`: draw (true) or erase a region's marker in the strategic
    /// map bitplane.
    RegionMarker { region: u8, drawn: bool },
    /// Inline word-bit block on the shared execution-mode word (1B84).
    ModeFlags { bits: u16, set: bool },
    /// `$06:9ACD`: controlled-flag 40 (6B65) for the primary player and,
    /// unless the stage layout word is C0, the secondary player.
    LinkControlledPilots,
    /// 1C06: the encounter variant byte.
    EncounterVariant(u8),
    /// 1E17: the stage-exit mode byte (`$0D:BBC3`, `$06:A09B`).
    StageExitMode(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitScene {
    /// 1D75.
    Primary,
    /// 1D76.
    Alternate,
}

/// GSU RAM words written by maps for the backdrop GSU program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GsuParameter {
    /// 702862: the first color the program passes to `$01:F1BE`.
    BackdropColor,
    /// 70285C: added to the pattern ROM pointer at `$01:C98F`.
    PatternOffset,
    /// 70285E (byte): stored to the program's 0055 at `$01:CB43`.
    PatternMode,
}

/// Stage-exit state written by maps and consumed by the bank-0D stage
/// actions: exit scene selections (1D75/1D76), the exit map script
/// (1D7B/1D7A) and a deferred scene load (1D80).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StageExit {
    pub primary_scene: Option<u8>,
    pub alternate_scene: Option<u8>,
    pub script: Option<MapCursor>,
    pub deferred_load: Option<u16>,
    pub mode: Option<u8>,
}

/// The mission-entry placement (1DE4..1DEA) written by map records.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapPlacement {
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub z: Option<i16>,
    pub heading: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightLimit {
    /// 1E32.
    Top,
    /// 1E34.
    Bottom,
}

/// The authored part of an opcode-90 record; the group is the current region
/// and the radius comes from the shape's catalog extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathRecord {
    pub position: super::Vector3,
    pub yaw: super::Angle,
    pub shape: super::ShapeId,
    pub path: super::PathCursor,
}

/// How a map-spawned path actor enters its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathEntry {
    /// `$7F:7E1E`: the shared one-time path prefix.
    Plain,
    /// `$7F:7E00`: health and attack 10, then the shared prefix.
    DefaultCombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSpawn {
    Actor(MapActorSpawn),
    /// A path actor; its path is installed by the following opcode 8C.
    PathActor {
        shape: super::ShapeId,
        position: super::Vector3,
        entry: PathEntry,
    },
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
    pub backdrop_program: Option<u8>,
    pub altitude_gauge_scale: Option<u16>,
    pub backdrop_color: Option<u16>,
    pub pattern_offset: Option<u16>,
    pub pattern_mode: Option<u8>,
    pub backdrop_table_selected: bool,
    /// Region markers written by `$0D:DA7A`, and which of them are drawn.
    pub region_markers_written: u128,
    pub region_markers_drawn: u128,
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
    MissingCurrentActor(ObjectId),
    /// An actor effect reached without a current actor.
    ActorEffectWithoutActor,
    MissingEncounterLayout,
    MissingMapRecords,
    MissingRegions,
    MissingRegionGroups,
    MissingShapeExtent(super::ShapeId),
    Region(RegionError),
    MissingPrimaryPlayer,
    PlayerRecords(super::scene_path_world::WorldInputError),
    MissingPlayerVertical,
    MissingPlayerCameraAngles,
    MissingCameraHeightLimits,
    IncompletePlacement,
    MissingProgramResources,
    MissingPlayerStorage,
    MissingPlayerAuxiliary,
    MissingFixedView,
    MissingSceneEvents,
    MissingStageLayout,
    MissingSecondaryPlayer,
    MissingModeSelection,
    MissingExecutionMode,
    MissingControlledFlags,
    MissingCampaign,
    MissingScenarioFlags,
}

/// The scene world as seen by map records. Continuations are returned to
/// the caller, which owns the `SceneMap`, after each record.
pub struct MapWorld<'a> {
    pub objects: &'a mut ObjectStore,
    pub world: &'a mut ScenePathWorld,
    /// Program resources holding player storage, for player placement.
    pub resources: Option<&'a mut super::program_resources::ProgramResources<super::program_state::ProgramData>>,
    pub presentation: &'a mut MapPresentation,
    /// A continuation saved by this visit; `Some(None)` is unrunnable.
    pub continuation: Option<Option<MapCursor>>,
}

impl MapWorld<'_> {
    /// The primary player and, unless the stage layout word (1916) is C0,
    /// the secondary player (`$06:9ACD` and its inline copies).
    fn for_each_stage_player(
        &mut self,
        mut apply: impl FnMut(&mut super::scene_path_world::PlayerPathRecords) -> Result<(), MapHostError>,
    ) -> Result<(), MapHostError> {
        const SINGLE_LAYOUT: u16 = 0x00C0;
        let primary = self.world.primary_player.ok_or(MapHostError::MissingPrimaryPlayer)?;
        let layout = self.world.stage_layout.ok_or(MapHostError::MissingStageLayout)?;
        let mut owners = vec![primary];
        if layout != SINGLE_LAYOUT {
            owners.push(self.world.secondary_player.ok_or(MapHostError::MissingSecondaryPlayer)?);
        }
        for owner in owners {
            apply(self.world.player_mut(self.objects, owner).map_err(MapHostError::PlayerRecords)?)?;
        }
        Ok(())
    }

    /// `$06:9A92`: the player's position and heading, its storage heading
    /// (6ABC), and the opposite heading in its stored rotation (6B34) and
    /// the fixed view's coarse yaw (033F byte 15).
    fn place_primary_player(&mut self) -> Result<(), MapHostError> {
        let MapPlacement { x: Some(x), y: Some(y), z: Some(z), heading: Some(heading) } =
            self.world.map_placement
        else {
            return Err(MapHostError::IncompletePlacement);
        };
        let owner = self.world.primary_player.ok_or(MapHostError::MissingPrimaryPlayer)?;
        let view = self.world.fixed_players[0].ok_or(MapHostError::MissingFixedView)?;
        let resources = self.resources.as_deref_mut().ok_or(MapHostError::MissingProgramResources)?;
        let storage = super::player_storage::get_mut(self.objects, resources, owner)
            .map_err(|_| MapHostError::MissingPlayerStorage)?;
        storage.fine_yaw = (storage.fine_yaw & 0x00FF) | (u16::from(heading) << 8);
        let opposite = heading.wrapping_neg();
        self.world
            .player_mut(self.objects, owner)
            .map_err(MapHostError::PlayerRecords)?
            .auxiliary
            .as_mut()
            .ok_or(MapHostError::MissingPlayerAuxiliary)?
            .stored_rotation
            .yaw = super::Angle::from_units(opposite);
        let player = self
            .objects
            .get_mut(owner)
            .ok_or(MapHostError::MissingCurrentActor(owner))?;
        player.base.position = super::Vector3 { x, y, z };
        player.base.yaw = super::Angle::from_units(heading);
        let view = self.objects.get_mut(view).ok_or(MapHostError::MissingCurrentActor(view))?;
        let mut angles = super::view_transition::FixedViewAngles::capture(view);
        angles.yaw = (angles.yaw & 0x00FF) | (u16::from(opposite) << 8);
        angles.write_to(view);
        Ok(())
    }

    fn primary_records(
        &mut self,
    ) -> Result<&mut super::scene_path_world::PlayerPathRecords, MapHostError> {
        let owner = self
            .world
            .primary_player
            .ok_or(MapHostError::MissingPrimaryPlayer)?;
        self.world
            .player_mut(self.objects, owner)
            .map_err(MapHostError::PlayerRecords)
    }

    fn declare_path_record(&mut self, record: PathRecord) -> Result<(), MapHostError> {
        let shape_extent = record
            .shape
            .catalog_entry()
            .ok_or(MapHostError::MissingShapeExtent(record.shape))?
            .size;
        let group = self
            .world
            .region_groups
            .ok_or(MapHostError::MissingRegionGroups)?
            .current;
        let records = self
            .world
            .map_records
            .as_mut()
            .ok_or(MapHostError::MissingMapRecords)?;
        // An exhausted pool skips the record (`$03:9575`).
        let _ = records.allocate(MapRecord {
            position: record.position,
            rotation: [super::Angle::ZERO, record.yaw, super::Angle::ZERO],
            shape: record.shape,
            shape_extent,
            // The native kind is a label; path records are classified as
            // enemies, like map-spawned path actors.
            kind: super::ObjectKind::Enemy,
            program: MapProgram::Path(record.path),
            flags: MapRecordFlags(MapRecordFlags::PATH),
            spawned: None,
            group,
        });
        Ok(())
    }
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
            MapCondition::ExternalEvent => self
                .world
                .scenario_flags
                .map(|flags| flags & 0x0400 != 0)
                .ok_or(MapHostError::MissingScenarioFlags),
            MapCondition::EncounterLayoutOdd => self
                .world
                .scene
                .encounter_layout
                .map(|current| current & 0x01 != 0)
                .ok_or(MapHostError::MissingEncounterLayout),
            MapCondition::SceneEvent => self
                .world
                .scene_events
                .map(|events| events.bits & 0x2000 != 0)
                .ok_or(MapHostError::MissingSceneEvents),
            MapCondition::EncounterLayout(layout) => self
                .world
                .scene
                .encounter_layout
                .map(|current| current == layout)
                .ok_or(MapHostError::MissingEncounterLayout),
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
            MapEffect::SaveContinuation(target) => self.continuation = Some(Some(target)),
            MapEffect::SaveUnrunnableContinuation => self.continuation = Some(None),
            MapEffect::CameraProjectionBase(bias) => self.world.camera_projection_base = Some(bias),
            MapEffect::Presentation(byte, value) => match byte {
                PresentationByte::SceneStyle => presentation.scene_style = Some(value),
                PresentationByte::TitleLayout => presentation.title_layout = Some(value),
                PresentationByte::PlayerCountLatch => presentation.player_count_latch = Some(value),
                PresentationByte::BackdropProgram => presentation.backdrop_program = Some(value),
            },
            MapEffect::InstallPath(_) | MapEffect::ActorHitPoints(_) | MapEffect::ActorAttackPower(_) => {
                return Err(MapHostError::ActorEffectWithoutActor);
            }
            MapEffect::DeclarePathRecord(record) => self.declare_path_record(record)?,
            MapEffect::RegisterRegion { index, region } => self
                .world
                .map_regions
                .as_mut()
                .ok_or(MapHostError::MissingRegions)?
                .register(index, region)
                .map_err(MapHostError::Region)?,
            MapEffect::PlayerVerticalProfile(profile) => {
                *self
                    .primary_records()?
                    .vertical
                    .as_mut()
                    .map(|vertical| &mut vertical.profile)
                    .ok_or(MapHostError::MissingPlayerVertical)? = profile;
            }
            MapEffect::CameraPitchProfile(profile) => {
                if self.world.primary_player.is_some() {
                    self.primary_records()?
                        .camera_angles
                        .as_mut()
                        .ok_or(MapHostError::MissingPlayerCameraAngles)?
                        .profile = profile;
                }
            }
            MapEffect::OccupancyExempt(exempt) => {
                self.primary_records()?.occupancy_exempt = Some(exempt);
            }
            MapEffect::CameraHeightLimit(half, height) => {
                let limits = self
                    .world
                    .camera_height_limits
                    .as_mut()
                    .ok_or(MapHostError::MissingCameraHeightLimits)?;
                match half {
                    HeightLimit::Top => limits.0 = height,
                    HeightLimit::Bottom => limits.1 = height,
                }
            }
            MapEffect::EnvironmentPlane(height) => self.world.environment_plane_height = Some(height),
            MapEffect::RenderPlane(height) => {
                self.world.render_environment.plane_height = Some(height)
            }
            MapEffect::PlayerCarryMode(mode) => self.world.player_carry_mode = Some(mode),
            MapEffect::PlayerConfiguration(configuration) => {
                self.world.scene.player_configuration = Some(configuration)
            }
            MapEffect::StreamingRadiusLimit(limit) => self.world.streaming_radius_limit = Some(limit),
            MapEffect::PlayerConfigurationVariant(variant) => {
                self.world.scene.player_configuration_variant = Some(variant)
            }
            MapEffect::PlacementCoordinate(axis, value) => {
                let placement = &mut self.world.map_placement;
                *match axis {
                    super::path_fields::Axis::X => &mut placement.x,
                    super::path_fields::Axis::Y => &mut placement.y,
                    super::path_fields::Axis::Z => &mut placement.z,
                } = Some(value);
            }
            MapEffect::PlacementHeading(heading) => self.world.map_placement.heading = Some(heading),
            MapEffect::PlacePrimaryPlayer => self.place_primary_player()?,
            MapEffect::ExitSceneSelection(ExitScene::Primary, selection) => {
                self.world.stage_exit.primary_scene = Some(selection)
            }
            MapEffect::ExitSceneSelection(ExitScene::Alternate, selection) => {
                self.world.stage_exit.alternate_scene = Some(selection)
            }
            MapEffect::StageExitScript(script) => self.world.stage_exit.script = Some(script),
            MapEffect::DeferredSceneLoad(load) => self.world.stage_exit.deferred_load = Some(load),
            MapEffect::AltitudeGaugeScale(scale) => presentation.altitude_gauge_scale = Some(scale),
            MapEffect::LinkPilotTransitions => self.for_each_stage_player(|records| {
                records
                    .mode_selection
                    .as_mut()
                    .map(|selection| selection.transition_control |= 0x40)
                    .ok_or(MapHostError::MissingModeSelection)
            })?,
            MapEffect::LinkControlledPilots => self.for_each_stage_player(|records| {
                records
                    .controlled_flags
                    .as_mut()
                    .map(|flags| flags.linked = true)
                    .ok_or(MapHostError::MissingControlledFlags)
            })?,
            MapEffect::EncounterVariant(variant) => {
                self.world
                    .campaign
                    .as_mut()
                    .ok_or(MapHostError::MissingCampaign)?
                    .encounter_variant = variant
            }
            MapEffect::StageExitMode(mode) => self.world.stage_exit.mode = Some(mode),
            MapEffect::ModeFlags { bits, set } => {
                let mode = self
                    .world
                    .view_transition_mode
                    .as_mut()
                    .ok_or(MapHostError::MissingExecutionMode)?;
                if set {
                    mode.flags |= bits;
                } else {
                    mode.flags &= !bits;
                }
            }
            MapEffect::SelectBackdropTable => presentation.backdrop_table_selected = true,
            MapEffect::RegionMarker { region, drawn } => {
                let bit = 1u128 << (region & 0x7F);
                presentation.region_markers_written |= bit;
                if drawn {
                    presentation.region_markers_drawn |= bit;
                } else {
                    presentation.region_markers_drawn &= !bit;
                }
            }
            MapEffect::GsuParameter(parameter, value) => match parameter {
                GsuParameter::BackdropColor => presentation.backdrop_color = Some(value),
                GsuParameter::PatternOffset => presentation.pattern_offset = Some(value),
                GsuParameter::PatternMode => presentation.pattern_mode = Some(value as u8),
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

    fn apply_to_current(&mut self, actor: ObjectId, effect: &MapEffect) -> Result<(), Self::Error> {
        let object = self
            .objects
            .get_mut(actor)
            .ok_or(MapHostError::MissingCurrentActor(actor))?;
        match *effect {
            MapEffect::InstallPath(path) => object.base.path = Some(path),
            MapEffect::ActorHitPoints(value) => object.base.hit_points = value,
            MapEffect::ActorAttackPower(value) => object.base.attack_power = value,
            _ => return self.apply(effect),
        }
        Ok(())
    }

    fn spawn(&mut self, specification: &MapSpawn) -> Result<Option<ObjectId>, Self::Error> {
        let defaults = self
            .world
            .spawn_defaults()
            .ok_or(MapHostError::MissingSpawnDefaults)?;
        Ok(match *specification {
            MapSpawn::Actor(spawn) => allocate_map_actor(self.objects, defaults, spawn),
            MapSpawn::PathActor { shape, position, entry } => {
                // The native kind is a label; path actors are classified as
                // enemies until a reviewed path proves otherwise.
                let spawn = MapActorSpawn {
                    kind: super::ObjectKind::Enemy,
                    shape,
                    behavior: super::Behavior::FollowPath,
                    position,
                };
                let created = allocate_map_actor(self.objects, defaults, spawn);
                if let Some(id) = created {
                    let state = &mut self
                        .objects
                        .get_mut(id)
                        .expect("fresh map actor")
                        .extension
                        .path_state;
                    state.needs_path_initialization = true;
                    state.default_combat_on_entry = entry == PathEntry::DefaultCombat;
                }
                created
            }
        })
    }
}

/// Visit the map once with the given world, then publish a continuation it
/// saved to the map owner (`SceneMap::save_continuation`).
pub fn visit(
    map: &mut SceneMap,
    catalog: &MapCatalog<'_, MapEffect, MapSpawn>,
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: Option<&mut super::program_resources::ProgramResources<super::program_state::ProgramData>>,
    presentation: &mut MapPresentation,
    budget: usize,
) -> Result<super::scene_map::MapReport, super::scene_map::MapError<MapHostError>> {
    let mut host = MapWorld {
        objects,
        world,
        resources,
        presentation,
        continuation: None,
    };
    let report = map.visit(catalog, &mut host, Default::default(), budget)?;
    match host.continuation {
        Some(Some(target)) => map
            .save_continuation(catalog, target)
            .map_err(|_| super::scene_map::MapError::Faulted)?,
        Some(None) => map
            .save_unrunnable_continuation()
            .map_err(|_| super::scene_map::MapError::Faulted)?,
        None => {}
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::super::map_streaming::{MapRecordStore, MapRegions, RegionGroups, NO_REGION};
    use super::super::{authored_paths, Angle, RandomState, ShapeId, Vector3};
    use super::*;

    fn world() -> ScenePathWorld {
        let mut world = ScenePathWorld::new(RandomState::default());
        world.map_records = Some(MapRecordStore::new());
        world.map_regions = Some(MapRegions::default());
        world.region_groups = Some(RegionGroups {
            current: 3,
            previous: NO_REGION,
        });
        world
    }

    #[test]
    fn path_records_join_the_current_region_with_the_catalog_extent() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut presentation = MapPresentation::default();
        let mut host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        let record = PathRecord {
            position: Vector3 { x: 2048, y: 512, z: -2048 },
            yaw: Angle::from_units(0x40),
            shape: ShapeId::TITLE_CRAFT,
            path: authored_paths::CONTACT_SUPPRESSED_ATTACHMENT,
        };
        host.apply(&MapEffect::DeclarePathRecord(record)).unwrap();
        let records = world.map_records.as_ref().unwrap();
        let stored = records.get(records.active_ids()[0]).unwrap();
        assert_eq!(stored.group, 3);
        assert_eq!(stored.position, record.position);
        assert_eq!(stored.rotation, [Angle::ZERO, record.yaw, Angle::ZERO]);
        assert_eq!(stored.shape_extent, ShapeId::TITLE_CRAFT.catalog_entry().unwrap().size);
        assert_eq!(stored.program, MapProgram::Path(record.path));
        assert_eq!(stored.flags, MapRecordFlags(MapRecordFlags::PATH));
    }

    #[test]
    fn regions_count_in_order_and_unwritten_slots_fault() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut presentation = MapPresentation::default();
        let mut host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        let region = MapRegion {
            origin_x: 0xFC00,
            origin_z: 0,
            width: 0x800,
            depth: 0xC00,
            entry: None,
        };
        host.apply(&MapEffect::RegisterRegion { index: 1, region }).unwrap();
        let regions = world.map_regions.as_ref().unwrap();
        assert_eq!(regions.count(), 1);
        assert_eq!(regions.regions(), Err(RegionError::Unwritten(0)));
    }

    #[test]
    fn layout_branches_read_the_encounter_layout() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut presentation = MapPresentation::default();
        let host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        assert_eq!(
            host.condition(MapCondition::EncounterLayout(2)),
            Err(MapHostError::MissingEncounterLayout)
        );
        world.scene.encounter_layout = Some(2);
        let host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        assert_eq!(host.condition(MapCondition::EncounterLayout(2)), Ok(true));
        assert_eq!(host.condition(MapCondition::EncounterLayout(3)), Ok(false));
    }

    #[test]
    fn shared_scene_stores_write_their_owners_and_player_calls_need_a_player() {
        let mut objects = ObjectStore::new();
        let mut world = world();
        world.camera_height_limits = Some((0, 0));
        let mut presentation = MapPresentation::default();
        let mut host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        host.apply(&MapEffect::CameraHeightLimit(HeightLimit::Top, -600)).unwrap();
        host.apply(&MapEffect::CameraHeightLimit(HeightLimit::Bottom, 15)).unwrap();
        host.apply(&MapEffect::StreamingRadiusLimit(4000)).unwrap();
        // $06:9A5F skips without a primary player; $06:9A2F and $06:9B04 do not.
        let camera = super::super::player_camera_angles::CameraPitchProfile { up: 16, down: -16 };
        host.apply(&MapEffect::CameraPitchProfile(camera)).unwrap();
        assert_eq!(
            host.apply(&MapEffect::OccupancyExempt(true)),
            Err(MapHostError::MissingPrimaryPlayer)
        );
        assert_eq!(world.camera_height_limits, Some((-600, 15)));
        assert_eq!(world.streaming_radius_limit, Some(4000));
    }

    #[test]
    fn primary_player_placement_needs_every_placement_store() {
        use super::super::path_fields::Axis;
        let mut objects = ObjectStore::new();
        let mut world = world();
        let mut presentation = MapPresentation::default();
        let mut host = MapWorld {
            objects: &mut objects,
            world: &mut world,
            resources: None,
            presentation: &mut presentation,
            continuation: None,
        };
        host.apply(&MapEffect::PlacementCoordinate(Axis::X, 256)).unwrap();
        host.apply(&MapEffect::PlacementCoordinate(Axis::Y, -140)).unwrap();
        host.apply(&MapEffect::PlacementCoordinate(Axis::Z, 20)).unwrap();
        assert_eq!(host.apply(&MapEffect::PlacePrimaryPlayer), Err(MapHostError::IncompletePlacement));
        host.apply(&MapEffect::PlacementHeading(0x40)).unwrap();
        assert_eq!(host.apply(&MapEffect::PlacePrimaryPlayer), Err(MapHostError::MissingPrimaryPlayer));
        assert_eq!(
            world.map_placement,
            MapPlacement { x: Some(256), y: Some(-140), z: Some(20), heading: Some(0x40) }
        );
    }

    #[test]
    fn scene_launchers_load_select_and_park_at_their_phase_gate() {
        use super::super::scene_map::{MapStop, SceneMap};
        use super::super::{attract_stage, authored_maps};
        let catalog = authored_maps::catalog().unwrap();
        for (root, load, selection) in [
            (authored_maps::SCENE_EIGHT_LAUNCHER, 0x1B, 8),
            (authored_maps::SCENE_FOUR_LAUNCHER, 0x57, 25),
            (authored_maps::SCENE_FIVE_LAUNCHER, 0x99, 5),
            (authored_maps::SCENE_THREE_LAUNCHER, 0xAB, 3),
            (authored_maps::SCENE_ONE_LAUNCHER, 0xAB, 1),
            (authored_maps::SCENE_ONE_ALTERNATE_LAUNCHER, 0x8D, 1),
            (authored_maps::SCENE_TWENTY_EIGHT_LAUNCHER, 0x8D, 28),
            (authored_maps::SCENE_TWENTY_FIVE_LAUNCHER, 0x57, 25),
            (authored_maps::SCENE_TWENTY_EIGHT_CHAINED_LAUNCHER, 0x57, 28),
            (authored_maps::SCENE_TWENTY_SIX_LAUNCHER, 0x5D, 26),
        ] {
            let mut objects = ObjectStore::new();
            let mut world = attract_stage::boot_world();
            world.campaign = Some(super::super::path_program::CampaignPathInputs {
                difficulty: super::super::Difficulty::Normal,
                encounter_variant: 0,
                secondary_variant: 0,
            });
            let mut presentation = MapPresentation {
                display_ready: Some(true),
                load_table_idle: Some(true),
                ..Default::default()
            };
            let mut map = SceneMap::new(&catalog, root).unwrap();
            let report = visit(&mut map, &catalog, &mut objects, &mut world, None, &mut presentation, 64)
                .unwrap();
            assert_eq!(report.stop, MapStop::Yielded(0x1388));
            assert_eq!(presentation.scene_load, Some(load));
            assert_eq!(world.scene_selection, Some(selection));
            if root == authored_maps::SCENE_TWENTY_SIX_LAUNCHER {
                assert_eq!(map.saved_continuation(), None);
                assert_eq!(
                    map.restore_continuation(),
                    Err(super::super::scene_map::MapRestoreError::UnrunnableContinuation)
                );
            } else {
                assert!(map.saved_continuation().is_some());
            }
            if root == authored_maps::SCENE_FOUR_LAUNCHER {
                assert_eq!(world.campaign.unwrap().encounter_variant, 4);
            }
        }
    }
}
