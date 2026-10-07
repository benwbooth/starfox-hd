use super::super::path_equipment::SelectedEquipment;
use super::*;
use crate::path_invocation::{InvocationEntry, InvocationWorld, PathInvocation};
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::player_action::{PlayerAction, PlayerActionState, ScenePalette};
use crate::scene_contact::PlayerContactControl;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategyHost};
use crate::{Behavior, Object, ObjectKind, ObjectSpawnDefaults, RandomState, ShapeId};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("no unported player mode may be dispatched by the prefix")
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

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
    catalog: PathCatalog,
    callbacks: Callbacks,
}
impl Scene {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([17, 23, 49, 81]));
        world.spawn_defaults = Some(ObjectSpawnDefaults::default());
        world.primary_player = Some(owner);
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    visit: Some(PlayerVisitControl {
                        shield_warning_clock: 10,
                        ..Default::default()
                    }),
                    contact: Some(PlayerContactControl::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 32,
                        action_flags: 0,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    action: Some(PlayerActionState::default()),
                    equipment: Some(SelectedEquipment {
                        packed_consumables: 0xF3,
                        consumable_type: 129,
                        weapon_level: 7,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            execution: SceneExecution::default(),
            owner,
            catalog: PathCatalog::new(vec![]).unwrap(),
            callbacks: Callbacks,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
    }
    fn visit(&mut self) -> Result<(), SceneError<&'static str>> {
        let owner = self.owner;
        self.host().begin_player_visit(owner, InputState::default())
    }
    fn cues(&mut self) -> Vec<SoundEvent> {
        self.world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .collect()
    }
}

#[test]
fn proximity_warning_reads_the_live_action_byte_for_every_flag_and_raw_pilot() {
    let mut scene = Scene::new();
    scene.records().auxiliary.as_mut().unwrap().mode = 0x11;
    scene.world.fixed_players[0] = Some(scene.owner);
    let mut obstacle = Object::new(
        ObjectKind::Enemy,
        ShapeId::TITLE_CRAFT,
        Behavior::Unassigned,
    );
    obstacle.base.flags.proximity_warning_source = true;
    obstacle.base.position.z = 70;
    let obstacle = scene.objects.allocate(obstacle).unwrap();
    for pilot in 0..=u8::MAX {
        for flags in 0..=u8::MAX {
            scene.records().visit.as_mut().unwrap().pilot_code = pilot;
            scene.records().auxiliary.as_mut().unwrap().action_flags = flags;
            scene
                .objects
                .get_mut(obstacle)
                .unwrap()
                .base
                .flags
                .proximity_warning_latched = false;
            scene.visit().unwrap();
            let admitted = flags & 0x20 == 0 && (pilot & 0xFE != 2 || flags & 0x40 != 0);
            assert_eq!(
                scene
                    .objects
                    .get(obstacle)
                    .unwrap()
                    .base
                    .flags
                    .proximity_warning_latched,
                admitted
            );
            assert_eq!(scene.records().auxiliary.unwrap().action_flags, flags);
            assert_eq!(scene.cues().len(), usize::from(admitted));
        }
    }
}

#[test]
fn live_brake_and_boost_bits_change_guarded_warning_admission_without_a_second_owner() {
    let mut scene = Scene::new();
    scene.records().visit.as_mut().unwrap().pilot_code = 2;
    scene.records().auxiliary.as_mut().unwrap().mode = 0x11;
    scene.world.fixed_players[0] = Some(scene.owner);
    let mut obstacle = Object::new(
        ObjectKind::Enemy,
        ShapeId::TITLE_CRAFT,
        Behavior::Unassigned,
    );
    obstacle.base.flags.proximity_warning_source = true;
    obstacle.base.position.z = 70;
    let obstacle = scene.objects.allocate(obstacle).unwrap();
    for (flags, admitted) in [
        (0, false),
        (0x40, true),
        (0x60, false),
        (0x40, true),
        (0, false),
    ] {
        scene.records().auxiliary.as_mut().unwrap().action_flags = flags;
        scene
            .objects
            .get_mut(obstacle)
            .unwrap()
            .base
            .flags
            .proximity_warning_latched = false;
        scene.visit().unwrap();
        assert_eq!(
            scene
                .objects
                .get(obstacle)
                .unwrap()
                .base
                .flags
                .proximity_warning_latched,
            admitted
        );
        assert_eq!(scene.cues().len(), usize::from(admitted));
    }
}

