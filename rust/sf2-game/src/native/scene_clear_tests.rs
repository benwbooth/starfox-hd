use super::*;
use crate::program_resources::ProgramResources;
use crate::program_state::ProgramData;
use crate::scene_proxy::{SceneProxyId, SCENE_PROXY_CAPACITY};
use crate::{Behavior, Object, ObjectId, ObjectKind, PathCursor, PathId, ShapeId};

fn actors(count: usize) -> ObjectStore {
    let mut objects = ObjectStore::new();
    for index in 0..count {
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
        actor.base.flags.general_search_eligible = true;
        actor.base.hit_points = index as u8 + 1;
        objects.allocate(actor).unwrap();
    }
    objects
}

fn capture(
    objects: &mut ObjectStore,
    proxies: &mut SceneProxyStore,
    owner: ObjectId,
    command_index: u16,
) -> Option<SceneProxyId> {
    proxies
        .capture_actor(
            objects,
            owner,
            PathCursor {
                path: PathId::from_catalog_index(2),
                command_index,
            },
            &ProgramResources::<ProgramData>::default(),
        )
        .unwrap()
}

#[test]
fn only_selected_actors_change_and_their_slots_and_relationships_survive() {
    for mask in 0..64 {
        let mut objects = actors(6);
        let ids = objects.active_ids().to_vec();
        let mut proxies = SceneProxyStore::default();
        for (index, &id) in ids.iter().enumerate() {
            let actor = objects.get_mut(id).unwrap();
            actor.base.flags.general_search_eligible = mask & (1 << index) != 0;
            actor.base.flags.remove_after_tick = index == 3;
            actor.base.attachment = ids.get(index + 1).copied();
            actor.base.attachment_next = ids.get(index + 2).copied();
            capture(&mut objects, &mut proxies, id, index as u16).unwrap();
        }
        let mut expected = objects.clone();
        for &id in &ids {
            let actor = expected.get_mut(id).unwrap();
            if actor.base.flags.general_search_eligible {
                actor.base.flags.remove_after_tick = true;
                actor.extension.scene_proxy = None;
            }
        }
        clear(&mut objects, &mut proxies).unwrap();
        assert_eq!(objects, expected);
        assert!(proxies.is_empty());
        clear(&mut objects, &mut proxies).unwrap();
        assert_eq!(objects, expected);
    }
}

#[test]
fn head_then_remaining_release_preserves_lifo_reuse_instead_of_resetting_the_pool() {
    let mut objects = actors(3);
    let ids = objects.active_ids().to_vec();
    let mut proxies = SceneProxyStore::default();
    let a = capture(&mut objects, &mut proxies, ids[0], 1).unwrap();
    let b = capture(&mut objects, &mut proxies, ids[1], 2).unwrap();
    let c = capture(&mut objects, &mut proxies, ids[2], 3).unwrap();
    let d = capture(&mut objects, &mut proxies, ids[0], 4).unwrap();
    assert_eq!(proxies.active_ids(), &[a, d, c, b]);
    proxies.retire_actor(&mut objects, ids[1]).unwrap();
    objects
        .get_mut(ids[2])
        .unwrap()
        .base
        .flags
        .general_search_eligible = false;
    // First release d via actor 0; then drain a, c, b from the active head.
    clear(&mut objects, &mut proxies).unwrap();
    for expected in [b, c, a, d] {
        assert_eq!(
            capture(&mut objects, &mut proxies, ids[0], 9),
            Some(expected)
        );
    }
}

#[test]
fn full_pool_and_overwritten_handles_are_all_reclaimed_in_original_order() {
    let mut objects = actors(1);
    let owner = objects.active_ids()[0];
    let mut proxies = SceneProxyStore::default();
    let ids: Vec<_> = (0..SCENE_PROXY_CAPACITY)
        .map(|index| capture(&mut objects, &mut proxies, owner, index as u16).unwrap())
        .collect();
    assert_eq!(capture(&mut objects, &mut proxies, owner, 999), None);
    clear(&mut objects, &mut proxies).unwrap();
    // Last capture first, then active head, then the reverse insertion list.
    let expected = ids[1..SCENE_PROXY_CAPACITY - 1]
        .iter()
        .copied()
        .chain([ids[0], ids[SCENE_PROXY_CAPACITY - 1]]);
    for id in expected {
        assert_eq!(capture(&mut objects, &mut proxies, owner, 999), Some(id));
    }
    assert_eq!(capture(&mut objects, &mut proxies, owner, 999), None);
}

#[test]
fn selected_stale_handle_is_cleared_before_error_without_marking_or_draining() {
    let mut objects = actors(3);
    let ids = objects.active_ids().to_vec();
    let mut proxies = SceneProxyStore::default();
    let first = capture(&mut objects, &mut proxies, ids[0], 0).unwrap();
    let stale = capture(&mut objects, &mut proxies, ids[1], 1).unwrap();
    let last = capture(&mut objects, &mut proxies, ids[2], 2).unwrap();
    proxies.release(stale).unwrap();
    assert_eq!(
        clear(&mut objects, &mut proxies),
        Err(SceneProxyError::MissingProxy(stale))
    );
    assert!(proxies.get(first).is_none());
    assert_eq!(proxies.active_ids(), &[last]);
    assert!(objects.get(ids[0]).unwrap().base.flags.remove_after_tick);
    for &id in &ids[1..] {
        assert!(!objects.get(id).unwrap().base.flags.remove_after_tick);
    }
    assert_eq!(objects.get(ids[1]).unwrap().extension.scene_proxy, None);
    assert_eq!(
        objects.get(ids[2]).unwrap().extension.scene_proxy,
        Some(last)
    );
}

#[test]
fn empty_actor_list_still_drains_detached_proxies() {
    let mut objects = actors(1);
    let owner = objects.active_ids()[0];
    let mut proxies = SceneProxyStore::default();
    capture(&mut objects, &mut proxies, owner, 0).unwrap();
    let mut empty = ObjectStore::new();
    clear(&mut empty, &mut proxies).unwrap();
    assert!(proxies.is_empty());
    assert!(empty.is_empty());
    clear(&mut empty, &mut proxies).unwrap();
}

struct Callbacks;
impl crate::scene_strategy::SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<crate::strategy_schedule::StrategyCompletion, ()> {
        panic!("scene clear does not dispatch or retire actors")
    }
    fn death_override(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<crate::strategy_schedule::StrategyCompletion>, ()> {
        panic!("scene clear does not dispatch death callbacks")
    }
    fn resume_map_on_death(
        _: &mut crate::scene_strategy::SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<(), ()> {
        panic!("scene clear does not resume the map")
    }
}

#[test]
fn scene_owner_latches_partial_failure_without_retry_or_early_retirement() {
    use crate::scene_path_world::ScenePathWorld;
    use crate::scene_strategy::{SceneActors, SceneError, SceneExecution};
    let mut objects = actors(3);
    let ids = objects.active_ids().to_vec();
    let mut world = ScenePathWorld::new(Default::default());
    let stale = capture(&mut objects, &mut world.proxies, ids[1], 0).unwrap();
    world.proxies.release(stale).unwrap();
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
        host.clear_scene_actors(),
        Err(SceneError::SceneClear(SceneProxyError::MissingProxy(stale)))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(host.objects.len(), 3);
    assert!(
        host.objects
            .get(ids[0])
            .unwrap()
            .base
            .flags
            .remove_after_tick
    );
    let objects_before = host.objects.clone();
    assert_eq!(host.clear_scene_actors(), Err(SceneError::Faulted));
    assert_eq!(host.objects, &objects_before);
}
