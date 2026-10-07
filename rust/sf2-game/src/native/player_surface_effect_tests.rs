use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::{ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("effect escaped native scheduling")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("unexpected effect death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("unexpected map continuation")
    }
}

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    origin: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let origin = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut PathRuntime::default(),
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 32,
                score: Default::default(),
            },
        )
        .unwrap();
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 42,
            run_when_paused: false,
        });
        world
            .player_mut(&objects, owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = 0x11;
        objects.get_mut(origin).unwrap().base.position = Vector3 {
            x: 1000,
            y: -200,
            z: 3000,
        };
        Self {
            objects,
            world,
            owner,
            origin,
        }
    }
    fn input(&self) -> SurfaceEffectInputs {
        SurfaceEffectInputs {
            origin: Some(self.origin),
            lateral: Some(20),
            vertical: Some(-31),
            forward: Some(40),
        }
    }
    fn spawn(&mut self) -> SurfaceEffectResult {
        let input = self.input();
        spawn(&mut self.objects, &mut self.world, self.owner, input)
            .unwrap()
            .unwrap()
    }
}

#[test]
fn actual_origin_rotation_feeds_steering_while_owner_and_relative_parent_stay_distinct() {
    let mut f = Fixture::new();
    f.objects.get_mut(f.owner).unwrap().base.yaw = Angle::from_units(53);
    f.objects.get_mut(f.origin).unwrap().base.pitch = Angle::from_units(64);
    let mut random = f.world.random.clone();
    let jitter = [random.next_byte(), random.next_byte(), random.next_byte()]
        .map(|value| i16::from(value & RANDOM_MASK) - RANDOM_CENTER);
    let effect = f.spawn();
    let actor = f.objects.get(effect.object).unwrap();
    let (y, z) = rotate_8yz(64, -31, 40);
    let (x, z) = rotate_8xz(0, 20, z as i8);
    assert_eq!(
        actor.base.position,
        Vector3 {
            x: 1000 + x + jitter[0],
            y: -200 + y + jitter[1],
            z: 3000 + z + jitter[2]
        }
    );
    assert_eq!(effect.steering_response_target, y as u16);
    assert_eq!(actor.base.attachment, Some(f.owner));
    assert_eq!(actor.base.effect_origin, f.objects.lifetime_id(f.origin));
    assert_eq!(actor.extension.parent, Some(effect.object));
    assert_eq!(actor.base.yaw.units(), 53);
    assert_eq!(actor.base.pitch, Angle::ZERO);
    assert_eq!(f.world.random, random);
}

#[test]
fn real_child_limit_and_walker_mode_gates_skip_all_allocation_and_context_dependencies() {
    let mut f = Fixture::new();
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .mode = 0x24;
    let before = f.objects.clone();
    assert_eq!(
        spawn(&mut f.objects, &mut f.world, f.owner, Default::default()),
        Ok(None)
    );
    assert_eq!(f.objects, before);
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .motion
        .as_mut()
        .unwrap()
        .walker_contact_control = 5;
    for _ in 0..CHILD_LIMIT {
        f.spawn();
    }
    f.world.player_mut(&f.objects, f.owner).unwrap().auxiliary = None;
    f.world.spawn_defaults = None;
    let before = f.objects.clone();
    let random = f.world.random.clone();
    assert_eq!(
        spawn(&mut f.objects, &mut f.world, f.owner, Default::default()),
        Ok(None)
    );
    assert_eq!(f.objects, before);
    assert_eq!(f.world.random, random);
}

#[test]
fn missing_origin_keeps_formatted_attached_child_and_scene_retry_does_not_allocate() {
    let mut f = Fixture::new();
    let mut execution = SceneExecution::default();
    let mut host = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut execution,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        callbacks: &mut Callbacks,
        statement_budget: 16,
    };
    let random = host.world.random.clone();
    assert_eq!(
        host.spawn_player_surface_effect(f.owner, Default::default()),
        Err(SceneError::PlayerSurfaceEffect(
            SurfaceEffectError::MissingOrigin
        ))
    );
    let child = host.objects.get(f.owner).unwrap().base.first_child.unwrap();
    let actor = host.objects.get(child).unwrap();
    assert_eq!(actor.base.child_number, EFFECT_NUMBER);
    assert_eq!(
        actor.base.behavior,
        Behavior::SurfaceEffect(SurfaceEffectPhase::Initialize)
    );
    assert_eq!(actor.extension.spawn_group, 255);
    assert!(actor.base.flags.scaled_sprite && actor.base.flags.collision_disabled);
    assert_eq!(host.world.random, random);
    assert_eq!(
        host.spawn_player_surface_effect(f.owner, Default::default()),
        Err(SceneError::Faulted)
    );
    assert_eq!(host.objects.len(), 3);
}