#[test]
fn all_pilot_and_shield_bytes_preserve_the_signed_clamp_and_publish_complete_equipment() {
    let mut scene = Scene::new();
    for pilot in 0..=u8::MAX {
        let (capacity, threshold) = match pilot {
            2 | 3 => (40u8, 35),
            4 | 5 => (24, 10),
            _ => (32, 25),
        };
        for shield in 0..=u8::MAX {
            scene.records().visit.as_mut().unwrap().pilot_code = pilot;
            scene.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
            scene.visit().unwrap();
            let diff = (i16::from(capacity) - i16::from(shield)).rem_euclid(256);
            let expected = if diff >= 128 { capacity } else { shield };
            assert_eq!(
                scene.records().contact.unwrap().hit.reserve_shield,
                expected
            );
            assert_eq!(scene.world.scene.active_shield, Some(expected));
            assert_eq!(scene.world.active_charge_threshold, Some(threshold));
            assert_eq!(scene.world.active_shield_capacity, Some(capacity));
            assert_eq!(
                scene.world.active_consumables,
                Some(PublishedConsumables {
                    packed_count: 0xF3,
                    kind: 129
                })
            );
            assert_eq!(scene.world.scene.active_weapon_level, Some(7));
        }
    }
    assert!(scene.cues().is_empty());
}

#[test]
fn warning_clock_wraps_to_ten_once_and_uses_signed_thresholds_and_caller_side() {
    let mut scene = Scene::new();
    for primary in [true, false] {
        scene.world.primary_player = if primary { Some(scene.owner) } else { None };
        for time in 0..=u8::MAX {
            for shield in 0..=u8::MAX {
                scene.records().visit.as_mut().unwrap().shield_warning_clock = time;
                scene.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
                scene.visit().unwrap();
                let updated = if time == 10 { 10 } else { time.wrapping_add(1) };
                assert_eq!(scene.records().visit.unwrap().shield_warning_clock, updated);
                let shield = if (32i16 - i16::from(shield)).rem_euclid(256) >= 128 {
                    32
                } else {
                    shield
                };
                let expected = if time == 9 && (i16::from(shield) - 13).rem_euclid(256) >= 128 {
                    let cue = if (i16::from(shield) - 5).rem_euclid(256) >= 128 {
                        23
                    } else {
                        22
                    };
                    vec![SoundEvent::Authored(AuthoredCue::new(
                        cue,
                        0,
                        if primary {
                            PlayerTarget::Primary
                        } else {
                            PlayerTarget::Secondary
                        },
                    ))]
                } else {
                    vec![]
                };
                assert_eq!(scene.cues(), expected, "time={time} shield={shield}");
            }
        }
    }
}

#[test]
fn every_mode_and_age_word_use_actual_actor_motion_and_saturating_shared_word() {
    let mut scene = Scene::new();
    for age in 0..=u16::MAX {
        let mode = age as u8;
        scene.records().auxiliary.as_mut().unwrap().mode = mode;
        // No obstacle candidates means no fixed-view read, even in flight.
        let position = Vector3 {
            x: age as i16,
            y: !(age as i16),
            z: age.wrapping_mul(37) as i16,
        };
        let velocity = Vector3 {
            x: 201,
            y: -203,
            z: 207,
        };
        let displacement = Vector3 {
            x: -101,
            y: 103,
            z: -107,
        };
        let actor = scene.objects.get_mut(scene.owner).unwrap();
        actor.base.position = position;
        actor.base.velocity = velocity;
        actor.base.flags.view_side_filter = true;
        actor.base.flags.visible = false;
        actor.extension.path_state.motion_delta = displacement;
        actor.extension.path_state.script_value = age;
        scene.visit().unwrap();
        let actor = scene.objects.get(scene.owner).unwrap();
        assert!(!actor.base.flags.view_side_filter);
        assert!(!actor.base.flags.visible);
        assert_eq!(
            actor.extension.path_state.script_value,
            if age == 65535 { age } else { age + 1 }
        );
        assert_eq!(
            scene.world.published_motion,
            Some(PublishedPlayerMotion {
                position,
                delta: if (16..32).contains(&mode) {
                    velocity
                } else {
                    displacement
                }
            })
        );
    }
}

#[test]
fn source_pause_resets_only_callers_shots_but_does_not_skip_publication_or_age() {
    let mut scene = Scene::new();
    let primary = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    scene.world.primary_player = Some(primary);
    scene
        .world
        .bind_shots(&scene.objects, primary, ActiveShots::from_count(91))
        .unwrap();
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(219))
        .unwrap();
    scene.world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    scene.records().action = None;
    scene.records().auxiliary.as_mut().unwrap().mode = 16;
    scene.objects.get_mut(scene.owner).unwrap().base.velocity.x = -61;
    scene.visit().unwrap();
    assert_eq!(
        scene
            .world
            .shots(&scene.objects, scene.owner)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        scene.world.shots(&scene.objects, primary).unwrap().count(),
        91
    );
    assert_eq!(scene.world.published_motion.unwrap().delta.x, -61);
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1
    );
    scene.world.spawn_defaults.as_mut().unwrap().run_when_paused = false;
    scene.records().action = Some(Default::default());
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(83))
        .unwrap();
    scene.execution.controls.paused = true;
    scene.visit().unwrap();
    assert_eq!(
        scene
            .world
            .shots(&scene.objects, scene.owner)
            .unwrap()
            .count(),
        83
    );
}

