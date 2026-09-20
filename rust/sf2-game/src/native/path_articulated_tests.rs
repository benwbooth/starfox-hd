//! Linked segment placement and the source-authored Queen Dioray constructor.
use super::super::path_fields::{Axis, ByteField};
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{ActiveNodeFlags, EncounterHealthDisplay};
use super::super::path_steering::{position_relative_to_linked, SteeringError, SteeringState};
use super::super::{
    authored_paths, Angle, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId, Vector3,
};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

// Independent signed-magnitude products; doubling wraps at byte width before
// multiplication. This deliberately covers the source -128 -> zero case.
fn product(value: i8, coefficient: i8) -> i8 {
    let magnitude = (((i16::from(value).abs() * 2) & 255) * i16::from(coefficient).abs()) / 256;
    if (value < 0) != (coefficient < 0) {
        -magnitude as i8
    } else {
        magnitude as i8
    }
}

fn positioned(actor: &Object, target: &Object, distance: i8, self_link: bool) -> Object {
    use sf_core::aim_angle::{sf2_pitch_to_target, sf2_xz_angle_distance, sf2_yaw_to_target};
    use sf_core::snes_trig::{COSTAB, SINTAB};
    let mut expected = actor.clone();
    let dx = target.base.position.x.wrapping_sub(actor.base.position.x);
    let dy = target.base.position.y.wrapping_sub(actor.base.position.y);
    let dz = target.base.position.z.wrapping_sub(actor.base.position.z);
    expected.base.pitch = Angle::from_units(sf2_pitch_to_target(dy, sf2_xz_angle_distance(dx, dz)));
    expected.base.yaw = Angle::from_units(sf2_yaw_to_target(dx, dz));
    let frame = if self_link { &expected } else { target };
    let pitch = usize::from(frame.base.pitch.units());
    let yaw = usize::from(frame.base.yaw.units().wrapping_neg());
    let y = product(distance, SINTAB[pitch]);
    let z = product(distance, COSTAB[pitch]);
    let x = product(z, SINTAB[yaw]);
    let z = product(z, COSTAB[yaw]);
    expected.base.position = Vector3 {
        x: target.base.position.x.wrapping_add(i16::from(x) * 8),
        y: target.base.position.y.wrapping_add(i16::from(y) * 8),
        z: target.base.position.z.wrapping_add(i16::from(z) * 8),
    };
    expected
}

#[test]
fn linked_segment_byte_rotations_and_word_wrap_preserve_all_other_fields() {
    let (_, mut objects, owner, _) = setup();
    let peer = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    let initial = objects.get(owner).unwrap().clone();
    for value in 0..=u8::MAX {
        let mut target = initial.clone();
        target.base.pitch = Angle::from_units(value);
        target.base.yaw = Angle::from_units(value.wrapping_mul(37));
        target.base.roll = Angle::from_units(value.wrapping_add(91));
        target.base.position = Vector3 {
            x: 32760,
            y: -32760,
            z: -91,
        };
        *objects.get_mut(peer).unwrap() = target.clone();
        for offset in 0..=u8::MAX {
            let mut actor = initial.clone();
            actor.base.attachment = Some(peer);
            actor.base.pitch = Angle::from_units(231);
            actor.base.yaw = Angle::from_units(119);
            actor.base.roll = Angle::from_units(79);
            actor.base.child_number = 55;
            actor.base.wait_timer = 131;
            actor.base.position = Vector3 {
                x: i16::from(value) * 128,
                y: -15001,
                z: 31234,
            };
            actor.extension.path_state.motion.attached_coordinates = offset & 1 != 0;
            actor.extension.path_state.motion.relative_coordinates = offset & 2 != 0;
            actor.extension.relative_position = Vector3 {
                x: 137,
                y: -449,
                z: 971,
            };
            actor.extension.parent = Some(owner);
            let expected = positioned(&actor, &target, offset as i8, false);
            *objects.get_mut(owner).unwrap() = actor;
            let mut state = SteeringState {
                unchanged_axes: 255,
            };
            position_relative_to_linked(&mut objects, owner, offset as i8, &mut state).unwrap();
            assert_eq!(
                objects.get(owner).unwrap(),
                &expected,
                "angle {value}, offset {offset}"
            );
            assert_eq!(objects.get(peer).unwrap(), &target);
            assert_eq!(state.unchanged_axes, 0);
        }
    }
}

