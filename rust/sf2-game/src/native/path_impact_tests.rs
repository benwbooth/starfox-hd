use super::super::collision_contacts::ContactStore;
use super::super::path_contact::ContactCommand;
use super::super::path_fields::ByteField;
use super::super::path_impact::ImpactState;
use super::super::{Behavior, ObjectKind, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn material_commands_replace_only_their_optional_record_and_import_preserves_other_state() {
    for importing in [false, true] {
        for suppressed in [false, true] {
            for value in 0..=u8::MAX {
                let statement = if importing {
                    Statement::ImportImpactMaterial {
                        destination: ByteField::Part,
                        next: at(1),
                    }
                } else {
                    Statement::Contact {
                        command: if suppressed {
                            ContactCommand::SuppressedImpactMaterial(value)
                        } else {
                            ContactCommand::OrdinaryImpactMaterial(value)
                        },
                        next: at(1),
                    }
                };
                let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.impact_materials.ordinary = Some(!value);
                actor.extension.impact_materials.suppressed = Some(!value);
                actor.base.wait_timer = 79;
                actor.extension.path_state.part = !value;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(at(1));
                if importing {
                    actor.extension.path_state.part = value;
                } else if suppressed {
                    actor.extension.impact_materials.suppressed = Some(value);
                } else {
                    actor.extension.impact_materials.ordinary = Some(value);
                }
                let mut impact = ImpactState {
                    material: value,
                    pair_suppressed: true,
                };
                let before_random = random;
                let mut inputs = world(&mut random);
                inputs.impact = Some(&mut impact);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(
                    impact,
                    ImpactState {
                        material: value,
                        pair_suppressed: true
                    }
                );
                assert!(runtime.branch.invert_next);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn impact_dispatch_selects_each_edge_immediately_without_consuming_ifnot_or_randomness() {
    let catalog = PathCatalog::new(vec![vec![Statement::ImpactBranch {
        first: at(1),
        second: at(2),
        third: at(3),
        next: at(4),
    }]])
    .unwrap();
    for expected_edge in 1..=4 {
        for inverted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let peer = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .unwrap();
            let target = objects.get_mut(peer).unwrap();
            target.base.flags.exclude_from_shape_footprint_search = true;
            target.base.hit_points = if expected_edge == 1 { 0 } else { 255 };
            target.base.contacts.suppress_contacts_next_epoch = expected_edge == 3;
            target.extension.impact_materials.ordinary = Some(42);
            target.extension.impact_materials.suppressed = Some(51);
            objects.get_mut(owner).unwrap().base.contacts.pending_hit = expected_edge != 4;
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, peer, [None; 2]).unwrap();
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(at(expected_edge));
            let mut impact = ImpactState {
                material: 231,
                pair_suppressed: true,
            };
            let before_random = random;
            let mut inputs = world(&mut random);
            inputs.impact = Some(&mut impact);
            inputs.contacts = Some(&contacts);
            inputs.surface_mode = Some(Default::default());
            runtime.branch.invert_next = inverted;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(expected_edge),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(
                impact.material,
                match expected_edge {
                    2 => 42,
                    3 => 51,
                    _ => 0,
                }
            );
            assert_eq!(impact.pair_suppressed, expected_edge >= 3);
            assert_eq!(runtime.branch.invert_next, inverted);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn impact_operations_require_explicit_shared_state_before_mutation() {
    for statement in [
        Statement::ImpactBranch {
            first: at(1),
            second: at(2),
            third: at(3),
            next: at(4),
        },
        Statement::ImportImpactMaterial {
            destination: ByteField::Part,
            next: at(1),
        },
    ] {
        let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingImpactState)
        );
        assert_eq!(objects, before);
    }
}