#[test]
fn obstacle_cue_precedes_low_shield_and_action_fault_preserves_only_earlier_writes() {
    let mut scene = Scene::new();
    scene.records().auxiliary.as_mut().unwrap().mode = 16;
    scene.records().visit.as_mut().unwrap().shield_warning_clock = 9;
    scene.records().contact.as_mut().unwrap().hit.reserve_shield = 3;
    scene
        .records()
        .action
        .as_mut()
        .unwrap()
        .install_triggered_projectile();
    let view = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    scene.world.fixed_players[0] = Some(view);
    let mut obstacle = Object::new(
        ObjectKind::Enemy,
        ShapeId::TITLE_CRAFT,
        Behavior::Unassigned,
    );
    obstacle.base.flags.proximity_warning_source = true;
    obstacle.base.position.z = 70;
    let obstacle = scene.objects.allocate(obstacle).unwrap();
    let old_position = Vector3 {
        x: 99,
        y: 88,
        z: 77,
    };
    scene.world.published_motion = Some(PublishedPlayerMotion {
        position: old_position,
        delta: Vector3 {
            x: 13,
            y: 15,
            z: 17,
        },
    });
    scene.world.scene.active_shield = Some(222);
    scene.world.scene.player_configuration = Some(0);
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .base
        .flags
        .view_side_filter = true;
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::Action(
            PlayerActionError::MissingPalette
        )))
    );
    assert!(scene.execution.is_faulted());
    assert!(
        scene
            .objects
            .get(obstacle)
            .unwrap()
            .base
            .flags
            .proximity_warning_latched
    );
    assert!(
        !scene
            .objects
            .get(scene.owner)
            .unwrap()
            .base
            .flags
            .view_side_filter
    );
    assert_eq!(
        scene.cues(),
        vec![
            SoundEvent::Authored(AuthoredCue::new(171, 0, PlayerTarget::Primary)),
            SoundEvent::Authored(AuthoredCue::new(23, 0, PlayerTarget::Primary)),
        ]
    );
    assert_eq!(
        scene.world.published_motion,
        Some(PublishedPlayerMotion {
            position: old_position,
            delta: Vector3::default()
        })
    );
    assert_eq!(scene.world.scene.active_shield, Some(222));
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0
    );
    assert_eq!(scene.records().action.unwrap().elapsed, 0);
    scene.world.palette = Some(ScenePalette {
        colors: [7; 128],
        saved_colors: [8; 128],
    });
    assert_eq!(scene.visit(), Err(SceneError::Faulted));
    assert_eq!(scene.world.palette.unwrap().saved_colors, [8; 128]);
}

#[test]
fn action_runs_before_equipment_publication_and_motion_position_precedes_mode_read() {
    let mut scene = Scene::new();
    scene.records().action = Some(PlayerActionState {
        action: Some(PlayerAction::TriggeredProjectile),
        elapsed: 0,
        auxiliary_counter: 71,
        total_updates: 19,
    });
    scene.world.scene.player_configuration = Some(0);
    scene.world.palette = Some(ScenePalette {
        colors: [0xFFFA; 128],
        saved_colors: [0; 128],
    });
    scene.records().equipment = None;
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::World(
            WorldInputError::MissingEquipment(scene.owner)
        )))
    );
    assert_eq!(
        scene.world.palette.as_ref().unwrap().saved_colors,
        [0xFFFA; 128]
    );
    assert_eq!(scene.records().action.unwrap().elapsed, 1);
    assert_eq!(scene.world.scene.active_shield, Some(0));
    assert!(scene.world.active_consumables.is_none());

    let mut scene = Scene::new();
    scene.world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    scene.records().auxiliary = None;
    let position = Vector3 {
        x: -19,
        y: 21,
        z: 313,
    };
    scene.objects.get_mut(scene.owner).unwrap().base.position = position;
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::World(
            WorldInputError::MissingAuxiliary(scene.owner)
        )))
    );
    assert_eq!(scene.world.scene.active_weapon_level, Some(7));
    assert_eq!(
        scene.world.published_motion,
        Some(PublishedPlayerMotion {
            position,
            delta: Vector3::default()
        })
    );
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0
    );
}

