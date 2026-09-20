use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_target::{PrimaryTarget, TargetAnchor, TargetSelection};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::scene_proxy::{SceneProxyFlags, SceneProxyStore};
use super::super::{authored_paths, Vector3};
use super::tests::{setup, world};
use super::*;

fn anchor() -> TargetAnchor {
    TargetAnchor {
        position: Vector3::default(),
        pitch: 0,
        yaw: 0,
    }
}

#[test]
fn target_proxy_callback_preserves_request_bits_and_marks_even_locked_or_rejected_candidates() {
    let catalog = authored_paths::catalog();
    for bits in 0..=u8::MAX {
        for flags in [0, 8, 0x10, 0x18, 0xE7, 0xFF] {
            for mode in 0..3 {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let entry = authored_paths::CONSIDER_TARGET_AND_MARK_SCENE_PROXY;
                let mut proxies = SceneProxyStore::default();
                objects.get_mut(owner).unwrap().base.position.z = 100;
                objects.get_mut(owner).unwrap().base.wait_timer = bits;
                let id = proxies
                    .capture_actor(&mut objects, owner, entry)
                    .unwrap()
                    .unwrap();
                proxies.get_mut(id).unwrap().flags = SceneProxyFlags::from_authored_bits(bits);
                // An unrelated proxy snapshot must not be marked.
                proxies.allocate(*proxies.get(id).unwrap()).unwrap();
                let mut expected_proxies = proxies.clone();
                expected_proxies.get_mut(id).unwrap().flags =
                    SceneProxyFlags::from_authored_bits(bits | 0x20);
                runtime
                    .add_trigger(
                        &mut objects,
                        owner,
                        Trigger {
                            kind: TriggerKind::Always,
                            path: entry,
                            timer: 0,
                        },
                    )
                    .unwrap();
                let interrupted_path = objects.get(owner).unwrap().base.path;
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert!(matches!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Run(_))
                ));
                let mut selection = TargetSelection {
                    distance: if mode == 1 { 0 } else { u16::MAX },
                    display_status: 255,
                    control_flags: flags,
                    forced_owner: (mode == 2).then_some(owner),
                    ..TargetSelection::default()
                };
                let mut expected = selection;
                if flags & 0x10 == 0 {
                    expected.display_status = 127;
                    if mode != 1 {
                        expected.control_flags = flags | if mode == 2 { 0x10 } else { 0 };
                        expected.candidate = Some(owner);
                        expected.distance = 56;
                        expected.auxiliary_distance = 100;
                        expected.position.z = 100;
                        expected.screen = [112, 96];
                    }
                }
                let before = objects.clone();
                let before_random = random;
                runtime.branch.invert_next = bits & 1 != 0;
                let mut inputs = world(&mut random);
                inputs.primary_player = Some(owner);
                inputs.primary_target = Some(PrimaryTarget {
                    anchor: anchor(),
                    selection: &mut selection,
                });
                inputs.scene_proxies = Some(&mut proxies);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 2)
                        .unwrap()
                        .step,
                    ControlStep::ResumeCallbacks
                );
                assert_eq!(selection, expected);
                assert_eq!(proxies, expected_proxies);
                // Return resumes the callback runner; completing that pass
                // restores the interrupted actor path.
                assert_eq!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Complete)
                );
                let after = objects.get(owner).unwrap();
                let old = before.get(owner).unwrap();
                let mut expected_base = old.base.clone();
                expected_base.path = interrupted_path;
                assert_eq!(after.base, expected_base);
                assert_eq!(after.extension, old.extension);
                assert_eq!(runtime.branch.invert_next, bits & 1 != 0);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn target_proxy_null_link_needs_no_store_and_preserves_all_control_flag_bytes() {
    let catalog = authored_paths::catalog();
    for flags in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let entry = authored_paths::CONSIDER_TARGET_AND_MARK_SCENE_PROXY;
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        let mut selection = TargetSelection {
            control_flags: flags,
            distance: u16::MAX,
            ..TargetSelection::default()
        };
        let mut inputs = world(&mut random);
        inputs.primary_player = Some(owner);
        inputs.primary_target = Some(PrimaryTarget {
            anchor: anchor(),
            selection: &mut selection,
        });
        assert!(matches!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded { executed: 1, .. })
        ));
        assert_eq!(selection.control_flags, flags);
        assert_eq!(
            selection.candidate,
            if flags & 0x10 == 0 { Some(owner) } else { None }
        );
    }
}

#[test]
fn target_proxy_missing_services_and_dangling_handle_are_retryable_before_any_mutation() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let entry = authored_paths::CONSIDER_TARGET_AND_MARK_SCENE_PROXY;
    objects.get_mut(owner).unwrap().base.path = Some(entry);
    let mut proxies = SceneProxyStore::default();
    let id = proxies
        .capture_actor(&mut objects, owner, entry)
        .unwrap()
        .unwrap();
    let retained = proxies.release(id).unwrap();
    let mut selection = TargetSelection {
        forced_owner: Some(owner),
        distance: u16::MAX,
        ..TargetSelection::default()
    };
    for store_present in [false, true] {
        let before = objects.clone();
        let before_selection = selection;
        let before_proxies = proxies.clone();
        let mut inputs = world(&mut random);
        inputs.primary_player = Some(owner);
        inputs.primary_target = Some(PrimaryTarget {
            anchor: anchor(),
            selection: &mut selection,
        });
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
        assert_eq!(selection, before_selection);
        assert_eq!(proxies, before_proxies);
    }
    assert_eq!(proxies.allocate(retained), Some(id));
    let mut inputs = world(&mut random);
    inputs.primary_player = Some(owner);
    inputs.primary_target = Some(PrimaryTarget {
        anchor: anchor(),
        selection: &mut selection,
    });
    inputs.scene_proxies = Some(&mut proxies);
    assert!(matches!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded { executed: 1, .. })
    ));
    assert_eq!(selection.control_flags, 0x10);
    assert_eq!(selection.candidate, Some(owner));
    assert_eq!(
        proxies.get(id).unwrap().flags.authored_bits(),
        retained.flags.authored_bits() | 0x20
    );
}
