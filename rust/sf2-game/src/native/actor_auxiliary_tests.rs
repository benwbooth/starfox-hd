use super::*;
use crate::program_resources::PROGRAM_CAPACITY;
use crate::{Behavior, Object, ObjectKind, ObjectStore, PathId, ShapeId};

fn owner(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap()
}

fn continuation(command_index: u16) -> AuxiliaryRecord {
    AuxiliaryRecord::SceneContinuation(PathCursor {
        path: PathId::from_catalog_index(7),
        command_index,
    })
}

#[test]
fn table_has_shared_capacity_exact_growth_and_allocation_free_replacement() {
    let mut objects = ObjectStore::new();
    let actor = owner(&mut objects);
    let mut pool = ProgramResources::default();
    let mut table = ActorAuxiliary::default();
    assert_eq!(table.find(&pool, actor, AuxiliaryKind::SavedView), Ok(None));
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);
    table.set(&mut pool, actor, continuation(3)).unwrap();
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 10);
    let first_id = table.storage;
    for index in 0..=u16::MAX {
        table.set(&mut pool, actor, continuation(index)).unwrap();
        assert_eq!(table.storage, first_id);
        assert_eq!(table.entries(&pool, actor).unwrap(), &[continuation(index)]);
        assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 10);
    }
    // The data handle is a semantic value. Table updates must not release it.
    let payload = pool
        .allocate_owned(actor, 63, ProgramData::PathStack(Default::default()))
        .unwrap();
    table
        .set(&mut pool, actor, AuxiliaryRecord::SavedView(payload))
        .unwrap();
    assert_ne!(table.storage, first_id);
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 14 - 68);
    assert_eq!(pool.owner_count(actor), 2);
    assert_eq!(
        table.entries(&pool, actor).unwrap(),
        &[continuation(u16::MAX), AuxiliaryRecord::SavedView(payload)]
    );
    assert_eq!(
        table.clear_after_owner_release(&pool, actor),
        Err(AuxiliaryError::StorageStillOwned)
    );
    assert_eq!(pool.release_owner(actor).len(), 2);
    table.clear_after_owner_release(&pool, actor).unwrap();
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);
    assert_eq!(table.entries(&pool, actor).unwrap(), &[]);
}

#[test]
fn growth_cannot_use_old_table_capacity_and_failure_preserves_original() {
    let mut objects = ObjectStore::new();
    let actor = owner(&mut objects);
    let mut pool = ProgramResources::default();
    let mut table = ActorAuxiliary::default();
    table.set(&mut pool, actor, continuation(17)).unwrap();
    let reserve = pool
        .allocate_shared(
            PROGRAM_CAPACITY - 10 - 12 - 2,
            ProgramData::PathStack(Default::default()),
        )
        .unwrap();
    assert_eq!(pool.available_capacity(), 12);
    let before = pool.clone();
    let table_before = table.clone();
    assert_eq!(
        table.set(&mut pool, actor, AuxiliaryRecord::SavedView(reserve)),
        Err(AuxiliaryError::Allocation(
            AllocationFailure::NoContiguousFit
        ))
    );
    assert_eq!(pool, before);
    assert_eq!(table, table_before);
    // An existing key can still be updated when allocation cannot succeed.
    table.set(&mut pool, actor, continuation(255)).unwrap();
    assert_eq!(pool.available_capacity(), 12);
    pool.release_shared(reserve).unwrap();
    table
        .set(&mut pool, actor, AuxiliaryRecord::SavedView(reserve))
        .unwrap();
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 14);
}

#[test]
fn foreign_or_stale_table_handles_fault_without_losing_ownership() {
    let mut objects = ObjectStore::new();
    let actor = owner(&mut objects);
    let other = owner(&mut objects);
    let mut pool = ProgramResources::default();
    let mut table = ActorAuxiliary::default();
    table.set(&mut pool, actor, continuation(17)).unwrap();
    let before = pool.clone();
    let table_before = table.clone();
    assert_eq!(
        table.find(&pool, other, AuxiliaryKind::SceneContinuation),
        Err(AuxiliaryError::MissingStorage)
    );
    assert_eq!(
        table.set(&mut pool, other, continuation(23)),
        Err(AuxiliaryError::MissingStorage)
    );
    assert_eq!(
        table.clear_after_owner_release(&pool, other),
        Err(AuxiliaryError::MissingStorage)
    );
    assert_eq!(table, table_before);
    assert_eq!(pool, before);
    pool.release_owner(actor);
    assert_eq!(
        table.find(&pool, actor, AuxiliaryKind::SceneContinuation),
        Err(AuxiliaryError::MissingStorage)
    );
    let before = pool.clone();
    assert_eq!(
        table.set(&mut pool, actor, continuation(91)),
        Err(AuxiliaryError::MissingStorage)
    );
    assert_eq!(pool, before);
}

#[test]
fn source_duplicate_lookup_is_first_match_and_count_wrap_is_not_a_valid_growth() {
    let mut objects = ObjectStore::new();
    let actor = owner(&mut objects);
    let mut pool = ProgramResources::default();
    // Source tables can retain duplicated keys. No deduplication on lookup.
    let entries = (0..255).map(continuation).collect();
    let id = pool
        .allocate_owned(
            actor,
            1021,
            ProgramData::ActorAuxiliary(AuxiliaryRecords { entries }),
        )
        .unwrap();
    let mut table = ActorAuxiliary { storage: Some(id) };
    assert_eq!(
        table.find(&pool, actor, AuxiliaryKind::SceneContinuation),
        Ok(Some(continuation(0)))
    );
    table.set(&mut pool, actor, continuation(999)).unwrap();
    assert_eq!(
        table.entries(&pool, actor).unwrap()[..2],
        [continuation(999), continuation(1)]
    );
    let before = pool.clone();
    assert_eq!(
        table.set(&mut pool, actor, AuxiliaryRecord::SavedView(id)),
        Err(AuxiliaryError::CountOverflow)
    );
    assert_eq!(pool, before);
}
