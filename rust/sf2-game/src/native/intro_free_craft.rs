//! The independent craft crossing the opening scene. Its visibility, world
//! motion and selected-player effect are authored state, not sampled frames.

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_destruction::{
    IntroDestructionCapacityError, IntroDestructionContext, IntroDestructionEffects,
    IntroExplosionActor, IntroExplosionProfile, IntroExplosionVolume,
};
use super::intro_motion::IntroScenePose;
use super::object::{ObjectId, ShapeId, Vector3};

const CRAFT_SHAPE: ShapeId = ShapeId::from_catalog_index(64);
const FLIGHT_SPEED: u8 = 20;
const VERTICAL_DRIFT: i16 = -5;
const FAR_SORT_DEPTH: i16 = 15_000;
const REAPPEAR_WAIT_UPDATES: u8 = 14;
const FIRST_POSITION: Vector3 = Vector3 {
    x: -1_700,
    y: 200,
    z: -2_300,
};
const SECOND_POSITION: Vector3 = Vector3 {
    x: 200,
    y: 800,
    z: -2_100,
};
const EFFECT_TRANSITION_MODE: u16 = 2;
const EFFECT_LIMIT: u16 = 255;
const EFFECT_AXIS_MODES: [u8; 3] = [1, 2, 2];
const EFFECT_TARGET_AXIS: u8 = 3;
const EFFECT_FULL_CONTROL: u8 = 31;
const LOW_BYTE_MASK: u16 = 255;
const HIGH_BYTE_MASK: u16 = !LOW_BYTE_MASK;

/// Selected-player auxiliary effect configuration. The spatial origin and
/// owner are distinct from the craft's current pose. The effect's downstream
/// presentation/update service is separate from these source-authored writes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroAuxiliaryEffect {
    pub frozen: bool,
    pub tracking: bool,
    pub axis_modes: [u8; 3],
    pub range: i16,
    pub origin: Vector3,
    pub owner: Option<ObjectId>,
    pub transition_mode: u16,
    pub limit: u16,
    pub remaining: i16,
    pub target_axis: u8,
    pub target_control: u8,
    pub axis_controls: [u8; 3],
}

