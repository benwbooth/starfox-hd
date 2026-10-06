use super::*;
use crate::path_commands::ControlCommand;
use crate::path_control::PlayerTarget;
use crate::path_invocation::{InvocationEntry, InvocationError, PathInvocation};
use crate::path_program::{PathCatalog, ProgramError, Statement};
use crate::path_sound::{AuthoredCue, CueListener};
use crate::scene_path_world::{AudioRouting, ScenePathWorld};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategyHost};
use crate::{
    Behavior, ObjectKind, ObjectSpawnDefaults, PathCursor, PathId, RandomState, ShapeId, SoundEvent,
};

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn actor(objects: &mut ObjectStore, path: Option<PathCursor>) -> ObjectId {
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.path = path;
    actor.base.hit_points = 1;
    objects.allocate(actor).unwrap()
}

#[test]
fn live_mode_overrides_observations_without_changing_group_or_unrelated_flags() {
    let mut world = ScenePathWorld::new(RandomState::default());
    assert_eq!(world.spawn_defaults(), None);
    assert_eq!(world.scripted_view_active(), None);
    for flags in 0..=u16::MAX {
        world.view_transition_mode = Some(ViewTransitionMode { flags });
        assert_eq!(world.scripted_view_active(), Some(flags & 2 != 0));
        for observed in [false, true] {
            let observation = ObjectSpawnDefaults {
                group: 219,
                run_when_paused: observed,
            };
            world.spawn_defaults = Some(observation);
            assert_eq!(
                world.spawn_defaults(),
                Some(ObjectSpawnDefaults {
                    group: 219,
                    run_when_paused: flags & 2 != 0
                })
            );
            assert_eq!(world.spawn_defaults, Some(observation));
            assert_eq!(world.view_transition_mode.unwrap().flags, flags);
        }
        world.spawn_defaults = None;
        assert_eq!(world.spawn_defaults(), None);
        assert_eq!(world.scripted_view_active(), Some(flags & 2 != 0));
    }
}

