use super::*;
use crate::collision_surface::{SurfaceMode, SurfaceSearch};
use crate::path_program::PathCatalog;
use crate::player_motion::PlayerSurfaceSupport;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
use crate::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId, Vector3};

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
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let owner = allocate();
        let proxy = allocate();
        let view = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 32,
                score: Default::default(),
            },
        )
        .unwrap();
        objects.get_mut(owner).unwrap().base.position.y = -100;
        world.primary_player = Some(owner);
        world.fixed_players[0] = Some(view);
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.action_gate = Some(Default::default());
        world.occupancy = Some(WorldOccupancy::default());
        world.player_pitch_target = Some(0);
        world.player_yaw_increment = Some(0);
        world.processed_player_input = Some(Default::default());
        world.contacts_enabled = Some(true);
        world.view_transition_mode = Some(ViewTransitionMode::default());
        world.surface_mode = Some(SurfaceMode { flags: 0 });
        world.player_carry_mode = Some(0);
        world.environment_plane_height = Some(0);
        world.player_surface_support = Some(PlayerSurfaceSupport::default());
        world.scene.player_configuration = Some(0);
        world.reflect_all_contacts = Some(false);
        world.spawn_defaults = Some(Default::default());
        world
            .player_mut(&objects, owner)
            .unwrap()
            .vertical
            .as_mut()
            .unwrap()
            .motion_axes = 0xE0;
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
    fn records(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn advance(&mut self, context: FlightContext) -> Result<FlightResult, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
        .advance_player_flight(self.owner, context)
    }
}

#[test]
fn missing_later_stage_preserves_roll_prefix_and_scene_prevents_second_integration() {
    let mut f = Fixture::new();
    f.records().roll.as_mut().unwrap().impulse = 64;
    f.world.view_transition_mode = None;
    f.world.spawn_defaults = None;
    assert_eq!(
        f.advance(Default::default()),
        Err(SceneError::PlayerFlight(FlightError::Ambient(
            AmbientError::MissingViewMode
        )))
    );
    let roll = f.records().roll.unwrap();
    assert!(roll.impulse < 64 && roll.impulse > 0);
    assert_ne!(f.records().protection.unwrap().control(), 0);
    assert_eq!(f.advance(Default::default()), Err(SceneError::Faulted));
    assert_eq!(f.records().roll, Some(roll));
}

#[test]
fn free_flight_runs_live_stages_and_skips_only_the_disabled_boundary_dependencies() {
    let mut f = Fixture::new();
    f.records().boundary = None;
    f.records().occupancy = None;
    f.world.occupancy = None;
    f.world.fixed_players = [None; 2];
    let original_position = f.objects.get(f.owner).unwrap().base.position;
    f.records().motion.as_mut().unwrap().previous_position = original_position;
    let result = f.advance(Default::default()).unwrap();
    assert_eq!(result.surface_response.effect_events, 0);
    assert!(result.surface_motion.is_none());
    assert_eq!(result.diagonal_tie_bias, None);
    assert_eq!(f.records().ambient.unwrap().bank_phase, 1);
    assert_eq!(f.records().ambient.unwrap().offset_phase, 1);
    assert!(f.objects.get(f.owner).unwrap().base.speed > 0);
    assert_ne!(
        f.objects.get(f.owner).unwrap().base.position,
        original_position
    );
    assert_eq!(
        f.records().motion.unwrap().previous_position,
        original_position
    );
    assert_ne!(f.records().flight_displacement.unwrap(), Vector3::default());
    assert_eq!(
        f.objects.get(f.proxy).unwrap().base.speed,
        f.objects.get(f.owner).unwrap().base.speed
    );
}

#[test]
fn same_visit_surface_events_feed_locked_flight_thrust_and_effects_are_real_children() {
    let mut f = Fixture::new();
    f.world.surface_mode = Some(SurfaceMode { flags: 1 });
    f.records().surface.as_mut().unwrap().plane_height = -100;
    f.records().surface.as_mut().unwrap().material = 4;
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.records().auxiliary.as_mut().unwrap().action_flags = 4;
    let count = f.objects.len();
    let result = f.advance(Default::default()).unwrap();
    assert_eq!(result.surface_response.effect_events, 7);
    assert_eq!(f.records().speed.unwrap().thrust, 1);
    assert_eq!(f.objects.len(), count + 2);
    let child_count = f
        .objects
        .active_objects()
        .filter(|(_, actor)| actor.base.attachment == Some(f.owner))
        .count();
    assert_eq!(child_count, 2);
}

#[test]
fn rejected_surface_candidate_clears_diagonal_bias_before_grid_without_inventing_a_hit() {
    for admitted in [true, false] {
        let mut f = Fixture::new();
        f.world.view_transition_mode = Some(ViewTransitionMode { flags: 2 });
        f.world.reflect_all_contacts = Some(true);
        f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
            x: 600,
            y: 0,
            z: 600,
        };
        let mut collider = Object::new(
            ObjectKind::Effect,
            ShapeId::from_catalog_index(7),
            Behavior::Unassigned,
        );
        collider.base.position = Vector3 {
            x: 600,
            y: -300,
            z: 600,
        };
        collider.base.contacts.first_strategy_visit = !admitted;
        let collider = f.objects.allocate(collider).unwrap();
        let query = crate::collision_surface::query_object_surface_geometry(
            &f.objects,
            f.owner,
            0,
            SurfaceSearch::Full,
        )
        .unwrap();
        assert_eq!(query.broad_candidate_seen, admitted);
        assert_eq!(query.surface.contact.supporting_object, None);
        let coverage = MarkerCoverage::from_rectangle(WorldRectangle {
            x: 600,
            z: 600,
            width: 1,
            depth: 1,
        })
        .unwrap();
        f.world
            .occupancy
            .as_mut()
            .unwrap()
            .apply(&coverage, OccupancyChange::Mark);
        let result = f.advance(FlightContext {
            occupancy: OccupancyContext {
                camera_override_active: Some(false),
                diagonal_tie_bias: None,
            },
            ..Default::default()
        });
        if admitted {
            assert_eq!(result.unwrap().diagonal_tie_bias, Some(0));
            assert_eq!(f.records().occupancy.unwrap().current_cell, [1, 0]);
            assert_eq!(
                f.objects.get(f.owner).unwrap().base.position,
                Vector3 {
                    x: 600,
                    y: 0,
                    z: 511
                }
            );
        } else {
            assert_eq!(
                result,
                Err(SceneError::PlayerFlight(FlightError::Occupancy(
                    OccupancyError::MissingDiagonalTieBias
                )))
            );
        }
        assert!(f.objects.get(collider).is_some());
    }
}
