use super::*;
use crate::path_program::PathCatalog;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::{Angle, ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("surface particle escaped native dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected surface particle death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.hit_points = 30;
    actor.base.position = Vector3 {
        x: i16::MAX,
        y: 51,
        z: i16::MIN,
    };
    actor.base.pitch = Angle::from_units(123);
    actor.extension.texture_scroll_x = 99;
    let mut world = ScenePathWorld::new(RandomState::default());
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 42,
        run_when_paused: false,
    });
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                flight_displacement: Some(Vector3 {
                    x: 17,
                    y: 192,
                    z: -13,
                }),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner)
}

fn input() -> ParticleInputs {
    ParticleInputs {
        lateral: 20,
        forward: -3,
        size: 2,
        frame: 0,
    }
}

#[test]
fn fresh_particles_append_duplicate_numbers_and_keep_independent_transform_parent() {
    let (mut objects, world, owner) = fixture();
    let first = spawn(&mut objects, &world, owner, SurfaceParticle::Short, input()).unwrap();
    let second = spawn(&mut objects, &world, owner, SurfaceParticle::Long, input()).unwrap();
    assert_eq!(objects.get(owner).unwrap().base.first_child, Some(first));
    assert_eq!(objects.get(first).unwrap().base.next_sibling, Some(second));
    assert_eq!(objects.get(owner).unwrap().extension.texture_scroll_x, 4);
    for id in [first, second] {
        let actor = objects.get(id).unwrap();
        assert_eq!(actor.base.child_number, 16);
        assert_eq!(actor.base.attachment, Some(owner));
        assert_eq!(actor.extension.parent, Some(id));
        assert!(actor.base.flags.remove_with_parent);
        assert!(actor.base.flags.exclude_from_shape_footprint_search);
        assert!(actor.base.flags.collision_disabled);
        assert!(actor.base.flags.scaled_sprite);
        assert!(!actor.base.flags.general_search_eligible);
        assert!(actor.base.contacts.run_when_paused);
        assert_eq!(actor.extension.spawn_group, 42);
        assert_eq!(actor.extension.texture_scroll_x, 2);
        assert_eq!(actor.base.pitch.units(), 123);
        assert_eq!(
            actor.base.velocity,
            Vector3 {
                x: 17,
                y: 0,
                z: -13
            }
        );
        assert!(actor.base.path.is_none());
    }
}

#[test]
fn missing_displacement_faults_after_installation_but_pool_exhaustion_precedes_all_input_reads() {
    let (mut objects, mut world, owner) = fixture();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .flight_displacement = None;
    assert_eq!(
        spawn(&mut objects, &world, owner, SurfaceParticle::Short, input()),
        Err(ParticleError::MissingFlightDisplacement(owner))
    );
    let child = objects.get(owner).unwrap().base.first_child.unwrap();
    assert_eq!(objects.get(child).unwrap().extension.parent, Some(child));
    assert_eq!(objects.get(owner).unwrap().extension.texture_scroll_x, 99);
    assert_eq!(
        objects.get(child).unwrap().base.velocity,
        Vector3::default()
    );
    while objects.len() < OBJECT_CAPACITY {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
    }
    world.spawn_defaults = None;
    let before = objects.clone();
    assert_eq!(
        spawn(&mut objects, &world, owner, SurfaceParticle::Long, input()),
        Err(ParticleError::ObjectPoolExhausted)
    );
    assert_eq!(objects, before);
}

#[test]
fn terminal_animation_preserves_motion_and_invalid_behavior_changes_nothing() {
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        let (mut objects, world, owner) = fixture();
        let child = spawn(&mut objects, &world, owner, kind, input()).unwrap();
        let actor = objects.get_mut(child).unwrap();
        actor
            .extension
            .path_state
            .animation
            .shape
            .initialize(kind.frames() - 1);
        let mut expected = actor.clone();
        expected.base.flags.remove_after_tick = true;
        step(actor).unwrap();
        assert_eq!(*actor, expected);
        actor.base.behavior = Behavior::Unassigned;
        let before = actor.clone();
        assert_eq!(step(actor), Err(WrongParticleBehavior));
        assert_eq!(*actor, before);
    }
}

#[test]
fn real_scheduler_dispatches_both_lifetimes_during_pause_then_detaches_and_retires() {
    for kind in [SurfaceParticle::Short, SurfaceParticle::Long] {
        let (mut objects, mut world, owner) = fixture();
        let child = spawn(&mut objects, &world, owner, kind, input()).unwrap();
        let mut execution = SceneExecution::default();
        execution.controls.paused = true;
        let catalog: PathCatalog = crate::authored_paths::catalog();
        let mut callbacks = Callbacks;
        let mut schedule = StrategySchedule::default();
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 16,
        };
        for visit in 1..=kind.frames() {
            host.execution.positional.begin_epoch();
            host.begin_strategy_epoch(&mut schedule).unwrap();
            schedule.run_overlapping(&mut host, || false).unwrap();
            schedule.run_remainder(&mut host).unwrap();
            assert_eq!(
                host.objects
                    .get(child)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick,
                visit == kind.frames()
            );
            host.clean_epoch().unwrap();
            assert_eq!(host.objects.get(child).is_some(), visit < kind.frames());
        }
        assert_eq!(host.objects.get(owner).unwrap().base.first_child, None);
        // Detaching the final child clears its link, not the parent's
        // independently retained child-chain update flag.
        assert!(
            host.objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .motion
                .refresh_child_chain
        );
        assert_eq!(host.objects.len(), 1);
    }
}

#[test]
fn scene_wrapper_latches_partial_particle_installation_without_allocating_on_retry() {
    let (mut objects, mut world, owner) = fixture();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .flight_displacement = None;
    let mut execution = SceneExecution::default();
    let catalog = crate::authored_paths::catalog();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 16,
    };
    assert_eq!(
        host.spawn_player_surface_particle(owner, SurfaceParticle::Short, input()),
        Err(SceneError::PlayerSurfaceParticle(
            ParticleError::MissingFlightDisplacement(owner)
        ))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(host.objects.len(), 2);
    let before = host.objects.clone();
    assert_eq!(
        host.spawn_player_surface_particle(owner, SurfaceParticle::Long, input()),
        Err(SceneError::Faulted)
    );
    assert_eq!(*host.objects, before);
}
