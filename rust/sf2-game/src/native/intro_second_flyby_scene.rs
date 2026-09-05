//! Live ordered traversal of the later flyby and every actor it creates.
//! Direct children insert after their spawner; common destruction inserts
//! after the scene head. Cleanup is deferred until traversal completes.

use super::intro_camera::OpeningCameraCue;
use super::intro_chain::{
    OpeningChainControls, OpeningChainDepartureBurst, OpeningChainPart, OpeningChainPhase,
    OpeningChainSegment,
};
use super::intro_destruction::{
    IntroDestructionCapacityError, IntroDestructionContext, IntroDestructionEffects,
    IntroExplosionActor, IntroExplosionPhase, IntroExplosionProfile, IntroExplosionVolume,
};
use super::intro_free_craft::IntroAuxiliaryEffect;
use super::intro_motion::IntroScenePose;
use super::intro_second_camera_target::OpeningSecondCameraTarget;
use super::intro_second_flyby::{OpeningSecondFlybyFlare, OpeningSecondFlybyTrail};
use super::intro_second_flyby_craft::{
    OpeningSecondFlybyChild, OpeningSecondFlybyCraft, OpeningSecondFlybyEvent,
    OpeningSecondFlybySpawn, OpeningSecondFlybySpawnPlacement,
};
use super::intro_second_flyby_wings::{
    OpeningAttachedWing, OpeningAttachedWingPhase, OpeningDepartingWing, OpeningDepartingWingPhase,
};
use super::object::{
    Behavior, Object, ObjectId, ObjectKind, ObjectLifetimeId, ObjectStore, ShapeId, OBJECT_CAPACITY,
};
use super::state::RandomState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyActor {
    Craft(OpeningSecondFlybyCraft),
    Chain(OpeningChainSegment),
    Flare(OpeningSecondFlybyFlare),
    Trail {
        parent: ObjectId,
        actor: OpeningSecondFlybyTrail,
    },
    CameraTarget(OpeningSecondCameraTarget),
    AttachedWing(OpeningAttachedWing),
    DepartingWing(OpeningDepartingWing),
    ChainBurst(OpeningChainDepartureBurst),
    Explosion(IntroExplosionActor),
}

impl OpeningSecondFlybyActor {
    pub fn pose(&self) -> IntroScenePose {
        match self {
            Self::Craft(actor) => actor.pose,
            Self::Chain(actor) => actor.pose,
            Self::Flare(actor) => actor.pose,
            Self::Trail { actor, .. } => actor.pose,
            Self::CameraTarget(actor) => actor.pose,
            Self::AttachedWing(actor) => actor.pose,
            Self::DepartingWing(actor) => actor.pose,
            Self::ChainBurst(actor) => actor.pose,
            Self::Explosion(actor) => IntroScenePose {
                position: actor.position,
                ..Default::default()
            },
        }
    }

    pub fn shape(&self) -> ShapeId {
        match self {
            Self::Craft(actor) => actor.shape(),
            Self::Chain(actor) => actor.shape(),
            Self::Flare(actor) => actor.shape(),
            Self::Trail { actor, .. } => actor.shape(),
            Self::CameraTarget(_) => ShapeId::EMPTY,
            Self::AttachedWing(actor) => actor.shape(),
            Self::DepartingWing(actor) => actor.shape(),
            Self::ChainBurst(actor) => actor.shape(),
            Self::Explosion(actor) => actor.shape(),
        }
    }

    fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        match self {
            Self::Chain(actor) => actor.publish_from_parent(parent, pose),
            Self::Flare(actor) => actor.publish_from_parent(parent, pose),
            Self::Trail {
                parent: owner,
                actor,
            } if *owner == parent => actor.publish_from_parent(pose),
            Self::AttachedWing(actor) => actor.publish_from_parent(parent, pose),
            _ => {}
        }
    }

    fn awaiting_destruction(&self) -> bool {
        match self {
            Self::AttachedWing(actor) => {
                actor.phase() == OpeningAttachedWingPhase::AwaitingDestruction
            }
            Self::DepartingWing(actor) => {
                actor.phase() == OpeningDepartingWingPhase::AwaitingDestruction
            }
            Self::Explosion(actor) => actor.phase() == IntroExplosionPhase::AwaitingDestruction,
            _ => false,
        }
    }

    fn eligible_for_pressure_retirement(&self) -> bool {
        matches!(self, Self::ChainBurst(_) | Self::Explosion(_))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningSecondFlybyFrameEvents {
    pub craft_events: Vec<OpeningSecondFlybyEvent>,
    pub explosion_audio: Vec<IntroExplosionVolume>,
    pub spawned: Vec<ObjectId>,
    pub retired: Vec<ObjectId>,
    pub selected_camera_target: Option<ObjectId>,
    pub allocation_pressure: bool,
}

/// A complete later-flyby subscene with its own source-sized object pool.
/// The enclosing opening scene supplies cue, listener and shared effect state.
/// Persistent actors remain live until explicit scene teardown; holding does
/// not synthesize chain departure or silently remove the flare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningSecondFlybyScene {
    objects: ObjectStore,
    actors: [Option<OpeningSecondFlybyActor>; OBJECT_CAPACITY],
    retiring: [bool; OBJECT_CAPACITY],
    craft: ObjectId,
    camera_target: Option<ObjectId>,
    pub chain_controls: OpeningChainControls,
}