#[test]
fn origin_retirement_retains_identity_and_slot_reuse_does_not_counterfeit_the_origin() {
    let mut f = Fixture::new();
    let effect = f.spawn().object;
    let origin_lifetime = f.objects.lifetime_id(f.origin).unwrap();
    f.objects.remove(f.origin).unwrap();
    assert_eq!(
        f.objects.get(effect).unwrap().base.effect_origin,
        Some(origin_lifetime)
    );
    let recycled = f
        .objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    assert_eq!(recycled, f.origin);
    assert_eq!(
        f.objects.get(effect).unwrap().base.effect_origin,
        Some(origin_lifetime)
    );
    let record = f.world.player_mut(&f.objects, f.owner).unwrap();
    record.mode_selection.as_mut().unwrap().surface_control = SURFACE_CARRY;
    record.motion.as_mut().unwrap().walker_motion_control = 1;
    step(&mut f.objects, &f.world, effect).unwrap();
    assert_eq!(
        step(&mut f.objects, &f.world, effect),
        Err(SurfaceEffectError::RetiredOrigin(origin_lifetime))
    );
    assert_eq!(
        f.objects.get(effect).unwrap().base.attachment,
        Some(f.owner)
    );
}

#[test]
fn scheduler_runs_first_visit_then_lifetime_during_pause_and_retires_the_real_child() {
    let mut f = Fixture::new();
    let effect = f.spawn().object;
    let mut execution = SceneExecution::default();
    execution.controls.paused = true;
    let mut schedule = StrategySchedule::default();
    let mut host = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut execution,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        callbacks: &mut Callbacks,
        statement_budget: 16,
    };
    for visit in 0..=EFFECT_LIFETIME {
        host.world.strategy_clock = u16::from(visit);
        host.world
            .player_mut(host.objects, f.owner)
            .unwrap()
            .surface
            .as_mut()
            .unwrap()
            .plane_height = 500 + i16::from(visit);
        host.execution.positional.begin_epoch();
        host.begin_strategy_epoch(&mut schedule).unwrap();
        schedule.run_overlapping(&mut host, || false).unwrap();
        schedule.run_remainder(&mut host).unwrap();
        let actor = host.objects.get(effect).unwrap();
        assert_eq!(actor.base.target_speed, EFFECT_LIFETIME - visit);
        if visit > 0 {
            assert_eq!(actor.base.position.y, 500 + i16::from(visit));
        }
        assert_eq!(actor.base.flags.remove_after_tick, visit == EFFECT_LIFETIME);
        host.clean_epoch().unwrap();
        assert_eq!(host.objects.get(effect).is_some(), visit != EFFECT_LIFETIME);
    }
    assert_eq!(host.objects.get(f.owner).unwrap().base.first_child, None);
    assert_eq!(host.objects.len(), 2);
}

#[test]
fn carried_effect_crossing_plane_marks_removal_before_lifetime_decrement() {
    let mut f = Fixture::new();
    let effect = f.spawn().object;
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .mode_selection
        .as_mut()
        .unwrap()
        .surface_control = SURFACE_CARRY;
    step(&mut f.objects, &f.world, effect).unwrap();
    let actor = f.objects.get_mut(effect).unwrap();
    actor.base.position.y = -200;
    actor.extension.path_state.script_value = (-201_i16) as u16;
    step(&mut f.objects, &f.world, effect).unwrap();
    let actor = f.objects.get(effect).unwrap();
    assert!(actor.base.flags.remove_after_tick);
    assert_eq!(actor.base.target_speed, EFFECT_LIFETIME);
    assert_eq!(actor.base.position.y, -204);
}

#[test]
fn full_pool_fails_before_formatting_but_after_admission_gates() {
    let mut f = Fixture::new();
    while f.objects.len() < OBJECT_CAPACITY {
        f.objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
    }
    let before = f.objects.clone();
    assert_eq!(
        spawn(&mut f.objects, &mut f.world, f.owner, Default::default()),
        Err(SurfaceEffectError::ObjectPoolExhausted)
    );
    assert_eq!(f.objects, before);
}
