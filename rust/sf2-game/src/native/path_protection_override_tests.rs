use super::super::path_protection::{PathProtection, ProtectionRules};
use super::super::PathId;
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn override_branch_reads_live_rules_without_linked_state_and_preserves_ifnot() {
    let catalog = PathCatalog::new(vec![vec![Statement::IfProtectionOverride {
        taken: at(2),
        next: at(1),
    }]])
    .unwrap();
    for enabled in [false, true] {
        for invert in [false, true] {
            for unrelated_flags in 0..8 {
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = invert;
                objects.get_mut(owner).unwrap().base.wait_timer = 197;
                let before = objects.clone();
                let before_random = random;
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::MissingProtection)
                );
                assert_eq!(objects, before);
                assert_eq!(runtime.branch.invert_next, invert);
                assert_eq!(random, before_random);
                let mut inputs = world(&mut random);
                inputs.protection = Some(PathProtection {
                    rules: ProtectionRules {
                        minimum_override: enabled,
                        special_character: unrelated_flags & 1 != 0,
                        blocked: unrelated_flags & 2 != 0,
                        contacts_enabled: unrelated_flags & 4 != 0,
                    },
                    linked: None,
                });
                for current in [enabled, !enabled, enabled] {
                    inputs.protection.as_mut().unwrap().rules.minimum_override = current;
                    objects.get_mut(owner).unwrap().base.path = Some(at(0));
                    let target = at(if current { 2 } else { 1 });
                    let mut expected = before.clone();
                    expected.get_mut(owner).unwrap().base.path = Some(target);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: target,
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(runtime.branch.invert_next, invert);
                    assert_eq!(*inputs.random, before_random);
                    assert_eq!(
                        inputs.protection.as_ref().unwrap().rules.minimum_override,
                        current
                    );
                }
            }
        }
    }
}