#[test]
fn one_scene_invocation_shares_mode_save_restore_and_fresh_allocation_policy() {
    for observed_pause in [false, true] {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects, Some(at(0)));
        objects.get_mut(owner).unwrap().extension.spawn_group = 137;
        objects.get_mut(owner).unwrap().base.position = Vector3 {
            x: 91,
            y: -231,
            z: 451,
        };
        let view = actor(&mut objects, None);
        let projectile = actor(&mut objects, None);
        objects
            .get_mut(projectile)
            .unwrap()
            .base
            .contacts
            .exclusion_groups = ExclusionGroups::from_authored_class(0x50);
        let saved = objects.get(view).unwrap().clone();
        let mut world = ScenePathWorld::new(RandomState::new([1, 11, 19, 37]));
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0xA5A5 });
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 193,
            run_when_paused: observed_pause,
        });
        world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::Other; 2],
            markers: None,
        });
        let spawn = crate::path_spawn::IndependentSpawn {
            shape: ShapeId::EMPTY,
            path: None,
            hit_points: 3,
            attack_power: 5,
        };
        let catalog = PathCatalog::new(vec![vec![
            Statement::ViewTransition {
                enabled: true,
                next: at(1),
            },
            Statement::SpawnIndependent {
                kind: ObjectKind::Effect,
                parameters: spawn,
                next: at(2),
            },
            Statement::MoveFixedView {
                snap: true,
                next: at(3),
            },
            Statement::ViewTransition {
                enabled: false,
                next: at(4),
            },
            Statement::SpawnIndependent {
                kind: ObjectKind::Effect,
                parameters: spawn,
                next: at(5),
            },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let mut paths = PathInvocation::default();
        paths.begin(owner, InvocationEntry::Program).unwrap();
        let before = objects.active_ids().to_vec();
        paths
            .resume(&catalog, &mut objects, &mut world, 30)
            .unwrap();
        let fresh: Vec<_> = objects
            .active_ids()
            .iter()
            .copied()
            .filter(|id| !before.contains(id))
            .collect();
        assert_eq!(fresh.len(), 2);
        assert_eq!(
            fresh
                .iter()
                .filter(|id| objects.get(**id).unwrap().base.contacts.run_when_paused)
                .count(),
            1
        );
        assert!(fresh
            .iter()
            .all(|id| objects.get(*id).unwrap().extension.spawn_group == 137));
        assert_eq!(objects.get(view).unwrap(), &saved);
        assert_eq!(objects.get(projectile).unwrap().base.hit_points, 0);
        assert!(
            objects
                .get(projectile)
                .unwrap()
                .base
                .contacts
                .run_when_paused
        );
        assert_eq!(world.view_transition_mode.unwrap().flags, 0xA5A5);
        assert!(!objects.get(owner).unwrap().base.contacts.run_when_paused);
        // Snapshot payload is freed; the counted auxiliary record remains.
        assert_eq!(paths.runtime.resources.owner_count(owner), 1);
        assert_eq!(world.random.bytes(), [1, 11, 19, 37]);
        assert_eq!(
            world
                .audio
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
            [
                SoundEvent::Authored(AuthoredCue::new(248, 0, PlayerTarget::Primary)),
                SoundEvent::Authored(AuthoredCue::new(247, 0, PlayerTarget::Primary)),
            ]
        );
    }
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("test uses real authored paths")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("no actor should reach death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("no map continuation")
    }
}

#[test]
fn actor_scheduler_resamples_transition_word_on_each_visit() {
    for observed_pause in [false, true] {
        let mut objects = ObjectStore::new();
        let controller = actor(&mut objects, Some(at(0)));
        let follower = actor(&mut objects, Some(at(2)));
        objects.get_mut(follower).unwrap().base.velocity = Vector3 { x: 7, y: 11, z: 17 };
        let view = actor(&mut objects, None);
        objects
            .get_mut(controller)
            .unwrap()
            .base
            .contacts
            .run_when_paused = true;
        let mut world = ScenePathWorld::new(RandomState::default());
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0xA5A5 });
        world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::Other; 2],
            markers: None,
        });
        let catalog = PathCatalog::new(vec![vec![
            Statement::ViewTransition {
                enabled: true,
                next: at(1),
            },
            Statement::Control(ControlCommand::WaitOne { next: at(3) }),
            Statement::Control(ControlCommand::WaitOne { next: at(2) }),
            Statement::ViewTransition {
                enabled: false,
                next: at(4),
            },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let mut execution = SceneExecution::default();
        execution.controls.paused = observed_pause;
        let mut callbacks = Callbacks;
        let mut scene = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 30,
        };
        scene.run_strategy(controller, 1).unwrap();
        assert!(scene.world.view_transition_mode.unwrap().active());
        let before = scene.objects.get(follower).unwrap().clone();
        scene.run_strategy(follower, 1).unwrap();
        let mut expected = before;
        expected.base.contacts.first_strategy_visit = false;
        assert_eq!(scene.objects.get(follower).unwrap(), &expected);
        // Jump to the real restore command; no synthetic mode assignment.
        scene.objects.get_mut(controller).unwrap().base.path = Some(at(3));
        scene.run_strategy(controller, 2).unwrap();
        assert!(!scene.world.view_transition_mode.unwrap().active());
        scene.run_strategy(follower, 2).unwrap();
        assert_eq!(
            scene.objects.get(follower).unwrap().base.position,
            Vector3 { x: 7, y: 11, z: 17 }
        );
        assert!(!scene.execution.is_faulted());
    }
}

#[test]
fn missing_transition_inputs_can_resume_without_earlier_effects_or_duplicate_save() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, Some(at(0)));
    let view = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut paths = PathInvocation::default();
    let catalog = PathCatalog::new(vec![vec![
        Statement::ViewTransition {
            enabled: true,
            next: at(1),
        },
        Statement::Control(ControlCommand::Hold),
    ]])
    .unwrap();
    paths.begin(owner, InvocationEntry::Program).unwrap();
    let before = objects.clone();
    for expected in [
        ProgramError::MissingViewTransitionMode,
        ProgramError::MissingAudio,
        ProgramError::MissingFixedView,
    ] {
        assert_eq!(
            paths.resume(&catalog, &mut objects, &mut world, 20),
            Err(InvocationError::Program(expected.clone()))
        );
        assert_eq!(objects, before);
        assert_eq!(paths.runtime.resources.owner_count(owner), 0);
        match expected {
            ProgramError::MissingViewTransitionMode => {
                world.view_transition_mode = Some(ViewTransitionMode::default())
            }
            ProgramError::MissingAudio => {
                world.audio_routing = Some(AudioRouting {
                    listeners: [CueListener::Other; 2],
                    markers: None,
                })
            }
            ProgramError::MissingFixedView => world.fixed_players[0] = Some(view),
            _ => unreachable!(),
        }
    }
    paths
        .resume(&catalog, &mut objects, &mut world, 20)
        .unwrap();
    assert_eq!(paths.runtime.resources.owner_count(owner), 2);
    assert_eq!(world.audio.take_events().into_iter().flatten().count(), 1);
}
