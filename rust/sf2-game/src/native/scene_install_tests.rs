use super::*;
use crate::player_action::PlayerActionState;
use crate::scene_path_world::PlayerPathRecords;
use crate::{ObjectSpawnDefaults, RandomState};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    player: ObjectId,
}

const PRIOR: PlayerActionState = PlayerActionState {
    action: None,
    elapsed: 77,
    auxiliary_counter: 99,
    total_updates: 1234,
};

fn fixture(count: usize, selection: u8, saved: u8) -> Fixture {
    let mut objects = ObjectStore::new();
    let player = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    for _ in 1..count {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
    }
    let mut world = ScenePathWorld::new(RandomState::default());
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 42,
        run_when_paused: false,
    });
    world.scene_selection = Some(selection);
    world
        .bind_player(
            &objects,
            player,
            PlayerPathRecords {
                action: Some(PRIOR),
                saved_scene_selection: Some(saved),
                ..Default::default()
            },
        )
        .unwrap();
    Fixture {
        objects,
        world,
        player,
    }
}

impl Fixture {
    fn install(&mut self) -> Result<Option<ObjectId>, SceneInstallError> {
        install(&mut self.objects, &mut self.world, self.player)
    }
    fn action(&self) -> PlayerActionState {
        self.world
            .player(&self.objects, self.player)
            .unwrap()
            .action
            .unwrap()
    }
}

#[test]
fn sentinels_allocate_nothing_even_with_a_full_pool() {
    for selection in [SKIP_FIRST, SKIP_SECOND] {
        let mut f = fixture(OBJECT_CAPACITY, selection, SCENE_NINE);
        assert_eq!(f.install(), Ok(None));
        assert_eq!(f.world.scene_selection, Some(selection));
        assert_eq!(f.action(), PRIOR);
    }
}

#[test]
fn scene_three_installs_its_path_action_and_zero_companion_seed() {
    let mut f = fixture(3, SCENE_THREE, 0);
    let created = f.install().unwrap().unwrap();
    let actor = f.objects.get(created).unwrap();
    assert_eq!(
        actor.base.path,
        Some(super::super::authored_paths::SCENE_THREE)
    );
    assert_eq!(
        f.action(),
        PlayerActionState {
            action: Some(PlayerAction::Scene(AuthoredSceneAction::Scene3)),
            elapsed: 0,
            auxiliary_counter: 0,
            total_updates: PRIOR.total_updates,
        }
    );
}

#[test]
fn scene_five_installs_its_path_action_and_zero_companion_seed() {
    let mut f = fixture(3, SCENE_FIVE, 0);
    let created = f.install().unwrap().unwrap();
    let actor = f.objects.get(created).unwrap();
    assert_eq!(
        actor.base.path,
        Some(super::super::authored_paths::SCENE_FIVE)
    );
    assert_eq!(
        f.action(),
        PlayerActionState {
            action: Some(PlayerAction::Scene(AuthoredSceneAction::Scene5)),
            elapsed: 0,
            auxiliary_counter: 0,
            total_updates: PRIOR.total_updates,
        }
    );
}

#[test]
fn scene_nine_installs_follow_path_actor_and_resets_both_counters() {
    let mut f = fixture(3, SCENE_NINE, 0);
    let created = f.install().unwrap().unwrap();
    // Allocation is after the global head.
    assert_eq!(f.objects.active_ids()[1], created);
    let actor = f.objects.get(created).unwrap();
    assert_eq!(actor.base.behavior, Behavior::FollowPath);
    assert_eq!(
        actor.base.path,
        Some(super::super::authored_paths::SCENE_NINE)
    );
    assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
    assert!(actor.base.flags.collision_disabled);
    assert_eq!(
        f.action(),
        PlayerActionState {
            action: Some(PlayerAction::Scene(AuthoredSceneAction::Scene9)),
            elapsed: 0,
            auxiliary_counter: SCENE_NINE_COMPANION_SEED,
            total_updates: PRIOR.total_updates,
        }
    );
}

#[test]
fn restore_selector_substitutes_saved_scene_without_retesting_sentinels() {
    let mut f = fixture(3, RESTORE_SAVED_SCENE, SCENE_NINE);
    assert!(f.install().unwrap().is_some());
    assert_eq!(f.world.scene_selection, Some(SCENE_NINE));

    // A saved sentinel is NOT skipped: it indexes table entry zero.
    let mut f = fixture(3, RESTORE_SAVED_SCENE, SKIP_FIRST);
    let error = f.install().unwrap_err();
    assert!(matches!(
        error,
        SceneInstallError::UnsupportedScene {
            selection: SKIP_FIRST,
            table_index: 0,
            ..
        }
    ));
    assert_eq!(f.world.scene_selection, Some(SKIP_FIRST));
}

#[test]
fn unsupported_scenes_fault_after_allocation_without_touching_the_action() {
    for selection in [0, 6, 7, 29, 30, 200] {
        let mut f = fixture(3, selection, 0);
        let Err(SceneInstallError::UnsupportedScene { actor, .. }) = f.install() else {
            panic!("selection {selection} must fault");
        };
        assert!(f.objects.get(actor).is_some());
        assert_eq!(f.objects.len(), 4);
        assert_eq!(f.action(), PRIOR);
    }
}

#[test]
fn exhausted_pool_faults_after_saved_scene_substitution() {
    let mut f = fixture(OBJECT_CAPACITY, RESTORE_SAVED_SCENE, SCENE_NINE);
    assert_eq!(f.install(), Err(SceneInstallError::ObjectPoolExhausted));
    assert_eq!(f.world.scene_selection, Some(SCENE_NINE));
    assert_eq!(f.action(), PRIOR);
}
