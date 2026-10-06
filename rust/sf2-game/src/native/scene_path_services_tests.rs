use super::super::path_countdown::PathCountdown;
use super::super::path_death::FriendHealth;
use super::super::path_protection::{
    self, DeflectionProtection, LinkedEffectActivity, ProtectionError,
};
use super::*;
use crate::authored_paths;
use crate::collision_surface::SurfaceMode;
use crate::path_commands::ControlCommand;
use crate::path_fields::{ByteField, BytePart, WordField};
use crate::path_invocation::{InvocationEntry, InvocationError, PathInvocation};
use crate::path_program::{PathCatalog, ProgramError, Statement};
use crate::path_radio::{RadioLayout, RadioRequest};
use crate::path_target::{PublishedHomingTarget, TargetingUpgradeState};
use crate::player_action::PlayerServiceFlags;
use crate::player_input::PlayerInputSettings;

fn cursor(path: u16, command_index: u16) -> crate::PathCursor {
    crate::PathCursor {
        path: crate::PathId::from_catalog_index(path),
        command_index,
    }
}

fn actor(objects: &mut ObjectStore, path: Option<crate::PathCursor>) -> ObjectId {
    let mut actor = crate::Object::new(
        crate::ObjectKind::Effect,
        crate::ShapeId::EMPTY,
        crate::Behavior::FollowPath,
    );
    actor.base.path = path;
    actor.base.hit_points = 1;
    objects.allocate(actor).unwrap()
}

fn visit(
    invocation: &mut PathInvocation,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) {
    invocation.begin(owner, InvocationEntry::Program).unwrap();
    assert_eq!(invocation.resume(catalog, objects, world, 128), Ok(owner));
}

#[test]
fn uninitialized_services_remain_absent_instead_of_fabricating_cleared_state() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    let input = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert!(input.reflection.is_none());
    assert!(input.friend_health.is_none());
    assert!(input.targeting_upgrade.is_none());
    assert!(input.environment_plane_height.is_none());
    assert!(input.linked_effect_activity.is_none());
    assert!(input.button_layout.is_none());
    assert!(input.published_homing_target.is_none());
    assert!(input.countdown.is_none());
    let protection = input.protection.unwrap();
    assert_eq!(protection.rules, Default::default());
    assert!(protection.linked.is_none());
}

#[test]
fn radio_uses_the_live_reticle_axis_after_its_first_publication() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.radio = Some((
        RadioRequest::default(),
        RadioLayout {
            compact_panel: false,
            tracked_screen_y: 200,
        },
    ));
    // Explicit entry observations remain usable before display publishes.
    world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap()
        .radio
        .unwrap()
        .request_message(3);
    assert!(world.radio.as_ref().unwrap().0.top_placement);
    for compact_panel in [false, true] {
        world.radio.as_mut().unwrap().1.compact_panel = compact_panel;
        for vertical in 0..=u8::MAX {
            world.target_reticle.vertical = Some(vertical);
            // Horizontal is deliberately on the opposite side of the panel
            // threshold; it must not feed the message-placement consumer.
            world.target_reticle.horizontal = Some(vertical.wrapping_add(128));
            world
                .path_world(&objects, owner, PlayerTarget::Primary)
                .unwrap()
                .radio
                .unwrap()
                .request_message(vertical);
            let request = world.radio.as_ref().unwrap().0;
            let top = !(18..146).contains(&vertical);
            assert_eq!(request.top_placement, top);
            assert_eq!(
                request.panel_y,
                if top {
                    35
                } else if compact_panel {
                    139
                } else {
                    151
                }
            );
            assert_eq!(world.radio.as_ref().unwrap().1.tracked_screen_y, 200);
        }
    }
}

