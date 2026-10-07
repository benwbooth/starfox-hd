use super::*;
use crate::actor_auxiliary::{AuxiliaryKind, AuxiliaryRecord};
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_program::PathCatalog;
use crate::path_shots::ActiveShots;
use crate::program_resources::PROGRAM_CAPACITY;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, InputState, Object, ObjectKind, ObjectSpawnDefaults, RandomState, ShapeId};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("player-storage prefix does not dispatch an unported mode")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

fn inputs(pilot_code: u8) -> PlayerStorageInputs {
    PlayerStorageInputs {
        pilot_code,
        reserve_shield: 173,
        score: PlayerScore::from_parts(0xABCD, 0xEF),
    }
}

fn owner(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap()
}

#[test]
fn replacement_initializes_every_supported_player_record_without_initializing_the_scene() {
    let mut objects = ObjectStore::new();
    let primary = owner(&mut objects);
    let other = owner(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::new([1, 2, 3, 4]));
    let mut runtime = PathRuntime::default();
    world.primary_player = Some(other);
    world.secondary_player = Some(other);
    world.fixed_players = [Some(other), Some(primary)];
    world.active_shield_capacity = Some(11);
    world.active_charge_threshold = Some(53);
    let actor = objects.get_mut(primary).unwrap();
    actor.base.pitch = Angle::from_units(29);
    actor.base.yaw = Angle::from_units(157);
    actor.base.roll = Angle::from_units(243);
    actor.extension.path_state.script_value = 391;
    actor.extension.path_state.hold_latched = true;
    actor.base.path = Some(crate::authored_paths::PLAYER_CHARGE_ORB);
    actor
        .extension
        .auxiliary
        .set(
            &mut runtime.resources,
            primary,
            AuxiliaryRecord::ReflectionShape(ShapeId::EMPTY),
        )
        .unwrap();
    let other_data = runtime
        .resources
        .allocate_owned(other, 100, ProgramData::PathStack(Default::default()))
        .unwrap();
    let old_actor = actor.clone();
    let other_before = objects.get(other).unwrap().clone();
    replace(&mut objects, &mut world, &mut runtime, primary, inputs(255)).unwrap();
    let mut expected = old_actor;
    expected.base.hit_points = 1;
    expected.base.path = None;
    expected.extension.spawn_group = 255;
    expected.extension.auxiliary = Default::default();
    expected.base.player_storage = objects.get(primary).unwrap().base.player_storage;
    assert_eq!(objects.get(primary), Some(&expected));
    assert_eq!(objects.get(other), Some(&other_before));
    assert!(runtime.resources.get_owned(other, other_data).is_some());
    assert_eq!(runtime.resources.owner_count(primary), 1);
    assert_eq!(
        runtime.resources.available_capacity(),
        PROGRAM_CAPACITY - 476 - 104
    );
    assert_eq!(
        get(&objects, &runtime.resources, primary).unwrap(),
        &PlayerStorage {
            fine_pitch: 29 * 256,
            fine_yaw: 157 * 256,
            bank: Angle::from_units(243),
            retained_shield: 173,
        }
    );
    let records = world.player(&objects, primary).unwrap();
    assert_eq!(records.contact.unwrap().hit.reserve_shield, 173);
    assert_eq!(records.visit.unwrap().pilot_code, 255);
    assert_eq!(records.injected_input, Some(crate::InputState::default()));
    assert_eq!(records.roll, Some(crate::player_roll::PlayerRoll::default()));
    assert_eq!(records.score, Some(inputs(255).score));
    assert!(
        records.protection.is_some() && records.auxiliary.is_some() && records.charge.is_some()
    );
    assert!(
        records.rapid_aim.is_some() && records.action.is_some() && records.consumable.is_some()
    );
    assert!(
        records.target_control.is_some()
            && records.particles.is_some()
            && records.carried.is_some()
    );
    assert_eq!(records.equipment, Some(Default::default()));
    assert_eq!(records.target_selection, Some(Default::default()));
    assert_eq!(records.target_lock, Some(Default::default()));
    assert_eq!(records.yaw_motion, Some(0));
    assert_eq!(records.pose, Some(Default::default()));
    assert_eq!(records.steering, Some(Default::default()));
    assert_eq!(records.view_distance, Some(Default::default()));
    assert_eq!(records.camera_angles, Some(Default::default()));
    assert_eq!(records.camera_tracking, Some(Default::default()));
    assert_eq!(records.camera_position, Some(Default::default()));
    assert_eq!(records.camera_ground, Some(Default::default()));
    assert_eq!(records.camera_surface, Some(Default::default()));
    assert_eq!(
        world.shots(&objects, primary),
        Some(ActiveShots::from_count(0))
    );
    assert_eq!(world.primary_player, Some(primary));
    assert_eq!(world.secondary_player, Some(other));
    assert_eq!(world.fixed_players, [Some(other), Some(primary)]);
    assert_eq!(world.active_shield_capacity, Some(11));
    assert_eq!(world.active_charge_threshold, Some(53));
    assert_eq!(world.random.bytes(), [1, 2, 3, 4]);
    assert!(
        world.spawn_defaults.is_none()
            && world.weapons.is_none()
            && world.published_motion.is_none()
    );
    assert!(world.contacts_enabled.is_none() && world.palette.is_none());
    assert_eq!(world.target_reticle, Default::default());
    assert!(world.published_homing_target.is_none());
}

