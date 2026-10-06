use super::*;
use crate::path_control::PlayerTarget;
use crate::path_fields::ByteField;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{PathCatalog, ProjectileTrigger, ProjectileTriggerCommand, Statement};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{
    Behavior, Buttons, Object, ObjectKind, ObjectSpawnDefaults, PathCursor, RandomState, ShapeId,
};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected assigned callback")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death callback")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map callback")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
}

impl Scene {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::TITLE_CRAFT,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([17, 23, 49, 81]));
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 0,
            run_when_paused: false,
        });
        world.scene.player_configuration = Some(0);
        world.projectile_trigger = Some(ProjectileTrigger::default());
        world.player_service_flags = Some(PlayerServiceFlags::from_bits(0xA5));
        world.palette = Some(ScenePalette {
            colors: std::array::from_fn(|i| (i as u16 * 251) | 0x8000),
            saved_colors: [0; SCENE_PALETTE_COLORS],
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    action: Some(PlayerActionState {
                        action: Some(PlayerAction::TriggeredProjectile),
                        elapsed: 0,
                        auxiliary_counter: 91,
                        total_updates: 77,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            owner,
            execution: SceneExecution::default(),
        }
    }
    fn action(&mut self) -> &mut PlayerActionState {
        self.world
            .player_mut(&self.objects, self.owner)
            .unwrap()
            .action
            .as_mut()
            .unwrap()
    }
    fn visit(&mut self, pressed: bool) -> Result<(), SceneError<&'static str>> {
        let mut callbacks = Callbacks;
        let catalog = PathCatalog::new(vec![]).unwrap();
        let mut host = SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 50,
        };
        host.advance_player_action(
            self.owner,
            InputState {
                // A held button without a new edge must not request detonation.
                held: Buttons::from_bits(Button::X as u16),
                pressed: Buttons::from_bits(if pressed { Button::X as u16 } else { 0 }),
            },
        )
    }
}

#[test]
fn full_stream_lifetime_saves_triggers_requests_then_stops_without_resetting_total_age() {
    let mut scene = Scene::new();
    let colors = scene.world.palette.as_ref().unwrap().colors;
    for time in 0..=40 {
        assert_eq!(scene.action().elapsed, time);
        scene.visit(false).unwrap();
        assert_eq!(scene.world.palette.as_ref().unwrap().colors, colors);
        assert_eq!(scene.world.palette.as_ref().unwrap().saved_colors, colors);
        assert_eq!(
            scene.world.projectile_trigger.unwrap().activation,
            u8::from(time >= 12)
        );
        assert_eq!(
            scene.world.player_service_flags.unwrap().bits(),
            if time >= 14 { 0xB5 } else { 0xA5 }
        );
        assert_eq!(scene.action().total_updates, 78 + time);
    }
    assert_eq!(
        *scene.action(),
        PlayerActionState {
            action: None,
            elapsed: 1,
            auxiliary_counter: 0,
            total_updates: 118,
        }
    );
    let ended = *scene.action();
    scene.world.palette = None;
    scene.world.scene.player_configuration = None;
    scene.world.projectile_trigger = None;
    scene.world.player_service_flags = None;
    scene.visit(true).unwrap();
    assert_eq!(*scene.action(), ended);
}

#[test]
fn every_clock_word_and_press_preserves_authored_interval_and_overflow_rules() {
    let mut scene = Scene::new();
    scene.world.scene.player_configuration = Some(9);
    scene.world.palette = None;
    for time in 0..=u16::MAX {
        for pressed in [false, true] {
            *scene.action() = PlayerActionState {
                action: Some(PlayerAction::TriggeredProjectile),
                elapsed: time,
                auxiliary_counter: 95,
                total_updates: u16::MAX,
            };
            scene.world.projectile_trigger.as_mut().unwrap().activation = 0;
            scene.world.player_service_flags = Some(PlayerServiceFlags::from_bits(0));
            scene.visit(pressed).unwrap();
            let jumped = pressed && time >= 2 && time < 11;
            let expected_time = if time == 40 {
                1
            } else if jumped {
                13
            } else if time == u16::MAX {
                u16::MAX
            } else {
                time + 1
            };
            assert_eq!(
                scene.action().elapsed,
                expected_time,
                "time {time}, pressed {pressed}"
            );
            assert_eq!(
                scene.action().total_updates,
                if time == u16::MAX { u16::MAX } else { 0 }
            );
            assert_eq!(scene.action().action.is_none(), time == 40);
            assert_eq!(
                scene.action().auxiliary_counter,
                if time == 40 { 0 } else { 95 }
            );
            assert_eq!(
                scene.world.projectile_trigger.unwrap().activation,
                u8::from(jumped || time == 12)
            );
            assert_eq!(
                scene.world.player_service_flags.unwrap().bits(),
                if time == 14 { 0x10 } else { 0 }
            );
        }
    }
}

#[test]
fn every_trigger_byte_causes_same_visit_detonation_only_inside_the_early_window() {
    let mut scene = Scene::new();
    for trigger in 0..=u8::MAX {
        for time in 1..=13 {
            scene.action().elapsed = time;
            scene.world.projectile_trigger.as_mut().unwrap().activation = trigger;
            scene.visit(false).unwrap();
            let jumped = trigger != 0 && (2..11).contains(&time);
            assert_eq!(scene.action().elapsed, if jumped { 13 } else { time + 1 });
            assert_eq!(
                scene.world.projectile_trigger.unwrap().activation,
                if jumped || time == 12 { 1 } else { trigger }
            );
        }
    }
}

