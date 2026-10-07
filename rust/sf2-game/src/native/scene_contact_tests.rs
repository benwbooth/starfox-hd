use super::*;
use crate::collision_contacts::{self, ContactHost};
use crate::hit_response::HitResponseHost;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{
    ActionGate, PathCatalog, SelectedAuxiliaryState, SelectedParticleEffects,
};
use crate::program_resources::PROGRAM_CAPACITY;
use crate::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use crate::scene_strategy::SceneExecution;
use crate::strategy_schedule::{StrategyCompletion, StrategyHost};
use crate::{
    Angle, Behavior, Object, ObjectKind, ObjectSpawnDefaults, RandomState, ShapeId, Vector3,
};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("these contact tests must not enter an external strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("these contact tests must not enter death dispatch")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("these contact tests must not enter map continuation")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
    owner: ObjectId,
    other: ObjectId,
}

impl Scene {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(
            ObjectKind::Player,
            ShapeId::TITLE_CRAFT,
            Behavior::Unassigned,
        );
        actor.base.hit_points = 30;
        actor.base.contacts.first_strategy_visit = false;
        actor.base.contacts.pending_hit = true;
        actor.base.hit_flags = 0xFF;
        let owner = objects.allocate(actor).unwrap();
        let mut incoming = Object::new(
            ObjectKind::Projectile,
            ShapeId::from_catalog_index(7),
            Behavior::Unassigned,
        );
        incoming.base.hit_points = 20;
        incoming.base.attack_power = 5;
        incoming.base.target_speed = 77;
        incoming.base.contacts.damages_player_parts = true;
        incoming.base.position = Vector3 {
            x: 100,
            y: -30,
            z: 200,
        };
        let other = objects.allocate(incoming).unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([23, 71, 127, 219]));
        world.primary_player = Some(owner);
        world.contacts_enabled = Some(true);
        world.action_gate = Some(ActionGate::default());
        world.scene.player_configuration = Some(0);
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    contact: Some(PlayerContactControl::default()),
                    pose: Some(Default::default()),
                    charge: Some(Default::default()),
                    motion: Some(Default::default()),
                    protection: Some(DeflectionProtection::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x10,
                        action_flags: 0x91,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    particles: Some(SelectedParticleEffects { flags: 0x0D }),
                    ..PlayerPathRecords::default()
                },
            )
            .unwrap();
        world
            .contacts
            .record_pair(owner, other, [None, None])
            .unwrap();
        let mut execution = SceneExecution::default();
        execution.controls.paused = true;
        execution.hit_context.other_parameter = 71;
        install_player(&mut objects, &mut execution.paths.runtime.resources, owner).unwrap();
        Self {
            objects,
            world,
            execution,
            catalog: PathCatalog::new(Vec::new()).unwrap(),
            callbacks: Callbacks,
            owner,
            other,
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
    fn visit(&mut self) -> Result<StrategyCompletion, SceneError<&'static str>> {
        let owner = self.owner;
        self.host().run_strategy(owner, 17)
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn events(&mut self) -> Vec<SoundEvent> {
        self.world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .collect()
    }
}

#[test]
fn contact_gate_reads_live_objective_low_byte_instead_of_stale_initializer_flag() {
    use crate::path_scene_state::{CoordinationCommand, EncounterObjectiveCounts, ObjectiveCountField};
    for word in [0, 1, 0x00FF, 0x0100, 0xAB01, 0xFF00, 0xFFFF] {
        let mut scene = Scene::new();
        scene.world.contacts_enabled = Some(word as u8 == 0);
        scene.world.objective_counts = Some(EncounterObjectiveCounts {
            remaining_word: word, ..Default::default()
        });
        assert_eq!(PlayerContactHost::contacts_enabled(&scene.host()), Ok(word as u8 != 0));
        scene.world.objective_counts.as_mut().unwrap().apply(
            scene.objects.get_mut(scene.owner).unwrap(), ObjectiveCountField::Remaining,
            CoordinationCommand::Decrement);
        assert_eq!(PlayerContactHost::contacts_enabled(&scene.host()), Ok((word as u8).wrapping_sub(1) != 0));
        assert_eq!(scene.world.objective_counts.unwrap().remaining_word >> 8, word >> 8);
    }
}

