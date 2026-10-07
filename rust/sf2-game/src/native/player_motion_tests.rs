use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
    owner: ObjectId,
    proxy: ObjectId,
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
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        world.scene.player_configuration = Some(0);
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world
            .player_mut(&objects, owner)
            .unwrap()
            .vertical
            .as_mut()
            .unwrap()
            .motion_axes = 0xE0;
        let actor = objects.get_mut(owner).unwrap();
        actor.base.speed = 42;
        actor.base.pitch = Angle::from_units(7);
        actor.base.yaw = Angle::from_units(13);
        actor.base.roll = Angle::from_units(21);
        Self {
            objects,
            world,
            runtime,
            owner,
            proxy,
        }
    }
    fn run(&mut self) -> Result<Option<SurfaceMotionResult>, MotionError> {
        advance(
            &mut self.objects,
            &mut self.world,
            &self.runtime.resources,
            self.owner,
            MotionContext::default(),
        )
    }
}

#[test]
fn negative_thrust_omits_vertical_delta_but_retains_base_velocity_and_real_proxy() {
    let mut fixture = Fixture::new();
    player_storage::get_mut(
        &fixture.objects,
        &mut fixture.runtime.resources,
        fixture.owner,
    )
    .unwrap()
    .fine_pitch = 64 << 8;
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .speed
        .as_mut()
        .unwrap()
        .thrust = -32;
    fixture.run().unwrap();
    let actor = fixture.objects.get(fixture.owner).unwrap();
    let proxy = fixture.objects.get(fixture.proxy).unwrap();
    let base = path_motion::direction_velocity(Angle::from_units(64), actor.base.yaw, 42, 1);
    assert_eq!(actor.base.velocity, base);
    assert_eq!(actor.base.position, base);
    assert_eq!(proxy.base.velocity, base);
    assert_eq!(proxy.base.pitch, Angle::from_units(64));
    assert_eq!(proxy.base.roll, actor.base.roll);
    assert_eq!(actor.base.pitch, Angle::from_units(7));
    assert_eq!(
        fixture
            .world
            .player(&fixture.objects, fixture.owner)
            .unwrap()
            .flight_displacement,
        Some(base)
    );
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .vertical
        .as_mut()
        .unwrap()
        .motion_axes = 0;
    let position = actor.base.position;
    fixture.run().unwrap();
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.position,
        position
    );
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.velocity,
        Vector3::default()
    );
    assert_eq!(
        fixture.objects.get(fixture.proxy).unwrap().base.velocity,
        base
    );
}

#[test]
fn scripted_view_skip_is_lazy_but_follows_protection_prefix() {
    let mut fixture = Fixture::new();
    fixture.world.view_transition_mode = Some(ViewTransitionMode { flags: 2 });
    fixture.world.weapons = None;
    fixture.world.scene.player_configuration = None;
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .speed = None;
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .vertical = None;
    let objects = fixture.objects.clone();
    assert_eq!(fixture.run(), Ok(None));
    assert_eq!(fixture.objects, objects);
    let record = fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap();
    record
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    record.vertical = Some(crate::player_vertical::PlayerVerticalControl {
        motion_axes: 0xFF,
        ..Default::default()
    });
    record.speed = Some(crate::player_speed::PlayerSpeed { thrust: -127 });
    fixture.world.player_carry_mode = Some(0);
    assert_eq!(fixture.run(), Err(MotionError::MissingEnvironmentPlane));
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 20);
    let record = fixture
        .world
        .player(&fixture.objects, fixture.owner)
        .unwrap();
    assert_eq!(record.vertical.unwrap().motion_axes, 0x5F);
    assert_eq!(record.speed.unwrap().thrust, 0);
    fixture.world.environment_plane_height = Some(-301);
    assert_eq!(fixture.run(), Ok(None));
    assert_eq!(
        fixture
            .world
            .player(&fixture.objects, fixture.owner)
            .unwrap()
            .speed
            .unwrap()
            .thrust,
        100
    );
}

#[test]
fn proxy_pose_is_published_before_missing_fine_pitch_and_base_copy_precedes_missing_axes() {
    let mut fixture = Fixture::new();
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .speed
        .as_mut()
        .unwrap()
        .thrust = 57;
    let resources = std::mem::take(&mut fixture.runtime.resources);
    assert_eq!(
        fixture.run(),
        Err(MotionError::Storage(PlayerStorageError::MissingStorage(
            fixture.owner
        )))
    );
    let proxy = fixture.objects.get(fixture.proxy).unwrap();
    assert_eq!(proxy.base.speed, 57);
    assert_eq!(proxy.base.pitch, Angle::from_units(7));
    assert_eq!(proxy.base.yaw, Angle::from_units(13));
    assert_eq!(proxy.base.roll, Angle::from_units(21));
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.position,
        Vector3::default()
    );
    fixture.runtime.resources = resources;
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .vertical = None;
    assert_eq!(
        fixture.run(),
        Err(MotionError::MissingVertical(fixture.owner))
    );
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.velocity,
        fixture.objects.get(fixture.proxy).unwrap().base.velocity
    );
    assert_ne!(
        fixture.objects.get(fixture.owner).unwrap().base.velocity,
        Vector3::default()
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("motion does not dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("motion does not dispatch death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("motion does not resume maps")
    }
}

#[test]
fn constrained_prefix_uses_shared_support_and_latches_before_missing_surface_input() {
    let mut fixture = Fixture::new();
    fixture.world.scene.player_configuration = Some(9);
    fixture.world.player_surface_support = Some(PlayerSurfaceSupport {
        object: Some(fixture.proxy),
        group: 197,
    });
    fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap()
        .motion
        .as_mut()
        .unwrap()
        .surface_velocity = [-3, 7];
    fixture
        .objects
        .get_mut(fixture.owner)
        .unwrap()
        .extension
        .surface_contact
        .flags = 0xA5;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = fixture.runtime;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut fixture.objects,
        world: &mut fixture.world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 1,
    };
    assert_eq!(
        scene.advance_player_motion(fixture.owner, MotionContext::default()),
        Err(SceneError::PlayerMotion(MotionError::MissingSurfaceMode))
    );
    let actor = scene.objects.get(fixture.owner).unwrap().clone();
    assert_eq!(actor.base.velocity, Vector3 { x: -3, y: 0, z: 7 });
    assert_eq!(
        actor.extension.surface_contact,
        crate::collision_surface::ActorSurfaceContact {
            supporting_object: Some(fixture.proxy),
            group: 197,
            flags: 0xA5
        }
    );
    scene.world.surface_mode = Some(Default::default());
    assert_eq!(
        scene.advance_player_motion(fixture.owner, MotionContext::default()),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.objects.get(fixture.owner), Some(&actor));
}
