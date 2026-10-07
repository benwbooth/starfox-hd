use super::*;
use crate::path_invocation::InvocationWorld;
use crate::path_program::PathCatalog;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
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
    anchor: ObjectId,
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
        let anchor = allocate();
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
        Self {
            objects,
            world,
            execution,
            owner,
            proxy,
            anchor,
            catalog: PathCatalog::new(vec![]).unwrap(),
            callbacks: Callbacks,
        }
    }
    fn record(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
    }
}

#[test]
fn disabled_corridor_is_lazy_and_leveling_prefix_precedes_missing_boundary_and_fault_latches() {
    let mut fixture = Fixture::new();
    let owner = fixture.owner;
    fixture.record().boundary = None;
    fixture.record().steering = None;
    fixture.world.weapons = None;
    assert_eq!(fixture.host().advance_player_corridor(owner), Ok(false));
    fixture.record().auxiliary.as_mut().unwrap().action_flags = 4;
    fixture.record().auxiliary.as_mut().unwrap().mode = 0x13;
    fixture.objects.get_mut(owner).unwrap().base.position.y = -80;
    assert_eq!(
        fixture.host().advance_player_corridor(owner),
        Err(SceneError::PlayerBoundary(BoundaryError::MissingBoundary(
            owner
        )))
    );
    let pitch = player_storage::get(
        &fixture.objects,
        &fixture.execution.paths.runtime.resources,
        owner,
    )
    .unwrap()
    .fine_pitch;
    assert_eq!(pitch, sf_core::aim_angle::sf2_atan16(40, 400));
    assert_eq!(fixture.objects.get(owner).unwrap().base.position.y, -75);
    assert_eq!(
        fixture.host().advance_player_corridor(owner),
        Err(SceneError::Faulted)
    );
    assert_eq!(fixture.objects.get(owner).unwrap().base.position.y, -75);
}

#[test]
fn corridor_installation_changes_the_same_flag_used_by_path_displacement_and_steering() {
    let mut fixture = Fixture::new();
    let owner = fixture.owner;
    let anchor = fixture.anchor;
    fixture.world.primary_player = Some(owner);
    fixture.world.published_motion = Some(crate::path_motion::PublishedPlayerMotion {
        position: Default::default(),
        delta: Vector3 {
            x: 11,
            y: 13,
            z: 17,
        },
    });
    let selected = crate::path_control::PlayerTarget::Primary;
    assert!(
        !fixture
            .world
            .displacement(&fixture.objects, selected)
            .unwrap()
            .suppress_horizontal
    );
    fixture.objects.get_mut(anchor).unwrap().base.yaw = Angle::from_units(159);
    fixture.record().auxiliary.as_mut().unwrap().action_flags = 0x81;
    assert_eq!(
        fixture.host().install_player_corridor(
            anchor,
            owner,
            RegionInputs {
                half_width: 100,
                half_height: -1,
                activation_radius: 1000,
                ..Default::default()
            }
        ),
        Ok(true)
    );
    let displacement = fixture
        .world
        .displacement(&fixture.objects, selected)
        .unwrap();
    assert!(displacement.suppress_horizontal);
    assert_eq!(
        displacement.world_delta,
        Vector3 {
            x: 11,
            y: 13,
            z: 17
        }
    );
    assert_eq!(fixture.record().auxiliary.unwrap().action_flags, 0x85);
    assert_eq!(
        fixture.record().steering.unwrap().locked_heading.units(),
        128
    );
    fixture.record().boundary.as_mut().unwrap().return_position = Vector3 { x: 7, y: 53, z: 11 };
    fixture.objects.get_mut(owner).unwrap().base.position = Vector3 {
        x: 101,
        y: -73,
        z: 127,
    };
    assert_eq!(fixture.host().advance_player_corridor(owner), Ok(true));
    assert_eq!(
        fixture.objects.get(owner).unwrap().base.position,
        Vector3 {
            x: 100,
            y: -73,
            z: 127
        }
    );
    assert_eq!(
        fixture.record().boundary.unwrap().return_position,
        Vector3 {
            x: 100,
            y: 53,
            z: 127
        }
    );
    fixture.record().auxiliary.as_mut().unwrap().action_flags &= !4;
    assert!(
        !fixture
            .world
            .displacement(&fixture.objects, selected)
            .unwrap()
            .suppress_horizontal
    );
}

#[test]
fn containment_preserves_owner_but_clears_proxy_marker_even_on_zero_distance_rejection() {
    let mut fixture = Fixture::new();
    let corridor = Corridor {
        half_width: 100,
        half_height: -1,
        ..Default::default()
    };
    fixture
        .objects
        .get_mut(fixture.owner)
        .unwrap()
        .base
        .position
        .x = -100;
    fixture
        .objects
        .get_mut(fixture.proxy)
        .unwrap()
        .base
        .contacts
        .hit_marked = true;
    assert!(!contains(
        &mut fixture.objects,
        &fixture.world,
        fixture.owner,
        corridor
    )
    .unwrap());
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.position.x,
        -100
    );
    assert_eq!(
        fixture.objects.get(fixture.proxy).unwrap().base.position.x,
        -100
    );
    assert!(
        !fixture
            .objects
            .get(fixture.proxy)
            .unwrap()
            .base
            .contacts
            .hit_marked
    );
    fixture.world.weapons = None;
    assert!(!contains(
        &mut fixture.objects,
        &fixture.world,
        fixture.owner,
        Corridor {
            half_height: 0,
            ..corridor
        }
    )
    .unwrap());
    assert_eq!(
        contains(
            &mut fixture.objects,
            &fixture.world,
            fixture.owner,
            corridor
        ),
        Err(BoundaryError::MissingProxy)
    );
}

#[test]
fn region_active_gate_skips_proxy_and_partial_install_keeps_flag_before_missing_heading() {
    let mut fixture = Fixture::new();
    fixture.world.weapons = None;
    fixture.record().auxiliary.as_mut().unwrap().action_flags = 4;
    fixture.record().steering = None;
    let anchor = fixture.anchor;
    let owner = fixture.owner;
    let input = RegionInputs {
        activation_radius: 0,
        ..Default::default()
    };
    assert_eq!(
        fixture.host().install_player_corridor(anchor, owner, input),
        Ok(false)
    );
    assert_eq!(
        fixture.host().install_player_corridor(
            anchor,
            owner,
            RegionInputs {
                activation_radius: 100,
                ..input
            }
        ),
        Err(SceneError::PlayerBoundary(BoundaryError::MissingSteering(
            owner
        )))
    );
    assert_eq!(fixture.record().auxiliary.unwrap().action_flags, 4);
    assert_eq!(
        fixture.host().install_player_corridor(anchor, owner, input),
        Err(SceneError::Faulted)
    );

    let mut fixture = Fixture::new();
    let owner = fixture.owner;
    let anchor = fixture.anchor;
    fixture.record().steering = None;
    assert_eq!(
        fixture.host().install_player_corridor(
            anchor,
            owner,
            RegionInputs {
                half_width: 100,
                half_height: -1,
                activation_radius: 1000,
                ..Default::default()
            }
        ),
        Err(SceneError::PlayerBoundary(BoundaryError::MissingSteering(
            owner
        )))
    );
    assert_eq!(fixture.record().auxiliary.unwrap().action_flags, 4);
    assert_eq!(
        fixture.record().boundary.unwrap(),
        PlayerBoundary::default()
    );
    assert!(fixture.execution.is_faulted());
}
