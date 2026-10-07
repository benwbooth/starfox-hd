use super::*;
use crate::path_scene_state::EncounterObjectiveCounts;
use crate::view_transition::ViewTransitionMode;
use crate::{ObjectSpawnDefaults, RandomState, Vector3};

fn fixture(count: usize) -> (ObjectStore, ScenePathWorld) {
    let mut objects = ObjectStore::new();
    for _ in 0..count {
        let mut actor = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::Unassigned);
        actor.base.position = Vector3 {
            x: 31111,
            y: -19273,
            z: -30000,
        };
        objects.allocate(actor).unwrap();
    }
    let mut world = ScenePathWorld::new(RandomState::new([17, 91, 131, 251]));
    world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: 0xFF31,
        node_record: 211,
        recorded_completions: 0xCAFE,
        signaled_completions: 0xFECA,
    });
    world.node_exit = NodeExitState {
        presentation_flags: Some(0xA7),
        completion_code: Some(0),
    };
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 77,
        run_when_paused: false,
    });
    (objects, world)
}

#[test]
fn all_request_bytes_and_completion_codes_preserve_unowned_state() {
    for flags in 0..=u8::MAX {
        for code in 0..=u8::MAX {
            let (mut objects, mut world) = fixture(3);
            world.node_exit.presentation_flags = Some(flags);
            world.node_exit.completion_code = Some(code);
            let before = world.objective_counts.unwrap();
            let random = world.random;
            let created = advance(&mut objects, &mut world).unwrap();
            let expected = flags & 0xC0 == 0x80;
            assert_eq!(created.is_some(), expected);
            assert_eq!(objects.len(), 3 + usize::from(expected));
            assert_eq!(
                world.node_exit.presentation_flags,
                Some(flags | if expected { 0x40 } else { 0 })
            );
            assert_eq!(world.node_exit.completion_code, Some(code));
            assert_eq!(
                world.objective_counts,
                Some(EncounterObjectiveCounts {
                    remaining_word: if code == 1 {
                        0xFF00
                    } else {
                        before.remaining_word
                    },
                    ..before
                })
            );
            assert_eq!(world.random, random);
        }
    }
}

#[test]
fn only_the_objective_low_byte_admits_creation_and_completion_never_consumes_its_request() {
    for word in 0..=u16::MAX {
        let (mut objects, mut world) = fixture(1);
        world.objective_counts.as_mut().unwrap().remaining_word = word;
        world.node_exit.completion_code = Some(1);
        let created = advance(&mut objects, &mut world).unwrap();
        assert_eq!(created.is_some(), word as u8 != 0);
        assert_eq!(
            world.objective_counts.unwrap().remaining_word,
            word & 0xFF00
        );
        assert_eq!(world.node_exit.completion_code, Some(1));
        assert_eq!(advance(&mut objects, &mut world), Ok(None));
    }
}

#[test]
fn creation_is_unattached_fresh_and_after_the_active_head_without_running_the_path() {
    for paused in [false, true] {
        let (mut objects, mut world) = fixture(3);
        let old_ids = objects.active_ids().to_vec();
        let old_actors: Vec<_> = old_ids
            .iter()
            .map(|id| objects.get(*id).unwrap().clone())
            .collect();
        world.view_transition_mode = Some(ViewTransitionMode {
            flags: if paused { 2 } else { 0 },
        });
        let id = advance(&mut objects, &mut world).unwrap().unwrap();
        assert_eq!(
            objects.active_ids(),
            &[old_ids[0], id, old_ids[1], old_ids[2]]
        );
        let actual = objects.get(id).unwrap();
        let mut expected = Object::new_authored(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::FollowPath,
            ObjectSpawnDefaults {
                group: 77,
                run_when_paused: paused,
            },
        );
        expected.base.previous = Some(old_ids[0]);
        expected.base.next = Some(old_ids[1]);
        expected.base.path = Some(crate::authored_paths::NODE_EXIT_PRESENTATION);
        expected.base.hit_points = 1;
        expected.base.attack_power = 1;
        expected.base.flags.collision_disabled = true;
        expected.extension.path_state.needs_path_initialization = true;
        assert_eq!(actual, &expected);
        for (index, old) in old_ids.iter().copied().zip(old_actors) {
            let mut expected = old;
            if index == old_ids[0] {
                expected.base.next = Some(id);
            }
            if index == old_ids[1] {
                expected.base.previous = Some(id);
            }
            assert_eq!(objects.get(index), Some(&expected));
        }
        assert_eq!(advance(&mut objects, &mut world), Ok(None));
    }
}

