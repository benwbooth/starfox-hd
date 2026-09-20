//! Static scenery placement and native authored lifetimes; no recorded input.
use super::super::collision_surface::{ActorSurfaceContact, SurfaceMode};
use super::super::path_fields::WordField;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId,
    Vector3,
};
use super::projectile_tests::{audio, callbacks};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn child_auxiliary_publication_uses_retained_spawn_and_preserves_all_other_state() {
    let catalog =
        PathCatalog::new(vec![vec![Statement::LinkLastSpawnToSelf { next: at(1) }]]).unwrap();
    for case in 0..4 {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let peer = objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap();
            let target = match case {
                0 => None,
                1 => Some(peer),
                2 => Some(owner),
                _ => {
                    objects.remove(peer);
                    Some(peer)
                }
            };
            objects.get_mut(owner).unwrap().base.attachment = Some(owner);
            objects.get_mut(owner).unwrap().base.linked_object = Some(owner);
            runtime.spawns.last_spawn = target;
            runtime.branch.invert_next = invert;
            let before = objects.clone();
            let before_random = random;
            let result =
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
            let mut expected = before;
            match case {
                0 => assert_eq!(
                    result,
                    Err(ProgramError::ActorContext(
                        super::super::path_actor_context::ActorContextError::MissingLastSpawn
                    ))
                ),
                3 => assert_eq!(
                    result,
                    Err(ProgramError::Runtime(PathRuntimeError::MissingActor(peer)))
                ),
                _ => {
                    assert_eq!(
                        result,
                        Err(ProgramError::BudgetExceeded {
                            cursor: at(1),
                            executed: 1
                        })
                    );
                    expected
                        .get_mut(target.unwrap())
                        .unwrap()
                        .base
                        .linked_object = Some(owner);
                    expected.get_mut(owner).unwrap().base.path = Some(at(1));
                }
            }
            assert_eq!(objects, expected);
            assert_eq!(random, before_random);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(runtime.spawns.last_spawn, target);
        }
    }
}

#[test]
fn attachment_auxiliary_swap_preserves_chain_pose_gates_and_pending_branch() {
    use super::super::path_relationships::RelationshipCommand;
    let catalog = PathCatalog::new(vec![vec![
        Statement::Relationship {
            command: RelationshipCommand::SwapAttachmentAndAuxiliary,
            next: at(1),
        },
        Statement::Relationship {
            command: RelationshipCommand::SwapAttachmentAndAuxiliary,
            next: at(2),
        },
    ]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let peer = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    let stale = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Effect,
        ))
        .unwrap();
    objects.remove(stale);
    for attachment in [None, Some(owner), Some(peer), Some(stale)] {
        for auxiliary in [None, Some(owner), Some(peer), Some(stale)] {
            for flags in 0..8 {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.attachment = attachment;
                actor.base.linked_object = auxiliary;
                actor.base.path = Some(at(0));
                actor.extension.path_state.motion.attached_coordinates = flags & 1 != 0;
                actor.extension.path_state.motion.relative_coordinates = flags & 2 != 0;
                runtime.branch.invert_next = flags & 4 != 0;
                let before = objects.clone();
                let before_random = random;
                let mut expected = before.clone();
                expected.get_mut(owner).unwrap().base.attachment = auxiliary;
                expected.get_mut(owner).unwrap().base.linked_object = attachment;
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                expected = before;
                expected.get_mut(owner).unwrap().base.path = Some(at(2));
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(2),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(random, before_random);
                assert_eq!(runtime.branch.invert_next, flags & 4 != 0);
            }
        }
    }
}

#[test]
fn placement_height_is_shared_and_preserves_every_signed_word_and_other_actor_fields() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before = objects.clone();
    let before_random = random;
    let read = PathCatalog::new(vec![vec![Statement::ImportSceneryPlacementHeight {
        next: at(1),
    }]])
    .unwrap();
    assert_eq!(
        runtime.resume_program(&read, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingSceneryPlacementHeight)
    );
    assert_eq!(objects, before);
    for bits in 0..=u16::MAX {
        let height = bits as i16;
        let catalog = PathCatalog::new(vec![vec![
            Statement::SetSceneryPlacementHeight {
                height,
                next: at(1),
            },
            Statement::ImportSceneryPlacementHeight { next: at(2) },
        ]])
        .unwrap();
        objects = before.clone();
        runtime.branch.invert_next = bits & 1 != 0;
        let mut expected = objects.clone();
        expected.get_mut(owner).unwrap().base.position.y = height;
        expected.get_mut(owner).unwrap().base.path = Some(at(2));
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Err(ProgramError::BudgetExceeded {
                cursor: at(2),
                executed: 2
            })
        );
        assert_eq!(objects, expected);
        assert_eq!(runtime.placement.lateral_or_height, Some(height));
        assert_eq!(runtime.branch.invert_next, bits & 1 != 0);
    }
    assert_eq!(random, before_random);
}

