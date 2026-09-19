//! Authored attached-child creation (`$7F:9042..918D`). Operands have already
//! been decoded into catalog identities and literal values. The two source
//! record forms share this service; the compact form supplies zero rotation.

use super::collision_pass::ExclusionGroups;
use super::path_relationships::{self, RelationshipError};
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, PathCursor, Rotation,
    ShapeId, Vector3,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildSpawn {
    pub shape: ShapeId,
    pub path: Option<PathCursor>,
    pub position: Vector3,
    pub rotation: Rotation,
    pub hit_points: u8,
    pub attack_power: u8,
    pub number: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    Relationships(RelationshipError),
    /// Source allocation failure publishes a null last-spawn and then writes
    /// fields through that null destination. Native execution reports a fault
    /// instead of emulating those unrelated memory corruptions.
    PoolExhausted,
    /// Only malformed preexisting native chains can reach this case. The
    /// allocated object remains live and identifiable for error reporting.
    Attachment {
        child: ObjectId,
        error: RelationshipError,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpawnState {
    /// Shared last-spawn selection, not a per-parent property (source D771).
    pub last_spawn: Option<ObjectId>,
}

impl SpawnState {
    /// Metadata `kind` belongs to the native caller, not a guessed source
    /// flag or shape classification. Authored behavior and contacts are set
    /// by this service independently of that presentation/gameplay category.
    pub fn child(
        &mut self,
        objects: &mut ObjectStore,
        caller: ObjectId,
        kind: ObjectKind,
        parameters: ChildSpawn,
        defaults: ObjectSpawnDefaults,
    ) -> Result<ObjectId, SpawnError> {
        let parent =
            path_relationships::spawn_parent(objects, caller).map_err(SpawnError::Relationships)?;
        let fresh = Object::new_authored(kind, parameters.shape, Behavior::FollowPath, defaults);
        let Some(child) = objects.allocate_scoped_after(caller, fresh) else {
            self.last_spawn = None;
            return Err(SpawnError::PoolExhausted);
        };
        path_relationships::attach_fresh_child(objects, parent, child, parameters.number)
            .map_err(|error| SpawnError::Attachment { child, error })?;
        let caller_state = objects.get(caller).expect("validated spawn caller");
        let selected_player = caller_state.extension.path_state.conditions.selected_player;
        let group = caller_state.extension.spawn_group;
        let actor = objects.get_mut(child).expect("freshly allocated child");
        actor.base.contacts.exclusion_groups = actor
            .base
            .contacts
            .exclusion_groups
            .union(ExclusionGroups::PATH_SPAWN);
        actor.extension.relative_position = parameters.position;
        actor.extension.relative_rotation = parameters.rotation;
        // Deliberately no eager conversion to world position. The source
        // publishes the parent's child chain later at the movement boundary.
        self.last_spawn = Some(child);
        actor.extension.parent = Some(caller);
        actor.base.hit_points = parameters.hit_points;
        actor.base.attack_power = parameters.attack_power;
        actor.extension.path_state.conditions.selected_player = selected_player;
        actor.extension.spawn_group = group;
        actor.base.path = parameters.path;
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::super::path_control::PlayerTarget;
    use super::super::{Angle, PathId, OBJECT_CAPACITY};
    use super::*;

    fn actor() -> Object {
        Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
    }

    fn parameters(number: u8) -> ChildSpawn {
        ChildSpawn {
            shape: ShapeId::TITLE_FORMATION_EFFECT,
            path: Some(PathCursor {
                path: PathId::from_catalog_index(0),
                command_index: 3,
            }),
            position: Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: -1,
            },
            rotation: Rotation {
                pitch: Angle::from_units(128),
                yaw: Angle::from_units(255),
                roll: Angle::from_units(64),
            },
            hit_points: 255,
            attack_power: 128,
            number,
        }
    }

    #[test]
    fn child_inherits_caller_not_attachment_parent_and_keeps_world_pose_zero_until_publication() {
        let mut objects = ObjectStore::new();
        let mut parent_actor = actor();
        parent_actor.base.position = Vector3 {
            x: 100,
            y: -200,
            z: 300,
        };
        parent_actor.extension.spawn_group = 11;
        let parent = objects.allocate(parent_actor).unwrap();
        let mut caller_actor = actor();
        caller_actor.base.position = Vector3 {
            x: 500,
            y: -600,
            z: 700,
        };
        caller_actor.extension.spawn_group = 42;
        caller_actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
        let caller = objects.allocate_after(Some(parent), caller_actor).unwrap();
        path_relationships::attach_fresh_child(&mut objects, parent, caller, 9).unwrap();
        let mut spawns = SpawnState::default();
        let requested = parameters(255);
        let child = spawns
            .child(
                &mut objects,
                caller,
                ObjectKind::Effect,
                requested,
                ObjectSpawnDefaults {
                    run_when_paused: true,
                    group: 77,
                },
            )
            .unwrap();
        assert_eq!(spawns.last_spawn, Some(child));
        assert_eq!(objects.active_ids(), &[parent, caller, child]);
        assert_eq!(objects.get(parent).unwrap().base.first_child, Some(caller));
        assert_eq!(objects.get(caller).unwrap().base.next_sibling, Some(child));
        let child = objects.get(child).unwrap();
        assert_eq!(child.base.kind, ObjectKind::Effect);
        assert_eq!(child.base.behavior, Behavior::FollowPath);
        assert_eq!(child.base.shape, requested.shape);
        assert_eq!(child.base.path, requested.path);
        assert_eq!(child.base.hit_points, 255);
        assert_eq!(child.base.attack_power, 128);
        assert_eq!(child.base.child_number, 255);
        assert_eq!(child.base.attachment, Some(parent));
        assert_eq!(child.extension.parent, Some(caller));
        assert_eq!(child.extension.relative_position, requested.position);
        assert_eq!(child.extension.relative_rotation, requested.rotation);
        assert_eq!(child.base.position, Vector3::default());
        assert_eq!(
            (child.base.pitch, child.base.yaw, child.base.roll),
            (Angle::ZERO, Angle::ZERO, Angle::ZERO)
        );
        assert_eq!(child.extension.spawn_group, 42);
        assert_eq!(
            child.extension.path_state.conditions.selected_player,
            PlayerTarget::Secondary
        );
        assert!(child.base.contacts.first_strategy_visit);
        assert!(child.base.contacts.run_when_paused);
        assert_eq!(
            child.base.contacts.exclusion_groups,
            ExclusionGroups::PATH_SPAWN
        );
        assert!(child.base.flags.remove_with_parent);
        assert!(child.base.flags.draw_list_admitted);
        assert!(child.base.flags.general_search_eligible);
        assert!(child.extension.path_state.hold_latched);
    }

    #[test]
    fn repeated_spawns_have_reverse_active_order_but_forward_child_order_and_shared_last_selection()
    {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(actor()).unwrap();
        let mut spawns = SpawnState::default();
        let first = spawns
            .child(
                &mut objects,
                parent,
                ObjectKind::Effect,
                parameters(0),
                ObjectSpawnDefaults::default(),
            )
            .unwrap();
        let mut compact = parameters(0);
        compact.rotation = Rotation::default();
        compact.path = None;
        let second = spawns
            .child(
                &mut objects,
                parent,
                ObjectKind::Scenery,
                compact,
                ObjectSpawnDefaults::default(),
            )
            .unwrap();
        assert_eq!(objects.active_ids(), &[parent, second, first]);
        assert_eq!(objects.get(parent).unwrap().base.first_child, Some(first));
        assert_eq!(objects.get(first).unwrap().base.next_sibling, Some(second));
        assert_eq!(objects.get(second).unwrap().base.next_sibling, None);
        assert_eq!(spawns.last_spawn, Some(second));
        assert_eq!(objects.get(second).unwrap().base.path, None);
        assert_eq!(
            objects.get(second).unwrap().extension.relative_rotation,
            Rotation::default()
        );
        assert!(!objects.get(second).unwrap().base.contacts.run_when_paused);
        assert_eq!(
            objects
                .get(second)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player,
            PlayerTarget::Primary
        );
        assert_eq!(
            path_relationships::find_child(&objects, parent, 0),
            Ok(Some(first))
        );
    }

    #[test]
    fn last_slot_pressure_scans_callers_suffix_without_changing_global_active_head() {
        let mut objects = ObjectStore::new();
        let mut ids = Vec::new();
        for _ in 0..OBJECT_CAPACITY - 1 {
            let mut candidate = actor();
            candidate.base.flags.reclaim_on_pool_pressure = true;
            ids.push(
                objects
                    .allocate_after(ids.last().copied(), candidate)
                    .unwrap(),
            );
        }
        let caller = ids[1];
        let mut spawns = SpawnState::default();
        let child = spawns
            .child(
                &mut objects,
                caller,
                ObjectKind::Effect,
                parameters(1),
                ObjectSpawnDefaults::default(),
            )
            .unwrap();
        assert_eq!(objects.len(), OBJECT_CAPACITY);
        assert_eq!(objects.active_ids()[0], ids[0]);
        assert_eq!(objects.active_ids()[1], caller);
        assert_eq!(objects.active_ids()[2], child);
        for (index, id) in ids.iter().copied().enumerate() {
            assert_eq!(
                objects.get(id).unwrap().base.flags.remove_after_tick,
                index != 0 && index != ids.len() - 1
            );
        }
        assert!(!objects.get(child).unwrap().base.flags.remove_after_tick);
        assert_eq!(objects.get(caller).unwrap().base.first_child, Some(child));
    }

    #[test]
    fn full_pool_clears_last_spawn_and_faults_without_a_fake_actor_or_null_field_writes() {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        for _ in 1..OBJECT_CAPACITY {
            objects.allocate(actor()).unwrap();
        }
        let before = objects.clone();
        let mut spawns = SpawnState {
            last_spawn: Some(caller),
        };
        assert_eq!(
            spawns.child(
                &mut objects,
                caller,
                ObjectKind::Effect,
                parameters(1),
                ObjectSpawnDefaults::default()
            ),
            Err(SpawnError::PoolExhausted)
        );
        assert_eq!(spawns.last_spawn, None);
        assert_eq!(objects, before);
    }

    #[test]
    fn invalid_caller_parent_is_rejected_before_allocation_or_last_spawn_changes() {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        objects
            .get_mut(caller)
            .unwrap()
            .extension
            .path_state
            .motion
            .attached_coordinates = true;
        let before = objects.clone();
        let mut spawns = SpawnState {
            last_spawn: Some(caller),
        };
        assert_eq!(
            spawns.child(
                &mut objects,
                caller,
                ObjectKind::Effect,
                parameters(1),
                ObjectSpawnDefaults::default()
            ),
            Err(SpawnError::Relationships(RelationshipError::MissingParent(
                caller
            )))
        );
        assert_eq!(spawns.last_spawn, Some(caller));
        assert_eq!(objects, before);
    }

    #[test]
    fn malformed_existing_chain_reports_the_retained_allocation_at_the_error_boundary() {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        objects.get_mut(caller).unwrap().base.first_child = Some(caller);
        let mut spawns = SpawnState {
            last_spawn: Some(caller),
        };
        let error = spawns
            .child(
                &mut objects,
                caller,
                ObjectKind::Effect,
                parameters(1),
                ObjectSpawnDefaults::default(),
            )
            .unwrap_err();
        let SpawnError::Attachment { child, error } = error else {
            panic!("expected an attachment-chain diagnostic");
        };
        assert_eq!(error, RelationshipError::ChildCycle(caller));
        assert_eq!(objects.len(), 2);
        assert_eq!(objects.active_ids(), &[caller, child]);
        assert_eq!(objects.get(child).unwrap().base.attachment, None);
        assert_eq!(objects.get(child).unwrap().base.path, None);
        assert_eq!(spawns.last_spawn, Some(caller));
    }
}