#[test]
fn radio_resamples_boss_bar_maximum_after_each_authored_store_in_the_same_visit() {
    use crate::path_fields::ByteOperand;
    use crate::path_radio::MessageIndex;
    use crate::path_scene_state::{
        CoordinationCommand, EncounterHealthDisplay, HealthDisplayField,
    };

    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, Some(cursor(0, 0)));
    let mut world = ScenePathWorld::new(RandomState::default());
    world.health_display = Some(EncounterHealthDisplay {
        current: 19,
        maximum: 0,
        label: None,
    });
    world.target_reticle.vertical = Some(100);
    world.radio = Some((
        RadioRequest::default(),
        RadioLayout {
            compact_panel: false,
            tracked_screen_y: 200,
        },
    ));
    for maximum in 0..=u8::MAX {
        world.health_display.as_mut().unwrap().maximum = maximum.wrapping_add(1);
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        let catalog = PathCatalog::new(vec![vec![
            Statement::HealthDisplay {
                field: HealthDisplayField::Maximum,
                command: CoordinationCommand::Assign(ByteOperand::Literal(maximum)),
                next: cursor(0, 1),
            },
            Statement::Message {
                number: ByteOperand::Literal(7),
                next: cursor(0, 2),
            },
            Statement::Control(ControlCommand::End),
        ]])
        .unwrap();
        visit(
            &mut PathInvocation::default(),
            &catalog,
            &mut objects,
            &mut world,
            owner,
        );
        let request = world.radio.as_ref().unwrap().0;
        assert_eq!(request.message, MessageIndex::from_authored_number(7));
        assert_eq!(request.panel_y, if maximum == 0 { 151 } else { 139 });
        assert!(!request.top_placement);
        assert_eq!(world.health_display.unwrap().current, 19);
    }
}

#[test]
fn shared_services_borrow_the_canonical_owner_and_resample_each_publication() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let target = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    for byte in 0..=u8::MAX {
        world.friend_health = Some(FriendHealth {
            remaining: [byte; 5],
        });
        world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: byte });
        world.environment_plane_height = Some(i16::from(byte) - 128);
        world.linked_effect_activity = Some(LinkedEffectActivity { recent_spawn: byte });
        world.countdown = Some(PathCountdown { remaining: byte });
        world.published_homing_target = Some(PublishedHomingTarget {
            object: (byte & 1 != 0).then_some(target),
        });
        world.player_input_settings = Some(PlayerInputSettings {
            flight_style: !byte,
            button_layout: byte,
        });
        let mut input = world
            .path_world(&objects, owner, PlayerTarget::Secondary)
            .unwrap();
        assert_eq!(input.environment_plane_height, Some(i16::from(byte) - 128));
        assert_eq!(input.button_layout, Some(byte));
        assert_eq!(
            input.published_homing_target.unwrap().object,
            (byte & 1 != 0).then_some(target)
        );
        input.friend_health.as_mut().unwrap().remaining[2] = byte ^ 0xA5;
        input
            .targeting_upgrade
            .as_mut()
            .unwrap()
            .acquire_for_active_pilot();
        input.linked_effect_activity.as_mut().unwrap().recent_spawn = byte.wrapping_add(1);
        input.countdown.as_mut().unwrap().remaining = byte.wrapping_sub(1);
        drop(input);
        assert_eq!(
            world.friend_health.unwrap().remaining,
            [byte, byte, byte ^ 0xA5, byte, byte]
        );
        assert_eq!(world.targeting_upgrade.unwrap().pilot_flags, byte | 0x80);
        assert_eq!(
            world.linked_effect_activity.unwrap().recent_spawn,
            byte.wrapping_add(1)
        );
        assert_eq!(world.countdown.unwrap().remaining, byte.wrapping_sub(1));
    }
}

#[test]
fn button_layout_import_uses_every_original_byte_without_flight_style_narrowing() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::ImportButtonLayout {
            destination: ByteField::WordPart {
                field: WordField::MotionPhase,
                part: BytePart::Low,
            },
            next: cursor(0, 1),
        },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    for byte in 0..=u8::MAX {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects, Some(cursor(0, 0)));
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xABCD;
        let mut world = ScenePathWorld::new(RandomState::default());
        world.player_input_settings = Some(PlayerInputSettings {
            flight_style: !byte,
            button_layout: byte,
        });
        visit(
            &mut PathInvocation::default(),
            &catalog,
            &mut objects,
            &mut world,
            owner,
        );
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase,
            0xAB00 | u16::from(byte)
        );
    }
}

