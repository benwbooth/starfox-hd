use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Angle, Behavior, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

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
    owner: ObjectId,
    execution: SceneExecution,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
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
                        stored_world_position: Default::default(),
                    }),
                    camera_angles: Some(Default::default()),
                    camera_tracking: Some(Default::default()),
                    charge: Some(Default::default()),
                    vertical: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.contacts_enabled = Some(true);
        world.processed_player_input = Some(Default::default());
        world.player_carry_mode = Some(1);
        Self {
            objects,
            world,
            owner,
            execution: Default::default(),
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn held(&mut self, held: u16) {
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::default(),
        });
    }
    fn step(
        &mut self,
        height: i16,
        style: TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<i16, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_camera_height(self.owner, height, style, auxiliary_camera)
    }
}

#[test]
fn alternate_family_skips_all_height_services_and_inactive_mode_only_decays_offset() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x3F;
    f.records().camera_tracking = None;
    f.records().camera_angles = None;
    f.records().charge = None;
    f.world.contacts_enabled = None;
    f.world.processed_player_input = None;
    assert_eq!(f.step(-137, TrackingStyle::Normal, true), Ok(-137));
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.records().auxiliary.as_mut().unwrap().action_flags = 0xFE;
    f.records().camera_tracking = Some(PlayerCameraTracking {
        anchor_height: 123,
        height_difference: -49,
        vertical_offset: 80,
    });
    assert_eq!(f.step(219, TrackingStyle::Normal, false), Ok(219));
    assert_eq!(
        f.records().camera_tracking.unwrap(),
        PlayerCameraTracking {
            anchor_height: 123,
            height_difference: -49,
            vertical_offset: 70
        }
    );
}

#[test]
fn linked_reset_uses_actor_height_and_pitch_without_prepared_height_or_input() {
    let mut f = Fixture::new();
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().camera_angles.as_mut().unwrap().height_control = 0xFF;
    f.objects.get_mut(f.owner).unwrap().base.position.y = -239;
    f.objects.get_mut(f.owner).unwrap().base.pitch = Angle::from_units(0x80);
    f.world.processed_player_input = None;
    assert_eq!(f.step(711, TrackingStyle::Surface, true), Ok(711));
    assert_eq!(
        f.records().camera_tracking.unwrap(),
        PlayerCameraTracking {
            anchor_height: -239,
            height_difference: 0,
            vertical_offset: -20
        }
    );
    assert_eq!(f.records().auxiliary.unwrap().stored_world_position.y, -239);
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xC5);
    f.objects.get_mut(f.owner).unwrap().base.pitch = Angle::from_units(0x7F);
    f.step(0, TrackingStyle::Normal, false).unwrap();
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xE8);
}

#[test]
fn missing_reset_angles_preserve_original_mutation_prefix_and_prevent_retry() {
    let mut f = Fixture::new();
    f.world.contacts_enabled = Some(false);
    f.records().charge = None;
    f.records().camera_angles = None;
    f.records()
        .camera_tracking
        .as_mut()
        .unwrap()
        .height_difference = 319;
    f.objects.get_mut(f.owner).unwrap().base.position.y = 91;
    assert_eq!(
        f.step(27, TrackingStyle::Normal, false),
        Err(SceneError::PlayerCameraTracking(
            CameraTrackingError::MissingAngles(f.owner)
        ))
    );
    assert_eq!(
        f.records().camera_tracking.unwrap(),
        PlayerCameraTracking {
            anchor_height: 91,
            height_difference: 0,
            vertical_offset: -20
        }
    );
    assert_eq!(f.records().auxiliary.unwrap().stored_world_position.y, 91);
    let before = *f.records();
    assert_eq!(
        f.step(27, TrackingStyle::Normal, false),
        Err(SceneError::Faulted)
    );
    assert_eq!(*f.records(), before);
}

#[test]
fn carried_walker_changes_only_prepared_height_with_signed_floor_correction() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x21;
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    f.records().camera_angles = None;
    f.records().camera_tracking = None;
    f.records().vertical = None;
    f.world.processed_player_input = None;
    let before = *f.records();
    assert_eq!(f.step(-33, TrackingStyle::Normal, true), Ok(-31));
    assert_eq!(*f.records(), before);
    f.world.player_carry_mode = Some(0);
    assert_eq!(f.step(-33, TrackingStyle::Normal, true), Ok(-33));
}

#[test]
fn auxiliary_camera_resets_anchor_but_preserves_height_control_and_skips_input() {
    let mut f = Fixture::new();
    f.records().camera_angles.as_mut().unwrap().height_control = 0xB7;
    f.records()
        .camera_tracking
        .as_mut()
        .unwrap()
        .height_difference = -319;
    f.objects.get_mut(f.owner).unwrap().base.position.y = 319;
    f.world.processed_player_input = None;
    assert_eq!(f.step(-711, TrackingStyle::Normal, true), Ok(-711));
    assert_eq!(
        f.records().camera_tracking.unwrap(),
        PlayerCameraTracking {
            anchor_height: 319,
            height_difference: 0,
            vertical_offset: 0
        }
    );
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xB7);
}

#[test]
fn opposing_controls_choose_up_and_direction_changes_clear_old_follow_state() {
    let mut f = Fixture::new();
    f.records().camera_angles.as_mut().unwrap().height_control = 0xC7;
    f.held(Button::Up as u16 | Button::Down as u16);
    f.step(31, TrackingStyle::ProjectionCorrected, false)
        .unwrap();
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xF8);
    assert_eq!(f.records().camera_tracking.unwrap().height_difference, 31);
    assert_eq!(f.records().camera_tracking.unwrap().vertical_offset, -14);
    f.held(Button::Down as u16);
    f.objects.get_mut(f.owner).unwrap().base.pitch = Angle::from_units(0xFF);
    f.step(-100, TrackingStyle::ProjectionCorrected, false)
        .unwrap();
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xC7);
    f.held(0);
    f.step(-100, TrackingStyle::ProjectionCorrected, false)
        .unwrap();
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xC4);
}

#[test]
fn ground_clearance_recovers_difference_before_offset_and_projection_skips_that_service() {
    let mut f = Fixture::new();
    f.records()
        .camera_tracking
        .as_mut()
        .unwrap()
        .height_difference = -11;
    f.records().camera_angles.as_mut().unwrap().height_control = 0xFA;
    f.step(-100, TrackingStyle::Normal, false).unwrap();
    assert_eq!(
        f.records().camera_tracking.unwrap(),
        PlayerCameraTracking {
            anchor_height: -94,
            height_difference: -6,
            vertical_offset: 2
        }
    );
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0xE0);
    f.records().vertical = None;
    f.step(10, TrackingStyle::ProjectionCorrected, false)
        .unwrap();
    assert_eq!(f.records().camera_tracking.unwrap().height_difference, 104);
}

#[test]
fn height_follow_flags_feed_the_existing_camera_pitch_owner() {
    let mut f = Fixture::new();
    f.held(Button::Up as u16);
    f.records().camera_angles.as_mut().unwrap().profile.down = -12;
    f.step(80, TrackingStyle::ProjectionCorrected, false)
        .unwrap();
    assert_eq!(f.records().camera_angles.unwrap().height_control, 0x38);
    super::super::player_camera_angles::advance_pitch(&f.objects, &mut f.world, f.owner).unwrap();
    let records = f.records();
    let camera = records.camera_angles.unwrap();
    assert_eq!(camera.pitch_increment, -448);
    assert_eq!(
        camera
            .capture(records.auxiliary.unwrap().stored_rotation)
            .pitch,
        (-448_i16) as u16
    );
}
