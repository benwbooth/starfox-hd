use super::*;
use crate::path_program::SelectedAuxiliaryState;
use crate::path_triggers::{Trigger, TriggerKind};
use crate::scene_path_world::PlayerPathRecords;
use crate::strategy_schedule::StrategySchedule;
use crate::{
    authored_paths, Angle, Object, ObjectKind, ObjectSpawnDefaults, PathCursor, PathId,
    RandomState, Rotation, ShapeId, SpatialLoop,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Assigned(ObjectId),
    DeathOverride(ObjectId),
    MapDeath(ObjectId, MapDeathCounts),
    Hit(ObjectId, ObjectId, HitCallback, u8, u8),
    Separate(ObjectId, usize),
}

#[derive(Default)]
struct Callbacks {
    events: Vec<Event>,
    assigned_return: Option<StrategyCompletion>,
    death_override: Option<StrategyCompletion>,
    hit_registration: Option<(ObjectId, HitCallback)>,
    hit_damage: Option<u8>,
    hit_health: Option<u8>,
    fail_hit: bool,
    map_registered: bool,
    map_suppresses_effects: bool,
}

impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        host.callbacks.events.push(Event::Assigned(owner));
        host.callbacks
            .assigned_return
            .ok_or("external strategy is unregistered")
    }
    fn death_override(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        host.callbacks.events.push(Event::DeathOverride(owner));
        Ok(host.callbacks.death_override)
    }
    fn has_hit_callback(
        host: &SceneActors<'_, Self>,
        owner: ObjectId,
        kind: HitCallback,
    ) -> Result<bool, SceneError<Self::Error>> {
        Ok(host.callbacks.hit_registration == Some((owner, kind)))
    }
    fn hit_callback(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
        other: ObjectId,
        kind: HitCallback,
        context: &mut HitContext,
    ) -> Result<(), SceneError<Self::Error>> {
        assert_eq!(host.callbacks.hit_registration, Some((owner, kind)));
        host.callbacks.events.push(Event::Hit(
            owner,
            other,
            kind,
            context.damage,
            context.other_parameter,
        ));
        if let Some(damage) = host.callbacks.hit_damage {
            context.damage = damage;
        }
        if let Some(health) = host.callbacks.hit_health {
            host.objects.get_mut(owner).unwrap().base.hit_points = health;
        }
        if host.callbacks.fail_hit {
            return Err(SceneError::Callbacks("hit callback fault"));
        }
        Ok(())
    }
    fn separation(
        host: &mut SceneActors<'_, Self>,
        _: ContactId,
        entry: Contact,
    ) -> Result<(), SceneError<Self::Error>> {
        assert!(host.objects.get(entry.owner).is_some());
        assert!(host.objects.get(entry.other).is_some());
        assert!(host.world.contacts.find(entry.owner, entry.other).is_some());
        let allocated = host
            .execution
            .paths
            .runtime
            .resources
            .owner_count(entry.owner);
        host.callbacks
            .events
            .push(Event::Separate(entry.owner, allocated));
        Ok(())
    }
    fn resume_map_on_death(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<(), Self::Error> {
        if !host.callbacks.map_registered {
            return Err("map continuation is unregistered");
        }
        host.callbacks
            .events
            .push(Event::MapDeath(owner, host.execution.map_counts.unwrap()));
        host.objects
            .get_mut(owner)
            .unwrap()
            .base
            .flags
            .suppress_death_effects = host.callbacks.map_suppresses_effects;
        Ok(())
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
}

impl Scene {
    fn new() -> Self {
        Self {
            objects: ObjectStore::new(),
            world: ScenePathWorld::new(RandomState::new([11, 17, 23, 29])),
            execution: SceneExecution::default(),
            catalog: authored_paths::catalog(),
            callbacks: Callbacks::default(),
        }
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 128,
        }
    }
    fn actor(&mut self, behavior: Behavior) -> ObjectId {
        let mut actor = Object::new(ObjectKind::Effect, ShapeId::TITLE_CRAFT, behavior);
        actor.base.hit_points = 1;
        self.objects.allocate(actor).unwrap()
    }
    fn loop_listener(&mut self) {
        self.execution.controls.loop_listener = Some(LoopListener {
            position: Vector3::default(),
            bearing: Angle::ZERO,
        });
    }
}

