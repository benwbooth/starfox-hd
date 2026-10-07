use super::*;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{ObjectSpawnDefaults, RandomState};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    objects.get_mut(owner).unwrap().base.hit_points = 1;
    let mut world = ScenePathWorld::new(RandomState::new([171, 37, 249, 62]));
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 93,
        run_when_paused: false,
    });
    world.contacts_enabled = Some(true);
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                particles: Some(Default::default()),
                auxiliary: Some(crate::path_program::SelectedAuxiliaryState {
                    mode: 0,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                roll: Some(Default::default()),
                motion: Some(Default::default()),
                charge: Some(Default::default()),
                contact: Some(Default::default()),
                pose: Some(Default::default()),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner)
}

#[test]
fn early_gates_skip_later_missing_inputs_and_age_write_precedes_missing_roll() {
    let (mut objects, mut world, owner) = fixture();
    world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    world.contacts_enabled = None;
    world.release_player_bindings(owner);
    advance(&mut objects, &mut world, owner).unwrap();
    world.spawn_defaults.as_mut().unwrap().run_when_paused = false;
    world.contacts_enabled = Some(false);
    advance(&mut objects, &mut world, owner).unwrap();
    world.contacts_enabled = Some(true);
    objects.get_mut(owner).unwrap().base.hit_points = 0;
    advance(&mut objects, &mut world, owner).unwrap();
    let (mut objects, mut world, owner) = fixture();
    let record = world.player_mut(&objects, owner).unwrap();
    record.particles = Some(SelectedParticleEffects {
        flags: 0x20,
        age: 255,
    });
    record.roll = None;
    assert_eq!(
        advance(&mut objects, &mut world, owner),
        Err(DamageEffectsError::MissingRoll(owner))
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .particles
            .unwrap()
            .age,
        0
    );
    world
        .player_mut(&objects, owner)
        .unwrap()
        .particles
        .as_mut()
        .unwrap()
        .age = 99;
    // Expiration bypasses the absent roll, mode, contact and charge owners.
    world.player_mut(&objects, owner).unwrap().auxiliary = None;
    advance(&mut objects, &mut world, owner).unwrap();
    assert_eq!(
        world.player(&objects, owner).unwrap().particles.unwrap(),
        SelectedParticleEffects { flags: 14, age: 0 }
    );
}

#[test]
fn tags_allow_shield_damage_without_feedback_and_zero_health_still_emits() {
    for recovery in [0, 0x40, 0x80, 0xC0] {
        let (mut objects, mut world, owner) = fixture();
        let record = world.player_mut(&objects, owner).unwrap();
        record.particles.as_mut().unwrap().flags = 0x20;
        record.contact.as_mut().unwrap().hit.recovery = recovery;
        record.contact.as_mut().unwrap().hit.reserve_shield = 1;
        advance(&mut objects, &mut world, owner).unwrap();
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 1);
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .contact
                .unwrap()
                .hit
                .reserve_shield,
            0
        );
        if recovery == 0 {
            let record = world.player_mut(&objects, owner).unwrap();
            assert_eq!(record.contact.unwrap().hit.recovery, 4);
            assert_eq!(record.pose.unwrap().heading_return_bank, 30);
            assert_eq!(record.charge.unwrap().control, 0x40);
            record.contact.as_mut().unwrap().hit.recovery = 0;
        } else {
            assert_eq!(
                world
                    .player(&objects, owner)
                    .unwrap()
                    .contact
                    .unwrap()
                    .hit
                    .recovery,
                recovery
            );
            assert_eq!(
                world
                    .player(&objects, owner)
                    .unwrap()
                    .pose
                    .unwrap()
                    .heading_return_bank,
                0
            );
        }
        advance(&mut objects, &mut world, owner).unwrap();
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
        assert_eq!(objects.len(), 3);
        let before = objects.clone();
        let random = world.random;
        advance(&mut objects, &mut world, owner).unwrap();
        assert_eq!(objects, before);
        assert_eq!(world.random, random);
    }
}

