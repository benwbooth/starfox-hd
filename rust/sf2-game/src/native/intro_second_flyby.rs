//! Authored poses and attached trail for the later opening flyby. The craft's
//! parent choreography and recursively linked children are separate consumers.

use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::object::{Angle, ObjectId, ShapeId, Vector3};
use super::render::Rotation;

const TRAIL_SHAPE: ShapeId = ShapeId::from_catalog_index(119);
const TRAIL_UPDATES: u8 = 20;
const TRAIL_DEPTH_STEP: i16 = 100;
const TRAIL_DEPTH_OFFSET: u8 = 5;
const FLARE_SHAPE: ShapeId = ShapeId::from_catalog_index(48);
pub const SECOND_FLYBY_FLARE_ATTACHMENT: IntroAttachment = IntroAttachment {
    offset: Vector3 { x: 0, y: 0, z: 20 },
    rotation: Rotation {
        pitch: Angle::from_units(192),
        yaw: Angle::ZERO,
        roll: Angle::ZERO,
    },
};
pub const SECOND_FLYBY_TRAIL_ATTACHMENT: IntroAttachment = IntroAttachment {
    offset: Vector3 {
        x: 0,
        y: 30,
        z: -984,
    },
    rotation: Rotation {
        pitch: Angle::ZERO,
        yaw: Angle::ZERO,
        roll: Angle::ZERO,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyPlacement {
    Arrival,
    MiddleCut,
    FinalCut,
    DepartingCraft,
}

impl OpeningSecondFlybyPlacement {
    /// Original indexed placement replaces all six pose channels at once.
    pub const fn pose(self) -> IntroScenePose {
        match self {
            Self::Arrival => IntroScenePose {
                position: Vector3 {
                    x: -1_500,
                    y: -1_500,
                    z: 2_964,
                },
                rotation: Rotation {
                    pitch: Angle::from_units(16),
                    yaw: Angle::from_units(136),
                    roll: Angle::ZERO,
                },
            },
            Self::MiddleCut => IntroScenePose {
                position: Vector3 {
                    x: -250,
                    y: -150,
                    z: 2_100,
                },
                rotation: Rotation {
                    pitch: Angle::ZERO,
                    yaw: Angle::HALF_TURN,
                    roll: Angle::ZERO,
                },
            },
            Self::FinalCut => IntroScenePose {
                position: Vector3 {
                    x: -200,
                    y: -100,
                    z: 0,
                },
                rotation: Rotation {
                    pitch: Angle::from_units(38),
                    yaw: Angle::from_units(236),
                    roll: Angle::from_units(246),
                },
            },
            Self::DepartingCraft => IntroScenePose {
                position: Vector3 {
                    x: -315,
                    y: 25,
                    z: 2_840,
                },
                rotation: Rotation {
                    pitch: Angle::ZERO,
                    yaw: Angle::from_units(226),
                    roll: Angle::from_units(216),
                },
            },
        }
    }
}

/// The later engine flare has no authored timeout. Its initial path disables
/// contact and enables the sort override, then permanently selects the common
/// attachment-update service. Scene teardown owns its eventual removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondFlybyFlare {
    pub pose: IntroScenePose,
    parent: ObjectId,
    initialized: bool,
}

impl OpeningSecondFlybyFlare {
    pub fn new(parent: ObjectId) -> Self {
        Self {
            pose: IntroScenePose::default(),
            parent,
            initialized: false,
        }
    }

    pub const fn shape(&self) -> ShapeId {
        FLARE_SHAPE
    }

    pub const fn attachment(&self) -> IntroAttachment {
        SECOND_FLYBY_FLARE_ATTACHMENT
    }

    pub fn parent(&self) -> ObjectId {
        self.parent
    }

    pub fn contact_disabled(&self) -> bool {
        self.initialized
    }

    pub fn sort_override(&self) -> bool {
        self.initialized
    }

    pub fn publish_from_parent(&mut self, parent: ObjectId, pose: IntroScenePose) {
        if parent == self.parent {
            self.pose = self.attachment().world_pose(pose);
        }
    }

    /// This actor does not independently move or animate. Publication belongs
    /// to the parent's attachment pass, including after the initial path ends.
    pub fn tick(&mut self) {
        self.initialized = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondFlybyTrail {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    updates_left: u8,
}

impl Default for OpeningSecondFlybyTrail {
    fn default() -> Self {
        Self::new()
    }
}

impl OpeningSecondFlybyTrail {
    pub fn new() -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment: SECOND_FLYBY_TRAIL_ATTACHMENT,
            updates_left: TRAIL_UPDATES,
        }
    }

    pub const fn shape(&self) -> ShapeId {
        TRAIL_SHAPE
    }
    pub const fn depth_offset(&self) -> u8 {
        TRAIL_DEPTH_OFFSET
    }
    pub fn is_finished(&self) -> bool {
        self.updates_left == 0
    }

    /// Publish in the parent's attachment pass, before this actor's local
    /// step. A newly created trail can miss that pass on its birth update.
    pub fn publish_from_parent(&mut self, parent: IntroScenePose) {
        if !self.is_finished() {
            self.pose = self.attachment.world_pose(parent);
        }
    }

    /// Returns true only for the transition to End. The last local increment
    /// executes before End, but cannot produce another visible world pose.
    pub fn tick(&mut self) -> bool {
        if self.is_finished() {
            return false;
        }
        self.attachment.offset.z = self.attachment.offset.z.wrapping_add(TRAIL_DEPTH_STEP);
        self.updates_left -= 1;
        self.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore};

    #[test]
    fn persistent_flare_uses_only_its_owner_publication_and_never_times_out() {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    FLARE_SHAPE,
                    Behavior::Effect,
                ))
                .unwrap()
        };
        let parent = allocate();
        let unrelated = allocate();
        let mut flare = OpeningSecondFlybyFlare::new(parent);
        assert!(!flare.contact_disabled());
        assert!(!flare.sort_override());
        flare.tick();
        assert!(flare.contact_disabled());
        assert!(flare.sort_override());
        assert_eq!(flare.pose, IntroScenePose::default());
        let pose = OpeningSecondFlybyPlacement::FinalCut.pose();
        flare.publish_from_parent(unrelated, pose);
        assert_eq!(flare.pose, IntroScenePose::default());
        flare.publish_from_parent(parent, pose);
        assert_eq!(flare.pose, SECOND_FLYBY_FLARE_ATTACHMENT.world_pose(pose));
        let initialized = flare;
        for _ in 0..1024 {
            flare.tick();
        }
        assert_eq!(flare, initialized);
    }

    #[test]
    fn birth_step_does_not_publish_and_end_never_restarts() {
        let mut trail = OpeningSecondFlybyTrail::new();
        assert!(!trail.tick());
        assert_eq!(trail.pose, IntroScenePose::default());
        for _ in 1..TRAIL_UPDATES - 1 {
            assert!(!trail.tick());
        }
        assert!(trail.tick());
        assert_eq!(
            trail.attachment.offset.z,
            SECOND_FLYBY_TRAIL_ATTACHMENT.offset.z + i16::from(TRAIL_UPDATES) * TRAIL_DEPTH_STEP
        );
        let ended = trail;
        trail.publish_from_parent(OpeningSecondFlybyPlacement::Arrival.pose());
        assert!(!trail.tick());
        assert_eq!(trail, ended);
    }
}
