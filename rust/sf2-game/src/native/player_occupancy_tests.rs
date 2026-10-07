use super::*;
use crate::path_program::PathCatalog;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
use crate::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not dispatched")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not dispatched")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not dispatched")
    }
}
struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
    proxy: ObjectId,
    catalog: PathCatalog,
    callbacks: Callbacks,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut actor = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let owner = actor();
        let proxy = actor();
        let view = actor();
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.fixed_players[0] = Some(view);
        world.action_gate = Some(Default::default());
        world.occupancy = Some(WorldOccupancy::default());
        world.player_yaw_increment = Some(0xA5F3);
        Self {
            objects,
            world,
            execution,
            owner,
            proxy,
            catalog: PathCatalog::new(vec![]).unwrap(),
            callbacks: Callbacks,
        }
    }
    fn record(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn advance(&mut self, context: OccupancyContext) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
        .advance_player_occupancy(self.owner, context)
    }
    fn mark(&mut self, x: i16, z: i16) {
        let coverage = MarkerCoverage::from_rectangle(WorldRectangle {
            x,
            z,
            width: 1,
            depth: 1,
        })
        .unwrap();
        self.world
            .occupancy
            .as_mut()
            .unwrap()
            .apply(&coverage, OccupancyChange::Mark);
    }
}
fn context() -> OccupancyContext {
    OccupancyContext {
        camera_override_active: Some(false),
        diagonal_tie_bias: Some(0),
    }
}

#[test]
fn gates_are_lazy_but_only_the_scene_gate_preserves_view_contact() {
    let mut f = Fixture::new();
    f.record().motion.as_mut().unwrap().contact_flags = 0xFF;
    f.world.action_gate.as_mut().unwrap().code = 1;
    f.world.occupancy = None;
    f.world.fixed_players = [None; 2];
    f.record().occupancy_exempt = None;
    assert_eq!(f.advance(Default::default()), Ok(()));
    assert_eq!(f.record().motion.unwrap().contact_flags, 0xFF);
    f.world.action_gate.as_mut().unwrap().code = 0;
    f.record().occupancy_exempt = Some(true);
    assert_eq!(f.advance(Default::default()), Ok(()));
    assert_eq!(f.record().motion.unwrap().contact_flags, 0x7F);
    f.record().occupancy_exempt = Some(false);
    assert_eq!(
        f.advance(OccupancyContext {
            camera_override_active: Some(true),
            ..Default::default()
        }),
        Ok(())
    );
    assert_eq!(
        f.advance(Default::default()),
        Err(SceneError::PlayerOccupancy(
            OccupancyError::MissingCameraTask
        ))
    );
    assert_eq!(f.advance(context()), Err(SceneError::Faulted));
}

#[test]
fn open_cell_updates_only_changed_history_axes_and_preserves_displacement() {
    let mut f = Fixture::new();
    f.mark(0, 0);
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: -1,
        y: 123,
        z: 1024,
    };
    let state = f.record().occupancy.as_mut().unwrap();
    state.current_cell = [-1, 7];
    state.previous_cell = [29, 31];
    state.displacement = [37, 41];
    f.record().motion.as_mut().unwrap().contact_flags = 0x7F;
    assert_eq!(
        f.advance(OccupancyContext {
            camera_override_active: Some(false),
            diagonal_tie_bias: None
        }),
        Ok(())
    );
    assert_eq!(
        f.record().occupancy.unwrap(),
        PlayerOccupancy {
            current_cell: [-1, 2],
            previous_cell: [29, 7],
            displacement: [37, 41]
        }
    );
    assert_eq!(f.record().motion.unwrap().contact_flags, 0xBF);
}

#[test]
fn diagonal_tie_requires_real_bias_after_both_proxy_publications_and_latches_failure() {
    let mut f = Fixture::new();
    f.mark(600, 600);
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: 600,
        y: 43,
        z: 600,
    };
    f.objects.get_mut(f.proxy).unwrap().base.position.y = 47;
    f.record().occupancy.as_mut().unwrap().current_cell = [0, 0];
    assert_eq!(
        f.advance(OccupancyContext {
            camera_override_active: Some(false),
            diagonal_tie_bias: None
        }),
        Err(SceneError::PlayerOccupancy(
            OccupancyError::MissingDiagonalTieBias
        ))
    );
    assert_eq!(
        f.objects.get(f.proxy).unwrap().base.position,
        Vector3 {
            x: 256,
            y: 47,
            z: 768
        }
    );
    assert_eq!(f.record().motion.unwrap().contact_flags, 0x50);
    assert_eq!(f.record().occupancy.unwrap().current_cell, [0, 0]);
    assert_eq!(f.advance(context()), Err(SceneError::Faulted));
}

#[test]
fn blocked_response_reuses_actual_heading_pose_history_and_low_yaw_publication() {
    let mut f = Fixture::new();
    f.mark(600, 100);
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: 600,
        y: 43,
        z: 100,
    };
    f.record().auxiliary.as_mut().unwrap().mode = 0x10;
    f.record().boundary.as_mut().unwrap().return_position.y = 47;
    player_storage::get_mut(
        &f.objects,
        &mut f.execution.paths.runtime.resources,
        f.owner,
    )
    .unwrap()
    .fine_yaw = 0x4000;
    assert_eq!(f.advance(context()), Ok(()));
    let storage =
        player_storage::get(&f.objects, &f.execution.paths.runtime.resources, f.owner).unwrap();
    assert_ne!(storage.fine_yaw, 0);
    assert_ne!(f.record().pose.unwrap().turning_lean, 0);
    assert_eq!(f.world.player_yaw_increment, Some(0xA500));
    assert_eq!(
        f.objects.get(f.owner).unwrap().base.position,
        Vector3 {
            x: 511,
            y: 43,
            z: 100
        }
    );
    assert_eq!(
        f.record().boundary.unwrap().return_position,
        Vector3 {
            x: 511,
            y: 47,
            z: 100
        }
    );
    assert_eq!(f.record().occupancy.unwrap().displacement, [0, 0]);
    f.record().motion.as_mut().unwrap().contact_flags &= !0x10;
    f.record().auxiliary.as_mut().unwrap().mode = 0;
    f.objects.get_mut(f.owner).unwrap().base.position.x = 600;
    assert_eq!(f.advance(context()), Ok(()));
    assert_eq!(f.record().motion.unwrap().contact_flags & 0x10, 0);
    assert_eq!(f.record().occupancy.unwrap().displacement, [-89, 0]);
}
