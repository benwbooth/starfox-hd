//! Source-authored opening root choreography. Spawn requests describe native
//! actors and attachments, not path bytecode or recorded poses.

use super::game::flight_velocity;
use super::intro_camera::OpeningCameraCue;
use super::intro_motion::{IntroAttachment, IntroPlayerAnchor, IntroScenePose};
use super::object::{Angle, ShapeId, Vector3};
use super::render::Rotation;

const INITIAL_WAIT_UPDATES: u8 = 96;
const ROOT_FLIGHT_SPEED: u8 = 10;
const BACKGROUND_HORIZONTAL_ORIGIN: i16 = 176;
const BACKGROUND_VERTICAL_ORIGIN: i16 = 400;
const CAMERA_TARGET_OFFSET: Vector3 = Vector3 { x: 0, y: 0, z: 300 };
const FLYBY_RIG_OFFSET: Vector3 = Vector3 {
    x: 0,
    y: -800,
    z: 1_000,
};
const FLYBY_RIG_ROTATION: Rotation = Rotation {
    pitch: Angle::from_units(50),
    yaw: Angle::from_units(128),
    roll: Angle::ZERO,
};
const ATTACHED_CRAFT_OFFSET: Vector3 = Vector3 {
    x: -250,
    y: -400,
    z: -1_500,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFormationMember {
    First,
    Second,
    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningRootActor {
    CameraTarget,
    NintendoLogo,
    Camera,
    FlybyRig,
    AttachedCraft,
    FreeCraft,
    FormationCraft(OpeningFormationMember),
    SecondFlybyCraft,
    SecondCameraTarget,
}

impl OpeningRootActor {
    pub const fn shape(self) -> ShapeId {
        const INVISIBLE: ShapeId = ShapeId::from_catalog_index(0);
        const FIRST_CRAFT: ShapeId = ShapeId::from_catalog_index(64);
        const FORMATION_CRAFT: ShapeId = ShapeId::from_catalog_index(89);
        const SECOND_CRAFT: ShapeId = ShapeId::from_catalog_index(338);
        match self {
            Self::AttachedCraft | Self::FreeCraft => FIRST_CRAFT,
            Self::FormationCraft(_) => FORMATION_CRAFT,
            Self::SecondFlybyCraft => SECOND_CRAFT,
            _ => INVISIBLE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningAttachmentGroup {
    TrackingAndCraft,
    FlybyRig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningSpawnPlacement {
    /// Independent actors copy the root's pose before its common movement.
    Independent(IntroScenePose),
    /// Attachments publish through the root after its common movement.
    Attached {
        local: IntroAttachment,
        group: OpeningAttachmentGroup,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningRootSpawn {
    pub actor: OpeningRootActor,
    pub placement: OpeningSpawnPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningBackgroundOrigin {
    pub horizontal: i16,
    pub vertical: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningRootEvent {
    Initialize {
        background_origin: OpeningBackgroundOrigin,
        player_anchor: IntroPlayerAnchor,
        depth_velocity: i16,
        cue: OpeningCameraCue,
    },
    Spawn(OpeningRootSpawn),
    /// The source queues the opening flyby audio marker before its actors.
    QueueFlybyAudio,
    /// Only the first attached actor in this group is marked for removal.
    RemoveFirstAttachment(OpeningAttachmentGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningRootPhase {
    Initializing,
    WaitingForFlyby { updates_left: u8 },
    AwaitingFirstCut,
    FirstCutPause,
    AwaitingThirdCut,
    AwaitingFourthCut,
    Holding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningSceneRoot {
    pub pose: IntroScenePose,
    velocity: Vector3,
    phase: OpeningRootPhase,
}

impl Default for OpeningSceneRoot {
    fn default() -> Self {
        Self {
            pose: IntroScenePose::default(),
            // The authored zero heading still traverses both fixed-point
            // cosine products. Speed is not interchangeable with velocity.
            velocity: flight_velocity(Angle::ZERO, Angle::ZERO, ROOT_FLIGHT_SPEED, 1),
            phase: OpeningRootPhase::Initializing,
        }
    }
}

impl OpeningSceneRoot {
    pub fn phase(&self) -> OpeningRootPhase {
        self.phase
    }

    pub const fn depth_velocity(&self) -> i16 {
        self.velocity.z
    }

    fn independent(&self, actor: OpeningRootActor) -> OpeningRootEvent {
        OpeningRootEvent::Spawn(OpeningRootSpawn {
            actor,
            placement: OpeningSpawnPlacement::Independent(self.pose),
        })
    }

    fn attached(
        actor: OpeningRootActor,
        offset: Vector3,
        rotation: Rotation,
        group: OpeningAttachmentGroup,
    ) -> OpeningRootEvent {
        OpeningRootEvent::Spawn(OpeningRootSpawn {
            actor,
            placement: OpeningSpawnPlacement::Attached {
                local: IntroAttachment { offset, rotation },
                group,
            },
        })
    }

    /// Execute once per original root-actor update. Preserve the event order
    /// when allocating actors and delivering audio. Child behavior and the
    /// scene-wide actor scheduler consume these requests separately.
    pub fn tick(&mut self, cue: OpeningCameraCue) -> Vec<OpeningRootEvent> {
        let mut events = Vec::new();
        loop {
            match self.phase {
                OpeningRootPhase::Initializing => {
                    self.pose = IntroScenePose::default();
                    let mut player_anchor = IntroPlayerAnchor::default();
                    player_anchor.capture_position(self.pose.position);
                    player_anchor.capture_rotation(self.pose.rotation);
                    events.push(OpeningRootEvent::Initialize {
                        background_origin: OpeningBackgroundOrigin {
                            horizontal: BACKGROUND_HORIZONTAL_ORIGIN,
                            vertical: BACKGROUND_VERTICAL_ORIGIN,
                        },
                        player_anchor,
                        depth_velocity: self.depth_velocity(),
                        cue: OpeningCameraCue::Opening,
                    });
                    events.push(Self::attached(
                        OpeningRootActor::CameraTarget,
                        CAMERA_TARGET_OFFSET,
                        Rotation::default(),
                        OpeningAttachmentGroup::TrackingAndCraft,
                    ));
                    events.push(self.independent(OpeningRootActor::NintendoLogo));
                    events.push(self.independent(OpeningRootActor::Camera));
                    events.push(Self::attached(
                        OpeningRootActor::FlybyRig,
                        FLYBY_RIG_OFFSET,
                        FLYBY_RIG_ROTATION,
                        OpeningAttachmentGroup::FlybyRig,
                    ));
                    self.phase = OpeningRootPhase::WaitingForFlyby {
                        updates_left: INITIAL_WAIT_UPDATES,
                    };
                }
                OpeningRootPhase::WaitingForFlyby { updates_left } => {
                    if updates_left != 0 {
                        self.phase = OpeningRootPhase::WaitingForFlyby {
                            updates_left: updates_left - 1,
                        };
                        break;
                    }
                    events.push(OpeningRootEvent::QueueFlybyAudio);
                    events.push(Self::attached(
                        OpeningRootActor::AttachedCraft,
                        ATTACHED_CRAFT_OFFSET,
                        Rotation::default(),
                        OpeningAttachmentGroup::TrackingAndCraft,
                    ));
                    events.push(self.independent(OpeningRootActor::FreeCraft));
                    for member in [
                        OpeningFormationMember::First,
                        OpeningFormationMember::Second,
                        OpeningFormationMember::Third,
                    ] {
                        events.push(self.independent(OpeningRootActor::FormationCraft(member)));
                    }
                    self.phase = OpeningRootPhase::AwaitingFirstCut;
                }
                OpeningRootPhase::AwaitingFirstCut => {
                    if cue != OpeningCameraCue::FirstCut {
                        break;
                    }
                    self.phase = OpeningRootPhase::FirstCutPause;
                    break;
                }
                OpeningRootPhase::FirstCutPause => {
                    events.push(OpeningRootEvent::RemoveFirstAttachment(
                        OpeningAttachmentGroup::FlybyRig,
                    ));
                    events.push(self.independent(OpeningRootActor::SecondFlybyCraft));
                    self.phase = OpeningRootPhase::AwaitingThirdCut;
                }
                OpeningRootPhase::AwaitingThirdCut => {
                    if cue != OpeningCameraCue::ThirdCut {
                        break;
                    }
                    events.push(self.independent(OpeningRootActor::SecondCameraTarget));
                    self.phase = OpeningRootPhase::AwaitingFourthCut;
                }
                OpeningRootPhase::AwaitingFourthCut => {
                    if cue != OpeningCameraCue::FourthCut {
                        break;
                    }
                    self.phase = OpeningRootPhase::Holding;
                }
                OpeningRootPhase::Holding => break,
            }
        }
        // Holding is persistent, not a stopped actor. The root keeps moving.
        self.pose.position.x = self.pose.position.x.wrapping_add(self.velocity.x);
        self.pose.position.y = self.pose.position.y.wrapping_add(self.velocity.y);
        self.pose.position.z = self.pose.position.z.wrapping_add(self.velocity.z);
        events
    }
}

#[cfg(test)]
mod tests {
    use super::super::intro_controller::{
        IntroColor, OpeningSceneController, OpeningScenePalette, INTRO_PALETTE_COLORS,
    };
    use super::*;

    #[test]
    fn authored_timeline_emits_each_actor_once_and_keeps_holding_root_in_motion() {
        const COMPLETE_TIMELINE_UPDATES: usize = 460;
        const ROOT_ACTOR_COUNT: usize = 11;
        let mut controller = OpeningSceneController::default();
        let mut palette = OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);
        let mut root = OpeningSceneRoot::default();
        let mut actors = Vec::new();
        let mut audio_count = 0;
        let mut removals = Vec::new();
        for _ in 0..COMPLETE_TIMELINE_UPDATES {
            controller.tick(&mut palette);
            for event in root.tick(controller.cue()) {
                match event {
                    OpeningRootEvent::Spawn(spawn) => actors.push(spawn.actor),
                    OpeningRootEvent::QueueFlybyAudio => audio_count += 1,
                    OpeningRootEvent::RemoveFirstAttachment(group) => removals.push(group),
                    OpeningRootEvent::Initialize { .. } => {}
                }
            }
        }
        assert_eq!(actors.len(), ROOT_ACTOR_COUNT);
        assert_eq!(actors.first(), Some(&OpeningRootActor::CameraTarget));
        assert_eq!(actors.last(), Some(&OpeningRootActor::SecondCameraTarget));
        assert_eq!(audio_count, 1);
        assert_eq!(removals, [OpeningAttachmentGroup::FlybyRig]);
        assert_eq!(root.phase(), OpeningRootPhase::Holding);
        let before = root.pose;
        assert!(root.tick(OpeningCameraCue::FinalCut).is_empty());
        assert_eq!(
            root.pose.position.z,
            before.position.z.wrapping_add(root.depth_velocity())
        );
    }

    #[test]
    fn missing_first_cut_never_invents_later_actors() {
        const AFTER_ALL_AUTHORED_EVENTS: usize = 500;
        const BEFORE_FIRST_CUT_ACTORS: usize = 9;
        let mut root = OpeningSceneRoot::default();
        let mut spawns = 0;
        for _ in 0..AFTER_ALL_AUTHORED_EVENTS {
            spawns += root
                .tick(OpeningCameraCue::FinalCut)
                .iter()
                .filter(|event| matches!(event, OpeningRootEvent::Spawn(_)))
                .count();
        }
        assert_eq!(spawns, BEFORE_FIRST_CUT_ACTORS);
        assert_eq!(root.phase(), OpeningRootPhase::AwaitingFirstCut);
        assert!(root.tick(OpeningCameraCue::FirstCut).is_empty());
        assert_eq!(root.phase(), OpeningRootPhase::FirstCutPause);
        let events = root.tick(OpeningCameraCue::ThirdCut);
        assert!(matches!(
            events.as_slice(),
            [
                OpeningRootEvent::RemoveFirstAttachment(OpeningAttachmentGroup::FlybyRig),
                OpeningRootEvent::Spawn(OpeningRootSpawn {
                    actor: OpeningRootActor::SecondFlybyCraft,
                    ..
                }),
                OpeningRootEvent::Spawn(OpeningRootSpawn {
                    actor: OpeningRootActor::SecondCameraTarget,
                    ..
                }),
            ]
        ));
    }
}