impl Default for OpeningSecondFlybyScene {
    fn default() -> Self {
        Self::new()
    }
}

impl OpeningSecondFlybyScene {
    pub fn new() -> Self {
        let mut objects = ObjectStore::new();
        let actor = OpeningSecondFlybyActor::Craft(OpeningSecondFlybyCraft::new());
        let craft = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                actor.shape(),
                Behavior::Effect,
            ))
            .unwrap();
        let mut actors = [None; OBJECT_CAPACITY];
        actors[craft.index()] = Some(actor);
        Self {
            objects,
            actors,
            retiring: [false; OBJECT_CAPACITY],
            craft,
            camera_target: None,
            chain_controls: OpeningChainControls::default(),
        }
    }

    pub fn craft(&self) -> ObjectId {
        self.craft
    }
    pub fn camera_target(&self) -> Option<ObjectId> {
        self.camera_target
    }
    pub fn actor(&self, id: ObjectId) -> Option<&OpeningSecondFlybyActor> {
        self.actors.get(id.index())?.as_ref()
    }
    pub fn actors(&self) -> impl Iterator<Item = (ObjectId, &OpeningSecondFlybyActor)> {
        self.objects
            .active_ids()
            .iter()
            .copied()
            .map(|id| (id, self.actor(id).unwrap()))
    }
    pub fn lifetime(&self, id: ObjectId) -> Option<ObjectLifetimeId> {
        self.objects.lifetime_id(id)
    }
    pub fn available_slots(&self) -> usize {
        OBJECT_CAPACITY - self.objects.len()
    }

    pub fn set_chain_health_at_strategy_entry(&mut self, part: OpeningChainPart, health: u8) {
        for actor in self.actors.iter_mut().flatten() {
            if let OpeningSecondFlybyActor::Chain(segment) = actor {
                if segment.part() == part {
                    segment.set_health_at_strategy_entry(health);
                }
            }
        }
    }

    fn synchronize(&mut self, id: ObjectId) {
        let actor = self.actors[id.index()].unwrap();
        let object = self.objects.get_mut(id).unwrap();
        object.base.shape = actor.shape();
        object.base.position = actor.pose().position;
    }

    fn allocate(
        &mut self,
        after: ObjectId,
        shape: ShapeId,
        create: impl FnOnce(ObjectId) -> OpeningSecondFlybyActor,
        events: &mut OpeningSecondFlybyFrameEvents,
    ) -> Result<ObjectId, IntroDestructionCapacityError> {
        let id = self
            .objects
            .allocate_after(
                Some(after),
                Object::new(ObjectKind::Effect, shape, Behavior::Effect),
            )
            .ok_or(IntroDestructionCapacityError {
                required_slots: 1,
                available_slots: self.available_slots(),
            })?;
        self.actors[id.index()] = Some(create(id));
        self.retiring[id.index()] = false;
        // The source sweep runs before initialization of the new allocation;
        // initialization clears that slot's retirement flag. All older actors
        // from the allocator's anchor through its penultimate node are tested.
        if self.available_slots() == 0 {
            events.allocation_pressure = true;
            let mut cursor = Some(after);
            while let Some(current) = cursor {
                cursor = self.objects.get(current).unwrap().base.next;
                if cursor.is_none() {
                    break;
                }
                if current != id
                    && self
                        .actor(current)
                        .unwrap()
                        .eligible_for_pressure_retirement()
                {
                    self.retiring[current.index()] = true;
                }
            }
        }
        self.synchronize(id);
        events.spawned.push(id);
        Ok(id)
    }

    fn spawn_child(
        &mut self,
        parent: ObjectId,
        spawn: OpeningSecondFlybySpawn,
        events: &mut OpeningSecondFlybyFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        use OpeningSecondFlybyActor as Actor;
        use OpeningSecondFlybyChild as Child;
        self.allocate(
            parent,
            spawn.shape(),
            |id| match (spawn.child, spawn.placement) {
                (Child::LinkedChain, OpeningSecondFlybySpawnPlacement::Attached(_)) => {
                    Actor::Chain(OpeningChainSegment::new(
                        id,
                        parent,
                        parent,
                        OpeningChainPart::First,
                    ))
                }
                (Child::EngineFlare, OpeningSecondFlybySpawnPlacement::Attached(_)) => {
                    Actor::Flare(OpeningSecondFlybyFlare::new(parent))
                }
                (Child::Trail, OpeningSecondFlybySpawnPlacement::Attached(_)) => Actor::Trail {
                    parent,
                    actor: OpeningSecondFlybyTrail::new(),
                },
                (Child::CameraTarget, OpeningSecondFlybySpawnPlacement::Independent(pose)) => {
                    Actor::CameraTarget(OpeningSecondCameraTarget::new(pose))
                }
                (Child::AttachedWing, OpeningSecondFlybySpawnPlacement::Attached(attachment)) => {
                    Actor::AttachedWing(OpeningAttachedWing::new(id, parent, attachment))
                }
                (Child::DepartingWing, OpeningSecondFlybySpawnPlacement::Independent(pose)) => {
                    Actor::DepartingWing(OpeningDepartingWing::new(id, pose))
                }
                _ => unreachable!("authored later-flyby spawn uses its declared placement"),
            },
            events,
        )?;
        Ok(())
    }

    fn common_destruction(
        &mut self,
        id: ObjectId,
        actor: OpeningSecondFlybyActor,
        context: &IntroDestructionContext,
        events: &mut OpeningSecondFlybyFrameEvents,
    ) -> Result<(), IntroDestructionCapacityError> {
        let profile = IntroExplosionProfile::for_shape(actor.shape())
            .expect("authored shape belongs to catalog");
        let (effects, audio) = IntroDestructionEffects::spawn(
            profile,
            actor.pose().position,
            &IntroDestructionContext {
                available_slots: self.available_slots(),
                suppress_effects: context.suppress_effects
                    && !matches!(actor, OpeningSecondFlybyActor::Explosion(_)),
                ..*context
            },
        )?;
        events.explosion_audio.extend(audio);
        for effect in effects.actors().copied() {
            let head = self.objects.active_ids()[0];
            self.allocate(
                head,
                effect.shape(),
                |_| OpeningSecondFlybyActor::Explosion(effect),
                events,
            )?;
        }
        self.retiring[id.index()] = true;
        Ok(())
    }

    /// This scene's pool supplies `available_slots`; the context supplies the
    /// listener, scroll and effect-suppression policy, not a second pool count.
    /// Capacity failure preserves the pending entire scene update, including
    /// the caller's shared RNG and auxiliary state. It is not a successful
    /// effect drop; callers may surface the source diagnostic or retry.
    pub fn tick(
        &mut self,
        cue: OpeningCameraCue,
        random: &mut RandomState,
        auxiliary: &mut IntroAuxiliaryEffect,
        context: &IntroDestructionContext,
    ) -> Result<OpeningSecondFlybyFrameEvents, IntroDestructionCapacityError> {
        let mut pending = self.clone();
        let mut next_random = *random;
        let mut next_auxiliary = *auxiliary;
        let events = pending.advance(cue, &mut next_random, &mut next_auxiliary, context)?;
        *self = pending;
        *random = next_random;
        *auxiliary = next_auxiliary;
        Ok(events)
    }

    fn advance(
        &mut self,
        cue: OpeningCameraCue,
        random: &mut RandomState,
        auxiliary: &mut IntroAuxiliaryEffect,
        context: &IntroDestructionContext,
    ) -> Result<OpeningSecondFlybyFrameEvents, IntroDestructionCapacityError> {
        use OpeningSecondFlybyActor as Actor;
        let mut events = OpeningSecondFlybyFrameEvents::default();
        let mut cursor = self.objects.active_ids().first().copied();
        while let Some(id) = cursor {
            let mut actor = self.actors[id.index()].unwrap();
            if actor.awaiting_destruction() {
                self.common_destruction(id, actor, context, &mut events)?;
            } else {
                match &mut actor {
                    Actor::Craft(craft) => {
                        for event in craft.tick(cue) {
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
                                    self.spawn_child(id, spawn, &mut events)?
                                }
                                OpeningSecondFlybyEvent::SelectAsCameraTarget => {
                                    self.camera_target = Some(id);
                                    events.selected_camera_target = Some(id);
                                }
                                OpeningSecondFlybyEvent::Sound { .. } => {}
                            }
                            events.craft_events.push(event);
                        }
                    }
                    Actor::Chain(segment) => {
                        if segment.phase() == OpeningChainPhase::Initializing
                            && segment.part() != OpeningChainPart::Tail
                        {
                            let next_part =
                                OpeningChainPart::ALL[usize::from(segment.part().ordinal())];
                            let parent = segment.parent();
                            self.allocate(
                                id,
                                segment.shape(),
                                |child| {
                                    Actor::Chain(OpeningChainSegment::new(
                                        child, parent, id, next_part,
                                    ))
                                },
                                &mut events,
                            )?;
                        }
                        let parent = self.actor(segment.parent()).unwrap().pose();
                        let predecessor = self.actor(segment.predecessor()).unwrap().pose();
                        if let Some(burst) =
                            segment.tick(parent, predecessor, self.chain_controls, random)
                        {
                            self.allocate(
                                id,
                                burst.shape(),
                                |_| Actor::ChainBurst(burst),
                                &mut events,
                            )?;
                            self.retiring[id.index()] = true;
                        }
                    }
                    Actor::Flare(flare) => flare.tick(),
                    Actor::Trail { actor: trail, .. } => {
                        self.retiring[id.index()] |= trail.tick();
                    }
                    Actor::CameraTarget(target) => {
                        if target.tick().select_as_camera_target {
                            self.camera_target = Some(id);
                            events.selected_camera_target = Some(id);
                        }
                    }
                    Actor::AttachedWing(wing) => {
                        wing.tick(auxiliary);
                    }
                    Actor::DepartingWing(wing) => {
                        wing.tick(auxiliary);
                    }
                    Actor::ChainBurst(burst) => {
                        burst.tick();
                        self.retiring[id.index()] |= burst.is_finished();
                    }
                    Actor::Explosion(effect) => {
                        effect.tick_animation(context);
                        self.retiring[id.index()] |= effect.is_finished();
                    }
                }
                self.actors[id.index()] = Some(actor);
                self.synchronize(id);
                if matches!(actor, Actor::Craft(_)) {
                    for child in self.objects.active_ids().to_vec() {
                        self.actors[child.index()]
                            .as_mut()
                            .unwrap()
                            .publish_from_parent(id, actor.pose());
                        self.synchronize(child);
                    }
                }
            }
            // Re-read the live next pointer, not a snapshot made before the
            // strategy: newborns after this cursor execute immediately.
            cursor = self.objects.get(id).unwrap().base.next;
        }
        for id in self.objects.active_ids().to_vec() {
            if self.retiring[id.index()] {
                self.objects.remove(id).unwrap();
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

    fn advance(
        scene: &mut OpeningSecondFlybyScene,
        cue: OpeningCameraCue,
    ) -> OpeningSecondFlybyFrameEvents {
        scene
            .tick(
                cue,
                &mut RandomState::default(),
                &mut IntroAuxiliaryEffect::default(),
                &IntroDestructionContext::default(),
            )
            .unwrap()
    }

    #[test]
    fn same_update_parent_children_reverse_at_parent_but_chain_remains_predecessor_order() {
        let mut scene = OpeningSecondFlybyScene::new();
        let events = advance(&mut scene, OpeningCameraCue::SecondCut);
        assert_eq!(events.spawned.len(), 10);
        let actors: Vec<_> = scene.actors().collect();
        assert!(matches!(actors[0].1, OpeningSecondFlybyActor::Craft(_)));
        assert!(matches!(actors[1].1, OpeningSecondFlybyActor::Flare(_)));
        for (index, (id, actor)) in actors[2..].iter().enumerate() {
            let OpeningSecondFlybyActor::Chain(segment) = actor else {
                panic!("missing chain part")
            };
            assert_eq!(segment.part(), OpeningChainPart::ALL[index]);
            assert_eq!(segment.actor(), *id);
            assert_eq!(segment.phase(), OpeningChainPhase::HiddenUntilNextUpdate);
            assert_eq!(
                segment.predecessor(),
                if index == 0 {
                    scene.craft()
                } else {
                    actors[index + 1].0
                }
            );
        }
    }

    #[test]
    fn common_effects_wait_at_the_visited_head_and_slot_reuse_changes_lifetime() {
        let mut scene = OpeningSecondFlybyScene::new();
        let mut previous_lifetimes = [None; OBJECT_CAPACITY];
        let mut saw_deferred_birth = false;
        let mut saw_slot_reuse = false;
        for update in 0..500 {
            let cue = match update {
                0..100 => OpeningCameraCue::SecondCut,
                100..150 => OpeningCameraCue::ThirdCut,
                150..300 => OpeningCameraCue::FourthCut,
                _ => OpeningCameraCue::FinalCut,
            };
            let events = advance(&mut scene, cue);
            for (id, actor) in scene.actors() {
                if let OpeningSecondFlybyActor::Explosion(effect) = actor {
                    if events.spawned.contains(&id) {
                        assert!(matches!(
                            effect.phase(),
                            IntroExplosionPhase::Animating { age: 0, .. }
                        ));
                        saw_deferred_birth = true;
                    }
                }
                let lifetime = scene.lifetime(id).unwrap();
                if previous_lifetimes[id.index()].is_some_and(|previous| previous != lifetime) {
                    saw_slot_reuse = true;
                }
                previous_lifetimes[id.index()] = Some(lifetime);
            }
        }
        assert!(saw_deferred_birth);
        assert!(saw_slot_reuse);
        assert_eq!(
            scene.actors().count(),
            12,
            "craft, persistent flare/target and nine chain segments"
        );
        assert!(!scene.chain_controls.depart);
    }

    #[test]
    fn pool_exhaustion_rolls_back_the_whole_pending_traversal() {
        let mut scene = OpeningSecondFlybyScene::new();
        let parent = scene.craft();
        let mut events = OpeningSecondFlybyFrameEvents::default();
        // Other live, non-retiring attachments consume the scene pool. Leave
        // enough room for some, but not all, recursively created segments.
        while scene.available_slots() > 4 {
            scene
                .allocate(
                    parent,
                    ShapeId::from_catalog_index(48),
                    |_| OpeningSecondFlybyActor::Flare(OpeningSecondFlybyFlare::new(parent)),
                    &mut events,
                )
                .unwrap();
        }
        let mut random = RandomState::new([1, 2, 3, 4]);
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let saved = scene.clone();
        let saved_random = random;
        let saved_auxiliary = auxiliary;
        assert_eq!(
            scene.tick(
                OpeningCameraCue::Opening,
                &mut random,
                &mut auxiliary,
                &IntroDestructionContext::default()
            ),
            Err(IntroDestructionCapacityError {
                required_slots: 1,
                available_slots: 0
            })
        );
        assert_eq!(scene, saved);
        assert_eq!(random, saved_random);
        assert_eq!(auxiliary, saved_auxiliary);
    }

    #[test]
    fn final_slot_sweep_uses_the_actual_list_tail_and_spares_the_new_allocation() {
        let mut scene = OpeningSecondFlybyScene::new();
        let parent = scene.craft();
        let mut events = OpeningSecondFlybyFrameEvents::default();
        let burst = OpeningChainDepartureBurst::new(parent, IntroScenePose::default());
        let tail = scene
            .allocate(
                parent,
                burst.shape(),
                |_| OpeningSecondFlybyActor::ChainBurst(burst),
                &mut events,
            )
            .unwrap();
        let middle = scene
            .allocate(
                parent,
                burst.shape(),
                |_| OpeningSecondFlybyActor::ChainBurst(burst),
                &mut events,
            )
            .unwrap();
        while scene.available_slots() > 1 {
            scene
                .allocate(
                    parent,
                    ShapeId::from_catalog_index(48),
                    |_| OpeningSecondFlybyActor::Flare(OpeningSecondFlybyFlare::new(parent)),
                    &mut events,
                )
                .unwrap();
        }
        let newborn = scene
            .allocate(
                parent,
                burst.shape(),
                |_| OpeningSecondFlybyActor::ChainBurst(burst),
                &mut events,
            )
            .unwrap();
        assert!(events.allocation_pressure);
        assert!(scene.retiring[middle.index()]);
        assert!(!scene.retiring[tail.index()]);
        assert!(!scene.retiring[newborn.index()]);
    }
}
