//! Authored opening formation motion. Scene composition and rendering are
//! deliberately separate from these source-time craft updates.

use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_free_craft::IntroAuxiliaryEffect;
use super::intro_motion::{chase_intro_coordinate, IntroScenePose};
use super::intro_root::OpeningFormationMember;
use super::object::{Angle, ObjectId, ObjectStore, ShapeId, StereoPosition, Vector3};

const MEMBER_COUNT: usize = 3;
const SHOT_COUNT: usize = 4;
const ANGLE_CHASE_DIVISOR: i16 = 8;
const ANGLE_FRACTION_BITS: u32 = 8;
const INITIAL_WAIT: u8 = 16;
const INITIAL_HOLD: u8 = 53;
const INITIAL_BANK_UPDATES: u8 = 15;
const TRACKING_BANK_UPDATES: u8 = 16;
const CLIMB_UPDATES: u8 = 10;
const REAPPEAR_WAIT: u8 = 4;
const REAPPEAR_BANK_UPDATES: u8 = 20;
const SECOND_SPEED: u8 = 11;
const TRACKING_SPEED: u8 = 50;
const REAPPEAR_SPEED: u8 = 30;
const EXIT_SPEED: u8 = 40;
const TRACKING_ROLL: Angle = Angle::from_units(40);
const ARRIVAL_ROLL: Angle = Angle::from_units(246);
const TRACKING_DRIFT: i16 = -5;
const TARGET_SHAPE: ShapeId = ShapeId::from_catalog_index(338);
const CRAFT_SHAPE: ShapeId = ShapeId::from_catalog_index(89);
const TRACKING_RANGE: i16 = 7_000;
const NO_IMPULSE: Vector3 = Vector3 { x: 0, y: 0, z: 0 };
const AUDIO_RANGE: u16 = 5_120;
const AUDIO_RIGHT_START: u8 = 16;
const AUDIO_REAR_CENTER_START: u8 = 112;
const AUDIO_LEFT_START: u8 = 144;
const AUDIO_FRONT_CENTER_START: u8 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFormationShot {
    Arrival,
    Pursuit,
    Reappearance,
    Exit,
}

impl OpeningFormationShot {
    const fn index(self) -> usize {
        match self {
            Self::Arrival => 0,
            Self::Pursuit => 1,
            Self::Reappearance => 2,
            Self::Exit => 3,
        }
    }
}

