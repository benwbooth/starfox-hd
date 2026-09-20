use super::*;
use crate::native::path_appearance::{AnimationChannels, AnimationControl};
use crate::native::path_control::PlayerTarget;
use crate::native::path_fields::{Axis, ByteField, BytePart, WordField};
use crate::native::program_resources::ProgramResources;
use crate::native::program_state::ProgramData;
use crate::{Angle, Behavior, ObjectKind, PathCursor, PathId, ShapeId};

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    }
}

#[test]
fn mode_changes_only_its_bit_and_drives_initializer_and_warning_views() {
    for flags in 0..=u16::MAX {
        for enabled in [false, true] {
            let mut mode = ViewTransitionMode { flags };
            mode.set_active(enabled);
            assert_eq!(mode.flags, if enabled { flags | 2 } else { flags & !2 });
            assert_eq!(mode.active(), enabled);
            let defaults = mode.spawn_defaults(crate::ObjectSpawnDefaults {
                group: flags as u8,
                run_when_paused: !enabled,
            });
            assert_eq!(
                defaults,
                crate::ObjectSpawnDefaults {
                    group: flags as u8,
                    run_when_paused: enabled
                }
            );
            let mut expected = crate::proximity_warning::WarningControl {
                globally_disabled: !enabled,
                movement_mode: flags as u8,
                inhibited: flags & 4 != 0,
                transition_mode: (flags >> 8) as u8,
                transition_ready: flags & 8 != 0,
            };
            let actual = mode.warning_control(expected);
            expected.globally_disabled = enabled;
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn cleanup_matches_every_class_and_preserves_non_target_state_and_pool() {
    for class in 0..=u8::MAX {
        for flags in 0..64 {
            let mut objects = ObjectStore::new();
            let mut target = Object::new(ObjectKind::Scenery, ShapeId::EMPTY, Behavior::Effect);
            target.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(class);
            target.base.contacts.weapon_formatted = class & 2 != 0;
            target.base.contacts.first_strategy_visit = class & 4 != 0;
            target.base.contacts.suppress_attack_damage = class & 1 != 0;
            target.base.contacts.credits_hit_side = class & 8 != 0;
            target.base.contacts.mutually_non_damaging = class & 128 != 0;
            target.base.flags.visible = flags & 1 != 0;
            target.base.flags.collision_disabled = flags & 2 != 0;
            target.base.flags.general_search_eligible = flags & 4 != 0;
            target.base.contacts.run_when_paused = flags & 8 != 0;
            target.base.flags.remove_after_tick = flags & 16 != 0;
            target.base.hit_points = if flags & 32 != 0 { 0 } else { 231 };
            target.base.attack_power = 157;
            target.extension.path_state.motion_delta = Vector3 {
                x: -32768,
                y: 299,
                z: -8192,
            };
            let id = objects.allocate(target).unwrap();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .unwrap();
            objects.get_mut(id).unwrap().base.linked_object = Some(other);
            objects
                .get_mut(other)
                .unwrap()
                .base
                .contacts
                .exclusion_groups = ExclusionGroups::from_authored_class(0x40);
            let mut expected = objects.clone();
            if class & 0x50 == 0x50 || class & 8 != 0 {
                let actor = expected.get_mut(id).unwrap();
                actor.base.contacts.run_when_paused = true;
                actor.base.flags.collision_disabled = true;
                actor.base.hit_points = 0;
            }
            disable_projectiles(&mut objects);
            assert_eq!(objects, expected, "class={class}, flags={flags}");
            disable_projectiles(&mut objects);
            assert_eq!(objects, expected);
        }
    }
    disable_projectiles(&mut ObjectStore::new());
}

#[test]
fn snapshot_restores_base_fields_but_preserves_live_extensions_and_allocations() {
    let mut objects = ObjectStore::new();
    let id = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let mut resources: ProgramResources<ProgramData> = ProgramResources::default();
    let original = objects.get_mut(id).unwrap();
    original.base.position = Vector3 {
        x: -32768,
        y: 1234,
        z: 32767,
    };
    original.base.velocity = Vector3 {
        x: -79,
        y: 8192,
        z: -3,
    };
    original.base.pitch = Angle::from_units(29);
    original.base.view_rear_distance = -12345;
    original.base.yaw = Angle::from_units(193);
    original.base.roll = Angle::from_units(137);
    original.base.child_number = 17;
    original.base.path = Some(cursor(7, 2));
    original.base.hit_points = 171;
    original.base.attack_power = 253;
    original.base.wait_timer = 39;
    original.base.contacts.hit_by_secondary = true;
    original.base.contacts.pending_hit = true;
    original.base.contacts.new_contact_latched = true;
    original.base.flags.maximum_draw_distance = true;
    let path = &mut original.extension.path_state;
    path.needs_path_initialization = true;
    path.script_parameter = 13;
    path.weapon_selection = 203;
    path.friend_health_slot = 9;
    path.hold_latched = true;
    path.motion = MotionSettings {
        refresh_child_chain: true,
        suppress_child_refresh: true,
        carry_selected_player: true,
        follow_player_displacement: true,
        generate_velocity_each_step: true,
        bank_turn: true,
        attached_coordinates: true,
        relative_coordinates: true,
        quadruple_velocity: true,
    };
    path.platform_carry.saved_position = Vector3 {
        x: 32,
        y: -169,
        z: 2048,
    };
    path.clear_on_path_exit_latch = true;
    path.conditions.hit_event_pending = true;
    path.conditions.selected_player = PlayerTarget::Secondary;
    path.conditions.crossing.sample([Some(-1), Some(1)]);
    path.repeat_counter = 249;
    let original = original.clone();
    let snapshot = ViewBaseSnapshot::capture(&original);

    let mut live = Object::new(
        ObjectKind::Effect,
        ShapeId::from_catalog_index(10),
        Behavior::FollowPath,
    );
    live.extension.relative_position = Vector3 {
        x: 900,
        y: -910,
        z: 920,
    };
    live.extension.parent = Some(id);
    live.extension.path_state.motion_phase = 0xBEEF;
    live.extension.path_state.motion_delta = Vector3 {
        x: -255,
        y: 720,
        z: 777,
    };
    live.extension.path_state.script_value = 0xBACA;
    live.extension.path_state.part = 231;
    live.extension.path_state.animation = AnimationChannels {
        shape: AnimationControl::from_packed(0xFA),
        color: AnimationControl::from_packed(0x43),
    };
    live.extension.depth_offset = 0x9876;
    live.extension.spawn_group = 137;
    live.extension.texture_scroll_x = 237;
    live.extension.texture_scroll_y = 193;
    live.extension.color_frame = 111;
    live.extension.animation_frame = 37;
    live.extension
        .auxiliary
        .set(
            &mut resources,
            id,
            AuxiliaryRecord::SceneContinuation(cursor(6, 91)),
        )
        .unwrap();
    live.extension
        .path_state
        .stack
        .begin(&mut resources, id, cursor(6, 18), 79)
        .unwrap();
    let live_extensions = live.extension.clone();
    let live_resources = resources.clone();
    snapshot.restore(&mut live);
    assert_eq!(live.base, original.base);
    // Expected rollback is stated independently, including the current live
    // stack allocation and every extension field not covered by the copy.
    let mut expected = original.clone();
    expected.extension = live_extensions;
    expected.extension.path_state.needs_path_initialization = true;
    expected.extension.path_state.script_parameter = 13;
    expected.extension.path_state.weapon_selection = 203;
    expected.extension.path_state.friend_health_slot = 9;
    expected.extension.path_state.hold_latched = true;
    expected.extension.path_state.motion = original.extension.path_state.motion;
    expected.extension.path_state.platform_carry.saved_position = Vector3 {
        x: 32,
        y: -169,
        z: 2048,
    };
    expected.extension.path_state.clear_on_path_exit_latch = true;
    expected.extension.path_state.conditions = original.extension.path_state.conditions;
    expected.extension.path_state.repeat_counter = 249;
    assert_eq!(live, expected);
    assert_eq!(resources, live_resources);
    assert_eq!(ByteField::RepeatCounter.read(&live), 249);
    assert_eq!(
        WordField::SavedPosition(Axis::Y).read(&live),
        (-169i16) as u16
    );
    assert_eq!(
        ByteField::WordPart {
            field: WordField::MotionDelta(Axis::X),
            part: BytePart::Low
        }
        .read(&live),
        1
    );
    assert_eq!(
        live.extension
            .path_state
            .conditions
            .crossing
            .sample([Some(-1), Some(-1)]),
        Some(PlayerTarget::Secondary)
    );
}

#[test]
fn view_capture_observes_cleanup_and_restore_does_not_undo_live_motion_work() {
    let mut objects = ObjectStore::new();
    let id = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let view = objects.get_mut(id).unwrap();
    view.base.hit_points = 199;
    view.base.contacts.exclusion_groups = ExclusionGroups::HIT_SIDE_CLASS;
    disable_projectiles(&mut objects);
    let snapshot = ViewBaseSnapshot::capture(objects.get(id).unwrap());
    let view = objects.get_mut(id).unwrap();
    view.base.hit_points = 79;
    view.base.contacts.run_when_paused = false;
    view.base.flags.collision_disabled = false;
    view.extension.path_state.motion_delta.x = 2077;
    snapshot.restore(view);
    assert_eq!(view.base.hit_points, 0);
    assert!(view.base.contacts.run_when_paused);
    assert!(view.base.flags.collision_disabled);
    assert_eq!(view.extension.path_state.motion_delta.x, 2077);
}

#[test]
fn repeated_saves_retain_owned_payloads_and_restore_keeps_a_stale_entry() {
    use crate::program_resources::PROGRAM_CAPACITY;
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    let mut pool = ProgramResources::default();
    let mut auxiliary = ActorAuxiliary::default();
    let mut view = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    let untouched = view.clone();
    assert_eq!(
        restore_view(&mut pool, &auxiliary, owner, &mut view),
        Ok(false)
    );
    assert_eq!(view, untouched);
    for count in 1..=3 {
        view.base.hit_points = count;
        save_view(&mut pool, &mut auxiliary, owner, &view).unwrap();
        assert_eq!(
            pool.available_capacity(),
            PROGRAM_CAPACITY - 10 - u16::from(count) * 68
        );
        assert_eq!(pool.owner_count(owner), usize::from(count) + 1);
    }
    let table_before = auxiliary.clone();
    let entries_before = auxiliary.entries(&pool, owner).unwrap().to_vec();
    view.base.hit_points = 254;
    assert_eq!(
        restore_view(&mut pool, &auxiliary, owner, &mut view),
        Ok(true)
    );
    assert_eq!(view.base.hit_points, 3);
    assert_eq!(pool.owner_count(owner), 3);
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 10 - 2 * 68);
    assert_eq!(auxiliary, table_before);
    assert_eq!(auxiliary.entries(&pool, owner).unwrap(), entries_before);
    let before = view.clone();
    let pool_before = pool.clone();
    assert_eq!(
        restore_view(&mut pool, &auxiliary, owner, &mut view),
        Err(ViewSaveError::MissingSavedView)
    );
    assert_eq!(view, before);
    assert_eq!(pool, pool_before);
    view.base.hit_points = 79;
    save_view(&mut pool, &mut auxiliary, owner, &view).unwrap();
    view.base.hit_points = 0;
    assert_eq!(
        restore_view(&mut pool, &auxiliary, owner, &mut view),
        Ok(true)
    );
    assert_eq!(view.base.hit_points, 79);
    assert_eq!(pool.owner_count(owner), 3);
    pool.release_owner(owner);
    auxiliary.clear_after_owner_release(&pool, owner).unwrap();
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY);
}

#[test]
fn failed_table_publication_leaves_new_payload_owned_and_old_continuation_intact() {
    use crate::program_resources::PROGRAM_CAPACITY;
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    let mut pool = ProgramResources::default();
    let mut auxiliary = ActorAuxiliary::default();
    auxiliary
        .set(
            &mut pool,
            owner,
            AuxiliaryRecord::SceneContinuation(cursor(8, 7)),
        )
        .unwrap();
    // A successful 68-cost snapshot leaves 8, insufficient for a 14-cost
    // replacement table even though freeing the old table would fund it.
    let reserve = pool
        .allocate_shared(
            PROGRAM_CAPACITY - 10 - 76 - 2,
            ProgramData::PathStack(Default::default()),
        )
        .unwrap();
    let view = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    let table_before = auxiliary.clone();
    assert_eq!(
        save_view(&mut pool, &mut auxiliary, owner, &view),
        Err(ViewSaveError::Auxiliary(AuxiliaryError::Allocation(
            AllocationFailure::NoContiguousFit
        )))
    );
    assert_eq!(pool.available_capacity(), 8);
    assert_eq!(pool.owner_count(owner), 2);
    assert_eq!(auxiliary, table_before);
    assert_eq!(
        auxiliary.find(&pool, owner, AuxiliaryKind::SavedView),
        Ok(None)
    );
    assert_eq!(
        auxiliary.find(&pool, owner, AuxiliaryKind::SceneContinuation),
        Ok(Some(AuxiliaryRecord::SceneContinuation(cursor(8, 7))))
    );
    let before = pool.clone();
    assert_eq!(
        save_view(&mut pool, &mut auxiliary, owner, &view),
        Err(ViewSaveError::Allocation(
            AllocationFailure::NoContiguousFit
        ))
    );
    assert_eq!(pool, before);
    pool.release_shared(reserve).unwrap();
    save_view(&mut pool, &mut auxiliary, owner, &view).unwrap();
    assert_eq!(pool.owner_count(owner), 3);
    assert_eq!(pool.available_capacity(), PROGRAM_CAPACITY - 14 - 2 * 68);
}