#[test]
fn last_spawn_attachment_and_deferred_removal_preserve_paths_and_do_not_end_execution() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::AttachLastSpawn { next: at(1) },
        Statement::MarkRemoval { next: at(2) },
    ]])
    .unwrap();
    for case in 0..4 {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let peer = objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Effect,
                ))
                .unwrap();
            objects.get_mut(owner).unwrap().base.attachment = Some(peer);
            let target = match case {
                0 => None,
                1 => Some(peer),
                2 => Some(owner),
                _ => {
                    objects.remove(peer);
                    Some(peer)
                }
            };
            runtime.spawns.last_spawn = target;
            runtime.branch.invert_next = invert;
            let before = objects.clone();
            let before_random = random;
            let result =
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2);
            if case == 3 {
                assert_eq!(
                    result,
                    Err(ProgramError::Runtime(PathRuntimeError::MissingActor(peer)))
                );
                assert_eq!(objects, before);
            } else {
                assert_eq!(
                    result,
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(2),
                        executed: 2
                    })
                );
                let mut expected = before;
                let actor = expected.get_mut(owner).unwrap();
                actor.base.attachment = target;
                actor.base.flags.remove_after_tick = true;
                actor.base.path = Some(at(2));
                assert_eq!(objects, expected);
            }
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn direct_surface_query_imports_height_not_identity_and_replaces_all_contact_outputs() {
    let catalog = PathCatalog::new(vec![vec![Statement::QuerySurfaceHeight {
        destination: WordField::ScriptValue,
        next: at(1),
    }]])
    .unwrap();
    for mode in 0..=u8::MAX {
        for with_surface in [false, true] {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let mut ground = Object::new(
                    ObjectKind::Scenery,
                    ShapeId::from_catalog_index(156),
                    Behavior::Effect,
                );
                ground.base.contacts.first_strategy_visit = !with_surface;
                let ground = objects.allocate(ground).unwrap();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.position.y = -403;
                actor.extension.surface_contact = ActorSurfaceContact {
                    supporting_object: Some(owner),
                    group: 233,
                    flags: 199,
                };
                let expected_query = super::super::collision_surface::query_object_surface(
                    &objects,
                    owner,
                    0,
                    SurfaceMode { flags: mode }.search(),
                )
                .unwrap();
                if !with_surface {
                    assert_eq!(
                        expected_query.height,
                        if mode & 7 == 0 { 16_384 } else { 0 }
                    );
                    assert_eq!(expected_query.contact.supporting_object, None);
                } else {
                    assert_eq!(expected_query.contact.supporting_object, Some(ground));
                }
                let before = objects.clone();
                runtime.branch.invert_next = invert;
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::MissingSurfaceMode)
                );
                assert_eq!(objects, before);
                let mut inputs = world(&mut random);
                inputs.surface_mode = Some(SurfaceMode { flags: mode });
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                let mut expected = before;
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(at(1));
                actor.extension.surface_contact = expected_query.contact;
                actor.extension.path_state.script_value = expected_query.height as u16;
                assert_eq!(objects, expected);
                assert_eq!(runtime.branch.invert_next, invert);
            }
        }
    }
}