#[test]
fn no_objectives_never_read_presentation_flags_or_allocation_defaults() {
    let (mut objects, mut world) = fixture(OBJECT_CAPACITY);
    world.objective_counts.as_mut().unwrap().remaining_word = 0xAB00;
    world.node_exit.presentation_flags = None;
    world.spawn_defaults = None;
    let before = objects.clone();
    assert_eq!(advance(&mut objects, &mut world), Ok(None));
    assert_eq!(objects, before);
    assert_eq!(world.node_exit.presentation_flags, None);
}

#[test]
fn full_pool_stops_before_objective_clear_or_late_dependencies() {
    let (mut objects, mut world) = fixture(OBJECT_CAPACITY);
    world.node_exit.completion_code = Some(1);
    world.spawn_defaults = None;
    let before = objects.clone();
    assert_eq!(
        advance(&mut objects, &mut world),
        Err(NodeExitError::ObjectPoolExhausted)
    );
    assert_eq!(objects, before);
    assert_eq!(world.objective_counts.unwrap().remaining_word, 0xFF31);
    assert_eq!(world.node_exit.presentation_flags, Some(0xA7));
}

#[test]
fn missing_inputs_preserve_the_real_completed_prefix() {
    for stage in 0..4 {
        let (mut objects, mut world) = fixture(3);
        let expected = match stage {
            0 => {
                world.objective_counts = None;
                NodeExitError::MissingObjectives
            }
            1 => {
                world.node_exit.presentation_flags = None;
                NodeExitError::MissingPresentationFlags
            }
            2 => {
                world.spawn_defaults = None;
                NodeExitError::MissingSpawnDefaults
            }
            _ => {
                world.node_exit.completion_code = None;
                NodeExitError::MissingCompletionCode
            }
        };
        assert_eq!(advance(&mut objects, &mut world), Err(expected));
        assert_eq!(objects.len(), if stage == 3 { 4 } else { 3 });
        if stage == 3 {
            assert_eq!(world.node_exit.presentation_flags, Some(0xE7));
            assert_eq!(world.objective_counts.unwrap().remaining_word, 0xFF31);
        }
    }
}

struct Callbacks;
impl crate::scene_strategy::SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<crate::strategy_schedule::StrategyCompletion, Self::Error> {
        panic!("admission does not execute a strategy")
    }
    fn death_override(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<crate::strategy_schedule::StrategyCompletion>, Self::Error> {
        panic!("admission does not execute a death callback")
    }
    fn resume_map_on_death(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), Self::Error> {
        panic!("admission does not resume the map")
    }
}

#[test]
fn scene_fault_keeps_the_created_actor_and_prevents_repeating_partial_admission() {
    use crate::scene_strategy::{SceneActors, SceneError, SceneExecution};
    let (mut objects, mut world) = fixture(3);
    world.node_exit.completion_code = None;
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let catalog = crate::authored_paths::catalog();
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.advance_player_node_exit(),
        Err(SceneError::PlayerNodeExit(
            NodeExitError::MissingCompletionCode
        ))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(host.objects.len(), 4);
    host.world.node_exit.completion_code = Some(1);
    assert_eq!(host.advance_player_node_exit(), Err(SceneError::Faulted));
    assert_eq!(host.objects.len(), 4);
    assert_eq!(host.world.objective_counts.unwrap().remaining_word, 0xFF31);
}

#[test]
fn scene_fault_keeps_the_consumed_layout_flag_without_replaying_its_prefix() {
    use crate::scene_strategy::{SceneActors, SceneError, SceneExecution};
    let (mut objects, mut world) = fixture(3);
    world.handoff = Some(crate::path_scene_state::EncounterHandoff {
        player_flags: 0xFF,
        ..Default::default()
    });
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let catalog = crate::authored_paths::catalog();
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.consume_player_layout_advance(),
        Err(SceneError::PlayerMission(
            crate::player_mission::MissionError::MissingLayout
        ))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(host.world.handoff.unwrap().player_flags, 0xFE);
    host.world.scene.encounter_layout = Some(255);
    assert_eq!(
        host.consume_player_layout_advance(),
        Err(SceneError::Faulted)
    );
    assert_eq!(host.world.scene.encounter_layout, Some(255));
}
