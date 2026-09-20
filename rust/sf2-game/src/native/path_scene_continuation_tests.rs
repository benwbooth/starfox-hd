use super::super::scene_proxy::SceneProxyStore;
use super::super::{authored_paths, PathId};
use super::tests::{setup, world};
use super::*;

fn retained(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(7),
        command_index: index,
    }
}

#[test]
fn authored_scene_retention_updates_only_proxy_or_fallback_before_ending() {
    let catalog = authored_paths::catalog();
    let entry = authored_paths::RETAIN_SCENE_CONTINUATION_AND_END;
    for linked in [false, true] {
        for old in [None, Some(retained(0)), Some(retained(u16::MAX))] {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let mut proxies = SceneProxyStore::default();
                let id = if linked {
                    proxies
                        .capture_actor(&mut objects, owner, retained(2))
                        .unwrap()
                } else {
                    None
                };
                objects.get_mut(owner).unwrap().extension.scene_continuation = old;
                objects.get_mut(owner).unwrap().base.path = Some(entry);
                objects.get_mut(owner).unwrap().base.wait_timer = 137;
                let mut expected = objects.clone();
                let next = PathCursor {
                    command_index: entry.command_index + 1,
                    ..entry
                };
                expected.get_mut(owner).unwrap().base.path = Some(next);
                if !linked {
                    expected
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .scene_continuation = Some(entry);
                }
                let mut expected_proxies = proxies.clone();
                if let Some(id) = id {
                    expected_proxies.get_mut(id).unwrap().continuation = entry;
                }
                runtime.branch.invert_next = invert;
                let before_random = random;
                let mut inputs = world(&mut random);
                if linked {
                    inputs.scene_proxies = Some(&mut proxies);
                }
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: next,
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(proxies, expected_proxies);
                assert_eq!(runtime.branch.invert_next, invert);
                assert_eq!(random, before_random);
                // END does not substitute RETURN or immediately erase the retained path.
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                assert_eq!(
                    objects.get(owner).unwrap().extension.scene_continuation,
                    if linked { old } else { Some(entry) }
                );
                runtime.release_actor_programs(&mut objects, owner).unwrap();
                assert_eq!(
                    objects.get(owner).unwrap().extension.scene_continuation,
                    None
                );
                assert_eq!(proxies, expected_proxies);
            }
        }
    }
}

#[test]
fn retained_fallback_is_live_input_to_later_capture_and_is_not_consumed_or_overwritten_by_proxy_branch(
) {
    let catalog = authored_paths::catalog();
    let entry = authored_paths::RETAIN_SCENE_CONTINUATION_AND_END;
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(entry);
    assert!(matches!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::BudgetExceeded { .. })
    ));
    let mut proxies = SceneProxyStore::default();
    let id = proxies
        .capture_actor(&mut objects, owner, retained(42))
        .unwrap()
        .unwrap();
    assert_eq!(proxies.get(id).unwrap().continuation, entry);
    assert_eq!(
        objects.get(owner).unwrap().extension.scene_continuation,
        Some(entry)
    );
    // A different decoded path instruction must retain its own identity,
    // without replacing the already-retained fallback behind a live proxy.
    let next = retained(1);
    let another =
        PathCatalog::new(vec![vec![Statement::PreserveSceneContinuation { next }]]).unwrap();
    let elsewhere = PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: 0,
    };
    objects.get_mut(owner).unwrap().base.path = Some(elsewhere);
    let mut inputs = world(&mut random);
    inputs.scene_proxies = Some(&mut proxies);
    assert_eq!(
        runtime.resume_program(&another, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: next,
            executed: 1
        })
    );
    assert_eq!(proxies.get(id).unwrap().continuation, elsewhere);
    assert_eq!(
        objects.get(owner).unwrap().extension.scene_continuation,
        Some(entry)
    );
    proxies.release_actor_proxy(&mut objects, owner).unwrap();
    let id = proxies
        .capture_actor(&mut objects, owner, retained(100))
        .unwrap()
        .unwrap();
    assert_eq!(proxies.get(id).unwrap().continuation, entry);
}

#[test]
fn scene_continuation_missing_proxy_inputs_do_not_fall_back_or_advance() {
    let catalog = authored_paths::catalog();
    let entry = authored_paths::RETAIN_SCENE_CONTINUATION_AND_END;
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(entry);
    objects.get_mut(owner).unwrap().extension.scene_continuation = Some(retained(65535));
    let mut proxies = SceneProxyStore::default();
    let id = proxies
        .capture_actor(&mut objects, owner, retained(4))
        .unwrap()
        .unwrap();
    let proxy = proxies.release(id).unwrap();
    for store_present in [false, true] {
        let before = objects.clone();
        let before_random = random;
        let before_proxies = proxies.clone();
        let mut inputs = world(&mut random);
        if store_present {
            inputs.scene_proxies = Some(&mut proxies);
        }
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(if store_present {
                ProgramError::MissingSceneProxy(id)
            } else {
                ProgramError::MissingSceneProxies
            })
        );
        assert_eq!(objects, before);
        assert_eq!(proxies, before_proxies);
        assert_eq!(random, before_random);
    }
    assert_eq!(proxies.allocate(proxy), Some(id));
    let mut inputs = world(&mut random);
    inputs.scene_proxies = Some(&mut proxies);
    assert!(matches!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded { .. })
    ));
    assert_eq!(proxies.get(id).unwrap().continuation, entry);
    assert_eq!(
        objects.get(owner).unwrap().extension.scene_continuation,
        Some(retained(65535))
    );
}
