use super::*;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState, Statement};
use crate::path_runtime::PathRuntime;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{
    Behavior, Buttons, InputState, Object, ObjectKind, PathCursor, RandomState, ShapeId, Vector3,
};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not dispatched")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not dispatched")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not dispatched")
    }
}
struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
    view: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let owner = allocate();
        let view = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x11,
                        action_flags: 1,
                        stored_rotation: Default::default(),
                        stored_world_position: Vector3 {
                            x: 17,
                            y: -29,
                            z: 313,
                        },
                    }),
                    camera_angles: Some(Default::default()),
                    charge: Some(Default::default()),
                    contact: Some(Default::default()),
                    vertical: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.fixed_players[0] = Some(view);
        world.primary_player = Some(owner);
        world.processed_player_input = Some(Default::default());
        Self {
            objects,
            world,
            execution: Default::default(),
            owner,
            view,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn set_angles(&mut self, angles: FixedViewAngles) {
        let records = self.records();
        records.camera_angles.as_mut().unwrap().write(
            &mut records.auxiliary.as_mut().unwrap().stored_rotation,
            angles,
        );
    }
    fn angles(&mut self) -> FixedViewAngles {
        let owner = self.owner;
        angles(self.records(), owner).unwrap()
    }
    fn pitch(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_camera_pitch(self.owner)
    }
    fn publish(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .publish_player_camera_pose(self.owner)
    }
}

#[test]
fn fine_camera_angles_and_selected_path_rotation_have_one_live_high_byte_owner() {
    let mut f = Fixture::new();
    let current = FixedViewAngles {
        pitch: 0xA317,
        yaw: 0xB329,
        roll: 0xC33B,
    };
    f.set_angles(current);
    assert_eq!(f.angles(), current);
    f.records().auxiliary.as_mut().unwrap().stored_rotation.yaw = Angle::from_units(0xD4);
    assert_eq!(f.angles().yaw, 0xD429);
    let path = PathCatalog::new(vec![vec![Statement::CopySelectedStoredRotation {
        next: PathCursor {
            path: crate::PathId::from_catalog_index(0),
            command_index: 1,
        },
    }]])
    .unwrap();
    f.objects.get_mut(f.view).unwrap().base.path = Some(PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index: 0,
    });
    let mut runtime = PathRuntime::default();
    let mut invocation = f
        .world
        .path_world(&f.objects, f.view, PlayerTarget::Primary)
        .unwrap();
    let exit = runtime
        .step_program(&path, &mut f.objects, f.view, &mut invocation)
        .unwrap();
    assert_eq!(exit.step, crate::path_commands::ControlStep::Continue);
    let actor = f.objects.get(f.view).unwrap();
    assert_eq!(
        (
            actor.base.pitch.units(),
            actor.base.yaw.units(),
            actor.base.roll.units()
        ),
        (0xA3, 0xD4, 0xC3)
    );
    drop(invocation);
    assert_eq!(f.angles().yaw, 0xD429);
}

#[test]
fn nonlinked_pitch_resets_increment_before_missing_limits_and_fault_blocks_retry() {
    let mut f = Fixture::new();
    f.set_angles(FixedViewAngles {
        pitch: 0xABCD,
        yaw: 0xEF12,
        roll: 0x3456,
    });
    f.records().camera_angles.as_mut().unwrap().pitch_increment = 313;
    f.records().vertical = None;
    assert_eq!(
        f.pitch(),
        Err(SceneError::PlayerCameraAngles(
            CameraAnglesError::MissingVertical(f.owner)
        ))
    );
    assert_eq!(f.records().camera_angles.unwrap().pitch_increment, 0);
    assert_eq!(f.angles().pitch, 0xABCD);
    let before = *f.records();
    assert_eq!(f.pitch(), Err(SceneError::Faulted));
    assert_eq!(*f.records(), before);
}

#[test]
fn linked_neutral_input_recovers_increment_without_reading_or_changing_pose_or_limits() {
    let mut f = Fixture::new();
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().auxiliary = None;
    f.records().vertical = None;
    f.records().camera_angles.as_mut().unwrap().pitch_increment = -513;
    f.pitch().unwrap();
    assert_eq!(f.records().camera_angles.unwrap().pitch_increment, -385);
    f.world.processed_player_input = None;
    let before = *f.records();
    assert_eq!(
        f.pitch(),
        Err(SceneError::PlayerCameraAngles(
            CameraAnglesError::MissingProcessedInput
        ))
    );
    assert_eq!(*f.records(), before);
}

