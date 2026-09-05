//! The later opening flyby's independent camera target. Authored waits and
//! speed changes are discrete game updates, independent of display cadence.

use super::game::flight_velocity;
use super::intro_motion::IntroScenePose;
use super::object::{Angle, Vector3};

const INITIAL_WAIT: u8 = 17;
const DECELERATING_UPDATES: u8 = 27;
const FORWARD_UPDATES: u8 = 40;
const INITIAL_FLIGHT_SPEED: u8 = 60;
const DECELERATION: u8 = 5;
const SECOND_FLIGHT_SPEED: u8 = 30;
const INITIAL_PITCH: Angle = Angle::from_units(20);
const INITIAL_YAW: Angle = Angle::from_units(226);
const SECOND_YAW: Angle = Angle::from_units(236);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondTargetPhase {
    Waiting { updates_left: u8 },
    Decelerating { updates_left: u8 },
    ForwardFlight { updates_left: u8 },
    Holding,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondTargetEvents {
    pub select_as_camera_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondCameraTarget {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    pub speed: u8,
    phase: OpeningSecondTargetPhase,
    selected: bool,
    slowing: bool,
}

impl OpeningSecondCameraTarget {
    pub fn new(inherited_pose: IntroScenePose) -> Self {
        Self {
            pose: inherited_pose,
            velocity: Vector3::default(),
            speed: 0,
            phase: OpeningSecondTargetPhase::Waiting {
                updates_left: INITIAL_WAIT,
            },
            selected: false,
            slowing: false,
        }
    }

    pub fn phase(&self) -> OpeningSecondTargetPhase {
        self.phase
    }
    pub fn is_decelerating(&self) -> bool {
        self.slowing
    }

    /// Holding is a live, persistent camera target, not an actor retirement.
    pub fn tick(&mut self) -> OpeningSecondTargetEvents {
        use OpeningSecondTargetPhase::*;
        let events = OpeningSecondTargetEvents {
            select_as_camera_target: !self.selected,
        };
        self.selected = true;
        loop {
            match self.phase {
                Waiting { updates_left } if updates_left > 0 => {
                    self.phase = Waiting {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                Waiting { .. } => {
                    self.pose.rotation.pitch = INITIAL_PITCH;
                    self.pose.rotation.yaw = INITIAL_YAW;
                    self.speed = INITIAL_FLIGHT_SPEED;
                    self.slowing = true;
                    self.phase = Decelerating {
                        updates_left: DECELERATING_UPDATES,
                    };
                }
                Decelerating { updates_left } if updates_left > 0 => {
                    self.phase = Decelerating {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                Decelerating { .. } => {
                    self.pose.rotation.pitch = Angle::ZERO;
                    self.pose.rotation.yaw = SECOND_YAW;
                    self.speed = SECOND_FLIGHT_SPEED;
                    self.phase = ForwardFlight {
                        updates_left: FORWARD_UPDATES,
                    };
                }
                ForwardFlight { updates_left } if updates_left > 0 => {
                    self.phase = ForwardFlight {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                ForwardFlight { .. } => {
                    self.speed = 0;
                    self.phase = Holding;
                }
                Holding => break,
            }
        }
        // The speed approach precedes movement, including the update that
        // installs it. An exact landing on zero retains the approach for one
        // more update: only an overshoot clamps and clears it. The later
        // speed-30 segment must not inherit the earlier deceleration.
        if self.slowing {
            self.slowing = self.speed >= DECELERATION;
            self.speed = self.speed.saturating_sub(DECELERATION);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deceleration_starts_before_first_flight_and_clears_before_second_segment() {
        let mut target = OpeningSecondCameraTarget::new(IntroScenePose::default());
        for update in 0..INITIAL_WAIT {
            assert_eq!(target.tick().select_as_camera_target, update == 0);
            assert_eq!(target.speed, 0);
        }
        target.tick();
        assert_eq!(target.speed, INITIAL_FLIGHT_SPEED - DECELERATION);
        for _ in 1..DECELERATING_UPDATES {
            target.tick();
        }
        assert_eq!(target.speed, 0);
        assert!(!target.is_decelerating());
        target.tick();
        assert_eq!(target.speed, SECOND_FLIGHT_SPEED);
        assert!(!target.is_decelerating());
    }

    #[test]
    fn exact_zero_keeps_approach_until_the_next_overshoot() {
        let mut target = OpeningSecondCameraTarget::new(IntroScenePose::default());
        for _ in 0..INITIAL_WAIT {
            target.tick();
        }
        for _ in 0..INITIAL_FLIGHT_SPEED / DECELERATION {
            target.tick();
        }
        assert_eq!(target.speed, 0);
        assert!(target.is_decelerating());
        target.tick();
        assert_eq!(target.speed, 0);
        assert!(!target.is_decelerating());
    }

    #[test]
    fn holding_does_not_restart_motion_or_reselect_camera() {
        let mut target = OpeningSecondCameraTarget::new(IntroScenePose::default());
        for _ in 0..=u16::from(INITIAL_WAIT)
            + u16::from(DECELERATING_UPDATES)
            + u16::from(FORWARD_UPDATES)
        {
            target.tick();
        }
        assert_eq!(target.phase(), OpeningSecondTargetPhase::Holding);
        let held = target;
        for _ in 0..256 {
            assert_eq!(target.tick(), OpeningSecondTargetEvents::default());
            assert_eq!(target, held);
        }
    }
}
