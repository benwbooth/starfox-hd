use super::*;
use crate::cinematic_exit::{
    CinematicExit, CinematicExitPolicy, CinematicSignals, CinematicSkipPolicy,
};
use crate::player_action_wait::ActionWaitPhase;
use crate::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
use crate::strategy_schedule::StrategySchedule;

const ACTIONS: [(AuthoredSceneAction, Option<u16>); 5] = [
    (AuthoredSceneAction::Scene3, Some(180)),
    (AuthoredSceneAction::Scene4, Some(144)),
    (AuthoredSceneAction::Scene5, Some(227)),
    (AuthoredSceneAction::Scene9, None),
    (AuthoredSceneAction::Scene25, Some(124)),
];

fn scene(action: AuthoredSceneAction) -> Scene {
    let mut scene = Scene::new();
    scene.action().install(PlayerAction::Scene(action));
    scene.world.cinematic_signals = Some(CinematicSignals {
        exit_requested: false,
        skip_ready: true,
    });
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .camera_dispatch = Some(Default::default());
    scene
}

#[test]
fn scene_actions_check_every_elapsed_word_without_stopping_or_resetting_companion_time() {
    for (action, exit_time) in ACTIONS {
        let mut scene = scene(action);
        for elapsed in 0..=u16::MAX {
            *scene.action() = PlayerActionState {
                action: Some(PlayerAction::Scene(action)),
                elapsed,
                auxiliary_counter: elapsed.rotate_left(9),
                total_updates: u16::MAX,
            };
            scene
                .world
                .cinematic_signals
                .as_mut()
                .unwrap()
                .exit_requested = false;
            scene
                .world
                .player_mut(&scene.objects, scene.owner)
                .unwrap()
                .camera_dispatch
                .as_mut()
                .unwrap()
                .projection_correction_disabled = false;
            scene.visit(true).unwrap();
            assert_eq!(
                scene.world.cinematic_signals.unwrap(),
                CinematicSignals {
                    exit_requested: Some(elapsed) == exit_time,
                    skip_ready: true,
                }
            );
            assert_eq!(
                scene
                    .world
                    .player(&scene.objects, scene.owner)
                    .unwrap()
                    .camera_dispatch
                    .unwrap()
                    .projection_correction_disabled,
                action == AuthoredSceneAction::Scene5 && elapsed == 0
            );
            assert_eq!(
                *scene.action(),
                PlayerActionState {
                    action: Some(PlayerAction::Scene(action)),
                    elapsed: elapsed.saturating_add(1),
                    auxiliary_counter: elapsed.rotate_left(9),
                    total_updates: if elapsed == u16::MAX { u16::MAX } else { 0 },
                }
            );
        }
    }
}

#[test]
fn non_null_empty_scene_advances_but_absent_action_does_not_and_pause_borrows_no_services() {
    let mut scene = scene(AuthoredSceneAction::Scene9);
    scene.world.cinematic_signals = None;
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .camera_dispatch = None;
    scene.action().auxiliary_counter = 567;
    scene.visit(false).unwrap();
    assert_eq!(scene.action().elapsed, 1);
    assert_eq!(scene.action().total_updates, 78);
    assert_eq!(scene.action().auxiliary_counter, 567);
    scene.action().action = None;
    let inactive = *scene.action();
    scene.visit(false).unwrap();
    assert_eq!(*scene.action(), inactive);
    scene
        .action()
        .install(PlayerAction::Scene(AuthoredSceneAction::Scene5));
    scene.world.view_transition_mode =
        Some(crate::view_transition::ViewTransitionMode { flags: 2 });
    let paused = *scene.action();
    scene.visit(true).unwrap();
    assert_eq!(*scene.action(), paused);
    scene.world.view_transition_mode.as_mut().unwrap().flags = 0;
    assert_eq!(
        scene.visit(false),
        Err(SceneError::PlayerAction(
            PlayerActionError::MissingCameraDispatch(scene.owner)
        ))
    );
    assert_eq!(*scene.action(), paused);
    assert!(scene.execution.is_faulted());
}