#[test]
fn authored_countdown_actors_share_one_live_record_without_adapter_clock_advances() {
    let catalog = authored_paths::catalog();
    let mut objects = ObjectStore::new();
    let first = actor(&mut objects, Some(authored_paths::SHARED_COUNTDOWN_SERVICE));
    let second = actor(&mut objects, Some(authored_paths::SHARED_COUNTDOWN_SERVICE));
    let mut world = ScenePathWorld::new(RandomState::default());
    world.countdown = Some(PathCountdown { remaining: 3 });
    let mut invocation = PathInvocation::default();
    for (owner, expected) in [(first, 2), (second, 1), (first, 0), (second, 0)] {
        visit(&mut invocation, &catalog, &mut objects, &mut world, owner);
        assert_eq!(world.countdown.unwrap().remaining, expected);
    }
    world.countdown.as_mut().unwrap().remaining = 197;
    visit(&mut invocation, &catalog, &mut objects, &mut world, second);
    assert_eq!(world.countdown.unwrap().remaining, 196);
    assert_eq!(
        objects.get(second).unwrap().extension.surface_contact.group,
        197
    );
    assert_eq!(world.strategy_clock, 0);
}

#[test]
fn protection_and_reflection_resolve_attachment_and_caller_not_selection() {
    let mut objects = ObjectStore::new();
    let caller = actor(&mut objects, None);
    let selected = actor(&mut objects, None);
    let linked = actor(&mut objects, None);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(caller);
    world.secondary_player = Some(selected);
    world.reflect_all_contacts = Some(true);
    for (owner, control) in [(caller, 0x40), (selected, 0x81), (linked, 0x1F)] {
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    protection: Some(DeflectionProtection::from_control(control)),
                    equipment: Some(SelectedEquipment::default()),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    for target in [linked, selected, caller] {
        objects.get_mut(caller).unwrap().base.attachment = Some(target);
        let mut input = world
            .path_world(&objects, caller, PlayerTarget::Secondary)
            .unwrap();
        let reflection = input.reflection.unwrap();
        assert_eq!(reflection.owner, caller);
        assert_eq!(reflection.player_scatter, Some(true));
        assert!(reflection.process_all);
        let protection = input.protection.as_mut().unwrap().linked.as_mut().unwrap();
        assert_eq!(protection.owner, target);
        *protection.state = DeflectionProtection::from_control(0x40);
        input.selected_equipment.as_mut().unwrap().weapon_level = 3;
        drop(input);
        assert_eq!(
            world
                .player(&objects, target)
                .unwrap()
                .protection
                .unwrap()
                .control(),
            0x40
        );
        assert_eq!(
            world
                .player(&objects, selected)
                .unwrap()
                .equipment
                .unwrap()
                .weapon_level,
            3
        );
    }
    // A replacement player allocation invalidates both observations even
    // while the object slot and published pointers remain unchanged.
    let mut resources = crate::program_resources::ProgramResources::default();
    let storage = resources
        .allocate_owned(
            caller,
            472,
            crate::program_state::ProgramData::PathStack(Default::default()),
        )
        .unwrap();
    objects.get_mut(caller).unwrap().base.player_storage = Some(storage);
    let input = world
        .path_world(&objects, caller, PlayerTarget::Secondary)
        .unwrap();
    assert_eq!(input.reflection.unwrap().player_scatter, None);
    assert!(input.protection.unwrap().linked.is_none());
    objects.remove(selected);
    let reused = actor(&mut objects, None);
    assert_eq!(reused, selected);
    objects.get_mut(caller).unwrap().base.attachment = Some(reused);
    let input = world
        .path_world(&objects, caller, PlayerTarget::Secondary)
        .unwrap();
    assert!(input.selected_equipment.is_none());
    assert!(input.protection.unwrap().linked.is_none());
}

#[test]
fn path_reflection_launches_from_live_contact_order_and_uses_caller_scatter() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::ReflectContactShots { next: cursor(0, 1) },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    for all in [false, true] {
        for scatter in [false, true] {
            let mut objects = ObjectStore::new();
            let owner = actor(&mut objects, Some(cursor(0, 0)));
            let selected = actor(&mut objects, None);
            let incoming = [actor(&mut objects, None), actor(&mut objects, None)];
            objects.get_mut(owner).unwrap().base.contacts.skip_contacts = true;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player = PlayerTarget::Secondary;
            let mut world = ScenePathWorld::new(RandomState::new([11, 17, 23, 29]));
            let mut expected_random = world.random;
            world.primary_player = Some(owner);
            world.secondary_player = Some(selected);
            world.reflect_all_contacts = Some(all);
            world.spawn_defaults = Some(ObjectSpawnDefaults::default());
            world.weapons = Some(Default::default());
            for (player, enabled) in [(owner, scatter), (selected, !scatter)] {
                world
                    .bind_player(
                        &objects,
                        player,
                        PlayerPathRecords {
                            protection: Some(DeflectionProtection::from_control(if enabled {
                                0x40
                            } else {
                                0
                            })),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
            for shot in incoming {
                objects
                    .get_mut(shot)
                    .unwrap()
                    .base
                    .contacts
                    .credits_hit_side = true;
                world
                    .contacts
                    .record_pair(owner, shot, [None, None])
                    .unwrap();
            }
            let first = world
                .contacts
                .get(world.contacts.first(owner).unwrap())
                .unwrap()
                .other;
            let reflected = if all { 2 } else { 1 };
            if scatter {
                for _ in 0..reflected * 2 {
                    expected_random.next_byte();
                }
            }
            visit(
                &mut PathInvocation::default(),
                &catalog,
                &mut objects,
                &mut world,
                owner,
            );
            assert_eq!(objects.len(), 4 + reflected);
            for shot in incoming {
                assert_eq!(
                    objects.get(shot).unwrap().base.flags.collision_disabled,
                    all || shot == first
                );
            }
            assert_eq!(world.random, expected_random);
        }
    }
}

#[test]
fn objective_low_byte_is_the_live_contact_gate_and_protection_observes_path_writes() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects, None);
    let linked = actor(&mut objects, None);
    objects.get_mut(owner).unwrap().base.attachment = Some(linked);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.contacts_enabled = Some(false);
    world.scene.player_configuration = Some(0);
    world.surface_mode = Some(SurfaceMode { flags: 1 });
    world.player_service_flags = Some(PlayerServiceFlags::from_bits(0));
    world
        .bind_player(
            &objects,
            linked,
            PlayerPathRecords {
                protection: Some(DeflectionProtection::from_control(31)),
                ..Default::default()
            },
        )
        .unwrap();
    for remaining in [0xAB01, 0xAB00, 0x00FF, 0] {
        world.objective_counts = Some(EncounterObjectiveCounts {
            remaining_word: remaining,
            ..Default::default()
        });
        assert_eq!(world.contacts_enabled(), Some(remaining as u8 != 0));
        world.player_mut(&objects, linked).unwrap().protection =
            Some(DeflectionProtection::from_control(31));
        let mut inputs = world
            .path_world(&objects, owner, PlayerTarget::Primary)
            .unwrap();
        assert_eq!(
            path_protection::update_effect(
                &mut objects,
                owner,
                inputs.protection.as_mut().unwrap(),
                inputs.surface_mode
            ),
            Ok(remaining as u8 != 0)
        );
        drop(inputs);
        assert_eq!(
            world
                .player(&objects, linked)
                .unwrap()
                .protection
                .unwrap()
                .control(),
            if remaining as u8 != 0 { 31 } else { 1 }
        );
    }
    use crate::path_scene_state::{CoordinationCommand, ObjectiveCountField};
    let catalog = PathCatalog::new(vec![vec![
        Statement::ObjectiveCounts {
            field: ObjectiveCountField::Remaining,
            command: CoordinationCommand::Decrement,
            next: cursor(0, 1),
        },
        Statement::UpdateProtectionEffect {
            ordinary_return: cursor(0, 2),
            flicker: cursor(0, 2),
        },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    world.objective_counts.as_mut().unwrap().remaining_word = 0xAB01;
    world.contacts_enabled = Some(true);
    world.player_mut(&objects, linked).unwrap().protection =
        Some(DeflectionProtection::from_control(31));
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
    visit(
        &mut PathInvocation::default(),
        &catalog,
        &mut objects,
        &mut world,
        owner,
    );
    assert_eq!(world.objective_counts.unwrap().remaining_word, 0xAB00);
    assert_eq!(world.contacts_enabled(), Some(false));
    assert_eq!(
        world
            .player(&objects, linked)
            .unwrap()
            .protection
            .unwrap()
            .control(),
        1
    );
}

#[test]
fn canonical_friend_health_and_homing_publication_are_used_by_actual_commands() {
    for slot in 1..=5 {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects, Some(cursor(0, 0)));
        let fresh_candidate = actor(&mut objects, None);
        let published = actor(&mut objects, None);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .friend_health_slot = slot;
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(fresh_candidate);
        world.friend_health = Some(FriendHealth { remaining: [40; 5] });
        world.published_homing_target = Some(PublishedHomingTarget {
            object: Some(published),
        });
        let catalog = PathCatalog::new(vec![vec![
            Statement::AttachPublishedHomingTarget { next: cursor(0, 1) },
            Statement::MarkForDeath,
        ]])
        .unwrap();
        visit(
            &mut PathInvocation::default(),
            &catalog,
            &mut objects,
            &mut world,
            owner,
        );
        let mut expected = [40; 5];
        expected[usize::from(slot - 1)] = 0;
        assert_eq!(world.friend_health.unwrap().remaining, expected);
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
        assert_eq!(objects.get(owner).unwrap().base.attachment, Some(published));
    }
}

#[test]
fn protection_override_branch_requires_only_its_own_observation() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::IfProtectionOverride {
            taken: cursor(0, 2),
            next: cursor(0, 1),
        },
        Statement::Control(ControlCommand::End),
        Statement::AcquireTargetingUpgrade { next: cursor(0, 1) },
    ]])
    .unwrap();
    for enabled in [false, true] {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects, Some(cursor(0, 0)));
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut invocation = PathInvocation::default();
        invocation.begin(owner, InvocationEntry::Program).unwrap();
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut world, 128),
            Err(InvocationError::Program(ProgramError::Protection(
                ProtectionError::MissingMinimumOverride
            )))
        );
        world.player_service_flags = Some(PlayerServiceFlags::from_bits(u8::from(enabled)));
        world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 7 });
        assert_eq!(
            invocation.resume(&catalog, &mut objects, &mut world, 128),
            Ok(owner)
        );
        assert_eq!(
            world.targeting_upgrade.unwrap().pilot_flags,
            if enabled { 0x87 } else { 7 }
        );
    }
}

