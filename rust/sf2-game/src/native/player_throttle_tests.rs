use super::*;
use crate::path_program::PathCatalog;
use crate::player_charge::PlayerCharge;
use crate::player_visit::PlayerVisitControl;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::{Angle, Buttons, InputState, ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected external throttle strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected throttle death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected throttle map continuation")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
}
impl Scene {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut player = Object::new(
            ObjectKind::Player,
            ShapeId::TITLE_CRAFT,
            Behavior::Unassigned,
        );
        player.base.hit_points = 30;
        player.base.position = Vector3 {
            x: -123,
            y: 24,
            z: 413,
        };
        player.base.pitch = Angle::from_units(37);
        player.base.yaw = Angle::from_units(84);
        player.base.roll = Angle::from_units(192);
        let owner = objects.allocate(player).unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(owner);
        world.contacts_enabled = Some(true);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 42,
            run_when_paused: false,
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    throttle: Some(PlayerThrottle::default()),
                    charge: Some(PlayerCharge::default()),
                    visit: Some(PlayerVisitControl::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        action_flags: 1,
                        mode: 0x10,
                        stored_rotation: Default::default(),
                        stored_world_position: Default::default(),
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
            catalog: authored_paths::catalog(),
            callbacks: Callbacks,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn state(&mut self) -> &mut PlayerThrottle {
        self.records().throttle.as_mut().unwrap()
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
    fn visit(&mut self, held: u16, pressed: u16) -> Result<(), SceneError<&'static str>> {
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
        let owner = self.owner;
        self.host().advance_player_throttle(owner)
    }
    fn child(&self, number: u8) -> Option<ObjectId> {
        path_relationships::find_direct_child(&self.objects, self.owner, number).unwrap()
    }
    fn fill(&mut self, count: usize) {
        while self.objects.len() < count {
            self.objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
    }
    fn epoch(&mut self, schedule: &mut StrategySchedule) {
        self.execution.positional.begin_epoch();
        self.host().begin_strategy_epoch(schedule).unwrap();
        schedule
            .run_overlapping(&mut self.host(), || false)
            .unwrap();
        schedule.run_remainder(&mut self.host()).unwrap();
        self.host().clean_epoch().unwrap();
    }
}

#[test]
fn button_preference_edges_and_all_incoming_values_follow_source_arbitration() {
    let both = Button::A as u16 | Button::Y as u16;
    for preference in 0..=255 {
        for pressed in [0, Button::A as u16, Button::Y as u16, both] {
            for held in [0, Button::A as u16, Button::Y as u16, both] {
                let mut scene = Scene::new();
                scene.records().charge.as_mut().unwrap().linked_mode = true;
                scene.state().brake_preference = preference;
                scene.visit(held, pressed).unwrap();
                let preferred = if pressed & Button::A as u16 != 0 {
                    1
                } else if pressed & Button::Y as u16 != 0 {
                    0
                } else {
                    preference
                };
                let action = if preferred == 0 && held & Button::Y as u16 != 0 {
                    BOOST
                } else if held & Button::A as u16 != 0 {
                    BRAKE
                } else {
                    0
                };
                assert_eq!(scene.records().auxiliary.unwrap().action_flags, action | 1);
                assert_eq!(
                    scene.state().brake_preference,
                    if action == 0 { 0 } else { preferred }
                );
            }
        }
    }
}

#[test]
fn gated_input_preserves_preference_and_decay_retains_other_bits() {
    for alternate in [false, true] {
        for flags in 0..=255 {
            for level in [0, 1, 15, 16, 31, 63, 127, 255] {
                for clock in 0..2 {
                    let mut scene = Scene::new();
                    scene.world.contacts_enabled = Some(false);
                    scene.world.processed_player_input = None;
                    scene.world.strategy_clock = clock;
                    *scene.state() = PlayerThrottle {
                        brake_preference: 243,
                        effect_flags: flags,
                        effect_level: level,
                        entry_marker: 17,
                    };
                    scene.records().auxiliary.as_mut().unwrap().action_flags = 255;
                    if alternate {
                        deactivate_effect(&scene.objects, &mut scene.world, scene.owner).unwrap();
                    } else {
                        advance(&mut scene.objects, &mut scene.world, scene.owner).unwrap();
                    }
                    let expected_flags = flags & if alternate { 0x7F } else { 0xBF };
                    let mut expected_level = level & 0x3F;
                    if expected_flags & 0x3F == 0 && expected_level & 0x0F != 0 && clock == 0 {
                        expected_level -= 1;
                    }
                    assert_eq!(
                        *scene.state(),
                        PlayerThrottle {
                            brake_preference: 243,
                            effect_flags: expected_flags,
                            effect_level: expected_level,
                            entry_marker: 17
                        }
                    );
                    assert_eq!(scene.records().auxiliary.unwrap().action_flags, 0x9F);
                }
            }
        }
    }
}

#[test]
fn installers_keep_global_head_order_pilot_pairs_and_exact_fresh_fields() {
    for pilot in 0..=255 {
        let mut scene = Scene::new();
        scene.records().visit.as_mut().unwrap().pilot_code = pilot;
        scene.fill(2);
        let head = scene.objects.active_ids()[0];
        scene.visit(Button::A as u16, Button::A as u16).unwrap();
        let left = scene.child(13).unwrap();
        let right = scene.child(14).unwrap();
        assert_eq!(
            scene.objects.active_ids(),
            &[head, right, left, scene.owner]
        );
        let pair = usize::from(if pilot < 6 { pilot / 2 } else { 0 });
        for (effect, direction) in [(left, -1), (right, 1)] {
            let record = scene.objects.get(effect).unwrap();
            let player = scene.objects.get(scene.owner).unwrap();
            assert_eq!(record.base.shape, EFFECT_SHAPE);
            assert_eq!(
                record.base.path,
                Some(authored_paths::AUXILIARY_GATED_SPRITE)
            );
            assert_eq!(
                record.extension.relative_position,
                Vector3 {
                    x: direction * BRAKE_X[pair],
                    y: BRAKE_Y[pair],
                    z: 15
                }
            );
            assert_eq!(
                (
                    record.base.position,
                    record.base.pitch,
                    record.base.yaw,
                    record.base.roll
                ),
                (
                    player.base.position,
                    player.base.pitch,
                    player.base.yaw,
                    player.base.roll
                )
            );
            assert_eq!(
                (
                    record.base.hit_points,
                    record.base.attack_power,
                    record.extension.spawn_group
                ),
                (1, 1, 255)
            );
            assert!(record.extension.path_state.needs_path_initialization);
            assert!(
                record.base.flags.remove_with_parent
                    && record.base.flags.collision_disabled
                    && record.base.flags.general_search_eligible
            );
            assert!(record.base.contacts.run_when_paused);
        }
        assert_eq!(
            scene.objects.get(left).unwrap().base.attachment_next,
            Some(right)
        );
    }
}

#[test]
fn pool_pressure_and_linked_mode_preserve_order_without_refreshing_existing_effects() {
    for capacity in [58, 59, 60] {
        let mut scene = Scene::new();
        scene.fill(capacity);
        if capacity == 60 {
            scene.world.spawn_defaults = None;
        }
        let result = scene.visit(Button::A as u16, 0);
        assert_eq!(
            result,
            if capacity == 58 {
                Ok(())
            } else {
                Err(SceneError::PlayerThrottle(
                    ThrottleError::ObjectPoolExhausted,
                ))
            }
        );
        assert_eq!(scene.child(13).is_some(), capacity < 60);
        assert_eq!(scene.child(14).is_some(), capacity < 59);
        assert_eq!(
            scene.records().auxiliary.unwrap().action_flags,
            if capacity == 58 { 0x21 } else { 1 }
        );
        if capacity > 58 {
            assert_eq!(scene.visit(0, 0), Err(SceneError::Faulted));
        }
    }
    for linked in [false, true] {
        for override_link in [false, true] {
            let mut scene = Scene::new();
            scene.records().charge.as_mut().unwrap().linked_mode = linked;
            scene
                .records()
                .charge
                .as_mut()
                .unwrap()
                .linked_muzzle_disabled = override_link;
            scene.visit(Button::Y as u16, 0).unwrap();
            assert_eq!(scene.child(12).is_some(), !linked || override_link);
        }
    }
    let mut scene = Scene::new();
    scene.visit(Button::A as u16, 0).unwrap();
    let left = scene.child(13).unwrap();
    scene
        .objects
        .get_mut(left)
        .unwrap()
        .base
        .flags
        .remove_after_tick = true;
    let before = scene.objects.get(left).unwrap().clone();
    scene.records().charge = None;
    scene.records().visit = None;
    scene.world.spawn_defaults = None;
    assert_eq!(
        ensure_brake_effect(
            &mut scene.objects,
            &scene.world,
            scene.owner,
            BrakeSide::Left
        ),
        Ok(None)
    );
    assert_eq!(scene.objects.get(left), Some(&before));
}

#[test]
fn retained_brake_skips_action_store_marker_reset_and_duplicate_sound() {
    for flags in 0..=255 {
        let mut scene = Scene::new();
        scene.world.primary_player = None;
        scene.records().charge.as_mut().unwrap().linked_mode = true;
        scene.records().auxiliary.as_mut().unwrap().action_flags = flags;
        scene.state().entry_marker = 37;
        brake(&mut scene.objects, &mut scene.world, scene.owner).unwrap();
        assert_eq!(
            scene.records().auxiliary.unwrap().action_flags,
            if flags & BRAKE != 0 {
                flags
            } else {
                (flags & !BOOST) | BRAKE
            }
        );
        assert_eq!(
            scene.state().entry_marker,
            if flags & BRAKE != 0 { 37 } else { 0 }
        );
        let events: Vec<_> = scene
            .world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .collect();
        assert_eq!(
            events,
            if flags & BRAKE != 0 {
                vec![]
            } else {
                vec![SoundEvent::Authored(AuthoredCue::new(
                    27,
                    0,
                    PlayerTarget::Secondary,
                ))]
            }
        );
    }
}

#[test]
fn real_effect_paths_follow_parent_action_then_retire_and_release_resources() {
    for held in [Button::Y as u16, Button::A as u16] {
        let mut scene = Scene::new();
        let mut schedule = StrategySchedule::default();
        scene.visit(held, held).unwrap();
        let effects: Vec<_> = [12, 13, 14]
            .into_iter()
            .filter_map(|number| scene.child(number))
            .collect();
        for _ in 0..8 {
            scene.epoch(&mut schedule);
        }
        for effect in &effects {
            assert!(scene.objects.get(*effect).is_some());
            assert!(scene.execution.paths.runtime.resources.owner_count(*effect) > 0);
        }
        scene.visit(0, 0).unwrap();
        for _ in 0..4 {
            scene.epoch(&mut schedule);
        }
        for effect in effects {
            assert!(scene.objects.get(effect).is_none());
            assert_eq!(
                scene.execution.paths.runtime.resources.owner_count(effect),
                0
            );
        }
        assert_eq!(scene.objects.len(), 1);
        assert!(!scene.execution.is_faulted());
    }
}

#[test]
fn missing_input_and_partial_installer_failures_latch_without_reexecution() {
    let mut scene = Scene::new();
    scene.state().entry_marker = 99;
    scene.records().charge = None;
    assert!(matches!(
        scene.visit(Button::Y as u16, 0),
        Err(SceneError::PlayerThrottle(ThrottleError::World(
            WorldInputError::MissingPlayerCharge(_)
        )))
    ));
    assert_eq!(scene.state().entry_marker, 0);
    assert_eq!(scene.objects.len(), 1);
    assert_eq!(scene.visit(0, 0), Err(SceneError::Faulted));
    let mut scene = Scene::new();
    scene.world.spawn_defaults = None;
    assert_eq!(
        scene.visit(Button::A as u16, Button::A as u16),
        Err(SceneError::PlayerThrottle(
            ThrottleError::MissingSpawnDefaults
        ))
    );
    assert_eq!(scene.state().brake_preference, 1);
    assert_eq!(scene.records().auxiliary.unwrap().action_flags, 1);
    assert_eq!(scene.objects.len(), 1);
}

#[test]
fn live_objective_low_byte_overrides_initial_activity_without_reading_disabled_input() {
    let mut scene = Scene::new();
    scene.world.objective_counts = Some(crate::path_scene_state::EncounterObjectiveCounts {
        remaining_word: 256,
        node_record: 0,
        recorded_completions: 0,
        signaled_completions: 0,
    });
    scene.world.contacts_enabled = Some(true);
    scene.state().brake_preference = 123;
    let owner = scene.owner;
    scene.host().advance_player_throttle(owner).unwrap();
    assert_eq!(scene.state().brake_preference, 123);
    scene
        .world
        .objective_counts
        .as_mut()
        .unwrap()
        .remaining_word = 257;
    assert_eq!(
        scene.host().advance_player_throttle(owner),
        Err(SceneError::PlayerThrottle(
            ThrottleError::MissingProcessedInput
        ))
    );
    assert_eq!(scene.state().brake_preference, 123);
}
