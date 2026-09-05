//! Native opening flyby effects, with attachment state separate from the
//! published world pose. Source-update timing is independent of rendering.

use super::intro_camera::OpeningCameraCue;
use super::intro_motion::{chase_intro_coordinate, IntroAttachment, IntroScenePose};
use super::object::{Angle, ShapeId, Vector3};

const RIG_WAIT_UPDATES: u8 = 96;
const RETREAT_UPDATES: u8 = 15;
const RETREAT_DEPTH_STEP: i16 = -20;
const LATERAL_CHASE_UPDATES: u8 = 8;
const LATERAL_CHASE_TARGET: i16 = -400;
const SETTLED_PITCH: Angle = Angle::from_units(10);
const STREAK_NEAR_UPDATES: u8 = 35;
const STREAK_FAR_UPDATES: u8 = 45;
const STREAK_DEPTH_STEP: i16 = 100;
const STREAK_SHAPE: ShapeId = ShapeId::from_catalog_index(119);
const STREAK_FAR_SORT_DEPTH: i16 = 15_000;

pub const OPENING_FLYBY_STREAK_OFFSETS: [Vector3; 3] = [
    Vector3 {
        x: 0,
        y: 0,
        z: -500,
    },
    Vector3 {
        x: 0,
        y: 0,
        z: -1_884,
    },
    Vector3 {
        x: 0,
        y: 0,
        z: -3_268,
    },
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OpeningStreakDepthOrder {
    #[default]
    Geometric,
    Far,
}

impl OpeningStreakDepthOrder {
    pub const fn sort_depth_override(self) -> Option<i16> {
        match self {
            Self::Geometric => None,
            Self::Far => Some(STREAK_FAR_SORT_DEPTH),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFlybyStreakPhase {
    InitialWait,
    Preparing,
    Near { updates_left: u8 },
    Far { updates_left: u8 },
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFlybyStreak {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    pub shape: Option<ShapeId>,
    pub depth_order: OpeningStreakDepthOrder,
    phase: OpeningFlybyStreakPhase,
}

impl OpeningFlybyStreak {
    pub fn new(attachment: IntroAttachment) -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment,
            shape: None,
            depth_order: OpeningStreakDepthOrder::Geometric,
            phase: OpeningFlybyStreakPhase::InitialWait,
        }
    }

    pub fn phase(&self) -> OpeningFlybyStreakPhase {
        self.phase
    }

    pub fn is_visible(&self) -> bool {
        self.shape.is_some() && self.phase != OpeningFlybyStreakPhase::Finished
    }

    pub fn publish_from_owner(&mut self, owner: IntroScenePose) {
        if self.phase != OpeningFlybyStreakPhase::Finished {
            self.pose = self.attachment.world_pose(owner);
        }
    }

    /// Returns true only on the update that requests removal. The source
    /// schedules 80 actual depth steps; the final End bypasses its callback.
    pub fn tick(&mut self) -> bool {
        loop {
            match self.phase {
                OpeningFlybyStreakPhase::InitialWait => {
                    self.phase = OpeningFlybyStreakPhase::Preparing;
                    return false;
                }
                OpeningFlybyStreakPhase::Preparing => {
                    self.shape = Some(STREAK_SHAPE);
                    self.phase = OpeningFlybyStreakPhase::Near {
                        updates_left: STREAK_NEAR_UPDATES,
                    };
                }
                OpeningFlybyStreakPhase::Near { updates_left } => {
                    if updates_left != 0 {
                        self.phase = OpeningFlybyStreakPhase::Near {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.depth_order = OpeningStreakDepthOrder::Far;
                    self.phase = OpeningFlybyStreakPhase::Far {
                        updates_left: STREAK_FAR_UPDATES,
                    };
                }
                OpeningFlybyStreakPhase::Far { updates_left } => {
                    if updates_left != 0 {
                        self.phase = OpeningFlybyStreakPhase::Far {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningFlybyStreakPhase::Finished;
                    return true;
                }
                OpeningFlybyStreakPhase::Finished => return false,
            }
        }
        self.attachment.offset.z = self.attachment.offset.z.wrapping_add(STREAK_DEPTH_STEP);
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFlybyRigPhase {
    Waiting { updates_left: u8 },
    Retreating { updates_left: u8 },
    ChasingSide { updates_left: u8 },
    AwaitingOpeningEnd,
    ExitWait,
    Finished,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFlybyRigEvents {
    /// These share the rig's parent attachment list, but their transform owner
    /// is the rig. List membership and transform inheritance are distinct.
    pub streaks: Option<[IntroAttachment; OPENING_FLYBY_STREAK_OFFSETS.len()]>,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFlybyRig {
    pub pose: IntroScenePose,
    pub attachment: IntroAttachment,
    phase: OpeningFlybyRigPhase,
    settling_pitch: bool,
    removal_requested: bool,
}

impl OpeningFlybyRig {
    pub fn new(attachment: IntroAttachment) -> Self {
        Self {
            pose: IntroScenePose::default(),
            attachment,
            phase: OpeningFlybyRigPhase::Waiting {
                updates_left: RIG_WAIT_UPDATES,
            },
            settling_pitch: false,
            removal_requested: false,
        }
    }

    pub fn phase(&self) -> OpeningFlybyRigPhase {
        self.phase
    }

    /// The owning scene can remove the rig without removing its sibling
    /// streaks. Publication and this update's strategy still precede cleanup.
    pub fn request_removal(&mut self) {
        if self.phase != OpeningFlybyRigPhase::Finished {
            self.removal_requested = true;
        }
    }

    /// Publication precedes the rig's own local motion and pitch schedule.
    pub fn publish_from_parent(&mut self, parent: IntroScenePose) {
        if self.phase != OpeningFlybyRigPhase::Finished {
            self.pose = self.attachment.world_pose(parent);
        }
    }

    pub fn tick(&mut self, cue: OpeningCameraCue) -> OpeningFlybyRigEvents {
        let mut events = OpeningFlybyRigEvents::default();
        loop {
            match self.phase {
                OpeningFlybyRigPhase::Waiting { updates_left } => {
                    if updates_left != 0 {
                        self.phase = OpeningFlybyRigPhase::Waiting {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.settling_pitch = true;
                    events.streaks =
                        Some(OPENING_FLYBY_STREAK_OFFSETS.map(|offset| IntroAttachment {
                            offset,
                            ..Default::default()
                        }));
                    self.phase = OpeningFlybyRigPhase::Retreating {
                        updates_left: RETREAT_UPDATES,
                    };
                }
                OpeningFlybyRigPhase::Retreating { updates_left } => {
                    self.attachment.offset.z =
                        self.attachment.offset.z.wrapping_add(RETREAT_DEPTH_STEP);
                    if updates_left > 1 {
                        self.phase = OpeningFlybyRigPhase::Retreating {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    // The final loop iteration falls through without a yield.
                    self.phase = OpeningFlybyRigPhase::ChasingSide {
                        updates_left: LATERAL_CHASE_UPDATES,
                    };
                }
                OpeningFlybyRigPhase::ChasingSide { updates_left } => {
                    self.attachment.offset.x =
                        chase_intro_coordinate(self.attachment.offset.x, LATERAL_CHASE_TARGET);
                    if updates_left > 1 {
                        self.phase = OpeningFlybyRigPhase::ChasingSide {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningFlybyRigPhase::AwaitingOpeningEnd;
                }
                OpeningFlybyRigPhase::AwaitingOpeningEnd => {
                    if cue == OpeningCameraCue::Opening {
                        break;
                    }
                    self.phase = OpeningFlybyRigPhase::ExitWait;
                    break;
                }
                OpeningFlybyRigPhase::ExitWait => {
                    self.phase = OpeningFlybyRigPhase::Finished;
                    events.finished = true;
                    return events; // End skips the final scheduled pitch step.
                }
                OpeningFlybyRigPhase::Finished => return events,
            }
        }
        if self.settling_pitch && self.attachment.rotation.pitch != SETTLED_PITCH {
            // Equality, not a lower bound: non-authored initial angles wrap.
            self.attachment.rotation.pitch = self.attachment.rotation.pitch.wrapping_add(-1);
        }
        if self.removal_requested {
            self.phase = OpeningFlybyRigPhase::Finished;
            events.finished = true;
        }
        events
    }
}

/// One rig and its three separately scheduled attachments. Retain completed
/// slots so a renderer cannot mistake replacement actors for these lifetimes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningFlybyEffects {
    pub rig: OpeningFlybyRig,
    streaks: [Option<OpeningFlybyStreak>; OPENING_FLYBY_STREAK_OFFSETS.len()],
}

impl OpeningFlybyEffects {
    pub fn new(attachment: IntroAttachment) -> Self {
        Self {
            rig: OpeningFlybyRig::new(attachment),
            streaks: [None; OPENING_FLYBY_STREAK_OFFSETS.len()],
        }
    }

    pub fn streaks(&self) -> impl Iterator<Item = &OpeningFlybyStreak> {
        self.streaks.iter().flatten()
    }

    pub fn is_finished(&self) -> bool {
        self.rig.phase() == OpeningFlybyRigPhase::Finished
            && self
                .streaks()
                .all(|streak| streak.phase() == OpeningFlybyStreakPhase::Finished)
    }

    pub fn tick(&mut self, parent: IntroScenePose, cue: OpeningCameraCue) {
        // The common parent's publication visits the rig first, then its
        // streaks. They inherit the freshly published rig pose, not the common
        // parent pose and not the rig's later local-motion result.
        self.rig.publish_from_parent(parent);
        for streak in self.streaks.iter_mut().flatten() {
            streak.publish_from_owner(self.rig.pose);
        }
        if let Some(attachments) = self.rig.tick(cue).streaks {
            for (slot, local) in self.streaks.iter_mut().zip(attachments) {
                *slot = Some(OpeningFlybyStreak::new(local));
            }
        }
        // Newly inserted actors run in this same update, in reverse spawn
        // order. None of the streaks takes its first depth step until next tick.
        for streak in self.streaks.iter_mut().rev().flatten() {
            streak.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_reveals_once_changes_sort_order_and_takes_exactly_eighty_steps() {
        let initial = Vector3 {
            x: 0,
            y: 0,
            z: i16::MAX,
        };
        let mut streak = OpeningFlybyStreak::new(IntroAttachment {
            offset: initial,
            ..Default::default()
        });
        let motion_updates = usize::from(STREAK_NEAR_UPDATES + STREAK_FAR_UPDATES);
        for update in 0..=motion_updates + 1 {
            let finished = streak.tick();
            assert_eq!(finished, update == motion_updates + 1);
            assert_eq!(streak.is_visible(), update != 0 && update <= motion_updates);
            assert_eq!(
                streak.depth_order == OpeningStreakDepthOrder::Far,
                update > usize::from(STREAK_NEAR_UPDATES)
            );
            assert_eq!(
                streak.attachment.offset.z,
                initial
                    .z
                    .wrapping_add((update.min(motion_updates) as i16) * STREAK_DEPTH_STEP)
            );
        }
        let before = streak;
        streak.publish_from_owner(IntroScenePose::default());
        assert!(!streak.tick());
        assert_eq!(streak, before);
    }

    #[test]
    fn removal_on_spawn_update_still_creates_independently_finishing_streaks() {
        let parent = IntroScenePose::default();
        let mut family = OpeningFlybyEffects::new(IntroAttachment::default());
        for _ in 0..RIG_WAIT_UPDATES {
            family.tick(parent, OpeningCameraCue::Opening);
        }
        family.rig.request_removal();
        family.tick(parent, OpeningCameraCue::Opening);
        assert_eq!(family.rig.phase(), OpeningFlybyRigPhase::Finished);
        assert_eq!(family.streaks().count(), OPENING_FLYBY_STREAK_OFFSETS.len());
        assert!(!family.is_finished());
        for _ in 0..=STREAK_NEAR_UPDATES + STREAK_FAR_UPDATES {
            family.tick(parent, OpeningCameraCue::Opening);
        }
        assert!(family.is_finished());
        let before = family.clone();
        family.rig.request_removal();
        family.tick(parent, OpeningCameraCue::FinalCut);
        assert_eq!(family, before);
    }

    #[test]
    fn streaks_use_rig_pose_from_publication_not_its_later_local_motion() {
        let mut family = OpeningFlybyEffects::new(IntroAttachment {
            offset: Vector3 {
                x: 0,
                y: -800,
                z: 1_000,
            },
            ..Default::default()
        });
        for _ in 0..=RIG_WAIT_UPDATES {
            family.tick(IntroScenePose::default(), OpeningCameraCue::Opening);
        }
        let parent = IntroScenePose {
            position: Vector3 {
                x: 53,
                y: 79,
                z: 113,
            },
            ..Default::default()
        };
        let expected_rig_pose = family.rig.attachment.world_pose(parent);
        let expected_streak_poses: Vec<_> = family
            .streaks()
            .map(|streak| streak.attachment.world_pose(expected_rig_pose))
            .collect();
        family.tick(parent, OpeningCameraCue::Opening);
        assert_eq!(family.rig.pose, expected_rig_pose);
        for (streak, expected) in family.streaks().zip(expected_streak_poses) {
            assert_eq!(streak.pose, expected);
        }
    }
}
