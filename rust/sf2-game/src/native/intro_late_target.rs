//! The opening's later camera target and its attached flight effect.
//!
//! Local motion is retained separately from the world pose published by the
//! parent. These source-time actors do not depend on recorded frame tracks.

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::object::{Angle, ShapeId, Vector3};
use super::render::Rotation;

const TARGET_FLIGHT_UPDATES: u8 = 17;
const TARGET_HOLD_UPDATES: u8 = 30;
const EFFECT_WAIT_UPDATES: u8 = 15;
const FLIGHT_SPEED: u8 = 127;
const VELOCITY_MULTIPLIER: i16 = 2;
const EFFECT_SHAPE: ShapeId = ShapeId::from_catalog_index(118);
const TARGET_POSE: IntroScenePose = IntroScenePose {
    position: Vector3 {
        x: -550,
        y: -220,
        z: 150,
    },
    rotation: Rotation {
        pitch: Angle::from_units(20),
        yaw: Angle::HALF_TURN,
        roll: Angle::ZERO,
    },
};
const EFFECT_OFFSET: Vector3 = Vector3 {
    x: -500,
    y: 0,
    z: -3_936,
};

fn doubled_flight(rotation: Rotation) -> Vector3 {
    // The source doubles already-truncated velocity, not the speed input.
    let velocity = flight_velocity(rotation.pitch, rotation.yaw, FLIGHT_SPEED, 1);
    Vector3 {
        x: velocity.x.wrapping_mul(VELOCITY_MULTIPLIER),
        y: velocity.y.wrapping_mul(VELOCITY_MULTIPLIER),
        z: velocity.z.wrapping_mul(VELOCITY_MULTIPLIER),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningLateTargetPhase {
    Initial,
    Flying { updates_left: u8 },
    Holding { updates_left: u8 },
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningLateEffectPhase {
    Waiting { updates_left: u8 },
    Flying,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningLateTargetEffect {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    pub speed: u8,
    pub velocity: Vector3,
    phase: OpeningLateEffectPhase,
}

impl OpeningLateTargetEffect {
    fn new() -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment: IntroAttachment {
                offset: EFFECT_OFFSET,
                ..Default::default()
            },
            speed: 0,
            velocity: Vector3::default(),
            phase: OpeningLateEffectPhase::Waiting {
                updates_left: EFFECT_WAIT_UPDATES,
            },
        }
    }

    pub fn phase(&self) -> OpeningLateEffectPhase {
        self.phase
    }

    pub const fn shape(&self) -> ShapeId {
        EFFECT_SHAPE
    }

    pub fn is_visible(&self) -> bool {
        self.phase != OpeningLateEffectPhase::Finished
    }

    fn tick(&mut self, cue: OpeningCameraCue) {
        match self.phase {
            OpeningLateEffectPhase::Finished => return,
            OpeningLateEffectPhase::Waiting { updates_left } if updates_left > 0 => {
                self.phase = OpeningLateEffectPhase::Waiting {
                    updates_left: updates_left - 1,
                };
            }
            OpeningLateEffectPhase::Waiting { .. } => {
                self.speed = FLIGHT_SPEED;
                self.velocity = doubled_flight(self.attachment.rotation);
                self.phase = OpeningLateEffectPhase::Flying;
            }
            OpeningLateEffectPhase::Flying => {}
        }
        if self.phase == OpeningLateEffectPhase::Flying && cue == OpeningCameraCue::FourthCut {
            self.phase = OpeningLateEffectPhase::Finished;
            return;
        }
        self.attachment.advance(self.velocity);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningLateTargetEvents {
    pub select_as_camera_target: bool,
    pub spawn_effect: bool,
    pub target_finished: bool,
    pub effect_finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningLateCameraTarget {
    pub pose: IntroScenePose,
    pub speed: u8,
    pub velocity: Vector3,
    pub effect: Option<OpeningLateTargetEffect>,
    phase: OpeningLateTargetPhase,
    removal_requested: bool,
}

impl OpeningLateCameraTarget {
    pub fn new(inherited_pose: IntroScenePose) -> Self {
        Self {
            pose: inherited_pose,
            speed: 0,
            velocity: Vector3::default(),
            effect: None,
            phase: OpeningLateTargetPhase::Initial,
            removal_requested: false,
        }
    }

    pub fn phase(&self) -> OpeningLateTargetPhase {
        self.phase
    }

    pub fn is_finished(&self) -> bool {
        self.phase == OpeningLateTargetPhase::Finished
    }

    /// Removal is finalized after this update's strategy and attachment pass.
    pub fn request_removal(&mut self) {
        self.removal_requested = true;
    }

    pub fn tick(&mut self, cue: OpeningCameraCue) -> OpeningLateTargetEvents {
        if self.is_finished() {
            return OpeningLateTargetEvents::default();
        }
        let mut events = OpeningLateTargetEvents::default();
        loop {
            match self.phase {
                OpeningLateTargetPhase::Initial => {
                    self.pose = TARGET_POSE;
                    self.effect = Some(OpeningLateTargetEffect::new());
                    self.speed = FLIGHT_SPEED;
                    self.velocity = doubled_flight(self.pose.rotation);
                    events.select_as_camera_target = true;
                    events.spawn_effect = true;
                    self.phase = OpeningLateTargetPhase::Flying {
                        updates_left: TARGET_FLIGHT_UPDATES,
                    };
                }
                OpeningLateTargetPhase::Flying { updates_left } if updates_left > 0 => {
                    self.phase = OpeningLateTargetPhase::Flying {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningLateTargetPhase::Flying { .. } => {
                    self.speed = 0;
                    self.velocity = Vector3::default();
                    self.phase = OpeningLateTargetPhase::Holding {
                        updates_left: TARGET_HOLD_UPDATES,
                    };
                }
                OpeningLateTargetPhase::Holding { updates_left } if updates_left > 0 => {
                    self.phase = OpeningLateTargetPhase::Holding {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                OpeningLateTargetPhase::Holding { .. } => {
                    self.phase = OpeningLateTargetPhase::Finished;
                    break;
                }
                OpeningLateTargetPhase::Finished => unreachable!(),
            }
        }
        if !self.is_finished() {
            self.pose.position.x = self.pose.position.x.wrapping_add(self.velocity.x);
            self.pose.position.y = self.pose.position.y.wrapping_add(self.velocity.y);
            self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
            if let Some(effect) = &mut self.effect {
                if effect.is_visible() {
                    effect.pose = effect.attachment.world_pose(self.pose);
                }
            }
        }
        let finishing = self.is_finished() || self.removal_requested;
        if let Some(effect) = &mut self.effect {
            let was_visible = effect.is_visible();
            effect.tick(cue);
            if finishing {
                effect.phase = OpeningLateEffectPhase::Finished;
            }
            events.effect_finished = was_visible && !effect.is_visible();
        }
        if finishing {
            self.phase = OpeningLateTargetPhase::Finished;
            events.target_finished = true;
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_local_motion_is_published_on_the_following_parent_update() {
        let mut target = OpeningLateCameraTarget::new(IntroScenePose::default());
        for _ in 0..EFFECT_WAIT_UPDATES {
            target.tick(OpeningCameraCue::ThirdCut);
        }
        let old_attachment = target.effect.unwrap().attachment;
        target.tick(OpeningCameraCue::ThirdCut);
        let effect = target.effect.unwrap();
        assert_eq!(effect.pose, old_attachment.world_pose(target.pose));
        assert_ne!(effect.attachment.offset, old_attachment.offset);
        target.tick(OpeningCameraCue::ThirdCut);
        assert_eq!(
            target.effect.unwrap().pose,
            effect.attachment.world_pose(target.pose)
        );
    }

    #[test]
    fn a_cut_during_the_wait_is_not_latched_or_replaced_by_a_later_cue() {
        let mut target = OpeningLateCameraTarget::new(IntroScenePose::default());
        let events = target.tick(OpeningCameraCue::FourthCut);
        assert!(events.select_as_camera_target && events.spawn_effect);
        assert!(!events.effect_finished);
        for _ in 0..EFFECT_WAIT_UPDATES {
            target.tick(OpeningCameraCue::FinalCut);
        }
        assert_eq!(
            target.effect.unwrap().phase(),
            OpeningLateEffectPhase::Flying
        );
        let events = target.tick(OpeningCameraCue::FourthCut);
        assert!(events.effect_finished);
        assert!(!events.target_finished);
    }

    #[test]
    fn parent_end_removes_surviving_effect_and_finished_family_is_inert() {
        let mut target = OpeningLateCameraTarget::new(IntroScenePose::default());
        for update in 0..TARGET_FLIGHT_UPDATES + TARGET_HOLD_UPDATES {
            let events = target.tick(OpeningCameraCue::ThirdCut);
            assert_eq!(events.select_as_camera_target, update == 0);
            assert!(!events.target_finished);
            assert!(target.effect.unwrap().is_visible());
        }
        let events = target.tick(OpeningCameraCue::ThirdCut);
        assert!(events.target_finished && events.effect_finished);
        let finished = target;
        assert_eq!(target.tick(OpeningCameraCue::FourthCut), Default::default());
        assert_eq!(target, finished);
    }
}