#[test]
fn released_storage_cannot_be_read_through_any_scene_path_borrow_before_slot_reuse() {
    let mut objects = ObjectStore::new();
    let primary = owner(&mut objects);
    let borrower = owner(&mut objects);
    objects.get_mut(borrower).unwrap().base.attachment = Some(primary);
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut runtime = PathRuntime::default();
    replace(&mut objects, &mut world, &mut runtime, primary, inputs(3)).unwrap();
    world.fixed_players[0] = Some(borrower);
    assert!(world
        .path_world(&objects, borrower, PlayerTarget::Primary)
        .unwrap()
        .primary_target
        .is_some());
    let lifetime = objects.lifetime_id(primary);
    let prior_storage = objects.get(primary).unwrap().base.player_storage;
    runtime
        .release_actor_programs(&mut objects, primary)
        .unwrap();
    assert_eq!(objects.lifetime_id(primary), lifetime);
    assert_eq!(
        world.player(&objects, primary),
        Err(WorldInputError::StalePlayerRecord(primary))
    );
    assert_eq!(
        world.player_mut(&objects, primary),
        Err(WorldInputError::StalePlayerRecord(primary))
    );
    assert_eq!(
        get(&objects, &runtime.resources, primary),
        Err(PlayerStorageError::MissingStorage(primary))
    );
    assert_eq!(world.shots(&objects, primary), None);
    assert!(world.caller_weapon_inputs(&objects, primary).is_none());
    let path = world
        .path_world(&objects, borrower, PlayerTarget::Primary)
        .unwrap();
    assert!(path.selected_auxiliary.is_none() && path.selected_equipment.is_none());
    assert!(path.selected_charge.is_none() && path.selected_score.is_none());
    assert!(
        path.primary_motion.is_none()
            && path.primary_control.is_none()
            && path.primary_feedback.is_none()
    );
    assert!(path.linked_shot_count.is_none());
    assert!(path.primary_target.is_none());
    drop(path);
    replace(&mut objects, &mut world, &mut runtime, primary, inputs(1)).unwrap();
    assert_ne!(
        objects.get(primary).unwrap().base.player_storage,
        prior_storage
    );
    assert_eq!(
        world
            .player(&objects, primary)
            .unwrap()
            .visit
            .unwrap()
            .pilot_code,
        1
    );
    assert_eq!(
        runtime.resources.available_capacity(),
        PROGRAM_CAPACITY - 476
    );
}

