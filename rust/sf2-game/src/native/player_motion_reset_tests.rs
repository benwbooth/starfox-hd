use super::*;
use crate::path_runtime::PathRuntime;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_path_world::ScenePathWorld;
use crate::{Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId};

fn fixture() -> (ObjectStore, ScenePathWorld, PathRuntime, ObjectId) {
    let mut objects = ObjectStore::new();
    let mut actor = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::Unassigned);
    actor.base.pitch = Angle::from_units(173);
    actor.base.yaw = Angle::from_units(193);
    actor.base.roll = Angle::from_units(251);
    let owner = objects.allocate(actor).unwrap();
    let mut world = ScenePathWorld::new(Default::default());
    let mut runtime = PathRuntime::default();
    player_storage::replace(
        &mut objects,
        &mut world,
        &mut runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 5,
            reserve_shield: 173,
            score: player_storage::PlayerScore::from_parts(59173, 197),
        },
    )
    .unwrap();
    (objects, world, runtime, owner)
}

fn prepared_fixture() -> (ObjectStore, ScenePathWorld, PathRuntime, ObjectId) {
    let (mut objects, mut world, runtime, owner) = fixture();
    world.render_environment.ambient_control =
        Some(crate::player_surface_render::AmbientParticleControl::from_bits(0xCDEF));
    world.player_pitch_target = Some(37);
    world.player_yaw_increment = Some(89);
    world.player_roll_increment = Some(129);
    world.scene.active_weapon_level = Some(173);
    world.active_consumables = Some(crate::player_visit::PublishedConsumables {
        packed_count: 191,
        kind: 253,
    });
    world.surface_mode = Some(crate::collision_surface::SurfaceMode { flags: 0xA7 });
    let actor = objects.get_mut(owner).unwrap();
    actor.base.velocity = crate::Vector3 {
        x: 129,
        y: -16384,
        z: 27319,
    };
    actor.extension.path_state.motion_delta = crate::Vector3 { x: -3, y: 5, z: -7 };
    actor.extension.path_state.platform_carry.saved_position = crate::Vector3 {
        x: 41,
        y: -43,
        z: 71,
    };
    actor.extension.path_state.motion.carry_selected_player = true;
    (objects, world, runtime, owner)
}

#[test]
fn scene_entry_preserves_ambient_upper_byte_and_surface_upper_flags_for_every_word() {
    let (mut objects, mut world, mut runtime, owner) = prepared_fixture();
    let velocity = objects.get(owner).unwrap().base.velocity;
    for bits in 0..=u16::MAX {
        world.render_environment.ambient_control =
            Some(crate::player_surface_render::AmbientParticleControl::from_bits(bits));
        world.surface_mode.as_mut().unwrap().flags = bits as u8;
        prepare_scene_entry(&mut objects, &mut world, &mut runtime.resources, owner).unwrap();
        assert_eq!(
            world.render_environment.ambient_control.unwrap().bits(),
            bits & 0xFF00
        );
        assert_eq!(world.surface_mode.unwrap().flags, (bits as u8 & 0xF8) | 1);
        assert_eq!(objects.get(owner).unwrap().base.velocity, velocity);
        assert_eq!(
            world.player(&objects, owner).unwrap().equipment.unwrap(),
            crate::path_equipment::SelectedEquipment {
                packed_consumables: 191,
                consumable_type: 253,
                weapon_level: 173,
            }
        );
    }
}

#[test]
fn scene_entry_late_missing_inputs_preserve_completed_earlier_effects() {
    for missing in 0..6 {
        let (mut objects, mut world, mut runtime, owner) = prepared_fixture();
        let expected = match missing {
            0 => {
                world.render_environment.ambient_control = None;
                MotionResetError::MissingAmbientControl
            }
            1 => {
                world.player_mut(&objects, owner).unwrap().contact = None;
                MotionResetError::MissingRetainedRecord(MissingRetainedRecord::Contact)
            }
            2 => {
                world.scene.active_weapon_level = None;
                MotionResetError::MissingPublishedWeaponLevel
            }
            3 => {
                world.player_mut(&objects, owner).unwrap().equipment = None;
                MotionResetError::MissingEquipment(owner)
            }
            4 => {
                world.active_consumables = None;
                MotionResetError::MissingPublishedConsumables
            }
            5 => {
                world.surface_mode = None;
                MotionResetError::MissingSurfaceMode
            }
            _ => unreachable!(),
        };
        let before_objects = objects.clone();
        assert_eq!(
            prepare_scene_entry(&mut objects, &mut world, &mut runtime.resources, owner),
            Err(expected)
        );
        if missing == 0 {
            assert_eq!(world.player_pitch_target, Some(37));
            assert_eq!(world.player_yaw_increment, Some(89));
            assert_eq!(world.player_roll_increment, Some(129));
        } else {
            assert_eq!(
                world.render_environment.ambient_control.unwrap().bits(),
                0xCD00
            );
            assert_eq!(world.player_pitch_target, Some(0));
            assert_eq!(world.player_yaw_increment, Some(0));
            assert_eq!(world.player_roll_increment, Some(0));
        }
        assert_eq!(
            player_storage::get(&objects, &runtime.resources, owner)
                .unwrap()
                .fine_pitch,
            if missing < 2 { 173 << 8 } else { 0 }
        );
        if missing == 4 {
            assert_eq!(
                world.player(&objects, owner).unwrap().equipment.unwrap(),
                crate::path_equipment::SelectedEquipment {
                    packed_consumables: 0,
                    consumable_type: 0,
                    weapon_level: 173,
                }
            );
        }
        if missing < 5 {
            assert_eq!(objects, before_objects);
        } else {
            let state = &objects.get(owner).unwrap().extension.path_state;
            assert_eq!(state.motion_delta, Default::default());
            assert_eq!(state.platform_carry.saved_position, Default::default());
            assert!(!state.motion.carry_selected_player);
        }
    }
}