impl IntroAuxiliaryEffect {
    pub fn configure_flyby(&mut self, actor: ObjectId, pose: IntroScenePose, range: i16) {
        // Only the low byte is doubled. A signed word multiply would change
        // the authored service for values whose low byte carries or wraps.
        let doubled =
            (((range as u16) & HIGH_BYTE_MASK) | u16::from((range as u8).wrapping_mul(2))) as i16;
        self.tracking = false;
        if !self.frozen {
            self.transition_mode = EFFECT_TRANSITION_MODE;
            self.origin = pose.position;
            self.limit = EFFECT_LIMIT;
            self.range = doubled;
            self.remaining = if doubled < 0 { 1 } else { doubled };
            self.owner = Some(actor);
            self.target_axis = EFFECT_TARGET_AXIS;
            self.target_control = EFFECT_FULL_CONTROL;
            self.axis_modes = EFFECT_AXIS_MODES;
            self.axis_controls = [EFFECT_FULL_CONTROL; 3];
        }
        // This refresh is a separate, unconditional service call. A frozen
        // effect still follows its existing owner when that owner invokes it.
        if self.owner == Some(actor) {
            self.origin = pose.position;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFreeCraftPhase {
    Initializing,
    AwaitingFirstCut,
    AwaitingThirdCut,
    Reappeared { updates_left: u8 },
    DeparturePause,
    AwaitingDestruction,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFreeCraftEvents {
    pub queue_departure_audio: bool,
    /// After this update's movement, the scene must transfer the actor to
    /// common destruction on its next update. This is not InvisibleOn/End.
    pub request_destruction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFreeCraft {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    actor: ObjectId,
    hidden: bool,
    phase: OpeningFreeCraftPhase,
}

impl OpeningFreeCraft {
    pub fn new(actor: ObjectId, pose: IntroScenePose) -> Self {
        Self {
            pose,
            velocity: Vector3::default(),
            actor,
            hidden: false,
            phase: OpeningFreeCraftPhase::Initializing,
        }
    }

    pub fn phase(&self) -> OpeningFreeCraftPhase {
        self.phase
    }

    pub fn is_visible(&self) -> bool {
        !self.hidden
    }

    pub const fn shape(&self) -> ShapeId {
        CRAFT_SHAPE
    }

    pub const fn sort_depth_override(&self) -> Option<i16> {
        Some(FAR_SORT_DEPTH)
    }

    pub fn tick(
        &mut self,
        cue: OpeningCameraCue,
        effect: &mut IntroAuxiliaryEffect,
    ) -> OpeningFreeCraftEvents {
        if self.phase == OpeningFreeCraftPhase::AwaitingDestruction {
            return OpeningFreeCraftEvents::default();
        }
        let mut events = OpeningFreeCraftEvents::default();
        loop {
            match self.phase {
                OpeningFreeCraftPhase::Initializing => {
                    self.velocity = flight_velocity(
                        self.pose.rotation.pitch,
                        self.pose.rotation.yaw,
                        FLIGHT_SPEED,
                        1,
                    );
                    self.velocity.y = VERTICAL_DRIFT;
                    self.pose.position = FIRST_POSITION;
                    self.phase = OpeningFreeCraftPhase::AwaitingFirstCut;
                }
                OpeningFreeCraftPhase::AwaitingFirstCut => {
                    if cue != OpeningCameraCue::FirstCut {
                        break;
                    }
                    self.phase = OpeningFreeCraftPhase::AwaitingThirdCut;
                }
                OpeningFreeCraftPhase::AwaitingThirdCut => {
                    if cue != OpeningCameraCue::ThirdCut {
                        self.hidden = true;
                        break;
                    }
                    self.pose.position = SECOND_POSITION;
                    self.hidden = false;
                    self.phase = OpeningFreeCraftPhase::Reappeared {
                        updates_left: REAPPEAR_WAIT_UPDATES,
                    };
                }
                OpeningFreeCraftPhase::Reappeared { updates_left } => {
                    if updates_left != 0 {
                        self.phase = OpeningFreeCraftPhase::Reappeared {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    effect.configure_flyby(self.actor, self.pose, 0);
                    events.queue_departure_audio = true;
                    self.phase = OpeningFreeCraftPhase::DeparturePause;
                    break;
                }
                OpeningFreeCraftPhase::DeparturePause => {
                    events.request_destruction = true;
                    self.phase = OpeningFreeCraftPhase::AwaitingDestruction;
                    break;
                }
                OpeningFreeCraftPhase::AwaitingDestruction => unreachable!(),
            }
        }
        // Invisible states and the final destruction request still integrate
        // velocity. Subsequent updates belong to the destruction consumer.
        self.pose.position.x = self.pose.position.x.wrapping_add(self.velocity.x);
        self.pose.position.y = self.pose.position.y.wrapping_add(self.velocity.y);
        self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
        events
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpeningFreeCraftSequenceEvents {
    pub queue_departure_audio: bool,
    pub explosion_audio: Vec<IntroExplosionVolume>,
}

/// Complete independent-craft lifetime, including the common destruction
/// effects which take over on the update after its health reaches zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningFreeCraftSequence {
    pub craft: OpeningFreeCraft,
    destruction: Option<IntroDestructionEffects>,
}

impl OpeningFreeCraftSequence {
    pub fn new(actor: ObjectId, pose: IntroScenePose) -> Self {
        Self {
            craft: OpeningFreeCraft::new(actor, pose),
            destruction: None,
        }
    }

    pub fn craft_is_visible(&self) -> bool {
        self.destruction.is_none() && self.craft.is_visible()
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

    pub fn tick(
        &mut self,
        cue: OpeningCameraCue,
        auxiliary: &mut IntroAuxiliaryEffect,
        context: &IntroDestructionContext,
    ) -> Result<OpeningFreeCraftSequenceEvents, IntroDestructionCapacityError> {
        let mut events = OpeningFreeCraftSequenceEvents::default();
        if self.destruction.is_none() {
            if self.craft.phase() == OpeningFreeCraftPhase::AwaitingDestruction {
                let profile = IntroExplosionProfile::for_shape(self.craft.shape())
                    .expect("opening craft belongs to the validated shape catalog");
                let (effects, audio) =
                    IntroDestructionEffects::spawn(profile, self.craft.pose.position, context)?;
                self.destruction = Some(effects);
                events.explosion_audio = audio;
            } else {
                events.queue_departure_audio =
                    self.craft.tick(cue, auxiliary).queue_departure_audio;
            }
        }
        if let Some(effects) = &mut self.destruction {
            events.explosion_audio.extend(effects.tick(context)?);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore};

    fn craft() -> OpeningFreeCraft {
        let actor = ObjectStore::new()
            .allocate(Object::new(
                ObjectKind::Effect,
                CRAFT_SHAPE,
                Behavior::Effect,
            ))
            .unwrap();
        OpeningFreeCraft::new(actor, IntroScenePose::default())
    }

    #[test]
    fn departure_effect_precedes_motion_and_destruction_is_a_separate_next_update() {
        let mut craft = craft();
        let mut effect = IntroAuxiliaryEffect::default();
        craft.tick(OpeningCameraCue::FirstCut, &mut effect);
        assert!(!craft.is_visible());
        for _ in 0..REAPPEAR_WAIT_UPDATES {
            assert_eq!(
                craft.tick(OpeningCameraCue::ThirdCut, &mut effect),
                Default::default()
            );
            assert!(craft.is_visible());
        }
        let before_audio = craft.pose;
        let events = craft.tick(OpeningCameraCue::ThirdCut, &mut effect);
        assert!(events.queue_departure_audio);
        assert!(!events.request_destruction);
        assert_eq!(effect.origin, before_audio.position);
        assert_ne!(craft.pose.position, effect.origin);
        let events = craft.tick(OpeningCameraCue::Opening, &mut effect);
        assert!(!events.queue_departure_audio);
        assert!(events.request_destruction);
        assert!(craft.is_visible()); // a health transition is not InvisibleOn
        let before = craft;
        assert_eq!(
            craft.tick(OpeningCameraCue::Opening, &mut effect),
            Default::default()
        );
        assert_eq!(craft, before);
    }

    #[test]
    fn missing_first_cut_does_not_jump_to_reappearance_or_destroy_the_craft() {
        let mut craft = craft();
        let mut effect = IntroAuxiliaryEffect::default();
        for _ in 0..1_024 {
            assert_eq!(
                craft.tick(OpeningCameraCue::ThirdCut, &mut effect),
                Default::default()
            );
        }
        assert_eq!(craft.phase(), OpeningFreeCraftPhase::AwaitingFirstCut);
        assert!(craft.is_visible());
        assert_eq!(effect, IntroAuxiliaryEffect::default());
    }

    #[test]
    fn frozen_effect_preserves_configuration_but_refreshes_only_its_owner() {
        let craft = craft();
        let pose = IntroScenePose {
            position: FIRST_POSITION,
            ..Default::default()
        };
        let mut effect = IntroAuxiliaryEffect {
            frozen: true,
            tracking: true,
            owner: Some(craft.actor),
            range: -1,
            ..Default::default()
        };
        let mut expected = effect;
        expected.tracking = false;
        expected.origin = pose.position;
        effect.configure_flyby(craft.actor, pose, 0);
        assert_eq!(effect, expected);
        effect.owner = None;
        let expected = effect;
        effect.configure_flyby(craft.actor, IntroScenePose::default(), 0);
        assert_eq!(effect, expected);
    }
}
