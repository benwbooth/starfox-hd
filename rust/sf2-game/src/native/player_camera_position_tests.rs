use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

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
    proxy: ObjectId,
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
        let proxy = allocate();
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
                            z: 100,
                        },
                    }),
                    camera_position: Some(Default::default()),
                    occupancy_exempt: Some(false),
                    camera_tracking: Some(Default::default()),
                    charge: Some(Default::default()),
                    contact: Some(Default::default()),
                    view_distance: Some(Default::default()),
                    motion: Some(Default::default()),
                    steering: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.processed_player_input = Some(Default::default());
        world.player_carry_mode = Some(1);
        Self {
            objects,
            world,
            execution: Default::default(),
            owner,
            proxy,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn lateral(&mut self, prepared: Vector3) -> Result<Vector3, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_camera_lateral(self.owner, prepared)
    }
    fn distance(
        &mut self,
        prepared: Vector3,
        style: TrackingStyle,
    ) -> Result<Vector3, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_camera_distance(self.owner, prepared, style)
    }
    fn boost(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_camera_boost(self.owner)
    }
}

#[test]
fn linked_lateral_clears_three_retained_fields_without_reading_auxiliary_or_input() {
    let mut f = Fixture::new();
    f.records().camera_position = Some(PlayerCameraPosition {
        lateral_offset: 39,
        secondary_lateral_offset: -17,
        lateral_accumulator: 77,
        longitudinal_offset: 53,
    });
    f.records().occupancy_exempt = Some(true);
    f.records().charge.as_mut().unwrap().linked_muzzle_disabled = true;
    f.records().auxiliary = None;
    f.records().motion = None;
    f.world.processed_player_input = None;
    let prepared = Vector3 {
        x: -131,
        y: 47,
        z: 299,
    };
    assert_eq!(f.lateral(prepared), Ok(prepared));
    assert_eq!(
        f.records().camera_position.unwrap(),
        PlayerCameraPosition {
            longitudinal_offset: 53,
            ..Default::default()
        }
    );
    assert_eq!(f.records().occupancy_exempt, Some(true));
}

#[test]
fn nonflight_lateral_resets_without_observing_flight_services() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x21;
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .lateral_accumulator = 100;
    f.records().motion = None;
    f.records().steering = None;
    f.world.processed_player_input = None;
    assert_eq!(f.lateral(Vector3::default()), Ok(Vector3::default()));
    assert_eq!(f.records().camera_position.unwrap().lateral_accumulator, 0);
}

#[test]
fn locked_heading_ignores_shoulder_and_obstruction_recovery_but_integrates_real_impulse() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().action_flags = 5;
    f.records().motion.as_mut().unwrap().contact_flags = 0xC0;
    f.records().motion.as_mut().unwrap().lateral_impulse = -128;
    f.records().steering.as_mut().unwrap().lateral_offset = i16::MIN;
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .lateral_accumulator = -13;
    f.world.processed_player_input = None;
    f.lateral(Vector3::default()).unwrap();
    assert_eq!(
        f.records().camera_position.unwrap().lateral_accumulator,
        100
    );
    assert_eq!(f.records().camera_position.unwrap().lateral_offset, 25);
}

#[test]
fn obstruction_and_shoulder_recovery_round_separately_before_lateral_chase() {
    let mut f = Fixture::new();
    f.records().motion.as_mut().unwrap().contact_flags = 0x80;
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .lateral_accumulator = -15;
    f.records().steering = None;
    f.world.processed_player_input = None;
    f.lateral(Vector3::default()).unwrap();
    assert_eq!(f.records().camera_position.unwrap().lateral_accumulator, -3);
    assert_eq!(f.records().camera_position.unwrap().lateral_offset, -1);
    f.records().motion.as_mut().unwrap().contact_flags = 0;
    f.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(0x10),
        pressed: Buttons::default(),
    });
    f.lateral(Vector3::default()).unwrap();
    assert_eq!(f.records().camera_position.unwrap().lateral_accumulator, 0);
    assert_eq!(f.records().camera_position.unwrap().lateral_offset, 0);
}

#[test]
fn ordinary_distance_reads_only_shared_angles_height_and_distance() {
    let mut f = Fixture::new();
    f.records().camera_position = None;
    f.records().charge = None;
    f.records().motion = None;
    f.records().contact = None;
    f.records().view_distance.as_mut().unwrap().distance = -210;
    f.records()
        .camera_tracking
        .as_mut()
        .unwrap()
        .vertical_offset = 20;
    f.world.player_carry_mode = None;
    assert_eq!(
        f.distance(Vector3::default(), TrackingStyle::Normal),
        Ok(Vector3 {
            x: 0,
            y: 19,
            z: -103
        })
    );
}