#[test]
fn selected_particle_mask_ors_every_byte_without_touching_action_flags_mode_or_ifnot() {
    for mask in 0..=u8::MAX {
        let catalog = PathCatalog::new(vec![vec![Statement::IncludeSelectedParticleFlags {
            mask,
            next: at(0),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        for flags in 0..=u8::MAX {
            runtime.branch.invert_next = flags & 1 != 0;
            let mut auxiliary = SelectedAuxiliaryState {
                stored_rotation: Default::default(),
                stored_world_position: Default::default(),
                mode: !flags,
                action_flags: flags,
            };
            let mut particles = SelectedParticleEffects { flags };
            let before = objects.clone();
            let random_before = random;
            let mut inputs = world(&mut random);
            inputs.selected_auxiliary = Some(&mut auxiliary);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::MissingSelectedParticleEffects)
            );
            inputs.selected_particle_effects = Some(&mut particles);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(0),
                    executed: 1
                })
            );
            assert_eq!(
                auxiliary,
                SelectedAuxiliaryState {
                    stored_rotation: Default::default(),
                    stored_world_position: Default::default(),
                    mode: !flags,
                    action_flags: flags
                }
            );
            assert_eq!(particles.flags, flags | mask);
            assert_eq!(objects, before);
            assert_eq!(random, random_before);
            assert_eq!(runtime.branch.invert_next, flags & 1 != 0);
        }
    }
}

#[test]
fn arc_emitter_repeats_with_selected_jitter_and_retains_the_fresh_attachment() {
    let catalog = authored_paths::catalog();
    for seed in 0..=u8::MAX {
        for mode in [0, 1, 8, 255] {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([seed, 31, 171, 71]);
            let mut expected_random = random;
            let mut selected =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            selected.base.position = Vector3 {
                x: i16::MAX,
                y: 317,
                z: i16::MIN,
            };
            selected.base.yaw = Angle::from_units(seed);
            let selected_position = selected.base.position;
            let selected = objects.allocate(selected).unwrap();
            let selected_before = objects.get(selected).unwrap().clone();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::SELECTED_SCENERY_ARC_EMITTER);
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.surface_mode = Some(SurfaceMode { flags: mode });
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.audio = Some(audio(&mut events));
            let mut previous = None;
            for visit in 1..=27 {
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100),
                    Ok(ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    })
                );
                if visit % 9 == 0 {
                    let yaw = expected_random.next_byte();
                    let dx = i16::from(expected_random.next_byte() as i8).wrapping_mul(6);
                    let dz = i16::from(expected_random.next_byte() as i8).wrapping_mul(6);
                    let spawned = runtime.spawns.last_spawn.unwrap();
                    assert_ne!(previous, Some(spawned));
                    let actor = objects.get(spawned).unwrap();
                    assert_eq!(
                        actor.base.path,
                        Some(authored_paths::HEIGHT_SELECTED_ARC_EFFECT)
                    );
                    assert_eq!(
                        actor.base.position,
                        Vector3 {
                            x: selected_position.x.wrapping_add(dx),
                            y: -500,
                            z: selected_position.z.wrapping_add(dz)
                        }
                    );
                    assert_eq!(actor.base.yaw, Angle::from_units(yaw));
                    assert_eq!(
                        actor.extension.path_state.motion_phase as u8,
                        dz.wrapping_div(6) as u8
                    );
                    assert_eq!(actor.base.hit_points, 10);
                    assert_eq!(actor.base.attack_power, 10);
                    let bearing = (sf_core::aim_angle::sf2_atan16(dx, dz) >> 8) as u8;
                    let bypassed_probe = bearing.wrapping_add(seed).wrapping_add(32) < 64;
                    let rejected = !bypassed_probe && mode & 7 == 0;
                    assert_eq!(actor.base.flags.remove_after_tick, rejected);
                    assert_eq!(
                        actor.extension.path_state.script_value,
                        if rejected { 16_384 } else { 0 }
                    );
                    assert_eq!(objects.get(owner).unwrap().base.attachment, Some(spawned));
                    assert_eq!(runtime.placement.lateral_or_height, Some(-500));
                    previous = Some(spawned);
                }
                assert_eq!(inputs.random, &expected_random);
                assert_eq!(objects.get(selected), Some(&selected_before));
                assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
            }
        }
    }
}