#[test]
fn native_registrations_share_auxiliary_capacity_order_and_allocation_free_replacement() {
    let mut scene = Scene::new();
    let pool = &mut scene.execution.paths.runtime.resources;
    let table = &mut scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .auxiliary;
    assert_eq!(
        table.entries(pool, scene.owner).unwrap(),
        &[
            AuxiliaryRecord::NewContact(ContactCallback::Player),
            AuxiliaryRecord::ContinuingContact(ContactCallback::Player),
            AuxiliaryRecord::Separation(ContactCallback::Player),
        ]
    );
    // Payload 13, word rounding and both ownership/allocation headers => 18.
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 18);
    table
        .set(
            pool,
            scene.owner,
            AuxiliaryRecord::OrdinaryImpactMaterial(23),
        )
        .unwrap();
    // The first two retired tables coalesced into a 24-unit free block.
    // First fit takes that whole block for the 22-unit request because its
    // two-unit remainder is below the source split threshold.
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 24);
    let before = pool.clone();
    install_player(&mut scene.objects, pool, scene.owner).unwrap();
    assert_eq!(*pool, before);
    let owner = scene.owner;
    assert_eq!(
        scene
            .host()
            .has_hit_callback(owner, HitCallback::NewContact),
        Ok(true)
    );
    assert_eq!(
        scene
            .host()
            .has_hit_callback(owner, HitCallback::ContinuingContact),
        Ok(true)
    );
    let other = scene.other;
    assert_eq!(
        scene
            .host()
            .has_hit_callback(other, HitCallback::NewContact),
        Ok(false)
    );
}

#[test]
fn registration_growth_failure_keeps_the_source_ordered_partial_table() {
    let mut scene = Scene::new();
    let owner = scene.other;
    let mut pool = ProgramResources::default();
    pool.allocate_shared(
        PROGRAM_CAPACITY - 32 - 2,
        ProgramData::PathStack(Default::default()),
    )
    .unwrap();
    assert_eq!(
        install_player(&mut scene.objects, &mut pool, owner),
        Err(RegistrationError::Auxiliary(AuxiliaryError::Allocation(
            crate::program_resources::AllocationFailure::NoContiguousFit
        )))
    );
    // The 10- and 14-unit first two tables leave separate 10- and 8-unit
    // holes. The third request needs 18 contiguous units; aggregate capacity
    // is not enough. Neither the failed registration nor earlier ones vanish.
    assert_eq!(pool.available_capacity(), 18);
    assert_eq!(
        scene
            .objects
            .get(owner)
            .unwrap()
            .extension
            .auxiliary
            .entries(&pool, owner)
            .unwrap(),
        &[
            AuxiliaryRecord::NewContact(ContactCallback::Player),
            AuxiliaryRecord::ContinuingContact(ContactCallback::Player),
        ]
    );
}

