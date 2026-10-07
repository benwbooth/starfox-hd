use super::*;
use crate::collision_surface::SurfaceMode;
use crate::path_program::{PathCatalog, ProjectileTrigger};
use crate::path_scene_state::{EncounterHandoff, SceneTransitionControl};
use crate::player_action::{PlayerAction, PlayerServiceFlags, ScenePalette};
use crate::player_storage::{self, PlayerStorageInputs};
use crate::player_visit::PublishedConsumables;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategyHost, StrategySchedule};
use crate::view_transition::ViewTransitionMode;
use crate::{Buttons, InputState, Object, ObjectKind, ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("action-wait strategy must use the native scheduler dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("no death in fixture")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("no death in fixture")
    }
}

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

fn fixture(phase: ActionWaitPhase) -> (ObjectStore, ScenePathWorld, SceneExecution, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::PlayerActionWait(phase),
        ))
        .unwrap();
    objects.get_mut(owner).unwrap().base.hit_points = 1;
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut execution = SceneExecution::default();
    player_storage::replace(
        &mut objects,
        &mut world,
        &mut execution.paths.runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 1,
            reserve_shield: 40,
            score: Default::default(),
        },
    )
    .unwrap();
    world.controller_inputs = [Some(input(0xABCD, 0x1234)), Some(input(0x1234, 0xABCD))];
    world.processed_player_input = Some(input(0xA55A, 0x5AA5));
    world.unmasked_player_input = world.processed_player_input;
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 47,
        run_when_paused: false,
    });
    world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
    world.render_environment.ambient_control =
        Some(crate::player_surface_render::AmbientParticleControl::from_bits(0xDEAD));
    world.scene.active_weapon_level = Some(3);
    world.active_consumables = Some(PublishedConsumables {
        packed_count: 0x25,
        kind: 5,
    });
    world.surface_mode = Some(SurfaceMode { flags: 0xFF });
    world.handoff = Some(EncounterHandoff {
        player_flags: 0xFF,
        x: -19,
        z: 371,
        heading_word: 0xABCD,
    });
    world.scene.player_configuration = Some(0);
    world.palette = Some(ScenePalette {
        colors: [0x9237; 128],
        saved_colors: [31; 128],
    });
    world.projectile_trigger = Some(ProjectileTrigger::default());
    world.player_service_flags = Some(PlayerServiceFlags::default());
    world.player_view_options_enabled = Some(true);
    world.scene_transition = Some(SceneTransitionControl::default());
    world.fixed_players[0] = Some(owner);
    execution.controls.positional_suppressed = true;
    (objects, world, execution, owner)
}

#[test]
fn action_wait_samples_every_raw_controller_word_and_side_without_remap_or_injection() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Active);
    world.player_mut(&objects, owner).unwrap().injected_input = Some(input(0xA55A, 0x5AA5));
    let unmasked = world.unmasked_player_input;
    for word in 0..=u16::MAX {
        for (side, selected) in [(HitSide::Primary, 0), (HitSide::Secondary, 1)] {
            objects.get_mut(owner).unwrap().base.contacts.hit_side = side;
            world.controller_inputs[selected] = Some(input(word, !word));
            world.controller_inputs[1 - selected] = None;
            step(
                &mut objects,
                &mut world,
                &mut execution.paths.runtime.resources,
                &mut execution.positional,
                owner,
            )
            .unwrap();
            assert_eq!(world.processed_player_input, Some(input(word, !word)));
            assert_eq!(world.unmasked_player_input, unmasked);
            assert_eq!(
                world.player(&objects, owner).unwrap().injected_input,
                Some(input(0xA55A, 0x5AA5))
            );
            assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
        }
    }
}