#[test]
fn missing_protection_gate_latches_after_the_source_spin_without_replaying_it() {
    use crate::path_program::{ProgramError, Statement};
    use crate::path_protection::ProtectionError;
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::FollowPath);
    let cursor = PathCursor { path: PathId::from_catalog_index(0), command_index: 0 };
    scene.objects.get_mut(owner).unwrap().base.path = Some(cursor);
    scene.catalog = PathCatalog::new(vec![vec![Statement::UpdateProtectionEffect {
        ordinary_return: cursor, flicker: cursor,
    }]]).unwrap();
    assert_eq!(scene.host().run_strategy(owner, 1),
        Err(SceneError::Path(InvocationError::Program(ProgramError::Protection(ProtectionError::MissingPlayerConfiguration)))));
    assert!(scene.execution.is_faulted());
    let actor = scene.objects.get(owner).unwrap();
    assert_eq!(actor.extension.relative_rotation.pitch, Angle::from_units(8));
    assert_eq!(actor.extension.relative_rotation.roll, Angle::from_units(6));
    assert_eq!(actor.base.path, Some(cursor));
    let after_failure = scene.objects.clone();
    scene.world.scene.player_configuration = Some(9);
    scene.world.action_gate = Some(crate::path_program::ActionGate { code: 1 });
    for _ in 0..3 {
        assert_eq!(scene.host().run_strategy(owner, 2), Err(SceneError::Faulted));
        assert_eq!(scene.objects, after_failure);
    }
}