#[test]
fn new_and_continuing_contacts_mutate_actual_health_auxiliary_and_shared_particle_byte() {
    let mut scene = Scene::new();
    scene.record().contact.as_mut().unwrap().hit.reserve_shield = 3;
    scene.visit().unwrap();
    assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 30);
    assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_flags, 0xF8);
    assert_eq!(scene.record().particles.unwrap().flags, 0xED);
    assert_eq!(scene.record().auxiliary.unwrap().action_flags, 0x91);
    let contact = scene.record().contact.unwrap();
    assert_eq!(contact.hit.reserve_shield, 0);
    assert_eq!(contact.hit.recovery, 10);
    assert_eq!(scene.record().pose.unwrap().heading_return_bank, -30);
    assert_eq!(scene.record().charge.unwrap().control, 0x40);
    assert_eq!(scene.record().pose.unwrap().yaw_trim.abs(), 64);
    assert_eq!(scene.record().motion.unwrap().lateral_impulse.abs(), 32);
    assert_eq!(scene.execution.hit_context.other_parameter, 71);
    assert_eq!(
        scene.events(),
        [SoundEvent::Authored(AuthoredCue::new(
            18,
            0,
            PlayerTarget::Primary
        ))]
    );
    scene.record().contact.as_mut().unwrap().hit.recovery = 0;
    scene.visit().unwrap();
    assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 25);
    assert_eq!(scene.execution.hit_context.other_parameter, 77);
    assert_eq!(scene.events().len(), 1);
    let mut path = scene
        .world
        .path_world(&scene.objects, scene.owner, PlayerTarget::Primary)
        .unwrap();
    let particle = path.selected_particle_effects.as_mut().unwrap();
    assert_eq!(particle.flags, 0xED);
    particle.flags = 7;
    path.selected_auxiliary.as_mut().unwrap().action_flags &= !1;
    let primary_feedback = path.primary_feedback.as_mut().unwrap();
    assert_eq!(primary_feedback.state, 0);
    primary_feedback.hit.feedback_duration = 23;
    drop(path);
    assert_eq!(scene.record().contact.unwrap().hit.feedback_duration, 23);
    // A path-side activity edit is observed by the very next real callback.
    scene.record().contact.as_mut().unwrap().hit.recovery = 0;
    scene.visit().unwrap();
    assert_eq!(scene.record().particles.unwrap().flags, 7);
    assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 25);
    assert!(scene.events().is_empty());
}