fn display() -> SceneDisplay {
    SceneDisplay {
        request: FadeRequest::Idle,
        progress: Intensity::FULL,
        bands: [DisplayBand {
            blanked: false,
            intensity: Intensity::FULL,
        }; 3],
        blank_hold: 255,
        interval_remaining: 0,
        interval_reload: 0,
    }
}

#[test]
fn scheduled_scene_action_requests_outer_exit_and_only_display_work_completes_the_fade() {
    for (action, exit_time) in ACTIONS.into_iter().filter(|(_, at)| at.is_some()) {
        let mut scene = scene(action);
        scene.objects.get_mut(scene.owner).unwrap().base.behavior =
            Behavior::PlayerActionWait(ActionWaitPhase::Active);
        scene.objects.get_mut(scene.owner).unwrap().base.hit_points = 1;
        scene.world.controller_inputs[0] = Some(InputState::default());
        scene.execution.controls.positional_suppressed = true;
        let mut schedule = StrategySchedule::default();
        let mut exit = CinematicExit::new(0);
        let mut display = display();
        let policy = CinematicExitPolicy {
            initial_intensity: Intensity::new(8),
            skip: CinematicSkipPolicy::Disabled,
        };
        let catalog = PathCatalog::new(vec![]).unwrap();
        let mut callbacks = Callbacks;
        let exit_time = exit_time.unwrap();
        for visit in 0..=exit_time {
            schedule
                .begin::<SceneError<&'static str>>(&scene.objects)
                .unwrap();
            let mut host = SceneActors {
                objects: &mut scene.objects,
                world: &mut scene.world,
                execution: &mut scene.execution,
                catalog: &catalog,
                callbacks: &mut callbacks,
                statement_budget: 10,
            };
            schedule
                .run_overlapping(&mut host, || visit & 1 != 0)
                .unwrap();
            schedule.run_remainder(&mut host).unwrap();
            let event = host
                .visit_cinematic_exit(&mut exit, policy, Buttons::default(), false, &mut display)
                .unwrap();
            assert!(!event.completed);
            assert_eq!(event.request_audio_exit, visit == exit_time);
            assert_eq!(
                display.progress,
                if visit == exit_time {
                    Intensity::new(8)
                } else {
                    Intensity::FULL
                }
            );
        }
        let retained_action = *scene.action();
        let mut host = SceneActors {
            objects: &mut scene.objects,
            world: &mut scene.world,
            execution: &mut scene.execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 10,
        };
        for _ in 0..10 {
            assert!(
                !host
                    .visit_cinematic_exit(
                        &mut exit,
                        policy,
                        Buttons::default(),
                        false,
                        &mut display
                    )
                    .unwrap()
                    .completed
            );
            assert_eq!(display.progress, Intensity::new(8));
        }
        for remaining in [6, 4, 2, 0] {
            display.visit_scene_fade(false);
            assert_eq!(display.progress, Intensity::new(remaining));
            assert_eq!(
                host.visit_cinematic_exit(
                    &mut exit,
                    policy,
                    Buttons::default(),
                    false,
                    &mut display
                )
                .unwrap()
                .completed,
                remaining == 0
            );
        }
        assert!(exit.completed());
        assert_eq!(
            host.world.cinematic_signals,
            Some(CinematicSignals::default())
        );
        assert_eq!(
            host.world.player(host.objects, scene.owner).unwrap().action,
            Some(retained_action)
        );
    }
}

#[test]
fn missing_scene_signal_fails_on_the_exact_service_without_advancing_the_action_clock() {
    let mut scene = scene(AuthoredSceneAction::Scene3);
    scene.world.cinematic_signals = None;
    scene.action().elapsed = 179;
    scene.visit(false).unwrap();
    let before = *scene.action();
    assert_eq!(
        scene.visit(false),
        Err(SceneError::PlayerAction(
            PlayerActionError::MissingCinematicSignals
        ))
    );
    assert_eq!(*scene.action(), before);
    scene.world.cinematic_signals = Some(Default::default());
    assert_eq!(scene.visit(false), Err(SceneError::Faulted));
    assert_eq!(*scene.action(), before);
}