#[test]
fn split_schedule_composes_authored_charge_callbacks_and_live_selected_records() {
    let mut scene = Scene::new();
    let player = scene.actor(Behavior::Unassigned);
    let orb = scene.actor(Behavior::FollowPath);
    scene.objects.get_mut(orb).unwrap().base.path = Some(authored_paths::PLAYER_CHARGE_ORB);
    scene.objects.get_mut(orb).unwrap().base.shape = ShapeId::EMPTY;
    scene.world.primary_player = Some(player);
    scene.world.active_charge_threshold = Some(25);
    scene
        .world
        .bind_player(
            &scene.objects,
            player,
            PlayerPathRecords {
                charge: Some(crate::player_charge::PlayerCharge {
                    linked_mode: false,
                    progress: 3 << 8,
                    ..Default::default()
                }),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    let mut schedule = StrategySchedule::default();
    for visit in 0..16u16 {
        if visit == 10 {
            scene
                .world
                .player_mut(&scene.objects, player)
                .unwrap()
                .charge
                .as_mut()
                .unwrap()
                .progress = 25 << 8;
        }
        scene.execution.positional.begin_epoch();
        scene.host().begin_strategy_epoch(&mut schedule).unwrap();
        let mut overlaps = 0;
        schedule
            .run_overlapping(&mut scene.host(), || {
                overlaps += 1;
                overlaps <= usize::from(visit % 3)
            })
            .unwrap();
        schedule.run_remainder(&mut scene.host()).unwrap();
        scene.host().clean_epoch().unwrap();
        assert_eq!(schedule.clock(), visit + 1);
        assert_eq!(scene.world.strategy_clock, visit + 1);
        let actor = scene.objects.get(orb).unwrap();
        assert_eq!(
            actor.base.shape,
            ShapeId::from_catalog_index(if visit < 2 {
                0
            } else if visit < 11 {
                15
            } else {
                17
            })
        );
        assert_eq!(
            actor.base.behavior,
            if visit < 11 {
                Behavior::FollowPath
            } else {
                Behavior::PathMovement
            }
        );
        assert!(!actor.base.contacts.first_strategy_visit);
        if visit >= 2 {
            assert_eq!(actor.extension.parent, Some(player));
        }
        assert!(scene.callbacks.events.is_empty());
        assert!(!scene.execution.is_faulted());
    }
    assert_eq!(scene.world.random.bytes(), [11, 17, 23, 29]);
    assert!(scene.execution.paths.runtime.resources.owner_count(orb) >= 2);
}

#[test]
fn paused_hit_response_uses_live_callback_health_and_still_samples_positional_sound() {
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Effect);
    let other = scene.actor(Behavior::Unassigned);
    let actor = scene.objects.get_mut(owner).unwrap();
    actor.base.hit_points = 10;
    actor.base.contacts.pending_hit = true;
    actor.base.contacts.latch_new_contact = true;
    actor.extension.spatial_loop = Some(SpatialLoop::CapitalEngine);
    actor.base.position.z = 100;
    scene.objects.get_mut(other).unwrap().base.attack_power = 9;
    scene
        .world
        .contacts
        .record_pair(owner, other, [None, None])
        .unwrap();
    scene.callbacks.hit_registration = Some((owner, HitCallback::NewContact));
    scene.callbacks.hit_damage = Some(3);
    scene.callbacks.hit_health = Some(20);
    scene.execution.hit_context.other_parameter = 77;
    scene.execution.controls.paused = true;
    scene.loop_listener();
    assert_eq!(
        scene.host().run_strategy(owner, 13).unwrap(),
        StrategyCompletion::keep(owner)
    );
    assert_eq!(
        scene.callbacks.events,
        [Event::Hit(owner, other, HitCallback::NewContact, 9, 77)]
    );
    let actor = scene.objects.get(owner).unwrap();
    assert_eq!(actor.base.hit_points, 17);
    assert!(actor.base.contacts.hit_marked && actor.base.contacts.new_contact_latched);
    assert!(!actor.base.contacts.first_strategy_visit);
    assert!(actor.base.contacts.pending_hit);
    assert_eq!(scene.execution.hit_context.damage, 3);
    assert_eq!(
        scene.execution.positional.pending().unwrap().sound.source,
        owner
    );
    scene.host().clean_epoch().unwrap();
    let actor = scene.objects.get(owner).unwrap();
    assert!(actor.base.contacts.previous_hit);
    assert!(!actor.base.contacts.pending_hit);
}

#[test]
fn scene_clear_keeps_contacts_programs_and_slots_until_normal_epoch_retirement() {
    let mut scene = Scene::new();
    let survivor = scene.actor(Behavior::Unassigned);
    let child = scene.actor(Behavior::Unassigned);
    let owner = scene.actor(Behavior::FollowPath);
    for id in [owner, child] {
        scene.objects.get_mut(id).unwrap().base.flags.general_search_eligible = true;
    }
    super::super::path_relationships::attach_fresh_child(&mut scene.objects, owner, child, 1).unwrap();
    scene.objects.get_mut(child).unwrap().base.flags.remove_with_parent = true;
    scene.objects.get_mut(survivor).unwrap().base.linked_object = Some(owner);
    let cursor = PathCursor { path: PathId::from_catalog_index(0), command_index: 0 };
    scene.execution.paths.runtime.add_trigger(
        &mut scene.objects, owner,
        Trigger { path: cursor, kind: TriggerKind::Always, timer: 0 },
    ).unwrap();
    let pair = scene.world.contacts.record_pair(owner, survivor, [None, None]).unwrap();
    scene.world.proxies.capture_actor(
        &mut scene.objects, owner, cursor, &scene.execution.paths.runtime.resources,
    ).unwrap().unwrap();
    let allocations = scene.execution.paths.runtime.resources.owner_count(owner);
    let contacts = pair.map(|id| scene.world.contacts.get(id).copied().unwrap());
    let resources = scene.execution.paths.runtime.resources.clone();
    let old_lifetime = scene.objects.lifetime_id(owner).unwrap();
    scene.host().clear_scene_actors().unwrap();
    assert_eq!(scene.objects.len(), 3);
    assert!(scene.world.proxies.is_empty());
    assert_eq!(scene.world.contacts.len(), 2);
    assert_eq!(pair.map(|id| scene.world.contacts.get(id).copied().unwrap()), contacts);
    assert_eq!(scene.execution.paths.runtime.resources, resources);
    assert!(scene.callbacks.events.is_empty());
    assert_eq!(scene.objects.get(survivor).unwrap().base.linked_object, Some(owner));
    assert_eq!(scene.objects.get(child).unwrap().base.attachment, Some(owner));
    scene.host().clean_epoch().unwrap();
    assert_eq!(scene.objects.active_ids(), &[survivor]);
    assert!(scene.world.contacts.is_empty());
    assert_eq!(scene.execution.paths.runtime.resources.owner_count(owner), 0);
    assert_eq!(scene.objects.get(survivor).unwrap().base.linked_object, None);
    assert_eq!(scene.callbacks.events, [Event::Separate(owner, allocations), Event::Separate(survivor, 0)]);
    let reused = scene.actor(Behavior::Unassigned);
    assert_eq!(reused, child);
    assert_eq!(scene.actor(Behavior::Unassigned), owner);
    assert_ne!(scene.objects.lifetime_id(owner), Some(old_lifetime));
}

#[test]
fn player_entry_reset_latches_after_world_clear_without_replaying_earlier_effects() {
    use crate::player_entry_reset::EntryResetError;
    use crate::player_motion_reset::MotionResetError;
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Unassigned);
    scene.objects.get_mut(owner).unwrap().base.flags.general_search_eligible = true;
    scene.world.spawn_defaults = Some(ObjectSpawnDefaults { group: 17, run_when_paused: true });
    scene.world.region_registration_count = Some(13);
    scene.world.secondary_region_group = Some(7);
    scene.world.render_environment.ambient_control = None;
    assert_eq!(scene.host().reset_player_entry(owner), Err(SceneError::PlayerEntryReset(EntryResetError::Motion(MotionResetError::MissingAmbientControl))));
    assert!(scene.execution.is_faulted());
    assert!(scene.objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert_eq!(scene.world.region_registration_count, Some(0));
    assert_eq!(scene.world.spawn_defaults.unwrap().group, 255);
    assert_eq!(scene.world.secondary_region_group, Some(255));
    assert_eq!(scene.world.occupancy, Some(crate::world_occupancy::WorldOccupancy::fully_occupied()));
    assert_eq!(scene.world.engine_sound_control, None);
    assert_eq!(scene.objects.get(owner).unwrap().base.shape, ShapeId::TITLE_CRAFT);
    scene.world.region_registration_count = Some(19);
    scene.world.render_environment.ambient_control = Some(crate::player_surface_render::AmbientParticleControl::from_bits(0xABCD));
    assert_eq!(scene.host().reset_player_entry(owner), Err(SceneError::Faulted));
    assert_eq!(scene.world.region_registration_count, Some(19));
    assert_eq!(scene.world.render_environment.ambient_control.unwrap().bits(), 0xABCD);
    assert!(scene.callbacks.events.is_empty());
}

#[test]
fn scene_world_reset_latches_a_partial_failure_against_outer_retry() {
    use crate::scene_world_reset::{RegionSelection, WorldResetError};
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Unassigned);
    scene.objects.get_mut(owner).unwrap().base.flags.general_search_eligible = true;
    scene.world.region_registration_count = Some(13);
    scene.world.secondary_region_group = Some(17);
    scene.world.spawn_defaults = None;
    scene.world.occupancy = None;
    assert_eq!(scene.host().clear_scene_world(RegionSelection::Clear), Err(SceneError::WorldReset(WorldResetError::MissingSpawnDefaults)));
    assert!(scene.execution.is_faulted());
    assert!(scene.objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert_eq!(scene.world.region_registration_count, Some(0));
    assert_eq!(scene.world.secondary_region_group, Some(17));
    assert_eq!(scene.world.occupancy, None);
    scene.world.spawn_defaults = Some(ObjectSpawnDefaults { group: 19, run_when_paused: true });
    scene.world.region_registration_count = Some(23);
    assert_eq!(scene.host().clear_scene_world(RegionSelection::Clear), Err(SceneError::Faulted));
    assert_eq!(scene.world.spawn_defaults.unwrap().group, 19);
    assert_eq!(scene.world.region_registration_count, Some(23));
    assert_eq!(scene.world.secondary_region_group, Some(17));
    assert_eq!(scene.world.occupancy, None);
}

#[test]
fn destruction_runs_map_accounting_then_full_contact_program_and_slot_retirement() {
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::FollowPath);
    let other = scene.actor(Behavior::Unassigned);
    let actor = scene.objects.get_mut(owner).unwrap();
    actor.base.hit_points = 0;
    actor.base.contacts.first_strategy_visit = false;
    actor.base.flags.tracked_map_actor = true;
    let cursor = PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: 0,
    };
    scene
        .execution
        .paths
        .runtime
        .add_trigger(
            &mut scene.objects,
            owner,
            Trigger {
                path: cursor,
                kind: TriggerKind::Always,
                timer: 0,
            },
        )
        .unwrap();
    scene
        .world
        .contacts
        .record_pair(owner, other, [None, None])
        .unwrap();
    let proxy = scene
        .world
        .proxies
        .capture_actor(
            &mut scene.objects,
            owner,
            cursor,
            &scene.execution.paths.runtime.resources,
        )
        .unwrap()
        .unwrap();
    scene.execution.map_counts = Some(MapDeathCounts {
        active: 0xAB00,
        destroyed: 0xCDFF,
    });
    scene.callbacks.map_registered = true;
    scene.execution.controls.death_effects = Some(EffectInputs {
        spawn: ObjectSpawnDefaults::default(),
        primary_marker: Vector3::default(),
        secondary_marker: None,
    });
    let allocations = scene.execution.paths.runtime.resources.owner_count(owner);
    assert_eq!(
        scene.host().run_strategy(owner, 1).unwrap(),
        StrategyCompletion::keep(owner)
    );
    assert_eq!(
        scene.callbacks.events,
        [
            Event::DeathOverride(owner),
            Event::MapDeath(
                owner,
                MapDeathCounts {
                    active: 0xABFF,
                    destroyed: 0xCD00
                }
            )
        ]
    );
    assert!(
        scene
            .objects
            .get(owner)
            .unwrap()
            .base
            .flags
            .remove_after_tick
    );
    assert!(scene.world.proxies.get(proxy).is_none());
    assert_eq!(
        scene.execution.paths.runtime.resources.owner_count(owner),
        allocations
    );
    assert_eq!(scene.world.contacts.len(), 2);
    let count = scene.objects.len();
    assert!(count > 2, "common death allocated its actual sprite");
    let events_before = scene.callbacks.events.len();
    scene.host().clean_epoch().unwrap();
    assert_eq!(scene.objects.len(), count - 1);
    assert!(scene.objects.get(owner).is_none());
    assert_eq!(
        scene.execution.paths.runtime.resources.owner_count(owner),
        0
    );
    assert!(scene.world.contacts.is_empty());
    assert_eq!(
        &scene.callbacks.events[events_before..],
        [
            Event::Separate(owner, allocations),
            Event::Separate(other, 0)
        ]
    );
}

