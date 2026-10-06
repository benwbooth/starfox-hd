//! Complete native opening-scene actor traversal.
//!
//! Every authored actor, including attached children and destruction effects,
//! occupies one entry in the same source-sized pool.  New path children are
//! inserted directly after their spawner and can therefore run later in the
//! current traversal.  Cleanup remains a separate, deferred pass.

use super::cinematic_exit::{
    CinematicExit, CinematicExitPolicy, CinematicExitVisit, CinematicSignals, OPENING_INPUT_HOLD,
};
use super::intro_attached_craft::{
    opening_burst, OpeningAttachedCraft, OpeningBurstAudio, OpeningBurstParticle,
    OpeningCraftFlare, OpeningDepartingCraft,
};
use super::intro_camera::{IntroCameraView, OpeningCameraRig};
use super::intro_chain::{
    OpeningChainControls, OpeningChainPart, OpeningChainPhase, OpeningChainSegment,
};
use super::intro_controller::{
    IntroColor, OpeningSceneController, OpeningScenePalette, INTRO_PALETTE_COLORS,
};
use super::intro_destruction::{
    IntroDestructionCapacityError, IntroDestructionContext, IntroDestructionEffects,
    IntroExplosionActor, IntroExplosionPhase, IntroExplosionProfile, IntroExplosionVolume,
};
use super::intro_flyby::{OpeningFlybyRig, OpeningFlybyStreak};
use super::intro_formation::{OpeningFormationAudio, OpeningFormationCraft, OpeningFormationPhase};
use super::intro_free_craft::{IntroAuxiliaryEffect, OpeningFreeCraft, OpeningFreeCraftPhase};
use super::intro_late_target::{OpeningLateCameraTarget, OpeningLateTargetEffect};
use super::intro_logo::{
    LogoActorPhase, LogoLayer, LogoSceneScroll, LogoSweepPhase, NintendoLogoActor,
    NintendoLogoAssembly, NintendoLogoOutline, NintendoLogoSweep,
};
use super::intro_material::SceneLighting;
use super::intro_motion::{IntroAttachment, IntroPlayerAnchor, IntroScenePose};
use super::intro_root::{
    OpeningAttachmentGroup, OpeningBackgroundOrigin, OpeningRootActor, OpeningRootEvent,
    OpeningRootSpawn, OpeningSceneRoot, OpeningSpawnPlacement,
};
use super::intro_second_flyby_craft::{
    OpeningSecondFlybyChild, OpeningSecondFlybyEvent, OpeningSecondFlybySpawn,
    OpeningSecondFlybySpawnPlacement,
};
use super::intro_second_flyby_scene::OpeningSecondFlybyActor;
use super::intro_second_flyby_wings::{OpeningAttachedWing, OpeningDepartingWing};
use super::object::{
    Behavior, Object, ObjectId, ObjectKind, ObjectLifetimeId, ObjectStore, ShapeId, Vector3,
    OBJECT_CAPACITY,
};
use super::render::{MaterialSetId, Rotation};
use super::scene_artwork::{
    ArtworkLoadPhase, ArtworkPublication, ArtworkResume, ForegroundSelection, OpeningArtworkLoad,
    SceneArtwork,
};
use super::scene_frame::NormalFrameBuffers;
use super::scene_video::{SceneLayerPolicy, SceneModePublication, SceneModeSetup, SceneVideo};
use super::state::{InterleavedRandom, RandomSource, RandomState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningArtworkRequestError {
    AlreadyLoading,
    NotStarted,
    MissingLayerPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeferredOpeningArtwork {
    artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
    skip_background_palette: bool,
    include_mode_setup: bool,
}

/// One independently scheduled member of the opening's shared actor pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSceneActor {
    /// The boot-created first player owns the parallel scene controller.
    Controller,
    /// The boot-created second player remains at the active-list tail.
    InactivePlayer,
    Root(OpeningSceneRoot),
    Camera(OpeningCameraRig),
    CameraTarget(super::intro_target::OpeningCameraTarget),
    LogoAssembly(NintendoLogoAssembly),
    LogoGlyph(NintendoLogoActor),
    LogoOutline {
        parent: ObjectId,
        actor: NintendoLogoOutline,
    },
    LogoSweep(NintendoLogoSweep),
    FlybyRig(OpeningFlybyRig),
    FlybyStreak {
        owner: ObjectId,
        actor: OpeningFlybyStreak,
    },
    AttachedCraft(OpeningAttachedCraft),
    DepartingCraft(OpeningDepartingCraft),
    CraftFlare {
        owner: ObjectId,
        actor: OpeningCraftFlare,
    },
    Burst(OpeningBurstParticle),
    FreeCraft(OpeningFreeCraft),
    FormationCraft(OpeningFormationCraft),
    LateCameraTarget(OpeningLateCameraTarget),
    LateTargetEffect {
        parent: ObjectId,
        actor: OpeningLateTargetEffect,
    },
    SecondFlyby(OpeningSecondFlybyActor),
    Explosion(IntroExplosionActor),
}

impl OpeningSceneActor {
    /// Authored controls consumed before camera-relative draw preparation.
    /// The far-order control is an additive bias, not an absolute camera Z.
    pub fn draw_controls(&self) -> OpeningDrawControls {
        let mut controls = OpeningDrawControls::default();
        match self {
            Self::LogoGlyph(actor) => {
                controls.depth_offset = actor.depth_offset;
                controls.material_override = actor.material_override;
                controls.texture_scroll_y = actor.texture_scroll_y;
            }
            Self::LogoOutline { actor, .. } => {
                controls.material_override = Some(actor.material);
            }
            Self::LogoSweep(_) => {
                controls.sort_bias = 15_000;
            }
            Self::FlybyStreak { actor, .. } => {
                controls.sort_bias = actor.depth_order.sort_depth_override().unwrap_or(0);
            }
            Self::DepartingCraft(actor) => {
                controls.sort_bias = actor.sort_depth_override().unwrap_or(0);
                controls.depth_offset = actor.depth_offset();
            }
            Self::CraftFlare { actor, .. } => {
                controls.sort_bias = actor.sort_depth_override().unwrap_or(0);
            }
            Self::FreeCraft(actor) => {
                controls.sort_bias = actor.sort_depth_override().unwrap_or(0);
            }
            Self::Burst(actor) => {
                controls.color_frame = OpeningAnimationFrame::Authored(actor.color_frame);
            }
            Self::Explosion(actor) => {
                controls.color_frame = OpeningAnimationFrame::Authored(actor.color_frame);
            }
            Self::SecondFlyby(actor) => match actor {
                OpeningSecondFlybyActor::Chain(actor) => {
                    controls.sort_bias = if actor.sort_override() { 15_000 } else { 0 };
                    controls.depth_offset = actor.depth_offset as u8;
                }
                OpeningSecondFlybyActor::Flare(actor) => {
                    controls.sort_bias = if actor.sort_override() { 15_000 } else { 0 };
                }
                OpeningSecondFlybyActor::Craft(actor) => {
                    controls.sort_bias = 15_000;
                    if actor.animation_enabled {
                        controls.shape_frame =
                            OpeningAnimationFrame::Authored(actor.animation_frame);
                    }
                }
                OpeningSecondFlybyActor::ChainBurst(actor) => {
                    if actor.is_sprite() {
                        controls.color_frame = OpeningAnimationFrame::Authored(actor.color_frame);
                    }
                }
                OpeningSecondFlybyActor::Explosion(actor) => {
                    controls.color_frame = OpeningAnimationFrame::Authored(actor.color_frame);
                }
                OpeningSecondFlybyActor::Trail { .. }
                | OpeningSecondFlybyActor::CameraTarget(_)
                | OpeningSecondFlybyActor::AttachedWing(_)
                | OpeningSecondFlybyActor::DepartingWing(_) => {}
            },
            Self::Controller
            | Self::InactivePlayer
            | Self::Root(_)
            | Self::Camera(_)
            | Self::CameraTarget(_)
            | Self::LogoAssembly(_)
            | Self::FlybyRig(_)
            | Self::AttachedCraft(_)
            | Self::FormationCraft(_)
            | Self::LateCameraTarget(_)
            | Self::LateTargetEffect { .. } => {}
        }
        controls
    }

    pub fn pose(&self) -> IntroScenePose {
        match self {
            Self::Controller | Self::InactivePlayer => IntroScenePose::default(),
            Self::Root(actor) => actor.pose,
            Self::Camera(actor) => IntroScenePose {
                position: actor.position,
                ..Default::default()
            },
            Self::CameraTarget(actor) => actor.pose,
            Self::LogoAssembly(actor) => IntroScenePose {
                position: actor.position(),
                ..Default::default()
            },
            Self::LogoGlyph(actor) => IntroScenePose {
                position: actor.position,
                rotation: actor.rotation,
            },
            Self::LogoOutline { actor, .. } => IntroScenePose {
                position: actor.position,
                rotation: actor.rotation,
            },
            Self::LogoSweep(actor) => IntroScenePose {
                position: actor.position,
                rotation: actor.rotation,
            },
            Self::FlybyRig(actor) => actor.pose,
            Self::FlybyStreak { actor, .. } => actor.pose,
            Self::AttachedCraft(actor) => actor.pose,
            Self::DepartingCraft(actor) => actor.pose,
            Self::CraftFlare { actor, .. } => actor.pose,
            Self::Burst(actor) => actor.pose,
            Self::FreeCraft(actor) => actor.pose,
            Self::FormationCraft(actor) => actor.pose,
            Self::LateCameraTarget(actor) => actor.pose,
            Self::LateTargetEffect { actor, .. } => actor.pose,
            Self::SecondFlyby(actor) => actor.pose(),
            Self::Explosion(actor) => IntroScenePose {
                position: actor.position,
                ..Default::default()
            },
        }
    }

    pub fn shape(&self) -> ShapeId {
        match self {
            Self::Controller
            | Self::InactivePlayer
            | Self::Root(_)
            | Self::Camera(_)
            | Self::CameraTarget(_)
            | Self::LogoAssembly(_)
            | Self::FlybyRig(_)
            | Self::LateCameraTarget(_) => ShapeId::EMPTY,
            Self::LogoGlyph(actor) => actor.glyph.shape(),
            Self::LogoOutline { .. } => NintendoLogoOutline::SHAPE,
            Self::LogoSweep(_) => NintendoLogoSweep::SHAPE,
            Self::FlybyStreak { actor, .. } => actor.shape.unwrap_or(ShapeId::EMPTY),
            Self::AttachedCraft(actor) => actor.shape(),
            Self::DepartingCraft(actor) => actor.shape,
            Self::CraftFlare { actor, .. } => actor.shape(),
            Self::Burst(actor) => actor.shape,
            Self::FreeCraft(actor) => actor.shape(),
            Self::FormationCraft(actor) => actor.shape(),
            Self::LateTargetEffect { actor, .. } => actor.shape(),
            Self::SecondFlyby(actor) => actor.shape(),
            Self::Explosion(actor) => actor.shape(),
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Self::Controller
            | Self::InactivePlayer
            | Self::Root(_)
            | Self::Camera(_)
            | Self::CameraTarget(_)
            | Self::LogoAssembly(_)
            | Self::FlybyRig(_)
            | Self::LateCameraTarget(_) => false,
            Self::LogoGlyph(actor) => actor.is_visible(),
            Self::LogoOutline { actor, .. } => actor.is_visible(),
            Self::LogoSweep(actor) => actor.phase() != LogoSweepPhase::Finished,
            Self::FlybyStreak { actor, .. } => actor.is_visible(),
            Self::AttachedCraft(actor) => actor.is_visible(),
            Self::DepartingCraft(actor) => actor.is_visible(),
            Self::CraftFlare { actor, .. } => actor.is_visible(),
            Self::Burst(actor) => !actor.is_finished(),
            Self::FreeCraft(actor) => actor.is_visible(),
            Self::FormationCraft(actor) => actor.is_visible(),
            Self::LateTargetEffect { actor, .. } => actor.is_visible(),
            Self::SecondFlyby(actor) => actor.is_visible(),
            Self::Explosion(actor) => !actor.is_finished() && actor.shape() != ShapeId::EMPTY,
        }
    }

    fn eligible_for_pressure_retirement(&self) -> bool {
        matches!(self, Self::Burst(_) | Self::Explosion(_))
            || matches!(self, Self::SecondFlyby(actor) if actor.eligible_for_pressure_retirement())
    }
}

/// Actor-authored draw inputs. Depth-color selection and texture scrolling
/// are independent of geometric sort bias. Clipping and sprite appearance
/// are separate controls and are not represented by this subset.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningDrawControls {
    pub sort_bias: i16,
    /// Only the low byte is submitted, even when an actor updates a word.
    pub depth_offset: u8,
    pub material_override: Option<MaterialSetId>,
    pub texture_scroll_y: u8,
    pub shape_frame: OpeningAnimationFrame,
    pub color_frame: OpeningAnimationFrame,
}