#[test]
fn secondary_contact_cues_and_primary_path_feedback_do_not_follow_the_same_selection() {
    let mut scene = Scene::new();
    scene.world.primary_player = Some(scene.other);
    scene.world.secondary_player = Some(scene.owner);
    scene
        .world
        .bind_player(
            &scene.objects,
            scene.other,
            PlayerPathRecords {
                contact: Some(PlayerContactControl {
                    hit: PlayerHitControl {
                        reserve_shield: 19,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    scene.visit().unwrap();
    assert_eq!(
        scene.events(),
        [SoundEvent::Authored(AuthoredCue::new(
            18,
            0,
            PlayerTarget::Secondary
        ))]
    );
    let mut path = scene
        .world
        .path_world(&scene.objects, scene.owner, PlayerTarget::Secondary)
        .unwrap();
    assert_eq!(path.selected_particle_effects.as_ref().unwrap().flags, 0xED);
    let primary = path.primary_feedback.as_mut().unwrap();
    assert_eq!(primary.state, 19);
    primary.hit.feedback_duration = 31;
    drop(path);
    assert_eq!(scene.record().contact.unwrap().hit.feedback_duration, 8);
    assert_eq!(
        scene
            .world
            .player(&scene.objects, scene.other)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .feedback_duration,
        31
    );
}

#[test]
fn separation_retains_shared_damage_and_parameter_without_subtracting_hull_health() {
    let mut scene = Scene::new();
    scene.record().contact.as_mut().unwrap().hit.reserve_shield = 15;
    scene.execution.hit_context.damage = 9;
    scene.execution.hit_context.other_parameter = 211;
    let first = scene.world.contacts.first(scene.owner).unwrap();
    let reciprocal = scene.world.contacts.get(first).unwrap().reciprocal;
    collision_contacts::separate_pair(&mut scene.host(), first).unwrap();
    assert!(scene.world.contacts.is_empty());
    assert_eq!(scene.record().contact.unwrap().hit.reserve_shield, 6);
    assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 30);
    assert_eq!(scene.execution.hit_context.damage, 0);
    assert_eq!(scene.execution.hit_context.other_parameter, 211);
    // The unregistered reverse side still publishes its cursor.
    assert_eq!(
        scene.execution.hit_context.current_contact,
        Some(reciprocal)
    );
    assert_eq!(
        scene.events(),
        [SoundEvent::Authored(AuthoredCue::new(
            18,
            0,
            PlayerTarget::Primary
        ))]
    );
}

#[test]
fn recovery_activity_and_global_gates_do_not_require_unreached_player_or_scene_inputs() {
    for gate in 0..4 {
        let mut scene = Scene::new();
        scene.world.scene.player_configuration = None;
        scene.record().protection = None;
        scene.record().particles = None;
        match gate {
            0 => {
                scene.record().contact.as_mut().unwrap().hit.recovery = 128;
                scene.record().auxiliary = None;
                scene.world.contacts_enabled = None;
                scene.world.action_gate = None;
            }
            1 => {
                scene.record().auxiliary.as_mut().unwrap().action_flags = 0;
                scene.world.contacts_enabled = None;
                scene.world.action_gate = None;
            }
            2 => {
                scene.world.contacts_enabled = Some(false);
                scene.world.action_gate = None;
            }
            _ => scene.world.action_gate = Some(ActionGate { code: 199 }),
        }
        scene.visit().unwrap();
        assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 30);
        assert!(
            !scene
                .objects
                .get(scene.owner)
                .unwrap()
                .base
                .contacts
                .hit_marked
        );
        assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_flags, 255);
        assert!(scene.events().is_empty());
    }
}

#[test]
fn missing_reached_input_and_stale_registration_latch_before_damage_and_cannot_replay() {
    for stale_table in [false, true] {
        let mut scene = Scene::new();
        if stale_table {
            scene
                .execution
                .paths
                .runtime
                .resources
                .release_owner(scene.owner);
        } else {
            scene.record().protection = None;
        }
        let error = scene.visit().unwrap_err();
        let SceneError::Hit(hit) = error else {
            panic!("unexpected error {error:?}");
        };
        if stale_table {
            assert_eq!(
                *hit,
                crate::hit_response::HitError::Host(SceneError::Auxiliary(
                    AuxiliaryError::MissingStorage
                ))
            );
        } else {
            assert_eq!(
                *hit,
                crate::hit_response::HitError::Host(SceneError::PlayerContact(Box::new(
                    player_contact::PlayerContactError::Host(SceneError::World(
                        WorldInputError::MissingPlayerProtection(scene.owner)
                    ))
                )))
            );
        }
        assert!(scene.execution.is_faulted());
        assert_eq!(scene.objects.get(scene.owner).unwrap().base.hit_points, 30);
        assert!(
            !scene
                .world
                .contacts
                .get(scene.world.contacts.first(scene.owner).unwrap())
                .unwrap()
                .new_contact
        );
        assert_eq!(scene.visit(), Err(SceneError::Faulted));
        assert!(scene.events().is_empty());
    }
}

#[test]
fn double_tap_roll_producer_drives_actual_projectile_reflection_at_both_boundaries() {
    use crate::player_roll::PlayerRoll;
    use crate::{Button, Buttons, InputState};

    for decays in [0, 1, 15, 16, 17] {
        let mut scene = Scene::new();
        scene.record().roll = Some(PlayerRoll::default());
        // Reflection's source gate admits this pending separation even
        // though the hit pass itself skips the owner.
        scene.objects.get_mut(scene.owner).unwrap().base.contacts.skip_contacts = true;
        scene.world.reflect_all_contacts = Some(false);
        scene.world.spawn_defaults = Some(ObjectSpawnDefaults::default());
        scene.world.weapons = Some(Default::default());
        scene.objects.get_mut(scene.other).unwrap().base.contacts.credits_hit_side = true;
        let owner = scene.owner;
        let shoulder = Button::LeftShoulder as u16;
        for (held, pressed) in [(shoulder, shoulder), (0, 0), (shoulder, shoulder)] {
            scene.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(held), pressed: Buttons::from_bits(pressed),
            });
            scene.host().prepare_player_shoulders(owner).unwrap();
            scene.host().advance_player_roll(owner).unwrap();
        }
        scene.world.processed_player_input = Some(InputState::default());
        for _ in 0..decays {
            scene.host().advance_player_roll(owner).unwrap();
        }
        let protected = (1..=16).contains(&decays);
        assert_eq!(scene.record().protection.unwrap().projectile_deflection(), protected);
        let mut expected_random = scene.world.random;
        if protected {
            for _ in 0..3 { expected_random.next_byte(); }
        }
        scene.execution.hit_context.damage = 5;
        let first = scene.world.contacts.first(owner).unwrap();
        let entry = *scene.world.contacts.get(first).unwrap();
        scene.host().on_separation(first, entry).unwrap();
        assert_eq!(scene.objects.len(), if protected { 3 } else { 2 });
        assert_eq!(scene.objects.get(scene.other).unwrap().base.flags.collision_disabled, protected);
        assert_eq!(scene.execution.hit_context.damage, if protected { 0 } else { 5 });
        assert_eq!(scene.world.random, expected_random);
        assert_eq!(scene.record().contact.unwrap().hit.feedback_flags & 8, 0);
        assert_eq!(scene.events(), [SoundEvent::Authored(AuthoredCue::new(
            if protected { 24 } else { 18 }, 0, PlayerTarget::Primary,
        ))]);
    }
}