#[test]
fn returned_actor_controls_sound_successor_and_immediate_full_retirement() {
    let mut scene = Scene::new();
    // Ordinary allocation prepends. Build owner -> returned -> tail.
    let tail = scene.actor(Behavior::Unassigned);
    let returned = scene.actor(Behavior::Unassigned);
    let owner = scene.actor(Behavior::Effect);
    scene
        .objects
        .get_mut(returned)
        .unwrap()
        .extension
        .spatial_loop = Some(SpatialLoop::CapitalEngine);
    scene.objects.get_mut(returned).unwrap().base.position.z = 100;
    scene.callbacks.assigned_return = Some(StrategyCompletion::retire(returned));
    scene.loop_listener();
    let mut schedule = StrategySchedule::default();
    scene.host().begin_strategy_epoch(&mut schedule).unwrap();
    schedule
        .run_overlapping(&mut scene.host(), || false)
        .unwrap();
    schedule.run_remainder(&mut scene.host()).unwrap();
    assert_eq!(scene.callbacks.events, [Event::Assigned(owner)]);
    assert!(scene.objects.get(owner).is_some());
    assert!(scene.objects.get(returned).is_none());
    assert!(
        !scene
            .objects
            .get(tail)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit
    );
    assert_eq!(
        scene.execution.positional.pending().unwrap().sound.source,
        returned
    );
    scene.execution.positional.publish(false);
    assert_eq!(
        scene.execution.positional.published().unwrap().control,
        0x8B
    );
}