#[test]
fn partial_reset_keeps_the_original_allocation_equipment_shots_and_shared_publications() {
    let (objects, mut world, mut runtime, owner) = fixture();
    world
        .bind_shots(
            &objects,
            owner,
            crate::path_shots::ActiveShots::from_count(213),
        )
        .unwrap();
    world.processed_player_input = Some(crate::InputState {
        held: crate::Buttons::from_bits(59311),
        pressed: crate::Buttons::from_bits(1237),
    });
    world.player_pitch_target = Some(59171);
    world.player_yaw_increment = Some(12713);
    let record = world.player_mut(&objects, owner).unwrap();
    record.equipment = Some(crate::path_equipment::SelectedEquipment {
        packed_consumables: 239,
        consumable_type: 135,
        weapon_level: 79,
    });
    record.protection = Some(crate::path_protection::DeflectionProtection::from_control(
        0xDF,
    ));
    record.action = Some(crate::player_action::PlayerActionState {
        action: Some(crate::player_action::PlayerAction::ForcedRetreat),
        elapsed: 49171,
        auxiliary_counter: 12971,
        total_updates: 65535,
    });
    record.charge.as_mut().unwrap().progress = 0xABCD;
    record.charge.as_mut().unwrap().control = 0xEF;
    record.charge.as_mut().unwrap().speed_impulse = -129;
    record.contact.as_mut().unwrap().hit.recovery = 179;
    record.contact.as_mut().unwrap().hit.secondary_protection = 241;
    record
        .contact
        .as_mut()
        .unwrap()
        .hit
        .deflection_sound_cooldown = 171;
    record.visit.as_mut().unwrap().shield_warning_clock = 251;
    let before = *record;
    let objects_before = objects.clone();
    let available = runtime.resources.available_capacity();
    reset(&objects, &mut world, &mut runtime.resources, owner).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.visit, before.visit);
    assert_eq!(record.equipment, before.equipment);
    assert_eq!(record.protection, before.protection);
    assert_eq!(record.action, before.action);
    assert_eq!(record.score, before.score);
    assert_eq!(record.charge.unwrap().progress, 0xABCD);
    assert_eq!(record.charge.unwrap().control, 0xEF);
    assert_eq!(record.charge.unwrap().speed_impulse, 0);
    assert_eq!(record.contact.unwrap().hit.recovery, 0);
    assert_eq!(record.contact.unwrap().hit.secondary_protection, 0);
    assert_eq!(record.contact.unwrap().hit.deflection_sound_cooldown, 0);
    assert_eq!(record.contact.unwrap().hit.reserve_shield, 173);
    assert_eq!(world.shots(&objects, owner).unwrap().count(), 213);
    assert_eq!(world.player_pitch_target, Some(59171));
    assert_eq!(world.player_yaw_increment, Some(12713));
    assert_eq!(world.processed_player_input.unwrap().held.bits(), 59311);
    assert_eq!(objects, objects_before);
    assert_eq!(runtime.resources.available_capacity(), available);
    assert_eq!(
        player_storage::get(&objects, &runtime.resources, owner).unwrap(),
        &PlayerStorage {
            fine_pitch: 0,
            fine_yaw: 0,
            bank: Angle::ZERO,
            retained_shield: 173,
        }
    );
}

#[test]
fn every_missing_retained_portion_is_diagnosed_before_mutating_the_allocation() {
    let cases: [(MissingRetainedRecord, fn(&mut PlayerPathRecords)); 10] = [
        (MissingRetainedRecord::Contact, |r| r.contact = None),
        (MissingRetainedRecord::Charge, |r| r.charge = None),
        (MissingRetainedRecord::Consumable, |r| r.consumable = None),
        (MissingRetainedRecord::TargetControl, |r| {
            r.target_control = None
        }),
        (MissingRetainedRecord::Vertical, |r| r.vertical = None),
        (MissingRetainedRecord::Boundary, |r| r.boundary = None),
        (MissingRetainedRecord::ModeSelection, |r| {
            r.mode_selection = None
        }),
        (MissingRetainedRecord::CameraAngles, |r| {
            r.camera_angles = None
        }),
        (MissingRetainedRecord::CameraGround, |r| {
            r.camera_ground = None
        }),
        (MissingRetainedRecord::Carried, |r| r.carried = None),
    ];
    for (missing, remove) in cases {
        let (objects, mut world, mut runtime, owner) = fixture();
        remove(world.player_mut(&objects, owner).unwrap());
        let records = *world.player(&objects, owner).unwrap();
        let resources = runtime.resources.clone();
        assert_eq!(
            reset(&objects, &mut world, &mut runtime.resources, owner),
            Err(MotionResetError::MissingRetainedRecord(missing))
        );
        assert_eq!(world.player(&objects, owner).unwrap(), &records);
        assert_eq!(runtime.resources, resources);
    }
}