/// Shape and color animation independently choose the scene clock or an
/// actor-authored frame (`$7F:1406..141B`). They never use texture scroll Y.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OpeningAnimationFrame {
    #[default]
    SceneClock,
    Authored(u8),
}

impl OpeningAnimationFrame {
    pub const fn resolve(self, scene_clock: u8) -> u8 {
        match self {
            Self::SceneClock => scene_clock & 127,
            Self::Authored(frame) => frame & 127,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningActorSnapshot {
    pub id: ObjectId,
    pub lifetime: ObjectLifetimeId,
    pub pose: IntroScenePose,
    pub shape: ShapeId,
    pub visible: bool,
    pub draw_controls: OpeningDrawControls,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningSceneFrameEvents {
    /// Consumer draws only; background entropy refreshes are not actor draws.
    pub random_draws: usize,
    /// Artwork published at the joined normal-frame boundary. This preserves
    /// loader order but does not assign display times to those publications.
    pub artwork_publications: Vec<ArtworkPublication>,
    pub scene_mode_publication: Option<SceneModePublication>,
    pub root_events: Vec<OpeningRootEvent>,
    pub second_flyby_events: Vec<OpeningSecondFlybyEvent>,
    pub spawned: Vec<ObjectId>,
    pub retired: Vec<ObjectId>,
    pub selected_camera_target: Option<ObjectId>,
    pub explosion_audio: Vec<IntroExplosionVolume>,
    pub burst_audio: Vec<OpeningBurstAudio>,
    pub formation_audio: Vec<OpeningFormationAudio>,
    pub free_craft_departure_audio: u8,
    pub flyby_audio: u8,
    pub allocation_pressure: bool,
}

/// Native opening state at source update granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningScene {
    objects: ObjectStore,
    actors: [Option<OpeningSceneActor>; OBJECT_CAPACITY],
    retiring: [bool; OBJECT_CAPACITY],
    root: ObjectId,
    controller_actor: ObjectId,
    inactive_player: ObjectId,
    controller: OpeningSceneController,
    exit: CinematicExit,
    palette: OpeningScenePalette,
    lighting: SceneLighting,
    video: SceneVideo,
    layer_policy: Option<SceneLayerPolicy>,
    artwork: SceneArtwork,
    artwork_load: Option<OpeningArtworkLoad>,
    deferred_artwork: Option<DeferredOpeningArtwork>,
    foreground_selection: ForegroundSelection,
    frame_buffers: NormalFrameBuffers,
    random: RandomState,
    camera: IntroCameraView,
    camera_target: Option<ObjectId>,
    auxiliary: IntroAuxiliaryEffect,
    chain_controls: OpeningChainControls,
    global_clock: u8,
    scene_depth_velocity: i16,
    logo_released: bool,
    background_origin: OpeningBackgroundOrigin,
    player_anchor: IntroPlayerAnchor,
}

impl Default for OpeningScene {
    fn default() -> Self {
        Self::new(
            RandomState::default(),
            OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
        )
    }
}

impl OpeningScene {
    pub fn new(random: RandomState, palette: OpeningScenePalette) -> Self {
        let mut objects = ObjectStore::new();
        let controller_actor = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Effect,
            ))
            .expect("opening controller fits the empty source pool");
        let inactive_player = objects
            .allocate_after(
                Some(controller_actor),
                Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect),
            )
            .expect("opening inactive player fits the source pool");
        let root = objects
            .allocate_after(
                Some(controller_actor),
                Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect),
            )
            .expect("opening root fits the source pool");
        debug_assert_eq!(controller_actor.index(), 0);
        debug_assert_eq!(inactive_player.index(), 1);
        debug_assert_eq!(root.index(), 2);
        let mut actors = [None; OBJECT_CAPACITY];
        actors[controller_actor.index()] = Some(OpeningSceneActor::Controller);
        actors[inactive_player.index()] = Some(OpeningSceneActor::InactivePlayer);
        actors[root.index()] = Some(OpeningSceneActor::Root(OpeningSceneRoot::default()));
        Self {
            objects,
            actors,
            retiring: [false; OBJECT_CAPACITY],
            root,
            controller_actor,
            inactive_player,
            controller: OpeningSceneController::default(),
            exit: CinematicExit::new(OPENING_INPUT_HOLD),
            palette,
            lighting: SceneLighting::default(),
            video: SceneVideo::default(),
            layer_policy: None,
            artwork: SceneArtwork::default(),
            artwork_load: None,
            deferred_artwork: None,
            foreground_selection: ForegroundSelection::STANDARD,
            frame_buffers: NormalFrameBuffers::default(),
            random,
            camera: IntroCameraView::default(),
            camera_target: None,
            auxiliary: IntroAuxiliaryEffect::default(),
            chain_controls: OpeningChainControls::default(),
            global_clock: 0,
            scene_depth_velocity: 0,
            logo_released: false,
            background_origin: OpeningBackgroundOrigin {
                horizontal: 0,
                vertical: 0,
            },
            player_anchor: IntroPlayerAnchor::default(),
        }
    }

    pub fn root(&self) -> ObjectId {
        self.root
    }

    pub fn exit(&self) -> &CinematicExit {
        &self.exit
    }

    /// Outer scene-loop visit after the ordinary frame, not an actor or
    /// display update. The display owner must supply actual fade visits;
    /// this method does not infer them from elapsed actor updates.
    pub fn visit_exit(
        &mut self,
        pressed: super::Buttons,
        display: &mut super::scene_display::SceneDisplay,
        audio: &mut super::AudioState,
    ) -> CinematicExitVisit {
        let mut signals = CinematicSignals {
            exit_requested: self.controller.transition_requested,
            skip_ready: false,
        };
        let event = self.exit.visit(
            CinematicExitPolicy::OPENING,
            &mut signals,
            pressed,
            false,
            display,
            audio,
        );
        self.controller.transition_requested = signals.exit_requested;
        event
    }
    pub fn controller_actor(&self) -> ObjectId {
        self.controller_actor
    }
    pub fn inactive_player(&self) -> ObjectId {
        self.inactive_player
    }
    pub fn controller(&self) -> &OpeningSceneController {
        &self.controller
    }
    pub fn palette(&self) -> &OpeningScenePalette {
        &self.palette
    }
    pub fn artwork(&self) -> &SceneArtwork {
        &self.artwork
    }
    pub fn lighting(&self) -> SceneLighting {
        self.lighting
    }
    pub fn video(&self) -> &SceneVideo {
        &self.video
    }
    pub fn publish_display_output(&mut self, band: super::scene_display::DisplayBand) {
        self.video.publish_display(band);
    }
    /// Bind the scene's real inherited layer policy. This does not publish
    /// a layout or blank the display. Request preparation and the later
    /// display service sample different parts of this live policy.
    pub fn set_scene_layer_policy(&mut self, policy: SceneLayerPolicy) {
        self.layer_policy = Some(policy);
    }
    pub fn scene_layer_policy(&self) -> Option<SceneLayerPolicy> {
        self.layer_policy
    }
    pub fn artwork_load_phase(&self) -> Option<ArtworkLoadPhase> {
        self.artwork_load.as_ref().map(OpeningArtworkLoad::phase)
    }
    pub fn artwork_request_pending(&self) -> bool {
        self.deferred_artwork.is_some()
    }
    /// Logical frame-barrier roles, not a sample of the asynchronous display
    /// at controller entry. An upload can finish before that controller runs.
    pub fn frame_buffers(&self) -> &NormalFrameBuffers {
        &self.frame_buffers
    }
    pub fn set_foreground_selection(&mut self, selection: ForegroundSelection) {
        self.foreground_selection = selection;
    }
    /// Queue artwork for the ordinary scene's source-owned load barrier.
    /// The latest not-yet-started request replaces the previous request, as
    /// the source map's selected loader does. It cannot replace an active
    /// service sequence. Published assets and palette policy stay unchanged
    /// until the request is accepted; scene-mode/reset/postload work is not
    /// implied by this artwork-only request.
    pub fn queue_artwork_load(
        &mut self,
        artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
        skip_background_palette: bool,
    ) -> Result<(), OpeningArtworkRequestError> {
        if self
            .artwork_load_phase()
            .is_some_and(|phase| phase != ArtworkLoadPhase::Complete)
        {
            return Err(OpeningArtworkRequestError::AlreadyLoading);
        }
        self.deferred_artwork = Some(DeferredOpeningArtwork {
            artwork,
            skip_background_palette,
            include_mode_setup: false,
        });
        Ok(())
    }
    /// Queue common scene-load dispatch and the opening's mode/layout setup
    /// together with its artwork. Visibility changes only at acceptance.
    /// Layer policy must be bound explicitly; there is no guessed boot layout.
    /// This does not include unrelated scene-reset or postload services.
    pub fn queue_scene_presentation(
        &mut self,
        artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
        skip_background_palette: bool,
    ) -> Result<(), OpeningArtworkRequestError> {
        if self.layer_policy.is_none() {
            return Err(OpeningArtworkRequestError::MissingLayerPolicy);
        }
        self.queue_artwork_load(artwork, skip_background_palette)?;
        self.deferred_artwork.as_mut().unwrap().include_mode_setup = true;
        Ok(())
    }
    /// Called by the scene host when the source's standard artwork request is
    /// accepted, not by the actor controller at a prescribed update number.
    /// Existing assets remain published until their individual service events.
    pub fn begin_artwork_load(
        &mut self,
        artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
        skip_background_palette: bool,
    ) -> Result<(), OpeningArtworkRequestError> {
        if self.deferred_artwork.is_some()
            || self
                .artwork_load_phase()
                .is_some_and(|phase| phase != ArtworkLoadPhase::Complete)
        {
            return Err(OpeningArtworkRequestError::AlreadyLoading);
        }
        self.artwork.skip_next_background_palette = skip_background_palette;
        self.artwork_load = Some(OpeningArtworkLoad::new(artwork));
        Ok(())
    }
    /// Accept the standard loader at its real scene barrier. Large-character
    /// choice is captured now; layer/map policy is read again on publication.
    pub fn begin_scene_presentation(
        &mut self,
        artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
        skip_background_palette: bool,
    ) -> Result<(), OpeningArtworkRequestError> {
        let policy = self
            .layer_policy
            .ok_or(OpeningArtworkRequestError::MissingLayerPolicy)?;
        self.begin_artwork_load(artwork, skip_background_palette)?;
        self.video
            .request_setup(SceneModeSetup::OffsetTallMap, policy.artwork_plane)
            .expect("mode setup cannot outlive its active artwork load");
        Ok(())
    }
    /// Accept the opening through common load-table dispatch, which restores
    /// standard layer visibility before its selected loader runs. An invalid
    /// or overlapping request must not change the inherited layer policy.
    pub fn begin_scene_load_sequence(
        &mut self,
        artwork: std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork>,
        skip_background_palette: bool,
    ) -> Result<(), OpeningArtworkRequestError> {
        // Acceptance captures only the artwork plane, not visibility, and
        // cannot publish a service synchronously. Validate before changing
        // policy so a rejected request is fully non-mutating.
        self.begin_scene_presentation(artwork, skip_background_palette)?;
        self.layer_policy
            .as_mut()
            .expect("accepted scene layer policy")
            .begin_load_sequence();
        Ok(())
    }
    pub fn resume_artwork_load(
        &mut self,
        foreground: ForegroundSelection,
    ) -> Result<ArtworkResume, OpeningArtworkRequestError> {
        let handoff = self.artwork_load_phase() == Some(ArtworkLoadPhase::RenderHandoff);
        let result = self
            .artwork_load
            .as_mut()
            .map(|load| load.resume(foreground))
            .ok_or(OpeningArtworkRequestError::NotStarted)?;
        if handoff {
            self.lighting.finish_opening_load();
        }
        Ok(result)
    }
    /// The host calls this only at an artwork service boundary. This does not
    /// advance actors, palette effects or clocks. Standard artwork services
    /// force blanking; an accepted mode request publishes its layout with the
    /// setup palette and normal depth thresholds. Other resets remain open.
    pub fn publish_artwork(&mut self) -> Option<ArtworkPublication> {
        let publication = self
            .artwork_load
            .as_mut()?
            .publish(&mut self.artwork, &mut self.palette)?;
        self.video.publish_transfer_blank();
        if publication == ArtworkPublication::PolygonPalette {
            if self.video.setup_pending() {
                self.video
                    .publish_setup(self.layer_policy.expect("accepted scene layer policy"));
            }
            self.lighting.setup_scene();
        }
        Some(publication)
    }

    /// Project the artwork portion of the main loop's blocking load onto its
    /// joined frame boundary. The original does not run another actor while
    /// waiting for these publications. Keep the fine-grained service API for
    /// a future display owner; this does not simulate its interrupt timings,
    /// fades, palette effects or the loader's other scene-setup side effects.
    fn publish_frame_artwork(&mut self, events: &mut OpeningSceneFrameEvents) {
        if !self.frame_buffers.ready_for_scene_load() {
            return;
        }
        let Some(request) = self.deferred_artwork.take() else {
            return;
        };
        let include_mode = request.include_mode_setup;
        if include_mode {
            self.begin_scene_load_sequence(request.artwork, request.skip_background_palette)
        } else {
            self.begin_artwork_load(request.artwork, request.skip_background_palette)
        }
        .expect("a queued request cannot overlap an active loader or lose its layer policy");
        loop {
            match self.artwork_load_phase().expect("accepted artwork request") {
                ArtworkLoadPhase::Complete => break,
                ArtworkLoadPhase::Pending(_) => {
                    let publication = self.publish_artwork().expect("pending artwork publication");
                    if include_mode && publication == ArtworkPublication::PolygonPalette {
                        events.scene_mode_publication = self.video.last_setup();
                    }
                    events.artwork_publications.push(publication);
                }
                ArtworkLoadPhase::RequestBackground
                | ArtworkLoadPhase::SelectForeground
                | ArtworkLoadPhase::RequestSprites
                | ArtworkLoadPhase::RenderHandoff => {
                    self.resume_artwork_load(self.foreground_selection)
                        .expect("accepted artwork request");
                }
            }
        }
    }
    pub fn random(&self) -> RandomState {
        self.random
    }
    pub fn camera(&self) -> IntroCameraView {
        self.camera
    }
    /// Native view at this actor-update boundary. Opening camera actors use
    /// zero follow distance; render-deadline selection is the scheduler's job.
    pub fn render_view(&self) -> super::intro_draw::ViewTransform {
        super::intro_draw::ViewTransform::from_camera(self.camera, 0)
    }
    pub fn camera_target(&self) -> Option<ObjectId> {
        self.camera_target
    }
    pub fn auxiliary(&self) -> IntroAuxiliaryEffect {
        self.auxiliary
    }
    pub fn global_clock(&self) -> u8 {
        self.global_clock
    }
    pub fn background_origin(&self) -> OpeningBackgroundOrigin {
        self.background_origin
    }
    pub fn player_anchor(&self) -> IntroPlayerAnchor {
        self.player_anchor
    }
    pub fn available_slots(&self) -> usize {
        OBJECT_CAPACITY - self.objects.len()
    }
    pub fn lifetime(&self, id: ObjectId) -> Option<ObjectLifetimeId> {
        self.objects.lifetime_id(id)
    }
    pub fn actor(&self, id: ObjectId) -> Option<&OpeningSceneActor> {
        self.actors.get(id.index())?.as_ref()
    }
    pub fn actors(&self) -> impl Iterator<Item = (ObjectId, &OpeningSceneActor)> {
        self.objects.active_ids().iter().copied().map(|id| {
            (
                id,
                self.actor(id).expect("active opening slot has an actor"),
            )
        })
    }
    pub fn snapshots(&self) -> impl Iterator<Item = OpeningActorSnapshot> + '_ {
        self.actors().map(|(id, actor)| OpeningActorSnapshot {
            id,
            lifetime: self
                .lifetime(id)
                .expect("active opening actor has a lifetime"),
            pose: actor.pose(),
            shape: actor.shape(),
            visible: actor.is_visible(),
            draw_controls: actor.draw_controls(),
        })
    }

    fn synchronize(&mut self, id: ObjectId) {
        let actor = *self
            .actor(id)
            .expect("opening actor exists while synchronizing");
        let pose = actor.pose();
        let object = self
            .objects
            .get_mut(id)
            .expect("opening object exists while synchronizing");
        object.base.shape = actor.shape();
        object.base.position = pose.position;
        object.base.pitch = pose.rotation.pitch;
        object.base.yaw = pose.rotation.yaw;
        object.base.roll = pose.rotation.roll;
    }

    fn allocate(
        &mut self,
        after: ObjectId,
        actor: OpeningSceneActor,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<ObjectId, IntroDestructionCapacityError> {
        let id = self
            .objects
            .allocate_after(
                Some(after),
                Object::new(ObjectKind::Effect, actor.shape(), Behavior::Effect),
            )
            .ok_or(IntroDestructionCapacityError {
                required_slots: 1,
                available_slots: self.available_slots(),
            })?;
        self.actors[id.index()] = Some(actor);
        self.retiring[id.index()] = false;
        if self.available_slots() == 0 {
            events.allocation_pressure = true;
            let mut cursor = Some(after);
            while let Some(current) = cursor {
                cursor = self
                    .objects
                    .get(current)
                    .expect("live allocation anchor")
                    .base
                    .next;
                if cursor.is_none() {
                    break;
                }
                if current != id
                    && self
                        .actor(current)
                        .is_some_and(OpeningSceneActor::eligible_for_pressure_retirement)
                {
                    self.retiring[current.index()] = true;
                }
            }
        }
        self.synchronize(id);
        events.spawned.push(id);
        Ok(id)
    }

    fn destruction_context(&self) -> IntroDestructionContext {
        IntroDestructionContext {
            primary_listener: self.camera.position,
            available_slots: self.available_slots(),
            scroll: Vector3 {
                x: 0,
                y: 0,
                z: self.scene_depth_velocity,
            },
            ..Default::default()
        }
    }

    fn common_destruction(
        &mut self,
        id: ObjectId,
        shape: ShapeId,
        position: Vector3,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        let profile = IntroExplosionProfile::for_shape(shape)
            .expect("authored opening actor shape belongs to the catalog");
        let context = self.destruction_context();
        let (effects, audio) = IntroDestructionEffects::spawn(profile, position, &context)?;
        events.explosion_audio.extend(audio);
        let head = self.objects.active_ids()[0];
        for effect in effects.actors().copied() {
            self.allocate(head, OpeningSceneActor::Explosion(effect), events)?;
        }
        self.retiring[id.index()] = true;
        Ok(())
    }

    fn spawn_root_actor(
        &mut self,
        root: ObjectId,
        spawn: OpeningRootSpawn,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<ObjectId, IntroDestructionCapacityError> {
        let inherited = match spawn.placement {
            OpeningSpawnPlacement::Independent(pose) => pose,
            OpeningSpawnPlacement::Attached { local, .. } => {
                local.world_pose(self.actor(root).expect("opening root exists").pose())
            }
        };
        use OpeningRootActor as RootActor;
        let placeholder = match (spawn.actor, spawn.placement) {
            (RootActor::CameraTarget, _) => OpeningSceneActor::CameraTarget(
                // The actor identity is filled after allocation below.
                super::intro_target::OpeningCameraTarget::new(root),
            ),
            (RootActor::NintendoLogo, _) => {
                OpeningSceneActor::LogoAssembly(NintendoLogoAssembly::new(inherited.position))
            }
            (RootActor::Camera, _) => {
                OpeningSceneActor::Camera(OpeningCameraRig::new(inherited.position))
            }
            (RootActor::FlybyRig, OpeningSpawnPlacement::Attached { local, .. }) => {
                OpeningSceneActor::FlybyRig(OpeningFlybyRig::new(local))
            }
            (RootActor::AttachedCraft, OpeningSpawnPlacement::Attached { local, .. }) => {
                OpeningSceneActor::AttachedCraft(OpeningAttachedCraft::new(local))
            }
            (RootActor::FreeCraft, _) => {
                OpeningSceneActor::FreeCraft(OpeningFreeCraft::new(root, inherited))
            }
            (RootActor::FormationCraft(member), _) => OpeningSceneActor::FormationCraft(
                OpeningFormationCraft::new(root, member, inherited),
            ),
            (RootActor::SecondFlybyCraft, _) => {
                OpeningSceneActor::SecondFlyby(OpeningSecondFlybyActor::Craft(Default::default()))
            }
            (RootActor::SecondCameraTarget, _) => {
                OpeningSceneActor::LateCameraTarget(OpeningLateCameraTarget::new(inherited))
            }
            _ => unreachable!("root actor uses its authored placement kind"),
        };
        let id = self.allocate(root, placeholder, events)?;
        // Three actor types retain their own semantic identity.
        self.actors[id.index()] = Some(match self.actors[id.index()].unwrap() {
            OpeningSceneActor::CameraTarget(_) => {
                OpeningSceneActor::CameraTarget(super::intro_target::OpeningCameraTarget::new(id))
            }
            OpeningSceneActor::FreeCraft(_) => {
                OpeningSceneActor::FreeCraft(OpeningFreeCraft::new(id, inherited))
            }
            OpeningSceneActor::FormationCraft(_) => {
                let RootActor::FormationCraft(member) = spawn.actor else {
                    unreachable!()
                };
                OpeningSceneActor::FormationCraft(OpeningFormationCraft::new(id, member, inherited))
            }
            actor => actor,
        });
        self.synchronize(id);
        Ok(id)
    }

    fn handle_root_event(
        &mut self,
        root: ObjectId,
        event: OpeningRootEvent,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        match event {
            OpeningRootEvent::Initialize {
                background_origin,
                player_anchor,
                depth_velocity,
                ..
            } => {
                self.background_origin = background_origin;
                self.player_anchor = player_anchor;
                self.scene_depth_velocity = depth_velocity;
            }
            OpeningRootEvent::Spawn(spawn) => {
                self.spawn_root_actor(root, spawn, events)?;
            }
            OpeningRootEvent::QueueFlybyAudio => {
                events.flyby_audio = events.flyby_audio.saturating_add(1)
            }
            OpeningRootEvent::RemoveFirstAttachment(OpeningAttachmentGroup::FlybyRig) => {
                if let Some(id) = self
                    .objects
                    .active_ids()
                    .iter()
                    .copied()
                    .find(|id| matches!(self.actor(*id), Some(OpeningSceneActor::FlybyRig(_))))
                {
                    let Some(OpeningSceneActor::FlybyRig(rig)) = self.actors[id.index()] else {
                        unreachable!()
                    };
                    let mut rig = rig;
                    rig.request_removal();
                    self.actors[id.index()] = Some(OpeningSceneActor::FlybyRig(rig));
                }
            }
            OpeningRootEvent::RemoveFirstAttachment(OpeningAttachmentGroup::TrackingAndCraft) => {
                unreachable!("the authored opening removes only its flyby rig group")
            }
        }
        Ok(())
    }

    fn publish_root_attachments(&mut self, root_pose: IntroScenePose) {
        // Direct children are published first in active-list order.  Sibling
        // effects then inherit the newly published transform of their owner.
        for id in self.objects.active_ids().to_vec() {
            match self.actors[id.index()].as_mut() {
                Some(OpeningSceneActor::CameraTarget(actor)) => {
                    actor.publish_from_parent(root_pose)
                }
                Some(OpeningSceneActor::FlybyRig(actor)) => actor.publish_from_parent(root_pose),
                Some(OpeningSceneActor::AttachedCraft(actor)) => {
                    actor.publish_from_parent(root_pose)
                }
                _ => {}
            }
            self.synchronize(id);
        }
        for id in self.objects.active_ids().to_vec() {
            let next = match self.actors[id.index()] {
                Some(OpeningSceneActor::FlybyStreak { owner, mut actor }) => {
                    if let Some(owner) = self.actor(owner) {
                        actor.publish_from_owner(owner.pose());
                    }
                    Some(OpeningSceneActor::FlybyStreak { owner, actor })
                }
                Some(OpeningSceneActor::CraftFlare { owner, mut actor }) => {
                    if let Some(owner) = self.actor(owner) {
                        actor.publish_from_owner(owner.pose());
                    }
                    Some(OpeningSceneActor::CraftFlare { owner, actor })
                }
                _ => None,
            };
            if let Some(actor) = next {
                self.actors[id.index()] = Some(actor);
                self.synchronize(id);
            }
        }
    }

    fn spawn_flyby_streaks(
        &mut self,
        owner: ObjectId,
        attachments: [IntroAttachment; 3],
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        for attachment in attachments {
            self.allocate(
                owner,
                OpeningSceneActor::FlybyStreak {
                    owner,
                    actor: OpeningFlybyStreak::new(attachment),
                },
                events,
            )?;
        }
        Ok(())
    }

    fn publish_logo_outline(&mut self, parent: ObjectId, pose: IntroScenePose) {
        for id in self.objects.active_ids().to_vec() {
            let Some(OpeningSceneActor::LogoOutline {
                parent: owner,
                mut actor,
            }) = self.actors[id.index()]
            else {
                continue;
            };
            if owner == parent {
                actor.position = pose.position;
                actor.rotation = pose.rotation;
                self.actors[id.index()] = Some(OpeningSceneActor::LogoOutline {
                    parent: owner,
                    actor,
                });
                self.synchronize(id);
            }
        }
    }

    fn retire_logo_outlines(&mut self, parent: ObjectId) {
        for id in self.objects.active_ids().to_vec() {
            if matches!(
                self.actor(id),
                Some(OpeningSceneActor::LogoOutline { parent: owner, .. }) if *owner == parent
            ) {
                self.retiring[id.index()] = true;
            }
        }
    }

    fn spawn_burst(
        &mut self,
        after: ObjectId,
        pose: IntroScenePose,
        random: &mut impl RandomSource,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        if let Some((particle, sound)) = opening_burst(pose, self.global_clock, random) {
            self.allocate(after, OpeningSceneActor::Burst(particle), events)?;
            if let Some(sound) = sound {
                events.burst_audio.push(OpeningBurstAudio {
                    sound,
                    source: pose.position,
                });
            }
        }
        Ok(())
    }

    fn spawn_second_flyby_child(
        &mut self,
        parent: ObjectId,
        spawn: OpeningSecondFlybySpawn,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        use OpeningSecondFlybyActor as Actor;
        use OpeningSecondFlybyChild as Child;
        let placeholder = match (spawn.child, spawn.placement) {
            (Child::LinkedChain, OpeningSecondFlybySpawnPlacement::Attached(_)) => Actor::Chain(
                OpeningChainSegment::new(parent, parent, parent, OpeningChainPart::First),
            ),
            (Child::EngineFlare, OpeningSecondFlybySpawnPlacement::Attached(_)) => Actor::Flare(
                super::intro_second_flyby::OpeningSecondFlybyFlare::new(parent),
            ),
            (Child::Trail, OpeningSecondFlybySpawnPlacement::Attached(_)) => Actor::Trail {
                parent,
                actor: super::intro_second_flyby::OpeningSecondFlybyTrail::new(),
            },
            (Child::CameraTarget, OpeningSecondFlybySpawnPlacement::Independent(pose)) => {
                Actor::CameraTarget(
                    super::intro_second_camera_target::OpeningSecondCameraTarget::new(pose),
                )
            }
            (Child::AttachedWing, OpeningSecondFlybySpawnPlacement::Attached(attachment)) => {
                Actor::AttachedWing(OpeningAttachedWing::new(parent, parent, attachment))
            }
            (Child::DepartingWing, OpeningSecondFlybySpawnPlacement::Independent(pose)) => {
                Actor::DepartingWing(OpeningDepartingWing::new(parent, pose))
            }
            _ => unreachable!("later flyby spawn uses its authored placement"),
        };
        let id = self.allocate(parent, OpeningSceneActor::SecondFlyby(placeholder), events)?;
        let actor = match self.actors[id.index()].unwrap() {
            OpeningSceneActor::SecondFlyby(Actor::Chain(_)) => Actor::Chain(
                OpeningChainSegment::new(id, parent, parent, OpeningChainPart::First),
            ),
            OpeningSceneActor::SecondFlyby(Actor::AttachedWing(_)) => {
                let OpeningSecondFlybySpawnPlacement::Attached(attachment) = spawn.placement else {
                    unreachable!()
                };
                Actor::AttachedWing(OpeningAttachedWing::new(id, parent, attachment))
            }
            OpeningSceneActor::SecondFlyby(Actor::DepartingWing(_)) => {
                let OpeningSecondFlybySpawnPlacement::Independent(pose) = spawn.placement else {
                    unreachable!()
                };
                Actor::DepartingWing(OpeningDepartingWing::new(id, pose))
            }
            OpeningSceneActor::SecondFlyby(actor) => actor,
            _ => unreachable!(),
        };
        self.actors[id.index()] = Some(OpeningSceneActor::SecondFlyby(actor));
        self.synchronize(id);
        Ok(())
    }

    fn publish_second_flyby_children(&mut self, parent: ObjectId, pose: IntroScenePose) {
        for id in self.objects.active_ids().to_vec() {
            let Some(OpeningSceneActor::SecondFlyby(mut child)) = self.actors[id.index()] else {
                continue;
            };
            child.publish_from_parent(parent, pose);
            self.actors[id.index()] = Some(OpeningSceneActor::SecondFlyby(child));
            self.synchronize(id);
        }
    }

    fn publish_late_effect(&mut self, parent: ObjectId, pose: IntroScenePose) {
        for id in self.objects.active_ids().to_vec() {
            let Some(OpeningSceneActor::LateTargetEffect {
                parent: owner,
                mut actor,
            }) = self.actors[id.index()]
            else {
                continue;
            };
            if owner == parent && actor.is_visible() {
                actor.pose = actor.attachment.world_pose(pose);
                self.actors[id.index()] = Some(OpeningSceneActor::LateTargetEffect {
                    parent: owner,
                    actor,
                });
                self.synchronize(id);
            }
        }
    }

    fn retire_late_effect(&mut self, parent: ObjectId) {
        for id in self.objects.active_ids().to_vec() {
            if matches!(
                self.actor(id),
                Some(OpeningSceneActor::LateTargetEffect { parent: owner, .. }) if *owner == parent
            ) {
                self.retiring[id.index()] = true;
            }
        }
    }

    fn advance_actor(
        &mut self,
        id: ObjectId,
        random: &mut impl RandomSource,
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        let cue = self.controller.cue();
        let actor = self.actors[id.index()].expect("live opening object has an actor");
        match actor {
            OpeningSceneActor::Controller => self.controller.tick(&mut self.palette),
            OpeningSceneActor::InactivePlayer => {}
            OpeningSceneActor::Root(mut root) => {
                let root_events = root.tick(self.controller.cue());
                let pose = root.pose;
                self.actors[id.index()] = Some(OpeningSceneActor::Root(root));
                self.synchronize(id);
                for event in root_events.iter().copied() {
                    self.handle_root_event(id, event, events)?;
                }
                events.root_events.extend(root_events);
                self.publish_root_attachments(pose);
                return Ok(());
            }
            OpeningSceneActor::Camera(mut camera) => {
                let target = self
                    .camera_target
                    .and_then(|target| self.actor(target))
                    .map(OpeningSceneActor::pose)
                    .unwrap_or_default()
                    .position;
                camera.tick(cue, self.scene_depth_velocity, target, &mut self.camera);
                self.actors[id.index()] = Some(OpeningSceneActor::Camera(camera));
            }
            OpeningSceneActor::CameraTarget(mut target) => {
                let step = target.tick(cue, &self.objects);
                if step.select_as_camera_target {
                    self.camera_target = Some(id);
                    events.selected_camera_target = Some(id);
                }
                if step.finished {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] = Some(OpeningSceneActor::CameraTarget(target));
            }
            OpeningSceneActor::LogoAssembly(mut assembly) => {
                let step = assembly.tick_with_scroll(LogoSceneScroll {
                    horizontal: 0,
                    depth: self.scene_depth_velocity,
                    horizontal_locked: true,
                });
                self.actors[id.index()] = Some(OpeningSceneActor::LogoAssembly(assembly));
                if let Some(pair) = step.glyph_pair {
                    // Primary is allocated first; secondary consequently runs
                    // first because both insert after the assembly.
                    self.allocate(
                        id,
                        OpeningSceneActor::LogoGlyph(NintendoLogoActor::new(
                            pair,
                            LogoLayer::Primary,
                            Rotation::default(),
                        )),
                        events,
                    )?;
                    self.allocate(
                        id,
                        OpeningSceneActor::LogoGlyph(NintendoLogoActor::new(
                            pair,
                            LogoLayer::Secondary,
                            Rotation::default(),
                        )),
                        events,
                    )?;
                }
                if let Some(position) = step.sweep_position {
                    self.allocate(
                        id,
                        OpeningSceneActor::LogoSweep(NintendoLogoSweep::new(position)),
                        events,
                    )?;
                }
                if step.release {
                    self.logo_released = true;
                    self.retiring[id.index()] = true;
                }
            }
            OpeningSceneActor::LogoGlyph(mut glyph) => {
                let step = glyph.tick(
                    self.logo_released,
                    LogoSceneScroll {
                        horizontal: 0,
                        depth: self.scene_depth_velocity,
                        horizontal_locked: true,
                    },
                    random,
                );
                let pose = IntroScenePose {
                    position: glyph.position,
                    rotation: glyph.rotation,
                };
                self.actors[id.index()] = Some(OpeningSceneActor::LogoGlyph(glyph));
                if step.spawn_outline_child {
                    self.allocate(
                        id,
                        OpeningSceneActor::LogoOutline {
                            parent: id,
                            actor: NintendoLogoOutline::new(&glyph),
                        },
                        events,
                    )?;
                }
                if step.finished || glyph.phase() == LogoActorPhase::Finished {
                    self.retiring[id.index()] = true;
                    self.retire_logo_outlines(id);
                } else {
                    self.publish_logo_outline(id, pose);
                }
            }
            OpeningSceneActor::LogoOutline { parent, mut actor } => {
                actor.tick();
                self.actors[id.index()] = Some(OpeningSceneActor::LogoOutline { parent, actor });
            }
            OpeningSceneActor::LogoSweep(mut sweep) => {
                if sweep.tick(
                    self.logo_released,
                    LogoSceneScroll {
                        horizontal: 0,
                        depth: self.scene_depth_velocity,
                        horizontal_locked: true,
                    },
                ) {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] = Some(OpeningSceneActor::LogoSweep(sweep));
            }
            OpeningSceneActor::FlybyRig(mut rig) => {
                let step = rig.tick(cue);
                self.actors[id.index()] = Some(OpeningSceneActor::FlybyRig(rig));
                if let Some(streaks) = step.streaks {
                    self.spawn_flyby_streaks(id, streaks, events)?;
                }
                if step.finished {
                    self.retiring[id.index()] = true;
                }
            }
            OpeningSceneActor::FlybyStreak { owner, mut actor } => {
                if actor.tick() {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] = Some(OpeningSceneActor::FlybyStreak { owner, actor });
            }
            OpeningSceneActor::AttachedCraft(mut craft) => {
                let step = craft.tick(id, &mut self.auxiliary);
                let pose = craft.pose;
                self.actors[id.index()] = Some(OpeningSceneActor::AttachedCraft(craft));
                if step.split {
                    // Authored construction creates the attached flare before
                    // the independent copy; insertion reverses their visits.
                    self.allocate(
                        id,
                        OpeningSceneActor::CraftFlare {
                            owner: id,
                            actor: OpeningCraftFlare::new(),
                        },
                        events,
                    )?;
                    self.allocate(
                        id,
                        OpeningSceneActor::DepartingCraft(OpeningDepartingCraft::new(pose)),
                        events,
                    )?;
                }
                if step.emit_burst {
                    self.spawn_burst(id, pose, random, events)?;
                }
                if step.request_destruction {
                    self.common_destruction(id, craft.shape(), pose.position, events)?;
                }
            }
            OpeningSceneActor::DepartingCraft(mut craft) => {
                let step = craft.tick(id, &mut self.auxiliary);
                let pose = craft.pose;
                let shape = craft.shape;
                self.actors[id.index()] = Some(OpeningSceneActor::DepartingCraft(craft));
                if step.emit_burst {
                    self.spawn_burst(id, pose, random, events)?;
                }
                if step.request_destruction {
                    self.common_destruction(id, shape, pose.position, events)?;
                }
            }
            OpeningSceneActor::CraftFlare { owner, mut actor } => {
                actor.tick();
                if !actor.is_visible() {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] = Some(OpeningSceneActor::CraftFlare { owner, actor });
            }
            OpeningSceneActor::Burst(mut burst) => {
                burst.tick();
                if burst.is_finished() {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] = Some(OpeningSceneActor::Burst(burst));
            }
            OpeningSceneActor::FreeCraft(mut craft) => {
                if craft.phase() == OpeningFreeCraftPhase::AwaitingDestruction {
                    self.common_destruction(id, craft.shape(), craft.pose.position, events)?;
                } else {
                    let step = craft.tick(cue, &mut self.auxiliary);
                    if step.queue_departure_audio {
                        events.free_craft_departure_audio =
                            events.free_craft_departure_audio.saturating_add(1);
                    }
                }
                self.actors[id.index()] = Some(OpeningSceneActor::FreeCraft(craft));
            }
            OpeningSceneActor::FormationCraft(mut craft) => {
                if craft.phase() == OpeningFormationPhase::AwaitingDestruction {
                    self.common_destruction(id, craft.shape(), craft.pose.position, events)?;
                } else {
                    let step = craft.tick(cue, &self.objects, &mut self.auxiliary);
                    if let Some(audio) = step.pursuit_audio {
                        events.formation_audio.push(audio);
                    }
                    if step.finished {
                        self.retiring[id.index()] = true;
                    }
                }
                self.actors[id.index()] = Some(OpeningSceneActor::FormationCraft(craft));
            }
            OpeningSceneActor::LateCameraTarget(mut target) => {
                let step = target.tick_parent();
                let mut newborn = target.effect.take();
                let pose = target.pose;
                self.actors[id.index()] = Some(OpeningSceneActor::LateCameraTarget(target));
                if step.select_as_camera_target {
                    self.camera_target = Some(id);
                    events.selected_camera_target = Some(id);
                }
                if step.spawn_effect {
                    let effect = newborn
                        .take()
                        .expect("late target creates its authored effect");
                    self.allocate(
                        id,
                        OpeningSceneActor::LateTargetEffect {
                            parent: id,
                            actor: effect,
                        },
                        events,
                    )?;
                } else if !step.target_finished {
                    self.publish_late_effect(id, pose);
                }
                if step.target_finished {
                    self.retiring[id.index()] = true;
                    self.retire_late_effect(id);
                }
            }
            OpeningSceneActor::LateTargetEffect { parent, mut actor } => {
                actor.tick(cue);
                if !actor.is_visible() {
                    self.retiring[id.index()] = true;
                }
                self.actors[id.index()] =
                    Some(OpeningSceneActor::LateTargetEffect { parent, actor });
            }
            OpeningSceneActor::SecondFlyby(mut actor) => {
                if actor.awaiting_destruction() {
                    self.common_destruction(id, actor.shape(), actor.pose().position, events)?;
                } else {
                    match &mut actor {
                        OpeningSecondFlybyActor::Craft(craft) => {
                            let step = craft.tick(cue);
                            let pose = craft.pose;
                            self.actors[id.index()] = Some(OpeningSceneActor::SecondFlyby(actor));
                            for event in step.iter().copied() {
                                match event {
                                    OpeningSecondFlybyEvent::InitializeChildControls => {
                                        self.chain_controls = OpeningChainControls {
                                            sort_override_on_reveal: true,
                                            ..Default::default()
                                        }
                                    }
                                    OpeningSecondFlybyEvent::EnableChildPitchSettling => {
                                        self.chain_controls.settle_pitch = true
                                    }
                                    OpeningSecondFlybyEvent::Spawn(spawn) => {
                                        self.spawn_second_flyby_child(id, spawn, events)?
                                    }
                                    OpeningSecondFlybyEvent::SelectAsCameraTarget => {
                                        self.camera_target = Some(id);
                                        events.selected_camera_target = Some(id);
                                    }
                                    OpeningSecondFlybyEvent::Sound { .. } => {}
                                }
                            }
                            events.second_flyby_events.extend(step);
                            self.publish_second_flyby_children(id, pose);
                            return Ok(());
                        }
                        OpeningSecondFlybyActor::Chain(segment) => {
                            if segment.phase() == OpeningChainPhase::Initializing
                                && segment.part() != OpeningChainPart::Tail
                            {
                                let next_part =
                                    OpeningChainPart::ALL[usize::from(segment.part().ordinal())];
                                let parent = segment.parent();
                                let predecessor = id;
                                let child = self.allocate(
                                    id,
                                    OpeningSceneActor::SecondFlyby(OpeningSecondFlybyActor::Chain(
                                        OpeningChainSegment::new(
                                            id,
                                            parent,
                                            predecessor,
                                            next_part,
                                        ),
                                    )),
                                    events,
                                )?;
                                self.actors[child.index()] = Some(OpeningSceneActor::SecondFlyby(
                                    OpeningSecondFlybyActor::Chain(OpeningChainSegment::new(
                                        child,
                                        parent,
                                        predecessor,
                                        next_part,
                                    )),
                                ));
                                self.synchronize(child);
                            }
                            let parent = self
                                .actor(segment.parent())
                                .expect("chain parent remains live")
                                .pose();
                            let predecessor = self
                                .actor(segment.predecessor())
                                .expect("chain predecessor remains live")
                                .pose();
                            if let Some(burst) = segment.tick(
                                parent,
                                predecessor,
                                self.chain_controls,
                                random,
                            ) {
                                self.allocate(
                                    id,
                                    OpeningSceneActor::SecondFlyby(
                                        OpeningSecondFlybyActor::ChainBurst(burst),
                                    ),
                                    events,
                                )?;
                                self.retiring[id.index()] = true;
                            }
                        }
                        OpeningSecondFlybyActor::Flare(flare) => flare.tick(),
                        OpeningSecondFlybyActor::Trail { actor: trail, .. } => {
                            self.retiring[id.index()] |= trail.tick();
                        }
                        OpeningSecondFlybyActor::CameraTarget(target) => {
                            if target.tick().select_as_camera_target {
                                self.camera_target = Some(id);
                                events.selected_camera_target = Some(id);
                            }
                        }
                        OpeningSecondFlybyActor::AttachedWing(wing) => {
                            wing.tick(&mut self.auxiliary);
                        }
                        OpeningSecondFlybyActor::DepartingWing(wing) => {
                            wing.tick(&mut self.auxiliary);
                        }
                        OpeningSecondFlybyActor::ChainBurst(burst) => {
                            burst.tick();
                            self.retiring[id.index()] |= burst.is_finished();
                        }
                        OpeningSecondFlybyActor::Explosion(effect) => {
                            effect.tick_animation(&self.destruction_context());
                            self.retiring[id.index()] |= effect.is_finished();
                        }
                    }
                    self.actors[id.index()] = Some(OpeningSceneActor::SecondFlyby(actor));
                }
            }
            OpeningSceneActor::Explosion(mut effect) => {
                if effect.phase() == IntroExplosionPhase::AwaitingDestruction {
                    self.common_destruction(id, effect.shape(), effect.position, events)?;
                } else {
                    effect.tick_animation(&self.destruction_context());
                    if effect.is_finished() {
                        self.retiring[id.index()] = true;
                    }
                }
                self.actors[id.index()] = Some(OpeningSceneActor::Explosion(effect));
            }
        }
        self.synchronize(id);
        Ok(())
    }

    /// Advance one complete actor traversal and its ordinary-frame artwork
    /// barrier, with the generic RNG refresh after the active-list tail.
    ///
    /// Retail's entropy update can interrupt a traversal, including between
    /// two draws by one actor. Callers with derived or observed ordering use
    /// [`Self::tick_with_random_refreshes`]. The coarser
    /// [`Self::tick_with_first_pass_budget`] supports only between-actor splits.
    /// Capacity failure rolls
    /// back the controller, palette, pool, RNG, camera and auxiliary state
    /// together. Frame work is joined at this coarse boundary; this API does
    /// not model upload deadlines or perform the scene's rendering itself.
    pub fn tick(&mut self) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        self.tick_with_refresh_boundaries(&[usize::MAX])
    }

    /// Advance one complete actor traversal split around a caller-supplied
    /// source-frame generic RNG refresh boundary.
    ///
    /// `first_pass_actor_budget` is the number of actual actor visits before
    /// the refresh.  It includes the controller and children inserted into the
    /// live list during this traversal.  Zero refreshes before the controller;
    /// a budget at least as large as the completed traversal refreshes after
    /// the tail.  Retiring actors remain linked across both passes and cleanup
    /// occurs only after the resumed traversal reaches the tail.
    pub fn tick_with_first_pass_budget(
        &mut self,
        first_pass_actor_budget: usize,
    ) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        self.tick_with_refresh_boundaries(&[first_pass_actor_budget])
    }

    /// Advance one complete actor traversal with generic RNG refreshes after
    /// the supplied numbers of completed actor visits.
    ///
    /// Boundaries must be sorted and may repeat: `[0, 0]` performs two
    /// refreshes before the controller, while repeated nonzero values perform
    /// consecutive refreshes between the same two actor visits.  Boundaries
    /// beyond the number of visits are all applied after the active-list tail.
    /// An empty slice performs no generic refresh during this traversal.
    /// Actor-spawned children count as visits when traversal reaches them, and
    /// cleanup remains deferred until every visit and refresh is complete.
    pub fn tick_with_refresh_boundaries(
        &mut self,
        refresh_after_visits: &[usize],
    ) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        assert!(
            refresh_after_visits
                .windows(2)
                .all(|pair| pair[0] <= pair[1]),
            "opening refresh boundaries must be sorted"
        );
        let mut pending = self.clone();
        let events = pending.advance(refresh_after_visits, &[])?;
        *self = pending;
        Ok(events)
    }

    /// Advance a complete traversal with entropy refreshes after the supplied
    /// numbers of actor random draws. Unlike actor-visit boundaries, this can
    /// preserve a refresh between one actor's direction, spin, or burst draws.
    /// Only ordering is supplied; actors and the shared generator remain native.
    /// Repeated and zero boundaries are allowed; remaining refreshes occur at
    /// the tail. This does not derive the source's autonomous refresh timing.
    pub fn tick_with_random_refreshes(
        &mut self,
        refresh_after_draws: &[usize],
    ) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        let mut pending = self.clone();
        let events = pending.advance(&[], refresh_after_draws)?;
        *self = pending;
        Ok(events)
    }

    fn advance(
        &mut self,
        refresh_after_visits: &[usize],
        refresh_after_draws: &[usize],
    ) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        let mut events = OpeningSceneFrameEvents::default();
        let mut random = InterleavedRandom::new(self.random, refresh_after_draws);
        self.frame_buffers
            .begin_frame()
            .expect("previous opening frame has joined its work");
        self.global_clock = self.global_clock.wrapping_add(1);
        let mut visits = 0usize;
        let mut next_refresh = 0usize;
        while refresh_after_visits.get(next_refresh) == Some(&0) {
            random.refresh();
            next_refresh += 1;
        }
        let mut cursor = self.objects.active_ids().first().copied();
        while let Some(id) = cursor {
            self.advance_actor(id, &mut random, &mut events)?;
            visits += 1;
            while refresh_after_visits.get(next_refresh) == Some(&visits) {
                random.refresh();
                next_refresh += 1;
            }
            // The strategy may have inserted a child after this actor.
            cursor = self.objects.get(id).expect("cleanup is deferred").base.next;
        }
        // Runtime $7F:058F advances the shared subtract generator. A traversal
        // can span multiple refreshes, including during an actor. Apply any
        // remaining caller-supplied refreshes at the tail before cleanup.
        while next_refresh < refresh_after_visits.len() {
            random.refresh();
            next_refresh += 1;
        }
        events.random_draws = random.draw_count();
        self.random = random.finish();
        for id in self.objects.active_ids().to_vec() {
            if self.retiring[id.index()] {
                self.objects
                    .remove(id)
                    .expect("retiring opening actor is live");
                self.actors[id.index()] = None;
                self.retiring[id.index()] = false;
                events.retired.push(id);
            }
        }
        // Both ordinary-frame jobs have joined before the source examines a
        // pending load. Their relative completion time is deliberately not
        // inferred from actor visits. Each owner advances once for this frame,
        // even if the next source upload later completes before its controller.
        self.frame_buffers
            .finish_draw()
            .expect("opening frame draw queued");
        self.frame_buffers
            .finish_upload()
            .expect("opening frame upload queued");
        self.publish_frame_artwork(&mut events);
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer_policy(plane: super::super::scene_video::ArtworkPlane) -> SceneLayerPolicy {
        use super::super::scene_video::{SceneLayerMask, TileMapGrid};
        SceneLayerPolicy {
            artwork_plane: plane,
            visible_layers: SceneLayerMask::STANDARD_SCENE,
            layered_large_characters: [true; 4],
            layered_foreground_priority: true,
            third_map_grid: TileMapGrid::LargeSquare,
        }
    }

    #[test]
    fn scene_presentation_requires_real_layer_policy_without_mutating_requests() {
        let mut scene = OpeningScene::default();
        let before = scene.clone();
        assert_eq!(
            scene.begin_scene_presentation(sample_artwork(17), false),
            Err(OpeningArtworkRequestError::MissingLayerPolicy)
        );
        assert_eq!(
            scene.queue_scene_presentation(sample_artwork(17), false),
            Err(OpeningArtworkRequestError::MissingLayerPolicy)
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn scene_layout_and_artwork_publish_without_advancing_fades_or_actors() {
        use super::super::scene_display::{DisplayBand, Intensity};
        use super::super::scene_video::{ArtworkPlane, SceneTileMode, TileMapGrid};
        let mut scene = OpeningScene::default();
        scene.set_scene_layer_policy(layer_policy(ArtworkPlane::First));
        scene
            .begin_scene_presentation(sample_artwork(17), false)
            .unwrap();
        let actors = scene.snapshots().collect::<Vec<_>>();
        let random = scene.random();
        assert!(scene.video().setup_pending());
        assert_eq!(scene.video().last_setup(), None);
        assert_eq!(scene.video().output(), None);
        scene.set_scene_layer_policy(layer_policy(ArtworkPlane::Second));
        let visible = DisplayBand {
            blanked: false,
            intensity: Intensity::new(7),
        };
        scene.publish_display_output(visible);
        scene.publish_artwork().unwrap();
        let layout = scene.video().last_setup().unwrap();
        assert_eq!(layout.mode, SceneTileMode::ColumnOffsets);
        assert_eq!(layout.large_characters, [true, false, false, false]);
        assert!(!layout.foreground_priority);
        assert_eq!(layout.artwork_plane, ArtworkPlane::Second);
        assert_eq!(layout.artwork_map_grid, TileMapGrid::Tall);
        assert_eq!(layout.third_map_grid, TileMapGrid::LargeSquare);
        assert_eq!(scene.video().output(), Some(DisplayBand::BLANK_FULL));
        for _ in 0..4 {
            scene
                .resume_artwork_load(ForegroundSelection::STANDARD)
                .unwrap();
            scene.publish_display_output(visible);
            scene.publish_artwork().unwrap();
            assert_eq!(scene.video().output(), Some(DisplayBand::BLANK_FULL));
            assert_eq!(scene.video().last_setup(), Some(layout));
        }
        // The main-loop handoff is not a display service and must not blank
        // an independently published output again.
        scene.publish_display_output(visible);
        scene
            .resume_artwork_load(ForegroundSelection::STANDARD)
            .unwrap();
        assert_eq!(scene.video().output(), Some(visible));
        assert_eq!(scene.artwork_load_phase(), Some(ArtworkLoadPhase::Complete));
        assert_eq!(scene.snapshots().collect::<Vec<_>>(), actors);
        assert_eq!(scene.random(), random);
        assert_eq!(scene.global_clock(), 0);
        assert_eq!(scene.controller().elapsed_updates(), 0);
    }

    #[test]
    fn queued_presentation_samples_mode_at_the_frame_barrier_and_publishes_once() {
        use super::super::scene_video::{ArtworkPlane, SceneLayerMask};
        let mut scene = OpeningScene::default();
        scene.set_scene_layer_policy(layer_policy(ArtworkPlane::First));
        scene
            .queue_scene_presentation(sample_artwork(17), false)
            .unwrap();
        let inherited = SceneLayerPolicy {
            visible_layers: SceneLayerMask::THIRD,
            ..layer_policy(ArtworkPlane::Second)
        };
        scene.set_scene_layer_policy(inherited);
        let waiting = scene.tick().unwrap();
        assert_eq!(waiting.scene_mode_publication, None);
        assert_eq!(scene.video().last_setup(), None);
        assert!(!scene.video().setup_pending());
        assert_eq!(scene.scene_layer_policy(), Some(inherited));
        let loaded = scene.tick().unwrap();
        let layout = loaded.scene_mode_publication.unwrap();
        assert_eq!(layout.large_characters, [false, true, false, false]);
        assert_eq!(layout.artwork_plane, ArtworkPlane::Second);
        assert_eq!(layout.visible_layers, SceneLayerMask::STANDARD_SCENE);
        assert_eq!(scene.video().last_setup(), Some(layout));
        assert_eq!(loaded.artwork_publications.len(), 5);
        assert_eq!(scene.artwork_load_phase(), Some(ArtworkLoadPhase::Complete));
        assert_eq!(scene.tick().unwrap().scene_mode_publication, None);
    }

    #[test]
    fn rejected_load_dispatch_preserves_layer_policy_and_pending_work() {
        use super::super::scene_video::{ArtworkPlane, SceneLayerMask};
        let mut scene = OpeningScene::default();
        let before = scene.clone();
        assert_eq!(
            scene.begin_scene_load_sequence(sample_artwork(17), true),
            Err(OpeningArtworkRequestError::MissingLayerPolicy)
        );
        assert_eq!(scene, before);
        scene.set_scene_layer_policy(SceneLayerPolicy {
            visible_layers: SceneLayerMask::THIRD,
            ..layer_policy(ArtworkPlane::Second)
        });
        scene
            .queue_scene_presentation(sample_artwork(18), false)
            .unwrap();
        let queued = scene.clone();
        assert_eq!(
            scene.begin_scene_load_sequence(sample_artwork(19), true),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene, queued);
        scene.tick().unwrap();
        scene.tick().unwrap();
        scene
            .begin_scene_presentation(sample_artwork(20), false)
            .unwrap();
        let active = scene.clone();
        assert_eq!(
            scene.begin_scene_load_sequence(sample_artwork(21), true),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene, active);
    }

    #[test]
    fn outer_exit_owns_skipping_without_advancing_any_actor_or_frame_work() {
        use super::super::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
        use super::super::{AudioState, Button, Buttons};
        let mut scene = OpeningScene::default();
        scene.tick().unwrap();
        let before = scene.clone();
        let mut display = SceneDisplay {
            request: FadeRequest::Idle,
            progress: Intensity::FULL,
            bands: [DisplayBand::BLANK_FULL; 3],
            blank_hold: 255,
            interval_remaining: 0,
            interval_reload: 0,
        };
        let mut audio = AudioState::default();
        let pressed = Buttons::from_bits(Button::B as u16);
        for _ in 0..OPENING_INPUT_HOLD {
            assert_eq!(
                scene.visit_exit(pressed, &mut display, &mut audio),
                CinematicExitVisit::default()
            );
            assert!(!scene.controller.transition_requested);
        }
        scene.visit_exit(pressed, &mut display, &mut audio);
        assert!(scene.controller.transition_requested);
        assert_eq!(audio.take_events().iter().flatten().count(), 1);
        let start = scene.visit_exit(Buttons::default(), &mut display, &mut audio);
        assert!(start.request_audio_exit);
        assert!(!start.completed);
        for _ in 0..4 {
            display.visit_scene_fade(false);
        }
        assert!(
            scene
                .visit_exit(Buttons::default(), &mut display, &mut audio)
                .completed
        );
        assert!(!scene.controller.transition_requested);
        assert!(scene.exit().completed());
        // Only the outer exit and its shared request can change here. Actor
        // age, pool membership, RNG, artwork and buffer ownership are intact.
        scene.exit = before.exit;
        scene.controller.transition_requested = before.controller.transition_requested;
        assert_eq!(scene, before);
    }

    fn sample_artwork(value: u8) -> std::sync::Arc<sf2_data::opening_artwork::OpeningArtwork> {
        std::sync::Arc::new(
            sf2_data::opening_artwork::OpeningArtwork::from_decoded(
                &vec![value; 8224],
                &vec![value; 4096],
                &vec![value; 9408],
            )
            .unwrap(),
        )
    }

    #[test]
    fn queued_artwork_uses_retained_buffer_roles_not_scene_age_or_refresh_count() {
        use super::super::scene_frame::BitmapBuffer;
        use sf2_data::opening_artwork::ForegroundPaletteId;
        for preceding_frames in 0..5 {
            for refreshes in [&[][..], &[0, 0, usize::MAX][..]] {
                let mut scene = OpeningScene::default();
                for _ in 0..preceding_frames {
                    scene.tick_with_refresh_boundaries(refreshes).unwrap();
                }
                let work = scene.frame_buffers().next_work();
                let asset = sample_artwork(17);
                scene.queue_artwork_load(asset.clone(), false).unwrap();
                // Choice is sampled when loading, not when requesting.
                scene.set_foreground_selection(ForegroundSelection {
                    use_catalog: true,
                    entry: 1,
                });
                let mut events = scene.tick_with_refresh_boundaries(refreshes).unwrap();
                if work.upload_source == BitmapBuffer::Second {
                    assert!(events.artwork_publications.is_empty());
                    assert!(scene.artwork_request_pending());
                    assert!(scene.artwork().characters.is_none());
                    assert_eq!(scene.artwork_load_phase(), None);
                    events = scene.tick_with_refresh_boundaries(refreshes).unwrap();
                }
                assert_eq!(
                    events.artwork_publications,
                    [
                        ArtworkPublication::PolygonPalette,
                        ArtworkPublication::BackgroundCharacters,
                        ArtworkPublication::BackgroundMap,
                        ArtworkPublication::ForegroundPalette(ForegroundPaletteId::CatalogOne),
                        ArtworkPublication::SpritePalette,
                    ]
                );
                assert!(!scene.artwork_request_pending());
                assert_eq!(scene.artwork_load_phase(), Some(ArtworkLoadPhase::Complete));
                assert!(std::sync::Arc::ptr_eq(
                    scene.artwork().characters.as_ref().unwrap(),
                    &asset.characters
                ));
                assert!(std::sync::Arc::ptr_eq(
                    scene.artwork().map.as_ref().unwrap(),
                    &asset.map
                ));
                assert!(scene.tick().unwrap().artwork_publications.is_empty());
            }
        }
    }

    #[test]
    fn replacing_a_deferred_request_preserves_frame_and_published_artwork() {
        let mut scene = OpeningScene::default();
        let first = sample_artwork(17);
        scene.queue_artwork_load(first, false).unwrap();
        let pending = scene.clone();
        let latest = sample_artwork(85);
        scene.queue_artwork_load(latest.clone(), true).unwrap();
        assert_eq!(scene.frame_buffers, pending.frame_buffers);
        assert_eq!(scene.artwork, pending.artwork);
        assert_eq!(scene.palette, pending.palette);
        assert!(scene.tick().unwrap().artwork_publications.is_empty());
        let before = scene.clone();
        assert_eq!(
            scene.begin_artwork_load(sample_artwork(34), false),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene, before);
        let mut without_load = scene.clone();
        without_load.deferred_artwork = None;
        without_load.tick().unwrap();
        assert_eq!(scene.tick().unwrap().artwork_publications.len(), 5);
        assert!(std::sync::Arc::ptr_eq(
            scene.artwork().characters.as_ref().unwrap(),
            &latest.characters
        ));
        assert!(!scene.artwork().skip_next_background_palette);
        assert_eq!(
            scene.palette().colors[..64],
            without_load.palette().colors[..64]
        );
        assert_eq!(scene.random(), without_load.random());
        assert_eq!(scene.controller(), without_load.controller());
        assert_eq!(
            scene.snapshots().collect::<Vec<_>>(),
            without_load.snapshots().collect::<Vec<_>>()
        );
    }

    #[test]
    fn deferred_requests_cannot_replace_an_active_manual_service_sequence() {
        let mut scene = OpeningScene::default();
        scene.begin_artwork_load(sample_artwork(17), false).unwrap();
        let before = scene.clone();
        assert_eq!(
            scene.queue_artwork_load(sample_artwork(85), true),
            Err(OpeningArtworkRequestError::AlreadyLoading)
        );
        assert_eq!(scene, before);
        // Advancing actors does not finish a sequence owned by the separate
        // fine-grained display API or invent a missing service publication.
        assert!(scene.tick().unwrap().artwork_publications.is_empty());
        assert_eq!(
            scene.artwork_load_phase(),
            Some(ArtworkLoadPhase::Pending(
                ArtworkPublication::PolygonPalette
            ))
        );
    }

    #[test]
    fn boot_slots_and_root_insertion_match_the_source_pool() {
        let scene = OpeningScene::default();
        let actors: Vec<_> = scene
            .actors()
            .map(|(id, actor)| (id.index(), *actor))
            .collect();
        assert!(matches!(actors[0], (0, OpeningSceneActor::Controller)));
        assert!(matches!(actors[1], (2, OpeningSceneActor::Root(_))));
        assert!(matches!(actors[2], (1, OpeningSceneActor::InactivePlayer)));
        assert_eq!(scene.available_slots(), OBJECT_CAPACITY - 3);
    }

    #[test]
    fn first_root_children_run_in_their_live_insertion_order() {
        let mut scene = OpeningScene::default();
        let events = scene.tick().unwrap();
        assert_eq!(scene.global_clock(), 1);
        assert_eq!(
            events.spawned.len(),
            7,
            "four root actors, a glyph pair and outline"
        );
        let kinds: Vec<_> = scene.actors().map(|(_, actor)| *actor).collect();
        assert!(matches!(kinds[0], OpeningSceneActor::Controller));
        assert!(matches!(kinds[1], OpeningSceneActor::Root(_)));
        assert!(matches!(kinds[2], OpeningSceneActor::FlybyRig(_)));
        assert!(matches!(kinds[3], OpeningSceneActor::Camera(_)));
        assert!(matches!(kinds[4], OpeningSceneActor::LogoAssembly(_)));
        assert!(matches!(kinds[5], OpeningSceneActor::LogoGlyph(_)));
        assert!(matches!(kinds[6], OpeningSceneActor::LogoGlyph(_)));
        assert!(matches!(kinds[7], OpeningSceneActor::LogoOutline { .. }));
        assert!(matches!(kinds[8], OpeningSceneActor::CameraTarget(_)));
        assert!(matches!(kinds[9], OpeningSceneActor::InactivePlayer));
    }

    #[test]
    fn complete_authored_controller_window_conserves_one_shared_pool() {
        for seed in [[0; 4], [17, 91, 211, 37]] {
            let mut scene = OpeningScene::new(
                RandomState::new(seed),
                OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
            );
            let mut maximum_live = 0;
            for _ in 0..460 {
                scene.tick().unwrap();
                let ids: Vec<_> = scene.actors().map(|(id, _)| id).collect();
                let mut unique = ids.clone();
                unique.sort_unstable();
                unique.dedup();
                assert_eq!(ids.len(), unique.len());
                assert_eq!(ids.len() + scene.available_slots(), OBJECT_CAPACITY);
                maximum_live = maximum_live.max(ids.len());
            }
            assert!(scene.controller().transition_requested);
            assert!(maximum_live > 20);
        }
    }

    #[test]
    fn first_pass_budget_interleaves_refresh_with_actor_rng_before_cleanup() {
        let mut scene = OpeningScene::new(
            RandomState::new([17, 91, 211, 37]),
            OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
        );
        // The logo releases on traversal 101.  Its primary layers then draw
        // from the shared RNG in active-list order while the secondary layers
        // retire without drawing.
        for _ in 0..100 {
            scene.tick().unwrap();
        }
        let first_primary_visit = scene
            .actors()
            .position(|(_, actor)| {
                matches!(
                    actor,
                    OpeningSceneActor::LogoGlyph(glyph) if glyph.layer == LogoLayer::Primary
                )
            })
            .expect("assembled logo contains a primary layer")
            + 1;

        let mut tail_refresh = scene.clone();
        let tail_events = tail_refresh.tick().unwrap();
        let mut split_refresh = scene;
        let split_events = split_refresh
            .tick_with_first_pass_budget(first_primary_visit)
            .unwrap();

        // Moving one refresh between the first and second RNG-consuming
        // actors changes only the subsequent draw assignment, not the final
        // RNG state, active-list lifecycle, or deferred cleanup result.
        assert_eq!(split_refresh.random(), tail_refresh.random());
        assert_eq!(split_events.spawned, tail_events.spawned);
        assert_eq!(split_events.retired, tail_events.retired);
        assert_eq!(
            split_refresh.actors().map(|(id, _)| id).collect::<Vec<_>>(),
            tail_refresh.actors().map(|(id, _)| id).collect::<Vec<_>>()
        );
        let primary_poses = |scene: &OpeningScene| {
            scene
                .actors()
                .filter_map(|(_, actor)| match actor {
                    OpeningSceneActor::LogoGlyph(glyph) if glyph.layer == LogoLayer::Primary => {
                        Some((glyph.glyph, glyph.position, glyph.rotation, glyph.velocity))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let split_poses = primary_poses(&split_refresh);
        let tail_poses = primary_poses(&tail_refresh);
        assert_eq!(split_poses[0], tail_poses[0]);
        assert_ne!(split_poses[1], tail_poses[1]);
    }

    #[test]
    fn refresh_boundary_list_accepts_zero_repeats_and_after_tail_values() {
        let random = RandomState::new([17, 91, 211, 37]);
        let palette = OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);

        let mut no_refresh = OpeningScene::new(random, palette.clone());
        no_refresh.tick_with_refresh_boundaries(&[]).unwrap();
        assert_eq!(no_refresh.random(), random);

        let mut expected = random;
        for _ in 0..3 {
            expected.next_byte();
        }
        let mut repeated = OpeningScene::new(random, palette);
        repeated
            .tick_with_refresh_boundaries(&[0, 0, usize::MAX])
            .unwrap();
        assert_eq!(repeated.random(), expected);
        assert_eq!(repeated.global_clock(), 1);
        assert_eq!(repeated.actors().count(), 10);
    }

    #[test]
    fn intra_actor_refresh_preserves_direction_but_changes_spin() {
        let mut scene = OpeningScene::new(
            RandomState::new([17, 91, 211, 37]),
            OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
        );
        for _ in 0..100 {
            scene.tick().unwrap();
        }
        let first_primary = scene.actors().find_map(|(id, actor)| {
            matches!(actor, OpeningSceneActor::LogoGlyph(glyph) if glyph.layer == LogoLayer::Primary)
                .then_some(id)
        }).unwrap();
        let mut tail = scene.clone();
        let tail_events = tail.tick().unwrap();
        let split_events = scene.tick_with_random_refreshes(&[2]).unwrap();
        let OpeningSceneActor::LogoGlyph(split) = scene.actor(first_primary).unwrap() else {
            panic!("first primary remains a glyph");
        };
        let OpeningSceneActor::LogoGlyph(unsplit) = tail.actor(first_primary).unwrap() else {
            panic!("first primary remains a glyph");
        };
        assert_eq!(split.position, unsplit.position);
        assert_eq!(split.velocity, unsplit.velocity);
        assert_ne!(split.rotation, unsplit.rotation);
        assert_eq!(scene.random(), tail.random());
        assert_eq!(split_events, tail_events);
    }

    #[test]
    fn split_traversal_capacity_error_rolls_back_even_an_early_refresh() {
        let mut scene = OpeningScene::new(
            RandomState::new([17, 91, 211, 37]),
            OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
        );
        let mut setup_events = OpeningSceneFrameEvents::default();
        scene.set_scene_layer_policy(layer_policy(super::super::scene_video::ArtworkPlane::First));
        scene
            .queue_scene_presentation(sample_artwork(17), false)
            .unwrap();
        while scene.available_slots() > 0 {
            scene
                .allocate(
                    scene.inactive_player,
                    OpeningSceneActor::InactivePlayer,
                    &mut setup_events,
                )
                .unwrap();
        }
        let before = scene.clone();
        let error = scene
            .tick_with_refresh_boundaries(&[0, 0, usize::MAX])
            .unwrap_err();
        assert_eq!(error.available_slots, 0);
        assert_eq!(scene, before);
        let error = scene
            .tick_with_random_refreshes(&[0, 0, usize::MAX])
            .unwrap_err();
        assert_eq!(error.available_slots, 0);
        assert_eq!(scene, before);
    }
}