#[test]
fn missing_death_inputs_latch_after_map_side_effects_instead_of_replaying_them() {
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Unassigned);
    let actor = scene.objects.get_mut(owner).unwrap();
    actor.base.hit_points = 0;
    actor.base.contacts.first_strategy_visit = false;
    actor.base.flags.tracked_map_actor = true;
    scene.callbacks.map_registered = true;
    scene.execution.map_counts = Some(MapDeathCounts {
        active: 3,
        destroyed: 7,
    });
    assert_eq!(
        scene.host().run_strategy(owner, 1),
        Err(SceneError::Destruction(Box::new(DestructionError::Host(
            SceneError::MissingDeathInputs
        ))))
    );
    let counts = scene.execution.map_counts;
    let actors = scene.objects.clone();
    let events = scene.callbacks.events.clone();
    for _ in 0..3 {
        assert_eq!(
            scene.host().run_strategy(owner, 2),
            Err(SceneError::Faulted)
        );
        assert_eq!(scene.host().clean_epoch(), Err(SceneError::Faulted));
        assert_eq!(scene.execution.map_counts, counts);
        assert_eq!(scene.objects, actors);
        assert_eq!(scene.callbacks.events, events);
    }
    assert_eq!(
        counts,
        Some(MapDeathCounts {
            active: 2,
            destroyed: 8
        })
    );
}

