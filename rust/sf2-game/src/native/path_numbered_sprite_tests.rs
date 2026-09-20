//! Complete source-authored counted constructors, without scheduler emulation.
use super::super::path_spawn::{SpawnArgument, SpawnError};
use super::super::{authored_paths, ObjectSpawnDefaults, PathId, ShapeId, OBJECT_CAPACITY};
use super::tests::{setup, world};
use super::*;

fn caller(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(1),
        command_index,
    }
}

fn called_catalog(target: PathCursor) -> PathCatalog {
    let mut catalog = authored_paths::catalog();
    catalog.paths.push(vec![
        Statement::Control(ControlCommand::Call {
            target,
            next: caller(1),
        }),
        Statement::Control(ControlCommand::WaitOne { next: caller(2) }),
    ]);
    catalog
}

#[test]
fn numbered_sprite_constructors_preserve_parent_arguments_and_wrap_child_numbers() {
    for (entry, shape) in [
        (authored_paths::SPAWN_NUMBERED_HIT_SPRITES, 19),
        (authored_paths::SPAWN_NUMBERED_LARGE_HIT_SPRITES, 21),
    ] {
        let catalog = called_catalog(entry);
        for first_number in 0..=u8::MAX {
            for count in [1u8, 3] {
                for offset in [i16::MIN, -80, 0, i16::MAX] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let attack = first_number ^ 0xFF;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(caller(0));
                    actor.extension.path_state.motion_phase = u16::from_le_bytes([attack, count]);
                    actor.extension.path_state.script_value = offset as u16;
                    let before = actor.clone();
                    let before_random = random;
                    runtime.spawns.companion_parameter = Some(first_number);
                    runtime.branch.invert_next = true;
                    runtime.placement.depth = Some(911);
                    let mut inputs = world(&mut random);
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                    assert_eq!(
                        runtime
                            .resume_program(&catalog, &mut objects, owner, &mut inputs, 256)
                            .unwrap(),
                        ProgramExit {
                            actor: owner,
                            step: ControlStep::Movement
                        }
                    );
                    assert_eq!(objects.len(), usize::from(count) + 1);
                    assert_eq!(runtime.spawns.parameter, Some(attack));
                    assert_eq!(
                        runtime.spawns.companion_parameter,
                        Some(first_number.wrapping_add(count))
                    );
                    assert_eq!(runtime.placement.primary, Some(offset));
                    assert_eq!(runtime.placement.depth, Some(911));
                    let mut expected = before;
                    expected.base.path = Some(caller(2));
                    expected.base.next = objects.get(owner).unwrap().base.next;
                    expected.base.first_child = objects.get(owner).unwrap().base.first_child;
                    expected.extension.path_state.motion.refresh_child_chain = true;
                    expected.extension.path_state.stack = objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .stack
                        .clone();
                    assert_eq!(
                        expected
                            .extension
                            .path_state
                            .stack
                            .pop_call(&mut runtime.resources),
                        Err(super::super::program_state::PathStackError::MissingLoop)
                    );
                    assert_eq!(objects.get(owner), Some(&expected));
                    let mut child = expected.base.first_child;
                    for sequence in 0..count {
                        let id = child.unwrap();
                        let actor = objects.get(id).unwrap();
                        let number = first_number.wrapping_add(sequence);
                        assert_eq!(actor.base.child_number, number);
                        assert_eq!(actor.base.attachment, Some(owner));
                        assert_eq!(actor.extension.parent, Some(owner));
                        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
                        assert_eq!(actor.base.hit_points, 100);
                        assert_eq!(actor.base.attack_power, attack);
                        assert_eq!(actor.extension.relative_position.z, offset);
                        assert_eq!(actor.extension.relative_position.x, 0);
                        assert_eq!(actor.extension.relative_position.y, 0);
                        assert_eq!(actor.extension.path_state.motion_phase as u8, number);
                        assert_eq!(actor.base.path, Some(authored_paths::HIT_TOGGLE_SPRITE));
                        if sequence + 1 == count {
                            assert_eq!(runtime.spawns.last_spawn, Some(id));
                        }
                        child = actor.base.next_sibling;
                    }
                    assert_eq!(child, None);
                    assert_eq!(random, before_random);
                    assert!(runtime.branch.invert_next);
                }
            }
        }
    }
}

#[test]
fn numbered_sprite_constructor_zero_count_is_not_empty_and_missing_argument_is_not_zero() {
    let catalog = called_catalog(authored_paths::SPAWN_NUMBERED_HIT_SPRITES);
    for missing in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(caller(0));
        runtime.spawns.companion_parameter = if missing { None } else { Some(250) };
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        let result = runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2048);
        if missing {
            assert_eq!(
                result,
                Err(ProgramError::MissingSpawnParameter(
                    SpawnArgument::Companion
                ))
            );
            // Publication and allocation precede the first companion read.
            assert_eq!(objects.len(), 2);
            assert_eq!(runtime.spawns.parameter, Some(0));
            assert_eq!(runtime.placement.primary, Some(0));
            assert_eq!(
                objects
                    .get(runtime.spawns.last_spawn.unwrap())
                    .unwrap()
                    .base
                    .child_number,
                9
            );
        } else {
            assert_eq!(result, Err(ProgramError::Spawn(SpawnError::PoolExhausted)));
            assert_eq!(objects.len(), OBJECT_CAPACITY);
            assert_eq!(runtime.spawns.last_spawn, None);
            assert_eq!(
                runtime.spawns.companion_parameter,
                Some(250u8.wrapping_add((OBJECT_CAPACITY - 1) as u8))
            );
        }
    }
}