#[test]
fn protected_surface_distance_keeps_its_source_prefix_when_later_offset_is_missing() {
    let mut f = Fixture::new();
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.records().camera_position = None;
    f.records().view_distance.as_mut().unwrap().distance = -200;
    assert_eq!(
        f.distance(Vector3::default(), TrackingStyle::Surface),
        Err(SceneError::PlayerCameraPosition(
            CameraPositionError::MissingPosition(f.owner)
        ))
    );
    assert_eq!(f.records().view_distance.unwrap().distance, -250);
    assert_eq!(
        f.distance(Vector3::default(), TrackingStyle::Surface),
        Err(SceneError::Faulted)
    );
    assert_eq!(f.records().view_distance.unwrap().distance, -250);
}

#[test]
fn surface_distance_uses_canonical_support_carry_and_contact_gates() {
    let mut f = Fixture::new();
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = Some(f.proxy);
    f.distance(Vector3::default(), TrackingStyle::Surface)
        .unwrap();
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, -6);
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .longitudinal_offset = 0;
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    f.distance(Vector3::default(), TrackingStyle::Surface)
        .unwrap();
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, -3);
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .longitudinal_offset = 0;
    f.records().motion.as_mut().unwrap().contact_flags = 0x80;
    f.distance(Vector3::default(), TrackingStyle::Surface)
        .unwrap();
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, 25);
}

#[test]
fn linked_boost_copies_real_proxy_but_preserves_impulse_and_does_not_project_decaying_offset() {
    let mut f = Fixture::new();
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().charge.as_mut().unwrap().speed_impulse = 333;
    f.records().charge.as_mut().unwrap().speed_impulse_ticks = 17;
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .longitudinal_offset = 80;
    f.records().view_distance = None;
    let stored = f.records().auxiliary.unwrap().stored_world_position;
    f.boost().unwrap();
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, 70);
    assert_eq!(f.records().charge.unwrap().speed_impulse, 333);
    assert_eq!(f.records().charge.unwrap().speed_impulse_ticks, 17);
    assert_eq!(f.objects.get(f.proxy).unwrap().base.position, stored);
    assert_eq!(f.records().auxiliary.unwrap().stored_world_position, stored);
}

#[test]
fn boost_target_uses_entering_impulse_and_displacement_uses_updated_impulse() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().action_flags = 0x61;
    f.records().view_distance.as_mut().unwrap().boost_response = 20;
    f.records().view_distance.as_mut().unwrap().brake_response = -200;
    f.records().charge.as_mut().unwrap().speed_impulse = 100;
    f.records().charge.as_mut().unwrap().speed_impulse_ticks = 1;
    f.boost().unwrap();
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, 15);
    assert_eq!(f.records().charge.unwrap().speed_impulse, 85);
    assert_eq!(f.records().charge.unwrap().speed_impulse_ticks, 0);
    assert_eq!(
        f.objects.get(f.proxy).unwrap().base.position,
        Vector3 {
            x: 17,
            y: -29,
            z: 213
        }
    );
    assert_eq!(f.records().auxiliary.unwrap().stored_world_position.z, 213);
}

#[test]
fn missing_boost_proxy_preserves_response_prefix_and_faults_before_position_copy() {
    let mut f = Fixture::new();
    f.records().charge.as_mut().unwrap().speed_impulse = 80;
    f.records()
        .camera_position
        .as_mut()
        .unwrap()
        .longitudinal_offset = 80;
    f.world.weapons = None;
    let before = f.records().auxiliary.unwrap().stored_world_position;
    assert_eq!(
        f.boost(),
        Err(SceneError::PlayerCameraPosition(
            CameraPositionError::MissingProxy
        ))
    );
    assert_eq!(f.records().charge.unwrap().speed_impulse, 75);
    assert_eq!(f.records().camera_position.unwrap().longitudinal_offset, 70);
    assert_eq!(f.records().auxiliary.unwrap().stored_world_position, before);
    let after = *f.records();
    assert_eq!(f.boost(), Err(SceneError::Faulted));
    assert_eq!(*f.records(), after);
}

#[test]
fn complete_position_prefix_retains_prepared_and_boosted_positions_as_distinct_values() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().action_flags = 0;
    f.records().ambient = Some(super::super::player_ambient::PlayerAmbient {
        retained_offset: 17,
        ..Default::default()
    });
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: 100,
        y: -200,
        z: 300,
    };
    f.records().view_distance.as_mut().unwrap().distance = -100;
    f.records().charge.as_mut().unwrap().speed_impulse = 80;
    let prepared = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        callbacks: &mut Callbacks,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        statement_budget: 64,
    }
    .prepare_player_camera_position(f.owner, TrackingStyle::Normal, false)
    .unwrap();
    assert_eq!(
        prepared,
        Vector3 {
            x: 100,
            y: -183,
            z: 251
        }
    );
    let stored = f.records().auxiliary.unwrap().stored_world_position;
    assert_eq!(
        stored,
        Vector3 {
            x: 100,
            y: -183,
            z: 324
        }
    );
    assert_eq!(f.objects.get(f.proxy).unwrap().base.position, stored);
    assert_eq!(
        f.objects.get(f.owner).unwrap().base.position,
        Vector3 {
            x: 100,
            y: -200,
            z: 300
        }
    );
}