#[test]
fn linked_position_samples_angle_operand_before_facing_and_self_frame_after_facing() {
    for self_link in [false, true] {
        for distance in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let peer = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .unwrap();
            let target = if self_link { owner } else { peer };
            objects.get_mut(peer).unwrap().base.pitch = Angle::from_units(37);
            objects.get_mut(peer).unwrap().base.yaw = Angle::from_units(173);
            let actor = objects.get_mut(owner).unwrap();
            actor.base.attachment = Some(target);
            actor.base.pitch = Angle::from_units(distance);
            actor.base.yaw = Angle::from_units(41);
            actor.base.position = Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: 37,
            };
            let mut expected = objects.clone();
            let mut expected_actor = positioned(
                objects.get(owner).unwrap(),
                objects.get(target).unwrap(),
                distance as i8,
                self_link,
            );
            expected_actor.base.path = Some(at(1));
            *expected.get_mut(owner).unwrap() = expected_actor;
            let catalog = PathCatalog::new(vec![vec![Statement::PositionRelativeToLinked {
                distance: ByteOperand::Actor(ByteField::Rotation(Axis::X)),
                next: at(1),
            }]])
            .unwrap();
            runtime.branch.invert_next = distance & 1 != 0;
            runtime.steering.unchanged_axes = 255;
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.steering.unchanged_axes, 0);
            assert_eq!(runtime.branch.invert_next, distance & 1 != 0);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn missing_linked_position_target_faults_before_clearing_result_or_mutating_actor() {
    for stale in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let target = objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap();
        objects.remove(target);
        objects.get_mut(owner).unwrap().base.attachment = stale.then_some(target);
        let before = objects.clone();
        runtime.steering.unchanged_axes = 197;
        runtime.branch.invert_next = true;
        let before_random = random;
        let catalog = PathCatalog::new(vec![vec![Statement::PositionRelativeToLinked {
            distance: ByteOperand::Literal(128),
            next: at(1),
        }]])
        .unwrap();
        let expected = if stale {
            SteeringError::MissingActor(target)
        } else {
            SteeringError::MissingLinked(owner)
        };
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Runtime(PathRuntimeError::Steering(expected)))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.steering.unchanged_axes, 197);
        assert!(runtime.branch.invert_next);
        assert_eq!(random, before_random);
    }
}

#[test]
fn queen_dioray_constructor_honors_completed_node_and_installs_three_limb_roots() {
    let catalog = authored_paths::catalog();
    for completed in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::QUEEN_DIORAY);
        actor.base.hit_points = 1;
        let mut flags = ActiveNodeFlags {
            bits: u16::from(completed),
        };
        let mut health = EncounterHealthDisplay::default();
        let mut signals = EncounterSignals::default();
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.active_node_flags = Some(&mut flags);
        inputs.health_display = Some(&mut health);
        inputs.encounter_signals = Some(&mut signals);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            if completed {
                ControlStep::Ended
            } else {
                ControlStep::Movement
            }
        );
        if completed {
            assert_eq!(objects.len(), 1);
            assert_eq!(health.label, None);
        } else {
            assert_eq!(objects.len(), 4);
            assert_eq!(health.label, Some("QUEEN DIORAY"));
            assert_eq!((health.current, health.maximum), (25, 25));
            for (number, x, z, yaw) in [(1, -5, 0, 48), (5, 5, 0, 208), (10, 0, 5, 128)] {
                let child = objects
                    .get(find_child(&objects, owner, number).unwrap().unwrap())
                    .unwrap();
                assert_eq!(child.base.shape, ShapeId::EMPTY);
                assert_eq!(child.base.attachment, Some(owner));
                assert_eq!(child.extension.parent, Some(owner));
                assert_eq!(child.extension.relative_position, Vector3 { x, y: -20, z });
                assert_eq!(child.extension.relative_rotation.yaw.units(), yaw);
            }
            assert!(!objects.get(owner).unwrap().base.flags.visible);
        }
        assert_eq!(random, before_random);
    }
}

#[test]
fn queen_dioray_builds_three_four_link_limbs_without_corrupting_the_shared_child_chain() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::QUEEN_DIORAY);
    objects.get_mut(owner).unwrap().base.hit_points = 1;
    let mut flags = ActiveNodeFlags { bits: 0 };
    let mut health = EncounterHealthDisplay::default();
    let mut signals = EncounterSignals::default();
    let mut inputs = world(&mut random);
    inputs.active_node_flags = Some(&mut flags);
    inputs.health_display = Some(&mut health);
    inputs.encounter_signals = Some(&mut signals);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    for first_number in [1, 5, 10] {
        let mut previous = None;
        for offset in 0..4 {
            let number = first_number + offset;
            let id = find_child(&objects, owner, number).unwrap().unwrap();
            let before_path = objects.get(id).unwrap().base.path;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, id, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement,
                "limb {first_number}, segment {offset}"
            );
            let segment = objects.get(id).unwrap();
            assert_ne!(segment.base.path, before_path);
            assert_eq!(segment.base.attachment, Some(owner));
            assert_eq!(segment.base.linked_object, previous);
            assert!(!segment.base.flags.visible);
            // Visibility(false) also disables contacts. The boss reveal
            // callback later re-enables visibility before suppressing only
            // the intermediate links; the tip is not hittable while hidden.
            assert!(segment.base.flags.collision_disabled);
            assert_eq!(
                segment.base.kind,
                if offset == 3 {
                    ObjectKind::Enemy
                } else {
                    ObjectKind::Effect
                }
            );
            if offset < 3 {
                let next = find_child(&objects, owner, number + 1).unwrap().unwrap();
                let child = objects.get(next).unwrap();
                assert_eq!(child.base.linked_object, Some(id));
                assert_eq!(child.extension.parent, Some(id));
                assert_eq!(child.base.attachment, Some(owner));
                assert_eq!(
                    child.base.shape,
                    ShapeId::from_catalog_index(if offset < 2 { 336 } else { 337 })
                );
            }
            previous = Some(id);
        }
    }
    assert_eq!(objects.len(), 13);
    for number in [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13] {
        assert!(find_child(&objects, owner, number).unwrap().is_some());
    }
}
