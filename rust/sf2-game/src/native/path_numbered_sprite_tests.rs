//! Complete source-authored counted constructors, without scheduler emulation.
use super::super::path_scene_state::{
    EncounterCoordination, EncounterObjectiveCounts, ObjectiveCompletion,
};
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

#[test]
fn numbered_sprite_encounters_decode_the_attack_weapon_overlap_before_completion_skip() {
    let catalog = authored_paths::catalog();
    for entry in [
        authored_paths::NUMBERED_SPRITE_PURSUER,
        authored_paths::NUMBERED_SPRITE_BANKING_ATTACKER,
    ] {
        for attack in 0..=u8::MAX {
            for weapon in [0u8, 1, 127, 128, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(entry);
                actor.base.attack_power = attack;
                actor.base.hit_points = 71;
                actor.extension.path_state.weapon_selection = weapon;
                actor.extension.path_state.motion_phase = 0xABCD;
                actor.extension.path_state.motion_delta.x = i16::MAX;
                let before = actor.clone();
                let before_random = random;
                let mut completion = ObjectiveCompletion { bits: u16::MAX };
                let mut inputs = world(&mut random);
                inputs.objective_completion = Some(&mut completion);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                let actor = objects.get(owner).unwrap();
                assert!(matches!(
                    catalog.statement(actor.base.path.unwrap()).unwrap(),
                    Statement::Control(ControlCommand::End)
                ));
                let mut expected = before;
                expected.base.path = actor.base.path;
                expected.base.attack_power = attack & 0x0F;
                expected.base.flags.remove_after_tick = true;
                expected.extension.path_state.weapon_selection = weapon;
                expected.extension.path_state.script_parameter = (attack >> 4) + 2;
                expected.extension.path_state.script_value = 100;
                expected.extension.path_state.stack = actor.extension.path_state.stack.clone();
                if entry == authored_paths::NUMBERED_SPRITE_PURSUER {
                    expected.extension.path_state.motion_delta.x = i16::MIN;
                }
                assert_eq!(actor, &expected);
                assert_eq!(objects.len(), 1);
                assert_eq!(runtime.spawns.last_spawn, None);
                assert_eq!(completion.bits, u16::MAX);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn numbered_sprite_encounters_construct_their_child_before_live_transition_wait() {
    let catalog = authored_paths::catalog();
    for (entry, attack, offset, weapon) in [
        (authored_paths::NUMBERED_SPRITE_PURSUER, 60u8, -80, 18),
        (
            authored_paths::NUMBERED_SPRITE_BANKING_ATTACKER,
            80,
            -240,
            20,
        ),
    ] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        objects.get_mut(owner).unwrap().base.position.y = 32760;
        let mut completion = ObjectiveCompletion::default();
        let mut coordination = EncounterCoordination::default();
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.objective_completion = Some(&mut completion);
        inputs.coordination = Some(&mut coordination);
        inputs.scene.height_offset = Some(20);
        inputs.scene.entry_heading = Some(37);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        for _ in 0..3 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 128)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.len(), 2);
            let actor = objects.get(owner).unwrap();
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
            assert_eq!(actor.base.position.y, 32760i16.wrapping_add(20));
            assert_eq!(actor.base.yaw.units(), 37);
            assert_eq!(actor.extension.path_state.weapon_selection, weapon);
            assert_eq!(runtime.spawns.parameter, Some(attack));
            assert_eq!(runtime.spawns.companion_parameter, Some(10));
            assert_eq!(runtime.placement.primary, Some(offset));
            let child = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(child.base.child_number, 9);
            assert_eq!(child.base.attachment, Some(owner));
            assert_eq!(child.base.attack_power, attack);
            assert_eq!(child.extension.relative_position.z, offset);
            assert_eq!(child.base.path, Some(authored_paths::HIT_TOGGLE_SPRITE));
        }
        assert_eq!(random, before_random);
    }
}

#[test]
fn ordinary_objective_completion_counts_before_recording_and_resumes_without_double_count() {
    let catalog = called_catalog(authored_paths::COUNT_AND_RECORD_OBJECTIVE);
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(caller(0));
    actor.extension.path_state.script_parameter = 1;
    let mut counts = EncounterObjectiveCounts {
        recorded_completions: u16::MAX,
        signaled_completions: 32767,
        node_record: 0x21,
        remaining_word: 0xAB00,
    };
    let mut inputs = world(&mut random);
    inputs.objective_counts = Some(&mut counts);
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32),
        Err(ProgramError::MissingObjectiveCompletion)
    );
    assert_eq!(
        inputs
            .objective_counts
            .as_deref()
            .unwrap()
            .recorded_completions,
        0
    );
    assert_eq!(
        inputs.objective_counts.as_deref().unwrap().node_record,
        0x21
    );
    assert_eq!(
        inputs.objective_counts.as_deref().unwrap().remaining_word,
        0xAB00
    );
    let mut completion = ObjectiveCompletion::default();
    inputs.objective_completion = Some(&mut completion);
    assert_eq!(
        runtime
            .resume_program(&catalog, &mut objects, owner, &mut inputs, 32)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(
        counts,
        EncounterObjectiveCounts {
            recorded_completions: 0,
            signaled_completions: 32767,
            node_record: 0x20,
            remaining_word: 0xABFF
        }
    );
    assert_eq!(completion.bits, 1);
}