#[test]
fn protection_feedback_reflection_sound_and_randomness_share_live_scene_owners() {
    for scatter in [false, true] {
        let mut scene = Scene::new();
        scene
            .objects
            .get_mut(scene.owner)
            .unwrap()
            .base
            .contacts
            .skip_contacts = true;
        scene
            .objects
            .get_mut(scene.other)
            .unwrap()
            .base
            .contacts
            .credits_hit_side = true;
        scene.objects.get_mut(scene.other).unwrap().base.pitch = Angle::from_units(29);
        scene.objects.get_mut(scene.other).unwrap().base.yaw = Angle::from_units(73);
        scene.record().protection = Some(DeflectionProtection::from_control(if scatter {
            0x40
        } else {
            1
        }));
        scene.world.reflect_all_contacts = Some(false);
        scene.world.spawn_defaults = Some(ObjectSpawnDefaults::default());
        scene.world.weapons = Some(Default::default());
        let mut expected_random = scene.world.random;
        let cooldown = expected_random.next_byte() & 7;
        if scatter {
            expected_random.next_byte();
            expected_random.next_byte();
        }
        let first = scene.world.contacts.first(scene.owner).unwrap();
        let entry = *scene.world.contacts.get(first).unwrap();
        // The hit pass skips this owner; the source separation callback still
        // invokes reflection while both contacts remain linked.
        scene.host().on_separation(first, entry).unwrap();
        assert_eq!(scene.world.contacts.len(), 2);
        assert_eq!(scene.objects.len(), 3);
        assert!(
            scene
                .objects
                .get(scene.other)
                .unwrap()
                .base
                .flags
                .collision_disabled
        );
        let reflected = *scene
            .objects
            .active_ids()
            .iter()
            .find(|&&id| id != scene.owner && id != scene.other)
            .unwrap();
        assert_eq!(
            scene.objects.get(reflected).unwrap().base.position,
            scene.objects.get(scene.other).unwrap().base.position
        );
        assert_eq!(
            scene.objects.get(reflected).unwrap().base.behavior,
            Behavior::FollowPath
        );
        assert_eq!(scene.world.random, expected_random);
        let hit = scene.record().contact.unwrap().hit;
        assert_eq!(hit.deflection_sound_cooldown, cooldown);
        assert_eq!(hit.feedback_flags & 8 != 0, !scatter);
        assert_eq!(
            scene.events(),
            [SoundEvent::Authored(AuthoredCue::new(
                24,
                0,
                PlayerTarget::Primary
            ))]
        );
    }
}

#[test]
fn retirement_dispatches_registered_separation_before_releasing_contact_and_program_pools() {
    let mut scene = Scene::new();
    scene.record().contact.as_mut().unwrap().hit.recovery = 1;
    let owner = scene.owner;
    scene.host().retire_object(owner).unwrap();
    assert!(scene.objects.get(owner).is_none());
    assert!(scene.world.contacts.is_empty());
    assert_eq!(
        scene.execution.paths.runtime.resources.owner_count(owner),
        0
    );
    assert_eq!(
        scene.execution.paths.runtime.resources.available_capacity(),
        PROGRAM_CAPACITY
    );
    let replacement = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    assert_eq!(replacement, owner);
    assert_eq!(
        scene.world.player(&scene.objects, replacement),
        Err(WorldInputError::StalePlayerRecord(owner))
    );
}