#[test]
fn cleared_records_are_initialized_but_absent_retained_records_are_not_invented() {
    let (objects, mut world, mut runtime, owner) = fixture();
    let record = world.player_mut(&objects, owner).unwrap();
    record.equipment = None;
    record.protection = None;
    record.action = None;
    record.score = None;
    record.visit = None;
    record.status = None;
    record.occupancy_exempt = None;
    record.rapid_rejection_consumes_queue = None;
    record.auxiliary = None;
    record.pose = None;
    record.target_selection = None;
    record.camera_dispatch = None;
    reset(&objects, &mut world, &mut runtime.resources, owner).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.equipment, None);
    assert_eq!(record.protection, None);
    assert_eq!(record.action, None);
    assert_eq!(record.score, None);
    assert_eq!(record.visit, None);
    assert_eq!(record.status, None);
    assert_eq!(record.occupancy_exempt, None);
    assert_eq!(record.rapid_rejection_consumes_queue, None);
    assert_eq!(record.auxiliary.unwrap().mode, 0);
    assert_eq!(record.pose, Some(Default::default()));
    assert_eq!(record.target_selection, Some(Default::default()));
    assert_eq!(record.camera_dispatch, Some(Default::default()));
}

#[test]
fn a_released_allocation_cannot_reset_a_stale_player_binding() {
    let (objects, mut world, mut runtime, owner) = fixture();
    let record = *world.player(&objects, owner).unwrap();
    runtime.resources.release_owner(owner);
    assert_eq!(
        reset(&objects, &mut world, &mut runtime.resources, owner),
        Err(MotionResetError::Storage(
            player_storage::PlayerStorageError::MissingStorage(owner)
        ))
    );
    assert_eq!(world.player(&objects, owner).unwrap(), &record);
    assert_eq!(runtime.resources.owner_count(owner), 0);
}

struct Callbacks;
impl crate::scene_strategy::SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<crate::strategy_schedule::StrategyCompletion, ()> {
        panic!("record reset must not run strategies")
    }
    fn death_override(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<crate::strategy_schedule::StrategyCompletion>, ()> {
        panic!("record reset must not run death callbacks")
    }
    fn resume_map_on_death(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), ()> {
        panic!("record reset must not resume maps")
    }
}

#[test]
fn scene_owner_latches_a_missing_retained_record_against_retry() {
    use crate::scene_strategy::{SceneActors, SceneError, SceneExecution};
    let (mut objects, mut world, runtime, owner) = fixture();
    world.player_mut(&objects, owner).unwrap().camera_ground = None;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = runtime;
    let catalog = crate::authored_paths::catalog();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.reset_player_motion(owner),
        Err(SceneError::PlayerMotionReset(
            MotionResetError::MissingRetainedRecord(MissingRetainedRecord::CameraGround)
        ))
    );
    assert!(host.execution.is_faulted());
    host.world
        .player_mut(host.objects, owner)
        .unwrap()
        .camera_ground = Some(Default::default());
    assert_eq!(host.reset_player_motion(owner), Err(SceneError::Faulted));
    assert_eq!(
        player_storage::get(host.objects, &host.execution.paths.runtime.resources, owner)
            .unwrap()
            .fine_pitch,
        173 << 8
    );
}

#[test]
fn scene_entry_owner_latches_after_partial_equipment_restore_without_replaying() {
    use crate::scene_strategy::{SceneActors, SceneError, SceneExecution};
    let (mut objects, mut world, runtime, owner) = prepared_fixture();
    world.active_consumables = None;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = runtime;
    let catalog = crate::authored_paths::catalog();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.prepare_player_scene_entry(owner),
        Err(SceneError::PlayerMotionReset(
            MotionResetError::MissingPublishedConsumables
        ))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(
        host.world
            .render_environment
            .ambient_control
            .unwrap()
            .bits(),
        0xCD00
    );
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .equipment
            .unwrap()
            .weapon_level,
        173
    );
    assert_eq!(
        host.objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion_delta
            .x,
        -3
    );
    host.world.active_consumables = Some(Default::default());
    assert_eq!(
        host.prepare_player_scene_entry(owner),
        Err(SceneError::Faulted)
    );
    assert_eq!(
        host.objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion_delta
            .x,
        -3
    );
    assert_eq!(host.world.surface_mode.unwrap().flags, 0xA7);
}