#[test]
fn complete_palette_words_are_saved_and_only_special_configuration_skips_the_service() {
    let mut scene = Scene::new();
    for packed in 0..=u16::MAX {
        scene.action().elapsed = 0;
        scene.world.palette.as_mut().unwrap().colors.fill(packed);
        scene.visit(false).unwrap();
        let palette = scene.world.palette.as_ref().unwrap();
        assert!(palette.colors.iter().all(|c| *c == packed));
        assert_eq!(palette.saved_colors, palette.colors);
    }
    for configuration in 0..=u8::MAX {
        scene.action().elapsed = 0;
        scene.world.scene.player_configuration = Some(configuration);
        scene.world.palette.as_mut().unwrap().saved_colors.fill(123);
        scene.visit(false).unwrap();
        assert!(scene
            .world
            .palette
            .as_ref()
            .unwrap()
            .saved_colors
            .iter()
            .all(|c| *c == if configuration == 9 { 123 } else { u16::MAX }));
    }
    scene.world.palette = None;
    scene.world.scene.player_configuration = Some(9);
    scene.action().elapsed = 0;
    scene.visit(false).unwrap();
}

#[test]
fn service_flags_preserve_all_other_bits_and_installation_retains_an_existing_stream() {
    let mut scene = Scene::new();
    for flags in 0..=u8::MAX {
        scene.action().elapsed = 14;
        scene.world.player_service_flags = Some(PlayerServiceFlags::from_bits(flags));
        scene.visit(false).unwrap();
        let updated = scene.world.player_service_flags.unwrap();
        assert_eq!(updated.bits(), flags | 0x10);
        assert_eq!(updated.minimum_protection(), flags & 1 != 0);
        assert!(updated.palette_restoration_requested());
    }
    let before = *scene.action();
    scene.action().install_triggered_projectile();
    assert_eq!(*scene.action(), before);
    scene.action().action = None;
    scene.action().install_triggered_projectile();
    assert_eq!(scene.action().elapsed, 0);
    assert_eq!(scene.action().auxiliary_counter, 0);
    assert_eq!(scene.action().total_updates, before.total_updates);
}

#[test]
fn pause_and_inactive_early_exits_do_not_resolve_unreached_inputs() {
    let mut scene = Scene::new();
    scene.world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .action = None;
    scene.objects.remove(scene.owner).unwrap();
    scene.visit(true).unwrap();
    scene.world.spawn_defaults = None;
    assert_eq!(
        scene.visit(true),
        Err(SceneError::PlayerAction(
            PlayerActionError::MissingSpawnDefaults
        ))
    );
    assert_eq!(scene.visit(true), Err(SceneError::Faulted));

    let mut scene = Scene::new();
    scene.action().action = None;
    let before = *scene.action();
    scene.world.palette = None;
    scene.world.projectile_trigger = None;
    scene.world.player_service_flags = None;
    scene.world.scene.player_configuration = None;
    scene.visit(true).unwrap();
    assert_eq!(*scene.action(), before);
}

#[test]
fn absent_services_fault_only_at_their_authored_times_without_incrementing_or_replaying() {
    for (time, expected) in [
        (0, PlayerActionError::MissingConfiguration),
        (2, PlayerActionError::MissingProjectileTrigger),
        (12, PlayerActionError::MissingProjectileTrigger),
        (14, PlayerActionError::MissingServiceFlags),
    ] {
        let mut scene = Scene::new();
        scene.action().elapsed = time;
        scene.world.scene.player_configuration = None;
        scene.world.projectile_trigger = None;
        scene.world.player_service_flags = None;
        assert_eq!(scene.visit(true), Err(SceneError::PlayerAction(expected)));
        assert_eq!(scene.action().elapsed, time);
        assert_eq!(scene.action().total_updates, 77);
        scene.world.scene.player_configuration = Some(0);
        scene.world.projectile_trigger = Some(ProjectileTrigger::default());
        scene.world.player_service_flags = Some(PlayerServiceFlags::default());
        assert_eq!(scene.visit(true), Err(SceneError::Faulted));
    }
    let mut scene = Scene::new();
    scene.world.palette = None;
    assert_eq!(
        scene.visit(false),
        Err(SceneError::PlayerAction(PlayerActionError::MissingPalette))
    );
}

#[test]
fn action_and_projectile_paths_share_the_same_mutable_trigger_owner() {
    let mut scene = Scene::new();
    let shot = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Projectile,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    let cursor = PathCursor {
        path: crate::PathId::from_catalog_index(0),
        command_index: 0,
    };
    scene.objects.get_mut(shot).unwrap().base.path = Some(cursor);
    let catalog = PathCatalog::new(vec![vec![Statement::ProjectileTrigger {
        command: ProjectileTriggerCommand::CopyTo(ByteField::AttackPower),
        next: cursor,
    }]])
    .unwrap();
    scene.action().elapsed = 2;
    scene.visit(true).unwrap();
    let mut inputs = scene
        .world
        .path_world(&scene.objects, shot, PlayerTarget::Primary)
        .unwrap();
    let exit = scene
        .execution
        .paths
        .runtime
        .step_program(&catalog, &mut scene.objects, shot, &mut inputs)
        .unwrap();
    assert_eq!(exit.actor, shot);
    assert_eq!(scene.objects.get(shot).unwrap().base.attack_power, 1);
    assert_eq!(scene.action().elapsed, 13);
}