const fn member_index(member: OpeningFormationMember) -> usize {
    match member {
        OpeningFormationMember::First => 0,
        OpeningFormationMember::Second => 1,
        OpeningFormationMember::Third => 2,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFormationPlacement {
    pub position: Vector3,
    pub pitch: Angle,
    pub yaw: Angle,
    pub duration: u8,
}

// Transposed from the authored per-axis tables, not sampled frame data.
const POSITIONS: [[Vector3; MEMBER_COUNT]; SHOT_COUNT] = [
    [
        Vector3 {
            x: 150,
            y: 300,
            z: -200,
        },
        Vector3 {
            x: 300,
            y: 100,
            z: -800,
        },
        Vector3 {
            x: -300,
            y: 200,
            z: -700,
        },
    ],
    [
        Vector3 {
            x: -130,
            y: 50,
            z: 1_397,
        },
        Vector3 {
            x: 50,
            y: -10,
            z: 1_197,
        },
        Vector3 {
            x: -150,
            y: 50,
            z: 1_197,
        },
    ],
    [
        Vector3 {
            x: 298,
            y: 278,
            z: -670,
        },
        Vector3 {
            x: 188,
            y: 328,
            z: -760,
        },
        Vector3 {
            x: 268,
            y: 58,
            z: -260,
        },
    ],
    [
        Vector3 {
            x: -139,
            y: 133,
            z: 1_547,
        },
        Vector3 {
            x: -339,
            y: -17,
            z: 1_877,
        },
        Vector3 {
            x: 100,
            y: 100,
            z: -100,
        },
    ],
];
const PITCHES: [[u8; MEMBER_COUNT]; SHOT_COUNT] =
    [[246, 241, 246], [2, 5, 5], [236, 236, 236], [246, 0, 0]];
const YAWS: [[u8; MEMBER_COUNT]; SHOT_COUNT] =
    [[8, 8, 8], [0, 0, 248], [0; MEMBER_COUNT], [0; MEMBER_COUNT]];
const DURATIONS: [[u8; MEMBER_COUNT]; SHOT_COUNT] = [
    [20, 15, 15],
    [20, 25, 32],
    [100, 100, 0],
    [19; MEMBER_COUNT],
];
const ARRIVAL_SPEEDS: [u8; MEMBER_COUNT] = [18, 15, 20];
const IMPULSES: [[Vector3; MEMBER_COUNT]; SHOT_COUNT - 1] = [
    [
        Vector3 { x: 5, y: 5, z: 30 },
        Vector3 {
            x: 10,
            y: -10,
            z: 10,
        },
        Vector3 {
            x: 10,
            y: 20,
            z: 10,
        },
    ],
    [Vector3 { x: 5, y: -8, z: 15 }, NO_IMPULSE, NO_IMPULSE],
    [
        Vector3 { x: 15, y: 15, z: 0 },
        Vector3 { x: 0, y: -30, z: 0 },
        NO_IMPULSE,
    ],
];
const ROLLS: [[u8; MEMBER_COUNT]; SHOT_COUNT - 1] = [[50, 206, 236], [30, 0, 0], [40, 216, 40]];

pub fn opening_formation_placement(
    member: OpeningFormationMember,
    shot: OpeningFormationShot,
) -> OpeningFormationPlacement {
    let member = member_index(member);
    let shot = shot.index();
    OpeningFormationPlacement {
        position: POSITIONS[shot][member],
        pitch: Angle::from_units(PITCHES[shot][member]),
        yaw: Angle::from_units(YAWS[shot][member]),
        duration: DURATIONS[shot][member],
    }
}

/// Coarse heading easing wraps before choosing the shortest signed arc.
/// Applying the fine-angle helper to widened bytes loses that wrap.
pub fn chase_formation_angle(current: Angle, target: Angle) -> Angle {
    let delta = i16::from(target.units().wrapping_sub(current.units()) as i8);
    if delta == 0 {
        return current;
    }
    let numerator = if delta < 0 {
        delta.min(-ANGLE_CHASE_DIVISOR)
    } else {
        delta.max(ANGLE_CHASE_DIVISOR)
    };
    current.wrapping_add((numerator / ANGLE_CHASE_DIVISOR) as i8)
}

/// The separate impulse is applied before its decay, independently of the
/// craft's heading-derived flight velocity later in the same update.
pub fn advance_formation_impulse(pose: &mut IntroScenePose, impulse: &mut Vector3) {
    pose.rotation.roll = chase_formation_angle(pose.rotation.roll, Angle::ZERO);
    pose.position.x = pose.position.x.wrapping_add(impulse.x);
    pose.position.y = pose.position.y.wrapping_add(impulse.y);
    pose.position.z = pose.position.z.wrapping_add(impulse.z);
    impulse.x = chase_intro_coordinate(impulse.x, 0);
    impulse.y = chase_intro_coordinate(impulse.y, 0);
    impulse.z = chase_intro_coordinate(impulse.z, 0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFormationAudio {
    pub source: Vector3,
}

impl OpeningFormationAudio {
    /// Fixed-range pursuit sound: unlike burst audio, this has no distance
    /// attenuation. Preserve the signed wrapped range comparison, including
    /// the original result for extreme translated listener coordinates.
    pub fn spatial(self, listener: IntroScenePose) -> Option<StereoPosition> {
        let x = self.source.x.wrapping_sub(listener.position.x);
        let z = self.source.z.wrapping_sub(listener.position.z);
        let squared = i64::from(x) * i64::from(x) + i64::from(z) * i64::from(z);
        let distance = (squared as u32).isqrt() as u16;
        if distance.wrapping_sub(AUDIO_RANGE) as i16 >= 0 {
            return None;
        }
        let relative = ((sf2_atan16(x, z) >> ANGLE_FRACTION_BITS) as u8)
            .wrapping_sub(listener.rotation.yaw.units());
        Some(
            if (AUDIO_RIGHT_START..AUDIO_REAR_CENTER_START).contains(&relative) {
                StereoPosition::Right
            } else if (AUDIO_LEFT_START..AUDIO_FRONT_CENTER_START).contains(&relative) {
                StereoPosition::Left
            } else {
                StereoPosition::Center
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFormationPhase {
    Initializing,
    InitialWait { updates_left: u8 },
    InitialHold { updates_left: u8 },
    ArrivalBank { updates_left: u8 },
    AwaitingFirstCut,
    FirstCutPause,
    PursuitBank { updates_left: u8 },
    TrackingBank { updates_left: u8 },
    Tracking { updates_left: u8 },
    Climbing { updates_left: u8 },
    AwaitingSecondCut,
    AwaitingThirdCut,
    Reappeared { updates_left: u8 },
    DepartureBank { updates_left: u8 },
    AwaitingFourthCut,
    Exiting { updates_left: u8 },
    AwaitingDestruction,
    Finished,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFormationEvents {
    /// The fixed-range sound request retains its pre-flight source position.
    /// The scene audio service must still apply its listener/range policy.
    pub pursuit_audio: Option<OpeningFormationAudio>,
    /// Transfer to common destruction on the following update. The third
    /// member still performs this update's impulse, flight and elapsed tick.
    pub request_destruction: bool,
    pub finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningFormationCraft {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    pub impulse: Vector3,
    pub speed: u8,
    pub trail_enabled: bool,
    pub elapsed_updates: u16,
    pub tracked_actor: Option<ObjectId>,
    actor: ObjectId,
    member: OpeningFormationMember,
    shot: OpeningFormationShot,
    phase: OpeningFormationPhase,
    hidden: bool,
}

impl OpeningFormationCraft {
    pub fn new(
        actor: ObjectId,
        member: OpeningFormationMember,
        inherited_pose: IntroScenePose,
    ) -> Self {
        Self {
            actor,
            member,
            pose: inherited_pose,
            velocity: Vector3::default(),
            impulse: Vector3::default(),
            speed: 0,
            trail_enabled: false,
            elapsed_updates: 0,
            tracked_actor: None,
            shot: OpeningFormationShot::Arrival,
            phase: OpeningFormationPhase::Initializing,
            hidden: false,
        }
    }

    pub fn phase(&self) -> OpeningFormationPhase {
        self.phase
    }
    pub fn shot(&self) -> OpeningFormationShot {
        self.shot
    }
    pub const fn shape(&self) -> ShapeId {
        CRAFT_SHAPE
    }
    pub fn is_visible(&self) -> bool {
        !self.hidden && self.phase != OpeningFormationPhase::Finished
    }

    fn place(&mut self, shot: OpeningFormationShot) {
        self.shot = shot;
        let placement = opening_formation_placement(self.member, shot);
        self.pose.position = placement.position;
        self.pose.rotation.pitch = placement.pitch;
        self.pose.rotation.yaw = placement.yaw;
    }

    fn load_impulse(&mut self) {
        self.impulse = IMPULSES[self.shot.index()][member_index(self.member)];
        self.pose.rotation.roll =
            Angle::from_units(ROLLS[self.shot.index()][member_index(self.member)]);
    }

    fn duration(&self) -> u8 {
        opening_formation_placement(self.member, self.shot).duration
    }

    fn select_target(&mut self, actors: &ObjectStore) {
        let mut nearest = TRACKING_RANGE;
        self.tracked_actor = None;
        for (id, candidate) in actors.active_objects() {
            if id == self.actor || candidate.base.shape != TARGET_SHAPE {
                continue;
            }
            let dx = candidate.base.position.x.wrapping_sub(self.pose.position.x);
            let dz = candidate.base.position.z.wrapping_sub(self.pose.position.z);
            let distance = sf2_xz_angle_distance(dx, dz);
            if distance >= 0 && distance < nearest {
                nearest = distance;
                self.tracked_actor = Some(id);
            }
        }
    }

    fn aim(&mut self, actors: &ObjectStore) {
        let Some(target) = self.tracked_actor.and_then(|id| actors.get(id)) else {
            return;
        };
        let dx = target.base.position.x.wrapping_sub(self.pose.position.x);
        let dy = target.base.position.y.wrapping_sub(self.pose.position.y);
        let dz = target.base.position.z.wrapping_sub(self.pose.position.z);
        let pitch = Angle::from_units(
            (sf2_atan16(dy, sf2_xz_angle_distance(dx, dz)) >> ANGLE_FRACTION_BITS) as u8,
        );
        let yaw =
            Angle::from_units(((sf2_atan16(dx, dz) >> ANGLE_FRACTION_BITS) as u8).wrapping_neg());
        self.pose.rotation.pitch = chase_formation_angle(self.pose.rotation.pitch, pitch);
        self.pose.rotation.yaw = chase_formation_angle(self.pose.rotation.yaw, yaw);
    }

    /// Run one discrete game update. Loop boundaries can execute the last
    /// action of one maneuver and the first action of the next before flight.
    pub fn tick(
        &mut self,
        cue: OpeningCameraCue,
        actors: &ObjectStore,
        auxiliary: &mut IntroAuxiliaryEffect,
    ) -> OpeningFormationEvents {
        use OpeningFormationPhase::*;
        if matches!(self.phase, Finished | AwaitingDestruction) {
            return OpeningFormationEvents::default();
        }
        let mut events = OpeningFormationEvents::default();
        loop {
            match self.phase {
                Initializing => {
                    self.place(OpeningFormationShot::Arrival);
                    self.pose.rotation.roll = ARRIVAL_ROLL;
                    self.speed = ARRIVAL_SPEEDS[member_index(self.member)];
                    self.phase = InitialWait {
                        updates_left: INITIAL_WAIT,
                    };
                }
                InitialWait { updates_left } if updates_left > 0 => {
                    self.phase = InitialWait {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                InitialWait { .. } => {
                    self.trail_enabled = self.member == OpeningFormationMember::First;
                    self.phase = InitialHold {
                        updates_left: INITIAL_HOLD,
                    };
                }
                InitialHold { updates_left } if updates_left > 0 => {
                    self.phase = InitialHold {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                InitialHold { .. } => {
                    self.load_impulse();
                    self.phase = ArrivalBank {
                        updates_left: INITIAL_BANK_UPDATES,
                    };
                }
                ArrivalBank { updates_left } if updates_left > 0 => {
                    advance_formation_impulse(&mut self.pose, &mut self.impulse);
                    self.phase = ArrivalBank {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                ArrivalBank { .. } => self.phase = AwaitingFirstCut,
                AwaitingFirstCut => {
                    if cue != OpeningCameraCue::FirstCut {
                        break;
                    }
                    self.phase = FirstCutPause;
                    break;
                }
                FirstCutPause => {
                    self.speed = SECOND_SPEED;
                    self.place(OpeningFormationShot::Pursuit);
                    self.load_impulse();
                    self.phase = PursuitBank {
                        updates_left: self.duration(),
                    };
                }
                PursuitBank { updates_left } if updates_left > 0 => {
                    advance_formation_impulse(&mut self.pose, &mut self.impulse);
                    self.phase = PursuitBank {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                PursuitBank { .. } => {
                    self.select_target(actors);
                    self.phase = TrackingBank {
                        updates_left: TRACKING_BANK_UPDATES,
                    };
                }
                TrackingBank { updates_left } if updates_left > 0 => {
                    self.pose.rotation.roll =
                        chase_formation_angle(self.pose.rotation.roll, TRACKING_ROLL);
                    self.pose.position.x = self.pose.position.x.wrapping_add(TRACKING_DRIFT);
                    self.phase = TrackingBank {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                TrackingBank { .. } => {
                    events.pursuit_audio = Some(OpeningFormationAudio {
                        source: self.pose.position,
                    });
                    self.phase = Tracking {
                        updates_left: self.duration(),
                    };
                }
                Tracking { updates_left } if updates_left > 0 => {
                    self.aim(actors);
                    self.speed = TRACKING_SPEED;
                    self.phase = Tracking {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                Tracking { .. } => {
                    self.phase = Climbing {
                        updates_left: CLIMB_UPDATES,
                    }
                }
                Climbing { updates_left } if updates_left > 0 => {
                    self.pose.rotation.pitch = self.pose.rotation.pitch.wrapping_add(-1);
                    self.phase = Climbing {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                Climbing { .. } => self.phase = AwaitingSecondCut,
                AwaitingSecondCut => {
                    if cue != OpeningCameraCue::SecondCut {
                        break;
                    }
                    self.hidden = true;
                    self.trail_enabled = false;
                    self.phase = AwaitingThirdCut;
                }
                AwaitingThirdCut => {
                    if cue != OpeningCameraCue::ThirdCut {
                        break;
                    }
                    self.trail_enabled = self.member == OpeningFormationMember::First;
                    self.hidden = false;
                    self.place(OpeningFormationShot::Reappearance);
                    self.speed = REAPPEAR_SPEED;
                    self.pose.rotation.roll = Angle::ZERO;
                    self.phase = Reappeared {
                        updates_left: REAPPEAR_WAIT,
                    };
                }
                Reappeared { updates_left } if updates_left > 0 => {
                    self.phase = Reappeared {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                Reappeared { .. } => {
                    if self.duration() == 0 {
                        auxiliary.configure_departure(self.actor, self.pose, 0);
                        events.request_destruction = true;
                    }
                    self.load_impulse();
                    self.phase = DepartureBank {
                        updates_left: REAPPEAR_BANK_UPDATES,
                    };
                }
                DepartureBank { updates_left } if updates_left > 0 => {
                    advance_formation_impulse(&mut self.pose, &mut self.impulse);
                    self.phase = DepartureBank {
                        updates_left: updates_left - 1,
                    };
                    if updates_left > 1 {
                        break;
                    }
                }
                DepartureBank { .. } => self.phase = AwaitingFourthCut,
                AwaitingFourthCut => {
                    if cue != OpeningCameraCue::FourthCut {
                        break;
                    }
                    self.place(OpeningFormationShot::Exit);
                    self.speed = EXIT_SPEED;
                    self.phase = Exiting {
                        updates_left: self.duration() - 1,
                    };
                    break;
                }
                Exiting { updates_left } if updates_left > 1 => {
                    self.phase = Exiting {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                Exiting { .. } => {
                    self.phase = Finished;
                    events.finished = true;
                    return events;
                }
                Finished | AwaitingDestruction => unreachable!(),
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
        self.elapsed_updates = self.elapsed_updates.wrapping_add(1);
        if events.request_destruction {
            self.phase = AwaitingDestruction;
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind};

    fn craft(member: OpeningFormationMember) -> (OpeningFormationCraft, ObjectStore) {
        let mut actors = ObjectStore::new();
        let id = actors
            .allocate(Object::new(
                ObjectKind::Effect,
                CRAFT_SHAPE,
                Behavior::Effect,
            ))
            .unwrap();
        (
            OpeningFormationCraft::new(id, member, IntroScenePose::default()),
            actors,
        )
    }

    #[test]
    fn terminal_loop_action_and_following_maneuver_share_one_flight_update() {
        let (mut craft, actors) = craft(OpeningFormationMember::First);
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for update in 0..=106 {
            craft.tick(
                if update < 86 {
                    OpeningCameraCue::Opening
                } else {
                    OpeningCameraCue::FirstCut
                },
                &actors,
                &mut auxiliary,
            );
        }
        assert_eq!(
            craft.phase(),
            OpeningFormationPhase::TrackingBank {
                updates_left: TRACKING_BANK_UPDATES - 1
            }
        );
        assert_eq!(
            craft.pose.position,
            Vector3 {
                x: -120,
                y: 14,
                z: 1697
            }
        );
    }

    #[test]
    fn a_missing_first_cut_does_not_skip_forward_and_elapsed_wraps() {
        let (mut craft, actors) = craft(OpeningFormationMember::First);
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for _ in 0..=u16::MAX {
            assert_eq!(
                craft.tick(OpeningCameraCue::ThirdCut, &actors, &mut auxiliary),
                OpeningFormationEvents::default()
            );
        }
        assert_eq!(craft.elapsed_updates, 0);
        assert_eq!(craft.phase(), OpeningFormationPhase::AwaitingFirstCut);
        assert!(craft.is_visible());
    }

    #[test]
    fn exit_has_eighteen_motion_updates_then_end_is_inert() {
        let (mut craft, actors) = craft(OpeningFormationMember::First);
        craft.phase = OpeningFormationPhase::AwaitingFourthCut;
        let mut auxiliary = IntroAuxiliaryEffect::default();
        let duration =
            opening_formation_placement(OpeningFormationMember::First, OpeningFormationShot::Exit)
                .duration;
        for _ in 0..duration - 1 {
            assert!(
                !craft
                    .tick(OpeningCameraCue::FourthCut, &actors, &mut auxiliary)
                    .finished
            );
        }
        let before = craft.pose;
        let elapsed = craft.elapsed_updates;
        assert!(
            craft
                .tick(OpeningCameraCue::FourthCut, &actors, &mut auxiliary)
                .finished
        );
        assert_eq!(craft.pose, before);
        assert_eq!(craft.elapsed_updates, elapsed);
        let retired = craft;
        assert_eq!(
            craft.tick(OpeningCameraCue::Opening, &actors, &mut auxiliary),
            OpeningFormationEvents::default()
        );
        assert_eq!(craft, retired);
    }

    #[test]
    fn third_member_auxiliary_uses_pre_motion_origin_and_handoff_is_inert() {
        let (mut craft, actors) = craft(OpeningFormationMember::Third);
        craft.phase = OpeningFormationPhase::AwaitingThirdCut;
        let mut auxiliary = IntroAuxiliaryEffect::default();
        for _ in 0..REAPPEAR_WAIT {
            craft.tick(OpeningCameraCue::ThirdCut, &actors, &mut auxiliary);
        }
        let before = craft.pose.position;
        assert!(
            craft
                .tick(OpeningCameraCue::ThirdCut, &actors, &mut auxiliary)
                .request_destruction
        );
        assert_eq!(auxiliary.origin, before);
        assert_ne!(craft.pose.position, before);
        assert!(craft.is_visible());
        let handed_off = craft;
        assert_eq!(
            craft.tick(OpeningCameraCue::FourthCut, &actors, &mut auxiliary),
            OpeningFormationEvents::default()
        );
        assert_eq!(craft, handed_off);
    }
}
