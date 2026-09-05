//! Source-authored opening camera, driven by scene cues, not display frames.
//! Surrounding actors and renderer integration remain separate from this rig.

use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

use super::intro_motion::{chase_intro_coordinate, settle_intro_camera_roll, AttractCameraAngles};
use super::object::Vector3;

const SLOW_FLIGHT_UPDATES: u8 = 18;
const FLIGHT_PAUSE_UPDATES: u8 = 5;
const AIMED_FLIGHT_UPDATES: u8 = 20;
const SLOW_FLIGHT_SPEED: i16 = 5;
const AIMED_FLIGHT_SPEED: i16 = 8;
const AIMED_FLIGHT_X: i16 = 150;
const AIMED_FLIGHT_Y: i16 = 100;

/// Authored camera cuts consumed by this scene, decoded from the original
/// coordinate tables. These are not camera poses sampled from a recording.
pub const OPENING_CAMERA_WAYPOINTS: [Vector3; 5] = [
    Vector3 {
        x: -30,
        y: 0,
        z: 1_418,
    },
    Vector3 {
        x: -1_000,
        y: -1_000,
        z: 2_200,
    },
    Vector3 { x: 400, y: 0, z: 0 },
    Vector3 {
        x: 0,
        y: 0,
        z: 2_000,
    },
    Vector3 { x: 0, y: 0, z: 0 },
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroCameraView {
    pub position: Vector3,
    pub angles: AttractCameraAngles,
}

impl IntroCameraView {
    /// Publish the rig position and look toward the separate tracking actor.
    /// Negation precedes the signed pitch half, retaining its fractional part.
    pub fn track_opening_target(&mut self, position: Vector3, target: Vector3) {
        let dx = target.x.wrapping_sub(position.x);
        let dy = target.y.wrapping_sub(position.y);
        let dz = target.z.wrapping_sub(position.z);
        let pitch = sf2_atan16(dy, sf2_xz_angle_distance(dx, dz)).wrapping_neg() as i16;
        self.position = position;
        self.angles.pitch = (pitch >> 1) as u16;
        self.angles.yaw = sf2_atan16(dx, dz);
        self.angles.roll = settle_intro_camera_roll(self.angles.roll);
    }
}

/// Scene cues are equality gates. Missing a cue must not silently skip a cut.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OpeningCameraCue {
    #[default]
    Opening,
    FirstCut,
    SecondCut,
    ThirdCut,
    FourthCut,
    FinalCut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningCameraPhase {
    InitialWait,
    FollowingScroll,
    AwaitingSecondCut,
    AwaitingThirdCut,
    AwaitingFourthCut,
    SlowFlight { updates_left: u8 },
    FlightPause { updates_left: u8 },
    AimedFlight { updates_left: u8 },
    AwaitingFinalCut,
    Holding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningCameraRig {
    pub position: Vector3,
    pub velocity: Vector3,
    phase: OpeningCameraPhase,
    next_waypoint: usize,
}

impl OpeningCameraRig {
    pub fn new(position: Vector3) -> Self {
        Self {
            position,
            velocity: Vector3::default(),
            phase: OpeningCameraPhase::InitialWait,
            next_waypoint: 0,
        }
    }

    pub fn phase(&self) -> OpeningCameraPhase {
        self.phase
    }

    pub fn cuts_taken(&self) -> usize {
        self.next_waypoint
    }

    fn cut(&mut self) {
        self.position = OPENING_CAMERA_WAYPOINTS[self.next_waypoint];
        self.next_waypoint += 1;
    }

    /// One original scene update. The first update waits without publishing
    /// a view. All subsequent yields, including Hold, keep the
    /// camera tracking. Cuts can fall through to movement in the same update.
    pub fn tick(
        &mut self,
        cue: OpeningCameraCue,
        scene_depth_velocity: i16,
        target: Vector3,
        view: &mut IntroCameraView,
    ) {
        let publish = self.phase != OpeningCameraPhase::InitialWait;
        loop {
            match self.phase {
                OpeningCameraPhase::InitialWait => {
                    self.phase = OpeningCameraPhase::FollowingScroll;
                    break;
                }
                OpeningCameraPhase::FollowingScroll => {
                    if cue != OpeningCameraCue::FirstCut {
                        self.velocity.z = scene_depth_velocity;
                        break;
                    }
                    self.cut();
                    self.phase = OpeningCameraPhase::AwaitingSecondCut;
                }
                OpeningCameraPhase::AwaitingSecondCut => {
                    if cue != OpeningCameraCue::SecondCut {
                        break;
                    }
                    self.cut();
                    self.velocity.z = 0;
                    self.phase = OpeningCameraPhase::AwaitingThirdCut;
                }
                OpeningCameraPhase::AwaitingThirdCut => {
                    if cue != OpeningCameraCue::ThirdCut {
                        break;
                    }
                    self.cut();
                    self.phase = OpeningCameraPhase::AwaitingFourthCut;
                }
                OpeningCameraPhase::AwaitingFourthCut => {
                    if cue != OpeningCameraCue::FourthCut {
                        break;
                    }
                    self.cut();
                    self.phase = OpeningCameraPhase::SlowFlight {
                        updates_left: SLOW_FLIGHT_UPDATES,
                    };
                }
                OpeningCameraPhase::SlowFlight { updates_left } => {
                    self.velocity.z = SLOW_FLIGHT_SPEED;
                    if updates_left > 1 {
                        self.phase = OpeningCameraPhase::SlowFlight {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningCameraPhase::FlightPause {
                        updates_left: FLIGHT_PAUSE_UPDATES,
                    };
                }
                OpeningCameraPhase::FlightPause { updates_left } => {
                    if updates_left > 0 {
                        self.phase = OpeningCameraPhase::FlightPause {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningCameraPhase::AimedFlight {
                        updates_left: AIMED_FLIGHT_UPDATES,
                    };
                }
                OpeningCameraPhase::AimedFlight { updates_left } => {
                    // Position chasing uses the same wrapping one-eighth
                    // arithmetic as the fine-angle chase, not float easing.
                    self.position.x = chase_intro_coordinate(self.position.x, AIMED_FLIGHT_X);
                    self.position.y = chase_intro_coordinate(self.position.y, AIMED_FLIGHT_Y);
                    self.velocity.z = AIMED_FLIGHT_SPEED;
                    if updates_left > 1 {
                        self.phase = OpeningCameraPhase::AimedFlight {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.phase = OpeningCameraPhase::AwaitingFinalCut;
                }
                OpeningCameraPhase::AwaitingFinalCut => {
                    if cue != OpeningCameraCue::FinalCut {
                        break;
                    }
                    self.velocity.z = 0;
                    self.cut();
                    self.phase = OpeningCameraPhase::Holding;
                }
                OpeningCameraPhase::Holding => break,
            }
        }
        self.position.x = self.position.x.wrapping_add(self.velocity.x);
        self.position.y = self.position.y.wrapping_add(self.velocity.y);
        self.position.z = self.position.z.wrapping_add(self.velocity.z);
        // This rig does not opt into common scene scrolling. It imports the
        // scene's depth velocity while following, then retains it at the first
        // cut. Adding global scrolling here would double its opening speed.
        if publish {
            view.track_opening_target(self.position, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_update_does_not_publish_or_import_motion() {
        let origin = Vector3 {
            x: 101,
            y: -77,
            z: 301,
        };
        let mut rig = OpeningCameraRig::new(origin);
        let mut view = IntroCameraView::default();
        let original_view = view;
        rig.tick(OpeningCameraCue::Opening, 10, Vector3::default(), &mut view);
        assert_eq!(view, original_view);
        assert_eq!(rig.position, origin);
        rig.tick(OpeningCameraCue::Opening, 10, Vector3::default(), &mut view);
        assert_eq!(rig.position.z, origin.z + 10);
        assert_eq!(view.position, rig.position);
    }

    #[test]
    fn cut_retains_imported_velocity_until_second_cut() {
        let mut rig = OpeningCameraRig::new(Vector3::default());
        let mut view = IntroCameraView::default();
        for cue in [
            OpeningCameraCue::Opening,
            OpeningCameraCue::Opening,
            OpeningCameraCue::FirstCut,
        ] {
            rig.tick(cue, 10, Vector3::default(), &mut view);
        }
        assert_eq!(rig.position.z, OPENING_CAMERA_WAYPOINTS[0].z + 10);
        rig.tick(
            OpeningCameraCue::FirstCut,
            -50,
            Vector3::default(),
            &mut view,
        );
        assert_eq!(rig.velocity.z, 10);
        rig.tick(
            OpeningCameraCue::SecondCut,
            -50,
            Vector3::default(),
            &mut view,
        );
        assert_eq!(rig.position, OPENING_CAMERA_WAYPOINTS[1]);
        assert_eq!(rig.velocity.z, 0);
    }

    #[test]
    fn missed_cue_keeps_waiting_instead_of_skipping_to_later_cut() {
        let mut rig = OpeningCameraRig::new(Vector3::default());
        let mut view = IntroCameraView::default();
        for cue in [
            OpeningCameraCue::Opening,
            OpeningCameraCue::SecondCut,
            OpeningCameraCue::FinalCut,
        ] {
            rig.tick(cue, 10, Vector3::default(), &mut view);
        }
        assert_eq!(rig.cuts_taken(), 0);
        assert_eq!(rig.phase(), OpeningCameraPhase::FollowingScroll);
        rig.tick(
            OpeningCameraCue::FirstCut,
            10,
            Vector3::default(),
            &mut view,
        );
        assert_eq!(rig.cuts_taken(), 1);
    }
}
