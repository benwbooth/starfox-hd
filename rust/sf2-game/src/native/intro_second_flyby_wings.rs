//! The later flyby's attached wing and independent departing wing. Attachment
//! publication precedes the child's strategy; destruction begins next update.

use super::game::flight_velocity;
use super::intro_destruction::{
    IntroDestructionCapacityError, IntroDestructionContext, IntroDestructionEffects,
    IntroExplosionActor, IntroExplosionBirthTiming, IntroExplosionProfile, IntroExplosionVolume,
};
use super::intro_free_craft::IntroAuxiliaryEffect;
use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::intro_second_flyby::OpeningSecondFlybyPlacement;
use super::object::{Angle, ObjectId, ShapeId, Vector3};

const WING_SHAPE: ShapeId = ShapeId::from_catalog_index(89);
const ATTACHED_WAIT: u8 = 27;
const DETACHED_SPIN: u8 = 18;
const DETACHED_DRIFT: Vector3 = Vector3 { x: 5, y: 0, z: 40 };
const DETACHED_YAW_STEP: i8 = 16;
const DETACHED_ROLL_STEP: i8 = 10;
const HIDDEN_TURN: u8 = 11;
const VISIBLE_TURN: u8 = 26;
const DEPARTURE_ROLL: u8 = 18;
const TURN_SPEED: u8 = 12;
const DEPARTURE_SPEED: u8 = 20;
const DEPARTURE_YAW: Angle = Angle::from_units(156);
const DEPARTURE_PITCH: Angle = Angle::from_units(6);
const DEPARTURE_ROLL_STEP: i8 = 8;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningWingEvents {
    /// The source unlinks the matching wing-group member from its parent.
    /// Scene ownership must stop publishing its attachment after this update.
    pub detached: bool,
    /// This update still completes motion. Common destruction takes over on
    /// the following update, rather than making the craft immediately hidden.
    pub request_destruction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningAttachedWingPhase {
    Waiting { updates_left: u8 },
    Spinning { updates_left: u8 },
    AwaitingDestruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningAttachedWing {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    pub velocity: Vector3,
    actor: ObjectId,
    parent: Option<ObjectId>,
    phase: OpeningAttachedWingPhase,
}

impl OpeningAttachedWing {
    pub fn new(actor: ObjectId, parent: ObjectId, attachment: IntroAttachment) -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment,
            velocity: Vector3::default(),
            actor,
            parent: Some(parent),
            phase: OpeningAttachedWingPhase::Waiting {
                updates_left: ATTACHED_WAIT,
            },
        }
    }

    pub const fn shape(&self) -> ShapeId {
        WING_SHAPE
    }
    pub fn phase(&self) -> OpeningAttachedWingPhase {
        self.phase
    }
    pub fn parent(&self) -> Option<ObjectId> {
        self.parent
    }

    /// Only the owning parent's earlier attachment pass can publish this pose.
    /// Birth can precede the first publication; detachment preserves the last
    /// published world pose and prevents subsequent parent movement leaking in.
    pub fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        if self.parent == Some(parent) {
            self.pose = self.attachment.world_pose(pose);
        }
    }

    pub fn tick(&mut self, auxiliary: &mut IntroAuxiliaryEffect) -> OpeningWingEvents {
        use OpeningAttachedWingPhase::*;
        let mut events = OpeningWingEvents::default();
        loop {
            match self.phase {
                Waiting { updates_left } if updates_left != 0 => {
                    self.phase = Waiting {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                Waiting { .. } => {
                    self.velocity = DETACHED_DRIFT;
                    self.parent = None;
                    events.detached = true;
                    self.phase = Spinning {
                        updates_left: DETACHED_SPIN,
                    };
                }
                Spinning { updates_left } => {
                    self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(DETACHED_YAW_STEP);
                    self.pose.rotation.pitch = self.pose.rotation.pitch.wrapping_add(1);
                    self.pose.rotation.roll =
                        self.pose.rotation.roll.wrapping_add(DETACHED_ROLL_STEP);
                    if updates_left > 1 {
                        self.phase = Spinning {
                            updates_left: updates_left - 1,
                        };
                    } else {
                        auxiliary.configure_departure(self.actor, self.pose, 0);
                        // The final SetVelocity runs after the final rotation
                        // body and before common motion, cancelling the drift.
                        self.velocity = Vector3::default();
                        events.request_destruction = true;
                        self.phase = AwaitingDestruction;
                    }
                    break;
                }
                AwaitingDestruction => return events,
            }
        }
        if self.parent.is_none() {
            self.pose.position.x = self.pose.position.x.wrapping_add(self.velocity.x);
            self.pose.position.y = self.pose.position.y.wrapping_add(self.velocity.y);
            self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningDepartingWingPhase {
    Initializing,
    HiddenTurn { updates_left: u8 },
    VisibleTurn { updates_left: u8 },
    Rolling { updates_left: u8 },
    AwaitingDestruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningDepartingWing {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    pub speed: u8,
    pub trail_enabled: bool,
    actor: ObjectId,
    hidden: bool,
    phase: OpeningDepartingWingPhase,
}

impl OpeningDepartingWing {
    pub fn new(actor: ObjectId, inherited_pose: IntroScenePose) -> Self {
        Self {
            pose: inherited_pose,
            velocity: Vector3::default(),
            speed: 0,
            trail_enabled: false,
            actor,
            hidden: false,
            phase: OpeningDepartingWingPhase::Initializing,
        }
    }

    pub const fn shape(&self) -> ShapeId {
        WING_SHAPE
    }
    pub fn phase(&self) -> OpeningDepartingWingPhase {
        self.phase
    }
    pub fn is_visible(&self) -> bool {
        !self.hidden
    }

    pub fn tick(&mut self, auxiliary: &mut IntroAuxiliaryEffect) -> OpeningWingEvents {
        use OpeningDepartingWingPhase::*;
        let mut events = OpeningWingEvents::default();
        loop {
            match self.phase {
                Initializing => {
                    self.trail_enabled = true;
                    self.pose = OpeningSecondFlybyPlacement::DepartingCraft.pose();
                    self.speed = TURN_SPEED;
                    self.hidden = true;
                    self.phase = HiddenTurn {
                        updates_left: HIDDEN_TURN,
                    };
                }
                HiddenTurn { updates_left } => {
                    self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(-1);
                    if updates_left > 1 {
                        self.phase = HiddenTurn {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.hidden = false;
                    self.phase = VisibleTurn {
                        updates_left: VISIBLE_TURN,
                    };
                }
                VisibleTurn { updates_left } => {
                    self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(-1);
                    if updates_left > 1 {
                        self.phase = VisibleTurn {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.pose.rotation.yaw = DEPARTURE_YAW;
                    self.pose.rotation.pitch = DEPARTURE_PITCH;
                    self.speed = DEPARTURE_SPEED;
                    self.phase = Rolling {
                        updates_left: DEPARTURE_ROLL,
                    };
                }
                Rolling { updates_left } => {
                    self.pose.rotation.roll =
                        self.pose.rotation.roll.wrapping_add(DEPARTURE_ROLL_STEP);
                    if updates_left > 1 {
                        self.phase = Rolling {
                            updates_left: updates_left - 1,
                        };
                    } else {
                        auxiliary.configure_flyby(self.actor, self.pose, 1);
                        events.request_destruction = true;
                        self.phase = AwaitingDestruction;
                    }
                    break;
                }
                AwaitingDestruction => return events,
            }
        }
        self.velocity = flight_velocity(
            self.pose.rotation.pitch,
            self.pose.rotation.yaw,
            self.speed,
            1,
        );
        self.pose.position.x = self.pose.position.x.wrapping_add(self.velocity.x);
        self.pose.position.y = self.pose.position.y.wrapping_add(self.velocity.y);
        self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningWing {
    Attached(OpeningAttachedWing),
    Departing(OpeningDepartingWing),
}

impl OpeningWing {
    pub fn pose(&self) -> IntroScenePose {
        match self {
            Self::Attached(wing) => wing.pose,
            Self::Departing(wing) => wing.pose,
        }
    }

    fn awaiting_destruction(&self) -> bool {
        match self {
            Self::Attached(wing) => wing.phase() == OpeningAttachedWingPhase::AwaitingDestruction,
            Self::Departing(wing) => wing.phase() == OpeningDepartingWingPhase::AwaitingDestruction,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningWingSequenceEvents {
    pub detached: bool,
    pub explosion_audio: Vec<IntroExplosionVolume>,
}

/// Full wing lifetime, including common destruction and effect retirement.
/// The scene supplies birth timing from its actual allocation/traversal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningWingSequence {
    pub wing: OpeningWing,
    destruction: Option<IntroDestructionEffects>,
}

impl OpeningWingSequence {
    pub fn new(wing: OpeningWing) -> Self {
        Self {
            wing,
            destruction: None,
        }
    }

    pub fn craft_is_visible(&self) -> bool {
        self.destruction.is_none()
            && match self.wing {
                OpeningWing::Attached(_) => true,
                OpeningWing::Departing(wing) => wing.is_visible(),
            }
    }

    pub fn craft_has_retired(&self) -> bool {
        self.destruction.is_some()
    }

    pub fn effects(&self) -> impl Iterator<Item = &IntroExplosionActor> {
        self.destruction
            .iter()
            .flat_map(IntroDestructionEffects::actors)
    }

    pub fn is_finished(&self) -> bool {
        self.destruction
            .as_ref()
            .is_some_and(IntroDestructionEffects::is_finished)
    }

    pub fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        if !self.craft_has_retired() {
            if let OpeningWing::Attached(wing) = &mut self.wing {
                wing.publish_from_parent(parent, pose);
            }
        }
    }

    pub fn tick(
        &mut self,
        auxiliary: &mut IntroAuxiliaryEffect,
        context: &IntroDestructionContext,
        birth_timing: IntroExplosionBirthTiming,
    ) -> Result<OpeningWingSequenceEvents, IntroDestructionCapacityError> {
        let mut events = OpeningWingSequenceEvents::default();
        let mut newborn = false;
        if self.destruction.is_none() {
            if self.wing.awaiting_destruction() {
                let profile = IntroExplosionProfile::for_shape(WING_SHAPE)
                    .expect("opening wing belongs to the validated shape catalog");
                let (effects, audio) =
                    IntroDestructionEffects::spawn(profile, self.wing.pose().position, context)?;
                self.destruction = Some(effects);
                events.explosion_audio = audio;
                newborn = true;
            } else {
                let craft_events = match &mut self.wing {
                    OpeningWing::Attached(wing) => wing.tick(auxiliary),
                    OpeningWing::Departing(wing) => wing.tick(auxiliary),
                };
                events.detached = craft_events.detached;
            }
        }
        if !newborn || birth_timing == IntroExplosionBirthTiming::ThisUpdate {
            if let Some(effects) = &mut self.destruction {
                events
                    .explosion_audio
                    .extend(effects.tick_with_birth_timing(context, birth_timing)?);
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore};

    fn ids() -> (ObjectId, ObjectId) {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    WING_SHAPE,
                    Behavior::Effect,
                ))
                .unwrap()
        };
        (allocate(), allocate())
    }

    #[test]
    fn destruction_capacity_failure_is_explicit_and_deferred_birth_starts_at_age_zero() {
        let (actor, parent) = ids();
        for wing in [
            OpeningWing::Attached(OpeningAttachedWing::new(
                actor,
                parent,
                IntroAttachment::default(),
            )),
            OpeningWing::Departing(OpeningDepartingWing::new(actor, IntroScenePose::default())),
        ] {
            let mut sequence = OpeningWingSequence::new(wing);
            let mut auxiliary = IntroAuxiliaryEffect::default();
            let context = IntroDestructionContext::default();
            while !sequence.wing.awaiting_destruction() {
                sequence
                    .tick(
                        &mut auxiliary,
                        &context,
                        IntroExplosionBirthTiming::NextUpdate,
                    )
                    .unwrap();
            }
            let before = sequence.clone();
            assert!(sequence
                .tick(
                    &mut auxiliary,
                    &context,
                    IntroExplosionBirthTiming::NextUpdate
                )
                .is_err());
            assert_eq!(sequence, before);
            let context = IntroDestructionContext {
                available_slots: 3,
                ..context
            };
            sequence
                .tick(
                    &mut auxiliary,
                    &context,
                    IntroExplosionBirthTiming::NextUpdate,
                )
                .unwrap();
            assert!(sequence.craft_has_retired());
            assert!(!sequence.craft_is_visible());
            assert!(sequence.effects().count() > 0);
            assert!(sequence.effects().all(|effect| matches!(
                effect.phase(),
                super::super::intro_destruction::IntroExplosionPhase::Animating { age: 0, .. }
            )));
        }
    }

    #[test]
    fn attachment_wait_detachment_and_last_motion_boundary_are_distinct() {
        let (actor, parent) = ids();
        let mut wing = OpeningAttachedWing::new(actor, parent, IntroAttachment::default());
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let pose = OpeningSecondFlybyPlacement::MiddleCut.pose();
        wing.publish_from_parent(actor, pose);
        assert_eq!(wing.pose, IntroScenePose::default());
        wing.publish_from_parent(parent, pose);
        for _ in 0..ATTACHED_WAIT {
            assert_eq!(wing.tick(&mut auxiliary), OpeningWingEvents::default());
            assert_eq!(wing.pose, pose);
        }
        let event = wing.tick(&mut auxiliary);
        assert!(event.detached && !event.request_destruction);
        assert_eq!(wing.parent(), None);
        let detached = wing;
        wing.publish_from_parent(parent, IntroScenePose::default());
        assert_eq!(wing, detached);
        for _ in 1..DETACHED_SPIN - 1 {
            assert!(!wing.tick(&mut auxiliary).request_destruction);
        }
        let last_position = wing.pose.position;
        assert!(wing.tick(&mut auxiliary).request_destruction);
        assert_eq!(wing.pose.position, last_position);
        assert_eq!(auxiliary.origin, last_position);
        let ended = wing;
        assert_eq!(wing.tick(&mut auxiliary), OpeningWingEvents::default());
        assert_eq!(wing, ended);
    }

    #[test]
    fn departing_wing_changes_visibility_at_loop_boundary_and_moves_before_handoff() {
        let (actor, _) = ids();
        let mut wing = OpeningDepartingWing::new(actor, IntroScenePose::default());
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for _ in 0..HIDDEN_TURN - 1 {
            wing.tick(&mut auxiliary);
            assert!(!wing.is_visible());
        }
        wing.tick(&mut auxiliary);
        assert!(wing.is_visible());
        assert_eq!(
            wing.phase(),
            OpeningDepartingWingPhase::VisibleTurn {
                updates_left: VISIBLE_TURN - 1
            }
        );
        for _ in 0..100 {
            let before = wing.pose.position;
            if wing.tick(&mut auxiliary).request_destruction {
                assert_eq!(auxiliary.origin, before);
                assert_eq!(auxiliary.range, 2);
                assert_ne!(wing.pose.position, before);
                assert!(wing.is_visible());
                let ended = wing;
                assert_eq!(wing.tick(&mut auxiliary), OpeningWingEvents::default());
                assert_eq!(wing, ended);
                return;
            }
        }
        panic!("departure never handed off to destruction");
    }
}
