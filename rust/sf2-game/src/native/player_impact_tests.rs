use super::*;
use crate::collision_surface::SurfaceMode;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::player_surface_damage::{self, SurfaceDamageError};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
use crate::{Angle, Behavior, Object, ObjectKind, RandomState, ShapeId, Vector3};

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
                reserve_shield: 32,
                score: Default::default(),
            },
        )
        .unwrap();
        world.scene.player_configuration = Some(0);
        world.surface_mode = Some(SurfaceMode { flags: 1 });
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            objects,
            world,
            runtime,
            owner,
            proxy,
        }
    }
    fn records(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn damage(&mut self) -> Result<(), SurfaceDamageError> {
        player_surface_damage::advance(
            &mut self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
        )
    }
}

#[test]
fn contact_turn_and_impact_are_consumed_by_the_actual_pose_and_charge_owners() {
    let mut fixture = Fixture::new();
    fixture
        .objects
        .get_mut(fixture.owner)
        .unwrap()
        .extension
        .path_state
        .motion_phase = 0xABCD;
    publish_turn(
        &mut fixture.objects,
        &mut fixture.world,
        fixture.owner,
        ContactTurn {
            target_yaw: Angle::from_units(37),
            yaw_impulse: 64,
            lateral_impulse: 32,
        },
    )
    .unwrap();
    assert_eq!(
        fixture
            .objects
            .get(fixture.owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase,
        0xAB25
    );
    assert_eq!(fixture.records().pose.unwrap().yaw_trim, 64);
    assert_eq!(fixture.records().motion.unwrap().lateral_impulse, 32);
    fixture.records().charge.as_mut().unwrap().control = 0xBF;
    fixture.world.strategy_clock = 1;
    impact(
        &fixture.objects,
        &mut fixture.world,
        fixture.owner,
        Impact::Heavy,
    )
    .unwrap();
    assert_eq!(fixture.records().charge.unwrap().control, 0x7F);
    fixture.records().auxiliary.as_mut().unwrap().mode = 0x10;
    fixture.world.player_pitch_target = Some(0);
    fixture.world.player_yaw_increment = Some(0);
    crate::player_pose::compose(
        &mut fixture.objects,
        &mut fixture.world,
        &mut fixture.runtime.resources,
        fixture.owner,
    )
    .unwrap();
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.yaw.units(),
        48
    );
    assert_eq!(
        fixture
            .objects
            .get(fixture.owner)
            .unwrap()
            .base
            .roll
            .units(),
        (-30_i8) as u8
    );
    assert_eq!(fixture.records().pose.unwrap().heading_return_bank, -30);
}

#[test]
fn impact_rejection_is_lazy_and_missing_consumers_preserve_the_exact_prefix() {
    let mut fixture = Fixture::new();
    fixture.records().contact.as_mut().unwrap().hit.recovery = 9;
    fixture.records().charge = None;
    fixture.records().pose = None;
    let before = *fixture.records();
    assert_eq!(
        impact(
            &fixture.objects,
            &mut fixture.world,
            fixture.owner,
            Impact::Light
        ),
        Ok(false)
    );
    assert_eq!(*fixture.records(), before);
    assert_eq!(
        impact(
            &fixture.objects,
            &mut fixture.world,
            fixture.owner,
            Impact::Heavy
        ),
        Err(ImpactError::World(WorldInputError::MissingPlayerCharge(
            fixture.owner
        )))
    );
    let hit = fixture.records().contact.unwrap().hit;
    assert_eq!(
        (
            hit.recovery,
            hit.feedback_duration,
            hit.feedback_flags,
            hit.camera_pitch_recoil
        ),
        (10, 8, 0x70, 128)
    );
    fixture.records().charge = Some(crate::player_charge::PlayerCharge {
        control: 0xAF,
        ..Default::default()
    });
    assert_eq!(
        impact(
            &fixture.objects,
            &mut fixture.world,
            fixture.owner,
            Impact::Heavy
        ),
        Err(ImpactError::MissingPose(fixture.owner))
    );
    assert_eq!(fixture.records().charge.unwrap().control, 0x6F);
}

