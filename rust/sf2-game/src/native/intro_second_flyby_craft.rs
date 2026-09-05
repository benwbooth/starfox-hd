//! Main later-flyby choreography, translated from the authored opening path.
//! Child actors consume explicit ordered events; they do not execute inside
//! this craft's update or share its motion state.

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_formation::chase_formation_angle;
use super::intro_motion::{IntroAttachment, IntroScenePose};
use super::intro_second_flyby::{
    OpeningSecondFlybyPlacement, SECOND_FLYBY_FLARE_ATTACHMENT, SECOND_FLYBY_TRAIL_ATTACHMENT,
};
use super::object::{Angle, ShapeId, Vector3};
use super::render::Rotation;

const CRAFT_SHAPE: ShapeId = ShapeId::from_catalog_index(338);
const ROCK_UPDATES: u8 = 8;
const ROCK_YAW_STEP: i8 = 6;
const FLIGHT_PITCH: Angle = Angle::from_units(20);
const FLIGHT_SPEED: u8 = 30;
const FULL_ANIMATION_LENGTH: u8 = 16;
const TURN_ANIMATION_LENGTH: u8 = 15;
const FIRST_ANIMATION_FRAME: u8 = 15;
const TURN_YAW: Angle = Angle::from_units(203);
const LEVEL_ROLL: Angle = Angle::from_units(226);
const LEVEL_PITCH: Angle = Angle::from_units(10);
const AIM_YAW: Angle = Angle::from_units(228);
const AIM_PITCH: Angle = Angle::from_units(30);
const AIM_ROLL: Angle = Angle::from_units(20);
const BOOST_SPEED: u8 = 70;
const BOOST_DECELERATION: u8 = 7;
const WING_WAIT: u8 = 2;
const WING_ANIMATION_FRAME: u8 = 4;
const BANK_YAW: Angle = Angle::from_units(140);
const EXIT_YAW: Angle = Angle::from_units(20);
const EXIT_TURN_YAW: Angle = Angle::from_units(10);
const EXIT_ANIMATION_FRAME: u8 = 8;
const FINAL_PITCH: Angle = Angle::from_units(236);
const FINAL_SPEED: u8 = 40;
const FINAL_DECELERATION: u8 = 2;
const FINAL_TURN_PITCH: Angle = Angle::from_units(40);
const HOLD_YAW: Angle = Angle::from_units(192);
const FINAL_YAW_STEPS: [i8; 10] = [4, -4, 3, -3, 2, -2, 1, -1, 1, -1];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyChild {
    LinkedChain,
    EngineFlare,
    Trail,
    CameraTarget,
    AttachedWing,
    DepartingWing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybySpawnPlacement {
    Independent(IntroScenePose),
    Attached(IntroAttachment),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyAttachmentGroup {
    Chain,
    Effects,
    Wing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondFlybySpawn {
    pub child: OpeningSecondFlybyChild,
    pub placement: OpeningSecondFlybySpawnPlacement,
}

impl OpeningSecondFlybySpawn {
    pub const fn attachment_group(self) -> Option<OpeningSecondFlybyAttachmentGroup> {
        use OpeningSecondFlybyAttachmentGroup::*;
        match self.child {
            OpeningSecondFlybyChild::LinkedChain => Some(Chain),
            OpeningSecondFlybyChild::EngineFlare | OpeningSecondFlybyChild::Trail => Some(Effects),
            OpeningSecondFlybyChild::AttachedWing => Some(Wing),
            OpeningSecondFlybyChild::CameraTarget | OpeningSecondFlybyChild::DepartingWing => None,
        }
    }

    /// The inline post-spawn assignment creates a second parent identity,
    /// independent of the ordinary transform/attachment relationship.
    pub const fn has_secondary_parent_link(self) -> bool {
        matches!(self.child, OpeningSecondFlybyChild::LinkedChain)
    }

    pub const fn shape(self) -> ShapeId {
        ShapeId::from_catalog_index(match self.child {
            OpeningSecondFlybyChild::LinkedChain => 340,
            OpeningSecondFlybyChild::EngineFlare => 48,
            OpeningSecondFlybyChild::Trail => 119,
            OpeningSecondFlybyChild::CameraTarget => 0,
            OpeningSecondFlybyChild::AttachedWing | OpeningSecondFlybyChild::DepartingWing => 89,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybySound {
    FlightBeat,
    TrailLeadIn,
    TrailAccent,
    WingDeparture,
    ExitBank,
    FinalBank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyEvent {
    /// Reset all child controls, then enable the children's sort override.
    InitializeChildControls,
    EnableChildPitchSettling,
    Spawn(OpeningSecondFlybySpawn),
    SelectAsCameraTarget,
    /// FlightBeat uses the selected-listener class-two spatial service;
    /// all other markers use the selected-listener direct queue service.
    Sound {
        sound: OpeningSecondFlybySound,
        source: IntroScenePose,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyManeuver {
    PitchUp,
    FirstPitchDown,
    SecondPitchDown,
    LastPitchDown,
    TurnTowardTrail,
    LevelForDeparture,
    AimDeparture,
    WingSeparationPause,
    BankAfterSeparation,
    ExitRoll,
    ExitTurn,
    SettleYaw,
    FinalApproach,
    FinalYawOscillation,
    FinalTurn,
}

impl OpeningSecondFlybyManeuver {
    const fn updates(self) -> u8 {
        use OpeningSecondFlybyManeuver::*;
        match self {
            PitchUp | FirstPitchDown | SecondPitchDown | TurnTowardTrail | LevelForDeparture
            | FinalApproach => 8,
            LastPitchDown | ExitRoll => 4,
            AimDeparture | FinalYawOscillation => 10,
            WingSeparationPause => 5,
            BankAfterSeparation => 15,
            ExitTurn => 6,
            SettleYaw => 18,
            FinalTurn => 9,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSecondFlybyPhase {
    Initializing,
    RockingOut {
        updates_left: u8,
    },
    RockingBack {
        updates_left: u8,
    },
    Maneuver {
        kind: OpeningSecondFlybyManeuver,
        updates_left: u8,
    },
    AwaitingThirdCut,
    AwaitingFourthCut,
    BeforeWingSpawn {
        updates_left: u8,
    },
    AfterWingSpawn {
        updates_left: u8,
    },
    WingDeparturePause,
    AwaitingFinalCut,
    Holding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSecondFlybyCraft {
    pub pose: IntroScenePose,
    pub velocity: Vector3,
    pub speed: u8,
    pub deceleration: u8,
    pub animation_frame: u8,
    pub animation_enabled: bool,
    phase: OpeningSecondFlybyPhase,
}

impl Default for OpeningSecondFlybyCraft {
    fn default() -> Self {
        Self::new()
    }
}

impl OpeningSecondFlybyCraft {
    pub fn new() -> Self {
        Self {
            pose: IntroScenePose::default(),
            velocity: Vector3::default(),
            speed: 0,
            deceleration: 0,
            animation_frame: 0,
            animation_enabled: false,
            phase: OpeningSecondFlybyPhase::Initializing,
        }
    }
    pub const fn shape(&self) -> ShapeId {
        CRAFT_SHAPE
    }
    pub fn phase(&self) -> OpeningSecondFlybyPhase {
        self.phase
    }
    fn maneuver(&mut self, kind: OpeningSecondFlybyManeuver) {
        self.phase = OpeningSecondFlybyPhase::Maneuver {
            kind,
            updates_left: kind.updates(),
        };
    }
    fn frame(&mut self, frame: u8) {
        self.animation_frame = frame;
        self.animation_enabled = true;
    }
    fn animate(&mut self, length: u8) {
        self.frame((self.animation_frame + 1) % length);
    }
    fn chase_pitch(&mut self, target: Angle) {
        self.pose.rotation.pitch = chase_formation_angle(self.pose.rotation.pitch, target);
    }
    fn chase_yaw(&mut self, target: Angle) {
        self.pose.rotation.yaw = chase_formation_angle(self.pose.rotation.yaw, target);
    }
    fn chase_roll(&mut self, target: Angle) {
        self.pose.rotation.roll = chase_formation_angle(self.pose.rotation.roll, target);
    }
    fn sound(&self, events: &mut Vec<OpeningSecondFlybyEvent>, sound: OpeningSecondFlybySound) {
        events.push(OpeningSecondFlybyEvent::Sound {
            sound,
            source: self.pose,
        });
    }
    fn spawn(&self, events: &mut Vec<OpeningSecondFlybyEvent>, child: OpeningSecondFlybyChild) {
        use OpeningSecondFlybyChild::*;
        use OpeningSecondFlybySpawnPlacement::*;
        let placement = match child {
            LinkedChain => Attached(IntroAttachment {
                offset: Vector3 { x: 0, y: 0, z: -11 },
                ..Default::default()
            }),
            EngineFlare => Attached(SECOND_FLYBY_FLARE_ATTACHMENT),
            Trail => Attached(SECOND_FLYBY_TRAIL_ATTACHMENT),
            CameraTarget | DepartingWing => Independent(self.pose),
            AttachedWing => Attached(IntroAttachment {
                offset: Vector3 { x: 0, y: 20, z: 50 },
                rotation: Rotation {
                    pitch: Angle::ZERO,
                    yaw: Angle::from_units(48),
                    roll: Angle::from_units(236),
                },
            }),
        };
        events.push(OpeningSecondFlybyEvent::Spawn(OpeningSecondFlybySpawn {
            child,
            placement,
        }));
    }

    fn act(&mut self, kind: OpeningSecondFlybyManeuver, updates_left: u8) {
        use OpeningSecondFlybyManeuver::*;
        match kind {
            PitchUp => {
                self.animate(FULL_ANIMATION_LENGTH);
                self.pose.rotation.pitch = self.pose.rotation.pitch.wrapping_add(1);
            }
            FirstPitchDown | SecondPitchDown | LastPitchDown => {
                self.animate(FULL_ANIMATION_LENGTH);
                self.pose.rotation.pitch = self.pose.rotation.pitch.wrapping_add(-1);
            }
            TurnTowardTrail => {
                self.animate(TURN_ANIMATION_LENGTH);
                self.chase_yaw(TURN_YAW);
            }
            LevelForDeparture => {
                self.chase_roll(LEVEL_ROLL);
                self.chase_pitch(LEVEL_PITCH);
            }
            AimDeparture => {
                self.chase_yaw(AIM_YAW);
                self.chase_yaw(AIM_YAW);
                self.chase_pitch(AIM_PITCH);
                self.chase_roll(AIM_ROLL);
            }
            // The authored counted loop has an empty body; its boundary
            // still advances to the departing-wing spawn in this update.
            WingSeparationPause => {}
            BankAfterSeparation => {
                self.chase_pitch(LEVEL_PITCH);
                self.chase_roll(AIM_ROLL);
                self.chase_yaw(BANK_YAW);
            }
            ExitRoll => {
                self.animate(FULL_ANIMATION_LENGTH);
                self.chase_roll(Angle::ZERO);
                self.chase_yaw(EXIT_YAW);
            }
            ExitTurn => {
                self.frame(EXIT_ANIMATION_FRAME);
                self.chase_roll(Angle::ZERO);
                self.chase_yaw(EXIT_TURN_YAW);
                self.chase_yaw(EXIT_TURN_YAW);
            }
            SettleYaw => self.chase_yaw(Angle::ZERO),
            FinalApproach => {
                self.animate(FULL_ANIMATION_LENGTH);
                self.chase_pitch(FINAL_PITCH);
                self.chase_roll(AIM_ROLL);
            }
            FinalYawOscillation => {
                self.pose.rotation.yaw = self
                    .pose
                    .rotation
                    .yaw
                    .wrapping_add(FINAL_YAW_STEPS[usize::from(kind.updates() - updates_left)]);
            }
            FinalTurn => {
                self.chase_yaw(Angle::HALF_TURN);
                self.chase_pitch(FINAL_TURN_PITCH);
            }
        }
    }

    fn after_maneuver(
        &mut self,
        kind: OpeningSecondFlybyManeuver,
        events: &mut Vec<OpeningSecondFlybyEvent>,
    ) {
        use OpeningSecondFlybyManeuver::*;
        use OpeningSecondFlybySound::*;
        match kind {
            PitchUp => {
                self.sound(events, FlightBeat);
                self.maneuver(FirstPitchDown);
            }
            FirstPitchDown => self.maneuver(SecondPitchDown),
            SecondPitchDown => {
                self.sound(events, FlightBeat);
                self.maneuver(LastPitchDown);
            }
            LastPitchDown => {
                self.frame(0);
                self.maneuver(TurnTowardTrail);
            }
            TurnTowardTrail => {
                self.sound(events, TrailLeadIn);
                self.sound(events, TrailAccent);
                self.spawn(events, OpeningSecondFlybyChild::Trail);
                self.phase = OpeningSecondFlybyPhase::AwaitingThirdCut;
            }
            LevelForDeparture => self.maneuver(AimDeparture),
            AimDeparture => {
                self.speed = BOOST_SPEED;
                self.deceleration = BOOST_DECELERATION;
                self.phase = OpeningSecondFlybyPhase::BeforeWingSpawn {
                    updates_left: WING_WAIT,
                };
            }
            WingSeparationPause => {
                self.spawn(events, OpeningSecondFlybyChild::DepartingWing);
                self.maneuver(BankAfterSeparation);
            }
            BankAfterSeparation => self.maneuver(ExitRoll),
            ExitRoll => {
                self.sound(events, ExitBank);
                self.maneuver(ExitTurn);
            }
            ExitTurn => self.maneuver(SettleYaw),
            SettleYaw => {
                self.pose = OpeningSecondFlybyPlacement::FinalCut.pose();
                self.phase = OpeningSecondFlybyPhase::AwaitingFinalCut;
            }
            FinalApproach => {
                self.sound(events, FinalBank);
                self.maneuver(FinalYawOscillation);
            }
            FinalYawOscillation => {
                self.speed = FINAL_SPEED;
                self.deceleration = FINAL_DECELERATION;
                self.maneuver(FinalTurn);
            }
            FinalTurn => {
                self.pose.position.z = 0;
                self.pose.rotation.yaw = HOLD_YAW;
                self.speed = 0;
                self.deceleration = 0;
                self.phase = OpeningSecondFlybyPhase::Holding;
            }
        }
    }

    pub fn tick(&mut self, cue: OpeningCameraCue) -> Vec<OpeningSecondFlybyEvent> {
        use OpeningSecondFlybyPhase::*;
        let mut events = Vec::new();
        loop {
            match self.phase {
                Initializing => {
                    events.push(OpeningSecondFlybyEvent::InitializeChildControls);
                    self.pose = OpeningSecondFlybyPlacement::Arrival.pose();
                    self.spawn(&mut events, OpeningSecondFlybyChild::LinkedChain);
                    self.phase = RockingOut {
                        updates_left: ROCK_UPDATES,
                    };
                }
                RockingOut { updates_left } | RockingBack { updates_left } => {
                    if cue == OpeningCameraCue::SecondCut {
                        self.pose.rotation.pitch = FLIGHT_PITCH;
                        self.pose.rotation.yaw = Angle::HALF_TURN;
                        events.push(OpeningSecondFlybyEvent::SelectAsCameraTarget);
                        self.speed = FLIGHT_SPEED;
                        self.spawn(&mut events, OpeningSecondFlybyChild::EngineFlare);
                        self.frame(FIRST_ANIMATION_FRAME);
                        self.maneuver(OpeningSecondFlybyManeuver::PitchUp);
                        continue;
                    }
                    let outward = matches!(self.phase, RockingOut { .. });
                    self.pose.rotation.pitch =
                        self.pose
                            .rotation
                            .pitch
                            .wrapping_add(if outward { -1 } else { 1 });
                    self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(if outward {
                        ROCK_YAW_STEP
                    } else {
                        -ROCK_YAW_STEP
                    });
                    if updates_left > 1 {
                        self.phase = if outward {
                            RockingOut {
                                updates_left: updates_left - 1,
                            }
                        } else {
                            RockingBack {
                                updates_left: updates_left - 1,
                            }
                        };
                        break;
                    }
                    self.phase = if outward {
                        RockingBack {
                            updates_left: ROCK_UPDATES,
                        }
                    } else {
                        RockingOut {
                            updates_left: ROCK_UPDATES,
                        }
                    };
                    if !outward {
                        break;
                    }
                }
                Maneuver { kind, updates_left } => {
                    self.act(kind, updates_left);
                    if updates_left > 1 {
                        self.phase = Maneuver {
                            kind,
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    self.after_maneuver(kind, &mut events);
                }
                AwaitingThirdCut => {
                    if cue != OpeningCameraCue::ThirdCut {
                        self.pose.rotation.yaw = self.pose.rotation.yaw.wrapping_add(1);
                        break;
                    }
                    self.speed = 0;
                    self.pose = OpeningSecondFlybyPlacement::MiddleCut.pose();
                    self.phase = AwaitingFourthCut;
                }
                AwaitingFourthCut => {
                    if cue != OpeningCameraCue::FourthCut {
                        break;
                    }
                    events.push(OpeningSecondFlybyEvent::EnableChildPitchSettling);
                    self.spawn(&mut events, OpeningSecondFlybyChild::CameraTarget);
                    self.maneuver(OpeningSecondFlybyManeuver::LevelForDeparture);
                }
                BeforeWingSpawn { updates_left } if updates_left > 0 => {
                    self.phase = BeforeWingSpawn {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                BeforeWingSpawn { .. } => {
                    self.spawn(&mut events, OpeningSecondFlybyChild::AttachedWing);
                    self.phase = AfterWingSpawn {
                        updates_left: WING_WAIT,
                    };
                }
                AfterWingSpawn { updates_left } if updates_left > 0 => {
                    self.phase = AfterWingSpawn {
                        updates_left: updates_left - 1,
                    };
                    break;
                }
                AfterWingSpawn { .. } => {
                    self.sound(&mut events, OpeningSecondFlybySound::WingDeparture);
                    self.phase = WingDeparturePause;
                    break;
                }
                WingDeparturePause => {
                    self.frame(WING_ANIMATION_FRAME);
                    self.maneuver(OpeningSecondFlybyManeuver::WingSeparationPause);
                }
                AwaitingFinalCut => {
                    if cue != OpeningCameraCue::FinalCut {
                        break;
                    }
                    events.push(OpeningSecondFlybyEvent::SelectAsCameraTarget);
                    self.frame(0);
                    self.maneuver(OpeningSecondFlybyManeuver::FinalApproach);
                }
                Holding => break,
            }
        }
        if self.deceleration != 0 {
            let step = self.deceleration;
            if self.speed < step {
                self.deceleration = 0;
            }
            self.speed = self.speed.saturating_sub(step);
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
    fn immediate_cut_keeps_initial_chain_and_flare_events_in_source_order() {
        let mut craft = OpeningSecondFlybyCraft::new();
        let events = craft.tick(OpeningCameraCue::SecondCut);
        assert!(matches!(
            events.as_slice(),
            [
                OpeningSecondFlybyEvent::InitializeChildControls,
                OpeningSecondFlybyEvent::Spawn(OpeningSecondFlybySpawn {
                    child: OpeningSecondFlybyChild::LinkedChain,
                    ..
                }),
                OpeningSecondFlybyEvent::SelectAsCameraTarget,
                OpeningSecondFlybyEvent::Spawn(OpeningSecondFlybySpawn {
                    child: OpeningSecondFlybyChild::EngineFlare,
                    ..
                }),
            ]
        ));
        assert_eq!(craft.animation_frame, 0);
        assert_eq!(craft.pose.rotation.pitch, FLIGHT_PITCH.wrapping_add(1));
        assert_eq!(craft.speed, FLIGHT_SPEED);
    }

    #[test]
    fn missing_second_cut_never_spawns_later_children() {
        let mut craft = OpeningSecondFlybyCraft::new();
        craft.tick(OpeningCameraCue::FinalCut);
        for _ in 0..1024 {
            assert!(craft.tick(OpeningCameraCue::FinalCut).is_empty());
        }
        assert!(matches!(
            craft.phase(),
            OpeningSecondFlybyPhase::RockingOut { .. }
                | OpeningSecondFlybyPhase::RockingBack { .. }
        ));
        assert_eq!(
            craft.pose.position,
            OpeningSecondFlybyPlacement::Arrival.pose().position
        );
    }

    #[test]
    fn complete_path_emits_once_then_holds_without_retirement_or_restart() {
        let mut craft = OpeningSecondFlybyCraft::new();
        let mut spawn_count = 0;
        let mut sound_count = 0;
        let mut target_count = 0;
        for update in 0..600 {
            let cue = match update {
                0..100 => OpeningCameraCue::SecondCut,
                100..150 => OpeningCameraCue::ThirdCut,
                150..300 => OpeningCameraCue::FourthCut,
                _ => OpeningCameraCue::FinalCut,
            };
            for event in craft.tick(cue) {
                match event {
                    OpeningSecondFlybyEvent::Spawn(_) => spawn_count += 1,
                    OpeningSecondFlybyEvent::Sound { .. } => sound_count += 1,
                    OpeningSecondFlybyEvent::SelectAsCameraTarget => target_count += 1,
                    _ => {}
                }
            }
        }
        assert_eq!((spawn_count, sound_count, target_count), (6, 7, 2));
        assert_eq!(craft.phase(), OpeningSecondFlybyPhase::Holding);
        assert_eq!(craft.pose.position.z, 0);
        assert_eq!(craft.pose.rotation.yaw, HOLD_YAW);
        let held = craft;
        assert!(craft.tick(OpeningCameraCue::Opening).is_empty());
        assert_eq!(craft, held);
    }
}