#[test]
fn low_countdown_puffs_are_fresh_signed_speed_children_with_primary_sound_and_no_birth_rng() {
    let (mut objects, mut world, owner) = fixture();
    world.primary_player = None;
    let random = world.random;
    for speed in [127, 128, 255, 1, 7] {
        objects.get_mut(owner).unwrap().base.speed = speed;
        let child = emit_puff(&mut objects, &mut world, owner).unwrap().unwrap();
        let actor = objects.get(child).unwrap();
        assert_eq!(
            actor.base.speed,
            ((speed as i8 >> 1) as i16 + (speed as i8 >> 2) as i16) as u8
        );
        assert_eq!(actor.base.child_number, 19);
        assert_eq!(actor.extension.parent, Some(child));
        assert_eq!(actor.base.attachment, Some(owner));
        assert_eq!(
            actor.base.path,
            Some(authored_paths::RANDOMIZED_COLOR_PARTICLE)
        );
        assert!(actor.extension.path_state.needs_path_initialization);
    }
    world.player_mut(&objects, owner).unwrap().motion = None;
    assert_eq!(emit_puff(&mut objects, &mut world, owner), Ok(None));
    assert_eq!(world.random, random);
    let cues: Vec<_> = world.audio.take_events().into_iter().flatten().collect();
    assert_eq!(
        cues,
        vec![SoundEvent::Authored(AuthoredCue::new(109, 0, PlayerTarget::Primary)); 5]
    );
}

#[test]
fn extinguishing_preserves_bit_four_and_decrements_tail_on_the_same_visit() {
    for cause in 0..3 {
        let (mut objects, mut world, owner) = fixture();
        world.strategy_clock = 1;
        let record = world.player_mut(&objects, owner).unwrap();
        record.particles = Some(SelectedParticleEffects {
            flags: 0xF1,
            age: if cause == 0 { 99 } else { 33 },
        });
        record.roll.as_mut().unwrap().impulse = if cause == 1 { -32 } else { 0 };
        record.auxiliary.as_mut().unwrap().mode = if cause == 2 { 0x2F } else { 0x10 };
        record.motion.as_mut().unwrap().walker_contact_control = 3;
        advance(&mut objects, &mut world, owner).unwrap();
        assert_eq!(
            world.player(&objects, owner).unwrap().particles.unwrap(),
            SelectedParticleEffects {
                flags: 0x1E,
                age: 0
            }
        );
        assert_eq!(objects.len(), 2);
        assert_eq!(
            objects
                .get(objects.get(owner).unwrap().base.attachment_next.unwrap())
                .unwrap()
                .base
                .child_number,
            19
        );
    }
}

#[test]
fn flame_randomness_precedes_mode_and_fatal_pool_checks_and_never_reuses_children() {
    let (mut objects, mut world, owner) = fixture();
    world.player_mut(&objects, owner).unwrap().auxiliary = None;
    let mut expected = world.random;
    expected.next_byte();
    assert_eq!(
        emit_flame(&mut objects, &mut world, owner, 0),
        Err(DamageEffectsError::World(
            WorldInputError::MissingAuxiliary(owner)
        ))
    );
    assert_eq!(world.random, expected);
    let (mut objects, mut world, owner) = fixture();
    for _ in 1..OBJECT_CAPACITY {
        emit_flame(&mut objects, &mut world, owner, 25).unwrap();
    }
    world.spawn_defaults = None;
    let mut expected = world.random;
    expected.next_byte();
    assert_eq!(
        emit_flame(&mut objects, &mut world, owner, 0),
        Err(DamageEffectsError::ObjectPoolExhausted)
    );
    assert_eq!(world.random, expected);
}

#[test]
fn ordered_flame_offsets_and_linked_override_are_independent_of_muzzle_disable() {
    for linked in [false, true] {
        let (mut objects, mut world, owner) = fixture();
        world.strategy_clock = 2;
        let record = world.player_mut(&objects, owner).unwrap();
        record.particles.as_mut().unwrap().flags = 0xE0;
        record.charge.as_mut().unwrap().linked_mode = linked;
        record.charge.as_mut().unwrap().linked_muzzle_disabled = true;
        let mut expected_random = world.random;
        let left = i16::from(expected_random.next_byte() & 15);
        let center = i16::from(expected_random.next_byte() & 15);
        advance(&mut objects, &mut world, owner).unwrap();
        assert_eq!(world.random, expected_random);
        let earlier = objects.get(owner).unwrap().base.attachment_next.unwrap();
        let latest = objects.get(earlier).unwrap().base.attachment_next.unwrap();
        for (id, sample, offset) in [(latest, center, 0), (earlier, left, -25)] {
            let actor = objects.get(id).unwrap();
            assert_eq!(
                actor.extension.relative_position,
                Vector3 {
                    x: sample - 7 + offset,
                    y: sample / 2 - if linked { 10 } else { 0 },
                    z: if linked { 30 } else { sample - 10 }
                }
            );
            assert_eq!(actor.extension.parent, None);
            assert_eq!(
                actor.base.path,
                Some(authored_paths::CHILD_DETACHING_SPRITE)
            );
        }
    }
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map")
    }
}