#[test]
fn following_and_explosion_scrolling_read_publication_but_inheritance_reads_live_primary() {
    let mut scene = Scene::new();
    scene.records().auxiliary.as_mut().unwrap().mode = 16;
    scene.objects.get_mut(scene.owner).unwrap().base.velocity = Vector3 {
        x: 31,
        y: 43,
        z: -59,
    };
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .path_state
        .motion_delta = Vector3 {
        x: 91,
        y: 93,
        z: 97,
    };
    scene.visit().unwrap();
    // Deliberately mutate both fresh primary sources after publication.
    scene.objects.get_mut(scene.owner).unwrap().base.velocity.x = 701;
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .path_state
        .motion_delta
        .x = 801;
    let secondary = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    scene.world.secondary_player = Some(secondary);
    scene
        .world
        .bind_player(
            &scene.objects,
            secondary,
            PlayerPathRecords {
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0,
                    action_flags: 4,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                ..Default::default()
            },
        )
        .unwrap();
    let mut actor = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    actor.extension.path_state.motion.follow_player_displacement = true;
    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
    actor.base.path = Some(crate::authored_paths::PLAYER_CHARGE_ORB);
    let follower = scene.objects.allocate(actor).unwrap();
    let mut invocation = PathInvocation::default();
    invocation.runtime.enter(&scene.objects, follower).unwrap();
    invocation
        .begin(follower, InvocationEntry::Movement)
        .unwrap();
    invocation
        .resume(&scene.catalog, &mut scene.objects, &mut scene.world, 64)
        .unwrap();
    assert_eq!(
        scene.objects.get(follower).unwrap().base.position,
        Vector3 { x: 0, y: 0, z: -59 }
    );
    let fresh = scene
        .world
        .path_world(&scene.objects, follower, PlayerTarget::Secondary)
        .unwrap();
    assert_eq!(fresh.primary_motion.unwrap().displacement.x, 801);
    assert_eq!(fresh.published_motion.unwrap().delta.x, 31);
    drop(fresh);
    let mut explosion = Object::new(
        ObjectKind::Effect,
        ShapeId::EMPTY,
        Behavior::Destruction(crate::common_destruction::EffectPhase::Animate),
    );
    explosion.base.hit_points = 1;
    explosion.base.acceleration = 20;
    let explosion = scene.objects.allocate(explosion).unwrap();
    scene.host().run_strategy(explosion, 7).unwrap();
    assert_eq!(
        scene.objects.get(explosion).unwrap().base.position,
        Vector3 {
            x: -62,
            y: 0,
            z: 118
        }
    );
}

#[test]
fn live_scripted_mode_gates_warnings_without_requiring_allocation_observations() {
    let mut scene = Scene::new();
    scene.world.spawn_defaults = None;
    scene.world.view_transition_mode =
        Some(crate::view_transition::ViewTransitionMode { flags: 2 });
    scene.records().auxiliary.as_mut().unwrap().mode = 16;
    let mut obstacle = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
    obstacle.base.flags.proximity_warning_source = true;
    scene.objects.allocate(obstacle).unwrap();
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(83))
        .unwrap();
    scene.visit().unwrap();
    assert_eq!(
        scene
            .world
            .shots(&scene.objects, scene.owner)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(scene.world.scene.active_weapon_level, Some(7));
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1
    );
    assert!(scene.cues().is_empty());

    // The canonical word, not the stale initializer, re-enables the warning.
    scene.world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 219,
        run_when_paused: true,
    });
    scene
        .world
        .view_transition_mode
        .as_mut()
        .unwrap()
        .set_active(false);
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::Warning(
            WarningError::MissingView
        )))
    );
    assert!(scene.execution.is_faulted());
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1
    );
}

#[test]
fn missing_warning_view_is_lazy_and_limits_precede_all_later_faults() {
    let mut scene = Scene::new();
    scene.records().auxiliary.as_mut().unwrap().mode = 16;
    let mut obstacle = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
    obstacle.base.flags.proximity_warning_source = true;
    obstacle.base.position.y = 300;
    let obstacle = scene.objects.allocate(obstacle).unwrap();
    scene.visit().unwrap();
    scene.objects.get_mut(obstacle).unwrap().base.position.y = 299;
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .base
        .flags
        .view_side_filter = true;
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::Warning(
            WarningError::MissingView
        )))
    );
    assert!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .base
            .flags
            .view_side_filter
    );

    let mut scene = Scene::new();
    scene.records().visit.as_mut().unwrap().pilot_code = 4;
    scene.records().contact = None;
    assert_eq!(
        scene.visit(),
        Err(SceneError::PlayerVisit(PlayerVisitError::World(
            WorldInputError::MissingPlayerContact(scene.owner)
        )))
    );
    assert_eq!(scene.world.active_charge_threshold, Some(10));
    assert_eq!(scene.world.active_shield_capacity, Some(24));
}