#[test]
fn null_strategy_skips_sound_but_excluded_actor_services_it_without_clearing_first_visit() {
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Unassigned);
    scene.objects.get_mut(owner).unwrap().extension.spatial_loop = Some(SpatialLoop::CapitalEngine);
    assert_eq!(
        scene.host().run_strategy(owner, 1).unwrap(),
        StrategyCompletion::keep(owner)
    );
    assert!(
        !scene
            .objects
            .get(owner)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit
    );
    assert_eq!(scene.execution.positional.pending(), None); // No marker required.
    scene
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .contacts
        .first_strategy_visit = true;
    scene.execution.controls.excluded_actor = Some(owner);
    scene.loop_listener();
    scene.host().run_strategy(owner, 2).unwrap();
    assert!(
        scene
            .objects
            .get(owner)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit
    );
    assert_eq!(
        scene.execution.positional.pending().unwrap().sound.source,
        owner
    );
    assert!(scene.callbacks.events.is_empty());
}

#[test]
fn destruction_animation_uses_primary_auxiliary_but_trailing_strategy_reads_none() {
    let mut scene = Scene::new();
    let player = scene.actor(Behavior::Unassigned);
    let effect = scene.actor(Behavior::Destruction(EffectPhase::Animate));
    scene.objects.get_mut(effect).unwrap().base.acceleration = 8;
    scene.world.primary_player = Some(player);
    scene
        .world
        .bind_player(
            &scene.objects,
            player,
            PlayerPathRecords {
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0x20,
                    action_flags: 0,
                    stored_world_position: Vector3::default(),
                    stored_rotation: Rotation::default(),
                }),
                ..PlayerPathRecords::default()
            },
        )
        .unwrap();
    // A non-scrolling primary mode does not require a displacement record.
    scene.host().run_strategy(effect, 1).unwrap();
    assert_eq!(scene.objects.get(effect).unwrap().base.target_speed, 1);
    scene.objects.get_mut(effect).unwrap().base.behavior =
        Behavior::Destruction(EffectPhase::Trail);
    scene.world.primary_player = None;
    scene.host().run_strategy(effect, 2).unwrap();
    assert_eq!(scene.objects.get(effect).unwrap().base.target_speed, 0);
    scene.host().run_strategy(effect, 3).unwrap();
    assert!(
        scene
            .objects
            .get(effect)
            .unwrap()
            .base
            .flags
            .remove_after_tick
    );
}

