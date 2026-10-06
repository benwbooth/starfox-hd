use super::*;
use crate::collision_pass::ExclusionGroups;
use crate::common_destruction::EffectInputs;
use crate::native::path_player_control::PlayerTargetControl;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{ActionGate, PathCatalog, ProjectileTrigger, SelectedAuxiliaryState};
use crate::path_sound::{AuthoredCue, CueListener};
use crate::player_action::{PlayerAction, PlayerActionState, PlayerServiceFlags};
use crate::scene_path_world::{AudioRouting, PlayerPathRecords};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::{
    Angle, Button, Buttons, InputState, ObjectSpawnDefaults, RandomState, SoundEvent, Vector3,
};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected external strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        // These authored effect paths install no native death registrations.
        Ok(None)
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
    owner: ObjectId,
    head: ObjectId,
}

impl Scene {
    fn new(kind: u8) -> Self {
        let mut objects = ObjectStore::new();
        let mut player = Object::new(
            ObjectKind::Player,
            ShapeId::TITLE_CRAFT,
            Behavior::Unassigned,
        );
        player.base.hit_points = 30;
        player.base.flags.exclude_from_shape_footprint_search = true;
        player.base.position = Vector3 {
            x: 135,
            y: -2000,
            z: 731,
        };
        player.base.pitch = Angle::from_units(3);
        player.base.yaw = Angle::from_units(17);
        player.base.roll = Angle::from_units(29);
        let owner = objects.allocate(player).unwrap();
        let mut marker = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
        marker.base.hit_points = 1;
        marker.base.flags.exclude_from_shape_footprint_search = true;
        let head = objects.allocate(marker).unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([13, 47, 89, 157]));
        world.primary_player = Some(owner);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 67,
            run_when_paused: false,
        });
        world.active_shield_capacity = Some(100);
        world.scene.active_shield = Some(42);
        // Special configuration suppresses only the action's palette snapshot.
        world.scene.player_configuration = Some(9);
        world.projectile_trigger = Some(ProjectileTrigger { activation: 251 });
        world.player_service_flags = Some(PlayerServiceFlags::from_bits(0x81));
        world.shield_recovery = Some(Default::default());
        world.action_gate = Some(ActionGate::default());
        world.surface_mode = Some(crate::collision_surface::SurfaceMode { flags: 1 });
        world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
            markers: None,
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    action: Some(PlayerActionState {
                        total_updates: u16::MAX,
                        ..Default::default()
                    }),
                    consumable: Some(PlayerConsumableControl {
                        input_control: 0xA7,
                        ..Default::default()
                    }),
                    target_control: Some(PlayerTargetControl::default()),
                    contact: Some(crate::scene_contact::PlayerContactControl {
                        hit: crate::player_hit_control::PlayerHitControl {
                            reserve_shield: 42,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    protection: Some(DeflectionProtection::from_control(0)),
                    equipment: Some(SelectedEquipment {
                        packed_consumables: 0xB3,
                        consumable_type: kind,
                        weapon_level: 2,
                    }),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x10,
                        action_flags: 1,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    occupancy_exempt: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut execution = SceneExecution::default();
        execution.controls.death_effects = Some(EffectInputs {
            spawn: world.spawn_defaults.unwrap(),
            primary_marker: objects.get(owner).unwrap().base.position,
            secondary_marker: None,
        });
        Self {
            objects,
            world,
            execution,
            catalog: authored_paths::catalog(),
            callbacks: Callbacks,
            owner,
            head,
        }
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 200,
        }
    }
    fn use_item(&mut self) -> Result<bool, SceneError<&'static str>> {
        let owner = self.owner;
        self.host().use_player_consumable(owner)
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn fill_pool(&mut self) {
        while self.objects.len() < OBJECT_CAPACITY {
            self.objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
    }
    fn effect(&self) -> Option<ObjectId> {
        path_relationships::find_direct_child(&self.objects, self.owner, 22).unwrap()
    }
    fn newest(&self) -> ObjectId {
        self.objects.active_ids()[1]
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
fn every_packed_count_and_type_preserves_wrapped_type_aliases_and_original_empty_handlers() {
    let mut scene = Scene::new(0);
    scene.fill_pool();
    for packed in 0..=u8::MAX {
        for kind in 0..=u8::MAX {
            *scene.records().equipment.as_mut().unwrap() = SelectedEquipment {
                packed_consumables: packed,
                consumable_type: kind,
                weapon_level: 177,
            };
            scene.records().action.as_mut().unwrap().action = None;
            scene.records().protection = Some(DeflectionProtection::from_control(0xA0));
            scene.world.projectile_trigger.as_mut().unwrap().activation = 231;
            let consumed = packed % 16 != 0 && kind % 128 != 0;
            assert_eq!(
                scene.use_item().unwrap(),
                consumed,
                "count {packed}, type {kind}"
            );
            assert_eq!(
                scene.records().equipment.unwrap(),
                SelectedEquipment {
                    packed_consumables: if consumed { packed - 1 } else { packed },
                    consumable_type: kind,
                    weapon_level: 177,
                }
            );
            assert_eq!(
                scene.records().action.unwrap().action.is_some(),
                consumed && kind % 128 == 1
            );
            assert_eq!(
                scene.world.projectile_trigger.unwrap().activation,
                if consumed && kind % 128 == 1 { 0 } else { 231 }
            );
            assert_eq!(
                scene.records().protection.unwrap().control(),
                if consumed && kind % 128 == 3 {
                    0xBF
                } else {
                    0xA0
                }
            );
            assert_eq!(scene.records().consumable.unwrap().input_control, 0xA7);
        }
    }
}

#[test]
fn all_deflection_flags_reject_active_count_or_projectile_protection_and_preserve_other_bits() {
    let mut scene = Scene::new(3);
    scene.records().action = None;
    scene.records().contact = None;
    scene.records().consumable = None;
    for flags in 0..=u8::MAX {
        scene.records().protection = Some(DeflectionProtection::from_control(flags));
        scene
            .records()
            .equipment
            .as_mut()
            .unwrap()
            .packed_consumables = 0xD1;
        let accepted = flags & 0x5F == 0;
        assert_eq!(scene.use_item().unwrap(), accepted);
        assert_eq!(
            scene.records().protection.unwrap().control(),
            if accepted { flags | 31 } else { flags }
        );
        assert_eq!(
            scene.records().equipment.unwrap().packed_consumables,
            if accepted { 0xD0 } else { 0xD1 }
        );
        assert_eq!(scene.objects.len(), 2);
    }
}

#[test]
fn healing_compares_caller_reserve_for_equality_to_published_capacity_not_active_shield() {
    let mut scene = Scene::new(0);
    scene.world.scene.active_shield = None;
    for shield in 0..=u8::MAX {
        for capacity in 0..=u8::MAX {
            scene
                .records()
                .equipment
                .as_mut()
                .unwrap()
                .packed_consumables = 2;
            scene.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
            scene.world.active_shield_capacity = Some(capacity);
            assert_eq!(scene.use_item().unwrap(), shield != capacity);
            assert_eq!(scene.records().contact.unwrap().hit.reserve_shield, shield);
            if let Some(effect) = scene.effect() {
                scene.objects.remove(effect).unwrap();
            }
            assert_eq!(scene.objects.len(), 2);
        }
    }
    scene.world.active_shield_capacity = None;
    scene.records().contact = None;
    scene
        .records()
        .consumable
        .as_mut()
        .unwrap()
        .recovery_blocked = true;
    assert!(!scene.use_item().unwrap());
}

#[test]
fn triggered_flags_and_active_stream_gate_before_allocation_and_unreached_inputs() {
    let mut scene = Scene::new(1);
    scene.fill_pool();
    scene.records().contact = None;
    scene.records().protection = None;
    for flags in 0..=u8::MAX {
        scene.records().action.as_mut().unwrap().action = None;
        scene
            .records()
            .equipment
            .as_mut()
            .unwrap()
            .packed_consumables = 1;
        scene
            .records()
            .consumable
            .as_mut()
            .unwrap()
            .projectile_blockers = TriggeredUseBlockers::from_control(flags);
        assert_eq!(
            scene
                .records()
                .consumable
                .unwrap()
                .projectile_blockers
                .bits(),
            flags & 0x18
        );
        assert_eq!(scene.use_item().unwrap(), flags & 0x18 == 0);
    }
    scene.records().action.as_mut().unwrap().action = Some(PlayerAction::TriggeredProjectile);
    scene
        .records()
        .equipment
        .as_mut()
        .unwrap()
        .packed_consumables = 1;
    scene.records().consumable = None;
    scene.world.projectile_trigger = None;
    assert!(!scene.use_item().unwrap());
}

#[test]
fn fresh_installers_share_formatting_but_keep_different_attachment_pause_and_self_frame_rules() {
    for kind in [0, 1, 128, 129] {
        let mut scene = Scene::new(kind);
        let before = scene.objects.get(scene.owner).unwrap().base.clone();
        let random = scene.world.random;
        assert!(scene.use_item().unwrap());
        let effect = scene.newest();
        assert_eq!(
            scene.objects.active_ids(),
            &[scene.head, effect, scene.owner]
        );
        let actor = scene.objects.get(effect).unwrap();
        let healing = kind % 128 == 0;
        assert_eq!(actor.base.behavior, Behavior::FollowPath);
        assert_eq!(actor.base.shape, ShapeId::EMPTY);
        assert_eq!(
            actor.base.path,
            Some(if healing {
                authored_paths::ATTACHED_RECOVERY_EFFECT
            } else {
                authored_paths::TRIGGERED_LINKED_PROJECTILE
            })
        );
        assert_eq!(actor.base.attachment, healing.then_some(scene.owner));
        assert_eq!(actor.base.child_number, if healing { 22 } else { 0 });
        assert_eq!(actor.extension.parent, healing.then_some(effect));
        assert_eq!(actor.extension.spawn_group, 255);
        assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
        assert!(actor.extension.path_state.needs_path_initialization);
        assert!(actor.base.flags.general_search_eligible && actor.base.flags.collision_disabled);
        assert_eq!(actor.base.contacts.run_when_paused, healing);
        assert!(!actor
            .base
            .contacts
            .exclusion_groups
            .excludes(ExclusionGroups::PATH_SPAWN));
        assert_eq!(
            (
                actor.base.position,
                actor.base.pitch,
                actor.base.yaw,
                actor.base.roll
            ),
            (before.position, before.pitch, before.yaw, before.roll)
        );
        assert_eq!(scene.records().equipment.unwrap().packed_consumables, 0xB2);
        assert_eq!(scene.world.random, random);
        if healing {
            scene
                .objects
                .get_mut(effect)
                .unwrap()
                .base
                .flags
                .remove_after_tick = true;
            scene.world.spawn_defaults = None;
            assert!(!install_recovery(&mut scene.objects, &scene.world, scene.owner).unwrap());
        }
    }
}

#[test]
fn full_pool_still_starts_triggered_timeline_and_consumes_but_healing_does_neither() {
    for kind in [0, 1] {
        let mut scene = Scene::new(kind);
        scene.fill_pool();
        assert_eq!(scene.use_item().unwrap(), kind == 1);
        assert_eq!(scene.objects.len(), OBJECT_CAPACITY);
        assert_eq!(
            scene.records().equipment.unwrap().packed_consumables,
            if kind == 1 { 0xB2 } else { 0xB3 }
        );
        if kind == 1 {
            assert_eq!(scene.world.projectile_trigger.unwrap().activation, 0);
            let owner = scene.owner;
            for time in 0..=40 {
                scene
                    .host()
                    .advance_player_action(owner, InputState::default())
                    .unwrap();
                assert_eq!(
                    scene.world.projectile_trigger.unwrap().activation,
                    u8::from(time >= 12)
                );
            }
            assert_eq!(
                scene.records().action.unwrap(),
                PlayerActionState {
                    action: None,
                    elapsed: 1,
                    auxiliary_counter: 0,
                    total_updates: 40
                }
            );
            assert!(scene.use_item().unwrap());
            assert_eq!(scene.world.projectile_trigger.unwrap().activation, 0);
        }
    }
}

#[test]
fn missing_trigger_fault_preserves_installed_path_and_action_but_prevents_duplicate_allocation() {
    let mut scene = Scene::new(1);
    scene.world.projectile_trigger = None;
    assert_eq!(
        scene.use_item(),
        Err(SceneError::Consumable(
            ConsumableError::MissingProjectileTrigger
        ))
    );
    assert_eq!(scene.objects.len(), 3);
    assert_eq!(
        scene.objects.get(scene.newest()).unwrap().base.path,
        Some(authored_paths::TRIGGERED_LINKED_PROJECTILE)
    );
    assert_eq!(
        scene.records().action.unwrap().action,
        Some(PlayerAction::TriggeredProjectile)
    );
    assert_eq!(scene.records().equipment.unwrap().packed_consumables, 0xB3);
    scene.world.projectile_trigger = Some(ProjectileTrigger::default());
    assert_eq!(scene.use_item(), Err(SceneError::Faulted));
    assert_eq!(scene.objects.len(), 3);

    let mut scene = Scene::new(0);
    scene.world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    scene.objects.remove(scene.owner).unwrap();
    assert!(!scene.use_item().unwrap());
}

#[test]
fn healing_paths_publish_real_shared_requests_and_release_all_children_and_program_resources() {
    let mut scene = Scene::new(0);
    // This scenario keeps the player stationary throughout the effect.
    scene.records().suppress_horizontal_follow = Some(false);
    scene.world.published_motion = Some(Default::default());
    assert!(scene.use_item().unwrap());
    let effect = scene.effect().unwrap();
    let mut schedule = StrategySchedule::default();
    let mut requests = Vec::new();
    let mut max_actors = 0;
    for _ in 0..100 {
        scene.epoch(&mut schedule);
        max_actors = max_actors.max(scene.objects.len());
        let amount = std::mem::take(&mut scene.world.shield_recovery.as_mut().unwrap().amount);
        if amount != 0 {
            requests.push(amount);
        }
        if scene.objects.len() == 2 {
            break;
        }
    }
    assert_eq!(max_actors, 5);
    // Both children publish 40 into the SAME shared request on each pulse;
    // these are replacement stores, not a fabricated 80-point accumulation.
    assert_eq!(requests, [40, 40, 40]);
    assert_eq!(scene.records().contact.unwrap().hit.reserve_shield, 42);
    assert_eq!(scene.objects.len(), 2);
    assert!(scene.objects.get(effect).is_none());
    assert_eq!(
        scene.execution.paths.runtime.resources.owner_count(effect),
        0
    );
    assert_eq!(scene.effect(), None);
    assert_eq!(
        scene.execution.paths.runtime.resources.available_capacity(),
        crate::program_resources::PROGRAM_CAPACITY
    );
}

#[test]
fn triggered_item_timeline_drives_real_projectile_lock_recoil_and_full_retirement() {
    for early in [false, true] {
        for initial_recoil in [0, 96, -32768] {
            let mut scene = Scene::new(1);
            scene
                .records()
                .contact
                .as_mut()
                .unwrap()
                .hit
                .camera_pitch_recoil = initial_recoil;
            assert!(scene.use_item().unwrap());
            let parent = scene.newest();
            scene
                .records()
                .consumable
                .as_mut()
                .unwrap()
                .projectile_blockers = TriggeredUseBlockers::from_control(0x18);
            let mut schedule = StrategySchedule::default();
            let mut child = None;
            let mut locked = false;
            let owner = scene.owner;
            for time in 0..=60 {
                let input = InputState {
                    held: Buttons::from_bits(Button::X as u16),
                    pressed: Buttons::from_bits(if early && time == 2 {
                        Button::X as u16
                    } else {
                        0
                    }),
                };
                scene.host().advance_player_action(owner, input).unwrap();
                scene.epoch(&mut schedule);
                if let Some(current) = scene
                    .objects
                    .get(parent)
                    .and_then(|actor| actor.base.first_child)
                {
                    assert_eq!(*child.get_or_insert(current), current);
                }
                let control = scene.records().target_control.unwrap();
                if let Some(shot) = child {
                    if control.owner == Some(shot) {
                        locked = true;
                        assert!(control.configuration_locked);
                        assert_eq!(
                            scene
                                .records()
                                .consumable
                                .unwrap()
                                .projectile_blockers
                                .bits(),
                            0x18
                        );
                        assert_eq!(control.axis_rates, [3, 3, 2]);
                        assert_eq!(
                            scene.records().contact.unwrap().hit.camera_pitch_recoil,
                            if initial_recoil == 0 {
                                128
                            } else {
                                initial_recoil
                            }
                        );
                    }
                }
                if scene.objects.len() == 2 && scene.records().action.unwrap().action.is_none() {
                    break;
                }
            }
            assert!(locked);
            let child = child.unwrap();
            assert_eq!(scene.objects.len(), 2);
            assert_eq!(scene.records().action.unwrap().action, None);
            assert_eq!(
                scene.execution.paths.runtime.resources.owner_count(parent),
                0
            );
            assert_eq!(
                scene.execution.paths.runtime.resources.owner_count(child),
                0
            );
            assert_eq!(
                scene.execution.paths.runtime.resources.available_capacity(),
                crate::program_resources::PROGRAM_CAPACITY
            );
            let sounds = scene
                .world
                .audio
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(
                sounds,
                [42, 43].map(|id| SoundEvent::Authored(AuthoredCue::new(
                    id,
                    0,
                    PlayerTarget::Primary
                )))
            );
        }
    }
}

#[test]
fn primary_control_borrow_never_uses_selected_player_or_requires_unused_link_mode() {
    let mut scene = Scene::new(1);
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
                target_control: Some(PlayerTargetControl {
                    mode: 231,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    let mut world = scene
        .world
        .path_world(&scene.objects, secondary, PlayerTarget::Secondary)
        .unwrap();
    let control = world.primary_control.as_mut().unwrap();
    assert_eq!(control.linked_mode, None);
    control.target.mode = 37;
    assert_eq!(scene.records().target_control.unwrap().mode, 37);
    assert_eq!(
        scene
            .world
            .player(&scene.objects, secondary)
            .unwrap()
            .target_control
            .unwrap()
            .mode,
        231
    );
}

#[test]
fn common_formatter_clears_only_path_exclusion_and_retains_all_other_actor_state() {
    let mut scene = Scene::new(0);
    let effect = scene
        .objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let pose = scene.objects.get(scene.owner).unwrap().base.clone();
    for class in 0..=u8::MAX {
        let object = scene.objects.get_mut(effect).unwrap();
        object.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(class);
        object.base.contacts.weapon_formatted = class & 2 != 0;
        object.base.contacts.first_strategy_visit = class & 4 != 0;
        object.base.contacts.suppress_attack_damage = class & 1 != 0;
        object.base.velocity = Vector3 {
            x: 39,
            y: -51,
            z: 103,
        };
        object.base.attachment = Some(scene.owner);
        object.extension.parent = Some(scene.head);
        object.extension.relative_position = Vector3 {
            x: 531,
            y: 733,
            z: -1224,
        };
        object.extension.path_state.script_value = 0xFA92;
        let mut expected = object.clone();
        expected.base.behavior = Behavior::FollowPath;
        expected.extension.path_state.needs_path_initialization = true;
        expected.extension.spawn_group = 255;
        expected.base.hit_points = 1;
        expected.base.attack_power = 1;
        expected.base.flags.general_search_eligible = true;
        expected.base.position = pose.position;
        expected.base.pitch = pose.pitch;
        expected.base.yaw = pose.yaw;
        expected.base.roll = pose.roll;
        expected.base.contacts.run_when_paused = true;
        expected.base.contacts.exclusion_groups =
            ExclusionGroups::from_authored_class(class & 0xEF);
        expected.base.flags.collision_disabled = true;
        crate::native::player_effect::format(&mut scene.objects, scene.owner, effect).unwrap();
        assert_eq!(scene.objects.get(effect), Some(&expected));
    }
}

#[test]
fn linked_mode_is_required_only_by_primary_control_commands_that_read_it() {
    use crate::native::path_player_control::PlayerControlCommand;
    use crate::path_program::{ProgramError, Statement};
    use crate::{PathCursor, PathId};
    for command in [
        PlayerControlCommand::LockToProjectile,
        PlayerControlCommand::LockForLinkedMode,
        PlayerControlCommand::FollowPrimaryPosition,
        PlayerControlCommand::RefreshOwnedOrigin,
    ] {
        let mut scene = Scene::new(1);
        let cursor = PathCursor {
            path: PathId::from_catalog_index(0),
            command_index: 0,
        };
        let catalog = PathCatalog::new(vec![vec![Statement::PlayerControl {
            command,
            next: cursor,
        }]])
        .unwrap();
        scene.objects.get_mut(scene.head).unwrap().base.path = Some(cursor);
        let before = scene.records().target_control;
        let mut world = scene
            .world
            .path_world(&scene.objects, scene.head, PlayerTarget::Secondary)
            .unwrap();
        let result = scene.execution.paths.runtime.step_program(
            &catalog,
            &mut scene.objects,
            scene.head,
            &mut world,
        );
        if matches!(
            command,
            PlayerControlCommand::LockForLinkedMode | PlayerControlCommand::FollowPrimaryPosition
        ) {
            assert_eq!(result, Err(ProgramError::MissingPrimaryLinkedMode));
            assert_eq!(scene.records().target_control, before);
        } else {
            assert_eq!(result.unwrap().actor, scene.head);
        }
    }
}