#[test]
fn recoil_uses_single_rounding_and_only_nonzero_impulses_integrate_retained_y() {
    let mut fixture = Fixture::new();
    let owner = fixture.objects.get_mut(fixture.owner).unwrap();
    owner.base.velocity = Vector3 { x: 5, y: 9, z: 11 };
    owner.base.yaw = Angle::from_units(192);
    owner.extension.path_state.motion_phase = 0x1234;
    let before = owner.clone();
    advance_recoil(&mut fixture.objects, &mut fixture.world, fixture.owner).unwrap();
    assert_eq!(fixture.objects.get(fixture.owner).unwrap(), &before);
    fixture.records().motion.as_mut().unwrap().lateral_impulse = 32;
    advance_recoil(&mut fixture.objects, &mut fixture.world, fixture.owner).unwrap();
    let owner = fixture.objects.get(fixture.owner).unwrap();
    assert_eq!(owner.base.velocity, Vector3 { x: 0, y: 9, z: 31 });
    assert_eq!(owner.base.position, owner.base.velocity);
    assert_eq!(owner.extension.path_state.motion_phase, 0x2000);
    assert_eq!(fixture.records().motion.unwrap().lateral_impulse, 12);
    advance_recoil(&mut fixture.objects, &mut fixture.world, fixture.owner).unwrap();
    assert_eq!(fixture.records().motion.unwrap().lateral_impulse, 0);
    assert_eq!(
        fixture.objects.get(fixture.owner).unwrap().base.position.y,
        18
    );
}

#[test]
fn unsupported_forecast_clears_only_obstruction_and_preserves_first_probe_group_and_height() {
    let mut fixture = Fixture::new();
    fixture.world.scene.player_configuration = Some(9);
    fixture.records().protection = None;
    fixture.records().contact = None;
    fixture.records().motion.as_mut().unwrap().contact_flags = 0xFF;
    fixture.records().motion.as_mut().unwrap().previous_position = Vector3 {
        x: i16::MAX,
        y: 2,
        z: -3,
    };
    fixture.records().flight_displacement = Some(Vector3 { x: 1, y: -2, z: 3 });
    fixture
        .objects
        .get_mut(fixture.owner)
        .unwrap()
        .extension
        .surface_contact
        .group = 173;
    fixture.damage().unwrap();
    assert_eq!(fixture.world.player_surface_height, Some(0));
    assert_eq!(fixture.records().motion.unwrap().contact_flags, 0xEF);
    assert_eq!(
        fixture
            .objects
            .get(fixture.owner)
            .unwrap()
            .extension
            .surface_contact
            .group,
        173
    );
    assert_eq!(
        fixture.objects.get(fixture.proxy).unwrap().base.position,
        Vector3 {
            x: i16::MIN,
            y: 0,
            z: 0
        }
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not a strategy dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not death dispatch")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not map dispatch")
    }
}

#[test]
fn missing_forecast_displacement_faults_after_first_probe_and_cannot_be_retried() {
    let mut fixture = Fixture::new();
    fixture.world.scene.player_configuration = Some(9);
    fixture.records().flight_displacement = None;
    fixture
        .objects
        .get_mut(fixture.owner)
        .unwrap()
        .extension
        .surface_contact
        .flags = 255;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = fixture.runtime;
    let mut callbacks = Callbacks;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut host = SceneActors {
        objects: &mut fixture.objects,
        world: &mut fixture.world,
        execution: &mut execution,
        callbacks: &mut callbacks,
        catalog: &catalog,
        statement_budget: 32,
    };
    assert_eq!(
        host.advance_player_surface_damage(fixture.owner),
        Err(SceneError::PlayerSurfaceDamage(
            SurfaceDamageError::MissingFlightDisplacement(fixture.owner)
        ))
    );
    assert_eq!(host.world.player_surface_height, Some(0));
    assert_eq!(
        host.objects
            .get(fixture.owner)
            .unwrap()
            .extension
            .surface_contact
            .flags,
        0
    );
    let objects = host.objects.clone();
    assert_eq!(
        host.advance_player_surface_damage(fixture.owner),
        Err(SceneError::Faulted)
    );
    assert_eq!(*host.objects, objects);
}