#[test]
fn sprite_emitter_runs_its_own_fallthrough_and_finishes_after_the_authored_waits() {
    let catalog = authored_paths::catalog();
    for initial_y in [-941, -940, -500, 0, i16::MIN, i16::MAX] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let selected = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::SELECTED_SCENERY_SPRITE_EMITTER);
        actor.base.position.y = initial_y;
        let mut events = AudioState::default();
        let mut auxiliary = SelectedAuxiliaryState {
            stored_rotation: Default::default(),
            stored_world_position: Default::default(),
            mode: 143,
            action_flags: 0,
        };
        let mut particles = SelectedParticleEffects::default();
        let mut inputs = world(&mut random);
        inputs.selected = Some(selected);
        inputs.surface_mode = Some(SurfaceMode { flags: 1 });
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.audio = Some(audio(&mut events));
        inputs.selected_auxiliary = Some(&mut auxiliary);
        inputs.selected_particle_effects = Some(&mut particles);
        for visit in 1..=25 {
            if visit >= 18 {
                assert_eq!(
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    1
                );
            }
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100),
                Ok(ProgramExit {
                    actor: owner,
                    step: if visit == 25 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                })
            );
            if visit == 17 {
                let spawned = runtime.spawns.last_spawn.unwrap();
                assert_eq!(objects.get(spawned).unwrap().base.position.y, 0);
                assert_eq!(
                    objects.get(owner).unwrap().base.position.y,
                    initial_y.wrapping_sub(60)
                );
                assert_eq!(objects.get(owner).unwrap().base.attachment, None);
            }
        }
        let y = initial_y.wrapping_sub(60);
        let skipped_second = y.wrapping_sub(-1000) > 0 && y.wrapping_sub(-500) <= 0;
        assert_eq!(objects.len(), if skipped_second { 3 } else { 4 });
        assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
        assert_eq!(auxiliary.mode, 143);
        assert_eq!(auxiliary.action_flags, 0);
        assert_eq!(
            particles.flags,
            if y.unsigned_abs() <= 80 { 0x20 } else { 0 }
        );
    }
}

#[test]
fn full_pool_reuses_retained_last_spawn_and_null_selection_faults_after_admission() {
    use super::super::{path_actor_context::ActorContextError, OBJECT_CAPACITY};
    let catalog = authored_paths::catalog();
    for root in [
        authored_paths::SELECTED_SCENERY_ARC_EMITTER,
        authored_paths::SELECTED_SCENERY_SPRITE_EMITTER,
    ] {
        for retained in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let selected = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let mut borrowed = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
            borrowed.base.path = Some(authored_paths::FADE_SPRITE);
            borrowed.base.position = Vector3 {
                x: 191,
                y: 511,
                z: -117,
            };
            let borrowed = objects.allocate(borrowed).unwrap();
            while objects.len() < OBJECT_CAPACITY {
                objects
                    .allocate(Object::new(
                        ObjectKind::Effect,
                        ShapeId::EMPTY,
                        Behavior::Effect,
                    ))
                    .unwrap();
            }
            runtime.spawns.last_spawn = retained.then_some(borrowed);
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            // Enter on the wait's terminal visit, preserving native init.
            actor.base.wait_timer = if root == authored_paths::SELECTED_SCENERY_ARC_EMITTER {
                8
            } else {
                16
            };
            let before = objects.clone();
            let mut expected_random = random;
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.surface_mode = Some(SurfaceMode { flags: 1 });
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.audio = Some(audio(&mut events));
            let result = runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100);
            if retained {
                let yaw = expected_random.next_byte();
                let dx = i16::from(expected_random.next_byte() as i8) * 6;
                let dz = i16::from(expected_random.next_byte() as i8) * 6;
                assert_eq!(
                    result,
                    Ok(ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    })
                );
                let actor = objects.get(borrowed).unwrap();
                assert_eq!(actor.base.path, before.get(borrowed).unwrap().base.path);
                assert_eq!(
                    actor.base.position,
                    Vector3 {
                        x: dx,
                        y: if root == authored_paths::SELECTED_SCENERY_ARC_EMITTER {
                            -500
                        } else {
                            0
                        },
                        z: dz
                    }
                );
                assert_eq!(actor.base.yaw, Angle::from_units(yaw));
            } else {
                assert_eq!(
                    result,
                    Err(ProgramError::ActorContext(
                        ActorContextError::MissingLastSpawn
                    ))
                );
                assert_eq!(objects.get(borrowed), before.get(borrowed));
            }
            assert_eq!(objects.len(), OBJECT_CAPACITY);
            for &id in objects.active_ids() {
                if id != owner && id != borrowed {
                    assert_eq!(objects.get(id), before.get(id));
                }
            }
            assert_eq!(runtime.spawns.last_spawn, retained.then_some(borrowed));
            assert_eq!(inputs.random, &expected_random);
            assert_eq!(
                objects.get(owner).unwrap().base.attachment,
                if root == authored_paths::SELECTED_SCENERY_ARC_EMITTER {
                    retained.then_some(borrowed)
                } else {
                    None
                }
            );
        }
    }
}
