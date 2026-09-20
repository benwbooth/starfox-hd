//! Scene-proxy lifetime (`$0D:D5B7`, `$7F:AF00`, `$7F:33D6..344E`).
//! A proxy is a scene-owned snapshot, not an actor, contact, or path callback.
//! Actor retirement detaches it; only the scene owner later releases its slot.

use super::{ObjectId, ObjectStore, PathCursor, Rotation, ShapeId, Vector3};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;

pub const SCENE_PROXY_CAPACITY: usize = 512;
const ACTOR_ATTACHED: u8 = 0x01;
const ACTOR_CAPTURED: u8 = 0x02;
const ACTOR_RETIRED: u8 = 0x10;
const TARGET_CONSIDERED: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneProxyId(usize);

impl SceneProxyId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneProxyFlags(u8);

impl SceneProxyFlags {
    /// Flags decoded from scene records, not object or collision flags.
    pub const fn from_authored_bits(bits: u8) -> Self {
        Self(bits)
    }
    pub const fn authored_bits(self) -> u8 {
        self.0
    }
    pub const fn actor_attached(self) -> bool {
        self.0 & ACTOR_ATTACHED != 0
    }
    pub const fn actor_retired(self) -> bool {
        self.0 & ACTOR_RETIRED != 0
    }
    /// `$7F:B273`: published even when target selection rejects the actor.
    pub fn mark_target_considered(&mut self) {
        self.0 |= TARGET_CONSIDERED;
    }
    fn retire_actor(&mut self) {
        self.0 = (self.0 & !ACTOR_ATTACHED) | ACTOR_RETIRED;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneProxy {
    pub position: Vector3,
    pub rotation: Rotation,
    pub shape: ShapeId,
    /// Decoded continuation retained for scene-driven actor creation.
    pub continuation: PathCursor,
    pub flags: SceneProxyFlags,
    pub owner: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneProxyError {
    Auxiliary(super::actor_auxiliary::AuxiliaryError),
    MissingActor(ObjectId),
    MissingProxy(SceneProxyId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneProxyStore {
    entries: [Option<SceneProxy>; SCENE_PROXY_CAPACITY],
    active: Vec<SceneProxyId>,
    free: Vec<SceneProxyId>,
}

impl Default for SceneProxyStore {
    fn default() -> Self {
        Self {
            entries: [None; SCENE_PROXY_CAPACITY],
            active: Vec::new(),
            free: (0..SCENE_PROXY_CAPACITY).rev().map(SceneProxyId).collect(),
        }
    }
}

impl SceneProxyStore {
    pub fn len(&self) -> usize {
        self.active.len()
    }
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
    pub fn active_ids(&self) -> &[SceneProxyId] {
        &self.active
    }
    pub fn get(&self, id: SceneProxyId) -> Option<&SceneProxy> {
        self.entries[id.0].as_ref()
    }
    pub fn get_mut(&mut self, id: SceneProxyId) -> Option<&mut SceneProxy> {
        self.entries[id.0].as_mut()
    }

    /// New entries follow the current head, not the tail or a new head.
    /// Exhaustion leaves both lists and all live entries unchanged.
    pub fn allocate(&mut self, proxy: SceneProxy) -> Option<SceneProxyId> {
        let id = self.free.pop()?;
        self.entries[id.0] = Some(proxy);
        self.active.insert(usize::from(!self.active.is_empty()), id);
        Some(id)
    }

    /// Explicit scene-owner release. Callers must first detach a live actor's
    /// handle, as free-list insertion does not visit actors to repair it.
    pub fn release(&mut self, id: SceneProxyId) -> Result<SceneProxy, SceneProxyError> {
        let index = self
            .active
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or(SceneProxyError::MissingProxy(id))?;
        let proxy = self.entries[id.0]
            .take()
            .ok_or(SceneProxyError::MissingProxy(id))?;
        self.active.remove(index);
        self.free.push(id);
        Ok(proxy)
    }

    /// The retained scene path (auxiliary type 3) overrides the supplied
    /// decoded instruction following capture. It is not a callback.
    /// A second capture replaces the actor's handle WITHOUT freeing its old
    /// proxy, matching the source; callers must not infer exclusive ownership.
    pub fn capture_actor(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        continuation: PathCursor,
        resources: &ProgramResources<ProgramData>,
    ) -> Result<Option<SceneProxyId>, SceneProxyError> {
        let actor = objects
            .get(owner)
            .ok_or(SceneProxyError::MissingActor(owner))?;
        let proxy = SceneProxy {
            position: actor.base.position,
            rotation: Rotation {
                pitch: actor.base.pitch,
                yaw: actor.base.yaw,
                roll: actor.base.roll,
            },
            shape: actor.base.shape,
            continuation: actor.extension.auxiliary.scene_continuation(resources, owner)
                .map_err(SceneProxyError::Auxiliary)?.unwrap_or(continuation),
            flags: SceneProxyFlags(ACTOR_ATTACHED | ACTOR_CAPTURED),
            owner: Some(owner),
        };
        let id = self.allocate(proxy);
        if let Some(id) = id {
            objects
                .get_mut(owner)
                .ok_or(SceneProxyError::MissingActor(owner))?
                .extension
                .scene_proxy = Some(id);
        }
        Ok(id)
    }

    /// `$7F:3425`: called BEFORE contact separation and object detachment.
    /// The proxy remains in the scene list and consumes capacity. Pose,
    /// continuation, shape, and flags other than attachment/retirement survive.
    pub fn retire_actor(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
    ) -> Result<(), SceneProxyError> {
        let id = objects
            .get_mut(owner)
            .ok_or(SceneProxyError::MissingActor(owner))?
            .extension
            .scene_proxy
            .take();
        if let Some(id) = id {
            let proxy = self.get_mut(id).ok_or(SceneProxyError::MissingProxy(id))?;
            proxy.flags.retire_actor();
            proxy.owner = None;
        }
        Ok(())
    }

    /// `$7F:33D6`: explicit immediate removal is distinct from retirement.
    pub fn release_actor_proxy(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
    ) -> Result<(), SceneProxyError> {
        let id = objects
            .get_mut(owner)
            .ok_or(SceneProxyError::MissingActor(owner))?
            .extension
            .scene_proxy
            .take();
        if let Some(id) = id {
            self.release(id)?;
        }
        Ok(())
    }

    /// `$7F:AFCB`: update only the currently linked proxy's continuation.
    /// With no proxy, the caller must store auxiliary type 3 instead.
    pub fn preserve_continuation(
        &mut self,
        objects: &ObjectStore,
        owner: ObjectId,
        continuation: PathCursor,
    ) -> Result<bool, SceneProxyError> {
        let id = objects
            .get(owner)
            .ok_or(SceneProxyError::MissingActor(owner))?
            .extension
            .scene_proxy;
        let Some(id) = id else { return Ok(false) };
        self.get_mut(id)
            .ok_or(SceneProxyError::MissingProxy(id))?
            .continuation = continuation;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Angle, Behavior, Object, ObjectKind, PathId};

    fn continuation(index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(2),
            command_index: index,
        }
    }

    fn actor(objects: &mut ObjectStore) -> ObjectId {
        let mut object = Object::new(
            ObjectKind::Enemy,
            ShapeId::ENEMY_LASER,
            Behavior::FollowPath,
        );
        object.base.position = Vector3 {
            x: -32_768,
            y: 31_234,
            z: -20,
        };
        object.base.pitch = Angle::from_units(200);
        object.base.yaw = Angle::from_units(90);
        object.base.roll = Angle::from_units(10);
        objects.allocate(object).unwrap()
    }

    #[test]
    fn capacity_is_independent_of_actor_pool_and_insertion_preserves_head() {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut proxies = SceneProxyStore::default();
        let ids: Vec<_> = (0..SCENE_PROXY_CAPACITY)
            .map(|index| {
                let id = proxies
                    .capture_actor(&mut objects, owner, continuation(index as u16), &ProgramResources::default())
                    .unwrap()
                    .unwrap();
                assert_eq!(id.index(), index);
                id
            })
            .collect();
        assert_eq!(proxies.active_ids()[0], ids[0]);
        assert_eq!(
            &proxies.active_ids()[1..],
            ids[1..].iter().rev().copied().collect::<Vec<_>>()
        );
        let before = proxies.clone();
        assert_eq!(
            proxies
                .capture_actor(&mut objects, owner, continuation(999), &ProgramResources::default())
                .unwrap(),
            None
        );
        assert_eq!(proxies, before);
        assert_eq!(objects.len(), 1);
        assert_eq!(
            objects.get(owner).unwrap().extension.scene_proxy,
            ids.last().copied()
        );
    }

    #[test]
    fn capture_snapshots_pose_and_continuation_does_not_follow_actor_mutations() {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut proxies = SceneProxyStore::default();
        let id = proxies
            .capture_actor(&mut objects, owner, continuation(31), &ProgramResources::default())
            .unwrap()
            .unwrap();
        let snapshot = *proxies.get(id).unwrap();
        assert_eq!(snapshot.position, objects.get(owner).unwrap().base.position);
        assert_eq!(snapshot.rotation.pitch.units(), 200);
        assert_eq!(snapshot.rotation.yaw.units(), 90);
        assert_eq!(snapshot.rotation.roll.units(), 10);
        assert_eq!(snapshot.shape, ShapeId::ENEMY_LASER);
        assert_eq!(snapshot.flags.authored_bits(), 3);
        assert_eq!(snapshot.owner, Some(owner));
        objects.get_mut(owner).unwrap().base.position.x = 7;
        assert_eq!(proxies.get(id), Some(&snapshot));
        assert!(proxies
            .preserve_continuation(&objects, owner, continuation(60))
            .unwrap());
        assert_eq!(proxies.get(id).unwrap().continuation, continuation(60));
        let other = actor(&mut objects);
        assert!(!proxies
            .preserve_continuation(&objects, other, continuation(61))
            .unwrap());
        assert_eq!(proxies.len(), 1);
    }

    #[test]
    fn retirement_keeps_scene_slot_and_snapshot_but_clears_owner_for_every_flag_value() {
        for bits in 0..=u8::MAX {
            let mut objects = ObjectStore::new();
            let owner = actor(&mut objects);
            let mut proxies = SceneProxyStore::default();
            let id = proxies
                .capture_actor(&mut objects, owner, continuation(31), &ProgramResources::default())
                .unwrap()
                .unwrap();
            proxies.get_mut(id).unwrap().flags = SceneProxyFlags::from_authored_bits(bits);
            let mut expected = *proxies.get(id).unwrap();
            expected.flags = SceneProxyFlags::from_authored_bits((bits & 0xFE) | 0x10);
            expected.owner = None;
            proxies.retire_actor(&mut objects, owner).unwrap();
            assert_eq!(proxies.get(id), Some(&expected));
            assert_eq!(proxies.active_ids(), [id]);
            assert_eq!(objects.get(owner).unwrap().extension.scene_proxy, None);
            assert_eq!(objects.len(), 1);
            proxies.retire_actor(&mut objects, owner).unwrap();
            assert_eq!(proxies.len(), 1);
            objects.remove(owner).unwrap();
            let replacement = actor(&mut objects);
            assert_eq!(replacement, owner);
            assert_eq!(proxies.get(id).unwrap().owner, None);
            let next = proxies
                .capture_actor(&mut objects, replacement, continuation(32), &ProgramResources::default())
                .unwrap()
                .unwrap();
            assert_ne!(next, id);
        }
    }

    #[test]
    fn immediate_release_unlinks_and_reuses_head_middle_and_tail_lifo() {
        for index in 0..3 {
            let mut objects = ObjectStore::new();
            let owners: Vec<_> = (0..3).map(|_| actor(&mut objects)).collect();
            let mut proxies = SceneProxyStore::default();
            let ids: Vec<_> = owners
                .iter()
                .map(|&owner| {
                    proxies
                        .capture_actor(&mut objects, owner, continuation(4), &ProgramResources::default())
                        .unwrap()
                        .unwrap()
                })
                .collect();
            let old_order = proxies.active_ids().to_vec();
            proxies
                .release_actor_proxy(&mut objects, owners[index])
                .unwrap();
            let expected: Vec<_> = old_order
                .into_iter()
                .filter(|id| *id != ids[index])
                .collect();
            assert_eq!(proxies.active_ids(), expected);
            assert_eq!(
                objects.get(owners[index]).unwrap().extension.scene_proxy,
                None
            );
            assert_eq!(proxies.get(ids[index]), None);
            let fresh = proxies
                .capture_actor(&mut objects, owners[index], continuation(5), &ProgramResources::default())
                .unwrap()
                .unwrap();
            assert_eq!(fresh, ids[index]);
            assert_eq!(proxies.active_ids()[1], fresh);
        }
    }

    #[test]
    fn repeated_capture_replaces_only_actor_handle_and_retirement_only_detaches_latest() {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects);
        let mut proxies = SceneProxyStore::default();
        let first = proxies
            .capture_actor(&mut objects, owner, continuation(1), &ProgramResources::default())
            .unwrap()
            .unwrap();
        let second = proxies
            .capture_actor(&mut objects, owner, continuation(2), &ProgramResources::default())
            .unwrap()
            .unwrap();
        proxies.retire_actor(&mut objects, owner).unwrap();
        assert_eq!(proxies.get(first).unwrap().owner, Some(owner));
        assert_eq!(proxies.get(second).unwrap().owner, None);
        assert_eq!(proxies.len(), 2);
    }
}