#[test]
fn scene_fault_retains_partial_age_and_does_not_repeat_mutations() {
    let (mut objects, mut world, owner) = fixture();
    let record = world.player_mut(&objects, owner).unwrap();
    record.particles.as_mut().unwrap().flags = 0x20;
    record.roll = None;
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let catalog = authored_paths::catalog();
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        callbacks: &mut callbacks,
        catalog: &catalog,
        statement_budget: 256,
    };
    assert_eq!(
        host.advance_player_damage_effects(owner),
        Err(SceneError::PlayerDamageEffects(
            DamageEffectsError::MissingRoll(owner)
        ))
    );
    assert!(host.execution.is_faulted());
    assert_eq!(
        host.advance_player_damage_effects(owner),
        Err(SceneError::Faulted)
    );
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .particles
            .unwrap()
            .age,
        1
    );
}

#[test]
fn authored_scenery_trigger_and_damage_consumer_share_the_live_selected_player_record() {
    use crate::strategy_schedule::StrategyHost;
    let (mut objects, mut world, owner) = fixture();
    world.primary_player = Some(owner);
    world.surface_mode = Some(crate::collision_surface::SurfaceMode { flags: 1 });
    world.audio_routing = Some(crate::scene_path_world::AudioRouting {
        listeners: [crate::path_sound::CueListener::PrimaryPlayer; 2],
        markers: Some(crate::path_sound::MarkerInputs {
            selected_sides: [PlayerTarget::Primary; 2],
            markers: [crate::path_sound::CueMarker {
                identity: crate::path_sound::CueListener::PrimaryPlayer,
                position: Vector3::default(),
                bearing: crate::Angle::ZERO,
            }; 2],
        }),
    });
    let record = world.player_mut(&objects, owner).unwrap();
    record.particles.as_mut().unwrap().age = 57;
    record.contact.as_mut().unwrap().hit.reserve_shield = 5;
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.path = Some(authored_paths::SELECTED_SCENERY_SPRITE_EMITTER);
    actor.base.hit_points = 10;
    actor.base.position.y = 60;
    let emitter = objects.allocate(actor).unwrap();
    let catalog = authored_paths::catalog();
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    for visit in 1..=25 {
        host.run_strategy(emitter, visit).unwrap();
    }
    assert!(
        host.objects
            .get(emitter)
            .unwrap()
            .base
            .flags
            .remove_after_tick
    );
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .particles
            .unwrap(),
        SelectedParticleEffects {
            flags: 0x20,
            age: 57
        }
    );
    host.world.strategy_clock = 32;
    host.advance_player_damage_effects(owner).unwrap();
    let record = host.world.player(host.objects, owner).unwrap();
    assert_eq!(
        record.particles.unwrap(),
        SelectedParticleEffects {
            flags: 0x20,
            age: 58
        }
    );
    assert_eq!(record.contact.unwrap().hit.reserve_shield, 4);
    let flame = host.objects.get(owner).unwrap().base.attachment_next.unwrap();
    assert_eq!(
        host.objects.get(flame).unwrap().base.path,
        Some(authored_paths::CHILD_DETACHING_SPRITE)
    );
}

#[test]
fn recycled_player_slots_cannot_reuse_the_previous_particle_record() {
    let (mut objects, mut world, owner) = fixture();
    objects.remove(owner).unwrap();
    let replacement = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    assert_eq!(replacement, owner);
    objects.get_mut(replacement).unwrap().base.hit_points = 1;
    assert_eq!(
        advance(&mut objects, &mut world, replacement),
        Err(DamageEffectsError::World(
            WorldInputError::StalePlayerRecord(replacement)
        ))
    );
}