#[test]
fn action_wait_initialization_keeps_action_and_companion_age_and_runs_time_zero_once() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Initialize);
    let action = world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap();
    action.install(PlayerAction::TriggeredProjectile);
    action.elapsed = 317;
    action.auxiliary_counter = 0xCDE7;
    action.total_updates = u16::MAX;
    let storage = objects.get(owner).unwrap().base.player_storage;
    let before_resources = execution.paths.runtime.resources.available_capacity();
    step(
        &mut objects,
        &mut world,
        &mut execution.paths.runtime.resources,
        &mut execution.positional,
        owner,
    )
    .unwrap();
    let actor = objects.get(owner).unwrap();
    assert_eq!(
        actor.base.behavior,
        Behavior::PlayerActionWait(ActionWaitPhase::Active)
    );
    assert_eq!(actor.base.shape, ShapeId::EMPTY);
    assert_eq!(actor.base.player_storage, storage);
    assert_eq!(
        execution.paths.runtime.resources.available_capacity(),
        before_resources
    );
    let r = world.player(&objects, owner).unwrap();
    let action = r.action.unwrap();
    assert_eq!(action.action, Some(PlayerAction::TriggeredProjectile));
    assert_eq!(action.elapsed, 1);
    assert_eq!(action.auxiliary_counter, 0xCDE7);
    assert_eq!(action.total_updates, 0);
    assert_eq!(r.equipment.unwrap().weapon_level, 3);
    assert_eq!(r.equipment.unwrap().packed_consumables, 0x25);
    assert_eq!(world.palette.as_ref().unwrap().saved_colors, [0x9237; 128]);
    assert_eq!(world.handoff.unwrap().player_flags, 0);
    assert_eq!(world.handoff.unwrap().heading_word, 0xABCD);
    assert_eq!(world.engine_sound_control.unwrap().bits(), 4);
    world.palette.as_mut().unwrap().colors.fill(1931);
    world.handoff.as_mut().unwrap().player_flags = 0xFF;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .pose
        .as_mut()
        .unwrap()
        .yaw_trim = 7;
    step(
        &mut objects,
        &mut world,
        &mut execution.paths.runtime.resources,
        &mut execution.positional,
        owner,
    )
    .unwrap();
    assert_eq!(world.palette.as_ref().unwrap().saved_colors, [0x9237; 128]);
    assert_eq!(world.handoff.unwrap().player_flags, 0xFF);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .pose
            .unwrap()
            .yaw_trim,
        7
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .action
            .unwrap()
            .elapsed,
        2
    );
}

#[test]
fn action_wait_paused_dispatch_still_initializes_and_samples_but_does_not_advance_action() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Initialize);
    world.view_transition_mode = Some(ViewTransitionMode { flags: 2 });
    world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap()
        .install(PlayerAction::TriggeredProjectile);
    world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap()
        .elapsed = 999;
    world.palette = None;
    step(
        &mut objects,
        &mut world,
        &mut execution.paths.runtime.resources,
        &mut execution.positional,
        owner,
    )
    .unwrap();
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .action
            .unwrap()
            .elapsed,
        0
    );
    assert_eq!(world.processed_player_input, world.controller_inputs[0]);
    assert_eq!(
        objects.get(owner).unwrap().base.behavior,
        Behavior::PlayerActionWait(ActionWaitPhase::Active)
    );
}

#[test]
fn action_wait_runs_from_both_shared_schedule_portions_without_flight_or_callback_fallback() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Initialize);
    world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap()
        .install(PlayerAction::TriggeredProjectile);
    let position = objects.get(owner).unwrap().base.position;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut schedule = StrategySchedule::default();
    for tick in 1..=10 {
        schedule.begin::<SceneError<()>>(&objects).unwrap();
        let mut scene = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 10,
        };
        schedule
            .run_overlapping(&mut scene, || tick & 1 == 0)
            .unwrap();
        schedule.run_remainder(&mut scene).unwrap();
        assert_eq!(scene.objects.get(owner).unwrap().base.position, position);
        assert_eq!(
            scene
                .world
                .player(scene.objects, owner)
                .unwrap()
                .action
                .unwrap()
                .elapsed,
            tick
        );
    }
}