#[test]
fn empty_and_suspended_passes_still_publish_exactly_one_strategy_clock() {
    let mut scene = Scene::new();
    let mut schedule = StrategySchedule::default();
    scene.host().begin_strategy_epoch(&mut schedule).unwrap();
    assert_eq!(scene.world.strategy_clock, 1);
    assert_eq!(
        scene.host().begin_strategy_epoch(&mut schedule),
        Err(ScheduleError::EpochAlreadyActive)
    );
    assert_eq!(scene.world.strategy_clock, 1);
    schedule
        .run_overlapping(&mut scene.host(), || true)
        .unwrap();
    schedule.run_remainder(&mut scene.host()).unwrap();
    let owner = scene.actor(Behavior::Effect);
    scene
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .flags
        .strategy_suspended = true;
    scene.objects.get_mut(owner).unwrap().extension.spatial_loop = Some(SpatialLoop::CapitalEngine);
    scene.host().begin_strategy_epoch(&mut schedule).unwrap();
    assert_eq!(scene.world.strategy_clock, 2);
    schedule
        .run_overlapping(&mut scene.host(), || true)
        .unwrap();
    schedule.run_remainder(&mut scene.host()).unwrap();
    assert!(
        scene
            .objects
            .get(owner)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit
    );
    assert!(scene.callbacks.events.is_empty());
    assert_eq!(scene.execution.positional.pending(), None);
}

#[test]
fn contact_callback_failure_retains_context_and_consumption_without_replaying_mutation() {
    let mut scene = Scene::new();
    let owner = scene.actor(Behavior::Unassigned);
    let other = scene.actor(Behavior::Unassigned);
    scene
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .contacts
        .pending_hit = true;
    scene.objects.get_mut(other).unwrap().base.attack_power = 9;
    let contact = scene
        .world
        .contacts
        .record_pair(owner, other, [None, None])
        .unwrap()[0];
    scene.callbacks.hit_registration = Some((owner, HitCallback::NewContact));
    scene.callbacks.hit_health = Some(20);
    scene.callbacks.hit_damage = Some(3);
    scene.callbacks.fail_hit = true;
    assert_eq!(
        scene.host().run_strategy(owner, 1),
        Err(SceneError::Hit(Box::new(HitError::Host(
            SceneError::Callbacks("hit callback fault")
        ))))
    );
    assert_eq!(scene.objects.get(owner).unwrap().base.hit_points, 20); // Damage was not yet applied.
    assert!(!scene.world.contacts.get(contact).unwrap().new_contact);
    assert_eq!(scene.execution.hit_context.current_contact, Some(contact));
    assert_eq!(scene.execution.hit_context.damage, 3);
    let before = scene.objects.clone();
    assert_eq!(
        scene.host().run_strategy(owner, 2),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.objects, before);
    assert_eq!(scene.callbacks.events.len(), 1);
}