#[test]
fn authored_protection_composes_live_scene_records_through_intro_and_shutdown() {
    let catalog = authored_paths::catalog();
    for initial_activity in [0, 1, 2, 255] {
        let mut objects = ObjectStore::new();
        let owner = actor(&mut objects, Some(authored_paths::LINKED_PROTECTION_EFFECT));
        let linked = actor(&mut objects, None);
        let selected = actor(&mut objects, None);
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        objects.get_mut(selected).unwrap().base.position = crate::Vector3 {
            x: 400,
            y: -700,
            z: 900,
        };
        let mut world = ScenePathWorld::new(RandomState::default());
        world.primary_player = Some(selected);
        world.scene.player_configuration = Some(0);
        world.surface_mode = Some(SurfaceMode { flags: 1 });
        world.player_service_flags = Some(PlayerServiceFlags::from_bits(0));
        world.contacts_enabled = Some(true);
        world.linked_effect_activity = Some(LinkedEffectActivity {
            recent_spawn: initial_activity,
        });
        world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::Other; 2],
            markers: None,
        });
        world
            .bind_player(
                &objects,
                linked,
                PlayerPathRecords {
                    protection: Some(DeflectionProtection::from_control(10)),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut invocation = PathInvocation::default();
        let first_callback = if initial_activity == 0 { 8 } else { 0 };
        for tick in 0..=first_callback + 7 {
            if tick == first_callback + 3 {
                world.player_mut(&objects, linked).unwrap().protection =
                    Some(DeflectionProtection::from_control(1));
            }
            if tick == first_callback + 6 {
                world.player_mut(&objects, linked).unwrap().protection =
                    Some(DeflectionProtection::from_control(0xE0));
            }
            visit(&mut invocation, &catalog, &mut objects, &mut world, owner);
            let effect = objects.get(owner).unwrap();
            assert_eq!(effect.base.attachment, Some(linked));
            assert_eq!(
                effect.base.flags.remove_after_tick,
                tick == first_callback + 7
            );
            assert_eq!(
                world.linked_effect_activity.unwrap().recent_spawn,
                if tick < first_callback {
                    initial_activity
                } else {
                    0
                }
            );
            if tick >= first_callback && tick <= first_callback + 6 {
                let callbacks = (tick - first_callback + 1) as u8;
                assert_eq!(
                    effect.extension.relative_rotation.pitch.units(),
                    callbacks.wrapping_mul(8)
                );
                assert_eq!(
                    effect.extension.relative_rotation.roll.units(),
                    callbacks.wrapping_mul(6)
                );
            }
        }
        assert_eq!(
            world
                .player(&objects, linked)
                .unwrap()
                .protection
                .unwrap()
                .control(),
            0xC0
        );
    }
}