#[test]
fn source_halves_profile_before_expansion_only_for_nonlinked_limit_response() {
    let mut f = Fixture::new();
    f.records().camera_angles.as_mut().unwrap().profile = CameraPitchProfile { up: 5, down: -7 };
    f.records().camera_angles.as_mut().unwrap().height_control = 1;
    f.records().vertical.as_mut().unwrap().limit_flags = UPPER_LIMIT;
    f.set_angles(FixedViewAngles {
        pitch: 600,
        ..Default::default()
    });
    f.pitch().unwrap();
    // Nonlinked: (5 >> 1) << 8 = 512, then eighth-chase 600 toward 512.
    assert_eq!(f.angles().pitch, 589);
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().camera_angles.as_mut().unwrap().pitch_increment = LINKED_INCREMENT;
    f.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(Button::Down as u16),
        pressed: Buttons::default(),
    });
    f.set_angles(FixedViewAngles {
        pitch: 600,
        ..Default::default()
    });
    f.pitch().unwrap();
    // Linked: (5 << 8) >> 1 = 640; the two half operations are not equivalent.
    assert_eq!(f.angles().pitch, 605);
}

#[test]
fn linked_up_wins_both_directions_but_active_view_transition_uses_height_tracking() {
    let mut f = Fixture::new();
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().camera_angles.as_mut().unwrap().profile = CameraPitchProfile { up: 12, down: -12 };
    f.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(Button::Up as u16 | Button::Down as u16),
        pressed: Buttons::default(),
    });
    f.pitch().unwrap();
    assert_eq!(f.records().camera_angles.unwrap().pitch_increment, -128);
    assert_eq!(f.angles().pitch, (-128_i16) as u16);
    f.records().charge.as_mut().unwrap().linked_muzzle_disabled = true;
    f.records().camera_angles.as_mut().unwrap().height_control = 3;
    f.world.processed_player_input = None;
    f.pitch().unwrap();
    assert_eq!(
        f.records().camera_angles.unwrap().pitch_increment,
        TRACKING_INCREMENT
    );
    assert_eq!(f.angles().pitch, 320);
}

#[test]
fn publication_keeps_fine_aliases_recoil_roll_and_retained_position_distinct() {
    let mut f = Fixture::new();
    f.set_angles(FixedViewAngles {
        pitch: 0x7FF0,
        yaw: 0x0003,
        roll: 0x8123,
    });
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .camera_pitch_recoil = 31;
    f.records().camera_angles.as_mut().unwrap().yaw_offset = -9;
    f.objects.get_mut(f.view).unwrap().base.view_rear_distance = -91;
    let before = *f.records();
    f.publish().unwrap();
    let view = f.objects.get(f.view).unwrap();
    assert_eq!(
        FixedViewAngles::capture(view),
        FixedViewAngles {
            pitch: 0x802E,
            yaw: 0xFFFA,
            roll: 0x8123
        }
    );
    assert_eq!(view.base.view_rear_distance, 0);
    assert_eq!(
        view.base.position,
        Vector3 {
            x: 17,
            y: -29,
            z: 313
        }
    );
    assert_eq!(f.world.published_camera_roll, Some(0x8123));
    assert_eq!(*f.records(), before);
    f.records().contact = None;
    f.set_angles(FixedViewAngles {
        pitch: 0xA7B3,
        yaw: 17,
        roll: 99,
    });
    assert_eq!(
        f.publish(),
        Err(SceneError::PlayerCameraAngles(CameraAnglesError::World(
            WorldInputError::MissingPlayerContact(f.owner)
        )))
    );
    assert_eq!(
        FixedViewAngles::capture(f.objects.get(f.view).unwrap()),
        FixedViewAngles {
            pitch: 0xA7B3,
            yaw: 0xFFFA,
            roll: 0x8123
        }
    );
    assert_eq!(f.world.published_camera_roll, Some(0x8123));
}

#[test]
fn scripted_roll_publication_requires_no_selected_player_or_input_state() {
    let mut f = Fixture::new();
    f.world.release_player_bindings(f.owner);
    f.world.processed_player_input = None;
    FixedViewAngles {
        pitch: 123,
        yaw: 456,
        roll: 0xCAFE,
    }
    .write_to(f.objects.get_mut(f.view).unwrap());
    let before = f.objects.clone();
    publish_existing_roll(&f.objects, &mut f.world).unwrap();
    assert_eq!(f.world.published_camera_roll, Some(0xCAFE));
    assert_eq!(f.objects, before);
}