#[test]
fn action_wait_missing_controller_after_initializer_preserves_source_prefix_and_latches_scene() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Initialize);
    world.controller_inputs[0] = None;
    let before_input = world.processed_player_input;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.run_strategy(owner, 1),
        Err(SceneError::PlayerActionWait(
            ActionWaitError::MissingController(HitSide::Primary)
        ))
    );
    assert_eq!(
        scene.objects.get(owner).unwrap().base.behavior,
        Behavior::PlayerActionWait(ActionWaitPhase::Active)
    );
    assert!(
        scene
            .objects
            .get(owner)
            .unwrap()
            .base
            .flags
            .collision_disabled
    );
    assert_eq!(scene.world.handoff.unwrap().player_flags, 0);
    assert_eq!(scene.world.processed_player_input, before_input);
    scene.world.controller_inputs[0] = Some(InputState::default());
    assert_eq!(scene.run_strategy(owner, 2), Err(SceneError::Faulted));
    assert_eq!(scene.world.processed_player_input, before_input);
}

#[test]
fn resetting_action_wait_prepares_every_visit_without_restarting_action_or_hiding_craft() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Resetting);
    world.primary_player = Some(owner);
    let action = world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap();
    action.install(PlayerAction::TriggeredProjectile);
    action.elapsed = 7;
    action.auxiliary_counter = 1937;
    action.total_updates = 65534;
    world.palette.as_mut().unwrap().colors.fill(3);
    world.palette.as_mut().unwrap().saved_colors.fill(0);
    world
        .player_mut(&objects, owner)
        .unwrap()
        .consumable
        .as_mut()
        .unwrap()
        .projectile_blockers = crate::player_consumable::TriggeredUseBlockers::from_control(0x10);
    let shape = objects.get(owner).unwrap().base.shape;
    for visit in 1..=3 {
        world
            .player_mut(&objects, owner)
            .unwrap()
            .pose
            .as_mut()
            .unwrap()
            .yaw_trim = 71;
        world.handoff.as_mut().unwrap().player_flags = 0xFF;
        step(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime.resources,
            &mut execution.positional,
            owner,
        )
        .unwrap();
        let records = world.player(&objects, owner).unwrap();
        let action = records.action.unwrap();
        assert_eq!(action.elapsed, 7 + visit);
        assert_eq!(action.auxiliary_counter, 1937);
        assert_eq!(action.total_updates, 65534u16.wrapping_add(visit));
        assert_eq!(records.pose.unwrap().yaw_trim, 0);
        assert_eq!(objects.get(owner).unwrap().base.shape, shape);
        assert_eq!(
            objects.get(owner).unwrap().base.behavior,
            Behavior::PlayerActionWait(ActionWaitPhase::Resetting)
        );
        assert_eq!(world.handoff.unwrap().player_flags, 0xAF);
        assert_eq!(world.palette.as_ref().unwrap().colors[64], 3 - visit);
        assert_eq!(world.palette.as_ref().unwrap().colors[63], 3);
    }
    // The palette dispatcher compares entering colors: the blocker survives
    // the visit which reaches its saved target.
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .consumable
            .unwrap()
            .projectile_blockers
            .bits(),
        0x10
    );
}

#[test]
fn resetting_action_wait_late_palette_failure_retains_action_visit_and_latches_scheduler() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Resetting);
    world.primary_player = None;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .action
        .as_mut()
        .unwrap()
        .install(PlayerAction::TriggeredProjectile);
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.run_strategy(owner, 1),
        Err(SceneError::PlayerActionWait(ActionWaitError::Palette(
            PaletteError::MissingPrimaryPlayer
        )))
    );
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .action
            .unwrap()
            .elapsed,
        1
    );
    assert_eq!(
        scene.world.palette.as_ref().unwrap().saved_colors,
        [0x9237; 128]
    );
    scene.world.primary_player = Some(owner);
    assert_eq!(scene.run_strategy(owner, 2), Err(SceneError::Faulted));
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .action
            .unwrap()
            .elapsed,
        1
    );
}

#[test]
fn resetting_action_wait_engine_write_precedes_missing_handoff_and_collision_suppression() {
    let (mut objects, mut world, mut execution, owner) = fixture(ActionWaitPhase::Resetting);
    world.handoff = None;
    assert_eq!(
        step(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime.resources,
            &mut execution.positional,
            owner
        ),
        Err(ActionWaitError::MissingHandoff)
    );
    assert_eq!(world.engine_sound_control.unwrap().bits(), 4);
    assert!(!objects.get(owner).unwrap().base.flags.collision_disabled);
}