#[test]
fn initialized_records_feed_the_real_player_prefix_and_replacement_resets_modified_fields() {
    let mut objects = ObjectStore::new();
    let primary = owner(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    world.spawn_defaults = Some(ObjectSpawnDefaults::default());
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 128,
    };
    for pilot in 0..=255 {
        scene
            .initialize_player_storage(primary, inputs(pilot))
            .unwrap();
        assert_eq!(scene.world.player_display_subject, Some(primary));
        let actor = scene.objects.get(primary).unwrap();
        assert!(!actor.extension.path_state.hold_latched);
        assert_eq!(
            actor.extension.path_state.animation.shape.fixed_frame(),
            Some(0)
        );
        assert!(actor.base.contacts.allow_same_shape);
        assert!(actor.base.flags.casts_shadow);
        assert!(actor.base.flags.exclude_from_shape_footprint_search);
        assert!(actor.base.flags.maximum_draw_distance);
        assert_eq!(
            scene
                .world
                .player(scene.objects, primary)
                .unwrap()
                .charge
                .unwrap()
                .progress,
            0
        );
        assert_eq!(
            scene
                .world
                .player(scene.objects, primary)
                .unwrap()
                .visit
                .unwrap()
                .shield_warning_clock,
            0
        );
        scene
            .begin_player_visit(primary, InputState::default())
            .unwrap();
        assert_eq!(
            scene
                .world
                .player(scene.objects, primary)
                .unwrap()
                .visit
                .unwrap()
                .shield_warning_clock,
            1
        );
        assert!(scene.world.published_motion.is_some());
        scene
            .world
            .player_mut(scene.objects, primary)
            .unwrap()
            .charge
            .as_mut()
            .unwrap()
            .progress = 65535;
        assert_eq!(
            scene.execution.paths.runtime.resources.owner_count(primary),
            1
        );
    }
    assert_eq!(
        scene
            .objects
            .get(primary)
            .unwrap()
            .extension
            .path_state
            .script_value,
        256
    );
    assert!(!scene.execution.is_faulted());
}

#[test]
fn exhausted_storage_releases_old_programs_but_latches_before_publication_or_retry() {
    let mut objects = ObjectStore::new();
    let primary = owner(&mut objects);
    let other = owner(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    world.primary_player = Some(other);
    world.player_display_subject = Some(other);
    world
        .bind_player(&objects, primary, PlayerPathRecords::default())
        .unwrap();
    objects
        .get_mut(primary)
        .unwrap()
        .extension
        .auxiliary
        .set(
            &mut execution.paths.runtime.resources,
            primary,
            AuxiliaryRecord::ReflectionShape(ShapeId::EMPTY),
        )
        .unwrap();
    let available = execution.paths.runtime.resources.available_capacity();
    execution
        .paths
        .runtime
        .resources
        .allocate_owned(
            other,
            available - 4,
            ProgramData::PathStack(Default::default()),
        )
        .unwrap();
    let before = objects.get(primary).unwrap().clone();
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 128,
    };
    assert_eq!(
        scene.initialize_player_storage(primary, inputs(2)),
        Err(SceneError::PlayerStorage(PlayerStorageError::Allocation(
            AllocationFailure::NoContiguousFit
        )))
    );
    assert!(scene.execution.is_faulted());
    assert_eq!(scene.world.primary_player, Some(other));
    assert_eq!(scene.world.player_display_subject, Some(other));
    assert!(scene.world.player(scene.objects, primary).is_err());
    assert_eq!(scene.objects.get(primary).unwrap().base, before.base);
    assert_eq!(
        scene.execution.paths.runtime.resources.owner_count(primary),
        0
    );
    assert_eq!(
        scene
            .objects
            .get(primary)
            .unwrap()
            .extension
            .auxiliary
            .find(
                &scene.execution.paths.runtime.resources,
                primary,
                AuxiliaryKind::ReflectionShape
            )
            .unwrap(),
        None
    );
    let resources = scene.execution.paths.runtime.resources.clone();
    assert_eq!(
        scene.initialize_player_storage(primary, inputs(2)),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.execution.paths.runtime.resources, resources);
}

