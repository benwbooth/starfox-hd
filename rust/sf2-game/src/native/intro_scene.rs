//! Complete native opening-scene actor traversal.
//!
//! Every authored actor, including attached children and destruction effects,
//! occupies one entry in the same source-sized pool.  New path children are
//! inserted directly after their spawner and can therefore run later in the
//! current traversal.  Cleanup remains a separate, deferred pass.

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
use super::render::Rotation;
use super::state::RandomState;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningActorSnapshot {
    pub id: ObjectId,
    pub lifetime: ObjectLifetimeId,
    pub pose: IntroScenePose,
    pub shape: ShapeId,
    pub visible: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningSceneFrameEvents {
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
    palette: OpeningScenePalette,
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
            palette,
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
        events: &mut OpeningSceneFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        if let Some((particle, sound)) = opening_burst(pose, self.global_clock, &mut self.random) {
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
                    &mut self.random,
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
                    self.spawn_burst(id, pose, events)?;
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
                    self.spawn_burst(id, pose, events)?;
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
                                &mut self.random,
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

    /// Advance one complete actor traversal with the generic RNG refresh after
    /// the active-list tail.
    ///
    /// Retail's timer/PPU update can perform that refresh before the actor
    /// traversal reaches its tail.  Callers which supply that observed or
    /// independently derived visit boundary must use
    /// [`Self::tick_with_first_pass_budget`] instead.  Capacity failure rolls
    /// back the controller, palette, pool, RNG, camera and auxiliary state
    /// together.
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
        let events = pending.advance(refresh_after_visits)?;
        *self = pending;
        Ok(events)
    }

    fn advance(
        &mut self,
        refresh_after_visits: &[usize],
    ) -> Result<OpeningSceneFrameEvents, IntroDestructionCapacityError> {
        let mut events = OpeningSceneFrameEvents::default();
        self.global_clock = self.global_clock.wrapping_add(1);
        let mut visits = 0usize;
        let mut next_refresh = 0usize;
        while refresh_after_visits.get(next_refresh) == Some(&0) {
            self.random.next_byte();
            next_refresh += 1;
        }
        let mut cursor = self.objects.active_ids().first().copied();
        while let Some(id) = cursor {
            self.advance_actor(id, &mut events)?;
            visits += 1;
            while refresh_after_visits.get(next_refresh) == Some(&visits) {
                self.random.next_byte();
                next_refresh += 1;
            }
            // The strategy may have inserted a child after this actor.
            cursor = self.objects.get(id).expect("cleanup is deferred").base.next;
        }
        // Runtime $7F:058C increments the entropy word and $7F:058F calls the
        // shared subtract generator at $7F:7BD4. A controller-to-controller
        // traversal can span multiple source frames, so any remaining
        // caller-supplied refreshes land after the active-list tail and before
        // the distinct $7F:402D cleanup pass.
        while next_refresh < refresh_after_visits.len() {
            self.random.next_byte();
            next_refresh += 1;
        }
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
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn split_traversal_capacity_error_rolls_back_even_an_early_refresh() {
        let mut scene = OpeningScene::new(
            RandomState::new([17, 91, 211, 37]),
            OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
        );
        let mut setup_events = OpeningSceneFrameEvents::default();
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
    }
}
