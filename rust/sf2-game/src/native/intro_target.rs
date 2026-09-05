//! The opening camera's attached target actor and its two authored flybys.

use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::object::{Angle, ObjectId, ObjectStore, ShapeId, Vector3};

const INITIAL_WAIT_UPDATES: u8 = 100;
const FIRST_FLIGHT_UPDATES: u8 = 20;
const RETARGET_WAIT_UPDATES: u8 = 5;
const SECOND_FLIGHT_UPDATES: u8 = 30;
const TRACKING_SPEED: u8 = 50;
const FIRST_LATERAL_DRIFT: i16 = -5;
const INITIAL_OFFSET: Vector3 = Vector3 { x: 0, y: 0, z: 300 };
const RETARGET_OFFSET: Vector3 = Vector3 {
    x: -500,
    y: 0,
    z: 300,
};
const FIRST_TRACKED_SHAPE: ShapeId = ShapeId::from_catalog_index(64);
const SECOND_TRACKED_SHAPE: ShapeId = ShapeId::from_catalog_index(338);
const MAXIMUM_TRACKING_DISTANCE: i16 = 7_000;
const ANGLE_FRACTION_BITS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningTargetPhase {
    InitialWait { updates_left: u8 },
    FirstFlight { updates_left: u8 },
    AwaitingOpeningEnd,
    RetargetWait { updates_left: u8 },
    SecondFlight { updates_left: u8 },
    AwaitingFirstCutEnd,
    Finished,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningTargetEvents {
    pub select_as_camera_target: bool,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningCameraTarget {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    pub speed: u8,
    pub velocity: Vector3,
    pub last_aim_actor: Option<ObjectId>,
    actor: ObjectId,
    phase: OpeningTargetPhase,
    selected: bool,
}

impl OpeningCameraTarget {
    pub fn new(actor: ObjectId) -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment: IntroAttachment {
                offset: INITIAL_OFFSET,
                ..Default::default()
            },
            velocity: Vector3::default(),
            speed: 0,
            last_aim_actor: None,
            actor,
            phase: OpeningTargetPhase::InitialWait {
                updates_left: INITIAL_WAIT_UPDATES,
            },
            selected: false,
        }
    }

    pub fn phase(&self) -> OpeningTargetPhase {
        self.phase
    }

    /// Called during the parent's update, before this actor's own tick. Do not
    /// immediately republish after local motion: the source retains that motion
    /// for the following parent's attachment pass.
    pub fn publish_from_parent(&mut self, parent: IntroScenePose) {
        if self.phase != OpeningTargetPhase::Finished {
            self.pose = self.attachment.world_pose(parent);
        }
    }

    fn aim_flight(&mut self, actors: &ObjectStore, shape: ShapeId) {
        let mut closest_distance = MAXIMUM_TRACKING_DISTANCE;
        let mut closest = None;
        for (id, candidate) in actors.active_objects() {
            if id == self.actor || candidate.base.shape != shape {
                continue;
            }
            let delta = Vector3 {
                x: candidate.base.position.x.wrapping_sub(self.pose.position.x),
                y: candidate.base.position.y.wrapping_sub(self.pose.position.y),
                z: candidate.base.position.z.wrapping_sub(self.pose.position.z),
            };
            let distance = sf2_xz_angle_distance(delta.x, delta.z);
            // Original search ignores height and keeps the first candidate on
            // equal distance. Negative overflowed distances are not eligible.
            if distance >= 0 && distance < closest_distance {
                closest_distance = distance;
                closest = Some((id, delta));
            }
        }
        self.last_aim_actor = closest.map(|(id, _)| id);
        if let Some((_, delta)) = closest {
            self.pose.rotation.pitch = Angle::from_units(
                (sf2_atan16(delta.y, sf2_xz_angle_distance(delta.x, delta.z))
                    >> ANGLE_FRACTION_BITS) as u8,
            );
            // This direction command negates the coarse yaw, unlike the view
            // camera's fine-angle negation and pitch attenuation.
            self.pose.rotation.yaw = Angle::from_units(
                ((sf2_atan16(delta.x, delta.z) >> ANGLE_FRACTION_BITS) as u8).wrapping_neg(),
            );
        }
        self.attachment.rotation.pitch = self.pose.rotation.pitch;
        self.attachment.rotation.yaw = self.pose.rotation.yaw;
        self.speed = TRACKING_SPEED;
        self.velocity = flight_velocity(
            self.attachment.rotation.pitch,
            self.attachment.rotation.yaw,
            self.speed,
            1,
        );
    }

    pub fn tick(&mut self, cue: OpeningCameraCue, actors: &ObjectStore) -> OpeningTargetEvents {
        if self.phase == OpeningTargetPhase::Finished {
            return OpeningTargetEvents::default();
        }
        let events = OpeningTargetEvents {
            select_as_camera_target: !self.selected,
            finished: false,
        };
        self.selected = true;
        loop {
            match self.phase {
                OpeningTargetPhase::InitialWait { updates_left } => {
                    if updates_left > 0 {
                        self.phase = OpeningTargetPhase::InitialWait {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.aim_flight(actors, FIRST_TRACKED_SHAPE);
                    self.velocity.x = self.velocity.x.wrapping_add(FIRST_LATERAL_DRIFT);
                    self.phase = OpeningTargetPhase::FirstFlight {
                        updates_left: FIRST_FLIGHT_UPDATES,
                    };
                }
                OpeningTargetPhase::FirstFlight { updates_left } => {
                    if updates_left > 0 {
                        self.phase = OpeningTargetPhase::FirstFlight {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.speed = 0;
                    self.velocity = Vector3::default();
                    self.phase = OpeningTargetPhase::AwaitingOpeningEnd;
                }
                OpeningTargetPhase::AwaitingOpeningEnd => {
                    if cue == OpeningCameraCue::Opening {
                        break;
                    }
                    self.attachment.offset = RETARGET_OFFSET;
                    self.phase = OpeningTargetPhase::RetargetWait {
                        updates_left: RETARGET_WAIT_UPDATES,
                    };
                }
                OpeningTargetPhase::RetargetWait { updates_left } => {
                    if updates_left > 0 {
                        self.phase = OpeningTargetPhase::RetargetWait {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.aim_flight(actors, SECOND_TRACKED_SHAPE);
                    self.phase = OpeningTargetPhase::SecondFlight {
                        updates_left: SECOND_FLIGHT_UPDATES,
                    };
                }
                OpeningTargetPhase::SecondFlight { updates_left } => {
                    if updates_left > 1 {
                        self.phase = OpeningTargetPhase::SecondFlight {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningTargetPhase::AwaitingFirstCutEnd;
                }
                OpeningTargetPhase::AwaitingFirstCutEnd => {
                    if cue == OpeningCameraCue::FirstCut {
                        break;
                    }
                    self.phase = OpeningTargetPhase::Finished;
                    // End skips common local-velocity integration.
                    return OpeningTargetEvents {
                        finished: true,
                        ..events
                    };
                }
                OpeningTargetPhase::Finished => unreachable!(),
            }
        }
        self.attachment.advance(self.velocity);
        events
    }
}