#[test]
fn direct_formatter_changes_only_its_authored_fields_without_player_storage() {
    for seed in 0..=u8::MAX {
        let mut objects = ObjectStore::new();
        let primary = owner(&mut objects);
        let other = owner(&mut objects);
        let mut world = ScenePathWorld::new(RandomState::new([1, 2, 3, seed]));
        world.primary_player = Some(other);
        world.secondary_player = Some(primary);
        world.fixed_players = [Some(other), Some(primary)];
        world.player_display_subject = Some(other);
        let actor = objects.get_mut(primary).unwrap();
        actor.base.behavior = Behavior::PathMovement;
        actor.base.attack_power = seed;
        actor.base.hit_points = seed;
        actor.base.flags.visible = seed & 1 != 0;
        actor.base.flags.collision_disabled = seed & 2 != 0;
        actor.base.flags.casts_shadow = seed & 4 != 0;
        actor.base.flags.maximum_draw_distance = seed & 8 != 0;
        actor.base.flags.exclude_from_shape_footprint_search = seed & 16 != 0;
        actor.base.contacts.allow_same_shape = seed & 32 != 0;
        actor.base.contacts.run_when_paused = seed & 64 != 0;
        actor.extension.path_state.hold_latched = seed & 128 != 0;
        actor.extension.path_state.animation.shape =
            crate::path_appearance::AnimationControl::from_packed(seed);
        actor.extension.path_state.animation.color =
            crate::path_appearance::AnimationControl::from_packed(!seed);
        actor.extension.animation_frame = seed;
        actor.extension.color_frame = !seed;
        let mut expected = actor.clone();
        expected.base.attack_power = 0;
        expected.base.contacts.allow_same_shape = true;
        expected.base.flags.casts_shadow = true;
        expected.base.flags.maximum_draw_distance = true;
        expected.base.flags.exclude_from_shape_footprint_search = true;
        expected.extension.path_state.hold_latched = false;
        expected.extension.path_state.animation.shape.initialize(0);
        let other_before = objects.get(other).unwrap().clone();

        format_for_scene(&mut objects, &mut world, primary).unwrap();
        assert_eq!(objects.get(primary), Some(&expected));
        assert_eq!(objects.get(other), Some(&other_before));
        assert_eq!(world.player_display_subject, Some(primary));
        assert_eq!(world.primary_player, Some(other));
        assert_eq!(world.secondary_player, Some(primary));
        assert_eq!(world.fixed_players, [Some(other), Some(primary)]);
        assert_eq!(world.random.bytes(), [1, 2, 3, seed]);
        assert!(world.player(&objects, primary).is_err());
        assert!(world.processed_player_input.is_none());
        assert!(world.view_transition_mode.is_none());

        // The manual control is consumed only when animation is published;
        // formatting must not rewrite an already-built display snapshot.
        crate::path_appearance::publish_animation(objects.get_mut(primary).unwrap(), !seed);
        assert_eq!(objects.get(primary).unwrap().extension.animation_frame, 0);
        assert_eq!(
            objects.get(primary).unwrap().extension.color_frame,
            expected.extension.path_state.animation.color.resolve(!seed)
        );
    }
}

#[test]
fn formatter_missing_actor_does_not_reselect_the_display_subject() {
    let mut objects = ObjectStore::new();
    let absent = owner(&mut objects);
    let retained = owner(&mut objects);
    objects.remove(absent).unwrap();
    let mut world = ScenePathWorld::new(RandomState::default());
    world.player_display_subject = Some(retained);
    assert_eq!(
        format_for_scene(&mut objects, &mut world, absent),
        Err(PlayerStorageError::World(WorldInputError::MissingActor(
            absent
        )))
    );
    assert_eq!(world.player_display_subject, Some(retained));
}
